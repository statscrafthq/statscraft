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
//! - ERRORBAR lays out means as a summary chart of MEAN does. A confidence interval is the mean
//!   plus and minus t at (1 + level) / 2 with W - 1 degrees of freedom times the standard
//!   error; STERROR and STDDEV reach n standard errors or deviations.
//! - PARETO sorts the categories by their total, largest first; the cumulative line is the
//!   running total as a percentage of the grand total, on a second axis from 0 to 100.
//! - BAR (RANGE) takes its series in pairs; LINE (DROP) needs two series or more, LINE
//!   (DIFFERENCE) two; HILO takes high, low and an optional close.
//! - PANEL draws a chart for each combination of the panel variables' categories.
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
//! - Each panel is a chart of its own, named in its title; a combination without cases has no
//!   chart. A case missing on a panel variable is left out unless REPORT.
//! - Pareto categories with equal totals keep their order by value. NOCUM leaves out the right
//!   axis with the line.

use crate::aggregate::{median, statistic};
use crate::chart::{
    self, Axis, Category, Chart, ChartKind, ErrorBars, Grouping, Orientation, Series, Statistic,
    Tier,
};
use crate::crosstabs::Key;
use crate::descriptives::case_weights;
use crate::dist::{beta_i, t_quantile};
use crate::examine::Distribution;
use crate::format::{Stat, procedure_format};
use crate::frequencies::Histogram;
use crate::plot::{CaseGroup, CaseValues, Plot, Point, PointGroup};
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Values, Variable};
use statscraft_syntax::{
    AggregateFunction as A, Graph, GraphChart, GraphData, GraphFunction as F, GraphInterval,
    GraphSummary, VarItem,
};
use std::cmp::Ordering;

/// The most categories a chart can show.
pub(crate) const MOST_CATEGORIES: usize = 3000;

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

/// What an error bar spans, in words.
fn interval_label(interval: GraphInterval) -> String {
    match interval {
        GraphInterval::Ci(level) => format!("{}% CI", plain(level)),
        GraphInterval::Se(n) => format!("{} SE", plain(n)),
        GraphInterval::StdDev(n) => format!("{} SD", plain(n)),
    }
}

/// The half length of the error bar around the mean of the (value, weight) pairs; NaN with a
/// total weight of 1 or less, where the spread is undefined.
fn margin(interval: GraphInterval, pairs: &[(f64, f64)]) -> f64 {
    let w: f64 = pairs.iter().map(|p| p.1).sum();
    let Some(sd) = statistic(A::Sd, pairs).filter(|_| w > 1.0) else {
        return f64::NAN;
    };
    let se = sd / w.sqrt();
    match interval {
        GraphInterval::Ci(level) => t_quantile((1.0 - level / 100.0) / 2.0, w - 1.0) * se,
        GraphInterval::Se(n) => n * se,
        GraphInterval::StdDev(n) => n * sd,
    }
}

