//! MULTIPLE CORRESPONDENCE: homogeneity analysis of categorical variables, with its tables,
//! charts and saved variables.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - Every analysis variable is a set of categories coded by positive whole numbers. Values
//!   below 1, system-missing and user-missing values are missing. A string variable is first
//!   coded by the sorted order of its values. A numeric variable with fractions is grouped into
//!   7 categories (fewer when it has fewer values) shaped like a normal distribution, unless
//!   DISCRETIZATION says otherwise. Discretizing ignores the case weights and sees values below
//!   1 as valid.
//! - The analysis gives every object (case) a point and every category a point so that each
//!   category sits at the weighted mean (centroid) of its objects and the objects lie as close
//!   as possible to their own categories. Each variable counts with its weight. A missing value
//!   that is passive simply leaves that variable out for that object. Objects are centered and
//!   normalized so that, weighted by the case weight times the summed weights of the variables
//!   they have, each dimension has mean 0, the dimensions are uncorrelated and their sum of
//!   squares is n times that summed weight.
//! - The discrimination measure of a variable in a dimension is the weighted variance of its
//!   category centroids (the squared correlation ratio); the eigenvalue of a dimension is the
//!   weighted sum of these over the active variables, its inertia the eigenvalue over the summed
//!   variable weights. Cronbach's alpha of eigenvalue e with m weighted variables is
//!   m(e - 1) / ((m - 1)e).
//! - Supplementary objects get scores from their categories after the fit; supplementary
//!   variables get centroids from the fitted objects. An object without any active value
//!   becomes supplementary.
//! - NORMALIZATION spreads the singular value of each dimension: the objects get the power
//!   (1 + q)/2, the categories (1 - q)/2, both from the variable principal solution in which the
//!   objects are standard and the categories are centroids.

use crate::chart::{Axis, Category, ChartKind, Series, Statistic};
use crate::descriptives::case_weights;
use crate::dist::{normal_cdf, normal_quantile};
use crate::matrix::{Matrix, symmetric_eigen};
use crate::means::is_valid;
use crate::npar_tests::text;
use crate::plot::{CaseGroup, CaseValues, Plot, Point, PointGroup};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Values, Variable};
use statscraft_syntax::{
    Discretize, Impute, McaMissing, McaNormalization, McaPlot, MultipleCorrespondence, VarItem,
};
use std::collections::BTreeMap;

const WHO: &str = "MULTIPLE CORRESPONDENCE";
/// Above this many categories the exact start is skipped and the iterations start from a
/// fixed pseudo-random configuration.
const EXACT_START_LIMIT: usize = 600;

/// Named groups of labelled points, for a chart.
type Groups = [(String, Vec<(String, Vec<f64>)>)];

/// One variable coded into categories.
struct Coded {
    name: String,
    title: String,
    /// Per case: the category code, at least 1, or None when missing.
    codes: Vec<Option<f64>>,
    /// Per code: the label shown and the plain value.
    labels: BTreeMap<i64, (String, String)>,
}

impl Coded {
    fn label(&self, code: i64, length: Option<usize>) -> String {
        let (shown, plain) = self
            .labels
            .get(&code)
            .cloned()
            .unwrap_or_else(|| (code.to_string(), code.to_string()));
        cut(shown, plain, length)
    }
}

/// A label for a chart: the value itself with length 0, else the label cut to `length`.
fn cut(shown: String, plain: String, length: Option<usize>) -> String {
    match length {
        None => shown,
        Some(0) => plain,
        Some(l) => shown.chars().take(l).collect(),
    }
}

