//! CROSSTABS: contingency tables, their cell statistics and chi-square tests, and the
//! clustered bar chart of BARCHART.

use crate::chart::{
    self, Axis, Category, Chart, ChartKind, Grouping, Orientation, Series, Statistic, Tier,
};
use crate::descriptives::case_weights;
use crate::dist::chi_square_p;
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Values, Variable};
use statscraft_syntax::{Crosstabs, CrosstabsCell as C, WeightCount};
use std::cmp::Ordering;

/// A category: the value of a case.
#[derive(Clone, PartialEq)]
pub(crate) enum Key {
    Number(f64),
    Text(String),
}

impl Key {
    pub(crate) fn compare(&self, other: &Key) -> Ordering {
        match (self, other) {
            (Key::Number(a), Key::Number(b)) => a.total_cmp(b),
            (Key::Text(a), Key::Text(b)) => a.cmp(b),
            // the values of one variable all have its type
            _ => Ordering::Equal,
        }
    }
}

/// The category of every case, None where the value is missing.
pub(crate) fn categories(variable: &Variable, include: bool) -> Vec<Option<Key>> {
    match variable.values() {
        Values::Numeric(values) => values
            .iter()
            .map(|v| {
                v.filter(|v| include || !variable.is_user_missing(*v))
                    .map(Key::Number)
            })
            .collect(),
        Values::String(values) => values
            .iter()
            .map(|v| (include || !variable.is_user_missing_str(v)).then(|| Key::Text(v.into())))
            .collect(),
    }
}

/// One layer of a table: the categories that occur in it and the weighted counts.
struct Block {
    layer: Vec<Key>,
    rows: Vec<Key>,
    columns: Vec<Key>,
    counts: Vec<Vec<f64>>,
    row_totals: Vec<f64>,
    column_totals: Vec<f64>,
    total: f64,
    /// `/COUNT TRUNCATE`: whole counts are truncated, not rounded.
    truncate: bool,
}

/// A weight or count as a whole number, as `/COUNT` asks.
fn whole(value: f64, truncate: bool) -> f64 {
    if truncate {
        value.trunc()
    } else {
        value.round()
    }
}

impl Block {
    /// The count of a cell; None stands for the Total row or column.
    fn count(&self, row: Option<usize>, column: Option<usize>) -> f64 {
        match (row, column) {
            (Some(i), Some(j)) => self.counts[i][j],
            (Some(i), None) => self.row_totals[i],
            (None, Some(j)) => self.column_totals[j],
            (None, None) => self.total,
        }
    }

    fn expected(&self, i: usize, j: usize) -> f64 {
        self.row_totals[i] * self.column_totals[j] / self.total
    }

    fn cell(&self, statistic: C, row: Option<usize>, column: Option<usize>) -> Cell {
        let n = self.count(row, column);
        let row_total = row.map_or(self.total, |i| self.row_totals[i]);
        let column_total = column.map_or(self.total, |j| self.column_totals[j]);
        let value = match (statistic, row.zip(column)) {
            (C::Count, _) => n,
            (C::Row, _) => 100.0 * n / row_total,
            (C::Column, _) => 100.0 * n / column_total,
            (C::Total, _) => 100.0 * n / self.total,
            (C::Expected, Some((i, j))) => self.expected(i, j),
            // the expected counts of a row or column add up to its observed count
            (C::Expected, None) => n,
            (_, None) => return Cell::Empty,
            (C::Resid, Some((i, j))) => n - self.expected(i, j),
            (C::Sresid, Some((i, j))) => (n - self.expected(i, j)) / self.expected(i, j).sqrt(),
            (C::Asresid, Some((i, j))) => {
                let share = (1.0 - row_total / self.total) * (1.0 - column_total / self.total);
                (n - self.expected(i, j)) / (self.expected(i, j) * share).sqrt()
            }
        };
        if value.is_finite() {
            Cell::Number(value)
        } else {
            Cell::Empty
        }
    }

