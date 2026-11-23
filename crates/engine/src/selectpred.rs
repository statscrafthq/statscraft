//! SELECTPRED: screens predictors and ranks each one alone by how it relates to the target.
//!
//! Rules, in our words, from the Command Syntax Reference and the Algorithms manual:
//! - Frequency weights are rounded to whole numbers; a case whose weight is missing or below
//!   0.5 is not used. Cases missing on the target, or on every predictor, are left out.
//!   User-missing values count as valid for categorical variables under USERMISSING=INCLUDE,
//!   never for scale ones.
//! - Predictors missing in every case or with one value in every case are left out; with
//!   screening on also those with more than PCTMISSING percent missing, categorical ones whose
//!   largest category holds more than PCTEQUAL percent of their cases or with more than
//!   PCTUNEQUAL percent of categories holding a single case, and scale ones with a standard
//!   deviation below STDDEV or, with a mean other than 0, |sd / mean| below CV.
//! - Each predictor is tested on the cases valid on it and on the target. Categorical target
//!   and predictor: Pearson X2 = sum (N_ij - E_ij)^2 / E_ij, likelihood ratio G2 = 2 sum N_ij
//!   ln(N_ij / E_ij), E_ij = N_i. N_.j / N, both with (I - 1)(J - 1) degrees of freedom;
//!   Cramer's V = root(X2 / (N (min(I, J) - 1))); lambda = (sum_i max_j N_ij - max_j N_.j) /
//!   (N - max_j N_.j). A scale predictor of a categorical target, or a categorical predictor
//!   of a scale target: the one-way F of the scale variable over the categories, with p 0 when
//!   the within-group part is 0. Two scale variables: Pearson's r, t = r root((N - 2) / (1 -
//!   r^2)), p = 2 P(T > |t|) with N - 2 degrees of freedom, 0 when r^2 = 1.
//! - Ranking: significance ascending, then the larger chi-square and the fewer degrees of
//!   freedom, the larger F and the larger N, or the larger r^2 and the larger N; Cramer's V
//!   and lambda descending, then the larger chi-square or the fewer categories; last the file
//!   order. Between categorical and scale predictors of equal significance the kind whose first
//!   predictor comes first in the file goes first. With both kinds, CRAMERSV and LAMBDA give
//!   way to the Pearson chi-square.
//! - SIZE=AUTO selects round(min(max(30, 2 root L0), L0)) of the L0 ranked predictors; when a
//!   significance ranks, only those below PCUTOFF (1 is no limit) may be selected.

use crate::chart::{Axis, Category, Chart, ChartKind, Grouping, Series, Statistic};
use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, f_p, t_p};
use crate::npar_tests::text;
use crate::pass::default_measure;
use crate::twostep::{category_codes, numeric_value, plain_table};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure};
use statscraft_syntax::{MeasurementLevel, Ranking, Selectpred};
use std::cmp::Ordering;
use std::time::Instant;

/// A case's value of a variable: a category, a number, or missing.
#[derive(Clone)]
enum Datum {
    Category(Key),
    Number(f64),
    Missing,
}

struct Predictor {
    name: String,
    categorical: bool,
    /// Position in the file.
    index: usize,
    values: Vec<Datum>,
}

/// The test of one predictor against the target.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Tested {
    /// Weighted count of the cases valid on both.
    pub(crate) n: f64,
    /// Categorical predictor of a categorical target.
    pub(crate) pearson: f64,
    pub(crate) pearson_p: f64,
    pub(crate) lr: f64,
    pub(crate) lr_p: f64,
    pub(crate) df: f64,
    pub(crate) cramers_v: f64,
    pub(crate) lambda: f64,
    /// Categories of the predictor.
    pub(crate) levels: usize,
    /// F test: F, its degrees of freedom and significance.
    pub(crate) f: f64,
    pub(crate) df2: f64,
    pub(crate) f_p: f64,
    /// Two scale variables: r and its significance.
    pub(crate) r: f64,
    pub(crate) r_p: f64,
}

