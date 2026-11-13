//! SURVIVAL: actuarial life tables over fixed intervals, the median survival time, plots of the
//! functions and the Wilcoxon (Gehan) comparison of groups.
//!
//! Rules from the Command Syntax Reference (SURVIVAL) and the Algorithms manual (SURVIVAL
//! Algorithms), in our words:
//! - Intervals start at 0. Each `THRU end BY width` adds intervals of that width from where
//!   the last period stopped until `end` is reached, the last one reaching past `end` when the
//!   width does not divide the period. An open interval after them holds every longer time.
//!   A time belongs to the interval whose start it reaches.
//! - Per interval: l entering (the first interval gets every case, the next ones what is left
//!   after withdrawals c and events d), exposed r = l - c/2, q = d/r, p = 1 - q, cumulative
//!   P = P_prev p, density (P_prev - P)/h and hazard 2q / (h (1 + p)) for width h. Standard
//!   errors: P sqrt(sum q/(r p)); for the density P_prev q / h times the root of the earlier
//!   sum plus p/(r q); for the hazard sqrt(hazard^2 / (r q) (1 - (hazard h / 2)^2)), 0 when
//!   q is 0. The open interval has no width, so no density or hazard. Intervals that no case
//!   enters are not shown.
//! - Median: inside the interval where P first drops below 0.5, interpolated linearly from
//!   P_prev at its start; when P never drops below 0.5 before the open interval, the start
//!   of the open interval with a plus sign.
//! - Gehan's comparison scores each case: a censored case counts the events at or before its
//!   time; an event counts the events strictly before it, minus the censored cases at or after
//!   it and the events after it. With W cases, SS_j the score sum of group j and T the sum
//!   of squared scores, D = (W - 1) sum(SS_j^2 / W_j) / T is chi-square with g - 1 degrees of
//!   freedom for g groups with cases. Weights multiply cases throughout.

use crate::chart::{Axis, Chart, ChartKind};
use crate::descriptives::{case_weights, number};
use crate::dist::chi_square_p;
use crate::means::is_valid;
use crate::plot::{Plot, Point, PointGroup};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Variable;
use statscraft_syntax::{LifePlot, Survival, TableSelection, VarItem};

/// The interval starts; the last one opens the interval without end. Also says whether some
/// period was stretched.
pub(crate) fn interval_starts(periods: &[(f64, f64)]) -> Result<(Vec<f64>, bool), Error> {
    let mut starts = vec![0.0];
    let mut stretched = false;
    for &(end, width) in periods {
        let base = *starts.last().expect("never empty");
        if end <= base {
            return Err(Error(format!(
                "INTERVAL: the period up to {end} lies inside the stretched period before it"
            )));
        }
        let ratio = (end - base) / width;
        let count = (ratio - 1e-9).ceil().max(1.0) as usize;
        stretched |= (ratio - ratio.round()).abs() > 1e-9;
        starts.extend((1..=count).map(|i| base + i as f64 * width));
    }
    Ok((starts, stretched))
}

/// One line of a life table.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Line {
    pub start: f64,
    /// None for the open interval.
    pub width: Option<f64>,
    pub entering: f64,
    pub withdrawn: f64,
    pub exposed: f64,
    pub events: f64,
    pub q: f64,
    pub p: f64,
    pub cumulative: f64,
    pub se_cumulative: f64,
    pub density: Option<f64>,
    pub se_density: Option<f64>,
    pub hazard: Option<f64>,
    pub se_hazard: Option<f64>,
}

