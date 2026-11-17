//! CROSSTABS /STATISTICS measures of association: phi, Cramer's V, the contingency coefficient,
//! lambda, Goodman and Kruskal's tau, Kendall's tau-b and tau-c, gamma, Pearson's r and
//! Spearman's correlation.
//!
//! Rules, in our words, from the Algorithms manual. f is a cell's weighted count, r and c the
//! row and column totals, W the grand total, rows and columns in ascending order of value.
//! - Phi is the root of chi-square over W, with the sign of the correlation in a 2 x 2 table;
//!   Cramer's V divides chi-square by W (q - 1) first, q the smaller of the row and column
//!   counts; the contingency coefficient is the root of chi-square over chi-square plus W. Their
//!   significance is that of the Pearson chi-square.
//! - Lambda for the column variable: the sum over rows of the largest cell, less the largest
//!   column total, over W less that total; for the row variable the same with rows and columns
//!   swapped; the symmetric lambda pools both. Standard errors as the manual gives them, the
//!   largest cell or total being the first one in order when several are equal.
//! - Goodman and Kruskal's tau for the column variable: (W sum f^2 / r - sum c^2) over
//!   (W^2 - sum c^2); (W - 1)(C - 1) tau is chi-square with (R - 1)(C - 1) degrees of freedom.
//! - From the concordant (P) and discordant (Q) pair counts, both doubled: tau-b is (P - Q)
//!   over the root of (W^2 - sum r^2)(W^2 - sum c^2), tau-c is q (P - Q) / (W^2 (q - 1)), gamma
//!   is (P - Q) / (P + Q).
//! - Pearson's r uses the category values, Spearman's correlation the mid ranks of the
//!   categories, both for numeric variables only; their t is r root(W - 2) / root(1 - r^2) with W - 2 degrees of freedom.
//! - Kappa, on the square table over the categories of both variables (a pair of values never
//!   seen counts 0): (W sum f_ii - sum r_i c_i) / (W^2 - sum r_i c_i), with the manual's
//!   variance for the standard error and the one under independence for T.
//! - The risk estimate of a 2 x 2 table: the odds ratio f11 f22 / (f12 f21) and, for each
//!   column, the ratio of the two rows' shares in it, each with a 95% interval exp(+-1.96 v) on
//!   the log scale.
//! - The uncertainty coefficient for the column variable: (U(X) + U(Y) - U(XY)) / U(Y) with
//!   the entropies U of the row totals, the column totals and the cells; symmetric, twice the
//!   gain over U(X) + U(Y). Its significance is that of the likelihood-ratio chi-square.
//! - McNemar: on the square table, for 2 x 2 the exact binomial test of the two off-diagonal
//!   counts; for larger tables the sum over pairs i < j with counts of (f_ij - f_ji)^2 /
//!   (f_ij + f_ji), chi-square with n (n - 1) / 2 degrees of freedom.
//! - Somers' d for the column variable: (P - Q) over W^2 less the sum of squared row totals;
//!   for the row variable the same with the column totals; symmetric, over the mean of both.
//! - Eta (numeric data) for the column variable: the root of 1 less the sum of squares of the
//!   column values within the rows over their total sum of squares.
//! - CMH, over the strata (layers) of 2 x 2 tables: Cochran's statistic sums f11 - E11 over
//!   the strata with both rows filled and divides by the root of sum r1 r2 / n p (1 - p), p the
//!   first column's share; Mantel-Haenszel's takes |sum| - 0.5 and n - 1 in place of n, over
//!   strata of more than one case; both squared, chi-square with 1 degree of freedom. The
//!   common odds ratio is sum f11 f22 / n over sum f12 f21 / n, its log's variance that of
//!   Robins, Breslow and Greenland; Breslow-Day sums (f11 - F)^2 / V over the strata, F the
//!   first cell that gives the stratum the common odds ratio with its margins and V the
//!   inverse of the sum of the inverse fitted cells; Tarone subtracts (sum (f11 - F))^2 / sum V;
//!   both chi-square with one less degree of freedom than strata.
//! - The approximate T of lambda, the uncertainty coefficient, Somers' d, tau-b, tau-c and gamma is the estimate over its standard
//!   error under independence, its significance two-sided normal. The standard error shown is
//!   the one without that assumption.

// the formulas index cells by row i and column j, as the manual writes them
#![allow(clippy::needless_range_loop)]

use crate::dist::{chi_square_p, normal_p, t_p};
use statscraft_syntax::CrosstabsMeasure as M;

