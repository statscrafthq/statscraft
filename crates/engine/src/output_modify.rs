//! OUTPUT MODIFY: deletes, hides, relabels, sorts, transposes and reformats the items of an
//! output document.
//!
//! Rules, from the description of the command:
//! - It works on the items the document holds when it runs. Its blocks run in order, each on
//!   what the blocks before it left, so their effects add up.
//! - /SELECT picks item kinds; /IF narrows them, and an item must meet every condition given.
//!   COMMANDS names commands, the nth or LAST run of one, or LAST, the last command of the
//!   document. LABELS compare the outline label, with case. INSTANCES count the items that
//!   met the other conditions within one command. VISIBLEONLY=YES skips hidden items.
//! - /INDEXING numbers the picked items in order; )INDEX shows the number in OUTLINELABEL,
//!   TABLETITLE and TEXT, next to )LABEL (the text as it was) and the date and time fields.
//! - SORT orders the rows by one column and leaves a table with nested row labels as it is.
//! - /TABLECELLS picks rows or columns by label or position, or the body, headers, title or
//!   caption, keeps the cells that meet SELECTCONDITION, and changes what APPLYTO names.
//!
//! Choices where the manual is silent:
//! - Statscraft documents hold commands (a heading over the command's items), tables, charts
//!   and items of unknown kind. HEADINGS and OUTLINEHEADERS pick the command headings,
//!   PAGETITLE the tables of TITLE and SUBTITLE, TABLES every other table, CHARTS the charts,
//!   UNKNOWN the unknown items. The other kinds have no items in a Statscraft document.
//! - A command identifier is the command name (FREQUENCIES, T-TEST), in any case; the
//!   identifiers Factor Analysis, Cases to Variables and Variables to Cases name FACTOR,
//!   CASESTOVARS and VARSTOCASES. A subtype is a table title, in any case.
//! - Deleting a heading deletes the command's items with it; VISIBLE on a heading hides or
//!   shows all of them. OUTLINELABEL and TEXT on a heading change the heading.
//! - Without /INDEXING, )INDEX counts 1, 2, 3, ...
//! - TABLECELLS works on the table as the block found it, then SORT, then TRANSPOSE.
//! - Labels in TABLECELLS SELECT match a row label or any level of a column header, in any
//!   case.
//!   Comparisons apply to numbers; a text cell compares as a number when it reads as one, and
//!   with a quoted value as text. SYSMIS is an empty cell, a number that is not finite or ".".
//!   Several comparisons must all hold; TOP and BOTTOM then pick among what is left, ties
//!   included.
//! - HIDE takes out the rows or columns that SELECT picked by label or position and that hold
//!   a cell meeting the conditions.
//! - SORT puts empty cells last in either direction and leaves rows labelled Total in place.
//!   Rows count as nested when an outer row label repeats from one row to the next.
//! - What changes nothing (fonts, colors, sizes, looks) gives a warning naming the keyword.

use crate::output_file::{self, CommandItem, Item};
use crate::{Cell, Chart, Engine, Error, Table, cell_text};
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use statscraft_syntax::{
    ApplyTo, CellArea, CellCondition, CommandPick, CompareOp, CompareValue, Dimension, IndexStyle,
    Instance, ItemKind, LabelTest, ModifyBlock, ModifyTarget, OutputModify, SortColumn, TableCells,
    Value, letters, roman,
};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

/// What one OUTPUT MODIFY changed in one document, so the window can follow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Modified {
    pub document: String,
    /// The tables, charts and other items that changed or went.
    pub items: Vec<ModifiedItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifiedItem {
    /// Its place among the document's items before the command, counted as
    /// `output_file::leaves` counts them, from 0.
    pub position: usize,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub deleted: bool,
    /// The new heading of its command, when that changed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    /// The item as it is now: a table, or a chart with its data table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub table: Option<Table>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chart: Option<Chart>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Heading,
    Table,
    PageTitle,
    Chart,
    Unknown,
}

/// One item OUTPUT MODIFY can pick, in document order.
struct Object {
    kind: Kind,
    /// The command that holds it, numbered depth first; a heading's own command.
    command: Option<usize>,
    command_name: String,
    /// The text in the outline.
    label: String,
    title: String,
    hidden: bool,
    /// Its place among the leaves; None for a heading.
    leaf: Option<usize>,
}

fn objects(items: &[Item]) -> Vec<Object> {
    fn walk(
        items: &[Item],
        parent: Option<(usize, &str)>,
        next: &mut (usize, usize),
        out: &mut Vec<Object>,
    ) {
        for item in items {
            let (command, command_name) = match parent {
                Some((seq, name)) => (Some(seq), name.to_string()),
                None => (None, String::new()),
            };
            let leaf = Some(next.1);
            let object = match item {
                Item::Command(c) => {
                    next.0 += 1;
                    let seq = next.0 - 1;
                    out.push(Object {
                        kind: Kind::Heading,
                        command: Some(seq),
                        command_name: c.command.clone(),
                        label: c.heading().to_string(),
                        title: c.heading().to_string(),
                        hidden: false,
                        leaf: None,
                    });
                    walk(&c.items, Some((seq, &c.command)), next, out);
                    continue;
                }
                Item::Table(t) => {
                    let page = ["TITLE", "SUBTITLE"].contains(&command_name.as_str());
                    Object {
                        kind: if page { Kind::PageTitle } else { Kind::Table },
                        command,
                        command_name,
                        label: t.outline.clone().unwrap_or_else(|| t.title.clone()),
                        title: t.title.clone(),
                        hidden: t.hidden,
                        leaf,
                    }
                }
                Item::Chart(c) => Object {
                    kind: Kind::Chart,
                    command,
                    command_name,
                    label: c.outline.clone().unwrap_or_else(|| c.title.clone()),
                    title: c.title.clone(),
                    hidden: c.hidden,
                    leaf,
                },
                Item::Unknown(_) => {
                    let title = item.shown().0.title;
                    Object {
                        kind: Kind::Unknown,
                        command,
                        command_name,
                        label: title.clone(),
                        title,
                        hidden: false,
                        leaf,
                    }
                }
            };
            next.1 += 1;
            out.push(object);
        }
    }
    let mut out = Vec::new();
    walk(items, None, &mut (0, 0), &mut out);
    out
}

