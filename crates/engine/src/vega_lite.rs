//! A chart as a Vega-Lite specification: plain JSON with the numbers inline, for agents and
//! for any program that draws Vega-Lite. Nothing here depends on a Vega library; the colors
//! are left to the drawing program's theme.

use crate::Cell;
use crate::chart::{Chart, ChartKind, Grouping, Orientation};
use serde_json::{Map, Value, json};

/// The version of the Vega-Lite schema the specifications follow.
pub const SCHEMA: &str = "https://vega.github.io/schema/vega-lite/v5.json";

/// How many points draw the normal curve of a histogram.
const CURVE_POINTS: usize = 101;

impl Chart {
    /// The chart as a Vega-Lite specification with its data inline.
    pub fn vega_lite(&self) -> Value {
        let mut spec = Map::new();
        spec.insert("$schema".into(), SCHEMA.into());
        spec.insert("description".into(), self.kind_text().into());
        let mut title = json!({ "text": self.title });
        if let Some(footnote) = &self.footnote {
            title["subtitle"] = footnote.as_str().into();
        }
        spec.insert("title".into(), title);
        let body = match (&self.plot, self.kind) {
            (Some(plot), _) => plot.vega_lite(self),
            (_, ChartKind::Histogram) => self.histogram(),
            (_, ChartKind::Pie) => self.pie(),
            _ => self.categorical(),
        };
        spec.extend(body);
        Value::Object(spec)
    }

    /// One row per category and series.
    fn rows(&self) -> Vec<Value> {
        let mut rows = Vec::new();
        for (j, series) in self.series.iter().enumerate() {
            for (i, category) in self.categories.iter().enumerate() {
                let mut row = json!({
                    "category": category.label,
                    "series": series.name,
                    "value": series.values[i],
                    "order": i,
                    "series_order": j,
                });
                if let Cell::Number(x) = category.value {
                    row["x"] = x.into();
                }
                rows.push(row);
            }
        }
        rows
    }

    fn labels(&self) -> Vec<&str> {
        self.categories.iter().map(|c| c.label.as_str()).collect()
    }

    fn series_names(&self) -> Vec<&str> {
        self.series.iter().map(|s| s.name.as_str()).collect()
    }

    /// The value axis as a quantitative channel.
    fn value_channel(&self) -> Value {
        let mut channel = json!({
            "field": "value",
            "type": "quantitative",
            "title": self.value_axis.title,
        });
        if let Some(axis) = axis_format(&self.value_axis.format) {
            channel["axis"] = axis;
        }
        if let Some(maximum) = self.value_axis.maximum {
            channel["scale"] = json!({ "domainMax": maximum });
        }
        channel
    }

    /// Bars, lines, areas and scatter plots: categories along one axis, values along the other,
    /// a color per series when there are several.
    fn categorical(&self) -> Map<String, Value> {
        let several = self.series.len() > 1;
        let mut value = self.value_channel();
        let stack = match (self.kind, self.grouping) {
            (ChartKind::Bar | ChartKind::Area, Some(Grouping::Stacked)) => json!("zero"),
            (ChartKind::Bar | ChartKind::Area, Some(Grouping::Percent)) => json!("normalize"),
            _ => Value::Null,
        };
        value["stack"] = stack;
        if self.grouping == Some(Grouping::Percent) {
            value["axis"] = json!({ "format": ".0%" });
        }
        let category = match self.kind {
            ChartKind::Scatter => json!({
                "field": "x",
                "type": "quantitative",
                "title": self.category_axis.title,
            }),
            _ => json!({
                "field": "category",
                "type": if self.kind == ChartKind::Bar { "nominal" } else { "ordinal" },
                "sort": self.labels(),
                "title": self.category_axis.title,
            }),
        };
        let horizontal = self.orientation == Some(Orientation::Horizontal);
        let (category_channel, value_channel, offset) = match horizontal {
            true => ("y", "x", "yOffset"),
            false => ("x", "y", "xOffset"),
        };
        let mut encoding = Map::new();
        encoding.insert(category_channel.into(), category);
        encoding.insert(value_channel.into(), value);
        if several {
            let mut color = json!({
                "field": "series",
                "type": "nominal",
                "sort": self.series_names(),
            });
            color["title"] = self.legend.clone().map_or(Value::Null, Value::from);
            encoding.insert("color".into(), color);
            let clustered = self.grouping.is_none_or(|g| g == Grouping::Clustered);
            if self.kind == ChartKind::Bar && clustered {
                encoding.insert(
                    offset.into(),
                    json!({ "field": "series", "sort": self.series_names() }),
                );
            }
        }
        let mark = match self.kind {
            ChartKind::Line => json!({ "type": "line", "point": true }),
            ChartKind::Area => json!({ "type": "area" }),
            ChartKind::Scatter => json!({ "type": "point" }),
            _ => json!({ "type": "bar" }),
        };
        let mut out = Map::new();
        out.insert("data".into(), json!({ "values": self.rows() }));
        out.insert("mark".into(), mark);
        out.insert("encoding".into(), Value::Object(encoding));
        out
    }

