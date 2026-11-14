//! MVA: missing value analysis. Describes where values are missing and how that relates to
//! the other variables, and estimates means and covariances in ways that allow for it.
//!
//! Rules from the Command Syntax Reference (MVA) and the Algorithms manual (MVA Algorithms),
//! in our words:
//! - String variables and the ones named on CATEGORICAL are categorical; long strings count
//!   by their first 8 bytes. A categorical variable with more distinct valid values than
//!   MAXCAT stops the command with a warning. The rest are quantitative. A value is missing
//!   when it is system-missing or user-missing; user-missing values are told apart by the
//!   position of their definition (A, B, C; a range is A and its extra value B).
//! - Univariate: per variable the valid N and the missing count and percent; for
//!   quantitative ones also the mean, the standard deviation (n - 1) and the number of
//!   extremes. With v variables and n cases, while v n log10(n) <= 150000 an extreme lies more
//!   than 1.5 interquartile ranges below the first or above the third quartile (quartiles at
//!   (n + 1) p); beyond that, more than 2 standard deviations from the mean.
//! - TTEST: for each variable j with at least PERCENT percent missing and each quantitative k,
//!   the cases valid on k split by whether j is missing; a separate-variance t test compares
//!   the two means, with Welch's degrees of freedom and a two-sided p value.
//! - CROSSTAB: per categorical variable, a column per category (and the total) with, for each
//!   other variable with at least PERCENT percent missing, the count and percent of valid
//!   values and the percent of each kind of missing value.
//! - MISMATCH: for variables with at least PERCENT percent missing, the percent of cases in
//!   which exactly one of a pair is missing; the diagonal is the percent missing.
//! - DPATTERN lists every case, MPATTERN the cases with a missing value, TPATTERN each
//!   distinct pattern with its count (patterns below PERCENT percent of the cases left out).
//!   Symbols: S system-missing, A B C user-missing, + and - extreme values; X marks a missing
//!   value in TPATTERN. Unless NOSORT the variables are ordered by their number of missing
//!   values, fewest first, and the rows by their pattern read from the variable with the most
//!   missing values, so that alike patterns stand together.
//! - LISTWISE: means, covariances (n - 1) and correlations of the quantitative variables over
//!   the cases valid on every analysis variable. PAIRWISE: for each pair the count, the means
//!   and standard deviations over the cases valid on both, the covariance and correlation.
//! - EM starts from the univariate means and the pairwise covariances. Each iteration fills
//!   every missing value with its regression on the variables the case has (swept with the
//!   TOLERANCE check), weighs each case (1 for the normal, (df + p) / (df + D^2) for Student's
//!   t, the ratio of mixture densities for the contaminated normal, p the observed variables,
//!   D^2 their Mahalanobis distance), takes the weighted means, and divides the weighted
//!   cross products plus the conditional covariances of the filled values by
//!   (n - 1) sum(w) / n. It stops when every variance changes by at most CONVERGENCE relative
//!   to its new value, or after ITERATIONS. Little's MCAR test sums, over the patterns, the
//!   pattern's case count times the Mahalanobis distance of its observed means from the EM
//!   means; its df is the summed number of observed variables minus the number of variables.
//! - REGRESSION fills each missing value from the univariate means and the pairwise
//!   covariances: forward stepwise over the predictors the case has, entering the one that
//!   lowers the residual variance most while its tolerance holds, its F to enter reaches
//!   FLIMIT and at most NPREDICTORS are in; then adds the residual of a random complete case,
//!   a normal or t draw scaled by the residual standard deviation, or nothing. The means and
//!   covariances (n - 1) of the filled data are the estimates.
//! - OUTFILE saves the active data with the missing values of the estimated variables filled.

use crate::crosstabs::Key;
use crate::dist::{chi_square_p, t_p};
use crate::examine::Distribution;
use crate::matrix::{Matrix, sweep};
use crate::random::draw;
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, MissingValues, Values, Variable};
use statscraft_syntax::{AddType, Mva, MvaEstimate, MvaPattern, PercentileMethod};
use std::collections::BTreeMap;

/// Whether a value is valid, system-missing or user-missing of the given type (0 is A).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Valid,
    System,
    User(usize),
}

/// One analysis variable with its values read once.
struct Column {
    name: String,
    title: String,
    categorical: bool,
    /// The value where valid and numeric, else NaN.
    values: Vec<f64>,
    /// The category of each valid case of a categorical variable.
    keys: Vec<Option<Key>>,
    states: Vec<State>,
    /// The user-missing definitions, A first, as text.
    user_types: Vec<String>,
    /// -1 extremely low, 1 extremely high, 0 otherwise.
    extremes: Vec<i8>,
}

impl Column {
    fn missing(&self, case: usize) -> bool {
        self.states[case] != State::Valid
    }

    fn missing_count(&self) -> usize {
        self.states.iter().filter(|s| **s != State::Valid).count()
    }

    fn valid(&self) -> Vec<f64> {
        self.values
            .iter()
            .copied()
            .filter(|v| !v.is_nan())
            .collect()
    }
}

fn user_type(missing: &MissingValues, value: f64) -> usize {
    match missing {
        MissingValues::Numbers(list) => list.iter().position(|&m| m == value).unwrap_or(0),
        MissingValues::Range { low, high, .. } => usize::from(!(*low <= value && value <= *high)),
        _ => 0,
    }
}

fn type_names(missing: &MissingValues) -> Vec<String> {
    match missing {
        MissingValues::None => Vec::new(),
        MissingValues::Numbers(list) => list.iter().map(|v| format!("{v}")).collect(),
        MissingValues::Range { low, high, extra } => {
            let mut names = vec![format!("{low} thru {high}")];
            names.extend(extra.map(|v| format!("{v}")));
            names
        }
        MissingValues::Strings(list) => list.clone(),
    }
}

