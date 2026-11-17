//! GGRAPH: a chart from a graph dataset (GRAPHDATASET) and a chart program (the BEGIN GPL
//! block after the command), in the description of `chart.rs`.
//!
//! Rules taken from the manual:
//! - A summary function among the variables makes one row per combination of the other
//!   variables' values, with the function over that combination's cases; without one, the
//!   graph dataset has a row per case, and the hidden column $CASENUM numbers them.
//! - MISSING=LISTWISE leaves out a case missing on any variable of the list; VARIABLEWISE
//!   leaves it out only of the functions of the variable it misses. REPORTMISSING=YES makes
//!   each user-missing value of a variable a category of its own, except for variables at the
//!   scale level (the LEVEL qualifier, else the dictionary); NO treats them as system-missing.
//! - TRANSFORM=VARSTOCASES turns the functions into rows: an index column numbers them and a
//!   summary column holds their values; the other variables stay as they are.
//! - CASELIMIT keeps the first rows of the graph dataset only.
//! - The chart program: SOURCE names a graph dataset (userSource), DATA takes its columns
//!   (col, with unit.category() for a categorical one), GUIDE titles the axes, the legend and
//!   the chart, SCALE sets linear and category scales, COORD clusters, transposes or makes a
//!   pie, and ELEMENT draws: interval (bars), line, area and point, with position crossing the
//!   variables of the dimensions and color splitting them into series.
//!
//! Where the manual is silent (recorded in docs/differences.md):
//! - Weights count in every function and in the summaries of the chart program; cases with a
//!   weight of zero or less are left out, as in GRAPH.
//! - Categories ascend by value, valid ones first, then user-missing ones (REPORTMISSING=YES).
//! - A bar or point that several rows reach without a summary function shows their sum, with
//!   a warning.
//! - interval with color and without cluster draws the series side by side, as interval.dodge.
//! - Statements, functions and settings this version does not apply give a warning that names
//!   them; the chart is drawn without them. A chart without an element it can draw is an
//!   error.

use crate::chart::{
    self, Axis, Category, Chart, ChartKind, Grouping, Orientation, Series, Statistic, Tier,
};
use crate::crosstabs::Key;
use crate::descriptives::case_weights;
use crate::format::{Stat, procedure_format};
use crate::frequencies::Histogram;
use crate::graph::{
    Class, Item, MOST_CATEGORIES, category, class_of, compute, function_name, order, statistic_of,
};
use crate::plot::{Plot, Point, PointGroup};
use crate::shown::Shown;
use crate::{Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Variable};
use statscraft_syntax::{
    Command, Ggraph, GplExpr, GplProgram, GraphDataset, GraphFunction as F, GraphItem, GraphStat,
};
use std::cmp::Ordering;
use std::collections::HashMap;

/// What the engine does with a command while GGRAPH waits for its BEGIN GPL blocks.
pub(crate) enum GplStep {
    Pass,
    Wait,
    Run(Box<Command>),
}

impl Engine {
    /// GGRAPH waits for the BEGIN GPL block of each GRAPHSPEC and runs with the last one.
    pub(crate) fn gpl_step(&mut self, command: &Command) -> Result<GplStep, Error> {
        match command {
            Command::Ggraph(g) if g.programs.len() < g.specs.len() => {
                if self.ggraph.replace((**g).clone()).is_some() {
                    self.warn(NOT_DRAWN);
                }
                Ok(GplStep::Wait)
            }
            Command::BeginGpl(program) => {
                let Some(mut g) = self.ggraph.take() else {
                    return Err(Error(
                        "BEGIN GPL belongs right after a GGRAPH command with GRAPHSPEC \
                         SOURCE=INLINE"
                            .into(),
                    ));
                };
                g.programs.push((**program).clone());
                if g.programs.len() < g.specs.len() {
                    self.ggraph = Some(g);
                    return Ok(GplStep::Wait);
                }
                Ok(GplStep::Run(Box::new(Command::Ggraph(Box::new(g)))))
            }
            _ => {
                if self.ggraph.take().is_some() {
                    self.warn(NOT_DRAWN);
                }
                Ok(GplStep::Pass)
            }
        }
    }

    pub(crate) fn ggraph(&mut self, command: &Ggraph) -> Result<Vec<Table>, Error> {
        let mut warnings: Vec<String> = command
            .skipped
            .iter()
            .map(|s| format!("GGRAPH: {s} is not supported yet and is skipped"))
            .collect();
        let charts = {
            let data = self.active()?;
            let weights = case_weights(data)?;
            let shown = self.shown();
            let numbers = self.case_numbers.as_deref();
            let sets = command
                .datasets
                .iter()
                .map(|set| build(data, set, weights.as_deref(), &shown, numbers))
                .collect::<Result<Vec<_>, _>>()?;
            let mut charts = Vec::new();
            for (spec, program) in command.specs.iter().zip(&command.programs) {
                let mut chart = draw(&sets, program, &shown, &mut warnings)?;
                chart.outline = spec.label.clone();
                charts.push(chart);
            }
            charts
        };
        for warning in warnings {
            self.warn(warning);
        }
        self.charts.extend(charts);
        Ok(Vec::new())
    }
}

const NOT_DRAWN: &str = "GGRAPH drew nothing: the BEGIN GPL block of each GRAPHSPEC with \
                         SOURCE=INLINE must follow the command";

/// A value of a graph dataset: a category of a variable, or a number.
#[derive(Clone)]
enum Datum {
    Missing,
    Class(Class),
    Number(f64),
}

enum Kind<'a> {
    Variable(Variable<'a>),
    Number,
    /// The index of TRANSFORM=VARSTOCASES: 1, 2 and so on, with these labels.
    Labels(Vec<String>),
}

struct Column<'a> {
    /// As the chart program names it.
    name: String,
    title: String,
    format: String,
    statistic: Statistic,
    kind: Kind<'a>,
    cells: Vec<Datum>,
}

