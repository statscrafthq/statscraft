//! VARCOMP: variance components of a linear model y = X0 b + sum_i Z_i u_i + e.
//!
//! Rules (our notes from the Command Syntax Reference and the Algorithms manual, VARCOMP, and
//! the expected mean squares of the GLM mixed model):
//! - Cases, design and effects as in UNIANOVA. An effect holding a factor named on RANDOM is
//!   random (Z_i, all its columns), the others fixed (X0, without its redundant columns). Every
//!   u_i has variance s_i^2 I and e has s_e^2 I; the regression weight w turns each row into
//!   sqrt(w) times itself, and a case counts as often as its rounded frequency weight.
//! - Everything comes from the cross products of [X0 | Z | y]. With ratios a_i on the random
//!   effects, V = I + sum a_i Z_i Z_i' and R = V^-1 - V^-1 X0 (X0'V^-1 X0)^- X0'V^-1 equals
//!   I - W O^-1 W' for W = [X0 | Z D^1/2], O = W'W plus the identity on the Z part; log |V| is
//!   log |I + D^1/2 Z'Z D^1/2| and log |V| + log |X0'V^-1 X0| is log |O|.
//! - MINQUE(a): solve S s = q with S_ij = SSQ(Z_i'R Z_j), S_i,e = SSQ(Z_i'R), S_e,e = SSQ(R),
//!   q_i = SSQ(Z_i'R y), q_e = SSQ(R y), prior a = 0 (MINQUE(0)) or 1 (MINQUE(1)).
//! - ML and REML in theta = (g_1..g_k, s_e^2), g_i = s_i^2 / s_e^2, the fixed effects profiled
//!   out (ML) or projected out (REML, n - rank X0 degrees of freedom). The log likelihood, its
//!   gradient, Hessian and expected Hessian follow the manual's formulas. Starting values: s_e^2
//!   the residual mean square (divisor n) of the model with every effect fixed, s_i^2 the
//!   variance (divisor m_i - 1) of the least squares estimates of Z_i alone; a residual of 0
//!   becomes 1E-8. The first step is Fisher scoring, then Newton while the negative Hessian is
//!   positive definite and the last step raised the log likelihood by at most 1; a step is
//!   halved up to 10 times until the log likelihood rises; a ratio below 0 becomes 0, and one at
//!   0 whose gradient points below 0 stays out of the step. Converged when the change of the log
//!   likelihood and the sum of absolute changes of theta are both below CONVERGE times the
//!   larger of 1 and their previous size. The covariance of the variances is J F^-1 J' with F
//!   the expected information in theta and J the Jacobian from theta to the variances.
//! - SSTYPE(n): each effect's hypothesis L of Type I or III gives SS = (Lb)'(LGL')^-(Lb) and
//!   df = rank (LGL'); the coefficient of s_i^2 in its expected mean square is
//!   tr((LGL')^- L_i L_i') / df, L_i the columns of L H of effect i (L H = L G X'X); the error
//!   has 1. A fixed effect with nonzero columns in L H enters the quadratic term. The random
//!   rows and the error row give a square system for the variances.

use crate::glm::{
    Fit, Part, between_factors, default_terms, effects_of, ginverse, intercept_term, model_data,
    sweep_in_order,
};
use crate::matrix::{Matrix, inverse, multiply, transpose};
use crate::mixed::{cholesky_inverse, matrix_table};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Variable};
use statscraft_syntax::{Varcomp, VarcompMethod};
use std::ops::Range;

/// The cross products the likelihood and MINQUE need.
struct Model {
    /// Over [X0 | Z_1 .. Z_k | y]; y is the last row and column.
    t: Matrix,
    /// The columns of X0.
    r: usize,
    /// The columns of each Z_i in `t`.
    blocks: Vec<Range<usize>>,
    n: f64,
    /// log |X0'X0|.
    log_xx: f64,
}

/// The quadratic forms of R at given ratios, over U = [Z | y] (y last).
struct Forms {
    /// U'R U.
    ur: Matrix,
    /// U'V^-1 U, for ML.
    uv: Matrix,
    log_omega: f64,
    log_m: f64,
    /// U'R^2 U and tr R^2, for MINQUE.
    r2: Option<(Matrix, f64)>,
}

fn ssq(m: &Matrix, rows: &Range<usize>, cols: &Range<usize>) -> f64 {
    rows.clone()
        .map(|i| cols.clone().map(|j| m[i][j] * m[i][j]).sum::<f64>())
        .sum()
}

