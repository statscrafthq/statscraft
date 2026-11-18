//! XGRAPH: the chart expression laid out as a GRAPH chart, the dimensions that do not fit on
//! the category axis or the series becoming panels; dot plots and binned histograms added here.
//!
//! Rules taken from the manual:
//! - y holds variables with a summary function or count functions without one, blended with +
//!   and named by the placeholder 1 where they form categories, clusters or stacks. VALUE
//!   draws every case, $CASENUM numbering them on an axis, LABEL naming them.
//! - The sign > after y stacks, after an x or z variable clusters. BAR draws summaries,
//!   HISTOBAR counts the cases in bins of a scale variable, POINT draws each case as a dot at
//!   its value.
//! - COORDINATE SPLIT=YES makes a population pyramid split by the variable after the last BY;
//!   a HISTOBAR chart is always split by it. BIN sets the bins of HISTOBAR, DISTRIBUTION adds
//!   the normal curve, DISPLAY stacks the dots.
//! - ERRORBAR: CI(level), STDDEV(n) or SE(n) around counts (and the summaries GRAPH has bars
//!   for). MISSING USE=LISTWISE drops a case missing on any y variable, REPORT=YES keeps the
//!   categories of missing values.
//!
//! Where the manual is silent (recorded in docs/differences.md):
//! - The z axis is not drawn in depth: z gives the series when nothing else does, and every
//!   dimension beyond the category axis and the series gives a chart per category (panel).
//! - A pyramid is drawn side by side, not back to back; a HISTOBAR pyramid as a histogram per
//!   side with the same bins.
//! - LINE and BOX elements are drawn as GRAPH and IGRAPH draw them.

use super::{Context, numeric, valid};
use crate::chart::{Axis, Category, Chart, ChartKind, Grouping, Orientation, Series};
use crate::descriptives::case_weights;
use crate::frequencies::{Histogram, bins};
use crate::plot::{Plot, Point, PointGroup};
use crate::{Engine, Error, Table};
use statscraft_data::Dataset;
use statscraft_syntax::{
    Graph, GraphChart, GraphData, GraphPanel, GraphSummary, VarItem, Xgraph, XgraphBinSize,
    XgraphDim as D, XgraphDot, XgraphElement as E, XgraphStat,
};

fn fail<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error(format!("XGRAPH: {}", message.into())))
}

/// The GRAPH command and what is changed on its charts.
struct Plan {
    graph: Graph,
    /// The y variables (the placeholder 1) go along the x axis, where GRAPH puts them in series.
    transpose: bool,
    stacked: bool,
    horizontal: bool,
}

/// The dimensions after the category axis, in the order they take the series: the stack, the
/// cluster of x, z and the cluster of z.
fn others(c: &Xgraph) -> Vec<(&'static str, &D)> {
    let x_cluster = c.x.as_ref().and_then(|a| a.cluster.as_ref());
    let z = c.z.as_ref().map(|a| &a.dim);
    let z_cluster = c.z.as_ref().and_then(|a| a.cluster.as_ref());
    [
        ("the stack", c.stack.as_ref()),
        ("the cluster of x", x_cluster),
        ("z", z),
        ("the cluster of z", z_cluster),
    ]
    .into_iter()
    .filter_map(|(role, dim)| Some((role, dim?)))
    .collect()
}

/// A dimension that becomes a panel: a variable; the placeholder and the cases cannot.
fn panel_of(role: &str, dim: &D) -> Result<String, Error> {
    match dim {
        D::Var { name, .. } => Ok(name.clone()),
        D::One => fail(format!(
            "the placeholder 1 cannot be {role} here; put it on the x axis or where the series go"
        )),
        D::Cases { .. } => fail(format!(
            "$CASENUM cannot be {role}; it goes on the x or z axis with [VALUE]"
        )),
    }
}

/// The variable of a dimension that must be one.
fn var_of<'a>(dim: Option<&'a D>, what: &str) -> Result<&'a str, Error> {
    match dim {
        Some(D::Var { name, .. }) => Ok(name),
        _ => fail(format!("{what} needs a variable after BY")),
    }
}

/// The y items of a chart that draws cases or bins: no variables and no function.
fn bare(c: &Xgraph, what: &str) -> Result<(), Error> {
    match &c.y[..] {
        [item] if item.variables.is_empty() && item.stat.is_none() => Ok(()),
        _ => fail(format!(
            "{what} takes no y variable or function: write ([{what}]) BY var"
        )),
    }
}

