//! BAYES LOGLINEAR: Bayes factors on whether the row and column variables of a two-way table
//! are independent, simulated posteriors of the interaction terms, and the crosstab with its
//! tests.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - The table counts the cases valid on both variables (user-missing values left out) with
//!   their rounded frequency weights. Only categories that occur form rows and columns. Bayes
//!   inference needs two rows and two columns at least; with fewer a warning says so.
//! - Conjugate priors (Gunel and Dickey) with the gamma shapes a of SHAPEPARAMS: the
//!   multinomial factor is a ratio of multivariate beta functions of the counts, the row and
//!   column totals and a; the prior sums of the null model are the row sums of a less S - 1
//!   and the column sums less R - 1, and the factor is missing when one of those is 0 or
//!   below. Fixed row totals swap the null part of the rows for the Dirichlet of the row sums
//!   of a. The Poisson model multiplies the factor of the fixed grand total by the ratio of
//!   the negative binomial laws of the total under both models, with the gamma scale b
//!   (default R S min(a) / N).
//! - Good's mixture: each set of counts (cells, row totals, column totals) is a sequence under a
//!   symmetric Dirichlet(k) with k mixed over a log-Cauchy density; BF10 divides the integral
//!   of the cells by those of the margins. With fixed column totals the column integral uses
//!   k / R; fixed rows swap the roles.
//! - Intrinsic priors (Casella and Moreno), 2 x 2 tables only: finite sums over all training
//!   tables z of size t (min(N, 500)) for the fixed grand total, or with the row totals of z
//!   min(row total, 5000) for fixed rows. Larger tables fall back to Good's mixture with a
//!   warning.
//! - Nonparametric (Quintana): two rows with fixed row totals (or two columns with fixed
//!   column totals); BF01 compares one Dirichlet(lambda) for both rows with one for each row.
//! - Posterior: draws of the cell probabilities from Dirichlet(counts + 1) give the
//!   interaction terms, log odds ratios against the last row and column; Besag's order
//!   statistics give the simultaneous credible region.
//! - PRINT: the crosstab with CELLS, and the Pearson chi-square (with the continuity correction
//!   for a 2 x 2 table, and Fisher's exact test when an expected count is below 5) for CHISQ,
//!   the likelihood ratio for LRT.

use super::*;
use crate::crosstabs::{Block, categories, cell_name};
use statscraft_syntax::{BayesLoglinear, CountModel, CrosstabsCell, FixedMargin, TablePrior};

type Counts = Vec<Vec<f64>>;

fn transpose(m: &Counts) -> Counts {
    (0..m[0].len())
        .map(|j| m.iter().map(|row| row[j]).collect())
        .collect()
}

fn row_sums(m: &Counts) -> Vec<f64> {
    m.iter().map(|row| row.iter().sum()).collect()
}

fn column_sums(m: &Counts) -> Vec<f64> {
    row_sums(&transpose(m))
}

fn plus(a: &[f64], b: &[f64]) -> Vec<f64> {
    a.iter().zip(b).map(|(x, y)| x + y).collect()
}

/// ln of the multivariate beta function.
fn ln_b(values: &[f64]) -> f64 {
    values.iter().map(|&v| ln_gamma(v)).sum::<f64>() - ln_gamma(values.iter().sum())
}

