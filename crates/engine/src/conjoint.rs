//! CONJOINT: part-worth utilities, importances and simulations from rankings or scores of the
//! profiles of a plan.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (CONJOINT), in our words:
//! - The plan's factors are all its variables but STATUS_ (0 experimental, 1 holdout, 2
//!   simulation; all 0 without it) and CARD_. Simulation profiles come last. The responses are
//!   the experimental and holdout profiles in plan order. Factors are numeric, without missing
//!   values, with 2 to 99 levels.
//! - SCORE: higher is better. RANK: rank r of each profile, turned into n + 1 - r. SEQUENCE:
//!   the profile numbers from the best down, turned into ranks first.
//! - Ordinary least squares on the experimental profiles: a DISCRETE factor of m levels has
//!   m - 1 columns coded 1 for its level, -1 for the last level and 0 otherwise; LINEAR the value;
//!   IDEAL and ANTIIDEAL the value and its square. Utilities: the coefficients of a DISCRETE
//!   factor, its last level minus their sum; b x for LINEAR; c1 x + c2 x^2 for the quadratic
//!   models; with standard errors from the coefficients' covariance. The constant comes with it.
//! - Importance: the factor's range of utilities over the sum of the ranges, in percent; with
//!   SUBJECT averaged over the subjects. Pearson's R and Kendall's tau of the predicted and the
//!   observed preferences of the experimental profiles, and Kendall's tau for the holdouts, with
//!   one-tailed significance.
//! - MORE and LESS (and IDEAL, ANTIIDEAL) do not change the estimates; a factor whose estimates
//!   go the other way is counted as a reversal.
//! - Simulations: per subject, maximum utility (1 for the best profile), Bradley-Terry-Luce
//!   (score over the sum of scores) and logit (exp(score) over the sum), averaged over the
//!   subjects; BTL and logit only over subjects whose simulation scores are all positive.
//! - SUBJECT: consecutive cases with the same value are one subject, cases without a value one
//!   more subject at the end; a subject with a missing response is skipped. Split groups of the
//!   data file are analyzed one by one. FACTORS values replace the existing values from the
//!   smallest up, and labels their labels. A plan that is not orthogonal gets Cramer's V.
//! - UTILITY writes per subject the split variables, the subject, CONSTANT, the utilities
//!   (name1, name2, ... for DISCRETE; name_L for LINEAR; name_L and name_Q for the quadratic
//!   models), SCOREn for the rated profiles and SIMULn for the simulations (n the position in
//!   the plan).
//!
//! Choices where the manual is silent (see docs/differences.md):
//! - n in n + 1 - r counts the rated (experimental and holdout) profiles.
//! - The standard errors divide by the residual degrees of freedom of the fit; the last level of
//!   a DISCRETE factor uses the variance of minus the sum of the others.
//! - With SUBJECT the summary is the fit to the mean responses of the subjects (equal weight
//!   per subject), which gives the mean utilities; without it all cases are one subject.
//! - SCOREn and SIMULn are predicted preferences on the scale of the fit (reversed for ranks).
//! - Ties for the best simulation profile share its maximum-utility probability.
//! - The weight variable is not used.

use crate::chart::{Axis, Category, ChartKind, Series, Statistic};
use crate::correlations::pair;
use crate::descriptives::number;
use crate::dist::t_p;
use crate::nonpar_corr::coefficient;
use crate::npar_tests::text;
use crate::orthoplan::plan_factors;
use crate::wls::least_squares;
use crate::{Cell, Chart, Engine, Error, Table, expand_in};
use statscraft_data::{Dataset, Value, Values};
use statscraft_syntax::{Command, Conjoint, ConjointModel, ConjointPrint, Direction, ResponseKind};

struct Factor {
    name: String,
    title: String,
    model: ConjointModel,
    /// The levels of the experimental profiles, ascending, after recoding.
    levels: Vec<f64>,
    level_titles: Vec<String>,
    /// The value of every profile of the plan, after recoding.
    values: Vec<f64>,
    /// (original, new, label) when FACTORS recoded the factor.
    recoded: Vec<(f64, f64, String)>,
}

impl Factor {
    fn rank(&self) -> u8 {
        match self.model {
            ConjointModel::Discrete(_) => 0,
            ConjointModel::Linear(_) => 1,
            _ => 2,
        }
    }

    fn columns(&self) -> usize {
        match self.rank() {
            0 => self.levels.len() - 1,
            r => r as usize,
        }
    }

    fn level(&self, v: f64) -> Option<usize> {
        self.levels.iter().position(|&l| l == v)
    }

    fn design(&self, v: f64, row: &mut Vec<f64>) {
        match self.rank() {
            0 => {
                let k = self.level(v).expect("levels checked");
                let m = self.levels.len() - 1;
                row.extend((0..m).map(|j| match k {
                    _ if k == m => -1.0,
                    _ if k == j => 1.0,
                    _ => 0.0,
                }));
            }
            1 => row.push(v),
            _ => row.extend([v, v * v]),
        }
    }

