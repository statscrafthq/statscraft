//! OMS, OMSEND, OMSINFO and OMSLOG: requests that send the output of the commands after them
//! to files, and keep chosen output out of the results.
//!
//! Rules, from the descriptions of these commands:
//! - A request lasts until OMSEND ends it. Requests are independent: one item can go to several
//!   files. Which items stay in the results is decided, item by item, by the latest request
//!   that chooses it.
//! - An item is chosen when its kind is selected, it meets every list of /IF, and it does not
//!   meet every list of /EXCEPTIF. COMMANDS, SUBTYPES and LABELS compare without case; a
//!   trailing * matches any text starting with what precedes it. INSTANCES keep the nth (or
//!   last) of the items of one command that met the kinds and the other /IF lists.
//! - FORMAT=SAV writes tables only: the string variables Command_, Subtype_ and Label_, the row
//!   labels as string variables Var1, Var2, ... and one variable per column, its name made of
//!   the column's header (levels joined by an underscore): characters a name cannot hold are
//!   dropped, a name that cannot start as it does gets @ in front, trailing underscores and
//!   periods go, and a repeated name gets _A, _B, ... Every table adds cases; a column header
//!   another table had adds to its variable, a new one adds a variable. NUMBERED adds the
//!   number of the table, TableNumber_ unless named.
//! - Requests that write one file share it, when their formats agree.
//! - /COLUMNS moves the named row and column elements to the columns and the rest to the rows,
//!   for the files only.
//! - OMSEND without TAG and FILE ends every request, and the log unless only LOG is given. A
//!   tag or a file no request has gives a warning.
//! - OMSLOG writes a line (TEXT) or an element (XML) when a request starts or ends; a new
//!   OMSLOG ends the log before it; TEXT with APPEND=YES adds to a file that exists.
//!
//! Choices where the manual is silent:
//! - Statscraft has no notes tables and no layers. Command identifiers are command names
//!   (Frequencies is FREQUENCIES) with the aliases of OUTPUT MODIFY; a subtype is a table
//!   title, and the one-variable tables of FREQUENCIES are also of subtype Frequencies.
//!   Subtype_ holds the subtype and Label_ the outline label (the title unless OUTPUT MODIFY
//!   gave one).
//! - Kinds: TABLES are tables, except those of ECHO, TITLE and SUBTITLE, which are TEXTS;
//!   CHARTS are charts; WARNINGS are the warnings of one command, as one item labelled
//!   Warnings. HEADINGS, LOGS, MODELS and TREES choose nothing: the files show each command's
//!   heading where the format has headings.
//! - Each file is written again after every command that adds to it, so it is complete
//!   without OMSEND; a request that never got an item writes no file (a warning at OMSEND).
//! - A table whose number of row labels differs from the first table of a data file is left
//!   out of it with a warning, instead of the file not being written at all.
//! - A column whose cells are all numbers (or empty) is a numeric variable, with as many
//!   decimals as its cells show; any other column is a string variable. The variable label
//!   is the column's full header.
//! - DIMNAMES finds a row element by the header above its label column; a table with none of
//!   the names keeps its layout. SEQUENCE counts row elements (R) by label column and column
//!   elements (C) by header level, outermost first; L names nothing.
//! - TEXT and TABTEXT leave charts out; HTML puts them in as pictures unless IMAGES=NO.
//! - Formats Statscraft does not write (OXML, SPV, XLS and the report formats), XMLWORKSPACE
//!   and OUTPUTSET give a warning; such a request writes nothing and hides nothing, and only
//!   OMSINFO lists it. Its other keywords that change nothing give a warning unless NOWARN.
//! - A request without TAG is named $Oms1, $Oms2, ... in start order.
//! - A file that cannot be written fails the command whose output it was writing.

use crate::output_docs::Shown;
use crate::output_modify::ALIASES;
use crate::{Cell, Engine, Error, Table, cell_text};
use statscraft_data::Dataset;
use statscraft_syntax::{
    ExportTarget, ImageExport, ImageTone, Instance, Oms, OmsColumns, OmsCondition, OmsKind, Omsend,
    Omslog, OutputExport, SheetLocation, SheetOperation, TextEncoding,
};
use std::collections::HashSet;
use std::sync::Arc;

const WRITTEN: [&str; 7] = ["SAV", "HTML", "TEXT", "TABTEXT", "XLSX", "DOC", "PDF"];

#[derive(Clone, Default)]
pub(crate) struct State {
    requests: Vec<Request>,
    destinations: Vec<Destination>,
    started: usize,
    log: Option<Log>,
}

