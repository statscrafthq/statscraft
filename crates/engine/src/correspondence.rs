//! CORRESPONDENCE: simple correspondence analysis of a two-way table.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (CORRESPONDENCE), in our
//! words:
//! - The table comes from two numeric variables with whole-number ranges (each case adds its
//!   weight to one cell; values are truncated to whole numbers, values outside the range and
//!   missing values leave the case out, a weight below 0 counts as 0) or, with ALL, from the
//!   first rows cases and columns variables (ROWCAT_ aside), where a missing or negative cell
//!   counts as 0 and WEIGHT does not apply.
//! - Rows (columns) in an EQUAL group are added into the group's first member, which stands
//!   for all of them. Supplementary rows and columns and empty ones stay out of the analysis;
//!   the margins and the grand total N are those of the active part.
//! - Z has the cells (g_ij - a_i b_j / N) / sqrt(a_i b_j): the chi-square measure and RCMEAN take
//!   g = f, a and b the row and column totals; RMEAN puts b = N/k2, CMEAN a = N/k1, RSUM scales
//!   every row to N/k1 and CSUM every column to N/k2, with a = N/k1 and b = N/k2. The masses are
//!   a/N and b/N.
//! - The singular value decomposition Z = K L' Lambda gives the singular values; at most
//!   min(k1, k2) - 1 dimensions, and those with a singular value up to sqrt(k1 k2) 1e-7 are
//!   dropped with a warning. Standard row scores are K over the root of the row mass, column
//!   scores L over the root of the column mass. Normalization q puts the power (1 + q)/2 of
//!   each singular value on the rows and (1 - q)/2 on the columns; PRINCIPAL puts it fully on
//!   both.
//! - The inertia of a point is the sum of its squared cells of Z, that of a dimension the
//!   square of its singular value. A point's share of a dimension is mass times standard score
//!   squared; a dimension's share of a point is that times the squared singular value over the
//!   point's inertia. Supplementary points are projected into the solution with the same Z
//!   cells and take no share of any dimension.
//! - Standard deviations and correlations come from the delta method under multinomial
//!   sampling of the active cells: the derivatives of the singular values and of the standard
//!   scores, then the standard deviation of a score is multiplied by the singular value raised to
//!   the power the normalization gives it.
//! - Sign: in each dimension the active row with the largest absolute score scores positive.

use crate::chart::{Axis, Category, Series, Statistic};
use crate::descriptives::{case_weights, number};
use crate::dist::chi_square_p;
use crate::matrix::transpose;
use crate::matrix_functions::thin_svd;
use crate::matrix_lang::Mat;
use crate::means::is_valid;
use crate::plot::{Plot, Point, PointGroup};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Values};
use statscraft_syntax::{
    Correspondence, CorrespondenceMeasure, CorrespondenceNormalization,
    CorrespondenceStandardize as Standardize, CorrespondenceTable, PlotLabels,
};
use std::ops::{Add, Div, Mul, Sub};

/// A number with its derivative, for the derivatives of Z.
#[derive(Clone, Copy, Debug)]
struct Dual {
    v: f64,
    d: f64,
}

impl Dual {
    fn c(v: f64) -> Dual {
        Dual { v, d: 0.0 }
    }

    fn sqrt(self) -> Dual {
        let r = self.v.sqrt();
        Dual {
            v: r,
            d: self.d / (2.0 * r),
        }
    }
}

impl Add for Dual {
    type Output = Dual;
    fn add(self, o: Dual) -> Dual {
        Dual {
            v: self.v + o.v,
            d: self.d + o.d,
        }
    }
}

impl Sub for Dual {
    type Output = Dual;
    fn sub(self, o: Dual) -> Dual {
        Dual {
            v: self.v - o.v,
            d: self.d - o.d,
        }
    }
}

impl Mul for Dual {
    type Output = Dual;
    fn mul(self, o: Dual) -> Dual {
        Dual {
            v: self.v * o.v,
            d: self.d * o.v + self.v * o.d,
        }
    }
}

impl Div for Dual {
    type Output = Dual;
    fn div(self, o: Dual) -> Dual {
        Dual {
            v: self.v / o.v,
            d: (self.d * o.v - self.v * o.d) / (o.v * o.v),
        }
    }
}

