//! Read access and every mutation the engine uses, with their error cases.

mod common;

use common::fixture;
use statscraft_data::{Dataset, Error, Measure, MissingValues, MultipleResponseSet, Value, Values};

/// 200 cases, 19 variables, sets `$q5` and `$q6`, weight variable `weight`.
fn survey() -> Dataset {
    fixture("survey_basic_phpwriter.sav")
}

fn text(s: &str) -> Value {
    Value::String(s.to_string())
}

fn names(data: &Dataset) -> Vec<&str> {
    data.variable_names().collect()
}

macro_rules! assert_error {
    ($result:expr, $pattern:pat, $message:expr) => {
        match $result {
            Err(error @ $pattern) => assert_eq!(error.to_string(), $message),
            other => panic!("expected an error, got {other:?}"),
        }
    };
}

#[test]
fn read_access() {
    let data = survey();
    assert_eq!((data.case_count(), data.variable_count()), (200, 19));
    assert_eq!(names(&data)[..3], ["resp_id", "gender", "age"]);
    assert_eq!(data.variable_index("gender"), Some(1));
    assert_eq!(data.variable_index("GENDER"), Some(1));
    assert_eq!(data.variable_index("gendr"), None);
    assert_eq!(
        data.file_label(),
        Some("StatsCraft synthetic customer survey")
    );
    assert_eq!(data.weight_variable(), Some("weight"));

    let sets = data.multiple_response_sets();
    assert_eq!((sets[0].name.as_str(), sets[0].kind), ("$q5", "dichotomy"));
    assert_eq!(sets[0].counted_value.as_deref(), Some("1"));
    assert_eq!(
        sets[1].variables,
        ["q6_mention1", "q6_mention2", "q6_mention3"]
    );

    let q1 = data.variable("Q1_Satisfaction").unwrap();
    assert_eq!(q1.name(), "q1_satisfaction");
    assert_eq!((q1.format(), q1.measure()), ("F2.0", Measure::Ordinal));
    assert!(q1.label().unwrap().starts_with("Q1."));
    assert_eq!(q1.value_labels()[0].0, Value::Number(1.0));
    assert_eq!(
        q1.missing_values(),
        &MissingValues::Numbers(vec![98.0, 99.0])
    );
    assert!(q1.is_user_missing(98.0) && !q1.is_user_missing(5.0));
    assert!(!q1.is_string() && q1.string_width().is_none());
    let values = q1.numeric().unwrap();
    assert_eq!(values.get(0), Some(3.0));
    assert_eq!(values.iter().count(), 200);
    assert_error!(
        q1.string().map(|_| ()),
        Error::WrongType { .. },
        "q1_satisfaction is not a string variable"
    );

    let q2 = data.variable("q2_recommend").unwrap();
    assert!(
        matches!(q2.missing_values(), MissingValues::Range { low, high, extra: None } if (*low, *high) == (97.0, 99.0))
    );
    assert!(q2.is_user_missing(97.5) && !q2.is_user_missing(96.0));
    let system_missing = q2.numeric().unwrap().iter().filter(Option::is_none).count();
    assert_eq!(
        system_missing,
        data.describe().variables[5].n_system_missing
    );

    let plan = data.variable_at(7);
    assert_eq!(
        (plan.name(), plan.format(), plan.string_width()),
        ("plan", "A10", Some(10))
    );
    let Values::String(values) = plan.values() else {
        panic!("plan is a string variable");
    };
    assert_eq!(values.get(0), "BASIC");
    assert_eq!(values.iter().next(), Some("BASIC"));
    assert_error!(
        plan.numeric().map(|_| ()),
        Error::WrongType { .. },
        "plan is not a numeric variable"
    );

    assert_eq!(data.variables().filter(|v| v.is_string()).count(), 2);
    assert_error!(
        data.variable("nope").map(|_| ()),
        Error::UnknownVariable(_),
        "unknown variable: nope"
    );
}

#[test]
fn variable_label() {
    let mut data = survey();
    data.set_variable_label("AGE", Some("Vanus aastates"))
        .unwrap();
    assert_eq!(
        data.variable("age").unwrap().label(),
        Some("Vanus aastates")
    );
    data.set_variable_label("age", None).unwrap();
    assert_eq!(data.variable("age").unwrap().label(), None);
    data.set_variable_label("gender", Some("")).unwrap();
    assert_eq!(data.variable("gender").unwrap().label(), None);
    assert_error!(
        data.set_variable_label("nope", None),
        Error::UnknownVariable(_),
        "unknown variable: nope"
    );
}

