//! OUTPUT NEW, NAME, ACTIVATE, CLOSE, DISPLAY, EXPORT, SAVE and OPEN: the output documents of
//! a session.
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
//! - OUTPUT SAVE writes the designated or the named document and leaves it open; a file that
//!   exists is replaced; a name no document has is an error.
//! - OUTPUT OPEN reads a file into a new document, which becomes the designated one, named by
//!   NAME (taken from a document that had it) or with a new name. A file that is missing or of
//!   another kind lets the run go on with the designation as it was.
//!
//! Choices where the manual is silent:
//! - Documents get the names Document1, Document2, ... The window shows each document in an
//!   output window of its own; the documents decide what OUTPUT EXPORT writes.
//! - OUTPUT ACTIVATE of a name that no document has and OUTPUT CLOSE of one are warnings, so
//!   the run goes on, as the manual has processing continue.
//! - The tables of an export stand one below the other with a blank row between, on one
//!   sheet named by SHEET or Tables. MODIFYSHEET leaves one blank column or row before the
//!   new tables. CREATESHEET and MODIFYSHEET add only to a workbook that OUTPUT EXPORT wrote
//!   in this session and that has not changed since: a workbook is rewritten as a whole, and
//!   what Statscraft did not write (formulas, charts, formatting) would be lost.
//! - TEXT writes UTF-8 for LOCALE. Columns are as wide as their widest cell, or with
//!   COLUMNWIDTH=n wrap at n characters; numbers align right.
//! - Pictures of charts: HTML holds each one inline (a data URL) in IMAGEFORMAT; TEXT writes
//!   each one as a file beside the text, named after it with a number from 0, and puts the
//!   chart's title and that file name where the chart stands. Without a document subcommand
//!   every image subcommand writes its own files, root0, root1, ... A chart of a kind not
//!   drawn yet, and every chart while its format is EMF or EPS, goes out as its data table.
//!   The pictures are on light paper; 100 PERCENTSIZE is the PNG the command line saves.
//!   DEPTH=COLORS16 and COLORS256 give full color, and COMPRESS=YES an uncompressed BMP.
//! - Documents are saved as Statscraft output files (`output_file`). A path with the extension
//!   of another program's output files is written with .scout instead, with a warning, and
//!   OUTPUT OPEN reads the .scout file of that name: their files are never overwritten.
//! - OUTPUT OPEN of a file that cannot be read is a warning, so the run goes on.
//! - A document remembers the file it was opened from or saved to, and whether items were
//!   added or deleted since, so the window can ask before closing it.

