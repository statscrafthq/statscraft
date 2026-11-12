//! The functions of the matrix language and the linear algebra behind them and CALL.
//!
//! Rules, from the descriptions of the functions (each takes matrices and gives one):
//! - Elementwise functions keep the shape of their argument. LN and LG10 need positive
//!   elements, SQRT ones that are not negative, ARSIN ones from -1 to 1.
//! - CMAX, CMIN, CSSQ and CSUM give a row with one value per column; RMAX, RMIN, RSSQ and RSUM a
//!   column with one value per row; MMAX, MMIN, MSSQ, MSUM, TRACE, NROW, NCOL, ALL, ANY, DET
//!   and RANK a scalar.
//! - CHOL(M) is the upper triangular B with T(B) * B = M; EVAL gives the eigenvalues of a
//!   symmetric matrix from the largest; GINV is the Moore-Penrose inverse; GSCH an orthonormal
//!   basis of the space of the columns, which must have the rank of the rows; SVAL the singular
//!   values from the largest; SOLVE(A, B) the X with A * X = B; SWEEP(M, k) the sweep on the
//!   pivot k with the formulas of the manual (a zero pivot zeroes its row and column).
//! - DESIGN gives one column per distinct value of each column, constant columns left out with
//!   a warning. GRADE numbers the elements 1, 2, ... from the smallest, ties in order;
//!   RNKORDER gives ties the mean of their ranks. MOD keeps the sign of the dividend; the
//!   divisor must not be 0. RESHAPE fills by rows. MAGIC needs at least 3.
//! - KRONEKER(A, B) puts A(i, j) * B in block (i, j). BLOCK puts its arguments down the
//!   diagonal. IDENT(r [, c]), MAKE(r, c, value) and MDIAG(v) build matrices.
//! - UNIFORM(r, c) and NORMAL(r, c) are r x c matrices of uniform (0, 1) and standard normal
//!   draws, from the generator SET RNG chooses. The distribution functions work element by
//!   element; any argument may be a scalar that goes with every element.
//!
//! Choices where the manual is silent:
//! - RND rounds a half away from zero, as RND does in COMPUTE (PSPP rounds it to even).
//! - DESIGN orders the columns of one variable by value.
//! - MAGIC: the Siamese square for odd sizes, the complement square for multiples of 4 and
//!   the LUX square for the other even sizes; PSPP gives other squares for even sizes.
//! - Eigenvectors (EIGEN) and singular vectors (SVD) are scaled to length 1 with their largest
//!   element (the last of equal ones) positive. CALL SVD gives U square (rows of M), Q with the
//!   dimensions of M and V square (columns of M), as the manual says.
//! - Symmetry (CHOL, EVAL, EIGEN) and singularity (INV, SOLVE, GSCH) are judged relative to the
//!   largest element; RANK and GINV ignore singular values below max(r, c) * largest * 2.2e-16.
//! - A distribution or random function whose parameters are out of range is an error.

use crate::matrix_lang::{Mat, fits};
use crate::random::Random;
use std::cell::RefCell;

pub(crate) type Result<T> = std::result::Result<T, String>;

fn scalar(m: &Mat, function: &str, which: &str) -> Result<f64> {
    if m.is_scalar() {
        Ok(m.data[0])
    } else {
        Err(format!(
            "the {which} argument of {function} must be a scalar, but it is {}",
            m.dims()
        ))
    }
}

/// A scalar that counts: truncated, at least `least`.
fn count(m: &Mat, function: &str, which: &str, least: f64) -> Result<usize> {
    let value = scalar(m, function, which)?.trunc();
    if value.is_nan() || value < least || value > 1e8 {
        return Err(format!(
            "the {which} argument of {function} must be a whole number of at least {least}, got {value}"
        ));
    }
    Ok(value as usize)
}

fn square(m: &Mat, function: &str) -> Result<()> {
    if m.rows == m.cols {
        Ok(())
    } else {
        Err(format!(
            "{function} needs a square matrix, but it is {}",
            m.dims()
        ))
    }
}

fn largest(m: &Mat) -> f64 {
    m.data.iter().fold(0.0_f64, |s, v| s.max(v.abs()))
}

