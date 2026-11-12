//! OUTPUT NEW, NAME, ACTIVATE, CLOSE, DISPLAY and EXPORT: the output documents of a session.
//!
//! Rules, from the descriptions of these commands:
//! - The tables of every command go to the designated document; OUTPUT NEW and OUTPUT
//!   ACTIVATE designate another. When none is open, the next table starts one.
//! - A name belongs to one document: OUTPUT NEW or OUTPUT NAME with a name that another
//!   document has gives that one a new name.
//! - OUTPUT CLOSE does not save; closing a document that does not exist is ignored.
//! - OUTPUT EXPORT writes the designated or the named document, replacing the file.
//!   CREATESHEET adds or replaces a sheet; MODIFYSHEET adds after the last column (the
//!   default), after the last row or from a cell; a sheet that does not exist is created.
//!
//! Choices where the manual is silent:
//! - Documents get the names Document1, Document2, ... The window shows the tables of all
//!   documents in one Output view; the documents decide what OUTPUT EXPORT writes.
//! - OUTPUT ACTIVATE of a name that no document has and OUTPUT CLOSE of one are warnings, so
//!   the run goes on, as the manual has processing continue.
//! - The tables of an export stand one below the other with a blank row between, on one
//!   sheet named by SHEET or Tables. MODIFYSHEET leaves one blank column or row before the
//!   new tables. CREATESHEET and MODIFYSHEET add only to a workbook that OUTPUT EXPORT wrote
//!   in this session and that has not changed since: a workbook is rewritten as a whole, and
//!   what Statscraft did not write (formulas, charts, formatting) would be lost.
//! - TEXT writes UTF-8 for LOCALE. Columns are as wide as their widest cell, or with
//!   COLUMNWIDTH=n wrap at n characters; numbers align right.

