//! AIM: attribute importance, how each variable sets every group apart from all cases.
//!
//! Rules, in our words, from the Command Syntax Reference and the Algorithms manual:
//! - A categorical variable is tested in each group i by chi-square = sum over its categories
//!   j of (n_ij - n_i p_j)^2 / (n_i p_j), p_j the share of category j among all cases, with
//!   C - 1 degrees of freedom; with a single category the statistic is 0 with no degrees of
//!   freedom and no significance.
//! - A continuous variable is tested in each group by t = (group mean - overall mean) /
//!   (s_i / root n_i), s_i the group's standard deviation (n_i - 1), with n_i - 1 degrees of
//!   freedom. A group of one case has no t; a group whose values are all equal has an infinite
//!   t and significance 0.
//! - The significance level is 1 - CI / 100; Bonferroni divides it by the number of tests of
//!   a variable, one per group; the error bar intervals use the same adjusted level.
//! - Charts show -log10 of the significance for Y=PVALUE. The weight variable is not used.
//! - A case missing (system-missing, or user-missing unless INCLUDE) on the group or any
//!   variable named is left out.
//!
//! Choices where the manual is silent (docs/differences.md): the t test is two-sided; a group
//! that equals the overall mean with no spread has t 0 and significance 1; a test passes when
//! its significance is at most the adjusted level.

use crate::chart::{Axis, Category, Chart, ChartKind, ErrorBars, Grouping, Series, Statistic};
use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, t_p, t_quantile};
use crate::npar_tests::text;
use crate::twostep::{category_codes, mean_sd, numeric_value, plain_table};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Variable;
use statscraft_syntax::Aim;

/// One test of a group against all cases.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Test {
    pub(crate) statistic: Option<f64>,
    pub(crate) df: f64,
    pub(crate) sig: Option<f64>,
}

/// Student's t of a group's values against the overall mean.
pub(crate) fn mean_test(values: &[f64], overall: f64) -> Test {
    let n = values.len() as f64;
    let (mean, sd) = mean_sd(values);
    let (Some(mean), Some(sd)) = (mean, sd) else {
        return Test {
            statistic: None,
            df: 0.0,
            sig: None,
        };
    };
    let diff = mean - overall;
    let (statistic, sig) = match sd > 0.0 {
        true => {
            let t = diff / (sd / n.sqrt());
            (t, t_p(t, n - 1.0))
        }
        false if diff == 0.0 => (0.0, 1.0),
        false => (diff.signum() * f64::INFINITY, 0.0),
    };
    Test {
        statistic: Some(statistic),
        df: n - 1.0,
        sig: Some(sig),
    }
}

/// Chi-square of a group's category counts against the overall shares of the categories.
pub(crate) fn proportion_test(counts: &[f64], shares: &[f64]) -> Test {
    let n: f64 = counts.iter().sum();
    let c = shares.len();
    if c < 2 {
        return Test {
            statistic: Some(0.0),
            df: 0.0,
            sig: None,
        };
    }
    let x: f64 = counts
        .iter()
        .zip(shares)
        .map(|(o, p)| (o - n * p).powi(2) / (n * p))
        .sum();
    let df = (c - 1) as f64;
    Test {
        statistic: Some(x),
        df,
        sig: Some(chi_square_p(x, df)),
    }
}

/// The value of chi-square with `df` degrees of freedom exceeded with probability `upper`.
fn chi_square_critical(upper: f64, df: f64) -> f64 {
    crate::distributions::evaluate("IDF.CHISQ", &[1.0 - upper, df]).unwrap_or(f64::NAN)
}

/// A clustered bar chart over categories.
fn bars(title: String, x: Axis, y: Axis, categories: Vec<Category>, series: Vec<Series>) -> Chart {
    let legend = (series.len() > 1).then(|| "Series".to_string());
    let mut chart = Chart::lines(title, x, y, categories, series);
    chart.kind = ChartKind::Bar;
    chart.grouping = Some(Grouping::Clustered);
    chart.legend = legend;
    chart
}

fn series(name: &str, statistic: Statistic, values: Vec<f64>) -> Series {
    Series {
        name: name.into(),
        statistic,
        variable: None,
        value: Cell::Empty,
        values,
    }
}

fn key_cell(key: &Key) -> Cell {
    match key {
        Key::Number(n) => Cell::Number(*n),
        Key::Text(t) => Cell::Text(t.clone()),
    }
}

