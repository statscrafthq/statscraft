//! Plugins: programs of their own that colour and comment the cells of tables with cell
//! statistics (docs/plugins.md). A plugin runs with `--describe` for its name, version and
//! settings, and once per batch of tables to decorate them: one JSON document in on its
//! standard input, one out on its standard output. A plugin that fails, answers badly or takes
//! too long gives an error and no decorations; the tables stay as they are.

use crate::{CellStats, Decoration, Table, cell_text};
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
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Number,
    Text,
    Colour,
    Bool,
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

    /// The value of every setting: from the data file attribute `stored` finds, else the
    /// plugin's default. A number or yes/no that does not read as one takes the default.
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

/// Runs the plugin once on the tables that have cell statistics, each with its id, and
/// returns the decorations of each table it answered for.
pub fn decorate(
    program: &Path,
    settings: &Map<String, Value>,
    tables: &[(u64, &Table)],
) -> Result<Vec<(u64, Vec<Decoration>)>, String> {
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
    let response: Response = serde_json::from_slice(&out)
        .map_err(|e| format!("the plugin's answer is not valid: {e}"))?;
    Ok(response
        .tables
        .into_iter()
        .map(|t| (t.id, t.cells))
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
            r##"{"name": "Sig Colours", "version": "1.0", "protocol": 1, "settings": [
                {"key": "level", "label": "Confidence level", "kind": "number", "default": 95},
                {"key": "up", "label": "Higher", "kind": "colour", "default": "#c6efce"},
                {"key": "notes", "label": "Comments", "kind": "bool", "default": true}]}"##,
        )
        .unwrap();
        assert_eq!(description.attribute("level"), "plugin_sig_colours_level");
        let values = description.values(|name| match name {
            "plugin_sig_colours_level" => Some("99".into()),
            "plugin_sig_colours_notes" => Some("maybe".into()),
            _ => None,
        });
        assert_eq!(values["level"], 99.0);
        assert_eq!(values["up"], "#c6efce");
        // a value that is not yes or no takes the default
        assert_eq!(values["notes"], true);
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
}
