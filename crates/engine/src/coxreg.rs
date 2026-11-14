//! COXREG: Cox proportional hazards regression by maximum partial likelihood, with strata,
//! categorical covariates, forward and backward stepwise selection, time-dependent covariates
//! from a TIME PROGRAM, the baseline survival, case statistics and survival curves.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (COXREG, TIME PROGRAM):
//! - A case needs valid time, status, strata and covariates and a positive weight; a negative
//!   time drops it. A censored case before the first event of its stratum adds nothing to the
//!   likelihood and is dropped too (it is counted on its own).
//! - Categorical covariates (CATEGORICAL, and every string) become k - 1 coded columns,
//!   DEVIATION with the last category as reference by default; a term's columns enter and
//!   leave together; columns that repeat the ones before them are left out.
//! - The partial likelihood treats tied event times as Breslow does: at each event time the
//!   weighted sum of the dying cases' linear predictors less their weight times the log of the
//!   weighted risk-set sum of exp(x'b). The covariates are centered at their weighted means.
//! - Newton-Raphson from b = 0 stops when the largest change of a coefficient relative to its
//!   previous value is below BCON, the log likelihood changes by less than LCON relative to its
//!   previous value, or after ITERATE iterations.
//! - Overall (score) test: the score statistic at b = 0 of the model's columns. Change from the
//!   previous step and block: differences of -2 log likelihood.
//! - Entry by the score statistic, removal by the conditional statistic, Wald or likelihood
//!   ratio, as in LOGISTIC REGRESSION.
//! - The baseline survival of each stratum is the maximum likelihood estimate given b: at each
//!   event time alpha solves sum over the dying of w e / (1 - alpha^e) = risk-set sum of w e
//!   (e = exp of the centered linear predictor), in closed form for one death; S0 is the
//!   product of the alphas, and the cumulative hazard -ln S0. Its variance is the Breslow form
//!   sum d / S^2 plus a' V a. The table's baseline cumulative hazard is at covariates 0; the
//!   other columns at the means.
//! - Case statistics at a case's own time and covariates: survival S0^exp(z'b), its standard
//!   error, the cumulative hazard, its log (LML), z'b with centered covariates (XBETA),
//!   Schoenfeld residuals for the events (PRESID) and the change in b from leaving the case
//!   out, approximated by its score residual times the covariance (DFBETA).
//! - TIME PROGRAM: every case is evaluated at each event time of its stratum up to its own time,
//!   with T_ set to that time; a missing time-dependent value there is an error.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, normal_quantile};
use crate::matrix::{Matrix, inverse};
use crate::model_terms::{
    Covariates, clone_key, distinct_terms, expand_terms, independent_in, listed_terms, method_name,
    prefix8, sorted, step_cell, wald,
};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Values};
use statscraft_syntax::VarItem;
use statscraft_syntax::{
    Bound, Command, Contrast, Coxreg, CoxregCriteria, CoxregTemp, LogisticMethodKind, MissingValue,
    Removal, Value,
};

/// The time program waiting for COXREG: its transformations, and whether it still collects.
#[derive(Clone, Default)]
pub(crate) struct TimeProgram {
    pub collecting: bool,
    pub commands: Vec<Command>,
    /// The vectors when COXREG took the program up, which its data pass then clears.
    pub vectors: std::collections::HashMap<String, Vec<String>>,
}

/// One row of the analysis: a case, or with time-dependent covariates a case at one event
/// time, at risk from after `start` up to `stop`.
struct Row {
    /// The index of the case among the analysis cases.
    case: usize,
    stratum: usize,
    start: f64,
    stop: f64,
    event: bool,
    w: f64,
    /// Every design column, centered.
    x: Vec<f64>,
}

/// An expanded row of a time program: the analysis case, and the start and stop of its time.
type Span = (usize, f64, f64);

/// A line of a plot: its name and the centered covariates of the model's columns.
type Line = (String, Vec<f64>);

/// The sums of one event time of a stratum over its risk set.
struct EventTime {
    stratum: usize,
    time: f64,
    /// The weighted number of events, the rows that end in them, and the weighted sum of
    /// their covariates.
    d: f64,
    deaths: Vec<usize>,
    xd: Vec<f64>,
    s0: f64,
    s1: Vec<f64>,
    s2: Matrix,
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn pick(x: &[f64], cols: &[usize]) -> Vec<f64> {
    cols.iter().map(|&c| x[c]).collect()
}

/// The event times of every stratum in ascending order with their risk-set sums at `b`;
/// `second`: with the second moments S2 (else left 0).
fn event_times(
    rows: &[Row],
    strata: usize,
    cols: &[usize],
    b: &[f64],
    second: bool,
) -> Vec<EventTime> {
    let p = cols.len();
    let mut out = Vec::new();
    for s in 0..strata {
        // the rows come sorted by stratum
        let mine: Vec<usize> = (rows.partition_point(|r| r.stratum < s)
            ..rows.partition_point(|r| r.stratum <= s))
            .collect();
        // within a stratum the rows come from the latest time down
        let by_stop = &mine;
        let mut times: Vec<f64> = mine
            .iter()
            .filter(|&&r| rows[r].event)
            .map(|&r| rows[r].stop)
            .collect();
        times.dedup();
        let (mut s0, mut s1, mut s2) = (0.0, vec![0.0; p], vec![vec![0.0; p]; p]);
        let add = |r: usize, s0: &mut f64, s1: &mut [f64], s2: &mut Matrix| {
            let x = pick(&rows[r].x, cols);
            let e = rows[r].w * dot(&x, b).exp();
            *s0 += e;
            for i in 0..p {
                s1[i] += e * x[i];
                if second {
                    for j in 0..p {
                        s2[i][j] += e * x[i] * x[j];
                    }
                }
            }
        };
        // with time-dependent rows each risk set is summed afresh, so no sum is a difference
        // of large numbers; otherwise the sums grow as the time goes down
        let fresh = mine.iter().any(|&r| rows[r].start > f64::NEG_INFINITY);
        let mut active: Vec<usize> = Vec::new();
        let mut a = 0;
        let mut here = Vec::new();
        for &t in &times {
            let mut deaths = Vec::new();
            while a < by_stop.len() && rows[by_stop[a]].stop >= t {
                let r = by_stop[a];
                if fresh {
                    active.push(r);
                } else {
                    add(r, &mut s0, &mut s1, &mut s2);
                }
                if rows[r].event && rows[r].stop == t {
                    deaths.push(r);
                }
                a += 1;
            }
            if fresh {
                active.retain(|&r| rows[r].start < t);
                (s0, s1, s2) = (0.0, vec![0.0; p], vec![vec![0.0; p]; p]);
                for &r in &active {
                    add(r, &mut s0, &mut s1, &mut s2);
                }
            }
            let mut xd = vec![0.0; p];
            let mut d = 0.0;
            for &r in &deaths {
                d += rows[r].w;
                for (i, &c) in cols.iter().enumerate() {
                    xd[i] += rows[r].w * rows[r].x[c];
                }
            }
            here.push(EventTime {
                stratum: s,
                time: t,
                d,
                deaths,
                xd,
                s0,
                s1: s1.clone(),
                s2: if second { s2.clone() } else { Vec::new() },
            });
        }
        here.reverse();
        out.extend(here);
    }
    out
}

/// The log partial likelihood, its first derivatives and the information matrix at `b`.
fn derivatives(rows: &[Row], strata: usize, cols: &[usize], b: &[f64]) -> (f64, Vec<f64>, Matrix) {
    let p = cols.len();
    let (mut ll, mut u, mut info) = (0.0, vec![0.0; p], vec![vec![0.0; p]; p]);
    for e in event_times(rows, strata, cols, b, true) {
        ll += e
            .deaths
            .iter()
            .map(|&r| rows[r].w * dot(&pick(&rows[r].x, cols), b))
            .sum::<f64>()
            - e.d * e.s0.ln();
        for (i, (u, row)) in u.iter_mut().zip(&mut info).enumerate() {
            *u += e.xd[i] - e.d * e.s1[i] / e.s0;
            for (j, cell) in row.iter_mut().enumerate() {
                *cell += e.d * (e.s2[i][j] / e.s0 - e.s1[i] * e.s1[j] / (e.s0 * e.s0));
            }
        }
    }
    (ll, u, info)
}

struct Fit {
    cols: Vec<usize>,
    b: Vec<f64>,
    cov: Matrix,
    ll: f64,
    /// Why the estimation stopped short, as a warning.
    problem: Option<String>,
}

impl Fit {
    fn at(&self, col: usize) -> Option<usize> {
        self.cols.iter().position(|&c| c == col)
    }
}

fn fit(rows: &[Row], strata: usize, cols: &[usize], criteria: &CoxregCriteria) -> Fit {
    let p = cols.len();
    let mut b = vec![0.0; p];
    let (mut ll, mut u, mut info) = derivatives(rows, strata, cols, &b);
    let mut converged = p == 0;
    let mut problem = None;
    for _ in 0..criteria.iterate {
        if p == 0 {
            break;
        }
        let Some((inv, _)) = inverse(&info) else {
            problem = Some(
                "the information matrix is singular, so a covariate cannot be estimated; the estimates are not reliable"
                    .to_string(),
            );
            break;
        };
        let delta: Vec<f64> = (0..p).map(|r| dot(&inv[r], &u)).collect();
        let mut step = 1.0;
        // halve a step that lowers the likelihood; when halving does not help, stop unconverged
        let accepted = loop {
            let tried: Vec<f64> = b.iter().zip(&delta).map(|(b, d)| b + step * d).collect();
            // only the likelihood decides; the derivatives wait for the accepted step
            let trial = log_likelihood(rows, strata, cols, &tried);
            // a loss within rounding of the log likelihood still counts as no loss
            if trial >= ll - 1e-12 * ll.abs().max(1.0) {
                let next = derivatives(rows, strata, cols, &tried);
                break Some((tried, next));
            }
            if step < 1e-3 {
                break None;
            }
            step /= 2.0;
        };
        let Some((nb, (nll, nu, ninfo))) = accepted else {
            problem = Some(
                "the estimation stopped where no step raised the likelihood; the estimates are those of the iteration before"
                    .to_string(),
            );
            break;
        };
        let relative = b
            .iter()
            .zip(&nb)
            .map(|(old, new)| {
                // relative to the previous value, absolute for a coefficient near 0
                (new - old).abs() / old.abs().max(1e-8)
            })
            .fold(0.0, f64::max);
        let ll_change = ((nll - ll) / ll.abs().max(f64::MIN_POSITIVE)).abs();
        b = nb;
        ll = nll;
        u = nu;
        info = ninfo;
        // a halved step is small because it was halved, which says nothing of convergence
        if step == 1.0
            && (relative < criteria.bcon || (criteria.lcon > 0.0 && ll_change < criteria.lcon))
        {
            converged = true;
            break;
        }
    }
    let cov = inverse(&info).map_or_else(|| vec![vec![f64::NAN; p]; p], |(inv, _)| inv);
    Fit {
        cols: cols.to_vec(),
        b,
        cov,
        ll,
        problem: problem.or_else(|| {
            (!converged).then(|| {
                format!(
                    "the estimation did not converge in {} iterations; the estimates are those of the last iteration",
                    criteria.iterate
                )
            })
        }),
    }
}

/// The score statistic of the columns `extra` at the estimates of `fit` (0 for `extra`).
fn score_test(rows: &[Row], strata: usize, fit: &Fit, extra: &[usize]) -> f64 {
    let m = fit.cols.len();
    let q = extra.len();
    if q == 0 {
        return 0.0;
    }
    let mut cols = fit.cols.clone();
    cols.extend(extra);
    let mut b = fit.b.clone();
    b.extend(std::iter::repeat_n(0.0, q));
    let (_, u, info) = derivatives(rows, strata, &cols, &b);
    let mut s = vec![vec![0.0; q]; q];
    for r in 0..q {
        for c in 0..q {
            s[r][c] = info[m + r][m + c];
        }
    }
    if m > 0 {
        let i11: Matrix = (0..m).map(|r| info[r][..m].to_vec()).collect();
        let Some((inv11, _)) = inverse(&i11) else {
            return 0.0;
        };
        for r in 0..q {
            for c in 0..q {
                let mut sum = 0.0;
                for i in 0..m {
                    for j in 0..m {
                        sum += info[m + r][i] * inv11[i][j] * info[j][m + c];
                    }
                }
                s[r][c] -= sum;
            }
        }
    }
    let Some((inv, det)) = inverse(&s) else {
        return 0.0;
    };
    if det <= 0.0 {
        return 0.0;
    }
    let ue = &u[m..];
    (0..q)
        .map(|r| ue[r] * dot(&inv[r], ue))
        .sum::<f64>()
        .max(0.0)
}

/// The score statistic at b = 0 of the columns of `fit`: the overall test.
fn overall_score(rows: &[Row], strata: usize, cols: &[usize]) -> f64 {
    let empty = Fit {
        cols: Vec::new(),
        b: Vec::new(),
        cov: Vec::new(),
        ll: 0.0,
        problem: None,
    };
    score_test(rows, strata, &empty, cols)
}

/// The log partial likelihood alone at `b`.
fn log_likelihood(rows: &[Row], strata: usize, cols: &[usize], b: &[f64]) -> f64 {
    event_times(rows, strata, cols, b, false)
        .iter()
        .map(|e| {
            e.deaths
                .iter()
                .map(|&r| rows[r].w * dot(&pick(&rows[r].x, cols), b))
                .sum::<f64>()
                - e.d * e.s0.ln()
        })
        .sum()
}

struct Model<'a> {
    rows: &'a [Row],
    strata: usize,
    criteria: &'a CoxregCriteria,
    terms: &'a [crate::model_terms::TermInfo],
    /// The weighted cross-products of the design columns centered within each stratum, for
    /// the redundancy check: a column constant within every stratum cannot be estimated.
    sscp: Matrix,
}

