//! MULTIPLE IMPUTATION: summaries of the missing values and several completed copies of the
//! data, each with its own random draws in place of the missing values.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (Multiple Imputation
//! Algorithms), in our words:
//! - Nominal, ordinal and string variables are categorical, the others scale (an unknown level
//!   is taken from the data). System-missing and user-missing values are both missing; a blank
//!   string is a valid value.
//! - WEIGHT replicates cases, rounded to whole numbers; ANALYSISWEIGHT weights the regressions.
//!   A case whose rounded weight or analysis weight is not positive is left out entirely. A case
//!   missing on every variable of the model counts in the summaries but is neither used nor
//!   filled. MAXPCTMISSING drops a variable with more missing values from the model.
//! - A scale variable is drawn from a linear regression on its predictors: fit on the cases
//!   where it is valid, draw sigma^2 as RSS / chi-square(N - p) (N the replicated cases, p the
//!   non-redundant parameters), the coefficients from a normal around the fit with covariance
//!   sigma^2 (X'WX)^-1, and each value as its prediction plus sigma / sqrt(w) times a normal.
//!   PMM draws the coefficients the same way, then takes the observed value of a case picked at
//!   random among the k whose predictions are nearest. A categorical variable is drawn from a
//!   multinomial logistic regression (last category the base) with coefficients drawn from the
//!   normal approximation of their posterior. A variable constant over its valid values gets
//!   that value.
//! - MIN and MAX: a drawn value outside them (after RND) is drawn again, up to MAXCASEDRAWS
//!   times; then new coefficients, up to MAXPARAMDRAWS sets, else an error. RND rounds to the
//!   nearest multiple. Neither applies to PMM bounds or to categorical variables; RND not to
//!   dates.
//! - MONOTONE goes through the variables once in order, each from the ones before it. FCS fills
//!   the missing values with random starting values (normal within the observed range, or
//!   categories in their observed proportions), then goes through the variables with missing
//!   values MAXITER times, each from all the others. AUTO sorts the variables from fewest to
//!   most missing, takes MONOTONE when the pattern is monotone in that order and FCS
//!   otherwise, always with linear models, main effects and the default iterations and
//!   singularity tolerance.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, summarize};
use crate::matrix::{Matrix, cholesky, inverse, sweep};
use crate::pass::default_measure;
use crate::random::{Random, draw};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, SplitFile, Value as DataValue};
use statscraft_syntax::{FormatType, ImputeMethod, ImputeRole, MultipleImputation, Value};

/// The name of the variable that numbers the imputations.
pub(crate) const IMPUTATION: &str = "Imputation_";

/// An imputation, an iteration and the mean and SD of each traced variable after it.
type Trace = (usize, usize, Vec<[Option<f64>; 2]>);

/// A row group of the descriptives: its label, the imputation and the weighted values.
type Summary<'a> = (&'a str, Cell, Vec<(f64, f64)>);

/// One analysis variable of the active split group.
struct Var {
    name: String,
    categorical: bool,
    date: bool,
    /// Categorical: the valid values in ascending order.
    levels: Vec<Key>,
    /// Scale: the value; categorical: the index of its level. None where missing.
    values: Vec<Option<f64>>,
    role: ImputeRole,
    min: Option<f64>,
    max: Option<f64>,
    rnd: Option<f64>,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Linear,
    Pmm(usize),
    Logistic,
    Constant,
}

/// What every model draw needs besides the variables.
struct Ctx<'a> {
    vars: &'a [Var],
    /// The replication weight and the regression weight of every case.
    freq: &'a [f64],
    weight: &'a [f64],
    usable: &'a [bool],
    singular: f64,
    case_draws: usize,
    param_draws: usize,
}

/// The weighted count, mean, standard deviation, minimum and maximum of (value, weight) pairs.
fn stats(pairs: &[(f64, f64)]) -> [Option<f64>; 5] {
    let s = summarize(pairs);
    [Some(s.n), s.mean, s.sd, s.min, s.max]
}

fn text(s: impl Into<String>) -> Cell {
    Cell::Text(s.into())
}

fn num(x: Option<f64>) -> Cell {
    x.filter(|v| v.is_finite())
        .map_or(Cell::Empty, Cell::Number)
}

fn table(title: &str, row_headers: usize, columns: &[&str], rows: Vec<Vec<Cell>>) -> Table {
    Table {
        title: title.into(),
        row_headers,
        column_groups: vec![Vec::new(); columns.len()],
        columns: columns.iter().map(|c| c.to_string()).collect(),
        rows,
        ..Table::default()
    }
}

/// The columns that are not redundant: each is swept in order unless what is left of its
/// diagonal falls to `singular` times its original value.
fn independent(xtx: &Matrix, singular: f64) -> Vec<usize> {
    let mut m = xtx.clone();
    let mut kept = Vec::new();
    for k in 0..m.len() {
        if xtx[k][k] > 0.0 && m[k][k] > singular * xtx[k][k] {
            sweep(&mut m, k);
            kept.push(k);
        }
    }
    kept
}

/// The lower Cholesky factor of `a`, with a little added to the diagonal when rounding left it
/// not quite positive definite.
fn root(a: &Matrix) -> Matrix {
    let mut jitter = 0.0;
    let top = a
        .iter()
        .enumerate()
        .map(|(i, r)| r[i].abs())
        .fold(0.0, f64::max);
    loop {
        let mut b = a.clone();
        for (i, row) in b.iter_mut().enumerate() {
            row[i] += jitter;
        }
        if let Some(l) = cholesky(&b) {
            return l;
        }
        jitter = if jitter == 0.0 {
            1e-12 * top.max(1e-300)
        } else {
            jitter * 100.0
        };
    }
}

fn normals(rng: &mut Random, n: usize) -> Vec<f64> {
    (0..n).map(|_| rng.normal()).collect()
}

/// `center + l v`
fn shifted(center: &[f64], l: &Matrix, v: &[f64], scale: f64) -> Vec<f64> {
    center
        .iter()
        .enumerate()
        .map(|(i, c)| c + scale * (0..=i).map(|k| l[i][k] * v[k]).sum::<f64>())
        .collect()
}

fn dot(x: &[f64], cols: &[usize], b: &[f64]) -> f64 {
    cols.iter().zip(b).map(|(&c, b)| x[c] * b).sum()
}

