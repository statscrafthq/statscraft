//! The numerical core of PROXSCAL: least-squares multidimensional scaling by majorization
//! (the Guttman transform), with optimal transformations of the proximities and the
//! individual differences models.
//!
//! Rules from the Algorithms manual (PROXSCAL), in our words:
//! - The loss is the weighted sum over sources and pairs of the squared difference between
//!   the transformed proximity and the distance in the source's space X_k = Z A_k; Z is the
//!   common space and A_k the space weights (I for the identity model, diagonal for the
//!   weighted model, any p x p matrix for the generalized model, rank r for the reduced one).
//! - A missing proximity or weight gets weight 0. Similarities change sign and are shifted so
//!   the smallest is 0 (within each source, or over all with CONDITION=UNCONDITIONAL).
//! - Each iteration majorizes the loss at the current spaces, updates Z (the Guttman transform,
//!   relaxed as 2 Z_new - Z_old for the identity model with ACCELERATION), then the space
//!   weights, then the transformation: RATIO keeps the proximities, INTERVAL regresses the
//!   distances on them with a nonnegative intercept and slope, ORDINAL is a weighted monotone
//!   regression (ties kept tied, or untied with UNTIE), SPLINE a regression with nonnegative
//!   coefficients on a monotone spline basis of the proximities. The transformed proximities
//!   are normalized to a sum of weighted squares of n(n-1)/2 per source.
//! - Iterations stop when the normalized raw stress is at most MINSTRESS, falls by at most
//!   DIFFSTRESS, or after MAXITER.
//! - Starts: SIMPLEX (all objects equally far apart, one transform, reduced to p dimensions by
//!   alternating least squares), TORGERSON (classical scaling of the mean squared
//!   proximities), RANDOM (uniform coordinates). The start is centered and dilated to fit.
//! - From the largest number of dimensions down, each smaller solution starts from the most
//!   important dimensions of the one before.
//! - On output the identity model is turned to principal axes; the weighted model scales Z so
//!   every column has sum of squares n and orders the dimensions by the sum of squared
//!   weights; the generalized model makes Z'Z = nI. Each dimension's sign makes the first
//!   object's coordinate (the first that is not 0) positive; diagonal weights are positive.

use crate::matrix::{Matrix, inverse, multiply, symmetric_eigen, transpose};
use crate::mds_variables::{self as variables, Variables};
use crate::unfolding::{align_to, pin, pinned_step};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Model {
    Identity,
    Weighted,
    Generalized,
    Reduced(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Transform {
    Ratio,
    Interval,
    Ordinal { untie: bool },
    Spline { degree: usize, inknot: usize },
}

/// The proximities of every source, made ready for scaling.
#[derive(Debug, Clone)]
pub(crate) struct Problem {
    pub n: usize,
    /// Per source: the dissimilarities (similarities already turned), NaN where missing.
    pub delta: Vec<Matrix>,
    /// Per source: the weight of every pair, 0 where missing.
    pub w: Vec<Matrix>,
    pub transform: Transform,
    /// The groups of sources whose proximities are compared with each other.
    pub groups: Vec<Vec<usize>>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Criteria {
    pub max_iterations: usize,
    pub diff_stress: f64,
    pub min_stress: f64,
    pub acceleration: bool,
}

/// One solution.
#[derive(Debug, Clone)]
pub(crate) struct Solution {
    /// n x p common space.
    pub z: Matrix,
    /// Per source: the p x p space weights.
    pub a: Vec<Matrix>,
    /// Per source: the transformed proximities.
    pub dhat: Vec<Matrix>,
    /// Normalized raw stress after every iteration, the start first.
    pub history: Vec<f64>,
    /// With independent variables restricting the space: their transformed values and weights.
    pub variables: Option<variables::Fit>,
}

/// A restriction of the common space.
pub(crate) enum Restriction<'a> {
    /// Fixed coordinates, `None` where free.
    Coordinates(&'a [Vec<Option<f64>>]),
    /// The space is a weighted sum of independent variables; with the fit to start from, when
    /// the iterations go on from an earlier solution.
    Variables(&'a Variables, Option<variables::Fit>),
}

pub(crate) fn zeros(r: usize, c: usize) -> Matrix {
    vec![vec![0.0; c]; r]
}

pub(crate) fn identity(p: usize) -> Matrix {
    (0..p)
        .map(|i| (0..p).map(|j| f64::from(u8::from(i == j))).collect())
        .collect()
}

/// The space weights to start from: the identity, cut to its first r dimensions for the
/// reduced rank model so the start obeys the model.
pub(crate) fn start_weights(model: Model, p: usize) -> Matrix {
    let mut a = identity(p);
    if let Model::Reduced(r) = model {
        (r.min(p)..p).for_each(|d| a[d][d] = 0.0);
    }
    a
}

/// Euclidean distances between the rows of `x`.
pub(crate) fn distances(x: &Matrix) -> Matrix {
    let n = x.len();
    let mut d = zeros(n, n);
    for i in 0..n {
        for j in i + 1..n {
            let s: f64 = x[i].iter().zip(&x[j]).map(|(a, b)| (a - b).powi(2)).sum();
            d[i][j] = s.sqrt();
            d[j][i] = d[i][j];
        }
    }
    d
}

/// The pairs i < j of every source with a positive weight.
fn pairs(problem: &Problem, k: usize) -> impl Iterator<Item = (usize, usize)> + '_ {
    let n = problem.n;
    (0..n)
        .flat_map(move |i| (i + 1..n).map(move |j| (i, j)))
        .filter(move |&(i, j)| problem.w[k][i][j] > 0.0)
}

/// The sums the stress measures are made of.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Sums {
    /// sum of w dhat^2, w d^2, w dhat d, w dhat^4, w d^4, w dhat^2 d^2, w (d - mean d)^2
    pub eta_dhat: f64,
    pub eta_x: f64,
    pub rho: f64,
    pub eta4_dhat: f64,
    pub eta4_x: f64,
    pub rho2: f64,
    pub kappa: f64,
}

pub(crate) fn sums(problem: &Problem, dhat: &[Matrix], x: &[Matrix]) -> Sums {
    let mut s = Sums::default();
    let (mut wsum, mut dsum) = (0.0, 0.0);
    let d: Vec<Matrix> = x.iter().map(distances).collect();
    for k in 0..dhat.len() {
        for (i, j) in pairs(problem, k) {
            let (w, e, f) = (problem.w[k][i][j], dhat[k][i][j], d[k][i][j]);
            s.eta_dhat += w * e * e;
            s.eta_x += w * f * f;
            s.rho += w * e * f;
            s.eta4_dhat += w * e.powi(4);
            s.eta4_x += w * f.powi(4);
            s.rho2 += w * e * e * f * f;
            wsum += w;
            dsum += w * f;
        }
    }
    let mean = dsum / wsum.max(f64::MIN_POSITIVE);
    for (k, dk) in d.iter().enumerate() {
        s.kappa += pairs(problem, k)
            .map(|(i, j)| problem.w[k][i][j] * (dk[i][j] - mean).powi(2))
            .sum::<f64>();
    }
    s
}

/// Normalized raw stress with the configuration optimally dilated.
pub(crate) fn raw_stress(s: &Sums) -> f64 {
    if s.eta_dhat <= 0.0 || s.eta_x <= 0.0 {
        return 1.0;
    }
    (1.0 - s.rho * s.rho / (s.eta_dhat * s.eta_x)).max(0.0)
}

/// The stress and fit measures: normalized raw stress, Stress-I, Stress-II, S-Stress,
/// dispersion accounted for and Tucker's coefficient of congruence.
pub(crate) fn fit_measures(s: &Sums) -> [f64; 6] {
    let raw = raw_stress(s);
    let alpha = s.eta_dhat / s.rho;
    let misfit = s.eta_dhat + alpha * alpha * s.eta_x - 2.0 * alpha * s.rho;
    let stress1 = (misfit.max(0.0) / (alpha * alpha * s.eta_x)).sqrt();
    let stress2 = (misfit.max(0.0) / (alpha * alpha * s.kappa)).sqrt();
    let a2 = s.rho2 / s.eta4_x;
    let sstress = (s.eta4_dhat + a2 * a2 * s.eta4_x - 2.0 * a2 * s.rho2).max(0.0) / s.eta4_dhat;
    [
        raw,
        stress1,
        stress2,
        sstress,
        1.0 - raw,
        (1.0 - raw).sqrt(),
    ]
}

/// V = sum of the weights as a Laplacian: v_ij = -w_ij, v_ii = sum of row i.
pub(crate) fn laplacian(w: &Matrix) -> Matrix {
    let n = w.len();
    let mut v = zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            if i != j {
                v[i][j] = -w[i][j];
                v[i][i] += w[i][j];
            }
        }
    }
    v
}

