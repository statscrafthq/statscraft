//! FREQUENCIES: frequency tables, summary statistics and percentiles.
//!
//! Rules from the Command Syntax Reference (FREQUENCIES) and the Algorithms manual
//! (FREQUENCIES, Grouped Percentiles):
//! - Cases with a missing, zero or negative weight are left out; the others count with their
//!   weight, not rounded. W is the summed weight of the valid values.
//! - Percent is of all cases, valid and cumulative percent of the valid ones. User-missing
//!   values are missing unless INCLUDE, which makes them valid everywhere.
//! - Variance, standard deviation and the standard error of the mean need W > 1; skewness and
//!   its standard error W >= 3, kurtosis and its standard error W >= 4, both a positive variance.
//! - The mode is the most frequent value; of tied ones the smallest, with a note.
//! - Percentiles: tc = (W + 1) p, see `Distribution::frequencies_percentile`; the median is the
//!   50th. A percentile below the first value is that value, above the last value the last.
//! - GROUPED alone (midpoints): cc_i = c_1 + ... + c_i-1 + c_i / 2 and, with cc_i-1 <= W p < cc_i,
//!   (1 - R) x_i-1 + R x_i, R = (W p - cc_i-1) / (cc_i - cc_i-1); undefined below cc_1 and
//!   above cc_k. With a width or boundaries the groups are known: the percentile lies in the
//!   first group whose cumulative weight reaches W p, interpolated linearly within it; in an
//!   open group (a value below the first or above the last boundary) it is undefined.
//! - DVALUE, AFREQ and DFREQ are ignored when percentiles or a histogram are asked for; the
//!   median is ignored when AFREQ or DFREQ order the table.
//!
//! Where the manual is silent: the Statistics table appears only when statistics or
//! percentiles are asked for or a frequency table is left out (the manual's basic
//! specification gives frequency tables alone); LIMIT counts every row of values, missing
//! ones too; values with equal frequencies keep ascending order of value; a value on a group
//! boundary belongs to the group above it, except the last boundary, which closes the last group.

use crate::descriptives::{case_weights, number, summarize};
use crate::examine::{Distribution, short};
use crate::format::{Stat, formats_of, procedure_format};
use crate::shown::{Show, Shown, drop_label_column};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Value, Values, Variable};
use statscraft_syntax::{Frequencies, FrequencyOrder as Order, FrequencyStatistic as S, Grouping};
use std::cmp::Ordering;

struct Row {
    value: Cell,
    label: Option<String>,
    count: f64,
    missing: bool,
}

/// What one variable gives.
struct Analysis {
    name: String,
    /// The title (SET OVARS) and the column header of the Statistics table (TVARS).
    title: String,
    heading: String,
    /// Valid values first, then missing ones, each in ascending order.
    rows: Vec<Row>,
    numeric: bool,
    /// The print format of the variable.
    format: String,
    valid: f64,
    total: f64,
    /// The distinct valid values in ascending order with their summed weights.
    pairs: Vec<(f64, f64)>,
}

fn label_of(variable: &Variable<'_>, is_value: impl Fn(&Value) -> bool) -> Option<String> {
    variable
        .value_labels()
        .iter()
        .find(|(v, _)| is_value(v))
        .map(|(_, l)| l.clone())
}

