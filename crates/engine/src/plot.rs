//! The data shapes of statistical plots: boxes, stem-and-leaf lines, points with reference
//! lines, merge trees, node trees and regions of the plane. A `Chart` of the statistical tier carries one of them in
//! `plot`; the category axis of the chart is then the horizontal axis and the value axis the
//! vertical one. Each shape gives a data table for the text output and the window, and a
//! Vega-Lite specification for agents.

use crate::chart::{Axis, Chart, ChartKind, Tier, number_text};
use crate::{Cell, Table};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case")]
pub enum Plot {
    /// One box per cell or variable.
    Boxes { boxes: Vec<BoxSummary> },
    /// A stem-and-leaf display.
    StemLeaf(StemLeaf),
    /// Points in groups, with straight reference lines.
    Points {
        groups: Vec<PointGroup>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        lines: Vec<ReferenceLine>,
        /// The third axis when the points have a z coordinate.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        z_axis: Option<Axis>,
    },
    /// A hierarchical clustering: the items, the order to lay them out in, and the merges.
    Merges {
        items: Vec<String>,
        /// Indices into `items`, top to bottom, so that no branches cross.
        order: Vec<usize>,
        merges: Vec<Merge>,
        /// Icicle plot: the numbers of clusters it shows, largest first; empty for all.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        solutions: Vec<usize>,
    },
    /// A tree of nodes; the root comes first, every other node after its parent.
    Tree { nodes: Vec<TreeNode> },
    /// The plane split into regions, each a convex polygon around its center.
    Regions { regions: Vec<Region> },
}

/// A region of the plane: its name, its center (such as a group centroid) and the corners of
/// its polygon in order around it; no corners when the region is empty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub name: String,
    pub center: [f64; 2],
    pub vertices: Vec<[f64; 2]>,
}

/// A box of a boxplot: Tukey's hinges, the median, whiskers to the furthest values that are
/// neither outliers nor extremes, and those cases marked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoxSummary {
    pub label: String,
    /// The cluster the box belongs to, when boxes are grouped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub n: f64,
    pub lower_whisker: f64,
    pub lower_hinge: f64,
    pub median: f64,
    pub upper_hinge: f64,
    pub upper_whisker: f64,
    /// Between 1.5 and 3 box lengths from the box.
    pub outliers: Vec<Mark>,
    /// 3 box lengths or more from the box.
    pub extremes: Vec<Mark>,
}

/// A case marked on a plot: its value and its label (case number or identifier).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    pub value: f64,
    pub case: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StemLeaf {
    /// The value of one step of the stem.
    pub stem_width: f64,
    /// How many cases one leaf stands for.
    pub each_leaf: f64,
    /// Cases left out at the low end, with the bound they lie at or below.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low_extremes: Option<Extremes>,
    pub lines: Vec<StemLine>,
    /// Cases left out at the high end, with the bound they lie at or above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_extremes: Option<Extremes>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extremes {
    pub frequency: f64,
    pub bound: f64,
}

/// A line of a stem-and-leaf display: the (weighted) count, the stem as shown and the leaf
/// digits in ascending order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StemLine {
    pub frequency: f64,
    pub stem: String,
    pub leaves: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PointGroup {
    pub name: String,
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// The line y = intercept + slope x across the plot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceLine {
    pub intercept: f64,
    pub slope: f64,
}

/// Two clusters joined: each named by its first item (the lowest index, as the agglomeration
/// schedule names them), at a height of the clustering criterion and that height rescaled to
/// 0 to 25.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Merge {
    pub left: usize,
    pub right: usize,
    pub height: f64,
    pub rescaled: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<usize>,
    /// The condition that leads from the parent to the node, such as "region = North; South".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub split: String,
    /// What the node shows, in order: statistic name and value.
    pub statistics: Vec<(String, Cell)>,
}

