//! MLP and RBF: neural networks that predict scale dependents and classify categorical ones
//! from factors and covariates.
//!
//! Rules, in our words, from the Command Syntax Reference and the Algorithms manual:
//! - A case is used when every dependent, factor, covariate and the partition variable is
//!   valid: user-missing values count only for categorical variables under
//!   USERMISSING=INCLUDE, and always for the partition variable.
//! - PARTITION shares assign each such case at random (SET SEED) to training, testing or
//!   holdout; VARIABLE assigns by sign: positive training, 0 testing, negative holdout.
//! - Categorical variables become one unit per category of the training cases (one-of-c). A
//!   testing or holdout case with a factor category that training lacks is left out and not
//!   scored; one with an unknown dependent category is left out but scored.
//! - Rescaling of covariates and scale dependents uses the training cases: standardized
//!   (x - mean) / sd, normalized (x - min) / (max - min), adjusted normalized twice that minus
//!   1; the dependents' correction e widens the range to [min - e, max + e]. Errors and
//!   relative errors are reported on the rescaled values.
//! - MLP's network is in `neural_net`. RBF finds J clusters of the training cases by two-step
//!   clustering; unit j is a Gaussian at the cluster's means (and category shares) with
//!   width sqrt(h) times the root mean square of the clusters' standard deviations (and of
//!   sqrt(p (1 - p)) for categories), zero widths taking the smallest other width; NRBF
//!   divides the units by their sum. The output weights are least squares, with a bias only for
//!   ORBF. The automatic count runs from min(K, R) (at least 2 for NRBF) to max(10, 2K, R),
//!   K the two-step count and R the output units, and keeps the lowest testing error, or BIC
//!   = M R ln(MSE) + k ln M without testing cases.
//! - Relative error of a scale dependent: sum (y - yhat)^2 over sum (y - mean)^2; categorical
//!   dependents: the percent of cases whose largest output is not their category.
//! - Importance: for each case and predictor, the largest distance between the outputs over
//!   the predictor's minimum, quartiles and maximum (or each category); averaged over the
//!   training and testing cases and scaled to sum to 1.
//! - ROC, gains and lift use the pseudo-probabilities shifted to be at least 0 and divided by
//!   their sum. The WEIGHT variable is not used.

use crate::chart::{Axis, Category, Chart, ChartKind, Orientation, Series, Statistic};
use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::examine::Distribution;
use crate::neural_net::{Net, Row, Sample, Settings, Stop, anneal, choose_units, shuffle, train};
use crate::npar_tests::text;
use crate::plot::{BoxSummary, Mark, Plot, Point, PointGroup};
use crate::tree::level_of;
use crate::twostep::{
    Cf, Model as Clustering, Settings as Tree, agglomerate, auto_count, clusters_at, free_name,
    key_json, numeric_value, plain_table, precluster,
};
use crate::{Cell, Engine, Error, Table};
use serde_json::json;
use statscraft_data::{Measure, Variable};
use statscraft_syntax::{Activation, Level, Neural, Partition, Rescale, Training, VarItem};

/// z = shift + scale x.
#[derive(Clone, Copy, Debug)]
struct Linear {
    shift: f64,
    scale: f64,
}

impl Linear {
    fn of(method: Rescale, values: &[f64]) -> Linear {
        let n = values.len() as f64;
        let (lo, hi) = values
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| {
                (a.min(x), b.max(x))
            });
        let range = |e: f64| (lo - e, (hi - lo + 2.0 * e).max(f64::MIN_POSITIVE));
        let (center, divisor, adjusted) = match method {
            Rescale::Standardized => {
                let mean = values.iter().sum::<f64>() / n;
                let ss: f64 = values.iter().map(|x| (x - mean) * (x - mean)).sum();
                let sd = (ss / (n - 1.0).max(1.0)).sqrt();
                (mean, if sd > 0.0 { sd } else { 1.0 }, false)
            }
            Rescale::Normalized(e) => (range(e).0, range(e).1, false),
            Rescale::AdjNormalized(e) => (range(e).0, range(e).1, true),
            Rescale::None => (0.0, 1.0, false),
        };
        let k = if adjusted { 2.0 } else { 1.0 };
        Linear {
            shift: -k * center / divisor - f64::from(u8::from(adjusted)),
            scale: k / divisor,
        }
    }

    fn apply(&self, x: f64) -> f64 {
        self.shift + self.scale * x
    }

    fn back(&self, z: f64) -> f64 {
        (z - self.shift) / self.scale
    }
}

fn rescale_name(method: Rescale) -> &'static str {
    match method {
        Rescale::Standardized => "Standardized",
        Rescale::Normalized(_) => "Normalized",
        Rescale::AdjNormalized(_) => "Adjusted Normalized",
        Rescale::None => "None",
    }
}

fn activation_name(f: Activation) -> &'static str {
    match f {
        Activation::Tanh => "Hyperbolic tangent",
        Activation::Sigmoid => "Sigmoid",
        Activation::Identity => "Identity",
        Activation::Softmax => "Softmax",
    }
}

/// A dependent's output units.
enum Target {
    Scale(Linear),
    /// The categories of the training cases and the first output unit.
    Categories(Vec<Key>, usize),
}

/// An RBF network.
struct Rbf {
    centers: Vec<Vec<f64>>,
    widths: Vec<f64>,
    /// Per output unit: the bias (ORBF only) and one weight per hidden unit.
    weights: Vec<Vec<f64>>,
    orbf: bool,
}

impl Rbf {
    fn hidden(&self, x: &[f64]) -> Vec<f64> {
        let mut phi: Vec<f64> = self
            .centers
            .iter()
            .zip(&self.widths)
            .map(|(c, s)| {
                let d: f64 = c.iter().zip(x).map(|(c, x)| (x - c) * (x - c)).sum();
                -d / (2.0 * s * s)
            })
            .collect();
        if self.orbf {
            phi.iter_mut().for_each(|v| *v = v.exp());
        } else {
            let top = phi.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            phi.iter_mut().for_each(|v| *v = (*v - top).exp());
            let sum: f64 = phi.iter().sum();
            phi.iter_mut().for_each(|v| *v /= sum);
        }
        if self.orbf {
            phi.insert(0, 1.0);
        }
        phi
    }

