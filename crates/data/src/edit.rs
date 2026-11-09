//! Changes to a dataset. Each one leaves data and dictionary consistent, or returns an error
//! and changes nothing.

use crate::{
    Dataset, Dictionary, Error, Measure, MissingValues, Value, VariableDef, non_empty, same_name,
    sav,
};
use arrow::array::{ArrayRef, BooleanArray, Float64Array, StringViewArray, UInt64Array};
use std::sync::Arc;

const RESERVED_WORDS: [&str; 13] = [
    "ALL", "AND", "BY", "EQ", "GE", "GT", "LE", "LT", "NE", "NOT", "OR", "TO", "WITH",
];
const MAX_NAME_BYTES: usize = 64;
const MAX_STRING_WIDTH: usize = 32767;
/// A file stores a value label behind a one-byte length, and readers expect at most this much.
const MAX_VALUE_LABEL_BYTES: usize = 120;
const MAX_MISSING_STRING_BYTES: usize = 8;

impl Dataset {
    /// `None` or an empty label removes the label.
    pub fn set_variable_label(&mut self, name: &str, label: Option<&str>) -> Result<(), Error> {
        self.def_mut(name)?.label = label.and_then(non_empty);
        Ok(())
    }

    /// Replaces all value labels of a variable; an empty list removes them. When a value occurs
    /// twice the later label wins.
    pub fn set_value_labels(
        &mut self,
        name: &str,
        labels: Vec<(Value, String)>,
    ) -> Result<(), Error> {
        let def = self.def_mut(name)?;
        let mut unique: Vec<(Value, String)> = Vec::with_capacity(labels.len());
        for (value, label) in labels {
            let value = match (value, def.width) {
                (Value::Number(v), None) => Value::Number(v),
                (Value::String(s), Some(width)) => Value::String(fitting(&s, width, def)?),
                (Value::String(_), None) => return Err(def.wrong_type("string")),
                (Value::Number(_), Some(_)) => return Err(def.wrong_type("numeric")),
            };
            if label.len() > MAX_VALUE_LABEL_BYTES {
                return Err(Error::Invalid(format!(
                    "value label longer than {MAX_VALUE_LABEL_BYTES} bytes: {label}"
                )));
            }
            match unique.iter_mut().find(|(seen, _)| *seen == value) {
                Some(entry) => entry.1 = label,
                None => unique.push((value, label)),
            }
        }
        def.value_labels = unique;
        Ok(())
    }

    /// `MissingValues::None` clears the definition.
    pub fn set_missing_values(&mut self, name: &str, missing: MissingValues) -> Result<(), Error> {
        let def = self.def_mut(name)?;
        let invalid = |why: &str| Err(Error::Invalid(format!("{}: {why}", def.name)));
        let missing = match missing {
            MissingValues::None => MissingValues::None,
            MissingValues::Numbers(v) if v.is_empty() => MissingValues::None,
            MissingValues::Strings(v) if v.is_empty() => MissingValues::None,
            MissingValues::Strings(values) => {
                let Some(width) = def.width else {
                    return Err(def.wrong_type("string"));
                };
                if values.len() > 3 {
                    return invalid("more than three missing values");
                }
                let width = width.min(MAX_MISSING_STRING_BYTES);
                let trimmed: Result<_, _> = values.iter().map(|s| fitting(s, width, def)).collect();
                MissingValues::Strings(trimmed?)
            }
            _ if def.width.is_some() => return Err(def.wrong_type("numeric")),
            MissingValues::Numbers(values) if values.len() > 3 => {
                return invalid("more than three missing values");
            }
            MissingValues::Numbers(values) if values.iter().any(|v| v.is_nan()) => {
                return invalid("a missing value must be a number");
            }
            MissingValues::Range { low, high, extra }
                if low.is_nan() || high.is_nan() || extra.is_some_and(f64::is_nan) =>
            {
                return invalid("a missing value must be a number");
            }
            MissingValues::Range { low, high, .. } if low > high => {
                return invalid("a missing range needs low <= high");
            }
            numeric => numeric,
        };
        def.missing = missing;
        Ok(())
    }

    pub fn set_measure(&mut self, name: &str, measure: Measure) -> Result<(), Error> {
        self.def_mut(name)?.measure = measure;
        Ok(())
    }

    /// Sets the display format, for example "F8.2", "COMMA12.0" or "DATE11". A string variable
    /// accepts only the A format of its own width.
    pub fn set_format(&mut self, name: &str, format: &str) -> Result<(), Error> {
        let def = self.def_mut(name)?;
        def.format = sav::canonical_format(format, def)?;
        Ok(())
    }

