//! GRAPH: one chart per command (per split group) in the description of `chart.rs`.
//!
//! Rules taken from the manual:
//! - A bar, line or pie chart shows one function by one or two category variables, several
//!   functions by one category variable, or several functions without one (a bar or slice per
//!   function). The data decide the layout; STACKED stacks the bars of several series, AREA
//!   fills under the lines and stacks them.
//! - Count functions count the cases of a category; PCT is a share of all cases in the chart.
//!   The summary functions take the valid values of their variable in the category.
//! - MISSING: LISTWISE drops a case missing on any summary variable of the chart, VARIABLE only
//!   from the functions of that variable. NOREPORT drops a case whose category is missing,
//!   REPORT keeps such categories. INCLUDE makes user-missing values valid everywhere.
//! - A category in which every series is empty is left out. VALUE draws every case; its
//!   missing values are empty bars whatever MISSING says.
//! - A histogram counts the valid values in equal bins; NORMAL adds the normal curve. A
//!   scatterplot draws a point per case valid on both variables, grouped by the control
//!   variable.
//!
//! Where the manual is silent (recorded in the planning notes):
//! - Weights count in every function; VALUE and scatterplots draw cases unweighted, leaving
//!   out cases with a zero, negative or missing weight.
//! - Categories ascend by value, valid ones first, then the user-missing ones and system
//!   missing (REPORT).
//! - CUFREQ, CUPCT and CUSUM accumulate within each series along the categories; CUPCT is a
//!   share of the series' own total, so each series ends at 100.
//! - Percentiles use the rule of FREQUENCIES, (W + 1) p; MEDIAN is the 50th; MODE is the
//!   smallest of tied values; GMEDIAN treats the values as midpoints of groups.
//! - Without TITLE the chart is named after what it shows, such as "Mean of price by region".

use crate::aggregate::{median, statistic};
use crate::chart::{
    self, Axis, Category, Chart, ChartKind, Grouping, Orientation, Series, Statistic, Tier,
};
use crate::crosstabs::Key;
use crate::descriptives::case_weights;
use crate::examine::Distribution;
use crate::format::{Stat, procedure_format};
use crate::frequencies::Histogram;
use crate::plot::{Plot, Point, PointGroup};
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Values, Variable};
use statscraft_syntax::{
    AggregateFunction as A, Graph, GraphChart, GraphData, GraphFunction as F, GraphSummary,
};
use std::cmp::Ordering;

/// The most categories a chart can show.
const MOST_CATEGORIES: usize = 3000;

/// The category of a case: its value (None when system-missing) and whether it is missing.
#[derive(Clone, PartialEq)]
struct Class {
    key: Option<Key>,
    missing: bool,
}

fn class_of(variable: &Variable, case: usize, include: bool) -> Class {
    match variable.values() {
        Values::Numeric(values) => match values.get(case) {
            Some(x) => Class {
                key: Some(Key::Number(x)),
                missing: !include && variable.is_user_missing(x),
            },
            None => Class {
                key: None,
                missing: true,
            },
        },
        Values::String(values) => {
            let text = values.get(case);
            Class {
                key: Some(Key::Text(text.into())),
                missing: !include && variable.is_user_missing_str(text),
            }
        }
    }
}

/// Valid categories first, then missing ones, each ascending; system-missing last.
fn order(a: &Class, b: &Class) -> Ordering {
    a.missing
        .cmp(&b.missing)
        .then_with(|| match (&a.key, &b.key) {
            (Some(x), Some(y)) => x.compare(y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        })
}

/// The distinct classes in their order.
fn distinct<'a>(classes: impl Iterator<Item = &'a Class>) -> Vec<Class> {
    let mut out: Vec<Class> = Vec::new();
    for class in classes {
        if !out.contains(class) {
            out.push(class.clone());
        }
    }
    out.sort_by(order);
    out
}

fn category(variable: &Variable, class: &Class, shown: &Shown) -> Category {
    let (label, value) = match &class.key {
        Some(key) => (
            shown.value(variable, key),
            match key {
                Key::Number(n) => Cell::Number(*n),
                Key::Text(t) => Cell::Text(t.clone()),
            },
        ),
        None => ("System missing".to_string(), Cell::Empty),
    };
    Category {
        label,
        value,
        missing: class.missing,
        bin: None,
    }
}

