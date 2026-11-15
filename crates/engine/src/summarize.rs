//! SUMMARIZE: the MEANS cell statistics of several dependents side by side within the groups
//! of control variables, optionally with the cases of each group listed.
//!
//! Rules from the Command Syntax Reference (SUMMARIZE); the statistics are those of MEANS:
//! - Each table list gives a Case Processing Summary, one row per dependent and combination of
//!   controls, and a Case Summaries table per combination of controls (one from each BY).
//!   Groups nest as in MEANS (first control outermost), each level closed by a Total; every
//!   cell statistic is a row of its group and every dependent a column.
//! - MISSING=VARIABLE (the default) drops a case from the statistics of each dependent it is
//!   missing on and lists it while one dependent is valid; TABLE drops it from the whole table
//!   when it is missing on any dependent. A case missing on a control is always dropped.
//!   INCLUDE makes user-missing values valid, DEPENDENT only those of the controls.
//! - FORMAT=LIST lists the cases of each innermost group before its total rows, VALIDLIST only
//!   the cases valid on every dependent. CASENUM numbers them by their place in the file.
//!   NOTOTAL keeps the total rows and leaves their label empty. MISSING='text' shows a
//!   system-missing value of the listing as that text (else a period).
//! - ANOVA and LINEARITY run as in MEANS, for each control of the first BY.
//!
//! Choices where the manual is silent:
//! - LIMIT=n restricts the whole procedure, statistics included, to the first n cases (of the
//!   split group, after the filter), and says so in a caption.
//! - VALIDLIST changes the listing only; the statistics follow MISSING.
//! - TABLE drops a case missing on any dependent or on a control of that table; the analysis
//!   of variance under TABLE uses the cases valid on every dependent.
//! - A variable named twice in one place counts once; a control after two BY is an error.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, summarize as summary_of};
use crate::format::{category_text, procedure_format};
use crate::means::{
    Group, anova_tables, cell_name, combinations, groups, is_valid, kind, members, statistic,
    string_statistic,
};
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Values, Variable};
use statscraft_syntax::{Summarize, SummarizeMissing};

/// What every Case Summaries table of one command shares.
struct Layout<'a> {
    command: &'a Summarize,
    shown: &'a Shown,
    weight: &'a dyn Fn(usize) -> f64,
    include_control: bool,
    case_numbers: Option<&'a [usize]>,
    title: String,
    caption: Option<String>,
}

/// A dependent with what its statistics need: per case whether it counts, and its numbers.
struct Dependent<'a> {
    variable: Variable<'a>,
    valid: Vec<bool>,
    /// The value of each case (NaN where missing), empty for a string.
    values: Vec<f64>,
}