impl Model<'_> {
    fn fit(&self, cols: &[usize]) -> Fit {
        fit(self.rows, self.strata, cols, self.criteria)
    }

    fn score(&self, fit: &Fit, extra: &[usize]) -> f64 {
        score_test(self.rows, self.strata, fit, extra)
    }

    /// The removal statistic of term `t` in `fit` with its df.
    fn removal(&self, fit: &Fit, t: usize, kind: Removal) -> (f64, usize) {
        let columns = &self.terms[t].columns;
        let positions: Vec<usize> = columns.iter().filter_map(|&c| fit.at(c)).collect();
        let rest: Vec<usize> = fit
            .cols
            .iter()
            .copied()
            .filter(|c| !columns.contains(c))
            .collect();
        let df = positions.len();
        match kind {
            Removal::Wald => (wald(&fit.b, &fit.cov, &positions), df),
            Removal::Lr => (2.0 * (fit.ll - self.fit(&rest).ll), df),
            Removal::Cond => {
                let others: Vec<usize> = (0..fit.cols.len())
                    .filter(|p| !positions.contains(p))
                    .collect();
                let c22: Matrix = positions
                    .iter()
                    .map(|&r| positions.iter().map(|&s| fit.cov[r][s]).collect())
                    .collect();
                let Some((inv, _)) = inverse(&c22) else {
                    return (f64::NAN, df);
                };
                let beta: Vec<f64> = positions.iter().map(|&r| fit.b[r]).collect();
                let solved: Vec<f64> = (0..df).map(|r| dot(&inv[r], &beta)).collect();
                let b: Vec<f64> = others
                    .iter()
                    .map(|&o| {
                        fit.b[o]
                            - positions
                                .iter()
                                .zip(&solved)
                                .map(|(&p, v)| fit.cov[o][p] * v)
                                .sum::<f64>()
                    })
                    .collect();
                let ll = log_likelihood(self.rows, self.strata, &rest, &b);
                (2.0 * (fit.ll - ll), df)
            }
        }
    }

    /// The columns of `columns` not redundant next to `fit`'s.
    fn new_columns(&self, fit: &Fit, columns: &[usize]) -> Vec<usize> {
        let mut wanted = fit.cols.clone();
        wanted.extend(columns);
        self.independent(&wanted)
            .into_iter()
            .filter(|c| columns.contains(c))
            .collect()
    }

    fn independent(&self, wanted: &[usize]) -> Vec<usize> {
        independent_in(&self.sscp, wanted, 1e-8)
    }
}

fn stratum_sscp(rows: &[Row], strata: usize, p: usize) -> Matrix {
    let mut sums = vec![(0.0, vec![0.0; p]); strata];
    for r in rows {
        sums[r.stratum].0 += r.w;
        for (s, x) in sums[r.stratum].1.iter_mut().zip(&r.x) {
            *s += r.w * x;
        }
    }
    let mut m = vec![vec![0.0; p]; p];
    for r in rows {
        let (w, sum) = &sums[r.stratum];
        let d: Vec<f64> = r.x.iter().zip(sum).map(|(x, s)| x - s / w).collect();
        for i in 0..p {
            for j in 0..p {
                m[i][j] += r.w * d[i] * d[j];
            }
        }
    }
    m
}

/// The baseline of one stratum at its event times, at the covariate means.
struct Baseline {
    times: Vec<f64>,
    /// Survival at the means (the product of the alphas) after each time.
    survival: Vec<f64>,
    /// Cumulative sums of d / S0 (Breslow hazard), d / S0^2 and d S1 / S0^2.
    breslow: Vec<f64>,
    var: Vec<f64>,
    a: Vec<Vec<f64>>,
}

impl Baseline {
    /// The position of the last event time at or before `t`.
    fn index(&self, t: f64) -> Option<usize> {
        self.times.partition_point(|&s| s <= t).checked_sub(1)
    }
}

