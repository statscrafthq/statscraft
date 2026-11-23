//! The independent variables restriction of PROXSCAL (identity model): the common space is a
//! weighted sum of independent variables, Z = Q B, each variable transformed on its own.
//!
//! One step follows the Algorithms manual's Restrictions section. For each column q_j of Q (the
//! variables, then the dummy variables that fill the dimensions the variables leave over, which
//! are free):
//!   1. U = Z - q_j b_j', T = C - V U (C is the mean of B(X_k) X_k over the sources, V the mean
//!      Laplacian);
//!   2. b_j = T' q_j / (q_j' V q_j);
//!   3. the target q~ = q_j + (T b_j - (b_j' b_j) V q_j) / k, with k = (b_j' b_j) times the
//!      largest eigenvalue of V, is fitted by the variable's transformation of its original
//!      values (a dummy variable takes q~ as it is); positions with a missing original value take
//!      q~; the result is centered, scaled to a sum of squares of n, and b_j takes the reverse.
//!
//! The fits differ from those of the proximities in having a free intercept and no sign
//! condition: a variable's sign is carried by its regression weights.

use crate::matrix::{Matrix, multiply, symmetric_eigen, transpose};
use crate::mds::{self, spline_basis};

/// How an independent variable is transformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Interval,
    Nominal,
    Ordinal { untie: bool },
    Spline { degree: usize, inknot: usize },
}

/// The independent variables: per variable its values over the objects (None where missing).
#[derive(Debug, Clone)]
pub(crate) struct Variables {
    pub names: Vec<String>,
    pub values: Vec<Vec<Option<f64>>>,
    pub kinds: Vec<Kind>,
    /// Per variable that is a spline: the centered basis columns at the observed positions.
    bases: Vec<Option<Matrix>>,
}

/// The variables, transformed and normalized, and their regression weights; the variables of the
/// command come first, then the dummy variables.
#[derive(Debug, Clone)]
pub(crate) struct Fit {
    /// n x t.
    pub q: Matrix,
    /// t x p.
    pub b: Matrix,
}

impl Variables {
    pub fn new(names: Vec<String>, values: Vec<Vec<Option<f64>>>, kinds: Vec<Kind>) -> Self {
        let bases = values
            .iter()
            .zip(&kinds)
            .map(|(column, kind)| match kind {
                Kind::Spline { degree, inknot } => {
                    let observed: Vec<f64> = column.iter().flatten().copied().collect();
                    match observed.len() > 1 {
                        true => Some(centered_monotone_columns(&observed, *degree, *inknot)),
                        false => None,
                    }
                }
                _ => None,
            })
            .collect();
        Variables {
            names,
            values,
            kinds,
            bases,
        }
    }

    pub fn count(&self) -> usize {
        self.values.len()
    }
}

/// The monotone spline columns without the intercept, each centered.
fn centered_monotone_columns(values: &[f64], degree: usize, inknot: usize) -> Matrix {
    let basis = spline_basis(values, degree, inknot);
    let n = basis.len() as f64;
    let columns = basis.first().map_or(0, Vec::len);
    let mean: Vec<f64> = (1..columns)
        .map(|c| basis.iter().map(|r| r[c]).sum::<f64>() / n)
        .collect();
    basis
        .iter()
        .map(|r| (1..columns).map(|c| r[c] - mean[c - 1]).collect())
        .collect()
}

/// Centers `col` and scales it to a sum of squares of its length; the factor applied, None (and
/// the column as it was) when it has no spread.
pub(crate) fn normalized(col: &mut [f64]) -> Option<f64> {
    let n = col.len() as f64;
    let mean = col.iter().sum::<f64>() / n;
    let ss: f64 = col.iter().map(|v| (v - mean).powi(2)).sum();
    let scale = col.iter().fold(0.0_f64, |s, v| s.max(v.abs()));
    if ss <= 1e-20 * (1.0 + scale * scale) * n {
        return None;
    }
    let factor = (n / ss).sqrt();
    col.iter_mut().for_each(|v| *v = (*v - mean) * factor);
    Some(factor)
}