#[derive(Clone)]
struct Request {
    spec: Arc<Oms>,
    tag: String,
    /// The key of its destination; None when it writes nothing.
    destination: Option<String>,
}

#[derive(Clone)]
struct Destination {
    /// The absolute path, or "dataset " and the lower-case name.
    key: String,
    outfile: String,
    format: &'static str,
    numbered: Option<String>,
    spec: Arc<Oms>,
    tables: Vec<SavTable>,
    items: Vec<(String, Shown)>,
    written: bool,
}

#[derive(Clone)]
struct SavTable {
    command: String,
    subtype: String,
    label: String,
    table: Table,
}

#[derive(Clone)]
struct Log {
    path: std::path::PathBuf,
    text: bool,
    /// XML: the elements so far, as the file is written whole each time.
    elements: Vec<String>,
}

/// One item of a command's output, as a request sees it.
struct Item {
    kind: OmsKind,
    label: String,
    subtype: Option<String>,
    what: What,
}

#[derive(Clone, Copy)]
enum What {
    Table(usize),
    Chart(usize),
    Warnings,
}

fn matches(pattern: &str, text: &str) -> bool {
    let (pattern, text) = (pattern.trim().to_lowercase(), text.trim().to_lowercase());
    match pattern.strip_suffix('*') {
        Some(start) => text.starts_with(start),
        None => pattern == text,
    }
}

fn names_command(pattern: &str, command: &str) -> bool {
    matches(pattern, command)
        || ALIASES
            .iter()
            .any(|(alias, name)| *name == command && matches(pattern, alias))
}

fn meets(c: &OmsCondition, command: &str, item: &Item) -> bool {
    (c.commands.is_empty() || c.commands.iter().any(|p| names_command(p, command)))
        && (c.subtypes.is_empty()
            || item
                .subtype
                .as_deref()
                .is_some_and(|s| c.subtypes.iter().any(|p| matches(p, s))))
        && (c.labels.is_empty() || c.labels.iter().any(|p| matches(p, &item.label)))
}

/// The items of one command a request chooses, in order.
fn chosen(spec: &Oms, command: &str, items: &[Item]) -> Vec<usize> {
    let met: Vec<usize> = (0..items.len())
        .filter(|&i| {
            spec.kinds.contains(&items[i].kind) && meets(&spec.condition, command, &items[i])
        })
        .collect();
    let met = if spec.condition.instances.is_empty() {
        met
    } else {
        let picked: Vec<usize> = spec
            .condition
            .instances
            .iter()
            .filter_map(|instance| match instance {
                Instance::Nth(n) => met.get(n - 1).copied(),
                Instance::Last => met.last().copied(),
            })
            .collect();
        met.into_iter().filter(|i| picked.contains(i)).collect()
    };
    let excepted =
        |i: &usize| !spec.exception.is_empty() && meets(&spec.exception, command, &items[*i]);
    met.into_iter().filter(|i| !excepted(i)).collect()
}

fn subtype(command: &str, table: &Table) -> String {
    // the one-variable tables of FREQUENCIES are titled by their variable
    if command == "FREQUENCIES" && table.title != "Statistics" {
        return "Frequencies".into();
    }
    table.title.clone()
}

fn kind_names(kinds: &[OmsKind]) -> String {
    if kinds.len() == 8 {
        return "ALL".into();
    }
    let names: Vec<&str> = kinds.iter().map(|k| k.name()).collect();
    names.join(" ")
}

/// The header of a body column from its outermost level to its leaf, "" for a missing level.
fn column_path(table: &Table, column: usize, depth: usize) -> Vec<String> {
    let groups = table
        .column_groups
        .get(column)
        .map_or(&[][..], Vec::as_slice);
    let mut path: Vec<String> = (0..depth - 1)
        .map(|level| groups.get(level).cloned().unwrap_or_default())
        .collect();
    path.push(table.columns[column].clone());
    path
}

