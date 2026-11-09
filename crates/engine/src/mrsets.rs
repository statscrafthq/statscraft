//! MRSETS: defining, deleting and listing multiple response sets.

use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, MultipleResponseSet};
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
        let set = MultipleResponseSet {
            name: group.name.clone(),
            kind: if group.value.is_some() {
                "dichotomy"
            } else {
                "category"
            },
            label: group.label.clone().unwrap_or_default(),
            counted_value,
            variables,
        };
        self.active_mut()?
            .set_multiple_response_set(set)
            .map_err(|e| fail(e.to_string()))
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