use crate::office::{CHART_COLUMNS, CHART_ROWS, Item, has_beside, put_chart};
use crate::output_file::{self, CommandItem};
use crate::picture;
use crate::xlsx::{put_table, set_widths, sheet_name};
use crate::{Cell, Chart, Engine, Error, Table, cell_text, header_rows};
use rust_xlsxwriter::{Workbook, XlsxError};
use statscraft_syntax::{
    ExportTarget, ImageExport, ImageFormat, ImageTone, OutputCommand, OutputExport, OutputOpen,
    OutputSave, SheetLocation, SheetOperation, TextEncoding,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

/// The open output documents.
#[derive(Clone, Default)]
pub(crate) struct Documents {
    pub(crate) open: Vec<Document>,
    pub(crate) designated: Option<usize>,
    /// The number of the last DocumentN name given.
    numbered: usize,
    /// The workbooks OUTPUT EXPORT wrote, by path, which later exports may add sheets to.
    books: HashMap<PathBuf, Book>,
    /// The text of the last TITLE command, the default title of REPORT.
    pub(crate) title: Option<String>,
    /// The files REPORT /OUTFILE wrote in this session, which later reports append to.
    pub(crate) report_files: std::collections::HashSet<PathBuf>,
    /// What the last OUTPUT OPEN read: the document's name, the file as written and its items,
    /// for the command's result.
    opened: Option<(String, String, Vec<output_file::Item>)>,
    /// What the last OUTPUT MODIFY changed, for the command's result.
    pub(crate) modified: Vec<crate::output_modify::Modified>,
}

#[derive(Clone)]
pub(crate) struct Document {
    pub(crate) name: String,
    /// The output of each command, in order, as an output file holds it.
    pub(crate) items: Vec<output_file::Item>,
    /// The file it was opened from or last saved to.
    file: Option<PathBuf>,
    /// Items were added, changed or deleted since it was opened, saved or started.
    pub(crate) changed: bool,
}

impl Document {
    /// Its tables and charts, each with the command that made it; a chart as its data table
    /// and itself. Items of an unknown kind are left out.
    fn shown(&self) -> Vec<(String, Shown)> {
        let leaves = output_file::leaves(&self.items).into_iter();
        leaves
            .filter_map(|(command, item)| {
                let shown = match item {
                    output_file::Item::Table(t) => (t.clone(), None),
                    output_file::Item::Chart(c) => (Arc::new(c.data_table()), Some(c.clone())),
                    _ => return None,
                };
                Some((command.to_string(), shown))
            })
            .collect()
    }

    fn count(&self) -> usize {
        output_file::leaves(&self.items).len()
    }
}

/// The extensions of another program's output files, which OUTPUT SAVE never writes.
const FOREIGN: [&str; 3] = ["spv", "spo", "spw"];

/// `path` with the .scout extension when it has one of another program's output files.
fn ours(path: &Path) -> Option<PathBuf> {
    let extension = path.extension()?.to_str()?;
    FOREIGN
        .iter()
        .any(|f| f.eq_ignore_ascii_case(extension))
        .then(|| path.with_extension(output_file::EXTENSION))
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

/// A table, or a chart with its data table.
type Shown = (Arc<Table>, Option<Arc<Chart>>);

/// The item of an export that `shown` is.
fn item((table, chart): &Shown) -> Item<'_> {
    match chart {
        Some(chart) => Item::Chart(chart),
        None => Item::Table(table),
    }
}

/// Tables written one below the other from a cell.
#[derive(Clone)]
struct Block {
    top: u32,
    left: u16,
    notes: bool,
    tables: Vec<Shown>,
}

impl Block {
    /// The row and the column just after the block.
    fn end(&self) -> (u32, u16) {
        let mut row = self.top;
        for (i, (table, chart)) in self.tables.iter().enumerate() {
            let caption = u32::from(self.notes && table.caption.is_some());
            let depth = header_rows(table).len() as u32;
            let mut height = 1 + depth + table.rows.len() as u32 + caption;
            if chart.as_deref().is_some_and(has_beside) {
                height = height.max(1 + CHART_ROWS);
            }
            row += u32::from(i > 0) + height;
        }
        let width = self.tables.iter().map(|(table, chart)| {
            let beside = chart.as_deref().is_some_and(has_beside);
            table.columns.len() as u16 + u16::from(beside) * (1 + CHART_COLUMNS)
        });
        (row, self.left + width.max().unwrap_or(0))
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
            for (table, chart) in &block.tables {
                let at = (top, block.left);
                top = match chart {
                    Some(chart) => {
                        put_chart(sheet, table, chart, at, block.notes, &mut widths, true)
                    }
                    None => put_table(sheet, table, at, block.notes, &mut widths),
                }? + 1;
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
            items: Vec::new(),
            file: None,
            changed: false,
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
    /// The number of tables it holds; a chart counts as one.
    pub tables: usize,
    /// The file it was opened from or last saved to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// Output was added or deleted since it was opened, saved or started.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unsaved: bool,
}

impl Engine {
    /// Adds the tables and the charts of one command (those it made so far, before
    /// `take_charts`), with its syntax and warnings, to the designated document and returns its
    /// name; None when there were none. With `append` they join the document's last command,
    /// which is this one.
    pub(crate) fn record_output(
        &mut self,
        command: &str,
        syntax: &str,
        tables: &[Table],
        append: bool,
    ) -> Option<String> {
        if tables.is_empty() && self.charts.is_empty() {
            return None;
        }
        use output_file::Item as Out;
        let tables = tables.iter().map(|t| Out::Table(Arc::new(t.clone())));
        let charts = self.charts.iter().map(|c| Out::Chart(Arc::new(c.clone())));
        let items: Vec<Out> = tables.chain(charts).collect();
        let warnings = self.warnings.clone();
        let documents = &mut self.documents;
        let at = documents.designated();
        let document = &mut documents.open[at];
        document.changed = true;
        match document.items.last_mut() {
            Some(Out::Command(last)) if append => last.items.extend(items),
            _ => document.items.push(Out::Command(CommandItem {
                command: command.to_string(),
                label: None,
                syntax: syntax.to_string(),
                warnings,
                items,
            })),
        }
        Some(document.name.clone())
    }

    /// The open output documents, in the order they were opened.
    pub fn output_documents(&self) -> Vec<OutputDocument> {
        let open = self.documents.open.iter();
        open.map(|d| OutputDocument {
            name: d.name.clone(),
            tables: d.count(),
            file: d.file.clone(),
            unsaved: d.changed,
        })
        .collect()
    }

    /// The items of an open output document, as an output file holds them.
    pub fn output_items(&self, name: &str) -> Option<&[output_file::Item]> {
        let d = &self.documents;
        d.find(name).map(|at| d.open[at].items.as_slice())
    }

    /// What the last OUTPUT OPEN read, taken: the document, the file as written, the items.
    pub(crate) fn take_opened(&mut self) -> Option<(String, String, Vec<output_file::Item>)> {
        self.documents.opened.take()
    }

    /// Deletes the tables, charts and other items at `positions` (counted as
    /// `output_file::leaves` counts them) from a document, as the output window does. Returns
    /// the titles of what went.
    pub(crate) fn delete_output(
        &mut self,
        name: &str,
        positions: &[usize],
    ) -> Result<Vec<String>, Error> {
        let d = &mut self.documents;
        let at = d
            .find(name)
            .ok_or_else(|| Error(format!("no open output document is named {name}")))?;
        let document = &mut d.open[at];
        let leaves = output_file::leaves(&document.items);
        let titles = positions
            .iter()
            .filter_map(|&p| leaves.get(p).map(|(_, item)| item.shown().0.title))
            .collect();
        if !output_file::remove(&mut document.items, positions) {
            return Err(Error(format!(
                "the output window and the document {name} hold different items; nothing was \
                 deleted"
            )));
        }
        document.changed |= !positions.is_empty();
        Ok(titles)
    }

    /// What the last OUTPUT MODIFY changed, taken.
    pub(crate) fn take_modified(&mut self) -> Vec<crate::output_modify::Modified> {
        std::mem::take(&mut self.documents.modified)
    }

    /// The document `name` names, else the designated one; None when none is open.
    pub(crate) fn document_at(
        &self,
        command: &str,
        name: Option<&str>,
    ) -> Result<Option<usize>, Error> {
        let documents = &self.documents;
        match name {
            None => Ok(documents.designated),
            Some(name) => documents.find(name).map(Some).ok_or_else(|| {
                Error(format!(
                    "{command}: no open output document is named {name}"
                ))
            }),
        }
    }

    /// Where the tables go now, for a warning.
    fn tables_go(&self) -> String {
        let d = &self.documents;
        let current = d.designated.map(|at| d.open[at].name.clone());
        current.unwrap_or_else(|| "a new document".into())
    }

    fn save_output(&mut self, save: &OutputSave) -> Result<(), Error> {
        let at = self.document_at("OUTPUT SAVE", save.name.as_deref())?;
        let mut path = self.path(&save.file);
        if let Some(scout) = ours(&path) {
            self.warn(format!(
                "{} is the name of another program's output file; Statscraft wrote its own \
                 output file {} instead and left {} alone",
                save.file,
                scout.display(),
                save.file
            ));
            path = scout;
        }
        if at.is_none() {
            self.warn("no output document is open; the file holds no output");
        }
        if self.dry_run {
            return Ok(());
        }
        let items = at.map_or(&[][..], |at| &self.documents.open[at].items[..]);
        std::fs::write(&path, output_file::write(items))
            .map_err(|e| Error(format!("OUTPUT SAVE: cannot write {}: {e}", save.file)))?;
        if let Some(at) = at {
            let document = &mut self.documents.open[at];
            document.file = Some(path);
            document.changed = false;
        }
        Ok(())
    }

    fn open_output(&mut self, open: &OutputOpen) -> Result<(), Error> {
        let file = &open.file;
        let mut path = self.path(file);
        if let Some(scout) = ours(&path) {
            if !scout.exists() {
                self.warn(format!(
                    "{file} is an output file of another program, which Statscraft cannot read; \
                     the tables still go to {}",
                    self.tables_go()
                ));
                return Ok(());
            }
            path = scout;
        }
        let read = std::fs::read_to_string(&path).map_err(|e| e.to_string());
        let read = read.and_then(|t| output_file::read_checked(&t).map_err(|e| e.to_string()));
        let items = match read {
            Ok((items, warnings)) => {
                for warning in warnings {
                    self.warn(format!("{file}: {warning}"));
                }
                items
            }
            Err(e) => {
                self.warn(format!(
                    "cannot open {file}: {e}; the tables still go to {}",
                    self.tables_go()
                ));
                return Ok(());
            }
        };
        let documents = &mut self.documents;
        let at = documents.start(open.name.as_deref());
        let document = &mut documents.open[at];
        document.items = items.clone();
        document.file = Some(path);
        documents.opened = Some((document.name.clone(), file.clone(), items));
        Ok(())
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
                        Cell::Number(d.count() as f64),
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
            OutputCommand::Save(save) => self.save_output(save)?,
            OutputCommand::Open(open) => self.open_output(open)?,
            OutputCommand::Modify(modify) => return self.output_modify(modify),
        }
        Ok(Vec::new())
    }

    fn export(&mut self, export: &OutputExport) -> Result<(), Error> {
        let at = self.document_at("OUTPUT EXPORT", export.name.as_deref())?;
        let mut all = at.map_or_else(Vec::new, |at| self.documents.open[at].shown());
        // CONTENTS EXPORT=VISIBLE leaves out what OUTPUT MODIFY hid
        all.retain(|(_, (table, _))| !(export.visible_only && table.hidden));
        let shown: Vec<Shown> = all.iter().map(|(_, s)| s.clone()).collect();
        let tables: Vec<(String, Arc<Table>)> = all
            .iter()
            .map(|(c, (t, _))| (c.clone(), t.clone()))
            .collect();
        if tables.is_empty() {
            self.warn("the output document holds no tables yet; the files have none");
        }
        if self.dry_run {
            return Ok(());
        }
        // the settings of the image subcommand of a format, or the defaults
        let settings = |format| {
            let given = export.images.iter().find(|i| i.format == format);
            given.cloned().unwrap_or(ImageExport {
                format,
                root: None,
                percent: 100.0,
                tone: ImageTone::Color,
            })
        };
        // without a document, the image subcommands write the pictures alone; with one,
        // IMAGEROOT is ignored
        if export.targets.is_empty() {
            for image in &export.images {
                let root = image.root.as_deref().unwrap_or_default();
                let (extension, _) = picture::file_kind(image.format);
                let pictures = self.pictures(&shown, image)?;
                let charts = shown.iter().filter(|(_, c)| c.is_some()).count();
                let drawn: Vec<Vec<u8>> = pictures.into_iter().flatten().collect();
                let raster = !matches!(image.format, ImageFormat::Emf | ImageFormat::Eps);
                if charts == 0 {
                    self.warn("OUTPUT EXPORT: the output document holds no charts, so no picture files are written");
                } else if raster && drawn.len() < charts {
                    self.warn(format!(
                        "OUTPUT EXPORT: {} of {charts} charts are of a kind not drawn yet and \
                         have no picture file",
                        charts - drawn.len()
                    ));
                }
                for (n, bytes) in drawn.iter().enumerate() {
                    self.write_file(&format!("{root}{n}.{extension}"), bytes)?;
                }
            }
        }
        for target in &export.targets {
            match target {
                ExportTarget::Xlsx {
                    file,
                    operation,
                    sheet,
                    location,
                    notes,
                } => self.export_xlsx(
                    file,
                    *operation,
                    sheet.as_deref(),
                    *location,
                    *notes,
                    shown.clone(),
                )?,
                ExportTarget::Html {
                    file,
                    notes,
                    styling,
                    image_format,
                } => {
                    let image = settings(*image_format);
                    let pictures = self.pictures(&shown, &image)?;
                    let (_, media) = picture::file_kind(image.format);
                    let width = (picture::FILE_WIDTH as f64 * image.percent / 100.0).round();
                    let pieces = all
                        .iter()
                        .zip(pictures)
                        .map(|((command, (table, chart)), p)| {
                            let piece = match (p, chart) {
                                (Some(bytes), Some(chart)) => {
                                    crate::html::figure(&bytes, media, width, &chart.title)
                                }
                                _ => crate::html::table_html(table, *notes),
                            };
                            (command.as_str(), piece)
                        });
                    let text = crate::html::page(pieces, *styling);
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
                    image_format,
                } => {
                    let image = settings(*image_format);
                    let pictures = self.pictures(&shown, &image)?;
                    // the pictures are named after the text file: its name without the
                    // extension and a number from 0
                    let root = Path::new(file).with_extension("");
                    let root = root.to_string_lossy();
                    let (extension, _) = picture::file_kind(image.format);
                    let mut text = String::new();
                    let mut n = 0;
                    for (i, ((table, chart), p)) in shown.iter().zip(pictures).enumerate() {
                        if i > 0 {
                            text.push('\n');
                        }
                        if let (Some(bytes), Some(chart)) = (p, chart) {
                            let name = format!("{root}{n}.{extension}");
                            n += 1;
                            self.write_file(&name, &bytes)?;
                            let shown_name = Path::new(&name)
                                .file_name()
                                .map_or_else(|| name.clone(), |f| f.to_string_lossy().into_owned());
                            text.push_str(&format!("{}\n{shown_name}\n", chart.title));
                            continue;
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
                ExportTarget::Doc { file, notes } => {
                    let items: Vec<Item> = shown.iter().map(item).collect();
                    let bytes = crate::office::document(&items, *notes)
                        .map_err(|e| Error(format!("OUTPUT EXPORT: {file}: {e}")))?;
                    self.write_file(file, &bytes)?;
                }
                ExportTarget::Ppt {
                    file,
                    notes,
                    titles,
                } => {
                    let items: Vec<Item> = shown.iter().map(item).collect();
                    let bytes = crate::office::presentation(&items, *notes, *titles)
                        .map_err(|e| Error(format!("OUTPUT EXPORT: {file}: {e}")))?;
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

    /// The picture of each chart of `shown` with the settings of `image`, in the order of
    /// `shown`: None for a table and for a chart of a kind not drawn yet. EMF and EPS are not
    /// written yet: a warning, and no pictures.
    fn pictures(
        &mut self,
        shown: &[Shown],
        image: &ImageExport,
    ) -> Result<Vec<Option<Vec<u8>>>, Error> {
        if matches!(image.format, ImageFormat::Emf | ImageFormat::Eps) {
            if shown.iter().any(|(_, chart)| chart.is_some()) {
                let (extension, _) = picture::file_kind(image.format);
                self.warn(format!(
                    "OUTPUT EXPORT: {} pictures are not written yet; charts go out as their \
                     data table, and no {extension} files are written",
                    extension.to_uppercase()
                ));
            }
            return Ok(vec![None; shown.len()]);
        }
        shown
            .iter()
            .map(|(_, chart)| {
                let file = chart.as_deref().and_then(|c| picture::chart_file(c, image));
                file.transpose()
                    .map_err(|e| Error(format!("OUTPUT EXPORT: cannot draw a chart: {e}")))
            })
            .collect()
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
        tables: Vec<Shown>,
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
