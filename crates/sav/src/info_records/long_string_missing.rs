use crate::error::Result;
use crate::io_utils::{self, RawText, take, take_len};

/// Missing value specification for a long string variable.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LongStringMissingEntry {
    /// Variable name bytes, undecoded (decoded in `resolve_dictionary`).
    pub var_name: RawText,
    /// Number of missing values.
    pub n_values: u8,
    /// Missing value byte arrays.
    pub values: Vec<Vec<u8>>,
}

/// Parse subtype 22: long string missing values.
///
/// Format (for each variable):
///   4-byte var_name_length
///   var_name bytes
///   1-byte n_missing_values
///   4-byte value_length (per SAV spec: the width)
///   For each missing value:
///     value_length bytes of value
pub fn parse_long_string_missing(data: &[u8]) -> Result<Vec<LongStringMissingEntry>> {
    let mut result = Vec::new();
    let mut pos = 0;

    // A record that ends early or holds a corrupt length keeps what came before it.
    while let Some(name_len) = take_len(data, &mut pos) {
        let Some(name) = take(data, &mut pos, name_len) else {
            break;
        };
        let var_name = io_utils::trim_ascii_nul(name).to_vec();
        let (Some(&[n_values]), Some(value_len)) = (take(data, &mut pos, 1), take_len(data, &mut pos))
        else {
            break;
        };

        let mut values = Vec::with_capacity(n_values as usize);
        for _ in 0..n_values {
            let Some(value) = take(data, &mut pos, value_len) else {
                break;
            };
            values.push(value.to_vec());
        }

        result.push(LongStringMissingEntry {
            var_name,
            n_values,
            values,
        });
    }

    Ok(result)
}

