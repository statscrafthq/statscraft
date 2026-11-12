//! FACTOR: factor analysis of the correlation matrix.
//!
//! Rules from the Command Syntax Reference (FACTOR) and the Algorithms manual (FACTOR):
//! - LISTWISE uses the cases valid on every variable of VARIABLES, PAIRWISE each pair's cases
//!   (each variable's mean and deviation from its own cases), MEANSUB every case with the mean in
//!   place of a missing value. SELECT limits the matrix to the cases with that value. Weights:
//!   W is the sum of the case weights; covariances divide by W - 1.
//! - The number of factors is FACTORS(n), else the count of eigenvalues of R above MINEIGEN.
//! - PC: loadings are the eigenvectors times the square roots of the eigenvalues. PAF iterates
//!   the communalities on the diagonal from the squared multiple correlations (or DIAGONAL).
//!   ALPHA and IMAGE follow Kaiser's formulas. ML, GLS and ULS minimize their fit function by
//!   Newton-Raphson on the unique variances, with step halving and the approximate second
//!   derivatives at the first step, when the exact ones are not positive definite or when a step
//!   is above 0.1; a second derivative below 0.05 (a Heywood variable) fixes that variable.
//! - Orthogonal rotations rotate pairs of factors by the angle tan 4P = X / Y, after Kaiser
//!   normalization unless NOKAISER, until the simplicity criterion changes by at most
//!   RCONVERGE; factors with a negative sum are reflected and the factors are ordered by their
//!   sums of squares. OBLIMIN is direct oblimin (Jennrich and Sampson); PROMAX starts from
//!   varimax.
//! - Factor score coefficients: regression, Bartlett or Anderson-Rubin as the manual gives
//!   them; scores are computed from the z-scores of every case with the values it needs, also
//!   the cases SELECT leaves out of the matrix.
//! - KMO and Bartlett's test, the anti-image matrices and the reproduced correlations follow
//!   the Optional Statistics of the manual.

use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, t_p};
use crate::matrix::{Matrix, inverse, multiply, symmetric_eigen, transpose};
use crate::means::is_valid;
use crate::npar_tests::text;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Values, Variable};
use statscraft_syntax::{
    Extraction, Factor, FactorCriteria, FactorMissing, MatrixKind, Rotation, ScoreMethod, Value,
    VarItem,
};

fn identity(n: usize) -> Matrix {
    (0..n)
        .map(|i| (0..n).map(|j| f64::from(u8::from(i == j))).collect())
        .collect()
}

/// The symmetric power of a symmetric positive definite matrix, by its eigenvalues.
fn symmetric_power(a: &Matrix, power: f64) -> Option<Matrix> {
    let pairs = symmetric_eigen(a);
    if pairs.iter().any(|(v, _)| *v <= 0.0) {
        return None;
    }
    let n = a.len();
    Some(
        (0..n)
            .map(|i| {
                (0..n)
                    .map(|j| pairs.iter().map(|(v, w)| v.powf(power) * w[i] * w[j]).sum())
                    .collect()
            })
            .collect(),
    )
}

/// True when the Cholesky factorization of the symmetric matrix succeeds.
fn positive_definite(a: &Matrix) -> bool {
    let n = a.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let s: f64 = a[i][j] - (0..j).map(|k| l[i][k] * l[j][k]).sum::<f64>();
            if i == j {
                if s <= 0.0 || s.is_nan() {
                    return false;
                }
                l[i][i] = s.sqrt();
            } else {
                l[i][j] = s / l[j][j];
            }
        }
    }
    true
}

fn column_sums_of_squares(a: &Matrix, m: usize) -> Vec<f64> {
    (0..m)
        .map(|j| a.iter().map(|row| row[j] * row[j]).sum())
        .collect()
}

fn row_sums_of_squares(a: &Matrix) -> Vec<f64> {
    a.iter()
        .map(|row| row.iter().map(|v| v * v).sum())
        .collect()
}

/// The correlation matrix of the analysis and what goes with it.
struct Input {
    names: Vec<String>,
    r: Matrix,
    cov: Option<Matrix>,
    mean: Option<Vec<f64>>,
    sd: Vec<f64>,
    /// Analysis N of each variable and, with PAIRWISE and MEANSUB, the missing count.
    n: Option<Vec<f64>>,
    missing: Option<Vec<f64>>,
    /// The N of the tests: W, or the smallest pair count; None when a matrix gives no N.
    w: Option<f64>,
    pair_n: Option<Matrix>,
    /// The z-scores of every case for the factor scores, None where a value is missing.
    z: Option<Vec<Option<Vec<f64>>>>,
}

/// The variables of ANALYSIS within `list`, TO running in the order of `list`.
fn analysis_names(list: &[String], items: &[VarItem]) -> Result<Vec<String>, Error> {
    let position = |name: &str| {
        list.iter()
            .position(|n| n.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                Error(format!(
                    "{name} is on ANALYSIS but not on VARIABLES of FACTOR"
                ))
            })
    };
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let range = match item {
            VarItem::All => 0..list.len(),
            VarItem::Name(name) => {
                let i = position(name)?;
                i..i + 1
            }
            VarItem::Range(a, b) => {
                let (i, j) = (position(a)?, position(b)?);
                if i > j {
                    return Err(Error(format!(
                        "in ANALYSIS {a} TO {b} the first variable must come first on VARIABLES"
                    )));
                }
                i..j + 1
            }
        };
        for name in &list[range] {
            if !out.contains(name) {
                out.push(name.clone());
            }
        }
    }
    Ok(out)
}

fn matches_value(variable: &Variable, case: usize, value: &Value) -> bool {
    match (variable.values(), value) {
        (Values::Numeric(values), Value::Number(n)) => values.get(case) == Some(*n),
        (Values::String(values), Value::String(s)) => values.get(case).trim_end() == s.trim_end(),
        _ => false,
    }
}

impl Engine {
    /// The correlation matrix from the cases of the active dataset.
    fn factor_data(&self, command: &Factor) -> Result<Input, Error> {
        let data = self.active()?;
        let all = self.expand(&command.variables)?;
        let names = match &command.analysis {
            Some(items) => analysis_names(&all, items)?,
            None => all.clone(),
        };
        let variables: Vec<Variable> = all
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = variables.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "FACTOR needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let cases = data.case_count();
        let weights = case_weights(data)?;
        let selected: Vec<bool> = match &command.select {
            None => vec![true; cases],
            Some((name, value)) => {
                let v = data.variable(name)?;
                (0..cases).map(|k| matches_value(&v, k, value)).collect()
            }
        };
        let w: Vec<f64> = (0..cases)
            .map(|k| match selected[k] {
                true => weights.as_ref().map_or(1.0, |w| w[k]),
                false => 0.0,
            })
            .collect();
        let value = |v: &Variable, k: usize| -> f64 {
            if !is_valid(v, k, command.include) {
                return f64::NAN;
            }
            match v.values() {
                Values::Numeric(values) => values.get(k).unwrap_or(f64::NAN),
                Values::String(_) => f64::NAN,
            }
        };
        let raw: Vec<Vec<f64>> = variables
            .iter()
            .map(|v| (0..cases).map(|k| value(v, k)).collect())
            .collect();
        let complete: Vec<bool> = (0..cases)
            .map(|k| raw.iter().all(|col| !col[k].is_nan()))
            .collect();
        let p = names.len();
        let cols: Vec<&Vec<f64>> = names
            .iter()
            .map(|n| &raw[all.iter().position(|a| a == n).expect("from VARIABLES")])
            .collect();
        let total: f64 = w.iter().filter(|w| **w > 0.0).sum();
        // the values the matrix is built from, NaN where a case does not count
        let used: Vec<Vec<f64>> = match command.missing {
            FactorMissing::Listwise => cols
                .iter()
                .map(|col| {
                    (0..cases)
                        .map(|k| match complete[k] && w[k] > 0.0 {
                            true => col[k],
                            false => f64::NAN,
                        })
                        .collect()
                })
                .collect(),
            FactorMissing::Pairwise => cols
                .iter()
                .map(|col| {
                    (0..cases)
                        .map(|k| if w[k] > 0.0 { col[k] } else { f64::NAN })
                        .collect()
                })
                .collect(),
            FactorMissing::MeanSubstitution => cols
                .iter()
                .map(|col| {
                    let (mut sum, mut n) = (0.0, 0.0);
                    for k in (0..cases).filter(|&k| w[k] > 0.0 && !col[k].is_nan()) {
                        sum += w[k] * col[k];
                        n += w[k];
                    }
                    let mean = sum / n;
                    (0..cases)
                        .map(|k| match (w[k] > 0.0, col[k].is_nan()) {
                            (false, _) => f64::NAN,
                            (true, true) => mean,
                            (true, false) => col[k],
                        })
                        .collect()
                })
                .collect(),
        };
        // each variable on its own valid cases
        let own = |i: usize| -> (f64, f64, f64) {
            let (mut n, mut sum) = (0.0, 0.0);
            for k in (0..cases).filter(|&k| !used[i][k].is_nan()) {
                n += w[k];
                sum += w[k] * used[i][k];
            }
            let mean = sum / n;
            let ss: f64 = (0..cases)
                .filter(|&k| !used[i][k].is_nan())
                .map(|k| w[k] * (used[i][k] - mean).powi(2))
                .sum();
            (n, mean, (ss / (n - 1.0)).sqrt())
        };
        let stats: Vec<(f64, f64, f64)> = (0..p).map(own).collect();
        let mut pair_n = vec![vec![0.0; p]; p];
        let mut r = vec![vec![1.0; p]; p];
        let mut cov = vec![vec![0.0; p]; p];
        // variables valid in the same cases share n and their means over the common cases:
        // taken once per variable, in the same order, so the results stay the same
        let (class, valid) = crate::regression::valid_classes(&used);
        let alone: Vec<(f64, f64)> = (0..p)
            .map(|i| {
                let both = &valid[class[i]];
                let n: f64 = both.iter().map(|&k| w[k]).sum();
                (n, both.iter().map(|&k| w[k] * used[i][k]).sum::<f64>() / n)
            })
            .collect();
        for i in 0..p {
            for j in 0..=i {
                let shared;
                let (both, n, mi, mj) = if class[i] == class[j] {
                    (&valid[class[i]], alone[i].0, alone[i].1, alone[j].1)
                } else {
                    shared = (0..cases)
                        .filter(|&k| !used[i][k].is_nan() && !used[j][k].is_nan())
                        .collect::<Vec<usize>>();
                    let n: f64 = shared.iter().map(|&k| w[k]).sum();
                    let mi = shared.iter().map(|&k| w[k] * used[i][k]).sum::<f64>() / n;
                    let mj = shared.iter().map(|&k| w[k] * used[j][k]).sum::<f64>() / n;
                    (&shared, n, mi, mj)
                };
                let (mut sij, mut sii, mut sjj) = (0.0, 0.0, 0.0);
                for &k in both {
                    let (a, b) = (used[i][k] - mi, used[j][k] - mj);
                    sij += w[k] * a * b;
                    sii += w[k] * a * a;
                    sjj += w[k] * b * b;
                }
                pair_n[i][j] = n;
                pair_n[j][i] = n;
                cov[i][j] = sij / (n - 1.0);
                cov[j][i] = cov[i][j];
                if i != j {
                    r[i][j] = sij / (sii * sjj).sqrt();
                    r[j][i] = r[i][j];
                }
            }
        }
        let smallest = pair_n
            .iter()
            .flatten()
            .copied()
            .fold(f64::INFINITY, f64::min);
        if smallest.is_nan() || smallest <= 1.0 {
            return Err(Error(format!(
                "FACTOR: too few cases (N = {}) for the analysis",
                if smallest.is_finite() { smallest } else { 0.0 }
            )));
        }
        if let Some(i) = (0..p).find(|&i| stats[i].2.is_nan() || stats[i].2 <= 0.0) {
            return Err(Error(format!(
                "FACTOR: {} has no variance in the cases of the analysis",
                names[i]
            )));
        }
        let mean: Vec<f64> = stats.iter().map(|s| s.1).collect();
        let sd: Vec<f64> = stats.iter().map(|s| s.2).collect();
        let n: Vec<f64> = stats.iter().map(|s| s.0).collect();
        let missing = match command.missing {
            FactorMissing::Listwise => None,
            FactorMissing::Pairwise => Some(n.iter().map(|n| total - n).collect()),
            FactorMissing::MeanSubstitution => Some(
                cols.iter()
                    .map(|col| {
                        (0..cases)
                            .filter(|&k| w[k] > 0.0 && col[k].is_nan())
                            .map(|k| w[k])
                            .sum()
                    })
                    .collect(),
            ),
        };
        // scores for every case with the values it needs, selected or not
        let z = (0..cases)
            .map(|k| {
                (0..p)
                    .map(|i| {
                        let x = cols[i][k];
                        match (x.is_nan(), command.missing) {
                            (false, _) => Some((x - mean[i]) / sd[i]),
                            (true, FactorMissing::MeanSubstitution) => Some(0.0),
                            (true, _) => None,
                        }
                    })
                    .collect::<Option<Vec<f64>>>()
            })
            .collect();
        Ok(Input {
            names,
            r,
            cov: Some(cov),
            mean: Some(mean),
            sd,
            n: Some(n),
            missing,
            w: Some(smallest),
            pair_n: Some(pair_n),
            z: Some(z),
        })
    }