    /// Rows of the chi-square table: name, value, df, asymptotic, exact two- and one-sided
    /// significance. Nothing but the case count when the table has one row or column.
    fn tests(&self, numeric: bool) -> Vec<(&'static str, [Option<f64>; 5])> {
        let (r, c) = (self.rows.len(), self.columns.len());
        let n_row = (
            "N of Valid Cases",
            [Some(self.total), None, None, None, None],
        );
        if r < 2 || c < 2 {
            return vec![n_row];
        }
        let cells = || (0..r).flat_map(move |i| (0..c).map(move |j| (i, j)));
        let df = ((r - 1) * (c - 1)) as f64;
        let asymptotic = |name, value: f64, df: f64| {
            (
                name,
                [
                    Some(value),
                    Some(df),
                    Some(chi_square_p(value, df)),
                    None,
                    None,
                ],
            )
        };
        let pearson: f64 = cells()
            .map(|(i, j)| (self.counts[i][j] - self.expected(i, j)).powi(2) / self.expected(i, j))
            .sum();
        let ratio: f64 = 2.0
            * cells()
                .filter(|&(i, j)| self.counts[i][j] > 0.0)
                .map(|(i, j)| self.counts[i][j] * (self.counts[i][j] / self.expected(i, j)).ln())
                .sum::<f64>();
        let mut rows = vec![
            asymptotic("Pearson Chi-Square", pearson, df),
            asymptotic("Likelihood Ratio", ratio, df),
        ];
        if (r, c) == (2, 2) {
            let rounded = |i: usize, j: usize| whole(self.counts[i][j], self.truncate) as u64;
            let (two, one) = fisher(rounded(0, 0), rounded(0, 1), rounded(1, 0), rounded(1, 1));
            rows.push((
                "Fisher's Exact Test",
                [None, None, None, Some(two), Some(one)],
            ));
            let yates: f64 = cells()
                .map(|(i, j)| {
                    let gap = ((self.counts[i][j] - self.expected(i, j)).abs() - 0.5).max(0.0);
                    gap * gap / self.expected(i, j)
                })
                .sum();
            rows.push(asymptotic("Continuity Correction", yates, 1.0));
        }
        if numeric {
            let score = |key: &Key| match key {
                Key::Number(n) => *n,
                Key::Text(_) => 0.0,
            };
            let mean = |keys: &[Key], totals: &[f64]| {
                keys.iter()
                    .zip(totals)
                    .map(|(k, n)| score(k) * n)
                    .sum::<f64>()
                    / self.total
            };
            let (mx, my) = (
                mean(&self.rows, &self.row_totals),
                mean(&self.columns, &self.column_totals),
            );
            let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
            for (i, j) in cells() {
                let (dx, dy) = (score(&self.rows[i]) - mx, score(&self.columns[j]) - my);
                let n = self.counts[i][j];
                sxx += n * dx * dx;
                syy += n * dy * dy;
                sxy += n * dx * dy;
            }
            let r = sxy / (sxx * syy).sqrt();
            rows.push(asymptotic(
                "Linear-by-Linear Association",
                (self.total - 1.0) * r * r,
                1.0,
            ));
        }
        rows.push(n_row);
        rows
    }
}

fn ordered(a: &Key, b: &Key, descending: bool) -> Ordering {
    let ordering = a.compare(b);
    if descending {
        ordering.reverse()
    } else {
        ordering
    }
}

