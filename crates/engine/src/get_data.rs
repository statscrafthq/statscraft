//! GET DATA /TYPE=TXT: a delimited or fixed-column text file as the new active dataset. The
//! workbook types are in `workbook`.
//!
//! Choices where the manual is silent:
//! - Without DELIMITERS a tab or a comma separates values, and blanks around values go.
//! - A value between qualifiers may hold delimiters and line breaks; a doubled qualifier
//!   inside stands for one. Blanks around a qualified value are dropped.
//! - A width left out: a string is as wide as its longest value (at least 1), a number keeps
//!   the format's usual width (F8.2 for F).
//! - DATETIME and YMDHMS values are a date (year-month-day, day-month-year or month/day/year)
//!   and a time of day h:m[:s]; TIME values are h:m[:s].
//! - A value that cannot be read is system-missing, with a warning naming the line.

use crate::{Engine, Error};
use statscraft_data::Dataset;
use statscraft_syntax::{Format, FormatType, GetData};

const SHOWN_WARNINGS: usize = 20;

/// Splits delimited text into records of values, honouring the qualifier across lines (GET DATA
/// /TYPE=TXT, and the window when it looks at a file to open). Each
/// record comes with its 1-based line number.
pub fn delimited(
    text: &str,
    delimiters: &[char],
    qualifier: Option<char>,
    ignore_multiple_spaces: bool,
) -> Vec<(usize, Vec<Option<String>>)> {
    let mut records = Vec::new();
    let mut values: Vec<Option<String>> = Vec::new();
    let mut value = String::new();
    let mut quoted = false;
    let mut was_quoted = false;
    let mut line = 1;
    let mut record_line = 1;
    let mut chars = text.chars().peekable();
    let finish_value =
        |value: &mut String, was_quoted: &mut bool, values: &mut Vec<Option<String>>| {
            let v = std::mem::take(value);
            let keep = *was_quoted || !v.trim().is_empty();
            values.push(keep.then(|| if *was_quoted { v } else { v.trim().to_string() }));
            *was_quoted = false;
        };
    while let Some(c) = chars.next() {
        if quoted {
            if Some(c) == qualifier {
                if chars.peek() == Some(&c) {
                    value.push(c);
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                if c == '\n' {
                    line += 1;
                }
                if c != '\r' {
                    value.push(c);
                }
            }
            continue;
        }
        match c {
            '\r' => {}
            '\n' => {
                if !(values.is_empty() && value.trim().is_empty() && !was_quoted) {
                    finish_value(&mut value, &mut was_quoted, &mut values);
                    records.push((record_line, std::mem::take(&mut values)));
                }
                line += 1;
                record_line = line;
            }
            c if Some(c) == qualifier && value.trim().is_empty() => {
                value.clear();
                quoted = true;
                was_quoted = true;
            }
            c if delimiters.contains(&c) => {
                // with MULTIPLESPACES IGNORE=YES a run of blanks is one delimiter, and blanks
                // at the start of a line are none
                let run = c == ' '
                    && ignore_multiple_spaces
                    && (chars.peek() == Some(&' ')
                        || (values.is_empty() && value.is_empty() && !was_quoted));
                if !run {
                    finish_value(&mut value, &mut was_quoted, &mut values);
                }
            }
            c => {
                if !was_quoted {
                    value.push(c);
                }
            }
        }
    }
    if !values.is_empty() || !value.trim().is_empty() || was_quoted {
        finish_value(&mut value, &mut was_quoted, &mut values);
        records.push((record_line, values));
    }
    records
}

/// Seconds of a time of day h:m[:s].
fn time_of_day(text: &str) -> Option<f64> {
    let parts: Vec<&str> = text.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return None;
    }
    let number = |s: &str| s.trim().parse::<f64>().ok();
    let (h, m) = (number(parts[0])?, number(parts[1])?);
    let s = match parts.get(2) {
        Some(s) => number(s)?,
        None => 0.0,
    };
    Some(h * 3600.0 + m * 60.0 + s)
}

/// A date in any of the usual orders: year first, day first with a month name or number,
/// or month/day/year.
fn any_date(text: &str, epoch: Option<f64>) -> Result<Option<f64>, &'static str> {
    let year_first = text
        .trim()
        .split(['-', '/', '.'])
        .next()
        .is_some_and(|p| p.len() == 4 && p.bytes().all(|b| b.is_ascii_digit()));
    if year_first {
        return crate::data_list::date(text, FormatType::Sdate, epoch);
    }
    if text.contains('/') {
        return crate::data_list::date(text, FormatType::Adate, epoch);
    }
    crate::data_list::date(text, FormatType::Date, epoch)
}