fn trace(m: &Matrix, range: &Range<usize>) -> f64 {
    range.clone().map(|i| m[i][i]).sum()
}

/// a - b c' for matrices with rows of the same length.
fn less_product(a: &Matrix, b: &Matrix, c: &Matrix) -> Matrix {
    let bc = multiply(b, &transpose(c));
    a.iter()
        .zip(&bc)
        .map(|(x, y)| x.iter().zip(y).map(|(u, v)| u - v).collect())
        .collect()
}

impl Model {
    fn k(&self) -> usize {
        self.blocks.len()
    }

    /// The index of y in `t`, which is also the number of columns of W.
    fn m(&self) -> usize {
        self.t.len() - 1
    }

    /// The blocks in U coordinates.
    fn u_blocks(&self) -> Vec<Range<usize>> {
        self.blocks
            .iter()
            .map(|b| b.start - self.r..b.end - self.r)
            .collect()
    }

    fn forms(&self, alpha: &[f64], minque: bool) -> Option<Forms> {
        let (m, r) = (self.m(), self.r);
        let mut s = vec![1.0; m];
        for (b, a) in self.blocks.iter().zip(alpha) {
            for v in &mut s[b.clone()] {
                *v = a.max(0.0).sqrt();
            }
        }
        let ww: Matrix = (0..m)
            .map(|i| (0..m).map(|j| s[i] * s[j] * self.t[i][j]).collect())
            .collect();
        let mut omega = ww.clone();
        for (i, row) in omega.iter_mut().enumerate().skip(r) {
            row[i] += 1.0;
        }
        let (omega_inv, log_omega) = cholesky_inverse(&omega)?;
        let mm: Matrix = omega[r..].iter().map(|row| row[r..].to_vec()).collect();
        let (m_inv, log_m) = cholesky_inverse(&mm)?;
        let uu: Matrix = self.t[r..].iter().map(|row| row[r..].to_vec()).collect();
        // T_UW, with the Z columns scaled
        let tuw: Matrix = self.t[r..]
            .iter()
            .map(|row| (0..m).map(|j| row[j] * s[j]).collect())
            .collect();
        let g = multiply(&tuw, &omega_inv);
        let ur = less_product(&uu, &g, &tuw);
        let tuz: Matrix = tuw.iter().map(|row| row[r..].to_vec()).collect();
        let uv = less_product(&uu, &multiply(&tuz, &m_inv), &tuz);
        let r2 = minque.then(|| {
            // R^2 = I - 2 W O^-1 W' + W O^-1 W'W O^-1 W'
            let gw = multiply(&g, &ww);
            let twice: Matrix = uu
                .iter()
                .zip(&ur)
                .map(|(a, b)| a.iter().zip(b).map(|(x, y)| 2.0 * y - x).collect())
                .collect();
            let u_r2 = twice
                .iter()
                .zip(&multiply(&gw, &transpose(&g)))
                .map(|(a, b)| a.iter().zip(b).map(|(x, y)| x + y).collect())
                .collect();
            let k = multiply(&omega_inv, &ww);
            let tr_k: f64 = (0..m).map(|i| k[i][i]).sum();
            let tr_kk: f64 = (0..m)
                .map(|i| (0..m).map(|j| k[i][j] * k[j][i]).sum::<f64>())
                .sum();
            (u_r2, self.n - 2.0 * tr_k + tr_kk)
        });
        Some(Forms {
            ur,
            uv,
            log_omega,
            log_m,
            r2,
        })
    }

    fn minque(&self, prior: f64) -> Option<Vec<f64>> {
        let k = self.k();
        let f = self.forms(&vec![prior; k], true)?;
        let (u_r2, tr_r2) = f.r2.as_ref()?;
        let blocks = self.u_blocks();
        let y = self.m() - self.r;
        let mut s = vec![vec![0.0; k + 1]; k + 1];
        let mut q = vec![0.0; k + 1];
        for i in 0..k {
            for j in 0..k {
                s[i][j] = ssq(&f.ur, &blocks[i], &blocks[j]);
            }
            s[i][k] = trace(u_r2, &blocks[i]);
            s[k][i] = s[i][k];
            q[i] = ssq(&f.ur, &blocks[i], &(y..y + 1));
        }
        s[k][k] = *tr_r2;
        q[k] = u_r2[y][y];
        let (inv, _) = ginverse(&s, 1e-10);
        Some(
            inv.iter()
                .map(|row| row.iter().zip(&q).map(|(a, b)| a * b).sum())
                .collect(),
        )
    }
}

