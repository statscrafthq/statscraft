//! Reading and writing `.sav` / `.zsav` files. The only module that knows the `statscraft-sav` crate.

use crate::{
    Dataset, Dictionary, Error, Measure, MissingValues, MultipleResponseSet, Value, VariableDef,
    non_empty,
};
use arrow::array::RecordBatch;
use arrow::datatypes::{DataType, Field, Schema};
use statscraft_sav::constants::SavFormat;
use std::path::Path;
use std::sync::Arc;

/// Display properties the model does not interpret, kept so that writing gives them back.
#[derive(Clone, Default, PartialEq)]
pub(crate) struct VariableExtras {
    display_width: Option<u32>,
    alignment: Option<statscraft_sav::Alignment>,
    role: Option<statscraft_sav::constants::Role>,
    attributes: Vec<(String, Vec<String>)>,
}

const ALIGNMENTS: [(&str, statscraft_sav::Alignment); 3] = [
    ("left", statscraft_sav::Alignment::Left),
    ("right", statscraft_sav::Alignment::Right),
    ("center", statscraft_sav::Alignment::Center),
];

const ROLES: [(&str, statscraft_sav::constants::Role); 6] = {
    use statscraft_sav::constants::Role;
    [
        ("input", Role::Input),
        ("target", Role::Target),
        ("both", Role::Both),
        ("none", Role::None),
        ("partition", Role::Partition),
        ("split", Role::Split),
    ]
};

impl<'a> crate::Variable<'a> {
    /// Display width in characters, when the file gives one.
    pub fn display_width(&self) -> Option<u32> {
        self.def.extras.display_width
    }

    /// "left", "right" or "center", when the file gives one.
    pub fn alignment(&self) -> Option<&'static str> {
        let alignment = self.def.extras.alignment?;
        ALIGNMENTS
            .iter()
            .find(|(_, a)| *a == alignment)
            .map(|e| e.0)
    }

    /// "input", "target", "both", "none", "partition" or "split", when the file gives one.
    pub fn role(&self) -> Option<&'static str> {
        let role = self.def.extras.role?;
        ROLES.iter().find(|(_, r)| *r == role).map(|e| e.0)
    }

    /// Custom attributes: name and its values, in file order.
    pub fn attributes(&self) -> &'a [(String, Vec<String>)] {
        &self.def.extras.attributes
    }
}

impl Dataset {
    /// Sets the display properties of a variable; the texts are those `Variable::alignment`
    /// and `Variable::role` return.
    pub fn set_display(
        &mut self,
        name: &str,
        width: Option<u32>,
        alignment: Option<&str>,
        role: Option<&str>,
    ) -> Result<(), Error> {
        fn find<T: Copy>(table: &[(&str, T)], text: Option<&str>) -> Result<Option<T>, Error> {
            text.map(|text| {
                table
                    .iter()
                    .find(|(t, _)| *t == text)
                    .map(|e| e.1)
                    .ok_or_else(|| Error::Invalid(format!("unknown display setting {text}")))
            })
            .transpose()
        }
        let alignment = find(&ALIGNMENTS, alignment)?;
        let role = find(&ROLES, role)?;
        let extras = &mut self.def_mut(name)?.extras;
        extras.display_width = width;
        extras.alignment = alignment;
        extras.role = role;
        Ok(())
    }

    /// Replaces the custom attributes of a variable.
    pub fn set_variable_attributes(
        &mut self,
        name: &str,
        attributes: Vec<(String, Vec<String>)>,
    ) -> Result<(), Error> {
        self.def_mut(name)?.extras.attributes = attributes;
        Ok(())
    }