fn symmetric(m: &Mat, function: &str) -> Result<()> {
    square(m, function)?;
    let tolerance = 1e-10 * largest(m);
    for i in 0..m.rows {
        for j in 0..i {
            if (m.at(i, j) - m.at(j, i)).abs() > tolerance {
                return Err(format!(
                    "{function} needs a symmetric matrix, but element ({}, {}) differs from ({}, {})",
                    i + 1,
                    j + 1,
                    j + 1,
                    i + 1
                ));
            }
        }
    }
    Ok(())
}

fn each(m: &Mat, f: impl Fn(f64) -> f64) -> Mat {
    m.map(f)
}

/// Elementwise with a check on the domain.
fn each_checked(
    m: &Mat,
    function: &str,
    ok: impl Fn(f64) -> bool,
    rule: &str,
    f: impl Fn(f64) -> f64,
) -> Result<Mat> {
    if let Some(bad) = m.data.iter().find(|v| !ok(**v)) {
        return Err(format!(
            "the elements of {function} must be {rule}, but one is {bad}"
        ));
    }
    Ok(m.map(f))
}

/// Rounds a half away from zero.
fn round(x: f64) -> f64 {
    x.round()
}

/// The shape that the elementwise arguments share: scalars go with any shape.
pub(crate) fn common_shape(args: &[Mat], function: &str) -> Result<(usize, usize)> {
    let mut shape: Option<(usize, usize)> = None;
    for a in args.iter().filter(|a| !a.is_scalar()) {
        match shape {
            None => shape = Some((a.rows, a.cols)),
            Some(s) if s != (a.rows, a.cols) => {
                return Err(format!(
                    "the arguments of {function} must have the same dimensions or be scalars, \
                     but they are {}x{} and {}",
                    s.0,
                    s.1,
                    a.dims()
                ));
            }
            _ => {}
        }
    }
    Ok(shape.unwrap_or((1, 1)))
}

/// `f` of the elements in the same place of each argument.
fn elementwise(
    args: &[Mat],
    function: &str,
    mut f: impl FnMut(&[f64]) -> Option<f64>,
) -> Result<Mat> {
    let (rows, cols) = common_shape(args, function)?;
    let mut data = Vec::with_capacity(rows * cols);
    let mut values = vec![0.0; args.len()];
    for k in 0..rows * cols {
        for (v, a) in values.iter_mut().zip(args) {
            *v = if a.is_scalar() { a.data[0] } else { a.data[k] };
        }
        match f(&values) {
            Some(x) => data.push(x),
            None => {
                let shown: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                return Err(format!(
                    "{function}({}) is out of range of its parameters",
                    shown.join(", ")
                ));
            }
        }
    }
    Ok(Mat::new(rows, cols, data))
}