/// One line of a measures table: its group ("Nominal by Nominal"), name, the dependent
/// variable for a directional measure (0 symmetric, 1 rows, 2 columns), value, asymptotic
/// standard error, approximate T and approximate significance.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Line {
    pub(crate) group: &'static str,
    pub(crate) name: &'static str,
    pub(crate) dependent: u8,
    pub(crate) value: f64,
    pub(crate) ase: Option<f64>,
    pub(crate) t: Option<f64>,
    pub(crate) sig: Option<f64>,
}

/// A two-way table of weighted counts, its categories in ascending order, with the values of
/// the categories when both variables are numeric.
pub(crate) struct Counts<'a> {
    pub(crate) f: &'a [Vec<f64>],
    pub(crate) values: Option<(Vec<f64>, Vec<f64>)>,
}

fn line(group: &'static str, name: &'static str, dependent: u8, value: f64) -> Line {
    Line {
        group,
        name,
        dependent,
        value,
        ase: None,
        t: None,
        sig: None,
    }
}

/// The first index of the largest value.
fn first_max(values: impl Iterator<Item = f64>) -> usize {
    let mut best = (0, f64::NEG_INFINITY);
    for (k, v) in values.enumerate() {
        if v > best.1 {
            best = (k, v);
        }
    }
    best.0
}

/// A value, its standard error and its standard error under independence: T and its two-sided
/// normal significance come from the latter.
fn normal_line(
    group: &'static str,
    name: &'static str,
    dependent: u8,
    (value, ase1, ase0): (f64, f64, f64),
) -> Line {
    let t = value / ase0;
    Line {
        ase: Some(ase1),
        t: t.is_finite().then_some(t),
        sig: t.is_finite().then(|| normal_p(t)),
        ..line(group, name, dependent, value)
    }
}

