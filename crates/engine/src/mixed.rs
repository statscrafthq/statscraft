//! MIXED: the linear mixed model y = X b + Z u + e, with V = Z G Z' + R.
//!
//! Rules (our notes from the Command Syntax Reference and the Algorithms manual, MIXED):
//! - A case is used when the dependent, every factor, covariate, subject and repeated variable
//!   is valid, its frequency weight rounds to at least 1 (it then counts that many times) and
//!   its regression weight is positive. The regression weight w scales the residual covariance:
//!   R becomes W^-1/2 R W^-1/2.
//! - The fixed design is that of UNIANOVA: one column per level combination that occurs,
//!   redundant columns found by sweeping (SINGULAR) and fixed at 0.
//! - Each RANDOM subcommand is one covariance block G_k over the columns of its effects, the
//!   same for every subject of its SUBJECT variables and independent between subjects and
//!   between subcommands. VC is a scaled identity per effect. REPEATED gives every subject of
//!   its SUBJECT variables a residual block over the sorted levels of its variables; without it
//!   R is one variance times the identity.
//! - -2 log likelihood: ML log|V| + r'V^-1 r + n log 2 pi; REML adds log|X'V^-1 X| and has
//!   (n - p) log 2 pi, p the rank of X. The covariance parameters minimize it by Newton steps
//!   (the expected Hessian for the first SCORING iterations), halving a step that leaves the
//!   parameter space or does not improve, up to MXSTEP times.
//! - Starting values: the residual variance s^2 of the fixed model fitted by least squares;
//!   every variance starts at s^2 / (K + 1) for K RANDOM subcommands, covariances and
//!   correlations at 0.
//! - b = (X'V^-1 X)^-1 X'V^-1 y with covariance C = (X'V^-1 X)^-1. The covariance parameters
//!   have the asymptotic covariance 2 H^-1, H the observed Hessian of -2 log likelihood.
//! - Satterthwaite: for one row l, df = 2 (l C l')^2 / (g' 2H^-1 g) with g the gradient of
//!   l C l' in the covariance parameters. For several rows, the eigenvalues d_m of L C L' give
//!   one df v_m per direction, E = sum v_m / (v_m - 2) over v_m > 2 and df = 2E / (E - q).
//!   DFMETHOD(RESIDUAL): n - rank X.
//! - Information criteria from -2LL: AIC + 2d, AICC + 2dn / (n - d - 1), BIC + d ln n,
//!   CAIC + d (ln n + 1); REML: n less the fixed parameters and d the covariance parameters;
//!   ML: n all cases and d both kinds.
//! - Wald intervals: variances on the log scale, correlations on the Fisher z scale, other
//!   parameters as they are.
//! - EMMEANS and TEST are combinations l b of the fixed effects, built as in UNIANOVA (a mean
//!   averages the cells of the other factors equally; covariates sit at their means over the
//!   cases, or at the WITH values). Each has the standard error sqrt(l C l') and its own
//!   Satterthwaite df; a row that is not estimable is left out. Pairwise comparisons adjust the
//!   significance and the interval for m comparisons (every pair, or each level against REFCAT):
//!   Bonferroni p m, Sidak 1 - (1 - p)^m. The F tests of several rows use the df rule above.
//! - PRINT G shows the covariance of one subject's random effects, R that of one subject's
//!   repeated levels (without regression weights); each is left out with a warning when there is
//!   no RANDOM or REPEATED. COVB is C, CORB its correlations; a redundant parameter has 0 and no
//!   correlation.

use std::collections::HashMap;

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::dist::{f_p, normal_p, normal_quantile, t_p, t_quantile};
use crate::glm::{
    Data, Effect, Fit, Part, adjusted, adjusted_t, adjustment_name, combinations, covariate_note,
    covariate_values, effects_of, emmeans_l, intercept_term, key_value_text, l_row, parameter_name,
    percent_title,
};
use crate::matrix::{Matrix, symmetric_eigen};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::shown::{Show, Shown};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Measure, Values, Variable};
use statscraft_syntax::{CovType, CustomTest, Emmeans, Mixed, MixedTemp, RefCat, Term, Value};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Variance,
    Correlation,
    General,
}

/// One covariance structure over `dim` levels, its parameters at `start..start + count` of θ.
struct Block {
    covtype: CovType,
    dim: usize,
    /// VC, ID and DIAG: the parameter of each level.
    group: Vec<usize>,
    start: usize,
    count: usize,
}

#[derive(Clone, Copy)]
enum What {
    Value,
    D1(usize),
    D2(usize, usize),
}

impl Block {
    /// `effect_of` gives the effect of each column, for VC.
    fn new(covtype: CovType, effect_of: Vec<usize>, start: usize) -> Block {
        let dim = effect_of.len();
        let group = match covtype {
            CovType::Vc => effect_of,
            CovType::Id => vec![0; dim],
            _ => (0..dim).collect(),
        };
        let count = match covtype {
            CovType::Vc | CovType::Id | CovType::Diag => group.iter().max().map_or(0, |m| m + 1),
            CovType::Un => dim * (dim + 1) / 2,
            CovType::Cs | CovType::Ar1 => 2,
        };
        Block {
            covtype,
            dim,
            group,
            start,
            count,
        }
    }

    fn touches(&self, what: What) -> bool {
        let mine = |s: usize| (self.start..self.start + self.count).contains(&s);
        match what {
            What::Value => true,
            What::D1(s) => mine(s),
            What::D2(s, t) => mine(s) && mine(t) && self.covtype == CovType::Ar1,
        }
    }

    /// Entry (i, j) of the matrix or of a derivative, `th` the whole parameter vector.
    fn value(&self, th: &[f64], i: usize, j: usize, what: What) -> f64 {
        let th = &th[self.start..self.start + self.count];
        let local = |s: usize| s - self.start;
        let one = |b: bool| f64::from(u8::from(b));
        match self.covtype {
            CovType::Vc | CovType::Id | CovType::Diag => match what {
                What::Value => one(i == j) * th[self.group[i]],
                What::D1(s) => one(i == j && self.group[i] == local(s)),
                What::D2(..) => 0.0,
            },
            CovType::Un => {
                let (a, b) = if i >= j { (i, j) } else { (j, i) };
                let k = a * (a + 1) / 2 + b;
                match what {
                    What::Value => th[k],
                    What::D1(s) => one(k == local(s)),
                    What::D2(..) => 0.0,
                }
            }
            CovType::Cs => match what {
                What::Value => th[0] * one(i == j) + th[1],
                What::D1(s) => {
                    if local(s) == 0 {
                        one(i == j)
                    } else {
                        1.0
                    }
                }
                What::D2(..) => 0.0,
            },
            CovType::Ar1 => {
                let d = i.abs_diff(j) as i32;
                let (s2, rho) = (th[0], th[1]);
                // rho^k for k >= 0, 0 for a negative power (its coefficient is 0 there)
                let pw = |k: i32| if k < 0 { 0.0 } else { rho.powi(k) };
                let df = f64::from(d);
                match what {
                    What::Value => s2 * pw(d),
                    What::D1(s) if local(s) == 0 => pw(d),
                    What::D1(_) => s2 * df * pw(d - 1),
                    What::D2(s, t) => match (local(s), local(t)) {
                        (0, 0) => 0.0,
                        (1, 1) => s2 * df * (df - 1.0) * pw(d - 2),
                        _ => df * pw(d - 1),
                    },
                }
            }
        }
    }

    fn kinds(&self) -> Vec<Kind> {
        match self.covtype {
            CovType::Vc | CovType::Id | CovType::Diag => vec![Kind::Variance; self.count],
            CovType::Un => (0..self.dim)
                .flat_map(|a| {
                    (0..=a).map(move |b| {
                        if a == b {
                            Kind::Variance
                        } else {
                            Kind::General
                        }
                    })
                })
                .collect(),
            CovType::Cs => vec![Kind::Variance, Kind::General],
            CovType::Ar1 => vec![Kind::Variance, Kind::Correlation],
        }
    }

    fn names(&self) -> Vec<String> {
        match self.covtype {
            CovType::Vc | CovType::Id => vec!["Variance".into(); self.count],
            CovType::Diag => (1..=self.count).map(|i| format!("Var({i})")).collect(),
            CovType::Un => (1..=self.dim)
                .flat_map(|a| (1..=a).map(move |b| format!("UN ({a},{b})")))
                .collect(),
            CovType::Cs => vec!["CS diagonal offset".into(), "CS covariance".into()],
            CovType::Ar1 => vec!["AR1 diagonal".into(), "AR1 rho".into()],
        }
    }

    fn start_values(&self, variance: f64) -> Vec<f64> {
        self.kinds()
            .iter()
            .map(|k| if *k == Kind::Variance { variance } else { 0.0 })
            .collect()
    }
}

fn structure_name(t: CovType) -> &'static str {
    match t {
        CovType::Vc => "Variance Components",
        CovType::Id => "Identity",
        CovType::Diag => "Diagonal",
        CovType::Un => "Unstructured",
        CovType::Cs => "Compound Symmetry",
        CovType::Ar1 => "First-Order Autoregressive",
    }
}

