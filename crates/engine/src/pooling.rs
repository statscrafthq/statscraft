//! Pooled results of procedures run on multiply imputed data, split by Imputation_.
//!
//! Rules from the Algorithms manual (Multiple Imputation: Pooling), in our words:
//! - With m imputations, the pooled estimate is the mean of the m estimates; U is the mean of
//!   their squared standard errors, B the variance of the estimates (m - 1 divisor), and the
//!   total variance T = U + (1 + 1/m) B. An imputation without an estimate is left out.
//! - r = (1 + 1/m) B / U is the relative increase in variance, the degrees of freedom are
//!   (m - 1)(1 + 1/r)^2, the fraction of missing information is (r + 2 / (df + 3)) / (r + 1)
//!   and the relative efficiency 1 / (1 + fraction / m). The interval is the estimate plus and
//!   minus the t quantile at those df times sqrt(T).
//!
//! SET MIOUTPUT chooses the parts shown: OBSERVED the original data (Imputation_ = 0),
//! IMPUTED each imputation, POOLED the pooled table and DIAGNOSTICS its diagnostic columns.

use crate::dist::{normal_quantile, t_quantile};
use crate::{Cell, Table};

/// What a procedure gives for pooling while it runs on one imputation.
#[derive(Clone)]
pub(crate) enum PoolItem {
    /// DESCRIPTIVES: the mean of a variable and its squared standard error.
    Mean { variable: String, q: f64, u: f64 },
    /// FREQUENCIES: one row of a frequency table.
    Count {
        variable: String,
        value: Cell,
        label: Option<String>,
        missing: bool,
        count: f64,
        total: f64,
        valid: f64,
    },
}

/// Rubin's rules for one scalar estimate.
#[derive(Debug, PartialEq)]
pub(crate) struct Pooled {
    pub estimate: f64,
    pub within: f64,
    pub between: f64,
    pub total: f64,
    /// None when the estimates do not vary between imputations: infinite df.
    pub df: Option<f64>,
    pub increase: f64,
    pub fraction: f64,
    pub efficiency: f64,
}

/// Pools (estimate, squared standard error) pairs; None with fewer than two usable pairs.
pub(crate) fn rubin(pairs: &[(f64, f64)]) -> Option<Pooled> {
    let pairs: Vec<(f64, f64)> = pairs
        .iter()
        .copied()
        .filter(|(q, u)| q.is_finite() && u.is_finite())
        .collect();
    let m = pairs.len() as f64;
    if pairs.len() < 2 {
        return None;
    }
    let estimate = pairs.iter().map(|p| p.0).sum::<f64>() / m;
    let within = pairs.iter().map(|p| p.1).sum::<f64>() / m;
    let between = pairs.iter().map(|p| (p.0 - estimate).powi(2)).sum::<f64>() / (m - 1.0);
    let total = within + (1.0 + 1.0 / m) * between;
    let (df, increase, fraction) = if between <= 0.0 {
        (None, 0.0, 0.0)
    } else if within <= 0.0 {
        (Some(m - 1.0), f64::INFINITY, 1.0)
    } else {
        let r = (1.0 + 1.0 / m) * between / within;
        let df = (m - 1.0) * (1.0 + 1.0 / r).powi(2);
        (Some(df), r, (r + 2.0 / (df + 3.0)) / (r + 1.0))
    };
    Some(Pooled {
        estimate,
        within,
        between,
        total,
        df,
        increase,
        fraction,
        efficiency: 1.0 / (1.0 + fraction / m),
    })
}

/// The settings of SET MIOUTPUT: observed, imputed, pooled, diagnostics.
pub(crate) fn mioutput(setting: &str) -> [bool; 4] {
    let upper = setting.to_uppercase();
    let words: Vec<&str> = upper
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    ["OBSERVED", "IMPUTED", "POOLED", "DIAGNOSTICS"].map(|k| words.contains(&k))
}

/// A pooled frequency row: variable, value, label, missing, and the averaged count, percent
/// and valid percent.
type CountRow<'a> = (&'a str, &'a Cell, &'a Option<String>, bool, [f64; 3]);

/// One split group's pooling items: the values of the other split variables and the items.
pub(crate) struct Imputed {
    pub others: Vec<String>,
    pub items: Vec<PoolItem>,
}

fn num(x: f64) -> Cell {
    if x.is_finite() {
        Cell::Number(x)
    } else {
        Cell::Empty
    }
}