/// The alpha of one event time: sum over the deaths of w e / (1 - alpha^e) = s0.
fn alpha(deaths: &[(f64, f64)], s0: f64) -> f64 {
    let dying: f64 = deaths.iter().map(|(w, e)| w * e).sum();
    if dying >= s0 * (1.0 - 1e-12) {
        return 0.0;
    }
    if let [(w, e)] = deaths {
        return (1.0 - w * e / s0).powf(1.0 / e);
    }
    let f = |a: f64| {
        deaths
            .iter()
            .map(|(w, e)| w * e / (1.0 - a.powf(*e)))
            .sum::<f64>()
            - s0
    };
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

fn baselines(rows: &[Row], strata: usize, fit: &Fit) -> Vec<Baseline> {
    let p = fit.cols.len();
    let mut out: Vec<Baseline> = (0..strata)
        .map(|_| Baseline {
            times: Vec::new(),
            survival: Vec::new(),
            breslow: Vec::new(),
            var: Vec::new(),
            a: Vec::new(),
        })
        .collect();
    for e in event_times(rows, strata, &fit.cols, &fit.b, false) {
        let base = &mut out[e.stratum];
        let deaths: Vec<(f64, f64)> = e
            .deaths
            .iter()
            .map(|&r| (rows[r].w, dot(&pick(&rows[r].x, &fit.cols), &fit.b).exp()))
            .collect();
        let s = base.survival.last().copied().unwrap_or(1.0) * alpha(&deaths, e.s0);
        let last = |v: &[f64]| v.last().copied().unwrap_or(0.0);
        let breslow = last(&base.breslow) + e.d / e.s0;
        let var = last(&base.var) + e.d / (e.s0 * e.s0);
        let mut a = base.a.last().cloned().unwrap_or_else(|| vec![0.0; p]);
        for (a, s1) in a.iter_mut().zip(&e.s1) {
            *a += e.d * s1 / (e.s0 * e.s0);
        }
        base.times.push(e.time);
        base.survival.push(s);
        base.breslow.push(breslow);
        base.var.push(var);
        base.a.push(a);
    }
    out
}

/// Survival, its standard error and the cumulative hazard at time `t` for the centered
/// covariates `z` of the model's columns.
fn at_time(base: &Baseline, fit: &Fit, t: f64, z: &[f64]) -> (f64, f64, f64) {
    let Some(i) = base.index(t) else {
        return (1.0, 0.0, 0.0);
    };
    let risk = dot(z, &fit.b).exp();
    let s = base.survival[i].powf(risk);
    let h = -base.survival[i].ln() * risk;
    let q: Vec<f64> = (0..z.len())
        .map(|j| base.a[i][j] - z[j] * base.breslow[i])
        .collect();
    let quad: f64 = (0..q.len()).map(|r| q[r] * dot(&fit.cov[r], &q)).sum();
    let se = s * (risk * risk * (base.var[i] + quad)).sqrt();
    (s, se, h)
}

/// What a step changed and the model after it.
struct Step {
    model: Vec<usize>,
    fit: Fit,
    entered: Vec<usize>,
    removed: Vec<usize>,
}

/// The status value of a case is an event.
fn is_event(key: &Key, events: &[MissingValue]) -> bool {
    events.iter().any(|v| match (v, key) {
        (MissingValue::Value(Value::Number(n)), Key::Number(x)) => n == x,
        (MissingValue::Value(Value::String(s)), Key::Text(t)) => s.trim_end() == t.trim_end(),
        (MissingValue::Range(lo, hi), Key::Number(x)) => {
            let bound = |b: &Bound| match b {
                Bound::Number(n) => *n,
                Bound::Lowest => f64::NEG_INFINITY,
                Bound::Highest => f64::INFINITY,
            };
            let (low, high) = (bound(lo), bound(hi));
            // a range written high THRU low means the same values
            (low.min(high)..=low.max(high)).contains(x)
        }
        _ => false,
    })
}

impl Engine {
    pub(crate) fn coxreg(&mut self, command: &Coxreg) -> Result<Vec<Table>, Error> {
        let program = self
            .running_program
            .clone()
            .filter(|p| !p.commands.is_empty());
        self.coxreg_with(command, program)
    }

    /// COXREG with the time program, if any, it runs.
    fn coxreg_with(
        &mut self,
        command: &Coxreg,
        program: Option<TimeProgram>,
    ) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let criteria = command.criteria;
        let mut warnings = Vec::new();
        let time_var = data.variable(&command.time)?;
        let status_var = data.variable(&command.status)?;
        let strata_var = command
            .strata
            .as_ref()
            .map(|n| data.variable(n))
            .transpose()?;
        if time_var.is_string() {
            return Err(Error(format!(
                "the survival time variable {} must be numeric",
                time_var.name()
            )));
        }
        let string_events = command
            .events
            .iter()
            .any(|v| matches!(v, MissingValue::Value(Value::String(_))));
        if string_events != status_var.is_string() {
            return Err(Error(format!(
                "STATUS: {} is {}, so its event values must be {}",
                status_var.name(),
                if status_var.is_string() {
                    "a string"
                } else {
                    "numeric"
                },
                if status_var.is_string() {
                    "quoted strings"
                } else {
                    "numbers"
                }
            )));
        }
        // `a TO b` spans the variables of the data between them
        let with = expand_terms(data, &command.terms)?;
        let lists: Vec<Vec<Vec<String>>> = command
            .methods
            .iter()
            .map(|m| expand_terms(data, &m.terms))
            .collect::<Result<_, _>>()?;
        let mut categorical = Vec::new();
        for item in &command.categorical {
            match item {
                VarItem::Name(name) => categorical.push(name.clone()),
                other => categorical.extend(crate::expand_in(data, std::slice::from_ref(other))?),
            }
        }
        if command.repeated_status {
            warnings.push("STATUS is given more than once; the last one counts".to_string());
        }
        let term_vars = distinct_terms(with.iter().chain(lists.iter().flatten()));
        if program.is_some() && data.variable_index("T_").is_some() {
            return Err(Error(
                "the data has a variable T_, which a TIME PROGRAM reserves for the time; rename it"
                    .into(),
            ));
        }
        if program.is_some()
            && term_vars
                .iter()
                .flatten()
                .any(|n| n.eq_ignore_ascii_case("T_"))
        {
            return Err(Error(
                "T_ is the time of a TIME PROGRAM and cannot be a covariate".into(),
            ));
        }
        let mut roles = vec![
            (
                time_var.name(),
                "is the survival time and cannot be a covariate",
            ),
            (
                status_var.name(),
                "is the status variable and cannot be a covariate",
            ),
        ];
        if let Some(s) = &strata_var {
            roles.push((s.name(), "is the strata variable and cannot be a covariate"));
        }
        // the variables of the original data, to screen the cases; one the time program computes
        // is checked after the program runs, where a missing value is an error
        // shortcut: only top-level COMPUTE and IF count as computing; a covariate a RECODE, COUNT,
        // DO REPEAT or vector element of the program changes is screened on the data's values
        let mut computed = Vec::new();
        for c in program.iter().flat_map(|p| &p.commands) {
            match c {
                Command::Compute { target, .. } | Command::If { target, .. } => {
                    computed.push(target)
                }
                _ => {}
            }
        }
        let present: Vec<Vec<String>> = term_vars
            .iter()
            .map(|t| {
                t.iter()
                    .filter(|n| {
                        data.variable_index(n).is_some()
                            && !computed.iter().any(|c| c.eq_ignore_ascii_case(n))
                    })
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .filter(|t: &Vec<String>| !t.is_empty())
            .collect();
        let screen = Covariates::resolve(
            data,
            &present,
            &roles,
            &[],
            &[],
            command.include,
            &mut Vec::new(),
        )?;
        let cases = data.case_count();
        let weights = case_weights(data)?;
        let weight = |k: usize| weights.as_ref().map_or(1.0, |w| w[k]);
        let time = |k: usize| match time_var.values() {
            Values::Numeric(v) => v
                .get(k)
                .filter(|t| command.include || !time_var.is_user_missing(*t)),
            Values::String(_) => None,
        };
        let status = categories(&status_var, command.include);
        let strata_keys = strata_var.as_ref().map(|v| categories(v, command.include));
        // why a case is out: 1 missing, 2 negative time, 3 censored before the first event
        let mut dropped = vec![0u8; cases];
        for k in 0..cases {
            let valid = time(k).is_some()
                && status[k].is_some()
                && strata_keys.as_ref().is_none_or(|s| s[k].is_some())
                && screen.valid(k)
                && weight(k) > 0.0;
            if !valid {
                dropped[k] = 1;
            } else if time(k).expect("valid") < 0.0 {
                dropped[k] = 2;
            }
        }
        let event: Vec<bool> = (0..cases)
            .map(|k| {
                status[k]
                    .as_ref()
                    .is_some_and(|s| is_event(s, &command.events))
            })
            .collect();
        // the strata among the valid cases, in ascending order
        let mut levels: Vec<Key> = Vec::new();
        if let Some(keys) = &strata_keys {
            for k in (0..cases).filter(|&k| dropped[k] == 0) {
                let key = keys[k].as_ref().expect("valid");
                if !levels.iter().any(|l| l.compare(key).is_eq()) {
                    levels.push(clone_key(key));
                }
            }
            levels.sort_by(|a, b| a.compare(b));
        }
        let stratum_index: Vec<usize> = (0..cases)
            .map(|k| match (&strata_keys, dropped[k]) {
                (Some(keys), 0) => {
                    let key = keys[k].as_ref().expect("valid");
                    levels
                        .binary_search_by(|l| l.compare(key))
                        .expect("a stratum")
                }
                _ => 0,
            })
            .collect();
        let stratum_of = |k: usize| stratum_index[k];
        let strata = levels.len().max(1);
        let mut first_event = vec![f64::INFINITY; strata];
        for k in (0..cases).filter(|&k| dropped[k] == 0 && event[k]) {
            let s = stratum_of(k);
            first_event[s] = first_event[s].min(time(k).expect("valid"));
        }
        for k in 0..cases {
            if dropped[k] == 0 && !event[k] && time(k).expect("valid") < first_event[stratum_of(k)]
            {
                dropped[k] = 3;
            }
        }
        let used: Vec<usize> = (0..cases).filter(|&k| dropped[k] == 0).collect();
        if !used.iter().any(|&k| event[k]) {
            return Err(Error(
                "COXREG needs at least one event among the cases used".into(),
            ));
        }
        let mut tables = Vec::new();

        // Case Processing Summary, unweighted
        let count = |f: &dyn Fn(usize) -> bool| (0..cases).filter(|&k| f(k)).count() as f64;
        let total = cases as f64;
        let events = count(&|k| dropped[k] == 0 && event[k]);
        let censored = count(&|k| dropped[k] == 0 && !event[k]);
        let missing = count(&|k| dropped[k] == 1);
        let negative = count(&|k| dropped[k] == 2);
        let early = count(&|k| dropped[k] == 3);
        let percent = |n: f64| number((total > 0.0).then(|| 100.0 * n / total));
        let row = |a: &str, b: &str, n: f64| {
            vec![
                if a.is_empty() { Cell::Empty } else { text(a) },
                text(b),
                Cell::Number(n),
                percent(n),
            ]
        };
        tables.push(table(
            "Case Processing Summary",
            2,
            plain(&["", "", "N", "Percent"]),
            vec![
                row("Cases available in analysis", "Event", events),
                row("", "Censored", censored),
                row("", "Total", events + censored),
                row("Cases dropped", "Cases with missing values", missing),
                row("", "Cases with negative time", negative),
                row(
                    "",
                    "Censored cases before the earliest event in a stratum",
                    early,
                ),
                row("", "Total", missing + negative + early),
                row("Total", "", total),
            ],
            Some(format!("Dependent Variable: {}", shown.variable(&time_var))),
        ));

        // the rows: one per case, or one per case and event time of its stratum
        let mut event_times_of: Vec<Vec<f64>> = vec![Vec::new(); strata];
        for &k in used.iter().filter(|&&k| event[k]) {
            event_times_of[stratum_of(k)].push(time(k).expect("valid"));
        }
        for t in &mut event_times_of {
            t.sort_by(f64::total_cmp);
            t.dedup();
        }
        if program.iter().flat_map(|p| &p.commands).any(|c| {
            matches!(
                c,
                Command::Print(_) | Command::PrintSpace(_) | Command::Xsave(_)
            )
        }) {
            return Err(Error(
                "TIME PROGRAM: PRINT, PRINT SPACE and XSAVE would act on every case at every event time; they cannot be in it"
                    .into(),
            ));
        }
        let mut program_warnings = Vec::new();
        let expanded: Option<(Dataset, Vec<Span>)> = if let Some(program) = &program {
            // shortcut: a fixed bound on the expanded rows; stream the risk sets if larger data
            // with time-dependent covariates is needed
            const MAX_ROWS: usize = 5_000_000;
            let count: usize = used
                .iter()
                .map(|&k| {
                    let times = &event_times_of[stratum_of(k)];
                    times.partition_point(|&t| t <= time(k).expect("valid"))
                })
                .sum();
            if count > MAX_ROWS {
                return Err(Error(format!(
                    "TIME PROGRAM: the cases at risk at every event time make {count} rows, more than the {MAX_ROWS} supported"
                )));
            }
            // (analysis case, start, stop) of every expanded row
            let mut origin = Vec::new();
            let mut take = Vec::new();
            for (i, &k) in used.iter().enumerate() {
                let times = &event_times_of[stratum_of(k)];
                let mut start = f64::NEG_INFINITY;
                for &t in times.iter().take_while(|&&t| t <= time(k).expect("valid")) {
                    origin.push((i, start, t));
                    take.push(Some(k));
                    start = t;
                }
            }
            let mut data = data.clone();
            // only the variables the program reads and the covariates go along
            let refs = crate::rows::Refs::of(&program.commands);
            if !refs.all && refs.ranges.is_empty() {
                // a vector the program names brings its variables
                let members = program
                    .vectors
                    .iter()
                    .filter(|(v, _)| refs.names.iter().any(|n| n.eq_ignore_ascii_case(v)))
                    .flat_map(|(_, m)| m);
                let named: Vec<&String> = refs
                    .names
                    .iter()
                    .chain(term_vars.iter().flatten())
                    .chain(members)
                    .collect();
                let wanted: Vec<&str> = (0..data.variable_count())
                    .map(|i| data.variable_at(i).name())
                    .filter(|n| named.iter().any(|w| w.eq_ignore_ascii_case(n)))
                    .collect();
                if !wanted.is_empty() {
                    let wanted: Vec<String> = wanted.iter().map(|n| n.to_string()).collect();
                    let names: Vec<&str> = wanted.iter().map(String::as_str).collect();
                    data.keep_variables(&names)?;
                }
            }
            data.take_cases(&take)?;
            data.set_numeric_values("T_", origin.iter().map(|&(_, _, t)| Some(t)).collect())?;
            let rows_before = data.case_count();
            let mut sub = self.clone();
            sub.dataset = Some(data);
            sub.time_program = None;
            sub.running_program = None;
            sub.vectors = program.vectors.clone();
            sub.case_numbers = None;
            sub.temporary = None;
            for c in &program.commands {
                sub.execute(c)
                    .map_err(|e| Error(format!("TIME PROGRAM: {}", e.0)))?;
                program_warnings.append(&mut sub.warnings);
            }
            sub.flush()?;
            if sub.collecting.is_some() || !sub.blocks.is_empty() {
                return Err(Error(
                    "TIME PROGRAM: a LOOP or DO IF in it is not closed".into(),
                ));
            }
            program_warnings.append(&mut sub.warnings);
            // the random numbers the program drew count as drawn
            self.random.swap(&sub.random);
            let data = sub.dataset.take().expect("the expanded data");
            if data.case_count() != rows_before {
                return Err(Error(
                    "TIME PROGRAM: the transformations cannot add or drop cases".into(),
                ));
            }
            // a covariate depends on time when the program creates it or changes its values
            let changed = |name: &String| -> Result<bool, Error> {
                let Some(_) = self.active()?.variable_index(name) else {
                    return Ok(true);
                };
                let before = categories(&self.active()?.variable(name)?, true);
                let after = categories(&data.variable(name)?, true);
                Ok((0..after.len()).any(|r| take[r].is_some_and(|k| after[r] != before[k])))
            };
            let mut any = false;
            for name in term_vars.iter().flatten() {
                any |= changed(name)?;
            }
            if !any {
                // the cases are screened afresh on every covariate without the program
                let tables = self.coxreg_with(command, None)?;
                for warning in program_warnings {
                    self.warn(warning);
                }
                self.warn("the TIME PROGRAM changes none of the covariates, so it is not used");
                return Ok(tables);
            }
            warnings.append(&mut program_warnings);
            Some((data, origin))
        } else {
            None
        };
        let time_dependent = expanded.is_some();
        let source = expanded.as_ref().map_or(data, |(d, _)| d);
        let mut covariates = Covariates::resolve(
            source,
            &term_vars,
            &roles,
            &categorical,
            &command.contrasts,
            command.include,
            &mut warnings,
        )?;
        // (source row, analysis case, start, stop)
        let layout: Vec<(usize, usize, f64, f64)> = match &expanded {
            Some((_, origin)) => origin
                .iter()
                .enumerate()
                .map(|(r, &(i, start, stop))| (r, i, start, stop))
                .collect(),
            None => used
                .iter()
                .enumerate()
                .map(|(i, &k)| (k, i, f64::NEG_INFINITY, time(k).expect("valid")))
                .collect(),
        };
        if let Some(&(_, i, _, stop)) = layout.iter().find(|&&(r, ..)| !covariates.valid(r)) {
            return Err(Error(format!(
                "TIME PROGRAM: a time-dependent covariate is missing for case {} at time {stop}",
                self.case_numbers.as_ref().map_or(used[i], |n| n[used[i]]) + 1
            )));
        }
        let mut in_source = vec![false; source.case_count()];
        for &(r, ..) in &layout {
            in_source[r] = true;
        }
        covariates.eight_bytes = true;
        covariates.code(
            &term_vars,
            &in_source,
            &command.contrasts,
            &Contrast::Deviation(None),
            &shown,
        )?;
        let names = covariates.names.clone();
        let mut rows: Vec<Row> = layout
            .iter()
            .map(|&(r, i, start, stop)| {
                let k = used[i];
                Row {
                    case: i,
                    stratum: stratum_of(k),
                    start,
                    stop,
                    event: event[k] && stop == time(k).expect("valid"),
                    w: weight(k),
                    x: covariates.row(r),
                }
            })
            .collect();
        // by stratum, and within it from the latest time down, as the risk-set sums want them
        rows.sort_by(|a, b| a.stratum.cmp(&b.stratum).then(b.stop.total_cmp(&a.stop)));
        let total_w: f64 = rows.iter().map(|r| r.w).sum();
        let means: Vec<f64> = (0..names.len())
            .map(|j| rows.iter().map(|r| r.w * r.x[j]).sum::<f64>() / total_w)
            .collect();
        for r in &mut rows {
            for (x, m) in r.x.iter_mut().zip(&means) {
                *x -= m;
            }
        }
        tables.extend(covariates.codings_table(
            "Categorical Variable Codings",
            &shown,
            |v, level| {
                // each case once: its first row starts at minus infinity
                layout
                    .iter()
                    .filter(|&&(r, _, start, _)| {
                        start == f64::NEG_INFINITY && covariates.level(v, r) == Some(level)
                    })
                    .map(|&(_, i, ..)| weight(used[i]))
                    .sum()
            },
        ));
        let terms = &covariates.terms;
        let model = Model {
            rows: &rows,
            strata,
            criteria: &criteria,
            terms,
            sscp: stratum_sscp(&rows, strata, names.len()),
        };

        // Block 0
        let initial = model.fit(&[]);
        let initial_ll = initial.ll;
        tables.push(table(
            "Omnibus Tests of Model Coefficients",
            0,
            plain(&["-2 Log Likelihood"]),
            vec![vec![number(Some(-2.0 * initial_ll))]],
            Some("Block 0: Beginning Block.".into()),
        ));
        let mut last = Step {
            model: Vec::new(),
            fit: initial,
            entered: Vec::new(),
            removed: Vec::new(),
        };
        let first_stepwise = command
            .methods
            .first()
            .is_some_and(|m| m.kind != LogisticMethodKind::Enter);
        let report = Report {
            model: &model,
            shown: &shown,
            names: &names,
            command,
        };
        if first_stepwise {
            let eligible = listed_terms(&lists[0], &with, &term_vars);
            tables.extend(report.not_in_equation(
                &[(0, &last)],
                &eligible,
                "Block 0: Beginning Block.",
            ));
        }
        let mut seen: Vec<usize> = Vec::new();
        let mut model_terms: Vec<usize> = Vec::new();
        for (b, method) in command.methods.iter().enumerate() {
            let eligible = listed_terms(&lists[b], &with, &term_vars);
            let caption = format!("Block {}: Method = {}.", b + 1, method_name(method.kind));
            let fit_model = |current: &[usize],
                             warnings: &mut Vec<String>,
                             seen: &mut Vec<usize>| {
                // the terms in the order they entered, so a new term cannot push out the
                // columns of one already in the model
                let wanted: Vec<usize> = current
                    .iter()
                    .flat_map(|&t| terms[t].columns.iter().copied())
                    .collect();
                let kept = model.independent(&wanted);
                for c in wanted.iter().filter(|c| !kept.contains(c)) {
                    if !seen.contains(c) {
                        seen.push(*c);
                        warnings.push(format!(
                            "{} is a linear combination of the covariates before it in the model and is left out",
                            names[*c]
                        ));
                    }
                }
                let fitted = model.fit(&kept);
                if let Some(note) = &fitted.problem
                    && !warnings.contains(note)
                {
                    warnings.push(note.clone());
                }
                fitted
            };
            let mut steps: Vec<Step> = Vec::new();
            let mut tried: Vec<Vec<usize>> = vec![sorted(&model_terms)];
            let enter_all = |model_terms: &mut Vec<usize>,
                             steps: &mut Vec<Step>,
                             warnings: &mut Vec<String>,
                             seen: &mut Vec<usize>| {
                let added: Vec<usize> = eligible
                    .iter()
                    .copied()
                    .filter(|t| !model_terms.contains(t))
                    .collect();
                model_terms.extend(&added);
                let fitted = fit_model(model_terms, warnings, seen);
                // a term whose columns all repeat others is not in the model
                let kept = |t: &usize| terms[*t].columns.iter().any(|&c| fitted.at(c).is_some());
                model_terms.retain(kept);
                let added: Vec<usize> = added.into_iter().filter(kept).collect();
                steps.push(Step {
                    model: model_terms.clone(),
                    fit: fitted,
                    entered: added,
                    removed: Vec::new(),
                });
            };
            match method.kind {
                LogisticMethodKind::Enter => {
                    enter_all(&mut model_terms, &mut steps, &mut warnings, &mut seen);
                }
                LogisticMethodKind::Fstep(kind) | LogisticMethodKind::Bstep(kind) => {
                    let mut forward = matches!(method.kind, LogisticMethodKind::Fstep(_));
                    if !forward {
                        enter_all(&mut model_terms, &mut steps, &mut warnings, &mut seen);
                        // removing back to the block's starting model is no cycle
                        tried = vec![sorted(&model_terms)];
                    }
                    for _ in 0..200 {
                        if !forward {
                            let current = steps.last().map_or(&last.fit, |s| &s.fit);
                            let worst = eligible
                                .iter()
                                .copied()
                                .filter(|t| model_terms.contains(t))
                                .map(|t| {
                                    let (s, df) = model.removal(current, t, kind);
                                    (t, chi_square_p(s, df as f64))
                                })
                                .filter(|(_, p)| p.is_finite())
                                .max_by(|a, b| a.1.total_cmp(&b.1));
                            if let Some((t, p)) = worst
                                && p > criteria.pout
                            {
                                let next: Vec<usize> =
                                    model_terms.iter().copied().filter(|&m| m != t).collect();
                                if tried.contains(&sorted(&next)) {
                                    break;
                                }
                                tried.push(sorted(&next));
                                model_terms = next;
                                let fitted = fit_model(&model_terms, &mut warnings, &mut seen);
                                steps.push(Step {
                                    model: model_terms.clone(),
                                    fit: fitted,
                                    entered: Vec::new(),
                                    removed: vec![t],
                                });
                                continue;
                            }
                        }
                        let current = steps.last().map_or(&last.fit, |s| &s.fit);
                        let best = eligible
                            .iter()
                            .copied()
                            .filter(|t| !model_terms.contains(t))
                            .map(|t| {
                                let cols = model.new_columns(current, &terms[t].columns);
                                let s = model.score(current, &cols);
                                (t, chi_square_p(s, cols.len() as f64))
                            })
                            .filter(|(_, p)| p.is_finite())
                            .min_by(|a, b| a.1.total_cmp(&b.1));
                        let Some((t, p)) = best else { break };
                        if p >= criteria.pin {
                            break;
                        }
                        let mut next = model_terms.clone();
                        next.push(t);
                        if tried.contains(&sorted(&next)) {
                            break;
                        }
                        tried.push(sorted(&next));
                        model_terms = next;
                        let fitted = fit_model(&model_terms, &mut warnings, &mut seen);
                        steps.push(Step {
                            model: model_terms.clone(),
                            fit: fitted,
                            entered: vec![t],
                            removed: Vec::new(),
                        });
                        forward = false;
                    }
                }
            }
            if steps.is_empty() {
                warnings.push(format!(
                    "block {}: no covariate entered or left the model",
                    b + 1
                ));
                tables.extend(report.not_in_equation(&[(0, &last)], &eligible, &caption));
                continue;
            }
            let shown_steps: Vec<(usize, &Step)> = if command.print.summary {
                vec![(steps.len(), steps.last().expect("nonempty"))]
            } else {
                steps.iter().enumerate().map(|(i, s)| (i + 1, s)).collect()
            };
            let previous: Vec<&Fit> = std::iter::once(&last.fit)
                .chain(steps.iter().map(|s| &s.fit))
                .collect();
            // Omnibus Tests of Model Coefficients
            let mut columns = plain(&["", "-2 Log Likelihood"]);
            for group in [
                "Overall (score)",
                "Change From Previous Step",
                "Change From Previous Block",
            ] {
                for c in ["Chi-square", "df", "Sig."] {
                    columns.push((group.to_string(), c.to_string()));
                }
            }
            let mut omnibus = Vec::new();
            // SUMMARY still gives a line here for every step
            for (n, s) in steps.iter().enumerate().map(|(i, s)| (i + 1, s)) {
                let p = s.fit.cols.len() as f64;
                let score = overall_score(&rows, strata, &s.fit.cols);
                let mut row = vec![
                    text(format!("Step {n}")),
                    number(Some(-2.0 * s.fit.ll)),
                    number((p > 0.0).then_some(score)),
                    Cell::Number(p),
                    number((p > 0.0).then(|| chi_square_p(score, p))),
                ];
                for base in [previous[n - 1], &last.fit] {
                    let chi = 2.0 * (s.fit.ll - base.ll);
                    let df = (p - base.cols.len() as f64).abs();
                    // no chi-square without degrees of freedom
                    row.push(number((df > 0.0).then_some(chi)));
                    row.push(Cell::Number(df));
                    row.push(number((df > 0.0).then(|| chi_square_p(chi.abs(), df))));
                }
                omnibus.push(row);
            }
            tables.push(table(
                "Omnibus Tests of Model Coefficients",
                1,
                columns,
                omnibus,
                Some(caption.clone()),
            ));
            let history: Vec<(usize, &Step)> =
                steps.iter().enumerate().map(|(i, s)| (i + 1, s)).collect();
            tables.push(report.in_equation(&shown_steps, &history, &caption));
            if command.print.corr {
                tables.push(report.correlations(&shown_steps, &caption));
            }
            if let LogisticMethodKind::Fstep(kind) | LogisticMethodKind::Bstep(kind) = method.kind
                && kind != Removal::Wald
            {
                tables.extend(report.term_removed(&shown_steps, &eligible, kind, &caption));
            }
            tables.extend(report.not_in_equation(&shown_steps, &eligible, &caption));
            last = steps.pop().expect("nonempty");
            model_terms = last.model.clone();
        }
        let final_fit = &last.fit;
        let bases = baselines(&rows, strata, final_fit);
        let stratum_name = |s: usize| -> String {
            match &strata_var {
                Some(v) => format!("{} = {}", shown.variable(v), shown.value(v, &levels[s])),
                None => String::new(),
            }
        };
        let xbar_b: f64 = final_fit
            .cols
            .iter()
            .zip(&final_fit.b)
            .map(|(&c, b)| means[c] * b)
            .sum();
        let wants_plots =
            command.plot.survival || command.plot.hazard || command.plot.lml || command.plot.oms;
        let patterns = if time_dependent {
            Vec::new()
        } else {
            report.patterns(&covariates, &means)?
        };
        if time_dependent && (command.print.baseline || wants_plots || !command.patterns.is_empty())
        {
            warnings.push(
                "the baseline table, PATTERN and the plots are not available with time-dependent covariates"
                    .into(),
            );
        } else {
            if command.print.baseline || wants_plots {
                let mut columns = plain(&["", "Mean"]);
                columns.extend(
                    (1..=command.patterns.len()).map(|i| (String::new(), format!("Pattern {i}"))),
                );
                let rows_out = names
                    .iter()
                    .enumerate()
                    .map(|(j, name)| {
                        let mut row = vec![text(name.clone()), number(Some(means[j]))];
                        row.extend(patterns.iter().map(|p| number(p.shown[j])));
                        row
                    })
                    .collect();
                tables.push(table("Covariate Means", 1, columns, rows_out, None));
            }
            if command.print.baseline {
                for (s, base) in bases
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| !b.times.is_empty())
                {
                    let rows_out = base
                        .times
                        .iter()
                        .map(|&t| {
                            let zero = vec![0.0; final_fit.cols.len()];
                            let (surv, se, h) = at_time(base, final_fit, t, &zero);
                            vec![
                                number(Some(t)),
                                number(Some(h * (-xbar_b).exp())),
                                number(Some(surv)),
                                number(Some(se)),
                                number(Some(h)),
                            ]
                        })
                        .collect();
                    let mut columns = plain(&["Time", "Baseline Cum Hazard"]);
                    for c in ["Survival", "SE", "Cum Hazard"] {
                        columns.push(("At mean of covariates".into(), c.into()));
                    }
                    let caption = strata_var.is_some().then(|| stratum_name(s));
                    tables.push(table("Survival Table", 0, columns, rows_out, caption));
                }
            }
            if wants_plots {
                let charts = report.plots(
                    &bases,
                    final_fit,
                    &patterns,
                    &means,
                    &stratum_name,
                    &shown.variable(&time_var),
                );
                self.charts.extend(charts);
            }
        }
        for warning in warnings {
            self.warn(warning);
        }
        if !command.save.is_empty() {
            let stats = CaseStats::new(&rows, strata, final_fit, &bases, used.len(), names.len());
            self.save_coxreg(
                command,
                &stats,
                &used,
                cases,
                final_fit,
                &names,
                time_dependent,
            )?;
        }
        Ok(tables)
    }

    #[allow(clippy::too_many_arguments)]
    fn save_coxreg(
        &mut self,
        command: &Coxreg,
        stats: &CaseStats,
        used: &[usize],
        cases: usize,
        fit: &Fit,
        names: &[String],
        time_dependent: bool,
    ) -> Result<(), Error> {
        let save: Vec<&(CoxregTemp, Option<String>)> = command
            .save
            .iter()
            .filter(|(t, _)| !time_dependent || *t == CoxregTemp::Dfbeta)
            .collect();
        if save.len() < command.save.len() {
            self.warn(
                "with time-dependent covariates SAVE gives DFBETA only; the other variables are not saved"
                    .to_string(),
            );
        }
        let mut row_of = vec![None; cases];
        for (i, &k) in used.iter().enumerate() {
            row_of[k] = Some(i);
        }
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            for (temp, given) in &save {
                // (suffix, label, values per analysis case)
                let parts: Vec<(String, String, Vec<f64>)> = match temp {
                    CoxregTemp::Survival => vec![(
                        String::new(),
                        "Survival function".into(),
                        stats.survival.clone(),
                    )],
                    CoxregTemp::Se => vec![(
                        String::new(),
                        "Standard error of the survival function".into(),
                        stats.se.clone(),
                    )],
                    CoxregTemp::Hazard => vec![(
                        String::new(),
                        "Cumulative hazard function".into(),
                        stats.hazard.clone(),
                    )],
                    CoxregTemp::Lml => vec![(
                        String::new(),
                        "Log minus log of the survival function".into(),
                        stats.hazard.iter().map(|h| h.ln()).collect(),
                    )],
                    CoxregTemp::Xbeta => vec![(
                        String::new(),
                        "Linear predictor with centered covariates".into(),
                        stats.xbeta.clone(),
                    )],
                    // one per coded covariate, so split groups with other models share names
                    CoxregTemp::Dfbeta => names
                        .iter()
                        .enumerate()
                        .map(|(c, name)| {
                            let j = fit.at(c);
                            (
                                (c + 1).to_string(),
                                format!("DFBETA for {name}"),
                                stats
                                    .dfbeta
                                    .iter()
                                    .map(|d| j.map_or(f64::NAN, |j| d[j]))
                                    .collect(),
                            )
                        })
                        .collect(),
                    CoxregTemp::Presid => names
                        .iter()
                        .enumerate()
                        .map(|(c, name)| {
                            (
                                (c + 1).to_string(),
                                format!("Partial residual for {name}"),
                                stats.presid.iter().map(|d| d[c]).collect(),
                            )
                        })
                        .collect(),
                };
                let root = match temp {
                    CoxregTemp::Survival => "SUR",
                    CoxregTemp::Se => "SE",
                    CoxregTemp::Hazard => "HAZ",
                    CoxregTemp::Lml => "LML",
                    CoxregTemp::Xbeta => "XBE",
                    CoxregTemp::Dfbeta => "DFB",
                    CoxregTemp::Presid => "PRE",
                };
                let new_names: Vec<String> = match given {
                    Some(name) if parts.len() == 1 && parts[0].0.is_empty() => vec![name.clone()],
                    Some(name) => parts.iter().map(|(s, ..)| format!("{name}{s}")).collect(),
                    None => {
                        let make = |n: usize| -> Vec<String> {
                            parts
                                .iter()
                                .map(|(s, ..)| format!("{root}{s}_{n}"))
                                .collect()
                        };
                        let mut n = 1;
                        while make(n)
                            .iter()
                            .any(|name| data.variable_index(name).is_some())
                        {
                            n += 1;
                        }
                        make(n)
                    }
                };
                for (name, (_, label, values)) in new_names.iter().zip(&parts) {
                    if data.variable_index(name).is_some() {
                        return Err(Error(format!(
                            "COXREG /SAVE: variable {name} already exists"
                        )));
                    }
                    let column: Vec<Option<f64>> = (0..cases)
                        .map(|k| row_of[k].map(|i| values[i]).filter(|v| v.is_finite()))
                        .collect();
                    data.set_numeric_values(name, column)?;
                    data.set_variable_label(name, Some(label))?;
                    data.set_format(name, "F11.5")?;
                    data.set_measure(name, Measure::Scale)?;
                }
            }
            Ok(())
        })
    }
}

