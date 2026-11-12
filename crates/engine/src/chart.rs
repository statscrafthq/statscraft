//! Charts as data: what a command asks to draw, with its numbers and labels and nothing about
//! colors or layout. Every way of showing a chart starts from this description: the data
//! table of the text output, the Vega-Lite specification for agents (`vega_lite`), and later
//! the office chart and the drawing in the window.
//!
//! Labels follow the rules of tables (`shown.rs`): categories and series after SET TNUMBERS,
//! axis titles after SET TVARS, the chart title after SET OVARS.

use crate::format::format_value;
use crate::{Cell, Table};
use serde::{Deserialize, Serialize};

/// One chart in a command's result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chart {
    pub kind: ChartKind,
    pub tier: Tier,
    /// Bars, lines and areas; None for the kinds without one, such as a pie.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub orientation: Option<Orientation>,
    /// How several series share a category: bars, lines and areas; None otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grouping: Option<Grouping>,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footnote: Option<String>,
    /// The axis of the categories; for a pie, what the slices are.
    pub category_axis: Axis,
    /// The axis of the numbers.
    pub value_axis: Axis,
    /// What tells the series apart, such as the column variable of a clustered bar chart;
    /// None when there is one series.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legend: Option<String>,
    pub categories: Vec<Category>,
    /// Each with one number per category, in the order of `categories`.
    pub series: Vec<Series>,
    /// Histogram: the normal curve drawn over the bars.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal: Option<NormalCurve>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartKind {
    Bar,
    Pie,
    Line,
    Area,
    /// Categories hold the x values (numbers), series the y values.
    Scatter,
    /// Categories are bins with their bounds; one series of counts.
    Histogram,
}

/// Presentation charts go to the office suite as editable charts; statistical plots are drawn
/// precisely and go out as pictures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Presentation,
    Statistical,
}

/// Vertical: categories along the horizontal axis (columns). Horizontal: along the vertical
/// axis (bars).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Orientation {
    Vertical,
    Horizontal,
}

/// Side by side, stacked, or stacked to 100%.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grouping {
    Clustered,
    Stacked,
    Percent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Axis {
    pub title: String,
    /// The display format of its numbers, such as "F8.0" or "PCT8.1"; empty for categories
    /// that are text.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub format: String,
    /// The top of the axis, when the syntax set one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,
    /// The variable it shows, by name as in the data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variable: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Category {
    pub label: String,
    /// The value it stands for; empty for one that groups values, such as all missing ones.
    pub value: Cell,
    /// It holds missing values.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub missing: bool,
    /// Histogram: the bin from its lower bound (included) to its upper bound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<[f64; 2]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Series {
    pub name: String,
    pub statistic: Statistic,
    /// The variable whose category the series is, with that category's value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variable: Option<String>,
    #[serde(default, skip_serializing_if = "is_empty")]
    pub value: Cell,
    pub values: Vec<f64>,
}

/// What the numbers of a series are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Statistic {
    /// Weighted counts of cases.
    Count,
    /// Percentages, 0 to 100.
    Percent,
}

/// A normal curve with the mean and standard deviation of the values, scaled to the bars: at
/// x its height is `total` times the bin width times the normal density.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalCurve {
    pub mean: f64,
    pub std_dev: f64,
    pub total: f64,
}

fn is_empty(cell: &Cell) -> bool {
    *cell == Cell::Empty
}

impl Chart {
    /// What kind of chart it is, in words: "Clustered bar chart".
    pub fn kind_text(&self) -> String {
        let bars = match self.orientation {
            Some(Orientation::Horizontal) => "bar chart (horizontal)",
            _ => "bar chart",
        };
        match (self.kind, self.grouping) {
            (ChartKind::Bar, Some(Grouping::Stacked)) => format!("Stacked {bars}"),
            (ChartKind::Bar, Some(Grouping::Percent)) => format!("100% stacked {bars}"),
            (ChartKind::Bar, _) if self.series.len() > 1 => format!("Clustered {bars}"),
            (ChartKind::Bar, _) => format!("Simple {bars}"),
            (ChartKind::Pie, _) => "Pie chart".into(),
            (ChartKind::Line, _) => "Line chart".into(),
            (ChartKind::Area, Some(Grouping::Percent)) => "100% stacked area chart".into(),
            (ChartKind::Area, Some(Grouping::Stacked)) => "Stacked area chart".into(),
            (ChartKind::Area, _) => "Area chart".into(),
            (ChartKind::Scatter, _) => "Scatter plot".into(),
            (ChartKind::Histogram, _) if self.normal.is_some() => {
                "Histogram with a normal curve".into()
            }
            (ChartKind::Histogram, _) => "Histogram".into(),
        }
    }

    /// The chart's numbers as a table: a row per category, a column per series; the caption
    /// says what kind of chart it is. The text output and the window show charts this way.
    pub fn data_table(&self) -> Table {
        let mut columns = vec![self.category_axis.title.clone()];
        columns.extend(self.series.iter().map(|s| s.name.clone()));
        let rows: Vec<Vec<Cell>> = self
            .categories
            .iter()
            .enumerate()
            .map(|(i, category)| {
                let mut row = vec![Cell::Text(category.label.clone())];
                row.extend(self.series.iter().map(|s| Cell::Number(s.values[i])));
                row
            })
            .collect();
        let format = |r: usize, c: usize| match c {
            0 => String::new(),
            _ => series_format(&self.series[c - 1], &self.value_axis.format, &rows, r, c),
        };
        let formats = crate::format::formats_of(&rows, format);
        let mut caption = format!("{}.", self.kind_text());
        if let Some(footnote) = &self.footnote {
            caption = format!("{caption} {footnote}");
        }
        Table {
            title: self.title.clone(),
            row_headers: 1,
            column_groups: match &self.legend {
                Some(legend) => {
                    let mut groups = vec![Vec::new()];
                    groups.extend(self.series.iter().map(|_| vec![legend.clone()]));
                    groups
                }
                None => vec![Vec::new(); columns.len()],
            },
            columns,
            rows,
            formats,
            caption: Some(caption),
        }
    }
}

/// A series' cells take the value axis format; counts that are not whole numbers get two
/// decimals so that weighted counts are not shown rounded.
fn series_format(series: &Series, axis: &str, rows: &[Vec<Cell>], r: usize, c: usize) -> String {
    match (&rows[r][c], series.statistic) {
        (Cell::Number(n), Statistic::Count) if n.fract() != 0.0 => "F8.2".into(),
        _ => axis.to_string(),
    }
}

/// The display format of counts: whole numbers unless a weight made some fractional.
pub(crate) fn count_format(values: impl IntoIterator<Item = f64>) -> String {
    match values.into_iter().all(|v| v.fract() == 0.0) {
        true => "F8.0".into(),
        false => "F8.2".into(),
    }
}

/// The format of percentages on an axis.
pub(crate) const PERCENT_FORMAT: &str = "PCT8.1";

/// A number in a display format, as a footnote shows it; plain when there is none.
pub(crate) fn number_text(value: f64, format: &str) -> String {
    match format.is_empty() {
        true if value.fract() == 0.0 => format!("{value:.0}"),
        true => format!("{value:.2}"),
        false => format_value(value, format, true),
    }
}
