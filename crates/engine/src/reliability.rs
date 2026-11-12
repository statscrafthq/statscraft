//! RELIABILITY: the reliability of an additive scale of items.
//!
//! Rules from the Command Syntax Reference (RELIABILITY) and the Algorithms manual
//! (RELIABILITY):
//! - A case missing on any variable of VARIABLES is left out (MISSING=INCLUDE makes user-missing
//!   values valid). W is the sum of the case weights; variances and covariances divide by W - 1.
//! - Method 2 (the covariance matrix) is used when the model is GUTTMAN, PARALLEL or
//!   STRICTPARALLEL, when CORR, COV, TUKEY or HOTELLING is asked for, when SUMMARY asks for more
//!   than TOTAL, or with METHOD=COVARIANCE. It drops items without variance, with a warning;
//!   method 1 keeps them.
//! - Alpha = k / (k - 1) (1 - sum of item variances / scale variance); the standardized alpha
//!   uses the mean inter-item correlation. SPLIT puts n items (default k / 2, the odd one in the
//!   first part) in the second part.
//! - The ANOVA table, Friedman's and Cochran's chi-square (SS between measures over the mean
//!   square within people, from the values as they are), Kendall's W, Tukey's test for
//!   nonadditivity, Hotelling's T-squared and the intraclass correlations follow the formulas of
//!   the Algorithms manual.
//! - PARALLEL and STRICTPARALLEL: the goodness-of-fit test uses the determinant of the
//!   constrained matrix, as the manual's text describes (its likelihood ratio formula prints
//!   k + 1 and 1 / k where that matrix has k - 1 and 1 / (k - 1)).

use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, f_p, f_quantile};
use crate::matrix::{Matrix, inverse};
use crate::means::is_valid;
use crate::npar_tests::text;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Values, Variable};
use statscraft_syntax::{AnovaTest, Icc, IccModel, Reliability, ReliabilityModel, VarItem};

fn plain_table(title: &str, row_headers: usize, columns: &[&str], rows: Vec<Vec<Cell>>) -> Table {
    Table {
        title: title.into(),
        row_headers,
        columns: columns.iter().map(|c| c.to_string()).collect(),
        column_groups: vec![Vec::new(); columns.len()],
        rows,
        ..Table::default()
    }
}

/// Mean, minimum, maximum, range, maximum / minimum and variance of a list of values, the
/// variance divided by n - 1.
fn summary(values: &[f64]) -> [f64; 6] {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    [mean, min, max, max - min, max / min, variance]
}

/// The statistics of the items of one scale.
struct Scale {
    names: Vec<String>,
    /// The items' values in the cases of the analysis, by case, with the case weights.
    rows: Vec<(Vec<f64>, f64)>,
    w: f64,
    mean: Vec<f64>,
    v: Matrix,
}

impl Scale {
    fn new(names: Vec<String>, rows: Vec<(Vec<f64>, f64)>) -> Self {
        let k = names.len();
        let w: f64 = rows.iter().map(|r| r.1).sum();
        let mean: Vec<f64> = (0..k)
            .map(|i| rows.iter().map(|r| r.0[i] * r.1).sum::<f64>() / w)
            .collect();
        let v: Matrix = (0..k)
            .map(|i| {
                (0..k)
                    .map(|j| {
                        rows.iter()
                            .map(|r| r.1 * (r.0[i] - mean[i]) * (r.0[j] - mean[j]))
                            .sum::<f64>()
                            / (w - 1.0)
                    })
                    .collect()
            })
            .collect();
        Scale {
            names,
            rows,
            w,
            mean,
            v,
        }
    }

    fn k(&self) -> usize {
        self.names.len()
    }

    /// The variance of the sum of the items in `range`.
    fn variance(&self, items: std::ops::Range<usize>) -> f64 {
        items
            .clone()
            .map(|i| items.clone().map(|j| self.v[i][j]).sum::<f64>())
            .sum()
    }

    fn alpha(&self, items: std::ops::Range<usize>) -> f64 {
        let k = items.len() as f64;
        let sum: f64 = items.clone().map(|i| self.v[i][i]).sum();
        k / (k - 1.0) * (1.0 - sum / self.variance(items))
    }

    fn corr(&self, i: usize, j: usize) -> f64 {
        self.v[i][j] / (self.v[i][i] * self.v[j][j]).sqrt()
    }

    /// The off-diagonal entries of a matrix, every ordered pair.
    fn off_diagonal(&self, f: impl Fn(usize, usize) -> f64) -> Vec<f64> {
        let k = self.k();
        (0..k)
            .flat_map(|i| (0..k).filter(move |&j| j != i).map(move |j| (i, j)))
            .map(|(i, j)| f(i, j))
            .collect()
    }

    fn standardized_alpha(&self) -> f64 {
        let k = self.k() as f64;
        let r = self.off_diagonal(|i, j| self.corr(i, j));
        let mean = r.iter().sum::<f64>() / r.len() as f64;
        k * mean / (1.0 + (k - 1.0) * mean)
    }