/// Cross counts of two categorical variables: rows the predictor, columns the target.
pub(crate) fn cross_test(f: &[Vec<f64>]) -> Tested {
    let rows: Vec<f64> = f.iter().map(|r| r.iter().sum()).collect();
    let c = f.first().map_or(0, Vec::len);
    let columns: Vec<f64> = (0..c).map(|j| f.iter().map(|r| r[j]).sum()).collect();
    let n: f64 = rows.iter().sum();
    let (mut x2, mut g2) = (0.0, 0.0);
    for (i, row) in f.iter().enumerate() {
        for (j, &o) in row.iter().enumerate() {
            let e = rows[i] * columns[j] / n;
            x2 += (o - e).powi(2) / e;
            if o > 0.0 {
                g2 += 2.0 * o * (o / e).ln();
            }
        }
    }
    let df = (f.len().saturating_sub(1) * c.saturating_sub(1)) as f64;
    let p = |x: f64| if df > 0.0 { chi_square_p(x, df) } else { 1.0 };
    let q = f.len().min(c) as f64 - 1.0;
    let largest = columns.iter().copied().fold(0.0, f64::max);
    let best: f64 = f
        .iter()
        .map(|r| r.iter().copied().fold(0.0, f64::max))
        .sum();
    Tested {
        n,
        pearson: x2,
        pearson_p: p(x2),
        lr: g2,
        lr_p: p(g2),
        df,
        cramers_v: if q > 0.0 { (x2 / (n * q)).sqrt() } else { 0.0 },
        lambda: if n > largest {
            (best - largest) / (n - largest)
        } else {
            0.0
        },
        levels: f.len(),
        ..Tested::default()
    }
}

/// One-way F of weighted values over groups: (group, value, weight) triples.
pub(crate) fn f_test(cases: &[(usize, f64, f64)], groups: usize) -> Tested {
    let mut n = vec![0.0; groups];
    let mut sum = vec![0.0; groups];
    for &(g, x, w) in cases {
        n[g] += w;
        sum[g] += w * x;
    }
    let total: f64 = n.iter().sum();
    let grand = sum.iter().sum::<f64>() / total;
    let mean: Vec<f64> = (0..groups).map(|g| sum[g] / n[g]).collect();
    let within: f64 = cases
        .iter()
        .map(|&(g, x, w)| w * (x - mean[g]).powi(2))
        .sum();
    let between: f64 = (0..groups)
        .filter(|&g| n[g] > 0.0)
        .map(|g| n[g] * (mean[g] - grand).powi(2))
        .sum();
    let k = n.iter().filter(|&&m| m > 0.0).count() as f64;
    let (df1, df2) = (k - 1.0, total - k);
    let (f, p) = if df1 < 1.0 || df2 <= 0.0 {
        (f64::NAN, 1.0)
    } else if within > 0.0 {
        let f = (between / df1) / (within / df2);
        (f, f_p(f, df1, df2))
    } else if between > 0.0 {
        (f64::INFINITY, 0.0)
    } else {
        (f64::NAN, 1.0)
    };
    Tested {
        n: total,
        df: df1,
        df2,
        f,
        f_p: p,
        levels: k as usize,
        ..Tested::default()
    }
}

/// Pearson's r of weighted pairs (x, y, weight) and its two-sided significance.
pub(crate) fn r_test(cases: &[(f64, f64, f64)]) -> Tested {
    let n: f64 = cases.iter().map(|c| c.2).sum();
    let mx = cases.iter().map(|c| c.2 * c.0).sum::<f64>() / n;
    let my = cases.iter().map(|c| c.2 * c.1).sum::<f64>() / n;
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for &(x, y, w) in cases {
        sxx += w * (x - mx).powi(2);
        syy += w * (y - my).powi(2);
        sxy += w * (x - mx) * (y - my);
    }
    let r = sxy / (sxx * syy).sqrt();
    let p = if n <= 2.0 || !r.is_finite() {
        1.0
    } else if r * r >= 1.0 {
        0.0
    } else {
        let t = r * ((n - 2.0) / (1.0 - r * r)).sqrt();
        t_p(t, n - 2.0)
    };
    Tested {
        n,
        df: n - 2.0,
        r,
        r_p: p,
        ..Tested::default()
    }
}

/// What ranks a predictor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Criterion {
    Pearson,
    LikelihoodRatio,
    CramersV,
    Lambda,
    F,
    R,
}

