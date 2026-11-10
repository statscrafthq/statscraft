use arrow::datatypes::{DataType, Field, Schema};

use crate::constants::VarType;
use crate::dictionary::ResolvedDictionary;
use crate::variable::VariableRecord;

/// Determine the Arrow DataType for a resolved variable. Date and time variables are Float64
/// too: the stored number, with the meaning given by the format.
pub fn var_to_arrow_type(var: &VariableRecord) -> DataType {
    match &var.var_type {
        VarType::Numeric => DataType::Float64,
        VarType::String(_) => DataType::Utf8View,
    }
}

/// Build an Arrow Schema from the resolved dictionary.
pub fn build_schema(dict: &ResolvedDictionary) -> Schema {
    let fields: Vec<Field> = dict
        .variables
        .iter()
        .map(|var| Field::new(&var.long_name, var_to_arrow_type(var), true))
        .collect();

    Schema::new(fields)
}