/// The table laid out with the elements `columns` names in the columns, each found as ('R' or
/// 'C', position from 1), and every other element in the rows: row elements first, then
/// column levels. The table as it is when no named element exists.
fn pivot(table: &Table, columns: &OmsColumns) -> Table {
    let h = table.row_headers;
    let depth = 1 + table.column_groups.iter().map(Vec::len).max().unwrap_or(0);
    let mut wanted: Vec<(char, usize)> = Vec::new();
    let mut want = |e: (char, usize)| {
        if !wanted.contains(&e) {
            wanted.push(e);
        }
    };
    match columns {
        OmsColumns::Dimnames(names) => {
            for name in names {
                let found =
                    (0..h).find(|&i| table.columns[i].trim().eq_ignore_ascii_case(name.trim()));
                if let Some(i) = found {
                    want(('R', i + 1));
                }
            }
        }
        OmsColumns::Sequence(items) => {
            for &(place, position) in items {
                let count = match place {
                    'R' => h,
                    'C' => depth,
                    _ => 0,
                };
                match position {
                    Some(n) if n <= count => want((place, n)),
                    Some(_) => {}
                    None => (1..=count).for_each(|n| want((place, n))),
                }
            }
        }
    }
    let default: Vec<(char, usize)> = (1..=depth).map(|n| ('C', n)).collect();
    if wanted.is_empty() || wanted == default {
        return table.clone();
    }
    let rest: Vec<(char, usize)> = (1..=h)
        .map(|n| ('R', n))
        .chain(default)
        .filter(|e| !wanted.contains(e))
        .collect();
    let mut row_keys: Vec<Vec<String>> = Vec::new();
    let mut col_keys: Vec<Vec<String>> = Vec::new();
    let mut cells = Vec::new();
    for r in 0..table.rows.len() {
        for c in h..table.columns.len().min(table.rows[r].len()) {
            let path = column_path(table, c, depth);
            let value = |&(place, n): &(char, usize)| match place {
                'R' => cell_text(table, r, n - 1),
                _ => path[n - 1].clone(),
            };
            let (rk, ck): (Vec<String>, Vec<String>) = (
                rest.iter().map(value).collect(),
                wanted.iter().map(value).collect(),
            );
            let index = |keys: &mut Vec<Vec<String>>, key: Vec<String>| match keys
                .iter()
                .position(|k| *k == key)
            {
                Some(i) => i,
                None => {
                    keys.push(key);
                    keys.len() - 1
                }
            };
            let (ri, ci) = (index(&mut row_keys, rk), index(&mut col_keys, ck));
            let format = table.formats.get(r).and_then(|f| f.get(c)).cloned();
            cells.push((ri, ci, table.rows[r][c].clone(), format.unwrap_or_default()));
        }
    }
    let n = rest.len();
    let mut columns: Vec<String> = rest
        .iter()
        .map(|&(place, i)| match place {
            'R' => table.columns[i - 1].clone(),
            _ => String::new(),
        })
        .collect();
    let mut groups = vec![Vec::new(); n];
    for key in &col_keys {
        columns.push(key.last().cloned().unwrap_or_default());
        groups.push(key[..key.len() - 1].to_vec());
    }
    let width = columns.len();
    let mut rows: Vec<Vec<Cell>> = row_keys
        .iter()
        .map(|key| {
            let mut row: Vec<Cell> = key.iter().map(|k| Cell::Text(k.clone())).collect();
            row.resize(width, Cell::Empty);
            row
        })
        .collect();
    let mut formats = vec![vec![String::new(); width]; rows.len()];
    for (ri, ci, cell, format) in cells {
        rows[ri][n + ci] = cell;
        formats[ri][n + ci] = format;
    }
    Table {
        title: table.title.clone(),
        row_headers: n,
        columns,
        column_groups: groups,
        rows,
        formats: if table.formats.is_empty() {
            Vec::new()
        } else {
            formats
        },
        caption: table.caption.clone(),
        outline: table.outline.clone(),
        ..Table::default()
    }
}

/// A variable name made from a column header, unique among `used` (lower case), which it
/// joins.
fn variable_name(label: &str, used: &mut HashSet<String>) -> String {
    let kept: String = label
        .chars()
        .filter(|c| c.is_alphanumeric() || "_.$#@".contains(*c))
        .collect();
    let mut name = kept.trim_end_matches(['_', '.']).to_string();
    if name.is_empty() {
        name = "Column".into();
    }
    if !name.starts_with(|c: char| c.is_alphabetic() || c == '@') {
        name.insert(0, '@');
    }
    name = statscraft_data::cut_to(&name, 60).to_string();
    if !statscraft_data::is_valid_name(&name) {
        // a reserved word
        name.insert(0, '@');
    }
    let mut unique = name.clone();
    for n in 0.. {
        if used.insert(unique.to_lowercase()) {
            break;
        }
        let suffix = if n < 26 {
            char::from(b'A' + n as u8).to_string()
        } else {
            (n + 1).to_string()
        };
        unique = format!("{name}_{suffix}");
    }
    unique
}

/// The decimals a number shows in `format`, such as 2 for F8.2.
fn decimals(format: &str) -> Option<usize> {
    format.rsplit_once('.').and_then(|(_, d)| d.parse().ok())
}