/// ln BF01 of the conjugate priors with the shapes `a`: the grand total fixed, or the row
/// totals when `rows_fixed`; `poisson` holds the gamma scale b of the Poisson model. None when
/// a prior sum of the null model is 0 or below.
fn ln_conjugate(y: &Counts, a: &Counts, rows_fixed: bool, poisson: Option<f64>) -> Option<f64> {
    let (r, s) = (y.len() as f64, y[0].len() as f64);
    let xi_rows: Vec<f64> = row_sums(a).iter().map(|v| v - (s - 1.0)).collect();
    let xi_columns: Vec<f64> = column_sums(a).iter().map(|v| v - (r - 1.0)).collect();
    if xi_columns
        .iter()
        .chain(if rows_fixed { &[][..] } else { &xi_rows })
        .any(|&v| v <= 0.0)
    {
        return None;
    }
    let (flat_y, flat_a) = (y.concat(), a.concat());
    let cells = ln_b(&flat_a) - ln_b(&plus(&flat_y, &flat_a));
    let columns = ln_b(&plus(&column_sums(y), &xi_columns)) - ln_b(&xi_columns);
    let rows = if rows_fixed {
        let a_rows = row_sums(a);
        ln_b(&plus(&row_sums(y), &a_rows)) - ln_b(&a_rows)
    } else {
        ln_b(&plus(&row_sums(y), &xi_rows)) - ln_b(&xi_rows)
    };
    let mut ln = cells + rows + columns;
    if let Some(b) = poisson {
        let df = (r - 1.0) * (s - 1.0);
        let (total_a, total_y): (f64, f64) = (flat_a.iter().sum(), flat_y.iter().sum());
        let x = total_a - df;
        ln += df * (1.0 / b).ln_1p() + ln_gamma(total_y + x) - ln_gamma(x) + ln_gamma(total_a)
            - ln_gamma(total_y + total_a);
    }
    Some(ln)
}

/// ln Γ(x + m) - ln Γ(x), by Stirling's series for large x where the difference of the two
/// logarithms would lose its digits.
fn ln_rising(x: f64, m: f64) -> f64 {
    if m == 0.0 {
        0.0
    } else if x < 1e5 {
        ln_gamma(x + m) - ln_gamma(x)
    } else {
        (x - 0.5) * (m / x).ln_1p() + m * (x + m).ln() - m + 1.0 / (12.0 * (x + m))
            - 1.0 / (12.0 * x)
    }
}

/// ln of Good's integral: the probability of one sequence with the category counts `m` under
/// a symmetric Dirichlet(k), with ln(k / `scale`) Cauchy with scale pi.
fn ln_good(m: &[f64], scale: f64, q: Quadrature) -> f64 {
    let (t, total) = (m.len() as f64, m.iter().sum::<f64>());
    // ln(k / scale) = pi tan(theta) turns the log-Cauchy weight into d theta / pi on a
    // finite range, where the integrand tends to finite limits at both ends
    let log_f = |theta: f64| {
        let k = scale * (PI * theta.tan()).clamp(-600.0, 600.0).exp();
        m.iter().map(|&n| ln_rising(k, n)).sum::<f64>() - ln_rising(t * k, total) - PI.ln()
    };
    log_integral(&log_f, -PI / 2.0, PI / 2.0, q.tol, q.limit)
}

/// ln BF10 of Good's mixture: the grand total fixed, or the row totals when `rows_fixed`.
fn ln_mixture(y: &Counts, rows_fixed: bool, q: Quadrature) -> f64 {
    let rows_scale = if rows_fixed { y[0].len() as f64 } else { 1.0 };
    ln_good(&y.concat(), 1.0, q)
        - ln_good(&column_sums(y), 1.0, q)
        - ln_good(&row_sums(y), rows_scale, q)
}

/// ln k! for k from 0 to `n`.
fn ln_factorials(n: usize) -> Vec<f64> {
    let mut f = vec![0.0; n + 1];
    for k in 1..=n {
        f[k] = f[k - 1] + (k as f64).ln();
    }
    f
}

/// ln of a sum of exponentials, added one at a time.
#[derive(Clone, Copy)]
struct LogSum {
    max: f64,
    sum: f64,
}

impl LogSum {
    const EMPTY: LogSum = LogSum {
        max: f64::NEG_INFINITY,
        sum: 0.0,
    };

    fn add(&mut self, x: f64) {
        if x > self.max {
            self.sum = self.sum * (self.max - x).exp() + 1.0;
            self.max = x;
        } else {
            self.sum += (x - self.max).exp();
        }
    }

    fn ln(&self) -> f64 {
        self.max + self.sum.ln()
    }
}

