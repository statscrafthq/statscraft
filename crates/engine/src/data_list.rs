//! DATA LIST and BEGIN DATA: reading text data into a new active dataset.
//!
//! Rules, from the command's description:
//! - FIXED reads each variable from its columns of its record (line); `records` lines make a
//!   case. Columns count bytes. A blank numeric field is system-missing. Without a decimal
//!   point or an exponent in the data, the last `d` digits of a field with `d` decimals are
//!   decimals. A string keeps every character of its columns, leading blanks too.
//! - FREE and LIST read values one after the other. Without delimiters blanks and commas
//!   separate values, extra blanks are ignored, two commas in a row leave a value empty, and a
//!   value may be quoted to hold blanks or commas. With delimiters each one ends a value, two
//!   in a row leave a value empty and quotes are ordinary characters; a string keeps the
//!   blanks before it. Decimals are never
//!   implied; the width of a numeric format only sets how the value is shown.
//! - FREE fills cases across lines; a last case without all its values is dropped. LIST reads
//!   one case per line: missing values at the end of a line are system-missing (or blank),
//!   values after the last variable are ignored.
//! - A value that cannot be read is system-missing, a string longer than its width is cut;
//!   both with a warning that names the line.

use crate::{Engine, Error, Table};
use statscraft_data::Dataset;
use statscraft_syntax::{DataField, DataLayout, DataList, FormatType};

/// Warnings beyond this many are counted, not listed.
pub(crate) const SHOWN_WARNINGS: usize = 20;

/// A value as read: a number (None: system-missing) or a string.
pub(crate) enum Read {
    Number(Option<f64>),
    Text(String),
}

/// One record of the data with how a warning names it, such as "line 12".
#[derive(Clone)]
pub(crate) struct Record {
    pub place: String,
    pub text: String,
}

/// What waits for BEGIN DATA: a DATA LIST, or an input program that reads inline data.
#[derive(Clone)]
pub(crate) enum Pending {
    List(DataList),
    Program(crate::input::Program),
    Matrix(Box<statscraft_syntax::MatrixData>),
}

impl Engine {
    pub(crate) fn data_list(&mut self, list: &DataList) -> Result<(), Error> {
        if list.end.is_some() {
            return Err(Error(
                "END on DATA LIST is for a DATA LIST inside INPUT PROGRAM".into(),
            ));
        }
        if list.fields.iter().all(|f| crate::rows::is_scratch(&f.name)) {
            return Err(Error(
                "DATA LIST defines no variables to keep; one without variables, or with scratch \
                 variables only, belongs inside INPUT PROGRAM"
                    .into(),
            ));
        }
        let Some(file) = &list.file else {
            self.pending_data = Some(Pending::List(list.clone()));
            return Ok(());
        };
        let records = self.file_records(file)?;
        self.read_data(list, records)
    }

    /// The lines of a text file as records.
    pub(crate) fn file_records(&self, file: &str) -> Result<Vec<Record>, Error> {
        let bytes = std::fs::read(self.path(file))
            .map_err(|e| Error(format!("cannot read {file}: {e}")))?;
        let text = match self.handle_encoding(file) {
            // FILE HANDLE /ENCODING
            Some(encoding) => {
                statscraft_syntax::decode_syntax(&bytes, Some(&encoding)).map_err(Error)?
            }
            None => String::from_utf8(bytes)
                .map_err(|_| Error(format!("{file} is not UTF-8 text; save it as UTF-8")))?,
        };
        let records = text.lines().enumerate().map(|(i, line)| Record {
            place: format!("record {} of {file}", i + 1),
            text: line.to_string(),
        });
        Ok(records.collect())
    }

