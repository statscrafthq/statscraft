//! MULT RESPONSE: frequency tables and crosstabulations of multiple response groups.
//!
//! Rules, from the description of the command:
//! - A dichotomy group (one value) has a category per variable: a case answers it when the
//!   variable holds the value. A category group (two values) has a category per value from
//!   min to max, counted over all its variables. VARIABLES gives single variables a range.
//! - Values are truncated to whole numbers; a value out of range does not count.
//! - A case is missing for a group that it answers nowhere. MISSING=MDGROUP leaves out of a
//!   dichotomy group a case missing on any of its variables; MRGROUP leaves out of a category
//!   group a case missing or out of range on any of its variables. A single variable is missing
//!   when its value is missing or out of range. INCLUDE makes user-missing values count.
//!   A table uses the cases valid on all its items.
//! - Frequencies: the count of every category (responses), its percent of all responses and
//!   its percent of the valid cases.
//! - Crosstabulations: a cell counts every pair of a row response and a column response of a
//!   case. With BASE=CASES (the default) the totals are cases: the row total of a category is
//!   the weight of its responses among the valid cases, the grand total the valid cases. With
//!   BASE=RESPONSES, and always in PAIRED tables, the totals are the sums of the cells.
//!   PAIRED pairs the n-th variable of every category group of the table; single variables and
//!   dichotomy groups are crossed with the pairs.
//! - Empty categories are left out. Labels: a dichotomy category shows the variable label (else
//!   the name), a category group the value labels of its first variable, a single variable its
//!   value labels; NOLABELS and CONDENSE show values instead, except for dichotomy groups.
//!
//! Choices where the manual is silent:
//! - A value that a case gives twice in a category group counts twice, as the description of
//!   the frequencies (summed over the variables) says.
//! - Fractional weights count as they are; a case with a weight that is missing, 0 or negative
//!   is left out, also from the missing cases.
//! - PAIRED pairs as many variables as the shortest category group of the table has.

use crate::crosstabs::{Key, label};
use crate::descriptives::case_weights;
use crate::format::category_text;
use crate::{Cell, Currencies, Engine, Error, Table};
use statscraft_data::{Dataset, Variable};
use statscraft_syntax::{CrosstabsCell as C, MultResponse, VarItem};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Dichotomy(f64),
    Categories(f64, f64),
    Single(f64, f64),
}

/// A group or a single variable, with what every case answers.
struct Item<'a> {
    name: String,
    label: Option<String>,
    kind: Kind,
    variables: Vec<Variable<'a>>,
    /// Per case: the categories answered (a dichotomy group: the variable's position), None
    /// when the case is missing.
    responses: Vec<Option<Vec<i64>>>,
    /// Per case and variable of a category group: the category, for PAIRED.
    components: Vec<Vec<Option<i64>>>,
}

/// The value of a variable at a case, truncated; None when missing.
fn value(
    variable: &Variable,
    values: &statscraft_data::NumericValues,
    case: usize,
    include: bool,
) -> Option<f64> {
    let v = values.get(case)?;
    (include || !variable.is_user_missing(v)).then_some(v.trunc())
}

impl Item<'_> {
    fn category(&self, category: i64, show_labels: bool, currencies: &Currencies) -> String {
        match self.kind {
            Kind::Dichotomy(_) => {
                let variable = &self.variables[category as usize];
                variable.label().unwrap_or(variable.name()).to_string()
            }
            _ if show_labels => label(
                &self.variables[0],
                &Key::Number(category as f64),
                currencies,
            ),
            _ => category_text(
                category as f64,
                &currencies.resolve(self.variables[0].format()),
            ),
        }
    }

    fn title(&self) -> String {
        self.label.clone().unwrap_or_else(|| self.name.clone())
    }

    fn note(&self) -> Option<String> {
        match self.kind {
            Kind::Dichotomy(v) => Some(format!(
                "{} is a dichotomy group counting the value {v}.",
                self.name
            )),
            _ => None,
        }
    }
}

fn text(s: impl Into<String>) -> Cell {
    Cell::Text(s.into())
}

fn number(n: f64) -> Cell {
    if n.is_finite() {
        Cell::Number(n)
    } else {
        Cell::Empty
    }
}

