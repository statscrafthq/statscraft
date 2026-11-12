//! ROC: the receiver operating characteristic curve of test variables against a state
//! variable, with the area under it.
//!
//! Rules from the Command Syntax Reference (ROC) and the Algorithms manual (ROC Algorithms):
//! - A case counts when its state and every test variable are valid (listwise); MISSING=INCLUDE
//!   makes user-missing values valid, system-missing never is. The cases whose state equals
//!   the given value are positive, all other valid states negative. Weights count cases.
//! - Cutoffs: the smallest test value minus 1, the midpoints of neighboring distinct values
//!   and the largest plus 1. With TESTPOS(LARGE) a value at or above the cutoff tests
//!   positive (above it with CUTOFF(EXCLUDE)); with SMALL at or below (below). Sensitivity is
//!   the share of positive cases that test positive, 1 - specificity the share of negative
//!   ones.
//! - Area W: over all pairs of a positive and a negative case, 1 when the positive one has
//!   the larger value, 1/2 for a tie, divided by n+ n-. With TESTPOS(SMALL) the area is 1 - W.
//! - Standard error sqrt((W(1 - W) + (n+ - 1)(Q1 - W^2) + (n- - 1)(Q2 - W^2)) / (n+ n-)), with
//!   Q1 the chance that two positive cases both beat one negative and Q2 that one positive
//!   beats two negatives, counted with ties as halves and thirds. DISTRIBUTION(NEGEXPO) with
//!   as many positive as negative cases takes Q1 = W / (2 - W), Q2 = 2 W^2 / (1 + W); with
//!   unequal groups it falls back to the nonparametric error and says so.
//! - Significance: two-sided normal test of area 0.5 with the null error
//!   sqrt((n+ + n- + 1) / (12 n+ n-)). Interval: area plus and minus z times the error.

use crate::chart::{Axis, Category, Chart, ChartKind, Orientation, Series, Statistic};
use crate::crosstabs::Key;
use crate::descriptives::{case_weights, number};
use crate::dist::{normal_p, normal_quantile};
use crate::means::is_valid;
use crate::plot::{Plot, Point, PointGroup, ReferenceLine};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Values;
use statscraft_syntax::{Roc, Value};

/// The area and its statistics for one test variable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Area {
    pub area: f64,
    pub se: f64,
    pub sig: f64,
    /// The bi-negative exponential error was asked for but the groups differ in size.
    pub fell_back: bool,
}

/// The area under the curve of (value, weight, positive) cases, larger values pointing to the
/// positive state; `negexpo` asks for the bi-negative exponential standard error.
pub(crate) fn area(cases: &[(f64, f64, bool)], negexpo: bool) -> Area {
    let mut sorted = cases.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n_pos: f64 = cases.iter().filter(|c| c.2).map(|c| c.1).sum();
    let n_neg: f64 = cases.iter().filter(|c| !c.2).map(|c| c.1).sum();
    // per distinct value: positive and negative weight at it
    let mut levels: Vec<(f64, f64)> = Vec::new();
    let mut start = 0;
    while start < sorted.len() {
        let end = start
            + sorted[start..]
                .iter()
                .take_while(|c| c.0 == sorted[start].0)
                .count();
        let at = |positive: bool| -> f64 {
            sorted[start..end]
                .iter()
                .filter(|c| c.2 == positive)
                .map(|c| c.1)
                .sum()
        };
        levels.push((at(true), at(false)));
        start = end;
    }
    let (mut w, mut q1, mut q2) = (0.0, 0.0, 0.0);
    let mut neg_below = 0.0;
    let mut pos_above = n_pos;
    for &(pos, neg) in &levels {
        pos_above -= pos;
        w += neg * (pos_above + pos / 2.0);
        q1 += neg * (pos_above * pos_above + pos_above * pos + pos * pos / 3.0);
        q2 += pos * (neg_below * neg_below + neg_below * neg + neg * neg / 3.0);
        neg_below += neg;
    }
    let w = w / (n_pos * n_neg);
    let (mut q1, mut q2) = (q1 / (n_neg * n_pos * n_pos), q2 / (n_pos * n_neg * n_neg));
    let fell_back = negexpo && n_pos != n_neg;
    if negexpo && !fell_back {
        q1 = w / (2.0 - w);
        q2 = 2.0 * w * w / (1.0 + w);
    }
    let variance = (w * (1.0 - w) + (n_pos - 1.0) * (q1 - w * w) + (n_neg - 1.0) * (q2 - w * w))
        / (n_pos * n_neg);
    let null_sd = ((n_pos + n_neg + 1.0) / (12.0 * n_pos * n_neg)).sqrt();
    Area {
        area: w,
        se: variance.max(0.0).sqrt(),
        sig: normal_p((w - 0.5) / null_sd),
        fell_back,
    }
}