    fn model_text(&self) -> String {
        let dir = |d: Option<Direction>| match d {
            Some(Direction::More) => " (more)",
            Some(Direction::Less) => " (less)",
            None => "",
        };
        match self.model {
            ConjointModel::Discrete(d) => format!("Discrete{}", dir(d)),
            ConjointModel::Linear(d) => format!("Linear{}", dir(d)),
            ConjointModel::Ideal => "Ideal".into(),
            ConjointModel::Antiideal => "Anti-ideal".into(),
        }
    }
}

/// A subject's label, first case and fit.
type Fitted = (String, usize, Estimate);

struct Plan {
    factors: Vec<Factor>,
    card: Vec<f64>,
    status: Vec<f64>,
    /// Rated profiles are the first `rated` of the plan.
    rated: usize,
}

impl Plan {
    fn of(&self, status: f64) -> Vec<usize> {
        (0..self.status.len())
            .filter(|&i| self.status[i] == status)
            .collect()
    }
}

/// The fit of one subject or of the summary.
struct Estimate {
    constant: (f64, f64),
    /// Per factor per level: the utility and its standard error.
    utilities: Vec<Vec<(f64, f64)>>,
    coefficients: Vec<Vec<f64>>,
    importance: Vec<f64>,
    /// Per profile of the plan.
    predicted: Vec<f64>,
    /// Per rated profile, the mean over the cases.
    observed: Vec<f64>,
    reversals: Vec<bool>,
}

fn fit(plan: &Plan, responses: &[Vec<f64>]) -> Option<Estimate> {
    let exp = plan.of(0.0);
    let (mut x, mut y) = (Vec::new(), Vec::new());
    for case in responses {
        for &i in &exp {
            let mut row = Vec::new();
            for f in &plan.factors {
                f.design(f.values[i], &mut row);
            }
            x.push(row);
            y.push(case[i]);
        }
    }
    let ones = vec![1.0; y.len()];
    let ls = least_squares(&x, &y, &ones, &ones, true)?;
    let mse = ls.mse();
    let cov = |a: usize, b: usize| mse * ls.slope_cov(a, b);
    let mut at = 0;
    let (mut utilities, mut coefficients) = (Vec::new(), Vec::new());
    for f in &plan.factors {
        let cols: Vec<usize> = (at..at + f.columns()).collect();
        at += f.columns();
        let b: Vec<f64> = cols.iter().map(|&a| ls.slopes[a]).collect();
        let u: Vec<(f64, f64)> = match f.rank() {
            0 => (0..f.levels.len())
                .map(|k| match cols.get(k) {
                    Some(&a) => (b[k], cov(a, a).sqrt()),
                    None => {
                        let pairs = cols.iter().flat_map(|&a| cols.iter().map(move |&c| (a, c)));
                        let var: f64 = pairs.map(|(a, c)| cov(a, c)).sum();
                        (-b.iter().sum::<f64>(), var.sqrt())
                    }
                })
                .collect(),
            1 => f
                .levels
                .iter()
                .map(|&v| (b[0] * v, v.abs() * cov(cols[0], cols[0]).sqrt()))
                .collect(),
            _ => f
                .levels
                .iter()
                .map(|&v| {
                    let (a, c) = (cols[0], cols[1]);
                    let var =
                        v * v * cov(a, a) + 2.0 * v.powi(3) * cov(a, c) + v.powi(4) * cov(c, c);
                    (b[0] * v + b[1] * v * v, var.sqrt())
                })
                .collect(),
        };
        coefficients.push(if f.rank() == 0 { Vec::new() } else { b });
        utilities.push(u);
    }
    let value_utility = |f: usize, v: f64| -> f64 {
        let factor = &plan.factors[f];
        let b = &coefficients[f];
        match factor.rank() {
            0 => utilities[f][factor.level(v).expect("levels checked")].0,
            1 => b[0] * v,
            _ => b[0] * v + b[1] * v * v,
        }
    };
    let predicted: Vec<f64> = (0..plan.status.len())
        .map(|i| {
            let parts =
                (0..plan.factors.len()).map(|f| value_utility(f, plan.factors[f].values[i]));
            ls.intercept + parts.sum::<f64>()
        })
        .collect();
    let observed: Vec<f64> = (0..plan.rated)
        .map(|i| responses.iter().map(|r| r[i]).sum::<f64>() / responses.len() as f64)
        .collect();
    let ranges: Vec<f64> = utilities
        .iter()
        .map(|u| {
            let max = u.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
            let min = u.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
            max - min
        })
        .collect();
    let total: f64 = ranges.iter().sum();
    let reversals = plan
        .factors
        .iter()
        .zip(&utilities)
        .zip(&coefficients)
        .map(|((f, u), b)| match f.model {
            ConjointModel::Discrete(Some(d)) => u.windows(2).any(|w| match d {
                Direction::More => w[1].0 < w[0].0,
                Direction::Less => w[1].0 > w[0].0,
            }),
            ConjointModel::Linear(Some(Direction::More)) => b[0] < 0.0,
            ConjointModel::Linear(Some(Direction::Less)) => b[0] > 0.0,
            ConjointModel::Ideal => b[1] > 0.0,
            ConjointModel::Antiideal => b[1] < 0.0,
            _ => false,
        })
        .collect();
    Some(Estimate {
        constant: (ls.intercept, ls.intercept_se()),
        importance: ranges.iter().map(|r| 100.0 * r / total).collect(),
        utilities,
        coefficients,
        predicted,
        observed,
        reversals,
    })
}