/// Every combination of one entry per list, the first list outermost.
fn product<T: Clone>(lists: &[Vec<T>]) -> Vec<Vec<T>> {
    let mut out: Vec<Vec<T>> = vec![Vec::new()];
    for list in lists {
        out = out
            .iter()
            .flat_map(|prefix| {
                list.iter().map(move |x| {
                    let mut next = prefix.clone();
                    next.push(x.clone());
                    next
                })
            })
            .collect();
    }
    out
}

/// Valid, missing and total weight as the six cells of a case summary row.
fn summary_cells(valid: f64, missing: f64) -> Vec<Cell> {
    let total = valid + missing;
    let percent = |n: f64| number(100.0 * n / total);
    vec![
        number(valid),
        percent(valid),
        number(missing),
        percent(missing),
        number(total),
        percent(total),
    ]
}

fn case_summary(rows: Vec<Vec<Cell>>) -> Table {
    let mut columns = vec![String::new()];
    let mut column_groups = vec![Vec::new()];
    for group in ["Valid", "Missing", "Total"] {
        for leaf in ["N", "Percent"] {
            columns.push(leaf.into());
            column_groups.push(vec!["Cases".into(), group.into()]);
        }
    }
    Table {
        title: "Case Summary".into(),
        row_headers: 1,
        columns,
        column_groups,
        rows,
        ..Table::default()
    }
}

/// The counts of one layer of a crosstabulation.
#[derive(Default)]
struct Layer {
    cells: BTreeMap<(i64, i64), f64>,
    rows: BTreeMap<i64, f64>,
    columns: BTreeMap<i64, f64>,
    total: f64,
}

impl Layer {
    /// A cell statistic; None for the Total row or column.
    fn cell(&self, statistic: C, row: Option<i64>, column: Option<i64>) -> Cell {
        let n = match (row, column) {
            (Some(r), Some(c)) => self.cells.get(&(r, c)).copied().unwrap_or(0.0),
            (Some(r), None) => self.rows[&r],
            (None, Some(c)) => self.columns[&c],
            (None, None) => self.total,
        };
        let row_total = row.map_or(self.total, |r| self.rows[&r]);
        let column_total = column.map_or(self.total, |c| self.columns[&c]);
        number(match statistic {
            C::Row => 100.0 * n / row_total,
            C::Column => 100.0 * n / column_total,
            C::Total => 100.0 * n / self.total,
            _ => n,
        })
    }
}

fn cell_name(statistic: C) -> &'static str {
    match statistic {
        C::Row => "Row %",
        C::Column => "Column %",
        C::Total => "Total %",
        _ => "Count",
    }
}

