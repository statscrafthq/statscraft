//! CROSSTABS: contingency tables, their cell statistics and chi-square tests, and the
//! clustered bar chart of BARCHART.

use crate::chart::{
    self, Axis, Category, Chart, ChartKind, Grouping, Orientation, Series, Statistic, Tier,
};
use crate::crosstabs_exact::{Found, Input, Outcome, Stat, significances};
use crate::crosstabs_measures::{Counts, cmh, mcnemar as mcnemar_test, risk};
use crate::descriptives::{case_weights, number};
use crate::dist::chi_square_p;
use crate::random::Random;
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Values, Variable};
use statscraft_syntax::{
    Crosstabs, CrosstabsCell as C, CrosstabsMeasure, CrosstabsMethod, VarItem, WeightCount,
};
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

/// Integer mode: the category of every case, its value cut to a whole number, None where it is
/// missing or outside `low` to `high`.
fn whole_categories(
    variable: &Variable,
    include: bool,
    (low, high): (f64, f64),
) -> Vec<Option<Key>> {
    let Values::Numeric(values) = variable.values() else {
        return Vec::new();
    };
    values
        .iter()
        .map(|v| {
            let v = v
                .filter(|v| include || !variable.is_user_missing(*v))?
                .trunc()
                + 0.0; // -0.4 cuts to -0, which would sort and show apart from 0
            (low..=high).contains(&v).then_some(Key::Number(v))
        })
        .collect()
}

/// Integer mode shows every whole value of a row or column variable's range, so a range is kept
/// to this many values.
const MOST_VALUES: f64 = 1000.0;

/// One layer of a table: the categories that occur in it and the weighted counts.
pub(crate) struct Block {
    layer: Vec<Key>,
    pub(crate) rows: Vec<Key>,
    pub(crate) columns: Vec<Key>,
    pub(crate) counts: Vec<Vec<f64>>,
    row_totals: Vec<f64>,
    column_totals: Vec<f64>,
    total: f64,
    /// `/COUNT TRUNCATE`: whole counts are truncated, not rounded.
    truncate: bool,
    /// `/MISSING=REPORT`: the rows and the columns of user-missing values, shown with their
    /// counts but kept out of the totals; empty when there are none.
    missing: [Vec<bool>; 2],
}

/// A weight or count as a whole number, as `/COUNT` asks.
pub(crate) fn whole(value: f64, truncate: bool) -> f64 {
    if truncate {
        value.trunc()
    } else {
        value.round()
    }
}

impl Block {
    /// A table without layers from its categories and whole counts.
    pub(crate) fn two_way(rows: Vec<Key>, columns: Vec<Key>, counts: Vec<Vec<f64>>) -> Block {
        let row_totals: Vec<f64> = counts.iter().map(|row| row.iter().sum()).collect();
        let column_totals = (0..columns.len())
            .map(|j| counts.iter().map(|row| row[j]).sum())
            .collect();
        Block {
            layer: Vec::new(),
            total: row_totals.iter().sum(),
            rows,
            columns,
            counts,
            row_totals,
            column_totals,
            truncate: false,
            missing: [Vec::new(), Vec::new()],
        }
    }

    /// The block with the rows and columns of `missing` marked, the totals left without them:
    /// a total adds up the cells of the other dimension's valid values.
    fn reported(mut self, missing: [Vec<bool>; 2]) -> Block {
        let (rows, columns) = (&missing[0], &missing[1]);
        let valid = |flags: &[bool], i: usize| !flags[i];
        self.row_totals = (self.counts.iter())
            .map(|row| {
                (0..row.len())
                    .filter(|&j| valid(columns, j))
                    .map(|j| row[j])
                    .sum()
            })
            .collect();
        self.column_totals = (0..self.columns.len())
            .map(|j| {
                (0..self.rows.len())
                    .filter(|&i| valid(rows, i))
                    .map(|i| self.counts[i][j])
                    .sum()
            })
            .collect();
        self.total = (0..self.rows.len())
            .filter(|&i| valid(rows, i))
            .map(|i| self.row_totals[i])
            .sum();
        self.missing = missing;
        self
    }

    /// Whether a row (dimension 0) or column (1) is of a reported user-missing value.
    fn is_missing(&self, dimension: usize, at: Option<usize>) -> bool {
        at.is_some_and(|i| self.missing[dimension].get(i) == Some(&true))
    }

    /// The count of a cell; None stands for the Total row or column.
    fn count(&self, row: Option<usize>, column: Option<usize>) -> f64 {
        match (row, column) {
            (Some(i), Some(j)) => self.counts[i][j],
            (Some(i), None) => self.row_totals[i],
            (None, Some(j)) => self.column_totals[j],
            (None, None) => self.total,
        }
    }

    pub(crate) fn expected(&self, i: usize, j: usize) -> f64 {
        self.row_totals[i] * self.column_totals[j] / self.total
    }