    /// The correlation matrix from matrix materials: ROWTYPE_, VARNAME_ and the variables.
    fn factor_matrix(
        &self,
        command: &Factor,
        kind: MatrixKind,
        source: Option<&String>,
    ) -> Result<Input, Error> {
        let file;
        let data: &Dataset = match source {
            None => self.active()?,
            Some(name) => {
                file = self.data_file(name)?;
                &file
            }
        };
        let column = |name: &str| -> Result<Variable, Error> {
            data.variables()
                .find(|v| v.name().eq_ignore_ascii_case(name) && v.is_string())
                .ok_or_else(|| {
                    Error(format!(
                        "FACTOR /MATRIX=IN needs the string variable {name} in the matrix file"
                    ))
                })
        };
        let (rowtype, varname) = (column("ROWTYPE_")?, column("VARNAME_")?);
        let all: Vec<String> = data
            .variables()
            .skip_while(|v| v.name() != varname.name())
            .skip(1)
            .filter(|v| !v.is_string())
            .map(|v| v.name().to_string())
            .collect();
        let names = match &command.analysis {
            Some(items) => analysis_names(&all, items)?,
            None => all.clone(),
        };
        let p = names.len();
        let text = |v: &Variable, k: usize| match v.values() {
            Values::String(s) => s.get(k).trim().to_uppercase(),
            Values::Numeric(_) => String::new(),
        };
        let number = |name: &str, k: usize| -> Option<f64> {
            match data.variable(name).ok()?.values() {
                Values::Numeric(v) => v.get(k),
                Values::String(_) => None,
            }
        };
        let mut n = None;
        let (mut mean, mut sd) = (None, None);
        let mut m = vec![vec![f64::NAN; p]; p];
        let wanted = match kind {
            MatrixKind::Correlation => "CORR",
            MatrixKind::Covariance => "COV",
        };
        for k in 0..data.case_count() {
            let row = || names.iter().map(|name| number(name, k)).collect::<Vec<_>>();
            match text(&rowtype, k).as_str() {
                "N" => n = Some(row()),
                "MEAN" => mean = Some(row()),
                "STDDEV" | "SD" => sd = Some(row()),
                t if t == wanted => {
                    let of = text(&varname, k);
                    if let Some(i) = names.iter().position(|n| n.to_uppercase() == of) {
                        for (j, value) in row().into_iter().enumerate() {
                            m[i][j] = value.unwrap_or(f64::NAN);
                        }
                    }
                }
                _ => {}
            }
        }
        // a triangle fills the other half
        for (i, j) in (0..p).flat_map(|i| (0..p).map(move |j| (i, j))) {
            if m[i][j].is_nan() {
                m[i][j] = m[j][i];
            }
        }
        if let Some((i, j)) = (0..p)
            .flat_map(|i| (0..p).map(move |j| (i, j)))
            .find(|&(i, j)| m[i][j].is_nan())
        {
            return Err(Error(format!(
                "FACTOR /MATRIX=IN: the matrix has no {wanted} value for {} with {}",
                names[i], names[j]
            )));
        }
        let (r, cov, sd) = match kind {
            MatrixKind::Correlation => {
                let sd: Option<Vec<f64>> = sd.and_then(|s| s.into_iter().collect::<Option<_>>());
                let cov = sd.as_ref().map(|sd| {
                    (0..p)
                        .map(|i| (0..p).map(|j| m[i][j] * sd[i] * sd[j]).collect())
                        .collect()
                });
                (m, cov, sd.unwrap_or_else(|| vec![f64::NAN; p]))
            }
            MatrixKind::Covariance => {
                let sd: Vec<f64> = (0..p).map(|i| m[i][i].sqrt()).collect();
                let r = (0..p)
                    .map(|i| (0..p).map(|j| m[i][j] / (sd[i] * sd[j])).collect())
                    .collect();
                (r, Some(m), sd)
            }
        };
        let n: Option<Vec<f64>> = n.and_then(|n| n.into_iter().collect());
        let w = n
            .as_ref()
            .map(|n| n.iter().copied().fold(f64::INFINITY, f64::min));
        Ok(Input {
            names,
            r,
            cov,
            mean: mean.and_then(|m| m.into_iter().collect()),
            sd,
            n,
            missing: None,
            w,
            pair_n: None,
            z: None,
        })
    }
}

/// The squared multiple correlations, or the largest absolute correlation of each variable
/// when R is singular.
fn initial_communalities(r: &Matrix, inv: Option<&Matrix>) -> Vec<f64> {
    let p = r.len();
    match inv {
        Some(inv) => (0..p).map(|i| 1.0 - 1.0 / inv[i][i]).collect(),
        None => (0..p)
            .map(|i| {
                (0..p)
                    .filter(|&j| j != i)
                    .map(|j| r[i][j].abs())
                    .fold(0.0, f64::max)
            })
            .collect(),
    }
}

/// Flips the sign of every column whose sum is negative.
fn reflect(loadings: &mut Matrix, m: usize) -> Vec<f64> {
    let signs: Vec<f64> = (0..m)
        .map(
            |j| match loadings.iter().map(|row| row[j]).sum::<f64>() < 0.0 {
                true => -1.0,
                false => 1.0,
            },
        )
        .collect();
    for row in loadings.iter_mut() {
        for j in 0..m {
            row[j] *= signs[j];
        }
    }
    signs
}

/// The first m eigenvectors times the square roots of the absolute eigenvalues.
fn eigen_loadings(pairs: &[(f64, Vec<f64>)], m: usize) -> Matrix {
    let p = pairs.len();
    (0..p)
        .map(|i| {
            (0..m)
                .map(|j| pairs[j].1[i] * pairs[j].0.abs().sqrt())
                .collect()
        })
        .collect()
}

struct Extracted {
    loadings: Matrix,
    initial: Vec<f64>,
    /// The eigenvalues of PC, for the regression scores.
    pc_values: Option<Vec<f64>>,
    /// Iterations used, for the methods that iterate.
    iterations: Option<usize>,
    /// Chi-square and degrees of freedom of ML and GLS.
    fit: Option<(f64, f64)>,
}

