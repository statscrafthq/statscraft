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
//! - Baseline functions per stratum at the reference pattern (centered columns 0), with steps
//!   at its event times: Breslow adds W / S0, Efron (W / d) sum over r of 1 / (S0 - f_r D0),
//!   the product limit multiplies by the alpha of COXREG (H0 = -ln S0). At a pattern z,
//!   H = H0 exp(z'b) and S = S0^exp(z'b).
//! - The variance of H is the design variance of the total of each record's influence
//!   r (v0 - g'W), r = exp(z'b), whatever the baseline method: v0 is the record's own event
//!   over S0 at its time less exp(x'b) times the sum of W / S0^2 over the event times it is at
//!   risk at, g the running sum of W S1 / S0^2 less z times that of W / S0, and W = J^-1 U.
//!   S's interval uses the normal quantile on the scale of S, ln S or ln(-ln S), cut to [0, 1].
//! - Residuals from H0 of the baseline method: martingale delta - (H0(end) - H0(start)) e,
//!   deviance and Cox-Snell from it, Schoenfeld w (x - xbar) at the event, score w U, DFBETA
//!   w J^-1 U.

use super::super::{Design, key_part, keys_of, numbers};
use super::{
    Estimates, Saved, Wald, information_inverse, padjust, padjust_note, parameter_names,
    test_cells, test_columns, test_values, wald,
};
use crate::coxreg::{TimeProgram, program_targets};
use crate::crosstabs::Key;
use crate::descriptives::number;
use crate::dist::{normal_quantile, t_p, t_quantile};
use crate::genlin::value_matches;
use crate::glm::{Data, Fit, Part, effects_of, intercept_term, key_value_text, l_row};
use crate::matrix::{Matrix, multiply};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::shown::{Show, Shown};
use crate::{Cell, Engine, Error, Table};
use serde_json::json;
use statscraft_data::{Dataset, Variable};
use statscraft_syntax::{
    CsCox, CsCoxBaseline, CsCoxCi, CsCoxSave, CsCoxTimeFunction, LRow, MissingValue, Term, Value,
    VarItem,
};
use std::collections::HashMap;

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
    /// The weighted risk-set sums of exp(x'b) and of x exp(x'b), that of exp(x'b) over the
    /// events, their summed weight and the records with the event.
    e0: f64,
    e1: Vec<f64>,
    d0: f64,
    wd: f64,
    deaths: Vec<usize>,
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
                e0: s0,
                e1: s1.clone(),
                d0,
                wd,
                deaths: Vec::new(),
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
            et.deaths = deaths;
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
    /// Per stratum the event times at the estimates.
    times: Vec<Vec<EventTime>>,
}

/// The fit from the parameters `start`.
fn fit_cox(
    recs: &[Record],
    strata: usize,
    q: usize,
    c: &CsCox,
    warnings: &mut Vec<String>,
    start: Vec<f64>,
) -> CoxFit {
    let k = &c.criteria;
    let mut b = start;
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
        times: at.times,
    }
}

/// The subject of every design case and the weight of every subject.
struct Subjects {
    of: Vec<usize>,
    weight: Vec<f64>,
}

/// J^-1 and the covariances J^-1 V J^-1 of the estimates from the score residuals `us` of the
/// design cases: V the design covariance of their total and, with the factor `srs` (the fpc
/// times N / (n - 1) over subjects), the one under simple random sampling of the subjects.
fn covariances(
    design: &Design,
    subjects: &Subjects,
    us: &[Vec<f64>],
    info: &Matrix,
    srs: Option<f64>,
    warnings: &mut Vec<String>,
) -> (Matrix, Matrix, Option<Matrix>) {
    let q = us.len();
    let jinv = if q > 0 {
        information_inverse(NAME, info, warnings)
    } else {
        Vec::new()
    };
    let sandwich = |m: &Matrix| -> Matrix { multiply(&multiply(&jinv, m), &jinv) };
    let v = sandwich(&design.covariance(us));
    // sum w U U' over the subjects with U about its weighted mean, which is 0 at a converged fit
    let vs = srs.map(|f| {
        let mut agg = vec![vec![0.0; subjects.weight.len()]; q];
        for (a, u) in us.iter().enumerate() {
            for (i, x) in u.iter().enumerate() {
                agg[a][subjects.of[i]] += x;
            }
        }
        let big_n: f64 = subjects.weight.iter().sum();
        let mean: Vec<f64> = agg
            .iter()
            .map(|u| dot(u, &subjects.weight) / big_n)
            .collect();
        let m: Matrix = (0..q)
            .map(|a| {
                (0..q)
                    .map(|b| {
                        let s: f64 = (0..subjects.weight.len())
                            .map(|j| subjects.weight[j] * agg[a][j] * agg[b][j])
                            .sum();
                        f * (s - big_n * mean[a] * mean[b])
                    })
                    .collect()
            })
            .collect();
        sandwich(&m)
    });
    (jinv, v, vs)
}

/// A line of a PATTERN: its name and the centered model columns of each step of its path up to
/// the step's end time (one step without end for a fixed pattern).
type Line = (String, Vec<(f64, Vec<f64>)>);
type Lines = Vec<Line>;

/// What CSCOXREG gives besides its tables: SAVE columns, OUTFILE data and charts.
struct Outputs {
    tables: Vec<Table>,
    saved: Vec<Saved>,
    matrix: Option<Dataset>,
    survival: Option<Dataset>,
    model: Option<serde_json::Value>,
    charts: Vec<crate::Chart>,
}

/// The baseline functions of one stratum after each of its event times, with what their
/// variance needs.
struct Base {
    times: Vec<f64>,
    h: Vec<f64>,
    s: Vec<f64>,
    /// Running sums over the event times of wd E1 / E0^2 and of wd / E0 (Breslow's hazard).
    a: Vec<Vec<f64>>,
    b: Vec<f64>,
    /// The design covariance of the totals of v0 and of each record's W = J^-1 U.
    cov: Vec<Matrix>,
}

impl Base {
    /// The cumulative hazard at the last event time at or before `t`, 0 before the first.
    fn h0(&self, t: f64) -> f64 {
        let k = self.times.partition_point(|&s| s <= t);
        if k == 0 { 0.0 } else { self.h[k - 1] }
    }

    /// The cumulative hazard, the survival and the cumulative hazard's standard error at `t`
    /// for the centered pattern `z`.
    fn at(&self, t: f64, z: &[f64], b: &[f64]) -> (f64, f64, f64) {
        let k = self.times.partition_point(|&s| s <= t);
        if k == 0 {
            return (0.0, 1.0, 0.0);
        }
        let j = k - 1;
        let r = dot(z, b).exp();
        // the influence of a record is r (v0 - g'W)
        let g: Vec<f64> = (0..z.len())
            .map(|m| self.a[j][m] - z[m] * self.b[j])
            .collect();
        let c = &self.cov[j];
        let mut var = c[0][0];
        for (m, gm) in g.iter().enumerate() {
            var -= 2.0 * gm * c[0][m + 1];
            for (l, gl) in g.iter().enumerate() {
                var += gm * gl * c[m + 1][l + 1];
            }
        }
        (self.h[j] * r, self.s[j].powf(r), r * var.max(0.0).sqrt())
    }
}

/// The interval of the survival `s` with cumulative hazard `h` and its standard error `se` on
/// the scale `ci`, kept within 0 and 1.
fn limits(s: f64, h: f64, se: f64, zq: f64, ci: CsCoxCi) -> (f64, f64) {
    if se <= 0.0 || h <= 0.0 {
        return (s, s);
    }
    let (lo, hi) = match ci {
        CsCoxCi::Original => (s - zq * s * se, s + zq * s * se),
        CsCoxCi::Log => (s * (-zq * se).exp(), s * (zq * se).exp()),
        CsCoxCi::Lml => (s.powf((zq * se / h).exp()), s.powf((-zq * se / h).exp())),
    };
    (lo.clamp(0.0, 1.0), hi.clamp(0.0, 1.0))
}

