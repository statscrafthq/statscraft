//! SAVE TRANSLATE: the active dataset as CSV, tab-delimited text or an Excel 2007 workbook.
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

use crate::excel::excel_number;
use crate::expr::{DAYS_TO_1970, civil_from_days};
use crate::workbook::clock;
use crate::{Engine, Error, format_value, is_date_time_or_currency};
use rust_xlsxwriter::{Format, Formula, Workbook};
use statscraft_data::{Dataset, Value, Values, Variable};
use statscraft_syntax::{SaveTranslate, TranslateType};
use std::collections::HashMap;

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
            Some(x) => match label(variable, &Value::Number(x)).filter(|_| command.labels) {
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
fn kind(format: &str) -> String {
    let letters = format.find(|c: char| !c.is_ascii_alphabetic());
    format[..letters.unwrap_or(format.len())].to_ascii_uppercase()
}

/// A number as FORMAT=PLAIN writes it.
fn plain(x: f64, format: &str, comma: bool) -> String {
    let date = |x: f64| {
        let (y, m, d) = civil_from_days((x / 86_400.0).floor() as i64 - DAYS_TO_1970);
        format!("{m:02}/{d:02}/{y:04}")
    };
    match kind(format).as_str() {
        "DATE" | "ADATE" | "EDATE" | "JDATE" | "SDATE" | "QYR" | "MOYR" | "WKYR" => date(x),
        "DATETIME" | "YMDHMS" => format!("{} {}", date(x), clock(x.rem_euclid(86_400.0))),
        "TIME" | "DTIME" | "MTIME" => clock(x),
        _ if comma => format!("{x}").replace('.', ","),
        _ => format!("{x}"),
    }
}

/// `text` in the encoding `name` (UTF-8 when None), with a byte order mark for a UTF encoding
/// when `bom`.
fn encode(text: &str, name: Option<&str>, bom: bool) -> Result<Vec<u8>, String> {
    let encoding = match name {
        None => statscraft_syntax::encoding_for("UTF8"),
        Some(name) => statscraft_syntax::encoding_for(name),
    }
    .ok_or_else(|| format!("unknown encoding {}", name.unwrap_or_default()))?;
    let mark = |bytes: &[u8]| if bom { bytes.to_vec() } else { Vec::new() };
    Ok(match encoding.name() {
        "UTF-8" => [mark(&[0xEF, 0xBB, 0xBF]), text.as_bytes().to_vec()].concat(),
        "UTF-16LE" => {
            let units = text.encode_utf16().flat_map(u16::to_le_bytes);
            mark(&[0xFF, 0xFE]).into_iter().chain(units).collect()
        }
        "UTF-16BE" => {
            let units = text.encode_utf16().flat_map(u16::to_be_bytes);
            mark(&[0xFE, 0xFF]).into_iter().chain(units).collect()
        }
        other => {
            let (bytes, _, lost) = encoding.encode(text);
            if lost {
                return Err(format!(
                    "the data has characters that {other} cannot hold; use /ENCODING='UTF8'"
                ));
            }
            bytes.into_owned()
        }
    })
}

impl Engine {
    pub(crate) fn save_translate(&mut self, command: &SaveTranslate) -> Result<(), Error> {
        let mut data = self.active()?.clone();
        if command.delete_unselected
            && let Some(filter) = data.filter_variable()
        {
            let variable = data.variable(filter)?;
            let keep: Vec<bool> = (0..data.case_count()).map(|c| variable.keeps(c)).collect();
            data.select_cases(&keep)?;
        }
        let data = self.file_options(data, &command.options, true)?;
        let path = self.path(&command.outfile);
        if path.exists() && !command.replace {
            return Err(Error(format!(
                "SAVE TRANSLATE: {} exists; add /REPLACE to overwrite it",
                command.outfile
            )));
        }
        if self.dry_run {
            return Ok(());
        }
        let written = match command.kind {
            TranslateType::Xlsx => self.write_workbook(&data, command, &path),
            _ => self.write_text(&data, command, &path),
        };
        written.map_err(|e| Error(format!("SAVE TRANSLATE: {}: {e}", command.outfile)))
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
        let field = |text: String| {
            if csv && text.contains([delimiter, q, '\n', '\r']) {
                format!("{q}{}{q}", text.replace(q, &format!("{q}{q}")))
            } else {
                text
            }
        };
        let variables: Vec<Variable> = data.variables().collect();
        let mut text = String::new();
        let line = |values: Vec<String>, text: &mut String| {
            let values: Vec<String> = values.into_iter().map(field).collect();
            text.push_str(&values.join(&delimiter.to_string()));
            text.push_str("\r\n");
        };
        if let Some(labels) = command.field_names {
            let names = variables.iter().map(|v| match v.label() {
                Some(label) if labels => label.to_string(),
                _ => v.name().to_string(),
            });
            line(names.collect(), &mut text);
        }
        for case in 0..data.case_count() {
            let values = variables.iter().map(|v| match out(v, case, command) {
                Out::Blank => String::new(),
                Out::Text(s) => s,
                Out::Number(x) if command.print_formats => {
                    let shown = format_value(x, &currencies.resolve(v.format()), true);
                    let shown = shown.trim().to_string();
                    let own_mark = is_date_time_or_currency(v.format())
                        || ["COMMA", "DOLLAR", "DOT"].contains(&kind(v.format()).as_str());
                    match comma && !own_mark {
                        true => shown.replace('.', ","),
                        false => shown,
                    }
                }
                Out::Number(x) => plain(x, v.format(), comma),
            });
            line(values.collect(), &mut text);
        }
        let bytes = encode(&text, command.encoding.as_deref(), command.bom)?;
        std::fs::write(path, bytes).map_err(|e| e.to_string())
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
                                    sheet.write_string(row, c, shown.trim()).map_err(xe)?;
                                }
                            }
                        }
                    }
                }
            }
            sheet.autofit();
        }
        book.save(path).map_err(xe)
    }
}
