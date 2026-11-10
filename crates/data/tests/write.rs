//! Writing `.sav` / `.zsav` files: what was read must come back unchanged.

mod common;

use common::{assert_survives_write, file_name, fixture, fixture_files, scratch};
use statscraft_data::{Dataset, Error, Measure, MissingValues, Value, VariableSet};

#[test]
fn every_fixture_survives_a_write_read_cycle() {
    for path in fixture_files() {
        let original = Dataset::read_sav(&path).unwrap();
        for extension in ["sav", "zsav"] {
            let file = format!("{}.roundtrip.{extension}", file_name(&path));
            assert_survives_write(&original, &file);
        }
    }
}

#[test]
fn an_edited_dataset_survives_a_write_read_cycle() {
    let mut data = fixture("survey_basic_phpwriter.sav");
    let n = data.case_count();
    let numbers = |f: fn(usize) -> Option<f64>| (0..n).map(f).collect::<Vec<_>>();

    let score = numbers(|i| (i % 7 != 0).then_some(i as f64 / 8.0));
    data.set_numeric_values("score", score).unwrap();
    data.set_variable_label("score", Some("Skoor (õäöü) 😀"))
        .unwrap();
    let range = MissingValues::Range {
        low: 20.0,
        high: 30.0,
        extra: Some(-1.0),
    };
    data.set_missing_values("score", range).unwrap();
    data.set_format("score", "comma12.3").unwrap();
    data.set_measure("score", Measure::Ordinal).unwrap();

    // half seconds in a datetime and microseconds in a duration are exact in the file
    let seen_at = numbers(|i| Some(13_918_176_000.5 + i as f64));
    data.set_numeric_values("seen_at", seen_at).unwrap();
    data.set_format("seen_at", "DATETIME20").unwrap();
    let took = numbers(|i| Some(3600.25 + i as f64 * 0.000001));
    data.set_numeric_values("took", took).unwrap();
    data.set_format("took", "TIME12").unwrap();
    let first_day = MissingValues::Numbers(vec![13_918_176_000.0]);
    data.set_missing_values("signup_date", first_day).unwrap();

    // widths around the 8 byte and 255 byte borders of the file format
    for width in [1, 8, 9, 255, 256, 300, 1000, 5000] {
        let values = (0..n).map(|i| match i % 3 {
            0 => "õ".repeat(width / 2),
            1 => "x".repeat(width),
            _ => String::new(),
        });
        data.set_string_values(&format!("text{width}"), width, values.collect())
            .unwrap();
    }
    let all_x = vec![(Value::String("xxxxxxxx".into()), "Kõik x".into())];
    data.set_value_labels("text8", all_x).unwrap();
    let nine_x = vec![(Value::String("xxxxxxxxx".into()), "Nine".into())];
    data.set_value_labels("text9", nine_x).unwrap();
    data.set_missing_values("text8", MissingValues::Strings(vec!["õõõõ".into()]))
        .unwrap();
    data.set_missing_values("text300", MissingValues::Strings(vec!["none".into()]))
        .unwrap();

    data.rename_variable("q5_1", "kanal_e_post").unwrap();
    data.delete_variables(&["q6_mention2", "age"]).unwrap();
    data.set_weight_variable(Some("score")).unwrap();
    let keep: Vec<bool> = (0..n).map(|i| i % 4 != 1).collect();
    data.select_cases(&keep).unwrap();

    let copy = assert_survives_write(&data, "edited.sav");
    assert_eq!(copy.weight_variable(), Some("score"));
    assert_eq!(
        copy.multiple_response_sets()[0].variables,
        ["kanal_e_post", "q5_2", "q5_3", "q5_4", "q5_5"]
    );
    assert_eq!(copy.variable("signup_date").unwrap().format(), "DATE11");
    assert_survives_write(&data, "edited.zsav");
}

#[test]
fn a_clone_keeps_the_state_before_an_edit() {
    let original = fixture("survey_basic.sav");
    let mut edited = original.clone();
    edited.delete_variables(&["age"]).unwrap();
    edited.set_variable_label("gender", None).unwrap();
    edited.select_cases(&[false; 200]).unwrap();

    assert_eq!(
        (original.case_count(), original.variable_count()),
        (200, 19)
    );
    assert!(original.variable("gender").unwrap().label().is_some());
    assert_eq!((edited.case_count(), edited.variable_count()), (0, 18));
    assert_survives_write(&edited, "no_cases.sav");
}

