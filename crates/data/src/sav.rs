//! Reading and writing `.sav` / `.zsav` files. The only module that knows the `ambers` crate.

use crate::{
    Dataset, Dictionary, Error, Measure, MissingValues, MultipleResponseSet, Value, VariableDef,
    non_empty,
};
use ambers::constants::{
    MICROS_PER_SECOND, SECONDS_PER_DAY, the reference program_EPOCH_OFFSET_DAYS, the reference program_EPOCH_OFFSET_SECONDS,
    SavFormat, TemporalKind,
};
use arrow::array::{ArrayRef, AsArray, Float64Array, RecordBatch};
use arrow::compute::unary;
use arrow::datatypes::{
    DataType, Date32Type, DurationMicrosecondType, Field, Float64Type, Schema, TimeUnit,
    TimestampMicrosecondType,
};
use std::path::Path;
use std::sync::Arc;

const EPOCH_DAYS: f64 = the reference program_EPOCH_OFFSET_DAYS as f64;
/// The years 1 to 9999 as seconds since 1582-10-14: the reader turns date and datetime values
/// outside them into system-missing.
const DATE_RANGE: std::ops::Range<f64> = -49_916_217_600.0..265_621_680_000.0;
/// The reader turns longer durations into system-missing.
const MAX_DURATION: f64 = 9.0e12;

/// Display properties the model does not interpret, kept so that writing gives them back.
#[derive(Clone, Default)]
pub(crate) struct VariableExtras {
    display_width: Option<u32>,
    alignment: Option<ambers::Alignment>,
    role: Option<ambers::constants::Role>,
    attributes: Vec<(String, Vec<String>)>,
}