impl Engine {
    pub(crate) fn summarize(&mut self, command: &Summarize) -> Result<Vec<Table>, Error> {
        for name in &command.repeated {
            self.warn(format!(
                "SUMMARIZE: {name} is given more than once; the last one counts"
            ));
        }
        let data = self.active()?;
        let (shown, outline) = (self.shown(), self.outline());
        let weights = case_weights(data)?;
        let weight = |case: usize| weights.as_ref().map_or(1.0, |w| w[case]);
        let limit = command.list.and_then(|l| l.limit);
        let limited = limit.filter(|&n| n < data.case_count());
        let counted: Vec<usize> = (0..limited.unwrap_or(data.case_count()))
            .filter(|&c| weight(c) > 0.0)
            .collect();
        let total: f64 = counted.iter().map(|&c| weight(c)).sum();
        let (include_dependent, include_control) = match command.missing {
            SummarizeMissing::Include => (true, true),
            SummarizeMissing::Dependent => (false, true),
            SummarizeMissing::Exclude => (false, false),
        };
        let note = limited.map(|n| format!("Limited to the first {n} cases."));
        let caption = match (&command.footnote, &note) {
            (Some(footnote), Some(note)) => Some(format!("{footnote}\n{note}")),
            (footnote, note) => footnote.clone().or(note.clone()),
        };
        let layout = Layout {
            command,
            shown: &shown,
            weight: &weight,
            include_control,
            case_numbers: self.case_numbers.as_deref(),
            title: command
                .title
                .clone()
                .unwrap_or_else(|| "Case Summaries".into()),
            caption,
        };
        let lookup = |names: &[String]| -> Result<Vec<Variable>, Error> {
            Ok(names
                .iter()
                .map(|n| data.variable(n))
                .collect::<Result<_, _>>()?)
        };
        let mut out = Vec::new();
        let mut warnings = Vec::new();
        for list in &command.tables {
            let mut dimensions: Vec<Vec<String>> = Vec::new();
            for items in list {
                let mut names = self.expand(items)?;
                let mut seen: Vec<String> = Vec::new();
                names.retain(|n| {
                    let new = !seen.contains(n);
                    seen.push(n.clone());
                    new
                });
                dimensions.push(names);
            }
            let controls_of_list: Vec<&String> = dimensions[1..].iter().flatten().collect();
            for (i, name) in controls_of_list.iter().enumerate() {
                if controls_of_list[..i].contains(name) {
                    return Err(Error(format!(
                        "SUMMARIZE: {name} follows two BY keywords of one table list"
                    )));
                }
            }
            let combos = combinations(&dimensions[1..]);
            for name in &command.within {
                let control = |v: &String| v.eq_ignore_ascii_case(name);
                if !controls_of_list.iter().any(|v| control(v)) {
                    return Err(Error(format!(
                        "NPCT({name}) and SPCT({name}) need {name} as a control variable of every table list"
                    )));
                }
                if combos.iter().any(|combo| !combo.iter().any(control)) {
                    warnings.push(format!(
                        "the percentages within {name} are empty in the tables without {name}"
                    ));
                }
            }
            let dependents: Vec<Dependent> = lookup(&dimensions[0])?
                .into_iter()
                .map(|variable| Dependent {
                    valid: (0..data.case_count())
                        .map(|c| is_valid(&variable, c, include_dependent))
                        .collect(),
                    values: match variable.values() {
                        Values::Numeric(values) => (0..data.case_count())
                            .map(|c| values.get(c).unwrap_or(f64::NAN))
                            .collect(),
                        Values::String(_) => Vec::new(),
                    },
                    variable,
                })
                .collect();
            let all_valid = |case: usize| dependents.iter().all(|d| d.valid[case]);
            let mut summary_rows = Vec::new();
            let mut reports = Vec::new();
            for combo in &combos {
                let controls = lookup(combo)?;
                let in_table: Vec<usize> = (counted.iter().copied())
                    .filter(|&c| controls.iter().all(|v| is_valid(v, c, include_control)))
                    .filter(|&c| match command.tablewise {
                        true => all_valid(c),
                        false => dependents.iter().any(|d| d.valid[c]),
                    })
                    .collect();
                for dependent in &dependents {
                    let n: f64 = (in_table.iter())
                        .filter(|&&c| dependent.valid[c])
                        .map(|&c| weight(c))
                        .sum();
                    let mut name = vec![shown.variable(&dependent.variable)];
                    name.extend(combo.iter().map(|c| shown.named(data, c)));
                    let mut row = vec![Cell::Text(name.join(" * "))];
                    for value in [n, total - n, total] {
                        row.push(Cell::Number(value));
                        row.push(crate::descriptives::number(Some(100.0 * value / total)));
                    }
                    summary_rows.push(row);
                }
                reports.push(case_summaries(&layout, &dependents, &controls, &in_table));
            }
            let mut columns = vec![String::new()];
            let mut column_groups = vec![Vec::new()];
            for group in ["Included", "Excluded", "Total"] {
                for leaf in ["N", "Percent"] {
                    columns.push(leaf.into());
                    column_groups.push(vec!["Cases".to_string(), group.to_string()]);
                }
            }
            out.push(Table {
                title: "Case Processing Summary".into(),
                row_headers: 1,
                columns,
                column_groups,
                rows: summary_rows,
                caption: note.clone(),
                ..Table::default()
            });
            out.extend(reports);
            if command.anova {
                let firsts = dimensions.get(1).cloned().unwrap_or_default();
                for dependent in &dependents {
                    if firsts.is_empty() {
                        out.extend(anova_tables(
                            command.linearity,
                            &dependent.variable,
                            None,
                            &[],
                            &[],
                            &weight,
                            (&shown, &outline),
                            &mut warnings,
                        ));
                    }
                    for control in lookup(&firsts)? {
                        let included: Vec<usize> = (counted.iter().copied())
                            .filter(|&c| dependent.valid[c])
                            .filter(|&c| is_valid(&control, c, include_control))
                            .filter(|&c| !command.tablewise || all_valid(c))
                            .collect();
                        let keys = categories(&control, include_control);
                        out.extend(anova_tables(
                            command.linearity,
                            &dependent.variable,
                            Some((&control, &keys)),
                            &included,
                            &dependent.values,
                            &weight,
                            (&shown, &outline),
                            &mut warnings,
                        ));
                    }
                }
            }
        }
        for warning in warnings {
            self.warn(warning);
        }
        Ok(out)
    }
}

