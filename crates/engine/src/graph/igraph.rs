//! IGRAPH: the older chart command, run as the GRAPH chart that matches its variables and
//! elements, with boxes, fit lines, category orders and axis titles added here.
//!
//! Rules taken from the manual:
//! - BAR, LINE and AREA summarize Y (a variable with a summary function, or $COUNT or $PCT)
//!   by the categories of X1, a series per category of the COLOR or STYLE variable; STACK
//!   stacks the series. X1 = $CASE draws the value of every case, named by CASELABEL.
//! - PIE: a slice per category of COLOR, sized by SUMMARYVAR ($COUNT, $PCT, or a variable
//!   with a function); X1, Y and STACK give a pie per category.
//! - BOX: Tukey's boxes of Y; outliers lie 1.5 to 3 box lengths out, extremes further.
//! - ERRORBAR: means of Y with CI(level), SD(n) or SE(n) bars, alone or on a bar or line chart.
//! - HISTOGRAM: X1 binned, counted with Y = $COUNT or as percentages with $PCT; CUM adds up
//!   the bins, CURVE adds the normal curve.
//! - SCATTER: Y against X1, X2 a third axis, groups by COLOR or STYLE; FITLINE adds a least
//!   squares line, one through the origin, or the mean of Y, over all points (TOTAL) or per
//!   group (MEFFECT).
//! - PANEL: a chart per combination of the panel variables' categories. CATORDER orders the
//!   categories by value, label, count or first occurrence. COORDINATE=HORIZONTAL turns bars
//!   on their side. NORMALIZE stacks to 100%.
//!
//! Where the manual is silent:
//! - BAR, LINE and AREA without a function take the MEAN; COLOR or STYLE without CLUSTER or
//!   STACK cluster the bars. A COLOR variable without TYPE splits the chart unless it is a
//!   scale variable, whose colors are only a look.
//! - Fit lines go through the points drawn, each case once, unweighted, as GRAPH draws them.
//! - MISSING takes the keywords of GRAPH; by default a case missing on any variable of the
//!   chart is left out.

use super::{Class, Context, category, class_of, distinct, numeric, valid};
use crate::chart::{self, Axis, Chart, ChartKind, Grouping, Orientation, Statistic};
use crate::descriptives::case_weights;
use crate::examine::{Distribution, short};
use crate::plot::{BoxSummary, Mark, Plot, Point, ReferenceLine};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Variable};
use statscraft_syntax::{
    Graph, GraphChart, GraphData, GraphFunction as F, GraphPanel, GraphSummary, Igraph,
    IgraphElement as E, IgraphFit, IgraphOrder, IgraphOrderBy, IgraphRole, IgraphVar, VarItem,
};

fn fail<T>(message: impl Into<String>) -> Result<T, Error> {
    Err(Error(format!("IGRAPH: {}", message.into())))
}

fn name_of(role: Option<&IgraphRole>) -> Option<&str> {
    match role.map(|r| &r.var) {
        Some(IgraphVar::Var(name)) => Some(name),
        _ => None,
    }
}

/// A title, subtitle or caption, its `\n` line breaks as spaces.
fn joined(text: &Option<String>) -> Option<String> {
    text.as_ref()
        .map(|t| t.split("\\n").collect::<Vec<_>>().join(" "))
}

/// The GRAPH command of an IGRAPH command and what is added to its charts.
struct Plan {
    graph: Graph,
    /// The legend variable that splits the chart, if any.
    legend: Option<String>,
    /// The element that draws the chart.
    main: E,
    fit: Option<E>,
}