#[test]
fn value_labels() {
    let mut data = survey();
    let labels = vec![
        (Value::Number(2.0), "Two".to_string()),
        (Value::Number(1.0), "One".to_string()),
        (Value::Number(2.0), "Second".to_string()),
    ];
    data.set_value_labels("gender", labels).unwrap();
    let expected = [
        (Value::Number(2.0), "Second".to_string()),
        (Value::Number(1.0), "One".to_string()),
    ];
    assert_eq!(data.variable("gender").unwrap().value_labels(), expected);
    data.set_value_labels("gender", Vec::new()).unwrap();
    assert!(data.variable("gender").unwrap().value_labels().is_empty());

    data.set_value_labels("plan", vec![(text("PRO   "), "Pro plan".to_string())])
        .unwrap();
    assert_eq!(
        data.variable("plan").unwrap().value_labels()[0].0,
        text("PRO")
    );

    let one = |value: Value| vec![(value, "x".to_string())];
    assert_error!(
        data.set_value_labels("plan", one(Value::Number(1.0))),
        Error::WrongType { .. },
        "plan is not a numeric variable"
    );
    assert_error!(
        data.set_value_labels("gender", one(text("m"))),
        Error::WrongType { .. },
        "gender is not a string variable"
    );
    assert_error!(
        data.set_value_labels("plan", one(text("ENTERPRISES"))),
        Error::Invalid(_),
        "plan: value longer than 10 bytes: ENTERPRISES"
    );
    // a value label has a one-byte length in the file
    let longest = vec![(Value::Number(1.0), "x".repeat(255))];
    data.set_value_labels("gender", longest).unwrap();
    let long = vec![(Value::Number(1.0), "x".repeat(256))];
    assert!(matches!(
        data.set_value_labels("gender", long),
        Err(Error::Invalid(_))
    ));
    assert_error!(
        data.set_value_labels("nope", Vec::new()),
        Error::UnknownVariable(_),
        "unknown variable: nope"
    );
    // a refused request changes nothing
    assert_eq!(data.variable("plan").unwrap().value_labels().len(), 1);
}

