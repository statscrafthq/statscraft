//! The numerical core of PREFSCAL: unfolding of row and column objects by minimizing
//! penalized stress. Rules in our words (from the Command Syntax Reference and the published
//! definition of penalized stress; see docs/differences.md for the choices made here):
//! - The proximities sit in transformation lists: each row of each source (CONDITION=ROW),
//!   each source (MATRIX) or all of them (UNCONDITIONAL). Similarities are mirrored within
//!   each list, largest and smallest swapping places.
//! - For a list l with transformed proximities g and distances d (weights w), the normalized
//!   stress is s_l = |g - d|^2 / |g|^2 and the penalty mu_l = 1 + omega_l / v(g)^2, v the
//!   coefficient of variation and omega_l = OMEGA v(p)^2 of the list's proximities p.
//!   Penalized stress is F = S^LAMBDA M with S and M the means of s_l and mu_l over the lists.
//! - With the transformation fixed, F falls with S, which is raw stress with the weights
//!   w / (L |g_l|^2): one majorization (Guttman) step of the common space, and of the space
//!   weights in the individual differences models, lowers it.
//! - With the configuration fixed, one projected gradient step in the metric of the weights
//!   lowers F: the target g - eta grad / w goes to the nearest admissible transformation of
//!   every list. eta doubles after a step that lowers F and halves until one does.
//! - Restricted objects keep given coordinates (a missing one leaves that coordinate free). The
//!   majorization step then minimizes its quadratic over the free coordinates alone, dimension
//!   by dimension, with the fixed ones held: V_FF z_F = t_F - V_FX z_X. The step still lowers
//!   the stress, and the space is neither centered, rescaled nor rotated, so the fixed
//!   coordinates stay where they were given.
//! - The admissible transformations are never negative: NONE keeps the proximities (with
//!   INTERCEPT plus a constant), LINEAR a slope times them (with INTERCEPT plus a constant
//!   from the smallest one up), ORDINAL a monotone function (tied values kept tied, or free
//!   with UNTIE), SPLINE nonnegative combinations of a monotone spline basis.

use crate::matrix::{Matrix, inverse, multiply, symmetric_eigen, transpose};
use crate::mds::{self, Model, Problem, Transform, zeros};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    None,
    NoneIntercept,
    Linear,
    LinearIntercept,
    Ordinal {
        untie: bool,
    },
    Spline {
        intercept: bool,
        degree: usize,
        inknot: usize,
    },
}

/// One proximity: source k, row i, column j.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Entry {
    pub k: usize,
    pub i: usize,
    pub j: usize,
    pub delta: f64,
    pub w: f64,
}