    /// Replaces the values of a numeric variable, or adds a new one (format F8.2, scale) after
    /// the existing variables. `None` is system-missing.
    pub fn set_numeric_values(
        &mut self,
        name: &str,
        values: Vec<Option<f64>>,
    ) -> Result<(), Error> {
        self.check_length(values.len())?;
        let column: ArrayRef = Arc::new(Float64Array::from(values));
        match self.variable_index(name) {
            Some(index) => match &self.dict.variables[index] {
                def if def.width.is_some() => Err(def.wrong_type("numeric")),
                _ => {
                    self.columns[index] = column;
                    Ok(())
                }
            },
            None => self.add_variable(name, "F8.2".to_string(), None, Measure::Scale, column),
        }
    }

    /// Replaces the values of a string variable of that width, or adds a new one (format
    /// A`width`, nominal) after the existing variables. Trailing spaces are padding. A value
    /// longer than `width` bytes is an error, not truncated.
    pub fn set_string_values(
        &mut self,
        name: &str,
        width: usize,
        values: Vec<String>,
    ) -> Result<(), Error> {
        self.check_length(values.len())?;
        if !(1..=MAX_STRING_WIDTH).contains(&width) {
            return Err(Error::Invalid(format!(
                "string width must be 1 to {MAX_STRING_WIDTH}, got {width}"
            )));
        }
        let trimmed = values.iter().map(|v| v.trim_end_matches(' '));
        if let Some(value) = trimmed.clone().find(|v| v.len() > width) {
            return Err(Error::Invalid(format!(
                "{name}: value does not fit in {width} bytes: {value}"
            )));
        }
        let column: ArrayRef = Arc::new(StringViewArray::from_iter_values(trimmed));
        match self.variable_index(name) {
            Some(index) => match &self.dict.variables[index] {
                def if def.width.is_none() => Err(def.wrong_type("string")),
                def if def.width != Some(width) => Err(Error::Invalid(format!(
                    "{} has width {}, not {width}",
                    def.name,
                    def.width.unwrap_or(0)
                ))),
                _ => {
                    self.columns[index] = column;
                    Ok(())
                }
            },
            None => self.add_variable(
                name,
                format!("A{width}"),
                Some(width),
                Measure::Nominal,
                column,
            ),
        }
    }

    /// Keeps the cases whose mask entry is true.
    pub fn select_cases(&mut self, keep: &[bool]) -> Result<(), Error> {
        self.check_length(keep.len())?;
        let mask = BooleanArray::from(keep.to_vec());
        let filtered: Result<Vec<_>, _> = self
            .columns
            .iter()
            .map(|column| arrow::compute::filter(column, &mask))
            .collect();
        self.columns = filtered.map_err(|e| Error::Invalid(e.to_string()))?;
        self.case_count = mask.true_count();
        Ok(())
    }

    /// Puts the cases in a new order: old case `order[i]` becomes case `i`. `order` must name
    /// every case exactly once.
    pub fn reorder_cases(&mut self, order: &[usize]) -> Result<(), Error> {
        self.check_length(order.len())?;
        let mut seen = vec![false; order.len()];
        for &case in order {
            match seen.get_mut(case) {
                Some(seen) if !*seen => *seen = true,
                _ => {
                    return Err(Error::Invalid(format!(
                        "case order must name every case once: {case} is repeated or out of range"
                    )));
                }
            }
        }
        let indices = UInt64Array::from_iter_values(order.iter().map(|&case| case as u64));
        let taken: Result<Vec<_>, _> = self
            .columns
            .iter()
            .map(|column| arrow::compute::take(column, &indices, None))
            .collect();
        self.columns = taken.map_err(|e| Error::Invalid(e.to_string()))?;
        Ok(())
    }

    /// Multiple response sets and the weight designation follow the new name.
    pub fn rename_variable(&mut self, name: &str, new_name: &str) -> Result<(), Error> {
        let index = self.index_of(name)?;
        check_name(new_name)?;
        if self.variable_index(new_name).is_some_and(|i| i != index) {
            return Err(Error::DuplicateName(new_name.to_string()));
        }
        let dict = Arc::make_mut(&mut self.dict);
        let old = std::mem::replace(&mut dict.variables[index].name, new_name.to_string());
        let references = dict
            .multiple_response_sets
            .iter_mut()
            .flat_map(|set| &mut set.variables)
            .chain(&mut dict.weight_variable);
        for reference in references.filter(|r| **r == old) {
            *reference = new_name.to_string();
        }
        Ok(())
    }

