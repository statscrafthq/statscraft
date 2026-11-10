pub mod encoding_record;
pub mod float_info;
pub mod integer_info;
pub mod long_string_labels;
pub mod long_string_missing;
pub mod long_var_names;
pub mod mr_sets;
pub mod var_attributes;
pub mod var_display;
pub mod very_long_strings;

use std::io::Read;

use crate::constants::*;
use crate::error::{Result, SavError};
use crate::io_utils::SavReader;

/// Header for a type 7 (info) record.
#[derive(Debug, Clone)]
pub struct InfoRecordHeader {
    pub subtype: i32,
    pub size: i32,
    pub count: i32,
}

impl InfoRecordHeader {
    /// Parse the info record header. The record type (7) has already been read.
    pub fn parse<R: Read>(reader: &mut SavReader<R>) -> Result<InfoRecordHeader> {
        let subtype = reader.read_i32()?;
        let size = reader.read_i32()?;
        let count = reader.read_i32()?;
        Ok(InfoRecordHeader {
            subtype,
            size,
            count,
        })
    }

    /// Total data bytes for this info record; corrupt sizes are an error.
    pub fn data_len(&self) -> Result<usize> {
        let size = usize::try_from(self.size).ok();
        let count = usize::try_from(self.count).ok();
        size.zip(count)
            .and_then(|(size, count)| size.checked_mul(count))
            .ok_or_else(|| {
                SavError::InvalidDictionary(format!(
                    "info record {} declares {} elements of {} bytes",
                    self.subtype, self.count, self.size
                ))
            })
    }
}

/// Parsed info record data.
#[derive(Debug)]
#[allow(dead_code)]
pub enum InfoRecord {
    IntegerInfo(integer_info::IntegerInfo),
    FloatInfo(float_info::FloatInfo),
    VarDisplay(Vec<var_display::VarDisplayEntry>),
    LongNames(Vec<(crate::io_utils::RawText, crate::io_utils::RawText)>),
    VeryLongStrings(Vec<(crate::io_utils::RawText, usize)>),
    Encoding(String),
    LongStringLabels(Vec<long_string_labels::LongStringLabelSet>),
    LongStringMissing(Vec<long_string_missing::LongStringMissingEntry>),
    MrSets(Vec<mr_sets::RawMrSet>),
    VarAttributes(Vec<var_attributes::VarAttributeSet>),
    FileAttributes(Vec<(crate::io_utils::RawText, Vec<crate::io_utils::RawText>)>),
    VariableSets(Vec<(crate::io_utils::RawText, Vec<crate::io_utils::RawText>)>),
    Unknown { subtype: i32 },
}

/// Parse a type 7 info record based on its subtype.
pub fn parse_info_record<R: Read>(
    reader: &mut SavReader<R>,
    header: &InfoRecordHeader,
) -> Result<InfoRecord> {
    let data_len = header.data_len()?;

    match header.subtype {
        INFO_MR_SETS => {
            let data = reader.read_bytes(data_len)?;
            let sets = mr_sets::parse_mr_sets(&data);
            Ok(InfoRecord::MrSets(sets))
        }
        INFO_MR_SETS_V2 => {
            let data = reader.read_bytes(data_len)?;
            let sets = mr_sets::parse_mr_sets_v2(&data);
            Ok(InfoRecord::MrSets(sets))
        }
        INFO_INTEGER => {
            let info = integer_info::IntegerInfo::parse(reader)?;
            Ok(InfoRecord::IntegerInfo(info))
        }
        INFO_FLOAT => {
            let info = float_info::FloatInfo::parse(reader)?;
            Ok(InfoRecord::FloatInfo(info))
        }
        // elements are 4-byte integers; any other size is skipped as unknown
        INFO_VAR_DISPLAY if header.size == 4 => {
            let entries = var_display::parse_var_display(reader, data_len / 4)?;
            Ok(InfoRecord::VarDisplay(entries))
        }
        INFO_LONG_NAMES => {
            let data = reader.read_bytes(data_len)?;
            let names = long_var_names::parse_long_var_names(&data);
            Ok(InfoRecord::LongNames(names))
        }
        INFO_VERY_LONG_STRINGS => {
            let data = reader.read_bytes(data_len)?;
            let entries = very_long_strings::parse_very_long_strings(&data);
            Ok(InfoRecord::VeryLongStrings(entries))
        }
        INFO_ENCODING => {
            let data = reader.read_bytes(data_len)?;
            let name = encoding_record::parse_encoding_record(&data);
            Ok(InfoRecord::Encoding(name))
        }
        INFO_LONG_STRING_LABELS => {
            let data = reader.read_bytes(data_len)?;
            let labels = long_string_labels::parse_long_string_labels(&data)?;
            Ok(InfoRecord::LongStringLabels(labels))
        }
        INFO_LONG_STRING_MISSING => {
            let data = reader.read_bytes(data_len)?;
            let entries = long_string_missing::parse_long_string_missing(&data)?;
            Ok(InfoRecord::LongStringMissing(entries))
        }
        INFO_VAR_ATTRIBUTES => {
            let data = reader.read_bytes(data_len)?;
            let attrs = var_attributes::parse_var_attributes(&data);
            Ok(InfoRecord::VarAttributes(attrs))
        }
        INFO_FILE_ATTRIBUTES => {
            let data = reader.read_bytes(data_len)?;
            Ok(InfoRecord::FileAttributes(
                var_attributes::parse_attributes(&data),
            ))
        }
        INFO_VARIABLE_SETS => {
            let data = reader.read_bytes(data_len)?;
            Ok(InfoRecord::VariableSets(parse_variable_sets(&data)))
        }
        _ => {
            // Unknown subtype -- skip the data
            reader.skip(data_len)?;
            Ok(InfoRecord::Unknown {
                subtype: header.subtype,
            })
        }
    }
}

/// Subtype 5: one variable set per line, `name= var1 var2 ...`, the variables by short name.
fn parse_variable_sets(
    data: &[u8],
) -> Vec<(crate::io_utils::RawText, Vec<crate::io_utils::RawText>)> {
    data.split(|&b| b == b'\n')
        .filter_map(|line| {
            let eq = line.iter().position(|&b| b == b'=')?;
            let (name, rest) = line.split_at(eq);
            let members = rest[1..]
                .split(|b| b.is_ascii_whitespace() || *b == 0)
                .filter(|s| !s.is_empty())
                .map(<[u8]>::to_vec)
                .collect();
            Some((crate::io_utils::trim_ascii_nul(name).to_vec(), members))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn variable_sets_are_split_into_names_and_members() {
        let sets = super::parse_variable_sets(b"demo= respid age\nmore=  a1 b2\n\n");
        let text = |s: &[u8]| String::from_utf8(s.to_vec()).unwrap();
        let sets: Vec<(String, Vec<String>)> = sets
            .iter()
            .map(|(n, m)| (text(n), m.iter().map(|v| text(v)).collect()))
            .collect();
        assert_eq!(
            sets,
            [
                (
                    "demo".to_string(),
                    vec!["respid".to_string(), "age".to_string()]
                ),
                ("more".to_string(), vec!["a1".to_string(), "b2".to_string()]),
            ]
        );
    }
}