/// One item to change, as `walk_mut` hands it over.
enum Target<'a> {
    Heading(&'a mut CommandItem),
    Leaf(&'a mut Item),
}

/// Calls `f` with every object, numbered as `objects` numbers them.
fn walk_mut(items: &mut [Item], f: &mut impl FnMut(usize, Target)) {
    fn walk(items: &mut [Item], next: &mut usize, f: &mut impl FnMut(usize, Target)) {
        for item in items {
            *next += 1;
            match item {
                Item::Command(c) => {
                    f(*next - 1, Target::Heading(c));
                    walk(&mut c.items, next, f);
                }
                leaf => f(*next - 1, Target::Leaf(leaf)),
            }
        }
    }
    walk(items, &mut 0, f);
}

fn kind_of(kind: ItemKind) -> Option<Kind> {
    match kind {
        ItemKind::Tables => Some(Kind::Table),
        ItemKind::Charts => Some(Kind::Chart),
        ItemKind::PageTitle => Some(Kind::PageTitle),
        ItemKind::Unknown => Some(Kind::Unknown),
        ItemKind::Headings | ItemKind::OutlineHeaders => Some(Kind::Heading),
        _ => None,
    }
}

/// True when the identifier of COMMANDS names the command.
fn names_command(identifier: &str, command: &str) -> bool {
    const ALIASES: [(&str, &str); 3] = [
        ("Factor Analysis", "FACTOR"),
        ("Cases to Variables", "CASESTOVARS"),
        ("Variables to Cases", "VARSTOCASES"),
    ];
    let identifier = identifier.trim();
    identifier.eq_ignore_ascii_case(command)
        || ALIASES
            .iter()
            .any(|(a, c)| a.eq_ignore_ascii_case(identifier) && *c == command)
}

/// The objects a block picks, in document order.
fn pick(block: &ModifyBlock, all: &[Object], gone: &dyn Fn(&Object) -> bool) -> Vec<usize> {
    let kinds: Vec<Kind> = block.kinds.iter().filter_map(|k| kind_of(*k)).collect();
    // the commands still there, in order, with their names
    let commands: Vec<(usize, &str)> = all
        .iter()
        .filter(|o| o.kind == Kind::Heading && !gone(o))
        .map(|o| {
            (
                o.command.expect("a heading has its command"),
                o.command_name.as_str(),
            )
        })
        .collect();
    let command_ok = |seq: usize, name: &str| {
        block.commands.iter().any(|pick| match pick {
            CommandPick::Last => commands.last().is_some_and(|(last, _)| *last == seq),
            CommandPick::Named {
                name: wanted,
                instance,
            } => {
                if !names_command(wanted, name) {
                    return false;
                }
                let runs = commands.iter().filter(|(_, n)| names_command(wanted, n));
                match instance {
                    None => true,
                    Some(Instance::Last) => {
                        runs.clone().next_back().is_some_and(|(s, _)| *s == seq)
                    }
                    Some(Instance::Nth(n)) => {
                        runs.clone().nth(n - 1).is_some_and(|(s, _)| *s == seq)
                    }
                }
            }
        })
    };
    let label_ok = |label: &str| {
        block.labels.iter().any(|(test, text)| match test {
            LabelTest::Exact => label == text,
            LabelTest::Begins => label.starts_with(text.as_str()),
            LabelTest::Ends => label.ends_with(text.as_str()),
            LabelTest::Contains => label.contains(text.as_str()),
        })
    };
    let met: Vec<usize> = (0..all.len())
        .filter(|&i| {
            let o = &all[i];
            !gone(o)
                && kinds.contains(&o.kind)
                && (block.commands.is_empty()
                    || o.command
                        .is_some_and(|seq| command_ok(seq, &o.command_name)))
                && (block.subtypes.is_empty()
                    || matches!(o.kind, Kind::Table | Kind::PageTitle)
                        && block
                            .subtypes
                            .iter()
                            .any(|s| s.trim().eq_ignore_ascii_case(&o.title)))
                && (block.labels.is_empty() || label_ok(&o.label))
                && !(block.visible_only && o.hidden)
        })
        .collect();
    if block.instances.is_empty() {
        return met;
    }
    // the nth of the items that met the rest, within each command
    let mut groups: HashMap<Option<usize>, Vec<usize>> = HashMap::new();
    for &i in &met {
        groups.entry(all[i].command).or_default().push(i);
    }
    let mut kept = BTreeSet::new();
    for members in groups.values() {
        for instance in &block.instances {
            let found = match instance {
                Instance::Nth(n) => members.get(n - 1),
                Instance::Last => members.last(),
            };
            kept.extend(found);
        }
    }
    kept.into_iter().collect()
}

/// An index value in the style of INDEXING.
fn index_text(style: IndexStyle, n: usize) -> String {
    match style {
        IndexStyle::Number => n.to_string(),
        IndexStyle::LetterLow => letters(n),
        IndexStyle::LetterCap => letters(n).to_uppercase(),
        IndexStyle::RomanLow => roman(n).to_lowercase(),
        IndexStyle::RomanCap => roman(n),
    }
}

/// A label template with its fields filled in: )LABEL, )INDEX and the date and time fields.
fn expand(template: &str, label: &str, index: &str, now: &DateTime<Local>) -> String {
    const FIELDS: [&str; 8] = [
        "LABEL", "INDEX", "ADATE", "SDATE", "EDATE", "ETIME", "DATE", "TIME",
    ];
    let mut out = String::new();
    let mut rest = template;
    while let Some(at) = rest.find(')') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let Some(field) = FIELDS.iter().find(|f| after.starts_with(*f)) else {
            out.push(')');
            rest = after;
            continue;
        };
        let value = match *field {
            "LABEL" => label.to_string(),
            "INDEX" => index.to_string(),
            "DATE" => now.format("%d-%b-%Y").to_string(),
            "ADATE" => now.format("%m/%d/%Y").to_string(),
            "SDATE" => now.format("%Y/%m/%d").to_string(),
            "EDATE" => now.format("%d.%m.%Y").to_string(),
            "TIME" => now.format("%I:%M:%S").to_string(),
            _ => now.format("%H:%M:%S").to_string(),
        };
        out.push_str(&value);
        rest = &after[field.len()..];
    }
    out.push_str(rest);
    out
}