    /// Deleted variables leave their multiple response sets (a set left empty is dropped) and
    /// stop being the weight variable. At least one variable must remain.
    pub fn delete_variables(&mut self, names: &[&str]) -> Result<(), Error> {
        let mut keep = vec![true; self.columns.len()];
        for name in names {
            keep[self.index_of(name)?] = false;
        }
        if !keep.contains(&true) {
            return Err(Error::Invalid("cannot delete every variable".to_string()));
        }
        let dict = Arc::make_mut(&mut self.dict);
        let mut kept = keep.iter();
        dict.variables.retain(|_| *kept.next().unwrap());
        let mut kept = keep.iter();
        self.columns.retain(|_| *kept.next().unwrap());
        drop_dangling_references(dict);
        Ok(())
    }

    /// Keeps only the named variables, in the order given. Each name may occur once and at
    /// least one is needed. References to the other variables end as in `delete_variables`.
    pub fn keep_variables(&mut self, names: &[&str]) -> Result<(), Error> {
        let mut indices = Vec::with_capacity(names.len());
        for name in names {
            let index = self.index_of(name)?;
            if indices.contains(&index) {
                return Err(Error::DuplicateName(name.to_string()));
            }
            indices.push(index);
        }
        if indices.is_empty() {
            return Err(Error::Invalid("cannot delete every variable".to_string()));
        }
        let dict = Arc::make_mut(&mut self.dict);
        dict.variables = indices
            .iter()
            .map(|&index| dict.variables[index].clone())
            .collect();
        self.columns = indices
            .iter()
            .map(|&index| self.columns[index].clone())
            .collect();
        drop_dangling_references(dict);
        Ok(())
    }

    /// `None` turns weighting off. The weight variable must be numeric.
    pub fn set_weight_variable(&mut self, name: Option<&str>) -> Result<(), Error> {
        let weight = match name {
            Some(name) => {
                let def = &self.dict.variables[self.index_of(name)?];
                if def.width.is_some() {
                    return Err(def.wrong_type("numeric"));
                }
                Some(def.name.clone())
            }
            None => None,
        };
        Arc::make_mut(&mut self.dict).weight_variable = weight;
        Ok(())
    }

    fn def_mut(&mut self, name: &str) -> Result<&mut VariableDef, Error> {
        let index = self.index_of(name)?;
        Ok(&mut Arc::make_mut(&mut self.dict).variables[index])
    }

    fn check_length(&self, actual: usize) -> Result<(), Error> {
        let expected = self.case_count;
        if actual == expected {
            Ok(())
        } else {
            Err(Error::LengthMismatch { expected, actual })
        }
    }

    fn add_variable(
        &mut self,
        name: &str,
        format: String,
        width: Option<usize>,
        measure: Measure,
        column: ArrayRef,
    ) -> Result<(), Error> {
        check_name(name)?;
        Arc::make_mut(&mut self.dict).variables.push(VariableDef {
            name: name.to_string(),
            label: None,
            format,
            width,
            measure,
            value_labels: Vec::new(),
            missing: MissingValues::None,
            extras: Default::default(),
        });
        self.columns.push(column);
        Ok(())
    }
}

fn drop_dangling_references(dict: &mut Dictionary) {
    let variables = &dict.variables;
    let exists = |name: &String| variables.iter().any(|v| v.name == *name);
    for set in &mut dict.multiple_response_sets {
        set.variables.retain(exists);
    }
    dict.multiple_response_sets
        .retain(|set| !set.variables.is_empty());
    if !dict.weight_variable.as_ref().is_some_and(exists) {
        dict.weight_variable = None;
    }
}

/// A string value of the dictionary (value label key, missing value) without its padding, or an
/// error when it is longer than `width` bytes.
fn fitting(value: &str, width: usize, def: &VariableDef) -> Result<String, Error> {
    let value = value.trim_end_matches(' ');
    if value.len() > width {
        return Err(Error::Invalid(format!(
            "{}: value longer than {width} bytes: {value}",
            def.name
        )));
    }
    Ok(value.to_string())
}

fn check_name(name: &str) -> Result<(), Error> {
    let mut chars = name.chars();
    // "#" and "$" start scratch and system variables, which never belong in a saved dataset
    let valid = chars.next().is_some_and(|c| c.is_alphabetic() || c == '@')
        && chars.all(|c| c.is_alphanumeric() || "_.$#@".contains(c))
        && name.len() <= MAX_NAME_BYTES
        && !name.ends_with('.')
        && !RESERVED_WORDS.iter().any(|word| same_name(word, name));
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidName(name.to_string()))
    }
}
