//! PRINT, PRINT EJECT, PRINT SPACE, WRITE and XSAVE: transformations that give something for
//! every case the data pass reads.
//!
//! Rules, from the descriptions of these commands:
//! - They run case by case when the data are next read, in the order of the cases; their output
//!   comes before that of the procedure that reads the data.
//! - A slash starts a line. Variables without a format show in their print format (PRINT) or
//!   write format (WRITE); PRINT puts a blank after each. A format list or columns after a list
//!   of variables apply to all of them (an asterisk or a text ends the list), with no blank
//!   after. Columns are shared equally by the variables; a format list needs one format per
//!   variable; Tn goes to column n and nX skips n columns. A text goes where it stands, or from
//!   the column after it.
//! - User-missing values show as valid values. System-missing is a period on PRINT and blanks on
//!   WRITE.
//! - PRINT EJECT starts a new page; PRINT SPACE gives blank lines, as many as its expression.
//! - WRITE has no carriage control. ENCODING applies only with OUTFILE; a UTF encoding gets a
//!   byte order mark unless BOM=NO.
//! - XSAVE writes the cases as they reach it, scratch variables left out; KEEP, DROP and RENAME
//!   shape the file as on SAVE, and MAP lists its variables.
//!
//! Choices where the manual is silent:
//! - A system-missing value on PRINT shows its period where the decimal point would be (the
//!   last column when there are no decimals), as PSPP does.
//! - PRINT to a file starts every line with the carriage control: a blank, or 1 for the first
//!   line after PRINT EJECT, as PSPP does. On the screen a new page is a new table.
//! - A file named on PRINT, WRITE or XSAVE is written anew at every data pass; the commands of
//!   one pass that name the same file write to it in case order.
//! - PRINT SPACE with a negative or missing number of lines prints one line, with a warning,
//!   as PSPP does; a fraction is cut off.
//! - Without ENCODING the file is UTF-8 without a byte order mark. RECORDS that does not match
//!   the lines gives a warning. A line has no length limit.
//! - The pass at the end of a run (see `Engine::finish`) also runs them.

use crate::{Cell, Engine, Error, Table, format_value};
use statscraft_data::{Dataset, Values};
use statscraft_syntax::{
    Command, Compression, FormatItem, Placement, Print, PrintItem, PrintKind, PrintSpace, Xsave,
};
use std::path::PathBuf;

/// What one of these commands gave for a case, collected in order during the pass.
#[derive(Clone)]
pub(crate) enum CaseOutput {
    Lines {
        /// As written; None: the output.
        file: Option<String>,
        encoding: Option<String>,
        bom: bool,
        /// PRINT: a carriage control character starts each line in a file.
        carriage: bool,
        /// PRINT EJECT: a new page starts with these lines.
        eject: bool,
        lines: Vec<String>,
    },
    /// One case for XSAVE (no case at all for the pass on no case, which gives the
    /// dictionary).
    Case { xsave: Box<Xsave>, data: Dataset },
}

/// One thing placed on a line: a variable in a format, or a text.
enum Piece {
    Value { name: String, format: String },
    Text(String),
}

struct Layout {
    /// Per line: (1-based column, piece).
    lines: Vec<Vec<(usize, Piece)>>,
}

/// (type, width, decimals) of a format text such as "F8.2".
fn split_format(format: &str) -> (String, usize, usize) {
    let format = format.split_once(' ').map_or(format, |(f, _)| f);
    let letters = format
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(format.len());
    let (kind, size) = format.split_at(letters);
    let (width, decimals) = size.split_once('.').unwrap_or((size, "0"));
    (
        kind.to_ascii_uppercase(),
        width.parse().unwrap_or(8),
        decimals.parse().unwrap_or(0),
    )
}

fn is_string_format(kind: &str) -> bool {
    kind == "A" || kind == "AHEX"
}

/// Writes `text` into `line` from the 1-based `column`, over what is there.
fn put(line: &mut Vec<char>, column: usize, text: &str) {
    let start = column.saturating_sub(1);
    for (i, c) in text.chars().enumerate() {
        if start + i < line.len() {
            line[start + i] = c;
        } else {
            line.resize(start + i, ' ');
            line.push(c);
        }
    }
}

