//! ADD FILES, MATCH FILES and UPDATE: one dataset from several, cases appended, joined or
//! updated.
//!
//! UPDATE, from its description: the first file is the primary file and may hold each key
//! once; a transaction value replaces the value of the case with the same key unless it is
//! missing (system- or user-missing) or blank. Transactions apply in order, case by case and
//! file by file, so the last value that is not missing wins. Keys and variables that only
//! transactions have are added. IN variables go at the end and cannot be dropped.

use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, MissingValues, Values, Variable};
use statscraft_syntax::{Merge, MergeInput, Subset, VarItem};
use std::cmp::Ordering;

/// One value of the BY variables.
#[derive(PartialEq)]
enum Key<'a> {
    Number(Option<f64>),
    Text(&'a str),
}

/// System-missing is lower than every number, as in SORT CASES.
fn compare(a: &[Key], b: &[Key]) -> Ordering {
    for (x, y) in a.iter().zip(b) {
        let ordering = match (x, y) {
            (Key::Number(x), Key::Number(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
            (Key::Text(x), Key::Text(y)) => x.cmp(y),
            _ => Ordering::Equal,
        };
        if ordering.is_ne() {
            return ordering;
        }
    }
    Ordering::Equal
}

/// An input after its renames, with how messages name it.
struct Input {
    name: String,
    data: Dataset,
    table: bool,
    /// The name every variable had before the renames, in dictionary order.
    old_names: Vec<String>,
}

impl Input {
    fn keys<'a>(&'a self, by: &[String]) -> Result<Vec<Vec<Key<'a>>>, Error> {
        let mut columns = Vec::new();
        for name in by {
            let variable = self.data.variable(name).map_err(|_| {
                Error(format!(
                    "the BY variable {name} is not in {}; every file needs the key variables",
                    self.name
                ))
            })?;
            columns.push(variable.values());
        }
        let key = |case: usize| -> Vec<Key<'a>> {
            columns
                .iter()
                .map(|values| match values {
                    Values::Numeric(v) => Key::Number(v.get(case)),
                    Values::String(v) => Key::Text(v.get(case)),
                })
                .collect()
        };
        let keys: Vec<Vec<Key>> = (0..self.data.case_count()).map(key).collect();
        if self.table
            && let Some(case) = (1..keys.len()).find(|&c| compare(&keys[c - 1], &keys[c]).is_eq())
        {
            return Err(Error(format!(
                "{} holds the same key twice (cases {case} and {}); a table may hold each key once",
                self.name,
                case + 1
            )));
        }
        if let Some(case) = (1..keys.len()).find(|&c| compare(&keys[c - 1], &keys[c]).is_gt()) {
            let keys = by.join(" ");
            let fix = if self.name.ends_with('*') {
                format!("sort the active dataset with SORT CASES BY {keys} first")
            } else {
                format!("sort it with SORT CASES BY {keys} and save it again")
            };
            return Err(Error(format!(
                "{} is not sorted by {keys}: case {} has a lower key than case {case}; {fix}",
                self.name,
                case + 1,
            )));
        }
        Ok(keys)
    }
}