/// The Moore-Penrose inverse of a Laplacian whose null space is the vector of ones:
/// (V + 11'/n)^-1 - 11'/n.
pub(crate) fn laplacian_inverse(v: &Matrix) -> Option<Matrix> {
    let n = v.len();
    let c = 1.0 / n as f64;
    let shifted: Matrix = v
        .iter()
        .map(|row| row.iter().map(|x| x + c).collect())
        .collect();
    let (inv, _) = inverse(&shifted)?;
    Some(
        inv.into_iter()
            .map(|row| row.into_iter().map(|x| x - c).collect())
            .collect(),
    )
}

/// B(X) X for one source: b_ij = -w_ij dhat_ij / d_ij(X) off the diagonal (0 where d is 0).
pub(crate) fn bx(w: &Matrix, dhat: &Matrix, x: &Matrix) -> Matrix {
    let n = x.len();
    let p = x.first().map_or(0, Vec::len);
    let d = distances(x);
    let mut b = zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            if i != j && d[i][j] > 0.0 && w[i][j] > 0.0 {
                b[i][j] = -w[i][j] * dhat[i][j] / d[i][j];
                b[i][i] -= b[i][j];
            }
        }
    }
    let mut out = zeros(n, p);
    for i in 0..n {
        for j in 0..n {
            if b[i][j] != 0.0 {
                for a in 0..p {
                    out[i][a] += b[i][j] * x[j][a];
                }
            }
        }
    }
    out
}

pub(crate) fn center(z: &mut Matrix) {
    let n = z.len() as f64;
    let p = z.first().map_or(0, Vec::len);
    for a in 0..p {
        let mean = z.iter().map(|r| r[a]).sum::<f64>() / n;
        z.iter_mut().for_each(|r| r[a] -= mean);
    }
}

fn scale(m: &mut Matrix, f: f64) {
    m.iter_mut().flatten().for_each(|v| *v *= f);
}

/// The space of every source.
pub(crate) fn spaces(z: &Matrix, a: &[Matrix]) -> Vec<Matrix> {
    a.iter().map(|ak| multiply(z, ak)).collect()
}

/// Weighted monotone (isotonic) regression by pooling adjacent violators.
pub(crate) fn monotone(values: &[f64], weights: &[f64]) -> Vec<f64> {
    // blocks of (mean, weight, length)
    let mut blocks: Vec<(f64, f64, usize)> = Vec::with_capacity(values.len());
    for (&v, &w) in values.iter().zip(weights) {
        blocks.push((v, w, 1));
        while blocks.len() > 1 {
            let (m2, w2, l2) = blocks[blocks.len() - 1];
            let (m1, w1, l1) = blocks[blocks.len() - 2];
            if m1 <= m2 {
                break;
            }
            blocks.pop();
            let w = w1 + w2;
            let m = if w > 0.0 {
                (m1 * w1 + m2 * w2) / w
            } else {
                (m1 + m2) / 2.0
            };
            *blocks.last_mut().expect("two blocks") = (m, w, l1 + l2);
        }
    }
    blocks
        .into_iter()
        .flat_map(|(m, _, l)| std::iter::repeat_n(m, l))
        .collect()
}

