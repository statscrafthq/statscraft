//! VARSTOCASES, CASESTOVARS, FLIP and SORT VARIABLES: commands that give the active dataset
//! another shape.
//!
//! Rules, from the descriptions of these commands:
//! - VARSTOCASES and CASESTOVARS use every case: SPLIT FILE (except as the row groups of
//!   CASESTOVARS), WEIGHT and FILTER do not change what they do, and they stay in force while
//!   their variables are in the new file. They cannot follow TEMPORARY.
//! - VARSTOCASES: each case gives one new case per variable of the FROM lists, in their order.
//!   A MAKE variable takes the dictionary entry of the first FROM variable (strings the widest
//!   width). NULL=DROP leaves out a new case whose MAKE values are all system-missing or blank.
//!   ID is F8.0, COUNT F4.0.
//! - CASESTOVARS: a row group is a run of cases with the same split and ID values; cases with
//!   a system-missing or blank ID are left out with a warning. A variable that varies within a
//!   group becomes one variable per index value; one that does not (one value plus
//!   system-missing counts as not varying) is copied. The new variables keep the dictionary
//!   entry of their source, except name and label. VIND variables are F1.0, COUNT is F4.0.
//! - FLIP: variables become cases named in CASE_LBL, cases become F8.2 variables; strings and
//!   user-missing values become system-missing; labels and the weight are gone. NEWNAMES (or
//!   CASE_LBL of an earlier FLIP) names the new variables: upper case, K_ before a number,
//!   other characters a name cannot hold as _, a repeated name with _A, _B, ... Transformations
//!   before FLIP become permanent.
//! - SORT VARIABLES orders the dictionary by one property; the manual's ascending orders.
//!
//! Choices where the manual is silent are marked "choice".

use crate::crosstabs::Key;
use crate::variables::duplicate;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, MissingValues, Value, Values, Variable, is_valid_name};
use statscraft_syntax::{
    CasesToVars, Flip, IndexKind, NewVariable, Subset, VarItem, VariableKey, VarsToCases,
};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// A dataset rebuilt from another. Variables copied as they are keep their names all along;
/// new ones carry a temporary name until `finish`, so they may take the name of a variable
/// that goes. The file's own information (label, documents, attributes) stays, and the weight,
/// split, filter and multiple response sets stay while their variables do.
struct Rebuild {
    data: Dataset,
    /// The variables of the result in order, by their present names.
    order: Vec<String>,
    /// (temporary name, final name)
    renames: Vec<(String, String)>,
}

impl Rebuild {
    /// `cases`: for every new case, the case of `source` the copied variables take their
    /// values from.
    fn new(source: &Dataset, cases: &[Option<usize>]) -> Result<Self, Error> {
        let mut data = source.clone();
        data.take_cases(cases)?;
        Ok(Rebuild {
            data,
            order: Vec::new(),
            renames: Vec::new(),
        })
    }

    fn keep(&mut self, name: &str) {
        self.order.push(name.to_string());
    }

    fn temporary(&mut self, name: &str) -> String {
        let temporary = (self.renames.len()..)
            .map(|i| format!("@sc.restructure.{i}"))
            .find(|t| self.data.variable_index(t).is_none())
            .expect("an unused name");
        self.order.push(temporary.clone());
        self.renames.push((temporary.clone(), name.to_string()));
        temporary
    }

    /// A new variable with the dictionary entry of `template` in `source` and the values of
    /// `source` at `cases`. A label replaces the template's.
    fn copy(
        &mut self,
        source: &Dataset,
        template: &str,
        name: &str,
        cases: &[Option<usize>],
        label: Option<&str>,
    ) -> Result<String, Error> {
        let temporary = self.temporary(name);
        self.data
            .copy_variable_as(source, template, &temporary, cases)?;
        if let Some(label) = label {
            self.data.set_variable_label(&temporary, Some(label))?;
        }
        Ok(temporary)
    }

    fn numeric(
        &mut self,
        variable: &NewVariable,
        values: Vec<Option<f64>>,
        format: &str,
        measure: Measure,
    ) -> Result<(), Error> {
        let name = self.temporary(&variable.name);
        self.data.set_numeric_values(&name, values)?;
        self.data.set_format(&name, format)?;
        self.data.set_measure(&name, measure)?;
        self.data
            .set_variable_label(&name, variable.label.as_deref())?;
        Ok(())
    }

    fn string(
        &mut self,
        variable: &NewVariable,
        width: usize,
        values: Vec<String>,
    ) -> Result<(), Error> {
        let name = self.temporary(&variable.name);
        self.data.set_string_values(&name, width.max(1), values)?;
        self.data
            .set_variable_label(&name, variable.label.as_deref())?;
        Ok(())
    }