impl Criterion {
    fn name(self) -> &'static str {
        match self {
            Criterion::Pearson => "Pearson Chi-Square",
            Criterion::LikelihoodRatio => "Likelihood Ratio",
            Criterion::CramersV => "Cramer's V",
            Criterion::Lambda => "Lambda",
            Criterion::F => "F",
            Criterion::R => "Pearson Correlation",
        }
    }

    fn uses_p(self) -> bool {
        !matches!(self, Criterion::CramersV | Criterion::Lambda)
    }
}

struct Row {
    predictor: usize,
    criterion: Criterion,
    test: Tested,
}

impl Row {
    /// The significance that ranks it, when one does.
    fn p(&self) -> f64 {
        let p = match self.criterion {
            Criterion::Pearson | Criterion::CramersV => self.test.pearson_p,
            Criterion::LikelihoodRatio => self.test.lr_p,
            Criterion::Lambda => f64::NAN,
            Criterion::F => self.test.f_p,
            Criterion::R => self.test.r_p,
        };
        if p.is_nan() { 1.0 } else { p }
    }

    /// (statistic, df1, df2) shown for the criterion.
    fn statistic(&self) -> (f64, Option<f64>, Option<f64>) {
        let t = &self.test;
        match self.criterion {
            Criterion::Pearson => (t.pearson, Some(t.df), None),
            Criterion::LikelihoodRatio => (t.lr, Some(t.df), None),
            Criterion::CramersV => (t.cramers_v, Some(t.df), None),
            Criterion::Lambda => (t.lambda, None, None),
            Criterion::F => (t.f, Some(t.df), Some(t.df2)),
            Criterion::R => (t.r, Some(t.df), None),
        }
    }

    /// The order within one kind of criterion after the significance or the value.
    fn ties(&self, other: &Row) -> Ordering {
        let (a, b) = (&self.test, &other.test);
        let desc = |x: f64, y: f64| y.total_cmp(&x);
        match self.criterion {
            Criterion::Pearson => desc(a.pearson, b.pearson).then(a.df.total_cmp(&b.df)),
            Criterion::LikelihoodRatio => desc(a.lr, b.lr).then(a.df.total_cmp(&b.df)),
            Criterion::CramersV => desc(a.pearson, b.pearson),
            Criterion::Lambda => a.levels.cmp(&b.levels),
            Criterion::F => desc(nan_low(a.f), nan_low(b.f)).then(desc(a.n, b.n)),
            Criterion::R => desc(nan_low(a.r * a.r), nan_low(b.r * b.r)).then(desc(a.n, b.n)),
        }
    }
}

fn nan_low(x: f64) -> f64 {
    if x.is_nan() { f64::NEG_INFINITY } else { x }
}

/// The measurement level of a variable, its data deciding when the dictionary does not.
fn measure_of(data: &Dataset, index: usize, scale_min: usize) -> Measure {
    match data.variable_at(index).measure() {
        Measure::Unknown => default_measure(data, index, scale_min),
        known => known,
    }
}

/// The automatic number of predictors to select out of `l0`.
pub(crate) fn auto_size(l0: usize) -> usize {
    let l0f = l0 as f64;
    (30f64.max(2.0 * l0f.sqrt())).min(l0f).round() as usize
}