impl Counts<'_> {
    fn dims(&self) -> (usize, usize) {
        (self.f.len(), self.f.first().map_or(0, Vec::len))
    }

    fn row_totals(&self) -> Vec<f64> {
        self.f.iter().map(|row| row.iter().sum()).collect()
    }

    fn column_totals(&self) -> Vec<f64> {
        let (_, c) = self.dims();
        (0..c)
            .map(|j| self.f.iter().map(|row| row[j]).sum())
            .collect()
    }

    fn transposed(&self) -> Vec<Vec<f64>> {
        let (r, c) = self.dims();
        (0..c)
            .map(|j| (0..r).map(|i| self.f[i][j]).collect())
            .collect()
    }

    /// The symmetric measures asked for, in the order of the table.
    pub(crate) fn symmetric(&self, asked: &[M]) -> Vec<Line> {
        let (r, c) = self.dims();
        let rows = self.row_totals();
        let columns = self.column_totals();
        let w: f64 = rows.iter().sum();
        let mut out = Vec::new();
        let chi = || {
            let mut x = 0.0;
            for i in 0..r {
                for j in 0..c {
                    let e = rows[i] * columns[j] / w;
                    x += (self.f[i][j] - e).powi(2) / e;
                }
            }
            (x, chi_square_p(x, ((r - 1) * (c - 1)) as f64))
        };
        let nominal = "Nominal by Nominal";
        if asked.contains(&M::Phi) {
            let (x, p) = chi();
            let mut phi = (x / w).sqrt();
            if (r, c) == (2, 2) && self.f[0][0] * self.f[1][1] < self.f[0][1] * self.f[1][0] {
                phi = -phi;
            }
            let q = r.min(c) as f64;
            let v = (x / (w * (q - 1.0))).sqrt();
            out.push(Line {
                sig: Some(p),
                ..line(nominal, "Phi", 0, phi)
            });
            out.push(Line {
                sig: Some(p),
                ..line(nominal, "Cramer's V", 0, v)
            });
        }
        if asked.contains(&M::Cc) {
            let (x, p) = chi();
            out.push(Line {
                sig: Some(p),
                ..line(nominal, "Contingency Coefficient", 0, (x / (x + w)).sqrt())
            });
        }
        let ordinal = "Ordinal by Ordinal";
        let pairs = || self.pairs(&rows, &columns, w);
        if asked.contains(&M::Btau) {
            out.push(normal_line(ordinal, "Kendall's tau-b", 0, pairs().tau_b()));
        }
        if asked.contains(&M::Ctau) {
            out.push(normal_line(ordinal, "Kendall's tau-c", 0, pairs().tau_c()));
        }
        if asked.contains(&M::Gamma) {
            out.push(normal_line(ordinal, "Gamma", 0, pairs().gamma()));
        }
        if let (true, Some((x, y))) = (asked.contains(&M::Corr), &self.values) {
            let ranks = |totals: &[f64]| -> Vec<f64> {
                let mut below = 0.0;
                totals
                    .iter()
                    .map(|n| {
                        let rank = below + (n + 1.0) / 2.0;
                        below += n;
                        rank
                    })
                    .collect()
            };
            out.push(self.correlation(
                ordinal,
                "Spearman Correlation",
                (&ranks(&rows), &ranks(&columns)),
                (&rows, &columns, w),
            ));
            out.push(self.correlation(
                "Interval by Interval",
                "Pearson's R",
                (x, y),
                (&rows, &columns, w),
            ));
        }
        out
    }

    /// The directional measures asked for: lambda (symmetric, rows, columns dependent) and
    /// Goodman and Kruskal's tau (rows, columns dependent).
    pub(crate) fn directional(&self, asked: &[M]) -> Vec<Line> {
        let group = "Nominal by Nominal";
        let t = self.transposed();
        let flipped = Counts {
            f: &t,
            values: None,
        };
        let mut out = Vec::new();
        if asked.contains(&M::Lambda) {
            out.extend([
                normal_line(group, "Lambda", 0, self.lambda_symmetric()),
                normal_line(group, "Lambda", 1, flipped.lambda()),
                normal_line(group, "Lambda", 2, self.lambda()),
            ]);
            for (dependent, counts) in [(1, &flipped), (2, self)] {
                let (value, ase, x, df) = counts.goodman_kruskal();
                out.push(Line {
                    ase: Some(ase),
                    sig: Some(chi_square_p(x, df)),
                    ..line(group, "Goodman and Kruskal tau", dependent, value)
                });
            }
        }
        if asked.contains(&M::Uc) {
            let p = self.likelihood_ratio_p();
            let name = "Uncertainty Coefficient";
            for (dependent, value) in [
                (0, self.uncertainty_symmetric()),
                (1, flipped.uncertainty()),
                (2, self.uncertainty()),
            ] {
                out.push(Line {
                    sig: Some(p),
                    ..normal_line(group, name, dependent, value)
                });
            }
        }
        if asked.contains(&M::D) {
            let rows = self.row_totals();
            let columns = self.column_totals();
            let w: f64 = rows.iter().sum();
            let pairs = self.pairs(&rows, &columns, w);
            let group = "Ordinal by Ordinal";
            for (dependent, value) in [
                (0, pairs.somers_symmetric()),
                (1, pairs.somers(false)),
                (2, pairs.somers(true)),
            ] {
                out.push(normal_line(group, "Somers' d", dependent, value));
            }
        }
        if let (true, Some((x, y))) = (asked.contains(&M::Eta), &self.values) {
            let group = "Nominal by Interval";
            out.push(line(group, "Eta", 1, self.eta(x, true)));
            out.push(line(group, "Eta", 2, self.eta(y, false)));
        }
        out
    }

    /// Eta with the row variable (values `scores`, `rows` true) or the column variable as the
    /// dependent one: the root of 1 - (the sum of squares within the categories of the other
    /// variable) / (the total sum of squares).
    fn eta(&self, scores: &[f64], rows: bool) -> f64 {
        let f: Vec<Vec<f64>> = if rows {
            self.transposed()
        } else {
            self.f.to_vec()
        };
        let mut total = (0.0, 0.0, 0.0);
        let mut within = 0.0;
        for line in &f {
            let n: f64 = line.iter().sum();
            let s: f64 = line.iter().zip(scores).map(|(c, y)| c * y).sum();
            let s2: f64 = line.iter().zip(scores).map(|(c, y)| c * y * y).sum();
            total = (total.0 + n, total.1 + s, total.2 + s2);
            if n > 0.0 {
                within += s2 - s * s / n;
            }
        }
        let spread = total.2 - total.1 * total.1 / total.0;
        (1.0 - within / spread).sqrt()
    }

    /// The entropies of the rows, the columns and the cells, and the sum P of the manual.
    fn entropies(&self) -> (f64, f64, f64, f64, f64) {
        let (r, c) = self.dims();
        let rows = self.row_totals();
        let columns = self.column_totals();
        let w: f64 = rows.iter().sum();
        let h = |n: f64| {
            if n > 0.0 {
                -(n / w) * (n / w).ln()
            } else {
                0.0
            }
        };
        let ux: f64 = rows.iter().map(|&n| h(n)).sum();
        let uy: f64 = columns.iter().map(|&n| h(n)).sum();
        let mut uxy = 0.0;
        let mut p = 0.0;
        for i in 0..r {
            for j in 0..c {
                let f = self.f[i][j];
                if f > 0.0 {
                    uxy += h(f);
                    p += f * (columns[j] * rows[i] / (w * f)).ln().powi(2);
                }
            }
        }
        (ux, uy, uxy, p, w)
    }

    /// The uncertainty coefficient for predicting the column category: value, standard error
    /// and that under independence.
    fn uncertainty(&self) -> (f64, f64, f64) {
        let (r, c) = self.dims();
        let rows = self.row_totals();
        let columns = self.column_totals();
        let (ux, uy, uxy, p, w) = self.entropies();
        let gain = ux + uy - uxy;
        let mut s = 0.0;
        for i in 0..r {
            for j in 0..c {
                let f = self.f[i][j];
                if f > 0.0 {
                    let term = uy * (f / rows[i]).ln() + (ux - uxy) * (columns[j] / w).ln();
                    s += f * term * term;
                }
            }
        }
        let ase1 = s.sqrt() / (w * uy * uy);
        let ase0 = (p - w * gain * gain).sqrt() / (w * uy);
        (gain / uy, ase1, ase0)
    }

    fn uncertainty_symmetric(&self) -> (f64, f64, f64) {
        let (r, c) = self.dims();
        let rows = self.row_totals();
        let columns = self.column_totals();
        let (ux, uy, uxy, p, w) = self.entropies();
        let gain = ux + uy - uxy;
        let mut s = 0.0;
        for i in 0..r {
            for j in 0..c {
                let f = self.f[i][j];
                if f > 0.0 {
                    let term =
                        uxy * (rows[i] * columns[j] / (w * w)).ln() - (ux + uy) * (f / w).ln();
                    s += f * term * term;
                }
            }
        }
        let ase1 = 2.0 * s.sqrt() / (w * (ux + uy).powi(2));
        let ase0 = 2.0 * (p - w * gain * gain).sqrt() / (w * (ux + uy));
        (2.0 * gain / (ux + uy), ase1, ase0)
    }

    /// The significance of the likelihood-ratio chi-square of the table.
    fn likelihood_ratio_p(&self) -> f64 {
        let (r, c) = self.dims();
        let rows = self.row_totals();
        let columns = self.column_totals();
        let w: f64 = rows.iter().sum();
        let mut g = 0.0;
        for i in 0..r {
            for j in 0..c {
                let f = self.f[i][j];
                if f > 0.0 {
                    g += 2.0 * f * (f * w / (rows[i] * columns[j])).ln();
                }
            }
        }
        chi_square_p(g, ((r - 1) * (c - 1)) as f64)
    }

    /// Lambda for predicting the column category: value, standard error, and that under
    /// independence.
    fn lambda(&self) -> (f64, f64, f64) {
        let (r, c) = self.dims();
        let columns = self.column_totals();
        let w: f64 = columns.iter().sum();
        let m = first_max(columns.iter().copied());
        let cm = columns[m];
        let best: Vec<usize> = self
            .f
            .iter()
            .map(|row| first_max(row.iter().copied()))
            .collect();
        let sum_max: f64 = (0..r).map(|i| self.f[i][best[i]]).sum();
        let lambda = (sum_max - cm) / (w - cm);
        let (mut s1, mut s0) = (0.0, 0.0);
        for i in 0..r {
            for j in 0..c {
                let dij = f64::from(u8::from(best[i] == j));
                let dj = f64::from(u8::from(m == j));
                s1 += self.f[i][j] * (dij - dj + lambda * dj).powi(2);
                s0 += self.f[i][j] * (dij - dj).powi(2);
            }
        }
        let ase1 = (s1 - w * lambda * lambda).sqrt() / (w - cm);
        let ase0 = (s0 - (sum_max - cm).powi(2) / w).sqrt() / (w - cm);
        (lambda, ase1, ase0)
    }

    fn lambda_symmetric(&self) -> (f64, f64, f64) {
        let (r, c) = self.dims();
        let rows = self.row_totals();
        let columns = self.column_totals();
        let w: f64 = rows.iter().sum();
        let (rm_at, cm_at) = (
            first_max(rows.iter().copied()),
            first_max(columns.iter().copied()),
        );
        let (rm, cm) = (rows[rm_at], columns[cm_at]);
        // the column of the largest cell of each row, the row of the largest cell of each column
        let in_row: Vec<usize> = self
            .f
            .iter()
            .map(|row| first_max(row.iter().copied()))
            .collect();
        let in_column: Vec<usize> = (0..c)
            .map(|j| first_max(self.f.iter().map(|row| row[j])))
            .collect();
        let sum_rows: f64 = (0..r).map(|i| self.f[i][in_row[i]]).sum();
        let sum_columns: f64 = (0..c).map(|j| self.f[in_column[j]][j]).sum();
        let gain = sum_rows + sum_columns - cm - rm;
        let denominator = 2.0 * w - rm - cm;
        let lambda = gain / denominator;
        let (mut s1, mut s0) = (0.0, 0.0);
        for i in 0..r {
            for j in 0..c {
                let d = |b: bool| f64::from(u8::from(b));
                let (rij, cij) = (d(in_column[j] == i), d(in_row[i] == j));
                let (ri, cj) = (d(rm_at == i), d(cm_at == j));
                let base = rij + cij - ri - cj;
                s1 += self.f[i][j] * (base + lambda * (ri + cj)).powi(2);
                s0 += self.f[i][j] * base.powi(2);
            }
        }
        let ase1 = (s1 - 4.0 * w * lambda * lambda).sqrt() / denominator;
        let ase0 = (s0 - gain * gain / w).sqrt() / denominator;
        (lambda, ase1, ase0)
    }

    /// Goodman and Kruskal's tau for predicting the column category: value, standard error,
    /// and the chi-square with its degrees of freedom for the significance.
    fn goodman_kruskal(&self) -> (f64, f64, f64, f64) {
        let (r, c) = self.dims();
        let rows = self.row_totals();
        let columns = self.column_totals();
        let w: f64 = rows.iter().sum();
        let sum_c2: f64 = columns.iter().map(|x| x * x).sum();
        let delta = w * w - sum_c2;
        let mut within = 0.0;
        for i in 0..r {
            for j in 0..c {
                within += self.f[i][j] * self.f[i][j] / rows[i];
            }
        }
        let v = w * within - sum_c2;
        let tau = v / delta;
        let mut s = 0.0;
        for i in 0..r {
            let fc: f64 = (0..c).map(|j| self.f[i][j] * columns[j]).sum::<f64>() / rows[i];
            let f2: f64 = (0..c).map(|j| self.f[i][j].powi(2)).sum::<f64>() / rows[i].powi(2);
            for j in 0..c {
                let term =
                    (v - delta) * (fc - columns[j]) - w * delta * (f2 - self.f[i][j] / rows[i]);
                s += self.f[i][j] * term * term;
            }
        }
        let ase = (4.0 / delta.powi(4) * s).sqrt();
        let x = (w - 1.0) * (c as f64 - 1.0) * tau;
        (tau, ase, x, ((r - 1) * (c - 1)) as f64)
    }

    /// The concordance of every cell with the others.
    fn pairs(&self, rows: &[f64], columns: &[f64], w: f64) -> Pairs {
        let (r, c) = self.dims();
        // s[a][b]: the sum of the cells above row a and left of column b, so each quadrant
        // around a cell is a few lookups and large tables stay fast
        let mut s = vec![vec![0.0; c + 1]; r + 1];
        for i in 0..r {
            for j in 0..c {
                s[i + 1][j + 1] = self.f[i][j] + s[i][j + 1] + s[i + 1][j] - s[i][j];
            }
        }
        let mut concordant = vec![vec![0.0; c]; r];
        let mut discordant = vec![vec![0.0; c]; r];
        for i in 0..r {
            for j in 0..c {
                let above_left = s[i][j];
                let above_right = s[i][c] - s[i][j + 1];
                let below_left = s[r][j] - s[i + 1][j];
                let below_right = s[r][c] - s[i + 1][c] - s[r][j + 1] + s[i + 1][j + 1];
                concordant[i][j] = above_left + below_right;
                discordant[i][j] = above_right + below_left;
            }
        }
        let mut p = 0.0;
        let mut q = 0.0;
        for i in 0..r {
            for j in 0..c {
                p += self.f[i][j] * concordant[i][j];
                q += self.f[i][j] * discordant[i][j];
            }
        }
        Pairs {
            f: self.f.to_vec(),
            concordant,
            discordant,
            p,
            q,
            rows: rows.to_vec(),
            columns: columns.to_vec(),
            w,
        }
    }

    /// Pearson's r of the scores x of the rows and y of the columns, with its standard error
    /// and the t test.
    fn correlation(
        &self,
        group: &'static str,
        name: &'static str,
        (x, y): (&[f64], &[f64]),
        (rows, columns, w): (&[f64], &[f64], f64),
    ) -> Line {
        let (r, c) = self.dims();
        let sx: f64 = (0..r).map(|i| x[i] * rows[i]).sum();
        let sy: f64 = (0..c).map(|j| y[j] * columns[j]).sum();
        let (mx, my) = (sx / w, sy / w);
        let ssx = (0..r).map(|i| x[i] * x[i] * rows[i]).sum::<f64>() - sx * sx / w;
        let ssy = (0..c).map(|j| y[j] * y[j] * columns[j]).sum::<f64>() - sy * sy / w;
        let mut cov = -sx * sy / w;
        for i in 0..r {
            for j in 0..c {
                cov += x[i] * y[j] * self.f[i][j];
            }
        }
        let scale = (ssx * ssy).sqrt();
        let rho = cov / scale;
        let mut var = 0.0;
        for i in 0..r {
            for j in 0..c {
                let (dx, dy) = (x[i] - mx, y[j] - my);
                let term = scale * dx * dy - cov / (2.0 * scale) * (dx * dx * ssy + dy * dy * ssx);
                var += self.f[i][j] * term * term;
            }
        }
        let ase = (var / scale.powi(4)).sqrt();
        let t = rho * (w - 2.0).sqrt() / (1.0 - rho * rho).sqrt();
        Line {
            ase: Some(ase),
            t: t.is_finite().then_some(t),
            sig: t.is_finite().then(|| t_p(t, w - 2.0)),
            ..line(group, name, 0, rho)
        }
    }
}