fn pad(text: &str, width: usize) -> String {
    let mut out: String = text.chars().take(width).collect();
    let n = out.chars().count();
    out.extend(std::iter::repeat_n(' ', width - n));
    out
}

/// The bytes of `text` in an encoding named on ENCODING.
fn encode(text: &str, encoding: Option<&str>, bom: bool) -> Result<Vec<u8>, Error> {
    let Some(name) = encoding else {
        return Ok(text.as_bytes().to_vec());
    };
    let key: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase();
    let utf16 = |big: bool| {
        let mut bytes = Vec::new();
        let units = (bom.then_some(0xFEFF))
            .into_iter()
            .chain(text.encode_utf16());
        for unit in units {
            bytes.extend(if big {
                unit.to_be_bytes()
            } else {
                unit.to_le_bytes()
            });
        }
        bytes
    };
    Ok(match key.as_str() {
        "UTF8" | "LOCALE" => {
            let mut bytes = if bom && key == "UTF8" {
                vec![0xEF, 0xBB, 0xBF]
            } else {
                Vec::new()
            };
            bytes.extend(text.as_bytes());
            bytes
        }
        "UTF16" | "UTF16LE" => utf16(false),
        "UTF16BE" => utf16(true),
        _ => {
            let encoding = statscraft_syntax::encoding_for(name)
                .ok_or_else(|| Error(format!("unknown encoding {name}")))?;
            let (bytes, _, unmappable) = encoding.encode(text);
            if unmappable {
                return Err(Error(format!(
                    "the text holds characters that {name} cannot write"
                )));
            }
            bytes.into_owned()
        }
    })
}