/// The first 8 bytes of a string value without trailing blanks, on a character boundary.
fn category_text(value: &str) -> String {
    let value = value.trim_end();
    let mut end = value.len().min(8);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

fn read_column(variable: &Variable, categorical: bool, shown: &Shown) -> Column {
    let missing = variable.missing_values();
    let (mut values, mut keys, mut states) = (Vec::new(), Vec::new(), Vec::new());
    match variable.values() {
        Values::Numeric(column) => {
            for value in column.iter() {
                let state = match value {
                    None => State::System,
                    Some(v) if variable.is_user_missing(v) => State::User(user_type(missing, v)),
                    Some(_) => State::Valid,
                };
                let valid = value.filter(|_| state == State::Valid);
                values.push(valid.unwrap_or(f64::NAN));
                keys.push(valid.filter(|_| categorical).map(Key::Number));
                states.push(state);
            }
        }
        Values::String(column) => {
            let list = match missing {
                MissingValues::Strings(list) => list.as_slice(),
                _ => &[],
            };
            for value in column.iter() {
                let state = match variable.is_user_missing_str(value) {
                    true => {
                        State::User(list.iter().position(|m| m == value.trim_end()).unwrap_or(0))
                    }
                    false => State::Valid,
                };
                values.push(f64::NAN);
                keys.push((state == State::Valid).then(|| Key::Text(category_text(value))));
                states.push(state);
            }
        }
    }
    Column {
        name: variable.name().to_string(),
        title: shown.variable(variable),
        categorical: categorical || variable.is_string(),
        extremes: vec![0; values.len()],
        values,
        keys,
        states,
        user_types: type_names(missing),
    }
}

fn mean_sd(values: &[f64]) -> (f64, f64) {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let ss: f64 = values.iter().map(|v| (v - mean).powi(2)).sum();
    (mean, (ss / (n - 1.0)).sqrt())
}

/// The limits below and above which a value is extreme.
fn extreme_limits(values: &[f64], quartiles: bool) -> Option<(f64, f64)> {
    if values.is_empty() {
        return None;
    }
    if quartiles {
        let pairs: Vec<(f64, f64)> = values.iter().map(|&v| (v, 1.0)).collect();
        let dist = Distribution::new(&pairs);
        let q1 = dist.percentile(25.0, PercentileMethod::Haverage)?;
        let q3 = dist.percentile(75.0, PercentileMethod::Haverage)?;
        Some((q1 - 1.5 * (q3 - q1), q3 + 1.5 * (q3 - q1)))
    } else {
        let (mean, sd) = mean_sd(values);
        (values.len() > 1).then_some((mean - 2.0 * sd, mean + 2.0 * sd))
    }
}

fn text(s: impl Into<String>) -> Cell {
    Cell::Text(s.into())
}

fn num(x: f64) -> Cell {
    match x.is_finite() {
        true => Cell::Number(x),
        false => Cell::Empty,
    }
}

fn count(x: usize) -> Cell {
    Cell::Number(x as f64)
}

fn percent(part: usize, whole: usize) -> f64 {
    match whole {
        0 => f64::NAN,
        _ => 100.0 * part as f64 / whole as f64,
    }
}

fn table(title: &str, row_headers: usize, columns: Vec<String>, rows: Vec<Vec<Cell>>) -> Table {
    Table {
        title: title.into(),
        row_headers,
        columns,
        rows,
        ..Table::default()
    }
}

/// A square table of a matrix over `titles`.
fn matrix_table(title: &str, titles: &[String], m: &Matrix) -> Table {
    let mut columns = vec![String::new()];
    columns.extend(titles.iter().cloned());
    let rows = titles
        .iter()
        .zip(m)
        .map(|(t, row)| {
            let mut cells = vec![text(t.clone())];
            cells.extend(row.iter().map(|&v| num(v)));
            cells
        })
        .collect();
    table(title, 1, columns, rows)
}

/// A one-row table of `values` per variable.
fn row_table(title: &str, label: &str, titles: &[String], values: &[f64]) -> Table {
    let mut columns = vec![String::new()];
    columns.extend(titles.iter().cloned());
    let mut row = vec![text(label)];
    row.extend(values.iter().map(|&v| num(v)));
    table(title, 1, columns, vec![row])
}

fn correlations(cov: &Matrix) -> Matrix {
    let v = cov.len();
    (0..v)
        .map(|j| {
            (0..v)
                .map(|k| cov[j][k] / (cov[j][j] * cov[k][k]).sqrt())
                .collect()
        })
        .collect()
}

/// Means and covariances (n - 1) of complete rows.
fn moments(rows: &[Vec<f64>]) -> (Vec<f64>, Matrix) {
    let v = rows.first().map_or(0, Vec::len);
    let n = rows.len() as f64;
    let mean: Vec<f64> = (0..v)
        .map(|j| rows.iter().map(|r| r[j]).sum::<f64>() / n)
        .collect();
    let cov = (0..v)
        .map(|j| {
            (0..v)
                .map(|k| {
                    rows.iter()
                        .map(|r| (r[j] - mean[j]) * (r[k] - mean[k]))
                        .sum::<f64>()
                        / (n - 1.0)
                })
                .collect()
        })
        .collect();
    (mean, cov)
}

/// Pairwise statistics of the columns of `x` (NaN missing): counts, the mean and standard
/// deviation of column k over the cases valid on j and k (`mean[j][k]`), covariances.
struct Pairwise {
    n: Vec<Vec<usize>>,
    mean: Matrix,
    sd: Matrix,
    cov: Matrix,
}

fn pairwise(x: &[Vec<f64>], v: usize) -> Pairwise {
    let mut p = Pairwise {
        n: vec![vec![0; v]; v],
        mean: vec![vec![f64::NAN; v]; v],
        sd: vec![vec![f64::NAN; v]; v],
        cov: vec![vec![f64::NAN; v]; v],
    };
    for j in 0..v {
        for k in 0..v {
            let pairs: Vec<(f64, f64)> = x
                .iter()
                .filter(|r| !r[j].is_nan() && !r[k].is_nan())
                .map(|r| (r[j], r[k]))
                .collect();
            let n = pairs.len();
            p.n[j][k] = n;
            if n == 0 {
                continue;
            }
            let ks: Vec<f64> = pairs.iter().map(|p| p.1).collect();
            let (mk, sk) = mean_sd(&ks);
            let mj = pairs.iter().map(|p| p.0).sum::<f64>() / n as f64;
            p.mean[j][k] = mk;
            p.sd[j][k] = sk;
            if n > 1 {
                p.cov[j][k] =
                    pairs.iter().map(|(a, b)| (a - mj) * (b - mk)).sum::<f64>() / (n - 1) as f64;
            }
        }
    }
    p
}

/// Sweeps `m` on each of `pivots` whose residual variance keeps at least `tolerance` of its
/// variance in `base`; returns the pivots swept.
fn sweep_on(m: &mut Matrix, base: &Matrix, pivots: &[usize], tolerance: f64) -> Vec<usize> {
    let mut swept = Vec::new();
    for &o in pivots {
        if base[o][o] > 0.0 && m[o][o] > 0.0 && m[o][o] / base[o][o] >= tolerance {
            sweep(m, o);
            swept.push(o);
        }
    }
    swept
}

/// The rows of `x` grouped by their pattern of missing values (true missing), in order of
/// their first case.
fn groups(x: &[Vec<f64>]) -> Vec<(Vec<bool>, Vec<usize>)> {
    let mut out: Vec<(Vec<bool>, Vec<usize>)> = Vec::new();
    let mut index: BTreeMap<Vec<bool>, usize> = BTreeMap::new();
    for (c, row) in x.iter().enumerate() {
        let pattern: Vec<bool> = row.iter().map(|v| v.is_nan()).collect();
        match index.get(&pattern) {
            Some(&i) => out[i].1.push(c),
            None => {
                index.insert(pattern.clone(), out.len());
                out.push((pattern, vec![c]));
            }
        }
    }
    out
}

/// The distribution EM assumes.
#[derive(Clone, Copy)]
enum Family {
    Normal,
    T(f64),
    /// Contamination proportion and ratio of standard deviations.
    Mixed(f64, f64),
}

impl Family {
    fn weight(self, d2: f64, p: usize) -> f64 {
        let p = p as f64;
        match self {
            Family::Normal => 1.0,
            Family::T(df) => (df + p) / (df + d2),
            Family::Mixed(alpha, lambda) => {
                let e = alpha * lambda.powf(p / 2.0) * ((1.0 - lambda) * d2 / 2.0).exp();
                match e.is_finite() {
                    true => (1.0 - alpha + e * lambda) / (1.0 - alpha + e),
                    false => lambda,
                }
            }
        }
    }
}

struct EmResult {
    mean: Vec<f64>,
    cov: Matrix,
    filled: Vec<Vec<f64>>,
    iterations: usize,
    converged: bool,
}

fn em(
    x: &[Vec<f64>],
    start: (Vec<f64>, Matrix),
    family: Family,
    options: &MvaEstimate,
) -> EmResult {
    let (n, v) = (x.len(), start.0.len());
    let groups = groups(x);
    let (mut mean, mut cov) = start;
    let mut filled = x.to_vec();
    let (mut iterations, mut converged) = (0, false);
    while iterations < options.iterations && !converged {
        iterations += 1;
        let mut weights = vec![1.0; n];
        let mut extra = vec![vec![0.0; v]; v];
        for (pattern, cases) in &groups {
            let observed: Vec<usize> = (0..v).filter(|&j| !pattern[j]).collect();
            let missing: Vec<usize> = (0..v).filter(|&j| pattern[j]).collect();
            let mut m = cov.clone();
            let swept = sweep_on(&mut m, &cov, &observed, options.tolerance);
            for &c in cases {
                let r: Vec<f64> = swept.iter().map(|&o| x[c][o] - mean[o]).collect();
                let d2: f64 = (0..swept.len())
                    .map(|a| {
                        (0..swept.len())
                            .map(|b| r[a] * m[swept[a]][swept[b]] * r[b])
                            .sum::<f64>()
                    })
                    .sum();
                weights[c] = family.weight(d2, swept.len());
                for &j in &missing {
                    filled[c][j] =
                        mean[j] + swept.iter().zip(&r).map(|(&o, r)| m[j][o] * r).sum::<f64>();
                }
            }
            for &j in &missing {
                for &k in &missing {
                    extra[j][k] += cases.len() as f64 * m[j][k];
                }
            }
        }
        let total: f64 = weights.iter().sum();
        let new_mean: Vec<f64> = (0..v)
            .map(|j| (0..n).map(|c| weights[c] * filled[c][j]).sum::<f64>() / total)
            .collect();
        let divisor = (n as f64 - 1.0) * total / n as f64;
        let new_cov: Matrix = (0..v)
            .map(|j| {
                (0..v)
                    .map(|k| {
                        let cross: f64 = (0..n)
                            .map(|c| {
                                weights[c]
                                    * (filled[c][j] - new_mean[j])
                                    * (filled[c][k] - new_mean[k])
                            })
                            .sum();
                        (cross + extra[j][k]) / divisor
                    })
                    .collect()
            })
            .collect();
        converged = (0..v).all(|j| {
            let change = (new_cov[j][j] - cov[j][j]).abs();
            change == 0.0 || change / new_cov[j][j] <= options.convergence
        });
        mean = new_mean;
        cov = new_cov;
    }
    EmResult {
        mean,
        cov,
        filled,
        iterations,
        converged,
    }
}

/// Little's MCAR chi-square and its degrees of freedom.
fn little(x: &[Vec<f64>], mean: &[f64], cov: &Matrix, tolerance: f64) -> (f64, f64) {
    let v = mean.len();
    let (mut chi, mut df) = (0.0, -(v as f64));
    for (pattern, cases) in groups(x) {
        let observed: Vec<usize> = (0..v).filter(|&j| !pattern[j]).collect();
        if observed.is_empty() {
            continue;
        }
        let mut m = cov.clone();
        let swept = sweep_on(&mut m, cov, &observed, tolerance);
        let d: Vec<f64> = swept
            .iter()
            .map(|&o| cases.iter().map(|&c| x[c][o]).sum::<f64>() / cases.len() as f64 - mean[o])
            .collect();
        let md: f64 = (0..swept.len())
            .map(|a| {
                (0..swept.len())
                    .map(|b| d[a] * m[swept[a]][swept[b]] * d[b])
                    .sum::<f64>()
            })
            .sum();
        chi += cases.len() as f64 * md;
        df += observed.len() as f64;
    }
    (chi, df)
}

/// A regression of one variable on chosen predictors: (predictor, coefficient), the residual
/// variance.
struct Fit {
    coefficients: Vec<(usize, f64)>,
    variance: f64,
}

/// Forward stepwise regression of `j` on `candidates` from the pairwise covariances.
fn stepwise(
    j: usize,
    candidates: &[usize],
    cov: &Matrix,
    valid: usize,
    options: &MvaEstimate,
) -> Fit {
    let mut m = cov.clone();
    let mut chosen: Vec<usize> = Vec::new();
    let limit = options.npredictors.unwrap_or(usize::MAX);
    while chosen.len() < limit {
        let mut best: Option<(usize, f64)> = None;
        for &l in candidates.iter().filter(|l| !chosen.contains(l)) {
            if !(cov[l][l] > 0.0 && m[l][l] > 0.0 && m[l][l] / cov[l][l] >= options.tolerance) {
                continue;
            }
            let reduction = m[j][l] * m[j][l] / m[l][l];
            if best.is_none_or(|(_, r)| reduction > r) {
                best = Some((l, reduction));
            }
        }
        let Some((l, reduction)) = best else { break };
        let df = valid as f64 - chosen.len() as f64 - 2.0;
        if df <= 0.0 {
            break;
        }
        let f = reduction / ((m[j][j] - reduction) / df);
        if f < options.flimit {
            break;
        }
        sweep(&mut m, l);
        chosen.push(l);
    }
    Fit {
        coefficients: chosen.iter().map(|&l| (l, m[j][l])).collect(),
        variance: m[j][j],
    }
}

/// The order of the columns of the pattern tables: fewest missing values first.
fn sorted_columns(columns: &[Column], nosort: bool) -> Vec<usize> {
    let mut order: Vec<usize> = (0..columns.len()).collect();
    if !nosort {
        order.sort_by_key(|&j| columns[j].missing_count());
    }
    order
}

/// The sort key of a pattern: read from the last column (most missing) to the first.
fn pattern_key(columns: &[Column], order: &[usize], case: usize) -> Vec<bool> {
    order
        .iter()
        .rev()
        .map(|&j| columns[j].missing(case))
        .collect()
}

fn symbol(column: &Column, case: usize) -> Cell {
    match column.states[case] {
        State::System => text("S"),
        State::User(t) => text(["A", "B", "C"][t.min(2)]),
        State::Valid => match column.extremes[case] {
            1 => text("+"),
            -1 => text("-"),
            _ => Cell::Empty,
        },
    }
}

/// A value of a DESCRIBE variable as it stands in the data.
fn plain_value(variable: &Variable, case: usize) -> Cell {
    match variable.values() {
        Values::Numeric(v) => v.get(case).map_or(Cell::Empty, Cell::Number),
        Values::String(v) => text(v.get(case).trim_end()),
    }
}

/// Variables of an estimation list that MVA can use: quantitative ones on the variable list.
fn estimation_list(names: &[String], columns: &[Column], what: &str) -> Result<Vec<usize>, Error> {
    let mut out = Vec::new();
    for name in names {
        let Some(j) = columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(name))
        else {
            return Err(Error(format!(
                "{what} names {name}, which is not on the MVA variable list"
            )));
        };
        if !columns[j].categorical && !out.contains(&j) {
            out.push(j);
        }
    }
    Ok(out)
}