/// ln((z + y)!) for z from 0 to `t`.
fn shifted(y: f64, t: usize) -> Vec<f64> {
    (0..=t).map(|z| ln_gamma(z as f64 + y + 1.0)).collect()
}

/// ln BF10 of the intrinsic prior of a 2 x 2 table with the grand total fixed, over all
/// training tables of size min(N, 500).
fn ln_intrinsic_total(y: &Counts) -> f64 {
    let cells = [y[0][0], y[0][1], y[1][0], y[1][1]];
    let total: f64 = cells.iter().sum();
    let t = total.min(500.0) as usize;
    let f = ln_factorials(t);
    let with: Vec<Vec<f64>> = cells.iter().map(|&n| shifted(n, t)).collect();
    let tf = t as f64;
    let margins = [
        y[0][0] + y[0][1],
        y[1][0] + y[1][1],
        y[0][0] + y[1][0],
        y[0][1] + y[1][1],
    ];
    let constant = ln_gamma(tf + 4.0) - ln_gamma(tf + total + 4.0) + 2.0 * ln_gamma(total + 2.0)
        - 2.0 * ln_gamma(tf + 2.0)
        - margins.iter().map(|&m| ln_gamma(m + 1.0)).sum::<f64>()
        + f[t];
    let mut sum = LogSum::EMPTY;
    for a in 0..=t {
        for b in 0..=t - a {
            for c in 0..=t - a - b {
                let d = t - a - b - c;
                sum.add(
                    f[a + b] + f[c + d] + f[a + c] + f[b + d] - 2.0 * (f[a] + f[b] + f[c] + f[d])
                        + with[0][a]
                        + with[1][b]
                        + with[2][c]
                        + with[3][d],
                );
            }
        }
    }
    constant + sum.ln()
}

/// ln BF01 of the intrinsic prior of a 2 x 2 table with fixed row totals; the training tables
/// have the row totals min(row total, 5000).
fn ln_intrinsic_rows(y: &Counts) -> f64 {
    let rows = [y[0][0] + y[0][1], y[1][0] + y[1][1]];
    let total = rows[0] + rows[1];
    let t = rows.map(|n| n.min(5000.0) as usize);
    let f = ln_factorials(t[0] + t[1]);
    let with: Vec<Vec<f64>> = [(0, 0), (0, 1), (1, 0), (1, 1)]
        .iter()
        .map(|&(i, j)| shifted(y[i][j], t[i]))
        .collect();
    let training = (t[0] + t[1]) as f64;
    let mut sum = LogSum::EMPTY;
    for a in 0..=t[0] {
        let b = t[0] - a;
        for c in 0..=t[1] {
            let d = t[1] - c;
            sum.add(
                f[a + c] + f[b + d] - 2.0 * (f[a] + f[b] + f[c] + f[d])
                    + with[0][a]
                    + with[1][b]
                    + with[2][c]
                    + with[3][d],
            );
        }
    }
    let mut ln_null = -ln_gamma(total + 2.0) + ln_gamma(training + 2.0);
    ln_null += column_sums(y)
        .iter()
        .map(|&m| ln_gamma(m + 1.0))
        .sum::<f64>();
    for (r, &n) in rows.iter().enumerate() {
        let tr = t[r] as f64;
        ln_null += -ln_gamma(tr + 2.0) - f[t[r]] + ln_gamma(tr + n + 2.0);
    }
    ln_null - sum.ln()
}

/// ln BF01 of the nonparametric model: two rows with fixed totals, Dirichlet(`lambda`).
fn ln_nonparametric(y: &Counts, lambda: &[f64]) -> f64 {
    let base = ln_b(lambda);
    let together = ln_b(&plus(lambda, &plus(&y[0], &y[1]))) - base;
    let apart = ln_b(&plus(lambda, &y[0])) + ln_b(&plus(lambda, &y[1])) - 2.0 * base;
    together - apart
}

/// Summaries of the simulated interaction terms: mean, median, variance and the bounds of
/// the simultaneous region.
struct Term {
    mean: f64,
    median: f64,
    variance: f64,
    lower: f64,
    upper: f64,
}