impl Column<'_> {
    /// The category of a row; None when it is missing.
    fn class(&self, row: usize) -> Option<Class> {
        let number = |x: f64| Class {
            key: Some(Key::Number(x)),
            missing: false,
        };
        match &self.cells[row] {
            Datum::Missing => None,
            Datum::Class(class) => Some(class.clone()),
            Datum::Number(x) => x.is_finite().then(|| number(*x)),
        }
    }

    /// The number of a row; None when it is missing or not a number.
    fn number(&self, row: usize) -> Option<f64> {
        match &self.cells[row] {
            Datum::Class(Class {
                key: Some(Key::Number(x)),
                missing: false,
            }) => Some(*x),
            Datum::Number(x) if x.is_finite() => Some(*x),
            _ => None,
        }
    }

    fn label(&self, class: &Class, shown: &Shown) -> Category {
        let number = match &class.key {
            Some(Key::Number(x)) => *x,
            _ => f64::NAN,
        };
        let label = match &self.kind {
            Kind::Variable(v) => return category(v, class, shown),
            Kind::Number => chart::number_text(number, &self.format),
            // an index that SCALE cat names outside the functions shows as its number
            Kind::Labels(labels) => match labels.get((number as usize).wrapping_sub(1)) {
                Some(label) => label.clone(),
                None => chart::number_text(number, ""),
            },
        };
        Category {
            label,
            value: crate::Cell::Number(number),
            missing: false,
            bin: None,
        }
    }

    fn axis(&self, title: String) -> Axis {
        let variable = match &self.kind {
            Kind::Variable(v) => Some(v),
            _ => None,
        };
        Axis {
            title,
            format: match variable.is_some_and(|v| v.is_string()) {
                true => String::new(),
                false => self.format.clone(),
            },
            maximum: None,
            variable: variable.map(|v| v.name().to_string()),
        }
    }
}

/// A graph dataset: its columns and the weight of each row.
struct Set<'a> {
    name: String,
    columns: Vec<Column<'a>>,
    weights: Vec<f64>,
}

/// The value of a variable at a case as a datum: Missing when it is system-missing, or
/// user-missing without `report`.
fn datum(v: &Variable, case: usize, report: bool) -> Datum {
    let class = class_of(v, case, false);
    match class.key.is_none() || (class.missing && !report) {
        true => Datum::Missing,
        false => Datum::Class(class),
    }
}

fn variable_column<'a>(
    v: Variable<'a>,
    name: String,
    cells: Vec<Datum>,
    shown: &Shown,
) -> Column<'a> {
    Column {
        name,
        title: shown.variable(&v),
        format: v.format().to_string(),
        statistic: Statistic::Value,
        kind: Kind::Variable(v),
        cells,
    }
}

/// A function with GRAPH's counterpart for its name, format and kind of number.
fn proxy(stat: GraphStat) -> F {
    match stat {
        GraphStat::Graph(f) => f,
        GraphStat::Nle(_) | GraphStat::Neq(_) | GraphStat::Nge(_) => F::N,
        GraphStat::Ple(_) | GraphStat::Peq(_) | GraphStat::Pge(_) => F::Pct,
        GraphStat::CaseValue => F::Mean,
    }
}

fn stat_value(stat: GraphStat, pairs: &[(f64, f64)]) -> f64 {
    let n: f64 = pairs.iter().map(|p| p.1).sum();
    let count = |keep: &dyn Fn(f64) -> bool| -> f64 {
        pairs.iter().filter(|p| keep(p.0)).map(|p| p.1).sum()
    };
    let share = |c: f64| match n > 0.0 {
        true => 100.0 * c / n,
        false => f64::NAN,
    };
    match stat {
        GraphStat::Graph(f) => compute(f, pairs),
        GraphStat::Nle(x) => count(&|v| v <= x),
        GraphStat::Ple(x) => share(count(&|v| v <= x)),
        GraphStat::Neq(x) => count(&|v| v == x),
        GraphStat::Peq(x) => share(count(&|v| v == x)),
        GraphStat::Nge(x) => count(&|v| v >= x),
        GraphStat::Pge(x) => share(count(&|v| v >= x)),
        GraphStat::CaseValue => f64::NAN,
    }
}

fn stat_title(stat: GraphStat, variable: Option<&str>) -> String {
    let of = variable.map_or(String::new(), |v| format!(" of {v}"));
    let x = |x: f64| format!("{x}");
    match stat {
        GraphStat::Nle(v) => format!("N{of} <= {}", x(v)),
        GraphStat::Ple(v) => format!("%{of} <= {}", x(v)),
        GraphStat::Neq(v) => format!("N{of} = {}", x(v)),
        GraphStat::Peq(v) => format!("%{of} = {}", x(v)),
        GraphStat::Nge(v) => format!("N{of} >= {}", x(v)),
        GraphStat::Pge(v) => format!("%{of} >= {}", x(v)),
        _ => function_name(proxy(stat), variable),
    }
}

