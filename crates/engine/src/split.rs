//! How procedures see the data: SPLIT FILE, FILTER, TEMPORARY, N OF CASES and SAMPLE.
//!
//! Rules, from the descriptions of these commands:
//! - A procedure leaves out the cases whose filter value is 0, system-missing or
//!   user-missing. The cases stay in the dataset.
//! - With a split, a procedure runs once per split group: a run of adjacent cases (of those
//!   the filter keeps) with the same values of the split variables. LAYERED shows the groups
//!   in one table with the split variables as the outermost row labels; SEPARATE gives each
//!   group tables of its own, the group named in the title.
//! - AGGREGATE ignores the split. It aggregates the cases the filter keeps; with
//!   MODE=ADDVARIABLES the other cases get system-missing values, as z-scores of DESCRIPTIVES
//!   do.
//! - TEMPORARY: everything after it is undone once the next command that reads the data has
//!   run. Variables that command adds stay, when the temporary changes kept the cases.
//! - N OF CASES keeps the first n cases at the next data pass, after case selection.
//! - SAMPLE draws its random numbers from the session's generator, which SET SEED, RNG and
//!   MTINDEX start.

use crate::crosstabs::Key;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Values};
use statscraft_syntax::{Command, Sample};

/// The state TEMPORARY returns to.
#[derive(Clone)]
pub(crate) struct Saved {
    dataset: Option<Dataset>,
    case_limit: Option<usize>,
}

/// One split group: the values that name it and its cases.
struct Group {
    /// One text per split variable as tables show it (SET TNUMBERS) and as titles do
    /// (ONUMBERS).
    values: Vec<String>,
    titles: Vec<String>,
    cases: Vec<usize>,
}

/// The value of every split variable at a case, to find where a group ends.
fn keys(data: &Dataset, names: &[String], case: usize) -> Vec<Option<Key>> {
    names
        .iter()
        .map(|name| {
            let variable = data.variable(name).expect("split variables exist");
            match variable.values() {
                Values::Numeric(values) => values.get(case).map(Key::Number),
                Values::String(values) => Some(Key::Text(values.get(case).to_string())),
            }
        })
        .collect()
}

impl Engine {
    pub(crate) fn random_state(&self) -> crate::RandomState {
        self.random.borrow().state()
    }

    pub(crate) fn set_random_state(&mut self, state: crate::RandomState) {
        self.random.get_mut().restore(state);
    }

    /// A number from 0 to 1 from the session's generator.
    pub(crate) fn random(&mut self) -> f64 {
        self.random.get_mut().uniform()
    }

    pub(crate) fn temporary(&mut self) {
        if self.temporary.is_none() {
            self.temporary = Some(Box::new(Saved {
                dataset: self.dataset.clone(),
                case_limit: self.case_limit,
            }));
        }
    }

    pub(crate) fn in_temporary(&self) -> bool {
        self.temporary.is_some()
    }

    /// The start of a command that reads the data: the transformations kept for the
    /// case-by-case pass run, scratch variables are gone and the pending N OF CASES takes
    /// effect.
    pub(crate) fn begin_pass(&mut self) -> Result<(), Error> {
        self.flush()?;
        self.leave.clear();
        self.drop_scratch()?;
        self.vectors.clear();
        if let (Some(limit), Some(data)) = (self.case_limit.take(), self.dataset.as_mut())
            && data.case_count() > limit
        {
            let keep: Vec<bool> = (0..data.case_count()).map(|case| case < limit).collect();
            data.select_cases(&keep)?;
        }
        Ok(())
    }

