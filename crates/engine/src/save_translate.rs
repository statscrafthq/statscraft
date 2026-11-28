//! SAVE TRANSLATE: the active dataset as CSV, tab-delimited text, an Excel 2007 workbook, a
//! Stata data file (see `stata.rs`), a SAS transport file (see `xport.rs`) or a dBASE file
//! (see `dbf.rs`).
//!
//! Choices where the manual is silent:
//! - Lines end with CR LF, as RFC 4180 has it for CSV.
//! - A CSV value is qualified when it holds the delimiter, the qualifier or a line break.
//! - The manual gives two rules for the default delimiter; the one followed is that a comma
//!   decimal mark (DECIMAL=COMMA, else SET DECIMAL=COMMA) makes it a semicolon, so a number
//!   never needs quotes.
//! - FORMAT=PLAIN writes a date with a time of day as mm/dd/yyyy hh:mm:ss, a time with its
//!   fraction of a second when it has one. WKDAY and MONTH are plain numbers.
//! - FORMAT=VARIABLE widens a number that does not fit its format rather than writing
//!   asterisks.
//! - A string user-missing value with MISSING=RECODE is empty in a workbook, a numeric one
//!   #NULL! like system-missing.
//! - The first sheet is Sheet1 without EXCELOPTIONS; further sheets add (2), (3) and so on.

use crate::dbf::{DbfLimits, dbf_names, write_dbf};
use crate::excel::excel_number;
use crate::expr::{DAYS_TO_1970, civil_from_days};
use crate::stata::{DtaOptions, stata_encoding, stata_names, write_dta};
use crate::workbook::clock;
use crate::xport::{member_name, write_xport, xport_names};
use crate::{Engine, Error, Table, format_value, is_date_time_or_currency};
use rust_xlsxwriter::{Format, Formula, Workbook};
use statscraft_data::{Dataset, Value, Values, Variable};
use statscraft_syntax::{SaveTranslate, TranslateType};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write;

/// The rows and columns of an Excel 2007 sheet.
const SHEET_ROWS: usize = 1_048_576;
const SHEET_COLUMNS: usize = 16_384;

/// One value as the file receives it.
enum Out {
    Blank,
    Number(f64),
    Text(String),
}

fn label(variable: &Variable, value: &Value) -> Option<String> {
    let labels = variable.value_labels();
    labels
        .iter()
        .find(|(v, _)| v == value)
        .map(|(_, l)| l.clone())
}

fn out(variable: &Variable, case: usize, command: &SaveTranslate) -> Out {
    match variable.values() {
        Values::Numeric(values) => match values.get(case) {
            None => Out::Blank,
            Some(x) if command.recode_missing && variable.is_user_missing(x) => Out::Blank,
            Some(x) => match command
                .labels
                .then(|| label(variable, &Value::Number(x)))
                .flatten()
            {
                Some(label) => Out::Text(label),
                None => Out::Number(x),
            },
        },
        Values::String(values) => {
            let text = values.get(case);
            if text.is_empty() || (command.recode_missing && variable.is_user_missing_str(text)) {
                return Out::Blank;
            }
            let labelled = command
                .labels
                .then(|| label(variable, &Value::String(text.into())));
            Out::Text(labelled.flatten().unwrap_or_else(|| text.to_string()))
        }
    }
}

/// The letters of a format: "DATE" of "DATE11".
pub(crate) fn kind(format: &str) -> String {
    let letters = format.find(|c: char| !c.is_ascii_alphabetic());
    format[..letters.unwrap_or(format.len())].to_ascii_uppercase()
}

/// How FORMAT=PLAIN writes the numbers of one variable.
#[derive(Clone, Copy)]
enum PlainKind {
    Date,
    DateTime,
    Time,
    Number,
}

impl PlainKind {
    fn of(kind: &str) -> Self {
        match kind {
            "DATE" | "ADATE" | "EDATE" | "JDATE" | "SDATE" | "QYR" | "MOYR" | "WKYR" => Self::Date,
            "DATETIME" | "YMDHMS" => Self::DateTime,
            "TIME" | "DTIME" | "MTIME" => Self::Time,
            _ => Self::Number,
        }
    }

    /// Appends a number as FORMAT=PLAIN writes it.
    fn write(self, text: &mut String, x: f64, comma: bool) {
        let date = |x: f64| {
            let (y, m, d) = civil_from_days((x / 86_400.0).floor() as i64 - DAYS_TO_1970);
            format!("{m:02}/{d:02}/{y:04}")
        };
        match self {
            Self::Date => text.push_str(&date(x)),
            Self::DateTime => {
                let _ = write!(text, "{} {}", date(x), clock(x.rem_euclid(86_400.0)));
            }
            Self::Time => text.push_str(&clock(x)),
            Self::Number => {
                let start = text.len();
                let _ = write!(text, "{x}");
                if comma {
                    // the only period is the decimal mark
                    if let Some(at) = text[start..].find('.') {
                        text.replace_range(start + at..start + at + 1, ",");
                    }
                }
            }
        }
    }
}

