//! Writing `.sav` / `.zsav` files: what was read must come back unchanged.

mod common;

use common::{assert_survives_write, file_name, fixture, fixture_files, scratch};
use statscraft_data::{Dataset, Error, Measure, MissingValues, Value};

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