    fn predict(&self, x: &[f64]) -> Vec<f64> {
        let phi = self.hidden(x);
        self.weights.iter().map(|w| dot(w, &phi)).collect()
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

enum Fitted {
    Mlp(Net, Vec<f64>, Stop),
    Rbf(Rbf),
}

impl Fitted {
    fn predict(&self, x: &[f64]) -> Vec<f64> {
        match self {
            Fitted::Mlp(net, w, _) => net.predict(w, x),
            Fitted::Rbf(rbf) => rbf.predict(x),
        }
    }
}

/// Least squares weights of each target column on the columns of `x`; a column that repeats
/// others gets 0.
pub(crate) fn least_squares(x: &[Vec<f64>], y: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let (c, r) = (x[0].len(), y[0].len());
    let mut m = vec![vec![0.0; c + r]; c + r];
    for (xi, yi) in x.iter().zip(y) {
        let row: Vec<f64> = xi.iter().chain(yi).copied().collect();
        for a in 0..c + r {
            for b in 0..c + r {
                m[a][b] += row[a] * row[b];
            }
        }
    }
    let diagonal: Vec<f64> = (0..c).map(|k| m[k][k]).collect();
    let mut swept = vec![false; c];
    for k in 0..c {
        if m[k][k] > 1e-10 * diagonal[k].max(1e-300) {
            crate::matrix::sweep(&mut m, k);
            swept[k] = true;
        }
    }
    (0..r)
        .map(|t| {
            (0..c)
                .map(|k| if swept[k] { -m[k][c + t] } else { 0.0 })
                .collect()
        })
        .collect()
}

/// The RBF units of the clusters, with the overlap factor h.
fn rbf_units(clusters: &[Cf], factors: &[usize], h: f64) -> (Vec<Vec<f64>>, Vec<f64>) {
    let mut centers = Vec::new();
    let mut widths: Vec<f64> = Vec::new();
    for c in clusters {
        let mut center = Vec::new();
        let mut spread = Vec::new();
        for (f, &levels) in factors.iter().enumerate() {
            for l in 0..levels {
                let share = c.counts[f][l] / c.n;
                center.push(share);
                spread.push(h * share * (1.0 - share));
            }
        }
        for k in 0..c.mean.len() {
            center.push(c.mean[k]);
            spread.push(h * c.ss[k] / (c.n - 1.0).max(1.0));
        }
        centers.push(center);
        widths.push((spread.iter().sum::<f64>() / spread.len() as f64).sqrt());
    }
    let smallest = widths
        .iter()
        .copied()
        .filter(|w| *w > 0.0)
        .fold(f64::INFINITY, f64::min);
    for w in widths.iter_mut().filter(|w| **w <= 0.0) {
        *w = if smallest.is_finite() {
            smallest
        } else {
            h.sqrt()
        };
    }
    (centers, widths)
}

fn fit_rbf(
    rows: &[Row],
    set: &[usize],
    centers: Vec<Vec<f64>>,
    widths: Vec<f64>,
    orbf: bool,
) -> Rbf {
    let mut rbf = Rbf {
        centers,
        widths,
        weights: Vec::new(),
        orbf,
    };
    let x: Vec<Vec<f64>> = set.iter().map(|&m| rbf.hidden(&rows[m].x)).collect();
    let y: Vec<Vec<f64>> = set.iter().map(|&m| rows[m].y.clone()).collect();
    rbf.weights = least_squares(&x, &y);
    rbf
}

fn sum_squares(fit: &Fitted, rows: &[Row], set: &[usize]) -> f64 {
    set.iter()
        .map(|&m| {
            let a = fit.predict(&rows[m].x);
            0.5 * a
                .iter()
                .zip(&rows[m].y)
                .map(|(a, y)| (y - a) * (y - a))
                .sum::<f64>()
        })
        .sum()
}

/// Tukey's box of the values.
pub(crate) fn box_of(label: String, group: String, values: &[(f64, usize)]) -> Option<BoxSummary> {
    let pairs: Vec<(f64, f64)> = values.iter().map(|v| (v.0, 1.0)).collect();
    let [q1, median, q3] = Distribution::new(&pairs).hinges()?;
    let step = 1.5 * (q3 - q1);
    let (mut low, mut high) = (q1, q3);
    let (mut outliers, mut extremes) = (Vec::new(), Vec::new());
    for &(y, case) in values {
        let mark = Mark {
            value: y,
            case: (case + 1).to_string(),
        };
        let off = (y - q3).max(q1 - y);
        if off >= 2.0 * step && off > 0.0 {
            extremes.push(mark);
        } else if off >= step && off > 0.0 {
            outliers.push(mark);
        } else {
            (low, high) = (low.min(y), high.max(y));
        }
    }
    Some(BoxSummary {
        label,
        group: Some(group),
        n: values.len() as f64,
        lower_whisker: low,
        lower_hinge: q1,
        median,
        upper_hinge: q3,
        upper_whisker: high,
        outliers,
        extremes,
    })
}

/// The pseudo-probabilities shifted to be at least 0 and divided by their sum.
fn rescaled(p: &[f64]) -> Vec<f64> {
    let low = p.iter().copied().fold(0.0, f64::min);
    let sum: f64 = p.iter().map(|v| v - low).sum();
    p.iter()
        .map(|v| {
            if sum > 0.0 {
                (v - low) / sum
            } else {
                1.0 / p.len() as f64
            }
        })
        .collect()
}

/// The area under the ROC curve: the share of positive-negative pairs ordered right, ties half.
fn roc_area(scores: &[(f64, bool)]) -> Option<f64> {
    let pos: Vec<f64> = scores.iter().filter(|s| s.1).map(|s| s.0).collect();
    let neg: Vec<f64> = scores.iter().filter(|s| !s.1).map(|s| s.0).collect();
    if pos.is_empty() || neg.is_empty() {
        return None;
    }
    let mut won = 0.0;
    for p in &pos {
        for n in &neg {
            won += if p > n {
                1.0
            } else if p == n {
                0.5
            } else {
                0.0
            };
        }
    }
    Some(won / (pos.len() * neg.len()) as f64)
}

fn duration(seconds: f64) -> String {
    let whole = seconds.floor() as u64;
    format!(
        "{}:{:02}:{:05.2}",
        whole / 3600,
        whole / 60 % 60,
        seconds - (whole - whole % 60) as f64
    )
}

fn stop_text(stop: Stop) -> String {
    match stop {
        Stop::ErrorSteps(n) => format!("{n} consecutive step(s) without a lower error"),
        Stop::Time => "The training time limit was reached".into(),
        Stop::Epochs(n) => format!("The maximum of {n} epochs was reached"),
        Stop::ErrorChange(v) => format!("Relative change of the training error below {v}"),
        Stop::ErrorRatio(v) => format!("Training error ratio below {v}"),
        Stop::Converged => "The error gradient vanished".into(),
    }
}

const SAMPLES: [&str; 3] = ["Training", "Testing", "Holdout"];

/// Shares of `part` in percent; empty when the whole is 0.
fn percent(part: f64, whole: f64) -> Cell {
    if whole > 0.0 {
        Cell::Number(100.0 * part / whole)
    } else {
        Cell::Empty
    }
}

impl Engine {
    pub(crate) fn neural(&mut self, c: &Neural) -> Result<Vec<Table>, Error> {
        let who = if c.rbf { "RBF" } else { "MLP" };
        let mut warnings: Vec<String> = c.warnings.clone();
        let shown = self.shown();
        let scale_min = self.scale_min();
        let split = self.split_group.is_some();
        let data = self.active()?;
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        let unique = |names: Vec<String>| {
            let mut out: Vec<String> = Vec::new();
            for n in names {
                if !out.iter().any(|o| same(o, &n)) {
                    out.push(n);
                }
            }
            out
        };
        let mut dependents: Vec<(Variable, Level)> = Vec::new();
        for (name, written) in &c.dependents {
            let name = self.expand(&[VarItem::Name(name.clone())])?.remove(0);
            if dependents.iter().any(|(d, _)| same(d.name(), &name)) {
                return Err(Error(format!(
                    "{who}: dependent variable {name} is given twice"
                )));
            }
            let v = data.variable(&name)?;
            let level = level_of(data, &v, *written, scale_min);
            if level == Level::Scale && v.is_string() {
                return Err(Error(format!(
                    "{who}: the string variable {name} cannot be a scale dependent; give it MLEVEL=N or MLEVEL=O"
                )));
            }
            dependents.push((v, level));
        }
        let except = self.expand(&c.except)?;
        let mut lists = [
            unique(self.expand(&c.factors)?),
            unique(self.expand(&c.covariates)?),
        ];
        for list in lists.iter_mut() {
            list.retain(|n| !except.iter().any(|e| same(e, n)));
            let dependent = list
                .iter()
                .find(|n| dependents.iter().any(|(v, _)| same(v.name(), n)));
            if let (Some(d), false) = (dependent, c.rbf) {
                return Err(Error(format!(
                    "MLP: the dependent variable {d} cannot also be a predictor"
                )));
            }
            list.retain(|n| !dependents.iter().any(|(v, _)| same(v.name(), n)));
        }
        let [factor_names, covariate_names] = lists;
        if let Some(both) = factor_names
            .iter()
            .find(|f| covariate_names.iter().any(|v| same(v, f)))
        {
            return Err(Error(format!(
                "{who}: {both} is both a factor and a covariate"
            )));
        }
        if factor_names.is_empty() && covariate_names.is_empty() {
            return Err(Error(format!(
                "{who} needs at least one predictor: factors after BY or covariates after WITH"
            )));
        }
        let factors: Vec<Variable> = factor_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let covariates: Vec<Variable> = covariate_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = covariates.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "{who}: covariate {} is a string variable",
                v.name()
            )));
        }
        let partition = match &c.partition {
            Partition::Variable(name) => {
                let v = data.variable(name)?;
                if v.is_string() {
                    return Err(Error(format!(
                        "{who}: the partition variable {name} must be numeric"
                    )));
                }
                let used = dependents
                    .iter()
                    .map(|d| d.0.name())
                    .chain(factor_names.iter().map(String::as_str));
                if used
                    .chain(covariate_names.iter().map(String::as_str))
                    .any(|n| same(n, v.name()))
                {
                    return Err(Error(format!(
                        "{who}: the partition variable {name} cannot be a dependent, factor or covariate"
                    )));
                }
                Some(v)
            }
            Partition::Shares(..) => None,
        };
        if let Some(split) = data.split_file() {
            let all = dependents
                .iter()
                .map(|d| d.0.name())
                .chain(factors.iter().chain(&covariates).map(|v| v.name()));
            if let Some(n) = all
                .chain(partition.as_ref().map(|v| v.name()))
                .find(|n| split.variables.iter().any(|s| same(s, n)))
            {
                return Err(Error(format!(
                    "{who}: the split variable {n} cannot be used in the analysis"
                )));
            }
        }
        if case_weights(data)?.is_some() {
            warnings.push(format!("{who} does not use the weight variable"));
        }

        // the valid cases and their samples
        let cases = data.case_count();
        let dep_keys: Vec<Option<Vec<Option<Key>>>> = dependents
            .iter()
            .map(|(v, l)| (*l != Level::Scale).then(|| categories(v, c.include)))
            .collect();
        let factor_keys: Vec<Vec<Option<Key>>> =
            factors.iter().map(|v| categories(v, c.include)).collect();
        let dep_value =
            |d: usize, case: usize| -> Option<f64> { numeric_value(&dependents[d].0, case, false) };
        let mut sample: Vec<Option<usize>> = (0..cases)
            .map(|case| {
                let ok = (0..dependents.len()).all(|d| match &dep_keys[d] {
                    Some(k) => k[case].is_some(),
                    None => dep_value(d, case).is_some(),
                }) && factor_keys.iter().all(|k| k[case].is_some())
                    && covariates
                        .iter()
                        .all(|v| numeric_value(v, case, false).is_some());
                ok.then_some(0)
            })
            .collect();
        {
            let mut u = self.random.borrow_mut();
            for (case, s) in sample.iter_mut().enumerate() {
                if s.is_none() {
                    continue;
                }
                *s = match (&c.partition, &partition) {
                    (_, Some(v)) => numeric_value(v, case, true).map(|x| {
                        if x > 0.0 {
                            0
                        } else if x == 0.0 {
                            1
                        } else {
                            2
                        }
                    }),
                    (Partition::Shares(t, s, h), None) => {
                        let draw = u.uniform() * (t + s + h);
                        Some(if draw < *t {
                            0
                        } else if draw < t + s {
                            1
                        } else {
                            2
                        })
                    }
                    _ => unreachable!("a partition variable comes with VARIABLE"),
                };
            }
        }
        let training: Vec<usize> = (0..cases).filter(|&k| sample[k] == Some(0)).collect();
        if training.len() < 2 {
            return Err(Error(format!(
                "{who} needs at least 2 training cases without missing values, found {}",
                training.len()
            )));
        }

        // one-of-c coding and rescaling from the training cases
        let levels = |keys: &[Option<Key>]| {
            let mut found: Vec<Key> = training.iter().filter_map(|&k| keys[k].clone()).collect();
            found.sort_by(|a, b| a.compare(b));
            found.dedup_by(|a, b| a.compare(b).is_eq());
            found
        };
        let factor_levels: Vec<Vec<Key>> = factor_keys.iter().map(|k| levels(k)).collect();
        let covariate_scales: Vec<Linear> = covariates
            .iter()
            .map(|v| {
                let values: Vec<f64> = training
                    .iter()
                    .map(|&k| numeric_value(v, k, false).expect("valid"))
                    .collect();
                Linear::of(c.rescale_covariates, &values)
            })
            .collect();
        let any_scale = dependents.iter().any(|d| d.1 == Level::Scale);
        let a = &c.architecture;
        let output = match (c.rbf, a.automatic, a.output) {
            (true, ..) => Activation::Identity,
            (false, false, Some(f)) => f,
            _ if any_scale => Activation::Identity,
            _ => Activation::Softmax,
        };
        if output == Activation::Softmax && any_scale {
            return Err(Error(
                "MLP: OUTPUTFUNCTION=SOFTMAX needs every dependent to be categorical".into(),
            ));
        }
        let dependent_rescale = match (c.rbf || a.automatic, output, c.rescale_dependents) {
            (true, _, given) if c.rbf => given.unwrap_or(Rescale::Standardized),
            (true, ..) => Rescale::Standardized,
            (_, Activation::Sigmoid, None) => Rescale::Normalized(0.02),
            (_, Activation::Tanh, None) => Rescale::AdjNormalized(0.02),
            (_, _, None) => Rescale::Standardized,
            (_, f, Some(given)) => {
                let fits = match f {
                    Activation::Sigmoid => matches!(given, Rescale::Normalized(_)),
                    Activation::Tanh => matches!(given, Rescale::AdjNormalized(_)),
                    _ => true,
                };
                if !fits && any_scale {
                    return Err(Error(format!(
                        "MLP: with OUTPUTFUNCTION={} the scale dependents must be rescaled by {}",
                        if f == Activation::Sigmoid {
                            "SIGMOID"
                        } else {
                            "TANH"
                        },
                        if f == Activation::Sigmoid {
                            "NORMALIZED"
                        } else {
                            "ADJNORMALIZED"
                        }
                    )));
                }
                given
            }
        };
        let mut targets = Vec::new();
        let mut units = 0;
        for (d, (v, _)) in dependents.iter().enumerate() {
            targets.push(match &dep_keys[d] {
                Some(keys) => {
                    let found = levels(keys);
                    if found.len() < 2 {
                        warnings.push(format!(
                            "{who}: the dependent {} has only {} category in the training cases",
                            v.name(),
                            found.len()
                        ));
                    }
                    units += found.len();
                    Target::Categories(found.clone(), units - found.len())
                }
                None => {
                    units += 1;
                    let values: Vec<f64> = training
                        .iter()
                        .map(|&k| dep_value(d, k).expect("valid"))
                        .collect();
                    Target::Scale(Linear::of(dependent_rescale, &values))
                }
            });
        }

        let first_unit: Vec<usize> = targets
            .iter()
            .scan(0, |at, t| {
                let here = *at;
                *at += match t {
                    Target::Scale(_) => 1,
                    Target::Categories(found, _) => found.len(),
                };
                Some(here)
            })
            .collect();
        // the rows of the cases that can be scored; `used` marks those in their sample
        let mut rows: Vec<Row> = Vec::new();
        let mut row_of: Vec<Option<usize>> = vec![None; cases];
        let mut used = Vec::new();
        for case in 0..cases {
            let Some(s) = sample[case] else { continue };
            let mut x = Vec::new();
            let mut known = true;
            for (keys, found) in factor_keys.iter().zip(&factor_levels) {
                let key = keys[case].as_ref().expect("valid");
                let at = found.iter().position(|l| l.compare(key).is_eq());
                known &= at.is_some();
                x.extend((0..found.len()).map(|i| f64::from(u8::from(Some(i) == at))));
            }
            for (v, scale) in covariates.iter().zip(&covariate_scales) {
                x.push(scale.apply(numeric_value(v, case, false).expect("valid")));
            }
            if !known {
                sample[case] = None;
                continue;
            }
            let mut y = Vec::new();
            let mut fits = true;
            for (d, target) in targets.iter().enumerate() {
                match target {
                    Target::Scale(scale) => y.push(scale.apply(dep_value(d, case).expect("valid"))),
                    Target::Categories(found, _) => {
                        let key = dep_keys[d].as_ref().expect("categorical")[case]
                            .as_ref()
                            .expect("valid");
                        let at = found.iter().position(|l| l.compare(key).is_eq());
                        fits &= at.is_some();
                        y.extend((0..found.len()).map(|i| f64::from(u8::from(Some(i) == at))));
                    }
                }
            }
            row_of[case] = Some(rows.len());
            if fits {
                used.push((rows.len(), s));
            } else {
                sample[case] = None;
            }
            rows.push(Row { x, y });
        }
        let sets: Vec<Vec<usize>> = (0..3)
            .map(|s| used.iter().filter(|u| u.1 == s).map(|u| u.0).collect())
            .collect();
        let inputs = rows[0].x.len();
        let has_test = !sets[1].is_empty();
        if c.stopping.both && !c.rbf && !has_test {
            return Err(Error(
                "MLP: STOPPINGRULES ERRORSTEPS (DATA=BOTH) needs testing cases".into(),
            ));
        }
        let groups: Vec<std::ops::Range<usize>> = targets
            .iter()
            .filter_map(|t| match t {
                Target::Categories(found, at) => Some(*at..*at + found.len()),
                Target::Scale(_) => None,
            })
            .collect();

        // the network
        let started = std::time::Instant::now();
        let (fitted, hidden_units, overlap) = if c.rbf {
            let factor_counts: Vec<usize> = factor_levels.iter().map(Vec::len).collect();
            let covs = covariates.len();
            let records: Vec<Cf> = sets[0]
                .iter()
                .map(|&m| {
                    let x = &rows[m].x;
                    let mut at = 0;
                    let codes: Vec<usize> = factor_counts
                        .iter()
                        .map(|&n| {
                            let code = x[at..at + n].iter().position(|v| *v == 1.0).expect("coded");
                            at += n;
                            code
                        })
                        .collect();
                    Cf::record(&x[inputs - covs..], &codes, &factor_counts)
                })
                .collect();
            let adjust = (0..covs)
                .map(|k| {
                    let v: Vec<f64> = records.iter().map(|r| r.mean[k]).collect();
                    crate::twostep::variance(&v).max(1e-10)
                })
                .collect();
            let model = Clustering {
                adjust,
                euclidean: false,
            };
            let settings = Tree {
                threshold: 0.0,
                branch: 8,
                levels: 3,
                noise: 0.0,
            };
            let subs = precluster(&model, records, &settings).subclusters;
            let joins = agglomerate(&model, &subs);
            let found = auto_count(&model, &subs, &joins, 15, false).1;
            let h = c.criteria.overlap.unwrap_or(1.0 + 0.1 * inputs as f64);
            let (low, high) = match a.units.first() {
                Some(&n) => {
                    if has_test {
                        warnings.push(
                            "RBF: with NUMUNITS the testing cases do not choose the number of hidden units".into(),
                        );
                    }
                    (n, n)
                }
                None => {
                    let low = a.min_units.unwrap_or(match a.orbf {
                        true => found.min(units),
                        false => found.min(units).max(2),
                    });
                    (
                        low,
                        a.max_units.unwrap_or(10.max(2 * found).max(units)).max(low),
                    )
                }
            };
            let possible = subs.len();
            if low > possible {
                warnings.push(format!(
                    "RBF: the training cases make only {possible} distinct groups, so the network has {possible} hidden units"
                ));
            }
            let m = sets[0].len() as f64;
            let r = units as f64;
            let mut best: Option<(f64, Rbf)> = None;
            for j in low.min(possible)..=high.min(possible) {
                let (centers, widths) =
                    rbf_units(&clusters_at(&subs, &joins, j), &factor_counts, h);
                let rbf = fit_rbf(&rows, &sets[0], centers, widths, a.orbf);
                let fitted = Fitted::Rbf(rbf);
                let score = if has_test && a.units.is_empty() {
                    sum_squares(&fitted, &rows, &sets[1])
                } else {
                    let mse = 2.0 * sum_squares(&fitted, &rows, &sets[0]) / (m * r);
                    let k = (inputs as f64 + 1.0 + r) * j as f64 + if a.orbf { r } else { 0.0 };
                    m * r * mse.max(1e-300).ln() + k * m.ln()
                };
                let Fitted::Rbf(rbf) = fitted else {
                    unreachable!()
                };
                if best.as_ref().is_none_or(|b| score < b.0) {
                    best = Some((score, rbf));
                }
            }
            let rbf = best.expect("one network").1;
            let j = rbf.centers.len();
            (Fitted::Rbf(rbf), vec![j], Some(h))
        } else {
            let cr = &c.criteria;
            let m = sets[0].len();
            let (p, r) = (inputs, units);
            let g = if r < 5 && p >= 8 {
                (4.5 + ((p + r) as f64).sqrt()).floor()
            } else {
                (0.5 + 0.5 * (p + r) as f64).floor()
            } as usize;
            let h = m.saturating_sub(r) / (p + r + 1);
            let start = g.min(20).min(h).max(1);
            let hidden = if a.automatic {
                Activation::Tanh
            } else {
                a.hidden
            };
            let mut net = Net {
                sizes: vec![p, start, r],
                hidden,
                output,
                groups: groups.clone(),
            };
            if !a.automatic && a.hidden_layers == 2 {
                let second = a.units.get(1).copied().unwrap_or(start.div_ceil(2));
                net.sizes = vec![p, a.units.first().copied().unwrap_or(start), second, r];
            } else if !a.automatic {
                net.sizes[1] = a.units.first().copied().unwrap_or(start);
            }
            let mut u = self.random.borrow_mut();
            let mut draw = || u.uniform();
            let interval = (cr.interval_center, cr.interval_offset);
            let sample = Sample::draw(&sets[0], &sets[1], cr.memsize.min(1000), &mut draw);
            let w0 = if a.automatic {
                let (min, max) = (a.min_units.unwrap_or(1), a.max_units.unwrap_or(50));
                let (k, w) = choose_units(
                    &net,
                    &rows,
                    &sample,
                    (start.clamp(min, max), min, max),
                    interval,
                    &mut draw,
                );
                net.sizes[1] = k;
                w
            } else {
                anneal(&net, &rows, &sample, interval, None, &mut draw).0
            };
            let mut order = sets[0].clone();
            shuffle(&mut order, &mut draw);
            drop(u);
            let s = Settings {
                training: cr.training,
                scaled_conjugate: cr
                    .scaled_conjugate
                    .unwrap_or(cr.training == Training::Batch),
                eta0: cr.learning_initial,
                eta_low: cr.learning_lower,
                momentum: cr.momentum,
                learning_epochs: cr.learning_epochs,
                lambda0: cr.lambda_initial,
                sigma0: cr.sigma_initial,
                batch: match cr.training {
                    Training::Online => 1,
                    _ => cr
                        .minibatch
                        .unwrap_or((m / 10).max(2).min(cr.memsize))
                        .min(m),
                },
                error_steps: c.stopping.error_steps,
                both: c.stopping.both,
                max_minutes: c.stopping.max_time,
                max_epochs: c.stopping.max_epochs,
                error_change: c.stopping.error_change,
                error_ratio: c.stopping.error_ratio,
            };
            let trained = train(&net, &s, w0, &rows, &sets[0], &sets[1], &order);
            let hidden_units = net.sizes[1..net.sizes.len() - 1].to_vec();
            (
                Fitted::Mlp(net, trained.w, trained.stop),
                hidden_units,
                None,
            )
        };
        let seconds = started.elapsed().as_secs_f64();
        let outputs: Vec<Option<Vec<f64>>> = (0..cases)
            .map(|k| row_of[k].map(|r| fitted.predict(&rows[r].x)))
            .collect();

        // labels of the units
        let dep_name = |d: usize| shown.variable(&dependents[d].0);
        let mut input_labels: Vec<String> = Vec::new();
        for (v, found) in factors.iter().zip(&factor_levels) {
            input_labels.extend(
                found
                    .iter()
                    .map(|k| format!("[{}={}]", v.name(), shown.value(v, k))),
            );
        }
        input_labels.extend(covariates.iter().map(|v| v.name().to_string()));
        let mut output_labels: Vec<String> = Vec::new();
        for (d, t) in targets.iter().enumerate() {
            let v = &dependents[d].0;
            match t {
                Target::Scale(_) => output_labels.push(v.name().to_string()),
                Target::Categories(found, _) => output_labels.extend(
                    found
                        .iter()
                        .map(|k| format!("[{}={}]", v.name(), shown.value(v, k))),
                ),
            }
        }
        // per dependent and used row: the predicted category index or the rescaled prediction
        let predicted_category = |out: &[f64], range: &std::ops::Range<usize>| {
            range
                .clone()
                .max_by(|a, b| out[*a].total_cmp(&out[*b]).then(b.cmp(a)))
                .expect("categories")
                - range.start
        };
        let cross_entropy = output == Activation::Softmax;
        let mut tables = Vec::new();
        let p = &c.print;

        if p[0] {
            let valid: usize = sets.iter().map(Vec::len).sum();
            let mut rows_out = Vec::new();
            for (s, name) in SAMPLES.iter().enumerate() {
                let n = sets[s].len() as f64;
                if s < 2 || n > 0.0 {
                    let head = if s == 0 { text("Sample") } else { Cell::Empty };
                    rows_out.push(vec![
                        head,
                        text(*name),
                        Cell::Number(n),
                        percent(n, valid as f64),
                    ]);
                }
            }
            rows_out.push(vec![
                text("Valid"),
                Cell::Empty,
                Cell::Number(valid as f64),
                percent(valid as f64, valid as f64),
            ]);
            rows_out.push(vec![
                text("Excluded"),
                Cell::Empty,
                Cell::Number((cases - valid) as f64),
                Cell::Empty,
            ]);
            rows_out.push(vec![
                text("Total"),
                Cell::Empty,
                Cell::Number(cases as f64),
                Cell::Empty,
            ]);
            tables.push(plain_table(
                "Case Processing Summary",
                2,
                &["", "", "N", "Percent"],
                rows_out,
            ));
        }
        if p[1] {
            let mut info: Vec<Vec<Cell>> = Vec::new();
            let mut add = |layer: &str, item: String, value: Cell| {
                let first = !info
                    .iter()
                    .any(|r: &Vec<Cell>| matches!(&r[0], Cell::Text(t) if t == layer));
                info.push(vec![
                    if first { text(layer) } else { Cell::Empty },
                    text(&item),
                    value,
                ]);
            };
            for (i, v) in factors.iter().enumerate() {
                add(
                    "Input Layer",
                    format!("Factors {}", i + 1),
                    text(shown.variable(v)),
                );
            }
            for (i, v) in covariates.iter().enumerate() {
                add(
                    "Input Layer",
                    format!("Covariates {}", i + 1),
                    text(shown.variable(v)),
                );
            }
            add(
                "Input Layer",
                "Number of Units".into(),
                Cell::Number(inputs as f64),
            );
            if !covariates.is_empty() {
                add(
                    "Input Layer",
                    "Rescaling Method for Covariates".into(),
                    text(rescale_name(c.rescale_covariates)),
                );
            }
            let hidden_layer = "Hidden Layer(s)";
            add(
                hidden_layer,
                "Number of Hidden Layers".into(),
                Cell::Number(hidden_units.len() as f64),
            );
            for (i, n) in hidden_units.iter().enumerate() {
                add(
                    hidden_layer,
                    format!("Number of Units in Hidden Layer {}", i + 1),
                    Cell::Number(*n as f64),
                );
            }
            let hidden_function = match &fitted {
                Fitted::Mlp(net, ..) => activation_name(net.hidden),
                Fitted::Rbf(rbf) if rbf.orbf => "Exponential (ordinary radial basis)",
                Fitted::Rbf(_) => "Softmax (normalized radial basis)",
            };
            add(
                hidden_layer,
                "Activation Function".into(),
                text(hidden_function),
            );
            for d in 0..dependents.len() {
                add(
                    "Output Layer",
                    format!("Dependent Variables {}", d + 1),
                    text(dep_name(d)),
                );
            }
            add(
                "Output Layer",
                "Number of Units".into(),
                Cell::Number(units as f64),
            );
            if any_scale {
                add(
                    "Output Layer",
                    "Rescaling Method for Scale Dependents".into(),
                    text(rescale_name(dependent_rescale)),
                );
            }
            add(
                "Output Layer",
                "Activation Function".into(),
                text(activation_name(output)),
            );
            let error = if cross_entropy {
                "Cross-entropy"
            } else {
                "Sum of Squares"
            };
            add("Output Layer", "Error Function".into(), text(error));
            let mut table = plain_table("Network Information", 2, &["", "", "Value"], info);
            if let Some(h) = overlap {
                table.caption = Some(format!(
                    "Overlap factor of the hidden units: {h}. The counts of units leave out the bias."
                ));
            } else {
                table.caption = Some("The counts of units leave out the bias.".into());
            }
            tables.push(table);
        }
        // the summary of each sample
        let used_cases =
            |s: usize| -> Vec<usize> { (0..cases).filter(|&k| sample[k] == Some(s)).collect() };
        if p[2] {
            let mut summary = Vec::new();
            for (s, name) in SAMPLES.iter().enumerate() {
                let members = used_cases(s);
                if members.is_empty() {
                    continue;
                }
                let error: f64 = members
                    .iter()
                    .map(|&k| {
                        let r = &rows[row_of[k].expect("scored")];
                        match &fitted {
                            Fitted::Mlp(net, w, _) => net.case_error(&net.predict(w, &r.x), &r.y),
                            Fitted::Rbf(_) => {
                                let a = outputs[k].as_ref().expect("scored");
                                0.5 * a
                                    .iter()
                                    .zip(&r.y)
                                    .map(|(a, y)| (y - a) * (y - a))
                                    .sum::<f64>()
                            }
                        }
                    })
                    .sum();
                let mut lines = vec![(
                    if cross_entropy {
                        "Cross Entropy Error"
                    } else {
                        "Sum of Squares Error"
                    }
                    .to_string(),
                    Cell::Number(error),
                )];
                let (mut res, mut tot, mut wrong) = (0.0, 0.0, Vec::new());
                let mut each = Vec::new();
                for (d, t) in targets.iter().enumerate() {
                    match t {
                        Target::Scale(_) => {
                            let unit = first_unit[d];
                            let ys: Vec<(f64, f64)> = members
                                .iter()
                                .map(|&k| {
                                    (
                                        rows[row_of[k].expect("scored")].y[unit],
                                        outputs[k].as_ref().expect("scored")[unit],
                                    )
                                })
                                .collect();
                            let mean = ys.iter().map(|v| v.0).sum::<f64>() / ys.len() as f64;
                            let r: f64 = ys.iter().map(|(y, a)| (y - a) * (y - a)).sum();
                            let t2: f64 = ys.iter().map(|(y, _)| (y - mean) * (y - mean)).sum();
                            res += r;
                            tot += t2;
                            each.push((
                                format!("Relative Error for {}", dep_name(d)),
                                number((t2 > 0.0).then(|| r / t2)),
                            ));
                        }
                        Target::Categories(found, at) => {
                            let range = *at..*at + found.len();
                            let missed = members
                                .iter()
                                .filter(|&&k| {
                                    let y = &rows[row_of[k].expect("scored")].y;
                                    let out = outputs[k].as_ref().expect("scored");
                                    y[range.start + predicted_category(out, &range)] != 1.0
                                })
                                .count();
                            let share = 100.0 * missed as f64 / members.len() as f64;
                            wrong.push(share);
                            each.push((
                                format!("Percent Incorrect Predictions for {}", dep_name(d)),
                                Cell::Number(share),
                            ));
                        }
                    }
                }
                if any_scale {
                    lines.push((
                        "Relative Error".into(),
                        number((tot > 0.0).then(|| res / tot)),
                    ));
                } else {
                    let mean = wrong.iter().sum::<f64>() / wrong.len() as f64;
                    lines.push(("Percent Incorrect Predictions".into(), Cell::Number(mean)));
                }
                if dependents.len() > 1 {
                    lines.extend(each);
                }
                if s == 0 {
                    if let Fitted::Mlp(_, _, stop) = &fitted {
                        lines.push(("Stopping Rule Used".into(), text(stop_text(*stop))));
                    }
                    lines.push(("Training Time".into(), text(duration(seconds))));
                }
                for (i, (label, value)) in lines.into_iter().enumerate() {
                    summary.push(vec![
                        if i == 0 { text(*name) } else { Cell::Empty },
                        text(&label),
                        value,
                    ]);
                }
            }
            let mut table = plain_table("Model Summary", 2, &["", "", "Value"], summary);
            table.caption = Some(format!(
                "Dependent variables: {}. Errors of scale dependents are on their rescaled values.",
                (0..dependents.len())
                    .map(dep_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            tables.push(table);
        }
        if p[3] {
            for (d, t) in targets.iter().enumerate() {
                let Target::Categories(found, at) = t else {
                    continue;
                };
                let v = &dependents[d].0;
                let range = *at..*at + found.len();
                let labels: Vec<String> = found.iter().map(|k| shown.value(v, k)).collect();
                let mut body = Vec::new();
                for (s, name) in SAMPLES.iter().enumerate() {
                    let members = used_cases(s);
                    if members.is_empty() {
                        continue;
                    }
                    let mut counts = vec![vec![0.0; found.len()]; found.len()];
                    for &k in &members {
                        let y = &rows[row_of[k].expect("scored")].y;
                        let observed = (0..found.len())
                            .find(|i| y[range.start + i] == 1.0)
                            .expect("coded");
                        counts[observed]
                            [predicted_category(outputs[k].as_ref().expect("scored"), &range)] +=
                            1.0;
                    }
                    let n = members.len() as f64;
                    for (o, row) in counts.iter().enumerate() {
                        let mut cells = vec![
                            if o == 0 { text(*name) } else { Cell::Empty },
                            text(&labels[o]),
                        ];
                        cells.extend(row.iter().map(|c| Cell::Number(*c)));
                        cells.push(percent(row[o], row.iter().sum()));
                        body.push(cells);
                    }
                    let mut overall = vec![Cell::Empty, text("Overall Percent")];
                    overall.extend(
                        (0..found.len()).map(|j| percent(counts.iter().map(|r| r[j]).sum(), n)),
                    );
                    overall.push(percent((0..found.len()).map(|i| counts[i][i]).sum(), n));
                    body.push(overall);
                }
                let mut columns = vec!["Sample".to_string(), "Observed".into()];
                columns.extend(labels.iter().cloned());
                columns.push("Percent Correct".into());
                let mut table = plain_table("Classification", 2, &columns, body);
                table.column_groups = columns
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        if i >= 2 && i < 2 + labels.len() {
                            vec!["Predicted".to_string()]
                        } else {
                            Vec::new()
                        }
                    })
                    .collect();
                table.caption = Some(format!("Dependent variable: {}", dep_name(d)));
                tables.push(table);
            }
        }
        if p[4] {
            tables.push(match &fitted {
                Fitted::Mlp(net, w, _) => mlp_solution(net, w, &input_labels, &output_labels),
                Fitted::Rbf(rbf) => rbf_solution(rbf, &input_labels, &output_labels),
            });
        }
        // the cases the importance and the charts describe: training and testing
        let described: Vec<usize> = (0..cases)
            .filter(|&k| matches!(sample[k], Some(0 | 1)))
            .collect();
        let mut charts = Vec::new();
        if p[5] {
            // the input units of each predictor and the values they take
            let mut spans: Vec<(String, std::ops::Range<usize>)> = Vec::new();
            let mut at = 0;
            for (v, found) in factors.iter().zip(&factor_levels) {
                spans.push((shown.variable(v), at..at + found.len()));
                at += found.len();
            }
            for v in &covariates {
                spans.push((shown.variable(v), at..at + 1));
                at += 1;
            }
            let quartiles = |unit: usize| -> Vec<Vec<f64>> {
                let mut xs: Vec<f64> = described
                    .iter()
                    .map(|&k| rows[row_of[k].expect("scored")].x[unit])
                    .collect();
                xs.sort_by(f64::total_cmp);
                let n = xs.len() - 1;
                [0.0, 0.25, 0.5, 0.75, 1.0]
                    .iter()
                    .map(|q| {
                        let pos = q * n as f64;
                        let (i, f) = (pos.floor() as usize, pos.fract());
                        vec![xs[i] + f * (xs[(i + 1).min(n)] - xs[i])]
                    })
                    .collect()
            };
            let mut importance: Vec<f64> = spans
                .iter()
                .map(|(_, span)| {
                    let values: Vec<Vec<f64>> = match span.len() {
                        1 if span.start >= inputs - covariates.len() => quartiles(span.start),
                        n => (0..n)
                            .map(|i| (0..n).map(|j| f64::from(u8::from(i == j))).collect())
                            .collect(),
                    };
                    let total: f64 = described
                        .iter()
                        .map(|&k| {
                            let mut x = rows[row_of[k].expect("scored")].x.clone();
                            let outs: Vec<Vec<f64>> = values
                                .iter()
                                .map(|v| {
                                    x[span.clone()].copy_from_slice(v);
                                    fitted.predict(&x)
                                })
                                .collect();
                            let mut most: f64 = 0.0;
                            for a in &outs {
                                for b in &outs {
                                    most = most.max(
                                        a.iter()
                                            .zip(b)
                                            .map(|(a, b)| (a - b) * (a - b))
                                            .sum::<f64>()
                                            .sqrt(),
                                    );
                                }
                            }
                            most
                        })
                        .sum();
                    total / described.len().max(1) as f64
                })
                .collect();
            let sum: f64 = importance.iter().sum();
            if sum > 0.0 {
                importance.iter_mut().for_each(|v| *v /= sum);
            }
            let top = importance.iter().copied().fold(0.0, f64::max);
            let normalized: Vec<f64> = importance
                .iter()
                .map(|v| if top > 0.0 { 100.0 * v / top } else { 0.0 })
                .collect();
            let body = spans
                .iter()
                .zip(importance.iter().zip(&normalized))
                .map(|((name, _), (i, n))| vec![text(name), Cell::Number(*i), Cell::Number(*n)])
                .collect();
            tables.push(plain_table(
                "Independent Variable Importance",
                1,
                &["", "Importance", "Normalized Importance"],
                body,
            ));
            let mut chart = Chart::lines(
                "Normalized Importance".into(),
                Axis::titled("Predictor"),
                Axis::titled("Normalized Importance"),
                spans
                    .iter()
                    .map(|(name, _)| Category {
                        label: name.clone(),
                        value: Cell::Empty,
                        missing: false,
                        bin: None,
                    })
                    .collect(),
                vec![Series {
                    name: "Normalized Importance".into(),
                    statistic: Statistic::Value,
                    variable: None,
                    value: Cell::Empty,
                    values: normalized,
                }],
            );
            chart.kind = ChartKind::Bar;
            chart.orientation = Some(Orientation::Horizontal);
            charts.push(chart);
        }
        let plot = &c.plot;
        if plot[0] {
            let mut layers: Vec<(String, Vec<String>)> = vec![("Input layer".into(), {
                let mut l = vec!["(Bias)".to_string()];
                l.extend(input_labels.iter().cloned());
                l
            })];
            for (i, n) in hidden_units.iter().enumerate() {
                let mut l: Vec<String> = match &fitted {
                    Fitted::Rbf(rbf) if !rbf.orbf => Vec::new(),
                    _ => vec!["(Bias)".into()],
                };
                l.extend((1..=*n).map(|j| format!("H({}:{j})", i + 1)));
                layers.push((format!("Hidden layer {}", i + 1), l));
            }
            layers.push(("Output layer".into(), output_labels.clone()));
            let groups = layers
                .into_iter()
                .enumerate()
                .map(|(i, (name, units))| {
                    let half = (units.len() as f64 - 1.0) / 2.0;
                    PointGroup {
                        name,
                        points: units
                            .into_iter()
                            .enumerate()
                            .map(|(j, label)| Point {
                                x: i as f64,
                                y: half - j as f64,
                                z: None,
                                label: Some(label),
                            })
                            .collect(),
                    }
                })
                .collect();
            let mut chart = Chart::plot(
                ChartKind::Scatter,
                "Network Diagram".into(),
                Axis::titled("Layer"),
                Axis::titled("Unit"),
                Plot::Points {
                    groups,
                    lines: Vec::new(),
                    z_axis: None,
                },
            );
            chart.footnote = Some(
                "The units of each layer; PRINT SOLUTION lists the weights between them.".into(),
            );
            charts.push(chart);
        }
        for (d, t) in targets.iter().enumerate() {
            let v = &dependents[d].0;
            match t {
                Target::Scale(scale) => {
                    let unit = first_unit[d];
                    let pairs: Vec<(f64, f64)> = described
                        .iter()
                        .map(|&k| {
                            (
                                dep_value(d, k).expect("valid"),
                                scale.back(outputs[k].as_ref().expect("scored")[unit]),
                            )
                        })
                        .collect();
                    let scatter = |title: String, x: &str, y: &str, points: Vec<(f64, f64)>| {
                        Chart::plot(
                            ChartKind::Scatter,
                            title,
                            Axis::titled(x),
                            Axis::titled(y),
                            Plot::Points {
                                groups: vec![PointGroup {
                                    name: v.name().to_string(),
                                    points: points
                                        .into_iter()
                                        .map(|(x, y)| Point {
                                            x,
                                            y,
                                            z: None,
                                            label: None,
                                        })
                                        .collect(),
                                }],
                                lines: Vec::new(),
                                z_axis: None,
                            },
                        )
                    };
                    if plot[1] {
                        charts.push(scatter(
                            format!("Predicted by observed: {}", dep_name(d)),
                            "Observed",
                            "Predicted",
                            pairs.clone(),
                        ));
                    }
                    if plot[2] {
                        let residuals = pairs.iter().map(|(y, a)| (*a, y - a)).collect();
                        charts.push(scatter(
                            format!("Residual by predicted: {}", dep_name(d)),
                            "Predicted",
                            "Residual",
                            residuals,
                        ));
                    }
                }
                Target::Categories(found, at) => {
                    let range = *at..*at + found.len();
                    let labels: Vec<String> = found.iter().map(|k| shown.value(v, k)).collect();
                    // (case, observed category, rescaled pseudo-probabilities)
                    let scored: Vec<(usize, usize, Vec<f64>)> = described
                        .iter()
                        .map(|&k| {
                            let y = &rows[row_of[k].expect("scored")].y;
                            let o = (0..found.len())
                                .find(|i| y[range.start + i] == 1.0)
                                .expect("coded");
                            (
                                k,
                                o,
                                outputs[k].as_ref().expect("scored")[range.clone()].to_vec(),
                            )
                        })
                        .collect();
                    if plot[1] {
                        let mut boxes = Vec::new();
                        for (o, observed) in labels.iter().enumerate() {
                            for (j, predicted) in labels.iter().enumerate() {
                                let values: Vec<(f64, usize)> = scored
                                    .iter()
                                    .filter(|s| s.1 == o)
                                    .map(|s| (s.2[j], s.0))
                                    .collect();
                                boxes.extend(box_of(observed.clone(), predicted.clone(), &values));
                            }
                        }
                        let mut chart = Chart::plot(
                            ChartKind::Boxplot,
                            format!("Predicted pseudo-probability by observed: {}", dep_name(d)),
                            Axis::titled("Observed"),
                            Axis::titled("Predicted pseudo-probability"),
                            Plot::Boxes { boxes },
                        );
                        chart.legend = Some("Predicted category".into());
                        charts.push(chart);
                    }
                    let mut curves = [Vec::new(), Vec::new(), Vec::new()];
                    let mut areas = Vec::new();
                    for (j, label) in labels.iter().enumerate() {
                        let mut s: Vec<(f64, bool)> = scored
                            .iter()
                            .map(|x| (rescaled(&x.2)[j], x.1 == j))
                            .collect();
                        areas.push(vec![
                            if j == 0 {
                                text(dep_name(d))
                            } else {
                                Cell::Empty
                            },
                            text(label),
                            number(roc_area(&s)),
                        ]);
                        s.sort_by(|a, b| b.0.total_cmp(&a.0));
                        let (pos, n) = (s.iter().filter(|x| x.1).count() as f64, s.len() as f64);
                        let mut roc = vec![Point {
                            x: 0.0,
                            y: 0.0,
                            z: None,
                            label: None,
                        }];
                        let (mut tp, mut fp) = (0.0, 0.0);
                        for (i, x) in s.iter().enumerate() {
                            if x.1 {
                                tp += 1.0
                            } else {
                                fp += 1.0
                            }
                            if s.get(i + 1).is_none_or(|next| next.0 != x.0) {
                                roc.push(Point {
                                    x: fp / (n - pos).max(1.0),
                                    y: tp / pos.max(1.0),
                                    z: None,
                                    label: None,
                                });
                            }
                        }
                        let mut gain = vec![Point {
                            x: 0.0,
                            y: 0.0,
                            z: None,
                            label: None,
                        }];
                        let mut lift = Vec::new();
                        for q in 1..=20 {
                            let share = q as f64 / 20.0;
                            let top = ((share * n).ceil() as usize).min(s.len());
                            let hits = s[..top].iter().filter(|x| x.1).count() as f64;
                            let g = 100.0 * hits / pos.max(1.0);
                            gain.push(Point {
                                x: 100.0 * share,
                                y: g,
                                z: None,
                                label: None,
                            });
                            lift.push(Point {
                                x: 100.0 * share,
                                y: g / (100.0 * share),
                                z: None,
                                label: None,
                            });
                        }
                        for (curve, points) in curves.iter_mut().zip([roc, gain, lift]) {
                            curve.push(PointGroup {
                                name: label.clone(),
                                points,
                            });
                        }
                    }
                    let [roc, gain, lift] = curves;
                    let titles = [
                        ("ROC curve", "1 - Specificity", "Sensitivity", 3, roc),
                        (
                            "Cumulative gains",
                            "Percentage of cases",
                            "Gain (%)",
                            4,
                            gain,
                        ),
                        ("Lift", "Percentage of cases", "Lift", 5, lift),
                    ];
                    for (title, x, y, flag, groups) in titles {
                        if plot[flag] {
                            let mut chart = Chart::plot(
                                ChartKind::Roc,
                                format!("{title}: {}", dep_name(d)),
                                Axis::titled(x),
                                Axis::titled(y),
                                Plot::Points {
                                    groups,
                                    lines: Vec::new(),
                                    z_axis: None,
                                },
                            );
                            chart.legend = Some("Category".into());
                            charts.push(chart);
                        }
                    }
                    if plot[3] {
                        let mut table =
                            plain_table("Area Under the Curve", 2, &["", "", "Area"], areas);
                        table.caption = Some("Each category against all others, from the training and testing cases.".into());
                        tables.push(table);
                    }
                }
            }
        }

        // the model files: the whole network, one file per dependent
        let model_json = (!c.outfile.is_empty() && !split).then(|| {
            let inputs_json: Vec<_> = factors
                .iter()
                .zip(&factor_levels)
                .map(|(v, found)| json!({"name": v.name(), "factor": found.iter().map(key_json).collect::<Vec<_>>()}))
                .chain(covariates.iter().zip(&covariate_scales).map(|(v, l)| {
                    json!({"name": v.name(), "covariate": {"shift": l.shift, "scale": l.scale}})
                }))
                .collect();
            let outputs_json: Vec<_> = targets
                .iter()
                .enumerate()
                .map(|(d, t)| match t {
                    Target::Scale(l) => json!({"name": dependents[d].0.name(), "scale": {"shift": l.shift, "scale": l.scale}}),
                    Target::Categories(found, _) => json!({"name": dependents[d].0.name(), "categories": found.iter().map(key_json).collect::<Vec<_>>()}),
                })
                .collect();
            let network = match &fitted {
                Fitted::Mlp(net, w, _) => json!({
                    "sizes": net.sizes,
                    "hidden": activation_name(net.hidden),
                    "output": activation_name(net.output),
                    "weights": w,
                    "weight_order": "layer by layer; for each unit of the upper layer its bias, then one weight per unit below",
                }),
                Fitted::Rbf(rbf) => json!({
                    "function": if rbf.orbf { "ordinary" } else { "normalized" },
                    "centers": rbf.centers,
                    "widths": rbf.widths,
                    "weights": rbf.weights,
                }),
            };
            json!({
                "format": if c.rbf { "statscraft-rbf-model" } else { "statscraft-mlp-model" },
                "inputs": inputs_json,
                "outputs": outputs_json,
                "network": network,
            })
        });
        if !c.outfile.is_empty() && split {
            warnings.push(format!("{who} /OUTFILE is not used with SPLIT FILE"));
        }

        // saved variables: (name or None for the default root, root, label, values, category texts)
        let mut saves: Vec<(Option<String>, String, String, SaveValues)> = Vec::new();
        let several = dependents.len() > 1;
        let mut categorical_seen = 0;
        for (d, t) in targets.iter().enumerate() {
            let v = &dependents[d].0;
            if let Some(names) = &c.predval {
                let root = match several {
                    true => format!("{who}_PredictedValue_{}", d + 1),
                    false => format!("{who}_PredictedValue"),
                };
                let values = match t {
                    Target::Scale(scale) => {
                        let unit = first_unit[d];
                        SaveValues::Numbers(
                            outputs
                                .iter()
                                .map(|o| o.as_ref().map(|o| scale.back(o[unit])))
                                .collect(),
                            Measure::Scale,
                        )
                    }
                    Target::Categories(found, at) => {
                        let range = *at..*at + found.len();
                        let best: Vec<Option<&Key>> = outputs
                            .iter()
                            .map(|o| o.as_ref().map(|o| &found[predicted_category(o, &range)]))
                            .collect();
                        match v.is_string() {
                            true => SaveValues::Texts(
                                best.iter()
                                    .map(|k| match k {
                                        Some(Key::Text(t)) => t.clone(),
                                        _ => String::new(),
                                    })
                                    .collect(),
                            ),
                            false => SaveValues::Numbers(
                                best.iter()
                                    .map(|k| match k {
                                        Some(Key::Number(n)) => Some(*n),
                                        _ => None,
                                    })
                                    .collect(),
                                Measure::Nominal,
                            ),
                        }
                    }
                };
                saves.push((
                    names.get(d).cloned(),
                    root,
                    format!("Predicted value or category for {}", v.name()),
                    values,
                ));
            }
            if let (Some(roots), Target::Categories(found, at)) = (&c.pseudoprob, t) {
                let given = roots.get(categorical_seen);
                let (root, count) = match given {
                    Some((r, n)) => (r.clone(), *n),
                    None if several => (format!("{who}_PseudoProbability_{}", d + 1), 25),
                    None => (format!("{who}_PseudoProbability"), 25),
                };
                for j in 0..found.len().min(count) {
                    let name = format!("{root}_{}", j + 1);
                    let values = outputs
                        .iter()
                        .map(|o| o.as_ref().map(|o| o[at + j]))
                        .collect();
                    let label = format!(
                        "Predicted pseudo-probability of {}={}",
                        v.name(),
                        shown.value(v, &found[j])
                    );
                    saves.push((
                        given.map(|_| name.clone()),
                        name,
                        label,
                        SaveValues::Numbers(values, Measure::Scale),
                    ));
                }
            }
            if matches!(t, Target::Categories(..)) {
                categorical_seen += 1;
            }
        }
        let names: Vec<String> = dependents.iter().map(|d| d.0.name().to_string()).collect();
        for w in warnings {
            self.warn(w);
        }
        self.charts.extend(charts);
        if let Some(model) = model_json {
            for (name, file) in names.iter().zip(&c.outfile) {
                let mut one = model.clone();
                one["dependent"] = json!(name);
                self.write_json(who, file, "model", &one)?;
            }
        }
        if !saves.is_empty() {
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                for (name, root, label, values) in saves {
                    let name = match name {
                        Some(n) if data.variable_index(&n).is_some() => {
                            return Err(Error(format!(
                                "{who} /SAVE: variable {n} already exists; choose a new name"
                            )));
                        }
                        Some(n) => n,
                        None => free_name(&root, |n| data.variable_index(n).is_some()),
                    };
                    match values {
                        SaveValues::Numbers(values, measure) => {
                            data.set_numeric_values(&name, values)?;
                            data.set_format(
                                &name,
                                if measure == Measure::Scale {
                                    "F8.3"
                                } else {
                                    "F8.0"
                                },
                            )?;
                            data.set_measure(&name, measure)?;
                        }
                        SaveValues::Texts(values) => {
                            let width = values.iter().map(String::len).max().unwrap_or(1).max(1);
                            data.set_string_values(&name, width, values)?;
                        }
                    }
                    data.set_variable_label(&name, Some(&label))?;
                }
                Ok(())
            })?;
        }
        Ok(tables)
    }
}