impl Engine {
    pub(crate) fn mult_response(&mut self, command: &MultResponse) -> Result<Vec<Table>, Error> {
        for layout in &command.layout {
            self.warn(format!(
                "FORMAT={layout} only changes the printed layout; Statscraft ignores it"
            ));
        }
        let data = self.active()?;
        let items = self.items(data, command)?;
        let weights = case_weights(data)?;
        let weight = |case: usize| weights.as_ref().map_or(1.0, |w| w[case]);
        let cases: Vec<usize> = (0..data.case_count())
            .filter(|&c| weight(c) > 0.0)
            .collect();
        let show_labels = !command.no_labels;
        let currencies = &self.currencies();
        let mut out = Vec::new();

        let chosen = resolve(&items, &command.frequencies)?;
        if !chosen.is_empty() {
            let mut summary = Vec::new();
            let mut tables = Vec::new();
            for &i in &chosen {
                let item = &items[i];
                let (mut valid, mut missing) = (0.0, 0.0);
                let mut counts: BTreeMap<i64, f64> = BTreeMap::new();
                for &case in &cases {
                    let w = weight(case);
                    match &item.responses[case] {
                        None => missing += w,
                        Some(answers) => {
                            valid += w;
                            for &a in answers {
                                *counts.entry(a).or_default() += w;
                            }
                        }
                    }
                }
                let mut row = vec![text(&item.name)];
                row.extend(summary_cells(valid, missing));
                summary.push(row);
                let responses: f64 = counts.values().sum();
                let labels = show_labels && !command.condense;
                let mut rows: Vec<Vec<Cell>> = counts
                    .iter()
                    .map(|(&category, &n)| {
                        vec![
                            text(item.category(category, labels, currencies)),
                            number(n),
                            number(100.0 * n / responses),
                            number(100.0 * n / valid),
                        ]
                    })
                    .collect();
                rows.push(vec![
                    text("Total"),
                    number(responses),
                    number(100.0),
                    number(100.0 * responses / valid),
                ]);
                tables.push(Table {
                    title: format!("{} Frequencies", item.name),
                    row_headers: 1,
                    columns: vec![
                        item.title(),
                        "N".into(),
                        "Percent".into(),
                        "Percent of Cases".into(),
                    ],
                    column_groups: vec![
                        Vec::new(),
                        vec!["Responses".into()],
                        vec!["Responses".into()],
                        Vec::new(),
                    ],
                    rows,
                    caption: item.note(),
                    ..Table::default()
                });
            }
            out.push(case_summary(summary));
            out.extend(tables);
        }

        for table in &command.tables {
            let dimensions: Vec<Vec<usize>> = table
                .dimensions
                .iter()
                .map(|d| resolve(&items, d))
                .collect::<Result<_, _>>()?;
            let responses = command.base_responses == Some(true) || table.paired;
            let mut summary = Vec::new();
            let mut tables = Vec::new();
            for combination in product(&dimensions) {
                let parts: Vec<&Item> = combination.iter().map(|&i| &items[i]).collect();
                let title = parts
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" by ");
                let (valid, missing, layers) =
                    crosstab(&parts, &cases, &weight, table.paired, responses);
                let mut row = vec![text(&title)];
                row.extend(summary_cells(valid, missing));
                summary.push(row);
                tables.push(crosstab_table(
                    &parts,
                    &title,
                    layers,
                    command,
                    (table.paired, responses, show_labels),
                    currencies,
                ));
            }
            out.push(case_summary(summary));
            out.extend(tables);
        }
        Ok(out)
    }

    /// The groups, then the single variables, in the order of definition.
    fn items<'a>(&self, data: &'a Dataset, command: &MultResponse) -> Result<Vec<Item<'a>>, Error> {
        let numeric = |name: &str| -> Result<Variable<'a>, Error> {
            let variable = data.variable(name)?;
            if variable.is_string() {
                return Err(Error(format!(
                    "{name} is a string variable; MULT RESPONSE takes numeric variables"
                )));
            }
            Ok(variable)
        };
        let range = |min: f64, max: f64, what: &str| match min <= max {
            true => Ok(()),
            false => Err(Error(format!(
                "the range of {what} runs backwards: {min} is above {max}"
            ))),
        };
        let mut items: Vec<Item> = Vec::new();
        let mut used: Vec<String> = Vec::new();
        for group in &command.groups {
            if items
                .iter()
                .any(|i| i.name.eq_ignore_ascii_case(&group.name))
            {
                return Err(Error(format!("the group {} is defined twice", group.name)));
            }
            let names = self.expand(&group.variables)?;
            used.extend(names.iter().cloned());
            let kind = match group.max {
                None => Kind::Dichotomy(group.value),
                Some(max) => {
                    range(group.value, max, &group.name)?;
                    Kind::Categories(group.value, max)
                }
            };
            let variables = names.iter().map(|n| numeric(n)).collect::<Result<_, _>>()?;
            items.push(Item {
                name: group.name.clone(),
                label: group.label.clone(),
                kind,
                variables,
                responses: Vec::new(),
                components: Vec::new(),
            });
        }
        for (list, min, max) in &command.variables {
            for name in self.expand(list)? {
                range(*min, *max, &name)?;
                used.push(name.clone());
                let variable = numeric(&name)?;
                // a variable named again takes the later range
                items.retain(|i| !(matches!(i.kind, Kind::Single(..)) && i.name == name));
                items.push(Item {
                    name,
                    label: variable.label().map(str::to_string),
                    kind: Kind::Single(*min, *max),
                    variables: vec![variable],
                    responses: Vec::new(),
                    components: Vec::new(),
                });
            }
        }
        for group in &command.groups {
            if let Some(name) = used.iter().find(|n| n.eq_ignore_ascii_case(&group.name)) {
                return Err(Error(format!(
                    "the group name {} is the name of the variable {name}, which this command uses",
                    group.name
                )));
            }
        }
        let include = command.include;
        for item in &mut items {
            let values: Vec<_> = item
                .variables
                .iter()
                .map(|v| v.numeric())
                .collect::<Result<_, _>>()?;
            let cases = data.case_count();
            let in_range = |v: f64, min: f64, max: f64| (min..=max).contains(&v);
            for case in 0..cases {
                let raw: Vec<Option<f64>> = item
                    .variables
                    .iter()
                    .zip(&values)
                    .map(|(var, vals)| value(var, vals, case, include))
                    .collect();
                let (answers, listwise_missing, components) = match item.kind {
                    Kind::Single(min, max) => {
                        let v = raw[0].filter(|v| in_range(*v, min, max));
                        (v.map(|v| v as i64).into_iter().collect(), false, Vec::new())
                    }
                    Kind::Dichotomy(counted) => {
                        let answers: Vec<i64> = (0..raw.len())
                            .filter(|&i| raw[i] == Some(counted))
                            .map(|i| i as i64)
                            .collect();
                        let missing = command.mdgroup && raw.iter().any(Option::is_none);
                        (answers, missing, Vec::new())
                    }
                    Kind::Categories(min, max) => {
                        let components: Vec<Option<i64>> = raw
                            .iter()
                            .map(|v| v.filter(|v| in_range(*v, min, max)).map(|v| v as i64))
                            .collect();
                        let missing = command.mrgroup && components.iter().any(Option::is_none);
                        (
                            components.iter().flatten().copied().collect(),
                            missing,
                            components,
                        )
                    }
                };
                let missing = listwise_missing || answers.is_empty();
                item.responses.push((!missing).then_some(answers));
                item.components.push(components);
            }
        }
        Ok(items)
    }
}