impl Counts<'_> {
    /// Cohen's kappa of a square table: value, standard error, T and significance.
    pub(crate) fn kappa(&self) -> Line {
        let (n, _) = self.dims();
        let rows = self.row_totals();
        let columns = self.column_totals();
        let w: f64 = rows.iter().sum();
        let diagonal: f64 = (0..n).map(|i| self.f[i][i]).sum();
        let chance: f64 = (0..n).map(|i| rows[i] * columns[i]).sum();
        let both: f64 = (0..n).map(|i| self.f[i][i] * (rows[i] + columns[i])).sum();
        let spread = w * w - chance;
        let kappa = (w * diagonal - chance) / spread;
        let mut cross = 0.0;
        for i in 0..n {
            for j in 0..n {
                cross += self.f[i][j] * (rows[j] + columns[i]).powi(2);
            }
        }
        let miss = w - diagonal;
        let var1 = w
            * (diagonal * miss / spread.powi(2)
                + 2.0 * miss * (2.0 * diagonal * chance - w * both) / spread.powi(3)
                + miss * miss * (w * cross - 4.0 * chance * chance) / spread.powi(4));
        let by_chance: f64 = (0..n)
            .map(|i| rows[i] * columns[i] * (rows[i] + columns[i]))
            .sum();
        let var0 = (w * w * chance + chance * chance - w * by_chance) / (w * spread * spread);
        let t = kappa / var0.sqrt();
        Line {
            ase: Some(var1.sqrt()),
            t: t.is_finite().then_some(t),
            sig: t.is_finite().then(|| normal_p(t)),
            ..line("Measure of Agreement", "Kappa", 0, kappa)
        }
    }
}