/// What an estimation (EM or REGRESSION) produced.
struct Estimate {
    /// The model's variables, as indices into the columns.
    model: Vec<usize>,
    /// The estimated (predicted) variables, as positions in `model`.
    predicted: Vec<usize>,
    mean: Vec<f64>,
    cov: Matrix,
    filled: Vec<Vec<f64>>,
}

impl Estimate {
    fn titles(&self, columns: &[Column]) -> Vec<String> {
        self.predicted
            .iter()
            .map(|&p| columns[self.model[p]].title.clone())
            .collect()
    }

    fn means(&self) -> Vec<f64> {
        self.predicted.iter().map(|&p| self.mean[p]).collect()
    }

    fn cov(&self) -> Matrix {
        self.predicted
            .iter()
            .map(|&a| self.predicted.iter().map(|&b| self.cov[a][b]).collect())
            .collect()
    }

    /// The mean and SD of each quantitative column, NaN where not estimated.
    fn per_column(&self, quantitative: &[usize]) -> (Vec<f64>, Vec<f64>) {
        quantitative
            .iter()
            .map(|q| {
                self.predicted
                    .iter()
                    .find(|&&p| self.model[p] == *q)
                    .map_or((f64::NAN, f64::NAN), |&p| {
                        (self.mean[p], self.cov[p][p].sqrt())
                    })
            })
            .unzip()
    }
}

