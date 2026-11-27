//! Portable files: what is written comes back, within what the format holds.

use crate::common;

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

/// The text as a portable file keeps it: the letters, digits and marks of the format's character
/// set stay, a control character is `0`, and anything else (a letter with an accent, a symbol)
/// is `?`, one for each character.
fn portable(text: &str) -> String {
    const SET: &str = " .<(+|&[]!$*);^-/,%_>?`:#@'=\"~{}\\";
    text.chars()
        .map(|c| match c {
            c if c.is_ascii_alphanumeric() || SET.contains(c) => c,
            c if c.is_ascii_control() => '0',
            _ => '?',
        })
        .collect()
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
            assert_eq!(
                after.label().map(str::to_string),
                before.label().map(portable),
                "{what}"
            );
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
                        .map(|s| {
                            let text = portable(s);
                            statscraft_data::cut_to(&text, width).trim_end().to_string()
                        })
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
                    let labels: Vec<(Value, String)> = before
                        .value_labels()
                        .iter()
                        .map(|(value, label)| (value.clone(), portable(label)))
                        .collect();
                    assert_eq!(after.value_labels(), labels, "{what}");
                }
            }
        }
        let weight = |data: &Dataset| data.weight_variable().and_then(|w| data.variable_index(w));
        assert_eq!(weight(&copy), weight(&original), "{shown}");
        let documents: Vec<String> = original
            .documents()
            .iter()
            .map(|d| statscraft_data::cut_to(&portable(d), 80).to_string())
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
            "küsimus: a portable file holds strings of at most 255 bytes; longer values are cut",
            "a portable file has no code for 6 characters outside its character set, such as \
             letters with accents: written as ?, so texts that differ only in them become equal"
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
    // a letter with an accent and the plus-minus sign are not in the format's character set
    assert_eq!(text.label(), Some("K?simus ? ?"));
    let values: Vec<&str> = text.string().unwrap().iter().collect();
    // a tab is a control character, which the file cannot carry; # is in the character set
    assert_eq!(values, ["?un ja ?", &"x".repeat(255), "tab0here #1"]);
    let number = copy.variable("A_RATHER").unwrap();
    assert_eq!(
        number.numeric().unwrap().iter().collect::<Vec<_>>(),
        [Some(1.0), None, Some(-0.5)]
    );
    assert_eq!(
        number.value_labels(),
        [(Value::Number(1.0), "?ks".to_string())]
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

/// The stream of a portable file: its lines of 80 characters joined, without the padding of the
/// last line.
fn stream_of(bytes: &[u8]) -> Vec<u8> {
    let mut stream: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|b| !b"\r\n".contains(b))
        .collect();
    while stream.last() == Some(&b'Z') {
        stream.pop();
    }
    stream
}

/// Lines of 80 characters from a stream, the end tag and its padding put back.
fn file_of(mut stream: Vec<u8>) -> Vec<u8> {
    stream.push(b'Z');
    let padding = (80 - stream.len() % 80) % 80;
    stream.extend(std::iter::repeat_n(b'Z', padding));
    stream
        .chunks(80)
        .flat_map(|line| line.iter().copied().chain(*b"\r\n"))
        .collect()
}

#[test]
fn text_goes_in_by_the_character_table() {
    let mut data = Dataset::new(3);
    data.set_string_values(
        "note",
        10,
        vec!["Zoë #1".into(), "ÄÖÜõ".into(), "plain".into()],
    )
    .unwrap();
    data.set_variable_label("note", Some("Märkus ≥ 5, no. #2"))
        .unwrap();
    let file = scratch("table.por");
    let warnings = data.write_por(&file, None).unwrap();
    // one ? for each character, not for each byte: ë, Ä, Ö, Ü and õ in the values, ä and ≥ in
    // the label
    assert_eq!(
        warnings,
        [
            "a portable file has no code for 7 characters outside its character set, such as \
             letters with accents: written as ?, so texts that differ only in them become equal"
        ]
    );
    let bytes = std::fs::read(&file).unwrap();
    assert!(bytes.is_ascii(), "every byte of the file is in the table");
    // the table names the character set's # (code 151), which text may hold
    let stream = stream_of(&bytes);
    assert_eq!(stream[200 + 151], b'#');
    let copy = Dataset::read_por(&file).unwrap();
    let note = copy.variable("NOTE").unwrap();
    assert_eq!(note.label(), Some("M?rkus ? 5, no. #2"));
    let values: Vec<&str> = note.string().unwrap().iter().collect();
    // the string keeps its width, so a ? takes the place of a character, not of its bytes
    assert_eq!(values, ["Zo? #1", "????", "plain"]);
}

