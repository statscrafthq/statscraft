//! The generalized Euclidean model of ALSCAL (GEMSCAL): every matrix k looks at the group space
//! through its own metric M_k, symmetric and of rank q (the number of directions),
//! d(i,j,k)^2 = (x_i - x_j)' M_k (x_i - x_j) with M_k = T_k diag(w_k) T_k', T_k holding q
//! directions (unit vectors) of the space and w_k their weights. INDSCAL is the case of the axes
//! as directions.
//!
//! The manual names the model, the directions and the weights but gives no formula. Choices:
//! - Each iteration takes the least squares metric of every matrix from its squared disparities
//!   (linear in the entries of M_k), then keeps its q largest eigenvalues, the positive ones
//!   unless NEGATIVE (which takes the q largest in size): M_k is of that rank and its
//!   eigenvectors are the directions. The coordinates are then moved one at a time as in the
//!   other models, the squared distance being a quadratic form.
//! - The start is the metric of the axes (M_k the identity) or the starting weights of a file as
//!   the diagonal of M_k.
//! - The output turns the group space to principal axes of the coordinates, gives each a sum of
//!   squares equal to the number of stimuli (the metrics follow) and signs it so that the first
//!   stimulus is positive. The directions of a matrix come ordered by weight, each signed so that
//!   its largest component is positive.

use super::{Data, cubic_roots};
use crate::matrix::{Matrix, inverse, multiply, symmetric_eigen, transpose};
use crate::mds::{first_positive, principal_rotation, zeros};

/// Squared distances of the model for every matrix.
pub(super) fn squared(x: &Matrix, metrics: &[Matrix]) -> Vec<Matrix> {
    let n = x.len();
    metrics
        .iter()
        .map(|m| {
            let mut e = zeros(n, n);
            for i in 0..n {
                for j in (0..n).filter(|&j| j != i) {
                    e[i][j] = quadratic(m, &x[i], &x[j]);
                }
            }
            e
        })
        .collect()
}

/// (a - b)' m (a - b).
fn quadratic(m: &Matrix, a: &[f64], b: &[f64]) -> f64 {
    let p = a.len();
    let mut total = 0.0;
    for r in 0..p {
        for c in 0..p {
            total += m[r][c] * (a[r] - b[r]) * (a[c] - b[c]);
        }
    }
    total
}

/// The metric of rank at most `q` nearest to the symmetric `m`: its largest eigenvalues, the
/// positive ones unless `negative`, with their eigenvectors; None when there is none.
fn project(m: &Matrix, q: usize, negative: bool) -> Option<Matrix> {
    let p = m.len();
    let mut eigen = symmetric_eigen(m);
    if negative {
        eigen.sort_by(|a, b| b.0.abs().total_cmp(&a.0.abs()));
    }
    let kept: Vec<&(f64, Vec<f64>)> = eigen
        .iter()
        .filter(|(value, _)| negative || *value > 0.0)
        .take(q)
        .collect();
    if kept.is_empty() {
        return None;
    }
    let mut out = zeros(p, p);
    for (value, vector) in kept {
        for r in 0..p {
            for c in 0..p {
                out[r][c] += value * vector[r] * vector[c];
            }
        }
    }
    Some(out)
}