/// The blocks of one table: every combination of layer categories that occurs, the last layer
/// variable outermost. `descending` orders the rows from the highest value; columns and layers
/// stay ascending. `/COUNT CELL` makes every cell a whole count before the totals.
fn blocks(
    keys: &[Vec<Option<Key>>],
    weights: Option<&[f64]>,
    descending: bool,
    (count, truncate): (WeightCount, bool),
) -> Vec<Block> {
    let order = |a: &Key, b: &Key| a.compare(b);
    let case_count = keys[0].len();
    let weight = |case: usize| weights.map_or(1.0, |w| w[case]);
    let cases: Vec<(Vec<&Key>, f64)> = (0..case_count)
        .filter(|&case| weight(case) > 0.0)
        .filter_map(|case| {
            let values: Option<Vec<&Key>> = keys.iter().map(|k| k[case].as_ref()).collect();
            Some((values?, weight(case)))
        })
        .collect();
    let distinct = |mut found: Vec<&Key>, descending: bool| {
        found.sort_by(|a, b| ordered(a, b, descending));
        found.dedup();
        found.into_iter().cloned().collect::<Vec<Key>>()
    };
    let mut layers: Vec<Vec<&Key>> = cases.iter().map(|(v, _)| v[2..].to_vec()).collect();
    let outermost_first = |a: &Vec<&Key>, b: &Vec<&Key>| {
        let pairs = a.iter().rev().zip(b.iter().rev());
        pairs
            .map(|(x, y)| order(x, y))
            .find(|o| o.is_ne())
            .unwrap_or(Ordering::Equal)
    };
    layers.sort_by(outermost_first);
    layers.dedup();

    layers
        .into_iter()
        .map(|layer| {
            let inside: Vec<&(Vec<&Key>, f64)> =
                cases.iter().filter(|(v, _)| v[2..] == layer[..]).collect();
            let rows = distinct(inside.iter().map(|(v, _)| v[0]).collect(), descending);
            let columns = distinct(inside.iter().map(|(v, _)| v[1]).collect(), false);
            let mut counts = vec![vec![0.0; columns.len()]; rows.len()];
            for (values, w) in inside {
                let i = rows
                    .iter()
                    .position(|k| k == values[0])
                    .expect("listed above");
                let j = columns
                    .iter()
                    .position(|k| k == values[1])
                    .expect("listed above");
                counts[i][j] += w;
            }
            if count == WeightCount::Cell {
                counts
                    .iter_mut()
                    .flatten()
                    .for_each(|n| *n = whole(*n, truncate));
            }
            let row_totals: Vec<f64> = counts.iter().map(|row| row.iter().sum()).collect();
            let column_totals: Vec<f64> = (0..columns.len())
                .map(|j| counts.iter().map(|row| row[j]).sum())
                .collect();
            Block {
                layer: layer.into_iter().cloned().collect(),
                total: row_totals.iter().sum(),
                rows,
                columns,
                counts,
                row_totals,
                column_totals,
                truncate,
            }
        })
        .collect()
}

fn cell_name(statistic: C) -> &'static str {
    match statistic {
        C::Count => "Count",
        C::Row => "Row %",
        C::Column => "Column %",
        C::Total => "Total %",
        C::Expected => "Expected",
        C::Resid => "Residual",
        C::Sresid => "Std. Residual",
        C::Asresid => "Adjusted Residual",
    }
}

