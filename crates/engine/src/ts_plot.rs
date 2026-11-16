//! TSPLOT and CASEPLOT: time series as charts, the cases along the horizontal axis (TSPLOT) or
//! down the vertical one (CASEPLOT).
//!
//! Rules noted from the Command Syntax Reference:
//! - Each series may be logged (values not above 0 become missing), then differenced DIFF
//!   times and over the period SDIFF times; without a period SDIFF is not done.
//! - The cases are labeled by ID (else TSET ID), else DATE_, else their numbers.
//! - One series: NOFILL or a filled area (left out when a value is missing or below 0) and an
//!   optional reference line at a value or at the mean. Several: NOJOIN, JOIN (a line across
//!   the values of each case) or HILO (the highest and lowest value of each case, at most
//!   three series).
//! - MARK: a line where a variable changes its value, or at a date.
//!
//! Choices where the manual is silent:
//! - The charts are data: a line, area, drop-line or high-low chart; a reference line is one
//!   more series of a single value, and marks are named in the footnote.
//! - SPLIT=UNIFORM and SCALE both put the reference line at the mean of the group; the
//!   common scale of UNIFORM is left to the drawing.

use crate::chart::{Axis, Category, ChartKind, Orientation, Series, Statistic};
use crate::timeseries::{Values, read_series};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_syntax::{PlotJoin, PlotMark, PlotReference, SeriesPlot};

/// `x` differenced at `lag`.
fn difference(x: &[Option<f64>], lag: usize) -> Values {
    (0..x.len())
        .map(|i| Some(x[i]? - x[i.checked_sub(lag)?]?))
        .collect()
}

impl Engine {
    /// The command over the previous one of its kind for APPLY; keeps it.
    pub(crate) fn series_plot_spec(&mut self, command: &SeriesPlot) -> Result<SeriesPlot, Error> {
        let name = if command.casewise {
            "CASEPLOT"
        } else {
            "TSPLOT"
        };
        let slot = usize::from(command.casewise);
        let mut given = match &command.apply {
            None => command.clone(),
            Some(Some(model)) => {
                return Err(Error(format!(
                    "{name} APPLY of the named model '{model}' is not supported yet; APPLY \
                     without a name uses the previous {name}"
                )));
            }
            Some(None) => {
                let Some(previous) = &self.ts_models.plots[slot] else {
                    return Err(Error(format!(
                        "{name} APPLY: there is no earlier {name} to apply"
                    )));
                };
                let mut given = (**previous).clone();
                if !command.variables.is_empty() {
                    given.variables.clone_from(&command.variables);
                }
                macro_rules! over {
                    ($($field:ident),*) => {
                        $(if command.$field.is_some() {
                            given.$field = command.$field.clone();
                        })*
                    };
                }
                over!(
                    diff,
                    sdiff,
                    period,
                    ln,
                    id,
                    mark,
                    split_scale,
                    fill,
                    reference,
                    join
                );
                given
            }
        };
        given.apply = None;
        self.ts_models.plots[slot] = Some(Box::new(given.clone()));
        Ok(given)
    }