/// One value in a numeric input format; a two-digit year falls in the window from `epoch`
/// (SET EPOCH, None: AUTOMATIC).
pub(crate) fn number(
    text: &str,
    format: Format,
    comma_decimal: bool,
    epoch: Option<f64>,
) -> Result<Option<f64>, &'static str> {
    let text = text.trim();
    if text.is_empty() || text == "." {
        return Ok(None);
    }
    match format.kind {
        FormatType::Datetime | FormatType::Ymdhms => {
            let (date, time) = match text.find([' ', 'T']) {
                Some(at) => (&text[..at], text[at + 1..].trim()),
                None => (text, ""),
            };
            let day = any_date(date, epoch)?.ok_or("is not a date")?;
            let time = if time.is_empty() {
                0.0
            } else {
                time_of_day(time).ok_or("is not a time of day")?
            };
            Ok(Some(day + time))
        }
        FormatType::Time | FormatType::Mtime => time_of_day(text).map(Some).ok_or("is not a time"),
        FormatType::Date | FormatType::Adate | FormatType::Edate | FormatType::Sdate => {
            crate::data_list::date(text, format.kind, epoch)
        }
        FormatType::F | FormatType::N | FormatType::E if comma_decimal => {
            crate::data_list::number(&text.replace('.', "").replace(',', "."), FormatType::F, 0)
        }
        kind => crate::data_list::number(text, kind, 0),
    }
}

impl Engine {
    pub(crate) fn get_data(&mut self, command: &GetData) -> Result<(), Error> {
        if let Some(options) = &command.spreadsheet {
            let data = self.read_workbook(command, options)?;
            self.make_active(data);
            return Ok(());
        }
        let path = self.path(&command.file);
        let bytes = std::fs::read(&path)
            .map_err(|e| Error(format!("GET DATA: cannot read {}: {e}", command.file)))?;
        let handle = self.handle_encoding(&command.file);
        let encoding = command
            .encoding
            .as_deref()
            .or(handle.as_deref())
            .filter(|e| !e.eq_ignore_ascii_case("locale"));
        let text = statscraft_syntax::decode_syntax(&bytes, encoding).map_err(Error)?;
        let comma = self.setting("DECIMAL").eq_ignore_ascii_case("COMMA");
        let reading = self.reading();
        let wanted = |count: usize| command.first_cases.map_or(count, |n| count.min(n as usize));
        // the values of every case, each with the line it starts on
        let mut cases: Vec<(usize, Vec<Option<String>>)> = Vec::new();
        if command.fixed {
            let lines: Vec<&str> = text.lines().collect();
            let start = (command.first_case as usize - 1).min(lines.len());
            let per_case = command.lines_per_case as usize;
            for (k, chunk) in lines[start..].chunks(per_case).enumerate() {
                if chunk.len() < per_case {
                    break;
                }
                let values = command.variables.iter().map(|v| {
                    let line = chunk.get(v.record as usize - 1).copied().unwrap_or("");
                    let (first, last) = v.columns.unwrap_or((0, 0));
                    let bytes = line.as_bytes();
                    let a = (first as usize).min(bytes.len());
                    let b = (last as usize + 1).min(bytes.len());
                    let text = String::from_utf8_lossy(&bytes[a..b]).into_owned();
                    (!text.trim().is_empty()).then_some(text)
                });
                cases.push((start + k * per_case + 1, values.collect()));
            }
        } else {
            let default = [',', '\t'];
            let delimiters = command.delimiters.as_deref().unwrap_or(&default);
            let records = delimited(
                &text,
                delimiters,
                command.qualifier,
                command.ignore_multiple_spaces,
            );
            let records = records
                .into_iter()
                .filter(|(line, _)| *line >= command.first_case as usize);
            match command.values_per_case {
                None => cases = records.collect(),
                Some(n) => {
                    let mut pending: Vec<Option<String>> = Vec::new();
                    let mut first_line = 0;
                    for (line, values) in records {
                        for value in values {
                            if pending.is_empty() {
                                first_line = line;
                            }
                            pending.push(value);
                            if pending.len() == n as usize {
                                cases.push((first_line, std::mem::take(&mut pending)));
                            }
                        }
                    }
                }
            }
            if command.ignore_leading_spaces {
                for (_, values) in &mut cases {
                    for v in values.iter_mut().flatten() {
                        *v = v.trim_start().to_string();
                    }
                }
            }
        }
        cases.truncate(wanted(cases.len()));
        let mut data = Dataset::new(cases.len());
        let mut warnings = Vec::new();
        for (index, variable) in command.variables.iter().enumerate() {
            if variable.skip {
                continue;
            }
            let column = || {
                cases
                    .iter()
                    .map(move |(line, values)| (*line, values.get(index).cloned().flatten()))
            };
            let format = match variable.format {
                Some(format) => format,
                None => {
                    // AUTO: a number when every value reads as one
                    let numeric = column().all(|(_, v)| {
                        v.is_none_or(|v| {
                            number(
                                &v,
                                Format {
                                    kind: FormatType::F,
                                    width: 8,
                                    decimals: 2,
                                },
                                comma,
                                reading.epoch,
                            )
                            .is_ok()
                        })
                    });
                    let kind = if numeric {
                        FormatType::F
                    } else {
                        FormatType::A
                    };
                    Format {
                        kind,
                        width: 0,
                        decimals: if numeric { 2 } else { 0 },
                    }
                }
            };
            if matches!(format.kind, FormatType::A | FormatType::Ahex) {
                let values: Vec<String> = column().map(|(_, v)| v.unwrap_or_default()).collect();
                let longest = values.iter().map(|v| v.trim_end().len()).max().unwrap_or(0);
                let width = match format.width {
                    0 => longest.max(1),
                    w => usize::from(w),
                };
                let values = values
                    .into_iter()
                    .zip(column())
                    .map(|(v, (line, _))| {
                        let cut = statscraft_data::cut_to(&v, width);
                        if cut.len() < v.trim_end().len() {
                            warnings.push(format!(
                                "line {line}: {} is cut to {width} bytes",
                                variable.name
                            ));
                        }
                        cut.to_string()
                    })
                    .collect();
                data.set_string_values(&variable.name, width, values)?;
            } else {
                let values = column()
                    .map(|(line, v)| (line, v.filter(|v| !v.trim().is_empty())))
                    .map(
                        |(line, v)| match v.map(|v| number(&v, format, comma, reading.epoch)) {
                            // an empty value is a blank field
                            None => reading.blanks,
                            Some(Ok(None)) => None,
                            Some(Ok(value)) => value,
                            Some(Err(why)) => {
                                warnings.push(format!(
                                    "line {line}: {} is system-missing: the value {why}",
                                    variable.name
                                ));
                                None
                            }
                        },
                    )
                    .collect();
                data.set_numeric_values(&variable.name, values)?;
                let shown = match (format.width, format.kind) {
                    (0, FormatType::F) => "F8.2".to_string(),
                    (0, kind) => kind.name().to_string(),
                    (w, kind) if format.decimals > 0 => {
                        format!("{}{w}.{}", kind.name(), format.decimals)
                    }
                    (w, kind) => format!("{}{w}", kind.name()),
                };
                // a format without a width takes the type's own
                if data.set_format(&variable.name, &shown).is_err() {
                    data.set_format(&variable.name, "F8.2")?;
                }
            }
        }
        self.warn_some(warnings);
        self.make_active(data);
        Ok(())
    }

