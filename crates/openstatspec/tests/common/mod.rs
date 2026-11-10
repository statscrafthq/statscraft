//! Helpers shared by the integration tests. Each test file uses a part of them.
#![allow(dead_code)]

use serde_json::Value;
use statscraft_data::{Dataset, Values};
use std::path::{Path, PathBuf};

/// The specification folder, read in place: OPENSTATSPEC_DIR or `openStatSpec/specification`
/// beside an ancestor of this crate. Nothing from it is copied into this repository.
pub fn spec_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("OPENSTATSPEC_DIR") {
        return Some(dir.into());
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|a| a.join("openStatSpec/specification"))
        .find(|p| p.join("conformance").is_dir())
}

/// Runs `test` with the conformance folder, or skips with a message (CI has no checkout).
pub fn with_spec(test: impl FnOnce(&Path)) {
    match spec_dir() {
        Some(dir) => test(&dir.join("conformance")),
        None => eprintln!(
            "skipped: OpenStatSpec specification not found; set OPENSTATSPEC_DIR to its folder"
        ),
    }
}

pub fn load(dir: &Path, name: &str) -> Value {
    let path = dir.join(name);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    serde_json::from_str(&text).unwrap()
}

pub fn cases(manifest: &Value) -> &Vec<Value> {
    manifest["cases"].as_array().expect("cases")
}

/// Everything the model holds that differs between two datasets, except the compression.
pub fn differences(a: &Dataset, b: &Dataset) -> Vec<String> {
    let mut out = vec![];
    let describe = |d: &Dataset| {
        let mut v = serde_json::to_value(d.describe()).unwrap();
        v["compression"] = Value::Null;
        v
    };
    if describe(a) != describe(b) {
        out.push(format!("describe: {} vs {}", describe(a), describe(b)));
    }
    if a.documents() != b.documents() {
        out.push("documents".into());
    }
    if a.encoding() != b.encoding() {
        out.push("encoding".into());
    }
    for (x, y) in a.variables().zip(b.variables()) {
        let name = x.name();
        let mut check = |what: &str, same: bool| {
            if !same {
                out.push(format!("{name}: {what}"));
            }
        };
        check("display width", x.display_width() == y.display_width());
        check("alignment", x.alignment() == y.alignment());
        check("role", x.role() == y.role());
        check("attributes", x.attributes() == y.attributes());
        check("value labels", x.value_labels() == y.value_labels());
        check("missing values", x.missing_values() == y.missing_values());
        let same = match (x.values(), y.values()) {
            (Values::Numeric(p), Values::Numeric(q)) => {
                let bits = |v: Option<f64>| v.map(f64::to_bits);
                p.iter().map(bits).eq(q.iter().map(bits))
            }
            (Values::String(p), Values::String(q)) => p.iter().eq(q.iter()),
            _ => false,
        };
        check("values", same);
    }
    out
}

/// Writes `d` to a scratch file of that name and reads it back.
pub fn via_sav(d: &Dataset, scratch: &Path, file: &str) -> Dataset {
    let out = scratch.join(file);
    d.write_sav(&out).unwrap();
    let again = Dataset::read_sav(&out).unwrap();
    std::fs::remove_file(&out).unwrap();
    again
}
