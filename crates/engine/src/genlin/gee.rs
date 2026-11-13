//! GENLIN REPEATED: generalized estimating equations.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (GENLIN), in our words:
//! - Subjects. With SORT=YES the cases that share the values of the SUBJECT variables make one
//!   subject. WITHINSUBJECT orders its measurements: each distinct combination of its values
//!   (over all subjects, ascending) is one position, and a position a subject lacks is a missing
//!   measurement. Without WITHINSUBJECT the cases of a subject keep their file order, so any
//!   missing ones count as the last. With SORT=NO every run of equal SUBJECT values in file
//!   order is a subject of its own and WITHINSUBJECT is ignored with a warning.
//! - The working correlation matrix R has a row per position. It is estimated from the Pearson
//!   residuals r = (y - mu) sqrt(w / V(mu)), w the prior weight: the mean product of the
//!   residual pairs of a subject (EXCHANGEABLE: all pairs; AR(1): pairs one position apart, R
//!   then the power of the distance; MDEPENDENT(m): pairs j apart for each j up to m, which is
//!   cut to the dimension less 1; UNSTRUCTURED: each pair of positions on its own), divided by
//!   the scale moment sum f r^2 / N', N' the measurements counted with their frequencies. With
//!   ADJUSTCORR=YES each count in a denominator loses the number of parameters, if it is larger.
//!   An estimate that is not positive definite is pulled toward the identity, (R + cI) / (1 + c);
//!   a FIXED matrix that is not positive definite is an error.
//! - Fitting starts at the estimates of the model without REPEATED. Each iteration is a Fisher
//!   step on sum f D' V^-1 (y - mu) = 0 with V = phi A^1/2 R A^1/2, A the variance function over
//!   the prior weight, D the derivative of the means. R is estimated at the start and again after
//!   every UPDATECORR iterations (0: never again). With SCALE=MLE the scale becomes the Pearson
//!   moment after every iteration; any other scale keeps the value of the first fit, and so does a
//!   negative binomial k.
//! - The iterations stop when the largest change of an estimate (absolute or relative) is below
//!   PCONVERGE and s' H^-1 s is below HCONVERGE, each when above 0. When HCONVERGE is 0, the
//!   last s' H^-1 s is held against 1E-4 and a warning says when it is above.
//! - The model-based covariance is H^-1, H = sum f D' V^-1 D; the robust one is
//!   H^-1 (sum f D' V^-1 e e' V^-1 D) H^-1 with e the residuals of a subject.
//! - QIC = -2 Q + 2 trace(H_I C) and QICC = -2 Q + 2 p: Q is the quasi-likelihood under
//!   independence at the estimates (full or kernel as LIKELIHOOD says), at a scale fixed on
//!   CRITERIA or else 1; H_I is H of the fit with independent measurements; C is the robust
//!   covariance.

use super::{Cases, Family, link_d1, link_inverse, mat_vec, mu_valid, variance};
use crate::crosstabs::Key;
use crate::glm::ginverse;
use crate::matrix::{Matrix, multiply, symmetric_eigen};
use statscraft_syntax::{GenlinCorrelation, GenlinLink, GenlinTolerance};
use std::cmp::Ordering;
use std::collections::HashMap;

/// The subjects of a fit.
pub(super) struct Subjects {
    /// The rows of the fit of each subject with their positions (0-based), in position order.
    pub(super) members: Vec<Vec<(usize, usize)>>,
    /// The number of positions: the dimension of the working correlation matrix.
    pub(super) dim: usize,
    /// The WITHINSUBJECT values of each position, when they set the positions.
    pub(super) positions: Vec<Vec<Key>>,
}

fn compare(a: &[Key], b: &[Key]) -> Ordering {
    a.iter()
        .zip(b)
        .map(|(x, y)| x.compare(y))
        .find(|o| o.is_ne())
        .unwrap_or(Ordering::Equal)
}