/// The interval of the median of the (value, weight) pairs at `level` percent, interpolated
/// between order statistics (Hettmansperger and Sheather): each case counts its weight times,
/// so the weights must be whole numbers. None without cases.
fn median_interval(level: f64, pairs: &[(f64, f64)]) -> Result<Option<[f64; 2]>, Error> {
    if pairs.iter().any(|p| p.1.fract() != 0.0) {
        return Err(Error(
            "GRAPH: the confidence interval of a median needs whole case weights".into(),
        ));
    }
    let mut sorted = pairs.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    let n = sorted.iter().map(|p| p.1).sum::<f64>() as usize;
    if n == 0 {
        return Ok(None);
    }
    // the i-th smallest value, 1-based, with each case repeated by its weight; the ends stand
    // for the indices 0 and n + 1, which come in with a factor of 0
    let x = |i: usize| {
        let i = i.clamp(1, n) as f64;
        let mut seen = 0.0;
        sorted
            .iter()
            .find(|p| {
                seen += p.1;
                seen >= i
            })
            .map_or(f64::NAN, |p| p.0)
    };
    // P(B >= i) and P(i <= B <= n - i) for B binomial with n trials and chance 1/2
    let at_least = |i: usize| match i {
        0 => 1.0,
        _ if i > n => 0.0,
        _ => beta_i(i as f64, (n - i + 1) as f64, 0.5),
    };
    let between = |i: usize| match i > n / 2 {
        true => 0.0,
        false => at_least(i) - at_least(n - i + 1),
    };
    let p = level / 100.0;
    // the coverage falls with k, so the first k whose next one is at most p
    let k = (0..=n / 2).find(|&k| between(k + 1) <= p).unwrap_or(n / 2);
    let (above, below) = (between(k), between(k + 1));
    let l = (above - p) / (above - below);
    let (nf, kf) = (n as f64, k as f64);
    let lambda = (nf - kf) * l / (kf + (nf - 2.0 * kf) * l);
    Ok(Some([
        lambda * x(k + 1) + (1.0 - lambda) * x(k),
        lambda * x(n - k) + (1.0 - lambda) * x(n - k + 1),
    ]))
}

/// How far the error bar of `function` reaches below and above its value over the pairs: a
/// mean's by `margin`, a median's to the ends of its interval. NaN where there is no bar.
fn bar_lengths(
    interval: GraphInterval,
    function: F,
    pairs: &[(f64, f64)],
) -> Result<[f64; 2], Error> {
    match (function, interval) {
        (F::Median, GraphInterval::Ci(level)) => {
            let value = compute(F::Median, pairs);
            Ok(match median_interval(level, pairs)? {
                Some([lo, hi]) => [value - lo, hi - value],
                None => [f64::NAN; 2],
            })
        }
        _ => {
            let m = margin(interval, pairs);
            Ok([m, m])
        }
    }
}

/// INTERVAL takes means, and medians with a confidence interval: the functions the error bars
/// have a formula for.
fn check_interval(interval: GraphInterval, function: F) -> Result<(), Error> {
    match (function, interval) {
        (F::Mean, _) | (F::Median, GraphInterval::Ci(_)) => Ok(()),
        (F::Median, _) => Err(Error(
            "GRAPH: the error bars of a median are its confidence interval; give INTERVAL CI, \
             or use MEAN for SE and STDDEV"
                .into(),
        )),
        (other, _) => Err(Error(format!(
            "GRAPH: INTERVAL draws error bars around means and medians; {} is not supported yet",
            function_name(other, None)
        ))),
    }
}

