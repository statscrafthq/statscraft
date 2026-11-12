//! The Statscraft output file (.scout): an output document as UTF-8 JSON, which OUTPUT SAVE
//! writes and OUTPUT OPEN reads.
//!
//! ```json
//! {
//!   "format": "statscraft-output",
//!   "version": 1,
//!   "written_by": "statscraft 0.8.0",
//!   "items": [
//!     {
//!       "kind": "command",
//!       "command": "FREQUENCIES",
//!       "syntax": "FREQUENCIES region.",
//!       "warnings": [],
//!       "items": [
//!         {
//!           "kind": "table",
//!           "title": "region",
//!           ...
//!           "rows": [
//!             ["Valid", "North", 41, 54.7],
//!             ["Valid", "South", 34, 45.3]
//!           ]
//!         }
//!       ]
//!     }
//!   ]
//! }
//! ```
//!
//! Rules:
//! - Items are typed by `kind`: `command` (one command's output, holding its own items),
//!   `table` (the engine `Table` as serde writes it) and `chart` (the `Chart` description
//!   under the key `chart`, as it has a `kind` of its own).
//!   Any kind may stand at the top level. An item of another kind is kept as it was read and
//!   written back unchanged; the window shows it as one line naming the kind.
//! - The writer puts keys in a fixed order, an array of plain values on one line (so a table
//!   row is one line), whole numbers without a fraction and no date, so files diff well.
//! - A table cell holding a number that is not finite is written as its text; a chart number
//!   that is not finite is written as null and reads back as NaN.
//! - `version` is one integer. Adding an optional field or a new item kind keeps it; changing
//!   what a field means, or adding one an older reader must not drop, raises it, with a step in
//!   `MIGRATIONS` that turns the older form into the newer one. A file from a newer version is
//!   refused. One golden file per version stays in the tests, and every release must read it.

use crate::{Cell, Chart, Table, cell_text};
use serde_json::{Map, Value};
use std::fmt::Write;
use std::sync::Arc;

pub const FORMAT: &str = "statscraft-output";
/// The version this build writes.
pub const VERSION: u64 = 1;
pub const EXTENSION: &str = "scout";

/// One step per version: entry n turns a file of version n + 1 into version n + 2.
const MIGRATIONS: &[fn(&mut Value)] = &[];

/// One item of an output document.
#[derive(Debug, Clone)]
pub enum Item {
    Command(CommandItem),
    Table(Arc<Table>),
    Chart(Arc<Chart>),
    /// An item of a kind this version does not know, as it was read.
    Unknown(Value),
}

/// The output of one command: its name, the syntax that ran, its warnings and its items.
#[derive(Debug, Clone, Default)]
pub struct CommandItem {
    pub command: String,
    pub syntax: String,
    pub warnings: Vec<String>,
    pub items: Vec<Item>,
}

impl Item {
    /// How a leaf shows in an output window: a table, or a chart as its data table with the
    /// chart. An unknown item shows as a table with only a title that names its kind.
    pub fn shown(&self) -> (Table, Option<Chart>) {
        match self {
            Item::Table(table) => ((**table).clone(), None),
            Item::Chart(chart) => (chart.data_table(), Some((**chart).clone())),
            Item::Unknown(value) => {
                let kind = value.get("kind").and_then(Value::as_str).unwrap_or("?");
                let title = format!(
                    "An output item of kind {kind}, which this version of Statscraft cannot \
                     show; it is kept and saved as it is"
                );
                let table = Table {
                    title,
                    ..Table::default()
                };
                (table, None)
            }
            Item::Command(c) => (
                Table {
                    title: c.command.clone(),
                    ..Table::default()
                },
                None,
            ),
        }
    }
}

/// Every table, chart and unknown item, depth first, each with the name of the command that
/// holds it ("" at the top level). An output window shows exactly these, in this order.
pub fn leaves(items: &[Item]) -> Vec<(&str, &Item)> {
    fn walk<'a>(items: &'a [Item], command: &'a str, out: &mut Vec<(&'a str, &'a Item)>) {
        for item in items {
            match item {
                Item::Command(c) => walk(&c.items, &c.command, out),
                leaf => out.push((command, leaf)),
            }
        }
    }
    let mut out = Vec::new();
    walk(items, "", &mut out);
    out
}

/// The items grouped as an output window shows them: per command its name, its syntax and its
/// leaves; a leaf at the top level stands alone under its own title.
pub fn by_command(items: &[Item]) -> Vec<(String, String, Vec<(Table, Option<Chart>)>)> {
    items
        .iter()
        .map(|item| match item {
            Item::Command(c) => {
                let shown = leaves(&c.items).into_iter().map(|(_, l)| l.shown());
                (c.command.clone(), c.syntax.clone(), shown.collect())
            }
            leaf => {
                let shown = leaf.shown();
                (shown.0.title.clone(), String::new(), vec![shown])
            }
        })
        .collect()
}