use crate::xlsx::{put_table, set_widths, sheet_name};
use crate::{Cell, Engine, Error, Table, cell_text, header_rows};
use rust_xlsxwriter::{Workbook, XlsxError};
use statscraft_syntax::{
    ExportTarget, OutputCommand, OutputExport, SheetLocation, SheetOperation, TextEncoding,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

/// The open output documents.
#[derive(Clone, Default)]
pub(crate) struct Documents {
    open: Vec<Document>,
    designated: Option<usize>,
    /// The number of the last DocumentN name given.
    numbered: usize,
    /// The workbooks OUTPUT EXPORT wrote, by path, which later exports may add sheets to.
    books: HashMap<PathBuf, Book>,
    /// The text of the last TITLE command, the default title of REPORT.
    pub(crate) title: Option<String>,
    /// The files REPORT /OUTFILE wrote in this session, which later reports append to.
    pub(crate) report_files: std::collections::HashSet<PathBuf>,
}

#[derive(Clone)]
struct Document {
    name: String,
    /// Each table with the command that made it.
    tables: Vec<(String, Arc<Table>)>,
}

/// A workbook as OUTPUT EXPORT wrote it, and when.
#[derive(Clone, Default)]
struct Book {
    sheets: Vec<Sheet>,
    written: Option<SystemTime>,
}

#[derive(Clone)]
struct Sheet {
    name: String,
    blocks: Vec<Block>,
}

/// Tables written one below the other from a cell.
#[derive(Clone)]
struct Block {
    top: u32,
    left: u16,
    notes: bool,
    tables: Vec<Arc<Table>>,
}

impl Block {
    /// The row and the column just after the block.
    fn end(&self) -> (u32, u16) {
        let mut row = self.top;
        for (i, table) in self.tables.iter().enumerate() {
            let caption = u32::from(self.notes && table.caption.is_some());
            let depth = header_rows(table).len() as u32;
            row += u32::from(i > 0) + 1 + depth + table.rows.len() as u32 + caption;
        }
        let width = self.tables.iter().map(|t| t.columns.len()).max();
        (row, self.left + width.unwrap_or(0) as u16)
    }
}

impl Sheet {
    /// The row and the column after everything on the sheet; None when it is empty.
    fn end(&self) -> Option<(u32, u16)> {
        let ends = self.blocks.iter().filter(|b| !b.tables.is_empty());
        ends.map(Block::end)
            .reduce(|(r, c), (r2, c2)| (r.max(r2), c.max(c2)))
    }
}

fn save(path: &Path, book: &Book) -> Result<(), XlsxError> {
    let mut workbook = Workbook::new();
    for model in &book.sheets {
        let sheet = workbook.add_worksheet().set_name(&model.name)?;
        let mut widths = Vec::new();
        for block in &model.blocks {
            let mut top = block.top;
            for table in &block.tables {
                top = put_table(sheet, table, (top, block.left), block.notes, &mut widths)? + 1;
            }
        }
        set_widths(sheet, &widths)?;
    }
    workbook.save(path)
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

impl Documents {
    fn find(&self, name: &str) -> Option<usize> {
        self.open
            .iter()
            .position(|d| d.name.eq_ignore_ascii_case(name))
    }

    fn new_name(&mut self) -> String {
        loop {
            self.numbered += 1;
            let name = format!("Document{}", self.numbered);
            if self.find(&name).is_none() {
                return name;
            }
        }
    }

    /// Gives `name` to the document `at`; a document that had it gets a new name.
    fn rename(&mut self, at: usize, name: &str) {
        if let Some(other) = self.find(name).filter(|&other| other != at) {
            self.open[other].name = self.new_name();
        }
        self.open[at].name = name.to_string();
    }

    /// Starts a new document and designates it.
    fn start(&mut self, name: Option<&str>) -> usize {
        let name = match name {
            Some(name) => {
                if let Some(other) = self.find(name) {
                    self.open[other].name = self.new_name();
                }
                name.to_string()
            }
            None => self.new_name(),
        };
        self.open.push(Document {
            name,
            tables: Vec::new(),
        });
        self.designated = Some(self.open.len() - 1);
        self.open.len() - 1
    }

    fn designated(&mut self) -> usize {
        match self.designated {
            Some(at) => at,
            None => self.start(None),
        }
    }

    fn close(&mut self, at: usize) {
        self.open.remove(at);
        self.designated = match self.designated {
            Some(d) if d == at => None,
            Some(d) if d > at => Some(d - 1),
            other => other,
        };
    }
}

/// An open output document, as `describe` and the window list it.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct OutputDocument {
    pub name: String,
    /// The number of tables it holds.
    pub tables: usize,
}

impl Engine {
    /// Adds the tables of one command to the designated document and returns its name; None
    /// when there were no tables.
    pub(crate) fn record_output(&mut self, command: &str, tables: &[Table]) -> Option<String> {
        if tables.is_empty() {
            return None;
        }
        let documents = &mut self.documents;
        let at = documents.designated();
        let tables = tables
            .iter()
            .map(|t| (command.to_string(), Arc::new(t.clone())));
        documents.open[at].tables.extend(tables);
        Some(documents.open[at].name.clone())
    }

    /// The open output documents, in the order they were opened.
    pub fn output_documents(&self) -> Vec<OutputDocument> {
        let open = self.documents.open.iter();
        open.map(|d| OutputDocument {
            name: d.name.clone(),
            tables: d.tables.len(),
        })
        .collect()
    }

    /// The name of the designated document, where the next tables go (OUTPUT NEW and OUTPUT
    /// ACTIVATE set it). None when no document is open: the next table starts one.
    pub fn designated_output(&self) -> Option<&str> {
        let d = &self.documents;
        d.designated.map(|at| d.open[at].name.as_str())
    }

    pub(crate) fn output_command(&mut self, command: &OutputCommand) -> Result<Vec<Table>, Error> {
        let documents = &mut self.documents;
        match command {
            OutputCommand::New(name) => {
                documents.start(name.as_deref());
            }
            OutputCommand::Name(name) => {
                let at = documents.designated();
                documents.rename(at, name);
            }
            OutputCommand::Activate(name) => match documents.find(name) {
                Some(at) => documents.designated = Some(at),
                None => {
                    let current = documents.designated.map(|d| documents.open[d].name.clone());
                    self.warn(format!(
                        "no open output document is named {name}; the tables still go to {}",
                        current.unwrap_or_else(|| "a new document".into())
                    ));
                }
            },
            OutputCommand::Close(None) => {
                if let Some(at) = documents.designated {
                    documents.close(at);
                }
            }
            OutputCommand::Close(Some(name)) => match documents.find(name) {
                Some(at) => documents.close(at),
                None => self.warn(format!(
                    "no open output document is named {name}; nothing was closed"
                )),
            },
            OutputCommand::CloseAll => {
                documents.open.clear();
                documents.designated = None;
            }
            OutputCommand::Display => {
                let rows = documents.open.iter().enumerate().map(|(i, d)| {
                    vec![
                        Cell::Text(d.name.clone()),
                        Cell::Number(d.tables.len() as f64),
                        Cell::Text(if documents.designated == Some(i) {
                            "Yes".into()
                        } else {
                            String::new()
                        }),
                    ]
                });
                return Ok(vec![Table {
                    title: "Output Documents".into(),
                    row_headers: 1,
                    columns: vec!["Name".into(), "Tables".into(), "Designated".into()],
                    rows: rows.collect(),
                    ..Table::default()
                }]);
            }
            OutputCommand::Export(export) => self.export(export)?,
        }
        Ok(Vec::new())
    }

    fn export(&mut self, export: &OutputExport) -> Result<(), Error> {
        let documents = &self.documents;
        let at = match &export.name {
            None => documents.designated,
            Some(name) => Some(documents.find(name).ok_or_else(|| {
                Error(format!(
                    "OUTPUT EXPORT: no open output document is named {name}"
                ))
            })?),
        };
        let tables: Vec<(String, Arc<Table>)> =
            at.map_or_else(Vec::new, |at| documents.open[at].tables.clone());
        if tables.is_empty() {
            self.warn("the output document holds no tables yet; the files have none");
        }
        if self.dry_run {
            return Ok(());
        }
        for target in &export.targets {
            match target {
                ExportTarget::Xlsx {
                    file,
                    operation,
                    sheet,
                    location,
                    notes,
                } => {
                    let tables = tables.iter().map(|(_, t)| t.clone()).collect();
                    self.export_xlsx(
                        file,
                        *operation,
                        sheet.as_deref(),
                        *location,
                        *notes,
                        tables,
                    )?
                }
                ExportTarget::Html {
                    file,
                    notes,
                    styling,
                } => {
                    let pairs = tables.iter().map(|(c, t)| (c.as_str(), t.as_ref()));
                    let text = crate::html::page(pairs, *notes, *styling);
                    self.write_file(file, text.as_bytes())?;
                }
                ExportTarget::Text {
                    file,
                    notes,
                    encoding,
                    column_width,
                    tab,
                    row_border,
                    column_border,
                } => {
                    let mut text = String::new();
                    for (i, (_, table)) in tables.iter().enumerate() {
                        if i > 0 {
                            text.push('\n');
                        }
                        let border = (!*tab).then_some((*row_border, *column_border));
                        text.push_str(&table_text(table, *notes, *column_width, border));
                    }
                    let bytes = match encoding {
                        TextEncoding::Utf16 => {
                            let mut bytes = vec![0xFF, 0xFE];
                            bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
                            bytes
                        }
                        TextEncoding::Locale | TextEncoding::Utf8 => text.into_bytes(),
                    };
                    self.write_file(file, &bytes)?;
                }
                ExportTarget::Csv { file, notes } => {
                    let mut text = String::new();
                    for (i, (_, table)) in tables.iter().enumerate() {
                        if i > 0 {
                            text.push('\n');
                        }
                        text.push_str(&table_csv(table, *notes));
                    }
                    self.write_file(file, text.as_bytes())?;
                }
            }
        }
        Ok(())
    }

    fn write_file(&self, file: &str, bytes: &[u8]) -> Result<(), Error> {
        std::fs::write(self.path(file), bytes)
            .map_err(|e| Error(format!("OUTPUT EXPORT: cannot write {file}: {e}")))
    }

    fn export_xlsx(
        &mut self,
        file: &str,
        operation: SheetOperation,
        sheet: Option<&str>,
        location: SheetLocation,
        notes: bool,
        tables: Vec<Arc<Table>>,
    ) -> Result<(), Error> {
        let path = self.path(file);
        let books = &mut self.documents.books;
        let mut book = match operation {
            SheetOperation::CreateFile => Book::default(),
            _ if !path.exists() => Book::default(),
            _ => match books.get(&path) {
                Some(book) if book.written.is_some() && book.written == modified(&path) => {
                    book.clone()
                }
                _ => {
                    return Err(Error(format!(
                        "OUTPUT EXPORT: {file} was not written by OUTPUT EXPORT in this session, \
                         or it changed since; CREATESHEET and MODIFYSHEET add only to such a \
                         workbook, because the whole workbook is written again and what else it \
                         holds would be lost. Use OPERATION=CREATEFILE or another file"
                    )));
                }
            },
        };
        let block = |top, left| Block {
            top,
            left,
            notes,
            tables: tables.clone(),
        };
        let existing = sheet.and_then(|name| {
            book.sheets
                .iter()
                .position(|s| s.name.eq_ignore_ascii_case(name))
        });
        match (operation, existing) {
            (SheetOperation::ModifySheet, Some(at)) => {
                let model = &mut book.sheets[at];
                let (top, left) = match (location, model.end()) {
                    (SheetLocation::StartCell(row, column), _) => (row, column),
                    (_, None) => (0, 0),
                    (SheetLocation::LastColumn, Some((_, column))) => (0, column + 1),
                    (SheetLocation::LastRow, Some((row, _))) => (row + 1, 0),
                };
                model.blocks.push(block(top, left));
            }
            (SheetOperation::CreateSheet, Some(at)) => book.sheets[at].blocks = vec![block(0, 0)],
            _ => {
                let mut used: Vec<String> = book.sheets.iter().map(|s| s.name.clone()).collect();
                let name = match sheet {
                    Some(name) => name.to_string(),
                    None => sheet_name("Tables", &mut used),
                };
                let (top, left) = match (operation, location) {
                    (SheetOperation::ModifySheet, SheetLocation::StartCell(row, column)) => {
                        (row, column)
                    }
                    _ => (0, 0),
                };
                book.sheets.push(Sheet {
                    name,
                    blocks: vec![block(top, left)],
                });
            }
        }
        save(&path, &book)
            .map_err(|e| Error(format!("OUTPUT EXPORT: cannot write {file}: {e}")))?;
        book.written = modified(&path);
        books.insert(path, book);
        Ok(())
    }
}

/// Numbers, and text that stands for one, such as <5 or .001, go right.
fn right(table: &Table, row: usize, column: usize) -> bool {
    match &table.rows[row][column] {
        Cell::Number(_) => true,
        Cell::Text(text) => text
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || "-+.<>".contains(c)),
        Cell::Empty => false,
    }
}