/// Nonnegative least squares, min |y - S b| over b >= 0, by Lawson and Hanson's active set.
pub(crate) fn nnls(s: &Matrix, y: &[f64]) -> Vec<f64> {
    let q = s.first().map_or(0, Vec::len);
    let mut b = vec![0.0; q];
    let mut passive = vec![false; q];
    let gradient = |b: &[f64]| -> Vec<f64> {
        let r: Vec<f64> = s
            .iter()
            .zip(y)
            .map(|(row, yi)| yi - row.iter().zip(b).map(|(a, c)| a * c).sum::<f64>())
            .collect();
        (0..q)
            .map(|j| s.iter().zip(&r).map(|(row, ri)| row[j] * ri).sum())
            .collect()
    };
    // least squares on the passive columns
    let solve = |passive: &[bool]| -> Vec<f64> {
        let cols: Vec<usize> = (0..q).filter(|&j| passive[j]).collect();
        let mut out = vec![0.0; q];
        if cols.is_empty() {
            return out;
        }
        let ata: Matrix = cols
            .iter()
            .map(|&a| {
                cols.iter()
                    .map(|&c| s.iter().map(|r| r[a] * r[c]).sum())
                    .collect()
            })
            .collect();
        let aty: Vec<f64> = cols
            .iter()
            .map(|&a| s.iter().zip(y).map(|(r, yi)| r[a] * yi).sum())
            .collect();
        if let Some((inv, _)) = inverse(&ata) {
            for (i, &a) in cols.iter().enumerate() {
                out[a] = inv[i].iter().zip(&aty).map(|(x, z)| x * z).sum();
            }
        }
        out
    };
    let scale: f64 = s.iter().flatten().map(|v| v * v).sum::<f64>().sqrt()
        + y.iter().map(|v| v * v).sum::<f64>().sqrt();
    let tol = 1e-12 * scale.max(1.0);
    for _ in 0..(3 * q + 10) {
        let g = gradient(&b);
        let Some(next) = (0..q)
            .filter(|&j| !passive[j] && g[j] > tol)
            .max_by(|&a, &c| g[a].total_cmp(&g[c]))
        else {
            break;
        };
        passive[next] = true;
        loop {
            let z = solve(&passive);
            if (0..q).filter(|&j| passive[j]).all(|j| z[j] > 0.0) {
                b = z;
                break;
            }
            let step = (0..q)
                .filter(|&j| passive[j] && z[j] <= 0.0)
                .map(|j| b[j] / (b[j] - z[j]))
                .fold(f64::INFINITY, f64::min);
            for j in 0..q {
                b[j] += step * (z[j] - b[j]);
                if passive[j] && b[j] <= tol {
                    passive[j] = false;
                    b[j] = 0.0;
                }
            }
        }
    }
    b
}

/// The B-spline basis of `degree` on the knots (with the boundary knots repeated) at x.
fn bsplines(x: f64, knots: &[f64], degree: usize) -> Vec<f64> {
    let k = knots.len() - degree - 1;
    let last = knots[knots.len() - 1];
    let mut b: Vec<f64> = (0..knots.len() - 1)
        .map(|i| {
            let inside = knots[i] <= x && x < knots[i + 1];
            // the right end belongs to the last nonempty interval
            let end = x == last && knots[i] < knots[i + 1] && knots[i + 1] == last;
            f64::from(u8::from(inside || end))
        })
        .collect();
    for d in 1..=degree {
        for i in 0..knots.len() - 1 - d {
            let left = match knots[i + d] - knots[i] {
                h if h > 0.0 => (x - knots[i]) / h * b[i],
                _ => 0.0,
            };
            let right = match knots[i + d + 1] - knots[i + 1] {
                h if h > 0.0 => (knots[i + d + 1] - x) / h * b[i + 1],
                _ => 0.0,
            };
            b[i] = left + right;
        }
    }
    b.truncate(k);
    b
}

/// The monotone spline basis: a column of ones (the intercept), then for every B-spline from
/// the second on the sum of it and all later ones, which rises from 0 at the smallest value
/// to 1 at the largest: 1 + `degree` + `inknot` columns. The interior knots sit at equally
/// spaced quantiles of the values. With nonnegative coefficients it holds every increasing
/// straight line, so it fits at least as well as INTERVAL.
pub(crate) fn spline_basis(values: &[f64], degree: usize, inknot: usize) -> Matrix {
    spline_rows(values, &spline_knots(values, degree, inknot), degree)
}

/// The knots of `spline_basis`: the smallest and largest value repeated `degree` + 1 times,
/// and `inknot` interior knots at equally spaced quantiles of the values.
pub(crate) fn spline_knots(values: &[f64], degree: usize, inknot: usize) -> Vec<f64> {
    let mut sorted: Vec<f64> = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let (lo, hi) = (sorted[0], sorted[sorted.len() - 1]);
    let mut knots = vec![lo; degree + 1];
    for t in 1..=inknot {
        let q = t as f64 / (inknot + 1) as f64 * (sorted.len() - 1) as f64;
        let (f, c) = (q.floor() as usize, q.ceil() as usize);
        let v = sorted[f] + (q - f as f64) * (sorted[c] - sorted[f]);
        knots.push(v.clamp(lo, hi));
    }
    knots.extend(std::iter::repeat_n(hi, degree + 1));
    knots
}

/// The rows of the monotone spline basis on `knots` at each of `points`.
pub(crate) fn spline_rows(points: &[f64], knots: &[f64], degree: usize) -> Matrix {
    points
        .iter()
        .map(|&x| {
            let b = bsplines(x, knots, degree);
            std::iter::once(1.0)
                .chain((1..b.len()).map(|j| b[j..].iter().sum()))
                .collect()
        })
        .collect()
}

/// The proximities turned and shifted as the transformation needs: similarities change sign;
/// INTERVAL and SPLINE (and similarities) start at 0 within each group.
pub(crate) fn prepare(problem: &mut Problem, similarities: bool) {
    for group in problem.groups.clone() {
        if similarities {
            for &k in &group {
                problem.delta[k].iter_mut().flatten().for_each(|v| *v = -*v);
            }
        }
        let shift = similarities
            || matches!(
                problem.transform,
                Transform::Interval | Transform::Spline { .. }
            );
        if shift {
            let min = group
                .iter()
                .flat_map(|&k| pairs(problem, k).map(move |(i, j)| (k, i, j)))
                .map(|(k, i, j)| problem.delta[k][i][j])
                .fold(f64::INFINITY, f64::min);
            if min.is_finite() {
                for &k in &group {
                    problem.delta[k]
                        .iter_mut()
                        .flatten()
                        .for_each(|v| *v -= min);
                }
            }
        }
    }
}

/// The entries (source, i, j) of a group with positive weight.
fn entries(problem: &Problem, group: &[usize]) -> Vec<(usize, usize, usize)> {
    group
        .iter()
        .flat_map(|&k| pairs(problem, k).map(move |(i, j)| (k, i, j)))
        .collect()
}

/// Scales the transformed proximities of a group to n(n-1)/2 weighted squares per source.
fn normalize(problem: &Problem, group: &[usize], dhat: &mut [Matrix]) {
    let n = problem.n as f64;
    let target = group.len() as f64 * n * (n - 1.0) / 2.0;
    let sum: f64 = entries(problem, group)
        .iter()
        .map(|&(k, i, j)| problem.w[k][i][j] * dhat[k][i][j].powi(2))
        .sum();
    if sum > 0.0 {
        let f = (target / sum).sqrt();
        for &k in group {
            scale(&mut dhat[k], f);
        }
    }
}