/// The life table of (time, event, weight) cases, only the intervals some case enters.
pub(crate) fn life_table(cases: &[(f64, bool, f64)], starts: &[f64]) -> Vec<Line> {
    let k = starts.len();
    let (mut c, mut d) = (vec![0.0; k], vec![0.0; k]);
    for &(t, event, w) in cases {
        let i = starts.partition_point(|&s| s <= t).saturating_sub(1);
        match event {
            true => d[i] += w,
            false => c[i] += w,
        }
    }
    let mut out = Vec::new();
    let mut entering: f64 = cases.iter().map(|c| c.2).sum();
    let (mut previous, mut sum) = (1.0, 0.0);
    for i in 0..k {
        if entering <= 0.0 {
            break;
        }
        let width = (i + 1 < k).then(|| starts[i + 1] - starts[i]);
        let r = entering - c[i] / 2.0;
        let q = if r > 0.0 { d[i] / r } else { 0.0 };
        let p = 1.0 - q;
        let cumulative = previous * p;
        let before = sum;
        if q > 0.0 && p > 0.0 {
            sum += q / (r * p);
        }
        let se_cumulative = cumulative * sum.sqrt();
        let (density, se_density, hazard, se_hazard) = match width {
            Some(h) => {
                let hazard = 2.0 * q / (h * (1.0 + p));
                let (se_f, se_h) = match q > 0.0 {
                    true => (
                        previous * q / h * (before + p / (r * q)).sqrt(),
                        (hazard * hazard / (r * q) * (1.0 - (hazard * h / 2.0).powi(2)))
                            .max(0.0)
                            .sqrt(),
                    ),
                    false => (0.0, 0.0),
                };
                (
                    Some((previous - cumulative) / h),
                    Some(se_f),
                    Some(hazard),
                    Some(se_h),
                )
            }
            None => (None, None, None, None),
        };
        out.push(Line {
            start: starts[i],
            width,
            entering,
            withdrawn: c[i],
            exposed: r,
            events: d[i],
            q,
            p,
            cumulative,
            se_cumulative,
            density,
            se_density,
            hazard,
            se_hazard,
        });
        entering -= c[i] + d[i];
        previous = cumulative;
    }
    out
}

/// The median survival time; `Err(start)` when it lies at or past the open interval's start.
pub(crate) fn median(lines: &[Line], starts: &[f64]) -> Result<f64, f64> {
    let open = *starts.last().expect("never empty");
    let mut previous = 1.0;
    for line in lines {
        if line.cumulative < 0.5 && previous >= 0.5 {
            return match line.width {
                Some(h) => Ok(line.start + h * (previous - 0.5) / (previous - line.cumulative)),
                None => Err(open),
            };
        }
        previous = line.cumulative;
    }
    Err(open)
}

/// Gehan's statistic of (time, event, weight, group) cases: D, its degrees of freedom and the
/// mean score of each group (by group index, None for an empty one).
pub(crate) fn gehan(cases: &[(f64, bool, f64, usize)], groups: usize) -> Option<(f64, usize)> {
    let mut sorted: Vec<f64> = cases.iter().map(|c| c.0).collect();
    sorted.sort_by(f64::total_cmp);
    sorted.dedup();
    let m = sorted.len();
    let (mut u_at, mut c_at) = (vec![0.0; m], vec![0.0; m]);
    let index = |t: f64| sorted.partition_point(|&s| s < t);
    for &(t, event, w, _) in cases {
        match event {
            true => u_at[index(t)] += w,
            false => c_at[index(t)] += w,
        }
    }
    let (mut u_le, mut c_le) = (vec![0.0; m], vec![0.0; m]);
    let (mut u, mut c) = (0.0, 0.0);
    for i in 0..m {
        u += u_at[i];
        c += c_at[i];
        u_le[i] = u;
        c_le[i] = c;
    }
    let (wu, wc) = (u, c);
    let (mut sums, mut weights) = (vec![0.0; groups], vec![0.0; groups]);
    let mut t2 = 0.0;
    for &(t, event, w, g) in cases {
        let i = index(t);
        let score = match event {
            false => u_le[i],
            true => (u_le[i] - u_at[i]) - (wc - c_le[i] + c_at[i]) - (wu - u_le[i]),
        };
        sums[g] += w * score;
        weights[g] += w;
        t2 += w * score * score;
    }
    let total: f64 = weights.iter().sum();
    let nonempty = weights.iter().filter(|&&w| w > 0.0).count();
    if nonempty < 2 || t2 <= 0.0 {
        return None;
    }
    let b: f64 = sums
        .iter()
        .zip(&weights)
        .filter(|(_, w)| **w > 0.0)
        .map(|(s, w)| s * s / w)
        .sum();
    Some(((total - 1.0) * b / t2, nonempty - 1))
}

/// A control variable resolved: its name and whole values from min to max.
struct ControlVar<'a> {
    variable: Variable<'a>,
    min: i64,
    max: i64,
}

