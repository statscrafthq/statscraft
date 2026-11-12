//! GET TRANSLATE: a tab-delimited text file, a dBASE file or an old Excel workbook as the new
//! active dataset.
//!
//! Choices where the manual is silent:
//! - TAB: a line break (LF, CR LF or CR) ends a row and a tab ends a value; quotes are part of
//!   the value. An empty first data value makes the column numeric, as an empty spreadsheet
//!   cell takes the default format. A number is what reads as one in plain notation (sign,
//!   digits, a decimal point, an exponent); its format is F8.2, or F with the width and
//!   decimals of the first value when that is wider. Text is UTF-8 when it is valid UTF-8, else
//!   the system's legacy code page.
//! - Repeated field names get 2, 3 and so on after the first; a name that does not start with
//!   a letter gets a V in front.
//! - DBF: D_R is added only when some record is marked deleted, after the fields. Text uses the
//!   code page the file names (its language driver) when Statscraft knows it, else as TAB.
//!   Numeric fields that do not read as numbers are system-missing, with a warning.
//! - XLS is read as GET DATA reads a workbook (the kind of a column comes from all its values),
//!   from the first sheet; without RANGE from column A and the first row that has a value.

use crate::expr::{DAYS_TO_1970, days_from_civil};
use crate::{Engine, Error, Table};
use statscraft_data::{Dataset, cut_to, is_valid_name};
use statscraft_syntax::{
    GetData, GetTranslate, Sheet, Spreadsheet, TranslateSource, decode_syntax,
};
use std::io::Cursor;

const RESERVED: [&str; 13] = [
    "ALL", "AND", "BY", "EQ", "GE", "GT", "LE", "LT", "NE", "NOT", "OR", "TO", "WITH",
];

/// Field names made valid variable names as the manual says: other characters become _, a
/// reserved word gets $, a repeated name a number. An empty name is None.
fn field_names<'a>(names: impl IntoIterator<Item = &'a str>) -> Vec<Option<String>> {
    let mut used: Vec<String> = Vec::new();
    names
        .into_iter()
        .map(|name| {
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            let mut clean: String = name
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || "_.$#@".contains(c) {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(&clean)) {
                clean.push('$');
            }
            if !clean.starts_with(char::is_alphabetic) {
                clean.insert(0, 'V');
            }
            let clean = cut_to(clean.trim_end_matches('.'), 60)
                .trim_end_matches('.')
                .to_string();
            let mut unique = clean.clone();
            for n in 2.. {
                if !used.iter().any(|u| u.eq_ignore_ascii_case(&unique)) {
                    break;
                }
                unique = format!("{clean}{n}");
            }
            used.push(unique.clone());
            Some(unique)
        })
        .collect()
}

/// Column letters: 0 is A, 26 is AA.
fn column_letters(mut column: u32) -> String {
    let mut letters = Vec::new();
    loop {
        letters.push(b'A' + (column % 26) as u8);
        if column < 26 {
            break;
        }
        column = column / 26 - 1;
    }
    letters.reverse();
    String::from_utf8(letters).expect("ASCII")
}

/// A value of a tab-delimited file that is a plain number.
fn plain_number(text: &str) -> Option<f64> {
    let plain = !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || "+-.eE".contains(c))
        && text.contains(|c: char| c.is_ascii_digit());
    plain.then(|| text.parse().ok()).flatten()
}