/// The cutoffs of the coordinates table: below the smallest value, between neighbors, above
/// the largest.
pub(crate) fn cutoffs(values: &[f64]) -> Vec<f64> {
    let mut distinct = values.to_vec();
    distinct.sort_by(f64::total_cmp);
    distinct.dedup();
    let (Some(&lo), Some(&hi)) = (distinct.first(), distinct.last()) else {
        return Vec::new();
    };
    let mut out = vec![lo - 1.0];
    out.extend(distinct.windows(2).map(|w| (w[0] + w[1]) / 2.0));
    out.push(hi + 1.0);
    out
}

impl Engine {
    pub(crate) fn roc(&mut self, command: &Roc) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let names = self.expand(&command.tests)?;
        let state = data.variable(&command.state)?;
        let mut tests = Vec::new();
        for name in &names {
            let variable = data.variable(name)?;
            if variable.is_string() {
                return Err(Error(format!(
                    "ROC needs numeric test variables, but {name} is a string variable"
                )));
            }
            tests.push(variable);
        }
        let positive_key = match (&command.positive, state.values()) {
            (Value::Number(n), Values::Numeric(_)) => Key::Number(*n),
            (Value::String(s), Values::String(_)) => Key::Text(s.trim_end().to_string()),
            _ => {
                return Err(Error(format!(
                    "the positive state value must have the type of {}",
                    command.state
                )));
            }
        };
        let is_positive = |c: usize| match (state.values(), &positive_key) {
            (Values::Numeric(v), Key::Number(n)) => v.get(c) == Some(*n),
            (Values::String(v), Key::Text(t)) => v.get(c).trim_end() == t,
            _ => false,
        };
        let weights = case_weights(data)?;
        let weight = |c: usize| weights.as_ref().map_or(1.0, |w| w[c]);
        let (mut used, mut missing) = (Vec::new(), 0.0);
        for c in (0..data.case_count()).filter(|&c| weight(c) > 0.0) {
            let valid = is_valid(&state, c, command.include)
                && tests.iter().all(|t| is_valid(t, c, command.include));
            if valid {
                used.push(c);
            } else {
                missing += weight(c);
            }
        }
        let n_pos: f64 = used
            .iter()
            .filter(|&&c| is_positive(c))
            .map(|&c| weight(c))
            .sum();
        let n_neg: f64 = used
            .iter()
            .filter(|&&c| !is_positive(c))
            .map(|&c| weight(c))
            .sum();
        if n_pos == 0.0 || n_neg == 0.0 {
            return Err(Error(format!(
                "ROC needs valid cases in both states, but there are {n_pos} positive and \
                 {n_neg} negative cases"
            )));
        }
        let state_text = shown.value(&state, &positive_key);
        let direction = match command.small_positive {
            false => "Larger",
            true => "Smaller",
        };
        let mut tables = vec![Table {
            title: "Case Processing Summary".into(),
            row_headers: 1,
            columns: vec![shown.variable(&state), "Valid N (listwise)".into()],
            rows: vec![
                vec![Cell::Text("Positive".into()), Cell::Number(n_pos)],
                vec![Cell::Text("Negative".into()), Cell::Number(n_neg)],
                vec![Cell::Text("Missing".into()), Cell::Number(missing)],
            ],
            caption: Some(format!(
                "{direction} test values point to the positive state, which is {} = {state_text}.",
                shown.variable(&state)
            )),
            ..Table::default()
        }];

