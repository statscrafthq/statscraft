//! CATREG: linear regression of a quantified response on quantified predictors. The data
//! handling it shares with CATPCA and MULTIPLE CORRESPONDENCE lives in `optimal_scaling`, the
//! level restrictions in `catpca`.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - Each analysis variable gets a value per category: centered, weighted sum of squares n_w
//!   (the summed case weights), restricted by its level as in CATPCA. The fit minimizes the
//!   weighted squared distance between the quantified response and the sum of the coefficients
//!   times the quantified predictors.
//! - One iteration: the response's free values are the category means of the prediction, then
//!   restricted and normalized; each predictor in turn gets as free values the category means
//!   of the response less the other predictors, restricted and normalized, and its coefficient
//!   is the weighted cross product of the free and the new values over n_w. The apparent
//!   prediction error (APE) is the weighted mean squared error; the iterations stop when it
//!   drops less than CRITITER.
//! - A predictor keeps the sign of its coefficient while ordinal and spline ordinal values
//!   rise; MULTISTART and FIXSIGNS fix those signs. Numeric values rise with the codes.
//! - Ridge divides the coefficient by 1 + the ridge penalty; the lasso moves it half the lasso
//!   penalty towards 0, stopping at 0; the elastic net does both.
//! - NUMERICAL first fits every variable as numeric (in closed form without a lasso penalty,
//!   since that is plain or ridge regression on standardized codes); RANDOM starts the levels
//!   from the standardized codes with the correlations with the response as coefficients.
//! - R is the correlation of the response and the prediction. Degrees of freedom: numeric 1,
//!   spline the basis size less zero coefficients, ordinal and nominal one less than the distinct
//!   values. Partial, part, Pratt's importance and tolerance follow from the coefficients, the
//!   zero-order correlations and the correlations of the predictors with nonzero coefficients.
//! - The coefficients' standard errors come from 1000 bootstrap samples.
//! - Supplementary cases take the active quantifications; a code only they have is interpolated
//!   for numeric and spline levels and extrapolated for numeric ones, else they get none.

use crate::catpca::{Kind, Quant, restrict, standardized};
use crate::chart::{Axis, Category, ChartKind, Series, Statistic};
use crate::dist::f_p;
use crate::matrix::{Matrix, inverse};
use crate::mds::nnls;
use crate::npar_tests::text;
use crate::optimal_scaling::{Prepared, correlation_table, correlations, free_set, num, resolve};
use crate::plot::{Plot, Point, PointGroup};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_data::{Dataset, Measure};
use statscraft_syntax::{
    Catreg, CatregInitial, CatregPenalty, CatregResample, McaMissing, ScalingCommon, VarItem,
};

const WHO: &str = "CATREG";
/// Bootstrap samples for the standard errors of the coefficients.
const SE_SAMPLES: usize = 1000;

/// An analysis variable as the fit sees it; the response comes first.
struct Var {
    q: Quant,
    /// The category of each active case.
    h: Vec<usize>,
}

#[derive(Clone)]
struct State {
    /// The quantification of every variable.
    y: Vec<Vec<f64>>,
    /// The coefficient of every predictor (index 0, the response, stays 0).
    b: Vec<f64>,
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
struct Penalty {
    ridge: f64,
    lasso: f64,
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Numeric,
    Nominal,
    Levels,
}

#[derive(Clone)]
enum Start {
    Numerical,
    Random,
    /// A nominal first phase, then the levels with these signs.
    Signs(Vec<f64>),
    /// Systematic starts: every pattern (None) or the threshold search.
    Multi(Option<f64>),
}

struct Run {
    state: State,
    /// R square and APE of each iteration, iteration 0 first.
    history: Vec<(f64, f64)>,
    converged: bool,
    /// The signs used, with MULTISTART or FIXSIGNS.
    signs: Option<Vec<f64>>,
    /// MULTISTART: every pattern tried with its fit.
    patterns: Vec<(Vec<f64>, f64)>,
}

struct Model<'a> {
    vars: &'a [Var],
    penalty: Penalty,
    max: usize,
    criterion: f64,
}

fn ordered(q: &Quant) -> bool {
    matches!(q.kind, Kind::Ordinal | Kind::Spline { ordinal: true })
}

fn category_weights(v: &Var, w: &[f64]) -> Vec<f64> {
    let mut d = vec![0.0; v.q.values.len()];
    for (i, &r) in v.h.iter().enumerate() {
        d[r] += w[i];
    }
    d
}

fn predict(vars: &[Var], s: &State) -> Vec<f64> {
    (0..vars[0].h.len())
        .map(|i| (1..vars.len()).map(|j| s.b[j] * s.y[j][vars[j].h[i]]).sum())
        .collect()
}

/// R square and the apparent prediction error.
fn measures(vars: &[Var], s: &State, w: &[f64]) -> (f64, f64) {
    let v = predict(vars, s);
    let n_w: f64 = w.iter().sum();
    let (mut yv, mut vv, mut e) = (0.0, 0.0, 0.0);
    for (i, vi) in v.iter().enumerate() {
        let y = s.y[0][vars[0].h[i]];
        yv += w[i] * y * vi;
        vv += w[i] * vi * vi;
        e += w[i] * (y - vi).powi(2);
    }
    let r2 = if vv > 0.0 { yv * yv / (n_w * vv) } else { 0.0 };
    (r2, e / n_w)
}

/// The restricted quantification of the free values, centered with weighted sum of squares
/// n_w; None when nothing varies.
fn quantify(q: &Quant, phase: Phase, free: &[f64], d: &[f64], n_w: f64) -> Option<Vec<f64>> {
    let total: f64 = d.iter().sum();
    // shortcut: a category a resample leaves out gets a negligible weight, so that the
    // restrictions stay defined; its value is estimated afterwards
    let dd: Vec<f64> = d.iter().map(|x| x.max(total * 1e-12)).collect();
    let ystar = match phase {
        Phase::Nominal => free.to_vec(),
        Phase::Numeric => restrict(q, true, free, &dd),
        Phase::Levels => restrict(q, false, free, &dd),
    };
    let mean = ystar.iter().zip(d).map(|(y, d)| y * d).sum::<f64>() / total;
    let ss: f64 = ystar
        .iter()
        .zip(d)
        .map(|(y, d)| d * (y - mean).powi(2))
        .sum();
    if ss <= 1e-20 * n_w {
        return None;
    }
    let mut y: Vec<f64> = ystar
        .iter()
        .map(|y| (y - mean) * (n_w / ss).sqrt())
        .collect();
    // numeric values rise with the codes; the coefficient carries the direction
    if phase == Phase::Numeric || (phase == Phase::Levels && q.kind == Kind::Numeric) {
        let slope: f64 = (0..y.len()).map(|r| d[r] * y[r] * q.values[r]).sum();
        if slope < 0.0 {
            y.iter_mut().for_each(|v| *v = -*v);
        }
    }
    Some(y)
}

fn shrink(beta: f64, p: Penalty) -> f64 {
    beta.signum() * (beta.abs() - p.lasso / 2.0).max(0.0) / (1.0 + p.ridge)
}

/// Initialization I: standardized codes, the correlations with the response as coefficients.
fn initial_state(vars: &[Var], w: &[f64]) -> State {
    let n_w: f64 = w.iter().sum();
    let y: Vec<Vec<f64>> = vars
        .iter()
        .map(|v| standardized(&v.q.values, &category_weights(v, w), n_w))
        .collect();
    let b = (0..vars.len())
        .map(|j| match j {
            0 => 0.0,
            _ => {
                (0..w.len())
                    .map(|i| w[i] * y[0][vars[0].h[i]] * y[j][vars[j].h[i]])
                    .sum::<f64>()
                    / n_w
            }
        })
        .collect();
    State { y, b }
}

