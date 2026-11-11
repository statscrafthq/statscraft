//! Contrasts among the k categories of a factor, shared by UNIANOVA, GLM and LOGISTIC
//! REGRESSION.
//!
//! Rules from the Command Syntax Reference (CONTRAST of UNIANOVA and LOGISTIC REGRESSION):
//! - DEVIATION: each category but the reference against the mean of all; SIMPLE: against the
//!   reference category; DIFFERENCE: each category but the first against the mean of the ones
//!   before it; HELMERT: each but the last against the mean of the ones after it; REPEATED: each
//!   but the last against the next; POLYNOMIAL: orthonormal polynomials of degree 1 to k - 1
//!   over the metric (default 1, 2, ..., k). The reference category is by default the last.
//! - In LOGISTIC REGRESSION a contrast becomes the coding of k - 1 design variables: the columns
//!   of the inverse of the matrix whose first row is the mean (1/k each) and whose other rows are
//!   the contrasts, so that each coefficient estimates its contrast. INDICATOR is the 0/1 coding
//!   of every category but the reference.

use crate::matrix::{Matrix, inverse};
use statscraft_syntax::Contrast;

fn reference(given: Option<u32>, k: usize) -> Result<usize, String> {
    match given {
        None => Ok(k - 1),
        Some(r) if (r as usize) <= k => Ok(r as usize - 1),
        Some(r) => Err(format!(
            "the reference category {r} is beyond the {k} categories"
        )),
    }
}

fn unit(k: usize, i: usize) -> Vec<f64> {
    (0..k).map(|j| f64::from(u8::from(i == j))).collect()
}

/// Orthonormal polynomial contrasts of degree 1 to k - 1 over `metric`, rising with the metric.
fn polynomial(metric: &[f64]) -> Vec<Vec<f64>> {
    let k = metric.len();
    let mean = metric.iter().sum::<f64>() / k as f64;
    let mut basis: Vec<Vec<f64>> = vec![vec![1.0 / (k as f64).sqrt(); k]];
    for degree in 1..k {
        let mut v: Vec<f64> = metric
            .iter()
            .map(|m| (m - mean).powi(degree as i32))
            .collect();
        for b in &basis {
            let dot: f64 = v.iter().zip(b).map(|(x, y)| x * y).sum();
            for (x, y) in v.iter_mut().zip(b) {
                *x -= dot * y;
            }
        }
        let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
        basis.push(v.iter().map(|x| x / norm).collect());
    }
    basis.remove(0);
    basis
}

/// The k - 1 contrast rows of k categories, each with its label, such as "Level 1 vs. Level 3".
/// INDICATOR gives the rows of SIMPLE; SPECIAL gives its rows (the last k - 1 when k are given).
pub(crate) fn contrast_rows(kind: &Contrast, k: usize) -> Result<Vec<(String, Vec<f64>)>, String> {
    let level = |i: usize| format!("Level {}", i + 1);
    let mean_of = |range: std::ops::Range<usize>| -> Vec<f64> {
        let n = range.len() as f64;
        (0..k)
            .map(|j| if range.contains(&j) { 1.0 / n } else { 0.0 })
            .collect()
    };
    let minus =
        |a: Vec<f64>, b: Vec<f64>| -> Vec<f64> { a.iter().zip(&b).map(|(x, y)| x - y).collect() };
    Ok(match kind {
        Contrast::Deviation(r) => {
            let r = reference(*r, k)?;
            (0..k)
                .filter(|&i| i != r)
                .map(|i| {
                    (
                        format!("{} vs. Mean", level(i)),
                        minus(unit(k, i), mean_of(0..k)),
                    )
                })
                .collect()
        }
        Contrast::Simple(r) | Contrast::Indicator(r) => {
            let r = reference(*r, k)?;
            (0..k)
                .filter(|&i| i != r)
                .map(|i| {
                    (
                        format!("{} vs. {}", level(i), level(r)),
                        minus(unit(k, i), unit(k, r)),
                    )
                })
                .collect()
        }
        Contrast::Difference => (1..k)
            .map(|i| {
                (
                    format!("{} vs. Previous", level(i)),
                    minus(unit(k, i), mean_of(0..i)),
                )
            })
            .collect(),
        Contrast::Helmert => (0..k - 1)
            .map(|i| {
                (
                    format!("{} vs. Later", level(i)),
                    minus(unit(k, i), mean_of(i + 1..k)),
                )
            })
            .collect(),
        Contrast::Repeated => (0..k - 1)
            .map(|i| {
                (
                    format!("{} vs. {}", level(i), level(i + 1)),
                    minus(unit(k, i), unit(k, i + 1)),
                )
            })
            .collect(),
        Contrast::Polynomial(metric) => {
            let metric: Vec<f64> = match metric {
                None => (1..=k).map(|i| i as f64).collect(),
                Some(m) if m.len() == k => m.clone(),
                Some(m) => {
                    return Err(format!(
                        "POLYNOMIAL has {} metric values for {k} categories",
                        m.len()
                    ));
                }
            };
            const NAMES: [&str; 5] = ["Linear", "Quadratic", "Cubic", "Order 4", "Order 5"];
            polynomial(&metric)
                .into_iter()
                .enumerate()
                .map(|(d, row)| {
                    let name = NAMES
                        .get(d)
                        .map_or(format!("Order {}", d + 1), |n| n.to_string());
                    (name, row)
                })
                .collect()
        }
        Contrast::Special(values) => {
            let rows = values.len() / k;
            if values.len() % k != 0 || !(rows == k || rows == k - 1) {
                return Err(format!(
                    "SPECIAL has {} values; {k} categories need {} or {} (rows of {k})",
                    values.len(),
                    (k - 1) * k,
                    k * k
                ));
            }
            values
                .chunks(k)
                .skip(rows + 1 - k)
                .enumerate()
                .map(|(i, row)| (format!("Contrast {}", i + 1), row.to_vec()))
                .collect()
        }
    })
}

