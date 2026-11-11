//! GET DATA /TYPE=XLSX, XLSM and XLS: one sheet of a workbook as the new active dataset.
//!
//! Choices where the manual is silent:
//! - The file's content decides how it is read, so a workbook with the other extension or TYPE
//!   still opens.
//! - CELLRANGE=FULL reads from the first to the last cell that holds something. Rows with no
//!   value are left out, and so are columns with neither a name nor a value.
//! - Without DATATYPEMIN a column is numeric when all its values are numbers (TRUE and FALSE
//!   are 1 and 0), a date when all are dates or times, else a string. With it, the kind that
//!   at least that percentage of the values share wins; the other values are system-missing.
//! - Numbers get F with 0 decimals when all are whole, else 2, at least 8 wide. Dates get
//!   DATE11, times of day and durations TIME8, a mix or a date with a time DATETIME20.
//! - In a string column a number is written as short as it reads back, a date as
//!   yyyy-mm-dd, a time as hh:mm:ss. A string variable is as wide as its longest value; for
//!   TYPE=XLS at most ASSUMEDSTRWIDTH (255 by default), for XLSX only when it is given.
//! - Error cells (#N/A, #NULL! and the like) are empty.
//! - READNAMES turns a name into a valid one: other characters become _, a name that does not
//!   start with a letter gets a V in front, a reserved word an _ after it, a repeated name _2,
//!   _3 and so on. A column without a name is V and its number in the range (V1, V2, ...).

use crate::expr::{DAYS_TO_1970, civil_from_days, days_from_civil};
use crate::{Engine, Error};
use calamine::{Data, Reader};
use statscraft_data::Dataset;
use statscraft_syntax::{Format, FormatType, GetData, Sheet, Spreadsheet};
use std::collections::HashSet;
use std::io::{Cursor, Read};

const DAY: f64 = 86_400.0;

/// A cell as a variable sees it; dates and times in seconds as the date formats count them.
#[derive(Clone, Debug, PartialEq)]
enum Cell {
    Blank,
    Number(f64),
    Date(f64),
    DateTime(f64),
    Time(f64),
    Text(String),
}

fn cell(data: Option<&Data>) -> Cell {
    match data {
        None | Some(Data::Empty | Data::Error(_)) => Cell::Blank,
        Some(Data::Int(n)) => Cell::Number(*n as f64),
        Some(Data::Float(n)) => Cell::Number(*n),
        Some(Data::Bool(b)) => Cell::Number(f64::from(u8::from(*b))),
        Some(Data::String(s)) if s.trim().is_empty() => Cell::Blank,
        Some(Data::String(s)) => Cell::Text(s.clone()),
        Some(Data::DateTime(d)) if d.is_duration() || d.as_f64() < 1.0 => {
            Cell::Time(d.as_f64() * DAY)
        }
        Some(Data::DateTime(d)) => {
            let (y, m, day, h, min, s, ms) = d.to_ymd_hms_milli();
            let days = days_from_civil(y.into(), m.into(), day.into()) + DAYS_TO_1970;
            let time = f64::from(h) * 3600.0
                + f64::from(min) * 60.0
                + f64::from(s)
                + f64::from(ms) / 1000.0;
            match time {
                0.0 => Cell::Date(days as f64 * DAY),
                t => Cell::DateTime(days as f64 * DAY + t),
            }
        }
        Some(Data::DateTimeIso(s)) => {
            let format = Format {
                kind: FormatType::Datetime,
                width: 20,
                decimals: 0,
            };
            match crate::get_data::number(s, format, false) {
                Ok(Some(v)) if v % DAY == 0.0 => Cell::Date(v),
                Ok(Some(v)) => Cell::DateTime(v),
                _ => Cell::Text(s.clone()),
            }
        }
        Some(Data::DurationIso(s)) => Cell::Text(s.clone()),
    }
}