/// A transformation list: its entries, spline or linear basis and penalty range.
#[derive(Debug, Clone)]
pub(crate) struct List {
    pub members: Vec<usize>,
    basis: Matrix,
    omega: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct Unfolding {
    pub n: usize,
    pub m: usize,
    pub sources: usize,
    pub entries: Vec<Entry>,
    pub lists: Vec<List>,
    pub kind: Kind,
    pub lambda: f64,
}

/// One solution: the common space (rows, then columns), the space weights, the transformed
/// proximities by entry and per iteration [penalized stress, stress part, penalty part].
#[derive(Debug, Clone)]
pub(crate) struct Fit {
    /// The start, centered and dilated.
    pub start: Matrix,
    pub z: Matrix,
    pub a: Vec<Matrix>,
    pub gamma: Vec<f64>,
    pub history: Vec<[f64; 3]>,
}

impl Fit {
    pub fn last(&self) -> [f64; 3] {
        *self.history.last().expect("the start is recorded")
    }
}

/// The squared coefficient of variation with weights; 0 unless the mean is positive.
fn nu2(x: impl Iterator<Item = (f64, f64)> + Clone) -> f64 {
    let (w, s1, s2) = x.fold((0.0, 0.0, 0.0), |(a, b, c), (v, w)| {
        (a + w, b + w * v, c + w * v * v)
    });
    match s1 > 0.0 {
        true => (w * s2 - s1 * s1) / (s1 * s1),
        false => 0.0,
    }
}

impl Unfolding {
    /// `shape` is (rows, columns, sources); `penalty` is (LAMBDA, OMEGA).
    pub fn new(
        (n, m, sources): (usize, usize, usize),
        mut entries: Vec<Entry>,
        groups: Vec<Vec<usize>>,
        kind: Kind,
        (lambda, omega): (f64, f64),
        similarities: bool,
    ) -> Unfolding {
        let mut lists = Vec::new();
        for members in groups.into_iter().filter(|g| !g.is_empty()) {
            if similarities {
                let (lo, hi) = members
                    .iter()
                    .map(|&e| entries[e].delta)
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                        (a.min(v), b.max(v))
                    });
                members
                    .iter()
                    .for_each(|&e| entries[e].delta = hi + lo - entries[e].delta);
            }
            let values: Vec<f64> = members.iter().map(|&e| entries[e].delta).collect();
            let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
            let basis = match kind {
                Kind::LinearIntercept => values.iter().map(|v| vec![1.0, v - lo]).collect(),
                Kind::Spline {
                    intercept,
                    degree,
                    inknot,
                } => {
                    let mut knots = mds::spline_knots(&values, degree, inknot);
                    if !intercept {
                        knots[..=degree].iter_mut().for_each(|k| *k = 0.0);
                    }
                    let rows = mds::spline_rows(&values, &knots, degree);
                    match intercept {
                        true => rows,
                        false => rows.into_iter().map(|r| r[1..].to_vec()).collect(),
                    }
                }
                _ => Matrix::new(),
            };
            let omega = omega * nu2(members.iter().map(|&e| (entries[e].delta, entries[e].w)));
            lists.push(List {
                members,
                basis,
                omega,
            });
        }
        Unfolding {
            n,
            m,
            sources,
            entries,
            lists,
            kind,
            lambda,
        }
    }

    /// The nearest admissible transformation of list `l` to `target`, by weighted least
    /// squares.
    fn project(&self, l: usize, target: &[f64]) -> Vec<f64> {
        let list = &self.lists[l];
        let e: Vec<&Entry> = list.members.iter().map(|&e| &self.entries[e]).collect();
        let delta: Vec<f64> = e.iter().map(|e| e.delta).collect();
        let w: Vec<f64> = e.iter().map(|e| e.w).collect();
        let dot = |a: &[f64], b: &[f64], c: &[f64]| -> f64 {
            a.iter().zip(b).zip(c).map(|((x, y), z)| x * y * z).sum()
        };
        match self.kind {
            Kind::None => delta,
            Kind::NoneIntercept => {
                let lo = delta.iter().copied().fold(f64::INFINITY, f64::min);
                let wsum: f64 = w.iter().sum();
                let shift: f64 = (0..delta.len())
                    .map(|i| w[i] * (target[i] - delta[i]))
                    .sum::<f64>()
                    / wsum;
                let c = shift.max(-lo);
                delta.iter().map(|d| d + c).collect()
            }
            Kind::Linear => {
                let den = dot(&w, &delta, &delta);
                let b = match den > 0.0 {
                    true => (dot(&w, &delta, target) / den).max(0.0),
                    false => 0.0,
                };
                delta.iter().map(|d| b * d).collect()
            }
            Kind::LinearIntercept | Kind::Spline { .. } => {
                let root: Vec<f64> = w.iter().map(|v| v.sqrt()).collect();
                let s: Matrix = list
                    .basis
                    .iter()
                    .zip(&root)
                    .map(|(row, r)| row.iter().map(|v| v * r).collect())
                    .collect();
                let y: Vec<f64> = target.iter().zip(&root).map(|(t, r)| t * r).collect();
                let b = mds::nnls(&s, &y);
                list.basis
                    .iter()
                    .map(|row| row.iter().zip(&b).map(|(x, c)| x * c).sum())
                    .collect()
            }
            Kind::Ordinal { untie } => {
                let mut order: Vec<usize> = (0..delta.len()).collect();
                order.sort_by(|&a, &b| {
                    let by = delta[a].total_cmp(&delta[b]);
                    match untie {
                        true => by.then(target[a].total_cmp(&target[b])),
                        false => by,
                    }
                });
                let mut blocks: Vec<Vec<usize>> = Vec::new();
                for &o in &order {
                    match blocks.last_mut() {
                        Some(block) if !untie && delta[block[0]] == delta[o] => block.push(o),
                        _ => blocks.push(vec![o]),
                    }
                }
                let (means, weights): (Vec<f64>, Vec<f64>) = blocks
                    .iter()
                    .map(|block| {
                        let ws: f64 = block.iter().map(|&o| w[o]).sum();
                        let s: f64 = block.iter().map(|&o| w[o] * target[o]).sum();
                        (if ws > 0.0 { s / ws } else { 0.0 }, ws)
                    })
                    .unzip();
                let mut out = vec![0.0; delta.len()];
                for (block, v) in blocks.iter().zip(mds::monotone(&means, &weights)) {
                    block.iter().for_each(|&o| out[o] = v.max(0.0));
                }
                out
            }
        }
    }

    /// The transformed proximities to start from: the proximities made admissible.
    pub fn start_gamma(&self) -> Vec<f64> {
        let mut gamma = vec![0.0; self.entries.len()];
        for (l, list) in self.lists.iter().enumerate() {
            let delta: Vec<f64> = list
                .members
                .iter()
                .map(|&e| self.entries[e].delta)
                .collect();
            for (&e, v) in list.members.iter().zip(self.project(l, &delta)) {
                gamma[e] = v;
            }
        }
        gamma
    }

    /// The distance of every entry in the spaces `x`.
    pub fn distances(&self, x: &[Matrix]) -> Vec<f64> {
        self.entries
            .iter()
            .map(|e| {
                let (r, c) = (&x[e.k][e.i], &x[e.k][self.n + e.j]);
                r.iter()
                    .zip(c)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>()
                    .sqrt()
            })
            .collect()
    }

    /// [penalized stress, mean normalized stress S, mean penalty M].
    pub fn parts(&self, gamma: &[f64], d: &[f64]) -> [f64; 3] {
        let l = self.lists.len() as f64;
        let (mut s, mut m) = (0.0, 0.0);
        for list in &self.lists {
            let (mut a, mut b) = (0.0, 0.0);
            for &e in &list.members {
                let w = self.entries[e].w;
                a += w * (gamma[e] - d[e]).powi(2);
                b += w * gamma[e] * gamma[e];
            }
            s += a / b;
            m += match list.omega > 0.0 {
                true => {
                    1.0 + list.omega
                        / nu2(list.members.iter().map(|&e| (gamma[e], self.entries[e].w)))
                }
                false => 1.0,
            };
        }
        let (s, m) = (s / l, m / l);
        [s.powf(self.lambda) * m, s, m]
    }

    /// The gradient of the penalized stress by entry, divided by the entry's weight.
    fn gradient(&self, gamma: &[f64], d: &[f64]) -> Vec<f64> {
        let l = self.lists.len() as f64;
        let [_, s, m] = self.parts(gamma, d);
        let mut out = vec![0.0; gamma.len()];
        for list in &self.lists {
            let (mut a, mut b, mut wsum, mut s1) = (0.0, 0.0, 0.0, 0.0);
            for &e in &list.members {
                let w = self.entries[e].w;
                a += w * (gamma[e] - d[e]).powi(2);
                b += w * gamma[e] * gamma[e];
                wsum += w;
                s1 += w * gamma[e];
            }
            let q = wsum * b - s1 * s1;
            for &e in &list.members {
                let ds = 2.0 * ((gamma[e] - d[e]) / b - a * gamma[e] / (b * b));
                let dinv = match list.omega > 0.0 {
                    true => 2.0 * s1 * (q - s1 * (wsum * gamma[e] - s1)) / (q * q),
                    false => 0.0,
                };
                out[e] = (self.lambda * s.powf(self.lambda - 1.0) * m * ds
                    + s.powf(self.lambda) * list.omega * dinv)
                    / l;
            }
        }
        out
    }

    /// The N x N matrices (N = rows + columns) of every source that hold `values` by entry
    /// between rows and columns.
    fn square(&self, values: &[f64]) -> Vec<Matrix> {
        let big = self.n + self.m;
        let mut out = vec![zeros(big, big); self.sources];
        for (e, v) in self.entries.iter().zip(values) {
            out[e.k][e.i][self.n + e.j] = *v;
            out[e.k][self.n + e.j][e.i] = *v;
        }
        out
    }

    /// The weights w / (L |g_l|^2) that make raw stress equal to S.
    fn weights(&self, gamma: &[f64]) -> Vec<f64> {
        let l = self.lists.len() as f64;
        let mut out = vec![0.0; gamma.len()];
        for list in &self.lists {
            let b: f64 = list
                .members
                .iter()
                .map(|&e| self.entries[e].w * gamma[e] * gamma[e])
                .sum();
            for &e in &list.members {
                out[e] = self.entries[e].w / (l * b);
            }
        }
        out
    }

    /// The scaling problem of `mds` with the weights for `gamma`.
    fn problem(&self, gamma: &[f64]) -> (Problem, Vec<Matrix>) {
        let dhat = self.square(gamma);
        let problem = Problem {
            n: self.n + self.m,
            delta: dhat.clone(),
            w: self.square(&self.weights(gamma)),
            transform: Transform::Ratio,
            groups: Vec::new(),
        };
        (problem, dhat)
    }

    /// Whether the rows and columns hang together through the proximities.
    pub fn connected(&self) -> bool {
        let gamma = vec![1.0; self.entries.len()];
        mds::mean_laplacian(&self.problem(&gamma).0).is_some()
    }

    /// Minimizes penalized stress from the common space `z` and the space weights `a`.
    pub fn iterate(
        &self,
        model: Model,
        criteria: &mds::Criteria,
        mut z: Matrix,
        mut a: Vec<Matrix>,
        mut gamma: Vec<f64>,
        restricted: Option<&[Vec<Option<f64>>]>,
    ) -> Fit {
        let (problem, dhat) = self.problem(&gamma);
        match restricted {
            Some(fixed) => {
                align_to(&mut z, fixed);
                pin(&mut z, fixed);
            }
            None => mds::dilate(&problem, &dhat, &mut z, &a),
        }
        let start = z.clone();
        let mut x = mds::spaces(&z, &a);
        let mut d = self.distances(&x);
        let total_w: f64 = self.entries.iter().map(|e| e.w).sum();
        let mut eta = self.lists.len() as f64
            * self
                .entries
                .iter()
                .zip(&gamma)
                .map(|(e, g)| e.w * g * g)
                .sum::<f64>()
            / total_w;
        let mut now = self.parts(&gamma, &d);
        let mut history = vec![now];
        let fixed = self.kind == Kind::None;
        for _ in 0..criteria.max_iterations {
            if now[0] <= criteria.min_stress {
                break;
            }
            let (problem, dhat) = self.problem(&gamma);
            let laplacians: Vec<Matrix> = problem.w.iter().map(mds::laplacian).collect();
            let Some((v, vinv)) = mds::mean_laplacian(&problem) else {
                break;
            };
            let same = model == Model::Identity;
            let c: Vec<Matrix> = (0..self.sources)
                .map(|k| mds::bx(&problem.w[k], &dhat[k], &x[k]))
                .collect();
            z = match restricted {
                Some(fixed) => pinned_step(&v, &vinv, &c, &z, fixed),
                None => mds::update_z(&laplacians, &vinv, same, &c, &z, &a),
            };
            if model != Model::Identity {
                a = mds::update_a(model, &laplacians, &c, &z, &a);
            }
            x = mds::spaces(&z, &a);
            d = self.distances(&x);
            let start = self.parts(&gamma, &d);
            if !fixed && start[1] > 0.0 {
                let grad: Vec<f64> = self
                    .gradient(&gamma, &d)
                    .iter()
                    .zip(&self.entries)
                    .map(|(g, e)| g / e.w)
                    .collect();
                for _ in 0..60 {
                    let target: Vec<f64> =
                        gamma.iter().zip(&grad).map(|(g, r)| g - eta * r).collect();
                    let mut next = gamma.clone();
                    for (l, list) in self.lists.iter().enumerate() {
                        let t: Vec<f64> = list.members.iter().map(|&e| target[e]).collect();
                        for (&e, v) in list.members.iter().zip(self.project(l, &t)) {
                            next[e] = v;
                        }
                    }
                    if self.parts(&next, &d)[0] < start[0] {
                        gamma = next;
                        eta *= 2.0;
                        break;
                    }
                    eta /= 2.0;
                }
            }
            let next = self.parts(&gamma, &d);
            let gain = now[0] - next[0];
            now = next;
            history.push(now);
            if now[0] <= criteria.min_stress || gain <= criteria.diff_stress * now[0] {
                break;
            }
        }
        Fit {
            start,
            z,
            a,
            gamma,
            history,
        }
    }

    /// The proximities averaged over the sources, n x m; a cell missing in all of them gets
    /// the mean of the others.
    fn mean_matrix(&self) -> Matrix {
        let (mut num, mut den) = (zeros(self.n, self.m), zeros(self.n, self.m));
        for e in &self.entries {
            num[e.i][e.j] += e.w * e.delta;
            den[e.i][e.j] += e.w;
        }
        let known: Vec<f64> = (0..self.n)
            .flat_map(|i| (0..self.m).map(move |j| (i, j)))
            .filter(|&(i, j)| den[i][j] > 0.0)
            .map(|(i, j)| num[i][j] / den[i][j])
            .collect();
        let mean = known.iter().sum::<f64>() / known.len().max(1) as f64;
        (0..self.n)
            .map(|i| {
                (0..self.m)
                    .map(|j| match den[i][j] > 0.0 {
                        true => num[i][j] / den[i][j],
                        false => mean,
                    })
                    .collect()
            })
            .collect()
    }

    /// The classical scaling start: the row-row and column-column distances are the middle
    /// of their triangle bounds (with `spearman` from the ranks within rows instead).
    pub fn classical(&self, spearman: bool, p: usize) -> Matrix {
        let (n, m) = (self.n, self.m);
        let mut dm = self.mean_matrix();
        if spearman {
            dm = dm.iter().map(|r| ranks(r)).collect();
        }
        let big = n + m;
        let mut full = zeros(big, big);
        for i in 0..n {
            for j in 0..m {
                full[i][n + j] = dm[i][j];
                full[n + j][i] = dm[i][j];
            }
        }
        let fill = |pairs: &dyn Fn(usize) -> (f64, f64), count: usize| -> f64 {
            match spearman {
                true => ((0..count)
                    .map(|t| {
                        let (a, b) = pairs(t);
                        (a - b).powi(2)
                    })
                    .sum::<f64>()
                    / count as f64)
                    .sqrt(),
                false => {
                    let (mut lo, mut hi) = (0.0_f64, f64::INFINITY);
                    for t in 0..count {
                        let (a, b) = pairs(t);
                        lo = lo.max((a - b).abs());
                        hi = hi.min(a + b);
                    }
                    (lo + hi) / 2.0
                }
            }
        };
        for a in 0..n {
            for b in 0..n {
                if a != b {
                    full[a][b] = fill(&|t| (dm[a][t], dm[b][t]), m);
                }
            }
        }
        for a in 0..m {
            for b in 0..m {
                if a != b {
                    full[n + a][n + b] = fill(&|t| (dm[t][a], dm[t][b]), n);
                }
            }
        }
        let ones: Matrix = (0..big)
            .map(|i| (0..big).map(|j| f64::from(u8::from(i != j))).collect())
            .collect();
        let problem = Problem {
            n: big,
            delta: vec![full.clone()],
            w: vec![ones],
            transform: Transform::Ratio,
            groups: Vec::new(),
        };
        mds::torgerson(&problem, &[full], p)
    }

    /// The Ross-Cliff start: the singular vectors of the double centered squared proximities,
    /// each set scaled by the roots of the singular values.
    pub fn ross_cliff(&self, p: usize) -> Matrix {
        let dm = self.mean_matrix();
        let sq: Matrix = dm
            .iter()
            .map(|r| r.iter().map(|v| v * v).collect())
            .collect();
        let (rows, cols) = (row_means(&sq), row_means(&transpose(&sq)));
        let all = rows.iter().sum::<f64>() / self.n as f64;
        let b: Matrix = (0..self.n)
            .map(|i| {
                (0..self.m)
                    .map(|j| -0.5 * (sq[i][j] - rows[i] - cols[j] + all))
                    .collect()
            })
            .collect();
        let (u, s, v) = svd(&b, p);
        stack(&scaled(&u, &s, 0.5), &scaled(&v, &s, 0.5))
    }

    /// The correspondence start: row and column scores of a correspondence analysis of the
    /// mirrored proximities, in symmetric normalization.
    pub fn correspondence(&self, p: usize) -> Matrix {
        let dm = self.mean_matrix();
        let (lo, hi) = dm
            .iter()
            .flatten()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), v| {
                (a.min(*v), b.max(*v))
            });
        // a hundredth of the range keeps every row and column in the analysis
        let f: Matrix = dm
            .iter()
            .map(|r| {
                r.iter()
                    .map(|v| hi - v + (hi - lo).max(1e-12) / 100.0)
                    .collect()
            })
            .collect();
        let total: f64 = f.iter().flatten().sum();
        let r: Vec<f64> = f
            .iter()
            .map(|row| row.iter().sum::<f64>() / total)
            .collect();
        let c: Vec<f64> = (0..self.m)
            .map(|j| f.iter().map(|row| row[j]).sum::<f64>() / total)
            .collect();
        let e: Matrix = (0..self.n)
            .map(|i| {
                (0..self.m)
                    .map(|j| (f[i][j] / total - r[i] * c[j]) / (r[i] * c[j]).sqrt())
                    .collect()
            })
            .collect();
        let (u, s, v) = svd(&e, p);
        let mut x = scaled(&u, &s, 0.5);
        let mut y = scaled(&v, &s, 0.5);
        x.iter_mut()
            .zip(&r)
            .for_each(|(row, ri)| row.iter_mut().for_each(|v| *v /= ri.sqrt()));
        y.iter_mut()
            .zip(&c)
            .for_each(|(row, cj)| row.iter_mut().for_each(|v| *v /= cj.sqrt()));
        stack(&x, &y)
    }

    /// The centroid start: the rows on the principal components of their proximities, every
    /// column at the centroid of the rows that have it among their `choices` smallest.
    pub fn centroids(&self, choices: usize, p: usize) -> Matrix {
        let dm = self.mean_matrix();
        let means = row_means(&transpose(&dm));
        let centered: Matrix = dm
            .iter()
            .map(|r| r.iter().zip(&means).map(|(v, mu)| v - mu).collect())
            .collect();
        let (u, s, _) = svd(&centered, p);
        let x = scaled(&u, &s, 1.0);
        let y: Matrix = (0..self.m)
            .map(|j| {
                let chosen: Vec<usize> = (0..self.n)
                    .filter(|&i| dm[i].iter().filter(|v| **v < dm[i][j]).count() < choices)
                    .collect();
                let from: Vec<usize> = match chosen.is_empty() {
                    true => (0..self.n).collect(),
                    false => chosen,
                };
                (0..p)
                    .map(|d| from.iter().map(|&i| x[i][d]).sum::<f64>() / from.len() as f64)
                    .collect()
            })
            .collect();
        stack(&x, &y)
    }
}