impl Destination {
    /// The data file of the tables so far.
    fn dataset(&self) -> Result<Dataset, Error> {
        let mut used: HashSet<String> = ["command_", "subtype_", "label_"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let numbered = self.numbered.as_ref().map(|n| {
            used.insert(n.to_lowercase());
            n.clone()
        });
        let row_labels = self.tables.first().map_or(0, |t| t.table.row_headers);
        let row_names: Vec<String> = (1..=row_labels)
            .map(|i| variable_name(&format!("Var{i}"), &mut used))
            .collect();
        // every column: its header, the cells of every case (None in cases of tables
        // without it)
        let cases: usize = self.tables.iter().map(|t| t.table.rows.len()).sum();
        // (name source, occurrence in its table, variable label)
        let mut headers: Vec<(String, usize, String)> = Vec::new();
        let mut values: Vec<Vec<Option<(Cell, String)>>> = Vec::new();
        let mut text = vec![Vec::new(); 3 + row_labels];
        let mut numbers = Vec::new();
        let mut case = 0;
        for (number, t) in self.tables.iter().enumerate() {
            let table = &t.table;
            let depth = 1 + table.column_groups.iter().map(Vec::len).max().unwrap_or(0);
            let mut seen: Vec<String> = Vec::new();
            let mut at = Vec::new();
            for c in table.row_headers..table.columns.len() {
                let mut path = column_path(table, c, depth);
                path.retain(|p| !p.is_empty());
                let header = path.join("_");
                let nth = seen.iter().filter(|s| **s == header).count();
                seen.push(header.clone());
                let i = match headers
                    .iter()
                    .position(|(h, n, _)| *h == header && *n == nth)
                {
                    Some(i) => i,
                    None => {
                        headers.push((header, nth, path.join(" / ")));
                        values.push(vec![None; cases]);
                        headers.len() - 1
                    }
                };
                at.push((c, i));
            }
            for r in 0..table.rows.len() {
                text[0].push(t.command.clone());
                text[1].push(t.subtype.clone());
                text[2].push(t.label.clone());
                for i in 0..row_labels {
                    text[3 + i].push(cell_text(table, r, i));
                }
                numbers.push(Some(number as f64 + 1.0));
                for &(c, i) in &at {
                    if c < table.rows[r].len() {
                        let format = table.formats.get(r).and_then(|f| f.get(c)).cloned();
                        values[i][case] =
                            Some((table.rows[r][c].clone(), format.unwrap_or_default()));
                    }
                }
                case += 1;
            }
        }
        let mut data = Dataset::new(cases);
        let width = |texts: &[String]| texts.iter().map(String::len).max().unwrap_or(0).max(1);
        let fixed = ["Command_", "Subtype_", "Label_"].map(String::from);
        for (name, texts) in fixed.iter().zip(&text) {
            data.set_string_values(name, width(texts), texts.clone())?;
        }
        if let Some(name) = &numbered {
            data.set_numeric_values(name, numbers)?;
            data.set_format(name, "F8.0")?;
        }
        for (name, texts) in row_names.iter().zip(&text[3..]) {
            data.set_string_values(name, width(texts), texts.clone())?;
        }
        for ((header, _, label), column) in headers.iter().zip(values) {
            let name = variable_name(header, &mut used);
            let numeric = column
                .iter()
                .flatten()
                .all(|(cell, _)| matches!(cell, Cell::Number(_) | Cell::Empty));
            if numeric {
                let shown = column
                    .iter()
                    .flatten()
                    .filter_map(|(cell, format)| match cell {
                        Cell::Number(n) => {
                            Some(decimals(format).unwrap_or(if n.fract() == 0.0 { 0 } else { 2 }))
                        }
                        _ => None,
                    });
                let d = shown.max().unwrap_or(0).min(16);
                let numbers = column.iter().map(|v| match v {
                    Some((Cell::Number(n), _)) if n.is_finite() => Some(*n),
                    _ => None,
                });
                data.set_numeric_values(&name, numbers.collect())?;
                data.set_format(&name, &format!("F{}.{d}", (8 + d).min(40)))?;
            } else {
                let texts: Vec<String> = column
                    .iter()
                    .map(|v| match v {
                        Some((Cell::Text(t), _)) => t.clone(),
                        Some((Cell::Number(n), format)) => {
                            let table = Table {
                                columns: vec![String::new()],
                                rows: vec![vec![Cell::Number(*n)]],
                                formats: vec![vec![format.clone()]],
                                ..Table::default()
                            };
                            cell_text(&table, 0, 0)
                        }
                        _ => String::new(),
                    })
                    .collect();
                data.set_string_values(&name, width(&texts), texts)?;
            }
            if !label.is_empty() && name != *label {
                data.set_variable_label(&name, Some(label))?;
            }
        }
        Ok(data)
    }

    fn export(&self) -> OutputExport {
        let (file, d) = (self.outfile.clone(), &self.spec.destination);
        let target = match self.format {
            "HTML" => ExportTarget::Html {
                file,
                notes: true,
                styling: true,
                image_format: d.image_format,
            },
            "TEXT" | "TABTEXT" => ExportTarget::Text {
                file,
                notes: true,
                encoding: TextEncoding::Utf8,
                column_width: None,
                tab: self.format == "TABTEXT",
                row_border: '-',
                column_border: '|',
                image_format: d.image_format,
            },
            "XLSX" => ExportTarget::Xlsx {
                file,
                operation: SheetOperation::CreateFile,
                sheet: None,
                location: SheetLocation::LastColumn,
                notes: true,
            },
            "DOC" => ExportTarget::Doc { file, notes: true },
            _ => ExportTarget::Pdf {
                file,
                bookmarks: true,
                embed_fonts: true,
            },
        };
        OutputExport {
            name: None,
            targets: vec![target],
            images: vec![ImageExport {
                format: d.image_format,
                root: None,
                percent: d.chart_size,
                tone: ImageTone::Color,
            }],
            visible_only: false,
        }
    }

    /// Whether the format takes charts.
    fn takes_charts(&self) -> bool {
        !matches!(self.format, "TEXT" | "TABTEXT")
            && !(self.format == "HTML" && !self.spec.destination.images)
    }
}

impl Engine {
    /// The key of an OUTFILE: a dataset DATASET DECLARE made, else the file's absolute path.
    fn destination_key(&self, outfile: &str) -> String {
        if self.is_dataset_name(outfile) {
            return format!("dataset {}", outfile.to_lowercase());
        }
        let path = self.path(outfile);
        std::path::absolute(&path)
            .unwrap_or(path)
            .display()
            .to_string()
    }

