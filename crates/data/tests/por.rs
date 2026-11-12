//! Portable files: what is written comes back, within what the format holds.

mod common;

use common::{cases, fixture_files, scratch};
use statscraft_data::{Dataset, MissingValues, Value, portable_names};

/// LO and HI of a missing range, whatever exact number stands for them.
fn same_missing(a: &MissingValues, b: &MissingValues) -> bool {
    let bound = |v: f64| v.clamp(-1e308, 1e308);
    match (a, b) {
        (
            MissingValues::Range { low, high, extra },
            MissingValues::Range {
                low: l,
                high: h,
                extra: e,
            },
        ) => bound(*low) == bound(*l) && bound(*high) == bound(*h) && extra == e,
        _ => a == b,
    }
}

#[test]
fn every_fixture_survives_a_portable_file() {
    for path in fixture_files() {
        let original = Dataset::read_sav(&path).unwrap();
        if original.variable_names().any(|n| n.starts_with('#')) {
            continue;
        }
        let file = scratch(&format!(
            "{}.por",
            path.file_name().unwrap().to_string_lossy()
        ));
        original.write_por(&file, None).unwrap();
        let copy = Dataset::read_por(&file).unwrap();
        let shown = path.display();
        assert_eq!(copy.case_count(), original.case_count(), "{shown}");
        let names = portable_names(original.variable_names());
        assert_eq!(copy.variable_names().collect::<Vec<_>>(), names, "{shown}");
        for ((before, after), (_, numbers, strings)) in
            original.variables().zip(copy.variables()).zip(cases(&copy))
        {
            let what = format!("{shown}: {}", before.name());
            assert_eq!(after.label(), before.label(), "{what}");
            assert!(
                same_missing(after.missing_values(), before.missing_values()),
                "{what}"
            );
            match before.string_width() {
                Some(width) => {
                    let width = width.min(255);
                    assert_eq!(after.string_width(), Some(width), "{what}");
                    let cut: Vec<String> = cases(&original)
                        .into_iter()
                        .find(|(n, _, _)| n == before.name())
                        .unwrap()
                        .2
                        .iter()
                        .map(|s| statscraft_data::cut_to(s, width).trim_end().to_string())
                        .collect();
                    assert_eq!(strings, cut, "{what}");
                }
                None => {
                    assert_eq!(after.format(), before.format(), "{what}");
                    assert_eq!(after.write_format(), before.write_format(), "{what}");
                    let original_bits = cases(&original)
                        .into_iter()
                        .find(|(n, _, _)| n == before.name())
                        .unwrap()
                        .1;
                    assert_eq!(numbers, original_bits, "{what}");
                    assert_eq!(after.value_labels(), before.value_labels(), "{what}");
                }
            }
        }
        let weight = |data: &Dataset| data.weight_variable().and_then(|w| data.variable_index(w));
        assert_eq!(weight(&copy), weight(&original), "{shown}");
        let documents: Vec<String> = original
            .documents()
            .iter()
            .map(|d| statscraft_data::cut_to(d, 80).to_string())
            .collect();
        assert_eq!(copy.documents(), documents, "{shown}");
    }
}

#[test]
fn text_keeps_its_characters_and_long_values_are_cut() {
    let mut data = Dataset::new(3);
    data.set_string_values(
        "küsimus",
        300,
        vec!["õun ja ä".into(), "x".repeat(300), "tab\there #1".into()],
    )
    .unwrap();
    data.set_numeric_values("a_rather_long_name", vec![Some(1.0), None, Some(-0.5)])
        .unwrap();
    data.set_variable_label("küsimus", Some("Küsimus ± õ"))
        .unwrap();
    data.set_value_labels(
        "a_rather_long_name",
        vec![(Value::Number(1.0), "üks".into())],
    )
    .unwrap();
    data.set_missing_values(
        "a_rather_long_name",
        MissingValues::Range {
            low: f64::MIN,
            high: -1.0,
            extra: Some(99.0),
        },
    )
    .unwrap();
    let file = scratch("text.por");
    let warnings = data.write_por(&file, None).unwrap();
    assert_eq!(
        warnings,
        [
            "a portable file holds names of at most 8 bytes: a_rather_long_name as A_RATHER \
             in the file",
            "küsimus: a portable file holds strings of at most 255 bytes; longer values are cut"
        ]
    );
    let bytes = std::fs::read(&file).unwrap();
    assert!(bytes.chunks(82).all(|line| line.ends_with(b"\r\n")));
    assert!(bytes.chunks(82).all(|line| line.len() == 82));

    let copy = Dataset::read_por(&file).unwrap();
    assert_eq!(
        copy.variable_names().collect::<Vec<_>>(),
        ["K_SIMUS", "A_RATHER"]
    );
    let text = copy.variable("K_SIMUS").unwrap();
    assert_eq!(text.label(), Some("Küsimus ± õ"));
    let values: Vec<&str> = text.string().unwrap().iter().collect();
    // a tab is a control character, which the file cannot carry
    assert_eq!(values, ["õun ja ä", &"x".repeat(255), "tab0here #1"]);
    let number = copy.variable("A_RATHER").unwrap();
    assert_eq!(
        number.numeric().unwrap().iter().collect::<Vec<_>>(),
        [Some(1.0), None, Some(-0.5)]
    );
    assert_eq!(
        number.value_labels(),
        [(Value::Number(1.0), "üks".to_string())]
    );
    assert_eq!(
        number.missing_values(),
        &MissingValues::Range {
            low: f64::MIN,
            high: -1.0,
            extra: Some(99.0)
        }
    );
}

