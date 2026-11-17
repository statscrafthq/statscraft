//! Plugins: programs of their own that colour and comment the cells of tables with cell
//! statistics (docs/plugins.md). A plugin runs with `--describe` for its name, version and
//! settings, and once per batch of tables to decorate them: one JSON document in on its
//! standard input, one out on its standard output. A plugin that fails, answers badly or takes
//! too long gives an error and no decorations; the tables stay as they are.

use crate::{
    CellStats, ColumnWidth, Decoration, Table, TableStyle, cell_text, header_rows,
    merge_decorations,
};
use serde::{Deserialize, Serialize};
pub use serde_json::{Map, Value};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The protocol version this build speaks.
pub const PROTOCOL: u64 = 1;
const DESCRIBE_TIME: Duration = Duration::from_secs(10);
const DECORATE_TIME: Duration = Duration::from_secs(120);

/// What `--describe` tells about a plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Description {
    pub name: String,
    pub version: String,
    pub protocol: u64,
    #[serde(default)]
    pub settings: Vec<Setting>,
}

/// A setting the plugin takes, with its value when none is set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Setting {
    pub key: String,
    pub label: String,
    pub kind: Kind,
    #[serde(default)]
    pub default: Value,
    /// The window offers saved values of the setting (a number or text) to choose from.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub presets: bool,
    /// The window keeps the setting for each data file and leaves it out of design templates.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub per_file: bool,
    /// The heading of the section the window shows the setting in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Number,
    Text,
    Colour,
    Bool,
    /// An object of optional font, size, bold, italic, text, fill and align keys.
    Style,
}

impl Description {
    /// The data file attribute that keeps a setting: `plugin_<name>_<key>` in lower case, every
    /// character but letters and digits as `_`, at most 64 characters, as a name allows.
    pub fn attribute(&self, key: &str) -> String {
        let clean = |text: &str| -> String {
            text.chars()
                .map(|c| match c.is_ascii_alphanumeric() {
                    true => c.to_ascii_lowercase(),
                    false => '_',
                })
                .collect()
        };
        let mut name = format!("plugin_{}_{}", clean(&self.name), clean(key));
        name.truncate(64);
        name
    }

    /// The data file attribute that keeps the name of the preset a setting's value came from:
    /// the setting's attribute, cut to leave room, with `_preset` after it.
    pub fn preset_attribute(&self, key: &str) -> String {
        let mut name = self.attribute(key);
        name.truncate(64 - "_preset".len());
        name + "_preset"
    }

    /// The value of every setting: from the data file attribute `stored` finds, else the
    /// plugin's default. A number or yes/no that does not read as one, and a style that is not a
    /// JSON object, take the default.
    pub fn values(&self, stored: impl Fn(&str) -> Option<String>) -> Map<String, Value> {
        let mut values = Map::new();
        for setting in &self.settings {
            let text = stored(&self.attribute(&setting.key));
            let value = text.and_then(|text| match setting.kind {
                Kind::Number => text.trim().parse::<f64>().ok().map(Value::from),
                Kind::Bool => match text.trim().to_ascii_lowercase().as_str() {
                    "1" | "true" | "yes" => Some(Value::Bool(true)),
                    "0" | "false" | "no" => Some(Value::Bool(false)),
                    _ => None,
                },
                Kind::Text | Kind::Colour => Some(Value::String(text)),
                Kind::Style => serde_json::from_str(&text).ok().filter(Value::is_object),
            });
            values.insert(
                setting.key.clone(),
                value.unwrap_or_else(|| setting.default.clone()),
            );
        }
        values
    }
}

#[derive(Serialize)]
struct Request<'a> {
    protocol: u64,
    settings: &'a Map<String, Value>,
    tables: Vec<RequestTable<'a>>,
}

#[derive(Serialize)]
struct RequestTable<'a> {
    id: u64,
    title: &'a str,
    row_headers: usize,
    header_rows: usize,
    columns: &'a [String],
    column_groups: &'a [Vec<String>],
    rows: Vec<Vec<String>>,
    meta: &'a CellStats,
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    tables: Vec<ResponseTable>,
}