fn build<'a>(
    data: &'a Dataset,
    set: &GraphDataset,
    weights: Option<&[f64]>,
    shown: &Shown,
    numbers: Option<&[usize]>,
) -> Result<Set<'a>, Error> {
    let weight = |case: usize| weights.map_or(1.0, |w| w[case]);
    let cases: Vec<usize> = (0..data.case_count())
        .filter(|&c| weight(c) > 0.0)
        .collect();
    // the variables with their names and whether their user-missing values are categories
    let mut vars: Vec<(Variable, String, bool)> = Vec::new();
    let mut functions: Vec<(GraphStat, Option<Variable>, String)> = Vec::new();
    for item in &set.items {
        match item {
            GraphItem::Variables { item, name, scale } => {
                for v in crate::expand_in(data, std::slice::from_ref(item))? {
                    let v = data.variable(&v)?;
                    let scale = scale.unwrap_or(v.measure() == Measure::Scale);
                    let name = name.clone().unwrap_or_else(|| v.name().to_string());
                    vars.push((v, name, set.report_missing && !scale));
                }
            }
            GraphItem::Function {
                stat,
                variable,
                name,
            } => {
                let v = variable.as_deref().map(|n| data.variable(n)).transpose()?;
                if let Some(v) = v.as_ref().filter(|v| v.is_string())
                    && *stat != GraphStat::CaseValue
                {
                    return Err(Error(format!(
                        "GGRAPH: {} is a string variable; a summary function needs a numeric one",
                        v.name()
                    )));
                }
                functions.push((*stat, v, name.clone()));
            }
        }
    }
    let summary = functions.iter().any(|f| f.0 != GraphStat::CaseValue);
    let mut columns = Vec::new();
    if !summary {
        for (v, name, report) in vars {
            let cells = cases.iter().map(|&c| datum(&v, c, report)).collect();
            columns.push(variable_column(v, name, cells, shown));
        }
        for (_, v, name) in functions {
            let v = v.expect("CASEVALUE takes a variable");
            let cells = cases.iter().map(|&c| datum(&v, c, false)).collect();
            columns.push(variable_column(v, name, cells, shown));
        }
        columns.push(Column {
            name: "$CASENUM".into(),
            title: "Case Number".into(),
            format: "F8.0".into(),
            statistic: Statistic::Value,
            kind: Kind::Number,
            cells: cases
                .iter()
                .map(|&c| Datum::Number((numbers.map_or(c, |n| n[c]) + 1) as f64))
                .collect(),
        });
        let mut set_weights: Vec<f64> = cases.iter().map(|&c| weight(c)).collect();
        set_weights.truncate(set.case_limit);
        for column in &mut columns {
            column.cells.truncate(set.case_limit);
        }
        return Ok(Set {
            name: set.name.clone(),
            columns,
            weights: set_weights,
        });
    }
    // the cases by their combination of categories, each with its valid function values
    let mut records: Vec<(Vec<Class>, Vec<Option<f64>>, f64)> = Vec::new();
    for &case in &cases {
        let mut classes = Vec::new();
        for (v, _, report) in &vars {
            match datum(v, case, *report) {
                Datum::Class(class) => classes.push(class),
                _ => break,
            }
        }
        if classes.len() < vars.len() {
            continue;
        }
        let values: Vec<Option<f64>> = functions
            .iter()
            .map(|(_, v, _)| match v {
                None => Some(0.0),
                Some(v) => v
                    .numeric()
                    .ok()?
                    .get(case)
                    .filter(|x| !v.is_user_missing(*x)),
            })
            .collect();
        if set.listwise && values.iter().any(Option::is_none) {
            continue;
        }
        records.push((classes, values, weight(case)));
    }
    let compare = |a: &[Class], b: &[Class]| {
        a.iter()
            .zip(b)
            .map(|(x, y)| order(x, y))
            .find(|o| o.is_ne())
            .unwrap_or(Ordering::Equal)
    };
    records.sort_by(|a, b| compare(&a.0, &b.0));
    let mut groups: Vec<(Vec<Class>, Vec<f64>)> = Vec::new();
    let mut start = 0;
    while start < records.len() {
        let end = (start..records.len())
            .find(|&i| compare(&records[i].0, &records[start].0).is_ne())
            .unwrap_or(records.len());
        let part = &records[start..end];
        let values = functions
            .iter()
            .enumerate()
            .map(|(f, (stat, ..))| {
                let pairs: Vec<(f64, f64)> =
                    part.iter().filter_map(|r| Some((r.1[f]?, r.2))).collect();
                stat_value(*stat, &pairs)
            })
            .collect();
        groups.push((part[0].0.clone(), values));
        start = end;
    }
    groups.truncate(set.case_limit);
    for (i, (v, name, _)) in vars.into_iter().enumerate() {
        let cells = groups
            .iter()
            .map(|g| Datum::Class(g.0[i].clone()))
            .collect();
        columns.push(variable_column(v, name, cells, shown));
    }
    let stats: Vec<Column> = functions
        .iter()
        .enumerate()
        .map(|(f, (stat, v, name))| {
            let values: Vec<f64> = groups.iter().map(|g| g.1[f]).collect();
            let item = Item {
                function: proxy(*stat),
                variable: *v,
            };
            Column {
                name: name.clone(),
                title: stat_title(*stat, v.as_ref().map(|v| shown.variable(v)).as_deref()),
                format: item.format(&values),
                statistic: statistic_of(item.function),
                kind: Kind::Number,
                cells: values.into_iter().map(Datum::Number).collect(),
            }
        })
        .collect();
    let mut weights = vec![1.0; groups.len()];
    let Some((summary_name, index_name)) = &set.to_cases else {
        columns.extend(stats);
        return Ok(Set {
            name: set.name.clone(),
            columns,
            weights,
        });
    };
    // a row per group and function
    let k = stats.len();
    for column in &mut columns {
        column.cells = column
            .cells
            .iter()
            .flat_map(|c| std::iter::repeat_n(c.clone(), k))
            .collect();
    }
    let rows = groups.len() * k;
    weights = vec![1.0; rows];
    let same = stats.iter().all(|s| s.statistic == stats[0].statistic);
    columns.push(Column {
        name: index_name.clone(),
        title: "Variables".into(),
        format: String::new(),
        statistic: Statistic::Value,
        kind: Kind::Labels(stats.iter().map(|s| s.title.clone()).collect()),
        cells: (0..rows)
            .map(|r| Datum::Number((r % k + 1) as f64))
            .collect(),
    });
    columns.push(Column {
        name: summary_name.clone(),
        title: match same {
            true => stats[0]
                .title
                .split(" of ")
                .next()
                .unwrap_or("Value")
                .to_string(),
            false => "Value".into(),
        },
        format: stats[0].format.clone(),
        statistic: match same {
            true => stats[0].statistic,
            false => Statistic::Value,
        },
        kind: Kind::Number,
        cells: (0..rows)
            .map(|r| stats[r % k].cells[r / k].clone())
            .collect(),
    });
    Ok(Set {
        name: set.name.clone(),
        columns,
        weights,
    })
}

/// A variable of the chart program: a column of a graph dataset.
#[derive(Clone, Copy)]
struct Var {
    set: usize,
    column: usize,
    category: bool,
}

/// What the statements other than ELEMENT set.
#[derive(Default)]
struct Settings {
    vars: HashMap<String, Var>,
    sources: HashMap<String, usize>,
    cluster: bool,
    transpose: bool,
    polar: bool,
    title: Option<String>,
    subtitle: Option<String>,
    footnote: Option<String>,
    /// Axis titles by dimension; Some(None) hides the title.
    axes: HashMap<usize, Option<String>>,
    legend: Option<Option<String>>,
    maximum: Option<f64>,
    /// SCALE cat include: values by dimension; 0 for the color aesthetic.
    includes: Vec<(usize, Vec<String>)>,
}

/// The text of label("...") among the arguments; Some(None) for null().
fn label(args: &[GplExpr]) -> Option<Option<String>> {
    args.iter().find_map(|a| match a {
        GplExpr::Call(name, parts) if name == "label" => Some(Some(
            parts
                .iter()
                .filter_map(|p| match p {
                    GplExpr::Str(s) => Some(s.as_str()),
                    _ => None,
                })
                .collect(),
        )),
        GplExpr::Call(name, _) if name == "null" => Some(None),
        _ => None,
    })
}

/// The dimension of dim(n) among the arguments.
fn dim(args: &[GplExpr]) -> Option<usize> {
    args.iter().find_map(|a| match a {
        GplExpr::Call(name, n) if name == "dim" => match n.first() {
            Some(GplExpr::Number(x)) => Some(*x as usize),
            _ => None,
        },
        _ => None,
    })
}

fn strings(args: &[GplExpr]) -> Vec<String> {
    args.iter()
        .filter_map(|a| match a {
            GplExpr::Str(s) => Some(s.clone()),
            GplExpr::Number(x) => Some(format!("{x}")),
            _ => None,
        })
        .collect()
}

