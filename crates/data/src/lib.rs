//! Dataset and dictionary model, with reading and writing of `.sav` / `.zsav` files.

mod edit;
mod por;
mod retype;
mod sav;

pub use edit::is_valid_name;
pub use por::portable_names;
pub use retype::{NewValues, cut_to};

use arrow::array::{Array, ArrayRef, AsArray, Float64Array, StringViewArray};
use serde::{Serialize, Serializer};
use serde_json::{Map, Number, Value as Json};
use std::sync::Arc;

/// A dataset held in memory: one Arrow column per variable plus the dictionary.
///
/// Every column is either Float64 (null = system-missing) or a UTF-8 string column. Date and
/// time variables are Float64 too: seconds since 1582-10-14 00:00:00 (dates, datetimes) or
/// seconds (durations), with the display format in the dictionary.
///
/// Cloning is cheap: columns and dictionary are reference counted. Changing a clone copies the
/// dictionary at most, and no column that the change does not replace.
#[derive(Clone, PartialEq)]
pub struct Dataset {
    columns: Vec<ArrayRef>,
    case_count: usize,
    dict: Arc<Dictionary>,
}

#[derive(Clone, PartialEq)]
struct Dictionary {
    encoding: String,
    compression: &'static str,
    file_label: Option<String>,
    weight_variable: Option<String>,
    /// SPLIT FILE and FILTER: they belong to the dataset but are not written to a file.
    split: Option<SplitFile>,
    filter_variable: Option<String>,
    variables: Vec<VariableDef>,
    multiple_response_sets: Vec<MultipleResponseSet>,
    variable_sets: Vec<VariableSet>,
    /// Data file attributes: name and values, in file order.
    attributes: Vec<(String, Vec<String>)>,
    warnings: Vec<String>,
    /// Document lines of the file, carried through to the next write.
    documents: Vec<String>,
}

#[derive(Clone, PartialEq)]
struct VariableDef {
    name: String,
    label: Option<String>,
    format: String,
    /// Width in bytes of a string variable, `None` for a numeric one.
    width: Option<usize>,
    measure: Measure,
    value_labels: Vec<(Value, String)>,
    missing: MissingValues,
    extras: sav::VariableExtras,
}

#[derive(Debug)]
pub enum Error {
    /// Reading or writing a file failed; the message starts with the path.
    File(String),
    UnknownVariable(String),
    DuplicateName(String),
    InvalidName(String),
    /// The named variable is not of the type the request needs.
    WrongType {
        variable: String,
        expected: &'static str,
    },
    /// A value vector or case mask does not have one entry per case.
    LengthMismatch {
        expected: usize,
        actual: usize,
    },
    /// Any other request the dataset cannot honour; the message says why.
    Invalid(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::File(message) | Error::Invalid(message) => f.write_str(message),
            Error::UnknownVariable(name) => write!(f, "unknown variable: {name}"),
            Error::DuplicateName(name) => write!(f, "variable name already in use: {name}"),
            Error::InvalidName(name) => write!(f, "invalid variable name: {name}"),
            Error::WrongType { variable, expected } => {
                write!(f, "{variable} is not a {expected} variable")
            }
            Error::LengthMismatch { expected, actual } => {
                write!(f, "expected {expected} values, got {actual}")
            }
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    Unknown,
    Nominal,
    Ordinal,
    Scale,
}

impl Measure {
    pub fn as_str(self) -> &'static str {
        match self {
            Measure::Unknown => "unknown",
            Measure::Nominal => "nominal",
            Measure::Ordinal => "ordinal",
            Measure::Scale => "scale",
        }
    }
}

/// A data value as it appears in the dictionary: the key of a value label.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    /// Right-trimmed text.
    String(String),
}

/// The user-missing definition of one variable.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum MissingValues {
    #[default]
    None,
    /// One to three discrete numbers.
    Numbers(Vec<f64>),
    /// An inclusive range, optionally with one discrete number outside it.
    Range {
        low: f64,
        high: f64,
        extra: Option<f64>,
    },
    /// One to three discrete strings, right-trimmed, at most 8 bytes each.
    Strings(Vec<String>),
}