/// The design row of a case: the constant, each scale predictor, an indicator of every level
/// of a categorical predictor but its last, and with `twoway` the products of the indicators
/// of every pair of categorical predictors.
fn design(vars: &[Var], preds: &[usize], twoway: bool, cur: &[Vec<f64>], case: usize) -> Vec<f64> {
    let mut row = vec![1.0];
    let mut dummies: Vec<Vec<f64>> = Vec::new();
    for &p in preds {
        let v = cur[p][case];
        if vars[p].categorical {
            let d: Vec<f64> = (0..vars[p].levels.len() - 1)
                .map(|l| f64::from(u8::from(v as usize == l)))
                .collect();
            row.extend(&d);
            dummies.push(d);
        } else {
            row.push(v);
        }
    }
    if twoway {
        for a in 0..dummies.len() {
            for b in a + 1..dummies.len() {
                for x in &dummies[a] {
                    row.extend(dummies[b].iter().map(|y| x * y));
                }
            }
        }
    }
    row
}

/// The number of columns `design` gives.
fn width(vars: &[Var], preds: &[usize], twoway: bool) -> usize {
    let cat: Vec<usize> = preds
        .iter()
        .filter(|&&p| vars[p].categorical)
        .map(|&p| vars[p].levels.len() - 1)
        .collect();
    let scale = preds.iter().filter(|&&p| !vars[p].categorical).count();
    let mut n = 1 + scale + cat.iter().sum::<usize>();
    if twoway {
        for a in 0..cat.len() {
            n += cat[a + 1..].iter().map(|b| cat[a] * b).sum::<usize>();
        }
    }
    n
}

/// Weighted cross products of the rows.
fn cross(rows: &[Vec<f64>], w: &[f64]) -> Matrix {
    let p = rows.first().map_or(0, Vec::len);
    let mut m = vec![vec![0.0; p]; p];
    for (row, &w) in rows.iter().zip(w) {
        for a in 0..p {
            if row[a] != 0.0 {
                for b in 0..p {
                    m[a][b] += w * row[a] * row[b];
                }
            }
        }
    }
    m
}

struct LinearFit {
    cols: Vec<usize>,
    beta: Vec<f64>,
    root: Matrix,
    rss: f64,
    df: f64,
}

fn fit_linear(rows: &[Vec<f64>], y: &[f64], w: &[f64], n: f64, singular: f64) -> Option<LinearFit> {
    let xtx = cross(rows, w);
    let cols = independent(&xtx, singular);
    let sub: Matrix = cols
        .iter()
        .map(|&a| cols.iter().map(|&b| xtx[a][b]).collect())
        .collect();
    let xty: Vec<f64> = cols
        .iter()
        .map(|&a| {
            rows.iter()
                .zip(y)
                .zip(w)
                .map(|((r, y), w)| w * r[a] * y)
                .sum()
        })
        .collect();
    let (inv, _) = inverse(&sub)?;
    let beta: Vec<f64> = inv
        .iter()
        .map(|r| r.iter().zip(&xty).map(|(a, b)| a * b).sum())
        .collect();
    let rss = rows
        .iter()
        .zip(y)
        .zip(w)
        .map(|((r, y), w)| w * (y - dot(r, &cols, &beta)).powi(2))
        .sum();
    let df = n - cols.len() as f64;
    (df > 0.0).then(|| LinearFit {
        root: root(&inv),
        cols,
        beta,
        rss,
        df,
    })
}

/// The coefficients and sigma of one draw from the posterior.
fn draw_linear(fit: &LinearFit, rng: &mut Random) -> (Vec<f64>, f64) {
    let u = draw(rng, "RV.CHISQ", &[fit.df]).unwrap_or(fit.df);
    let sigma = (fit.rss / u).sqrt();
    let v = normals(rng, fit.beta.len());
    (shifted(&fit.beta, &fit.root, &v, sigma), sigma)
}

/// The probability of every category, the last the base, from the linear predictors.
fn probabilities(eta: &[f64]) -> Vec<f64> {
    let top = eta.iter().copied().fold(0.0_f64, f64::max);
    let mut p: Vec<f64> = eta.iter().map(|e| (e - top).exp()).collect();
    p.push((-top).exp());
    let sum: f64 = p.iter().sum();
    p.iter().map(|v| v / sum).collect()
}

fn etas(row: &[f64], cols: &[usize], b: &[f64], k: usize) -> Vec<f64> {
    let q = cols.len();
    (0..k - 1)
        .map(|l| dot(row, cols, &b[l * q..(l + 1) * q]))
        .collect()
}

struct LogitFit {
    cols: Vec<usize>,
    b: Vec<f64>,
    root: Matrix,
    k: usize,
}

/// Maximum likelihood by Newton's method with step halving; the covariance is the inverse of
/// the information at the estimate.
fn fit_logistic(rows: &[Vec<f64>], y: &[usize], w: &[f64], k: usize, singular: f64) -> LogitFit {
    let cols = independent(&cross(rows, w), singular);
    let q = cols.len();
    let size = q * (k - 1);
    let loglik = |b: &[f64]| -> f64 {
        rows.iter()
            .zip(y)
            .zip(w)
            .map(|((r, &y), w)| w * probabilities(&etas(r, &cols, b, k))[y].max(1e-300).ln())
            .sum()
    };
    let information = |b: &[f64]| -> (Matrix, Vec<f64>) {
        let mut info = vec![vec![0.0; size]; size];
        let mut g = vec![0.0; size];
        for ((r, &y), &w) in rows.iter().zip(y).zip(w) {
            let p = probabilities(&etas(r, &cols, b, k));
            let x: Vec<f64> = cols.iter().map(|&c| r[c]).collect();
            for l in 0..k - 1 {
                let resid = f64::from(u8::from(y == l)) - p[l];
                for s in 0..q {
                    g[l * q + s] += w * resid * x[s];
                }
                for h in 0..k - 1 {
                    let c = w * p[l] * (f64::from(u8::from(l == h)) - p[h]);
                    for s in 0..q {
                        for t in 0..q {
                            info[l * q + s][h * q + t] += c * x[s] * x[t];
                        }
                    }
                }
            }
        }
        (info, g)
    };
    let mut b = vec![0.0; size];
    let mut ll = loglik(&b);
    for _ in 0..50 {
        let (info, g) = information(&b);
        let Some((inv, _)) = inverse(&info) else {
            break;
        };
        let step: Vec<f64> = inv
            .iter()
            .map(|r| r.iter().zip(&g).map(|(a, v)| a * v).sum())
            .collect();
        let mut scale = 1.0;
        let mut moved = false;
        for _ in 0..20 {
            let candidate: Vec<f64> = b.iter().zip(&step).map(|(b, s)| b + scale * s).collect();
            let lc = loglik(&candidate);
            if lc.is_finite() && lc >= ll - 1e-12 * ll.abs() {
                let change = step.iter().fold(0.0_f64, |m, s| m.max((scale * s).abs()));
                b = candidate;
                moved = (lc - ll).abs() > 1e-10 * (1.0 + ll.abs()) && change > 1e-9;
                ll = lc;
                break;
            }
            scale /= 2.0;
        }
        if !moved {
            break;
        }
    }
    let (mut info, _) = information(&b);
    // separation leaves the information singular; a small ridge keeps the draw defined
    let cov = inverse(&info).map(|(m, _)| m).unwrap_or_else(|| {
        for (i, row) in info.iter_mut().enumerate() {
            row[i] += 1e-8;
        }
        inverse(&info).map_or_else(|| vec![vec![0.0; size]; size], |(m, _)| m)
    });
    LogitFit {
        root: root(&cov),
        cols,
        b,
        k,
    }
}