/// The chart expression as words, for )CHART: the variables, by their labels where they have
/// them, without functions and markers.
fn expression(c: &Xgraph, data: &Dataset) -> String {
    let named = |name: &str| {
        data.variable(name)
            .ok()
            .and_then(|v| v.label().map(String::from))
            .unwrap_or_else(|| name.to_string())
    };
    let dim = |d: &D| match d {
        D::Var { name, .. } => named(name),
        D::One => "1".into(),
        D::Cases { .. } => "$CASENUM".into(),
    };
    let y: Vec<String> =
        c.y.iter()
            .flat_map(|i| &i.variables)
            .map(|n| named(n))
            .collect();
    let mut parts = Vec::new();
    if !y.is_empty() {
        parts.push(y.join(" + "));
    }
    for axis in [&c.x, &c.z].into_iter().flatten() {
        let mut text = dim(&axis.dim);
        if let Some(cluster) = &axis.cluster {
            text = format!("{text} > {}", dim(cluster));
        }
        parts.push(text);
    }
    parts.join(" BY ")
}

/// )DATE, )TIME and )CHART in a line of text.
fn fill(text: &str, chart: &str) -> String {
    let now = chrono::Local::now();
    text.replace(")DATE", &now.format("%d-%b-%Y").to_string())
        .replace(")TIME", &now.format("%H:%M:%S").to_string())
        .replace(")CHART", chart)
}