#[test]
fn missing_values() {
    let mut data = survey();
    let user_missing = |data: &Dataset, name: &str| {
        let index = data.variable_index(name).unwrap();
        data.describe().variables[index].n_user_missing
    };

    data.set_missing_values("age", MissingValues::Numbers(vec![49.0, 1.0, 2.0]))
        .unwrap();
    assert!(data.variable("age").unwrap().is_user_missing(49.0));
    assert!(user_missing(&data, "age") > 0);
    data.set_missing_values("age", MissingValues::None).unwrap();
    assert_eq!(user_missing(&data, "age"), 0);
    data.set_missing_values("age", MissingValues::Numbers(Vec::new()))
        .unwrap();
    assert_eq!(
        data.variable("age").unwrap().missing_values(),
        &MissingValues::None
    );

    let range = MissingValues::Range {
        low: 60.0,
        high: 200.0,
        extra: Some(18.0),
    };
    data.set_missing_values("age", range.clone()).unwrap();
    let age = data.variable("age").unwrap();
    assert_eq!(age.missing_values(), &range);
    assert!(age.is_user_missing(60.0) && age.is_user_missing(18.0) && !age.is_user_missing(59.9));

    // a date is a number like any other: the first case signed up on 2023-11-01
    data.set_missing_values(
        "signup_date",
        MissingValues::Numbers(vec![13_918_176_000.0]),
    )
    .unwrap();
    assert!(user_missing(&data, "signup_date") > 0);

    data.set_missing_values("plan", MissingValues::Strings(vec!["BASIC  ".to_string()]))
        .unwrap();
    let plan = data.variable("plan").unwrap();
    assert_eq!(
        plan.missing_values(),
        &MissingValues::Strings(vec!["BASIC".to_string()])
    );
    assert!(plan.is_user_missing_str("BASIC") && plan.is_user_missing_str("BASIC   "));
    assert!(!plan.is_user_missing_str("PRO") && !plan.is_user_missing(1.0));
    assert!(user_missing(&data, "plan") > 0);

    let four = MissingValues::Numbers(vec![1.0, 2.0, 3.0, 4.0]);
    assert_error!(
        data.set_missing_values("age", four),
        Error::Invalid(_),
        "age: more than three missing values"
    );
    let reversed = MissingValues::Range {
        low: 9.0,
        high: 1.0,
        extra: None,
    };
    assert_error!(
        data.set_missing_values("age", reversed),
        Error::Invalid(_),
        "age: a missing range needs low <= high"
    );
    let not_a_number = MissingValues::Numbers(vec![f64::NAN]);
    assert!(matches!(
        data.set_missing_values("age", not_a_number),
        Err(Error::Invalid(_))
    ));
    let strings =
        |values: &[&str]| MissingValues::Strings(values.iter().map(|s| s.to_string()).collect());
    assert_error!(
        data.set_missing_values("age", strings(&["x"])),
        Error::WrongType { .. },
        "age is not a string variable"
    );
    assert_error!(
        data.set_missing_values("plan", MissingValues::Numbers(vec![1.0])),
        Error::WrongType { .. },
        "plan is not a numeric variable"
    );
    assert_error!(
        data.set_missing_values("plan", range),
        Error::WrongType { .. },
        "plan is not a numeric variable"
    );
    assert_error!(
        data.set_missing_values("plan", strings(&["a", "b", "c", "d"])),
        Error::Invalid(_),
        "plan: more than three missing values"
    );
    assert_error!(
        data.set_missing_values("plan", strings(&["123456789"])),
        Error::Invalid(_),
        "plan: value longer than 8 bytes: 123456789"
    );
    assert_error!(
        data.set_missing_values("nope", MissingValues::None),
        Error::UnknownVariable(_),
        "unknown variable: nope"
    );
    assert_eq!(
        data.variable("plan").unwrap().missing_values(),
        &strings(&["BASIC"])
    );
}

#[test]
fn measure_and_format() {
    let mut data = survey();
    data.set_measure("age", Measure::Nominal).unwrap();
    assert_eq!(data.variable("age").unwrap().measure(), Measure::Nominal);
    assert_error!(
        data.set_measure("nope", Measure::Scale),
        Error::UnknownVariable(_),
        "unknown variable: nope"
    );

    for (given, stored) in [
        ("f8.1", "F8.1"),
        ("F5", "F5.0"),
        ("comma12.2", "COMMA12.2"),
        ("date11", "DATE11"),
        ("DATETIME23.2", "DATETIME23.2"),
        ("E16.8", "E16.8"),
    ] {
        data.set_format("age", given).unwrap();
        assert_eq!(data.variable("age").unwrap().format(), stored);
    }
    data.set_format("plan", "a10").unwrap();
    assert_eq!(data.variable("plan").unwrap().format(), "A10");

    for invalid in ["", "F", "XYZ5", "F0.0", "F99.2", "F8.20", "F8.x", "8.2"] {
        let result = data.set_format("age", invalid);
        assert!(
            matches!(result, Err(Error::Invalid(_))),
            "{invalid}: {result:?}"
        );
    }
    assert_error!(
        data.set_format("age", "F99.2"),
        Error::Invalid(_),
        "invalid format: F99.2"
    );
    assert_error!(
        data.set_format("age", "A8"),
        Error::WrongType { .. },
        "age is not a string variable"
    );
    assert_error!(
        data.set_format("plan", "F8.2"),
        Error::WrongType { .. },
        "plan is not a numeric variable"
    );
    assert_error!(
        data.set_format("plan", "A20"),
        Error::Invalid(_),
        "plan has width 10, format A20 does not fit"
    );
    assert_error!(
        data.set_format("nope", "F8.2"),
        Error::UnknownVariable(_),
        "unknown variable: nope"
    );
    assert_eq!(data.variable("age").unwrap().format(), "E16.8");
}