    /// A slice per category, in the order of the categories.
    fn pie(&self) -> Map<String, Value> {
        let mut out = Map::new();
        out.insert("data".into(), json!({ "values": self.rows() }));
        out.insert("mark".into(), json!({ "type": "arc" }));
        out.insert(
            "encoding".into(),
            json!({
                "theta": { "field": "value", "type": "quantitative", "title": self.value_axis.title },
                "color": {
                    "field": "category",
                    "type": "nominal",
                    "sort": self.labels(),
                    "title": self.category_axis.title,
                },
                "order": { "field": "order", "type": "quantitative" },
            }),
        );
        out
    }

    /// Bars over the bins, and the normal curve as a line when there is one.
    fn histogram(&self) -> Map<String, Value> {
        let counts = self.series.first().map_or(&[][..], |s| &s.values[..]);
        let bins: Vec<[f64; 2]> = self.categories.iter().filter_map(|c| c.bin).collect();
        let rows: Vec<Value> = bins
            .iter()
            .zip(counts)
            .map(|([from, to], value)| json!({ "from": from, "to": to, "value": value }))
            .collect();
        let width = bins.first().map_or(1.0, |[from, to]| to - from);
        let mut x = json!({
            "field": "from",
            "type": "quantitative",
            "bin": { "binned": true, "step": width },
            "title": self.category_axis.title,
        });
        if let Some(axis) = axis_format(&self.category_axis.format) {
            x["axis"] = axis;
        }
        let bars = json!({
            "data": { "values": rows },
            "mark": { "type": "bar" },
            "encoding": { "x": x, "x2": { "field": "to" }, "y": self.value_channel() },
        });
        let mut layers = vec![bars];
        if let (Some(normal), Some(first), Some(last)) = (&self.normal, bins.first(), bins.last()) {
            let (low, high) = (first[0], last[1]);
            let points: Vec<Value> = (0..CURVE_POINTS)
                .map(|i| {
                    let x = low + (high - low) * i as f64 / (CURVE_POINTS - 1) as f64;
                    let z = (x - normal.mean) / normal.std_dev;
                    let density = (-0.5 * z * z).exp()
                        / (normal.std_dev * (2.0 * std::f64::consts::PI).sqrt());
                    json!({ "x": x, "y": normal.total * width * density })
                })
                .collect();
            layers.push(json!({
                "data": { "values": points },
                "mark": { "type": "line" },
                "encoding": {
                    "x": { "field": "x", "type": "quantitative" },
                    "y": { "field": "y", "type": "quantitative" },
                },
            }));
        }
        let mut out = Map::new();
        out.insert("layer".into(), Value::Array(layers));
        out
    }
}

/// The axis labels in a display format: the decimals of F and COMMA, and PCT with a percent
/// sign. None for the formats Vega-Lite has no form for, such as dates.
fn axis_format(format: &str) -> Option<Value> {
    let letters = format.find(|c: char| !c.is_ascii_alphabetic())?;
    let (kind, size) = format.split_at(letters);
    let decimals: usize = size
        .split_once('.')
        .map_or(Some(0), |(_, d)| d.parse().ok())?;
    match kind.to_ascii_uppercase().as_str() {
        "F" => Some(json!({ "format": format!(".{decimals}f") })),
        "COMMA" => Some(json!({ "format": format!(",.{decimals}f") })),
        "PCT" => Some(json!({
            "labelExpr": format!("format(datum.value, '.{decimals}f') + '%'")
        })),
        _ => None,
    }
}

#[cfg(test)]
#[test]
fn axis_formats() {
    assert_eq!(axis_format("F8.2"), Some(json!({ "format": ".2f" })));
    assert_eq!(axis_format("COMMA10"), Some(json!({ "format": ",.0f" })));
    assert_eq!(
        axis_format("PCT8.1"),
        Some(json!({ "labelExpr": "format(datum.value, '.1f') + '%'" }))
    );
    assert_eq!(axis_format("DATE11"), None);
    assert_eq!(axis_format(""), None);
}
