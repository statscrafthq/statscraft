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
        // reading the data uses a waiting time program up; a COXREG took its own already
        self.time_program = None;
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

    /// The cases of every split group the filter keeps, in order; one group of all cases when
    /// nothing splits or filters.
    pub(crate) fn case_groups(&self) -> Result<Vec<Vec<usize>>, Error> {
        // CREATE ignores USE
        Ok(match self.groups(true, false)? {
            Some(groups) => groups.into_iter().map(|g| g.cases).collect(),
            None => vec![(0..self.active()?.case_count()).collect()],
        })
    }

    /// The split groups of the cases the filter keeps; None when every case is used as it is.
    fn groups(&self, use_split: bool, use_range: bool) -> Result<Option<Vec<Group>>, Error> {
        let data = self.active()?;
        let (shown, outline) = (self.shown(), self.outline());
        let split = data.split_file().filter(|_| use_split);
        let filter = data.filter_variable();
        let range = match use_range {
            true => self.use_bounds(data, None)?,
            false => None,
        };
        if split.is_none() && filter.is_none() && range.is_none() {
            return Ok(None);
        }
        let mut kept: Vec<usize> = (0..data.case_count()).collect();
        if let Some((first, last)) = range {
            kept.retain(|&case| case >= first && case <= last);
        }
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
        // TREE, DETECTANOMALY, OPTIMAL BINNING, MULTIPLE CORRESPONDENCE, CATPCA and the complex samples procedures ignore SPLIT FILE, as
        // TREE, DETECTANOMALY, MULTIPLE CORRESPONDENCE, CATPCA, CATREG and the complex samples procedures ignore SPLIT FILE, as
        // the manual says
        // TREE, DETECTANOMALY, MULTIPLE CORRESPONDENCE, CATPCA and the complex samples
        // procedures ignore SPLIT FILE, as the manual says; HOMALS, PRINCALS and OVERALS do as
        // the engines they share
        let use_split = !matches!(
            command,
            Command::Aggregate(_)
                | Command::Tree(_)
                | Command::Detectanomaly(_)
                | Command::OptimalBinning(_)
                | Command::GenLinMixed(_)
                | Command::MultipleCorrespondence(_)
                | Command::Catpca(_)
                | Command::Catreg(_)
                | Command::Homals(_)
                | Command::Princals(_)
                | Command::Overals(_)
                | Command::Csdescriptives(_)
                | Command::Csselect(_)
                | Command::Cstabulate(_)
                | Command::CsModel(_)
                | Command::CsCox(_)
        );
        let result = self.in_view(command, use_split);
        // also after an early error inside a group
        self.split_group = None;
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
        // EXSMOOTH, ARIMA and AREG apply USE themselves, as they also forecast after it
        let use_range = !matches!(
            command,
            Command::Exsmooth(_) | Command::Arima(_) | Command::Areg(_)
        );
        let Some(groups) = self.groups(use_split, use_range)? else {
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
        // data split by Imputation_ first: the imputation of every result and the items to pool
        let imputation_split = split.as_ref().is_some_and(|s| {
            s.variables
                .first()
                .is_some_and(|v| v.eq_ignore_ascii_case(crate::multiple_imputation::IMPUTATION))
        });
        let pools = imputation_split
            && matches!(
                command,
                Command::Descriptives(_)
                    | Command::Frequencies(_)
                    | Command::Means(_)
                    | Command::TTest(_)
                    | Command::Correlations(_)
                    | Command::Regression(_)
                    | Command::Logistic(_)
                    | Command::Nomreg(_)
                    | Command::Plum(_)
            );
        let mut result_imputation: Vec<Option<f64>> = Vec::new();
        let mut pooled: Vec<crate::pooling::Imputed> = Vec::new();
        let mut afters: Vec<Dataset> = Vec::new();
        // whether the procedure ran in each group
        let mut ran: Vec<bool> = Vec::new();
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
            self.split_group = split.as_ref().map(|s| {
                let first = group.cases.first().copied().unwrap_or(0);
                let values = keys(&full, &s.variables, first);
                s.variables.iter().cloned().zip(values).collect()
            });
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
            let charts = self.charts.len();
            let imputation = imputation_split
                .then(|| {
                    let first = *group.cases.first()?;
                    full.variable(&split.as_ref()?.variables[0])
                        .ok()?
                        .numeric()
                        .ok()?
                        .get(first)
                })
                .flatten();
            self.pool_items = (pools && imputation.is_some_and(|i| i >= 1.0)).then(Vec::new);
            let result = self.run_procedure(command);
            ran.push(result.is_ok());
            if let Some(items) = self.pool_items.take()
                && result.is_ok()
            {
                let others = texts[1..].to_vec();
                pooled.push(crate::pooling::Imputed { others, items });
            }
            match result {
                Ok(tables) => {
                    results.push((texts.clone(), tables));
                    result_imputation.push(imputation);
                }
                Err(error) => errors.push(prefix(&error.0)),
            }
            // a chart per group, the group named in the title as SEPARATE names it in tables
            for chart in &mut self.charts[charts..] {
                if !name.is_empty() {
                    chart.title = format!("{} ({name})", chart.title);
                }
            }
            given.push((name, std::mem::take(&mut self.warnings)));
            afters.push(self.dataset.take().expect("procedures keep a dataset"));
        }
        self.case_numbers = None;
        self.split_group = None;
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
        // SET MIOUTPUT: the original data, the imputations and the pooled results
        let mut pooled_tables = Vec::new();
        if imputation_split {
            let [observed, imputed, pool, diagnostics] =
                crate::pooling::mioutput(&self.setting("MIOUTPUT"));
            let mut shown = result_imputation.iter().map(|i| match i {
                Some(i) if *i == 0.0 => observed,
                Some(_) => imputed,
                None => true,
            });
            results.retain(|_| shown.next().unwrap_or(true));
            if pool {
                let warnings;
                (pooled_tables, warnings) =
                    crate::pooling::tables(&pooled, &split_names[1..], diagnostics);
                self.warnings.extend(warnings);
            }
        }
        // datasets share their columns, so the clone is cheap and keeps the data on an error
        match write_back(full.clone(), &groups, &afters, &ran) {
            Ok(merged) => self.dataset = Some(merged),
            Err(error) => {
                self.dataset = Some(full);
                return Err(error);
            }
        }
        let mut out = match split {
            None => results.into_iter().flat_map(|(_, tables)| tables).collect(),
            Some(_) if !layered_split => separate(&split_names, results),
            Some(_) => layered(&split_names, results),
        };
        out.extend(pooled_tables);
        Ok(out)
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

/// The full dataset with the variables the procedure added to any group where it ran, in order
/// of first appearance and defined as in the first group that added them (strings as wide as
/// the widest group's); system-missing (or blank) in the other cases.
fn write_back(
    mut full: Dataset,
    groups: &[Group],
    afters: &[Dataset],
    ran: &[bool],
) -> Result<Dataset, Error> {
    // a group whose procedure failed adds nothing, whatever it left behind
    let afters: Vec<Option<&Dataset>> = afters
        .iter()
        .zip(ran)
        .map(|(a, r)| r.then_some(a))
        .collect();
    let existing = full.variable_count();
    let blank = vec![None; full.case_count()];
    // (name, first group that added it); the definitions go in at once, so the dataset's own
    // case-blind lookup finds a name another group added before
    let mut added: Vec<(String, usize)> = Vec::new();
    for (g, after) in afters.iter().enumerate() {
        let Some(after) = after else { continue };
        // procedures add their variables after the existing ones
        for name in after.variable_names().skip(existing) {
            if full.variable_index(name).is_none() {
                full.copy_variable(after, name, &blank)?;
                added.push((name.to_string(), g));
            }
        }
    }
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
    for (name, owner) in added {
        let source = afters[owner].expect("only groups that ran add variables");
        let columns: Vec<_> = afters
            .iter()
            .map(|a| a.and_then(|a| a.variable(&name).ok()))
            .collect();
        match source.variable(&name)?.values() {
            Values::Numeric(_) => {
                let values = at.iter().map(|place| {
                    let (g, i) = (*place)?;
                    columns[g]?.numeric().ok()?.get(i)
                });
                full.set_numeric_values(&name, values.collect())?;
            }
            Values::String(_) => {
                let width = columns
                    .iter()
                    .flatten()
                    .filter_map(|v| v.string_width())
                    .max()
                    .expect("the defining group saved a string");
                if full.variable(&name)?.string_width() != Some(width) {
                    full.set_string_width(&name, width)?;
                }
                let values = at.iter().map(|place| {
                    let text = |(g, i): (usize, usize)| {
                        Some(columns[g]?.string().ok()?.get(i).to_string())
                    };
                    place.and_then(text).unwrap_or_default()
                });
                full.set_string_values(&name, width, values.collect())?;
            }
        }
    }
    Ok(full)
}

#[cfg(test)]
#[test]
fn saved_variables_are_the_union_of_the_groups_that_ran() {
    let group = |c: usize| Group {
        values: Vec::new(),
        titles: Vec::new(),
        cases: vec![c],
    };
    let part = |strings: &[(&str, usize, &str)], numbers: &[(&str, f64)]| {
        let mut data = Dataset::new(1);
        data.set_numeric_values("x", vec![Some(1.0)]).unwrap();
        for (name, width, text) in strings {
            data.set_string_values(name, *width, vec![text.to_string()])
                .unwrap();
        }
        for (name, value) in numbers {
            data.set_numeric_values(name, vec![Some(*value)]).unwrap();
        }
        data
    };
    let mut full = Dataset::new(4);
    full.set_numeric_values("x", vec![Some(1.0); 4]).unwrap();
    // the first group failed, so what it left is ignored; the second saves a short string, the
    // third the same name in other case, wider, and a number
    let afters = [
        part(&[], &[("z", 9.0), ("n", 7.0)]),
        part(&[("S", 2, "ab")], &[]),
        part(&[("s", 4, "abcd")], &[("n", 5.0)]),
        part(&[], &[]),
    ];
    let groups = [group(0), group(1), group(2), group(3)];
    let ran = [false, true, true, true];
    let out = write_back(full, &groups, &afters, &ran).unwrap();
    assert_eq!(out.variable_names().collect::<Vec<_>>(), ["x", "S", "n"]);
    let s = out.variable("S").unwrap();
    assert_eq!(s.string_width(), Some(4));
    let s = s.string().unwrap();
    assert_eq!(
        [s.get(0), s.get(1), s.get(2), s.get(3)],
        ["", "ab", "abcd", ""]
    );
    let n = out.variable("n").unwrap().numeric().unwrap();
    assert_eq!(
        [n.get(0), n.get(1), n.get(2), n.get(3)],
        [None, None, Some(5.0), None]
    );
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