    pub fn read_sav(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let file_error = |e: String| Error::File(format!("{}: {e}", path.display()));
        let (batch, mut meta) = statscraft_sav::read_sav(path).map_err(|e| file_error(e.to_string()))?;

        // date and time variables arrive as the stored numbers, which is what the model keeps
        let columns = batch.columns().to_vec();
        let mut variables = Vec::with_capacity(batch.num_columns());
        for (name, column) in meta.variable_names.iter().zip(batch.columns()) {
            let format = meta.variable_formats.swap_remove(name).unwrap_or_default();
            let width = (column.data_type() == &DataType::Utf8View).then(|| {
                // the declared width is in the format (A10); the reader's storage width is padded
                let digits = format.trim_start_matches(|c: char| c.is_ascii_alphabetic());
                digits.parse().unwrap_or(0)
            });
            variables.push(VariableDef {
                name: name.clone(),
                label: meta.variable_labels.get(name).and_then(|l| non_empty(l)),
                format,
                width,
                measure: match meta.variable_measures.get(name) {
                    Some(statscraft_sav::Measure::Nominal) => Measure::Nominal,
                    Some(statscraft_sav::Measure::Ordinal) => Measure::Ordinal,
                    Some(statscraft_sav::Measure::Scale) => Measure::Scale,
                    _ => Measure::Unknown,
                },
                value_labels: meta
                    .variable_value_labels
                    .swap_remove(name)
                    .into_iter()
                    .flatten()
                    .map(|(value, label)| (model_value(value), label))
                    .collect(),
                missing: model_missing(meta.variable_missing_values.swap_remove(name)),
                extras: VariableExtras {
                    display_width: meta.variable_display_widths.get(name).copied(),
                    alignment: meta.variable_alignments.get(name).copied(),
                    role: meta.variable_roles.get(name).copied(),
                    attributes: meta
                        .variable_attributes
                        .swap_remove(name)
                        .into_iter()
                        .flatten()
                        .collect(),
                },
            });
        }

        let multiple_response_sets = meta
            .mr_sets
            .into_values()
            .map(|set| MultipleResponseSet {
                // the reader strips the "$" that set names carry in syntax
                name: format!("${}", set.name),
                kind: match set.mr_type {
                    statscraft_sav::MrType::MultipleDichotomy => "dichotomy",
                    statscraft_sav::MrType::MultipleCategory => "category",
                },
                label: set.label,
                counted_value: set.counted_value,
                variables: set.variables,
            })
            .collect();
        Ok(Self {
            columns,
            case_count: batch.num_rows(),
            dict: Arc::new(Dictionary {
                encoding: meta.file_encoding,
                compression: match meta.compression {
                    statscraft_sav::Compression::None => "none",
                    statscraft_sav::Compression::Bytecode => "bytecode",
                    statscraft_sav::Compression::Zlib => "zlib",
                },
                file_label: non_empty(meta.file_label.trim_end()),
                weight_variable: meta.weight_variable,
                variables,
                multiple_response_sets,
                warnings: meta.warnings,
                documents: meta.notes,
            }),
        })
    }

    /// Writes the dataset as UTF-8: zlib-compressed when the extension is `.zsav`, otherwise
    /// bytecode-compressed. An existing file is replaced only after the new one is complete.
    pub fn write_sav(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        let path = path.as_ref();
        let file_error = |e: String| Error::File(format!("{}: {e}", path.display()));
        let dict = &*self.dict;
        // shortcut: text from a file in another encoding can outgrow its byte width as UTF-8 and
        // the writer then cuts it; widen such variables when a real code-page file arrives
        let mut meta = statscraft_sav::SavMetadata {
            file_label: dict.file_label.clone().unwrap_or_default(),
            notes: dict.documents.clone(),
            weight_variable: dict.weight_variable.clone(),
            ..Default::default()
        };

        let mut fields = Vec::with_capacity(self.columns.len());
        let mut columns = Vec::with_capacity(self.columns.len());
        for variable in self.variables() {
            let def = variable.def;
            let name = &def.name;
            let column = variable.column.clone();
            fields.push(Field::new(name, column.data_type().clone(), true));
            columns.push(column);

            meta.variable_names.push(name.clone());
            meta.variable_formats
                .insert(name.clone(), def.format.clone());
            let measure = match def.measure {
                // a file has no "not decided yet": write the level the reference program writes
                Measure::Unknown if def.width.is_some() => statscraft_sav::Measure::Nominal,
                Measure::Unknown => statscraft_sav::Measure::Scale,
                Measure::Nominal => statscraft_sav::Measure::Nominal,
                Measure::Ordinal => statscraft_sav::Measure::Ordinal,
                Measure::Scale => statscraft_sav::Measure::Scale,
            };
            meta.variable_measures.insert(name.clone(), measure);
            if let Some(label) = &def.label {
                meta.variable_labels.insert(name.clone(), label.clone());
            }
            if !def.value_labels.is_empty() {
                let labels = def.value_labels.iter().map(|(value, label)| {
                    let value = match value {
                        Value::Number(v) => statscraft_sav::Value::Numeric(*v),
                        Value::String(s) => statscraft_sav::Value::String(s.clone()),
                    };
                    (value, label.clone())
                });
                meta.variable_value_labels
                    .insert(name.clone(), labels.collect());
            }
            let missing = writer_missing(&def.missing);
            if !missing.is_empty() {
                meta.variable_missing_values.insert(name.clone(), missing);
            }
            let extras = &def.extras;
            if let Some(width) = extras.display_width {
                meta.variable_display_widths.insert(name.clone(), width);
            }
            if let Some(alignment) = extras.alignment {
                meta.variable_alignments.insert(name.clone(), alignment);
            }
            if let Some(role) = extras.role {
                meta.variable_roles.insert(name.clone(), role);
            }
            if !extras.attributes.is_empty() {
                let attributes = extras.attributes.iter().cloned().collect();
                meta.variable_attributes.insert(name.clone(), attributes);
            }
        }
        for set in &dict.multiple_response_sets {
            let name = set.name.trim_start_matches('$').to_string();
            let set = statscraft_sav::MrSet {
                name: name.clone(),
                label: set.label.clone(),
                mr_type: match set.kind {
                    "dichotomy" => statscraft_sav::MrType::MultipleDichotomy,
                    _ => statscraft_sav::MrType::MultipleCategory,
                },
                counted_value: set.counted_value.clone(),
                variables: set.variables.clone(),
            };
            meta.mr_sets.insert(name, set);
        }

        let batch = RecordBatch::try_new(Arc::new(Schema::new(fields)), columns)
            .map_err(|e| file_error(e.to_string()))?;
        let zsav = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("zsav"));
        let compression = if zsav {
            statscraft_sav::Compression::Zlib
        } else {
            statscraft_sav::Compression::Bytecode
        };
        // a failed or interrupted write must not destroy the previous session state
        let mut partial = path.as_os_str().to_owned();
        partial.push(format!(".{}.partial", std::process::id()));
        statscraft_sav::write_sav(&partial, &batch, &meta, compression, None)
            .map_err(|e| e.to_string())
            .and_then(|()| std::fs::rename(&partial, path).map_err(|e| e.to_string()))
            .map_err(|e| {
                let _ = std::fs::remove_file(&partial);
                file_error(e)
            })
    }
}