fn analyse(
    data: &Dataset,
    (shown, outline): (&Shown, &Shown),
    name: &str,
    weights: &Option<Vec<f64>>,
    include: bool,
) -> Result<Analysis, Error> {
    let variable = data.variable(name)?;
    let weight = |case: usize| weights.as_ref().map_or(1.0, |w| w[case]);
    let mut rows = match variable.values() {
        Values::Numeric(values) => {
            let mut cases: Vec<(f64, f64)> = Vec::new();
            let mut system_missing = 0.0;
            for (case, v) in values.iter().enumerate() {
                match v {
                    Some(v) => cases.push((v, weight(case))),
                    None => system_missing += weight(case),
                }
            }
            cases.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut rows: Vec<Row> = Vec::new();
            for (v, w) in cases {
                match rows.last_mut() {
                    Some(Row {
                        value: Cell::Number(last),
                        count,
                        ..
                    }) if *last == v => *count += w,
                    _ => rows.push(Row {
                        value: Cell::Number(v),
                        label: label_of(&variable, |l| matches!(l, Value::Number(n) if *n == v)),
                        count: w,
                        missing: !include && variable.is_user_missing(v),
                    }),
                }
            }
            if system_missing > 0.0 {
                rows.push(Row {
                    value: Cell::Empty,
                    label: Some("System".into()),
                    count: system_missing,
                    missing: true,
                });
            }
            rows
        }
        Values::String(values) => {
            let mut cases: Vec<(&str, f64)> = values
                .iter()
                .enumerate()
                .map(|(c, v)| (v, weight(c)))
                .collect();
            cases.sort_by(|a, b| a.0.cmp(b.0));
            let mut rows: Vec<Row> = Vec::new();
            for (v, w) in cases {
                match rows.last_mut() {
                    Some(Row {
                        value: Cell::Text(last),
                        count,
                        ..
                    }) if last == v => *count += w,
                    _ => rows.push(Row {
                        value: Cell::Text(v.to_string()),
                        label: label_of(
                            &variable,
                            |l| matches!(l, Value::String(s) if s.trim_end() == v),
                        ),
                        count: w,
                        missing: !include && variable.is_user_missing_str(v),
                    }),
                }
            }
            rows
        }
    };
    rows.retain(|r| r.count > 0.0);
    // valid values first, then missing ones, each in ascending order
    rows.sort_by_key(|r| r.missing);
    let pairs: Vec<(f64, f64)> = rows
        .iter()
        .filter(|r| !r.missing)
        .filter_map(|r| match r.value {
            Cell::Number(v) => Some((v, r.count)),
            _ => None,
        })
        .collect();
    Ok(Analysis {
        name: name.to_string(),
        title: outline.variable(&variable),
        heading: shown.variable(&variable),
        numeric: !variable.is_string(),
        format: variable.format().to_string(),
        valid: rows.iter().filter(|r| !r.missing).map(|r| r.count).sum(),
        total: rows.iter().map(|r| r.count).sum(),
        rows,
        pairs,
    })
}

/// Rows in the order asked for; missing values stay after the valid ones.
fn order_rows(rows: &mut [Row], order: Order) {
    let by_value = |a: &Row, b: &Row| match (&a.value, &b.value) {
        (Cell::Number(x), Cell::Number(y)) => x.total_cmp(y),
        (Cell::Text(x), Cell::Text(y)) => x.cmp(y),
        // the system-missing row stays last
        (Cell::Empty, _) => Ordering::Greater,
        (_, Cell::Empty) => Ordering::Less,
        _ => Ordering::Equal,
    };
    rows.sort_by(|a, b| {
        a.missing.cmp(&b.missing).then_with(|| match order {
            Order::AValue => by_value(a, b),
            Order::DValue if a.value == Cell::Empty || b.value == Cell::Empty => by_value(a, b),
            Order::DValue => by_value(b, a),
            Order::AFreq => a.count.total_cmp(&b.count).then_with(|| by_value(a, b)),
            Order::DFreq => b.count.total_cmp(&a.count).then_with(|| by_value(a, b)),
        })
    });
}

/// `labels`: the Label column, which SET TNUMBERS=VALUES leaves out; the Value column
/// stays whatever the setting, so the value is never lost.
fn frequency_table(analysis: &Analysis, labels: bool) -> Table {
    let (total, valid) = (analysis.total, analysis.valid);
    let percent = |part: f64, whole: f64| {
        if whole > 0.0 {
            Cell::Number(100.0 * part / whole)
        } else {
            Cell::Empty
        }
    };
    let mut cumulative = 0.0;
    let mut out: Vec<Vec<Cell>> = analysis
        .rows
        .iter()
        .map(|r| {
            let (valid_percent, cumulative_percent) = if r.missing {
                (Cell::Empty, Cell::Empty)
            } else {
                cumulative += r.count;
                (percent(r.count, valid), percent(cumulative, valid))
            };
            vec![
                Cell::Text(if r.missing { "missing" } else { "valid" }.into()),
                r.value.clone(),
                r.label.clone().map_or(Cell::Empty, Cell::Text),
                Cell::Number(r.count),
                percent(r.count, total),
                valid_percent,
                cumulative_percent,
            ]
        })
        .collect();
    out.push(vec![
        Cell::Text("total".into()),
        Cell::Empty,
        Cell::Empty,
        Cell::Number(total),
        percent(total, total),
        Cell::Empty,
        Cell::Empty,
    ]);
    let value_format = procedure_format(&analysis.format, Stat::Value);
    let mut table = Table {
        formats: formats_of(&out, |_, c| match c {
            1 => value_format.clone(),
            _ => String::new(),
        }),
        title: analysis.title.clone(),
        row_headers: 3,
        column_groups: vec![Vec::new(); 7],
        columns: [
            "Category",
            "Value",
            "Label",
            "Frequency",
            "Percent",
            "Valid Percent",
            "Cumulative Percent",
        ]
        .map(String::from)
        .to_vec(),
        rows: out,
        ..Table::default()
    };
    if !labels {
        drop_label_column(&mut table, 2);
    }
    table
}

