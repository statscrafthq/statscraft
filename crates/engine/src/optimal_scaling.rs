//! What MULTIPLE CORRESPONDENCE and CATPCA share: coding the variables into categories
//! (with DISCRETIZATION), choosing the active cases and imputing missing values (MISSING,
//! SUPPLEMENTARY), the configuration file, the exact start, orthonormalization, and the
//! tables and charts that both show the same way.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - Every analysis variable is a set of categories coded by positive whole numbers. Values
//!   below 1, system-missing and user-missing values are missing. A string variable is first
//!   coded by the sorted order of its values. A numeric variable with fractions is grouped into
//!   7 categories (fewer when it has fewer values) shaped like a normal distribution, unless
//!   DISCRETIZATION says otherwise. Discretizing ignores the case weights and sees values below
//!   1 as valid.
//! - ACTIVE imputes before the analysis: the weighted mode (the smallest code on ties), a new
//!   category above every code, or a random category drawn by weighted frequency. LISTWISE
//!   drops the case, after the imputation. PASSIVE leaves the variable out for that object.
//!   An object without any active value becomes supplementary; at least three active objects
//!   are needed.
//! - Object scores are centered and orthonormal in the metric of the case weight times the
//!   summed weights of the variables an object has.

use crate::chart::{Axis, Category, ChartKind, Series, Statistic};
use crate::descriptives::case_weights;
use crate::dist::{normal_cdf, normal_quantile};
use crate::matrix::{Matrix, symmetric_eigen};
use crate::means::is_valid;
use crate::npar_tests::text;
use crate::plot::{CaseGroup, CaseValues, Plot, Point, PointGroup};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_data::{Dataset, Values, Variable};
use statscraft_syntax::{Discretize, Impute, McaMissing, ScalingCommon, VarItem};
use std::collections::BTreeMap;

/// Above this many categories the exact start is skipped and the iterations start from a
/// fixed pseudo-random configuration.
pub(crate) const EXACT_START_LIMIT: usize = 600;

/// Named groups of labelled points, for a chart.
pub(crate) type Groups = [(String, Vec<(String, Vec<f64>)>)];
/// Named groups of labelled points, owned.
pub(crate) type GroupList = Vec<(String, Vec<(String, Vec<f64>)>)>;

/// One variable coded into categories.
pub(crate) struct Coded {
    pub(crate) name: String,
    pub(crate) title: String,
    /// Per case: the category code, at least 1, or None when missing.
    pub(crate) codes: Vec<Option<f64>>,
    /// Per code: the label shown and the plain value.
    pub(crate) labels: BTreeMap<i64, (String, String)>,
}

impl Coded {
    pub(crate) fn label(&self, code: i64, length: Option<usize>) -> String {
        let (shown, plain) = self
            .labels
            .get(&code)
            .cloned()
            .unwrap_or_else(|| (code.to_string(), code.to_string()));
        cut(shown, plain, length)
    }
}

/// A label for a chart: the value itself with length 0, else the label cut to `length`.
pub(crate) fn cut(shown: String, plain: String, length: Option<usize>) -> String {
    match length {
        None => shown,
        Some(0) => plain,
        Some(l) => shown.chars().take(l).collect(),
    }
}