#[test]
fn numeric_values() {
    let mut data = survey();
    let doubled: Vec<Option<f64>> = (0..200)
        .map(|i| (i != 3).then_some(i as f64 * 2.0))
        .collect();

    data.set_numeric_values("double", doubled.clone()).unwrap();
    assert_eq!(names(&data).last(), Some(&"double"));
    assert_eq!(data.variable_count(), 20);
    let new = data.variable("double").unwrap();
    assert_eq!(
        (new.format(), new.measure(), new.label()),
        ("F8.2", Measure::Unknown, None)
    );
    assert_eq!(new.numeric().unwrap().iter().collect::<Vec<_>>(), doubled);
    assert_eq!(data.describe().variables[19].n_system_missing, 1);

    // replacing values keeps the dictionary entry and the position
    data.set_numeric_values("AGE", vec![None; 200]).unwrap();
    let age = data.variable_at(2);
    assert_eq!((age.name(), age.format()), ("age", "F3.0"));
    assert!(age.label().is_some());
    assert_eq!(age.numeric().unwrap().get(0), None);
    assert_eq!(data.variable_count(), 20);

    assert_error!(
        data.set_numeric_values("age", vec![None; 199]),
        Error::LengthMismatch { .. },
        "expected 200 values, got 199"
    );
    assert_error!(
        data.set_numeric_values("plan", vec![None; 200]),
        Error::WrongType { .. },
        "plan is not a numeric variable"
    );
    let too_long = "x".repeat(65);
    for invalid in [
        "",
        "1abc",
        "a b",
        "all",
        "x.",
        "$sys",
        "a-b",
        too_long.as_str(),
    ] {
        let result = data.set_numeric_values(invalid, vec![None; 200]);
        assert!(
            matches!(result, Err(Error::InvalidName(_))),
            "{invalid}: {result:?}"
        );
    }
    // a scratch variable may live in a dataset, never in a file
    let mut scratch = data.clone();
    scratch.set_numeric_values("#tmp", vec![None; 200]).unwrap();
    let file = std::env::temp_dir().join(format!("statscraft-scratch-{}.sav", std::process::id()));
    let refused = scratch.write_sav(&file).unwrap_err().to_string();
    assert_eq!(
        refused,
        "#tmp is a scratch variable, which a data file cannot hold"
    );
    assert_error!(
        data.set_numeric_values("a b", vec![None; 200]),
        Error::InvalidName(_),
        "invalid variable name: a b"
    );
    assert_eq!(data.variable_count(), 20);
    for valid in ["õun_2", "@x", "a.b", "コメント", "x".repeat(64).as_str()] {
        data.set_numeric_values(valid, vec![None; 200]).unwrap();
    }
}

#[test]
fn string_values() {
    let mut data = survey();
    let codes: Vec<String> = (0..200).map(|i| format!("k{}  ", i % 10)).collect();

    data.set_string_values("code", 4, codes).unwrap();
    let new = data.variable("code").unwrap();
    assert_eq!((new.format(), new.string_width()), ("A4", Some(4)));
    assert_eq!((new.measure(), new.label()), (Measure::Nominal, None));
    assert_eq!(new.string().unwrap().get(13), "k3");

    data.set_string_values("PLAN", 10, vec!["Ülikõrge".to_string(); 200])
        .unwrap();
    let plan = data.variable_at(7);
    assert_eq!(
        (plan.name(), plan.string().unwrap().get(199)),
        ("plan", "Ülikõrge")
    );
    assert!(plan.label().is_some());

    let blank = |n: usize| vec![String::new(); n];
    assert_error!(
        data.set_string_values("plan", 10, blank(3)),
        Error::LengthMismatch { .. },
        "expected 200 values, got 3"
    );
    assert_error!(
        data.set_string_values("plan", 12, blank(200)),
        Error::Invalid(_),
        "plan has width 10, not 12"
    );
    assert_error!(
        data.set_string_values("age", 3, blank(200)),
        Error::WrongType { .. },
        "age is not a string variable"
    );
    // width counts bytes: "õõõ" needs 6
    let wide = vec!["õõõ".to_string(); 200];
    assert_error!(
        data.set_string_values("short", 5, wide),
        Error::Invalid(_),
        "short: value does not fit in 5 bytes: õõõ"
    );
    assert_error!(
        data.set_string_values("zero", 0, blank(200)),
        Error::Invalid(_),
        "string width must be 1 to 32767, got 0"
    );
    assert!(matches!(
        data.set_string_values("huge", 32768, blank(200)),
        Err(Error::Invalid(_))
    ));
    assert_error!(
        data.set_string_values("1x", 5, blank(200)),
        Error::InvalidName(_),
        "invalid variable name: 1x"
    );
    assert_eq!(data.variable_count(), 20);
}