enum SaveValues {
    Numbers(Vec<Option<f64>>, Measure),
    Texts(Vec<String>),
}

/// The weights as a table: one row per unit they come from, one column per unit they go to.
fn mlp_solution(net: &Net, w: &[f64], inputs: &[String], outputs: &[String]) -> Table {
    let layers = net.sizes.len();
    let name = |i: usize, j: usize| match i {
        0 => inputs[j].clone(),
        _ if i + 1 == layers => outputs[j].clone(),
        _ => format!("H({i}:{})", j + 1),
    };
    let layer_name = |i: usize| match i {
        0 => "Input Layer".to_string(),
        _ if i + 1 == layers => "Output Layer".into(),
        _ => format!("Hidden Layer {i}"),
    };
    let mut columns = vec![String::new(), String::new()];
    let mut groups = vec![Vec::new(), Vec::new()];
    for (i, &size) in net.sizes.iter().enumerate().take(layers).skip(1) {
        for j in 0..size {
            columns.push(name(i, j));
            groups.push(vec![layer_name(i)]);
        }
    }
    let width = columns.len();
    let mut body = Vec::new();
    let mut at = 0;
    let mut column = 2;
    for i in 0..layers - 1 {
        let n = net.sizes[i] + 1;
        for j in 0..n {
            let mut row = vec![
                if j == 0 {
                    text(layer_name(i))
                } else {
                    Cell::Empty
                },
                text(&if j == 0 {
                    "(Bias)".to_string()
                } else {
                    name(i, j - 1)
                }),
            ];
            row.resize(width, Cell::Empty);
            for k in 0..net.sizes[i + 1] {
                row[column + k] = Cell::Number(w[at + k * n + j]);
            }
            body.push(row);
        }
        at += n * net.sizes[i + 1];
        column += net.sizes[i + 1];
    }
    let mut table = plain_table("Parameter Estimates", 2, &columns, body);
    table.column_groups = groups;
    table.caption =
        Some("Rows: the unit a weight comes from; columns: the unit it goes to.".into());
    table
}