/// The positions of the items a FREQUENCIES or TABLES list names; TO runs in the order of
/// definition, within the groups or within the variables.
fn resolve(items: &[Item], list: &[VarItem]) -> Result<Vec<usize>, Error> {
    let find = |name: &str| {
        items
            .iter()
            .position(|i| i.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                Error(format!(
                    "{name} is neither a group of GROUPS nor a variable of VARIABLES"
                ))
            })
    };
    let group = |i: usize| !matches!(items[i].kind, Kind::Single(..));
    let mut out = Vec::new();
    for item in list {
        match item {
            VarItem::Name(name) => out.push(find(name)?),
            VarItem::Range(first, last) => {
                let (a, b) = (find(first)?, find(last)?);
                if group(a) != group(b) {
                    return Err(Error(format!(
                        "{first} TO {last} joins a group and a variable; TO runs within the groups \
                         or within the variables"
                    )));
                }
                if a > b {
                    return Err(Error(format!(
                        "in {first} TO {last} the first must come before the second"
                    )));
                }
                out.extend(a..=b);
            }
            VarItem::All => {
                return Err(Error("name the groups and variables instead of ALL".into()));
            }
        }
    }
    Ok(out)
}

/// Valid and missing weight and the counts of every layer, by layer categories.
fn crosstab(
    parts: &[&Item],
    cases: &[usize],
    weight: &dyn Fn(usize) -> f64,
    paired: bool,
    responses: bool,
) -> (f64, f64, BTreeMap<Vec<i64>, Layer>) {
    let (mut valid, mut missing) = (0.0, 0.0);
    let mut layers: BTreeMap<Vec<i64>, Layer> = BTreeMap::new();
    let pairs_of: Vec<usize> = (0..parts.len())
        .filter(|&d| paired && matches!(parts[d].kind, Kind::Categories(..)))
        .collect();
    for &case in cases {
        let w = weight(case);
        let answers: Option<Vec<&Vec<i64>>> =
            parts.iter().map(|p| p.responses[case].as_ref()).collect();
        let Some(answers) = answers else {
            missing += w;
            continue;
        };
        valid += w;
        let tuples: Vec<Vec<i64>> = if pairs_of.is_empty() {
            let lists: Vec<Vec<i64>> = answers.iter().map(|a| a.to_vec()).collect();
            product(&lists)
        } else {
            // the n-th variables of the category groups as one response, crossed with the rest
            let n = pairs_of
                .iter()
                .map(|&d| parts[d].components[case].len())
                .min()
                .unwrap_or(0);
            let mut tuples = Vec::new();
            for k in 0..n {
                let paired_values: Option<Vec<i64>> = pairs_of
                    .iter()
                    .map(|&d| parts[d].components[case][k])
                    .collect();
                let Some(paired_values) = paired_values else {
                    continue;
                };
                let lists: Vec<Vec<i64>> = (0..parts.len())
                    .map(|d| match pairs_of.iter().position(|&p| p == d) {
                        Some(j) => vec![paired_values[j]],
                        None => answers[d].to_vec(),
                    })
                    .collect();
                tuples.extend(product(&lists));
            }
            tuples
        };
        for tuple in &tuples {
            let layer = layers.entry(tuple[2..].to_vec()).or_default();
            *layer.cells.entry((tuple[0], tuple[1])).or_default() += w;
        }
        if !responses {
            let layer_lists: Vec<Vec<i64>> = answers[2..].iter().map(|a| a.to_vec()).collect();
            for key in product(&layer_lists) {
                let layer = layers.entry(key).or_default();
                layer.total += w;
                for &r in answers[0] {
                    *layer.rows.entry(r).or_default() += w;
                }
                for &c in answers[1] {
                    *layer.columns.entry(c).or_default() += w;
                }
            }
        }
    }
    if responses {
        for layer in layers.values_mut() {
            for (&(r, c), &n) in &layer.cells {
                *layer.rows.entry(r).or_default() += n;
                *layer.columns.entry(c).or_default() += n;
                layer.total += n;
            }
        }
    }
    // a layer, row or column without a count is empty and left out
    layers.retain(|_, layer| layer.cells.values().any(|&n| n > 0.0));
    for layer in layers.values_mut() {
        let cells = &layer.cells;
        layer
            .rows
            .retain(|r, _| cells.iter().any(|(&(i, _), &n)| i == *r && n > 0.0));
        layer
            .columns
            .retain(|c, _| cells.iter().any(|(&(_, j), &n)| j == *c && n > 0.0));
    }
    (valid, missing, layers)
}

