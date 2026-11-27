//! KEYED DATA LIST: variables read from the record of a text file that a key variable picks.
//!
//! The manual describes direct-access and keyed files in terms of file systems that text
//! files are not; the rules here are the ones that carry over to a text file, where every line
//! is a record:
//! - A numeric key is a record number: 1 reads the first line, and a key that is not a whole
//!   number from 1 to the number of lines finds no record. This is the direct-access file.
//! - A string key picks the first record that begins with it, without its trailing blanks;
//!   the file is keyed. The key has to be written as the file has it (the manual says to
//!   convert a number with STRING so that it does), and an empty key finds no record.
//! - IN is 1 where a record was read and 0 where there was none, for the cases the command
//!   runs for (those of the open DO IF branch inside an input program). A case without a
//!   record keeps the values the variables had, or has them missing.
//! - The variables are read as DATA LIST reads one line in fixed columns, so formats,
//!   implied decimals, blanks and cut strings follow its rules, with the same warnings.
//! - In an input program a run of the program reads one record, found or not, so a program
//!   that only uses this command is not mistaken for one that never reads.

use crate::data_list::{self, Reading, Record, SHOWN_WARNINGS};
use crate::expr::Column;
use crate::{Engine, Error};
use statscraft_data::Values;
use statscraft_syntax::{FormatType, KeyedDataList};

/// The record a key picks: by number for a number, by its start for a string.
enum Key<'a> {
    Number(Option<f64>),
    Text(&'a str),
}

fn record<'a>(key: &Key, records: &'a [Record]) -> Option<&'a Record> {
    match key {
        Key::Number(Some(n)) if n.fract() == 0.0 && *n >= 1.0 && *n <= records.len() as f64 => {
            Some(&records[*n as usize - 1])
        }
        Key::Number(_) => None,
        Key::Text(text) => {
            let key = text.trim_end();
            (!key.is_empty())
                .then(|| records.iter().find(|r| r.text.starts_with(key)))
                .flatten()
        }
    }
}

impl Engine {
    /// The lines of the file as records, in its ENCODING when it has one.
    pub(crate) fn keyed_records(
        &self,
        file: &str,
        encoding: Option<&str>,
    ) -> Result<Vec<Record>, Error> {
        let Some(name) = encoding else {
            return self.file_records(file, None);
        };
        let bytes = std::fs::read(self.path(file))
            .map_err(|e| Error(format!("cannot read {file}: {e}")))?;
        let (text, by_chars) = statscraft_syntax::decode_text(&bytes, Some(name)).map_err(Error)?;
        Ok(text
            .lines()
            .enumerate()
            .map(|(i, line)| Record {
                by_chars,
                place: format!("record {} of {file}", i + 1),
                text: line.to_string(),
            })
            .collect())
    }

    pub(crate) fn keyed_data_list(&mut self, command: &KeyedDataList) -> Result<(), Error> {
        let cases = self.active()?.case_count();
        if cases == 0 {
            // the pass of an input program on no case, which gives the dictionary
            return self.declare_fields(&command.fields, Some(&command.found));
        }
        let records = match self.program_keyed_records(&command.file) {
            Some(records) => records,
            None => self.keyed_records(&command.file, command.encoding.as_deref())?,
        };
        // the cases the command runs for: all, or those of the open DO IF branch
        let active: Vec<bool> = match self.active_cases() {
            Some(chosen) => chosen.to_vec(),
            None => vec![true; cases],
        };
        let data = self.active()?;
        let key = data.variable(&command.key).map_err(|_| {
            Error(format!(
                "KEYED DATA LIST: the key variable {} does not exist; give it a value with \
                 COMPUTE, GET or DATA LIST first",
                command.key
            ))
        })?;
        let picked: Vec<Option<&Record>> = (0..cases)
            .map(|case| {
                if !active[case] {
                    return None;
                }
                match key.values() {
                    Values::Numeric(v) => record(&Key::Number(v.get(case)), &records),
                    Values::String(v) => record(&Key::Text(v.get(case)), &records),
                }
            })
            .collect();
        let found: Vec<bool> = picked.iter().map(Option::is_some).collect();
        let reading: Reading = self.reading();
        let mut warnings = Vec::new();
        let mut columns: Vec<Column> = Vec::new();
        for field in &command.fields {
            let string = field.format.kind == FormatType::A;
            let values: Vec<data_list::Read> = picked
                .iter()
                .map(|record| match record {
                    Some(record) => {
                        let text = data_list::columns(record, field);
                        data_list::read(&text, field, true, reading, &record.place, &mut warnings)
                    }
                    None => match string {
                        true => data_list::Read::Text(String::new()),
                        false => data_list::Read::Number(None),
                    },
                })
                .collect();
            columns.push(match string {
                true => Column::String(
                    values
                        .into_iter()
                        .map(|v| match v {
                            data_list::Read::Text(t) => t,
                            data_list::Read::Number(_) => String::new(),
                        })
                        .collect(),
                ),
                false => Column::Number(
                    values
                        .into_iter()
                        .map(|v| match v {
                            data_list::Read::Number(n) => n,
                            data_list::Read::Text(_) => None,
                        })
                        .collect(),
                ),
            });
        }
        // the variables: made when new, a number and a string apart
        for (field, column) in command.fields.iter().zip(columns) {
            let string = field.format.kind == FormatType::A;
            let data = self.active_mut()?;
            let new = match data.variable(&field.name) {
                Ok(existing) if existing.is_string() != string => {
                    return Err(Error(format!(
                        "KEYED DATA LIST: {} is {} in the dataset and {} in the record",
                        field.name,
                        if string { "a number" } else { "a string" },
                        if string { "a string" } else { "a number" },
                    )));
                }
                Ok(_) => false,
                Err(_) => true,
            };
            if new && string {
                let width = usize::from(field.format.width);
                data.set_string_values(&field.name, width, vec![String::new(); cases])?;
            }
            self.put(&field.name, column, Some(found.clone()))?;
            if new && !string {
                let shown = field.display;
                let text = format!("{}{}.{}", shown.kind.name(), shown.width, shown.decimals);
                self.active_mut()?.set_format(&field.name, &text)?;
            }
        }
        let flag: Vec<Option<f64>> = (0..cases)
            .map(|case| active[case].then(|| f64::from(u8::from(found[case]))))
            .collect();
        let new = self.active()?.variable(&command.found).is_err();
        self.put(&command.found, Column::Number(flag), Some(active))?;
        if new {
            self.active_mut()?.set_format(&command.found, "F1.0")?;
        }
        let hidden = warnings.len().saturating_sub(SHOWN_WARNINGS);
        warnings.truncate(SHOWN_WARNINGS);
        warnings.into_iter().for_each(|w| self.warn(w));
        if hidden > 0 {
            self.warn(format!("{hidden} more values could not be read"));
        }
        self.count_read();
        Ok(())
    }
}