/// The valid value of a numeric variable at a case.
fn valid(variable: &Variable, case: usize, include: bool) -> Option<f64> {
    let value = variable.numeric().ok()?.get(case)?;
    (include || !variable.is_user_missing(value)).then_some(value)
}

fn numeric<'a>(data: &'a Dataset, name: &str, role: &str) -> Result<Variable<'a>, Error> {
    let variable = data.variable(name)?;
    match variable.is_string() {
        true => Err(Error(format!(
            "GRAPH: {} is a string variable; {role} needs a numeric one",
            variable.name()
        ))),
        false => Ok(variable),
    }
}

/// A number as written in a label: 2.5, 10.
fn plain(x: f64) -> String {
    format!("{x}")
}

/// What a function computes, with its variable when it has one.
fn function_name(function: F, variable: Option<&str>) -> String {
    let of = variable.map_or(String::new(), |v| format!(" of {v}"));
    match function {
        F::Count => "Count".into(),
        F::Pct => "Percent".into(),
        F::Cupct => "Cumulative Percent".into(),
        F::Cufreq => "Cumulative Count".into(),
        F::Minimum => format!("Minimum{of}"),
        F::Maximum => format!("Maximum{of}"),
        F::N => format!("N{of}"),
        F::Sum => format!("Sum{of}"),
        F::Cusum => format!("Cumulative Sum{of}"),
        F::Mean => format!("Mean{of}"),
        F::StdDev => format!("Std. Deviation{of}"),
        F::Variance => format!("Variance{of}"),
        F::Median => format!("Median{of}"),
        F::Gmedian => format!("Grouped Median{of}"),
        F::Mode => format!("Mode{of}"),
        F::Ptile(p) => format!("Percentile {}{of}", plain(p)),
        F::Plt(x) => format!("%{of} < {}", plain(x)),
        F::Pgt(x) => format!("%{of} > {}", plain(x)),
        F::Nlt(x) => format!("N{of} < {}", plain(x)),
        F::Ngt(x) => format!("N{of} > {}", plain(x)),
        F::Pin(a, b) => format!("%{of} in {} to {}", plain(a), plain(b)),
        F::Nin(a, b) => format!("N{of} in {} to {}", plain(a), plain(b)),
    }
}

/// What the numbers of a function are.
fn statistic_of(function: F) -> Statistic {
    match function {
        F::Count | F::Cufreq | F::N | F::Nlt(_) | F::Ngt(_) | F::Nin(..) => Statistic::Count,
        F::Pct | F::Cupct | F::Plt(_) | F::Pgt(_) | F::Pin(..) => Statistic::Percent,
        F::Mean => Statistic::Mean,
        _ => Statistic::Value,
    }
}

/// The most frequent value; the smallest of tied ones.
fn mode(pairs: &[(f64, f64)]) -> Option<f64> {
    let mut sorted = pairs.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut best: Option<(f64, f64)> = None;
    let mut i = 0;
    while i < sorted.len() {
        let value = sorted[i].0;
        let mut weight = 0.0;
        while i < sorted.len() && sorted[i].0 == value {
            weight += sorted[i].1;
            i += 1;
        }
        if best.is_none_or(|(_, w)| weight > w) {
            best = Some((value, weight));
        }
    }
    best.map(|(value, _)| value)
}

