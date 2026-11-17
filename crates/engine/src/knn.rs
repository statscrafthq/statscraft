//! KNN: nearest neighbor classification and prediction.
//!
//! Rules, in our words, from the Command Syntax Reference and the Algorithms manual:
//! - Predictors: factors after BY (coded one column per training category, 1 in the column
//!   of the case's category) and covariates after WITH (rescaled by default to
//!   2 (x - min) / (max - min) - 1 with the training minimum and maximum). EXCEPT drops names
//!   from both lists; duplicates within a list are dropped.
//! - Cases with a missing target, factor or covariate are left out (user-missing factor and
//!   categorical target values count with USERMISSING=INCLUDE), and so are cases with a
//!   system-missing partition variable. The rest is split into training and holdout: by the
//!   partition variable (above 0 trains) or at random with the TRAINING and HOLDOUT shares.
//! - Distance: Euclidean (root of the weighted sum of squared differences) or city block (the
//!   weighted sum of absolute differences); the weight of a predictor's columns is 1, or its
//!   normalized importance with WEIGHTFEATURES=YES.
//! - A categorical target takes the most frequent category of the k neighbors (ties: the
//!   category with more training cases, then the lower value), with probabilities
//!   (count + 1) / (k + J) for the J training categories. A scale target takes the mean or
//!   the median of the neighbors.
//! - Error: the share of cases misclassified, or the sum of squared errors.
//! - NEIGHBORS=AUTO with all features: V-fold cross-validation of the training cases; each k
//!   from KMIN to KMAX gets the mean of the fold errors, and the lowest wins (ties: smaller k).
//! - FEATURES=AUTO: forward selection from the forced predictors, each step adding the
//!   predictor that gives the lowest error, until the fixed number is added or the error ratio
//!   rule stops it. With NEIGHBORS=AUTO the selection runs for each k and the k with the
//!   lowest final error wins.
//! - Importance of predictor p among m: the error without p plus 1/m, normalized to sum 1.
//! - Holdout cases with a factor category the training cases lack are not scored. WEIGHT is
//!   ignored with a warning.

use crate::crosstabs::{Key, categories};
use crate::descriptives::case_weights;
use crate::npar_tests::text;
use crate::pass::default_measure;
use crate::twostep::{free_name, key_json, numeric_value, plain_table};
use crate::{Cell, Engine, Error, Table};
use serde_json::json;
use statscraft_data::{Dataset, Measure, Value};
use statscraft_syntax::{
    Knn, KnnFeatureCount, KnnFolds, KnnLevel, KnnNeighbors, KnnPartition, KnnSave, VarItem,
};
use std::ops::Range;

/// What the target holds, over the analyzed rows.
enum Response {
    None,
    Scale(Vec<f64>),
    /// The index of each row's category among the training categories, None for a category
    /// only holdout cases have.
    Class {
        observed: Vec<Option<usize>>,
        /// Training cases per category, for breaking ties.
        counts: Vec<f64>,
    },
}

/// One predicted target.
struct Prediction {
    class: usize,
    value: f64,
    probabilities: Vec<f64>,
}

/// The coded rows and what predicting from them needs.
struct Space<'a> {
    x: &'a [Vec<f64>],
    columns: &'a [Range<usize>],
    city: bool,
    median: bool,
    response: &'a Response,
}