/// The least squares metric of every matrix, then cut to q directions.
pub(super) fn update_metrics(
    data: &Data,
    x: &Matrix,
    f: &[Vec<f64>],
    q: usize,
    negative: bool,
    old: &[Matrix],
) -> Vec<Matrix> {
    let p = x.first().map_or(0, Vec::len);
    let entries: Vec<(usize, usize)> = (0..p).flat_map(|a| (a..p).map(move |b| (a, b))).collect();
    (0..old.len())
        .map(|k| {
            if data.cells[k].len() < entries.len() {
                return old[k].clone();
            }
            let rows: Vec<(Vec<f64>, f64)> = data.cells[k]
                .iter()
                .zip(&f[k])
                .map(|(cell, target)| {
                    let diff: Vec<f64> = (0..p).map(|a| x[cell.a][a] - x[cell.b][a]).collect();
                    let row = entries
                        .iter()
                        .map(|&(a, b)| match a == b {
                            true => diff[a] * diff[a],
                            false => 2.0 * diff[a] * diff[b],
                        })
                        .collect();
                    (row, *target)
                })
                .collect();
            let size = entries.len();
            let ptp: Matrix = (0..size)
                .map(|a| {
                    (0..size)
                        .map(|b| rows.iter().map(|r| r.0[a] * r.0[b]).sum())
                        .collect()
                })
                .collect();
            let pty: Vec<f64> = (0..size)
                .map(|a| rows.iter().map(|r| r.0[a] * r.1).sum())
                .collect();
            let Some((inv, _)) = inverse(&ptp) else {
                return old[k].clone();
            };
            let theta: Vec<f64> = inv
                .iter()
                .map(|r| r.iter().zip(&pty).map(|(a, b)| a * b).sum())
                .collect();
            let mut m = zeros(p, p);
            for (&(a, b), t) in entries.iter().zip(theta) {
                m[a][b] = t;
                m[b][a] = t;
            }
            project(&m, q, negative).unwrap_or_else(|| old[k].clone())
        })
        .collect()
}

/// Every coordinate in turn at the minimum of S-stress with the others held; the squared
/// distance of a pair is a quadratic form, so for one coordinate t of point l the squared
/// distance is m u^2 + 2 s u + r with u = t - x_j and the derivative of S-stress a cubic.
pub(super) fn update_coordinates(data: &Data, x: &mut Matrix, metrics: &[Matrix], f: &[Vec<f64>]) {
    let n = data.n;
    let p = x.first().map_or(0, Vec::len);
    let c: Vec<f64> = (0..metrics.len())
        .map(|k| {
            let s: f64 = f[k].iter().map(|v| v.powi(2)).sum();
            if s > 0.0 { 1.0 / s } else { 0.0 }
        })
        .collect();
    // (weight of the pair's subject, m, s, r - f, x_j)
    let mut terms: Vec<(f64, f64, f64, f64, f64)> = Vec::new();
    for l in (0..n).filter(|&l| !data.fixed.points[l]) {
        for _ in 0..20 {
            let mut change: f64 = 0.0;
            for a in 0..p {
                terms.clear();
                for (k, metric) in metrics.iter().enumerate().filter(|&(k, _)| c[k] != 0.0) {
                    for &(cell, j) in &data.incident[k][l] {
                        let diff: Vec<f64> = (0..p).map(|b| x[l][b] - x[j][b]).collect();
                        let m = metric[a][a];
                        let s: f64 = (0..p)
                            .filter(|&b| b != a)
                            .map(|b| metric[a][b] * diff[b])
                            .sum();
                        let mut r = 0.0;
                        for b in (0..p).filter(|&b| b != a) {
                            for d in (0..p).filter(|&d| d != a) {
                                r += metric[b][d] * diff[b] * diff[d];
                            }
                        }
                        terms.push((c[k], m, s, r - f[k][cell], x[j][a]));
                    }
                }
                let value = |t: f64| -> f64 {
                    terms
                        .iter()
                        .map(|&(ck, m, s, rest, xj)| {
                            let u = t - xj;
                            ck * (m * u * u + 2.0 * s * u + rest).powi(2)
                        })
                        .sum()
                };
                // derivative / 4: sum c (m^2 u^3 + 3 m s u^2 + (2 s^2 + m rest) u + s rest)
                let (mut c3, mut c2, mut c1, mut c0) = (0.0, 0.0, 0.0, 0.0);
                for &(ck, m, s, rest, xj) in &terms {
                    let (k3, k2, k1, k0) = (m * m, 3.0 * m * s, 2.0 * s * s + m * rest, s * rest);
                    // in t: u = t - xj
                    c3 += ck * k3;
                    c2 += ck * (k2 - 3.0 * xj * k3);
                    c1 += ck * (k1 - 2.0 * xj * k2 + 3.0 * xj * xj * k3);
                    c0 += ck * (k0 - xj * k1 + xj * xj * k2 - xj.powi(3) * k3);
                }
                if c3 == 0.0 {
                    continue;
                }
                let old = x[l][a];
                let best = cubic_roots(c3, c2, c1, c0)
                    .into_iter()
                    .filter(|t| t.is_finite())
                    .chain(std::iter::once(old))
                    .min_by(|s, t| value(*s).total_cmp(&value(*t)))
                    .unwrap_or(old);
                change = change.max((best - old).abs());
                x[l][a] = best;
            }
            let size = x
                .iter()
                .flatten()
                .fold(0.0_f64, |s, v| s.max(v.abs()))
                .max(f64::MIN_POSITIVE);
            if change <= 1e-12 * size {
                break;
            }
        }
    }
}