/// The McNemar test of a square table: for 2 x 2 the exact two-sided binomial significance of
/// the off-diagonal counts made whole (`truncate` or rounded); for larger tables the
/// McNemar-Bowker chi-square with n (n - 1) / 2 degrees of freedom. Name, value, df,
/// asymptotic and exact two-sided significance.
pub(crate) fn mcnemar(
    f: &[Vec<f64>],
    whole: impl Fn(f64) -> f64,
) -> (&'static str, [Option<f64>; 4]) {
    let n = f.len();
    if n == 2 {
        let (b, c) = (whole(f[0][1]), whole(f[1][0]));
        let total = b + c;
        let ln_choose = |k: f64| {
            crate::dist::ln_gamma(total + 1.0)
                - crate::dist::ln_gamma(k + 1.0)
                - crate::dist::ln_gamma(total - k + 1.0)
        };
        let tail: f64 = (0..=b.min(c) as u64)
            .map(|k| (ln_choose(k as f64) - total * std::f64::consts::LN_2).exp())
            .sum();
        return (
            "McNemar Test",
            [None, None, None, Some((2.0 * tail).min(1.0))],
        );
    }
    let mut x = 0.0;
    for i in 0..n {
        for j in i + 1..n {
            let pair = f[i][j] + f[j][i];
            if pair > 0.0 {
                x += (f[i][j] - f[j][i]).powi(2) / pair;
            }
        }
    }
    let df = (n * (n - 1) / 2) as f64;
    (
        "McNemar-Bowker Test",
        [Some(x), Some(df), Some(chi_square_p(x, df)), None],
    )
}