/// Pareto: the categories by their total over the series, largest first, and the cumulative
/// percentage as a last series on a second axis when `cumulative`.
fn pareto(chart: &mut Chart, cumulative: bool) {
    let total = |i: usize| -> f64 {
        chart
            .series
            .iter()
            .map(|s| s.values[i])
            .filter(|v| v.is_finite())
            .sum()
    };
    let totals: Vec<f64> = (0..chart.categories.len()).map(total).collect();
    let mut order: Vec<usize> = (0..totals.len()).collect();
    // stable, and with -0.0 equal to 0.0 (an empty sum): equal totals keep their order by value
    order.sort_by(|&a, &b| totals[b].partial_cmp(&totals[a]).unwrap_or(Ordering::Equal));
    chart.categories = order.iter().map(|&i| chart.categories[i].clone()).collect();
    for series in &mut chart.series {
        series.values = order.iter().map(|&i| series.values[i]).collect();
    }
    if chart.series.len() > 1 {
        chart.grouping = Some(Grouping::Stacked);
    }
    if !cumulative {
        return;
    }
    let grand: f64 = totals.iter().sum();
    let mut running = 0.0;
    let values = order
        .iter()
        .map(|&i| {
            running += totals[i];
            100.0 * running / grand
        })
        .collect();
    let title = "Cumulative Percent".to_string();
    chart.series.push(Series {
        name: title.clone(),
        statistic: Statistic::Percent,
        variable: None,
        value: Cell::Empty,
        values,
    });
    chart.second_axis = Some(Axis {
        title,
        format: chart::PERCENT_FORMAT.into(),
        maximum: Some(100.0),
        variable: None,
    });
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

    /// The categories and series of a summary chart, with the axes and the made-up title, and
    /// error bars around the values when an `interval` is given.
    fn summary(
        &self,
        functions: &[GraphSummary],
        by: &[String],
        interval: Option<GraphInterval>,
    ) -> Result<Chart, Error> {
        let (include, report) = (self.command.include, self.command.report);
        let items = self.items(functions)?;
        if let Some(interval) = interval {
            for item in &items {
                check_interval(interval, item.function)?;
            }
        }
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
        let error_bars = match interval {
            None => None,
            Some(interval) => {
                let mut margins = vec![vec![f64::NAN; n_categories]; n_series];
                let mut lower = margins.clone();
                for s in 0..n_series {
                    for k in 0..n_categories {
                        let function = items[item_of(s, k)].function;
                        [lower[s][k], margins[s][k]] =
                            bar_lengths(interval, function, &cells[s][k])?;
                    }
                }
                // only a series with a median has bars that differ below and above
                let uneven = |s: usize| {
                    (0..n_categories).any(|k| items[item_of(s, k)].function == F::Median)
                };
                for (s, l) in lower.iter_mut().enumerate() {
                    if !uneven(s) {
                        l.clear();
                    }
                }
                if lower.iter().all(Vec::is_empty) {
                    lower.clear();
                }
                Some(ErrorBars {
                    label: interval_label(interval),
                    margins,
                    lower,
                })
            }
        };
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
            error_bars,
            second_axis: None,
            outline: None,
            hidden: false,
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
            second_axis: None,
            outline: None,
            hidden: false,
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

    /// The numeric variables of a list.
    fn numeric_list(&self, list: &[VarItem]) -> Result<Vec<Variable<'_>>, Error> {
        crate::expand_in(self.data, list)?
            .iter()
            .map(|name| numeric(self.data, name, "SCATTERPLOT"))
            .collect()
    }

    /// The label of a case's point: the value of the NAME variable; none when it is
    /// system-missing, while a user-missing value shows.
    fn point_label(&self, names: Option<&Variable>, case: usize) -> Option<String> {
        let v = names?;
        let class = class_of(v, case, true);
        class.key.map(|key| self.shown.value(v, &key))
    }

    /// The items in groups by the classes of the control variable, in category order; one
    /// group named `single` without a control variable.
    fn grouped<T: Clone>(
        &self,
        control: Option<&Variable>,
        items: Vec<(Option<Class>, T)>,
        single: String,
    ) -> Vec<(String, Vec<T>)> {
        match control {
            None => vec![(single, items.into_iter().map(|(_, t)| t).collect())],
            Some(v) => distinct(items.iter().filter_map(|(c, _)| c.as_ref()))
                .into_iter()
                .map(|class| {
                    let members = items
                        .iter()
                        .filter(|(c, _)| c.as_ref() == Some(&class))
                        .map(|(_, t)| t.clone())
                        .collect();
                    (category(v, &class, &self.shown).label, members)
                })
                .collect(),
        }
    }

    /// The class of a case on the control variable; Err(()) when the case is left out because
    /// that class is missing and MISSING is not REPORT.
    fn control_class(&self, control: Option<&Variable>, case: usize) -> Result<Option<Class>, ()> {
        let class = control.map(|v| class_of(v, case, self.command.include));
        match class.as_ref().is_some_and(|c| c.missing) && !self.command.report {
            true => Err(()),
            false => Ok(class),
        }
    }

    /// One plot of y against x, with a third axis z when given: a point per case valid on
    /// all of them.
    fn scatter(
        &self,
        x: &str,
        y: &str,
        z: Option<&str>,
        control: Option<&str>,
        names: Option<&str>,
    ) -> Result<Chart, Error> {
        let include = self.command.include;
        let (xv, yv) = (
            numeric(self.data, x, "SCATTERPLOT")?,
            numeric(self.data, y, "SCATTERPLOT")?,
        );
        let zv = z
            .map(|name| numeric(self.data, name, "SCATTERPLOT"))
            .transpose()?;
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
            let pz = zv.as_ref().map(|v| valid(v, case, include));
            if pz == Some(None) {
                continue;
            }
            let Ok(class) = self.control_class(control.as_ref(), case) else {
                continue;
            };
            points.push((
                class,
                Point {
                    x: px,
                    y: py,
                    z: pz.flatten(),
                    label: self.point_label(names.as_ref(), case),
                },
            ));
        }
        if points.is_empty() {
            return Err(Error("GRAPH: no case has valid values on all axes".into()));
        }
        let groups = self
            .grouped(control.as_ref(), points, self.shown.variable(&yv))
            .into_iter()
            .map(|(name, points)| PointGroup { name, points })
            .collect();
        let mut made = format!(
            "{} by {}",
            self.outline.variable(&yv),
            self.outline.variable(&xv)
        );
        if let Some(zv) = &zv {
            made = format!("{made} and {}", self.outline.variable(zv));
        }
        let mut chart = Chart::plot(
            ChartKind::Scatter,
            self.title(made),
            self.axis(&xv, true),
            self.axis(&yv, true),
            Plot::Points {
                groups,
                lines: Vec::new(),
                z_axis: zv.as_ref().map(|v| self.axis(v, true)),
            },
        );
        chart.legend = control.as_ref().map(|v| self.shown.variable(v));
        if let Some(zv) = &zv {
            // shortcut: a plot with three axes is not drawn; its table lists the points until
            // a projected drawing is made
            chart.footnote = Some(format!(
                "The third axis, {}, is not drawn; the table lists every point.",
                self.shown.variable(zv)
            ));
        }
        Ok(chart)
    }

    /// Several plots in one frame: a group of points per pair of an x and a y variable, every
    /// combination or, with `pair`, the first with the first and so on.
    fn overlay(
        &self,
        x: &[VarItem],
        y: &[VarItem],
        pair: bool,
        names: Option<&str>,
    ) -> Result<Chart, Error> {
        let include = self.command.include;
        let (xs, ys) = (self.numeric_list(x)?, self.numeric_list(y)?);
        let pairs: Vec<(usize, usize)> = match pair {
            true if xs.len() != ys.len() => {
                return Err(Error(format!(
                    "GRAPH: PAIR needs as many variables after WITH as before; got {} and {}",
                    xs.len(),
                    ys.len()
                )));
            }
            true => (0..xs.len()).map(|i| (i, i)).collect(),
            false => (0..xs.len())
                .flat_map(|i| (0..ys.len()).map(move |j| (i, j)))
                .collect(),
        };
        let names = names.map(|name| self.data.variable(name)).transpose()?;
        let shown = &self.shown;
        let mut groups: Vec<PointGroup> = pairs
            .iter()
            .map(|&(i, j)| PointGroup {
                name: format!("{} by {}", shown.variable(&ys[j]), shown.variable(&xs[i])),
                points: Vec::new(),
            })
            .collect();
        for case in 0..self.data.case_count() {
            if self.weight(case) <= 0.0 {
                continue;
            }
            let xv: Vec<Option<f64>> = xs.iter().map(|v| valid(v, case, include)).collect();
            let yv: Vec<Option<f64>> = ys.iter().map(|v| valid(v, case, include)).collect();
            if self.command.listwise && xv.iter().chain(&yv).any(Option::is_none) {
                continue;
            }
            let label = self.point_label(names.as_ref(), case);
            for (group, &(i, j)) in groups.iter_mut().zip(&pairs) {
                if let (Some(px), Some(py)) = (xv[i], yv[j]) {
                    group.points.push(Point {
                        x: px,
                        y: py,
                        z: None,
                        label: label.clone(),
                    });
                }
            }
        }
        if groups.iter().all(|g| g.points.is_empty()) {
            return Err(Error(
                "GRAPH: no case has valid values on both variables of a pair".into(),
            ));
        }
        // an axis is titled by its variables, each once
        let titled = |vars: &[Variable], names: &Shown| -> Vec<String> {
            let mut titles: Vec<String> = Vec::new();
            for v in vars {
                let title = names.variable(v);
                if !titles.contains(&title) {
                    titles.push(title);
                }
            }
            titles
        };
        // the axis shows its numbers in the format of its variable with the most decimals
        let decimals = |v: &Variable| {
            let format = v.format();
            let after = format.rsplit_once('.').map_or("", |(_, d)| d);
            after.parse::<usize>().unwrap_or(0)
        };
        let list_axis = |vars: &[Variable]| {
            // the first of equals: max_by_key keeps the last, so the list is read backwards
            let finest = vars
                .iter()
                .rev()
                .max_by_key(|v| decimals(v))
                .unwrap_or(&vars[0]);
            let mut axis = self.axis(finest, true);
            let titles = titled(vars, shown);
            if titles.len() > 1 {
                axis.title = titles.join(", ");
                axis.variable = None;
            }
            axis
        };
        let made = format!(
            "{} by {}",
            titled(&ys, &self.outline).join(", "),
            titled(&xs, &self.outline).join(", ")
        );
        Ok(Chart::plot(
            ChartKind::Scatter,
            self.title(made),
            list_axis(&xs),
            list_axis(&ys),
            Plot::Points {
                groups,
                lines: Vec::new(),
                z_axis: None,
            },
        ))
    }

    /// A scatterplot matrix: every case with a valid value on two of the variables or more,
    /// grouped by the control variable.
    fn matrix(
        &self,
        variables: &[VarItem],
        control: Option<&str>,
        names: Option<&str>,
    ) -> Result<Chart, Error> {
        let include = self.command.include;
        let vars = self.numeric_list(variables)?;
        if vars.len() < 2 {
            return Err(Error(
                "GRAPH: a scatterplot matrix needs two variables or more".into(),
            ));
        }
        let control = control.map(|name| self.data.variable(name)).transpose()?;
        let names = names.map(|name| self.data.variable(name)).transpose()?;
        let mut cases: Vec<(Option<Class>, CaseValues)> = Vec::new();
        for case in 0..self.data.case_count() {
            if self.weight(case) <= 0.0 {
                continue;
            }
            let values: Vec<Option<f64>> = vars.iter().map(|v| valid(v, case, include)).collect();
            let known = values.iter().flatten().count();
            if known < 2 || (self.command.listwise && known < values.len()) {
                continue;
            }
            let Ok(class) = self.control_class(control.as_ref(), case) else {
                continue;
            };
            cases.push((
                class,
                CaseValues {
                    values: values.iter().map(|v| v.unwrap_or(f64::NAN)).collect(),
                    label: self.point_label(names.as_ref(), case),
                },
            ));
        }
        if cases.is_empty() {
            return Err(Error(
                "GRAPH: no case has valid values on two of the variables".into(),
            ));
        }
        let groups = self
            .grouped(control.as_ref(), cases, String::new())
            .into_iter()
            .map(|(name, cases)| CaseGroup { name, cases })
            .collect();
        let listed: Vec<String> = vars.iter().map(|v| self.outline.variable(v)).collect();
        let mut chart = Chart::plot(
            ChartKind::Scatter,
            self.title(format!("Scatterplot matrix of {}", listed.join(", "))),
            Axis::titled(String::new()),
            Axis::titled(String::new()),
            Plot::Matrix {
                variables: vars.iter().map(|v| self.axis(v, true)).collect(),
                groups,
            },
        );
        chart.legend = control.as_ref().map(|v| self.shown.variable(v));
        Ok(chart)
    }
}