/// One iteration: the response, then each predictor.
fn iterate_once(
    m: &Model,
    s: &mut State,
    w: &[f64],
    ds: &[Vec<f64>],
    phase: Phase,
    signs: Option<&[f64]>,
) {
    let vars = m.vars;
    let n_w: f64 = w.iter().sum();
    let mut v = predict(vars, s);
    let mut free = vec![0.0; ds[0].len()];
    for (i, vi) in v.iter().enumerate() {
        free[vars[0].h[i]] += w[i] * vi;
    }
    for (f, d) in free.iter_mut().zip(&ds[0]) {
        if *d > 0.0 {
            *f /= d;
        }
    }
    if let Some(y) = quantify(&vars[0].q, phase, &free, &ds[0], n_w) {
        s.y[0] = y;
    }
    for j in 1..vars.len() {
        let (var, d) = (&vars[j], &ds[j]);
        let mut free = vec![0.0; d.len()];
        for (i, vi) in v.iter().enumerate() {
            let r = var.h[i];
            free[r] += w[i] * (s.y[0][vars[0].h[i]] - vi + s.b[j] * s.y[j][r]);
        }
        for (f, d) in free.iter_mut().zip(d) {
            if *d > 0.0 {
                *f /= d;
            }
        }
        let sign = match signs {
            Some(sg) if phase == Phase::Levels && ordered(&var.q) => sg[j],
            _ if s.b[j] < 0.0 => -1.0,
            _ => 1.0,
        };
        let signed: Vec<f64> = free.iter().map(|x| sign * x).collect();
        let (old_y, old_b) = (s.y[j].clone(), s.b[j]);
        if let Some(y) = quantify(&var.q, phase, &signed, d, n_w) {
            s.y[j] = y;
        }
        let beta = (0..d.len())
            .map(|r| d[r] * free[r] * s.y[j][r])
            .sum::<f64>()
            / n_w;
        let b = shrink(beta, m.penalty);
        for (i, vi) in v.iter_mut().enumerate() {
            let r = var.h[i];
            *vi += b * s.y[j][r] - old_b * old_y[r];
        }
        s.b[j] = b;
    }
}

/// Iterates until the APE drops less than the criterion (true) or the most iterations; the
/// R square and APE of each iteration.
fn cycle(
    m: &Model,
    s: &mut State,
    w: &[f64],
    phase: Phase,
    signs: Option<&[f64]>,
) -> (Vec<(f64, f64)>, bool) {
    let ds: Vec<Vec<f64>> = m.vars.iter().map(|v| category_weights(v, w)).collect();
    let mut last = measures(m.vars, s, w).1;
    let mut out = Vec::new();
    for _ in 0..m.max {
        iterate_once(m, s, w, &ds, phase, signs);
        let now = measures(m.vars, s, w);
        out.push(now);
        if (last - now.1).abs() < m.criterion {
            return (out, true);
        }
        last = now.1;
    }
    (out, false)
}

/// The numeric first phase in closed form when no lasso penalty applies: the values are the
/// standardized codes and the coefficients solve (R + ridge I) b = r, the point the
/// iterations approach. None when the correlations are singular.
fn numeric_exact(m: &Model, w: &[f64]) -> Option<State> {
    if m.penalty.lasso > 0.0 {
        return None;
    }
    let mut s = initial_state(m.vars, w);
    let all = correlations(&columns(m.vars, &s), w);
    let p = m.vars.len() - 1;
    let a: Matrix = (1..=p)
        .map(|i| {
            (1..=p)
                .map(|j| all[i][j] + if i == j { m.penalty.ridge } else { 0.0 })
                .collect()
        })
        .collect();
    let (inv, _) = inverse(&a)?;
    for i in 1..=p {
        s.b[i] = (1..=p).map(|j| inv[i - 1][j - 1] * all[0][j]).sum();
    }
    s.b.iter().all(|b| b.is_finite()).then_some(s)
}

/// The fit with the regularized R square 1 - APE.
fn regu_fit(run: &Run) -> f64 {
    1.0 - run.history.last().map_or(1.0, |h| h.1)
}

/// The whole fit from a start.
fn fit(m: &Model, w: &[f64], start: &Start) -> Run {
    let init = initial_state(m.vars, w);
    let first = |phase: Phase| {
        let mut s = init.clone();
        let (h, _) = cycle(m, &mut s, w, phase, None);
        let last = h.last().copied().unwrap_or_else(|| measures(m.vars, &s, w));
        (s, last)
    };
    let levels = |mut s: State, zero: (f64, f64), signs: Option<Vec<f64>>| {
        let (h, converged) = cycle(m, &mut s, w, Phase::Levels, signs.as_deref());
        let mut history = vec![zero];
        history.extend(h);
        Run {
            state: s,
            history,
            converged,
            signs,
            patterns: Vec::new(),
        }
    };
    match start {
        Start::Numerical => {
            let (s, last) = match numeric_exact(m, w) {
                Some(s) => {
                    let last = measures(m.vars, &s, w);
                    (s, last)
                }
                None => first(Phase::Numeric),
            };
            levels(s, last, None)
        }
        Start::Random => levels(init.clone(), measures(m.vars, &init, w), None),
        Start::Signs(signs) => {
            let (s, last) = first(Phase::Nominal);
            levels(s, last, Some(signs.clone()))
        }
        Start::Multi(None) => {
            let ord: Vec<usize> = (1..m.vars.len())
                .filter(|&j| ordered(&m.vars[j].q))
                .collect();
            let zero = measures(m.vars, &init, w);
            let mut best: Option<Run> = None;
            let mut patterns = Vec::new();
            for mask in 0..1usize << ord.len() {
                let mut signs = vec![1.0; m.vars.len()];
                for (k, &j) in ord.iter().enumerate() {
                    if mask >> k & 1 == 1 {
                        signs[j] = -1.0;
                    }
                }
                let run = levels(init.clone(), zero, Some(signs.clone()));
                patterns.push((signs, regu_fit(&run)));
                if best
                    .as_ref()
                    .is_none_or(|b| regu_fit(&run) > regu_fit(b) + 1e-12)
                {
                    best = Some(run);
                }
            }
            let mut best = best.expect("at least one pattern");
            best.patterns = patterns;
            best
        }
        Start::Multi(Some(threshold)) => {
            let ord: Vec<usize> = (1..m.vars.len())
                .filter(|&j| ordered(&m.vars[j].q))
                .collect();
            let (nominal, last) = first(Phase::Nominal);
            // the share of variance each ordered predictor loses to its order restriction
            let mut one = nominal.clone();
            let ds: Vec<Vec<f64>> = m.vars.iter().map(|v| category_weights(v, w)).collect();
            let positive = vec![1.0; m.vars.len()];
            iterate_once(m, &mut one, w, &ds, Phase::Levels, Some(&positive));
            let allowed: Vec<usize> = ord
                .iter()
                .copied()
                .filter(|&j| {
                    let before = nominal.b[j].powi(2);
                    before > 0.0 && 100.0 * (before - one.b[j].powi(2)) / before > *threshold
                })
                .collect();
            let mut patterns = Vec::new();
            let mut try_signs = |signs: Vec<f64>| {
                let run = levels(nominal.clone(), last, Some(signs.clone()));
                patterns.push((signs, regu_fit(&run)));
                run
            };
            let mut best = try_signs(positive.clone());
            let mut negative: Vec<usize> = Vec::new();
            loop {
                let mut round: Option<(usize, Run)> = None;
                for &j in allowed.iter().filter(|j| !negative.contains(j)) {
                    let mut signs = positive.clone();
                    for &k in negative.iter().chain(std::iter::once(&j)) {
                        signs[k] = -1.0;
                    }
                    let run = try_signs(signs);
                    if round
                        .as_ref()
                        .is_none_or(|(_, r)| regu_fit(&run) > regu_fit(r))
                    {
                        round = Some((j, run));
                    }
                }
                match round {
                    Some((j, run)) if regu_fit(&run) > regu_fit(&best) + 1e-12 => {
                        negative.push(j);
                        best = run;
                    }
                    _ => break,
                }
            }
            best.patterns = patterns;
            best
        }
    }
}