/// The starting transformed proximities: the proximities, their ranks for ORDINAL,
/// normalized.
pub(crate) fn initial_dhat(problem: &Problem) -> Vec<Matrix> {
    let n = problem.n;
    let mut dhat: Vec<Matrix> = problem.delta.clone();
    for group in &problem.groups {
        if let Transform::Ordinal { .. } = problem.transform {
            let mut all = entries(problem, group);
            all.sort_by(|a, b| {
                problem.delta[a.0][a.1][a.2].total_cmp(&problem.delta[b.0][b.1][b.2])
            });
            // ties share their mean rank
            let mut start = 0;
            while start < all.len() {
                let value = |e: &(usize, usize, usize)| problem.delta[e.0][e.1][e.2];
                let mut end = start + 1;
                while end < all.len() && value(&all[end]) == value(&all[start]) {
                    end += 1;
                }
                let rank = (start + end + 1) as f64 / 2.0;
                for &(k, i, j) in &all[start..end] {
                    dhat[k][i][j] = rank;
                }
                start = end;
            }
        }
        normalize(problem, group, &mut dhat);
    }
    for d in &mut dhat {
        for (i, j) in (0..n).flat_map(|i| (0..=i).map(move |j| (i, j))) {
            d[i][j] = if i == j { 0.0 } else { d[j][i] };
        }
    }
    dhat
}

/// The optimal transformation of every group for the distances of the spaces `x`.
fn transform(problem: &Problem, x: &[Matrix], dhat: &mut [Matrix], basis: &[Matrix]) {
    if problem.transform == Transform::Ratio {
        return;
    }
    let d: Vec<Matrix> = x.iter().map(distances).collect();
    for (g, group) in problem.groups.iter().enumerate() {
        let all = entries(problem, group);
        if all.is_empty() {
            continue;
        }
        let delta = |e: &(usize, usize, usize)| problem.delta[e.0][e.1][e.2];
        let dist = |e: &(usize, usize, usize)| d[e.0][e.1][e.2];
        let weight = |e: &(usize, usize, usize)| problem.w[e.0][e.1][e.2];
        let fitted: Vec<f64> = match problem.transform {
            Transform::Ratio => unreachable!("returned above"),
            Transform::Interval => {
                let wsum: f64 = all.iter().map(weight).sum();
                let mx = all.iter().map(|e| weight(e) * delta(e)).sum::<f64>() / wsum;
                let my = all.iter().map(|e| weight(e) * dist(e)).sum::<f64>() / wsum;
                let sxy: f64 = all
                    .iter()
                    .map(|e| weight(e) * (delta(e) - mx) * (dist(e) - my))
                    .sum();
                let sxx: f64 = all
                    .iter()
                    .map(|e| weight(e) * (delta(e) - mx).powi(2))
                    .sum();
                let (mut alpha, mut beta) = match sxx > 0.0 {
                    true => (my - sxy / sxx * mx, sxy / sxx),
                    false => (my, 0.0),
                };
                if beta < 0.0 {
                    (alpha, beta) = (my, 0.0);
                } else if alpha < 0.0 {
                    let xx: f64 = all.iter().map(|e| weight(e) * delta(e).powi(2)).sum();
                    let xy: f64 = all.iter().map(|e| weight(e) * delta(e) * dist(e)).sum();
                    (alpha, beta) = (0.0, if xx > 0.0 { xy / xx } else { 0.0 });
                }
                all.iter().map(|e| alpha + beta * delta(e)).collect()
            }
            Transform::Ordinal { untie } => {
                let mut order: Vec<usize> = (0..all.len()).collect();
                order.sort_by(|&a, &b| {
                    let by_delta = delta(&all[a]).total_cmp(&delta(&all[b]));
                    match untie {
                        true => by_delta.then(dist(&all[a]).total_cmp(&dist(&all[b]))),
                        false => by_delta,
                    }
                });
                // tie blocks: kept together under KEEPTIES, one entry each under UNTIE
                let mut blocks: Vec<Vec<usize>> = Vec::new();
                for &o in &order {
                    match blocks.last_mut() {
                        Some(block) if !untie && delta(&all[block[0]]) == delta(&all[o]) => {
                            block.push(o)
                        }
                        _ => blocks.push(vec![o]),
                    }
                }
                let (means, weights): (Vec<f64>, Vec<f64>) = blocks
                    .iter()
                    .map(|block| {
                        let w: f64 = block.iter().map(|&o| weight(&all[o])).sum();
                        let s: f64 = block.iter().map(|&o| weight(&all[o]) * dist(&all[o])).sum();
                        (if w > 0.0 { s / w } else { 0.0 }, w)
                    })
                    .unzip();
                let fit = monotone(&means, &weights);
                let mut out = vec![0.0; all.len()];
                for (block, value) in blocks.iter().zip(fit) {
                    for &o in block {
                        out[o] = value;
                    }
                }
                out
            }
            Transform::Spline { .. } => {
                let s = &basis[g];
                let root: Vec<f64> = all.iter().map(|e| weight(e).sqrt()).collect();
                let ws: Matrix = s
                    .iter()
                    .zip(&root)
                    .map(|(row, r)| row.iter().map(|v| v * r).collect())
                    .collect();
                let wy: Vec<f64> = all.iter().zip(&root).map(|(e, r)| dist(e) * r).collect();
                let b = nnls(&ws, &wy);
                s.iter()
                    .map(|row| row.iter().zip(&b).map(|(a, c)| a * c).sum())
                    .collect()
            }
        };
        if fitted.iter().all(|v| *v <= 0.0) {
            // a transformation of all zeros carries no information: keep the last one
            continue;
        }
        for (&(k, i, j), v) in all.iter().zip(fitted) {
            dhat[k][i][j] = v;
            dhat[k][j][i] = v;
        }
        normalize(problem, group, dhat);
    }
}