/// The summary of Y for a bar, line or area chart: $COUNT and $PCT take no function, a
/// variable takes MEAN unless one is given.
fn y_summary(c: &Igraph, function: Option<F>, what: &str) -> Result<GraphSummary, Error> {
    let summary = |function, variables| GraphSummary {
        function,
        variables,
    };
    match (c.y.as_ref().map(|r| &r.var), function) {
        (Some(IgraphVar::Count | IgraphVar::Pct), Some(_)) => fail(format!(
            "{what} takes no summary function when Y is $COUNT or $PCT"
        )),
        (Some(IgraphVar::Count), None) => Ok(summary(F::Count, Vec::new())),
        (Some(IgraphVar::Pct), None) => Ok(summary(F::Pct, Vec::new())),
        (Some(IgraphVar::Var(name)), function) => {
            let function = function.unwrap_or(F::Mean);
            if c.y.as_ref().and_then(|r| r.scale) == Some(false) && function != F::Mode {
                return fail(format!("a categorical Y takes only MODE on {what}"));
            }
            Ok(summary(function, vec![VarItem::Name(name.clone())]))
        }
        (Some(IgraphVar::Case), _) => fail("$CASE goes on X1"),
        (None, _) => fail(format!("{what} needs Y: a variable, $COUNT or $PCT")),
    }
}

/// The variable a role must hold.
fn needed<'a>(role: Option<&'a IgraphRole>, what: &str) -> Result<&'a str, Error> {
    match name_of(role) {
        Some(name) => Ok(name),
        None => fail(format!("{what} needs a variable")),
    }
}