        let value = |t: &statscraft_data::Variable, c: usize| match t.values() {
            Values::Numeric(v) => v.get(c).unwrap_or(f64::NAN),
            Values::String(_) => f64::NAN,
        };
        let z = normal_quantile(0.5 + command.ci / 200.0);
        let mut area_rows = Vec::new();
        let mut coordinate_rows = Vec::new();
        let mut groups = Vec::new();
        let (mut tied, mut fell_back) = (Vec::new(), false);
        let mut lower_bounds = Vec::new();
        let mut charts = Vec::new();
        for test in &tests {
            let label = shown.variable(test);
            let cases: Vec<(f64, f64, bool)> = used
                .iter()
                .map(|&c| (value(test, c), weight(c), is_positive(c)))
                .collect();
            // both directions share the error; SMALL turns the area around
            let mut a = area(&cases, command.negexpo);
            if command.small_positive {
                a.area = 1.0 - a.area;
            }
            fell_back |= a.fell_back;
            let ties = cases
                .iter()
                .any(|p| p.2 && cases.iter().any(|q| !q.2 && q.0 == p.0));
            if ties {
                tied.push(label.clone());
            }
            let mut row = vec![Cell::Text(label.clone()), Cell::Number(a.area)];
            if command.se {
                let (lo, hi) = (a.area - z * a.se, a.area + z * a.se);
                row.extend([number(Some(a.se)), number(Some(a.sig)), number(Some(lo))]);
                row.push(number(Some(hi)));
                lower_bounds.push(lo);
            }
            area_rows.push(row);
            // the curve from (0, 0) to (1, 1) and the table in ascending cutoffs
            let values: Vec<f64> = cases.iter().map(|c| c.0).collect();
            let mut points = Vec::new();
            for (k, &cut) in cutoffs(&values).iter().enumerate() {
                let tests_positive = |x: f64| match (command.small_positive, command.exclude_cutoff)
                {
                    (false, false) => x >= cut,
                    (false, true) => x > cut,
                    (true, false) => x <= cut,
                    (true, true) => x < cut,
                };
                let share = |positive: bool, total: f64| -> f64 {
                    // 0.0 + keeps an empty sum (-0) from showing as -0
                    0.0 + cases
                        .iter()
                        .filter(|c| c.2 == positive && tests_positive(c.0))
                        .map(|c| c.1)
                        .sum::<f64>()
                        / total
                };
                let (sensitivity, fpr) = (share(true, n_pos), share(false, n_neg));
                let name = if k == 0 { label.clone() } else { String::new() };
                coordinate_rows.push(vec![
                    Cell::Text(name),
                    Cell::Number(cut),
                    Cell::Number(sensitivity),
                    Cell::Number(fpr),
                ]);
                points.push(Point {
                    x: fpr,
                    y: sensitivity,
                    z: None,
                    label: None,
                });
            }
            if !command.small_positive {
                points.reverse();
            }
            groups.push(PointGroup {
                name: label,
                points,
            });
        }