impl Engine {
    pub(crate) fn mva(&mut self, command: &Mva) -> Result<Vec<Table>, Error> {
        let names = self.expand(&command.variables)?;
        let categorical = self.expand(&command.categorical)?;
        for name in &categorical {
            if !names.contains(name) {
                return Err(Error(format!(
                    "CATEGORICAL names {name}, which is not on the MVA variable list"
                )));
            }
        }
        let outfile = command
            .em
            .iter()
            .chain(&command.regression)
            .any(|e| e.outfile.is_some());
        if outfile && self.split_group.is_some() {
            return Err(Error(
                "OUTFILE of MVA with SPLIT FILE is not supported yet; turn SPLIT FILE off".into(),
            ));
        }
        let data = self.active()?;
        let shown = self.shown();
        let n = data.case_count();
        let mut columns = Vec::new();
        for name in &names {
            let variable = data.variable(name)?;
            columns.push(read_column(&variable, categorical.contains(name), &shown));
        }
        let mut warnings = Vec::new();
        for column in columns.iter().filter(|c| c.categorical) {
            let mut keys: Vec<&Key> = column.keys.iter().flatten().collect();
            keys.sort_by(|a, b| a.compare(b));
            keys.dedup_by(|a, b| a.compare(b).is_eq());
            if keys.len() > command.maxcat {
                self.warn(format!(
                    "MVA did not run: {} has {} categories, more than MAXCAT={}",
                    column.name,
                    keys.len(),
                    command.maxcat
                ));
                return Ok(Vec::new());
            }
        }
        let v = columns.len();
        let quartiles = v as f64 * n as f64 * (n as f64).log10() <= 150_000.0;
        for column in columns.iter_mut().filter(|c| !c.categorical) {
            if let Some((low, high)) = extreme_limits(&column.valid(), quartiles) {
                for (e, &x) in column.extremes.iter_mut().zip(&column.values) {
                    *e = if x < low {
                        -1
                    } else if x > high {
                        1
                    } else {
                        0
                    };
                }
            }
        }
        let quantitative: Vec<usize> = (0..v).filter(|&j| !columns[j].categorical).collect();
        let case_label = |c: usize| -> Cell {
            match &command.id {
                Some(id) => match data.variable(id) {
                    Ok(var) => match var.values() {
                        Values::Numeric(values) => values
                            .get(c)
                            .map_or(Cell::Empty, |x| text(shown.number(&var, x))),
                        Values::String(values) => text(values.get(c).trim_end()),
                    },
                    Err(_) => Cell::Empty,
                },
                None => count(self.case_numbers.as_ref().map_or(c, |n| n[c]) + 1),
            }
        };
        if let Some(id) = &command.id {
            data.variable(id)?;
        }
        let mut tables = Vec::new();

        if command.univariate {
            tables.push(univariate(&columns, n, quartiles));
        }
        if let Some(options) = &command.ttest {
            match ttests(&columns, &quantitative, n, options) {
                Some(t) => tables.push(t),
                None => warnings.push(format!(
                    "TTEST: no variable has {}% or more missing values",
                    options.percent
                )),
            }
        }
        if let Some(limit) = command.crosstab {
            for c in (0..v).filter(|&c| columns[c].categorical) {
                tables.push(crosstab(&columns, c, n, limit, &shown, data)?);
            }
        }
        if let Some((limit, nosort)) = command.mismatch {
            tables.push(mismatch(&columns, n, limit, nosort));
        }
        let patterns = [
            (&command.dpattern, 'D'),
            (&command.mpattern, 'M'),
            (&command.tpattern, 'T'),
        ];
        for (pattern, kind) in patterns {
            if let Some(options) = pattern {
                let describe = self.expand(&options.describe)?;
                let table = match kind {
                    'T' => tabulated(&columns, n, options, &describe, &categorical, data, &shown)?,
                    _ => {
                        let sort: Vec<(Variable, bool)> = options
                            .sort
                            .iter()
                            .map(|(name, desc)| data.variable(name).map(|v| (v, *desc)))
                            .collect::<Result<_, _>>()?;
                        case_patterns(
                            &columns,
                            n,
                            kind == 'M',
                            options,
                            &sort,
                            &describe,
                            data,
                            &shown,
                            &case_label,
                        )?
                    }
                };
                tables.push(table);
            }
        }

        let titles: Vec<String> = quantitative
            .iter()
            .map(|&j| columns[j].title.clone())
            .collect();
        let x: Vec<Vec<f64>> = (0..n)
            .map(|c| quantitative.iter().map(|&j| columns[j].values[c]).collect())
            .collect();
        let univariate_mean: Vec<f64> = quantitative
            .iter()
            .map(|&j| mean_sd(&columns[j].valid()).0)
            .collect();
        let univariate_sd: Vec<f64> = quantitative
            .iter()
            .map(|&j| mean_sd(&columns[j].valid()).1)
            .collect();
        let pair = pairwise(&x, quantitative.len());
        let mut summary: Vec<(&str, Vec<f64>, Vec<f64>)> =
            vec![("All Values", univariate_mean.clone(), univariate_sd)];

        if command.listwise {
            let complete: Vec<Vec<f64>> = (0..n)
                .filter(|&c| columns.iter().all(|col| !col.missing(c)))
                .map(|c| x[c].clone())
                .collect();
            let (mean, cov) = match complete.len() {
                0 => (
                    vec![f64::NAN; titles.len()],
                    vec![vec![f64::NAN; titles.len()]; titles.len()],
                ),
                _ => moments(&complete),
            };
            let mut means = row_table("Listwise Means", "Listwise", &titles, &mean);
            means.caption = Some(format!(
                "Over the {} cases with no missing value in any analysis variable.",
                complete.len()
            ));
            tables.push(means);
            tables.push(matrix_table("Listwise Covariances", &titles, &cov));
            tables.push(matrix_table(
                "Listwise Correlations",
                &titles,
                &correlations(&cov),
            ));
            summary.push((
                "Listwise",
                mean,
                cov.iter().enumerate().map(|(j, r)| r[j].sqrt()).collect(),
            ));
        }
        if command.pairwise {
            let counts: Matrix = pair
                .n
                .iter()
                .map(|r| r.iter().map(|&c| c as f64).collect())
                .collect();
            tables.push(matrix_table("Pairwise Frequencies", &titles, &counts));
            let mut means = matrix_table("Pairwise Means", &titles, &pair.mean);
            means.caption = Some(
                "Each cell is the mean of the column variable over the cases valid on both \
                 variables."
                    .into(),
            );
            tables.push(means);
            let mut sds = matrix_table("Pairwise Standard Deviations", &titles, &pair.sd);
            sds.caption = Some(
                "Each cell is the standard deviation of the column variable over the cases \
                 valid on both variables."
                    .into(),
            );
            tables.push(sds);
            tables.push(matrix_table("Pairwise Covariances", &titles, &pair.cov));
            let cor: Matrix = (0..titles.len())
                .map(|j| {
                    (0..titles.len())
                        .map(|k| pair.cov[j][k] / (pair.sd[j][k] * pair.sd[k][j]))
                        .collect()
                })
                .collect();
            tables.push(matrix_table("Pairwise Correlations", &titles, &cor));
        }

        let mut saved: Vec<(String, Estimate)> = Vec::new();
        for (options, is_em) in [(&command.em, true), (&command.regression, false)] {
            let Some(options) = options else { continue };
            let what = if is_em { "EM" } else { "REGRESSION" };
            let predicted_names = self.expand(&options.predicted)?;
            let predictor_names = self.expand(&options.predictors)?;
            let mut predicted = estimation_list(&predicted_names, &columns, what)?;
            let mut predictors = estimation_list(&predictor_names, &columns, what)?;
            if options.predicted.is_empty() {
                predicted.clone_from(&quantitative);
            }
            if options.predictors.is_empty() {
                predictors.clone_from(&quantitative);
            }
            // the model holds both lists in the order of the variable list
            let model: Vec<usize> = quantitative
                .iter()
                .copied()
                .filter(|j| predicted.contains(j) || predictors.contains(j))
                .collect();
            if predicted.is_empty() {
                return Err(Error(format!(
                    "{what} of MVA needs a quantitative variable"
                )));
            }
            let at = |j: usize| {
                quantitative
                    .iter()
                    .position(|&q| q == j)
                    .expect("quantitative")
            };
            let mx: Vec<Vec<f64>> = x
                .iter()
                .map(|r| model.iter().map(|&j| r[at(j)]).collect())
                .collect();
            let start_mean: Vec<f64> = model.iter().map(|&j| univariate_mean[at(j)]).collect();
            let start_cov: Matrix = model
                .iter()
                .map(|&a| {
                    model
                        .iter()
                        .map(|&b| {
                            let c = pair.cov[at(a)][at(b)];
                            if c.is_finite() { c } else { 0.0 }
                        })
                        .collect()
                })
                .collect();
            let predicted_at: Vec<usize> = (0..model.len())
                .filter(|&p| predicted.contains(&model[p]))
                .collect();
            let estimate = if is_em {
                let family = match (options.tdf, options.lambda, options.proportion) {
                    (Some(df), _, _) => Family::T(df),
                    (None, Some(lambda), Some(alpha)) => Family::Mixed(alpha, lambda),
                    _ => Family::Normal,
                };
                let result = em(&mx, (start_mean, start_cov), family, options);
                if !result.converged {
                    warnings.push(format!(
                        "EM did not converge in {} iterations; the estimates are those of the \
                         last iteration",
                        result.iterations
                    ));
                }
                let (chi, df) = little(&mx, &result.mean, &result.cov, options.tolerance);
                let estimate = Estimate {
                    model,
                    predicted: predicted_at,
                    mean: result.mean,
                    cov: result.cov,
                    filled: result.filled,
                };
                let titles = estimate.titles(&columns);
                let mut means = row_table("EM Means", "EM", &titles, &estimate.means());
                means.caption = Some(format!(
                    "{} after {} iterations.",
                    match result.converged {
                        true => "Converged",
                        false => "Not converged",
                    },
                    result.iterations
                ));
                tables.push(means);
                let cov = estimate.cov();
                tables.push(matrix_table("EM Covariances", &titles, &cov));
                tables.push(matrix_table(
                    "EM Correlations",
                    &titles,
                    &correlations(&cov),
                ));
                let sig = if df > 0.0 {
                    chi_square_p(chi, df)
                } else {
                    f64::NAN
                };
                tables.push(table(
                    "Little's MCAR Test",
                    0,
                    vec!["Chi-Square".into(), "df".into(), "Sig.".into()],
                    vec![vec![num(chi), num(df), num(sig)]],
                ));
                estimate
            } else {
                let complete: Vec<usize> = (0..n)
                    .filter(|&c| mx[c].iter().all(|v| !v.is_nan()))
                    .collect();
                let mut addtype = options.addtype.unwrap_or(if 2 * complete.len() < n {
                    AddType::Normal
                } else {
                    AddType::Residual
                });
                if addtype == AddType::Residual && complete.is_empty() {
                    warnings.push(
                        "REGRESSION: no case is complete, so normal errors replace residuals"
                            .into(),
                    );
                    addtype = AddType::Normal;
                }
                let valid: Vec<usize> = (0..model.len())
                    .map(|p| mx.iter().filter(|r| !r[p].is_nan()).count())
                    .collect();
                let is_predictor: Vec<bool> =
                    model.iter().map(|j| predictors.contains(j)).collect();
                let mut fits: BTreeMap<(Vec<bool>, usize), Fit> = BTreeMap::new();
                let mut filled = mx.clone();
                let mut random = self.random.borrow_mut();
                for c in 0..n {
                    let pattern: Vec<bool> = mx[c].iter().map(|v| v.is_nan()).collect();
                    for &j in predicted_at.iter().filter(|&&j| pattern[j]) {
                        let fit = fits.entry((pattern.clone(), j)).or_insert_with(|| {
                            let candidates: Vec<usize> = (0..model.len())
                                .filter(|&l| l != j && is_predictor[l] && !pattern[l])
                                .collect();
                            stepwise(j, &candidates, &start_cov, valid[j], options)
                        });
                        let predict = |row: &[f64]| {
                            start_mean[j]
                                + fit
                                    .coefficients
                                    .iter()
                                    .map(|&(l, b)| b * (row[l] - start_mean[l]))
                                    .sum::<f64>()
                        };
                        let sd = fit.variance.max(0.0).sqrt();
                        let error = match addtype {
                            AddType::None => 0.0,
                            AddType::Normal => {
                                sd * draw(&mut random, "RV.NORMAL", &[0.0, 1.0]).unwrap_or(0.0)
                            }
                            AddType::T(df) => sd * draw(&mut random, "RV.T", &[df]).unwrap_or(0.0),
                            AddType::Residual => {
                                let pick = ((random.uniform() * complete.len() as f64) as usize)
                                    .min(complete.len() - 1);
                                let row = &mx[complete[pick]];
                                row[j] - predict(row)
                            }
                        };
                        filled[c][j] = predict(&mx[c]) + error;
                    }
                }
                drop(random);
                let (mean, cov) = moments(&filled);
                let estimate = Estimate {
                    model,
                    predicted: predicted_at,
                    mean,
                    cov,
                    filled,
                };
                let titles = estimate.titles(&columns);
                let mut means =
                    row_table("Regression Means", "Regression", &titles, &estimate.means());
                means.caption = Some(format!(
                    "Missing values estimated by regression with {} added.",
                    match addtype {
                        AddType::None => "no error term".to_string(),
                        AddType::Normal => "normal errors".to_string(),
                        AddType::T(df) => format!("t({df}) errors"),
                        AddType::Residual => "residuals of random complete cases".to_string(),
                    }
                ));
                tables.push(means);
                let cov = estimate.cov();
                tables.push(matrix_table("Regression Covariances", &titles, &cov));
                tables.push(matrix_table(
                    "Regression Correlations",
                    &titles,
                    &correlations(&cov),
                ));
                estimate
            };
            let (mean, sd) = estimate.per_column(&quantitative);
            summary.push((if is_em { "EM" } else { "Regression" }, mean, sd));
            if let Some(file) = &options.outfile {
                saved.push((file.clone(), estimate));
            }
        }
        if summary.len() > 2 {
            let rows = |sd: bool| -> Vec<Vec<Cell>> {
                summary
                    .iter()
                    .map(|(label, means, sds)| {
                        let mut row = vec![text(*label)];
                        row.extend((if sd { sds } else { means }).iter().map(|&v| num(v)));
                        row
                    })
                    .collect()
            };
            let mut header = vec![String::new()];
            header.extend(titles.iter().cloned());
            tables.push(table(
                "Summary of Estimated Means",
                1,
                header.clone(),
                rows(false),
            ));
            tables.push(table(
                "Summary of Estimated Standard Deviations",
                1,
                header,
                rows(true),
            ));
        }

        let mut files = Vec::new();
        for (file, estimate) in saved {
            let mut out: Dataset = data.clone();
            for &p in &estimate.predicted {
                let column = &columns[estimate.model[p]];
                let values = (0..n)
                    .map(|c| {
                        let value = match column.missing(c) {
                            true => estimate.filled[c][p],
                            false => column.values[c],
                        };
                        match (column.states[c], value.is_finite()) {
                            (_, true) => Some(value),
                            (State::User(_), false) => out_original(data, &column.name, c),
                            _ => None,
                        }
                    })
                    .collect();
                out.set_numeric_values(&column.name, values)?;
            }
            files.push((file, out));
        }
        for warning in warnings {
            self.warn(warning);
        }
        for (file, out) in files {
            self.write_data_file(&file, out)?;
        }
        Ok(tables)
    }
}

