//! ALSCAL: multidimensional scaling by alternating least squares on squared distances, for
//! symmetric matrices with the Euclidean and the weighted Euclidean (INDSCAL) models.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (ALSCAL), in our words:
//! - Each case is a row of a square matrix (the triangle below the diagonal is used); more
//!   cases than variables hold one matrix per subject. A value below CUTOFF (0) or missing is
//!   left out. The weight variable is not used.
//! - The start: each subject's proximities get the smallest constant that makes them
//!   nonnegative and meet every triangle inequality, are squared and double centered into
//!   scalar products, scaled to a root mean square of 1, averaged over the subjects, and the
//!   coordinates are the main eigenvectors times the roots of their eigenvalues. A missing
//!   value takes the mean of its subject for the start.
//! - Each iteration: the squared distances of the model (with the subject's dimension weights
//!   for INDSCAL); the optimal scaling of the data to them, within each matrix or over all
//!   (UNCONDITIONAL); the squared disparities rescaled per subject to fit the squared
//!   distances best; S-stress, the root of the mean over subjects of the sum of squared
//!   differences of squared distances and squared disparities over the sum of the fourth
//!   powers of the disparities; for INDSCAL the weights by least squares (0 for a negative one
//!   unless NEGATIVE); then every coordinate in turn at the minimum of S-stress with the others
//!   held, each point's coordinates repeated until they settle.
//! - It stops when S-stress improves by less than CONVERGE (not after the first iteration), is
//!   at most STRESSMIN, or after ITER iterations.
//! - ORDINAL is a monotone regression on the order of the data, ties kept tied unless UNTIE,
//!   the order turned for SIMILAR; NOMINAL gives equal values equal disparities (their mean).
//!   INTERVAL(n) and RATIO(n) make the squared disparities a polynomial of degree n of the
//!   squared data, with and without an intercept, fitted by least squares to the squared
//!   distances (so RATIO(1) keeps the disparities proportional to the data).
//! - Fit per matrix: Kruskal's stress formula 1 of the distances and the disparities, and RSQ,
//!   their squared correlation; averaged as the root of the mean square and the mean.

use crate::chart::{Axis, ChartKind};
use crate::descriptives::number;
use crate::matrix::{Matrix, inverse, symmetric_eigen};
use crate::mds::{first_positive, monotone, principal_axes, zeros};
use crate::npar_tests::text;
use crate::plot::{Plot, Point, PointGroup};
use crate::proxscal::{Reading, coordinates, space_chart, square};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_data::Dataset;
use statscraft_syntax::{Alscal, AlscalLevel, ProxShape};

/// The data of the scaling: per subject the proximities and which pairs count.
struct Data {
    n: usize,
    o: Vec<Matrix>,
    present: Vec<Vec<Vec<bool>>>,
    level: AlscalLevel,
    groups: Vec<Vec<usize>>,
}

struct Fit {
    x: Matrix,
    /// Per subject the weight of every dimension (all 1 for EUCLID).
    w: Vec<Vec<f64>>,
    /// Squared disparities.
    f: Vec<Matrix>,
    history: Vec<f64>,
}

fn pairs(n: usize) -> impl Iterator<Item = (usize, usize)> {
    (0..n).flat_map(move |i| (0..i).map(move |j| (i, j)))
}

/// Squared distances of the model for every subject.
fn squared(x: &Matrix, w: &[Vec<f64>]) -> Vec<Matrix> {
    let n = x.len();
    w.iter()
        .map(|wk| {
            let mut e = zeros(n, n);
            for (i, j) in pairs(n) {
                let v: f64 = wk
                    .iter()
                    .enumerate()
                    .map(|(a, wa)| wa * (x[i][a] - x[j][a]).powi(2))
                    .sum();
                e[i][j] = v;
                e[j][i] = v;
            }
            e
        })
        .collect()
}