/// Puts the given coordinates in `z`.
fn pin(z: &mut Matrix, fixed: &[Vec<Option<f64>>]) {
    for (row, given) in z.iter_mut().zip(fixed) {
        for (v, g) in row.iter_mut().zip(given) {
            if let Some(g) = g {
                *v = *g;
            }
        }
    }
}

/// Moves, turns and scales `z` so that the objects with all their coordinates given lie as
/// near as possible to them (Procrustes), a start the fixed objects can live with. Nothing
/// moves with fewer than two such objects or when they leave no rotation.
fn align_to(z: &mut Matrix, fixed: &[Vec<Option<f64>>]) {
    let p = z.first().map_or(0, Vec::len);
    let whole: Vec<usize> = (0..z.len())
        .filter(|&i| fixed[i].iter().all(Option::is_some))
        .collect();
    if p == 0 || whole.len() < 2 {
        return;
    }
    let count = whole.len() as f64;
    let mean = |get: &dyn Fn(usize, usize) -> f64, d: usize| -> f64 {
        whole.iter().map(|&i| get(i, d)).sum::<f64>() / count
    };
    let target = |i: usize, d: usize| fixed[i][d].expect("whole");
    let from = |i: usize, d: usize| z[i][d];
    let (cz, cf): (Vec<f64>, Vec<f64>) = (0..p).map(|d| (mean(&from, d), mean(&target, d))).unzip();
    let x: Matrix = whole
        .iter()
        .map(|&i| (0..p).map(|d| z[i][d] - cz[d]).collect())
        .collect();
    let y: Matrix = whole
        .iter()
        .map(|&i| (0..p).map(|d| target(i, d) - cf[d]).collect())
        .collect();
    let (u, s, v) = svd(&multiply(&transpose(&x), &y), p);
    let spread: f64 = x.iter().flatten().map(|v| v * v).sum();
    if spread <= 0.0 || s.iter().any(|&v| v <= 1e-12 * s[0]) {
        return;
    }
    let turn = multiply(&u, &transpose(&v));
    let scale = s.iter().sum::<f64>() / spread;
    for row in z.iter_mut() {
        let centered: Vec<f64> = row.iter().zip(&cz).map(|(a, c)| a - c).collect();
        for d in 0..p {
            row[d] = scale * (0..p).map(|e| centered[e] * turn[e][d]).sum::<f64>() + cf[d];
        }
    }
}