/// What the text writer works out for each variable once.
struct Form {
    plain: PlainKind,
    /// The display format carries its own decimal mark (dates, times, currencies).
    setting_mark: bool,
    /// The display format, for FORMAT=VARIABLE.
    format: Option<String>,
}

/// The size at which the text writer hands a piece to the encoder and the file.
const CHUNK: usize = 1 << 20;

/// A text encoding for the output, applied piece by piece so a large file never sits in memory
/// twice.
struct Encoder {
    encoding: &'static encoding_rs::Encoding,
    bom: bool,
}

impl Encoder {
    /// The encoding `name` (UTF-8 when None), with a byte order mark for a UTF encoding when
    /// `bom`.
    fn new(name: Option<&str>, bom: bool) -> Result<Self, String> {
        let encoding = match name {
            None => statscraft_syntax::encoding_for("UTF8"),
            Some(name) => statscraft_syntax::encoding_for(name),
        }
        .ok_or_else(|| format!("unknown encoding {}", name.unwrap_or_default()))?;
        Ok(Encoder { encoding, bom })
    }

    /// The bytes that open the file.
    fn start(&self) -> &'static [u8] {
        match (self.bom, self.encoding.name()) {
            (true, "UTF-8") => &[0xEF, 0xBB, 0xBF],
            (true, "UTF-16LE") => &[0xFF, 0xFE],
            (true, "UTF-16BE") => &[0xFE, 0xFF],
            _ => &[],
        }
    }

    /// Appends `text` to `bytes`. Every encoding offered here is stateless (encoding_rs writes
    /// the ISO-2022 family as UTF-8), so each piece encodes alone; keep one encoder across
    /// pieces if a stateful encoding is ever added.
    fn push(&self, text: &str, bytes: &mut Vec<u8>) -> Result<(), String> {
        match self.encoding.name() {
            "UTF-8" => bytes.extend_from_slice(text.as_bytes()),
            "UTF-16LE" => bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes)),
            "UTF-16BE" => bytes.extend(text.encode_utf16().flat_map(u16::to_be_bytes)),
            other => {
                let (encoded, _, lost) = self.encoding.encode(text);
                if lost {
                    return Err(format!(
                        "the data has characters that {other} cannot hold; use /ENCODING='UTF8'"
                    ));
                }
                bytes.extend_from_slice(&encoded);
            }
        }
        Ok(())
    }
}

