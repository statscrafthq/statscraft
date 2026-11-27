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
    delimited_each(
        text,
        delimiters,
        qualifier,
        ignore_multiple_spaces,
        &mut |line, values| {
            let owned = values.iter().map(|v| v.map(str::to_string));
            records.push((line, owned.collect()));
            true
        },
    );
    records
}

/// `delimited`, one record at a time: `each` gets the line the record starts on and its values
/// and says whether to go on. A large file never has all its records in memory at once, and no
/// value is a string of its own.
pub(crate) fn delimited_each(
    text: &str,
    delimiters: &[char],
    qualifier: Option<char>,
    ignore_multiple_spaces: bool,
    each: &mut dyn FnMut(usize, &[Option<&str>]) -> bool,
) {
    // the texts of the values of the record being read, one after the other; a value is the
    // part of it between two offsets, or None when it is empty
    let mut record = String::new();
    let mut spans: Vec<Option<(usize, usize)>> = Vec::new();
    // where the value being read starts in `record`
    let mut start = 0;
    let mut quoted = false;
    let mut was_quoted = false;
    let mut line = 1;
    let mut record_line = 1;
    let mut chars = text.chars().peekable();
    let finish_value = |record: &str,
                        spans: &mut Vec<Option<(usize, usize)>>,
                        start: &mut usize,
                        was_quoted: &mut bool| {
        let value = &record[*start..];
        let span = if *was_quoted {
            Some((*start, record.len()))
        } else {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then(|| {
                let lead = *start + value.len() - value.trim_start().len();
                (lead, lead + trimmed.len())
            })
        };
        spans.push(span);
        *start = record.len();
        *was_quoted = false;
    };
    let give = |record: &str,
                spans: &[Option<(usize, usize)>],
                line: usize,
                each: &mut dyn FnMut(usize, &[Option<&str>]) -> bool|
     -> bool {
        let cells: Vec<Option<&str>> = spans
            .iter()
            .map(|span| span.map(|(a, b)| &record[a..b]))
            .collect();
        each(line, &cells)
    };
    while let Some(c) = chars.next() {
        if quoted {
            if Some(c) == qualifier {
                if chars.peek() == Some(&c) {
                    record.push(c);
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                if c == '\n' {
                    line += 1;
                }
                if c != '\r' {
                    record.push(c);
                }
            }
            continue;
        }
        match c {
            '\r' => {}
            '\n' => {
                if !(spans.is_empty() && record[start..].trim().is_empty() && !was_quoted) {
                    finish_value(&record, &mut spans, &mut start, &mut was_quoted);
                    if !give(&record, &spans, record_line, each) {
                        return;
                    }
                    record.clear();
                    spans.clear();
                    start = 0;
                }
                line += 1;
                record_line = line;
            }
            c if Some(c) == qualifier && record[start..].trim().is_empty() => {
                record.truncate(start);
                quoted = true;
                was_quoted = true;
            }
            c if delimiters.contains(&c) => {
                // with MULTIPLESPACES IGNORE=YES a run of blanks is one delimiter, and blanks
                // at the start of a line are none
                let run = c == ' '
                    && ignore_multiple_spaces
                    && (chars.peek() == Some(&' ')
                        || (spans.is_empty() && record.len() == start && !was_quoted));
                if !run {
                    finish_value(&record, &mut spans, &mut start, &mut was_quoted);
                }
            }
            c => {
                if !was_quoted {
                    record.push(c);
                }
            }
        }
    }
    if !spans.is_empty() || !record[start..].trim().is_empty() || was_quoted {
        finish_value(&record, &mut spans, &mut start, &mut was_quoted);
        give(&record, &spans, record_line, each);
    }
}

/// One value in a numeric input format; a two-digit year falls in the window from `epoch`
/// (SET EPOCH, None: AUTOMATIC).
pub(crate) fn number(
    text: &str,
    format: Format,
    comma_decimal: bool,
    epoch: Option<f64>,
) -> Result<Option<f64>, &'static str> {
    number_in(text, format, comma_decimal, epoch, false)
}