/// The spline basis of every group, over its entries in the order `entries` gives.
pub(crate) fn spline_bases(problem: &Problem) -> Vec<Matrix> {
    match problem.transform {
        Transform::Spline { degree, inknot } => problem
            .groups
            .iter()
            .map(|group| {
                let values: Vec<f64> = entries(problem, group)
                    .iter()
                    .map(|&(k, i, j)| problem.delta[k][i][j])
                    .collect();
                match values.is_empty() {
                    true => Matrix::new(),
                    false => spline_basis(&values, degree, inknot),
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Dilates z to fit the transformed proximities best, about the origin of the fixed
/// coordinates: only the dimensions with no fixed coordinate are centered. Returns the factor.
fn dilate_fixed(
    problem: &Problem,
    dhat: &[Matrix],
    z: &mut Matrix,
    a: &[Matrix],
    fixed: &[Vec<Option<f64>>],
) -> f64 {
    let p = z.first().map_or(0, Vec::len);
    for d in 0..p {
        if fixed.iter().all(|r| r[d].is_none()) {
            let mean = z.iter().map(|r| r[d]).sum::<f64>() / z.len() as f64;
            z.iter_mut().for_each(|r| r[d] -= mean);
        }
    }
    let s = sums(problem, dhat, &spaces(z, a));
    if s.eta_x > 0.0 && s.rho > 0.0 {
        let factor = s.rho / s.eta_x;
        scale(z, factor);
        return factor;
    }
    1.0
}

/// Centers z and dilates it to fit the transformed proximities best.
pub(crate) fn dilate(problem: &Problem, dhat: &[Matrix], z: &mut Matrix, a: &[Matrix]) {
    center(z);
    let s = sums(problem, dhat, &spaces(z, a));
    if s.eta_x > 0.0 && s.rho > 0.0 {
        scale(z, s.rho / s.eta_x);
    }
}

/// Classical scaling of the weighted mean squared transformed proximities.
pub(crate) fn torgerson(problem: &Problem, dhat: &[Matrix], p: usize) -> Matrix {
    let n = problem.n;
    let mut d2 = zeros(n, n);
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            let (mut s, mut w) = (0.0, 0.0);
            for (wk, dk) in problem.w.iter().zip(dhat) {
                s += wk[i][j] * dk[i][j].powi(2);
                w += wk[i][j];
            }
            d2[i][j] = if w > 0.0 { s / w } else { f64::NAN };
        }
    }
    // a pair missing in every source takes the mean of the others
    let known: Vec<f64> = (0..n)
        .flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j)))
        .map(|(i, j)| d2[i][j])
        .filter(|v| v.is_finite())
        .collect();
    let mean = known.iter().sum::<f64>() / known.len().max(1) as f64;
    d2.iter_mut()
        .flatten()
        .filter(|v| v.is_nan())
        .for_each(|v| *v = mean);
    let row: Vec<f64> = d2
        .iter()
        .map(|r| r.iter().sum::<f64>() / n as f64)
        .collect();
    let all = row.iter().sum::<f64>() / n as f64;
    let b: Matrix = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| -0.5 * (d2[i][j] - row[i] - row[j] + all))
                .collect()
        })
        .collect();
    let eigen = symmetric_eigen(&b);
    let mut z = zeros(n, p);
    for (a, (value, vector)) in eigen.iter().take(p).enumerate() {
        let root = value.max(0.0).sqrt();
        for i in 0..n {
            z[i][a] = vector[i] * root;
        }
    }
    z
}

/// The polar factor M (M'M)^-1/2 of an n x p matrix.
fn polar(m: &Matrix) -> Matrix {
    let mtm = multiply(&transpose(m), m);
    let eigen = symmetric_eigen(&mtm);
    let p = mtm.len();
    let mut root_inv = zeros(p, p);
    for (value, vector) in &eigen {
        if *value > 1e-14 * eigen[0].0.max(f64::MIN_POSITIVE) {
            for a in 0..p {
                for b in 0..p {
                    root_inv[a][b] += vector[a] * vector[b] / value.sqrt();
                }
            }
        }
    }
    multiply(m, &root_inv)
}

/// The simplex start: B(J) of all objects one distance apart, reduced to p dimensions.
pub(crate) fn simplex(problem: &Problem, dhat: &[Matrix], vinv: &Matrix, p: usize) -> Matrix {
    let n = problem.n;
    let m = dhat.len() as f64;
    let root2 = 2f64.sqrt();
    let mut b = zeros(n, n);
    for (wk, dk) in problem.w.iter().zip(dhat) {
        for i in 0..n {
            for j in 0..n {
                if i != j && wk[i][j] > 0.0 {
                    let v = -wk[i][j] * dk[i][j] / root2 / m;
                    b[i][j] += v;
                    b[i][i] -= v;
                }
            }
        }
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&x, &y| b[y][y].total_cmp(&b[x][x]));
    let mut h = zeros(n, p);
    for (a, &c) in order.iter().take(p).enumerate() {
        h[c][a] = 1.0;
    }
    let vb = multiply(vinv, &b);
    let mut z = zeros(n, p);
    for _ in 0..100 {
        let mut next = multiply(&vb, &h);
        scale(&mut next, 1.0 / root2);
        let change: f64 = next
            .iter()
            .flatten()
            .zip(z.iter().flatten())
            .map(|(a, c)| (a - c).powi(2))
            .sum();
        z = next;
        h = polar(&multiply(&b, &z));
        if change <= 1e-20 * z.iter().flatten().map(|v| v * v).sum::<f64>().max(1.0) {
            break;
        }
    }
    z
}

/// The mean Laplacian of the weights and its inverse.
pub(crate) fn mean_laplacian(problem: &Problem) -> Option<(Matrix, Matrix)> {
    let n = problem.n;
    let m = problem.w.len() as f64;
    let mut v = zeros(n, n);
    for w in &problem.w {
        let l = laplacian(w);
        for i in 0..n {
            for j in 0..n {
                v[i][j] += l[i][j] / m;
            }
        }
    }
    let vinv = laplacian_inverse(&v)?;
    Some((v, vinv))
}