impl ControlVar<'_> {
    /// The control value of a case, None when missing or out of range.
    fn value(&self, case: usize, include: bool) -> Option<i64> {
        if !is_valid(&self.variable, case, include) {
            return None;
        }
        let v = self.variable.numeric().ok()?.get(case)?.trunc();
        (v >= self.min as f64 && v <= self.max as f64).then_some(v as i64)
    }
}

/// A table selection of PLOTS or COMPARE as sets of names per place; None means every one.
fn selected(
    engine: &Engine,
    selection: &TableSelection,
    places: &[Vec<String>],
) -> Result<Vec<Option<Vec<String>>>, Error> {
    let mut out = Vec::new();
    for (k, names) in places.iter().enumerate() {
        match selection.get(k) {
            None => out.push(None),
            Some(items) if items.contains(&VarItem::All) => out.push(None),
            Some(items) => {
                let chosen = engine.expand(items)?;
                if let Some(bad) = chosen.iter().find(|n| !names.contains(n)) {
                    return Err(Error(format!(
                        "{bad} is not in that place on TABLES, so it cannot be selected here"
                    )));
                }
                out.push(Some(chosen));
            }
        }
    }
    Ok(out)
}

fn is_chosen(choice: &Option<Vec<String>>, name: &str) -> bool {
    choice
        .as_ref()
        .is_none_or(|names| names.iter().any(|n| n == name))
}