impl MissingValues {
    fn contains_number(&self, value: f64) -> bool {
        match self {
            MissingValues::Numbers(discrete) => discrete.contains(&value),
            MissingValues::Range { low, high, extra } => {
                (*low <= value && value <= *high) || *extra == Some(value)
            }
            _ => false,
        }
    }

    fn contains_str(&self, value: &str) -> bool {
        let value = value.trim_end_matches(' ');
        matches!(self, MissingValues::Strings(discrete) if discrete.iter().any(|d| d == value))
    }
}

/// Read access to one variable: its dictionary entry and its values.
#[derive(Clone, Copy, PartialEq)]
pub struct Variable<'a> {
    def: &'a VariableDef,
    column: &'a ArrayRef,
}

impl<'a> Variable<'a> {
    pub fn name(&self) -> &'a str {
        &self.def.name
    }

    pub fn label(&self) -> Option<&'a str> {
        self.def.label.as_deref()
    }

    /// Display format, for example "F8.2", "DATE11" or "A10".
    pub fn format(&self) -> &'a str {
        &self.def.format
    }

    pub fn measure(&self) -> Measure {
        self.def.measure
    }

    /// Value labels in file order.
    pub fn value_labels(&self) -> &'a [(Value, String)] {
        &self.def.value_labels
    }

    pub fn missing_values(&self) -> &'a MissingValues {
        &self.def.missing
    }

    pub fn is_string(&self) -> bool {
        self.def.width.is_some()
    }

    /// Width in bytes of a string variable, `None` for a numeric one.
    pub fn string_width(&self) -> Option<usize> {
        self.def.width
    }

    pub fn values(&self) -> Values<'a> {
        match self.def.width {
            None => Values::Numeric(NumericValues(self.column.as_primitive())),
            Some(_) => Values::String(StringValues(self.column.as_string_view())),
        }
    }

    pub fn numeric(&self) -> Result<NumericValues<'a>, Error> {
        match self.values() {
            Values::Numeric(values) => Ok(values),
            Values::String(_) => Err(self.def.wrong_type("numeric")),
        }
    }

    pub fn string(&self) -> Result<StringValues<'a>, Error> {
        match self.values() {
            Values::String(values) => Ok(values),
            Values::Numeric(_) => Err(self.def.wrong_type("string")),
        }
    }

    /// True when `value` is one of the variable's user-missing numbers or lies in its range.
    pub fn is_user_missing(&self, value: f64) -> bool {
        self.def.missing.contains_number(value)
    }

    /// True when `value`, right-trimmed, is one of the variable's user-missing strings.
    pub fn is_user_missing_str(&self, value: &str) -> bool {
        self.def.missing.contains_str(value)
    }

    /// Whether this variable, as the FILTER variable, keeps `case`: a value that is neither 0
    /// nor missing. A string variable keeps no case.
    pub fn keeps(&self, case: usize) -> bool {
        let value = self.numeric().ok().and_then(|values| values.get(case));
        value.is_some_and(|v| v != 0.0 && !self.is_user_missing(v))
    }
}

impl VariableDef {
    fn wrong_type(&self, expected: &'static str) -> Error {
        Error::WrongType {
            variable: self.name.clone(),
            expected,
        }
    }
}

/// The values of one variable, one per case.
#[derive(Clone, Copy)]
pub enum Values<'a> {
    Numeric(NumericValues<'a>),
    String(StringValues<'a>),
}

/// Values of a numeric variable; `None` is system-missing.
#[derive(Clone, Copy)]
pub struct NumericValues<'a>(&'a Float64Array);

impl<'a> NumericValues<'a> {
    /// Panics when `case` is out of range.
    pub fn get(&self, case: usize) -> Option<f64> {
        self.0.is_valid(case).then(|| self.0.value(case))
    }

    pub fn iter(&self) -> impl Iterator<Item = Option<f64>> + 'a {
        self.0.iter()
    }
}

/// Values of a string variable, without the padding spaces.
#[derive(Clone, Copy)]
pub struct StringValues<'a>(&'a StringViewArray);

impl<'a> StringValues<'a> {
    /// Panics when `case` is out of range.
    pub fn get(&self, case: usize) -> &'a str {
        self.0.value(case).trim_end_matches(' ')
    }

    pub fn iter(&self) -> impl Iterator<Item = &'a str> + 'a {
        let values = *self;
        (0..self.0.len()).map(move |case| values.get(case))
    }
}