    fn finish(mut self, command: &str) -> Result<Dataset, Error> {
        if self.order.is_empty() {
            return Err(Error(format!(
                "{command} would leave no variables; the new file needs at least one"
            )));
        }
        let names: Vec<String> = self
            .order
            .iter()
            .map(|name| match self.renames.iter().find(|(t, _)| t == name) {
                Some((_, last)) => last.clone(),
                None => name.clone(),
            })
            .collect();
        if let Some(name) = duplicate(&names) {
            return Err(Error(format!(
                "{command} would make two variables named {name}; choose another name"
            )));
        }
        let order: Vec<&str> = self.order.iter().map(String::as_str).collect();
        self.data.keep_variables(&order)?;
        for (temporary, name) in &self.renames {
            self.data.rename_variable(temporary, name)?;
        }
        Ok(self.data)
    }
}

fn plain(name: &str) -> NewVariable {
    NewVariable {
        name: name.to_string(),
        label: None,
    }
}

/// A system-missing or blank value.
fn null(values: &Values, case: usize) -> bool {
    match values {
        Values::Numeric(v) => v.get(case).is_none(),
        Values::String(v) => v.get(case).trim_end_matches(' ').is_empty(),
    }
}

/// A key value as a map key: numbers by their bits.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Hashed {
    Number(u64),
    Text(String),
}

fn hashed(key: &Key) -> Hashed {
    match key {
        Key::Number(n) => Hashed::Number(n.to_bits()),
        Key::Text(t) => Hashed::Text(t.trim_end_matches(' ').to_string()),
    }
}

/// Orders combinations of key values, the first value first.
pub(crate) fn compare_keys(a: &[Key], b: &[Key]) -> Ordering {
    a.iter()
        .zip(b)
        .map(|(a, b)| a.compare(b))
        .find(|o| o.is_ne())
        .unwrap_or(Ordering::Equal)
}

/// A key value as text: a whole number without decimals, text without its padding.
fn key_text(key: &Key) -> String {
    match key {
        Key::Number(n) => format!("{n}"),
        Key::Text(t) => t.trim_end_matches(' ').to_string(),
    }
}

/// Whether a made-up name can name a new variable: valid and no scratch or system name.
fn usable(name: &str) -> bool {
    is_valid_name(name) && !name.starts_with(['#', '$'])
}

impl Engine {
    /// The names of `items`, each once.
    fn expand_once(&self, items: &[VarItem]) -> Result<Vec<String>, Error> {
        let mut names = self.expand(items)?;
        let mut seen = HashSet::new();
        names.retain(|name| seen.insert(name.to_lowercase()));
        Ok(names)
    }

    pub(crate) fn sort_variables(
        &mut self,
        key: &VariableKey,
        descending: bool,
    ) -> Result<(), Error> {
        let data = self.active()?;
        let mut order: Vec<usize> = (0..data.variable_count()).collect();
        order.sort_by(|&a, &b| compare_variables(key, &data.variable_at(a), &data.variable_at(b)));
        // like the reference program, D reverses the whole order, ties too
        if descending {
            order.reverse();
        }
        let names: Vec<String> = order
            .iter()
            .map(|&i| data.variable_at(i).name().to_string())
            .collect();
        if !names.is_empty() {
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            self.active_mut()?.keep_variables(&names)?;
        }
        Ok(())
    }

    pub(crate) fn flip(&mut self, flip: &Flip) -> Result<Vec<Table>, Error> {
        self.begin_pass()?;
        // FLIP does not obey TEMPORARY: the temporary changes become permanent
        self.temporary = None;
        let data = self.active()?;
        let mut flipped = match &flip.variables {
            Some(items) => self.expand_once(items)?,
            None => data.variable_names().map(str::to_string).collect(),
        };
        let names_from = match &flip.newnames {
            Some(name) => Some(self.expand(&[VarItem::Name(name.clone())])?.remove(0)),
            None => data
                .variable_index("CASE_LBL")
                .map(|i| data.variable_at(i).name().to_string()),
        };
        flipped.retain(|name| Some(name) != names_from.as_ref());

        let cases = data.case_count();
        let mut taken: HashSet<String> = HashSet::from(["case_lbl".to_string()]);
        let mut new_names = Vec::with_capacity(cases);
        let source = names_from
            .as_ref()
            .map(|name| data.variable(name))
            .transpose()?;
        for case in 0..cases {
            let base = source
                .as_ref()
                .and_then(|variable| flip_name(variable, case))
                .unwrap_or_else(|| format!("VAR{:03}", case + 1));
            let name = unique(&base, &taken);
            taken.insert(name.to_lowercase());
            new_names.push(name);
        }

        let mut out = Rebuild::new(data, &vec![None; flipped.len()])?;
        let width = flipped.iter().map(String::len).max().unwrap_or(0).max(8);
        out.string(&plain("CASE_LBL"), width, flipped.clone())?;
        let columns: Vec<Variable> = flipped
            .iter()
            .map(|name| data.variable(name))
            .collect::<Result<_, _>>()?;
        for (case, name) in new_names.iter().enumerate() {
            let values = columns
                .iter()
                .map(|variable| match variable.values() {
                    Values::Numeric(v) => v.get(case).filter(|v| !variable.is_user_missing(*v)),
                    Values::String(_) => None,
                })
                .collect();
            out.numeric(&plain(name), values, "F8.2", Measure::Unknown)?;
        }
        let out = out.finish("FLIP")?;
        let rows = out
            .variable_names()
            .map(|name| vec![Cell::Text(name.to_string())])
            .collect();
        self.dataset = Some(out);
        Ok(vec![Table {
            title: "Variables of the Transposed File".to_string(),
            row_headers: 0,
            columns: vec!["Variable".to_string()],
            rows,
            ..Table::default()
        }])
    }