fn tab(
    text: &str,
    named: bool,
    range: Option<(u32, u32, u32, u32)>,
) -> Result<(Dataset, Vec<String>), Error> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    let mut rows: Vec<Vec<&str>> = lines.iter().map(|l| l.split('\t').collect()).collect();
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    let (r0, c0, r1, c1) = range.unwrap_or((0, 0, u32::MAX, width.max(1) as u32 - 1));
    rows = rows
        .into_iter()
        .skip(r0 as usize)
        .take((r1 - r0) as usize + 1)
        .map(|row| {
            (c0..=c1)
                .map(|c| row.get(c as usize).copied().unwrap_or("").trim())
                .collect()
        })
        .collect();
    let columns = (c1 - c0 + 1) as usize;
    let names: Vec<Option<String>> = if named && !rows.is_empty() {
        field_names(rows.remove(0))
    } else {
        (1..=columns).map(|n| Some(format!("VAR{n}"))).collect()
    };
    let mut data = Dataset::new(rows.len());
    let mut warnings = Vec::new();
    for (k, name) in names.iter().enumerate() {
        let Some(name) = name else { continue };
        let values = || rows.iter().map(move |row| row[k]);
        let first = rows.first().map_or("", |row| row[k]);
        if first.is_empty() || plain_number(first).is_some() {
            let numbers = values()
                .enumerate()
                .map(|(case, v)| match (v.is_empty(), plain_number(v)) {
                    (true, _) => None,
                    (false, Some(n)) => Some(n),
                    (false, None) => {
                        warnings.push(format!(
                            "case {}: {name} is system-missing: '{v}' is not a number",
                            case + 1
                        ));
                        None
                    }
                })
                .collect();
            data.set_numeric_values(name, numbers)?;
            let decimals = first.split_once('.').map_or(0, |(_, d)| {
                d.chars().take_while(char::is_ascii_digit).count()
            });
            if first.len() > 8 && !first.contains(['e', 'E']) {
                let format = format!("F{}.{}", first.len().min(40), decimals.min(16));
                if data.set_format(name, &format).is_err() {
                    data.set_format(name, "F8.2")?;
                }
            }
        } else {
            let width = first.len().max(8);
            let strings = values()
                .enumerate()
                .map(|(case, v)| {
                    let cut = cut_to(v, width);
                    if cut.len() < v.len() {
                        warnings.push(format!("case {}: {name} is cut to {width} bytes", case + 1));
                    }
                    cut.to_string()
                })
                .collect();
            data.set_string_values(name, width, strings)?;
        }
    }
    Ok((data, warnings))
}

/// The encoding a dBASE language driver byte stands for, when it is one Statscraft can read.
fn dbf_encoding(driver: u8) -> Option<&'static str> {
    Some(match driver {
        0x03 | 0x57 | 0x58 | 0x59 => "windows-1252",
        0x65 => "cp866",
        0x7D => "windows-1255",
        0x7E => "windows-1256",
        0xC8 => "windows-1250",
        0xC9 => "windows-1251",
        0xCA => "windows-1254",
        0xCB => "windows-1253",
        0xCC => "windows-1257",
        _ => return None,
    })
}

struct Field {
    name: String,
    kind: u8,
    offset: usize,
    length: usize,
    decimals: usize,
}