/// For every output case, the case of each input it is made of: `cases[input][output]`.
fn match_cases(
    inputs: &[Input],
    keys: &[Vec<Vec<Key>>],
    by: bool,
    add: bool,
) -> Vec<Vec<Option<usize>>> {
    let count = inputs.len();
    let mut cases: Vec<Vec<Option<usize>>> = vec![Vec::new(); count];
    let mut push = |row: Vec<Option<usize>>| {
        for (input, case) in row.into_iter().enumerate() {
            cases[input].push(case);
        }
    };
    if add {
        let mut all: Vec<(usize, usize)> = (0..count)
            .flat_map(|input| (0..inputs[input].data.case_count()).map(move |c| (input, c)))
            .collect();
        if by {
            // stable: equal keys keep the order of the files
            all.sort_by(|a, b| compare(&keys[a.0][a.1], &keys[b.0][b.1]));
        }
        for (input, case) in all {
            let mut row = vec![None; count];
            row[input] = Some(case);
            push(row);
        }
    } else if !by {
        let longest = inputs
            .iter()
            .map(|i| i.data.case_count())
            .max()
            .unwrap_or(0);
        for case in 0..longest {
            push(
                inputs
                    .iter()
                    .map(|i| (case < i.data.case_count()).then_some(case))
                    .collect(),
            );
        }
    } else {
        let mut next = vec![0; count];
        loop {
            // the lowest key not yet used among the files that add cases
            let lowest = (0..count)
                .filter(|&i| !inputs[i].table && next[i] < keys[i].len())
                .map(|i| &keys[i][next[i]])
                .min_by(|a, b| compare(a, b));
            let Some(lowest) = lowest else { break };
            let mut groups = Vec::with_capacity(count);
            for input in 0..count {
                let matching = |c: &usize| compare(&keys[input][*c], lowest).is_eq();
                if inputs[input].table {
                    while next[input] < keys[input].len()
                        && compare(&keys[input][next[input]], lowest).is_lt()
                    {
                        next[input] += 1;
                    }
                    let found = Some(next[input]).filter(|c| *c < keys[input].len() && matching(c));
                    groups.push((found, 1));
                } else {
                    let start = next[input];
                    while next[input] < keys[input].len() && matching(&next[input]) {
                        next[input] += 1;
                    }
                    groups.push((Some(start), next[input] - start));
                }
            }
            let longest = (0..count)
                .filter(|&i| !inputs[i].table)
                .map(|i| groups[i].1)
                .max()
                .unwrap_or(0);
            for k in 0..longest {
                push(
                    (0..count)
                        .map(|i| match groups[i] {
                            (found, _) if inputs[i].table => found,
                            (Some(start), length) if k < length => Some(start + k),
                            _ => None,
                        })
                        .collect(),
                );
            }
        }
    }
    cases
}

/// The dictionary entry a later file adds to a variable: what the earlier ones left empty.
fn fill_dictionary(out: &mut Dataset, name: &str, from: &Dataset) -> Result<(), Error> {
    let (have, other) = (out.variable(name)?, from.variable(name)?);
    let label = other.label().filter(|_| have.label().is_none());
    let labels = (have.value_labels().is_empty() && !other.value_labels().is_empty())
        .then(|| other.value_labels().to_vec());
    let missing =
        (*have.missing_values() == MissingValues::None).then(|| other.missing_values().clone());
    if let Some(label) = label.map(str::to_string) {
        out.set_variable_label(name, Some(&label))?;
    }
    if let Some(labels) = labels {
        out.set_value_labels(name, labels)?;
    }
    if let Some(missing) = missing {
        out.set_missing_values(name, missing)?;
    }
    Ok(())
}

impl Engine {
    fn merge_input(&self, input: &MergeInput) -> Result<Input, Error> {
        let (name, data) = match &input.file {
            None => ("FILE=*".to_string(), self.active()?.clone()),
            Some(file) => (format!("FILE='{file}'"), self.data_file(file)?),
        };
        let old_names = data.variable_names().map(str::to_string).collect();
        let mut renamed = Engine::with_dataset(Some(data));
        if !input.rename.is_empty() {
            renamed
                .rename_variables(&input.rename)
                .map_err(|e| Error(format!("/RENAME of {name}: {e}")))?;
        }
        Ok(Input {
            name: if input.table {
                name.replace("FILE", "TABLE")
            } else {
                name
            },
            data: renamed.dataset.expect("set above"),
            table: input.table,
            old_names,
        })
    }