/// hh:mm:ss of a number of seconds, with the fraction when there is one.
pub(crate) fn clock(seconds: f64) -> String {
    let sign = if seconds < 0.0 { "-" } else { "" };
    let s = seconds.abs();
    let whole = s.trunc() as i64;
    let mut out = format!(
        "{sign}{:02}:{:02}:{:02}",
        whole / 3600,
        whole / 60 % 60,
        whole % 60
    );
    let fraction = s.fract();
    if fraction > 0.0 {
        let digits = format!("{fraction:.3}");
        out.push_str(digits.trim_start_matches('0').trim_end_matches('0'));
    }
    out
}

/// yyyy-mm-dd of a date value.
fn day_text(value: f64) -> String {
    let (y, m, d) = civil_from_days((value / DAY).floor() as i64 - DAYS_TO_1970);
    format!("{y:04}-{m:02}-{d:02}")
}

/// A cell as the text of a string variable.
fn text(cell: &Cell) -> String {
    match cell {
        Cell::Blank => String::new(),
        Cell::Number(n) => format!("{n}"),
        Cell::Date(v) => day_text(*v),
        Cell::DateTime(v) => format!("{} {}", day_text(*v), clock(v.rem_euclid(DAY))),
        Cell::Time(v) => clock(*v),
        Cell::Text(s) => s.clone(),
    }
}

/// A valid variable name made of a column heading, not yet in `used` (lower case), which it
/// joins. Blank: V and the column's number.
pub fn variable_name(heading: &str, column: usize, used: &mut HashSet<String>) -> String {
    let mut name: String = heading
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || "_.$#@".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    if !name.is_empty() && !name.starts_with(char::is_alphabetic) {
        name.insert(0, 'V');
    }
    name = statscraft_data::cut_to(name.trim_end_matches('.'), 60)
        .trim_end_matches('.')
        .to_string();
    if !name.is_empty() && !statscraft_data::is_valid_name(&name) {
        // only a reserved word is left
        name.push('_');
    }
    if name.is_empty() || !statscraft_data::is_valid_name(&name) {
        name = format!("V{}", column + 1);
    }
    let mut unique = name.clone();
    for n in 2.. {
        if used.insert(unique.to_lowercase()) {
            break;
        }
        unique = format!("{name}_{n}");
    }
    unique
}

/// The sheets of a workbook in their order, for a dialog that offers them.
pub fn workbook_sheets(path: &std::path::Path) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let book = calamine::open_workbook_auto_from_rs(Cursor::new(bytes.as_slice()))
        .map_err(|e| e.to_string())?;
    Ok(book.sheet_names())
}

/// The value of `name="..."` in the text of an XML element.
fn attribute<'a>(element: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let start = element.find(&key)? + key.len();
    Some(&element[start..start + element[start..].find('"')?])
}

/// The elements of `xml` with the tag `tag` (with or without a namespace prefix), each as the
/// text after the tag name up to the next element.
fn elements<'a>(xml: &'a str, tag: &'a str) -> impl Iterator<Item = &'a str> {
    xml.split('<').filter_map(move |piece| {
        let name_end = piece.find([' ', '/', '>']).unwrap_or(piece.len());
        let name = &piece[..name_end];
        let local = name.rsplit(':').next().unwrap_or(name);
        (local == tag).then_some(&piece[name_end..])
    })
}

/// The hidden rows and columns (from 0) of the `index`th sheet of an XLSX workbook.
fn hidden_cells(bytes: &[u8], index: usize) -> Result<(HashSet<u32>, HashSet<u32>), String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut part = |name: &str| -> Result<String, String> {
        let mut text = String::new();
        let mut file = zip.by_name(name).map_err(|e| format!("{name}: {e}"))?;
        file.read_to_string(&mut text).map_err(|e| e.to_string())?;
        Ok(text)
    };
    let workbook = part("xl/workbook.xml")?;
    let relations = part("xl/_rels/workbook.xml.rels")?;
    let id = elements(&workbook, "sheet")
        .nth(index)
        .and_then(|s| attribute(s, "r:id"))
        .ok_or("the workbook lists no such sheet")?;
    let target = elements(&relations, "Relationship")
        .find(|r| attribute(r, "Id") == Some(id))
        .and_then(|r| attribute(r, "Target"))
        .ok_or("the workbook does not say where the sheet is")?;
    let path = match target.strip_prefix('/') {
        Some(absolute) => absolute.to_string(),
        None => format!("xl/{target}"),
    };
    let sheet = part(&path)?;
    let hidden = |e: &str| matches!(attribute(e, "hidden"), Some("1" | "true"));
    let number = |e: &str, name: &str| attribute(e, name).and_then(|v| v.parse::<u32>().ok());
    let rows = elements(&sheet, "row")
        .filter(|e| hidden(e))
        .filter_map(|e| number(e, "r"))
        .map(|r| r - 1)
        .collect();
    let mut columns = HashSet::new();
    for e in elements(&sheet, "col").filter(|e| hidden(e)) {
        if let (Some(min), Some(max)) = (number(e, "min"), number(e, "max")) {
            columns.extend((min - 1)..max.min(16_384));
        }
    }
    Ok((rows, columns))
}

