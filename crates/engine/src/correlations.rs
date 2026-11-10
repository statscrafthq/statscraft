//! CORRELATIONS: Pearson correlations with their significance, case counts and optionally
//! cross-products, covariances and descriptive statistics.
//!
//! Rules from the Command Syntax Reference (CORRELATIONS) and the Algorithms manual
//! (CORRELATIONS):
//! - A list alone gives a square matrix; WITH a rectangular one, left variables in rows.
//! - PAIRWISE (default) computes each coefficient from the cases valid on both variables;
//!   LISTWISE uses the cases valid on every variable of every list and shows N once, in a
//!   caption. INCLUDE makes user-missing values valid.
//! - The Descriptive Statistics table treats missing values variable by variable whatever
//!   /MISSING says (the Command Syntax Reference; the Algorithms manual applies LISTWISE).
//! - The significance comes from t = r sqrt((W - 2) / (1 - r^2)) with W - 2 df; ONETAIL halves
//!   it. A coefficient that cannot be computed is empty.
//! - NOSIG marks coefficients significant at 0.05 with * and at 0.01 with **; a marked
//!   coefficient is shown as text with three decimals.

use crate::descriptives::{case_weights, number};
use crate::dist::t_p;
use crate::means::is_valid;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Values, Variable};
use statscraft_syntax::{Correlations, CorrelationsLayout};

/// Sums of one pair of variables over their common cases.
pub(crate) struct Pair {
    w: f64,
    cross: f64,
    pub r: f64,
}

pub(crate) fn pair(x: &[(f64, f64, f64)]) -> Pair {
    let w: f64 = x.iter().map(|p| p.2).sum();
    let mx = x.iter().map(|p| p.0 * p.2).sum::<f64>() / w;
    let my = x.iter().map(|p| p.1 * p.2).sum::<f64>() / w;
    let (mut cxx, mut cyy, mut cxy) = (0.0, 0.0, 0.0);
    for &(a, b, weight) in x {
        cxx += weight * (a - mx) * (a - mx);
        cyy += weight * (b - my) * (b - my);
        cxy += weight * (a - mx) * (b - my);
    }
    Pair {
        w,
        cross: cxy,
        r: cxy / (cxx * cyy).sqrt(),
    }
}