/// A function of the (value, weight) pairs of one cell; NaN where it is undefined. The count
/// functions give the summed weight here; percentages and accumulation come after.
fn compute(function: F, pairs: &[(f64, f64)]) -> f64 {
    let n: f64 = pairs.iter().map(|p| p.1).sum();
    let count = |keep: &dyn Fn(f64) -> bool| -> f64 {
        pairs.iter().filter(|p| keep(p.0)).map(|p| p.1).sum()
    };
    let share = |c: f64| match n > 0.0 {
        true => 100.0 * c / n,
        false => f64::NAN,
    };
    let value = match function {
        F::Count | F::Pct | F::Cufreq | F::Cupct | F::N => Some(n),
        F::Sum | F::Cusum => statistic(A::Sum, pairs),
        F::Mean => statistic(A::Mean, pairs),
        F::StdDev => statistic(A::Sd, pairs),
        F::Variance => statistic(A::Sd, pairs).map(|sd| sd * sd),
        F::Median => median(pairs),
        F::Gmedian => crate::means::grouped_median(pairs),
        F::Mode => mode(pairs),
        F::Minimum => pairs.iter().map(|p| p.0).reduce(f64::min),
        F::Maximum => pairs.iter().map(|p| p.0).reduce(f64::max),
        F::Ptile(p) => Distribution::new(pairs).frequencies_percentile(p),
        F::Plt(x) => Some(share(count(&|v| v < x))),
        F::Pgt(x) => Some(share(count(&|v| v > x))),
        F::Nlt(x) => Some(count(&|v| v < x)),
        F::Ngt(x) => Some(count(&|v| v > x)),
        F::Pin(a, b) => Some(share(count(&|v| v >= a && v <= b))),
        F::Nin(a, b) => Some(count(&|v| v >= a && v <= b)),
    };
    value.unwrap_or(f64::NAN)
}

/// One function over one variable (none for a count function).
struct Item<'a> {
    function: F,
    variable: Option<Variable<'a>>,
}

impl Item<'_> {
    fn name(&self, shown: &Shown) -> String {
        let variable = self.variable.as_ref().map(|v| shown.variable(v));
        function_name(self.function, variable.as_deref())
    }

    /// The display format of the item's numbers.
    fn format(&self, values: &[f64]) -> String {
        match statistic_of(self.function) {
            Statistic::Count => {
                chart::count_format(values.iter().copied().filter(|v| v.is_finite()))
            }
            Statistic::Percent => chart::PERCENT_FORMAT.into(),
            _ => {
                let kind = match self.function {
                    F::Sum | F::Cusum => Stat::Sum,
                    F::StdDev => Stat::Spread,
                    F::Variance => Stat::Other,
                    _ => Stat::Value,
                };
                let format = self.variable.as_ref().map_or("", |v| v.format());
                procedure_format(format, kind)
            }
        }
    }
}

/// Everything a chart builder needs.
struct Context<'a> {
    data: &'a Dataset,
    shown: Shown,
    outline: Shown,
    weights: Option<Vec<f64>>,
    command: &'a Graph,
    case_numbers: Option<&'a [usize]>,
}