/// Draws of the interaction terms ln(p_jk p_RS / (p_jS p_Rk)), j < R and k < S, from the
/// Dirichlet(counts + 1) posterior; one vector of draws per term, j outermost.
fn interaction_draws(y: &Counts, draws: usize, seed: u64) -> Vec<Vec<f64>> {
    let (r, s) = (y.len(), y[0].len());
    let mut random = Random::seeded(seed);
    let mut terms = vec![Vec::with_capacity(draws); (r - 1) * (s - 1)];
    let mut g = vec![vec![0.0; s]; r];
    for _ in 0..draws {
        // the normalizing sum of the gamma draws cancels in the odds ratios
        for (row, counts) in g.iter_mut().zip(y) {
            for (cell, &n) in row.iter_mut().zip(counts) {
                *cell = random.gamma(n + 1.0).ln();
            }
        }
        for j in 0..r - 1 {
            for k in 0..s - 1 {
                terms[j * (s - 1) + k].push(g[j][k] - g[j][s - 1] - g[r - 1][k] + g[r - 1][s - 1]);
            }
        }
    }
    terms
}

/// Besag's simultaneous region for the draws of every term: the order statistics N + 1 - k
/// and k, with k the smallest rank bound under which `level` of the draws stay in every term.
fn summaries(terms: Vec<Vec<f64>>, level: f64) -> Vec<Term> {
    let n = terms[0].len();
    let mut extreme = vec![0usize; n];
    let mut sorted = Vec::with_capacity(terms.len());
    for draws in terms {
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| draws[a].total_cmp(&draws[b]));
        for (rank, &i) in order.iter().enumerate() {
            extreme[i] = extreme[i].max((rank + 1).max(n - rank));
        }
        let mut values = draws;
        values.sort_by(f64::total_cmp);
        sorted.push(values);
    }
    extreme.sort_unstable();
    let kept = ((level * n as f64).ceil() as usize).clamp(1, n);
    let k = extreme[kept - 1];
    sorted
        .into_iter()
        .map(|values| {
            let mean = values.iter().sum::<f64>() / n as f64;
            let ss: f64 = values.iter().map(|v| (v - mean).powi(2)).sum();
            let median = if n % 2 == 1 {
                values[n / 2]
            } else {
                (values[n / 2 - 1] + values[n / 2]) / 2.0
            };
            Term {
                mean,
                median,
                variance: ss / (n as f64 - 1.0),
                lower: values[n - k],
                upper: values[k - 1],
            }
        })
        .collect()
}

/// SHAPEPARAMS as an R x S matrix in the order of the table.
fn shapes(given: &[Vec<f64>], r: usize, s: usize) -> Result<Counts, Error> {
    match given {
        [] => Ok(vec![vec![1.0; s]; r]),
        [one] if one.len() == 1 => Ok(vec![vec![one[0]; s]; r]),
        rows if rows.len() == r && rows.iter().all(|row| row.len() == s) => Ok(rows.to_vec()),
        _ => Err(Error(format!(
            "SHAPEPARAMS needs one value, or {r} rows of {s} values each ending with ';' to match the table"
        ))),
    }
}

const MARGINS: [&str; 3] = ["grand total", "row totals", "column totals"];