/// Pearson's R, Kendall's tau and Kendall's tau of the holdouts, each with its one-tailed
/// significance.
fn correlations(plan: &Plan, e: &Estimate) -> Vec<(&'static str, f64, f64)> {
    // predictions equal but for rounding count as ties for Kendall's tau
    let scale = e.predicted.iter().fold(1.0_f64, |m, v| m.max(v.abs())) * 1e-10;
    let pairs = |rows: &[usize]| -> Vec<(f64, f64, f64)> {
        rows.iter()
            .map(|&i| ((e.predicted[i] / scale).round() * scale, e.observed[i], 1.0))
            .collect()
    };
    let exp = pairs(&plan.of(0.0));
    let r = pair(&exp).r;
    let df = exp.len() as f64 - 2.0;
    let t = r * (df / (1.0 - r * r)).sqrt();
    let tau = coefficient(&exp, true);
    let mut out = vec![
        ("Pearson's R", r, t_p(t, df) / 2.0),
        ("Kendall's tau", tau[0], tau[1] / 2.0),
    ];
    let hold = plan.of(1.0);
    if hold.len() >= 2 {
        let tau = coefficient(&pairs(&hold), true);
        out.push(("Kendall's tau for Holdouts", tau[0], tau[1] / 2.0));
    }
    out
}

/// Maximum-utility probabilities, and BTL and logit ones when they apply.
type Choices = (Vec<f64>, Option<(Vec<f64>, Vec<f64>)>);

/// Per simulation profile the maximum-utility, BTL and logit probabilities of one subject;
/// BTL and logit are None unless every score is positive.
fn probabilities(scores: &[f64]) -> Choices {
    let max = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let ties = scores.iter().filter(|&&s| s == max).count() as f64;
    let best = scores
        .iter()
        .map(|&s| if s == max { 1.0 / ties } else { 0.0 })
        .collect();
    if scores.iter().any(|&s| s <= 0.0) {
        return (best, None);
    }
    let sum: f64 = scores.iter().sum();
    let exp: Vec<f64> = scores.iter().map(|s| (s - max).exp()).collect();
    let exp_sum: f64 = exp.iter().sum();
    let btl = scores.iter().map(|s| s / sum).collect();
    (best, Some((btl, exp.iter().map(|e| e / exp_sum).collect())))
}

/// The name of a saved variable: the factor's name cut so the suffix fits in 64 bytes.
fn suffixed(base: &str, suffix: &str) -> String {
    let mut end = base.len().min(64 - suffix.len());
    while !base.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{suffix}", &base[..end])
}

fn number_text(v: f64) -> String {
    match v.fract() == 0.0 {
        true => format!("{v:.0}"),
        false => format!("{v}"),
    }
}

impl Engine {
    pub(crate) fn conjoint(&mut self, c: &Conjoint) -> Result<Vec<Table>, Error> {
        self.conjoint_out.clear();
        let tables = match c.data {
            Some(Some(_)) => {
                self.begin_pass()?;
                let before = self.dataset.clone();
                let result = self.conjoint_group(c);
                self.end_pass(before.as_ref());
                result?
            }
            _ => self.procedure(&Command::Conjoint(Box::new(c.clone())))?,
        };
        let parts = std::mem::take(&mut self.conjoint_out);
        if let (Some(file), Some((first, rest))) = (&c.utility, parts.split_first()) {
            self.write_data_file(file, Dataset::from_parts(first, rest)?)?;
        }
        Ok(tables)
    }