/// Calls the matrix function `name` (upper case, in full) on its evaluated arguments.
pub(crate) fn call(
    name: &str,
    args: &[Mat],
    random: &RefCell<Random>,
    warnings: &mut Vec<String>,
) -> Result<Mat> {
    let a = &args[0];
    let one = |m: Mat| Ok(m);
    let s = |x: f64| Ok(Mat::scalar(x));
    let bool_value = |b: bool| f64::from(u8::from(b));
    match name {
        "ABS" => one(each(a, f64::abs)),
        "ALL" => s(bool_value(a.data.iter().all(|v| *v != 0.0))),
        "ANY" => s(bool_value(a.data.iter().any(|v| *v != 0.0))),
        "ARSIN" => each_checked(
            a,
            name,
            |v| (-1.0..=1.0).contains(&v),
            "from -1 to 1",
            f64::asin,
        ),
        "ARTAN" => one(each(a, f64::atan)),
        "COS" => one(each(a, f64::cos)),
        "SIN" => one(each(a, f64::sin)),
        "EXP" => one(each(a, f64::exp)),
        "LN" => each_checked(a, name, |v| v > 0.0, "positive", f64::ln),
        "LG10" => each_checked(a, name, |v| v > 0.0, "positive", f64::log10),
        "SQRT" => each_checked(a, name, |v| v >= 0.0, "0 or more", f64::sqrt),
        "RND" => one(each(a, round)),
        "TRUNC" => one(each(a, f64::trunc)),
        "CDFNORM" => one(each(a, crate::dist::normal_cdf)),
        "PROBIT" => each_checked(
            a,
            name,
            |v| v > 0.0 && v < 1.0,
            "between 0 and 1",
            crate::dist::normal_quantile,
        ),
        "CMAX" => one(by_columns(a, f64::NEG_INFINITY, f64::max)),
        "CMIN" => one(by_columns(a, f64::INFINITY, f64::min)),
        "CSUM" => one(by_columns(a, 0.0, |s, v| s + v)),
        "CSSQ" => one(by_columns(a, 0.0, |s, v| s + v * v)),
        "RMAX" => one(by_columns(&a.transpose(), f64::NEG_INFINITY, f64::max).transpose()),
        "RMIN" => one(by_columns(&a.transpose(), f64::INFINITY, f64::min).transpose()),
        "RSUM" => one(by_columns(&a.transpose(), 0.0, |s, v| s + v).transpose()),
        "RSSQ" => one(by_columns(&a.transpose(), 0.0, |s, v| s + v * v).transpose()),
        "MMAX" => s(a.data.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
        "MMIN" => s(a.data.iter().copied().fold(f64::INFINITY, f64::min)),
        "MSUM" => s(a.data.iter().sum()),
        "MSSQ" => s(a.data.iter().map(|v| v * v).sum()),
        "NROW" => s(a.rows as f64),
        "NCOL" => s(a.cols as f64),
        "TRACE" => s((0..a.rows.min(a.cols)).map(|i| a.at(i, i)).sum()),
        "TRANSPOS" => one(a.transpose()),
        "DIAG" => {
            let n = a.rows.min(a.cols);
            one(Mat::new(n, 1, (0..n).map(|i| a.at(i, i)).collect()))
        }
        "MDIAG" => {
            if !a.is_vector() {
                return Err(format!("MDIAG needs a vector, but it is {}", a.dims()));
            }
            let n = a.data.len();
            fits(n, n)?;
            let mut m = Mat::filled(n, n, 0.0);
            for (i, v) in a.data.iter().enumerate() {
                m.set(i, i, *v);
            }
            one(m)
        }
        "IDENT" => {
            let rows = count(a, name, "first", 0.0)?;
            let cols = match args.get(1) {
                Some(b) => count(b, name, "second", 0.0)?,
                None => rows,
            };
            fits(rows, cols)?;
            let mut m = Mat::filled(rows, cols, 0.0);
            for i in 0..rows.min(cols) {
                m.set(i, i, 1.0);
            }
            one(m)
        }
        "MAKE" => {
            let rows = count(a, name, "first", 0.0)?;
            let cols = count(&args[1], name, "second", 0.0)?;
            let value = scalar(&args[2], name, "third")?;
            fits(rows, cols)?;
            one(Mat::filled(rows, cols, value))
        }
        "RESHAPE" => {
            let rows = count(&args[1], name, "second", 0.0)?;
            let cols = count(&args[2], name, "third", 0.0)?;
            if rows * cols != a.data.len() {
                return Err(format!(
                    "RESHAPE cannot make {rows}x{cols} of the {} elements of a {} matrix",
                    a.data.len(),
                    a.dims()
                ));
            }
            one(Mat::new(rows, cols, a.data.clone()))
        }
        "MOD" => {
            let divisor = scalar(&args[1], name, "second")?;
            if divisor == 0.0 {
                return Err("the divisor of MOD must not be 0".into());
            }
            one(each(a, |v| v % divisor))
        }
        "BLOCK" => {
            let rows = args.iter().map(|m| m.rows).sum();
            let cols = args.iter().map(|m| m.cols).sum();
            let mut out = Mat::filled(rows, cols, 0.0);
            let (mut r0, mut c0) = (0, 0);
            for m in args {
                for i in 0..m.rows {
                    for j in 0..m.cols {
                        out.set(r0 + i, c0 + j, m.at(i, j));
                    }
                }
                r0 += m.rows;
                c0 += m.cols;
            }
            one(out)
        }
        "KRONEKER" => {
            let b = &args[1];
            let too_big = || format!("KRONEKER of {} and {} is too large", a.dims(), b.dims());
            let rows = a.rows.checked_mul(b.rows).ok_or_else(too_big)?;
            let cols = a.cols.checked_mul(b.cols).ok_or_else(too_big)?;
            fits(rows, cols)?;
            let mut out = Mat::filled(rows, cols, 0.0);
            for i in 0..a.rows {
                for j in 0..a.cols {
                    for k in 0..b.rows {
                        for l in 0..b.cols {
                            out.set(i * b.rows + k, j * b.cols + l, a.at(i, j) * b.at(k, l));
                        }
                    }
                }
            }
            one(out)
        }
        "SSCP" => {
            fits(a.cols, a.cols)?;
            one(a.transpose().product(a))
        }
        "DET" => {
            square(a, name)?;
            s(determinant(a))
        }
        "INV" => {
            square(a, name)?;
            match crate::matrix::inverse(&a.to_rows()) {
                Some((inverse, _)) => one(Mat::from_rows(&inverse, a.cols)),
                None => Err("INV needs a nonsingular matrix, but this one is singular".into()),
            }
        }
        "SOLVE" => {
            square(a, name)?;
            let b = &args[1];
            if b.rows != a.rows {
                return Err(format!(
                    "SOLVE needs as many rows in its second argument as in its first, but they are {} and {}",
                    b.dims(),
                    a.dims()
                ));
            }
            solve(a, b).ok_or_else(|| {
                "SOLVE needs a nonsingular first argument, but it is singular".into()
            })
        }
        "CHOL" => {
            symmetric(a, name)?;
            cholesky(a).ok_or_else(|| "CHOL needs a positive definite matrix".into())
        }
        "EVAL" => {
            symmetric(a, name)?;
            let (values, _) = eigen(a);
            one(values)
        }
        "SVAL" => {
            let (_, q, _) = svd(a);
            let n = a.rows.min(a.cols);
            one(Mat::new(n, 1, (0..n).map(|i| q.at(i, i)).collect()))
        }
        "RANK" => {
            let (_, q, _) = svd(a);
            let n = a.rows.min(a.cols);
            let tolerance = svd_tolerance(a, &q);
            s((0..n).filter(|&i| q.at(i, i) > tolerance).count() as f64)
        }
        "GINV" => one(pseudo_inverse(a)),
        "GSCH" => gram_schmidt(a),
        "SWEEP" => {
            let k = count(&args[1], name, "second", 1.0)?;
            if k > a.rows || k > a.cols {
                return Err(format!(
                    "the pivot of SWEEP must be at most the rows and the columns, but it is {k} for a {} matrix",
                    a.dims()
                ));
            }
            one(sweep(a, k - 1))
        }
        "DESIGN" => design(a, warnings),
        "GRADE" => one(ranks(a, false)),
        "RNKORDER" => one(ranks(a, true)),
        "MAGIC" => {
            let n = count(a, name, "first", 3.0)?;
            fits(n, n)?;
            one(magic(n))
        }
        "UNIFORM" | "NORMAL" if args.len() == 2 => {
            let rows = count(a, name, "first", 0.0)?;
            let cols = count(&args[1], name, "second", 0.0)?;
            fits(rows, cols)?;
            let mut random = random.borrow_mut();
            let data = (0..rows * cols)
                .map(|_| {
                    if name == "UNIFORM" {
                        random.uniform()
                    } else {
                        random.normal()
                    }
                })
                .collect();
            one(Mat::new(rows, cols, data))
        }
        "CHICDF" => call("CDF.CHISQ", args, random, warnings),
        "FCDF" => call("CDF.F", args, random, warnings),
        "TCDF" => call("CDF.T", args, random, warnings),
        _ if name.starts_with("RV.") || name == "UNIFORM" || name == "NORMAL" => {
            let mut random = random.borrow_mut();
            elementwise(args, name, |p| crate::random::draw(&mut random, name, p))
        }
        _ => elementwise(args, name, |p| crate::distributions::evaluate(name, p)),
    }
}

/// One value per column, folded from `start`.
fn by_columns(m: &Mat, start: f64, f: impl Fn(f64, f64) -> f64) -> Mat {
    let data = (0..m.cols)
        .map(|j| (0..m.rows).fold(start, |acc, i| f(acc, m.at(i, j))))
        .collect();
    Mat::new(1, m.cols, data)
}

/// The determinant by elimination with partial pivoting.
pub(crate) fn determinant(a: &Mat) -> f64 {
    let n = a.rows;
    let mut m = a.to_rows();
    let mut det = 1.0;
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|&x, &y| m[x][col].abs().total_cmp(&m[y][col].abs()))
            .expect("rows left");
        if m[pivot][col] == 0.0 {
            return 0.0;
        }
        if pivot != col {
            m.swap(pivot, col);
            det = -det;
        }
        det *= m[col][col];
        for i in col + 1..n {
            let f = m[i][col] / m[col][col];
            subtract_row(&mut m, i, col, f, col);
        }
    }
    det
}