fn principal_axis(
    r: &Matrix,
    m: usize,
    mut h: Vec<f64>,
    c: &FactorCriteria,
    warnings: &mut Vec<String>,
) -> (Matrix, usize) {
    let p = r.len();
    let mut loadings = Vec::new();
    let mut iterations = 0;
    let mut change = f64::INFINITY;
    while iterations < c.iterate && change >= c.econverge {
        iterations += 1;
        let mut reduced = r.clone();
        for i in 0..p {
            reduced[i][i] = h[i];
        }
        let pairs = symmetric_eigen(&reduced);
        loadings = eigen_loadings(&pairs, m);
        let next = row_sums_of_squares(&loadings);
        change = (0..p).map(|i| (next[i] - h[i]).abs()).fold(0.0, f64::max);
        h = next;
    }
    if change >= c.econverge {
        warnings.push(format!(
            "FACTOR: the communalities did not converge in {} iterations (largest change {change:.4}); the last estimates are shown",
            c.iterate
        ));
    }
    (loadings, iterations)
}

/// Alpha factoring (Kaiser and Caffrey): the eigenvalues of H^-1/2 (R - I) H^-1/2 + I.
fn alpha_factoring(
    r: &Matrix,
    m: usize,
    mut h: Vec<f64>,
    c: &FactorCriteria,
    warnings: &mut Vec<String>,
) -> Result<(Matrix, usize), Error> {
    let p = r.len();
    let mut loadings = Vec::new();
    let mut iterations = 0;
    let mut change = f64::INFINITY;
    while iterations < c.iterate && change >= c.econverge {
        iterations += 1;
        if h.iter().any(|h| *h <= 0.0) {
            return Err(Error(
                "FACTOR: a communality reached 0; alpha factoring stops".into(),
            ));
        }
        // the manual prints H^1/2 here; only H^-1/2 makes the communality update hold
        let a: Matrix = (0..p)
            .map(|i| {
                (0..p)
                    .map(|j| match i == j {
                        true => 1.0,
                        false => r[i][j] / (h[i] * h[j]).sqrt(),
                    })
                    .collect()
            })
            .collect();
        let pairs = symmetric_eigen(&a);
        loadings = (0..p)
            .map(|i| {
                (0..m)
                    .map(|j| h[i].sqrt() * pairs[j].1[i] * pairs[j].0.abs().sqrt())
                    .collect()
            })
            .collect();
        let next = row_sums_of_squares(&loadings);
        change = (0..p).map(|i| (next[i] - h[i]).abs()).fold(0.0, f64::max);
        h = next;
    }
    if change >= c.econverge {
        warnings.push(format!(
            "FACTOR: the communalities did not converge in {} iterations (largest change {change:.4}); the last estimates are shown",
            c.iterate
        ));
    }
    Ok((loadings, iterations))
}

/// Image factoring (Kaiser): the eigenvalues of S^-1 R S^-1 with S^2 = diag(1 / r^ii).
fn image_factoring(r: &Matrix, inv: &Matrix, m: usize) -> (Matrix, usize) {
    let p = r.len();
    let s_inv: Vec<f64> = (0..p).map(|i| inv[i][i].sqrt()).collect();
    let a: Matrix = (0..p)
        .map(|i| (0..p).map(|j| s_inv[i] * r[i][j] * s_inv[j]).collect())
        .collect();
    let pairs = symmetric_eigen(&a);
    let m = m.min(pairs.iter().filter(|(v, _)| *v > 1.0).count());
    let loadings = (0..p)
        .map(|i| {
            (0..m)
                .map(|j| {
                    let g = pairs[j].0;
                    pairs[j].1[i] * (g - 1.0) / g.sqrt() / s_inv[i]
                })
                .collect()
        })
        .collect();
    (loadings, m)
}

#[derive(Clone, Copy, PartialEq)]
enum Fit {
    Uls,
    Gls,
    Ml,
}

struct Objective {
    f: f64,
    gradient: Vec<f64>,
    approximate: Matrix,
    pairs: Vec<(f64, Vec<f64>)>,
}

/// The fit function of ML, GLS or ULS at x, from the eigenvalues of psi R^-1 psi (ascending;
/// x the logarithm of the unique variances) or of R - psi^2 (descending; x is psi).
fn objective(fit: Fit, r: &Matrix, inv: &Matrix, m: usize, x: &[f64]) -> Objective {
    let p = r.len();
    let (a, ascending): (Matrix, bool) = match fit {
        Fit::Ml | Fit::Gls => {
            let psi: Vec<f64> = x.iter().map(|x| (x / 2.0).exp()).collect();
            let a = (0..p)
                .map(|i| (0..p).map(|j| psi[i] * inv[i][j] * psi[j]).collect())
                .collect();
            (a, true)
        }
        Fit::Uls => {
            let mut a = r.clone();
            for i in 0..p {
                a[i][i] -= x[i] * x[i];
            }
            (a, false)
        }
    };
    let mut pairs = symmetric_eigen(&a);
    if ascending {
        pairs.reverse();
    }
    let rest = &pairs[m..];
    let f = rest
        .iter()
        .map(|(g, _)| match fit {
            Fit::Ml => g.ln() + 1.0 / g - 1.0,
            Fit::Gls => (g - 1.0).powi(2) / 2.0,
            Fit::Uls => g * g / 2.0,
        })
        .sum();
    let gradient = (0..p)
        .map(|i| {
            rest.iter()
                .map(|(g, w)| {
                    let w2 = w[i] * w[i];
                    match fit {
                        Fit::Ml => (1.0 - 1.0 / g) * w2,
                        Fit::Gls => (g * g - g) * w2,
                        Fit::Uls => -2.0 * x[i] * g * w2,
                    }
                })
                .sum()
        })
        .collect();
    let approximate = (0..p)
        .map(|i| {
            (0..p)
                .map(|j| {
                    let s: f64 = rest.iter().map(|(_, w)| w[i] * w[j]).sum();
                    match fit {
                        Fit::Uls => 4.0 * x[i] * x[j] * s * s,
                        _ => s * s,
                    }
                })
                .collect()
        })
        .collect();
    Objective {
        f,
        gradient,
        approximate,
        pairs,
    }
}

/// A second derivative below 0.05 marks a Heywood variable: its row and column become those
/// of the identity.
fn heywood(h: &mut Matrix) {
    let p = h.len();
    let heywood: Vec<usize> = (0..p).filter(|&i| h[i][i] < 0.05).collect();
    for i in heywood {
        for row in h.iter_mut() {
            row[i] = 0.0;
        }
        h[i] = vec![0.0; p];
        h[i][i] = 1.0;
    }
}

fn newton_step(h: &Matrix, g: &[f64]) -> Option<Vec<f64>> {
    let (inv, _) = inverse(h)?;
    Some(
        inv.iter()
            .map(|row| row.iter().zip(g).map(|(a, b)| a * b).sum())
            .collect(),
    )
}

/// ML, GLS and ULS: Newton-Raphson on x. The exact second derivatives are taken as central
/// differences of the exact first derivatives.
fn least_squares(
    fit: Fit,
    r: &Matrix,
    inv: &Matrix,
    m: usize,
    w: Option<f64>,
    c: &FactorCriteria,
    warnings: &mut Vec<String>,
) -> (Matrix, usize, Option<(f64, f64)>) {
    let p = r.len();
    let pf = p as f64;
    let start = 1.0 - m as f64 / (2.0 * pf);
    let mut x: Vec<f64> = (0..p)
        .map(|i| match fit {
            Fit::Uls => (start / inv[i][i]).sqrt(),
            _ => (start / inv[i][i]).ln(),
        })
        .collect();
    let mut now = objective(fit, r, inv, m, &x);
    let mut iterations = 0;
    let mut converged = false;
    while iterations < c.iterate {
        iterations += 1;
        let approximate = || {
            let mut h = now.approximate.clone();
            heywood(&mut h);
            newton_step(&h, &now.gradient)
        };
        let exact = (iterations > 1).then(|| {
            let step = 1e-5;
            let mut h = vec![vec![0.0; p]; p];
            for j in 0..p {
                let mut up = x.clone();
                let mut down = x.clone();
                up[j] += step;
                down[j] -= step;
                let (gu, gd) = (
                    objective(fit, r, inv, m, &up).gradient,
                    objective(fit, r, inv, m, &down).gradient,
                );
                for i in 0..p {
                    h[i][j] = (gu[i] - gd[i]) / (2.0 * step);
                }
            }
            for (i, j) in (0..p).flat_map(|i| (0..i).map(move |j| (i, j))) {
                let mean = (h[i][j] + h[j][i]) / 2.0;
                h[i][j] = mean;
                h[j][i] = mean;
            }
            heywood(&mut h);
            h
        });
        let d = match exact {
            Some(h) if positive_definite(&h) => match newton_step(&h, &now.gradient) {
                Some(d) if d.iter().all(|d| d.abs() <= 0.1) => Some(d),
                _ => approximate(),
            },
            _ => approximate(),
        };
        let Some(mut d) = d else {
            break;
        };
        let largest = d.iter().fold(0.0_f64, |a, d| a.max(d.abs()));
        let mut accepted = None;
        for _ in 0..=25 {
            let next: Vec<f64> = x.iter().zip(&d).map(|(x, d)| x - d).collect();
            let candidate = objective(fit, r, inv, m, &next);
            if candidate.f < now.f {
                accepted = Some((next, candidate));
                break;
            }
            d.iter_mut().for_each(|d| *d /= 2.0);
        }
        match accepted {
            Some((next, candidate)) => {
                x = next;
                now = candidate;
            }
            None => {
                converged = largest < c.econverge;
                break;
            }
        }
        if largest < c.econverge {
            converged = true;
            break;
        }
    }
    if !converged {
        warnings.push(format!(
            "FACTOR: the extraction did not converge in {} iterations; the last estimates are shown",
            c.iterate
        ));
    }
    let loadings = match fit {
        Fit::Uls => eigen_loadings(&now.pairs, m)
            .into_iter()
            .map(|row| row.into_iter().collect())
            .collect::<Matrix>(),
        _ => {
            let psi: Vec<f64> = x.iter().map(|x| (x / 2.0).exp()).collect();
            (0..p)
                .map(|i| {
                    (0..m)
                        .map(|j| {
                            let (g, w) = &now.pairs[j];
                            psi[i] * w[i] * (1.0 / g - 1.0).max(0.0).sqrt()
                        })
                        .collect()
                })
                .collect()
        }
    };
    let loadings = match fit {
        // the eigenvalues of R - psi^2 that are negative give no loading
        Fit::Uls => (0..p)
            .map(|i| {
                (0..m)
                    .map(|j| match now.pairs[j].0 > 0.0 {
                        true => loadings[i][j],
                        false => 0.0,
                    })
                    .collect()
            })
            .collect(),
        _ => loadings,
    };
    let test = match (fit, w) {
        (Fit::Ml | Fit::Gls, Some(w)) => {
            let mf = m as f64;
            let df = ((pf - mf).powi(2) - pf - mf) / 2.0;
            let chi = (w - 1.0 - (2.0 * pf + 5.0) / 6.0 - 2.0 * mf / 3.0) * now.f;
            Some((chi, df))
        }
        _ => None,
    };
    (loadings, iterations, test)
}