    fn conjoint_plan(&self, c: &Conjoint, data: &Dataset) -> Result<Plan, Error> {
        let fail = |m: String| Error(format!("CONJOINT: {m}"));
        if data.split_file().is_some() && !matches!(c.plan, Some(Some(_))) {
            return Err(fail(
                "the plan file cannot be split; end SPLIT FILE first".into(),
            ));
        }
        let n = data.case_count();
        let column = |name: &str| -> Result<Vec<f64>, Error> {
            let v = data.variable(name)?;
            if v.is_string() {
                return Err(fail(format!(
                    "{name} is a string variable; factors are numeric"
                )));
            }
            let values: Vec<f64> = v.numeric()?.iter().map(|x| x.unwrap_or(f64::NAN)).collect();
            if values.iter().any(|x| x.is_nan() || v.is_user_missing(*x)) {
                return Err(fail(format!("the plan has missing values of {name}")));
            }
            Ok(values)
        };
        let find = |want: &str| data.variable_names().find(|v| v.eq_ignore_ascii_case(want));
        let status = match find("STATUS_") {
            Some(name) => column(name)?,
            None => vec![0.0; n],
        };
        if let Some(bad) = status.iter().find(|s| ![0.0, 1.0, 2.0].contains(*s)) {
            return Err(fail(format!("STATUS_ is {bad}; it must be 0, 1 or 2")));
        }
        let rated = status.iter().take_while(|&&s| s != 2.0).count();
        if status[rated..].iter().any(|&s| s != 2.0) {
            return Err(fail(
                "simulation profiles (STATUS_ 2) must follow the experimental and holdout profiles"
                    .into(),
            ));
        }
        let card = match find("CARD_") {
            Some(name) => column(name)?,
            None => (1..=n).map(|i| i as f64).collect(),
        };
        let names = plan_factors(data);
        // the factors named on FACTORS first, a later specification of a name replacing an earlier one
        let mut specs: Vec<&statscraft_syntax::ConjointFactor> = Vec::new();
        for spec in &c.factors {
            if !names.iter().any(|n| n.eq_ignore_ascii_case(&spec.name)) {
                return Err(fail(format!("{} is not a factor of the plan", spec.name)));
            }
            match specs
                .iter_mut()
                .find(|s| s.name.eq_ignore_ascii_case(&spec.name))
            {
                Some(s) => *s = spec,
                None => specs.push(spec),
            }
        }
        let mut order: Vec<String> = specs.iter().map(|s| s.name.clone()).collect();
        order.extend(
            names
                .iter()
                .filter(|n| !specs.iter().any(|s| s.name.eq_ignore_ascii_case(n)))
                .cloned(),
        );
        let shown = self.shown();
        let mut factors = Vec::new();
        for name in order {
            let spec = specs.iter().find(|s| s.name.eq_ignore_ascii_case(&name));
            let v = data.variable(&name)?;
            let original = column(&name)?;
            let mut existing = original.clone();
            existing.sort_by(f64::total_cmp);
            existing.dedup();
            let recode = spec.map_or(&[][..], |s| &s.recode[..]);
            if recode.len() > existing.len() {
                return Err(fail(format!(
                    "FACTORS gives {} values for {name}, which has {}",
                    recode.len(),
                    existing.len()
                )));
            }
            let label_of = |orig: f64, new: f64, given: Option<&String>| match given {
                Some(l) => l.clone(),
                None if v
                    .value_labels()
                    .iter()
                    .any(|(x, _)| *x == Value::Number(orig)) =>
                {
                    shown.number(&v, orig)
                }
                None => number_text(new),
            };
            let map: Vec<(f64, f64, String)> = existing
                .iter()
                .enumerate()
                .map(|(k, &orig)| {
                    let (new, label) = recode.get(k).map_or((None, None), |r| (r.0, r.1.as_ref()));
                    let new = new.unwrap_or(orig);
                    (orig, new, label_of(orig, new, label))
                })
                .collect();
            let mut news: Vec<f64> = map.iter().map(|m| m.1).collect();
            news.sort_by(f64::total_cmp);
            news.dedup();
            if news.len() < map.len() {
                return Err(fail(format!("the new values of {name} repeat a value")));
            }
            let lookup = |orig: f64| map.iter().find(|m| m.0 == orig).expect("from the data");
            let values: Vec<f64> = original.iter().map(|&o| lookup(o).1).collect();
            let mut levels: Vec<f64> = (0..n)
                .filter(|&i| status[i] == 0.0)
                .map(|i| values[i])
                .collect();
            levels.sort_by(f64::total_cmp);
            levels.dedup();
            let model = spec.map_or(ConjointModel::Discrete(None), |s| s.model);
            if !(2..=99).contains(&levels.len()) {
                return Err(fail(format!(
                    "{name} has {} levels in the experimental profiles; a factor needs 2 to 99",
                    levels.len()
                )));
            }
            let level_titles = levels
                .iter()
                .map(|&l| map.iter().find(|m| m.1 == l).expect("a level").2.clone())
                .collect();
            let factor = Factor {
                title: spec
                    .and_then(|s| s.label.clone())
                    .unwrap_or_else(|| shown.variable(&v)),
                name: v.name().to_string(),
                model,
                levels,
                level_titles,
                values,
                recoded: if recode.is_empty() { Vec::new() } else { map },
            };
            if factor.rank() == 0
                && let Some(i) = (0..n).find(|&i| factor.level(factor.values[i]).is_none())
            {
                return Err(fail(format!(
                    "profile {} has a level of {name} that no experimental profile has",
                    i + 1
                )));
            }
            factors.push(factor);
        }
        if factors.is_empty() {
            return Err(fail("the plan has no factors".into()));
        }
        factors.sort_by_key(Factor::rank);
        Ok(Plan {
            factors,
            card,
            status,
            rated,
        })
    }