fn category(label: String, value: Cell) -> Category {
    Category {
        label,
        value,
        missing: false,
        bin: None,
    }
}

/// The tests of one variable, one per group, with what a chart needs of them.
struct Tested {
    name: String,
    label: String,
    continuous: bool,
    tests: Vec<Test>,
}

impl Tested {
    fn passes(&self, i: usize, level: f64) -> bool {
        self.tests[i].sig.is_some_and(|p| p <= level)
    }

    /// The critical value of group `i`'s test at `level`: t two-sided or chi-square.
    fn critical(&self, i: usize, level: f64) -> f64 {
        let df = self.tests[i].df;
        if df <= 0.0 {
            f64::NAN
        } else if self.continuous {
            t_quantile(level / 2.0, df)
        } else {
            chi_square_critical(level, df)
        }
    }
}

fn minus_log10(p: Option<f64>) -> f64 {
    p.map_or(f64::NAN, |p| -p.log10())
}

impl Engine {
    pub(crate) fn aim(&mut self, command: &Aim) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let group = data.variable(&command.group)?;
        let continuous_names = self.expand(&command.continuous)?;
        let categorical_names = self.expand(&command.categorical)?;
        let mut seen: Vec<&String> = Vec::new();
        for name in continuous_names.iter().chain(&categorical_names) {
            if name.eq_ignore_ascii_case(group.name()) {
                return Err(Error(format!(
                    "AIM: the grouping variable {name} cannot also be a categorical or continuous variable"
                )));
            }
            if seen.iter().any(|s| s.eq_ignore_ascii_case(name)) {
                return Err(Error(format!("AIM: {name} is named more than once")));
            }
            seen.push(name);
        }
        let continuous: Vec<Variable> = continuous_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = continuous.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "AIM: continuous variable {} is a string variable",
                v.name()
            )));
        }
        let categorical: Vec<Variable> = categorical_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let mut warnings = Vec::new();
        if case_weights(data)?.is_some() {
            warnings.push("AIM does not use the weight variable".to_string());
        }
        let include = command.include;
        let group_keys = categories(&group, include);
        let keys: Vec<Vec<Option<Key>>> =
            categorical.iter().map(|v| categories(v, include)).collect();
        let used: Vec<usize> = (0..data.case_count())
            .filter(|&case| {
                group_keys[case].is_some()
                    && keys.iter().all(|k| k[case].is_some())
                    && continuous
                        .iter()
                        .all(|v| numeric_value(v, case, include).is_some())
            })
            .collect();
        if used.is_empty() {
            return Err(Error(
                "AIM has no case without missing values on the variables".into(),
            ));
        }
        let (groups, membership) = category_codes(
            &used
                .iter()
                .map(|&c| group_keys[c].clone())
                .collect::<Vec<_>>(),
        );
        let membership: Vec<usize> = membership.into_iter().map(|m| m.expect("valid")).collect();
        let g = groups.len();
        let sizes: Vec<f64> = (0..g)
            .map(|i| membership.iter().filter(|&&m| m == i).count() as f64)
            .collect();
        let alpha = 1.0 - command.ci / 100.0;
        let level = match command.bonferroni {
            true => alpha / g as f64,
            false => alpha,
        };
        let shown = self.shown();
        let group_title = shown.variable(&group);
        let group_labels: Vec<String> = groups.iter().map(|k| shown.value(&group, k)).collect();
        let group_categories: Vec<Category> = groups
            .iter()
            .zip(&group_labels)
            .map(|(k, label)| category(label.clone(), key_cell(k)))
            .collect();
        let caption = match command.bonferroni && g > 1 {
            true => format!(
                "Significance level {} divided by {g} groups (Bonferroni): {}",
                round4(alpha),
                round4(level)
            ),
            false => format!("Significance level {}", round4(alpha)),
        };

        // continuous variables: the values of each group
        let mut tested = Vec::new();
        let mut mean_rows = Vec::new();
        let mut error_bars = Vec::new();
        for v in &continuous {
            let values: Vec<f64> = used
                .iter()
                .map(|&c| numeric_value(v, c, include).expect("valid"))
                .collect();
            let (overall, overall_sd) = mean_sd(&values);
            let overall = overall.expect("cases");
            let per_group: Vec<Vec<f64>> = (0..g)
                .map(|i| {
                    values
                        .iter()
                        .zip(&membership)
                        .filter(|(_, m)| **m == i)
                        .map(|(x, _)| *x)
                        .collect()
                })
                .collect();
            let tests: Vec<Test> = per_group.iter().map(|x| mean_test(x, overall)).collect();
            let item = Tested {
                name: v.name().to_string(),
                label: shown.variable(v),
                continuous: true,
                tests,
            };
            let mut means = Vec::new();
            let mut margins = Vec::new();
            for (i, x) in per_group.iter().enumerate() {
                let (mean, sd) = mean_sd(x);
                let test = item.tests[i];
                mean_rows.push(vec![
                    if i == 0 {
                        text(&item.label)
                    } else {
                        Cell::Empty
                    },
                    text(&group_labels[i]),
                    Cell::Number(sizes[i]),
                    number(mean),
                    number(sd),
                    number(test.statistic),
                    Cell::Number(test.df),
                    number(test.sig),
                    text(if item.passes(i, level) { "Yes" } else { "No" }),
                ]);
                means.push(mean.unwrap_or(f64::NAN));
                let n = x.len() as f64;
                margins.push(match sd {
                    Some(sd) if n > 1.0 => t_quantile(level / 2.0, n - 1.0) * sd / n.sqrt(),
                    _ => f64::NAN,
                });
            }
            mean_rows.push(vec![
                Cell::Empty,
                text("All cases"),
                Cell::Number(values.len() as f64),
                Cell::Number(overall),
                number(overall_sd),
                Cell::Empty,
                Cell::Empty,
                Cell::Empty,
                Cell::Empty,
            ]);
            if command.error_bar {
                error_bars.push(self.error_bar_chart(
                    command,
                    &item,
                    (group_title.as_str(), group_categories.as_slice()),
                    (means, margins, overall),
                    level,
                ));
            }
            tested.push(item);
        }

        // categorical variables: the category counts of each group
        let mut share_rows = Vec::new();
        let mut category_charts = Vec::new();
        for (v, k) in categorical.iter().zip(&keys) {
            let (cats, codes) =
                category_codes(&used.iter().map(|&c| k[c].clone()).collect::<Vec<_>>());
            let total = used.len() as f64;
            let counts: Vec<Vec<f64>> = (0..g)
                .map(|i| {
                    (0..cats.len())
                        .map(|j| {
                            membership
                                .iter()
                                .zip(&codes)
                                .filter(|(m, c)| **m == i && **c == Some(j))
                                .count() as f64
                        })
                        .collect()
                })
                .collect();
            let overall: Vec<f64> = (0..cats.len())
                .map(|j| counts.iter().map(|row| row[j]).sum::<f64>())
                .collect();
            let shares: Vec<f64> = overall.iter().map(|n| n / total).collect();
            let item = Tested {
                name: v.name().to_string(),
                label: shown.variable(v),
                continuous: false,
                tests: counts.iter().map(|c| proportion_test(c, &shares)).collect(),
            };
            for (i, test) in item.tests.iter().enumerate() {
                share_rows.push(vec![
                    if i == 0 {
                        text(&item.label)
                    } else {
                        Cell::Empty
                    },
                    text(&group_labels[i]),
                    Cell::Number(sizes[i]),
                    number(test.statistic),
                    Cell::Number(test.df),
                    number(test.sig),
                    text(if item.passes(i, level) { "Yes" } else { "No" }),
                ]);
            }
            if command.category_plot {
                let mut lines: Vec<Series> = (0..g)
                    .filter(|&i| !command.hide_not_significant || item.passes(i, level))
                    .map(|i| Series {
                        name: group_labels[i].clone(),
                        statistic: Statistic::Percent,
                        variable: Some(group.name().to_string()),
                        value: key_cell(&groups[i]),
                        values: counts[i].iter().map(|n| 100.0 * n / sizes[i]).collect(),
                    })
                    .collect();
                lines.push(series(
                    "All cases",
                    Statistic::Percent,
                    shares.iter().map(|p| 100.0 * p).collect(),
                ));
                let x = Axis {
                    variable: Some(v.name().to_string()),
                    ..Axis::titled(item.label.clone())
                };
                let y = Axis {
                    format: crate::chart::PERCENT_FORMAT.into(),
                    ..Axis::titled("Percent within group")
                };
                let mut chart = bars(
                    format!("{} within each group of {group_title}", item.label),
                    x,
                    y,
                    cats.iter()
                        .map(|c| category(shown.value(v, c), key_cell(c)))
                        .collect(),
                    lines,
                );
                chart.legend = Some(group_title.clone());
                category_charts.push(chart);
            }
            tested.push(item);
        }

        let mut tables = Vec::new();
        if !mean_rows.is_empty() {
            let columns = [
                "Variable",
                "Group",
                "N",
                "Mean",
                "Std. Deviation",
                "t",
                "df",
                "Sig.",
                "Significant",
            ];
            let mut table = plain_table(
                "Group Means against the Overall Mean",
                2,
                &columns,
                mean_rows,
            );
            table.caption = Some(caption.clone());
            tables.push(table);
        }
        if !share_rows.is_empty() {
            let columns = [
                "Variable",
                "Group",
                "N",
                "Chi-Square",
                "df",
                "Sig.",
                "Significant",
            ];
            let mut table = plain_table(
                "Group Category Shares against the Overall Shares",
                2,
                &columns,
                share_rows,
            );
            table.caption = Some(caption.clone());
            tables.push(table);
        }

        let mut charts = Vec::new();
        if let Some(pie) = command.cluster_plot {
            let mut chart = bars(
                format!("Cases in each group of {group_title}"),
                Axis {
                    variable: Some(group.name().to_string()),
                    ..Axis::titled(group_title.clone())
                },
                Axis {
                    format: crate::chart::count_format(sizes.iter().copied()),
                    ..Axis::titled("Count")
                },
                group_categories.clone(),
                vec![series("Count", Statistic::Count, sizes.clone())],
            );
            if pie {
                chart.kind = ChartKind::Pie;
                chart.orientation = None;
                chart.grouping = None;
            }
            charts.push(chart);
        }
        charts.extend(category_charts);
        charts.extend(error_bars);
        if let Some(importance) = command.importance {
            charts.extend(importance_charts(
                command,
                importance,
                &tested,
                (
                    group_title.as_str(),
                    group_labels.as_slice(),
                    group_categories.as_slice(),
                ),
                level,
            ));
        }
        for warning in warnings {
            self.warn(warning);
        }
        self.charts.extend(charts);
        Ok(tables)
    }

    /// The means of the groups with their intervals, and the overall mean as a reference.
    fn error_bar_chart(
        &self,
        command: &Aim,
        item: &Tested,
        (group_title, group_categories): (&str, &[Category]),
        (means, margins, overall): (Vec<f64>, Vec<f64>, f64),
        level: f64,
    ) -> Chart {
        let keep: Vec<usize> = (0..means.len())
            .filter(|&i| !command.hide_not_significant || item.passes(i, level))
            .collect();
        let pick = |values: &[f64]| keep.iter().map(|&i| values[i]).collect::<Vec<f64>>();
        let mut lines = vec![series("Mean", Statistic::Mean, pick(&means))];
        let mut bars_margins = vec![pick(&margins)];
        if command.reference_lines {
            lines.push(series(
                "Overall mean",
                Statistic::Mean,
                vec![overall; keep.len()],
            ));
            bars_margins.push(Vec::new());
        }
        let mut chart = Chart::lines(
            format!("{} by {group_title}", item.label),
            Axis::titled(group_title),
            Axis {
                variable: Some(item.name.clone()),
                ..Axis::titled(item.label.clone())
            },
            keep.iter().map(|&i| group_categories[i].clone()).collect(),
            lines,
        );
        chart.kind = ChartKind::ErrorBar;
        chart.error_bars = Some(ErrorBars {
            label: format!("{}% CI", round4(100.0 * (1.0 - level))),
            margins: bars_margins,
            lower: Vec::new(),
        });
        chart
    }
}