impl Context<'_> {
    fn weight(&self, case: usize) -> f64 {
        self.weights.as_ref().map_or(1.0, |w| w[case])
    }

    /// The title the syntax gave, or `made`.
    fn title(&self, made: String) -> String {
        match self.command.title.is_empty() {
            true => made,
            false => self.command.title.join(" "),
        }
    }

    fn axis(&self, variable: &Variable, numeric_format: bool) -> Axis {
        Axis {
            title: self.shown.variable(variable),
            format: match numeric_format && !variable.is_string() {
                true => variable.format().to_string(),
                false => String::new(),
            },
            maximum: None,
            variable: Some(variable.name().to_string()),
        }
    }

    fn items(&self, functions: &[GraphSummary]) -> Result<Vec<Item<'_>>, Error> {
        let mut items = Vec::new();
        for summary in functions {
            if summary.function.counts_cases() {
                items.push(Item {
                    function: summary.function,
                    variable: None,
                });
                continue;
            }
            for name in crate::expand_in(self.data, &summary.variables)? {
                items.push(Item {
                    function: summary.function,
                    variable: Some(numeric(self.data, &name, "a summary function")?),
                });
            }
        }
        Ok(items)
    }

    /// The categories and series of a summary chart, with the axes and the made-up title.
    fn summary(&self, functions: &[GraphSummary], by: &[String]) -> Result<Chart, Error> {
        let (include, report) = (self.command.include, self.command.report);
        let items = self.items(functions)?;
        let counting = items.iter().any(|i| i.function.counts_cases());
        if counting && by.is_empty() {
            return Err(Error("GRAPH: a count function needs a BY variable".into()));
        }
        if items.len() > 1 && by.len() == 2 {
            return Err(Error(
                "GRAPH: with two BY variables the chart takes one function of one variable".into(),
            ));
        }
        let by_vars: Vec<Variable> = by
            .iter()
            .map(|name| self.data.variable(name))
            .collect::<Result<_, _>>()?;
        // (classes, item, value, weight) of every valid value
        let mut records: Vec<(Vec<Class>, usize, f64, f64)> = Vec::new();
        for case in 0..self.data.case_count() {
            let w = self.weight(case);
            if w <= 0.0 {
                continue;
            }
            let classes: Vec<Class> = by_vars.iter().map(|v| class_of(v, case, include)).collect();
            if !report && classes.iter().any(|c| c.missing) {
                continue;
            }
            let values: Vec<Option<f64>> = items
                .iter()
                .map(|item| match &item.variable {
                    None => Some(0.0),
                    Some(v) => valid(v, case, include),
                })
                .collect();
            if self.command.listwise && values.iter().any(Option::is_none) {
                continue;
            }
            for (i, value) in values.into_iter().enumerate() {
                if let Some(value) = value {
                    records.push((classes.clone(), i, value, w));
                }
            }
        }
        let rows = distinct(records.iter().filter_map(|r| r.0.first()));
        let columns = distinct(records.iter().filter_map(|r| r.0.get(1)));
        let (n_categories, n_series) = match by.len() {
            0 => (items.len(), 1),
            1 => (rows.len(), items.len()),
            _ => (rows.len(), columns.len()),
        };
        if n_categories == 0 {
            return Err(Error("GRAPH: no valid cases to chart".into()));
        }
        if n_categories > MOST_CATEGORIES {
            return Err(Error(format!(
                "GRAPH: the chart would have {n_categories} categories, more than the \
                 {MOST_CATEGORIES} a chart can show"
            )));
        }
        // the item of the cell in series s and category k
        let item_of = |s: usize, k: usize| match by.len() {
            0 => k,
            1 => s,
            _ => 0,
        };
        let mut cells = vec![vec![Vec::new(); n_categories]; n_series];
        for (classes, i, value, w) in &records {
            let (s, k) = match by.len() {
                0 => (0, *i),
                1 => (
                    *i,
                    rows.iter().position(|c| *c == classes[0]).expect("seen"),
                ),
                _ => (
                    columns.iter().position(|c| *c == classes[1]).expect("seen"),
                    rows.iter().position(|c| *c == classes[0]).expect("seen"),
                ),
            };
            cells[s][k].push((*value, *w));
        }
        let mut values: Vec<Vec<f64>> = (0..n_series)
            .map(|s| {
                (0..n_categories)
                    .map(|k| compute(items[item_of(s, k)].function, &cells[s][k]))
                    .collect()
            })
            .collect();
        // shares of the whole chart, and sums along the categories
        let whole: f64 = values.iter().flatten().filter(|v| v.is_finite()).sum();
        for (s, series) in values.iter_mut().enumerate() {
            if by.is_empty() {
                break;
            }
            let function = items[item_of(s, 0)].function;
            let own: f64 = series.iter().filter(|v| v.is_finite()).sum();
            let mut running = 0.0;
            for v in series.iter_mut() {
                if v.is_finite() {
                    running += *v;
                }
                match function {
                    F::Pct => *v = 100.0 * *v / whole,
                    F::Cufreq | F::Cusum => *v = running,
                    F::Cupct => *v = 100.0 * running / own,
                    _ => {}
                }
            }
        }
        let shown = &self.shown;
        let item_names: Vec<String> = items.iter().map(|i| i.name(shown)).collect();
        let categories: Vec<Category> = match by.len() {
            0 => item_names
                .iter()
                .map(|name| Category {
                    label: name.clone(),
                    value: Cell::Empty,
                    missing: false,
                    bin: None,
                })
                .collect(),
            _ => rows
                .iter()
                .map(|c| category(&by_vars[0], c, shown))
                .collect(),
        };
        let series: Vec<Series> = values
            .into_iter()
            .enumerate()
            .map(|(s, values)| {
                let item = &items[item_of(s, 0)];
                let (name, variable, value) = match by.len() {
                    2 => {
                        let c = category(&by_vars[1], &columns[s], shown);
                        (c.label, Some(by_vars[1].name().to_string()), c.value)
                    }
                    0 => (
                        match items.len() {
                            1 => item_names[0].clone(),
                            _ => "Value".into(),
                        },
                        None,
                        Cell::Empty,
                    ),
                    _ => (
                        item_names[s].clone(),
                        item.variable.as_ref().map(|v| v.name().to_string()),
                        Cell::Empty,
                    ),
                };
                Series {
                    name,
                    statistic: statistic_of(item.function),
                    variable,
                    value,
                    values,
                }
            })
            .collect();
        let value_title = match &items[..] {
            [one] => one.name(shown),
            [first, ..] if items.iter().all(|i| i.function == first.function) => {
                function_name(first.function, None)
            }
            _ => "Value".into(),
        };
        let all: Vec<f64> = series.iter().flat_map(|s| s.values.clone()).collect();
        let value_axis = Axis {
            title: value_title,
            format: items[0].format(&all),
            maximum: None,
            variable: None,
        };
        let category_axis = match by_vars.first() {
            Some(v) => self.axis(v, true),
            None => Axis::titled(String::new()),
        };
        let outline_names: Vec<String> = by_vars.iter().map(|v| self.outline.variable(v)).collect();
        let lead = match &items[..] {
            [one] => one.name(&self.outline),
            _ => items
                .iter()
                .map(|i| i.name(&self.outline))
                .collect::<Vec<_>>()
                .join(", "),
        };
        let made = std::iter::once(lead)
            .chain(outline_names)
            .collect::<Vec<_>>()
            .join(" by ");
        Ok(Chart {
            kind: ChartKind::Bar,
            tier: Tier::Presentation,
            orientation: Some(Orientation::Vertical),
            grouping: Some(Grouping::Clustered),
            title: self.title(made),
            subtitle: None,
            footnote: None,
            category_axis,
            value_axis,
            legend: by_vars.get(1).map(|v| shown.variable(v)),
            categories,
            series,
            normal: None,
            plot: None,
            error_bars: None,
        })
    }

    /// VALUE: a category per case, a series per variable; missing values are empty.
    fn values(
        &self,
        variables: &[statscraft_syntax::VarItem],
        by: Option<&str>,
    ) -> Result<Chart, Error> {
        let n = self.data.case_count();
        if n > MOST_CATEGORIES {
            return Err(Error(format!(
                "GRAPH: VALUE would draw {n} cases, more than the {MOST_CATEGORIES} categories \
                 a chart can show"
            )));
        }
        let vars: Vec<Variable> = crate::expand_in(self.data, variables)?
            .iter()
            .map(|name| numeric(self.data, name, "VALUE"))
            .collect::<Result<_, _>>()?;
        let by = by.map(|name| self.data.variable(name)).transpose()?;
        let kept: Vec<usize> = (0..n).filter(|&case| self.weight(case) > 0.0).collect();
        let number = |case: usize| self.case_numbers.map_or(case, |n| n[case]) + 1;
        let categories: Vec<Category> = kept
            .iter()
            .map(|&case| match &by {
                Some(v) => category(v, &class_of(v, case, true), &self.shown),
                None => Category {
                    label: number(case).to_string(),
                    value: Cell::Number(number(case) as f64),
                    missing: false,
                    bin: None,
                },
            })
            .collect();
        let series: Vec<Series> = vars
            .iter()
            .map(|v| Series {
                name: self.shown.variable(v),
                statistic: Statistic::Value,
                variable: Some(v.name().to_string()),
                value: Cell::Empty,
                values: kept
                    .iter()
                    .map(|&case| valid(v, case, false).unwrap_or(f64::NAN))
                    .collect(),
            })
            .collect();
        if series.iter().all(|s| s.values.iter().all(|v| v.is_nan())) {
            return Err(Error("GRAPH: VALUE found no valid value to chart".into()));
        }
        let names: Vec<String> = vars.iter().map(|v| self.outline.variable(v)).collect();
        let value_axis = Axis {
            title: match &vars[..] {
                [one] => self.shown.variable(one),
                _ => "Value".into(),
            },
            format: procedure_format(vars[0].format(), Stat::Value),
            maximum: None,
            variable: None,
        };
        let category_axis = match &by {
            Some(v) => self.axis(v, true),
            None => Axis::titled("Case Number"),
        };
        Ok(Chart {
            kind: ChartKind::Bar,
            tier: Tier::Presentation,
            orientation: Some(Orientation::Vertical),
            grouping: Some(Grouping::Clustered),
            title: self.title(format!("Values of {}", names.join(", "))),
            subtitle: None,
            footnote: None,
            category_axis,
            value_axis,
            legend: None,
            categories,
            series,
            normal: None,
            plot: None,
            error_bars: None,
        })
    }

    fn histogram(&self, name: &str, normal: bool) -> Result<Chart, Error> {
        let variable = numeric(self.data, name, "HISTOGRAM")?;
        let mut pairs: Vec<(f64, f64)> = (0..self.data.case_count())
            .filter(|&case| self.weight(case) > 0.0)
            .filter_map(|case| {
                Some((
                    valid(&variable, case, self.command.include)?,
                    self.weight(case),
                ))
            })
            .collect();
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let total: f64 = pairs.iter().map(|p| p.1).sum();
        let title = self.title(self.outline.variable(&variable));
        let heading = self.shown.variable(&variable);
        let histogram = Histogram {
            title: &title,
            heading: &heading,
            format: variable.format(),
            name: variable.name(),
            normal,
            scale: None,
        };
        histogram
            .chart(&pairs, &pairs, total)
            .map_err(|e| Error(format!("GRAPH: {}: {e}", variable.name())))
    }

    fn scatter(
        &self,
        x: &str,
        y: &str,
        control: Option<&str>,
        names: Option<&str>,
    ) -> Result<Chart, Error> {
        let include = self.command.include;
        let (xv, yv) = (
            numeric(self.data, x, "SCATTERPLOT")?,
            numeric(self.data, y, "SCATTERPLOT")?,
        );
        let control = control.map(|name| self.data.variable(name)).transpose()?;
        let names = names.map(|name| self.data.variable(name)).transpose()?;
        let mut points: Vec<(Option<Class>, Point)> = Vec::new();
        for case in 0..self.data.case_count() {
            if self.weight(case) <= 0.0 {
                continue;
            }
            let (Some(px), Some(py)) = (valid(&xv, case, include), valid(&yv, case, include))
            else {
                continue;
            };
            let class = control.as_ref().map(|v| class_of(v, case, include));
            if class.as_ref().is_some_and(|c| c.missing) && !self.command.report {
                continue;
            }
            // a system-missing name shows no label; a user-missing one does
            let label = names.as_ref().and_then(|v| {
                let class = class_of(v, case, true);
                class.key.map(|key| self.shown.value(v, &key))
            });
            points.push((
                class,
                Point {
                    x: px,
                    y: py,
                    z: None,
                    label,
                },
            ));
        }
        if points.is_empty() {
            return Err(Error("GRAPH: no case has valid values on both axes".into()));
        }
        let groups = match &control {
            None => vec![PointGroup {
                name: self.shown.variable(&yv),
                points: points.into_iter().map(|(_, p)| p).collect(),
            }],
            Some(v) => distinct(points.iter().filter_map(|(c, _)| c.as_ref()))
                .into_iter()
                .map(|class| PointGroup {
                    name: category(v, &class, &self.shown).label,
                    points: points
                        .iter()
                        .filter(|(c, _)| c.as_ref() == Some(&class))
                        .map(|(_, p)| p.clone())
                        .collect(),
                })
                .collect(),
        };
        let made = format!(
            "{} by {}",
            self.outline.variable(&yv),
            self.outline.variable(&xv)
        );
        let mut chart = Chart::plot(
            ChartKind::Scatter,
            self.title(made),
            self.axis(&xv, true),
            self.axis(&yv, true),
            Plot::Points {
                groups,
                lines: Vec::new(),
                z_axis: None,
            },
        );
        chart.legend = control.as_ref().map(|v| self.shown.variable(v));
        Ok(chart)
    }
}