impl Engine {
    pub(crate) fn selectpred(&mut self, command: &Selectpred) -> Result<Vec<Table>, Error> {
        let started = Instant::now();
        let data = self.active()?;
        let scale_min = self.scale_min();
        let mut warnings: Vec<String> = Vec::new();
        let target_index = data
            .variable_index(&command.dependent)
            .ok_or_else(|| Error(format!("unknown variable {}", command.dependent)))?;
        let target = data.variable_at(target_index);
        let weight_name = data.weight_variable().map(str::to_string);
        let is_weight = |n: &str| {
            weight_name
                .as_deref()
                .is_some_and(|w| w.eq_ignore_ascii_case(n))
        };
        if is_weight(target.name()) {
            return Err(Error(
                "SELECTPRED: the weight variable cannot also be the target".into(),
            ));
        }
        let target_categorical = match command.level {
            Some(MeasurementLevel::Scale) => {
                if target.is_string() {
                    return Err(Error(format!(
                        "SELECTPRED: {} is a string variable and can be nominal or ordinal only",
                        target.name()
                    )));
                }
                false
            }
            Some(_) => true,
            None => {
                target.is_string() || measure_of(data, target_index, scale_min) != Measure::Scale
            }
        };

        // the predictors: BY and WITH, or every other variable by its measurement level
        let mut lists: Vec<(String, bool)> = Vec::new();
        for (list, categorical, keyword) in [
            (&command.factors, true, "BY"),
            (&command.covariates, false, "WITH"),
        ] {
            match list {
                Some(items) if items.is_empty() => warnings.push(format!(
                    "SELECTPRED: {keyword} names no variables and is ignored"
                )),
                Some(items) => {
                    let mut seen: Vec<String> = Vec::new();
                    for name in self.expand(items)? {
                        let name = data
                            .variable_at(data.variable_index(&name).expect("expanded"))
                            .name()
                            .to_string();
                        if name.eq_ignore_ascii_case(target.name())
                            || is_weight(&name)
                            || seen.iter().any(|s| s.eq_ignore_ascii_case(&name))
                        {
                            continue;
                        }
                        if lists.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                            return Err(Error(format!(
                                "SELECTPRED: {name} is both a factor (BY) and a covariate (WITH)"
                            )));
                        }
                        seen.push(name.clone());
                        lists.push((name, categorical));
                    }
                }
                None => {}
            }
        }
        let given = command.factors.as_ref().is_some_and(|l| !l.is_empty())
            || command.covariates.as_ref().is_some_and(|l| !l.is_empty());
        if !given {
            for index in 0..data.variable_count() {
                let v = data.variable_at(index);
                if index == target_index || is_weight(v.name()) {
                    continue;
                }
                let categorical =
                    v.is_string() || measure_of(data, index, scale_min) != Measure::Scale;
                lists.push((v.name().to_string(), categorical));
            }
        }
        let except = self.expand(&command.except)?;
        lists.retain(|(n, _)| !except.iter().any(|e| e.eq_ignore_ascii_case(n)));
        if lists.is_empty() {
            return Err(Error("SELECTPRED has no predictor to screen".into()));
        }

        // the values, with user-missing values valid only for categorical variables
        let include = command.include;
        let datums = |index: usize, categorical: bool| -> Vec<Datum> {
            let v = data.variable_at(index);
            match categorical {
                true => categories(&v, include)
                    .into_iter()
                    .map(|k| k.map_or(Datum::Missing, Datum::Category))
                    .collect(),
                false => (0..data.case_count())
                    .map(|c| numeric_value(&v, c, false).map_or(Datum::Missing, Datum::Number))
                    .collect(),
            }
        };
        let mut predictors = Vec::new();
        for (name, categorical) in lists {
            let index = data.variable_index(&name).expect("listed");
            if !categorical && data.variable_at(index).is_string() {
                return Err(Error(format!(
                    "SELECTPRED: covariate {name} is a string variable; covariates must be numeric"
                )));
            }
            predictors.push(Predictor {
                name,
                categorical,
                index,
                values: datums(index, categorical),
            });
        }
        let target_values = datums(target_index, target_categorical);

        // the cases
        let cases = data.case_count();
        let weights: Vec<f64> = match case_weights(data)? {
            Some(w) => w
                .iter()
                .map(|w| if *w < 0.5 { 0.0 } else { w.round() })
                .collect(),
            None => vec![1.0; cases],
        };
        let weighted = weight_name.is_some();
        let missing = |d: &Datum| matches!(d, Datum::Missing);
        let (mut bad_weight, mut no_target, mut no_predictor) = (0usize, 0usize, 0usize);
        let mut weighted_out = [0.0; 2];
        let mut used = Vec::new();
        for case in 0..cases {
            if weights[case] == 0.0 {
                bad_weight += 1;
            } else if missing(&target_values[case]) {
                no_target += 1;
                weighted_out[0] += weights[case];
            } else if predictors.iter().all(|p| missing(&p.values[case])) {
                no_predictor += 1;
                weighted_out[1] += weights[case];
            } else {
                used.push(case);
            }
        }
        if used.is_empty() {
            return Err(Error(
                "SELECTPRED has no case with a valid target and predictor".into(),
            ));
        }
        let total: f64 = used.iter().map(|&c| weights[c]).sum();
        let target_codes: Vec<Option<usize>>;
        let target_levels: usize;
        if target_categorical {
            let keys: Vec<Option<Key>> = target_values
                .iter()
                .map(|d| match d {
                    Datum::Category(k) => Some(k.clone()),
                    _ => None,
                })
                .collect();
            let (levels, codes) = category_codes(&keys);
            target_levels = levels.len();
            target_codes = codes;
            if target_levels < 2 {
                return Err(Error(format!(
                    "SELECTPRED: the categorical target {} needs at least two categories",
                    target.name()
                )));
            }
        } else {
            target_levels = 0;
            target_codes = Vec::new();
        }