struct Rotated {
    /// The pattern matrix.
    pattern: Matrix,
    /// The factor correlations of an oblique rotation.
    phi: Option<Matrix>,
    /// The transformation matrix of an orthogonal rotation.
    transform: Option<Matrix>,
    iterations: usize,
}

/// Kaiser normalization: each row divided by the square root of its communality.
fn normalized(a: &Matrix, kaiser: bool) -> (Matrix, Vec<f64>) {
    let h: Vec<f64> = row_sums_of_squares(a)
        .into_iter()
        .map(|h| if kaiser && h > 0.0 { h.sqrt() } else { 1.0 })
        .collect();
    let l = a
        .iter()
        .zip(&h)
        .map(|(row, h)| row.iter().map(|v| v / h).collect())
        .collect();
    (l, h)
}

fn orthogonal(
    a: &Matrix,
    kind: Rotation,
    c: &FactorCriteria,
    warnings: &mut Vec<String>,
) -> Rotated {
    let m = a[0].len();
    let n = a.len() as f64;
    let (mut l, h) = normalized(a, c.kaiser);
    let mut t = identity(m);
    let simplicity = |l: &Matrix| -> f64 {
        (0..m)
            .map(|j| {
                let s2: f64 = l.iter().map(|row| row[j].powi(2)).sum();
                let s4: f64 = l.iter().map(|row| row[j].powi(4)).sum();
                (n * s4 - s2 * s2) / (n * n)
            })
            .sum()
    };
    let mut last = simplicity(&l);
    let mut iterations = 0;
    let mut converged = false;
    while iterations < c.iterate {
        iterations += 1;
        let mut turn: f64 = 0.0;
        for j in 0..m {
            for k in j + 1..m {
                let (mut sa, mut sb, mut sc, mut sd) = (0.0, 0.0, 0.0, 0.0);
                for row in &l {
                    let u = row[j] * row[j] - row[k] * row[k];
                    let v = 2.0 * row[j] * row[k];
                    sa += u;
                    sb += v;
                    sc += u * u - v * v;
                    sd += 2.0 * u * v;
                }
                let mf = m as f64;
                let (x, y) = match kind {
                    Rotation::Equamax => (
                        sd - mf * sa * sb / n,
                        sc - mf * (sa * sa - sb * sb) / (2.0 * n),
                    ),
                    Rotation::Quartimax => (sd, sc),
                    _ => (sd - 2.0 * sa * sb / n, sc - (sa * sa - sb * sb) / n),
                };
                let angle = x.atan2(y) / 4.0;
                turn = turn.max(angle.sin().abs());
                if angle.sin().abs() <= 1e-15 {
                    continue;
                }
                let (cos, sin) = (angle.cos(), angle.sin());
                for row in l.iter_mut().chain(t.iter_mut()) {
                    let (a, b) = (row[j], row[k]);
                    row[j] = a * cos + b * sin;
                    row[k] = -a * sin + b * cos;
                }
            }
        }
        let now = simplicity(&l);
        // the manual stops on the change of the criterion alone, which can leave the loadings
        // unsettled in the third decimal; a sweep must also turn no pair by more than RCONVERGE
        if (now - last).abs() <= c.rconverge && turn <= c.rconverge {
            converged = true;
            break;
        }
        last = now;
    }
    if !converged {
        warnings.push(format!(
            "FACTOR: the rotation did not converge in {} iterations; the last estimates are shown",
            c.iterate
        ));
    }
    let mut pattern: Matrix = l
        .iter()
        .zip(&h)
        .map(|(row, h)| row.iter().map(|v| v * h).collect())
        .collect();
    let signs = reflect(&mut pattern, m);
    for row in t.iter_mut() {
        for j in 0..m {
            row[j] *= signs[j];
        }
    }
    let ss = column_sums_of_squares(&pattern, m);
    let mut order: Vec<usize> = (0..m).collect();
    order.sort_by(|&a, &b| ss[b].total_cmp(&ss[a]));
    let permute = |x: &Matrix| -> Matrix {
        x.iter()
            .map(|row| order.iter().map(|&j| row[j]).collect())
            .collect()
    };
    Rotated {
        pattern: permute(&pattern),
        phi: None,
        transform: Some(permute(&t)),
        iterations,
    }
}

/// The real roots of c3 a^3 + c2 a^2 + c1 a + c0.
fn cubic_roots(c3: f64, c2: f64, c1: f64, c0: f64) -> Vec<f64> {
    let scale = c3.abs().max(c2.abs()).max(c1.abs()).max(c0.abs());
    if scale == 0.0 {
        return Vec::new();
    }
    if c3.abs() <= 1e-12 * scale {
        if c2.abs() <= 1e-12 * scale {
            return if c1 != 0.0 {
                vec![-c0 / c1]
            } else {
                Vec::new()
            };
        }
        let disc = c1 * c1 - 4.0 * c2 * c0;
        if disc < 0.0 {
            return Vec::new();
        }
        return vec![
            (-c1 + disc.sqrt()) / (2.0 * c2),
            (-c1 - disc.sqrt()) / (2.0 * c2),
        ];
    }
    let (a, b, c) = (c2 / c3, c1 / c3, c0 / c3);
    let q = (a * a - 3.0 * b) / 9.0;
    let r = (2.0 * a * a * a - 9.0 * a * b + 27.0 * c) / 54.0;
    if r * r < q * q * q {
        let theta = (r / (q * q * q).sqrt()).clamp(-1.0, 1.0).acos();
        let s = -2.0 * q.sqrt();
        (0..3)
            .map(|k| s * ((theta + 2.0 * std::f64::consts::PI * k as f64) / 3.0).cos() - a / 3.0)
            .collect()
    } else {
        let big = -r.signum() * (r.abs() + (r * r - q * q * q).sqrt()).cbrt();
        let small = if big != 0.0 { q / big } else { 0.0 };
        vec![big + small - a / 3.0]
    }
}

/// Direct oblimin (Jennrich and Sampson): each factor pair (p, q) in turn takes the step that
/// minimizes the criterion, which is a quartic in the step, as in the manual's cubic for it.
fn oblimin(a: &Matrix, delta: f64, c: &FactorCriteria, warnings: &mut Vec<String>) -> Rotated {
    let m = a[0].len();
    let n = a.len() as f64;
    let (mut l, h) = normalized(a, c.kaiser);
    let mut phi = identity(m);
    let criterion = |l: &Matrix| -> f64 {
        let rows: f64 = l
            .iter()
            .map(|row| {
                let s: f64 = row.iter().map(|v| v * v).sum();
                let q: f64 = row.iter().map(|v| v.powi(4)).sum();
                s * s - q
            })
            .sum();
        let u = column_sums_of_squares(l, m);
        let total: f64 = u.iter().sum();
        let squares: f64 = u.iter().map(|u| u * u).sum();
        rows - delta / n * (total * total - squares)
    };
    let step = |l: &Matrix, phi: &Matrix, p: usize, q: usize, s: f64| -> Matrix {
        let t1 = (1.0 + 2.0 * s * phi[p][q] + s * s).sqrt();
        l.iter()
            .map(|row| {
                let mut row = row.clone();
                row[q] -= s * row[p];
                row[p] *= t1;
                row
            })
            .collect()
    };
    let first = criterion(&l);
    let mut last = first;
    let mut iterations = 0;
    let mut converged = false;
    while iterations < c.iterate {
        iterations += 1;
        for p in 0..m {
            for q in (0..m).filter(|&q| q != p) {
                // the criterion is a quartic in the step: fit it at five points
                let points = [-2.0, -1.0, 0.0, 1.0, 2.0];
                let values: Vec<f64> = points
                    .iter()
                    .map(|&s| criterion(&step(&l, &phi, p, q, s)))
                    .collect();
                let vandermonde: Matrix = points
                    .iter()
                    .map(|&s| (0..5).map(|k| f64::powi(s, k)).collect())
                    .collect();
                let Some((inv, _)) = inverse(&vandermonde) else {
                    continue;
                };
                let k: Vec<f64> = inv
                    .iter()
                    .map(|row| row.iter().zip(&values).map(|(a, b)| a * b).sum())
                    .collect();
                let quartic = |s: f64| k[0] + s * (k[1] + s * (k[2] + s * (k[3] + s * k[4])));
                let best = cubic_roots(4.0 * k[4], 3.0 * k[3], 2.0 * k[2], k[1])
                    .into_iter()
                    .filter(|s| s.is_finite())
                    .fold((0.0, quartic(0.0)), |best, s| match quartic(s) < best.1 {
                        true => (s, quartic(s)),
                        false => best,
                    })
                    .0;
                if best == 0.0 {
                    continue;
                }
                let t1 = (1.0 + 2.0 * best * phi[p][q] + best * best).sqrt();
                l = step(&l, &phi, p, q, best);
                let column: Vec<f64> = (0..m)
                    .map(|i| phi[i][p] / t1 + best / t1 * phi[i][q])
                    .collect();
                for i in (0..m).filter(|&i| i != p) {
                    phi[i][p] = column[i];
                    phi[p][i] = column[i];
                }
                phi[p][p] = 1.0;
            }
        }
        let now = criterion(&l);
        if (now - last).abs() < first.abs() * c.rconverge {
            converged = true;
            break;
        }
        last = now;
    }
    if !converged {
        warnings.push(format!(
            "FACTOR: the rotation did not converge in {} iterations; the last estimates are shown",
            c.iterate
        ));
    }
    let mut pattern: Matrix = l
        .iter()
        .zip(&h)
        .map(|(row, h)| row.iter().map(|v| v * h).collect())
        .collect();
    let signs = reflect(&mut pattern, m);
    for i in 0..m {
        for j in 0..m {
            phi[i][j] *= signs[i] * signs[j];
        }
    }
    Rotated {
        pattern,
        phi: Some(phi),
        transform: None,
        iterations,
    }
}

