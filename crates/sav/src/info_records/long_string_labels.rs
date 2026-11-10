use crate::error::Result;
use crate::io_utils::{self, RawText, take, take_len};

/// A set of value labels for a long string variable.
#[derive(Debug, Clone)]
pub struct LongStringLabelSet {
    /// Variable name bytes, undecoded (decoded in `resolve_dictionary`).
    pub var_name: RawText,
    /// (value, label) pairs.
    pub labels: Vec<(Vec<u8>, Vec<u8>)>,
}

/// Parse subtype 21: long string value labels.
///
/// Format (for each variable):
///   4-byte var_name_length
///   var_name bytes
///   4-byte var_width
///   4-byte label_count
///   For each label:
///     4-byte value_length
///     value bytes
///     4-byte label_length
///     label bytes
pub fn parse_long_string_labels(data: &[u8]) -> Result<Vec<LongStringLabelSet>> {
    let mut result = Vec::new();
    let mut pos = 0;

    // A record that ends early or holds a corrupt length keeps what came before it.
    while let Some(name_len) = take_len(data, &mut pos) {
        let Some(name) = take(data, &mut pos, name_len) else {
            break;
        };
        let var_name = io_utils::trim_ascii_nul(name).to_vec();
        // variable width: not needed, the values carry their own length
        let (Some(_), Some(label_count)) = (take(data, &mut pos, 4), take_len(data, &mut pos))
        else {
            break;
        };

        let mut labels = Vec::with_capacity(label_count.min(4096));
        for _ in 0..label_count {
            let value = take_len(data, &mut pos).and_then(|n| take(data, &mut pos, n));
            let Some(value) = value.map(<[u8]>::to_vec) else {
                break;
            };
            let label = take_len(data, &mut pos).and_then(|n| take(data, &mut pos, n));
            let Some(label) = label.map(<[u8]>::to_vec) else {
                break;
            };
            labels.push((value, label));
        }

        result.push(LongStringLabelSet { var_name, labels });
    }

    Ok(result)
}