        // screening
        let pct = |x: f64| format!("{}%", round4(x));
        let mut screened: Vec<(usize, String)> = Vec::new();
        let mut kept: Vec<usize> = Vec::new();
        for (k, p) in predictors.iter().enumerate() {
            let valid: Vec<usize> = used
                .iter()
                .copied()
                .filter(|&c| !missing(&p.values[c]))
                .collect();
            let w_valid: f64 = valid.iter().map(|&c| weights[c]).sum();
            let mut reason = None;
            if valid.is_empty() {
                reason = Some("Missing in every case".to_string());
            }
            // category weights, or the weighted mean and standard deviation
            let (mut counts, mut moments) = (Vec::new(), (0.0, 0.0));
            if reason.is_none() {
                if p.categorical {
                    let keys: Vec<Option<Key>> = valid
                        .iter()
                        .map(|&c| match &p.values[c] {
                            Datum::Category(key) => Some(key.clone()),
                            _ => None,
                        })
                        .collect();
                    let (levels, codes) = category_codes(&keys);
                    counts = vec![0.0; levels.len()];
                    for (code, &c) in codes.iter().zip(&valid) {
                        counts[code.expect("valid")] += weights[c];
                    }
                    if levels.len() < 2 {
                        reason = Some("Constant".to_string());
                    }
                } else {
                    let x: Vec<(f64, f64)> = valid
                        .iter()
                        .map(|&c| match p.values[c] {
                            Datum::Number(x) => (x, weights[c]),
                            _ => unreachable!("valid"),
                        })
                        .collect();
                    let mean = x.iter().map(|(x, w)| x * w).sum::<f64>() / w_valid;
                    let ss: f64 = x.iter().map(|(x, w)| w * (x - mean).powi(2)).sum();
                    let sd = if w_valid > 1.0 {
                        (ss / (w_valid - 1.0)).sqrt()
                    } else {
                        0.0
                    };
                    moments = (mean, sd);
                    if x.iter().all(|(v, _)| *v == x[0].0) {
                        reason = Some("Constant".to_string());
                    }
                }
            }
            if reason.is_none() && command.screening {
                let missing_pct = 100.0 * (total - w_valid) / total;
                reason = if missing_pct > command.pct_missing {
                    Some(format!("Missing values over {}", pct(command.pct_missing)))
                } else if p.categorical {
                    let largest = counts.iter().copied().fold(0.0, f64::max);
                    let single = counts.iter().filter(|&&n| n == 1.0).count() as f64;
                    if 100.0 * largest / w_valid > command.pct_equal {
                        Some(format!(
                            "One category holds over {} of the cases",
                            pct(command.pct_equal)
                        ))
                    } else if 100.0 * single / counts.len() as f64 > command.pct_unequal {
                        Some(format!(
                            "Over {} of the categories hold one case",
                            pct(command.pct_unequal)
                        ))
                    } else {
                        None
                    }
                } else {
                    let (mean, sd) = moments;
                    if command.min_sd > 0.0 && sd < command.min_sd {
                        Some(format!("Standard deviation below {}", command.min_sd))
                    } else if mean != 0.0 && (sd / mean).abs() < command.min_cv {
                        Some(format!("Coefficient of variation below {}", command.min_cv))
                    } else {
                        None
                    }
                };
            }
            match reason {
                Some(r) => screened.push((k, r)),
                None => kept.push(k),
            }
        }