    pub(crate) fn cell(&self, statistic: C, row: Option<usize>, column: Option<usize>) -> Cell {
        let n = self.count(row, column);
        // a reported missing value shows its count and nothing else
        if self.is_missing(0, row) || self.is_missing(1, column) {
            return match statistic {
                C::Count => Cell::Number(n),
                _ => Cell::Empty,
            };
        }
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
    pub(crate) fn tests(&self, numeric: bool) -> Vec<(&'static str, [Option<f64>; 5])> {
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
/// stay ascending. `/COUNT CELL` makes every cell a whole count before the totals. `fixed` gives
/// the rows and columns of integer mode, every value of their ranges.
fn blocks(
    keys: &[Vec<Option<Key>>],
    weights: Option<&[f64]>,
    (descending, fixed): (bool, Option<&[Vec<Key>; 2]>),
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
            let (rows, columns) = match fixed {
                Some([rows, columns]) => {
                    (distinct(rows.iter().collect(), descending), columns.clone())
                }
                None => (
                    distinct(inside.iter().map(|(v, _)| v[0]).collect(), descending),
                    distinct(inside.iter().map(|(v, _)| v[1]).collect(), false),
                ),
            };
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
            Block {
                layer: layer.into_iter().cloned().collect(),
                truncate,
                ..Block::two_way(rows, columns, counts)
            }
        })
        .collect()
}

pub(crate) fn cell_name(statistic: C) -> &'static str {
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
        subtitle: None,
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
        error_bars: None,
        second_axis: None,
        plot: None,
        outline: None,
        hidden: false,
    }
}

/// The tables of one crosstabulation, its chart, and its row of the Case Processing Summary.
type Crosstab = (Vec<Table>, Option<Chart>, Vec<Cell>);