/// The log likelihood at theta with its gradient, Hessian and expected Hessian.
struct Eval {
    l: f64,
    grad: Vec<f64>,
    hess: Matrix,
    fisher: Matrix,
}

fn evaluate(model: &Model, th: &[f64], reml: bool) -> Option<Eval> {
    let k = model.k();
    let s2 = th[k];
    if s2.is_nan() || s2 <= 0.0 {
        return None;
    }
    let f = model.forms(&th[..k], false)?;
    let y = model.m() - model.r;
    let yry = f.ur[y][y];
    let tau = 2.0 * std::f64::consts::PI;
    let (dfs, l) = if reml {
        let dfs = model.n - model.r as f64;
        let l = -0.5 * (dfs * tau.ln() + dfs * s2.ln() + f.log_omega - model.log_xx + yry / s2);
        (dfs, l)
    } else {
        let n = model.n;
        (n, -0.5 * (n * tau.ln() + n * s2.ln() + f.log_m + yry / s2))
    };
    let a = if reml { &f.ur } else { &f.uv };
    let blocks = model.u_blocks();
    let zry: Vec<Vec<f64>> = blocks
        .iter()
        .map(|b| b.clone().map(|u| f.ur[u][y]).collect())
        .collect();
    let norm: Vec<f64> = zry.iter().map(|v| v.iter().map(|x| x * x).sum()).collect();
    let mut grad = vec![0.0; k + 1];
    let mut hess = vec![vec![0.0; k + 1]; k + 1];
    let mut fisher = vec![vec![0.0; k + 1]; k + 1];
    for i in 0..k {
        let tr = trace(a, &blocks[i]);
        grad[i] = 0.5 * norm[i] / s2 - 0.5 * tr;
        for j in 0..=i {
            let quad: f64 = blocks[i]
                .clone()
                .enumerate()
                .map(|(p, u)| {
                    zry[i][p]
                        * blocks[j]
                            .clone()
                            .enumerate()
                            .map(|(q, v)| f.ur[u][v] * zry[j][q])
                            .sum::<f64>()
                })
                .sum();
            let sq = ssq(a, &blocks[i], &blocks[j]);
            hess[i][j] = -quad / s2 + 0.5 * sq;
            fisher[i][j] = -0.5 * sq;
            hess[j][i] = hess[i][j];
            fisher[j][i] = fisher[i][j];
        }
        hess[i][k] = -norm[i] / (2.0 * s2 * s2);
        hess[k][i] = hess[i][k];
        fisher[i][k] = -tr / (2.0 * s2);
        fisher[k][i] = fisher[i][k];
    }
    grad[k] = yry / (2.0 * s2 * s2) - dfs / (2.0 * s2);
    hess[k][k] = dfs / (2.0 * s2 * s2) - yry / (s2 * s2 * s2);
    fisher[k][k] = -dfs / (2.0 * s2 * s2);
    Some(Eval {
        l,
        grad,
        hess,
        fisher,
    })
}

/// The variances from theta.
fn variances(th: &[f64]) -> Vec<f64> {
    let k = th.len() - 1;
    let mut out: Vec<f64> = th[..k].iter().map(|g| g * th[k]).collect();
    out.push(th[k]);
    out
}

/// The solution of the rows and columns `free` of a x = b, other entries 0; None when that
/// part of `a` is not positive definite.
fn solve_free(a: &Matrix, b: &[f64], free: &[usize]) -> Option<Vec<f64>> {
    let sub: Matrix = free
        .iter()
        .map(|&i| free.iter().map(|&j| a[i][j]).collect())
        .collect();
    let (inv, _) = cholesky_inverse(&sub)?;
    let mut out = vec![0.0; b.len()];
    for (p, &i) in free.iter().enumerate() {
        out[i] = free
            .iter()
            .enumerate()
            .map(|(q, &j)| inv[p][q] * b[j])
            .sum();
    }
    Some(out)
}

fn negated(m: &Matrix) -> Matrix {
    m.iter().map(|r| r.iter().map(|v| -v).collect()).collect()
}