#[derive(Deserialize)]
struct ResponseTable {
    id: u64,
    #[serde(default)]
    cells: Vec<Decoration>,
    #[serde(default)]
    header_cells: Vec<Decoration>,
    font: Option<String>,
    font_size: Option<f64>,
    #[serde(default)]
    plain: bool,
    row_labels: Option<String>,
    #[serde(default)]
    column_widths: Vec<ColumnWidth>,
    freeze_columns: Option<u16>,
    #[serde(default)]
    header_heights: Vec<f64>,
    rows_above: Option<u32>,
    /// The caption's look: a cell style without its row and column.
    caption: Option<Map<String, Value>>,
    /// The title's look, the same way.
    title: Option<Map<String, Value>>,
}

/// A cell style without its row and column, as a decoration at 0, 0; None when it is not one.
fn placeless(mut look: Map<String, Value>) -> Option<Decoration> {
    look.insert("row".into(), 0.into());
    look.insert("col".into(), 0.into());
    serde_json::from_value(Value::Object(look)).ok()
}

/// Runs `program --describe`.
pub fn describe(program: &Path) -> Result<Description, String> {
    let out = run(program, &["--describe"], None, DESCRIBE_TIME)?;
    let description: Description = serde_json::from_slice(&out)
        .map_err(|e| format!("the plugin's description is not valid: {e}"))?;
    if description.protocol != PROTOCOL {
        return Err(format!(
            "the plugin speaks protocol {}, this Statscraft protocol {PROTOCOL}",
            description.protocol
        ));
    }
    Ok(description)
}

/// The decorations of a table's cells, and the font and header styles of the table.
pub type Answer = (Vec<Decoration>, TableStyle);

/// Runs the plugin once on the tables that have cell statistics, each with its id, and
/// returns the decorations of each table it answered for.
pub fn decorate(
    program: &Path,
    settings: &Map<String, Value>,
    tables: &[(u64, &Table)],
) -> Result<Vec<(u64, Answer)>, String> {
    let tables: Vec<RequestTable> = tables
        .iter()
        .filter_map(|&(id, table)| {
            let meta = table.cell_stats.as_deref()?;
            let rows = (0..table.rows.len())
                .map(|r| {
                    let cells = 0..table.rows[r].len();
                    cells.map(|c| cell_text(table, r, c)).collect()
                })
                .collect();
            Some(RequestTable {
                id,
                title: &table.title,
                row_headers: table.row_headers,
                header_rows: header_rows(table).len(),
                columns: &table.columns,
                column_groups: &table.column_groups,
                rows,
                meta,
            })
        })
        .collect();
    if tables.is_empty() {
        return Ok(Vec::new());
    }
    let request = Request {
        protocol: PROTOCOL,
        settings,
        tables,
    };
    let input = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    let out = run(program, &[], Some(input), DECORATE_TIME)?;
    answers(&out)
}

/// Lays the answers of a later plugin over those of the plugins before it, table by table:
/// cells, header cells and the table style key by key, what the later plugin sets winning
/// (`Decoration::merge`, `TableStyle::merge`). A table only the later plugin answers for is
/// added.
pub fn merge(answers: &mut Vec<(u64, Answer)>, later: Vec<(u64, Answer)>) {
    for (id, (cells, style)) in later {
        match answers.iter_mut().find(|(i, _)| *i == id) {
            Some((_, (mine, my_style))) => {
                merge_decorations(mine, cells);
                my_style.merge(style);
            }
            None => answers.push((id, (cells, style))),
        }
    }
}

/// The tables of a plugin's answer.
fn answers(out: &[u8]) -> Result<Vec<(u64, Answer)>, String> {
    let response: Response = serde_json::from_slice(out)
        .map_err(|e| format!("the plugin's answer is not valid: {e}"))?;
    Ok(response
        .tables
        .into_iter()
        .map(|t| {
            let style = TableStyle {
                font: t.font,
                font_size: t.font_size,
                header_cells: t.header_cells,
                plain: t.plain,
                row_labels: t.row_labels,
                column_widths: t.column_widths,
                freeze_columns: t.freeze_columns,
                header_heights: t.header_heights,
                rows_above: t.rows_above,
                caption: t.caption.and_then(placeless),
                title: t.title.and_then(placeless),
            };
            (t.id, (t.cells, style))
        })
        .collect())
}

