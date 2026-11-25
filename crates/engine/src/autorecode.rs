//! AUTORECODE: the values of variables as consecutive integers in new variables.
//!
//! Rules, from the command's description:
//! - The valid values of a variable are sorted (ascending, or with DESCENDING descending) and
//!   numbered from 1. User-missing values follow, numbered after the valid ones in the same
//!   order, and the new codes are user-missing. System-missing stays system-missing.
//! - A blank string is a valid value, or with BLANK=MISSING a user-missing one.
//! - GROUP makes one scheme for all the variables (of one type): user-missing are only the
//!   values the first variable with missing values defines as such.
//! - The new variable takes the variable label of the old one; each code gets the value label
//!   of its old value, or the old value as shown by its format.
//! - AUTORECODE reads the data: it ignores the split, and SELECT IF before it has taken effect.
//! - SAVE TEMPLATE writes the scheme of the first variable (the common one with GROUP or APPLY
//!   TEMPLATE), its valid values only, as a data file of Source_ and Target_; its extension is
//!   .sat unless the name has one. APPLY TEMPLATE keeps the codes of the template for its
//!   values and gives the other valid values the codes above the highest, in the order of the
//!   command, then the user-missing ones; several variables share the scheme.

use crate::crosstabs::Key;
use crate::{Cell, Engine, Error, Table, format_value};
use statscraft_data::{Measure, MissingValues, Value, Values, Variable};
use statscraft_syntax::Autorecode;
use std::cmp::Ordering;

/// One old value with whether it is missing.
#[derive(Clone)]
struct Old {
    key: Key,
    missing: bool,
    /// The new value; set when the scheme is made.
    code: f64,
}

/// Whether two values are the same, a string without regard to trailing blanks.
fn same(a: &Key, b: &Key) -> bool {
    match (a, b) {
        (Key::Text(x), Key::Text(y)) => x.trim_end() == y.trim_end(),
        _ => a == b,
    }
}

/// The name of a template file: .sat unless the name has an extension.
fn template_name(file: &str) -> String {
    match std::path::Path::new(file).extension() {
        Some(_) => file.to_string(),
        None => format!("{file}.sat"),
    }
}

fn compare(a: &Key, b: &Key) -> Ordering {
    a.compare(b)
}

/// The value label of an old value, else the value as its format shows it.
fn label_of(variable: &Variable, key: &Key) -> String {
    let labelled = variable
        .value_labels()
        .iter()
        .find(|(value, _)| match (value, key) {
            (Value::Number(a), Key::Number(b)) => a == b,
            (Value::String(a), Key::Text(b)) => a == b,
            _ => false,
        });
    match (labelled, key) {
        (Some((_, label)), _) => label.clone(),
        // the value in its format, right-aligned in its width
        (None, Key::Number(n)) => {
            let format = variable.format();
            let size = format.trim_start_matches(|c: char| c.is_ascii_alphabetic());
            let width: usize = size
                .split('.')
                .next()
                .and_then(|w| w.parse().ok())
                .unwrap_or(0);
            format!("{:>width$}", format_value(*n, format, false))
        }
        (None, Key::Text(text)) => text.clone(),
    }
}