impl Engine {
    /// PRINT, PRINT EJECT, WRITE, PRINT SPACE and XSAVE on the one case of a case-by-case pass,
    /// or on no case to check them and give their tables.
    pub(crate) fn case_output(&mut self, command: &Command) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let cases = data.case_count();
        if cases > 1 {
            return Err(Error("PRINT, WRITE and XSAVE run case by case only".into()));
        }
        // a case outside the open DO IF branch or loop gives nothing
        let open = cases == 1 && self.active_cases().is_none_or(|active| active[0]);
        match command {
            Command::Print(print) => {
                let layout = self.layout(print)?;
                let mut tables = Vec::new();
                if cases == 0 {
                    let lines = print.lines.len() as u32;
                    if print.records.is_some_and(|records| records != lines) {
                        self.warn(format!(
                            "RECORDS={} does not match the {lines} lines given; RECORDS is ignored",
                            print.records.unwrap_or(0)
                        ));
                    }
                    if print.table {
                        tables.push(self.layout_table(print, &layout));
                    }
                }
                if open {
                    let lines = self.render(print.kind == PrintKind::Write, &layout)?;
                    self.case_outputs.push(CaseOutput::Lines {
                        file: print.outfile.clone(),
                        encoding: print.encoding.clone(),
                        bom: !print.no_bom,
                        carriage: print.kind != PrintKind::Write,
                        eject: print.kind == PrintKind::PrintEject,
                        lines,
                    });
                }
                Ok(tables)
            }
            Command::PrintSpace(space) => {
                self.print_space(space, open)?;
                Ok(Vec::new())
            }
            Command::Xsave(xsave) => self.xsave(xsave, cases == 0 || open),
            _ => unreachable!("only case output comes here"),
        }
    }

    fn print_space(&mut self, space: &PrintSpace, open: bool) -> Result<(), Error> {
        let count = match &space.lines {
            None => 1,
            Some(expr) => match self.evaluator(None)?.eval(expr)? {
                crate::expr::Column::Number(values) => match values.first().copied().flatten() {
                    Some(n) if n >= 0.0 => n.min(1e6) as usize,
                    // a case outside the open branch, or the pass on no case, prints nothing
                    _ if !open => 0,
                    n => {
                        let shown = n.map_or("system-missing".to_string(), |n| n.to_string());
                        self.warn(format!(
                            "the expression on PRINT SPACE gave {shown}; one line is printed"
                        ));
                        1
                    }
                },
                crate::expr::Column::String(_) => {
                    return Err(Error(
                        "PRINT SPACE needs a number of lines, not a string".into(),
                    ));
                }
            },
        };
        if open {
            self.case_outputs.push(CaseOutput::Lines {
                file: space.outfile.clone(),
                encoding: space.encoding.clone(),
                bom: true,
                carriage: true,
                eject: false,
                lines: vec![String::new(); count],
            });
        }
        Ok(())
    }

    fn xsave(&mut self, xsave: &Xsave, open: bool) -> Result<Vec<Table>, Error> {
        if matches!(xsave.compression, Some(Compression::Uncompressed)) {
            return Err(Error("XSAVE /UNCOMPRESSED is not supported yet".into()));
        }
        let mut data = self.active()?.clone();
        let scratch: Vec<String> = data
            .variable_names()
            .filter(|n| crate::rows::is_scratch(n))
            .map(str::to_string)
            .collect();
        if scratch.len() == data.variable_count() {
            return Err(Error("XSAVE has no variables to save".into()));
        }
        let names: Vec<&str> = scratch.iter().map(String::as_str).collect();
        data.delete_variables(&names)?;
        let mut tables = Vec::new();
        if data.case_count() == 0 && xsave.map {
            // the labels carry the names in the active dataset through KEEP, DROP and RENAME
            let mut named = data.clone();
            let all: Vec<String> = named.variable_names().map(str::to_string).collect();
            for name in &all {
                named.set_variable_label(name, Some(name))?;
            }
            let named = self.file_options(named, &xsave.options)?;
            let rows = named
                .variables()
                .map(|v| {
                    vec![
                        Cell::Text(v.name().into()),
                        Cell::Text(v.label().unwrap_or_default().into()),
                    ]
                })
                .collect();
            tables.push(Table {
                title: "Variables in the file".into(),
                columns: vec!["Name in the file".into(), "Variable".into()],
                rows,
                ..Table::default()
            });
        }
        let data = self.file_options(data, &xsave.options)?;
        if open {
            self.case_outputs.push(CaseOutput::Case {
                xsave: Box::new(xsave.clone()),
                data,
            });
        }
        Ok(tables)
    }

    /// Where every variable and text of the command goes.
    fn layout(&self, print: &Print) -> Result<Layout, Error> {
        let data = self.active()?;
        let write = print.kind == PrintKind::Write;
        let mut lines = Vec::new();
        for items in &print.lines {
            let mut cursor = 1;
            let mut placed = Vec::new();
            for item in items {
                match item {
                    PrintItem::Text { text, columns } => {
                        let text = match columns {
                            Some((first, last)) => {
                                cursor = *first as usize;
                                match last {
                                    Some(last) if *last >= *first => {
                                        pad(text, (last - first + 1) as usize)
                                    }
                                    _ => text.clone(),
                                }
                            }
                            None => text.clone(),
                        };
                        let width = text.chars().count();
                        placed.push((cursor, Piece::Text(text)));
                        cursor += width;
                    }
                    PrintItem::Variables(vars, placement) => {
                        let names = self.expand(vars)?;
                        let fits = |name: &str, kind: &str, width: usize| -> Result<(), Error> {
                            let variable = data.variable(name)?;
                            let string = variable.is_string();
                            // WRITE writes a string in its full width
                            let full = match kind {
                                "AHEX" => variable.string_width().map(|w| w * 2),
                                _ => variable.string_width(),
                            };
                            if write && string && full.is_some_and(|full| full != width) {
                                return Err(Error(format!(
                                    "{name} is {} bytes wide; WRITE needs {kind}{} for it",
                                    variable.string_width().unwrap_or(0),
                                    full.unwrap_or(0)
                                )));
                            }
                            match (string, is_string_format(kind)) {
                                (true, false) => Err(Error(format!(
                                    "{name} is a string variable; give it an A format"
                                ))),
                                (false, true) => Err(Error(format!(
                                    "{name} is numeric and cannot take format {kind}"
                                ))),
                                _ => Ok(()),
                            }
                        };
                        match placement {
                            None => {
                                for name in names {
                                    let v = data.variable(&name)?;
                                    let format = if write { v.write_format() } else { v.format() };
                                    let width = split_format(format).1;
                                    let format = format.to_string();
                                    placed.push((cursor, Piece::Value { name, format }));
                                    cursor += width;
                                    if !write {
                                        placed.push((cursor, Piece::Text(" ".into())));
                                        cursor += 1;
                                    }
                                }
                            }
                            Some(Placement::Columns {
                                first,
                                last,
                                kind,
                                decimals,
                            }) => {
                                let columns = (last - first + 1) as usize;
                                if !columns.is_multiple_of(names.len()) {
                                    return Err(Error(format!(
                                        "columns {first}-{last} cannot be shared equally by {} \
                                         variables",
                                        names.len()
                                    )));
                                }
                                let share = columns / names.len();
                                for (i, name) in names.into_iter().enumerate() {
                                    let string = data.variable(&name)?.is_string();
                                    let kind = match kind {
                                        Some(kind) => kind.name(),
                                        None if string => "A",
                                        None => "F",
                                    };
                                    fits(&name, kind, share)?;
                                    let format = match (is_string_format(kind), decimals) {
                                        (false, d) if *d > 0 => format!("{kind}{share}.{d}"),
                                        _ => format!("{kind}{share}"),
                                    };
                                    let column = *first as usize + i * share;
                                    placed.push((column, Piece::Value { name, format }));
                                }
                                cursor = *last as usize + 1;
                            }
                            Some(Placement::Formats(items)) => {
                                let formats = items
                                    .iter()
                                    .filter(|i| matches!(i, FormatItem::Format(_)))
                                    .count();
                                if formats != names.len() {
                                    return Err(Error(format!(
                                        "{} variables but {formats} formats: give one format \
                                         per variable",
                                        names.len()
                                    )));
                                }
                                let mut names = names.into_iter();
                                for item in items {
                                    match item {
                                        FormatItem::Tab(column) => cursor = *column as usize,
                                        FormatItem::Skip(n) => cursor += *n as usize,
                                        FormatItem::Format(format) => {
                                            let name = names.next().expect("counted");
                                            fits(&name, format.kind.name(), format.width as usize)?;
                                            let mut text =
                                                format!("{}{}", format.kind.name(), format.width);
                                            if format.decimals > 0 {
                                                text.push_str(&format!(".{}", format.decimals));
                                            }
                                            placed.push((
                                                cursor,
                                                Piece::Value { name, format: text },
                                            ));
                                            cursor += format.width as usize;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            lines.push(placed);
        }
        Ok(Layout { lines })
    }

    /// The lines of the one case.
    fn render(&self, write: bool, layout: &Layout) -> Result<Vec<String>, Error> {
        let data = self.active()?;
        let mut out = Vec::new();
        for placed in &layout.lines {
            let mut line = Vec::new();
            for (column, piece) in placed {
                match piece {
                    Piece::Text(text) => put(&mut line, *column, text),
                    Piece::Value { name, format } => {
                        let text = self.value_text(data, name, format, write)?;
                        put(&mut line, *column, &text);
                    }
                }
            }
            out.push(line.into_iter().collect());
        }
        Ok(out)
    }

    /// The value of the variable in the one case, exactly as wide as the format.
    fn value_text(
        &self,
        data: &Dataset,
        name: &str,
        format: &str,
        write: bool,
    ) -> Result<String, Error> {
        let (kind, width, decimals) = split_format(format);
        Ok(match data.variable(name)?.values() {
            Values::String(values) => {
                let value = values.get(0);
                if kind == "AHEX" {
                    let hex: String = value.bytes().map(|b| format!("{b:02X}")).collect();
                    let filler = "20".repeat(width.div_ceil(2));
                    pad(&(hex + &filler), width)
                } else {
                    pad(value, width)
                }
            }
            Values::Numeric(values) => match values.get(0) {
                None if write => " ".repeat(width),
                None => {
                    let at = if decimals > 0 && decimals + 1 < width {
                        width - decimals - 1
                    } else {
                        width - 1
                    };
                    let mut text = " ".repeat(width);
                    text.replace_range(at..at + 1, ".");
                    text
                }
                Some(value) => {
                    let text = format_value(value, &self.currency_format(format), false);
                    if text.chars().count() > width {
                        "*".repeat(width)
                    } else {
                        format!("{text:>width$}")
                    }
                }
            },
        })
    }

    fn layout_table(&self, print: &Print, layout: &Layout) -> Table {
        let mut rows = Vec::new();
        for (record, placed) in layout.lines.iter().enumerate() {
            for (column, piece) in placed {
                if let Piece::Value { name, format } = piece {
                    let width = split_format(format).1;
                    rows.push(vec![
                        Cell::Text(name.clone()),
                        Cell::Number(record as f64 + 1.0),
                        Cell::Text(format!("{column}-{}", column + width - 1)),
                        Cell::Text(format.clone()),
                    ]);
                }
            }
        }
        let count = print.lines.len();
        Table {
            title: format!(
                "Writing {count} record{}",
                if count == 1 { "" } else { "s" }
            ),
            columns: vec![
                "Variable".into(),
                "Record".into(),
                "Columns".into(),
                "Format".into(),
            ],
            rows,
            ..Table::default()
        }
    }

    /// At the end of a case-by-case pass: shows the lines, writes the files.
    pub(crate) fn emit_case_output(&mut self, outputs: Vec<CaseOutput>) -> Result<(), Error> {
        let mut tables: Vec<Table> = Vec::new();
        let mut texts: Vec<(PathBuf, Option<String>, bool, String)> = Vec::new();
        let mut saves: Vec<(PathBuf, Box<Xsave>, Vec<Dataset>)> = Vec::new();
        for output in outputs {
            match output {
                CaseOutput::Lines {
                    file: None,
                    eject,
                    lines,
                    ..
                } => {
                    if eject || tables.is_empty() {
                        tables.push(Table {
                            title: "Text output".into(),
                            columns: vec!["Line".into()],
                            ..Table::default()
                        });
                    }
                    let table = tables.last_mut().expect("pushed above");
                    table
                        .rows
                        .extend(lines.into_iter().map(|l| vec![Cell::Text(l)]));
                }
                CaseOutput::Lines {
                    file: Some(file),
                    encoding,
                    bom,
                    carriage,
                    eject,
                    lines,
                } => {
                    let path = self.path(&file);
                    let at = match texts.iter().position(|t| t.0 == path) {
                        Some(at) => at,
                        None => {
                            texts.push((path, encoding, bom, String::new()));
                            texts.len() - 1
                        }
                    };
                    let text = &mut texts[at].3;
                    for (i, line) in lines.iter().enumerate() {
                        if carriage {
                            text.push(if eject && i == 0 { '1' } else { ' ' });
                        }
                        text.push_str(line);
                        text.push('\n');
                    }
                }
                CaseOutput::Case { xsave, data } => {
                    let path = self.path(&xsave.outfile);
                    match saves.iter_mut().find(|s| s.0 == path) {
                        Some(save) if save.1 != xsave => {
                            return Err(Error(format!(
                                "two XSAVE commands write {} in one data pass",
                                path.display()
                            )));
                        }
                        Some(save) => save.2.push(data),
                        None => saves.push((path, xsave, vec![data])),
                    }
                }
            }
        }
        self.flushed.extend(tables);
        if self.dry_run {
            return Ok(());
        }
        for (path, encoding, bom, text) in texts {
            let bytes = encode(&text, encoding.as_deref(), bom)?;
            std::fs::write(&path, bytes).map_err(|e| Error(format!("{}: {e}", path.display())))?;
        }
        for (path, xsave, parts) in saves {
            let data = Dataset::from_parts(&parts[0], &parts)?;
            data.write_sav(&path)?;
            if let Some(read_only) = xsave.read_only {
                crate::output::set_permissions(&path, read_only)?;
            }
        }
        Ok(())
    }
}