#[test]
fn files_of_earlier_versions_with_utf8_text_still_open() {
    // an earlier writer put the UTF-8 bytes of a letter in the file, which the table does not
    // name; the reader takes such a byte as it stands and reads the text as UTF-8, or as
    // windows-1252 when it is not
    let mut data = Dataset::new(2);
    data.set_string_values("s", 8, vec!["a?b".into(), "c?d".into()])
        .unwrap();
    data.set_variable_label("s", Some("X?Y")).unwrap();
    let file = scratch("old.por");
    data.write_por(&file, None).unwrap();
    let stream = stream_of(&std::fs::read(&file).unwrap());
    let patch = |stream: &[u8], from: &[u8], to: &[u8]| -> Vec<u8> {
        let at = stream
            .windows(from.len())
            .position(|w| w == from)
            .unwrap_or_else(|| panic!("{}", String::from_utf8_lossy(from)));
        let mut out = stream[..at].to_vec();
        out.extend(to);
        out.extend(&stream[at + from.len()..]);
        out
    };
    // the strings are padded to 8 bytes: é is two bytes in UTF-8, one in windows-1252
    let utf8 = patch(&stream, b"8/a?b     ", "8/aéb    ".as_bytes());
    let utf8 = patch(&utf8, b"8/c?d     ", "8/cÿd    ".as_bytes());
    // the label has no padding, so its length follows the letter: 3 bytes become 4
    let utf8 = patch(&utf8, b"3/X?Y", "4/XéY".as_bytes());
    let old = scratch("old_utf8.por");
    std::fs::write(&old, file_of(utf8)).unwrap();
    let copy = Dataset::read_por(&old).unwrap();
    let s = copy.variable("S").unwrap();
    let values: Vec<&str> = s.string().unwrap().iter().collect();
    assert_eq!(values, ["aéb", "cÿd"]);
    assert_eq!(s.label(), Some("XéY"));
    // text that is not UTF-8 is windows-1252: one byte 0xE9 is é
    let latin = patch(&stream, b"8/a?b     ", b"8/a\xe9b     ");
    let latin = patch(&latin, b"8/c?d     ", b"8/c\xe9d     ");
    let old = scratch("old_latin.por");
    std::fs::write(&old, file_of(latin)).unwrap();
    let copy = Dataset::read_por(&old).unwrap();
    let values: Vec<&str> = copy
        .variable("S")
        .unwrap()
        .string()
        .unwrap()
        .iter()
        .collect();
    assert_eq!(values, ["aéb", "céd"]);
}

#[test]
fn only_the_characters_that_are_written_are_counted_and_cut() {
    // a file holds 255 bytes of a string: of 300 accented letters 255 are written, as 255 ?, and
    // only those count
    let mut data = Dataset::new(1);
    data.set_string_values("s", 600, vec!["é".repeat(300)])
        .unwrap();
    let file = scratch("cut.por");
    let warnings = data.write_por(&file, None).unwrap();
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("no code for 255 characters")),
        "{warnings:?}"
    );
    let copy = Dataset::read_por(&file).unwrap();
    let values: Vec<&str> = copy
        .variable("S")
        .unwrap()
        .string()
        .unwrap()
        .iter()
        .collect();
    assert_eq!(values, ["?".repeat(255)]);

    // a line of documents holds 80 characters of the file: 80 accented letters are 80 ?, not the
    // 40 that 80 of their UTF-8 bytes would have left
    let mut data = Dataset::new(1);
    data.set_numeric_values("x", vec![Some(1.0)]).unwrap();
    data.set_documents(vec!["é".repeat(90)]);
    let file = scratch("document.por");
    let warnings = data.write_por(&file, None).unwrap();
    assert!(
        warnings[0].contains("no code for 80 characters"),
        "{warnings:?}"
    );
    let copy = Dataset::read_por(&file).unwrap();
    assert_eq!(copy.documents(), ["?".repeat(80)]);
}