struct Fitted {
    th: Vec<f64>,
    /// Iteration, log likelihood and variances, the start first.
    history: Vec<(usize, f64, Vec<f64>)>,
    /// The covariance of the variances.
    cov: Option<Matrix>,
}

fn maximize(
    model: &Model,
    start: Vec<f64>,
    reml: bool,
    c: &Varcomp,
    warnings: &mut Vec<String>,
) -> Result<Fitted, Error> {
    let k = model.k();
    let mut th = start;
    let not_pd = || Error("VARCOMP: the likelihood cannot be evaluated at the start values".into());
    let mut e = evaluate(model, &th, reml).ok_or_else(not_pd)?;
    let mut history = vec![(0, e.l, variances(&th))];
    let mut increase = f64::INFINITY;
    let mut converged = false;
    for iteration in 1..=c.iterate {
        // a ratio held at 0 by a gradient pointing below 0 stays out of the step
        let free: Vec<usize> = (0..=k)
            .filter(|&i| i == k || th[i] > 0.0 || e.grad[i] > 0.0)
            .collect();
        let newton = iteration > 1 && increase <= 1.0;
        let step = newton
            .then(|| solve_free(&negated(&e.hess), &e.grad, &free))
            .flatten()
            .or_else(|| solve_free(&negated(&e.fisher), &e.grad, &free));
        let Some(step) = step else {
            warnings.push(format!(
                "VARCOMP stopped at iteration {iteration}: the information matrix is singular. The estimates may not be final."
            ));
            break;
        };
        let mut rho = 1.0;
        let mut accepted = None;
        for _ in 0..=10 {
            let candidate: Vec<f64> = th
                .iter()
                .zip(&step)
                .enumerate()
                .map(|(i, (t, d))| {
                    let v = t + rho * d;
                    if i < k { v.max(0.0) } else { v }
                })
                .collect();
            if let Some(next) = evaluate(model, &candidate, reml)
                // near the top the log likelihood is flat to its rounding error, so a step that
                // keeps it within that error still counts
                && next.l > e.l - 1e-9 * e.l.abs().max(1.0)
            {
                accepted = Some((candidate, next));
                break;
            }
            rho /= 2.0;
        }
        // no step raises the likelihood: it is at its top within the precision of the numbers
        let Some((next_th, next)) = accepted else {
            converged = true;
            break;
        };
        let change_l = next.l - e.l;
        let change_th: f64 = th.iter().zip(&next_th).map(|(a, b)| (a - b).abs()).sum();
        let size_th: f64 = th.iter().map(|v| v.abs()).sum();
        let done = change_l.abs() < c.converge * e.l.abs().max(1.0)
            && change_th < c.converge * size_th.max(1.0);
        increase = change_l;
        th = next_th;
        e = next;
        history.push((iteration, e.l, variances(&th)));
        if done {
            converged = true;
            break;
        }
    }
    if !converged {
        warnings.push(format!(
            "VARCOMP reached ITERATE ({}) iterations without converging. The estimates may not be final.",
            c.iterate
        ));
    }
    let cov = inverse(&negated(&e.fisher)).map(|(inv, _)| {
        let s2 = th[k];
        let mut j = vec![vec![0.0; k + 1]; k + 1];
        for i in 0..k {
            j[i][i] = s2;
            j[i][k] = th[i];
        }
        j[k][k] = 1.0;
        multiply(&multiply(&j, &inv), &transpose(&j))
    });
    if cov.is_none() {
        warnings.push(
            "VARCOMP: the information matrix is singular; the asymptotic covariance matrix is left out"
                .into(),
        );
    }
    Ok(Fitted { th, history, cov })
}

/// The start values of ML and REML, as ratios with the residual variance last.
fn start_values(model: &Model, fit: &Fit, eps: f64) -> Vec<f64> {
    let y = model.m();
    let mut s2 = fit.s[0][0] / model.n;
    if s2.is_nan() || s2 <= 0.0 {
        s2 = 1e-8;
    }
    let mut th: Vec<f64> = model
        .blocks
        .iter()
        .map(|b| {
            let m = b.len();
            if m < 2 {
                return 0.0;
            }
            let zz: Matrix = b.clone().map(|i| model.t[i][b.clone()].to_vec()).collect();
            let (g, _) = ginverse(&zz, eps);
            let est: Vec<f64> = (0..m)
                .map(|i| (0..m).map(|j| g[i][j] * model.t[b.start + j][y]).sum())
                .collect();
            let mean = est.iter().sum::<f64>() / m as f64;
            let var = est.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (m - 1) as f64;
            var / s2
        })
        .collect();
    th.push(s2);
    th
}

