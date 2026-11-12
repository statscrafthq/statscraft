//! EXPORT and IMPORT (portable files) and GET TRANSLATE on files made up here.

use statscraft_data::{Dataset, MissingValues, Value, Values};
use statscraft_engine::Cell;
use statscraft_engine::session::{RunResult, Session};
use std::path::{Path, PathBuf};

fn temp_dir(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("statscraft-portable-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn session(dir: &Path) -> Session {
    let mut session = Session::default();
    session.set_base_dir(Some(dir.to_path_buf()));
    run(&mut session, &format!("CD '{}'.", dir.display()));
    session
}

fn run(session: &mut Session, syntax: &str) -> RunResult {
    session
        .run(syntax, None)
        .unwrap_or_else(|e| panic!("{syntax}: {e:?}"))
}

fn error(session: &mut Session, syntax: &str) -> String {
    session.run(syntax, None).unwrap_err()[0].message.clone()
}

fn numbers(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => v.iter().collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

fn strings(data: &Dataset, name: &str) -> Vec<String> {
    match data.variable(name).unwrap().values() {
        Values::String(v) => v.iter().map(str::to_string).collect(),
        Values::Numeric(_) => panic!("{name} is numeric"),
    }
}

fn names(data: &Dataset) -> Vec<&str> {
    data.variable_names().collect()
}

/// The text of every cell of the tables of a run, row by row.
fn rows(result: &RunResult) -> Vec<Vec<Vec<String>>> {
    let text = |cell: &Cell| match cell {
        Cell::Text(t) => t.clone(),
        other => format!("{other:?}"),
    };
    result
        .commands
        .iter()
        .flat_map(|c| &c.tables)
        .map(|t| {
            t.rows
                .iter()
                .map(|r| r.iter().map(text).collect())
                .collect()
        })
        .collect()
}

fn warnings(result: &RunResult) -> Vec<String> {
    result
        .commands
        .iter()
        .flat_map(|c| c.warnings.clone())
        .collect()
}

const DATA: &str = "DATA LIST LIST /id (F4.0) score (F8.2) name (A12) longvariablename (F5.1)
  when (DATE11).
BEGIN DATA
1 3.25 alpha 10.5 01-JAN-2020
2 -7.5 'beta gamma' 0.333333333 15-MAR-1999
3 . '' 1e20 .
4 9 delta 99 31-DEC-2001
END DATA.
VARIABLE LABELS id 'Identifier' score 'Test score'.
VALUE LABELS id 1 'one' 2 'two' /name 'alpha' 'First letter'.
MISSING VALUES score (9) longvariablename (90 THRU 100).
MISSING VALUES name ('delta').
WRITE FORMATS score (F10.3).
WEIGHT BY score.
DOCUMENT a test document line.
";

#[test]
fn a_dataset_comes_back_from_a_portable_file() {
    let dir = temp_dir("roundtrip");
    let mut s = session(&dir);
    run(&mut s, DATA);
    let score = s.dataset().unwrap().variable("score").unwrap();
    let formats = (score.format().to_string(), score.write_format().to_string());
    let documents = s.dataset().unwrap().documents().to_vec();
    assert_eq!(documents[0], "a test document line");
    let exported = run(
        &mut s,
        "EXPORT OUTFILE='p.por' /RENAME=(score=points) /MAP.",
    );
    assert_eq!(
        rows(&exported),
        [[
            ["ID", "id"],
            ["POINTS", "score"],
            ["NAME", "name"],
            ["LONGVARI", "longvariablename"],
            ["WHEN", "when"],
        ]]
    );
    assert_eq!(
        warnings(&exported),
        [
            "a portable file holds names of at most 8 bytes: longvariablename as LONGVARI in the \
          file"
        ]
    );
    // the active dataset is as it was
    assert_eq!(names(s.dataset().unwrap())[1], "score");

    let imported = run(&mut s, "IMPORT FILE='p.por' /RENAME=(longvari=ratio) /MAP.");
    assert_eq!(rows(&imported)[0][3], ["ratio", "LONGVARI"]);
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["ID", "POINTS", "NAME", "ratio", "WHEN"]);
    assert_eq!(
        numbers(data, "ID"),
        [Some(1.0), Some(2.0), Some(3.0), Some(4.0)]
    );
    assert_eq!(
        numbers(data, "POINTS"),
        [Some(3.25), Some(-7.5), None, Some(9.0)]
    );
    assert_eq!(
        numbers(data, "ratio"),
        [Some(10.5), Some(0.333333333), Some(1e20), Some(99.0)]
    );
    assert_eq!(strings(data, "NAME"), ["alpha", "beta gamma", "", "delta"]);
    let when = numbers(data, "WHEN");
    assert_eq!(when[0], Some(13_797_216_000.0));
    assert_eq!(when[2], None);
    let id = data.variable("ID").unwrap();
    assert_eq!(id.label(), Some("Identifier"));
    assert_eq!(
        id.value_labels(),
        [
            (Value::Number(1.0), "one".to_string()),
            (Value::Number(2.0), "two".to_string())
        ]
    );
    let points = data.variable("POINTS").unwrap();
    assert_eq!(
        (points.format(), points.write_format()),
        (formats.0.as_str(), formats.1.as_str())
    );
    assert_eq!(formats.1, "F10.3");
    assert_eq!(points.missing_values(), &MissingValues::Numbers(vec![9.0]));
    assert_eq!(
        data.variable("ratio").unwrap().missing_values(),
        &MissingValues::Range {
            low: 90.0,
            high: 100.0,
            extra: None
        }
    );
    let name = data.variable("NAME").unwrap();
    assert_eq!(
        name.missing_values(),
        &MissingValues::Strings(vec!["delta".into()])
    );
    assert_eq!(
        name.value_labels(),
        [(Value::String("alpha".into()), "First letter".into())]
    );
    assert_eq!(data.variable("WHEN").unwrap().format(), "DATE11");
    assert_eq!(data.weight_variable(), Some("POINTS"));
    assert_eq!(data.documents(), documents);
}

#[test]
fn export_keeps_reorders_rounds_and_drops_unselected_cases() {
    let dir = temp_dir("options");
    let mut s = session(&dir);
    run(&mut s, DATA);
    run(
        &mut s,
        "COMPUTE keep = id <> 2.
FILTER BY keep.
EXPORT OUTFILE='p.por' /KEEP=longvariablename id /MAP /DROP=id /DIGITS=3 /UNSELECTED=DELETE.",
    );
    run(&mut s, "IMPORT FILE='p.por'.");
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["LONGVARI"]);
    assert_eq!(
        numbers(data, "LONGVARI"),
        [Some(10.5), Some(1e20), Some(99.0)]
    );

    run(&mut s, "NEW FILE.");
    run(&mut s, DATA);
    run(&mut s, "EXPORT OUTFILE='q.por' /DIGITS=3.");
    let result = run(&mut s, "IMPORT FILE='q.por' /KEEP=longvari id /MAP.");
    assert_eq!(rows(&result)[0], [["LONGVARI", "LONGVARI"], ["ID", "ID"]]);
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["LONGVARI", "ID"]);
    assert_eq!(numbers(data, "LONGVARI")[1], Some(0.333));

    assert!(
        error(&mut s, "IMPORT FILE='missing.por'.").contains("missing.por"),
        "a file that is not there is an error"
    );
    std::fs::write(dir.join("text.por"), "x".repeat(600)).unwrap();
    assert!(error(&mut s, "IMPORT FILE='text.por'.").ends_with("its tag is missing"));
}