/// The clustered bar chart of a table: a cluster per row category, a bar per column category,
/// the counts summed over the layers. Only the categories of valid values take part, as in the
/// table.
///
/// The two manuals word the roles differently; the dialog manual's reading, clusters by the
/// row variable and bars by the column variable, is the one followed here.
fn bar_chart(
    blocks: &[Block],
    variables: &[Variable],
    shown: &Shown,
    (title, descending): (String, bool),
) -> Chart {
    let union = |of: &dyn Fn(&Block) -> &Vec<Key>, descending: bool| {
        let mut keys: Vec<&Key> = blocks.iter().flat_map(of).collect();
        keys.sort_by(|a, b| ordered(a, b, descending));
        keys.dedup();
        keys.into_iter().cloned().collect::<Vec<Key>>()
    };
    let rows = union(&|b| &b.rows, descending);
    let columns = union(&|b| &b.columns, false);
    let mut counts = vec![vec![0.0; rows.len()]; columns.len()];
    for block in blocks {
        for (i, row) in block.rows.iter().enumerate() {
            let r = rows.iter().position(|k| k == row).expect("in the union");
            for (j, column) in block.columns.iter().enumerate() {
                let c = columns
                    .iter()
                    .position(|k| k == column)
                    .expect("in the union");
                counts[c][r] += block.counts[i][j];
            }
        }
    }
    let value = |key: &Key| match key {
        Key::Number(n) => Cell::Number(*n),
        Key::Text(t) => Cell::Text(t.clone()),
    };
    let numeric_format = |v: &Variable| match v.is_string() {
        true => String::new(),
        false => v.format().to_string(),
    };
    Chart {
        kind: ChartKind::Bar,
        tier: Tier::Presentation,
        orientation: Some(Orientation::Vertical),
        grouping: Some(Grouping::Clustered),
        title,
        footnote: None,
        category_axis: Axis {
            title: shown.variable(&variables[0]),
            format: numeric_format(&variables[0]),
            maximum: None,
            variable: Some(variables[0].name().to_string()),
        },
        value_axis: Axis {
            title: "Count".into(),
            format: chart::count_format(counts.iter().flatten().copied()),
            maximum: None,
            variable: None,
        },
        legend: Some(shown.variable(&variables[1])),
        categories: rows
            .iter()
            .map(|key| Category {
                label: shown.value(&variables[0], key),
                value: value(key),
                missing: false,
                bin: None,
            })
            .collect(),
        series: columns
            .iter()
            .zip(counts)
            .map(|(key, values)| Series {
                name: shown.value(&variables[1], key),
                statistic: Statistic::Count,
                variable: Some(variables[1].name().to_string()),
                value: value(key),
                values,
            })
            .collect(),
        normal: None,
        plot: None,
    }
}

/// The tables of one combination of variables: rows, columns, then layers; and its chart.
fn tables(
    data: &Dataset,
    names: &[String],
    command: &Crosstabs,
    (shown, outline): (&Shown, &Shown),
) -> Result<(Vec<Table>, Option<Chart>), Error> {
    let variables: Vec<Variable> = names
        .iter()
        .map(|name| data.variable(name))
        .collect::<Result<_, _>>()?;
    let keys: Vec<Vec<Option<Key>>> = variables
        .iter()
        .map(|v| categories(v, command.include))
        .collect();
    let mut weights = case_weights(data)?;
    if command.count == WeightCount::Case {
        // every case's weight as a whole number; one that becomes 0 is left out
        for w in weights.iter_mut().flatten() {
            *w = whole(*w, command.truncate);
        }
    }
    let count = (command.count, command.truncate);
    let blocks = blocks(&keys, weights.as_deref(), command.descending, count);
    let layer_variables = &variables[2..];
    let layer_cells = |block: &Block| -> Vec<Cell> {
        let pairs = layer_variables.iter().zip(&block.layer);
        pairs.map(|(v, k)| Cell::Text(shown.value(v, k))).collect()
    };
    let layer_names = || layer_variables.iter().map(|v| shown.variable(v));
    let title: Vec<String> = variables.iter().map(|v| outline.variable(v)).collect();
    let title = title.join(" by ");
    let mut out = Vec::new();

    if !command.cells.is_empty() && !blocks.is_empty() {
        // the columns of every layer; a layer without one of them leaves its cells blank
        let mut union: Vec<&Key> = blocks.iter().flat_map(|b| &b.columns).collect();
        union.sort_by(|a, b| a.compare(b));
        union.dedup();
        let mut columns: Vec<String> = layer_names().collect();
        columns.extend([shown.variable(&variables[0]), "Statistic".into()]);
        let headers = columns.len();
        columns.extend(union.iter().map(|k| shown.value(&variables[1], k)));
        columns.push("Total".into());
        let mut column_groups = vec![Vec::new(); columns.len()];
        for group in &mut column_groups[headers..columns.len() - 1] {
            group.push(shown.variable(&variables[1]));
        }
        let mut rows = Vec::new();
        for block in &blocks {
            let own: Vec<Option<usize>> = union
                .iter()
                .map(|k| block.columns.iter().position(|c| c == *k))
                .collect();
            let row_indices = (0..block.rows.len()).map(Some).chain([None]);
            for i in row_indices {
                for &statistic in &command.cells {
                    // residuals have no Total row; the reference program leaves out the line
                    if i.is_none() && matches!(statistic, C::Resid | C::Sresid | C::Asresid) {
                        continue;
                    }
                    let mut row = layer_cells(block);
                    row.push(Cell::Text(match i {
                        Some(i) => shown.value(&variables[0], &block.rows[i]),
                        None => "Total".into(),
                    }));
                    row.push(Cell::Text(cell_name(statistic).into()));
                    row.extend(own.iter().map(|j| match j {
                        Some(j) => block.cell(statistic, i, Some(*j)),
                        None => Cell::Empty,
                    }));
                    row.push(block.cell(statistic, i, None));
                    rows.push(row);
                }
            }
        }
        out.push(Table {
            title: title.clone(),
            row_headers: headers,
            columns,
            column_groups,
            rows,
            ..Table::default()
        });
    }

    if command.chisq {
        let numeric = !variables[0].is_string() && !variables[1].is_string();
        let tests: Vec<_> = blocks.iter().map(|b| (b, b.tests(numeric))).collect();
        let exact = blocks
            .iter()
            .any(|b| (b.rows.len(), b.columns.len()) == (2, 2));
        let mut columns: Vec<String> = layer_names().collect();
        let headers = columns.len() + 1;
        columns
            .extend(["Statistic", "Value", "df", "Asymptotic Sig. (2-tailed)"].map(String::from));
        if exact {
            columns.extend(["Exact Sig. (2-tailed)", "Exact Sig. (1-tailed)"].map(String::from));
        }
        let mut rows = Vec::new();
        for (block, block_tests) in tests {
            for (name, values) in block_tests {
                let mut row = layer_cells(block);
                row.push(Cell::Text(name.into()));
                let shown = if exact { 5 } else { 3 };
                row.extend(values[..shown].iter().map(|v| match v {
                    Some(v) if v.is_finite() => Cell::Number(*v),
                    _ => Cell::Empty,
                }));
                rows.push(row);
            }
        }
        out.push(Table {
            title: format!("Chi-Square Tests: {title}"),
            row_headers: headers,
            column_groups: vec![Vec::new(); columns.len()],
            columns,
            rows,
            ..Table::default()
        });
    }
    let chart = (command.barchart && !blocks.is_empty())
        .then(|| bar_chart(&blocks, &variables, shown, (title, command.descending)));
    Ok((out, chart))
}