fn statistic_name(stat: S) -> &'static str {
    match stat {
        S::Mean => "Mean",
        S::SeMean => "Std. Error of Mean",
        S::Median => "Median",
        S::Mode => "Mode",
        S::StdDev => "Std. Deviation",
        S::Variance => "Variance",
        S::Skewness => "Skewness",
        S::SeSkewness => "Std. Error of Skewness",
        S::Kurtosis => "Kurtosis",
        S::SeKurtosis => "Std. Error of Kurtosis",
        S::Range => "Range",
        S::Minimum => "Minimum",
        S::Maximum => "Maximum",
        S::Sum => "Sum",
    }
}

/// How a statistic relates to the values, for its format; None for one without their unit.
fn statistic_kind(stat: S) -> Option<Stat> {
    match stat {
        S::Mean | S::Median | S::Mode | S::Minimum | S::Maximum => Some(Stat::Value),
        S::SeMean | S::StdDev | S::Range => Some(Stat::Spread),
        S::Sum => Some(Stat::Sum),
        S::Variance => Some(Stat::Other),
        S::Skewness | S::SeSkewness | S::Kurtosis | S::SeKurtosis => None,
    }
}

/// The most frequent value and whether others are as frequent.
fn mode(pairs: &[(f64, f64)]) -> Option<(f64, bool)> {
    let most = pairs.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let mut tied = pairs.iter().filter(|p| p.1 == most);
    let first = tied.next()?;
    Some((first.0, tied.next().is_some()))
}

/// Why a grouped percentile is undefined.
const LOWER_UNKNOWN: &str =
    "the lower bound of the first group is unknown, so some grouped percentiles are undefined";
const UPPER_UNKNOWN: &str =
    "the upper bound of the last group is unknown, so some grouped percentiles are undefined";

/// The `percent` percentile of grouped values (distinct, ascending, positive weights).
fn grouped_percentile(
    pairs: &[(f64, f64)],
    grouping: &Grouping,
    percent: f64,
) -> Result<Option<f64>, &'static str> {
    let w: f64 = pairs.iter().map(|p| p.1).sum();
    if pairs.is_empty() {
        return Ok(None);
    }
    let wp = w * percent / 100.0;
    let fuzz = 1e-10 * w.max(1.0);
    // (lower, upper, weight) of every group in ascending order; None is an open end
    let groups: Vec<(Option<f64>, Option<f64>, f64)> = match grouping {
        Grouping::Midpoints => {
            let mut before = 0.0;
            let cc: Vec<f64> = pairs
                .iter()
                .map(|(_, c)| {
                    let mid = before + c / 2.0;
                    before += c;
                    mid
                })
                .collect();
            let last = cc.len() - 1;
            if wp < cc[0] - fuzz {
                return Err(LOWER_UNKNOWN);
            }
            if wp > cc[last] + fuzz {
                return Err(UPPER_UNKNOWN);
            }
            if wp >= cc[last] - fuzz {
                return Ok(Some(pairs[last].0));
            }
            let i = cc.partition_point(|&c| c <= wp + fuzz).max(1);
            let r = ((wp - cc[i - 1]) / (cc[i] - cc[i - 1])).max(0.0);
            return Ok(Some((1.0 - r) * pairs[i - 1].0 + r * pairs[i].0));
        }
        Grouping::Width(h) => pairs
            .iter()
            .map(|&(x, c)| (Some(x - h / 2.0), Some(x + h / 2.0), c))
            .collect(),
        Grouping::Boundaries(bounds) => {
            let last = bounds.len() - 1;
            let mut groups: Vec<(Option<f64>, Option<f64>, f64)> = Vec::new();
            groups.push((None, Some(bounds[0]), 0.0));
            groups.extend(bounds.windows(2).map(|b| (Some(b[0]), Some(b[1]), 0.0)));
            groups.push((Some(bounds[last]), None, 0.0));
            for &(x, c) in pairs {
                let index = if x == bounds[last] {
                    last
                } else {
                    bounds.partition_point(|&b| b <= x)
                };
                groups[index].2 += c;
            }
            groups
        }
    };
    let mut before = 0.0;
    let nonempty: Vec<&(Option<f64>, Option<f64>, f64)> =
        groups.iter().filter(|g| g.2 > 0.0).collect();
    let mut chosen = None;
    for g in &nonempty {
        if before + g.2 + fuzz >= wp {
            chosen = Some((*g, before));
            break;
        }
        before += g.2;
    }
    let ((lower, upper, c), before) = match chosen {
        Some(found) => found,
        None => (
            *nonempty.last().expect("pairs are not empty"),
            w - nonempty.last().unwrap().2,
        ),
    };
    let (Some(lower), Some(upper)) = (lower, upper) else {
        return Err(if lower.is_none() {
            LOWER_UNKNOWN
        } else {
            UPPER_UNKNOWN
        });
    };
    let share = ((wp - before) / c).clamp(0.0, 1.0);
    Ok(Some(lower + share * (upper - lower)))
}

