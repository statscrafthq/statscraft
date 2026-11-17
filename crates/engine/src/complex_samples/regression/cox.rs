//! CSCOXREG: Cox proportional hazards regression on a complex sample.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - Records: the design cases with a valid end time of at least 0 (and a valid start time of at
//!   least 0 below it), a valid status, baseline stratum, factors and covariates. Records left
//!   out, by the domain or by a missing value, stay in the design with zero scores.
//! - A record is at risk at time t when start < t <= end (no start: when t <= end). It has the
//!   event at its end time when its status is among the event values.
//! - The hazard is h0_g(t) exp(x'b) with one baseline per baseline stratum g. The design matrix
//!   is that of the other complex samples models with its intercept only to find the redundant
//!   columns: the last level of each factor is the reference and the intercept has no
//!   parameter. The columns are centered at the reference pattern (factors at their last level,
//!   covariates at their weighted means), which changes no estimate.
//! - The weighted log partial likelihood sums, over the event times t of each stratum, the
//!   weighted linear predictors of the d records with the event less (W / d) sum over r < d of
//!   ln(S0 - f_r D0), W their summed weights, S0 the weighted risk-set sum of exp(x'b), D0 that
//!   of the events, f_r = r / d (Efron) or 0 (Breslow; then W ln S0). The first and second
//!   derivatives follow; Newton-Raphson with step-halving starts at b = 0 and stops as
//!   CSLOGISTIC does.
//! - The score residual of a record is its own term at its event time less, for each event time
//!   in its at-risk interval, its share of the expected term: exp(x'b) (W / d) sum over r of
//!   (x - xbar_r)(1 - f_r [its own event time]) / (S0 - f_r D0). The covariance of b is
//!   J^-1 V(sum w U) J^-1 with the design covariance of the score total; the SRS one uses
//!   fpc N / (n - 1) sum w U U' as the other models do.
//! - Intervals, t tests, Wald tests, CUSTOM, PADJUST and design effects as in CSGLM; exp(B) and
//!   its interval exponentiate the estimate and its limits.

use super::super::{Design, keys_of, numbers};
use super::{
    Estimates, Wald, information_inverse, padjust, padjust_note, parameter_names, test_cells,
    test_columns, test_values, wald,
};
use crate::crosstabs::Key;
use crate::descriptives::number;
use crate::dist::{t_p, t_quantile};
use crate::genlin::value_matches;
use crate::glm::{Data, Fit, Part, effects_of, intercept_term, key_value_text, l_row};
use crate::matrix::{Matrix, multiply};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::shown::{Show, Shown};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Variable;
use statscraft_syntax::{CsCox, LRow, MissingValue, Term, Value};

const NAME: &str = "CSCOXREG";

/// One analysis record with its centered model columns.
struct Record {
    stratum: usize,
    start: f64,
    stop: f64,
    event: bool,
    w: f64,
    z: Vec<f64>,
}

/// What an event time gives the score residuals: the times are ascending within the stratum.
struct EventTime {
    time: f64,
    /// The mean column vector at t, weighted as the likelihood weights the tied events.
    xbar: Vec<f64>,
    /// sum coef / E0 and sum coef E1 / E0^2, and the same with the factor f_r (Efron's share
    /// of a record's own event time).
    a: f64,
    b: Vec<f64>,
    fa: f64,
    fb: Vec<f64>,
}

