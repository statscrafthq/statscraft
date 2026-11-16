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
//! - The approximate T of lambda, tau-b, tau-c and gamma is the estimate over its standard
//!   error under independence, its significance two-sided normal. The standard error shown is
//!   the one without that assumption.

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
        if !asked.contains(&M::Lambda) {
            return Vec::new();
        }
        let group = "Nominal by Nominal";
        let t = self.transposed();
        let flipped = Counts {
            f: &t,
            values: None,
        };
        let mut out = vec![
            normal_line(group, "Lambda", 0, self.lambda_symmetric()),
            normal_line(group, "Lambda", 1, flipped.lambda()),
            normal_line(group, "Lambda", 2, self.lambda()),
        ];
        for (dependent, counts) in [(1, &flipped), (2, self)] {
            let (value, ase, x, df) = counts.goodman_kruskal();
            out.push(Line {
                ase: Some(ase),
                sig: Some(chi_square_p(x, df)),
                ..line(group, "Goodman and Kruskal tau", dependent, value)
            });
        }
        out
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
        let mut concordant = vec![vec![0.0; c]; r];
        let mut discordant = vec![vec![0.0; c]; r];
        for i in 0..r {
            for j in 0..c {
                for h in 0..r {
                    for k in 0..c {
                        let f = self.f[h][k];
                        if (h < i && k < j) || (h > i && k > j) {
                            concordant[i][j] += f;
                        } else if (h < i && k > j) || (h > i && k < j) {
                            discordant[i][j] += f;
                        }
                    }
                }
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
