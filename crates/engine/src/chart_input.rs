//! Charts from outside: BEGIN CHART, a Statscraft extension, reads a chart as JSON so that an
//! agent or a script can put a chart that no procedure makes into the output. Two forms:
//! - the `Chart` that `statscraft run --json` writes, optionally with `"version": 1`; what
//!   Statscraft writes, it reads back;
//! - a Vega-Lite specification of one view with inline values: mark bar, line, area, point or
//!   arc, encoded by x, y, color, theta and xOffset or yOffset. Nothing is aggregated, binned
//!   or transformed; what does not map cleanly is refused with a message that names it.
//!
//! The JSON comes from outside, so it is checked before anything draws it: sizes are capped,
//! fields Statscraft does not know are refused, and the shape must be one every drawing (the
//! window, the pictures, the office charts, Vega-Lite) can take without failing.
//!
//! Charts read from an output file (OUTPUT OPEN) pass the same checks with wider limits, since
//! procedures make larger charts than an agent should send, and with the plot shapes only
//! procedures make (merges, tree, regions, stem-and-leaf) checked for their indexes as well.
//! Fields the chart does not have are dropped there: the file format lets a newer version add
//! optional fields.

use crate::Cell;
use crate::chart::{Axis, Category, Chart, ChartKind, Grouping, Orientation, Series, Statistic};
use crate::chart::{Tier, count_format};
use crate::plot::{Plot, PointGroup};
use serde_json::{Map, Value};
use std::collections::HashMap;

/// The version of the chart JSON this Statscraft reads and writes.
const VERSION: u64 = 1;
const MOST_SERIES: usize = 100;
const MOST_CATEGORIES: usize = crate::graph::MOST_CATEGORIES;
/// Numbers of a chart: series times categories, points, or Vega-Lite rows.
const MOST_VALUES: usize = 100_000;
/// Characters of any one string.
const LONGEST_TEXT: usize = 1000;
/// Bytes of the JSON text.
const MOST_BYTES: usize = 16 << 20;

/// How much a chart may hold, and which plot shapes it may have.
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    /// Characters of any one string.
    pub(crate) text: usize,
    series: usize,
    categories: usize,
    /// Series times categories, or points, or marked cases.
    values: usize,
    /// Merges, tree and regions plots and stem-and-leaf displays, which only procedures make.
    every_shape: bool,
}

/// A chart from an agent or a script.
const OUTSIDE: Limits = Limits {
    text: LONGEST_TEXT,
    series: MOST_SERIES,
    categories: MOST_CATEGORIES,
    values: MOST_VALUES,
    every_shape: false,
};

/// A chart in an output file, which a procedure made. A string variable holds up to 32767
/// bytes, and a bar chart or a scatter plot has a category or a point per value or case.
pub(crate) const SAVED: Limits = Limits {
    text: 32_768,
    series: 10_000,
    categories: 100_000,
    values: 2_000_000,
    every_shape: true,
};

/// Nodes of a tree diagram, and how deep it goes.
const MOST_NODES: usize = 10_000;
const DEEPEST: usize = 1_000;

/// A chart of an output file, checked as chart input is; fields it does not have are dropped.
pub(crate) fn saved(value: &Value) -> Result<Chart, String> {
    sizes(value, &SAVED, &mut 0)?;
    let chart: Chart = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    check(&chart, &SAVED)?;
    Ok(chart)
}

/// Reads the JSON lines of BEGIN CHART, each with its line number in the syntax.
pub(crate) fn read(lines: &[(u32, String)]) -> Result<Chart, String> {
    let Some(&(first, _)) = lines.first() else {
        return Err(
            "BEGIN CHART holds no chart: put its JSON on the lines before END CHART".into(),
        );
    };
    let text: Vec<&str> = lines.iter().map(|(_, l)| l.as_str()).collect();
    let text = text.join("\n");
    if text.len() > MOST_BYTES {
        return Err(format!("the chart is larger than {MOST_BYTES} bytes"));
    }
    let value: Value = serde_json::from_str(&text).map_err(|e| {
        let message = e.to_string();
        let message = message.split(" at line ").next().unwrap_or_default();
        let line = first as usize + e.line().saturating_sub(1);
        format!("line {line}, column {}: {message}", e.column())
    })?;
    sizes(&value, &OUTSIDE, &mut 0)?;
    let Value::Object(map) = value else {
        return Err("the chart must be a JSON object".into());
    };
    let chart = match is_vega_lite(&map) {
        true => vega_lite(&map).map_err(|e| format!("Vega-Lite: {e}"))?,
        false => statscraft(map)?,
    };
    check(&chart, &OUTSIDE)?;
    Ok(chart)
}