impl Ctx<'_> {
    /// The cases used to fit the model of `j` and the cases whose value of `j` is drawn.
    fn cases(&self, j: usize) -> (Vec<usize>, Vec<usize>) {
        (0..self.usable.len())
            .filter(|&c| self.usable[c])
            .partition(|&c| self.vars[j].values[c].is_some())
    }

    /// New values of `j` for its missing cases, given the current values `cur`.
    fn impute(
        &self,
        j: usize,
        preds: &[usize],
        twoway: bool,
        kind: Kind,
        cur: &[Vec<f64>],
        rng: &mut Random,
    ) -> Result<Vec<(usize, f64)>, Error> {
        let var = &self.vars[j];
        let (obs, mis) = self.cases(j);
        if mis.is_empty() {
            return Ok(Vec::new());
        }
        if kind == Kind::Constant {
            let value = cur[j][obs[0]];
            return Ok(mis.iter().map(|&c| (c, value)).collect());
        }
        let rows: Vec<Vec<f64>> = obs
            .iter()
            .map(|&c| design(self.vars, preds, twoway, cur, c))
            .collect();
        let w: Vec<f64> = obs.iter().map(|&c| self.freq[c] * self.weight[c]).collect();
        let targets: Vec<Vec<f64>> = mis
            .iter()
            .map(|&c| design(self.vars, preds, twoway, cur, c))
            .collect();
        if kind == Kind::Logistic {
            let y: Vec<usize> = obs.iter().map(|&c| cur[j][c] as usize).collect();
            let fit = fit_logistic(&rows, &y, &w, var.levels.len(), self.singular);
            let z = normals(rng, fit.b.len());
            let b = shifted(&fit.b, &fit.root, &z, 1.0);
            return Ok(mis
                .iter()
                .zip(&targets)
                .map(|(&c, x)| {
                    let p = probabilities(&etas(x, &fit.cols, &b, fit.k));
                    let u = rng.uniform();
                    let mut sum = 0.0;
                    let pick = p.iter().position(|p| {
                        sum += p;
                        u < sum
                    });
                    (c, pick.unwrap_or(fit.k - 1) as f64)
                })
                .collect());
        }
        let y: Vec<f64> = obs.iter().map(|&c| cur[j][c]).collect();
        let n: f64 = obs.iter().map(|&c| self.freq[c]).sum();
        let Some(fit) = fit_linear(&rows, &y, &w, n, self.singular) else {
            return Err(Error(format!(
                "{}: too few valid cases to fit its imputation model ({n} for {} parameters)",
                var.name,
                width(self.vars, preds, twoway)
            )));
        };
        if let Kind::Pmm(k) = kind {
            let (beta, _) = draw_linear(&fit, rng);
            let fitted: Vec<f64> = rows.iter().map(|r| dot(r, &fit.cols, &beta)).collect();
            return Ok(mis
                .iter()
                .zip(&targets)
                .map(|(&c, x)| {
                    let target = dot(x, &fit.cols, &beta);
                    let mut order: Vec<usize> = (0..fitted.len()).collect();
                    order.sort_by(|&a, &b| {
                        (fitted[a] - target)
                            .abs()
                            .total_cmp(&(fitted[b] - target).abs())
                    });
                    let near = k.min(order.len());
                    let pick = ((rng.uniform() * near as f64) as usize).min(near - 1);
                    (c, self.rounded(var, y[order[pick]]))
                })
                .collect());
        }
        for _ in 0..self.param_draws {
            let (beta, sigma) = draw_linear(&fit, rng);
            let mut out = Vec::with_capacity(mis.len());
            for (&c, x) in mis.iter().zip(&targets) {
                let mean = dot(x, &fit.cols, &beta);
                let spread = sigma / self.weight[c].sqrt();
                let value = (0..self.case_draws).find_map(|_| {
                    let v = self.rounded(var, mean + spread * rng.normal());
                    (var.min.is_none_or(|m| v >= m) && var.max.is_none_or(|m| v <= m)).then_some(v)
                });
                match value {
                    Some(v) => out.push((c, v)),
                    None => break,
                }
            }
            if out.len() == mis.len() {
                return Ok(out);
            }
        }
        Err(Error(format!(
            "{}: no values within its MIN and MAX after {} draws per case and {} sets of \
             parameters; check the CONSTRAINTS or raise MAXCASEDRAWS and MAXPARAMDRAWS",
            var.name, self.case_draws, self.param_draws
        )))
    }

    fn rounded(&self, var: &Var, v: f64) -> f64 {
        match var.rnd.filter(|_| !var.date) {
            Some(step) => (v / step).round() * step,
            None => v,
        }
    }

    /// Random starting values of FCS for the missing cases of `j`.
    fn start(&self, j: usize, cur: &mut [Vec<f64>], rng: &mut Random) {
        let var = &self.vars[j];
        let (obs, mis) = self.cases(j);
        if obs.is_empty() {
            return;
        }
        let pairs: Vec<(f64, f64)> = obs.iter().map(|&c| (cur[j][c], self.freq[c])).collect();
        if var.categorical {
            let mut shares = vec![0.0; var.levels.len()];
            for (v, f) in &pairs {
                shares[*v as usize] += f;
            }
            let total: f64 = shares.iter().sum();
            for &c in &mis {
                let u = rng.uniform() * total;
                let mut sum = 0.0;
                let pick = shares.iter().position(|s| {
                    sum += s;
                    u < sum
                });
                cur[j][c] = pick.unwrap_or(shares.len() - 1) as f64;
            }
        } else {
            let [_, mean, sd, min, max] = stats(&pairs);
            let (mean, sd) = (mean.unwrap_or(0.0), sd.unwrap_or(0.0));
            for &c in &mis {
                let v = mean + sd * rng.normal();
                cur[j][c] = v.clamp(min.unwrap_or(v), max.unwrap_or(v));
            }
        }
    }
}