/// The GRAPH command of an IGRAPH command, with the notes the run needs; None when there is
/// no chart element.
fn plan(c: &Igraph, data: &Dataset, ignored: &mut Vec<String>) -> Result<Option<Plan>, Error> {
    let (mut main, mut bars, mut fit, mut extra) = (None, None, None, false);
    for element in &c.elements {
        match element {
            E::ErrorBar(interval) if bars.is_none() => bars = Some(*interval),
            E::FitLine { .. } if fit.is_none() => fit = Some(element.clone()),
            E::ErrorBar(_) | E::FitLine { .. } => extra = true,
            _ if main.is_none() => main = Some(element.clone()),
            _ => extra = true,
        }
    }
    let fits = matches!(
        (&main, bars.is_some(), fit.is_some()),
        (_, false, false)
            | (Some(E::Bar(_) | E::Line(_)) | None, true, false)
            | (Some(E::Scatter), false, true)
    );
    if extra || !fits {
        return fail(
            "this mix of elements cannot be drawn: a chart takes one of BAR, LINE, AREA, PIE, \
             BOX, ERRORBAR, HISTOGRAM and SCATTER, with ERRORBAR also on a bar or line chart and \
             FITLINE on a scatterplot",
        );
    }
    let Some(main) = main.or(bars.map(E::ErrorBar)) else {
        return Ok(None);
    };
    if fit.is_some() && c.x2.is_some() {
        return fail("a fit line over three axes cannot be drawn yet");
    }
    // the legend variable: COLOR or STYLE, categorical
    let mut legend: Option<(String, bool)> = None;
    for (what, role) in [("COLOR", &c.color), ("STYLE", &c.style)] {
        let Some(role) = role else { continue };
        let IgraphVar::Var(name) = &role.var else {
            return fail(format!("{what} takes a variable"));
        };
        let variable = data.variable(name)?;
        let scale = role
            .scale
            .unwrap_or(!variable.is_string() && variable.measure() == Measure::Scale);
        if scale {
            ignored.push(format!("{what} TYPE=SCALE"));
            continue;
        }
        let stack = role.stack == Some(true);
        match &legend {
            Some((other, _)) if !other.eq_ignore_ascii_case(name) => {
                return fail(
                    "COLOR and STYLE name different variables; the chart takes one legend variable",
                );
            }
            Some((_, stacked)) => legend = Some((name.clone(), stack || *stacked)),
            None => legend = Some((name.clone(), stack)),
        }
    }
    if c.size.is_some() {
        ignored.push("SIZE".into());
    }
    let (legend_name, stacked) = match &legend {
        Some((name, stacked)) => (Some(name.clone()), *stacked),
        None => (None, false),
    };
    let three = matches!(main, E::Scatter) && c.x2.is_some();
    if c.x2.is_some() && !three {
        return fail("X2, a third axis, is drawn on a scatterplot only");
    }
    if c.three && !three {
        ignored.push("COORDINATE=THREE".into());
    }
    if c.horizontal && !matches!(main, E::Bar(_) | E::Line(_) | E::Area(_)) {
        ignored.push("COORDINATE=HORIZONTAL".into());
    }
    let x1 = c.x1.as_ref().map(|r| &r.var);
    if matches!(x1, Some(IgraphVar::Count | IgraphVar::Pct)) {
        return fail("$COUNT and $PCT go on Y");
    }
    let mut by: Vec<String> = name_of(c.x1.as_ref())
        .into_iter()
        .map(String::from)
        .collect();
    by.extend(legend_name.clone());
    let mut panel = GraphPanel {
        columns: c.panel.clone(),
        rows: Vec::new(),
    };
    let what = match &main {
        E::Bar(_) => "BAR",
        E::Line(_) => "LINE",
        E::Area(_) => "AREA",
        _ => "",
    };
    let data_of = |function| -> Result<GraphData, Error> {
        if x1 == Some(&IgraphVar::Case) {
            let y = needed(
                c.y.as_ref(),
                &format!("{what} of every case (X1 = $CASE): Y"),
            )?;
            return Ok(GraphData::Values {
                variables: vec![VarItem::Name(y.into())],
                by: c.case_label.clone(),
            });
        }
        Ok(GraphData::Summary {
            functions: vec![y_summary(c, function, what)?],
            by: by.clone(),
        })
    };
    let chart = match &main {
        E::Bar(function) => GraphChart::Bar {
            stacked,
            data: data_of(*function)?,
        },
        E::Line(function) => GraphChart::Line {
            area: false,
            data: data_of(*function)?,
        },
        E::Area(function) => GraphChart::Line {
            area: true,
            data: data_of(*function)?,
        },
        E::Pie(function) => {
            let Some(slices) = legend_name.clone() else {
                return fail("PIE needs a categorical COLOR or STYLE variable for its slices");
            };
            let summary = match (c.summary_var.as_ref().map(|r| &r.var), function) {
                (None | Some(IgraphVar::Count), None) => GraphSummary {
                    function: F::Count,
                    variables: Vec::new(),
                },
                (Some(IgraphVar::Pct), None) => GraphSummary {
                    function: F::Pct,
                    variables: Vec::new(),
                },
                (Some(IgraphVar::Var(name)), Some(function)) => GraphSummary {
                    function: *function,
                    variables: vec![VarItem::Name(name.clone())],
                },
                (Some(IgraphVar::Var(_)), None) => {
                    return fail(
                        "PIE needs a summary function, such as PIE (SUM), for a SUMMARYVAR variable",
                    );
                }
                _ => return fail("PIE takes a summary function only with a SUMMARYVAR variable"),
            };
            // a pie per category of X1, STACK and Y
            panel
                .columns
                .extend(name_of(c.x1.as_ref()).map(String::from));
            panel.columns.extend(c.stack.clone());
            panel.rows.extend(name_of(c.y.as_ref()).map(String::from));
            GraphChart::Pie(GraphData::Summary {
                functions: vec![summary],
                by: vec![slices],
            })
        }
        E::Box { outliers, extremes } => GraphChart::Boxes {
            variable: needed(c.y.as_ref(), "BOX: Y")?.into(),
            by,
            outliers: *outliers,
            extremes: *extremes,
        },
        E::ErrorBar(interval) => GraphChart::ErrorBar {
            interval: *interval,
            variables: vec![VarItem::Name(needed(c.y.as_ref(), "ERRORBAR: Y")?.into())],
            by,
        },
        E::Histogram { cumulative, curve } => {
            if legend.is_some() {
                return fail(
                    "a histogram takes no COLOR or STYLE variable; PANEL gives one per group",
                );
            }
            if *cumulative && *curve {
                return fail("a cumulative histogram takes no normal curve");
            }
            if matches!(
                c.y.as_ref().map(|r| &r.var),
                Some(IgraphVar::Var(_) | IgraphVar::Case)
            ) {
                return fail("HISTOGRAM counts its cases: Y is $COUNT or $PCT");
            }
            GraphChart::Histogram {
                variable: needed(c.x1.as_ref(), "HISTOGRAM: X1")?.into(),
                normal: *curve,
            }
        }
        _ => {
            let x = needed(c.x1.as_ref(), "SCATTER: X1")?.to_string();
            let y = needed(c.y.as_ref(), "SCATTER: Y")?.to_string();
            let names = c.point_label.clone();
            match name_of(c.x2.as_ref()) {
                Some(z) => GraphChart::Xyz {
                    x,
                    y,
                    z: z.into(),
                    control: legend_name.clone(),
                    names,
                },
                None => GraphChart::Scatter {
                    x,
                    y,
                    control: legend_name.clone(),
                    names,
                },
            }
        }
    };
    let interval = match main {
        E::Bar(_) | E::Line(_) => bars,
        _ => None,
    };
    let graph = Graph {
        title: joined(&c.title).into_iter().collect(),
        subtitle: joined(&c.subtitle),
        footnote: joined(&c.caption).into_iter().collect(),
        chart,
        listwise: c.listwise,
        report: c.report,
        include: c.include,
        panel: (!panel.columns.is_empty() || !panel.rows.is_empty()).then_some(panel),
        interval,
    };
    Ok(Some(Plan {
        graph,
        legend: legend_name,
        main,
        fit,
    }))
}