/// Strings no longer than the limit, and not more numbers and strings than a chart can hold
/// with its labels.
pub(crate) fn sizes(value: &Value, limits: &Limits, scalars: &mut usize) -> Result<(), String> {
    match value {
        Value::String(s) if s.chars().count() > limits.text => Err(format!(
            "a string is longer than {} characters: {}...",
            limits.text,
            s.chars().take(40).collect::<String>()
        )),
        Value::Array(items) => items.iter().try_for_each(|v| sizes(v, limits, scalars)),
        Value::Object(map) => map.iter().try_for_each(|(k, v)| {
            sizes(&Value::String(k.clone()), limits, scalars)?;
            sizes(v, limits, scalars)
        }),
        _ => {
            *scalars += 1;
            match *scalars > 10 * limits.values {
                true => Err(format!("it holds more than {} values", 10 * limits.values)),
                false => Ok(()),
            }
        }
    }
}

fn is_vega_lite(map: &Map<String, Value>) -> bool {
    let schema = map.get("$schema").and_then(Value::as_str);
    map.contains_key("mark") || schema.is_some_and(|s| s.contains("vega"))
}

/// The chart as `--json` writes it. A field the chart does not have is refused, so that a
/// misspelt one is not lost; one that is null, false or empty is the same as left out.
fn statscraft(mut map: Map<String, Value>) -> Result<Chart, String> {
    match map.remove("version") {
        None => {}
        Some(v) if v == VERSION => {}
        Some(v) => {
            return Err(format!(
                "chart version {v}: this Statscraft reads version {VERSION}; update Statscraft"
            ));
        }
    }
    // the Vega-Lite specification that --vega-lite writes beside the chart
    map.remove("vega_lite");
    let value = Value::Object(map);
    let chart: Chart = serde_json::from_value(value.clone()).map_err(|e| format!("chart: {e}"))?;
    let known = serde_json::to_value(&chart).expect("plain data");
    match unknown(&value, &known, String::new()) {
        Some(path) => Err(format!("chart: unknown field {path}")),
        None => Ok(chart),
    }
}

/// The path of the first field of `given` that reading it into a chart dropped.
fn unknown(given: &Value, known: &Value, path: String) -> Option<String> {
    let empty = |v: &Value| match v {
        Value::Null => true,
        Value::Bool(b) => !b,
        Value::String(s) => s.is_empty(),
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        Value::Number(_) => false,
    };
    match (given, known) {
        (Value::Object(given), Value::Object(known)) => given.iter().find_map(|(key, v)| {
            let path = match path.is_empty() {
                true => key.clone(),
                false => format!("{path}.{key}"),
            };
            match known.get(key) {
                Some(k) => unknown(v, k, path),
                None if empty(v) => None,
                None => Some(path),
            }
        }),
        (Value::Array(given), Value::Array(known)) => given
            .iter()
            .zip(known)
            .enumerate()
            .find_map(|(i, (g, k))| unknown(g, k, format!("{path}[{i}]"))),
        _ => None,
    }
}

/// The name of a kind as the JSON spells it.
fn kind_name(kind: ChartKind) -> String {
    let name = serde_json::to_value(kind).expect("plain data");
    name.as_str().unwrap_or_default().to_string()
}

