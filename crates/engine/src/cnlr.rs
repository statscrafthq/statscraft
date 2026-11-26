//! CNLR: constrained nonlinear regression, the model of MODEL PROGRAM fitted by minimizing the
//! sum of squared residuals or a loss function of the model program, under bounds, linear
//! constraints and constrained functions.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (CNLR):
//! - The objective is the sum over cases of W (y - f)^2, or with LOSS the sum of W times the loss
//!   variable of the case (weights are not case replicates). Derivatives the program gives are
//!   those of the loss (of PRED without LOSS); the others are estimated numerically.
//! - BOUNDS holds simple bounds, linear constraints and nonlinear bounds, apart by semicolons.
//!   Simple and linear bounds hold at every trial point; a constrained function (computed by the
//!   CONSTRAINED FUNCTIONS program from the parameters) is bounded by a constant and meets its
//!   bound at the solution.
//! - A squared loss gives the tables of NLR (asymptotic statistics, which ignore the bounds);
//!   with a LOSS only the estimates, the iteration history and the stop are given, and BOOTSTRAP
//!   adds standard errors, correlations and the 95% trimmed range from resamples: m = 10 p (p + 1)
//!   / 2 by default; the covariance of the estimates S has the sums of cross products over the
//!   m samples, the standard error is the root of s_jj / (m - 1), the correlation s_ij over the
//!   root of s_ii s_jj, the trimmed range the extreme estimates left after the g = floor(0.025 m)
//!   largest and smallest are removed.
//! - OUTFILE has the parameters, SSE and NCASES; with BOOTSTRAP one case per sample after the
//!   first, which holds the estimates.
//!
//! The manual names a particular package for the solver; this is an own method. Where the
//! manual is silent:
//! - The method: an active-set quasi-Newton method (BFGS on the null space of the active
//!   constraints) keeps the starting point, projected onto the bounds and linear constraints, and
//!   every trial point feasible; the nonlinear constraints are met by an augmented Lagrangian
//!   outer loop. The Hessian starts at the Gauss-Newton matrix for the squared loss.
//! - ITER bounds the iterations (outer iterations count their inner ones); MINORITERATION
//!   the iterations of one inner minimization; CRSHTOL moves a starting value that lies within
//!   that relative distance of a bound onto it; STEPLIMIT bounds the length of a step to that
//!   factor of the length of the parameters (at least 1); FTOLERANCE, LFTOLERANCE and NFTOLERANCE
//!   the violation of constraints that counts as met; LSTOLERANCE the accuracy of the line
//!   search (a curvature condition with that constant, which extends the step); OPTOLERANCE and
//!   FPRECISION: the iterations end when the gradient on the free parameters is at most the
//!   square root of the larger of the two times one plus the objective; ISTEP: a step longer than
//!   that stops the run as unbounded.
//! - A constrained function takes its value from the first case used; one that differs between
//!   cases is refused.
//! - A resample whose fit fails (the model cannot be computed) is left out and counted in a
//!   warning; resamples that stop on ITER count.

use crate::matrix::{Matrix, inverse};
use crate::nlr::{Eval, Fit, Prepared, Runner, Setup};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Engine, Error, Table};
use statscraft_syntax::{BoundRelation, Cnlr, LinearForm, ParameterBound};

/// A linear constraint lower <= a'x <= upper.
struct Row {
    a: Vec<f64>,
    lower: f64,
    upper: f64,
}

/// What BOUNDS says: a box, linear rows and bounds on constrained functions.
#[derive(Default)]
struct Bounds {
    lo: Vec<f64>,
    hi: Vec<f64>,
    rows: Vec<Row>,
    /// (index of the constrained function, lower, upper)
    nonlinear: Vec<(usize, f64, f64)>,
}

