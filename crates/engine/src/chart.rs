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
    /// A line under the title.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
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
    /// Bars and lines: an error bar around each value, such as a confidence interval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_bars: Option<ErrorBars>,
    /// Bars: a second value axis on the right for the last series, which is drawn as a line
    /// with points over the bars, such as the cumulative percentage of a Pareto chart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_axis: Option<Axis>,
    /// Statistical plots other than the histogram: their data in its own shape; categories
    /// and series are then empty, and the category axis is the horizontal one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plot: Option<crate::plot::Plot>,
    /// The label in the window's outline when OUTPUT MODIFY gave one; else the title shows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline: Option<String>,
    /// Hidden by OUTPUT MODIFY, as a table can be.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
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
    /// A point at each value with its error bar; several series side by side as bars are.
    ErrorBar,
    /// A floating bar between each pair of series: the first and second, the third and
    /// fourth, and so on.
    RangeBar,
    /// A point at each value and a line from the lowest to the highest value of each
    /// category.
    DropLine,
    /// The same marks for high and low values and an optional close, which is the third
    /// series.
    HighLow,
    /// Two lines with the area between them filled.
    Difference,
    /// Categories are bins with their bounds; one series of counts.
    Histogram,
    /// The kinds below carry their data in `plot`. Boxes.
    Boxplot,
    /// A stem-and-leaf display.
    StemLeaf,
    /// Points: observed values against the expected normal scores, with the line of the
    /// normal distribution.
    NormalQq,
    /// Points: observed values against their deviation from the normal.
    DetrendedNormalQq,
    /// Points: the observed cumulative probability against the expected one, with the
    /// diagonal.
    NormalPp,
    /// Points: observed values against the expected quantiles of a test distribution, with
    /// the diagonal.
    Qq,
    /// Points: observed values against their deviation from the expected quantiles.
    DetrendedQq,
    /// Points: the observed cumulative proportion against the expected one of a test
    /// distribution, with the diagonal.
    Pp,
    /// Points: the observed cumulative proportion against its deviation from the expected one.
    DetrendedPp,
    /// Points joined in order, one line per group: sensitivity against 1 - specificity.
    Roc,
    /// Points: the spread of every cell against its level, with the regression line.
    SpreadLevel,
    /// Merges: the clustering as a tree with heights.
    Dendrogram,
    /// Merges: the clusters at every number of clusters.
    Icicle,
    /// A tree of nodes.
    TreeDiagram,
    /// Regions: the plane of two discriminant functions split into the territory of each
    /// group.
    TerritorialMap,
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
    #[serde(deserialize_with = "numbers")]
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
    /// Means of a variable.
    Mean,
    /// Any other number, such as an eigenvalue.
    Value,
}

/// A normal curve with the mean and standard deviation of the values, scaled to the bars: at
/// x its height is `total` times the bin width times the normal density.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalCurve {
    #[serde(deserialize_with = "number")]
    pub mean: f64,
    #[serde(deserialize_with = "number")]
    pub std_dev: f64,
    #[serde(deserialize_with = "number")]
    pub total: f64,
}

/// Error bars around the values of the series, the same length on both sides.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ErrorBars {
    /// What a bar spans, such as "95% CI" or "2 SE".
    pub label: String,
    /// For each series in order, the half length of the bar at each category: the bar goes
    /// from the value less it to the value plus it. Empty for a series without bars, such as
    /// a reference line; null where there is no bar.
    #[serde(deserialize_with = "number_lists")]
    pub margins: Vec<Vec<f64>>,
    /// For a bar that reaches less far below the value than above it, such as the interval of
    /// a median: per series the length below, laid out as `margins`, which is then the length
    /// above. Empty for a series whose bars are symmetric.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        deserialize_with = "number_lists"
    )]
    pub lower: Vec<Vec<f64>>,
}

fn is_empty(cell: &Cell) -> bool {
    *cell == Cell::Empty
}

/// A number that is not finite has no JSON form: it is written as null, which reads back as
/// NaN, so a saved output file with such a chart opens again.
fn number<'de, D: serde::Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(d)?.unwrap_or(f64::NAN))
}