/// The importance charts: per variable over the groups, or per group over the variables.
fn importance_charts(
    command: &Aim,
    importance: statscraft_syntax::AimImportance,
    tested: &[Tested],
    (group_title, group_labels, group_categories): (&str, &[String], &[Category]),
    level: f64,
) -> Vec<Chart> {
    let hide = command.hide_not_significant;
    let reference = command.reference_lines;
    let value_of = |item: &Tested, i: usize| match importance.pvalue {
        true => minus_log10(item.tests[i].sig),
        false => item.tests[i].statistic.unwrap_or(f64::NAN),
    };
    let y_title = match importance.pvalue {
        true => "-log10(Sig.)",
        false => "Test statistic",
    };
    let mut charts = Vec::new();
    if !importance.by_variable {
        for item in tested {
            let keep: Vec<usize> = (0..group_labels.len())
                .filter(|&i| !hide || item.passes(i, level))
                .collect();
            if keep.is_empty() {
                continue;
            }
            let name = match (importance.pvalue, item.continuous) {
                (true, _) => "-log10(Sig.)",
                (false, true) => "Student's t",
                (false, false) => "Chi-square",
            };
            let mut lines = vec![series(
                name,
                Statistic::Value,
                keep.iter().map(|&i| value_of(item, i)).collect(),
            )];
            if reference {
                lines.extend(reference_lines(importance.pvalue, level, |both| {
                    keep.iter().map(|&i| both(item, i)).collect::<Vec<f64>>()
                }));
            }
            charts.push(bars(
                format!(
                    "Importance of {} for each group of {group_title}",
                    item.label
                ),
                Axis::titled(group_title),
                Axis::titled(y_title),
                keep.iter().map(|&i| group_categories[i].clone()).collect(),
                lines,
            ));
        }
        return charts;
    }
    for (i, label) in group_labels.iter().enumerate() {
        let keep: Vec<&Tested> = tested
            .iter()
            .filter(|item| !hide || item.passes(i, level))
            .collect();
        if keep.is_empty() {
            continue;
        }
        let pick = |kind: bool| -> Vec<f64> {
            keep.iter()
                .map(|item| match item.continuous == kind {
                    true => value_of(item, i),
                    false => f64::NAN,
                })
                .collect()
        };
        let mut lines = match importance.pvalue {
            true => vec![series(
                "-log10(Sig.)",
                Statistic::Value,
                keep.iter().map(|item| value_of(item, i)).collect(),
            )],
            false => vec![
                series("Student's t", Statistic::Value, pick(true)),
                series("Chi-square", Statistic::Value, pick(false)),
            ],
        };
        lines.retain(|s| s.values.iter().any(|v| !v.is_nan()) || importance.pvalue);
        if reference {
            lines.extend(reference_lines(importance.pvalue, level, |both| {
                keep.iter().map(|item| both(item, i)).collect::<Vec<f64>>()
            }));
        }
        charts.push(bars(
            format!("Importance of each variable for {group_title} = {label}"),
            Axis::titled("Variable"),
            Axis::titled(y_title),
            keep.iter()
                .map(|item| category(item.label.clone(), text(&item.name)))
                .collect(),
            lines,
        ));
    }
    charts
}