/// The majorization step of the identity model with fixed coordinates: per dimension the free
/// coordinates solve V_FF z_F = t_F - V_FX z_X; a dimension without a fixed coordinate takes the
/// usual step.
fn pinned_step(
    v: &Matrix,
    vinv: &Matrix,
    c: &[Matrix],
    z: &Matrix,
    fixed: &[Vec<Option<f64>>],
) -> Matrix {
    let (n, p) = (z.len(), z.first().map_or(0, Vec::len));
    let m = c.len() as f64;
    let mut out = z.clone();
    for d in 0..p {
        let t: Vec<f64> = (0..n)
            .map(|i| c.iter().map(|ck| ck[i][d]).sum::<f64>() / m)
            .collect();
        let held: Vec<usize> = (0..n).filter(|&i| fixed[i][d].is_some()).collect();
        let free: Vec<usize> = (0..n).filter(|&i| fixed[i][d].is_none()).collect();
        if held.is_empty() {
            for i in 0..n {
                out[i][d] = (0..n).map(|j| vinv[i][j] * t[j]).sum();
            }
            continue;
        }
        for &i in &held {
            out[i][d] = fixed[i][d].expect("held");
        }
        if free.is_empty() {
            continue;
        }
        let block: Matrix = free
            .iter()
            .map(|&i| free.iter().map(|&j| v[i][j]).collect())
            .collect();
        let Some((inv, _)) = inverse(&block) else {
            continue;
        };
        let rhs: Vec<f64> = free
            .iter()
            .map(|&i| t[i] - held.iter().map(|&x| v[i][x] * out[x][d]).sum::<f64>())
            .collect();
        for (a, &i) in free.iter().enumerate() {
            out[i][d] = (0..free.len()).map(|b| inv[a][b] * rhs[b]).sum();
        }
    }
    out
}