/// Row `target` minus `f` times row `source`, from column `from` on.
fn subtract_row(m: &mut [Vec<f64>], target: usize, source: usize, f: f64, from: usize) {
    let source = m[source].clone();
    for (t, s) in m[target][from..].iter_mut().zip(&source[from..]) {
        *t -= f * s;
    }
}

/// X with A X = B, by elimination with partial pivoting; None when A is singular.
fn solve(a: &Mat, b: &Mat) -> Option<Mat> {
    let n = a.rows;
    let mut m = a.to_rows();
    let mut x = b.to_rows();
    let scale = largest(a).max(f64::MIN_POSITIVE);
    for col in 0..n {
        let pivot = (col..n).max_by(|&p, &q| m[p][col].abs().total_cmp(&m[q][col].abs()))?;
        if m[pivot][col].abs() <= 1e-13 * scale {
            return None;
        }
        m.swap(pivot, col);
        x.swap(pivot, col);
        for i in (0..n).filter(|&i| i != col) {
            let f = m[i][col] / m[col][col];
            if f == 0.0 {
                continue;
            }
            subtract_row(&mut m, i, col, f, col);
            subtract_row(&mut x, i, col, f, 0);
        }
    }
    for (i, row) in x.iter_mut().enumerate() {
        for v in row.iter_mut() {
            *v /= m[i][i];
        }
    }
    Some(Mat::from_rows(&x, b.cols))
}

