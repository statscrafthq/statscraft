//! The mutations that ADD FILES and MATCH FILES build on: taking cases, copying a variable from
//! another dataset and changing a string width.

mod common;

use common::fixture;
use statscraft_data::{Dataset, Values};

/// 200 cases, `resp_id` 1001 to 1200, `plan` is A10.
fn survey() -> Dataset {
    fixture("survey_basic_phpwriter.sav")
}

fn numbers(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    data.variable(name)
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect()
}

fn strings(data: &Dataset, name: &str) -> Vec<String> {
    let values = data.variable(name).unwrap().string().unwrap();
    values.iter().map(str::to_string).collect()
}

#[test]
fn take_cases_repeats_drops_and_blanks() {
    let mut data = survey();
    data.take_cases(&[Some(2), None, Some(0), Some(2)]).unwrap();
    assert_eq!(data.case_count(), 4);
    assert_eq!(
        numbers(&data, "resp_id"),
        [Some(1003.0), None, Some(1001.0), Some(1003.0)]
    );
    assert_eq!(strings(&data, "plan"), ["BASIC", "", "BASIC", "BASIC"]);
    // the result is a consistent dataset: it can be extended and saved
    data.set_numeric_values("n", vec![Some(1.0); 4]).unwrap();
    let path = std::env::temp_dir().join(format!("take-{}.sav", std::process::id()));
    data.write_sav(&path).unwrap();
    let back = Dataset::read_sav(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(numbers(&back, "resp_id")[1], None);

    data.take_cases(&[]).unwrap();
    assert_eq!(data.case_count(), 0);
    let error = survey().take_cases(&[Some(200)]).unwrap_err();
    assert_eq!(
        error.to_string(),
        "case 200 is out of range: there are 200 cases"
    );
}

#[test]
fn copy_variable_brings_its_dictionary() {
    let source = survey();
    let mut data = survey();
    data.delete_variables(&["q1_satisfaction"]).unwrap();
    data.take_cases(&[Some(0), Some(1)]).unwrap();
    data.copy_variable(&source, "Q1_SATISFACTION", &[Some(7), None])
        .unwrap();
    data.copy_variable(&source, "plan", &[None, Some(1)])
        .unwrap_err();
    let copied = data.variable("q1_satisfaction").unwrap();
    let original = source.variable("q1_satisfaction").unwrap();
    assert_eq!(data.variable_names().last(), Some("q1_satisfaction"));
    assert_eq!(copied.label(), original.label());
    assert_eq!(copied.format(), original.format());
    assert_eq!(copied.missing_values(), original.missing_values());
    assert_eq!(copied.value_labels(), original.value_labels());
    let Values::Numeric(values) = copied.values() else {
        panic!()
    };
    assert_eq!(values.iter().collect::<Vec<_>>(), [Some(99.0), None]);

    assert!(data.copy_variable(&source, "age", &[None]).is_err());
    assert!(data.copy_variable(&source, "nope", &[None, None]).is_err());
}

#[test]
fn set_string_width() {
    let mut data = survey();
    data.set_string_width("plan", 20).unwrap();
    let plan = data.variable("plan").unwrap();
    assert_eq!((plan.string_width(), plan.format()), (Some(20), "A20"));
    assert_eq!(strings(&data, "plan")[0], "BASIC");
    data.set_string_values("plan", 20, vec!["x".repeat(20); 200])
        .unwrap();

    let error = data.set_string_width("plan", 5).unwrap_err();
    assert!(
        error.to_string().contains("does not fit in 5 bytes"),
        "{error}"
    );
    assert!(data.set_string_width("age", 5).is_err());
    assert!(data.set_string_width("plan", 0).is_err());
}