/// `text` cut into lines of at most `width` characters, at blanks where it can.
fn wrap(text: &str, width: Option<u32>) -> Vec<String> {
    let Some(width) = width.map(|w| w.max(1) as usize) else {
        return vec![text.to_string()];
    };
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split(' ') {
        let mut word: Vec<char> = word.chars().collect();
        loop {
            let used = line.chars().count();
            let room = if used == 0 {
                width
            } else {
                width.saturating_sub(used + 1)
            };
            if word.len() <= room {
                if used > 0 {
                    line.push(' ');
                }
                line.extend(&word);
                break;
            }
            if used > 0 {
                lines.push(std::mem::take(&mut line));
                continue;
            }
            // a word longer than the column is cut
            lines.push(word[..width].iter().collect());
            word.drain(..width);
        }
    }
    lines.push(line);
    lines
}

/// One table as text: its title, then with `border` (row, column) the columns lined up
/// between borders, else the cells separated by tabs.
fn table_text(
    table: &Table,
    notes: bool,
    column_width: Option<u32>,
    border: Option<(char, char)>,
) -> String {
    let mut out = format!("{}\n", table.title);
    let headers = header_rows(table);
    let columns = table.columns.len();
    // each header row as a grid row: a spanning label in its first column
    let mut grid: Vec<(Vec<String>, Vec<bool>)> = headers
        .iter()
        .map(|cells| {
            let mut row = vec![String::new(); columns];
            for cell in cells {
                row[cell.first] = cell.label.clone();
            }
            (row, vec![false; columns])
        })
        .collect();
    let header_count = grid.len();
    for r in 0..table.rows.len() {
        let texts = (0..columns).map(|c| {
            if c < table.rows[r].len() {
                cell_text(table, r, c)
            } else {
                String::new()
            }
        });
        let rights = (0..columns).map(|c| c < table.rows[r].len() && right(table, r, c));
        grid.push((texts.collect(), rights.collect()));
    }
    let Some((row_border, column_border)) = border else {
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], " ");
        for (row, _) in &grid {
            let cells: Vec<String> = row.iter().map(|c| clean(c)).collect();
            out.push_str(&cells.join("\t"));
            out.push('\n');
        }
        if let Some(caption) = table.caption.as_ref().filter(|_| notes) {
            out.push_str(&clean(caption));
            out.push('\n');
        }
        return out;
    };
    let wrapped: Vec<Vec<Vec<String>>> = grid
        .iter()
        .map(|(row, _)| row.iter().map(|c| wrap(c, column_width)).collect())
        .collect();
    let mut widths = vec![0; columns];
    for row in &wrapped {
        for (c, lines) in row.iter().enumerate() {
            let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
            widths[c] = widths[c].max(longest);
        }
    }
    let total: usize = widths.iter().map(|w| w + 3).sum::<usize>() + 1;
    let rule = format!("{}\n", row_border.to_string().repeat(total));
    out.push_str(&rule);
    for (r, row) in wrapped.iter().enumerate() {
        let height = row.iter().map(Vec::len).max().unwrap_or(1);
        for line in 0..height {
            out.push(column_border);
            for (c, lines) in row.iter().enumerate() {
                let text = lines.get(line).map_or("", String::as_str);
                let pad = widths[c] - text.chars().count();
                if grid[r].1[c] {
                    out.push_str(&format!(" {}{text} ", " ".repeat(pad)));
                } else {
                    out.push_str(&format!(" {text}{} ", " ".repeat(pad)));
                }
                out.push(column_border);
            }
            out.push('\n');
        }
        if r + 1 == header_count {
            out.push_str(&rule);
        }
    }
    out.push_str(&rule);
    if let Some(caption) = table.caption.as_ref().filter(|_| notes) {
        out.push_str(caption);
        out.push('\n');
    }
    out
}

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