/// One Case Summaries table: the dependents by one combination of controls.
fn case_summaries(
    layout: &Layout,
    dependents: &[Dependent],
    controls: &[Variable],
    in_table: &[usize],
) -> Table {
    let (command, shown, weight) = (layout.command, layout.shown, layout.weight);
    let keys: Vec<Vec<Option<Key>>> = controls
        .iter()
        .map(|v| categories(v, layout.include_control))
        .collect();
    let listing = command.list;
    let total_label = if command.total_label { "Total" } else { "" };
    let totals: Vec<(f64, f64)> = (dependents.iter())
        .map(|d| sums(d, weight, in_table.iter().copied()))
        .collect();
    let positions: Vec<Option<usize>> = (command.within.iter())
        .map(|name| (controls.iter()).position(|v| v.name().eq_ignore_ascii_case(name)))
        .collect();
    let formats_of_dependents: Vec<String> = (dependents.iter())
        .map(|d| shown.currencies.resolve(d.variable.format()))
        .collect();
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let mut formats: Vec<Vec<String>> = Vec::new();
    let heads = controls.len() + usize::from(listing.is_some()) + 1;
    let all_groups = groups(&keys, in_table);
    let mut all_members = members(&all_groups, &keys, in_table);
    for (group, cases) in all_groups.iter().zip(all_members.iter_mut()) {
        if cases.is_empty() {
            continue;
        }
        let cases = std::mem::take(cases);
        let labels: Vec<Cell> = group
            .iter()
            .zip(controls)
            .map(|(choice, v)| {
                Cell::Text(
                    choice
                        .as_ref()
                        .map_or(total_label.into(), |k| shown.value(v, k)),
                )
            })
            .collect();
        let innermost = group.iter().all(Option::is_some);
        if let Some(list) = listing.filter(|_| innermost) {
            for &case in &cases {
                if list.valid_only && !dependents.iter().all(|d| d.valid[case]) {
                    continue;
                }
                let mut row = labels.clone();
                let number = layout.case_numbers.map_or(case, |n| n[case]) + 1;
                row.push(match command.case_numbers {
                    true => Cell::Text(number.to_string()),
                    false => Cell::Empty,
                });
                row.push(Cell::Empty);
                let mut row_formats = vec![String::new(); heads];
                for (d, format) in dependents.iter().zip(&formats_of_dependents) {
                    let (cell, cell_format) = listed(layout, d, format, case);
                    row.push(cell);
                    row_formats.push(cell_format);
                }
                rows.push(row);
                formats.push(row_formats);
            }
        }
        for &cell in &command.cells {
            let mut row = labels.clone();
            if listing.is_some() {
                row.push(match innermost {
                    true => Cell::Text(total_label.into()),
                    false => Cell::Empty,
                });
            }
            row.push(Cell::Text(cell_name(cell, &command.within)));
            let mut row_formats = vec![String::new(); heads];
            for (d, total) in dependents.iter().zip(&totals) {
                let own: Vec<(f64, f64, usize)> = (cases.iter())
                    .filter(|&&c| d.valid[c])
                    .map(|&c| (d.values.get(c).copied().unwrap_or(0.0), weight(c), c))
                    .collect();
                let within = within_sums(&positions, group, &keys, in_table, d, weight);
                row.push(if d.variable.is_string() {
                    string_statistic(cell, &d.variable, &own, total.0, &within)
                } else {
                    let pairs: Vec<(f64, f64)> = own.iter().map(|&(x, w, _)| (x, w)).collect();
                    statistic(cell, &own, (&pairs, &summary_of(&pairs)), *total, &within)
                });
                row_formats.push(kind(cell).map_or_else(String::new, |kind| {
                    procedure_format(d.variable.format(), kind)
                }));
            }
            rows.push(row);
            formats.push(row_formats);
        }
    }
    let mut columns: Vec<String> = controls.iter().map(|v| shown.variable(v)).collect();
    columns.resize(heads, String::new());
    columns.extend(dependents.iter().map(|d| shown.variable(&d.variable)));
    if formats.iter().flatten().all(String::is_empty) {
        formats.clear();
    }
    Table {
        title: layout.title.clone(),
        row_headers: heads,
        column_groups: vec![Vec::new(); columns.len()],
        columns,
        rows,
        formats,
        caption: layout.caption.clone(),
        ..Table::default()
    }
}