#[test]
fn select_cases() {
    let mut data = survey();
    let ids: Vec<Option<f64>> = data
        .variable("resp_id")
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect();
    let keep: Vec<bool> = (0..200).map(|i| i % 10 == 9).collect();

    data.select_cases(&keep).unwrap();
    assert_eq!((data.case_count(), data.variable_count()), (20, 19));
    let kept: Vec<Option<f64>> = data
        .variable("resp_id")
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect();
    assert_eq!(
        kept,
        ids.iter().copied().skip(9).step_by(10).collect::<Vec<_>>()
    );
    assert_eq!(
        data.variable("plan")
            .unwrap()
            .string()
            .unwrap()
            .iter()
            .count(),
        20
    );
    assert_eq!(data.describe().case_count, 20);

    assert_error!(
        data.select_cases(&keep),
        Error::LengthMismatch { .. },
        "expected 20 values, got 200"
    );
    data.select_cases(&[false; 20]).unwrap();
    assert_eq!(data.case_count(), 0);
    data.set_numeric_values("empty", Vec::new()).unwrap();
}

#[test]
fn reorder_cases() {
    let mut data = survey();
    let before = common::cases(&data);
    let reversed: Vec<usize> = (0..200).rev().collect();

    data.reorder_cases(&reversed).unwrap();
    assert_eq!((data.case_count(), data.variable_count()), (200, 19));
    let ids = data.variable("resp_id").unwrap().numeric().unwrap();
    assert_eq!((ids.get(0), ids.get(199)), (Some(1200.0), Some(1001.0)));
    let plans = data.variable("plan").unwrap().string().unwrap();
    assert_eq!((plans.get(199), plans.get(198)), ("BASIC", "PREMIUM"));
    // every column moved together: reversing again restores the file
    data.reorder_cases(&reversed).unwrap();
    assert_eq!(common::cases(&data), before);
    common::assert_survives_write(&data, "reordered.sav");

    let mut repeated = reversed.clone();
    repeated[0] = 5;
    assert_error!(
        data.reorder_cases(&repeated),
        Error::Invalid(_),
        "case order must name every case once: 5 is repeated or out of range"
    );
    repeated[0] = 200;
    assert_error!(
        data.reorder_cases(&repeated),
        Error::Invalid(_),
        "case order must name every case once: 200 is repeated or out of range"
    );
    assert_error!(
        data.reorder_cases(&reversed[..10]),
        Error::LengthMismatch { .. },
        "expected 200 values, got 10"
    );
    assert_eq!(common::cases(&data), before);
}

#[test]
fn rename_variable() {
    let mut data = survey();
    data.rename_variable("Q5_2", "kanal_telefon").unwrap();
    data.rename_variable("weight", "kaal").unwrap();
    assert_eq!(data.variable_index("kanal_telefon"), Some(11));
    assert_eq!(data.variable_index("q5_2"), None);
    assert_eq!(
        data.multiple_response_sets()[0].variables[1],
        "kanal_telefon"
    );
    assert_eq!(data.weight_variable(), Some("kaal"));
    // only the case changes: not a duplicate of itself
    data.rename_variable("kaal", "KAAL").unwrap();
    assert_eq!(data.weight_variable(), Some("KAAL"));

    assert_error!(
        data.rename_variable("age", "Gender"),
        Error::DuplicateName(_),
        "variable name already in use: Gender"
    );
    assert_error!(
        data.rename_variable("age", "2nd"),
        Error::InvalidName(_),
        "invalid variable name: 2nd"
    );
    assert_error!(
        data.rename_variable("nope", "x"),
        Error::UnknownVariable(_),
        "unknown variable: nope"
    );
    assert_eq!(names(&data)[2], "age");
}