/// The model in the form the likelihood needs.
struct Model {
    y: Vec<f64>,
    /// The fixed design without its redundant columns.
    x: Vec<Vec<f64>>,
    p: usize,
    ml: bool,
    /// The residual block first, then one per RANDOM subcommand.
    blocks: Vec<Block>,
    /// For each RANDOM: the nonzero columns of every case and its subject.
    nz: Vec<Vec<Vec<(usize, f64)>>>,
    subject: Vec<Vec<usize>>,
    /// REPEATED: the subject and the level of every case.
    repeated: Option<(Vec<usize>, Vec<usize>)>,
    /// Regression weights.
    g: Vec<f64>,
    /// Sets of cases that no covariance links: V is block diagonal over them.
    groups: Vec<Vec<usize>>,
    np: usize,
}

struct Eval {
    f: f64,
    beta: Vec<f64>,
    a_inv: Matrix,
    grad: Vec<f64>,
    hess: Matrix,
    /// X'V^-1 (dV/dθ_s) V^-1 X for each parameter.
    b: Vec<Matrix>,
    /// V^-1 of every group.
    vi: Vec<Matrix>,
}

#[derive(Clone, Copy, PartialEq)]
enum Depth {
    Objective,
    Hessian { expected: bool },
}

/// The inverse of a symmetric positive definite matrix and its log determinant, by Cholesky;
/// None when it is not positive definite.
fn cholesky_inverse(a: &Matrix) -> Option<(Matrix, f64)> {
    let n = a.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let s = a[i][j] - (0..j).map(|k| l[i][k] * l[j][k]).sum::<f64>();
            if i == j {
                if !(s > 0.0 && s.is_finite()) {
                    return None;
                }
                l[i][i] = s.sqrt();
            } else {
                l[i][j] = s / l[j][j];
            }
        }
    }
    let logdet = 2.0 * (0..n).map(|i| l[i][i].ln()).sum::<f64>();
    let mut li = vec![vec![0.0; n]; n];
    for i in 0..n {
        let row: Vec<f64> = (0..i)
            .map(|j| -(j..i).map(|k| l[i][k] * li[k][j]).sum::<f64>() / l[i][i])
            .collect();
        li[i][..i].copy_from_slice(&row);
        li[i][i] = 1.0 / l[i][i];
    }
    let mut inv = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let v: f64 = (i..n).map(|k| li[k][i] * li[k][j]).sum();
            inv[i][j] = v;
            inv[j][i] = v;
        }
    }
    Some((inv, logdet))
}

fn mat_vec(m: &Matrix, v: &[f64]) -> Vec<f64> {
    m.iter()
        .map(|row| row.iter().zip(v).map(|(a, b)| a * b).sum())
        .collect()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// a' b for two n x p matrices.
fn cross(a: &Matrix, b: &Matrix) -> Matrix {
    let p = a.first().map_or(0, Vec::len);
    let q = b.first().map_or(0, Vec::len);
    let mut out = vec![vec![0.0; q]; p];
    for (ra, rb) in a.iter().zip(b) {
        for (i, x) in ra.iter().enumerate() {
            if *x != 0.0 {
                for (j, y) in rb.iter().enumerate() {
                    out[i][j] += x * y;
                }
            }
        }
    }
    out
}

fn product(a: &Matrix, b: &Matrix) -> Matrix {
    let q = b.first().map_or(0, Vec::len);
    a.iter()
        .map(|row| {
            let mut out = vec![0.0; q];
            for (k, x) in row.iter().enumerate() {
                if *x != 0.0 {
                    for (o, y) in out.iter_mut().zip(&b[k]) {
                        *o += x * y;
                    }
                }
            }
            out
        })
        .collect()
}

/// tr(a b) for square a and b.
fn trace_product(a: &Matrix, b: &Matrix) -> f64 {
    (0..a.len())
        .map(|i| (0..a.len()).map(|j| a[i][j] * b[j][i]).sum::<f64>())
        .sum()
}

fn add_to(acc: &mut Matrix, m: &Matrix) {
    for (r, s) in acc.iter_mut().zip(m) {
        for (a, b) in r.iter_mut().zip(s) {
            *a += b;
        }
    }
}

impl Model {
    /// Entry (a, c) of R, or of its derivative.
    fn residual_entry(&self, th: &[f64], a: usize, c: usize, what: What) -> f64 {
        let residual = &self.blocks[0];
        if !residual.touches(what) {
            return 0.0;
        }
        let at = match &self.repeated {
            Some((subject, level)) => (subject[a] == subject[c]).then(|| (level[a], level[c])),
            None => (a == c).then_some((0, 0)),
        };
        at.map_or(0.0, |(i, j)| {
            residual.value(th, i, j, what) / (self.g[a] * self.g[c]).sqrt()
        })
    }

    /// Entry (a, c) of V = Z G Z' + R, or of its derivative.
    fn entry(&self, th: &[f64], a: usize, c: usize, what: What) -> f64 {
        let mut v = self.residual_entry(th, a, c, what);
        for (k, block) in self.blocks[1..].iter().enumerate() {
            if !block.touches(what) || self.subject[k][a] != self.subject[k][c] {
                continue;
            }
            for &(u, za) in &self.nz[k][a] {
                for &(w, zc) in &self.nz[k][c] {
                    v += za * zc * block.value(th, u, w, what);
                }
            }
        }
        v
    }

    fn matrix(&self, cases: &[usize], th: &[f64], what: What) -> Matrix {
        let n = cases.len();
        let mut m = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in 0..=i {
                let v = self.entry(th, cases[i], cases[j], what);
                m[i][j] = v;
                m[j][i] = v;
            }
        }
        m
    }

    fn valid(&self, th: &[f64]) -> bool {
        self.blocks.iter().all(|b| {
            b.kinds()
                .iter()
                .zip(&th[b.start..b.start + b.count])
                .all(|(k, v)| match k {
                    Kind::Variance => *v >= 0.0,
                    Kind::Correlation => v.abs() < 1.0,
                    Kind::General => v.is_finite(),
                })
        })
    }

    fn block_of(&self, s: usize) -> usize {
        self.blocks
            .iter()
            .position(|b| (b.start..b.start + b.count).contains(&s))
            .expect("a parameter of a block")
    }

    /// -2 log likelihood at θ, with its derivatives as deep as asked; None where V is not
    /// positive definite.
    fn evaluate(&self, th: &[f64], depth: Depth) -> Option<Eval> {
        let p = self.p;
        let n = self.y.len() as f64;
        let mut logdet = 0.0;
        let mut a = vec![vec![0.0; p]; p];
        let mut c = vec![0.0; p];
        let mut yy = 0.0;
        let mut vis = Vec::with_capacity(self.groups.len());
        let mut us = Vec::with_capacity(self.groups.len());
        for cases in &self.groups {
            let v = self.matrix(cases, th, What::Value);
            let (vi, ld) = cholesky_inverse(&v)?;
            logdet += ld;
            let xb: Matrix = cases.iter().map(|&i| self.x[i].clone()).collect();
            let yb: Vec<f64> = cases.iter().map(|&i| self.y[i]).collect();
            let u = product(&vi, &xb);
            let vy = mat_vec(&vi, &yb);
            add_to(&mut a, &cross(&xb, &u));
            for (j, cj) in c.iter_mut().enumerate() {
                *cj += u.iter().zip(&yb).map(|(row, y)| row[j] * y).sum::<f64>();
            }
            yy += dot(&yb, &vy);
            vis.push(vi);
            us.push(u);
        }
        let (a_inv, log_a) = if p > 0 {
            cholesky_inverse(&a)?
        } else {
            (Vec::new(), 0.0)
        };
        let beta = mat_vec(&a_inv, &c);
        let rvr = yy - dot(&c, &beta);
        let two_pi = (2.0 * std::f64::consts::PI).ln();
        let f = if self.ml {
            logdet + rvr + n * two_pi
        } else {
            logdet + rvr + log_a + (n - p as f64) * two_pi
        };
        let k = self.np;
        let mut out = Eval {
            f,
            beta,
            a_inv,
            grad: Vec::new(),
            hess: Vec::new(),
            b: Vec::new(),
            vi: Vec::new(),
        };
        if depth == Depth::Objective {
            out.vi = vis;
            return Some(out);
        }
        let hessian = matches!(depth, Depth::Hessian { .. });
        let mut tr = vec![0.0; k];
        let mut qvq = vec![0.0; k];
        let mut b = vec![vec![vec![0.0; p]; p]; k];
        let mut h = vec![vec![0.0; p]; k];
        let mut t1 = vec![vec![0.0; k]; k];
        let mut t2 = vec![vec![vec![vec![0.0; p]; p]; k]; k];
        let mut avia = vec![vec![0.0; k]; k];
        let mut tr2 = vec![vec![0.0; k]; k];
        let mut qv2q = vec![vec![0.0; k]; k];
        let mut uv2u = vec![vec![vec![vec![0.0; p]; p]; k]; k];
        for (gi, cases) in self.groups.iter().enumerate() {
            let vi = &vis[gi];
            let u = &us[gi];
            let r: Vec<f64> = cases
                .iter()
                .map(|&i| self.y[i] - dot(&self.x[i], &out.beta))
                .collect();
            let q = mat_vec(vi, &r);
            let mut ws = Vec::new();
            let mut ds = Vec::new();
            let mut es = Vec::new();
            let mut avs = Vec::new();
            let mut vias = Vec::new();
            for s in 0..k {
                let dv = self.matrix(cases, th, What::D1(s));
                tr[s] += trace_product(vi, &dv);
                let a_s = mat_vec(&dv, &q);
                qvq[s] += dot(&q, &a_s);
                let d_s = product(&dv, u);
                add_to(&mut b[s], &cross(u, &d_s));
                for (j, hj) in h[s].iter_mut().enumerate() {
                    *hj += u.iter().zip(&a_s).map(|(row, x)| row[j] * x).sum::<f64>();
                }
                if hessian {
                    ws.push(product(vi, &dv));
                    es.push(product(vi, &d_s));
                    vias.push(mat_vec(vi, &a_s));
                    ds.push(d_s);
                    avs.push(a_s);
                }
            }
            if !hessian {
                continue;
            }
            for s in 0..k {
                for t in s..k {
                    t1[s][t] += trace_product(&ws[s], &ws[t]);
                    add_to(&mut t2[s][t], &cross(&ds[s], &es[t]));
                    avia[s][t] += dot(&avs[s], &vias[t]);
                    if self.blocks[self.block_of(s)].touches(What::D2(s, t)) {
                        let dv2 = self.matrix(cases, th, What::D2(s, t));
                        tr2[s][t] += trace_product(vi, &dv2);
                        qv2q[s][t] += dot(&q, &mat_vec(&dv2, &q));
                        add_to(&mut uv2u[s][t], &cross(u, &product(&dv2, u)));
                    }
                }
            }
        }
        let ai = &out.a_inv;
        let tr_ai = |m: &Matrix| trace_product(ai, m);
        out.grad = (0..k)
            .map(|s| tr[s] - qvq[s] - if self.ml { 0.0 } else { tr_ai(&b[s]) })
            .collect();
        if let Depth::Hessian { expected } = depth {
            let mut hm = vec![vec![0.0; k]; k];
            for s in 0..k {
                for t in s..k {
                    let aib_s = product(ai, &b[s]);
                    let aib_t = product(ai, &b[t]);
                    let bb = trace_product(&aib_s, &aib_t);
                    let v = if expected {
                        if self.ml {
                            t1[s][t]
                        } else {
                            t1[s][t] - 2.0 * tr_ai(&t2[s][t]) + bb
                        }
                    } else {
                        let ml = -t1[s][t]
                            + tr2[s][t]
                            + 2.0 * (avia[s][t] - dot(&h[s], &mat_vec(ai, &h[t])))
                            - qv2q[s][t];
                        if self.ml {
                            ml
                        } else {
                            ml - bb + 2.0 * tr_ai(&t2[s][t]) - tr_ai(&uv2u[s][t])
                        }
                    };
                    hm[s][t] = v;
                    hm[t][s] = v;
                }
            }
            out.hess = hm;
        }
        out.b = b;
        out.vi = vis;
        Some(out)
    }

    /// l C l' and its gradient in θ, for l in the reduced fixed coordinates.
    fn variance_and_gradient(&self, e: &Eval, l: &[f64]) -> (f64, Vec<f64>) {
        let cl = mat_vec(&e.a_inv, l);
        let var = dot(l, &cl);
        let grad = e.b.iter().map(|b| dot(&cl, &mat_vec(b, &cl))).collect();
        (var, grad)
    }
}

