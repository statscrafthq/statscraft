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
        let lines: Vec<&str> = [&self.subtitle, &self.footnote]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect();
        match lines[..] {
            [] => {}
            [one] => title["subtitle"] = one.into(),
            _ => title["subtitle"] = lines.into(),
        }
        spec.insert("title".into(), title);
        let body = match (&self.plot, self.kind) {
            (Some(plot), _) => plot.vega_lite(self),
            (_, ChartKind::Histogram) => self.histogram(),
            (_, ChartKind::Pie) => self.pie(),
            (
                _,
                ChartKind::RangeBar
                | ChartKind::DropLine
                | ChartKind::HighLow
                | ChartKind::Difference,
            ) => self.spans(),
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
                if let Some([lower, upper]) = self.error_bar(j, i) {
                    row["lower"] = lower.into();
                    row["upper"] = upper.into();
                }
                if self.second_axis.is_some() && j + 1 == self.series.len() {
                    row["second"] = true.into();
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
        // marks that float over a span need not stand on zero
        if matches!(
            self.kind,
            ChartKind::ErrorBar
                | ChartKind::RangeBar
                | ChartKind::DropLine
                | ChartKind::HighLow
                | ChartKind::Difference
        ) {
            channel["scale"]["zero"] = false.into();
        }
        channel
    }

    /// The category axis as an ordered channel.
    fn category_channel(&self) -> Value {
        json!({
            "field": "category",
            "type": if self.kind == ChartKind::Bar { "nominal" } else { "ordinal" },
            "sort": self.labels(),
            "title": self.category_axis.title,
        })
    }

    /// Range bars from the first to the second series of each pair; or, for drop-line,
    /// high-low and difference charts, a rule or an area from the lowest to the highest value
    /// of each category under the points or lines of the series.
    fn spans(&self) -> Map<String, Value> {
        let mut out = Map::new();
        let low = |field: &str| {
            let mut channel = self.value_channel();
            channel["field"] = field.into();
            channel
        };
        let n = self.categories.len();
        if self.kind == ChartKind::RangeBar {
            let mut rows = Vec::new();
            let mut names = Vec::new();
            for pair in self.series.chunks(2) {
                let name = format!("{} to {}", pair[0].name, pair[1].name);
                for i in 0..n {
                    let (a, b) = (pair[0].values[i], pair[1].values[i]);
                    if a.is_finite() && b.is_finite() {
                        rows.push(json!({
                            "category": self.categories[i].label,
                            "series": name,
                            "low": a,
                            "high": b,
                        }));
                    }
                }
                names.push(name);
            }
            let mut encoding = json!({
                "x": self.category_channel(),
                "y": low("low"),
                "y2": { "field": "high" },
                "color": { "field": "series", "type": "nominal", "sort": names, "title": null },
            });
            if names.len() > 1 {
                encoding["xOffset"] = json!({ "field": "series", "sort": names });
            }
            out.insert("data".into(), json!({ "values": rows }));
            out.insert("mark".into(), json!({ "type": "bar" }));
            out.insert("encoding".into(), encoding);
            return out;
        }
        let spans: Vec<Value> = (0..n)
            .filter_map(|i| {
                let column: Vec<f64> = self.series.iter().map(|s| s.values[i]).collect();
                let (lo, hi) = match self.kind {
                    ChartKind::Difference => (column[0], column[1]),
                    _ => (
                        column.iter().copied().fold(f64::INFINITY, f64::min),
                        column.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    ),
                };
                (lo.is_finite() && hi.is_finite())
                    .then(|| json!({ "category": self.categories[i].label, "low": lo, "high": hi }))
            })
            .collect();
        let span_mark = match self.kind {
            ChartKind::Difference => json!({ "type": "area", "opacity": 0.35 }),
            _ => json!({ "type": "rule" }),
        };
        let points = match self.kind {
            ChartKind::Difference => json!({ "type": "line", "point": true }),
            _ => json!({ "type": "point", "filled": true }),
        };
        let mut color = json!({
            "field": "series",
            "type": "nominal",
            "sort": self.series_names(),
        });
        color["title"] = self.legend.clone().map_or(Value::Null, Value::from);
        out.insert(
            "layer".into(),
            json!([
                {
                    "data": { "values": spans },
                    "mark": span_mark,
                    "encoding": {
                        "x": self.category_channel(),
                        "y": low("low"),
                        "y2": { "field": "high" },
                    },
                },
                {
                    "data": { "values": self.rows() },
                    "mark": points,
                    "encoding": {
                        "x": self.category_channel(),
                        "y": self.value_channel(),
                        "color": color,
                    },
                },
            ]),
        );
        out
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
            _ => self.category_channel(),
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
            // the series side by side, without the line on a second axis
            let mut names = self.series_names();
            names.truncate(self.series.len() - usize::from(self.second_axis.is_some()));
            let clustered = self.grouping.is_none_or(|g| g == Grouping::Clustered);
            let beside = matches!(self.kind, ChartKind::Bar | ChartKind::ErrorBar);
            if beside && clustered && names.len() > 1 {
                encoding.insert(offset.into(), json!({ "field": "series", "sort": names }));
            }
        }
        let mark = match self.kind {
            ChartKind::Line => json!({ "type": "line", "point": true }),
            ChartKind::Area => json!({ "type": "area" }),
            ChartKind::Scatter => json!({ "type": "point" }),
            ChartKind::ErrorBar => json!({ "type": "point", "filled": true }),
            _ => json!({ "type": "bar" }),
        };
        let mut out = Map::new();
        out.insert("data".into(), json!({ "values": self.rows() }));
        let mut layers = vec![json!({ "mark": mark, "encoding": encoding })];
        if self.error_bars.is_some() {
            // the bars from each row's lower to its upper end, over the same categories, colors
            // and offsets as the marks; rows without bars have no ends and draw none
            let mut ends = encoding.clone();
            ends.insert(
                value_channel.into(),
                json!({ "field": "lower", "type": "quantitative", "title": self.value_axis.title }),
            );
            ends.insert(format!("{value_channel}2"), json!({ "field": "upper" }));
            layers.push(json!({ "mark": { "type": "errorbar", "ticks": true }, "encoding": ends }));
        }
        if let Some(second) = &self.second_axis {
            // the last series as a line on its own scale with its axis on the right
            for layer in &mut layers {
                layer["transform"] = json!([{ "filter": "!datum.second" }]);
            }
            let mut line = encoding.clone();
            line.remove(offset);
            let mut scale =
                json!({ "field": "value", "type": "quantitative", "title": second.title });
            scale["axis"] = axis_format(&second.format).unwrap_or(json!({}));
            scale["axis"]["orient"] = json!(if horizontal { "top" } else { "right" });
            if let Some(maximum) = second.maximum {
                scale["scale"] = json!({ "domain": [0.0, maximum] });
            }
            line.insert(value_channel.into(), scale);
            layers.push(json!({
                "transform": [{ "filter": "datum.second" }],
                "mark": { "type": "line", "point": true },
                "encoding": line,
            }));
            let mut independent = Map::new();
            independent.insert(value_channel.into(), "independent".into());
            out.insert("resolve".into(), json!({ "scale": independent }));
        }
        match layers.len() {
            1 => {
                let layer = layers.pop().expect("one layer");
                out.insert("mark".into(), layer["mark"].clone());
                out.insert("encoding".into(), layer["encoding"].clone());
            }
            _ => {
                out.insert("layer".into(), Value::Array(layers));
            }
        }
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