    pub(crate) fn oms(&mut self, spec: &Oms) -> Result<Vec<Table>, Error> {
        let state = &self.oms;
        if let Some(tag) = &spec.tag
            && state
                .requests
                .iter()
                .any(|r| r.tag.eq_ignore_ascii_case(tag))
        {
            return Err(Error(format!(
                "OMS: a request with TAG '{tag}' is in force already; end it with OMSEND or use another tag"
            )));
        }
        let d = &spec.destination;
        let mut destination = None;
        let mut warnings = Vec::new();
        let writes = d
            .format
            .filter(|f| WRITTEN.contains(f) && d.other_target.is_none());
        if let Some(target) = d.other_target {
            warnings.push(format!(
                "OMS: {target} is not supported; this request writes nothing and leaves the results as they are. Use OUTFILE"
            ));
        } else if let Some(format) = d.format.filter(|f| !WRITTEN.contains(f)) {
            warnings.push(format!(
                "OMS: Statscraft does not write FORMAT={format}; this request writes nothing and leaves the results as they are. It writes {}",
                WRITTEN.join(", ")
            ));
        }
        if let (Some(format), Some(outfile)) = (writes, &d.outfile) {
            let key = self.destination_key(outfile);
            match self.oms.destinations.iter().find(|x| x.key == key) {
                Some(x) if x.format != format => {
                    return Err(Error(format!(
                        "OMS: {outfile} is written as {} by another request; one file takes one format",
                        x.format
                    )));
                }
                Some(_) => {}
                None => self.oms.destinations.push(Destination {
                    key: key.clone(),
                    outfile: outfile.clone(),
                    format,
                    numbered: d.numbered.clone().filter(|_| format == "SAV"),
                    spec: Arc::new(spec.clone()),
                    tables: Vec::new(),
                    items: Vec::new(),
                    written: false,
                }),
            }
            destination = Some(key);
        }
        if !spec.nowarn {
            warnings.extend(
                d.ignored
                    .iter()
                    .map(|k| format!("OMS: {k} changes nothing in Statscraft")),
            );
            if d.numbered.is_some() && writes != Some("SAV") {
                warnings.push("OMS: NUMBERED applies to FORMAT=SAV only".into());
            }
        }
        let state = &mut self.oms;
        state.started += 1;
        let tag = spec
            .tag
            .clone()
            .unwrap_or_else(|| format!("$Oms{}", state.started));
        let file = destination.clone().unwrap_or_default();
        // a request that writes nothing hides nothing either
        let mut spec = spec.clone();
        if destination.is_none() && d.format.is_some() {
            spec.destination.viewer = true;
        }
        state.requests.push(Request {
            spec: Arc::new(spec),
            tag: tag.clone(),
            destination,
        });
        for warning in warnings {
            self.warn(warning);
        }
        self.log_event("start", &tag, &file)?;
        Ok(Vec::new())
    }