    /// The data of the DATA LIST or input program before it, then the commands that waited.
    pub(crate) fn begin_data(&mut self, lines: &[(u32, String)]) -> Result<Vec<Table>, Error> {
        let Some(pending) = self.pending_data.take() else {
            return Err(Error(
                "BEGIN DATA needs a DATA LIST or MATRIX DATA without FILE, or an input program \
                 that reads inline data, before it"
                    .into(),
            ));
        };
        let records = lines.iter().map(|(line, text)| Record {
            place: format!("line {line}"),
            text: text.clone(),
        });
        let mut tables = match pending {
            Pending::List(list) => {
                self.read_data(&list, records.collect())?;
                Vec::new()
            }
            Pending::Program(program) => self.run_program(program, Some(records.collect()))?,
            Pending::Matrix(matrix) => {
                self.read_matrix(&matrix, records.collect())?;
                Vec::new()
            }
        };
        let mut warnings = std::mem::take(&mut self.warnings);
        for command in std::mem::take(&mut self.queued) {
            tables.extend(self.execute(&command)?);
            warnings.append(&mut self.warnings);
        }
        self.warnings = warnings;
        Ok(tables)
    }

    fn read_data(&mut self, list: &DataList, records: Vec<Record>) -> Result<(), Error> {
        let records = &records[(list.skip as usize).min(records.len())..];
        let mut warnings = Vec::new();
        let cases = match list.layout {
            DataLayout::Fixed { records: per_case } => {
                fixed(list, records, per_case, &mut warnings)
            }
            DataLayout::Free | DataLayout::List => free(list, records, &mut warnings),
        };
        let mut data = Dataset::new(cases.len());
        for (index, field) in list.fields.iter().enumerate() {
            // a scratch variable only passes the value on inside an input program
            if crate::rows::is_scratch(&field.name) {
                continue;
            }
            let values = cases.iter().map(|case| &case[index]);
            if field.format.kind == FormatType::A {
                let text = |read: &Read| match read {
                    Read::Text(text) => text.clone(),
                    Read::Number(_) => String::new(),
                };
                let width = usize::from(field.format.width);
                data.set_string_values(&field.name, width, values.map(text).collect())?;
            } else {
                let number = |read: &Read| match read {
                    Read::Number(n) => *n,
                    Read::Text(_) => None,
                };
                data.set_numeric_values(&field.name, values.map(number).collect())?;
                let shown = field.display;
                let text = format!("{}{}.{}", shown.kind.name(), shown.width, shown.decimals);
                data.set_format(&field.name, &text)?;
            }
        }
        let hidden = warnings.len().saturating_sub(SHOWN_WARNINGS);
        warnings.truncate(SHOWN_WARNINGS);
        warnings.into_iter().for_each(|w| self.warn(w));
        if hidden > 0 {
            self.warn(format!("{hidden} more values could not be read"));
        }
        self.dataset = Some(data);
        self.default_measures();
        Ok(())
    }
}

/// The cases of fixed-format data.
pub(crate) fn fixed(
    list: &DataList,
    records: &[Record],
    per_case: u32,
    warnings: &mut Vec<String>,
) -> Vec<Vec<Read>> {
    let chunks = records.chunks(per_case as usize);
    let mut cases = Vec::new();
    for chunk in chunks {
        if chunk.len() < per_case as usize {
            warnings.push(format!(
                "{}: the last case has {} of its {per_case} records and is left out",
                chunk[0].place,
                chunk.len()
            ));
            break;
        }
        let case = list.fields.iter().map(|field| {
            let record = &chunk[field.record as usize - 1];
            let text = columns(&record.text, field);
            read(&text, field, true, &record.place, warnings)
        });
        cases.push(case.collect());
    }
    cases
}

/// The text of a field's columns; columns past the end of the line are blank.
pub(crate) fn columns(line: &str, field: &DataField) -> String {
    let bytes = line.as_bytes();
    let start = (field.column as usize - 1).min(bytes.len());
    let end = (start + usize::from(field.format.width)).min(bytes.len());
    String::from_utf8_lossy(&bytes[start..end]).into_owned()
}