    pub(crate) fn merge_files(&mut self, merge: &Merge, add: bool) -> Result<(), Error> {
        let inputs: Vec<Input> = merge
            .inputs
            .iter()
            .map(|input| self.merge_input(input))
            .collect::<Result<_, _>>()?;
        let by = Engine::with_dataset(Some(inputs[0].data.clone())).expand(&merge.by)?;
        let variables = union(&inputs)?;

        let keys: Vec<Vec<Vec<Key>>> = if by.is_empty() {
            inputs.iter().map(|_| Vec::new()).collect()
        } else {
            inputs
                .iter()
                .map(|i| i.keys(&by))
                .collect::<Result<_, _>>()?
        };
        let cases = match_cases(&inputs, &keys, !by.is_empty(), add);
        // without a table, equal keys are matched in order, case by case, and the manual asks
        // for a warning; with a table, repeated keys in the files are what it is for
        let tables = inputs.iter().any(|input| input.table);
        let repeated: Vec<&str> = (inputs.iter().zip(&keys))
            .filter(|(_, keys)| !add && !tables && keys.windows(2).any(|k| k[0] == k[1]))
            .map(|(input, _)| input.name.as_str())
            .collect();
        drop(keys);
        for name in repeated {
            self.warn(format!(
                "{name} holds some keys more than once; cases with the same key are matched in order"
            ));
        }

        let mut out = inputs[0].data.clone();
        out.take_cases(&cases[0])?;
        for (name, holders) in &variables {
            let owner = holders[0];
            if owner != 0 {
                out.copy_variable(&inputs[owner].data, name, &cases[owner])?;
            }
            if holders.len() == 1 {
                continue;
            }
            // the value of a case comes from the first file that has the case
            let sources: Vec<Option<(usize, usize)>> = (0..out.case_count())
                .map(|output| {
                    let holder = holders.iter().find(|&&h| cases[h][output].is_some())?;
                    Some((*holder, cases[*holder][output]?))
                })
                .collect();
            set_from(&mut out, name, holders, &inputs, &sources)?;
        }
        // ADD FILES keeps the weighting when every file is weighted by the same variable
        let weight = |input: &Input| input.data.weight_variable().map(str::to_lowercase);
        let weighted = add && inputs.iter().all(|i| weight(i) == weight(&inputs[0]));
        if !weighted {
            out.set_weight_variable(None)?;
        }
        self.finish_merge(out, merge, &inputs).map(|_| ())
    }