/// The text of a cell or a label for the conditions: a number when it is one.
enum Seen<'a> {
    Number(f64),
    Text(&'a str),
    Missing,
}

fn seen(cell: &Cell) -> Seen<'_> {
    match cell {
        Cell::Number(n) if n.is_finite() => Seen::Number(*n),
        Cell::Number(_) | Cell::Empty => Seen::Missing,
        Cell::Text(t) if t.trim() == "." || t.trim().is_empty() => Seen::Missing,
        Cell::Text(t) => match t.trim().parse::<f64>() {
            Ok(n) if n.is_finite() => Seen::Number(n),
            _ => Seen::Text(t),
        },
    }
}

fn holds(op: CompareOp, order: std::cmp::Ordering) -> bool {
    use std::cmp::Ordering::*;
    match op {
        CompareOp::Lt => order == Less,
        CompareOp::Le => order != Greater,
        CompareOp::Gt => order == Greater,
        CompareOp::Ge => order != Less,
        CompareOp::Eq => order == Equal,
        CompareOp::Ne => order != Equal,
    }
}

fn compare(value: &Seen, abs: bool, op: CompareOp, against: &CompareValue) -> bool {
    match (value, against) {
        (Seen::Missing, CompareValue::Sysmis) => op == CompareOp::Eq,
        (_, CompareValue::Sysmis) => op == CompareOp::Ne,
        (Seen::Number(n), CompareValue::Number(m)) => {
            let n = if abs { n.abs() } else { *n };
            n.partial_cmp(m).is_some_and(|o| holds(op, o))
        }
        (Seen::Number(n), CompareValue::Text(t)) => {
            holds(op, n.to_string().as_str().cmp(t.as_str()))
        }
        (Seen::Text(s), CompareValue::Text(t)) => holds(op, s.cmp(&t.as_str())),
        // a text against a number, or a missing value against anything but SYSMIS
        _ => op == CompareOp::Ne,
    }
}

/// The places TABLECELLS changes.
#[derive(Default)]
struct Places {
    cells: BTreeSet<(usize, usize)>,
    /// Columns whose innermost header label changes.
    headers: BTreeSet<usize>,
    title: bool,
    caption: bool,
    /// Rows and columns HIDE takes out.
    hide_rows: BTreeSet<usize>,
    hide_columns: BTreeSet<usize>,
}