/// The value as stored, for a user-missing value that could not be estimated.
fn out_original(data: &Dataset, name: &str, case: usize) -> Option<f64> {
    match data.variable(name).ok()?.values() {
        Values::Numeric(v) => v.get(case),
        Values::String(_) => None,
    }
}

fn univariate(columns: &[Column], n: usize, quartiles: bool) -> Table {
    let mut rows = Vec::new();
    for column in columns {
        let missing = column.missing_count();
        let mut row = vec![text(column.title.clone()), count(n - missing)];
        if column.categorical {
            row.extend([Cell::Empty, Cell::Empty]);
        } else {
            let (mean, sd) = mean_sd(&column.valid());
            row.extend([num(mean), num(sd)]);
        }
        row.extend([count(missing), num(percent(missing, n))]);
        if column.categorical {
            row.extend([Cell::Empty, Cell::Empty]);
        } else {
            let low = column.extremes.iter().filter(|&&e| e < 0).count();
            let high = column.extremes.iter().filter(|&&e| e > 0).count();
            row.extend([count(low), count(high)]);
        }
        rows.push(row);
    }
    let columns_ = [
        "",
        "N",
        "Mean",
        "Std. Deviation",
        "Count",
        "Percent",
        "Low",
        "High",
    ];
    let groups = [
        "",
        "",
        "",
        "",
        "Missing",
        "Missing",
        "No. of Extremes",
        "No. of Extremes",
    ];
    Table {
        title: "Univariate Statistics".into(),
        row_headers: 1,
        columns: columns_.iter().map(|s| s.to_string()).collect(),
        column_groups: groups.iter().map(|g| vec![g.to_string()]).collect(),
        rows,
        caption: Some(
            match quartiles {
                true => {
                    "Extremes lie more than 1.5 interquartile ranges below the first quartile \
                         or above the third."
                }
                false => "Extremes lie more than 2 standard deviations from the mean.",
            }
            .into(),
        ),
        ..Table::default()
    }
}