/// The values of one line of freefield data; None is an empty value.
pub(crate) fn values(line: &str, delimiters: &[char]) -> Vec<Option<String>> {
    if !delimiters.is_empty() {
        let mut pieces: Vec<&str> = line.split(|c| delimiters.contains(&c)).collect();
        // a delimiter that ends the line ends the value before it, it starts no empty one
        if pieces.len() > 1 && pieces.last().is_some_and(|p| p.trim().is_empty()) {
            pieces.pop();
        }
        if line.trim().is_empty() {
            return Vec::new();
        }
        return pieces
            .into_iter()
            .map(|p| (!p.trim().is_empty()).then(|| p.to_string()))
            .collect();
    }
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    let skip_blanks = |i: &mut usize| {
        while chars.get(*i).is_some_and(|c| c.is_whitespace()) {
            *i += 1;
        }
    };
    loop {
        skip_blanks(&mut i);
        let Some(&c) = chars.get(i) else { break };
        if c == ',' {
            out.push(None);
            i += 1;
            continue;
        }
        let value: String = if c == '\'' || c == '"' {
            let close = (i + 1..chars.len()).find(|&k| chars[k] == c);
            let end = close.unwrap_or(chars.len());
            let value = chars[i + 1..end].iter().collect();
            i = (end + 1).min(chars.len());
            value
        } else {
            let start = i;
            while chars
                .get(i)
                .is_some_and(|c| !c.is_whitespace() && *c != ',')
            {
                i += 1;
            }
            chars[start..i].iter().collect()
        };
        out.push(Some(value));
        skip_blanks(&mut i);
        if chars.get(i) == Some(&',') {
            i += 1;
        }
    }
    out
}

/// The cases of FREE and LIST data.
pub(crate) fn free(
    list: &DataList,
    records: &[Record],
    warnings: &mut Vec<String>,
) -> Vec<Vec<Read>> {
    let fields = &list.fields;
    let mut cases = Vec::new();
    let empty = |field: &DataField| match field.format.kind {
        FormatType::A => Read::Text(String::new()),
        _ => Read::Number(None),
    };
    let mut case: Vec<Read> = Vec::new();
    let mut place = "";
    for record in records {
        let found = values(&record.text, &list.delimiters);
        if list.layout == DataLayout::List {
            if found.len() < fields.len() {
                warnings.push(format!(
                    "{}: {} of {} values; the others are missing",
                    record.place,
                    found.len(),
                    fields.len()
                ));
            } else if found.len() > fields.len() {
                warnings.push(format!(
                    "{}: {} values for {} variables; the rest is ignored",
                    record.place,
                    found.len(),
                    fields.len()
                ));
            }
            let mut found = found.into_iter();
            let case = fields.iter().map(|field| match found.next().flatten() {
                Some(text) => read(&text, field, false, &record.place, warnings),
                None => empty(field),
            });
            cases.push(case.collect());
            continue;
        }
        for value in found {
            let field = &fields[case.len()];
            case.push(match value {
                Some(text) => read(&text, field, false, &record.place, warnings),
                None => empty(field),
            });
            if case.len() == fields.len() {
                cases.push(std::mem::take(&mut case));
            }
        }
        place = &record.place;
    }
    if !case.is_empty() {
        warnings.push(format!(
            "{place}: the last case has {} of its {} values and is left out",
            case.len(),
            fields.len()
        ));
    }
    cases
}

/// One value by the field's input format. `fixed` data may imply decimals.
pub(crate) fn read(
    text: &str,
    field: &DataField,
    fixed: bool,
    place: &str,
    warnings: &mut Vec<String>,
) -> Read {
    let format = field.format;
    if format.kind == FormatType::A {
        let width = usize::from(format.width);
        let mut end = width.min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        if end < text.trim_end().len() {
            warnings.push(format!(
                "{place}: {} is cut to {width} bytes: {text}",
                field.name
            ));
        }
        return Read::Text(text[..end].to_string());
    }
    let decimals = if fixed { format.decimals } else { 0 };
    let result = match format.kind {
        FormatType::Date | FormatType::Adate | FormatType::Edate | FormatType::Sdate => {
            date(text, format.kind)
        }
        kind => number(text, kind, decimals),
    };
    match result {
        Ok(value) => Read::Number(value),
        Err(why) => {
            warnings.push(format!(
                "{place}: {} is system-missing: '{}' {why}",
                field.name,
                text.trim()
            ));
            Read::Number(None)
        }
    }
}

