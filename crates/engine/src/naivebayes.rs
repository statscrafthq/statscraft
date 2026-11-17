//! NAIVEBAYES: naive Bayes classification with forward selection of the predictors.
//!
//! Rules, in our words, from the Command Syntax Reference and the Algorithms manual:
//! - Predictors are the factors after BY and the covariates after WITH, or without either every
//!   other variable by its level (strings, nominal and ordinal ones as factors). The target, the
//!   weight and the TRAININGSAMPLE variable never are; EXCEPT takes names off; FORCE puts
//!   predictors into every subset.
//! - Training cases come from TRAININGSAMPLE (a random PERCENT, or VARIABLE equal to 1); the
//!   rest are test cases. A case without a valid target or a usable weight, or with every
//!   predictor missing, is left out; a predictor missing or constant over the training cases is
//!   left out. Weights are rounded to whole numbers.
//! - A covariate is cut at BINS - 1 equal-width points between its training minimum and maximum
//!   into intervals (lower bound open, upper closed, the end ones unbounded); a run of empty inner
//!   intervals is merged by putting one point halfway between the bounds of its non-empty
//!   neighbours.
//! - With N the training cases and lambda = f = 1/N, a prior is (N_k + lambda) / (N + K lambda)
//!   and a conditional (N_jmk + f) / (N_jk + M_j f), N_jk counting the cases of category k with
//!   predictor j present. A case's missing (or in the test sample unseen) predictors drop out of
//!   its product.
//! - Selection adds the predictor that raises the training mean log-likelihood most, up to
//!   EXACTSIZE or the MAXSIZE limit; of the subsets passed the one with the least pseudo-BIC
//!   (-l + J ln(N) / 2N) or test criterion (-l of the test cases) wins.
//! - A case goes to the category of highest posterior; ties go to the higher prior.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::npar_tests::text;
use crate::pass::default_measure;
use crate::twostep::{category_codes, free_name, key_json, numeric_value, plain_table};
use crate::{Cell, Engine, Error, Table};
use serde_json::json;
use statscraft_data::Measure;
use statscraft_syntax::{Naivebayes, NbSubset, NbTraining, VarItem};

struct Predictor {
    name: String,
    scale: bool,
    /// Categorical: the categories seen in training.
    levels: Vec<Key>,
    /// Scale: the inner bin bounds.
    bounds: Vec<f64>,
    /// The category of every case; None where missing or not seen in training.
    codes: Vec<Option<usize>>,
}

impl Predictor {
    fn categories(&self) -> usize {
        match self.scale {
            true => self.bounds.len() + 1,
            false => self.levels.len(),
        }
    }
}

/// The bin of `x`: the number of bounds below it.
fn bin_of(bounds: &[f64], x: f64) -> usize {
    bounds.partition_point(|b| *b < x)
}

/// Equal-width bounds over the values, with every run of empty inner bins merged into one
/// bound halfway between its non-empty neighbours. Empty for a constant.
fn bin_bounds(values: &[f64], count: usize) -> Vec<f64> {
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if min >= max {
        return Vec::new();
    }
    let width = (max - min) / count as f64;
    let mut bounds: Vec<f64> = (1..count).map(|i| min + i as f64 * width).collect();
    loop {
        let mut filled = vec![false; bounds.len() + 1];
        for &x in values {
            filled[bin_of(&bounds, x)] = true;
        }
        let Some(first) = (1..bounds.len()).find(|&t| !filled[t]) else {
            return bounds;
        };
        let last = (first..bounds.len())
            .take_while(|&t| !filled[t])
            .last()
            .expect("one empty bin");
        let middle = (bounds[first - 1] + bounds[last]) / 2.0;
        bounds.splice(first - 1..=last, [middle]);
    }
}

fn log_sum_exp(values: &[f64]) -> f64 {
    let top = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    top + values.iter().map(|v| (v - top).exp()).sum::<f64>().ln()
}

fn percent(part: f64, whole: f64) -> Cell {
    number((whole > 0.0).then(|| 100.0 * part / whole))
}