fn ttests(
    columns: &[Column],
    quantitative: &[usize],
    n: usize,
    options: &statscraft_syntax::MvaTtest,
) -> Option<Table> {
    let mut rows = Vec::new();
    for (j, indicator) in columns.iter().enumerate() {
        if percent(indicator.missing_count(), n) < options.percent {
            continue;
        }
        let mut stats: Vec<(&str, Vec<Cell>)> = Vec::new();
        let (mut t_row, mut df_row, mut p_row) = (Vec::new(), Vec::new(), Vec::new());
        let (mut present_row, mut missing_row) = (Vec::new(), Vec::new());
        let (mut mean_present, mut mean_missing) = (Vec::new(), Vec::new());
        for &k in quantitative {
            let x = &columns[k];
            let (a, b): (Vec<f64>, Vec<f64>) = {
                let valid = (0..n).filter(|&c| !x.missing(c));
                let (a, b): (Vec<usize>, Vec<usize>) = valid.partition(|&c| !indicator.missing(c));
                (
                    a.iter().map(|&c| x.values[c]).collect(),
                    b.iter().map(|&c| x.values[c]).collect(),
                )
            };
            let (ma, sa) = mean_sd(&a);
            let (mb, sb) = mean_sd(&b);
            let (na, nb) = (a.len() as f64, b.len() as f64);
            let (va, vb) = (sa * sa / na, sb * sb / nb);
            let ok = j != k && a.len() >= 2 && b.len() >= 2 && va + vb > 0.0;
            let t = (ma - mb) / (va + vb).sqrt();
            let df = (va + vb).powi(2) / (va * va / (na - 1.0) + vb * vb / (nb - 1.0));
            t_row.push(if ok { num(t) } else { Cell::Empty });
            df_row.push(if ok { num(df) } else { Cell::Empty });
            p_row.push(if ok { num(t_p(t, df)) } else { Cell::Empty });
            present_row.push(count(a.len()));
            missing_row.push(count(b.len()));
            mean_present.push(num(ma));
            mean_missing.push(num(mb));
        }
        if options.t {
            stats.push(("t", t_row));
        }
        if options.df {
            stats.push(("df", df_row));
        }
        if options.prob {
            stats.push(("P(2-tail)", p_row));
        }
        if options.counts {
            stats.push(("# Present", present_row));
            stats.push(("# Missing", missing_row));
        }
        if options.means {
            stats.push(("Mean(Present)", mean_present));
            stats.push(("Mean(Missing)", mean_missing));
        }
        for (label, cells) in stats {
            let mut row = vec![text(indicator.title.clone()), text(label)];
            row.extend(cells);
            rows.push(row);
        }
    }
    if rows.is_empty() || quantitative.is_empty() {
        return None;
    }
    let mut header = vec![String::new(), String::new()];
    header.extend(quantitative.iter().map(|&k| columns[k].title.clone()));
    let mut t = table("Separate Variance t Tests", 2, header, rows);
    t.caption = Some(format!(
        "Each column variable compared between the cases where the row variable is present and \
         where it is missing.{}",
        match options.percent > 0.0 {
            true => format!(
                " Variables with less than {}% missing are not shown.",
                options.percent
            ),
            false => String::new(),
        }
    ));
    Some(t)
}