/// The log likelihood with its derivatives, and per stratum the event times.
struct Pass {
    ll: f64,
    u: Vec<f64>,
    info: Matrix,
    times: Vec<Vec<EventTime>>,
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// One pass over the event times at `b`; `second` adds the information matrix.
fn pass(recs: &[Record], strata: usize, b: &[f64], breslow: bool, second: bool) -> Pass {
    let q = b.len();
    let e: Vec<f64> = recs.iter().map(|r| dot(&r.z, b).exp()).collect();
    let mut out = Pass {
        ll: 0.0,
        u: vec![0.0; q],
        info: vec![vec![0.0; q]; q],
        times: Vec::new(),
    };
    for g in 0..strata {
        let mine: Vec<usize> = (0..recs.len()).filter(|&i| recs[i].stratum == g).collect();
        let mut by_stop = mine.clone();
        by_stop.sort_by(|&i, &j| recs[j].stop.total_cmp(&recs[i].stop));
        let mut by_start = mine.clone();
        by_start.sort_by(|&i, &j| recs[j].start.total_cmp(&recs[i].start));
        let mut times: Vec<f64> = by_stop
            .iter()
            .filter(|&&i| recs[i].event)
            .map(|&i| recs[i].stop)
            .collect();
        times.dedup();
        let (mut s0, mut s1, mut s2) = (0.0, vec![0.0; q], vec![vec![0.0; q]; q]);
        let add = |i: usize, sign: f64, s0: &mut f64, s1: &mut [f64], s2: &mut Matrix| {
            let we = sign * recs[i].w * e[i];
            *s0 += we;
            for a in 0..q {
                s1[a] += we * recs[i].z[a];
                if second {
                    for (cell, zc) in s2[a].iter_mut().zip(&recs[i].z) {
                        *cell += we * recs[i].z[a] * zc;
                    }
                }
            }
        };
        let (mut next_stop, mut next_start) = (0, 0);
        let mut here = Vec::new();
        for &t in &times {
            let mut deaths = Vec::new();
            while next_stop < by_stop.len() && recs[by_stop[next_stop]].stop >= t {
                let i = by_stop[next_stop];
                add(i, 1.0, &mut s0, &mut s1, &mut s2);
                if recs[i].event && recs[i].stop == t {
                    deaths.push(i);
                }
                next_stop += 1;
            }
            while next_start < by_start.len() && recs[by_start[next_start]].start >= t {
                add(by_start[next_start], -1.0, &mut s0, &mut s1, &mut s2);
                next_start += 1;
            }
            let (mut d0, mut d1, mut d2) = (0.0, vec![0.0; q], vec![vec![0.0; q]; q]);
            let mut wd = 0.0;
            for &i in &deaths {
                add(i, 1.0, &mut d0, &mut d1, &mut d2);
                wd += recs[i].w;
                out.ll += recs[i].w * dot(&recs[i].z, b);
                for a in 0..q {
                    out.u[a] += recs[i].w * recs[i].z[a];
                }
            }
            let d = deaths.len();
            // (coefficient, f): Efron splits the tied events into d steps
            let parts: Vec<(f64, f64)> = if breslow {
                vec![(wd, 0.0)]
            } else {
                (0..d)
                    .map(|r| (wd / d as f64, r as f64 / d as f64))
                    .collect()
            };
            let mut et = EventTime {
                time: t,
                xbar: vec![0.0; q],
                a: 0.0,
                b: vec![0.0; q],
                fa: 0.0,
                fb: vec![0.0; q],
            };
            for &(coef, f) in &parts {
                let e0 = s0 - f * d0;
                let e1: Vec<f64> = (0..q).map(|a| s1[a] - f * d1[a]).collect();
                out.ll -= coef * e0.ln();
                for a in 0..q {
                    out.u[a] -= coef * e1[a] / e0;
                    et.xbar[a] += coef * e1[a] / e0 / wd;
                    et.b[a] += coef * e1[a] / (e0 * e0);
                    et.fb[a] += coef * f * e1[a] / (e0 * e0);
                    if second {
                        for c in 0..q {
                            let e2 = s2[a][c] - f * d2[a][c];
                            out.info[a][c] += coef * (e2 / e0 - e1[a] * e1[c] / (e0 * e0));
                        }
                    }
                }
                et.a += coef / e0;
                et.fa += coef * f / e0;
            }
            here.push(et);
        }
        here.reverse();
        out.times.push(here);
    }
    out
}

/// The score residual of every record at `b` from a pass there.
fn score_residuals(recs: &[Record], b: &[f64], at: &Pass) -> Vec<Vec<f64>> {
    let q = b.len();
    // per stratum the running sums of a and b over the ascending event times
    let sums: Vec<(Vec<f64>, Vec<Vec<f64>>)> = at
        .times
        .iter()
        .map(|times| {
            let mut a = vec![0.0];
            let mut bs = vec![vec![0.0; q]];
            for t in times {
                a.push(a.last().expect("first") + t.a);
                let last = bs.last().expect("first");
                let next = (0..q).map(|k| last[k] + t.b[k]).collect();
                bs.push(next);
            }
            (a, bs)
        })
        .collect();
    recs.iter()
        .map(|r| {
            let times = &at.times[r.stratum];
            let (a, bs) = &sums[r.stratum];
            let lo = times.partition_point(|t| t.time <= r.start);
            let hi = times.partition_point(|t| t.time <= r.stop);
            let e = dot(&r.z, b).exp();
            let mut u: Vec<f64> = (0..q)
                .map(|k| -e * (r.z[k] * (a[hi] - a[lo]) - (bs[hi][k] - bs[lo][k])))
                .collect();
            if r.event {
                let own = &times[hi - 1];
                for (k, uk) in u.iter_mut().enumerate() {
                    *uk += r.z[k] - own.xbar[k] + e * (r.z[k] * own.fa - own.fb[k]);
                }
            }
            u
        })
        .collect()
}

struct CoxFit {
    b: Vec<f64>,
    ll0: f64,
    ll: f64,
    info: Matrix,
    scores: Vec<Vec<f64>>,
    /// (iteration, -2 l, step-halvings, parameters)
    history: Vec<(usize, f64, usize, Vec<f64>)>,
}

fn fit_cox(
    recs: &[Record],
    strata: usize,
    q: usize,
    c: &CsCox,
    warnings: &mut Vec<String>,
) -> CoxFit {
    let k = &c.criteria;
    let mut b = vec![0.0; q];
    let mut at = pass(recs, strata, &b, c.breslow, true);
    let ll0 = at.ll;
    let mut history = vec![(0, -2.0 * ll0, 0, b.clone())];
    let mut iteration = 0;
    while iteration < k.mxiter as usize && q > 0 {
        let inv = information_inverse(NAME, &at.info, warnings);
        let step: Vec<f64> = inv.iter().map(|row| dot(row, &at.u)).collect();
        let mut accepted = None;
        let mut factor = 1.0;
        for halvings in 0..k.mxstep.max(1) as usize {
            let candidate: Vec<f64> = b.iter().zip(&step).map(|(v, s)| v + factor * s).collect();
            let lc = pass(recs, strata, &candidate, c.breslow, false).ll;
            if lc >= at.ll - 1e-12 * (1.0 + at.ll.abs()) {
                accepted = Some((candidate, lc, halvings));
                break;
            }
            factor /= 2.0;
        }
        iteration += 1;
        let Some((candidate, lc, halvings)) = accepted else {
            warnings.push(format!(
                "{NAME}: the log pseudo-likelihood did not increase with any of the {} step sizes (MXSTEP) at iteration {iteration}; the estimates before it are shown",
                k.mxstep
            ));
            break;
        };
        let (pc, relative_p) = k.pconverge;
        let change = b
            .iter()
            .zip(&candidate)
            .map(|(old, new)| {
                let d = (new - old).abs();
                if relative_p {
                    d / (old.abs() + 1e-6)
                } else {
                    d
                }
            })
            .fold(0.0_f64, f64::max);
        let (lcv, relative_l) = k.lconverge;
        let ll_change = if relative_l {
            (lc - at.ll).abs() / (at.ll.abs() + 1e-6)
        } else {
            (lc - at.ll).abs()
        };
        b = candidate;
        at = pass(recs, strata, &b, c.breslow, true);
        history.push((iteration, -2.0 * at.ll, halvings, b.clone()));
        if (pc > 0.0 && change < pc) || (lcv > 0.0 && ll_change < lcv) {
            break;
        }
        if iteration == k.mxiter as usize {
            warnings.push(format!(
                "{NAME}: the estimates did not converge within {} iterations (MXITER)",
                k.mxiter
            ));
        }
    }
    let scores = score_residuals(recs, &b, &at);
    CoxFit {
        b,
        ll0,
        ll: at.ll,
        info: at.info,
        scores,
        history,
    }
}

impl Engine {
    pub(crate) fn cs_cox(&mut self, c: &CsCox) -> Result<Vec<Table>, Error> {
        let mut warnings = Vec::new();
        let tables = self.cs_cox_tables(c, &mut warnings)?;
        self.warn_weight_and_split(NAME);
        for w in warnings {
            self.warn(w);
        }
        Ok(tables)
    }