impl Engine {
    pub(crate) fn graph(&mut self, command: &Graph) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let context = Context {
            data,
            shown: self.shown(),
            outline: self.outline(),
            weights: case_weights(data)?,
            command,
            case_numbers: self.case_numbers.as_deref(),
        };
        let mut chart = match &command.chart {
            GraphChart::Bar { stacked, data } => {
                let mut chart = context.categorical(data)?;
                if *stacked && chart.series.len() > 1 {
                    chart.grouping = Some(Grouping::Stacked);
                }
                chart
            }
            GraphChart::Line { area, data } => {
                let mut chart = context.categorical(data)?;
                let several = chart.series.len() > 1;
                chart.kind = match area {
                    true => ChartKind::Area,
                    false => ChartKind::Line,
                };
                // several areas stack, so each line is the sum of the ones below
                chart.grouping = (*area && several).then_some(Grouping::Stacked);
                chart
            }
            GraphChart::Pie(data) => {
                let mut chart = context.categorical(data)?;
                if chart.series.len() > 1 {
                    return Err(Error(
                        "GRAPH: a pie takes one function by one BY variable, or summary \
                         functions without BY"
                            .into(),
                    ));
                }
                chart.kind = ChartKind::Pie;
                chart.orientation = None;
                chart.grouping = None;
                chart
            }
            GraphChart::Histogram { variable, normal } => context.histogram(variable, *normal)?,
            GraphChart::Scatter {
                x,
                y,
                control,
                names,
            } => context.scatter(x, y, control.as_deref(), names.as_deref())?,
        };
        chart.subtitle = command.subtitle.clone();
        if !command.footnote.is_empty() {
            let given = command.footnote.join(" ");
            chart.footnote = Some(match chart.footnote.take() {
                // the histogram's own note on the mean and spread stays after it
                Some(own) => format!("{}. {own}", given.trim_end_matches('.')),
                None => given,
            });
        }
        self.charts.push(chart);
        Ok(Vec::new())
    }
}