impl Engine {
    pub(super) fn bayes_loglinear(&self, c: &BayesLoglinear) -> Result<models::Output, Error> {
        let data = self.active()?;
        let mut run = Run::new(self, data, &c.criteria)?;
        let variables = self.variables(&[c.row.clone(), c.column.clone()])?;
        let (row_var, column_var) = (&variables[0], &variables[1]);
        let cases = run.cases(&[row_var, column_var]);
        if cases.is_empty() {
            return Err(Error(format!(
                "BAYES LOGLINEAR has no cases with valid values of both {} and {}",
                row_var.name(),
                column_var.name()
            )));
        }
        let ordered = |mut keys: Vec<Key>| {
            if c.criteria.descending {
                keys.reverse();
            }
            keys
        };
        let rows = ordered(models::levels(row_var, &cases));
        let columns = ordered(models::levels(column_var, &cases));
        let (row_keys, column_keys) = (categories(row_var, false), categories(column_var, false));
        let position = |keys: &[Key], key: &Option<Key>| {
            let key = key.as_ref().expect("a valid case has a category");
            keys.iter()
                .position(|k| k.compare(key).is_eq())
                .expect("listed above")
        };
        let mut counts = vec![vec![0.0; columns.len()]; rows.len()];
        for &case in &cases {
            let i = position(&rows, &row_keys[case]);
            let j = position(&columns, &column_keys[case]);
            counts[i][j] += run.weight(case);
        }
        let (r, s) = (rows.len(), columns.len());
        let block = Block::two_way(rows, columns, counts.clone());
        let title = format!(
            "{} by {}",
            run.shown.variable(row_var),
            run.shown.variable(column_var)
        );
        let mut out = Vec::new();
        if !c.suppress_table {
            out.push(crosstab(
                &run,
                &block,
                (row_var, column_var),
                &c.cells,
                &title,
            ));
        }
        if c.chisq || c.lrt {
            out.push(tests(&block, c.chisq, c.lrt, &title));
        }
        if r < 2 || s < 2 {
            run.warnings.push(format!(
                "no Bayesian inference: {title} needs at least two rows and two columns with cases"
            ));
            return Ok((out, run.warnings, run.charts));
        }
        let rows_fixed = c.margin != FixedMargin::Total;
        let conjugate = c.model == CountModel::Poisson
            || (c.model == CountModel::Multinomial && c.prior == TablePrior::Conjugate);
        let unused = [
            (!c.shapes.is_empty() && !conjugate, "SHAPEPARAMS"),
            (
                c.scale.is_some() && c.model != CountModel::Poisson,
                "SCALEPARAM",
            ),
            (
                !c.lambdas.is_empty() && c.model != CountModel::Nonparametric,
                "LAMBDAS",
            ),
        ];
        for (_, keyword) in unused.iter().filter(|u| u.0) {
            run.warnings.push(format!(
                "{keyword} does not apply to this model and prior and is not used"
            ));
        }
        if c.analysis.factor() {
            // fixed column totals: the same formulas on the transposed table
            let (y, a) = {
                let a = shapes(&c.shapes, r, s)?;
                if c.margin == FixedMargin::Column {
                    (transpose(&counts), transpose(&a))
                } else {
                    (counts.clone(), a)
                }
            };
            let total: f64 = y.concat().iter().sum();
            let margin = MARGINS[c.margin as usize];
            let (model, prior, ln_bf01) = match c.model {
                CountModel::Poisson => {
                    let min = a.concat().into_iter().fold(f64::INFINITY, f64::min);
                    let b = c.scale.unwrap_or((r * s) as f64 * min / total);
                    let prior = format!("Conjugate, gamma scale {b:.4}");
                    (
                        "Poisson".to_string(),
                        prior,
                        ln_conjugate(&y, &a, false, Some(b)),
                    )
                }
                CountModel::Nonparametric => {
                    let free = y[0].len();
                    let lambda = match c.lambdas.as_slice() {
                        [] => vec![1.0; free],
                        [one] => vec![*one; free],
                        given if given.len() == free => given.to_vec(),
                        _ => {
                            return Err(Error(format!(
                                "LAMBDAS needs one value or {free}, one for each category whose total is not fixed"
                            )));
                        }
                    };
                    let bf = if y.len() == 2 {
                        Some(ln_nonparametric(&y, &lambda))
                    } else {
                        run.warnings.push(format!(
                            "no Bayes factor: MODEL=NONPARAMETRIC needs exactly two categories with fixed {margin}"
                        ));
                        None
                    };
                    let model = format!("Nonparametric, {margin} fixed");
                    (model, "Dirichlet".into(), bf)
                }
                CountModel::Multinomial => {
                    let model = format!("Multinomial, {margin} fixed");
                    let two_by_two = r == 2 && s == 2;
                    if c.prior == TablePrior::Intrinsic && !two_by_two {
                        run.warnings.push(
                            "the intrinsic prior needs a 2 x 2 table; the mixture of Dirichlet priors is used"
                                .into(),
                        );
                    }
                    match c.prior {
                        TablePrior::Conjugate => {
                            let bf = ln_conjugate(&y, &a, rows_fixed, None);
                            (model, "Conjugate".into(), bf)
                        }
                        TablePrior::Intrinsic if two_by_two => {
                            let bf = if rows_fixed {
                                ln_intrinsic_rows(&y)
                            } else {
                                -ln_intrinsic_total(&y)
                            };
                            (model, "Intrinsic".into(), Some(bf))
                        }
                        _ => {
                            let bf = -ln_mixture(&y, rows_fixed, run.quadrature);
                            (model, "Mixture of Dirichlet".into(), Some(bf))
                        }
                    }
                }
            };
            if conjugate && ln_bf01.is_none() {
                run.warnings.push(
                    "no Bayes factor: a row sum of SHAPEPARAMS must exceed the number of columns less 1, and a column sum the number of rows less 1"
                        .into(),
                );
            }
            let row = vec![
                Cell::Text(model),
                Cell::Text(prior),
                Cell::Number(total),
                number(ln_bf01.map(f64::exp)),
            ];
            let columns = ["Model", "Prior", "N", "Bayes Factor"];
            let mut t = table("Bayes Factor Independence Test", &columns, vec![row]);
            t.caption = Some(format!("{BF01} The null hypothesis is independence."));
            out.push(t);
        }
        if c.analysis.posterior() {
            let seed = c.criteria.seed.unwrap_or_else(time_seed);
            let draws = c.criteria.postsamples;
            let terms = summaries(interaction_draws(&counts, draws, seed), c.criteria.ci);
            let mut lines = Vec::new();
            for j in 0..r - 1 {
                for k in 0..s - 1 {
                    let p = &terms[j * (s - 1) + k];
                    let mut line = vec![
                        Cell::Text(run.shown.value(row_var, &block.rows[j])),
                        Cell::Text(run.shown.value(column_var, &block.columns[k])),
                    ];
                    line.extend(
                        [p.mean, p.median, p.variance, p.lower, p.upper].map(|v| number(Some(v))),
                    );
                    lines.push(line);
                }
            }
            let columns = [
                run.shown.variable(row_var),
                run.shown.variable(column_var),
                "Mean".into(),
                "Median".into(),
                "Variance".into(),
                "Lower".into(),
                "Upper".into(),
            ];
            let interval = run.ci.replace("Credible", "Simultaneous Credible");
            out.push(Table {
                title: "Posterior Distribution of the Interaction Terms".into(),
                row_headers: 2,
                column_groups: (0..columns.len())
                    .map(|i| if i >= 5 { vec![interval.clone()] } else { Vec::new() })
                    .collect(),
                columns: columns.to_vec(),
                rows: lines,
                caption: Some(format!(
                    "Each term is the log odds ratio of its cell against the last row and the last column. {draws} draws from the Dirichlet posterior of the cell probabilities under a uniform prior, seed {seed}. The intervals hold together; one without 0 points to association."
                )),
                ..Table::default()
            });
        }
        Ok((out, run.warnings, run.charts))
    }
}