pub(crate) fn numbers<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<f64>, D::Error> {
    let values = Vec::<Option<f64>>::deserialize(d)?;
    Ok(values.into_iter().map(|v| v.unwrap_or(f64::NAN)).collect())
}

fn number_lists<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Vec<f64>>, D::Error> {
    let lists = Vec::<Vec<Option<f64>>>::deserialize(d)?;
    Ok(lists
        .into_iter()
        .map(|l| l.into_iter().map(|v| v.unwrap_or(f64::NAN)).collect())
        .collect())
}

impl Chart {
    /// The error bar of series `j` at category `i`, from its lower to its upper end; None
    /// where there is none.
    pub fn error_bar(&self, j: usize, i: usize) -> Option<[f64; 2]> {
        let bars = self.error_bars.as_ref()?;
        let margin = *bars.margins.get(j)?.get(i)?;
        let below = match bars.lower.get(j).filter(|l| !l.is_empty()) {
            Some(lower) => *lower.get(i)?,
            None => margin,
        };
        let value = *self.series.get(j)?.values.get(i)?;
        (value.is_finite() && margin.is_finite() && below.is_finite())
            .then_some([value - below, value + margin])
    }

    /// What kind of chart it is, in words: "Clustered bar chart".
    pub fn kind_text(&self) -> String {
        let bars = match self.orientation {
            Some(Orientation::Horizontal) => "bar chart (horizontal)",
            _ => "bar chart",
        };
        match &self.plot {
            Some(crate::plot::Plot::Matrix { .. }) => return "Scatterplot matrix".into(),
            Some(crate::plot::Plot::Points {
                z_axis: Some(_), ..
            }) if self.kind == ChartKind::Scatter => {
                return "Scatter plot in three dimensions".into();
            }
            _ => {}
        }
        if self.kind == ChartKind::Bar && self.second_axis.is_some() {
            let kind = match self.grouping {
                Some(Grouping::Stacked) => "Stacked bar chart",
                _ => "Bar chart",
            };
            return format!("{kind} with a line on a second axis");
        }
        match (self.kind, self.grouping) {
            (ChartKind::Bar, Some(Grouping::Stacked)) => format!("Stacked {bars}"),
            (ChartKind::Bar, Some(Grouping::Percent)) => format!("100% stacked {bars}"),
            (ChartKind::Bar, _) if self.series.len() > 1 => format!("Clustered {bars}"),
            (ChartKind::Bar, _) => format!("Simple {bars}"),
            (ChartKind::ErrorBar, _) if self.series.len() > 1 => "Clustered error bar chart".into(),
            (ChartKind::ErrorBar, _) => "Simple error bar chart".into(),
            (ChartKind::RangeBar, _) => "Range bar chart".into(),
            (ChartKind::DropLine, _) => "Drop-line chart".into(),
            (ChartKind::HighLow, _) if self.series.len() > 2 => "High-low-close chart".into(),
            (ChartKind::HighLow, _) => "High-low chart".into(),
            (ChartKind::Difference, _) => "Difference area chart".into(),
            (ChartKind::Pie, _) => "Pie chart".into(),
            (ChartKind::Line, _) => "Line chart".into(),
            (ChartKind::Area, Some(Grouping::Percent)) => "100% stacked area chart".into(),
            (ChartKind::Area, Some(Grouping::Stacked)) => "Stacked area chart".into(),
            (ChartKind::Area, _) => "Area chart".into(),
            (ChartKind::Scatter, _) => "Scatter plot".into(),
            (ChartKind::Boxplot, _) => "Boxplot".into(),
            (ChartKind::StemLeaf, _) => "Stem-and-leaf plot".into(),
            (ChartKind::NormalQq, _) => "Normal Q-Q plot".into(),
            (ChartKind::DetrendedNormalQq, _) => "Detrended normal Q-Q plot".into(),
            (ChartKind::NormalPp, _) => "Normal P-P plot".into(),
            (ChartKind::Qq, _) => "Q-Q plot".into(),
            (ChartKind::DetrendedQq, _) => "Detrended Q-Q plot".into(),
            (ChartKind::Pp, _) => "P-P plot".into(),
            (ChartKind::DetrendedPp, _) => "Detrended P-P plot".into(),
            (ChartKind::Roc, _) => "ROC curve".into(),
            (ChartKind::SpreadLevel, _) => "Spread-versus-level plot".into(),
            (ChartKind::Dendrogram, _) => "Dendrogram".into(),
            (ChartKind::Icicle, _) => "Icicle plot".into(),
            (ChartKind::TreeDiagram, _) => "Tree diagram".into(),
            (ChartKind::TerritorialMap, _) => "Territorial map".into(),
            (ChartKind::Histogram, _) if self.normal.is_some() => {
                "Histogram with a normal curve".into()
            }
            (ChartKind::Histogram, _) => "Histogram".into(),
        }
    }