        // the criterion of each kind of predictor
        let any_categorical = kept.iter().any(|&k| predictors[k].categorical);
        let any_scale = kept.iter().any(|&k| !predictors[k].categorical);
        let categorical_criterion =
            match (target_categorical, command.ranking) {
                (false, _) => Criterion::F,
                (true, Ranking::LikelihoodRatio) => Criterion::LikelihoodRatio,
                (true, Ranking::PearsonChiSquare) => Criterion::Pearson,
                (true, other) if any_scale => {
                    warnings.push(format!(
                    "SELECTPRED: RANKING={} cannot rank categorical and scale predictors together; \
                     the Pearson chi-square significance ranks them",
                    if other == Ranking::CramersV { "CRAMERSV" } else { "LAMBDA" }
                ));
                    Criterion::Pearson
                }
                (true, Ranking::CramersV) => Criterion::CramersV,
                (true, Ranking::Lambda) => Criterion::Lambda,
            };
        let scale_criterion = if target_categorical {
            Criterion::F
        } else {
            Criterion::R
        };

        // the tests
        let timer = command.timer * 60.0;
        let mut rows = Vec::new();
        for &k in &kept {
            if timer > 0.0 && started.elapsed().as_secs_f64() > timer {
                return Err(Error(format!(
                    "SELECTPRED stopped after the TIMER limit of {} minutes; no results",
                    command.timer
                )));
            }
            let p = &predictors[k];
            let pairs: Vec<usize> = used
                .iter()
                .copied()
                .filter(|&c| !missing(&p.values[c]))
                .collect();
            let test = match (target_categorical, p.categorical) {
                (true, true) => {
                    let keys: Vec<Option<Key>> = pairs
                        .iter()
                        .map(|&c| match &p.values[c] {
                            Datum::Category(key) => Some(key.clone()),
                            _ => None,
                        })
                        .collect();
                    let (levels, codes) = category_codes(&keys);
                    let mut f = vec![vec![0.0; target_levels]; levels.len()];
                    for (code, &c) in codes.iter().zip(&pairs) {
                        f[code.expect("valid")][target_codes[c].expect("valid")] += weights[c];
                    }
                    // the target categories seen with this predictor
                    let seen: Vec<usize> = (0..target_levels)
                        .filter(|&j| f.iter().any(|row| row[j] > 0.0))
                        .collect();
                    let f: Vec<Vec<f64>> = f
                        .iter()
                        .map(|row| seen.iter().map(|&j| row[j]).collect())
                        .collect();
                    cross_test(&f)
                }
                (true, false) => {
                    let triples: Vec<(usize, f64, f64)> = pairs
                        .iter()
                        .map(|&c| {
                            (
                                target_codes[c].expect("valid"),
                                number_of(&p.values[c]),
                                weights[c],
                            )
                        })
                        .collect();
                    f_test(&triples, target_levels)
                }
                (false, true) => {
                    let keys: Vec<Option<Key>> = pairs
                        .iter()
                        .map(|&c| match &p.values[c] {
                            Datum::Category(key) => Some(key.clone()),
                            _ => None,
                        })
                        .collect();
                    let (levels, codes) = category_codes(&keys);
                    let triples: Vec<(usize, f64, f64)> = codes
                        .iter()
                        .zip(&pairs)
                        .map(|(code, &c)| {
                            (
                                code.expect("valid"),
                                number_of(&target_values[c]),
                                weights[c],
                            )
                        })
                        .collect();
                    f_test(&triples, levels.len())
                }
                (false, false) => {
                    let pairs: Vec<(f64, f64, f64)> = pairs
                        .iter()
                        .map(|&c| {
                            (
                                number_of(&p.values[c]),
                                number_of(&target_values[c]),
                                weights[c],
                            )
                        })
                        .collect();
                    r_test(&pairs)
                }
            };
            let criterion = match p.categorical {
                true => categorical_criterion,
                false => scale_criterion,
            };
            rows.push(Row {
                predictor: k,
                criterion,
                test,
            });
        }

        // ranking: between kinds, the kind whose first predictor comes first in the file
        let first = |categorical: bool| {
            kept.iter()
                .filter(|&&k| predictors[k].categorical == categorical)
                .map(|&k| predictors[k].index)
                .min()
                .unwrap_or(usize::MAX)
        };
        let (first_categorical, first_scale) = (first(true), first(false));
        let kind_order = |row: &Row| match predictors[row.predictor].categorical {
            true => first_categorical,
            false => first_scale,
        };
        let file = |row: &Row| predictors[row.predictor].index;
        rows.sort_by(|a, b| {
            let primary = match a.criterion {
                Criterion::CramersV => b.test.cramers_v.total_cmp(&a.test.cramers_v),
                Criterion::Lambda => b.test.lambda.total_cmp(&a.test.lambda),
                _ => a.p().total_cmp(&b.p()),
            };
            let tie = match a.criterion == b.criterion {
                true => a.ties(b),
                false => kind_order(a).cmp(&kind_order(b)),
            };
            primary.then(tie).then(file(a).cmp(&file(b)))
        });