    pub(crate) fn vars_to_cases(&mut self, command: &VarsToCases) -> Result<(), Error> {
        self.not_temporary("VARSTOCASES")?;
        self.begin_pass()?;
        let data = self.active()?;
        let lists: Vec<Vec<String>> = command
            .makes
            .iter()
            .map(|(_, from)| self.expand(from))
            .collect::<Result<_, _>>()?;
        let length = lists[0].len();
        for ((make, _), list) in command.makes.iter().zip(&lists) {
            if list.len() != length {
                return Err(Error(format!(
                    "every FROM list needs the same number of variables: MAKE {} has {length}, \
                     MAKE {} has {}",
                    command.makes[0].0.name,
                    make.name,
                    list.len()
                )));
            }
            let string = data.variable(&list[0])?.is_string();
            if let Some(other) = list[1..]
                .iter()
                .find(|n| data.variable(n).is_ok_and(|v| v.is_string() != string))
            {
                return Err(Error(format!(
                    "the FROM list of MAKE {} mixes numeric and string variables ({} and {other})",
                    make.name, list[0]
                )));
            }
        }
        let mut from: HashMap<String, usize> = HashMap::new();
        for (m, list) in lists.iter().enumerate() {
            for name in list {
                if let Some(&other) = from.get(&name.to_lowercase())
                    && other != m
                {
                    return Err(Error(format!(
                        "{name} is on the FROM lists of MAKE {} and MAKE {}; a variable may \
                         serve one MAKE",
                        command.makes[other].0.name, command.makes[m].0.name
                    )));
                }
                from.insert(name.to_lowercase(), m);
            }
        }
        let on_from = |name: &String| from.contains_key(&name.to_lowercase());
        let all: Vec<String> = data.variable_names().map(str::to_string).collect();
        let kept: Vec<String> = match &command.subset {
            None => all.iter().filter(|n| !on_from(n)).cloned().collect(),
            Some(Subset::Keep(items)) | Some(Subset::Drop(items)) => {
                let keep = matches!(command.subset, Some(Subset::Keep(_)));
                let mut named = Vec::new();
                for item in items {
                    match item {
                        // ALL is every variable not on a FROM list
                        VarItem::All => named.extend(all.iter().filter(|n| !on_from(n)).cloned()),
                        _ => {
                            let names = self.expand(std::slice::from_ref(item))?;
                            if let Some(name) = names.iter().find(|n| on_from(n)) {
                                return Err(Error(format!(
                                    "{name} is on a FROM list and cannot be named on KEEP or DROP"
                                )));
                            }
                            named.extend(names);
                        }
                    }
                }
                let mut seen = HashSet::new();
                named.retain(|name| seen.insert(name.to_lowercase()));
                if keep {
                    named
                } else {
                    let dropped: HashSet<String> = named.iter().map(|n| n.to_lowercase()).collect();
                    (all.iter())
                        .filter(|n| !on_from(n) && !dropped.contains(&n.to_lowercase()))
                        .cloned()
                        .collect()
                }
            }
            Some(Subset::Map) => unreachable!("VARSTOCASES has no MAP"),
        };

        // the index values of the j-th new case of a case, and their checks
        let mut index_values: Vec<Vec<Option<f64>>> = Vec::new();
        let mut index_names: Option<(usize, Vec<String>)> = None;
        let levels: Vec<usize> = command
            .index
            .iter()
            .filter_map(|index| match index.kind {
                IndexKind::Levels(n) => Some(n),
                _ => None,
            })
            .collect();
        if !levels.is_empty() && levels.iter().product::<usize>() != length {
            return Err(Error(format!(
                "the INDEX levels ({}) multiply to {}, but the FROM lists have {length} variables",
                levels
                    .iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(" x "),
                levels.iter().product::<usize>()
            )));
        }
        for (k, index) in command.index.iter().enumerate() {
            match &index.kind {
                IndexKind::Sequence => {
                    index_values.push((1..=length).map(|j| Some(j as f64)).collect())
                }
                IndexKind::Levels(n) => {
                    let faster: usize = levels[k + 1..].iter().product();
                    index_values.push(
                        (0..length)
                            .map(|j| Some(((j / faster) % n + 1) as f64))
                            .collect(),
                    );
                }
                IndexKind::Names(make) => {
                    let position = command
                        .makes
                        .iter()
                        .position(|(m, _)| m.name.eq_ignore_ascii_case(make))
                        .ok_or_else(|| {
                            Error(format!(
                                "INDEX {} ({make}): {make} is not the name of a MAKE variable",
                                index.variable.name
                            ))
                        })?;
                    index_names = Some((k, lists[position].clone()));
                    index_values.push(Vec::new());
                }
            }
        }

        let columns: Vec<Vec<Values>> = lists
            .iter()
            .map(|list| {
                list.iter()
                    .map(|n| Ok(data.variable(n)?.values()))
                    .collect()
            })
            .collect::<Result<_, Error>>()?;
        let mut rows: Vec<(usize, usize)> = Vec::new();
        let mut counts = vec![0.0; data.case_count()];
        for (case, count) in counts.iter_mut().enumerate() {
            for j in 0..length {
                let empty = columns.iter().all(|list| null(&list[j], case));
                if command.keep_null || !empty {
                    rows.push((case, j));
                    *count += 1.0;
                }
            }
        }

        let cases: Vec<Option<usize>> = rows.iter().map(|&(case, _)| Some(case)).collect();
        let mut out = Rebuild::new(data, &cases)?;
        if let Some(id) = &command.id {
            let values = rows.iter().map(|&(case, _)| Some(case as f64 + 1.0));
            out.numeric(id, values.collect(), "F8.0", Measure::Nominal)?;
        }
        for name in &kept {
            out.keep(name);
        }
        if let Some(count) = &command.count {
            let values = rows.iter().map(|&(case, _)| Some(counts[case]));
            out.numeric(count, values.collect(), "F4.0", Measure::Scale)?;
        }
        for (k, index) in command.index.iter().enumerate() {
            match &index_names {
                Some((at, names)) if *at == k => {
                    let width = names.iter().map(String::len).max().unwrap_or(1);
                    let values = rows.iter().map(|&(_, j)| names[j].clone()).collect();
                    out.string(&index.variable, width, values)?;
                }
                _ => {
                    let values = rows.iter().map(|&(_, j)| index_values[k][j]).collect();
                    out.numeric(&index.variable, values, "F8.0", Measure::Nominal)?;
                }
            }
        }
        for (((make, _), list), values) in command.makes.iter().zip(&lists).zip(&columns) {
            let name = out.copy(data, &list[0], &make.name, &cases, make.label.as_deref())?;
            match values[0] {
                Values::Numeric(_) => {
                    let column = rows.iter().map(|&(case, j)| match values[j] {
                        Values::Numeric(v) => v.get(case),
                        Values::String(_) => unreachable!("checked: one type per list"),
                    });
                    out.data.set_numeric_values(&name, column.collect())?;
                }
                Values::String(_) => {
                    let width = list
                        .iter()
                        .filter_map(|n| data.variable(n).ok()?.string_width())
                        .max()
                        .unwrap_or(1);
                    let column = rows.iter().map(|&(case, j)| match values[j] {
                        Values::String(v) => v.get(case).to_string(),
                        Values::Numeric(_) => unreachable!("checked: one type per list"),
                    });
                    out.data.set_string_width(&name, width)?;
                    out.data.set_string_values(&name, width, column.collect())?;
                }
            }
        }
        let out = out.finish("VARSTOCASES")?;
        self.dataset = Some(out);
        Ok(())
    }