impl Engine {
    /// The sheet GET DATA asks for as a dataset.
    pub(crate) fn read_workbook(
        &mut self,
        command: &GetData,
        options: &Spreadsheet,
    ) -> Result<Dataset, Error> {
        let fail = |e: String| Error(format!("GET DATA: {}: {e}", command.file));
        let bytes = std::fs::read(self.path(&command.file)).map_err(|e| fail(e.to_string()))?;
        let mut book = calamine::open_workbook_auto_from_rs(Cursor::new(bytes.as_slice()))
            .map_err(|e| fail(format!("not a workbook that can be read ({e})")))?;
        let names = book.sheet_names();
        let index = match &options.sheet {
            Sheet::Index(n) => {
                let index = *n as usize - 1;
                if index >= names.len() {
                    return Err(fail(format!(
                        "there is no sheet {n}; the workbook has {}",
                        names.len()
                    )));
                }
                index
            }
            Sheet::Name(name) => names
                .iter()
                .position(|s| s == name)
                .or_else(|| names.iter().position(|s| s.eq_ignore_ascii_case(name)))
                .ok_or_else(|| {
                    fail(format!(
                        "there is no sheet '{name}'; the sheets are {}",
                        names.join(", ")
                    ))
                })?,
        };
        let range = book
            .worksheet_range(&names[index])
            .map_err(|e| fail(e.to_string()))?;
        let (hidden_rows, hidden_columns) =
            if options.ignore_hidden && matches!(book, calamine::Sheets::Xlsx(_)) {
                hidden_cells(&bytes, index).map_err(fail)?
            } else {
                Default::default()
            };
        let (r0, c0, r1, c1) = match (options.range, range.start(), range.end()) {
            (Some(bounds), _, _) => bounds,
            (None, Some((r0, c0)), Some((r1, c1))) => (r0, c0, r1, c1),
            _ => return Err(fail(format!("sheet '{}' is empty", names[index]))),
        };
        let columns: Vec<u32> = (c0..=c1).filter(|c| !hidden_columns.contains(c)).collect();
        let mut rows = (r0..=r1).filter(|r| !hidden_rows.contains(r)).map(|r| {
            let row: Vec<Cell> = columns
                .iter()
                .map(|&c| cell(range.get_value((r, c))))
                .collect();
            (r, row)
        });
        let headings: Vec<String> = match options.read_names {
            true => rows
                .next()
                .map(|(_, row)| row.iter().map(text).collect())
                .unwrap_or_default(),
            false => Vec::new(),
        };
        let rows: Vec<(u32, Vec<Cell>)> = rows
            .filter(|(_, row)| row.iter().any(|c| *c != Cell::Blank))
            .collect();
        let mut data = Dataset::new(rows.len());
        let mut used = HashSet::new();
        let mut warnings = Vec::new();
        for (k, _) in columns.iter().enumerate() {
            let heading = headings.get(k).map_or("", |h| h.trim());
            let values: Vec<&Cell> = rows.iter().map(|(_, row)| &row[k]).collect();
            if heading.is_empty() && values.iter().all(|c| **c == Cell::Blank) {
                continue;
            }
            let name = variable_name(heading, k, &mut used);
            let column = Column {
                name: &name,
                values: &values,
                rows: &rows,
            };
            column.store(
                &mut data,
                options,
                command.ignore_leading_spaces,
                &mut warnings,
            )?;
        }
        self.warn_some(warnings);
        Ok(data)
    }
}