/// The tests and the estimate of CMH over strata of 2 x 2 tables [f11, f12, f21, f22].
pub(crate) struct Cmh {
    /// Breslow-Day's and Tarone's chi-squares with their degrees of freedom.
    pub(crate) homogeneity: Option<(f64, f64, f64)>,
    /// Cochran's and Mantel-Haenszel's chi-squares (1 degree of freedom).
    pub(crate) independence: (Option<f64>, Option<f64>),
    /// The common odds ratio, the standard error of its log and the significance against the
    /// null value.
    pub(crate) odds: Option<(f64, f64, f64)>,
}

pub(crate) fn cmh(strata: &[[f64; 4]], null_odds: f64) -> Cmh {
    let strata: Vec<&[f64; 4]> = strata
        .iter()
        .filter(|s| s.iter().sum::<f64>() > 0.0)
        .collect();
    let margins = |s: &[f64; 4]| {
        let [a, b, c, d] = *s;
        (a + b + c + d, a + b, c + d, a + c, b + d)
    };
    // Cochran: strata with both rows filled; Mantel-Haenszel: more than one case
    let (mut gap, mut cochran_var) = (0.0, 0.0);
    let (mut mh_gap, mut mh_var) = (0.0, 0.0);
    let mut cochran_any = false;
    let mut mh_any = false;
    for s in &strata {
        let (n, r1, r2, c1, c2) = margins(s);
        let p = c1 / n;
        if r1 > 0.0 && r2 > 0.0 {
            gap += s[0] - r1 * c1 / n;
            cochran_var += r1 * r2 / n * p * (1.0 - p);
            cochran_any |= c1 > 0.0 && c2 > 0.0;
        }
        if n > 1.0 {
            mh_gap += s[0] - r1 * c1 / n;
            mh_var += r1 * r2 / (n - 1.0) * p * (1.0 - p);
            mh_any |= r1 > 0.0 && r2 > 0.0 && c1 > 0.0 && c2 > 0.0;
        }
    }
    let cochran = cochran_any.then(|| gap * gap / cochran_var);
    let mantel = mh_any.then(|| (mh_gap.abs() - 0.5).powi(2) / mh_var);

    // the Mantel-Haenszel common odds ratio and the variance of its log (Robins and others)
    let (mut r, mut s_, mut pr, mut ps_qr, mut qs) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for s in &strata {
        let [a, b, c, d] = **s;
        let n = a + b + c + d;
        r += a * d / n;
        s_ += b * c / n;
        pr += (a + d) * a * d / (n * n);
        ps_qr += ((a + d) * b * c + (b + c) * a * d) / (n * n);
        qs += (b + c) * b * c / (n * n);
    }
    let theta = r / s_;
    let odds = (theta.is_finite() && theta > 0.0).then(|| {
        let var = pr / (2.0 * r * r) + ps_qr / (2.0 * r * s_) + qs / (2.0 * s_ * s_);
        let se = var.sqrt();
        let z = (theta.ln() - null_odds.ln()) / se;
        (theta, se, normal_p(z))
    });

    // Breslow-Day and Tarone, from the expected first cells at the common odds ratio
    let homogeneity = odds.and_then(|(theta, _, _)| {
        let (mut b, mut sum_gap, mut sum_v) = (0.0, 0.0, 0.0);
        let mut used = false;
        for s in &strata {
            let (n, r1, _, c1, _) = margins(s);
            if r1 == 0.0 || c1 == 0.0 {
                continue;
            }
            let (low, high) = ((r1 + c1 - n).max(0.0), r1.min(c1));
            // f (n - r1 - c1 + f) = theta (r1 - f) (c1 - f), a quadratic in f
            let qa = 1.0 - theta;
            let qb = n - r1 - c1 + theta * (r1 + c1);
            let qc = -theta * r1 * c1;
            let f = if qa.abs() < 1e-12 {
                -qc / qb
            } else {
                let root = (qb * qb - 4.0 * qa * qc).max(0.0).sqrt();
                let first = (-qb + root) / (2.0 * qa);
                let second = (-qb - root) / (2.0 * qa);
                if (low - 1e-9..=high + 1e-9).contains(&first) {
                    first
                } else {
                    second
                }
            };
            if f <= 0.0 {
                continue;
            }
            let v = 1.0 / (1.0 / f + 1.0 / (r1 - f) + 1.0 / (c1 - f) + 1.0 / (n - r1 - c1 + f));
            // an empty second row or column leaves a fitted cell 0: no information
            if !(v > 0.0 && v.is_finite()) {
                continue;
            }
            b += (s[0] - f).powi(2) / v;
            sum_gap += s[0] - f;
            sum_v += v;
            used = true;
        }
        let df = strata.len() as f64 - 1.0;
        used.then(|| (b, b - sum_gap * sum_gap / sum_v, df))
    });
    Cmh {
        homogeneity,
        independence: (cochran, mantel),
        odds,
    }
}