/// What `statscraft describe` reports: the dictionary plus basic counts.
#[derive(Serialize)]
pub struct Description {
    pub encoding: String,
    pub compression: &'static str,
    pub case_count: usize,
    pub variable_count: usize,
    pub file_label: Option<String>,
    pub weight_variable: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub split_file: Option<SplitFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter_variable: Option<String>,
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

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct MultipleResponseSet {
    /// With the leading "$".
    pub name: String,
    /// "dichotomy" or "category".
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub label: String,
    /// The counted value of a dichotomy set, as the file states it.
    #[serde(serialize_with = "text_or_number")]
    pub counted_value: Option<String>,
    pub variables: Vec<String>,
    /// CATEGORYLABELS=COUNTEDVALUES: a dichotomy set labels its categories with the value
    /// label of the counted value in each variable, not with the variable labels.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub counted_value_labels: bool,
    /// LABELSOURCE=VARLABEL: `label` was taken from the variable labels of the set (only with
    /// `counted_value_labels`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub label_from_variables: bool,
}

/// SPLIT FILE: procedures run on each group of adjacent cases with equal values of the
/// variables.
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct SplitFile {
    pub variables: Vec<String>,
    /// The groups share each table; otherwise every group has tables of its own.
    pub layered: bool,
}

/// A named list of variables, a shortcut for choosing variables in a user interface.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct VariableSet {
    pub name: String,
    pub variables: Vec<String>,
}

impl Dataset {
    /// A dataset with `case_count` cases and no variables yet, as UTF-8 text. Variables come
    /// from `set_numeric_values` and `set_string_values`.
    pub fn new(case_count: usize) -> Self {
        Self {
            columns: Vec::new(),
            case_count,
            dict: Arc::new(Dictionary {
                encoding: "UTF-8".to_string(),
                compression: "none",
                file_label: None,
                weight_variable: None,
                split: None,
                filter_variable: None,
                variables: Vec::new(),
                multiple_response_sets: Vec::new(),
                variable_sets: Vec::new(),
                attributes: Vec::new(),
                warnings: Vec::new(),
                documents: Vec::new(),
            }),
        }
    }

    pub fn case_count(&self) -> usize {
        self.case_count
    }

    pub fn variable_count(&self) -> usize {
        self.columns.len()
    }

    pub fn variable_names(&self) -> impl Iterator<Item = &str> {
        self.dict.variables.iter().map(|v| v.name.as_str())
    }

    /// Position of a variable: an exact name match wins, otherwise case is ignored.
    pub fn variable_index(&self, name: &str) -> Option<usize> {
        // shortcut: linear scan per lookup, add a name index when datasets with thousands of
        // variables make lookups show up in a profile
        let variables = &self.dict.variables;
        variables
            .iter()
            .position(|v| v.name == name)
            .or_else(|| variables.iter().position(|v| same_name(&v.name, name)))
    }