    pub(crate) fn cases_to_vars(&mut self, command: &CasesToVars) -> Result<(), Error> {
        self.not_temporary("CASESTOVARS")?;
        self.begin_pass()?;
        let data = self.active()?;
        let split: Vec<String> = data
            .split_file()
            .map(|s| s.variables.clone())
            .unwrap_or_default();
        let id = self.expand_once(&command.id)?;
        let index = self.expand_once(&command.index)?;
        let fixed = self.expand_once(&command.fixed)?;
        let dropped = self.expand_once(&command.drop)?;
        let lower = |names: &[String]| -> HashSet<String> {
            names.iter().map(|n| n.to_lowercase()).collect()
        };
        let (split_set, id_set, index_set, fixed_set, drop_set) = (
            lower(&split),
            lower(&id),
            lower(&index),
            lower(&fixed),
            lower(&dropped),
        );
        let clash =
            |a: &HashSet<String>, b: &HashSet<String>, what: &str| match a.intersection(b).next() {
                Some(name) => Err(Error(format!("CASESTOVARS: {name} {what}"))),
                None => Ok(()),
            };
        clash(
            &id_set,
            &fixed_set,
            "is on ID and FIXED; an ID variable cannot be fixed",
        )?;
        clash(
            &id_set,
            &index_set,
            "is on ID and INDEX; an ID variable cannot be an index",
        )?;
        clash(&index_set, &fixed_set, "is on INDEX and FIXED")?;
        clash(
            &index_set,
            &drop_set,
            "is on INDEX and DROP; an index variable cannot be dropped",
        )?;
        clash(&fixed_set, &drop_set, "is on FIXED and DROP")?;
        let mut roots: HashMap<String, String> = HashMap::new();
        for (variable, root) in &command.rename {
            let name = self.expand(&[VarItem::Name(variable.clone())])?.remove(0);
            let lower = name.to_lowercase();
            let sets = [&split_set, &id_set, &fixed_set, &index_set, &drop_set];
            if sets.iter().any(|set| set.contains(&lower)) {
                return Err(Error(format!(
                    "RENAME {name}: only a variable that becomes a variable group can be renamed, \
                     not a split, ID, FIXED, INDEX or DROP variable"
                )));
            }
            if roots.insert(lower, root.clone()).is_some() {
                return Err(Error(format!("RENAME names {name} twice")));
            }
        }

        // the row groups: runs of cases with the same split and ID values
        let mut keys = split.clone();
        keys.extend(
            id.iter()
                .filter(|n| !split_set.contains(&n.to_lowercase()))
                .cloned(),
        );
        let key_columns: Vec<Variable> = keys
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let mut left_out = 0;
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut seen: HashMap<Vec<Hashed>, usize> = HashMap::new();
        let mut previous: Option<Vec<Hashed>> = None;
        for case in 0..data.case_count() {
            if key_columns.iter().any(|v| null(&v.values(), case)) {
                left_out += 1;
                continue;
            }
            let key: Vec<Hashed> = key_columns
                .iter()
                .map(|v| hashed(&value_key(v, case)))
                .collect();
            if previous.as_ref() == Some(&key) {
                groups.last_mut().expect("a group is open").push(case);
                continue;
            }
            if let Some(first) = seen.get(&key) {
                return Err(Error(format!(
                    "the cases are not sorted by {}: case {} continues the group of case {}; \
                     sort them with SORT CASES BY {0} first",
                    keys.join(" "),
                    case + 1,
                    first + 1
                )));
            }
            seen.insert(key.clone(), case);
            previous = Some(key);
            groups.push(vec![case]);
        }
        let mut warnings = Vec::new();
        if left_out > 0 {
            warnings.push(format!(
                "CASESTOVARS left out {left_out} cases with a system-missing or blank value of {}",
                keys.join(", ")
            ));
        }

        // the index values of every case used, and every combination that occurs
        let index_columns: Vec<Variable> = index
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let mut row_index: HashMap<usize, Vec<Key>> = HashMap::new();
        for group in &groups {
            let mut in_group: HashMap<Vec<Hashed>, usize> = HashMap::new();
            for (position, &case) in group.iter().enumerate() {
                let values: Vec<Key> = if index.is_empty() {
                    vec![Key::Number(position as f64 + 1.0)]
                } else {
                    index_columns
                        .iter()
                        .map(|v| index_value(v, case))
                        .collect::<Result<_, _>>()?
                };
                let key: Vec<Hashed> = values.iter().map(hashed).collect();
                if let Some(other) = in_group.insert(key, case) {
                    return Err(Error(format!(
                        "cases {} and {} of one row group have the same index value ({}); \
                         every case of a group needs its own",
                        other + 1,
                        case + 1,
                        values.iter().map(key_text).collect::<Vec<_>>().join(", ")
                    )));
                }
                row_index.insert(case, values);
            }
        }
        let mut combinations: Vec<Vec<Key>> = Vec::new();
        let mut known: HashSet<Vec<Hashed>> = HashSet::new();
        for group in &groups {
            for case in group {
                let values = &row_index[case];
                if known.insert(values.iter().map(hashed).collect()) {
                    combinations.push(values.clone());
                }
            }
        }
        combinations.sort_by(|a, b| compare_keys(a, b));
        let position: HashMap<Vec<Hashed>, usize> = combinations
            .iter()
            .enumerate()
            .map(|(i, values)| (values.iter().map(hashed).collect(), i))
            .collect();
        // `at[group][combination]`: the case of the group with that index value
        let mut at = vec![vec![None; combinations.len()]; groups.len()];
        for (g, group) in groups.iter().enumerate() {
            for &case in group {
                let key: Vec<Hashed> = row_index[&case].iter().map(hashed).collect();
                at[g][position[&key]] = Some(case);
            }
        }

        // fixed variables and variable groups
        let candidates: Vec<String> = data
            .variable_names()
            .filter(|n| {
                let n = n.to_lowercase();
                ![&split_set, &id_set, &index_set, &drop_set]
                    .iter()
                    .any(|set| set.contains(&n))
            })
            .map(str::to_string)
            .collect();
        let mut copied = Vec::new();
        let mut spread = Vec::new();
        let mut not_executed = Vec::new();
        for name in &candidates {
            let variable = data.variable(name)?;
            let constant = !groups.iter().any(|group| varies(&variable, group));
            let listed = fixed_set.contains(&name.to_lowercase());
            match (command.autofix, listed, constant) {
                (true, _, true) | (false, true, true) => copied.push(name.clone()),
                (true, _, false) => {
                    if listed {
                        warnings.push(format!(
                            "FIXED variable {name} varies within row groups; it becomes a \
                             variable group"
                        ));
                    }
                    spread.push(name.clone());
                }
                (false, true, false) => not_executed.push(name.clone()),
                (false, false, true) => {
                    warnings.push(format!(
                        "{name} does not vary within row groups; with AUTOFIX=NO it becomes a \
                         variable group"
                    ));
                    spread.push(name.clone());
                }
                (false, false, false) => spread.push(name.clone()),
            }
        }
        if !not_executed.is_empty() {
            return Err(Error(format!(
                "the FIXED variables {} vary within row groups; with AUTOFIX=NO CASESTOVARS \
                 does not run",
                not_executed.join(", ")
            )));
        }

        let first: Vec<Option<usize>> = groups.iter().map(|g| Some(g[0])).collect();
        let mut out = Rebuild::new(data, &first)?;
        let carried: Vec<String> = data
            .variable_names()
            .filter(|n| {
                let n = n.to_lowercase();
                (split_set.contains(&n) || id_set.contains(&n)) && !drop_set.contains(&n)
                    || copied.iter().any(|c| c.to_lowercase() == n)
            })
            .map(str::to_string)
            .collect();
        for name in &carried {
            out.keep(name);
            if copied.contains(name) {
                // a fixed variable takes the first value of the group that is not missing
                let variable = data.variable(name)?;
                let values = variable.values();
                let cases: Vec<Option<usize>> = groups
                    .iter()
                    .map(|g| Some(*g.iter().find(|&&c| !null(&values, c)).unwrap_or(&g[0])))
                    .collect();
                let mut column = Dataset::new(groups.len());
                column.copy_variable(data, name, &cases)?;
                match column.variable(name)?.values() {
                    Values::Numeric(v) => out.data.set_numeric_values(name, v.iter().collect())?,
                    Values::String(v) => {
                        let width = variable.string_width().expect("a string");
                        let values = v.iter().map(str::to_string).collect();
                        out.data.set_string_values(name, width, values)?
                    }
                }
            }
        }

        // the names of the new variables
        let mut taken: HashSet<String> = carried.iter().map(|n| n.to_lowercase()).collect();
        let mut generic = 0;
        let mut name_for = |wanted: String, taken: &mut HashSet<String>| -> String {
            let name = if usable(&wanted) && !taken.contains(&wanted.to_lowercase()) {
                wanted
            } else {
                loop {
                    generic += 1;
                    let name = format!("V{generic}");
                    if !taken.contains(&name.to_lowercase()) {
                        break name;
                    }
                }
            };
            taken.insert(name.to_lowercase());
            name
        };
        let labels = crate::shown::Shown::labels(self.currencies());
        let separator = &command.separator;
        let string_index = index.len() == 1 && index_columns[0].is_string();
        let parts = |values: &[Key]| -> Vec<String> { values.iter().map(key_text).collect() };
        let describe = |values: &[Key]| -> String {
            if index.is_empty() {
                return key_text(&values[0]);
            }
            let pairs = index_columns
                .iter()
                .zip(values)
                .map(|(v, key)| format!("{} = {}", v.name(), labels.value(v, key)));
            pairs.collect::<Vec<_>>().join(", ")
        };
        let mut names: HashMap<(usize, usize), String> = HashMap::new();
        for (s, source) in spread.iter().enumerate() {
            let root = roots
                .get(&source.to_lowercase())
                .cloned()
                .unwrap_or_else(|| source.clone());
            for (k, values) in combinations.iter().enumerate() {
                // choice: one string index names the variables by its values when one variable
                // is spread, as the manual says; with several the values follow root and separator
                let wanted = if string_index && spread.len() == 1 {
                    key_text(&values[0])
                } else {
                    let mut name = root.clone();
                    for part in parts(values) {
                        name.push_str(separator);
                        name.push_str(&part);
                    }
                    name
                };
                names.insert((s, k), name_for(wanted, &mut taken));
            }
        }
        let mut order: Vec<(usize, usize)> = Vec::new();
        if command.group_by_index {
            for k in 0..combinations.len() {
                order.extend((0..spread.len()).map(|s| (s, k)));
            }
        } else {
            for s in 0..spread.len() {
                order.extend((0..combinations.len()).map(|k| (s, k)));
            }
        }
        for (s, k) in order {
            let source = data.variable(&spread[s])?;
            // choice: the label names the index values after the source's label or name
            let label = format!(
                "{}, {}",
                source.label().unwrap_or(source.name()),
                describe(&combinations[k])
            );
            let cases: Vec<Option<usize>> = at.iter().map(|row| row[k]).collect();
            out.copy(data, &spread[s], &names[&(s, k)], &cases, Some(&label))?;
        }
        if let Some(root) = &command.indicators {
            for (k, values) in combinations.iter().enumerate() {
                let name = name_for(
                    format!("{root}{}", parts(values).join(separator)),
                    &mut taken,
                );
                let variable = NewVariable {
                    name,
                    label: Some(describe(values)),
                };
                let values = at
                    .iter()
                    .map(|row| Some(if row[k].is_some() { 1.0 } else { 0.0 }));
                out.numeric(&variable, values.collect(), "F1.0", Measure::Nominal)?;
            }
        }
        if let Some(count) = &command.count {
            let values = groups.iter().map(|g| Some(g.len() as f64)).collect();
            out.numeric(count, values, "F4.0", Measure::Scale)?;
        }
        self.dataset = Some(out.finish("CASESTOVARS")?);
        self.warnings.extend(warnings);
        Ok(())
    }
}

