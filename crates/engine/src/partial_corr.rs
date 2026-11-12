//! PARTIAL CORR: partial correlations controlling for other variables.
//!
//! Rules from the Command Syntax Reference (PARTIAL CORR) and the Algorithms manual (PARTIAL
//! CORR Algorithms):
//! - Zero-order Pearson correlations with weights as they are; a case with a missing, zero or
//!   negative weight is left out. LISTWISE (default) uses the cases valid on every variable of
//!   the list, controls included; ANALYSIS each pair's valid cases.
//! - Partials come from the lower orders: r_ij.S = (r_ij.S' - r_ik.S' r_jk.S') /
//!   sqrt((1 - r_ik.S'^2)(1 - r_jk.S'^2)) with S = S' and k. A denominator below 1e-20, a
//!   missing lower coefficient or a result beyond 1 in absolute value leaves it empty.
//! - t = r sqrt(df / (1 - r^2)) with df = M - order - 2, M the smallest weight sum of the
//!   zero-order correlations among the variables involved. The significance is two-tailed by
//!   default, as the Command Syntax Reference gives it (the Algorithms manual says one-tailed).
//! - Order values give one block per combination of that many controls, in list order.
//! - CORR shows the zero-order correlations of every variable of the list first; BADCORR only
//!   when one of them cannot be computed. CONDENSED shows the coefficients alone, marked * at
//!   0.05 or less and ** at 0.01 or less; SERIAL leaves the redundant half of a square matrix
//!   empty.

use crate::correlations::pair;
use crate::descriptives::{case_weights, number};
use crate::dist::t_p;
use crate::means::is_valid;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Values, Variable};
use statscraft_syntax::{PartialCorr, PartialFormat};
use std::collections::HashMap;

/// Every choice of `k` of the indices 0..n, in order.
fn choices(n: usize, k: usize) -> Vec<Vec<usize>> {
    if k == 0 {
        return vec![Vec::new()];
    }
    if k > n {
        return Vec::new();
    }
    let mut out = Vec::new();
    for last in k - 1..n {
        for mut rest in choices(last, k - 1) {
            rest.push(last);
            out.push(rest);
        }
    }
    out
}

/// The partial correlation of i and j controlling for `controls`, by recursion on its last
/// element; `memo` keys are (i, j, number of leading controls).
fn partial(
    r: &[Vec<Option<f64>>],
    i: usize,
    j: usize,
    controls: &[usize],
    memo: &mut HashMap<(usize, usize, usize), Option<f64>>,
) -> Option<f64> {
    let Some((&k, rest)) = controls.split_last() else {
        return r[i][j];
    };
    let key = (i.min(j), i.max(j), controls.len());
    if let Some(found) = memo.get(&key) {
        return *found;
    }
    let value = (|| {
        let a = partial(r, i, j, rest, memo)?;
        let b = partial(r, i, k, rest, memo)?;
        let c = partial(r, j, k, rest, memo)?;
        let denominator = ((1.0 - b * b) * (1.0 - c * c)).sqrt();
        if denominator.is_nan() || denominator < 1e-20 {
            return None;
        }
        let value = (a - b * c) / denominator;
        (value.abs() <= 1.0).then_some(value)
    })();
    memo.insert(key, value);
    value
}