/// Where a TABLECELLS subcommand applies in a table. `warn` gets what it cannot do.
fn places(table: &Table, spec: &TableCells, warn: &mut dyn FnMut(String)) -> Places {
    let rh = table.row_headers.min(table.columns.len());
    let width = table.columns.len();
    let data = |r: usize| (rh..width.min(table.rows[r].len())).map(move |c| (r, c));
    let mut out = Places::default();
    // rows and columns picked by label or position, and the cells they cover
    let mut rows = BTreeSet::new();
    let mut columns = BTreeSet::new();
    let mut area = BTreeSet::new();
    let mut texts: Vec<(Place, String)> = Vec::new();
    let by_rows = spec.dimension != Dimension::Columns;
    let by_columns = spec.dimension != Dimension::Rows;
    for item in &spec.select {
        match item {
            CellArea::Labels(labels) => {
                let named = |text: &str| labels.iter().any(|l| l.eq_ignore_ascii_case(text.trim()));
                if by_columns {
                    for c in rh..width {
                        let groups = table.column_groups.get(c).map_or(&[][..], Vec::as_slice);
                        if named(&table.columns[c]) || groups.iter().any(|g| named(g)) {
                            columns.insert(c);
                        }
                    }
                }
                if by_rows {
                    for (r, row) in table.rows.iter().enumerate() {
                        let labels = row.iter().take(rh);
                        if labels
                            .into_iter()
                            .any(|cell| matches!(cell, Cell::Text(t) if named(t)))
                        {
                            rows.insert(r);
                        }
                    }
                }
            }
            CellArea::Position(n) => {
                if by_columns && rh + n - 1 < width {
                    columns.insert(rh + n - 1);
                }
                if by_rows && n - 1 < table.rows.len() {
                    rows.insert(n - 1);
                }
            }
            CellArea::Body => area.extend((0..table.rows.len()).flat_map(data)),
            CellArea::Headers => {
                for r in 0..table.rows.len() {
                    for c in 0..rh.min(table.rows[r].len()) {
                        texts.push((Place::Cell(r, c), cell_text(table, r, c)));
                    }
                }
                for c in 0..width {
                    texts.push((Place::Header(c), table.columns[c].clone()));
                }
            }
            CellArea::Title => texts.push((Place::Title, table.title.clone())),
            CellArea::Footnotes | CellArea::Caption => {
                if let Some(caption) = &table.caption {
                    texts.push((Place::Caption, caption.clone()));
                }
            }
        }
    }
    for &r in &rows {
        area.extend(data(r));
    }
    for &c in &columns {
        area.extend(
            (0..table.rows.len())
                .filter(|&r| c < table.rows[r].len())
                .map(|r| (r, c)),
        );
    }
    let compares: Vec<_> = spec
        .conditions
        .iter()
        .filter_map(|c| match c {
            CellCondition::Compare { abs, op, value } => Some((*abs, *op, value)),
            _ => None,
        })
        .collect();
    let passes = |value: &Seen| {
        compares
            .iter()
            .all(|(abs, op, v)| compare(value, *abs, *op, v))
    };
    let mut matched: Vec<(usize, usize)> = area
        .into_iter()
        .filter(|&(r, c)| passes(&seen(&table.rows[r][c])))
        .collect();
    let ranks: Vec<(bool, usize)> = spec
        .conditions
        .iter()
        .filter_map(|c| match c {
            CellCondition::Top(n) => Some((true, *n)),
            CellCondition::Bottom(n) => Some((false, *n)),
            _ => None,
        })
        .collect();
    if !ranks.is_empty() {
        let number = |&(r, c): &(usize, usize)| match seen(&table.rows[r][c]) {
            Seen::Number(n) => Some(n),
            _ => None,
        };
        let mut values: Vec<f64> = matched.iter().filter_map(number).collect();
        values.sort_by(f64::total_cmp);
        matched.retain(|cell| {
            number(cell).is_some_and(|v| {
                ranks.iter().any(|&(top, n)| {
                    let n = n.min(values.len());
                    match top {
                        true => n > 0 && v >= values[values.len() - n],
                        false => n > 0 && v <= values[n - 1],
                    }
                })
            })
        });
    }
    for (place, text) in texts {
        let value = match text.trim().parse::<f64>() {
            Ok(n) => Seen::Number(n),
            Err(_) if text.trim().is_empty() => Seen::Missing,
            Err(_) => Seen::Text(&text),
        };
        if passes(&value) {
            match place {
                Place::Cell(r, c) => {
                    out.cells.insert((r, c));
                }
                Place::Header(c) => {
                    out.headers.insert(c);
                }
                Place::Title => out.title = true,
                Place::Caption => out.caption = true,
            }
        }
    }
    let hit_rows: BTreeSet<usize> = matched.iter().map(|(r, _)| *r).collect();
    let hit_columns: BTreeSet<usize> = matched.iter().map(|(_, c)| *c).collect();
    if spec.hide {
        out.hide_rows = rows.intersection(&hit_rows).copied().collect();
        out.hide_columns = columns.intersection(&hit_columns).copied().collect();
        let by_lines = spec
            .select
            .iter()
            .any(|a| matches!(a, CellArea::Labels(_) | CellArea::Position(_)));
        if !by_lines {
            warn(format!(
                "{}: HIDE takes rows or columns picked by label or position; nothing was hidden",
                table.title
            ));
        }
    }
    let all_row = |r: usize| (0..table.rows[r].len()).map(move |c| (r, c));
    match spec.apply_to {
        ApplyTo::Cell => out.cells.extend(matched),
        ApplyTo::Row => out.cells.extend(hit_rows.iter().flat_map(|&r| all_row(r))),
        ApplyTo::RowData => out.cells.extend(hit_rows.iter().flat_map(|&r| data(r))),
        ApplyTo::RowHeader => {
            let inner = |&r: &usize| (rh > 0 && rh <= table.rows[r].len()).then_some((r, rh - 1));
            out.cells.extend(hit_rows.iter().filter_map(inner));
        }
        ApplyTo::Column | ApplyTo::ColumnData | ApplyTo::ColumnHeader => {
            if spec.apply_to != ApplyTo::ColumnHeader {
                for &c in &hit_columns {
                    let rows = (0..table.rows.len()).filter(|&r| c < table.rows[r].len());
                    out.cells.extend(rows.map(|r| (r, c)));
                }
            }
            if spec.apply_to != ApplyTo::ColumnData {
                out.headers.extend(&hit_columns);
            }
        }
    }
    out
}

enum Place {
    Cell(usize, usize),
    Header(usize),
    Title,
    Caption,
}

fn replacement(value: &Value) -> (Cell, String) {
    match value {
        Value::Number(n) => (Cell::Number(*n), n.to_string()),
        Value::String(s) => (Cell::Text(s.clone()), s.clone()),
    }
}

