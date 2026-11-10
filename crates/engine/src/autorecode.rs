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
        let group = command.group && variables.len() > 1;
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
            olds
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
                    .position(|old| old.key == *key)
                    .map(|i| i as f64 + 1.0)
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
                .enumerate()
                .filter_map(|(i, old)| {
                    let owner = if group {
                        variables
                            .iter()
                            .find(|v| has_value(v, &old.key))
                            .unwrap_or(variable)
                    } else {
                        variable
                    };
                    let label = label_of(owner, &old.key);
                    (!label.is_empty()).then_some((Value::Number(i as f64 + 1.0), label))
                })
                .collect();
            let missing_codes: Vec<f64> = scheme
                .iter()
                .enumerate()
                .filter(|(_, old)| old.missing)
                .map(|(i, _)| i as f64 + 1.0)
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
                let rows = scheme.iter().enumerate().map(|(i, old)| {
                    let shown = match &old.key {
                        Key::Number(n) => format_value(*n, variable.format(), false).trim().into(),
                        Key::Text(t) => t.clone(),
                    };
                    let label = labels
                        .iter()
                        .find(|(v, _)| *v == Value::Number(i as f64 + 1.0))
                        .map_or(String::new(), |(_, l)| l.clone());
                    vec![
                        Cell::Text(shown),
                        Cell::Number(i as f64 + 1.0),
                        Cell::Text(label),
                    ]
                });
                tables.push(Table {
                    title: format!("Recoding {} into {}", variable.name(), targets[at]),
                    row_headers: 1,
                    columns: vec!["Old Value".into(), "New Value".into(), "Value Label".into()],
                    rows: rows.collect(),
                    ..Table::default()
                });
            }
            let width = scheme.len().max(1).to_string().len();
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
        Ok(tables)
    }
}

fn has_value(variable: &Variable, key: &Key) -> bool {
    match (variable.values(), key) {
        (Values::Numeric(values), Key::Number(n)) => values.iter().flatten().any(|v| v == *n),
        (Values::String(values), Key::Text(t)) => values.iter().any(|v| v == t),
        _ => false,
    }
}