    pub(crate) fn omsend(&mut self, end: &Omsend) -> Result<Vec<Table>, Error> {
        let everything = !end.all_tags && end.tags.is_empty() && end.files.is_empty();
        let mut ending: Vec<usize> = Vec::new();
        if end.all_tags || (everything && !end.log) {
            ending.extend(0..self.oms.requests.len());
        }
        for tag in &end.tags {
            match self
                .oms
                .requests
                .iter()
                .position(|r| r.tag.eq_ignore_ascii_case(tag))
            {
                Some(i) => ending.push(i),
                None => self.warn(format!("OMSEND: no OMS request has the tag '{tag}'")),
            }
        }
        for file in &end.files {
            let key = self.destination_key(file);
            let found: Vec<usize> = (0..self.oms.requests.len())
                .filter(|&i| self.oms.requests[i].destination.as_deref() == Some(key.as_str()))
                .collect();
            if found.is_empty() {
                self.warn(format!("OMSEND: no OMS request writes {file}"));
            }
            ending.extend(found);
        }
        ending.sort_unstable();
        ending.dedup();
        // in start order, so the log lists the ends as the requests began
        for (removed, &i) in ending.iter().enumerate() {
            let request = self.oms.requests.remove(i - removed);
            let key = request.destination.clone().unwrap_or_default();
            self.log_event("end", &request.tag, &key)?;
            let used = self
                .oms
                .requests
                .iter()
                .any(|r| r.destination == request.destination);
            if let (Some(key), false) = (request.destination, used)
                && let Some(at) = self.oms.destinations.iter().position(|x| x.key == key)
            {
                let x = self.oms.destinations.remove(at);
                if !x.written && !x.spec.nowarn {
                    self.warn(format!(
                        "OMSEND: no output went to {}, so it was not written",
                        x.outfile
                    ));
                }
            }
        }
        if end.log || everything {
            self.oms.log = None;
        }
        Ok(Vec::new())
    }

    pub(crate) fn omsinfo(&self) -> Vec<Table> {
        let rows = self.oms.requests.iter().map(|r| {
            let d = &r.spec.destination;
            vec![
                Cell::Text(r.tag.clone()),
                Cell::Text(kind_names(&r.spec.kinds)),
                Cell::Text(d.format.unwrap_or_default().to_string()),
                Cell::Text(d.outfile.clone().unwrap_or_default()),
                Cell::Text(if d.viewer { "Yes" } else { "No" }.into()),
            ]
        });
        let rows: Vec<Vec<Cell>> = rows.collect();
        let mut notes = Vec::new();
        if rows.is_empty() {
            notes.push("No OMS request is in force.".to_string());
        }
        if let Some(log) = &self.oms.log {
            notes.push(format!("The OMS log goes to {}.", log.path.display()));
        }
        vec![Table {
            title: "OMS Requests".into(),
            row_headers: 1,
            columns: ["Tag", "Selects", "Format", "Destination", "Shown"]
                .map(String::from)
                .to_vec(),
            rows,
            caption: (!notes.is_empty()).then(|| notes.join(" ")),
            ..Table::default()
        }]
    }

    pub(crate) fn omslog(&mut self, log: &Omslog) -> Result<Vec<Table>, Error> {
        let path = self.path(&log.file);
        let path = std::path::absolute(&path).unwrap_or(path);
        let keep = log.text && log.append && path.exists();
        if !self.dry_run && !keep {
            let empty = if log.text {
                ""
            } else {
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<omsLog>\n</omsLog>\n"
            };
            std::fs::write(&path, empty)
                .map_err(|e| Error(format!("OMSLOG: cannot write {}: {e}", log.file)))?;
        }
        self.oms.log = Some(Log {
            path,
            text: log.text,
            elements: Vec::new(),
        });
        Ok(Vec::new())
    }