    /// One split group of the data, or all of it.
    pub(crate) fn conjoint_group(&mut self, c: &Conjoint) -> Result<Vec<Table>, Error> {
        let data = match &c.data {
            Some(Some(file)) => self.data_file(file)?,
            _ => self.active()?.clone(),
        };
        let plan_data = match &c.plan {
            Some(Some(file)) => self.data_file(file)?,
            _ => self.active()?.clone(),
        };
        let plan = self.conjoint_plan(c, &plan_data)?;
        for f in &plan.factors {
            if matches!(f.model, ConjointModel::Ideal | ConjointModel::Antiideal)
                && f.levels.len() < 3
            {
                self.warn(format!(
                    "CONJOINT: {} has {} levels; IDEAL and ANTIIDEAL need at least three",
                    f.name,
                    f.levels.len()
                ));
            }
        }
        let (kind, items) = &c.responses;
        let names = expand_in(&data, items)?;
        if names.len() != plan.rated {
            return Err(Error(format!(
                "CONJOINT: {} response variables, but the plan has {} experimental and holdout \
                 profiles",
                names.len(),
                plan.rated
            )));
        }
        let n_cases = data.case_count();
        let mut responses: Vec<Vec<Option<f64>>> = vec![Vec::new(); n_cases];
        for name in &names {
            let v = data.variable(name)?;
            if v.is_string() {
                return Err(Error(format!("CONJOINT: {name} is a string variable")));
            }
            for (case, values) in v.numeric()?.iter().enumerate() {
                responses[case].push(values.filter(|x| !v.is_user_missing(*x)));
            }
        }
        // subjects: runs of cases with one value, the cases without a value last
        let shown = self.shown();
        let mut subjects: Vec<(String, Vec<usize>)> = Vec::new();
        let mut missing_subject = Vec::new();
        if let Some(name) = &c.subject {
            let v = data.variable(name)?;
            let mut previous: Option<String> = None;
            for case in 0..n_cases {
                let key = match v.values() {
                    Values::Numeric(values) => values
                        .get(case)
                        .filter(|x| !v.is_user_missing(*x))
                        .map(|x| {
                            let labelled = v.value_labels().iter().any(|l| l.0 == Value::Number(x));
                            match labelled {
                                true => shown.number(&v, x),
                                false => number_text(x),
                            }
                        }),
                    Values::String(values) => {
                        let s = values.get(case);
                        (!v.is_user_missing_str(s)).then(|| s.trim_end().to_string())
                    }
                };
                match key {
                    None => missing_subject.push(case),
                    Some(key) if previous.as_ref() == Some(&key) => {
                        subjects.last_mut().expect("seen").1.push(case)
                    }
                    Some(key) => {
                        subjects.push((key.clone(), vec![case]));
                        previous = Some(key);
                    }
                }
            }
            if !missing_subject.is_empty() {
                subjects.push(("Missing".into(), missing_subject));
            }
        } else {
            subjects.push((String::new(), (0..n_cases).collect()));
        }
        // the preferences: higher is better
        let rated = plan.rated;
        let mut ties = false;
        let mut prefer = |raw: &[Option<f64>]| -> Option<Vec<f64>> {
            let raw: Vec<f64> = raw.iter().copied().collect::<Option<_>>()?;
            Some(match kind {
                ResponseKind::Score => raw,
                ResponseKind::Rank => {
                    let mut sorted = raw.clone();
                    sorted.sort_by(f64::total_cmp);
                    ties |= sorted.windows(2).any(|w| w[0] == w[1]);
                    raw.iter().map(|r| (rated + 1) as f64 - r).collect()
                }
                ResponseKind::Sequence => {
                    let mut y = vec![f64::NAN; rated];
                    for (i, &k) in raw.iter().enumerate() {
                        let ok = k.fract() == 0.0 && k >= 1.0 && k <= rated as f64;
                        if !ok || !y[k as usize - 1].is_nan() {
                            return None;
                        }
                        y[k as usize - 1] = (rated - i) as f64;
                    }
                    y
                }
            })
        };
        let mut skipped = Vec::new();
        let mut fits: Vec<Fitted> = Vec::new();
        for (label, cases) in &subjects {
            let rows: Option<Vec<Vec<f64>>> =
                cases.iter().map(|&k| prefer(&responses[k])).collect();
            match rows.and_then(|rows| fit(&plan, &rows)) {
                Some(e) => fits.push((label.clone(), cases[0], e)),
                None => skipped.push(label.clone()),
            }
        }
        if ties {
            self.warn("CONJOINT: some subjects give tied ranks");
        }
        if !skipped.is_empty() {
            let who = match c.subject {
                Some(_) => format!("subjects {}", skipped.join(", ")),
                None => "the data".into(),
            };
            self.warn(format!(
                "CONJOINT: {who} skipped: a response is missing or not a valid profile number, \
                 or the responses do not vary"
            ));
        }
        if fits.is_empty() {
            return Err(Error("CONJOINT: no subject has valid responses".into()));
        }
        let by_subject = c.subject.is_some();
        let summary = match by_subject {
            true => {
                let means: Vec<Vec<f64>> = fits.iter().map(|f| f.2.observed.clone()).collect();
                let mut e = fit(&plan, &means)
                    .ok_or_else(|| Error("CONJOINT: the mean responses do not vary".into()))?;
                let s = fits.len() as f64;
                e.importance = (0..plan.factors.len())
                    .map(|f| fits.iter().map(|x| x.2.importance[f]).sum::<f64>() / s)
                    .collect();
                e
            }
            false => fits.pop().expect("one").2,
        };

        let mut tables = Vec::new();
        let analysis = matches!(c.print, ConjointPrint::All | ConjointPrint::Analysis);
        let simulation = matches!(c.print, ConjointPrint::All | ConjointPrint::Simulation);
        let sims = plan.of(2.0);
        if c.print != ConjointPrint::None {
            tables.extend(self.plan_tables(&plan));
        }
        let per_subject = by_subject && !c.summary_only;
        for (label, _, e) in fits.iter().filter(|_| per_subject) {
            let suffix = format!(": Subject {label}");
            if analysis {
                tables.extend(estimate_tables(&plan, e, &suffix));
            }
            if simulation && !sims.is_empty() {
                tables.push(simulation_scores(&plan, e, &suffix));
            }
        }
        if analysis {
            tables.extend(estimate_tables(&plan, &summary, ""));
            tables.extend(reversal_tables(&plan, &fits, &summary, by_subject));
        }
        if simulation && !sims.is_empty() {
            tables.push(simulation_scores(&plan, &summary, ""));
            let subjects: Vec<&Estimate> = match by_subject {
                true => fits.iter().map(|f| &f.2).collect(),
                false => vec![&summary],
            };
            tables.push(simulation_probabilities(&plan, &subjects));
        }
        if c.print == ConjointPrint::None {
            tables.clear();
        }
        self.conjoint_charts(c, &plan, &fits, &summary);
        if c.utility.is_some() {
            let out = self.utility_rows(c, &plan, &data, &fits, &summary)?;
            self.conjoint_out.push(out);
        }
        Ok(tables)
    }