/// Promax (Hendrickson and White) from the varimax solution.
fn promax(a: &Matrix, kappa: f64, c: &FactorCriteria, warnings: &mut Vec<String>) -> Rotated {
    let varimax = orthogonal(a, Rotation::Varimax, c, warnings);
    let lr = &varimax.pattern;
    let m = lr[0].len();
    let target: Matrix = lr
        .iter()
        .map(|row| {
            let h = row.iter().map(|v| v * v).sum::<f64>().sqrt();
            row.iter()
                .map(|v| match *v == 0.0 {
                    true => 0.0,
                    false => (v / h).abs().powf(kappa + 1.0) * h / v,
                })
                .collect()
        })
        .collect();
    let lt = transpose(lr);
    let Some((ltl_inv, _)) = inverse(&multiply(&lt, lr)) else {
        return varimax;
    };
    let l = multiply(&ltl_inv, &multiply(&lt, &target));
    let ltl = multiply(&transpose(&l), &l);
    let q: Matrix = l
        .iter()
        .map(|row| (0..m).map(|j| row[j] / ltl[j][j].sqrt()).collect())
        .collect();
    let Some((qtq_inv, _)) = inverse(&multiply(&transpose(&q), &q)) else {
        return varimax;
    };
    let cdiag: Vec<f64> = (0..m).map(|j| 1.0 / qtq_inv[j][j].sqrt()).collect();
    let lq = multiply(lr, &q);
    let mut pattern: Matrix = lq
        .iter()
        .map(|row| (0..m).map(|j| row[j] / cdiag[j]).collect())
        .collect();
    let mut phi: Matrix = (0..m)
        .map(|i| {
            (0..m)
                .map(|j| cdiag[i] * qtq_inv[i][j] * cdiag[j])
                .collect()
        })
        .collect();
    let signs = reflect(&mut pattern, m);
    for i in 0..m {
        for j in 0..m {
            phi[i][j] *= signs[i] * signs[j];
        }
    }
    Rotated {
        pattern,
        phi: Some(phi),
        transform: None,
        iterations: varimax.iterations,
    }
}

/// The rows of a loading matrix grouped by the factor of their largest absolute loading, each
/// group in descending order of that loading.
fn sorted_rows(l: &Matrix) -> Vec<usize> {
    let key = |i: usize| -> (usize, f64) {
        let row = &l[i];
        let j = (0..row.len())
            .max_by(|&a, &b| row[a].abs().total_cmp(&row[b].abs()))
            .unwrap_or(0);
        (j, row.get(j).map_or(0.0, |v| v.abs()))
    };
    let mut order: Vec<usize> = (0..l.len()).collect();
    order.sort_by(|&a, &b| {
        let (ka, kb) = (key(a), key(b));
        ka.0.cmp(&kb.0).then(kb.1.total_cmp(&ka.1))
    });
    order
}

/// What the plots of FACTOR show.
struct FactorSolution<'a> {
    /// "Component" or "Factor".
    word: &'a str,
    names: &'a [String],
    /// The eigenvalues of the correlation matrix, in descending order.
    eigenvalues: &'a [f64],
    /// The loadings plotted: the rotated pattern, else the extracted loadings; None when
    /// nothing was extracted.
    pattern: Option<&'a Matrix>,
    rotated: bool,
}

/// The scree plot (EIGEN): every eigenvalue of the correlation matrix against its number. The
/// loading plots (ROTATION): every variable as a point at its loadings, in the rotated space
/// when there is a rotation. Without pairs: the first two factors, with the third as depth
/// when there are three or more; nothing with one factor.
fn factor_plots(
    plot: &statscraft_syntax::FactorPlot,
    s: FactorSolution,
    warnings: &mut Vec<String>,
) -> Vec<crate::Chart> {
    use crate::chart::{Axis, Category, ChartKind, Series, Statistic};
    use crate::plot::{Plot, Point, PointGroup};
    let mut charts = Vec::new();
    if plot.eigen {
        let categories = (1..=s.eigenvalues.len())
            .map(|k| Category {
                label: k.to_string(),
                value: Cell::Number(k as f64),
                missing: false,
                bin: None,
            })
            .collect();
        let series = vec![Series {
            name: "Eigenvalue".into(),
            statistic: Statistic::Value,
            variable: None,
            value: Cell::Empty,
            values: s.eigenvalues.to_vec(),
        }];
        charts.push(crate::Chart::lines(
            "Scree Plot".into(),
            Axis::titled(format!("{} Number", s.word)),
            Axis::titled("Eigenvalue"),
            categories,
            series,
        ));
    }
    let (Some(pairs), Some(pattern)) = (&plot.rotation, s.pattern) else {
        return charts;
    };
    let m = pattern.first().map_or(0, Vec::len);
    if m < 2 {
        warnings.push(format!(
            "FACTOR: one {} only, so there is no loading plot",
            s.word.to_lowercase()
        ));
        return charts;
    }
    let mut wanted: Vec<(usize, usize, Option<usize>)> =
        pairs.iter().map(|&(a, b)| (a, b, None)).collect();
    if wanted.is_empty() {
        wanted.push((1, 2, (m > 2).then_some(3)));
    }
    let space = match s.rotated {
        true => format!("Rotated {} Plot", s.word),
        false => format!("{} Plot", s.word),
    };
    for (a, b, c) in wanted {
        if a > m || b > m {
            warnings.push(format!(
                "FACTOR: no loading plot of {} {a} and {b}: the solution has {m}",
                s.word.to_lowercase()
            ));
            continue;
        }
        let points = (0..s.names.len())
            .map(|i| Point {
                x: pattern[i][a - 1],
                y: pattern[i][b - 1],
                z: c.map(|c| pattern[i][c - 1]),
                label: Some(s.names[i].clone()),
            })
            .collect();
        let axis = |k: usize| {
            let mut axis = Axis::titled(format!("{} {k}", s.word));
            axis.format = "F8.3".into();
            axis
        };
        charts.push(crate::Chart::plot(
            ChartKind::Scatter,
            space.clone(),
            axis(a),
            axis(b),
            Plot::Points {
                groups: vec![PointGroup {
                    name: "Loadings".into(),
                    points,
                }],
                lines: Vec::new(),
                z_axis: c.map(axis),
            },
        ));
    }
    charts
}

fn label(names: &[String], i: usize) -> Cell {
    text(names[i].clone())
}

fn table(
    title: &str,
    row_headers: usize,
    groups: &[(&str, Vec<String>)],
    rows: Vec<Vec<Cell>>,
) -> Table {
    let mut columns = Vec::new();
    let mut column_groups = Vec::new();
    for _ in 0..row_headers {
        columns.push(String::new());
        column_groups.push(Vec::new());
    }
    for (group, names) in groups {
        for name in names {
            columns.push(name.clone());
            column_groups.push(match group.is_empty() {
                true => Vec::new(),
                false => vec![group.to_string()],
            });
        }
    }
    Table {
        title: title.into(),
        row_headers,
        columns,
        column_groups,
        rows,
        ..Table::default()
    }
}

fn plain(names: &[&str]) -> (&'static str, Vec<String>) {
    ("", names.iter().map(|s| s.to_string()).collect())
}

/// A matrix with one row per variable and one column per factor.
fn loading_table(
    title: &str,
    word: &str,
    names: &[String],
    values: &Matrix,
    sort: bool,
    blank: Option<f64>,
    caption: String,
) -> Table {
    let m = values.first().map_or(0, Vec::len);
    let order: Vec<usize> = match sort {
        true => sorted_rows(values),
        false => (0..values.len()).collect(),
    };
    let rows = order
        .iter()
        .map(|&i| {
            let mut row = vec![label(names, i)];
            row.extend(values[i].iter().map(|v| match blank {
                Some(b) if v.abs() < b => Cell::Empty,
                _ => number(Some(*v)),
            }));
            row
        })
        .collect();
    let mut t = table(
        title,
        1,
        &[(word, (1..=m).map(|j| j.to_string()).collect())],
        rows,
    );
    t.caption = Some(caption);
    t
}