/// A number in one of the numeric input formats; None for a blank field.
pub(crate) fn number(
    text: &str,
    kind: FormatType,
    decimals: u8,
) -> Result<Option<f64>, &'static str> {
    let text = text.trim();
    // a lone period is the system-missing value, as values are shown
    if text.is_empty() || text == "." {
        return Ok(None);
    }
    let cleaned: String = match kind {
        FormatType::Comma => text.replace(',', ""),
        FormatType::Dollar => text.replace([',', '$'], ""),
        FormatType::Dot => text.replace('.', "").replace(',', "."),
        FormatType::Pct => text.strip_suffix('%').unwrap_or(text).to_string(),
        FormatType::N if !text.bytes().all(|b| b.is_ascii_digit()) => {
            return Err("is not a number: N format takes digits only");
        }
        _ => text.to_string(),
    };
    let invalid = "is not a number";
    let (negative, rest) = match cleaned.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, cleaned.strip_prefix('+').unwrap_or(&cleaned)),
    };
    let rest = rest.trim_start();
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(rest.len());
    let (mantissa, tail) = rest.split_at(end);
    let digits = mantissa.bytes().filter(u8::is_ascii_digit).count();
    if digits == 0 || mantissa.matches('.').count() > 1 {
        return Err(invalid);
    }
    let exponent = if tail.is_empty() {
        None
    } else {
        // 1.5E2, 1.5E+2, 1.5D2, 1.5+2 and 1.5E 2 all have an exponent
        let tail = tail
            .strip_prefix(['e', 'E', 'd', 'D'])
            .map_or(tail, str::trim_start);
        let unsigned = tail.strip_prefix(['+', '-']).unwrap_or(tail);
        if unsigned.is_empty() || !unsigned.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid);
        }
        Some(tail.parse::<i32>().map_err(|_| invalid)?)
    };
    let shift = match exponent {
        Some(exponent) => exponent,
        None if !mantissa.contains('.') => -i32::from(decimals),
        None => 0,
    };
    let value: f64 = format!("{mantissa}e{shift}").parse().map_err(|_| invalid)?;
    if !value.is_finite() {
        return Err(invalid);
    }
    Ok(Some(if negative { -value } else { value }))
}

/// The month of a date field: a number, a Roman numeral or at least three letters of its
/// English name.
fn month(text: &str) -> Option<f64> {
    const NAMES: [&str; 12] = [
        "JANUARY",
        "FEBRUARY",
        "MARCH",
        "APRIL",
        "MAY",
        "JUNE",
        "JULY",
        "AUGUST",
        "SEPTEMBER",
        "OCTOBER",
        "NOVEMBER",
        "DECEMBER",
    ];
    const ROMAN: [&str; 12] = [
        "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII",
    ];
    let upper = text.to_uppercase();
    if let Ok(n) = upper.parse::<u8>() {
        return Some(f64::from(n));
    }
    let position = ROMAN.iter().position(|r| *r == upper).or_else(|| {
        (upper.len() >= 3)
            .then(|| NAMES.iter().position(|name| name.starts_with(&upper)))
            .flatten()
    });
    position.map(|i| i as f64 + 1.0)
}

