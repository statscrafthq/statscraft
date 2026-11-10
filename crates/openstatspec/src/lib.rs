//! OpenStatSpec driver. Part one: the the reference program syntax frontend profiles 0.1, 0.2 and 0.3 that
//! lower bounded syntax to a Transformation Plan 0.1 or 0.2, plan validation, canonical JSON
//! and the plan and source hashes. Part two (`catalog`, `apply`): a dataset stored as one wide
//! table plus the catalog in SQL, read back, and plans applied to it in place, over the
//! dialect profiles in `db` and the connections in `sqlite`, `postgres` and `mysql`.
//!
//! This is a separate, strictly bounded frontend with its own lexer and parser. StatsCraft's
//! own engine does not run on these plans.

pub mod apply;
pub mod catalog;
pub mod db;
mod frontend;
pub mod mysql;
pub mod plan;
pub mod postgres;
pub mod sqlite;

pub use frontend::{Compiled, Diagnostic, Output, Span, compile_request, normalize_source};
pub use plan::{Plan, PlanError, validate_plan};

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Request contracts of the frontend profiles, oldest first.
pub const FRONTEND_CONTRACTS: [&str; 3] = [
    "openstatspec-spss-syntax-frontend-v0.1",
    "openstatspec-spss-syntax-frontend-v0.2",
    "openstatspec-spss-syntax-frontend-v0.3",
];

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// RFC 8785 serialization over the restricted domain of the specification: strings, objects,
/// arrays, booleans, null and integers of magnitude at most 2^53 - 1. Any other number gives
/// `None`. serde_json's own writer is not used because the workspace enables `preserve_order`,
/// which keeps insertion order instead of sorting keys.
pub fn canonical_json(value: &Value) -> Option<String> {
    let mut out = String::new();
    write_canonical(value, &mut out)?;
    Some(out)
}

fn write_canonical(value: &Value, out: &mut String) -> Option<()> {
    const MAX_SAFE: u64 = (1 << 53) - 1;
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            let magnitude = n.as_u64().or(n.as_i64().map(i64::unsigned_abs))?;
            if magnitude > MAX_SAFE {
                return None;
            }
            // integers print as plain decimal digits
            out.push_str(&n.to_string());
        }
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(item, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (i, key) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                write_canonical(&map[key], out)?;
            }
            out.push('}');
        }
    }
    Some(())
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_json_sorts_keys_by_utf16_and_escapes_like_rfc8785() {
        // U+E000 sorts before U+1F600 in UTF-8 byte order but after it in UTF-16 code units.
        let v = json!({"b": [1, -2, true, null], "a": "q\"\\\n\u{1}\u{7f}é", "\u{1F600}": 0, "\u{E000}": 0});
        assert_eq!(
            canonical_json(&v).unwrap(),
            "{\"a\":\"q\\\"\\\\\\n\\u0001\u{7f}é\",\"b\":[1,-2,true,null],\"\u{1F600}\":0,\"\u{E000}\":0}"
        );
        assert_eq!(canonical_json(&json!(1.5)), None);
        assert_eq!(canonical_json(&json!(9007199254740992u64)), None);
    }

    #[test]
    fn sha256_of_canonical_bytes() {
        // Independent value: printf '{"a":1}' | sha256sum
        assert_eq!(
            sha256_hex(canonical_json(&json!({"a": 1})).unwrap().as_bytes()),
            "015abd7f5cc57a2dd94b7590f04ad8084273905ee33ec5cebeae62276a97f862"
        );
    }
}