/// A file as another writer may lay it out: the weight before the variable count, lines of 80
/// characters with their trailing blanks left off and an extra carriage return, an empty
/// string, a number with an exponent and a string variable with a missing value.
#[test]
fn a_file_laid_out_by_another_writer_is_read() {
    let mut data = Dataset::new(0);
    data.set_numeric_values("x", Vec::new()).unwrap();
    let ours = scratch("header.por");
    data.write_por(&ours, None).unwrap();
    let stream: Vec<u8> = std::fs::read(&ours)
        .unwrap()
        .into_iter()
        .filter(|b| !b"\r\n".contains(b))
        .collect();
    let mut file = stream[..464].to_vec();
    file.extend(
        b"A8/202611126/1200001B/Some writer62/W143/5B/\
          70/2/W15/8/2/5/8/2/CA/The weight\
          73/2/S11/3/0/1/3/0/81/xC4/Text\
          70/2/N15/5/0/5/6/1/B1/3/\
          D1/2/S11/1/a5/Alpha\
          E1/H/a document line  \
          F1.F/1/a2+2/    2/0/*.Z"
            .iter(),
    );
    file.extend(std::iter::repeat_n(b'Z', 80 - file.len() % 80));
    let mut bytes = Vec::new();
    for line in file.chunks(80) {
        let trimmed = line.len() - line.iter().rev().take_while(|&&b| b == b' ').count();
        bytes.extend(&line[..trimmed]);
        bytes.extend(b"\r\r\n");
    }
    let path = scratch("other_writer.por");
    std::fs::write(&path, bytes).unwrap();

    let data = Dataset::read_por(&path).unwrap();
    assert_eq!(data.case_count(), 2);
    assert_eq!(data.weight_variable(), Some("W1"));
    assert_eq!(data.documents(), ["a document line"]);
    let w = data.variable("W1").unwrap();
    assert_eq!(w.label(), Some("The weight"));
    assert_eq!(w.format(), "F8.2");
    assert_eq!(
        w.numeric().unwrap().iter().collect::<Vec<_>>(),
        [Some(1.5), Some(2.0)]
    );
    let s = data.variable("S1").unwrap();
    assert_eq!(s.string().unwrap().iter().collect::<Vec<_>>(), ["a", ""]);
    assert_eq!(
        s.missing_values(),
        &MissingValues::Strings(vec!["x".into()])
    );
    assert_eq!(
        s.value_labels(),
        [(Value::String("a".into()), "Alpha".into())]
    );
    let n = data.variable("N1").unwrap();
    assert_eq!(n.format(), "F5.0");
    assert_eq!(n.write_format(), "F6.1");
    assert_eq!(
        n.numeric().unwrap().iter().collect::<Vec<_>>(),
        [Some(1800.0), None]
    );
    assert_eq!(
        n.missing_values(),
        &MissingValues::Range {
            low: 1.0,
            high: 3.0,
            extra: None
        }
    );
}

#[test]
fn what_is_not_a_portable_file_is_refused() {
    let path = scratch("not.por");
    std::fs::write(&path, "x".repeat(600)).unwrap();
    let error = Dataset::read_por(&path).err().unwrap().to_string();
    assert!(
        error.ends_with("not a portable file: its tag is missing"),
        "{error}"
    );
    std::fs::write(&path, "short").unwrap();
    let error = Dataset::read_por(&path).err().unwrap().to_string();
    assert!(error.ends_with("not a portable file: too short"), "{error}");
}