/// Monotone in `order`: in every case, once a variable is missing so are all after it.
fn monotone(vars: &[Var], order: &[usize], usable: &[bool]) -> bool {
    (0..usable.len()).filter(|&c| usable[c]).all(|c| {
        let mut gone = false;
        order.iter().all(|&j| {
            let missing = vars[j].values[c].is_none();
            let ok = !gone || missing;
            gone |= missing;
            ok
        })
    })
}

fn role_name(role: ImputeRole) -> &'static str {
    match role {
        ImputeRole::Both => "Dependent and Predictor",
        ImputeRole::Dependent => "Dependent only",
        ImputeRole::Predictor => "Predictor only",
    }
}

/// The value of a MIN or MAX constraint: a number, or a date in the variable's date format.
fn limit(value: &Value, name: &str, format: &str, epoch: Option<f64>) -> Result<f64, Error> {
    match value {
        Value::Number(n) => Ok(*n),
        Value::String(s) => {
            let upper = format.to_uppercase();
            let kind = match upper.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.') {
                "DATE" => FormatType::Date,
                "ADATE" => FormatType::Adate,
                "EDATE" => FormatType::Edate,
                "SDATE" => FormatType::Sdate,
                _ => {
                    return Err(Error(format!(
                        "CONSTRAINTS: {name} takes a number as its limit, not '{s}' \
                         (quoted limits are dates in DATE, ADATE, EDATE or SDATE format)"
                    )));
                }
            };
            match crate::data_list::date(s, kind, epoch) {
                Ok(Some(v)) => Ok(v),
                _ => Err(Error(format!(
                    "CONSTRAINTS: '{s}' is not a date in the {upper} format of {name}"
                ))),
            }
        }
    }
}

impl Engine {
    pub(crate) fn multiple_imputation(
        &mut self,
        c: &MultipleImputation,
    ) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let mut names = self.expand(&c.variables)?;
        // a variable named again takes the place of its last mention
        let mut seen = std::collections::HashSet::new();
        names.reverse();
        names.retain(|n| seen.insert(n.to_lowercase()));
        names.reverse();
        if names.len() < 2 {
            return Err(Error(
                "MULTIPLE IMPUTATION needs two or more analysis variables".into(),
            ));
        }
        let imputing = c.method != ImputeMethod::None;
        if imputing {
            if data.variable_index(IMPUTATION).is_some() {
                return Err(Error(format!(
                    "the data already have a variable named {IMPUTATION}; rename it before imputing"
                )));
            }
            if c.fcs_iterations.is_some() {
                for name in ["Iteration_", "SummaryStatistic_"] {
                    if data.variable_index(name).is_some() {
                        return Err(Error(format!(
                            "FCSITERATIONS needs the name {name}, which the data already use"
                        )));
                    }
                }
            }
            if self.split_group.as_ref().is_some_and(|s| s.len() >= 8) {
                return Err(Error(
                    "imputing needs a split variable of its own, and eight are in use".into(),
                ));
            }
        }
        let n = data.case_count();
        // replication weights rounded; a case with a weight below 1 or a bad analysis weight is out
        let freq: Vec<f64> = match case_weights(data)? {
            Some(w) => w.iter().map(|w| w.round()).collect(),
            None => vec![1.0; n],
        };
        let weight: Vec<f64> = match &c.analysis_weight {
            Some(name) => {
                let variable = data.variable(name)?;
                let values = variable.numeric().map_err(|_| {
                    Error(format!("ANALYSISWEIGHT {name} must be a numeric variable"))
                })?;
                (0..n)
                    .map(|i| {
                        values
                            .get(i)
                            .filter(|v| !variable.is_user_missing(*v) && *v > 0.0)
                            .unwrap_or(0.0)
                    })
                    .collect()
            }
            None => vec![1.0; n],
        };
        let counted: Vec<bool> = (0..n).map(|i| freq[i] > 0.0 && weight[i] > 0.0).collect();

        let scale_min = self.scale_min();
        let mut vars = Vec::new();
        for name in &names {
            let index = data.variable_index(name).expect("expanded");
            let variable = data.variable_at(index);
            let measure = match variable.measure() {
                Measure::Unknown => default_measure(data, index, scale_min),
                m => m,
            };
            let categorical = variable.is_string() || measure != Measure::Scale;
            let keys = categories(&variable, false);
            let format = variable.format().to_uppercase();
            let kind = format.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
            let date = !categorical
                && [
                    "DATE", "ADATE", "EDATE", "JDATE", "SDATE", "QYR", "MOYR", "WKYR", "DATETIME",
                    "YMDHMS", "TIME", "MTIME", "DTIME",
                ]
                .contains(&kind);
            let mut levels: Vec<Key> = Vec::new();
            if categorical {
                levels = keys
                    .iter()
                    .zip(&counted)
                    .filter(|(_, c)| **c)
                    .filter_map(|(k, _)| k.clone())
                    .collect();
                levels.sort_by(|a, b| a.compare(b));
                levels.dedup();
            }
            let values = keys
                .into_iter()
                .map(|k| match k? {
                    Key::Number(v) if !categorical => Some(v),
                    k => levels.iter().position(|l| *l == k).map(|i| i as f64),
                })
                .collect();
            vars.push(Var {
                name: variable.name().to_string(),
                categorical,
                date,
                levels,
                values,
                role: ImputeRole::Both,
                min: None,
                max: None,
                rnd: None,
            });
        }