#[test]
fn get_translate_reads_tab_delimited_text() {
    let dir = temp_dir("tab");
    std::fs::write(
        dir.join("wave.txt"),
        "a\tb\tGE\tb\n1\tx\t3.25\t\r\n2.5\tlonger text here\tz\t7\r\n",
    )
    .unwrap();
    let mut s = session(&dir);
    let result = run(
        &mut s,
        "GET TRANSLATE FILE='wave.txt' /TYPE=TAB /FIELDNAMES /DROP=a /MAP.",
    );
    assert_eq!(rows(&result), [[["b", "b"], ["GE$", "GE$"], ["b2", "b2"]]]);
    assert_eq!(
        warnings(&result),
        [
            "case 2: b is cut to 8 bytes",
            "case 2: GE$ is system-missing: 'z' is not a number"
        ]
    );
    let data = s.dataset().unwrap();
    assert_eq!(strings(data, "b"), ["x", "longer t"]);
    assert_eq!(data.variable("b").unwrap().format(), "A8");
    assert_eq!(numbers(data, "GE$"), [Some(3.25), None]);
    assert_eq!(data.variable("GE$").unwrap().format(), "F8.2");
    // an empty first value makes a number
    assert_eq!(numbers(data, "b2"), [None, Some(7.0)]);

    run(&mut s, "GET TRANSLATE FILE='wave.txt' /TYPE=TAB.");
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["VAR1", "VAR2", "VAR3", "VAR4"]);
    assert_eq!(strings(data, "VAR1"), ["a", "1", "2.5"]);

    std::fs::write(dir.join("wide.tab"), "id\tvalue\n7\t123456789.125\n8\t1\n").unwrap();
    run(&mut s, "GET TRANSLATE FILE='wide.tab' /RANGE='B1:B3'.");
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["VAR1"]);
    // the first value makes the string 8 bytes wide
    assert_eq!(strings(data, "VAR1"), ["value", "12345678", "1"]);
    run(
        &mut s,
        "GET TRANSLATE FILE='wide.tab' /FIELDNAMES /RANGE=B1..B3.",
    );
    let data = s.dataset().unwrap();
    assert_eq!(numbers(data, "value"), [Some(123_456_789.125), Some(1.0)]);
    assert_eq!(data.variable("value").unwrap().format(), "F13.3");
}