/// Date and time variables are written and read as the stored numbers, so every value comes
/// back bit for bit: a time of day in a date variable, fractions of a second, dates outside the
/// years 1 to 9999 and durations of any length.
#[test]
fn date_and_time_values_survive_exactly() {
    let mut data = fixture("survey_basic.sav");
    for (format, value) in [
        ("DATE11", 13_918_176_000.0 + 12.0 * 3600.0),
        ("DATE11", -1.0e11),
        ("DATE11", 0.5),
        ("ADATE10", 1.0e15),
        ("TIME12", 1.001),
        ("TIME12", 1.0 / 3.0),
        ("TIME12", 1.0e13),
        ("DTIME14.3", -0.000_001),
        ("DATETIME20", 13_918_176_000.094),
        ("DATETIME23.2", 1.0e12 + 0.25),
        ("TIME12", f64::NAN),
    ] {
        data.set_numeric_values("x", vec![Some(value); 200])
            .unwrap();
        data.set_format("x", format).unwrap();
        let copy = assert_survives_write(&data, "dates.sav");
        assert_eq!(copy.variable("x").unwrap().format(), format);
    }
}

#[test]
fn a_failed_write_keeps_the_existing_file() {
    let mut data = fixture("survey_basic.sav");
    let path = scratch("kept.sav");
    data.write_sav(&path).unwrap();
    let before = std::fs::read(&path).unwrap();

    // the partial file cannot be created where a directory of that name is in the way
    let partial = scratch(&format!("kept.sav.{}.partial", std::process::id()));
    std::fs::create_dir_all(&partial).unwrap();
    data.delete_variables(&["age"]).unwrap();
    assert!(data.write_sav(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    std::fs::remove_dir(&partial).unwrap();

    let unwritable = scratch("no_such_directory/x.sav");
    let error = fixture("empty.sav").write_sav(unwritable).unwrap_err();
    assert!(matches!(error, Error::File(_)), "{error}");
}

/// A value label has a one-byte length in the file, so it holds up to 255 bytes; labels of
/// long string variables (their own record) keep the same limit.
#[test]
fn value_labels_up_to_255_bytes_survive() {
    let mut data = fixture("survey_basic.sav");
    let longest = "õ".repeat(127) + "x"; // 255 bytes of UTF-8
    data.set_value_labels("gender", vec![(Value::Number(1.0), longest.clone())])
        .unwrap();
    let text = vec![(Value::String("ENTERPRISE".into()), longest.clone())];
    data.set_value_labels("plan", text).unwrap();
    let copy = assert_survives_write(&data, "long_value_labels.sav");
    assert_eq!(
        copy.variable("gender").unwrap().value_labels()[0].1,
        longest
    );
    assert_eq!(copy.variable("plan").unwrap().value_labels()[0].1, longest);
}

/// The file label has a fixed 64-byte field in the header and no other record, so 64 bytes
/// is the limit: a longer label is refused when writing instead of being cut.
#[test]
fn the_file_label_holds_64_bytes() {
    let mut data = fixture("survey_basic.sav");
    let label = "ä".repeat(32);
    data.set_file_label(Some(&label));
    let copy = assert_survives_write(&data, "file_label.sav");
    assert_eq!(copy.file_label(), Some(label.as_str()));

    data.set_file_label(Some(&format!("{label}x")));
    let path = scratch("file_label_too_long.sav");
    let error = data.write_sav(&path).unwrap_err();
    assert!(error.to_string().contains("at most 64"), "{error}");
    assert!(!path.exists());
}

/// CATEGORYLABELS=COUNTEDVALUES and LABELSOURCE=VARLABEL are kept in the file. Such sets have
/// their own record after the one of the other sets, so they come back after those.
#[test]
fn multiple_response_label_settings_survive() {
    let mut data = fixture("survey_basic_phpwriter.sav");
    let mut set = data.multiple_response_sets()[0].clone();
    set.name = "$counted".into();
    set.counted_value_labels = true;
    set.label_from_variables = true;
    data.set_multiple_response_set(set.clone()).unwrap();
    let mut plain = set.clone();
    plain.name = "$plain".into();
    plain.label_from_variables = false;
    data.set_multiple_response_set(plain).unwrap();
    let copy = assert_survives_write(&data, "mr_labels.sav");
    assert_eq!(copy.multiple_response_sets()[2], set);
    assert_survives_write(&data, "mr_labels.zsav");

    // only a dichotomy set has counted values, and a label from the variables needs them
    let mut wrong = data.multiple_response_sets()[1].clone();
    wrong.counted_value_labels = true;
    assert!(data.set_multiple_response_set(wrong).is_err());
    set.counted_value_labels = false;
    assert!(data.set_multiple_response_set(set).is_err());
}

/// Variable sets and data file attributes are kept, and follow renamed and deleted variables.
#[test]
fn variable_sets_and_file_attributes_survive() {
    let mut data = fixture("survey_basic.sav");
    let sets = vec![
        VariableSet {
            name: "demographics".into(),
            variables: vec!["AGE".into(), "gender".into()],
        },
        VariableSet {
            name: "channels".into(),
            variables: vec!["q5_1".into(), "q5_2".into()],
        },
    ];
    data.set_variable_sets(sets).unwrap();
    let attributes = vec![
        ("origin".to_string(), vec!["synthetic".to_string()]),
        (
            "waves".to_string(),
            vec!["1".to_string(), "2 õ".to_string()],
        ),
    ];
    data.set_attributes(attributes.clone());
    data.rename_variable("q5_1", "phone").unwrap();
    data.delete_variables(&["q5_2"]).unwrap();
    let copy = assert_survives_write(&data, "variable_sets.sav");
    assert_eq!(copy.variable_sets(), data.variable_sets());
    assert_eq!(copy.variable_sets()[0].variables, ["age", "gender"]);
    assert_eq!(copy.variable_sets()[1].variables, ["phone"]);
    assert_eq!(copy.attributes(), attributes);

    let unknown = vec![VariableSet {
        name: "x".into(),
        variables: vec!["nope".into()],
    }];
    assert!(data.set_variable_sets(unknown).is_err());
}

/// A file is written in the encoding of the dataset: the one it was read in, or the one
/// `set_encoding` chose. Text that encoding cannot hold is refused.
#[test]
fn a_file_is_written_in_the_chosen_encoding() {
    let mut data = fixture("survey_basic.sav");
    assert_eq!(data.encoding(), "UTF-8");
    data.set_variable_label("age", Some("Vanus (õäöü šž)"))
        .unwrap();
    // 10 characters, 20 bytes of UTF-8, 10 bytes of windows-1257
    let text: Vec<String> = (0..200).map(|_| "õ".repeat(10)).collect();
    data.set_string_values("baltic", 20, text).unwrap();
    data.set_encoding("1257").unwrap();
    assert_eq!(data.encoding(), "windows-1257");
    let copy = assert_survives_write(&data, "baltic.sav");
    assert_eq!(copy.encoding(), "windows-1257");

    // the copy keeps its encoding: written again, it is still windows-1257
    let again = assert_survives_write(&copy, "baltic_again.zsav");
    assert_eq!(again.encoding(), "windows-1257");

    // and written as UTF-8 when asked
    let mut utf8 = copy.clone();
    utf8.set_encoding("utf8").unwrap();
    assert_eq!(
        assert_survives_write(&utf8, "baltic_utf8.sav").encoding(),
        "UTF-8"
    );

    let mut cyrillic = data.clone();
    cyrillic.set_variable_label("age", Some("Возраст")).unwrap();
    let path = scratch("cyrillic_in_1257.sav");
    let error = cyrillic.write_sav(&path).unwrap_err();
    assert!(
        error.to_string().contains("windows-1257 cannot hold"),
        "{error}"
    );
    assert!(!path.exists());
    assert!(data.set_encoding("UTF-16").is_err());
    assert!(data.set_encoding("no such encoding").is_err());
}
