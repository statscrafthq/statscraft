//! `describe` must agree with the ground truth stored next to every fixture.

use serde_json::Value as Json;
use statscraft_data::Dataset;
use std::{fs, path::PathBuf};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// Every key the sidecar states must have the same value in our description.
fn assert_covers(file: &str, at: &str, expected: &Json, actual: &Json) {
    match expected {
        Json::Object(map) => {
            for (key, want) in map {
                let got = actual.get(key).unwrap_or(&Json::Null);
                assert_covers(file, &format!("{at}.{key}"), want, got);
            }
        }
        Json::Array(items) => {
            let got = actual
                .as_array()
                .unwrap_or_else(|| panic!("{file}{at}: not an array"));
            assert_eq!(got.len(), items.len(), "{file}{at}: length");
            for (i, want) in items.iter().enumerate() {
                assert_covers(file, &format!("{at}[{i}]"), want, &got[i]);
            }
        }
        Json::Number(want) => assert_eq!(actual.as_f64(), want.as_f64(), "{file}{at}"),
        _ => assert_eq!(actual, expected, "{file}{at}"),
    }
}

#[test]
fn describe_matches_every_fixture() {
    let mut checked = 0;
    for entry in fs::read_dir(fixtures()).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        if !name.ends_with(".sav") && !name.ends_with(".zsav") {
            continue;
        }
        let sidecar = format!(
            "{}.expected.json",
            name.strip_suffix(".sav").unwrap_or(&name)
        );
        let mut expected: Json =
            serde_json::from_str(&fs::read_to_string(path.with_file_name(sidecar)).unwrap())
                .unwrap();
        // case values are checked once the engine exposes them (stage E3)
        expected.as_object_mut().unwrap().remove("cases_sample");

        let actual = serde_json::to_value(Dataset::read_sav(&path).unwrap().describe()).unwrap();
        assert_covers(&name, "", &expected, &actual);
        checked += 1;
    }
    assert_eq!(checked, 7, "fixture count changed, update this number");
}

#[test]
fn user_missing_values_are_counted() {
    let d = Dataset::read_sav(fixtures().join("edge_cases.sav"))
        .unwrap()
        .describe();
    for v in &d.variables {
        assert_eq!(
            v.n_valid + v.n_system_missing + v.n_user_missing,
            d.case_count,
            "{}",
            v.name
        );
    }
    let all_missing = d
        .variables
        .iter()
        .find(|v| v.name == "all_missing")
        .unwrap();
    assert_eq!((all_missing.n_valid, all_missing.n_system_missing), (0, 12));
}

#[test]
fn missing_file_is_an_error() {
    assert!(Dataset::read_sav(fixtures().join("no_such_file.sav")).is_err());
}