/// Canonical spelling of a display format ("f8" becomes "F8.0"), checked against the type of
/// the variable it is meant for.
pub(crate) fn canonical_format(format: &str, def: &VariableDef) -> Result<String, Error> {
    let invalid = || Error::Invalid(format!("invalid format: {format}"));
    let parsed = SavFormat::from_string(format).ok_or_else(invalid)?;
    let kind = parsed.format_type;
    match def.width {
        // a display format cannot change the width of a string variable
        Some(width) if format.trim().eq_ignore_ascii_case(&format!("A{width}")) => {
            Ok(format!("A{width}"))
        }
        Some(width) if kind.is_string() => Err(Error::Invalid(format!(
            "{} has width {width}, format {format} does not fit",
            def.name
        ))),
        Some(_) => Err(def.wrong_type("numeric")),
        None if kind.is_string() => Err(def.wrong_type("string")),
        None if !(1..=40).contains(&parsed.width) || parsed.decimals > 16 => Err(invalid()),
        None if !format.contains(|c: char| c.is_ascii_digit()) => Err(invalid()),
        None if kind.is_date_time() && parsed.decimals == 0 => {
            Ok(format!("{}{}", kind.prefix(), parsed.width))
        }
        None => Ok(format!(
            "{}{}.{}",
            kind.prefix(),
            parsed.width,
            parsed.decimals
        )),
    }
}

fn model_value(value: statscraft_sav::Value) -> Value {
    match value {
        statscraft_sav::Value::Numeric(v) => Value::Number(v),
        statscraft_sav::Value::String(s) => Value::String(s.trim_end_matches(' ').to_string()),
    }
}

fn model_missing(specs: Option<Vec<statscraft_sav::MissingSpec>>) -> MissingValues {
    let (mut numbers, mut strings, mut range) = (Vec::new(), Vec::new(), None);
    for spec in specs.into_iter().flatten() {
        match spec {
            statscraft_sav::MissingSpec::Value(v) => numbers.push(v),
            statscraft_sav::MissingSpec::Range { lo, hi } => range = Some((lo, hi)),
            statscraft_sav::MissingSpec::StringValue(s) => {
                strings.push(s.trim_end_matches(' ').to_string())
            }
        }
    }
    match range {
        Some((low, high)) => MissingValues::Range {
            low,
            high,
            extra: numbers.first().copied(),
        },
        None if !strings.is_empty() => MissingValues::Strings(strings),
        None if !numbers.is_empty() => MissingValues::Numbers(numbers),
        None => MissingValues::None,
    }
}

fn writer_missing(missing: &MissingValues) -> Vec<statscraft_sav::MissingSpec> {
    use statscraft_sav::MissingSpec;
    match missing {
        MissingValues::None => Vec::new(),
        MissingValues::Numbers(discrete) => {
            discrete.iter().map(|v| MissingSpec::Value(*v)).collect()
        }
        MissingValues::Range { low, high, extra } => {
            let range = MissingSpec::Range {
                lo: *low,
                hi: *high,
            };
            std::iter::once(range)
                .chain(extra.map(MissingSpec::Value))
                .collect()
        }
        MissingValues::Strings(discrete) => discrete
            .iter()
            .map(|s| MissingSpec::StringValue(s.clone()))
            .collect(),
    }
}