/// The value of `code` under the quantification `y` fitted on the `present` categories: its
/// own value, else interpolated (numeric and spline levels) or extrapolated (numeric).
fn estimate(q: &Quant, y: &[f64], present: &[bool], code: f64) -> Option<f64> {
    if let Ok(r) = q.values.binary_search_by(|v| v.total_cmp(&code))
        && present[r]
    {
        return Some(y[r]);
    }
    if !matches!(q.kind, Kind::Numeric | Kind::Spline { .. }) {
        return None;
    }
    let refs: Vec<usize> = (0..y.len())
        .filter(|&r| present[r] && Some(r) != q.extra)
        .collect();
    let line = |a: usize, b: usize| {
        y[a] + (y[b] - y[a]) * (code - q.values[a]) / (q.values[b] - q.values[a])
    };
    let below = refs.iter().rev().find(|&&r| q.values[r] < code);
    let above = refs.iter().find(|&&r| q.values[r] > code);
    match (below, above) {
        (Some(&a), Some(&b)) => Some(line(a, b)),
        _ if q.kind == Kind::Numeric && refs.len() >= 2 => {
            Some(line(refs[0], refs[refs.len() - 1]))
        }
        _ => None,
    }
}

/// The quantified response and the prediction of a case with these codes.
fn predict_codes(
    vars: &[Var],
    s: &State,
    present: &[Vec<bool>],
    codes: &[Option<f64>],
) -> (Option<f64>, Option<f64>) {
    let value = |j: usize| estimate(&vars[j].q, &s.y[j], &present[j], codes[j]?);
    let prediction = (1..vars.len())
        .filter(|&j| s.b[j] != 0.0)
        .map(|j| value(j).map(|y| s.b[j] * y))
        .sum::<Option<f64>>();
    (value(0), prediction)
}

fn present(vars: &[Var], w: &[f64]) -> Vec<Vec<bool>> {
    vars.iter()
        .map(|v| category_weights(v, w).iter().map(|d| *d > 0.0).collect())
        .collect()
}

/// The squared errors of the active cases `cases` under a fit on the weights `w`.
fn held_out(vars: &[Var], s: &State, w: &[f64], cases: &[usize]) -> Vec<(usize, f64)> {
    let pres = present(vars, w);
    cases
        .iter()
        .filter_map(|&i| {
            let codes: Vec<Option<f64>> = vars.iter().map(|v| Some(v.q.values[v.h[i]])).collect();
            match predict_codes(vars, s, &pres, &codes) {
                (Some(y), Some(p)) => Some((i, (y - p).powi(2))),
                _ => None,
            }
        })
        .collect()
}

/// Correlations, OLS coefficients and tolerances of the quantified predictors.
struct Stats {
    /// Zero-order correlation of each predictor with the response.
    zero: Vec<f64>,
    /// Correlations of the quantified predictors.
    r: Matrix,
    tolerance: Vec<f64>,
    /// The OLS coefficients on the predictors with nonzero coefficients, and their R square.
    ols: Vec<f64>,
    ols_r2: f64,
}

fn columns(vars: &[Var], s: &State) -> Vec<Vec<f64>> {
    vars.iter()
        .zip(&s.y)
        .map(|(v, y)| v.h.iter().map(|&r| y[r]).collect())
        .collect()
}

/// The inverse of the correlations among `keep`, as tolerances (1 over the diagonal) and the
/// OLS coefficients on `zero`.
fn tolerances(r: &Matrix, zero: &[f64], keep: &[usize]) -> (Vec<f64>, Vec<f64>) {
    let p = r.len();
    let (mut tol, mut ols) = (vec![f64::NAN; p], vec![0.0; p]);
    let sub: Matrix = keep
        .iter()
        .map(|&a| keep.iter().map(|&b| r[a][b]).collect())
        .collect();
    if let Some((inv, _)) = inverse(&sub) {
        for (x, &a) in keep.iter().enumerate() {
            tol[a] = 1.0 / inv[x][x];
            ols[a] = keep
                .iter()
                .enumerate()
                .map(|(y, &b)| inv[x][y] * zero[b])
                .sum();
        }
    }
    (tol, ols)
}

fn stats(vars: &[Var], s: &State, w: &[f64]) -> Stats {
    let all = correlations(&columns(vars, s), w);
    let p = vars.len() - 1;
    let zero: Vec<f64> = (1..=p).map(|j| all[0][j]).collect();
    let r: Matrix = (1..=p)
        .map(|a| (1..=p).map(|b| all[a][b]).collect())
        .collect();
    let keep: Vec<usize> = (0..p).filter(|&k| s.b[k + 1] != 0.0).collect();
    let (tolerance, ols) = tolerances(&r, &zero, &keep);
    let ols_r2 = ols.iter().zip(&zero).map(|(a, b)| a * b).sum();
    Stats {
        zero,
        r,
        tolerance,
        ols,
        ols_r2,
    }
}

/// Degrees of freedom of a predictor's quantification.
fn degrees(var: &Var, y: &[f64], d: &[f64]) -> f64 {
    match var.q.kind {
        Kind::Numeric => 1.0,
        Kind::Spline { ordinal } => {
            let basis = var.q.basis.as_ref().expect("a spline basis");
            let k = basis[0].len() - 1;
            if !ordinal {
                return k as f64;
            }
            // the zero coefficients of the rising spline through the final values
            let keep: Vec<usize> = (0..y.len())
                .filter(|&r| d[r] > 0.0 && Some(r) != var.q.extra)
                .collect();
            let total: f64 = keep.iter().map(|&r| d[r]).sum();
            let mean =
                |f: &dyn Fn(usize) -> f64| keep.iter().map(|&r| d[r] * f(r)).sum::<f64>() / total;
            let means: Vec<f64> = (1..=k).map(|c| mean(&|r| basis[r][c])).collect();
            let ybar = mean(&|r| y[r]);
            let rows: Matrix = keep
                .iter()
                .map(|&r| {
                    (1..=k)
                        .map(|c| (basis[r][c] - means[c - 1]) * d[r].sqrt())
                        .collect()
                })
                .collect();
            let t: Vec<f64> = keep.iter().map(|&r| (y[r] - ybar) * d[r].sqrt()).collect();
            let coef = nnls(&rows, &t);
            coef.iter().filter(|c| **c > 1e-8).count() as f64
        }
        Kind::Ordinal | Kind::Nominal | Kind::Multiple => {
            let mut values: Vec<f64> = y
                .iter()
                .zip(d)
                .filter(|(_, d)| **d > 0.0)
                .map(|(y, _)| (y * 1e7).round())
                .collect();
            values.sort_by(f64::total_cmp);
            values.dedup();
            values.len().saturating_sub(1) as f64
        }
    }
}