/// Solves a x = b for symmetric positive definite a; None otherwise.
fn solve_pd(a: &Matrix, b: &[f64]) -> Option<Vec<f64>> {
    cholesky_inverse(a).map(|(inv, _)| mat_vec(&inv, b))
}

/// Indices of the distinct value combinations of `columns` over `rows`, in ascending order.
fn index_by(rows: &[usize], columns: &[&Vec<Option<Key>>]) -> (Vec<usize>, Vec<Vec<usize>>) {
    let key = |k: usize| -> Vec<&Key> {
        columns
            .iter()
            .map(|col| col[k].as_ref().expect("valid"))
            .collect()
    };
    let compare = |a: &Vec<&Key>, b: &Vec<&Key>| {
        a.iter()
            .zip(b)
            .map(|(x, y)| x.compare(y))
            .find(|o| o.is_ne())
            .unwrap_or(std::cmp::Ordering::Equal)
    };
    let mut order: Vec<usize> = (0..rows.len()).collect();
    order.sort_by(|&a, &b| compare(&key(rows[a]), &key(rows[b])));
    let mut index = vec![0; rows.len()];
    let mut first: Vec<Vec<usize>> = Vec::new();
    for (pos, &i) in order.iter().enumerate() {
        if pos > 0 && compare(&key(rows[order[pos - 1]]), &key(rows[i])).is_ne() || pos == 0 {
            first.push(Vec::new());
        }
        index[i] = first.len() - 1;
        first.last_mut().expect("a level").push(i);
    }
    (index, first)
}

fn find(parent: &mut [usize], i: usize) -> usize {
    let mut r = i;
    while parent[r] != r {
        r = parent[r];
    }
    let mut i = i;
    while parent[i] != r {
        let next = parent[i];
        parent[i] = r;
        i = next;
    }
    r
}

/// Satterthwaite degrees of freedom of one direction: 2 (l C l')^2 / (g' Σ g).
fn satterthwaite(var: f64, grad: &[f64], sigma: Option<&Matrix>) -> f64 {
    let Some(sigma) = sigma else {
        return f64::NAN;
    };
    let denominator = dot(grad, &mat_vec(sigma, grad));
    if denominator > 0.0 {
        2.0 * var * var / denominator
    } else {
        f64::NAN
    }
}

/// Inference on combinations of the fixed effects, l in the reduced fixed coordinates.
struct Infer<'a> {
    model: &'a Model,
    e: &'a Eval,
    sigma: Option<&'a Matrix>,
    /// DFMETHOD(RESIDUAL): n - rank X for every test; Satterthwaite otherwise.
    residual_df: Option<f64>,
}

impl Infer<'_> {
    /// l b with its standard error and degrees of freedom.
    fn estimate(&self, l: &[f64]) -> (f64, f64, f64) {
        let (var, grad) = self.model.variance_and_gradient(self.e, l);
        let df = self
            .residual_df
            .unwrap_or_else(|| satterthwaite(var, &grad, self.sigma));
        (dot(l, &self.e.beta), var.sqrt(), df)
    }

    /// The F test of L b = a: the rank q of L C L', the denominator df and F; None when q is 0.
    fn f_test(&self, l: &Matrix, a: &[f64]) -> Option<(usize, f64, f64)> {
        let e = self.e;
        let lcl: Matrix = l
            .iter()
            .map(|x| l.iter().map(|y| dot(x, &mat_vec(&e.a_inv, y))).collect())
            .collect();
        let pairs = symmetric_eigen(&lcl);
        let top = pairs.first().map_or(0.0, |p| p.0);
        // each eigenvector v gives the direction v'L, tested by v'(L b - a)
        let used: Vec<(f64, Vec<f64>, f64)> = pairs
            .into_iter()
            .filter(|(d, _)| top > 0.0 && *d > 1e-8 * top)
            .map(|(d, v)| {
                let lm: Vec<f64> = (0..e.beta.len())
                    .map(|j| v.iter().zip(l).map(|(c, row)| c * row[j]).sum())
                    .collect();
                let shift = dot(&lm, &e.beta) - dot(&v, a);
                (d, lm, shift)
            })
            .collect();
        let q = used.len();
        if q == 0 {
            return None;
        }
        let f = used.iter().map(|(d, _, s)| s * s / d).sum::<f64>() / q as f64;
        let df = self.residual_df.unwrap_or_else(|| {
            let nus: Vec<f64> = used
                .iter()
                .map(|(d, lm, _)| {
                    let (_, grad) = self.model.variance_and_gradient(e, lm);
                    satterthwaite(*d, &grad, self.sigma)
                })
                .collect();
            if q == 1 {
                return nus[0];
            }
            let big_e: f64 = nus
                .iter()
                .filter(|v| **v > 2.0)
                .map(|v| v / (v - 2.0))
                .sum();
            if big_e > q as f64 {
                2.0 * big_e / (big_e - q as f64)
            } else {
                f64::NAN
            }
        });
        Some((q, df, f))
    }
}

/// The t value of a two-sided interval at `alpha` on `df`, NaN when df is not known.
fn t_crit(alpha: f64, df: f64) -> f64 {
    if df.is_finite() {
        t_quantile(alpha / 2.0, df)
    } else {
        f64::NAN
    }
}