/// Removes the leaves at `positions` (in the order of `leaves`) and the commands left without
/// any. False, and nothing removed, when a position is past the last leaf.
pub(crate) fn remove(items: &mut Vec<Item>, positions: &[usize]) -> bool {
    if positions.iter().any(|&p| p >= leaves(items).len()) {
        return false;
    }
    fn walk(items: &mut Vec<Item>, positions: &[usize], next: &mut usize) {
        items.retain_mut(|item| match item {
            Item::Command(c) => {
                let before = leaves(&c.items).len();
                walk(&mut c.items, positions, next);
                before == 0 || !leaves(&c.items).is_empty()
            }
            _ => {
                *next += 1;
                !positions.contains(&(*next - 1))
            }
        });
    }
    walk(items, positions, &mut 0);
    true
}

/// The text of an output file holding `items`.
pub fn write(items: &[Item]) -> String {
    let mut file = Map::new();
    file.insert("format".into(), FORMAT.into());
    file.insert("version".into(), VERSION.into());
    let by = concat!("statscraft ", env!("CARGO_PKG_VERSION"));
    file.insert("written_by".into(), by.into());
    file.insert("items".into(), items.iter().map(item_value).collect());
    let mut out = String::new();
    print(&mut out, &Value::Object(file), 0);
    out.push('\n');
    out
}

fn tagged(kind: &str, value: Value) -> Value {
    let mut map = Map::new();
    map.insert("kind".into(), kind.into());
    if let Value::Object(fields) = value {
        map.extend(fields);
    }
    Value::Object(map)
}

fn item_value(item: &Item) -> Value {
    match item {
        Item::Command(c) => {
            let mut map = Map::new();
            map.insert("kind".into(), "command".into());
            map.insert("command".into(), c.command.clone().into());
            map.insert("syntax".into(), c.syntax.clone().into());
            map.insert("warnings".into(), c.warnings.clone().into());
            map.insert("items".into(), c.items.iter().map(item_value).collect());
            Value::Object(map)
        }
        Item::Table(table) => {
            let mut finite = (**table).clone();
            for (r, row) in finite.rows.iter_mut().enumerate() {
                for (c, cell) in row.iter_mut().enumerate() {
                    if matches!(cell, Cell::Number(n) if !n.is_finite()) {
                        *cell = Cell::Text(cell_text(table, r, c));
                    }
                }
            }
            tagged("table", serde_json::to_value(finite).expect("plain data"))
        }
        Item::Chart(chart) => {
            // the chart has a kind of its own (bar, pie), so it stands under its own key
            let mut map = Map::new();
            map.insert("kind".into(), "chart".into());
            let chart = serde_json::to_value(&**chart).expect("plain data");
            map.insert("chart".into(), chart);
            Value::Object(map)
        }
        Item::Unknown(value) => value.clone(),
    }
}

fn scalar(value: &Value) -> bool {
    !matches!(value, Value::Array(_) | Value::Object(_))
}

/// JSON with two spaces of indent, an array of plain values on one line.
fn print(out: &mut String, value: &Value, depth: usize) {
    let pad = |depth: usize| "  ".repeat(depth);
    match value {
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Object(map) if map.is_empty() => out.push_str("{}"),
        Value::Array(items) if items.iter().all(scalar) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                print(out, item, depth);
            }
            out.push(']');
        }
        Value::Array(items) => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                out.push_str(&pad(depth + 1));
                print(out, item, depth + 1);
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            out.push_str(&pad(depth));
            out.push(']');
        }
        Value::Object(map) => {
            out.push_str("{\n");
            for (i, (key, item)) in map.iter().enumerate() {
                let _ = write!(out, "{}{}: ", pad(depth + 1), Value::from(key.as_str()));
                print(out, item, depth + 1);
                out.push_str(if i + 1 < map.len() { ",\n" } else { "\n" });
            }
            out.push_str(&pad(depth));
            out.push('}');
        }
        Value::Number(n) => match n.as_f64() {
            // whole numbers without ".0"; both forms read back as the same number
            Some(x) if n.is_f64() && x.fract() == 0.0 && x.abs() < 9.0e15 => {
                let _ = write!(out, "{}", x as i64);
            }
            _ => out.push_str(&n.to_string()),
        },
        other => out.push_str(&other.to_string()),
    }
}

/// Why a file could not be read as an output file.
#[derive(Debug, PartialEq)]
pub struct ReadError(pub String);

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The items of an output file: the format checked, older versions migrated.
pub fn read(text: &str) -> Result<Vec<Item>, ReadError> {
    let not_ours = || ReadError("it is not a Statscraft output file".into());
    let mut file: Value = serde_json::from_str(text).map_err(|_| not_ours())?;
    if file.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(not_ours());
    }
    let version = file.get("version").and_then(Value::as_u64).unwrap_or(0);
    if version == 0 {
        return Err(not_ours());
    }
    if version > VERSION {
        return Err(ReadError(format!(
            "it was written by a newer Statscraft (output format {version}); update Statscraft"
        )));
    }
    for step in &MIGRATIONS[(version - 1) as usize..] {
        step(&mut file);
    }
    match file.get("items") {
        Some(Value::Array(items)) => items.iter().map(read_item).collect(),
        None => Ok(Vec::new()),
        Some(_) => Err(ReadError("its items are not a list".into())),
    }
}

