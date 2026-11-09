//! `describe` and the case values must agree with the ground truth stored next to every fixture.

mod common;

use common::{file_name, fixture, fixture_files, fixtures, sidecar};
use serde_json::Value as Json;
use statscraft_data::{Dataset, Values};

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
    for path in fixture_files() {
        let name = file_name(&path);
        let mut expected = sidecar(&path);
        // case values are not part of the description, `case_values_match_every_fixture` has them
        expected.as_object_mut().unwrap().remove("cases_sample");

        let actual = serde_json::to_value(Dataset::read_sav(&path).unwrap().describe()).unwrap();
        assert_covers(&name, "", &expected, &actual);
    }
}

#[test]
fn case_values_match_every_fixture() {
    for path in fixture_files() {
        let name = file_name(&path);
        let expected = sidecar(&path);
        let sample = expected["cases_sample"].as_array().unwrap();
        let data = Dataset::read_sav(&path).unwrap();
        assert!(data.warnings().is_empty(), "{name}");

        // the sample holds the first 5 and the last 2 cases
        let n = data.case_count();
        let cases: Vec<usize> = (0..n.min(5)).chain(n.saturating_sub(2).max(5)..n).collect();
        assert_eq!(sample.len(), cases.len(), "{name}: sample size");
        for (row, case) in sample.iter().zip(cases) {
            let row = row.as_array().unwrap();
            assert_eq!(row.len(), data.variable_count(), "{name}: case {case}");
            for (want, variable) in row.iter().zip(data.variables()) {
                let at = format!("{name}: case {case}, {}", variable.name());
                match variable.values() {
                    Values::Numeric(values) => {
                        assert!(want.is_number() || want.is_null(), "{at}");
                        assert_eq!(values.get(case), want.as_f64(), "{at}");
                    }
                    Values::String(values) => {
                        let want = want.as_str().unwrap_or_else(|| panic!("{at}: not text"));
                        assert_eq!(values.get(case), want.trim_end(), "{at}");
                    }
                }
            }
        }
    }
}

#[test]
fn date_variables_are_numbers() {
    let data = fixture("survey_basic.sav");
    let signup = data.variable("signup_date").unwrap();
    assert_eq!(signup.format(), "DATE11");
    // 2023-11-01 as seconds since 1582-10-14
    assert_eq!(signup.numeric().unwrap().get(0), Some(13_918_176_000.0));
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
