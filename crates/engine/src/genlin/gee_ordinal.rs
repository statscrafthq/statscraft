//! GENLIN REPEATED with DISTRIBUTION=MULTINOMIAL: the ordinal generalized estimating equations
//! with a working association between the measurements of a subject.
//!
//! Rules from the Algorithms manual (GENLIN), in our words:
//! - A measurement of a subject is the vector z of the indicators of the first J - 1 categories
//!   (the last category is all zeros), with the category probabilities pi from the cumulative
//!   link. The parameters are the J - 1 thresholds and the regression coefficients.
//! - The covariance of the stacked measurements of a subject is V = phi A^1/2 R A^1/2. A has the
//!   diagonal blocks diag(pi (1 - pi)) / w (w the scale weight of the measurement); R has the
//!   correlation matrix of the multinomial indicators on its diagonal blocks and an association
//!   block, a (J - 1) square matrix, for each pair of measurements.
//! - The association is estimated from the Pearson-like residuals r = (z - pi) sqrt(w / (pi (1 -
//!   pi))), the residual vectors of two measurements multiplied: EXCHANGEABLE puts the mean of the
//!   symmetrised products of every pair into each block; AR(1) puts the mean of the pairs one
//!   position apart into a matrix whose power by the distance is the block; MDEPENDENT(m) uses a
//!   matrix of its own for each distance up to m and zero beyond; UNSTRUCTURED gives each pair of
//!   positions its own matrix, the block of the later position first being the transpose. The
//!   means divide by the number of pairs (counted with the frequencies of their subjects) less
//!   the number of parameters with ADJUSTCORR=YES when the number is larger. The scale does not
//!   divide them. FIXED is not allowed for this model.
//! - A subject's R that is not positive definite is pulled toward the identity, (R + cI) /
//!   (1 + c), until its smallest eigenvalue is 0.01 (the manual leaves the ridge open; each
//!   subject has its own R, so each gets its own ridge, and one that is positive definite with
//!   room to spare is left alone, which keeps the fit continuous). A subject with one measurement
//!   has no association and is never changed.
//! - Fitting starts at the estimates of the model without REPEATED. Each iteration is a Fisher
//!   step on sum f D' V^- (z - pi) = 0, D the derivative of pi by the parameters; the association
//!   is estimated at the start and again after every UPDATECORR iterations (0: never again). A
//!   step that gives a case an invalid probability is halved.
//! - The iterations stop as for the scalar equations: the largest change of an estimate below
//!   PCONVERGE (a full step only) and s' H^-1 s below HCONVERGE, each when above 0.
//! - The model-based covariance is H^-1, H = sum f D' V^- D; the robust one is
//!   H^-1 (sum f g g') H^-1 with g = D' V^- (z - pi) of a subject.

use super::gee::Subjects;
use super::{Ordinal, dot, largest_change, link_d1, link_inverse, mat_vec};
use crate::glm::ginverse;
use crate::matrix::{Matrix, multiply, symmetric_eigen};
use statscraft_syntax::{GenlinCorrelation, GenlinLink, GenlinRepeated, GenlinTolerance};
use std::collections::HashMap;

/// The smallest eigenvalue a subject's R is pulled up to when it is not positive definite.
const FLOOR: f64 = 0.01;

/// The settings of REPEATED that the fit uses.
pub(super) struct Control<'a> {
    correlation: &'a GenlinCorrelation,
    adjust: bool,
    maxiterations: u32,
    pconverge: GenlinTolerance,
    hconverge: f64,
    updatecorr: u32,
    singular: f64,
}

impl<'a> Control<'a> {
    pub(super) fn of(r: &'a GenlinRepeated, singular: f64) -> Self {
        Control {
            correlation: &r.correlation,
            adjust: r.adjust,
            maxiterations: r.maxiterations,
            pconverge: r.pconverge,
            hconverge: r.hconverge,
            updatecorr: r.updatecorr,
            singular,
        }
    }
}

pub(super) struct OrdinalGee {
    pub(super) b: Vec<f64>,
    pub(super) model: Matrix,
    pub(super) robust: Matrix,
    /// The association blocks (J - 1 square) of every pair of positions; None is a zero block.
    pub(super) blocks: Blocks,
    pub(super) converged: bool,
    pub(super) halving_failed: bool,
    pub(super) hessian_ok: bool,
    /// The number of subjects whose R was pulled toward the identity at the last evaluation.
    pub(super) ridged: usize,
    pub(super) history: Vec<Vec<f64>>,
    pub(super) score: Vec<f64>,
    pub(super) information: Matrix,
}