        // selection
        let l0 = rows.len();
        let size = command.size.map_or_else(|| auto_size(l0), |s| s.min(l0));
        let limit = command.p_cutoff < 1.0;
        let mut selected = vec![false; rows.len()];
        let mut chosen = 0;
        for (r, row) in rows.iter().enumerate() {
            if chosen < size && !(limit && row.criterion.uses_p() && row.p() >= command.p_cutoff) {
                selected[r] = true;
                chosen += 1;
            }
        }
        let mut shown_rows: Vec<usize> = (0..rows.len()).filter(|&r| selected[r]).collect();
        shown_rows.extend(
            (0..rows.len())
                .filter(|&r| !selected[r])
                .take(command.show_unselected),
        );
        shown_rows.sort_unstable();

        // the extra criteria of the summary
        let mut extras: Vec<Ranking> = Vec::new();
        for &c in &command.summary_criteria {
            if target_categorical && any_categorical {
                extras.push(c);
            } else {
                warnings.push(format!(
                    "SELECTPRED: PRINT SUMMARY({}) applies to categorical predictors of a categorical target and is ignored",
                    ranking_word(c)
                ));
            }
        }
        let kind = |k: usize| {
            if predictors[k].categorical {
                "Categorical"
            } else {
                "Scale"
            }
        };