/// The tables of one combination of variables (rows, columns, then layers), its chart, and its
/// row of the Case Processing Summary. `ranges` holds the range of every variable in integer
/// mode and is empty in general mode.
fn tables(
    data: &Dataset,
    names: &[String],
    command: &Crosstabs,
    ranges: &[(String, f64, f64)],
    (shown, outline): (&Shown, &Shown),
    (random, warnings): (&mut Random, &mut Vec<String>),
) -> Result<Crosstab, Error> {
    let variables: Vec<Variable> = names
        .iter()
        .map(|name| data.variable(name))
        .collect::<Result<_, _>>()?;
    let range = |v: &Variable| {
        let found = ranges.iter().find(|(name, ..)| name == v.name());
        found.map(|&(_, low, high)| (low, high))
    };
    let keys: Vec<Vec<Option<Key>>> = variables
        .iter()
        .map(|v| match range(v) {
            Some(r) => whole_categories(v, command.include, r),
            None => categories(v, command.include),
        })
        .collect();
    // integer mode: the rows and columns are every value of their ranges
    let every = |v: &Variable| -> Result<Vec<Key>, Error> {
        let (low, high) = range(v).expect("integer mode lists every variable");
        if high - low + 1.0 > MOST_VALUES {
            return Err(Error(format!(
                "the range of {} holds {} values; as a row or column it can hold at most {MOST_VALUES}",
                v.name(),
                high - low + 1.0
            )));
        }
        Ok(((low as i64)..=(high as i64))
            .map(|x| Key::Number(x as f64))
            .collect())
    };
    let fixed = match ranges.is_empty() {
        true => None,
        false => Some([every(&variables[0])?, every(&variables[1])?]),
    };
    let mut weights = case_weights(data)?;
    if command.count == WeightCount::Case {
        // every case's weight as a whole number; one that becomes 0 is left out
        for w in weights.iter_mut().flatten() {
            *w = whole(*w, command.truncate);
        }
    }
    let count = (command.count, command.truncate);
    // cases with a positive weight, valid on every variable or not
    let (mut valid, mut missing) = (0.0, 0.0);
    for case in 0..data.case_count() {
        let w = weights.as_ref().map_or(1.0, |w| w[case]);
        if w > 0.0 {
            match keys.iter().all(|k| k[case].is_some()) {
                true => valid += w,
                false => missing += w,
            }
        }
    }
    let total = valid + missing;
    let mut summary = vec![Cell::Text(
        variables
            .iter()
            .map(|v| shown.variable(v))
            .collect::<Vec<_>>()
            .join(" * "),
    )];
    for n in [valid, missing, total] {
        summary.push(Cell::Number(n));
        summary.push(number((total > 0.0).then(|| 100.0 * n / total)));
    }
    let blocks = blocks(&keys, weights.as_deref(), (command.descending, None), count);
    // integer mode shows empty values of the ranges too; the statistics use the values that occur
    // REPORT: the table keeps the user-missing values of its rows and columns, marked
    let reported = |d: usize, k: &Key| {
        command.report && matches!(k, Key::Number(x) if variables[d].is_user_missing(*x))
    };
    let owned: Vec<Block>;
    let shown_blocks = match &fixed {
        Some(fixed) => {
            let mut keys = keys.clone();
            if command.report {
                // shortcut: a fractional user-missing value whose whole part is not missing
                // shows as valid here; integer mode expects whole values
                for (d, v) in variables.iter().enumerate().take(2) {
                    keys[d] = whole_categories(v, true, range(v).expect("integer mode"));
                }
            }
            let marks = |d: usize, keys: &[Key]| keys.iter().map(|k| reported(d, k)).collect();
            owned = self::blocks(
                &keys,
                weights.as_deref(),
                (command.descending, Some(fixed)),
                count,
            )
            .into_iter()
            .map(|b| {
                let missing = [marks(0, &b.rows), marks(1, &b.columns)];
                b.reported(missing)
            })
            .collect();
            &owned
        }
        None => &blocks,
    };
    let layer_variables = &variables[2..];
    let layer_cells = |block: &Block| -> Vec<Cell> {
        let pairs = layer_variables.iter().zip(&block.layer);
        pairs.map(|(v, k)| Cell::Text(shown.value(v, k))).collect()
    };
    let layer_names = || layer_variables.iter().map(|v| shown.variable(v));
    let title: Vec<String> = variables.iter().map(|v| outline.variable(v)).collect();
    let title = title.join(" by ");
    let mut out = Vec::new();
    let sigs: Vec<Found> = match command.method {
        Some(method) => (blocks.iter())
            .map(|b| {
                block_significances(
                    b,
                    &variables,
                    command,
                    method,
                    (&mut *random, &mut *warnings),
                )
            })
            .collect(),
        None => vec![Vec::new(); blocks.len()],
    };

    if !command.cells.is_empty() && !shown_blocks.is_empty() {
        // the columns of every layer; a layer without one of them leaves its cells blank
        let mut union: Vec<&Key> = shown_blocks.iter().flat_map(|b| &b.columns).collect();
        union.sort_by(|a, b| a.compare(b));
        union.dedup();
        let mut columns: Vec<String> = layer_names().collect();
        columns.extend([shown.variable(&variables[0]), "Statistic".into()]);
        let headers = columns.len();
        let label = |d: usize, k: &Key| match reported(d, k) {
            true => format!("{} (missing)", shown.value(&variables[d], k)),
            false => shown.value(&variables[d], k),
        };
        columns.extend(union.iter().map(|k| label(1, k)));
        columns.push("Total".into());
        let mut column_groups = vec![Vec::new(); columns.len()];
        for group in &mut column_groups[headers..columns.len() - 1] {
            group.push(shown.variable(&variables[1]));
        }
        let mut rows = Vec::new();
        for block in shown_blocks {
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
                        Some(i) => label(0, &block.rows[i]),
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
        let marked = shown_blocks
            .iter()
            .any(|b| b.missing.iter().flatten().any(|m| *m));
        out.push(Table {
            title: title.clone(),
            row_headers: headers,
            columns,
            column_groups,
            rows,
            caption: marked.then(|| {
                "Values marked (missing) are user-missing: their cells show counts only and stay out of totals, percentages and statistics.".into()
            }),
            ..Table::default()
        });
    }

    let mcnemar = command.measures.contains(&CrosstabsMeasure::Mcnemar);
    if command.chisq || mcnemar {
        let numeric = !variables[0].is_string() && !variables[1].is_string();
        let mut note = None;
        let tests: Vec<_> = blocks
            .iter()
            .zip(&sigs)
            .map(|(b, sigs)| {
                let mut rows = match command.chisq {
                    true => b.tests(numeric),
                    false => vec![(
                        "N of Valid Cases",
                        [Some(b.total), None, None, None, None],
                    )],
                };
                if mcnemar {
                    // a test of symmetry needs the same values in the rows and the columns
                    let same = b.rows.len() == b.columns.len()
                        && b.rows.iter().all(|r| b.columns.iter().any(|c| c.compare(r).is_eq()));
                    match square(b, &variables).filter(|f| same && f.len() >= 2) {
                        Some(f) => {
                            let (name, [value, df, asymptotic, exact]) =
                                mcnemar_test(&f, |n| whole(n, command.truncate));
                            let at = rows.len() - 1;
                            rows.insert(at, (name, [value, df, asymptotic, exact, None]));
                        }
                        None => note = Some("The McNemar test needs a square table: the same values, at least two, in the rows and the columns."),
                    }
                }
                (b, with_significance(rows, sigs, b.rows.len() == 2 && b.columns.len() == 2))
            })
            .collect();
        // the value, df and asymptotic significance, then each group of columns some row fills
        let groups: [&[usize]; 5] = [&[0, 1, 2], &[3, 4], &[5], &[6, 7, 8], &[9, 10, 11]];
        let filled = |group: &[usize]| {
            (tests.iter()).any(|(_, rows)| rows.iter().any(|(_, v)| v[group[0]].is_some()))
        };
        let shown: Vec<usize> = (groups.iter().enumerate())
            .filter(|&(k, g)| k == 0 || filled(g))
            .flat_map(|(_, g)| g.iter().copied())
            .collect();
        let mut columns: Vec<String> = layer_names().collect();
        let headers = columns.len() + 1;
        columns.push("Statistic".into());
        let mut column_groups = vec![Vec::new(); columns.len()];
        let interval = |side: &str| match command.method {
            Some(CrosstabsMethod::MonteCarlo { level, .. }) => {
                vec![side.to_string(), format!("{level}% Confidence Interval")]
            }
            _ => Vec::new(),
        };
        for &k in &shown {
            let (name, group) = match k {
                0 => ("Value", Vec::new()),
                1 => ("df", Vec::new()),
                2 => ("Asymptotic Sig. (2-tailed)", Vec::new()),
                3 => ("Exact Sig. (2-tailed)", Vec::new()),
                4 => ("Exact Sig. (1-tailed)", Vec::new()),
                5 => ("Point Probability", Vec::new()),
                6 => (
                    "Significance",
                    vec!["Monte Carlo Sig. (2-tailed)".to_string()],
                ),
                7 => ("Lower Bound", interval("Monte Carlo Sig. (2-tailed)")),
                8 => ("Upper Bound", interval("Monte Carlo Sig. (2-tailed)")),
                9 => (
                    "Significance",
                    vec!["Monte Carlo Sig. (1-tailed)".to_string()],
                ),
                10 => ("Lower Bound", interval("Monte Carlo Sig. (1-tailed)")),
                _ => ("Upper Bound", interval("Monte Carlo Sig. (1-tailed)")),
            };
            columns.push(name.into());
            column_groups.push(group);
        }
        let mut rows = Vec::new();
        for (block, block_tests) in tests {
            for (name, values) in block_tests {
                let mut row = layer_cells(block);
                row.push(Cell::Text(name.into()));
                row.extend(shown.iter().map(|&k| match values[k] {
                    Some(v) if v.is_finite() => Cell::Number(v),
                    _ => Cell::Empty,
                }));
                rows.push(row);
            }
        }
        out.push(Table {
            caption: note.map(String::from),
            title: format!("Chi-Square Tests: {title}"),
            row_headers: headers,
            column_groups,
            columns,
            rows,
            ..Table::default()
        });
    }
    if !command.measures.is_empty() {
        out.extend(measure_tables(
            (&blocks, &sigs),
            &variables,
            command,
            (shown, &title),
            layer_names(),
        ));
    }
    if command.measures.contains(&CrosstabsMeasure::Cmh) {
        out.extend(cmh_tables(&blocks, command.cmh_odds, &title));
    }
    let chart = (command.barchart && !blocks.is_empty())
        .then(|| bar_chart(&blocks, &variables, shown, (title, command.descending)));
    Ok((out, chart, summary))
}

/// The CMH tables: the layers are the strata, each a 2 x 2 table over the two values the row
/// and the column variable take in all of them.
fn cmh_tables(blocks: &[Block], null_odds: f64, title: &str) -> Vec<Table> {
    let mut rows: Vec<&Key> = blocks.iter().flat_map(|b| &b.rows).collect();
    let mut columns: Vec<&Key> = blocks.iter().flat_map(|b| &b.columns).collect();
    for keys in [&mut rows, &mut columns] {
        keys.sort_by(|a, b| a.compare(b));
        keys.dedup_by(|a, b| a.compare(b).is_eq());
    }
    let plain =
        |name: &str, columns: Vec<String>, rows: Vec<Vec<Cell>>, caption: Option<String>| Table {
            title: format!("{name}: {title}"),
            row_headers: 1,
            column_groups: vec![Vec::new(); columns.len()],
            columns,
            rows,
            caption,
            ..Table::default()
        };
    let tests = || {
        ["", "Chi-Squared", "df", "Asymptotic Significance (2-sided)"]
            .map(String::from)
            .to_vec()
    };
    if rows.len() != 2 || columns.len() != 2 {
        let note = "The CMH tests need a row and a column variable with two values each.";
        return vec![plain(
            "Tests of Conditional Independence",
            tests(),
            Vec::new(),
            Some(note.into()),
        )];
    }
    let at = |keys: &[&Key], k: &Key| {
        keys.iter()
            .position(|x| x.compare(k).is_eq())
            .expect("listed")
    };
    let strata: Vec<[f64; 4]> = blocks
        .iter()
        .map(|b| {
            let mut f = [0.0; 4];
            for (i, r) in b.rows.iter().enumerate() {
                for (j, c) in b.columns.iter().enumerate() {
                    f[2 * at(&rows, r) + at(&columns, c)] += b.counts[i][j];
                }
            }
            f
        })
        .collect();
    let result = cmh(&strata, null_odds);
    let number = |v: Option<f64>| match v {
        Some(v) if v.is_finite() => Cell::Number(v),
        _ => Cell::Empty,
    };
    let chi_row = |name: &str, x: Option<f64>, df: f64| {
        vec![
            Cell::Text(name.into()),
            number(x),
            number(x.map(|_| df)),
            number(x.map(|x| chi_square_p(x, df))),
        ]
    };
    let (breslow, tarone, df) = match result.homogeneity {
        Some((b, t, df)) => (Some(b), Some(t), df),
        None => (None, None, 0.0),
    };
    let homogeneity = vec![
        chi_row("Breslow-Day", breslow, df),
        chi_row("Tarone's", tarone, df),
    ];
    let (cochran, mantel) = result.independence;
    let independence = vec![
        chi_row("Cochran's", cochran, 1.0),
        chi_row("Mantel-Haenszel", mantel, 1.0),
    ];
    let z = crate::dist::normal_quantile(0.975);
    let lines: Vec<(&str, Option<f64>)> = match result.odds {
        Some((theta, se, p)) => {
            let ln = theta.ln();
            vec![
                ("Estimate", Some(theta)),
                ("ln(Estimate)", Some(ln)),
                ("Standard Error of ln(Estimate)", Some(se)),
                ("Asymptotic Significance (2-sided)", Some(p)),
                ("Common Odds Ratio Lower Bound", Some((ln - z * se).exp())),
                ("Common Odds Ratio Upper Bound", Some((ln + z * se).exp())),
                ("ln(Common Odds Ratio) Lower Bound", Some(ln - z * se)),
                ("ln(Common Odds Ratio) Upper Bound", Some(ln + z * se)),
            ]
        }
        None => vec![("Estimate", None)],
    };
    let estimate = lines
        .into_iter()
        .map(|(name, v)| vec![Cell::Text(name.into()), number(v)])
        .collect();
    let caption = format!(
        "The confidence bounds are asymptotic 95% bounds. The significance tests the common odds ratio against {null_odds}."
    );
    vec![
        plain(
            "Tests of Homogeneity of the Odds Ratio",
            tests(),
            homogeneity,
            None,
        ),
        plain(
            "Tests of Conditional Independence",
            tests(),
            independence,
            None,
        ),
        plain(
            "Mantel-Haenszel Common Odds Ratio Estimate",
            vec![String::new(), "Value".into()],
            estimate,
            Some(caption),
        ),
    ]
}

/// The square table of counts over the categories of both variables, in ascending order, for
/// kappa; None when one variable holds numbers and the other strings.
fn square(block: &Block, variables: &[Variable]) -> Option<Vec<Vec<f64>>> {
    if variables[0].is_string() != variables[1].is_string() {
        return None;
    }
    let mut keys: Vec<&Key> = block.rows.iter().chain(&block.columns).collect();
    keys.sort_by(|a, b| a.compare(b));
    keys.dedup_by(|a, b| a.compare(b).is_eq());
    let at = |k: &Key| {
        keys.iter()
            .position(|x| x.compare(k).is_eq())
            .expect("listed")
    };
    let mut f = vec![vec![0.0; keys.len()]; keys.len()];
    for (i, r) in block.rows.iter().enumerate() {
        for (j, c) in block.columns.iter().enumerate() {
            f[at(r)][at(c)] += block.counts[i][j];
        }
    }
    Some(f)
}

/// The Symmetric Measures and Directional Measures tables, a block of lines per layer. A layer
/// with one row or column has only its case count.
fn measure_tables(
    (blocks, sigs): (&[Block], &[Found]),
    variables: &[Variable],
    command: &Crosstabs,
    (shown, title): (&Shown, &str),
    layer_names: impl Iterator<Item = String>,
) -> Vec<Table> {
    let numeric = !variables[0].is_string() && !variables[1].is_string();
    let layer_names: Vec<String> = layer_names.collect();
    let value = |k: &Key| match k {
        Key::Number(n) => *n,
        Key::Text(_) => 0.0,
    };
    let mut symmetric = Vec::new();
    let mut directional = Vec::new();
    let mut risks = Vec::new();
    let mut notes: Vec<&str> = Vec::new();
    let mut notes_risk = false;
    for (block, sigs) in blocks.iter().zip(sigs) {
        let method_cells = |name: &str, dependent: u8| {
            let stat = Stat::of(name, dependent);
            let found = sigs.iter().find(|(s, _)| Some(*s) == stat);
            measure_cells(command.method, found.map(|(_, o)| o))
        };
        let layer: Vec<Cell> = (2..variables.len())
            .zip(&block.layer)
            .map(|(v, k)| Cell::Text(shown.value(&variables[v], k)))
            .collect();
        // the measures take the categories in ascending order, whatever DVALUE shows
        let mut f = block.counts.clone();
        let mut row_values: Vec<f64> = block.rows.iter().map(value).collect();
        if command.descending {
            f.reverse();
            row_values.reverse();
        }
        let counts = Counts {
            f: &f,
            values: numeric.then(|| (row_values, block.columns.iter().map(value).collect())),
        };
        let usable = block.rows.len() >= 2 && block.columns.len() >= 2;
        let number = |v: Option<f64>| match v {
            Some(v) if v.is_finite() => Cell::Number(v),
            _ => Cell::Empty,
        };
        let mut group_shown = "";
        let mut lines = if usable {
            counts.symmetric(&command.measures)
        } else {
            Vec::new()
        };
        if usable && command.measures.contains(&CrosstabsMeasure::Kappa) {
            match square(block, variables) {
                Some(f) => lines.push(
                    Counts {
                        f: &f,
                        values: None,
                    }
                    .kappa(),
                ),
                None => {
                    notes.push("Kappa needs two variables of the same type, numbers or strings.")
                }
            }
        }
        for l in lines {
            let mut row = layer.clone();
            row.push(Cell::Text(if l.group == group_shown {
                String::new()
            } else {
                l.group.into()
            }));
            group_shown = l.group;
            row.push(Cell::Text(l.name.into()));
            row.extend([Some(l.value), l.ase, l.t, l.sig].map(number));
            row.extend(method_cells(l.name, l.dependent));
            symmetric.push(row);
        }
        let mut row = layer.clone();
        row.extend([Cell::Text("N of Valid Cases".into()), Cell::Empty]);
        row.extend([
            Cell::Number(block.total),
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
        ]);
        row.extend(measure_cells(command.method, None));
        symmetric.push(row);
        if command.measures.contains(&CrosstabsMeasure::Risk) {
            if (block.rows.len(), block.columns.len()) == (2, 2) {
                let (first, second) = match command.descending {
                    true => (&block.rows[1], &block.rows[0]),
                    false => (&block.rows[0], &block.rows[1]),
                };
                let (row_name, column_name) =
                    (shown.variable(&variables[0]), shown.variable(&variables[1]));
                let labels = [
                    format!(
                        "Odds Ratio for {row_name} ({} / {})",
                        shown.value(&variables[0], first),
                        shown.value(&variables[0], second)
                    ),
                    format!(
                        "For cohort {column_name} = {}",
                        shown.value(&variables[1], &block.columns[0])
                    ),
                    format!(
                        "For cohort {column_name} = {}",
                        shown.value(&variables[1], &block.columns[1])
                    ),
                ];
                for (label, (value, low, high)) in labels.into_iter().zip(risk(&f)) {
                    let mut row = layer.clone();
                    row.push(Cell::Text(label));
                    row.extend([value, low, high].map(|v| number(Some(v))));
                    risks.push(row);
                }
            } else {
                notes_risk = true;
            }
            let mut row = layer.clone();
            row.push(Cell::Text("N of Valid Cases".into()));
            row.extend([Cell::Number(block.total), Cell::Empty, Cell::Empty]);
            risks.push(row);
        }
        if usable {
            let (mut group_shown, mut name_shown) = ("", "");
            for l in counts.directional(&command.measures) {
                let mut row = layer.clone();
                row.push(Cell::Text(if l.group == group_shown {
                    String::new()
                } else {
                    l.group.into()
                }));
                group_shown = l.group;
                row.push(Cell::Text(if l.name == name_shown {
                    String::new()
                } else {
                    l.name.into()
                }));
                name_shown = l.name;
                row.push(Cell::Text(match l.dependent {
                    0 => "Symmetric".into(),
                    d => format!(
                        "{} Dependent",
                        shown.variable(&variables[usize::from(d) - 1])
                    ),
                }));
                row.extend([Some(l.value), l.ase, l.t, l.sig].map(number));
                row.extend(method_cells(l.name, l.dependent));
                directional.push(row);
            }
        }
    }
    let leaves = [
        "Value",
        "Asymptotic Standard Error",
        "Approximate T",
        "Approximate Significance",
    ];
    let table = |name: &str, labels: usize, rows: Vec<Vec<Cell>>| {
        let mut columns = layer_names.clone();
        columns.extend(std::iter::repeat_n(String::new(), labels));
        let headers = columns.len();
        columns.extend(leaves.map(String::from));
        let mut column_groups = vec![Vec::new(); columns.len()];
        match command.method {
            Some(CrosstabsMethod::Exact { .. }) => {
                columns.push("Exact Significance".into());
                column_groups.push(Vec::new());
            }
            Some(CrosstabsMethod::MonteCarlo { level, .. }) => {
                let group = "Monte Carlo Significance".to_string();
                let interval = vec![group.clone(), format!("{level}% Confidence Interval")];
                columns.extend(["Significance", "Lower Bound", "Upper Bound"].map(String::from));
                column_groups.extend([vec![group], interval.clone(), interval]);
            }
            None => {}
        }
        Table {
            title: format!("{name}: {title}"),
            row_headers: headers,
            column_groups,
            columns,
            rows,
            ..Table::default()
        }
    };
    use CrosstabsMeasure as M;
    let symmetric_asked = command.measures.iter().any(|m| {
        matches!(
            m,
            M::Phi | M::Cc | M::Btau | M::Ctau | M::Gamma | M::Corr | M::Kappa
        )
    });
    let mut out = Vec::new();
    if symmetric_asked {
        let mut t = table("Symmetric Measures", 2, symmetric);
        notes.dedup();
        t.caption = (!notes.is_empty()).then(|| notes.join(" "));
        out.push(t);
    }
    if !directional.is_empty() {
        out.push(table("Directional Measures", 3, directional));
    }
    if !risks.is_empty() {
        let mut columns = layer_names.clone();
        columns.push(String::new());
        let headers = columns.len();
        columns.extend(["Value", "Lower", "Upper"].map(String::from));
        let mut column_groups = vec![Vec::new(); columns.len()];
        for group in &mut column_groups[headers + 1..] {
            group.push("95% Confidence Interval".to_string());
        }
        out.push(Table {
            title: format!("Risk Estimate: {title}"),
            row_headers: headers,
            columns,
            column_groups,
            rows: risks,
            caption: notes_risk
                .then(|| "Risk estimates are computed only for 2 x 2 tables.".into()),
            ..Table::default()
        });
    }
    out
}

/// The exact or Monte Carlo cells of a measures line: its significance, or that with its
/// interval; none without METHOD.
fn measure_cells(method: Option<CrosstabsMethod>, outcome: Option<&Outcome>) -> Vec<Cell> {
    let mut cells = match method {
        None => Vec::new(),
        Some(CrosstabsMethod::Exact { .. }) => vec![Cell::Empty],
        Some(CrosstabsMethod::MonteCarlo { .. }) => vec![Cell::Empty; 3],
    };
    match outcome {
        Some(Outcome::Exact { two, .. }) => cells[0] = Cell::Number(*two),
        Some(Outcome::MonteCarlo { two, .. }) => {
            for (cell, v) in cells.iter_mut().zip(two) {
                *cell = Cell::Number(*v);
            }
        }
        None => {}
    }
    cells
}

/// The rows of the chi-square table widened for METHOD: 3 and 4 the exact two- and one-sided
/// significance, 5 the point probability, 6 to 8 and 9 to 11 the Monte Carlo two- and one-sided
/// significance with its interval. Fisher's test of a table larger than 2 x 2 gets a row of
/// its own; that of a 2 x 2 table keeps its exact values, adding the point probability.
type Row = (&'static str, [Option<f64>; 12]);

fn with_significance(
    rows: Vec<(&'static str, [Option<f64>; 5])>,
    sigs: &[(Stat, Outcome)],
    two_by_two: bool,
) -> Vec<Row> {
    let mut out: Vec<Row> = (rows.into_iter())
        .map(|(name, v)| {
            let mut wide = [None; 12];
            wide[..5].copy_from_slice(&v);
            (name, wide)
        })
        .collect();
    let fisher = sigs.iter().any(|(s, _)| *s == Stat::Fisher);
    let ratio = out.iter().position(|(n, _)| *n == "Likelihood Ratio");
    if let (true, false, Some(at)) = (fisher, two_by_two, ratio) {
        out.insert(at + 1, ("Fisher's Exact Test", [None; 12]));
    }
    for (name, v) in &mut out {
        let Some(stat) = Stat::of(name, 0) else {
            continue;
        };
        let Some((_, outcome)) = sigs.iter().find(|(s, _)| *s == stat) else {
            continue;
        };
        let kept = stat == Stat::Fisher && two_by_two;
        match *outcome {
            Outcome::Exact { two, one, point } => {
                if !kept {
                    (v[3], v[4]) = (Some(two), one);
                }
                v[5] = Some(point);
            }
            Outcome::MonteCarlo { two, one } if !kept => {
                for k in 0..3 {
                    v[6 + k] = Some(two[k]);
                    v[9 + k] = one.map(|o| o[k]);
                }
            }
            Outcome::MonteCarlo { .. } => {}
        }
    }
    out
}

/// The exact or Monte Carlo significance of the statistics the command asks for, on one block.
fn block_significances(
    block: &Block,
    variables: &[Variable],
    command: &Crosstabs,
    method: CrosstabsMethod,
    (random, warnings): (&mut Random, &mut Vec<String>),
) -> Found {
    use CrosstabsMeasure as M;
    let mut stats = Vec::new();
    if command.chisq {
        stats.extend([Stat::Pearson, Stat::Ratio, Stat::Fisher, Stat::Linear]);
    }
    for m in &command.measures {
        stats.extend_from_slice(match m {
            M::Phi | M::Cc => &[Stat::Pearson][..],
            M::Lambda => &[
                Stat::LambdaRows,
                Stat::LambdaColumns,
                Stat::TauRows,
                Stat::TauColumns,
            ],
            M::Uc => &[Stat::Ratio],
            M::Btau | M::Ctau | M::D => &[Stat::Concordance],
            M::Corr => &[Stat::Spearman, Stat::Linear],
            M::Kappa => &[Stat::Kappa],
            _ => &[],
        });
    }
    let mut unique = Vec::new();
    for s in stats {
        if !unique.contains(&s) {
            unique.push(s);
        }
    }
    let numeric = !variables[0].is_string() && !variables[1].is_string();
    let value = |k: &Key| match k {
        Key::Number(n) => *n,
        Key::Text(_) => 0.0,
    };
    // ascending rows, as the measures take them
    let mut f = block.counts.clone();
    let mut row_values: Vec<f64> = block.rows.iter().map(value).collect();
    if command.descending {
        f.reverse();
        row_values.reverse();
    }
    let column_values: Vec<f64> = block.columns.iter().map(value).collect();
    let square = match unique.contains(&Stat::Kappa) {
        true => square(block, variables),
        false => None,
    };
    let input = Input {
        f: &f,
        values: numeric.then_some((&row_values[..], &column_values[..])),
        square: square.as_deref(),
        truncate: block.truncate,
    };
    significances(&input, &unique, method, random, warnings)
}

/// Integer mode: the names of a TABLES list, where TO and ALL follow the order of VARIABLES.
fn listed(items: &[VarItem], ranges: &[(String, f64, f64)]) -> Result<Vec<String>, Error> {
    let position = |name: &str| {
        let found = ranges
            .iter()
            .position(|(n, ..)| n.eq_ignore_ascii_case(name));
        found.ok_or_else(|| {
            Error(format!(
                "{name} is not on VARIABLES; in integer mode TABLES uses only those"
            ))
        })
    };
    let mut names = Vec::new();
    for item in items {
        let (first, last) = match item {
            VarItem::All => (0, ranges.len() - 1),
            VarItem::Name(name) => (position(name)?, position(name)?),
            VarItem::Range(a, b) => (position(a)?, position(b)?),
        };
        if first > last {
            return Err(Error(format!(
                "in a TO list the first variable must come before the second on VARIABLES ({} TO {})",
                ranges[first].0, ranges[last].0
            )));
        }
        names.extend(ranges[first..=last].iter().map(|(n, ..)| n.clone()));
    }
    Ok(names)
}

impl Engine {
    pub(crate) fn crosstabs(&mut self, command: &Crosstabs) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let (shown, outline) = (self.shown(), self.outline());
        let mut out = Vec::new();
        let mut charts = Vec::new();
        let mut summary = Vec::new();
        let mut warnings = Vec::new();
        // integer mode: every variable of VARIABLES with its range, a later range replacing an
        // earlier one
        let mut ranges: Vec<(String, f64, f64)> = Vec::new();
        for (items, low, high) in &command.ranges {
            for name in self.expand(items)? {
                if data.variable(&name)?.is_string() {
                    return Err(Error(format!(
                        "integer mode needs numeric variables, and {name} is a string"
                    )));
                }
                match ranges.iter_mut().find(|(n, ..)| *n == name) {
                    Some(found) => *found = (name, *low, *high),
                    None => ranges.push((name, *low, *high)),
                }
            }
        }
        for dimensions in &command.tables {
            let lists: Vec<Vec<String>> = dimensions
                .iter()
                .map(|items| match ranges.is_empty() {
                    true => self.expand(items),
                    false => listed(items, &ranges),
                })
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
                let (tables, chart, row) = tables(
                    data,
                    &names,
                    command,
                    &ranges,
                    (&shown, &outline),
                    (&mut self.random.borrow_mut(), &mut warnings),
                )?;
                out.extend(tables);
                charts.extend(chart);
                summary.push(row);
            }
        }
        self.charts.extend(charts);
        self.warnings.extend(warnings);
        let mut columns = vec![String::new()];
        let mut column_groups = vec![Vec::new()];
        for group in ["Valid", "Missing", "Total"] {
            for leaf in ["N", "Percent"] {
                columns.push(leaf.into());
                column_groups.push(vec!["Cases".to_string(), group.to_string()]);
            }
        }
        out.insert(
            0,
            Table {
                title: "Case Processing Summary".into(),
                row_headers: 1,
                columns,
                column_groups,
                rows: summary,
                ..Table::default()
            },
        );
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