impl Engine {
    pub(crate) fn frequencies(&mut self, command: &Frequencies) -> Result<Vec<Table>, Error> {
        let names = self.expand(&command.variables)?;
        let mut groupings: Vec<(String, &Grouping)> = Vec::new();
        for (items, grouping) in &command.grouped {
            for name in self.expand(items)? {
                if !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                    return Err(Error(format!(
                        "GROUPED names {name}, which is not on VARIABLES of FREQUENCIES"
                    )));
                }
                groupings.retain(|(n, _)| !n.eq_ignore_ascii_case(&name));
                groupings.push((name, grouping));
            }
        }
        let data = self.active()?;
        let (shown, outline) = (self.shown(), self.outline());
        let weights = case_weights(data)?;
        let mut analyses: Vec<Analysis> = names
            .iter()
            .map(|name| analyse(data, (&shown, &outline), name, &weights, command.include))
            .collect::<Result<_, _>>()?;

        let mut warnings = Vec::new();
        if !command.charts.is_empty() {
            warnings.push(format!(
                "charts are not drawn yet: {} left out; the tables and statistics are shown",
                command.charts.join(", ")
            ));
        }
        let histogram = command.charts.iter().any(|c| c == "HISTOGRAM");
        let mut order = command.order;
        if order != Order::AValue && (histogram || !command.percentiles.is_empty()) {
            warnings.push(
                "DVALUE, AFREQ and DFREQ are ignored with percentiles or a histogram; the values \
                 are in ascending order"
                    .into(),
            );
            order = Order::AValue;
        }
        let mut statistics = command.statistics.clone();
        if matches!(order, Order::AFreq | Order::DFreq) && statistics.contains(&S::Median) {
            warnings.push("the median is left out when AFREQ or DFREQ order the values".into());
            statistics.retain(|s| *s != S::Median);
        }

        let mut tables_of: Vec<Option<Table>> = Vec::new();
        for analysis in &mut analyses {
            let table = !command.notable
                && command
                    .limit
                    .is_none_or(|limit| analysis.rows.len() <= limit);
            tables_of.push(table.then(|| {
                order_rows(&mut analysis.rows, order);
                frequency_table(analysis, shown.values != Show::Plain)
            }));
        }
        let wanted = !statistics.is_empty()
            || !command.percentiles.is_empty()
            || tables_of.iter().any(Option::is_none);