        let mut tables = Vec::new();
        let mut warnings = Vec::new();
        // missing counts over the cases that count, replicated
        let missing_of = |v: &Var| -> f64 {
            (0..n)
                .filter(|&i| counted[i] && v.values[i].is_none())
                .map(|i| freq[i])
                .sum::<f64>()
                + 0.0
        };
        let total: f64 = (0..n).filter(|&i| counted[i]).map(|i| freq[i]).sum::<f64>() + 0.0;
        let missing: Vec<f64> = vars.iter().map(missing_of).collect();
        let pct = |m: f64| if total > 0.0 { 100.0 * m / total } else { 0.0 };
        if c.overall {
            let incomplete_vars = missing.iter().filter(|&&m| m > 0.0).count() as f64;
            let incomplete_cases: f64 = (0..n)
                .filter(|&i| counted[i] && vars.iter().any(|v| v.values[i].is_none()))
                .map(|i| freq[i])
                .sum::<f64>()
                + 0.0;
            let cells = missing.iter().sum::<f64>() + 0.0;
            let row = |label: &str, bad: f64, all: f64| {
                let share = |x: f64| if all > 0.0 { 100.0 * x / all } else { 0.0 };
                vec![
                    text(label),
                    Cell::Number(all - bad),
                    Cell::Number(share(all - bad)),
                    Cell::Number(bad),
                    Cell::Number(share(bad)),
                    Cell::Number(all),
                ]
            };
            tables.push(table(
                "Overall Summary of Missing Values",
                1,
                &[
                    "",
                    "Complete N",
                    "Complete Percent",
                    "Incomplete N",
                    "Incomplete Percent",
                    "Total N",
                ],
                vec![
                    row("Variables", incomplete_vars, vars.len() as f64),
                    row("Cases", incomplete_cases, total),
                    row("Values", cells, total * vars.len() as f64),
                ],
            ));
        }
        if let Some((maxvars, minpct)) = c.variable_summary {
            let mut order: Vec<usize> = (0..vars.len())
                .filter(|&j| missing[j] > 0.0 && pct(missing[j]) >= minpct)
                .collect();
            order.sort_by(|&a, &b| missing[b].total_cmp(&missing[a]));
            order.truncate(maxvars);
            if order.is_empty() {
                warnings.push(format!(
                    "Variable Summary: no variable has {minpct}% or more missing values"
                ));
            } else {
                let rows = order
                    .iter()
                    .map(|&j| {
                        let v = &vars[j];
                        let mut row = vec![
                            text(&v.name),
                            Cell::Number(missing[j]),
                            Cell::Number(pct(missing[j])),
                            Cell::Number(total - missing[j]),
                        ];
                        if v.categorical {
                            row.extend([Cell::Empty, Cell::Empty]);
                        } else {
                            let pairs: Vec<(f64, f64)> = (0..n)
                                .filter(|&i| counted[i])
                                .filter_map(|i| Some((v.values[i]?, freq[i] * weight[i])))
                                .collect();
                            let [_, mean, sd, _, _] = stats(&pairs);
                            row.extend([num(mean), num(sd)]);
                        }
                        row
                    })
                    .collect();
                tables.push(table(
                    "Variable Summary",
                    1,
                    &[
                        "",
                        "Missing N",
                        "Missing Percent",
                        "Valid N",
                        "Mean",
                        "Std. Deviation",
                    ],
                    rows,
                ));
            }
        }
        if c.patterns {
            tables.push(patterns(&vars, &missing, &counted, &freq, total));
        }
        if !imputing {
            if !c.overall && c.variable_summary.is_none() && !c.patterns {
                warnings.push(
                    "MULTIPLE IMPUTATION did nothing: imputation is off and every missing value \
                     summary is suppressed"
                        .into(),
                );
            }
            for w in warnings {
                self.warn(w);
            }
            return Ok(tables);
        }

        // AUTO keeps to linear models, main effects and the default iterations and tolerance
        let auto = c.method == ImputeMethod::Auto;
        if auto {
            for keyword in ["SCALEMODEL", "INTERACTIONS", "MAXITER", "SINGULAR"] {
                if c.given.contains(&keyword) {
                    warnings.push(format!("{keyword} is ignored with METHOD=AUTO"));
                }
            }
        }
        let (pmm, twoway, maxiter, singular) = match auto {
            true => (None, false, 10, 1e-12),
            false => (c.pmm, c.twoway, c.maxiter, c.singular),
        };

        // constraints, later settings winning keyword by keyword
        let epoch = self.reading().epoch;
        let mut constrained = vec![false; vars.len()];
        for group in &c.constraints {
            let listed = self.expand(&group.variables)?;
            for name in listed {
                let Some(j) = vars.iter().position(|v| v.name.eq_ignore_ascii_case(&name)) else {
                    continue;
                };
                constrained[j] = true;
                let format = data.variable(&name)?.format().to_string();
                let v = &mut vars[j];
                if let Some(min) = &group.min {
                    v.min = min
                        .as_ref()
                        .map(|m| limit(m, &name, &format, epoch))
                        .transpose()?;
                }
                if let Some(max) = &group.max {
                    v.max = max
                        .as_ref()
                        .map(|m| limit(m, &name, &format, epoch))
                        .transpose()?;
                }
                if let Some(rnd) = group.rnd {
                    v.rnd = rnd;
                }
                if let Some(role) = group.role {
                    v.role = role;
                }
            }
        }
        for v in &mut vars {
            if let (Some(min), Some(max)) = (v.min, v.max)
                && min >= max
            {
                return Err(Error(format!(
                    "CONSTRAINTS: the MIN of {} ({min}) must be below its MAX ({max})",
                    v.name
                )));
            }
            if v.categorical {
                // bounds and rounding are for scale values
                (v.min, v.max, v.rnd) = (None, None, None);
            }
            if pmm.is_some() {
                (v.min, v.max) = (None, None);
            }
        }

        // the model: variables within MAXPCTMISSING; the cases with a valid value on one of them
        // a variable without a valid value has nothing to impute from
        let in_model: Vec<bool> = missing
            .iter()
            .map(|&m| m < total && c.maxpctmissing.is_none_or(|limit| pct(m) <= limit))
            .collect();
        for (v, &m) in vars.iter().zip(&missing) {
            if m >= total {
                warnings.push(format!("{} has no valid values and is not imputed", v.name));
            }
        }
        let model: Vec<usize> = (0..vars.len()).filter(|&j| in_model[j]).collect();
        let usable: Vec<bool> = (0..n)
            .map(|i| counted[i] && model.iter().any(|&j| vars[j].values[i].is_some()))
            .collect();
        let has_missing = |j: usize| (0..n).any(|i| usable[i] && vars[j].values[i].is_none());
        let mut order = model.clone();
        if auto {
            order.sort_by(|&a, &b| missing[a].total_cmp(&missing[b]));
        }
        let is_monotone = monotone(&vars, &order, &usable);
        let fcs = match c.method {
            ImputeMethod::Monotone if !is_monotone => {
                return Err(Error(
                    "METHOD=MONOTONE needs a monotone pattern of missing values in the order of \
                     the variable list: where a variable is valid, every variable before it must \
                     be valid too; use METHOD=FCS or reorder the list"
                        .into(),
                ));
            }
            ImputeMethod::Monotone => false,
            ImputeMethod::Fcs => true,
            _ => !is_monotone,
        };
        let predictor = |j: usize| vars[j].role != ImputeRole::Dependent;
        let sequence: Vec<usize> = order.iter().copied().filter(|&j| has_missing(j)).collect();
        let mut plan: Vec<(usize, Vec<usize>, bool, Kind)> = Vec::new();
        for &j in &sequence {
            let preds: Vec<usize> = if fcs {
                order
                    .iter()
                    .copied()
                    .filter(|&p| p != j && predictor(p))
                    .collect()
            } else {
                let at = order.iter().position(|&p| p == j).expect("in order");
                order[..at]
                    .iter()
                    .copied()
                    .filter(|&p| predictor(p))
                    .collect()
            };
            let pairwise = twoway && vars[j].role != ImputeRole::Predictor;
            let observed: Vec<f64> = (0..n)
                .filter(|&i| usable[i])
                .filter_map(|i| vars[j].values[i])
                .collect();
            let constant = observed.iter().all(|v| *v == observed[0]);
            let kind = match (constant, vars[j].categorical, pmm) {
                (true, _, _) => Kind::Constant,
                (false, true, _) => Kind::Logistic,
                (false, false, Some(k)) => Kind::Pmm(k),
                (false, false, None) => Kind::Linear,
            };
            let width = width(&vars, &preds, pairwise);
            let params = match kind {
                Kind::Logistic => width * (vars[j].levels.len() - 1),
                _ => width,
            };
            if params > c.maxmodelparam {
                return Err(Error(format!(
                    "the imputation model of {} has {params} parameters, more than \
                     MAXMODELPARAM={}; nothing was imputed",
                    vars[j].name, c.maxmodelparam
                )));
            }
            plan.push((j, preds, pairwise, kind));
        }
        if sequence.is_empty() {
            warnings.push("no analysis variable has missing values to impute".into());
        }

