//! How variables and values show in output: the one place that turns a variable or a value
//! into its text, after SET TVARS and TNUMBERS (headers, row labels and cells of tables) and
//! SET OVARS and ONUMBERS (table titles, which are Statscraft's outline).
//!
//! Rules, from SET in the Command Syntax Reference:
//! - TVARS and OVARS take NAMES, LABELS or BOTH; LABELS is the default. LABELS shows the
//!   variable label, or the name when the variable has none. BOTH shows the name, a space and
//!   the label ("q1 Satisfaction"), the name alone without a label.
//! - TNUMBERS and ONUMBERS take VALUES, LABELS or BOTH; LABELS is the default. LABELS shows the
//!   value label, or the value in the variable's format when it has none. BOTH shows the value,
//!   a space and the label ("1 A great deal"), as the Custom Tables manual pictures it.
//! - A procedure's own subcommand wins over these: CTABLES /VLABELS, MULT RESPONSE
//!   /FORMAT=NOLABELS.

use crate::crosstabs::Key;
use crate::format::{Currencies, category_text};
use crate::{Engine, Table};
use statscraft_data::{Dataset, Value, Variable};

/// Names (of variables) or values, labels, or both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Show {
    Plain,
    Labels,
    Both,
}

impl Show {
    fn of(setting: &str) -> Show {
        match setting.to_uppercase().as_str() {
            "NAMES" | "VALUES" => Show::Plain,
            "BOTH" => Show::Both,
            _ => Show::Labels,
        }
    }

    /// A name or value with its label, if any.
    pub(crate) fn join(self, plain: &str, label: Option<&str>) -> String {
        match (self, label.filter(|l| !l.is_empty())) {
            (Show::Labels, Some(label)) => label.to_string(),
            (Show::Both, Some(label)) => format!("{plain} {label}"),
            _ => plain.to_string(),
        }
    }
}

/// The values SET TVARS and OVARS take, and those of TNUMBERS and ONUMBERS.
pub(crate) const VARIABLE_CHOICES: [&str; 3] = ["NAMES", "LABELS", "BOTH"];
pub(crate) const VALUE_CHOICES: [&str; 3] = ["VALUES", "LABELS", "BOTH"];

/// How one kind of output shows variables and values.
#[derive(Clone, Debug)]
pub(crate) struct Shown {
    pub(crate) variables: Show,
    pub(crate) values: Show,
    pub(crate) currencies: Currencies,
}

impl Shown {
    /// Labels whatever the settings, for text that goes into the data, such as new variable
    /// labels.
    pub(crate) fn labels(currencies: Currencies) -> Shown {
        Shown {
            variables: Show::Labels,
            values: Show::Labels,
            currencies,
        }
    }

    pub(crate) fn variable(&self, variable: &Variable) -> String {
        self.variables.join(variable.name(), variable.label())
    }

    /// A variable by name; a name the data do not have shows as it is.
    pub(crate) fn named(&self, data: &Dataset, name: &str) -> String {
        data.variable(name)
            .map_or_else(|_| name.to_string(), |v| self.variable(&v))
    }

    /// A value of `variable`: a date, time or custom currency in the variable's format.
    pub(crate) fn value(&self, variable: &Variable, key: &Key) -> String {
        let label = variable
            .value_labels()
            .iter()
            .find(|(value, _)| match (value, key) {
                (Value::Number(a), Key::Number(b)) => a == b,
                (Value::String(a), Key::Text(b)) => a == b,
                _ => false,
            })
            .map(|(_, label)| label.as_str());
        let plain = match key {
            Key::Number(n) => category_text(*n, &self.currencies.resolve(variable.format())),
            Key::Text(text) => text.clone(),
        };
        self.values.join(&plain, label)
    }

    pub(crate) fn number(&self, variable: &Variable, value: f64) -> String {
        self.value(variable, &Key::Number(value))
    }
}

/// Leaves out a row header column, as FREQUENCIES and DESCRIPTIVES do with their Label column
/// under SET TNUMBERS=VALUES and TVARS=NAMES.
pub(crate) fn drop_label_column(table: &mut Table, column: usize) {
    table.row_headers -= 1;
    table.columns.remove(column);
    if !table.column_groups.is_empty() {
        table.column_groups.remove(column);
    }
    for row in &mut table.rows {
        row.remove(column);
    }
    for row in &mut table.formats {
        row.remove(column);
    }
}

impl Engine {
    /// How tables show variables and values: SET TVARS and TNUMBERS.
    pub(crate) fn shown(&self) -> Shown {
        Shown {
            variables: Show::of(&self.setting("TVARS")),
            values: Show::of(&self.setting("TNUMBERS")),
            currencies: self.currencies(),
        }
    }

    /// How table titles show variables and values: SET OVARS and ONUMBERS.
    pub(crate) fn outline(&self) -> Shown {
        Shown {
            variables: Show::of(&self.setting("OVARS")),
            values: Show::of(&self.setting("ONUMBERS")),
            currencies: self.currencies(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_names_values_and_both() {
        assert_eq!(
            Show::Labels.join("q1", Some("Satisfaction")),
            "Satisfaction"
        );
        assert_eq!(Show::Labels.join("q1", None), "q1");
        assert_eq!(Show::Labels.join("q1", Some("")), "q1");
        assert_eq!(Show::Plain.join("q1", Some("Satisfaction")), "q1");
        assert_eq!(Show::Both.join("1", Some("A great deal")), "1 A great deal");
        assert_eq!(Show::Both.join("2", None), "2");
        assert_eq!(Show::of("names"), Show::Plain);
        assert_eq!(Show::of("VALUES"), Show::Plain);
        assert_eq!(Show::of("BOTH"), Show::Both);
        assert_eq!(Show::of("LABELS"), Show::Labels);
    }
}