/// The fixed part for EMMEANS and TEST: combinations over every parameter of `fit` are checked
/// for estimability and reduced to the kept columns.
struct FixedPart<'a> {
    inf: Infer<'a>,
    d: &'a Data,
    fit: &'a Fit,
    h: Matrix,
    kept: &'a [usize],
    cin: f64,
    dependent: &'a str,
}

impl FixedPart<'_> {
    fn reduce(&self, l: &[f64]) -> Option<Vec<f64>> {
        self.fit
            .estimable(l, &self.h)
            .then(|| self.kept.iter().map(|&j| l[j]).collect())
    }

    fn alpha(&self) -> f64 {
        1.0 - self.cin / 100.0
    }

    fn emmeans(
        &self,
        e: &Emmeans,
        label_of: &dyn Fn(usize, usize) -> String,
        level_of: &dyn Fn(usize, &Value) -> Option<usize>,
    ) -> Result<Vec<Table>, Error> {
        let d = self.d;
        let find = |name: &str| -> Result<usize, Error> {
            d.factors
                .iter()
                .position(|v| v.eq_ignore_ascii_case(name))
                .ok_or_else(|| Error(format!("EMMEANS names {name}, which is not a factor")))
        };
        let factors: Vec<usize> = e
            .factors
            .iter()
            .map(|n| find(n))
            .collect::<Result<_, _>>()?;
        let with = covariate_values(d, &e.with, "EMMEANS")?;
        let alpha = self.alpha();
        let combos = combinations(
            &factors
                .iter()
                .map(|&f| d.values[f].len())
                .collect::<Vec<_>>(),
        );
        // every mean over all parameters; a difference is checked on its own, since it can be
        // estimable when the two means are not
        let full: Vec<Vec<f64>> = combos
            .iter()
            .map(|combo| {
                let fixed: Vec<(usize, usize)> =
                    factors.iter().copied().zip(combo.iter().copied()).collect();
                emmeans_l(d, self.fit, &fixed, &with)
            })
            .collect();
        let ls: Vec<Option<Vec<f64>>> = full.iter().map(|l| self.reduce(l)).collect();
        let ci = |columns: &mut Vec<(String, String)>, what: &str| {
            let group = format!("{}{what}", ci_title(self.cin));
            columns.push((group.clone(), "Lower Bound".into()));
            columns.push((group, "Upper Bound".into()));
        };
        let mut columns: Vec<(String, String)> = factors
            .iter()
            .map(|&f| (String::new(), d.factor_texts[f].clone()))
            .collect();
        columns.extend(plain(&["Mean", "Std. Error", "df"]));
        ci(&mut columns, "");
        let mut rows = Vec::new();
        let mut not_estimable = false;
        for (combo, l) in combos.iter().zip(&ls) {
            let mut row: Vec<Cell> = factors
                .iter()
                .zip(combo)
                .enumerate()
                .map(|(k, (&f, &lv))| {
                    if rows.is_empty() || combo[k + 1..].iter().all(|&x| x == 0) {
                        text(label_of(f, lv))
                    } else {
                        Cell::Empty
                    }
                })
                .collect();
            match l {
                Some(l) => {
                    let (m, se, df) = self.inf.estimate(l);
                    let tc = t_crit(alpha, df);
                    row.extend([m, se, df, m - tc * se, m + tc * se].map(|v| number(Some(v))));
                }
                None => {
                    not_estimable = true;
                    row.extend(std::iter::repeat_n(Cell::Empty, 5));
                }
            }
            rows.push(row);
        }
        let dependent = self.dependent;
        let mut caption = format!(
            "Dependent Variable: {dependent}.{}",
            covariate_note(d, &with)
        );
        if not_estimable {
            caption.push_str(" A mean without a value is not estimable.");
        }
        let title = if factors.is_empty() {
            "Estimated Marginal Means: Grand Mean".to_string()
        } else {
            format!("Estimated Marginal Means: {}", e.factors.join(" * "))
        };
        let mut out = vec![table(&title, factors.len(), columns, rows, Some(caption))];
        let Some(compare) = &e.compare else {
            return Ok(out);
        };
        let compared = match compare {
            Some(n) => find(n)?,
            None => factors[0],
        };
        let Some(at) = factors.iter().position(|&f| f == compared) else {
            return Err(Error(format!(
                "COMPARE names {}, which is not in TABLES",
                d.factors[compared]
            )));
        };
        let k = d.values[compared].len();
        let reference = match &e.refcat {
            None => None,
            Some(RefCat::First) => Some(0),
            Some(RefCat::Last) => Some(k - 1),
            Some(RefCat::Value(v)) => Some(level_of(compared, v).ok_or_else(|| {
                let shown_value = match v {
                    Value::Number(n) => n.to_string(),
                    Value::String(s) => format!("'{s}'"),
                };
                Error(format!(
                    "REFCAT {shown_value} is not a level of {} among the cases used",
                    d.factors[compared]
                ))
            })?),
        };
        let pairs: Vec<(usize, usize)> = match reference {
            Some(r) => (0..k).filter(|&i| i != r).map(|i| (i, r)).collect(),
            None => (0..k)
                .flat_map(|i| (0..k).filter(move |&j| j != i).map(move |j| (i, j)))
                .collect(),
        };
        let m = if reference.is_some() {
            k as f64 - 1.0
        } else {
            (k * (k - 1) / 2) as f64
        };
        let others: Vec<usize> = (0..factors.len()).filter(|&i| i != at).collect();
        let other_combos = combinations(
            &others
                .iter()
                .map(|&i| d.values[factors[i]].len())
                .collect::<Vec<_>>(),
        );
        let mean_at = |other: &[usize], lv: usize| -> &Vec<f64> {
            let mut combo = vec![0; factors.len()];
            for (o, &i) in other.iter().zip(&others) {
                combo[i] = *o;
            }
            combo[at] = lv;
            &full[combos
                .iter()
                .position(|c| *c == combo)
                .expect("a combination")]
        };
        // the difference of two means over the kept columns, when it is estimable
        let difference = |a: &[f64], b: &[f64]| -> Option<Vec<f64>> {
            self.reduce(&a.iter().zip(b).map(|(x, y)| x - y).collect::<Vec<f64>>())
        };
        let mut missing = false;
        let mut columns: Vec<(String, String)> = others
            .iter()
            .map(|&i| (String::new(), d.factor_texts[factors[i]].clone()))
            .collect();
        let name = d.factor_texts[compared].as_str();
        columns.push((String::new(), format!("(I) {name}")));
        columns.push((String::new(), format!("(J) {name}")));
        columns.extend(plain(&[
            "Mean Difference (I-J)",
            "Std. Error",
            "df",
            "Sig.",
        ]));
        ci(&mut columns, " for Difference");
        let mut rows = Vec::new();
        let mut tests = Vec::new();
        for other in &other_combos {
            for (n, &(i, j)) in pairs.iter().enumerate() {
                let mut row: Vec<Cell> = others
                    .iter()
                    .zip(other)
                    .map(|(&o, &lv)| {
                        if n == 0 {
                            text(label_of(factors[o], lv))
                        } else {
                            Cell::Empty
                        }
                    })
                    .collect();
                let first_of_i = n == 0 || pairs[n - 1].0 != i;
                row.push(if first_of_i {
                    text(label_of(compared, i))
                } else {
                    Cell::Empty
                });
                row.push(text(label_of(compared, j)));
                match difference(mean_at(other, i), mean_at(other, j)) {
                    Some(diff) => {
                        let (md, se, df) = self.inf.estimate(&diff);
                        let p = adjusted(t_p(md / se, df), m, e.adjustment);
                        let tc = if df.is_finite() {
                            adjusted_t(alpha, m, df, e.adjustment)
                        } else {
                            f64::NAN
                        };
                        row.extend(
                            [md, se, df, p, md - tc * se, md + tc * se].map(|v| number(Some(v))),
                        );
                    }
                    None => {
                        missing = true;
                        row.extend(std::iter::repeat_n(Cell::Empty, 6));
                    }
                }
                rows.push(row);
            }
            // the simple effect of the compared factor at these levels of the others: every
            // estimable difference; f_test finds their rank
            let l: Matrix = (0..k)
                .flat_map(|i| (i + 1..k).map(move |j| (i, j)))
                .filter_map(|(i, j)| difference(mean_at(other, i), mean_at(other, j)))
                .collect();
            missing |= l.len() < k * (k - 1) / 2;
            let label: Vec<String> = others
                .iter()
                .zip(other)
                .map(|(&o, &lv)| {
                    format!(
                        "{} = {}",
                        d.factor_texts[factors[o]],
                        label_of(factors[o], lv)
                    )
                })
                .collect();
            let mut row = vec![text(if label.is_empty() {
                name.to_string()
            } else {
                label.join(", ")
            })];
            match self.inf.f_test(&l, &vec![0.0; l.len()]) {
                Some((q, df, f)) => row.extend([
                    Cell::Number(q as f64),
                    number(Some(df)),
                    number(Some(f)),
                    number(Some(f_p(f, q as f64, df))),
                ]),
                None => row.extend(std::iter::repeat_n(Cell::Empty, 4)),
            }
            tests.push(row);
        }
        out.push(table(
            "Pairwise Comparisons",
            others.len() + 2,
            columns,
            rows,
            Some(format!(
                "Dependent Variable: {dependent}. Based on estimated marginal means. Adjustment for multiple comparisons: {}.{}",
                adjustment_name(e.adjustment),
                if missing {
                    " A difference without a value is not estimable; the F tests use the estimable ones."
                } else {
                    ""
                }
            )),
        ));
        out.push(table(
            "Univariate Tests",
            1,
            plain(&["", "Numerator df", "Denominator df", "F", "Sig."]),
            tests,
            Some(format!(
                "Dependent Variable: {dependent}. The F tests the effect of {name}, based on the linearly independent pairwise comparisons among the estimated marginal means."
            )),
        ));
        Ok(out)
    }

    /// TEST: one row per estimable combination and the joint F test of all of them.
    fn custom_test(
        &self,
        n: usize,
        t: &CustomTest,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<Table>, Error> {
        let title = t
            .label
            .clone()
            .unwrap_or_else(|| format!("Custom Hypothesis Test {n}"));
        let alpha = self.alpha();
        let mut rows = Vec::new();
        let mut l = Vec::new();
        let mut a = Vec::new();
        for (r, (row, constant)) in t.rows.iter().zip(&t.constants).enumerate() {
            let full = l_row(self.d, self.fit, row, "TEST")?;
            let name = format!("L{}", r + 1);
            let Some(lr) = self.reduce(&full) else {
                warnings.push(format!(
                    "TEST {title}: {name} is not estimable and is not tested"
                ));
                continue;
            };
            let (est, se, df) = self.inf.estimate(&lr);
            let tv = (est - constant) / se;
            let tc = t_crit(alpha, df);
            let mut cells = vec![text(name)];
            cells.extend(
                [
                    est,
                    *constant,
                    se,
                    df,
                    tv,
                    t_p(tv, df),
                    est - tc * se,
                    est + tc * se,
                ]
                .map(|v| number(Some(v))),
            );
            rows.push(cells);
            l.push(lr);
            a.push(*constant);
        }
        let caption = format!("{title}. Dependent Variable: {}.", self.dependent);
        let mut out = Vec::new();
        if rows.is_empty() {
            return Ok(out);
        }
        let mut columns = plain(&[
            "Contrast",
            "Estimate",
            "Test Value",
            "Std. Error",
            "df",
            "t",
            "Sig.",
        ]);
        let group = ci_title(self.cin);
        columns.push((group.clone(), "Lower Bound".into()));
        columns.push((group, "Upper Bound".into()));
        out.push(table(
            "Contrast Estimates",
            1,
            columns,
            rows,
            Some(caption.clone()),
        ));
        if let Some((q, df, f)) = self.inf.f_test(&l, &a) {
            out.push(table(
                "Test Results",
                1,
                plain(&["Source", "Numerator df", "Denominator df", "F", "Sig."]),
                vec![vec![
                    text(title),
                    Cell::Number(q as f64),
                    number(Some(df)),
                    number(Some(f)),
                    number(Some(f_p(f, q as f64, df))),
                ]],
                Some(caption),
            ));
        }
        Ok(out)
    }
}