    /// The error variance of every item: one over the diagonal of V^-1, None when V is singular.
    fn errors(&self) -> Option<Vec<f64>> {
        let (inv, _) = inverse(&self.v)?;
        Some((0..self.k()).map(|i| 1.0 / inv[i][i]).collect())
    }

    /// The sums of squares of the ANOVA table: between people, between measures, residual.
    fn anova(&self) -> (f64, f64, f64) {
        let k = self.k() as f64;
        let w = self.w;
        let sum_var: f64 = (0..self.k()).map(|i| self.v[i][i]).sum();
        let total_var = self.variance(0..self.k());
        let grand = self.mean.iter().sum::<f64>() / k;
        let between_people = (w - 1.0) * total_var / k;
        let between_measures = w * self.mean.iter().map(|m| (m - grand).powi(2)).sum::<f64>();
        let residual = (w - 1.0) * (sum_var - total_var / k);
        (between_people, between_measures, residual)
    }
}

impl Engine {
    pub(crate) fn reliability(&mut self, command: &Reliability) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let all = self.expand(&command.variables)?;
        let (label, items) = match &command.scale {
            None => ("ALL".to_string(), all.clone()),
            Some((label, list)) if list == &[VarItem::All] => (label.clone(), all.clone()),
            Some((label, list)) => {
                let items = self.expand(list)?;
                if let Some(name) = items.iter().find(|n| !all.contains(n)) {
                    return Err(Error(format!(
                        "{name} is on SCALE but not on VARIABLES of RELIABILITY"
                    )));
                }
                (label.clone(), items)
            }
        };
        let mut unique: Vec<String> = Vec::new();
        for name in items {
            if !unique.contains(&name) {
                unique.push(name);
            }
        }
        let variables: Vec<Variable> = all
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = variables.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "RELIABILITY needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        if unique.len() < 2 {
            return Err(Error(
                "RELIABILITY needs at least two items in the scale".into(),
            ));
        }
        let weights = case_weights(data)?;
        let value = |v: &Variable, k: usize| match v.values() {
            Values::Numeric(values) => values.get(k).unwrap_or(f64::NAN),
            Values::String(_) => f64::NAN,
        };
        let positions: Vec<usize> = unique
            .iter()
            .map(|n| all.iter().position(|a| a == n).expect("checked"))
            .collect();
        let (mut total, mut rows) = (0.0, Vec::new());
        for k in 0..data.case_count() {
            let w = weights.as_ref().map_or(1.0, |w| w[k]);
            if w <= 0.0 {
                continue;
            }
            total += w;
            if variables.iter().all(|v| is_valid(v, k, command.include)) {
                let values = positions.iter().map(|&i| value(&variables[i], k)).collect();
                rows.push((values, w));
            }
        }
        let valid: f64 = rows.iter().map(|r| r.1).sum();
        let percent = |n: f64| number(Some(100.0 * n / total));
        let mut processing = plain_table(
            "Case Processing Summary",
            1,
            &["Cases", "N", "Percent"],
            vec![
                vec![text("Valid"), Cell::Number(valid), percent(valid)],
                vec![
                    text("Excluded"),
                    Cell::Number(total - valid),
                    percent(total - valid),
                ],
                vec![text("Total"), Cell::Number(total), percent(total)],
            ],
        );
        processing.caption = Some(format!(
            "Scale: {label}. Listwise deletion based on all variables in the procedure."
        ));
        let mut tables = vec![processing];
        if valid <= 1.0 {
            self.warn(format!(
                "RELIABILITY: too few cases (N = {valid}) for the scale {label}"
            ));
            return Ok(tables);
        }
        // the items as tables show them (SET TVARS)
        let shown = self.shown();
        let texts = positions
            .iter()
            .map(|&i| shown.variable(&variables[i]))
            .collect();
        let mut scale = Scale::new(texts, rows);
        let s = &command.summary;
        let method2 = !matches!(
            command.model,
            ReliabilityModel::Alpha | ReliabilityModel::Split(_)
        ) || command.correlations
            || command.covariances
            || command.tukey
            || command.hotelling
            || s.means
            || s.variances
            || s.covariances
            || s.correlations
            || command.covariance_method;
        if method2 {
            let constant: Vec<usize> = (0..scale.k())
                .filter(|&i| scale.v[i][i].is_nan() || scale.v[i][i] <= 0.0)
                .collect();
            if !constant.is_empty() {
                let names: Vec<&str> = constant.iter().map(|&i| scale.names[i].as_str()).collect();
                self.warn(format!(
                    "RELIABILITY: {} {} no variance and {} dropped from the scale {label}",
                    names.join(", "),
                    if names.len() == 1 { "has" } else { "have" },
                    if names.len() == 1 { "is" } else { "are" },
                ));
                let keep: Vec<usize> = (0..scale.k()).filter(|i| !constant.contains(i)).collect();
                let names = keep.iter().map(|&i| scale.names[i].clone()).collect();
                let rows = scale
                    .rows
                    .iter()
                    .map(|(v, w)| (keep.iter().map(|&i| v[i]).collect(), *w))
                    .collect();
                scale = Scale::new(names, rows);
                if scale.k() < 2 {
                    self.warn(format!(
                        "RELIABILITY: fewer than two items with variance remain in the scale {label}"
                    ));
                    return Ok(tables);
                }
            }
        }
        let k = scale.k();
        let kf = k as f64;
        let w = scale.w;
        let total_var = scale.variance(0..k);
        let sum_var: f64 = (0..k).map(|i| scale.v[i][i]).sum();
        // the parts of SPLIT, and of Guttman's lambda 4
        let second = match command.model {
            ReliabilityModel::Split(Some(n)) => n as usize,
            _ => k / 2,
        };
        if second == 0 || second >= k {
            return Err(Error(format!(
                "SPLIT({second}) must leave at least one of the {k} items in each part"
            )));
        }
        let first = k - second;
        let (part1, part2) = (0..first, first..k);
        let (var1, var2) = (scale.variance(part1.clone()), scale.variance(part2.clone()));
        let guttman_split = 2.0 * (total_var - var1 - var2) / total_var;
        let errors = scale.errors();
        let mut warnings = Vec::new();

        let negative = |alpha: f64| {
            (alpha < 0.0).then(|| {
                "The value is negative because the average covariance among the items is \
                 negative, which violates the assumptions of the reliability model."
                    .to_string()
            })
        };
        let scale_caption = format!("Scale: {label}.");
        match command.model {
            ReliabilityModel::Alpha => {
                let alpha = scale.alpha(0..k);
                let mut t = plain_table(
                    "Reliability Statistics",
                    0,
                    &[
                        "Cronbach's Alpha",
                        "Cronbach's Alpha Based on Standardized Items",
                        "N of Items",
                    ],
                    vec![vec![
                        number(Some(alpha)),
                        number(Some(scale.standardized_alpha())),
                        Cell::Number(kf),
                    ]],
                );
                t.caption = Some(match negative(alpha) {
                    Some(note) => format!("{scale_caption} {note}"),
                    None => scale_caption.clone(),
                });
                tables.push(t);
            }
            ReliabilityModel::Split(_) => {
                let r = (total_var - var1 - var2) / (2.0 * (var1 * var2).sqrt());
                let alpha =
                    |part: std::ops::Range<usize>| (part.len() > 1).then(|| scale.alpha(part));
                let (k1, k2) = (first as f64, second as f64);
                let f = (1.0 - r * r) * k1 * k2 / (kf * kf);
                let unequal = (-r * r + (r.powi(4) + 4.0 * r * r * f).sqrt()) / (2.0 * f);
                let rows = vec![
                    vec![
                        text("Cronbach's Alpha"),
                        text("Part 1"),
                        text("Value"),
                        number(alpha(part1.clone())),
                    ],
                    vec![
                        Cell::Empty,
                        Cell::Empty,
                        text("N of Items"),
                        Cell::Number(k1),
                    ],
                    vec![
                        Cell::Empty,
                        text("Part 2"),
                        text("Value"),
                        number(alpha(part2.clone())),
                    ],
                    vec![
                        Cell::Empty,
                        Cell::Empty,
                        text("N of Items"),
                        Cell::Number(k2),
                    ],
                    vec![
                        Cell::Empty,
                        text("Total N of Items"),
                        Cell::Empty,
                        Cell::Number(kf),
                    ],
                    vec![
                        text("Correlation Between Forms"),
                        Cell::Empty,
                        Cell::Empty,
                        number(Some(r)),
                    ],
                    vec![
                        text("Spearman-Brown Coefficient"),
                        text("Equal Length"),
                        Cell::Empty,
                        number(Some(2.0 * r / (1.0 + r))),
                    ],
                    vec![
                        Cell::Empty,
                        text("Unequal Length"),
                        Cell::Empty,
                        number(Some(unequal)),
                    ],
                    vec![
                        text("Guttman Split-Half Coefficient"),
                        Cell::Empty,
                        Cell::Empty,
                        number(Some(guttman_split)),
                    ],
                ];
                let mut t = plain_table("Reliability Statistics", 3, &["", "", "", "Value"], rows);
                let list = |part: std::ops::Range<usize>| scale.names[part].join(", ");
                t.caption = Some(format!(
                    "{scale_caption} Part 1: {}. Part 2: {}.",
                    list(part1.clone()),
                    list(part2.clone())
                ));
                tables.push(t);
            }
            ReliabilityModel::Guttman => {
                let l1 = 1.0 - sum_var / total_var;
                let off2: f64 = scale
                    .off_diagonal(|i, j| scale.v[i][j].powi(2))
                    .iter()
                    .sum();
                let l2 = l1 + (kf / (kf - 1.0) * off2).sqrt() / total_var;
                let l3 = kf / (kf - 1.0) * l1;
                let l5_max = (0..k)
                    .map(|i| {
                        (0..k)
                            .filter(|&j| j != i)
                            .map(|j| scale.v[i][j].powi(2))
                            .sum::<f64>()
                    })
                    .fold(0.0, f64::max);
                let l5 = l1 + 2.0 * l5_max.sqrt() / total_var;
                let l6 = errors
                    .as_ref()
                    .map(|e| 1.0 - e.iter().sum::<f64>() / total_var);
                let mut rows: Vec<Vec<Cell>> = [
                    Some(l1),
                    Some(l2),
                    Some(l3),
                    Some(guttman_split),
                    Some(l5),
                    l6,
                ]
                .iter()
                .enumerate()
                .map(|(i, v)| vec![text("Lambda"), Cell::Number((i + 1) as f64), number(*v)])
                .collect();
                rows.push(vec![text("N of Items"), Cell::Empty, Cell::Number(kf)]);
                let mut t = plain_table("Reliability Statistics", 2, &["", "", "Value"], rows);
                t.caption = Some(scale_caption.clone());
                tables.push(t);
            }
            ReliabilityModel::Parallel | ReliabilityModel::StrictParallel => {
                let strict = command.model == ReliabilityModel::StrictParallel;
                let var_mean = sum_var / kf;
                let cov_mean = (total_var - sum_var) / (kf * (kf - 1.0));
                let grand = scale.mean.iter().sum::<f64>() / kf;
                let spread: f64 = scale.mean.iter().map(|m| (m - grand).powi(2)).sum();
                let det = inverse(&scale.v).map(|(_, d)| d);
                let mut rows = Vec::new();
                let (log_c, df, statistics) = if strict {
                    let (_, between_measures, residual) = scale.anova();
                    let ms_within = (between_measures + residual) / (w * (kf - 1.0));
                    let common = var_mean + spread / kf;
                    let error = ms_within;
                    let true_var = common - error;
                    let r = (cov_mean - spread / ((kf - 1.0) * kf)) / (var_mean + spread / kf);
                    let rel = kf * r / (1.0 + (kf - 1.0) * r);
                    let log_c = ((var_mean + (kf - 1.0) * cov_mean)
                        * (var_mean - cov_mean + spread / (kf - 1.0)).powf(kf - 1.0))
                    .ln();
                    (
                        log_c,
                        kf * (kf + 3.0) / 2.0 - 3.0,
                        vec![
                            ("Common Mean", grand),
                            ("Common Variance", common),
                            ("True Variance", true_var),
                            ("Error Variance", error),
                            ("Common Inter-Item Correlation", r),
                            ("Reliability of Scale", rel),
                            (
                                "Reliability of Scale (Unbiased)",
                                (3.0 + (w - 3.0) * rel) / w,
                            ),
                        ],
                    )
                } else {
                    let alpha = scale.alpha(0..k);
                    let log_c = ((var_mean - cov_mean).powf(kf - 1.0)
                        * (var_mean + (kf - 1.0) * cov_mean))
                        .ln();
                    (
                        log_c,
                        kf * (kf + 1.0) / 2.0 - 2.0,
                        vec![
                            ("Common Variance", var_mean),
                            ("True Variance", cov_mean),
                            ("Error Variance", var_mean - cov_mean),
                            ("Common Inter-Item Correlation", cov_mean / var_mean),
                            ("Reliability of Scale", alpha),
                            (
                                "Reliability of Scale (Unbiased)",
                                (2.0 + (w - 3.0) * alpha) / (w - 1.0),
                            ),
                        ],
                    )
                };
                match det.filter(|d| *d > 0.0) {
                    Some(det) => {
                        let log_uc = det.ln();
                        let middle = if strict {
                            kf * (kf + 3.0) / 2.0 - 3.0
                        } else {
                            kf * (kf + 1.0) / 2.0 - 2.0
                        };
                        let factor = 1.0
                            - kf * (kf + 1.0).powi(2) * (2.0 * kf - 3.0)
                                / (12.0 * (kf - 1.0) * middle * (w - 1.0));
                        let chi = -(w - 1.0) * factor * (log_uc - log_c);
                        rows.push(vec![text("Chi-Square"), text("Value"), number(Some(chi))]);
                        rows.push(vec![Cell::Empty, text("df"), number(Some(df))]);
                        rows.push(vec![Cell::Empty, text("Sig."), number(Some(chi_square_p(chi, df)))]);
                        rows.push(vec![
                            text("Log of Determinant of"),
                            text("Unconstrained Matrix"),
                            number(Some(log_uc)),
                        ]);
                        rows.push(vec![Cell::Empty, text("Constrained Matrix"), number(Some(log_c))]);
                        let mut t = plain_table("Test for Goodness of Fit", 2, &["", "", "Value"], rows);
                        t.caption = Some(format!(
                            "Under the {} model assumption. {scale_caption}",
                            if strict { "strictly parallel" } else { "parallel" }
                        ));
                        tables.push(t);
                    }
                    None => warnings.push(
                        "the covariance matrix of the items is singular, so the goodness-of-fit test is left out"
                            .to_string(),
                    ),
                }
                let rows = statistics
                    .iter()
                    .map(|(name, v)| vec![text(*name), number(Some(*v))])
                    .collect();
                let mut t = plain_table("Reliability Statistics", 1, &["", "Value"], rows);
                t.caption = Some(scale_caption.clone());
                tables.push(t);
            }
        }

        if command.descriptive {
            let rows = (0..k)
                .map(|i| {
                    vec![
                        Cell::Text(scale.names[i].clone()),
                        number(Some(scale.mean[i])),
                        number(Some(scale.v[i][i].sqrt())),
                        Cell::Number(w),
                    ]
                })
                .collect();
            tables.push(plain_table(
                "Item Statistics",
                1,
                &["", "Mean", "Std. Deviation", "N"],
                rows,
            ));
        }
        let mut header = vec![String::new()];
        header.extend(scale.names.iter().cloned());
        let header: Vec<&str> = header.iter().map(String::as_str).collect();
        for (wanted, title, correlation) in [
            (command.correlations, "Inter-Item Correlation Matrix", true),
            (command.covariances, "Inter-Item Covariance Matrix", false),
        ] {
            if !wanted {
                continue;
            }
            let rows = (0..k)
                .map(|i| {
                    let mut row = vec![Cell::Text(scale.names[i].clone())];
                    row.extend((0..k).map(|j| {
                        number(Some(if correlation {
                            scale.corr(i, j)
                        } else {
                            scale.v[i][j]
                        }))
                    }));
                    row
                })
                .collect();
            tables.push(plain_table(title, 1, &header, rows));
        }
        if s.means || s.variances || s.covariances || s.correlations {
            let split = matches!(command.model, ReliabilityModel::Split(_));
            let parts: Vec<(&str, std::ops::Range<usize>)> = if split {
                vec![
                    ("Part 1", part1.clone()),
                    ("Part 2", part2.clone()),
                    ("Both Parts", 0..k),
                ]
            } else {
                vec![("", 0..k)]
            };
            let mut rows = Vec::new();
            for (wanted, name, kind) in [
                (s.means, "Item Means", 0),
                (s.variances, "Item Variances", 1),
                (s.covariances, "Inter-Item Covariances", 2),
                (s.correlations, "Inter-Item Correlations", 3),
            ] {
                if !wanted {
                    continue;
                }
                for (p, (part, range)) in parts.iter().enumerate() {
                    let pairs = || {
                        range.clone().flat_map(|i| {
                            range.clone().filter(move |&j| j != i).map(move |j| (i, j))
                        })
                    };
                    let values: Vec<f64> = match kind {
                        0 => range.clone().map(|i| scale.mean[i]).collect(),
                        1 => range.clone().map(|i| scale.v[i][i]).collect(),
                        2 => pairs().map(|(i, j)| scale.v[i][j]).collect(),
                        _ => pairs().map(|(i, j)| scale.corr(i, j)).collect(),
                    };
                    let mut row = vec![if p == 0 { text(name) } else { Cell::Empty }, text(*part)];
                    if values.is_empty() {
                        row.extend((0..6).map(|_| Cell::Empty));
                    } else {
                        row.extend(summary(&values).iter().map(|v| number(Some(*v))));
                    }
                    row.push(Cell::Number(range.len() as f64));
                    rows.push(row);
                }
            }
            let mut t = plain_table(
                "Summary Item Statistics",
                2,
                &[
                    "",
                    "",
                    "Mean",
                    "Minimum",
                    "Maximum",
                    "Range",
                    "Maximum / Minimum",
                    "Variance",
                    "N of Items",
                ],
                rows,
            );
            if !split {
                t.row_headers = 1;
                t.columns.remove(1);
                t.column_groups.remove(1);
                for row in &mut t.rows {
                    row.remove(1);
                }
            }
            tables.push(t);
        }
        if s.total {
            let scale_mean: f64 = scale.mean.iter().sum();
            let mut columns = vec![
                "",
                "Scale Mean if Item Deleted",
                "Scale Variance if Item Deleted",
                "Corrected Item-Total Correlation",
            ];
            let smc = method2.then_some(errors.as_ref()).flatten();
            if smc.is_some() {
                columns.push("Squared Multiple Correlation");
            }
            columns.push("Cronbach's Alpha if Item Deleted");
            let rows = (0..k)
                .map(|i| {
                    let cov_ip: f64 = (0..k).map(|j| scale.v[i][j]).sum();
                    let si2 = scale.v[i][i];
                    let deleted = total_var + si2 - 2.0 * cov_ip;
                    let r = (cov_ip - si2) / (si2 * deleted).sqrt();
                    let alpha = (k > 2)
                        .then(|| (kf - 1.0) / (kf - 2.0) * (1.0 - (sum_var - si2) / deleted));
                    let mut row = vec![
                        Cell::Text(scale.names[i].clone()),
                        number(Some(scale_mean - scale.mean[i])),
                        number(Some(deleted)),
                        number(Some(r)),
                    ];
                    if let Some(e) = smc {
                        row.push(number(Some(1.0 - e[i] / si2)));
                    }
                    row.push(number(alpha));
                    row
                })
                .collect();
            tables.push(plain_table("Item-Total Statistics", 1, &columns, rows));
        }
        if command.scale_statistics {
            let row = |name: &str, var: f64, mean: f64, n: usize| {
                vec![
                    text(name),
                    number(Some(mean)),
                    number(Some(var)),
                    number(Some(var.sqrt())),
                    Cell::Number(n as f64),
                ]
            };
            let mean_of = |range: std::ops::Range<usize>| range.map(|i| scale.mean[i]).sum::<f64>();
            let rows = if matches!(command.model, ReliabilityModel::Split(_)) {
                vec![
                    row("Part 1", var1, mean_of(part1.clone()), first),
                    row("Part 2", var2, mean_of(part2.clone()), second),
                    row("Both Parts", total_var, mean_of(0..k), k),
                ]
            } else {
                vec![row("", total_var, mean_of(0..k), k)]
            };
            let mut t = plain_table(
                "Scale Statistics",
                1,
                &["", "Mean", "Variance", "Std. Deviation", "N of Items"],
                rows,
            );
            if !matches!(command.model, ReliabilityModel::Split(_)) {
                t.row_headers = 0;
                t.columns.remove(0);
                t.column_groups.remove(0);
                t.rows[0].remove(0);
            }
            tables.push(t);
        }
        let (ss_people, ss_measures, ss_residual) = scale.anova();
        let df_people = w - 1.0;
        let df_measures = kf - 1.0;
        let df_residual = (w - 1.0) * (kf - 1.0);
        let ss_within = ss_measures + ss_residual;
        let df_within = w * (kf - 1.0);
        let grand = scale.mean.iter().sum::<f64>() / kf;
        if command.anova.is_some() || command.tukey {
            tables.push(anova_table(
                &scale,
                command.anova.unwrap_or(AnovaTest::F),
                command.tukey,
                [ss_people, ss_measures, ss_residual],
                grand,
            ));
        }
        if command.hotelling {
            let last = k - 1;
            let b: Matrix = (0..last)
                .map(|i| {
                    (0..last)
                        .map(|j| {
                            scale.v[i][j] - scale.v[i][last] - scale.v[j][last]
                                + scale.v[last][last]
                        })
                        .collect()
                })
                .collect();
            let y: Vec<f64> = (0..last)
                .map(|i| scale.mean[i] - scale.mean[last])
                .collect();
            match inverse(&b).filter(|(_, det)| det.abs() >= 1e-30 && w >= kf) {
                Some((inv, _)) => {
                    let quad: f64 = (0..last)
                        .map(|i| (0..last).map(|j| y[i] * inv[i][j] * y[j]).sum::<f64>())
                        .sum();
                    let t2 = w * quad;
                    let (d1, d2) = (kf - 1.0, w - kf + 1.0);
                    let f = (w - kf + 1.0) / ((w - 1.0) * (kf - 1.0)) * t2;
                    let mut t = plain_table(
                        "Hotelling's T-Squared Test",
                        0,
                        &["Hotelling's T-Squared", "F", "df1", "df2", "Sig."],
                        vec![vec![
                            number(Some(t2)),
                            number(Some(f)),
                            number(Some(d1)),
                            number(Some(d2)),
                            number(Some(f_p(f, d1, d2))),
                        ]],
                    );
                    t.caption = Some(format!("Grand Mean = {grand}."));
                    tables.push(t);
                }
                None => warnings.push(
                    "Hotelling's T-squared needs more cases than items and a nonsingular covariance matrix of the differences; it is left out"
                        .to_string(),
                ),
            }
        }
        if let Some(icc) = &command.icc {
            tables.push(icc_table(
                icc,
                kf,
                w,
                [
                    ss_people / df_people,
                    ss_measures / df_measures,
                    ss_residual / df_residual,
                    ss_within / df_within,
                ],
            ));
        }
        for warning in warnings {
            self.warn(format!("RELIABILITY: {warning}"));
        }
        Ok(tables)
    }
}

