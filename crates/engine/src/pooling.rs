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
//! - A vector of k estimates is tested against 0 jointly (Li, Raghunathan and Rubin 1991):
//!   with U and B now matrices, r = (1 + 1/m) trace(B U^-1) / k, the total variance is taken as
//!   (1 + r) U, F = Q' ((1 + r) U)^- Q / k on k and v degrees of freedom, k the rank of U and,
//!   with t = k (m - 1), v = t (1 + 1/k) (1 + 1/r)^2 / 2 when t <= 4, else
//!   4 + (t - 4) (1 + (1 - 2/t) / r)^2.
//! - Every imputation must give the same parameters: a row or a test missing from some of them
//!   is left out with a warning. A parameter that one imputation could not estimate (NaN) is
//!   pooled from the others.
//!
//! SET MIOUTPUT chooses the parts shown: OBSERVED the original data (Imputation_ = 0),
//! IMPUTED each imputation, POOLED the pooled table and DIAGNOSTICS its diagnostic columns.

use crate::dist::{chi_square_p, f_p, normal_p, normal_quantile, t_p, t_quantile};
use crate::matrix::Matrix;
use crate::{Cell, Table};

/// How a pooled estimate is shown.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Kind {
    /// A mean: estimate, standard error, df and interval.
    Mean,
    /// A parameter tested against 0, the estimate column named `name`; `exp` adds Exp(B) and
    /// puts the interval on that scale.
    Test { name: &'static str, exp: bool },
    /// A correlation, pooled as Fisher's z = atanh(r) and shown back on the scale of r.
    Correlation,
}

/// What a procedure gives for pooling while it runs on one imputation.
#[derive(Clone)]
pub(crate) enum PoolItem {
    /// One estimate and its squared standard error: a row of the pooled table `title`, whose
    /// leading columns are `heads` and hold `label`.
    Estimate {
        title: String,
        heads: Vec<String>,
        label: Vec<Cell>,
        kind: Kind,
        q: f64,
        u: f64,
    },
    /// Estimates and their covariance matrix, tested jointly against 0: a row of `title`.
    Joint {
        title: String,
        heads: Vec<String>,
        label: Vec<Cell>,
        q: Vec<f64>,
        u: Matrix,
    },
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

/// The joint test of a vector of estimates against 0.
#[derive(Debug)]
pub(crate) struct JointTest {
    pub f: f64,
    pub df1: f64,
    /// None when the estimates do not vary between imputations: the test is then chi-square.
    pub df2: Option<f64>,
    pub p: f64,
}

/// Pools (estimates, covariance matrix) pairs into the F test that every element is 0; an
/// imputation with a value that is not finite is left out. None with fewer than two usable
/// imputations or a covariance of rank 0.
pub(crate) fn joint(sets: &[(&[f64], &Matrix)]) -> Option<JointTest> {
    let sets: Vec<&(&[f64], &Matrix)> = sets
        .iter()
        .filter(|(q, u)| q.iter().chain(u.iter().flatten()).all(|x| x.is_finite()))
        .collect();
    let k = sets.first()?.0.len();
    if sets.len() < 2 || k == 0 {
        return None;
    }
    let m = sets.len() as f64;
    let mean = |f: &dyn Fn(&[f64], &Matrix) -> f64| sets.iter().map(|s| f(s.0, s.1)).sum::<f64>();
    let q: Vec<f64> = (0..k).map(|j| mean(&|q, _| q[j]) / m).collect();
    let square = |f: &dyn Fn(usize, usize) -> f64| -> Matrix {
        (0..k).map(|i| (0..k).map(|j| f(i, j)).collect()).collect()
    };
    let within = square(&|i, j| mean(&|_, u| u[i][j]) / m);
    let between = square(&|i, j| mean(&|e, _| (e[i] - q[i]) * (e[j] - q[j])) / (m - 1.0));
    let (inverse, rank) = crate::glm::ginverse(&within, 1e-10);
    if rank == 0 {
        return None;
    }
    let df1 = rank as f64;
    let sum = |f: &dyn Fn(usize, usize) -> f64| {
        (0..k)
            .flat_map(|i| (0..k).map(move |j| (i, j)))
            .map(|(i, j)| f(i, j))
            .sum::<f64>()
    };
    let increase = (1.0 + 1.0 / m) * sum(&|i, j| between[i][j] * inverse[j][i]) / df1;
    let f = sum(&|i, j| q[i] * inverse[i][j] * q[j]) / ((1.0 + increase) * df1);
    let t = df1 * (m - 1.0);
    let df2 = (increase > 0.0).then(|| {
        if t <= 4.0 {
            t * (1.0 + 1.0 / df1) * (1.0 + 1.0 / increase).powi(2) / 2.0
        } else {
            4.0 + (t - 4.0) * (1.0 + (1.0 - 2.0 / t) / increase).powi(2)
        }
    });
    let p = match df2 {
        Some(d) => f_p(f, df1, d),
        None => chi_square_p(f * df1, df1),
    };
    Some(JointTest { f, df1, df2, p })
}

/// The table, leading columns and row label of an estimate or a joint test.
fn place(item: &PoolItem) -> Option<(&String, &Vec<String>, &Vec<Cell>)> {
    match item {
        PoolItem::Estimate {
            title,
            heads,
            label,
            ..
        }
        | PoolItem::Joint {
            title,
            heads,
            label,
            ..
        } => Some((title, heads, label)),
        PoolItem::Count { .. } => None,
    }
}

/// The columns after the row labels of a pooled table: an estimate kind, None for joint tests.
fn heads(kind: Option<Kind>, diagnostics: bool) -> Vec<&'static str> {
    let mut out = match kind {
        None => return vec!["F", "df1", "df2", "Sig."],
        Some(Kind::Mean) => vec!["Mean", "Std. Error", "df", "95% CI Lower", "95% CI Upper"],
        Some(Kind::Test { name, exp: false }) => vec![
            name,
            "Std. Error",
            "t",
            "df",
            "Sig. (2-tailed)",
            "95% CI Lower",
            "95% CI Upper",
        ],
        Some(Kind::Test { name, exp: true }) => vec![
            name,
            "Std. Error",
            "t",
            "df",
            "Sig.",
            "Exp(B)",
            "95% CI for Exp(B) Lower",
            "95% CI for Exp(B) Upper",
        ],
        Some(Kind::Correlation) => vec![
            "Correlation",
            "Sig. (2-tailed)",
            "df",
            "95% CI Lower",
            "95% CI Upper",
        ],
    };
    if diagnostics {
        out.extend([
            "Fraction Missing Info.",
            "Relative Increase Variance",
            "Relative Efficiency",
        ]);
    }
    out
}