fn cholesky(a: &Mat) -> Option<Mat> {
    let n = a.rows;
    let mut b = Mat::filled(n, n, 0.0);
    for j in 0..n {
        let d = a.at(j, j) - (0..j).map(|k| b.at(k, j).powi(2)).sum::<f64>();
        if d <= 0.0 {
            return None;
        }
        let d = d.sqrt();
        b.set(j, j, d);
        for i in j + 1..n {
            let v = a.at(j, i) - (0..j).map(|k| b.at(k, j) * b.at(k, i)).sum::<f64>();
            b.set(j, i, v / d);
        }
    }
    Some(b)
}

/// A vector with its largest element (the last of equal ones) positive.
fn orient(v: &mut [f64]) {
    let mut at = 0;
    for (i, x) in v.iter().enumerate() {
        if x.abs() >= v[at].abs() - 1e-12 * v[at].abs() {
            at = i;
        }
    }
    if v.get(at).is_some_and(|x| *x < 0.0) {
        v.iter_mut().for_each(|x| *x = -*x);
    }
}

/// The eigenvalues of a symmetric matrix from the largest, as a column, and the eigenvectors
/// as the columns of a matrix.
pub(crate) fn eigen(a: &Mat) -> (Mat, Mat) {
    let n = a.rows;
    let pairs = crate::matrix::symmetric_eigen(&a.to_rows());
    let mut vectors = Mat::filled(n, n, 0.0);
    let mut values = Vec::with_capacity(n);
    for (j, (value, mut vector)) in pairs.into_iter().enumerate() {
        orient(&mut vector);
        for (i, x) in vector.into_iter().enumerate() {
            vectors.set(i, j, x);
        }
        values.push(value);
    }
    (Mat::new(n, 1, values), vectors)
}