    /// The chart's numbers as a table: a row per category, a column per series; the caption
    /// says what kind of chart it is. The text output and the window show charts this way.
    /// The chart as a table of its numbers, outline label and hidden state included.
    pub fn data_table(&self) -> Table {
        Table {
            outline: self.outline.clone(),
            hidden: self.hidden,
            ..self.numbers_table()
        }
    }

    fn numbers_table(&self) -> Table {
        let mut caption = format!("{}.", self.kind_text());
        if let Some(subtitle) = &self.subtitle {
            caption = format!("{caption} {subtitle}.");
        }
        if let Some(footnote) = &self.footnote {
            caption = format!("{caption} {footnote}");
        }
        if let Some(plot) = &self.plot {
            return Table::of_plot(self, plot.table(self), caption);
        }
        let mut columns = vec![self.category_axis.title.clone()];
        columns.extend(self.series.iter().map(|s| s.name.clone()));
        let mut column_groups = vec![Vec::new()];
        column_groups.extend(
            self.series
                .iter()
                .map(|_| self.legend.iter().cloned().collect()),
        );
        // after the values, the half length of each series' error bars, then the length below
        // of the bars that are not symmetric: the office chart reads its bars from these columns
        let mut margins: Vec<&Vec<f64>> = Vec::new();
        if let Some(bars) = &self.error_bars {
            let uneven = |s: usize| bars.lower.get(s).is_some_and(|l| !l.is_empty());
            for (s, (series, m)) in self.series.iter().zip(&bars.margins).enumerate() {
                if !m.is_empty() {
                    margins.push(m);
                    columns.push(series.name.clone());
                    column_groups.push(vec![match uneven(s) {
                        true => format!("Error bar above, {}", bars.label),
                        false => format!("Error bar \u{b1} {}", bars.label),
                    }]);
                }
            }
            for (series, lower) in self.series.iter().zip(&bars.lower) {
                if !lower.is_empty() {
                    margins.push(lower);
                    columns.push(series.name.clone());
                    column_groups.push(vec![format!("Error bar below, {}", bars.label)]);
                }
            }
        }
        // a value the command could not compute, such as a mean that is not estimable
        let cell = |v: f64| match v.is_nan() {
            true => Cell::Empty,
            false => Cell::Number(v),
        };
        let rows: Vec<Vec<Cell>> = self
            .categories
            .iter()
            .enumerate()
            .map(|(i, category)| {
                let mut row = vec![Cell::Text(category.label.clone())];
                row.extend(self.series.iter().map(|s| cell(s.values[i])));
                row.extend(
                    margins
                        .iter()
                        .map(|m| cell(m.get(i).copied().unwrap_or(f64::NAN))),
                );
                row
            })
            .collect();
        let format = |r: usize, c: usize| match c {
            0 => String::new(),
            _ if c > self.series.len() => self.value_axis.format.clone(),
            _ => {
                let axis = match &self.second_axis {
                    Some(second) if c == self.series.len() => &second.format,
                    _ => &self.value_axis.format,
                };
                series_format(&self.series[c - 1], axis, &rows, r, c)
            }
        };
        let formats = crate::format::formats_of(&rows, format);
        Table {
            title: self.title.clone(),
            row_headers: 1,
            column_groups,
            columns,
            rows,
            formats,
            caption: Some(caption),
            outline: None,
            hidden: false,
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