/// Groups the rows of the fit (in file order) into subjects. `subject` and `within` hold the
/// values of each row.
pub(super) fn subjects(
    subject: &[Vec<Key>],
    within: Option<&[Vec<Key>]>,
    sort: bool,
) -> Result<Subjects, String> {
    let n = subject.len();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    if sort {
        let mut order: Vec<usize> = (0..n).collect();
        // stable: without WITHINSUBJECT the file order stays within a subject
        order.sort_by(|a, b| compare(&subject[*a], &subject[*b]));
        for (at, &r) in order.iter().enumerate() {
            if at == 0 || compare(&subject[order[at - 1]], &subject[r]).is_ne() {
                groups.push(Vec::new());
            }
            groups.last_mut().expect("a group").push(r);
        }
    } else {
        for r in 0..n {
            if r == 0 || compare(&subject[r - 1], &subject[r]).is_ne() {
                groups.push(Vec::new());
            }
            groups.last_mut().expect("a group").push(r);
        }
    }
    let within = within.filter(|_| sort);
    let mut positions: Vec<Vec<Key>> = Vec::new();
    if let Some(w) = within {
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|a, b| compare(&w[*a], &w[*b]));
        for (at, &r) in order.iter().enumerate() {
            if at == 0 || compare(&w[order[at - 1]], &w[r]).is_ne() {
                positions.push(w[r].clone());
            }
        }
    }
    let mut members = Vec::with_capacity(groups.len());
    for g in groups {
        let mut m: Vec<(usize, usize)> = match within {
            Some(w) => g
                .iter()
                .map(|&r| {
                    let p = positions
                        .binary_search_by(|k| compare(k, &w[r]))
                        .expect("a position");
                    (r, p)
                })
                .collect(),
            None => g.iter().enumerate().map(|(p, &r)| (r, p)).collect(),
        };
        m.sort_by_key(|&(_, p)| p);
        if m.windows(2).any(|w| w[0].1 == w[1].1) {
            return Err(
                "GENLIN REPEATED: two cases of one subject have the same WITHINSUBJECT values"
                    .into(),
            );
        }
        members.push(m);
    }
    let dim = if within.is_some() {
        positions.len()
    } else {
        members.iter().map(Vec::len).max().unwrap_or(0)
    };
    Ok(Subjects {
        members,
        dim,
        positions,
    })
}

/// What the fit needs besides the cases.
pub(super) struct Settings<'a> {
    pub(super) family: Family,
    pub(super) link: GenlinLink,
    pub(super) k: f64,
    pub(super) correlation: &'a GenlinCorrelation,
    pub(super) adjust: bool,
    /// SCALE=MLE: the scale follows the Pearson residuals.
    pub(super) update_scale: bool,
    pub(super) maxiterations: u32,
    pub(super) pconverge: GenlinTolerance,
    pub(super) hconverge: f64,
    pub(super) updatecorr: u32,
    pub(super) singular: f64,
}

pub(super) struct GeeFit {
    pub(super) beta: Vec<f64>,
    pub(super) mu: Vec<f64>,
    pub(super) phi: f64,
    pub(super) r: Matrix,
    /// H = sum f D' V^-1 D at the estimates.
    pub(super) information: Matrix,
    pub(super) model: Matrix,
    pub(super) robust: Matrix,
    pub(super) converged: bool,
    pub(super) hessian_ok: bool,
    /// The final estimated matrix was not positive definite and was pulled toward the identity.
    pub(super) ridged: bool,
}

fn means(c: &Cases, x: &[Vec<f64>], set: &Settings, beta: &[f64]) -> Option<Vec<f64>> {
    (0..c.n())
        .map(|i| {
            let eta = super::dot(&x[i], beta) + c.offset[i];
            let mu = link_inverse(set.link, eta, set.k);
            mu_valid(set.family, mu).then_some(mu)
        })
        .collect()
}

/// The working correlation matrix at the means `mu`, the scale moment, and whether a ridge was
/// needed.
/// The Pearson residual of every row.
fn residuals(c: &Cases, set: &Settings, mu: &[f64]) -> Vec<f64> {
    (0..c.n())
        .map(|i| (c.y[i] - mu[i]) * (c.a(i) / variance(set.family, mu[i], set.k)).sqrt())
        .collect()
}