fn caption(kind: Option<Kind>) -> &'static str {
    match kind {
        None => {
            "Tests that every parameter is 0, pooled over the imputations by the F test of Li, \
             Raghunathan and Rubin (1991); df2 is empty when the imputations agree, and the \
             significance then comes from the chi-square distribution."
        }
        Some(Kind::Correlation) => {
            "Pooled over the imputations by Rubin's rules on Fisher's z = atanh(r) with variance \
             1 / (N - 3), then turned back into correlations; df, significance and interval come \
             from z, and df is empty when the imputations agree."
        }
        Some(_) => {
            "Pooled over the imputations by Rubin's rules; df is empty when the imputations \
             agree, and the interval then uses the normal distribution."
        }
    }
}

/// The cells of a pooled estimate after its label.
fn estimate_cells(kind: Kind, p: &Pooled, diagnostics: bool) -> Vec<Cell> {
    let se = p.total.sqrt();
    let half = se
        * match p.df {
            Some(df) => t_quantile(0.025, df),
            None => -normal_quantile(0.025),
        };
    let (low, high) = (p.estimate - half, p.estimate + half);
    let t = p.estimate / se;
    let sig = num(match p.df {
        Some(df) => t_p(t, df),
        None => normal_p(t),
    });
    let df = p.df.map_or(Cell::Empty, num);
    let mut row = match kind {
        Kind::Mean => vec![num(p.estimate), num(se), df, num(low), num(high)],
        Kind::Test { exp: false, .. } => {
            vec![
                num(p.estimate),
                num(se),
                num(t),
                df,
                sig,
                num(low),
                num(high),
            ]
        }
        Kind::Test { exp: true, .. } => vec![
            num(p.estimate),
            num(se),
            num(t),
            df,
            sig,
            num(p.estimate.exp()),
            num(low.exp()),
            num(high.exp()),
        ],
        Kind::Correlation => vec![
            num(p.estimate.tanh()),
            sig,
            df,
            num(low.tanh()),
            num(high.tanh()),
        ],
    };
    if diagnostics {
        row.extend([num(p.fraction), num(p.increase), num(p.efficiency)]);
    }
    row
}