        let mut tables = Vec::new();
        if command.print_cps {
            let row = |label: &str, n: usize, w: Option<f64>| {
                let mut row = vec![text(label), Cell::Number(n as f64)];
                row.push(match weighted {
                    true => number(w),
                    false => Cell::Number(100.0 * n as f64 / cases as f64),
                });
                row
            };
            let excluded = bad_weight + no_target + no_predictor;
            let rows = vec![
                row("Included", used.len(), Some(total)),
                row("Excluded: missing target", no_target, Some(weighted_out[0])),
                row(
                    "Excluded: missing every predictor",
                    no_predictor,
                    Some(weighted_out[1]),
                ),
                row("Excluded: weight missing or below 0.5", bad_weight, None),
                row(
                    "Excluded in all",
                    excluded,
                    Some(weighted_out[0] + weighted_out[1]),
                ),
                row(
                    "Total",
                    cases,
                    Some(total + weighted_out[0] + weighted_out[1]),
                ),
            ];
            let last = if weighted { "Weighted N" } else { "Percent" };
            tables.push(plain_table(
                "Case Processing Summary",
                1,
                &["", "N", last],
                rows,
            ));
        }
        if command.print_excluded && !screened.is_empty() {
            let rows = screened
                .iter()
                .map(|(k, reason)| vec![text(&predictors[*k].name), text(kind(*k)), text(reason)])
                .collect();
            tables.push(plain_table(
                "Excluded Predictors",
                1,
                &["Predictor", "Type", "Reason"],
                rows,
            ));
        }
        if command.print_summary {
            let mut columns: Vec<String> = [
                "Rank",
                "Predictor",
                "Type",
                "N",
                "Criterion",
                "Statistic",
                "df1",
                "df2",
                "Sig.",
            ]
            .map(String::from)
            .to_vec();
            columns.extend(extras.iter().map(|&c| ranking_name(c).to_string()));
            columns.push("Selected".into());
            let body = shown_rows
                .iter()
                .map(|&r| {
                    let row = &rows[r];
                    let k = row.predictor;
                    let (statistic, df1, df2) = row.statistic();
                    let sig = match row.criterion {
                        Criterion::Lambda => None,
                        _ => Some(row.p()),
                    };
                    let mut cells = vec![
                        Cell::Number((r + 1) as f64),
                        text(&predictors[k].name),
                        text(kind(k)),
                        Cell::Number(row.test.n),
                        text(row.criterion.name()),
                        number(Some(statistic)),
                        number(df1),
                        number(df2),
                        number(sig),
                    ];
                    for &c in &extras {
                        let t = &row.test;
                        cells.push(match predictors[k].categorical {
                            false => Cell::Empty,
                            true => Cell::Number(match c {
                                Ranking::PearsonChiSquare => t.pearson,
                                Ranking::LikelihoodRatio => t.lr,
                                Ranking::CramersV => t.cramers_v,
                                Ranking::Lambda => t.lambda,
                            }),
                        });
                    }
                    cells.push(text(if selected[r] { "Yes" } else { "No" }));
                    cells
                })
                .collect();
            let mut table = plain_table("Predictor Ranking", 2, columns.as_slice(), body);
            table.caption = Some(format!(
                "{chosen} of {l0} predictors selected{}",
                match limit {
                    true => format!(
                        ", those ranked by significance only below {}",
                        command.p_cutoff
                    ),
                    false => String::new(),
                }
            ));
            tables.push(table);
        }
        if command.print_selected {
            let names = |categorical: bool| -> String {
                (0..rows.len())
                    .filter(|&r| {
                        selected[r] && predictors[rows[r].predictor].categorical == categorical
                    })
                    .map(|r| predictors[rows[r].predictor].name.clone())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let body = vec![
                vec![text("Categorical"), text(names(true))],
                vec![text("Scale"), text(names(false))],
            ];
            tables.push(plain_table(
                "Selected Predictors",
                1,
                &["Type", "Predictors"],
                body,
            ));
        }
        if command.plot && !shown_rows.is_empty() {
            let value = |r: usize| {
                let row = &rows[r];
                match row.criterion {
                    Criterion::CramersV => row.test.cramers_v,
                    Criterion::Lambda => row.test.lambda,
                    _ => 1.0 - row.p(),
                }
            };
            let pick = |want: bool| -> Vec<f64> {
                shown_rows
                    .iter()
                    .map(|&r| {
                        if selected[r] == want {
                            value(r)
                        } else {
                            f64::NAN
                        }
                    })
                    .collect()
            };
            let by_value = matches!(
                categorical_criterion,
                Criterion::CramersV | Criterion::Lambda
            );
            let y = match by_value {
                true => categorical_criterion.name().to_string(),
                false => "1 - Sig.".to_string(),
            };
            let categories: Vec<Category> = shown_rows
                .iter()
                .map(|&r| {
                    let name = &predictors[rows[r].predictor].name;
                    Category {
                        label: name.clone(),
                        value: text(name),
                        missing: false,
                        bin: None,
                    }
                })
                .collect();
            let series = |name: &str, values| Series {
                name: name.into(),
                statistic: Statistic::Value,
                variable: None,
                value: Cell::Empty,
                values,
            };
            let mut chart = Chart::lines(
                format!("Predictors of {} by rank", target.name()),
                Axis::titled("Predictor"),
                Axis::titled(y),
                categories,
                vec![
                    series("Selected", pick(true)),
                    series("Not selected", pick(false)),
                ],
            );
            chart.kind = ChartKind::Bar;
            chart.grouping = Some(Grouping::Clustered);
            chart.legend = Some("Status".into());
            self.charts.push(chart);
        }
        for warning in warnings {
            self.warn(warning);
        }
        Ok(tables)
    }
}

fn number_of(d: &Datum) -> f64 {
    match d {
        Datum::Number(x) => *x,
        _ => f64::NAN,
    }
}

fn ranking_word(r: Ranking) -> &'static str {
    match r {
        Ranking::PearsonChiSquare => "PEARSONCHISQ",
        Ranking::LikelihoodRatio => "LRCHISQ",
        Ranking::CramersV => "CRAMERSV",
        Ranking::Lambda => "LAMBDA",
    }
}

fn ranking_name(r: Ranking) -> &'static str {
    match r {
        Ranking::PearsonChiSquare => "Pearson Chi-Square",
        Ranking::LikelihoodRatio => "Likelihood Ratio",
        Ranking::CramersV => "Cramer's V",
        Ranking::Lambda => "Lambda",
    }
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_size_is_the_rounded_bounded_double_root() {
        // L0 values picked so that 2 root(L0) is whole or clearly rounds one way
        for (l0, l) in [
            (12, 12),
            (64, 30),
            (400, 40),
            (900, 60),
            (3600, 120),
            (1001, 63),
        ] {
            assert_eq!(auto_size(l0), l, "{l0}");
        }
    }
}