/// A date in DATE (day month year), ADATE (month day year), EDATE (day month year) or SDATE
/// (year month day) form. Blanks, dashes, slashes, periods and commas separate the parts.
pub(crate) fn date(text: &str, kind: FormatType) -> Result<Option<f64>, &'static str> {
    let parts: Vec<&str> = text
        .split([' ', '-', '/', '.', ','])
        .filter(|p| !p.is_empty())
        .collect();
    if parts.is_empty() {
        return Ok(None);
    }
    let [a, b, c] = parts[..] else {
        return Err("is not a date: it needs a day, a month and a year");
    };
    let (day, month_text, year) = match kind {
        FormatType::Adate => (b, a, c),
        FormatType::Sdate => (c, b, a),
        _ => (a, b, c),
    };
    let whole = |text: &str| text.parse::<u32>().ok().map(f64::from);
    let (Some(day), Some(month), Some(year)) = (whole(day), month(month_text), whole(year)) else {
        return Err("is not a date");
    };
    let year = if year < 100.0 { century(year) } else { year };
    if !(1.0..=31.0).contains(&day) || !(1.0..=12.0).contains(&month) {
        return Err("is not a date: the day or the month is out of range");
    }
    crate::expr::date(Some(day), Some(month), Some(year))
        .map(Some)
        .ok_or("is not a date the calendar holds")
}

/// The year of a two-digit year: the one in the window from 69 years before the current
/// year to 30 years after it.
fn century(two_digits: f64) -> f64 {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (current, _, _) = crate::expr::civil_from_days((seconds / 86_400) as i64);
    let first = current as f64 - 69.0;
    let year = (first / 100.0).floor() * 100.0 + two_digits;
    if year < first { year + 100.0 } else { year }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_like_the_input_formats() {
        let f = |text, kind, d| number(text, kind, d).unwrap();
        assert_eq!(f("12345", FormatType::F, 2), Some(123.45));
        assert_eq!(f("  -201", FormatType::F, 2), Some(-2.01));
        assert_eq!(f("1.5", FormatType::F, 2), Some(1.5));
        assert_eq!(f("- 3", FormatType::F, 0), Some(-3.0));
        assert_eq!(f("1+2", FormatType::E, 1), Some(100.0));
        assert_eq!(f("12e1", FormatType::E, 1), Some(120.0));
        assert_eq!(f("1.234E 3", FormatType::F, 0), Some(1234.0));
        assert_eq!(f("1,234", FormatType::Dollar, 2), Some(12.34));
        assert_eq!(f("$-12", FormatType::Dollar, 2), Some(-0.12));
        assert_eq!(f("1.234,5", FormatType::Dot, 1), Some(1234.5));
        assert_eq!(f("5%", FormatType::Pct, 0), Some(5.0));
        assert_eq!(f("   ", FormatType::F, 0), None);
        assert_eq!(f(" . ", FormatType::F, 0), None);
        assert!(number("1 2", FormatType::F, 0).is_err());
        assert!(number("4a", FormatType::F, 0).is_err());
        assert!(number("-1", FormatType::N, 0).is_err());
    }

    #[test]
    fn dates_accept_names_numbers_and_roman_months() {
        let day = |text, kind| date(text, kind).unwrap().unwrap() / 86400.0;
        let first = day("1-JAN-2001", FormatType::Date);
        assert_eq!(day("01/01/2001", FormatType::Adate), first);
        assert_eq!(day("1.I.2001", FormatType::Edate), first);
        assert_eq!(day("2001/1/1", FormatType::Sdate), first);
        assert_eq!(day("3 2 2001", FormatType::Date) - first, 33.0);
        assert!(date("13/45/1990", FormatType::Adate).is_err());
        // two-digit years fall in the window from 69 years back to 30 years ahead
        assert_eq!(day("1-JAN-01", FormatType::Date), first);
        assert_eq!(century(99.0), 1999.0);
    }

    #[test]
    fn freefield_values() {
        let some = |v: &[Option<&str>]| -> Vec<Option<String>> {
            v.iter().map(|v| v.map(str::to_string)).collect()
        };
        assert_eq!(
            values(r#"3,,"x y" 4 , 5"#, &[]),
            some(&[Some("3"), None, Some("x y"), Some("4"), Some("5")])
        );
        assert_eq!(values("6,,", &[',']), some(&[Some("6"), None]));
        assert_eq!(
            values(" 4 , 5 ,\"q", &[',']),
            some(&[Some(" 4 "), Some(" 5 "), Some("\"q")])
        );
        assert_eq!(
            values("3\t\t4", &['\t']),
            some(&[Some("3"), None, Some("4")])
        );
    }
}