/// Runs a program with `input` on its standard input and returns its standard output; an
/// error when it cannot start, fails, or runs longer than `time` (it is then stopped).
fn run(
    program: &Path,
    args: &[&str],
    input: Option<Vec<u8>>,
    time: Duration,
) -> Result<Vec<u8>, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("the plugin {} cannot start: {e}", program.display()))?;
    // written and read on threads of their own, so a large table cannot block both sides
    let mut stdin = child.stdin.take().expect("piped");
    std::thread::spawn(move || {
        if let Some(bytes) = input {
            let _ = stdin.write_all(&bytes);
        }
    });
    let mut stdout = child.stdout.take().expect("piped");
    let out = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let mut stderr = child.stderr.take().expect("piped");
    let err = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let deadline = Instant::now() + time;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "the plugin took longer than {} seconds and was stopped",
                    time.as_secs()
                ));
            }
            Err(e) => return Err(format!("the plugin cannot be watched: {e}")),
        }
    };
    let out = out.join().ok().and_then(Result::ok).unwrap_or_default();
    if !status.success() {
        let err = err.join().unwrap_or_default();
        let why = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        return Err(format!("the plugin failed ({status}) {why}")
            .trim()
            .to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_come_from_attributes_or_defaults() {
        let description: Description = serde_json::from_str(
            r##"{"name": "Heat Map", "version": "1.0", "protocol": 1, "settings": [
                {"key": "level", "label": "Colour steps", "kind": "number", "default": 95},
                {"key": "up", "label": "Higher", "kind": "colour", "default": "#c6efce"},
                {"key": "notes", "label": "Comments", "kind": "bool", "default": true},
                {"key": "universe", "label": "Population", "kind": "number", "default": 0,
                 "per_file": true}]}"##,
        )
        .unwrap();
        // only a setting that says so is kept per data file
        let per_file: Vec<bool> = description.settings.iter().map(|s| s.per_file).collect();
        assert_eq!(per_file, [false, false, false, true]);
        assert_eq!(description.attribute("level"), "plugin_heat_map_level");
        let values = description.values(|name| match name {
            "plugin_heat_map_level" => Some("99".into()),
            "plugin_heat_map_notes" => Some("maybe".into()),
            _ => None,
        });
        assert_eq!(values["level"], 99.0);
        assert_eq!(values["up"], "#c6efce");
        // a value that is not yes or no takes the default
        assert_eq!(values["notes"], true);
    }

    /// A style setting reaches the plugin as its stored object, or the default when the
    /// attribute is missing or not a JSON object; a group is read with its setting.
    #[test]
    fn style_settings_come_as_objects() {
        let description: Description = serde_json::from_str(
            r##"{"name": "House", "version": "1", "protocol": 1, "settings": [
                {"key": "head", "label": "Header", "kind": "style", "group": "Header",
                 "default": {"font": "Verdana", "size": 8, "bold": true}},
                {"key": "data", "label": "Data cell", "kind": "style", "default": {}},
                {"key": "total", "label": "Total cell", "kind": "style",
                 "default": {"fill": "#eeeeee"}}]}"##,
        )
        .unwrap();
        assert_eq!(description.settings[0].kind, Kind::Style);
        assert_eq!(description.settings[0].group.as_deref(), Some("Header"));
        assert_eq!(description.settings[1].group, None);
        let values = description.values(|name| match name {
            "plugin_house_head" => Some(r##"{"italic":true,"text":"#1f4e79"}"##.into()),
            "plugin_house_data" => Some("{not json".into()),
            _ => None,
        });
        // the stored object as it is: no key filled in from the default
        assert_eq!(
            values["head"],
            serde_json::json!({"italic": true, "text": "#1f4e79"})
        );
        assert_eq!(values["data"], serde_json::json!({}));
        assert_eq!(values["total"], serde_json::json!({"fill": "#eeeeee"}));
        // a JSON value that is not an object takes the default too
        let values = description.values(|_| Some("[1]".into()));
        assert_eq!(values["total"], serde_json::json!({"fill": "#eeeeee"}));
    }

    /// Two plugins' answers merge key by key: a fill from one and bold from the other both
    /// stay, the later one wins where both set a key, and a bad value of the later one does
    /// not wipe out a good one.
    #[test]
    fn answers_of_two_plugins_merge_key_by_key() {
        let first = br##"{"tables": [{"id": 1, "font": "Cambria", "font_size": 9,
            "column_widths": [{"col": 1, "width": 20}, {"col": 2, "width": 5}],
            "header_heights": [30, 15], "caption": {"italic": true},
            "cells": [
              {"row": 0, "col": 1, "fill": "#ff0000", "text": "#000000", "comment": "n = 7",
               "border": {"top": "#111111", "left": "#222222"}},
              {"row": 0, "col": 2, "fill": "#00ff00"}],
            "header_cells": [{"row": 0, "col": 1, "fill": "#dddddd"}]},
            {"id": 2, "cells": [{"row": 0, "col": 1, "italic": true}]}]}"##;
        let second = br##"{"tables": [{"id": 1, "font_size": 11, "plain": true,
            "column_widths": [{"col": 2, "width": 8}], "header_heights": [40],
            "caption": {"text": "#094e56"}, "title": {"bold": true},
            "cells": [
              {"row": 0, "col": 1, "bold": true, "text": "#9c0006", "fill": "teal",
               "border": {"top": "#333333"}},
              {"row": 0, "col": 1, "italic": true},
              {"row": 0, "col": 0, "comment": "label"}],
            "header_cells": [{"row": 0, "col": 1, "bold": true}]},
            {"id": 3, "cells": [{"row": 0, "col": 2, "bold": true}]}]}"##;
        let mut merged = answers(first).unwrap();
        merge(&mut merged, answers(second).unwrap());
        let ids: Vec<u64> = merged.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids, [1, 2, 3]);
        let mut table = Table {
            row_headers: 1,
            columns: vec!["".into(), "a".into(), "b".into()],
            rows: vec![vec![
                crate::Cell::Text("r".into()),
                crate::Cell::Number(1.0),
                crate::Cell::Number(2.0),
            ]],
            ..Table::default()
        };
        let (cells, style) = merged[0].1.clone();
        table.set_decorations(cells, style);
        let d = table.decoration(0, 1).unwrap();
        // the fill of the first stays (the second's is not a colour), bold comes from the
        // second, its text colour wins, and the comment of the first stays
        assert_eq!(d.fill.as_deref(), Some("#ff0000"));
        assert!(
            d.bold && !d.italic,
            "a second style of the same cell is dropped"
        );
        assert_eq!(d.text.as_deref(), Some("#9c0006"));
        assert_eq!(d.comment.as_deref(), Some("n = 7"));
        let border = d.border.as_ref().unwrap();
        assert_eq!(
            (border.top.as_deref(), border.left.as_deref()),
            (Some("#333333"), Some("#222222"))
        );
        let fill = |r, c| table.decoration(r, c).unwrap().fill.clone();
        assert_eq!(fill(0, 2).as_deref(), Some("#00ff00"));
        let label = table.decoration(0, 0).unwrap();
        assert_eq!(label.comment.as_deref(), Some("label"));
        let h = table.header_decoration(0, 1).unwrap();
        assert_eq!((h.fill.as_deref(), h.bold), (Some("#dddddd"), true));
        let s = &table.style;
        assert_eq!(
            (s.font.as_deref(), s.font_size),
            (Some("Cambria"), Some(11.0))
        );
        assert!(s.plain);
        let widths: Vec<(usize, f64)> = s.column_widths.iter().map(|w| (w.col, w.width)).collect();
        assert_eq!(widths, [(1, 20.0), (2, 8.0)]);
        let caption = s.caption.as_ref().unwrap();
        assert!(caption.italic);
        assert_eq!(caption.text.as_deref(), Some("#094e56"));
        assert!(s.title.as_ref().unwrap().bold);
        // one header line here, so only the first height is kept: the second plugin's
        assert_eq!(s.header_heights, [40.0]);
    }

    #[test]
    fn a_plugin_that_cannot_run_gives_an_error() {
        let missing = Path::new("/no/such/plugin/anywhere");
        assert!(describe(missing).unwrap_err().contains("cannot start"));
        let table = Table {
            cell_stats: Some(Box::default()),
            ..Table::default()
        };
        let error = decorate(missing, &Map::new(), &[(1, &table)]).unwrap_err();
        assert!(error.contains("cannot start"), "{error}");
        // no table with cell statistics: the plugin is not run at all
        assert_eq!(decorate(missing, &Map::new(), &[]), Ok(Vec::new()));
    }

    /// The style fields of an answer reach the table; bad colours, cells off the body or the
    /// header, a font name that is not plain and a size out of range are dropped.
    #[test]
    fn styles_in_an_answer_reach_the_table() {
        let answer = br##"{"tables": [{"id": 3, "font": "Cambria", "font_size": 8,
            "plain": true, "row_labels": "stacked", "column_widths": [{"col": 2, "width": 3}],
            "header_heights": [30, 500, 12, 9], "rows_above": 1,
            "caption": {"text": "#094e56", "italic": true, "fill": "nope"},
            "title": {"text": "#1f4e5a", "bold": true, "size": 10},
            "cells": [
              {"row": 0, "col": 1, "bold": true, "italic": true, "text": "#1f4e5a",
               "align": "center", "font": "Verdana", "size": 7.5,
               "border": {"bottom": "#4bacc6", "top": "teal"}},
              {"row": 0, "col": 0, "fill": "#eeeeee", "font": "a<b", "size": 500},
              {"row": 5, "col": 1, "bold": true}],
            "header_cells": [
              {"row": 1, "col": 2, "bold": true, "border": {"left": "#7f7f7f"}, "comment": "x"},
              {"row": 2, "col": 1, "bold": true}]},
            {"id": 4, "font": "Bad'; name", "font_size": 0, "row_labels": "sideways",
             "rows_above": 0}]}"##;
        let answers = answers(answer).unwrap();
        let mut table = Table {
            row_headers: 1,
            columns: vec!["".into(), "a".into(), "b".into()],
            column_groups: vec![vec![], vec!["g".into()], vec!["g".into()]],
            rows: vec![vec![
                crate::Cell::Text("r".into()),
                crate::Cell::Number(1.0),
                crate::Cell::Number(2.0),
            ]],
            ..Table::default()
        };
        assert_eq!(header_rows(&table).len(), 2);
        let mut bad = table.clone();
        let (cells, style) = answers[0].1.clone();
        table.set_decorations(cells, style);
        assert_eq!(table.style.font.as_deref(), Some("Cambria"));
        assert_eq!(table.style.font_size, Some(8.0));
        let d = table.decoration(0, 1).unwrap();
        assert!(d.bold && d.italic);
        assert_eq!(d.align.as_deref(), Some("center"));
        assert_eq!((d.font.as_deref(), d.size), (Some("Verdana"), Some(7.5)));
        // a font name that is not plain and a size beyond Excel's are dropped
        let first = table.decoration(0, 0).unwrap();
        assert_eq!((&first.font, first.size), (&None, None));
        // two header lines: the heights of lines beyond them dropped, the rest clamped
        assert_eq!(table.style.header_heights, [30.0, 409.0]);
        assert_eq!(table.style.rows_above, Some(1));
        let caption = table.style.caption.as_ref().unwrap();
        assert_eq!(
            (caption.text.as_deref(), caption.italic),
            (Some("#094e56"), true)
        );
        assert_eq!(caption.fill, None);
        let title = table.style.title.as_ref().unwrap();
        assert_eq!(
            (title.text.as_deref(), title.bold, title.size),
            (Some("#1f4e5a"), true, Some(10.0))
        );
        assert!(table.style.plain);
        assert_eq!(table.style.row_labels.as_deref(), Some("stacked"));
        assert_eq!(table.style.column_widths[0].width, 3.0);
        let border = d.border.as_ref().unwrap();
        assert_eq!(
            (border.bottom.as_deref(), &border.top),
            (Some("#4bacc6"), &None)
        );
        assert_eq!(
            table.decoration(0, 0).unwrap().fill.as_deref(),
            Some("#eeeeee")
        );
        assert_eq!(table.decorations.len(), 2);
        // header line 2 does not exist; a header cell has no comment
        assert_eq!(table.style.header_cells.len(), 1);
        let h = table.header_decoration(1, 2).unwrap();
        assert!(h.bold && h.comment.is_none());
        let (cells, style) = answers[1].1.clone();
        bad.set_decorations(cells, style);
        assert!(bad.style.is_empty());
        // and they survive the JSON of a saved document
        let json = serde_json::to_string(&table).unwrap();
        let read: Table = serde_json::from_str(&json).unwrap();
        assert_eq!(
            (read.decorations, read.style),
            (table.decorations, table.style)
        );
    }
}