/// A square matrix with one row and column per variable, in blocks under `blocks` labels.
fn square_table(title: &str, names: &[String], blocks: &[(&str, Matrix, bool)]) -> Table {
    let rows = blocks
        .iter()
        .flat_map(|(block, values, blank_diagonal)| {
            (0..names.len()).map(move |i| {
                let mut row = Vec::new();
                if !block.is_empty() {
                    row.push(if i == 0 { text(*block) } else { Cell::Empty });
                }
                row.push(label(names, i));
                row.extend(values[i].iter().enumerate().map(|(j, v)| {
                    match i == j && *blank_diagonal {
                        true => Cell::Empty,
                        false => number(Some(*v)),
                    }
                }));
                row
            })
        })
        .collect();
    let headers = if blocks.iter().any(|b| !b.0.is_empty()) {
        2
    } else {
        1
    };
    table(title, headers, &[("", names.to_vec())], rows)
}

fn method_name(e: Extraction) -> &'static str {
    match e {
        Extraction::PrincipalComponents => "Principal Component Analysis",
        Extraction::PrincipalAxis => "Principal Axis Factoring",
        Extraction::Alpha => "Alpha Factoring",
        Extraction::Image => "Image Factoring",
        Extraction::UnweightedLeastSquares => "Unweighted Least Squares",
        Extraction::GeneralizedLeastSquares => "Generalized Least Squares",
        Extraction::MaximumLikelihood => "Maximum Likelihood",
    }
}

fn rotation_name(r: Rotation, kaiser: bool) -> String {
    let name = match r {
        Rotation::Varimax => "Varimax",
        Rotation::Equamax => "Equamax",
        Rotation::Quartimax => "Quartimax",
        Rotation::Oblimin(_) => "Oblimin",
        Rotation::Promax(_) => "Promax",
        Rotation::None => "None",
    };
    match kaiser {
        true => format!("{name} with Kaiser Normalization"),
        false => name.to_string(),
    }
}