        let mut columns: Vec<Vec<Cell>> = Vec::new();
        let mut multiple_modes = Vec::new();
        for analysis in &analyses {
            let grouping = groupings
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(&analysis.name))
                .map(|(_, g)| *g);
            let (column, tied) = self.statistics_column(
                analysis,
                &statistics,
                &command.percentiles,
                grouping,
                &mut warnings,
            );
            if tied {
                multiple_modes.push(analysis.name.clone());
            }
            columns.push(column);
        }
        for warning in warnings {
            self.warn(warning);
        }

        let mut row_labels: Vec<[Cell; 2]> = vec![
            [Cell::Text("N".into()), Cell::Text("Valid".into())],
            [Cell::Text("N".into()), Cell::Text("Missing".into())],
        ];
        row_labels.extend(
            statistics
                .iter()
                .map(|s| [Cell::Text(statistic_name(*s).into()), Cell::Empty]),
        );
        row_labels.extend(
            command
                .percentiles
                .iter()
                .map(|p| [Cell::Text("Percentiles".into()), Cell::Text(short(*p))]),
        );
        let statistics_table = |indices: &[usize]| {
            let rows: Vec<Vec<Cell>> = row_labels
                .iter()
                .enumerate()
                .map(|(r, labels)| {
                    let mut row = labels.to_vec();
                    row.extend(indices.iter().map(|&i| columns[i][r].clone()));
                    row
                })
                .collect();
            let mut headers = vec!["Statistic".to_string(), String::new()];
            headers.extend(indices.iter().map(|&i| analyses[i].heading.clone()));
            let tied: Vec<&str> = indices
                .iter()
                .map(|&i| analyses[i].name.as_str())
                .filter(|n| multiple_modes.iter().any(|m| m == n))
                .collect();
            // the two N rows and the label columns are plain; percentiles are values
            let formats = formats_of(&rows, |r, c| {
                let kind = match r.checked_sub(2) {
                    _ if c < 2 => None,
                    Some(s) if s < statistics.len() => statistic_kind(statistics[s]),
                    Some(_) => Some(Stat::Value),
                    None => None,
                };
                kind.map_or_else(String::new, |kind| {
                    procedure_format(&analyses[indices[c - 2]].format, kind)
                })
            });
            Table {
                title: "Statistics".into(),
                formats,
                row_headers: 2,
                column_groups: vec![Vec::new(); headers.len()],
                columns: headers,
                rows,
                caption: (!tied.is_empty()).then(|| {
                    format!(
                        "Several values share the highest frequency ({}); the mode shown is the smallest.",
                        tied.join(", ")
                    )
                }),
            }
        };
        let mut out = Vec::new();
        if command.by_variable {
            for (i, table) in tables_of.into_iter().enumerate() {
                if wanted {
                    out.push(statistics_table(&[i]));
                }
                out.extend(table);
            }
        } else {
            if wanted {
                let all: Vec<usize> = (0..analyses.len()).collect();
                out.push(statistics_table(&all));
            }
            out.extend(tables_of.into_iter().flatten());
        }
        Ok(out)
    }

    /// The cells of one variable in the Statistics table, and whether its mode is tied.
    fn statistics_column(
        &self,
        analysis: &Analysis,
        statistics: &[S],
        percentiles: &[f64],
        grouping: Option<&Grouping>,
        warnings: &mut Vec<String>,
    ) -> (Vec<Cell>, bool) {
        let mut cells = vec![
            Cell::Number(analysis.valid),
            Cell::Number(analysis.total - analysis.valid),
        ];
        if !analysis.numeric {
            cells.resize(2 + statistics.len() + percentiles.len(), Cell::Empty);
            return (cells, false);
        }
        let pairs = &analysis.pairs;
        let summary = summarize(pairs);
        let w = summary.n;
        let dist = Distribution::new(pairs);
        let mut tied = false;
        let mut percentile = |p: f64| -> Cell {
            match grouping {
                None => number(dist.frequencies_percentile(p)),
                Some(grouping) => match grouped_percentile(pairs, grouping, p) {
                    Ok(value) => number(value),
                    Err(why) => {
                        let warning = format!("{}: {why}", analysis.name);
                        if !warnings.contains(&warning) {
                            warnings.push(warning);
                        }
                        Cell::Empty
                    }
                },
            }
        };
        let variance = summary.sd.filter(|_| w > 1.0).map(|sd| sd * sd);
        let spread = variance.filter(|v| *v > 0.0);
        for stat in statistics {
            cells.push(match stat {
                S::Mean => number(summary.mean),
                S::SeMean => number(variance.map(|v| (v / w).sqrt())),
                S::Median => percentile(50.0),
                S::Mode => match mode(pairs) {
                    Some((value, several)) => {
                        tied |= several;
                        Cell::Number(value)
                    }
                    None => Cell::Empty,
                },
                S::StdDev => number(variance.map(f64::sqrt)),
                S::Variance => number(variance),
                S::Skewness => number(summary.skewness.filter(|_| w >= 3.0 && spread.is_some())),
                S::SeSkewness => {
                    number(summary.se_skewness.filter(|_| w >= 3.0 && spread.is_some()))
                }
                S::Kurtosis => number(summary.kurtosis.filter(|_| w >= 4.0 && spread.is_some())),
                S::SeKurtosis => {
                    number(summary.se_kurtosis.filter(|_| w >= 4.0 && spread.is_some()))
                }
                S::Range => number(summary.max.zip(summary.min).map(|(max, min)| max - min)),
                S::Minimum => number(summary.min),
                S::Maximum => number(summary.max),
                S::Sum => number(summary.sum.filter(|_| w > 0.0)),
            });
        }
        cells.extend(percentiles.iter().map(|&p| percentile(p)));
        (cells, tied)
    }
}