    /// Looks a variable up like `variable_index` does.
    pub fn variable(&self, name: &str) -> Result<Variable<'_>, Error> {
        self.index_of(name).map(|index| self.variable_at(index))
    }

    /// Panics when `index` is out of range.
    pub fn variable_at(&self, index: usize) -> Variable<'_> {
        Variable {
            def: &self.dict.variables[index],
            column: &self.columns[index],
        }
    }

    pub fn variables(&self) -> impl Iterator<Item = Variable<'_>> {
        (0..self.columns.len()).map(|index| self.variable_at(index))
    }

    pub fn weight_variable(&self) -> Option<&str> {
        self.dict.weight_variable.as_deref()
    }

    /// The SPLIT FILE in effect, with the variable names as the dictionary spells them.
    pub fn split_file(&self) -> Option<&SplitFile> {
        self.dict.split.as_ref()
    }

    /// The FILTER variable in effect.
    pub fn filter_variable(&self) -> Option<&str> {
        self.dict.filter_variable.as_deref()
    }

    pub fn multiple_response_sets(&self) -> &[MultipleResponseSet] {
        &self.dict.multiple_response_sets
    }

    pub fn variable_sets(&self) -> &[VariableSet] {
        &self.dict.variable_sets
    }

    /// Data file attributes: name and its values, in file order.
    pub fn attributes(&self) -> &[(String, Vec<String>)] {
        &self.dict.attributes
    }

    pub fn file_label(&self) -> Option<&str> {
        self.dict.file_label.as_deref()
    }

    /// Character encoding of the source file.
    pub fn encoding(&self) -> &str {
        &self.dict.encoding
    }

    /// "none", "bytecode" or "zlib": the compression of the source file.
    pub fn compression(&self) -> &'static str {
        self.dict.compression
    }

    /// Document lines, in file order.
    pub fn documents(&self) -> &[String] {
        &self.dict.documents
    }

    /// What the reader found wrong with the file this dataset was loaded from, for example
    /// date values it could not represent and turned into system-missing.
    pub fn warnings(&self) -> &[String] {
        &self.dict.warnings
    }

    fn index_of(&self, name: &str) -> Result<usize, Error> {
        self.variable_index(name)
            .ok_or_else(|| Error::UnknownVariable(name.to_string()))
    }

    pub fn describe(&self) -> Description {
        let dict = &*self.dict;
        Description {
            encoding: dict.encoding.clone(),
            compression: dict.compression,
            case_count: self.case_count,
            variable_count: self.columns.len(),
            file_label: dict.file_label.clone(),
            weight_variable: dict.weight_variable.clone(),
            split_file: dict.split.clone(),
            filter_variable: dict.filter_variable.clone(),
            variables: self.variables().map(describe_variable).collect(),
            multiple_response_sets: dict.multiple_response_sets.clone(),
        }
    }
}

fn describe_variable(variable: Variable<'_>) -> VariableDescription {
    let def = variable.def;
    let (mut missing_values, mut missing_range) = (Vec::new(), None);
    match &def.missing {
        MissingValues::None => {}
        MissingValues::Numbers(discrete) => missing_values.extend(discrete.iter().map(number)),
        MissingValues::Range { low, high, extra } => {
            missing_values.extend(extra.iter().map(number));
            missing_range = Some([*low, *high]);
        }
        MissingValues::Strings(discrete) => {
            missing_values.extend(discrete.iter().map(|s| Json::from(s.as_str())))
        }
    }
    let (n_system_missing, n_user_missing) = match variable.values() {
        Values::Numeric(values) => (
            values.0.null_count(),
            values
                .iter()
                .flatten()
                .filter(|v| def.missing.contains_number(*v))
                .count(),
        ),
        Values::String(values) => (
            0,
            values
                .iter()
                .filter(|v| def.missing.contains_str(v))
                .count(),
        ),
    };
    VariableDescription {
        name: def.name.clone(),
        kind: if def.width.is_some() {
            "string"
        } else {
            "numeric"
        },
        width: def.width,
        label: def.label.clone(),
        format: def.format.clone(),
        measure: def.measure.as_str(),
        value_labels: def
            .value_labels
            .iter()
            .map(|(value, label)| {
                let key = match value {
                    Value::Number(v) => number(v).to_string(),
                    Value::String(s) => s.clone(),
                };
                (key, Json::from(label.as_str()))
            })
            .collect(),
        missing_values,
        missing_range,
        n_valid: variable.column.len() - n_system_missing - n_user_missing,
        n_system_missing,
        n_user_missing,
    }
}

/// Variable names are case-insensitive.
fn same_name(a: &str, b: &str) -> bool {
    // most names are ASCII; this path keeps a lookup among 10 000 variables fast
    if a.is_ascii() && b.is_ascii() {
        return a.eq_ignore_ascii_case(b);
    }
    a.chars()
        .flat_map(char::to_lowercase)
        .eq(b.chars().flat_map(char::to_lowercase))
}

/// Whole numbers print as integers ("1", not "1.0"), like the values a user types.
fn number(v: &f64) -> Json {
    if v.fract() == 0.0 && v.abs() < 9.0e15 {
        Json::from(*v as i64)
    } else {
        Number::from_f64(*v).map_or(Json::Null, Json::Number)
    }
}

fn text_or_number<S: Serializer>(value: &Option<String>, serializer: S) -> Result<S::Ok, S::Error> {
    value
        .as_deref()
        .map(|s| {
            s.trim()
                .parse::<f64>()
                .map_or_else(|_| Json::from(s), |v| number(&v))
        })
        .serialize(serializer)
}

fn non_empty(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_string())
}