    /// The end of a command that reads the data: TEMPORARY ends. `before` is the dataset the
    /// command started with; the variables the command added stay.
    pub(crate) fn end_pass(&mut self, before: Option<&Dataset>) {
        let Some(saved) = self.temporary.take() else {
            return;
        };
        let after = std::mem::replace(&mut self.dataset, saved.dataset);
        self.case_limit = saved.case_limit;
        let (Some(after), Some(before), Some(restored)) = (after, before, self.dataset.as_mut())
        else {
            return;
        };
        let added: Vec<&str> = after
            .variable_names()
            .filter(|name| before.variable_index(name).is_none())
            .collect();
        if added.is_empty() {
            return;
        }
        if after.case_count() != restored.case_count() {
            self.warnings.push(format!(
                "the variables {} are dropped with the temporary changes: those changed the cases",
                added.join(", ")
            ));
            return;
        }
        let cases: Vec<Option<usize>> = (0..after.case_count()).map(Some).collect();
        for name in added {
            if restored.variable_index(name).is_none() {
                restored
                    .copy_variable(&after, name, &cases)
                    .expect("same cases, new name");
            }
        }
    }

    /// SAMPLE: keeps a random sample of the cases of the open DO IF branch; the other cases
    /// stay.
    pub(crate) fn sample(&mut self, sample: &Sample) -> Result<(), Error> {
        let cases = self.active()?.case_count();
        let eligible: Vec<bool> = match self.active_cases() {
            Some(active) => active.to_vec(),
            None => vec![true; cases],
        };
        let mut keep = vec![true; cases];
        let (mut seen, mut chosen) = (0u64, 0u64);
        for case in (0..cases).filter(|&case| eligible[case]) {
            keep[case] = match *sample {
                Sample::Fraction(p) => self.random() < p,
                // selection sampling: every set of n of the first m cases is equally likely
                Sample::Exactly { n, from } => {
                    let take = seen < from
                        && (self.random() * (from - seen) as f64) < n.saturating_sub(chosen) as f64;
                    seen += 1;
                    chosen += u64::from(take);
                    take
                }
            };
        }
        self.select_kept(&keep)
    }

    /// Keeps the cases whose entry is true, in the dataset and in the open DO IF blocks.
    pub(crate) fn select_kept(&mut self, keep: &[bool]) -> Result<(), Error> {
        for block in &mut self.blocks {
            block.retain(keep);
        }
        for mask in self.scratch_assigned.values_mut() {
            let mut kept = keep.iter();
            mask.retain(|_| *kept.next().unwrap_or(&true));
        }
        Ok(self.active_mut()?.select_cases(keep)?)
    }

    /// The split groups of the cases the filter keeps; None when every case is used as it is.
    fn groups(&self, use_split: bool) -> Result<Option<Vec<Group>>, Error> {
        let data = self.active()?;
        let (shown, outline) = (self.shown(), self.outline());
        let split = data.split_file().filter(|_| use_split);
        let filter = data.filter_variable();
        if split.is_none() && filter.is_none() {
            return Ok(None);
        }
        let mut kept: Vec<usize> = (0..data.case_count()).collect();
        if let Some(filter) = filter {
            let variable = data.variable(filter)?;
            variable.numeric()?;
            kept.retain(|&case| variable.keeps(case));
        }
        let Some(split) = split else {
            return Ok(Some(vec![Group {
                values: Vec::new(),
                titles: Vec::new(),
                cases: kept,
            }]));
        };
        let mut groups: Vec<Group> = Vec::new();
        let mut previous = None;
        for case in kept {
            let key = keys(data, &split.variables, case);
            if previous.as_ref() != Some(&key) {
                let values = |shown: &crate::shown::Shown| {
                    let pairs = split.variables.iter().zip(&key);
                    let texts = pairs.map(|(name, key)| {
                        let variable = data.variable(name).expect("split variables exist");
                        key.as_ref()
                            .map_or(".".to_string(), |key| shown.value(&variable, key))
                    });
                    texts.collect()
                };
                groups.push(Group {
                    values: values(&shown),
                    titles: values(&outline),
                    cases: Vec::new(),
                });
                previous = Some(key);
            }
            groups.last_mut().expect("just pushed").cases.push(case);
        }
        Ok(Some(groups))
    }