    fn plan_tables(&mut self, plan: &Plan) -> Vec<Table> {
        let mut tables = vec![Table {
            title: "Model Description".into(),
            columns: ["Factor", "Model", "Number of Levels"]
                .map(String::from)
                .to_vec(),
            rows: plan
                .factors
                .iter()
                .map(|f| {
                    vec![
                        text(f.title.clone()),
                        text(f.model_text()),
                        Cell::Number(f.levels.len() as f64),
                    ]
                })
                .collect(),
            row_headers: 1,
            ..Table::default()
        }];
        for f in plan.factors.iter().filter(|f| !f.recoded.is_empty()) {
            tables.push(Table {
                title: format!("Recoded Values: {}", f.title),
                columns: ["Original Value", "Recoded Value", "Label"]
                    .map(String::from)
                    .to_vec(),
                rows: f
                    .recoded
                    .iter()
                    .map(|(o, n, l)| vec![Cell::Number(*o), Cell::Number(*n), text(l.clone())])
                    .collect(),
                row_headers: 1,
                ..Table::default()
            });
        }
        // Cramer's V of every pair of factors over the experimental profiles
        let exp = plan.of(0.0);
        let total = exp.len() as f64;
        let k = plan.factors.len();
        let mut v: Vec<Vec<f64>> = vec![vec![0.0; k]; k];
        let mut orthogonal = true;
        for (a, va) in v.iter_mut().enumerate() {
            for (b, vab) in va.iter_mut().enumerate().take(a) {
                let (fa, fb) = (&plan.factors[a], &plan.factors[b]);
                let count = |f: &Factor, l: usize| {
                    exp.iter()
                        .filter(|&&i| f.level(f.values[i]) == Some(l))
                        .count() as f64
                };
                let mut chi = 0.0;
                for i in 0..fa.levels.len() {
                    for j in 0..fb.levels.len() {
                        let both = exp
                            .iter()
                            .filter(|&&r| {
                                fa.level(fa.values[r]) == Some(i)
                                    && fb.level(fb.values[r]) == Some(j)
                            })
                            .count() as f64;
                        let expected = count(fa, i) * count(fb, j) / total;
                        chi += (both - expected).powi(2) / expected;
                    }
                }
                let q = fa.levels.len().min(fb.levels.len()) as f64 - 1.0;
                *vab = (chi / (total * q)).sqrt();
                orthogonal &= *vab < 1e-9;
            }
        }
        if !orthogonal {
            self.warn("CONJOINT: the factors of the plan are not orthogonal; see Cramer's V");
            let mut columns = vec![String::new()];
            columns.extend(plan.factors.iter().map(|f| f.title.clone()));
            tables.push(Table {
                title: "Cramer's V".into(),
                columns,
                rows: (0..k)
                    .map(|a| {
                        let mut row = vec![text(plan.factors[a].title.clone())];
                        row.extend((0..k).map(|b| match b.cmp(&a) {
                            std::cmp::Ordering::Less => Cell::Number(v[a][b]),
                            std::cmp::Ordering::Equal => Cell::Number(1.0),
                            std::cmp::Ordering::Greater => Cell::Empty,
                        }));
                        row
                    })
                    .collect(),
                row_headers: 1,
                ..Table::default()
            });
        }
        tables
    }