impl Dataset {
    pub fn read_sav(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let file_error = |e: String| Error::File(format!("{}: {e}", path.display()));
        let (batch, mut meta) = ambers::read_sav(path).map_err(|e| file_error(e.to_string()))?;

        let mut columns = Vec::with_capacity(batch.num_columns());
        let mut variables = Vec::with_capacity(batch.num_columns());
        for (name, column) in meta.variable_names.iter().zip(batch.columns()) {
            columns.push(model_column(column).map_err(|e| file_error(format!("{name}: {e}")))?);
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
                    Some(ambers::Measure::Nominal) => Measure::Nominal,
                    Some(ambers::Measure::Ordinal) => Measure::Ordinal,
                    Some(ambers::Measure::Scale) => Measure::Scale,
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
                    ambers::MrType::MultipleDichotomy => "dichotomy",
                    ambers::MrType::MultipleCategory => "category",
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
                    ambers::Compression::None => "none",
                    ambers::Compression::Bytecode => "bytecode",
                    ambers::Compression::Zlib => "zlib",
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
        let mut meta = ambers::SavMetadata {
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
            let column = writer_column(def, variable.column)?;
            fields.push(Field::new(name, column.data_type().clone(), true));
            columns.push(column);

            meta.variable_names.push(name.clone());
            meta.variable_formats
                .insert(name.clone(), def.format.clone());
            let measure = match def.measure {
                Measure::Unknown => ambers::Measure::Unknown,
                Measure::Nominal => ambers::Measure::Nominal,
                Measure::Ordinal => ambers::Measure::Ordinal,
                Measure::Scale => ambers::Measure::Scale,
            };
            meta.variable_measures.insert(name.clone(), measure);
            if let Some(label) = &def.label {
                meta.variable_labels.insert(name.clone(), label.clone());
            }
            if !def.value_labels.is_empty() {
                let labels = def.value_labels.iter().map(|(value, label)| {
                    let value = match value {
                        Value::Number(v) => ambers::Value::Numeric(*v),
                        Value::String(s) => ambers::Value::String(s.clone()),
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
            let set = ambers::MrSet {
                name: name.clone(),
                label: set.label.clone(),
                mr_type: match set.kind {
                    "dichotomy" => ambers::MrType::MultipleDichotomy,
                    _ => ambers::MrType::MultipleCategory,
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
            ambers::Compression::Zlib
        } else {
            ambers::Compression::Bytecode
        };
        // a failed or interrupted write must not destroy the previous session state
        let mut partial = path.as_os_str().to_owned();
        partial.push(format!(".{}.partial", std::process::id()));
        ambers::write_sav(&partial, &batch, &meta, compression, None)
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

fn date_number(days: i32) -> f64 {
    (days as f64 + EPOCH_DAYS) * SECONDS_PER_DAY
}

fn datetime_number(micros: i64) -> f64 {
    micros as f64 / MICROS_PER_SECOND + the reference program_EPOCH_OFFSET_SECONDS
}

fn duration_number(micros: i64) -> f64 {
    micros as f64 / MICROS_PER_SECOND
}

/// The reader hands date and time variables over as Arrow temporal types; the model wants the
/// stored numbers back.
fn model_column(column: &ArrayRef) -> Result<ArrayRef, String> {
    let numbers: Float64Array = match column.data_type() {
        DataType::Float64 | DataType::Utf8View => return Ok(column.clone()),
        DataType::Date32 => unary(column.as_primitive::<Date32Type>(), date_number),
        DataType::Timestamp(TimeUnit::Microsecond, _) => unary(
            column.as_primitive::<TimestampMicrosecondType>(),
            datetime_number,
        ),
        DataType::Duration(TimeUnit::Microsecond) => unary(
            column.as_primitive::<DurationMicrosecondType>(),
            duration_number,
        ),
        other => return Err(format!("unsupported column type {other}")),
    };
    Ok(Arc::new(numbers))
}

/// The writer wants date and time variables as Arrow temporal types again.
fn writer_column(def: &VariableDef, column: &ArrayRef) -> Result<ArrayRef, Error> {
    if def.width.is_some() {
        return Ok(column.clone());
    }
    let kind = SavFormat::from_string(&def.format).and_then(|f| f.format_type.temporal_kind());
    let Some(kind) = kind else {
        return Ok(column.clone());
    };
    let numbers: &Float64Array = column.as_primitive();
    // shortcut: date and time values pass through the reader's and writer's Arrow temporal types
    // (whole days for dates, whole microseconds otherwise, years 1 to 9999), so a value those
    // cannot hold is refused here instead of coming back changed; hide the date formats from
    // ambers (or get it an option for raw numbers) when such values need saving. The
    // conversions below mirror the ones in the ambers reader, which is what makes the check
    // predict the next load.
    let temporal: ArrayRef = match kind {
        TemporalKind::Date => Arc::new(unary::<_, _, Date32Type>(numbers, |v| {
            (v / SECONDS_PER_DAY - EPOCH_DAYS) as i32
        })),
        TemporalKind::Timestamp => {
            Arc::new(unary::<_, _, TimestampMicrosecondType>(numbers, |v| {
                ((v - the reference program_EPOCH_OFFSET_SECONDS) * MICROS_PER_SECOND) as i64
            }))
        }
        TemporalKind::Duration => Arc::new(unary::<_, _, DurationMicrosecondType>(numbers, |v| {
            (v * MICROS_PER_SECOND) as i64
        })),
    };
    let in_range = |v: f64| match kind {
        TemporalKind::Duration => v.abs() <= MAX_DURATION,
        _ => DATE_RANGE.contains(&v),
    };
    let reloaded = model_column(&temporal).map_err(Error::Invalid)?;
    let reloaded = reloaded.as_primitive::<Float64Type>();
    let changed = numbers.iter().zip(reloaded).find_map(|pair| match pair {
        (Some(v), Some(back)) if v.to_bits() != back.to_bits() || !in_range(v) => Some(v),
        _ => None,
    });
    match changed {
        Some(value) => Err(Error::Invalid(format!(
            "{}: {value} cannot be saved exactly as {}",
            def.name, def.format
        ))),
        None => Ok(temporal),
    }
}

fn model_value(value: ambers::Value) -> Value {
    match value {
        ambers::Value::Numeric(v) => Value::Number(v),
        ambers::Value::String(s) => Value::String(s.trim_end_matches(' ').to_string()),
    }
}

fn model_missing(specs: Option<Vec<ambers::MissingSpec>>) -> MissingValues {
    let (mut numbers, mut strings, mut range) = (Vec::new(), Vec::new(), None);
    for spec in specs.into_iter().flatten() {
        match spec {
            ambers::MissingSpec::Value(v) => numbers.push(v),
            ambers::MissingSpec::Range { lo, hi } => range = Some((lo, hi)),
            ambers::MissingSpec::StringValue(s) => {
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

fn writer_missing(missing: &MissingValues) -> Vec<ambers::MissingSpec> {
    use ambers::MissingSpec;
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