/// The statistics SAVE gives, per analysis case; NaN where there is none.
struct CaseStats {
    survival: Vec<f64>,
    se: Vec<f64>,
    hazard: Vec<f64>,
    xbeta: Vec<f64>,
    /// Per case, one per column of the final model.
    dfbeta: Vec<Vec<f64>>,
    /// Per case, one per design column; only events and columns in the model.
    presid: Vec<Vec<f64>>,
}

impl CaseStats {
    fn new(
        rows: &[Row],
        strata: usize,
        fit: &Fit,
        bases: &[Baseline],
        cases: usize,
        columns: usize,
    ) -> CaseStats {
        let p = fit.cols.len();
        let nan = f64::NAN;
        let mut out = CaseStats {
            survival: vec![nan; cases],
            se: vec![nan; cases],
            hazard: vec![nan; cases],
            xbeta: vec![nan; cases],
            dfbeta: vec![vec![0.0; p]; cases],
            presid: vec![vec![nan; columns]; cases],
        };
        for e in event_times(rows, strata, &fit.cols, &fit.b, false) {
            for &r in &e.deaths {
                for (j, &c) in fit.cols.iter().enumerate() {
                    out.presid[rows[r].case][c] = rows[r].x[c] - e.s1[j] / e.s0;
                }
            }
        }
        // the running sums of d / S0 and d S1 / S0^2 up to time t
        let upto = |s: usize, t: f64| -> (f64, Vec<f64>) {
            let base = &bases[s];
            match base.index(t) {
                Some(i) => (base.breslow[i], base.a[i].clone()),
                None => (0.0, vec![0.0; p]),
            }
        };
        let mut score = vec![vec![0.0; p]; cases];
        for row in rows {
            let z = pick(&row.x, &fit.cols);
            let e = dot(&z, &fit.b).exp();
            let (h1, a1) = upto(row.stratum, row.stop);
            let (h0, a0) = upto(row.stratum, row.start);
            for j in 0..p {
                let mut l = -e * (z[j] * (h1 - h0) - (a1[j] - a0[j]));
                if row.event {
                    l += out.presid[row.case][fit.cols[j]];
                }
                score[row.case][j] += row.w * l;
            }
            if row.start == f64::NEG_INFINITY {
                let (s, se, h) = at_time(&bases[row.stratum], fit, row.stop, &z);
                out.survival[row.case] = s;
                out.se[row.case] = se;
                out.hazard[row.case] = h;
                out.xbeta[row.case] = dot(&z, &fit.b);
            }
        }
        for (i, u) in score.iter().enumerate() {
            out.dfbeta[i] = (0..p).map(|j| dot(&fit.cov[j], u)).collect();
        }
        out
    }
}

