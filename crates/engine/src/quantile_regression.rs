//! QUANTILE REGRESSION: linear models of a conditional quantile, fitted by linear programming.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (QUANTILE REGRESSION), in
//! our words:
//! - The estimate at quantile p minimizes the sum over cases of w_i rho_p(y_i - x_i'b), where
//!   rho_p(r) is p r above zero and (p - 1) r below, and w_i is the frequency (the case weight
//!   rounded, used when 1 or more) times the regression weight (REGWGT, used when above 0). A case
//!   missing on any variable is left out; CLASSMISSING=INCLUDE keeps user-missing
//!   factor values. A quantile of 0 or 1 is fitted as 1E-10 or 1 - 1E-10.
//! - The design is the one of UNIANOVA: the intercept and the main effects by default, a column
//!   that depends on earlier ones is redundant, its estimate 0 and its statistics empty.
//! - SIMPLEX moves between bases of k cases that the fitted plane passes through, along the edge
//!   that lowers the objective most, as far as the objective keeps falling (the steps of
//!   Barrodale and Roberts). INTERIOR_POINT is the Frisch-Newton predictor-corrector method on
//!   the dual problem; from its end point the simplex finishes at the exact vertex. AUTO takes
//!   the simplex up to 1000 cases.
//! - With IID the covariance is p(1-p) s^2 (X'WX)^-1, the sparsity s the difference of the
//!   residuals' order statistics at ceil(N(p+h)) and ceil(N(p-h)) over 2h, h by Bofinger or
//!   Hall-Sheather. Without IID the model is fitted at p+h and p-h, each case gets the density
//!   2h / x_i'(b+ - b-) (0 when not positive), H = X'WFX and the covariance is
//!   p(1-p) H^-1 X'WX H^-1. t tests and intervals use N - k degrees of freedom.
//! - The pseudo R square is 1 - V/V0, V0 the objective of the intercept alone; the mean absolute
//!   error is sum(w |e|) / N. Intervals of predicted values are x'b +- z sqrt(p(1-p) x'(X'WX)^-1 x)
//!   times the sparsity (IID) or 1 / the density at x (otherwise).

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::dist::{normal_quantile, t_p, t_quantile};
use crate::glm::{
    Data, Effect, Fit, default_terms, effects_of, emmeans_l, ginverse, intercept_term,
    key_value_text, parameter_name,
};
use crate::matrix::{Matrix, inverse};
use crate::npar_tests::text;
use crate::plot::{Plot, Point, PointGroup};
use crate::regression::{plain, table};
use crate::shown::Shown;
use crate::{Cell, Chart, Engine, Error, Table};
use serde_json::json;
use statscraft_data::{Dataset, Measure, Values, Variable};
use statscraft_syntax::{QrMethod, QrSave, QuantileRegression, Term};

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn rho(r: f64, p: f64) -> f64 {
    if r >= 0.0 { p * r } else { (p - 1.0) * r }
}

/// The rate of change of rho_p(r + t a) just after t = 0; a residual within `zero` of 0 counts
/// as 0 and moves to the side `a` takes it.
fn slope(r: f64, a: f64, p: f64, zero: f64) -> f64 {
    if r > zero || (r.abs() <= zero && a > 0.0) {
        p * a
    } else {
        (p - 1.0) * a
    }
}

/// The first k rows, taken in `order`, that are linearly independent.
fn independent_rows(x: &[Vec<f64>], order: impl Iterator<Item = usize>) -> Option<Vec<usize>> {
    let k = x.first()?.len();
    let mut basis: Vec<Vec<f64>> = Vec::new();
    let mut rows = Vec::new();
    for i in order {
        let mut v = x[i].clone();
        let size = dot(&v, &v).sqrt();
        // twice, so that the rounding of the first pass does not let a dependent row in
        for _ in 0..2 {
            for q in &basis {
                let c = dot(&v, q);
                v.iter_mut().zip(q).for_each(|(a, b)| *a -= c * b);
            }
        }
        let rest = dot(&v, &v).sqrt();
        if size > 0.0 && rest > 1e-9 * size {
            basis.push(v.iter().map(|a| a / rest).collect());
            rows.push(i);
            if rows.len() == k {
                return Some(rows);
            }
        }
    }
    None
}

/// The exact minimizer of sum rho_p(y_i - x_i'b) (rows already weighted) from the basis
/// `start`: a vertex where no edge lowers the objective.
fn simplex(x: &[Vec<f64>], y: &[f64], p: f64, start: Vec<usize>) -> Option<(Vec<f64>, Vec<usize>)> {
    let (n, k) = (y.len(), start.len());
    let mut h = start;
    let mut in_basis = vec![false; n];
    h.iter().for_each(|&i| in_basis[i] = true);
    let mut c = vec![vec![0.0; n]; k];
    // every step lowers the objective, so no basis comes back; the cap only guards rounding
    for _ in 0..100 * (n + k) + 1000 {
        let b: Matrix = h.iter().map(|&i| x[i].clone()).collect();
        let (binv, _) = inverse(&b)?;
        let yh: Vec<f64> = h.iter().map(|&i| y[i]).collect();
        let beta: Vec<f64> = binv.iter().map(|row| dot(row, &yh)).collect();
        let mut r = vec![0.0; n];
        let mut zero = vec![0.0; n];
        for i in (0..n).filter(|&i| !in_basis[i]) {
            r[i] = y[i] - dot(&x[i], &beta);
            let size: f64 = x[i].iter().zip(&beta).map(|(a, b)| (a * b).abs()).sum();
            zero[i] = 1e-12 * (y[i].abs() + size) + 1e-300;
            for (j, column) in c.iter_mut().enumerate() {
                column[i] = (0..k).map(|l| x[i][l] * binv[l][j]).sum();
            }
        }
        // the edge where basis case j leaves the plane upward (s = 1: its residual falls
        // below 0) or downward, along which the objective falls fastest
        let mut best: Option<(f64, usize, f64)> = None;
        for (j, column) in c.iter().enumerate() {
            let (mut up, mut down, mut size) = (1.0 - p, p, 1.0);
            for i in (0..n).filter(|&i| !in_basis[i]) {
                up += slope(r[i], -column[i], p, zero[i]);
                down += slope(r[i], column[i], p, zero[i]);
                size += column[i].abs();
            }
            for (g, s) in [(up, 1.0), (down, -1.0)] {
                if g < -1e-11 * size && best.is_none_or(|(b, _, _)| g < b) {
                    best = Some((g, j, s));
                }
            }
        }
        let Some((g, j, s)) = best else {
            return Some((beta, h));
        };
        // along the edge the objective is convex and piecewise linear: walk its breaks until
        // the slope turns up; the case at that break enters the basis
        let mut breaks: Vec<(f64, f64, usize)> = (0..n)
            .filter(|&i| !in_basis[i])
            .filter_map(|i| {
                let a = -s * c[j][i];
                let toward = (r[i] > zero[i] && a < 0.0) || (r[i] < -zero[i] && a > 0.0);
                toward.then(|| (-r[i] / a, a.abs(), i))
            })
            .collect();
        breaks.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut rate = g;
        let enter = breaks.iter().find_map(|&(_, w, i)| {
            rate += w;
            (rate >= 0.0).then_some(i)
        })?;
        in_basis[h[j]] = false;
        h[j] = enter;
        in_basis[enter] = true;
    }
    None
}