/// The association block of each ordered pair of positions.
pub(super) type Blocks = Vec<Vec<Option<Matrix>>>;

/// The probabilities of one case's first J - 1 categories, their derivative by the parameters
/// (a row per category) and the indicators.
struct Case {
    pi: Vec<f64>,
    d: Matrix,
    z: Vec<f64>,
    /// sqrt(pi (1 - pi))
    root: Vec<f64>,
}

fn case(o: &Ordinal, x: &[Vec<f64>], link: GenlinLink, b: &[f64], i: usize) -> Option<Case> {
    let k = o.j - 1;
    let xb = dot(&x[i], &b[k..]);
    let gamma: Vec<f64> = (0..k)
        .map(|j| link_inverse(link, b[j] - xb + o.offset[i], 0.0))
        .collect();
    let dens: Vec<f64> = gamma.iter().map(|&g| 1.0 / link_d1(link, g, 0.0)).collect();
    let mut pi = Vec::with_capacity(k);
    let mut d = vec![vec![0.0; b.len()]; k];
    for j in 0..k {
        let lower = if j > 0 { gamma[j - 1] } else { 0.0 };
        pi.push(gamma[j] - lower);
        d[j][j] += dens[j];
        let below = if j > 0 {
            d[j][j - 1] -= dens[j - 1];
            dens[j - 1]
        } else {
            0.0
        };
        for (t, v) in x[i].iter().enumerate() {
            d[j][k + t] = -(dens[j] - below) * v;
        }
    }
    let last = 1.0 - gamma[k - 1];
    let valid = last > 0.0 && pi.iter().all(|p| p.is_finite() && *p > 0.0 && *p < 1.0);
    let root = pi.iter().map(|p| (p * (1.0 - p)).sqrt()).collect();
    let z = (0..k)
        .map(|j| f64::from(u8::from(o.category[i] == j)))
        .collect();
    valid.then_some(Case { pi, d, z, root })
}

fn all_valid(o: &Ordinal, x: &[Vec<f64>], link: GenlinLink, b: &[f64]) -> bool {
    (0..o.category.len()).all(|i| case(o, x, link, b, i).is_some())
}

fn zero(k: usize) -> Matrix {
    vec![vec![0.0; k]; k]
}

fn symmetrised(m: &Matrix) -> Matrix {
    let k = m.len();
    (0..k)
        .map(|a| (0..k).map(|b| (m[a][b] + m[b][a]) / 2.0).collect())
        .collect()
}

fn transposed(m: &Matrix) -> Matrix {
    let k = m.len();
    (0..k).map(|a| (0..k).map(|b| m[b][a]).collect()).collect()
}

fn add_to(into: &mut Matrix, m: &Matrix, factor: f64) {
    for (row, from) in into.iter_mut().zip(m) {
        for (v, w) in row.iter_mut().zip(from) {
            *v += factor * w;
        }
    }
}

fn scaled(m: &Matrix, factor: f64) -> Matrix {
    m.iter()
        .map(|r| r.iter().map(|v| v * factor).collect())
        .collect()
}