/// The value of a key variable at a case that is not null.
fn value_key(variable: &Variable, case: usize) -> Key {
    match variable.values() {
        Values::Numeric(v) => Key::Number(v.get(case).expect("not system-missing")),
        Values::String(v) => Key::Text(v.get(case).trim_end_matches(' ').to_string()),
    }
}

/// The value of an index variable: a whole number from 0 up, or text that is not blank.
fn index_value(variable: &Variable, case: usize) -> Result<Key, Error> {
    let bad = |what: &str| {
        Err(Error(format!(
            "INDEX variable {} has {what} in case {}; an index needs whole numbers from 0 up \
             or text that is not blank",
            variable.name(),
            case + 1
        )))
    };
    match variable.values() {
        Values::Numeric(v) => match v.get(case) {
            None => bad("the system-missing value"),
            Some(n) if n < 0.0 || n.fract() != 0.0 => bad(&format!("the value {n}")),
            Some(n) => Ok(Key::Number(n)),
        },
        Values::String(v) if v.get(case).trim_end_matches(' ').is_empty() => bad("a blank value"),
        Values::String(v) => Ok(Key::Text(v.get(case).trim_end_matches(' ').to_string())),
    }
}

/// Whether a variable takes two different values in the cases of `group` that are not
/// system-missing or blank.
fn varies(variable: &Variable, group: &[usize]) -> bool {
    let values = variable.values();
    let mut first: Option<Key> = None;
    for &case in group.iter().filter(|&&c| !null(&values, c)) {
        let key = value_key(variable, case);
        match &first {
            Some(seen) if *seen != key => return true,
            Some(_) => {}
            None => first = Some(key),
        }
    }
    false
}

