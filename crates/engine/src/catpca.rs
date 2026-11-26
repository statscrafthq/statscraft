//! CATPCA: principal components of variables quantified by optimal scaling, with its tables,
//! charts and saved variables. The data handling it shares with MULTIPLE CORRESPONDENCE lives
//! in `optimal_scaling`.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - Each variable gets a quantification: a value per category. A multiple nominal variable
//!   gets one per dimension, the centroids of its objects. Every other level gets a single
//!   value per category times a loading per dimension, and restricts that single value:
//!   nominal leaves it free, ordinal keeps the order of the codes (weighted monotone
//!   regression by pooling adjacent violators), numeric keeps equal steps (weighted linear
//!   regression on the codes), and the spline levels fit a piecewise polynomial of the codes
//!   (I-splines of the given degree with interior knots, by weighted least squares; spline
//!   ordinal keeps the coefficients nonnegative so the result rises).
//! - The loss is the weighted distance of the objects from their quantified categories,
//!   summed over variables with their weights. Objects are centered and orthonormal with
//!   the case weights times the summed weights of the variables each object has; a passive
//!   missing value leaves that variable out for that object.
//! - Alternating least squares: from the objects, each variable's centroids, the free single
//!   value along its loading, the restriction, a sum of squares of n (the weighted cases), the
//!   loading as the cross product with the centroids; then new objects from the quantified
//!   categories, centered and made orthonormal. A first phase treats every level but multiple
//!   nominal as numeric; its result is iteration 0. The fit is the variance accounted for:
//!   the summed squared loadings of the single variables plus, for multiple nominal ones, the
//!   mean over dimensions of the weighted variance of their centroids. Iterations stop when
//!   the fit gains less than CRITITER or after MAXITER.
//! - An extra category made by ACTIVE EXTRACAT takes part in the numeric first phase but is
//!   left out of the restriction afterwards.
//! - After the fit the dimensions turn to principal axes, and a dimension is reflected when
//!   its negative loadings have the larger mean square.
//! - The eigenvalue of a dimension is its variance accounted for; Cronbach's alpha is
//!   m(e - 1) / ((m - 1)e) with m the summed variable weights.
//! - NORMALIZATION q: the objects get the power (1 + q)/2 of the root of the eigenvalue, the
//!   loadings and multiple nominal centroids the power (1 - q)/2 less one; INDEPENDENT gives
//!   the objects the root and leaves the variables.
//! - Supplementary variables are fitted to the final objects; supplementary objects get scores
//!   from their categories through the last update.

use crate::chart::{Axis, Category, ChartKind, Series, Statistic};
use crate::factor::{oblimin, orthogonal, promax, sorted_rows};
use crate::matrix::{Matrix, inverse, multiply, transpose};
use crate::mds::{monotone, nnls, spline_knots, spline_rows};
use crate::npar_tests::text;
use crate::optimal_scaling::{
    AVar, GroupList, Prepared, centroids, coded_index, correlation_table, correlations, cut,
    eigen_sorted, free_set, num, object_groups, orthonormal_transform, orthonormalize, point_chart,
    resolve, scatter, transform_rows, transformation_chart,
};
use crate::plot::Point;
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_data::{Dataset, Measure};
use statscraft_syntax::{
    Catpca, CatpcaPlot, FactorCriteria, Impute, McaMissing, McaNormalization, Rotation,
    ScalingLevel, VarItem,
};
use std::collections::BTreeMap;

const WHO: &str = "CATPCA";

/// How the fit quantifies a variable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    Multiple,
    Nominal,
    Ordinal,
    Numeric,
    Spline { ordinal: bool },
}

/// A variable's quantification.
pub(crate) struct Quant {
    pub(crate) kind: Kind,
    /// The code of each category, for the linear, monotone and spline restrictions.
    pub(crate) values: Vec<f64>,
    /// The extra category of ACTIVE EXTRACAT, outside the restriction in the final phase.
    pub(crate) extra: Option<usize>,
    /// The spline basis of each category (intercept first), for the spline levels.
    pub(crate) basis: Option<Matrix>,
    /// Single levels: the value of each category, weighted sum of squares n.
    pub(crate) y: Vec<f64>,
    /// Single levels: the loading in each dimension.
    a: Vec<f64>,
    /// The weighted mean object of each category.
    centroids: Matrix,
}

impl Quant {
    /// The quantification of `v` at the numeric start: the standardized codes, with the
    /// spline basis for the spline levels.
    pub(crate) fn new(v: &AVar, level: ScalingLevel, n_w: f64) -> Quant {
        let kind = match level {
            ScalingLevel::MultipleNominal => Kind::Multiple,
            ScalingLevel::Nominal => Kind::Nominal,
            ScalingLevel::Ordinal => Kind::Ordinal,
            ScalingLevel::Numeric => Kind::Numeric,
            ScalingLevel::SplineOrdinal { .. } => Kind::Spline { ordinal: true },
            ScalingLevel::SplineNominal { .. } => Kind::Spline { ordinal: false },
        };
        let values: Vec<f64> = v.cats.iter().map(|c| *c as f64).collect();
        let extra = v.extra.and_then(|e| v.cats.binary_search(&e).ok());
        let basis = match level {
            ScalingLevel::SplineOrdinal { degree, inknot }
            | ScalingLevel::SplineNominal { degree, inknot } => {
                // knots at quantiles of the objects' codes, the extra category left out
                let sample: Vec<f64> =
                    v.h.iter()
                        .flatten()
                        .filter(|&&r| Some(r) != extra)
                        .map(|&r| values[r])
                        .collect();
                let inside = values.len() - usize::from(extra.is_some());
                (inside >= 2).then(|| {
                    let inknot = inknot.min(inside - 2);
                    spline_rows(&values, &spline_knots(&sample, degree, inknot), degree)
                })
            }
            _ => None,
        };
        let kind = match (kind, &basis) {
            (Kind::Spline { .. }, None) => Kind::Nominal,
            _ => kind,
        };
        let y = standardized(&values, &v.d, n_w);
        Quant {
            kind,
            values,
            extra,
            basis,
            y,
            a: Vec::new(),
            centroids: Vec::new(),
        }
    }

    /// The point of category r: its centroid, or its value times the loadings.
    fn point(&self, r: usize, s: usize) -> f64 {
        match self.kind {
            Kind::Multiple => self.centroids[r][s],
            _ => self.y[r] * self.a[s],
        }
    }

    fn single(&self) -> bool {
        self.kind != Kind::Multiple
    }
}

/// One step of the history: the fit, the loss of the centroids and the further loss of
/// confining single variables to a vector.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Step {
    fit: f64,
    multiple_loss: f64,
    single_loss: f64,
}

/// The codes standardized with the category weights: mean 0, weighted sum of squares n.
pub(crate) fn standardized(values: &[f64], d: &[f64], n_w: f64) -> Vec<f64> {
    let total: f64 = d.iter().sum();
    if total <= 0.0 {
        return vec![0.0; values.len()];
    }
    let mean = values.iter().zip(d).map(|(x, w)| x * w).sum::<f64>() / total;
    let ss: f64 = values
        .iter()
        .zip(d)
        .map(|(x, w)| w * (x - mean).powi(2))
        .sum();
    match ss > 0.0 {
        true => values
            .iter()
            .map(|x| (x - mean) * (n_w / ss).sqrt())
            .collect(),
        false => vec![0.0; values.len()],
    }
}

/// Weighted least squares of `y` on an intercept and the columns of `rows`; with `nonnegative`
/// the column coefficients stay at or above 0. Returns the fitted values.
fn regression(rows: &Matrix, y: &[f64], w: &[f64], nonnegative: bool) -> Vec<f64> {
    let total: f64 = w.iter().sum();
    let q = rows.first().map_or(0, Vec::len);
    let ybar = y.iter().zip(w).map(|(a, b)| a * b).sum::<f64>() / total;
    let means: Vec<f64> = (0..q)
        .map(|c| rows.iter().zip(w).map(|(r, b)| r[c] * b).sum::<f64>() / total)
        .collect();
    // centered and scaled by the root of the weights: the intercept is then free
    let s: Matrix = rows
        .iter()
        .zip(w)
        .map(|(r, b)| (0..q).map(|c| (r[c] - means[c]) * b.sqrt()).collect())
        .collect();
    let t: Vec<f64> = y
        .iter()
        .zip(w)
        .map(|(v, b)| (v - ybar) * b.sqrt())
        .collect();
    let coef = match nonnegative {
        true => nnls(&s, &t),
        false => least_squares(&s, &t),
    };
    rows.iter()
        .map(|r| ybar + (0..q).map(|c| (r[c] - means[c]) * coef[c]).sum::<f64>())
        .collect()
}

/// Least squares coefficients by the pseudo-inverse, so that collinear columns do no harm.
fn least_squares(s: &Matrix, t: &[f64]) -> Vec<f64> {
    let q = s.first().map_or(0, Vec::len);
    let sts: Matrix = (0..q)
        .map(|a| {
            (0..q)
                .map(|c| s.iter().map(|r| r[a] * r[c]).sum())
                .collect()
        })
        .collect();
    let stt: Vec<f64> = (0..q)
        .map(|a| s.iter().zip(t).map(|(r, v)| r[a] * v).sum())
        .collect();
    let (values, vectors) = eigen_sorted(&sts);
    let top = values.first().copied().unwrap_or(0.0).max(0.0);
    let mut coef = vec![0.0; q];
    for (k, &e) in values.iter().enumerate() {
        if e <= 1e-10 * top || e <= 0.0 {
            continue;
        }
        let proj: f64 = (0..q).map(|a| vectors[a][k] * stt[a]).sum::<f64>() / e;
        for a in 0..q {
            coef[a] += vectors[a][k] * proj;
        }
    }
    coef
}