    /// Runs a procedure as the split, the filter and TEMPORARY say.
    pub(crate) fn procedure(&mut self, command: &Command) -> Result<Vec<Table>, Error> {
        self.begin_pass()?;
        let before = self.dataset.clone();
        // TREE ignores SPLIT FILE, as the manual says
        let use_split = !matches!(command, Command::Aggregate(_) | Command::Tree(_));
        let result = self.in_view(command, use_split);
        if let Command::Aggregate(aggregate) = command
            && aggregate.outfile.is_none()
            && !aggregate.add_variables
        {
            // the aggregated dataset replaces the active one for good
            self.temporary = None;
        }
        self.end_pass(before.as_ref());
        result
    }

    fn in_view(&mut self, command: &Command, use_split: bool) -> Result<Vec<Table>, Error> {
        let Some(groups) = self.groups(use_split)? else {
            return self.run_procedure(command);
        };
        let full = self.dataset.take().expect("checked by groups");
        let earlier = std::mem::take(&mut self.warnings);
        let split = full.split_file().filter(|_| use_split).cloned();
        // LAYERED puts the groups in the tables, SEPARATE in the titles
        let layered_split = split.as_ref().is_some_and(|s| s.layered);
        let shown = match layered_split {
            true => self.shown(),
            false => self.outline(),
        };
        let split_names: Vec<String> = split
            .iter()
            .flat_map(|s| &s.variables)
            .map(|name| shown.named(&full, name))
            .collect();
        let mut results: Vec<(Vec<String>, Vec<Table>)> = Vec::new();
        let mut afters: Vec<Dataset> = Vec::new();
        // the warnings of each group, with the group's name
        let (mut given, mut errors) = (Vec::new(), Vec::new());
        for group in &groups {
            let mut part = full.clone();
            let cases: Vec<Option<usize>> = group.cases.iter().map(|&c| Some(c)).collect();
            part.take_cases(&cases)?;
            part.set_split_file(None)?;
            part.set_filter_variable(None)?;
            self.dataset = Some(part);
            self.case_numbers = Some(group.cases.clone());
            self.warnings.clear();
            let texts = if layered_split {
                &group.values
            } else {
                &group.titles
            };
            let name = group_name(Some(split_names.as_slice()), texts);
            let prefix = |message: &str| match name.is_empty() {
                true => message.to_string(),
                false => format!("{name}: {message}"),
            };
            match self.run_procedure(command) {
                Ok(tables) => results.push((texts.clone(), tables)),
                Err(error) => errors.push(prefix(&error.0)),
            }
            given.push((name, std::mem::take(&mut self.warnings)));
            afters.push(self.dataset.take().expect("procedures keep a dataset"));
        }
        self.case_numbers = None;
        self.warnings = earlier;
        self.warnings.extend(group_warnings(given));
        if results.is_empty() && !errors.is_empty() {
            self.dataset = Some(full);
            return Err(Error(errors.swap_remove(0)));
        }
        self.warnings.extend(errors);
        if let Command::Aggregate(aggregate) = command
            && aggregate.outfile.is_none()
            && !aggregate.add_variables
        {
            self.dataset = afters.pop();
            return Ok(Vec::new());
        }
        self.dataset = Some(write_back(full, &groups, &afters)?);
        Ok(match split {
            None => results.into_iter().flat_map(|(_, tables)| tables).collect(),
            Some(_) if !layered_split => separate(&split_names, results),
            Some(_) => layered(&split_names, results),
        })
    }
}

/// The warnings of the split groups: one that every group gave, such as plots not drawn, once;
/// the others after the name of their group.
fn group_warnings(given: Vec<(String, Vec<String>)>) -> Vec<String> {
    let every = |w: &String| given.len() > 1 && given.iter().all(|(_, ws)| ws.contains(w));
    let common: Vec<String> = given.first().map_or(Vec::new(), |(_, ws)| {
        ws.iter().filter(|w| every(w)).cloned().collect()
    });
    let mut out = common.clone();
    for (name, warnings) in &given {
        for warning in warnings.iter().filter(|w| !common.contains(w)) {
            out.push(match name.is_empty() {
                true => warning.clone(),
                false => format!("{name}: {warning}"),
            });
        }
    }
    out
}