/// Applies one TABLECELLS subcommand.
fn table_cells(table: &mut Table, spec: &TableCells, warn: &mut dyn FnMut(String)) {
    let places = places(table, spec, warn);
    if let Some(format) = &spec.format {
        if table.formats.is_empty() {
            table.formats = table
                .rows
                .iter()
                .map(|r| vec![String::new(); r.len()])
                .collect();
        }
        for &(r, c) in &places.cells {
            let row = &mut table.formats[r];
            if row.len() < table.rows[r].len() {
                row.resize(table.rows[r].len(), String::new());
            }
            if matches!(table.rows[r][c], Cell::Number(_)) && c >= table.row_headers {
                row[c] = format.clone();
            }
        }
    }
    if let Some(value) = &spec.replace {
        let (cell, text) = replacement(value);
        for &(r, c) in &places.cells {
            table.rows[r][c] = cell.clone();
            if let Some(f) = table.formats.get_mut(r).and_then(|row| row.get_mut(c)) {
                f.clear();
            }
        }
        for &c in &places.headers {
            table.columns[c] = text.clone();
        }
        if places.title {
            table.title = text.clone();
        }
        if places.caption {
            table.caption = Some(text);
        }
    }
    for &r in places.hide_rows.iter().rev() {
        table.rows.remove(r);
        if r < table.formats.len() {
            table.formats.remove(r);
        }
    }
    for &c in places.hide_columns.iter().rev() {
        table.columns.remove(c);
        if c < table.column_groups.len() {
            table.column_groups.remove(c);
        }
        for row in &mut table.rows {
            if c < row.len() {
                row.remove(c);
            }
        }
        for row in &mut table.formats {
            if c < row.len() {
                row.remove(c);
            }
        }
    }
}

/// SORT: the rows in the order of one column. Err says why the table stays as it is.
fn sort_rows(table: &mut Table, by: &SortColumn, descending: bool) -> Result<(), String> {
    // nested: an outer row label spans rows, so it repeats from one row to the next
    let rh = table.row_headers.min(table.columns.len());
    let label = |r: usize, c: usize| table.rows[r].get(c).map(|_| cell_text(table, r, c));
    let nested = (0..rh.saturating_sub(1))
        .any(|c| (1..table.rows.len()).any(|r| label(r, c) == label(r - 1, c)));
    if nested {
        return Err("its rows have nested labels".into());
    }
    let column = match by {
        SortColumn::Position(n) if *n <= table.columns.len() => n - 1,
        SortColumn::Position(n) => return Err(format!("it has no column {n}")),
        SortColumn::Label(label) => {
            let titles = table.column_titles();
            let found = (0..table.columns.len())
                .find(|&c| table.columns[c] == *label || titles[c] == *label);
            found.ok_or_else(|| format!("it has no column labelled {label}"))?
        }
    };
    let total = |r: usize| {
        let labels = table.rows[r].iter().take(rh.max(1));
        labels
            .into_iter()
            .any(|c| matches!(c, Cell::Text(t) if t.trim().eq_ignore_ascii_case("Total")))
    };
    let moving: Vec<usize> = (0..table.rows.len()).filter(|&r| !total(r)).collect();
    let rank = |r: usize| match table.rows[r].get(column).map(seen) {
        Some(Seen::Number(n)) => (0, n, String::new()),
        Some(Seen::Text(t)) => (1, 0.0, t.to_string()),
        _ => (2, 0.0, String::new()),
    };
    let mut order = moving.clone();
    order.sort_by(|&a, &b| {
        let (ka, kb) = (rank(a), rank(b));
        let within = ka.1.total_cmp(&kb.1).then_with(|| ka.2.cmp(&kb.2));
        let within = if descending { within.reverse() } else { within };
        ka.0.cmp(&kb.0).then(within)
    });
    let rows = table.rows.clone();
    let formats = table.formats.clone();
    for (slot, from) in moving.into_iter().zip(order) {
        table.rows[slot] = rows[from].clone();
        if let (Some(to), Some(f)) = (table.formats.get_mut(slot), formats.get(from)) {
            *to = f.clone();
        }
    }
    Ok(())
}

/// TRANSPOSE: the columns become the rows. The header levels of a column become the row
/// labels of its row; the row labels of a row become the header levels of its column.
fn transpose(table: &Table) -> Table {
    let rh = table.row_headers.min(table.columns.len());
    let depth = 1 + table.column_groups.iter().map(Vec::len).max().unwrap_or(0);
    let path = |c: usize| {
        let mut levels = table.column_groups.get(c).cloned().unwrap_or_default();
        levels.resize(depth - 1, String::new());
        levels.push(table.columns[c].clone());
        levels
    };
    let label = |r: usize, c: usize| match table.rows[r].get(c) {
        Some(_) => cell_text(table, r, c),
        None => String::new(),
    };
    let mut columns = vec![String::new(); depth];
    let mut groups = vec![Vec::new(); depth];
    for r in 0..table.rows.len() {
        match rh {
            0 => {
                columns.push((r + 1).to_string());
                groups.push(Vec::new());
            }
            _ => {
                columns.push(label(r, rh - 1));
                groups.push((0..rh - 1).map(|c| label(r, c)).collect());
            }
        }
    }
    let has_formats = !table.formats.is_empty();
    let mut rows = Vec::new();
    let mut formats = Vec::new();
    for c in rh..table.columns.len() {
        let mut row: Vec<Cell> = path(c).into_iter().map(Cell::Text).collect();
        let mut format = vec![String::new(); depth];
        for r in 0..table.rows.len() {
            row.push(table.rows[r].get(c).cloned().unwrap_or_default());
            let f = table.formats.get(r).and_then(|f| f.get(c)).cloned();
            format.push(f.unwrap_or_default());
        }
        rows.push(row);
        formats.push(format);
    }
    Table {
        title: table.title.clone(),
        row_headers: depth,
        columns,
        column_groups: groups,
        rows,
        formats: if has_formats { formats } else { Vec::new() },
        caption: table.caption.clone(),
        outline: table.outline.clone(),
        hidden: table.hidden,
    }
}

/// What one picked item gets from a block.
struct Change<'a> {
    block: &'a ModifyBlock,
    index: String,
}