impl Engine {
    pub(crate) fn naivebayes(&mut self, command: &Naivebayes) -> Result<Vec<Table>, Error> {
        let started = std::time::Instant::now();
        let data = self.active()?;
        let cases = data.case_count();
        let mut warnings: Vec<String> = Vec::new();
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        let target = data.variable(&command.dependent)?;
        let target_name = target.name().to_string();
        let weight_name = data.weight_variable().map(str::to_string);
        if weight_name
            .as_deref()
            .is_some_and(|w| same(w, &target_name))
        {
            return Err(Error(format!(
                "NAIVEBAYES: the target {target_name} is the weight variable"
            )));
        }
        let scale_min = self.scale_min();
        let measure = |name: &str| {
            let index = data.variable_index(name).expect("known");
            match data.variable_at(index).measure() {
                Measure::Unknown => default_measure(data, index, scale_min),
                known => known,
            }
        };
        if !target.is_string() && measure(&target_name) == Measure::Scale {
            return Err(Error(format!(
                "NAIVEBAYES: the target {target_name} must be nominal or ordinal (see VARIABLE LEVEL)"
            )));
        }
        let training_name = match &command.training {
            Some(NbTraining::Variable(name)) => {
                let variable = data.variable(name)?;
                let name = variable.name().to_string();
                if variable.is_string() {
                    return Err(Error(format!(
                        "NAIVEBAYES: the TRAININGSAMPLE variable {name} must be numeric"
                    )));
                }
                if same(&name, &target_name)
                    || weight_name.as_deref().is_some_and(|w| same(w, &name))
                {
                    return Err(Error(format!(
                        "NAIVEBAYES: the TRAININGSAMPLE variable {name} cannot be the target or the weight"
                    )));
                }
                Some(name)
            }
            Some(NbTraining::Percent(_)) if weight_name.is_some() => {
                return Err(Error(
                    "NAIVEBAYES: TRAININGSAMPLE PERCENT cannot be used with a weight variable"
                        .into(),
                ));
            }
            _ => None,
        };
        let special = |n: &str| {
            same(n, &target_name)
                || weight_name.as_deref().is_some_and(|w| same(w, n))
                || training_name.as_deref().is_some_and(|t| same(t, n))
        };
        let unique = |items: &[VarItem]| -> Result<Vec<String>, Error> {
            let mut out: Vec<String> = Vec::new();
            for n in self.expand(items)? {
                if !out.iter().any(|o| same(o, &n)) {
                    out.push(n);
                }
            }
            Ok(out)
        };

        // the predictor lists
        let forced_factors = unique(&command.force_factors)?;
        let forced_covariates = unique(&command.force_covariates)?;
        let forced: Vec<String> = forced_factors
            .iter()
            .chain(&forced_covariates)
            .cloned()
            .collect();
        if let Some(bad) = forced.iter().find(|n| special(n)) {
            return Err(Error(format!(
                "NAIVEBAYES: FORCE cannot name {bad}, the target, the weight or the TRAININGSAMPLE variable"
            )));
        }
        if let Some(both) = forced_factors
            .iter()
            .find(|f| forced_covariates.iter().any(|c| same(c, f)))
        {
            return Err(Error(format!(
                "NAIVEBAYES: {both} is both a forced factor and a forced covariate"
            )));
        }
        let mut lists = [&command.factors, &command.covariates].map(|list| match list {
            Some(items) if items.is_empty() => None,
            other => other.clone(),
        });
        for (keyword, (given, kept)) in ["BY", "WITH"]
            .iter()
            .zip([&command.factors, &command.covariates].iter().zip(&lists))
        {
            if given.is_some() && kept.is_none() {
                warnings.push(format!(
                    "NAIVEBAYES: {keyword} names no variables; it is ignored"
                ));
            }
        }
        let (mut factors, mut covariates) = if lists.iter().any(Option::is_some) {
            let [factors, covariates] =
                std::mem::take(&mut lists).map(|list| unique(list.as_deref().unwrap_or(&[])));
            let (factors, covariates) = (factors?, covariates?);
            if let Some(both) = factors
                .iter()
                .find(|f| covariates.iter().any(|c| same(c, f)))
            {
                return Err(Error(format!(
                    "NAIVEBAYES: {both} is both a factor and a covariate"
                )));
            }
            (factors, covariates)
        } else {
            let (mut factors, mut covariates) = (Vec::new(), Vec::new());
            for index in 0..data.variable_count() {
                let variable = data.variable_at(index);
                let name = variable.name().to_string();
                match !variable.is_string() && measure(&name) == Measure::Scale {
                    true => covariates.push(name),
                    false => factors.push(name),
                }
            }
            (factors, covariates)
        };
        let except = self.expand(&command.except)?;
        for list in [&mut factors, &mut covariates] {
            list.retain(|n| {
                !special(n)
                    && !except.iter().any(|e| same(e, n))
                    && !forced.iter().any(|f| same(f, n))
            });
        }
        let listed = forced.len() + factors.len() + covariates.len();
        match command.subset {
            NbSubset::Max { size: Some(n), .. } if n > listed => {
                return Err(Error(format!(
                    "NAIVEBAYES: MAXSIZE={n} is more than the {listed} predictors"
                )));
            }
            NbSubset::Exact(n) if n >= listed => {
                return Err(Error(format!(
                    "NAIVEBAYES: EXACTSIZE={n} must be below the number of predictors ({listed})"
                )));
            }
            _ => {}
        }
        let kinds: Vec<(String, bool)> = forced_factors
            .iter()
            .map(|n| (n.clone(), false))
            .chain(forced_covariates.iter().map(|n| (n.clone(), true)))
            .chain(factors.iter().map(|n| (n.clone(), false)))
            .chain(covariates.iter().map(|n| (n.clone(), true)))
            .collect();
        if kinds.is_empty() {
            return Err(Error("NAIVEBAYES has no predictors".into()));
        }

        // cases: weights, samples and the target
        let weights: Vec<f64> = match case_weights(data)? {
            Some(w) => w.iter().map(|w| w.round()).collect(),
            None => vec![1.0; cases],
        };
        let in_training: Vec<bool> = match &command.training {
            None => vec![true; cases],
            Some(NbTraining::Percent(p)) => {
                let mut random = self.random.borrow_mut();
                (0..cases).map(|_| random.uniform() < p / 100.0).collect()
            }
            Some(NbTraining::Variable(name)) => {
                let variable = data.variable(name)?;
                (0..cases)
                    .map(|c| numeric_value(&variable, c, true) == Some(1.0))
                    .collect()
            }
        };
        let include = command.include_user_missing;
        let target_keys = categories(&target, include);
        let usable = |c: usize| weights[c] > 0.0 && target_keys[c].is_some();

        // the predictors, coded over the training cases
        let mut predictors: Vec<Predictor> = Vec::new();
        let mut excluded: Vec<(String, bool, &str)> = Vec::new();
        let forced_count = forced.len();
        let mut forced_kept = 0;
        for (at, (name, scale)) in kinds.iter().enumerate() {
            let variable = data.variable(name)?;
            let predictor = if *scale {
                if variable.is_string() {
                    return Err(Error(format!(
                        "NAIVEBAYES: the covariate {name} is a string variable"
                    )));
                }
                let values: Vec<Option<f64>> = (0..cases)
                    .map(|c| numeric_value(&variable, c, false))
                    .collect();
                let seen: Vec<f64> = (0..cases)
                    .filter(|&c| in_training[c] && usable(c))
                    .filter_map(|c| values[c])
                    .collect();
                let bounds = match seen.is_empty() {
                    true => Vec::new(),
                    false => bin_bounds(&seen, command.bins),
                };
                let empty = seen.is_empty();
                let codes = values
                    .iter()
                    .map(|v| v.filter(|_| !empty).map(|x| bin_of(&bounds, x)))
                    .collect();
                (
                    empty,
                    Predictor {
                        name: name.clone(),
                        scale: true,
                        levels: Vec::new(),
                        bounds,
                        codes,
                    },
                )
            } else {
                let keys = categories(&variable, include);
                let seen: Vec<Option<Key>> = (0..cases)
                    .filter(|&c| in_training[c] && usable(c))
                    .map(|c| keys[c].clone())
                    .collect();
                let (levels, _) = category_codes(&seen);
                let codes = keys
                    .iter()
                    .map(|k| {
                        k.as_ref()
                            .and_then(|k| levels.iter().position(|l| l.compare(k).is_eq()))
                    })
                    .collect();
                (
                    levels.is_empty(),
                    Predictor {
                        name: name.clone(),
                        scale: false,
                        levels,
                        bounds: Vec::new(),
                        codes,
                    },
                )
            };
            let (empty, predictor) = predictor;
            if empty || predictor.categories() < 2 {
                let reason = match empty {
                    true => "Missing in every case",
                    false => "Constant",
                };
                excluded.push((name.clone(), *scale, reason));
                continue;
            }
            if at < forced_count {
                forced_kept += 1;
            }
            predictors.push(predictor);
        }
        if !excluded.is_empty() {
            let names: Vec<&str> = excluded.iter().map(|e| e.0.as_str()).collect();
            warnings.push(format!(
                "NAIVEBAYES leaves out {}: missing or constant over the training cases",
                names.join(", ")
            ));
        }
        if predictors.is_empty() {
            return Err(Error(
                "NAIVEBAYES has no predictor left that varies over the training cases".into(),
            ));
        }
        let any_predictor = |c: usize| predictors.iter().any(|p| p.codes[c].is_some());
        let training_cases: Vec<usize> = (0..cases)
            .filter(|&c| in_training[c] && usable(c) && any_predictor(c))
            .collect();
        let seen: Vec<Option<Key>> = training_cases
            .iter()
            .map(|&c| target_keys[c].clone())
            .collect();
        let (classes, _) = category_codes(&seen);
        let k = classes.len();
        if k < 2 {
            return Err(Error(format!(
                "NAIVEBAYES needs at least two categories of {target_name} among the training cases, found {k}"
            )));
        }
        let class: Vec<Option<usize>> = target_keys
            .iter()
            .map(|t| {
                t.as_ref()
                    .and_then(|t| classes.iter().position(|l| l.compare(t).is_eq()))
            })
            .collect();
        let test_cases: Vec<usize> = (0..cases)
            .filter(|&c| !in_training[c] && usable(c) && any_predictor(c) && class[c].is_some())
            .collect();

        // the model: priors and conditional probabilities
        let n: f64 = training_cases.iter().map(|&c| weights[c]).sum();
        let smooth = 1.0 / n;
        let mut class_counts = vec![0.0; k];
        for &c in &training_cases {
            class_counts[class[c].expect("training class")] += weights[c];
        }
        let priors: Vec<f64> = class_counts
            .iter()
            .map(|nk| (nk + smooth) / (n + k as f64 * smooth))
            .collect();
        let log_prior: Vec<f64> = priors.iter().map(|p| p.ln()).collect();
        let mut counts: Vec<Vec<Vec<f64>>> = Vec::new();
        let mut probabilities: Vec<Vec<Vec<f64>>> = Vec::new();
        for predictor in &predictors {
            let m = predictor.categories();
            let mut table = vec![vec![0.0; k]; m];
            for &c in &training_cases {
                if let Some(code) = predictor.codes[c] {
                    table[code][class[c].expect("training class")] += weights[c];
                }
            }
            let totals: Vec<f64> = (0..k).map(|y| table.iter().map(|r| r[y]).sum()).collect();
            probabilities.push(
                table
                    .iter()
                    .map(|row| {
                        (0..k)
                            .map(|y| (row[y] + smooth) / (totals[y] + m as f64 * smooth))
                            .collect::<Vec<f64>>()
                    })
                    .collect::<Vec<_>>(),
            );
            counts.push(table);
        }
        let log_p: Vec<Vec<Vec<f64>>> = probabilities
            .iter()
            .map(|t| {
                t.iter()
                    .map(|r| r.iter().map(|p| p.ln()).collect())
                    .collect()
            })
            .collect();
        // the log of prior times product for each case and class, over a set of predictors
        let joint = |c: usize, set: &[usize]| -> Vec<f64> {
            let mut out = log_prior.clone();
            for &j in set {
                if let Some(code) = predictors[j].codes[c] {
                    for (y, o) in out.iter_mut().enumerate() {
                        *o += log_p[j][code][y];
                    }
                }
            }
            out
        };
        let mean_log_likelihood = |rows: &[Vec<f64>], sample: &[usize]| -> f64 {
            let mut total = 0.0;
            let mut weight = 0.0;
            for (row, &c) in rows.iter().zip(sample) {
                let y = class[c].expect("analyzed class");
                total += weights[c] * (row[y] - log_sum_exp(row));
                weight += weights[c];
            }
            total / weight
        };

        // selection
        let initial: Vec<usize> = (0..forced_kept).collect();
        let total = predictors.len();
        let stop = match command.subset {
            NbSubset::NoSelection => total,
            NbSubset::Exact(more) => forced_kept + more,
            NbSubset::Max {
                size: Some(more), ..
            } => forced_kept + more,
            NbSubset::Max { size: None, .. } => forced_kept + 100.min(20.max(total / 5)),
        };
        if stop > total && matches!(command.subset, NbSubset::Exact(_)) {
            warnings.push(format!(
                "NAIVEBAYES: only {} predictors can be added, fewer than EXACTSIZE",
                total - forced_kept
            ));
        }
        let stop = stop.min(total);
        let selecting = command.subset != NbSubset::NoSelection;
        let mut chosen = initial.clone();
        let mut train_rows: Vec<Vec<f64>> =
            training_cases.iter().map(|&c| joint(c, &chosen)).collect();
        let mut test_rows: Vec<Vec<f64>> = test_cases.iter().map(|&c| joint(c, &chosen)).collect();
        // per step: the predictor added and the training and test mean log-likelihoods
        let mut steps: Vec<(Option<usize>, f64, Option<f64>)> = Vec::new();
        let test_ll = |rows: &[Vec<f64>]| {
            (!test_cases.is_empty()).then(|| mean_log_likelihood(rows, &test_cases))
        };
        steps.push((
            None,
            mean_log_likelihood(&train_rows, &training_cases),
            test_ll(&test_rows),
        ));
        let add = |rows: &mut [Vec<f64>], sample: &[usize], j: usize| {
            for (row, &c) in rows.iter_mut().zip(sample) {
                if let Some(code) = predictors[j].codes[c] {
                    for (y, o) in row.iter_mut().enumerate() {
                        *o += log_p[j][code][y];
                    }
                }
            }
        };
        if !selecting {
            chosen = (0..total).collect();
        }
        while selecting && chosen.len() < stop {
            if command.timer > 0.0 && started.elapsed().as_secs_f64() > command.timer * 60.0 {
                return Err(Error(format!(
                    "NAIVEBAYES stopped after {} minutes (CRITERIA TIMER)",
                    command.timer
                )));
            }
            let mut best: Option<(usize, f64)> = None;
            for j in (0..total).filter(|j| !chosen.contains(j)) {
                let mut rows = train_rows.clone();
                add(&mut rows, &training_cases, j);
                let ll = mean_log_likelihood(&rows, &training_cases);
                if best.is_none_or(|b| ll > b.1) {
                    best = Some((j, ll));
                }
            }
            let (j, ll) = best.expect("a candidate");
            add(&mut train_rows, &training_cases, j);
            add(&mut test_rows, &test_cases, j);
            chosen.push(j);
            steps.push((Some(j), ll, test_ll(&test_rows)));
        }
        let partitioned = command.training.is_some();
        let mut by_test = match command.subset {
            NbSubset::Max {
                test: Some(true), ..
            } if !partitioned => {
                return Err(Error(
                    "NAIVEBAYES: BESTSUBSET=TESTDATA needs test cases from TRAININGSAMPLE".into(),
                ));
            }
            NbSubset::Max { test, .. } => test.unwrap_or(partitioned),
            _ => false,
        };
        if by_test && test_cases.is_empty() {
            warnings.push("NAIVEBAYES has no test cases; the pseudo-BIC chooses the subset".into());
            by_test = false;
        }
        let criterion = |step: usize| {
            let (_, train, test) = steps[step];
            match by_test {
                true => -test.expect("test cases"),
                false => -train + 0.5 * (initial.len() + step) as f64 * n.ln() / n,
            }
        };
        let best_step = match command.subset {
            NbSubset::Max { .. } => (0..steps.len())
                .reduce(|a, b| if criterion(b) < criterion(a) { b } else { a })
                .expect("a step"),
            _ => steps.len() - 1,
        };
        if selecting {
            chosen.truncate(initial.len() + best_step);
        }

        // classification
        let posterior = |c: usize| -> Vec<f64> {
            let row = joint(c, &chosen);
            let total = log_sum_exp(&row);
            row.iter().map(|v| (v - total).exp()).collect()
        };
        let predict = |probabilities: &[f64]| -> usize {
            let mut best = 0;
            for y in 1..k {
                let (p, q) = (probabilities[y], probabilities[best]);
                if p > q + 1e-12 || ((p - q).abs() <= 1e-12 && log_prior[y] > log_prior[best]) {
                    best = y;
                }
            }
            best
        };
        let confusion = |sample: &[usize]| -> Vec<Vec<f64>> {
            let mut table = vec![vec![0.0; k]; k];
            for &c in sample {
                table[class[c].expect("analyzed class")][predict(&posterior(c))] += weights[c];
            }
            table
        };

        // tables
        let shown = self.shown();
        let class_label = |y: usize| shown.value(&target, &classes[y]);
        let mut tables = Vec::new();
        if command.print_cps {
            let sum = |sample: &[usize]| sample.iter().map(|&c| weights[c]).sum::<f64>();
            let all: f64 = weights.iter().sum();
            let (train, test) = (sum(&training_cases), sum(&test_cases));
            let mut rows = vec![vec![
                text("Included"),
                text("Training"),
                Cell::Number(train),
                percent(train, all),
            ]];
            if partitioned {
                rows.push(vec![
                    Cell::Empty,
                    text("Test"),
                    Cell::Number(test),
                    percent(test, all),
                ]);
            }
            let left = all - train - test;
            rows.push(vec![
                text("Excluded"),
                Cell::Empty,
                Cell::Number(left),
                percent(left, all),
            ]);
            rows.push(vec![
                text("Total"),
                Cell::Empty,
                Cell::Number(all),
                percent(all, all),
            ]);
            tables.push(plain_table(
                "Case Processing Summary",
                2,
                &["", "", "N", "Percent"],
                rows,
            ));
        }
        if command.print_excluded && !excluded.is_empty() {
            let rows = excluded
                .iter()
                .map(|(name, scale, reason)| {
                    let kind = if *scale { "Scale" } else { "Categorical" };
                    vec![text(name.as_str()), text(kind), text(*reason)]
                })
                .collect();
            tables.push(plain_table(
                "Excluded Predictors",
                1,
                &["Predictor", "Type", "Reason"],
                rows,
            ));
        }
        if command.print_summary && selecting {
            let mut columns = vec![
                "Step",
                "Predictor Added",
                "Predictors",
                "Training Mean Log-likelihood",
            ];
            let with_test = !test_cases.is_empty();
            if with_test {
                columns.push("Test Mean Log-likelihood");
            }
            let max = matches!(command.subset, NbSubset::Max { .. });
            if max {
                columns.push(if by_test {
                    "Test Criterion"
                } else {
                    "Pseudo-BIC"
                });
            }
            let rows = steps
                .iter()
                .enumerate()
                .map(|(s, (added, train, test))| {
                    let added = match added {
                        Some(j) => text(predictors[*j].name.as_str()),
                        None => text("Initial subset"),
                    };
                    let mut row = vec![
                        Cell::Number(s as f64),
                        added,
                        Cell::Number((initial.len() + s) as f64),
                        Cell::Number(*train),
                    ];
                    if with_test {
                        row.push(number(*test));
                    }
                    if max {
                        row.push(Cell::Number(criterion(s)));
                    }
                    row
                })
                .collect();
            let mut table = plain_table("Predictor Selection Summary", 1, &columns, rows);
            table.caption = Some(format!("The subset of step {best_step} is the model."));
            tables.push(table);
        }
        if command.print_selected && selecting {
            let mut rows = Vec::new();
            for (kind, scale) in [("Categorical", false), ("Scale", true)] {
                let names: Vec<&str> = chosen
                    .iter()
                    .filter(|&&j| predictors[j].scale == scale)
                    .map(|&j| predictors[j].name.as_str())
                    .collect();
                if !names.is_empty() {
                    rows.push(vec![text(kind), text(names.join(" "))]);
                }
            }
            let mut table = plain_table("Selected Predictors", 1, &["Type", "Predictors"], rows);
            if chosen.is_empty() {
                table.caption =
                    Some("No predictor was selected: the model uses the priors only.".into());
            }
            tables.push(table);
        }
        if command.print_classification {
            let mut columns = vec!["Sample".to_string(), "Observed".to_string()];
            columns.extend((0..k).map(class_label));
            columns.push("Percent Correct".into());
            let mut groups = vec![Vec::new(), Vec::new()];
            groups.extend(vec![vec!["Predicted".to_string()]; k + 1]);
            let mut rows = Vec::new();
            let mut formats = Vec::new();
            let width = columns.len();
            let samples = [("Training", &training_cases), ("Test", &test_cases)];
            for (name, sample) in samples
                .iter()
                .filter(|(name, _)| *name == "Training" || partitioned)
            {
                let table = confusion(sample);
                let all: f64 = table.iter().flatten().sum();
                for (y, row) in table.iter().enumerate() {
                    let mut cells = vec![
                        if y == 0 { text(*name) } else { Cell::Empty },
                        text(class_label(y)),
                    ];
                    cells.extend(row.iter().map(|&w| Cell::Number(w)));
                    cells.push(percent(row[y], row.iter().sum()));
                    rows.push(cells);
                    let mut format = vec![String::new(); width];
                    format[width - 1] = "PCT6.1".into();
                    formats.push(format);
                }
                let mut overall = vec![Cell::Empty, text("Overall Percent")];
                overall.extend((0..k).map(|p| percent(table.iter().map(|r| r[p]).sum(), all)));
                overall.push(percent((0..k).map(|y| table[y][y]).sum(), all));
                rows.push(overall);
                let mut format = vec!["PCT6.1".to_string(); width];
                format[0].clear();
                format[1].clear();
                formats.push(format);
            }
            tables.push(Table {
                title: "Classification".into(),
                row_headers: 2,
                columns,
                column_groups: groups,
                rows,
                formats,
                ..Table::default()
            });
        }

        if let Some(file) = &command.outfile {
            let used: Vec<_> = chosen
                .iter()
                .map(|&j| {
                    let p = &predictors[j];
                    let mut entry = json!({
                        "name": p.name,
                        "kind": if p.scale { "scale" } else { "categorical" },
                        "counts": counts[j],
                        "probabilities": probabilities[j],
                    });
                    match p.scale {
                        true => entry["bounds"] = json!(p.bounds),
                        false => {
                            entry["categories"] =
                                json!(p.levels.iter().map(key_json).collect::<Vec<_>>())
                        }
                    }
                    entry
                })
                .collect();
            let model = json!({
                "format": "statscraft-naivebayes-model",
                "target": target_name,
                "categories": classes.iter().map(key_json).collect::<Vec<_>>(),
                "counts": class_counts,
                "priors": priors,
                "training_cases": n,
                "smoothing": smooth,
                "predictors": used,
            });
            self.write_json("NAIVEBAYES", file, "model", &model)?;
        }

        // saved variables: every case with a predictor present
        let predictions: Vec<Option<Vec<f64>>> = (0..cases)
            .map(|c| any_predictor(c).then(|| posterior(c)))
            .collect();
        let mut names: Vec<String> = Vec::new();
        let mut name_for = |given: &Option<String>, want: String| -> Result<String, Error> {
            let taken =
                |n: &str| data.variable_index(n).is_some() || names.iter().any(|m| same(m, n));
            let name = match given {
                Some(_) if data.variable_index(&want).is_some() => {
                    return Err(Error(format!(
                        "NAIVEBAYES /SAVE names {want}, a variable that already exists; choose another name"
                    )));
                }
                Some(_) if taken(&want) => {
                    return Err(Error(format!(
                        "NAIVEBAYES /SAVE gives the name {want} to two saved variables; choose another name"
                    )));
                }
                Some(_) => want,
                None => free_name(&want, taken),
            };
            names.push(name.clone());
            Ok(name)
        };
        let value_name = match &command.save_value {
            Some(given) => Some(name_for(
                given,
                given.clone().unwrap_or("PredictedValue".into()),
            )?),
            None => None,
        };
        let mut probability_names = Vec::new();
        if let Some((root, count)) = &command.save_probability {
            let base = root.clone().unwrap_or("PredictedProbability".into());
            for y in 0..(*count).min(k) {
                probability_names.push((name_for(root, format!("{base}_{}", y + 1))?, y));
            }
        }
        let label = shown.variable(&target);
        let target_format = target.format().to_string();
        let target_measure = measure(&target_name);
        let target_labels = target.value_labels().to_vec();
        let target_width = target.string_width();
        let predicted: Vec<Option<Key>> = predictions
            .iter()
            .map(|p| p.as_ref().map(|p| classes[predict(p)].clone()))
            .collect();
        let probability_labels: Vec<String> = probability_names
            .iter()
            .map(|(_, y)| format!("Predicted probability of {label} = {}", class_label(*y)))
            .collect();
        for warning in warnings {
            self.warn(warning);
        }
        if value_name.is_none() && probability_names.is_empty() {
            return Ok(tables);
        }
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            if let Some(name) = &value_name {
                match target_width {
                    Some(width) => {
                        let values = predicted
                            .iter()
                            .map(|p| match p {
                                Some(Key::Text(t)) => t.clone(),
                                _ => String::new(),
                            })
                            .collect();
                        data.set_string_values(name, width, values)?;
                    }
                    None => {
                        let values = predicted
                            .iter()
                            .map(|p| match p {
                                Some(Key::Number(x)) => Some(*x),
                                _ => None,
                            })
                            .collect();
                        data.set_numeric_values(name, values)?;
                        data.set_format(name, &target_format)?;
                    }
                }
                data.set_measure(name, target_measure)?;
                data.set_value_labels(name, target_labels)?;
                data.set_variable_label(name, Some(&format!("Predicted value of {label}")))?;
            }
            for ((name, y), label) in probability_names.iter().zip(&probability_labels) {
                let values = predictions
                    .iter()
                    .map(|p| p.as_ref().map(|p| p[*y]))
                    .collect();
                data.set_numeric_values(name, values)?;
                data.set_format(name, "F8.3")?;
                data.set_measure(name, Measure::Scale)?;
                data.set_variable_label(name, Some(label))?;
            }
            Ok(())
        })?;
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Values 0 to 10 cut in 5: bounds 2, 4, 6, 8. With 0, 1 and 10 the bins (2, 4], (4, 6] and
    /// (6, 8] are empty: bounds 2 to 8 give way to 5, halfway between them. With 0, 3 and 10
    /// the bins (4, 6] and (6, 8] are empty: bounds 4 to 8 give way to 6.
    #[test]
    fn empty_bins_merge() {
        assert_eq!(bin_bounds(&[0.0, 1.0, 10.0], 5), vec![5.0]);
        assert_eq!(bin_bounds(&[0.0, 3.0, 10.0], 5), vec![2.0, 6.0]);
        assert_eq!(
            bin_bounds(&[0.0, 2.0, 4.0, 6.0, 8.0, 10.0], 5),
            vec![2.0, 4.0, 6.0, 8.0]
        );
        assert!(bin_bounds(&[3.0, 3.0], 5).is_empty());
        assert_eq!(bin_of(&[2.0, 4.0], 2.0), 0);
        assert_eq!(bin_of(&[2.0, 4.0], 2.5), 1);
    }
}