/// One cell of Z with the row and column metrics a and b: `f` the count, `fi` and `fj` the row
/// and column totals, `n` the grand total of the active table.
fn z_cell(st: Standardize, f: Dual, fi: Dual, fj: Dual, n: Dual, k: (f64, f64)) -> [Dual; 3] {
    let (r, c) = (n / Dual::c(k.0), n / Dual::c(k.1));
    let (g, a, b) = match st {
        Standardize::Rcmean => (f, fi, fj),
        Standardize::Rmean => (f, fi, c),
        Standardize::Cmean => (f, r, fj),
        Standardize::Rsum => (f * r / fi, r, c),
        Standardize::Csum => (f * c / fj, r, c),
    };
    [(g - a * b / n) / (a * b).sqrt(), a, b]
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Status {
    Active,
    Supplementary,
    /// In an EQUAL group, added into this member.
    Merged(usize),
    Empty,
}

/// What the analysis gives for the rows or the columns; NaN where a statistic does not apply.
#[derive(Debug)]
struct Points {
    status: Vec<Status>,
    mass: Vec<f64>,
    /// Normalized scores, [point][dimension].
    score: Vec<Vec<f64>>,
    inertia: Vec<f64>,
    of_point: Vec<Vec<f64>>,
    of_dimension: Vec<Vec<f64>>,
    /// Covariances of the normalized scores between dimensions, active points only.
    cov: Vec<Vec<Vec<f64>>>,
}

impl Points {
    fn new(status: Vec<Status>, p: usize) -> Points {
        let t = status.len();
        Points {
            status,
            mass: vec![f64::NAN; t],
            score: vec![vec![f64::NAN; p]; t],
            inertia: vec![f64::NAN; t],
            of_point: vec![vec![f64::NAN; p]; t],
            of_dimension: vec![vec![f64::NAN; p]; t],
            cov: vec![Vec::new(); t],
        }
    }
}

struct Setup {
    counts: Vec<Vec<f64>>,
    row_supplementary: Vec<bool>,
    column_supplementary: Vec<bool>,
    row_groups: Vec<Vec<usize>>,
    column_groups: Vec<Vec<usize>>,
    standardize: Standardize,
    /// DIMENSION when given; 2 otherwise.
    dimension: Option<usize>,
    /// The powers of the singular values on the row and the column scores.
    alpha: f64,
    beta: f64,
    /// Whether the standard deviations of the scores are wanted.
    scores: bool,
}

#[derive(Debug)]
struct Solution {
    /// The singular values of every dimension kept.
    singular: Vec<f64>,
    /// The dimensions of the solution.
    p: usize,
    total: f64,
    n: f64,
    k: (usize, usize),
    /// Covariances of the singular values.
    singular_cov: Vec<Vec<f64>>,
    rows: Points,
    columns: Points,
    warnings: Vec<String>,
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Statuses after merging the EQUAL groups into their first members in `f`, by rows.
fn merge(f: &mut [Vec<f64>], supplementary: &[bool], groups: &[Vec<usize>]) -> Vec<Status> {
    let mut status: Vec<Status> = supplementary
        .iter()
        .map(|&s| match s {
            true => Status::Supplementary,
            false => Status::Active,
        })
        .collect();
    for group in groups {
        let first = *group.iter().min().expect("a group has members");
        for &i in group.iter().filter(|&&i| i != first) {
            let moved = std::mem::take(&mut f[i]);
            for (x, y) in f[first].iter_mut().zip(&moved) {
                *x += y;
            }
            f[i] = vec![0.0; moved.len()];
            status[i] = Status::Merged(first);
        }
    }
    status
}

fn solve(s: &Setup) -> Result<Solution, String> {
    let mut f = s.counts.clone();
    let row_status = merge(&mut f, &s.row_supplementary, &s.row_groups);
    let mut ft = transpose(&f);
    let column_status = merge(&mut ft, &s.column_supplementary, &s.column_groups);
    let f = transpose(&ft);
    let (t1, t2) = (row_status.len(), column_status.len());
    // totals over the active part of the other side
    let totals = |rs: &[Status], cs: &[Status]| -> (Vec<f64>, Vec<f64>) {
        let rows = (0..t1)
            .map(|i| {
                (0..t2)
                    .filter(|&j| cs[j] == Status::Active)
                    .map(|j| f[i][j])
                    .sum()
            })
            .collect();
        let columns = (0..t2)
            .map(|j| {
                (0..t1)
                    .filter(|&i| rs[i] == Status::Active)
                    .map(|i| f[i][j])
                    .sum()
            })
            .collect();
        (rows, columns)
    };
    let (mut row_status, mut column_status) = (row_status, column_status);
    // twice: the second time without the empty ones, which a supplementary point may have its
    // only counts in
    for _ in 0..2 {
        let (row_total, column_total) = totals(&row_status, &column_status);
        for (status, total) in row_status
            .iter_mut()
            .zip(&row_total)
            .chain(column_status.iter_mut().zip(&column_total))
        {
            if matches!(status, Status::Active | Status::Supplementary) && *total <= 0.0 {
                *status = Status::Empty;
            }
        }
    }
    let (row_total, column_total) = totals(&row_status, &column_status);
    let ar: Vec<usize> = (0..t1)
        .filter(|&i| row_status[i] == Status::Active)
        .collect();
    let ac: Vec<usize> = (0..t2)
        .filter(|&j| column_status[j] == Status::Active)
        .collect();
    let (k1, k2) = (ar.len(), ac.len());
    if k1 < 2 || k2 < 2 {
        return Err(format!(
            "the analysis needs at least two active rows and two active columns with counts; there are {k1} and {k2}"
        ));
    }
    let k = (k1 as f64, k2 as f64);
    let n: f64 = ar.iter().map(|&i| row_total[i]).sum();
    let st = s.standardize;
    let c = Dual::c;
    let cell = |i: usize, j: usize| {
        z_cell(st, c(f[i][j]), c(row_total[i]), c(column_total[j]), c(n), k).map(|d| d.v)
    };
    let mut z = Mat::filled(k1, k2, 0.0);
    let mut row_mass = vec![0.0; k1];
    let mut column_mass = vec![0.0; k2];
    for (a, &i) in ar.iter().enumerate() {
        for (b, &j) in ac.iter().enumerate() {
            let [v, ra, cb] = cell(i, j);
            z.set(a, b, v);
            row_mass[a] = ra / n;
            column_mass[b] = cb / n;
        }
    }
    let (mut u, q, mut v) = thin_svd(&z);
    let lambda: Vec<f64> = (0..k1.min(k2)).map(|t| q.at(t, t)).collect();
    let tolerance = k.0.sqrt() * k.1.sqrt() * 1e-7;
    let rank = lambda.iter().filter(|&&l| l > tolerance).count();
    let max_dims = k1.min(k2) - 1;
    // every dimension with a singular value above 0 is listed, also one beyond max_dims that
    // a standardization removing only row or only column means leaves
    if rank == 0 {
        return Err("every singular value is zero: the rows do not differ from each other".into());
    }
    let mut warnings = Vec::new();
    if let Some(d) = s.dimension
        && d > max_dims
    {
        warnings.push(format!(
            "CORRESPONDENCE: the table allows at most {max_dims} dimension{}, so {} computed",
            if max_dims == 1 { "" } else { "s" },
            max_dims.min(rank),
        ));
    }
    if rank < max_dims {
        warnings.push(format!(
            "CORRESPONDENCE: {} dimension{} with a singular value near zero left out",
            max_dims - rank,
            if max_dims - rank == 1 { "" } else { "s" }
        ));
    }
    let p = s.dimension.unwrap_or(2).min(max_dims).min(rank);
    let total: f64 = z.data.iter().map(|x| x * x).sum();
    // the sign: the active row with the largest absolute standard score scores positive
    for t in 0..rank {
        let mut best = (0, 0.0_f64);
        for (a, m) in row_mass.iter().enumerate() {
            let x = u.at(a, t) / m.sqrt();
            if x.abs() > best.1.abs() * (1.0 + 1e-9) {
                best = (a, x);
            }
        }
        if best.1 < 0.0 {
            for a in 0..k1 {
                u.set(a, t, -u.at(a, t));
            }
            for b in 0..k2 {
                v.set(b, t, -v.at(b, t));
            }
        }
    }
    let col = |m: &Mat, t: usize| -> Vec<f64> { (0..m.rows).map(|i| m.at(i, t)).collect() };
    let us: Vec<Vec<f64>> = (0..rank).map(|t| col(&u, t)).collect();
    let vs: Vec<Vec<f64>> = (0..rank).map(|t| col(&v, t)).collect();

    // points: standard scores of active points from K and L, supplementary ones projected
    let mut rows = Points::new(row_status.clone(), p);
    let mut columns = Points::new(column_status.clone(), p);
    let (alpha, beta) = (s.alpha, s.beta);
    let place = |points: &mut Points,
                 at: usize,
                 mass: f64,
                 cells: &[f64],
                 projected: &[f64],
                 active: bool,
                 power: f64| {
        let inertia: f64 = cells.iter().map(|x| x * x).sum();
        points.mass[at] = mass;
        points.inertia[at] = inertia;
        for t in 0..p {
            // projected[t] is the singular vector element; its square is the point's share
            let standard = projected[t] / mass.sqrt();
            points.score[at][t] = standard * lambda[t].powf(power);
            points.of_point[at][t] = if active {
                projected[t] * projected[t]
            } else {
                0.0
            };
            points.of_dimension[at][t] = if inertia > 0.0 {
                projected[t] * projected[t] * lambda[t] * lambda[t] / inertia
            } else {
                f64::NAN
            };
        }
    };
    // the place of every active row (column) among the active ones
    let mut position = vec![0; t1.max(t2)];
    for (a, &i) in ar.iter().enumerate() {
        position[i] = a;
    }
    for (i, status) in row_status.iter().enumerate() {
        match status {
            Status::Active => {
                let a = position[i];
                let cells: Vec<f64> = (0..k2).map(|b| z.at(a, b)).collect();
                let element: Vec<f64> = (0..p).map(|t| us[t][a]).collect();
                place(&mut rows, i, row_mass[a], &cells, &element, true, alpha);
            }
            Status::Supplementary => {
                let parts: Vec<[f64; 3]> = ac.iter().map(|&j| cell(i, j)).collect();
                let cells: Vec<f64> = parts.iter().map(|x| x[0]).collect();
                let element: Vec<f64> = (0..p).map(|t| dot(&cells, &vs[t]) / lambda[t]).collect();
                place(
                    &mut rows,
                    i,
                    parts[0][1] / n,
                    &cells,
                    &element,
                    false,
                    alpha,
                );
            }
            _ => {}
        }
    }
    for (b, &j) in ac.iter().enumerate() {
        position[j] = b;
    }
    for (j, status) in column_status.iter().enumerate() {
        match status {
            Status::Active => {
                let b = position[j];
                let cells: Vec<f64> = (0..k1).map(|a| z.at(a, b)).collect();
                let element: Vec<f64> = (0..p).map(|t| vs[t][b]).collect();
                place(
                    &mut columns,
                    j,
                    column_mass[b],
                    &cells,
                    &element,
                    true,
                    beta,
                );
            }
            Status::Supplementary => {
                let parts: Vec<[f64; 3]> = ar.iter().map(|&i| cell(i, j)).collect();
                let cells: Vec<f64> = parts.iter().map(|x| x[0]).collect();
                let element: Vec<f64> = (0..p).map(|t| dot(&cells, &us[t]) / lambda[t]).collect();
                place(
                    &mut columns,
                    j,
                    parts[0][2] / n,
                    &cells,
                    &element,
                    false,
                    beta,
                );
            }
            _ => {}
        }
    }
    for points in [&mut rows, &mut columns] {
        for i in 0..points.status.len() {
            if let Status::Merged(first) = points.status[i] {
                points.score[i] = points.score[first].clone();
            }
        }
    }

    // The delta method: derivatives of the statistics in each active cell proportion p_kl.
    // A change of p_kl moves the cell itself, row k's total, column l's total and the grand
    // total, so dZ = zf [i=k][j=l] + zfi [i=k] + zfj [j=l] + zn, with the four partial
    // derivatives of every cell of Z taken once; the masses depend on their own total and N.
    let pi: Vec<Vec<f64>> = ar
        .iter()
        .map(|&i| ac.iter().map(|&j| f[i][j] / n).collect())
        .collect();
    let rp: Vec<f64> = ar.iter().map(|&i| row_total[i] / n).collect();
    let cp: Vec<f64> = ac.iter().map(|&j| column_total[j] / n).collect();
    let seeded = |a: usize, b: usize, seed: [f64; 4]| {
        let d = |v: f64, d: f64| Dual { v, d };
        let total = d(1.0, seed[3]);
        let [z, ra, cb] = z_cell(
            st,
            d(pi[a][b], seed[0]),
            d(rp[a], seed[1]),
            d(cp[b], seed[2]),
            total,
            k,
        );
        [z.d, (ra / total).d, (cb / total).d]
    };
    let partial = |which: usize| -> Vec<Vec<f64>> {
        let mut seed = [0.0; 4];
        seed[which] = 1.0;
        (0..k1)
            .map(|a| (0..k2).map(|b| seeded(a, b, seed)[0]).collect())
            .collect()
    };
    let (zf, zfi, zfj, zn) = (partial(0), partial(1), partial(2), partial(3));
    // the row masses in their own total and in N, the column masses likewise
    let row_dm: Vec<[f64; 2]> = (0..k1)
        .map(|a| {
            [
                seeded(a, 0, [0.0, 1.0, 0.0, 0.0])[1],
                seeded(a, 0, [0.0, 0.0, 0.0, 1.0])[1],
            ]
        })
        .collect();
    let column_dm: Vec<[f64; 2]> = (0..k2)
        .map(|b| {
            [
                seeded(0, b, [0.0, 0.0, 1.0, 0.0])[2],
                seeded(0, b, [0.0, 0.0, 0.0, 1.0])[2],
            ]
        })
        .collect();
    let rows_by = |m: &[Vec<f64>], v: &[f64]| -> Vec<f64> { m.iter().map(|r| dot(r, v)).collect() };
    let columns_by = |m: &[Vec<f64>], u: &[f64]| -> Vec<f64> {
        (0..k2)
            .map(|b| (0..k1).map(|a| m[a][b] * u[a]).sum())
            .collect()
    };
    // a derivative of a singular value or vector needs that value to be distinct
    let tied: Vec<bool> = (0..rank)
        .map(|t| {
            (0..rank).any(|o| {
                o != t && (lambda[t].powi(2) - lambda[o].powi(2)).abs() <= 1e-8 * lambda[t].powi(2)
            })
        })
        .collect();
    let feasible = score_work_fits(k1, k2, rank, p);
    if s.scores && !feasible {
        warnings.push(format!(
            "CORRESPONDENCE: {k1} by {k2} active categories are too many for standard deviations of the scores"
        ));
    }
    let scores = s.scores && feasible;
    if tied.iter().any(|&x| x) {
        warnings.push(
            "CORRESPONDENCE: equal singular values have no standard deviations, nor do the scores of their dimensions"
                .into(),
        );
    }
    // dZ v_t = [i=k] (zf_kl v_l + rfi_k) + zfj_il v_l + nv_i, and dZ' u_t likewise
    struct Dim {
        rfi: Vec<f64>,
        cfj: Vec<f64>,
        nv: Vec<f64>,
        /// zn' u_t, for the scores only
        nu: Vec<f64>,
        /// u_t . nv
        unv: f64,
        /// For the scores: du = dZ v / lambda + the sums over the other singular vectors, each
        /// a table indexed by the changed cell's row or column, then the element. The fourth
        /// table of dv is the transpose of that of du, which stands for it.
        du: [Vec<Vec<f64>>; 6],
        dv: [Vec<Vec<f64>>; 5],
    }
    // out[x][i] = sum over o of by[o][x] w[o] over[o][i]
    let combine = |by: &[Vec<f64>], w: &[f64], over: &[Vec<f64>]| -> Vec<Vec<f64>> {
        let size = over.first().map_or(0, Vec::len);
        (0..by.first().map_or(0, Vec::len))
            .map(|x| {
                let mut out = vec![0.0; size];
                for o in 0..rank {
                    let c = by[o][x] * w[o];
                    if c != 0.0 {
                        for (y, z) in out.iter_mut().zip(&over[o]) {
                            *y += c * z;
                        }
                    }
                }
                out
            })
            .collect()
    };
    // sums of (u_o)_i zfj_il over i and of zfi_kj (v_o)_j over j
    let (q1, q3): (Vec<Vec<f64>>, Vec<Vec<f64>>) = match scores {
        true => (
            us.iter().map(|u| columns_by(&zfj, u)).collect(),
            vs.iter().map(|v| rows_by(&zfi, v)).collect(),
        ),
        false => (Vec::new(), Vec::new()),
    };
    let dims: Vec<Dim> = (0..rank)
        .map(|t| {
            let wanted = scores && t < p && !tied[t];
            let nv = rows_by(&zn, &vs[t]);
            let nu = match wanted {
                true => columns_by(&zn, &us[t]),
                false => Vec::new(),
            };
            let mut dim = Dim {
                rfi: rows_by(&zfi, &vs[t]),
                cfj: columns_by(&zfj, &us[t]),
                unv: dot(&us[t], &nv),
                nv,
                nu,
                du: Default::default(),
                dv: Default::default(),
            };
            if wanted {
                let ls = lambda[t];
                // the coefficient of u_o is wa_o (u_o' dZ v_t) + wb_o (u_t' dZ v_o), and that of
                // v_o is wa_o (u_t' dZ v_o) + wb_o (u_o' dZ v_t); the -1/lambda part takes the
                // null space as I - U U'
                let wa: Vec<f64> = (0..rank)
                    .map(|o| match o == t {
                        true => -1.0 / ls,
                        false => -1.0 / ls + ls / (ls * ls - lambda[o] * lambda[o]),
                    })
                    .collect();
                let wb: Vec<f64> = (0..rank)
                    .map(|o| match o == t {
                        true => 0.0,
                        false => lambda[o] / (ls * ls - lambda[o] * lambda[o]),
                    })
                    .collect();
                let q2: Vec<Vec<f64>> = us.iter().map(|u| vec![dot(u, &dim.nv)]).collect();
                let q4: Vec<Vec<f64>> = vs.iter().map(|v| vec![dot(&dim.nu, v)]).collect();
                dim.du = [
                    combine(&us, &wa, &us),
                    combine(&q1, &wa, &us),
                    combine(&q2, &wa, &us),
                    combine(&vs, &wb, &us),
                    combine(&q3, &wb, &us),
                    combine(&q4, &wb, &us),
                ];
                dim.dv = [
                    combine(&vs, &wa, &vs),
                    combine(&q3, &wa, &vs),
                    combine(&q4, &wa, &vs),
                    combine(&q1, &wb, &vs),
                    combine(&q2, &wb, &vs),
                ];
            }
            dim
        })
        .collect();
    let root =
        |m: &[f64]| -> Vec<[f64; 2]> { m.iter().map(|m| [1.0 / m.sqrt(), m.powf(-1.5)]).collect() };
    let (row_root, column_root) = (root(&row_mass), root(&column_mass));
    // weighted sums of the gradients and of their products, for the covariances
    let mut lambda_means = vec![0.0; rank];
    let mut lambda_squares = vec![0.0; rank];
    let mut lambda_pairs = (vec![0.0; p], vec![vec![0.0; p]; p]);
    let mut row_sums = vec![(vec![0.0; p], vec![vec![0.0; p]; p]); k1];
    let mut column_sums = vec![(vec![0.0; p], vec![vec![0.0; p]; p]); k2];
    let add = |sums: &mut (Vec<f64>, Vec<Vec<f64>>), g: &[f64], weight: f64| {
        for (s, gs) in g.iter().enumerate() {
            sums.0[s] += weight * gs;
            for (t, gt) in g.iter().enumerate() {
                sums.1[s][t] += weight * gs * gt;
            }
        }
    };
    let mut gl = vec![0.0; rank];
    let mut gx = vec![vec![0.0; p]; k1];
    let mut gy = vec![vec![0.0; p]; k2];
    for kr in 0..k1 {
        for kc in 0..k2 {
            let weight = pi[kr][kc];
            if weight == 0.0 {
                continue;
            }
            let own = zf[kr][kc];
            for (t, g) in gl.iter_mut().enumerate() {
                let (u, v, q) = (&us[t], &vs[t], &dims[t]);
                *g = u[kr] * (own * v[kc] + q.rfi[kr]) + v[kc] * q.cfj[kc] + q.unv;
            }
            // squares of every singular value gradient, products only among the first p
            for (t, g) in gl.iter().enumerate() {
                lambda_means[t] += weight * g;
                lambda_squares[t] += weight * g * g;
            }
            add(&mut lambda_pairs, &gl[..p], weight);
            if !scores {
                continue;
            }
            for t in (0..p).filter(|&t| !tied[t]) {
                let (u, v, q) = (&us[t], &vs[t], &dims[t]);
                let ls = lambda[t];
                // the changed cell's parts of dZ v_t (its row) and dZ' u_t (its column)
                let row_part = own * v[kc] + q.rfi[kr];
                let column_part = own * u[kr] + q.cfj[kc];
                let [a1, a2, a3, a4, a5, a6] = &q.du;
                for (a, g) in gx.iter_mut().enumerate() {
                    let w = zfj[a][kc] * v[kc] + q.nv[a] + if a == kr { row_part } else { 0.0 };
                    let du = w / ls
                        + a1[kr][a] * row_part
                        + a2[kc][a] * v[kc]
                        + a3[0][a]
                        + a4[kc][a] * column_part
                        + a5[kr][a] * u[kr]
                        + a6[0][a];
                    let dm = row_dm[a][0] * f64::from(u8::from(a == kr)) + row_dm[a][1];
                    g[t] = du * row_root[a][0] - 0.5 * u[a] * row_root[a][1] * dm;
                }
                let [b1, b2, b3, b5, b6] = &q.dv;
                for (b, g) in gy.iter_mut().enumerate() {
                    let w = zfi[kr][b] * u[kr] + q.nu[b] + if b == kc { column_part } else { 0.0 };
                    let dv = w / ls
                        + b1[kc][b] * column_part
                        + b2[kr][b] * u[kr]
                        + b3[0][b]
                        + a4[b][kr] * row_part
                        + b5[kc][b] * v[kc]
                        + b6[0][b];
                    let dm = column_dm[b][0] * f64::from(u8::from(b == kc)) + column_dm[b][1];
                    g[t] = dv * column_root[b][0] - 0.5 * v[b] * column_root[b][1] * dm;
                }
            }
            for (sums, g) in row_sums.iter_mut().zip(&gx) {
                add(sums, g, weight);
            }
            for (sums, g) in column_sums.iter_mut().zip(&gy) {
                add(sums, g, weight);
            }
        }
    }
    let covariances = |sums: &(Vec<f64>, Vec<Vec<f64>>), scale: &dyn Fn(usize) -> f64| {
        let d = sums.0.len();
        (0..d)
            .map(|s| {
                (0..d)
                    .map(|t| (sums.1[s][t] - sums.0[s] * sums.0[t]) / n * scale(s) * scale(t))
                    .collect::<Vec<f64>>()
            })
            .collect::<Vec<Vec<f64>>>()
    };
    let pairs = covariances(&lambda_pairs, &|_| 1.0);
    let singular_cov: Vec<Vec<f64>> = (0..rank)
        .map(|t| {
            (0..rank)
                .map(|o| {
                    if tied[t] || tied[o] {
                        f64::NAN
                    } else if o == t {
                        (lambda_squares[t] - lambda_means[t] * lambda_means[t]) / n
                    } else if o < p && t < p {
                        pairs[t][o]
                    } else {
                        // only the pairs among the first p dimensions were summed
                        f64::NAN
                    }
                })
                .collect()
        })
        .collect();
    // NaN where the scores were not wanted or their dimension is tied
    let score_cov = |sums: &(Vec<f64>, Vec<Vec<f64>>), power: f64| -> Vec<Vec<f64>> {
        let mut c = covariances(sums, &|t| lambda[t].powf(power));
        for (t, row) in c.iter_mut().enumerate() {
            for (o, x) in row.iter_mut().enumerate() {
                if !scores || tied[t] || tied[o] {
                    *x = f64::NAN;
                }
            }
        }
        c
    };
    for (a, &i) in ar.iter().enumerate() {
        rows.cov[i] = score_cov(&row_sums[a], alpha);
    }
    for (b, &j) in ac.iter().enumerate() {
        columns.cov[j] = score_cov(&column_sums[b], beta);
    }
    Ok(Solution {
        singular: lambda[..rank].to_vec(),
        p,
        total,
        n,
        k: (k1, k2),
        singular_cov,
        rows,
        columns,
        warnings,
    })
}

/// One side of the table as the output names it.
struct Side {
    /// The variable as tables show it, or Row / Column for ALL.
    title: String,
    /// The variable's name for OUTFILE.
    name: String,
    labels: Vec<String>,
}

fn text(s: impl Into<String>) -> Cell {
    Cell::Text(s.into())
}

fn nan_cell(x: f64) -> Cell {
    number(Some(x))
}

/// A table whose columns have the group labels in `groups`, all of the same depth.
fn grouped(title: &str, columns: Vec<(Vec<&str>, String)>, rows: Vec<Vec<Cell>>) -> Table {
    Table {
        title: title.into(),
        row_headers: 1,
        column_groups: columns
            .iter()
            .map(|(g, _)| g.iter().map(|s| s.to_string()).collect())
            .collect(),
        columns: columns.into_iter().map(|(_, c)| c).collect(),
        rows,
        ..Table::default()
    }
}

fn correspondence_table(
    title: &str,
    rows: &Side,
    columns: &Side,
    cells: Vec<Vec<Cell>>,
    last_column: &str,
) -> Table {
    let mut header = vec![(vec![""], rows.title.clone())];
    header.extend(
        columns
            .labels
            .iter()
            .map(|l| (vec![columns.title.as_str()], l.clone())),
    );
    header.push((vec![""], last_column.to_string()));
    grouped(title, header, cells)
}

fn normalization_text(n: CorrespondenceNormalization) -> String {
    match n {
        CorrespondenceNormalization::Principal => "Principal normalization.".into(),
        CorrespondenceNormalization::Q(0.0) => "Symmetrical normalization.".into(),
        CorrespondenceNormalization::Q(1.0) => "Row principal normalization.".into(),
        CorrespondenceNormalization::Q(-1.0) => "Column principal normalization.".into(),
        CorrespondenceNormalization::Q(q) => format!("Normalization {q}."),
    }
}

fn overview(title: &str, side: &Side, points: &Points, p: usize, caption: String) -> Table {
    let dims: Vec<String> = (1..=p).map(|s| s.to_string()).collect();
    let mut header = vec![
        (vec!["", ""], side.title.clone()),
        (vec!["", ""], "Mass".to_string()),
    ];
    header.extend(
        dims.iter()
            .map(|d| (vec!["", "Score in Dimension"], d.clone())),
    );
    header.push((vec!["", ""], "Inertia".into()));
    let point = "Of Point to Inertia of Dimension";
    let dimension = "Of Dimension to Inertia of Point";
    header.extend(
        dims.iter()
            .map(|d| (vec!["Contribution", point], d.clone())),
    );
    header.extend(
        dims.iter()
            .map(|d| (vec!["Contribution", dimension], d.clone())),
    );
    header.push((vec!["Contribution", dimension], "Total".into()));
    let mut rows = Vec::new();
    let (mut mass, mut inertia) = (0.0, 0.0);
    let mut shares = vec![0.0; p];
    for (i, label) in side.labels.iter().enumerate() {
        let mut row = vec![text(label.clone())];
        let status = points.status[i];
        let shown = matches!(status, Status::Active | Status::Supplementary);
        let when = |x: f64| if shown { nan_cell(x) } else { Cell::Empty };
        row.push(when(points.mass[i]));
        row.extend(points.score[i].iter().map(|&x| nan_cell(x)));
        row.push(when(points.inertia[i]));
        row.extend(points.of_point[i].iter().map(|&x| when(x)));
        row.extend(points.of_dimension[i].iter().map(|&x| when(x)));
        let sum: f64 = points.of_dimension[i].iter().sum();
        row.push(when(sum));
        if status == Status::Active {
            mass += points.mass[i];
            inertia += points.inertia[i];
            for (s, x) in shares.iter_mut().zip(&points.of_point[i]) {
                *s += x;
            }
        }
        rows.push(row);
    }
    let mut total = vec![text("Active Total"), Cell::Number(mass)];
    total.extend((0..p).map(|_| Cell::Empty));
    total.push(Cell::Number(inertia));
    total.extend(shares.iter().map(|&x| Cell::Number(x)));
    total.extend((0..=p).map(|_| Cell::Empty));
    rows.push(total);
    let mut t = grouped(title, header, rows);
    t.caption = Some(caption);
    t
}

fn confidence(title: &str, side: &Side, points: &Points, p: usize) -> Table {
    let mut header = vec![(vec![""], side.title.clone())];
    header.extend((1..=p).map(|s| (vec!["Standard Deviation in Dimension"], s.to_string())));
    let pairs: Vec<(usize, usize)> = (0..p)
        .flat_map(|s| (s + 1..p).map(move |t| (s, t)))
        .collect();
    header.extend(
        pairs
            .iter()
            .map(|(s, t)| (vec!["Correlation"], format!("{}-{}", s + 1, t + 1))),
    );
    let rows = side
        .labels
        .iter()
        .enumerate()
        .filter(|(i, _)| points.status[*i] == Status::Active)
        .map(|(i, label)| {
            let c = &points.cov[i];
            let mut row = vec![text(label.clone())];
            row.extend((0..p).map(|s| nan_cell(c[s][s].sqrt())));
            row.extend(
                pairs
                    .iter()
                    .map(|&(s, t)| nan_cell(c[s][t] / (c[s][s] * c[t][t]).sqrt())),
            );
            row
        })
        .collect();
    grouped(title, header, rows)
}

/// Whether the standard deviations of the scores take a reasonable time and memory: the
/// combined tables hold about 2 (k1 + k2)^2 numbers per dimension and cost the rank times that
/// to build, and every cell of the table then costs k1 + k2 per dimension and per pair of
/// dimensions.
fn score_work_fits(k1: usize, k2: usize, rank: usize, p: usize) -> bool {
    let side = (k1 + k2) as f64;
    let tables = 2.0 * side * side * p as f64;
    // building the tables, then for every cell each point's gradient and its products
    let cells = (k1 * k2) as f64;
    let work = tables * rank as f64 + cells * side * (p + p * p) as f64;
    tables <= 25e6 && work <= 2e9
}

/// A limit on the cells of the table, which is held in full.
fn too_large(rows: usize, columns: usize) -> Result<(), Error> {
    match rows.saturating_mul(columns) > 1_000_000 {
        true => Err(Error(format!(
            "CORRESPONDENCE: a table of {rows} by {columns} categories is too large (a million cells at most)"
        ))),
        false => Ok(()),
    }
}

fn short(label: &str, length: Option<usize>) -> Option<String> {
    let label: String = match length {
        Some(l) => label.chars().take(l).collect(),
        None => label.to_string(),
    };
    (!label.is_empty()).then_some(label)
}

impl Engine {
    pub(crate) fn correspondence(&mut self, command: &Correspondence) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let mut warnings = Vec::new();
        // the table, its category values (for SUPPLEMENTARY and EQUAL) and its labels
        let (counts, row_values, column_values, rows, columns) = match &command.table {
            CorrespondenceTable::Variables {
                row,
                row_range,
                column,
                column_range,
            } => {
                let rv = data.variable(row)?;
                let cv = data.variable(column)?;
                if let Some(v) = [&rv, &cv].into_iter().find(|v| v.is_string()) {
                    return Err(Error(format!(
                        "CORRESPONDENCE needs numeric variables, but {} is a string variable",
                        v.name()
                    )));
                }
                let values = |(min, max): (f64, f64)| -> Vec<f64> {
                    (0..=(max - min) as usize).map(|k| min + k as f64).collect()
                };
                let (row_values, column_values) = (values(*row_range), values(*column_range));
                too_large(row_values.len(), column_values.len())?;
                let weights = case_weights(data)?;
                let index =
                    |var: &statscraft_data::Variable, case: usize, (min, max): (f64, f64)| {
                        if !is_valid(var, case, false) {
                            return None;
                        }
                        let Values::Numeric(values) = var.values() else {
                            return None;
                        };
                        let x = values.get(case)?.trunc();
                        (x >= min && x <= max).then_some((x - min) as usize)
                    };
                let mut counts = vec![vec![0.0; column_values.len()]; row_values.len()];
                for case in 0..data.case_count() {
                    let w = weights.as_ref().map_or(1.0, |w| w[case]);
                    if w <= 0.0 {
                        continue;
                    }
                    if let (Some(i), Some(j)) = (
                        index(&rv, case, *row_range),
                        index(&cv, case, *column_range),
                    ) {
                        counts[i][j] += w;
                    }
                }
                let side = |var: &statscraft_data::Variable, values: &[f64]| Side {
                    title: shown.variable(var),
                    name: var.name().to_string(),
                    labels: values.iter().map(|&v| shown.number(var, v)).collect(),
                };
                let (rows, columns) = (side(&rv, &row_values), side(&cv, &column_values));
                (counts, row_values, column_values, rows, columns)
            }
            CorrespondenceTable::All {
                rows: r,
                columns: c,
            } => {
                let category = data
                    .variables()
                    .find(|v| v.name().eq_ignore_ascii_case("ROWCAT_"));
                let vars: Vec<statscraft_data::Variable> = data
                    .variables()
                    .filter(|v| !v.name().eq_ignore_ascii_case("ROWCAT_"))
                    .take(*c)
                    .collect();
                if vars.len() < *c {
                    return Err(Error(format!(
                        "CORRESPONDENCE TABLE=ALL asks for {c} columns, but the data have {} variables besides ROWCAT_",
                        vars.len()
                    )));
                }
                too_large(*r, *c)?;
                if *r > data.case_count() {
                    return Err(Error(format!(
                        "CORRESPONDENCE TABLE=ALL asks for {r} rows, but the data have {} cases",
                        data.case_count()
                    )));
                }
                if let Some(v) = vars.iter().chain(category.as_ref()).find(|v| v.is_string()) {
                    return Err(Error(format!(
                        "CORRESPONDENCE TABLE=ALL needs numeric variables, but {} is a string variable",
                        v.name()
                    )));
                }
                if data.weight_variable().is_some() {
                    warnings.push(
                        "CORRESPONDENCE: WEIGHT does not apply to TABLE=ALL; the cells are the counts"
                            .to_string(),
                    );
                }
                let counts: Vec<Vec<f64>> = (0..*r)
                    .map(|case| {
                        vars.iter()
                            .map(|v| match v.values() {
                                Values::Numeric(values) => values
                                    .get(case)
                                    .filter(|x| !v.is_user_missing(*x))
                                    .unwrap_or(0.0)
                                    .max(0.0),
                                Values::String(_) => 0.0,
                            })
                            .collect()
                    })
                    .collect();
                let row_labels: Vec<String> = (0..*r)
                    .map(|case| {
                        let value = category.as_ref().and_then(|v| match v.values() {
                            Values::Numeric(values) => values.get(case),
                            Values::String(_) => None,
                        });
                        match (&category, value) {
                            (Some(v), Some(x)) => shown.number(v, x),
                            _ => (case + 1).to_string(),
                        }
                    })
                    .collect();
                let rows = Side {
                    title: "Row".into(),
                    name: "ROW".into(),
                    labels: row_labels,
                };
                let columns = Side {
                    title: "Column".into(),
                    name: "COLUMN".into(),
                    labels: vars.iter().map(|v| shown.variable(v)).collect(),
                };
                let positions = |k: usize| (1..=k).map(|x| x as f64).collect::<Vec<f64>>();
                (counts, positions(*r), positions(*c), rows, columns)
            }
        };
        if counts.is_empty() || counts[0].is_empty() {
            return Err(Error(
                "CORRESPONDENCE: the table has no rows or no columns".into(),
            ));
        }
        let what = match command.table {
            CorrespondenceTable::Variables { .. } => "a category value in the TABLE range",
            CorrespondenceTable::All { .. } => "a position in the table",
        };
        let find = |values: &[f64], side: &Side, x: f64| -> Result<usize, Error> {
            values.iter().position(|&v| v == x).ok_or_else(|| {
                Error(format!(
                    "CORRESPONDENCE: {x} for {} is not {what}",
                    side.title
                ))
            })
        };
        let supplementary =
            |list: &[f64], values: &[f64], side: &Side| -> Result<Vec<bool>, Error> {
                let mut out = vec![false; values.len()];
                for &x in list {
                    out[find(values, side, x)?] = true;
                }
                Ok(out)
            };
        let row_supplementary = supplementary(&command.supplementary_rows, &row_values, &rows)?;
        let column_supplementary =
            supplementary(&command.supplementary_columns, &column_values, &columns)?;
        let groups = |list: &[Vec<f64>],
                      values: &[f64],
                      side: &Side,
                      supplementary: &[bool]|
         -> Result<Vec<Vec<usize>>, Error> {
            let mut seen = vec![false; values.len()];
            let mut out = Vec::new();
            for group in list {
                let mut members = Vec::new();
                for &x in group {
                    let i = find(values, side, x)?;
                    if supplementary[i] {
                        return Err(Error(format!(
                            "CORRESPONDENCE: {x} of {} is supplementary and cannot be in an EQUAL group",
                            side.title
                        )));
                    }
                    if seen[i] {
                        return Err(Error(format!(
                            "CORRESPONDENCE: {x} of {} is in more than one EQUAL group or twice in one",
                            side.title
                        )));
                    }
                    seen[i] = true;
                    members.push(i);
                }
                out.push(members);
            }
            Ok(out)
        };
        let row_groups = groups(&command.equal_rows, &row_values, &rows, &row_supplementary)?;
        let column_groups = groups(
            &command.equal_columns,
            &column_values,
            &columns,
            &column_supplementary,
        )?;
        let (alpha, beta) = match command.normalization {
            CorrespondenceNormalization::Principal => (1.0, 1.0),
            CorrespondenceNormalization::Q(q) => ((1.0 + q) / 2.0, (1.0 - q) / 2.0),
        };
        let chisq = command.measure == CorrespondenceMeasure::Chisq;
        let setup = Setup {
            counts: counts.clone(),
            row_supplementary: row_supplementary.clone(),
            column_supplementary: column_supplementary.clone(),
            row_groups,
            column_groups,
            standardize: if chisq {
                Standardize::Rcmean
            } else {
                command.standardize
            },
            dimension: command.dimension,
            alpha,
            beta,
            scores: command.print.rconf
                || command.print.cconf
                || command.outfile_variance.is_some(),
        };
        let s = solve(&setup).map_err(|e| Error(format!("CORRESPONDENCE: {e}")))?;
        warnings.extend(s.warnings.iter().cloned());
        let p = s.p;
        let print = command.print;
        let norm = normalization_text(command.normalization);
        let mut tables = Vec::new();

        // the input table and the profiles over it; margins over the active rows and columns
        let in_analysis = |st: &Status| !matches!(st, Status::Supplementary | Status::Empty);
        let in_rows: Vec<bool> = s.rows.status.iter().map(in_analysis).collect();
        let in_columns: Vec<bool> = s.columns.status.iter().map(in_analysis).collect();
        let row_margin: Vec<f64> = counts
            .iter()
            .map(|row| {
                row.iter()
                    .zip(&in_columns)
                    .filter(|x| *x.1)
                    .map(|x| x.0)
                    .sum()
            })
            .collect();
        let column_margin: Vec<f64> = (0..columns.labels.len())
            .map(|j| {
                counts
                    .iter()
                    .zip(&in_rows)
                    .filter(|x| *x.1)
                    .map(|x| x.0[j])
                    .sum()
            })
            .collect();
        let grand: f64 = row_margin
            .iter()
            .zip(&in_rows)
            .filter(|x| *x.1)
            .map(|x| x.0)
            .sum();
        let ratio = |a: f64, b: f64| {
            if b > 0.0 {
                Cell::Number(a / b)
            } else {
                Cell::Empty
            }
        };
        let body = |cell: &dyn Fn(usize, usize) -> Cell, last: &dyn Fn(usize) -> Cell| {
            (0..rows.labels.len())
                .map(|i| {
                    let mut row = vec![text(rows.labels[i].clone())];
                    row.extend((0..columns.labels.len()).map(|j| cell(i, j)));
                    row.push(last(i));
                    row
                })
                .collect::<Vec<Vec<Cell>>>()
        };
        if print.table {
            let mut cells = body(&|i, j| Cell::Number(counts[i][j]), &|i| {
                Cell::Number(row_margin[i])
            });
            let mut last = vec![text("Active Margin")];
            last.extend(column_margin.iter().map(|&x| Cell::Number(x)));
            last.push(Cell::Number(grand));
            cells.push(last);
            tables.push(correspondence_table(
                "Correspondence Table",
                &rows,
                &columns,
                cells,
                "Active Margin",
            ));
        }
        if print.rprofiles {
            let mut cells = body(&|i, j| ratio(counts[i][j], row_margin[i]), &|i| {
                ratio(row_margin[i], row_margin[i])
            });
            let mut last = vec![text("Mass")];
            last.extend(column_margin.iter().map(|&x| ratio(x, grand)));
            last.push(Cell::Empty);
            cells.push(last);
            tables.push(correspondence_table(
                "Row Profiles",
                &rows,
                &columns,
                cells,
                "Active Margin",
            ));
        }
        if print.cprofiles {
            let mut cells = body(&|i, j| ratio(counts[i][j], column_margin[j]), &|i| {
                ratio(row_margin[i], grand)
            });
            let mut last = vec![text("Active Margin")];
            last.extend(column_margin.iter().map(|&x| ratio(x, x)));
            last.push(Cell::Empty);
            cells.push(last);
            tables.push(correspondence_table(
                "Column Profiles",
                &rows,
                &columns,
                cells,
                "Mass",
            ));
        }

        // Summary
        let kept = s.singular.len();
        let mut header = vec![
            (vec!["", ""], "Dimension".to_string()),
            (vec!["", ""], "Singular Value".into()),
            (vec!["", ""], "Inertia".into()),
            (vec!["", ""], "Chi Square".into()),
            (vec!["", ""], "Sig.".into()),
            (vec!["Proportion of Inertia", ""], "Accounted for".into()),
            (vec!["Proportion of Inertia", ""], "Cumulative".into()),
            (
                vec!["Confidence Singular Value", ""],
                "Standard Deviation".into(),
            ),
        ];
        header.extend((2..=p).map(|t| {
            (
                vec!["Confidence Singular Value", "Correlation"],
                t.to_string(),
            )
        }));
        let mut summary_rows = Vec::new();
        let mut cumulative = 0.0;
        for (t, &l) in s.singular.iter().enumerate() {
            let share = l * l / s.total;
            cumulative += share;
            let mut row = vec![
                text((t + 1).to_string()),
                Cell::Number(l),
                Cell::Number(l * l),
                Cell::Empty,
                Cell::Empty,
                Cell::Number(share),
                Cell::Number(cumulative),
                nan_cell(s.singular_cov[t][t].sqrt()),
            ];
            let c = &s.singular_cov;
            row.extend((1..p).map(|u| {
                if t < p && u > t {
                    nan_cell(c[t][u] / (c[t][t] * c[u][u]).sqrt())
                } else {
                    Cell::Empty
                }
            }));
            summary_rows.push(row);
        }
        let df = ((s.k.0 - 1) * (s.k.1 - 1)) as f64;
        let chi = s.n * s.total;
        let mut total = vec![
            text("Total"),
            Cell::Empty,
            Cell::Number(s.total),
            if chisq {
                Cell::Number(chi)
            } else {
                Cell::Empty
            },
            if chisq {
                Cell::Number(chi_square_p(chi, df))
            } else {
                Cell::Empty
            },
            Cell::Number(1.0),
            Cell::Number(1.0),
        ];
        total.extend((0..p).map(|_| Cell::Empty));
        summary_rows.push(total);
        let mut summary = grouped("Summary", header, summary_rows);
        let plural = if kept == 1 { "" } else { "s" };
        summary.caption = Some(if chisq {
            format!("{df} degrees of freedom; {kept} dimension{plural} in the table.")
        } else {
            format!("{kept} dimension{plural} in the table.")
        });
        tables.push(summary);

        // overview of the points
        let list = |side: &Side, status: &[Status], pick: &dyn Fn(Status) -> bool| -> Vec<String> {
            side.labels
                .iter()
                .zip(status)
                .filter(|(_, s)| pick(**s))
                .map(|(l, _)| l.clone())
                .collect()
        };
        let caption = |side: &Side, points: &Points| -> String {
            let mut out = vec![norm.clone()];
            let supp = list(side, &points.status, &|s| s == Status::Supplementary);
            if !supp.is_empty() {
                out.push(format!("Supplementary: {}.", supp.join(", ")));
            }
            let merged: Vec<String> = side
                .labels
                .iter()
                .zip(&points.status)
                .filter_map(|(l, s)| match s {
                    Status::Merged(first) => Some(format!("{l} as {}", side.labels[*first])),
                    _ => None,
                })
                .collect();
            if !merged.is_empty() {
                out.push(format!("Equal scores: {}.", merged.join(", ")));
            }
            let empty = list(side, &points.status, &|s| s == Status::Empty);
            if !empty.is_empty() {
                out.push(format!("No counts: {}.", empty.join(", ")));
            }
            out.join(" ")
        };
        if print.rpoints {
            tables.push(overview(
                "Overview Row Points",
                &rows,
                &s.rows,
                p,
                caption(&rows, &s.rows),
            ));
        }
        if print.cpoints {
            tables.push(overview(
                "Overview Column Points",
                &columns,
                &s.columns,
                p,
                caption(&columns, &s.columns),
            ));
        }
        if print.rconf {
            tables.push(confidence("Confidence Row Points", &rows, &s.rows, p));
        }
        if print.cconf {
            tables.push(confidence(
                "Confidence Column Points",
                &columns,
                &s.columns,
                p,
            ));
        }
        if let Some(n) = print.permutation {
            for t in 0..n.min(p) {
                let order = |points: &Points| -> Vec<usize> {
                    let mut idx: Vec<usize> = (0..points.score.len()).collect();
                    let key = |i: &usize| points.score[*i][t];
                    idx.sort_by(|a, b| match (key(a).is_nan(), key(b).is_nan()) {
                        (false, false) => key(a).total_cmp(&key(b)),
                        (x, y) => x.cmp(&y),
                    });
                    idx
                };
                let (ro, co) = (order(&s.rows), order(&s.columns));
                let permuted_columns = Side {
                    title: columns.title.clone(),
                    name: columns.name.clone(),
                    labels: co.iter().map(|&j| columns.labels[j].clone()).collect(),
                };
                let mut cells: Vec<Vec<Cell>> = ro
                    .iter()
                    .map(|&i| {
                        let mut row = vec![text(rows.labels[i].clone())];
                        row.extend(co.iter().map(|&j| Cell::Number(counts[i][j])));
                        row.push(Cell::Number(row_margin[i]));
                        row
                    })
                    .collect();
                let mut last = vec![text("Active Margin")];
                last.extend(co.iter().map(|&j| Cell::Number(column_margin[j])));
                last.push(Cell::Number(grand));
                cells.push(last);
                let mut table = correspondence_table(
                    "Permuted Correspondence Table",
                    &rows,
                    &permuted_columns,
                    cells,
                    "Active Margin",
                );
                table.caption = Some(format!(
                    "Rows and columns in the order of their scores in dimension {}.",
                    t + 1
                ));
                tables.push(table);
            }
        }

        let charts = plots(command, &s, &rows, &columns, &norm, &mut warnings);

        if command.outfile_score.is_some() || command.outfile_variance.is_some() {
            let string = |values: &[String]| -> (usize, Vec<String>) {
                let width = values
                    .iter()
                    .map(String::len)
                    .max()
                    .unwrap_or(1)
                    .clamp(1, 32767);
                (width, values.to_vec())
            };
            let put = |out: &mut Dataset, name: &str, values: Vec<String>| -> Result<(), Error> {
                let (width, values) = string(&values);
                out.set_string_values(name, width, values)?;
                Ok(())
            };
            if let Some(file) = &command.outfile_score {
                let all: Vec<(&str, &Side, &Points, usize)> = (0..rows.labels.len())
                    .map(|i| ("ROW", &rows, &s.rows, i))
                    .chain((0..columns.labels.len()).map(|j| ("COLUMN", &columns, &s.columns, j)))
                    .collect();
                let mut out = Dataset::new(all.len());
                put(
                    &mut out,
                    "ROWTYPE_",
                    all.iter().map(|x| x.0.to_string()).collect(),
                )?;
                put(
                    &mut out,
                    "LEVEL_",
                    all.iter().map(|x| x.1.labels[x.3].clone()).collect(),
                )?;
                put(
                    &mut out,
                    "VARNAME_",
                    all.iter().map(|x| x.1.name.clone()).collect(),
                )?;
                for t in 0..p {
                    let values = all
                        .iter()
                        .map(|x| Some(x.2.score[x.3][t]).filter(|v| v.is_finite()))
                        .collect();
                    out.set_numeric_values(&format!("DIM{}", t + 1), values)?;
                }
                self.write_data_file(file, out)?;
            }
            if let Some(file) = &command.outfile_variance {
                // (variable, level, dimension, covariances with every dimension)
                let mut lines: Vec<(String, String, usize, Vec<f64>)> = Vec::new();
                for t in 0..p {
                    lines.push((
                        "SINGULAR".into(),
                        String::new(),
                        t,
                        s.singular_cov[t][..p].to_vec(),
                    ));
                }
                for (side, points) in [(&rows, &s.rows), (&columns, &s.columns)] {
                    for (i, label) in side.labels.iter().enumerate() {
                        // the file leaves out the rows and columns of EQUAL groups
                        let grouped = points.status.iter().any(|s| *s == Status::Merged(i));
                        if points.status[i] == Status::Active && !grouped {
                            for t in 0..p {
                                lines.push((
                                    side.name.clone(),
                                    label.clone(),
                                    t,
                                    points.cov[i][t].clone(),
                                ));
                            }
                        }
                    }
                }
                let mut out = Dataset::new(lines.len());
                put(&mut out, "ROWTYPE_", vec!["COV".to_string(); lines.len()])?;
                put(
                    &mut out,
                    "VARNAME_",
                    lines.iter().map(|l| l.0.clone()).collect(),
                )?;
                put(
                    &mut out,
                    "LEVEL_",
                    lines.iter().map(|l| l.1.clone()).collect(),
                )?;
                put(
                    &mut out,
                    "DIMNMBR_",
                    lines.iter().map(|l| (l.2 + 1).to_string()).collect(),
                )?;
                for t in 0..p {
                    let values = lines
                        .iter()
                        .map(|l| Some(l.3[t]).filter(|v| v.is_finite()))
                        .collect();
                    out.set_numeric_values(&format!("DIM{}", t + 1), values)?;
                }
                self.write_data_file(file, out)?;
            }
        }
        self.charts.extend(charts);
        for warning in warnings {
            self.warn(warning);
        }
        Ok(tables)
    }
}

/// The scatter plots of the points and the transformation plots.
fn plots(
    command: &Correspondence,
    s: &Solution,
    rows: &Side,
    columns: &Side,
    norm: &str,
    warnings: &mut Vec<String>,
) -> Vec<crate::Chart> {
    let plot = command.plot;
    let p = s.p;
    let mut charts = Vec::new();
    let (first, last) = match plot.ndim {
        None => (1, p),
        Some((a, b)) => (a, b.unwrap_or(p).min(p)),
    };
    let any = [
        plot.biplot,
        plot.rpoints,
        plot.cpoints,
        plot.trrows,
        plot.trcolumns,
    ]
    .iter()
    .any(Option::is_some);
    if first > p {
        if any && plot.given {
            warnings.push(format!(
            "CORRESPONDENCE: NDIM starts at dimension {first}, but the solution has {p}; no plots"
        ));
        }
        return charts;
    }
    let axis = |t: usize| {
        let mut axis = Axis::titled(format!("Dimension {t}"));
        axis.format = "F8.3".into();
        axis
    };
    let group =
        |side: &Side, points: &Points, length: Option<usize>, a: usize, b: usize| PointGroup {
            name: side.title.clone(),
            points: side
                .labels
                .iter()
                .enumerate()
                .filter(|(i, _)| points.score[*i][a - 1].is_finite())
                .map(|(i, label)| Point {
                    x: points.score[i][a - 1],
                    y: points.score[i][b - 1],
                    z: None,
                    label: short(label, length),
                })
                .collect(),
        };
    let pairs: Vec<(usize, usize)> = match plot.ndim {
        None => (1..=p)
            .flat_map(|a| (a + 1..=p).map(move |b| (a, b)))
            .collect(),
        Some(_) => (first + 1..=last).map(|b| (first, b)).collect(),
    };
    let mut biplot = plot.biplot;
    if biplot.is_some() && command.normalization == CorrespondenceNormalization::Principal {
        // the default biplot is dropped quietly; one asked for gets a warning
        if plot.given {
            warnings.push(
            "CORRESPONDENCE: no biplot with NORMALIZATION=PRINCIPAL; plot RPOINTS and CPOINTS instead"
                .into(),
            );
        }
        biplot = None;
    }
    let scatter: [(PlotLabels, &str, bool, bool); 3] = [
        (biplot, "Row and Column Points", true, true),
        (plot.rpoints, "Row Points", true, false),
        (plot.cpoints, "Column Points", false, true),
    ];
    if scatter.iter().any(|x| x.0.is_some()) && pairs.is_empty() && plot.given {
        warnings.push(match p {
            1 => "CORRESPONDENCE: one dimension only, so there is no scatter of points".into(),
            _ => format!(
                "CORRESPONDENCE: NDIM leaves no pair of the {p} dimensions to plot against each other"
            ),
        });
    }
    for (labels, title, with_rows, with_columns) in scatter {
        let Some(length) = labels else { continue };
        for &(a, b) in &pairs {
            let mut groups = Vec::new();
            if with_rows {
                groups.push(group(rows, &s.rows, length, a, b));
            }
            if with_columns {
                groups.push(group(columns, &s.columns, length, a, b));
            }
            let mut chart = crate::Chart::plot(
                crate::chart::ChartKind::Scatter,
                title.into(),
                axis(a),
                axis(b),
                Plot::Points {
                    groups,
                    lines: Vec::new(),
                    z_axis: None,
                },
            );
            chart.footnote = Some(norm.to_string());
            charts.push(chart);
        }
    }
    for (labels, side, points) in [
        (plot.trrows, rows, &s.rows),
        (plot.trcolumns, columns, &s.columns),
    ] {
        let Some(length) = labels else { continue };
        let shown: Vec<usize> = (0..side.labels.len())
            .filter(|&i| points.score[i].first().is_some_and(|x| x.is_finite()))
            .collect();
        for t in first..=last {
            let categories = shown
                .iter()
                .map(|&i| Category {
                    label: short(&side.labels[i], length).unwrap_or_default(),
                    value: Cell::Number((i + 1) as f64),
                    missing: false,
                    bin: None,
                })
                .collect();
            let series = vec![Series {
                name: format!("Dimension {t}"),
                statistic: Statistic::Value,
                variable: None,
                value: Cell::Empty,
                values: shown.iter().map(|&i| points.score[i][t - 1]).collect(),
            }];
            let mut chart = crate::Chart::lines(
                format!("{}, Dimension {t}", side.title),
                Axis::titled(side.title.clone()),
                axis(t),
                categories,
                series,
            );
            chart.footnote = Some(norm.to_string());
            charts.push(chart);
        }
    }
    charts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(counts: Vec<Vec<f64>>) -> Setup {
        let (t1, t2) = (counts.len(), counts[0].len());
        Setup {
            counts,
            row_supplementary: vec![false; t1],
            column_supplementary: vec![false; t2],
            row_groups: Vec::new(),
            column_groups: Vec::new(),
            standardize: Standardize::Rcmean,
            dimension: Some(2),
            alpha: 0.5,
            beta: 0.5,
            scores: true,
        }
    }

    /// The analytic derivatives against central differences of the whole solution: the
    /// singular values and the standard scores of rows and columns in two dimensions, for the
    /// chi-square measure and a Euclidean standardization.
    #[test]
    fn delta_method_derivatives_match_differences() {
        let counts = vec![
            vec![12.0, 7.0, 3.0, 9.0, 5.0],
            vec![4.0, 15.0, 8.0, 2.0, 6.0],
            vec![6.0, 5.0, 14.0, 10.0, 3.0],
            vec![9.0, 3.0, 4.0, 13.0, 11.0],
        ];
        let n: f64 = counts.iter().flatten().sum();
        let pi: Vec<f64> = counts.iter().flatten().map(|x| x / n).collect();
        let var = |g: &[f64]| {
            let m: f64 = pi.iter().zip(g).map(|(p, x)| p * x).sum();
            pi.iter().zip(g).map(|(p, x)| p * x * x).sum::<f64>() / n - m * m / n
        };
        for standardize in [Standardize::Rcmean, Standardize::Csum, Standardize::Rmean] {
            let make = |c: Vec<Vec<f64>>| {
                let mut s = setup(c);
                s.standardize = standardize;
                solve(&s).unwrap()
            };
            let s = make(counts.clone());
            // (rows?, point, dimension); the standard score is the score over sqrt(lambda)
            let picks = [(true, 0, 0), (true, 2, 1), (false, 1, 0), (false, 4, 1)];
            let standard = |sol: &Solution, (rows, i, t): (bool, usize, usize)| {
                let points = if rows { &sol.rows } else { &sol.columns };
                points.score[i][t] / sol.singular[t].sqrt()
            };
            let mut grads = vec![Vec::new(); picks.len() + 2];
            let h = 1e-6;
            for a in 0..4 {
                for b in 0..5 {
                    let shifted = |d: f64| {
                        let mut c = counts.clone();
                        c[a][b] += d * n;
                        make(c)
                    };
                    let (up, down) = (shifted(h), shifted(-h));
                    for (t, g) in grads.iter_mut().enumerate().take(2) {
                        g.push((up.singular[t] - down.singular[t]) / (2.0 * h));
                    }
                    for (g, &pick) in grads[2..].iter_mut().zip(&picks) {
                        g.push((standard(&up, pick) - standard(&down, pick)) / (2.0 * h));
                    }
                }
            }
            for (t, g) in grads.iter().enumerate().take(2) {
                let sd = s.singular_cov[t][t].sqrt();
                assert!((sd - var(g).sqrt()).abs() < 1e-6, "{standardize:?} {t}");
            }
            for (g, &(rows, i, t)) in grads[2..].iter().zip(&picks) {
                let points = if rows { &s.rows } else { &s.columns };
                let sd = points.cov[i][t][t].sqrt() / s.singular[t].sqrt();
                let expected = var(g).sqrt();
                assert!(
                    (sd - expected).abs() < 1e-5 * expected.max(1.0),
                    "{standardize:?} {rows} {i} {t}: {sd} {expected}"
                );
            }
        }
    }

    #[test]
    fn score_deviations_have_a_size_limit() {
        assert!(score_work_fits(20, 30, 19, 2));
        assert!(score_work_fits(200, 200, 199, 2));
        assert!(!score_work_fits(800, 800, 799, 2));
        assert!(!score_work_fits(2, 100_000, 1, 1));
    }

    /// Dimension 1 stands apart from a tied pair: it keeps its deviations, which match central
    /// differences, while the pair gets none.
    #[test]
    fn a_dimension_next_to_a_tie_keeps_its_deviations() {
        let counts = vec![
            vec![8.0, 2.0, 3.0, 2.0],
            vec![2.0, 8.0, 2.0, 3.0],
            vec![3.0, 2.0, 8.0, 2.0],
            vec![2.0, 3.0, 2.0, 8.0],
        ];
        let mut base = setup(counts.clone());
        base.dimension = Some(3);
        let s = solve(&base).unwrap();
        assert!(s.warnings.iter().any(|w| w.contains("equal")));
        assert!(s.singular_cov[1][1].is_nan() && s.singular_cov[2][2].is_nan());
        assert!(s.rows.cov[0][1][1].is_nan());
        let n: f64 = counts.iter().flatten().sum();
        let pi: Vec<f64> = counts.iter().flatten().map(|x| x / n).collect();
        let (mut gl, mut gx) = (Vec::new(), Vec::new());
        let h = 1e-6;
        for a in 0..4 {
            for b in 0..4 {
                let shifted = |d: f64| {
                    let mut c = counts.clone();
                    c[a][b] += d * n;
                    let mut t = setup(c);
                    t.dimension = Some(3);
                    solve(&t).unwrap()
                };
                let (up, down) = (shifted(h), shifted(-h));
                gl.push((up.singular[0] - down.singular[0]) / (2.0 * h));
                // every row scores equally far out in this dimension, so the sign rule may flip
                // under a small change; the absolute score has the same derivative up to sign
                let x = |sol: &Solution| (sol.rows.score[1][0] / sol.singular[0].sqrt()).abs();
                gx.push((x(&up) - x(&down)) / (2.0 * h));
            }
        }
        let sd = |g: &[f64]| {
            let m: f64 = pi.iter().zip(g).map(|(p, x)| p * x).sum();
            (pi.iter().zip(g).map(|(p, x)| p * x * x).sum::<f64>() / n - m * m / n).sqrt()
        };
        assert!((s.singular_cov[0][0].sqrt() - sd(&gl)).abs() < 1e-6);
        let analytic = s.rows.cov[1][0][0].sqrt() / s.singular[0].sqrt();
        assert!((analytic - sd(&gx)).abs() < 1e-5, "{analytic} {}", sd(&gx));
    }
}