/// The start of the coordinates (see the module notes).
fn start(data: &Data, p: usize) -> Matrix {
    let n = data.n;
    let mut mean_b = zeros(n, n);
    for (o, present) in data.o.iter().zip(&data.present) {
        let similar = matches!(data.level, AlscalLevel::Ordinal { similar: true, .. });
        let given: Vec<f64> = pairs(n)
            .filter(|&(i, j)| present[i][j])
            .map(|(i, j)| o[i][j])
            .collect();
        if given.is_empty() {
            continue;
        }
        let top = given.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let mean = given.iter().sum::<f64>() / given.len() as f64;
        let mut d = zeros(n, n);
        for (i, j) in pairs(n) {
            let v = match present[i][j] {
                true => o[i][j],
                false => mean,
            };
            let v = if similar { top - v } else { v };
            d[i][j] = v;
            d[j][i] = v;
        }
        // the smallest constant for positivity and every triangle inequality
        let mut c = -d
            .iter()
            .enumerate()
            .flat_map(|(i, r)| {
                r.iter()
                    .enumerate()
                    .filter(move |(j, _)| *j != i)
                    .map(|(_, v)| *v)
            })
            .fold(f64::INFINITY, f64::min);
        for i in 0..n {
            for j in 0..n {
                for l in 0..n {
                    if i != j && j != l && i != l {
                        c = c.max(d[i][l] - d[i][j] - d[j][l]);
                    }
                }
            }
        }
        let c = c.max(0.0);
        let sq: Matrix = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| if i == j { 0.0 } else { (d[i][j] + c).powi(2) })
                    .collect()
            })
            .collect();
        let row: Vec<f64> = sq
            .iter()
            .map(|r| r.iter().sum::<f64>() / n as f64)
            .collect();
        let all = row.iter().sum::<f64>() / n as f64;
        let b: Matrix = (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| -0.5 * (sq[i][j] - row[i] - row[j] + all))
                    .collect()
            })
            .collect();
        let rms = (b.iter().flatten().map(|v| v * v).sum::<f64>() / (n * (n - 1)) as f64).sqrt();
        if rms > 0.0 {
            for i in 0..n {
                for j in 0..n {
                    mean_b[i][j] += b[i][j] / rms / data.o.len() as f64;
                }
            }
        }
    }
    let eigen = symmetric_eigen(&mean_b);
    let mut x = zeros(n, p);
    for (a, (value, vector)) in eigen.iter().take(p).enumerate() {
        let root = value.max(0.0).sqrt();
        for i in 0..n {
            x[i][a] = vector[i] * root;
        }
    }
    x
}