/// The names of the calls in an expression, outermost first.
fn calls<'e>(expr: &'e GplExpr, out: &mut Vec<&'e str>) {
    if let GplExpr::Call(name, args) = expr {
        out.push(name);
        for arg in args {
            calls(arg, out);
        }
    }
}

/// How a position is computed.
enum Position {
    /// The variables of the dimensions crossed, with a summary function when one is given.
    Cross(Option<String>, Vec<String>),
    /// summary.count(bin.rect(x)): a histogram; true when bin.rect has settings.
    Histogram(String, String, bool),
    /// density.normal(x): the normal curve over a histogram.
    Normal(String),
}

/// The names crossed by `*`.
fn crossed(expr: &GplExpr, out: &mut Vec<String>) -> bool {
    match expr {
        GplExpr::Op('*', a, b) => crossed(a, out) && crossed(b, out),
        GplExpr::Name(name) => {
            out.push(name.to_lowercase());
            true
        }
        _ => false,
    }
}

fn position(expr: &GplExpr) -> Option<Position> {
    let mut names = Vec::new();
    if crossed(expr, &mut names) {
        return Some(Position::Cross(None, names));
    }
    let GplExpr::Call(name, args) = expr else {
        return None;
    };
    let function = name.strip_prefix("summary.");
    match (function, name.as_str(), &args[..]) {
        (Some(f), _, [GplExpr::Call(bin, inner)]) if bin == "bin.rect" => match &inner[..] {
            [GplExpr::Name(x), rest @ ..] => Some(Position::Histogram(
                f.to_string(),
                x.to_lowercase(),
                !rest.is_empty(),
            )),
            _ => None,
        },
        (Some(f), _, [inner]) => {
            crossed(inner, &mut names).then(|| Position::Cross(Some(f.to_string()), names))
        }
        (_, "density.normal", [GplExpr::Name(x), ..]) => Some(Position::Normal(x.to_lowercase())),
        _ => None,
    }
}