impl Chart {
    /// A statistical plot with its data shape; the axes are horizontal and vertical.
    pub(crate) fn plot(kind: ChartKind, title: String, x: Axis, y: Axis, plot: Plot) -> Chart {
        Chart {
            kind,
            tier: Tier::Statistical,
            orientation: None,
            grouping: None,
            title,
            subtitle: None,
            footnote: None,
            category_axis: x,
            value_axis: y,
            legend: None,
            categories: Vec::new(),
            series: Vec::new(),
            normal: None,
            error_bars: None,
            second_axis: None,
            plot: Some(plot),
            outline: None,
            hidden: false,
        }
    }

    /// A line chart over categories, one series per line: a means plot, a scree plot.
    pub(crate) fn lines(
        title: String,
        x: Axis,
        y: Axis,
        categories: Vec<crate::chart::Category>,
        series: Vec<crate::chart::Series>,
    ) -> Chart {
        Chart {
            kind: ChartKind::Line,
            tier: Tier::Presentation,
            orientation: Some(crate::chart::Orientation::Vertical),
            grouping: None,
            title,
            subtitle: None,
            footnote: None,
            category_axis: x,
            value_axis: y,
            legend: None,
            categories,
            series,
            normal: None,
            error_bars: None,
            second_axis: None,
            plot: None,
            outline: None,
            hidden: false,
        }
    }
}

impl Axis {
    /// An axis with a title only.
    pub(crate) fn titled(title: impl Into<String>) -> Axis {
        Axis {
            title: title.into(),
            format: String::new(),
            maximum: None,
            variable: None,
        }
    }
}