/// The coordinates centered, turned to principal axes, given a sum of squares of n per
/// dimension and signed, with the metrics following.
fn turned(mut x: Matrix, metrics: &[Matrix]) -> (Matrix, Vec<Matrix>) {
    let n = x.len();
    let p = x.first().map_or(0, Vec::len);
    for a in 0..p {
        let mean = x.iter().map(|r| r[a]).sum::<f64>() / n as f64;
        x.iter_mut().for_each(|r| r[a] -= mean);
    }
    let turn = principal_rotation(&x);
    x = multiply(&x, &turn);
    let mut metrics: Vec<Matrix> = metrics
        .iter()
        .map(|m| multiply(&multiply(&transpose(&turn), m), &turn))
        .collect();
    // the scale of every dimension: a sum of squares of n
    let scale: Vec<f64> = (0..p)
        .map(|a| {
            let ss: f64 = x.iter().map(|r| r[a] * r[a]).sum();
            if ss > 0.0 {
                (n as f64 / ss).sqrt()
            } else {
                1.0
            }
        })
        .collect();
    for r in &mut x {
        for a in 0..p {
            r[a] *= scale[a];
        }
    }
    for m in &mut metrics {
        for a in 0..p {
            for b in 0..p {
                m[a][b] /= scale[a] * scale[b];
            }
        }
    }
    let signs = first_positive(&mut x);
    for m in &mut metrics {
        for a in 0..p {
            for b in 0..p {
                m[a][b] *= signs[a] * signs[b];
            }
        }
    }
    (x, metrics)
}

/// The solution in its output form: the coordinates turned, scaled and signed as the module
/// notes say, and the directions and weights of every matrix.
pub(super) struct Solution {
    pub x: Matrix,
    /// Per matrix its weights, largest first, q of them.
    pub weights: Vec<Vec<f64>>,
    /// Per matrix its directions as the columns of a p by q matrix.
    pub directions: Vec<Matrix>,
}

pub(super) fn finish(
    mut x: Matrix,
    metrics: &[Matrix],
    q: usize,
    negative: bool,
    fixed: bool,
) -> Solution {
    let p = x.first().map_or(0, Vec::len);
    let mut metrics: Vec<Matrix> = metrics.to_vec();
    if !fixed {
        (x, metrics) = turned(x, &metrics);
    }
    let mut weights = Vec::new();
    let mut directions = Vec::new();
    for m in &metrics {
        let mut eigen = symmetric_eigen(m);
        if negative {
            eigen.sort_by(|a, b| b.0.abs().total_cmp(&a.0.abs()));
        }
        let mut w = vec![0.0; q];
        let mut t = zeros(p, q);
        for (b, (value, vector)) in eigen.iter().take(q).enumerate() {
            if !negative && *value <= 0.0 {
                break;
            }
            w[b] = *value;
            // the largest component positive, so that the sign is not arbitrary
            let big = vector
                .iter()
                .copied()
                .fold(0.0_f64, |s, v| if v.abs() > s.abs() { v } else { s });
            let sign = if big < 0.0 { -1.0 } else { 1.0 };
            for a in 0..p {
                t[a][b] = sign * vector[a];
            }
        }
        weights.push(w);
        directions.push(t);
    }
    Solution {
        x,
        weights,
        directions,
    }
}