/// One source of the ANOVA method.
struct Source {
    label: String,
    ss: f64,
    df: usize,
    /// The coefficient of each random variance and of the error variance.
    ems: Vec<f64>,
    /// The fixed effects in the quadratic term.
    quadratic: Vec<String>,
}

fn anova_sources(fit: &Fit, random: &[bool], sstype: u8) -> Vec<Source> {
    let h = fit.h();
    let k = random.iter().filter(|&&r| r).count();
    let mut out = Vec::new();
    for (e, effect) in fit.effects.iter().enumerate() {
        let l = if sstype == 1 {
            fit.type1(e)
        } else {
            fit.type34(e, false)
        };
        let (hyp, df) = fit.hypothesis(&l, None, None);
        let mut source = Source {
            label: effect.label.clone(),
            ss: hyp[0][0],
            df,
            ems: vec![0.0; k + 1],
            quadratic: Vec::new(),
        };
        if df > 0 {
            let lh = multiply(&l, &h);
            let scale = lh.iter().flatten().fold(0.0_f64, |m, v| m.max(v.abs()));
            let lgl = multiply(&multiply(&l, &fit.ginv), &transpose(&l));
            let (inv, _) = ginverse(&lgl, 1e-10);
            let mut i = 0;
            for (j, other) in fit.effects.iter().enumerate() {
                let li: Matrix = lh.iter().map(|row| row[other.range()].to_vec()).collect();
                if random[j] {
                    let c = multiply(&multiply(&transpose(&li), &inv), &li);
                    source.ems[i] = (0..c.len()).map(|a| c[a][a]).sum::<f64>() / df as f64;
                    i += 1;
                } else if li.iter().flatten().any(|v| v.abs() > 1e-8 * scale) {
                    source.quadratic.push(other.label.clone());
                }
            }
            source.ems[k] = 1.0;
        }
        out.push(source);
    }
    out
}

fn method_text(m: VarcompMethod) -> String {
    match m {
        VarcompMethod::Minque(0) => {
            "Minimum norm quadratic unbiased estimation (weight 0 for the random effects, 1 for the residual)".into()
        }
        VarcompMethod::Minque(_) => {
            "Minimum norm quadratic unbiased estimation (weight 1 for the random effects and the residual)".into()
        }
        VarcompMethod::Ml => "Maximum likelihood".into(),
        VarcompMethod::Reml => "Restricted maximum likelihood".into(),
        VarcompMethod::Sstype(t) => format!("ANOVA (Type {} sums of squares)", roman(t)),
    }
}

fn roman(t: u8) -> &'static str {
    if t == 1 { "I" } else { "III" }
}