impl Engine {
    pub(crate) fn factor(&mut self, command: &Factor) -> Result<Vec<Table>, Error> {
        let mut warnings: Vec<String> = command
            .ignored
            .iter()
            .map(|s| format!("FACTOR: {s} is ignored, since a later subcommand cancels it"))
            .collect();
        let input = match &command.matrix_in {
            None => self.factor_data(command)?,
            Some((kind, source)) => {
                if command.save.is_some() {
                    return Err(Error(
                        "FACTOR /SAVE needs the cases; it cannot follow MATRIX=IN".into(),
                    ));
                }
                self.factor_matrix(command, *kind, source.as_ref())?
            }
        };
        // the variables as tables show them (SET TVARS)
        let shown = self.shown();
        let texts: Vec<String> = match self.dataset.as_ref() {
            Some(data) => input.names.iter().map(|n| shown.named(data, n)).collect(),
            None => input.names.clone(),
        };
        let names = &texts;
        let p = names.len();
        if p < 2 {
            return Err(Error("FACTOR needs at least two variables".into()));
        }
        let r = &input.r;
        let print = command.print;
        let pc = command.extraction == Extraction::PrincipalComponents;
        let word = if pc { "Component" } else { "Factor" };
        let inv = inverse(r);
        let det = match &inv {
            Some((_, det)) => *det,
            None => 0.0,
        };
        let inv = inv.map(|(inv, _)| inv);
        let mut tables = Vec::new();

        if print.univariate {
            match (&input.mean, &input.n) {
                (Some(mean), Some(n)) => {
                    let mut columns = vec!["Mean", "Std. Deviation", "Analysis N"];
                    if input.missing.is_some() {
                        columns.push("Missing N");
                    }
                    let rows = (0..p)
                        .map(|i| {
                            let mut row = vec![
                                label(names, i),
                                number(Some(mean[i])),
                                number(Some(input.sd[i])),
                                Cell::Number(n[i]),
                            ];
                            if let Some(missing) = &input.missing {
                                row.push(Cell::Number(missing[i]));
                            }
                            row
                        })
                        .collect();
                    tables.push(table("Descriptive Statistics", 1, &[plain(&columns)], rows));
                }
                _ => warnings
                    .push("FACTOR: the matrix file has no means or counts for UNIVARIATE".into()),
            }
        }
        if print.correlation || print.sig || print.det {
            let mut blocks = Vec::new();
            if print.correlation {
                blocks.push(("Correlation", r.clone(), false));
            }
            match (print.sig, input.w) {
                (true, Some(w)) => {
                    let sig = (0..p)
                        .map(|i| {
                            (0..p)
                                .map(|j| {
                                    let n = input.pair_n.as_ref().map_or(w, |n| n[i][j]);
                                    let rr = r[i][j];
                                    let t = rr * ((n - 2.0) / (1.0 - rr * rr)).sqrt();
                                    t_p(t, n - 2.0) / 2.0
                                })
                                .collect()
                        })
                        .collect();
                    blocks.push(("Sig. (1-tailed)", sig, true));
                }
                (true, None) => {
                    warnings.push("FACTOR: the matrix file has no N for SIG".into());
                }
                _ => {}
            }
            let caption = match det.abs() >= 0.001 {
                true => format!("Determinant = {det:.3}"),
                false => format!("Determinant = {det:.3E}"),
            };
            if blocks.is_empty() {
                let mut t = table(
                    "Correlation Matrix",
                    0,
                    &[plain(&["Determinant"])],
                    vec![vec![number(Some(det))]],
                );
                t.caption = None;
                tables.push(t);
            } else {
                let mut t = square_table("Correlation Matrix", names, &blocks);
                if print.det {
                    t.caption = Some(caption);
                }
                tables.push(t);
            }
        }
        if print.covariance {
            match &input.cov {
                Some(cov) => tables.push(square_table(
                    "Covariance Matrix",
                    names,
                    &[("", cov.clone(), false)],
                )),
                None => warnings.push(
                    "FACTOR: a correlation matrix without standard deviations has no covariances"
                        .into(),
                ),
            }
        }
        if print.inv {
            match &inv {
                Some(inv) => tables.push(square_table(
                    "Inverse of Correlation Matrix",
                    names,
                    &[("", inv.clone(), false)],
                )),
                None => warnings
                    .push("FACTOR: the correlation matrix is singular and has no inverse".into()),
            }
        }
        if print.kmo || print.aic {
            match &inv {
                None => warnings.push(
                    "FACTOR: the correlation matrix is singular, so KMO and the anti-image matrices cannot be computed"
                        .into(),
                ),
                Some(inv) => {
                    let partial: Matrix = (0..p)
                        .map(|i| {
                            (0..p)
                                .map(|j| -inv[i][j] / (inv[i][i] * inv[j][j]).sqrt())
                                .collect()
                        })
                        .collect();
                    let off = |i: usize, f: &dyn Fn(usize, usize) -> f64| -> f64 {
                        (0..p).filter(|&j| j != i).map(|j| f(i, j).powi(2)).sum()
                    };
                    let rr = |i: usize, j: usize| r[i][j];
                    let aa = |i: usize, j: usize| partial[i][j];
                    let msa: Vec<f64> = (0..p)
                        .map(|i| off(i, &rr) / (off(i, &rr) + off(i, &aa)))
                        .collect();
                    if print.kmo {
                        let (sr, sa): (f64, f64) =
                            (0..p).fold((0.0, 0.0), |(x, y), i| (x + off(i, &rr), y + off(i, &aa)));
                        let kmo = sr / (sr + sa);
                        let pf = p as f64;
                        let df = pf * (pf - 1.0) / 2.0;
                        let (chi, sig) = match input.w {
                            Some(w) => {
                                let chi = -(w - 1.0 - (2.0 * pf + 5.0) / 6.0) * det.ln();
                                (number(Some(chi)), number(Some(chi_square_p(chi, df))))
                            }
                            None => (Cell::Empty, Cell::Empty),
                        };
                        let rows = vec![
                            vec![
                                text("Kaiser-Meyer-Olkin Measure of Sampling Adequacy."),
                                Cell::Empty,
                                number(Some(kmo)),
                            ],
                            vec![
                                text("Bartlett's Test of Sphericity"),
                                text("Approx. Chi-Square"),
                                chi,
                            ],
                            vec![Cell::Empty, text("df"), Cell::Number(df)],
                            vec![Cell::Empty, text("Sig."), sig],
                        ];
                        tables.push(table("KMO and Bartlett's Test", 2, &[plain(&["Value"])], rows));
                    }
                    if print.aic {
                        let cov: Matrix = (0..p)
                            .map(|i| (0..p).map(|j| inv[i][j] / (inv[i][i] * inv[j][j])).collect())
                            .collect();
                        // the anti-image correlation is r^ij / sqrt(r^ii r^jj), minus the partial correlation
                        let mut corr: Matrix = partial.iter().map(|row| row.iter().map(|v| -v).collect()).collect();
                        for i in 0..p {
                            corr[i][i] = msa[i];
                        }
                        let mut t = square_table(
                            "Anti-image Matrices",
                            names,
                            &[
                                ("Anti-image Covariance", cov, false),
                                ("Anti-image Correlation", corr, false),
                            ],
                        );
                        t.caption = Some(
                            "The diagonal of the anti-image correlations is the measure of sampling adequacy (MSA) of each variable."
                                .into(),
                        );
                        tables.push(t);
                    }
                }
            }
        }

        // the extraction
        let initial_pairs = symmetric_eigen(r);
        let eigenvalues: Vec<f64> = initial_pairs.iter().map(|p| p.0).collect();
        let c = &command.extraction_criteria;
        let mut m = c
            .factors
            .unwrap_or_else(|| eigenvalues.iter().filter(|v| **v > c.min_eigen).count());
        let limit = match command.extraction {
            Extraction::PrincipalComponents | Extraction::Image => p,
            _ => p - 1,
        };
        if m > limit {
            warnings.push(format!(
                "FACTOR: {m} factors asked for, but {limit} is the most {} can extract from {p} variables",
                method_name(command.extraction)
            ));
            m = limit;
        }
        let needs_inverse = !matches!(
            command.extraction,
            Extraction::PrincipalComponents | Extraction::PrincipalAxis | Extraction::Alpha
        );
        if needs_inverse && inv.is_none() {
            return Err(Error(format!(
                "FACTOR: the correlation matrix is singular, so {} cannot be computed",
                method_name(command.extraction)
            )));
        }
        let smc = initial_communalities(r, inv.as_ref());
        let extracted = if m == 0 {
            warnings.push(format!(
                "FACTOR: no eigenvalue is above MINEIGEN({}), so no factor is extracted",
                c.min_eigen
            ));
            None
        } else {
            Some(match command.extraction {
                Extraction::PrincipalComponents => Extracted {
                    loadings: eigen_loadings(&initial_pairs, m),
                    initial: vec![1.0; p],
                    pc_values: Some(eigenvalues[..m].to_vec()),
                    iterations: None,
                    fit: None,
                },
                Extraction::PrincipalAxis => {
                    let initial = match &command.diagonal {
                        Some(values) if values.len() == p => values.clone(),
                        Some(values) => {
                            return Err(Error(format!(
                                "FACTOR /DIAGONAL gives {} values for {p} variables",
                                values.len()
                            )));
                        }
                        None => smc.clone(),
                    };
                    let (loadings, iterations) =
                        principal_axis(r, m, initial.clone(), c, &mut warnings);
                    Extracted {
                        loadings,
                        initial,
                        pc_values: None,
                        iterations: Some(iterations),
                        fit: None,
                    }
                }
                Extraction::Alpha => {
                    let ok = det.abs() >= 1e-8 && smc.iter().all(|h| (0.0..=1.0).contains(h));
                    let initial = match ok {
                        true => smc.clone(),
                        false => initial_communalities(r, None),
                    };
                    let (loadings, iterations) =
                        alpha_factoring(r, m, initial.clone(), c, &mut warnings)?;
                    Extracted {
                        loadings,
                        initial,
                        pc_values: None,
                        iterations: Some(iterations),
                        fit: None,
                    }
                }
                Extraction::Image => {
                    let (loadings, kept) = image_factoring(r, inv.as_ref().expect("checked"), m);
                    if kept < m {
                        warnings.push(format!(
                            "FACTOR: image factoring keeps the {kept} factors whose eigenvalues are above 1"
                        ));
                    }
                    if kept == 0 {
                        return Err(Error(
                            "FACTOR: image factoring found no eigenvalue above 1".into(),
                        ));
                    }
                    Extracted {
                        loadings,
                        initial: smc.clone(),
                        pc_values: None,
                        iterations: None,
                        fit: None,
                    }
                }
                method => {
                    let fit = match method {
                        Extraction::UnweightedLeastSquares => Fit::Uls,
                        Extraction::GeneralizedLeastSquares => Fit::Gls,
                        _ => Fit::Ml,
                    };
                    let (loadings, iterations, test) = least_squares(
                        fit,
                        r,
                        inv.as_ref().expect("checked"),
                        m,
                        input.w,
                        c,
                        &mut warnings,
                    );
                    Extracted {
                        loadings,
                        initial: smc.clone(),
                        pc_values: None,
                        iterations: Some(iterations),
                        fit: test,
                    }
                }
            })
        };
        let extraction_caption = format!("Extraction Method: {}.", method_name(command.extraction));
        let mut extracted = extracted;
        if let Some(e) = extracted.as_mut() {
            let m = e.loadings[0].len();
            reflect(&mut e.loadings, m);
        }
        let m = extracted.as_ref().map_or(0, |e| e.loadings[0].len());
        let communalities = extracted.as_ref().map(|e| row_sums_of_squares(&e.loadings));
        let initial: Vec<f64> = extracted.as_ref().map_or_else(
            || match pc {
                true => vec![1.0; p],
                false => smc.clone(),
            },
            |e| e.initial.clone(),
        );

        // the rotation
        let rc = &command.rotation_criteria;
        let rotated = match (extracted.as_ref(), command.rotation) {
            (_, Rotation::None) | (None, _) => None,
            (Some(_), _) if m < 2 => {
                warnings.push(format!(
                    "FACTOR: only one {} was extracted, so the solution cannot be rotated",
                    word.to_lowercase()
                ));
                None
            }
            (Some(e), rotation) => Some(match rotation {
                Rotation::Oblimin(delta) => oblimin(&e.loadings, delta, rc, &mut warnings),
                Rotation::Promax(kappa) => promax(&e.loadings, kappa, rc, &mut warnings),
                kind => orthogonal(&e.loadings, kind, rc, &mut warnings),
            }),
        };
        let rotation_caption = format!(
            "{extraction_caption} Rotation Method: {}.",
            rotation_name(command.rotation, rc.kaiser)
        );

        if print.initial || print.extraction {
            let mut columns = Vec::new();
            if print.initial {
                columns.push("Initial");
            }
            if print.extraction && communalities.is_some() {
                columns.push("Extraction");
            }
            let rows = (0..p)
                .map(|i| {
                    let mut row = vec![label(names, i)];
                    if print.initial {
                        row.push(number(Some(initial[i])));
                    }
                    if let (true, Some(h)) = (print.extraction, &communalities) {
                        row.push(number(Some(h[i])));
                    }
                    row
                })
                .collect();
            let mut t = table("Communalities", 1, &[plain(&columns)], rows);
            t.caption = Some(extraction_caption.clone());
            tables.push(t);

            // Total Variance Explained
            let three = ["Total", "% of Variance", "Cumulative %"];
            let block = |values: &[f64], rows: &mut Vec<Vec<Cell>>| {
                let mut cumulative = 0.0;
                for (k, row) in rows.iter_mut().enumerate() {
                    match values.get(k) {
                        Some(v) => {
                            let percent = 100.0 * v / p as f64;
                            cumulative += percent;
                            row.extend([
                                number(Some(*v)),
                                number(Some(percent)),
                                number(Some(cumulative)),
                            ]);
                        }
                        None => row.extend([Cell::Empty, Cell::Empty, Cell::Empty]),
                    }
                }
            };
            let count = if print.initial { p } else { m };
            let mut rows: Vec<Vec<Cell>> =
                (1..=count).map(|k| vec![Cell::Number(k as f64)]).collect();
            let mut groups = Vec::new();
            let three_names = || three.iter().map(|s| s.to_string()).collect::<Vec<_>>();
            if print.initial {
                block(&eigenvalues, &mut rows);
                groups.push(("Initial Eigenvalues", three_names()));
            }
            if let (true, Some(e)) = (print.extraction, &extracted) {
                block(&column_sums_of_squares(&e.loadings, m), &mut rows);
                groups.push(("Extraction Sums of Squared Loadings", three_names()));
            }
            let mut oblique_note = false;
            if let (true, Some(rot)) = (print.rotation, &rotated) {
                match &rot.phi {
                    None => {
                        block(&column_sums_of_squares(&rot.pattern, m), &mut rows);
                        groups.push(("Rotation Sums of Squared Loadings", three_names()));
                    }
                    Some(phi) => {
                        let structure = multiply(&rot.pattern, phi);
                        let ss = column_sums_of_squares(&structure, m);
                        for (k, row) in rows.iter_mut().enumerate() {
                            row.push(ss.get(k).map_or(Cell::Empty, |v| number(Some(*v))));
                        }
                        groups.push(("Rotation Sums of Squared Loadings", vec!["Total".into()]));
                        oblique_note = true;
                    }
                }
            }
            let mut t = table("Total Variance Explained", 1, &groups, rows);
            t.columns[0] = word.to_string();
            t.caption = Some(match oblique_note {
                true => format!(
                    "{extraction_caption} When {}s are correlated, sums of squared loadings cannot be added to obtain a total variance.",
                    word.to_lowercase()
                ),
                false => extraction_caption.clone(),
            });
            tables.push(t);
        }

        let Some(extracted) = extracted else {
            if let Some(plot) = &command.plot {
                let solution = FactorSolution {
                    word,
                    names,
                    eigenvalues: &eigenvalues,
                    pattern: None,
                    rotated: false,
                };
                let charts = factor_plots(plot, solution, &mut warnings);
                self.charts.extend(charts);
            }
            self.warn_all(warnings);
            return Ok(tables);
        };
        let loadings = &extracted.loadings;
        if print.extraction {
            let plural = if m == 1 { "" } else { "s" };
            let mut caption = format!(
                "{extraction_caption} {m} {}{plural} extracted.",
                word.to_lowercase()
            );
            if let Some(iterations) = extracted.iterations {
                caption.push_str(&format!(" {iterations} iterations required."));
            }
            tables.push(loading_table(
                &format!("{word} Matrix"),
                word,
                names,
                loadings,
                command.sort,
                command.blank,
                caption,
            ));
            if let Some((chi, df)) = extracted.fit {
                let sig = if df > 0.0 {
                    number(Some(chi_square_p(chi, df)))
                } else {
                    Cell::Empty
                };
                let mut t = table(
                    "Goodness-of-fit Test",
                    0,
                    &[plain(&["Chi-Square", "df", "Sig."])],
                    vec![vec![number(Some(chi)), Cell::Number(df), sig]],
                );
                t.caption = Some(extraction_caption.clone());
                tables.push(t);
            }
        }
        if print.repr {
            let reproduced = multiply(loadings, &transpose(loadings));
            let residual: Matrix = (0..p)
                .map(|i| (0..p).map(|j| r[i][j] - reproduced[i][j]).collect())
                .collect();
            let large = (0..p)
                .flat_map(|i| (0..i).map(move |j| (i, j)))
                .filter(|&(i, j)| residual[i][j].abs() > 0.05)
                .count();
            let pairs = p * (p - 1) / 2;
            let mut t = square_table(
                "Reproduced Correlations",
                names,
                &[
                    ("Reproduced Correlation", reproduced, false),
                    ("Residual", residual, true),
                ],
            );
            t.caption = Some(format!(
                "{extraction_caption} Residuals are computed between observed and reproduced correlations. There are {large} ({}%) nonredundant residuals with absolute values greater than 0.05. The diagonal of the reproduced correlations holds the communalities.",
                (100.0 * large as f64 / pairs as f64).round()
            ));
            tables.push(t);
        }
        if let (true, Some(rot)) = (print.rotation, &rotated) {
            let caption = format!(
                "{rotation_caption} Rotation converged in {} iterations.",
                rot.iterations
            );
            match &rot.phi {
                None => tables.push(loading_table(
                    &format!("Rotated {word} Matrix"),
                    word,
                    names,
                    &rot.pattern,
                    command.sort,
                    command.blank,
                    caption,
                )),
                Some(phi) => {
                    tables.push(loading_table(
                        "Pattern Matrix",
                        word,
                        names,
                        &rot.pattern,
                        command.sort,
                        command.blank,
                        caption,
                    ));
                    tables.push(loading_table(
                        "Structure Matrix",
                        word,
                        names,
                        &multiply(&rot.pattern, phi),
                        command.sort,
                        command.blank,
                        rotation_caption.clone(),
                    ));
                }
            }
            let (title, values) = match (&rot.transform, &rot.phi) {
                (Some(t), _) => (format!("{word} Transformation Matrix"), t),
                (None, Some(phi)) => (format!("{word} Correlation Matrix"), phi),
                (None, None) => unreachable!("a rotation has one or the other"),
            };
            let numbers: Vec<String> = (1..=m).map(|j| j.to_string()).collect();
            let rows = (0..m)
                .map(|i| {
                    let mut row = vec![Cell::Number((i + 1) as f64)];
                    row.extend(values[i].iter().map(|v| number(Some(*v))));
                    row
                })
                .collect();
            let mut t = table(&title, 1, &[("", numbers)], rows);
            t.columns[0] = word.to_string();
            t.caption = Some(rotation_caption.clone());
            tables.push(t);
        }

        // factor scores
        let pattern = rotated.as_ref().map_or(loadings, |rot| &rot.pattern);
        let structure = rotated
            .as_ref()
            .and_then(|rot| rot.phi.as_ref())
            .map(|phi| multiply(pattern, phi));
        let method = command
            .save
            .as_ref()
            .map_or(ScoreMethod::Regression, |s| s.method);
        if print.fscore || command.save.is_some() {
            let coefficients = score_coefficients(
                method,
                r,
                inv.as_ref(),
                pattern,
                structure.as_ref(),
                &extracted,
                rotated.is_some(),
            );
            match coefficients {
                None => warnings.push(
                    "FACTOR: the factor score coefficients cannot be computed, since a matrix they need is singular"
                        .into(),
                ),
                Some(w) => {
                    let method_caption = match method {
                        ScoreMethod::Regression => "Regression",
                        ScoreMethod::Bartlett => "Bartlett",
                        ScoreMethod::AndersonRubin => "Anderson-Rubin",
                    };
                    let caption = match rotated {
                        Some(_) => format!("{rotation_caption} {} Scores: {method_caption}.", word),
                        None => format!("{extraction_caption} {word} Scores: {method_caption}."),
                    };
                    if print.fscore {
                        tables.push(loading_table(
                            &format!("{word} Score Coefficient Matrix"),
                            word,
                            names,
                            &w,
                            false,
                            None,
                            caption.clone(),
                        ));
                        let covariance = multiply(&transpose(&w), &multiply(r, &w));
                        let numbers: Vec<String> = (1..=m).map(|j| j.to_string()).collect();
                        let rows = (0..m)
                            .map(|i| {
                                let mut row = vec![Cell::Number((i + 1) as f64)];
                                row.extend(covariance[i].iter().map(|v| number(Some(*v))));
                                row
                            })
                            .collect();
                        let mut t = table(
                            &format!("{word} Score Covariance Matrix"),
                            1,
                            &[("", numbers)],
                            rows,
                        );
                        t.columns[0] = word.to_string();
                        t.caption = Some(caption);
                        tables.push(t);
                    }
                    if let Some(save) = &command.save {
                        let count = save.count.unwrap_or(m);
                        if count > m {
                            warnings.push(format!(
                                "FACTOR /SAVE: {count} scores asked for, but the solution has {m}; {m} are saved"
                            ));
                        }
                        let z = input.z.as_ref().expect("cases give z-scores");
                        self.save_scores(save.root.as_deref(), method, &w, z, count.min(m))?;
                    }
                }
            }
        }
        if let Some(plot) = &command.plot {
            let pattern = rotated.as_ref().map_or(loadings, |rot| &rot.pattern);
            let charts = factor_plots(
                plot,
                FactorSolution {
                    word,
                    names,
                    eigenvalues: &eigenvalues,
                    pattern: Some(pattern),
                    rotated: rotated.is_some(),
                },
                &mut warnings,
            );
            self.charts.extend(charts);
        }
        self.warn_all(warnings);
        Ok(tables)
    }