fn same_name(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn combine(form: &LinearForm, other: &LinearForm, sign: f64) -> LinearForm {
    let mut out = form.clone();
    for (name, c) in &other.terms {
        match out.terms.iter_mut().find(|(n, _)| same_name(n, name)) {
            Some((_, v)) => *v += sign * c,
            None => out.terms.push((name.clone(), sign * c)),
        }
    }
    out.constant += sign * other.constant;
    out
}

fn resolve_bounds(
    bounds: &[ParameterBound],
    parameters: &[String],
    functions: &[String],
) -> Result<Bounds, Error> {
    let p = parameters.len();
    let mut out = Bounds {
        lo: vec![f64::NEG_INFINITY; p],
        hi: vec![f64::INFINITY; p],
        ..Bounds::default()
    };
    for bound in bounds {
        for (i, relation) in bound.relations.iter().enumerate() {
            // side[i] relation side[i+1], as form <= 0, >= 0 or = 0
            let form = combine(&bound.sides[i], &bound.sides[i + 1], -1.0);
            let names: Vec<&str> = form.terms.iter().map(|(n, _)| n.as_str()).collect();
            let is_parameter = |n: &str| parameters.iter().any(|q| same_name(q, n));
            let is_function = |n: &str| functions.iter().any(|q| same_name(q, n));
            if let Some(unknown) = names.iter().find(|n| !is_parameter(n) && !is_function(n)) {
                return Err(Error(format!(
                    "CNLR /BOUNDS: {unknown} is neither a parameter nor a constrained function"
                )));
            }
            let (low, high) = match relation {
                BoundRelation::Le => (f64::NEG_INFINITY, 0.0),
                BoundRelation::Ge => (0.0, f64::INFINITY),
                BoundRelation::Eq => (0.0, 0.0),
            };
            if names.iter().any(|n| is_function(n)) {
                if names.iter().any(|n| is_parameter(n)) || names.len() != 1 {
                    return Err(Error(
                        "CNLR /BOUNDS: a constrained function cannot be combined with a parameter or another function in one bound"
                            .into(),
                    ));
                }
                let (name, c) = &form.terms[0];
                if *c == 0.0 {
                    continue;
                }
                let k = functions
                    .iter()
                    .position(|q| same_name(q, name))
                    .expect("found");
                // c f + constant in [low, high]
                let (a, b) = ((low - form.constant) / c, (high - form.constant) / c);
                let (lower, upper) = if *c > 0.0 { (a, b) } else { (b, a) };
                out.nonlinear.push((k, lower, upper));
                continue;
            }
            let mut a = vec![0.0; p];
            for (name, c) in &form.terms {
                let j = parameters
                    .iter()
                    .position(|q| same_name(q, name))
                    .expect("found");
                a[j] += c;
            }
            let (lower, upper) = (low - form.constant, high - form.constant);
            let nonzero: Vec<usize> = (0..p).filter(|&j| a[j] != 0.0).collect();
            match nonzero.as_slice() {
                [] => {
                    if lower > 1e-12 || upper < -1e-12 {
                        return Err(Error(
                            "CNLR /BOUNDS: a bound without parameters is not met".into(),
                        ));
                    }
                }
                [j] => {
                    let c = a[*j];
                    let (x, y) = (lower / c, upper / c);
                    let (l, u) = if c > 0.0 { (x, y) } else { (y, x) };
                    out.lo[*j] = out.lo[*j].max(l);
                    out.hi[*j] = out.hi[*j].min(u);
                }
                _ => out.rows.push(Row { a, lower, upper }),
            }
        }
    }
    for (j, name) in parameters.iter().enumerate() {
        if out.lo[j] > out.hi[j] {
            return Err(Error(format!(
                "CNLR /BOUNDS: the bounds of {name} exclude every value"
            )));
        }
    }
    Ok(out)
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

/// The constraints of the box and the rows as `a'x >= b` pairs; the flag marks equalities.
struct Inequality {
    a: Vec<f64>,
    b: f64,
    equality: bool,
}

fn inequalities(bounds: &Bounds, p: usize) -> Vec<Inequality> {
    let mut out = Vec::new();
    let unit = |j: usize, sign: f64| {
        let mut a = vec![0.0; p];
        a[j] = sign;
        a
    };
    for j in 0..p {
        if bounds.lo[j] == bounds.hi[j] {
            out.push(Inequality {
                a: unit(j, 1.0),
                b: bounds.lo[j],
                equality: true,
            });
            continue;
        }
        if bounds.lo[j].is_finite() {
            out.push(Inequality {
                a: unit(j, 1.0),
                b: bounds.lo[j],
                equality: false,
            });
        }
        if bounds.hi[j].is_finite() {
            out.push(Inequality {
                a: unit(j, -1.0),
                b: -bounds.hi[j],
                equality: false,
            });
        }
    }
    for row in &bounds.rows {
        if row.lower == row.upper {
            out.push(Inequality {
                a: row.a.clone(),
                b: row.lower,
                equality: true,
            });
            continue;
        }
        if row.lower.is_finite() {
            out.push(Inequality {
                a: row.a.clone(),
                b: row.lower,
                equality: false,
            });
        }
        if row.upper.is_finite() {
            out.push(Inequality {
                a: row.a.iter().map(|v| -v).collect(),
                b: -row.upper,
                equality: false,
            });
        }
    }
    out
}

/// The point nearest `x` that meets every constraint (alternating projections, Dykstra's
/// correction); None when none is found.
fn feasible_start(x: &[f64], constraints: &[Inequality]) -> Option<Vec<f64>> {
    let violation = |x: &[f64]| {
        constraints
            .iter()
            .map(|c| {
                let g = dot(&c.a, x) - c.b;
                if c.equality { g.abs() } else { (-g).max(0.0) }
            })
            .fold(0.0, f64::max)
    };
    if violation(x) <= 1e-12 {
        return Some(x.to_vec());
    }
    let mut x = x.to_vec();
    let mut corrections = vec![vec![0.0; x.len()]; constraints.len()];
    for _ in 0..20000 {
        for (c, correction) in constraints.iter().zip(corrections.iter_mut()) {
            let y: Vec<f64> = x
                .iter()
                .zip(correction.iter())
                .map(|(a, b)| a + b)
                .collect();
            let g = dot(&c.a, &y) - c.b;
            let n2 = dot(&c.a, &c.a);
            let projected: Vec<f64> = if (c.equality || g < 0.0) && n2 > 0.0 {
                y.iter().zip(&c.a).map(|(v, a)| v - g / n2 * a).collect()
            } else {
                y.clone()
            };
            for k in 0..x.len() {
                correction[k] = y[k] - projected[k];
            }
            x = projected;
        }
        if violation(&x) <= 1e-12 {
            return Some(x);
        }
    }
    (violation(&x) <= 1e-9).then_some(x)
}

/// An orthonormal basis of the null space of the rows `a` (as vectors of length `p`).
fn null_space(rows: &[&Vec<f64>], p: usize) -> Vec<Vec<f64>> {
    let mut basis: Vec<Vec<f64>> = Vec::new();
    let add = |v: Vec<f64>, basis: &mut Vec<Vec<f64>>| -> bool {
        let mut w = v;
        for _ in 0..2 {
            for b in basis.iter() {
                let c = dot(&w, b);
                for (wk, bk) in w.iter_mut().zip(b) {
                    *wk -= c * bk;
                }
            }
        }
        let n = norm(&w);
        if n > 1e-10 {
            basis.push(w.iter().map(|v| v / n).collect());
            true
        } else {
            false
        }
    };
    for r in rows {
        add((*r).clone(), &mut basis);
    }
    let rank = basis.len();
    for k in 0..p {
        let mut e = vec![0.0; p];
        e[k] = 1.0;
        add(e, &mut basis);
    }
    basis.split_off(rank)
}

/// A function the inner minimization works on.
trait Objective {
    /// The value, None where the model cannot be computed.
    fn value(&mut self, x: &[f64]) -> Result<Option<f64>, Error>;
    fn gradient(&mut self, x: &[f64]) -> Result<Vec<f64>, Error>;
}

#[derive(Clone, Copy, PartialEq)]
enum Ended {
    Optimal,
    Iterations,
    NoImprovement,
    Unbounded,
}

struct Tolerances {
    iterations: usize,
    gradient: f64,
    steplimit: f64,
    line_search: f64,
    istep: f64,
}

struct Inner {
    x: Vec<f64>,
    ended: Ended,
    steps: Vec<Vec<f64>>,
    hessian: Matrix,
}

/// Minimizes `f` from the feasible `x0` over the constraints, the Hessian starting at `h0`.
fn minimize(
    f: &mut dyn Objective,
    x0: Vec<f64>,
    constraints: &[Inequality],
    h0: Matrix,
    tol: &Tolerances,
) -> Result<Inner, Error> {
    let p = x0.len();
    let mut x = x0;
    let mut h = h0;
    let mut value = f
        .value(&x)?
        .ok_or_else(|| Error("CNLR: the model cannot be computed at the starting values".into()))?;
    let mut g = f.gradient(&x)?;
    let on = |c: &Inequality, x: &[f64]| (dot(&c.a, x) - c.b).abs() <= 1e-9 * (1.0 + c.b.abs());
    let mut working: Vec<usize> = (0..constraints.len())
        .filter(|&i| constraints[i].equality || on(&constraints[i], &x))
        .collect();
    let mut steps = Vec::new();
    let mut ended = Ended::Iterations;
    for _ in 0..tol.iterations {
        let rows: Vec<&Vec<f64>> = working.iter().map(|&i| &constraints[i].a).collect();
        let z = null_space(&rows, p);
        let q = z.len();
        let gz: Vec<f64> = z.iter().map(|zk| dot(zk, &g)).collect();
        let scale = 1.0 + value.abs();
        if norm(&gz) <= tol.gradient * scale {
            // optimal on the working set: release a constraint with a negative multiplier
            let dropped = if working.is_empty() {
                None
            } else {
                let m = working.len();
                let mut aat: Matrix = vec![vec![0.0; m]; m];
                for i in 0..m {
                    for j in 0..m {
                        aat[i][j] = dot(rows[i], rows[j]);
                    }
                }
                inverse(&aat).and_then(|(inv, _)| {
                    let ag: Vec<f64> = rows.iter().map(|r| dot(r, &g)).collect();
                    let lambda: Vec<f64> = (0..m)
                        .map(|i| (0..m).map(|j| inv[i][j] * ag[j]).sum())
                        .collect();
                    (0..m)
                        .filter(|&i| {
                            !constraints[working[i]].equality && lambda[i] < -tol.gradient * scale
                        })
                        .min_by(|&a, &b| lambda[a].total_cmp(&lambda[b]))
                })
            };
            match dropped {
                Some(i) => {
                    working.remove(i);
                    continue;
                }
                None => {
                    ended = Ended::Optimal;
                    break;
                }
            }
        }
        // the Newton direction on the null space
        let mut hz: Matrix = vec![vec![0.0; q]; q];
        for a in 0..q {
            for b in 0..q {
                let hzb: f64 = (0..p)
                    .map(|r| (0..p).map(|c| z[a][r] * h[r][c] * z[b][c]).sum::<f64>())
                    .sum();
                hz[a][b] = hzb;
            }
        }
        let Some((hinv, _)) = inverse(&hz) else {
            // restart from steepest descent
            h = (0..p)
                .map(|i| (0..p).map(|j| f64::from(u8::from(i == j))).collect())
                .collect();
            continue;
        };
        let coefficients: Vec<f64> = (0..q)
            .map(|a| -(0..q).map(|b| hinv[a][b] * gz[b]).sum::<f64>())
            .collect();
        let mut d = vec![0.0; p];
        for (c, zk) in coefficients.iter().zip(&z) {
            for (dk, zv) in d.iter_mut().zip(zk) {
                *dk += c * zv;
            }
        }
        let slope = dot(&g, &d);
        if slope >= 0.0 {
            h = (0..p)
                .map(|i| (0..p).map(|j| f64::from(u8::from(i == j))).collect())
                .collect();
            continue;
        }
        // the longest step that stays feasible, and the step limit
        let mut alpha_max: f64 = 1.0;
        let mut blocking = None;
        for (i, c) in constraints.iter().enumerate() {
            if working.contains(&i) {
                continue;
            }
            let rate = dot(&c.a, &d);
            if rate < -1e-14 {
                let room = (dot(&c.a, &x) - c.b).max(0.0) / -rate;
                if room < alpha_max {
                    alpha_max = room;
                    blocking = Some(i);
                }
            }
        }
        let dn = norm(&d);
        let limit = tol.steplimit * norm(&x).max(1.0) / dn;
        if limit < alpha_max {
            alpha_max = limit;
            blocking = None;
        }
        if dn * alpha_max > tol.istep {
            ended = Ended::Unbounded;
            break;
        }
        // backtracking line search, with one extension when the slope is still steep
        let mut alpha = alpha_max;
        let mut accepted: Option<(Vec<f64>, f64)> = None;
        while alpha * dn > 1e-14 * (1.0 + norm(&x)) {
            let trial: Vec<f64> = x.iter().zip(&d).map(|(a, b)| a + alpha * b).collect();
            if let Some(v) = f.value(&trial)?
                && v <= value + 1e-4 * alpha * slope
            {
                accepted = Some((trial, v));
                break;
            }
            alpha *= 0.5;
            blocking = None;
        }
        let Some((trial, v)) = accepted else {
            ended = Ended::NoImprovement;
            break;
        };
        let mut next = trial;
        let mut next_value = v;
        // curvature test: a steep slope at the new point and room left: try a longer step once
        let mut gn = f.gradient(&next)?;
        if blocking.is_none() && alpha < alpha_max && dot(&gn, &d) < tol.line_search * slope {
            let longer = (alpha * 2.0).min(alpha_max);
            let trial: Vec<f64> = x.iter().zip(&d).map(|(a, b)| a + longer * b).collect();
            if let Some(v2) = f.value(&trial)?
                && v2 < next_value
            {
                next = trial;
                next_value = v2;
                gn = f.gradient(&next)?;
                if (longer - alpha_max).abs() < 1e-15 {
                    blocking = None;
                }
            }
        }
        // BFGS update of the Hessian
        let s: Vec<f64> = next.iter().zip(&x).map(|(a, b)| a - b).collect();
        let yv: Vec<f64> = gn.iter().zip(&g).map(|(a, b)| a - b).collect();
        let sy = dot(&s, &yv);
        if sy > 1e-12 * norm(&s) * norm(&yv) {
            let hs: Vec<f64> = (0..p).map(|i| dot(&h[i], &s)).collect();
            let shs = dot(&s, &hs);
            for i in 0..p {
                for j in 0..p {
                    h[i][j] += yv[i] * yv[j] / sy - hs[i] * hs[j] / shs;
                }
            }
        }
        let moved = norm(&s);
        x = next;
        value = next_value;
        g = gn;
        steps.push(x.clone());
        if let Some(i) = blocking
            && (dot(&constraints[i].a, &x) - constraints[i].b).abs()
                <= 1e-9 * (1.0 + constraints[i].b.abs())
        {
            working.push(i);
        }
        if moved <= 1e-15 * (1.0 + norm(&x)) {
            ended = Ended::NoImprovement;
            break;
        }
    }
    Ok(Inner {
        x,
        ended,
        steps,
        hessian: h,
    })
}

/// The objective of the estimation: squared residuals or the loss, with the multiplicities of a
/// bootstrap sample, and the nonlinear constraints by augmented Lagrangian terms.
struct Problem<'a> {
    runner: &'a mut Runner,
    y: &'a [f64],
    used: &'a [usize],
    /// How often each used case is drawn (1 outside a bootstrap).
    multiplicity: Vec<f64>,
    squared: bool,
    weights_vary: bool,
    nonlinear: &'a [(usize, f64, f64)],
    /// Multipliers and penalty of the augmented Lagrangian.
    lambda: Vec<f64>,
    rho: f64,
}

impl Problem<'_> {
    fn objective_of(&self, e: &Eval) -> Option<f64> {
        let mut sum = 0.0;
        for (row, &i) in self.used.iter().enumerate() {
            let w = e.weight[i];
            if !w.is_finite() {
                return None;
            }
            let term = if self.squared {
                let f = e.pred[i];
                if !f.is_finite() {
                    return None;
                }
                (self.y[i] - f).powi(2)
            } else {
                let l = e.loss.as_ref().expect("loss column")[i];
                if !l.is_finite() {
                    return None;
                }
                l
            };
            sum += self.multiplicity[row] * w.max(0.0) * term;
        }
        sum.is_finite().then_some(sum)
    }

    /// The constrained functions' values at an evaluation, each constraint's `c(x)`.
    fn constraint_values(&self, e: &Eval) -> Result<Vec<f64>, Error> {
        let mut values = Vec::new();
        for (k, ..) in self.nonlinear {
            let column = &e.constraints[*k];
            let first = column[self.used[0]];
            if column
                .iter()
                .enumerate()
                .filter(|(i, _)| self.used.contains(i))
                .any(|(_, v)| (v - first).abs() > 1e-9 * (1.0 + first.abs()))
            {
                return Err(Error(
                    "CNLR: a constrained function takes different values in different cases; it can depend on the parameters only"
                        .into(),
                ));
            }
            values.push(first);
        }
        Ok(values)
    }

    /// The constraint terms `h_i(x) >= 0` (a lower or an upper side of each bound).
    fn sides(&self, c: &[f64]) -> Vec<(f64, bool)> {
        let mut out = Vec::new();
        for (v, (_, lower, upper)) in c.iter().zip(self.nonlinear) {
            if lower == upper {
                out.push((v - lower, true));
                continue;
            }
            if lower.is_finite() {
                out.push((v - lower, false));
            }
            if upper.is_finite() {
                out.push((upper - v, false));
            }
        }
        out
    }

    fn penalty(&self, c: &[f64]) -> f64 {
        let mut sum = 0.0;
        for ((h, equality), lambda) in self.sides(c).into_iter().zip(&self.lambda) {
            sum += if equality {
                -lambda * h + 0.5 * self.rho * h * h
            } else {
                let t = (lambda - self.rho * h).max(0.0);
                (t * t - lambda * lambda) / (2.0 * self.rho)
            };
        }
        sum
    }

    /// The gradient of the objective alone.
    fn objective_gradient(&mut self, x: &[f64], e: &Eval) -> Result<Vec<f64>, Error> {
        let p = x.len();
        if self.squared {
            let jac =
                crate::nlr::jacobian(self.runner, x, e, self.y, self.used, self.weights_vary)?;
            return Ok((0..p)
                .map(|k| {
                    2.0 * jac
                        .j
                        .iter()
                        .zip(&jac.r)
                        .zip(&self.multiplicity)
                        .map(|((row, r), m)| m * row[k] * r)
                        .sum::<f64>()
                })
                .collect());
        }
        let mut g = vec![0.0; p];
        for (k, gk) in g.iter_mut().enumerate() {
            let column: Vec<f64> = match &e.derivatives[k] {
                Some(d) => d.clone(),
                None => {
                    let h = crate::nlr::step_of(x[k]);
                    let (mut up, mut down) = (x.to_vec(), x.to_vec());
                    up[k] += h;
                    down[k] -= h;
                    let (u, d) = (self.runner.run(&up, false)?, self.runner.run(&down, false)?);
                    let (lu, ld) = (
                        u.loss.as_ref().expect("loss column"),
                        d.loss.as_ref().expect("loss column"),
                    );
                    (0..lu.len()).map(|i| (lu[i] - ld[i]) / (2.0 * h)).collect()
                }
            };
            *gk = self
                .used
                .iter()
                .enumerate()
                .map(|(row, &i)| self.multiplicity[row] * e.weight[i].max(0.0) * column[i])
                .sum();
        }
        Ok(g)
    }
}