/// The risk estimates of a 2 x 2 table: the odds ratio and the risk ratios for the first and
/// the second column, each as value, lower and upper 95% limit.
pub(crate) fn risk(f: &[Vec<f64>]) -> [(f64, f64, f64); 3] {
    let z = crate::dist::normal_quantile(0.975);
    let interval = |value: f64, v: f64| (value, value * (-z * v).exp(), value * (z * v).exp());
    let (a, b, c, d) = (f[0][0], f[0][1], f[1][0], f[1][1]);
    let odds = interval(
        a * d / (b * c),
        (1.0 / a + 1.0 / b + 1.0 / c + 1.0 / d).sqrt(),
    );
    let first = interval(
        a * (c + d) / (c * (a + b)),
        (b / (a * (a + b)) + d / (c * (c + d))).sqrt(),
    );
    let second = interval(
        b * (c + d) / (d * (a + b)),
        (a / (b * (a + b)) + c / (d * (c + d))).sqrt(),
    );
    [odds, first, second]
}

/// The pair counts behind tau-b, tau-c and gamma.
struct Pairs {
    f: Vec<Vec<f64>>,
    concordant: Vec<Vec<f64>>,
    discordant: Vec<Vec<f64>>,
    p: f64,
    q: f64,
    rows: Vec<f64>,
    columns: Vec<f64>,
    w: f64,
}