fn column(m: &Matrix, j: usize) -> Vec<f64> {
    m.iter().map(|r| r[j]).collect()
}

/// Q B.
pub(crate) fn space(fit: &Fit) -> Matrix {
    multiply(&fit.q, &fit.b)
}

/// The start: the variables centered and normalized (a missing value at the mean), their weights
/// by least squares on the start configuration `z`, and for each dimension the variables do not
/// cover a dummy variable from the leading components of what is left.
pub(crate) fn start(vars: &Variables, z: &Matrix) -> Fit {
    let n = z.len();
    let p = z.first().map_or(0, Vec::len);
    let h = vars.count();
    let dummies = p.saturating_sub(h);
    let mut q = mds::zeros(n, h + dummies);
    for (j, column) in vars.values.iter().enumerate() {
        let observed: Vec<f64> = column.iter().flatten().copied().collect();
        let mean = observed.iter().sum::<f64>() / observed.len().max(1) as f64;
        let mut col: Vec<f64> = column.iter().map(|v| v.unwrap_or(mean)).collect();
        if normalized(&mut col).is_none() {
            col.iter_mut().for_each(|v| *v = 0.0);
        }
        for (i, v) in col.iter().enumerate() {
            q[i][j] = *v;
        }
    }
    let mut zc = z.clone();
    mds::center(&mut zc);
    let b_real = least_squares(&q, h, &zc);
    let mut b = mds::zeros(h + dummies, p);
    for (row, r) in b.iter_mut().zip(&b_real) {
        *row = r.clone();
    }
    if dummies > 0 {
        let fitted = multiply(&q, &b);
        let rest: Matrix = zc
            .iter()
            .zip(&fitted)
            .map(|(a, c)| a.iter().zip(c).map(|(x, y)| x - y).collect())
            .collect();
        // R = U S V': dummy a is sqrt(n) u_a with weights s_a v_a / sqrt(n)
        let eigen = symmetric_eigen(&multiply(&transpose(&rest), &rest));
        let root = (n as f64).sqrt();
        for (a, (value, v)) in eigen.iter().take(dummies).enumerate() {
            let s = value.max(0.0).sqrt();
            if s <= 1e-12 {
                continue;
            }
            let u: Vec<f64> = rest
                .iter()
                .map(|r| r.iter().zip(v).map(|(x, y)| x * y).sum::<f64>() / s)
                .collect();
            for i in 0..n {
                q[i][h + a] = root * u[i];
            }
            for d in 0..p {
                b[h + a][d] = s * v[d] / root;
            }
        }
    }
    Fit { q, b }
}

/// Weights of the first `h` columns of `q` that fit `z` best (pseudo-inverse of the cross
/// products, so a variable with no spread or a collinear one gets weight 0).
fn least_squares(q: &Matrix, h: usize, z: &Matrix) -> Matrix {
    let p = z.first().map_or(0, Vec::len);
    if h == 0 {
        return Matrix::new();
    }
    let used: Matrix = q.iter().map(|r| r[..h].to_vec()).collect();
    let qtq = multiply(&transpose(&used), &used);
    let inverse = mds::pseudo_inverse(&qtq);
    let b = multiply(&inverse, &multiply(&transpose(&used), z));
    debug_assert_eq!(b.len(), h);
    debug_assert!(b.iter().all(|r| r.len() == p));
    b
}