/// The squared disparities of every group fitted to the squared distances `e`, then rescaled
/// per subject.
fn scale_data(data: &Data, e: &[Matrix]) -> Vec<Matrix> {
    let n = data.n;
    let mut f: Vec<Matrix> = vec![zeros(n, n); e.len()];
    for group in &data.groups {
        let all: Vec<(usize, usize, usize)> = group
            .iter()
            .flat_map(|&k| {
                pairs(n)
                    .filter(move |&(i, j)| data.present[k][i][j])
                    .map(move |(i, j)| (k, i, j))
            })
            .collect();
        let o = |c: &(usize, usize, usize)| data.o[c.0][c.1][c.2];
        let target = |c: &(usize, usize, usize)| e[c.0][c.1][c.2];
        let fitted: Vec<f64> = match data.level {
            AlscalLevel::Ordinal { untie, similar } => {
                let mut order: Vec<usize> = (0..all.len()).collect();
                order.sort_by(|&a, &b| {
                    let by = o(&all[a]).total_cmp(&o(&all[b]));
                    let by = if similar { by.reverse() } else { by };
                    match untie {
                        true => by.then(target(&all[a]).total_cmp(&target(&all[b]))),
                        false => by,
                    }
                });
                let mut blocks: Vec<Vec<usize>> = Vec::new();
                for &i in &order {
                    match blocks.last_mut() {
                        Some(block) if !untie && o(&all[block[0]]) == o(&all[i]) => block.push(i),
                        _ => blocks.push(vec![i]),
                    }
                }
                let (means, weights): (Vec<f64>, Vec<f64>) = blocks
                    .iter()
                    .map(|b| {
                        (
                            b.iter().map(|&i| target(&all[i])).sum::<f64>() / b.len() as f64,
                            b.len() as f64,
                        )
                    })
                    .unzip();
                let fit = monotone(&means, &weights);
                let mut out = vec![0.0; all.len()];
                for (block, v) in blocks.iter().zip(fit) {
                    block.iter().for_each(|&i| out[i] = v);
                }
                out
            }
            AlscalLevel::Nominal => {
                // equal values side by side after sorting, each run gets its mean
                let mut order: Vec<usize> = (0..all.len()).collect();
                order.sort_by(|&a, &b| o(&all[a]).total_cmp(&o(&all[b])));
                let mut out = vec![0.0; all.len()];
                for run in order.chunk_by(|&a, &b| o(&all[a]) == o(&all[b])) {
                    let mean = run.iter().map(|&j| target(&all[j])).sum::<f64>() / run.len() as f64;
                    run.iter().for_each(|&j| out[j] = mean);
                }
                out
            }
            AlscalLevel::Interval(degree) | AlscalLevel::Ratio(degree) => {
                // a polynomial of the squared data fitted to the squared distances
                let intercept = matches!(data.level, AlscalLevel::Interval(_));
                let powers: Vec<i32> = match intercept {
                    true => (0..=degree as i32).collect(),
                    false => (1..=degree as i32).collect(),
                };
                let rows: Matrix = all
                    .iter()
                    .map(|c| powers.iter().map(|&q| (o(c) * o(c)).powi(q)).collect())
                    .collect();
                let y: Vec<f64> = all.iter().map(target).collect();
                let q = powers.len();
                let xtx: Matrix = (0..q)
                    .map(|a| {
                        (0..q)
                            .map(|b| rows.iter().map(|r| r[a] * r[b]).sum())
                            .collect()
                    })
                    .collect();
                let xty: Vec<f64> = (0..q)
                    .map(|a| rows.iter().zip(&y).map(|(r, v)| r[a] * v).sum())
                    .collect();
                match inverse(&xtx) {
                    Some((inv, _)) => {
                        let beta: Vec<f64> = inv
                            .iter()
                            .map(|r| r.iter().zip(&xty).map(|(a, b)| a * b).sum())
                            .collect();
                        rows.iter()
                            .map(|r| {
                                r.iter()
                                    .zip(&beta)
                                    .map(|(a, b)| a * b)
                                    .sum::<f64>()
                                    .max(0.0)
                            })
                            .collect()
                    }
                    None => all.iter().map(target).collect(),
                }
            }
        };
        for (&(k, i, j), v) in all.iter().zip(fitted) {
            f[k][i][j] = v;
            f[k][j][i] = v;
        }
    }
    // each subject's disparities rescaled to fit its squared distances best
    for (k, fk) in f.iter_mut().enumerate() {
        let (mut ee, mut ef) = (0.0, 0.0);
        for (i, j) in pairs(n).filter(|&(i, j)| data.present[k][i][j]) {
            ee += e[k][i][j] * e[k][i][j];
            ef += e[k][i][j] * fk[i][j];
        }
        if ef > 0.0 {
            fk.iter_mut().flatten().for_each(|v| *v *= ee / ef);
        }
    }
    f
}

fn sstress(data: &Data, e: &[Matrix], f: &[Matrix]) -> f64 {
    let m = e.len() as f64;
    let total: f64 = (0..e.len())
        .map(|k| {
            let (mut num, mut den) = (0.0, 0.0);
            for (i, j) in pairs(data.n).filter(|&(i, j)| data.present[k][i][j]) {
                num += (e[k][i][j] - f[k][i][j]).powi(2);
                den += f[k][i][j].powi(2);
            }
            if den > 0.0 { num / den } else { 0.0 }
        })
        .sum();
    (total / m).sqrt()
}

/// The real roots of a t^3 + b t^2 + c t + d.
fn cubic_roots(a: f64, b: f64, c: f64, d: f64) -> Vec<f64> {
    let scale = a.abs().max(b.abs()).max(c.abs()).max(d.abs());
    if a.abs() <= 1e-14 * scale {
        if b.abs() <= 1e-14 * scale {
            return match c.abs() > 0.0 {
                true => vec![-d / c],
                false => Vec::new(),
            };
        }
        let disc = c * c - 4.0 * b * d;
        return match disc >= 0.0 {
            true => vec![
                (-c + disc.sqrt()) / (2.0 * b),
                (-c - disc.sqrt()) / (2.0 * b),
            ],
            false => Vec::new(),
        };
    }
    let (b, c, d) = (b / a, c / a, d / a);
    let q = (3.0 * c - b * b) / 9.0;
    let r = (9.0 * b * c - 27.0 * d - 2.0 * b * b * b) / 54.0;
    let disc = q * q * q + r * r;
    let shift = -b / 3.0;
    if disc > 0.0 {
        let s = (r + disc.sqrt()).cbrt();
        let t = (r - disc.sqrt()).cbrt();
        vec![shift + s + t]
    } else {
        let theta = match q < 0.0 {
            true => (r / (-q * q * q).sqrt()).clamp(-1.0, 1.0).acos(),
            false => 0.0,
        };
        let root = 2.0 * (-q).max(0.0).sqrt();
        (0..3)
            .map(|k| shift + root * ((theta + 2.0 * std::f64::consts::PI * k as f64) / 3.0).cos())
            .collect()
    }
}

