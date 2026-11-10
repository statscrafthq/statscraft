//! Changes to a dataset. Each one leaves data and dictionary consistent, or returns an error
//! and changes nothing.

use crate::{
    Dataset, Dictionary, Error, Measure, MissingValues, MultipleResponseSet, SplitFile, Value,
    VariableDef, VariableSet, non_empty, same_name, sav,
};
use arrow::array::{ArrayRef, AsArray, BooleanArray, Float64Array, StringViewArray, UInt64Array};
use std::sync::Arc;

const RESERVED_WORDS: [&str; 13] = [
    "ALL", "AND", "BY", "EQ", "GE", "GT", "LE", "LT", "NE", "NOT", "OR", "TO", "WITH",
];
const MAX_NAME_BYTES: usize = 64;
const MAX_STRING_WIDTH: usize = 32767;
/// A file stores a value label behind a one-byte length.
const MAX_VALUE_LABEL_BYTES: usize = 255;
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

    /// `None` or an empty label removes the file label.
    pub fn set_file_label(&mut self, label: Option<&str>) {
        Arc::make_mut(&mut self.dict).file_label = label.and_then(non_empty);
    }

    pub fn set_documents(&mut self, lines: Vec<String>) {
        Arc::make_mut(&mut self.dict).documents = lines;
    }

    /// Records where the data came from: the encoding name and "none", "bytecode" or "zlib".
    pub fn set_source(&mut self, encoding: &str, compression: &str) -> Result<(), Error> {
        let compression = ["none", "bytecode", "zlib"]
            .into_iter()
            .find(|c| *c == compression)
            .ok_or_else(|| Error::Invalid(format!("unknown compression {compression}")))?;
        let dict = Arc::make_mut(&mut self.dict);
        dict.encoding = encoding.to_string();
        dict.compression = compression;
        Ok(())
    }

    /// Replaces the data file attributes.
    pub fn set_attributes(&mut self, attributes: Vec<(String, Vec<String>)>) {
        Arc::make_mut(&mut self.dict).attributes = attributes;
    }

    /// Replaces the variable sets; every variable must exist, and is stored as the dictionary
    /// spells it.
    pub fn set_variable_sets(&mut self, sets: Vec<VariableSet>) -> Result<(), Error> {
        let mut checked = Vec::with_capacity(sets.len());
        for set in sets {
            let variables = set
                .variables
                .iter()
                .map(|name| Ok(self.dict.variables[self.index_of(name)?].name.clone()))
                .collect::<Result<_, Error>>()?;
            checked.push(VariableSet { variables, ..set });
        }
        Arc::make_mut(&mut self.dict).variable_sets = checked;
        Ok(())
    }

    /// The encoding `write_sav` writes text in: an encoding name such as "UTF-8" or
    /// "windows-1252", or a Windows code page number such as "1252".
    pub fn set_encoding(&mut self, encoding: &str) -> Result<(), Error> {
        let name = sav::encoding_name(encoding)?;
        Arc::make_mut(&mut self.dict).encoding = name;
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
            // unknown, like the original: procedures that care (CTABLES) guess the level from the data
            None => self.add_variable(name, "F8.2".to_string(), None, Measure::Unknown, column),
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

    /// Cases `offset` to `offset + length`, sharing the values and the dictionary. Panics when
    /// they are out of range.
    pub fn slice(&self, offset: usize, length: usize) -> Dataset {
        Dataset {
            columns: self
                .columns
                .iter()
                .map(|c| c.slice(offset, length))
                .collect(),
            case_count: length,
            dict: self.dict.clone(),
        }
    }

    /// The cases of `parts` one after the other, with the dictionary of `template`. A part
    /// that lacks a variable of the template has it blank there (system-missing, empty).
    pub fn from_parts(template: &Dataset, parts: &[Dataset]) -> Result<Dataset, Error> {
        let mut columns = Vec::with_capacity(template.columns.len());
        for (index, def) in template.dict.variables.iter().enumerate() {
            let mut arrays: Vec<ArrayRef> = Vec::with_capacity(parts.len() + 1);
            arrays.push(template.columns[index].clone());
            for part in parts {
                let same = part
                    .dict
                    .variables
                    .get(index)
                    .is_some_and(|d| d.name == def.name);
                let found = if same {
                    Some(index)
                } else {
                    part.variable_index(&def.name)
                };
                arrays.push(match found {
                    Some(i) => part.columns[i].clone(),
                    None if def.width.is_some() => Arc::new(StringViewArray::from_iter_values(
                        std::iter::repeat_n("", part.case_count),
                    )),
                    None => Arc::new(Float64Array::from(vec![None; part.case_count])),
                });
            }
            let refs: Vec<&dyn arrow::array::Array> = arrays.iter().map(|a| a.as_ref()).collect();
            columns.push(arrow::compute::concat(&refs).map_err(|e| Error::Invalid(e.to_string()))?);
        }
        Ok(Dataset {
            columns,
            case_count: template.case_count + parts.iter().map(|p| p.case_count).sum::<usize>(),
            dict: template.dict.clone(),
        })
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

    /// Builds a new list of cases: entry `i` names the old case that becomes case `i`, or is
    /// `None` for a blank case (system-missing, empty strings). Cases may repeat or drop out.
    pub fn take_cases(&mut self, cases: &[Option<usize>]) -> Result<(), Error> {
        self.columns = self
            .columns
            .iter()
            .map(|column| taken(column, cases))
            .collect::<Result<_, _>>()?;
        self.case_count = cases.len();
        Ok(())
    }

    /// Adds variable `name` of `from`, with its whole dictionary entry, after the existing
    /// variables. Its values come from the cases of `from` that `cases` names, one entry per
    /// case of this dataset; `None` gives a blank value.
    pub fn copy_variable(
        &mut self,
        from: &Dataset,
        name: &str,
        cases: &[Option<usize>],
    ) -> Result<(), Error> {
        self.check_length(cases.len())?;
        let index = from.index_of(name)?;
        let def = from.dict.variables[index].clone();
        if self.variable_index(&def.name).is_some() {
            return Err(Error::DuplicateName(def.name));
        }
        let column = taken(&from.columns[index], cases)?;
        Arc::make_mut(&mut self.dict).variables.push(def);
        self.columns.push(column);
        Ok(())
    }

    /// Changes the width of a string variable; its format becomes A`width`. Every value must
    /// fit in the new width.
    pub fn set_string_width(&mut self, name: &str, width: usize) -> Result<(), Error> {
        let index = self.index_of(name)?;
        let variable = self.variable_at(index);
        let values = variable.string()?;
        if !(1..=MAX_STRING_WIDTH).contains(&width) {
            return Err(Error::Invalid(format!(
                "string width must be 1 to {MAX_STRING_WIDTH}, got {width}"
            )));
        }
        if let Some(value) = values.iter().find(|v| v.len() > width) {
            return Err(Error::Invalid(format!(
                "{name}: value does not fit in {width} bytes: {value}"
            )));
        }
        let def = &mut Arc::make_mut(&mut self.dict).variables[index];
        def.width = Some(width);
        def.format = format!("A{width}");
        Ok(())
    }

    /// Multiple response sets, the weight and filter variables and the split follow the new
    /// name.
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
            .chain(
                dict.variable_sets
                    .iter_mut()
                    .flat_map(|set| &mut set.variables),
            )
            .chain(&mut dict.weight_variable)
            .chain(&mut dict.filter_variable)
            .chain(dict.split.iter_mut().flat_map(|split| &mut split.variables));
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

    /// Sets or (with `None`) turns off the split. The variables must exist; they are stored as
    /// the dictionary spells them.
    pub fn set_split_file(&mut self, split: Option<SplitFile>) -> Result<(), Error> {
        let split = match split {
            Some(mut split) => {
                for name in &mut split.variables {
                    *name = self.dict.variables[self.index_of(name)?].name.clone();
                }
                Some(split)
            }
            None => None,
        };
        Arc::make_mut(&mut self.dict).split = split;
        Ok(())
    }

    /// Sets or (with `None`) turns off the filter. The variable must be numeric.
    pub fn set_filter_variable(&mut self, name: Option<&str>) -> Result<(), Error> {
        let filter = match name {
            Some(name) => {
                let def = &self.dict.variables[self.index_of(name)?];
                if def.width.is_some() {
                    return Err(def.wrong_type("numeric"));
                }
                Some(def.name.clone())
            }
            None => None,
        };
        Arc::make_mut(&mut self.dict).filter_variable = filter;
        Ok(())
    }

    /// Adds a multiple response set, or replaces the set of the same name (names compare
    /// without regard to case). The name is "$" followed by a valid variable name. The set
    /// has at least two different variables of one type; they are stored as the dictionary
    /// spells them. A dichotomy set has a counted value, a whole number for numeric variables
    /// or a text no wider than the narrowest string variable; a category set has none.
    pub fn set_multiple_response_set(&mut self, set: MultipleResponseSet) -> Result<(), Error> {
        let invalid = |why: String| Err(Error::Invalid(format!("{}: {why}", set.name)));
        let rest = set.name.strip_prefix('$').unwrap_or("");
        if !set.name.starts_with('$')
            || set.name.len() > MAX_NAME_BYTES
            || check_name(rest).is_err()
        {
            return Err(Error::Invalid(format!(
                "invalid name of a multiple response set: {} (a set name is $ followed by a variable name)",
                set.name
            )));
        }
        if !matches!(set.kind, "dichotomy" | "category") {
            return invalid(format!("unknown kind of set {}", set.kind));
        }
        if set.counted_value_labels && set.kind != "dichotomy" {
            return invalid("only a dichotomy set labels its categories by counted value".into());
        }
        if set.label_from_variables && !set.counted_value_labels {
            return invalid("a label from the variables needs counted value labels".into());
        }
        if set.variables.len() < 2 {
            return invalid("a set needs at least two variables".into());
        }
        let mut defs: Vec<&VariableDef> = Vec::with_capacity(set.variables.len());
        for name in &set.variables {
            let def = &self.dict.variables[self.index_of(name)?];
            if defs.iter().any(|d| d.name == def.name) {
                return invalid(format!("{} is listed twice", def.name));
            }
            if def.width.is_some() != defs.first().unwrap_or(&def).width.is_some() {
                return invalid(format!(
                    "{} and {} differ in type; the variables of a set are all numeric or all strings",
                    defs[0].name, def.name
                ));
            }
            defs.push(def);
        }
        let variables = defs.iter().map(|d| d.name.clone()).collect();
        let narrowest = defs.iter().filter_map(|d| d.width).min();
        let counted_value = match (set.kind, &set.counted_value, narrowest) {
            ("category", None, _) => None,
            ("category", Some(_), _) => {
                return invalid("a category set has no counted value".into());
            }
            (_, None, _) => return invalid("a dichotomy set needs a counted value".into()),
            (_, Some(text), None) => match text.trim().parse::<f64>() {
                Ok(v) if v.fract() == 0.0 && v.abs() < 9.0e15 => Some((v as i64).to_string()),
                _ => {
                    return invalid(format!(
                        "the counted value of numeric variables must be a whole number, not {text}"
                    ));
                }
            },
            (_, Some(text), Some(narrowest)) => {
                let text = text.trim_end_matches(' ');
                if text.len() > narrowest {
                    return invalid(format!(
                        "the counted value '{text}' is wider than {narrowest} bytes, the width of the narrowest variable"
                    ));
                }
                Some(text.to_string())
            }
        };
        let set = MultipleResponseSet {
            variables,
            counted_value,
            ..set
        };
        let sets = &mut Arc::make_mut(&mut self.dict).multiple_response_sets;
        match sets
            .iter_mut()
            .find(|known| same_name(&known.name, &set.name))
        {
            Some(known) => *known = set,
            None => sets.push(set),
        }
        Ok(())
    }

    /// Removes the named multiple response sets (names compare without regard to case). An
    /// unknown name is an error and removes nothing.
    pub fn delete_multiple_response_sets(&mut self, names: &[&str]) -> Result<(), Error> {
        let sets = &self.dict.multiple_response_sets;
        if let Some(name) = names
            .iter()
            .find(|name| !sets.iter().any(|set| same_name(&set.name, name)))
        {
            return Err(Error::Invalid(format!(
                "unknown multiple response set {name}"
            )));
        }
        Arc::make_mut(&mut self.dict)
            .multiple_response_sets
            .retain(|set| !names.iter().any(|name| same_name(&set.name, name)));
        Ok(())
    }

    pub(crate) fn def_mut(&mut self, name: &str) -> Result<&mut VariableDef, Error> {
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

/// The values of `column` at `cases`; `None` is a blank value. String columns hold no nulls,
/// so a blank string is an empty one.
fn taken(column: &ArrayRef, cases: &[Option<usize>]) -> Result<ArrayRef, Error> {
    if let Some(&case) = cases.iter().flatten().find(|&&case| case >= column.len()) {
        return Err(Error::Invalid(format!(
            "case {case} is out of range: there are {} cases",
            column.len()
        )));
    }
    if let Some(strings) = column.as_string_view_opt() {
        let values = cases
            .iter()
            .map(|case| case.map_or("", |c| strings.value(c)));
        return Ok(Arc::new(StringViewArray::from_iter_values(values)));
    }
    let indices = UInt64Array::from_iter(cases.iter().map(|case| case.map(|c| c as u64)));
    arrow::compute::take(column, &indices, None).map_err(|e| Error::Invalid(e.to_string()))
}

fn drop_dangling_references(dict: &mut Dictionary) {
    let variables = &dict.variables;
    let exists = |name: &String| variables.iter().any(|v| v.name == *name);
    for set in &mut dict.multiple_response_sets {
        set.variables.retain(exists);
    }
    dict.multiple_response_sets
        .retain(|set| !set.variables.is_empty());
    for set in &mut dict.variable_sets {
        set.variables.retain(exists);
    }
    if !dict.weight_variable.as_ref().is_some_and(exists) {
        dict.weight_variable = None;
    }
    if !dict.filter_variable.as_ref().is_some_and(exists) {
        dict.filter_variable = None;
    }
    if !dict.split.iter().flat_map(|s| &s.variables).all(exists) {
        dict.split = None;
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
    // "$" starts system variables; "#" scratch variables, which a file never holds
    let valid = chars
        .next()
        .is_some_and(|c| c.is_alphabetic() || c == '@' || c == '#')
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