/// The repeated measures ANOVA table, with Friedman's or Cochran's test and Tukey's test for
/// nonadditivity when asked for.
fn anova_table(
    scale: &Scale,
    test: AnovaTest,
    tukey: bool,
    [ss_people, ss_measures, ss_residual]: [f64; 3],
    grand: f64,
) -> Table {
    let k = scale.k() as f64;
    let w = scale.w;
    let df_people = w - 1.0;
    let df_measures = k - 1.0;
    let df_residual = (w - 1.0) * (k - 1.0);
    let ss_within = ss_measures + ss_residual;
    let df_within = w * (k - 1.0);
    let ms_within = ss_within / df_within;
    let ms_residual = ss_residual / df_residual;
    let mut captions = vec![format!("Grand Mean = {grand}.")];
    // (label, label, SS, df, the rank test with its significance, F with its significance)
    type Row = (
        &'static str,
        &'static str,
        f64,
        f64,
        Option<(f64, f64)>,
        Option<(f64, f64)>,
    );
    let mut rows: Vec<Row> = Vec::new();
    rows.push(("Between People", "", ss_people, df_people, None, None));
    let measures_f = ss_measures / df_measures / ms_residual;
    let (rank, f) = match test {
        AnovaTest::F => (
            None,
            Some((measures_f, f_p(measures_f, df_measures, df_residual))),
        ),
        AnovaTest::Friedman | AnovaTest::Cochran => {
            let chi = ss_measures / ms_within;
            (Some((chi, chi_square_p(chi, df_measures))), None)
        }
    };
    if test == AnovaTest::Friedman {
        let total = ss_people + ss_within;
        captions.push(format!(
            "Kendall's coefficient of concordance W = {}.",
            ss_measures / total
        ));
    }
    rows.push((
        "Within People",
        "Between Items",
        ss_measures,
        df_measures,
        rank,
        f,
    ));
    rows.push(("", "Residual", ss_residual, df_residual, None, None));
    if tukey {
        // the residual split into nonadditivity and balance
        let items = scale.k();
        let person_mean = |r: &(Vec<f64>, f64)| r.0.iter().sum::<f64>() / k;
        let item_spread: f64 = scale.mean.iter().map(|m| (m - grand).powi(2)).sum();
        let person_spread: f64 = scale
            .rows
            .iter()
            .map(|r| r.1 * (person_mean(r) - grand).powi(2))
            .sum();
        let d = item_spread * person_spread;
        let m: f64 = scale
            .rows
            .iter()
            .map(|r| {
                r.1 * (person_mean(r) - grand)
                    * (0..items)
                        .map(|i| r.0[i] * (scale.mean[i] - grand))
                        .sum::<f64>()
            })
            .sum();
        let ss_nonadd = m * m / d;
        let ss_balance = ss_residual - ss_nonadd;
        let df_balance = df_residual - 1.0;
        let fv = ss_nonadd / (ss_balance / df_balance);
        rows.push((
            "",
            "Nonadditivity",
            ss_nonadd,
            1.0,
            None,
            Some((fv, f_p(fv, 1.0, df_balance))),
        ));
        rows.push(("", "Balance", ss_balance, df_balance, None, None));
        captions.push(format!(
            "Tukey's estimate of power to which observations must be raised to achieve additivity = {}.",
            1.0 - m / d * grand
        ));
    }
    rows.push(("", "Total", ss_within, df_within, None, None));
    rows.push(("Total", "", ss_people + ss_within, w * k - 1.0, None, None));
    let rank_name = match test {
        AnovaTest::Friedman => Some("Friedman's Chi-Square"),
        AnovaTest::Cochran => Some("Cochran's Q"),
        AnovaTest::F => None,
    };
    let has_f = rows.iter().any(|r| r.5.is_some());
    let mut columns = vec!["", "", "Sum of Squares", "df", "Mean Square"];
    if let Some(name) = rank_name {
        columns.extend([name, "Sig."]);
    }
    if has_f {
        columns.extend(["F", "Sig."]);
    }
    let cells = |v: Option<(f64, f64)>| match v {
        Some((a, b)) => [number(Some(a)), number(Some(b))],
        None => [Cell::Empty, Cell::Empty],
    };
    let table_rows = rows
        .iter()
        .map(|&(a, b, ss, df, rank, f)| {
            let mut row = vec![
                if a.is_empty() { Cell::Empty } else { text(a) },
                if b.is_empty() { Cell::Empty } else { text(b) },
                number(Some(ss)),
                number(Some(df)),
                // the totals show no mean square
                if a == "Total" || b == "Total" {
                    Cell::Empty
                } else {
                    number(Some(ss / df))
                },
            ];
            if rank_name.is_some() {
                row.extend(cells(rank));
            }
            if has_f {
                row.extend(cells(f));
            }
            row
        })
        .collect();
    let mut t = plain_table("ANOVA", 2, &columns, table_rows);
    t.caption = Some(captions.join(" "));
    t
}