        // the imputations
        let ctx = Ctx {
            vars: &vars,
            freq: &freq,
            weight: &weight,
            usable: &usable,
            singular,
            case_draws: c.maxcasedraws,
            param_draws: c.maxparamdraws,
        };
        let observed: Vec<Vec<f64>> = vars
            .iter()
            .map(|v| v.values.iter().map(|x| x.unwrap_or(f64::NAN)).collect())
            .collect();
        let rounds = if fcs { maxiter } else { 1 };
        let mut completed: Vec<Vec<Vec<f64>>> = Vec::new();
        // per imputation and round: the mean and SD of the imputed values of each scale variable
        let mut history: Vec<Trace> = Vec::new();
        let traced: Vec<usize> = sequence
            .iter()
            .copied()
            .filter(|&j| !vars[j].categorical)
            .collect();
        {
            let mut rng = self.random.borrow_mut();
            for l in 1..=c.nimputations {
                let mut cur = observed.clone();
                if fcs {
                    for &j in &sequence {
                        ctx.start(j, &mut cur, &mut rng);
                    }
                }
                for round in 1..=rounds {
                    for (j, preds, pairwise, kind) in &plan {
                        for (case, v) in ctx.impute(*j, preds, *pairwise, *kind, &cur, &mut rng)? {
                            cur[*j][case] = v;
                        }
                    }
                    if fcs {
                        let row = traced
                            .iter()
                            .map(|&j| {
                                let (_, mis) = ctx.cases(j);
                                let pairs: Vec<(f64, f64)> = mis
                                    .iter()
                                    .map(|&i| (cur[j][i], freq[i] * weight[i]))
                                    .collect();
                                let [_, mean, sd, _, _] = stats(&pairs);
                                [mean, sd]
                            })
                            .collect();
                        history.push((l, round, row));
                    }
                }
                completed.push(cur);
            }
        }

