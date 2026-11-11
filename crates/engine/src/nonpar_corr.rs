//! NONPAR CORR: Spearman's rho and Kendall's tau-b with their significance.
//!
//! Rules from the Command Syntax Reference (NONPAR CORR) and the Algorithms manual (NONPAR CORR
//! Algorithms):
//! - Each variable is ranked on the cases of the coefficient, ties getting their average rank.
//! - A weight repeats a case as many times as the weight rounded to a whole number; a case whose
//!   rounded weight is 0 drops out, and N counts the repeated cases.
//! - rho = (Tx + Ty - sum d^2) / (2 sqrt(Tx Ty)) with Tx = (N^3 - N - sum(t^3 - t)) / 12, which is
//!   the Pearson correlation of the ranks; it is tested with t = rho sqrt((N - 2) / (1 - rho^2))
//!   on N - 2 df.
//! - tau-b = S / sqrt((N^2 - N - tx) / 2 (N^2 - N - ty) / 2), S = concordant minus discordant
//!   pairs, tested with Z = S / sqrt(var S) and the tie-corrected variance of Kendall (1955).
//! - A coefficient whose denominator is 0 (a constant variable) is not computed and shows empty.
//! - PAIRWISE (default) uses the cases valid on both variables; LISTWISE the cases valid on
//!   every variable of the list, each list on its own, and shows N once in a caption.
//! - ONETAIL halves the two-tailed significance; NOSIG marks coefficients significant at 0.05
//!   with * and at 0.01 with ** and shows them as text with three decimals.

use crate::correlations::pair;
use crate::descriptives::{case_weights, number};
use crate::dist::{normal_p, t_p};
use crate::means::is_valid;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Values, Variable};
use statscraft_syntax::{CorrelationsLayout, NonparCorr};

/// The average ranks of `values` (each standing for `weights` cases), and over the groups of
/// t tied cases the sums of t^2 - t, (t^2 - t)(t - 2) and (t^2 - t)(2t + 5).
pub(crate) fn ranks(values: &[f64], weights: &[f64]) -> (Vec<f64>, [f64; 3]) {
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| values[a].total_cmp(&values[b]));
    let mut ranks = vec![0.0; values.len()];
    let mut ties = [0.0; 3];
    let (mut below, mut start) = (0.0, 0);
    while start < order.len() {
        let end = start
            + order[start..]
                .iter()
                .take_while(|&&i| values[i] == values[order[start]])
                .count();
        let t: f64 = order[start..end].iter().map(|&i| weights[i]).sum();
        for &i in &order[start..end] {
            ranks[i] = below + (t + 1.0) / 2.0;
        }
        let square = t * t - t;
        ties[0] += square;
        ties[1] += square * (t - 2.0);
        ties[2] += square * (2.0 * t + 5.0);
        below += t;
        start = end;
    }
    (ranks, ties)
}

/// Kendall's S: the weight of concordant minus discordant pairs, by a sweep over x with a
/// Fenwick tree of the weights already seen at each rank of y.
fn kendall_s(x: &[f64], y: &[f64], weights: &[f64]) -> f64 {
    let mut levels: Vec<f64> = y.to_vec();
    levels.sort_by(f64::total_cmp);
    levels.dedup();
    let level = |v: f64| levels.partition_point(|&l| l < v);
    let mut tree = vec![0.0; levels.len() + 1];
    // the weight seen at levels 0..=i
    let prefix = |tree: &[f64], i: usize| {
        let (mut sum, mut k) = (0.0, i + 1);
        while k > 0 {
            sum += tree[k];
            k &= k - 1;
        }
        sum
    };
    let mut order: Vec<usize> = (0..x.len()).collect();
    order.sort_by(|&a, &b| x[a].total_cmp(&x[b]));
    let (mut s, mut seen, mut start) = (0.0, 0.0, 0);
    while start < order.len() {
        let end = start
            + order[start..]
                .iter()
                .take_while(|&&i| x[i] == x[order[start]])
                .count();
        // pairs tied on x count for nothing, so the group is compared before it is added
        for &i in &order[start..end] {
            let l = level(y[i]);
            let lower = if l == 0 { 0.0 } else { prefix(&tree, l - 1) };
            let higher = seen - prefix(&tree, l);
            s += weights[i] * (lower - higher);
        }
        for &i in &order[start..end] {
            let mut k = level(y[i]) + 1;
            while k < tree.len() {
                tree[k] += weights[i];
                k += k & k.wrapping_neg();
            }
            seen += weights[i];
        }
        start = end;
    }
    s
}