impl Objective for Problem<'_> {
    fn value(&mut self, x: &[f64]) -> Result<Option<f64>, Error> {
        let Ok(e) = self.runner.run(x, false) else {
            return Ok(None);
        };
        let Some(v) = self.objective_of(&e) else {
            return Ok(None);
        };
        if self.nonlinear.is_empty() {
            return Ok(Some(v));
        }
        let c = self.constraint_values(&e)?;
        Ok(Some(v + self.penalty(&c)).filter(|v| v.is_finite()))
    }

    fn gradient(&mut self, x: &[f64]) -> Result<Vec<f64>, Error> {
        let e = self.runner.run(x, true)?;
        let mut g = self.objective_gradient(x, &e)?;
        if self.nonlinear.is_empty() {
            return Ok(g);
        }
        // the derivative of every constraint term by central differences
        let c0 = self.constraint_values(&e)?;
        let sides = self.sides(&c0);
        let mut weights = Vec::new();
        for (((h, equality), lambda), _) in sides.iter().zip(&self.lambda).zip(0..) {
            weights.push(if *equality {
                -lambda + self.rho * h
            } else {
                -(lambda - self.rho * h).max(0.0)
            });
        }
        for (k, gk) in g.iter_mut().enumerate() {
            let h = crate::nlr::step_of(x[k]);
            let (mut up, mut down) = (x.to_vec(), x.to_vec());
            up[k] += h;
            down[k] -= h;
            let eu = self.runner.run(&up, false)?;
            let ed = self.runner.run(&down, false)?;
            let cu = self.constraint_values(&eu)?;
            let cd = self.constraint_values(&ed)?;
            let dc: Vec<f64> = cu
                .iter()
                .zip(&cd)
                .map(|(a, b)| (a - b) / (2.0 * h))
                .collect();
            // d h_i / d x_k for the sides of every constraint, in the order of `sides`
            let mut i = 0;
            for (c, (_, lower, upper)) in dc.iter().zip(self.nonlinear) {
                if lower == upper {
                    *gk += weights[i] * c;
                    i += 1;
                    continue;
                }
                if lower.is_finite() {
                    *gk += weights[i] * c;
                    i += 1;
                }
                if upper.is_finite() {
                    *gk -= weights[i] * c;
                    i += 1;
                }
            }
        }
        Ok(g)
    }
}