        let tie_note = (!tied.is_empty()).then(|| {
            format!(
                "{}: some test values occur in both the positive and the negative group, which \
                 may bias the statistics.",
                tied.join(", ")
            )
        });
        let mut columns: Vec<String> = ["Test Result Variable(s)", "Area"].map(String::from).into();
        let mut column_groups = vec![Vec::new(); 2];
        let mut notes: Vec<String> = tie_note.iter().cloned().collect();
        if command.se {
            let level = format!("Asymptotic {}% Confidence Interval", short(command.ci));
            columns.extend(
                [
                    "Std. Error",
                    "Asymptotic Sig.",
                    "Lower Bound",
                    "Upper Bound",
                ]
                .map(String::from),
            );
            column_groups.extend([vec![], vec![], vec![level.clone()], vec![level]]);
            notes.push(match (command.negexpo, fell_back) {
                (false, _) => "The standard error assumes no distribution.".to_string(),
                (true, false) => {
                    "The standard error assumes bi-negative exponential distributions.".into()
                }
                (true, true) => "The standard error assumes bi-negative exponential \
                    distributions where the positive and negative groups are equally large; \
                    for the other variables it assumes no distribution."
                    .into(),
            });
            notes.push("The significance tests the null hypothesis that the area is 0.5.".into());
        }
        tables.push(Table {
            title: "Area Under the Curve".into(),
            row_headers: 1,
            columns,
            column_groups,
            rows: area_rows,
            caption: (!notes.is_empty()).then(|| notes.join(" ")),
            ..Table::default()
        });
        if command.coordinates {
            let rule = match (command.small_positive, command.exclude_cutoff) {
                (false, false) => "Positive if Greater Than or Equal To",
                (false, true) => "Positive if Greater Than",
                (true, false) => "Positive if Less Than or Equal To",
                (true, true) => "Positive if Less Than",
            };
            let mut notes: Vec<String> = tie_note.iter().cloned().collect();
            notes.push(
                "The cutoffs run from 1 below the smallest test value to 1 above the largest; \
                 the others lie halfway between neighboring test values."
                    .into(),
            );
            tables.push(Table {
                title: "Coordinates of the Curve".into(),
                row_headers: 1,
                columns: [
                    "Test Result Variable(s)",
                    rule,
                    "Sensitivity",
                    "1 - Specificity",
                ]
                .map(String::from)
                .into(),
                rows: coordinate_rows,
                caption: Some(notes.join(" ")),
                ..Table::default()
            });
        }
        if command.curve {
            let several = groups.len() > 1;
            let mut chart = Chart::plot(
                ChartKind::Roc,
                "ROC curve".into(),
                Axis::titled("1 - Specificity"),
                Axis::titled("Sensitivity"),
                Plot::Points {
                    groups,
                    lines: match command.reference {
                        true => vec![ReferenceLine {
                            intercept: 0.0,
                            slope: 1.0,
                        }],
                        false => Vec::new(),
                    },
                    z_axis: None,
                },
            );
            if several {
                chart.legend = Some("Test variable".into());
            }
            if !tied.is_empty() {
                chart.footnote =
                    Some("A diagonal step comes from values tied between the groups".into());
            }
            charts.push(chart);
        }
        if command.model_quality {
            let mut chart = Chart::lines(
                "Overall Model Quality".into(),
                Axis::titled("Test variable"),
                Axis {
                    maximum: Some(1.0),
                    ..Axis::titled("Area: lower confidence bound")
                },
                names
                    .iter()
                    .zip(&tests)
                    .map(|(_, t)| Category {
                        label: shown.variable(t),
                        value: Cell::Empty,
                        missing: false,
                        bin: None,
                    })
                    .collect(),
                vec![Series {
                    name: "Lower bound".into(),
                    statistic: Statistic::Value,
                    variable: None,
                    value: Cell::Empty,
                    values: lower_bounds.iter().map(|v| v.max(0.0)).collect(),
                }],
            );
            chart.kind = ChartKind::Bar;
            chart.orientation = Some(Orientation::Horizontal);
            chart.footnote = Some(
                "Above 0.5 the test does better than chance; at or below 0.5 it does not".into(),
            );
            charts.push(chart);
        }
        self.charts.extend(charts);
        Ok(tables)
    }
}

/// A confidence level in a header: 95, 99.5.
fn short(v: f64) -> String {
    let text = format!("{v:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_counts_pairs_with_ties_as_halves() {
        // positives 3, 5; negatives 1, 3: pairs 3>1, 3=3, 5>1, 5>3 give 3.5 of 4
        let cases = [
            (3.0, 1.0, true),
            (5.0, 1.0, true),
            (1.0, 1.0, false),
            (3.0, 1.0, false),
        ];
        let a = area(&cases, false);
        assert!((a.area - 0.875).abs() < 1e-12);
        // a weight of 2 on the positive 5 is the same as two cases
        let weighted = [
            (3.0, 1.0, true),
            (5.0, 2.0, true),
            (1.0, 1.0, false),
            (3.0, 1.0, false),
        ];
        let repeated = [
            (3.0, 1.0, true),
            (5.0, 1.0, true),
            (5.0, 1.0, true),
            (1.0, 1.0, false),
            (3.0, 1.0, false),
        ];
        assert_eq!(area(&weighted, false), area(&repeated, false));
        assert_eq!(cutoffs(&[3.0, 1.0, 3.0, 5.0]), [0.0, 2.0, 4.0, 6.0]);
    }
}