/// One table as CSV: the title, the header rows (a spanning label in its first column), the
/// body and, with `notes`, the caption, each on a line of its own.
fn table_csv(table: &Table, notes: bool) -> String {
    let line = |cells: Vec<String>| -> String {
        let fields: Vec<String> = cells.iter().map(|c| csv_field(c)).collect();
        format!("{}\n", fields.join(","))
    };
    let mut out = line(vec![table.title.clone()]);
    for cells in header_rows(table) {
        let mut row = vec![String::new(); table.columns.len()];
        for cell in cells {
            row[cell.first] = cell.label;
        }
        out.push_str(&line(row));
    }
    for r in 0..table.rows.len() {
        out.push_str(&line(
            (0..table.rows[r].len())
                .map(|c| cell_text(table, r, c))
                .collect(),
        ));
    }
    if let Some(caption) = table.caption.as_ref().filter(|_| notes) {
        out.push_str(&line(vec![caption.clone()]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::wrap;

    #[test]
    fn wrapping_breaks_at_blanks_and_cuts_long_words() {
        assert_eq!(wrap("Valid Percent", Some(8)), ["Valid", "Percent"]);
        assert_eq!(wrap("abcdefghij", Some(4)), ["abcd", "efgh", "ij"]);
        assert_eq!(wrap("a b c", Some(3)), ["a b", "c"]);
        assert_eq!(wrap("", Some(3)), [""]);
        assert_eq!(wrap("as is", None), ["as is"]);
    }
}
