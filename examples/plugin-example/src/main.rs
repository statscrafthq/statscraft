//! A small Statscraft plugin (see docs/plugins.md): a heat map. Each cell of a custom table is
//! shaded from one colour to another by its value among the cells of the same statistic in the
//! table, totals left out; the largest value of each row takes the style "Largest value of a
//! row" (bold unless changed); and every shaded cell gets a comment with its base.

use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::io::Read;

fn describe() -> Value {
    json!({
        "name": "Example",
        "version": env!("CARGO_PKG_VERSION"),
        "protocol": 1,
        "settings": [
            {"key": "comments", "label": "Show each cell's base in a comment", "kind": "bool", "default": true},
            {"key": "low", "label": "Colour of the smallest value", "kind": "colour", "default": "#ffffff", "group": "Look"},
            {"key": "high", "label": "Colour of the largest value", "kind": "colour", "default": "#5b9bd5", "group": "Look"},
            {"key": "largest", "label": "Largest value of a row", "kind": "style", "default": {"bold": true}, "group": "Look"}
        ]
    })
}

/// `#rrggbb` as its three channels.
fn channels(colour: &str) -> Option<[f64; 3]> {
    let hex = colour.strip_prefix('#').filter(|h| h.len() == 6)?;
    let channel = |i: usize| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok();
    Some([channel(0)?, channel(2)?, channel(4)?].map(f64::from))
}

/// The colour a share `t` (0 to 1) of the way from `low` to `high`.
fn blend(low: [f64; 3], high: [f64; 3], t: f64) -> String {
    let [r, g, b] = [0, 1, 2].map(|i| (low[i] + (high[i] - low[i]) * t).round() as u8);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// A statistic of one cell.
struct Stat<'a> {
    row: u64,
    col: u64,
    kind: &'a str,
    estimate: f64,
    base: f64,
}

/// The decorations of one table.
fn decorate(table: &Value, settings: &Map<String, Value>) -> Vec<Value> {
    let colour = |key: &str, default: &str| {
        let value = settings.get(key).and_then(Value::as_str);
        channels(value.unwrap_or(default)).or_else(|| channels(default))
    };
    let (Some(low), Some(high)) = (colour("low", "#ffffff"), colour("high", "#5b9bd5")) else {
        return Vec::new();
    };
    let largest = settings.get("largest").and_then(Value::as_object);
    let comments = settings
        .get("comments")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let meta = &table["meta"];
    let numbers = |key: &str| -> Vec<u64> {
        let list = meta[key].as_array().into_iter().flatten();
        list.filter_map(Value::as_u64).collect()
    };
    let (total_rows, total_columns) = (numbers("total_rows"), numbers("total_columns"));
    // the cells to shade: every statistic outside the total rows and columns
    let stats: Vec<Stat> = meta["cells"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|cell| {
            Some(Stat {
                row: cell["row"].as_u64()?,
                col: cell["col"].as_u64()?,
                kind: cell["kind"].as_str()?,
                estimate: cell["estimate"].as_f64()?,
                base: cell["base"].as_f64().unwrap_or(0.0),
            })
        })
        .filter(|s| !total_rows.contains(&s.row) && !total_columns.contains(&s.col))
        .collect();
    // the range of each statistic, and the largest value of each row by statistic
    let mut range: HashMap<&str, (f64, f64)> = HashMap::new();
    let mut top: HashMap<(u64, &str), f64> = HashMap::new();
    for s in &stats {
        let (min, max) = range.entry(s.kind).or_insert((s.estimate, s.estimate));
        (*min, *max) = (min.min(s.estimate), max.max(s.estimate));
        let best = top.entry((s.row, s.kind)).or_insert(s.estimate);
        *best = best.max(s.estimate);
    }
    let mut out = Vec::new();
    for s in &stats {
        let (min, max) = range[s.kind];
        let t = if max > min {
            (s.estimate - min) / (max - min)
        } else {
            0.0
        };
        let mut cell = json!({"row": s.row, "col": s.col, "fill": blend(low, high, t)});
        if s.estimate == top[&(s.row, s.kind)] {
            // the style's keys are those of a cell; the fill stays the shade
            let keys = largest.into_iter().flatten();
            for (key, value) in keys.filter(|(key, _)| *key != "fill") {
                cell[key.as_str()] = value.clone();
            }
        }
        if comments {
            cell["comment"] = json!(format!("n = {}", s.base));
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