/// U, Q and V with A = U Q T(V): U square with the rows of A, Q with the dimensions of A and
/// the singular values from the largest on its diagonal, V square with the columns of A. By
/// one-sided Jacobi rotations.
pub(crate) fn svd(a: &Mat) -> (Mat, Mat, Mat) {
    if a.rows < a.cols {
        let (u, q, v) = svd(&a.transpose());
        return (v, q.transpose(), u);
    }
    let (m, n) = (a.rows, a.cols);
    let mut w = a.to_rows();
    let mut v: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| f64::from(u8::from(i == j))).collect())
        .collect();
    for _ in 0..100 {
        let mut rotated = false;
        for p in 0..n {
            for q in p + 1..n {
                let (mut alpha, mut beta, mut gamma) = (0.0, 0.0, 0.0);
                for row in &w {
                    alpha += row[p] * row[p];
                    beta += row[q] * row[q];
                    gamma += row[p] * row[q];
                }
                if gamma == 0.0 || gamma.abs() <= 1e-15 * (alpha * beta).sqrt() {
                    continue;
                }
                rotated = true;
                let zeta = (beta - alpha) / (2.0 * gamma);
                let t = zeta.signum() / (zeta.abs() + (1.0 + zeta * zeta).sqrt());
                let t = if zeta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = c * t;
                for row in w.iter_mut().chain(v.iter_mut()) {
                    let (x, y) = (row[p], row[q]);
                    row[p] = c * x - s * y;
                    row[q] = s * x + c * y;
                }
            }
        }
        if !rotated {
            break;
        }
    }
    let norms: Vec<f64> = (0..n)
        .map(|j| w.iter().map(|row| row[j] * row[j]).sum::<f64>().sqrt())
        .collect();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&x, &y| norms[y].total_cmp(&norms[x]));
    let mut q = Mat::filled(m, n, 0.0);
    let mut vm = Mat::filled(n, n, 0.0);
    let mut columns: Vec<Vec<f64>> = Vec::new();
    let top = norms.iter().copied().fold(0.0, f64::max);
    let tolerance = m.max(n) as f64 * top * f64::EPSILON;
    for (k, &j) in order.iter().enumerate() {
        let mut vector: Vec<f64> = v.iter().map(|row| row[j]).collect();
        orient(&mut vector);
        for (i, x) in vector.iter().enumerate() {
            vm.set(i, k, *x);
        }
        q.set(k, k, norms[j]);
        if norms[j] > tolerance {
            // the left vector from the oriented right one
            let u: Vec<f64> = (0..m)
                .map(|i| (0..n).map(|l| a.at(i, l) * vector[l]).sum::<f64>() / norms[j])
                .collect();
            columns.push(u);
        }
    }
    let u = complete_basis(columns, m);
    (u, q, vm)
}

/// Orthonormal columns completed to a square orthonormal matrix with unit vectors.
fn complete_basis(mut columns: Vec<Vec<f64>>, m: usize) -> Mat {
    for e in 0..m {
        if columns.len() == m {
            break;
        }
        let mut x: Vec<f64> = (0..m).map(|i| f64::from(u8::from(i == e))).collect();
        for _ in 0..2 {
            for c in &columns {
                let dot: f64 = c.iter().zip(&x).map(|(a, b)| a * b).sum();
                for (xi, ci) in x.iter_mut().zip(c) {
                    *xi -= dot * ci;
                }
            }
        }
        let norm = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm > 1e-8 {
            columns.push(x.into_iter().map(|v| v / norm).collect());
        }
    }
    let mut u = Mat::filled(m, m, 0.0);
    for (j, c) in columns.iter().enumerate() {
        for (i, x) in c.iter().enumerate() {
            u.set(i, j, *x);
        }
    }
    u
}

fn svd_tolerance(a: &Mat, q: &Mat) -> f64 {
    let top = if a.rows.min(a.cols) > 0 {
        q.at(0, 0)
    } else {
        0.0
    };
    a.rows.max(a.cols) as f64 * top * f64::EPSILON
}

fn pseudo_inverse(a: &Mat) -> Mat {
    let (u, q, v) = svd(a);
    let tolerance = svd_tolerance(a, &q);
    let mut out = Mat::filled(a.cols, a.rows, 0.0);
    for k in 0..a.rows.min(a.cols) {
        let sigma = q.at(k, k);
        if sigma <= tolerance {
            continue;
        }
        for i in 0..a.cols {
            for j in 0..a.rows {
                let value = out.at(i, j) + v.at(i, k) * u.at(j, k) / sigma;
                out.set(i, j, value);
            }
        }
    }
    out
}