        // tables of the imputation
        let method_name = |fcs: bool| match fcs {
            true => "Fully Conditional Specification",
            false => "Monotone",
        };
        let mut spec = vec![
            vec![
                text("Imputation Method"),
                text(match c.method {
                    ImputeMethod::Auto => "Automatic",
                    _ => method_name(fcs),
                }),
            ],
            vec![
                text("Number of Imputations"),
                Cell::Number(c.nimputations as f64),
            ],
            vec![
                text("Model for Scale Variables"),
                text(match pmm {
                    Some(k) => format!("Predictive Mean Matching ({k} nearest)"),
                    None => "Linear Regression".into(),
                }),
            ],
            vec![
                text("Interactions Included in Models"),
                text(if twoway { "Two-way" } else { "None" }),
            ],
            vec![
                text("Maximum Percentage of Missing Values"),
                text(match c.maxpctmissing {
                    Some(p) => format!("{p}%"),
                    None => "100.0%".into(),
                }),
            ],
            vec![
                text("Maximum Parameters in Imputation Model"),
                Cell::Number(c.maxmodelparam as f64),
            ],
            vec![
                text("Analysis Weight"),
                text(c.analysis_weight.clone().unwrap_or_else(|| "None".into())),
            ],
        ];
        if fcs && !auto {
            spec.push(vec![
                text("Maximum Iterations"),
                Cell::Number(maxiter as f64),
            ]);
        }
        tables.push(table("Imputation Specifications", 1, &["", "Value"], spec));
        if constrained.contains(&true) {
            let rows = (0..vars.len())
                .filter(|&j| constrained[j])
                .map(|j| {
                    let v = &vars[j];
                    vec![
                        text(&v.name),
                        text(role_name(v.role)),
                        num(v.min),
                        num(v.max),
                        num(v.rnd),
                    ]
                })
                .collect();
            tables.push(table(
                "Imputation Constraints",
                1,
                &["", "Role", "Minimum", "Maximum", "Rounding"],
                rows,
            ));
        }
        let list = |js: &mut dyn Iterator<Item = usize>| {
            js.map(|j| vars[j].name.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let mut results = vec![vec![text("Imputation Method"), text(method_name(fcs))]];
        if fcs {
            results.push(vec![
                text("Fully Conditional Specification Iterations"),
                Cell::Number(rounds as f64),
            ]);
        }
        let imputed_shown = list(
            &mut sequence
                .iter()
                .copied()
                .filter(|&j| vars[j].role != ImputeRole::Predictor),
        );
        results.push(vec![
            text("Dependent Variables Imputed"),
            text(imputed_shown),
        ]);
        let dropped = list(&mut (0..vars.len()).filter(|&j| !in_model[j]));
        if !dropped.is_empty() {
            results.push(vec![
                text("Not Imputed (Too Many Missing Values)"),
                text(dropped),
            ]);
        }
        let complete = list(
            &mut model
                .iter()
                .copied()
                .filter(|&j| !has_missing(j) && vars[j].role != ImputeRole::Predictor),
        );
        if !complete.is_empty() {
            results.push(vec![
                text("Not Imputed (No Missing Values)"),
                text(complete),
            ]);
        }
        results.push(vec![
            text("Imputation Sequence"),
            text(list(&mut order.iter().copied())),
        ]);
        tables.push(table("Imputation Results", 1, &["", "Value"], results));
        if c.models && !plan.is_empty() {
            let rows = plan
                .iter()
                .map(|(j, preds, pairwise, kind)| {
                    let mut effects: Vec<String> =
                        preds.iter().map(|&p| vars[p].name.clone()).collect();
                    if *pairwise {
                        let cats: Vec<&str> = preds
                            .iter()
                            .filter(|&&p| vars[p].categorical)
                            .map(|&p| vars[p].name.as_str())
                            .collect();
                        for a in 0..cats.len() {
                            for b in &cats[a + 1..] {
                                effects.push(format!("{}*{b}", cats[a]));
                            }
                        }
                    }
                    let missing_cases = ctx.cases(*j).1.len();
                    vec![
                        text(&vars[*j].name),
                        text(match kind {
                            Kind::Linear => "Linear Regression".to_string(),
                            Kind::Pmm(k) => format!("Predictive Mean Matching ({k})"),
                            Kind::Logistic => "Logistic Regression".into(),
                            Kind::Constant => "Constant".into(),
                        }),
                        text(if effects.is_empty() {
                            "(constant only)".into()
                        } else {
                            effects.join(" ")
                        }),
                        Cell::Number(missing_cases as f64),
                        Cell::Number((missing_cases * c.nimputations) as f64),
                    ]
                })
                .collect();
            tables.push(table(
                "Imputation Models",
                1,
                &[
                    "",
                    "Model Type",
                    "Effects",
                    "Missing Values",
                    "Imputed Values",
                ],
                rows,
            ));
        }
        if let Some(items) = &c.descriptives {
            let wanted: Vec<String> = match items.is_empty() {
                true => Vec::new(),
                false => self.expand(items)?,
            };
            let shown = self.shown();
            for &j in &sequence {
                let v = &vars[j];
                let listed =
                    wanted.is_empty() || wanted.iter().any(|w| w.eq_ignore_ascii_case(&v.name));
                if v.role == ImputeRole::Predictor || !listed {
                    continue;
                }
                let (_, mis) = ctx.cases(j);
                let variable = data.variable(&v.name)?;
                tables.push(descriptives(
                    v,
                    j,
                    &variable,
                    &shown,
                    &observed[j],
                    &completed,
                    &mis,
                    &counted,
                    &freq,
                    &weight,
                ));
            }
        }

        // the output datasets
        let saved: Vec<usize> = sequence
            .iter()
            .copied()
            .filter(|&j| vars[j].role != ImputeRole::Predictor)
            .collect();
        let mut parts = vec![data.clone()];
        for cur in &completed {
            let mut part = data.clone();
            for &j in &saved {
                let v = &vars[j];
                let original = data.variable(&v.name)?;
                match &v.levels.first() {
                    Some(Key::Text(_)) => {
                        let old = original.string()?;
                        let values = (0..n)
                            .map(|i| match (cur[j][i], v.values[i]) {
                                (x, None) if x.is_finite() => match &v.levels[x as usize] {
                                    Key::Text(t) => t.clone(),
                                    Key::Number(_) => unreachable!("string levels"),
                                },
                                _ => old.get(i).to_string(),
                            })
                            .collect();
                        let width = original.string_width().expect("a string");
                        part.set_string_values(&v.name, width, values)?;
                    }
                    _ => {
                        let old = original.numeric()?;
                        let values = (0..n)
                            .map(|i| match (cur[j][i], v.values[i]) {
                                (x, None) if x.is_finite() => {
                                    Some(match v.levels.get(x as usize) {
                                        Some(Key::Number(level)) if v.categorical => *level,
                                        _ => x,
                                    })
                                }
                                _ => old.get(i),
                            })
                            .collect();
                        part.set_numeric_values(&v.name, values)?;
                    }
                }
            }
            parts.push(part);
        }
        let mut stacked = Vec::new();
        for (l, mut part) in parts.into_iter().enumerate() {
            part.set_numeric_values(IMPUTATION, vec![Some(l as f64); n])?;
            let mut order: Vec<String> = vec![IMPUTATION.to_string()];
            order.extend(data.variable_names().map(str::to_string));
            let refs: Vec<&str> = order.iter().map(String::as_str).collect();
            part.keep_variables(&refs)?;
            part.set_format(IMPUTATION, "F8.0")?;
            part.set_measure(IMPUTATION, Measure::Nominal)?;
            part.set_variable_label(IMPUTATION, Some("Imputation Number"))?;
            part.set_value_labels(
                IMPUTATION,
                vec![(DataValue::Number(0.0), "Original data".into())],
            )?;
            stacked.push(part);
        }
        let iterations = match (&c.fcs_iterations, fcs) {
            (None, _) => None,
            (Some(_), false) => {
                warnings
                    .push("FCSITERATIONS is ignored: the monotone method has no iterations".into());
                None
            }
            (Some(_), true) if traced.is_empty() => {
                warnings.push("FCSITERATIONS is not written: no scale variable is imputed".into());
                None
            }
            (Some(_), true) => {
                let split: Vec<String> = self
                    .split_group
                    .iter()
                    .flatten()
                    .map(|(name, _)| name.clone())
                    .collect();
                Some(iteration_data(&vars, &traced, &history, data, &split)?)
            }
        };
        self.imputed.push((stacked, iterations));
        for w in warnings {
            self.warn(w);
        }
        Ok(tables)
    }

    /// After MULTIPLE IMPUTATION ran in every split group: stacks the groups' imputations,
    /// imputation by imputation, and writes the results.
    pub(crate) fn write_imputations(
        &mut self,
        c: &MultipleImputation,
        split: Option<SplitFile>,
    ) -> Result<(), Error> {
        let groups = std::mem::take(&mut self.imputed);
        let (Some(file), Some(first)) = (&c.imputations, groups.first()) else {
            return Ok(());
        };
        let template = first.0[0].clone();
        let mut pieces = Vec::new();
        for l in 0..=c.nimputations {
            pieces.extend(groups.iter().map(|(parts, _)| parts[l].clone()));
        }
        let mut out = Dataset::from_parts(&template, &pieces[1..])?;
        let mut by = vec![IMPUTATION.to_string()];
        by.extend(split.into_iter().flat_map(|s| s.variables));
        out.set_split_file(Some(SplitFile {
            variables: by,
            layered: true,
        }))?;
        out.set_filter_variable(None)?;
        self.write_data_file(file, out)?;
        if let Some(file) = &c.fcs_iterations {
            let traces: Vec<&Dataset> = groups.iter().filter_map(|(_, t)| t.as_ref()).collect();
            if let Some((first, rest)) = traces.split_first() {
                let rest: Vec<Dataset> = rest.iter().map(|d| (*d).clone()).collect();
                self.write_data_file(file, Dataset::from_parts(first, &rest)?)?;
            }
        }
        Ok(())
    }
}

/// The variables in order of their missing values, fewest first; each pattern of missing
/// values with its replicated count, ordered from the last variable back to the first, valid
/// before missing.
fn patterns(vars: &[Var], missing: &[f64], counted: &[bool], freq: &[f64], total: f64) -> Table {
    let mut order: Vec<usize> = (0..vars.len()).collect();
    order.sort_by(|&a, &b| missing[a].total_cmp(&missing[b]));
    let mut found: Vec<(Vec<bool>, f64)> = Vec::new();
    for i in (0..counted.len()).filter(|&i| counted[i]) {
        let key: Vec<bool> = order.iter().map(|&j| vars[j].values[i].is_none()).collect();
        match found.iter_mut().find(|(k, _)| *k == key) {
            Some((_, count)) => *count += freq[i],
            None => found.push((key, freq[i])),
        }
    }
    found.sort_by(|a, b| a.0.iter().rev().cmp(b.0.iter().rev()));
    let mut columns: Vec<&str> = vec!["Pattern"];
    columns.extend(order.iter().map(|&j| vars[j].name.as_str()));
    columns.extend(["N", "Percent"]);
    let rows = found
        .iter()
        .enumerate()
        .map(|(p, (key, count))| {
            let mut row = vec![Cell::Number((p + 1) as f64)];
            row.extend(key.iter().map(|&m| if m { text("X") } else { Cell::Empty }));
            row.push(Cell::Number(*count));
            row.push(Cell::Number(if total > 0.0 {
                100.0 * count / total
            } else {
                0.0
            }));
            row
        })
        .collect();
    let mut t = table("Missing Value Patterns", 1, &columns, rows);
    t.caption =
        Some("X marks a missing value. Variables run from fewest to most missing values.".into());
    t
}

/// IMPUTATIONSUMMARIES DESCRIPTIVES of imputed variable `j`: its original values, the imputed
/// ones and the completed data of every imputation.
#[allow(clippy::too_many_arguments)]
fn descriptives(
    v: &Var,
    j: usize,
    variable: &statscraft_data::Variable,
    shown: &crate::shown::Shown,
    observed: &[f64],
    completed: &[Vec<Vec<f64>>],
    mis: &[usize],
    counted: &[bool],
    freq: &[f64],
    weight: &[f64],
) -> Table {
    let n = observed.len();
    let pairs = |values: &[f64], cases: &mut dyn Iterator<Item = usize>| -> Vec<(f64, f64)> {
        cases
            .filter(|&i| values[i].is_finite())
            .map(|i| (values[i], freq[i] * weight[i]))
            .collect()
    };
    let all = || (0..n).filter(|&i| counted[i]);
    let mut groups: Vec<Summary> =
        vec![("Original Data", Cell::Empty, pairs(observed, &mut all()))];
    for (l, cur) in completed.iter().enumerate() {
        let imputed = pairs(&cur[j], &mut mis.iter().copied());
        groups.push(("Imputed Values", Cell::Number((l + 1) as f64), imputed));
    }
    for (l, cur) in completed.iter().enumerate() {
        let whole = pairs(&cur[j], &mut all());
        groups.push((
            "Complete Data After Imputation",
            Cell::Number((l + 1) as f64),
            whole,
        ));
    }
    let title = format!("{}: Descriptive Statistics", v.name);
    if !v.categorical {
        let rows = groups
            .into_iter()
            .map(|(label, imp, p)| {
                let mut row = vec![text(label), imp];
                row.extend(stats(&p).into_iter().map(num));
                row
            })
            .collect();
        return table(
            &title,
            2,
            &[
                "Data",
                "Imputation",
                "N",
                "Mean",
                "Std. Deviation",
                "Minimum",
                "Maximum",
            ],
            rows,
        );
    }
    let mut rows = Vec::new();
    for (label, imp, p) in groups {
        let total: f64 = p.iter().map(|(_, w)| w).sum::<f64>() + 0.0;
        for (k, key) in v.levels.iter().enumerate() {
            let count: f64 = p
                .iter()
                .filter(|(x, _)| *x as usize == k)
                .map(|(_, w)| w)
                .fold(0.0, |a, b| a + b);
            rows.push(vec![
                text(label),
                imp.clone(),
                text(shown.value(variable, key)),
                Cell::Number(count),
                num((total > 0.0).then(|| 100.0 * count / total)),
            ]);
        }
    }
    table(
        &title,
        3,
        &["Data", "Imputation", "Category", "N", "Percent"],
        rows,
    )
}

/// FCSITERATIONS: per imputation and round, a row of means and a row of standard deviations
/// of the imputed values of every scale variable; with a split, the group's split values.
fn iteration_data(
    vars: &[Var],
    traced: &[usize],
    history: &[Trace],
    data: &Dataset,
    split: &[String],
) -> Result<Dataset, Error> {
    let rows = history.len() * 2;
    let mut out = Dataset::new(rows);
    let repeat = |f: &dyn Fn(&Trace) -> Option<f64>| -> Vec<Option<f64>> {
        history
            .iter()
            .flat_map(|trace| [f(trace), f(trace)])
            .collect()
    };
    out.set_numeric_values(IMPUTATION, repeat(&|(l, _, _)| Some(*l as f64)))?;
    out.set_numeric_values("Iteration_", repeat(&|(_, t, _)| Some(*t as f64)))?;
    let kinds: Vec<Option<f64>> = (0..rows).map(|r| Some((r % 2 + 1) as f64)).collect();
    out.set_numeric_values("SummaryStatistic_", kinds)?;
    for name in [IMPUTATION, "Iteration_", "SummaryStatistic_"] {
        out.set_format(name, "F8.0")?;
        out.set_measure(name, Measure::Nominal)?;
    }
    out.set_value_labels(
        "SummaryStatistic_",
        vec![
            (DataValue::Number(1.0), "Mean".into()),
            (DataValue::Number(2.0), "Std. Deviation".into()),
        ],
    )?;
    let first = vec![Some(0); rows];
    for name in split {
        out.copy_variable(data, name, &first)?;
    }
    for (k, &j) in traced.iter().enumerate() {
        let values = history
            .iter()
            .flat_map(|(_, _, h)| [h[k][0], h[k][1]])
            .collect();
        out.set_numeric_values(&vars[j].name, values)?;
        let format = data.variable(&vars[j].name)?.format().to_string();
        out.set_format(&vars[j].name, &format)?;
        out.set_measure(&vars[j].name, Measure::Scale)?;
    }
    Ok(out)
}