fn row_means(m: &Matrix) -> Vec<f64> {
    m.iter()
        .map(|r| r.iter().sum::<f64>() / r.len().max(1) as f64)
        .collect()
}

/// Ranks with ties at their mean rank.
pub(crate) fn ranks(values: &[f64]) -> Vec<f64> {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let mut out = vec![0.0; values.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while end < order.len() && values[order[end]] == values[order[start]] {
            end += 1;
        }
        let rank = (start + end + 1) as f64 / 2.0;
        order[start..end].iter().for_each(|&o| out[o] = rank);
        start = end;
    }
    out
}

/// The leading p singular triples of `b` (U, values, V), from the eigenvectors of B'B.
fn svd(b: &Matrix, p: usize) -> (Matrix, Vec<f64>, Matrix) {
    let eigen = symmetric_eigen(&multiply(&transpose(b), b));
    let mut u = zeros(b.len(), p);
    let mut v = zeros(eigen.len(), p);
    let mut s = vec![0.0; p];
    for (d, (value, vector)) in eigen.iter().take(p).enumerate() {
        s[d] = value.max(0.0).sqrt();
        for (j, x) in vector.iter().enumerate() {
            v[j][d] = *x;
        }
        if s[d] > 0.0 {
            for (i, row) in b.iter().enumerate() {
                u[i][d] = row.iter().zip(vector).map(|(a, x)| a * x).sum::<f64>() / s[d];
            }
        }
    }
    (u, s, v)
}