fn gram_schmidt(a: &Mat) -> Result<Mat> {
    let m = a.rows;
    let mut columns: Vec<Vec<f64>> = Vec::new();
    for j in 0..a.cols {
        if columns.len() == m {
            break;
        }
        let original: Vec<f64> = (0..m).map(|i| a.at(i, j)).collect();
        let size = original.iter().map(|v| v * v).sum::<f64>().sqrt();
        let mut x = original;
        for _ in 0..2 {
            for c in &columns {
                let dot: f64 = c.iter().zip(&x).map(|(a, b)| a * b).sum();
                for (xi, ci) in x.iter_mut().zip(c) {
                    *xi -= dot * ci;
                }
            }
        }
        let norm = x.iter().map(|v| v * v).sum::<f64>().sqrt();
        if size > 0.0 && norm > 1e-10 * size {
            columns.push(x.into_iter().map(|v| v / norm).collect());
        }
    }
    if columns.len() < m {
        return Err(format!(
            "GSCH needs as many linearly independent columns as rows ({m}), but the columns span {}",
            columns.len()
        ));
    }
    let mut out = Mat::filled(m, m, 0.0);
    for (j, c) in columns.iter().enumerate() {
        for (i, x) in c.iter().enumerate() {
            out.set(i, j, *x);
        }
    }
    Ok(out)
}

/// SWEEP on the 0-based pivot `k` with the formulas of the manual.
fn sweep(m: &Mat, k: usize) -> Mat {
    let pivot = m.at(k, k);
    let mut a = m.clone();
    for i in 0..m.rows {
        for j in 0..m.cols {
            let value = if pivot == 0.0 {
                if i == k || j == k { 0.0 } else { m.at(i, j) }
            } else if i == k && j == k {
                1.0 / pivot
            } else if j == k {
                -m.at(i, k) / pivot
            } else if i == k {
                m.at(k, j) / pivot
            } else {
                (m.at(i, j) * pivot - m.at(i, k) * m.at(k, j)) / pivot
            };
            a.set(i, j, value);
        }
    }
    a
}

fn design(a: &Mat, warnings: &mut Vec<String>) -> Result<Mat> {
    let mut columns: Vec<Vec<f64>> = Vec::new();
    for j in 0..a.cols {
        let mut values: Vec<f64> = (0..a.rows).map(|i| a.at(i, j)).collect();
        values.sort_by(f64::total_cmp);
        values.dedup();
        if values.len() < 2 {
            warnings.push(format!(
                "column {} of the argument of DESIGN is constant and is left out",
                j + 1
            ));
            continue;
        }
        for value in values {
            columns.push(
                (0..a.rows)
                    .map(|i| f64::from(u8::from(a.at(i, j) == value)))
                    .collect(),
            );
        }
    }
    if columns.is_empty() && a.cols > 0 {
        return Err("every column of the argument of DESIGN is constant".into());
    }
    let mut out = Mat::filled(a.rows, columns.len(), 0.0);
    for (j, c) in columns.iter().enumerate() {
        for (i, x) in c.iter().enumerate() {
            out.set(i, j, *x);
        }
    }
    Ok(out)
}

/// Ranks of the elements, in order by rows for ties, or the mean rank of ties.
fn ranks(a: &Mat, mean_of_ties: bool) -> Mat {
    let mut order: Vec<usize> = (0..a.data.len()).collect();
    order.sort_by(|&x, &y| a.data[x].total_cmp(&a.data[y]).then(x.cmp(&y)));
    let mut out = vec![0.0; a.data.len()];
    let mut start = 0;
    while start < order.len() {
        let mut end = start + 1;
        while mean_of_ties && end < order.len() && a.data[order[end]] == a.data[order[start]] {
            end += 1;
        }
        let rank = (start + end + 1) as f64 / 2.0;
        for &k in &order[start..end] {
            out[k] = rank;
        }
        start = end;
    }
    Mat::new(a.rows, a.cols, out)
}