/// Minimizes the loss from the start `z`; `a` holds the starting space weights.
pub(crate) fn iterate(
    problem: &Problem,
    model: Model,
    criteria: &Criteria,
    mut z: Matrix,
    mut a: Vec<Matrix>,
    mut dhat: Vec<Matrix>,
    restriction: Option<&Restriction>,
) -> Solution {
    let n = problem.n;
    let p = z.first().map_or(0, Vec::len);
    let m = problem.w.len();
    let basis = spline_bases(problem);
    let (v, vinv) = mean_laplacian(problem).expect("checked before");
    let laplacians: Vec<Matrix> = problem.w.iter().map(laplacian).collect();
    let mut scale = 1.0;
    let fixed = match restriction {
        Some(Restriction::Coordinates(f)) => Some(*f),
        _ => None,
    };
    let mut fit: Option<variables::Fit> = None;
    let vars = match restriction {
        Some(Restriction::Variables(vs, start)) => {
            fit = Some(match start {
                Some(f) => f.clone(),
                None => variables::start(vs, &z),
            });
            Some(*vs)
        }
        _ => None,
    };
    let lambda = vars.map_or(0.0, |_| variables::largest_eigenvalue(&v));
    // with one Laplacian for all sources, or identity weights, Z = V^- T S^-1 is exact
    let same_weights = model == Model::Identity || problem.w.windows(2).all(|w| w[0] == w[1]);
    match fixed {
        Some(fixed) => {
            align_to(&mut z, fixed);
            pin(&mut z, fixed);
            scale = dilate_fixed(problem, &dhat, &mut z, &a, fixed);
        }
        None => match fit.as_mut() {
            Some(f) => z = variables::dilate(problem, &dhat, f, &a),
            None => dilate(problem, &dhat, &mut z, &a),
        },
    }
    let mut x = spaces(&z, &a);
    let mut stress = raw_stress(&sums(problem, &dhat, &x));
    let mut history = vec![stress];
    let relaxed = criteria.acceleration && model == Model::Identity && restriction.is_none();
    for _ in 0..criteria.max_iterations {
        if stress <= criteria.min_stress {
            break;
        }
        let c: Vec<Matrix> = (0..m).map(|k| bx(&problem.w[k], &dhat[k], &x[k])).collect();
        let held: Option<Vec<Vec<Option<f64>>>> = fixed.map(|f| {
            f.iter()
                .map(|r| r.iter().map(|g| g.map(|g| g * scale)).collect())
                .collect()
        });
        let step = |relax: bool,
                    z: &Matrix,
                    a: &[Matrix]|
         -> (Matrix, Vec<Matrix>, Option<variables::Fit>) {
            let mut changed = fit.clone();
            let mut znew = match (&held, vars, changed.as_mut()) {
                (Some(held), _, _) => pinned_step(&v, &vinv, &c, z, held),
                (None, Some(vs), Some(f)) => {
                    let mut mean = zeros(n, p);
                    for ck in &c {
                        for i in 0..n {
                            for d in 0..p {
                                mean[i][d] += ck[i][d] / m as f64;
                            }
                        }
                    }
                    variables::step(f, vs, &v, lambda, &mean)
                }
                _ => update_z(&laplacians, &vinv, same_weights, &c, z, a),
            };
            if relax {
                for i in 0..n {
                    for d in 0..p {
                        znew[i][d] = 2.0 * znew[i][d] - z[i][d];
                    }
                }
            }
            let anew = match model {
                Model::Identity => a.to_vec(),
                _ => update_a(model, &laplacians, &c, &znew, a),
            };
            (znew, anew, changed)
        };
        let attempt = |relax: bool| {
            let (mut znew, anew, mut fitted) = step(relax, &z, &a);
            let mut xnew = spaces(&znew, &anew);
            let mut dnew = dhat.clone();
            transform(problem, &xnew, &mut dnew, &basis);
            let mut factor = 1.0;
            if let Some(fixed) = fixed {
                factor = dilate_fixed(problem, &dnew, &mut znew, &anew, fixed);
                xnew = spaces(&znew, &anew);
            }
            if let Some(f) = fitted.as_mut() {
                znew = variables::dilate(problem, &dnew, f, &anew);
                xnew = spaces(&znew, &anew);
            }
            let s = raw_stress(&sums(problem, &dnew, &xnew));
            (znew, anew, xnew, dnew, s, factor, fitted)
        };
        let mut next = attempt(relaxed);
        if relaxed && next.4 > stress {
            next = attempt(false);
        }
        let (znew, anew, xnew, dnew, s, factor, fitted) = next;
        scale *= factor;
        fit = fitted;
        let gain = stress - s;
        (z, a, x, dhat, stress) = (znew, anew, xnew, dnew, s);
        history.push(stress);
        if stress <= criteria.min_stress || gain <= criteria.diff_stress {
            break;
        }
    }
    if fixed.is_some() {
        // back to the fixed coordinates; the distances and the transformed proximities with them
        z.iter_mut().flatten().for_each(|v| *v /= scale);
        dhat.iter_mut().for_each(|d| self::scale(d, 1.0 / scale));
    }
    Solution {
        z,
        a,
        dhat,
        history,
        variables: fit,
    }
}

/// The Guttman transform of the common space for the space weights `a`.
pub(crate) fn update_z(
    laplacians: &[Matrix],
    vinv: &Matrix,
    same_weights: bool,
    c: &[Matrix],
    z: &Matrix,
    a: &[Matrix],
) -> Matrix {
    let n = z.len();
    let p = z.first().map_or(0, Vec::len);
    let m = c.len() as f64;
    // T = 1/m sum C_k A_k' and S = 1/m sum A_k A_k'
    let mut t = zeros(n, p);
    let mut s = zeros(p, p);
    for (ck, ak) in c.iter().zip(a) {
        let cat = multiply(ck, &transpose(ak));
        let aat = multiply(ak, &transpose(ak));
        for i in 0..n {
            for d in 0..p {
                t[i][d] += cat[i][d] / m;
            }
        }
        for d in 0..p {
            for e in 0..p {
                s[d][e] += aat[d][e] / m;
            }
        }
    }
    if same_weights {
        // V Z S = T, so Z = V^- T S^-1
        if let Some((sinv, _)) = inverse(&s) {
            return multiply(&multiply(vinv, &t), &sinv);
        }
        // directions no source weighs (the reduced rank model) keep their old coordinates,
        // which leaves every space as it is and lets the weights grow into them later
        let sinv = pseudo_inverse(&s);
        let mut keep = identity(p);
        let ssp = multiply(&s, &sinv);
        for d in 0..p {
            for e in 0..p {
                keep[d][e] -= ssp[d][e];
            }
        }
        let mut out = multiply(&multiply(vinv, &t), &sinv);
        let old = multiply(z, &keep);
        for (row, o) in out.iter_mut().zip(old) {
            row.iter_mut().zip(o).for_each(|(v, w)| *v += w);
        }
        return out;
    }
    // the whole system (sum A_k A_k' (x) V_k) vec Z = vec T, with the null space added
    let np = n * p;
    let mut h = zeros(np, np);
    let c1 = 1.0 / n as f64;
    for (vk, ak) in laplacians.iter().zip(a) {
        let aat = multiply(ak, &transpose(ak));
        for d in 0..p {
            for e in 0..p {
                if aat[d][e] == 0.0 {
                    continue;
                }
                for i in 0..n {
                    for j in 0..n {
                        h[d * n + i][e * n + j] += aat[d][e] * vk[i][j] / m;
                    }
                }
            }
        }
    }
    for d in 0..p {
        for i in 0..n {
            for j in 0..n {
                h[d * n + i][d * n + j] += c1;
            }
        }
    }
    let tv: Vec<f64> = (0..np).map(|r| t[r % n][r / n]).collect();
    let mut out = zeros(n, p);
    match inverse(&h) {
        Some((hinv, _)) => {
            for r in 0..np {
                out[r % n][r / n] = hinv[r].iter().zip(&tv).map(|(x, y)| x * y).sum();
            }
        }
        None => {
            // as above: the unweighted directions keep their old coordinates
            let hinv = pseudo_inverse(&h);
            let zv: Vec<f64> = (0..np).map(|r| z[r % n][r / n]).collect();
            let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
            let hz: Vec<f64> = h.iter().map(|row| dot(row, &zv)).collect();
            for r in 0..np {
                out[r % n][r / n] = dot(&hinv[r], &tv) + zv[r] - dot(&hinv[r], &hz);
            }
        }
    }
    center(&mut out);
    out
}