/// The weighted count and sum of a dependent's valid values among `cases`.
fn sums(
    d: &Dependent,
    weight: &dyn Fn(usize) -> f64,
    cases: impl Iterator<Item = usize>,
) -> (f64, f64) {
    cases.filter(|&c| d.valid[c]).fold((0.0, 0.0), |(w, s), c| {
        let x = d.values.get(c).copied().unwrap_or(0.0);
        (w + weight(c), s + x * weight(c))
    })
}

/// The count and sum of the base of each NPCT(var) and SPCT(var): the cases in the group's
/// category of that control (all cases on its Total rows); None where the table lacks it.
fn within_sums(
    positions: &[Option<usize>],
    group: &Group,
    keys: &[Vec<Option<Key>>],
    in_table: &[usize],
    dependent: &Dependent,
    weight: &dyn Fn(usize) -> f64,
) -> Vec<Option<(f64, f64)>> {
    positions
        .iter()
        .map(|position| {
            position.map(|p| {
                let base = (in_table.iter().copied())
                    .filter(|&c| group[p].is_none() || keys[p][c] == group[p]);
                sums(dependent, weight, base)
            })
        })
        .collect()
}

/// A dependent's value at `case` in the listing, with its display format: the value label
/// where SET TNUMBERS shows one, a period or FORMAT MISSING for system-missing.
fn listed(layout: &Layout, d: &Dependent, format: &str, case: usize) -> (Cell, String) {
    match d.variable.values() {
        Values::String(values) => (
            Cell::Text(values.get(case).trim_end().to_string()),
            String::new(),
        ),
        Values::Numeric(values) => match values.get(case) {
            None => {
                let text = layout.command.missing_text.as_deref().unwrap_or(".");
                (Cell::Text(text.to_string()), String::new())
            }
            Some(x) => {
                let shown = layout.shown.number(&d.variable, x);
                if shown == category_text(x, format) {
                    (Cell::Number(x), format.to_string())
                } else {
                    (Cell::Text(shown), String::new())
                }
            }
        },
    }
}