    /// UPDATE: the primary file with the values of the transaction files, key by key.
    pub(crate) fn update_files(&mut self, merge: &Merge) -> Result<Vec<Table>, Error> {
        let inputs: Vec<Input> = merge
            .inputs
            .iter()
            .map(|input| self.merge_input(input))
            .collect::<Result<_, _>>()?;
        let by = Engine::with_dataset(Some(inputs[0].data.clone())).expand(&merge.by)?;
        let variables = union(&inputs)?;
        let flags: Vec<(usize, &String)> = (merge.inputs.iter().enumerate())
            .filter_map(|(i, input)| Some((i, input.flag.as_ref()?)))
            .collect();
        let subsets = merge.subsets.iter().filter_map(|subset| match subset {
            Subset::Keep(items) | Subset::Drop(items) => Some(items),
            Subset::Map => None,
        });
        for item in subsets.flatten() {
            if let VarItem::Name(name) = item
                && flags
                    .iter()
                    .any(|(_, flag)| flag.eq_ignore_ascii_case(name))
            {
                return Err(Error(format!(
                    "{name} is an /IN variable: it goes at the end of the result and cannot be \
                     named on KEEP or DROP"
                )));
            }
        }
        let keys: Vec<Vec<Vec<Key>>> = inputs
            .iter()
            .map(|i| i.keys(&by))
            .collect::<Result<_, _>>()?;
        if let Some(case) =
            (1..keys[0].len()).find(|&c| compare(&keys[0][c - 1], &keys[0][c]).is_eq())
        {
            return Err(Error(format!(
                "the primary file {} holds the same key twice (cases {case} and {}); UPDATE \
                 needs each key once in it",
                inputs[0].name,
                case + 1
            )));
        }
        // for every output case, the cases of each input with its key: `cases[input][output]`
        let count = inputs.len();
        let mut cases: Vec<Vec<Vec<usize>>> = vec![Vec::new(); count];
        let mut next = vec![0; count];
        loop {
            let lowest = (0..count)
                .filter(|&i| next[i] < keys[i].len())
                .map(|i| &keys[i][next[i]])
                .min_by(|a, b| compare(a, b));
            let Some(lowest) = lowest else { break };
            for input in 0..count {
                let start = next[input];
                while next[input] < keys[input].len()
                    && compare(&keys[input][next[input]], lowest).is_eq()
                {
                    next[input] += 1;
                }
                cases[input].push((start..next[input]).collect());
            }
        }
        drop(keys);
        let outputs = cases[0].len();

        let mut out = inputs[0].data.clone();
        out.take_cases(
            &cases[0]
                .iter()
                .map(|c| c.first().copied())
                .collect::<Vec<_>>(),
        )?;
        for (name, holders) in &variables {
            let owner = holders[0];
            if owner != 0 {
                out.copy_variable(&inputs[owner].data, name, &vec![None; outputs])?;
            }
            let key = by.iter().any(|b| b.eq_ignore_ascii_case(name));
            let sources: Vec<Option<(usize, usize)>> = (0..outputs)
                .map(|output| {
                    let first = holders
                        .iter()
                        .find_map(|&h| Some((h, *cases[h][output].first()?)));
                    if key {
                        return first;
                    }
                    // the primary value, then every transaction value that is not missing
                    let mut source = first.filter(|(h, _)| *h == 0);
                    for &h in holders.iter().filter(|&&h| h != 0) {
                        let variable = inputs[h].data.variable(name).expect("a holder has it");
                        for &case in &cases[h][output] {
                            if !missing_at(&variable, case) {
                                source = Some((h, case));
                            }
                        }
                    }
                    source
                })
                .collect();
            set_from(&mut out, name, holders, &inputs, &sources)?;
        }
        // choice: the primary file's weight stays while its variable does
        let weight = inputs[0].data.weight_variable().map(str::to_string);
        let tables = self.finish_merge(out, merge, &inputs)?;
        let data = self.active_mut()?;
        for (input, flag) in flags {
            if data.variable_index(flag).is_some() {
                return Err(Error(format!(
                    "the /IN variable {flag} has the name of a variable of the result"
                )));
            }
            let values = cases[input]
                .iter()
                .map(|c| Some(if c.is_empty() { 0.0 } else { 1.0 }))
                .collect();
            data.set_numeric_values(flag, values)?;
            data.set_format(flag, "F1.0")?;
        }
        if let Some(weight) = weight.filter(|w| data.variable_index(w).is_some()) {
            data.set_weight_variable(Some(&weight))?;
        }
        Ok(tables)
    }

    /// KEEP, DROP and MAP of a merge in the order written, then the filter and the split of the
    /// active dataset when it was merged. The result becomes the active dataset.
    fn finish_merge(
        &mut self,
        out: Dataset,
        merge: &Merge,
        inputs: &[Input],
    ) -> Result<Vec<Table>, Error> {
        let mut tables = Vec::new();
        let mut result = Engine::with_dataset(Some(out));
        for subset in &merge.subsets {
            let (items, keep) = match subset {
                Subset::Keep(items) => (items, true),
                Subset::Drop(items) => (items, false),
                Subset::Map => {
                    tables.push(map(result.active()?, inputs));
                    continue;
                }
            };
            let mut names = result.expand(items)?;
            // a variable named twice, as with `id ALL`, keeps its first place
            let mut seen = Vec::new();
            names.retain(|name| {
                let fresh = !seen.contains(name);
                seen.push(name.clone());
                fresh
            });
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            let data = result.active_mut()?;
            if keep {
                data.keep_variables(&names)?;
            } else {
                data.delete_variables(&names)?;
            }
        }
        let mut out = result.dataset.expect("the merge made a dataset");
        // the filter and the split stay when the active dataset was merged and the result
        // still has their variables
        let active = self
            .dataset
            .as_ref()
            .filter(|_| merge.inputs.iter().any(|i| i.file.is_none()));
        let filter = active.and_then(Dataset::filter_variable);
        let split = active.and_then(Dataset::split_file).cloned();
        let numeric = |name: &str| out.variable(name).is_ok_and(|v| !v.is_string());
        let filter = filter.filter(|name| numeric(name)).map(str::to_string);
        out.set_filter_variable(filter.as_deref())?;
        let split = split.filter(|s| s.variables.iter().all(|n| out.variable_index(n).is_some()));
        out.set_split_file(split)?;
        self.dataset = Some(out);
        Ok(tables)
    }
}