/// A dBASE III file: `fields` as (name, type, length, decimals); every value already as wide
/// as its field.
fn dbf(fields: &[(&str, u8, u8, u8)], records: &[(bool, Vec<&str>)]) -> Vec<u8> {
    let header = 32 + 32 * fields.len() + 1;
    let record = 1 + fields.iter().map(|f| usize::from(f.2)).sum::<usize>();
    let mut bytes = vec![0x03, 126, 11, 12];
    bytes.extend((records.len() as u32).to_le_bytes());
    bytes.extend((header as u16).to_le_bytes());
    bytes.extend((record as u16).to_le_bytes());
    bytes.resize(29, 0);
    bytes.push(0x57);
    bytes.resize(32, 0);
    for (name, kind, length, decimals) in fields {
        let mut field = name.as_bytes().to_vec();
        field.resize(11, 0);
        field.push(*kind);
        field.extend([0; 4]);
        field.extend([*length, *decimals]);
        field.resize(32, 0);
        bytes.extend(field);
    }
    bytes.push(0x0D);
    for (deleted, values) in records {
        bytes.push(if *deleted { b'*' } else { b' ' });
        for (value, field) in values.iter().zip(fields) {
            assert_eq!(value.len(), usize::from(field.2), "{value}");
            bytes.extend(value.as_bytes());
        }
    }
    bytes.push(0x1A);
    bytes
}

#[test]
fn get_translate_reads_dbase_files() {
    let dir = temp_dir("dbf");
    let fields = [
        ("NAME", b'C', 6, 0),
        ("AGE", b'N', 3, 0),
        ("PAY", b'N', 8, 2),
        ("BORN", b'D', 8, 0),
        ("OK", b'L', 1, 0),
        ("NOTE", b'M', 10, 0),
        ("BY", b'C', 2, 0),
        ("PHONE:NO", b'C', 4, 0),
    ];
    let records = [
        (
            false,
            vec![
                "Ann   ",
                " 34",
                " 1234.50",
                "20200131",
                "T",
                "         1",
                "x ",
                "5551",
            ],
        ),
        (
            true,
            vec![
                "Bob   ",
                "   ",
                "     abc",
                "        ",
                "F",
                "          ",
                "  ",
                "    ",
            ],
        ),
    ];
    std::fs::write(dir.join("address.dbf"), dbf(&fields, &records)).unwrap();
    let mut s = session(&dir);
    let result = run(
        &mut s,
        "GET TRANSLATE FILE='address.dbf' /FIELDNAMES /DROP=phone_no /MAP.",
    );
    assert_eq!(
        warnings(&result),
        ["record 2: PAY is system-missing: 'abc' is not a number"]
    );
    let data = s.dataset().unwrap();
    assert_eq!(
        names(data),
        ["NAME", "AGE", "PAY", "BORN", "OK", "BY$", "D_R"]
    );
    assert_eq!(rows(&result)[0].len(), 7);
    assert_eq!(strings(data, "NAME"), ["Ann", "Bob"]);
    assert_eq!(data.variable("NAME").unwrap().format(), "A6");
    assert_eq!(numbers(data, "AGE"), [Some(34.0), None]);
    assert_eq!(data.variable("AGE").unwrap().format(), "F3.0");
    assert_eq!(numbers(data, "PAY"), [Some(1234.5), None]);
    assert_eq!(data.variable("PAY").unwrap().format(), "F8.2");
    assert_eq!(
        numbers(data, "BORN"),
        [Some(13_797_216_000.0 + 30.0 * 86_400.0), None]
    );
    assert_eq!(data.variable("BORN").unwrap().format(), "DATE11");
    assert_eq!(strings(data, "OK"), ["T", "F"]);
    assert_eq!(strings(data, "BY$"), ["x", ""]);
    assert_eq!(strings(data, "D_R"), ["", "*"]);

    let error = error(&mut s, "GET TRANSLATE FILE='address.dbf' /RANGE=A1..B2.");
    assert_eq!(
        error,
        "RANGE applies to spreadsheets and tab-delimited files, not to dBASE files"
    );
}

#[test]
fn get_translate_reads_a_workbook_with_column_letters() {
    let dir = temp_dir("xls");
    let mut book = rust_xlsxwriter::Workbook::new();
    let sheet = book.add_worksheet();
    for (c, heading) in ["id", "town"].iter().enumerate() {
        sheet.write_string(1, c as u16 + 1, *heading).unwrap();
    }
    sheet.write_number(2, 1, 1.0).unwrap();
    sheet.write_string(2, 2, "Tartu").unwrap();
    sheet.write_number(3, 1, 2.0).unwrap();
    sheet.write_string(3, 2, "Pärnu").unwrap();
    // the content decides how a workbook is read, so this one stands in for an old one
    book.save(dir.join("old.xls")).unwrap();
    let mut s = session(&dir);
    run(&mut s, "GET TRANSLATE FILE='old.xls' /FIELDNAMES.");
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["id", "town"]);
    assert_eq!(numbers(data, "id"), [Some(1.0), Some(2.0)]);
    run(&mut s, "GET TRANSLATE FILE='old.xls' /KEEP=c.");
    let data = s.dataset().unwrap();
    assert_eq!(names(data), ["C"]);
    assert_eq!(strings(data, "C"), ["town", "Tartu", "Pärnu"]);
    run(&mut s, "GET TRANSLATE FILE='old.xls' /RANGE='C3:C4'.");
    assert_eq!(names(s.dataset().unwrap()), ["C"]);
}