/// The crosstab of PRINT CELLS, laid out as in CROSSTABS.
fn crosstab(
    run: &Run,
    block: &Block,
    (row_var, column_var): (&Variable, &Variable),
    cells: &[CrosstabsCell],
    title: &str,
) -> Table {
    let mut columns = vec![run.shown.variable(row_var), "Statistic".into()];
    columns.extend(block.columns.iter().map(|k| run.shown.value(column_var, k)));
    columns.push("Total".into());
    let mut column_groups = vec![Vec::new(); columns.len()];
    for group in &mut column_groups[2..columns.len() - 1] {
        group.push(run.shown.variable(column_var));
    }
    let mut rows = Vec::new();
    for i in (0..block.rows.len()).map(Some).chain([None]) {
        for &statistic in cells {
            let mut row = vec![
                Cell::Text(i.map_or("Total".into(), |i| run.shown.value(row_var, &block.rows[i]))),
                Cell::Text(cell_name(statistic).into()),
            ];
            row.extend((0..block.columns.len()).map(|j| block.cell(statistic, i, Some(j))));
            row.push(block.cell(statistic, i, None));
            rows.push(row);
        }
    }
    Table {
        title: title.into(),
        row_headers: 2,
        columns,
        column_groups,
        rows,
        ..Table::default()
    }
}