#[test]
fn keep_variables() {
    let mut data = survey();
    data.keep_variables(&["Q5_2", "plan", "gender", "q5_1"])
        .unwrap();
    assert_eq!(names(&data), ["q5_2", "plan", "gender", "q5_1"]);
    assert_eq!(data.case_count(), 200);
    // columns moved with their dictionary entries
    let plan = data.variable("plan").unwrap();
    assert_eq!(plan.string().unwrap().get(1), "PREMIUM");
    assert_eq!(plan.string_width(), Some(10));
    let gender = data.variable("gender").unwrap();
    assert_eq!(gender.numeric().unwrap().get(3), Some(3.0));
    assert_eq!(gender.value_labels().len(), 3);
    assert_eq!(data.multiple_response_sets().len(), 1);
    assert_eq!(data.multiple_response_sets()[0].variables, ["q5_1", "q5_2"]);
    assert_eq!(data.weight_variable(), None);
    common::assert_survives_write(&data, "kept.sav");

    assert_error!(
        data.keep_variables(&["plan", "nope"]),
        Error::UnknownVariable(_),
        "unknown variable: nope"
    );
    assert_error!(
        data.keep_variables(&["plan", "PLAN"]),
        Error::DuplicateName(_),
        "variable name already in use: PLAN"
    );
    assert_error!(
        data.keep_variables(&[]),
        Error::Invalid(_),
        "cannot delete every variable"
    );
    assert_eq!(data.variable_count(), 4);
}

#[test]
fn delete_variables() {
    let mut data = survey();
    data.delete_variables(&["Q5_1", "weight", "plan"]).unwrap();
    assert_eq!(data.variable_count(), 16);
    assert!(
        !names(&data)
            .iter()
            .any(|name| ["q5_1", "weight", "plan"].contains(name))
    );
    assert_eq!(
        data.multiple_response_sets()[0].variables,
        ["q5_2", "q5_3", "q5_4", "q5_5"]
    );
    assert_eq!(data.weight_variable(), None);
    // columns moved with their dictionary entries
    assert_eq!(
        data.variable("q3_comment")
            .unwrap()
            .string()
            .unwrap()
            .iter()
            .count(),
        200
    );
    assert_eq!(
        data.variable("signup_date")
            .unwrap()
            .numeric()
            .unwrap()
            .get(0),
        Some(13_918_176_000.0)
    );

    // a set that loses its last variable goes away
    data.delete_variables(&["q6_mention1", "q6_mention2", "q6_mention3", "q6_mention3"])
        .unwrap();
    let sets: Vec<&str> = data
        .multiple_response_sets()
        .iter()
        .map(|set| set.name.as_str())
        .collect();
    assert_eq!(sets, ["$q5"]);

    assert_error!(
        data.delete_variables(&["age", "nope"]),
        Error::UnknownVariable(_),
        "unknown variable: nope"
    );
    assert_eq!(data.variable_count(), 13);
    let all = names(&data)
        .iter()
        .map(|name| name.to_string())
        .collect::<Vec<_>>();
    let all: Vec<&str> = all.iter().map(String::as_str).collect();
    assert_error!(
        data.delete_variables(&all),
        Error::Invalid(_),
        "cannot delete every variable"
    );
    assert_eq!(data.variable_count(), 13);
}

#[test]
fn weight_variable() {
    let mut data = survey();
    data.set_weight_variable(None).unwrap();
    assert_eq!(data.weight_variable(), None);
    data.set_weight_variable(Some("MONTHLY_SPEND")).unwrap();
    assert_eq!(data.weight_variable(), Some("monthly_spend"));
    assert_eq!(
        data.describe().weight_variable.as_deref(),
        Some("monthly_spend")
    );

    assert_error!(
        data.set_weight_variable(Some("plan")),
        Error::WrongType { .. },
        "plan is not a numeric variable"
    );
    assert_error!(
        data.set_weight_variable(Some("nope")),
        Error::UnknownVariable(_),
        "unknown variable: nope"
    );
    assert_eq!(data.weight_variable(), Some("monthly_spend"));
}

fn set(
    name: &str,
    kind: &'static str,
    counted: Option<&str>,
    variables: &[&str],
) -> MultipleResponseSet {
    MultipleResponseSet {
        name: name.into(),
        kind,
        label: "Label".into(),
        counted_value: counted.map(str::to_string),
        variables: variables.iter().map(|v| v.to_string()).collect(),
        ..Default::default()
    }
}