    /// The first warnings about values, and how many more there are.
    pub(crate) fn warn_some(&mut self, mut warnings: Vec<String>) {
        let hidden = warnings.len().saturating_sub(SHOWN_WARNINGS);
        warnings.truncate(SHOWN_WARNINGS);
        warnings.into_iter().for_each(|w| self.warn(w));
        if hidden > 0 {
            self.warn(format!("{hidden} more values could not be read"));
        }
    }

    pub(crate) fn make_active(&mut self, data: Dataset) {
        self.park_active();
        self.dataset = Some(data);
        self.scratch_assigned.clear();
        self.temporary = None;
        self.case_limit = None;
        self.default_measures();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delimited_values_with_qualifiers_over_lines() {
        let text = "id,name\n1,'a, b'\n2,'it''s\nlong'\n3,,x\n";
        let records = delimited(text, &[','], Some('\''), false);
        let values: Vec<Vec<Option<&str>>> = records
            .iter()
            .map(|(_, v)| v.iter().map(|x| x.as_deref()).collect())
            .collect();
        assert_eq!(values[1], [Some("1"), Some("a, b")]);
        assert_eq!(values[2], [Some("2"), Some("it's\nlong")]);
        assert_eq!(values[3], [Some("3"), None, Some("x")]);
        assert_eq!(records[3].0, 5);
        let blanks = delimited("1   2\n", &[' '], None, true);
        assert_eq!(blanks[0].1, [Some("1".to_string()), Some("2".to_string())]);
        let blanks = delimited("1  2\n", &[' '], None, false);
        assert_eq!(
            blanks[0].1,
            [Some("1".to_string()), None, Some("2".to_string())]
        );
    }

    #[test]
    fn datetimes_read_in_the_usual_orders() {
        let format = Format {
            kind: FormatType::Datetime,
            width: 23,
            decimals: 2,
        };
        let a = number("2024-03-05 10:30:15", format, false, None)
            .unwrap()
            .unwrap();
        let b = number("05-MAR-2024 10:30:15", format, false, None)
            .unwrap()
            .unwrap();
        let c = number("03/05/2024 10:30:15", format, false, None)
            .unwrap()
            .unwrap();
        assert_eq!(a, b);
        assert_eq!(a, c);
        let midnight = number("2024-03-05", format, false, None).unwrap().unwrap();
        assert_eq!(a - midnight, 10.0 * 3600.0 + 30.0 * 60.0 + 15.0);
        assert!(number("yesterday", format, false, None).is_err());
        let f = Format {
            kind: FormatType::F,
            width: 8,
            decimals: 2,
        };
        assert_eq!(number("1.234,5", f, true, None), Ok(Some(1234.5)));
    }
}