#[cfg(test)]
#[test]
fn a_warning_of_every_group_comes_once_the_others_with_their_group() {
    let given = |groups: &[(&str, &[&str])]| -> Vec<(String, Vec<String>)> {
        let words = |ws: &[&str]| ws.iter().map(|w| w.to_string()).collect();
        groups
            .iter()
            .map(|(n, ws)| (n.to_string(), words(ws)))
            .collect()
    };
    let two = given(&[("g = 1", &["plots", "empty"]), ("g = 2", &["plots"])]);
    assert_eq!(group_warnings(two), ["plots", "g = 1: empty"]);
    // with one group the name stays, as nothing tells it apart
    let one = given(&[("g = 1", &["plots"])]);
    assert_eq!(group_warnings(one), ["g = 1: plots"]);
}

/// "region = North, gender = Female"
fn group_name(names: Option<&[String]>, values: &[String]) -> String {
    let pairs = names.unwrap_or_default().iter().zip(values);
    let pairs: Vec<String> = pairs.map(|(n, v)| format!("{n} = {v}")).collect();
    pairs.join(", ")
}

/// The full dataset with the variables the procedure added to each group, system-missing
/// (or blank) in the cases outside every group.
fn write_back(mut full: Dataset, groups: &[Group], afters: &[Dataset]) -> Result<Dataset, Error> {
    let Some(first) = afters.first() else {
        return Ok(full);
    };
    let added: Vec<String> = first
        .variable_names()
        .filter(|name| full.variable_index(name).is_none())
        .map(str::to_string)
        .collect();
    let mut at = vec![None; full.case_count()];
    for (group, case) in groups.iter().enumerate().flat_map(|(g, group)| {
        group
            .cases
            .iter()
            .enumerate()
            .map(move |(i, &c)| (g, (i, c)))
    }) {
        at[case.1] = Some((group, case.0));
    }
    for name in added {
        let from_first: Vec<Option<usize>> = at
            .iter()
            .map(|place| place.filter(|(g, _)| *g == 0).map(|(_, i)| i))
            .collect();
        full.copy_variable(first, &name, &from_first)?;
        if afters.len() == 1 {
            continue;
        }
        let variable = |g: usize| afters[g].variable(&name);
        match variable(0)?.values() {
            Values::Numeric(_) => {
                let values = at.iter().map(|place| match place {
                    Some((g, i)) => variable(*g).ok()?.numeric().ok()?.get(*i),
                    None => None,
                });
                full.set_numeric_values(&name, values.collect())?;
            }
            Values::String(_) => {
                let width = variable(0)?.string_width().unwrap_or(1);
                let values = at.iter().map(|place| match place {
                    Some((g, i)) => variable(*g)
                        .ok()
                        .and_then(|v| v.string().ok().map(|s| s.get(*i).to_string()))
                        .unwrap_or_default(),
                    None => String::new(),
                });
                full.set_string_values(&name, width, values.collect())?;
            }
        }
    }
    Ok(full)
}

/// SEPARATE: every group's tables, the group named in the title.
fn separate(names: &[String], results: Vec<(Vec<String>, Vec<Table>)>) -> Vec<Table> {
    let mut out = Vec::new();
    for (values, tables) in results {
        let name = group_name(Some(names), &values);
        for mut table in tables {
            table.title = format!("{} ({name})", table.title);
            out.push(table);
        }
    }
    out
}

/// The full header of a column: the labels above it and its own.
fn path(table: &Table, column: usize) -> Vec<String> {
    let mut path = table.column_groups.get(column).cloned().unwrap_or_default();
    path.push(table.columns[column].clone());
    path
}