/// As `number`; the decimals of the format are implied in a number without a decimal point when
/// the columns are fixed, as in DATA LIST (delimited values are read as they are written).
pub(crate) fn number_in(
    text: &str,
    format: Format,
    comma_decimal: bool,
    epoch: Option<f64>,
    fixed: bool,
) -> Result<Option<f64>, &'static str> {
    let decimals = if fixed { format.decimals } else { 0 };
    let text = text.trim();
    if text.is_empty() || text == "." {
        return Ok(None);
    }
    match format.kind {
        FormatType::Datetime | FormatType::Ymdhms => {
            crate::date_input::datetime(text, format.kind == FormatType::Ymdhms, epoch)
        }
        FormatType::Time => crate::date_input::clock(text, false).map(Some),
        // MTIME is minutes and seconds
        FormatType::Mtime => crate::date_input::clock(text, true).map(Some),
        FormatType::Date
        | FormatType::Adate
        | FormatType::Edate
        | FormatType::Sdate
        | FormatType::Jdate
        | FormatType::Qyr
        | FormatType::Moyr
        | FormatType::Wkyr => crate::data_list::date(text, format.kind, epoch),
        FormatType::F | FormatType::N | FormatType::E if comma_decimal => {
            let point = text.contains(',');
            crate::data_list::number(
                &text.replace('.', "").replace(',', "."),
                FormatType::F,
                if point { 0 } else { decimals },
            )
        }
        kind => crate::data_list::number(text, kind, decimals),
    }
}

/// The values of one variable as its cases are read.
enum Column {
    Skipped,
    /// The text of every value, for a variable whose type or width the whole file decides.
    Raw(Vec<Option<String>>),
    Text(TextColumn),
    Number(NumberColumn),
}

/// A string variable of a known width; a value is cut to it with a warning.
struct TextColumn {
    width: usize,
    values: Vec<String>,
    warnings: Vec<String>,
}