/// The k x (k - 1) coding of the design variables of a categorical covariate.
pub(crate) fn coding(kind: &Contrast, k: usize) -> Result<Matrix, String> {
    if let Contrast::Indicator(r) = kind {
        let r = reference(*r, k)?;
        let kept: Vec<usize> = (0..k).filter(|&i| i != r).collect();
        return Ok((0..k)
            .map(|i| kept.iter().map(|&j| f64::from(u8::from(i == j))).collect())
            .collect());
    }
    let mut m: Matrix = vec![vec![1.0 / k as f64; k]];
    m.extend(contrast_rows(kind, k)?.into_iter().map(|(_, row)| row));
    let Some((inv, _)) = inverse(&m) else {
        return Err("the contrasts are linearly dependent".into());
    };
    Ok(inv.iter().map(|row| row[1..].to_vec()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &Matrix, b: &[[f64; 2]]) -> bool {
        a.iter()
            .zip(b)
            .all(|(x, y)| x.iter().zip(y).all(|(p, q)| (p - q).abs() < 1e-12))
    }

    #[test]
    fn codings_of_three_categories() {
        let third = 1.0 / 3.0;
        let coding = |c: Contrast| coding(&c, 3).unwrap();
        assert!(close(
            &coding(Contrast::Indicator(None)),
            &[[1.0, 0.0], [0.0, 1.0], [0.0, 0.0]]
        ));
        assert!(close(
            &coding(Contrast::Indicator(Some(1))),
            &[[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]]
        ));
        assert!(close(
            &coding(Contrast::Deviation(None)),
            &[[1.0, 0.0], [0.0, 1.0], [-1.0, -1.0]]
        ));
        assert!(close(
            &coding(Contrast::Simple(None)),
            &[
                [1.0 - third, -third],
                [-third, 1.0 - third],
                [-third, -third]
            ]
        ));
        assert!(close(
            &coding(Contrast::Helmert),
            &[[2.0 * third, 0.0], [-third, 0.5], [-third, -0.5]]
        ));
        assert!(close(
            &coding(Contrast::Difference),
            &[[-0.5, -third], [0.5, -third], [0.0, 2.0 * third]]
        ));
        let (a, b) = (1.0 / 2f64.sqrt(), 1.0 / 6f64.sqrt());
        assert!(close(
            &coding(Contrast::Polynomial(None)),
            &[[-a, b], [0.0, -2.0 * b], [a, b]]
        ));
        // an uneven metric: the linear contrast follows the spacing
        let rows = contrast_rows(&Contrast::Polynomial(Some(vec![1.0, 2.0, 4.0])), 3).unwrap();
        let linear = &rows[0].1;
        assert!(((linear[2] - linear[1]) / (linear[1] - linear[0]) - 2.0).abs() < 1e-12);
        assert!(super::coding(&Contrast::Indicator(Some(4)), 3).is_err());
    }
}