fn crosstab(
    columns: &[Column],
    c: usize,
    n: usize,
    limit: f64,
    shown: &Shown,
    data: &Dataset,
) -> Result<Table, Error> {
    let by = &columns[c];
    let mut keys: Vec<Key> = by.keys.iter().flatten().cloned().collect();
    keys.sort_by(|a, b| a.compare(b));
    keys.dedup_by(|a, b| a.compare(b).is_eq());
    let variable = data.variable(&by.name)?;
    let mut header = vec![
        String::new(),
        String::new(),
        String::new(),
        "Total".to_string(),
    ];
    header.extend(keys.iter().map(|k| shown.value(&variable, k)));
    let groups: Vec<Vec<usize>> = std::iter::once((0..n).collect())
        .chain(keys.iter().map(|key| {
            (0..n)
                .filter(|&case| {
                    by.keys[case]
                        .as_ref()
                        .is_some_and(|k| k.compare(key).is_eq())
                })
                .collect()
        }))
        .collect();
    let mut rows = Vec::new();
    for (l, other) in columns.iter().enumerate() {
        if l == c || percent(other.missing_count(), n) < limit {
            continue;
        }
        let share = |test: &dyn Fn(State) -> bool| -> Vec<Cell> {
            groups
                .iter()
                .map(|g| {
                    num(percent(
                        g.iter().filter(|&&i| test(other.states[i])).count(),
                        g.len(),
                    ))
                })
                .collect()
        };
        let mut add = |kind: &str, label: String, cells: Vec<Cell>| {
            let mut row = vec![text(other.title.clone()), text(kind), text(label)];
            row.extend(cells);
            rows.push(row);
        };
        add(
            "Present",
            "Count".into(),
            groups
                .iter()
                .map(|g| count(g.iter().filter(|&&i| !other.missing(i)).count()))
                .collect(),
        );
        add("Present", "Percent".into(), share(&|s| s == State::Valid));
        add("Missing", "% SysMis".into(), share(&|s| s == State::System));
        for (t, name) in other.user_types.iter().enumerate().take(3) {
            let letter = ["A", "B", "C"][t];
            add(
                "Missing",
                format!("% {letter} ({name})"),
                share(&|s| s == State::User(t)),
            );
        }
    }
    let mut t = table(&format!("Crosstabulation of {}", by.title), 3, header, rows);
    t.caption = Some(format!(
        "Percents are of the cases in each column.{}",
        match limit > 0.0 {
            true => format!(" Variables with less than {limit}% missing are not shown."),
            false => String::new(),
        }
    ));
    Ok(t)
}