impl Engine {
    pub(crate) fn correlations(&mut self, command: &Correlations) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let mut lists: Vec<(Vec<String>, Option<Vec<String>>)> = Vec::new();
        for (rows, columns) in &command.lists {
            let rows = self.expand(rows)?;
            let columns = columns.as_ref().map(|c| self.expand(c)).transpose()?;
            lists.push((rows, columns));
        }
        let mut named: Vec<String> = Vec::new();
        for (rows, columns) in &lists {
            for name in rows.iter().chain(columns.iter().flatten()) {
                if !named.contains(name) {
                    named.push(name.clone());
                }
            }
        }
        let variables: Vec<Variable> = named
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = variables.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "CORRELATIONS needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let index = |name: &str| named.iter().position(|n| n == name).expect("named above");
        let weights = case_weights(data)?;
        let weight = |c: usize| weights.as_ref().map_or(1.0, |w| w[c]);
        let cases: Vec<usize> = (0..data.case_count())
            .filter(|&c| weight(c) > 0.0)
            .collect();
        let valid = |v: &Variable, c: usize| is_valid(v, c, command.include);
        let value = |v: &Variable, c: usize| match v.values() {
            Values::Numeric(values) => values.get(c).unwrap_or(f64::NAN),
            Values::String(_) => f64::NAN,
        };
        let listwise: Vec<usize> = cases
            .iter()
            .copied()
            .filter(|&c| variables.iter().all(|v| valid(v, c)))
            .collect();
        let listwise_n: f64 = listwise.iter().map(|&c| weight(c)).sum();
        let compute = |a: usize, b: usize| {
            let (x, y) = (&variables[a], &variables[b]);
            let triples: Vec<(f64, f64, f64)> = if command.listwise {
                listwise
                    .iter()
                    .map(|&c| (value(x, c), value(y, c), weight(c)))
                    .collect()
            } else {
                cases
                    .iter()
                    .filter(|&&c| valid(x, c) && valid(y, c))
                    .map(|&c| (value(x, c), value(y, c), weight(c)))
                    .collect()
            };
            pair(&triples)
        };
        let mut out = Vec::new();
        if command.descriptives {
            let rows = variables
                .iter()
                .map(|v| {
                    let pairs: Vec<(f64, f64)> = cases
                        .iter()
                        .filter(|&&c| valid(v, c))
                        .map(|&c| (value(v, c), weight(c)))
                        .collect();
                    let s = crate::descriptives::summarize(&pairs);
                    vec![
                        Cell::Text(v.name().to_string()),
                        number(s.mean),
                        number(s.sd),
                        Cell::Number(s.n),
                    ]
                })
                .collect();
            out.push(Table {
                title: "Descriptive Statistics".into(),
                row_headers: 1,
                columns: ["", "Mean", "Std. Deviation", "N"].map(String::from).into(),
                column_groups: vec![Vec::new(); 4],
                rows,
                ..Table::default()
            });
        }
        let tail = if command.one_tailed {
            "Sig. (1-tailed)"
        } else {
            "Sig. (2-tailed)"
        };
        for (rows, columns) in &lists {
            let square = columns.is_none();
            let columns = columns.as_ref().unwrap_or(rows);
            let mut table_rows = Vec::new();
            let mut flagged = false;
            for (i, row_name) in rows.iter().enumerate() {
                let a = index(row_name);
                let mut statistics: Vec<(&str, Vec<Cell>)> =
                    vec![("Pearson Correlation", Vec::new()), (tail, Vec::new())];
                if command.xprod {
                    statistics.push(("Sum of Squares and Cross-products", Vec::new()));
                    statistics.push(("Covariance", Vec::new()));
                }
                if !command.listwise {
                    statistics.push(("N", Vec::new()));
                }
                for (j, column_name) in columns.iter().enumerate() {
                    let b = index(column_name);
                    let hidden = square
                        && match command.layout {
                            CorrelationsLayout::Full => false,
                            CorrelationsLayout::Lower => j > i,
                            CorrelationsLayout::LowerNoDiagonal => j >= i,
                        };
                    let p = compute(a, b);
                    let same = a == b;
                    let r = if same && p.w > 0.0 { 1.0 } else { p.r };
                    let t = r * ((p.w - 2.0) / (1.0 - r * r)).sqrt();
                    let mut sig = t_p(t, p.w - 2.0);
                    if command.one_tailed {
                        sig /= 2.0;
                    }
                    let sig = (!same && r.is_finite()).then_some(sig);
                    let r_cell = match sig {
                        Some(s) if command.flag && s < 0.05 => {
                            flagged = true;
                            let stars = if s < 0.01 { "**" } else { "*" };
                            Cell::Text(format!("{r:.3}{stars}"))
                        }
                        _ => number(Some(r)),
                    };
                    let mut values = vec![r_cell, number(sig)];
                    if command.xprod {
                        values.push(number(Some(p.cross)));
                        values.push(number(Some(p.cross / (p.w - 1.0))));
                    }
                    if !command.listwise {
                        values.push(Cell::Number(p.w));
                    }
                    for ((_, cells), value) in statistics.iter_mut().zip(values) {
                        cells.push(if hidden { Cell::Empty } else { value });
                    }
                }
                for (k, (name, cells)) in statistics.into_iter().enumerate() {
                    let mut row = vec![
                        if k == 0 {
                            Cell::Text(row_name.clone())
                        } else {
                            Cell::Empty
                        },
                        Cell::Text(name.to_string()),
                    ];
                    row.extend(cells);
                    table_rows.push(row);
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
            let mut header = vec![String::new(), String::new()];
            header.extend(columns.iter().cloned());
            out.push(Table {
                title: "Correlations".into(),
                row_headers: 2,
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