fn scaled(u: &Matrix, s: &[f64], power: f64) -> Matrix {
    u.iter()
        .map(|r| r.iter().zip(s).map(|(v, x)| v * x.powf(power)).collect())
        .collect()
}

fn stack(x: &Matrix, y: &Matrix) -> Matrix {
    x.iter().chain(y).cloned().collect()
}

/// The fit measures of a solution, in the order of `MEASURES`.
pub(crate) const MEASURES: [&str; 16] = [
    "Penalized Stress",
    "Stress Part",
    "Penalty Part",
    "Normalized Stress",
    "Kruskal's Stress-I",
    "Kruskal's Stress-II",
    "Young's S-Stress-I",
    "Young's S-Stress-II",
    "Dispersion Accounted For",
    "Variance Accounted For",
    "Recovered Preference Orders",
    "Spearman's Rho",
    "Kendall's Tau-b",
    "Variation Proximities",
    "Variation Transformed Proximities",
    "Variation Distances",
];

impl Unfolding {
    pub fn measures(&self, fit: &Fit) -> [f64; 16] {
        let x = mds::spaces(&fit.z, &fit.a);
        let d = self.distances(&x);
        let g = &fit.gamma;
        let [f, s, m] = self.parts(g, &d);
        let e = &self.entries;
        let sum = |h: &dyn Fn(usize) -> f64| -> f64 { (0..e.len()).map(|t| e[t].w * h(t)).sum() };
        let wsum = sum(&|_| 1.0);
        let mean_d = sum(&|t| d[t]) / wsum;
        let mean_d2 = sum(&|t| d[t] * d[t]) / wsum;
        let misfit = sum(&|t| (g[t] - d[t]).powi(2));
        let smisfit = sum(&|t| (g[t] * g[t] - d[t] * d[t]).powi(2));
        let mean_g = sum(&|t| g[t]) / wsum;
        let cov = sum(&|t| (g[t] - mean_g) * (d[t] - mean_d));
        let var_g = sum(&|t| (g[t] - mean_g).powi(2));
        let var_d = sum(&|t| (d[t] - mean_d).powi(2));
        // the rows of every source, for the rank measures
        let mut rows: Vec<Vec<usize>> = vec![Vec::new(); self.sources * self.n];
        for (t, entry) in e.iter().enumerate() {
            rows[entry.k * self.n + entry.i].push(t);
        }
        let (mut agree, mut pairs) = (0.0, 0.0);
        let (mut rho, mut tau, mut counted) = (0.0, 0.0, 0.0);
        for row in rows.iter().filter(|r| r.len() >= 2) {
            let pd: Vec<f64> = row.iter().map(|&t| e[t].delta).collect();
            let dd: Vec<f64> = row.iter().map(|&t| d[t]).collect();
            let (mut concordant, mut discordant, mut tie_p, mut tie_d, mut all) =
                (0.0, 0.0, 0.0, 0.0, 0.0_f64);
            for a in 0..row.len() {
                for b in a + 1..row.len() {
                    let (sp, sd) = ((pd[a] - pd[b]).signum(), (dd[a] - dd[b]).signum());
                    all += 1.0;
                    match (pd[a] == pd[b], dd[a] == dd[b]) {
                        (true, true) => {
                            tie_p += 1.0;
                            tie_d += 1.0;
                        }
                        (true, false) => tie_p += 1.0,
                        (false, true) => {
                            tie_d += 1.0;
                            pairs += 1.0;
                        }
                        (false, false) => {
                            pairs += 1.0;
                            if sp == sd {
                                concordant += 1.0;
                                agree += 1.0;
                            } else {
                                discordant += 1.0;
                            }
                        }
                    }
                }
            }
            let denominator = ((all - tie_p) * (all - tie_d)).sqrt();
            if denominator > 0.0 {
                tau += (concordant - discordant) / denominator;
                rho += pearson(&ranks(&pd), &ranks(&dd));
                counted += 1.0;
            }
        }
        let nu = |v: &dyn Fn(usize) -> f64| nu2((0..e.len()).map(|t| (v(t), e[t].w))).sqrt();
        [
            f,
            s.powf(self.lambda),
            m,
            s,
            (misfit / sum(&|t| d[t] * d[t])).sqrt(),
            (misfit / var_d).sqrt(),
            (smisfit / sum(&|t| d[t].powi(4))).sqrt(),
            (smisfit / sum(&|t| (d[t] * d[t] - mean_d2).powi(2))).sqrt(),
            1.0 - s,
            cov * cov / (var_g * var_d),
            agree / pairs,
            rho / counted,
            tau / counted,
            nu(&|t| e[t].delta),
            nu(&|t| g[t]),
            nu(&|t| d[t]),
        ]
    }

    /// Each entry's share of the normalized stress, w (g - d)^2 / (L |g_l|^2).
    pub fn shares(&self, fit: &Fit) -> Vec<f64> {
        let d = self.distances(&mds::spaces(&fit.z, &fit.a));
        self.weights(&fit.gamma)
            .iter()
            .enumerate()
            .map(|(t, v)| v * (fit.gamma[t] - d[t]).powi(2))
            .collect()
    }
}

fn pearson(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len() as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let cov: f64 = a.iter().zip(b).map(|(x, y)| (x - ma) * (y - mb)).sum();
    let va: f64 = a.iter().map(|x| (x - ma).powi(2)).sum();
    let vb: f64 = b.iter().map(|y| (y - mb).powi(2)).sum();
    cov / (va * vb).sqrt()
}