impl Context<'_> {
    fn categorical(&self, data: &GraphData) -> Result<Chart, Error> {
        match data {
            GraphData::Summary { functions, by } => self.summary(functions, by),
            GraphData::Values { variables, by } => self.values(variables, by.as_deref()),
        }
    }
}

#[cfg(test)]
#[test]
fn functions_of_one_cell() {
    // values 2, 4, 4, 9 with weights 1, 1, 2, 1
    let pairs = [(2.0, 1.0), (4.0, 1.0), (4.0, 2.0), (9.0, 1.0)];
    assert_eq!(compute(F::N, &pairs), 5.0);
    assert_eq!(compute(F::Sum, &pairs), 2.0 + 4.0 + 8.0 + 9.0);
    assert_eq!(compute(F::Mean, &pairs), 23.0 / 5.0);
    assert_eq!(compute(F::Mode, &pairs), 4.0);
    assert_eq!(compute(F::Minimum, &pairs), 2.0);
    assert_eq!(compute(F::Maximum, &pairs), 9.0);
    assert_eq!(compute(F::Median, &pairs), 4.0);
    assert_eq!(compute(F::Nlt(4.0), &pairs), 1.0);
    assert_eq!(compute(F::Pgt(4.0), &pairs), 20.0);
    assert_eq!(compute(F::Nin(4.0, 9.0), &pairs), 4.0);
    assert_eq!(compute(F::Pin(3.0, 5.0), &pairs), 60.0);
    // sum of squares about 4.6: 6.76 + 0.36 + 0.72 + 19.36 = 27.2, over W - 1 = 4
    assert!((compute(F::Variance, &pairs) - 6.8).abs() < 1e-12);
    assert!(compute(F::Mean, &[]).is_nan());
    assert_eq!(compute(F::Count, &[]), 0.0);
    // tied modes: the smaller value
    assert_eq!(mode(&[(3.0, 1.0), (1.0, 1.0)]), Some(1.0));
}