/// The restricted single quantification of the free one `yt`.
pub(crate) fn restrict(q: &Quant, initial: bool, yt: &[f64], d: &[f64]) -> Vec<f64> {
    let kind = if initial { Kind::Numeric } else { q.kind };
    if kind == Kind::Nominal {
        return yt.to_vec();
    }
    let keep: Vec<usize> = (0..yt.len())
        .filter(|&r| initial || Some(r) != q.extra)
        .collect();
    if keep.len() < 2 {
        return yt.to_vec();
    }
    let sub = |v: &[f64]| -> Vec<f64> { keep.iter().map(|&r| v[r]).collect() };
    let (y, w) = (sub(yt), sub(d));
    let fitted = match kind {
        Kind::Ordinal => monotone(&y, &w),
        Kind::Spline { ordinal } => {
            let basis = q.basis.as_ref().expect("a spline basis");
            let rows: Matrix = keep.iter().map(|&r| basis[r][1..].to_vec()).collect();
            regression(&rows, &y, &w, ordinal)
        }
        _ => {
            let rows: Matrix = keep.iter().map(|&r| vec![q.values[r]]).collect();
            regression(&rows, &y, &w, false)
        }
    };
    let mut out = yt.to_vec();
    for (&r, f) in keep.iter().zip(fitted) {
        out[r] = f;
    }
    out
}

/// One quantification step for a variable from the objects `x`.
fn quantify(v: &AVar, q: &mut Quant, x: &Matrix, w: &[f64], n_w: f64, initial: bool) {
    let p = x[0].len();
    q.centroids = centroids(v, x, w, p);
    if !q.single() {
        return;
    }
    let yt: Vec<f64> = q
        .centroids
        .iter()
        .map(|row| row.iter().zip(&q.a).map(|(c, a)| c * a).sum())
        .collect();
    let ystar = restrict(q, initial, &yt, &v.d);
    let ss: f64 = ystar.iter().zip(&v.d).map(|(y, d)| d * y * y).sum();
    // a quantification without spread carries nothing: keep the last one
    if ss > 1e-20 * n_w {
        q.y = ystar.iter().map(|y| y * (n_w / ss).sqrt()).collect();
    }
    q.a = (0..p)
        .map(|s| {
            q.centroids
                .iter()
                .zip(&v.d)
                .zip(&q.y)
                .map(|((c, d), y)| d * c[s] * y)
                .sum::<f64>()
                / n_w
        })
        .collect();
}

/// The weighted sum of squares of the centroids over n, per dimension.
fn centroid_fit(v: &AVar, q: &Quant, n_w: f64, p: usize) -> Vec<f64> {
    (0..p)
        .map(|s| {
            q.centroids
                .iter()
                .zip(&v.d)
                .map(|(c, d)| d * c[s] * c[s])
                .sum::<f64>()
                / n_w
        })
        .collect()
}

/// The fit and the two losses of the active variables' current quantifications.
fn step(prep: &Prepared, quants: &[Quant], p: usize) -> Step {
    let (mut multiple, mut single, mut vector) = (0.0, 0.0, 0.0);
    let (mut m1, mut m2) = (0.0, 0.0);
    for &j in &prep.active_vars {
        let (v, q) = (&prep.avars[j], &quants[j]);
        let cf: f64 = centroid_fit(v, q, prep.n_w, p).iter().sum();
        match q.single() {
            true => {
                m2 += v.weight;
                single += v.weight * cf;
                vector += v.weight * q.a.iter().map(|a| a * a).sum::<f64>();
            }
            false => {
                m1 += v.weight;
                multiple += v.weight * cf / p as f64;
            }
        }
    }
    let total = m1 + p as f64 * m2;
    Step {
        fit: multiple + vector,
        multiple_loss: total - multiple - single,
        single_loss: single - vector,
    }
}

/// The objects as the weighted mean of their quantified categories (before centering), for
/// the active objects and, with `cases`, for every case that has an active category.
fn object_sums(prep: &Prepared, quants: &[Quant], p: usize, case: Option<usize>) -> Vec<Matrix> {
    let rows: Vec<usize> = match case {
        Some(c) => vec![c],
        None => prep.active.clone(),
    };
    let mut out = Vec::new();
    for c in rows {
        let (mut z, mut m) = (vec![0.0; p], 0.0);
        for &j in &prep.active_vars {
            let v = &prep.avars[j];
            let Some(r) = v.codes[c].and_then(|x| v.cats.binary_search(&(x as i64)).ok()) else {
                continue;
            };
            m += v.weight;
            for (s, zs) in z.iter_mut().enumerate() {
                *zs += v.weight * quants[j].point(r, s);
            }
        }
        out.push(match m > 0.0 {
            true => vec![z.iter().map(|x| x / m).collect()],
            false => Vec::new(),
        });
    }
    out
}

/// The next objects: the weighted mean of the quantified categories, centered and
/// orthonormal; None when a dimension collapses.
fn next_objects(prep: &Prepared, quants: &[Quant], p: usize) -> Option<Matrix> {
    let mut z: Matrix = object_sums(prep, quants, p, None)
        .into_iter()
        .map(|mut r| r.pop().expect("an active object has an active value"))
        .collect();
    orthonormalize(&mut z, &prep.metric(), prep.n_w * prep.m_w)?;
    Some(z)
}

fn quantify_active(prep: &Prepared, quants: &mut [Quant], x: &Matrix, initial: bool) {
    for &j in &prep.active_vars {
        quantify(
            &prep.avars[j],
            &mut quants[j],
            x,
            &prep.w,
            prep.n_w,
            initial,
        );
    }
}

/// Iterates from `x`, whose quantifications give the last step in `steps`, until the fit
/// gains less than `criterion` (true) or after `max` iterations.
fn iterate(
    prep: &Prepared,
    quants: &mut [Quant],
    x: &mut Matrix,
    initial: bool,
    max: usize,
    criterion: f64,
    steps: &mut Vec<Step>,
) -> bool {
    let p = x[0].len();
    for _ in 0..max {
        let Some(z) = next_objects(prep, quants, p) else {
            return false;
        };
        *x = z;
        quantify_active(prep, quants, x, initial);
        let now = step(prep, quants, p);
        let gain = now.fit - steps.last().map_or(0.0, |s| s.fit);
        steps.push(now);
        if gain < criterion {
            return true;
        }
    }
    false
}

/// Fits a supplementary variable to the final objects: centroids once for multiple nominal,
/// else the quantification and loadings alternated from the numeric start until the squared
/// length of the loadings changes less than 0.00001, at most 100 times.
fn fit_supplementary(v: &AVar, q: &mut Quant, x: &Matrix, w: &[f64], n_w: f64) {
    let p = x[0].len();
    q.centroids = centroids(v, x, w, p);
    if !q.single() {
        return;
    }
    q.y = standardized(&q.values, &v.d, n_w);
    let a: Vec<f64> = (0..p)
        .map(|s| {
            q.centroids
                .iter()
                .zip(&v.d)
                .zip(&q.y)
                .map(|((c, d), y)| d * c[s] * y)
                .sum::<f64>()
        })
        .collect();
    let length = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    q.a = match length > 0.0 {
        true => a.iter().map(|x| x / length).collect(),
        false => a,
    };
    let mut last = f64::INFINITY;
    for _ in 0..100 {
        quantify(v, q, x, w, n_w, false);
        let now: f64 = q.a.iter().map(|x| x * x).sum();
        if (now - last).abs() < 0.00001 {
            break;
        }
        last = now;
    }
}

/// A table with a row header column and numeric cells.
/// Fits a bootstrap sample from the numeric start and summarizes it; None when it fails.
pub(crate) fn fit_quants_for_sample(
    engine: &Engine,
    command: &Catpca,
    prep: &Prepared,
    p: usize,
) -> Option<Solution> {
    let mut quants: Vec<Quant> = prep
        .avars
        .iter()
        .map(|v| Quant::new(v, command.analysis[v.list].1.level, prep.n_w))
        .collect();
    let (_, _, converged) = engine
        .fit_quants(command, prep, &mut quants, p, &None)
        .ok()?;
    Some(summarize(command, prep, &quants, p, converged))
}

/// What the bootstrap compares between solutions.
pub(crate) struct Solution {
    pub(crate) eigen: Vec<f64>,
    /// Per analysis variable the loadings as the Component Loadings table shows them (single
    /// levels), else empty.
    pub(crate) loadings: Vec<Vec<f64>>,
    /// Per analysis variable the value of each of its categories (single levels), else empty.
    pub(crate) values: Vec<Vec<f64>>,
    pub(crate) converged: bool,
}

/// The eigenvalue of every dimension: the variance the active variables account for in it.
pub(crate) fn eigen_values(prep: &Prepared, quants: &[Quant], p: usize) -> Vec<f64> {
    (0..p)
        .map(|s| {
            prep.active_vars
                .iter()
                .map(|&j| {
                    let (v, q) = (&prep.avars[j], &quants[j]);
                    match q.single() {
                        true => v.weight * q.a[s] * q.a[s],
                        false => v.weight * centroid_fit(v, q, prep.n_w, p)[s],
                    }
                })
                .sum()
        })
        .collect()
}