    fn cs_cox_tables(&self, c: &CsCox, warnings: &mut Vec<String>) -> Result<Vec<Table>, Error> {
        let plan = self.read_plan(&c.plan)?;
        let data = self.active()?;
        let shown = self.shown();
        let design = Design::new(data, &plan, c.include, false)?;
        let factor_names = self.expand(&c.factors)?;
        let covariate_names = self.expand(&c.covariates)?;
        let end_var = data.variable(&c.end)?;
        let start_var = c.start.as_ref().map(|v| data.variable(v)).transpose()?;
        let status_var = data.variable(&c.status)?;
        let strata_var = c.strata.as_ref().map(|v| data.variable(v)).transpose()?;
        let time_names: Vec<String> = start_var
            .iter()
            .chain([&end_var])
            .map(|v| v.name().to_string())
            .collect();
        let mut seen: Vec<String> = Vec::new();
        for n in time_names
            .iter()
            .chain(&factor_names)
            .chain(&covariate_names)
        {
            let upper = n.to_uppercase();
            if seen.contains(&upper) {
                return Err(Error(format!(
                    "{n} appears more than once among the time variables, factors and covariates"
                )));
            }
            if design.plan_vars.contains(&upper) {
                return Err(Error(format!(
                    "{n} is a plan variable and cannot be in the model"
                )));
            }
            seen.push(upper);
        }
        let mut roles = vec![("STATUS", status_var.name().to_string())];
        roles.extend(
            strata_var
                .iter()
                .map(|v| ("BASELINESTRATA", v.name().to_string())),
        );
        roles.extend(c.domain.iter().map(|(v, _)| ("DOMAIN", v.clone())));
        for (role, v) in &roles {
            if seen.contains(&v.to_uppercase()) {
                return Err(Error(format!(
                    "{v} is on the variable list and cannot be the {role} variable"
                )));
            }
        }
        if let Some((v, _)) = &c.domain {
            let upper = v.to_uppercase();
            if design.plan_vars.contains(&upper) && !design.strata_clusters.contains(&upper) {
                return Err(Error(format!(
                    "{v} is a plan variable other than a stratum or cluster and cannot define the DOMAIN"
                )));
            }
        }
        if let Some(v) = start_var.iter().chain([&end_var]).find(|v| v.is_string()) {
            return Err(Error(format!(
                "{NAME} needs numeric time variables, but {} is a string variable",
                v.name()
            )));
        }
        let string_events = c
            .events
            .iter()
            .any(|v| matches!(v, MissingValue::Value(Value::String(_))));
        if string_events != status_var.is_string() {
            return Err(Error(format!(
                "STATUS: {} is {}, so its event values must be {}",
                status_var.name(),
                if string_events { "numeric" } else { "a string" },
                if string_events {
                    "numbers"
                } else {
                    "quoted strings"
                }
            )));
        }
        let factor_vars: Vec<Variable> = factor_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let cov_vars: Vec<Variable> = covariate_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = cov_vars.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "{NAME} needs numeric covariates, but {} is a string variable",
                v.name()
            )));
        }
        let n = design.cases();
        let at = |values: Vec<Option<f64>>| -> Vec<Option<f64>> {
            design.rows.iter().map(|&r| values[r]).collect()
        };
        let keys_at = |v: &Variable| -> Vec<Option<Key>> {
            let all = keys_of(v, c.include);
            design.rows.iter().map(|&r| all[r].clone()).collect()
        };
        let ends = at(numbers(&end_var)?);
        let starts = match &start_var {
            Some(v) => at(numbers(v)?),
            None => vec![Some(f64::NEG_INFINITY); n],
        };
        let status = keys_at(&status_var);
        let strata_keys = strata_var.as_ref().map(keys_at);
        let factor_keys: Vec<Vec<Option<Key>>> = factor_vars.iter().map(keys_at).collect();
        let cov_values: Vec<Vec<Option<f64>>> = cov_vars
            .iter()
            .map(|v| Ok(at(numbers(v)?)))
            .collect::<Result<_, Error>>()?;
        let valid: Vec<bool> = (0..n)
            .map(|i| {
                let times_ok = match (starts[i], ends[i]) {
                    (Some(s), Some(e)) => {
                        e >= 0.0 && (s == f64::NEG_INFINITY || (s >= 0.0 && s < e))
                    }
                    _ => false,
                };
                times_ok
                    && status[i].is_some()
                    && strata_keys.as_ref().is_none_or(|k| k[i].is_some())
                    && factor_keys.iter().all(|k| k[i].is_some())
                    && cov_values.iter().all(|v| v[i].is_some())
            })
            .collect();
        let in_domain: Vec<bool> = match &c.domain {
            None => vec![true; n],
            Some((v, value)) => {
                let variable = data.variable(v)?;
                keys_at(&variable)
                    .iter()
                    .map(|k| {
                        k.as_ref()
                            .is_some_and(|k| value_matches(&variable, k, value))
                    })
                    .collect()
            }
        };
        let rows: Vec<usize> = (0..n).filter(|&i| valid[i] && in_domain[i]).collect();
        if rows.is_empty() {
            return Err(Error(format!(
                "{NAME}: no case in the design{} has valid values on every model variable",
                if c.domain.is_some() {
                    " and the domain"
                } else {
                    ""
                }
            )));
        }
        let event: Vec<bool> = rows
            .iter()
            .map(|&i| crate::km::matches(&c.events, status[i].as_ref().expect("valid")))
            .collect();
        if !event.contains(&true) {
            return Err(Error(format!(
                "{NAME}: no record among the cases analyzed has the event"
            )));
        }
        let sorted_levels = |keys: &[Option<Key>]| -> Vec<Key> {
            let mut found: Vec<Key> = Vec::new();
            for &i in &rows {
                let key = keys[i].as_ref().expect("valid");
                if !found.iter().any(|l| l.compare(key).is_eq()) {
                    found.push(key.clone());
                }
            }
            found.sort_by(|a, b| a.compare(b));
            found
        };
        let position = |levels: &[Key], key: &Key| -> usize {
            levels
                .iter()
                .position(|l| l.compare(key).is_eq())
                .expect("a level")
        };
        let levels: Vec<Vec<Key>> = factor_keys.iter().map(|k| sorted_levels(k)).collect();
        let strata_levels: Vec<Key> = strata_keys
            .as_ref()
            .map(|k| sorted_levels(k))
            .unwrap_or_default();
        let stratum: Vec<usize> = rows
            .iter()
            .map(|&i| match &strata_keys {
                Some(k) => position(&strata_levels, k[i].as_ref().expect("valid")),
                None => 0,
            })
            .collect();
        let strata = strata_levels.len().max(1);
        let labels = Shown::labels(shown.currencies.clone());
        let w_rows: Vec<f64> = rows.iter().map(|&i| design.w[i]).collect();
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
            labelled: factor_vars.iter().any(|v| !v.value_labels().is_empty())
                && shown.values != Show::Plain,
            covariates: covariate_names.clone(),
            case: rows.iter().map(|&i| design.rows[i]).collect(),
            level: rows
                .iter()
                .map(|&i| {
                    factor_keys
                        .iter()
                        .zip(&levels)
                        .map(|(k, ls)| position(ls, k[i].as_ref().expect("valid")))
                        .collect()
                })
                .collect(),
            cov: rows
                .iter()
                .map(|&i| cov_values.iter().map(|v| v[i].expect("valid")).collect())
                .collect(),
            y: vec![Vec::new(); rows.len()],
            f: w_rows.clone(),
            g: vec![1.0; rows.len()],
        };
        // the intercept only finds the redundant columns; the baseline hazard takes its place
        let mut terms = vec![intercept_term()];
        match &c.model {
            Some(model) => terms.extend(model.iter().cloned()),
            None => terms.extend(factor_names.iter().chain(&covariate_names).map(|v| Term {
                variables: vec![v.clone()],
                label: v.clone(),
            })),
        }
        let mut effects = effects_of(&terms, &factor_names, &covariate_names)?;
        let covariate_texts: Vec<String> = cov_vars.iter().map(|v| shown.variable(v)).collect();
        for e in effects
            .iter_mut()
            .filter(|e| !e.is_intercept() && !e.label.contains('('))
        {
            let parts = e.parts.iter().map(|p| match p {
                Part::Factor(f) => d.factor_texts[*f].clone(),
                Part::Covariate(c) => covariate_texts[*c].clone(),
            });
            e.label = parts.collect::<Vec<_>>().join(" * ");
        }
        let fit = Fit::new(&d, effects, &d.y, c.criteria.singular);
        let p = fit.p;
        let cols: Vec<usize> = (1..p).filter(|&j| !fit.redundant[j]).collect();
        // the reference pattern: factors at their last level, covariates at their weighted means
        let total_w: f64 = w_rows.iter().sum();
        let means: Vec<f64> = (0..cov_vars.len())
            .map(|k| {
                (0..rows.len())
                    .map(|r| w_rows[r] * d.cov[r][k])
                    .sum::<f64>()
                    / total_w
            })
            .collect();
        let mut reference = vec![0.0; p];
        for e in fit.effects.iter().filter(|e| !e.is_intercept()) {
            for (k, cell) in e.cells.iter().enumerate() {
                let last = e
                    .factors
                    .iter()
                    .zip(cell)
                    .all(|(&f, &l)| l + 1 == levels[f].len());
                if last {
                    reference[e.start + k] = e.covariates.iter().map(|&v| means[v]).product();
                }
            }
        }
        let recs: Vec<Record> = (0..rows.len())
            .map(|r| Record {
                stratum: stratum[r],
                start: starts[rows[r]].expect("valid"),
                stop: ends[rows[r]].expect("valid"),
                event: event[r],
                w: w_rows[r],
                z: cols.iter().map(|&j| fit.x[r][j] - reference[j]).collect(),
            })
            .collect();
        let q = cols.len();
        let cox = fit_cox(&recs, strata, q, c, warnings);

        // the design covariance and the SRS one
        let mut us = vec![vec![0.0; n]; q];
        for (r, &i) in rows.iter().enumerate() {
            for (u, s) in us.iter_mut().zip(&cox.scores[r]) {
                u[i] = *s;
            }
        }
        let big_n = design.total_w;
        let wants_srs = matches!(
            c.test,
            statscraft_syntax::CsTestType::AdjF | statscraft_syntax::CsTestType::AdjChiSquare
        ) || c.statistics.is_some_and(|s| s.deff || s.deffsqrt);
        let srs_factor = if design.srs_wr {
            Some(1.0)
        } else if (n as f64) < big_n {
            Some(1.0 - n as f64 / big_n)
        } else {
            None
        }
        .filter(|_| n > 1)
        .map(|fpc| fpc * big_n / (n as f64 - 1.0));
        if wants_srs && srs_factor.is_none() {
            warnings.push(format!(
                "{NAME}: the simple random sampling variance is not defined (without replacement it needs a sample smaller than the estimated population), so design effects and adjusted tests of several rows are left empty"
            ));
        }
        let jinv = if q > 0 {
            information_inverse(NAME, &cox.info, warnings)
        } else {
            Vec::new()
        };
        let sandwich = |m: &Matrix| -> Matrix { multiply(&multiply(&jinv, m), &jinv) };
        let v_small = sandwich(&design.covariance(&us));
        // sum w U U' with U about its weighted mean, which is 0 at a converged fit
        let vs_small = srs_factor.filter(|_| wants_srs).map(|f| {
            let mean: Vec<f64> = us.iter().map(|u| dot(u, &design.w) / big_n).collect();
            let m: Matrix = (0..q)
                .map(|a| {
                    (0..q)
                        .map(|b| {
                            let s: f64 = (0..n).map(|i| design.w[i] * us[a][i] * us[b][i]).sum();
                            f * (s - big_n * mean[a] * mean[b])
                        })
                        .collect()
                })
                .collect();
            sandwich(&m)
        });
        let expand = |m: &Matrix| -> Matrix {
            let mut out = vec![vec![0.0; p]; p];
            for (a, &ja) in cols.iter().enumerate() {
                for (b, &jb) in cols.iter().enumerate() {
                    out[ja][jb] = m[a][b];
                }
            }
            out
        };
        let mut b = vec![0.0; p];
        for (s, &j) in cols.iter().enumerate() {
            b[j] = cox.b[s];
        }
        let est = Estimates {
            b,
            v: expand(&v_small),
            vs: vs_small.as_ref().map(expand),
            redundant: (0..p).map(|j| j == 0 || fit.redundant[j]).collect(),
            p,
            logits: 1,
        };
        let names = parameter_names(&d, &fit);
        // a coefficient that a covariate range turns large while its error stays large may be
        // infinite
        for &j in &cols {
            let (lo, hi) = fit
                .x
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| {
                    (lo.min(x[j]), hi.max(x[j]))
                });
            let bj = est.b[j].abs();
            if bj * (hi - lo) >= 10.0 && est.v[j][j].max(0.0).sqrt() >= 3.0 * bj {
                warnings.push(format!(
                    "{NAME}: the estimate of {} may be infinite (perhaps no event in a group), so tests involving it are not reliable",
                    names[j]
                ));
            }
        }

        let df = c.criteria.df.unwrap_or(design.df);
        let level = c.criteria.cilevel;
        let tq = if df > 0.0 {
            t_quantile((1.0 - level / 100.0) / 2.0, df)
        } else {
            f64::NAN
        };
        let ci_group = format!("{level}% Confidence Interval");
        let time_text = match &start_var {
            Some(s) => format!(
                "Time Variables: {} to {}",
                shown.variable(s),
                shown.variable(&end_var)
            ),
            None => format!("Time Variable: {}", shown.variable(&end_var)),
        };
        let caption_base = format!(
            "{time_text}. Status Variable: {}",
            shown.variable(&status_var)
        );
        let model_text = fit
            .effects
            .iter()
            .filter(|e| !e.is_intercept())
            .map(|e| e.label.clone())
            .collect::<Vec<_>>()
            .join(", ");
        let ties = if c.breslow { "Breslow" } else { "Efron" };
        let caption = format!(
            "{caption_base}. Model: {}. Ties: {ties}.",
            if model_text.is_empty() {
                "baseline only"
            } else {
                &model_text
            }
        );
        let mut tables = Vec::new();

        // Sample Design Information
        let event_w: f64 = (0..rows.len())
            .filter(|&r| event[r])
            .map(|r| w_rows[r])
            .sum();
        let events = event.iter().filter(|e| **e).count();
        if c.print.sampleinfo {
            let valid_count = valid.iter().filter(|v| **v).count();
            let valid_w: f64 = (0..n).filter(|&i| valid[i]).map(|i| design.w[i]).sum();
            let mut out = vec![
                vec![
                    text("N of Unweighted Cases"),
                    text("Valid"),
                    Cell::Number(valid_count as f64),
                ],
                vec![
                    text(""),
                    text("Invalid"),
                    Cell::Number((data.case_count() - valid_count) as f64),
                ],
                vec![
                    text(""),
                    text("Total"),
                    Cell::Number(data.case_count() as f64),
                ],
                vec![text("Population Size"), text(""), Cell::Number(valid_w)],
            ];
            if c.domain.is_some() {
                out.push(vec![
                    text("Subpopulation"),
                    text("Unweighted Count"),
                    Cell::Number(rows.len() as f64),
                ]);
                out.push(vec![
                    text(""),
                    text("Population Size"),
                    Cell::Number(total_w),
                ]);
            }
            out.push(vec![
                text("Events"),
                text("Unweighted Count"),
                Cell::Number(events as f64),
            ]);
            out.push(vec![
                text(""),
                text("Population Size"),
                Cell::Number(event_w),
            ]);
            out.push(vec![
                text("Censored"),
                text("Unweighted Count"),
                Cell::Number((rows.len() - events) as f64),
            ]);
            out.push(vec![
                text(""),
                text("Population Size"),
                Cell::Number(total_w - event_w),
            ]);
            let first = &design.stages[0];
            out.push(vec![
                text("Stage 1"),
                text("Strata"),
                Cell::Number(first.n_units.len() as f64),
            ]);
            out.push(vec![
                text(""),
                text("Units"),
                Cell::Number(first.unit_stratum.len() as f64),
            ]);
            out.push(vec![
                text("Sampling Design Degrees of Freedom"),
                text(""),
                Cell::Number(design.df),
            ]);
            let mut cols = plain(&["", ""]);
            cols.push((String::new(), "Value".into()));
            tables.push(table("Sample Design Information", 2, cols, out, None));
        }

        let stratum_text = |g: usize| match &strata_var {
            Some(v) => shown.value(v, &strata_levels[g]),
            None => "Total".to_string(),
        };
        let first_column = match &strata_var {
            Some(v) => shown.variable(v),
            None => String::new(),
        };

        // Event and Censoring Summary
        if c.print.eventinfo {
            let mut out = Vec::new();
            let groups: Vec<Option<usize>> = if strata_var.is_some() {
                (0..strata).map(Some).chain([None]).collect()
            } else {
                vec![None]
            };
            for g in groups {
                let mine: Vec<usize> = (0..rows.len())
                    .filter(|&r| g.is_none_or(|g| stratum[r] == g))
                    .collect();
                let sum = |pick: &dyn Fn(usize) -> bool| -> (f64, f64) {
                    mine.iter()
                        .filter(|&&r| pick(r))
                        .fold((0.0, 0.0), |(w, k), &r| (w + w_rows[r], k + 1.0))
                };
                let (ew, ek) = sum(&|r| event[r]);
                let (cw, ck) = sum(&|r| !event[r]);
                let label = g.map_or_else(|| "Total".to_string(), stratum_text);
                out.push(vec![
                    text(label),
                    Cell::Number(ew),
                    Cell::Number(ek),
                    Cell::Number(cw),
                    Cell::Number(ck),
                    Cell::Number(ew + cw),
                    Cell::Number(ek + ck),
                ]);
            }
            let mut cols = vec![(String::new(), first_column.clone())];
            for group in ["Events", "Censored", "Total"] {
                cols.push((group.into(), "Population Size".into()));
                cols.push((group.into(), "Unweighted Count".into()));
            }
            tables.push(table(
                "Event and Censoring Summary",
                1,
                cols,
                out,
                Some(caption_base.clone() + "."),
            ));
        }

        // Risk and Event Information
        if c.print.riskinfo {
            let mut out = Vec::new();
            for g in 0..strata {
                let mine: Vec<usize> = (0..rows.len()).filter(|&r| stratum[r] == g).collect();
                let mut times: Vec<f64> = mine
                    .iter()
                    .filter(|&&r| event[r])
                    .map(|&r| recs[r].stop)
                    .collect();
                times.sort_by(f64::total_cmp);
                times.dedup();
                for (k, &t) in times.iter().enumerate() {
                    let (mut ew, mut ek, mut rw, mut rk) = (0.0, 0.0, 0.0, 0.0);
                    for &r in &mine {
                        if recs[r].start < t && t <= recs[r].stop {
                            rw += w_rows[r];
                            rk += 1.0;
                            if event[r] && recs[r].stop == t {
                                ew += w_rows[r];
                                ek += 1.0;
                            }
                        }
                    }
                    let mut row = Vec::new();
                    if strata_var.is_some() {
                        row.push(text(if k == 0 {
                            stratum_text(g)
                        } else {
                            String::new()
                        }));
                    }
                    row.extend([
                        Cell::Number(t),
                        Cell::Number(ew),
                        Cell::Number(ek),
                        Cell::Number(rw),
                        Cell::Number(rk),
                    ]);
                    out.push(row);
                }
            }
            let mut cols = if strata_var.is_some() {
                vec![(String::new(), first_column.clone())]
            } else {
                Vec::new()
            };
            cols.push((String::new(), "Time".into()));
            for group in ["Events", "At Risk"] {
                cols.push((group.into(), "Population Size".into()));
                cols.push((group.into(), "Unweighted Count".into()));
            }
            tables.push(table(
                "Risk and Event Information",
                usize::from(strata_var.is_some()),
                cols,
                out,
                Some(caption_base.clone() + "."),
            ));
        }

        // Iteration History
        if let Some(every) = c.print.history {
            let every = every.max(1) as usize;
            let last = cox.history.len() - 1;
            let mut columns = plain(&["Iteration", "-2 Log Pseudo Likelihood", "Step-halving"]);
            columns.extend(
                cols.iter()
                    .map(|&j| ("Parameter".to_string(), names[j].clone())),
            );
            let out = cox
                .history
                .iter()
                .enumerate()
                .filter(|(k, _)| k % every == 0 || *k == last)
                .map(|(_, (iteration, m2ll, halvings, small))| {
                    let mut row = vec![
                        text(iteration.to_string()),
                        Cell::Number(*m2ll),
                        Cell::Number(*halvings as f64),
                    ];
                    row.extend(small.iter().map(|v| Cell::Number(*v)));
                    row
                })
                .collect();
            tables.push(table(
                "Iteration History",
                1,
                columns,
                out,
                Some(caption.clone()),
            ));
        }

        // Model Summary
        tables.push(table(
            "Model Summary",
            1,
            plain(&["", "-2 Log Pseudo Likelihood"]),
            vec![
                vec![text("Baseline Only"), Cell::Number(-2.0 * cox.ll0)],
                vec![text("Model"), Cell::Number(-2.0 * cox.ll)],
            ],
            Some(caption.clone()),
        ));

        // Tests of Model Effects
        let h = fit.h();
        let snap = |v: f64| match v.round() {
            r if (v - r).abs() <= 1e-9 => r + 0.0,
            _ => v,
        };
        let mut lmatrices: Vec<(String, Matrix)> = Vec::new();
        if !cols.is_empty() {
            let mut sources: Vec<(String, Matrix)> = vec![(
                "(Model)".into(),
                cols.iter()
                    .map(|&j| {
                        (0..p)
                            .map(|k| match fit.redundant[k] {
                                false => f64::from(u8::from(k == j)),
                                true => h[j][k],
                            })
                            .collect()
                    })
                    .collect(),
            )];
            for (e, effect) in fit.effects.iter().enumerate().skip(1) {
                sources.push((effect.label.clone(), fit.type34(e, false)));
            }
            let mut out = Vec::new();
            for (label, l) in sources {
                let l: Matrix = l
                    .iter()
                    .map(|row| row.iter().map(|&v| snap(v)).collect())
                    .collect();
                let w = wald(&est, &l, &vec![0.0; l.len()]);
                let mut row = vec![text(label.clone())];
                row.extend(test_cells(c.test, w.as_ref(), df, None));
                out.push(row);
                lmatrices.push((label, l));
            }
            let mut columns = plain(&["Source"]);
            columns.extend(test_columns(c.test));
            tables.push(table(
                "Tests of Model Effects",
                1,
                columns,
                out,
                Some(caption.clone()),
            ));
        }

        // Parameter Estimates
        if let Some(s) = c.statistics.filter(|_| p > 1) {
            let mut columns = plain(&["Parameter"]);
            if s.parameter {
                columns.extend(plain(&["Estimate"]));
            }
            if s.se {
                columns.extend(plain(&["Std. Error"]));
            }
            if s.cinterval && (s.parameter || !s.exp) {
                columns.push((ci_group.clone(), "Lower".into()));
                columns.push((ci_group.clone(), "Upper".into()));
            }
            if s.ttest {
                for leaf in ["t", "df", "Sig."] {
                    columns.push(("Hypothesis Test".into(), leaf.into()));
                }
            }
            if s.exp {
                columns.extend(plain(&["Exp(B)"]));
                if s.cinterval {
                    let group = format!("{level}% Confidence Interval for Exp(B)");
                    columns.push((group.clone(), "Lower".into()));
                    columns.push((group, "Upper".into()));
                }
            }
            if s.deff {
                columns.extend(plain(&["Design Effect"]));
            }
            if s.deffsqrt {
                columns.extend(plain(&["Square Root of Design Effect"]));
            }
            let width = columns.len() - 1;
            let mut out = Vec::new();
            let mut redundant_seen = false;
            for j in 1..p {
                let b = est.b[j];
                if est.redundant[j] {
                    redundant_seen = true;
                    let mut row = vec![text(format!("{} (redundant)", names[j]))];
                    if s.parameter {
                        row.push(Cell::Number(0.0));
                        row.extend(std::iter::repeat_n(Cell::Empty, width - 1));
                    } else {
                        row.extend(std::iter::repeat_n(Cell::Empty, width));
                    }
                    out.push(row);
                    continue;
                }
                let mut row = vec![text(names[j].clone())];
                let se = est.v[j][j].max(0.0).sqrt();
                let (lo, hi) = (b - tq * se, b + tq * se);
                if s.parameter {
                    row.push(number(Some(b)));
                }
                if s.se {
                    row.push(number(Some(se)));
                }
                if s.cinterval && (s.parameter || !s.exp) {
                    row.extend([number(Some(lo)), number(Some(hi))]);
                }
                if s.ttest {
                    let ok = se > 0.0 && df > 0.0;
                    row.push(number(ok.then_some(b / se)));
                    row.push(number(Some(df)));
                    row.push(number(ok.then(|| t_p(b / se, df))));
                }
                if s.exp {
                    row.push(number(Some(b.exp())));
                    if s.cinterval {
                        row.extend([number(Some(lo.exp())), number(Some(hi.exp()))]);
                    }
                }
                let deff = est
                    .vs
                    .as_ref()
                    .map(|vs| vs[j][j])
                    .filter(|v| *v > 0.0)
                    .map(|v| est.v[j][j] / v);
                if s.deff {
                    row.push(number(deff));
                }
                if s.deffsqrt {
                    row.push(number(deff.map(f64::sqrt)));
                }
                out.push(row);
            }
            let mut cap = caption.clone();
            if redundant_seen {
                cap.push_str(
                    " A parameter marked redundant is set to zero because it is redundant.",
                );
            }
            tables.push(table("Parameter Estimates", 1, columns, out, Some(cap)));
        }

        // Covariances and correlations of the parameter estimates
        for (wanted, title) in [
            (c.print.covb, "Covariances of Parameter Estimates"),
            (c.print.corb, "Correlations of Parameter Estimates"),
        ] {
            if !wanted || cols.is_empty() {
                continue;
            }
            let corr = title.starts_with("Corr");
            let mut columns = plain(&["Parameter"]);
            columns.extend(cols.iter().map(|&j| (String::new(), names[j].clone())));
            let out = cols
                .iter()
                .map(|&a| {
                    let mut row = vec![text(names[a].clone())];
                    row.extend(cols.iter().map(|&b| {
                        let v = est.v[a][b];
                        number(Some(if corr {
                            v / (est.v[a][a] * est.v[b][b]).sqrt()
                        } else {
                            v
                        }))
                    }));
                    row
                })
                .collect();
            tables.push(table(title, 1, columns, out, Some(caption.clone())));
        }

        // Custom hypothesis tests
        for (number_of, custom) in c.custom.iter().enumerate() {
            let what = format!("CUSTOM {}", number_of + 1);
            let k_values = custom
                .k
                .clone()
                .unwrap_or_else(|| vec![0.0; custom.rows.len()]);
            if k_values.len() != custom.rows.len() {
                return Err(Error(format!(
                    "{what}: KMATRIX gives {} values for {} rows",
                    k_values.len(),
                    custom.rows.len()
                )));
            }
            let mut l: Matrix = Vec::new();
            for (i, row) in custom.rows.iter().enumerate() {
                l.push(custom_row(&d, &fit, row, &what)?);
                if !fit.estimable(&l[i], &h) {
                    return Err(Error(format!(
                        "{what}: row {} of the L matrix is not estimable",
                        i + 1
                    )));
                }
            }
            let tests: Vec<Option<Wald>> = l
                .iter()
                .zip(&k_values)
                .map(|(row, k)| wald(&est, &vec![row.clone()], &[*k]))
                .collect();
            let ps: Vec<Option<f64>> = tests
                .iter()
                .map(|w| {
                    w.as_ref()
                        .and_then(|w| test_values(c.test, w, df))
                        .map(|(_, p)| p)
                })
                .collect();
            let keyed: Vec<(Vec<f64>, f64)> =
                l.iter().cloned().zip(k_values.iter().copied()).collect();
            let adjusted = padjust(c.padjust, &ps, &keyed);
            let mut out: Vec<Vec<Cell>> = Vec::new();
            for (i, row) in l.iter().enumerate() {
                let e = est.estimate(row);
                let se = est.quad(&est.v, row).max(0.0).sqrt();
                let k = k_values[i];
                let mut cells = vec![
                    text(format!("Contrast {}", i + 1)),
                    number(Some(e)),
                    number(Some(k)),
                    number(Some(e - k)),
                    number(Some(se)),
                    number(Some(e - k - tq * se)),
                    number(Some(e - k + tq * se)),
                ];
                cells.extend(test_cells(c.test, tests[i].as_ref(), df, adjusted[i]));
                out.push(cells);
            }
            let mut overall = vec![text("Overall Test")];
            overall.extend(std::iter::repeat_n(Cell::Empty, 6));
            overall.extend(test_cells(
                c.test,
                wald(&est, &l, &k_values).as_ref(),
                df,
                None,
            ));
            out.push(overall);
            let mut columns = plain(&[
                "Contrast",
                "Estimate",
                "Hypothesized Value",
                "Difference (Estimate - Hypothesized)",
                "Std. Error",
            ]);
            columns.push((ci_group.clone(), "Lower".into()));
            columns.push((ci_group.clone(), "Upper".into()));
            columns.extend(test_columns(c.test));
            let mut cap = caption.clone();
            if let Some(label) = &custom.label {
                cap = format!("{label}. {cap}");
            }
            if let Some(m) = padjust_note(c.padjust) {
                cap.push_str(&format!(
                    " The significance of each contrast is adjusted by the {m} method."
                ));
            }
            tables.push(table("Custom Hypothesis Tests", 1, columns, out, Some(cap)));
            lmatrices.push((custom.label.clone().unwrap_or(what), l));
        }

        // L matrices
        if c.print.lmatrix {
            for (label, l) in &lmatrices {
                let mut columns = plain(&["Contrast"]);
                columns.extend((1..p).map(|j| (String::new(), names[j].clone())));
                let out = l
                    .iter()
                    .enumerate()
                    .map(|(i, row)| {
                        let mut cells = vec![text(format!("L{}", i + 1))];
                        cells.extend((1..p).map(|j| Cell::Number(row[j])));
                        cells
                    })
                    .collect();
                tables.push(table(
                    "Contrast Coefficients (L Matrix)",
                    1,
                    columns,
                    out,
                    Some(format!("{label}. {caption}")),
                ));
            }
        }
        Ok(tables)
    }
}

/// An L row over the design columns: effects with their coefficients, or ALL with one per
/// column but the intercept (the model has none).
fn custom_row(d: &Data, fit: &Fit, row: &LRow, what: &str) -> Result<Vec<f64>, Error> {
    let all = row.iter().any(|(e, _)| e.is_none());
    if !all {
        return l_row(d, fit, row, what);
    }
    if row.iter().any(|(e, _)| e.is_some()) {
        return Err(Error(format!(
            "{what}: a row with ALL cannot also name effects"
        )));
    }
    let mut values = vec![0.0; fit.p];
    for (_, list) in row {
        if list.len() != fit.p - 1 {
            return Err(Error(format!(
                "{what} gives {} coefficients after ALL where the model has {} parameters",
                list.len(),
                fit.p - 1
            )));
        }
        for (j, x) in list.iter().enumerate() {
            values[j + 1] += x;
        }
    }
    Ok(values)
}
