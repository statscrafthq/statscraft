//! Changing the type or the string width of a variable in place, for ALTER TYPE.

use crate::{Dataset, Error, Measure, MissingValues, Value, sav};
use arrow::array::{ArrayRef, Float64Array, StringViewArray};
use std::sync::Arc;

/// The new values of a variable whose type or width changes.
pub enum NewValues {
    Numeric(Vec<Option<f64>>),
    /// Values no longer than `width` bytes.
    String {
        width: usize,
        values: Vec<String>,
    },
}

impl Dataset {
    /// Gives a variable new values and a new format in its place. Its label, role and display
    /// settings stay. When the type changes, its value labels and missing values go, and a scale
    /// variable that becomes a string becomes nominal; when only the string width changes, value
    /// labels and missing values that no longer fit are cut to the new width.
    pub fn retype_variable(
        &mut self,
        name: &str,
        values: NewValues,
        format: &str,
    ) -> Result<(), Error> {
        let index = self.index_of(name)?;
        let (column, width): (ArrayRef, Option<usize>) = match values {
            NewValues::Numeric(values) => {
                if values.len() != self.case_count {
                    return Err(Error::Invalid("one value per case is needed".into()));
                }
                (Arc::new(Float64Array::from(values)), None)
            }
            NewValues::String { width, values } => {
                if values.len() != self.case_count {
                    return Err(Error::Invalid("one value per case is needed".into()));
                }
                if let Some(v) = values.iter().find(|v| v.trim_end().len() > width) {
                    return Err(Error::Invalid(format!(
                        "{name}: value does not fit in {width} bytes: {v}"
                    )));
                }
                let trimmed = values.iter().map(|v| v.trim_end_matches(' '));
                (
                    Arc::new(StringViewArray::from_iter_values(trimmed)),
                    Some(width),
                )
            }
        };
        let mut def = self.dict.variables[index].clone();
        let type_changes = def.width.is_some() != width.is_some();
        def.width = width;
        if type_changes {
            def.value_labels.clear();
            def.missing = MissingValues::None;
            if width.is_some() && def.measure == Measure::Scale {
                def.measure = Measure::Nominal;
            }
        } else if let Some(width) = width {
            let cut = |s: &str| cut_to(s, width).to_string();
            for (value, _) in &mut def.value_labels {
                if let Value::String(s) = value {
                    *s = cut(s);
                }
            }
            if let MissingValues::Strings(values) = &mut def.missing {
                values.iter_mut().for_each(|v| *v = cut(v));
            }
        }
        def.format = sav::canonical_format(format, &def)?;
        Arc::make_mut(&mut self.dict).variables[index] = def;
        self.columns[index] = column;
        Ok(())
    }
}

/// The longest start of `s` that fits in `width` bytes, cut at a character boundary.
pub fn cut_to(s: &str, width: usize) -> &str {
    let mut end = width.min(s.len());
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}