impl Engine {
    pub(crate) fn save_translate(&mut self, command: &SaveTranslate) -> Result<Vec<Table>, Error> {
        let fail = |e: String| Error(format!("SAVE TRANSLATE: {}: {e}", command.outfile));
        let mut data = self.active()?.clone();
        if command.delete_unselected
            && let Some(filter) = data.filter_variable()
        {
            let variable = data.variable(filter)?;
            let keep: Vec<bool> = (0..data.case_count()).map(|c| variable.keeps(c)).collect();
            data.select_cases(&keep)?;
        }
        let names: Vec<String> = data.variable_names().map(str::to_string).collect();
        let stata = command.kind == TranslateType::Stata;
        let unicode = command.stata_version >= 14;
        let dbase = matches!(command.kind, TranslateType::Dbase3 | TranslateType::Dbase4);
        let sas = command.kind == TranslateType::SasTransport;
        let file_names = |names: Vec<&str>| match (stata, dbase, sas) {
            (true, ..) => stata_names(names, unicode),
            (_, true, _) => dbf_names(names),
            (.., true) => xport_names(names),
            _ => names.into_iter().map(str::to_string).collect(),
        };
        let tables = self.map_tables(&data, &names, &command.options, true, Some(&file_names))?;
        let data = self.file_options(data, &command.options, true)?;
        let path = self.path(&command.outfile);
        if path.exists() && !command.replace {
            return Err(Error(format!(
                "SAVE TRANSLATE: {} exists; add /REPLACE to overwrite it",
                command.outfile
            )));
        }
        let encoding = match (stata, command.encoding.as_deref()) {
            (true, Some(name)) => {
                Some(stata_encoding(name).ok_or_else(|| fail(format!("unknown encoding {name}")))?)
            }
            _ => None,
        };
        if self.dry_run {
            return Ok(tables);
        }
        let written = match command.kind {
            TranslateType::Xlsx => self.write_workbook(&data, command, &path),
            TranslateType::Stata => {
                let options = DtaOptions {
                    version: command.stata_version,
                    se: command.stata_se,
                    encoding,
                    recode_missing: command.recode_missing,
                    big: false,
                };
                write_dta(&data, &options).and_then(|(bytes, warnings)| {
                    self.warn_some(warnings);
                    statscraft_data::write_bytes(&path, &bytes)
                })
            }
            TranslateType::SasTransport => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let member = member_name(&path.to_string_lossy());
                write_xport(&data, &member, now).and_then(|(bytes, warnings)| {
                    self.warn_some(warnings);
                    statscraft_data::write_bytes(&path, &bytes)
                })
            }
            TranslateType::Dbase3 | TranslateType::Dbase4 => {
                let limits = DbfLimits::new(command.kind == TranslateType::Dbase4);
                let seconds = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let today = civil_from_days((seconds / 86_400) as i64);
                write_dbf(&data, &limits, command.recode_missing, today).and_then(
                    |(bytes, warnings)| {
                        self.warn_some(warnings);
                        statscraft_data::write_bytes(&path, &bytes)
                    },
                )
            }
            _ => self.write_text(&data, command, &path),
        };
        written.map_err(fail)?;
        Ok(tables)
    }

    fn write_text(
        &self,
        data: &Dataset,
        command: &SaveTranslate,
        path: &std::path::Path,
    ) -> Result<(), String> {
        let currencies = self.currencies();
        let comma = command
            .decimal_comma
            .unwrap_or_else(|| self.setting("DECIMAL").eq_ignore_ascii_case("COMMA"));
        let csv = command.kind == TranslateType::Csv;
        let delimiter = match csv {
            true => command.delimiter.unwrap_or(if comma { ';' } else { ',' }),
            false => '\t',
        };
        let q = command.qualifier;
        let variables: Vec<Variable> = data.variables().collect();
        let encoder = Encoder::new(command.encoding.as_deref(), command.bom)?;
        // the form of each variable's numbers, found once instead of for every cell
        let forms: Vec<Form> = variables
            .iter()
            .map(|v| {
                let kind = kind(v.format());
                let setting_mark = crate::format::takes_setting_mark(v.format());
                Form {
                    plain: PlainKind::of(&kind),
                    setting_mark,
                    format: command
                        .print_formats
                        .then(|| currencies.resolve(v.format())),
                }
            })
            .collect();
        statscraft_data::write_file(path, |file| {
            let mut file = std::io::BufWriter::new(file);
            let mut bytes = encoder.start().to_vec();
            let mut text = String::new();
            let mut flush = |text: &mut String, bytes: &mut Vec<u8>, last: bool| {
                if text.len() >= CHUNK || (last && !text.is_empty()) {
                    encoder.push(text, bytes)?;
                    text.clear();
                }
                if bytes.len() >= CHUNK || (last && !bytes.is_empty()) {
                    file.write_all(bytes).map_err(|e| e.to_string())?;
                    bytes.clear();
                }
                Ok::<(), String>(())
            };
            // one value after the text so far; a CSV value that holds the delimiter, the
            // qualifier or a line break goes between qualifiers, the qualifier doubled
            let field = |text: &mut String, start: usize| {
                if csv && text[start..].contains([delimiter, q, '\n', '\r']) {
                    let value = text.split_off(start);
                    text.push(q);
                    text.push_str(&value.replace(q, &format!("{q}{q}")));
                    text.push(q);
                }
            };
            if let Some(labels) = command.field_names {
                for (k, v) in variables.iter().enumerate() {
                    if k > 0 {
                        text.push(delimiter);
                    }
                    let start = text.len();
                    match v.label() {
                        Some(label) if labels => text.push_str(label),
                        _ => text.push_str(v.name()),
                    }
                    field(&mut text, start);
                }
                text.push_str("\r\n");
            }
            for case in 0..data.case_count() {
                for (k, (v, form)) in variables.iter().zip(&forms).enumerate() {
                    if k > 0 {
                        text.push(delimiter);
                    }
                    let start = text.len();
                    match out(v, case, command) {
                        Out::Blank => {}
                        Out::Text(s) => text.push_str(&s),
                        Out::Number(x) => match &form.format {
                            Some(format) => {
                                let shown = format_value(x, format, true);
                                let shown = shown.trim();
                                match comma && form.setting_mark {
                                    true => text.push_str(&shown.replace('.', ",")),
                                    false => text.push_str(shown),
                                }
                            }
                            None => form.plain.write(&mut text, x, comma),
                        },
                    }
                    field(&mut text, start);
                }
                text.push_str("\r\n");
                flush(&mut text, &mut bytes, false)?;
            }
            flush(&mut text, &mut bytes, true)?;
            file.flush().map_err(|e| e.to_string())
        })
    }

    fn write_workbook(
        &mut self,
        data: &Dataset,
        command: &SaveTranslate,
        path: &std::path::Path,
    ) -> Result<(), String> {
        let currencies = self.currencies();
        let mut variables: Vec<Variable> = data.variables().collect();
        if variables.len() > SHEET_COLUMNS {
            self.warn(format!(
                "a sheet holds {SHEET_COLUMNS} columns: the last {} variables are left out",
                variables.len() - SHEET_COLUMNS
            ));
            variables.truncate(SHEET_COLUMNS);
        }
        let header = usize::from(command.field_names.is_some());
        let per_sheet = SHEET_ROWS - header;
        let cases = data.case_count();
        let sheets = cases.div_ceil(per_sheet).max(1);
        let base = command.sheet.as_deref().unwrap_or("Sheet1");
        let mut book = Workbook::new();
        // one Format per Excel number format
        let mut formats: HashMap<String, Format> = HashMap::new();
        // a number format that is no date, time or currency gives every cell of its variable the
        // same Excel format (except no format at all, which depends on the value): found once
        let plain: Vec<Option<Format>> = variables
            .iter()
            .map(|v| {
                let format = currencies.resolve(v.format());
                (!format.is_empty() && !is_date_time_or_currency(&format)).then(|| {
                    let (_, code) = excel_number(&format, 0.0, "").expect("a plain format");
                    Format::new().set_num_format(code)
                })
            })
            .collect();
        let xe = |e: rust_xlsxwriter::XlsxError| e.to_string();
        for k in 0..sheets {
            let sheet = book.add_worksheet();
            let name = match k {
                0 => base.to_string(),
                k => {
                    let suffix = format!(" ({})", k + 1);
                    let room = 31 - suffix.len();
                    format!("{}{suffix}", base.chars().take(room).collect::<String>())
                }
            };
            sheet.set_name(name).map_err(xe)?;
            if let Some(labels) = command.field_names {
                for (c, v) in variables.iter().enumerate() {
                    let name = match v.label() {
                        Some(label) if labels => label,
                        _ => v.name(),
                    };
                    sheet.write_string(0, c as u16, name).map_err(xe)?;
                }
            }
            let first = k * per_sheet;
            for case in first..cases.min(first + per_sheet) {
                let row = (case - first + header) as u32;
                for (c, v) in variables.iter().enumerate() {
                    let c = c as u16;
                    match out(v, case, command) {
                        Out::Blank if v.is_string() => {}
                        Out::Blank => {
                            let null = Formula::new("#NULL!").set_result("#NULL!");
                            sheet.write_formula(row, c, null).map_err(xe)?;
                        }
                        Out::Text(s) => {
                            sheet.write_string(row, c, s).map_err(xe)?;
                        }
                        Out::Number(x) if plain[c as usize].is_some() => {
                            let style = plain[c as usize].as_ref().expect("checked");
                            sheet
                                .write_number_with_format(row, c, x, style)
                                .map_err(xe)?;
                        }
                        Out::Number(x) => {
                            let format = currencies.resolve(v.format());
                            // the text only decides the form of a date
                            let shown = match is_date_time_or_currency(&format) {
                                true => format_value(x, &format, true),
                                false => String::new(),
                            };
                            match excel_number(&format, x, shown.trim()) {
                                Some((value, code)) => {
                                    let style = formats
                                        .entry(code.clone())
                                        .or_insert_with(|| Format::new().set_num_format(code));
                                    sheet
                                        .write_number_with_format(row, c, value, style)
                                        .map_err(xe)?;
                                }
                                None => {
                                    // a date the display format cannot show (before the
                                    // calendar starts, after year 9999) is its plain text, as
                                    // in a CSV file, and not asterisks; other formats keep
                                    // what they show
                                    let kind = PlainKind::of(&kind(&format));
                                    let text = if shown.contains('*')
                                        && !matches!(kind, PlainKind::Number)
                                    {
                                        let mut text = String::new();
                                        kind.write(&mut text, x, false);
                                        text
                                    } else {
                                        shown.trim().to_string()
                                    };
                                    sheet.write_string(row, c, text).map_err(xe)?;
                                }
                            }
                        }
                    }
                }
            }
            sheet.autofit();
        }
        statscraft_data::write_file(path, |file| {
            let mut file = std::io::BufWriter::new(file);
            book.save_to_writer(&mut file).map_err(xe)?;
            file.flush().map_err(|e| e.to_string())
        })
    }
}
