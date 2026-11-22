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
use crate::dist::{normal_quantile, t_p};
use crate::matrix::Matrix;
use crate::matrix_file::MatrixRows;
use crate::means::is_valid;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Values, Variable};
use statscraft_syntax::{Correlations, CorrelationsLayout};

/// The interval of a correlation from Fisher's transformation of the Algorithms manual: with
/// zeta = atanh(r) and its standard error `se`, zeta - bias -/+ z se transformed back by tanh;
/// the one-tailed bound limits the side away from zero by the sign of r (-1 or 1 on the other).
pub(crate) fn fisher_interval(
    r: f64,
    se: f64,
    bias: f64,
    level: f64,
    one_tailed: bool,
) -> Option<(f64, f64)> {
    if !r.is_finite() || r.abs() >= 1.0 || !se.is_finite() {
        return None;
    }
    let zeta = r.atanh() - bias;
    let alpha = 1.0 - level / 100.0;
    if one_tailed {
        let z = normal_quantile(1.0 - alpha);
        Some(if r <= 0.0 {
            (-1.0, (zeta + z * se).tanh())
        } else {
            ((zeta - z * se).tanh(), 1.0)
        })
    } else {
        let z = normal_quantile(1.0 - alpha / 2.0);
        Some(((zeta - z * se).tanh(), (zeta + z * se).tanh()))
    }
}

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
        let shown = self.shown();
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
                        Cell::Text(shown.variable(v)),
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
        // what pooling over imputations needs: Fisher's z of each pair once, its variance
        let (ci_lower, ci_upper) = match &command.ci {
            Some(ci) => {
                let side = if command.one_tailed {
                    "1-tailed"
                } else {
                    "2-tailed"
                };
                (
                    format!("{}% CI Lower ({side})", ci.level),
                    format!("{}% CI Upper ({side})", ci.level),
                )
            }
            None => (String::new(), String::new()),
        };
        let mut pool = Vec::new();
        let mut pooled: Vec<(usize, usize)> = Vec::new();
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
                if command.ci.is_some() {
                    statistics.push((ci_lower.as_str(), Vec::new()));
                    statistics.push((ci_upper.as_str(), Vec::new()));
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
                    if !same && !pooled.contains(&(a.min(b), a.max(b))) {
                        pooled.push((a.min(b), a.max(b)));
                        pool.push(crate::pooling::PoolItem::Estimate {
                            title: "Pooled Correlations".into(),
                            heads: vec![String::new(); 2],
                            label: vec![
                                Cell::Text(shown.named(data, row_name)),
                                Cell::Text(shown.named(data, column_name)),
                            ],
                            kind: crate::pooling::Kind::Correlation,
                            q: p.r.atanh(),
                            u: if p.w > 3.0 {
                                1.0 / (p.w - 3.0)
                            } else {
                                f64::NAN
                            },
                        });
                    }
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
                    if let Some(ci) = &command.ci {
                        let interval = (!same)
                            .then(|| {
                                let bias = if ci.bias {
                                    r / (2.0 * (p.w - 1.0))
                                } else {
                                    0.0
                                };
                                (p.w > 3.0)
                                    .then(|| {
                                        fisher_interval(
                                            r,
                                            (1.0 / (p.w - 3.0)).sqrt(),
                                            bias,
                                            ci.level,
                                            command.one_tailed,
                                        )
                                    })
                                    .flatten()
                            })
                            .flatten();
                        values.push(number(interval.map(|i| i.0)));
                        values.push(number(interval.map(|i| i.1)));
                    }
                    for ((_, cells), value) in statistics.iter_mut().zip(values) {
                        cells.push(if hidden { Cell::Empty } else { value });
                    }
                }
                for (k, (name, cells)) in statistics.into_iter().enumerate() {
                    let mut row = vec![
                        if k == 0 {
                            Cell::Text(shown.named(data, row_name))
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
            header.extend(columns.iter().map(|n| shown.named(data, n)));
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
        let mut matrix_file = None;
        // MATRIX=OUT: the last list without WITH; the means and standard deviations come from
        // the cases of the matrix (listwise) or of each variable alone (pairwise)
        if command.matrix.output.is_some()
            && let Some((names, _)) = lists.iter().rev().find(|(_, c)| c.is_none())
        {
            let cols: Vec<usize> = names.iter().map(|n| index(n)).collect();
            let labels = cols
                .iter()
                .map(|&i| variables[i].label().map(str::to_string))
                .collect();
            let mut file = MatrixRows::new(names.clone(), labels);
            let mut means = Vec::new();
            let mut sds = Vec::new();
            for &i in &cols {
                let v = &variables[i];
                let used: &[usize] = if command.listwise { &listwise } else { &cases };
                let pairs: Vec<(f64, f64)> = used
                    .iter()
                    .filter(|&&c| valid(v, c))
                    .map(|&c| (value(v, c), weight(c)))
                    .collect();
                let s = crate::descriptives::summarize(&pairs);
                means.push(s.mean.unwrap_or(f64::NAN));
                sds.push(s.sd.unwrap_or(f64::NAN));
            }
            file.push("MEAN", "", means);
            file.push("STDDEV", "", sds);
            let pairs: Vec<Vec<_>> = cols
                .iter()
                .map(|&a| cols.iter().map(|&b| compute(a, b)).collect())
                .collect();
            if command.listwise {
                file.push("N", "", vec![listwise_n; cols.len()]);
            } else {
                let counts: Matrix = pairs
                    .iter()
                    .map(|row| row.iter().map(|p| p.w).collect())
                    .collect();
                file.counts(&counts);
            }
            let corr: Matrix = pairs
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    row.iter()
                        .enumerate()
                        .map(|(j, p)| if i == j && p.w > 0.0 { 1.0 } else { p.r })
                        .collect()
                })
                .collect();
            file.matrix("CORR", &corr);
            matrix_file = Some(self.matrix_part("CORRELATIONS", &file)?);
        }
        if let Some(items) = self.pool_items.as_mut() {
            items.extend(pool);
        }
        if let Some(part) = matrix_file {
            self.matrix_parts.1.push(part);
        }
        Ok(out)
    }
}