/// How the whole estimation ended.
struct Solution {
    theta: Vec<f64>,
    objective: f64,
    history: Vec<(usize, f64, Vec<f64>)>,
    ended: Ended,
    infeasible: f64,
}

fn ended_text(ended: Ended) -> &'static str {
    match ended {
        Ended::Optimal => "an optimal solution was found",
        Ended::Iterations => "the maximum number of iterations was reached",
        Ended::NoImprovement => "no further improvement was possible at the accuracy asked for",
        Ended::Unbounded => "a step longer than ISTEP stopped the run: the problem looks unbounded",
    }
}

/// The settings of the solver, from CRITERIA.
struct Settings {
    iterations: usize,
    minor: usize,
    crshtol: f64,
    steplimit: f64,
    ftol: f64,
    lftol: f64,
    nftol: f64,
    lstol: f64,
    gradient: f64,
    istep: f64,
}

fn solve(
    problem: &mut Problem,
    start: &[f64],
    bounds: &Bounds,
    settings: &Settings,
    start_hessian: Matrix,
    record: bool,
) -> Result<Solution, Error> {
    let p = start.len();
    // a starting value close to a bound is put on it
    let mut x0 = start.to_vec();
    for (j, x) in x0.iter_mut().enumerate() {
        for edge in [bounds.lo[j], bounds.hi[j]] {
            if edge.is_finite() && (*x - edge).abs() <= settings.crshtol * (1.0 + edge.abs()) {
                *x = edge;
            }
        }
    }
    let constraints = inequalities(bounds, p);
    let x0 = feasible_start(&x0, &constraints).ok_or_else(|| {
        Error(format!(
            "CNLR: no point meets the bounds and the linear constraints (LFTOLERANCE {})",
            settings.lftol
        ))
    })?;
    let tol = Tolerances {
        iterations: settings.minor,
        gradient: settings.gradient,
        steplimit: settings.steplimit,
        line_search: settings.lstol,
        istep: settings.istep,
    };
    let mut history = Vec::new();
    let mut count = 0usize;
    let mut x = x0;
    let mut hessian = start_hessian;
    let first = problem
        .runner
        .run(&x, false)
        .ok()
        .and_then(|e| problem.objective_of(&e))
        .ok_or_else(|| Error("CNLR: the model cannot be computed at the starting values".into()))?;
    if record {
        history.push((0, first, x.clone()));
    }
    let (mut ended, mut violation);
    let mut previous_violation = f64::INFINITY;
    loop {
        problem.lambda = if problem.nonlinear.is_empty() {
            Vec::new()
        } else {
            problem.lambda.clone()
        };
        if !problem.nonlinear.is_empty() && problem.lambda.is_empty() {
            problem.lambda = vec![0.0; problem.sides(&vec![0.0; problem.nonlinear.len()]).len()];
        }
        let inner = minimize(problem, x.clone(), &constraints, hessian.clone(), &tol)?;
        count += inner.steps.len();
        if record {
            for s in &inner.steps {
                let e = problem.runner.run(s, false)?;
                if let Some(v) = problem.objective_of(&e) {
                    history.push((history.len(), v, s.clone()));
                }
            }
        }
        x = inner.x;
        hessian = inner.hessian;
        ended = inner.ended;
        if problem.nonlinear.is_empty() {
            violation = 0.0;
            break;
        }
        let e = problem.runner.run(&x, false)?;
        let c = problem.constraint_values(&e)?;
        let sides = problem.sides(&c);
        violation = sides
            .iter()
            .map(|(h, equality)| if *equality { h.abs() } else { (-h).max(0.0) })
            .fold(0.0, f64::max);
        if violation <= settings.nftol && ended != Ended::Iterations {
            break;
        }
        if count >= settings.iterations {
            ended = Ended::Iterations;
            break;
        }
        // multipliers, then a stiffer penalty if the violation did not fall by a good part
        let rho = problem.rho;
        for ((h, equality), lambda) in sides.iter().zip(problem.lambda.iter_mut()) {
            *lambda = if *equality {
                *lambda - rho * h
            } else {
                (*lambda - rho * h).max(0.0)
            };
        }
        if violation > 0.25 * previous_violation {
            problem.rho *= 10.0;
        }
        previous_violation = violation;
        if problem.rho > 1e14 {
            ended = Ended::NoImprovement;
            break;
        }
    }
    let e = problem.runner.run(&x, false)?;
    let objective = problem
        .objective_of(&e)
        .ok_or_else(|| Error("CNLR: the model cannot be computed at the estimates".into()))?;
    if count >= settings.iterations && ended == Ended::Optimal {
        ended = Ended::Optimal;
    }
    Ok(Solution {
        theta: x,
        objective,
        history,
        ended,
        infeasible: violation,
    })
}