impl Engine {
    pub(crate) fn varcomp(&mut self, c: &Varcomp) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let factor_names = self.expand(&c.factors)?;
        let covariate_names = self.expand(&c.covariates)?;
        let dependent = data.variable(&c.dependent)?;
        let mut seen: Vec<&str> = vec![dependent.name()];
        for n in factor_names.iter().chain(&covariate_names) {
            if seen.iter().any(|s| s.eq_ignore_ascii_case(n)) {
                return Err(Error(format!(
                    "{n} appears more than once among the dependent variable, factors and covariates"
                )));
            }
            seen.push(n);
        }
        let vars = |names: &[String]| -> Result<Vec<Variable>, Error> {
            names.iter().map(|n| Ok(data.variable(n)?)).collect()
        };
        let factor_vars = vars(&factor_names)?;
        let cov_vars = vars(&covariate_names)?;
        for v in std::iter::once(&dependent).chain(&cov_vars) {
            if v.is_string() {
                return Err(Error(format!(
                    "VARCOMP needs a numeric dependent variable and numeric covariates, but {} is a string variable",
                    v.name()
                )));
            }
        }
        let regwgt = match &c.regwgt {
            Some(n) => {
                let v = data.variable(n)?;
                if v.is_string() {
                    return Err(Error(format!(
                        "REGWGT needs a numeric variable, but {n} is a string variable"
                    )));
                }
                Some(v)
            }
            None => None,
        };
        let mut random_factors = Vec::new();
        for name in &c.random {
            let Some(f) = factor_names
                .iter()
                .position(|n| n.eq_ignore_ascii_case(name))
            else {
                return Err(Error(format!(
                    "VARCOMP: {name} on RANDOM is not a factor after BY"
                )));
            };
            random_factors.push(f);
        }
        let (d, _) = model_data(
            data,
            &shown,
            std::slice::from_ref(&dependent),
            &factor_vars,
            &cov_vars,
            regwgt.as_ref(),
            c.include,
        )?;
        if d.rows() == 0 {
            return Err(Error(
                "VARCOMP: no case has valid values on every variable".into(),
            ));
        }
        let terms = match &c.design {
            Some(terms) => {
                let mut terms = terms.clone();
                if c.intercept && !terms.iter().any(|t| t.variables.is_empty()) {
                    terms.insert(0, intercept_term());
                }
                terms
            }
            None => default_terms(&factor_names, &covariate_names, c.intercept),
        };
        let mut effects = effects_of(&terms, &factor_names, &covariate_names)?;
        let covariate_texts: Vec<String> = cov_vars.iter().map(|v| shown.variable(v)).collect();
        for e in effects
            .iter_mut()
            .filter(|e| !e.is_intercept() && !e.label.contains('('))
        {
            let parts = e.parts.iter().map(|p| match p {
                Part::Factor(f) => d.factor_texts[*f].clone(),
                Part::Covariate(v) => covariate_texts[*v].clone(),
            });
            e.label = parts.collect::<Vec<_>>().join(" * ");
        }
        let fit = Fit::new(&d, effects, &d.y, c.eps);
        let random: Vec<bool> = fit
            .effects
            .iter()
            .map(|e| e.factors.iter().any(|f| random_factors.contains(f)))
            .collect();
        if !random.contains(&true) {
            return Err(Error(
                "VARCOMP: no effect of the design holds a factor named on RANDOM".into(),
            ));
        }
        let mut labels: Vec<String> = fit
            .effects
            .iter()
            .zip(&random)
            .filter(|(_, r)| **r)
            .map(|(e, _)| format!("Var({})", e.label))
            .collect();
        labels.push("Var(Error)".into());
        let dfe = fit.n - fit.rank as f64;
        if dfe <= 0.0 {
            return Err(Error(
                "VARCOMP: the design leaves no degrees of freedom for the error".into(),
            ));
        }
        let mut warnings = Vec::new();
        let ml_like = matches!(c.method, VarcompMethod::Ml | VarcompMethod::Reml);
        let sstype = match c.method {
            VarcompMethod::Sstype(t) => Some(t),
            _ => None,
        };
        if sstype.is_none() && (c.print.ems || c.print.ss) {
            warnings.push("VARCOMP: PRINT EMS and SS apply to METHOD=SSTYPE only".into());
        }
        if !ml_like && c.print.history.is_some() {
            warnings.push("VARCOMP: PRINT HISTORY applies to METHOD=ML and REML only".into());
        }
        let dependent_text = shown.variable(&dependent);
        let mut factor_table = between_factors(&d);
        factor_table.title = "Factor Level Information".into();
        let mut tables = vec![factor_table];
        let mut cov = None;
        let estimates = if let Some(t) = sstype {
            let sources = anova_sources(&fit, &random, t);
            if c.print.ss {
                tables.push(ss_table(&sources, &d, &fit, t));
            }
            if c.print.ems {
                tables.push(ems_table(&sources, &labels, t));
            }
            anova_estimates(&sources, &random, fit.s[0][0] / dfe)?
        } else {
            let model = model_of(&fit, &random)?;
            if let VarcompMethod::Minque(prior) = c.method {
                model
                    .minque(f64::from(prior))
                    .ok_or_else(|| Error("VARCOMP: the MINQUE equations cannot be formed".into()))?
            } else {
                let reml = c.method == VarcompMethod::Reml;
                let start = start_values(&model, &fit, c.eps);
                let fitted = maximize(&model, start, reml, c, &mut warnings)?;
                if let Some(every) = c.print.history {
                    tables.push(history_table(&fitted.history, &labels, every));
                }
                cov = fitted.cov;
                variances(&fitted.th)
            }
        };
        let rows = labels
            .iter()
            .zip(&estimates)
            .map(|(l, v)| vec![text(l.clone()), Cell::Number(*v)])
            .collect();
        tables.push(table(
            "Variance Estimates",
            1,
            plain(&["Component", "Estimate"]),
            rows,
            Some(format!(
                "Dependent Variable: {dependent_text}. Method: {}.",
                method_text(c.method)
            )),
        ));
        if let Some(v) = &cov {
            tables.push(matrix_table(
                "Asymptotic Covariance Matrix",
                &labels,
                &|i, j| Cell::Number(v[i][j]),
                format!(
                    "Dependent Variable: {dependent_text}. Method: {}.",
                    method_text(c.method)
                ),
            ));
        }
        if let Some(out) = &c.outfile {
            if out.matrix.is_some() && !ml_like {
                warnings
                    .push("VARCOMP: OUTFILE COVB and CORB apply to METHOD=ML and REML only".into());
            }
            let matrix = out.matrix.filter(|_| ml_like).and_then(|corr| {
                cov.as_ref().map(|v| {
                    let k = v.len();
                    (0..k)
                        .map(|i| {
                            (0..k)
                                .map(|j| {
                                    if corr {
                                        v[i][j] / (v[i][i] * v[j][j]).sqrt()
                                    } else {
                                        v[i][j]
                                    }
                                })
                                .collect()
                        })
                        .collect::<Matrix>()
                })
            });
            if out.varest || matrix.is_some() {
                let file = out_data(&labels, out.varest.then_some(&estimates[..]), matrix)?;
                self.write_data_file(&out.file, file)?;
            }
        }
        for w in warnings {
            self.warn(w);
        }
        Ok(tables)
    }
}

