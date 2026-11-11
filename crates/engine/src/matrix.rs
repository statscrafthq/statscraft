//! Small dense matrix helpers for the procedures that need linear algebra (REGRESSION,
//! RELIABILITY): the sweep operator, the inverse with the determinant, and the eigenvalues
//! of a symmetric matrix.

pub(crate) type Matrix = Vec<Vec<f64>>;

/// Sweeps the square matrix on pivot `k` as the Algorithms manual describes (Dempster 1969):
/// the pivot becomes 1/pivot, its column is divided by the pivot, its row by minus the pivot,
/// and the rest loses the product of row and column over the pivot. Sweeping twice on the same
/// pivot gives the matrix back, so the same call enters and removes a variable.
pub(crate) fn sweep(m: &mut Matrix, k: usize) {
    let pivot = m[k][k];
    let n = m.len();
    for i in (0..n).filter(|&i| i != k) {
        for j in (0..n).filter(|&j| j != k) {
            m[i][j] -= m[i][k] * m[k][j] / pivot;
        }
    }
    for i in (0..n).filter(|&i| i != k) {
        m[i][k] /= pivot;
        m[k][i] /= -pivot;
    }
    m[k][k] = 1.0 / pivot;
}

/// The transpose of a matrix.
pub(crate) fn transpose(a: &Matrix) -> Matrix {
    let cols = a.first().map_or(0, Vec::len);
    (0..cols)
        .map(|j| a.iter().map(|row| row[j]).collect())
        .collect()
}

/// The product a b.
pub(crate) fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let inner = b.len();
    let cols = b.first().map_or(0, Vec::len);
    a.iter()
        .map(|row| {
            (0..cols)
                .map(|j| (0..inner).map(|k| row[k] * b[k][j]).sum())
                .collect()
        })
        .collect()
}

/// The inverse and the determinant by Gauss-Jordan elimination with partial pivoting; None
/// when the matrix is singular.
pub(crate) fn inverse(a: &Matrix) -> Option<(Matrix, f64)> {
    let n = a.len();
    let mut m: Matrix = a.clone();
    let mut inv: Matrix = (0..n)
        .map(|i| (0..n).map(|j| f64::from(u8::from(i == j))).collect())
        .collect();
    let mut det = 1.0;
    let scale = a
        .iter()
        .flatten()
        .fold(0.0_f64, |s, v| s.max(v.abs()))
        .max(f64::MIN_POSITIVE);
    for col in 0..n {
        let pivot = (col..n).max_by(|&x, &y| m[x][col].abs().total_cmp(&m[y][col].abs()))?;
        if m[pivot][col].abs() <= 1e-13 * scale {
            return None;
        }
        if pivot != col {
            m.swap(pivot, col);
            inv.swap(pivot, col);
            det = -det;
        }
        let p = m[col][col];
        det *= p;
        for j in 0..n {
            m[col][j] /= p;
            inv[col][j] /= p;
        }
        for i in (0..n).filter(|&i| i != col) {
            let f = m[i][col];
            if f != 0.0 {
                for j in 0..n {
                    m[i][j] -= f * m[col][j];
                    inv[i][j] -= f * inv[col][j];
                }
            }
        }
    }
    Some((inv, det))
}

/// The eigenvalues of a symmetric matrix in descending order, each with its unit eigenvector,
/// by Jacobi rotations.
pub(crate) fn symmetric_eigen(a: &Matrix) -> Vec<(f64, Vec<f64>)> {
    let n = a.len();
    let mut m = a.clone();
    let mut v: Matrix = (0..n)
        .map(|i| (0..n).map(|j| f64::from(u8::from(i == j))).collect())
        .collect();
    for _ in 0..100 {
        let off: f64 = (0..n)
            .flat_map(|i| (0..n).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| m[i][j] * m[i][j])
            .sum();
        let size: f64 = m.iter().flatten().map(|x| x * x).sum();
        if off <= 1e-30 * size.max(f64::MIN_POSITIVE) {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                if m[p][q] == 0.0 {
                    continue;
                }
                let theta = (m[q][q] - m[p][p]) / (2.0 * m[p][q]);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for row in m.iter_mut() {
                    let (mkp, mkq) = (row[p], row[q]);
                    row[p] = c * mkp - s * mkq;
                    row[q] = s * mkp + c * mkq;
                }
                let (row_p, row_q) = (m[p].clone(), m[q].clone());
                for k in 0..n {
                    m[p][k] = c * row_p[k] - s * row_q[k];
                    m[q][k] = s * row_p[k] + c * row_q[k];
                }
                for row in v.iter_mut() {
                    let (vp, vq) = (row[p], row[q]);
                    row[p] = c * vp - s * vq;
                    row[q] = s * vp + c * vq;
                }
            }
        }
    }
    let mut pairs: Vec<(f64, Vec<f64>)> = (0..n)
        .map(|i| (m[i][i], v.iter().map(|row| row[i]).collect()))
        .collect();
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0));
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn sweeping_gives_the_inverse_and_undoes_itself() {
        let r = vec![
            vec![1.0, 0.5, 0.3, 0.6],
            vec![0.5, 1.0, 0.2, 0.4],
            vec![0.3, 0.2, 1.0, 0.1],
            vec![0.6, 0.4, 0.1, 1.0],
        ];
        let mut m = r.clone();
        sweep(&mut m, 0);
        sweep(&mut m, 1);
        let (inv, _) = inverse(&vec![vec![1.0, 0.5], vec![0.5, 1.0]]).unwrap();
        for i in 0..2 {
            for j in 0..2 {
                assert!(close(m[i][j], inv[i][j]));
            }
        }
        // removing the first variable leaves the matrix swept on the second alone
        sweep(&mut m, 0);
        let mut only = r.clone();
        sweep(&mut only, 1);
        for i in 0..4 {
            for j in 0..4 {
                assert!(close(m[i][j], only[i][j]), "{i} {j}");
            }
        }
        // 1 - R squared of the last on the first two: 1 - b' r
        let mut full = r.clone();
        sweep(&mut full, 0);
        sweep(&mut full, 1);
        let beta = [full[3][0], full[3][1]];
        assert!(close(full[3][3], 1.0 - beta[0] * 0.6 - beta[1] * 0.4));
    }

    #[test]
    fn inverse_and_determinant() {
        let a = vec![vec![4.0, 2.0], vec![2.0, 3.0]];
        let (inv, det) = inverse(&a).unwrap();
        assert!(close(det, 8.0));
        assert!(close(inv[0][0], 3.0 / 8.0) && close(inv[0][1], -0.25));
        assert!(inverse(&vec![vec![1.0, 2.0], vec![2.0, 4.0]]).is_none());
    }

    #[test]
    fn eigenvalues_of_a_symmetric_matrix() {
        let a = vec![
            vec![2.0, 1.0, 0.0],
            vec![1.0, 2.0, 0.0],
            vec![0.0, 0.0, 5.0],
        ];
        let pairs = symmetric_eigen(&a);
        let values: Vec<f64> = pairs.iter().map(|p| p.0).collect();
        assert!(close(values[0], 5.0) && close(values[1], 3.0) && close(values[2], 1.0));
        let v = &pairs[1].1;
        assert!(close(v[0].abs(), 0.5_f64.sqrt()) && close(v[0], v[1]));
    }
}