/// What every drawing of a chart relies on.
fn check(chart: &Chart, limits: &Limits) -> Result<(), String> {
    let kind = kind_name(chart.kind);
    let (most_values, most_categories) = (limits.values, limits.categories);
    let axes = [&chart.category_axis, &chart.value_axis];
    for axis in axes.into_iter().chain(&chart.second_axis) {
        if !is_format(&axis.format) {
            return Err(format!(
                "axis {}: format {} is not a display format such as F8.2 or PCT8.1",
                axis.title, axis.format
            ));
        }
    }
    if let Some(normal) = &chart.normal {
        let numbers = [normal.mean, normal.std_dev, normal.total];
        if chart.kind != ChartKind::Histogram
            || numbers.iter().any(|v| !v.is_finite())
            || normal.std_dev <= 0.0
        {
            return Err(
                "normal: a histogram's normal curve needs a mean, a standard deviation \
                        above 0 and a total"
                    .into(),
            );
        }
    }
    let points_kind = data_of(chart.kind) == Data::Points;
    match &chart.plot {
        Some(Plot::Points { groups, lines, .. }) if points_kind => {
            let points: usize = groups.iter().map(|g| g.points.len()).sum();
            if points == 0 || points > most_values {
                return Err(format!("plot: give 1 to {most_values} points"));
            }
            if groups.len() > limits.series || lines.len() > limits.series {
                return Err(format!("plot: at most {} groups and lines", limits.series));
            }
            Ok(())
        }
        Some(Plot::Boxes { boxes }) if data_of(chart.kind) == Data::Boxes => {
            let marks: usize = boxes
                .iter()
                .map(|b| b.outliers.len() + b.extremes.len())
                .sum();
            if boxes.is_empty() || boxes.len() > most_categories || marks > most_values {
                return Err(format!(
                    "plot: give 1 to {most_categories} boxes with at most {most_values} marked \
                     cases"
                ));
            }
            Ok(())
        }
        Some(plot) if limits.every_shape => check_shape(chart.kind, plot, limits),
        Some(plot) => {
            let shape = serde_json::to_value(plot).expect("plain data");
            let shape = shape["shape"].as_str().unwrap_or_default().to_string();
            Err(format!(
                "plot: a chart of kind {kind} with plot shape {shape} is not read; a chart from \
                 outside takes plot shape points (kinds scatter, normal_qq, detrended_normal_qq, \
                 normal_pp, qq, detrended_qq, pp, detrended_pp, roc, spread_level) or boxes \
                 (kind boxplot)"
            ))
        }
        None => check_categories(chart, &kind, limits),
    }
}

/// Where a kind gives its data.
#[derive(PartialEq)]
enum Data {
    Categories,
    Points,
    Boxes,
    /// A plot shape only procedures make: merges, tree, regions or stem-and-leaf lines.
    Procedure,
}

/// Every kind, deliberately: a new kind does not compile until it is placed here.
fn data_of(kind: ChartKind) -> Data {
    use ChartKind::*;
    match kind {
        Bar | Pie | Line | Area | ErrorBar | RangeBar | DropLine | HighLow | Difference
        | Histogram => Data::Categories,
        Scatter | NormalQq | DetrendedNormalQq | NormalPp | Qq | DetrendedQq | Pp | DetrendedPp
        | Roc | SpreadLevel => Data::Points,
        Boxplot => Data::Boxes,
        StemLeaf | Dendrogram | Icicle | TreeDiagram | TerritorialMap => Data::Procedure,
    }
}

/// The plot shapes only procedures make: every index points into its list, a clustering has
/// fewer merges than items, and a tree is a tree.
fn check_shape(kind: ChartKind, plot: &Plot, limits: &Limits) -> Result<(), String> {
    let wrong = |what: &str| Err(format!("plot: {what}"));
    match (plot, kind) {
        (
            Plot::Merges {
                items,
                order,
                merges,
                ..
            },
            ChartKind::Dendrogram | ChartKind::Icicle,
        ) => {
            let n = items.len();
            if n == 0 || n > limits.categories {
                return wrong(&format!("a clustering of 1 to {} items", limits.categories));
            }
            if order.len() != n || order.iter().any(|&i| i >= n) {
                return wrong("order lists every item once by its index");
            }
            if merges.len() >= n || merges.iter().any(|m| m.left >= n || m.right >= n) {
                return wrong("merges join items by their index, at most one fewer than items");
            }
            Ok(())
        }
        (Plot::Tree { nodes }, ChartKind::TreeDiagram) => {
            if nodes.is_empty() || nodes.len() > MOST_NODES {
                return wrong(&format!("a tree of 1 to {MOST_NODES} nodes"));
            }
            // the root first, every other node after its parent, each id once: no cycles
            let mut depth: HashMap<usize, usize> = HashMap::new();
            for (i, node) in nodes.iter().enumerate() {
                let level = match (i, node.parent) {
                    (0, None) => 0,
                    (_, Some(p)) if i > 0 => match depth.get(&p) {
                        Some(&d) if d < DEEPEST => d + 1,
                        _ => return wrong("every node but the first comes after its parent"),
                    },
                    _ => return wrong("the first node is the root, the only one without parent"),
                };
                if depth.insert(node.id, level).is_some() {
                    return wrong(&format!("node id {} is given twice", node.id));
                }
            }
            Ok(())
        }
        (Plot::Regions { regions }, ChartKind::TerritorialMap) => {
            let corners: usize = regions.iter().map(|r| r.vertices.len()).sum();
            match regions.len() <= limits.categories && corners <= limits.values {
                true => Ok(()),
                false => wrong("too many regions or corners"),
            }
        }
        (Plot::StemLeaf(s), ChartKind::StemLeaf) => match s.lines.len() <= limits.values {
            true => Ok(()),
            false => wrong("too many lines"),
        },
        _ => wrong(&format!(
            "shape and kind {} do not go together",
            kind_name(kind)
        )),
    }
}