/// The fit of `target` by the transformation `kind` of the values `x`: a free intercept, values
/// not increasing against the target allowed to be flat. Positions with a missing `x` take the
/// target. None when the original values have no spread.
fn fit_target(
    kind: Kind,
    x: &[Option<f64>],
    target: &[f64],
    basis: Option<&Matrix>,
) -> Option<Vec<f64>> {
    let seen: Vec<usize> = (0..x.len()).filter(|&i| x[i].is_some()).collect();
    if seen.len() < 2 {
        return None;
    }
    let xs = |i: usize| x[i].expect("seen");
    if seen.iter().all(|&i| xs(i) == xs(seen[0])) {
        return None;
    }
    let count = seen.len() as f64;
    let mean_t = seen.iter().map(|&i| target[i]).sum::<f64>() / count;
    let mut out = target.to_vec();
    match kind {
        Kind::Interval => {
            let mean_x = seen.iter().map(|&i| xs(i)).sum::<f64>() / count;
            let sxx: f64 = seen.iter().map(|&i| (xs(i) - mean_x).powi(2)).sum();
            let sxt: f64 = seen
                .iter()
                .map(|&i| (xs(i) - mean_x) * (target[i] - mean_t))
                .sum();
            for &i in &seen {
                out[i] = mean_t + sxt / sxx * (xs(i) - mean_x);
            }
        }
        Kind::Nominal => {
            let mut groups: Vec<(u64, f64, f64)> = Vec::new();
            for &i in &seen {
                let key = xs(i).to_bits();
                match groups.iter_mut().find(|g| g.0 == key) {
                    Some(g) => {
                        g.1 += target[i];
                        g.2 += 1.0;
                    }
                    None => groups.push((key, target[i], 1.0)),
                }
            }
            for &i in &seen {
                let g = groups
                    .iter()
                    .find(|g| g.0 == xs(i).to_bits())
                    .expect("group");
                out[i] = g.1 / g.2;
            }
        }
        Kind::Ordinal { untie } => {
            let mut order = seen.clone();
            order.sort_by(|&a, &b| {
                let by_x = xs(a).total_cmp(&xs(b));
                match untie {
                    true => by_x.then(target[a].total_cmp(&target[b])),
                    false => by_x,
                }
            });
            // tie blocks: together under KEEPTIES, one value each under UNTIE
            let mut blocks: Vec<Vec<usize>> = Vec::new();
            for &o in &order {
                match blocks.last_mut() {
                    Some(block) if !untie && xs(block[0]) == xs(o) => block.push(o),
                    _ => blocks.push(vec![o]),
                }
            }
            let means: Vec<f64> = blocks
                .iter()
                .map(|b| b.iter().map(|&o| target[o]).sum::<f64>() / b.len() as f64)
                .collect();
            let weights: Vec<f64> = blocks.iter().map(|b| b.len() as f64).collect();
            let fitted = mds::monotone(&means, &weights);
            for (block, value) in blocks.iter().zip(fitted) {
                for &o in block {
                    out[o] = value;
                }
            }
        }
        Kind::Spline { .. } => {
            let s = basis?;
            let y: Vec<f64> = seen.iter().map(|&i| target[i] - mean_t).collect();
            let b = mds::nnls(s, &y);
            for (row, &i) in s.iter().zip(&seen) {
                out[i] = mean_t + row.iter().zip(&b).map(|(a, c)| a * c).sum::<f64>();
            }
        }
    }
    Some(out)
}