impl Engine {
    pub(crate) fn graph(&mut self, command: &Graph) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let mut context = Context {
            data,
            shown: self.shown(),
            outline: self.outline(),
            weights: case_weights(data)?,
            command,
            case_numbers: self.case_numbers.as_deref(),
        };
        let Some(panel) = &command.panel else {
            let chart = context.chart()?;
            self.charts.push(chart);
            return Ok(Vec::new());
        };
        // a chart per panel: the cases outside it weigh nothing, which leaves them out
        let vars: Vec<Variable> = panel
            .rows
            .iter()
            .chain(&panel.columns)
            .map(|name| data.variable(name))
            .collect::<Result<_, _>>()?;
        let classes: Vec<Option<Vec<Class>>> = (0..data.case_count())
            .map(|case| {
                let classes: Vec<Class> = vars
                    .iter()
                    .map(|v| class_of(v, case, command.include))
                    .collect();
                (command.report || classes.iter().all(|c| !c.missing)).then_some(classes)
            })
            .collect();
        let mut panels: Vec<&Vec<Class>> = Vec::new();
        for cell in classes.iter().flatten() {
            if !panels.contains(&cell) {
                panels.push(cell);
            }
        }
        panels.sort_by(|a, b| {
            a.iter()
                .zip(b.iter())
                .map(|(x, y)| order(x, y))
                .find(|o| o.is_ne())
                .unwrap_or(Ordering::Equal)
        });
        let weights = context.weights.take();
        let (mut charts, mut failed) = (Vec::new(), Vec::new());
        for cell in panels {
            context.weights = Some(
                (0..data.case_count())
                    .map(|case| match classes[case].as_ref() == Some(cell) {
                        true => weights.as_ref().map_or(1.0, |w| w[case]),
                        false => 0.0,
                    })
                    .collect(),
            );
            let name = vars
                .iter()
                .zip(cell)
                .map(|(v, class)| {
                    let value = category(v, class, &context.outline).label;
                    format!("{} = {value}", context.outline.variable(v))
                })
                .collect::<Vec<_>>()
                .join(", ");
            match context.chart() {
                Ok(mut chart) => {
                    chart.title = format!("{} ({name})", chart.title);
                    charts.push(chart);
                }
                Err(e) => failed.push(format!("{name}: {}", e.0)),
            }
        }
        let Some(first) = failed.first().cloned() else {
            self.charts.extend(charts);
            return Ok(Vec::new());
        };
        if charts.is_empty() {
            return Err(Error(first));
        }
        self.charts.extend(charts);
        for message in failed {
            self.warn(message);
        }
        Ok(Vec::new())
    }
}