/// The name FLIP makes of a value of the NEWNAMES variable; None for a missing or blank value.
fn flip_name(variable: &Variable, case: usize) -> Option<String> {
    let text = match variable.values() {
        Values::Numeric(v) => {
            let n = v.get(case)?;
            format!("K_{n}")
        }
        Values::String(v) => {
            let text = v.get(case).trim().to_uppercase();
            if text.is_empty() {
                return None;
            }
            text
        }
    };
    let mut name: String = text
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || "_.$#@".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    // choice: a name must start with a letter; K_ goes before one that does not, as before a number
    if !name.starts_with(char::is_alphabetic) {
        name.insert_str(0, "K_");
    }
    if name.ends_with('.') {
        name.pop();
        name.push('_');
    }
    Some(cut(&name, 64).to_string())
}

/// At most `bytes` bytes of `text`, cut at a character boundary.
fn cut(text: &str, bytes: usize) -> &str {
    let mut end = text.len().min(bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// `base`, or base with _A, _B, ... _Z, _AA, ... when it is taken (ignoring case) or not
/// usable as a name.
fn unique(base: &str, taken: &HashSet<String>) -> String {
    if usable(base) && !taken.contains(&base.to_lowercase()) {
        return base.to_string();
    }
    (1..)
        .map(|n| {
            let mut suffix = String::new();
            let mut n: usize = n;
            while n > 0 {
                n -= 1;
                suffix.insert(0, (b'A' + (n % 26) as u8) as char);
                n /= 26;
            }
            format!("{}_{suffix}", cut(base, 63 - suffix.len()))
        })
        .find(|name| usable(name) && !taken.contains(&name.to_lowercase()))
        .expect("a free name")
}

/// The type codes of the data file format, in code order: FORMAT sorts by them.
const FORMAT_CODES: [&str; 37] = [
    "A", "AHEX", "COMMA", "DOLLAR", "F", "IB", "PIBHEX", "P", "PIB", "PK", "RB", "RBHEX", "Z", "N",
    "E", "DATE", "TIME", "DATETIME", "ADATE", "JDATE", "DTIME", "WKDAY", "MONTH", "MOYR", "QYR",
    "WKYR", "PCT", "DOT", "CCA", "CCB", "CCC", "CCD", "CCE", "EDATE", "SDATE", "MTIME", "YMDHMS",
];

const ROLES: [&str; 6] = ["input", "target", "both", "none", "partition", "split"];

fn compare_variables(key: &VariableKey, a: &Variable, b: &Variable) -> Ordering {
    match key {
        VariableKey::Name => name_order(a.name(), b.name()),
        VariableKey::Type => a.string_width().cmp(&b.string_width()),
        VariableKey::Format => format_rank(a.format()).cmp(&format_rank(b.format())),
        VariableKey::Label => {
            let label = |v: &Variable| v.label().map(str::to_lowercase);
            label(a).cmp(&label(b))
        }
        VariableKey::Values => compare_labels(a.value_labels(), b.value_labels()),
        VariableKey::Missing => compare_missing(a.missing_values(), b.missing_values()),
        VariableKey::Measure => {
            let rank = |v: &Variable| match v.measure() {
                Measure::Unknown => 0,
                Measure::Nominal => 1,
                Measure::Ordinal => 2,
                Measure::Scale => 3,
            };
            rank(a).cmp(&rank(b))
        }
        VariableKey::Role => {
            let rank = |v: &Variable| ROLES.iter().position(|r| Some(*r) == v.role());
            rank(a).unwrap_or(0).cmp(&rank(b).unwrap_or(0))
        }
        VariableKey::Columns => {
            // without a width of its own: 8 for a number, the width up to 32 for a string
            let columns = |v: &Variable| {
                v.display_width()
                    .unwrap_or_else(|| v.string_width().map_or(8, |w| w.min(32) as u32))
            };
            columns(a).cmp(&columns(b))
        }
        VariableKey::Alignment => {
            let rank = |v: &Variable| match v.alignment().unwrap_or(if v.is_string() {
                "left"
            } else {
                "right"
            }) {
                "left" => 0,
                "right" => 1,
                _ => 2,
            };
            rank(a).cmp(&rank(b))
        }
        VariableKey::Attribute(name) => {
            let values = |v: &Variable| {
                let found = v
                    .attributes()
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(name));
                found.map(|(_, values)| values.iter().map(|s| s.to_lowercase()).collect::<Vec<_>>())
            };
            values(a).cmp(&values(b))
        }
    }
}

/// Letters without regard to case, then a number at the end by its value: v2 before v10.
fn name_order(a: &str, b: &str) -> Ordering {
    let split = |name: &str| {
        let stem = name.trim_end_matches(|c: char| c.is_ascii_digit());
        let number = name[stem.len()..].trim_start_matches('0').to_string();
        (stem.to_lowercase(), number)
    };
    let ((stem_a, number_a), (stem_b, number_b)) = (split(a), split(b));
    stem_a
        .cmp(&stem_b)
        .then(number_a.len().cmp(&number_b.len()))
        .then(number_a.cmp(&number_b))
}

/// Format type in code order, then width and decimals.
fn format_rank(format: &str) -> (usize, u32, u32) {
    let upper = format.to_uppercase();
    let kind = upper.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    let rest = &upper[kind.len()..];
    let (width, decimals) = rest.split_once('.').unwrap_or((rest, "0"));
    let code = FORMAT_CODES
        .iter()
        .position(|c| *c == kind)
        .unwrap_or(FORMAT_CODES.len());
    (
        code,
        width.parse().unwrap_or(0),
        decimals.parse().unwrap_or(0),
    )
}

fn compare_values(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.total_cmp(y),
        (Value::String(x), Value::String(y)) => x.cmp(y),
        (Value::Number(_), Value::String(_)) => Ordering::Less,
        (Value::String(_), Value::Number(_)) => Ordering::Greater,
    }
}