/// A square matrix table with the same names on its rows and columns.
fn matrix_table(
    title: &str,
    names: &[String],
    value: &dyn Fn(usize, usize) -> Cell,
    caption: String,
) -> Table {
    let mut columns = plain(&[""]);
    columns.extend(names.iter().map(|n| (String::new(), n.clone())));
    let rows = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let mut row = vec![text(n.clone())];
            row.extend((0..names.len()).map(|j| value(i, j)));
            row
        })
        .collect();
    table(title, 1, columns, rows, Some(caption))
}

/// The largest block of cases the dense likelihood takes.
// shortcut: V is formed densely per independent set of cases; a random effect without SUBJECT
// that links every case makes one set of all cases. Upgrade to the sparse mixed model
// equations when larger files need it.
const LARGEST_GROUP: usize = 2000;

struct Fitted {
    th: Vec<f64>,
    eval: Eval,
    sigma: Option<Matrix>,
}

fn fit_model(
    model: &Model,
    start: Vec<f64>,
    command: &Mixed,
) -> Result<(Fitted, Vec<String>), Error> {
    let mut warnings = Vec::new();
    let mut th = start;
    let mut converged = false;
    let mut iteration = 0;
    while iteration < command.mxiter {
        iteration += 1;
        let scoring = iteration <= command.scoring;
        let Some(e) = model.evaluate(&th, Depth::Hessian { expected: scoring }) else {
            return Err(Error(
                "MIXED: the covariance matrix of the cases is not positive definite at the start"
                    .into(),
            ));
        };
        let mut step = solve_pd(&e.hess, &e.grad);
        if step.is_none() && !scoring {
            let ex = model.evaluate(&th, Depth::Hessian { expected: true });
            step = ex.and_then(|ex| solve_pd(&ex.hess, &ex.grad));
        }
        let Some(step) = step else {
            warnings.push(format!(
                "MIXED stopped at iteration {iteration}: the Hessian is not positive definite. The estimates may not be final."
            ));
            break;
        };
        let l_now = -e.f / 2.0;
        if command.hconverge.value > 0.0 {
            let mut crit = dot(&e.grad, &step);
            if command.hconverge.relative {
                crit /= if l_now == 0.0 { 1.0 } else { l_now.abs() };
            }
            if crit.abs() < command.hconverge.value {
                converged = true;
                break;
            }
        }
        let mut rho = 1.0;
        let mut accepted = None;
        for _ in 0..=command.mxstep {
            let candidate: Vec<f64> = th.iter().zip(&step).map(|(t, d)| t - rho * d).collect();
            if model.valid(&candidate)
                && let Some(o) = model.evaluate(&candidate, Depth::Objective)
                && o.f <= e.f + 1e-10 * e.f.abs().max(1.0)
            {
                accepted = Some((candidate, o.f));
                break;
            }
            rho /= 2.0;
        }
        let Some((next, f_next)) = accepted else {
            warnings.push(format!(
                "MIXED stopped at iteration {iteration}: no step of up to MXSTEP halvings improved the likelihood. The estimates may not be final."
            ));
            break;
        };
        let pc = command.pconverge;
        let change = th
            .iter()
            .zip(&next)
            .map(|(a, b)| {
                let d = (b - a).abs();
                if pc.relative {
                    d / if *a == 0.0 { 1.0 } else { a.abs() }
                } else {
                    d
                }
            })
            .fold(0.0_f64, f64::max);
        let lc = command.lconverge;
        let mut l_change = ((f_next - e.f) / 2.0).abs();
        if lc.relative {
            l_change /= if l_now == 0.0 { 1.0 } else { l_now.abs() };
        }
        th = next;
        if (pc.value > 0.0 && change < pc.value) || (lc.value > 0.0 && l_change < lc.value) {
            converged = true;
            break;
        }
    }
    if !converged && warnings.is_empty() {
        warnings.push(format!(
            "MIXED reached MXITER ({}) iterations without converging. The estimates may not be final.",
            command.mxiter
        ));
    }
    let eval = model
        .evaluate(&th, Depth::Hessian { expected: false })
        .ok_or_else(|| {
            Error("MIXED: the final covariance matrix is not positive definite".into())
        })?;
    let sigma = cholesky_inverse(&eval.hess).map(|(inv, _)| {
        inv.iter()
            .map(|row| row.iter().map(|v| 2.0 * v).collect())
            .collect()
    });
    Ok((Fitted { th, eval, sigma }, warnings))
}

fn ci_title(cin: f64) -> String {
    format!("{} Confidence Interval", percent_title(1.0 - cin / 100.0))
}