/// A number as an interval bound in text: 30, 2.5.
fn bound_text(v: f64) -> String {
    let text = format!("{:.2}", v);
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

impl Engine {
    pub(crate) fn life_tables(&mut self, command: &Survival) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let numeric = |name: &str| -> Result<Variable, Error> {
            let v = data.variable(name)?;
            match v.is_string() {
                true => Err(Error(format!(
                    "SURVIVAL needs numeric variables, but {name} is a string variable"
                ))),
                false => Ok(v),
            }
        };
        let times = self.expand(&command.variables)?;
        let mut controls: Vec<Vec<ControlVar>> = Vec::new();
        for list in &command.controls {
            let mut order = Vec::new();
            for control in list {
                for name in self.expand(std::slice::from_ref(&control.variable))? {
                    if times.contains(&name) {
                        return Err(Error(format!(
                            "{name} is a survival variable and cannot also be a control"
                        )));
                    }
                    order.push(ControlVar {
                        variable: numeric(&name)?,
                        min: control.min,
                        max: control.max,
                    });
                }
            }
            controls.push(order);
        }
        // the status of each survival variable: FOR first, then the last STATUS without FOR
        let mut status: Vec<Option<&statscraft_syntax::SurvivalStatus>> = vec![None; times.len()];
        for spec in &command.status {
            if let Some(items) = &spec.applies_to {
                for name in self.expand(items)? {
                    let Some(i) = times.iter().position(|t| *t == name) else {
                        return Err(Error(format!(
                            "STATUS ... FOR names {name}, which is not a survival variable"
                        )));
                    };
                    status[i] = Some(spec);
                }
            }
        }
        if let Some(general) = command.status.iter().rfind(|s| s.applies_to.is_none()) {
            for s in status.iter_mut().filter(|s| s.is_none()) {
                *s = Some(general);
            }
        }
        let (starts, stretched) = interval_starts(&command.intervals)?;
        let mut warnings = Vec::new();
        if stretched {
            warnings.push(format!(
                "INTERVAL: a width did not divide its period evenly, so the period was stretched; \
                 the intervals start at {}",
                starts
                    .iter()
                    .map(|s| bound_text(*s))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let weights = case_weights(data)?;
        let weight = |c: usize| weights.as_ref().map_or(1.0, |w| w[c]);
        let include = command.include;
        let mut time_vars = Vec::new();
        let mut status_vars = Vec::new();
        for (name, spec) in times.iter().zip(&status) {
            let Some(spec) = spec else {
                return Err(Error(format!("SURVIVAL needs a STATUS for {name}")));
            };
            time_vars.push(numeric(name)?);
            status_vars.push((numeric(&spec.variable)?, spec.min, spec.max));
        }
        let time_of = |v: usize, c: usize| -> Option<f64> {
            if !is_valid(&time_vars[v], c, include) {
                return None;
            }
            time_vars[v].numeric().ok()?.get(c).filter(|t| *t >= 0.0)
        };
        let event_of = |v: usize, c: usize| -> Option<bool> {
            let (var, lo, hi) = &status_vars[v];
            if !is_valid(var, c, include) {
                return None;
            }
            var.numeric().ok()?.get(c).map(|s| s >= *lo && s <= *hi)
        };
        let n = data.case_count();
        // LISTWISE: a case missing on any TABLES variable is out of every table
        let listwise_ok: Vec<bool> = (0..n)
            .map(|c| {
                !command.listwise
                    || ((0..times.len()).all(|v| time_of(v, c).is_some())
                        && controls
                            .iter()
                            .flatten()
                            .all(|cv| is_valid(&cv.variable, c, include)))
            })
            .collect();

        let first_names: Vec<String> = controls.first().map_or(Vec::new(), |l| {
            l.iter().map(|c| c.variable.name().to_string()).collect()
        });
        let second_names: Vec<String> = controls.get(1).map_or(Vec::new(), |l| {
            l.iter().map(|c| c.variable.name().to_string()).collect()
        });
        let places = [times.clone(), first_names.clone(), second_names.clone()];
        let plot_choice = match &command.plots {
            Some((_, selection)) if !command.compare_only => {
                Some(selected(self, selection, &places)?)
            }
            _ => None,
        };
        let compare_choice = match &command.compare {
            Some(selection) => Some(selected(self, selection, &places)?),
            None => None,
        };
        if command.compare.is_some() && first_names.is_empty() {
            warnings.push(
                "COMPARE needs a first order control variable; no comparison was made".to_string(),
            );
        }

        let mut tables = Vec::new();
        let mut charts = Vec::new();
        let mut overall_rows = Vec::new();
        let mut pair_rows = Vec::new();
        let mut median_rows = Vec::new();
        let has_second = !second_names.is_empty();
        let firsts: Vec<Option<&ControlVar>> = match controls.first() {
            Some(list) => list.iter().map(Some).collect(),
            None => vec![None],
        };
        let seconds: Vec<Option<&ControlVar>> = match controls.get(1) {
            Some(list) => list.iter().map(Some).collect(),
            None => vec![None],
        };
        for (v, time_name) in times.iter().enumerate() {
            let time_label = shown.variable(&time_vars[v]);
            let mut table_rows = Vec::new();
            for second in &seconds {
                for first in &firsts {
                    let second_values: Vec<Option<i64>> = match second {
                        Some(cv) => (cv.min..=cv.max).map(Some).collect(),
                        None => vec![None],
                    };
                    for b in second_values {
                        // the cases of each first order value in this second order group
                        let first_values: Vec<Option<i64>> = match first {
                            Some(cv) => (cv.min..=cv.max).map(Some).collect(),
                            None => vec![None],
                        };
                        let mut groups: Vec<(Option<i64>, Vec<(f64, bool, f64)>)> = Vec::new();
                        for &a in &first_values {
                            let cases: Vec<(f64, bool, f64)> = (0..n)
                                .filter(|&c| weight(c) > 0.0 && listwise_ok[c])
                                .filter(|&c| second.is_none_or(|cv| cv.value(c, include) == b))
                                .filter(|&c| first.is_none_or(|cv| cv.value(c, include) == a))
                                .filter_map(|c| Some((time_of(v, c)?, event_of(v, c)?, weight(c))))
                                .collect();
                            if !cases.is_empty() {
                                groups.push((a, cases));
                            }
                        }
                        let value_text = |cv: &ControlVar, value: i64| {
                            shown.value(&cv.variable, &crate::crosstabs::Key::Number(value as f64))
                        };
                        let first_name = first.map(|cv| cv.variable.name().to_string());
                        let second_name = second.map(|cv| cv.variable.name().to_string());
                        let group_label = |a: Option<i64>| -> Vec<Cell> {
                            let mut out = Vec::new();
                            if let (Some(cv), Some(b)) = (second, b) {
                                out.push(Cell::Text(format!(
                                    "{} = {}",
                                    shown.variable(&cv.variable),
                                    value_text(cv, b)
                                )));
                            }
                            if let (Some(cv), Some(a)) = (first, a) {
                                out.push(Cell::Text(format!(
                                    "{} = {}",
                                    shown.variable(&cv.variable),
                                    value_text(cv, a)
                                )));
                            }
                            out
                        };
                        let mut lines_of = Vec::new();
                        for (a, cases) in &groups {
                            let lines = life_table(cases, &starts);
                            if !command.compare_only {
                                for (k, line) in lines.iter().enumerate() {
                                    let mut row = match k {
                                        0 => group_label(*a),
                                        _ => vec![
                                            Cell::Text(String::new());
                                            usize::from(first.is_some())
                                                + usize::from(has_second)
                                        ],
                                    };
                                    row.resize(
                                        usize::from(first.is_some()) + usize::from(has_second),
                                        Cell::Text(String::new()),
                                    );
                                    row.extend([
                                        Cell::Number(line.start),
                                        Cell::Number(line.entering),
                                        Cell::Number(line.withdrawn),
                                        Cell::Number(line.exposed),
                                        Cell::Number(line.events),
                                        Cell::Number(line.q),
                                        Cell::Number(line.p),
                                        Cell::Number(line.cumulative),
                                        Cell::Number(line.se_cumulative),
                                        number(line.density),
                                        number(line.se_density),
                                        number(line.hazard),
                                        number(line.se_hazard),
                                    ]);
                                    table_rows.push(row);
                                }
                                let mut row = vec![Cell::Text(time_label.clone())];
                                let mut labels = group_label(*a);
                                labels.resize(2, Cell::Text(String::new()));
                                row.extend(labels);
                                row.push(match median(&lines, &starts) {
                                    Ok(m) => Cell::Number(m),
                                    Err(open) => Cell::Text(format!("{}+", bound_text(open))),
                                });
                                median_rows.push(row);
                            }
                            lines_of.push((*a, lines));
                        }
                        // the comparison of the first order groups
                        let compare_this = compare_choice.as_ref().is_some_and(|choice| {
                            is_chosen(&choice[0], time_name)
                                && first_name
                                    .as_ref()
                                    .is_some_and(|f| is_chosen(&choice[1], f))
                                && second_name
                                    .as_ref()
                                    .is_none_or(|s| is_chosen(&choice[2], s))
                        });
                        if compare_this && let Some(cv) = first {
                            let score_time = |t: f64| match command.approximate {
                                true => starts[starts.partition_point(|&s| s <= t) - 1],
                                false => t,
                            };
                            let all: Vec<(f64, bool, f64, usize)> = groups
                                .iter()
                                .enumerate()
                                .flat_map(|(g, (_, cases))| {
                                    cases.iter().map(move |&(t, e, w)| (score_time(t), e, w, g))
                                })
                                .collect();
                            let mut lead = vec![
                                Cell::Text(time_label.clone()),
                                Cell::Text(shown.variable(&cv.variable)),
                            ];
                            if let (Some(sv), Some(b)) = (second, b) {
                                lead.push(Cell::Text(format!(
                                    "{} = {}",
                                    shown.variable(&sv.variable),
                                    value_text(sv, b)
                                )));
                            }
                            let result = |r: Option<(f64, usize)>| -> [Cell; 3] {
                                match r {
                                    Some((d, df)) => [
                                        Cell::Number(d),
                                        Cell::Number(df as f64),
                                        Cell::Number(chi_square_p(d, df as f64)),
                                    ],
                                    None => [Cell::Empty, Cell::Empty, Cell::Empty],
                                }
                            };
                            let mut row = lead.clone();
                            row.extend(result(gehan(&all, groups.len())));
                            overall_rows.push(row);
                            if command.pairwise {
                                for i in 0..groups.len() {
                                    for j in i + 1..groups.len() {
                                        let pair: Vec<(f64, bool, f64, usize)> = all
                                            .iter()
                                            .filter(|c| c.3 == i || c.3 == j)
                                            .map(|&(t, e, w, g)| (t, e, w, usize::from(g == j)))
                                            .collect();
                                        let mut row = lead.clone();
                                        for k in [i, j] {
                                            let text = groups[k]
                                                .0
                                                .map_or(String::new(), |a| value_text(cv, a));
                                            row.push(Cell::Text(text));
                                        }
                                        row.extend(result(gehan(&pair, 2)));
                                        pair_rows.push(row);
                                    }
                                }
                            }
                        }
                        // the plots of this group of tables
                        if let Some(choice) = &plot_choice
                            && let Some((kinds, _)) = &command.plots
                            && is_chosen(&choice[0], time_name)
                            && first_name.as_ref().is_none_or(|f| is_chosen(&choice[1], f))
                            && second_name
                                .as_ref()
                                .is_none_or(|s| is_chosen(&choice[2], s))
                        {
                            for &kind in kinds {
                                let groups: Vec<PointGroup> = lines_of
                                    .iter()
                                    .map(|(a, lines)| PointGroup {
                                        name: match (first, a) {
                                            (Some(cv), Some(a)) => value_text(cv, *a),
                                            _ => time_label.clone(),
                                        },
                                        points: plot_points(lines, kind),
                                    })
                                    .filter(|g| !g.points.is_empty())
                                    .collect();
                                if groups.is_empty() {
                                    continue;
                                }
                                let (title, axis) = match kind {
                                    LifePlot::Survival => {
                                        ("Survival Function", "Cumulative Survival")
                                    }
                                    LifePlot::LogSurvival => {
                                        ("Log Survival Function", "Log Cumulative Survival")
                                    }
                                    LifePlot::Hazard => ("Hazard Function", "Hazard Rate"),
                                    LifePlot::Density => {
                                        ("Density Function", "Probability Density")
                                    }
                                    LifePlot::OneMinusSurvival => (
                                        "One Minus Survival Function",
                                        "One Minus Cumulative Survival",
                                    ),
                                };
                                let mut chart = Chart::plot(
                                    ChartKind::Line,
                                    format!("{title} of {time_label}"),
                                    Axis::titled(time_label.clone()),
                                    Axis::titled(axis),
                                    Plot::Points {
                                        groups,
                                        lines: Vec::new(),
                                        z_axis: None,
                                    },
                                );
                                if let (Some(cv), Some(b)) = (second, b) {
                                    chart.subtitle = Some(format!(
                                        "{} = {}",
                                        shown.variable(&cv.variable),
                                        value_text(cv, b)
                                    ));
                                }
                                if let Some(cv) = first {
                                    chart.legend = Some(shown.variable(&cv.variable));
                                }
                                charts.push(chart);
                            }
                        }
                    }
                }
            }
            if command.table && !command.compare_only && !table_rows.is_empty() {
                let lead = usize::from(!first_names.is_empty()) + usize::from(has_second);
                let mut columns: Vec<String> = Vec::new();
                if has_second {
                    columns.push("Second Order Control".into());
                }
                if !first_names.is_empty() {
                    columns.push("First Order Control".into());
                }
                columns.extend(
                    [
                        "Interval Start Time",
                        "Number Entering Interval",
                        "Number Withdrawing during Interval",
                        "Number Exposed to Risk",
                        "Number of Terminal Events",
                        "Proportion Terminating",
                        "Proportion Surviving",
                        "Cumulative Proportion Surviving at End of Interval",
                        "Std. Error of Cumulative Proportion Surviving at End of Interval",
                        "Probability Density",
                        "Std. Error of Probability Density",
                        "Hazard Rate",
                        "Std. Error of Hazard Rate",
                    ]
                    .map(String::from),
                );
                tables.push(Table {
                    title: format!("Life Table of {time_label}"),
                    row_headers: lead + 1,
                    columns,
                    rows: table_rows,
                    caption: Some(
                        "The last interval has no end, so it has no density or hazard.".into(),
                    ),
                    ..Table::default()
                });
            }
        }
        if !median_rows.is_empty() {
            tables.push(Table {
                title: "Median Survival Time".into(),
                row_headers: 3,
                columns: [
                    "Survival Variable",
                    "First Order Control",
                    "Second Order Control",
                    "Median Time",
                ]
                .map(String::from)
                .into(),
                rows: median_rows
                    .into_iter()
                    .map(|mut r| {
                        // labels come second order first; show the first order one first
                        if has_second {
                            r.swap(1, 2);
                        }
                        r
                    })
                    .collect(),
                caption: Some(
                    "A plus sign: survival stays at 0.5 or above up to the start of the last, \
                     open interval."
                        .into(),
                ),
                ..Table::default()
            });
        }
        let mut lead_columns: Vec<String> = ["Survival Variable", "Control Variable"]
            .map(String::from)
            .into();
        if has_second {
            lead_columns.push("Second Order Control".into());
        }
        let method = match command.approximate {
            true => "Approximate comparison: each time is scored by the start of its interval.",
            false => "Exact comparison.",
        };
        if !overall_rows.is_empty() {
            let mut columns = lead_columns.clone();
            columns.extend(["Wilcoxon (Gehan) Statistic", "df", "Sig."].map(String::from));
            tables.push(Table {
                title: "Overall Comparisons".into(),
                row_headers: lead_columns.len(),
                columns,
                rows: overall_rows,
                caption: Some(format!(
                    "Tests whether the groups of the control share one survival distribution. \
                     {method}"
                )),
                ..Table::default()
            });
        }
        if !pair_rows.is_empty() {
            let mut columns = lead_columns.clone();
            columns.extend(
                [
                    "First Group",
                    "Second Group",
                    "Wilcoxon (Gehan) Statistic",
                    "df",
                    "Sig.",
                ]
                .map(String::from),
            );
            tables.push(Table {
                title: "Pairwise Comparisons".into(),
                row_headers: lead_columns.len() + 2,
                columns,
                rows: pair_rows,
                caption: Some(method.into()),
                ..Table::default()
            });
        }
        for w in warnings {
            self.warn(w);
        }
        self.charts.extend(charts);
        Ok(tables)
    }
}

/// The points of one group's function: survival, its log and one minus it at the ends of the
/// closed intervals joined by straight lines; hazard and density as a level across each
/// closed interval.
fn plot_points(lines: &[Line], kind: LifePlot) -> Vec<Point> {
    let point = |x: f64, y: f64| Point {
        x,
        y,
        z: None,
        label: None,
    };
    let mut out = Vec::new();
    match kind {
        LifePlot::Survival | LifePlot::LogSurvival | LifePlot::OneMinusSurvival => {
            let f = |p: f64| match kind {
                LifePlot::Survival => p,
                LifePlot::LogSurvival => p.ln(),
                _ => 1.0 - p,
            };
            out.push(point(0.0, f(1.0)));
            for line in lines {
                if let Some(h) = line.width {
                    out.push(point(line.start + h, f(line.cumulative)));
                }
            }
        }
        LifePlot::Hazard | LifePlot::Density => {
            for line in lines {
                let value = match kind {
                    LifePlot::Hazard => line.hazard,
                    _ => line.density,
                };
                if let (Some(h), Some(y)) = (line.width, value) {
                    out.push(point(line.start, y));
                    out.push(point(line.start + h, y));
                }
            }
        }
    }
    out.retain(|p| p.y.is_finite());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periods_stretch_to_whole_intervals() {
        let (starts, stretched) = interval_starts(&[(20.0, 6.0), (30.0, 4.0)]).unwrap();
        assert_eq!(starts, [0.0, 6.0, 12.0, 18.0, 24.0, 28.0, 32.0]);
        assert!(stretched);
        let (starts, stretched) = interval_starts(&[(9.0, 3.0)]).unwrap();
        assert_eq!(starts, [0.0, 3.0, 6.0, 9.0]);
        assert!(!stretched);
        // stretching the first period past the end of the second is an error
        assert!(interval_starts(&[(10.0, 7.0), (12.0, 1.0)]).is_err());
    }

    #[test]
    fn gehan_weights_count_as_repeated_cases() {
        let weighted = [
            (1.0, true, 2.0, 0),
            (2.0, false, 1.0, 1),
            (3.0, true, 1.0, 1),
        ];
        let repeated = [
            (1.0, true, 1.0, 0),
            (1.0, true, 1.0, 0),
            (2.0, false, 1.0, 1),
            (3.0, true, 1.0, 1),
        ];
        assert_eq!(gehan(&weighted, 2), gehan(&repeated, 2));
    }
}