impl Plot {
    /// The plot's data as a table; `chart` gives the titles and formats.
    pub(crate) fn table(&self, chart: &Chart) -> (Vec<String>, Vec<Vec<Cell>>, Vec<Vec<String>>) {
        let (x, y) = (&chart.category_axis, &chart.value_axis);
        let text = |s: &str| Cell::Text(s.to_string());
        let mut columns: Vec<String> = Vec::new();
        let mut rows: Vec<Vec<Cell>> = Vec::new();
        let mut formats: Vec<Vec<String>> = Vec::new();
        match self {
            Plot::Boxes { boxes } => {
                let grouped = boxes.iter().any(|b| b.group.is_some());
                columns.push(x.title.clone());
                if grouped {
                    columns.push(chart.legend.clone().unwrap_or_default());
                }
                columns.extend(
                    [
                        "N",
                        "Lower whisker",
                        "Lower hinge",
                        "Median",
                        "Upper hinge",
                        "Upper whisker",
                        "Outliers",
                        "Extremes",
                    ]
                    .map(String::from),
                );
                let marks = |marks: &[Mark]| {
                    let list: Vec<String> = marks
                        .iter()
                        .map(|m| format!("{} (case {})", number_text(m.value, &y.format), m.case))
                        .collect();
                    Cell::Text(list.join(", "))
                };
                for b in boxes {
                    let mut row = vec![text(&b.label)];
                    if grouped {
                        row.push(text(b.group.as_deref().unwrap_or("")));
                    }
                    let lead = row.len();
                    row.push(Cell::Number(b.n));
                    row.extend(
                        [
                            b.lower_whisker,
                            b.lower_hinge,
                            b.median,
                            b.upper_hinge,
                            b.upper_whisker,
                        ]
                        .map(Cell::Number),
                    );
                    row.push(marks(&b.outliers));
                    row.push(marks(&b.extremes));
                    let mut f = vec![String::new(); row.len()];
                    f[lead] = crate::chart::count_format([b.n]);
                    for cell in &mut f[lead + 1..lead + 6] {
                        cell.clone_from(&y.format);
                    }
                    formats.push(f);
                    rows.push(row);
                }
            }
            Plot::StemLeaf(s) => {
                columns = ["Frequency", "Stem", "Leaf"].map(String::from).to_vec();
                let bound = |e: &Extremes, sign: &str| {
                    let row = vec![
                        Cell::Number(e.frequency),
                        text("Extremes"),
                        Cell::Text(format!("({sign}{})", number_text(e.bound, &x.format))),
                    ];
                    row
                };
                if let Some(e) = &s.low_extremes {
                    rows.push(bound(e, "=<"));
                }
                for line in &s.lines {
                    rows.push(vec![
                        Cell::Number(line.frequency),
                        text(&line.stem),
                        text(&line.leaves),
                    ]);
                }
                if let Some(e) = &s.high_extremes {
                    rows.push(bound(e, ">="));
                }
                formats = rows
                    .iter()
                    .map(|r| {
                        let n = match r[0] {
                            Cell::Number(n) => n,
                            _ => 0.0,
                        };
                        vec![
                            crate::chart::count_format([n]),
                            String::new(),
                            String::new(),
                        ]
                    })
                    .collect();
            }
            Plot::Points { groups, z_axis, .. } => {
                let several = groups.len() > 1;
                let labelled = groups
                    .iter()
                    .flat_map(|g| &g.points)
                    .any(|p| p.label.is_some());
                if several {
                    columns.push(chart.legend.clone().unwrap_or_else(|| "Group".into()));
                }
                if labelled {
                    columns.push(String::new());
                }
                columns.push(x.title.clone());
                columns.push(y.title.clone());
                if let Some(z) = z_axis {
                    columns.push(z.title.clone());
                }
                for g in groups {
                    for p in &g.points {
                        let mut row = Vec::new();
                        let mut f = Vec::new();
                        if several {
                            row.push(text(&g.name));
                            f.push(String::new());
                        }
                        if labelled {
                            row.push(text(p.label.as_deref().unwrap_or("")));
                            f.push(String::new());
                        }
                        row.push(Cell::Number(p.x));
                        f.push(x.format.clone());
                        row.push(Cell::Number(p.y));
                        f.push(y.format.clone());
                        if let Some(z) = z_axis {
                            row.push(p.z.map_or(Cell::Empty, Cell::Number));
                            f.push(z.format.clone());
                        }
                        rows.push(row);
                        formats.push(f);
                    }
                }
            }
            Plot::Merges {
                items,
                order,
                merges,
                solutions,
            } => match chart.kind {
                ChartKind::Icicle => {
                    // a row per number of clusters, a column per item in the order of the plot:
                    // its cluster, numbered from 1 along that order
                    columns.push("Number of clusters".into());
                    columns.extend(order.iter().map(|&i| items[i].clone()));
                    let n = items.len();
                    let mut cluster: Vec<usize> = (0..n).collect();
                    let mut by_count = vec![cluster.clone()];
                    for m in merges {
                        let (keep, gone) = (cluster[m.left], cluster[m.right]);
                        for c in &mut cluster {
                            if *c == gone {
                                *c = keep;
                            }
                        }
                        by_count.push(cluster.clone());
                    }
                    for (step, state) in by_count.iter().enumerate().skip(1).rev() {
                        let k = n - step;
                        if !solutions.is_empty() && !solutions.contains(&k) {
                            continue;
                        }
                        let mut row = vec![Cell::Number(k as f64)];
                        let mut seen: Vec<usize> = Vec::new();
                        for &i in order {
                            let number = match seen.iter().position(|&c| c == state[i]) {
                                Some(at) => at + 1,
                                None => {
                                    seen.push(state[i]);
                                    seen.len()
                                }
                            };
                            row.push(Cell::Number(number as f64));
                        }
                        rows.push(row);
                    }
                }
                _ => {
                    columns = [
                        "Stage",
                        "Cluster 1",
                        "Cluster 2",
                        "Height",
                        "Rescaled height",
                    ]
                    .map(String::from)
                    .to_vec();
                    for (stage, m) in merges.iter().enumerate() {
                        rows.push(vec![
                            Cell::Number(stage as f64 + 1.0),
                            text(&items[m.left]),
                            text(&items[m.right]),
                            Cell::Number(m.height),
                            Cell::Number(m.rescaled),
                        ]);
                        formats.push(["F8.0", "", "", "F12.3", "F8.1"].map(String::from).to_vec());
                    }
                }
            },
            Plot::Tree { nodes } => {
                let mut names: Vec<String> = Vec::new();
                for node in nodes {
                    for (name, _) in &node.statistics {
                        if !names.contains(name) {
                            names.push(name.clone());
                        }
                    }
                }
                columns = ["Node", "Parent", "Split"].map(String::from).to_vec();
                columns.extend(names.iter().cloned());
                for node in nodes {
                    let mut row = vec![
                        Cell::Number(node.id as f64),
                        node.parent.map_or(Cell::Empty, |p| Cell::Number(p as f64)),
                        text(&node.split),
                    ];
                    for name in &names {
                        let value = node.statistics.iter().find(|(n, _)| n == name);
                        row.push(value.map_or(Cell::Empty, |(_, v)| v.clone()));
                    }
                    rows.push(row);
                }
            }
            Plot::Regions { regions } => {
                columns = vec![
                    chart.legend.clone().unwrap_or_else(|| "Region".into()),
                    "Point".into(),
                    x.title.clone(),
                    y.title.clone(),
                ];
                for region in regions {
                    let corners = region.vertices.iter().enumerate();
                    let points = std::iter::once(("Center".to_string(), region.center))
                        .chain(corners.map(|(k, v)| (format!("Corner {}", k + 1), *v)));
                    for (point, [px, py]) in points {
                        rows.push(vec![
                            text(&region.name),
                            Cell::Text(point),
                            Cell::Number(px),
                            Cell::Number(py),
                        ]);
                        formats.push(vec![
                            String::new(),
                            String::new(),
                            x.format.clone(),
                            y.format.clone(),
                        ]);
                    }
                }
            }
        }
        (columns, rows, formats)
    }