/// The baseline functions of every stratum at the estimates `b`. `place` gives each record's
/// design case and `ws` the design cases' W = J^-1 U.
fn baselines(
    recs: &[Record],
    times: &[Vec<EventTime>],
    b: &[f64],
    method: CsCoxBaseline,
    place: &[usize],
    ws: &[Vec<f64>],
    design: &Design,
) -> Vec<Base> {
    let e: Vec<f64> = recs.iter().map(|r| dot(&r.z, b).exp()).collect();
    let q = b.len();
    times
        .iter()
        .enumerate()
        .map(|(g, times)| {
            let mine: Vec<usize> = (0..recs.len()).filter(|&r| recs[r].stratum == g).collect();
            // per record the number of event times through its start and through its end
            let span: Vec<(usize, usize)> = mine
                .iter()
                .map(|&r| {
                    let upto = |x: f64| times.partition_point(|t| t.time <= x);
                    (upto(recs[r].start), upto(recs[r].stop))
                })
                .collect();
            let mut vecs = vec![vec![0.0; design.cases()]];
            vecs.extend(ws.iter().cloned());
            let mut base = Base {
                times: Vec::new(),
                h: Vec::new(),
                s: Vec::new(),
                a: Vec::new(),
                b: Vec::new(),
                cov: Vec::new(),
            };
            // c[k]: the sum of wd / E0^2 over the first k event times
            let mut c = vec![0.0];
            let (mut h, mut s, mut a, mut breslow) = (0.0, 1.0, vec![0.0; q], 0.0);
            for (j, t) in times.iter().enumerate() {
                let step = t.wd / t.e0;
                match method {
                    CsCoxBaseline::Breslow => h += step,
                    CsCoxBaseline::Efron => {
                        let d = t.deaths.len() as f64;
                        h += (0..t.deaths.len())
                            .map(|r| t.wd / d / (t.e0 - r as f64 / d * t.d0))
                            .sum::<f64>();
                    }
                    CsCoxBaseline::ProductLimit => {
                        let deaths: Vec<(f64, f64)> =
                            t.deaths.iter().map(|&i| (recs[i].w, e[i])).collect();
                        let alpha = crate::coxreg::alpha(&deaths, t.e0);
                        s *= alpha;
                        // where every record at risk fails, Breslow's step keeps the hazard finite
                        h += if alpha > 0.0 { -alpha.ln() } else { step };
                    }
                }
                if method != CsCoxBaseline::ProductLimit {
                    s = (-h).exp();
                }
                breslow += step;
                for (ak, e1) in a.iter_mut().zip(&t.e1) {
                    *ak += t.wd * e1 / (t.e0 * t.e0);
                }
                c.push(c[j] + t.wd / (t.e0 * t.e0));
                for (&r, &(lo, hi)) in mine.iter().zip(&span) {
                    let upto = hi.min(j + 1);
                    let own = if recs[r].event && hi <= j + 1 {
                        1.0 / times[hi - 1].e0
                    } else {
                        0.0
                    };
                    let risk = if upto > lo { c[upto] - c[lo] } else { 0.0 };
                    vecs[0][place[r]] = own - e[r] * risk;
                }
                base.times.push(t.time);
                base.h.push(h);
                base.s.push(s);
                base.a.push(a.clone());
                base.b.push(breslow);
                base.cov.push(design.covariance(&vecs));
            }
            base
        })
        .collect()
}

impl Engine {
    pub(crate) fn cs_cox(&mut self, c: &CsCox) -> Result<Vec<Table>, Error> {
        let mut warnings = Vec::new();
        let program = self
            .running_program
            .clone()
            .filter(|p| !p.commands.is_empty());
        let out = self.cs_cox_tables(c, program.as_ref(), &mut warnings)?;
        self.warn_weight_and_split(NAME);
        for w in warnings {
            self.warn(w);
        }
        if let (Some(data), Some((file, _))) = (out.matrix, &c.outfile.matrix) {
            self.write_data_file(file, data)?;
        }
        if let (Some(data), Some(file)) = (out.survival, &c.outfile.survival) {
            self.write_data_file(file, data)?;
        }
        if let (Some(model), Some(file)) = (out.model, &c.outfile.model)
            && !self.dry_run
        {
            self.write_json(NAME, file, "model", &model)?;
        }
        self.charts.extend(out.charts);
        if !out.saved.is_empty() {
            let cases = self.active()?.case_count();
            self.cs_save(NAME, out.saved, cases)?;
        }
        Ok(out.tables)
    }