/// A chart of categories and series: every series has a value for every category, and the
/// kind's own needs are met.
fn check_categories(chart: &Chart, kind: &str, limits: &Limits) -> Result<(), String> {
    use ChartKind::*;
    if data_of(chart.kind) != Data::Categories {
        // a scatter plot from outside gives its points in plot, as GRAPH does
        return Err(format!(
            "a chart of kind {kind} gives its data in plot; Statscraft reads plot shape points \
             and boxes from outside"
        ));
    }
    let (n, m) = (chart.categories.len(), chart.series.len());
    let (most_values, most_categories) = (limits.values, limits.categories);
    if n == 0 || n > most_categories {
        return Err(format!("categories: give 1 to {most_categories}"));
    }
    if m == 0 || m > limits.series {
        return Err(format!("series: give 1 to {}", limits.series));
    }
    if n * m > most_values {
        return Err(format!(
            "{m} series of {n} categories are more than {most_values} values"
        ));
    }
    if let Some(s) = chart.series.iter().find(|s| s.values.len() != n) {
        return Err(format!(
            "series {}: {} values for {n} categories; give one per category, null where there \
             is none",
            s.name,
            s.values.len()
        ));
    }
    if let Some(bars) = &chart.error_bars {
        let wrong = bars.margins.iter().any(|m| !m.is_empty() && m.len() != n);
        if bars.margins.len() > m || wrong {
            return Err(format!(
                "error_bars: a list of margins per series, each empty or with {n} numbers"
            ));
        }
    }
    match chart.kind {
        RangeBar if m % 2 == 1 => Err("a range bar chart needs its series in pairs".into()),
        Difference if m != 2 => Err("a difference area chart needs two series".into()),
        Histogram => {
            let bins = chart.categories.iter().map(|c| c.bin);
            match bins.into_iter().all(|b| b.is_some_and(|[lo, hi]| lo < hi)) {
                true => Ok(()),
                false => Err("a histogram needs a bin [lower, upper] for every category".into()),
            }
        }
        _ if chart.second_axis.is_some() && (chart.kind != Bar || m < 2) => {
            Err("second_axis: only a bar chart with two or more series has one".into())
        }
        _ => Ok(()),
    }
}

/// A display format such as F8.2, PCT8.1 or COMMA12, or none; a custom currency format may
/// carry its specification after a blank, as SET CCA gives it. Width and decimals are capped,
/// since the text of a number is made that wide.
pub(crate) fn is_format(format: &str) -> bool {
    if format.is_empty() {
        return true;
    }
    let format = format.split_once(' ').map_or(format, |(format, _)| format);
    let letters = format.find(|c: char| !c.is_ascii_alphabetic());
    let (kind, size) = format.split_at(letters.unwrap_or(format.len()));
    let (width, decimals) = size.split_once('.').unwrap_or((size, "0"));
    let number = |s: &str, most: u32| {
        !s.is_empty()
            && s.chars().all(|c| c.is_ascii_digit())
            && s.parse::<u32>().is_ok_and(|n| n <= most)
    };
    (1..=8).contains(&kind.len()) && number(width, 32_767) && number(decimals, 16)
}

// ---- Vega-Lite ----

const VIEW: &str = "Statscraft reads one view with inline data.values, mark bar, line, area, \
                    point or arc, and encodings x, y, color, theta and xOffset or yOffset";

/// Members of a specification that change only how the chart looks; Statscraft draws in its
/// own way.
const LOOKS: [&str; 9] = [
    "$schema",
    "description",
    "width",
    "height",
    "config",
    "autosize",
    "padding",
    "background",
    "name",
];

#[derive(Clone, Copy, PartialEq)]
enum Type {
    Quantitative,
    Nominal,
    Ordinal,
    Temporal,
}

enum Sort {
    Ascending,
    Descending,
    /// As the values come in the data.
    Data,
    /// These first, the others after them in ascending order.
    Listed(Vec<Value>),
}

struct Channel {
    field: String,
    kind: Type,
    title: Option<String>,
    sort: Sort,
    stack: Option<Value>,
}