    /// The plot as Vega-Lite layers and data; `chart` gives the titles.
    pub(crate) fn vega_lite(&self, chart: &Chart) -> Map<String, Value> {
        let (xt, yt) = (&chart.category_axis.title, &chart.value_axis.title);
        let mut out = Map::new();
        match self {
            Plot::Boxes { boxes } => {
                let rows: Vec<Value> = boxes
                    .iter()
                    .map(|b| {
                        json!({
                            "box": box_name(b),
                            "low": b.lower_whisker, "q1": b.lower_hinge, "median": b.median,
                            "q3": b.upper_hinge, "high": b.upper_whisker, "n": b.n,
                        })
                    })
                    .collect();
                let mut marks = Vec::new();
                for b in boxes {
                    for (kind, list) in [("outlier", &b.outliers), ("extreme", &b.extremes)] {
                        marks.extend(list.iter().map(|m| {
                            json!({ "box": box_name(b), "value": m.value, "case": m.case, "kind": kind })
                        }));
                    }
                }
                let order: Vec<String> = boxes.iter().map(box_name).collect();
                let x = json!({ "field": "box", "type": "nominal", "sort": order, "title": xt });
                let y =
                    |field: &str| json!({ "field": field, "type": "quantitative", "title": yt });
                let layers = json!([
                    { "data": { "values": rows }, "mark": "rule",
                      "encoding": { "x": x, "y": y("low"), "y2": { "field": "high" } } },
                    { "data": { "values": rows }, "mark": { "type": "bar", "size": 20 },
                      "encoding": { "x": x, "y": y("q1"), "y2": { "field": "q3" } } },
                    { "data": { "values": rows }, "mark": { "type": "tick", "size": 20, "color": "black" },
                      "encoding": { "x": x, "y": y("median") } },
                    { "data": { "values": marks }, "mark": { "type": "point" },
                      "encoding": {
                          "x": x, "y": y("value"),
                          "shape": { "field": "kind", "type": "nominal",
                                     "scale": { "domain": ["outlier", "extreme"], "range": ["circle", "cross"] } },
                          "tooltip": [{ "field": "case" }, { "field": "value" }] } },
                    { "data": { "values": marks }, "mark": { "type": "text", "dx": 8, "align": "left" },
                      "encoding": { "x": x, "y": y("value"), "text": { "field": "case" } } },
                ]);
                out.insert("layer".into(), layers);
            }
            Plot::StemLeaf(_) => {
                let (_, rows, _) = self.table(chart);
                let lines: Vec<Value> = rows
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        let cell = |c: &Cell| match c {
                            Cell::Text(t) => t.clone(),
                            Cell::Number(n) => number_text(*n, ""),
                            Cell::Empty => String::new(),
                        };
                        json!({ "line": i, "text": format!("{:>8} {:>8} . {}", cell(&r[0]), cell(&r[1]), cell(&r[2])) })
                    })
                    .collect();
                out.insert("data".into(), json!({ "values": lines }));
                out.insert(
                    "mark".into(),
                    json!({ "type": "text", "align": "left", "font": "monospace" }),
                );
                out.insert(
                    "encoding".into(),
                    json!({
                        "y": { "field": "line", "type": "ordinal", "axis": null },
                        "text": { "field": "text" },
                    }),
                );
            }
            Plot::Points { groups, lines, .. } => {
                let roc = chart.kind == ChartKind::Roc;
                let rows: Vec<Value> = groups
                    .iter()
                    .flat_map(|g| {
                        g.points.iter().enumerate().map(move |(k, p)| {
                            let mut row = json!({ "group": g.name, "x": p.x, "y": p.y });
                            if roc {
                                row["order"] = k.into();
                            }
                            if let Some(z) = p.z {
                                row["z"] = z.into();
                            }
                            if let Some(label) = &p.label {
                                row["label"] = label.as_str().into();
                            }
                            row
                        })
                    })
                    .collect();
                let x = json!({ "field": "x", "type": "quantitative", "title": xt, "scale": { "zero": false } });
                let y = json!({ "field": "y", "type": "quantitative", "title": yt, "scale": { "zero": false } });
                let mark = match chart.kind {
                    ChartKind::Roc => json!({ "type": "line", "point": true }),
                    _ => json!({ "type": "point" }),
                };
                let mut points = json!({
                    "data": { "values": rows },
                    "mark": mark,
                    "encoding": { "x": x, "y": y },
                });
                if roc {
                    // the curve runs through the points in their order, also up a vertical step
                    points["encoding"]["order"] =
                        json!({ "field": "order", "type": "quantitative" });
                }
                if groups.len() > 1 {
                    points["encoding"]["color"] = json!({
                        "field": "group", "type": "nominal",
                        "sort": groups.iter().map(|g| g.name.as_str()).collect::<Vec<_>>(),
                        "title": chart.legend,
                    });
                }
                let mut layers = vec![points];
                if groups
                    .iter()
                    .flat_map(|g| &g.points)
                    .any(|p| p.label.is_some())
                {
                    layers.push(json!({
                        "data": { "values": rows },
                        "mark": { "type": "text", "dx": 6, "align": "left" },
                        "encoding": { "x": x, "y": y, "text": { "field": "label" } },
                    }));
                }
                let xs = groups.iter().flat_map(|g| &g.points).map(|p| p.x);
                let (low, high) = xs.fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), x| {
                    (a.min(x), b.max(x))
                });
                if low.is_finite() {
                    for line in lines {
                        let ends: Vec<Value> = [low, high]
                            .iter()
                            .map(|&x| json!({ "x": x, "y": line.intercept + line.slope * x }))
                            .collect();
                        layers.push(json!({
                            "data": { "values": ends },
                            "mark": { "type": "line" },
                            "encoding": { "x": x, "y": y },
                        }));
                    }
                }
                out.insert("layer".into(), Value::Array(layers));
            }
            Plot::Merges {
                items,
                order,
                merges,
                ..
            } => {
                let position = |i: usize| order.iter().position(|&o| o == i).unwrap_or(0) as f64;
                let labels: Vec<Value> = order
                    .iter()
                    .enumerate()
                    .map(|(p, &i)| json!({ "y": p, "label": items[i] }))
                    .collect();
                let mut segments = Vec::new();
                // every cluster: where it sits and the height it was formed at
                let mut place: Vec<(f64, f64)> =
                    (0..items.len()).map(|i| (position(i), 0.0)).collect();
                for m in merges {
                    let (a, b) = (place[m.left], place[m.right]);
                    let h = m.rescaled;
                    segments.push(json!({ "x": a.1, "x2": h, "y": a.0, "y2": a.0 }));
                    segments.push(json!({ "x": b.1, "x2": h, "y": b.0, "y2": b.0 }));
                    segments.push(json!({ "x": h, "x2": h, "y": a.0, "y2": b.0 }));
                    place[m.left] = ((a.0 + b.0) / 2.0, h);
                }
                // icicle: a bar over each run of items in one cluster, per number of clusters
                let mut rects = Vec::new();
                if chart.kind == ChartKind::Icicle {
                    let (_, rows, _) = self.table(chart);
                    for row in rows {
                        let mut start = 1;
                        for end in 2..=row.len() {
                            if end == row.len() || row[end] != row[start] {
                                rects.push(json!({
                                    "clusters": row[0],
                                    "from": start as f64 - 1.4,
                                    "to": end as f64 - 1.6,
                                }));
                                start = end;
                            }
                        }
                    }
                }
                let y = json!({ "field": "y", "type": "quantitative", "axis": null });
                let layers = match chart.kind {
                    ChartKind::Icicle => json!([
                        { "data": { "values": rects }, "mark": "bar",
                          "encoding": {
                              "x": { "field": "clusters", "type": "ordinal", "title": xt, "sort": "descending" },
                              "y": { "field": "from", "type": "quantitative", "axis": null },
                              "y2": { "field": "to" } } },
                        { "data": { "values": labels }, "mark": { "type": "text", "align": "right", "dx": -4 },
                          "encoding": { "y": y, "text": { "field": "label" } } },
                    ]),
                    _ => json!([
                        { "data": { "values": segments }, "mark": "rule",
                          "encoding": {
                              "x": { "field": "x", "type": "quantitative", "title": xt, "scale": { "domain": [0, 25] } },
                              "x2": { "field": "x2" }, "y": y, "y2": { "field": "y2" } } },
                        { "data": { "values": labels }, "mark": { "type": "text", "align": "right", "dx": -4 },
                          "encoding": { "x": { "datum": 0, "type": "quantitative" }, "y": y, "text": { "field": "label" } } },
                    ]),
                };
                out.insert("layer".into(), layers);
            }
            Plot::Tree { nodes } => {
                // leaves side by side, a parent above the middle of its children
                let children = |id: usize| nodes.iter().filter(move |n| n.parent == Some(id));
                let mut x = vec![0.0; nodes.len()];
                let mut depth = vec![0usize; nodes.len()];
                let index = |id: usize| nodes.iter().position(|n| n.id == id).unwrap_or(0);
                for (i, node) in nodes.iter().enumerate() {
                    depth[i] = node.parent.map_or(0, |p| depth[index(p)] + 1);
                }
                let mut next = 0.0;
                fn place(
                    i: usize,
                    x: &mut [f64],
                    next: &mut f64,
                    children: &dyn Fn(usize) -> Vec<usize>,
                ) {
                    let kids = children(i);
                    if kids.is_empty() {
                        x[i] = *next;
                        *next += 1.0;
                        return;
                    }
                    for &k in &kids {
                        place(k, x, next, children);
                    }
                    x[i] = kids.iter().map(|&k| x[k]).sum::<f64>() / kids.len() as f64;
                }
                let kids = |i: usize| -> Vec<usize> {
                    children(nodes[i].id).map(|n| index(n.id)).collect()
                };
                if !nodes.is_empty() {
                    place(0, &mut x, &mut next, &kids);
                }
                let points: Vec<Value> = nodes
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        let mut text = vec![format!("Node {}", n.id)];
                        text.extend(n.statistics.iter().map(|(name, v)| match v {
                            Cell::Number(v) => format!("{name}: {}", number_text(*v, "")),
                            Cell::Text(t) => format!("{name}: {t}"),
                            Cell::Empty => name.clone(),
                        }));
                        json!({ "x": x[i], "depth": depth[i], "text": text, "split": n.split })
                    })
                    .collect();
                let edges: Vec<Value> = nodes
                    .iter()
                    .enumerate()
                    .filter_map(|(i, n)| {
                        let p = index(n.parent?);
                        Some(json!({ "x": x[p], "x2": x[i], "depth": depth[p], "depth2": depth[i], "split": n.split }))
                    })
                    .collect();
                let y = json!({ "field": "depth", "type": "quantitative", "scale": { "reverse": true }, "axis": null });
                let xa = json!({ "field": "x", "type": "quantitative", "axis": null });
                out.insert(
                    "layer".into(),
                    json!([
                        { "data": { "values": edges }, "mark": "rule",
                          "encoding": { "x": xa, "x2": { "field": "x2" }, "y": y, "y2": { "field": "depth2" } } },
                        { "data": { "values": points }, "mark": { "type": "text", "baseline": "top" },
                          "encoding": { "x": xa, "y": y, "text": { "field": "text" } } },
                    ]),
                );
            }
            Plot::Regions { regions } => {
                // each region's outline, closed, in its color; the centers as labelled crosses
                let mut corners = Vec::new();
                for region in regions {
                    let closed = region.vertices.iter().chain(region.vertices.first());
                    for (k, [px, py]) in closed.enumerate() {
                        corners
                            .push(json!({ "region": region.name, "order": k, "x": px, "y": py }));
                    }
                }
                let centers: Vec<Value> = regions
                    .iter()
                    .map(|r| json!({ "region": r.name, "x": r.center[0], "y": r.center[1] }))
                    .collect();
                let names: Vec<&str> = regions.iter().map(|r| r.name.as_str()).collect();
                let x = json!({ "field": "x", "type": "quantitative", "title": xt, "scale": { "zero": false } });
                let y = json!({ "field": "y", "type": "quantitative", "title": yt, "scale": { "zero": false } });
                let color = json!({ "field": "region", "type": "nominal", "sort": names, "title": chart.legend });
                out.insert(
                    "layer".into(),
                    json!([
                        { "data": { "values": corners }, "mark": "line",
                          "encoding": { "x": x, "y": y, "color": color,
                                        "order": { "field": "order", "type": "quantitative" } } },
                        { "data": { "values": centers }, "mark": { "type": "point", "shape": "cross" },
                          "encoding": { "x": x, "y": y, "color": color } },
                        { "data": { "values": centers }, "mark": { "type": "text", "dx": 8, "align": "left" },
                          "encoding": { "x": x, "y": y, "text": { "field": "region" } } },
                    ]),
                );
            }
        }
        out
    }
}