/// The eigenvalues, loadings and category values of a fitted solution.
pub(crate) fn summarize(
    command: &Catpca,
    prep: &Prepared,
    quants: &[Quant],
    p: usize,
    converged: bool,
) -> Solution {
    let eigen = eigen_values(prep, quants, p);
    // the power of the root of each eigenvalue that scales the loadings, as in the tables
    let b_pow = match command.common.normalization {
        McaNormalization::Independent => 1.0,
        McaNormalization::Q(q) => (1.0 - q) / 2.0,
    };
    let factor: Vec<f64> = eigen
        .iter()
        .map(|e| match b_pow == 1.0 {
            true => 1.0,
            false => e.max(0.0).sqrt().powf(b_pow - 1.0),
        })
        .collect();
    Solution {
        eigen,
        loadings: quants
            .iter()
            .map(|q| match q.single() {
                true => q.a.iter().zip(&factor).map(|(a, f)| a * f).collect(),
                false => Vec::new(),
            })
            .collect(),
        values: quants
            .iter()
            .map(|q| match q.single() {
                true => q.y.clone(),
                false => Vec::new(),
            })
            .collect(),
        converged,
    }
}

fn grid(title: impl Into<String>, columns: Vec<String>, rows: Vec<Vec<Cell>>) -> Table {
    Table {
        title: title.into(),
        columns,
        rows,
        row_headers: 1,
        ..Table::default()
    }
}

/// Rows of a p-column matrix with row labels.
fn matrix_rows(labels: &[String], m: &Matrix) -> Vec<Vec<Cell>> {
    labels
        .iter()
        .zip(m)
        .map(|(label, row)| {
            let mut out = vec![text(label.clone())];
            out.extend(row.iter().map(|x| num(*x)));
            out
        })
        .collect()
}

fn mat_vec(row: &[f64], m: &Matrix) -> Vec<f64> {
    let p = m.first().map_or(0, Vec::len);
    (0..p)
        .map(|b| row.iter().zip(m).map(|(x, r)| x * r[b]).sum())
        .collect()
}

impl Engine {
    /// The fit: alternating least squares from the start to convergence, then the principal axes
    /// and the reflection rule. Returns the object scores, the steps and whether it converged.
    pub(crate) fn fit_quants(
        &self,
        command: &Catpca,
        prep: &Prepared,
        quants: &mut [Quant],
        p: usize,
        configuration: &Option<(bool, String)>,
    ) -> Result<(Matrix, Vec<Step>, bool), Error> {
        let (avars, w, n_w, fixed) = (&prep.avars, &prep.w, prep.n_w, prep.fixed);
        let is_single: Vec<bool> = quants.iter().map(Quant::single).collect();
        let single = |j: usize| is_single[j];
        let basis: Vec<Option<Vec<f64>>> = prep
            .active_vars
            .iter()
            .map(|&j| single(j).then(|| quants[j].y.clone()))
            .collect();
        let mut x = prep.start(self, WHO, configuration, &basis, p)?;
        let mut history: Vec<Step> = Vec::new();
        let mut converged = true;
        if !fixed {
            for &j in &prep.active_vars {
                let (v, q) = (&avars[j], &mut quants[j]);
                q.centroids = centroids(v, &x, w, p);
                q.a = (0..p)
                    .map(|s| {
                        q.centroids
                            .iter()
                            .zip(&v.d)
                            .zip(&q.y)
                            .map(|((c, d), y)| d * c[s] * y)
                            .sum::<f64>()
                            / n_w
                    })
                    .collect();
            }
            quantify_active(prep, quants, &x, true);
            let mut first = vec![step(prep, quants, p)];
            let max = command.common.max_iterations;
            iterate(
                prep,
                quants,
                &mut x,
                true,
                max,
                command.common.criterion,
                &mut first,
            );
            history.push(*first.last().expect("a first step"));
            converged = iterate(
                prep,
                quants,
                &mut x,
                false,
                max,
                command.common.criterion,
                &mut history,
            );
            // principal axes of the fitted cross products, then the reflection rule
            let mut f = vec![vec![0.0; p]; p];
            for &j in &prep.active_vars {
                let (v, q) = (&avars[j], &quants[j]);
                for a in 0..p {
                    for b in 0..p {
                        f[a][b] += v.weight
                            * match q.single() {
                                true => q.a[a] * q.a[b],
                                false => {
                                    q.centroids
                                        .iter()
                                        .zip(&v.d)
                                        .map(|(c, d)| d * c[a] * c[b])
                                        .sum::<f64>()
                                        / n_w
                                }
                            };
                    }
                }
            }
            let (_, l) = eigen_sorted(&f);
            for row in x.iter_mut() {
                *row = mat_vec(row, &l);
            }
            for q in quants.iter_mut().filter(|q| q.single()) {
                if !q.a.is_empty() {
                    q.a = mat_vec(&q.a, &l);
                }
            }
            let singles: Vec<usize> = prep
                .active_vars
                .iter()
                .copied()
                .filter(|&j| single(j))
                .collect();
            for s in 0..p {
                let flip = match singles.is_empty() {
                    false => {
                        let mean = |neg: bool| {
                            let picked: Vec<f64> = singles
                                .iter()
                                .map(|&j| quants[j].a[s])
                                .filter(|a| (*a < 0.0) == neg && *a != 0.0)
                                .collect();
                            match picked.is_empty() {
                                true => 0.0,
                                false => {
                                    picked.iter().map(|a| a * a).sum::<f64>() / picked.len() as f64
                                }
                            }
                        };
                        mean(true) > mean(false)
                    }
                    true => {
                        // no loadings: the object with the largest absolute score is positive
                        let big = x.iter().map(|r| r[s].abs()).fold(0.0, f64::max);
                        x.iter()
                            .find(|r| r[s].abs() >= big * (1.0 - 1e-9))
                            .is_some_and(|r| r[s] < 0.0)
                    }
                };
                if flip {
                    x.iter_mut().for_each(|r| r[s] = -r[s]);
                    for q in quants.iter_mut().filter(|q| q.single() && !q.a.is_empty()) {
                        q.a[s] = -q.a[s];
                    }
                }
            }
            quantify_active(prep, quants, &x, false);
        }
        Ok((x, history, converged))
    }