/// The penalties of every model: each ridge value with each lasso value.
fn penalties(spec: &Option<(CatregPenalty, Option<String>)>) -> Vec<Penalty> {
    let range = |(a, b, c): (f64, f64, f64)| -> Vec<f64> {
        match c > 0.0 {
            true => (0..)
                .map(|k| a + k as f64 * c)
                .take_while(|x| *x <= b + c * 1e-9)
                .collect(),
            false => vec![a],
        }
    };
    match spec {
        None => vec![Penalty::default()],
        Some((CatregPenalty::Ridge(r), _)) => range(*r)
            .into_iter()
            .map(|ridge| Penalty { ridge, lasso: 0.0 })
            .collect(),
        Some((CatregPenalty::Lasso(l), _)) => range(*l)
            .into_iter()
            .map(|lasso| Penalty { ridge: 0.0, lasso })
            .collect(),
        Some((CatregPenalty::Enet(r, l), _)) => range(*r)
            .into_iter()
            .flat_map(|ridge| {
                range(*l)
                    .into_iter()
                    .map(move |lasso| Penalty { ridge, lasso })
            })
            .collect(),
    }
}

/// The standardized sum of coefficients: against the OLS coefficients on the same
/// quantifications, squared for ridge, absolute otherwise.
fn standardized_sum(s: &State, st: &Stats, ridge_only: bool) -> f64 {
    let b = &s.b[1..];
    let f = |x: f64| if ridge_only { x * x } else { x.abs() };
    let top: f64 = st.ols.iter().map(|x| f(*x)).sum();
    match top > 0.0 {
        true => b.iter().map(|x| f(*x)).sum::<f64>() / top,
        false => f64::NAN,
    }
}