/// The pooled tables of the imputations (Imputation_ 1 and up). `others` names the split
/// variables after Imputation_.
pub(crate) fn tables(groups: &[Imputed], others: &[String], diagnostics: bool) -> Vec<Table> {
    // the combinations of the other split variables, in order of appearance
    let mut keys: Vec<&Vec<String>> = Vec::new();
    for g in groups {
        if !keys.contains(&&g.others) {
            keys.push(&g.others);
        }
    }
    let mut means: Vec<Vec<Cell>> = Vec::new();
    let mut counts: Vec<Vec<Cell>> = Vec::new();
    for key in keys {
        let mine: Vec<&Imputed> = groups.iter().filter(|g| &g.others == key).collect();
        let lead = || key.iter().map(|t| Cell::Text(t.clone()));
        // means, variable by variable in order of appearance
        let mut names: Vec<&str> = Vec::new();
        for item in mine.iter().flat_map(|g| &g.items) {
            if let PoolItem::Mean { variable, .. } = item
                && !names.contains(&variable.as_str())
            {
                names.push(variable);
            }
        }
        for name in names {
            let pairs: Vec<(f64, f64)> = mine
                .iter()
                .flat_map(|g| &g.items)
                .filter_map(|item| match item {
                    PoolItem::Mean { variable, q, u } if variable == name => Some((*q, *u)),
                    _ => None,
                })
                .collect();
            let Some(p) = rubin(&pairs) else { continue };
            let se = p.total.sqrt();
            let half = se
                * match p.df {
                    Some(df) => t_quantile(0.025, df),
                    None => -normal_quantile(0.025),
                };
            let mut row: Vec<Cell> = lead().collect();
            row.push(Cell::Text(name.to_string()));
            row.extend([
                num(p.estimate),
                num(se),
                p.df.map_or(Cell::Empty, num),
                num(p.estimate - half),
                num(p.estimate + half),
            ]);
            if diagnostics {
                row.extend([num(p.fraction), num(p.increase), num(p.efficiency)]);
            }
            means.push(row);
        }
        // frequencies: each row averaged over the imputations, absent rows counting 0
        let m = mine.len() as f64;
        let mut rows: Vec<CountRow> = Vec::new();
        for g in &mine {
            for item in &g.items {
                let PoolItem::Count {
                    variable,
                    value,
                    label,
                    missing,
                    count,
                    total,
                    valid,
                } = item
                else {
                    continue;
                };
                let share = |part: f64, whole: f64| match whole > 0.0 {
                    true => 100.0 * part / whole,
                    false => 0.0,
                };
                let add = [
                    count / m,
                    share(*count, *total) / m,
                    if *missing {
                        0.0
                    } else {
                        share(*count, *valid) / m
                    },
                ];
                match rows
                    .iter_mut()
                    .find(|r| r.0 == variable && r.1 == value && r.3 == *missing)
                {
                    Some(r) => (0..3).for_each(|i| r.4[i] += add[i]),
                    None => rows.push((variable, value, label, *missing, add)),
                }
            }
        }
        // valid values first, each variable in order of appearance
        let mut order: Vec<&str> = Vec::new();
        for r in &rows {
            if !order.contains(&r.0) {
                order.push(r.0);
            }
        }
        for name in order {
            for missing in [false, true] {
                for (variable, value, label, miss, [count, percent, valid]) in &rows {
                    if *variable != name || *miss != missing {
                        continue;
                    }
                    let mut row: Vec<Cell> = lead().collect();
                    row.push(Cell::Text(name.to_string()));
                    row.push(Cell::Text(if missing { "Missing" } else { "Valid" }.into()));
                    row.push((*value).clone());
                    row.push(
                        label
                            .as_ref()
                            .map_or(Cell::Empty, |l| Cell::Text(l.clone())),
                    );
                    row.extend([
                        num(*count),
                        num(*percent),
                        if missing { Cell::Empty } else { num(*valid) },
                    ]);
                    counts.push(row);
                }
            }
        }
    }
    let lead: Vec<String> = others.to_vec();
    let mut out = Vec::new();
    let make = |title: &str, heads: &[&str], rows: Vec<Vec<Cell>>, row_headers: usize| {
        let mut columns = lead.clone();
        columns.extend(heads.iter().map(|h| h.to_string()));
        Table {
            title: title.into(),
            row_headers: lead.len() + row_headers,
            column_groups: vec![Vec::new(); columns.len()],
            columns,
            rows,
            ..Table::default()
        }
    };
    if !means.is_empty() {
        let mut heads = vec![
            "Variable",
            "Mean",
            "Std. Error",
            "df",
            "95% CI Lower",
            "95% CI Upper",
        ];
        if diagnostics {
            heads.extend([
                "Fraction Missing Info.",
                "Relative Increase Variance",
                "Relative Efficiency",
            ]);
        }
        let mut t = make("Pooled Descriptive Statistics", &heads, means, 1);
        t.caption = Some(
            "Pooled over the imputations by Rubin's rules; df is empty when the imputations \
             agree, and the interval then uses the normal distribution."
                .into(),
        );
        out.push(t);
    }
    if !counts.is_empty() {
        let heads = [
            "Variable",
            "",
            "Value",
            "Label",
            "Frequency",
            "Percent",
            "Valid Percent",
        ];
        let mut t = make("Pooled Frequencies", &heads, counts, 4);
        t.caption = Some("Frequencies and percents averaged over the imputations.".into());
        out.push(t);
    }
    out
}

#[cfg(test)]
#[test]
fn rubins_rules_on_hand_numbers() {
    // estimates 10, 12, 11 with squared standard errors 4, 4, 4: B = 1, T = 4 + 4/3
    let p = rubin(&[(10.0, 4.0), (12.0, 4.0), (11.0, 4.0), (f64::NAN, 1.0)]).unwrap();
    assert_eq!((p.estimate, p.within, p.between), (11.0, 4.0, 1.0));
    assert!((p.total - 16.0 / 3.0).abs() < 1e-12);
    // r = 1/3, df = 2 * 16 = 32, fraction (1/3 + 2/35) / (4/3)
    assert!((p.increase - 1.0 / 3.0).abs() < 1e-12);
    assert!((p.df.unwrap() - 32.0).abs() < 1e-9);
    let fraction = (1.0 / 3.0 + 2.0 / 35.0) / (4.0 / 3.0);
    assert!((p.fraction - fraction).abs() < 1e-12);
    assert!((p.efficiency - 1.0 / (1.0 + fraction / 3.0)).abs() < 1e-12);
    // equal estimates: no between variance, infinite df
    let same = rubin(&[(5.0, 1.0), (5.0, 3.0)]).unwrap();
    assert_eq!((same.df, same.total, same.efficiency), (None, 2.0, 1.0));
    assert!(rubin(&[(5.0, 1.0)]).is_none());
    assert_eq!(mioutput("[observed POOLED]"), [true, false, true, false]);
}