/// The categories of a variable in the order CATORDER asks for: the value of each with its
/// label.
fn order_of(
    data: &Dataset,
    shown: &crate::shown::Shown,
    order: &IgraphOrder,
) -> Result<Vec<(Cell, String)>, Error> {
    let variable = data.variable(&order.variable)?;
    let weights = case_weights(data)?;
    // (key, weighted count, first case)
    let mut seen: Vec<(super::Key, f64, usize)> = Vec::new();
    for case in 0..data.case_count() {
        let Some(key) = class_of(&variable, case, true).key else {
            continue;
        };
        let w = weights.as_ref().map_or(1.0, |w| w[case]).max(0.0);
        match seen.iter_mut().find(|s| s.0 == key) {
            Some(s) => s.1 += w,
            None => seen.push((key, w, case)),
        }
    }
    let label = |key: &super::Key| shown.value(&variable, key);
    seen.sort_by(|a, b| match order.by {
        IgraphOrderBy::Value => a.0.compare(&b.0),
        IgraphOrderBy::Label => label(&a.0).cmp(&label(&b.0)),
        IgraphOrderBy::Count => a.1.total_cmp(&b.1).then_with(|| a.0.compare(&b.0)),
        IgraphOrderBy::Occurrence => a.2.cmp(&b.2),
    });
    if order.descending {
        seen.reverse();
    }
    Ok(seen
        .into_iter()
        .map(|(key, ..)| {
            let text = label(&key);
            let cell = match key {
                super::Key::Number(n) => Cell::Number(n),
                super::Key::Text(t) => Cell::Text(t),
            };
            (cell, text)
        })
        .collect())
}

/// Reorders `items` by the rank of each; unranked ones go last in their order.
fn permute<T: Clone>(items: &mut Vec<T>, rank: &[usize]) {
    let mut index: Vec<usize> = (0..items.len()).collect();
    index.sort_by_key(|&i| rank.get(i).copied().unwrap_or(usize::MAX));
    *items = index.iter().map(|&i| items[i].clone()).collect();
}