fn dbf(bytes: &[u8]) -> Result<(Dataset, Vec<String>), String> {
    if bytes.len() < 32 {
        return Err("not a dBASE file: too short".into());
    }
    if bytes[0] == 0x02 {
        return Err("dBASE II files are not supported; dBASE III and IV files are".into());
    }
    let u16_at = |i: usize| usize::from(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
    let records = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let (header, record_length) = (u16_at(8), u16_at(10));
    let encoding = dbf_encoding(bytes[29]);
    let decode = |raw: &[u8]| decode_syntax(raw, encoding).unwrap_or_default();
    let mut fields = Vec::new();
    let mut offset = 1;
    let mut at = 32;
    while at + 32 <= header.min(bytes.len()) && bytes[at] != 0x0D {
        let raw = &bytes[at..at + 32];
        let end = raw[..11].iter().position(|&b| b == 0).unwrap_or(11);
        let field = Field {
            name: decode(&raw[..end]).replace(':', "_"),
            kind: raw[11],
            offset,
            length: usize::from(raw[16]),
            decimals: usize::from(raw[17]),
        };
        if !b"CNFDLM".contains(&field.kind) {
            return Err(format!(
                "field {} has type '{}', which dBASE III and IV files do not use",
                field.name,
                char::from(field.kind)
            ));
        }
        offset += field.length;
        fields.push(field);
        at += 32;
    }
    if offset > record_length || header > bytes.len() {
        return Err("not a dBASE file: the fields do not fit in a record".into());
    }
    let available = (bytes.len() - header) / record_length.max(1);
    let mut warnings = Vec::new();
    if available < records {
        warnings.push(format!(
            "the file holds {available} of its {records} records; the rest are missing"
        ));
    }
    let records: Vec<&[u8]> = (0..records.min(available))
        .map(|i| &bytes[header + i * record_length..header + (i + 1) * record_length])
        .collect();
    let names = field_names(fields.iter().map(|f| f.name.as_str()));
    let mut data = Dataset::new(records.len());
    for (field, name) in fields.iter().zip(&names) {
        let Some(name) = name else { continue };
        let raw = |record: &&[u8]| decode(&record[field.offset..field.offset + field.length]);
        match field.kind {
            b'M' => continue,
            b'C' | b'L' => {
                let values: Vec<String> = records
                    .iter()
                    .map(|r| raw(r).trim_end().to_string())
                    .collect();
                let width = values
                    .iter()
                    .map(String::len)
                    .max()
                    .unwrap_or(0)
                    .max(field.length)
                    .max(1);
                data.set_string_values(name, width, values)
                    .map_err(|e| e.to_string())?;
            }
            b'D' => {
                let values = records
                    .iter()
                    .enumerate()
                    .map(|(case, r)| {
                        let text = raw(r);
                        let text = text.trim();
                        let part = |a: usize, b: usize| text.get(a..b)?.parse::<i64>().ok();
                        match (text.len(), part(0, 4), part(4, 6), part(6, 8)) {
                            (0, ..) => None,
                            (8, Some(y), Some(m @ 1..=12), Some(d @ 1..=31)) => {
                                Some((days_from_civil(y, m, d) + DAYS_TO_1970) as f64 * 86_400.0)
                            }
                            _ => {
                                warnings.push(format!(
                                    "record {}: {name} is system-missing: '{text}' is not a date",
                                    case + 1
                                ));
                                None
                            }
                        }
                    })
                    .collect();
                data.set_numeric_values(name, values)
                    .map_err(|e| e.to_string())?;
                data.set_format(name, "DATE11").map_err(|e| e.to_string())?;
            }
            _ => {
                let values = records
                    .iter()
                    .enumerate()
                    .map(|(case, r)| {
                        let text = raw(r);
                        let text = text.trim();
                        match text.parse::<f64>() {
                            _ if text.is_empty() => None,
                            Ok(v) if v.is_finite() => Some(v),
                            _ => {
                                warnings.push(format!(
                                    "record {}: {name} is system-missing: '{text}' is not a number",
                                    case + 1
                                ));
                                None
                            }
                        }
                    })
                    .collect();
                data.set_numeric_values(name, values)
                    .map_err(|e| e.to_string())?;
                let width = field.length.clamp(1, 40);
                let decimals = field.decimals.min(16).min(width - 1);
                if data
                    .set_format(name, &format!("F{width}.{decimals}"))
                    .is_err()
                {
                    data.set_format(name, "F8.2").map_err(|e| e.to_string())?;
                }
            }
        }
    }
    if records.iter().any(|r| r[0] == b'*') {
        let marks = records
            .iter()
            .map(|r| if r[0] == b'*' { "*" } else { "" }.to_string())
            .collect();
        data.set_string_values("D_R", 1, marks)
            .map_err(|e| e.to_string())?;
    }
    Ok((data, warnings))
}

impl Engine {
    pub(crate) fn get_translate(&mut self, command: &GetTranslate) -> Result<Vec<Table>, Error> {
        let fail = |e: String| Error(format!("GET TRANSLATE: {}: {e}", command.file));
        let (data, warnings) = match command.source {
            TranslateSource::Xls => (self.translate_workbook(command)?, Vec::new()),
            source => {
                let bytes =
                    std::fs::read(self.path(&command.file)).map_err(|e| fail(e.to_string()))?;
                if source == TranslateSource::Dbf {
                    dbf(&bytes).map_err(fail)?
                } else {
                    let text = decode_syntax(&bytes, None).map_err(fail)?;
                    tab(&text, command.field_names, command.range)?
                }
            }
        };
        self.warn_some(warnings);
        let names: Vec<String> = data.variable_names().map(str::to_string).collect();
        let tables = self.map_tables(&data, &names, &command.options, true, false)?;
        let data = self.file_options(data, &command.options, true)?;
        self.make_active(data);
        Ok(tables)
    }

    /// The first sheet as GET DATA reads it; without FIELDNAMES the column letters name the
    /// variables.
    fn translate_workbook(&mut self, command: &GetTranslate) -> Result<Dataset, Error> {
        let fail = |e: String| Error(format!("GET TRANSLATE: {}: {e}", command.file));
        let range = match command.range {
            Some(range) => range,
            None => {
                use calamine::Reader;
                let bytes =
                    std::fs::read(self.path(&command.file)).map_err(|e| fail(e.to_string()))?;
                let mut book = calamine::open_workbook_auto_from_rs(Cursor::new(bytes.as_slice()))
                    .map_err(|e| fail(format!("not a workbook that can be read ({e})")))?;
                let first = book.sheet_names().first().cloned().unwrap_or_default();
                let used = book
                    .worksheet_range(&first)
                    .map_err(|e| fail(e.to_string()))?;
                match (used.start(), used.end()) {
                    (Some((r0, _)), Some((r1, c1))) => (r0, 0, r1, c1),
                    _ => return Err(fail("the first sheet is empty".into())),
                }
            }
        };
        let options = Spreadsheet {
            binary: true,
            sheet: Sheet::Index(1),
            range: Some(range),
            read_names: command.field_names,
            assumed_width: None,
            ignore_hidden: false,
            ignore_trailing_spaces: false,
            type_min: None,
        };
        let get = GetData {
            file: command.file.clone(),
            encoding: None,
            fixed: false,
            first_case: 1,
            values_per_case: None,
            lines_per_case: 1,
            delimiters: None,
            qualifier: None,
            first_cases: None,
            ignore_multiple_spaces: false,
            ignore_leading_spaces: false,
            variables: Vec::new(),
            spreadsheet: Some(options.clone()),
        };
        let mut data = self
            .read_workbook(&get, &options)
            .map_err(|Error(e)| Error(e.replacen("GET DATA", "GET TRANSLATE", 1)))?;
        if !command.field_names {
            let names: Vec<String> = data.variable_names().map(str::to_string).collect();
            for name in names {
                let Some(k) = name.strip_prefix('V').and_then(|n| n.parse::<u32>().ok()) else {
                    continue;
                };
                let mut letters = column_letters(range.1 + k - 1);
                if !is_valid_name(&letters) {
                    letters.push('$');
                }
                data.rename_variable(&name, &letters)?;
            }
        }
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_manual() {
        assert_eq!(
            field_names(["Name", "GE", "name", "", "2nd try", "a:b", "by"]),
            [
                Some("Name".to_string()),
                Some("GE$".to_string()),
                Some("name2".to_string()),
                None,
                Some("V2nd_try".to_string()),
                Some("a_b".to_string()),
                Some("by$".to_string()),
            ]
        );
        assert_eq!(column_letters(0), "A");
        assert_eq!(column_letters(25), "Z");
        assert_eq!(column_letters(26), "AA");
        assert_eq!(column_letters(701), "ZZ");
        assert_eq!(column_letters(702), "AAA");
    }
}