/// Variables without value labels first; then the labels in value order, so that equal sets
/// of labels sit together.
fn compare_labels(a: &[(Value, String)], b: &[(Value, String)]) -> Ordering {
    let sorted = |labels: &[(Value, String)]| {
        let mut labels: Vec<(Value, String)> = labels
            .iter()
            .map(|(v, l)| (v.clone(), l.to_lowercase()))
            .collect();
        labels.sort_by(|x, y| compare_values(&x.0, &y.0));
        labels
    };
    let (a, b) = (sorted(a), sorted(b));
    a.is_empty().cmp(&b.is_empty()).reverse().then_with(|| {
        for (x, y) in a.iter().zip(&b) {
            let ordering = compare_values(&x.0, &y.0).then_with(|| x.1.cmp(&y.1));
            if ordering.is_ne() {
                return ordering;
            }
        }
        a.len().cmp(&b.len())
    })
}

/// Variables without missing values first; then discrete numbers, ranges and strings, so that
/// equal definitions sit together.
fn compare_missing(a: &MissingValues, b: &MissingValues) -> Ordering {
    let rank = |m: &MissingValues| -> (u8, Vec<f64>, Vec<String>) {
        match m {
            MissingValues::None => (0, Vec::new(), Vec::new()),
            MissingValues::Numbers(numbers) => {
                let mut numbers = numbers.clone();
                numbers.sort_by(f64::total_cmp);
                (1, numbers, Vec::new())
            }
            MissingValues::Range { low, high, extra } => {
                let mut numbers = vec![*low, *high];
                numbers.extend(extra);
                (2, numbers, Vec::new())
            }
            MissingValues::Strings(strings) => {
                let mut strings = strings.clone();
                strings.sort();
                (3, Vec::new(), strings)
            }
        }
    };
    let ((kind_a, numbers_a, strings_a), (kind_b, numbers_b, strings_b)) = (rank(a), rank(b));
    kind_a
        .cmp(&kind_b)
        .then_with(|| {
            let pairs = numbers_a.iter().zip(&numbers_b);
            let first = pairs.map(|(x, y)| x.total_cmp(y)).find(|o| o.is_ne());
            first.unwrap_or_else(|| numbers_a.len().cmp(&numbers_b.len()))
        })
        .then_with(|| strings_a.cmp(&strings_b))
}