/// The tests of PRINT STATISTICS from the chi-square tests of CROSSTABS.
fn tests(block: &Block, chisq: bool, lrt: bool, title: &str) -> Table {
    let (r, s) = (block.rows.len(), block.columns.len());
    let small = (0..r).any(|i| (0..s).any(|j| block.expected(i, j) < 5.0));
    let kept: Vec<_> = block
        .tests(false)
        .into_iter()
        .filter(|(name, _)| match *name {
            "Pearson Chi-Square" | "Continuity Correction" => chisq,
            "Fisher's Exact Test" => chisq && small,
            "Likelihood Ratio" => lrt,
            _ => true,
        })
        .collect();
    let exact = kept.iter().any(|(name, _)| *name == "Fisher's Exact Test");
    let mut columns: Vec<String> = ["Statistic", "Value", "df", "Asymptotic Sig. (2-tailed)"]
        .map(String::from)
        .to_vec();
    if exact {
        columns.extend(["Exact Sig. (2-tailed)", "Exact Sig. (1-tailed)"].map(String::from));
    }
    let rows = kept
        .into_iter()
        .map(|(name, values)| {
            let mut row = vec![Cell::Text(name.into())];
            row.extend(values[..columns.len() - 1].iter().map(|v| number(*v)));
            row
        })
        .collect();
    Table {
        title: format!("Chi-Square Tests: {title}"),
        row_headers: 1,
        column_groups: vec![Vec::new(); columns.len()],
        columns,
        rows,
        ..Table::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rising_factorials_agree_across_the_switch() {
        for (x, m) in [(99_999.0, 37.0), (100_000.0, 37.0), (2e5, 1e4)] {
            let direct = ln_gamma(x + m) - ln_gamma(x);
            assert!((ln_rising(x, m) - direct).abs() < 1e-6 * direct.abs().max(1.0));
        }
        assert_eq!(ln_rising(1e300, 0.0), 0.0);
        // far out the factor is m ln x
        assert!((ln_rising(1e200, 5.0) - 5.0 * 1e200f64.ln()).abs() < 1e-9);
    }

    #[test]
    fn the_simultaneous_region_holds_the_level_of_the_draws() {
        let terms = interaction_draws(&vec![vec![12.0, 5.0, 3.0], vec![4.0, 9.0, 8.0]], 4000, 77);
        let copy = terms.clone();
        let summary = summaries(terms, 0.9);
        let inside = (0..4000)
            .filter(|&t| {
                summary
                    .iter()
                    .zip(&copy)
                    .all(|(p, d)| d[t] >= p.lower && d[t] <= p.upper)
            })
            .count();
        assert!((3600..3700).contains(&inside), "{inside}");
        // every marginal interval is wider than its own 90% interval
        for (p, d) in summary.iter().zip(&copy) {
            let below = d.iter().filter(|&&v| v < p.lower).count();
            assert!(below < 200, "{below}");
        }
    }

    #[test]
    fn log_sums_match_the_direct_sum() {
        let mut s = LogSum::EMPTY;
        for x in [-1.0, 3.0, 0.5, 2.0] {
            s.add(x);
        }
        let direct = [-1.0f64, 3.0, 0.5, 2.0]
            .iter()
            .map(|x| x.exp())
            .sum::<f64>()
            .ln();
        assert!((s.ln() - direct).abs() < 1e-12);
    }
}