impl Space<'_> {
    fn distance(&self, a: usize, b: usize, features: &[usize], weights: &[f64]) -> f64 {
        let mut sum = 0.0;
        for &f in features {
            for c in self.columns[f].clone() {
                let d = self.x[a][c] - self.x[b][c];
                sum += weights[f] * if self.city { d.abs() } else { d * d };
            }
        }
        if self.city { sum } else { sum.sqrt() }
    }

    /// The `k` rows of `pool` nearest to row `q`, `q` itself left out; equal distances keep
    /// row order.
    fn neighbors(
        &self,
        q: usize,
        pool: &[usize],
        features: &[usize],
        weights: &[f64],
        k: usize,
    ) -> Vec<(usize, f64)> {
        let mut all: Vec<(usize, f64)> = pool
            .iter()
            .filter(|&&r| r != q)
            .map(|&r| (r, self.distance(q, r, features, weights)))
            .collect();
        all.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        all.truncate(k);
        all
    }

    fn predict(&self, neighbors: &[(usize, f64)]) -> Prediction {
        match self.response {
            Response::Scale(y) => {
                let mut v: Vec<f64> = neighbors.iter().map(|n| y[n.0]).collect();
                let value = if self.median {
                    v.sort_by(f64::total_cmp);
                    let h = v.len() / 2;
                    if v.len() % 2 == 1 {
                        v[h]
                    } else {
                        (v[h - 1] + v[h]) / 2.0
                    }
                } else {
                    v.iter().sum::<f64>() / v.len() as f64
                };
                Prediction {
                    class: 0,
                    value,
                    probabilities: Vec::new(),
                }
            }
            Response::Class { observed, counts } => {
                let j = counts.len();
                let mut votes = vec![0.0f64; j];
                for n in neighbors {
                    votes[observed[n.0].expect("training rows have a category")] += 1.0;
                }
                // the most votes, then the most training cases, then the lowest category
                let class = (0..j)
                    .rev()
                    .max_by(|&a, &b| {
                        votes[a]
                            .total_cmp(&votes[b])
                            .then(counts[a].total_cmp(&counts[b]))
                    })
                    .unwrap_or(0);
                let k = neighbors.len() as f64;
                Prediction {
                    class,
                    value: class as f64,
                    probabilities: votes.iter().map(|v| (v + 1.0) / (k + j as f64)).collect(),
                }
            }
            Response::None => Prediction {
                class: 0,
                value: 0.0,
                probabilities: Vec::new(),
            },
        }
    }

    /// The loss of one row: 1 when misclassified, else the squared error.
    fn loss(&self, row: usize, p: &Prediction) -> f64 {
        match self.response {
            Response::Scale(y) => (y[row] - p.value).powi(2),
            Response::Class { observed, .. } => f64::from(observed[row] != Some(p.class)),
            Response::None => 0.0,
        }
    }

    /// The error rate (a share) or the sum of squared errors of `queries` predicted from
    /// `pool`, each query left out of its own neighbors.
    fn error(
        &self,
        queries: &[usize],
        pool: &[usize],
        features: &[usize],
        weights: &[f64],
        k: usize,
    ) -> f64 {
        let total: f64 = queries
            .iter()
            .map(|&q| {
                self.loss(
                    q,
                    &self.predict(&self.neighbors(q, pool, features, weights, k)),
                )
            })
            .sum();
        match self.response {
            Response::Class { .. } => total / queries.len().max(1) as f64,
            _ => total,
        }
    }

    /// Forward selection from `forced`: the chosen predictors, each step's added predictor
    /// with the error after it, and the final error.
    fn select(
        &self,
        train: &[usize],
        forced: &[usize],
        candidates: &[usize],
        k: usize,
        rule: Rule,
    ) -> (Vec<usize>, Vec<(usize, f64)>, f64) {
        let ones = vec![1.0; self.columns.len()];
        let mut set = forced.to_vec();
        let mut error = (!set.is_empty()).then(|| self.error(train, train, &set, &ones, k));
        let mut steps = Vec::new();
        loop {
            let remaining: Vec<usize> = candidates
                .iter()
                .copied()
                .filter(|c| !set.contains(c))
                .collect();
            if remaining.is_empty()
                || matches!(rule, Rule::Fixed(n) if steps.len() >= n)
                || matches!(rule, Rule::Ratio(_)) && error == Some(0.0)
            {
                break;
            }
            let mut best: Option<(usize, f64)> = None;
            for &c in &remaining {
                let mut with = set.clone();
                with.push(c);
                let e = self.error(train, train, &with, &ones, k);
                if best.is_none_or(|b| e < b.1) {
                    best = Some((c, e));
                }
            }
            let (c, e) = best.expect("a remaining predictor");
            let mut stop = false;
            if let (Rule::Ratio(change), Some(before)) = (rule, error) {
                let ratio = (before - e).abs() / before;
                if before < e && ratio > 2.0 * change {
                    break;
                }
                stop = before >= e && ratio <= change;
            }
            set.push(c);
            steps.push((c, e));
            error = Some(e);
            if stop {
                break;
            }
        }
        let error = error.unwrap_or(f64::NAN);
        (set, steps, error)
    }

    /// The normalized importance of each of `features`: the error without it plus 1/m.
    fn importance(&self, train: &[usize], features: &[usize], k: usize) -> Vec<f64> {
        let m = features.len();
        if m == 1 {
            return vec![1.0];
        }
        let ones = vec![1.0; self.columns.len()];
        let raw: Vec<f64> = (0..m)
            .map(|i| {
                let without: Vec<usize> = (0..m).filter(|&j| j != i).map(|j| features[j]).collect();
                self.error(train, train, &without, &ones, k) + 1.0 / m as f64
            })
            .collect();
        let total: f64 = raw.iter().sum();
        raw.iter().map(|r| r / total).collect()
    }
}

/// Rows with a distance, or predictors with an error.
type Neighbors = Vec<(usize, f64)>;

#[derive(Clone, Copy)]
enum Rule {
    Fixed(usize),
    Ratio(f64),
}

/// A variable name, compared without case.
fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn sorted_levels(keys: impl Iterator<Item = Key>) -> Vec<Key> {
    let mut levels: Vec<Key> = Vec::new();
    for key in keys {
        if !levels.iter().any(|l| l.compare(&key).is_eq()) {
            levels.push(key);
        }
    }
    levels.sort_by(|a, b| a.compare(b));
    levels
}

fn position(levels: &[Key], key: &Key) -> Option<usize> {
    levels.iter().position(|l| l.compare(key).is_eq())
}

/// The value of a numeric variable at every case: user-missing values kept, system-missing
/// None.
fn raw_numbers(data: &Dataset, name: &str, what: &str) -> Result<Vec<Option<f64>>, Error> {
    let variable = data.variable(name)?;
    if variable.is_string() {
        return Err(Error(format!("KNN: the {what} {name} must be numeric")));
    }
    Ok((0..data.case_count())
        .map(|c| numeric_value(&variable, c, true))
        .collect())
}