/// Puts the categories, series or boxes of `variable` in the order of `list`.
fn reorder(chart: &mut Chart, variable: &str, list: &[(Cell, String)], plan: &Plan) {
    let rank = |cell: &Cell| {
        list.iter()
            .position(|(c, _)| c == cell)
            .unwrap_or(usize::MAX)
    };
    let label_rank = |text: &str| {
        list.iter()
            .position(|(_, l)| l == text)
            .unwrap_or(usize::MAX)
    };
    let is = |name: Option<&str>| name.is_some_and(|n| n.eq_ignore_ascii_case(variable));
    if let Some(Plot::Boxes { boxes }) = &mut chart.plot {
        let by_x = is(chart.category_axis.variable.as_deref());
        let by_legend = is(plan.legend.as_deref());
        let ranks: Vec<usize> = boxes
            .iter()
            .map(|b| match (by_x, by_legend) {
                (true, _) => label_rank(&b.label),
                (_, true) => b.group.as_deref().map_or(usize::MAX, label_rank),
                _ => 0,
            })
            .collect();
        permute(boxes, &ranks);
        return;
    }
    if is(chart.category_axis.variable.as_deref()) && !chart.categories.is_empty() {
        let ranks: Vec<usize> = chart.categories.iter().map(|c| rank(&c.value)).collect();
        permute(&mut chart.categories, &ranks);
        for series in &mut chart.series {
            permute(&mut series.values, &ranks);
        }
        if let Some(bars) = &mut chart.error_bars {
            for list in bars.margins.iter_mut().chain(bars.lower.iter_mut()) {
                permute(list, &ranks);
            }
        }
    }
    if chart
        .series
        .iter()
        .any(|s| is(s.variable.as_deref()) && s.value != Cell::Empty)
    {
        let ranks: Vec<usize> = chart.series.iter().map(|s| rank(&s.value)).collect();
        permute(&mut chart.series, &ranks);
        if let Some(bars) = &mut chart.error_bars {
            permute(&mut bars.margins, &ranks);
            if bars.lower.len() == ranks.len() {
                permute(&mut bars.lower, &ranks);
            }
        }
    }
}

/// A fit line over the points; None where it is undefined.
fn fit_line(method: IgraphFit, points: &[&Point]) -> Option<ReferenceLine> {
    let n = points.len() as f64;
    let mean = |f: fn(&Point) -> f64| points.iter().map(|p| f(p)).sum::<f64>() / n;
    let (x, y) = (mean(|p| p.x), mean(|p| p.y));
    let sum = |f: &dyn Fn(&Point) -> f64| points.iter().map(|p| f(p)).sum::<f64>();
    let line = |intercept, slope| Some(ReferenceLine { intercept, slope });
    match method {
        IgraphFit::None => None,
        _ if points.is_empty() => None,
        IgraphFit::Mean => line(y, 0.0),
        IgraphFit::Regression => {
            let sxx = sum(&|p| (p.x - x) * (p.x - x));
            let sxy = sum(&|p| (p.x - x) * (p.y - y));
            (sxx > 0.0).then(|| ReferenceLine {
                intercept: y - sxy / sxx * x,
                slope: sxy / sxx,
            })
        }
        IgraphFit::Origin => {
            let xx = sum(&|p| p.x * p.x);
            (xx > 0.0).then(|| ReferenceLine {
                intercept: 0.0,
                slope: sum(&|p| p.x * p.y) / xx,
            })
        }
    }
}

/// Adds the fit lines to a scatterplot, with a footnote that gives each.
fn add_fit(chart: &mut Chart, fit: &E) {
    let E::FitLine {
        method,
        total,
        groups: by_group,
    } = fit
    else {
        return;
    };
    let Some(Plot::Points { groups, lines, .. }) = &mut chart.plot else {
        return;
    };
    let mut notes = Vec::new();
    let mut add = |name: Option<&str>, points: Vec<&Point>| {
        if let Some(line) = fit_line(*method, &points) {
            let text = format!("y = {} + {} x", short(line.intercept), short(line.slope));
            notes.push(match name {
                Some(name) => format!("{name}: {text}"),
                None => text,
            });
            lines.push(line);
        }
    };
    if *total || groups.len() == 1 {
        add(None, groups.iter().flat_map(|g| &g.points).collect());
    }
    if *by_group && groups.len() > 1 {
        for group in groups.iter() {
            add(Some(&group.name), group.points.iter().collect());
        }
    }
    if !notes.is_empty() {
        let note = format!("Fit line: {}", notes.join("; "));
        chart.footnote = Some(match chart.footnote.take() {
            Some(given) => format!("{}. {note}", given.trim_end_matches('.')),
            None => note,
        });
    }
}