impl Engine {
    pub(crate) fn crosstabs(&mut self, command: &Crosstabs) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let (shown, outline) = (self.shown(), self.outline());
        let mut out = Vec::new();
        let mut charts = Vec::new();
        for dimensions in &command.tables {
            let lists: Vec<Vec<String>> = dimensions
                .iter()
                .map(|items| self.expand(items))
                .collect::<Result<_, _>>()?;
            // every combination, the first dimension outermost
            let mut combinations: Vec<Vec<String>> = vec![Vec::new()];
            for list in &lists {
                combinations = combinations
                    .iter()
                    .flat_map(|prefix| {
                        list.iter().map(move |name| {
                            let mut next = prefix.clone();
                            next.push(name.clone());
                            next
                        })
                    })
                    .collect();
            }
            for names in combinations {
                let (tables, chart) = tables(data, &names, command, (&shown, &outline))?;
                out.extend(tables);
                charts.extend(chart);
            }
        }
        self.charts.extend(charts);
        Ok(out)
    }
}

/// Fisher's exact test of the 2 x 2 table [[a, b], [c, d]]: the two-sided significance (the
/// tables no more likely than the observed one) and the one-sided one (the smaller tail).
pub(crate) fn fisher(a: u64, b: u64, c: u64, d: u64) -> (f64, f64) {
    let n = (a + b + c + d) as usize;
    let mut ln_factorial = vec![0.0; n + 1];
    for k in 1..=n {
        ln_factorial[k] = ln_factorial[k - 1] + (k as f64).ln();
    }
    let f = |k: u64| ln_factorial[k as usize];
    let (row1, row2, column1, column2) = (a + b, c + d, a + c, b + d);
    let fixed = f(row1) + f(row2) + f(column1) + f(column2) - f(n as u64);
    let p = |x: u64| (fixed - f(x) - f(row1 - x) - f(column1 - x) - f(row2 + x - column1)).exp();
    let low = column1.saturating_sub(row2);
    let high = row1.min(column1);
    let observed = p(a);
    let (mut two, mut left, mut right) = (0.0, 0.0, 0.0);
    for x in low..=high {
        let px = p(x);
        // relative slack: tables as likely as the observed one may differ in the last bits
        if px <= observed * (1.0 + 1e-7) {
            two += px;
        }
        if x <= a {
            left += px;
        }
        if x >= a {
            right += px;
        }
    }
    (two.min(1.0), left.min(right).min(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dist::{gamma_q, ln_gamma};

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance * b.abs().max(1e-300)
    }

    #[test]
    fn ln_gamma_known_values() {
        assert!(ln_gamma(1.0).abs() < 1e-14 && ln_gamma(2.0).abs() < 1e-14);
        assert!(close(
            ln_gamma(0.5),
            std::f64::consts::PI.sqrt().ln(),
            1e-14
        ));
        assert!(close(ln_gamma(10.0), 362_880.0_f64.ln(), 1e-14));
        assert!(close(ln_gamma(100.5), 361.435_540_467_777_6, 1e-13));
        assert!(close(ln_gamma(0.1), 2.252_712_651_734_206, 1e-13));
    }

    #[test]
    fn chi_square_significance_known_values() {
        // upper tail probabilities of the chi-square distribution, computed with 30 digits elsewhere
        let cases = [
            (3.841_458_820_694_124, 1.0, 0.05),
            (6.634_896_601_021_214, 1.0, 0.01),
            (1.0, 1.0, 0.317_310_507_862_914),
            (2.0, 2.0, (-1.0_f64).exp()),
            (10.0, 2.0, (-5.0_f64).exp()),
            (11.070_497_693_516_35, 5.0, 0.05),
            (20.0, 20.0, 0.457_929_714_471_852_6),
            (0.5, 10.0, 0.999_993_388_289_439),
            (150.0, 100.0, 0.000_903_932_042_354_009_1),
            (31.410_432_844_230_92, 20.0, 0.05),
        ];
        for (value, df, expected) in cases {
            let p = chi_square_p(value, df);
            assert!(close(p, expected, 1e-11), "{value} {df}: {p} vs {expected}");
        }
        assert_eq!(chi_square_p(0.0, 3.0), 1.0);
        assert!(chi_square_p(1e4, 1.0) < 1e-300);
    }

    #[test]
    fn gamma_q_both_branches_agree_with_closed_forms() {
        // Q(1, x) = exp(-x) and Q(0.5, x) = erfc(sqrt(x))
        for x in [0.1, 0.9, 1.9, 2.1, 7.0, 40.0] {
            assert!(close(gamma_q(1.0, x), (-x).exp(), 1e-13), "{x}");
        }
        assert!(close(gamma_q(0.5, 1.0), 0.157_299_207_050_285_13, 1e-12));
        assert!(close(gamma_q(0.5, 0.25), 0.479_500_122_186_953_5, 1e-12));
        assert!(gamma_q(2.0, f64::NAN).is_nan());
    }

    #[test]
    fn fisher_exact_known_values() {
        // tea tasting: [[3, 1], [1, 3]]
        let (two, one) = fisher(3, 1, 1, 3);
        assert!(close(two, 34.0 / 70.0, 1e-12) && close(one, 17.0 / 70.0, 1e-12));
        let (two, one) = fisher(88, 54, 32, 26);
        assert!(close(two, 0.427_256_673_786_149, 1e-9), "{two}");
        assert!(close(one, 0.231_650_963_988_766_8, 1e-9), "{one}");
        let (two, one) = fisher(2, 1, 2, 1);
        assert!(close(two, 1.0, 1e-12) && close(one, 0.8, 1e-12));
        assert_eq!(fisher(0, 0, 0, 5), (1.0, 1.0));
    }
}
