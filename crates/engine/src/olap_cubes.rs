//! OLAP CUBES: the MEANS cell statistics of summary variables within the nested groups of
//! grouping variables, with differences between two categories or two summary variables.
//!
//! Rules from the Command Syntax Reference (OLAP CUBES); the statistics are those of MEANS:
//! - Each combination of grouping variables (one from each BY) is one group of the report.
//!   Categories are sorted; each level ends in a Total. A variable named twice in one place
//!   counts once; the same variable after two BY is an error. Summary variables are numeric.
//! - DEFAULT cells: sum, count, mean, standard deviation, percent of total sum and of total N.
//! - GAC: in every group, each statistic of the second category minus that of the first; GPC:
//!   that change divided by the first. VAC and VPC: the same between two summary variables.
//!   Labels go to the differences in order; DEFAULT and missing labels take a generated name.
//! - HIDESMALLCOUNTS: counts below n (weighted when weighting is on) show as "<n".
//!
//! Choices where the manual is silent:
//! - A case counts for a summary variable when it is valid on it and on the grouping variables
//!   of the table (user-missing values are missing), as MEANS without /MISSING.
//! - The layers become rows: one table per combination with the grouping categories nested
//!   in the rows, a row per summary variable inside each group, statistics in columns.
//! - The rows of a category difference come after the Total of its grouping variable, and
//!   show only where both categories have cases. GPC is shown as a percentage, 100 * (second -
//!   first) / first, and is empty where the first is 0.
//! - Under HIDESMALLCOUNTS a difference of counts is empty when either count is hidden.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number, summarize};
use crate::format::procedure_format;
use crate::means::{Group, cell_name, combinations, distinct, is_valid, kind, members, statistic};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Values, Variable};
use statscraft_syntax::{MeansCell as M, OlapCubes, OlapPair, Value};

/// A choice of one grouping variable in a row.
#[derive(Clone)]
enum Choice {
    Category(Key),
    Total,
    /// A difference of /CREATE between two categories, an index into `differences`.
    Difference(usize),
}

/// The value as a key of `variable`, None when its type does not fit.
fn key_of(value: &Value, variable: &Variable) -> Option<Key> {
    match (value, variable.is_string()) {
        (Value::Number(n), false) => Some(Key::Number(*n)),
        (Value::String(s), true) => Some(Key::Text(s.trim_end().to_string())),
        _ => None,
    }
}

fn same(a: &Key, b: &Key) -> bool {
    match (a, b) {
        (Key::Text(a), Key::Text(b)) => a.trim_end() == b.trim_end(),
        _ => a == b,
    }
}