/// The model for MINQUE, ML and REML: X0 without its redundant columns, every Z_i whole.
fn model_of(fit: &Fit, random: &[bool]) -> Result<Model, Error> {
    let fixed: Vec<usize> = fit
        .effects
        .iter()
        .zip(random)
        .filter(|(_, r)| !**r)
        .flat_map(|(e, _)| e.range())
        .collect();
    let (_, swept) = sweep_in_order(&fit.zwz, &fixed, fit.eps);
    let mut idx: Vec<usize> = fixed.into_iter().filter(|&j| swept[j]).collect();
    let r = idx.len();
    let mut blocks = Vec::new();
    for (e, _) in fit.effects.iter().zip(random).filter(|(_, r)| **r) {
        let start = idx.len();
        idx.extend(e.range());
        blocks.push(start..idx.len());
    }
    idx.push(fit.p);
    let t: Matrix = idx
        .iter()
        .map(|&i| idx.iter().map(|&j| fit.zwz[i][j]).collect())
        .collect();
    let xx: Matrix = t[..r].iter().map(|row| row[..r].to_vec()).collect();
    let log_xx = if r == 0 {
        0.0
    } else {
        cholesky_inverse(&xx)
            .ok_or_else(|| Error("VARCOMP: the fixed design is singular".into()))?
            .1
    };
    Ok(Model {
        t,
        r,
        blocks,
        n: fit.n,
        log_xx,
    })
}

fn anova_estimates(sources: &[Source], random: &[bool], mse: f64) -> Result<Vec<f64>, Error> {
    let k = random.iter().filter(|&&r| r).count();
    let mut e = Vec::new();
    let mut ms = Vec::new();
    for (s, _) in sources.iter().zip(random).filter(|(_, r)| **r) {
        if s.df == 0 {
            return Err(Error(format!(
                "VARCOMP: the random effect {} has no degrees of freedom of its own with these sums of squares",
                s.label
            )));
        }
        e.push(s.ems.clone());
        ms.push(s.ss / s.df as f64);
    }
    let mut error_row = vec![0.0; k + 1];
    error_row[k] = 1.0;
    e.push(error_row);
    ms.push(mse);
    let (inv, _) = inverse(&e).ok_or_else(|| {
        Error(
            "VARCOMP: the equations of the random rows and the error row have no unique solution"
                .into(),
        )
    })?;
    Ok(inv
        .iter()
        .map(|row| row.iter().zip(&ms).map(|(a, b)| a * b).sum())
        .collect())
}