fn magic(n: usize) -> Mat {
    let mut m = Mat::filled(n, n, 0.0);
    if n % 2 == 1 {
        for i in 0..n {
            for j in 0..n {
                // the Siamese method as a formula, 1-based I and J
                let (ii, jj) = (i + 1, j + 1);
                let a = (ii + jj + n - (n + 3) / 2) % n;
                let b = (ii + 2 * jj + n - 2) % n;
                m.set(i, j, (n * a + b + 1) as f64);
            }
        }
    } else if n.is_multiple_of(4) {
        for i in 0..n {
            for j in 0..n {
                let value = (i * n + j + 1) as f64;
                let keep = ((i + 1) % 4 / 2 == (j + 1) % 4 / 2) as usize;
                let total = (n * n + 1) as f64;
                m.set(i, j, if keep == 1 { total - value } else { value });
            }
        }
    } else {
        let p = n / 2;
        let small = magic(p);
        let shift = [0, 2, 3, 1];
        for (block, (bi, bj)) in [(0, 0), (0, 1), (1, 0), (1, 1)].into_iter().enumerate() {
            let add = (shift[block] * p * p) as f64;
            for i in 0..p {
                for j in 0..p {
                    m.set(bi * p + i, bj * p + j, small.at(i, j) + add);
                }
            }
        }
        let k = (n - 2) / 4;
        let swap = |m: &mut Mat, i: usize, j: usize| {
            let (x, y) = (m.at(i, j), m.at(i + p, j));
            m.set(i, j, y);
            m.set(i + p, j, x);
        };
        let columns: Vec<usize> = (0..k).chain(n - k + 1..n).collect();
        for i in 0..p {
            for &j in &columns {
                swap(&mut m, i, j);
            }
        }
        for j in [0, k] {
            swap(&mut m, k, j);
        }
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(rows: usize, cols: usize, data: &[f64]) -> Mat {
        Mat::new(rows, cols, data.to_vec())
    }

    fn close(a: &Mat, b: &Mat) {
        assert_eq!((a.rows, a.cols), (b.rows, b.cols));
        for (x, y) in a.data.iter().zip(&b.data) {
            assert!((x - y).abs() < 1e-9, "{a:?} != {b:?}");
        }
    }

    #[test]
    fn magic_squares_have_equal_sums() {
        for n in 3..=12 {
            let sq = magic(n);
            let total = (n * (n * n + 1) / 2) as f64;
            let mut seen: Vec<f64> = sq.data.clone();
            seen.sort_by(f64::total_cmp);
            assert_eq!(
                seen,
                (1..=n * n).map(|v| v as f64).collect::<Vec<_>>(),
                "n={n}"
            );
            for i in 0..n {
                assert_eq!(
                    (0..n).map(|j| sq.at(i, j)).sum::<f64>(),
                    total,
                    "row {i} n={n}"
                );
                assert_eq!(
                    (0..n).map(|j| sq.at(j, i)).sum::<f64>(),
                    total,
                    "col {i} n={n}"
                );
            }
        }
        close(
            &magic(3),
            &m(3, 3, &[8.0, 1.0, 6.0, 3.0, 5.0, 7.0, 4.0, 9.0, 2.0]),
        );
    }

    #[test]
    fn svd_rebuilds_the_matrix() {
        for a in [
            m(3, 2, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
            m(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
            m(3, 3, &[1.0, 2.0, 3.0, 2.0, 4.0, 6.0, 1.0, 0.0, 1.0]),
        ] {
            let (u, q, v) = svd(&a);
            close(&u.product(&q).product(&v.transpose()), &a);
            close(&u.transpose().product(&u), &Mat::identity(a.rows));
            close(&v.transpose().product(&v), &Mat::identity(a.cols));
        }
    }

    #[test]
    fn decompositions() {
        let a = m(2, 2, &[4.0, 2.0, 2.0, 5.0]);
        close(&cholesky(&a).unwrap(), &m(2, 2, &[2.0, 1.0, 0.0, 2.0]));
        assert!((determinant(&m(2, 2, &[1.0, 2.0, 3.0, 4.0])) + 2.0).abs() < 1e-12);
        close(
            &solve(&m(2, 2, &[2.0, 1.0, 1.0, 3.0]), &m(2, 1, &[3.0, 5.0])).unwrap(),
            &m(2, 1, &[0.8, 1.4]),
        );
        let g = pseudo_inverse(&m(3, 2, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]));
        close(
            &g,
            &m(
                2,
                3,
                &[
                    -4.0 / 3.0,
                    -1.0 / 3.0,
                    2.0 / 3.0,
                    13.0 / 12.0,
                    1.0 / 3.0,
                    -5.0 / 12.0,
                ],
            ),
        );
    }
}