impl Engine {
    pub(crate) fn olap_cubes(&mut self, command: &OlapCubes) -> Result<Vec<Table>, Error> {
        for name in &command.repeated {
            self.warn(format!(
                "OLAP CUBES: {name} is given more than once; the last one counts"
            ));
        }
        let data = self.active()?;
        let shown = self.shown();
        let dedup = |mut names: Vec<String>| {
            let mut seen: Vec<String> = Vec::new();
            names.retain(|n| {
                let new = !seen.iter().any(|s| s.eq_ignore_ascii_case(n));
                seen.push(n.clone());
                new
            });
            names
        };
        let summary_names = dedup(self.expand(&command.summary)?);
        let dimensions: Vec<Vec<String>> = (command.groups.iter())
            .map(|items| self.expand(items).map(dedup))
            .collect::<Result<_, _>>()?;
        let all_groups: Vec<&String> = dimensions.iter().flatten().collect();
        for (i, name) in all_groups.iter().enumerate() {
            if all_groups[..i].iter().any(|n| n.eq_ignore_ascii_case(name)) {
                return Err(Error(format!("OLAP CUBES: {name} follows two BY keywords")));
            }
        }
        let is_group = |name: &str| all_groups.iter().any(|g| g.eq_ignore_ascii_case(name));
        let summaries: Vec<Variable> = (summary_names.iter())
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = summaries.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "OLAP CUBES: the summary variable {} is a string; summary variables must be numeric",
                v.name()
            )));
        }
        let combos = combinations(&dimensions);
        let mut warnings = Vec::new();
        for name in &command.within {
            if !is_group(name) {
                return Err(Error(format!(
                    "NPCT({name}) and SPCT({name}) need {name} as a grouping variable"
                )));
            }
            if combos
                .iter()
                .any(|c| !c.iter().any(|v| v.eq_ignore_ascii_case(name)))
            {
                warnings.push(format!(
                    "the percentages within {name} are empty in the tables without {name}"
                ));
            }
        }
        // the summary variables of each VAC and VPC pair, as positions
        let position = |name: &str| {
            (summary_names.iter())
                .position(|n| n.eq_ignore_ascii_case(name))
                .ok_or_else(|| {
                    Error(format!(
                        "CREATE: {name} is not a summary variable of OLAP CUBES"
                    ))
                })
        };
        let mut variable_pairs = Vec::new();
        for difference in &command.differences {
            match &difference.pair {
                OlapPair::Variables(a, b) => variable_pairs.push((position(a)?, position(b)?)),
                OlapPair::Categories(..) => {}
            }
        }
        let create_variable = match &command.create_variable {
            Some(name) if !is_group(name) => {
                return Err(Error(format!(
                    "CREATE: {name} is not a grouping variable of OLAP CUBES"
                )));
            }
            Some(name) => {
                let variable = data.variable(name)?;
                for difference in &command.differences {
                    if let OlapPair::Categories(a, b) = &difference.pair
                        && (key_of(a, &variable).is_none() || key_of(b, &variable).is_none())
                    {
                        return Err(Error(format!(
                            "CREATE: the values of {name} must be {}",
                            if variable.is_string() {
                                "quoted strings"
                            } else {
                                "numbers"
                            }
                        )));
                    }
                }
                Some(variable)
            }
            None => None,
        };
        let weights = case_weights(data)?;
        let weight = |case: usize| weights.as_ref().map_or(1.0, |w| w[case]);
        let counted: Vec<usize> = (0..data.case_count())
            .filter(|&c| weight(c) > 0.0)
            .collect();
        let total: f64 = counted.iter().map(|&c| weight(c)).sum();
        let values: Vec<Vec<f64>> = (summaries.iter())
            .map(|v| match v.values() {
                Values::Numeric(values) => (0..data.case_count())
                    .map(|c| values.get(c).unwrap_or(f64::NAN))
                    .collect(),
                Values::String(_) => Vec::new(),
            })
            .collect();
        let mut summary_rows = Vec::new();
        let mut reports = Vec::new();
        for combo in &combos {
            let controls: Vec<Variable> = combo
                .iter()
                .map(|n| data.variable(n))
                .collect::<Result<_, _>>()?;
            let keys: Vec<Vec<Option<Key>>> =
                controls.iter().map(|v| categories(v, false)).collect();
            let included: Vec<Vec<usize>> = (summaries.iter())
                .map(|s| {
                    (counted.iter().copied())
                        .filter(|&c| is_valid(s, c, false))
                        .filter(|&c| keys.iter().all(|k| k[c].is_some()))
                        .collect()
                })
                .collect();
            for (s, cases) in summaries.iter().zip(&included) {
                let n: f64 = cases.iter().map(|&c| weight(c)).sum();
                let mut name = vec![shown.variable(s)];
                name.extend(controls.iter().map(|c| shown.variable(c)));
                let mut row = vec![Cell::Text(name.join(" * "))];
                for value in [n, total - n, total] {
                    row.push(Cell::Number(value));
                    row.push(number(Some(100.0 * value / total)));
                }
                summary_rows.push(row);
            }
            let mut union: Vec<usize> = included.iter().flatten().copied().collect();
            union.sort_unstable();
            union.dedup();
            let report = Report {
                command,
                shown: &shown,
                summaries: &summaries,
                values: &values,
                variable_pairs: &variable_pairs,
                weight: &weight,
            };
            let difference_at = create_variable.as_ref().and_then(|v| {
                (controls.iter()).position(|c| c.name().eq_ignore_ascii_case(v.name()))
            });
            reports.push(report.table(
                &controls,
                &keys,
                &included,
                &union,
                difference_at,
                &mut warnings,
            ));
        }
        let mut columns = vec![String::new()];
        let mut column_groups = vec![Vec::new()];
        for group in ["Included", "Excluded", "Total"] {
            for leaf in ["N", "Percent"] {
                columns.push(leaf.into());
                column_groups.push(vec!["Cases".to_string(), group.to_string()]);
            }
        }
        let mut out = vec![Table {
            title: "Case Processing Summary".into(),
            row_headers: 1,
            columns,
            column_groups,
            rows: summary_rows,
            ..Table::default()
        }];
        out.extend(reports);
        let mut said: Vec<String> = Vec::new();
        for warning in warnings {
            if !said.contains(&warning) {
                said.push(warning.clone());
                self.warn(warning);
            }
        }
        Ok(out)
    }
}