impl Engine {
    pub(crate) fn partial_corr(&mut self, command: &PartialCorr) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let weights = case_weights(data)?;
        let weight = |c: usize| weights.as_ref().map_or(1.0, |w| w[c]);
        let value = |v: &Variable, c: usize| match v.values() {
            Values::Numeric(values) => values.get(c).unwrap_or(f64::NAN),
            Values::String(_) => f64::NAN,
        };
        let tail = if command.one_tailed {
            "Significance (1-tailed)"
        } else {
            "Significance (2-tailed)"
        };
        let mut out = Vec::new();
        for list in &command.lists {
            let rows = self.expand(&list.rows)?;
            let square = list.columns.is_none();
            let columns = match &list.columns {
                Some(columns) => self.expand(columns)?,
                None => rows.clone(),
            };
            let controls = self.expand(&list.controls)?;
            let mut all: Vec<String> = Vec::new();
            for name in rows.iter().chain(&columns).chain(&controls) {
                if !all.contains(name) {
                    all.push(name.clone());
                }
            }
            let variables: Vec<Variable> = all
                .iter()
                .map(|n| data.variable(n))
                .collect::<Result<_, _>>()?;
            if let Some(v) = variables.iter().find(|v| v.is_string()) {
                return Err(Error(format!(
                    "PARTIAL CORR needs numeric variables, but {} is a string variable",
                    v.name()
                )));
            }
            let orders: Vec<usize> = if list.orders.is_empty() {
                vec![controls.len()]
            } else {
                list.orders.iter().map(|&o| o as usize).collect()
            };
            if let Some(o) = orders.iter().find(|&&o| o > controls.len()) {
                return Err(Error(format!(
                    "PARTIAL CORR: order {o} needs at least {o} control variables, and the list has {}",
                    controls.len()
                )));
            }
            let index = |name: &String| all.iter().position(|a| a == name).expect("listed");
            let valid = |v: &Variable, c: usize| is_valid(v, c, command.include);
            let cases: Vec<usize> = (0..data.case_count())
                .filter(|&c| weight(c) > 0.0)
                .filter(|&c| command.pairwise || variables.iter().all(|v| valid(v, c)))
                .collect();
            let m = all.len();
            let mut r = vec![vec![None; m]; m];
            let mut w = vec![vec![0.0; m]; m];
            for a in 0..m {
                for b in a..m {
                    let (x, y) = (&variables[a], &variables[b]);
                    let used: Vec<(f64, f64, f64)> = cases
                        .iter()
                        .filter(|&&c| valid(x, c) && valid(y, c))
                        .map(|&c| (value(x, c), value(y, c), weight(c)))
                        .collect();
                    let sum: f64 = used.iter().map(|u| u.2).sum();
                    let coefficient = if a == b {
                        Some(1.0)
                    } else {
                        Some(pair(&used).r).filter(|r| r.is_finite() && r.abs() <= 1.0)
                    };
                    r[a][b] = coefficient;
                    r[b][a] = coefficient;
                    w[a][b] = sum;
                    w[b][a] = sum;
                }
            }
            if command.descriptives {
                let rows = variables
                    .iter()
                    .map(|v| {
                        let used: Vec<(f64, f64)> = cases
                            .iter()
                            .filter(|&&c| valid(v, c))
                            .map(|&c| (value(v, c), weight(c)))
                            .collect();
                        let n: f64 = used.iter().map(|u| u.1).sum();
                        let mean = used.iter().map(|(x, w)| x * w).sum::<f64>() / n;
                        let squares: f64 = used.iter().map(|(x, w)| w * (x - mean).powi(2)).sum();
                        let sd = (n > 1.0).then(|| (squares / (n - 1.0)).sqrt());
                        vec![
                            Cell::Text(shown.variable(v)),
                            number(Some(mean)),
                            number(sd),
                            Cell::Number(n),
                        ]
                    })
                    .collect();
                out.push(Table {
                    title: "Descriptive Statistics".into(),
                    row_headers: 1,
                    columns: ["", "Mean", "Std. Deviation", "N"]
                        .map(String::from)
                        .to_vec(),
                    column_groups: vec![Vec::new(); 4],
                    rows,
                    ..Table::default()
                });
            }
            let zero_missing = (0..m).any(|a| (0..m).any(|b| r[a][b].is_none()));
            let zero_order = command.zero_order || (command.bad_corr && zero_missing);
            // the zero-order block names the controls too, in rows and columns
            let mut row_names = rows.clone();
            let mut column_names = columns.clone();
            if zero_order {
                for c in &controls {
                    if !row_names.contains(c) {
                        row_names.push(c.clone());
                    }
                    if !column_names.contains(c) {
                        column_names.push(c.clone());
                    }
                }
            }
            let mut blocks: Vec<(String, Vec<usize>, usize)> = Vec::new();
            if zero_order {
                blocks.push(("-none-".into(), Vec::new(), row_names.len()));
            }
            for &order in &orders {
                for chosen in choices(controls.len(), order) {
                    let label = chosen
                        .iter()
                        .map(|&k| shown.named(data, &controls[k]))
                        .collect::<Vec<_>>()
                        .join(" & ");
                    let set = chosen.iter().map(|&k| index(&controls[k])).collect();
                    blocks.push((label, set, rows.len()));
                }
            }
            let statistics: Vec<&str> = if command.format == PartialFormat::Condensed {
                vec!["Correlation"]
            } else {
                vec!["Correlation", tail, "df"]
            };
            let mut table_rows = Vec::new();
            let mut flagged = false;
            for (label, set, count) in &blocks {
                let mut memo = HashMap::new();
                for (ri, row_name) in row_names.iter().take(*count).enumerate() {
                    let i = index(row_name);
                    let mut cells: Vec<Vec<Cell>> = vec![Vec::new(); statistics.len()];
                    for (ci, column_name) in column_names.iter().enumerate() {
                        let j = index(column_name);
                        let in_block = set.is_empty() || ci < columns.len();
                        let redundant = command.format == PartialFormat::Serial
                            && square
                            && ci <= ri
                            && ci < row_names.len();
                        if !in_block || redundant {
                            for c in cells.iter_mut() {
                                c.push(Cell::Empty);
                            }
                            continue;
                        }
                        let coefficient = if i == j {
                            Some(1.0)
                        } else {
                            partial(&r, i, j, set, &mut memo)
                        };
                        let mut involved = vec![i, j];
                        involved.extend(set);
                        let smallest = involved
                            .iter()
                            .flat_map(|&a| involved.iter().map(move |&b| (a, b)))
                            .map(|(a, b)| w[a][b])
                            .fold(f64::INFINITY, f64::min);
                        let df = if i == j {
                            0.0
                        } else {
                            smallest - set.len() as f64 - 2.0
                        };
                        let sig = coefficient.filter(|_| i != j && df > 0.0).map(|rho| {
                            let t = rho * (df / (1.0 - rho * rho)).sqrt();
                            let p = t_p(t, df);
                            if command.one_tailed { p / 2.0 } else { p }
                        });
                        let coefficient_cell = match (command.format, coefficient, sig) {
                            (PartialFormat::Condensed, Some(rho), Some(p)) if p <= 0.05 => {
                                flagged = true;
                                let stars = if p <= 0.01 { "**" } else { "*" };
                                Cell::Text(format!("{rho:.3}{stars}"))
                            }
                            _ => number(coefficient),
                        };
                        cells[0].push(coefficient_cell);
                        if cells.len() > 1 {
                            cells[1].push(number(sig));
                            cells[2].push(number(coefficient.map(|_| df)));
                        }
                    }
                    for (k, (statistic, values)) in statistics.iter().zip(cells).enumerate() {
                        let first = |text: &str, yes: bool| {
                            if yes {
                                Cell::Text(text.to_string())
                            } else {
                                Cell::Empty
                            }
                        };
                        let mut row = vec![
                            first(label, ri == 0 && k == 0),
                            first(&shown.named(data, row_name), k == 0),
                            Cell::Text(statistic.to_string()),
                        ];
                        row.extend(values);
                        table_rows.push(row);
                    }
                }
            }
            let mut header = vec![
                "Control Variables".to_string(),
                String::new(),
                String::new(),
            ];
            header.extend(column_names.iter().map(|n| shown.named(data, n)));
            let mut captions = Vec::new();
            if flagged {
                let side = if command.one_tailed {
                    "1-tailed"
                } else {
                    "2-tailed"
                };
                captions.push(format!(
                    "* significant at the 0.05 level, ** at the 0.01 level ({side})."
                ));
            }
            out.push(Table {
                title: "Correlations".into(),
                row_headers: 3,
                column_groups: vec![Vec::new(); header.len()],
                columns: header,
                rows: table_rows,
                caption: (!captions.is_empty()).then(|| captions.join(" ")),
                ..Table::default()
            });
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_in_order() {
        assert_eq!(choices(3, 2), vec![vec![0, 1], vec![0, 2], vec![1, 2]]);
        assert_eq!(choices(2, 0), vec![Vec::<usize>::new()]);
    }

    #[test]
    fn first_order_partial_by_formula() {
        let r = vec![
            vec![Some(1.0), Some(0.5), Some(0.3)],
            vec![Some(0.5), Some(1.0), Some(0.4)],
            vec![Some(0.3), Some(0.4), Some(1.0)],
        ];
        let expected = (0.5 - 0.3 * 0.4) / ((1.0 - 0.09) * (1.0 - 0.16f64)).sqrt();
        let found = partial(&r, 0, 1, &[2], &mut HashMap::new()).unwrap();
        assert!((found - expected).abs() < 1e-15);
    }
}