impl TextColumn {
    fn new(width: usize) -> Self {
        TextColumn {
            width,
            values: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn push(&mut self, name: &str, line: usize, value: &str) {
        let cut = statscraft_data::cut_to(value, self.width);
        if cut.len() < value.trim_end().len() {
            self.warnings.push(format!(
                "line {line}: {name} is cut to {} bytes",
                self.width
            ));
        }
        self.values.push(cut.to_string());
    }
}

/// A numeric variable read in one input format.
struct NumberColumn {
    format: Format,
    /// The type was chosen by AUTO: a value that is no number is expected, not a warning.
    auto: bool,
    /// The decimals of the format are implied (fixed columns, a stated format).
    implied: bool,
    values: Vec<Option<f64>>,
    odd: usize,
    warnings: Vec<String>,
}

impl NumberColumn {
    fn new(format: Format, auto: bool, fixed: bool) -> Self {
        NumberColumn {
            format,
            auto,
            implied: fixed && !auto,
            values: Vec::new(),
            odd: 0,
            warnings: Vec::new(),
        }
    }

    /// `settings`: the decimal comma and the SET values of the reading.
    fn push(
        &mut self,
        name: &str,
        line: usize,
        value: Option<&str>,
        (comma, reading): (bool, &crate::data_list::Reading),
    ) {
        let value = value.filter(|v| !v.trim().is_empty());
        let read =
            match value.map(|v| number_in(v, self.format, comma, reading.epoch, self.implied)) {
                // an empty value is a blank field
                None => reading.blanks,
                Some(Ok(None)) => None,
                Some(Ok(value)) => value,
                // AUTO chose the type by the share of numbers, so the others are expected
                Some(Err(_)) if self.auto => {
                    self.odd += 1;
                    None
                }
                Some(Err(why)) => {
                    self.warnings.push(format!(
                        "line {line}: {name} is system-missing: the value {why}"
                    ));
                    None
                }
            };
        self.values.push(read);
    }

    fn finish(
        self,
        variable: &statscraft_syntax::GetDataVariable,
        data: &mut Dataset,
        warnings: &mut Vec<String>,
    ) -> Result<(), Error> {
        let format = self.format;
        warnings.extend(self.warnings);
        if self.odd > 0 {
            warnings.push(format!(
                "{} values of {} are not numbers and are system-missing",
                self.odd, variable.name
            ));
        }
        data.set_numeric_values(&variable.name, self.values)?;
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
        Ok(())
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
        // columns of a code page or UTF-16 file count letters, of a UTF-8 file bytes
        let (text, by_chars) =
            statscraft_syntax::decode_text_owned(bytes, encoding).map_err(Error)?;
        let comma = self.setting("DECIMAL").eq_ignore_ascii_case("COMMA");
        let reading = self.reading();
        let wanted = command.first_cases.map_or(usize::MAX, |n| n as usize);
        // each value goes to its variable's column as its case is read, so a large file is
        // never held as text twice
        let mut columns: Vec<Column> = command
            .variables
            .iter()
            .map(|variable| match variable.format {
                _ if variable.skip => Column::Skipped,
                Some(format) if matches!(format.kind, FormatType::A | FormatType::Ahex) => {
                    match format.width {
                        0 => Column::Raw(Vec::new()),
                        w => Column::Text(TextColumn::new(usize::from(w))),
                    }
                }
                Some(format) => Column::Number(NumberColumn::new(format, false, command.fixed)),
                None => Column::Raw(Vec::new()),
            })
            .collect();
        let mut lines: Vec<usize> = Vec::new();
        let mut take = |line: usize, values: &[Option<&str>]| -> bool {
            if lines.len() >= wanted {
                return false;
            }
            for (index, column) in columns.iter_mut().enumerate() {
                let value = values.get(index).copied().flatten();
                let (name, number) = (&command.variables[index].name, (comma, &reading));
                match column {
                    Column::Skipped => {}
                    Column::Raw(raw) => raw.push(value.map(str::to_string)),
                    Column::Text(text) => text.push(name, line, value.unwrap_or_default()),
                    Column::Number(numbers) => numbers.push(name, line, value, number),
                }
            }
            lines.push(line);
            lines.len() < wanted
        };
        if command.fixed {
            let rows: Vec<&str> = text.lines().collect();
            let start = (command.first_case as usize - 1).min(rows.len());
            let per_case = command.lines_per_case as usize;
            for (k, chunk) in rows[start..].chunks(per_case).enumerate() {
                if chunk.len() < per_case {
                    break;
                }
                let values: Vec<Option<String>> = command
                    .variables
                    .iter()
                    .map(|v| {
                        let line = chunk.get(v.record as usize - 1).copied().unwrap_or("");
                        let (first, last) = v.columns.unwrap_or((0, 0));
                        let text = if by_chars {
                            let letters = line.chars().skip(first as usize);
                            letters
                                .take((last as usize + 1).saturating_sub(first as usize))
                                .collect()
                        } else {
                            let bytes = line.as_bytes();
                            let a = (first as usize).min(bytes.len());
                            let b = (last as usize + 1).min(bytes.len());
                            String::from_utf8_lossy(&bytes[a..b]).into_owned()
                        };
                        (!text.trim().is_empty()).then_some(text)
                    })
                    .collect();
                let values: Vec<Option<&str>> = values.iter().map(Option::as_deref).collect();
                if !take(start + k * per_case + 1, &values) {
                    break;
                }
            }
        } else {
            let default = [',', '\t'];
            let delimiters = command.delimiters.as_deref().unwrap_or(&default);
            let first_case = command.first_case as usize;
            let mut pending: Vec<Option<String>> = Vec::new();
            let mut first_line = 0;
            delimited_each(
                &text,
                delimiters,
                command.qualifier,
                command.ignore_multiple_spaces,
                &mut |line, values| {
                    if line < first_case {
                        return true;
                    }
                    let trimmed: Vec<Option<&str>>;
                    let values = if command.ignore_leading_spaces {
                        trimmed = values.iter().map(|v| v.map(str::trim_start)).collect();
                        &trimmed[..]
                    } else {
                        values
                    };
                    let Some(n) = command.values_per_case else {
                        return take(line, values);
                    };
                    for &value in values {
                        if pending.is_empty() {
                            first_line = line;
                        }
                        pending.push(value.map(str::to_string));
                        if pending.len() == n as usize {
                            let case = std::mem::take(&mut pending);
                            let case: Vec<Option<&str>> =
                                case.iter().map(Option::as_deref).collect();
                            if !take(first_line, &case) {
                                return false;
                            }
                        }
                    }
                    true
                },
            );
        }
        drop(text);
        let cases = lines.len();
        let mut data = Dataset::new(cases);
        let mut warnings = Vec::new();
        for (variable, column) in command.variables.iter().zip(columns) {
            match column {
                Column::Skipped => {}
                Column::Text(mut text) => {
                    warnings.append(&mut text.warnings);
                    data.set_string_values(&variable.name, text.width, text.values)?;
                }
                Column::Number(numbers) => {
                    numbers.finish(variable, &mut data, &mut warnings)?;
                }
                Column::Raw(raw) => {
                    let column = || {
                        lines
                            .iter()
                            .zip(&raw)
                            .map(|(line, v)| (*line, v.as_deref()))
                    };
                    let format = match variable.format {
                        Some(format) => format,
                        None => {
                            // AUTO: a number when DATATYPEMIN percent (95 unless given) of the
                            // non-blank values read as one; the others are then system-missing
                            let (mut filled, mut numbers) = (0_usize, 0_usize);
                            for (_, v) in column() {
                                // a blank and a lone period (system-missing) say nothing about the type
                                let Some(v) = v.filter(|v| !matches!(v.trim(), "" | ".")) else {
                                    continue;
                                };
                                filled += 1;
                                let format = Format {
                                    kind: FormatType::F,
                                    width: 8,
                                    decimals: 2,
                                };
                                if number(v, format, comma, reading.epoch).is_ok() {
                                    numbers += 1;
                                }
                            }
                            let needed = command.type_min.unwrap_or(95.0);
                            let numeric = numbers as f64 * 100.0 >= needed * filled as f64;
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
                        // a width left out: as wide as the longest value
                        let longest = raw
                            .iter()
                            .map(|v| v.as_deref().unwrap_or_default().trim_end().len())
                            .max()
                            .unwrap_or(0);
                        let mut text = TextColumn::new(longest.max(1));
                        for (line, v) in column() {
                            text.push(&variable.name, line, v.unwrap_or_default());
                        }
                        warnings.append(&mut text.warnings);
                        data.set_string_values(&variable.name, text.width, text.values)?;
                    } else {
                        // an AUTO number has no decimals of its own to imply
                        let auto = variable.format.is_none();
                        let mut numbers = NumberColumn::new(format, auto, command.fixed);
                        for (line, v) in column() {
                            numbers.push(&variable.name, line, v, (comma, &reading));
                        }
                        numbers.finish(variable, &mut data, &mut warnings)?;
                    }
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
    fn delimited_each_stops_when_told_and_keeps_the_values_of_delimited() {
        let text = " a , 'b ' ,c\n\n1,,'x'y\n3,4\n";
        let mut seen = Vec::new();
        delimited_each(text, &[','], Some('\''), false, &mut |line, values| {
            seen.push((
                line,
                values
                    .iter()
                    .map(|v| v.map(str::to_string))
                    .collect::<Vec<_>>(),
            ));
            seen.len() < 3
        });
        let owned = |values: &[Option<&str>]| -> Vec<Option<String>> {
            values.iter().map(|v| v.map(str::to_string)).collect()
        };
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[0], (1, owned(&[Some("a"), Some("b "), Some("c")])));
        assert_eq!(seen[1], (3, owned(&[Some("1"), None, Some("x")])));
        assert_eq!(seen[2], (4, owned(&[Some("3"), Some("4")])));
        // the same records as `delimited` gives, which reads on to the end
        assert_eq!(delimited(text, &[','], Some('\''), false).len(), 3);
    }

    #[test]
    fn numbers_without_an_exponent_read_as_written() {
        let f = Format {
            kind: FormatType::F,
            width: 8,
            decimals: 2,
        };
        for (text, want) in [
            ("5.", 5.0),
            (".5", 0.5),
            ("-1.50", -1.5),
            ("+12", 12.0),
            ("1e3", 1000.0),
            ("1.5E-2", 0.015),
            ("007", 7.0),
        ] {
            assert_eq!(number(text, f, false, None), Ok(Some(want)), "{text}");
        }
        assert!(number("1.2.3", f, false, None).is_err());
        assert!(number("-", f, false, None).is_err());
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