fn plain_number(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

/// The thresholds that split a standard normal variable into `k` groups with the least
/// squared error (the optimal quantizer), found by alternating centroids and midpoints.
fn normal_thresholds(k: usize) -> Vec<f64> {
    let pdf = |x: f64| (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let mut t: Vec<f64> = (1..k)
        .map(|r| normal_quantile(r as f64 / k as f64))
        .collect();
    for _ in 0..500 {
        let bound = |r: usize| -> f64 {
            match r {
                0 => f64::NEG_INFINITY,
                r if r == k => f64::INFINITY,
                r => t[r - 1],
            }
        };
        let centers: Vec<f64> = (0..k)
            .map(|r| {
                let (a, b) = (bound(r), bound(r + 1));
                let pa = if a.is_finite() { pdf(a) } else { 0.0 };
                let pb = if b.is_finite() { pdf(b) } else { 0.0 };
                let mass = normal_cdf(b) - normal_cdf(a);
                (pa - pb) / mass.max(1e-300)
            })
            .collect();
        let next: Vec<f64> = (0..k - 1)
            .map(|r| (centers[r] + centers[r + 1]) / 2.0)
            .collect();
        let change = next
            .iter()
            .zip(&t)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        t = next;
        if change < 1e-12 {
            break;
        }
    }
    t
}

/// The sorted distinct values with their counts.
fn distinct(values: &[f64]) -> Vec<(f64, usize)> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mut out: Vec<(f64, usize)> = Vec::new();
    for x in sorted {
        match out.last_mut() {
            Some((v, c)) if *v == x => *c += 1,
            _ => out.push((x, 1)),
        }
    }
    out
}

/// Mean and sample standard deviation.
fn mean_sd(values: &[f64]) -> (f64, f64) {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let ss: f64 = values.iter().map(|x| (x - mean).powi(2)).sum();
    (mean, (ss / (n - 1.0).max(1.0)).sqrt())
}

/// Recodes the valid values as `how` says; the map goes from a value to its code.
fn discretize(values: &[f64], how: Discretize) -> BTreeMap<u64, f64> {
    let levels = distinct(values);
    let key = |x: f64| x.to_bits();
    let mut map = BTreeMap::new();
    if levels.is_empty() {
        return map;
    }
    let rank = |map: &mut BTreeMap<u64, f64>| {
        for (r, (x, _)) in levels.iter().enumerate() {
            map.insert(key(*x), (r + 1) as f64);
        }
    };
    match how {
        Discretize::Ranking => rank(&mut map),
        Discretize::Multiplying => {
            let (mean, sd) = mean_sd(values);
            let z = |x: f64| match sd > 0.0 {
                true => (10.0 * (x - mean) / sd).round(),
                false => 0.0,
            };
            let low = levels
                .iter()
                .map(|(x, _)| z(*x))
                .fold(f64::INFINITY, f64::min);
            for (x, _) in &levels {
                map.insert(key(*x), z(*x) - low + 1.0);
            }
        }
        Discretize::Intervals(width) => {
            let low = levels[0].0;
            for (x, _) in &levels {
                map.insert(key(*x), ((x - low) / width).floor() + 1.0);
            }
        }
        Discretize::Grouping { ncat, normal } => {
            let k = ncat.unwrap_or(7);
            if levels.len() <= k {
                rank(&mut map);
            } else if normal {
                let (mean, sd) = mean_sd(values);
                let t = normal_thresholds(k);
                for (x, _) in &levels {
                    let z = (x - mean) / sd;
                    map.insert(key(*x), (1 + t.iter().filter(|&&b| b < z).count()) as f64);
                }
            } else {
                // groups of whole values, each as close to n/k cases as the values allow
                let target = (values.len() as f64 / k as f64).round();
                let (mut group, mut count) = (1usize, 0.0);
                for (x, c) in &levels {
                    let c = *c as f64;
                    if count > 0.0
                        && group < k
                        && (count + c - target).abs() > (count - target).abs()
                    {
                        group += 1;
                        count = 0.0;
                    }
                    map.insert(key(*x), group as f64);
                    count += c;
                }
            }
        }
    }
    map
}

/// The categories of one variable: its codes as read and as the analysis uses them.
fn code_variable(
    var: &Variable,
    how: Option<Discretize>,
    shown: &crate::shown::Shown,
) -> Result<Coded, Error> {
    let n = match var.values() {
        Values::Numeric(v) => v.iter().count(),
        Values::String(v) => v.iter().count(),
    };
    let mut labels = BTreeMap::new();
    // the value of each case on a numeric scale, strings by their sorted order
    let (raw, string_labels): (Vec<Option<f64>>, Option<Vec<String>>) = match var.values() {
        Values::Numeric(values) => (
            (0..n)
                .map(|c| values.get(c).filter(|_| is_valid(var, c, false)))
                .collect(),
            None,
        ),
        Values::String(values) => {
            let mut texts: Vec<String> = (0..n)
                .filter(|&c| is_valid(var, c, false))
                .map(|c| values.get(c).trim_end().to_string())
                .collect();
            texts.sort();
            texts.dedup();
            let codes = (0..n)
                .map(|c| {
                    is_valid(var, c, false).then(|| {
                        let t = values.get(c).trim_end();
                        (texts.binary_search_by(|x| x.as_str().cmp(t)).unwrap_or(0) + 1) as f64
                    })
                })
                .collect();
            (codes, Some(texts))
        }
    };
    let fractional = raw.iter().flatten().any(|x| x.fract() != 0.0);
    let how = how.or(match (&string_labels, fractional) {
        (Some(_), _) => None,
        (None, true) => Some(Discretize::Grouping {
            ncat: None,
            normal: true,
        }),
        (None, false) => None,
    });
    let label_of = |x: f64| match &string_labels {
        Some(texts) => {
            let t = texts[x as usize - 1].clone();
            (t.clone(), t)
        }
        None => (shown.number(var, x), plain_number(x)),
    };
    let codes: Vec<Option<f64>> = match how {
        None => raw
            .iter()
            .map(|x| x.filter(|x| *x >= 1.0).map(f64::trunc))
            .collect(),
        Some(how) => {
            let valid: Vec<f64> = raw.iter().flatten().copied().collect();
            let map = discretize(&valid, how);
            let keep_labels = matches!(how, Discretize::Ranking)
                || matches!(how, Discretize::Grouping { ncat, .. }
                    if distinct(&valid).len() <= ncat.unwrap_or(7));
            for (bits, code) in &map {
                if keep_labels {
                    labels.insert(*code as i64, label_of(f64::from_bits(*bits)));
                }
            }
            raw.iter()
                .map(|x| x.and_then(|x| map.get(&x.to_bits()).copied()))
                .collect()
        }
    };
    for code in codes.iter().flatten() {
        let c = *code as i64;
        labels.entry(c).or_insert_with(|| match how {
            None => label_of(*code),
            Some(_) => (plain_number(*code), plain_number(*code)),
        });
    }
    Ok(Coded {
        name: var.name().to_string(),
        title: shown.variable(var),
        codes,
        labels,
    })
}

/// Names of `items`: TO runs over `order`, single names must be among `allowed`.
fn resolve(items: &[VarItem], order: &[String], allowed: &[String]) -> Result<Vec<String>, Error> {
    let find = |list: &[String], name: &str| list.iter().position(|v| v.eq_ignore_ascii_case(name));
    let mut out = Vec::new();
    for item in items {
        match item {
            VarItem::All => out.extend(order.iter().cloned()),
            VarItem::Name(name) => match find(allowed, name) {
                Some(i) => out.push(allowed[i].clone()),
                None => {
                    return Err(Error(format!(
                        "{WHO}: {name} is not among the variables this list may name"
                    )));
                }
            },
            VarItem::Range(a, b) => {
                let (Some(i), Some(j)) = (find(order, a), find(order, b)) else {
                    return Err(Error(format!(
                        "{WHO}: {a} TO {b} must run between variables of the list it refers to"
                    )));
                };
                if i > j {
                    return Err(Error(format!(
                        "{WHO}: in {a} TO {b} the first variable must come first"
                    )));
                }
                out.extend(order[i..=j].iter().cloned());
            }
        }
    }
    Ok(out)
}

fn coded_index(coded: &[Coded], name: &str) -> usize {
    coded
        .iter()
        .position(|c| c.name.eq_ignore_ascii_case(name))
        .expect("named on VARIABLES")
}

/// One variable of ANALYSIS as the fit sees it.
struct AVar {
    /// Index into the coded variables.
    coded: usize,
    weight: f64,
    missing: McaMissing,
    supplementary: bool,
    /// Per case: the code after imputation.
    codes: Vec<Option<f64>>,
    /// The codes the active objects have, sorted.
    cats: Vec<i64>,
    /// Per active object: its category index.
    h: Vec<Option<usize>>,
    /// Weighted frequency of each category among the active objects.
    d: Vec<f64>,
}

/// The fitted solution, in variable principal form.
struct Fit {
    /// Per active object: the standard scores.
    x: Matrix,
    /// The start, then every iteration: (variance accounted for, loss).
    history: Vec<(f64, f64)>,
    converged: bool,
}

/// Weighted category centroids of the scores.
fn centroids(v: &AVar, x: &Matrix, w: &[f64], p: usize) -> Matrix {
    let mut y = vec![vec![0.0; p]; v.cats.len()];
    for (i, h) in v.h.iter().enumerate() {
        if let Some(r) = h {
            for s in 0..p {
                y[*r][s] += w[i] * x[i][s];
            }
        }
    }
    for (r, row) in y.iter_mut().enumerate() {
        if v.d[r] > 0.0 {
            row.iter_mut().for_each(|e| *e /= v.d[r]);
        }
    }
    y
}

/// The p x p symmetric matrix `a`^-1/2 or `a`^1/2, with its eigenvectors and values.
fn eigen_sorted(a: &Matrix) -> (Vec<f64>, Matrix) {
    let pairs = symmetric_eigen(a);
    let values = pairs.iter().map(|(v, _)| *v).collect();
    let p = a.len();
    let vectors = (0..p)
        .map(|i| pairs.iter().map(|(_, v)| v[i]).collect())
        .collect();
    (values, vectors)
}

/// Centers `v` and makes it orthonormal in the metric `mw` with sum of squares `scale`.
fn orthonormalize(v: &mut Matrix, mw: &[f64], scale: f64) -> Option<()> {
    let p = v.first()?.len();
    let total: f64 = mw.iter().sum();
    for s in 0..p {
        let mean = v.iter().zip(mw).map(|(r, m)| m * r[s]).sum::<f64>() / total;
        v.iter_mut().for_each(|r| r[s] -= mean);
    }
    let cross: Matrix = (0..p)
        .map(|a| {
            (0..p)
                .map(|b| v.iter().zip(mw).map(|(r, m)| m * r[a] * r[b]).sum())
                .collect()
        })
        .collect();
    let (values, l) = eigen_sorted(&cross);
    if values
        .iter()
        .any(|&e| e <= 1e-12 * values[0].abs().max(1e-300))
    {
        return None;
    }
    // v L diag(1/sqrt(value)) L', the orthonormal matrix nearest to v
    let t: Matrix = (0..p)
        .map(|a| {
            (0..p)
                .map(|b| {
                    (0..p)
                        .map(|k| l[a][k] * l[b][k] / values[k].sqrt())
                        .sum::<f64>()
                        * scale.sqrt()
                })
                .collect()
        })
        .collect();
    for row in v.iter_mut() {
        let new: Vec<f64> = (0..p)
            .map(|b| (0..p).map(|a| row[a] * t[a][b]).sum())
            .collect();
        *row = new;
    }
    Some(())
}

/// The exact solution from the eigenvectors of the category cross products, the trivial one
/// taken out.
fn exact_start(vars: &[&AVar], w: &[f64], mstar: &[f64], p: usize, nm: f64) -> Option<Matrix> {
    let mut offset = Vec::new();
    let mut k = 0;
    for v in vars {
        offset.push(k);
        k += v.cats.len();
    }
    let mut scale = vec![0.0; k];
    let mut trivial = vec![0.0; k];
    for (j, v) in vars.iter().enumerate() {
        for r in 0..v.cats.len() {
            scale[offset[j] + r] = (v.weight / v.d[r]).sqrt();
            trivial[offset[j] + r] = (v.weight * v.d[r]).sqrt();
        }
    }
    let n = w.len();
    let mut b = vec![vec![0.0; k]; k];
    let mut cats = Vec::with_capacity(vars.len());
    for i in 0..n {
        cats.clear();
        cats.extend(
            vars.iter()
                .enumerate()
                .filter_map(|(j, v)| v.h[i].map(|r| offset[j] + r)),
        );
        let f = w[i] / mstar[i];
        for &a in &cats {
            for &c in &cats {
                b[a][c] += f * scale[a] * scale[c];
            }
        }
    }
    let norm: f64 = trivial.iter().map(|t| t * t).sum();
    for a in 0..k {
        for c in 0..k {
            b[a][c] -= trivial[a] * trivial[c] / norm;
        }
    }
    let pairs = symmetric_eigen(&b);
    let mut x = vec![vec![0.0; p]; n];
    for (s, (value, vector)) in pairs.iter().take(p).enumerate() {
        if *value <= 1e-10 {
            return None;
        }
        for i in 0..n {
            let sum: f64 = vars
                .iter()
                .enumerate()
                .filter_map(|(j, v)| v.h[i].map(|r| offset[j] + r))
                .map(|a| scale[a] * vector[a])
                .sum();
            x[i][s] = (nm / value).sqrt() * sum / mstar[i];
        }
    }
    Some(x)
}

/// Alternating least squares from `x`: centroids, objects as the weighted mean of their
/// categories, then centering and orthonormalization, until the fit gains less than
/// `criterion`. Ends with the dimensions turned to principal axes.
fn iterate(
    vars: &[&AVar],
    w: &[f64],
    mstar: &[f64],
    mut x: Matrix,
    max_iterations: usize,
    criterion: f64,
) -> Fit {
    let p = x[0].len();
    let n_w: f64 = w.iter().sum();
    let m_w: f64 = vars.iter().map(|v| v.weight).sum();
    let mw: Vec<f64> = w.iter().zip(mstar).map(|(a, b)| a * b).collect();
    let fit_of = |x: &Matrix| -> f64 {
        vars.iter()
            .map(|v| {
                let y = centroids(v, x, w, p);
                v.weight
                    * y.iter()
                        .zip(&v.d)
                        .map(|(r, d)| d * r.iter().map(|e| e * e).sum::<f64>())
                        .sum::<f64>()
            })
            .sum::<f64>()
            / (n_w * p as f64)
    };
    let mut fit = fit_of(&x);
    let mut history = vec![(fit, m_w - fit)];
    let mut converged = false;
    for _ in 0..max_iterations {
        let mut z = vec![vec![0.0; p]; w.len()];
        for v in vars {
            let y = centroids(v, &x, w, p);
            for (i, h) in v.h.iter().enumerate() {
                if let Some(r) = h {
                    for s in 0..p {
                        z[i][s] += v.weight * y[*r][s] / mstar[i];
                    }
                }
            }
        }
        if orthonormalize(&mut z, &mw, n_w * m_w).is_none() {
            break;
        }
        x = z;
        let next = fit_of(&x);
        history.push((next, m_w - next));
        let gain = next - fit;
        fit = next;
        if gain < criterion {
            converged = true;
            break;
        }
    }
    // principal axes: the eigenvectors of the summed weighted category cross products
    let mut t = vec![vec![0.0; p]; p];
    for v in vars {
        let y = centroids(v, &x, w, p);
        for (r, row) in y.iter().enumerate() {
            for a in 0..p {
                for b in 0..p {
                    t[a][b] += v.weight * v.d[r] * row[a] * row[b];
                }
            }
        }
    }
    let (_, l) = eigen_sorted(&t);
    for row in x.iter_mut() {
        let new: Vec<f64> = (0..p)
            .map(|b| (0..p).map(|a| row[a] * l[a][b]).sum())
            .collect();
        *row = new;
    }
    // the object with the largest absolute score is positive in every dimension
    for s in 0..p {
        let big = x.iter().map(|r| r[s].abs()).fold(0.0, f64::max);
        if let Some(r) = x.iter().find(|r| r[s].abs() >= big * (1.0 - 1e-9))
            && r[s] < 0.0
        {
            x.iter_mut().for_each(|r| r[s] = -r[s]);
        }
    }
    Fit {
        x,
        history,
        converged,
    }
}

/// Weighted Pearson correlations of the columns; NaN where a column is constant.
fn correlations(columns: &[Vec<f64>], w: &[f64]) -> Matrix {
    let total: f64 = w.iter().sum();
    let centered: Vec<Vec<f64>> = columns
        .iter()
        .map(|c| {
            let mean = c.iter().zip(w).map(|(x, w)| x * w).sum::<f64>() / total;
            c.iter().map(|x| x - mean).collect()
        })
        .collect();
    let dot = |a: &[f64], b: &[f64]| {
        a.iter()
            .zip(b)
            .zip(w)
            .map(|((a, b), w)| a * b * w)
            .sum::<f64>()
    };
    centered
        .iter()
        .map(|a| {
            centered
                .iter()
                .map(|b| {
                    let d = (dot(a, a) * dot(b, b)).sqrt();
                    if d > 1e-12 { dot(a, b) / d } else { f64::NAN }
                })
                .collect()
        })
        .collect()
}

fn num(x: f64) -> Cell {
    if x.is_finite() {
        Cell::Number(x)
    } else {
        Cell::Empty
    }
}

/// A correlation table with the eigenvalues of the active part weighted by the variable
/// weights in its last row.
fn correlation_table(
    title: String,
    names: &[String],
    r: &Matrix,
    active: &[(usize, f64)],
) -> Table {
    let mut columns = vec![String::new()];
    columns.extend(names.iter().cloned());
    let mut rows: Vec<Vec<Cell>> = names
        .iter()
        .zip(r)
        .map(|(name, row)| {
            let mut out = vec![text(name.clone())];
            out.extend(row.iter().map(|x| num(*x)));
            out
        })
        .collect();
    let weighted: Matrix = active
        .iter()
        .map(|&(a, va)| {
            active
                .iter()
                .map(|&(b, vb)| {
                    let x = r[a][b];
                    (va * vb).sqrt() * if x.is_finite() { x } else { 0.0 }
                })
                .collect()
        })
        .collect();
    let mut eigen = vec![text("Eigenvalue")];
    if !weighted.is_empty() {
        eigen.extend(symmetric_eigen(&weighted).iter().map(|(v, _)| num(*v)));
    }
    eigen.resize(names.len() + 1, Cell::Empty);
    rows.push(eigen);
    Table {
        title,
        columns,
        rows,
        row_headers: 1,
        caption: Some(
            "The last row holds the eigenvalues of the matrix of the active variables, each \
             correlation weighted by the root of the two variable weights, from the largest."
                .into(),
        ),
        ..Table::default()
    }
}

/// A scatter of named point groups: one chart for two dimensions, a scatterplot matrix for
/// more, one chart per pair with NDIM.
fn scatter(title: &str, groups: &Groups, p: usize, ndim: Option<(usize, usize)>) -> Vec<Chart> {
    let axis = |d: usize| Axis::titled(format!("Dimension {d}"));
    let pairs: Vec<(usize, usize)> = match (ndim, p) {
        (Some((a, b)), _) => (a + 1..=b.min(p))
            .map(|t| (a, t))
            .filter(|_| a < p)
            .collect(),
        (None, 1) => vec![(1, 0)],
        (None, 2) => vec![(1, 2)],
        (None, _) => Vec::new(),
    };
    if pairs.is_empty() && ndim.is_none() {
        return vec![Chart::plot(
            ChartKind::Scatter,
            title.to_string(),
            Axis::titled(String::new()),
            Axis::titled(String::new()),
            Plot::Matrix {
                variables: (1..=p).map(axis).collect(),
                groups: groups
                    .iter()
                    .map(|(name, points)| CaseGroup {
                        name: name.clone(),
                        cases: points
                            .iter()
                            .map(|(label, values)| CaseValues {
                                values: values.clone(),
                                label: Some(label.clone()),
                            })
                            .collect(),
                    })
                    .collect(),
            },
        )];
    }
    pairs
        .into_iter()
        .map(|(a, b)| {
            let groups = groups
                .iter()
                .map(|(name, points)| PointGroup {
                    name: name.clone(),
                    points: points
                        .iter()
                        .map(|(label, values)| Point {
                            x: values[a - 1],
                            y: if b == 0 { 0.0 } else { values[b - 1] },
                            z: None,
                            label: Some(label.clone()),
                        })
                        .collect(),
                })
                .collect();
            let named = match ndim {
                Some(_) => format!("{title}, Dimension {a} by {b}"),
                None => title.to_string(),
            };
            Chart::plot(
                ChartKind::Scatter,
                named,
                axis(a),
                if b == 0 {
                    Axis::titled(String::new())
                } else {
                    axis(b)
                },
                Plot::Points {
                    groups,
                    lines: Vec::new(),
                    z_axis: None,
                },
            )
        })
        .collect()
}

/// The first free set number n for names made by `name(n)`.
fn free_set(data: &Dataset, names: &dyn Fn(usize) -> Vec<String>) -> usize {
    (1..)
        .find(|&n| {
            names(n)
                .iter()
                .all(|name| data.variable_index(name).is_none())
        })
        .expect("some set is free")
}

impl Engine {
    pub(crate) fn multiple_correspondence(
        &mut self,
        command: &MultipleCorrespondence,
    ) -> Result<Vec<Table>, Error> {
        let shown = self.shown();
        let data = self.active()?;
        let n_tot = data.case_count();
        let mut warnings: Vec<String> = Vec::new();
        let fixed = matches!(command.configuration, Some((true, _)));

        // the lists
        let variables = crate::expand_in(data, &command.variables)?;
        let mut analysis: Vec<(String, f64)> = Vec::new();
        for (items, weight) in &command.analysis {
            for name in resolve(items, &variables, &variables)? {
                if analysis.iter().any(|(a, _)| a.eq_ignore_ascii_case(&name)) {
                    return Err(Error(format!("{WHO}: ANALYSIS names {name} twice")));
                }
                analysis.push((name, *weight));
            }
        }
        if !fixed && (variables.len() < 2 || analysis.len() < 2) {
            return Err(Error(format!(
                "{WHO} needs at least two variables on VARIABLES and on ANALYSIS"
            )));
        }
        let analysis_names: Vec<String> = analysis.iter().map(|(a, _)| a.clone()).collect();
        let mut discretization: BTreeMap<String, Discretize> = BTreeMap::new();
        for (items, how) in &command.discretization {
            for name in resolve(items, &variables, &variables)? {
                discretization.insert(name.to_uppercase(), *how);
            }
        }
        let mut missing: BTreeMap<String, McaMissing> = BTreeMap::new();
        for (items, how) in &command.missing {
            for name in resolve(items, &analysis_names, &analysis_names)? {
                missing.insert(name.to_uppercase(), *how);
            }
        }
        let mut supplementary: Vec<String> = Vec::new();
        if fixed {
            if !command.supplementary_objects.is_empty() {
                return Err(Error(format!(
                    "{WHO}: SUPPLEMENTARY OBJECT cannot go with CONFIGURATION=FIXED"
                )));
            }
            if !command.supplementary_variables.is_empty() {
                warnings.push(format!(
                    "{WHO}: with a FIXED configuration every variable is supplementary; SUPPLEMENTARY VARIABLE is ignored"
                ));
            }
            supplementary = analysis_names.clone();
        } else {
            for name in resolve(
                &command.supplementary_variables,
                &variables,
                &analysis_names,
            )? {
                supplementary.push(name);
            }
        }
        let is_sup = |name: &str| supplementary.iter().any(|s| s.eq_ignore_ascii_case(name));
        for (name, weight) in &analysis {
            if is_sup(name) && *weight != 1.0 && !fixed {
                warnings.push(format!(
                    "{WHO}: the weight of supplementary variable {name} is ignored"
                ));
            }
        }

        // the codes of every variable on VARIABLES
        let mut coded: Vec<Coded> = Vec::new();
        for name in &variables {
            let var = data.variable(name)?;
            coded.push(code_variable(
                &var,
                discretization.get(&name.to_uppercase()).copied(),
                &shown,
            )?);
        }

        // cases: weights, supplementary objects, imputation, listwise deletion
        let weights = case_weights(data)?.unwrap_or_else(|| vec![1.0; n_tot]);
        let mut sup_object = vec![false; n_tot];
        for &(a, b) in &command.supplementary_objects {
            if b > n_tot {
                return Err(Error(format!(
                    "{WHO}: SUPPLEMENTARY OBJECT names case {b}, but the data have {n_tot} cases"
                )));
            }
            sup_object[a - 1..b].iter_mut().for_each(|s| *s = true);
        }
        let mut excluded: Vec<bool> = weights.iter().map(|w| *w <= 0.0).collect();
        let mut avars: Vec<AVar> = Vec::new();
        for (name, weight) in &analysis {
            let c = coded_index(&coded, name);
            let sup = is_sup(name);
            let how = missing
                .get(&name.to_uppercase())
                .copied()
                .unwrap_or(McaMissing::Passive(Impute::Mode));
            if sup && how == McaMissing::Listwise {
                return Err(Error(format!(
                    "{WHO}: supplementary variable {name} cannot have LISTWISE missing values"
                )));
            }
            let mut codes = coded[c].codes.clone();
            if let McaMissing::Active(impute) = how {
                let mut freq: BTreeMap<i64, f64> = BTreeMap::new();
                for case in 0..n_tot {
                    if let Some(code) = codes[case]
                        && !sup_object[case]
                        && !excluded[case]
                    {
                        *freq.entry(code as i64).or_default() += weights[case];
                    }
                }
                let mode = freq
                    .iter()
                    .fold(None, |best: Option<(i64, f64)>, (&k, &f)| match best {
                        Some((_, bf)) if bf >= f => best,
                        _ => Some((k, f)),
                    })
                    .map(|(k, _)| k as f64);
                // above every code any case has, supplementary ones included
                let extra = codes.iter().flatten().fold(0.0, |a: f64, b| a.max(*b)) + 1.0;
                let total: f64 = freq.values().sum();
                for code in codes.iter_mut() {
                    if code.is_none() {
                        *code = match impute {
                            Impute::Mode => mode,
                            Impute::ExtraCategory => Some(extra),
                            Impute::Random => {
                                let u = self.random.borrow_mut().uniform() * total;
                                let mut cum = 0.0;
                                freq.iter()
                                    .find(|(_, f)| {
                                        cum += **f;
                                        cum >= u
                                    })
                                    .or(freq.iter().last())
                                    .map(|(k, _)| *k as f64)
                            }
                        };
                    }
                }
                if impute == Impute::ExtraCategory {
                    coded[c]
                        .labels
                        .insert(extra as i64, ("Missing".into(), plain_number(extra)));
                }
            }
            if how == McaMissing::Listwise {
                for case in 0..n_tot {
                    excluded[case] |= codes[case].is_none();
                }
            }
            avars.push(AVar {
                coded: c,
                weight: if sup { 1.0 } else { *weight },
                missing: how,
                supplementary: sup,
                codes,
                cats: Vec::new(),
                h: Vec::new(),
                d: Vec::new(),
            });
        }
        let active_vars: Vec<usize> = (0..avars.len())
            .filter(|&j| !avars[j].supplementary)
            .collect();
        // an object without any active value is supplementary
        let mut active: Vec<usize> = (0..n_tot)
            .filter(|&c| !excluded[c] && !sup_object[c])
            .collect();
        let without_data: Vec<usize> = active
            .iter()
            .copied()
            .filter(|&c| !fixed && active_vars.iter().all(|&j| avars[j].codes[c].is_none()))
            .collect();
        active.retain(|c| !without_data.contains(c));
        let n = active.len();
        if n < 3 {
            return Err(Error(format!(
                "{WHO} needs at least three active cases, but has {n}"
            )));
        }
        let w: Vec<f64> = active.iter().map(|&c| weights[c]).collect();
        let n_w: f64 = w.iter().sum();
        for v in avars.iter_mut() {
            let mut cats: Vec<i64> = active
                .iter()
                .filter_map(|&c| v.codes[c].map(|x| x as i64))
                .collect();
            cats.sort_unstable();
            cats.dedup();
            v.h = active
                .iter()
                .map(|&c| v.codes[c].and_then(|x| cats.binary_search(&(x as i64)).ok()))
                .collect();
            v.d = vec![0.0; cats.len()];
            for (i, h) in v.h.iter().enumerate() {
                if let Some(r) = h {
                    v.d[*r] += w[i];
                }
            }
            v.cats = cats;
        }
        let mstar: Vec<f64> = (0..n)
            .map(|i| {
                active_vars
                    .iter()
                    .filter(|&&j| avars[j].h[i].is_some())
                    .map(|&j| avars[j].weight)
                    .sum()
            })
            .collect();
        let m_w: f64 = active_vars.iter().map(|&j| avars[j].weight).sum();
        let passive_missing = active_vars
            .iter()
            .any(|&j| avars[j].h.iter().any(Option::is_none));

        // dimensions
        let mut p = command.dimension;
        if !fixed {
            let k: usize = active_vars.iter().map(|&j| avars[j].cats.len()).sum();
            let m1 = active_vars
                .iter()
                .filter(|&&j| avars[j].h.iter().all(Option::is_some))
                .count();
            let less = match passive_missing {
                true => m1.max(1),
                false => active_vars.len(),
            };
            let pmax = (n - 1).min(k.saturating_sub(less));
            if pmax < 1 {
                return Err(Error(format!(
                    "{WHO}: the data leave no dimension to compute (every variable has one category)"
                )));
            }
            if p > pmax {
                warnings.push(format!(
                    "{WHO}: DIMENSION is {p}, but these data allow at most {pmax}; {pmax} are computed"
                ));
                p = pmax;
            }
        }

        // the configuration from a file
        let configuration: Option<Matrix> = match &command.configuration {
            None => None,
            Some((_, file)) => {
                let config = self.data_file(file)?;
                if config.case_count() != n_tot {
                    return Err(Error(format!(
                        "{WHO}: the configuration in {file} has {} cases, but the data have {n_tot}",
                        config.case_count()
                    )));
                }
                let columns: Vec<Vec<Option<f64>>> = config
                    .variables()
                    .filter_map(|v| v.numeric().ok().map(|x| x.iter().collect()))
                    .take(p)
                    .collect();
                if columns.len() < p {
                    return Err(Error(format!(
                        "{WHO}: the configuration in {file} has {} numeric variables for {p} dimensions",
                        columns.len()
                    )));
                }
                let rows: Option<Matrix> = active
                    .iter()
                    .map(|&c| {
                        columns
                            .iter()
                            .map(|col| col[c])
                            .collect::<Option<Vec<f64>>>()
                    })
                    .collect();
                Some(rows.ok_or_else(|| {
                    Error(format!(
                        "{WHO}: the configuration in {file} has a missing coordinate for an active case"
                    ))
                })?)
            }
        };

        // the fit
        let act: Vec<&AVar> = active_vars.iter().map(|&j| &avars[j]).collect();
        let k_total: usize = act.iter().map(|v| v.cats.len()).sum();
        let fit = if fixed {
            let mut x = configuration.expect("FIXED has a file");
            if orthonormalize(&mut x, &w, n_w).is_none() {
                return Err(Error(format!(
                    "{WHO}: the fixed configuration has a dimension without variance or two that coincide"
                )));
            }
            Fit {
                x,
                history: Vec::new(),
                converged: true,
            }
        } else {
            let start = match configuration {
                Some(mut x) => {
                    let mw: Vec<f64> = w.iter().zip(&mstar).map(|(a, b)| a * b).collect();
                    if orthonormalize(&mut x, &mw, n_w * m_w).is_none() {
                        return Err(Error(format!(
                            "{WHO}: the initial configuration has a dimension without variance or two that coincide"
                        )));
                    }
                    x
                }
                None => {
                    let exact = (k_total <= EXACT_START_LIMIT)
                        .then(|| exact_start(&act, &w, &mstar, p, n_w * m_w))
                        .flatten();
                    match exact {
                        Some(x) => x,
                        None if k_total <= EXACT_START_LIMIT => {
                            return Err(Error(format!(
                                "{WHO}: the data support fewer than {p} dimensions; lower DIMENSION"
                            )));
                        }
                        None => {
                            // a fixed pseudo-random start, so that runs repeat
                            let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
                            let mut x: Matrix = (0..n)
                                .map(|_| {
                                    (0..p)
                                        .map(|_| {
                                            state = state
                                                .wrapping_mul(6_364_136_223_846_793_005)
                                                .wrapping_add(1_442_695_040_888_963_407);
                                            (state >> 11) as f64 / (1u64 << 53) as f64 - 0.5
                                        })
                                        .collect()
                                })
                                .collect();
                            let mw: Vec<f64> = w.iter().zip(&mstar).map(|(a, b)| a * b).collect();
                            if orthonormalize(&mut x, &mw, n_w * m_w).is_none() {
                                return Err(Error(format!(
                                    "{WHO}: the data support fewer than {p} dimensions; lower DIMENSION"
                                )));
                            }
                            x
                        }
                    }
                }
            };
            iterate(
                &act,
                &w,
                &mstar,
                start,
                command.max_iterations,
                command.criterion,
            )
        };
        if !fit.converged {
            warnings.push(format!(
                "{WHO}: the fit did not converge within {} iterations",
                command.max_iterations
            ));
        }
        let x = fit.x;

        // centroids, discrimination measures, eigenvalues
        let ys: Vec<Matrix> = avars.iter().map(|v| centroids(v, &x, &w, p)).collect();
        let discr: Vec<Vec<f64>> = avars
            .iter()
            .zip(&ys)
            .map(|(v, y)| {
                (0..p)
                    .map(|s| {
                        y.iter()
                            .zip(&v.d)
                            .map(|(r, d)| d * r[s] * r[s])
                            .sum::<f64>()
                            / n_w
                    })
                    .collect()
            })
            .collect();
        let eigen: Vec<f64> = (0..p)
            .map(|s| {
                active_vars
                    .iter()
                    .map(|&j| avars[j].weight * discr[j][s])
                    .sum()
            })
            .collect();
        let inertia: Vec<f64> = eigen.iter().map(|e| e / m_w).collect();
        // the normalization powers of the singular value sqrt(inertia)
        let (a_pow, b_pow) = match (fixed, command.normalization) {
            (true, _) => (0.0, 1.0),
            (false, McaNormalization::Independent) => (1.0, 1.0),
            (false, McaNormalization::Q(q)) => ((1.0 + q) / 2.0, (1.0 - q) / 2.0),
        };
        if fixed && command.normalization != McaNormalization::Q(-1.0) {
            warnings.push(format!(
                "{WHO}: with a FIXED configuration the scores keep their own scale; NORMALIZATION is ignored"
            ));
        }
        let sigma: Vec<f64> = inertia.iter().map(|l| l.max(0.0).sqrt()).collect();
        let x_factor: Vec<f64> = sigma.iter().map(|s| s.powf(a_pow)).collect();
        let y_factor: Vec<f64> = sigma
            .iter()
            .map(|s| {
                if b_pow == 1.0 {
                    1.0
                } else {
                    s.powf(b_pow - 1.0)
                }
            })
            .collect();

        // scores of every case: active from the fit, others from their categories
        let mut scores: Vec<Option<Vec<f64>>> = vec![None; n_tot];
        for (i, &c) in active.iter().enumerate() {
            scores[c] = Some(x[i].clone());
        }
        for c in 0..n_tot {
            if excluded[c] || scores[c].is_some() {
                continue;
            }
            let (mut z, mut m) = (vec![0.0; p], 0.0);
            let fitting: Vec<usize> = match fixed {
                true => Vec::new(),
                false => active_vars.clone(),
            };
            for &j in &fitting {
                let v = &avars[j];
                let Some(r) = v.codes[c].and_then(|x| v.cats.binary_search(&(x as i64)).ok())
                else {
                    continue;
                };
                m += v.weight;
                for s in 0..p {
                    z[s] += v.weight * ys[j][r][s];
                }
            }
            if m > 0.0 {
                scores[c] = Some(
                    (0..p)
                        .map(|s| match inertia[s] > 0.0 {
                            true => z[s] / (m * inertia[s]),
                            false => 0.0,
                        })
                        .collect(),
                );
            }
        }
        // categories that only supplementary objects have: the mean of their scores
        let mut extra_cats: Vec<BTreeMap<i64, (f64, Vec<f64>)>> =
            vec![BTreeMap::new(); avars.len()];
        for (j, v) in avars.iter().enumerate() {
            for (c, score) in scores.iter().enumerate() {
                let (Some(code), Some(score)) = (v.codes[c], score) else {
                    continue;
                };
                if active.binary_search(&c).is_ok() || v.cats.binary_search(&(code as i64)).is_ok()
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
        // the quantification of each code, normalized
        let quantification = |j: usize, code: f64| -> Option<Vec<f64>> {
            let v = &avars[j];
            let y = match v.cats.binary_search(&(code as i64)) {
                Ok(r) => ys[j][r].clone(),
                Err(_) => extra_cats[j].get(&(code as i64))?.1.clone(),
            };
            Some((0..p).map(|s| y[s] * y_factor[s]).collect())
        };
        let normalized = |score: &Vec<f64>| -> Vec<f64> {
            score.iter().zip(&x_factor).map(|(a, b)| a * b).collect()
        };

        // tables
        let print = &command.print;
        let mut tables = Vec::new();
        let analysis_titles: Vec<String> =
            avars.iter().map(|v| coded[v.coded].title.clone()).collect();
        let dims: Vec<String> = (1..=p).map(|s| format!("Dimension {s}")).collect();
        {
            let passive_cases = (0..n)
                .filter(|&i| active_vars.iter().any(|&j| avars[j].h[i].is_none()))
                .count();
            let sup_count = (0..n_tot)
                .filter(|&c| !excluded[c] && (sup_object[c] || without_data.contains(&c)))
                .count();
            let excluded_count = excluded.iter().filter(|e| **e).count();
            let row = |label: &str, v: usize| vec![text(label), Cell::Number(v as f64)];
            tables.push(Table {
                title: "Case Processing Summary".into(),
                columns: vec![String::new(), "N".into()],
                rows: vec![
                    row("Valid Active Cases", n - passive_cases),
                    row("Active Cases with Missing Values", passive_cases),
                    row("Supplementary Cases", sup_count),
                    row("Excluded Cases", excluded_count),
                    row("Total", n_tot),
                    row("Cases Used in Analysis", n),
                ],
                row_headers: 1,
                ..Table::default()
            });
        }
        if let Some(items) = &print.descrip {
            let names = match items.is_empty() {
                true => analysis_names.clone(),
                false => resolve(items, &analysis_names, &variables)?,
            };
            for name in names {
                let c = &coded[coded_index(&coded, &name)];
                let mut freq: BTreeMap<i64, f64> = BTreeMap::new();
                let mut lacking = 0.0;
                for (i, &case) in active.iter().enumerate() {
                    match c.codes[case] {
                        Some(code) => *freq.entry(code as i64).or_default() += w[i],
                        None => lacking += w[i],
                    }
                }
                let mode = freq
                    .iter()
                    .fold(None, |best: Option<(i64, f64)>, (&k, &f)| match best {
                        Some((_, bf)) if bf >= f => best,
                        _ => Some((k, f)),
                    })
                    .map(|(k, _)| k);
                let mut rows: Vec<Vec<Cell>> = freq
                    .iter()
                    .map(|(k, f)| vec![text(c.label(*k, None)), Cell::Number(*f)])
                    .collect();
                rows.push(vec![text("Missing"), Cell::Number(lacking)]);
                rows.push(vec![text("Total"), Cell::Number(n_w)]);
                tables.push(Table {
                    title: c.title.clone(),
                    columns: vec![String::new(), "Frequency".into()],
                    rows,
                    row_headers: 1,
                    caption: mode.map(|m| format!("Mode: {}", c.label(m, None))),
                    ..Table::default()
                });
            }
        }
        if !fixed {
            let shown_history: Vec<(usize, &(f64, f64))> = match print.history {
                true => fit.history.iter().enumerate().collect(),
                false => fit
                    .history
                    .iter()
                    .enumerate()
                    .next_back()
                    .into_iter()
                    .collect(),
            };
            let rows = shown_history
                .into_iter()
                .map(|(k, (fit_k, loss))| {
                    let before = match k {
                        0 => Cell::Empty,
                        k => Cell::Number(fit_k - fit.history[k - 1].0),
                    };
                    vec![
                        Cell::Number(k as f64),
                        Cell::Number(*fit_k),
                        before,
                        Cell::Number(*loss),
                    ]
                })
                .collect();
            tables.push(Table {
                title: "Iteration History".into(),
                columns: vec![
                    "Iteration Number".into(),
                    "Total".into(),
                    "Increase".into(),
                    "Loss".into(),
                ],
                column_groups: vec![
                    Vec::new(),
                    vec!["Variance Accounted For".into()],
                    vec!["Variance Accounted For".into()],
                    Vec::new(),
                ],
                rows,
                row_headers: 1,
                caption: Some(match fit.converged {
                    true => "The fit stopped gaining more than the convergence criterion.".into(),
                    false => "The most iterations were reached.".into(),
                }),
                ..Table::default()
            });
            let alpha = |e: f64| match m_w > 1.0 && e != 0.0 {
                true => num(m_w * (e - 1.0) / ((m_w - 1.0) * e)),
                false => Cell::Empty,
            };
            let total_e: f64 = eigen.iter().sum();
            let mut rows: Vec<Vec<Cell>> = (0..p)
                .map(|s| {
                    vec![
                        text((s + 1).to_string()),
                        alpha(eigen[s]),
                        Cell::Number(eigen[s]),
                        Cell::Number(inertia[s]),
                        Cell::Number(100.0 * inertia[s]),
                    ]
                })
                .collect();
            rows.push(vec![
                text("Total"),
                alpha(total_e),
                Cell::Number(total_e),
                Cell::Number(total_e / m_w),
                Cell::Number(100.0 * total_e / m_w),
            ]);
            let mean = total_e / p as f64;
            rows.push(vec![
                text("Mean"),
                alpha(mean),
                Cell::Number(mean),
                Cell::Number(mean / m_w),
                Cell::Number(100.0 * mean / m_w),
            ]);
            let group = vec!["Variance Accounted For".to_string()];
            tables.push(Table {
                title: "Model Summary".into(),
                columns: vec![
                    "Dimension".into(),
                    "Cronbach's Alpha".into(),
                    "Total (Eigenvalue)".into(),
                    "Inertia".into(),
                    "% of Variance".into(),
                ],
                column_groups: vec![Vec::new(), Vec::new(), group.clone(), group.clone(), group],
                rows,
                row_headers: 1,
                caption: Some(
                    "The Total row's alpha comes from the summed eigenvalue, the Mean row's from the mean."
                        .into(),
                ),
                ..Table::default()
            });
        }
        let active_positions: Vec<(usize, f64)> =
            active_vars.iter().map(|&j| (j, avars[j].weight)).collect();
        // the code of each active object on each analysis variable, passive gaps filled
        let filled = |j: usize, i: usize, how: Impute, quantified: Option<usize>| -> f64 {
            let v = &avars[j];
            let pick = |r: usize| match quantified {
                Some(s) => ys[j][r][s],
                None => v.cats[r] as f64,
            };
            match v.h[i] {
                Some(r) => pick(r),
                None => {
                    let mode = (0..v.cats.len()).fold(None, |best: Option<usize>, r| match best {
                        Some(b) if v.d[b] >= v.d[r] => best,
                        _ => Some(r),
                    });
                    match (how, quantified) {
                        (Impute::ExtraCategory, Some(s)) => {
                            let (mut sw, mut sx) = (0.0, 0.0);
                            for (k, h) in v.h.iter().enumerate() {
                                if h.is_none() {
                                    sw += w[k];
                                    sx += w[k] * x[k][s];
                                }
                            }
                            sx / sw
                        }
                        _ => mode.map_or(f64::NAN, pick),
                    }
                }
            }
        };
        let impute_of = |j: usize| match avars[j].missing {
            McaMissing::Passive(how) => how,
            _ => Impute::Mode,
        };
        if print.ocorr {
            let columns: Vec<Vec<f64>> = (0..avars.len())
                .map(|j| (0..n).map(|i| filled(j, i, Impute::Mode, None)).collect())
                .collect();
            tables.push(correlation_table(
                "Correlations Original Variables".into(),
                &analysis_titles,
                &correlations(&columns, &w),
                &active_positions,
            ));
        }
        if print.corr {
            for s in 0..p {
                let columns: Vec<Vec<f64>> = (0..avars.len())
                    .map(|j| {
                        (0..n)
                            .map(|i| filled(j, i, impute_of(j), Some(s)))
                            .collect()
                    })
                    .collect();
                tables.push(correlation_table(
                    format!("Correlations Transformed Variables: Dimension {}", s + 1),
                    &analysis_titles,
                    &correlations(&columns, &w),
                    &active_positions,
                ));
            }
        }
        if print.discrim {
            let mut columns = vec![String::new()];
            columns.extend(dims.iter().cloned());
            columns.push("Mean".into());
            let mut rows: Vec<Vec<Cell>> = avars
                .iter()
                .enumerate()
                .map(|(j, v)| {
                    let mut row = vec![text(analysis_titles[j].clone())];
                    row.extend(discr[j].iter().map(|d| Cell::Number(*d)));
                    row.push(Cell::Number(discr[j].iter().sum::<f64>() / p as f64));
                    if v.supplementary {
                        row[0] = text(format!("{} (supplementary)", analysis_titles[j]));
                    }
                    row
                })
                .collect();
            if !fixed {
                let mut total = vec![text("Active Total")];
                total.extend(eigen.iter().map(|e| Cell::Number(*e)));
                total.push(Cell::Number(eigen.iter().sum::<f64>() / p as f64));
                rows.push(total);
                let mut share = vec![text("% of Variance")];
                share.extend(inertia.iter().map(|l| Cell::Number(100.0 * l)));
                share.push(Cell::Number(100.0 * inertia.iter().sum::<f64>() / p as f64));
                rows.push(share);
            }
            tables.push(Table {
                title: "Discrimination Measures".into(),
                columns,
                rows,
                row_headers: 1,
                caption: Some(
                    "The Active Total weights each variable by its weight; supplementary variables take no part in it."
                        .into(),
                ),
                ..Table::default()
            });
        }
        let tr_mw: f64 = w.iter().zip(&mstar).map(|(a, b)| a * b).sum();
        if let Some(items) = &print.quant {
            let names = match items.is_empty() {
                true => analysis_names.clone(),
                false => resolve(items, &analysis_names, &analysis_names)?,
            };
            for name in names {
                let j = analysis_names
                    .iter()
                    .position(|a| a.eq_ignore_ascii_case(&name))
                    .expect("resolved");
                let v = &avars[j];
                let c = &coded[v.coded];
                let mut columns = vec![String::new(), "Frequency".into()];
                let mut groups: Vec<Vec<String>> = vec![Vec::new(), Vec::new()];
                let label_group = |g: &str| vec![g.to_string()];
                for d in &dims {
                    columns.push(d.clone());
                    groups.push(label_group("Centroid Coordinates"));
                }
                columns.push("Mass".into());
                groups.push(Vec::new());
                columns.push("Inertia".into());
                groups.push(Vec::new());
                for d in &dims {
                    columns.push(d.clone());
                    groups.push(label_group("Contribution of Point to Inertia of Dimension"));
                }
                for d in dims.iter().cloned().chain(["Total".to_string()]) {
                    columns.push(d);
                    groups.push(label_group("Contribution of Dimension to Inertia of Point"));
                }
                let mut rows = Vec::new();
                for (r, code) in v.cats.iter().enumerate() {
                    let mut row = vec![text(c.label(*code, None)), Cell::Number(v.d[r])];
                    let q = quantification(j, *code as f64).expect("an active category");
                    row.extend(q.iter().map(|x| Cell::Number(*x)));
                    if v.supplementary || fixed {
                        row.resize(columns.len(), Cell::Empty);
                    } else {
                        let mass = v.weight * v.d[r] / tr_mw;
                        let spread: f64 = (0..n)
                            .filter(|&i| v.h[i] == Some(r))
                            .map(|i| w[i] / mstar[i])
                            .sum();
                        let cat_inertia = v.weight * spread / v.d[r] - mass;
                        let part: Vec<f64> = (0..p)
                            .map(|s| v.weight * v.d[r] * ys[j][r][s].powi(2) / (n_w * m_w))
                            .collect();
                        row.push(Cell::Number(mass));
                        row.push(Cell::Number(cat_inertia));
                        row.extend((0..p).map(|s| num(part[s] / inertia[s])));
                        row.extend(part.iter().map(|x| num(x / cat_inertia)));
                        row.push(num(part.iter().sum::<f64>() / cat_inertia));
                    }
                    rows.push(row);
                }
                for (code, (count, _)) in &extra_cats[j] {
                    let mut row = vec![
                        text(format!("{} (supplementary objects)", c.label(*code, None))),
                        Cell::Number(*count),
                    ];
                    let q = quantification(j, *code as f64).expect("a supplementary category");
                    row.extend(q.iter().map(|x| Cell::Number(*x)));
                    row.resize(columns.len(), Cell::Empty);
                    rows.push(row);
                }
                tables.push(Table {
                    title: format!("Quantifications: {}", c.title),
                    columns,
                    column_groups: groups,
                    rows,
                    row_headers: 1,
                    ..Table::default()
                });
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
        if let Some((label, items)) = &print.object {
            let label = match label {
                Some(name) => {
                    let found = resolve(&[VarItem::Name(name.clone())], &variables, &variables)?;
                    Some(coded_index(&coded, &found[0]))
                }
                None => None,
            };
            let shown_vars: Vec<usize> = resolve(items, &analysis_names, &variables)?
                .iter()
                .map(|name| coded_index(&coded, name))
                .collect();
            let mut columns = vec![String::new()];
            columns.extend(shown_vars.iter().map(|&k| coded[k].title.clone()));
            columns.extend(dims.iter().cloned());
            let mut rows = Vec::new();
            let mut sups = Vec::new();
            for (case, score) in scores.iter().enumerate() {
                let Some(score) = score else { continue };
                if active.binary_search(&case).is_err() {
                    sups.push((case + 1).to_string());
                }
                let mut row = vec![text(object_label(case, &label))];
                row.extend(shown_vars.iter().map(|&k| match coded[k].codes[case] {
                    Some(code) => Cell::Number(code),
                    None => Cell::Empty,
                }));
                row.extend(normalized(score).into_iter().map(Cell::Number));
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
            if !fixed {
                let mut columns = vec![String::new(), "Mass".into(), "Inertia".into()];
                let mut groups: Vec<Vec<String>> = vec![Vec::new(); 3];
                for d in &dims {
                    columns.push(d.clone());
                    groups.push(vec!["Contribution of Point to Inertia of Dimension".into()]);
                }
                for d in dims.iter().cloned().chain(["Total".to_string()]) {
                    columns.push(d);
                    groups.push(vec!["Contribution of Dimension to Inertia of Point".into()]);
                }
                let rows = active
                    .iter()
                    .enumerate()
                    .map(|(i, &case)| {
                        let mass = mstar[i] * w[i] / tr_mw;
                        let spread: f64 = active_vars
                            .iter()
                            .filter_map(|&j| avars[j].h[i].map(|r| avars[j].weight / avars[j].d[r]))
                            .sum();
                        let point_inertia = w[i] * spread / mstar[i] - mass;
                        let part: Vec<f64> = (0..p)
                            .map(|s| mstar[i] * w[i] * x[i][s].powi(2) / (n_w * m_w))
                            .collect();
                        let mut row = vec![
                            text(object_label(case, &label)),
                            Cell::Number(mass),
                            Cell::Number(point_inertia),
                        ];
                        row.extend(part.iter().map(|c| Cell::Number(*c)));
                        let of_point: Vec<f64> = (0..p)
                            .map(|s| part[s] * inertia[s] / point_inertia)
                            .collect();
                        row.extend(of_point.iter().map(|c| num(*c)));
                        row.push(num(of_point.iter().sum()));
                        row
                    })
                    .collect();
                tables.push(Table {
                    title: "Object Contributions".into(),
                    columns,
                    column_groups: groups,
                    rows,
                    row_headers: 1,
                    ..Table::default()
                });
            }
        }

        // charts
        let mut charts = Vec::new();
        let ndim = command.plot.ndim;
        if let Some((a, b)) = ndim
            && b > p
        {
            warnings.push(format!(
                "{WHO}: NDIM({a},{b}) goes beyond the {p} dimensions of the solution"
            ));
        }
        let plot_vars = |items: &[VarItem], allowed: &[String]| -> Result<Vec<usize>, Error> {
            Ok(resolve(items, &analysis_names, allowed)?
                .iter()
                .map(|name| coded_index(&coded, name))
                .collect())
        };
        let analysis_of = |k: usize| avars.iter().position(|v| v.coded == k);
        let object_points = |by: Option<usize>, length: Option<usize>| {
            let mut groups: BTreeMap<Option<i64>, Vec<(String, Vec<f64>)>> = BTreeMap::new();
            for (case, score) in scores.iter().enumerate() {
                let Some(score) = score else { continue };
                let code = by.and_then(|k| coded[k].codes[case]).map(|c| c as i64);
                let label = match (by, code) {
                    (Some(k), Some(code)) => coded[k].label(code, length),
                    _ => (case + 1).to_string(),
                };
                groups
                    .entry(code)
                    .or_default()
                    .push((label, normalized(score)));
            }
            groups
                .into_iter()
                .map(|(code, points)| {
                    let name = match (by, code) {
                        (Some(k), Some(code)) => coded[k].label(code, length),
                        (Some(_), None) => "Missing".to_string(),
                        (None, _) => "Objects".to_string(),
                    };
                    (name, points)
                })
                .collect::<Vec<_>>()
        };
        let category_points = |j: usize, length: Option<usize>| {
            let v = &avars[j];
            let c = &coded[v.coded];
            v.cats
                .iter()
                .copied()
                .chain(extra_cats[j].keys().copied())
                .filter_map(|code| Some((c.label(code, length), quantification(j, code as f64)?)))
                .collect::<Vec<_>>()
        };
        let var_title = |j: usize, length: Option<usize>| {
            let c = &coded[avars[j].coded];
            cut(c.title.clone(), c.name.clone(), length)
        };
        for (chart, length) in &command.plot.charts {
            let length = *length;
            match chart {
                McaPlot::Object(items) => {
                    let by = plot_vars(items, &variables)?;
                    if by.is_empty() {
                        charts.extend(scatter(
                            "Object Points",
                            &object_points(None, length),
                            p,
                            ndim,
                        ));
                    }
                    for k in by {
                        let title = format!(
                            "Object Points Labeled by {}",
                            cut(coded[k].title.clone(), coded[k].name.clone(), length)
                        );
                        charts.extend(scatter(&title, &object_points(Some(k), length), p, ndim));
                    }
                }
                McaPlot::Category(items) => {
                    for k in plot_vars(items, &analysis_names)? {
                        let j = analysis_of(k).expect("an analysis variable");
                        let title = format!("Category Points: {}", var_title(j, length));
                        let groups = vec![(var_title(j, length), category_points(j, length))];
                        charts.extend(scatter(&title, &groups, p, ndim));
                    }
                }
                McaPlot::Jointcat(items) => {
                    let ks = match items.is_empty() {
                        true => avars.iter().map(|v| v.coded).collect(),
                        false => plot_vars(items, &analysis_names)?,
                    };
                    let groups: Vec<_> = ks
                        .iter()
                        .filter_map(|&k| analysis_of(k))
                        .map(|j| (var_title(j, length), category_points(j, length)))
                        .collect();
                    charts.extend(scatter("Joint Plot of Category Points", &groups, p, ndim));
                }
                McaPlot::Discrim(items) => {
                    let ks: Vec<usize> = match items.is_empty() {
                        true => avars.iter().map(|v| v.coded).collect(),
                        false => plot_vars(items, &analysis_names)?,
                    };
                    let points = ks
                        .iter()
                        .filter_map(|&k| analysis_of(k))
                        .map(|j| (var_title(j, length), discr[j].clone()))
                        .collect();
                    let groups = vec![("Variables".to_string(), points)];
                    charts.extend(scatter("Discrimination Measures", &groups, p, ndim));
                }
                McaPlot::Biplot(centroid_items, label_items) => {
                    if command.normalization == McaNormalization::Independent {
                        warnings.push(format!(
                            "{WHO}: a biplot is not drawn with NORMALIZATION=INDEPENDENT"
                        ));
                        continue;
                    }
                    let ks: Vec<usize> = match centroid_items.is_empty() {
                        true => avars.iter().map(|v| v.coded).collect(),
                        false => plot_vars(centroid_items, &analysis_names)?,
                    };
                    let centroid_groups: Vec<_> = ks
                        .iter()
                        .filter_map(|&k| analysis_of(k))
                        .map(|j| (var_title(j, length), category_points(j, length)))
                        .collect();
                    let by = plot_vars(label_items, &variables)?;
                    let labels: Vec<Option<usize>> = match by.is_empty() {
                        true => vec![None],
                        false => by.into_iter().map(Some).collect(),
                    };
                    for k in labels {
                        let title = match k {
                            None => "Biplot".to_string(),
                            Some(k) => format!(
                                "Biplot Labeled by {}",
                                cut(coded[k].title.clone(), coded[k].name.clone(), length)
                            ),
                        };
                        let mut groups = object_points(k, length);
                        groups.extend(centroid_groups.iter().cloned());
                        charts.extend(scatter(&title, &groups, p, ndim));
                    }
                }
                McaPlot::Trans(items) | McaPlot::Resid(items) => {
                    let trans = matches!(chart, McaPlot::Trans(_));
                    for (item, count) in items {
                        for name in
                            resolve(std::slice::from_ref(item), &analysis_names, &analysis_names)?
                        {
                            let j = analysis_names
                                .iter()
                                .position(|a| a.eq_ignore_ascii_case(&name))
                                .expect("resolved");
                            let v = &avars[j];
                            let c = &coded[v.coded];
                            if *count > p {
                                warnings.push(format!(
                                    "{WHO}: {name} asks for {count} dimensions in a plot, but the solution has {p}"
                                ));
                            }
                            for s in 0..(*count).min(p) {
                                let title = format!(
                                    "{}: {}, Dimension {}",
                                    if trans { "Transformation" } else { "Residuals" },
                                    var_title(j, if trans { None } else { length }),
                                    s + 1
                                );
                                if trans {
                                    let codes: Vec<i64> = v
                                        .cats
                                        .iter()
                                        .copied()
                                        .chain(extra_cats[j].keys().copied())
                                        .collect::<std::collections::BTreeSet<_>>()
                                        .into_iter()
                                        .collect();
                                    let categories = codes
                                        .iter()
                                        .map(|code| Category {
                                            label: c.label(*code, None),
                                            value: Cell::Number(*code as f64),
                                            missing: false,
                                            bin: None,
                                        })
                                        .collect();
                                    let series = vec![Series {
                                        name: format!("Dimension {}", s + 1),
                                        statistic: Statistic::Value,
                                        variable: None,
                                        value: Cell::Empty,
                                        values: codes
                                            .iter()
                                            .map(|code| {
                                                quantification(j, *code as f64)
                                                    .map_or(f64::NAN, |q| q[s])
                                            })
                                            .collect(),
                                    }];
                                    charts.push(Chart::lines(
                                        title,
                                        Axis::titled("Category Indicators"),
                                        Axis::titled("Quantifications"),
                                        categories,
                                        series,
                                    ));
                                } else {
                                    let points = active
                                        .iter()
                                        .enumerate()
                                        .filter_map(|(i, &case)| {
                                            let r = v.h[i]?;
                                            Some(Point {
                                                x: x[i][s] * x_factor[s],
                                                y: ys[j][r][s] * y_factor[s],
                                                z: None,
                                                label: Some((case + 1).to_string()),
                                            })
                                        })
                                        .collect();
                                    charts.push(Chart::plot(
                                        ChartKind::Scatter,
                                        title,
                                        Axis::titled("Approximation"),
                                        Axis::titled("Quantification"),
                                        Plot::Points {
                                            groups: vec![PointGroup {
                                                name: c.title.clone(),
                                                points,
                                            }],
                                            lines: Vec::new(),
                                            z_axis: None,
                                        },
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        // the values of each case for SAVE and OUTFILE
        let transformed = |j: usize, s: usize| -> Vec<Option<f64>> {
            (0..n_tot)
                .map(|c| {
                    if excluded[c] {
                        return None;
                    }
                    let code = avars[j].codes[c]?;
                    quantification(j, code).map(|q| q[s])
                })
                .collect()
        };
        let object_column = |s: usize| -> Vec<Option<f64>> {
            scores
                .iter()
                .map(|score| score.as_ref().map(|sc| sc[s] * x_factor[s]))
                .collect()
        };
        let dims_of = |asked: Option<usize>, what: &str, warnings: &mut Vec<String>| match asked {
            Some(k) if k > p => {
                warnings.push(format!(
                    "{WHO}: SAVE {what} asks for {k} dimensions, but the solution has {p}"
                ));
                p
            }
            Some(k) => k,
            None => p,
        };
        let mut saves: Vec<(String, String, Vec<Option<f64>>)> = Vec::new();
        if let Some((root, asked)) = &command.save_trdata {
            let count = dims_of(*asked, "TRDATA", &mut warnings);
            let names = |set: usize| -> Vec<String> {
                (1..=avars.len())
                    .flat_map(|k| (1..=count).map(move |s| format!("{root}{k}_{s}_{set}")))
                    .collect()
            };
            let set = free_set(data, &names);
            for (j, v) in avars.iter().enumerate() {
                for s in 0..count {
                    saves.push((
                        format!("{root}{}_{}_{set}", j + 1, s + 1),
                        format!(
                            "Quantified {} in dimension {} ({WHO} {set})",
                            coded[v.coded].name,
                            s + 1
                        ),
                        transformed(j, s),
                    ));
                }
            }
        }
        if let Some((root, asked)) = &command.save_object {
            let count = dims_of(*asked, "OBJECT", &mut warnings);
            let names = |set: usize| -> Vec<String> {
                (1..=count).map(|s| format!("{root}{s}_{set}")).collect()
            };
            let set = free_set(data, &names);
            for s in 0..count {
                saves.push((
                    format!("{root}{}_{set}", s + 1),
                    format!("Object score in dimension {} ({WHO} {set})", s + 1),
                    object_column(s),
                ));
            }
        }
        let mut files: Vec<(String, Dataset)> = Vec::new();
        if let Some(file) = &command.outfile_discrdata {
            let mut out = Dataset::new(n_tot);
            for v in &avars {
                let c = &coded[v.coded];
                let values = (0..n_tot)
                    .map(|case| c.codes[case].filter(|_| !excluded[case]))
                    .collect();
                out.set_numeric_values(&c.name, values)?;
            }
            files.push((file.clone(), out));
        }
        if let Some(file) = &command.outfile_trdata {
            let mut out = Dataset::new(n_tot);
            for j in 0..avars.len() {
                for s in 0..p {
                    out.set_numeric_values(&format!("TRA{}_{}", j + 1, s + 1), transformed(j, s))?;
                }
            }
            files.push((file.clone(), out));
        }
        if let Some(file) = &command.outfile_object {
            let mut out = Dataset::new(n_tot);
            for s in 0..p {
                out.set_numeric_values(&format!("OBSCO{}", s + 1), object_column(s))?;
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
    fn normal_thresholds_are_symmetric_and_known_for_two_groups() {
        let t = normal_thresholds(2);
        assert!(t[0].abs() < 1e-9);
        let t = normal_thresholds(7);
        for r in 0..6 {
            assert!((t[r] + t[5 - r]).abs() < 1e-8, "{t:?}");
        }
        // the optimal quantizer of 4 levels splits at 0 and about 0.9816
        let t = normal_thresholds(4);
        assert!((t[2] - 0.9816).abs() < 1e-3, "{t:?}");
    }

    #[test]
    fn discretizing_ranks_multiplies_and_cuts_intervals() {
        let values = [2.5, 0.5, 2.5, 7.0, 4.0];
        let get = |m: &BTreeMap<u64, f64>, x: f64| m[&x.to_bits()];
        let ranks = discretize(&values, Discretize::Ranking);
        assert_eq!(get(&ranks, 0.5), 1.0);
        assert_eq!(get(&ranks, 7.0), 4.0);
        let cut = discretize(&values, Discretize::Intervals(2.0));
        assert_eq!(get(&cut, 0.5), 1.0);
        assert_eq!(get(&cut, 2.5), 2.0);
        assert_eq!(get(&cut, 7.0), 4.0);
        let times = discretize(&values, Discretize::Multiplying);
        assert_eq!(get(&times, 0.5), 1.0);
        assert!(get(&times, 7.0) > get(&times, 4.0));
        let uniform = discretize(
            &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
            Discretize::Grouping {
                ncat: Some(3),
                normal: false,
            },
        );
        let groups: Vec<f64> = (1..=9).map(|x| get(&uniform, x as f64)).collect();
        assert_eq!(groups, vec![1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 3.0, 3.0, 3.0]);
    }
}