impl Engine {
    pub(crate) fn cnlr(&mut self, command: &Cnlr) -> Result<Vec<Table>, Error> {
        let program = self
            .model_program
            .clone()
            .ok_or_else(|| Error("CNLR needs a MODEL PROGRAM before it".into()))?;
        // the constrained functions: the variables the program computes, which the bounds name
        let functions: Vec<String> = crate::coxreg::program_targets(&program.constraints);
        let mut seen: Vec<String> = Vec::new();
        for f in functions {
            if !seen.iter().any(|s| same_name(s, &f)) {
                seen.push(f);
            }
        }
        let functions = seen;
        let parameters: Vec<String> = program.parameters.iter().map(|q| q.name.clone()).collect();
        let bounds = resolve_bounds(&command.bounds, &parameters, &functions)?;
        if !bounds.nonlinear.is_empty() && program.constraints.is_empty() {
            return Err(Error(
                "CNLR: a bound on a constrained function needs a CONSTRAINED FUNCTIONS program right before CNLR"
                    .into(),
            ));
        }
        if let Some(loss) = &command.loss
            && crate::coxreg::program_targets(&program.model)
                .iter()
                .all(|t| !same_name(t, loss))
        {
            return Err(Error(format!(
                "CNLR: the model program does not compute {loss}, the loss variable"
            )));
        }
        let squared = command.loss.is_none();
        let setup = Setup {
            dependent: &command.dependent,
            pred: &command.pred,
            file: &command.file,
            outfile: command.outfile.is_some(),
            ckder: command.criteria.ckder,
            loss: command.loss.as_deref(),
            constraint_names: &functions,
        };
        let Prepared {
            mut runner,
            y,
            used,
            names,
            theta,
            start,
            weights_vary,
            n,
        } = self.prepare("CNLR", &setup)?;
        let p = theta.len();
        let cases = y.len();
        let criteria = &command.criteria;
        let nonlinear_count = bounds.nonlinear.len();
        let linear_count = bounds.rows.len();
        let optolerance = criteria.optolerance.unwrap_or(f64::EPSILON.powf(0.8));
        let fprecision = criteria.fprecision.unwrap_or(f64::EPSILON.powf(0.9));
        let feasibility = f64::EPSILON.sqrt();
        let settings = Settings {
            iterations: criteria.iterations.map_or(
                (50).max(3 * (p + linear_count) + 10 * nonlinear_count),
                |i| i as usize,
            ),
            minor: criteria
                .minor_iterations
                .map_or(50.max(3 * (p + linear_count + nonlinear_count)), |i| {
                    i as usize
                }),
            crshtol: criteria.crshtol.unwrap_or(0.01),
            steplimit: criteria.steplimit.unwrap_or(2.0),
            ftol: criteria.ftolerance.unwrap_or(feasibility),
            lftol: criteria
                .lftolerance
                .or(criteria.ftolerance)
                .unwrap_or(feasibility),
            nftol: criteria
                .nftolerance
                .or(criteria.ftolerance)
                .unwrap_or(feasibility),
            lstol: criteria.lstolerance.unwrap_or(0.9),
            gradient: optolerance.max(fprecision).sqrt(),
            istep: criteria.istep.unwrap_or(1e20),
        };
        let _ = settings.ftol;
        // the Hessian starts at the Gauss-Newton matrix for the squared loss, else at the identity
        let identity: Matrix = (0..p)
            .map(|i| (0..p).map(|j| f64::from(u8::from(i == j))).collect())
            .collect();
        let start_hessian = if squared {
            let jac = crate::nlr::jacobian(&mut runner, &theta, &start, &y, &used, weights_vary)?;
            let mut h = identity.clone();
            let mut trace = 0.0;
            for a in 0..p {
                for b in 0..p {
                    h[a][b] = 2.0 * jac.j.iter().map(|r| r[a] * r[b]).sum::<f64>();
                }
                trace += h[a][a];
            }
            for (a, row) in h.iter_mut().enumerate() {
                row[a] += 1e-8 * trace.max(1e-8) / p as f64;
            }
            h
        } else {
            identity.clone()
        };
        let mut problem = Problem {
            runner: &mut runner,
            y: &y,
            used: &used,
            multiplicity: vec![1.0; used.len()],
            squared,
            weights_vary,
            nonlinear: &bounds.nonlinear,
            lambda: Vec::new(),
            rho: 10.0,
        };
        let solution = solve(
            &mut problem,
            &theta,
            &bounds,
            &settings,
            start_hessian.clone(),
            true,
        )?;
        drop(problem);
        if solution.ended != Ended::Optimal {
            self.warn(format!(
                "CNLR stopped because {}",
                ended_text(solution.ended)
            ));
        }
        if solution.infeasible > settings.nftol.max(1e-6) {
            self.warn(format!(
                "CNLR: the constrained functions miss their bounds by {:.3e} at the end",
                solution.infeasible
            ));
        }
        let theta = solution.theta.clone();
        let current = runner.run(&theta, true)?;
        let mut tables = Vec::new();
        let weights_sum_ok = used.iter().all(|&i| current.weight[i].is_finite());
        if squared && weights_sum_ok {
            let jac = crate::nlr::jacobian(&mut runner, &theta, &current, &y, &used, weights_vary)?;
            let stop = ended_text(solution.ended);
            tables.extend(self.statistics_tables(&Fit {
                who: "CNLR",
                dependent: &command.dependent,
                names: &names,
                theta: &theta,
                value: solution.objective,
                y: &y,
                used: &used,
                current: &current,
                jac: &jac,
                history: &solution.history,
                iterations: solution.history.len().saturating_sub(1),
                runs: runner.runs,
                stop,
            }));
        } else {
            tables.push(self.loss_tables(command, &names, &solution, runner.runs));
        }
        // BOOTSTRAP
        let mut samples: Vec<(Vec<f64>, f64, f64)> =
            vec![(theta.clone(), solution.objective, n as f64)];
        if let Some(count) = command.bootstrap {
            let m = count.map_or(10 * p * (p + 1) / 2, |c| c as usize);
            let mut failed = 0;
            let mut estimates: Vec<Vec<f64>> = Vec::new();
            for _ in 0..m {
                // draw the cases of the sample with replacement from the session's generator
                let mut multiplicity = vec![0.0; used.len()];
                for _ in 0..used.len() {
                    let k = ((self.random() * used.len() as f64) as usize).min(used.len() - 1);
                    multiplicity[k] += 1.0;
                }
                let distinct = multiplicity.iter().filter(|m| **m > 0.0).count();
                let mut problem = Problem {
                    runner: &mut runner,
                    y: &y,
                    used: &used,
                    multiplicity,
                    squared,
                    weights_vary,
                    nonlinear: &bounds.nonlinear,
                    lambda: Vec::new(),
                    rho: 10.0,
                };
                match solve(
                    &mut problem,
                    &theta,
                    &bounds,
                    &settings,
                    start_hessian.clone(),
                    false,
                ) {
                    Ok(s) => {
                        estimates.push(s.theta.clone());
                        samples.push((s.theta, s.objective, distinct as f64));
                    }
                    Err(_) => failed += 1,
                }
            }
            if failed > 0 {
                self.warn(format!(
                    "CNLR BOOTSTRAP: {failed} of {m} samples could not be estimated and are left out"
                ));
            }
            if estimates.len() >= 3 {
                tables.extend(bootstrap_tables(&names, &theta, &estimates));
            }
        }
        if command.outfile.is_some() {
            self.outfile_cases(&names, &samples)?;
        }
        self.save_variables("CNLR", &command.save, &mut runner, &theta, &y, &current)?;
        if command.save.loss
            && let (Some(name), Some(loss)) = (&command.loss, &current.loss)
        {
            let values = loss
                .iter()
                .map(|v| Some(*v).filter(|v| v.is_finite()))
                .collect();
            let data = self.active_mut()?;
            if data.variable_index(name).is_some() {
                self.warn(format!(
                    "CNLR /SAVE: the variable {name} exists already, so it is not saved"
                ));
            } else {
                data.set_numeric_values(name, values)?;
                data.set_format(name, &current.format)?;
            }
        }
        let _ = cases;
        Ok(tables)
    }