    fn warn_all(&mut self, warnings: Vec<String>) {
        for w in warnings {
            self.warn(w);
        }
    }

    fn save_scores(
        &mut self,
        root: Option<&str>,
        method: ScoreMethod,
        w: &Matrix,
        z: &[Option<Vec<f64>>],
        count: usize,
    ) -> Result<(), Error> {
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            let exists = |data: &Dataset, name: &str| data.variable_index(name).is_some();
            let (names, set): (Vec<String>, usize) = match root {
                Some(root) => ((1..=count).map(|j| format!("{root}{j}")).collect(), 1),
                None => {
                    let mut set = 1;
                    while (1..=count).any(|j| exists(data, &format!("FAC{j}_{set}"))) {
                        set += 1;
                    }
                    ((1..=count).map(|j| format!("FAC{j}_{set}")).collect(), set)
                }
            };
            let prefix = match method {
                ScoreMethod::Regression => "REGR",
                ScoreMethod::Bartlett => "BART",
                ScoreMethod::AndersonRubin => "AR",
            };
            for (j, name) in names.iter().enumerate() {
                if exists(data, name) {
                    return Err(Error(format!(
                        "FACTOR /SAVE: variable {name} already exists"
                    )));
                }
                let values: Vec<Option<f64>> = z
                    .iter()
                    .map(|z| {
                        z.as_ref()
                            .map(|z| z.iter().zip(w).map(|(z, row)| z * row[j]).sum())
                    })
                    .collect();
                data.set_numeric_values(name, values)?;
                let label = format!("{prefix} factor score {} for analysis {set}", j + 1);
                data.set_variable_label(name, Some(&label))?;
                data.set_format(name, "F11.5")?;
                data.set_measure(name, Measure::Scale)?;
            }
            Ok(())
        })
    }
}

/// The factor score coefficients, one row per variable and one column per factor.
fn score_coefficients(
    method: ScoreMethod,
    r: &Matrix,
    inv: Option<&Matrix>,
    pattern: &Matrix,
    structure: Option<&Matrix>,
    extracted: &Extracted,
    rotated: bool,
) -> Option<Matrix> {
    let p = r.len();
    let m = pattern[0].len();
    match method {
        ScoreMethod::Regression => match (&extracted.pc_values, rotated) {
            (Some(values), false) => {
                if values.iter().any(|v| v.abs() <= 1e-8) {
                    return None;
                }
                Some(
                    pattern
                        .iter()
                        .map(|row| (0..m).map(|j| row[j] / values[j]).collect())
                        .collect(),
                )
            }
            (Some(_), true) => {
                let ltl = multiply(&transpose(pattern), pattern);
                let (ltl_inv, det) = inverse(&ltl)?;
                if det < 1e-8 {
                    return None;
                }
                Some(multiply(pattern, &ltl_inv))
            }
            (None, _) => Some(multiply(inv?, structure.unwrap_or(pattern))),
        },
        ScoreMethod::Bartlett | ScoreMethod::AndersonRubin => {
            // U^2 is the diagonal of R minus the reproduced correlations: the unique variances
            let h = row_sums_of_squares(&extracted.loadings);
            let u2: Vec<f64> = (0..p).map(|i| 1.0 - h[i]).collect();
            if u2.iter().any(|u| *u <= 0.0) {
                return None;
            }
            // Lambda' U^-2, m x p
            let lu: Matrix = (0..m)
                .map(|j| (0..p).map(|i| pattern[i][j] / u2[i]).collect())
                .collect();
            let middle = match method {
                ScoreMethod::Bartlett => {
                    let (j_inv, _) = inverse(&multiply(&lu, pattern))?;
                    j_inv
                }
                _ => {
                    let lur = multiply(&lu, r);
                    let ul: Matrix = transpose(&lu);
                    symmetric_power(&multiply(&lur, &ul), -0.5)?
                }
            };
            Some(transpose(&multiply(&middle, &lu)))
        }
    }
}