impl Channel {
    fn title(&self) -> String {
        self.title.clone().unwrap_or_else(|| self.field.clone())
    }
}

/// A category or series value: a number or a text.
#[derive(Clone, PartialEq)]
enum Key {
    Number(f64),
    Text(String),
}

impl Key {
    fn of(value: Option<&Value>) -> Option<Key> {
        match value? {
            Value::Number(n) => n.as_f64().map(Key::Number),
            Value::String(s) => Some(Key::Text(s.clone())),
            Value::Bool(b) => Some(Key::Text(b.to_string())),
            _ => None,
        }
    }

    fn id(&self) -> String {
        match self {
            Key::Number(n) => format!("n{}", n.to_bits()),
            Key::Text(t) => format!("t{t}"),
        }
    }

    fn label(&self) -> String {
        match self {
            Key::Number(n) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{n:.0}"),
            Key::Number(n) => n.to_string(),
            Key::Text(t) => t.clone(),
        }
    }

    fn cell(&self) -> Cell {
        match self {
            Key::Number(n) => Cell::Number(*n),
            Key::Text(t) => Cell::Text(t.clone()),
        }
    }

    /// Numbers before texts, numbers by size, texts by their characters.
    fn order(&self, other: &Key) -> std::cmp::Ordering {
        match (self, other) {
            (Key::Number(a), Key::Number(b)) => a.total_cmp(b),
            (Key::Number(_), Key::Text(_)) => std::cmp::Ordering::Less,
            (Key::Text(_), Key::Number(_)) => std::cmp::Ordering::Greater,
            (Key::Text(a), Key::Text(b)) => a.cmp(b),
        }
    }
}

type Row = Map<String, Value>;

/// The distinct values of `channel`'s field in its sort order, and for each row the position
/// of its value among them.
fn distinct(rows: &[&Row], channel: &Channel) -> Result<(Vec<Key>, Vec<usize>), String> {
    let mut keys: Vec<Key> = Vec::new();
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut at = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let key = Key::of(row.get(&channel.field)).ok_or_else(|| {
            format!(
                "row {} of data.values has no value for {}",
                i + 1,
                channel.field
            )
        })?;
        let next = keys.len();
        let k = *seen.entry(key.id()).or_insert(next);
        if k == next {
            keys.push(key);
        }
        at.push(k);
    }
    let mut order: Vec<usize> = (0..keys.len()).collect();
    match &channel.sort {
        Sort::Data => {}
        Sort::Ascending => order.sort_by(|&a, &b| keys[a].order(&keys[b])),
        Sort::Descending => order.sort_by(|&a, &b| keys[b].order(&keys[a])),
        Sort::Listed(listed) => {
            let mut places = HashMap::new();
            for (p, v) in listed.iter().enumerate().rev() {
                if let Some(key) = Key::of(Some(v)) {
                    places.insert(key.id(), p);
                }
            }
            let place: Vec<Option<usize>> =
                keys.iter().map(|k| places.get(&k.id()).copied()).collect();
            order.sort_by(|&a, &b| match (place[a], place[b]) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => keys[a].order(&keys[b]),
            });
        }
    }
    let mut rank = vec![0; keys.len()];
    for (r, &k) in order.iter().enumerate() {
        rank[k] = r;
    }
    let sorted = order.iter().map(|&k| keys[k].clone()).collect();
    Ok((sorted, at.into_iter().map(|k| rank[k]).collect()))
}

/// A number of a quantitative field; None for null or a missing field.
fn number(row: &Row, i: usize, field: &str) -> Result<Option<f64>, String> {
    match row.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => Ok(n.as_f64()),
        Some(other) => Err(format!(
            "row {} of data.values: {field} is {other}, not a number",
            i + 1
        )),
    }
}