/// The association blocks at the estimates `b`; None when a case has an invalid probability.
fn estimate(
    o: &Ordinal,
    x: &[Vec<f64>],
    link: GenlinLink,
    s: &Subjects,
    ctl: &Control,
    b: &[f64],
) -> Option<Blocks> {
    let (k, t) = (o.j - 1, s.dim);
    let mut blocks: Blocks = vec![vec![None; t]; t];
    let mut residuals = Vec::with_capacity(o.category.len());
    for i in 0..o.category.len() {
        let c = case(o, x, link, b, i)?;
        residuals.push(
            (0..k)
                .map(|j| (c.z[j] - c.pi[j]) * (o.omega[i] / (c.root[j] * c.root[j])).sqrt())
                .collect::<Vec<f64>>(),
        );
    }
    // the products of the residual vectors at positions u < v, and the numbers of pairs
    let mut pairs: HashMap<(usize, usize), (Matrix, f64)> = HashMap::new();
    for m in &s.members {
        let f = o.f[m[0].0];
        for (at, &(i, u)) in m.iter().enumerate() {
            for &(j, v) in &m[at + 1..] {
                let entry = pairs.entry((u, v)).or_insert_with(|| (zero(k), 0.0));
                for (a, ra) in residuals[i].iter().enumerate() {
                    for (c, rc) in residuals[j].iter().enumerate() {
                        entry.0[a][c] += f * ra * rc;
                    }
                }
                entry.1 += f;
            }
        }
    }
    let parameters = b.len() as f64;
    let divided = |sum: &Matrix, count: f64| {
        let den = if ctl.adjust && count > parameters {
            count - parameters
        } else {
            count
        };
        if count > 0.0 {
            scaled(sum, 1.0 / den)
        } else {
            zero(k)
        }
    };
    // the mean of the symmetrised products of the pairs `distance` apart
    let lag = |distance: usize| {
        let (mut sum, mut count) = (zero(k), 0.0);
        for u in 0..t.saturating_sub(distance) {
            if let Some((m, n)) = pairs.get(&(u, u + distance)) {
                add_to(&mut sum, &symmetrised(m), 1.0);
                count += n;
            }
        }
        divided(&sum, count)
    };
    match ctl.correlation {
        GenlinCorrelation::Exchangeable => {
            let (mut sum, mut count) = (zero(k), 0.0);
            for (m, n) in pairs.values() {
                add_to(&mut sum, &symmetrised(m), 1.0);
                count += n;
            }
            let alpha = divided(&sum, count);
            for (u, row) in blocks.iter_mut().enumerate() {
                for (v, cell) in row.iter_mut().enumerate() {
                    if u != v {
                        *cell = Some(alpha.clone());
                    }
                }
            }
        }
        GenlinCorrelation::Ar1 => {
            let alpha = lag(1);
            let mut power = alpha.clone();
            for distance in 1..t {
                for u in 0..t - distance {
                    blocks[u][u + distance] = Some(power.clone());
                    blocks[u + distance][u] = Some(power.clone());
                }
                power = multiply(&power, &alpha);
            }
        }
        GenlinCorrelation::MDependent(order) => {
            for distance in 1..=(*order as usize).min(t.saturating_sub(1)) {
                let alpha = lag(distance);
                for u in 0..t - distance {
                    blocks[u][u + distance] = Some(alpha.clone());
                    blocks[u + distance][u] = Some(alpha.clone());
                }
            }
        }
        GenlinCorrelation::Unstructured => {
            for (&(u, v), (m, n)) in &pairs {
                let alpha = divided(m, *n);
                blocks[v][u] = Some(transposed(&alpha));
                blocks[u][v] = Some(alpha);
            }
        }
        GenlinCorrelation::Independent | GenlinCorrelation::Fixed(_) => {}
    }
    Some(blocks)
}

/// The score, H, the middle of the robust covariance and the number of subjects whose R was
/// pulled toward the identity, at the estimates `b`.
fn parts(
    o: &Ordinal,
    x: &[Vec<f64>],
    link: GenlinLink,
    s: &Subjects,
    blocks: &Blocks,
    phi: f64,
    b: &[f64],
) -> Option<(Vec<f64>, Matrix, Matrix, usize)> {
    let (k, q) = (o.j - 1, b.len());
    let mut score = vec![0.0; q];
    let mut h = vec![vec![0.0; q]; q];
    let mut meat = vec![vec![0.0; q]; q];
    let mut ridged = 0;
    for m in &s.members {
        let n = m.len();
        let f = o.f[m[0].0];
        let cases: Vec<Case> = m
            .iter()
            .map(|&(i, _)| case(o, x, link, b, i))
            .collect::<Option<_>>()?;
        let size = n * k;
        let mut r = vec![vec![0.0; size]; size];
        for (a, ca) in cases.iter().enumerate() {
            for (j, row) in r[a * k..(a + 1) * k].iter_mut().enumerate() {
                for (l, cell) in row[a * k..(a + 1) * k].iter_mut().enumerate() {
                    *cell = if j == l {
                        1.0
                    } else {
                        -ca.pi[j] * ca.pi[l] / (ca.root[j] * ca.root[l])
                    };
                }
            }
            for (bb, &(_, v)) in m.iter().enumerate() {
                let Some(block) = &blocks[m[a].1][v] else {
                    continue;
                };
                if bb == a {
                    continue;
                }
                for (j, row) in r[a * k..(a + 1) * k].iter_mut().enumerate() {
                    row[bb * k..(bb + 1) * k].copy_from_slice(&block[j]);
                }
            }
        }
        if n > 1 {
            let smallest = symmetric_eigen(&r)
                .iter()
                .map(|(v, _)| *v)
                .fold(f64::INFINITY, f64::min);
            if smallest < FLOOR {
                let ridge = (FLOOR - smallest) / (1.0 - FLOOR);
                for (a, row) in r.iter_mut().enumerate() {
                    for (c, cell) in row.iter_mut().enumerate() {
                        *cell = (*cell + if a == c { ridge } else { 0.0 }) / (1.0 + ridge);
                    }
                }
                ridged += 1;
            }
        }
        let inverse = ginverse(&r, 1e-12).0;
        // A^-1/2 over the stacked categories
        let root: Vec<f64> = m
            .iter()
            .zip(&cases)
            .flat_map(|(&(i, _), c)| c.root.iter().map(move |v| v / o.omega[i].sqrt()))
            .collect();
        let d: Matrix = cases.iter().flat_map(|c| c.d.clone()).collect();
        let e: Vec<f64> = cases
            .iter()
            .flat_map(|c| c.z.iter().zip(&c.pi).map(|(z, p)| z - p))
            .collect();
        let w = |a: usize, c: usize| inverse[a][c] / (phi * root[a] * root[c]);
        let mut g = vec![0.0; q];
        for a in 0..size {
            let mut we = 0.0;
            let mut wd = vec![0.0; q];
            for c in 0..size {
                let wac = w(a, c);
                we += wac * e[c];
                for (t, v) in wd.iter_mut().enumerate() {
                    *v += wac * d[c][t];
                }
            }
            for j in 0..q {
                g[j] += d[a][j] * we;
                for l in 0..q {
                    h[j][l] += f * d[a][j] * wd[l];
                }
            }
        }
        for j in 0..q {
            score[j] += f * g[j];
            for l in 0..q {
                meat[j][l] += f * g[j] * g[l];
            }
        }
    }
    Some((score, h, meat, ridged))
}