/// Coefficient, two-tailed significance and N of one pair of variables over (x, y, weight).
fn coefficient(cases: &[(f64, f64, f64)], kendall: bool) -> [f64; 3] {
    let x: Vec<f64> = cases.iter().map(|c| c.0).collect();
    let y: Vec<f64> = cases.iter().map(|c| c.1).collect();
    let w: Vec<f64> = cases.iter().map(|c| c.2).collect();
    let n: f64 = w.iter().sum();
    let (rx, tx) = ranks(&x, &w);
    let (ry, ty) = ranks(&y, &w);
    if !kendall {
        let ranked: Vec<(f64, f64, f64)> = (0..x.len()).map(|i| (rx[i], ry[i], w[i])).collect();
        let rho = pair(&ranked).r;
        let t = rho * ((n - 2.0) / (1.0 - rho * rho)).sqrt();
        return [rho, t_p(t, n - 2.0), n];
    }
    let s = kendall_s(&x, &y, &w);
    let k = n * n - n;
    let tau = s / ((k - tx[0]) / 2.0 * (k - ty[0]) / 2.0).sqrt();
    let variance = (k * (2.0 * n + 5.0) - tx[2] - ty[2]) / 18.0
        + tx[1] * ty[1] / (9.0 * k * (n - 2.0))
        + tx[0] * ty[0] / (2.0 * k);
    let sig = if tau.is_finite() {
        normal_p(s / variance.sqrt())
    } else {
        f64::NAN
    };
    [tau, sig, n]
}