/// The centers and widths of the hidden units and the weights to the outputs.
fn rbf_solution(rbf: &Rbf, inputs: &[String], outputs: &[String]) -> Table {
    let j = rbf.centers.len();
    let mut columns = vec![String::new(), String::new()];
    let mut groups = vec![Vec::new(), Vec::new()];
    for k in 1..=j {
        columns.push(format!("H({k})"));
        groups.push(vec!["Hidden Layer".to_string()]);
    }
    for o in outputs {
        columns.push(o.clone());
        groups.push(vec!["Output Layer".to_string()]);
    }
    let width = columns.len();
    let mut body = Vec::new();
    for (p, label) in inputs.iter().enumerate() {
        let mut row = vec![
            if p == 0 {
                text("Input Layer")
            } else {
                Cell::Empty
            },
            text(label),
        ];
        row.extend(rbf.centers.iter().map(|c| Cell::Number(c[p])));
        row.resize(width, Cell::Empty);
        body.push(row);
    }
    let mut row = vec![Cell::Empty, text("Hidden Unit Width")];
    row.extend(rbf.widths.iter().map(|w| Cell::Number(*w)));
    row.resize(width, Cell::Empty);
    body.push(row);
    let first = usize::from(!rbf.orbf);
    for h in first..=j {
        let mut row = vec![
            if h == first {
                text("Hidden Layer")
            } else {
                Cell::Empty
            },
            text(&if h == 0 {
                "(Bias)".to_string()
            } else {
                format!("H({h})")
            }),
        ];
        row.resize(2 + j, Cell::Empty);
        row.extend(rbf.weights.iter().map(|w| Cell::Number(w[h - first])));
        body.push(row);
    }
    let mut table = plain_table("Parameter Estimates", 2, &columns, body);
    table.column_groups = groups;
    table.caption = Some(
        "Input rows give each hidden unit's center; hidden rows give the weights to the outputs."
            .into(),
    );
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn least_squares_by_hand() {
        // y = 1 + 2 x on x = 0, 1, 2, 3 with noise +-0.5: normal equations
        // [4 6; 6 14] b = [sum y; sum x y]
        let x: Vec<Vec<f64>> = (0..4).map(|i| vec![1.0, i as f64]).collect();
        let y = vec![vec![1.5], vec![2.5], vec![5.5], vec![6.5]];
        // sum y = 16, sum x y = 0 + 2.5 + 11 + 19.5 = 33; b1 = (4*33 - 6*16) / (4*14 - 36) = 1.8,
        // b0 = (16 - 6 * 1.8) / 4 = 1.3
        let b = least_squares(&x, &y);
        assert!(
            (b[0][0] - 1.3).abs() < 1e-12 && (b[0][1] - 1.8).abs() < 1e-12,
            "{b:?}"
        );
        // a repeated column gets 0
        let x2: Vec<Vec<f64>> = x.iter().map(|r| vec![r[0], r[1], r[1]]).collect();
        let b = least_squares(&x2, &y);
        assert!((b[0][1] - 1.8).abs() < 1e-12 && b[0][2] == 0.0);
    }

    #[test]
    fn rescaling_and_its_inverse() {
        let v = [2.0, 4.0, 6.0];
        let s = Linear::of(Rescale::Standardized, &v);
        assert!((s.apply(6.0) - 1.0).abs() < 1e-12 && (s.back(1.0) - 6.0).abs() < 1e-12);
        let n = Linear::of(Rescale::Normalized(0.0), &v);
        assert!((n.apply(4.0) - 0.5).abs() < 1e-12);
        let a = Linear::of(Rescale::AdjNormalized(1.0), &v);
        // the range widens to [1, 7]: x = 7 gives 1, x = 1 gives -1
        assert!((a.apply(7.0) - 1.0).abs() < 1e-12 && (a.apply(1.0) + 1.0).abs() < 1e-12);
    }

    #[test]
    fn rbf_units_from_cluster_summaries() {
        // one factor with 2 categories and one covariate; cluster of 4 cases: categories
        // (3, 1), covariate values 1, 2, 3, 4 (mean 2.5, ss 5, sd^2 = 5/3)
        let mut c = Cf::record(&[1.0], &[0], &[2]);
        for (x, k) in [(2.0, 0), (3.0, 0), (4.0, 1)] {
            c.add(&Cf::record(&[x], &[k], &[2]));
        }
        let (centers, widths) = rbf_units(&[c], &[2], 2.0);
        assert_eq!(centers[0], vec![0.75, 0.25, 2.5]);
        // h p (1 - p) twice and h sd^2: (2 * 0.1875 * 2 + 2 * 5 / 3) / 3
        let want = ((2.0 * 0.1875 * 2.0 + 2.0 * 5.0 / 3.0) / 3.0f64).sqrt();
        assert!((widths[0] - want).abs() < 1e-12);
    }

    #[test]
    fn rescaled_pseudo_probabilities_and_roc_area() {
        let r = rescaled(&[-0.3, 0.5, 1.3]);
        assert!((r[0]).abs() < 1e-12 && (r[1] - 0.8 / 2.4).abs() < 1e-12);
        // 3 positives and 2 negatives, one tie between groups
        let area = roc_area(&[
            (0.9, true),
            (0.7, true),
            (0.4, true),
            (0.4, false),
            (0.1, false),
        ]);
        assert_eq!(area, Some((4.0 + 0.5 + 1.0) / 6.0));
    }
}