impl Engine {
    pub(crate) fn output_modify(&mut self, modify: &OutputModify) -> Result<Vec<Table>, Error> {
        let at: Vec<usize> = match &modify.target {
            ModifyTarget::All => (0..self.documents.open.len()).collect(),
            ModifyTarget::Designated => self.documents.designated.into_iter().collect(),
            ModifyTarget::Named(name) => self
                .document_at("OUTPUT MODIFY", Some(name))?
                .into_iter()
                .collect(),
        };
        if at.is_empty() {
            self.warn("no output document is open; OUTPUT MODIFY changed nothing");
        }
        let mut warned = HashSet::new();
        for block in &modify.blocks {
            for (keyword, reason) in &block.not_applied {
                if warned.insert(keyword.clone()) {
                    self.warn(format!("{keyword} changes nothing: {reason}"));
                }
            }
        }
        let now = Local::now();
        let mut report = Vec::new();
        let mut warnings = Vec::new();
        for at in at {
            let document = &mut self.documents.open[at];
            let (modified, rows) =
                modify_items(&mut document.items, &modify.blocks, &now, &mut |w| {
                    warnings.push(w)
                });
            if !modified.is_empty() {
                document.changed = true;
                self.documents.modified.push(Modified {
                    document: document.name.clone(),
                    items: modified,
                });
            }
            let name = document.name.clone();
            report.extend(rows.into_iter().map(|(label, command, what)| {
                vec![
                    Cell::Text(name.clone()),
                    Cell::Text(label),
                    Cell::Text(command),
                    Cell::Text(what),
                ]
            }));
        }
        for w in warnings {
            if warned.insert(w.clone()) {
                self.warn(w);
            }
        }
        if !modify.report {
            return Ok(Vec::new());
        }
        let caption = report
            .is_empty()
            .then(|| "No item met the conditions.".to_string());
        Ok(vec![Table {
            title: "Output Changes".into(),
            row_headers: 0,
            columns: vec![
                "Document".into(),
                "Item".into(),
                "Command".into(),
                "Changes".into(),
            ],
            rows: report,
            caption,
            ..Table::default()
        }])
    }
}

/// The changes of every block on the items of one document. Returns what changed for the
/// window, and per changed object its label, command and the changes, for the report.
fn modify_items(
    items: &mut Vec<Item>,
    blocks: &[ModifyBlock],
    now: &DateTime<Local>,
    warn: &mut dyn FnMut(String),
) -> (Vec<ModifiedItem>, Vec<(String, String, String)>) {
    let before = items.clone();
    let mut deleted_leaves = HashSet::new();
    let mut deleted_commands = HashSet::new();
    // per object number: its label for the report and what was done
    let mut done: Vec<(usize, String, String, Vec<&'static str>)> = Vec::new();
    for block in blocks {
        let all = objects(items);
        // the items earlier blocks deleted
        let (leaves_gone, commands_gone) = (deleted_leaves.clone(), deleted_commands.clone());
        let gone = |o: &Object| {
            o.leaf.is_some_and(|l| leaves_gone.contains(&l))
                || o.command.is_some_and(|c| commands_gone.contains(&c))
        };
        let picked = pick(block, &all, &gone);
        let start = block.indexing.map_or(1, |i| i.start);
        let style = block.indexing.map_or(IndexStyle::Number, |i| i.style);
        let mut changes: HashMap<usize, Change> = HashMap::new();
        // VISIBLE on a heading reaches the items of its command
        let mut shown: HashMap<usize, bool> = HashMap::new();
        for (n, &i) in picked.iter().enumerate() {
            let o = &all[i];
            let mut what = Vec::new();
            if block.delete {
                match o.leaf {
                    Some(leaf) => deleted_leaves.insert(leaf),
                    None => deleted_commands.insert(o.command.expect("a heading")),
                };
                what.push("deleted");
            } else {
                changes.insert(
                    i,
                    Change {
                        block,
                        index: index_text(style, start + n),
                    },
                );
                if let Some(visible) = block.visible.filter(|_| o.kind != Kind::Unknown) {
                    if o.kind == Kind::Heading {
                        let members = all.iter().enumerate().filter(|(_, m)| {
                            m.leaf.is_some() && m.command == o.command && !gone(m)
                        });
                        shown.extend(members.map(|(j, _)| (j, visible)));
                    } else {
                        shown.insert(i, visible);
                    }
                    what.push(if visible { "shown" } else { "hidden" });
                }
                what.extend(described(block, o.kind));
            }
            if !what.is_empty() {
                done.push((i, o.label.clone(), o.command_name.clone(), what));
            }
        }
        walk_mut(items, &mut |i, target| {
            let o = &all[i];
            let target = match (shown.get(&i), target) {
                (Some(&visible), Target::Leaf(item)) => {
                    hide(item, !visible);
                    Target::Leaf(item)
                }
                (_, target) => target,
            };
            if let Some(change) = changes.get(&i) {
                apply(o, target, change, now, warn);
            }
        });
    }
    let objects_before = objects(&before);
    // shortcut: a command inside a command (no Statscraft command writes one) goes with the
    // outer one, but its leaves are not counted as deleted below; count them when such files
    // turn up.
    output_file::retain(
        items,
        |c| !deleted_commands.contains(&c),
        |l| !deleted_leaves.contains(&l),
    );
    // what the window shows: compare every leaf with what it was
    let mut leaves_after: Vec<Option<(String, &Item)>> = Vec::new();
    {
        let mut after_leaves = leaves_with_heading(items).into_iter();
        for o in objects_before.iter().filter(|o| o.leaf.is_some()) {
            let gone = deleted_leaves.contains(&o.leaf.expect("a leaf"))
                || o.command.is_some_and(|c| deleted_commands.contains(&c));
            leaves_after.push(if gone { None } else { after_leaves.next() });
        }
    }
    let old = leaves_with_heading(&before);
    let mut modified = Vec::new();
    for (position, (now_leaf, (old_heading, old_item))) in leaves_after.iter().zip(old).enumerate()
    {
        let item = match now_leaf {
            None => {
                modified.push(ModifiedItem {
                    position,
                    deleted: true,
                    heading: None,
                    table: None,
                    chart: None,
                });
                continue;
            }
            Some(now) => now,
        };
        let heading = (item.0 != old_heading).then(|| item.0.clone());
        let same = output_value(item.1) == output_value(old_item);
        if same && heading.is_none() {
            continue;
        }
        let (table, chart) = item.1.shown();
        modified.push(ModifiedItem {
            position,
            deleted: false,
            heading,
            table: Some(table),
            chart,
        });
    }
    // one report row per item, with all that happened to it
    let mut rows: Vec<(String, String, String)> = Vec::new();
    let mut order: Vec<usize> = Vec::new();
    let mut by_object: HashMap<usize, (String, String, Vec<&str>)> = HashMap::new();
    for (i, label, command, what) in done {
        let entry = by_object.entry(i).or_insert_with(|| {
            order.push(i);
            (label, command, Vec::new())
        });
        for w in what {
            if !entry.2.contains(&w) {
                entry.2.push(w);
            }
        }
    }
    for i in order {
        let (label, command, what) = by_object.remove(&i).expect("listed");
        rows.push((label, command, what.join(", ")));
    }
    (modified, rows)
}

/// The words of the report for what a block does to an item of this kind.
fn described(block: &ModifyBlock, kind: Kind) -> Vec<&'static str> {
    let mut what = Vec::new();
    let leafy = kind != Kind::Unknown;
    if block.outline_label.is_some() && leafy {
        what.push("outline label");
    }
    if kind == Kind::Table {
        if block.table_title.is_some() {
            what.push("title");
        }
        if !block.cells.is_empty() {
            what.push("cells");
        }
        if block.sort.is_some() {
            what.push("sorted");
        }
        if block.transpose {
            what.push("transposed");
        }
    }
    if block.text.is_some() && matches!(kind, Kind::PageTitle | Kind::Heading) {
        what.push("text");
    }
    what
}

