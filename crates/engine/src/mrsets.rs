//! MRSETS: defining, deleting and listing multiple response sets.

use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, MultipleResponseSet, Value as StoredValue};
use statscraft_syntax::{Mrsets, MrsetsGroup, Value};

impl Engine {
    /// Runs the subcommands in order; when one fails, the dataset is left as it was.
    pub(crate) fn mrsets(&mut self, subcommands: &[Mrsets]) -> Result<Vec<Table>, Error> {
        let mut tables = Vec::new();
        self.all_or_nothing(|engine| {
            for subcommand in subcommands {
                match subcommand {
                    Mrsets::Define(group) => engine.define_set(group)?,
                    Mrsets::Delete(names) => {
                        let data = engine.active_mut()?;
                        let names = names.clone().unwrap_or_else(|| all_names(data));
                        let names: Vec<&str> = names.iter().map(String::as_str).collect();
                        data.delete_multiple_response_sets(&names)
                            .map_err(|e| Error(format!("/DELETE: {e}")))?;
                    }
                    Mrsets::Display(names) => tables.push(display(engine.active()?, names)?),
                }
            }
            Ok(())
        })?;
        Ok(tables)
    }

    fn define_set(&mut self, group: &MrsetsGroup) -> Result<(), Error> {
        let subcommand = if group.value.is_some() {
            "MDGROUP"
        } else {
            "MCGROUP"
        };
        let fail = |message: String| Error(format!("/{subcommand}: {message}"));
        let variables = self.expand(&group.variables).map_err(|Error(m)| fail(m))?;
        let string = self.active()?.variable(&variables[0])?.is_string();
        let counted_value = match (&group.value, string) {
            (None, _) => None,
            (Some(Value::Number(n)), false) => Some(n.to_string()),
            (Some(Value::String(s)), true) => Some(s.clone()),
            (Some(Value::Number(_)), true) => {
                return Err(fail(format!(
                    "{} is a string variable, so VALUE is a quoted text",
                    variables[0]
                )));
            }
            (Some(Value::String(_)), false) => {
                return Err(fail(format!(
                    "{} is numeric, so VALUE is a number, not a quoted text",
                    variables[0]
                )));
            }
        };
        // the manual's label warnings concern categories labelled by variable labels
        let warnings = if group.counted_value_labels {
            Vec::new()
        } else {
            label_conflicts(self.active()?, &variables, group.value.as_ref())?
        };
        for warning in warnings {
            self.warn(format!("/{subcommand} {}: {warning}", group.name));
        }
        let data = self.active()?;
        if group.counted_value_labels {
            // every counted value needs a value label, and no two may be the same
            let mut seen: Vec<String> = Vec::new();
            for name in &variables {
                let variable = data.variable(name)?;
                let label = variable.value_labels().iter().find_map(|(value, label)| {
                    let counted = match (value, &group.value) {
                        (StoredValue::Number(v), Some(Value::Number(n))) => v == n,
                        (StoredValue::String(v), Some(Value::String(s))) => {
                            v == s.trim_end_matches(' ')
                        }
                        _ => false,
                    };
                    counted.then_some(label)
                });
                let Some(label) = label else {
                    return Err(fail(format!(
                        "CATEGORYLABELS=COUNTEDVALUES needs a value label for the counted value in {name}"
                    )));
                };
                if seen
                    .iter()
                    .any(|s| s.to_lowercase() == label.to_lowercase())
                {
                    return Err(fail(format!(
                        "CATEGORYLABELS=COUNTEDVALUES needs a different label for the counted value in every variable; {name} repeats '{label}'"
                    )));
                }
                seen.push(label.clone());
            }
        }
        let label = if group.label_from_variables {
            // the first variable label of the set, else the name of the first variable
            let labelled = variables
                .iter()
                .find_map(|name| data.variable(name).ok()?.label().map(str::to_string));
            labelled.unwrap_or_else(|| variables[0].clone())
        } else {
            group.label.clone().unwrap_or_default()
        };
        let set = MultipleResponseSet {
            name: group.name.clone(),
            kind: if group.value.is_some() {
                "dichotomy"
            } else {
                "category"
            },
            label,
            counted_value,
            variables,
            counted_value_labels: group.counted_value_labels,
            label_from_variables: group.label_from_variables,
        };
        self.active_mut()?
            .set_multiple_response_set(set)
            .map_err(|e| fail(e.to_string()))
    }
}