fn channel(name: &str, def: &Value) -> Result<Channel, String> {
    let def = def
        .as_object()
        .ok_or_else(|| format!("encoding {name} must be an object with field and type"))?;
    for key in def.keys() {
        if ![
            "field", "type", "title", "sort", "stack", "axis", "legend", "scale",
        ]
        .contains(&key.as_str())
        {
            return Err(format!(
                "{key} in encoding {name} is not supported; Statscraft draws the values as they \
                 are, without aggregate, bin, timeUnit or a fixed value"
            ));
        }
    }
    let field = def
        .get("field")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("encoding {name} needs a field"))?;
    let kind = match def.get("type").and_then(Value::as_str) {
        Some("quantitative") => Type::Quantitative,
        Some("nominal") => Type::Nominal,
        Some("ordinal") => Type::Ordinal,
        Some("temporal") => Type::Temporal,
        None if name != "x" && name != "y" && name != "theta" => Type::Nominal,
        Some(other) => return Err(format!("type {other} in encoding {name} is not supported")),
        None => return Err(format!("encoding {name} needs a type")),
    };
    let title = match def.get("title") {
        None => None,
        Some(Value::Null) => Some(String::new()),
        Some(title) => Some(text(title).ok_or_else(|| format!("title of encoding {name}"))?),
    };
    let sort = match def.get("sort") {
        None => Sort::Ascending,
        Some(Value::Null) => Sort::Data,
        Some(Value::String(s)) if s == "ascending" => Sort::Ascending,
        Some(Value::String(s)) if s == "descending" => Sort::Descending,
        Some(Value::Array(values)) => Sort::Listed(values.clone()),
        Some(other) => {
            return Err(format!(
                "sort {other} in encoding {name} is not supported; give \"ascending\", \
                 \"descending\", null (as in the data) or a list of values"
            ));
        }
    };
    Ok(Channel {
        field: field.to_string(),
        kind,
        title,
        sort,
        stack: def.get("stack").cloned(),
    })
}

/// A title: a string, or lines as an array of strings, joined with spaces.
fn text(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Array(lines) => {
            let lines: Option<Vec<&str>> = lines.iter().map(Value::as_str).collect();
            Some(lines?.join(" "))
        }
        _ => None,
    }
}

fn vega_lite(spec: &Map<String, Value>) -> Result<Chart, String> {
    for key in spec.keys() {
        let ours = ["title", "data", "mark", "encoding"].contains(&key.as_str());
        if !ours && !LOOKS.contains(&key.as_str()) {
            return Err(format!("{key} is not supported; {VIEW}"));
        }
    }
    if let Some(schema) = spec.get("$schema").and_then(Value::as_str)
        && !schema.contains("vega-lite")
    {
        return Err(format!("{schema} is not a Vega-Lite schema; {VIEW}"));
    }
    let mark = match spec.get("mark") {
        Some(Value::String(mark)) => mark.as_str(),
        Some(Value::Object(mark)) => mark
            .get("type")
            .and_then(Value::as_str)
            .ok_or("mark needs a type")?,
        _ => return Err(format!("mark is missing; {VIEW}")),
    };
    let allowed: &[&str] = match mark {
        "bar" => &["x", "y", "color", "xOffset", "yOffset"],
        "line" | "area" | "point" => &["x", "y", "color"],
        "arc" => &["theta", "color"],
        other => {
            return Err(format!(
                "mark {other} is not supported; Statscraft reads bar, line, area, point and arc"
            ));
        }
    };
    let data = spec
        .get("data")
        .and_then(Value::as_object)
        .ok_or("data is missing; give the rows inline in data.values")?;
    if let Some(key) = data.keys().find(|k| *k != "values") {
        return Err(format!(
            "data.{key} is not supported; give the rows inline in data.values"
        ));
    }
    let values = data
        .get("values")
        .and_then(Value::as_array)
        .ok_or("give the rows inline in data.values, as an array of objects")?;
    if values.is_empty() || values.len() > MOST_VALUES {
        return Err(format!("data.values: give 1 to {MOST_VALUES} rows"));
    }
    let rows: Vec<&Row> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_object()
                .ok_or_else(|| format!("row {} of data.values is not an object", i + 1))
        })
        .collect::<Result<_, _>>()?;
    let encoding = spec
        .get("encoding")
        .and_then(Value::as_object)
        .ok_or("encoding is missing")?;
    let mut channels = HashMap::new();
    for (name, def) in encoding {
        if !allowed.contains(&name.as_str()) {
            return Err(format!("encoding {name} is not supported with mark {mark}"));
        }
        channels.insert(name.as_str(), channel(name, def)?);
    }
    let (title, subtitle) = match spec.get("title") {
        Some(Value::Object(t)) => (
            t.get("text").and_then(text),
            t.get("subtitle").and_then(text),
        ),
        Some(t) => (text(t), None),
        None => (None, None),
    };
    let mut chart = match mark {
        "arc" => pie(&rows, &channels)?,
        "point" => scatter(&rows, &channels)?,
        _ => categorical(mark, &rows, &channels)?,
    };
    if let Some(title) = title {
        chart.title = title;
    }
    chart.subtitle = subtitle;
    Ok(chart)
}