    /// The iteration history and the estimates of a fit with a loss function.
    fn loss_tables(
        &self,
        command: &Cnlr,
        names: &[String],
        solution: &Solution,
        runs: usize,
    ) -> Table {
        let mut columns = plain(&["Iteration", "Loss Function"]);
        columns.extend(names.iter().map(|n| ("Parameters".to_string(), n.clone())));
        let rows = solution
            .history
            .iter()
            .map(|(k, v, t)| {
                let mut row = vec![Cell::Number(*k as f64), Cell::Number(*v)];
                row.extend(t.iter().map(|x| Cell::Number(*x)));
                row
            })
            .collect();
        table(
            "Iteration History",
            1,
            columns,
            rows,
            Some(format!(
                "Dependent variable: {}. Loss function: {}. Run stopped after {} iterations ({} model evaluations): {}. Final estimates: {}.",
                command.dependent,
                command.loss.as_deref().unwrap_or("squared residuals"),
                solution.history.len().saturating_sub(1),
                runs,
                ended_text(solution.ended),
                names
                    .iter()
                    .zip(&solution.theta)
                    .map(|(n, v)| format!("{n} = {v}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        )
    }
}

/// The bootstrap standard errors, trimmed ranges and correlations of the estimates.
fn bootstrap_tables(names: &[String], theta: &[f64], estimates: &[Vec<f64>]) -> Vec<Table> {
    let p = names.len();
    let m = estimates.len();
    let mean: Vec<f64> = (0..p)
        .map(|j| estimates.iter().map(|e| e[j]).sum::<f64>() / m as f64)
        .collect();
    let mut s = vec![vec![0.0; p]; p];
    for e in estimates {
        for a in 0..p {
            for b in 0..p {
                s[a][b] += (e[a] - mean[a]) * (e[b] - mean[b]);
            }
        }
    }
    let trim = (0.025 * m as f64).floor() as usize;
    let mut columns = plain(&["Parameter", "Estimate", "Std. Error"]);
    columns.push(("95% Trimmed Range".into(), "Lower Bound".into()));
    columns.push(("95% Trimmed Range".into(), "Upper Bound".into()));
    let rows = (0..p)
        .map(|j| {
            let mut sorted: Vec<f64> = estimates.iter().map(|e| e[j]).collect();
            sorted.sort_by(f64::total_cmp);
            vec![
                text(&names[j]),
                Cell::Number(theta[j]),
                Cell::Number((s[j][j] / (m as f64 - 1.0)).sqrt()),
                Cell::Number(sorted[trim]),
                Cell::Number(sorted[m - 1 - trim]),
            ]
        })
        .collect();
    let mut tables = vec![table(
        "Bootstrap Parameter Estimates",
        1,
        columns,
        rows,
        Some(format!("Based on {m} bootstrap samples.")),
    )];
    let rows = (0..p)
        .map(|a| {
            let mut row = vec![text(&names[a])];
            row.extend((0..p).map(|b| Cell::Number(s[a][b] / (s[a][a] * s[b][b]).sqrt())));
            row
        })
        .collect();
    let mut columns = vec![(String::new(), String::new())];
    columns.extend(names.iter().map(|n| (String::new(), n.clone())));
    tables.push(table(
        "Bootstrap Correlation Matrix of the Parameter Estimates",
        1,
        columns,
        rows,
        None,
    ));
    tables
}