impl Context<'_> {
    /// The chart of the command over the cases with a positive weight.
    fn chart(&self) -> Result<Chart, Error> {
        let command = self.command;
        let interval = command.interval;
        if interval.is_some()
            && !matches!(
                command.chart,
                GraphChart::Bar { .. } | GraphChart::Line { .. }
            )
        {
            return Err(Error(
                "GRAPH: INTERVAL adds error bars to bar and line charts only; ERRORBAR draws \
                 means with their bars"
                    .into(),
            ));
        }
        let mut chart = match &command.chart {
            GraphChart::Bar { stacked, data } => {
                let mut chart = self.categorical(data, interval)?;
                if *stacked && chart.series.len() > 1 {
                    chart.grouping = Some(Grouping::Stacked);
                }
                chart
            }
            GraphChart::Line { area, data } => {
                let mut chart = self.categorical(data, interval)?;
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
                let mut chart = self.categorical(data, None)?;
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
            GraphChart::Histogram { variable, normal } => self.histogram(variable, *normal)?,
            GraphChart::Scatter {
                x,
                y,
                control,
                names,
            } => self.scatter(x, y, None, control.as_deref(), names.as_deref())?,
            GraphChart::Xyz {
                x,
                y,
                z,
                control,
                names,
            } => self.scatter(x, y, Some(z), control.as_deref(), names.as_deref())?,
            GraphChart::Overlay { x, y, pair, names } => {
                self.overlay(x, y, *pair, names.as_deref())?
            }
            GraphChart::Matrix {
                variables,
                control,
                names,
            } => self.matrix(variables, control.as_deref(), names.as_deref())?,
            GraphChart::ErrorBar {
                interval,
                variables,
                by,
            } => {
                let means = [GraphSummary {
                    function: F::Mean,
                    variables: variables.clone(),
                }];
                let mut chart = self.summary(&means, by, Some(*interval))?;
                chart.kind = ChartKind::ErrorBar;
                chart
            }
            GraphChart::Pareto { cumulative, data } => {
                let mut chart = self.categorical(data, None)?;
                pareto(&mut chart, *cumulative);
                chart
            }
            GraphChart::Range(data) => self.ranges(data, ChartKind::RangeBar)?,
            GraphChart::Drop(data) => self.ranges(data, ChartKind::DropLine)?,
            GraphChart::Difference(data) => self.ranges(data, ChartKind::Difference)?,
            GraphChart::Hilo(data) => self.ranges(data, ChartKind::HighLow)?,
        };
        if interval.is_some() && chart.grouping == Some(Grouping::Stacked) {
            return Err(Error(
                "GRAPH: INTERVAL takes simple and clustered charts; stacked bars and areas have \
                 no error bars"
                    .into(),
            ));
        }
        chart.subtitle = command.subtitle.clone();
        if !command.footnote.is_empty() {
            let given = command.footnote.join(" ");
            chart.footnote = Some(match chart.footnote.take() {
                // the histogram's own note on the mean and spread stays after it
                Some(own) => format!("{}. {own}", given.trim_end_matches('.')),
                None => given,
            });
        }
        Ok(chart)
    }

    /// A range bar, drop-line, difference or high-low chart: the series are the ends of the
    /// ranges, so their number must fit the kind.
    fn ranges(&self, data: &GraphData, kind: ChartKind) -> Result<Chart, Error> {
        let mut chart = self.categorical(data, None)?;
        let n = chart.series.len();
        let problem = match kind {
            ChartKind::RangeBar if n < 2 || n % 2 == 1 => Some(
                "BAR(RANGE) takes its ends in pairs: an even number of functions or variables, \
                 or a second BY variable with two values",
            ),
            ChartKind::DropLine if n < 2 => Some(
                "LINE(DROP) needs two series or more: several functions, or a second BY variable",
            ),
            ChartKind::Difference if n != 2 => Some(
                "LINE(DIFFERENCE) needs two series: two functions, or a second BY variable with \
                 two values",
            ),
            ChartKind::HighLow if n < 2 => Some(
                "HILO needs a high and a low series: two or three functions, or a second BY \
                 variable with two or three values",
            ),
            ChartKind::HighLow if n > 3 => {
                return Err(Error(format!(
                    "GRAPH: HILO with {n} series (GROUPED) is not supported yet; give two or \
                     three"
                )));
            }
            _ => None,
        };
        if let Some(problem) = problem {
            return Err(Error(format!("GRAPH: {problem}; this chart has {n}")));
        }
        chart.kind = kind;
        chart.grouping = None;
        Ok(chart)
    }

    fn categorical(
        &self,
        data: &GraphData,
        interval: Option<GraphInterval>,
    ) -> Result<Chart, Error> {
        match data {
            GraphData::Summary { functions, by } => self.summary(functions, by, interval),
            GraphData::Values { .. } if interval.is_some() => Err(Error(
                "GRAPH: INTERVAL needs a summary function; a VALUE chart shows single cases".into(),
            )),
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