    pub(crate) fn catpca(&mut self, command: &Catpca) -> Result<Vec<Table>, Error> {
        if command.resample.is_some() && matches!(command.common.configuration, Some((true, _))) {
            return Err(Error(format!(
                "{WHO}: RESAMPLE=BOOTSTRAP cannot be used with a FIXED configuration"
            )));
        }
        let lists: Vec<(&[VarItem], f64)> = command
            .analysis
            .iter()
            .map(|(items, spec)| (items.as_slice(), spec.weight))
            .collect();
        let prep = self.prepare_scaling(WHO, &command.common, &lists)?;
        let mut warnings = prep.warnings.clone();
        for keyword in &command.ellipses {
            warnings.push(format!(
                "{WHO}: {keyword} draws confidence ellipses, which are not drawn yet; it is ignored"
            ));
        }
        let fixed = prep.fixed;
        let (avars, coded) = (&prep.avars, &prep.coded);
        let (w, n_w, m_w) = (&prep.w, prep.n_w, prep.m_w);
        let n = prep.active.len();

        // the quantification of every variable, at its numeric start
        let mut quants: Vec<Quant> = avars
            .iter()
            .map(|v| Quant::new(v, command.analysis[v.list].1.level, n_w))
            .collect();
        let is_single: Vec<bool> = quants.iter().map(Quant::single).collect();
        let single = |j: usize| is_single[j];

        // dimensions
        let mut p = command.common.dimension;
        if !fixed {
            let act = &prep.active_vars;
            let m2 = act.iter().filter(|&&j| single(j)).count();
            let multiple: Vec<usize> = act.iter().copied().filter(|&j| !single(j)).collect();
            let pmax = match multiple.is_empty() {
                true => m2,
                false => {
                    let k: usize = multiple.iter().map(|&j| avars[j].cats.len()).sum();
                    let complete = multiple
                        .iter()
                        .filter(|&&j| avars[j].h.iter().all(Option::is_some))
                        .count();
                    let less = match complete < multiple.len() {
                        true => complete.max(1),
                        false => multiple.len(),
                    };
                    (k + m2).saturating_sub(less)
                }
            }
            .min(n - 1);
            if pmax < 1 {
                return Err(Error(format!(
                    "{WHO}: the data leave no dimension to compute"
                )));
            }
            if p > pmax {
                warnings.push(format!(
                    "{WHO}: DIMENSION is {p}, but these data allow at most {pmax}; {pmax} are computed"
                ));
                p = pmax;
            }
        }

        // the fit
        let (x, history, converged) = self.fit_quants(
            command,
            &prep,
            &mut quants,
            p,
            &command.common.configuration,
        )?;
        if !converged {
            warnings.push(format!(
                "{WHO}: the fit did not converge within {} iterations",
                command.common.max_iterations
            ));
        }
        for (j, v) in avars.iter().enumerate() {
            if v.supplementary {
                fit_supplementary(v, &mut quants[j], &x, w, n_w);
            }
        }

        // eigenvalues and the parts of the variance accounted for
        let vaf_centroid: Vec<Vec<f64>> = avars
            .iter()
            .zip(&quants)
            .map(|(v, q)| {
                centroid_fit(v, q, n_w, p)
                    .into_iter()
                    .map(|c| v.weight * c)
                    .collect()
            })
            .collect();
        let vaf_vector: Vec<Option<Vec<f64>>> = avars
            .iter()
            .zip(&quants)
            .map(|(v, q)| {
                q.single()
                    .then(|| q.a.iter().map(|a| v.weight * a * a).collect())
            })
            .collect();
        let eigen = eigen_values(&prep, &quants, p);
        let total_eigen: f64 = prep
            .active_vars
            .iter()
            .map(|&j| match &vaf_vector[j] {
                Some(vec) => vec.iter().sum::<f64>(),
                None => vaf_centroid[j].iter().sum::<f64>() / p as f64,
            })
            .sum();

        // normalization: powers of the root of each eigenvalue
        let normalization = command.common.normalization;
        let (a_pow, b_pow) = match (fixed, normalization) {
            (true, _) => (0.0, 1.0),
            (false, McaNormalization::Independent) => (1.0, 1.0),
            (false, McaNormalization::Q(q)) => ((1.0 + q) / 2.0, (1.0 - q) / 2.0),
        };
        if fixed && normalization != McaNormalization::Q(-1.0) {
            warnings.push(format!(
                "{WHO}: with a FIXED configuration the scores keep their own scale; NORMALIZATION is ignored"
            ));
        }
        let sigma: Vec<f64> = eigen.iter().map(|e| e.max(0.0).sqrt()).collect();
        let power = |s: usize, k: f64| match k == 0.0 {
            true => 1.0,
            false => sigma[s].powf(k),
        };
        let x_factor: Vec<f64> = (0..p).map(|s| power(s, a_pow)).collect();
        let v_factor: Vec<f64> = (0..p).map(|s| power(s, b_pow - 1.0)).collect();

        // scores of every case: active from the fit, the others through the last update
        let mut scores: Vec<Option<Vec<f64>>> = vec![None; prep.n_tot];
        for (i, &c) in prep.active.iter().enumerate() {
            scores[c] = Some(x[i].clone());
        }
        let others: Vec<usize> = (0..prep.n_tot)
            .filter(|&c| !prep.excluded[c] && scores[c].is_none() && !fixed)
            .collect();
        if !others.is_empty() {
            let z: Matrix = object_sums(&prep, &quants, p, None)
                .into_iter()
                .map(|mut r| r.pop().expect("an active value"))
                .collect();
            if let Some((means, t)) = orthonormal_transform(&z, &prep.metric(), n_w * m_w) {
                for c in others {
                    let mut row = object_sums(&prep, &quants, p, Some(c)).remove(0);
                    if row.is_empty() {
                        continue;
                    }
                    transform_rows(&mut row, &means, &t);
                    scores[c] = row.pop();
                }
            }
        }

        // the rotation of the loadings of the active single variables
        let mut normalized_a: Vec<Vec<f64>> = quants
            .iter()
            .map(|q| match q.single() {
                true => {
                    q.a.iter()
                        .enumerate()
                        .map(|(s, a)| a * v_factor[s])
                        .collect()
                }
                false => Vec::new(),
            })
            .collect();
        let unrotated = normalized_a.clone();
        // the transformation T of the rotation: scores and centroids times T, loadings
        // (pattern) times the inverse of T'
        let mut rotation: Option<(Matrix, Option<Matrix>)> = None;
        if let Some((method, kaiser)) = command.rotation
            && method != Rotation::None
        {
            let rows: Vec<usize> = prep
                .active_vars
                .iter()
                .copied()
                .filter(|&j| single(j))
                .collect();
            if p < 2 || rows.len() < 2 {
                warnings.push(format!(
                    "{WHO}: a rotation needs two dimensions and two variables with loadings; none is done"
                ));
            } else {
                let a: Matrix = rows.iter().map(|&j| normalized_a[j].clone()).collect();
                let criteria = FactorCriteria {
                    kaiser,
                    iterate: 100,
                    ..FactorCriteria::default()
                };
                let mut notes = Vec::new();
                let rotated = match method {
                    Rotation::Oblimin(delta) => oblimin(&a, delta, &criteria, &mut notes),
                    Rotation::Promax(kappa) => promax(&a, kappa, &criteria, &mut notes),
                    other => orthogonal(&a, other, &criteria, &mut notes),
                };
                warnings.extend(notes.into_iter().map(|n| n.replacen("FACTOR", WHO, 1)));
                // A = P T', so T = A'P (P'P)^-1
                let pt = transpose(&rotated.pattern);
                let t = inverse(&multiply(&pt, &rotated.pattern))
                    .map(|(inv, _)| multiply(&multiply(&transpose(&a), &rotated.pattern), &inv));
                match t.and_then(|t| inverse(&transpose(&t)).map(|(inv, _)| (t, inv))) {
                    Some((t, t_inv)) => {
                        for row in normalized_a.iter_mut().filter(|r| !r.is_empty()) {
                            *row = mat_vec(row, &t_inv);
                        }
                        rotation = Some((t, rotated.phi));
                    }
                    None => warnings.push(format!("{WHO}: the rotation failed; none is done")),
                }
            }
        }
        let turn = |point: Vec<f64>| -> Vec<f64> {
            match &rotation {
                Some((t, _)) => mat_vec(&point, t),
                None => point,
            }
        };
        let object_point = |score: &[f64]| -> Vec<f64> {
            turn(score.iter().zip(&x_factor).map(|(a, b)| a * b).collect())
        };
        let shown_scores: Vec<Option<Vec<f64>>> = scores
            .iter()
            .map(|s| s.as_deref().map(object_point))
            .collect();
        // categories that only supplementary objects have: the plain mean of their scores
        let mut extra_cats: Vec<BTreeMap<i64, (f64, Vec<f64>)>> =
            vec![BTreeMap::new(); avars.len()];
        for (j, v) in avars.iter().enumerate() {
            for (c, score) in scores.iter().enumerate() {
                let (Some(code), Some(score)) = (v.codes[c], score) else {
                    continue;
                };
                if prep.active.binary_search(&c).is_ok()
                    || v.cats.binary_search(&(code as i64)).is_ok()
                {
                    continue;
                }
                let e = extra_cats[j]
                    .entry(code as i64)
                    .or_insert((0.0, vec![0.0; p]));
                e.0 += 1.0;
                e.1.iter_mut().zip(score).for_each(|(a, b)| *a += b);
            }
            for (count, sum) in extra_cats[j].values_mut() {
                sum.iter_mut().for_each(|x| *x /= *count);
            }
        }
        // the centroid coordinates shown for code `code` of variable j
        let centroid_of = |j: usize, code: i64| -> Option<Vec<f64>> {
            let v = &avars[j];
            let (raw, multiple) = match v.cats.binary_search(&code) {
                Ok(r) => (quants[j].centroids[r].clone(), !quants[j].single()),
                Err(_) => (extra_cats[j].get(&code)?.1.clone(), !quants[j].single()),
            };
            let factor = match multiple {
                true => &v_factor,
                false => &x_factor,
            };
            Some(turn(raw.iter().zip(factor).map(|(a, b)| a * b).collect()))
        };
        let vector_of = |j: usize, r: usize| -> Vec<f64> {
            normalized_a[j].iter().map(|a| quants[j].y[r] * a).collect()
        };

        // tables
        let print = &command.print;
        let mut tables = vec![prep.case_summary()];
        if let Some(items) = &print.descrip {
            tables.extend(prep.descriptives(WHO, items)?);
        }
        let titles: Vec<String> = avars.iter().map(|v| coded[v.coded].title.clone()).collect();
        let row_title = |j: usize| match avars[j].supplementary {
            true => format!("{} (supplementary)", titles[j]),
            false => titles[j].clone(),
        };
        let dims: Vec<String> = (1..=p).map(|s| format!("Dimension {s}")).collect();
        let grouped = |first: Vec<String>, groups: &[(&str, Vec<String>)]| {
            let mut columns = first.clone();
            let mut column_groups = vec![Vec::new(); first.len()];
            for (group, names) in groups {
                for name in names {
                    columns.push(name.clone());
                    column_groups.push(vec![group.to_string()]);
                }
            }
            (columns, column_groups)
        };
        if !fixed {
            let restricted = prep.active_vars.iter().any(|&j| {
                let q = &quants[j];
                !matches!(q.kind, Kind::Numeric | Kind::Multiple) || q.extra.is_some()
            });
            let picked: Vec<usize> = match print.history {
                true => (0..history.len()).collect(),
                false => {
                    let mut rows = vec![history.len() - 1];
                    if restricted && history.len() > 1 {
                        rows.insert(0, 0);
                    }
                    rows
                }
            };
            let rows = picked
                .iter()
                .filter(|&&k| k > 0 || restricted)
                .map(|&k| {
                    let h = history[k];
                    vec![
                        Cell::Number(k as f64),
                        Cell::Number(h.fit),
                        match k {
                            0 => Cell::Empty,
                            k => Cell::Number(h.fit - history[k - 1].fit),
                        },
                        Cell::Number(h.multiple_loss + h.single_loss),
                        Cell::Number(h.multiple_loss),
                        Cell::Number(h.single_loss),
                    ]
                })
                .collect();
            let (columns, column_groups) = grouped(
                vec!["Iteration Number".into()],
                &[
                    (
                        "Variance Accounted For",
                        vec!["Total".into(), "Increase".into()],
                    ),
                    (
                        "Variance Not Accounted For",
                        vec![
                            "Total".into(),
                            "Centroid Coordinates".into(),
                            "Restriction of Centroid to Vector Coordinates".into(),
                        ],
                    ),
                ],
            );
            tables.push(Table {
                title: "Iteration History".into(),
                columns,
                column_groups,
                rows,
                row_headers: 1,
                caption: Some(match converged {
                    true => "The fit stopped gaining more than the convergence criterion. Iteration 0 is the last step of the first phase, in which every variable but the multiple nominal ones is numeric.".into(),
                    false => "The most iterations were reached.".into(),
                }),
                ..Table::default()
            });
            let alpha = |e: f64| match m_w > 1.0 && e != 0.0 {
                true => num(m_w * (e - 1.0) / ((m_w - 1.0) * e)),
                false => Cell::Empty,
            };
            let mut rows: Vec<Vec<Cell>> = (0..p)
                .map(|s| {
                    vec![
                        text((s + 1).to_string()),
                        alpha(eigen[s]),
                        Cell::Number(eigen[s]),
                        Cell::Number(100.0 * eigen[s] / m_w),
                    ]
                })
                .collect();
            rows.push(vec![
                text("Total"),
                alpha(eigen.iter().sum()),
                Cell::Number(total_eigen),
                Cell::Number(100.0 * total_eigen / m_w),
            ]);
            let (columns, column_groups) = grouped(
                vec!["Dimension".into(), "Cronbach's Alpha".into()],
                &[(
                    "Variance Accounted For",
                    vec!["Total (Eigenvalue)".into(), "% of Variance".into()],
                )],
            );
            tables.push(Table {
                title: "Model Summary".into(),
                columns,
                column_groups,
                rows,
                row_headers: 1,
                caption: Some(
                    "The Total eigenvalue sums the dimensions for single variables and averages them for multiple nominal ones; the Total alpha comes from the summed eigenvalues of the dimensions."
                        .into(),
                ),
                ..Table::default()
            });
        }
        let analysis_names = &prep.analysis_names;
        if let Some(items) = &print.quant {
            let names = match items.is_empty() {
                true => analysis_names.clone(),
                false => resolve(WHO, items, analysis_names, analysis_names)?,
            };
            for name in names {
                let j = prep.analysis_index(&name);
                let (v, q) = (&avars[j], &quants[j]);
                let c = &coded[v.coded];
                let mut first = vec![String::new(), "Frequency".into()];
                let mut groups = vec![("Centroid Coordinates", dims.clone())];
                if q.single() {
                    first.push("Quantification".into());
                    groups.push(("Vector Coordinates", dims.clone()));
                }
                let (columns, column_groups) = grouped(first, &groups);
                let mut rows = Vec::new();
                for (r, code) in v.cats.iter().enumerate() {
                    let mut row = vec![text(c.label(*code, None)), Cell::Number(v.d[r])];
                    if q.single() {
                        row.push(Cell::Number(q.y[r]));
                    }
                    row.extend(
                        centroid_of(j, *code)
                            .expect("an active category")
                            .into_iter()
                            .map(num),
                    );
                    if q.single() {
                        row.extend(vector_of(j, r).into_iter().map(num));
                    }
                    rows.push(row);
                }
                for (code, (count, _)) in &extra_cats[j] {
                    let mut row = vec![
                        text(format!("{} (supplementary objects)", c.label(*code, None))),
                        Cell::Number(*count),
                    ];
                    if q.single() {
                        row.push(Cell::Empty);
                    }
                    row.extend(
                        centroid_of(j, *code)
                            .expect("a supplementary category")
                            .into_iter()
                            .map(num),
                    );
                    row.resize(columns.len(), Cell::Empty);
                    rows.push(row);
                }
                tables.push(Table {
                    title: format!("Quantifications: {}", c.title),
                    columns,
                    column_groups,
                    rows,
                    row_headers: 1,
                    ..Table::default()
                });
            }
        }
        if print.vaf {
            let mean_total = |mut d: Vec<String>, last: &str| {
                d.push(last.to_string());
                d
            };
            let (columns, column_groups) = grouped(
                vec![String::new()],
                &[
                    ("Centroid Coordinates", mean_total(dims.clone(), "Mean")),
                    ("Vector Coordinates", mean_total(dims.clone(), "Total")),
                ],
            );
            let line = |label: String, centroid: &[f64], vector: Option<&Vec<f64>>| {
                let mut row = vec![text(label)];
                row.extend(centroid.iter().map(|x| Cell::Number(*x)));
                row.push(Cell::Number(centroid.iter().sum::<f64>() / p as f64));
                match vector {
                    Some(vec) => {
                        row.extend(vec.iter().map(|x| Cell::Number(*x)));
                        row.push(Cell::Number(vec.iter().sum()));
                    }
                    None => row.resize(2 * p + 3, Cell::Empty),
                }
                row
            };
            let mut rows: Vec<Vec<Cell>> = (0..avars.len())
                .map(|j| line(row_title(j), &vaf_centroid[j], vaf_vector[j].as_ref()))
                .collect();
            if !fixed {
                let sum = |f: &dyn Fn(usize) -> Option<Vec<f64>>| -> Vec<f64> {
                    (0..p)
                        .map(|s| {
                            prep.active_vars
                                .iter()
                                .filter_map(|&j| f(j).map(|v| v[s]))
                                .sum()
                        })
                        .collect()
                };
                let centroid_total = sum(&|j| Some(vaf_centroid[j].clone()));
                let vector_total = sum(&|j| vaf_vector[j].clone());
                rows.push(line(
                    "Active Total".into(),
                    &centroid_total,
                    Some(&vector_total),
                ));
            }
            tables.push(Table {
                title: "Variance Accounted For".into(),
                columns,
                column_groups,
                rows,
                row_headers: 1,
                caption: Some(
                    "Centroid coordinates: the weighted variance of a variable's category centroids; vector coordinates: its squared loadings. Both carry the variable weight."
                        .into(),
                ),
                ..Table::default()
            });
        }
        let active_positions: Vec<(usize, f64)> = prep
            .active_vars
            .iter()
            .map(|&j| (j, avars[j].weight))
            .collect();
        let mode_of = |j: usize| -> Option<usize> {
            let d = &avars[j].d;
            (0..d.len()).fold(None, |best: Option<usize>, r| match best {
                Some(b) if d[b] >= d[r] => best,
                _ => Some(r),
            })
        };
        if print.ocorr {
            let columns: Vec<Vec<f64>> = (0..avars.len())
                .map(|j| {
                    let v = &avars[j];
                    (0..n)
                        .map(|i| match v.h[i].or_else(|| mode_of(j)) {
                            Some(r) => v.cats[r] as f64,
                            None => f64::NAN,
                        })
                        .collect()
                })
                .collect();
            tables.push(correlation_table(
                "Correlations Original Variables".into(),
                &titles,
                &correlations(&columns, w),
                &active_positions,
            ));
        }
        if print.corr {
            // random categories for PASSIVE RANDIMPU, drawn once
            let mut drawn: Vec<Vec<Option<usize>>> = Vec::new();
            for v in avars {
                let total: f64 = v.d.iter().sum();
                let random = matches!(v.missing, McaMissing::Passive(Impute::Random));
                drawn.push(
                    v.h.iter()
                        .map(|h| {
                            (h.is_none() && random && total > 0.0).then(|| {
                                let u = self.random.borrow_mut().uniform() * total;
                                let mut cum = 0.0;
                                v.d.iter()
                                    .position(|d| {
                                        cum += d;
                                        cum >= u
                                    })
                                    .unwrap_or(v.d.len() - 1)
                            })
                        })
                        .collect(),
                );
            }
            let any_multiple = prep.active_vars.iter().any(|&j| !single(j));
            let layers = if any_multiple { p } else { 1 };
            for s in 0..layers {
                let columns: Vec<Vec<f64>> = (0..avars.len())
                    .map(|j| {
                        let (v, q) = (&avars[j], &quants[j]);
                        let missing_mean = || {
                            let (mut sw, mut sx) = (vec![0.0; p], 0.0);
                            for (i, h) in v.h.iter().enumerate() {
                                if h.is_none() {
                                    sx += w[i];
                                    for t in 0..p {
                                        sw[t] += w[i] * x[i][t];
                                    }
                                }
                            }
                            sw.iter().map(|z| z / sx).collect::<Vec<f64>>()
                        };
                        // the value of category r, and of the extra category of the gaps
                        let (value, extra): (Box<dyn Fn(usize) -> f64>, f64) = match q.single() {
                            true => {
                                let aa: f64 = q.a.iter().map(|a| a * a).sum();
                                let mean = missing_mean();
                                let extra =
                                    mean.iter().zip(&q.a).map(|(m, a)| m * a).sum::<f64>() / aa;
                                (Box::new(move |r: usize| q.y[r]), extra)
                            }
                            false => {
                                let ss: f64 = centroid_fit(v, q, n_w, p)[s];
                                let scale = if ss > 0.0 { 1.0 / ss.sqrt() } else { 0.0 };
                                let extra = missing_mean()[s] * scale;
                                (Box::new(move |r: usize| q.centroids[r][s] * scale), extra)
                            }
                        };
                        (0..n)
                            .map(|i| match (v.h[i], v.missing) {
                                (Some(r), _) => value(r),
                                (None, McaMissing::Passive(Impute::ExtraCategory)) => extra,
                                (None, McaMissing::Passive(Impute::Random)) => {
                                    drawn[j][i].map_or(f64::NAN, &value)
                                }
                                (None, _) => mode_of(j).map_or(f64::NAN, &value),
                            })
                            .collect()
                    })
                    .collect();
                let title = match any_multiple {
                    true => format!("Correlations Transformed Variables: Dimension {}", s + 1),
                    false => "Correlations Transformed Variables".into(),
                };
                tables.push(correlation_table(
                    title,
                    &titles,
                    &correlations(&columns, w),
                    &active_positions,
                ));
            }
        }
        let object_label = |case: usize, label: &Option<usize>| match label {
            Some(l) => coded[*l]
                .codes
                .get(case)
                .copied()
                .flatten()
                .map(|code| coded[*l].label(code as i64, None))
                .unwrap_or_else(|| (case + 1).to_string()),
            None => (case + 1).to_string(),
        };
        let variables = &prep.variables;
        if let Some((label, items)) = &print.object {
            let label = match label {
                Some(name) => {
                    let found = resolve(WHO, &[VarItem::Name(name.clone())], variables, variables)?;
                    Some(coded_index(coded, &found[0]))
                }
                None => None,
            };
            let shown_vars: Vec<usize> = resolve(WHO, items, analysis_names, variables)?
                .iter()
                .map(|name| coded_index(coded, name))
                .collect();
            let mut columns = vec![String::new()];
            columns.extend(shown_vars.iter().map(|&k| coded[k].title.clone()));
            columns.extend(dims.iter().cloned());
            let mut rows = Vec::new();
            let mut sups = Vec::new();
            for (case, score) in shown_scores.iter().enumerate() {
                let Some(score) = score else { continue };
                if prep.active.binary_search(&case).is_err() {
                    sups.push((case + 1).to_string());
                }
                let mut row = vec![text(object_label(case, &label))];
                row.extend(shown_vars.iter().map(|&k| match coded[k].codes[case] {
                    Some(code) => Cell::Number(code),
                    None => Cell::Empty,
                }));
                row.extend(score.iter().map(|x| Cell::Number(*x)));
                rows.push(row);
            }
            tables.push(Table {
                title: "Object Scores".into(),
                columns,
                rows,
                row_headers: 1,
                caption: (!sups.is_empty())
                    .then(|| format!("Supplementary objects: {}.", sups.join(", "))),
                ..Table::default()
            });
        }
        let single_vars: Vec<usize> = (0..avars.len()).filter(|&j| single(j)).collect();
        if let Some(sorted) = print.loading
            && !single_vars.is_empty()
        {
            let mut columns = vec![String::new()];
            columns.extend(dims.iter().cloned());
            let loading_table = |title: &str, m: &Matrix| {
                let order: Vec<usize> = match sorted {
                    true => sorted_rows(m),
                    false => (0..m.len()).collect(),
                };
                let labels: Vec<String> =
                    order.iter().map(|&k| row_title(single_vars[k])).collect();
                let ordered: Matrix = order.iter().map(|&k| m[k].clone()).collect();
                grid(title, columns.clone(), matrix_rows(&labels, &ordered))
            };
            let m: Matrix = single_vars.iter().map(|&j| unrotated[j].clone()).collect();
            tables.push(loading_table("Component Loadings", &m));
            if let Some((t, phi)) = &rotation {
                let pattern: Matrix = single_vars
                    .iter()
                    .map(|&j| normalized_a[j].clone())
                    .collect();
                match phi {
                    None => {
                        tables.push(loading_table("Rotated Component Loadings", &pattern));
                        let labels: Vec<String> = (1..=p).map(|s| s.to_string()).collect();
                        let mut cols = vec!["Component".to_string()];
                        cols.extend((1..=p).map(|s| s.to_string()));
                        tables.push(grid(
                            "Component Transformation Matrix",
                            cols,
                            matrix_rows(&labels, t),
                        ));
                    }
                    Some(phi) => {
                        tables.push(loading_table("Pattern Matrix", &pattern));
                        let structure: Matrix = single_vars
                            .iter()
                            .map(|&j| mat_vec(&unrotated[j], t))
                            .collect();
                        tables.push(loading_table("Structure Matrix", &structure));
                        let labels: Vec<String> = (1..=p).map(|s| s.to_string()).collect();
                        let mut cols = vec!["Component".to_string()];
                        cols.extend((1..=p).map(|s| s.to_string()));
                        tables.push(grid(
                            "Component Correlation Matrix",
                            cols,
                            matrix_rows(&labels, phi),
                        ));
                    }
                }
            }
        }

        if let Some(spec) = &command.resample {
            let original = summarize(command, &prep, &quants, p, converged);
            let more = self.catpca_bootstrap(command, spec, &prep, &original, p, &mut warnings)?;
            tables.extend(more);
        }
        // charts
        let data = self.active()?;
        let mut charts = Vec::new();
        let ndim = command.plot.ndim;
        if let Some((a, b)) = ndim
            && b > p
        {
            warnings.push(format!(
                "{WHO}: NDIM({a},{b}) goes beyond the {p} dimensions of the solution"
            ));
        }
        let analysis_of = |k: usize| avars.iter().position(|v| v.coded == k);
        let plot_vars = |items: &[VarItem], allowed: &[String]| -> Result<Vec<usize>, Error> {
            Ok(resolve(WHO, items, analysis_names, allowed)?
                .iter()
                .map(|name| coded_index(coded, name))
                .collect())
        };
        let picked =
            |items: &[VarItem], keep: &dyn Fn(usize) -> bool| -> Result<Vec<usize>, Error> {
                Ok(match items.is_empty() {
                    true => (0..avars.len()).filter(|&j| keep(j)).collect(),
                    false => plot_vars(items, analysis_names)?
                        .into_iter()
                        .filter_map(analysis_of)
                        .filter(|&j| keep(j))
                        .collect(),
                })
            };
        let var_title = |j: usize, length: Option<usize>| {
            let c = &coded[avars[j].coded];
            cut(c.title.clone(), c.name.clone(), length)
        };
        let category_points = |j: usize, length: Option<usize>, scale: f64| {
            let v = &avars[j];
            let c = &coded[v.coded];
            v.cats
                .iter()
                .copied()
                .chain(extra_cats[j].keys().copied())
                .filter_map(|code| {
                    let point = centroid_of(j, code)?;
                    Some((
                        c.label(code, length),
                        point.iter().map(|x| x * scale).collect(),
                    ))
                })
                .collect::<Vec<_>>()
        };
        let loading_points = |js: &[usize], length: Option<usize>| -> Vec<(String, Vec<f64>)> {
            js.iter()
                .map(|&j| (var_title(j, length), normalized_a[j].clone()))
                .collect()
        };
        // objects and centroids shrink to the loadings in plots that show both
        let scale_factor = {
            let top: f64 = (0..p)
                .map(|s| {
                    single_vars
                        .iter()
                        .map(|&j| normalized_a[j][s])
                        .fold(f64::NEG_INFINITY, f64::max)
                })
                .filter(|v| v.is_finite())
                .sum();
            let spread: f64 = (0..p)
                .map(|s| {
                    let values = shown_scores.iter().flatten().map(|r| r[s]);
                    let lo = values.clone().fold(f64::INFINITY, f64::min);
                    let hi = values.fold(f64::NEG_INFINITY, f64::max);
                    lo.abs() + hi
                })
                .sum();
            match spread > 0.0 && top > 0.0 {
                true => 2.0 * top / spread,
                false => 1.0,
            }
        };
        let scaled_objects = |by: Option<usize>, length: Option<usize>, scale: f64| {
            let scaled: Vec<Option<Vec<f64>>> = shown_scores
                .iter()
                .map(|s| s.as_ref().map(|r| r.iter().map(|x| x * scale).collect()))
                .collect();
            object_groups(coded, &scaled, by, length)
        };
        let labelled = |items: &[VarItem]| -> Result<Vec<Option<usize>>, Error> {
            let by = plot_vars(items, variables)?;
            Ok(match by.is_empty() {
                true => vec![None],
                false => by.into_iter().map(Some).collect(),
            })
        };
        let labelled_title = |base: &str, k: Option<usize>, length: Option<usize>| match k {
            None => base.to_string(),
            Some(k) => format!(
                "{base} Labeled by {}",
                cut(coded[k].title.clone(), coded[k].name.clone(), length)
            ),
        };
        let independent = normalization == McaNormalization::Independent;
        for (chart, length) in &command.plot.charts {
            let length = *length;
            match chart {
                CatpcaPlot::Object(items) => {
                    for k in labelled(items)? {
                        let title = labelled_title("Object Points", k, length);
                        charts.extend(scatter(&title, &scaled_objects(k, length, 1.0), p, ndim));
                    }
                }
                CatpcaPlot::Loading(items, centr) => {
                    let js = picked(items, &|j| single(j))?;
                    if js.is_empty() {
                        warnings.push(format!(
                            "{WHO}: the loadings plot needs a variable that is not multiple nominal"
                        ));
                        continue;
                    }
                    let mut groups: GroupList =
                        vec![("Component Loadings".into(), loading_points(&js, length))];
                    let title = match centr {
                        Some(centr) => {
                            for j in picked(centr, &|j| !single(j))? {
                                groups.push((
                                    var_title(j, length),
                                    category_points(j, length, scale_factor),
                                ));
                            }
                            "Component Loadings and Centroids"
                        }
                        None => "Component Loadings",
                    };
                    charts.extend(scatter(title, &groups, p, ndim));
                }
                CatpcaPlot::Category(items) => {
                    for j in picked(items, &|_| true)? {
                        let mut groups = vec![(
                            "Centroid Coordinates".to_string(),
                            category_points(j, length, 1.0),
                        )];
                        if single(j) {
                            let c = &coded[avars[j].coded];
                            groups.push((
                                "Vector Coordinates".into(),
                                (0..avars[j].cats.len())
                                    .map(|r| (c.label(avars[j].cats[r], length), vector_of(j, r)))
                                    .collect(),
                            ));
                        }
                        let title = format!("Category Points: {}", var_title(j, length));
                        charts.extend(scatter(&title, &groups, p, ndim));
                    }
                }
                CatpcaPlot::Jointcat(items) => {
                    let groups: Vec<_> = picked(items, &|_| true)?
                        .into_iter()
                        .map(|j| (var_title(j, length), category_points(j, length, 1.0)))
                        .collect();
                    charts.extend(scatter("Joint Plot of Category Points", &groups, p, ndim));
                }
                CatpcaPlot::Trans(items) | CatpcaPlot::Resid(items) => {
                    let trans = matches!(chart, CatpcaPlot::Trans(_));
                    for (item, count) in items {
                        for name in resolve(
                            WHO,
                            std::slice::from_ref(item),
                            analysis_names,
                            analysis_names,
                        )? {
                            let j = prep.analysis_index(&name);
                            let (v, q) = (&avars[j], &quants[j]);
                            let c = &coded[v.coded];
                            let layers = match q.single() {
                                true => 1,
                                false => (*count).min(p),
                            };
                            if !q.single() && *count > p {
                                warnings.push(format!(
                                    "{WHO}: {name} asks for {count} dimensions in a plot, but the solution has {p}"
                                ));
                            }
                            for s in 0..layers {
                                let title = match q.single() {
                                    true => format!(
                                        "{}: {}",
                                        if trans { "Transformation" } else { "Residuals" },
                                        var_title(j, length)
                                    ),
                                    false => format!(
                                        "{}: {}, Dimension {}",
                                        if trans { "Transformation" } else { "Residuals" },
                                        var_title(j, length),
                                        s + 1
                                    ),
                                };
                                if trans {
                                    let values = v
                                        .cats
                                        .iter()
                                        .map(|code| match q.single() {
                                            true => {
                                                q.y[v.cats.binary_search(code).expect("a category")]
                                            }
                                            false => {
                                                centroid_of(j, *code).map_or(f64::NAN, |p| p[s])
                                            }
                                        })
                                        .collect();
                                    let series = match q.single() {
                                        true => "Quantification".to_string(),
                                        false => format!("Dimension {}", s + 1),
                                    };
                                    charts.push(transformation_chart(
                                        title, c, &v.cats, &series, values,
                                    ));
                                } else {
                                    let points = prep
                                        .active
                                        .iter()
                                        .enumerate()
                                        .filter_map(|(i, &case)| {
                                            let r = v.h[i]?;
                                            let score = shown_scores[case].as_ref()?;
                                            let (approx, quantified) = match q.single() {
                                                true => (
                                                    score
                                                        .iter()
                                                        .zip(&normalized_a[j])
                                                        .map(|(a, b)| a * b)
                                                        .sum(),
                                                    q.y[r],
                                                ),
                                                false => (score[s], centroid_of(j, v.cats[r])?[s]),
                                            };
                                            Some(Point {
                                                x: approx,
                                                y: quantified,
                                                z: None,
                                                label: Some((case + 1).to_string()),
                                            })
                                        })
                                        .collect();
                                    charts.push(point_chart(
                                        title,
                                        "Approximation",
                                        "Quantification",
                                        c.title.clone(),
                                        points,
                                    ));
                                }
                            }
                        }
                    }
                }
                CatpcaPlot::Biplot {
                    centroids: with_centroids,
                    variables: items,
                    labels,
                } => {
                    if independent {
                        warnings.push(format!(
                            "{WHO}: a biplot is not drawn with NORMALIZATION=INDEPENDENT"
                        ));
                        continue;
                    }
                    let (extra_groups, scale): (GroupList, f64) = match with_centroids {
                        true => (
                            picked(items, &|_| true)?
                                .into_iter()
                                .map(|j| (var_title(j, length), category_points(j, length, 1.0)))
                                .collect(),
                            1.0,
                        ),
                        false => {
                            let js = picked(items, &|j| single(j))?;
                            (
                                vec![("Component Loadings".into(), loading_points(&js, length))],
                                scale_factor,
                            )
                        }
                    };
                    for k in labelled(labels)? {
                        let title = labelled_title("Biplot", k, length);
                        let mut groups = scaled_objects(k, length, scale);
                        groups.extend(extra_groups.iter().cloned());
                        charts.extend(scatter(&title, &groups, p, ndim));
                    }
                }
                CatpcaPlot::Triplot(items, labels) => {
                    if independent {
                        warnings.push(format!(
                            "{WHO}: a triplot is not drawn with NORMALIZATION=INDEPENDENT"
                        ));
                        continue;
                    }
                    let js = picked(items, &|_| true)?;
                    let vectors: Vec<usize> = js.iter().copied().filter(|&j| single(j)).collect();
                    let mut extra_groups = vec![(
                        "Component Loadings".to_string(),
                        loading_points(&vectors, length),
                    )];
                    for &j in js.iter().filter(|&&j| !single(j)) {
                        extra_groups.push((
                            var_title(j, length),
                            category_points(j, length, scale_factor),
                        ));
                    }
                    for k in labelled(labels)? {
                        let title = labelled_title("Triplot", k, length);
                        let mut groups = scaled_objects(k, length, scale_factor);
                        groups.extend(extra_groups.iter().cloned());
                        charts.extend(scatter(&title, &groups, p, ndim));
                    }
                }
                CatpcaPlot::Projcentr(from, onto) => {
                    let from = resolve(
                        WHO,
                        &[VarItem::Name(from.clone())],
                        analysis_names,
                        analysis_names,
                    )?;
                    let l = prep.analysis_index(&from[0]);
                    let mut targets = Vec::new();
                    for name in resolve(WHO, onto, analysis_names, analysis_names)? {
                        let j = prep.analysis_index(&name);
                        if !single(j) {
                            return Err(Error(format!(
                                "{WHO}: PROJCENTR cannot project onto {name}, which is multiple nominal"
                            )));
                        }
                        targets.push(j);
                    }
                    let c = &coded[avars[l].coded];
                    let codes = &avars[l].cats;
                    // the centroid coordinates of each category on each variable's direction
                    let projected: Vec<Vec<f64>> = codes
                        .iter()
                        .map(|&code| {
                            let raw = turn(
                                quants[l].centroids
                                    [avars[l].cats.binary_search(&code).expect("a category")]
                                .iter()
                                .zip(&x_factor)
                                .map(|(a, b)| a * b)
                                .collect(),
                            );
                            targets
                                .iter()
                                .map(|&j| {
                                    let a = &normalized_a[j];
                                    let len = a.iter().map(|x| x * x).sum::<f64>().sqrt();
                                    raw.iter().zip(a).map(|(x, y)| x * y).sum::<f64>() / len
                                })
                                .collect()
                        })
                        .collect();
                    let mut columns = vec![String::new()];
                    columns.extend(targets.iter().map(|&j| titles[j].clone()));
                    let labels: Vec<String> =
                        codes.iter().map(|code| c.label(*code, None)).collect();
                    tables.push(grid(
                        format!("Projected Centroids: {}", c.title),
                        columns,
                        matrix_rows(&labels, &projected),
                    ));
                    let categories = targets
                        .iter()
                        .map(|&j| Category {
                            label: var_title(j, length),
                            value: Cell::Text(titles[j].clone()),
                            missing: false,
                            bin: None,
                        })
                        .collect();
                    let series = codes
                        .iter()
                        .enumerate()
                        .map(|(r, code)| Series {
                            name: c.label(*code, length),
                            statistic: Statistic::Value,
                            variable: None,
                            value: Cell::Empty,
                            values: projected[r].clone(),
                        })
                        .collect();
                    charts.push(Chart::lines(
                        format!("Projected Centroids of {}", var_title(l, length)),
                        Axis::titled("Variables"),
                        Axis::titled("Projected Centroid"),
                        categories,
                        series,
                    ));
                }
                CatpcaPlot::Vaf => {
                    let js: Vec<usize> = (0..avars.len()).collect();
                    let share = |j: usize, s: Option<usize>| -> f64 {
                        match (&vaf_vector[j], s) {
                            (Some(v), Some(s)) => v[s],
                            (Some(v), None) => v.iter().sum(),
                            (None, Some(s)) => vaf_centroid[j][s],
                            (None, None) => vaf_centroid[j].iter().sum::<f64>() / p as f64,
                        }
                    };
                    for s in (0..p).map(Some).chain([None]) {
                        let categories = js
                            .iter()
                            .map(|&j| Category {
                                label: var_title(j, length),
                                value: Cell::Text(titles[j].clone()),
                                missing: false,
                                bin: None,
                            })
                            .collect();
                        let name = match s {
                            Some(s) => format!("Dimension {}", s + 1),
                            None => "Total".to_string(),
                        };
                        let mut chart = Chart::lines(
                            format!("Variance Accounted For: {name}"),
                            Axis::titled("Variables"),
                            Axis::titled("Variance Accounted For"),
                            categories,
                            vec![Series {
                                name,
                                statistic: Statistic::Value,
                                variable: None,
                                value: Cell::Empty,
                                values: js.iter().map(|&j| share(j, s)).collect(),
                            }],
                        );
                        chart.kind = ChartKind::Bar;
                        charts.push(chart);
                    }
                }
            }
        }

        // the values of each case for SAVE and OUTFILE
        let multiple_count =
            |asked: Option<usize>, what: &str, warnings: &mut Vec<String>| match asked {
                Some(k) if k > p => {
                    warnings.push(format!(
                        "{WHO}: SAVE {what} asks for {k} dimensions, but the solution has {p}"
                    ));
                    p
                }
                Some(k) => k,
                None => p,
            };
        // the quantified value of each case: the single value, or the centroid in dimension s
        let transformed = |j: usize, s: usize| -> Vec<Option<f64>> {
            (0..prep.n_tot)
                .map(|c| {
                    if prep.excluded[c] {
                        return None;
                    }
                    let code = avars[j].codes[c]? as i64;
                    match single(j) {
                        true => avars[j]
                            .cats
                            .binary_search(&code)
                            .ok()
                            .map(|r| quants[j].y[r]),
                        false => centroid_of(j, code).map(|q| q[s]),
                    }
                })
                .collect()
        };
        let object_column = |s: usize| -> Vec<Option<f64>> {
            shown_scores
                .iter()
                .map(|score| score.as_ref().map(|sc| sc[s]))
                .collect()
        };
        let approximation = |j: usize| -> Vec<Option<f64>> {
            shown_scores
                .iter()
                .map(|score| {
                    let score = score.as_ref()?;
                    Some(match single(j) {
                        true => score.iter().zip(&normalized_a[j]).map(|(a, b)| a * b).sum(),
                        false => score[0],
                    })
                })
                .collect()
        };
        let mut saves: Vec<(String, String, Vec<Option<f64>>)> = Vec::new();
        if let Some((root, asked)) = &command.save_trdata {
            let count = multiple_count(*asked, "TRDATA", &mut warnings);
            let short: String = root.chars().take(3).collect();
            let name_of = |j: usize, s: usize, set: usize| match single(j) {
                true => format!("{root}{}_{set}", j + 1),
                false => format!("{short}{}_{}_{set}", j + 1, s + 1),
            };
            let names = |set: usize| -> Vec<String> {
                (0..avars.len())
                    .flat_map(|j| {
                        let dims = if single(j) { 1 } else { count };
                        (0..dims).map(move |s| name_of(j, s, set))
                    })
                    .collect()
            };
            let set = free_set(data, &names);
            for (j, v) in avars.iter().enumerate() {
                let dims = if single(j) { 1 } else { count };
                for s in 0..dims {
                    let label = match single(j) {
                        true => format!("Quantified {} ({WHO} {set})", coded[v.coded].name),
                        false => format!(
                            "Quantified {} in dimension {} ({WHO} {set})",
                            coded[v.coded].name,
                            s + 1
                        ),
                    };
                    saves.push((name_of(j, s, set), label, transformed(j, s)));
                }
            }
        }
        if let Some((root, asked)) = &command.save_object {
            let count = multiple_count(*asked, "OBJECT", &mut warnings);
            let names = |set: usize| -> Vec<String> {
                (1..=count).map(|s| format!("{root}{s}_{set}")).collect()
            };
            let set = free_set(data, &names);
            for s in 0..count {
                saves.push((
                    format!("{root}{}_{set}", s + 1),
                    format!("Component score in dimension {} ({WHO} {set})", s + 1),
                    object_column(s),
                ));
            }
        }
        if let Some(root) = &command.save_approx {
            let names = |set: usize| -> Vec<String> {
                (1..=avars.len())
                    .map(|k| format!("{root}{k}_{set}"))
                    .collect()
            };
            let set = free_set(data, &names);
            for (j, v) in avars.iter().enumerate() {
                saves.push((
                    format!("{root}{}_{set}", j + 1),
                    format!("Approximation of {} ({WHO} {set})", coded[v.coded].name),
                    approximation(j),
                ));
            }
        }
        let mut files: Vec<(String, Dataset)> = Vec::new();
        if let Some(file) = &command.outfile_discrdata {
            let mut out = Dataset::new(prep.n_tot);
            for v in avars {
                let c = &coded[v.coded];
                let values = (0..prep.n_tot)
                    .map(|case| c.codes[case].filter(|_| !prep.excluded[case]))
                    .collect();
                out.set_numeric_values(&c.name, values)?;
            }
            files.push((file.clone(), out));
        }
        if let Some(file) = &command.outfile_trdata {
            let mut out = Dataset::new(prep.n_tot);
            for j in 0..avars.len() {
                match single(j) {
                    true => out.set_numeric_values(&format!("TRA{}", j + 1), transformed(j, 0))?,
                    false => {
                        for s in 0..p {
                            out.set_numeric_values(
                                &format!("TRA{}_{}", j + 1, s + 1),
                                transformed(j, s),
                            )?;
                        }
                    }
                }
            }
            files.push((file.clone(), out));
        }
        if let Some(file) = &command.outfile_object {
            let mut out = Dataset::new(prep.n_tot);
            for s in 0..p {
                out.set_numeric_values(&format!("OBSCO{}", s + 1), object_column(s))?;
            }
            files.push((file.clone(), out));
        }
        if let Some(file) = &command.outfile_approx {
            let mut out = Dataset::new(prep.n_tot);
            for j in 0..avars.len() {
                out.set_numeric_values(&format!("APP{}", j + 1), approximation(j))?;
            }
            files.push((file.clone(), out));
        }
        if !saves.is_empty() {
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                for (name, label, values) in saves {
                    data.set_numeric_values(&name, values)?;
                    data.set_variable_label(&name, Some(&label))?;
                    data.set_format(&name, "F8.4")?;
                    data.set_measure(&name, Measure::Scale)?;
                }
                Ok(())
            })?;
        }
        for (file, out) in files {
            self.write_data_file(&file, out)?;
        }
        // PRINCALS MATRIX=OUT: quantifications, coordinates and loadings
        if let Some(target) = command
            .common
            .legacy
            .as_ref()
            .and_then(|l| l.matrix_out.as_ref())
        {
            use crate::legacy_scaling::{MatrixRow, vartype};
            let mut rows = Vec::new();
            for (j, v) in avars.iter().enumerate() {
                let c = &coded[v.coded];
                let kind = quants[j].kind;
                let row = |rowtype, level: String, dims| MatrixRow {
                    rowtype,
                    level,
                    varname: c.name.clone(),
                    vartype: Some(vartype(
                        kind == Kind::Multiple,
                        kind == Kind::Nominal,
                        kind == Kind::Ordinal,
                    )),
                    set: None,
                    dims,
                };
                for (r, &code) in v.cats.iter().enumerate() {
                    let centroid = centroid_of(j, code).unwrap_or_default();
                    let mut quant = vec![0.0; p];
                    quant[0] = quants[j].y[r];
                    match single(j) {
                        true => {
                            rows.push(row("QUANT", c.label(code, None), quant));
                            rows.push(row("SCOOR_", c.label(code, None), vector_of(j, r)));
                        }
                        false => {
                            rows.push(row("QUANT", c.label(code, None), centroid.clone()));
                            rows.push(row("MCOOR_", c.label(code, None), centroid));
                        }
                    }
                }
                if single(j) {
                    rows.push(row("LOADING_", String::new(), normalized_a[j].clone()));
                }
            }
            self.write_matrix(target, &rows, p)?;
        }
        for warning in warnings {
            self.warn(warning);
        }
        self.charts.extend(charts);
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regression_keeps_the_mean_and_respects_signs() {
        let rows = vec![vec![1.0], vec![2.0], vec![3.0], vec![4.0]];
        let y = [4.0, 3.0, 2.0, 1.0];
        let w = [1.0, 2.0, 2.0, 1.0];
        let free = regression(&rows, &y, &w, false);
        assert!((free[0] - 4.0).abs() < 1e-12 && (free[3] - 1.0).abs() < 1e-12);
        // a falling line cannot have a positive slope: the weighted mean remains
        let rising = regression(&rows, &y, &w, true);
        for f in rising {
            assert!((f - 2.5).abs() < 1e-12);
        }
    }

    #[test]
    fn standardized_codes_have_weighted_mean_zero_and_sum_of_squares_n() {
        let d = [2.0, 1.0, 3.0];
        let y = standardized(&[1.0, 2.0, 4.0], &d, 6.0);
        let mean: f64 = y.iter().zip(&d).map(|(a, b)| a * b).sum();
        let ss: f64 = y.iter().zip(&d).map(|(a, b)| b * a * a).sum();
        assert!(mean.abs() < 1e-12 && (ss - 6.0).abs() < 1e-12);
    }
}