/// The Moore-Penrose inverse of a symmetric matrix that is not of full rank.
pub(crate) fn pseudo_inverse(a: &Matrix) -> Matrix {
    let p = a.len();
    let eigen = symmetric_eigen(a);
    let top = eigen.first().map_or(0.0, |e| e.0.abs());
    let mut out = zeros(p, p);
    for (value, vector) in eigen.iter().filter(|e| e.0.abs() > 1e-12 * top) {
        for i in 0..p {
            for j in 0..p {
                out[i][j] += vector[i] * vector[j] / value;
            }
        }
    }
    out
}

/// The space weights for the common space `z`.
pub(crate) fn update_a(
    model: Model,
    laplacians: &[Matrix],
    c: &[Matrix],
    z: &Matrix,
    a: &[Matrix],
) -> Vec<Matrix> {
    let p = z.first().map_or(0, Vec::len);
    let zt = transpose(z);
    laplacians
        .iter()
        .zip(c)
        .zip(a)
        .map(|((vk, ck), old)| {
            let zvz = multiply(&multiply(&zt, vk), z);
            let zc = multiply(&zt, ck);
            match model {
                Model::Identity => old.clone(),
                Model::Weighted => {
                    let mut out = zeros(p, p);
                    for d in 0..p {
                        out[d][d] = match zvz[d][d] > 0.0 {
                            true => zc[d][d] / zvz[d][d],
                            false => old[d][d],
                        };
                    }
                    out
                }
                Model::Generalized | Model::Reduced(_) => {
                    // a dimension of Z that is all 0 gets no weight
                    let inv = inverse(&zvz).map_or_else(|| pseudo_inverse(&zvz), |(inv, _)| inv);
                    let full = multiply(&inv, &zc);
                    match model {
                        // the best rank r in the metric of Z'V_kZ = L L': L' A of rank r
                        Model::Reduced(r) if r < p => match cholesky(&zvz) {
                            Some(l) => {
                                let lt = transpose(&l);
                                match inverse(&lt) {
                                    Some((ltinv, _)) => {
                                        multiply(&ltinv, &low_rank(&multiply(&lt, &full), r))
                                    }
                                    None => low_rank(&full, r),
                                }
                            }
                            None => low_rank(&full, r),
                        },
                        _ => full,
                    }
                }
            }
        })
        .collect()
}

/// The best rank-r approximation of a square matrix.
fn low_rank(a: &Matrix, r: usize) -> Matrix {
    let p = a.len();
    let ata = multiply(&transpose(a), a);
    let eigen = symmetric_eigen(&ata);
    // A Q_r Q_r' keeps the r largest singular values
    let mut proj = zeros(p, p);
    for (_, q) in eigen.iter().take(r) {
        for i in 0..p {
            for j in 0..p {
                proj[i][j] += q[i] * q[j];
            }
        }
    }
    multiply(a, &proj)
}

/// The rotation that turns z to principal axes, largest variance first.
pub(crate) fn principal_rotation(z: &Matrix) -> Matrix {
    let ztz = multiply(&transpose(z), z);
    let eigen = symmetric_eigen(&ztz);
    let p = ztz.len();
    (0..p)
        .map(|i| (0..p).map(|a| eigen[a].1[i]).collect())
        .collect()
}

/// z turned to principal axes, largest variance first.
pub(crate) fn principal_axes(z: &Matrix) -> Matrix {
    multiply(z, &principal_rotation(z))
}

/// Turns each dimension so the first object with a coordinate other than 0 is positive;
/// returns the signs.
pub(crate) fn first_positive(z: &mut Matrix) -> Vec<f64> {
    let p = z.first().map_or(0, Vec::len);
    let scale = z.iter().flatten().fold(0.0_f64, |s, v| s.max(v.abs()));
    (0..p)
        .map(|d| {
            let first = z.iter().map(|r| r[d]).find(|v| v.abs() > 1e-10 * scale);
            let sign = match first {
                Some(v) if v < 0.0 => -1.0,
                _ => 1.0,
            };
            z.iter_mut().for_each(|r| r[d] *= sign);
            sign
        })
        .collect()
}

/// The solution in its output form (see the module notes).
pub(crate) fn finish(model: Model, mut s: Solution) -> Solution {
    let n = s.z.len() as f64;
    let p = s.z.first().map_or(0, Vec::len);
    center(&mut s.z);
    match model {
        Model::Identity => {
            s.z = principal_axes(&s.z);
            first_positive(&mut s.z);
        }
        Model::Weighted => {
            // columns of Z to sum of squares n, the weights the other way
            for d in 0..p {
                let ss: f64 = s.z.iter().map(|r| r[d] * r[d]).sum();
                if ss > 0.0 {
                    let f = (n / ss).sqrt();
                    s.z.iter_mut().for_each(|r| r[d] *= f);
                    s.a.iter_mut().for_each(|a| a[d][d] /= f);
                }
            }
            s.a.iter_mut()
                .for_each(|a| (0..p).for_each(|d| a[d][d] = a[d][d].abs()));
            let importance: Vec<f64> = (0..p)
                .map(|d| s.a.iter().map(|a| a[d][d] * a[d][d]).sum())
                .collect();
            let mut order: Vec<usize> = (0..p).collect();
            order.sort_by(|&x, &y| importance[y].total_cmp(&importance[x]));
            s.z =
                s.z.iter()
                    .map(|r| order.iter().map(|&d| r[d]).collect())
                    .collect();
            s.a =
                s.a.iter()
                    .map(|a| {
                        let mut out = zeros(p, p);
                        for (to, &from) in order.iter().enumerate() {
                            out[to][to] = a[from][from];
                        }
                        out
                    })
                    .collect();
            first_positive(&mut s.z);
        }
        Model::Generalized | Model::Reduced(_) => {
            // Z'Z = L L'; Z sqrt(n) L'^-1 and A_k = L' A_k / sqrt(n)
            let ztz = multiply(&transpose(&s.z), &s.z);
            if let Some(l) = cholesky(&ztz)
                && let Some((linv, _)) = inverse(&transpose(&l))
            {
                let root = n.sqrt();
                s.z = multiply(&s.z, &linv);
                scale(&mut s.z, root);
                let lt = transpose(&l);
                s.a =
                    s.a.iter()
                        .map(|a| {
                            let mut out = multiply(&lt, a);
                            scale(&mut out, 1.0 / root);
                            out
                        })
                        .collect();
            }
            let signs = first_positive(&mut s.z);
            for a in &mut s.a {
                for (d, sign) in signs.iter().enumerate() {
                    a[d].iter_mut().for_each(|v| *v *= sign);
                }
            }
        }
    }
    s
}