/// The scale moment sum f r^2 / N', N' less the parameters with ADJUSTCORR=YES when larger.
fn moment(c: &Cases, s: &Subjects, set: &Settings, px: usize, r: &[f64]) -> f64 {
    let n_prime: f64 = s.members.iter().map(|m| c.f[m[0].0] * m.len() as f64).sum();
    let px = px as f64;
    let den = if set.adjust && n_prime > px {
        n_prime - px
    } else {
        n_prime
    };
    (0..c.n()).map(|i| c.f[i] * r[i] * r[i]).sum::<f64>() / den
}

fn estimate(
    c: &Cases,
    s: &Subjects,
    set: &Settings,
    px: usize,
    mu: &[f64],
) -> Result<(Matrix, f64, bool), String> {
    let t = s.dim;
    let px = px as f64;
    let adj = |den: f64| {
        if set.adjust && den > px {
            den - px
        } else {
            den
        }
    };
    let r = residuals(c, set, mu);
    // the residual of each subject at each position; the frequency of each subject
    let grid: Vec<Vec<Option<f64>>> = s
        .members
        .iter()
        .map(|m| {
            let mut row = vec![None; t];
            for &(i, p) in m {
                row[p] = Some(r[i]);
            }
            row
        })
        .collect();
    let freq: Vec<f64> = s.members.iter().map(|m| c.f[m[0].0]).collect();
    // the divisor of every correlation estimate, also when the model's scale is fixed
    let phi = moment(c, s, set, px as usize, &r);
    let ratio = |num: f64, den: f64| {
        let v = num / adj(den) / phi;
        if den > 0.0 && v.is_finite() { v } else { 0.0 }
    };
    // the sum of products and the number of pairs at positions u and v
    let pair = |u: usize, v: usize| -> (f64, f64) {
        let (mut num, mut den) = (0.0, 0.0);
        for (row, f) in grid.iter().zip(&freq) {
            if let (Some(a), Some(b)) = (row[u], row[v]) {
                num += f * a * b;
                den += f;
            }
        }
        (num, den)
    };
    let lag = |j: usize| -> f64 {
        let (num, den) = (0..t - j)
            .map(|u| pair(u, u + j))
            .fold((0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1));
        ratio(num, den)
    };
    let mut m: Matrix = (0..t)
        .map(|u| (0..t).map(|v| f64::from(u8::from(u == v))).collect())
        .collect();
    match set.correlation {
        GenlinCorrelation::Independent => return Ok((m, phi, false)),
        GenlinCorrelation::Fixed(list) => {
            if list.len() != t * t.saturating_sub(1) / 2 {
                return Err(format!(
                    "GENLIN REPEATED: CORRTYPE=FIXED gives {} correlations; the working correlation matrix has dimension {t}, so it needs {}",
                    list.len(),
                    t * t.saturating_sub(1) / 2
                ));
            }
            // the lower triangle row by row
            let below = (1..t).flat_map(|u| (0..u).map(move |v| (u, v)));
            for ((u, v), value) in below.zip(list) {
                m[u][v] = *value;
                m[v][u] = *value;
            }
            let smallest = symmetric_eigen(&m)
                .iter()
                .map(|(v, _)| *v)
                .fold(f64::INFINITY, f64::min);
            if smallest <= 0.0 {
                return Err(
                    "GENLIN REPEATED: the FIXED working correlation matrix is not positive definite"
                        .into(),
                );
            }
            return Ok((m, phi, false));
        }
        GenlinCorrelation::Exchangeable => {
            let mut num = 0.0;
            let mut den = 0.0;
            for (row, (mm, f)) in grid.iter().zip(s.members.iter().zip(&freq)) {
                let n = mm.len() as f64;
                den += f * n * (n - 1.0) / 2.0;
                for u in 0..t {
                    for v in u + 1..t {
                        if let (Some(a), Some(b)) = (row[u], row[v]) {
                            num += f * a * b;
                        }
                    }
                }
            }
            let alpha = ratio(num, den);
            for (u, row) in m.iter_mut().enumerate() {
                for (v, cell) in row.iter_mut().enumerate() {
                    if u != v {
                        *cell = alpha;
                    }
                }
            }
        }
        GenlinCorrelation::Ar1 => {
            let estimate = if t > 1 { lag(1) } else { 0.0 };
            // a power matrix is positive definite only for |alpha| < 1
            let alpha = estimate.clamp(-0.99, 0.99);
            if alpha != estimate {
                return Ok((ar1(t, alpha), phi, true));
            }
            for (u, row) in m.iter_mut().enumerate() {
                for (v, cell) in row.iter_mut().enumerate() {
                    *cell = alpha.powi(u.abs_diff(v) as i32);
                }
            }
            return Ok((m, phi, false));
        }
        GenlinCorrelation::MDependent(order) => {
            let order = (*order as usize).min(t.saturating_sub(1));
            for j in 1..=order {
                let alpha = lag(j);
                for u in 0..t - j {
                    m[u][u + j] = alpha;
                    m[u + j][u] = alpha;
                }
            }
        }
        GenlinCorrelation::Unstructured => {
            for (u, v) in (0..t).flat_map(|u| (u + 1..t).map(move |v| (u, v))) {
                let (num, den) = pair(u, v);
                let alpha = ratio(num, den);
                m[u][v] = alpha;
                m[v][u] = alpha;
            }
        }
    }
    let smallest = symmetric_eigen(&m)
        .iter()
        .map(|(v, _)| *v)
        .fold(f64::INFINITY, f64::min);
    if smallest > 0.0 {
        return Ok((m, phi, false));
    }
    // the ridge that leaves 0.01 as the smallest eigenvalue
    let ridge = (0.01 - smallest) / 0.99;
    for (u, row) in m.iter_mut().enumerate() {
        for (v, cell) in row.iter_mut().enumerate() {
            *cell = (*cell + if u == v { ridge } else { 0.0 }) / (1.0 + ridge);
        }
    }
    Ok((m, phi, true))
}