/// LAYERED: the tables of the groups merged into one each, matched by title and order, with
/// the split variables as the outermost row labels. A column that only some groups have is
/// empty in the others.
fn layered(names: &[String], results: Vec<(Vec<String>, Vec<Table>)>) -> Vec<Table> {
    // (title, occurrence) of every table, in order of first appearance
    let mut keys: Vec<(String, usize)> = Vec::new();
    let mut members: Vec<Vec<(Vec<String>, Table)>> = Vec::new();
    for (values, tables) in results {
        let mut seen: Vec<String> = Vec::new();
        for table in tables {
            let occurrence = seen.iter().filter(|t| **t == table.title).count();
            seen.push(table.title.clone());
            let key = (table.title.clone(), occurrence);
            let index = match keys.iter().position(|k| *k == key) {
                Some(index) => index,
                None => {
                    keys.push(key);
                    members.push(Vec::new());
                    keys.len() - 1
                }
            };
            members[index].push((values.clone(), table));
        }
    }
    let mut out = Vec::new();
    for group in members {
        let headers = group[0].1.row_headers;
        if group.iter().any(|(_, t)| t.row_headers != headers) {
            out.extend(separate(
                names,
                group.into_iter().map(|(v, t)| (v, vec![t])).collect(),
            ));
            continue;
        }
        out.push(merge(names, group));
    }
    out
}

fn merge(names: &[String], group: Vec<(Vec<String>, Table)>) -> Table {
    let first = &group[0].1;
    let headers = first.row_headers;
    // the data columns of all groups, each in its place among the columns of the first
    // group that has it
    let mut columns: Vec<Vec<String>> = Vec::new();
    for (_, table) in &group {
        let mut after: Option<usize> = None;
        for column in headers..table.columns.len() {
            let path = path(table, column);
            match columns.iter().position(|c| *c == path) {
                Some(at) => after = Some(at),
                None => {
                    let at = after.map_or(0, |a| a + 1);
                    columns.insert(at, path);
                    after = Some(at);
                }
            }
        }
    }
    let k = names.len();
    let grouped = group.iter().any(|(_, t)| !t.column_groups.is_empty());
    let formatted = group.iter().any(|(_, t)| !t.formats.is_empty());
    let mut table = Table {
        title: first.title.clone(),
        row_headers: headers + k,
        caption: first.caption.clone(),
        ..Table::default()
    };
    table.columns = names.to_vec();
    table
        .columns
        .extend(first.columns[..headers].iter().cloned());
    table.columns.extend(
        columns
            .iter()
            .map(|p| p.last().cloned().unwrap_or_default()),
    );
    if grouped {
        table.column_groups = vec![Vec::new(); k];
        table
            .column_groups
            .extend((0..headers).map(|c| first.column_groups.get(c).cloned().unwrap_or_default()));
        table
            .column_groups
            .extend(columns.iter().map(|p| p[..p.len() - 1].to_vec()));
    }
    for (values, part) in &group {
        let places: Vec<Option<usize>> = columns
            .iter()
            .map(|p| (headers..part.columns.len()).find(|&c| path(part, c) == *p))
            .collect();
        for (r, row) in part.rows.iter().enumerate() {
            let mut cells: Vec<Cell> = match r {
                0 => values.iter().cloned().map(Cell::Text).collect(),
                _ => vec![Cell::Empty; k],
            };
            cells.extend(row[..headers].iter().cloned());
            cells.extend(
                places
                    .iter()
                    .map(|p| p.map_or(Cell::Empty, |c| row[c].clone())),
            );
            table.rows.push(cells);
            if formatted {
                let format = |c: usize| {
                    part.formats
                        .get(r)
                        .and_then(|f| f.get(c))
                        .cloned()
                        .unwrap_or_default()
                };
                let mut formats = vec![String::new(); k];
                formats.extend((0..headers).map(format));
                formats.extend(places.iter().map(|p| p.map(format).unwrap_or_default()));
                table.formats.push(formats);
            }
        }
    }
    let captions: Vec<&String> = group
        .iter()
        .filter_map(|(_, t)| t.caption.as_ref())
        .collect();
    if captions.iter().any(|c| Some(*c) != first.caption.as_ref()) {
        let mut distinct: Vec<&String> = Vec::new();
        for caption in captions {
            if !distinct.contains(&caption) {
                distinct.push(caption);
            }
        }
        table.caption = Some(distinct.into_iter().cloned().collect::<Vec<_>>().join("\n"));
    }
    table
}