/// The intraclass correlation coefficients from the mean squares between people, between
/// measures, residual and within people.
fn icc_table(icc: &Icc, k: f64, w: f64, [bp, bm, res, wp]: [f64; 4]) -> Table {
    let alpha = 1.0 - icc.level / 100.0;
    let rho = icc.test_value;
    let (d1, d2_two) = (w - 1.0, (w - 1.0) * (k - 1.0));
    let upper = |d2: f64| f_quantile(alpha / 2.0, d1, d2);
    let lower = |d2: f64| f_quantile(1.0 - alpha / 2.0, d1, d2);
    // (single, average): coefficient, interval, test value, df2
    let rows: [(f64, (f64, f64), f64, f64); 2] = match (icc.model, icc.absolute) {
        (IccModel::OneWay, _) => {
            let f = bp / wp;
            let d2 = w * (k - 1.0);
            let (fu, fl) = (upper(d2), lower(d2));
            [
                (
                    (bp - wp) / (bp + (k - 1.0) * wp),
                    (
                        (f - fu) / (f + (k - 1.0) * fu),
                        (f - fl) / (f + (k - 1.0) * fl),
                    ),
                    f * (1.0 - rho) / (1.0 + (k - 1.0) * rho),
                    d2,
                ),
                (
                    (bp - wp) / bp,
                    ((f - fu) / f, (f - fl) / f),
                    f * (1.0 - rho),
                    d2,
                ),
            ]
        }
        (_, false) => {
            let f = bp / res;
            let (fu, fl) = (upper(d2_two), lower(d2_two));
            [
                (
                    (bp - res) / (bp + (k - 1.0) * res),
                    (
                        (f - fu) / (f + (k - 1.0) * fu),
                        (f - fl) / (f + (k - 1.0) * fl),
                    ),
                    f * (1.0 - rho) / (1.0 + (k - 1.0) * rho),
                    d2_two,
                ),
                (
                    (bp - res) / bp,
                    ((f - fu) / f, (f - fl) / f),
                    f * (1.0 - rho),
                    d2_two,
                ),
            ]
        }
        (_, true) => {
            let single = (bp - res) / (bp + (k - 1.0) * res + k * (bm - res) / w);
            let average = (bp - res) / (bp + (bm - res) / w);
            // Satterthwaite's degrees of freedom of a weighted sum of the mean squares
            let v = |a: f64, b: f64| {
                (a * bm + b * res).powi(2)
                    / ((a * bm).powi(2) / (k - 1.0) + (b * res).powi(2) / d2_two)
            };
            let a = k * single / (w * (1.0 - single));
            let b = 1.0 + k * single * (w - 1.0) / (w * (1.0 - single));
            let df = v(a, b);
            let (fu, fl) = (upper(df), lower(df));
            let bound = |fs: f64, spread: f64| w * (bp - fs * res) / (fs * spread + w * bp);
            let single_spread = k * bm + (k * w - k - w) * res;
            let test = |a0: f64, b0: f64| (bp / (a0 * bm + b0 * res), v(a0, b0));
            let (fs, dfs) = test(
                k * rho / (w * (1.0 - rho)),
                1.0 + k * rho * (w - 1.0) / (w * (1.0 - rho)),
            );
            let (fa, dfa) = test(
                rho / (w * (1.0 - rho)),
                1.0 + rho * (w - 1.0) / (w * (1.0 - rho)),
            );
            [
                (
                    single,
                    (bound(fu, single_spread), bound(fl, single_spread)),
                    fs,
                    dfs,
                ),
                (average, (bound(fu, bm - res), bound(fl, bm - res)), fa, dfa),
            ]
        }
    };
    let names = ["Single Measures", "Average Measures"];
    let table_rows = rows
        .iter()
        .zip(names)
        .map(|(&(value, (lo, hi), f, d2), name)| {
            vec![
                text(name),
                number(Some(value)),
                number(Some(lo)),
                number(Some(hi)),
                number(Some(f)),
                number(Some(d1)),
                number(Some(d2)),
                number(Some(f_p(f, d1, d2))),
            ]
        })
        .collect();
    let interval = format!("{}% Confidence Interval", icc.level);
    let test = format!("F Test with True Value {}", icc.test_value);
    let groups = [
        "",
        "",
        interval.as_str(),
        interval.as_str(),
        test.as_str(),
        test.as_str(),
        test.as_str(),
        test.as_str(),
    ];
    let mut t = plain_table(
        "Intraclass Correlation Coefficient",
        1,
        &[
            "",
            "Intraclass Correlation",
            "Lower Bound",
            "Upper Bound",
            "Value",
            "df1",
            "df2",
            "Sig.",
        ],
        table_rows,
    );
    t.column_groups = groups
        .iter()
        .map(|g| {
            if g.is_empty() {
                Vec::new()
            } else {
                vec![g.to_string()]
            }
        })
        .collect();
    let model = match icc.model {
        IccModel::OneWay => "One-way random effects model where people effects are random.",
        IccModel::Random => {
            "Two-way random effects model where both people effects and measures effects are random."
        }
        IccModel::Mixed => {
            "Two-way mixed effects model where people effects are random and measures effects are fixed."
        }
    };
    let mut caption = model.to_string();
    if icc.model != IccModel::OneWay {
        caption.push_str(if icc.absolute {
            " Type A intraclass correlation coefficients using an absolute agreement definition."
        } else {
            " Type C intraclass correlation coefficients using a consistency definition: the between-measure variance is excluded from the denominator variance."
        });
        caption.push_str(
            " The estimator is the same whether the interaction effect is present or not.",
        );
        if icc.model == IccModel::Mixed {
            caption.push_str(
                " The average measures are computed assuming the interaction effect is absent, because they are not estimable otherwise.",
            );
        }
    }
    t.caption = Some(caption);
    t
}