    fn cs_cox_tables(
        &self,
        c: &CsCox,
        program: Option<&TimeProgram>,
        warnings: &mut Vec<String>,
    ) -> Result<Outputs, Error> {
        let plan = self.read_plan(&c.plan)?;
        let joint = self.joint_file(&plan, &c.plan, c.jointprob.as_deref())?;
        let data = self.active()?;
        let shown = self.shown();
        let id_var = c.id.as_ref().map(|v| data.variable(v)).transpose()?;
        let subject_keys = id_var.as_ref().map(|v| keys_of(v, c.include));
        let design = Design::with_subjects(
            data,
            &plan,
            c.include,
            false,
            joint.as_ref(),
            subject_keys.as_deref(),
        )?;
        // a name that only the time program creates stays as it is
        let expand = |items: &[VarItem]| -> Result<Vec<String>, Error> {
            let mut out = Vec::new();
            for item in items {
                match item {
                    VarItem::Name(n) if program.is_some() && data.variable_index(n).is_none() => {
                        out.push(n.clone())
                    }
                    other => out.extend(self.expand(std::slice::from_ref(other))?),
                }
            }
            Ok(out)
        };
        let factor_names = expand(&c.factors)?;
        let covariate_names = expand(&c.covariates)?;
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
        roles.extend(id_var.iter().map(|v| ("ID", v.name().to_string())));
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
        let computed = program
            .map(|p| program_targets(&p.commands))
            .unwrap_or_default();
        let is_computed = |n: &str| computed.iter().any(|t| t.eq_ignore_ascii_case(n));
        if program.is_some() {
            if data.variable_index("T_").is_some() {
                return Err(Error(
                    "the data has a variable T_, which a TIME PROGRAM reserves for the time; rename it"
                        .into(),
                ));
            }
            if seen.iter().any(|n| n == "T_") {
                return Err(Error(
                    "T_ is the time of a TIME PROGRAM and cannot be a factor or covariate".into(),
                ));
            }
            let others = time_names.iter().chain(roles.iter().map(|(_, v)| v));
            if let Some(v) = others.clone().find(|v| is_computed(v)) {
                return Err(Error(format!(
                    "TIME PROGRAM: only factors and covariates may come from it, but it computes {v}"
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
        let numeric = |vars: &[Variable]| -> Result<(), Error> {
            match vars.iter().find(|v| v.is_string()) {
                Some(v) => Err(Error(format!(
                    "{NAME} needs numeric covariates, but {} is a string variable",
                    v.name()
                ))),
                None => Ok(()),
            }
        };
        // the model variables that the data gives before any time program runs
        let from_data = |n: &&String| program.is_none() || !is_computed(n);
        let screen_factors: Vec<Variable> = factor_names
            .iter()
            .filter(from_data)
            .filter(|n| data.variable_index(n).is_some())
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let screen_covs: Vec<Variable> = covariate_names
            .iter()
            .filter(from_data)
            .filter(|n| data.variable_index(n).is_some())
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        numeric(&screen_covs)?;
        let n = design.cases();
        let at = |values: Vec<Option<f64>>| -> Vec<Option<f64>> {
            design.rows.iter().map(|&r| values[r]).collect()
        };
        let keys_at = |v: &Variable| -> Vec<Option<Key>> {
            let all = keys_of(v, c.include);
            design.rows.iter().map(|&r| all[r].clone()).collect()
        };
        let ends = at(numbers(&end_var)?);
        let subject: Option<Vec<Option<Key>>> = subject_keys
            .as_ref()
            .map(|k| design.rows.iter().map(|&r| k[r].clone()).collect());
        let starts: Vec<Option<f64>> = match (&start_var, &subject) {
            (Some(v), _) => at(numbers(v)?),
            // with ID and one time, a record starts where the subject's previous record ended:
            // previous in time, so the file need not be sorted (ties keep the file order)
            (None, Some(subject)) => {
                let mut order: Vec<usize> = (0..n).filter(|&i| ends[i].is_some()).collect();
                order.sort_by(|&a, &b| ends[a].unwrap().total_cmp(&ends[b].unwrap()));
                let mut last: HashMap<String, f64> = HashMap::new();
                let mut starts = vec![None; n];
                for i in (0..n).filter(|&i| ends[i].is_none()) {
                    starts[i] = subject[i].as_ref().map(|_| f64::NEG_INFINITY);
                }
                for i in order {
                    let Some(k) = subject[i].as_ref() else {
                        continue;
                    };
                    let key = key_part(k);
                    starts[i] = Some(last.get(&key).copied().unwrap_or(f64::NEG_INFINITY));
                    last.insert(key, ends[i].unwrap());
                }
                starts
            }
            (None, None) => vec![Some(f64::NEG_INFINITY); n],
        };
        let status = keys_at(&status_var);
        let strata_keys = strata_var.as_ref().map(keys_at);
        let screen_keys: Vec<Vec<Option<Key>>> = screen_factors.iter().map(keys_at).collect();
        let screen_values: Vec<Vec<Option<f64>>> = screen_covs
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
                    && subject.as_ref().is_none_or(|k| k[i].is_some())
                    && screen_keys.iter().all(|k| k[i].is_some())
                    && screen_values.iter().all(|v| v[i].is_some())
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
        let sorted_levels = |keys: Vec<&Key>| -> Vec<Key> {
            let mut found: Vec<Key> = Vec::new();
            for key in keys {
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
        let strata_levels: Vec<Key> = strata_keys
            .as_ref()
            .map(|k| {
                sorted_levels(
                    rows.iter()
                        .map(|&i| k[i].as_ref().expect("valid"))
                        .collect(),
                )
            })
            .unwrap_or_default();
        let stratum: Vec<usize> = rows
            .iter()
            .map(|&i| match &strata_keys {
                Some(k) => position(&strata_levels, k[i].as_ref().expect("valid")),
                None => 0,
            })
            .collect();
        let strata = strata_levels.len().max(1);
        let w_rows: Vec<f64> = rows.iter().map(|&i| design.w[i]).collect();
        // each record's dataset case and its interval
        let rec_case: Vec<usize> = rows.iter().map(|&i| design.rows[i]).collect();
        let spans: Vec<(f64, f64)> = rows
            .iter()
            .map(|&i| (starts[i].expect("valid"), ends[i].expect("valid")))
            .collect();

        // the model rows (record, source row, start, stop): each record, or with a time program
        // each record at every event time of its stratum within its interval
        let mut layout: Vec<(usize, usize, f64, f64)> = (0..rows.len())
            .map(|r| (r, rec_case[r], spans[r].0, spans[r].1))
            .collect();
        let mut expanded = None;
        if let Some(program) = program {
            let mut times_of: Vec<Vec<f64>> = vec![Vec::new(); strata];
            for r in (0..rows.len()).filter(|&r| event[r]) {
                times_of[stratum[r]].push(spans[r].1);
            }
            for t in &mut times_of {
                t.sort_by(f64::total_cmp);
                t.dedup();
            }
            let mut pieces = Vec::new();
            for &(r, case, start, stop) in &layout {
                let mut from = start;
                for &t in times_of[stratum[r]]
                    .iter()
                    .filter(|&&t| t > start && t <= stop)
                {
                    pieces.push((r, case, from, t));
                    from = t;
                }
            }
            // shortcut: a fixed bound on the expanded rows, as in COXREG
            const MAX_ROWS: usize = 5_000_000;
            if pieces.len() > MAX_ROWS {
                return Err(Error(format!(
                    "TIME PROGRAM: the records at risk at every event time make {} rows, more than the {MAX_ROWS} supported",
                    pieces.len()
                )));
            }
            let take: Vec<Option<usize>> = pieces.iter().map(|p| Some(p.1)).collect();
            let random_before = self.random.clone();
            let keep: Vec<String> = factor_names
                .iter()
                .chain(&covariate_names)
                .cloned()
                .collect();
            let (source, mut ran) = self.run_time_program(
                program,
                &take,
                pieces.iter().map(|p| Some(p.3)).collect(),
                &keep,
            )?;
            warnings.append(&mut ran);
            let mut changes = false;
            for name in &keep {
                changes |= self.program_changes(&source, &take, name)?;
            }
            if !changes {
                // as if the unused program had not run
                self.random.swap(&random_before);
                let out = self.cs_cox_tables(c, None, warnings)?;
                warnings.push(format!(
                    "{NAME}: the TIME PROGRAM changes none of the factors and covariates, so it is not used"
                ));
                return Ok(out);
            }
            layout = pieces
                .into_iter()
                .enumerate()
                .map(|(m, (r, _, start, stop))| (r, m, start, stop))
                .collect();
            expanded = Some(source);
        }
        let time_dependent = expanded.is_some();
        let source = expanded.as_ref().unwrap_or(data);
        let factor_vars: Vec<Variable> = factor_names
            .iter()
            .map(|n| source.variable(n))
            .collect::<Result<_, _>>()?;
        let cov_vars: Vec<Variable> = covariate_names
            .iter()
            .map(|n| source.variable(n))
            .collect::<Result<_, _>>()?;
        numeric(&cov_vars)?;
        let factor_keys: Vec<Vec<Option<Key>>> = factor_vars
            .iter()
            .map(|v| {
                let all = keys_of(v, c.include);
                layout.iter().map(|l| all[l.1].clone()).collect()
            })
            .collect();
        let cov_values: Vec<Vec<Option<f64>>> = cov_vars
            .iter()
            .map(|v| {
                let all = numbers(v)?;
                Ok(layout.iter().map(|l| all[l.1]).collect())
            })
            .collect::<Result<_, Error>>()?;
        // shortcut: a value the time program leaves missing stops the command; leave the record
        // out (as COXREG does) when that is needed
        let missing = (0..layout.len()).find_map(|m| {
            let factor = factor_keys.iter().position(|k| k[m].is_none());
            let covariate = cov_values.iter().position(|v| v[m].is_none());
            match (factor, covariate) {
                (Some(f), _) => Some((m, factor_vars[f].name().to_string())),
                (_, Some(k)) => Some((m, cov_vars[k].name().to_string())),
                _ => None,
            }
        });
        if let Some((m, name)) = missing {
            let k = rec_case[layout[m].0];
            return Err(Error(format!(
                "TIME PROGRAM: {name} is missing for case {} at time {}",
                self.case_numbers.as_ref().map_or(k, |n| n[k]) + 1,
                layout[m].3
            )));
        }
        let levels: Vec<Vec<Key>> = factor_keys
            .iter()
            .map(|k| sorted_levels(k.iter().map(|k| k.as_ref().expect("valid")).collect()))
            .collect();
        let labels = Shown::labels(shown.currencies.clone());
        let w_model: Vec<f64> = layout.iter().map(|l| w_rows[l.0]).collect();
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
            case: layout.iter().map(|l| rec_case[l.0]).collect(),
            level: (0..layout.len())
                .map(|m| {
                    factor_keys
                        .iter()
                        .zip(&levels)
                        .map(|(k, ls)| position(ls, k[m].as_ref().expect("valid")))
                        .collect()
                })
                .collect(),
            cov: (0..layout.len())
                .map(|m| cov_values.iter().map(|v| v[m].expect("valid")).collect())
                .collect(),
            y: vec![Vec::new(); layout.len()],
            f: w_model.clone(),
            g: vec![1.0; layout.len()],
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
        // (over the model rows, which with a time program are the records at each event time)
        let total_w: f64 = w_rows.iter().sum();
        let model_w: f64 = w_model.iter().sum();
        let means: Vec<f64> = (0..cov_vars.len())
            .map(|k| {
                (0..layout.len())
                    .map(|m| w_model[m] * d.cov[m][k])
                    .sum::<f64>()
                    / model_w
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
        let recs: Vec<Record> = layout
            .iter()
            .enumerate()
            .map(|(m, &(r, _, start, stop))| Record {
                stratum: stratum[r],
                start,
                stop,
                event: event[r] && stop == spans[r].1,
                w: w_rows[r],
                z: cols.iter().map(|&j| fit.x[m][j] - reference[j]).collect(),
            })
            .collect();
        // each record's last model row, None for a record in no risk set
        let mut last_row: Vec<Option<usize>> = vec![None; rows.len()];
        for (m, l) in layout.iter().enumerate() {
            last_row[l.0] = Some(m);
        }
        let q = cols.len();
        let cox = fit_cox(&recs, strata, q, c, warnings, vec![0.0; q]);
        let by_record = |scores: &[Vec<f64>], rec_of: &dyn Fn(usize) -> usize| {
            let mut out = vec![vec![0.0; scores.first().map_or(0, Vec::len)]; rows.len()];
            for (m, s) in scores.iter().enumerate() {
                for (o, v) in out[rec_of(m)].iter_mut().zip(s) {
                    *o += v;
                }
            }
            out
        };
        let rec_scores = by_record(&cox.scores, &|m| layout[m].0);

        // the subjects: with ID the records of one ID value, otherwise each design case
        let subjects = {
            let mut index: HashMap<String, usize> = HashMap::new();
            let mut weight = Vec::new();
            let of = (0..n)
                .map(|i| {
                    let key = match subject.as_ref().and_then(|s| s[i].as_ref()) {
                        Some(k) => key_part(k),
                        None => format!("case{i}"),
                    };
                    let next = index.len();
                    let s = *index.entry(key).or_insert(next);
                    if s == weight.len() {
                        weight.push(design.w[i]);
                    }
                    s
                })
                .collect();
            Subjects { of, weight }
        };
        let to_design = |scores: &[Vec<f64>]| -> Matrix {
            let width = scores.first().map_or(0, Vec::len);
            let mut us = vec![vec![0.0; n]; width];
            for (r, &i) in rows.iter().enumerate() {
                for (u, s) in us.iter_mut().zip(&scores[r]) {
                    u[i] = *s;
                }
            }
            us
        };
        let us = to_design(&rec_scores);
        let big_n: f64 = subjects.weight.iter().sum();
        let n_s = subjects.weight.len();
        let wants_srs = matches!(
            c.test,
            statscraft_syntax::CsTestType::AdjF | statscraft_syntax::CsTestType::AdjChiSquare
        ) || c.statistics.is_some_and(|s| s.deff || s.deffsqrt);
        let srs_factor = if design.srs_wr {
            Some(1.0)
        } else if (n_s as f64) < big_n {
            Some(1.0 - n_s as f64 / big_n)
        } else {
            None
        }
        .filter(|_| n_s > 1)
        .map(|fpc| fpc * big_n / (n_s as f64 - 1.0));
        if wants_srs && srs_factor.is_none() {
            warnings.push(format!(
                "{NAME}: the simple random sampling variance is not defined (without replacement it needs a sample smaller than the estimated population), so design effects and adjusted tests of several rows are left empty"
            ));
        }
        let srs = srs_factor.filter(|_| wants_srs);
        let (jinv, v_small, vs_small) =
            covariances(&design, &subjects, &us, &cox.info, srs, warnings);
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
                    .map(|&r| spans[r].1)
                    .collect();
                times.sort_by(f64::total_cmp);
                times.dedup();
                for (k, &t) in times.iter().enumerate() {
                    let (mut ew, mut ek, mut rw, mut rk) = (0.0, 0.0, 0.0, 0.0);
                    for &r in &mine {
                        if spans[r].0 < t && t <= spans[r].1 {
                            rw += w_rows[r];
                            rk += 1.0;
                            if event[r] && spans[r].1 == t {
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

        // General Estimable Function
        if c.print.gef && !cols.is_empty() {
            tables.push(gef_table(&fit, &names, &caption));
        }

        // TESTASSUMPTIONS
        let rec_of: Vec<usize> = layout.iter().map(|l| l.0).collect();
        tables.extend(prophazard_tables(
            &Original {
                recs: &recs,
                rec_of: &rec_of,
                spans: &spans,
                event: &event,
                w: &w_rows,
                strata,
                b: &cox.b,
                times: &cox.times,
                rows: &rows,
                design: &design,
                subjects: &subjects,
                srs,
                names: cols.iter().map(|&j| names[j].clone()).collect(),
            },
            c,
            df,
            &caption,
            warnings,
        )?);

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

        // PATTERN: the centered model columns of each line, the reference values first
        let centered = |lv: &[usize], cv: &[f64]| -> Vec<f64> {
            let mut x = vec![0.0; p];
            for e in fit.effects.iter().filter(|e| !e.is_intercept()) {
                let mine: Vec<usize> = e.factors.iter().map(|&f| lv[f]).collect();
                if let Some(at) = e.cells.iter().position(|c| *c == mine) {
                    x[e.start + at] = e.covariates.iter().map(|&k| cv[k]).product();
                }
            }
            cols.iter().map(|&j| x[j] - reference[j]).collect()
        };
        let same = |a: &String, b: &str| a.eq_ignore_ascii_case(b);
        let mut sets: Vec<(String, Lines)> = vec![(
            "at the reference values".into(),
            vec![("Reference".into(), vec![(f64::INFINITY, vec![0.0; q])])],
        )];
        if time_dependent && !c.patterns.is_empty() {
            warnings.push(format!(
                "{NAME}: PATTERN is not available with covariates from a TIME PROGRAM, so it is ignored"
            ));
        }
        for (k, pattern) in c.patterns.iter().enumerate().filter(|_| !time_dependent) {
            // the factor levels and covariate values of each step of the path
            let ends: Vec<f64> = pattern.path.clone().unwrap_or(vec![f64::INFINITY]);
            let mut steps: Vec<(Vec<usize>, Vec<f64>)> =
                vec![(levels.iter().map(|l| l.len() - 1).collect(), means.clone()); ends.len()];
            for (name, values) in &pattern.values {
                for (s, (lv, cv)) in steps.iter_mut().enumerate() {
                    let value = &values[s.min(values.len() - 1)];
                    if let Some(f) = factor_names.iter().position(|n| same(n, name)) {
                        lv[f] = super::level_of(&factor_vars[f], &levels[f], value).ok_or_else(
                            || {
                                Error(format!(
                                    "PATTERN: {} is not a level of {name} among the records analyzed",
                                    super::value_text(value)
                                ))
                            },
                        )?;
                    } else if let Some(v) = covariate_names.iter().position(|n| same(n, name)) {
                        let Value::Number(x) = value else {
                            return Err(Error(format!(
                                "PATTERN: {name} is a covariate and takes a number"
                            )));
                        };
                        cv[v] = *x;
                    } else {
                        return Err(Error(format!(
                            "PATTERN names {name}, which is not a factor or covariate of {NAME}"
                        )));
                    }
                }
            }
            let path = |steps: &[(Vec<usize>, Vec<f64>)]| -> Vec<(f64, Vec<f64>)> {
                ends.iter()
                    .zip(steps)
                    .map(|(&e, (lv, cv))| (e, centered(lv, cv)))
                    .collect()
            };
            let title = format!("Pattern {}", k + 1);
            let lines = match &pattern.by {
                None => vec![(title.clone(), path(&steps))],
                Some(by) => {
                    let f = factor_names
                        .iter()
                        .position(|n| same(n, by))
                        .filter(|_| !pattern.values.iter().any(|(n, _)| same(n, by)))
                        .ok_or_else(|| {
                            Error(format!(
                                "PATTERN BY {by}: it must be a factor of {NAME} without a value on the PATTERN"
                            ))
                        })?;
                    (0..levels[f].len())
                        .map(|l| {
                            for (lv, _) in steps.iter_mut() {
                                lv[f] = l;
                            }
                            let value = shown.value(&factor_vars[f], &levels[f][l]);
                            (
                                format!("{title}, {} = {value}", d.factor_texts[f]),
                                path(&steps),
                            )
                        })
                        .collect()
                }
            };
            sets.push((format!("for {title}"), lines));
        }

        // the baseline functions, when something needs them; not with a time program
        let method = c.baseline.unwrap_or(if c.breslow {
            CsCoxBaseline::Breslow
        } else {
            CsCoxBaseline::Efron
        });
        let unavailable = |s: &CsCoxSave| {
            !matches!(
                s,
                CsCoxSave::Schoenfeld
                    | CsCoxSave::Score
                    | CsCoxSave::Dfbeta
                    | CsCoxSave::AggScore
                    | CsCoxSave::AggDfbeta
                    | CsCoxSave::Xbeta
            )
        };
        let wants_base = c.print.baseline
            || c.plot.is_some()
            || c.outfile.survival.is_some()
            || c.outfile.model.is_some()
            || c.save.iter().any(|(s, ..)| unavailable(s));
        if time_dependent
            && (c.print.baseline
                || c.plot.is_some()
                || c.outfile.survival.is_some()
                || c.save.iter().any(|(s, ..)| unavailable(s)))
        {
            warnings.push(format!(
                "{NAME}: with covariates from a TIME PROGRAM there are no baseline functions, plots, SURVIVAL file or residuals that need them; SAVE gives only SCHOENFELD, SCORE, DFBETA, their AGG forms and XBETA"
            ));
        }
        let needs_base = wants_base && !time_dependent;
        // each design case's W = J^-1 U
        let ws: Vec<Vec<f64>> = (0..q)
            .map(|k| {
                (0..n)
                    .map(|i| (0..q).map(|l| jinv[k][l] * us[l][i]).sum())
                    .collect()
            })
            .collect();
        let bases = if needs_base {
            baselines(&recs, &cox.times, &cox.b, method, &rows, &ws, &design)
        } else {
            Vec::new()
        };
        let zq = normal_quantile(0.5 + level / 200.0);
        let method_text = match method {
            CsCoxBaseline::Breslow => "Breslow",
            CsCoxBaseline::Efron => "Efron",
            CsCoxBaseline::ProductLimit => "product-limit",
        };
        // (stratum, time, h, s, se of h) of a line at each event time of each stratum
        let curve = |line: &[(f64, Vec<f64>)]| -> Vec<(usize, f64, f64, f64, f64)> {
            if let [(end, z)] = line
                && end.is_infinite()
            {
                let mut out = Vec::new();
                for (g, base) in bases.iter().enumerate() {
                    for &t in &base.times {
                        let (h, s, se) = base.at(t, z, &cox.b);
                        out.push((g, t, h, s, se));
                    }
                }
                return out;
            }
            path_curve(&recs, &cox.times, &bases, &cox.b, line, &rows, &ws, &design)
        };

        // Baseline Functions
        if c.print.baseline && !time_dependent {
            let mut out = Vec::new();
            let mut last = None;
            for (g, t, h, s, se) in curve(&[(f64::INFINITY, vec![0.0; q])]) {
                let mut row = Vec::new();
                if strata_var.is_some() {
                    row.push(text(if last == Some(g) {
                        String::new()
                    } else {
                        stratum_text(g)
                    }));
                }
                last = Some(g);
                let (lo, hi) = limits(s, h, se, zq, c.ci);
                row.extend([t, h, se, s, s * se, lo, hi].map(Cell::Number));
                out.push(row);
            }
            let mut columns = if strata_var.is_some() {
                vec![(String::new(), first_column.clone())]
            } else {
                Vec::new()
            };
            columns.push((String::new(), "Time".into()));
            for leaf in ["Estimate", "Std. Error"] {
                columns.push(("Cumulative Hazard".into(), leaf.into()));
            }
            for leaf in ["Estimate", "Std. Error"] {
                columns.push(("Survival".into(), leaf.into()));
            }
            let group = format!("{level}% Confidence Interval for Survival");
            columns.push((group.clone(), "Lower".into()));
            columns.push((group, "Upper".into()));
            tables.push(table(
                "Baseline Functions",
                usize::from(strata_var.is_some()),
                columns,
                out,
                Some(format!(
                    "{caption} At the reference values: factors at their last level, covariates at their means. Baseline method: {method_text}."
                )),
            ));
        }

        // PLOT: each function for the reference values and for each PATTERN
        let mut charts = Vec::new();
        if let Some(plot) = c.plot.filter(|_| !time_dependent) {
            use crate::chart::{Axis, ChartKind};
            use crate::plot::{Plot, Point, PointGroup};
            let kinds = [
                (plot.survival, "Survival Function", "Cum Survival"),
                (plot.cumhazard, "Hazard Function", "Cum Hazard"),
                (plot.lml, "LML Function", "Log minus log"),
                (plot.oms, "One Minus Survival Function", "1 - Cum Survival"),
            ];
            for (k, (_, title, ylabel)) in kinds.iter().enumerate().filter(|(_, k)| k.0) {
                // the function of the survival and the cumulative hazard
                let value = |s: f64, h: f64| [s, h, h.ln(), 1.0 - s][k];
                for (part, lines) in &sets {
                    let mut groups = Vec::new();
                    for (name, z) in lines {
                        let points = curve(z);
                        for g in 0..bases.len() {
                            let mine: Vec<_> = points.iter().filter(|p| p.0 == g).collect();
                            // (name, (s, h) after each time): the function and its limits
                            let mut drawn = vec![(String::new(), Vec::new())];
                            if plot.ci {
                                drawn.push((" lower".to_string(), Vec::new()));
                                drawn.push((" upper".to_string(), Vec::new()));
                            }
                            for &&(_, _, h, s, se) in &mine {
                                let (lo, hi) = limits(s, h, se, zq, c.ci);
                                let ends = [(s, h), (lo, -lo.ln()), (hi, -hi.ln())];
                                // the other functions fall as the survival rises: swap
                                let ends = if k == 0 {
                                    ends
                                } else {
                                    [ends[0], ends[2], ends[1]]
                                };
                                for (line, end) in drawn.iter_mut().zip(ends) {
                                    line.1.push(end);
                                }
                            }
                            for (suffix, values) in drawn {
                                let mut out = Vec::new();
                                let mut previous = (1.0, 0.0);
                                if k != 2 {
                                    out.push((0.0, value(1.0, 0.0)));
                                }
                                for (&&(_, t, ..), (s, h)) in mine.iter().zip(values) {
                                    if k != 2 || previous.1 > 0.0 {
                                        out.push((t, value(previous.0, previous.1)));
                                    }
                                    out.push((t, value(s, h)));
                                    previous = (s, h);
                                }
                                let label = match &strata_var {
                                    Some(_) => format!("{}, {name}{suffix}", stratum_text(g)),
                                    None => format!("{name}{suffix}"),
                                };
                                groups.push(PointGroup {
                                    name: label,
                                    points: out
                                        .into_iter()
                                        .filter(|p| p.1.is_finite())
                                        .map(|(x, y)| Point {
                                            x,
                                            y,
                                            z: None,
                                            label: None,
                                        })
                                        .collect(),
                                });
                            }
                        }
                    }
                    charts.push(crate::Chart::plot(
                        ChartKind::Line,
                        format!("{title} {part}"),
                        Axis::titled(shown.variable(&end_var)),
                        Axis::titled(*ylabel),
                        Plot::Points {
                            groups,
                            lines: Vec::new(),
                            z_axis: None,
                        },
                    ));
                }
            }
        }

        // SAVE
        let mut saved = Vec::new();
        let cases = data.case_count();
        let column = |values: Vec<f64>| -> Vec<Option<f64>> {
            let mut out = vec![None; cases];
            for (r, v) in values.into_iter().enumerate() {
                out[rec_case[r]] = v.is_finite().then_some(v);
            }
            out
        };
        // per record (each its own model row here): h, s, se of h at its own end time and
        // pattern, and its martingale residual
        let own: Vec<(f64, f64, f64, f64)> = if bases.is_empty() || c.save.is_empty() {
            Vec::new()
        } else {
            recs.iter()
                .map(|r| {
                    let base = &bases[r.stratum];
                    let (h, s, se) = base.at(r.stop, &r.z, &cox.b);
                    let e = dot(&r.z, &cox.b).exp();
                    let m = f64::from(u8::from(r.event)) - (base.h0(r.stop) - base.h0(r.start)) * e;
                    (h, s, se, m)
                })
                .collect()
        };
        let delta = |r: usize| f64::from(u8::from(event[r]));
        // the sum over each subject's records, on its last record and missing on the others
        let aggregate = |values: &dyn Fn(usize) -> f64| -> Vec<f64> {
            let mut sums: HashMap<usize, (f64, usize)> = HashMap::new();
            for (r, &i) in rows.iter().enumerate() {
                let sum = sums.entry(subjects.of[i]).or_insert((0.0, r));
                *sum = (sum.0 + values(r), r);
            }
            (0..rows.len())
                .map(|r| match sums[&subjects.of[rows[r]]] {
                    (sum, last) if last == r => sum,
                    _ => f64::NAN,
                })
                .collect()
        };
        let mut agg_warned = false;
        for (kind, given, count) in &c.save {
            let one = |root: &str, label: &str, values: Vec<f64>| Saved {
                name: given.clone(),
                root: root.into(),
                label: label.into(),
                numbers: Some(column(values)),
                texts: None,
                category: None,
            };
            let per_parameter = |root: &str, label: &str, values: &dyn Fn(usize, usize) -> f64| {
                let root = given.clone().unwrap_or_else(|| root.to_string());
                (0..q.min(*count as usize))
                    .map(|k| Saved {
                        name: None,
                        root: format!("{root}_{}", k + 1),
                        label: format!("{label} for {}", names[cols[k]]),
                        numbers: Some(column((0..rows.len()).map(|r| values(r, k)).collect())),
                        texts: None,
                        category: None,
                    })
                    .collect::<Vec<_>>()
            };
            let limit = |r: usize| {
                let (h, s, se, _) = own[r];
                limits(s, h, se, zq, c.ci)
            };
            let all = |f: &dyn Fn(usize) -> f64| (0..rows.len()).map(f).collect::<Vec<f64>>();
            let is_agg = matches!(
                kind,
                CsCoxSave::AggMartingale
                    | CsCoxSave::AggDeviance
                    | CsCoxSave::AggCoxSnell
                    | CsCoxSave::AggScore
                    | CsCoxSave::AggDfbeta
            );
            if is_agg && subject.is_none() {
                if !std::mem::replace(&mut agg_warned, true) {
                    warnings.push(format!(
                        "{NAME}: aggregated residuals need VARIABLES ID, so SAVE leaves them out"
                    ));
                }
                continue;
            }
            // with a time program the residuals from the baseline are not available (warned)
            if time_dependent && unavailable(kind) {
                continue;
            }
            let score = |r: usize, k: usize| w_rows[r] * rec_scores[r][k];
            let dfbeta = |r: usize, k: usize| w_rows[r] * dot(&jinv[k], &rec_scores[r]);
            match kind {
                CsCoxSave::Schoenfeld => saved.extend(per_parameter(
                    "Resid_Schoenfeld",
                    "Schoenfeld residual",
                    &|r, k| {
                        let Some(rec) = last_row[r].map(|m| &recs[m]).filter(|rec| rec.event)
                        else {
                            return f64::NAN;
                        };
                        let times = &cox.times[rec.stratum];
                        let at = times.partition_point(|t| t.time <= rec.stop) - 1;
                        rec.w * (rec.z[k] - times[at].xbar[k])
                    },
                )),
                CsCoxSave::Score => {
                    saved.extend(per_parameter("Resid_Score", "Score residual", &score))
                }
                CsCoxSave::Dfbeta => saved.extend(per_parameter("Resid_DFBETA", "DFBETA", &dfbeta)),
                CsCoxSave::AggScore => {
                    let sums: Vec<Vec<f64>> = (0..q).map(|k| aggregate(&|r| score(r, k))).collect();
                    saved.extend(per_parameter(
                        "AggResid_Score",
                        "Aggregated score residual",
                        &|r, k| sums[k][r],
                    ))
                }
                CsCoxSave::AggDfbeta => {
                    let sums: Vec<Vec<f64>> =
                        (0..q).map(|k| aggregate(&|r| dfbeta(r, k))).collect();
                    saved.extend(per_parameter(
                        "AggResid_DFBETA",
                        "Aggregated DFBETA",
                        &|r, k| sums[k][r],
                    ))
                }
                CsCoxSave::Martingale => saved.push(one(
                    "Resid_Martingale",
                    "Martingale residual",
                    all(&|r| own[r].3),
                )),
                CsCoxSave::Deviance => saved.push(one(
                    "Resid_Deviance",
                    "Deviance residual",
                    all(&|r| deviance(own[r].3, delta(r))),
                )),
                CsCoxSave::CoxSnell => saved.push(one(
                    "Resid_CoxSnell",
                    "Cox-Snell residual",
                    all(&|r| delta(r) - own[r].3),
                )),
                CsCoxSave::AggMartingale => saved.push(one(
                    "AggResid_Martingale",
                    "Aggregated martingale residual",
                    aggregate(&|r| own[r].3),
                )),
                CsCoxSave::AggDeviance => {
                    let (m, events) = (aggregate(&|r| own[r].3), aggregate(&delta));
                    saved.push(one(
                        "AggResid_Deviance",
                        "Aggregated deviance residual",
                        all(&|r| deviance(m[r], events[r])),
                    ))
                }
                CsCoxSave::AggCoxSnell => saved.push(one(
                    "AggResid_CoxSnell",
                    "Aggregated Cox-Snell residual",
                    aggregate(&|r| delta(r) - own[r].3),
                )),
                CsCoxSave::Xbeta => saved.push(one(
                    "XBETA",
                    "Linear predictor of the predictors centered at their reference values",
                    all(&|r| last_row[r].map_or(f64::NAN, |m| dot(&recs[m].z, &cox.b))),
                )),
                CsCoxSave::Survival => {
                    saved.push(one("Survival", "Survival function", all(&|r| own[r].1)))
                }
                CsCoxSave::LclSurvival => saved.push(one(
                    "LCL_Survival",
                    "Lower confidence limit of the survival function",
                    all(&|r| limit(r).0),
                )),
                CsCoxSave::UclSurvival => saved.push(one(
                    "UCL_Survival",
                    "Upper confidence limit of the survival function",
                    all(&|r| limit(r).1),
                )),
                CsCoxSave::CumHazard => saved.push(one(
                    "CumHazard",
                    "Cumulative hazard function",
                    all(&|r| own[r].0),
                )),
                CsCoxSave::LclCumHazard => saved.push(one(
                    "LCL_CumHazard",
                    "Lower confidence limit of the cumulative hazard function",
                    all(&|r| -limit(r).1.ln()),
                )),
                CsCoxSave::UclCumHazard => saved.push(one(
                    "UCL_CumHazard",
                    "Upper confidence limit of the cumulative hazard function",
                    all(&|r| -limit(r).0.ln()),
                )),
            }
        }

        // OUTFILE
        let parameters: Vec<usize> = (1..p).collect();
        let matrix = c
            .outfile
            .matrix
            .as_ref()
            .map(|(_, corr)| {
                super::matrix_file(&est, &parameters, df, *corr, &|j| names[j].clone())
            })
            .transpose()?;
        let model = c.outfile.model.as_ref().map(|_| {
            let baseline: Vec<_> = bases
                .iter()
                .enumerate()
                .map(|(g, base)| {
                    json!({
                        "stratum": strata_var.as_ref().map(|_| stratum_text(g)),
                        "time": base.times,
                        "cumulative_hazard": base.h,
                        "survival": base.s,
                    })
                })
                .collect();
            json!({
                "format": "statscraft-cs-model",
                "version": 1,
                "command": NAME,
                "plan": c.plan,
                "factors": factor_names,
                "covariates": covariate_names,
                "model": model_text,
                "df": df,
                "ties": ties,
                "baseline_method": method_text,
                "parameters": parameters.iter().map(|&j| json!({
                    "name": names[j],
                    "estimate": est.b[j],
                    "redundant": est.redundant[j],
                    "reference": reference[j],
                })).collect::<Vec<_>>(),
                "covariance": parameters.iter().map(|&a| parameters.iter().map(|&b| {
                    (!est.redundant[a] && !est.redundant[b]).then_some(est.v[a][b])
                }).collect::<Vec<_>>()).collect::<Vec<_>>(),
                "baseline": baseline,
            })
        });
        let survival = match c.outfile.survival.as_ref().filter(|_| !time_dependent) {
            None => None,
            Some(_) => {
                // (line, stratum, time, s, se of s, lower, upper, h)
                let mut rows_out: Vec<(String, String, [f64; 6])> = Vec::new();
                for (_, lines) in &sets {
                    for (name, z) in lines {
                        for (g, t, h, s, se) in curve(z) {
                            let (lo, hi) = limits(s, h, se, zq, c.ci);
                            rows_out.push((
                                name.clone(),
                                stratum_text(g),
                                [t, s, s * se, lo, hi, h],
                            ));
                        }
                    }
                }
                let mut out = Dataset::new(rows_out.len());
                let strings = |k: usize| -> Vec<String> {
                    rows_out
                        .iter()
                        .map(|r| if k == 0 { r.0.clone() } else { r.1.clone() })
                        .collect()
                };
                let width = |v: &[String]| v.iter().map(String::len).max().unwrap_or(1).max(1);
                let patterns = strings(0);
                out.set_string_values("Pattern", width(&patterns), patterns)?;
                if strata_var.is_some() {
                    let strata = strings(1);
                    out.set_string_values("Stratum", width(&strata), strata)?;
                }
                for (k, name) in [
                    "Time",
                    "Survival",
                    "SE_Survival",
                    "LCL_Survival",
                    "UCL_Survival",
                    "CumHazard",
                ]
                .iter()
                .enumerate()
                {
                    out.set_numeric_values(
                        name,
                        rows_out
                            .iter()
                            .map(|r| Some(r.2[k]).filter(|v| v.is_finite()))
                            .collect(),
                    )?;
                }
                Some(out)
            }
        };
        Ok(Outputs {
            tables,
            saved,
            matrix,
            survival,
            model,
            charts,
        })
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

/// The deviance residual from a martingale residual `m` and the number of events `delta`.
fn deviance(m: f64, delta: f64) -> f64 {
    let log = if delta > 0.0 { (delta - m).ln() } else { 0.0 };
    m.signum() * (2.0 * (-m - delta * log)).max(0.0).sqrt()
}

/// The cumulative hazard, the survival and the cumulative hazard's standard error of a
/// piecewise constant path (centered columns up to each step's end time) at each event time of
/// each stratum up to the path's last end time. The baseline's steps are weighted by r =
/// exp(z'b) of the step in force, and the influence of a record sums r (v0 terms) over the event
/// times less G'W with G the sum of r (wd E1 / E0^2 - z wd / E0), as for a fixed pattern.
#[allow(clippy::too_many_arguments)]
fn path_curve(
    recs: &[Record],
    times: &[Vec<EventTime>],
    bases: &[Base],
    b: &[f64],
    path: &[(f64, Vec<f64>)],
    place: &[usize],
    ws: &[Vec<f64>],
    design: &Design,
) -> Vec<(usize, f64, f64, f64, f64)> {
    let q = b.len();
    let last = path.last().map_or(0.0, |s| s.0);
    let e: Vec<f64> = recs.iter().map(|r| dot(&r.z, b).exp()).collect();
    let mut out = Vec::new();
    for (g, (times, base)) in times.iter().zip(bases).enumerate() {
        let mine: Vec<usize> = (0..recs.len()).filter(|&r| recs[r].stratum == g).collect();
        let mut v0 = vec![0.0; design.cases()];
        let mut big_g = vec![0.0; q];
        let (mut h, mut s) = (0.0, 1.0);
        for (j, t) in times.iter().enumerate().take_while(|(_, t)| t.time <= last) {
            let z = &path
                .iter()
                .find(|step| t.time <= step.0)
                .expect("within the path")
                .1;
            let r = dot(z, b).exp();
            let (h_before, s_before) = match j {
                0 => (0.0, 1.0),
                _ => (base.h[j - 1], base.s[j - 1]),
            };
            h += r * (base.h[j] - h_before);
            s *= if s_before > 0.0 {
                (base.s[j] / s_before).powf(r)
            } else {
                0.0
            };
            let c = t.wd / (t.e0 * t.e0);
            for &i in &mine {
                if recs[i].start < t.time && t.time <= recs[i].stop {
                    v0[place[i]] -= r * e[i] * c;
                }
            }
            for &i in &t.deaths {
                v0[place[i]] += r / t.e0;
            }
            for (m, gm) in big_g.iter_mut().enumerate() {
                *gm += r * (t.wd * t.e1[m] / (t.e0 * t.e0) - z[m] * t.wd / t.e0);
            }
            let influence: Vec<f64> = (0..design.cases())
                .map(|i| v0[i] - (0..q).map(|m| big_g[m] * ws[m][i]).sum::<f64>())
                .collect();
            let var = design.covariance(&[influence])[0][0];
            out.push((g, t.time, h, s, var.max(0.0).sqrt()));
        }
    }
    out
}

/// PRINT GEF through the shared basis of the other models; column 0, the intercept the Cox
/// model drops, is neither a parameter nor a free coefficient.
fn gef_table(fit: &Fit, names: &[String], caption: &str) -> Table {
    let mut redundant = fit.redundant.clone();
    redundant[0] = true;
    let basis = super::gef_basis(&fit.h(), &redundant);
    let order: Vec<usize> = (1..fit.p).collect();
    super::gef_table(&basis, &order, &|k| names[k].clone(), caption.to_string())
}

/// What the test of proportional hazards needs from the fitted model.
struct Original<'a> {
    recs: &'a [Record],
    /// The record of each model row, and of each record its interval, event and weight.
    rec_of: &'a [usize],
    spans: &'a [(f64, f64)],
    event: &'a [bool],
    w: &'a [f64],
    strata: usize,
    b: &'a [f64],
    times: &'a [Vec<EventTime>],
    /// The design cases of the records, the subjects and the SRS factor.
    rows: &'a [usize],
    design: &'a Design,
    subjects: &'a Subjects,
    srs: Option<f64>,
    names: Vec<String>,
}

/// TESTASSUMPTIONS: the alternative model adds every predictor times g(t), its coefficients
/// fitted from (b, 0) on the model rows split at the event times; the Wald test of the added
/// coefficients uses the design covariance as the model's own tests do.
fn prophazard_tables(
    o: &Original,
    c: &CsCox,
    df: f64,
    caption: &str,
    warnings: &mut Vec<String>,
) -> Result<Vec<Table>, Error> {
    let Some(a) = c.assumptions else {
        return Ok(Vec::new());
    };
    let q = o.b.len();
    if q == 0 {
        warnings.push(format!(
            "{NAME}: the model has no predictors, so there is no test of proportional hazards"
        ));
        return Ok(Vec::new());
    }
    // the distinct event times of all records, whatever their baseline stratum
    let mut all: Vec<f64> = (0..o.spans.len())
        .filter(|&r| o.event[r])
        .map(|r| o.spans[r].1)
        .collect();
    all.sort_by(f64::total_cmp);
    all.dedup();
    let weights = o.w;
    // the weighted Kaplan-Meier survival just after each event time
    let mut km = Vec::with_capacity(all.len());
    let mut s = 1.0;
    for &t in &all {
        let (mut died, mut at_risk) = (0.0, 0.0);
        for (r, &(start, stop)) in o.spans.iter().enumerate() {
            if start < t && t <= stop {
                at_risk += weights[r];
                if o.event[r] && stop == t {
                    died += weights[r];
                }
            }
        }
        s *= 1.0 - died / at_risk;
        km.push(s);
    }
    let (g_text, label) = match a.function {
        CsCoxTimeFunction::Km => ("1 - Kaplan-Meier survival", "(1 - KM)"),
        CsCoxTimeFunction::Identity => ("time", "T"),
        CsCoxTimeFunction::Log => ("log of time", "ln(T)"),
        CsCoxTimeFunction::Rank => ("rank of the event time", "rank(T)"),
    };
    if a.function == CsCoxTimeFunction::Log && all[0] <= 0.0 {
        return Err(Error(format!(
            "TESTASSUMPTIONS PROPHAZARD=LOG needs event times above 0, but one is {}",
            all[0]
        )));
    }
    let g = |t: f64| -> f64 {
        let k = all.partition_point(|&x| x <= t);
        match a.function {
            CsCoxTimeFunction::Km => 1.0 - if k == 0 { 1.0 } else { km[k - 1] },
            CsCoxTimeFunction::Identity => t,
            CsCoxTimeFunction::Log => t.ln(),
            CsCoxTimeFunction::Rank => (k + 1) as f64,
        }
    };
    // the model rows split at the event times of their stratum: (model row, record)
    let mut alt = Vec::new();
    let mut origin = Vec::new();
    for (m, rec) in o.recs.iter().enumerate() {
        let mut from = rec.start;
        for t in o.times[rec.stratum]
            .iter()
            .filter(|t| t.time > rec.start && t.time <= rec.stop)
        {
            let gt = g(t.time);
            let mut z = rec.z.clone();
            z.extend(rec.z.iter().map(|x| x * gt));
            alt.push(Record {
                stratum: rec.stratum,
                start: from,
                stop: t.time,
                event: rec.event && t.time == rec.stop,
                w: rec.w,
                z,
            });
            origin.push(o.rec_of[m]);
            from = t.time;
        }
    }
    let mut start = o.b.to_vec();
    start.resize(2 * q, 0.0);
    let fit = fit_cox(&alt, o.strata, 2 * q, c, warnings, start);
    let n = o.design.cases();
    let mut us = vec![vec![0.0; n]; 2 * q];
    for (piece, scores) in fit.scores.iter().enumerate() {
        let i = o.rows[origin[piece]];
        for (u, s) in us.iter_mut().zip(scores) {
            u[i] += s;
        }
    }
    let (_, v, vs) = covariances(o.design, o.subjects, &us, &fit.info, o.srs, warnings);
    let est = Estimates {
        b: fit.b.clone(),
        v,
        vs,
        redundant: vec![false; 2 * q],
        p: 2 * q,
        logits: 1,
    };
    let l: Matrix = (0..q)
        .map(|k| {
            (0..2 * q)
                .map(|j| f64::from(u8::from(j == q + k)))
                .collect()
        })
        .collect();
    let w = wald(&est, &l, &vec![0.0; q]);
    let cap = format!(
        "{caption} Alternative model: each predictor also enters times g(t) = {g_text}; the test is that all those coefficients are 0."
    );
    let mut columns = plain(&["Time Function"]);
    columns.extend(test_columns(c.test));
    let mut row = vec![text(match a.function {
        CsCoxTimeFunction::Km => "Kaplan-Meier",
        CsCoxTimeFunction::Identity => "Identity",
        CsCoxTimeFunction::Log => "Log",
        CsCoxTimeFunction::Rank => "Rank",
    })];
    row.extend(test_cells(c.test, w.as_ref(), df, None));
    let mut tables = vec![table(
        "Overall Test of Proportional Hazards",
        1,
        columns,
        vec![row],
        Some(cap.clone()),
    )];
    let mut names = o.names.clone();
    names.extend(o.names.iter().map(|n| format!("{n} * {label}")));
    if a.parameter {
        let rows = (0..2 * q)
            .map(|j| {
                vec![
                    text(names[j].clone()),
                    number(Some(est.b[j])),
                    number(Some(est.v[j][j].max(0.0).sqrt())),
                ]
            })
            .collect();
        tables.push(table(
            "Parameter Estimates of the Alternative Model",
            1,
            plain(&["Parameter", "Estimate", "Std. Error"]),
            rows,
            Some(cap.clone()),
        ));
    }
    if a.covb {
        let mut columns = plain(&["Parameter"]);
        columns.extend(names.iter().map(|n| (String::new(), n.clone())));
        let rows = (0..2 * q)
            .map(|i| {
                let mut row = vec![text(names[i].clone())];
                row.extend((0..2 * q).map(|j| number(Some(est.v[i][j]))));
                row
            })
            .collect();
        tables.push(table(
            "Covariances of the Alternative Model Parameters",
            1,
            columns,
            rows,
            Some(cap),
        ));
    }
    Ok(tables)
}