struct Column<'a> {
    name: &'a str,
    values: &'a [&'a Cell],
    rows: &'a [(u32, Vec<Cell>)],
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Number,
    Date,
    Text,
}

impl Column<'_> {
    fn kind(&self, type_min: Option<f64>) -> Kind {
        let of = |c: &Cell| match c {
            Cell::Blank => None,
            Cell::Number(_) => Some(Kind::Number),
            Cell::Text(_) => Some(Kind::Text),
            _ => Some(Kind::Date),
        };
        let kinds: Vec<Kind> = self.values.iter().filter_map(|c| of(c)).collect();
        let share = |kind| kinds.iter().filter(|k| **k == kind).count() as f64 * 100.0;
        let needed = type_min.unwrap_or(100.0) * kinds.len() as f64;
        if share(Kind::Number) >= needed {
            Kind::Number
        } else if share(Kind::Date) >= needed {
            Kind::Date
        } else {
            Kind::Text
        }
    }

    fn store(
        &self,
        data: &mut Dataset,
        options: &Spreadsheet,
        leading: bool,
        warnings: &mut Vec<String>,
    ) -> Result<(), Error> {
        let name = self.name;
        // the row of the sheet a value comes from, as Excel numbers it
        let row = |case: usize| self.rows[case].0 + 1;
        let kind = self.kind(options.type_min);
        if kind == Kind::Text {
            let limit = options
                .assumed_width
                .map(|w| w as usize)
                .or(options.binary.then_some(255))
                .unwrap_or(32_767);
            let mut values: Vec<String> = self.values.iter().map(|c| text(c)).collect();
            if leading {
                for v in &mut values {
                    *v = v.trim_start().to_string();
                }
            }
            let length = |v: &String| match options.ignore_trailing_spaces {
                true => v.trim_end().len(),
                false => v.len(),
            };
            let width = values.iter().map(length).max().unwrap_or(0).clamp(1, limit);
            for (case, v) in values.iter_mut().enumerate() {
                let cut = statscraft_data::cut_to(v, width).to_string();
                if cut.len() < v.trim_end().len() {
                    warnings.push(format!("row {}: {name} is cut to {width} bytes", row(case)));
                }
                *v = cut;
            }
            data.set_string_values(name, width, values)?;
            return Ok(());
        }
        let mut wanted = |case: usize, what: &str| {
            warnings.push(format!(
                "row {}: {name} is system-missing: the cell holds {what}",
                row(case)
            ));
            None
        };
        let values: Vec<Option<f64>> = self
            .values
            .iter()
            .enumerate()
            .map(|(case, c)| match (kind, c) {
                (_, Cell::Blank) => None,
                (Kind::Number, Cell::Number(n)) => Some(*n),
                (Kind::Date, Cell::Date(v) | Cell::DateTime(v) | Cell::Time(v)) => Some(*v),
                (Kind::Number, _) => wanted(case, "no number"),
                _ => wanted(case, "no date or time"),
            })
            .collect();
        let format = match kind {
            Kind::Date => {
                let all =
                    |f: fn(&Cell) -> bool| self.values.iter().all(|c| **c == Cell::Blank || f(c));
                if all(|c| matches!(c, Cell::Date(_))) {
                    "DATE11".to_string()
                } else if all(|c| matches!(c, Cell::Time(_))) {
                    "TIME8".to_string()
                } else {
                    "DATETIME20".to_string()
                }
            }
            _ => {
                let numbers = values.iter().flatten();
                let decimals = if numbers.clone().all(|n| n.fract() == 0.0) {
                    0
                } else {
                    2
                };
                let digits = numbers
                    .map(|n| format!("{:.0}", n.trunc()).len())
                    .max()
                    .unwrap_or(1);
                let width = (digits + if decimals > 0 { 3 } else { 0 }).clamp(8, 40);
                format!("F{width}.{decimals}")
            }
        };
        data.set_numeric_values(name, values)?;
        data.set_format(name, &format)?;
        Ok(())
    }
}
