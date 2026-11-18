//! RESPONSE RATE: the rate of a numerator over a denominator in each group, with cumulative
//! rates and colour-coded limits.
//!
//! Rules, in our words, from the Command Syntax Reference:
//! - Within each value of GROUP the numerator and the denominator are summed; the rate is
//!   100 * numerator sum / denominator sum.
//! - Groups go in ascending order of their value; the cumulative rate of a group divides the
//!   numerator summed over it and all lower groups by the denominator summed the same way.
//! - A group whose denominator sum is 0 or negative is left out of everything.
//! - A case missing on any of the three variables is left out (listwise).
//! - With MINRATE a row is green when its cumulative rate reaches the minimum, with MAXCOUNT
//!   when its cumulative denominator sum stays within the maximum, with both when it does both;
//!   other rows are red. PERCENT is a share of the denominator total of the groups kept.
//!
//! Choices where the manual is silent (docs/differences.md): case weights multiply both
//! values; both limits are inclusive; the table also shows the numerator sums; the two chart
//! panels are two charts; SPLIT FILE runs each split on its own.

use crate::chart::{Axis, Category, Chart, PERCENT_FORMAT, Series, Statistic};
use crate::crosstabs::{Key, categories};
use crate::descriptives::case_weights;
use crate::npar_tests::text;
use crate::twostep::{numeric_value, plain_table};
use crate::{Cell, Decoration, Engine, Error, Table};
use statscraft_syntax::{MaxCount, ResponseRate};

const GREEN: &str = "#c6efce";
const RED: &str = "#ffc7ce";

/// One group's sums.
struct Group {
    key: Key,
    numerator: f64,
    denominator: f64,
}

fn line(name: &str, statistic: Statistic, values: Vec<f64>) -> Series {
    Series {
        name: name.into(),
        statistic,
        variable: None,
        value: Cell::Empty,
        values,
    }
}

impl Engine {
    pub(crate) fn response_rate(&mut self, command: &ResponseRate) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let numerator = data.variable(&command.numerator)?;
        let denominator = data.variable(&command.denominator)?;
        let group = data.variable(&command.group)?;
        for v in [&numerator, &denominator] {
            if v.is_string() {
                return Err(Error(format!(
                    "RESPONSE RATE: {} is a string variable; NUMERATOR and DENOMINATOR must be numeric",
                    v.name()
                )));
            }
        }
        let weights = case_weights(data)?;
        let keys = categories(&group, false);
        let mut cases: Vec<(&Key, f64, f64)> = (0..data.case_count())
            .filter_map(|c| {
                let w = weights.as_ref().map_or(1.0, |w| w[c]);
                let key = keys[c].as_ref()?;
                let n = numeric_value(&numerator, c, false)?;
                let d = numeric_value(&denominator, c, false)?;
                (w > 0.0).then_some((key, w * n, w * d))
            })
            .collect();
        cases.sort_by(|a, b| a.0.compare(b.0));
        let mut groups: Vec<Group> = Vec::new();
        for (key, n, d) in cases {
            match groups.last_mut() {
                Some(g) if g.key.compare(key).is_eq() => {
                    g.numerator += n;
                    g.denominator += d;
                }
                _ => groups.push(Group {
                    key: key.clone(),
                    numerator: n,
                    denominator: d,
                }),
            }
        }
        let shown = self.shown();
        let mut warnings = Vec::new();
        let dropped: Vec<String> = groups
            .iter()
            .filter(|g| g.denominator <= 0.0)
            .map(|g| shown.value(&group, &g.key))
            .collect();
        if !dropped.is_empty() {
            warnings.push(format!(
                "RESPONSE RATE leaves out the groups of {} whose sum of {} is 0 or negative: {}",
                group.name(),
                denominator.name(),
                dropped.join(", ")
            ));
        }
        groups.retain(|g| g.denominator > 0.0);
        if groups.is_empty() {
            return Err(Error(format!(
                "RESPONSE RATE has no group with a positive sum of {}",
                denominator.name()
            )));
        }

        let total: f64 = groups.iter().map(|g| g.denominator).sum();
        let max_count = command.max_count.map(|m| match m {
            MaxCount::Count(n) => n,
            MaxCount::Percent(p) => p / 100.0 * total,
        });
        let group_title = shown.variable(&group);
        let num_title = shown.variable(&numerator);
        let den_title = shown.variable(&denominator);
        let labels: Vec<String> = groups.iter().map(|g| shown.value(&group, &g.key)).collect();

