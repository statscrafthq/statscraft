//! A small Statscraft plugin (see docs/plugins.md). In every row it compares each cell with the
//! row's total of the same statistic: a cell above the total by more than the threshold turns
//! one colour, a cell below it by more than the threshold another, both in the marked style
//! (bold unless changed), and every compared cell gets a comment with the difference.

use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::io::Read;

fn describe() -> Value {
    json!({
        "name": "Example",
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": 1,
        "settings": [
            {"key": "threshold", "label": "Smallest difference marked", "kind": "number", "default": 0.05},
            {"key": "comments", "label": "Write comments", "kind": "bool", "default": true},
            {"key": "higher", "label": "Colour of a higher cell", "kind": "colour", "default": "#c6efce", "group": "Look"},
            {"key": "lower", "label": "Colour of a lower cell", "kind": "colour", "default": "#ffc7ce", "group": "Look"},
            {"key": "marked", "label": "Marked cell", "kind": "style", "default": {"bold": true}, "group": "Look"}
        ]
    })
}

/// The decorations of one table.
fn decorate(table: &Value, settings: &Map<String, Value>) -> Vec<Value> {
    let threshold = settings
        .get("threshold")
        .and_then(Value::as_f64)
        .unwrap_or(0.05);
    let colour = |key: &str, default: &str| {
        let value = settings.get(key).and_then(Value::as_str);
        value.unwrap_or(default).to_string()
    };
    let (higher, lower) = (colour("higher", "#c6efce"), colour("lower", "#ffc7ce"));
    let marked = settings.get("marked").and_then(Value::as_object);
    let comments = settings
        .get("comments")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let meta = &table["meta"];
    let numbers = |key: &str| -> Vec<u64> {
        let list = meta[key].as_array().into_iter().flatten();
        list.filter_map(Value::as_u64).collect()
    };
    let total_columns = numbers("total_columns");
    // every statistic by its cell
    let mut cells: HashMap<(u64, u64), (&str, f64)> = HashMap::new();
    for cell in meta["cells"].as_array().into_iter().flatten() {
        let (Some(row), Some(col), Some(kind), Some(estimate)) = (
            cell["row"].as_u64(),
            cell["col"].as_u64(),
            cell["kind"].as_str(),
            cell["estimate"].as_f64(),
        ) else {
            continue;
        };
        cells.insert((row, col), (kind, estimate));
    }
    let mut out = Vec::new();
    for (&(row, col), &(kind, estimate)) in &cells {
        if total_columns.contains(&col) {
            continue;
        }
        // the row's first total column with the same statistic
        let total = total_columns.iter().find_map(|&t| {
            let (k, e) = cells.get(&(row, t))?;
            (*k == kind).then_some(*e)
        });
        let Some(total) = total else {
            continue;
        };
        let difference = estimate - total;
        let mut cell = json!({"row": row, "col": col});
        if difference > threshold {
            cell["fill"] = json!(higher);
        } else if difference < -threshold {
            cell["fill"] = json!(lower);
        }
        if difference.abs() > threshold {
            // the style's keys are those of a cell; the fill stays the higher or lower colour
            for (key, value) in marked
                .into_iter()
                .flatten()
                .filter(|(key, _)| *key != "fill")
            {
                cell[key.as_str()] = value.clone();
            }
        }
        if comments {
            let text = match kind.contains("PCT") {
                true => format!("{:+.1} points from the total", 100.0 * difference),
                false => format!("{difference:+.3} from the total"),
            };
            cell["comment"] = json!(text);
        }
        out.push(cell);
    }
    out
}

fn main() {
    if std::env::args().any(|a| a == "--describe") {
        println!("{}", describe());
        return;
    }
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("cannot read the request: {e}");
        std::process::exit(1);
    }
    let request: Value = match serde_json::from_str(&input) {
        Ok(request) => request,
        Err(e) => {
            eprintln!("the request is not JSON: {e}");
            std::process::exit(1);
        }
    };
    if request["protocol"] != 1 {
        eprintln!("this plugin speaks protocol 1 only");
        std::process::exit(1);
    }
    let settings = request["settings"].as_object().cloned().unwrap_or_default();
    let tables: Vec<Value> = request["tables"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|table| json!({"id": table["id"], "cells": decorate(table, &settings)}))
        .collect();
    println!("{}", json!({ "tables": tables }));
}