fn ar1(t: usize, alpha: f64) -> Matrix {
    (0..t)
        .map(|u| (0..t).map(|v| alpha.powi(u.abs_diff(v) as i32)).collect())
        .collect()
}

/// The estimating equations s, H and the middle of the robust covariance at the means `mu`.
fn parts(
    c: &Cases,
    x: &[Vec<f64>],
    s: &Subjects,
    set: &Settings,
    mu: &[f64],
    r: &Matrix,
    phi: f64,
) -> (Vec<f64>, Matrix, Matrix) {
    let q = x.first().map_or(0, Vec::len);
    let mut score = vec![0.0; q];
    let mut h = vec![vec![0.0; q]; q];
    let mut meat = vec![vec![0.0; q]; q];
    // the inverse of R over the positions of a subject, once per pattern of positions
    let mut inverses: HashMap<Vec<usize>, Matrix> = HashMap::new();
    for m in &s.members {
        let n = m.len();
        let f = c.f[m[0].0];
        let pattern: Vec<usize> = m.iter().map(|&(_, u)| u).collect();
        let inverse = inverses.entry(pattern).or_insert_with(|| {
            let sub: Matrix = m
                .iter()
                .map(|&(_, u)| m.iter().map(|&(_, v)| r[u][v]).collect())
                .collect();
            ginverse(&sub, 1e-12).0
        });
        let root: Vec<f64> = m
            .iter()
            .map(|&(i, _)| (variance(set.family, mu[i], set.k) / c.a(i)).sqrt())
            .collect();
        // the rows of D
        let d: Vec<Vec<f64>> = m
            .iter()
            .map(|&(i, _)| {
                let g = 1.0 / link_d1(set.link, mu[i], set.k);
                x[i].iter().map(|v| v * g).collect()
            })
            .collect();
        let e: Vec<f64> = m.iter().map(|&(i, _)| c.y[i] - mu[i]).collect();
        let w = |a: usize, b: usize| inverse[a][b] / (phi * root[a] * root[b]);
        let mut g = vec![0.0; q];
        for a in 0..n {
            let mut we = 0.0;
            let mut wd = vec![0.0; q];
            for b in 0..n {
                let wab = w(a, b);
                we += wab * e[b];
                for (t, v) in wd.iter_mut().enumerate() {
                    *v += wab * d[b][t];
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
    (score, h, meat)
}

/// Fits the generalized estimating equations from `beta0` and the scale `phi0`.
pub(super) fn fit(
    c: &Cases,
    x: &[Vec<f64>],
    s: &Subjects,
    set: &Settings,
    beta0: &[f64],
    phi0: f64,
) -> Result<GeeFit, String> {
    let q = beta0.len();
    let failed =
        || "GENLIN REPEATED: the estimating equations give a case an invalid mean".to_string();
    let mut beta = beta0.to_vec();
    let mut mu = means(c, x, set, &beta).ok_or_else(failed)?;
    let (mut r, start_moment, mut ridged) = estimate(c, s, set, q, &mu)?;
    // with SCALE=MLE the scale is the Pearson moment from the start
    let mut phi = if set.update_scale { start_moment } else { phi0 };
    let (mut score, mut h, mut meat) = parts(c, x, s, set, &mu, &r, phi);
    let mut inverse = ginverse(&h, set.singular).0;
    let quadratic =
        |score: &[f64], inverse: &Matrix| super::dot(score, &mat_vec(inverse, score)).abs();
    let mut converged = set.maxiterations == 0 || q == 0;
    for iteration in 1..=set.maxiterations {
        if q == 0 {
            break;
        }
        // a step that gives a case an invalid mean is halved
        let step = mat_vec(&inverse, &score);
        let mut size = 1.0;
        let (next, next_mu) = loop {
            let next: Vec<f64> = beta.iter().zip(&step).map(|(b, d)| b + size * d).collect();
            if let Some(m) = means(c, x, set, &next) {
                break (next, m);
            }
            size /= 2.0;
            if size < 1e-9 {
                return Err(failed());
            }
        };
        mu = next_mu;
        let update = set.updatecorr > 0 && iteration % set.updatecorr == 0;
        if update {
            let (nr, m, nridged) = estimate(c, s, set, q, &mu)?;
            r = nr;
            ridged = nridged;
            if set.update_scale {
                phi = m;
            }
        } else if set.update_scale {
            phi = moment(c, s, set, q, &residuals(c, set, &mu));
        }
        let change = super::largest_change(&beta, &next, set.pconverge.relative);
        beta = next;
        (score, h, meat) = parts(c, x, s, set, &mu, &r, phi);
        inverse = ginverse(&h, set.singular).0;
        let p_met = set.pconverge.value == 0.0 || change < set.pconverge.value;
        let h_met = set.hconverge == 0.0 || quadratic(&score, &inverse) < set.hconverge;
        if p_met && h_met {
            converged = true;
            break;
        }
    }
    let hessian_ok = set.hconverge > 0.0
        || set.maxiterations == 0
        || q == 0
        || quadratic(&score, &inverse) < 1e-4;
    let model = inverse;
    let robust = multiply(&multiply(&model, &meat), &model);
    Ok(GeeFit {
        beta,
        mu,
        phi,
        r,
        information: h,
        model,
        robust,
        converged,
        hessian_ok,
        ridged,
    })
}

/// The quasi-likelihood of one case per unit of prior weight at scale 1: the kernel, or the
/// full form (the kernel less its value at mu = y).
pub(super) fn quasi(family: Family, y: f64, mu: f64, k: f64, full: bool) -> f64 {
    let ylog = |m: f64| if y == 0.0 { 0.0 } else { y * m.ln() };
    let kernel = |m: f64| match family {
        Family::Normal => -0.5 * (y - m) * (y - m),
        Family::Binomial => {
            ylog(m)
                + if y < 1.0 {
                    (1.0 - y) * (-m).ln_1p()
                } else {
                    0.0
                }
        }
        Family::Gamma => -(y / m + m.ln()),
        Family::InverseGaussian => -(y / (2.0 * m * m) - 1.0 / m),
        Family::NegBin if k > 0.0 => ylog(k * m) - (y + 1.0 / k) * (k * m).ln_1p(),
        Family::NegBin | Family::Poisson => ylog(m) - m,
    };
    if full && family != Family::Normal {
        kernel(mu) - kernel(y)
    } else {
        kernel(mu)
    }
}