fn plain_table(
    title: &str,
    columns: Vec<String>,
    rows: Vec<Vec<Cell>>,
    caption: Option<String>,
) -> Table {
    Table {
        title: title.into(),
        columns,
        rows,
        row_headers: 1,
        caption,
        ..Table::default()
    }
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

impl Engine {
    /// Draws n case indices with replacement: the count of each.
    fn draw_counts(&self, n: usize) -> Vec<f64> {
        let mut counts = vec![0.0; n];
        for _ in 0..n {
            let k = ((self.random.borrow_mut().uniform() * n as f64) as usize).min(n - 1);
            counts[k] += 1.0;
        }
        counts
    }

    /// The prediction error by cross validation or the .632 bootstrap, with its standard error.
    fn resampled_error(
        &self,
        m: &Model,
        w: &[f64],
        start: &Start,
        ape: f64,
        how: CatregResample,
    ) -> Result<(f64, f64), Error> {
        let n = w.len();
        let n_w: f64 = w.iter().sum();
        match how {
            CatregResample::None => Ok((f64::NAN, f64::NAN)),
            CatregResample::Crossval(k) => {
                if k > n {
                    return Err(Error(format!(
                        "{WHO}: CROSSVAL asks for {k} folds, but there are {n} active cases"
                    )));
                }
                // a random order dealt round the folds
                let mut order: Vec<(f64, usize)> = (0..n)
                    .map(|i| (self.random.borrow_mut().uniform(), i))
                    .collect();
                order.sort_by(|a, b| a.0.total_cmp(&b.0));
                let (mut sum, mut sq) = (0.0, 0.0);
                for fold in 0..k {
                    let cases: Vec<usize> =
                        order.iter().skip(fold).step_by(k).map(|x| x.1).collect();
                    let mut wk = w.to_vec();
                    cases.iter().for_each(|&i| wk[i] = 0.0);
                    let run = fit(m, &wk, start);
                    for (i, e) in held_out(m.vars, &run.state, &wk, &cases) {
                        sum += w[i] * e;
                        sq += w[i] * e * e;
                    }
                }
                Ok((sum / n_w, (sq / (n_w * n_w)).sqrt()))
            }
            CatregResample::Bootstrap(b) => {
                let (mut sum, mut count) = (vec![0.0; n], vec![0usize; n]);
                for _ in 0..b {
                    let counts = self.draw_counts(n);
                    let wb: Vec<f64> = w.iter().zip(&counts).map(|(a, c)| a * c).collect();
                    let out: Vec<usize> = (0..n).filter(|&i| counts[i] == 0.0).collect();
                    let run = fit(m, &wb, start);
                    for (i, e) in held_out(m.vars, &run.state, &wb, &out) {
                        sum[i] += e;
                        count[i] += 1;
                    }
                }
                let used: Vec<usize> = (0..n).filter(|&i| count[i] > 0).collect();
                let n1: f64 = used.iter().map(|&i| w[i]).sum();
                if n1 == 0.0 {
                    return Err(Error(format!(
                        "{WHO}: no case could be predicted from a bootstrap sample without it; use more samples"
                    )));
                }
                let err: Vec<f64> = (0..n).map(|i| sum[i] / count[i].max(1) as f64).collect();
                let err1 = used.iter().map(|&i| w[i] * err[i]).sum::<f64>() / n1;
                let se = (used
                    .iter()
                    .map(|&i| w[i] * (err[i] - err1).powi(2))
                    .sum::<f64>()
                    / (n1 * n1))
                    .sqrt();
                Ok((ape + 0.632 * (err1 - ape), se))
            }
        }
    }

    /// Standard errors of the coefficients from bootstrap samples, each started from the fit.
    fn coefficient_errors(&self, m: &Model, w: &[f64], s: &State) -> Vec<f64> {
        let p = m.vars.len();
        let (mut sum, mut sq) = (vec![0.0; p], vec![0.0; p]);
        for _ in 0..SE_SAMPLES {
            let counts = self.draw_counts(w.len());
            let wb: Vec<f64> = w.iter().zip(&counts).map(|(a, c)| a * c).collect();
            let mut sb = s.clone();
            cycle(m, &mut sb, &wb, Phase::Levels, None);
            for j in 1..p {
                sum[j] += sb.b[j];
                sq[j] += sb.b[j] * sb.b[j];
            }
        }
        let k = SE_SAMPLES as f64;
        (0..p)
            .map(|j| ((sq[j] - sum[j] * sum[j] / k) / (k - 1.0)).max(0.0).sqrt())
            .collect()
    }

    pub(crate) fn catreg(&mut self, command: &Catreg) -> Result<Vec<Table>, Error> {
        let mut warnings: Vec<String> = command
            .repeated
            .iter()
            .map(|s| format!("{WHO}: {s} appears more than once; the last one counts"))
            .collect();
        // MISSING: LISTWISE unless the subcommand says otherwise
        let mut common: ScalingCommon = command.common.clone();
        common
            .missing
            .insert(0, (vec![VarItem::All], McaMissing::Listwise));
        let dep_items = vec![VarItem::Name(command.dependent.0.clone())];
        let mut lists: Vec<(&[VarItem], f64)> = vec![(dep_items.as_slice(), 1.0)];
        lists.extend(
            command
                .predictors
                .iter()
                .map(|(items, _)| (items.as_slice(), 1.0)),
        );
        let prep: Prepared = self.prepare_scaling(WHO, &common, &lists)?;
        warnings.extend(prep.warnings.iter().cloned());
        let (avars, coded) = (&prep.avars, &prep.coded);
        let w = &prep.w;
        let n_w = prep.n_w;
        let n = prep.active.len();
        let p = avars.len() - 1;
        if p > 200 {
            return Err(Error(format!(
                "{WHO} takes at most 200 predictors, got {p}"
            )));
        }
        if n <= p + 1 {
            return Err(Error(format!(
                "{WHO} needs more active cases than predictors plus 1: {n} cases, {p} predictors"
            )));
        }
        let titles: Vec<String> = avars.iter().map(|v| coded[v.coded].title.clone()).collect();
        let vars: Vec<Var> = avars
            .iter()
            .map(|v| {
                let level = match v.list {
                    0 => command.dependent.1,
                    k => command.predictors[k - 1].1,
                };
                Var {
                    q: Quant::new(v, level, n_w),
                    h: v.h
                        .iter()
                        .map(|h| h.expect("no gaps among active cases"))
                        .collect(),
                }
            })
            .collect();
        if let Some(j) = vars.iter().position(|v| v.q.values.len() < 2) {
            return Err(Error(format!(
                "{WHO}: {} has only one category among the active cases",
                prep.analysis_names[j]
            )));
        }
        let ord_count = vars.iter().skip(1).filter(|v| ordered(&v.q)).count();
        let start = match &command.initial {
            CatregInitial::Numerical => Start::Numerical,
            CatregInitial::Random => Start::Random,
            CatregInitial::Multistart(..) | CatregInitial::Fixsigns(..) if ord_count == 0 => {
                return Err(Error(format!(
                    "{WHO}: MULTISTART and FIXSIGNS need a predictor with LEVEL=ORDI or SPORD"
                )));
            }
            CatregInitial::Multistart(None, _) if ord_count > 20 => {
                return Err(Error(format!(
                    "{WHO}: MULTISTART(ALL) with {ord_count} ordered predictors means 2^{ord_count} starts; give a threshold instead"
                )));
            }
            CatregInitial::Multistart(t, _) => Start::Multi(*t),
            CatregInitial::Fixsigns(row, file) => {
                let data = self.data_file(file)?;
                let signs: Vec<f64> = data
                    .variables()
                    .filter_map(|v| v.numeric().ok().map(|x| x.iter().nth(row - 1).flatten()))
                    .take(ord_count)
                    .map(|x| x.filter(|x| *x != 0.0).map(f64::signum))
                    .collect::<Option<Vec<f64>>>()
                    .filter(|s| s.len() == ord_count)
                    .ok_or_else(|| {
                        Error(format!(
                            "{WHO}: row {row} of {file} needs {ord_count} nonzero signs in its first numeric variables"
                        ))
                    })?;
                let mut full = vec![1.0; vars.len()];
                let ord: Vec<usize> = (1..vars.len()).filter(|&j| ordered(&vars[j].q)).collect();
                for (j, sg) in ord.iter().zip(signs) {
                    full[*j] = sg;
                }
                Start::Signs(full)
            }
        };

        // every model of the penalty grid
        let grid = penalties(&command.regularization);
        let model = |penalty: Penalty| Model {
            vars: &vars,
            penalty,
            max: command.common.max_iterations,
            criterion: command.common.criterion,
        };
        let sup_cases: Vec<usize> = (0..prep.n_tot)
            .filter(|&c| !prep.excluded[c] && prep.active.binary_search(&c).is_err())
            .collect();
        let case_codes =
            |c: usize| -> Vec<Option<f64>> { avars.iter().map(|v| v.codes[c]).collect() };
        let full_present = present(&vars, w);
        // the test error of the supplementary cases and the training error with their SEs
        let errors = |s: &State| -> ((f64, f64), Option<(f64, f64)>) {
            let pred = predict(&vars, s);
            let e: Vec<f64> = (0..n)
                .map(|i| (s.y[0][vars[0].h[i]] - pred[i]).powi(2))
                .collect();
            let train = e.iter().zip(w).map(|(e, w)| e * w).sum::<f64>() / n_w;
            let train_se =
                (e.iter().zip(w).map(|(e, w)| w * e * e).sum::<f64>() / (n_w * n_w)).sqrt();
            let test: Vec<f64> = sup_cases
                .iter()
                .filter_map(
                    |&c| match predict_codes(&vars, s, &full_present, &case_codes(c)) {
                        (Some(y), Some(p)) => Some((y - p).powi(2)),
                        _ => None,
                    },
                )
                .collect();
            let k = test.len() as f64;
            let test_stats = (!test.is_empty()).then(|| {
                let mean = test.iter().sum::<f64>() / k;
                (
                    mean,
                    (test.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / (k * k)).sqrt(),
                )
            });
            ((train, train_se), test_stats)
        };
        let resample = command.resample;
        let mut models: Vec<(Penalty, Run, f64, f64)> = Vec::new();
        for &penalty in &grid {
            let m = model(penalty);
            let run = fit(&m, w, &start);
            let again = match &run.signs {
                Some(s) => Start::Signs(s.clone()),
                None => start.clone(),
            };
            let ape = run.history.last().map_or(f64::NAN, |h| h.1);
            let (epe, se) = match resample {
                CatregResample::None => match errors(&run.state).1 {
                    Some((e, s)) => (e, s),
                    None => (f64::NAN, f64::NAN),
                },
                how => self.resampled_error(&m, w, &again, ape, how)?,
            };
            models.push((penalty, run, epe, se));
        }
        let regularized = command.regularization.is_some();
        let selectable = resample != CatregResample::None || !sup_cases.is_empty();
        let chosen = match (models.len(), selectable) {
            (1, _) => Some(0),
            (_, true) => (0..models.len()).min_by(|&a, &b| models[a].2.total_cmp(&models[b].2)),
            (_, false) => None,
        };

        let print = &command.print;
        let mut tables = vec![prep.case_summary()];
        let mut charts: Vec<Chart> = Vec::new();
        let pred_names: Vec<String> = titles[1..].to_vec();
        let ridge_only = matches!(command.regularization, Some((CatregPenalty::Ridge(_), _)));

        // the table of every regularized model
        if (print.regu || command.plot.regu.is_some()) && !regularized {
            warnings.push(format!(
                "{WHO}: REGU on PRINT or PLOT needs REGULARIZATION; it is ignored"
            ));
        }
        if regularized {
            let has_ridge = !matches!(command.regularization, Some((CatregPenalty::Lasso(_), _)));
            let has_lasso = !ridge_only;
            let has_error = models.iter().any(|m| m.2.is_finite());
            let mut columns = vec!["Model".to_string()];
            if has_ridge {
                columns.push("Ridge Penalty".into());
            }
            if has_lasso {
                columns.push("Lasso Penalty".into());
            }
            columns.push("R Square (1 - APE)".into());
            if has_error {
                columns.extend(names(&["Prediction Error", "Std. Error"]));
            }
            columns.push("Standardized Sum of Coefficients".into());
            columns.extend(pred_names.iter().cloned());
            let mut rows = Vec::new();
            let mut sums = Vec::new();
            for (k, (pen, run, epe, se)) in models.iter().enumerate() {
                let st = stats(&vars, &run.state, w);
                let sum = standardized_sum(&run.state, &st, ridge_only);
                sums.push(sum);
                let mut row = vec![text((k + 1).to_string())];
                if has_ridge {
                    row.push(Cell::Number(pen.ridge));
                }
                if has_lasso {
                    row.push(Cell::Number(pen.lasso));
                }
                row.push(Cell::Number(regu_fit(run)));
                if has_error {
                    row.extend([num(*epe), num(*se)]);
                }
                row.push(num(sum));
                row.extend(run.state.b[1..].iter().map(|b| Cell::Number(*b)));
                rows.push(row);
            }
            if let Some((_, Some(file))) = &command.regularization {
                let mut out = Dataset::new(models.len());
                let col = |f: &dyn Fn(usize) -> f64| {
                    (0..models.len()).map(|k| Some(f(k))).collect::<Vec<_>>()
                };
                out.set_numeric_values("RIDGE", col(&|k| models[k].0.ridge))?;
                out.set_numeric_values("LASSO", col(&|k| models[k].0.lasso))?;
                out.set_numeric_values("RSQ", col(&|k| regu_fit(&models[k].1)))?;
                if has_error {
                    out.set_numeric_values("EPE", col(&|k| models[k].2))?;
                    out.set_numeric_values("SE", col(&|k| models[k].3))?;
                }
                out.set_numeric_values("SUMCOEF", col(&|k| sums[k]))?;
                for (j, name) in prep.analysis_names.iter().enumerate().skip(1) {
                    out.set_numeric_values(name, col(&|k| models[k].1.state.b[j]))?;
                }
                self.write_data_file(file, out)?;
            }
            if print.regu {
                tables.push(plain_table(
                    "Regularized Models",
                    columns,
                    rows,
                    Some(match chosen {
                        Some(k) if models.len() > 1 => format!("Model {} has the least prediction error; the other output describes it.", k + 1),
                        _ => "R square is 1 less the apparent prediction error; the standardized sum compares the coefficients with the least squares ones on the same quantifications.".into(),
                    }),
                ));
            }
            if let Some(listed) = &command.plot.regu {
                let mut ridges: Vec<f64> = models.iter().map(|m| m.0.ridge).collect();
                ridges.dedup();
                let enet = matches!(command.regularization, Some((CatregPenalty::Enet(..), _)));
                let picked: Vec<f64> = match (enet, listed.is_empty()) {
                    (true, false) => {
                        let mut out = Vec::new();
                        for v in listed {
                            match ridges.iter().find(|r| (*r - v).abs() < 1e-9) {
                                Some(r) => out.push(*r),
                                None => warnings.push(format!(
                                    "{WHO}: no model has the ridge penalty {v}; its plot is skipped"
                                )),
                            }
                        }
                        out
                    }
                    _ => ridges.clone(),
                };
                let groups_for = |keep: &dyn Fn(&Penalty) -> bool| -> Vec<PointGroup> {
                    (1..vars.len())
                        .map(|j| PointGroup {
                            name: pred_names[j - 1].clone(),
                            points: models
                                .iter()
                                .zip(&sums)
                                .filter(|(m, _)| keep(&m.0))
                                .map(|(m, s)| Point {
                                    x: *s,
                                    y: m.1.state.b[j],
                                    z: None,
                                    label: None,
                                })
                                .collect(),
                        })
                        .collect()
                };
                let mut chart_of = |title: String, groups: Vec<PointGroup>| {
                    charts.push(Chart::plot(
                        ChartKind::Scatter,
                        title,
                        Axis::titled("Standardized Sum of Coefficients"),
                        Axis::titled("Coefficients"),
                        Plot::Points {
                            groups,
                            lines: Vec::new(),
                            z_axis: None,
                        },
                    ))
                };
                match enet {
                    true => {
                        for r in picked {
                            chart_of(
                                format!("Elastic Net Paths, Ridge Penalty {r}"),
                                groups_for(&|p| (p.ridge - r).abs() < 1e-12),
                            );
                        }
                    }
                    false => chart_of(
                        format!("{} Paths", if ridge_only { "Ridge" } else { "Lasso" }),
                        groups_for(&|_| true),
                    ),
                }
            }
        }
        let Some(chosen) = chosen else {
            if command.save_trdata.is_some()
                || command.save_pred.is_some()
                || command.save_res.is_some()
                || command.outfile_trdata.is_some()
            {
                warnings.push(format!(
                    "{WHO}: with several regularized models and neither RESAMPLE nor supplementary cases, SAVE and OUTFILE TRDATA are ignored"
                ));
            }
            self.write_discrdata(command, &prep)?;
            for warning in warnings {
                self.warn(warning);
            }
            self.charts.extend(charts);
            return Ok(tables);
        };
        let (penalty, run, epe, se) = &models[chosen];
        let s = &run.state;
        if !run.converged {
            warnings.push(format!(
                "{WHO}: the fit did not converge within {} iterations",
                command.common.max_iterations
            ));
        }
        let m = model(*penalty);

        if let Some(items) = &print.descrip {
            tables.extend(prep.descriptives(WHO, items)?);
        }
        if print.history {
            let rows = run
                .history
                .iter()
                .enumerate()
                .map(|(k, (r2, ape))| {
                    vec![
                        Cell::Number(k as f64),
                        Cell::Number(*r2),
                        Cell::Number(*ape),
                        match k {
                            0 => Cell::Empty,
                            _ => Cell::Number(run.history[k - 1].1 - ape),
                        },
                    ]
                })
                .collect();
            tables.push(plain_table(
                "Iteration History",
                names(&["Iteration Number", "R Square", "Apparent Prediction Error", "Decrease"]),
                rows,
                Some(match command.initial {
                    CatregInitial::Numerical => "Iteration 0 is the end of the first phase, in which every variable is numeric.".into(),
                    CatregInitial::Random => "Iteration 0 is the start: standardized codes with the correlations as coefficients.".into(),
                    _ => "Iteration 0 is the end of the first phase, in which every variable is nominal.".into(),
                }),
            ));
        }
        if let CatregInitial::Multistart(_, file) = &command.initial {
            let ord: Vec<usize> = (1..vars.len()).filter(|&j| ordered(&vars[j].q)).collect();
            let mut out = Dataset::new(run.patterns.len());
            for (k, &j) in ord.iter().enumerate() {
                out.set_numeric_values(
                    &format!("SIGN{}", k + 1),
                    run.patterns.iter().map(|(s, _)| Some(s[j])).collect(),
                )?;
            }
            out.set_numeric_values("RSQ", run.patterns.iter().map(|(_, f)| Some(*f)).collect())?;
            self.write_data_file(file, out)?;
        }

        // summary statistics
        let (r2, ape) = measures(&vars, s, w);
        let ds: Vec<Vec<f64>> = vars.iter().map(|v| category_weights(v, w)).collect();
        let df: Vec<f64> = (1..vars.len())
            .map(|j| degrees(&vars[j], &s.y[j], &ds[j]))
            .collect();
        let df_sum: f64 = df.iter().sum();
        let st = stats(&vars, s, w);
        let adjusted = |k: f64| 1.0 - (1.0 - r2) * (n_w - 1.0) / (n_w - 1.0 - k);
        if print.r {
            tables.push(plain_table(
                "Model Summary",
                names(&[
                    "",
                    "Multiple R",
                    "R Square",
                    "Adjusted R Square",
                    "Adjusted R Square (Optimal Scaling df)",
                    "Apparent Prediction Error",
                ]),
                vec![vec![
                    text("Standardized Data"),
                    Cell::Number(r2.sqrt()),
                    Cell::Number(r2),
                    num(adjusted(p as f64)),
                    num(adjusted(df_sum)),
                    Cell::Number(ape),
                ]],
                Some(format!(
                    "Response: {}. Predictors: {}.",
                    titles[0],
                    pred_names.join(", ")
                )),
            ));
            let ((train, train_se), test) = errors(s);
            let mut rows = Vec::new();
            if resample != CatregResample::None || test.is_some() {
                rows.push(vec![
                    text("Training data (apparent)"),
                    Cell::Number(train),
                    Cell::Number(train_se),
                ]);
            }
            if let Some((e, se)) = test {
                rows.push(vec![
                    text("Supplementary cases (test)"),
                    Cell::Number(e),
                    Cell::Number(se),
                ]);
            }
            match resample {
                CatregResample::Crossval(k) => rows.push(vec![
                    text(format!("Cross validation, {k} folds")),
                    num(*epe),
                    num(*se),
                ]),
                CatregResample::Bootstrap(b) => rows.push(vec![
                    text(format!(".632 bootstrap, {b} samples")),
                    num(*epe),
                    num(*se),
                ]),
                CatregResample::None => {}
            }
            if !rows.is_empty() {
                tables.push(plain_table(
                    "Prediction Error",
                    names(&["", "Prediction Error", "Std. Error"]),
                    rows,
                    Some("Mean squared error of the standardized quantified response.".into()),
                ));
            }
        }
        if print.anova {
            for (title, k) in [("ANOVA", p as f64), ("ANOVA (Optimal Scaling df)", df_sum)] {
                let (ss_reg, ss_res) = (n_w * r2, n_w * (1.0 - r2));
                let df_res = n_w - 1.0 - k;
                let (ms_reg, ms_res) = (ss_reg / k, ss_res / df_res);
                let f = ms_reg / ms_res;
                tables.push(plain_table(
                    title,
                    names(&["", "Sum of Squares", "df", "Mean Square", "F", "Sig."]),
                    vec![
                        vec![
                            text("Regression"),
                            Cell::Number(ss_reg),
                            Cell::Number(k),
                            num(ms_reg),
                            num(f),
                            num(f_p(f, k, df_res)),
                        ],
                        vec![
                            text("Residual"),
                            Cell::Number(ss_res),
                            Cell::Number(df_res),
                            num(ms_res),
                            Cell::Empty,
                            Cell::Empty,
                        ],
                        vec![
                            text("Total"),
                            Cell::Number(n_w),
                            Cell::Number(n_w - 1.0),
                            Cell::Empty,
                            Cell::Empty,
                            Cell::Empty,
                        ],
                    ],
                    None,
                ));
            }
        }
        if print.coeff {
            let se_beta = self.coefficient_errors(&m, w, s);
            let df_res = n_w - 1.0 - df_sum;
            let rows = (1..vars.len())
                .map(|j| {
                    let f = (s.b[j] / se_beta[j]).powi(2);
                    vec![
                        text(pred_names[j - 1].clone()),
                        Cell::Number(s.b[j]),
                        num(se_beta[j]),
                        Cell::Number(df[j - 1]),
                        num(f),
                        num(f_p(f, df[j - 1], df_res)),
                    ]
                })
                .collect();
            tables.push(plain_table(
                "Coefficients",
                names(&["", "Beta", &format!("Std. Error (Bootstrap, {SE_SAMPLES})"), "df", "F", "Sig."]),
                rows,
                Some(format!("Response: {}. F is the squared ratio of beta to its standard error, with the predictor's and the residual optimal scaling degrees of freedom.", titles[0])),
            ));
            // before transformation: the codes the analysis used
            let codes: Vec<Vec<f64>> = vars
                .iter()
                .map(|v| v.h.iter().map(|&r| v.q.values[r]).collect())
                .collect();
            let orig = correlations(&codes, w);
            let o: Matrix = (1..=p)
                .map(|a| (1..=p).map(|b| orig[a][b]).collect())
                .collect();
            let keep: Vec<usize> = (0..p).filter(|&k| s.b[k + 1] != 0.0).collect();
            let (tol_before, _) = tolerances(&o, &vec![0.0; p], &keep);
            let (beta, fit_r2): (Vec<f64>, f64) = match regularized {
                true => (st.ols.clone(), st.ols_r2),
                false => (s.b[1..].to_vec(), r2),
            };
            let mut low = Vec::new();
            let rows = (0..p)
                .map(|k| {
                    let t = st.tolerance[k];
                    if t < 0.0001 {
                        low.push(pred_names[k].clone());
                    }
                    let usable = t >= 1e-12;
                    let partial = beta[k] / ((1.0 - fit_r2) / t + beta[k] * beta[k]).sqrt();
                    let part = beta[k] * t.sqrt();
                    vec![
                        text(pred_names[k].clone()),
                        num(st.zero[k]),
                        if usable { num(partial) } else { Cell::Empty },
                        if usable { num(part) } else { Cell::Empty },
                        match regularized {
                            true => Cell::Empty,
                            false => num(s.b[k + 1] * st.zero[k] / r2),
                        },
                        num(t),
                        num(tol_before[k]),
                    ]
                })
                .collect();
            let mut caption = String::from(
                "Importance is Pratt's measure, beta times the zero-order correlation over R square.",
            );
            if regularized {
                caption.push_str(" With a penalty, partial and part correlations come from the least squares coefficients on the same quantifications, and importance is not shown.");
            }
            if !low.is_empty() {
                caption.push_str(&format!(" Tolerance below 0.0001 for {}.", low.join(", ")));
            }
            tables.push(plain_table(
                "Correlations and Tolerance",
                names(&[
                    "",
                    "Zero-Order",
                    "Partial",
                    "Part",
                    "Importance",
                    "Tolerance After Transformation",
                    "Tolerance Before Transformation",
                ]),
                rows,
                Some(caption),
            ));
        }
        let unit: Vec<(usize, f64)> = (0..p).map(|k| (k, 1.0)).collect();
        if print.corr {
            tables.push(correlation_table(
                "Correlations Transformed Variables".into(),
                &pred_names,
                &st.r,
                &unit,
            ));
        }
        if print.ocorr {
            let codes: Vec<Vec<f64>> = vars[1..]
                .iter()
                .map(|v| v.h.iter().map(|&r| v.q.values[r]).collect())
                .collect();
            tables.push(correlation_table(
                "Correlations Original Variables".into(),
                &pred_names,
                &correlations(&codes, w),
                &unit,
            ));
        }
        let analysis_names = &prep.analysis_names;
        // the quantified value of every case, active or supplementary
        let value_of = |j: usize, c: usize| -> Option<f64> {
            if prep.excluded[c] {
                return None;
            }
            estimate(&vars[j].q, &s.y[j], &full_present[j], avars[j].codes[c]?)
        };
        if let Some(items) = &print.quant {
            let picked = match items.is_empty() {
                true => analysis_names.clone(),
                false => resolve(WHO, items, analysis_names, analysis_names)?,
            };
            for name in picked {
                let j = prep.analysis_index(&name);
                let c = &coded[avars[j].coded];
                let mut rows: Vec<Vec<Cell>> = avars[j]
                    .cats
                    .iter()
                    .enumerate()
                    .map(|(r, code)| {
                        vec![
                            text(c.label(*code, None)),
                            Cell::Number(ds[j][r]),
                            Cell::Number(s.y[j][r]),
                        ]
                    })
                    .collect();
                let mut extra: Vec<i64> = sup_cases
                    .iter()
                    .filter_map(|&k| avars[j].codes[k].map(|x| x as i64))
                    .filter(|x| avars[j].cats.binary_search(x).is_err())
                    .collect();
                extra.sort_unstable();
                extra.dedup();
                for code in extra {
                    rows.push(vec![
                        text(format!("{} (supplementary)", c.label(code, None))),
                        Cell::Empty,
                        estimate(&vars[j].q, &s.y[j], &full_present[j], code as f64)
                            .map_or(Cell::Empty, Cell::Number),
                    ]);
                }
                tables.push(plain_table(
                    &format!("Quantifications: {}", c.title),
                    names(&["", "Frequency", "Quantification"]),
                    rows,
                    None,
                ));
            }
        }

        // charts
        if let Some((items, length)) = &command.plot.trans {
            for name in resolve(WHO, items, analysis_names, analysis_names)? {
                let j = prep.analysis_index(&name);
                let c = &coded[avars[j].coded];
                let categories = avars[j]
                    .cats
                    .iter()
                    .map(|code| Category {
                        label: c.label(*code, *length),
                        value: Cell::Number(*code as f64),
                        missing: false,
                        bin: None,
                    })
                    .collect();
                charts.push(Chart::lines(
                    format!("Transformation: {}", c.title),
                    Axis::titled("Category Indicators"),
                    Axis::titled("Quantifications"),
                    categories,
                    vec![Series {
                        name: "Quantification".into(),
                        statistic: Statistic::Value,
                        variable: None,
                        value: Cell::Empty,
                        values: s.y[j].clone(),
                    }],
                ));
            }
        }
        if let Some((items, _)) = &command.plot.resid {
            let pred = predict(&vars, s);
            for name in resolve(WHO, items, analysis_names, analysis_names)? {
                let j = prep.analysis_index(&name);
                if j == 0 {
                    return Err(Error(format!(
                        "{WHO}: RESID plots predictors only; {name} is the response"
                    )));
                }
                let var = &vars[j];
                let residuals = (0..n)
                    .map(|i| Point {
                        x: var.q.values[var.h[i]],
                        y: s.y[0][vars[0].h[i]] - pred[i] + s.b[j] * s.y[j][var.h[i]],
                        z: None,
                        label: Some((prep.active[i] + 1).to_string()),
                    })
                    .collect();
                let quantified = (0..var.q.values.len())
                    .map(|r| Point {
                        x: var.q.values[r],
                        y: s.b[j] * s.y[j][r],
                        z: None,
                        label: None,
                    })
                    .collect();
                charts.push(Chart::plot(
                    ChartKind::Scatter,
                    format!("Residuals: {}", titles[j]),
                    Axis::titled("Category Indicators"),
                    Axis::titled("Residuals and Coefficient times Quantification"),
                    Plot::Points {
                        groups: vec![
                            PointGroup {
                                name: "Residuals".into(),
                                points: residuals,
                            },
                            PointGroup {
                                name: "Coefficient times Quantification".into(),
                                points: quantified,
                            },
                        ],
                        lines: Vec::new(),
                        z_axis: None,
                    },
                ));
            }
        }

        // saved variables and files
        let data = self.active()?;
        let prediction_of = |c: usize| -> (Option<f64>, Option<f64>) {
            if prep.excluded[c] {
                return (None, None);
            }
            predict_codes(&vars, s, &full_present, &case_codes(c))
        };
        let mut saves: Vec<(String, String, Vec<Option<f64>>)> = Vec::new();
        if let Some(root) = &command.save_trdata {
            let set = free_set(data, &|set| {
                (1..=avars.len())
                    .map(|k| format!("{root}{k}_{set}"))
                    .collect()
            });
            for (j, v) in avars.iter().enumerate() {
                saves.push((
                    format!("{root}{}_{set}", j + 1),
                    format!("Transformed {} ({WHO} {set})", coded[v.coded].name),
                    (0..prep.n_tot).map(|c| value_of(j, c)).collect(),
                ));
            }
        }
        if let Some(root) = &command.save_pred {
            let set = free_set(data, &|set| vec![format!("{root}_{set}")]);
            saves.push((
                format!("{root}_{set}"),
                format!("Predicted value ({WHO} {set})"),
                (0..prep.n_tot).map(|c| prediction_of(c).1).collect(),
            ));
        }
        if let Some(root) = &command.save_res {
            let set = free_set(data, &|set| vec![format!("{root}_{set}")]);
            saves.push((
                format!("{root}_{set}"),
                format!("Residual ({WHO} {set})"),
                (0..prep.n_tot)
                    .map(|c| match prediction_of(c) {
                        (Some(y), Some(p)) => Some(y - p),
                        _ => None,
                    })
                    .collect(),
            ));
        }
        let trdata = match &command.outfile_trdata {
            Some(file) => {
                let mut out = Dataset::new(prep.n_tot);
                for j in 0..avars.len() {
                    out.set_numeric_values(
                        &format!("TRA{}", j + 1),
                        (0..prep.n_tot).map(|c| value_of(j, c)).collect(),
                    )?;
                }
                Some((file.clone(), out))
            }
            None => None,
        };
        if !saves.is_empty() {
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                for (name, label, values) in saves {
                    data.set_numeric_values(&name, values)?;
                    data.set_variable_label(&name, Some(&label))?;
                    data.set_format(&name, "F8.4")?;
                    data.set_measure(&name, Measure::Scale)?;
                }
                Ok(())
            })?;
        }
        if let Some((file, out)) = trdata {
            self.write_data_file(&file, out)?;
        }
        self.write_discrdata(command, &prep)?;
        for warning in warnings {
            self.warn(warning);
        }
        self.charts.extend(charts);
        Ok(tables)
    }

    /// OUTFILE DISCRDATA: the codes the analysis used, under the variables' names.
    fn write_discrdata(&mut self, command: &Catreg, prep: &Prepared) -> Result<(), Error> {
        let Some(file) = &command.outfile_discrdata else {
            return Ok(());
        };
        let mut out = Dataset::new(prep.n_tot);
        for v in &prep.avars {
            let c = &prep.coded[v.coded];
            let values = (0..prep.n_tot)
                .map(|case| c.codes[case].filter(|_| !prep.excluded[case]))
                .collect();
            out.set_numeric_values(&c.name, values)?;
        }
        self.write_data_file(file, out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shrinking_follows_ridge_and_lasso() {
        let none = Penalty::default();
        assert_eq!(shrink(0.4, none), 0.4);
        assert!(
            (shrink(
                0.4,
                Penalty {
                    ridge: 1.0,
                    lasso: 0.0
                }
            ) - 0.2)
                .abs()
                < 1e-12
        );
        assert!(
            (shrink(
                -0.4,
                Penalty {
                    ridge: 0.0,
                    lasso: 0.2
                }
            ) + 0.3)
                .abs()
                < 1e-12
        );
        assert_eq!(
            shrink(
                0.05,
                Penalty {
                    ridge: 0.0,
                    lasso: 0.2
                }
            ),
            0.0
        );
        // elastic net: soft threshold, then the ridge division
        assert!(
            (shrink(
                0.5,
                Penalty {
                    ridge: 1.0,
                    lasso: 0.2
                }
            ) - 0.2)
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn penalty_grid_runs_from_start_to_stop() {
        let grid = penalties(&Some((CatregPenalty::Lasso((0.01, 3.8, 0.05)), None)));
        assert_eq!(grid.len(), 76);
        assert!((grid[75].lasso - 3.76).abs() < 1e-9);
        let enet = penalties(&Some((
            CatregPenalty::Enet((0.5, 2.5, 0.25), (1.0, 1.0, 0.0)),
            None,
        )));
        assert_eq!(enet.len(), 9);
    }
}