fn crosstab_table(
    parts: &[&Item],
    title: &str,
    layers: BTreeMap<Vec<i64>, Layer>,
    command: &MultResponse,
    (paired, responses, show_labels): (bool, bool, bool),
    currencies: &Currencies,
) -> Table {
    let (row_item, column_item, layer_items) = (parts[0], parts[1], &parts[2..]);
    let mut union: Vec<i64> = layers
        .values()
        .flat_map(|l| l.columns.keys().copied())
        .collect();
    union.sort_unstable();
    union.dedup();
    let mut columns: Vec<String> = layer_items.iter().map(|i| i.title()).collect();
    columns.extend([row_item.title(), "Statistic".into()]);
    let headers = columns.len();
    columns.extend(
        union
            .iter()
            .map(|&c| column_item.category(c, show_labels, currencies)),
    );
    columns.push("Total".into());
    let mut column_groups = vec![Vec::new(); columns.len()];
    for group in &mut column_groups[headers..columns.len() - 1] {
        group.push(column_item.title());
    }
    let mut rows = Vec::new();
    for (key, layer) in &layers {
        let layer_cells: Vec<Cell> = layer_items
            .iter()
            .zip(key)
            .map(|(item, &k)| text(item.category(k, show_labels, currencies)))
            .collect();
        let row_keys = layer.rows.keys().map(|&r| Some(r)).chain([None]);
        for r in row_keys {
            for &statistic in &command.cells {
                let mut row = layer_cells.clone();
                row.push(text(match r {
                    Some(r) => row_item.category(r, show_labels, currencies),
                    None => "Total".into(),
                }));
                row.push(text(cell_name(statistic)));
                row.extend(union.iter().map(|&c| match layer.columns.contains_key(&c) {
                    true => layer.cell(statistic, r, Some(c)),
                    false => Cell::Empty,
                }));
                row.push(layer.cell(statistic, r, None));
                rows.push(row);
            }
        }
    }
    let mut notes = vec![match responses {
        true => "Percentages and totals are based on responses.".to_string(),
        false => "Percentages and totals are based on cases.".to_string(),
    }];
    if paired {
        notes.push("Paired group: the n-th variables of the category groups are paired.".into());
    }
    notes.extend(parts.iter().filter_map(|p| p.note()));
    notes.dedup();
    Table {
        title: title.to_string(),
        row_headers: headers,
        columns,
        column_groups,
        rows,
        caption: Some(notes.join(" ")),
        ..Table::default()
    }
}