fn mismatch(columns: &[Column], n: usize, limit: f64, nosort: bool) -> Table {
    let order: Vec<usize> = sorted_columns(columns, nosort)
        .into_iter()
        .filter(|&j| percent(columns[j].missing_count(), n) >= limit)
        .collect();
    let titles: Vec<String> = order.iter().map(|&j| columns[j].title.clone()).collect();
    let m: Matrix = order
        .iter()
        .map(|&a| {
            order
                .iter()
                .map(|&b| {
                    let hits = (0..n)
                        .filter(|&c| match a == b {
                            true => columns[a].missing(c),
                            false => columns[a].missing(c) != columns[b].missing(c),
                        })
                        .count();
                    percent(hits, n)
                })
                .collect()
        })
        .collect();
    let mut t = matrix_table("Percent Mismatch of Indicator Variables", &titles, &m);
    t.caption = Some(format!(
        "The diagonal holds the percent missing of each variable, the other cells the percent of \
         cases where one variable of the pair is missing and the other present.{}",
        match limit > 0.0 {
            true => format!(" Variables with less than {limit}% missing are not shown."),
            false => String::new(),
        }
    ));
    t
}

/// DPATTERN (every case) or MPATTERN (the cases with a missing value).
#[allow(clippy::too_many_arguments)]
fn case_patterns(
    columns: &[Column],
    n: usize,
    only_missing: bool,
    options: &MvaPattern,
    sort: &[(Variable, bool)],
    describe: &[String],
    data: &Dataset,
    shown: &Shown,
    case_label: &dyn Fn(usize) -> Cell,
) -> Result<Table, Error> {
    let order = match only_missing {
        true => sorted_columns(columns, options.nosort),
        false => (0..columns.len()).collect(),
    };
    let missing_of = |c: usize| columns.iter().filter(|col| col.missing(c)).count();
    let mut cases: Vec<usize> = (0..n)
        .filter(|&c| !only_missing || missing_of(c) > 0)
        .collect();
    if only_missing {
        cases.sort_by_key(|&c| pattern_key(columns, &order, c));
    } else if !sort.is_empty() {
        cases.sort_by(|&a, &b| {
            for (variable, descending) in sort {
                let ordering = match variable.values() {
                    Values::Numeric(v) => {
                        let key = |c| v.get(c).unwrap_or(f64::NEG_INFINITY);
                        key(a).total_cmp(&key(b))
                    }
                    Values::String(v) => v.get(a).cmp(v.get(b)),
                };
                let ordering = if *descending {
                    ordering.reverse()
                } else {
                    ordering
                };
                if ordering.is_ne() {
                    return ordering;
                }
            }
            std::cmp::Ordering::Equal
        });
    }
    let described: Vec<Variable> = describe
        .iter()
        .map(|name| data.variable(name))
        .collect::<Result<_, _>>()?;
    let mut header = vec!["Case".to_string(), "# Missing".into(), "% Missing".into()];
    header.extend(order.iter().map(|&j| columns[j].title.clone()));
    header.extend(described.iter().map(|v| shown.variable(v)));
    let rows = cases
        .iter()
        .map(|&c| {
            let missing = missing_of(c);
            let mut row = vec![
                case_label(c),
                count(missing),
                num(percent(missing, columns.len())),
            ];
            row.extend(order.iter().map(|&j| symbol(&columns[j], c)));
            row.extend(described.iter().map(|v| plain_value(v, c)));
            row
        })
        .collect();
    let mut t = table(
        match only_missing {
            true => "Missing Patterns",
            false => "Data Patterns",
        },
        1,
        header,
        rows,
    );
    t.caption = Some(
        match only_missing {
            true => "Cases with a missing value. ",
            false => "All cases. ",
        }
        .to_string()
            + "S: system-missing, A, B, C: user-missing of the first, second, third kind, \
               +: extremely high, -: extremely low.",
    );
    Ok(t)
}

fn tabulated(
    columns: &[Column],
    n: usize,
    options: &MvaPattern,
    describe: &[String],
    categorical: &[String],
    data: &Dataset,
    shown: &Shown,
) -> Result<Table, Error> {
    let order = sorted_columns(columns, options.nosort);
    let mut patterns: BTreeMap<Vec<bool>, Vec<usize>> = BTreeMap::new();
    for c in 0..n {
        patterns
            .entry(pattern_key(columns, &order, c))
            .or_default()
            .push(c);
    }
    let mut header = vec!["Number of Cases".to_string()];
    header.extend(order.iter().map(|&j| columns[j].title.clone()));
    // DESCRIBE: a count per category of a categorical variable, the mean of a quantitative one
    let mut describers: Vec<(Variable, Option<Vec<Key>>)> = Vec::new();
    for name in describe {
        let variable = data.variable(name)?;
        let is_categorical = variable.is_string() || categorical.contains(name);
        let keys = is_categorical.then(|| {
            let mut keys: Vec<Key> = read_column(&variable, true, shown)
                .keys
                .into_iter()
                .flatten()
                .collect();
            keys.sort_by(|a, b| a.compare(b));
            keys.dedup_by(|a, b| a.compare(b).is_eq());
            keys
        });
        match &keys {
            Some(keys) => header.extend(keys.iter().map(|k| {
                format!(
                    "{} = {}",
                    shown.variable(&variable),
                    shown.value(&variable, k)
                )
            })),
            None => header.push(format!("{} (Mean)", shown.variable(&variable))),
        }
        describers.push((variable, keys));
    }
    let read: Vec<Column> = describers
        .iter()
        .map(|(v, keys)| read_column(v, keys.is_some(), shown))
        .collect();
    let minimum = options.percent / 100.0 * n as f64;
    let mut rows = Vec::new();
    for (key, cases) in &patterns {
        if (cases.len() as f64) < minimum {
            continue;
        }
        let mut row = vec![count(cases.len())];
        // the key runs from the last column to the first
        row.extend(key.iter().rev().map(|&m| match m {
            true => text("X"),
            false => Cell::Empty,
        }));
        for ((_, keys), column) in describers.iter().zip(&read) {
            match keys {
                Some(keys) => row.extend(keys.iter().map(|k| {
                    count(
                        cases
                            .iter()
                            .filter(|&&c| {
                                column.keys[c]
                                    .as_ref()
                                    .is_some_and(|x| x.compare(k).is_eq())
                            })
                            .count(),
                    )
                })),
                None => {
                    let values: Vec<f64> = cases
                        .iter()
                        .map(|&c| column.values[c])
                        .filter(|v| !v.is_nan())
                        .collect();
                    row.push(num(mean_sd(&values).0));
                }
            }
        }
        rows.push(row);
    }
    let mut t = table("Tabulated Patterns", 0, header, rows);
    t.caption = Some(format!(
        "X marks a missing value.{}",
        match options.percent > 0.0 {
            true => format!(
                " Patterns of less than {}% of the cases are not shown.",
                options.percent
            ),
            false => String::new(),
        }
    ));
    Ok(t)
}