/// The reference lines of an importance chart: -log10 of the level for significances, else
/// the critical values (both signs for t). `over` maps a value of (variable, group) over the
/// chart's bars.
fn reference_lines(
    pvalue: bool,
    level: f64,
    over: impl Fn(&dyn Fn(&Tested, usize) -> f64) -> Vec<f64>,
) -> Vec<Series> {
    if pvalue {
        let line = -level.log10();
        return vec![series(
            "Critical value",
            Statistic::Value,
            over(&|_, _| line),
        )];
    }
    let upper = over(&|item, i| item.critical(i, level));
    let lower = over(&|item, i| match item.continuous {
        true => -item.critical(i, level),
        false => f64::NAN,
    });
    let mut lines = vec![series("Critical value", Statistic::Value, upper)];
    if lower.iter().any(|v| !v.is_nan()) {
        lines.push(series("Negative critical value", Statistic::Value, lower));
    }
    lines
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_group_gets_an_infinite_t_or_none() {
        let t = mean_test(&[3.0, 3.0, 3.0], 2.0);
        assert_eq!(
            (t.statistic, t.df, t.sig),
            (Some(f64::INFINITY), 2.0, Some(0.0))
        );
        let t = mean_test(&[2.0, 2.0], 2.0);
        assert_eq!((t.statistic, t.sig), (Some(0.0), Some(1.0)));
        let t = mean_test(&[5.0], 2.0);
        assert_eq!((t.statistic, t.df, t.sig), (None, 0.0, None));
        let t = proportion_test(&[4.0], &[1.0]);
        assert_eq!((t.statistic, t.df, t.sig), (Some(0.0), 0.0, None));
    }
}