/// What every table of one command shares.
struct Report<'a> {
    command: &'a OlapCubes,
    shown: &'a crate::shown::Shown,
    summaries: &'a [Variable<'a>],
    /// The value of every case of each summary variable, NaN where missing.
    values: &'a [Vec<f64>],
    variable_pairs: &'a [(usize, usize)],
    weight: &'a dyn Fn(usize) -> f64,
}

impl Report<'_> {
    /// The statistics of summary variable `s` in `group`, a value per cell; with the group's
    /// weighted count.
    fn statistics(
        &self,
        s: usize,
        group: &Group,
        cases: &[usize],
        keys: &[Vec<Option<Key>>],
        (included, positions): (&[usize], &[Option<usize>]),
    ) -> Vec<Option<f64>> {
        let weight = self.weight;
        let value = |c: usize| self.values[s][c];
        let sums = |cases: &mut dyn Iterator<Item = usize>| {
            cases.fold((0.0, 0.0), |(w, sum), c| {
                (w + weight(c), sum + value(c) * weight(c))
            })
        };
        let totals = sums(&mut included.iter().copied());
        // the count and sum of the cases in the group's category of that grouping variable
        let within: Vec<Option<(f64, f64)>> = (positions.iter())
            .map(|position| {
                position.map(|p| {
                    sums(
                        &mut included
                            .iter()
                            .copied()
                            .filter(|&c| group[p].is_none() || keys[p][c] == group[p]),
                    )
                })
            })
            .collect();
        let own: Vec<(f64, f64, usize)> = cases.iter().map(|&c| (value(c), weight(c), c)).collect();
        let pairs: Vec<(f64, f64)> = own.iter().map(|&(x, w, _)| (x, w)).collect();
        let summary = summarize(&pairs);
        (self.command.cells.iter())
            .map(
                |&cell| match statistic(cell, &own, (&pairs, &summary), totals, &within) {
                    Cell::Number(x) => Some(x),
                    _ => None,
                },
            )
            .collect()
    }

    /// The cells of a row of statistics, with their formats.
    fn plain(&self, s: usize, stats: &[Option<f64>]) -> (Vec<Cell>, Vec<String>) {
        let hide = self.command.hide_small_counts;
        (self.command.cells.iter().zip(stats))
            .map(|(&cell, value)| {
                let format = kind(cell).map_or_else(String::new, |kind| {
                    procedure_format(self.summaries[s].format(), kind)
                });
                match (value, hide) {
                    (Some(v), Some(n)) if cell == M::Count && *v < f64::from(n) => {
                        (Cell::Text(format!("<{n}")), String::new())
                    }
                    (value, _) => (number(*value), format),
                }
            })
            .unzip()
    }

    /// The cells of a difference row: second minus first, as a percentage of the first when
    /// `percent`; `s` gives the format.
    fn difference(
        &self,
        s: usize,
        (first, second): (&[Option<f64>], &[Option<f64>]),
        percent: bool,
    ) -> (Vec<Cell>, Vec<String>) {
        let hidden = |v: &Option<f64>| {
            (self.command.hide_small_counts).is_some_and(|n| v.is_some_and(|v| v < f64::from(n)))
        };
        (self.command.cells.iter().zip(first.iter().zip(second)))
            .map(|(&cell, (a, b))| {
                if cell == M::Count && (hidden(a) || hidden(b)) {
                    return (Cell::Empty, String::new());
                }
                let value = a.zip(*b).and_then(|(a, b)| match percent {
                    true => (a != 0.0).then(|| 100.0 * (b - a) / a),
                    false => Some(b - a),
                });
                let format = match percent {
                    true => "PCT40.1".to_string(),
                    false => kind(cell).map_or_else(String::new, |kind| {
                        procedure_format(self.summaries[s].format(), kind)
                    }),
                };
                (number(value), format)
            })
            .unzip()
    }

    /// The generated name of a difference.
    fn default_label(&self, pair: &OlapPair, variable: Option<&Variable>, percent: bool) -> String {
        let (a, b) = match (pair, variable) {
            (OlapPair::Categories(a, b), Some(v)) => {
                let shown = |value: &Value| {
                    key_of(value, v).map_or_else(String::new, |k| self.shown.value(v, &k))
                };
                (shown(a), shown(b))
            }
            (OlapPair::Variables(a, b), _) => {
                let shown = |name: &str| {
                    (self.summaries.iter())
                        .find(|v| v.name().eq_ignore_ascii_case(name))
                        .map_or_else(|| name.to_string(), |v| self.shown.variable(v))
                };
                (shown(a), shown(b))
            }
            _ => (String::new(), String::new()),
        };
        match percent {
            true => format!("{b} - {a} (%)"),
            false => format!("{b} - {a}"),
        }
    }

    /// The OLAP Cubes table of one combination of grouping variables. `difference_at` is the
    /// position of the CREATE grouping variable among `controls`.
    fn table(
        &self,
        controls: &[Variable],
        keys: &[Vec<Option<Key>>],
        included: &[Vec<usize>],
        union: &[usize],
        difference_at: Option<usize>,
        warnings: &mut Vec<String>,
    ) -> Table {
        let command = self.command;
        let categories_of: Vec<Vec<&Key>> = keys.iter().map(|k| distinct(k, union)).collect();
        // the categories of each category difference, None where one is not among the cases
        let found: Vec<Option<(Key, Key)>> = (command.differences.iter())
            .map(|d| {
                let (Some(p), OlapPair::Categories(a, b)) = (difference_at, &d.pair) else {
                    return None;
                };
                let mut find = |value: &Value| {
                    let key = key_of(value, &controls[p])?;
                    let found = categories_of[p].iter().find(|k| same(k, &key)).copied();
                    if found.is_none() {
                        warnings.push(format!(
                            "CREATE: {} has no cases with the value {}",
                            controls[p].name(),
                            self.shown.value(&controls[p], &key)
                        ));
                    }
                    found.cloned()
                };
                let a = find(a);
                a.zip(find(b))
            })
            .collect();
        let levels: Vec<Vec<Choice>> = (categories_of.iter().enumerate())
            .map(|(l, found_here)| {
                let mut choices: Vec<Choice> = (found_here.iter())
                    .map(|k| Choice::Category((*k).clone()))
                    .chain([Choice::Total])
                    .collect();
                if difference_at == Some(l) {
                    choices.extend((0..command.differences.len()).map(Choice::Difference));
                }
                choices
            })
            .collect();
        let mut rows_of_choices: Vec<Vec<Choice>> = vec![Vec::new()];
        for level in &levels {
            rows_of_choices = (rows_of_choices.iter())
                .flat_map(|prefix| {
                    level.iter().map(move |choice| {
                        let mut next = prefix.clone();
                        next.push(choice.clone());
                        next
                    })
                })
                .collect();
        }
        let plain = |row: &[Choice]| -> Option<Group> {
            (row.iter())
                .map(|choice| match choice {
                    Choice::Category(k) => Some(Some(k.clone())),
                    Choice::Total => Some(None),
                    Choice::Difference(_) => None,
                })
                .collect()
        };
        let groups: Vec<Group> = rows_of_choices.iter().filter_map(|r| plain(r)).collect();
        let positions: Vec<Option<usize>> = (command.within.iter())
            .map(|name| (controls.iter()).position(|v| v.name().eq_ignore_ascii_case(name)))
            .collect();
        // statistics[s][g] and whether group g has cases of any summary variable
        let mut shown_group = vec![false; groups.len()];
        // the groups come from the cases of all summary variables, so members() takes those
        let members_of_all = members(&groups, keys, union);
        let statistics: Vec<Vec<Vec<Option<f64>>>> = (included.iter().enumerate())
            .map(|(s, cases)| {
                let mut counts = vec![false; union.last().map_or(0, |l| l + 1)];
                cases.iter().for_each(|&c| counts[c] = true);
                (groups.iter().zip(&members_of_all).enumerate())
                    .map(|(g, (group, all))| {
                        let own: Vec<usize> = all.iter().copied().filter(|&c| counts[c]).collect();
                        shown_group[g] |= !own.is_empty();
                        self.statistics(s, group, &own, keys, (cases, &positions))
                    })
                    .collect()
            })
            .collect();
        let index = |group: &Group| groups.iter().position(|g| g == group);
        let heads = controls.len() + 1;
        let mut rows = Vec::new();
        let mut formats = Vec::new();
        for choices in &rows_of_choices {
            let labels: Vec<Cell> = (choices.iter().zip(controls))
                .map(|(choice, v)| {
                    Cell::Text(match choice {
                        Choice::Category(k) => self.shown.value(v, k),
                        Choice::Total => "Total".into(),
                        Choice::Difference(d) => {
                            let difference = &command.differences[*d];
                            (difference.label.clone()).unwrap_or_else(|| {
                                self.default_label(&difference.pair, Some(v), difference.percent)
                            })
                        }
                    })
                })
                .collect();
            let mut push = |name: String, (cells, cell_formats): (Vec<Cell>, Vec<String>)| {
                let mut row = labels.clone();
                row.push(Cell::Text(name));
                row.extend(cells);
                rows.push(row);
                let mut row_formats = vec![String::new(); heads];
                row_formats.extend(cell_formats);
                formats.push(row_formats);
            };
            let names = |s: usize| self.shown.variable(&self.summaries[s]);
            if let Some(group) = plain(choices) {
                let g = index(&group).expect("a group of the list");
                if !shown_group[g] {
                    continue;
                }
                for (s, of) in statistics.iter().enumerate() {
                    push(names(s), self.plain(s, &of[g]));
                }
                let variable_differences = (command.differences.iter())
                    .filter(|d| matches!(d.pair, OlapPair::Variables(..)));
                for (d, &(a, b)) in variable_differences.zip(self.variable_pairs) {
                    let name = (d.label.clone())
                        .unwrap_or_else(|| self.default_label(&d.pair, None, d.percent));
                    let sides = (statistics[a][g].as_slice(), statistics[b][g].as_slice());
                    push(name, self.difference(b, sides, d.percent));
                }
                continue;
            }
            // a category difference: the same row with each of its two categories
            let p = difference_at.expect("a difference column");
            let Choice::Difference(d) = choices[p] else {
                continue;
            };
            let Some((a, b)) = &found[d] else {
                continue;
            };
            let side = |key: &Key| {
                let mut row = choices.clone();
                row[p] = Choice::Category(key.clone());
                plain(&row)
                    .and_then(|g| index(&g))
                    .filter(|&g| shown_group[g])
            };
            let (Some(ga), Some(gb)) = (side(a), side(b)) else {
                continue;
            };
            for (s, of) in statistics.iter().enumerate() {
                let sides = (of[ga].as_slice(), of[gb].as_slice());
                let cells = self.difference(s, sides, command.differences[d].percent);
                push(names(s), cells);
            }
        }
        let mut columns: Vec<String> = controls.iter().map(|v| self.shown.variable(v)).collect();
        columns.push(String::new());
        columns.extend(command.cells.iter().map(|c| cell_name(*c, &command.within)));
        if formats.iter().flatten().all(String::is_empty) {
            formats.clear();
        }
        Table {
            title: command.title.clone().unwrap_or_else(|| "OLAP Cubes".into()),
            row_headers: heads,
            column_groups: vec![Vec::new(); columns.len()],
            columns,
            rows,
            formats,
            caption: command.footnote.clone(),
            ..Table::default()
        }
    }
}