impl Engine {
    pub(crate) fn nonpar_corr(&mut self, command: &NonparCorr) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let mut lists: Vec<(Vec<String>, Vec<String>, bool)> = Vec::new();
        for (rows, columns) in &command.lists {
            let rows = self.expand(rows)?;
            let columns = columns.as_ref().map(|c| self.expand(c)).transpose()?;
            let square = columns.is_none();
            lists.push((rows.clone(), columns.unwrap_or(rows), square));
        }
        let weights = case_weights(data)?;
        // a weight repeats the case as many times as it rounds to
        let weight = |c: usize| weights.as_ref().map_or(1.0, |w| w[c].round());
        let cases: Vec<usize> = (0..data.case_count())
            .filter(|&c| weight(c) > 0.0)
            .collect();
        let valid = |v: &Variable, c: usize| is_valid(v, c, command.include);
        let value = |v: &Variable, c: usize| match v.values() {
            Values::Numeric(values) => values.get(c).unwrap_or(f64::NAN),
            Values::String(_) => f64::NAN,
        };
        let tail = if command.one_tailed {
            "Sig. (1-tailed)"
        } else {
            "Sig. (2-tailed)"
        };
        let mut methods = Vec::new();
        if command.kendall {
            methods.push((true, "Kendall's tau_b"));
        }
        if command.spearman {
            methods.push((false, "Spearman's rho"));
        }
        let mut out = Vec::new();
        for (rows, columns, square) in &lists {
            let mut variables = Vec::new();
            for name in rows.iter().chain(columns) {
                let variable = data.variable(name)?;
                if variable.is_string() {
                    return Err(Error(format!(
                        "NONPAR CORR needs numeric variables, but {name} is a string variable"
                    )));
                }
                variables.push(variable);
            }
            let (row_vars, column_vars) = variables.split_at(rows.len());
            let complete: Vec<usize> = cases
                .iter()
                .copied()
                .filter(|&c| variables.iter().all(|v| valid(v, c)))
                .collect();
            let listwise_n: f64 = complete.iter().map(|&c| weight(c)).sum();
            let mut table_rows = Vec::new();
            let mut flagged = false;
            for &(kendall, method) in &methods {
                for (i, x) in row_vars.iter().enumerate() {
                    let mut statistics: Vec<(&str, Vec<Cell>)> =
                        vec![("Correlation Coefficient", Vec::new()), (tail, Vec::new())];
                    if !command.listwise {
                        statistics.push(("N", Vec::new()));
                    }
                    for (j, y) in column_vars.iter().enumerate() {
                        let hidden = *square
                            && match command.layout {
                                CorrelationsLayout::Full => false,
                                CorrelationsLayout::Lower => j > i,
                                CorrelationsLayout::LowerNoDiagonal => j >= i,
                            };
                        let used: Vec<(f64, f64, f64)> =
                            if command.listwise { &complete } else { &cases }
                                .iter()
                                .filter(|&&c| valid(x, c) && valid(y, c))
                                .map(|&c| (value(x, c), value(y, c), weight(c)))
                                .collect();
                        let [r, mut sig, n] = coefficient(&used, kendall);
                        if command.one_tailed {
                            sig /= 2.0;
                        }
                        let same = x.name() == y.name();
                        let sig = (!same && r.is_finite()).then_some(sig);
                        let r_cell = match sig {
                            Some(s) if command.flag && s < 0.05 => {
                                flagged = true;
                                let stars = if s < 0.01 { "**" } else { "*" };
                                Cell::Text(format!("{r:.3}{stars}"))
                            }
                            _ => number(Some(if same && r.is_finite() { 1.0 } else { r })),
                        };
                        let mut values = vec![r_cell, number(sig)];
                        if !command.listwise {
                            values.push(Cell::Number(n));
                        }
                        for ((_, cells), value) in statistics.iter_mut().zip(values) {
                            cells.push(if hidden { Cell::Empty } else { value });
                        }
                    }
                    for (k, (name, cells)) in statistics.into_iter().enumerate() {
                        let first = |text: &str, yes: bool| {
                            if yes {
                                Cell::Text(text.to_string())
                            } else {
                                Cell::Empty
                            }
                        };
                        let mut row = vec![
                            first(method, i == 0 && k == 0),
                            first(x.name(), k == 0),
                            Cell::Text(name.to_string()),
                        ];
                        row.extend(cells);
                        table_rows.push(row);
                    }
                }
            }
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
            if command.listwise {
                captions.push(format!("Listwise N = {listwise_n}."));
            }
            let mut header = vec![String::new(); 3];
            header.extend(columns.iter().cloned());
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
    fn kendall_s_counts_pairs() {
        // brute force over every pair of repeated cases
        let x: [f64; 7] = [1.0, 2.0, 2.0, 3.0, 5.0, 4.0, 1.0];
        let y = [2.0, 1.0, 3.0, 3.0, 4.0, 4.0, 2.0];
        let w = [1.0, 2.0, 1.0, 3.0, 1.0, 2.0, 1.0];
        let mut brute = 0.0;
        for i in 0..x.len() {
            for j in i + 1..x.len() {
                let sign = ((x[j] - x[i]) * (y[j] - y[i])).signum();
                if x[j] != x[i] && y[j] != y[i] {
                    brute += w[i] * w[j] * sign;
                }
            }
        }
        assert_eq!(kendall_s(&x, &y, &w), brute);
        let (ranks, ties) = ranks(&x, &w);
        // values 1 (2 cases), 2 (3), 3 (3), 4 (2), 5 (1)
        assert_eq!(ranks, [1.5, 4.0, 4.0, 7.0, 11.0, 9.5, 1.5]);
        assert_eq!(ties[0], 2.0 + 6.0 + 6.0 + 2.0);
    }
}