fn ss_table(sources: &[Source], d: &crate::glm::Data, fit: &Fit, sstype: u8) -> Table {
    let w: Vec<f64> = (0..d.rows()).map(|i| d.f[i] * d.g[i]).collect();
    let sw: f64 = w.iter().sum();
    let mean = (0..d.rows()).map(|i| w[i] * d.y[i][0]).sum::<f64>() / sw;
    let total = fit.zwz[fit.p][fit.p];
    let corrected: f64 = (0..d.rows())
        .map(|i| w[i] * (d.y[i][0] - mean).powi(2))
        .sum();
    let ms = |ss: f64, df: f64| {
        if df > 0.0 {
            Cell::Number(ss / df)
        } else {
            Cell::Empty
        }
    };
    let mut rows: Vec<Vec<Cell>> = sources
        .iter()
        .map(|s| {
            vec![
                text(s.label.clone()),
                Cell::Number(s.ss),
                Cell::Number(s.df as f64),
                ms(s.ss, s.df as f64),
            ]
        })
        .collect();
    let sse = fit.s[0][0];
    let dfe = fit.n - fit.rank as f64;
    rows.push(vec![
        text("Error"),
        Cell::Number(sse),
        Cell::Number(dfe),
        ms(sse, dfe),
    ]);
    rows.push(vec![
        text("Total"),
        Cell::Number(total),
        Cell::Number(fit.n),
        Cell::Empty,
    ]);
    if fit.effects.iter().any(|e| e.is_intercept()) {
        rows.push(vec![
            text("Corrected Total"),
            Cell::Number(corrected),
            Cell::Number(fit.n - 1.0),
            Cell::Empty,
        ]);
    }
    let ss_title = format!("Type {} Sum of Squares", roman(sstype));
    table(
        "ANOVA",
        1,
        plain(&["Source", &ss_title, "df", "Mean Square"]),
        rows,
        None,
    )
}

fn ems_table(sources: &[Source], labels: &[String], sstype: u8) -> Table {
    let mut columns = plain(&["Source"]);
    columns.extend(
        labels
            .iter()
            .map(|l| ("Variance Component".to_string(), l.clone())),
    );
    columns.extend(plain(&["Quadratic Term"]));
    let k = labels.len() - 1;
    let mut rows: Vec<Vec<Cell>> = sources
        .iter()
        .map(|s| {
            let mut row = vec![text(s.label.clone())];
            if s.df == 0 {
                row.extend((0..=k).map(|_| Cell::Empty));
            } else {
                row.extend(s.ems.iter().map(|v| Cell::Number(*v)));
            }
            row.push(text(s.quadratic.join(", ")));
            row
        })
        .collect();
    let mut error = vec![text("Error")];
    error.extend((0..=k).map(|i| Cell::Number(if i == k { 1.0 } else { 0.0 })));
    error.push(text(""));
    rows.push(error);
    table(
        "Expected Mean Squares",
        1,
        columns,
        rows,
        Some(format!(
            "From the Type {} sums of squares. Each expected mean square adds up the variances times their coefficients in its row, plus a quadratic form in the fixed effects named under Quadratic Term.",
            roman(sstype)
        )),
    )
}

fn history_table(history: &[(usize, f64, Vec<f64>)], labels: &[String], every: usize) -> Table {
    let mut columns = plain(&["Iteration", "Log Likelihood"]);
    columns.extend(labels.iter().map(|l| (String::new(), l.clone())));
    let last = history.len() - 1;
    let rows = history
        .iter()
        .enumerate()
        .filter(|(p, (it, _, _))| it % every == 0 || *p == last)
        .map(|(_, (it, l, v))| {
            let mut row = vec![text(it.to_string()), Cell::Number(*l)];
            row.extend(v.iter().map(|x| Cell::Number(*x)));
            row
        })
        .collect();
    table("Iteration History", 1, columns, rows, None)
}

/// The OUTFILE data: one row per component with its label, estimate and matrix row.
fn out_data(
    labels: &[String],
    estimates: Option<&[f64]>,
    matrix: Option<Matrix>,
) -> Result<Dataset, Error> {
    let mut out = Dataset::new(labels.len());
    let width = labels.iter().map(String::len).max().unwrap_or(8).max(8);
    out.set_string_values("VarName_", width, labels.to_vec())?;
    if let Some(est) = estimates {
        out.set_numeric_values("Estimate", est.iter().map(|v| Some(*v)).collect())?;
    }
    if let Some(m) = matrix {
        for (j, label) in labels.iter().enumerate() {
            let name = format!("VC{}", j + 1);
            out.set_numeric_values(&name, m.iter().map(|row| Some(row[j])).collect())?;
            out.set_variable_label(&name, Some(label))?;
        }
    }
    Ok(out)
}