impl Engine {
    pub(crate) fn autorecode(&mut self, command: &Autorecode) -> Result<Vec<Table>, Error> {
        let sources = self.expand(&command.variables)?;
        let targets = crate::variables::new_names(&command.into)?;
        if sources.len() != targets.len() {
            return Err(Error(format!(
                "{} variables to recode but {} after INTO; give one new name per variable",
                sources.len(),
                targets.len()
            )));
        }
        if let Some(name) = crate::variables::duplicate(&targets) {
            return Err(Error(format!("the new name {name} is given twice")));
        }
        let data = self.active()?;
        for target in &targets {
            if data.variable_index(target).is_some() {
                return Err(Error(format!(
                    "{target} already exists: AUTORECODE needs new variables after INTO"
                )));
            }
        }
        let variables: Vec<Variable> = sources
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let strings = variables.iter().any(Variable::is_string);
        if command.apply.is_some() && variables.iter().any(|v| v.is_string() != strings) {
            return Err(Error(
                "APPLY TEMPLATE needs variables of one type: all numeric or all strings".into(),
            ));
        }
        let template = match &command.apply {
            Some(file) => Some(self.read_template(file, strings)?),
            None => None,
        };
        // several variables share the scheme of a template
        let group = (command.group || template.is_some()) && variables.len() > 1;
        if group
            && variables
                .iter()
                .any(|v| v.is_string() != variables[0].is_string())
        {
            return Err(Error(
                "GROUP needs variables of one type: all numeric or all strings".into(),
            ));
        }
        // with GROUP the missing values of the first variable that has any count for all
        let definer = variables
            .iter()
            .position(|v| *v.missing_values() != MissingValues::None);
        let blank_missing = command.blank_missing;
        let is_missing = |at: usize, key: &Key| -> bool {
            let owner = if group { definer } else { Some(at) };
            match key {
                Key::Text(t) if t.is_empty() && blank_missing => true,
                Key::Number(n) => owner.is_some_and(|o| variables[o].is_user_missing(*n)),
                Key::Text(t) => owner.is_some_and(|o| variables[o].is_user_missing_str(t)),
            }
        };
        let values_of = |at: usize| -> Vec<Old> {
            let variable = &variables[at];
            let keys: Vec<Key> = match variable.values() {
                Values::Numeric(values) => values.iter().flatten().map(Key::Number).collect(),
                Values::String(values) => values.iter().map(|v| Key::Text(v.into())).collect(),
            };
            keys.into_iter()
                .map(|key| Old {
                    missing: is_missing(at, &key),
                    key,
                    code: 0.0,
                })
                .collect()
        };
        let scheme = |olds: Vec<Old>| -> Vec<Old> {
            let mut olds = olds;
            olds.sort_by(|a, b| {
                let order = a.missing.cmp(&b.missing);
                let by_value = match command.descending {
                    true => compare(&b.key, &a.key),
                    false => compare(&a.key, &b.key),
                };
                order.then(by_value)
            });
            olds.dedup_by(|a, b| a.key == b.key);
            match &template {
                None => {
                    for (i, old) in olds.iter_mut().enumerate() {
                        old.code = i as f64 + 1.0;
                    }
                    olds
                }
                Some(pairs) => {
                    let mut all: Vec<Old> = pairs
                        .iter()
                        .map(|(key, code)| Old {
                            key: key.clone(),
                            missing: false,
                            code: *code,
                        })
                        .collect();
                    let mut next = all.iter().map(|o| o.code).fold(0.0, f64::max);
                    for mut old in olds {
                        if !pairs.iter().any(|(key, _)| same(key, &old.key)) {
                            next += 1.0;
                            old.code = next;
                            all.push(old);
                        }
                    }
                    all
                }
            }
        };
        let schemes: Vec<Vec<Old>> = if group {
            let all = (0..variables.len()).flat_map(values_of).collect();
            vec![scheme(all); variables.len()]
        } else {
            (0..variables.len())
                .map(|at| scheme(values_of(at)))
                .collect()
        };

        // the new columns, value labels and missing values, before anything changes
        let mut columns = Vec::new();
        let mut tables = Vec::new();
        for (at, variable) in variables.iter().enumerate() {
            let scheme = &schemes[at];
            let code = |key: &Key| -> Option<f64> {
                scheme
                    .iter()
                    .find(|old| same(&old.key, key))
                    .map(|old| old.code)
            };
            let values: Vec<Option<f64>> = match variable.values() {
                Values::Numeric(values) => values.iter().map(|v| code(&Key::Number(v?))).collect(),
                Values::String(values) => {
                    values.iter().map(|v| code(&Key::Text(v.into()))).collect()
                }
            };
            // each code is labelled by its value in the variable that has it, the first first
            let labels: Vec<(Value, String)> = scheme
                .iter()
                .filter_map(|old| {
                    let owner = if group {
                        variables
                            .iter()
                            .find(|v| has_value(v, &old.key))
                            .unwrap_or(variable)
                    } else {
                        variable
                    };
                    let label = label_of(owner, &old.key);
                    (!label.is_empty()).then_some((Value::Number(old.code), label))
                })
                .collect();
            let missing_codes: Vec<f64> = scheme
                .iter()
                .filter(|old| old.missing)
                .map(|old| old.code)
                .collect();
            let missing = match missing_codes.as_slice() {
                [] => MissingValues::None,
                codes if codes.len() <= 3 => MissingValues::Numbers(codes.to_vec()),
                [first, .., last] => MissingValues::Range {
                    low: *first,
                    high: *last,
                    extra: None,
                },
                _ => unreachable!("more than three codes"),
            };
            if command.print {
                let rows = scheme.iter().map(|old| {
                    let shown = match &old.key {
                        Key::Number(n) => format_value(*n, variable.format(), false).trim().into(),
                        Key::Text(t) => t.clone(),
                    };
                    let label = labels
                        .iter()
                        .find(|(v, _)| *v == Value::Number(old.code))
                        .map_or(String::new(), |(_, l)| l.clone());
                    vec![Cell::Text(shown), Cell::Number(old.code), Cell::Text(label)]
                });
                tables.push(Table {
                    title: format!("Recoding {} into {}", variable.name(), targets[at]),
                    row_headers: 1,
                    columns: vec!["Old Value".into(), "New Value".into(), "Value Label".into()],
                    rows: rows.collect(),
                    ..Table::default()
                });
            }
            let highest = scheme.iter().map(|old| old.code).fold(1.0, f64::max);
            let width = (highest as u64).to_string().len();
            columns.push((
                values,
                labels,
                missing,
                variable.label().map(str::to_string),
                width,
            ));
        }
        let data = self.active_mut()?;
        for (target, (values, labels, missing, label, width)) in targets.iter().zip(columns) {
            data.set_numeric_values(target, values)?;
            data.set_format(target, &format!("F{width}.0"))?;
            data.set_value_labels(target, labels)?;
            data.set_missing_values(target, missing)?;
            data.set_variable_label(target, label.as_deref())?;
            data.set_measure(target, Measure::Nominal)?;
        }
        // the template is written once the recoding has worked
        if let Some(file) = &command.save {
            self.save_template(file, &schemes[0], strings)?;
        }
        Ok(tables)
    }
}