    pub(crate) fn series_plot(&mut self, spec: &SeriesPlot) -> Result<Vec<Table>, Error> {
        let name = if spec.casewise { "CASEPLOT" } else { "TSPLOT" };
        let data = self.active()?;
        let shown = self.shown();
        let names = self.expand(&spec.variables)?;
        let include = self.tset.include;
        let cases = data.case_count();
        let mut warnings = Vec::new();
        let ln = spec.ln.unwrap_or(false);
        let diff = spec.diff.unwrap_or(0) as usize;
        let mut sdiff = spec.sdiff.unwrap_or(0) as usize;
        let period = self.ts_period(spec.period);
        if sdiff > 0 && period.is_none() {
            warnings.push(format!(
                "{name}: no period is set (PERIOD, TSET PERIOD or DATE), so SDIFF is left out"
            ));
            sdiff = 0;
        }
        let mut series = Vec::new();
        for n in &names {
            let (mut x, _, _) = read_series(data, n, include, name)?;
            if ln {
                x = x
                    .iter()
                    .map(|v| v.filter(|v| *v > 0.0).map(f64::ln))
                    .collect();
            }
            for _ in 0..diff {
                x = difference(&x, 1);
            }
            for _ in 0..sdiff {
                x = difference(&x, period.expect("checked"));
            }
            series.push((shown.named(data, n), x));
        }
        // the case labels: ID, else DATE_, else the case numbers of the file
        let id = spec
            .id
            .clone()
            .or_else(|| self.tset.id.clone().filter(|id| data.variable(id).is_ok()));
        let label_variable = match &id {
            Some(id) => Some(data.variable(id)?),
            None => data.variable("DATE_").ok(),
        };
        let number = |k: usize| self.case_numbers.as_ref().map_or(k, |c| c[k]) + 1;
        let labels: Vec<String> = (0..cases)
            .map(|k| match &label_variable {
                Some(v) => crate::cluster::value_text(v, k).trim().to_string(),
                None => number(k).to_string(),
            })
            .collect();
        let axis_title = match (&id, &label_variable) {
            (Some(id), _) => shown.named(data, id),
            (None, Some(_)) => "Date".to_string(),
            (None, None) => "Case Number".to_string(),
        };
        let categories: Vec<Category> = (0..cases)
            .map(|k| Category {
                label: labels[k].clone(),
                value: Cell::Number(number(k) as f64),
                missing: false,
                bin: None,
            })
            .collect();

        // the marks
        let mut marks = Vec::new();
        match &spec.mark {
            None => {}
            Some(PlotMark::Variable(v)) => {
                let (m, _, _) = read_series(data, v, true, name)?;
                for k in 1..cases {
                    if m[k] != m[k - 1] {
                        marks.push(labels[k].clone());
                    }
                }
            }
            Some(PlotMark::Date(parts)) => {
                let columns: Vec<(Values, f64)> = parts
                    .iter()
                    .map(|(unit, value)| {
                        let (c, _, _) = read_series(data, &format!("{}_", unit.name()), true, name)
                            .map_err(|_| {
                                Error(format!(
                                    "{name}: MARK names {}, but there is no date variable \
                                         {}_; define the dates with DATE",
                                    unit.name(),
                                    unit.name()
                                ))
                            })?;
                        Ok((c, *value))
                    })
                    .collect::<Result<_, Error>>()?;
                match (0..cases).find(|&k| columns.iter().all(|(c, v)| c[k] == Some(*v))) {
                    Some(k) => marks.push(labels[k].clone()),
                    None => warnings.push(format!("{name}: no case has the date of MARK")),
                }
            }
        }

        let values = |x: &Values| -> Vec<f64> { x.iter().map(|v| v.unwrap_or(f64::NAN)).collect() };
        let one = |title: &str, v: Vec<f64>| Series {
            name: title.into(),
            statistic: Statistic::Value,
            variable: None,
            value: Cell::Empty,
            values: v,
        };
        let join = spec.join.unwrap_or(PlotJoin::NoJoin);
        let (kind, mut chart_series) = if series.len() == 1 {
            let (title, x) = &series[0];
            let fill = spec.fill.unwrap_or(false);
            let fillable = x.iter().all(|v| v.is_some_and(|v| v >= 0.0));
            if fill && !fillable {
                warnings.push(format!(
                    "{name}: {title} has missing or negative values, so the area is not filled"
                ));
            }
            let kind = if fill && fillable {
                ChartKind::Area
            } else {
                ChartKind::Line
            };
            (kind, vec![one(title, values(x))])
        } else {
            match join {
                PlotJoin::Hilo if series.len() <= 3 => {
                    let pick = |better: fn(f64, f64) -> f64| -> Vec<f64> {
                        (0..cases)
                            .map(|k| {
                                let v: Vec<f64> = series.iter().filter_map(|s| s.1[k]).collect();
                                v.into_iter().reduce(better).unwrap_or(f64::NAN)
                            })
                            .collect()
                    };
                    (
                        ChartKind::HighLow,
                        vec![
                            one("Highest", pick(f64::max)),
                            one("Lowest", pick(f64::min)),
                        ],
                    )
                }
                PlotJoin::Join => (
                    ChartKind::DropLine,
                    series.iter().map(|(t, x)| one(t, values(x))).collect(),
                ),
                _ => (
                    ChartKind::Line,
                    series.iter().map(|(t, x)| one(t, values(x))).collect(),
                ),
            }
        };
        // a reference line goes with one series only
        let reference = match (spec.reference, series.len()) {
            (Some(PlotReference::Value(v)), 1) => Some(v),
            (Some(PlotReference::Mean), 1) => {
                let valid: Vec<f64> = series[0].1.iter().flatten().copied().collect();
                (!valid.is_empty()).then(|| valid.iter().sum::<f64>() / valid.len() as f64)
            }
            _ => None,
        };
        if let Some(r) = reference {
            chart_series.push(one("Reference line", vec![r; cases]));
        }
        let title = series
            .iter()
            .map(|s| s.0.clone())
            .collect::<Vec<_>>()
            .join(", ");
        let mut notes = Vec::new();
        let mut steps = Vec::new();
        if ln {
            steps.push("natural log".to_string());
        }
        if diff > 0 {
            steps.push(format!("difference ({diff})"));
        }
        if sdiff > 0 {
            steps.push(format!(
                "seasonal difference ({sdiff}, period {})",
                period.expect("checked")
            ));
        }
        if !steps.is_empty() {
            notes.push(format!("Transformations: {}.", steps.join(", ")));
        }
        if !marks.is_empty() {
            notes.push(format!("Marked at {}.", marks.join(", ")));
        }
        let value_title = if series.len() == 1 {
            series[0].0.clone()
        } else {
            "Value".to_string()
        };
        let mut chart = Chart::lines(
            title,
            Axis::titled(axis_title),
            Axis::titled(value_title),
            categories,
            chart_series,
        );
        chart.kind = kind;
        chart.orientation = Some(if spec.casewise {
            Orientation::Horizontal
        } else {
            Orientation::Vertical
        });
        chart.footnote = (!notes.is_empty()).then(|| notes.join(" "));
        if chart.series.len() > 1 {
            chart.legend = Some("Series".into());
        }
        for w in warnings {
            self.warn(w);
        }
        self.charts.push(chart);
        Ok(Vec::new())
    }
}