/// A covariate pattern for the plots: its lines, each a name and a design row (not centered).
struct Pattern {
    title: String,
    lines: Vec<(String, Vec<f64>)>,
    /// The column values for the Covariate Means table; empty for the columns of a BY
    /// variable, which change from line to line.
    shown: Vec<Option<f64>>,
}

/// What the tables of every block share.
struct Report<'a> {
    model: &'a Model<'a>,
    shown: &'a crate::shown::Shown,
    names: &'a [String],
    command: &'a Coxreg,
}

impl Report<'_> {
    fn in_equation(
        &self,
        steps: &[(usize, &Step)],
        history: &[(usize, &Step)],
        caption: &str,
    ) -> Table {
        let ci = self.command.print.ci;
        let mut columns = plain(&["", "", "B", "SE", "Wald", "df", "Sig.", "Exp(B)"]);
        if let Some(level) = ci {
            let group = format!("{level}% CI for Exp(B)");
            columns.push((group.clone(), "Lower".into()));
            columns.push((group, "Upper".into()));
        }
        let z = ci.map(|level| normal_quantile(0.5 + level / 200.0));
        let mut rows = Vec::new();
        let mut notes = Vec::new();
        for &(n, s) in steps {
            let fit = &s.fit;
            let mut first = true;
            // the terms in the order of the command
            for &t in &sorted(&s.model) {
                let term = &self.model.terms[t];
                let positions: Vec<(usize, usize)> = term
                    .columns
                    .iter()
                    .filter_map(|&c| fit.at(c).map(|p| (c, p)))
                    .collect();
                if term.categorical && positions.len() > 1 {
                    let at: Vec<usize> = positions.iter().map(|p| p.1).collect();
                    let w = wald(&fit.b, &fit.cov, &at);
                    let df = at.len() as f64;
                    let mut row = vec![
                        step_cell(first, n),
                        text(term.label.clone()),
                        Cell::Empty,
                        Cell::Empty,
                        number(Some(w)),
                        Cell::Number(df),
                        number(Some(chi_square_p(w, df))),
                        Cell::Empty,
                    ];
                    if z.is_some() {
                        row.extend([Cell::Empty, Cell::Empty]);
                    }
                    rows.push(row);
                    first = false;
                }
                for (c, p) in positions {
                    let b = fit.b[p];
                    let se = fit.cov[p][p].sqrt();
                    let w = (b / se).powi(2);
                    let mut row = vec![
                        step_cell(first, n),
                        text(self.names[c].clone()),
                        number(Some(b)),
                        number(Some(se)),
                        number(Some(w)),
                        Cell::Number(1.0),
                        number(Some(chi_square_p(w, 1.0))),
                        number(Some(b.exp())),
                    ];
                    if let Some(z) = z {
                        row.push(number(Some((b - z * se).exp())));
                        row.push(number(Some((b + z * se).exp())));
                    }
                    rows.push(row);
                    first = false;
                }
            }
        }
        // every step of the block says what it changed, also under SUMMARY
        for (n, s) in history {
            let list = |ts: &[usize]| {
                ts.iter()
                    .map(|&t| self.model.terms[t].label.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            if !s.entered.is_empty() {
                notes.push(format!(
                    "Variable(s) entered on step {n}: {}.",
                    list(&s.entered)
                ));
            }
            if !s.removed.is_empty() {
                notes.push(format!(
                    "Variable(s) removed on step {n}: {}.",
                    list(&s.removed)
                ));
            }
        }
        let mut note = caption.to_string();
        for e in notes {
            note.push(' ');
            note.push_str(&e);
        }
        table("Variables in the Equation", 2, columns, rows, Some(note))
    }

    fn correlations(&self, steps: &[(usize, &Step)], caption: &str) -> Table {
        // the columns of every step shown, each estimate under its own name
        let mut all: Vec<usize> = steps.iter().flat_map(|(_, s)| s.fit.cols.clone()).collect();
        all.sort_unstable();
        all.dedup();
        let mut columns = plain(&["", ""]);
        columns.extend(all.iter().map(|&c| (String::new(), self.names[c].clone())));
        let mut rows = Vec::new();
        for &(n, s) in steps {
            let cov = &s.fit.cov;
            for (r, &c) in s.fit.cols.iter().enumerate() {
                let mut row = vec![step_cell(r == 0, n), text(self.names[c].clone())];
                row.extend(all.iter().map(|&col| {
                    number(
                        s.fit
                            .at(col)
                            .map(|k| cov[r][k] / (cov[r][r] * cov[k][k]).sqrt()),
                    )
                }));
                rows.push(row);
            }
        }
        table(
            "Correlation Matrix of Regression Coefficients",
            2,
            columns,
            rows,
            Some(caption.to_string()),
        )
    }

    fn term_removed(
        &self,
        steps: &[(usize, &Step)],
        eligible: &[usize],
        kind: Removal,
        caption: &str,
    ) -> Option<Table> {
        let mut rows = Vec::new();
        for &(n, s) in steps {
            let mut first = true;
            for &t in eligible.iter().filter(|t| s.model.contains(t)) {
                let (chi, df) = self.model.removal(&s.fit, t, kind);
                rows.push(vec![
                    step_cell(first, n),
                    text(self.model.terms[t].label.clone()),
                    number(Some(chi)),
                    Cell::Number(df as f64),
                    number(Some(chi_square_p(chi, df as f64))),
                ]);
                first = false;
            }
        }
        (!rows.is_empty()).then(|| {
            table(
                "Model if Term Removed",
                2,
                plain(&["", "Term Removed", "Loss Chi-square", "df", "Sig."]),
                rows,
                Some(caption.to_string()),
            )
        })
    }

    fn not_in_equation(
        &self,
        steps: &[(usize, &Step)],
        eligible: &[usize],
        caption: &str,
    ) -> Option<Table> {
        let m = self.model;
        let mut rows = Vec::new();
        for &(n, s) in steps {
            let out: Vec<usize> = eligible
                .iter()
                .copied()
                .filter(|t| !s.model.contains(t))
                .collect();
            if out.is_empty() {
                continue;
            }
            let mut first = true;
            let mut all = Vec::new();
            for &t in &out {
                let cols = m.new_columns(&s.fit, &m.terms[t].columns);
                all.extend(cols.iter().copied());
                let mut push = |name: String, cols: &[usize], rows: &mut Vec<Vec<Cell>>| {
                    let score = m.score(&s.fit, cols);
                    let df = cols.len() as f64;
                    rows.push(vec![
                        step_cell(first, n),
                        text(name),
                        number(Some(score)),
                        Cell::Number(df),
                        number(Some(chi_square_p(score, df))),
                    ]);
                    first = false;
                };
                if m.terms[t].categorical && cols.len() > 1 {
                    push(m.terms[t].label.clone(), &cols, &mut rows);
                }
                for &c in &cols {
                    push(self.names[c].clone(), &[c], &mut rows);
                }
            }
            let overall = m.new_columns(&s.fit, &all);
            let score = m.score(&s.fit, &overall);
            let df = overall.len() as f64;
            rows.push(vec![
                Cell::Empty,
                text("Residual Chi Square"),
                number(Some(score)),
                Cell::Number(df),
                number(Some(chi_square_p(score, df))),
            ]);
        }
        (!rows.is_empty()).then(|| {
            table(
                "Variables not in the Equation",
                2,
                plain(&["", "", "Score", "df", "Sig."]),
                rows,
                Some(caption.to_string()),
            )
        })
    }

    /// The design rows of every PATTERN: the named values, the column means elsewhere; with BY,
    /// one line per category of that variable.
    fn patterns(&self, covariates: &Covariates, means: &[f64]) -> Result<Vec<Pattern>, Error> {
        let mut out = Vec::new();
        for (n, pattern) in self.command.patterns.iter().enumerate() {
            let mut fixed: Vec<(usize, Vec<f64>)> = Vec::new();
            for (name, value) in &pattern.values {
                let Some(v) = covariates
                    .variables
                    .iter()
                    .position(|v| v.name().eq_ignore_ascii_case(name))
                else {
                    return Err(Error(format!(
                        "PATTERN names {name}, which is not a covariate of the model"
                    )));
                };
                fixed.push((v, self.values_of(covariates, v, value)?));
            }
            let by = match &pattern.by {
                Some(name) => {
                    let v = covariates
                        .variables
                        .iter()
                        .position(|v| v.name().eq_ignore_ascii_case(name))
                        .filter(|&v| covariates.codings[v].is_some())
                        .ok_or_else(|| {
                            Error(format!(
                                "PATTERN BY {name}: it must be a categorical covariate of the model"
                            ))
                        })?;
                    if fixed.iter().any(|(f, _)| *f == v) {
                        return Err(Error(format!(
                            "PATTERN: {name} cannot have a value and follow BY"
                        )));
                    }
                    Some(v)
                }
                None => None,
            };
            let row_for = |extra: Option<(usize, Vec<f64>)>| -> Vec<f64> {
                let mut all = fixed.clone();
                all.extend(extra);
                (0..means.len())
                    .map(|j| {
                        let parts = covariates.column_parts(j);
                        let values: Option<Vec<f64>> = parts
                            .iter()
                            .map(|&(v, c)| {
                                all.iter()
                                    .find(|(f, _)| *f == v)
                                    .map(|(_, vals)| vals[c.unwrap_or(0)])
                            })
                            .collect();
                        // shortcut: a column with an unnamed variable takes its mean, also in
                        // an interaction; refine if patterns of interactions are asked for
                        values.map_or(means[j], |v| v.iter().product())
                    })
                    .collect()
            };
            let lines = match by {
                Some(v) => {
                    let (levels, m) = covariates.codings[v].as_ref().expect("categorical");
                    let var = &covariates.variables[v];
                    levels
                        .iter()
                        .enumerate()
                        .map(|(at, level)| {
                            (
                                format!(
                                    "{} = {}",
                                    self.shown.variable(var),
                                    self.shown.value(var, level)
                                ),
                                row_for(Some((v, m[at].clone()))),
                            )
                        })
                        .collect()
                }
                None => vec![(format!("Pattern {}", n + 1), row_for(None))],
            };
            let shown = row_for(None)
                .into_iter()
                .enumerate()
                .map(|(j, v)| {
                    let varies =
                        by.is_some_and(|b| covariates.column_parts(j).iter().any(|&(v, _)| v == b));
                    (!varies).then_some(v)
                })
                .collect();
            out.push(Pattern {
                title: format!("pattern {}", n + 1),
                lines,
                shown,
            });
        }
        Ok(out)
    }

    /// The column values a variable takes at `value`: its coding row, or the value.
    fn values_of(
        &self,
        covariates: &Covariates,
        v: usize,
        value: &Value,
    ) -> Result<Vec<f64>, Error> {
        match &covariates.codings[v] {
            Some((levels, m)) => levels
                .iter()
                .position(|l| match (l, value) {
                    // strings by their first eight bytes, as the categories are told apart
                    (Key::Text(t), Value::String(x)) => prefix8(t) == prefix8(x),
                    _ => crate::genlin::value_matches(&covariates.variables[v], l, value),
                })
                .map(|at| m[at].clone())
                .ok_or_else(|| {
                    Error(format!(
                        "PATTERN: {} is not a category of {} among the cases used",
                        match value {
                            Value::Number(n) => n.to_string(),
                            Value::String(s) => format!("'{s}'"),
                        },
                        covariates.variables[v].name()
                    ))
                }),
            None => match value {
                Value::Number(n) => Ok(vec![*n]),
                Value::String(_) => Err(Error(format!(
                    "PATTERN: {} is numeric and takes a number",
                    covariates.variables[v].name()
                ))),
            },
        }
    }

    fn plots(
        &self,
        bases: &[Baseline],
        fit: &Fit,
        patterns: &[Pattern],
        means: &[f64],
        stratum_name: &dyn Fn(usize) -> String,
        time_title: &str,
    ) -> Vec<crate::Chart> {
        use crate::chart::{Axis, ChartKind};
        use crate::plot::{Plot, Point, PointGroup};
        let plot = &self.command.plot;
        let kinds: Vec<(&str, &str, usize)> = [
            (plot.survival, "Survival Function", "Cum Survival", 0),
            (plot.hazard, "Hazard Function", "Cum Hazard", 1),
            (plot.lml, "LML Function", "Log minus log", 2),
            (
                plot.oms,
                "One Minus Survival Function",
                "1 - Cum Survival",
                3,
            ),
        ]
        .into_iter()
        .filter(|k| k.0)
        .map(|(_, a, b, c)| (a, b, c))
        .collect();
        let zero = vec![0.0; fit.cols.len()];
        // (title part, lines: name and centered z of the model's columns)
        let mut sets: Vec<(String, Vec<Line>)> =
            vec![("at mean of covariates".into(), vec![(String::new(), zero)])];
        for p in patterns {
            let lines = p
                .lines
                .iter()
                .map(|(name, row)| {
                    let z: Vec<f64> = fit.cols.iter().map(|&c| row[c] - means[c]).collect();
                    (name.clone(), z)
                })
                .collect();
            sets.push((format!("for {}", p.title), lines));
        }
        let mut charts = Vec::new();
        for (title, ylabel, kind) in &kinds {
            for (part, lines) in &sets {
                let mut groups = Vec::new();
                for (s, base) in bases
                    .iter()
                    .enumerate()
                    .filter(|(_, b)| !b.times.is_empty())
                {
                    for (name, z) in lines {
                        let value = |surv: f64| match kind {
                            0 => surv,
                            1 => -surv.ln(),
                            2 => (-surv.ln()).ln(),
                            _ => 1.0 - surv,
                        };
                        let mut points = Vec::new();
                        let mut previous = 1.0;
                        if *kind != 2 {
                            points.push(Point {
                                x: 0.0,
                                y: value(1.0),
                                z: None,
                                label: None,
                            });
                        }
                        for &t in &base.times {
                            let (surv, _, _) = at_time(base, fit, t, z);
                            if *kind != 2 || previous < 1.0 {
                                points.push(Point {
                                    x: t,
                                    y: value(previous),
                                    z: None,
                                    label: None,
                                });
                            }
                            points.push(Point {
                                x: t,
                                y: value(surv),
                                z: None,
                                label: None,
                            });
                            previous = surv;
                        }
                        points.retain(|p| p.y.is_finite());
                        let group = [stratum_name(s), name.clone()]
                            .into_iter()
                            .filter(|s| !s.is_empty())
                            .collect::<Vec<_>>()
                            .join(", ");
                        groups.push(PointGroup {
                            name: group,
                            points,
                        });
                    }
                }
                charts.push(crate::Chart::plot(
                    ChartKind::Line,
                    format!("{title} {part}"),
                    Axis::titled(time_title),
                    Axis::titled(*ylabel),
                    Plot::Points {
                        groups,
                        lines: Vec::new(),
                        z_axis: None,
                    },
                ));
            }
        }
        charts
    }
}