/// One cycle over the variables; returns the new common space. `v` is the mean Laplacian,
/// `lambda` its largest eigenvalue and `c` the mean of B(X_k) X_k.
pub(crate) fn step(fit: &mut Fit, vars: &Variables, v: &Matrix, lambda: f64, c: &Matrix) -> Matrix {
    let n = fit.q.len();
    let p = fit.b.first().map_or(0, Vec::len);
    let t = fit.b.len();
    let mut z = space(fit);
    for j in 0..t {
        let qj = column(&fit.q, j);
        let bj = fit.b[j].clone();
        let mut u = z.clone();
        for i in 0..n {
            for d in 0..p {
                u[i][d] -= qj[i] * bj[d];
            }
        }
        let vu = multiply(v, &u);
        let tm: Matrix = (0..n)
            .map(|i| (0..p).map(|d| c[i][d] - vu[i][d]).collect())
            .collect();
        let vq: Vec<f64> = v
            .iter()
            .map(|r| r.iter().zip(&qj).map(|(a, b)| a * b).sum())
            .collect();
        let denom: f64 = qj.iter().zip(&vq).map(|(a, b)| a * b).sum();
        if denom <= 1e-14 {
            continue;
        }
        let b_new: Vec<f64> = (0..p)
            .map(|d| (0..n).map(|i| tm[i][d] * qj[i]).sum::<f64>() / denom)
            .collect();
        let bb: f64 = b_new.iter().map(|x| x * x).sum();
        let mut q_new = qj.clone();
        let mut b_out = b_new.clone();
        if bb > 1e-20 && lambda > 0.0 {
            let k = bb * lambda;
            let target: Vec<f64> = (0..n)
                .map(|i| {
                    let tb: f64 = (0..p).map(|d| tm[i][d] * b_new[d]).sum();
                    qj[i] + (tb - bb * vq[i]) / k
                })
                .collect();
            let fitted = match j < vars.count() {
                true => fit_target(
                    vars.kinds[j],
                    &vars.values[j],
                    &target,
                    vars.bases[j].as_ref(),
                ),
                false => Some(target),
            };
            if let Some(mut y) = fitted
                && let Some(factor) = normalized(&mut y)
            {
                q_new = y;
                b_out = b_new.iter().map(|x| x / factor).collect();
            }
        } else {
            b_out = b_new;
        }
        for (row, v) in fit.q.iter_mut().zip(&q_new) {
            row[j] = *v;
        }
        fit.b[j] = b_out;
        z = u;
        for (row, v) in z.iter_mut().zip(&q_new) {
            for (x, w) in row.iter_mut().zip(&fit.b[j]) {
                *x += v * w;
            }
        }
    }
    z
}

/// The largest eigenvalue of a symmetric matrix.
pub(crate) fn largest_eigenvalue(m: &Matrix) -> f64 {
    symmetric_eigen(m).first().map_or(0.0, |e| e.0)
}

/// Dilates the space to fit the transformed proximities best, through the weights. Returns it.
pub(crate) fn dilate(
    problem: &mds::Problem,
    dhat: &[Matrix],
    fit: &mut Fit,
    a: &[Matrix],
) -> Matrix {
    let mut z = space(fit);
    let s = mds::sums(problem, dhat, &mds::spaces(&z, a));
    if s.eta_x > 0.0 && s.rho > 0.0 {
        let factor = s.rho / s.eta_x;
        fit.b.iter_mut().flatten().for_each(|x| *x *= factor);
        z.iter_mut().flatten().for_each(|x| *x *= factor);
    }
    z
}

/// The weights, space and the variables turned with the space to principal axes, each
/// dimension signed so that the first object with a coordinate is positive: returns the
/// rotated space.
pub(crate) fn orient(fit: &mut Fit) -> Matrix {
    let z = space(fit);
    let rotation = mds::principal_rotation(&z);
    let mut turned = multiply(&z, &rotation);
    let signs = mds::first_positive(&mut turned);
    let mut b = multiply(&fit.b, &rotation);
    for row in &mut b {
        for (x, s) in row.iter_mut().zip(&signs) {
            *x *= s;
        }
    }
    fit.b = b;
    turned
}

/// The correlation of each of the first `h` variables with each dimension of `z`.
pub(crate) fn correlations(fit: &Fit, h: usize, z: &Matrix) -> Matrix {
    let n = z.len() as f64;
    let p = z.first().map_or(0, Vec::len);
    (0..h)
        .map(|j| {
            let q = column(&fit.q, j);
            let mq = q.iter().sum::<f64>() / n;
            let sq: f64 = q.iter().map(|v| (v - mq).powi(2)).sum();
            (0..p)
                .map(|d| {
                    let zd = column(z, d);
                    let mz = zd.iter().sum::<f64>() / n;
                    let sz: f64 = zd.iter().map(|v| (v - mz).powi(2)).sum();
                    let sqz: f64 = q.iter().zip(&zd).map(|(a, b)| (a - mq) * (b - mz)).sum();
                    match sq > 0.0 && sz > 0.0 {
                        true => sqz / (sq * sz).sqrt(),
                        false => f64::NAN,
                    }
                })
                .collect()
        })
        .collect()
}