/// The simplex on y moved by tiny distinct amounts, so that no more than k cases lie on a
/// fitted plane (where the edges of one basis would not show every way down), then once more
/// on y itself from the basis found: its vertex is optimal for y as well.
fn solve_lp(x: &[Vec<f64>], y: &[f64], p: f64, start: Vec<usize>) -> Option<Vec<f64>> {
    let scale = y.iter().map(|v| v.abs()).fold(0.0, f64::max);
    // a hash of the case number (splitmix64), not a smooth sequence that a design column of
    // case numbers could follow
    let noise = |i: usize| {
        let mut z = (i as u64)
            .wrapping_add(1)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
    };
    let moved: Vec<f64> = y
        .iter()
        .enumerate()
        .map(|(i, v)| v + 1e-8 * scale * noise(i))
        .collect();
    let (_, basis) = simplex(x, &moved, p, start)?;
    simplex(x, y, p, basis).map(|(b, _)| b)
}

/// Least squares of the weighted rows: X'X b = X'y.
fn least_squares(x: &[Vec<f64>], y: &[f64]) -> Vec<f64> {
    let k = x.first().map_or(0, Vec::len);
    let mut xx = vec![vec![0.0; k]; k];
    let mut xy = vec![0.0; k];
    for (row, &v) in x.iter().zip(y) {
        for a in 0..k {
            xy[a] += row[a] * v;
            for b in 0..k {
                xx[a][b] += row[a] * row[b];
            }
        }
    }
    let (g, _) = ginverse(&xx, 1e-12);
    g.iter().map(|row| dot(row, &xy)).collect()
}

/// A Newton direction: the changes of b, a, u and v (s changes by minus that of a).
type NewtonStep = (Vec<f64>, Vec<f64>, Vec<f64>, Vec<f64>);

/// The Frisch-Newton interior-point method on the dual problem (the Algorithms manual's
/// steps), with y scaled to at most 1 in size. Returns the coefficients and whether the duality
/// gap fell below the criterion within `maxiter` iterations.
fn interior_point(
    x: &[Vec<f64>],
    y: &[f64],
    p: f64,
    tol: f64,
    conv: f64,
    maxiter: u32,
) -> (Vec<f64>, bool) {
    let (n, k) = (y.len(), x[0].len());
    let scale = y.iter().map(|v| v.abs()).fold(0.0, f64::max).max(1e-300);
    let y: Vec<f64> = y.iter().map(|v| v / scale).collect();
    let eps = conv.min(p.min(1.0 - p) / 10.0);
    let mut beta = least_squares(x, &y);
    let r: Vec<f64> = (0..n).map(|i| y[i] - dot(&x[i], &beta)).collect();
    // a small shift keeps u and v inside the positive orthant; it leaves u - v unchanged
    let shift = 1e-3 * r.iter().map(|v| v.abs()).sum::<f64>() / n as f64 + 1e-8;
    let mut u: Vec<f64> = r.iter().map(|v| v.max(0.0) + shift).collect();
    let mut v: Vec<f64> = r.iter().map(|v| (-v).max(0.0) + shift).collect();
    let mut a = vec![1.0 - p; n];
    let mut s = vec![p; n];
    let length = |value: &[f64], delta: &[f64], other: &[f64], d_other: &[f64]| {
        let mut m = f64::INFINITY;
        for (x, d) in value.iter().zip(delta).chain(other.iter().zip(d_other)) {
            if *d < 0.0 {
                m = m.min(-x / d);
            }
        }
        (0.99995 * m).min(1.0)
    };
    // one Newton direction for the given xi; returns (db, da, du, dv) with ds = -da
    let direction = |a: &[f64],
                     s: &[f64],
                     u: &[f64],
                     v: &[f64],
                     xi: &[f64],
                     extra_u: &dyn Fn(usize) -> f64,
                     extra_v: &dyn Fn(usize) -> f64|
     -> Option<NewtonStep> {
        let g: Vec<f64> = (0..n).map(|i| 1.0 / (u[i] / s[i] + v[i] / a[i])).collect();
        let mut m = vec![vec![0.0; k]; k];
        let mut rhs = vec![0.0; k];
        for i in 0..n {
            for c in 0..k {
                rhs[c] += x[i][c] * (a[i] - (1.0 - p) + g[i] * xi[i]);
                for d in 0..k {
                    m[c][d] += g[i] * x[i][c] * x[i][d];
                }
            }
        }
        let (inv, rank) = ginverse(&m, tol);
        if rank < k {
            return None;
        }
        let db: Vec<f64> = inv.iter().map(|row| dot(row, &rhs)).collect();
        let da: Vec<f64> = (0..n).map(|i| g[i] * (xi[i] - dot(&x[i], &db))).collect();
        let du: Vec<f64> = (0..n)
            .map(|i| -u[i] + u[i] / s[i] * da[i] + extra_u(i))
            .collect();
        let dv: Vec<f64> = (0..n)
            .map(|i| -v[i] - v[i] / a[i] * da[i] + extra_v(i))
            .collect();
        Some((db, da, du, dv))
    };
    let mut converged = false;
    for _ in 0..maxiter {
        let gap = dot(&a, &v) + dot(&s, &u);
        if gap < eps {
            converged = true;
            break;
        }
        let xi: Vec<f64> = (0..n).map(|i| y[i] - dot(&x[i], &beta)).collect();
        let none = |_: usize| 0.0;
        let Some((mut db, mut da, mut du, mut dv)) = direction(&a, &s, &u, &v, &xi, &none, &none)
        else {
            break;
        };
        let ds: Vec<f64> = da.iter().map(|d| -d).collect();
        let mut gp = length(&a, &da, &s, &ds);
        let mut gd = length(&u, &du, &v, &dv);
        if gp < 1.0 || gd < 1.0 {
            let g0 = gap;
            let g1: f64 = (0..n)
                .map(|i| {
                    (s[i] + gp * ds[i]) * (u[i] + gd * du[i])
                        + (a[i] + gp * da[i]) * (v[i] + gd * dv[i])
                })
                .sum();
            let mu = (g1 / g0).powi(3) * g0 / (2.0 * n as f64);
            let xi: Vec<f64> = (0..n)
                .map(|i| {
                    xi[i] + mu * (1.0 / a[i] - 1.0 / s[i]) - da[i] * dv[i] / a[i]
                        + ds[i] * du[i] / s[i]
                })
                .collect();
            let (pa, ps, pu, pv) = (da.clone(), ds.clone(), du.clone(), dv.clone());
            let extra_u = |i: usize| mu / s[i] - ps[i] * pu[i] / s[i];
            let extra_v = |i: usize| mu / a[i] - pa[i] * pv[i] / a[i];
            let Some(next) = direction(&a, &s, &u, &v, &xi, &extra_u, &extra_v) else {
                break;
            };
            (db, da, du, dv) = next;
            let ds: Vec<f64> = da.iter().map(|d| -d).collect();
            gp = length(&a, &da, &s, &ds);
            gd = length(&u, &du, &v, &dv);
        }
        for i in 0..n {
            a[i] += gp * da[i];
            s[i] -= gp * da[i];
            u[i] += gd * du[i];
            v[i] += gd * dv[i];
        }
        beta.iter_mut().zip(&db).for_each(|(b, d)| *b += gd * d);
    }
    (beta.iter().map(|b| b * scale).collect(), converged)
}