/// The start, width and number (when fixed) of the bins of HISTOBAR over all valid values.
fn bin_layout(c: &Xgraph, data: &Dataset, name: &str) -> Result<(f64, f64, Option<usize>), Error> {
    let variable = numeric(data, name, "HISTOBAR")?;
    let weights = case_weights(data)?;
    let mut pairs: Vec<(f64, f64)> = (0..data.case_count())
        .filter_map(|case| {
            let w = weights.as_ref().map_or(1.0, |w| w[case]);
            (w > 0.0).then_some((valid(&variable, case, false)?, w))
        })
        .collect();
    if pairs.is_empty() {
        return fail(format!("{name} has no valid values to bin"));
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (min, max) = (pairs[0].0, pairs[pairs.len() - 1].0);
    let (start, size) = c.bin.unwrap_or_default();
    Ok(match size {
        XgraphBinSize::Auto => {
            let (first, width, _) = bins(&pairs);
            (start.unwrap_or(first * width), width, None)
        }
        XgraphBinSize::Width(w) => (start.unwrap_or((min / w).floor() * w), w, None),
        XgraphBinSize::Count(n) => {
            let start = start.unwrap_or(min);
            let width = match max > start {
                true => (max - start) / n as f64,
                false => 1.0,
            };
            (start, width, Some(n))
        }
    })
}

fn plan(c: &Xgraph, data: &Dataset, warnings: &mut Vec<String>) -> Result<Plan, Error> {
    let x = c.x.as_ref().map(|a| &a.dim);
    let mut panels: Vec<String> = Vec::new();
    let (mut transpose, mut stacked, mut horizontal) = (false, false, false);
    let pyramid = c.split || c.element == E::Histobar;
    if c.split && !matches!(c.element, E::Bar | E::Histobar) {
        return fail("COORDINATE SPLIT=YES makes a population pyramid of [BAR] or [HISTOBAR]");
    }
    if pyramid && c.panel.as_ref().is_some_and(|p| !p.rows.is_empty()) {
        return fail("a population pyramid takes PANEL COLVAR only, not ROWVAR");
    }
    if c.interval.is_some() && !matches!(c.element, E::Bar | E::Line) {
        return fail("ERRORBAR draws bars around the summaries of [BAR] and [LINE] only");
    }
    for (given, element, what) in [
        (c.bin.is_some(), E::Histobar, "BIN"),
        (c.normal, E::Histobar, "DISTRIBUTION"),
        (c.dot.is_some(), E::Point, "DISPLAY"),
    ] {
        if given && c.element != element {
            warnings.push(format!(
                "{what} does not apply to this chart and is ignored"
            ));
        }
    }
    if c.template {
        warnings.push("TEMPLATE only changes the look of the chart and is ignored".into());
    }
    let chart = match c.element {
        E::Point => {
            bare(c, "POINT")?;
            let variable = var_of(x, "[POINT]")?.to_string();
            for (role, dim) in others(c) {
                if role == "the stack" {
                    return fail("dots do not stack; a variable after > on y needs [BAR]");
                }
                panels.push(panel_of(role, dim)?);
            }
            GraphChart::Dots {
                variable,
                dot: c.dot.unwrap_or_default(),
            }
        }
        E::Histobar => {
            bare(c, "HISTOBAR")?;
            let variable = var_of(x, "[HISTOBAR]")?.to_string();
            if matches!(
                x,
                Some(D::Var {
                    scale: Some(false),
                    ..
                })
            ) {
                return fail(
                    "[HISTOBAR] bins a scale variable; a categorical one takes [COUNT] [BAR]",
                );
            }
            for (role, dim) in others(c) {
                if role != "z" {
                    return fail(format!(
                        "a [HISTOBAR] chart takes no stack or cluster; {role} cannot be drawn"
                    ));
                }
                // the splitter: a histogram for each side
                panels.push(panel_of(role, dim)?);
            }
            let (start, width, bins) = bin_layout(c, data, &variable)?;
            GraphChart::Binned {
                variable,
                normal: c.normal,
                start,
                width,
                bins,
            }
        }
        E::Box => {
            let variable = match &c.y[..] {
                [item] if item.variables.len() == 1 && item.stat.is_none() => {
                    item.variables[0].clone()
                }
                _ => {
                    return fail(
                        "[BOX] draws the values of one variable without a function: write \
                         (var [BOX]) BY var",
                    );
                }
            };
            let mut by: Vec<String> = Vec::new();
            if let Some(dim) = x {
                by.push(panel_of("the x axis", dim)?);
            }
            for (role, dim) in others(c) {
                if role == "the stack" {
                    return fail("boxes do not stack; cluster them with > after x instead");
                }
                match by.len() < 2 {
                    true => by.push(panel_of(role, dim)?),
                    false => panels.push(panel_of(role, dim)?),
                }
            }
            GraphChart::Boxes {
                variable,
                by,
                outliers: true,
                extremes: true,
            }
        }
        E::Bar | E::Line => {
            let data = match c.y.iter().any(|i| i.stat == Some(XgraphStat::Value)) {
                true => values(c, &mut panels, &mut transpose, &mut stacked)?,
                false => summaries(c, &mut panels, &mut transpose, &mut stacked)?,
            };
            if c.split {
                horizontal = true;
                if !matches!(c.z.as_ref().map(|a| &a.dim), Some(D::Var { .. })) {
                    return fail(
                        "a population pyramid needs a splitter variable after the second BY",
                    );
                }
                if others(c).len() > 1 {
                    return fail("a population pyramid takes no stack or cluster");
                }
            }
            match c.element {
                E::Line if stacked => {
                    return fail("lines do not stack; use [BAR] for a stacked chart");
                }
                E::Line => GraphChart::Line { area: false, data },
                _ => GraphChart::Bar { stacked, data },
            }
        }
    };
    let mut panel = GraphPanel {
        columns: panels,
        rows: Vec::new(),
    };
    if let Some(given) = &c.panel {
        panel.columns.extend(given.columns.iter().cloned());
        panel.rows.extend(given.rows.iter().cloned());
    }
    let text = expression(c, data);
    let graph = Graph {
        title: c.title.iter().map(|t| fill(t, &text)).collect(),
        subtitle: c.subtitle.as_ref().map(|t| fill(t, &text)),
        footnote: c.footnote.iter().map(|t| fill(t, &text)).collect(),
        chart,
        listwise: c.listwise,
        report: c.report,
        include: false,
        panel: (!panel.columns.is_empty() || !panel.rows.is_empty()).then_some(panel),
        interval: c.interval,
    };
    Ok(Plan {
        graph,
        transpose,
        stacked,
        horizontal,
    })
}

/// VALUE: the cases on the axis that holds $CASENUM, the variables in series when the
/// placeholder 1 names them, other variables in panels.
fn values(
    c: &Xgraph,
    panels: &mut Vec<String>,
    transpose: &mut bool,
    stacked: &mut bool,
) -> Result<GraphData, Error> {
    if !c
        .y
        .iter()
        .all(|i| i.stat == Some(XgraphStat::Value) && !i.variables.is_empty())
    {
        return fail("[VALUE] goes with every y variable and with no other function");
    }
    let x = c.x.as_ref().map(|a| &a.dim);
    let z = c.z.as_ref().map(|a| &a.dim);
    let (label, other) = match (x, z) {
        (Some(D::Cases { label }), other) => (label, other),
        (other, Some(D::Cases { label })) => (label, other),
        _ => return fail("[VALUE] draws single cases: put $CASENUM on the x or z axis"),
    };
    let mut dims: Vec<(&str, &D)> = Vec::new();
    if let Some(stack) = &c.stack {
        dims.push(("the stack", stack));
    }
    dims.extend(
        c.x.as_ref()
            .and_then(|a| a.cluster.as_ref())
            .map(|d| ("the cluster of x", d)),
    );
    dims.extend(other.map(|d| ("an axis", d)));
    dims.extend(
        c.z.as_ref()
            .and_then(|a| a.cluster.as_ref())
            .map(|d| ("the cluster of z", d)),
    );
    let mut one = false;
    for (role, dim) in dims {
        match dim {
            D::One if !one => {
                one = true;
                *stacked = role == "the stack";
                *transpose = matches!(x, Some(D::One));
            }
            _ => panels.push(panel_of(role, dim)?),
        }
    }
    let variables: Vec<VarItem> =
        c.y.iter()
            .flat_map(|i| &i.variables)
            .map(|n| VarItem::Name(n.clone()))
            .collect();
    if variables.len() > 1 && !one {
        return fail(
            "several y variables need the placeholder 1 on an axis, a cluster or the stack",
        );
    }
    Ok(GraphData::Values {
        variables,
        by: label.clone(),
    })
}

/// Summary functions: x along the axis, the first of the stack, the clusters and z in series,
/// the rest in panels.
fn summaries(
    c: &Xgraph,
    panels: &mut Vec<String>,
    transpose: &mut bool,
    stacked: &mut bool,
) -> Result<GraphData, Error> {
    let mut functions = Vec::new();
    let mut items = 0;
    for item in &c.y {
        let named = item.variables.first().map_or("a count", |v| v.as_str());
        let Some(XgraphStat::Summary(function)) = item.stat else {
            return fail(format!(
                "{named} needs a function in brackets, such as [MEAN] or [COUNT]"
            ));
        };
        match (function.counts_cases(), item.variables.is_empty()) {
            (true, false) => {
                return fail(format!(
                    "{named}: a count function counts cases and takes no variable"
                ));
            }
            (false, true) => return fail("a summary function needs a variable before it"),
            _ => {}
        }
        items += item.variables.len().max(1);
        functions.push(GraphSummary {
            function,
            variables: item
                .variables
                .iter()
                .map(|n| VarItem::Name(n.clone()))
                .collect(),
        });
    }
    let Some(x) = c.x.as_ref().map(|a| &a.dim) else {
        return fail("a chart of summaries needs BY and a variable for the x axis");
    };
    let mut rest = others(c).into_iter();
    let series = match c.split {
        true => c.z.as_ref().map(|a| ("z", &a.dim)),
        false => rest.next(),
    };
    for (role, dim) in rest {
        if !c.split {
            panels.push(panel_of(role, dim)?);
        }
    }
    *stacked = series.is_some_and(|(role, _)| role == "the stack");
    let mut by = Vec::new();
    match (x, series) {
        (D::One, Some((_, D::One))) => return fail("the placeholder 1 can stand once"),
        (D::One, series) => {
            *transpose = series.is_some();
            by.extend(series.map(|(role, d)| panel_of(role, d)).transpose()?);
        }
        (x, Some((_, D::One))) => by.push(panel_of("the x axis", x)?),
        (x, series) => {
            if items > 1 {
                return fail(
                    "several y variables need the placeholder 1 on the x axis, a cluster, z or the stack",
                );
            }
            by.push(panel_of("the x axis", x)?);
            by.extend(series.map(|(role, d)| panel_of(role, d)).transpose()?);
        }
    }
    Ok(GraphData::Summary { functions, by })
}

/// Categories become series and series categories, error bars with them.
fn transpose(chart: &mut Chart) {
    let n = chart.categories.len();
    let statistic = chart.series[0].statistic;
    let categories: Vec<Category> = chart
        .series
        .iter()
        .map(|s| Category {
            label: s.name.clone(),
            value: s.value.clone(),
            missing: false,
            bin: None,
        })
        .collect();
    let series: Vec<Series> = (0..n)
        .map(|k| Series {
            name: chart.categories[k].label.clone(),
            statistic,
            variable: chart.category_axis.variable.clone(),
            value: chart.categories[k].value.clone(),
            values: chart.series.iter().map(|s| s.values[k]).collect(),
        })
        .collect();
    let flip = |rows: &[Vec<f64>]| -> Vec<Vec<f64>> {
        (0..n)
            .map(|k| {
                rows.iter()
                    .map(|r| r.get(k).copied().unwrap_or(f64::NAN))
                    .collect()
            })
            .collect()
    };
    if let Some(bars) = &mut chart.error_bars {
        if !bars.lower.is_empty() {
            // a series without its own lengths below is symmetric
            let lower: Vec<Vec<f64>> = (0..bars.margins.len())
                .map(|s| match bars.lower.get(s).filter(|l| !l.is_empty()) {
                    Some(l) => l.clone(),
                    None => bars.margins[s].clone(),
                })
                .collect();
            bars.lower = flip(&lower);
        }
        bars.margins = flip(&bars.margins);
    }
    let legend = chart.legend.take().unwrap_or_default();
    chart.legend = Some(chart.category_axis.title.clone());
    chart.category_axis = Axis::titled(legend);
    chart.categories = categories;
    chart.series = series;
}

impl Engine {
    pub(crate) fn xgraph(&mut self, command: &Xgraph) -> Result<Vec<Table>, Error> {
        let mut warnings = Vec::new();
        let plan = plan(command, self.active()?, &mut warnings)?;
        let start = self.charts.len();
        self.graph(&plan.graph)
            .map_err(|e| Error(e.0.replacen("GRAPH:", "XGRAPH:", 1)))?;
        for chart in &mut self.charts[start..] {
            if plan.transpose && !chart.series.is_empty() {
                transpose(chart);
            }
            if plan.stacked && chart.series.len() > 1 {
                chart.grouping = Some(Grouping::Stacked);
            }
            if plan.horizontal {
                chart.orientation = Some(Orientation::Horizontal);
            }
        }
        for warning in warnings {
            self.warn(format!("XGRAPH: {warning}"));
        }
        Ok(Vec::new())
    }
}

impl Context<'_> {
    /// A dot per case at its value; the dots of equal values stack upwards from 1
    /// (ASYMMETRIC), around 0 (SYMMETRIC) or not at all (FLAT). Unweighted, as GRAPH draws
    /// cases.
    pub(super) fn dots(&self, name: &str, dot: XgraphDot) -> Result<Chart, Error> {
        let variable = numeric(self.data, name, "a dot plot")?;
        let mut values: Vec<f64> = (0..self.data.case_count())
            .filter(|&case| self.weight(case) > 0.0)
            .filter_map(|case| valid(&variable, case, self.command.include))
            .collect();
        if values.is_empty() {
            return Err(Error("GRAPH: no valid cases to chart".into()));
        }
        values.sort_by(f64::total_cmp);
        let mut points = Vec::new();
        for run in values.chunk_by(|a, b| a == b) {
            let k = run.len() as f64;
            for (j, &x) in run.iter().enumerate() {
                let y = match dot {
                    XgraphDot::Asymmetric => j as f64 + 1.0,
                    XgraphDot::Symmetric => j as f64 + 1.0 - (k + 1.0) / 2.0,
                    XgraphDot::Flat => 0.0,
                };
                points.push(Point {
                    x,
                    y,
                    z: None,
                    label: None,
                });
            }
        }
        let height = match dot {
            XgraphDot::Asymmetric => "Count",
            _ => "",
        };
        Ok(Chart::plot(
            ChartKind::Scatter,
            self.title(format!("Dot plot of {}", self.outline.variable(&variable))),
            self.axis(&variable, true),
            Axis::titled(height),
            Plot::Points {
                groups: vec![PointGroup {
                    name: self.shown.variable(&variable),
                    points,
                }],
                lines: Vec::new(),
                z_axis: None,
            },
        ))
    }

    /// A histogram with the bins given: values below `start` are left out; with `bins`, the
    /// last bin also holds its upper bound.
    pub(super) fn binned(
        &self,
        name: &str,
        normal: bool,
        start: f64,
        width: f64,
        bins: Option<usize>,
    ) -> Result<Chart, Error> {
        let variable = numeric(self.data, name, "HISTOBAR")?;
        let mut pairs: Vec<(f64, f64)> = (0..self.data.case_count())
            .filter(|&case| self.weight(case) > 0.0)
            .filter_map(|case| {
                let value = valid(&variable, case, self.command.include)?;
                (value >= start - 1e-9 * width).then(|| (value, self.weight(case)))
            })
            .collect();
        if pairs.is_empty() {
            return Err(Error(format!("GRAPH: {name} has no values in the bins")));
        }
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let index = |v: f64| {
            let i = ((v - start) / width + 1e-9).floor().max(0.0) as usize;
            bins.map_or(i, |n| i.min(n - 1))
        };
        let mut counts = vec![0.0; bins.unwrap_or(index(pairs[pairs.len() - 1].0) + 1)];
        for &(v, w) in &pairs {
            counts[index(v)] += w;
        }
        let total: f64 = pairs.iter().map(|p| p.1).sum();
        let title = self.title(self.outline.variable(&variable));
        let heading = self.shown.variable(&variable);
        Histogram {
            title: &title,
            heading: &heading,
            format: variable.format(),
            name: variable.name(),
            normal,
            scale: None,
        }
        .binned((start / width, width, counts), &pairs, total)
        .map_err(|e| Error(format!("GRAPH: {}: {e}", variable.name())))
    }
}