/// A histogram of percentages and its running total, as $PCT and CUM ask.
fn histogram(chart: &mut Chart, percent: bool, cumulative: bool) {
    let Some(series) = chart.series.first_mut() else {
        return;
    };
    let total: f64 = series.values.iter().sum();
    if percent && total > 0.0 {
        for v in &mut series.values {
            *v *= 100.0 / total;
        }
        series.statistic = Statistic::Percent;
        series.name = "Percent".into();
        chart.value_axis.title = "Percent".into();
        chart.value_axis.format = chart::PERCENT_FORMAT.into();
        if let Some(normal) = &mut chart.normal {
            normal.total = 100.0;
        }
    }
    if cumulative {
        let mut running = 0.0;
        for v in &mut series.values {
            running += *v;
            *v = running;
        }
        series.name = format!("Cumulative {}", series.name);
        chart.value_axis.title = format!("Cumulative {}", chart.value_axis.title);
    }
}

impl Engine {
    pub(crate) fn igraph(&mut self, command: &Igraph) -> Result<Vec<Table>, Error> {
        let mut ignored = command.ignored.clone();
        let data = self.active()?;
        let Some(plan) = plan(command, data, &mut ignored)? else {
            self.warn(
                "IGRAPH: no chart element (such as BAR or SCATTER) is given; nothing is drawn",
            );
            return Ok(Vec::new());
        };
        let shown = self.shown();
        let orders: Vec<(String, Vec<(Cell, String)>)> = command
            .orders
            .iter()
            .map(|o| Ok((o.variable.clone(), order_of(data, &shown, o)?)))
            .collect::<Result<_, Error>>()?;
        let start = self.charts.len();
        self.graph(&plan.graph)
            .map_err(|e| Error(e.0.replacen("GRAPH:", "IGRAPH:", 1)))?;
        let legend_title = [&command.color, &command.style]
            .into_iter()
            .flatten()
            .find_map(|r| r.title.clone());
        let mut normalized = false;
        for chart in &mut self.charts[start..] {
            if let Some(name) = &command.viewname {
                chart.outline = Some(name.clone());
            }
            if let Some(x1) = &command.x1 {
                if let Some(title) = &x1.title {
                    chart.category_axis.title = title.clone();
                }
                if x1.max.is_some() && chart.plot.is_some() {
                    chart.category_axis.maximum = x1.max;
                }
            }
            if let Some(y) = &command.y {
                if let Some(title) = &y.title {
                    chart.value_axis.title = title.clone();
                }
                chart.value_axis.maximum = y.max.or(chart.value_axis.maximum);
            }
            if let (Some(title), Some(_)) = (&legend_title, &chart.legend) {
                chart.legend = Some(title.clone());
            }
            if command.horizontal
                && chart.orientation.is_some()
                && chart.kind != ChartKind::Histogram
            {
                chart.orientation = Some(Orientation::Horizontal);
            }
            if command.normalize
                && chart.series.len() > 1
                && matches!(chart.kind, ChartKind::Bar | ChartKind::Area)
            {
                chart.grouping = Some(Grouping::Percent);
                normalized = true;
            }
            if let E::Histogram { cumulative, .. } = plan.main {
                let percent = command.y.as_ref().map(|r| &r.var) == Some(&IgraphVar::Pct);
                histogram(chart, percent, cumulative);
            }
            if let Some(fit) = &plan.fit {
                add_fit(chart, fit);
            }
            for (variable, list) in &orders {
                reorder(chart, variable, list, &plan);
            }
        }
        if command.normalize && !normalized {
            ignored.push("NORMALIZE".into());
        }
        let mut names: Vec<String> = Vec::new();
        for name in ignored {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        if !names.is_empty() {
            self.warn(format!(
                "IGRAPH: {} only change the look of the chart and are ignored",
                names.join(", ")
            ));
        }
        Ok(Vec::new())
    }
}

impl Context<'_> {
    /// Boxes of a variable per category of up to two variables, the second telling apart the
    /// boxes of a cluster.
    pub(super) fn boxes(
        &self,
        name: &str,
        by: &[String],
        outliers: bool,
        extremes: bool,
    ) -> Result<Chart, Error> {
        let include = self.command.include;
        let y = numeric(self.data, name, "BOX")?;
        let by_vars: Vec<Variable> = by
            .iter()
            .map(|name| self.data.variable(name))
            .collect::<Result<_, _>>()?;
        // (classes, value, weight, case)
        let mut records: Vec<(Vec<Class>, f64, f64, usize)> = Vec::new();
        for case in 0..self.data.case_count() {
            let w = self.weight(case);
            if w <= 0.0 {
                continue;
            }
            let classes: Vec<Class> = by_vars.iter().map(|v| class_of(v, case, include)).collect();
            if !self.command.report && classes.iter().any(|c| c.missing) {
                continue;
            }
            if let Some(value) = valid(&y, case, include) {
                records.push((classes, value, w, case));
            }
        }
        if records.is_empty() {
            return fail("no valid cases to chart");
        }
        let levels = |k: usize| -> Vec<Option<Class>> {
            match k < by_vars.len() {
                true => distinct(records.iter().filter_map(|r| r.0.get(k)))
                    .into_iter()
                    .map(Some)
                    .collect(),
                false => vec![None],
            }
        };
        let mut boxes = Vec::new();
        for row in levels(0) {
            for column in levels(1) {
                let mut cell: Vec<(f64, f64, usize)> = records
                    .iter()
                    .filter(|r| r.0.first() == row.as_ref() && r.0.get(1) == column.as_ref())
                    .map(|r| (r.1, r.2, r.3))
                    .collect();
                cell.sort_by(|a, b| a.0.total_cmp(&b.0));
                let pairs: Vec<(f64, f64)> = cell.iter().map(|c| (c.0, c.1)).collect();
                let Some([q1, median, q3]) = Distribution::new(&pairs).hinges() else {
                    continue;
                };
                // Tukey's fences, as EXAMINE draws them
                let step = 1.5 * (q3 - q1);
                let (mut low, mut high) = (q1, q3);
                let (mut out, mut far) = (Vec::new(), Vec::new());
                for &(value, _, case) in &cell {
                    let mark = Mark {
                        value,
                        case: (self.case_numbers.map_or(case, |n| n[case]) + 1).to_string(),
                    };
                    let beyond = |k: f64| {
                        (value > q3 && value >= q3 + k * step)
                            || (value < q1 && value <= q1 - k * step)
                    };
                    if beyond(2.0) {
                        far.push(mark);
                    } else if beyond(1.0) {
                        out.push(mark);
                    } else {
                        (low, high) = (low.min(value), high.max(value));
                    }
                }
                boxes.push(BoxSummary {
                    label: match &row {
                        Some(class) => category(&by_vars[0], class, &self.shown).label,
                        None => self.shown.variable(&y),
                    },
                    group: column
                        .as_ref()
                        .map(|c| category(&by_vars[1], c, &self.shown).label),
                    n: pairs.iter().map(|p| p.1).sum(),
                    lower_whisker: low,
                    lower_hinge: q1,
                    median,
                    upper_hinge: q3,
                    upper_whisker: high,
                    outliers: if outliers { out } else { Vec::new() },
                    extremes: if extremes { far } else { Vec::new() },
                });
            }
        }
        let made = std::iter::once(format!("Boxplot of {}", self.outline.variable(&y)))
            .chain(by_vars.iter().map(|v| self.outline.variable(v)))
            .collect::<Vec<_>>()
            .join(" by ");
        let x = match by_vars.first() {
            Some(v) => self.axis(v, true),
            None => Axis::titled(String::new()),
        };
        let mut chart = Chart::plot(
            ChartKind::Boxplot,
            self.title(made),
            x,
            self.axis(&y, true),
            Plot::Boxes { boxes },
        );
        chart.legend = by_vars.get(1).map(|v| self.shown.variable(v));
        Ok(chart)
    }
}