#[test]
fn multiple_response_sets_are_added_replaced_and_deleted() {
    let mut data = fixture("survey_basic.sav");
    assert!(data.multiple_response_sets().is_empty());
    data.set_multiple_response_set(set("$ch", "dichotomy", Some("1.0"), &["Q5_1", "q5_2"]))
        .unwrap();
    data.set_multiple_response_set(set(
        "$prov",
        "category",
        None,
        &["q6_mention1", "q6_mention2"],
    ))
    .unwrap();
    let sets = data.multiple_response_sets();
    // names as the dictionary spells them, the counted value as a plain whole number
    assert_eq!(sets[0].variables, ["q5_1", "q5_2"]);
    assert_eq!(sets[0].counted_value.as_deref(), Some("1"));
    assert_eq!(sets[1].counted_value, None);

    // the same name in other case replaces the set in its place
    data.set_multiple_response_set(set(
        "$CH",
        "dichotomy",
        Some("0"),
        &["q5_3", "q5_4", "q5_5"],
    ))
    .unwrap();
    let sets = data.multiple_response_sets();
    assert_eq!((sets.len(), sets[0].name.as_str()), (2, "$CH"));
    assert_eq!(sets[0].variables, ["q5_3", "q5_4", "q5_5"]);

    let copy = common::assert_survives_write(&data, "sets.sav");
    assert_eq!(copy.multiple_response_sets(), data.multiple_response_sets());

    assert_error!(
        data.delete_multiple_response_sets(&["$prov", "$nope"]),
        Error::Invalid(_),
        "unknown multiple response set $nope"
    );
    assert_eq!(data.multiple_response_sets().len(), 2);
    data.delete_multiple_response_sets(&["$PROV"]).unwrap();
    assert_eq!(data.multiple_response_sets()[0].name, "$CH");
}

#[test]
fn a_string_set_counts_a_text_that_fits_the_narrowest_variable() {
    let mut data = fixture("survey_basic.sav");
    data.set_string_values("short", 3, vec!["yes".into(); 200])
        .unwrap();
    data.set_multiple_response_set(set("$s", "dichotomy", Some("yes  "), &["plan", "short"]))
        .unwrap();
    assert_eq!(
        data.multiple_response_sets()[0].counted_value.as_deref(),
        Some("yes")
    );
    assert_error!(
        data.set_multiple_response_set(set("$s", "dichotomy", Some("four"), &["plan", "short"])),
        Error::Invalid(_),
        "$s: the counted value 'four' is wider than 3 bytes, the width of the narrowest variable"
    );
}

#[test]
fn invalid_multiple_response_sets_change_nothing() {
    let mut data = fixture("survey_basic.sav");
    for (bad, message) in [
        (
            set("q5", "dichotomy", Some("1"), &["q5_1", "q5_2"]),
            "invalid name of a multiple response set: q5 (a set name is $ followed by a variable name)",
        ),
        (
            set("$1x", "category", None, &["q5_1", "q5_2"]),
            "invalid name of a multiple response set: $1x (a set name is $ followed by a variable name)",
        ),
        (
            set("$x", "category", None, &["q5_1"]),
            "$x: a set needs at least two variables",
        ),
        (
            set("$x", "category", None, &["q5_1", "Q5_1"]),
            "$x: q5_1 is listed twice",
        ),
        (
            set("$x", "category", None, &["q5_1", "plan"]),
            "$x: q5_1 and plan differ in type; the variables of a set are all numeric or all strings",
        ),
        (
            set("$x", "dichotomy", None, &["q5_1", "q5_2"]),
            "$x: a dichotomy set needs a counted value",
        ),
        (
            set("$x", "category", Some("1"), &["q5_1", "q5_2"]),
            "$x: a category set has no counted value",
        ),
        (
            set("$x", "dichotomy", Some("1.5"), &["q5_1", "q5_2"]),
            "$x: the counted value of numeric variables must be a whole number, not 1.5",
        ),
        (
            set("$x", "dichotomy", Some("yes"), &["q5_1", "q5_2"]),
            "$x: the counted value of numeric variables must be a whole number, not yes",
        ),
    ] {
        assert_error!(
            data.set_multiple_response_set(bad),
            Error::Invalid(_),
            message
        );
    }
    assert_error!(
        data.set_multiple_response_set(set("$x", "category", None, &["q5_1", "nope"])),
        Error::UnknownVariable(_),
        "unknown variable: nope"
    );
    assert!(data.multiple_response_sets().is_empty());
}