fn output_value(item: &Item) -> String {
    output_file::write(std::slice::from_ref(item))
}

/// Every leaf with the heading of the command that holds it ("" at the top level).
fn leaves_with_heading(items: &[Item]) -> Vec<(String, &Item)> {
    fn walk<'a>(items: &'a [Item], heading: &str, out: &mut Vec<(String, &'a Item)>) {
        for item in items {
            match item {
                Item::Command(c) => walk(&c.items, c.heading(), out),
                leaf => out.push((heading.to_string(), leaf)),
            }
        }
    }
    let mut out = Vec::new();
    walk(items, "", &mut out);
    out
}

fn hide(item: &mut Item, hidden: bool) {
    match item {
        Item::Table(t) if t.hidden != hidden => Arc::make_mut(t).hidden = hidden,
        Item::Chart(c) if c.hidden != hidden => Arc::make_mut(c).hidden = hidden,
        _ => {}
    }
}

/// Applies one block's changes to one picked item.
fn apply(
    o: &Object,
    target: Target,
    change: &Change,
    now: &DateTime<Local>,
    warn: &mut dyn FnMut(String),
) {
    let block = change.block;
    let index = change.index.as_str();
    match target {
        Target::Heading(c) => {
            for template in [&block.outline_label, &block.text].into_iter().flatten() {
                let label = expand(template, c.heading(), index, now);
                c.label = Some(label);
            }
        }
        Target::Leaf(Item::Chart(chart)) => {
            if let Some(template) = &block.outline_label {
                let chart = Arc::make_mut(chart);
                let current = chart.outline.clone().unwrap_or_else(|| chart.title.clone());
                chart.outline = Some(expand(template, &current, index, now));
            }
        }
        Target::Leaf(Item::Table(table)) => {
            let table = Arc::make_mut(table);
            if let Some(template) = &block.outline_label {
                let current = table.outline.clone().unwrap_or_else(|| table.title.clone());
                table.outline = Some(expand(template, &current, index, now));
            }
            if o.kind == Kind::PageTitle {
                if let Some(template) = &block.text
                    && let Some(cell) = table.rows.first_mut().and_then(|r| r.first_mut())
                {
                    let current = match cell {
                        Cell::Text(t) => t.clone(),
                        _ => String::new(),
                    };
                    *cell = Cell::Text(expand(template, &current, index, now));
                }
                return;
            }
            if let Some(template) = &block.table_title {
                table.title = expand(template, &table.title, index, now);
            }
            for cells in &block.cells {
                table_cells(table, cells, warn);
            }
            if let Some(by) = &block.sort
                && let Err(why) = sort_rows(table, by, block.descending)
            {
                warn(format!("{} is not sorted: {why}", table.title));
            }
            if block.transpose {
                *table = transpose(table);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn text(t: &str) -> Cell {
        Cell::Text(t.into())
    }

    #[test]
    fn fields_of_a_label_template() {
        let now = Local.with_ymd_and_hms(2031, 3, 4, 17, 5, 9).unwrap();
        let filled = expand(")INDEX: )LABEL on )SDATE", "Crop yield", "iv", &now);
        assert_eq!(filled, "iv: Crop yield on 2031/03/04");
        assert_eq!(
            expand(")EDATE )ADATE", "", "", &now),
            "04.03.2031 03/04/2031"
        );
        assert_eq!(expand(")TIME )ETIME", "", "", &now), "05:05:09 17:05:09");
        assert_eq!(expand(")DATE", "", "", &now), "04-Mar-2031");
        // a parenthesis that starts no field stays, and the text of )LABEL is not read again
        assert_eq!(expand("(a) )X", "", "1", &now), "(a) )X");
        assert_eq!(expand(")LABEL", ")INDEX", "1", &now), ")INDEX");
    }

    /// One label column, then the harvest and a share under the group Field.
    fn plots() -> Table {
        Table {
            title: "Harvest".into(),
            row_headers: 1,
            columns: vec!["Plot".into(), "Kilos".into(), "Share".into()],
            column_groups: vec![vec![], vec!["Field".into()], vec!["Field".into()]],
            rows: vec![
                vec![text("east"), Cell::Number(30.0), Cell::Number(0.3)],
                vec![text("west"), Cell::Empty, Cell::Number(0.1)],
                vec![text("north"), Cell::Number(50.0), Cell::Number(0.6)],
                vec![text("Total"), Cell::Number(80.0), Cell::Number(1.0)],
            ],
            ..Table::default()
        }
    }

    fn first_column(table: &Table) -> Vec<String> {
        (0..table.rows.len())
            .map(|r| cell_text(table, r, 0))
            .collect()
    }

    #[test]
    fn sorting_keeps_the_total_and_puts_empty_cells_last() {
        let mut t = plots();
        sort_rows(&mut t, &SortColumn::Label("Kilos".into()), false).unwrap();
        assert_eq!(first_column(&t), ["east", "north", "west", "Total"]);
        sort_rows(&mut t, &SortColumn::Label("Field / Kilos".into()), true).unwrap();
        assert_eq!(first_column(&t), ["north", "east", "west", "Total"]);
        sort_rows(&mut t, &SortColumn::Position(1), false).unwrap();
        assert_eq!(first_column(&t), ["east", "north", "west", "Total"]);
        let missing = sort_rows(&mut t, &SortColumn::Position(9), false);
        assert_eq!(missing.unwrap_err(), "it has no column 9");
        // an outer label that repeats makes the rows nested
        let mut nested = plots();
        nested.row_headers = 2;
        nested.rows[1][0] = text("east");
        assert!(sort_rows(&mut nested, &SortColumn::Position(3), false).is_err());
    }

    #[test]
    fn transposing_turns_header_levels_into_row_labels() {
        let t = transpose(&plots());
        assert_eq!(t.row_headers, 2);
        assert_eq!(t.columns, ["", "", "east", "west", "north", "Total"]);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(
            t.rows[0][..3],
            [text("Field"), text("Kilos"), Cell::Number(30.0)]
        );
        assert_eq!(t.rows[1][4], Cell::Number(0.6));
        assert_eq!(t.rows[0][3], Cell::Empty);
        // and back: the body is as it was
        let back = transpose(&t);
        assert_eq!(back.rows[2][1..], [Cell::Number(50.0), Cell::Number(0.6)]);
    }

    fn cells(spec: &str) -> TableCells {
        let source = format!("OUTPUT MODIFY /SELECT TABLES /TABLECELLS {spec}.");
        let (parsed, errors) = statscraft_syntax::parse(&source);
        assert!(errors.is_empty(), "{errors:?}");
        match &parsed[0].command {
            statscraft_syntax::Command::Output(statscraft_syntax::OutputCommand::Modify(m)) => {
                m.blocks[0].cells[0].clone()
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn table_cells_pick_by_label_condition_and_rank() {
        let mut warnings = Vec::new();
        let mut warn = |w: String| warnings.push(w);
        let mut t = plots();
        let share = "SELECT=['share'] SELECTDIMENSION=COLUMNS SELECTCONDITION=['x>=0.3'] \
                     FORMAT=F5.3";
        table_cells(&mut t, &cells(share), &mut warn);
        let shares: Vec<String> = (0..4).map(|r| cell_text(&t, r, 2)).collect();
        assert_eq!(shares, [".300", "0.10", ".600", "1.000"]);
        // the row of the lowest number in the body gets a zero in every data cell
        let lowest = "SELECT=[BODY] SELECTCONDITION=[BOTTOM(1)] APPLYTO=ROWDATA REPLACE=0";
        table_cells(&mut t, &cells(lowest), &mut warn);
        assert_eq!(t.rows[1][1..], [Cell::Number(0.0), Cell::Number(0.0)]);
        // the group label picks both columns, but no cell there is empty any more
        let empty =
            "SELECT=['Field'] SELECTDIMENSION=COLUMNS SELECTCONDITION=['x=SYSMIS'] HIDE=YES";
        table_cells(&mut t, &cells(empty), &mut warn);
        assert_eq!(t.columns.len(), 3);
        let rows = "SELECT=['total' POSITION(2)] SELECTDIMENSION=ROWS HIDE=YES";
        table_cells(&mut t, &cells(rows), &mut warn);
        assert_eq!(first_column(&t), ["east", "north"]);
        table_cells(
            &mut t,
            &cells("SELECT=[TITLE] REPLACE='Kept plots'"),
            &mut warn,
        );
        assert_eq!(t.title, "Kept plots");
        let header = "SELECT=['Kilos'] SELECTDIMENSION=COLUMNS APPLYTO=COLUMNHEADER REPLACE='kg'";
        table_cells(&mut t, &cells(header), &mut warn);
        assert_eq!(t.columns, ["Plot", "kg", "Share"]);
        table_cells(&mut t, &cells("SELECT=[BODY] HIDE=YES"), &mut warn);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(
            warnings[0].contains("HIDE takes rows or columns"),
            "{warnings:?}"
        );
    }
}