/// GPL summary functions with their counterpart; percent is computed apart.
fn summary_function(name: &str) -> Option<F> {
    Some(match name {
        "count" => F::Count,
        "mean" => F::Mean,
        "sum" => F::Sum,
        "median" => F::Median,
        "mode" => F::Mode,
        "max" | "maximum" => F::Maximum,
        "min" | "minimum" => F::Minimum,
        "percent" | "percent.sum" | "percent.count" => F::Pct,
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Mark {
    Bar,
    Line,
    Area,
    Point,
}

struct Element {
    line: u32,
    mark: Mark,
    stacked: bool,
    position: Position,
    /// The variable or the literal name of the color.
    color: Option<Result<String, String>>,
}

fn settings(
    program: &GplProgram,
    sets: &[Set],
    warn: &mut dyn FnMut(u32, String),
) -> Result<(Settings, Vec<Element>), Error> {
    let mut s = Settings::default();
    let mut elements = Vec::new();
    for statement in &program.statements {
        let line = statement.line;
        let kind = statement.kind.as_str();
        let Some(body) = &statement.body else {
            warn(line, format!("{kind} could not be read and is skipped"));
            continue;
        };
        let skip = |what: &str| format!("{kind} {what} is not supported yet and is skipped");
        match (kind, body) {
            ("SOURCE", GplExpr::Assign(name, source)) => {
                let id = match &**source {
                    GplExpr::Call(f, args) if f == "usersource" => {
                        args.iter().find_map(|a| match a {
                            GplExpr::Call(i, v) if i == "id" => strings(v).pop(),
                            _ => None,
                        })
                    }
                    _ => None,
                };
                let Some(id) = id else {
                    return Err(Error(format!(
                        "GGRAPH: line {line}: SOURCE {} is not supported; the chart program \
                         reads a graph dataset with userSource(id(\"name\"))",
                        source.head()
                    )));
                };
                let Some(index) = sets.iter().position(|d| d.name.eq_ignore_ascii_case(&id)) else {
                    return Err(Error(format!(
                        "GGRAPH: line {line}: no GRAPHDATASET is named \"{id}\""
                    )));
                };
                s.sources.insert(name.to_lowercase(), index);
            }
            ("DATA", GplExpr::Assign(name, col)) => {
                let GplExpr::Call(f, args) = &**col else {
                    warn(line, skip(col.head()));
                    continue;
                };
                if f != "col" {
                    warn(line, skip(f));
                    continue;
                }
                let (mut set, mut column, mut category) = (None, None, false);
                for arg in args {
                    match arg {
                        GplExpr::Call(a, v) if a == "source" => {
                            set = match v.first() {
                                Some(GplExpr::Name(n)) => s.sources.get(&n.to_lowercase()).copied(),
                                _ => None,
                            };
                        }
                        GplExpr::Call(a, v) if a == "name" => column = strings(v).pop(),
                        GplExpr::Call(a, _) if a == "unit.category" => category = true,
                        other => warn(
                            line,
                            format!(
                                "DATA {name}: {} is not supported yet and is skipped",
                                other.head()
                            ),
                        ),
                    }
                }
                let Some(set) = set else {
                    return Err(Error(format!(
                        "GGRAPH: line {line}: DATA {name} needs source() with a name that SOURCE \
                         defines"
                    )));
                };
                let Some(column) = column else {
                    return Err(Error(format!(
                        "GGRAPH: line {line}: DATA {name} needs name(\"column\")"
                    )));
                };
                let Some(index) = sets[set]
                    .columns
                    .iter()
                    .position(|c| c.name.eq_ignore_ascii_case(&column))
                else {
                    return Err(Error(format!(
                        "GGRAPH: line {line}: graph dataset \"{}\" has no column \"{column}\"",
                        sets[set].name
                    )));
                };
                let c = &sets[set].columns[index];
                let text = matches!(&c.kind, Kind::Variable(v) if v.is_string());
                s.vars.insert(
                    name.to_lowercase(),
                    Var {
                        set,
                        column: index,
                        category: category || text || matches!(c.kind, Kind::Labels(_)),
                    },
                );
            }
            ("COORD", _) => {
                let mut names = Vec::new();
                calls(body, &mut names);
                for name in names {
                    match name {
                        "rect" | "dim" | "startangle" => {}
                        "cluster" => s.cluster = true,
                        "transpose" => s.transpose = true,
                        "polar.theta" => s.polar = true,
                        other => warn(line, skip(other)),
                    }
                }
            }
            ("SCALE", GplExpr::Call(f, args)) if f == "linear" || f == "cat" => {
                let target = match dim(args) {
                    Some(d) => Some(d),
                    // the color aesthetic
                    None => args.iter().any(|a| a.head() == "aesthetic").then_some(0),
                };
                for arg in args {
                    match (f.as_str(), arg) {
                        (_, GplExpr::Call(a, _)) if a == "dim" || a == "aesthetic" => {}
                        ("linear", GplExpr::Call(a, _))
                            if a == "include" || a == "datamaximum" || a == "dataminimum" => {}
                        ("linear", GplExpr::Call(a, v)) if a == "max" && target == Some(2) => {
                            if let Some(GplExpr::Number(x)) = v.first() {
                                s.maximum = Some(*x);
                            }
                        }
                        ("cat", GplExpr::Call(a, v)) if a == "include" => {
                            if let Some(t) = target {
                                s.includes.push((t, strings(v)));
                            }
                        }
                        (_, other) => warn(line, skip(&format!("{f}: {}", other.head()))),
                    }
                }
            }
            ("GUIDE", GplExpr::Call(f, args)) => match f.as_str() {
                "axis" => match dim(args) {
                    Some(d) => {
                        if let Some(text) = label(args) {
                            s.axes.insert(d, text);
                        }
                        for arg in args {
                            if !["dim", "label", "null"].contains(&arg.head()) {
                                warn(line, skip(&format!("axis: {}", arg.head())));
                            }
                        }
                    }
                    None => warn(line, skip("axis without dim()")),
                },
                "legend" => s.legend = label(args),
                "text.title" => s.title = label(args).flatten(),
                "text.subtitle" => s.subtitle = label(args).flatten(),
                "text.footnote" => s.footnote = label(args).flatten(),
                other => warn(line, skip(other)),
            },
            ("ELEMENT", GplExpr::Call(f, args)) => {
                let (mark, stacked) = match f.as_str() {
                    "interval" | "interval.dodge" => (Mark::Bar, false),
                    "interval.stack" => (Mark::Bar, true),
                    "line" => (Mark::Line, false),
                    "area" => (Mark::Area, false),
                    "area.stack" => (Mark::Area, true),
                    "point" => (Mark::Point, false),
                    other => {
                        warn(line, skip(other));
                        continue;
                    }
                };
                let mut found = None;
                let mut color = None;
                for arg in args {
                    let GplExpr::Call(a, v) = arg else {
                        warn(line, skip(&format!("{f}: {}", arg.head())));
                        continue;
                    };
                    let group = ["color", "color.interior", "color.exterior", "shape"]
                        .contains(&a.as_str())
                        || a.starts_with("pattern");
                    match (a.as_str(), v.first()) {
                        ("position", Some(p)) => found = Some(position(p)),
                        (_, Some(GplExpr::Name(n)))
                            if group && s.vars.contains_key(&n.to_lowercase()) =>
                        {
                            color = Some(Ok(n.to_lowercase()));
                        }
                        (_, Some(GplExpr::Str(text))) if a.starts_with("color") => {
                            color = Some(Err(text.clone()));
                        }
                        // bars are drawn square, and a line passes over empty categories
                        ("shape.interior" | "shape", Some(GplExpr::Name(n)))
                            if n.eq_ignore_ascii_case("shape.square") => {}
                        ("missing.wings", _) => {}
                        _ => warn(line, skip(&format!("{f}: {a}"))),
                    }
                }
                match found {
                    Some(Some(position)) => elements.push(Element {
                        line,
                        mark,
                        stacked,
                        position,
                        color,
                    }),
                    Some(None) => warn(line, skip(&format!("{f} with this position"))),
                    None => warn(line, skip(&format!("{f} without position"))),
                }
            }
            ("SOURCE" | "DATA" | "SCALE" | "GUIDE" | "ELEMENT", _) => {
                warn(line, skip(body.head()));
            }
            _ => warn(line, format!("{kind} is not supported yet and is skipped")),
        }
    }
    Ok((s, elements))
}

/// The chart of one program.
fn draw(
    sets: &[Set],
    program: &GplProgram,
    shown: &Shown,
    warnings: &mut Vec<String>,
) -> Result<Chart, Error> {
    let mut warn = |line: u32, text: String| warnings.push(format!("GGRAPH: line {line}: {text}"));
    let (s, mut elements) = settings(program, sets, &mut warn)?;
    let var = |name: &str, line: u32| {
        s.vars.get(name).copied().ok_or_else(|| {
            Error(format!(
                "GGRAPH: line {line}: {name} is not a variable of the chart program; DATA \
                 defines them"
            ))
        })
    };
    let column = |v: Var| &sets[v.set].columns[v.column];
    let axis_title = |d: usize, made: String| match s.axes.get(&d) {
        Some(Some(text)) => text.clone(),
        Some(None) => String::new(),
        None => made,
    };
    // a histogram, with its normal curve when an element draws one
    if let Some(i) = elements
        .iter()
        .position(|e| matches!(e.position, Position::Histogram(..)))
    {
        let e = elements.remove(i);
        let Position::Histogram(function, x, binned) = &e.position else {
            unreachable!("found above");
        };
        if function != "count" {
            warn(
                e.line,
                format!("summary.{function} of a histogram is not supported yet; it shows counts"),
            );
        }
        if *binned {
            warn(e.line, "the settings of bin.rect are not supported yet; the bins are chosen as FREQUENCIES chooses them".into());
        }
        if e.color.is_some() {
            warn(
                e.line,
                "color in a histogram is not supported yet and is skipped".into(),
            );
        }
        let mut normal = false;
        for other in &elements {
            match &other.position {
                Position::Normal(n) if n == x => normal = true,
                _ => warn(
                    other.line,
                    "ELEMENT is skipped: the chart is a histogram".into(),
                ),
            }
        }
        let v = var(x, e.line)?;
        let c = column(v);
        let mut pairs: Vec<(f64, f64)> = (0..sets[v.set].weights.len())
            .filter_map(|r| Some((c.number(r)?, sets[v.set].weights[r])))
            .collect();
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let total: f64 = pairs.iter().map(|p| p.1).sum();
        let title = s.title.clone().unwrap_or_else(|| c.title.clone());
        let heading = axis_title(1, c.title.clone());
        let mut chart = Histogram {
            title: &title,
            heading: &heading,
            format: &c.format,
            name: &c.name,
            normal,
            scale: None,
        }
        .chart(&pairs, &pairs, total)
        .map_err(|e| Error(format!("GGRAPH: {}: {e}", c.name)))?;
        if let Some(Some(text)) = s.axes.get(&2) {
            chart.value_axis.title = text.clone();
        }
        chart.subtitle = s.subtitle.clone();
        if let Some(footnote) = &s.footnote {
            chart.footnote = Some(footnote.clone());
        }
        return Ok(chart);
    }
    elements.retain(|e| match e.position {
        Position::Normal(_) => {
            warn(
                e.line,
                "density.normal draws over a histogram only and is skipped".into(),
            );
            false
        }
        _ => true,
    });
    let Some(first) = elements.first() else {
        return Err(Error(
            "GGRAPH: the chart program has no ELEMENT that can be drawn; see the warnings".into(),
        ));
    };
    let (mark, stacked) = (first.mark, first.stacked);
    elements.retain(|e| {
        let same = e.mark == mark;
        if !same {
            warn(
                e.line,
                "ELEMENT is skipped: the chart shows one kind of element, as the first one draws"
                    .into(),
            );
        }
        same
    });
    let mut chart = match mark {
        Mark::Point if is_scatter(&elements[0], &s) => scatter(sets, &s, &elements, shown)?,
        _ => {
            let mut chart = categorical(sets, &s, &elements, shown, &mut warn)?;
            chart.kind = match mark {
                Mark::Bar => ChartKind::Bar,
                Mark::Line => ChartKind::Line,
                Mark::Area => ChartKind::Area,
                Mark::Point => ChartKind::ErrorBar,
            };
            chart.grouping = match (mark, stacked) {
                (Mark::Bar | Mark::Area, true) if chart.series.len() > 1 => Some(Grouping::Stacked),
                (Mark::Area, _) => None,
                _ => Some(Grouping::Clustered),
            };
            if s.transpose && mark != Mark::Point {
                chart.orientation = Some(Orientation::Horizontal);
            }
            if s.polar {
                pie(&mut chart, mark)?;
            }
            chart.value_axis.maximum = s.maximum;
            chart
        }
    };
    chart.subtitle = s.subtitle.clone();
    chart.footnote = s.footnote.clone();
    if let Some(title) = &s.title {
        chart.title = title.clone();
    }
    match &s.legend {
        Some(Some(text)) if chart.legend.is_some() || chart.series.len() > 1 => {
            chart.legend = Some(text.clone())
        }
        Some(None) => chart.legend = None,
        _ => {}
    }
    Ok(chart)
}

fn is_scatter(e: &Element, s: &Settings) -> bool {
    match &e.position {
        Position::Cross(None, dims) if dims.len() == 2 => dims
            .iter()
            .all(|d| s.vars.get(d).is_some_and(|v| !v.category)),
        _ => false,
    }
}

/// The groups of points of point elements over two scale dimensions.
fn scatter(
    sets: &[Set],
    s: &Settings,
    elements: &[Element],
    shown: &Shown,
) -> Result<Chart, Error> {
    let mut groups: Vec<PointGroup> = Vec::new();
    let mut axes = None;
    let mut legend = None;
    for e in elements {
        let Position::Cross(None, dims) = &e.position else {
            continue;
        };
        let (Some(&x), Some(&y)) = (
            s.vars.get(&dims[0]),
            dims.get(1).and_then(|d| s.vars.get(d)),
        ) else {
            continue;
        };
        if x.set != y.set {
            return Err(Error(format!(
                "GGRAPH: line {}: an element takes its variables from one graph dataset",
                e.line
            )));
        }
        let set = &sets[x.set];
        let (cx, cy) = (&set.columns[x.column], &set.columns[y.column]);
        let group_var = match &e.color {
            Some(Ok(name)) => s.vars.get(name).copied().filter(|g| g.set == x.set),
            _ => None,
        };
        let mut points: Vec<(Option<Class>, Point)> = Vec::new();
        for r in 0..set.weights.len() {
            let (Some(px), Some(py)) = (cx.number(r), cy.number(r)) else {
                continue;
            };
            let class = match group_var {
                Some(g) => match set.columns[g.column].class(r) {
                    Some(class) => Some(class),
                    None => continue,
                },
                None => None,
            };
            points.push((
                class,
                Point {
                    x: px,
                    y: py,
                    z: None,
                    label: None,
                },
            ));
        }
        let mut classes: Vec<Class> = Vec::new();
        for class in points.iter().filter_map(|p| p.0.as_ref()) {
            if !classes.contains(class) {
                classes.push(class.clone());
            }
        }
        classes.sort_by(order);
        match group_var {
            Some(g) => {
                let gc = &set.columns[g.column];
                legend = Some(gc.title.clone());
                for class in classes {
                    groups.push(PointGroup {
                        name: gc.label(&class, shown).label,
                        points: points
                            .iter()
                            .filter(|p| p.0.as_ref() == Some(&class))
                            .map(|p| p.1.clone())
                            .collect(),
                    });
                }
            }
            None => groups.push(PointGroup {
                name: match &e.color {
                    Some(Err(text)) => text.clone(),
                    _ => cy.title.clone(),
                },
                points: points.into_iter().map(|p| p.1).collect(),
            }),
        }
        axes.get_or_insert((cx, cy));
    }
    let Some((cx, cy)) = axes else {
        return Err(Error(
            "GGRAPH: the point element has no variables to draw".into(),
        ));
    };
    if groups.iter().all(|g| g.points.is_empty()) {
        return Err(Error("GGRAPH: no row has valid values on both axes".into()));
    }
    let title_of = |d: usize, made: &str| match s.axes.get(&d) {
        Some(Some(text)) => text.clone(),
        Some(None) => String::new(),
        None => made.to_string(),
    };
    let x_axis = cx.axis(title_of(1, &cx.title));
    let y_axis = cy.axis(title_of(2, &cy.title));
    let made = format!("{} by {}", cy.title, cx.title);
    let mut chart = Chart::plot(
        ChartKind::Scatter,
        made,
        x_axis,
        y_axis,
        Plot::Points {
            groups,
            lines: Vec::new(),
            z_axis: None,
        },
    );
    chart.legend = legend;
    Ok(chart)
}

/// The rows of a category and series: (value, weight) pairs.
type Cell = (Option<Class>, Option<Class>, Vec<(f64, f64)>);

/// One series of a categorical element: its name (or the category of the color variable)
/// and its value in each category.
struct Part {
    name: Result<Class, String>,
    values: Vec<(Option<Class>, f64)>,
}

/// Bars, lines, areas or points by category.
fn categorical(
    sets: &[Set],
    s: &Settings,
    elements: &[Element],
    shown: &Shown,
    warn: &mut dyn FnMut(u32, String),
) -> Result<Chart, Error> {
    let var = |name: &String, line: u32| {
        s.vars.get(name).copied().ok_or_else(|| {
            Error(format!(
                "GGRAPH: line {line}: {name} is not a variable of the chart program; DATA \
                 defines them"
            ))
        })
    };
    // the category variable, the series variable, and the value's title, format and kind
    let mut roles: Option<(Option<Var>, Option<Var>, usize, usize)> = None;
    let mut value_info: Option<(String, String, Statistic)> = None;
    let mut parts: Vec<Part> = Vec::new();
    for e in elements {
        let Position::Cross(function, dims) = &e.position else {
            continue;
        };
        let dims: Vec<Var> = dims
            .iter()
            .map(|d| var(d, e.line))
            .collect::<Result<_, _>>()?;
        let f = match function {
            None => None,
            Some(name) => match summary_function(name) {
                Some(f) => Some(f),
                None => {
                    warn(
                        e.line,
                        format!("ELEMENT with summary.{name} is not supported yet and is skipped"),
                    );
                    continue;
                }
            },
        };
        // (category dimension, value dimension, series dimension) by index into dims
        let (cat, value, inner): (Option<usize>, Option<usize>, Option<usize>) = match dims.len() {
            1 if f.is_some() && dims[0].category => (Some(0), None, None),
            1 => (None, Some(0), None),
            2 => (Some(0), Some(1), None),
            3 if s.cluster => (Some(2), Some(1), Some(0)),
            3 => {
                return Err(Error(format!(
                    "GGRAPH: line {}: a third dimension draws panels, which are not supported \
                     yet; COORD: rect(dim(1,2), cluster(3)) clusters the bars instead",
                    e.line
                )));
            }
            _ => {
                return Err(Error(format!(
                    "GGRAPH: line {}: position takes two or three dimensions",
                    e.line
                )));
            }
        };
        let group = match &e.color {
            Some(Ok(name)) => Some(var(name, e.line)?),
            _ => inner.map(|i| dims[i]),
        };
        let set_index = dims[0].set;
        if dims.iter().chain(&group).any(|d| d.set != set_index) {
            return Err(Error(format!(
                "GGRAPH: line {}: an element takes its variables from one graph dataset",
                e.line
            )));
        }
        let set = &sets[set_index];
        let col = |v: Var| &set.columns[v.column];
        let cat_var = cat.map(|i| dims[i]);
        let value_var = value.map(|i| dims[i]);
        match &roles {
            None => {
                roles = Some((
                    cat_var,
                    group,
                    cat.map_or(0, |c| match inner {
                        Some(_) => 3,
                        None => c + 1,
                    }),
                    inner.map_or(0, |i| i + 1),
                ))
            }
            Some((c, ..)) if c.map(|v| (v.set, v.column)) != cat_var.map(|v| (v.set, v.column)) => {
                warn(
                    e.line,
                    "ELEMENT is skipped: its categories differ from the first element's".into(),
                );
                continue;
            }
            _ => {}
        }
        // the cells: (category, series) with their (value, weight) pairs
        let mut cells: Vec<Cell> = Vec::new();
        for r in 0..set.weights.len() {
            let c = match cat_var {
                Some(v) => match col(v).class(r) {
                    Some(class) => Some(class),
                    None => continue,
                },
                None => None,
            };
            let g = match group {
                Some(v) => match col(v).class(r) {
                    Some(class) => Some(class),
                    None => continue,
                },
                None => None,
            };
            let x = match value_var {
                Some(v) => match col(v).number(r) {
                    Some(x) => x,
                    None => continue,
                },
                None => 0.0,
            };
            match cells.iter_mut().find(|cell| cell.0 == c && cell.1 == g) {
                Some(cell) => cell.2.push((x, set.weights[r])),
                None => cells.push((c, g, vec![(x, set.weights[r])])),
            }
            if cells.len() > MOST_CATEGORIES * 100 {
                return Err(Error(format!(
                    "GGRAPH: the chart would have more than the {MOST_CATEGORIES} categories a \
                     chart can show"
                )));
            }
        }
        let mut doubled = false;
        let mut values: Vec<(Option<Class>, Option<Class>, f64)> = cells
            .iter()
            .map(|(c, g, pairs)| {
                let v = match f {
                    // shares are taken of the element's total below
                    Some(F::Pct) if value_var.is_some() => {
                        pairs.iter().map(|p| p.0 * p.1).sum::<f64>()
                    }
                    Some(F::Pct) => compute(F::Count, pairs),
                    Some(f) => compute(f, pairs),
                    None => {
                        doubled |= pairs.len() > 1;
                        pairs.iter().map(|p| p.0).sum::<f64>()
                    }
                };
                (c.clone(), g.clone(), v)
            })
            .collect();
        if doubled {
            warn(e.line, "several rows fall on one bar or point; it shows their sum (a summary function such as summary.mean() says how to combine them)".into());
        }
        if f == Some(F::Pct) {
            let total: f64 = values.iter().map(|v| v.2).sum();
            for v in &mut values {
                v.2 = 100.0 * v.2 / total;
            }
        }
        let (title, format, statistic) = match (f, value_var) {
            (Some(F::Count), _) => (
                "Count".to_string(),
                chart::count_format(values.iter().map(|v| v.2)),
                Statistic::Count,
            ),
            (Some(F::Pct), _) => (
                "Percent".to_string(),
                chart::PERCENT_FORMAT.into(),
                Statistic::Percent,
            ),
            (Some(f), Some(v)) => (
                function_name(f, Some(col(v).title.as_str())),
                procedure_format(&col(v).format, Stat::Value),
                statistic_of(f),
            ),
            (Some(f), None) => (function_name(f, None), String::new(), statistic_of(f)),
            (None, Some(v)) => (
                col(v).title.clone(),
                col(v).format.clone(),
                col(v).statistic,
            ),
            (None, None) => ("Value".to_string(), String::new(), Statistic::Value),
        };
        value_info.get_or_insert((title.clone(), format, statistic));
        let mut series: Vec<Option<Class>> = Vec::new();
        for (_, g, _) in &values {
            if !series.contains(g) {
                series.push(g.clone());
            }
        }
        series.sort_by(|a, b| match (a, b) {
            (Some(a), Some(b)) => order(a, b),
            _ => Ordering::Equal,
        });
        if series.is_empty() {
            series.push(None);
        }
        for g in series {
            parts.push(Part {
                name: match (&g, &e.color) {
                    (Some(class), _) => Ok(class.clone()),
                    (None, Some(Err(text))) => Err(text.clone()),
                    (None, _) => Err(title.clone()),
                },
                values: values
                    .iter()
                    .filter(|v| v.1 == g)
                    .map(|v| (v.0.clone(), v.2))
                    .collect(),
            });
        }
    }
    let Some((cat_var, group, cat_dim, inner_dim)) = roles else {
        return Err(Error(
            "GGRAPH: the chart program has no ELEMENT that can be drawn; see the warnings".into(),
        ));
    };
    let (value_title, value_format, statistic) = value_info.expect("set with roles");
    let col = |v: Var| &sets[v.set].columns[v.column];
    // the categories, with the values SCALE cat includes
    let mut classes: Vec<Option<Class>> = Vec::new();
    for (c, _) in parts.iter().flat_map(|p| &p.values) {
        if !classes.contains(c) {
            classes.push(c.clone());
        }
    }
    let included = |d: usize, v: Option<Var>| -> Vec<Class> {
        let Some(v) = v else { return Vec::new() };
        s.includes
            .iter()
            .filter(|(t, _)| *t == d)
            .flat_map(|(_, values)| values.iter().filter_map(|text| include(col(v), text)))
            .collect()
    };
    if cat_dim > 0 {
        for class in included(cat_dim, cat_var) {
            if !classes.contains(&Some(class.clone())) {
                classes.push(Some(class));
            }
        }
    }
    let mut extra: Vec<Class> = Vec::new();
    for d in [inner_dim, 0] {
        for class in included(d, group) {
            if !parts.iter().any(|p| p.name.as_ref().ok() == Some(&class))
                && !extra.contains(&class)
            {
                extra.push(class);
            }
        }
    }
    let mut extra_parts: Vec<Part> = extra
        .into_iter()
        .map(|class| Part {
            name: Ok(class),
            values: Vec::new(),
        })
        .collect();
    if group.is_some() {
        parts.append(&mut extra_parts);
        parts.sort_by(|a, b| match (&a.name, &b.name) {
            (Ok(a), Ok(b)) => order(a, b),
            _ => Ordering::Equal,
        });
    }
    classes.sort_by(|a, b| match (a, b) {
        (Some(a), Some(b)) => order(a, b),
        _ => Ordering::Equal,
    });
    if classes.is_empty() {
        return Err(Error("GGRAPH: no row has valid values to chart".into()));
    }
    if classes.len() > MOST_CATEGORIES {
        return Err(Error(format!(
            "GGRAPH: the chart would have {} categories, more than the {MOST_CATEGORIES} a chart \
             can show",
            classes.len()
        )));
    }
    let categories: Vec<Category> = classes
        .iter()
        .map(|c| match (c, cat_var) {
            (Some(c), Some(v)) => col(v).label(c, shown),
            _ => Category {
                label: String::new(),
                value: crate::Cell::Empty,
                missing: false,
                bin: None,
            },
        })
        .collect();
    let series: Vec<Series> = parts
        .iter()
        .map(|p| {
            let (name, value) = match (&p.name, group) {
                (Ok(class), Some(g)) => {
                    let c = col(g).label(class, shown);
                    (c.label, c.value)
                }
                (Err(text), _) => (text.clone(), crate::Cell::Empty),
                _ => (value_title.clone(), crate::Cell::Empty),
            };
            Series {
                name,
                statistic,
                variable: group.and_then(|g| match &col(g).kind {
                    Kind::Variable(v) => Some(v.name().to_string()),
                    _ => None,
                }),
                value,
                values: classes
                    .iter()
                    .map(|c| {
                        p.values
                            .iter()
                            .find(|v| &v.0 == c)
                            .map_or(f64::NAN, |v| v.1)
                    })
                    .collect(),
            }
        })
        .collect();
    let title_of = |d: usize, made: String| match s.axes.get(&d) {
        Some(Some(text)) => text.clone(),
        Some(None) => String::new(),
        None => made,
    };
    let category_axis = match cat_var {
        Some(v) => col(v).axis(title_of(1, col(v).title.clone())),
        None => Axis::titled(title_of(1, String::new())),
    };
    let value_axis = Axis {
        title: title_of(2, value_title.clone()),
        format: value_format,
        maximum: None,
        variable: None,
    };
    let legend = group.map(|g| col(g).title.clone());
    let made = std::iter::once(value_title)
        .chain(cat_var.map(|v| col(v).title.clone()))
        .chain(legend.clone())
        .collect::<Vec<_>>()
        .join(" by ");
    Ok(Chart {
        kind: ChartKind::Bar,
        tier: Tier::Presentation,
        orientation: Some(Orientation::Vertical),
        grouping: Some(Grouping::Clustered),
        title: made,
        subtitle: None,
        footnote: None,
        category_axis,
        value_axis,
        legend,
        categories,
        series,
        normal: None,
        error_bars: None,
        second_axis: None,
        plot: None,
        outline: None,
        hidden: false,
    })
}

/// The category of a value that SCALE cat includes, as the column holds it; None when the
/// text is not a value of the column.
fn include(column: &Column, text: &str) -> Option<Class> {
    let Kind::Variable(v) = &column.kind else {
        let x: f64 = text.trim().parse().ok()?;
        return Some(Class {
            key: Some(Key::Number(x)),
            missing: false,
        });
    };
    let (key, missing) = match v.is_string() {
        true => (
            Key::Text(text.trim_end().to_string()),
            v.is_user_missing_str(text),
        ),
        false => {
            let x: f64 = text.trim().parse().ok()?;
            (Key::Number(x), v.is_user_missing(x))
        }
    };
    // a user-missing value is a category only when the data report it
    let reported = column
        .cells
        .iter()
        .any(|c| matches!(c, Datum::Class(c) if c.missing));
    (!missing || reported).then_some(Class {
        key: Some(key),
        missing,
    })
}

/// COORD polar.theta: the slices are the series of a single stack.
fn pie(chart: &mut Chart, mark: Mark) -> Result<(), Error> {
    if mark != Mark::Bar || chart.categories.len() != 1 {
        return Err(Error(
            "GGRAPH: polar coordinates draw a pie: interval.stack with one value dimension \
             split by color"
                .into(),
        ));
    }
    chart.categories = chart
        .series
        .iter()
        .map(|s| Category {
            label: s.name.clone(),
            value: s.value.clone(),
            missing: false,
            bin: None,
        })
        .collect();
    let values = chart.series.iter().map(|s| s.values[0]).collect();
    let mut series = chart.series.remove(0);
    series.name = chart.value_axis.title.clone();
    series.value = crate::Cell::Empty;
    series.values = values;
    chart.series = vec![series];
    chart.kind = ChartKind::Pie;
    chart.orientation = None;
    chart.grouping = None;
    chart.category_axis.title = chart.legend.take().unwrap_or_default();
    Ok(())
}