pub(crate) fn plain_number(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

/// The thresholds that split a standard normal variable into `k` groups with the least
/// squared error (the optimal quantizer), found by alternating centroids and midpoints.
pub(crate) fn normal_thresholds(k: usize) -> Vec<f64> {
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
pub(crate) fn distinct(values: &[f64]) -> Vec<(f64, usize)> {
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
pub(crate) fn mean_sd(values: &[f64]) -> (f64, f64) {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let ss: f64 = values.iter().map(|x| (x - mean).powi(2)).sum();
    (mean, (ss / (n - 1.0).max(1.0)).sqrt())
}

/// Recodes the valid values as `how` says; the map goes from a value to its code.
pub(crate) fn discretize(values: &[f64], how: Discretize) -> BTreeMap<u64, f64> {
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
pub(crate) fn code_variable(
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
pub(crate) fn resolve(
    who: &str,
    items: &[VarItem],
    order: &[String],
    allowed: &[String],
) -> Result<Vec<String>, Error> {
    let find = |list: &[String], name: &str| list.iter().position(|v| v.eq_ignore_ascii_case(name));
    let mut out = Vec::new();
    for item in items {
        match item {
            VarItem::All => out.extend(order.iter().cloned()),
            VarItem::Name(name) => match find(allowed, name) {
                Some(i) => out.push(allowed[i].clone()),
                None => {
                    return Err(Error(format!(
                        "{who}: {name} is not among the variables this list may name"
                    )));
                }
            },
            VarItem::Range(a, b) => {
                let (Some(i), Some(j)) = (find(order, a), find(order, b)) else {
                    return Err(Error(format!(
                        "{who}: {a} TO {b} must run between variables of the list it refers to"
                    )));
                };
                if i > j {
                    return Err(Error(format!(
                        "{who}: in {a} TO {b} the first variable must come first"
                    )));
                }
                out.extend(order[i..=j].iter().cloned());
            }
        }
    }
    Ok(out)
}

pub(crate) fn coded_index(coded: &[Coded], name: &str) -> usize {
    coded
        .iter()
        .position(|c| c.name.eq_ignore_ascii_case(name))
        .expect("named on VARIABLES")
}

/// One variable of ANALYSIS as the fit sees it.
pub(crate) struct AVar {
    /// The ANALYSIS list it was named in.
    pub(crate) list: usize,
    /// Index into the coded variables.
    pub(crate) coded: usize,
    pub(crate) weight: f64,
    pub(crate) missing: McaMissing,
    /// The code of the extra category that ACTIVE EXTRACAT made.
    pub(crate) extra: Option<i64>,
    pub(crate) supplementary: bool,
    /// Per case: the code after imputation.
    pub(crate) codes: Vec<Option<f64>>,
    /// The codes the active objects have, sorted.
    pub(crate) cats: Vec<i64>,
    /// Per active object: its category index.
    pub(crate) h: Vec<Option<usize>>,
    /// Weighted frequency of each category among the active objects.
    pub(crate) d: Vec<f64>,
}

/// Weighted category centroids of the scores.
pub(crate) fn centroids(v: &AVar, x: &Matrix, w: &[f64], p: usize) -> Matrix {
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
pub(crate) fn eigen_sorted(a: &Matrix) -> (Vec<f64>, Matrix) {
    let pairs = symmetric_eigen(a);
    let values = pairs.iter().map(|(v, _)| *v).collect();
    let p = a.len();
    let vectors = (0..p)
        .map(|i| pairs.iter().map(|(_, v)| v[i]).collect())
        .collect();
    (values, vectors)
}

/// The centering means and the transformation that make `v` centered and orthonormal in the
/// metric `mw` with sum of squares `scale`; None when a dimension has no variance left.
pub(crate) fn orthonormal_transform(
    v: &Matrix,
    mw: &[f64],
    scale: f64,
) -> Option<(Vec<f64>, Matrix)> {
    let p = v.first()?.len();
    let total: f64 = mw.iter().sum();
    let means: Vec<f64> = (0..p)
        .map(|s| v.iter().zip(mw).map(|(r, m)| m * r[s]).sum::<f64>() / total)
        .collect();
    let cross: Matrix = (0..p)
        .map(|a| {
            (0..p)
                .map(|b| {
                    v.iter()
                        .zip(mw)
                        .map(|(r, m)| m * (r[a] - means[a]) * (r[b] - means[b]))
                        .sum()
                })
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
    // L diag(1/sqrt(value)) L': v times it is the orthonormal matrix nearest to v
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
    Some((means, t))
}

/// Applies a centering and transformation from `orthonormal_transform` to the rows of `v`.
pub(crate) fn transform_rows(v: &mut Matrix, means: &[f64], t: &Matrix) {
    let p = means.len();
    for row in v.iter_mut() {
        let new: Vec<f64> = (0..p)
            .map(|b| (0..p).map(|a| (row[a] - means[a]) * t[a][b]).sum())
            .collect();
        *row = new;
    }
}

/// Centers `v` and makes it orthonormal in the metric `mw` with sum of squares `scale`.
pub(crate) fn orthonormalize(v: &mut Matrix, mw: &[f64], scale: f64) -> Option<()> {
    let (means, t) = orthonormal_transform(v, mw, scale)?;
    transform_rows(v, &means, &t);
    Some(())
}

/// The exact solution of the problem in which each variable's quantified values are confined
/// to fixed columns: one indicator column per category (`basis` None) or the single column of
/// values per category that `basis` gives. The objects lie in the span of their columns as
/// closely as possible: the leading eigenvectors of the cross products of the columns, each
/// scaled to unit sum of squares and with the constant direction taken out, mapped back to the
/// objects. Centered and orthonormal as the fit needs; None when fewer than `p` dimensions
/// carry variance.
pub(crate) fn exact_start(
    vars: &[&AVar],
    basis: &[Option<Vec<f64>>],
    w: &[f64],
    mstar: &[f64],
    p: usize,
    nm: f64,
) -> Option<Matrix> {
    // per column: its variable, the value of each category on it, and its scale
    let mut columns: Vec<(usize, Vec<f64>, f64)> = Vec::new();
    for (j, v) in vars.iter().enumerate() {
        match &basis[j] {
            Some(values) => {
                let ss: f64 = values.iter().zip(&v.d).map(|(q, d)| d * q * q).sum();
                if ss > 0.0 {
                    columns.push((j, values.clone(), (v.weight / ss).sqrt()));
                }
            }
            None => {
                for r in 0..v.cats.len() {
                    let mut values = vec![0.0; v.cats.len()];
                    values[r] = 1.0;
                    columns.push((j, values, (v.weight / v.d[r]).sqrt()));
                }
            }
        }
    }
    let k = columns.len();
    let n = w.len();
    let value = |a: usize, i: usize| -> f64 {
        let (j, values, scale) = &columns[a];
        vars[*j].h[i].map_or(0.0, |r| scale * values[r])
    };
    let mut b = vec![vec![0.0; k]; k];
    let mut row = vec![0.0; k];
    for i in 0..n {
        for (a, x) in row.iter_mut().enumerate() {
            *x = value(a, i);
        }
        let f = w[i] / mstar[i];
        for a in 0..k {
            if row[a] != 0.0 {
                for c in 0..k {
                    b[a][c] += f * row[a] * row[c];
                }
            }
        }
    }
    // the image of the constant object vector, taken out
    let trivial: Vec<f64> = (0..k)
        .map(|a| (0..n).map(|i| w[i] * value(a, i)).sum())
        .collect();
    let norm: f64 = w.iter().zip(mstar).map(|(a, b)| a * b).sum();
    for a in 0..k {
        for c in 0..k {
            b[a][c] -= trivial[a] * trivial[c] / norm;
        }
    }
    let pairs = symmetric_eigen(&b);
    if pairs.len() < p || pairs[..p].iter().any(|(e, _)| *e <= 1e-10) {
        return None;
    }
    let mut x = vec![vec![0.0; p]; n];
    for (s, (_, vector)) in pairs.iter().take(p).enumerate() {
        for i in 0..n {
            x[i][s] = (0..k).map(|a| value(a, i) * vector[a]).sum::<f64>() / mstar[i];
        }
    }
    let mw: Vec<f64> = w.iter().zip(mstar).map(|(a, b)| a * b).collect();
    orthonormalize(&mut x, &mw, nm)?;
    Some(x)
}

/// A fixed pseudo-random configuration, centered and orthonormal, so that runs repeat.
pub(crate) fn pseudo_random_start(w: &[f64], mstar: &[f64], p: usize, nm: f64) -> Option<Matrix> {
    let mut state: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut x: Matrix = (0..w.len())
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
    let mw: Vec<f64> = w.iter().zip(mstar).map(|(a, b)| a * b).collect();
    orthonormalize(&mut x, &mw, nm)?;
    Some(x)
}

/// Weighted Pearson correlations of the columns; NaN where a column is constant.
pub(crate) fn correlations(columns: &[Vec<f64>], w: &[f64]) -> Matrix {
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

pub(crate) fn num(x: f64) -> Cell {
    if x.is_finite() {
        Cell::Number(x)
    } else {
        Cell::Empty
    }
}

/// A correlation table with the eigenvalues of the active part weighted by the variable
/// weights in its last row.
pub(crate) fn correlation_table(
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
pub(crate) fn scatter(
    title: &str,
    groups: &Groups,
    p: usize,
    ndim: Option<(usize, usize)>,
) -> Vec<Chart> {
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

/// Object points grouped by the category of the variable `by` (one group without it), each
/// labelled by that category or else by its case number.
pub(crate) fn object_groups(
    coded: &[Coded],
    scores: &[Option<Vec<f64>>],
    by: Option<usize>,
    length: Option<usize>,
) -> GroupList {
    let mut groups: BTreeMap<Option<i64>, Vec<(String, Vec<f64>)>> = BTreeMap::new();
    for (case, score) in scores.iter().enumerate() {
        let Some(score) = score else { continue };
        let code = by.and_then(|k| coded[k].codes[case]).map(|c| c as i64);
        let label = match (by, code) {
            (Some(k), Some(code)) => coded[k].label(code, length),
            _ => (case + 1).to_string(),
        };
        groups.entry(code).or_default().push((label, score.clone()));
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
        .collect()
}

/// A transformation chart: the quantification of each category (NaN for none) against its
/// code.
pub(crate) fn transformation_chart(
    title: String,
    coded: &Coded,
    codes: &[i64],
    series: &str,
    values: Vec<f64>,
) -> Chart {
    let categories = codes
        .iter()
        .map(|code| Category {
            label: coded.label(*code, None),
            value: Cell::Number(*code as f64),
            missing: false,
            bin: None,
        })
        .collect();
    Chart::lines(
        title,
        Axis::titled("Category Indicators"),
        Axis::titled("Quantifications"),
        categories,
        vec![Series {
            name: series.to_string(),
            statistic: Statistic::Value,
            variable: None,
            value: Cell::Empty,
            values,
        }],
    )
}

/// A scatter of labelled points in one group.
pub(crate) fn point_chart(
    title: String,
    x: &str,
    y: &str,
    group: String,
    points: Vec<Point>,
) -> Chart {
    Chart::plot(
        ChartKind::Scatter,
        title,
        Axis::titled(x),
        Axis::titled(y),
        Plot::Points {
            groups: vec![PointGroup {
                name: group,
                points,
            }],
            lines: Vec::new(),
            z_axis: None,
        },
    )
}

/// The first free set number n for names made by `name(n)`.
pub(crate) fn free_set(data: &Dataset, names: &dyn Fn(usize) -> Vec<String>) -> usize {
    (1..)
        .find(|&n| {
            names(n)
                .iter()
                .all(|name| data.variable_index(name).is_none())
        })
        .expect("some set is free")
}

/// The data of an analysis after coding, imputation and the choice of active cases.
pub(crate) struct Prepared {
    pub(crate) variables: Vec<String>,
    pub(crate) analysis_names: Vec<String>,
    /// Every variable on VARIABLES, coded.
    pub(crate) coded: Vec<Coded>,
    /// Every variable on ANALYSIS, in order.
    pub(crate) avars: Vec<AVar>,
    pub(crate) active_vars: Vec<usize>,
    /// The case numbers (from 0) of the active objects, ascending.
    pub(crate) active: Vec<usize>,
    /// Cases that became supplementary because they have no active value.
    pub(crate) without_data: Vec<usize>,
    pub(crate) excluded: Vec<bool>,
    pub(crate) sup_object: Vec<bool>,
    /// Per active object: its case weight.
    pub(crate) w: Vec<f64>,
    pub(crate) n_w: f64,
    pub(crate) m_w: f64,
    /// Per active object: the summed weights of the active variables it has.
    pub(crate) mstar: Vec<f64>,
    pub(crate) passive_missing: bool,
    pub(crate) fixed: bool,
    pub(crate) n_tot: usize,
    pub(crate) warnings: Vec<String>,
}

impl Prepared {
    /// The case weight times `mstar` of every active object.
    pub(crate) fn metric(&self) -> Vec<f64> {
        self.w.iter().zip(&self.mstar).map(|(a, b)| a * b).collect()
    }

    pub(crate) fn analysis_index(&self, name: &str) -> usize {
        self.analysis_names
            .iter()
            .position(|a| a.eq_ignore_ascii_case(name))
            .expect("an analysis variable")
    }

    /// Case Processing Summary.
    pub(crate) fn case_summary(&self) -> Table {
        let n = self.active.len();
        let passive_cases = (0..n)
            .filter(|&i| {
                self.active_vars
                    .iter()
                    .any(|&j| self.avars[j].h[i].is_none())
            })
            .count();
        let sup_count = (0..self.n_tot)
            .filter(|&c| {
                !self.excluded[c] && (self.sup_object[c] || self.without_data.contains(&c))
            })
            .count();
        let excluded_count = self.excluded.iter().filter(|e| **e).count();
        let row = |label: &str, v: usize| vec![text(label), Cell::Number(v as f64)];
        Table {
            title: "Case Processing Summary".into(),
            columns: vec![String::new(), "N".into()],
            rows: vec![
                row("Valid Active Cases", n - passive_cases),
                row("Active Cases with Missing Values", passive_cases),
                row("Supplementary Cases", sup_count),
                row("Excluded Cases", excluded_count),
                row("Total", self.n_tot),
                row("Cases Used in Analysis", n),
            ],
            row_headers: 1,
            ..Table::default()
        }
    }

    /// PRINT DESCRIP: weighted frequencies of the codes before imputation, the missing
    /// count and the mode, one table per variable (the analysis variables for an empty list).
    pub(crate) fn descriptives(&self, who: &str, items: &[VarItem]) -> Result<Vec<Table>, Error> {
        let names = match items.is_empty() {
            true => self.analysis_names.clone(),
            false => resolve(who, items, &self.analysis_names, &self.variables)?,
        };
        let mut tables = Vec::new();
        for name in names {
            let c = &self.coded[coded_index(&self.coded, &name)];
            let mut freq: BTreeMap<i64, f64> = BTreeMap::new();
            let mut lacking = 0.0;
            for (i, &case) in self.active.iter().enumerate() {
                match c.codes[case] {
                    Some(code) => *freq.entry(code as i64).or_default() += self.w[i],
                    None => lacking += self.w[i],
                }
            }
            let mode = weighted_mode(&freq);
            let mut rows: Vec<Vec<Cell>> = freq
                .iter()
                .map(|(k, f)| vec![text(c.label(*k, None)), Cell::Number(*f)])
                .collect();
            rows.push(vec![text("Missing"), Cell::Number(lacking)]);
            rows.push(vec![text("Total"), Cell::Number(self.n_w)]);
            tables.push(Table {
                title: c.title.clone(),
                columns: vec![String::new(), "Frequency".into()],
                rows,
                row_headers: 1,
                caption: mode.map(|m| format!("Mode: {}", c.label(m, None))),
                ..Table::default()
            });
        }
        Ok(tables)
    }

    /// The configuration in a file: its first `p` numeric variables, one row per active
    /// object.
    pub(crate) fn configuration(
        &self,
        engine: &Engine,
        who: &str,
        file: &str,
        p: usize,
    ) -> Result<Matrix, Error> {
        let config = engine.data_file(file)?;
        if config.case_count() != self.n_tot {
            return Err(Error(format!(
                "{who}: the configuration in {file} has {} cases, but the data have {}",
                config.case_count(),
                self.n_tot
            )));
        }
        let columns: Vec<Vec<Option<f64>>> = config
            .variables()
            .filter_map(|v| v.numeric().ok().map(|x| x.iter().collect()))
            .take(p)
            .collect();
        if columns.len() < p {
            return Err(Error(format!(
                "{who}: the configuration in {file} has {} numeric variables for {p} dimensions",
                columns.len()
            )));
        }
        let rows: Option<Matrix> = self
            .active
            .iter()
            .map(|&c| {
                columns
                    .iter()
                    .map(|col| col[c])
                    .collect::<Option<Vec<f64>>>()
            })
            .collect();
        rows.ok_or_else(|| {
            Error(format!(
                "{who}: the configuration in {file} has a missing coordinate for an active case"
            ))
        })
    }
}

impl Prepared {
    /// The objects to start from. With CONFIGURATION, the file's coordinates: FIXED centered
    /// and orthonormal with the case weights, INITIAL as the fit needs them. Without it the
    /// exact start for the active variables confined to `basis` (see `exact_start`), or a
    /// pseudo-random one when there are more than EXACT_START_LIMIT columns.
    pub(crate) fn start(
        &self,
        engine: &Engine,
        who: &str,
        configuration: &Option<(bool, String)>,
        basis: &[Option<Vec<f64>>],
        p: usize,
    ) -> Result<Matrix, Error> {
        let nm = self.n_w * self.m_w;
        if let Some((fixed, file)) = configuration {
            let mut x = self.configuration(engine, who, file, p)?;
            let done = match fixed {
                true => orthonormalize(&mut x, &self.w, self.n_w),
                false => orthonormalize(&mut x, &self.metric(), nm),
            };
            if done.is_none() {
                return Err(Error(format!(
                    "{who}: the {} configuration has a dimension without variance or two that coincide",
                    if *fixed { "fixed" } else { "initial" }
                )));
            }
            return Ok(x);
        }
        let act: Vec<&AVar> = self.active_vars.iter().map(|&j| &self.avars[j]).collect();
        let columns: usize = act
            .iter()
            .zip(basis)
            .map(|(v, b)| b.as_ref().map_or(v.cats.len(), |_| 1))
            .sum();
        let x = match columns <= EXACT_START_LIMIT {
            true => exact_start(&act, basis, &self.w, &self.mstar, p, nm),
            false => pseudo_random_start(&self.w, &self.mstar, p, nm),
        };
        x.ok_or_else(|| {
            Error(format!(
                "{who}: the data support fewer than {p} dimensions; lower DIMENSION"
            ))
        })
    }
}

/// The code with the largest weighted frequency, the smallest on ties.
pub(crate) fn weighted_mode(freq: &BTreeMap<i64, f64>) -> Option<i64> {
    freq.iter()
        .fold(None, |best: Option<(i64, f64)>, (&k, &f)| match best {
            Some((_, bf)) if bf >= f => best,
            _ => Some((k, f)),
        })
        .map(|(k, _)| k)
}

impl Engine {
    /// Resolves the lists, codes the variables, imputes ACTIVE missing values and chooses the
    /// active objects. `analysis` holds the ANALYSIS lists with their variable weights.
    pub(crate) fn prepare_scaling(
        &self,
        who: &str,
        common: &ScalingCommon,
        analysis_lists: &[(&[VarItem], f64)],
    ) -> Result<Prepared, Error> {
        let shown = self.shown();
        let data = self.active()?;
        let n_tot = data.case_count();
        let mut warnings: Vec<String> = Vec::new();
        let fixed = matches!(common.configuration, Some((true, _)));

        // the lists
        let variables = crate::expand_in(data, &common.variables)?;
        let mut analysis: Vec<(String, f64, usize)> = Vec::new();
        for (list, (items, weight)) in analysis_lists.iter().enumerate() {
            for name in resolve(who, items, &variables, &variables)? {
                if analysis
                    .iter()
                    .any(|(a, _, _)| a.eq_ignore_ascii_case(&name))
                {
                    return Err(Error(format!("{who}: ANALYSIS names {name} twice")));
                }
                analysis.push((name, *weight, list));
            }
        }
        if !fixed && (variables.len() < 2 || analysis.len() < 2) {
            return Err(Error(format!(
                "{who} needs at least two variables on VARIABLES and on ANALYSIS"
            )));
        }
        let analysis_names: Vec<String> = analysis.iter().map(|(a, _, _)| a.clone()).collect();
        let mut discretization: BTreeMap<String, Discretize> = BTreeMap::new();
        for (items, how) in &common.discretization {
            for name in resolve(who, items, &variables, &variables)? {
                discretization.insert(name.to_uppercase(), *how);
            }
        }
        let mut missing: BTreeMap<String, McaMissing> = BTreeMap::new();
        for (items, how) in &common.missing {
            for name in resolve(who, items, &analysis_names, &analysis_names)? {
                missing.insert(name.to_uppercase(), *how);
            }
        }
        let mut supplementary: Vec<String> = Vec::new();
        if fixed {
            if !common.supplementary_objects.is_empty() {
                return Err(Error(format!(
                    "{who}: SUPPLEMENTARY OBJECT cannot go with CONFIGURATION=FIXED"
                )));
            }
            if !common.supplementary_variables.is_empty() {
                warnings.push(format!(
                    "{who}: with a FIXED configuration every variable is supplementary; SUPPLEMENTARY VARIABLE is ignored"
                ));
            }
            supplementary = analysis_names.clone();
        } else {
            for name in resolve(
                who,
                &common.supplementary_variables,
                &variables,
                &analysis_names,
            )? {
                supplementary.push(name);
            }
        }
        let is_sup = |name: &str| supplementary.iter().any(|s| s.eq_ignore_ascii_case(name));
        for (name, weight, _) in &analysis {
            if is_sup(name) && *weight != 1.0 && !fixed {
                warnings.push(format!(
                    "{who}: the weight of supplementary variable {name} is ignored"
                ));
            }
        }

        // the codes of every variable on VARIABLES
        let largest = match &common.legacy {
            Some(legacy) => crate::legacy_scaling::largest_categories(who, data, &legacy.ranges)?,
            None => BTreeMap::new(),
        };
        let mut coded: Vec<Coded> = Vec::new();
        for name in &variables {
            let var = data.variable(name)?;
            coded.push(match largest.get(&name.to_uppercase()) {
                Some(&max) => crate::legacy_scaling::code_in_range(who, &var, max, &shown)?,
                None => code_variable(
                    &var,
                    discretization.get(&name.to_uppercase()).copied(),
                    &shown,
                )?,
            });
        }

        // cases: weights, supplementary objects, imputation, listwise deletion
        let weights = case_weights(data)?.unwrap_or_else(|| vec![1.0; n_tot]);
        let mut sup_object = vec![false; n_tot];
        for &(a, b) in &common.supplementary_objects {
            if b > n_tot {
                return Err(Error(format!(
                    "{who}: SUPPLEMENTARY OBJECT names case {b}, but the data have {n_tot} cases"
                )));
            }
            sup_object[a - 1..b].iter_mut().for_each(|s| *s = true);
        }
        let first_cases = common.legacy.as_ref().and_then(|l| l.first_cases);
        let mut excluded: Vec<bool> = (weights.iter().enumerate())
            .map(|(c, w)| *w <= 0.0 || first_cases.is_some_and(|n| c >= n))
            .collect();
        let mut avars: Vec<AVar> = Vec::new();
        for (name, weight, list) in &analysis {
            let c = coded_index(&coded, name);
            let sup = is_sup(name);
            let how = missing
                .get(&name.to_uppercase())
                .copied()
                .unwrap_or(McaMissing::Passive(Impute::Mode));
            if sup && how == McaMissing::Listwise {
                return Err(Error(format!(
                    "{who}: supplementary variable {name} cannot have LISTWISE missing values"
                )));
            }
            let mut codes = coded[c].codes.clone();
            let mut extra_code = None;
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
                let mode = weighted_mode(&freq).map(|k| k as f64);
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
                    extra_code = Some(extra as i64);
                }
            }
            if how == McaMissing::Listwise {
                for case in 0..n_tot {
                    excluded[case] |= codes[case].is_none();
                }
            }
            avars.push(AVar {
                list: *list,
                coded: c,
                weight: if sup { 1.0 } else { *weight },
                missing: how,
                extra: extra_code,
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
                "{who} needs at least three active cases, but has {n}"
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
        Ok(Prepared {
            variables,
            analysis_names,
            coded,
            avars,
            active_vars,
            active,
            without_data,
            excluded,
            sup_object,
            w,
            n_w,
            m_w,
            mstar,
            passive_missing,
            fixed,
            n_tot,
            warnings,
        })
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