impl Pairs {
    fn cells(&self) -> impl Iterator<Item = (f64, f64, f64, usize, usize)> + '_ {
        self.f.iter().enumerate().flat_map(move |(i, row)| {
            row.iter()
                .enumerate()
                .map(move |(j, &f)| (f, self.concordant[i][j], self.discordant[i][j], i, j))
        })
    }

    /// The sum of f (C - D)^2 less (P - Q)^2 / W, under the root of every null standard error.
    fn spread(&self) -> f64 {
        let s: f64 = self
            .cells()
            .map(|(f, c, d, _, _)| f * (c - d).powi(2))
            .sum();
        s - (self.p - self.q).powi(2) / self.w
    }

    fn tau_b(&self) -> (f64, f64, f64) {
        let dr = self.w * self.w - self.rows.iter().map(|x| x * x).sum::<f64>();
        let dc = self.w * self.w - self.columns.iter().map(|x| x * x).sum::<f64>();
        let root = (dr * dc).sqrt();
        let tau = (self.p - self.q) / root;
        let s: f64 = self
            .cells()
            .map(|(f, c, d, i, j)| {
                let v = self.rows[i] * dc + self.columns[j] * dr;
                f * (2.0 * root * (c - d) + tau * v).powi(2)
            })
            .sum();
        let ase1 = (s - self.w.powi(3) * tau * tau * (dr + dc).powi(2)).sqrt() / (dr * dc);
        let ase0 = 2.0 * (self.spread() / (dr * dc)).sqrt();
        (tau, ase1, ase0)
    }

    /// Somers' d with the column variable (`columns` true) or the row variable dependent.
    fn somers(&self, columns: bool) -> (f64, f64, f64) {
        let totals = if columns { &self.rows } else { &self.columns };
        let dt = self.w * self.w - totals.iter().map(|x| x * x).sum::<f64>();
        let d = (self.p - self.q) / dt;
        let s: f64 = self
            .cells()
            .map(|(f, c, dis, i, j)| {
                let other = if columns {
                    self.rows[i]
                } else {
                    self.columns[j]
                };
                f * (dt * (c - dis) - (self.p - self.q) * (self.w - other)).powi(2)
            })
            .sum();
        let ase1 = 2.0 / (dt * dt) * s.sqrt();
        let ase0 = 2.0 / dt * self.spread().sqrt();
        (d, ase1, ase0)
    }

    fn somers_symmetric(&self) -> (f64, f64, f64) {
        let dr = self.w * self.w - self.rows.iter().map(|x| x * x).sum::<f64>();
        let dc = self.w * self.w - self.columns.iter().map(|x| x * x).sum::<f64>();
        let d = (self.p - self.q) / ((dr + dc) / 2.0);
        // d is tau-b scaled by 2 root(Dr Dc) / (Dr + Dc); so is its standard error
        let (_, tau_ase, _) = self.tau_b();
        let ase1 = 2.0 * tau_ase * (dr * dc).sqrt() / (dr + dc);
        let ase0 = 4.0 / (dc + dr) * self.spread().sqrt();
        (d, ase1, ase0)
    }

    fn tau_c(&self) -> (f64, f64, f64) {
        let q = self.rows.len().min(self.columns.len()) as f64;
        let tau = q * (self.p - self.q) / (self.w * self.w * (q - 1.0));
        let ase = 2.0 * q / ((q - 1.0) * self.w * self.w) * self.spread().sqrt();
        (tau, ase, ase)
    }

    fn gamma(&self) -> (f64, f64, f64) {
        let total = self.p + self.q;
        let gamma = (self.p - self.q) / total;
        let s: f64 = self
            .cells()
            .map(|(f, c, d, _, _)| f * (self.q * c - self.p * d).powi(2))
            .sum();
        let ase1 = 4.0 / (total * total) * s.sqrt();
        let ase0 = 2.0 / total * self.spread().sqrt();
        (gamma, ase1, ase0)
    }
}