/// Every variable once, in the order the files bring them, with the files that have it. A
/// variable may not be numeric in one file and a string in another.
fn union(inputs: &[Input]) -> Result<Vec<(String, Vec<usize>)>, Error> {
    let mut variables: Vec<(String, Vec<usize>)> = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        for name in input.data.variable_names() {
            let seen = variables
                .iter_mut()
                .find(|(n, _)| n.eq_ignore_ascii_case(name));
            match seen {
                Some((_, holders)) => holders.push(index),
                None => variables.push((name.to_string(), vec![index])),
            }
        }
    }
    for (name, holders) in &variables {
        let first = &inputs[holders[0]];
        let first_is_string = first.data.variable(name)?.is_string();
        for &other in &holders[1..] {
            if inputs[other].data.variable(name)?.is_string() != first_is_string {
                let kind = |string: bool| if string { "a string" } else { "numeric" };
                return Err(Error(format!(
                    "{name} is {} in {} but {} in {}; rename one of them with /RENAME or change its type",
                    kind(first_is_string),
                    first.name,
                    kind(!first_is_string),
                    inputs[other].name
                )));
            }
        }
    }
    Ok(variables)
}

/// A system-missing, user-missing or blank value, which UPDATE does not take from a
/// transaction.
fn missing_at(variable: &Variable, case: usize) -> bool {
    match variable.values() {
        Values::Numeric(v) => v.get(case).is_none_or(|v| variable.is_user_missing(v)),
        Values::String(v) => {
            let value = v.get(case);
            value.trim_end_matches(' ').is_empty() || variable.is_user_missing_str(value)
        }
    }
}

/// Gives variable `name` of `out` the values of `sources`: for every case the input and its
/// case, None for a blank value. A string variable takes its widest width in the holders, and
/// the later holders fill in its dictionary entry.
fn set_from(
    out: &mut Dataset,
    name: &str,
    holders: &[usize],
    inputs: &[Input],
    sources: &[Option<(usize, usize)>],
) -> Result<(), Error> {
    let width = holders
        .iter()
        .map(|&h| inputs[h].data.variable(name).map(|v| v.string_width()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max()
        .flatten();
    let variable = |input: usize| inputs[input].data.variable(name);
    match width {
        Some(width) => {
            if out.variable(name)?.string_width() < Some(width) {
                out.set_string_width(name, width)?;
            }
            let values = sources
                .iter()
                .map(|source| match *source {
                    Some((input, case)) => Ok(variable(input)?.string()?.get(case).to_string()),
                    None => Ok(String::new()),
                })
                .collect::<Result<Vec<_>, Error>>()?;
            out.set_string_values(name, width, values)?;
        }
        None => {
            let values = sources
                .iter()
                .map(|source| match *source {
                    Some((input, case)) => Ok(variable(input)?.numeric()?.get(case)),
                    None => Ok(None),
                })
                .collect::<Result<Vec<_>, Error>>()?;
            out.set_numeric_values(name, values)?;
        }
    }
    for &later in &holders[1..] {
        fill_dictionary(out, name, &inputs[later].data)?;
    }
    Ok(())
}

/// MAP: the variables of the result as it is now, with their names in each input file.
fn map(data: &Dataset, inputs: &[Input]) -> Table {
    let mut columns = vec!["Result".to_string()];
    columns.extend(inputs.iter().map(|input| input.name.clone()));
    let rows = data
        .variable_names()
        .map(|name| {
            let mut row = vec![Cell::Text(name.to_string())];
            row.extend(
                inputs
                    .iter()
                    .map(|input| match input.data.variable_index(name) {
                        Some(index) => Cell::Text(input.old_names[index].clone()),
                        None => Cell::Empty,
                    }),
            );
            row
        })
        .collect();
    Table {
        title: "Variable Map".to_string(),
        row_headers: 1,
        columns,
        rows,
        ..Table::default()
    }
}
