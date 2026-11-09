//! Dataset and dictionary model, and reading `.sav` / `.zsav` files.

use arrow::array::{Array, Float64Array, RecordBatch, StringArray};
use arrow::datatypes::DataType;
use serde::Serialize;
use serde_json::{Map, Number, Value as Json};
use std::path::Path;

/// A dataset held in memory.
pub struct Dataset {
    // shortcut: columns stay in the Arrow form the reader returns (date variables arrive as
    // Arrow temporal types, not as the stored numbers); replace with our own numeric columns
    // when transformations need date arithmetic (stage E3).
    batch: RecordBatch,
    meta: ambers::SavMetadata,
}

#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// What `statscraft describe` reports: the dictionary plus basic counts.
#[derive(Serialize)]
pub struct Description {
    pub encoding: String,
    pub compression: &'static str,
    pub case_count: usize,
    pub variable_count: usize,
    pub file_label: Option<String>,
    pub weight_variable: Option<String>,
    pub variables: Vec<VariableDescription>,
    pub multiple_response_sets: Vec<MultipleResponseSet>,
}

#[derive(Serialize)]
pub struct VariableDescription {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<usize>,
    pub label: Option<String>,
    pub format: String,
    pub measure: &'static str,
    /// Value (as text) to label, in file order.
    pub value_labels: Map<String, Json>,
    pub missing_values: Vec<Json>,
    pub missing_range: Option<[f64; 2]>,
    pub n_valid: usize,
    pub n_system_missing: usize,
    pub n_user_missing: usize,
}

#[derive(Serialize)]
pub struct MultipleResponseSet {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub label: String,
    pub counted_value: Option<Json>,
    pub variables: Vec<String>,
}

impl Dataset {
    pub fn read_sav(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let (batch, meta) =
            ambers::read_sav(path).map_err(|e| Error(format!("{}: {e}", path.display())))?;
        Ok(Self { batch, meta })
    }

    pub fn describe(&self) -> Description {
        let m = &self.meta;
        let variables = m
            .variable_names
            .iter()
            .enumerate()
            .map(|(i, name)| self.describe_variable(i, name))
            .collect();
        let multiple_response_sets = m
            .mr_sets
            .values()
            .map(|s| MultipleResponseSet {
                // the reader strips the "$" that set names carry in syntax
                name: format!("${}", s.name),
                kind: match s.mr_type {
                    ambers::MrType::MultipleDichotomy => "dichotomy",
                    ambers::MrType::MultipleCategory => "category",
                },
                label: s.label.clone(),
                counted_value: s.counted_value.as_deref().map(text_or_number),
                variables: s.variables.clone(),
            })
            .collect();
        Description {
            encoding: m.file_encoding.clone(),
            compression: match m.compression {
                ambers::Compression::None => "none",
                ambers::Compression::Bytecode => "bytecode",
                ambers::Compression::Zlib => "zlib",
            },
            case_count: self.batch.num_rows(),
            variable_count: self.batch.num_columns(),
            file_label: non_empty(m.file_label.trim_end()),
            weight_variable: m.weight_variable.clone(),
            variables,
            multiple_response_sets,
        }
    }

    fn describe_variable(&self, index: usize, name: &str) -> VariableDescription {
        let m = &self.meta;
        let column = self.batch.column(index);
        let format = m.variable_formats.get(name).cloned().unwrap_or_default();
        let is_string = format.starts_with('A') && !format.starts_with("ADATE");

        let mut missing_values = Vec::new();
        let mut missing_range = None;
        for spec in m.variable_missing_values.get(name).into_iter().flatten() {
            match spec {
                ambers::MissingSpec::Value(v) => missing_values.push(number(*v)),
                ambers::MissingSpec::Range { lo, hi } => missing_range = Some([*lo, *hi]),
                ambers::MissingSpec::StringValue(s) => {
                    missing_values.push(Json::from(s.trim_end()))
                }
            }
        }

        let n_system_missing = if is_string { 0 } else { column.null_count() };
        let n_user_missing = count_user_missing(column, &missing_values, missing_range);
        let value_labels = m
            .variable_value_labels
            .get(name)
            .into_iter()
            .flatten()
            .map(|(value, label)| {
                let key = match value {
                    ambers::Value::Numeric(v) => number(*v).to_string(),
                    ambers::Value::String(s) => s.trim_end().to_string(),
                };
                (key, Json::from(label.as_str()))
            })
            .collect();

        VariableDescription {
            name: name.to_string(),
            kind: if is_string { "string" } else { "numeric" },
            // the declared width is in the format (A10); the reader's storage width is padded
            width: is_string.then(|| format[1..].parse().unwrap_or(0)),
            label: m.variable_labels.get(name).and_then(|l| non_empty(l)),
            format,
            measure: match m.variable_measures.get(name) {
                Some(ambers::Measure::Nominal) => "nominal",
                Some(ambers::Measure::Ordinal) => "ordinal",
                Some(ambers::Measure::Scale) => "scale",
                _ => "unknown",
            },
            value_labels,
            missing_values,
            missing_range,
            n_valid: column.len() - n_system_missing - n_user_missing,
            n_system_missing,
            n_user_missing,
        }
    }
}

fn count_user_missing(column: &dyn Array, discrete: &[Json], range: Option<[f64; 2]>) -> usize {
    if discrete.is_empty() && range.is_none() {
        return 0;
    }
    match column.data_type() {
        DataType::Float64 => {
            let values = column.as_any().downcast_ref::<Float64Array>().unwrap();
            let discrete: Vec<f64> = discrete.iter().filter_map(Json::as_f64).collect();
            values
                .iter()
                .flatten()
                .filter(|v| {
                    discrete.contains(v) || range.is_some_and(|[lo, hi]| lo <= *v && *v <= hi)
                })
                .count()
        }
        DataType::Utf8 => {
            let values = column.as_any().downcast_ref::<StringArray>().unwrap();
            values
                .iter()
                .flatten()
                .filter(|v| discrete.iter().any(|d| d.as_str() == Some(v.trim_end())))
                .count()
        }
        // shortcut: user-missing values of date/time variables are not counted, see `Dataset::batch`
        _ => 0,
    }
}

/// Whole numbers print as integers ("1", not "1.0"), like the values a user types.
fn number(v: f64) -> Json {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        Json::from(v as i64)
    } else {
        Number::from_f64(v).map_or(Json::Null, Json::Number)
    }
}

fn text_or_number(s: &str) -> Json {
    s.trim()
        .parse::<f64>()
        .map_or_else(|_| Json::from(s), number)
}

fn non_empty(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_string())
}