fn read_item(value: &Value) -> Result<Item, ReadError> {
    let bad = |e: &dyn std::fmt::Display| ReadError(format!("a broken item: {e}"));
    let kind = value.get("kind").and_then(Value::as_str);
    let fields = || {
        let mut map = value.as_object().cloned().unwrap_or_default();
        map.remove("kind");
        Value::Object(map)
    };
    match kind {
        Some("command") => {
            let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or("");
            let warnings = value.get("warnings").cloned().unwrap_or(Value::Null);
            let warnings: Option<Vec<String>> =
                serde_json::from_value(warnings).map_err(|e| bad(&e))?;
            let items = match value.get("items") {
                Some(Value::Array(items)) => items.iter().map(read_item).collect(),
                None => Ok(Vec::new()),
                Some(_) => Err(bad(&"the items of a command are not a list")),
            }?;
            Ok(Item::Command(CommandItem {
                command: text("command").into(),
                syntax: text("syntax").into(),
                warnings: warnings.unwrap_or_default(),
                items,
            }))
        }
        Some("table") => {
            let table: Table = serde_json::from_value(fields()).map_err(|e| bad(&e))?;
            Ok(Item::Table(Arc::new(table)))
        }
        Some("chart") => {
            let chart = value.get("chart").cloned().unwrap_or(Value::Null);
            // shortcut: a chart that does not read, such as a plot number that was not finite
            // (written as null), is kept as it is, like an unknown item; read null as NaN in
            // more plot fields when such a chart turns up.
            Ok(match serde_json::from_value::<Chart>(chart) {
                Ok(chart) => Item::Chart(Arc::new(chart)),
                Err(_) => Item::Unknown(value.clone()),
            })
        }
        Some(_) => Ok(Item::Unknown(value.clone())),
        None => Err(bad(&"an item without a kind")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(title: &str) -> Item {
        Item::Table(Arc::new(Table {
            title: title.into(),
            ..Table::default()
        }))
    }

    fn titles(items: &[Item]) -> Vec<String> {
        leaves(items)
            .iter()
            .map(|(_, l)| l.shown().0.title)
            .collect()
    }

    #[test]
    fn removing_leaves_drops_the_commands_left_empty() {
        let command = |name: &str, items| {
            Item::Command(CommandItem {
                command: name.into(),
                items,
                ..CommandItem::default()
            })
        };
        let mut items = vec![
            command("A", vec![table("a1"), table("a2")]),
            table("top"),
            command("B", vec![table("b1")]),
            command("EMPTY", vec![]),
        ];
        assert!(!remove(&mut items, &[4]));
        assert_eq!(titles(&items), ["a1", "a2", "top", "b1"]);
        assert!(remove(&mut items, &[1, 3]));
        assert_eq!(titles(&items), ["a1", "top"]);
        // B went with its last table; the command that never had one stays
        assert_eq!(items.len(), 3);
        assert!(remove(&mut items, &[0]));
        assert_eq!(items.len(), 2);
        assert_eq!(by_command(&items)[0].0, "top");
    }

    #[test]
    fn rows_stand_one_per_line_and_whole_numbers_have_no_fraction() {
        let item = Item::Table(Arc::new(Table {
            title: "t".into(),
            row_headers: 1,
            columns: vec!["a".into(), "b".into()],
            column_groups: vec![vec![], vec![]],
            rows: vec![
                vec![Cell::Text("x".into()), Cell::Number(41.0)],
                vec![Cell::Text("y".into()), Cell::Number(0.1)],
            ],
            ..Table::default()
        }));
        let text = write(&[item]);
        assert!(text.contains("\n        [\"x\", 41],\n"), "{text}");
        assert!(text.contains("\n        [\"y\", 0.1]\n"), "{text}");
        assert!(text.contains("\"column_groups\": [\n"), "{text}");
        assert!(text.starts_with("{\n  \"format\": \"statscraft-output\",\n  \"version\": 1,\n"));
    }

    #[test]
    fn foreign_and_newer_files_are_refused() {
        let not_ours = Err(ReadError("it is not a Statscraft output file".into()));
        assert_eq!(read("PK\u{3}\u{4}").map(|_| ()), not_ours);
        assert_eq!(read("{\"format\": \"other\"}").map(|_| ()), not_ours);
        assert_eq!(
            read("{\"format\": \"statscraft-output\"}").map(|_| ()),
            not_ours
        );
        let newer = read("{\"format\": \"statscraft-output\", \"version\": 2, \"items\": []}");
        assert!(
            newer
                .unwrap_err()
                .0
                .contains("newer Statscraft (output format 2)")
        );
    }
}