/// The lower triangular L with L L' = a, None unless a is positive definite.
fn cholesky(a: &Matrix) -> Option<Matrix> {
    let p = a.len();
    let mut l = zeros(p, p);
    for i in 0..p {
        for j in 0..=i {
            let s: f64 = (0..j).map(|k| l[i][k] * l[j][k]).sum();
            if i == j {
                let v = a[i][i] - s;
                if v <= 0.0 {
                    return None;
                }
                l[i][i] = v.sqrt();
            } else {
                l[i][j] = (a[i][j] - s) / l[j][j];
            }
        }
    }
    Some(l)
}

/// The start of the next smaller dimensionality: the most important p - 1 dimensions.
pub(crate) fn reduce(model: Model, s: &Solution) -> (Matrix, Vec<Matrix>) {
    let p = s.z.first().map_or(0, Vec::len);
    match model {
        Model::Weighted => {
            // the spread of each dimension in the sources' spaces, whatever the scale of Z
            let importance: Vec<f64> = (0..p)
                .map(|d| {
                    let zz: f64 = s.z.iter().map(|r| r[d] * r[d]).sum();
                    s.a.iter().map(|a| a[d][d] * a[d][d]).sum::<f64>() * zz
                })
                .collect();
            let mut order: Vec<usize> = (0..p).collect();
            order.sort_by(|&x, &y| importance[y].total_cmp(&importance[x]));
            order.truncate(p - 1);
            let z =
                s.z.iter()
                    .map(|r| order.iter().map(|&d| r[d]).collect())
                    .collect();
            let a =
                s.a.iter()
                    .map(|a| {
                        let mut out = zeros(p - 1, p - 1);
                        for (to, &from) in order.iter().enumerate() {
                            out[to][to] = a[from][from];
                        }
                        out
                    })
                    .collect();
            (z, a)
        }
        _ => {
            // the spaces' principal axes for the identity model; the common space's for the
            // others, which restart with identity weights
            let z: Matrix = principal_axes(&s.z)
                .into_iter()
                .map(|mut r| {
                    r.truncate(p - 1);
                    r
                })
                .collect();
            (z, vec![start_weights(model, p - 1); s.a.len()])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem(delta: Matrix, transform: Transform) -> Problem {
        let n = delta.len();
        let w: Matrix = (0..n)
            .map(|i| (0..n).map(|j| f64::from(u8::from(i != j))).collect())
            .collect();
        Problem {
            n,
            delta: vec![delta],
            w: vec![w],
            transform,
            groups: vec![vec![0]],
        }
    }

    #[test]
    fn monotone_regression_pools_violators() {
        let fit = monotone(&[1.0, 3.0, 2.0, 4.0], &[1.0, 1.0, 1.0, 1.0]);
        assert_eq!(fit, [1.0, 2.5, 2.5, 4.0]);
        let fit = monotone(&[3.0, 1.0], &[1.0, 3.0]);
        assert_eq!(fit, [1.5, 1.5]);
    }

    #[test]
    fn nonnegative_least_squares() {
        // y = 2 x1 - 1 x2 + noise-free: the negative coefficient is held at 0
        let s = vec![vec![1.0, 0.0], vec![0.0, 1.0], vec![1.0, 1.0]];
        let b = nnls(&s, &[2.0, -1.0, 1.0]);
        assert!((b[0] - 1.5).abs() < 1e-12 && b[1] == 0.0, "{b:?}");
        let b = nnls(&s, &[2.0, 1.0, 3.0]);
        assert!(
            (b[0] - 2.0).abs() < 1e-12 && (b[1] - 1.0).abs() < 1e-12,
            "{b:?}"
        );
    }

    #[test]
    fn spline_basis_rises_from_zero_to_one() {
        let values: Vec<f64> = (0..11).map(f64::from).collect();
        let s = spline_basis(&values, 2, 1);
        assert_eq!(s[0].len(), 4);
        assert!(s.iter().all(|r| r[0] == 1.0));
        assert!(s[0][1..].iter().all(|v| v.abs() < 1e-12));
        assert!(s[10].iter().all(|v| (v - 1.0).abs() < 1e-12));
        for pair in s.windows(2) {
            assert!(pair[1].iter().zip(&pair[0]).all(|(b, a)| b >= &(a - 1e-12)));
        }
    }

    #[test]
    fn exact_euclidean_distances_are_recovered_with_stress_zero() {
        let points = [
            [0.0, 0.0],
            [3.0, 0.0],
            [0.0, 4.0],
            [3.0, 4.0],
            [1.0, 2.0],
            [5.0, 1.0],
        ];
        let x: Matrix = points.iter().map(|p| p.to_vec()).collect();
        let delta = distances(&x);
        for transform in [
            Transform::Ratio,
            Transform::Interval,
            Transform::Ordinal { untie: false },
        ] {
            let mut prob = problem(delta.clone(), transform);
            prepare(&mut prob, false);
            let dhat = initial_dhat(&prob);
            let z = torgerson(&prob, &dhat, 2);
            let criteria = Criteria {
                max_iterations: 500,
                diff_stress: 1e-14,
                min_stress: 1e-14,
                acceleration: true,
            };
            let s = iterate(
                &prob,
                Model::Identity,
                &criteria,
                z,
                vec![identity(2)],
                dhat,
                None,
            );
            let stress = *s.history.last().unwrap();
            assert!(stress < 1e-10, "{transform:?}: {stress}");
            if transform != Transform::Ratio {
                continue;
            }
            // the distances are proportional to the true ones
            let d = distances(&s.z);
            let ratio = d[0][1] / delta[0][1];
            for i in 0..6 {
                for j in i + 1..6 {
                    assert!(
                        (d[i][j] / delta[i][j] - ratio).abs() < 1e-4,
                        "{transform:?}"
                    );
                }
            }
        }
    }
}