    fn conjoint_charts(&mut self, c: &Conjoint, plan: &Plan, fits: &[Fitted], summary: &Estimate) {
        let bars = |title: String, x: &str, y: &str, cats: &[String], series: Vec<Series>| {
            let categories = cats
                .iter()
                .map(|l| Category {
                    label: l.clone(),
                    value: Cell::Text(l.clone()),
                    missing: false,
                    bin: None,
                })
                .collect();
            let mut chart =
                Chart::lines(title, Axis::titled(x), Axis::titled(y), categories, series);
            chart.kind = ChartKind::Bar;
            chart
        };
        let series = |name: &str, values: Vec<f64>| Series {
            name: name.into(),
            statistic: Statistic::Value,
            variable: None,
            value: Cell::Empty,
            values,
        };
        let factor_titles: Vec<String> = plan.factors.iter().map(|f| f.title.clone()).collect();
        let utilities = |e: &Estimate, f: usize| e.utilities[f].iter().map(|u| u.0).collect();
        let mut charts = Vec::new();
        if c.plot_summary {
            charts.push(bars(
                "Importance Summary".into(),
                "Factor",
                "Averaged Importance",
                &factor_titles,
                vec![series("Importance", summary.importance.clone())],
            ));
            for (f, factor) in plan.factors.iter().enumerate() {
                charts.push(bars(
                    format!("Summary Utilities: {}", factor.title),
                    &factor.title,
                    "Utility",
                    &factor.level_titles,
                    vec![series("Utility", utilities(summary, f))],
                ));
            }
        }
        if c.plot_subject && c.subject.is_none() {
            self.warn("CONJOINT: PLOT=SUBJECT needs SUBJECT; no subject charts are drawn");
        } else if c.plot_subject {
            let per = |f: &dyn Fn(&Estimate) -> Vec<f64>| -> Vec<Series> {
                fits.iter()
                    .map(|x| series(&format!("Subject {}", x.0), f(&x.2)))
                    .collect()
            };
            charts.push(bars(
                "Importance by Subject".into(),
                "Factor",
                "Importance",
                &factor_titles,
                per(&|e| e.importance.clone()),
            ));
            for (f, factor) in plan.factors.iter().enumerate() {
                charts.push(bars(
                    format!("Utilities by Subject: {}", factor.title),
                    &factor.title,
                    "Utility",
                    &factor.level_titles,
                    per(&|e| utilities(e, f)),
                ));
            }
        }
        self.charts.extend(charts);
    }

    fn utility_rows(
        &self,
        c: &Conjoint,
        plan: &Plan,
        data: &Dataset,
        fits: &[Fitted],
        summary: &Estimate,
    ) -> Result<Dataset, Error> {
        let rows: Vec<(Option<usize>, &Estimate)> = match c.subject {
            Some(_) => fits.iter().map(|f| (Some(f.1), &f.2)).collect(),
            None => vec![((data.case_count() > 0).then_some(0), summary)],
        };
        let mut out = Dataset::new(rows.len());
        let cases: Vec<Option<usize>> = rows.iter().map(|r| r.0).collect();
        let split = self
            .split_group
            .iter()
            .flatten()
            .map(|(name, _)| name.clone());
        for name in split.chain(c.subject.clone()) {
            out.copy_variable(data, &name, &cases)?;
        }
        let mut put = |name: String, f: &dyn Fn(&Estimate) -> f64| {
            out.set_numeric_values(&name, rows.iter().map(|r| Some(f(r.1))).collect())
        };
        put("CONSTANT".into(), &|e| e.constant.0)?;
        for (i, factor) in plan.factors.iter().enumerate() {
            match factor.rank() {
                0 => {
                    for k in 0..factor.levels.len() {
                        put(suffixed(&factor.name, &(k + 1).to_string()), &|e| {
                            e.utilities[i][k].0
                        })?;
                    }
                }
                r => {
                    put(suffixed(&factor.name, "_L"), &|e| e.coefficients[i][0])?;
                    if r == 2 {
                        put(suffixed(&factor.name, "_Q"), &|e| e.coefficients[i][1])?;
                    }
                }
            }
        }
        for i in 0..plan.status.len() {
            let name = match plan.status[i] == 2.0 {
                true => format!("SIMUL{}", i + 1),
                false => format!("SCORE{}", i + 1),
            };
            put(name, &|e| e.predicted[i])?;
        }
        Ok(out)
    }
}

fn estimate_tables(plan: &Plan, e: &Estimate, suffix: &str) -> Vec<Table> {
    let mut rows = Vec::new();
    for (f, factor) in plan.factors.iter().enumerate() {
        for (k, title) in factor.level_titles.iter().enumerate() {
            let (u, se) = e.utilities[f][k];
            rows.push(vec![
                text(factor.title.clone()),
                text(title.clone()),
                number(Some(u)),
                number(Some(se)),
            ]);
        }
    }
    rows.push(vec![
        text("(Constant)"),
        Cell::Empty,
        number(Some(e.constant.0)),
        number(Some(e.constant.1)),
    ]);
    let mut tables = vec![
        Table {
            title: format!("Utilities{suffix}"),
            columns: ["Factor", "Level", "Utility Estimate", "Std. Error"]
                .map(String::from)
                .to_vec(),
            rows,
            row_headers: 2,
            ..Table::default()
        },
        Table {
            title: format!("Importance Values{suffix}"),
            columns: vec!["Factor".into(), "Importance".into()],
            rows: plan
                .factors
                .iter()
                .zip(&e.importance)
                .map(|(f, i)| vec![text(f.title.clone()), number(Some(*i))])
                .collect(),
            row_headers: 1,
            ..Table::default()
        },
    ];
    let mut coefficients = Vec::new();
    for (f, factor) in plan.factors.iter().enumerate() {
        let names: &[&str] = match factor.rank() {
            0 => &[],
            1 => &["Linear"],
            _ => &["Linear", "Quadratic"],
        };
        for (name, b) in names.iter().zip(&e.coefficients[f]) {
            coefficients.push(vec![
                text(factor.title.clone()),
                text(*name),
                number(Some(*b)),
            ]);
        }
    }
    if !coefficients.is_empty() {
        tables.push(Table {
            title: format!("Coefficients{suffix}"),
            columns: ["Factor", "Term", "B Coefficient"]
                .map(String::from)
                .to_vec(),
            rows: coefficients,
            row_headers: 2,
            ..Table::default()
        });
    }
    tables.push(Table {
        title: format!("Correlations{suffix}"),
        columns: ["", "Value", "Sig."].map(String::from).to_vec(),
        rows: correlations(plan, e)
            .into_iter()
            .map(|(name, v, sig)| vec![text(name), number(Some(v)), number(Some(sig))])
            .collect(),
        row_headers: 1,
        caption: Some(
            "Correlations between the observed and the estimated preferences; one-tailed \
             significance."
                .into(),
        ),
        ..Table::default()
    });
    tables
}