fn label_text(label: &[Cell]) -> String {
    label
        .iter()
        .filter_map(|c| match c {
            Cell::Text(t) => Some(t.clone()),
            Cell::Number(n) => Some(n.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A pooled table under construction: title, label heads, kind (None: joint tests) and rows.
type Built = (String, Vec<String>, Option<Kind>, Vec<Vec<Cell>>);

/// The pooled tables of the imputations (Imputation_ 1 and up), with warnings about what could
/// not be pooled. `others` names the split variables after Imputation_.
pub(crate) fn tables(
    groups: &[Imputed],
    others: &[String],
    diagnostics: bool,
) -> (Vec<Table>, Vec<String>) {
    // the combinations of the other split variables, in order of appearance
    let mut keys: Vec<&Vec<String>> = Vec::new();
    for g in groups {
        if !keys.contains(&&g.others) {
            keys.push(&g.others);
        }
    }
    let mut built: Vec<Built> = Vec::new();
    let mut warnings = Vec::new();
    let mut counts: Vec<Vec<Cell>> = Vec::new();
    for key in keys {
        let mine: Vec<&Imputed> = groups.iter().filter(|g| &g.others == key).collect();
        let lead = || key.iter().map(|t| Cell::Text(t.clone()));
        let within = match key.is_empty() {
            true => String::new(),
            false => format!(" ({})", key.join(", ")),
        };
        // estimates and joint tests, row by row in order of appearance
        let mut seen: Vec<(&String, &Vec<String>, &Vec<Cell>)> = Vec::new();
        for item in mine.iter().flat_map(|g| &g.items) {
            if let Some(at) = place(item)
                && !seen.contains(&at)
            {
                seen.push(at);
            }
        }
        for at in seen {
            let found: Vec<&PoolItem> = mine
                .iter()
                .filter_map(|g| g.items.iter().find(|i| place(i) == Some(at)))
                .collect();
            let (title, heads, label) = at;
            let mut row: Vec<Cell> = lead().chain(label.iter().cloned()).collect();
            let shapes = |i: &&PoolItem| match i {
                PoolItem::Joint { q, .. } => q.len(),
                _ => 0,
            };
            if found.len() < mine.len() || found.iter().any(|i| shapes(i) != shapes(&found[0])) {
                warnings.push(format!(
                    "{title}: {}{within} is not pooled because the imputations do not all give it",
                    label_text(label)
                ));
                continue;
            }
            let kind = match found[0] {
                PoolItem::Estimate { kind, .. } => {
                    let pairs: Vec<(f64, f64)> = found
                        .iter()
                        .filter_map(|i| match i {
                            PoolItem::Estimate { q, u, .. } => Some((*q, *u)),
                            _ => None,
                        })
                        .collect();
                    let Some(p) = rubin(&pairs) else { continue };
                    row.extend(estimate_cells(*kind, &p, diagnostics));
                    Some(*kind)
                }
                _ => {
                    let sets: Vec<(&[f64], &Matrix)> = found
                        .iter()
                        .filter_map(|i| match i {
                            PoolItem::Joint { q, u, .. } => Some((q.as_slice(), u)),
                            _ => None,
                        })
                        .collect();
                    let Some(test) = joint(&sets) else { continue };
                    row.extend([
                        num(test.f),
                        num(test.df1),
                        test.df2.map_or(Cell::Empty, num),
                        num(test.p),
                    ]);
                    None
                }
            };
            match built.iter_mut().find(|b| &b.0 == title) {
                Some(b) => b.3.push(row),
                None => built.push((title.clone(), heads.clone(), kind, vec![row])),
            }
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
    let mut out = Vec::new();
    let make = |title: &str, heads: Vec<String>, rows: Vec<Vec<Cell>>, row_headers: usize| {
        let mut columns = others.to_vec();
        columns.extend(heads);
        Table {
            title: title.into(),
            row_headers: others.len() + row_headers,
            column_groups: vec![Vec::new(); columns.len()],
            columns,
            rows,
            ..Table::default()
        }
    };
    for (title, labels, kind, rows) in built {
        let size = labels.len();
        let mut columns = labels;
        let shown = diagnostics && kind.is_some();
        columns.extend(heads(kind, shown).into_iter().map(String::from));
        let mut t = make(&title, columns, rows, size);
        t.caption = Some(caption(kind).into());
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
        let heads = heads.iter().map(|h| h.to_string()).collect();
        let mut t = make("Pooled Frequencies", heads, counts, 4);
        t.caption = Some("Frequencies and percents averaged over the imputations.".into());
        out.push(t);
    }
    (out, warnings)
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

#[cfg(test)]
#[test]
fn joint_test_on_hand_numbers() {
    // one parameter, six imputations: B = 0.8, U = 1, r = 7/6 * 0.8, t = 5 takes the long df
    let one = vec![vec![1.0]];
    let estimates = [[1.0], [2.0], [3.0], [2.0], [1.0], [3.0]];
    let sets: Vec<(&[f64], &Matrix)> = estimates.iter().map(|q| (&q[..], &one)).collect();
    let test = joint(&sets).unwrap();
    let r = 7.0 / 6.0 * 0.8;
    assert!((test.f - 4.0 / (1.0 + r)).abs() < 1e-12);
    let df2 = 4.0 + (1.0 + 0.6 / r).powi(2);
    assert!((test.df2.unwrap() - df2).abs() < 1e-12);
    // a general statistics package's F distribution at these numbers
    assert!((test.p - 0.195354572060394).abs() < 1e-12);
    // two parameters that agree in both imputations: chi-square on 2 df of 2 F
    let identity = vec![vec![1.0, 0.0], vec![0.0, 1.0]];
    let q = [1.0, 0.5];
    let test = joint(&[(&q[..], &identity), (&q[..], &identity)]).unwrap();
    assert_eq!((test.f, test.df1, test.df2), (0.625, 2.0, None));
    assert!((test.p - (-0.625_f64).exp()).abs() < 1e-12);
    assert!(joint(&[(&q[..], &identity)]).is_none());
}