/// The part of a convex polygon where a x + b >= 0 (one step of Sutherland and Hodgman's
/// clipping).
pub(crate) fn clip(polygon: &[[f64; 2]], a: [f64; 2], b: f64) -> Vec<[f64; 2]> {
    let side = |p: [f64; 2]| a[0] * p[0] + a[1] * p[1] + b;
    let mut out = Vec::new();
    for (k, &p) in polygon.iter().enumerate() {
        let q = polygon[(k + 1) % polygon.len()];
        let (sp, sq) = (side(p), side(q));
        if sp >= 0.0 {
            out.push(p);
        }
        if (sp >= 0.0) != (sq >= 0.0) {
            let t = sp / (sp - sq);
            out.push([p[0] + t * (q[0] - p[0]), p[1] + t * (q[1] - p[1])]);
        }
    }
    out
}

fn box_name(b: &BoxSummary) -> String {
    match &b.group {
        Some(group) => format!("{} {group}", b.label),
        None => b.label.clone(),
    }
}

impl Table {
    /// Wraps plot data into a table titled like its chart.
    pub(crate) fn of_plot(
        chart: &Chart,
        (columns, rows, formats): (Vec<String>, Vec<Vec<Cell>>, Vec<Vec<String>>),
        caption: String,
    ) -> Table {
        let formats = match formats.iter().flatten().all(String::is_empty) {
            true => Vec::new(),
            false => formats,
        };
        Table {
            title: chart.title.clone(),
            row_headers: 1,
            column_groups: vec![Vec::new(); columns.len()],
            columns,
            rows,
            formats,
            caption: Some(caption),
            outline: None,
            hidden: false,
        }
    }
}