/// Labels that make the categories of a set ambiguous, compared ignoring case: for a dichotomy
/// set two variables with the same variable label, or different value labels of the counted
/// value; for a category set different value labels of one value.
fn label_conflicts(
    data: &Dataset,
    variables: &[String],
    counted: Option<&Value>,
) -> Result<Vec<String>, Error> {
    let mut warnings = Vec::new();
    let mut seen: Vec<(StoredValue, String, &str)> = Vec::new();
    let mut labels: Vec<(String, &str)> = Vec::new();
    for name in variables {
        let variable = data.variable(name)?;
        if counted.is_some()
            && let Some(label) = variable.label()
        {
            match labels.iter().find(|(l, _)| l.eq_ignore_ascii_case(label)) {
                Some((_, first)) => warnings.push(format!(
                    "{first} and {name} have the same variable label, so their categories look alike"
                )),
                None => labels.push((label.to_string(), name)),
            }
        }
        for (value, label) in variable.value_labels() {
            let counted_here = match (counted, value) {
                (None, _) => true,
                (Some(Value::Number(n)), StoredValue::Number(v)) => n == v,
                (Some(Value::String(s)), StoredValue::String(v)) => s.trim_end() == v.trim_end(),
                _ => false,
            };
            if !counted_here {
                continue;
            }
            match seen.iter().find(|(v, _, _)| v == value) {
                Some((_, first_label, first)) if !first_label.eq_ignore_ascii_case(label) => {
                    warnings.push(format!(
                        "{first} and {name} label the value {} differently ('{first_label}' and '{label}')",
                        show(value)
                    ))
                }
                Some(_) => {}
                None => seen.push((value.clone(), label.clone(), name)),
            }
        }
    }
    Ok(warnings)
}

fn show(value: &StoredValue) -> String {
    match value {
        StoredValue::Number(n) => n.to_string(),
        StoredValue::String(s) => format!("'{}'", s.trim_end()),
    }
}

fn all_names(data: &Dataset) -> Vec<String> {
    data.multiple_response_sets()
        .iter()
        .map(|set| set.name.clone())
        .collect()
}

/// One row per set: name, label, kind, counted value and variables.
fn display(data: &Dataset, names: &Option<Vec<String>>) -> Result<Table, Error> {
    let sets = data.multiple_response_sets();
    let names = names.clone().unwrap_or_else(|| all_names(data));
    let mut rows = Vec::new();
    for name in &names {
        let set = sets
            .iter()
            .find(|set| set.name.eq_ignore_ascii_case(name))
            .ok_or_else(|| Error(format!("/DISPLAY: unknown multiple response set {name}")))?;
        let text = |s: &str| Cell::Text(s.to_string());
        rows.push(vec![
            text(&set.name),
            text(&set.label),
            text(if set.kind == "dichotomy" {
                "Multiple dichotomy"
            } else {
                "Multiple category"
            }),
            set.counted_value.as_deref().map_or(Cell::Empty, text),
            text(&set.variables.join(" ")),
        ]);
    }
    Ok(Table {
        title: "Multiple Response Sets".into(),
        row_headers: 1,
        columns: ["Name", "Label", "Type", "Counted value", "Variables"]
            .map(String::from)
            .into(),
        caption: rows
            .is_empty()
            .then(|| "The dataset has no multiple response sets.".into()),
        rows,
        ..Table::default()
    })
}