/// How the coefficients of the standardised design map back: beta = T beta*.
struct Back {
    /// Each column's divisor.
    spread: Vec<f64>,
    /// Each column's centre (0 unless the model has an intercept) and the intercept's column.
    centre: Vec<f64>,
    intercept: Option<usize>,
}

impl Back {
    fn apply(&self, b: &[f64]) -> Vec<f64> {
        let mut out: Vec<f64> = b.iter().zip(&self.spread).map(|(v, s)| v / s).collect();
        if let Some(c) = self.intercept {
            let shift: f64 = (0..b.len())
                .filter(|&j| j != c)
                .map(|j| self.centre[j] * out[j])
                .sum();
            out[c] = b[c] / self.spread[c] - shift;
        }
        out
    }

    fn matrix(&self) -> Matrix {
        let q = self.spread.len();
        let mut t = vec![vec![0.0; q]; q];
        for j in 0..q {
            t[j][j] = 1.0 / self.spread[j];
        }
        if let Some(c) = self.intercept {
            for j in (0..q).filter(|&j| j != c) {
                t[c][j] = -self.centre[j] / self.spread[j];
            }
        }
        t
    }

    fn cov(&self, c: &Matrix) -> Matrix {
        let t = self.matrix();
        let tt: Matrix = (0..t.len())
            .map(|i| (0..t.len()).map(|j| t[j][i]).collect())
            .collect();
        crate::matrix::multiply(&crate::matrix::multiply(&t, c), &tt)
    }
}

/// The design with every column but the intercept centred (when there is one) and every column
/// divided by its weighted spread.
fn standardised(x: Vec<Vec<f64>>, w: &[f64]) -> (Vec<Vec<f64>>, Back) {
    let k = x.first().map_or(0, Vec::len);
    let intercept = (0..k).find(|&j| x.iter().all(|row| row[j] == 1.0));
    let total: f64 = w.iter().sum();
    let mut centre = vec![0.0; k];
    let mut spread = vec![1.0; k];
    for j in 0..k {
        if Some(j) == intercept || total <= 0.0 {
            continue;
        }
        if intercept.is_some() {
            centre[j] = x.iter().zip(w).map(|(r, wi)| wi * r[j]).sum::<f64>() / total;
        }
        let ms = x
            .iter()
            .zip(w)
            .map(|(r, wi)| wi * (r[j] - centre[j]).powi(2))
            .sum::<f64>()
            / total;
        if ms > 0.0 && ms.is_finite() {
            spread[j] = ms.sqrt();
        }
    }
    // shortcut: only a design whose columns differ in size by more than 1E4 or lie farther from
    // 0 than 1E4 spreads is standardised; on any other the simplex keeps the vertex it picks on
    // the columns as given when the optimum is not unique
    let varying: Vec<f64> = (0..k)
        .filter(|&j| Some(j) != intercept)
        .map(|j| spread[j])
        .collect();
    let widest = varying.iter().copied().fold(0.0, f64::max);
    let narrowest = varying.iter().copied().fold(f64::INFINITY, f64::min);
    let far = (0..k).any(|j| centre[j].abs() > 1e4 * spread[j]);
    if varying.is_empty() || (widest <= 1e4 * narrowest && !far) {
        return (
            x,
            Back {
                spread: vec![1.0; k],
                centre: vec![0.0; k],
                intercept,
            },
        );
    }
    let scaled = x
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(j, v)| (v - centre[j]) / spread[j])
                .collect()
        })
        .collect();
    (
        scaled,
        Back {
            spread,
            centre,
            intercept,
        },
    )
}

fn bandwidth(p: f64, n: f64, hall_sheather: bool, cilevel: f64) -> f64 {
    let z = normal_quantile(p);
    let phi = (-0.5 * z * z).exp() / (2.0 * std::f64::consts::PI).sqrt();
    if hall_sheather {
        let za = normal_quantile(0.5 + cilevel / 200.0);
        n.powf(-1.0 / 3.0)
            * za.powf(2.0 / 3.0)
            * (1.5 * phi * phi / (2.0 * z * z + 1.0)).powf(1.0 / 3.0)
    } else {
        n.powf(-0.2) * (4.5 * phi.powi(4) / (2.0 * z * z + 1.0).powi(2)).powf(0.2)
    }
}

/// The quantile suffix of names: the digits after the decimal point, at least two.
fn suffix(p: f64) -> String {
    if p >= 1.0 {
        return "100".into();
    }
    let text = format!("{p}");
    let digits = text.split_once('.').map_or("", |(_, d)| d);
    format!("{digits:0<2}")
}

/// One quantile's fit: the coefficients over the estimated columns, the objective, the
/// covariance (None when it cannot be estimated) and what intervals of predictions need.
struct QuantileFit {
    p: f64,
    b: Vec<f64>,
    objective: f64,
    covariance: Option<Matrix>,
    /// IID: the sparsity; otherwise the coefficients at the upper and lower quantile.
    sparsity: Option<f64>,
    spread: Option<(Vec<f64>, Vec<f64>, f64)>,
    exact: bool,
}

/// What the effect of PREDICTION_TABLES or PREDICTION_LINES shows.
enum PredictionKind {
    Table,
    Line(usize),
    Mixed(usize),
}

fn kind_of(e: &Effect) -> Option<PredictionKind> {
    let one_covariate = e.covariates.windows(2).all(|w| w[0] == w[1]);
    match (e.factors.len(), e.covariates.first()) {
        (1 | 2, None) => Some(PredictionKind::Table),
        (0, Some(&c)) if one_covariate => Some(PredictionKind::Line(c)),
        (1 | 2, Some(&c)) if one_covariate => Some(PredictionKind::Mixed(c)),
        _ => None,
    }
}