impl Engine {
    pub(crate) fn mixed_model(&mut self, command: &Mixed) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let factor_names = self.expand(&command.factors)?;
        let covariate_names = self.expand(&command.covariates)?;
        let dependent = data.variable(&command.dependent)?;
        let mut seen: Vec<String> = Vec::new();
        for n in std::iter::once(dependent.name().to_string())
            .chain(factor_names.iter().cloned())
            .chain(covariate_names.iter().cloned())
        {
            if seen.iter().any(|s| s.eq_ignore_ascii_case(&n)) {
                return Err(Error(format!(
                    "{n} appears more than once among the dependent variable, factors and covariates"
                )));
            }
            seen.push(n);
        }
        let factor_vars: Vec<Variable> = factor_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let cov_vars: Vec<Variable> = covariate_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        for v in std::iter::once(&dependent).chain(&cov_vars) {
            if v.is_string() {
                return Err(Error(format!(
                    "MIXED needs a numeric dependent variable and numeric covariates, but {} is a string variable",
                    v.name()
                )));
            }
        }
        let regwgt = match &command.regwgt {
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
        // the subject and repeated variables, each once
        let mut group_names: Vec<String> = Vec::new();
        let lists = command.random.iter().map(|r| &r.subject).chain(
            command
                .repeated
                .iter()
                .flat_map(|r| [&r.subject, &r.variables]),
        );
        for list in lists {
            for n in list {
                if !group_names.iter().any(|g| g.eq_ignore_ascii_case(n)) {
                    group_names.push(n.clone());
                }
            }
        }
        let group_vars: Vec<Variable> = group_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(rep) = &command.repeated {
            for r in &command.random {
                if let Some(v) = r
                    .subject
                    .iter()
                    .find(|v| !rep.subject.iter().any(|s| s.eq_ignore_ascii_case(v)))
                {
                    return Err(Error(format!(
                        "MIXED: the RANDOM subject variable {v} must also be a subject variable of REPEATED"
                    )));
                }
            }
        }
        let cases = data.case_count();
        let weights = case_weights(data)?;
        let numeric = |v: &Variable, k: usize| -> Option<f64> {
            if !crate::means::is_valid(v, k, command.include) {
                return None;
            }
            match v.values() {
                Values::Numeric(values) => values.get(k),
                Values::String(_) => None,
            }
        };
        let factor_keys: Vec<Vec<Option<Key>>> = factor_vars
            .iter()
            .map(|v| categories(v, command.include))
            .collect();
        let group_keys: Vec<Vec<Option<Key>>> = group_vars
            .iter()
            .map(|v| categories(v, command.include))
            .collect();
        // a case counts as many times as its rounded weight
        let mut rows: Vec<usize> = Vec::new();
        for k in 0..cases {
            let f = weights.as_ref().map_or(1.0, |w| w[k].round());
            let g = match &regwgt {
                Some(v) => numeric(v, k).unwrap_or(0.0),
                None => 1.0,
            };
            let valid = f >= 1.0
                && g > 0.0
                && numeric(&dependent, k).is_some()
                && cov_vars.iter().all(|v| numeric(v, k).is_some())
                && factor_keys.iter().all(|col| col[k].is_some())
                && group_keys.iter().all(|col| col[k].is_some());
            if valid {
                if f != 1.0 && command.repeated.is_some() {
                    return Err(Error(
                        "MIXED with REPEATED takes no frequency weight other than 1".into(),
                    ));
                }
                rows.extend(std::iter::repeat_n(k, f as usize));
            }
        }
        if rows.is_empty() {
            return Err(Error(
                "MIXED: no case has valid values on every variable".into(),
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
        let labels = Shown::labels(shown.currencies.clone());
        let mut d = Data {
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
            level: rows
                .iter()
                .map(|&k| {
                    factor_keys
                        .iter()
                        .zip(&levels)
                        .map(|(col, ls)| {
                            let key = col[k].as_ref().expect("valid");
                            ls.iter()
                                .position(|l| l.compare(key).is_eq())
                                .expect("a level")
                        })
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
            f: vec![1.0; rows.len()],
            g: rows
                .iter()
                .map(|&k| {
                    regwgt
                        .as_ref()
                        .map_or(1.0, |v| numeric(v, k).expect("valid"))
                })
                .collect(),
            case: rows.clone(),
        };
        let n = rows.len();
        let covariate_texts: Vec<String> = cov_vars.iter().map(|v| shown.variable(v)).collect();
        let relabel = |effects: &mut Vec<Effect>| {
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
        };
        // the fixed part
        let mut terms: Vec<Term> = command
            .fixed
            .iter()
            .filter(|t| !t.variables.is_empty())
            .cloned()
            .collect();
        if command.intercept {
            terms.insert(0, intercept_term());
        }
        let mut fixed_effects = effects_of(&terms, &factor_names, &covariate_names)?;
        relabel(&mut fixed_effects);
        let fixed = Fit::new(&d, fixed_effects, &d.y, command.singular);
        let kept: Vec<usize> = (0..fixed.p).filter(|&j| !fixed.redundant[j]).collect();
        let p = kept.len();
        if n <= p {
            return Err(Error(format!(
                "MIXED: {n} cases are too few for {p} fixed parameters"
            )));
        }
        // the random part
        let empty = vec![Vec::new(); n];
        let mut random_fits = Vec::new();
        let mut blocks = Vec::new();
        let mut nz = Vec::new();
        let mut subject = Vec::new();
        let mut subject_counts = Vec::new();
        let group_column = |name: &String| {
            let at = group_names
                .iter()
                .position(|g| g.eq_ignore_ascii_case(name))
                .expect("listed");
            &group_keys[at]
        };
        // the residual block comes first; its size is known once REPEATED is read
        let mut warnings = Vec::new();
        let repeated = match &command.repeated {
            Some(rep) => {
                let subject_cols: Vec<&Vec<Option<Key>>> =
                    rep.subject.iter().map(group_column).collect();
                let level_cols: Vec<&Vec<Option<Key>>> =
                    rep.variables.iter().map(group_column).collect();
                let (subj, subjects) = index_by(&rows, &subject_cols);
                let (level, distinct) = index_by(&rows, &level_cols);
                for members in &subjects {
                    let mut seen: Vec<usize> = members.iter().map(|&i| level[i]).collect();
                    seen.sort_unstable();
                    if seen.windows(2).any(|w| w[0] == w[1]) {
                        return Err(Error(
                            "MIXED: a subject of REPEATED has the same repeated level twice".into(),
                        ));
                    }
                }
                let mut covtype = rep.covtype;
                if covtype == CovType::Vc {
                    warnings.push(
                        "MIXED: COVTYPE(VC) on REPEATED is run as DIAG, one variance per level"
                            .to_string(),
                    );
                    covtype = CovType::Diag;
                }
                blocks.push(Block::new(covtype, vec![0; distinct.len()], 0));
                Some((subj, level, subjects.len(), distinct))
            }
            None => {
                blocks.push(Block::new(CovType::Id, vec![0], 0));
                None
            }
        };
        let mut start = blocks[0].count;
        for r in &command.random {
            let mut effects = effects_of(&r.terms, &factor_names, &covariate_names)?;
            relabel(&mut effects);
            let fit = Fit::new(&d, effects, &empty, command.singular);
            let effect_of: Vec<usize> = fit
                .effects
                .iter()
                .enumerate()
                .flat_map(|(k, e)| std::iter::repeat_n(k, e.cells.len()))
                .collect();
            let block = Block::new(r.covtype, effect_of, start);
            start += block.count;
            blocks.push(block);
            nz.push(
                fit.x
                    .iter()
                    .map(|row| {
                        row.iter()
                            .enumerate()
                            .filter(|(_, v)| **v != 0.0)
                            .map(|(j, v)| (j, *v))
                            .collect::<Vec<_>>()
                    })
                    .collect::<Vec<_>>(),
            );
            if r.subject.is_empty() {
                subject.push(vec![0; n]);
                subject_counts.push(1);
            } else {
                let cols: Vec<&Vec<Option<Key>>> = r.subject.iter().map(group_column).collect();
                let (index, members) = index_by(&rows, &cols);
                subject.push(index);
                subject_counts.push(members.len());
            }
            random_fits.push(fit);
        }
        let np = start;
        // independent sets of cases
        let mut parent: Vec<usize> = (0..n).collect();
        let mut link = |key: (usize, usize, usize), i: usize, map: &mut HashMap<_, usize>| {
            let first = *map.entry(key).or_insert(i);
            let (a, b) = (find(&mut parent, first), find(&mut parent, i));
            if a != b {
                parent[a] = b;
            }
        };
        let mut map: HashMap<(usize, usize, usize), usize> = HashMap::new();
        for (k, block) in blocks[1..].iter().enumerate() {
            let diagonal = matches!(block.covtype, CovType::Vc | CovType::Id | CovType::Diag);
            for i in 0..n {
                for &(col, _) in &nz[k][i] {
                    let key = (k + 1, subject[k][i], if diagonal { col } else { 0 });
                    link(key, i, &mut map);
                }
            }
        }
        if let Some((subj, ..)) = &repeated {
            for (i, &s) in subj.iter().enumerate() {
                link((0, s, 0), i, &mut map);
            }
        }
        let mut by_root: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            by_root.entry(find(&mut parent, i)).or_default().push(i);
        }
        let mut groups: Vec<Vec<usize>> = by_root.into_values().collect();
        groups.sort();
        if let Some(big) = groups.iter().map(Vec::len).max()
            && big > LARGEST_GROUP
        {
            return Err(Error(format!(
                "MIXED: random effects that link {big} cases into one covariance block are not supported yet (at most {LARGEST_GROUP}); give RANDOM a SUBJECT"
            )));
        }
        let model = Model {
            y: d.y.iter().map(|y| y[0]).collect(),
            x: fixed
                .x
                .iter()
                .map(|row| kept.iter().map(|&j| row[j]).collect())
                .collect(),
            p,
            ml: command.ml,
            blocks,
            nz,
            subject,
            repeated: repeated
                .as_ref()
                .map(|(subj, level, ..)| (subj.clone(), level.clone())),
            g: d.g.clone(),
            groups,
            np,
        };
        // start: the least squares residual variance shared out
        let s2 = fixed.s[0][0] / (n - p) as f64;
        if s2.is_nan() || s2 <= 0.0 {
            return Err(Error(
                "MIXED: the fixed effects fit the dependent variable exactly".into(),
            ));
        }
        let share = s2 / (command.random.len() + 1) as f64;
        let start_values: Vec<f64> = model
            .blocks
            .iter()
            .flat_map(|b| b.start_values(share))
            .collect();
        // EMMEANS holds covariates at their means over the cases, not weighted by REGWGT
        d.g = vec![1.0; n];
        let (fitted, more) = fit_model(&model, start_values, command)?;
        warnings.extend(more);
        let dep_text = shown.variable(&dependent);
        let caption = format!("Dependent Variable: {dep_text}.");
        let e = &fitted.eval;
        let sigma = fitted.sigma.as_ref();
        let residual_df = (n - p) as f64;
        let inf = Infer {
            model: &model,
            e,
            sigma,
            residual_df: command.residual_df.then_some(residual_df),
        };
        let mut tables = Vec::new();

        // Model Dimension
        let mut rows_md: Vec<Vec<Cell>> = Vec::new();
        let mut total_levels = 0usize;
        let mut total_params = 0usize;
        for (i, ef) in fixed.effects.iter().enumerate() {
            let params = ef.range().filter(|&j| !fixed.redundant[j]).count();
            total_levels += ef.cells.len();
            total_params += params;
            rows_md.push(vec![
                text(if i == 0 { "Fixed Effects" } else { "" }),
                text(ef.label.clone()),
                Cell::Number(ef.cells.len() as f64),
                Cell::Empty,
                Cell::Number(params as f64),
                Cell::Empty,
                Cell::Empty,
            ]);
        }
        for (k, (r, fit)) in command.random.iter().zip(&random_fits).enumerate() {
            let block = &model.blocks[k + 1];
            total_params += block.count;
            let subjects = r.subject.join(" * ");
            for (i, ef) in fit.effects.iter().enumerate() {
                total_levels += ef.cells.len();
                let params = match block.covtype {
                    CovType::Vc => Some(1.0),
                    _ if i == 0 => Some(block.count as f64),
                    _ => None,
                };
                rows_md.push(vec![
                    text(if k == 0 && i == 0 {
                        "Random Effects"
                    } else {
                        ""
                    }),
                    text(ef.label.clone()),
                    Cell::Number(ef.cells.len() as f64),
                    text(structure_name(block.covtype)),
                    number(params),
                    text(subjects.clone()),
                    Cell::Empty,
                ]);
            }
        }
        match (&command.repeated, &repeated) {
            (Some(rep), Some((_, _, subjects, distinct))) => {
                let block = &model.blocks[0];
                total_levels += distinct.len();
                total_params += block.count;
                rows_md.push(vec![
                    text("Repeated Effects"),
                    text(rep.variables.join(" * ")),
                    Cell::Number(distinct.len() as f64),
                    text(structure_name(block.covtype)),
                    Cell::Number(block.count as f64),
                    text(rep.subject.join(" * ")),
                    Cell::Number(*subjects as f64),
                ]);
            }
            _ => {
                total_params += 1;
                rows_md.push(vec![
                    text("Residual"),
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Number(1.0),
                    Cell::Empty,
                    Cell::Empty,
                ]);
            }
        }
        rows_md.push(vec![
            text("Total"),
            Cell::Empty,
            Cell::Number(total_levels as f64),
            Cell::Empty,
            Cell::Number(total_params as f64),
            Cell::Empty,
            Cell::Empty,
        ]);
        tables.push(table(
            "Model Dimension",
            2,
            plain(&[
                "",
                "",
                "Number of Levels",
                "Covariance Structure",
                "Number of Parameters",
                "Subject Variables",
                "Number of Subjects",
            ]),
            rows_md,
            Some(caption.clone()),
        ));

        // Information Criteria
        let m2ll = e.f;
        let (nn, dd) = if command.ml {
            (n as f64, (p + np) as f64)
        } else {
            ((n - p) as f64, np as f64)
        };
        let criteria = [
            (
                if command.ml {
                    "-2 Log Likelihood"
                } else {
                    "-2 Restricted Log Likelihood"
                },
                m2ll,
            ),
            ("Akaike's Information Criterion (AIC)", m2ll + 2.0 * dd),
            (
                "Hurvich and Tsai's Criterion (AICC)",
                m2ll + 2.0 * dd * nn / (nn - dd - 1.0),
            ),
            ("Bozdogan's Criterion (CAIC)", m2ll + dd * (nn.ln() + 1.0)),
            ("Schwarz's Bayesian Criterion (BIC)", m2ll + dd * nn.ln()),
        ];
        tables.push(table(
            "Information Criteria",
            1,
            plain(&["", "Value"]),
            criteria
                .iter()
                .map(|(label, v)| vec![text(*label), number(Some(*v))])
                .collect(),
            Some(format!("Smaller values mean a better fit. {caption}")),
        ));

        // Type III Tests of Fixed Effects
        let reduce = |row: &[f64]| -> Vec<f64> { kept.iter().map(|&j| row[j]).collect() };
        let mut tests = Vec::new();
        for (i, ef) in fixed.effects.iter().enumerate() {
            let l: Matrix = fixed.type34(i, false).iter().map(|r| reduce(r)).collect();
            let Some((q, df, f)) = inf.f_test(&l, &vec![0.0; l.len()]) else {
                tests.push(vec![
                    text(ef.label.clone()),
                    Cell::Number(0.0),
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                ]);
                continue;
            };
            tests.push(vec![
                text(ef.label.clone()),
                Cell::Number(q as f64),
                number(Some(df)),
                number(Some(f)),
                number(Some(f_p(f, q as f64, df))),
            ]);
        }
        if !tests.is_empty() {
            tables.push(table(
                "Type III Tests of Fixed Effects",
                1,
                plain(&["Source", "Numerator df", "Denominator df", "F", "Sig."]),
                tests,
                Some(caption.clone()),
            ));
        }

        // Estimates of Fixed Effects
        let alpha = 1.0 - command.cin / 100.0;
        if command.print.solution && fixed.p > 0 {
            let mut columns = plain(&["Parameter", "Estimate", "Std. Error", "df", "t", "Sig."]);
            let group = ci_title(command.cin);
            columns.push((group.clone(), "Lower Bound".into()));
            columns.push((group, "Upper Bound".into()));
            let mut out = Vec::new();
            let mut redundant = false;
            for ef in &fixed.effects {
                for (k, cell) in ef.cells.iter().enumerate() {
                    let j = ef.start + k;
                    let name = parameter_name(&d, ef, cell);
                    let Some(at) = kept.iter().position(|&c| c == j) else {
                        redundant = true;
                        let mut row = vec![text(format!("{name} (redundant)")), Cell::Number(0.0)];
                        row.extend(std::iter::repeat_n(Cell::Empty, 6));
                        out.push(row);
                        continue;
                    };
                    let mut l = vec![0.0; p];
                    l[at] = 1.0;
                    let (b, se, df) = inf.estimate(&l);
                    let t = b / se;
                    let tc = t_crit(alpha, df);
                    out.push(vec![
                        text(name),
                        number(Some(b)),
                        number(Some(se)),
                        number(Some(df)),
                        number(Some(t)),
                        number(Some(t_p(t, df))),
                        number(Some(b - tc * se)),
                        number(Some(b + tc * se)),
                    ]);
                }
            }
            let mut cap = caption.clone();
            if redundant {
                cap.push_str(" A parameter marked redundant is set to zero.");
            }
            tables.push(table(
                "Estimates of Fixed Effects",
                1,
                columns,
                out,
                Some(cap),
            ));
        }
        let names: Vec<String> = fixed
            .effects
            .iter()
            .flat_map(|ef| ef.cells.iter().map(|cell| parameter_name(&d, ef, cell)))
            .collect();
        let at = |j: usize| kept.iter().position(|&c| c == j);
        if command.print.covb {
            tables.push(matrix_table(
                "Covariance Matrix for Estimates of Fixed Effects",
                &names,
                &|i, j| match (at(i), at(j)) {
                    (Some(a), Some(b)) => Cell::Number(e.a_inv[a][b]),
                    _ => Cell::Number(0.0),
                },
                caption.clone(),
            ));
        }
        if command.print.corb {
            tables.push(matrix_table(
                "Correlation Matrix for Estimates of Fixed Effects",
                &names,
                &|i, j| match (at(i), at(j)) {
                    (Some(a), Some(b)) => {
                        Cell::Number(e.a_inv[a][b] / (e.a_inv[a][a] * e.a_inv[b][b]).sqrt())
                    }
                    _ => Cell::Empty,
                },
                caption.clone(),
            ));
        }
        let part = FixedPart {
            inf,
            d: &d,
            fit: &fixed,
            // H only serves the estimability checks of TEST and EMMEANS
            h: if command.tests.is_empty() && command.emmeans.is_empty() {
                Vec::new()
            } else {
                fixed.h()
            },
            kept: &kept,
            cin: command.cin,
            dependent: &dep_text,
        };
        for (k, t) in command.tests.iter().enumerate() {
            tables.extend(part.custom_test(k + 1, t, &mut warnings)?);
        }

        // Estimates of Covariance Parameters
        let mut columns = plain(&["", "", "Estimate"]);
        if command.print.testcov {
            columns.extend(plain(&["Std. Error", "Wald Z", "Sig."]));
            let group = ci_title(command.cin);
            columns.push((group.clone(), "Lower Bound".into()));
            columns.push((group, "Upper Bound".into()));
        }
        let z = normal_quantile(1.0 - alpha / 2.0);
        let mut cov_rows = Vec::new();
        let mut push_block = |block: &Block, groups: Vec<String>| {
            let names = block.names();
            let kinds = block.kinds();
            for i in 0..block.count {
                let s = block.start + i;
                let est = fitted.th[s];
                let first = i == 0 || groups[i] != groups[i - 1];
                let name = if command.repeated.is_none() && s == 0 {
                    String::new()
                } else {
                    names[i].clone()
                };
                let mut row = vec![
                    text(if first {
                        groups[i].clone()
                    } else {
                        String::new()
                    }),
                    text(name),
                    number(Some(est)),
                ];
                if command.print.testcov {
                    let se = sigma.map_or(f64::NAN, |m| m[s][s].sqrt());
                    let wald = est / se;
                    let (lo, hi) = match kinds[i] {
                        Kind::Variance => {
                            let w = z * se / est;
                            (est * (-w).exp(), est * w.exp())
                        }
                        Kind::Correlation => {
                            let w = z * se / (1.0 - est * est);
                            ((est.atanh() - w).tanh(), (est.atanh() + w).tanh())
                        }
                        Kind::General => (est - z * se, est + z * se),
                    };
                    row.extend([
                        number(Some(se)),
                        number(Some(wald)),
                        number(Some(normal_p(wald))),
                        number(Some(lo)),
                        number(Some(hi)),
                    ]);
                }
                cov_rows.push(row);
            }
        };
        let residual_group = if command.repeated.is_some() {
            "Repeated Measures"
        } else {
            "Residual"
        };
        push_block(
            &model.blocks[0],
            vec![residual_group.into(); model.blocks[0].count],
        );
        for (k, (r, fit)) in command.random.iter().zip(&random_fits).enumerate() {
            let block = &model.blocks[k + 1];
            let subject = if r.subject.is_empty() {
                String::new()
            } else {
                format!(" [subject = {}]", r.subject.join(" * "))
            };
            let groups = if block.covtype == CovType::Vc {
                fit.effects
                    .iter()
                    .map(|ef| format!("{}{subject}", ef.label))
                    .collect()
            } else {
                let all: Vec<&str> = fit.effects.iter().map(|ef| ef.label.as_str()).collect();
                vec![format!("{}{subject}", all.join(" + ")); block.count]
            };
            push_block(block, groups);
        }
        tables.push(table(
            "Estimates of Covariance Parameters",
            2,
            columns,
            cov_rows,
            Some(caption.clone()),
        ));
        if command.print.g && command.random.is_empty() {
            warnings.push("MIXED: PRINT G needs a RANDOM subcommand and is ignored".into());
        }
        if command.print.g {
            for (k, (r, fit)) in command.random.iter().zip(&random_fits).enumerate() {
                let block = &model.blocks[k + 1];
                let names: Vec<String> = fit
                    .effects
                    .iter()
                    .flat_map(|ef| ef.cells.iter().map(|cell| parameter_name(&d, ef, cell)))
                    .collect();
                let effects: Vec<&str> = fit.effects.iter().map(|ef| ef.label.as_str()).collect();
                let subject = if r.subject.is_empty() {
                    String::new()
                } else {
                    format!(" [subject = {}]", r.subject.join(" * "))
                };
                tables.push(matrix_table(
                    "Random Effect Covariance Structure (G)",
                    &names,
                    &|i, j| Cell::Number(block.value(&fitted.th, i, j, What::Value)),
                    format!(
                        "{}: {}{subject}. {caption}",
                        structure_name(block.covtype),
                        effects.join(" + ")
                    ),
                ));
            }
        }
        match (&command.repeated, &repeated) {
            (Some(rep), Some((_, _, _, distinct))) if command.print.r => {
                let block = &model.blocks[0];
                let names: Vec<String> = distinct
                    .iter()
                    .map(|members| {
                        let k = rows[members[0]];
                        rep.variables
                            .iter()
                            .map(|v| {
                                let at = group_names
                                    .iter()
                                    .position(|g| g.eq_ignore_ascii_case(v))
                                    .expect("listed");
                                let key = group_keys[at][k].as_ref().expect("valid");
                                format!("[{v} = {}]", key_value_text(&group_vars[at], key))
                            })
                            .collect::<Vec<_>>()
                            .join(" * ")
                    })
                    .collect();
                tables.push(matrix_table(
                    "Residual Covariance (R) Matrix",
                    &names,
                    &|i, j| Cell::Number(block.value(&fitted.th, i, j, What::Value)),
                    format!("{}. {caption}", structure_name(block.covtype)),
                ));
            }
            _ if command.print.r => {
                warnings.push("MIXED: PRINT R needs a REPEATED subcommand and is ignored".into());
            }
            _ => {}
        }
        let level_texts: Vec<Vec<String>> = factor_vars
            .iter()
            .zip(&levels)
            .map(|(v, ls)| ls.iter().map(|k| shown.value(v, k)).collect())
            .collect();
        let label_of = |f: usize, l: usize| level_texts[f][l].clone();
        let level_of = |f: usize, v: &Value| -> Option<usize> {
            levels[f].iter().position(|key| match (v, key) {
                (Value::Number(a), Key::Number(b)) => a == b,
                (Value::String(a), Key::Text(b)) => a.trim_end() == b.trim_end(),
                (Value::String(a), Key::Number(b)) => a.trim().parse::<f64>().ok() == Some(*b),
                _ => false,
            })
        };
        for em in &command.emmeans {
            tables.extend(part.emmeans(em, &label_of, &level_of)?);
        }
        for w in warnings {
            self.warn(w);
        }
        if !command.save.is_empty() {
            let mut values: Vec<[f64; 5]> = vec![[f64::NAN; 5]; n];
            for (gi, cases) in model.groups.iter().enumerate() {
                let r: Vec<f64> = cases
                    .iter()
                    .map(|&i| model.y[i] - dot(&model.x[i], &e.beta))
                    .collect();
                let q = mat_vec(&e.vi[gi], &r);
                // y - PRED = R V^-1 r, since Z G Z' = V - R
                let resid: Vec<f64> = cases
                    .iter()
                    .map(|&i| {
                        cases
                            .iter()
                            .zip(&q)
                            .map(|(&j, qj)| {
                                model.residual_entry(&fitted.th, i, j, What::Value) * qj
                            })
                            .sum()
                    })
                    .collect();
                for (a, &i) in cases.iter().enumerate() {
                    let fixpred = dot(&model.x[i], &e.beta);
                    let (_, se, df) = part.inf.estimate(&model.x[i]);
                    values[i] = [fixpred, model.y[i] - resid[a], resid[a], se, df];
                }
            }
            self.save_mixed(command, &d.case, &values, &dep_text, cases)?;
        }
        Ok(tables)
    }

    fn save_mixed(
        &mut self,
        command: &Mixed,
        rows: &[usize],
        values: &[[f64; 5]],
        dependent: &str,
        cases: usize,
    ) -> Result<(), Error> {
        let temps = [
            (MixedTemp::Fixpred, "FXPRED", "Fixed Predicted Values"),
            (MixedTemp::Pred, "PRED", "Predicted Values"),
            (MixedTemp::Resid, "RESID", "Residuals"),
            (
                MixedTemp::Sefixp,
                "SEFIXP",
                "Standard Errors of Fixed Predicted Values",
            ),
            (
                MixedTemp::Dffixp,
                "DFFIXP",
                "Degrees of Freedom of Fixed Predicted Values",
            ),
        ];
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            for (temp, given) in &command.save {
                let at = temps.iter().position(|t| t.0 == *temp).expect("listed");
                let (_, root, what) = temps[at];
                let var = match given {
                    Some(n) => n.clone(),
                    None => {
                        let mut k = 1;
                        while data.variable_index(&format!("{root}_{k}")).is_some() {
                            k += 1;
                        }
                        format!("{root}_{k}")
                    }
                };
                if data.variable_index(&var).is_some() {
                    return Err(Error(format!("MIXED /SAVE: variable {var} already exists")));
                }
                let mut column = vec![None; cases];
                for (i, &k) in rows.iter().enumerate() {
                    let v = values[i][at];
                    column[k] = v.is_finite().then_some(v);
                }
                data.set_numeric_values(&var, column)?;
                data.set_variable_label(&var, Some(&format!("{what} for {dependent}")))?;
                data.set_format(&var, "F11.5")?;
                data.set_measure(&var, Measure::Scale)?;
            }
            Ok(())
        })
    }
}