impl Engine {
    pub(crate) fn knn(&mut self, command: &Knn) -> Result<Vec<Table>, Error> {
        let mut warnings: Vec<String> = Vec::new();
        let data = self.active()?;
        if case_weights(data)?.is_some() {
            warnings.push("KNN does not use the weight variable".into());
        }
        let except = self.expand(&command.except)?;
        let list = |items: &Option<Vec<VarItem>>, word: &str, warnings: &mut Vec<String>| {
            let Some(items) = items else {
                return Ok(Vec::new());
            };
            if items.is_empty() {
                warnings.push(format!("KNN: {word} names no variables and is ignored"));
            }
            let mut out: Vec<String> = Vec::new();
            for n in self.expand(items)? {
                if !out.iter().any(|o| same(o, &n)) && !except.iter().any(|e| same(e, &n)) {
                    out.push(n);
                }
            }
            Ok::<_, Error>(out)
        };
        let factors = list(&command.factors, "BY", &mut warnings)?;
        let covariates = list(&command.covariates, "WITH", &mut warnings)?;
        let predictors: Vec<String> = factors.iter().chain(&covariates).cloned().collect();
        if predictors.is_empty() {
            return Err(Error("KNN has no predictors left after EXCEPT".into()));
        }
        if let Some(both) = factors
            .iter()
            .find(|f| covariates.iter().any(|c| same(c, f)))
        {
            return Err(Error(format!(
                "KNN: {both} is both a factor and a covariate"
            )));
        }
        let target = match &command.target {
            Some(name) => Some(data.variable(name)?.name().to_string()),
            None => None,
        };
        let has_target = target.is_some();
        let split_names: Vec<String> = self
            .split_group
            .iter()
            .flatten()
            .map(|g| g.0.clone())
            .collect();
        let mut used: Vec<(&str, &str)> = predictors
            .iter()
            .map(|p| (p.as_str(), "predictor"))
            .collect();
        if let Some(t) = &target {
            if predictors.iter().any(|p| same(p, t)) {
                return Err(Error(format!(
                    "KNN: the target {t} cannot also be a factor or covariate"
                )));
            }
            used.push((t, "target"));
        }
        if let Some(s) = split_names
            .iter()
            .find(|s| used.iter().any(|u| same(u.0, s)))
        {
            return Err(Error(format!(
                "KNN: the split variable {s} cannot be a target or predictor"
            )));
        }
        for name in &covariates {
            if data.variable(name)?.is_string() {
                return Err(Error(format!("KNN: the covariate {name} must be numeric")));
            }
        }

        // the settings in effect
        let mut neighbors = command.neighbors;
        let mut selection = command.feature_selection.clone();
        let mut weight_features = command.weight_features;
        if !has_target {
            if matches!(neighbors, KnnNeighbors::Auto { .. }) {
                warnings.push("KNN without a target uses a fixed number of neighbors (3); NEIGHBORS=AUTO is ignored".into());
                neighbors = KnnNeighbors::Fixed(3);
            }
            if selection.take().is_some() {
                warnings.push(
                    "KNN without a target uses all features; FEATURES=AUTO is ignored".into(),
                );
            }
            if std::mem::take(&mut weight_features) {
                warnings.push(
                    "KNN without a target cannot weight features; WEIGHTFEATURES=YES is ignored"
                        .into(),
                );
            }
        }
        let cross_validate = matches!(neighbors, KnnNeighbors::Auto { .. }) && selection.is_none();
        if command.folds.is_some() && !cross_validate {
            warnings.push("KNN: CROSSVALIDATION is used only with NEIGHBORS=AUTO and FEATURES=ALL; it is ignored".into());
        }
        let fold_variable = match (&command.folds, cross_validate) {
            (Some(KnnFolds::Variable(v)), true) => Some(data.variable(v)?.name().to_string()),
            _ => None,
        };
        let partition_variable = match &command.partition {
            KnnPartition::Variable(v) => Some(data.variable(v)?.name().to_string()),
            KnnPartition::Ratio { .. } => None,
        };
        for (name, what) in [(&partition_variable, "partition"), (&fold_variable, "fold")] {
            if let Some(name) = name
                && (used.iter().any(|u| same(u.0, name))
                    || split_names.iter().any(|s| same(s, name)))
            {
                return Err(Error(format!(
                    "KNN: the {what} variable {name} cannot be a target, predictor or split variable"
                )));
            }
        }
        let mut forced: Vec<usize> = Vec::new();
        if let Some(force) = &selection {
            for name in self.expand(force)? {
                let Some(i) = predictors.iter().position(|p| same(p, &name)) else {
                    return Err(Error(format!(
                        "KNN: FORCE names {name}, which is not a predictor"
                    )));
                };
                if !forced.contains(&i) {
                    forced.push(i);
                }
            }
            if forced.len() == predictors.len() {
                return Err(Error(
                    "KNN: FORCE takes every predictor; leave at least one for selection".into(),
                ));
            }
        }

        // the target's level
        let target_var = target.as_ref().map(|t| data.variable(t)).transpose()?;
        let scale_target = match (&target_var, command.level) {
            (None, _) => false,
            (Some(v), Some(KnnLevel::Scale)) if v.is_string() => {
                return Err(Error(format!(
                    "KNN: the target {} is a string variable and cannot be scale",
                    v.name()
                )));
            }
            (Some(_), Some(level)) => level == KnnLevel::Scale,
            (Some(v), None) => {
                let index = data.variable_index(v.name()).expect("found");
                let measure = match v.measure() {
                    Measure::Unknown => default_measure(data, index, self.scale_min()),
                    known => known,
                };
                !v.is_string() && measure == Measure::Scale
            }
        };

        // the values of every case
        let cases = data.case_count();
        let factor_keys: Vec<Vec<Option<Key>>> = factors
            .iter()
            .map(|f| Ok(categories(&data.variable(f)?, command.include)))
            .collect::<Result<_, Error>>()?;
        let covariate_values: Vec<Vec<Option<f64>>> = covariates
            .iter()
            .map(|c| {
                let v = data.variable(c)?;
                Ok((0..cases).map(|i| numeric_value(&v, i, false)).collect())
            })
            .collect::<Result<_, Error>>()?;
        let target_keys: Option<Vec<Option<Key>>> =
            target_var.as_ref().map(|v| match scale_target {
                true => (0..cases)
                    .map(|i| numeric_value(v, i, false).map(Key::Number))
                    .collect(),
                false => categories(v, command.include),
            });
        let partition_values = partition_variable
            .as_ref()
            .map(|n| raw_numbers(data, n, "partition variable"))
            .transpose()?;
        let fold_values = fold_variable
            .as_ref()
            .map(|n| raw_numbers(data, n, "fold variable"))
            .transpose()?;
        let focal_values = command
            .focal
            .as_ref()
            .map(|n| raw_numbers(data, n, "focal case variable"))
            .transpose()?;
        if let Some(f) = &focal_values
            && !f.iter().any(|v| v.is_some_and(|v| v > 0.0))
        {
            return Err(Error(
                "KNN: the FOCALCASES variable has no positive values; it marks no focal case"
                    .into(),
            ));
        }
        let case_labels: Option<Vec<String>> = match &command.case_labels {
            Some(name) => {
                let v = data.variable(name)?;
                let shown = self.shown();
                Some(
                    (0..cases)
                        .map(|i| match categories(&v, true)[i].clone() {
                            Some(key) => shown.value(&v, &key),
                            None => String::new(),
                        })
                        .collect(),
                )
            }
            None => None,
        };
        let numbers: Vec<usize> = (0..cases)
            .map(|c| self.case_numbers.as_ref().map_or(c, |n| n[c]) + 1)
            .collect();

        // the analyzed cases
        let rows: Vec<usize> = (0..cases)
            .filter(|&c| {
                target_keys.as_ref().is_none_or(|t| t[c].is_some())
                    && factor_keys.iter().all(|f| f[c].is_some())
                    && covariate_values.iter().all(|v| v[c].is_some())
                    && partition_values.as_ref().is_none_or(|p| p[c].is_some())
                    && fold_values.as_ref().is_none_or(|p| p[c].is_some())
            })
            .collect();
        if rows.is_empty() {
            return Err(Error("KNN has no cases without missing values".into()));
        }
        let n = rows.len();
        let training: Vec<bool> = match (&partition_values, &command.partition) {
            (Some(p), _) => rows
                .iter()
                .map(|&c| p[c].is_some_and(|v| v > 0.0))
                .collect(),
            (None, KnnPartition::Ratio { training, holdout }) => {
                let share = training / (training + holdout);
                rows.iter()
                    .map(|_| *holdout == 0.0 || self.random() < share)
                    .collect()
            }
            (None, KnnPartition::Variable(_)) => unreachable!("read above"),
        };
        let train: Vec<usize> = (0..n).filter(|&r| training[r]).collect();
        let holdout: Vec<usize> = (0..n).filter(|&r| !training[r]).collect();
        if train.is_empty() {
            return Err(Error("KNN has no training cases".into()));
        }

        // coding from the training cases
        let mut columns: Vec<Range<usize>> = Vec::new();
        let mut factor_levels: Vec<Vec<Key>> = Vec::new();
        let mut width = 0;
        for keys in &factor_keys {
            let levels =
                sorted_levels(train.iter().map(|&r| keys[rows[r]].clone().expect("valid")));
            columns.push(width..width + levels.len());
            width += levels.len();
            factor_levels.push(levels);
        }
        let mut ranges: Vec<(f64, f64)> = Vec::new();
        for (j, values) in covariate_values.iter().enumerate() {
            let v = train.iter().map(|&r| values[rows[r]].expect("valid"));
            let (lo, hi) = v.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
                (a.min(x), b.max(x))
            });
            if command.rescale && lo == hi {
                warnings.push(format!(
                    "KNN: the covariate {} has one value in the training cases and adds nothing to the distances",
                    covariates[j]
                ));
            }
            ranges.push((lo, hi));
            columns.push(width..width + 1);
            width += 1;
        }
        let mut scorable = vec![true; n];
        let x: Vec<Vec<f64>> = (0..n)
            .map(|r| {
                let case = rows[r];
                let mut out = vec![0.0; width];
                for (f, keys) in factor_keys.iter().enumerate() {
                    match position(&factor_levels[f], keys[case].as_ref().expect("valid")) {
                        Some(i) => out[columns[f].start + i] = 1.0,
                        None => scorable[r] = false,
                    }
                }
                for (j, values) in covariate_values.iter().enumerate() {
                    let v = values[case].expect("valid");
                    let (lo, hi) = ranges[j];
                    out[columns[factors.len() + j].start] = match command.rescale {
                        false => v,
                        true if hi > lo => 2.0 * (v - lo) / (hi - lo) - 1.0,
                        true => 0.0,
                    };
                }
                out
            })
            .collect();
        let unscored = holdout.iter().filter(|&&r| !scorable[r]).count();
        if unscored > 0 {
            warnings.push(format!(
                "KNN: {unscored} holdout case(s) have a factor category the training cases lack and are not scored"
            ));
        }
        let scored_holdout: Vec<usize> = holdout.iter().copied().filter(|&r| scorable[r]).collect();

        // the target over the rows
        let (response, target_levels, observed_keys) = match &target_keys {
            None => (Response::None, Vec::new(), Vec::new()),
            Some(keys) => {
                let row_keys: Vec<Key> = rows
                    .iter()
                    .map(|&c| keys[c].clone().expect("valid"))
                    .collect();
                if scale_target {
                    let y = row_keys.iter().map(|k| match k {
                        Key::Number(v) => *v,
                        Key::Text(_) => f64::NAN,
                    });
                    (Response::Scale(y.collect()), Vec::new(), row_keys)
                } else {
                    let levels = sorted_levels(train.iter().map(|&r| row_keys[r].clone()));
                    let observed: Vec<Option<usize>> =
                        row_keys.iter().map(|k| position(&levels, k)).collect();
                    let mut counts = vec![0.0; levels.len()];
                    for &r in &train {
                        counts[observed[r].expect("a training category")] += 1.0;
                    }
                    (Response::Class { observed, counts }, levels, row_keys)
                }
            }
        };
        let space = Space {
            x: &x,
            columns: &columns,
            city: command.city_block,
            median: command.median,
            response: &response,
        };
        let p = predictors.len();
        let all: Vec<usize> = (0..p).collect();
        let ones = vec![1.0; p];

        // folds for cross-validation
        let mut fold = vec![None; n];
        let mut fold_count = 0;
        if cross_validate {
            match &fold_values {
                Some(values) => {
                    for &r in &train {
                        let v = values[rows[r]].expect("valid");
                        if v.fract() != 0.0 || v < 1.0 {
                            return Err(Error(format!(
                                "KNN: the fold variable must hold whole numbers from 1, got {v}"
                            )));
                        }
                        fold[r] = Some(v as usize - 1);
                        fold_count = fold_count.max(v as usize);
                    }
                    if let Some(gap) =
                        (0..fold_count).find(|f| !train.iter().any(|&r| fold[r] == Some(*f)))
                    {
                        return Err(Error(format!(
                            "KNN: no training case is in fold {} of the fold variable",
                            gap + 1
                        )));
                    }
                }
                None => {
                    let asked = match &command.folds {
                        Some(KnnFolds::Count(v)) => *v,
                        _ => 10,
                    };
                    fold_count = asked.min(train.len());
                    // draw a number per training case; in that order the cases take the folds in turn
                    let draws: Vec<f64> = train.iter().map(|_| self.random()).collect();
                    let mut order: Vec<usize> = (0..train.len()).collect();
                    order.sort_by(|&a, &b| draws[a].total_cmp(&draws[b]).then(a.cmp(&b)));
                    for (rank, &i) in order.iter().enumerate() {
                        fold[train[i]] = Some(rank % fold_count);
                    }
                }
            }
            if fold_count < 2 {
                return Err(Error(
                    "KNN: cross-validation needs at least two folds".into(),
                ));
            }
        }

        // the number of neighbors and the features
        let (k_low, k_high) = match neighbors {
            KnnNeighbors::Fixed(k) => (k, k),
            KnnNeighbors::Auto { min, max } => (min, max),
        };
        let pool_size = if cross_validate {
            let largest = (0..fold_count)
                .map(|f| train.iter().filter(|&&r| fold[r] == Some(f)).count())
                .max()
                .unwrap_or(0);
            train.len() - largest
        } else {
            train.len() - 1
        };
        if k_high > pool_size {
            return Err(Error(format!(
                "KNN: {k_high} neighbors need more training cases; at most {pool_size} can be neighbors of a training case{}",
                if cross_validate {
                    " outside its fold"
                } else {
                    ""
                }
            )));
        }
        let mut k = k_low;
        let mut features = all.clone();
        let mut cv_errors: Vec<(usize, f64)> = Vec::new();
        let mut selection_log: Vec<(usize, Neighbors)> = Vec::new();
        if has_target && selection.is_some() {
            let candidates: Vec<usize> = all
                .iter()
                .copied()
                .filter(|i| !forced.contains(i))
                .collect();
            let rule = match command.feature_count {
                KnnFeatureCount::Auto => Rule::Fixed(20usize.min(p).saturating_sub(forced.len())),
                KnnFeatureCount::Fixed(m) if m > candidates.len() => {
                    return Err(Error(format!(
                        "KNN: NUMFEATURES=FIXED({m}) is more than the {} predictors open to selection",
                        candidates.len()
                    )));
                }
                KnnFeatureCount::Fixed(m) => Rule::Fixed(m),
                KnnFeatureCount::ErrorRatio(change) => Rule::Ratio(change),
            };
            let mut best_error = f64::INFINITY;
            for kk in k_low..=k_high {
                let (set, steps, error) = space.select(&train, &forced, &candidates, kk, rule);
                if error < best_error {
                    best_error = error;
                    k = kk;
                    features = set;
                }
                selection_log.push((kk, steps));
            }
        } else if cross_validate {
            let mut best = f64::INFINITY;
            for kk in k_low..=k_high {
                let mut total = 0.0;
                for f in 0..fold_count {
                    let queries: Vec<usize> = train
                        .iter()
                        .copied()
                        .filter(|&r| fold[r] == Some(f))
                        .collect();
                    let pool: Vec<usize> = train
                        .iter()
                        .copied()
                        .filter(|&r| fold[r] != Some(f))
                        .collect();
                    total += space.error(&queries, &pool, &all, &ones, kk);
                }
                let mean = total / fold_count as f64;
                if mean < best {
                    best = mean;
                    k = kk;
                }
                cv_errors.push((kk, mean));
            }
        }
        let importance = match has_target {
            true => space.importance(&train, &features, k),
            false => Vec::new(),
        };
        let mut weights = ones.clone();
        if weight_features {
            for (i, &f) in features.iter().enumerate() {
                weights[f] = importance[i];
            }
        }

        // predictions: training cases from the other training cases, scored holdout cases
        let mut predicted: Vec<Option<Prediction>> = (0..n).map(|_| None).collect();
        if has_target {
            for &r in train.iter().chain(&scored_holdout) {
                let near = space.neighbors(r, &train, &features, &weights, k);
                predicted[r] = Some(space.predict(&near));
            }
        }
        let focal: Vec<(usize, Vec<(usize, f64)>)> = match &focal_values {
            Some(values) => (0..n)
                .filter(|&r| scorable[r] && values[rows[r]].is_some_and(|v| v > 0.0))
                .map(|r| (r, space.neighbors(r, &train, &features, &weights, k)))
                .collect(),
            None => Vec::new(),
        };
        for w in warnings.drain(..) {
            self.warn(w);
        }

        // tables
        let data = self.active()?;
        let shown = self.shown();
        let target_var = target.as_ref().map(|t| data.variable(t)).transpose()?;
        let level_text = |key: &Key| match &target_var {
            Some(v) => shown.value(v, key),
            None => String::new(),
        };
        let predictor_text = |i: usize| shown.named(data, &predictors[i]);
        let classify = matches!(response, Response::Class { .. });
        let error_name = if classify {
            "Percent Incorrect"
        } else {
            "Sum of Squares Error"
        };
        let shown_error = |e: f64| Cell::Number(if classify { 100.0 * e } else { e });
        let mut tables = Vec::new();
        if command.print_cps {
            let held = holdout.len();
            let pct = |c: usize| Cell::Number(100.0 * c as f64 / n as f64);
            let body = vec![
                vec![
                    text("Sample"),
                    text("Training"),
                    Cell::Number(train.len() as f64),
                    pct(train.len()),
                ],
                vec![
                    text("Sample"),
                    text("Holdout"),
                    Cell::Number(held as f64),
                    pct(held),
                ],
                vec![text("Valid"), text(""), Cell::Number(n as f64), pct(n)],
                vec![
                    text("Excluded"),
                    text(""),
                    Cell::Number((cases - n) as f64),
                    Cell::Empty,
                ],
                vec![
                    text("Total"),
                    text(""),
                    Cell::Number(cases as f64),
                    Cell::Empty,
                ],
            ];
            tables.push(plain_table(
                "Case Processing Summary",
                2,
                &["", "", "N", "Percent"],
                body,
            ));
        }
        if command.view {
            let how_k = match (neighbors, cross_validate) {
                (KnnNeighbors::Fixed(_), _) => "Fixed".to_string(),
                (_, true) => format!("Cross-validation, {fold_count} folds"),
                (_, false) => "Lowest error of feature selection".to_string(),
            };
            let feature_names: Vec<String> = features.iter().map(|&f| predictor_text(f)).collect();
            let mut summary = vec![
                (
                    "Target",
                    target_var
                        .as_ref()
                        .map_or("None".into(), |v| shown.variable(v)),
                ),
                ("Number of Nearest Neighbors (k)", k.to_string()),
                ("k Chosen by", how_k),
                (
                    "Distance",
                    if command.city_block {
                        "City block"
                    } else {
                        "Euclidean"
                    }
                    .into(),
                ),
                ("Features", feature_names.join(", ")),
                (
                    "Covariate Rescaling",
                    if command.rescale {
                        "Adjusted normalization"
                    } else {
                        "None"
                    }
                    .into(),
                ),
                (
                    "Feature Weighting",
                    if weight_features {
                        "Normalized importance"
                    } else {
                        "None"
                    }
                    .into(),
                ),
            ];
            if has_target {
                summary.insert(
                    1,
                    (
                        "Target Level",
                        if scale_target { "Scale" } else { "Categorical" }.into(),
                    ),
                );
            }
            if has_target && scale_target {
                summary.push((
                    "Prediction",
                    if command.median { "Median" } else { "Mean" }.into(),
                ));
            }
            let body = summary
                .into_iter()
                .map(|(a, b)| vec![text(a), text(b)])
                .collect();
            tables.push(plain_table("Model Summary", 1, &["", "Value"], body));
            if !cv_errors.is_empty() {
                let body = cv_errors
                    .iter()
                    .map(|(kk, e)| vec![Cell::Number(*kk as f64), shown_error(*e)])
                    .collect();
                let mut table = plain_table("k Selection Error Log", 0, &["k", error_name], body);
                table.caption = Some(format!(
                    "Mean error over {fold_count} cross-validation folds; k = {k} is chosen."
                ));
                tables.push(table);
            }
            if !selection_log.is_empty() {
                let auto_k = k_low < k_high;
                let mut body = Vec::new();
                for (kk, steps) in &selection_log {
                    for (step, (f, e)) in steps.iter().enumerate() {
                        let mut row = vec![
                            Cell::Number((step + 1) as f64),
                            text(predictor_text(*f)),
                            Cell::Number((forced.len() + step + 1) as f64),
                            shown_error(*e),
                        ];
                        if auto_k {
                            row.insert(0, Cell::Number(*kk as f64));
                        }
                        body.push(row);
                    }
                }
                let mut columns = vec!["Step", "Feature Added", "Number of Features", error_name];
                if auto_k {
                    columns.insert(0, "k");
                }
                let mut table = plain_table("Feature Selection Error Log", 0, &columns, body);
                if !forced.is_empty() {
                    let names: Vec<String> = forced.iter().map(|&f| predictor_text(f)).collect();
                    table.caption = Some(format!("Forced features: {}.", names.join(", ")));
                }
                tables.push(table);
            }
            if has_target {
                let body = features
                    .iter()
                    .zip(&importance)
                    .map(|(&f, &w)| vec![text(predictor_text(f)), Cell::Number(w)])
                    .collect();
                tables.push(plain_table(
                    "Feature Importance",
                    1,
                    &["Predictor", "Normalized Importance"],
                    body,
                ));
                let mut body = Vec::new();
                for (name, part) in [("Training", &train), ("Holdout", &scored_holdout)] {
                    if part.is_empty() {
                        continue;
                    }
                    let loss: f64 = part
                        .iter()
                        .map(|&r| space.loss(r, predicted[r].as_ref().expect("scored")))
                        .sum();
                    let e = if classify {
                        loss / part.len() as f64
                    } else {
                        loss
                    };
                    body.push(vec![
                        text(name),
                        Cell::Number(part.len() as f64),
                        shown_error(e),
                    ]);
                }
                tables.push(plain_table(
                    "Error Summary",
                    1,
                    &["Partition", "N", error_name],
                    body,
                ));
            }
            if classify {
                let observed = sorted_levels(
                    target_levels.iter().cloned().chain(
                        train
                            .iter()
                            .chain(&scored_holdout)
                            .map(|&r| observed_keys[r].clone()),
                    ),
                );
                let j = target_levels.len();
                let mut body = Vec::new();
                for (name, part) in [("Training", &train), ("Holdout", &scored_holdout)] {
                    if part.is_empty() {
                        continue;
                    }
                    let mut totals = vec![0.0; j];
                    let mut right = 0.0;
                    for level in &observed {
                        let mut counts = vec![0.0; j];
                        let mut total = 0.0;
                        for &r in part
                            .iter()
                            .filter(|&&r| observed_keys[r].compare(level).is_eq())
                        {
                            let c = predicted[r].as_ref().expect("scored").class;
                            counts[c] += 1.0;
                            total += 1.0;
                        }
                        if total == 0.0 && position(&target_levels, level).is_none() {
                            continue;
                        }
                        let hit = position(&target_levels, level).map_or(0.0, |i| counts[i]);
                        right += hit;
                        let mut row = vec![text(name), text(level_text(level))];
                        row.extend(counts.iter().map(|&c| Cell::Number(c)));
                        row.push(if total > 0.0 {
                            Cell::Number(100.0 * hit / total)
                        } else {
                            Cell::Empty
                        });
                        body.push(row);
                        for (t, c) in totals.iter_mut().zip(&counts) {
                            *t += c;
                        }
                    }
                    let size = part.len() as f64;
                    let mut row = vec![text(name), text("Overall Percent")];
                    row.extend(totals.iter().map(|&t| Cell::Number(100.0 * t / size)));
                    row.push(Cell::Number(100.0 * right / size));
                    body.push(row);
                }
                let mut columns = vec!["Partition".to_string(), "Observed".to_string()];
                columns.extend(target_levels.iter().map(&level_text));
                columns.push("Percent Correct".into());
                let mut table = plain_table("Classification Table", 2, &columns, body);
                for group in &mut table.column_groups[2..2 + j] {
                    group.push("Predicted".into());
                }
                tables.push(table);
            }
            if !focal.is_empty() {
                let name = |r: usize| match &case_labels {
                    Some(labels) => text(labels[rows[r]].clone()),
                    None => Cell::Number(numbers[rows[r]] as f64),
                };
                let body = focal
                    .iter()
                    .map(|(r, near)| {
                        let mut row = vec![name(*r)];
                        row.extend((0..k).map(|i| near.get(i).map_or(Cell::Empty, |n| name(n.0))));
                        row.extend(
                            (0..k).map(|i| near.get(i).map_or(Cell::Empty, |n| Cell::Number(n.1))),
                        );
                        row
                    })
                    .collect();
                let mut columns = vec!["Focal Case".to_string()];
                columns.extend((1..=k).map(|i| i.to_string()));
                columns.extend((1..=k).map(|i| i.to_string()));
                let mut table = plain_table("Nearest Neighbors and Distances", 1, &columns, body);
                for (i, group) in table.column_groups.iter_mut().enumerate().skip(1) {
                    group.push(
                        if i <= k {
                            "Nearest Neighbors"
                        } else {
                            "Nearest Distances"
                        }
                        .into(),
                    );
                }
                tables.push(table);
            }
        }

        // saved variables
        let mut saved: Vec<Saved> = Vec::new();
        let mut save_warnings: Vec<String> = Vec::new();
        for (what, given) in &command.save {
            let (root, label) = match what {
                KnnSave::PredVal => ("KNN_PredictedValue", "Predicted value"),
                KnnSave::PredProb => ("KNN_Probability", "Predicted probability"),
                KnnSave::Partition => ("KNN_Partition", "Partition"),
                KnnSave::Fold => ("KNN_Fold", "Cross-validation fold"),
            };
            let skip = match what {
                KnnSave::PredVal if !has_target => Some("PREDVAL needs a target"),
                KnnSave::PredProb if !classify => Some("PREDPROB needs a categorical target"),
                KnnSave::Partition if partition_variable.is_some() => {
                    Some("PARTITION repeats the partition variable")
                }
                KnnSave::Fold if !cross_validate => {
                    Some("FOLD needs cross-validation (NEIGHBORS=AUTO with FEATURES=ALL)")
                }
                KnnSave::Fold if fold_variable.is_some() => Some("FOLD repeats the fold variable"),
                _ => None,
            };
            if let Some(why) = skip {
                save_warnings.push(format!("KNN /SAVE: {why}; it is not saved"));
                continue;
            }
            let wanted: Vec<String> = match what {
                KnnSave::PredProb => (1..=command.max_categories.min(target_levels.len()))
                    .map(|i| format!("{}_{i}", given.as_deref().unwrap_or(root)))
                    .collect(),
                _ => vec![given.clone().unwrap_or(root.to_string())],
            };
            for (i, want) in wanted.into_iter().enumerate() {
                let taken = |m: &str| {
                    data.variable_index(m).is_some() || saved.iter().any(|o| same(&o.name, m))
                };
                let name = match given {
                    Some(_) if taken(&want) => {
                        return Err(Error(format!(
                            "KNN /SAVE names {want}, a variable that already exists; choose another name"
                        )));
                    }
                    Some(_) => want,
                    None => free_name(&want, taken),
                };
                let mut column = Saved {
                    name,
                    values: vec![None; cases],
                    strings: None,
                    format: "F8.0".into(),
                    measure: Measure::Nominal,
                    label: label.into(),
                    labels: Vec::new(),
                };
                for r in 0..n {
                    column.values[rows[r]] = match what {
                        KnnSave::PredVal | KnnSave::PredProb => {
                            predicted[r].as_ref().and_then(|p| match what {
                                KnnSave::PredProb => Some(p.probabilities[i]),
                                _ if classify => match &target_levels[p.class] {
                                    Key::Number(v) => Some(*v),
                                    Key::Text(_) => None,
                                },
                                _ => Some(p.value),
                            })
                        }
                        KnnSave::Partition => Some(f64::from(u8::from(training[r]))),
                        KnnSave::Fold => fold[r].map(|f| (f + 1) as f64),
                    };
                }
                let v = target_var.as_ref();
                match what {
                    KnnSave::PredVal => {
                        let v = v.expect("a target");
                        column.format = v.format().to_string();
                        if !classify {
                            column.measure = Measure::Scale;
                        } else if v.is_string() {
                            let mut strings = vec![String::new(); cases];
                            for r in 0..n {
                                if let Some(p) = &predicted[r]
                                    && let Key::Text(t) = &target_levels[p.class]
                                {
                                    strings[rows[r]] = t.clone();
                                }
                            }
                            column.strings = Some(strings);
                        }
                        if classify {
                            column.labels = v.value_labels().to_vec();
                        }
                    }
                    KnnSave::PredProb => {
                        column.label =
                            format!("Predicted probability of {}", level_text(&target_levels[i]));
                        column.format = "F8.3".into();
                        column.measure = Measure::Scale;
                    }
                    KnnSave::Partition => {
                        column.labels = vec![
                            (Value::Number(1.0), "Training".into()),
                            (Value::Number(0.0), "Holdout".into()),
                        ];
                    }
                    KnnSave::Fold => {}
                }
                saved.push(column);
            }
        }
        let width = target_var
            .as_ref()
            .and_then(|v| v.string_width())
            .unwrap_or(8);

        // files
        let in_split = self.split_group.is_some();
        if let Some(file) = &command.model_file {
            if in_split {
                self.warn("KNN does not write OUTFILE MODEL under SPLIT FILE");
            } else {
                let data = self.active()?;
                let described: Vec<_> = (0..p)
                    .map(|i| match i < factors.len() {
                        true => json!({
                            "name": predictors[i],
                            "kind": "factor",
                            "categories": factor_levels[i].iter().map(key_json).collect::<Vec<_>>(),
                        }),
                        false => json!({
                            "name": predictors[i],
                            "kind": "covariate",
                            "min": ranges[i - factors.len()].0,
                            "max": ranges[i - factors.len()].1,
                        }),
                    })
                    .collect();
                let cases: Vec<_> = train
                    .iter()
                    .map(|&r| {
                        let mut case = json!({"case": numbers[rows[r]], "coded": x[r]});
                        if has_target {
                            case["target"] = key_json(&observed_keys[r]);
                        }
                        case
                    })
                    .collect();
                let model = json!({
                    "format": "statscraft-knn-model",
                    "target": target.as_ref().map(|t| json!({
                        "name": data.variable(t).map_or(t.clone(), |v| v.name().to_string()),
                        "level": if scale_target { "scale" } else { "categorical" },
                        "categories": target_levels.iter().map(key_json).collect::<Vec<_>>(),
                    })),
                    "metric": if command.city_block { "city block" } else { "euclidean" },
                    "rescale": command.rescale,
                    "k": k,
                    "prediction": if command.median { "median" } else { "mean" },
                    "predictors": described,
                    "features": features.iter().map(|&f| &predictors[f]).collect::<Vec<_>>(),
                    "weights": features.iter().map(|&f| weights[f]).collect::<Vec<_>>(),
                    "training": cases,
                });
                self.write_json("KNN", file, "model", &model)?;
            }
        }
        if let Some(file) = &command.focal_file {
            if focal_values.is_none() {
                self.warn(
                    "KNN: OUTFILE FOCALCASES needs a FOCALCASES variable; no file is written",
                );
            } else if in_split {
                self.warn("KNN does not write OUTFILE FOCALCASES under SPLIT FILE");
            } else {
                let mut out = Dataset::new(focal.len());
                let column = |f: &dyn Fn(&(usize, Neighbors)) -> Option<f64>| {
                    focal.iter().map(f).collect::<Vec<_>>()
                };
                out.set_numeric_values(
                    "KNN_FocalCase_CaseNumber",
                    column(&|(r, _)| Some(numbers[rows[*r]] as f64)),
                )?;
                for i in 0..k {
                    let name = format!("KNN_NearestNeighbor_CaseNumber_{}", i + 1);
                    out.set_numeric_values(
                        &name,
                        column(&|(_, near)| near.get(i).map(|n| numbers[rows[n.0]] as f64)),
                    )?;
                    out.set_format(&name, "F8.0")?;
                }
                for i in 0..k {
                    let name = format!("KNN_NearestNeighbor_Distance_{}", i + 1);
                    out.set_numeric_values(&name, column(&|(_, near)| near.get(i).map(|n| n.1)))?;
                    out.set_format(&name, "F10.4")?;
                }
                out.set_format("KNN_FocalCase_CaseNumber", "F8.0")?;
                self.write_data_file(file, out)?;
            }
        }

        for w in save_warnings {
            self.warn(w);
        }
        if saved.is_empty() {
            return Ok(tables);
        }
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            for column in saved {
                let name = &column.name;
                match column.strings {
                    Some(strings) => data.set_string_values(name, width, strings)?,
                    None => {
                        data.set_numeric_values(name, column.values)?;
                        data.set_format(name, &column.format)?;
                    }
                }
                data.set_measure(name, column.measure)?;
                if !column.labels.is_empty() {
                    data.set_value_labels(name, column.labels)?;
                }
                data.set_variable_label(name, Some(&column.label))?;
            }
            Ok(())
        })?;
        Ok(tables)
    }
}

/// A variable to save: numbers, or text for the predicted category of a string target.
struct Saved {
    name: String,
    values: Vec<Option<f64>>,
    strings: Option<Vec<String>>,
    format: String,
    measure: Measure,
    label: String,
    labels: Vec<(Value, String)>,
}