/// The fit from the estimates `b0` of the model of independent measurements, at the scale `phi`.
pub(super) fn fit(
    o: &Ordinal,
    x: &[Vec<f64>],
    link: GenlinLink,
    s: &Subjects,
    ctl: &Control,
    (b0, phi): (&[f64], f64),
) -> Result<OrdinalGee, String> {
    let failed = || {
        "GENLIN: REPEATED: the estimating equations give a case an invalid probability".to_string()
    };
    let mut b = b0.to_vec();
    let mut blocks = estimate(o, x, link, s, ctl, &b).ok_or_else(failed)?;
    let (mut score, mut h, mut meat, mut ridged) =
        parts(o, x, link, s, &blocks, phi, &b).ok_or_else(failed)?;
    let mut inverse = ginverse(&h, ctl.singular).0;
    let quadratic = |score: &[f64], inverse: &Matrix| dot(score, &mat_vec(inverse, score)).abs();
    let mut converged = ctl.maxiterations == 0;
    let mut halving_failed = false;
    let mut history = vec![b.clone()];
    for iteration in 1..=ctl.maxiterations {
        let step = mat_vec(&inverse, &score);
        let mut size = 1.0;
        let next = loop {
            let next: Vec<f64> = b.iter().zip(&step).map(|(v, d)| v + size * d).collect();
            if all_valid(o, x, link, &next) {
                break Some(next);
            }
            size /= 2.0;
            if size < 1e-9 {
                break None;
            }
        };
        let Some(next) = next else {
            halving_failed = true;
            break;
        };
        if ctl.updatecorr > 0 && iteration % ctl.updatecorr == 0 {
            blocks = estimate(o, x, link, s, ctl, &next).ok_or_else(failed)?;
        }
        let change = largest_change(&b, &next, ctl.pconverge.relative);
        b = next;
        history.push(b.clone());
        (score, h, meat, ridged) = parts(o, x, link, s, &blocks, phi, &b).ok_or_else(failed)?;
        inverse = ginverse(&h, ctl.singular).0;
        // a shortened step says nothing about convergence
        let p_met = ctl.pconverge.value == 0.0 || (size == 1.0 && change < ctl.pconverge.value);
        let h_met = ctl.hconverge == 0.0 || quadratic(&score, &inverse) < ctl.hconverge;
        if p_met && h_met {
            converged = true;
            break;
        }
    }
    let hessian_ok =
        ctl.hconverge > 0.0 || ctl.maxiterations == 0 || quadratic(&score, &inverse) < 1e-4;
    let model = inverse;
    let robust = multiply(&multiply(&model, &meat), &model);
    Ok(OrdinalGee {
        b,
        model,
        robust,
        blocks,
        converged,
        halving_failed,
        hessian_ok,
        ridged,
        history,
        score,
        information: h,
    })
}