fn reversal_tables(
    plan: &Plan,
    fits: &[Fitted],
    summary: &Estimate,
    by_subject: bool,
) -> Vec<Table> {
    let directed: Vec<usize> = (0..plan.factors.len())
        .filter(|&f| {
            !matches!(
                plan.factors[f].model,
                ConjointModel::Discrete(None) | ConjointModel::Linear(None)
            )
        })
        .collect();
    if directed.is_empty() {
        return Vec::new();
    }
    let all: Vec<&Estimate> = match by_subject {
        true => fits.iter().map(|f| &f.2).collect(),
        false => vec![summary],
    };
    let mut tables = vec![Table {
        title: "Number of Reversals".into(),
        columns: vec!["Factor".into(), "Reversals".into()],
        rows: directed
            .iter()
            .map(|&f| {
                let n = all.iter().filter(|e| e.reversals[f]).count();
                vec![text(plan.factors[f].title.clone()), Cell::Number(n as f64)]
            })
            .collect(),
        row_headers: 1,
        ..Table::default()
    }];
    if by_subject {
        let counts: Vec<usize> = all
            .iter()
            .map(|e| directed.iter().filter(|&&f| e.reversals[f]).count())
            .collect();
        let rows: Vec<Vec<Cell>> = (1..=directed.len())
            .filter_map(|k| {
                let who: Vec<&str> = (0..fits.len())
                    .filter(|&s| counts[s] == k)
                    .map(|s| fits[s].0.as_str())
                    .collect();
                (!who.is_empty()).then(|| {
                    vec![
                        text(format!("{k} reversals")),
                        text(format!("Subjects {}", who.join(", "))),
                    ]
                })
            })
            .collect();
        if !rows.is_empty() {
            tables.push(Table {
                title: "Reversal Summary".into(),
                columns: vec!["Reversals".into(), "Subjects".into()],
                rows,
                row_headers: 1,
                ..Table::default()
            });
        }
    }
    tables
}

fn simulation_scores(plan: &Plan, e: &Estimate, suffix: &str) -> Table {
    Table {
        title: format!("Preference Scores of Simulations{suffix}"),
        columns: vec!["Card Number".into(), "Score".into()],
        rows: plan
            .of(2.0)
            .into_iter()
            .map(|i| vec![Cell::Number(plan.card[i]), number(Some(e.predicted[i]))])
            .collect(),
        row_headers: 1,
        ..Table::default()
    }
}

fn simulation_probabilities(plan: &Plan, subjects: &[&Estimate]) -> Table {
    let sims = plan.of(2.0);
    let m = sims.len();
    let (mut best, mut btl, mut logit) = (vec![0.0; m], vec![0.0; m], vec![0.0; m]);
    let mut positive = 0;
    for e in subjects {
        let scores: Vec<f64> = sims.iter().map(|&i| e.predicted[i]).collect();
        let (b, rest) = probabilities(&scores);
        (0..m).for_each(|j| best[j] += b[j]);
        if let Some((t, l)) = rest {
            positive += 1;
            (0..m).for_each(|j| {
                btl[j] += t[j];
                logit[j] += l[j];
            });
        }
    }
    let s = subjects.len() as f64;
    let p = positive as f64;
    Table {
        title: "Preference Probabilities of Simulations".into(),
        columns: [
            "Card Number",
            "Maximum Utility",
            "Bradley-Terry-Luce",
            "Logit",
        ]
        .map(String::from)
        .to_vec(),
        rows: (0..m)
            .map(|j| {
                vec![
                    Cell::Number(plan.card[sims[j]]),
                    number(Some(best[j] / s)),
                    number(Some(btl[j] / p)),
                    number(Some(logit[j] / p)),
                ]
            })
            .collect(),
        row_headers: 1,
        caption: Some(format!(
            "Mean probabilities over {} subjects; Bradley-Terry-Luce and Logit over the {positive} \
             whose simulation scores are all positive.",
            subjects.len()
        )),
        ..Table::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_probabilities_share_ties() {
        let (best, rest) = probabilities(&[2.0, 4.0, 4.0]);
        assert_eq!(best, [0.0, 0.5, 0.5]);
        let (btl, logit) = rest.unwrap();
        assert!((btl[0] - 0.2).abs() < 1e-12);
        let e = 1.0 / (2.0 + (-2.0f64).exp());
        assert!((logit[1] - e).abs() < 1e-12);
        assert!(probabilities(&[1.0, -1.0]).1.is_none());
        assert_eq!(suffixed(&"x".repeat(70), "_L").len(), 64);
    }
}