/// Every coordinate in turn at the minimum of S-stress with the others held.
fn update_coordinates(data: &Data, x: &mut Matrix, w: &[Vec<f64>], f: &[Matrix]) {
    let n = data.n;
    let p = x.first().map_or(0, Vec::len);
    let m = w.len();
    let c: Vec<f64> = (0..m)
        .map(|k| {
            let s: f64 = pairs(n)
                .filter(|&(i, j)| data.present[k][i][j])
                .map(|(i, j)| f[k][i][j].powi(2))
                .sum();
            if s > 0.0 { 1.0 / s } else { 0.0 }
        })
        .collect();
    let mut terms: Vec<(f64, f64, f64, f64)> = Vec::new();
    for l in 0..n {
        for _ in 0..20 {
            let mut change: f64 = 0.0;
            for a in 0..p {
                // g(t) = sum_k c_k sum_j (w_ka (t - x_ja)^2 + q_jk)^2, its derivative a cubic
                // (c_k, w_ka, x_ja, q_jk) of every pair of point l
                terms.clear();
                for k in (0..m).filter(|&k| w[k][a] != 0.0 && c[k] != 0.0) {
                    for j in (0..n).filter(|&j| j != l && data.present[k][l][j]) {
                        let rest: f64 = (0..p)
                            .filter(|&b| b != a)
                            .map(|b| w[k][b] * (x[l][b] - x[j][b]).powi(2))
                            .sum();
                        terms.push((c[k], w[k][a], x[j][a], rest - f[k][l][j]));
                    }
                }
                let (mut c3, mut c2, mut c1, mut c0) = (0.0, 0.0, 0.0, 0.0);
                for &(ck, wa, xj, q) in &terms {
                    // c_k 4 w (w (t - xj)^3 + q (t - xj))
                    let s = ck * 4.0 * wa;
                    c3 += s * wa;
                    c2 += s * wa * (-3.0 * xj);
                    c1 += s * (wa * 3.0 * xj * xj + q);
                    c0 += s * (-wa * xj.powi(3) - q * xj);
                }
                if c3 == 0.0 {
                    continue;
                }
                let g = |t: f64| -> f64 {
                    terms
                        .iter()
                        .map(|&(ck, wa, xj, q)| ck * (wa * (t - xj).powi(2) + q).powi(2))
                        .sum()
                };
                let old = x[l][a];
                let best = cubic_roots(c3, c2, c1, c0)
                    .into_iter()
                    .filter(|t| t.is_finite())
                    .chain(std::iter::once(old))
                    .min_by(|s, t| g(*s).total_cmp(&g(*t)))
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

/// Least squares dimension weights of every subject; negative ones become 0 unless allowed.
fn update_weights(
    data: &Data,
    x: &Matrix,
    f: &[Matrix],
    negative: bool,
    old: &[Vec<f64>],
) -> Vec<Vec<f64>> {
    let n = data.n;
    let p = x.first().map_or(0, Vec::len);
    (0..f.len())
        .map(|k| {
            let rows: Vec<(Vec<f64>, f64)> = pairs(n)
                .filter(|&(i, j)| data.present[k][i][j])
                .map(|(i, j)| {
                    (
                        (0..p).map(|a| (x[i][a] - x[j][a]).powi(2)).collect(),
                        f[k][i][j],
                    )
                })
                .collect();
            let fit = |cols: &[usize]| -> Option<Vec<f64>> {
                let ptp: Matrix = cols
                    .iter()
                    .map(|&a| {
                        cols.iter()
                            .map(|&b| rows.iter().map(|r| r.0[a] * r.0[b]).sum())
                            .collect()
                    })
                    .collect();
                let pty: Vec<f64> = cols
                    .iter()
                    .map(|&a| rows.iter().map(|r| r.0[a] * r.1).sum())
                    .collect();
                let (inv, _) = inverse(&ptp)?;
                let beta: Vec<f64> = inv
                    .iter()
                    .map(|r| r.iter().zip(&pty).map(|(a, b)| a * b).sum())
                    .collect();
                let mut out = vec![0.0; p];
                cols.iter().zip(beta).for_each(|(&a, b)| out[a] = b);
                Some(out)
            };
            if !negative {
                // nonnegative least squares
                let design: Matrix = rows.iter().map(|r| r.0.clone()).collect();
                let target: Vec<f64> = rows.iter().map(|r| r.1).collect();
                return crate::mds::nnls(&design, &target);
            }
            let all: Vec<usize> = (0..p).collect();
            fit(&all).unwrap_or_else(|| old[k].clone())
        })
        .collect()
}

fn iterate(data: &Data, command: &Alscal, x: Matrix, w: Vec<Vec<f64>>) -> Fit {
    let (mut x, mut w) = (x, w);
    let mut history: Vec<f64> = Vec::new();
    let mut f;
    loop {
        let e = squared(&x, &w);
        f = scale_data(data, &e);
        let s = sstress(data, &e, &f);
        let improvement = history.last().map(|last| last - s);
        history.push(s);
        if improvement.is_some_and(|d| d < command.converge)
            || s <= command.stress_min
            || history.len() >= command.iterations
        {
            break;
        }
        if command.indscal {
            w = update_weights(data, &x, &f, command.negative, &w);
        }
        update_coordinates(data, &mut x, &w, &f);
    }
    Fit { x, w, f, history }
}

/// Centered, turned and scaled for output (see the help).
fn finish(mut fit: Fit, indscal: bool) -> Fit {
    let n = fit.x.len();
    let p = fit.x.first().map_or(0, Vec::len);
    for a in 0..p {
        let mean = fit.x.iter().map(|r| r[a]).sum::<f64>() / n as f64;
        fit.x.iter_mut().for_each(|r| r[a] -= mean);
    }
    if indscal {
        for a in 0..p {
            let ss: f64 = fit.x.iter().map(|r| r[a] * r[a]).sum();
            if ss > 0.0 {
                let s = (n as f64 / ss).sqrt();
                fit.x.iter_mut().for_each(|r| r[a] *= s);
                fit.w.iter_mut().for_each(|w| w[a] /= s * s);
            }
        }
        let importance: Vec<f64> = (0..p)
            .map(|a| fit.w.iter().map(|w| w[a] * w[a]).sum())
            .collect();
        let mut order: Vec<usize> = (0..p).collect();
        order.sort_by(|&a, &b| importance[b].total_cmp(&importance[a]));
        fit.x = fit
            .x
            .iter()
            .map(|r| order.iter().map(|&a| r[a]).collect())
            .collect();
        fit.w = fit
            .w
            .iter()
            .map(|w| order.iter().map(|&a| w[a]).collect())
            .collect();
    } else {
        fit.x = principal_axes(&fit.x);
        let ss: f64 = fit.x.iter().flatten().map(|v| v * v).sum();
        if ss > 0.0 {
            let s = ((n * p) as f64 / ss).sqrt();
            fit.x.iter_mut().flatten().for_each(|v| *v *= s);
            fit.f
                .iter_mut()
                .flatten()
                .flatten()
                .for_each(|v| *v *= s * s);
        }
    }
    first_positive(&mut fit.x);
    fit
}

impl Engine {
    pub(crate) fn alscal(&mut self, command: &Alscal) -> Result<Vec<Table>, Error> {
        let input = self.proximity_input(&Reading {
            who: "ALSCAL",
            variables: &command.variables,
            weights: &[],
            table: None,
            shape: ProxShape::Lower,
            matrix_in: &command.matrix_in,
            matrix_items: true,
        })?;
        let mut warnings = input.warnings.clone();
        let n = input.objects.len();
        let m = input.sources.len();
        if let Some(rows) = command.rows
            && command.matrix_in.is_none()
            && rows != n
        {
            return Err(Error(format!(
                "ALSCAL: INPUT=ROWS({rows}) with {n} variables needs SHAPE=RECTANGULAR, which is not supported yet"
            )));
        }
        if n < 4 {
            return Err(Error(format!(
                "ALSCAL needs at least 4 objects, but there are {n}"
            )));
        }
        let (min_dim, max_dim) = command.dimensions;
        if max_dim > n - 1 {
            return Err(Error(format!(
                "ALSCAL: {max_dim} dimensions need at least {} objects, but there are {n}",
                max_dim + 1
            )));
        }
        let mut indscal = command.indscal;
        if indscal && m < 2 {
            warnings.push(
                "ALSCAL: INDSCAL needs more than one matrix; the Euclidean model is used".into(),
            );
            indscal = false;
        }
        let mut level = command.level;
        if input.similarity
            && let AlscalLevel::Ordinal { untie, .. } = level
        {
            level = AlscalLevel::Ordinal {
                untie,
                similar: true,
            };
        }
        let present: Vec<Vec<Vec<bool>>> = (0..m)
            .map(|k| {
                (0..n)
                    .map(|i| {
                        (0..n)
                            .map(|j| {
                                input.w[k][i][j] > 0.0 && input.delta[k][i][j] >= command.cutoff
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect();
        let mut o = input.delta.clone();
        if input.similarity && matches!(level, AlscalLevel::Interval(_) | AlscalLevel::Ratio(_)) {
            // the metric levels need dissimilarities: each from the largest similarity
            warnings.push(
                "ALSCAL: the matrix holds similarities; each is taken from the largest to give dissimilarities"
                    .into(),
            );
            for (ok, pk) in o.iter_mut().zip(&present) {
                let top = pairs(n)
                    .filter(|&(i, j)| pk[i][j])
                    .map(|(i, j)| ok[i][j])
                    .fold(f64::NEG_INFINITY, f64::max);
                ok.iter_mut().flatten().for_each(|v| *v = top - *v);
            }
        }
        let data = Data {
            n,
            o,
            present,
            level,
            groups: match command.unconditional {
                true => vec![(0..m).collect()],
                false => (0..m).map(|k| vec![k]).collect(),
            },
        };
        if data.present.iter().all(|s| s.iter().flatten().all(|v| !v)) {
            return Err(Error("ALSCAL: every proximity is missing".into()));
        }
        let run = Alscal {
            indscal,
            ..command.clone()
        };
        let mut fits: Vec<(usize, Fit)> = Vec::new();
        for p in (min_dim..=max_dim).rev() {
            let x = match fits.last() {
                None => start(&data, p),
                Some((_, previous)) => previous.x.iter().map(|r| r[..p].to_vec()).collect(),
            };
            let w = match fits.last() {
                Some((_, previous)) if indscal => {
                    previous.w.iter().map(|w| w[..p].to_vec()).collect()
                }
                _ => vec![vec![1.0; p]; m],
            };
            let fit = finish(iterate(&data, &run, x, w), indscal);
            fits.push((p, fit));
        }
        let several = fits.len() > 1;
        let mut tables = Vec::new();
        let mut charts = Vec::new();
        if command.print_header {
            let level_name = match level {
                AlscalLevel::Ordinal { .. } => "Ordinal",
                AlscalLevel::Interval(_) => "Interval",
                AlscalLevel::Ratio(_) => "Ratio",
                AlscalLevel::Nominal => "Nominal",
            };
            let rows = [
                ("Stimuli", n.to_string()),
                ("Matrices", m.to_string()),
                ("Level of measurement", level_name.to_string()),
                (
                    "Conditionality",
                    if command.unconditional {
                        "Unconditional"
                    } else {
                        "Matrix"
                    }
                    .to_string(),
                ),
                (
                    "Model",
                    if indscal {
                        "Individual differences (INDSCAL)"
                    } else {
                        "Euclidean"
                    }
                    .to_string(),
                ),
                ("Dimensions", format!("{min_dim} to {max_dim}")),
                ("Maximum iterations", command.iterations.to_string()),
                ("Convergence criterion", command.converge.to_string()),
                ("Smallest S-stress", command.stress_min.to_string()),
                ("Cutoff", command.cutoff.to_string()),
            ];
            tables.push(Table {
                title: "Analysis Options".into(),
                columns: vec![String::new(), "Value".into()],
                rows: rows
                    .iter()
                    .map(|(a, b)| vec![text(*a), text(b.clone())])
                    .collect(),
                row_headers: 1,
                ..Table::default()
            });
        }
        if command.print_data {
            for (k, name) in input.sources.iter().enumerate() {
                let title = match m {
                    1 => "Raw Data".to_string(),
                    _ => format!("Raw Data: Matrix {} ({name})", k + 1),
                };
                tables.push(square(
                    title,
                    &input.objects,
                    &input.delta[k],
                    Some(&input.w[k]),
                ));
            }
        }
        for (p, fit) in &fits {
            let suffix = |title: &str| match several {
                true => format!("{title} ({p} Dimensions)"),
                false => title.to_string(),
            };
            tables.push(Table {
                title: suffix("Iteration History"),
                columns: ["Iteration", "S-stress", "Improvement"]
                    .map(String::from)
                    .to_vec(),
                rows: fit
                    .history
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let gain = match i {
                            0 => Cell::Empty,
                            _ => Cell::Number(fit.history[i - 1] - s),
                        };
                        vec![Cell::Number((i + 1) as f64), Cell::Number(*s), gain]
                    })
                    .collect(),
                row_headers: 1,
                ..Table::default()
            });
            let e = squared(&fit.x, &fit.w);
            let mut rows = Vec::new();
            let (mut stress_sq, mut rsq_sum) = (0.0, 0.0);
            for (k, ek) in e.iter().enumerate() {
                let (d, dstar): (Vec<f64>, Vec<f64>) = pairs(n)
                    .filter(|&(i, j)| data.present[k][i][j])
                    .map(|(i, j)| (ek[i][j].sqrt(), fit.f[k][i][j].max(0.0).sqrt()))
                    .unzip();
                let num: f64 = d.iter().zip(&dstar).map(|(a, b)| (a - b).powi(2)).sum();
                let den: f64 = d.iter().map(|a| a * a).sum();
                let stress = if den > 0.0 { (num / den).sqrt() } else { 0.0 };
                let rsq = correlation(&d, &dstar).powi(2);
                stress_sq += stress * stress / m as f64;
                rsq_sum += rsq / m as f64;
                rows.push(vec![
                    text(input.sources[k].clone()),
                    number(Some(stress)),
                    number(Some(rsq)),
                ]);
            }
            if m > 1 {
                rows.push(vec![
                    text("Averaged"),
                    number(Some(stress_sq.sqrt())),
                    number(Some(rsq_sum)),
                ]);
            }
            tables.push(Table {
                title: suffix("Stress and Squared Correlation (RSQ) in Distances"),
                columns: ["Matrix", "Stress", "RSQ"].map(String::from).to_vec(),
                rows,
                row_headers: 1,
                caption: Some("Stress is Kruskal's stress formula 1; RSQ is the squared correlation of the disparities and the distances.".into()),
                ..Table::default()
            });
            let dims: Vec<String> = (1..=*p).map(|d| format!("Dimension {d}")).collect();
            tables.push(coordinates(
                suffix("Stimulus Coordinates"),
                &input.objects,
                &fit.x,
                &dims,
            ));
            if indscal {
                let mut columns = vec!["Matrix".to_string()];
                columns.extend(dims.iter().cloned());
                tables.push(Table {
                    title: suffix("Subject Weights"),
                    columns,
                    rows: input
                        .sources
                        .iter()
                        .zip(&fit.w)
                        .map(|(name, w)| {
                            let mut row = vec![text(name.clone())];
                            row.extend(w.iter().map(|v| Cell::Number(*v)));
                            row
                        })
                        .collect(),
                    row_headers: 1,
                    ..Table::default()
                });
            }
            if command.print_data {
                for k in 0..m {
                    let scaled: Matrix = fit.f[k]
                        .iter()
                        .map(|r| r.iter().map(|v| v.max(0.0).sqrt()).collect())
                        .collect();
                    let title = match m {
                        1 => suffix("Optimally Scaled Data"),
                        _ => suffix(&format!("Optimally Scaled Data: Matrix {}", k + 1)),
                    };
                    tables.push(square(title, &input.objects, &scaled, Some(&input.w[k])));
                }
            }
        }
        let (p, fit) = fits.last().expect("one dimensionality at least");
        if let Some(all) = command.plot {
            charts.push(space_chart(
                "Derived Stimulus Configuration".into(),
                &input.objects,
                &fit.x,
                *p,
            ));
            if indscal && *p >= 2 {
                let points = input
                    .sources
                    .iter()
                    .zip(&fit.w)
                    .map(|(name, w)| Point {
                        x: w[0],
                        y: w[1],
                        z: None,
                        label: Some(name.clone()),
                    })
                    .collect();
                charts.push(points_chart(
                    "Derived Subject Weights",
                    "Dimension 1",
                    "Dimension 2",
                    vec![PointGroup {
                        name: "Matrices".into(),
                        points,
                    }],
                ));
            }
            let e = squared(&fit.x, &fit.w);
            let group = |k: usize, what: &dyn Fn(usize, usize) -> (f64, f64)| PointGroup {
                name: input.sources[k].clone(),
                points: pairs(n)
                    .filter(|&(i, j)| data.present[k][i][j])
                    .map(|(i, j)| {
                        let (x, y) = what(i, j);
                        Point {
                            x,
                            y,
                            z: None,
                            label: None,
                        }
                    })
                    .collect(),
            };
            let linear = (0..m)
                .map(|k| {
                    group(k, &|i, j| {
                        (fit.f[k][i][j].max(0.0).sqrt(), e[k][i][j].sqrt())
                    })
                })
                .collect();
            charts.push(points_chart(
                "Scatterplot of Linear Fit",
                "Disparities",
                "Distances",
                linear,
            ));
            if matches!(level, AlscalLevel::Ordinal { .. } | AlscalLevel::Nominal) {
                let nonlinear = (0..m)
                    .map(|k| group(k, &|i, j| (input.delta[k][i][j], e[k][i][j].sqrt())))
                    .collect();
                charts.push(points_chart(
                    "Scatterplot of Nonlinear Fit",
                    "Observations",
                    "Distances",
                    nonlinear,
                ));
            }
            let transformation = |ks: Vec<usize>, title: String| {
                let groups = ks
                    .iter()
                    .map(|&k| {
                        group(k, &|i, j| {
                            (input.delta[k][i][j], fit.f[k][i][j].max(0.0).sqrt())
                        })
                    })
                    .collect();
                points_chart(&title, "Observations", "Disparities", groups)
            };
            if all && m > 1 && !command.unconditional {
                for k in 0..m {
                    charts.push(transformation(
                        vec![k],
                        format!("Transformation Plot: {}", input.sources[k]),
                    ));
                }
            } else {
                charts.push(transformation(
                    (0..m).collect(),
                    "Transformation Plot".into(),
                ));
            }
        }
        if let Some(file) = &command.outfile {
            // one block per dimensionality: the coordinates, then the subject weights
            let mut kinds = Vec::new();
            let mut dimens = Vec::new();
            let mut matnum = Vec::new();
            let mut values: Vec<Vec<f64>> = Vec::new();
            let top = max_dim;
            for (p, fit) in &fits {
                for r in &fit.x {
                    kinds.push("CON".to_string());
                    dimens.push(Some(*p as f64));
                    matnum.push(Some(0.0));
                    values.push(r.clone());
                }
                if indscal {
                    for (k, w) in fit.w.iter().enumerate() {
                        kinds.push("SUB".to_string());
                        dimens.push(Some(*p as f64));
                        matnum.push(Some((k + 1) as f64));
                        values.push(w.clone());
                    }
                }
            }
            let mut out = Dataset::new(kinds.len());
            out.set_string_values("TYPE_", 8, kinds)?;
            out.set_numeric_values("DIMENS", dimens)?;
            out.set_numeric_values("MATNUM", matnum)?;
            for a in 0..top {
                out.set_numeric_values(
                    &format!("DIM{}", a + 1),
                    values.iter().map(|r| r.get(a).copied()).collect(),
                )?;
            }
            self.write_data_file(file, out)?;
        }
        for warning in warnings {
            self.warn(warning);
        }
        self.charts.extend(charts);
        Ok(tables)
    }
}

fn correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    let (mx, my) = (x.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
    let sxy: f64 = x.iter().zip(y).map(|(a, b)| (a - mx) * (b - my)).sum();
    let sxx: f64 = x.iter().map(|a| (a - mx).powi(2)).sum();
    let syy: f64 = y.iter().map(|b| (b - my).powi(2)).sum();
    match sxx > 0.0 && syy > 0.0 {
        true => sxy / (sxx * syy).sqrt(),
        false => 0.0,
    }
}

fn points_chart(title: &str, x: &str, y: &str, groups: Vec<PointGroup>) -> Chart {
    Chart::plot(
        ChartKind::Scatter,
        title.into(),
        Axis::titled(x),
        Axis::titled(y),
        Plot::Points {
            groups,
            lines: Vec::new(),
            z_axis: None,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_roots_are_found() {
        let mut r = cubic_roots(1.0, -6.0, 11.0, -6.0);
        r.sort_by(f64::total_cmp);
        assert!(
            r.iter()
                .zip([1.0, 2.0, 3.0])
                .all(|(a, b)| (a - b).abs() < 1e-9),
            "{r:?}"
        );
        let r = cubic_roots(2.0, 0.0, 0.0, -16.0);
        assert!(r.len() == 1 && (r[0] - 2.0).abs() < 1e-12);
    }
}