        let (mut cum_n, mut cum_d) = (0.0, 0.0);
        let mut rates = Vec::new();
        let mut cum_rates = Vec::new();
        let mut cum_counts = Vec::new();
        let mut rows = Vec::new();
        let mut decorations = Vec::new();
        for (r, (g, label)) in groups.iter().zip(&labels).enumerate() {
            cum_n += g.numerator;
            cum_d += g.denominator;
            let rate = 100.0 * g.numerator / g.denominator;
            let cum_rate = 100.0 * cum_n / cum_d;
            rows.push(vec![
                text(label),
                Cell::Number(g.numerator),
                Cell::Number(g.denominator),
                Cell::Number(rate),
                Cell::Number(cum_n),
                Cell::Number(cum_d),
                Cell::Number(cum_rate),
            ]);
            if command.min_rate.is_some() || max_count.is_some() {
                let keeps = command.min_rate.is_none_or(|m| cum_rate >= m)
                    && max_count.is_none_or(|m| cum_d <= m);
                let fill = if keeps { GREEN } else { RED };
                decorations.extend((0..7).map(|col| Decoration {
                    row: r,
                    col,
                    fill: Some(fill.into()),
                    ..Decoration::default()
                }));
            }
            rates.push(rate);
            cum_rates.push(cum_rate);
            cum_counts.push(cum_d);
        }
        let columns = [
            group_title.clone(),
            format!("Sum of {num_title}"),
            format!("Sum of {den_title}"),
            "Response Rate".into(),
            format!("Cumulative Sum of {num_title}"),
            format!("Cumulative Sum of {den_title}"),
            "Cumulative Response Rate".into(),
        ];
        let mut table = plain_table("Response Rates", 1, &columns, rows);
        let pct = |c: usize| matches!(c, 3 | 6);
        table.formats = vec![
            (0..7)
                .map(|c| if pct(c) {
                    "PCT8.2".into()
                } else {
                    String::new()
                })
                .collect();
            groups.len()
        ];
        let mut limits = Vec::new();
        if let Some(m) = command.min_rate {
            limits.push(format!("cumulative response rate at least {}%", round4(m)));
        }
        if let Some(m) = max_count {
            limits.push(format!(
                "cumulative sum of {den_title} at most {}",
                round4(m)
            ));
        }
        if !limits.is_empty() {
            table.caption = Some(format!(
                "Green rows keep the limits ({}); red rows do not.",
                limits.join(", ")
            ));
        }
        table.decorations = decorations;

        let categories: Vec<Category> = groups
            .iter()
            .zip(&labels)
            .map(|(g, label)| Category {
                label: label.clone(),
                value: match &g.key {
                    Key::Number(n) => Cell::Number(*n),
                    Key::Text(t) => Cell::Text(t.clone()),
                },
                missing: false,
                bin: None,
            })
            .collect();
        let x = Axis {
            variable: Some(group.name().to_string()),
            ..Axis::titled(group_title.clone())
        };
        let k = groups.len();
        let mut series = vec![
            line("Response rate", Statistic::Percent, rates),
            line("Cumulative response rate", Statistic::Percent, cum_rates),
        ];
        if let Some(m) = command.min_rate {
            series.push(line("Minimum rate", Statistic::Percent, vec![m; k]));
        }
        let mut upper = Chart::lines(
            format!("Response rates by {group_title}"),
            x.clone(),
            Axis {
                format: PERCENT_FORMAT.into(),
                ..Axis::titled("Response rate")
            },
            categories.clone(),
            series,
        );
        upper.legend = Some("Series".into());
        let mut series = vec![line(
            &format!("Cumulative sum of {den_title}"),
            Statistic::Value,
            cum_counts,
        )];
        if let Some(m) = max_count {
            series.push(line("Maximum count", Statistic::Value, vec![m; k]));
        }
        let legend = (series.len() > 1).then(|| "Series".to_string());
        let mut lower = Chart::lines(
            format!("Cumulative sum of {den_title} by {group_title}"),
            x,
            Axis {
                variable: Some(denominator.name().to_string()),
                ..Axis::titled(format!("Cumulative sum of {den_title}"))
            },
            categories,
            series,
        );
        lower.legend = legend;

        for warning in warnings {
            self.warn(warning);
        }
        self.charts.extend([upper, lower]);
        Ok(vec![table])
    }
}

fn round4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}