impl Engine {
    /// The values and codes of a template file: the variables Source_ and Target_.
    fn read_template(&self, file: &str, strings: bool) -> Result<Vec<(Key, f64)>, Error> {
        let fail = |message: String| Error(format!("AUTORECODE APPLY TEMPLATE {file}: {message}"));
        let data = self
            .data_file(&template_name(file))
            .map_err(|e| fail(e.0))?;
        let find = |name: &str| {
            data.variables()
                .find(|v| v.name().eq_ignore_ascii_case(name))
                .ok_or_else(|| fail(format!("the file has no variable {name}")))
        };
        let (source, target) = (find("Source_")?, find("Target_")?);
        if source.is_string() != strings {
            return Err(fail(format!(
                "the template holds {} but the variables to recode are {}",
                if source.is_string() {
                    "strings"
                } else {
                    "numbers"
                },
                if strings { "strings" } else { "numbers" }
            )));
        }
        let Values::Numeric(codes) = target.values() else {
            return Err(fail("Target_ is a string variable".into()));
        };
        let keys: Vec<Option<Key>> = match source.values() {
            Values::Numeric(values) => values.iter().map(|v| v.map(Key::Number)).collect(),
            Values::String(values) => values
                .iter()
                .map(|v| Some(Key::Text(v.trim_end().into())))
                .collect(),
        };
        Ok(keys
            .into_iter()
            .enumerate()
            .filter_map(|(k, key)| Some((key?, codes.get(k)?)))
            .collect())
    }

    /// SAVE TEMPLATE: the valid values of `scheme` with their codes.
    fn save_template(&mut self, file: &str, scheme: &[Old], strings: bool) -> Result<(), Error> {
        let valid: Vec<&Old> = scheme.iter().filter(|old| !old.missing).collect();
        let mut data = statscraft_data::Dataset::new(valid.len());
        if strings {
            let texts: Vec<String> = valid
                .iter()
                .map(|old| match &old.key {
                    Key::Text(t) => t.clone(),
                    Key::Number(_) => String::new(),
                })
                .collect();
            let width = texts.iter().map(String::len).max().unwrap_or(1).max(1);
            data.set_string_values("Source_", width, texts)?;
        } else {
            let numbers = valid
                .iter()
                .map(|old| match old.key {
                    Key::Number(n) => Some(n),
                    Key::Text(_) => None,
                })
                .collect();
            data.set_numeric_values("Source_", numbers)?;
        }
        data.set_numeric_values("Target_", valid.iter().map(|old| Some(old.code)).collect())?;
        data.set_format("Target_", "F8.0")?;
        self.write_data_file(&template_name(file), data)
    }
}

fn has_value(variable: &Variable, key: &Key) -> bool {
    match (variable.values(), key) {
        (Values::Numeric(values), Key::Number(n)) => values.iter().flatten().any(|v| v == *n),
        (Values::String(values), Key::Text(t)) => values.iter().any(|v| v == t),
        _ => false,
    }
}