fn axis(channel: &Channel) -> Axis {
    Axis {
        title: channel.title(),
        format: String::new(),
        maximum: None,
        variable: None,
    }
}

/// The format of a value axis: as many decimals as the values have, at most four.
fn value_format(values: impl Iterator<Item = f64>) -> String {
    let values: Vec<f64> = values.filter(|v| v.is_finite()).collect();
    let decimals = values
        .iter()
        .map(|v| v.to_string().split_once('.').map_or(0, |(_, d)| d.len()))
        .max()
        .unwrap_or(0);
    match decimals {
        0 => count_format(values),
        d => format!("F12.{}", d.min(4)),
    }
}

/// The values of `value` by category and series, from rows in any order. A category and
/// series given twice is an error: nothing is added up.
fn table(
    rows: &[&Row],
    categories: &[usize],
    series: Option<&[usize]>,
    counts: (usize, usize),
    value: &Channel,
) -> Result<Vec<Vec<f64>>, String> {
    let (n, m) = counts;
    let mut values = vec![vec![f64::NAN; n]; m];
    let mut from = vec![vec![None; n]; m];
    for (i, row) in rows.iter().enumerate() {
        let (c, s) = (categories[i], series.map_or(0, |s| s[i]));
        if let Some(earlier) = from[s][c] {
            return Err(format!(
                "rows {} and {} of data.values give the same category{}; Statscraft draws the \
                 values as they are and adds nothing up",
                earlier + 1,
                i + 1,
                if series.is_some() { " and series" } else { "" }
            ));
        }
        from[s][c] = Some(i);
        values[s][c] = number(row, i, &value.field)?.unwrap_or(f64::NAN);
    }
    Ok(values)
}

fn categorical(
    mark: &str,
    rows: &[&Row],
    channels: &HashMap<&str, Channel>,
) -> Result<Chart, String> {
    let (Some(x), Some(y)) = (channels.get("x"), channels.get("y")) else {
        return Err(format!("mark {mark} needs x and y"));
    };
    let quantitative = |c: &Channel| c.kind == Type::Quantitative;
    let (category, value, horizontal) = match (quantitative(x), quantitative(y)) {
        (_, true) => (x, y, false),
        (true, false) if mark == "bar" => (y, x, true),
        (true, false) => return Err(format!("a {mark} runs along x: put the categories on x")),
        (false, false) => {
            return Err("one of x and y must be quantitative: the values to draw".into());
        }
    };
    let color = channels.get("color");
    let offset = channels.get("xOffset").or(channels.get("yOffset"));
    if let Some(offset) = offset
        && color.is_none_or(|c| c.field != offset.field)
    {
        return Err("xOffset and yOffset must use the field of color".into());
    }
    let stack = value.stack.as_ref();
    let stacked = || match stack {
        None => Ok(Some(Grouping::Stacked)),
        Some(Value::Bool(true)) => Ok(Some(Grouping::Stacked)),
        Some(Value::String(s)) if s == "zero" => Ok(Some(Grouping::Stacked)),
        Some(Value::String(s)) if s == "normalize" => Ok(Some(Grouping::Percent)),
        Some(Value::Null | Value::Bool(false)) => Ok(None),
        Some(other) => Err(format!("stack {other} is not supported")),
    };
    let grouping = match (mark, color) {
        ("bar", None) => Some(Grouping::Clustered),
        ("bar", Some(_)) if offset.is_some() => Some(Grouping::Clustered),
        // bars that overlap in Vega-Lite stand side by side here
        ("bar", Some(_)) => stacked()?.or(Some(Grouping::Clustered)),
        ("area", Some(_)) => stacked()?,
        ("line", _) if stack.is_some_and(|s| !matches!(s, Value::Null | Value::Bool(false))) => {
            return Err("stacked lines are not supported".into());
        }
        _ => None,
    };
    let (keys, at) = distinct(rows, category)?;
    let (names, series_at) = match color {
        Some(color) => {
            let (keys, at) = distinct(rows, color)?;
            (keys.iter().map(Key::label).collect(), Some(at))
        }
        None => (vec![value.title()], None),
    };
    if keys.len() > MOST_CATEGORIES || names.len() > MOST_SERIES {
        return Err(format!(
            "at most {MOST_CATEGORIES} categories and {MOST_SERIES} colors"
        ));
    }
    let counts = (keys.len(), names.len());
    let values = table(rows, &at, series_at.as_deref(), counts, value)?;
    let value_axis = Axis {
        format: value_format(values.iter().flatten().copied()),
        ..axis(value)
    };
    let category_axis = axis(category);
    Ok(Chart {
        kind: match mark {
            "bar" => ChartKind::Bar,
            "line" => ChartKind::Line,
            _ => ChartKind::Area,
        },
        tier: Tier::Presentation,
        orientation: Some(match horizontal {
            true => Orientation::Horizontal,
            false => Orientation::Vertical,
        }),
        grouping,
        title: format!("{} by {}", value_axis.title, category_axis.title),
        subtitle: None,
        footnote: None,
        category_axis,
        value_axis,
        legend: color.map(Channel::title),
        categories: keys.iter().map(category_of).collect(),
        series: names
            .into_iter()
            .zip(values)
            .map(|(name, values)| Series {
                name,
                statistic: Statistic::Value,
                variable: None,
                value: Cell::Empty,
                values,
            })
            .collect(),
        normal: None,
        error_bars: None,
        second_axis: None,
        plot: None,
        outline: None,
        hidden: false,
    })
}