    fn log_event(&mut self, event: &str, tag: &str, file: &str) -> Result<(), Error> {
        let dry_run = self.dry_run;
        let Some(log) = self.oms.log.as_mut() else {
            return Ok(());
        };
        let time = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S");
        let shown_path = log.path.display().to_string();
        let result = if log.text {
            let line = format!("{time}\t{event}\t{tag}\t{file}\n");
            if dry_run {
                return Ok(());
            }
            std::fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(&log.path)
                .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()))
        } else {
            let attribute = |s: &str| crate::html::escape(s).replace('"', "&quot;");
            log.elements.push(format!(
                "  <event name=\"{event}\" tag=\"{}\" file=\"{}\" time=\"{time}\"/>\n",
                attribute(tag),
                attribute(file)
            ));
            if dry_run {
                return Ok(());
            }
            let text = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<omsLog>\n{}</omsLog>\n",
                log.elements.concat()
            );
            std::fs::write(&log.path, text)
        };
        result.map_err(|e| Error(format!("OMSLOG: cannot write {shown_path}: {e}")))
    }

    /// Sends the output of one command to the requests in force and takes out of `tables`, the
    /// charts and the warnings what a request keeps out of the results. With `whole` false
    /// only `tables` are looked at: the tables a run printed at its end.
    pub(crate) fn route_output(
        &mut self,
        command: &str,
        tables: &mut Vec<Table>,
        whole: bool,
    ) -> Result<(), Error> {
        if self.oms.requests.is_empty() {
            return Ok(());
        }
        let texts = ["ECHO", "TITLE", "SUBTITLE"].contains(&command);
        let mut items: Vec<Item> = tables
            .iter()
            .enumerate()
            .map(|(i, t)| Item {
                kind: if texts {
                    OmsKind::Texts
                } else {
                    OmsKind::Tables
                },
                label: t.outline.clone().unwrap_or_else(|| t.title.clone()),
                subtype: Some(subtype(command, t)),
                what: What::Table(i),
            })
            .collect();
        if whole {
            items.extend(self.charts.iter().enumerate().map(|(i, c)| Item {
                kind: OmsKind::Charts,
                label: c.title.clone(),
                subtype: None,
                what: What::Chart(i),
            }));
            if !self.warnings.is_empty() {
                items.push(Item {
                    kind: OmsKind::Warnings,
                    label: "Warnings".into(),
                    subtype: None,
                    what: What::Warnings,
                });
            }
        }
        let mut hidden = vec![false; items.len()];
        let mut sent: Vec<(String, usize, bool)> = Vec::new();
        for request in &self.oms.requests {
            for i in chosen(&request.spec, command, &items) {
                hidden[i] = !request.spec.destination.viewer;
                if let Some(key) = &request.destination
                    && !sent.iter().any(|(k, j, _)| k == key && *j == i)
                {
                    sent.push((key.clone(), i, request.spec.nowarn));
                }
            }
        }
        let warnings_table = Table {
            title: "Warnings".into(),
            columns: vec!["Text".into()],
            rows: self
                .warnings
                .iter()
                .map(|w| vec![Cell::Text(w.clone())])
                .collect(),
            ..Table::default()
        };
        let mut touched: Vec<String> = Vec::new();
        let mut notes = Vec::new();
        for (key, i, nowarn) in sent {
            let x = self
                .oms
                .destinations
                .iter_mut()
                .find(|x| x.key == key)
                .expect("a request's destination is kept while it lasts");
            let item = &items[i];
            let table = match item.what {
                What::Table(t) => Some(&tables[t]),
                What::Warnings => Some(&warnings_table),
                What::Chart(_) => None,
            };
            if x.format == "SAV" {
                let Some(table) = table.filter(|_| item.kind == OmsKind::Tables) else {
                    continue;
                };
                let table = match &x.spec.columns {
                    Some(columns) => pivot(table, columns),
                    None => table.clone(),
                };
                let first = x.tables.first().map(|t| t.table.row_headers);
                if first.is_some_and(|n| n != table.row_headers) {
                    if !nowarn {
                        notes.push(format!(
                            "OMS: the table {} has {} row label columns and the earlier tables of {} have {}; it was left out of that file",
                            table.title,
                            table.row_headers,
                            x.outfile,
                            first.unwrap_or(0)
                        ));
                    }
                    continue;
                }
                x.tables.push(SavTable {
                    command: command.to_string(),
                    subtype: item.subtype.clone().unwrap_or_default(),
                    label: item.label.clone(),
                    table,
                });
            } else {
                let shown: Shown = match item.what {
                    What::Chart(c) if x.takes_charts() => {
                        let chart = &self.charts[c];
                        (Arc::new(chart.data_table()), Some(Arc::new(chart.clone())))
                    }
                    What::Chart(_) => continue,
                    _ => {
                        let table = table.expect("not a chart");
                        let table = match &x.spec.columns {
                            Some(columns) if item.kind == OmsKind::Tables => pivot(table, columns),
                            _ => table.clone(),
                        };
                        (Arc::new(table), None)
                    }
                };
                x.items.push((command.to_string(), shown));
            }
            if !touched.contains(&key) {
                touched.push(key);
            }
        }
        for key in touched {
            self.write_destination(&key)?;
        }
        // what a request keeps out of the results goes, the latest request deciding
        let keep = |what: fn(&What) -> Option<usize>, count: usize| -> Vec<bool> {
            let mut kept = vec![true; count];
            for (item, hide) in items.iter().zip(&hidden) {
                if let Some(i) = what(&item.what) {
                    kept[i] = !hide;
                }
            }
            kept
        };
        let tables_kept = keep(
            |w| {
                if let What::Table(i) = w {
                    Some(*i)
                } else {
                    None
                }
            },
            tables.len(),
        );
        let charts_kept = keep(
            |w| {
                if let What::Chart(i) = w {
                    Some(*i)
                } else {
                    None
                }
            },
            self.charts.len(),
        );
        let mut kept = tables_kept.into_iter();
        tables.retain(|_| kept.next().unwrap_or(true));
        let mut kept = charts_kept.into_iter();
        self.charts.retain(|_| kept.next().unwrap_or(true));
        let warnings_hidden = items
            .iter()
            .zip(&hidden)
            .any(|(item, hide)| matches!(item.what, What::Warnings) && *hide);
        if warnings_hidden {
            self.warnings.clear();
        }
        for note in notes {
            self.warn(note);
        }
        Ok(())
    }

    fn write_destination(&mut self, key: &str) -> Result<(), Error> {
        let at = self.oms.destinations.iter().position(|x| x.key == key);
        let x = self.oms.destinations[at.expect("touched destinations exist")].clone();
        let result = if x.format == "SAV" {
            if x.tables.is_empty() {
                return Ok(());
            }
            let data = x.dataset()?;
            self.write_data_file(&x.outfile, data)
        } else {
            self.export_items(&x.export(), x.items.clone())
        };
        result.map_err(|e| Error(format!("OMS: {}", e.0.replace("OUTPUT EXPORT: ", ""))))?;
        self.oms.destinations[at.expect("checked")].written = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        // two row elements (group, variable) and a column level over the statistics
        let text = |s: &str| Cell::Text(s.into());
        Table {
            title: "T".into(),
            row_headers: 2,
            columns: ["Group", "Variable", "N", "Mean"]
                .map(String::from)
                .to_vec(),
            column_groups: vec![vec![], vec![], vec!["Stats".into()], vec!["Stats".into()]],
            rows: vec![
                vec![text("a"), text("x"), Cell::Number(3.0), Cell::Number(1.5)],
                vec![text("a"), text("y"), Cell::Number(4.0), Cell::Number(2.5)],
            ],
            ..Table::default()
        }
    }

    #[test]
    fn names_follow_the_header_rules() {
        let mut used = HashSet::new();
        assert_eq!(
            variable_name("This (Column) Label", &mut used),
            "ThisColumnLabel"
        );
        assert_eq!(variable_name("2nd", &mut used), "@2nd");
        assert_eq!(variable_name("Count", &mut used), "Count");
        assert_eq!(variable_name("Count", &mut used), "Count_A");
        assert_eq!(variable_name("Count", &mut used), "Count_B");
        assert_eq!(variable_name("Sig._", &mut used), "Sig");
        assert_eq!(variable_name("ALL", &mut used), "@ALL");
    }

    #[test]
    fn a_pivot_moves_elements_between_rows_and_columns() {
        let t = table();
        // the variables to the columns: the group and both column levels go to the rows
        let p = pivot(&t, &OmsColumns::Dimnames(vec!["variable".into()]));
        assert_eq!(p.row_headers, 3);
        assert_eq!(p.columns, ["Group", "", "", "x", "y"]);
        assert_eq!(p.rows.len(), 2);
        assert_eq!(
            p.rows[1][..3],
            [
                Cell::Text("a".into()),
                Cell::Text("Stats".into()),
                Cell::Text("Mean".into())
            ]
        );
        assert_eq!(p.rows[1][3..], [Cell::Number(1.5), Cell::Number(2.5)]);
        // everything in the columns: one row
        let all = OmsColumns::Sequence(vec![('C', None), ('R', None)]);
        let p = pivot(&t, &all);
        assert_eq!((p.row_headers, p.rows.len(), p.columns.len()), (0, 1, 4));
        assert_eq!(p.column_groups[3], ["Stats", "Mean", "a"]);
        assert_eq!(p.columns[3], "y");
        // an unknown name leaves the table as it is
        let p = pivot(&t, &OmsColumns::Dimnames(vec!["Nothing".into()]));
        assert_eq!(p.columns, t.columns);
    }

    #[test]
    fn wildcards_and_aliases() {
        assert!(matches("Desc*", "Descriptive Statistics"));
        assert!(matches("first variable **", "First Variable * Second"));
        assert!(!matches("Desc", "Descriptive Statistics"));
        assert!(names_command("factor analysis", "FACTOR"));
        assert!(names_command("Explo*", "EXAMINE"));
        assert!(names_command("frequencies", "FREQUENCIES"));
    }
}