impl Engine {
    pub(crate) fn quantile_regression(
        &mut self,
        c: &QuantileRegression,
    ) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let factor_names = self.expand(&c.factors)?;
        let covariate_names = self.expand(&c.covariates)?;
        let dependent = data.variable(&c.dependent)?;
        let mut seen = vec![dependent.name().to_string()];
        for n in factor_names.iter().chain(&covariate_names) {
            if seen.iter().any(|s| s.eq_ignore_ascii_case(n)) {
                return Err(Error(format!(
                    "{n} appears more than once among the dependent variable, factors and covariates"
                )));
            }
            seen.push(n.clone());
        }
        let factor_vars: Vec<Variable> = factor_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let cov_vars: Vec<Variable> = covariate_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let regwgt = c.regwgt.as_ref().map(|n| data.variable(n)).transpose()?;
        for v in std::iter::once(&dependent)
            .chain(&cov_vars)
            .chain(regwgt.as_ref())
        {
            if v.is_string() {
                return Err(Error(format!(
                    "QUANTILE REGRESSION needs numeric dependent, covariates and regression weight, but {} is a string variable",
                    v.name()
                )));
            }
        }
        let cases = data.case_count();
        let weights = case_weights(data)?;
        let freq = |k: usize| weights.as_ref().map_or(1.0, |w| w[k].round());
        let numeric = |v: &Variable, k: usize| -> Option<f64> {
            if !crate::means::is_valid(v, k, false) {
                return None;
            }
            match v.values() {
                Values::Numeric(values) => values.get(k),
                Values::String(_) => None,
            }
        };
        let factor_keys: Vec<Vec<Option<Key>>> = factor_vars
            .iter()
            .map(|v| categories(v, c.include))
            .collect();
        let predictors_valid = |k: usize| {
            factor_keys.iter().all(|col| col[k].is_some())
                && cov_vars.iter().all(|v| numeric(v, k).is_some())
        };
        let regression_weight = |k: usize| regwgt.as_ref().map_or(Some(1.0), |v| numeric(v, k));
        let rows: Vec<usize> = (0..cases)
            .filter(|&k| {
                predictors_valid(k)
                    && numeric(&dependent, k).is_some()
                    && freq(k) > 0.0
                    && regression_weight(k).is_some_and(|g| g > 0.0)
            })
            .collect();
        let mut used = vec![false; cases];
        rows.iter().for_each(|&k| used[k] = true);
        let excluded: f64 = (0..cases)
            .filter(|&k| !used[k])
            .map(|k| freq(k).max(0.0))
            .sum();
        if rows.is_empty() {
            return Err(Error(
                "QUANTILE REGRESSION: no case has valid values on every variable and positive weights".into(),
            ));
        }
        if rows.len() == 1 {
            return Err(Error(
                "QUANTILE REGRESSION: only one case is valid; no model is fitted".into(),
            ));
        }
        let mut levels: Vec<Vec<Key>> = Vec::new();
        for col in &factor_keys {
            let mut found: Vec<Key> = Vec::new();
            for &k in &rows {
                let key = col[k].as_ref().expect("valid");
                if !found.iter().any(|l| l.compare(key).is_eq()) {
                    found.push(match key {
                        Key::Number(n) => Key::Number(*n),
                        Key::Text(t) => Key::Text(t.clone()),
                    });
                }
            }
            found.sort_by(|a, b| a.compare(b));
            levels.push(found);
        }
        let level_of = |f: usize, k: usize| -> Option<usize> {
            let key = factor_keys[f][k].as_ref()?;
            levels[f].iter().position(|l| l.compare(key).is_eq())
        };
        let labels = Shown::labels(shown.currencies.clone());
        let d = Data {
            factors: factor_names.clone(),
            factor_texts: factor_vars.iter().map(|v| shown.variable(v)).collect(),
            values: factor_vars
                .iter()
                .zip(&levels)
                .map(|(v, ls)| ls.iter().map(|k| key_value_text(v, k)).collect())
                .collect(),
            labels: factor_vars
                .iter()
                .zip(&levels)
                .map(|(v, ls)| ls.iter().map(|k| labels.value(v, k)).collect())
                .collect(),
            labelled: false,
            covariates: covariate_names.clone(),
            level: rows
                .iter()
                .map(|&k| {
                    (0..factor_vars.len())
                        .map(|f| level_of(f, k).expect("a level"))
                        .collect()
                })
                .collect(),
            cov: rows
                .iter()
                .map(|&k| {
                    cov_vars
                        .iter()
                        .map(|v| numeric(v, k).expect("valid"))
                        .collect()
                })
                .collect(),
            y: rows
                .iter()
                .map(|&k| vec![numeric(&dependent, k).expect("valid")])
                .collect(),
            f: rows.iter().map(|&k| freq(k)).collect(),
            g: rows
                .iter()
                .map(|&k| regression_weight(k).expect("valid"))
                .collect(),
            case: rows.clone(),
        };
        let y: Vec<f64> = d.y.iter().map(|r| r[0]).collect();
        if y.iter().all(|&v| v == y[0]) {
            return Err(Error(format!(
                "QUANTILE REGRESSION: the dependent variable {} is constant; no model is fitted",
                dependent.name()
            )));
        }
        let terms: Vec<Term> = match &c.effects {
            Some(effects) => {
                let mut t = effects.clone();
                if c.intercept {
                    t.insert(0, intercept_term());
                }
                t
            }
            None => default_terms(&[], &covariate_names, c.intercept)
                .into_iter()
                .chain(factor_names.iter().map(|n| Term {
                    variables: vec![n.clone()],
                    label: n.clone(),
                }))
                .collect(),
        };
        if terms.is_empty() {
            return Err(Error(
                "QUANTILE REGRESSION needs the intercept or at least one predictor".into(),
            ));
        }
        let effects = effects_of(&terms, &factor_names, &covariate_names)?;
        let fit = Fit::new(&d, effects, &d.y, 1e-8);
        let kept: Vec<usize> = (0..fit.p).filter(|&j| !fit.redundant[j]).collect();
        let k = kept.len();
        let n = y.len();
        let w: Vec<f64> = (0..n).map(|i| d.f[i] * d.g[i]).collect();
        let big_n: f64 = d.f.iter().sum();
        let x: Vec<Vec<f64>> = fit
            .x
            .iter()
            .map(|row| kept.iter().map(|&j| row[j]).collect())
            .collect();
        // The fit runs on columns scaled to unit spread (and centred when the model has an
        // intercept), whose units and distance from 0 then cannot upset the simplex steps; the
        // coefficients and their covariances are mapped back at the end. beta = T beta*.
        let x_raw = x.clone();
        let (x, back) = standardised(x, &w);
        let xs: Vec<Vec<f64>> = x
            .iter()
            .zip(&w)
            .map(|(row, wi)| row.iter().map(|v| v * wi).collect())
            .collect();
        let ys: Vec<f64> = y.iter().zip(&w).map(|(v, wi)| v * wi).collect();
        let mut xwx = vec![vec![0.0; k]; k];
        for i in 0..n {
            for a in 0..k {
                for b in 0..k {
                    xwx[a][b] += w[i] * x[i][a] * x[i][b];
                }
            }
        }
        let (xwx_inv, _) = ginverse(&xwx, 1e-12);
        let mut warnings: Vec<String> = Vec::new();
        let mut quantiles: Vec<f64> = Vec::new();
        for &q in &c.quantiles {
            if quantiles.contains(&q) {
                warnings.push(format!(
                    "Quantile {q} is given more than once; it is fitted once"
                ));
            } else {
                quantiles.push(q);
            }
        }
        quantiles.sort_by(f64::total_cmp);
        let simplex_first = match c.method {
            QrMethod::Simplex => true,
            QrMethod::InteriorPoint => false,
            QrMethod::Auto => n <= 1000,
        };
        let mut not_converged = false;
        let mut solve = |p: f64| -> Result<Vec<f64>, Error> {
            let p = p.clamp(1e-10, 1.0 - 1e-10);
            let start = if simplex_first {
                let ls = least_squares(&xs, &ys);
                let r: Vec<f64> = (0..n).map(|i| (ys[i] - dot(&xs[i], &ls)).abs()).collect();
                let mut order: Vec<usize> = (0..n).collect();
                order.sort_by(|a, b| r[*a].total_cmp(&r[*b]));
                order
            } else {
                let (b, ok) = interior_point(&xs, &ys, p, c.tol, c.conv, c.maxiter);
                not_converged |= !ok;
                let r: Vec<f64> = (0..n).map(|i| (ys[i] - dot(&xs[i], &b)).abs()).collect();
                let mut order: Vec<usize> = (0..n).collect();
                order.sort_by(|a, b| r[*a].total_cmp(&r[*b]));
                order
            };
            let basis = independent_rows(&xs, start.into_iter()).ok_or_else(|| {
                Error("QUANTILE REGRESSION: the design has no full set of independent cases".into())
            })?;
            solve_lp(&xs, &ys, p, basis).ok_or_else(|| {
                Error("QUANTILE REGRESSION: the linear program could not be solved".into())
            })
        };
        let objective = |b: &[f64], p: f64| -> f64 {
            (0..n).map(|i| w[i] * rho(y[i] - dot(&x[i], b), p)).sum()
        };
        let mut fits: Vec<QuantileFit> = Vec::new();
        for &q in &quantiles {
            let p = q.clamp(1e-10, 1.0 - 1e-10);
            let b = solve(q)?;
            let resid: Vec<f64> = (0..n).map(|i| y[i] - dot(&x[i], &b)).collect();
            let exact = resid
                .iter()
                .zip(&y)
                .all(|(r, v)| r.abs() <= 1e-10 * (1.0 + v.abs()));
            let h = bandwidth(p, big_n, c.hall_sheather, c.cilevel);
            let mut covariance = None;
            let (mut sparsity, mut spread) = (None, None);
            if exact {
                covariance = Some(vec![vec![0.0; k]; k]);
                sparsity = Some(0.0);
            } else if c.iid {
                let mut order: Vec<usize> = (0..n).collect();
                order.sort_by(|a, b| resid[*a].total_cmp(&resid[*b]));
                let at = |rank: f64| -> f64 {
                    let rank = rank.clamp(1.0, big_n);
                    let mut total = 0.0;
                    for &i in &order {
                        total += d.f[i];
                        if total >= rank - 1e-9 {
                            return resid[i];
                        }
                    }
                    resid[order[n - 1]]
                };
                let s = (at((big_n * (p + h)).ceil()) - at((big_n * (p - h)).ceil())) / (2.0 * h);
                if s.is_finite() {
                    sparsity = Some(s);
                    covariance = Some(
                        xwx_inv
                            .iter()
                            .map(|row| row.iter().map(|v| p * (1.0 - p) * s * s * v).collect())
                            .collect(),
                    );
                }
            } else {
                let (hi, lo) = ((p + h).min(1.0 - 1e-10), (p - h).max(1e-10));
                let (bh, bl) = (solve(hi)?, solve(lo)?);
                let mut hm = vec![vec![0.0; k]; k];
                for i in 0..n {
                    let gap = dot(&x[i], &bh) - dot(&x[i], &bl);
                    let density = if gap > 0.0 { (hi - lo) / gap } else { 0.0 };
                    for a in 0..k {
                        for b2 in 0..k {
                            hm[a][b2] += w[i] * density * x[i][a] * x[i][b2];
                        }
                    }
                }
                let (hinv, rank) = ginverse(&hm, 1e-12);
                if rank == k {
                    let m = crate::matrix::multiply(&crate::matrix::multiply(&hinv, &xwx), &hinv);
                    covariance = Some(
                        m.iter()
                            .map(|row| row.iter().map(|v| p * (1.0 - p) * v).collect())
                            .collect(),
                    );
                } else {
                    warnings.push(format!(
                        "At quantile {q} the estimated densities leave the matrix H singular; the standard errors are not shown"
                    ));
                }
                spread = Some((bh, bl, hi - lo));
            }
            let objective_here = objective(&b, p);
            fits.push(QuantileFit {
                p: q,
                objective: objective_here,
                b: back.apply(&b),
                covariance: covariance.map(|c| back.cov(&c)),
                sparsity,
                spread: spread.map(|(bh, bl, gap)| (back.apply(&bh), back.apply(&bl), gap)),
                exact,
            });
        }
        if not_converged {
            warnings.push(format!(
                "The interior-point method did not reach the convergence criterion within {} iterations (MAXITER); the simplex finished the fit from where it stopped",
                c.maxiter
            ));
        }
        // the parameters in design order: (name, column among the estimated ones)
        let mut parameters: Vec<(String, Option<usize>)> = Vec::new();
        for e in &fit.effects {
            for (cell_at, cell) in e.cells.iter().enumerate() {
                let j = e.start + cell_at;
                parameters.push((
                    parameter_name(&d, e, cell),
                    kept.iter().position(|&kj| kj == j),
                ));
            }
        }
        let df = big_n - k as f64;
        let level = c.cilevel;
        let level_text = if level.fract() == 0.0 {
            format!("{level:.0}%")
        } else {
            format!("{level}%")
        };
        let dep_text = shown.variable(&dependent);
        let model_text = fit
            .effects
            .iter()
            .map(|e| e.label.clone())
            .collect::<Vec<_>>()
            .join(" + ");
        let caption = format!("Dependent variable: {dep_text}. Model: {model_text}.");
        let mut tables = Vec::new();
        tables.push(table(
            "Case Processing Summary",
            1,
            plain(&["", "N", "Percent"]),
            [
                ("Included", big_n),
                ("Excluded", excluded),
                ("Total", big_n + excluded),
            ]
            .into_iter()
            .map(|(name, v)| {
                vec![
                    text(name),
                    Cell::Number(v),
                    number(Some(100.0 * v / (big_n + excluded))),
                ]
            })
            .collect(),
            None,
        ));
        let (intercept_y, intercept_w): (Vec<f64>, Vec<f64>) = {
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|a, b| y[*a].total_cmp(&y[*b]));
            order.iter().map(|&i| (y[i], w[i])).unzip()
        };
        let total_w: f64 = w.iter().sum();
        let summary_rows = fits
            .iter()
            .map(|f| {
                let p = f.p.clamp(1e-10, 1.0 - 1e-10);
                // the intercept alone: the weighted p-th quantile of y
                let mut cum = 0.0;
                let m = intercept_y
                    .iter()
                    .zip(&intercept_w)
                    .find(|(_, wi)| {
                        cum += *wi;
                        cum >= p * total_w - 1e-12 * total_w
                    })
                    .map_or(intercept_y[n - 1], |(v, _)| *v);
                let v0: f64 = (0..n).map(|i| w[i] * rho(y[i] - m, p)).sum();
                let mae: f64 = (0..n)
                    .map(|i| w[i] * (y[i] - dot(&x_raw[i], &f.b)).abs())
                    .sum::<f64>()
                    / big_n;
                vec![
                    text(format!("{}", f.p)),
                    number((v0 > 0.0).then(|| 1.0 - f.objective / v0)),
                    Cell::Number(mae),
                ]
            })
            .collect();
        tables.push(table(
            "Model Quality",
            1,
            plain(&["Quantile", "Pseudo R Squared", "Mean Absolute Error"]),
            summary_rows,
            Some(caption.clone()),
        ));
        let se_note = if c.iid {
            format!(
                "Standard errors assume independent and identically distributed errors ({} bandwidth).",
                if c.hall_sheather {
                    "Hall-Sheather"
                } else {
                    "Bofinger"
                }
            )
        } else {
            format!(
                "Standard errors from the densities at each case ({} bandwidth).",
                if c.hall_sheather {
                    "Hall-Sheather"
                } else {
                    "Bofinger"
                }
            )
        };
        let tq = (df > 0.0).then(|| t_quantile((1.0 - level / 100.0) / 2.0, df));
        if c.print_parameter {
            let group = format!("{level_text} Confidence Interval");
            let mut columns = plain(&[
                "Quantile",
                "Parameter",
                "Coefficient",
                "Std. Error",
                "t",
                "df",
                "Sig.",
            ]);
            columns.push((group.clone(), "Lower Bound".into()));
            columns.push((group, "Upper Bound".into()));
            let mut out = Vec::new();
            let mut any_redundant = false;
            for f in &fits {
                for (at, (name, col)) in parameters.iter().enumerate() {
                    let head = if at == 0 {
                        text(format!("{}", f.p))
                    } else {
                        Cell::Empty
                    };
                    let Some(j) = *col else {
                        any_redundant = true;
                        out.push(vec![
                            head,
                            text(name.clone()),
                            Cell::Number(0.0),
                            Cell::Empty,
                            Cell::Empty,
                            Cell::Empty,
                            Cell::Empty,
                            Cell::Empty,
                            Cell::Empty,
                        ]);
                        continue;
                    };
                    let b = f.b[j];
                    let se = f.covariance.as_ref().map(|m| m[j][j].max(0.0).sqrt());
                    let t = se.filter(|s| *s > 0.0).map(|s| b / s);
                    out.push(vec![
                        head,
                        text(name.clone()),
                        Cell::Number(b),
                        number(se),
                        number(t),
                        number((df > 0.0).then_some(df)),
                        number(t.filter(|_| df > 0.0).map(|t| t_p(t, df))),
                        number(se.zip(tq).map(|(s, q)| b - q * s)),
                        number(se.zip(tq).map(|(s, q)| b + q * s)),
                    ]);
                }
            }
            let mut text_caption = format!("{caption} {se_note}");
            if any_redundant {
                text_caption.push_str(" A parameter of 0 without statistics is redundant.");
            }
            if fits.iter().any(|f| f.exact) {
                text_caption.push_str(" The model fits every case exactly; the covariance is 0.");
            }
            tables.push(table(
                "Parameter Estimates",
                2,
                columns,
                out,
                Some(text_caption),
            ));
        }
        let names: Vec<String> = parameters
            .iter()
            .filter(|(_, col)| col.is_some())
            .map(|(n, _)| n.clone())
            .collect();
        let correlation = |m: &Matrix, a: usize, b: usize| {
            let v = m[a][a] * m[b][b];
            (v > 0.0).then(|| m[a][b] / v.sqrt())
        };
        for (title, corr, wanted) in [
            ("Parameter Covariance", false, c.print_covb),
            ("Parameter Correlation", true, c.print_corb),
        ] {
            if !wanted {
                continue;
            }
            let mut columns = plain(&["Quantile", "Parameter"]);
            columns.extend(names.iter().map(|n| (String::new(), n.clone())));
            let mut out = Vec::new();
            for f in &fits {
                for a in 0..k {
                    let mut row = vec![
                        if a == 0 {
                            text(format!("{}", f.p))
                        } else {
                            Cell::Empty
                        },
                        text(names[a].clone()),
                    ];
                    row.extend((0..k).map(|b| {
                        number(f.covariance.as_ref().and_then(|m| {
                            if corr {
                                correlation(m, a, b)
                            } else {
                                Some(m[a][b])
                            }
                        }))
                    }));
                    out.push(row);
                }
            }
            tables.push(table(
                title,
                2,
                columns,
                out,
                Some(format!("{caption} {se_note}")),
            ));
        }
        // predictions of the model's effects: tables of factor cells, lines along a covariate
        let multi = fits.len() > 1;
        let full_b = |f: &QuantileFit| -> Vec<f64> {
            let mut out = vec![0.0; fit.p];
            for (j, &kj) in kept.iter().enumerate() {
                out[kj] = f.b[j];
            }
            out
        };
        let explicit = c.prediction_tables.is_some() || c.prediction_lines.is_some();
        let mut for_tables: Vec<usize> = Vec::new();
        let mut for_lines: Vec<usize> = Vec::new();
        let find = |term: &Term| -> Result<usize, Error> {
            match crate::glm::find_effect(&d, &fit.effects, term)? {
                Some(e) => Ok(fit
                    .effects
                    .iter()
                    .position(|o| std::ptr::eq(o, e))
                    .expect("found")),
                None => Err(Error(format!(
                    "QUANTILE REGRESSION: {} is not an effect of the model",
                    term.label
                ))),
            }
        };
        let estimated = |e: &Effect| e.range().any(|j| !fit.redundant[j]);
        if explicit {
            for (list, lines) in [(&c.prediction_tables, false), (&c.prediction_lines, true)] {
                for term in list.iter().flatten() {
                    let at = find(term)?;
                    let e = &fit.effects[at];
                    match (kind_of(e), lines) {
                        (Some(PredictionKind::Table), false) => for_tables.push(at),
                        (Some(PredictionKind::Line(_) | PredictionKind::Mixed(_)), true) => for_lines.push(at),
                        _ => warnings.push(format!(
                            "{} gets no prediction {}: tables take one or two factors, lines one covariate (alone or with one or two factors)",
                            term.label,
                            if lines { "lines" } else { "table" }
                        )),
                    }
                }
            }
        } else {
            for (at, e) in fit.effects.iter().enumerate() {
                if for_tables.len() + for_lines.len() >= c.top_effects {
                    break;
                }
                match kind_of(e) {
                    Some(PredictionKind::Table) if estimated(e) => for_tables.push(at),
                    Some(PredictionKind::Line(_) | PredictionKind::Mixed(_)) if estimated(e) => {
                        for_lines.push(at)
                    }
                    _ => {}
                }
            }
        }
        let note = "Other covariates at their means, other factors averaged over their levels.";
        let cell_label = |e: &Effect, cell: &[usize]| -> Vec<String> {
            e.factors
                .iter()
                .zip(cell)
                .map(|(&f, &l)| d.labels[f][l].clone())
                .collect()
        };
        for &at in &for_tables {
            let e = &fit.effects[at];
            let mut columns: Vec<(String, String)> = e
                .factors
                .iter()
                .map(|&f| (String::new(), d.factor_texts[f].clone()))
                .collect();
            columns.extend(
                fits.iter()
                    .map(|f| ("Quantile".to_string(), format!("{}", f.p))),
            );
            let rows = e
                .cells
                .iter()
                .map(|cell| {
                    let fixed: Vec<(usize, usize)> = e
                        .factors
                        .iter()
                        .copied()
                        .zip(cell.iter().copied())
                        .collect();
                    let l = emmeans_l(&d, &fit, &fixed, &[]);
                    let mut row: Vec<Cell> = cell_label(e, cell).into_iter().map(text).collect();
                    row.extend(fits.iter().map(|f| Cell::Number(dot(&l, &full_b(f)))));
                    row
                })
                .collect();
            tables.push(table(
                &format!("Predicted Values: {}", e.label),
                e.factors.len(),
                columns,
                rows,
                Some(format!("{caption} {note}")),
            ));
        }
        let mut charts: Vec<Chart> = Vec::new();
        for &at in &for_lines {
            let e = &fit.effects[at];
            let cov = match kind_of(e) {
                Some(PredictionKind::Line(cv) | PredictionKind::Mixed(cv)) => cv,
                _ => continue,
            };
            let (lo, hi) = d
                .cov
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), r| {
                    (a.min(r[cov]), b.max(r[cov]))
                });
            let grid: Vec<f64> = (0..=10)
                .map(|i| lo + (hi - lo) * f64::from(i) / 10.0)
                .collect();
            let categories: Vec<crate::chart::Category> = grid
                .iter()
                .map(|v| crate::chart::Category {
                    label: format!("{}", (v * 1e4).round() / 1e4),
                    value: Cell::Number(*v),
                    missing: false,
                    bin: None,
                })
                .collect();
            let line =
                |name: String, fixed: &[(usize, usize)], f: &QuantileFit| crate::chart::Series {
                    name,
                    statistic: crate::chart::Statistic::Mean,
                    variable: None,
                    value: Cell::Empty,
                    values: grid
                        .iter()
                        .map(|v| dot(&emmeans_l(&d, &fit, fixed, &[(cov, *v)]), &full_b(f)))
                        .collect(),
                };
            let x_axis = crate::chart::Axis::titled(d.covariates[cov].clone());
            let y_axis = crate::chart::Axis::titled(format!("Predicted {dep_text}"));
            if e.factors.is_empty() {
                let series = fits
                    .iter()
                    .map(|f| line(format!("Quantile {}", f.p), &[], f))
                    .collect();
                let mut chart = Chart::lines(
                    format!("Predicted {dep_text} by {}", e.label),
                    x_axis,
                    y_axis,
                    categories,
                    series,
                );
                chart.legend = Some("Quantile".into());
                chart.footnote = Some(note.into());
                charts.push(chart);
                continue;
            }
            for f in &fits {
                let series = e
                    .cells
                    .iter()
                    .take(c.max_categories)
                    .map(|cell| {
                        let fixed: Vec<(usize, usize)> = e
                            .factors
                            .iter()
                            .copied()
                            .zip(cell.iter().copied())
                            .collect();
                        line(cell_label(e, cell).join(", "), &fixed, f)
                    })
                    .collect();
                let mut chart = Chart::lines(
                    format!("Predicted {dep_text} by {} at quantile {}", e.label, f.p),
                    x_axis.clone(),
                    y_axis.clone(),
                    categories.clone(),
                    series,
                );
                chart.legend = Some(
                    e.factors
                        .iter()
                        .map(|&g| d.factor_texts[g].clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                chart.footnote = Some(note.into());
                charts.push(chart);
            }
        }
        if c.predicted_by_observed {
            let groups = fits
                .iter()
                .map(|f| PointGroup {
                    name: format!("Quantile {}", f.p),
                    points: (0..n)
                        .map(|i| Point {
                            x: y[i],
                            y: dot(&x_raw[i], &f.b),
                            z: None,
                            label: None,
                        })
                        .collect(),
                })
                .collect();
            let mut chart = Chart::plot(
                crate::chart::ChartKind::Scatter,
                format!("Predicted by observed: {dep_text}"),
                crate::chart::Axis::titled(format!("Observed {dep_text}")),
                crate::chart::Axis::titled(format!("Predicted {dep_text}")),
                Plot::Points {
                    groups,
                    lines: Vec::new(),
                    z_axis: None,
                },
            );
            chart.footnote = Some(caption.clone());
            charts.push(chart);
        }
        if multi {
            let limit = c.param_est.unwrap_or(usize::MAX);
            for (a, name) in names.iter().enumerate().take(limit) {
                let categories = fits
                    .iter()
                    .map(|f| crate::chart::Category {
                        label: format!("{}", f.p),
                        value: Cell::Number(f.p),
                        missing: false,
                        bin: None,
                    })
                    .collect();
                let margins = fits
                    .iter()
                    .map(|f| {
                        f.covariance
                            .as_ref()
                            .zip(tq)
                            .map_or(f64::NAN, |(m, q)| q * m[a][a].max(0.0).sqrt())
                    })
                    .collect();
                let mut chart = Chart::lines(
                    format!("Parameter estimates by quantile: {name}"),
                    crate::chart::Axis::titled("Quantile"),
                    crate::chart::Axis::titled("Coefficient"),
                    categories,
                    vec![crate::chart::Series {
                        name: name.clone(),
                        statistic: crate::chart::Statistic::Mean,
                        variable: None,
                        value: Cell::Empty,
                        values: fits.iter().map(|f| f.b[a]).collect(),
                    }],
                );
                chart.error_bars = Some(crate::chart::ErrorBars {
                    label: format!("{level_text} CI"),
                    margins: vec![margins],
                    lower: Vec::new(),
                });
                chart.footnote = Some(caption.clone());
                charts.push(chart);
            }
        }
        // OUTFILE: the estimates with their covariances or correlations, and the model file
        let suffixed = |file: &str, p: f64| -> String {
            let s = suffix(p);
            match file.rsplit_once('.') {
                Some((stem, ext)) if !ext.contains('/') && !ext.contains('\\') => {
                    format!("{stem}_{s}.{ext}")
                }
                _ => format!("{file}_{s}"),
            }
        };
        let mut files: Vec<(String, Dataset)> = Vec::new();
        for (file, corr) in [(&c.outfile_covb, false), (&c.outfile_corb, true)] {
            let Some(file) = file else { continue };
            let groups: Vec<Vec<&QuantileFit>> = if multi && c.file_separate {
                fits.iter().map(|f| vec![f]).collect()
            } else {
                vec![fits.iter().collect()]
            };
            for group in groups {
                let with_quantile = group.len() > 1;
                let mut rows: Vec<(f64, &str, String, Vec<Option<f64>>)> = Vec::new();
                for f in &group {
                    let se = |col: Option<usize>| {
                        col.and_then(|j| f.covariance.as_ref().map(|m| m[j][j].max(0.0).sqrt()))
                    };
                    let est = parameters
                        .iter()
                        .map(|(_, col)| Some(col.map_or(0.0, |j| f.b[j])))
                        .collect();
                    let ses = parameters.iter().map(|(_, col)| se(*col)).collect();
                    let sig = parameters
                        .iter()
                        .map(|(_, col)| {
                            se(*col)
                                .filter(|s| *s > 0.0 && df > 0.0)
                                .map(|s| t_p(f.b[col.expect("estimated")] / s, df))
                        })
                        .collect();
                    let dfs = parameters
                        .iter()
                        .map(|(_, col)| col.and((df > 0.0).then_some(df)))
                        .collect();
                    rows.push((f.p, "EST", String::new(), est));
                    rows.push((f.p, "SE", String::new(), ses));
                    rows.push((f.p, "SIG", String::new(), sig));
                    rows.push((f.p, "DF", String::new(), dfs));
                    for (i, (_, ci)) in parameters.iter().enumerate() {
                        let values = parameters
                            .iter()
                            .map(|(_, cj)| {
                                let (a, b) = ((*ci)?, (*cj)?);
                                let m = f.covariance.as_ref()?;
                                if corr {
                                    correlation(m, a, b)
                                } else {
                                    Some(m[a][b])
                                }
                            })
                            .collect();
                        rows.push((
                            f.p,
                            if corr { "CORR" } else { "COV" },
                            format!("P{}", i + 1),
                            values,
                        ));
                    }
                }
                let mut out = Dataset::new(rows.len());
                if with_quantile {
                    out.set_numeric_values("Quantile", rows.iter().map(|r| Some(r.0)).collect())?;
                }
                out.set_string_values(
                    "RowType_",
                    8,
                    rows.iter().map(|r| r.1.to_string()).collect(),
                )?;
                out.set_string_values("VarName_", 8, rows.iter().map(|r| r.2.clone()).collect())?;
                for (i, (name, _)) in parameters.iter().enumerate() {
                    let var = format!("P{}", i + 1);
                    out.set_numeric_values(&var, rows.iter().map(|r| r.3[i]).collect())?;
                    out.set_variable_label(&var, Some(name))?;
                }
                let target = if multi && c.file_separate {
                    suffixed(file, group[0].p)
                } else {
                    file.clone()
                };
                files.push((target, out));
            }
        }
        let mut models: Vec<(String, serde_json::Value)> = Vec::new();
        if let Some((file, with_covariance)) = &c.outfile_model {
            for f in &fits {
                let mut model = json!({
                    "format": "statscraft-quantile-regression",
                    "version": 1,
                    "dependent": dependent.name(),
                    "factors": factor_names,
                    "covariates": covariate_names,
                    "model": model_text,
                    "quantile": f.p,
                    "df": df,
                    "parameters": parameters.iter().map(|(name, col)| json!({
                        "name": name,
                        "estimate": col.map_or(0.0, |j| f.b[j]),
                        "redundant": col.is_none(),
                    })).collect::<Vec<_>>(),
                });
                if *with_covariance {
                    model["covariance"] = json!(f.covariance);
                }
                let target = if multi {
                    suffixed(file, f.p)
                } else {
                    file.clone()
                };
                models.push((target, model));
            }
        }
        // SAVE: every case with valid predictors and fitted factor levels
        let mut saved: Vec<(String, String, Vec<Option<f64>>)> = Vec::new();
        if !c.save.is_empty() {
            let z = normal_quantile(0.5 + level / 200.0);
            let rows_x: Vec<Option<Vec<f64>>> = (0..cases)
                .map(|kc| {
                    if !predictors_valid(kc) {
                        return None;
                    }
                    let lv: Vec<usize> = (0..factor_vars.len())
                        .map(|f| level_of(f, kc))
                        .collect::<Option<_>>()?;
                    let cv: Vec<f64> = cov_vars
                        .iter()
                        .map(|v| numeric(v, kc))
                        .collect::<Option<_>>()?;
                    let mut row = vec![0.0; fit.p];
                    for e in &fit.effects {
                        let mine: Vec<usize> = e.factors.iter().map(|&f| lv[f]).collect();
                        let at = e.cells.iter().position(|cell| *cell == mine)?;
                        row[e.start + at] = e.covariates.iter().map(|&cc| cv[cc]).product();
                    }
                    Some(kept.iter().map(|&j| row[j]).collect())
                })
                .collect();
            for f in &fits {
                let p = f.p.clamp(1e-10, 1.0 - 1e-10);
                let half = |xr: &[f64]| -> Option<f64> {
                    let quad: f64 = (0..k).map(|a| xr[a] * dot(&xwx_inv[a], xr)).sum();
                    let base = (p * (1.0 - p) * quad.max(0.0)).sqrt();
                    let s = match (&f.spread, f.sparsity) {
                        (_, Some(s)) => s,
                        (Some((bh, bl, width)), None) => {
                            let gap = dot(xr, bh) - dot(xr, bl);
                            if gap <= 0.0 {
                                return None;
                            }
                            gap / width
                        }
                        (None, None) => return None,
                    };
                    Some(z * base * s)
                };
                for (kind, name) in &c.save {
                    let root = name.clone().unwrap_or_else(|| {
                        match kind {
                            QrSave::Pred => "PRED",
                            QrSave::Resid => "RESID",
                            QrSave::Cipredl => "CIPREDL",
                            QrSave::Cipredu => "CIPREDU",
                        }
                        .to_string()
                    });
                    let var = if multi {
                        format!("{root}_{}", suffix(f.p))
                    } else {
                        root
                    };
                    let label = match kind {
                        QrSave::Pred => format!("Predicted value at quantile {}", f.p),
                        QrSave::Resid => format!("Residual at quantile {}", f.p),
                        QrSave::Cipredl => format!("Lower {level_text} bound at quantile {}", f.p),
                        QrSave::Cipredu => format!("Upper {level_text} bound at quantile {}", f.p),
                    };
                    let values = (0..cases)
                        .map(|kc| {
                            let xr = rows_x[kc].as_ref()?;
                            let pred = dot(xr, &f.b);
                            match kind {
                                QrSave::Pred => Some(pred),
                                QrSave::Resid => numeric(&dependent, kc).map(|v| v - pred),
                                QrSave::Cipredl => half(xr).map(|h| pred - h),
                                QrSave::Cipredu => half(xr).map(|h| pred + h),
                            }
                        })
                        .collect();
                    saved.push((var, label, values));
                }
            }
        }
        for w in warnings {
            self.warn(w);
        }
        for (file, out) in files {
            self.write_data_file(&file, out)?;
        }
        if !self.dry_run {
            for (file, model) in &models {
                self.write_json("QUANTILE REGRESSION", file, "model", model)?;
            }
        }
        if !saved.is_empty() {
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                for (name, label, values) in saved {
                    if data.variable_index(&name).is_some() {
                        return Err(Error(format!(
                            "QUANTILE REGRESSION /SAVE: variable {name} already exists"
                        )));
                    }
                    data.set_numeric_values(&name, values)?;
                    data.set_variable_label(&name, Some(&label))?;
                    data.set_format(&name, "F8.2")?;
                    data.set_measure(&name, Measure::Scale)?;
                }
                Ok(())
            })?;
        }
        self.charts.extend(charts);
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suffixes_keep_two_digits_at_least() {
        assert_eq!(suffix(0.5), "50");
        assert_eq!(suffix(0.25), "25");
        assert_eq!(suffix(0.502), "502");
        assert_eq!(suffix(0.05), "05");
        assert_eq!(suffix(1.0), "100");
        assert_eq!(suffix(0.0), "00");
    }

    #[test]
    fn simplex_and_interior_point_find_the_median_line() {
        // y = 1 + 2x except two outliers; the median line passes through the clean points
        let x: Vec<Vec<f64>> = (0..7).map(|i| vec![1.0, f64::from(i)]).collect();
        let mut y: Vec<f64> = (0..7).map(|i| 1.0 + 2.0 * f64::from(i)).collect();
        y[1] += 10.0;
        y[5] -= 7.0;
        let start = independent_rows(&x, 0..7).unwrap();
        let b = solve_lp(&x, &y, 0.5, start).unwrap();
        assert!(
            (b[0] - 1.0).abs() < 1e-12 && (b[1] - 2.0).abs() < 1e-12,
            "{b:?}"
        );
        let (ip, ok) = interior_point(&x, &y, 0.5, 1e-12, 1e-9, 200);
        assert!(
            ok && (ip[0] - 1.0).abs() < 1e-4 && (ip[1] - 2.0).abs() < 1e-4,
            "{ip:?}"
        );
    }
}