fn category_of(key: &Key) -> Category {
    Category {
        label: key.label(),
        value: key.cell(),
        missing: false,
        bin: None,
    }
}

fn pie(rows: &[&Row], channels: &HashMap<&str, Channel>) -> Result<Chart, String> {
    let (Some(theta), Some(color)) = (channels.get("theta"), channels.get("color")) else {
        return Err("mark arc needs theta (the values) and color (the slices)".into());
    };
    if theta.kind != Type::Quantitative {
        return Err("theta of mark arc must be quantitative".into());
    }
    let (keys, at) = distinct(rows, color)?;
    if keys.len() > MOST_CATEGORIES {
        return Err(format!("at most {MOST_CATEGORIES} slices"));
    }
    let values = table(rows, &at, None, (keys.len(), 1), theta)?;
    let value_axis = Axis {
        format: value_format(values[0].iter().copied()),
        ..axis(theta)
    };
    let category_axis = axis(color);
    Ok(Chart {
        kind: ChartKind::Pie,
        tier: Tier::Presentation,
        orientation: None,
        grouping: None,
        title: format!("{} by {}", value_axis.title, category_axis.title),
        subtitle: None,
        footnote: None,
        series: vec![Series {
            name: value_axis.title.clone(),
            statistic: Statistic::Value,
            variable: None,
            value: Cell::Empty,
            values: values.into_iter().next().unwrap_or_default(),
        }],
        category_axis,
        value_axis,
        legend: None,
        categories: keys.iter().map(category_of).collect(),
        normal: None,
        error_bars: None,
        second_axis: None,
        plot: None,
        outline: None,
        hidden: false,
    })
}

/// Points in groups by color, as GRAPH draws a scatter plot; a row without both numbers is
/// left out.
fn scatter(rows: &[&Row], channels: &HashMap<&str, Channel>) -> Result<Chart, String> {
    let (Some(x), Some(y)) = (channels.get("x"), channels.get("y")) else {
        return Err("mark point needs x and y".into());
    };
    if x.kind != Type::Quantitative || y.kind != Type::Quantitative {
        return Err(
            "mark point needs quantitative x and y; for categories use mark bar or line".into(),
        );
    }
    let color = channels.get("color");
    let (names, at) = match color {
        Some(color) => {
            let (keys, at) = distinct(rows, color)?;
            (keys.iter().map(Key::label).collect(), at)
        }
        None => (vec![y.title()], vec![0; rows.len()]),
    };
    if names.len() > MOST_SERIES {
        return Err(format!("at most {MOST_SERIES} colors"));
    }
    let mut groups: Vec<PointGroup> = names
        .into_iter()
        .map(|name| PointGroup {
            name,
            points: Vec::new(),
        })
        .collect();
    for (i, row) in rows.iter().enumerate() {
        if let (Some(px), Some(py)) = (number(row, i, &x.field)?, number(row, i, &y.field)?) {
            groups[at[i]].points.push(crate::plot::Point {
                x: px,
                y: py,
                z: None,
                label: None,
            });
        }
    }
    groups.retain(|g| !g.points.is_empty());
    let title = format!("{} by {}", y.title(), x.title());
    let mut chart = Chart::plot(
        ChartKind::Scatter,
        title,
        axis(x),
        axis(y),
        Plot::Points {
            groups,
            lines: Vec::new(),
            z_axis: None,
        },
    );
    chart.legend = color.map(Channel::title);
    Ok(chart)
}
