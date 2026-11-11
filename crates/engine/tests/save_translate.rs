//! SAVE TRANSLATE to CSV, tab-delimited text and workbooks, read back where it can be.

use calamine::{Data, Reader};
use statscraft_data::{Dataset, Values};
use statscraft_engine::session::Session;
use std::path::{Path, PathBuf};

const DATA: &str = "DATA LIST LIST /id (F2) name (A8) score (F6.2) born (ADATE10) t (F5) grp (F1).
BEGIN DATA
1 'Ann Lee' 1.5 01/02/2020 22500 1
2 'Bob, Jr' . 12/31/1999 25230 2
3 'Cy \"C\"' 2.25 02/29/2024 . 9
END DATA.
VARIABLE LABELS score 'Test score'.
VALUE LABELS grp 1 'Yes' 2 'No' 9 'Missing'.
MISSING VALUES grp (9).
FORMATS born (DATE11) t (TIME8).
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("statscraft-translate-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn session(dir: &Path) -> Session {
    let mut session = Session::default();
    session.set_base_dir(Some(dir.to_path_buf()));
    let source = format!("CD '{}'.\n{DATA}", dir.display());
    session
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    session
}

fn run(session: &mut Session, syntax: &str) {
    session
        .run(syntax, None)
        .unwrap_or_else(|e| panic!("{syntax}: {e:?}"));
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

fn text(path: &Path) -> String {
    String::from_utf8(std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn csv_with_names_plain_values_and_qualifiers() {
    let dir = temp_dir("csv");
    let mut s = session(&dir);
    run(
        &mut s,
        "SAVE TRANSLATE OUTFILE='plain.csv' /TYPE=CSV /FIELDNAMES.",
    );
    assert_eq!(
        text(&dir.join("plain.csv")),
        "\u{feff}id,name,score,born,t,grp\r\n\
         1,Ann Lee,1.5,01/02/2020,06:15:00,1\r\n\
         2,\"Bob, Jr\",,12/31/1999,07:00:30,2\r\n\
         3,\"Cy \"\"C\"\"\",2.25,02/29/2024,,9\r\n"
    );
    // labels, user-missing recoded, a comma decimal mark and its semicolon
    run(
        &mut s,
        "SAVE TRANSLATE OUTFILE='labels.csv' /FIELDNAMES VALUE=LABELS /CELLS=LABELS\n\
         /MISSING=RECODE /TEXTOPTIONS DECIMAL=COMMA /ENCODING='UTF8' BOM=NO.",
    );
    assert_eq!(
        text(&dir.join("labels.csv")),
        "id;name;Test score;born;t;grp\r\n\
         1;Ann Lee;1,5;01/02/2020;06:15:00;Yes\r\n\
         2;Bob, Jr;;12/31/1999;07:00:30;No\r\n\
         3;\"Cy \"\"C\"\"\";2,25;02/29/2024;;\r\n"
    );
    // REPLACE is needed to overwrite
    let message = error(&mut s, "SAVE TRANSLATE OUTFILE='plain.csv' /TYPE=CSV.");
    assert_eq!(
        message,
        "SAVE TRANSLATE: plain.csv exists; add /REPLACE to overwrite it"
    );
    // a delimiter and a qualifier of one's own
    run(
        &mut s,
        "SAVE TRANSLATE OUTFILE='plain.csv' /TYPE=CSV /REPLACE /KEEP=name id\n\
         /TEXTOPTIONS DELIMITER='|' QUALIFIER=\"'\".",
    );
    assert_eq!(
        text(&dir.join("plain.csv")),
        "\u{feff}1|Ann Lee\r\n2|Bob, Jr\r\n3|Cy \"C\"\r\n"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn tab_with_display_formats() {
    let dir = temp_dir("tab");
    let mut s = session(&dir);
    run(
        &mut s,
        "SAVE TRANSLATE OUTFILE='data.dat' /TYPE=TAB /TEXTOPTIONS FORMAT=VARIABLE\n\
         /ENCODING='UTF8' BOM=NO.",
    );
    assert_eq!(
        text(&dir.join("data.dat")),
        "1\tAnn Lee\t1.50\t02-JAN-2020\t06:15:00\t1\r\n\
         2\tBob, Jr\t\t31-DEC-1999\t07:00:30\t2\r\n\
         3\tCy \"C\"\t2.25\t29-FEB-2024\t\t9\r\n"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn keep_rename_and_unselected_cases() {
    let dir = temp_dir("subset");
    let mut s = session(&dir);
    run(
        &mut s,
        "COMPUTE f = id <> 2.\nFILTER BY f.\n\
         SAVE TRANSLATE OUTFILE='subset.csv' /TYPE=CSV /FIELDNAMES /UNSELECTED=DELETE\n\
         /RENAME=(id=key) /KEEP=grp key /ENCODING='UTF8' BOM=NO.",
    );
    // KEEP keeps the order of the dataset; the active dataset is unchanged
    assert_eq!(text(&dir.join("subset.csv")), "key,grp\r\n1,1\r\n3,9\r\n");
    let data = s.dataset().unwrap();
    assert_eq!(data.case_count(), 3);
    assert!(data.variable("id").is_ok());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn encodings_and_byte_order_marks() {
    let dir = temp_dir("encoding");
    let mut s = Session::default();
    s.set_base_dir(Some(dir.clone()));
    let source = format!(
        "CD '{}'.\nDATA LIST LIST /name (A8).\nBEGIN DATA\nJüri\nEND DATA.",
        dir.display()
    );
    run(&mut s, &source);
    run(
        &mut s,
        "SAVE TRANSLATE OUTFILE='a.csv' /TYPE=CSV /ENCODING='windows-1252' BOM=NO.",
    );
    assert_eq!(std::fs::read(dir.join("a.csv")).unwrap(), b"J\xFCri\r\n");
    run(
        &mut s,
        "SAVE TRANSLATE OUTFILE='b.csv' /TYPE=CSV /ENCODING='UTF16LE'.",
    );
    let bytes = std::fs::read(dir.join("b.csv")).unwrap();
    assert_eq!(&bytes[..6], [0xFF, 0xFE, b'J', 0, 0xFC, 0]);
    run(&mut s, "COMPUTE name = 'Ωmega'.");
    let message = error(
        &mut s,
        "SAVE TRANSLATE OUTFILE='c.csv' /TYPE=CSV /ENCODING='windows-1252'.",
    );
    assert_eq!(
        message,
        "SAVE TRANSLATE: c.csv: the data has characters that windows-1252 cannot hold; use /ENCODING='UTF8'"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_workbook_round_trip_keeps_numbers_dates_and_missing_values() {
    let dir = temp_dir("xlsx");
    let mut s = session(&dir);
    run(
        &mut s,
        "SAVE TRANSLATE OUTFILE='wave.xlsx' /FIELDNAMES /EXCELOPTIONS SHEET='Wave 1'.",
    );
    // system-missing is the error #NULL!, a date an Excel date
    let mut book: calamine::Xlsx<_> = calamine::open_workbook(dir.join("wave.xlsx")).unwrap();
    assert_eq!(book.sheet_names(), ["Wave 1"]);
    let range = book.worksheet_range("Wave 1").unwrap();
    assert_eq!(range.get_value((0, 2)), Some(&Data::String("score".into())));
    assert!(matches!(range.get_value((2, 2)), Some(Data::Error(_))));
    match range.get_value((1, 3)) {
        Some(Data::DateTime(d)) => assert_eq!(d.as_f64(), 43_832.0),
        other => panic!("{other:?}"),
    }
    run(&mut s, "GET DATA /TYPE=XLSX /FILE='wave.xlsx'.");
    let data = s.dataset().unwrap();
    assert_eq!(numbers(data, "id"), [Some(1.0), Some(2.0), Some(3.0)]);
    assert_eq!(strings(data, "name"), ["Ann Lee", "Bob, Jr", "Cy \"C\""]);
    assert_eq!(numbers(data, "score"), [Some(1.5), None, Some(2.25)]);
    let day = |n: f64| Some(n * 86_400.0);
    // 2020-01-02, 1999-12-31 and 2024-02-29 in days from 1582-10-14
    assert_eq!(
        numbers(data, "born"),
        [day(159_691.0), day(152_384.0), day(161_210.0)]
    );
    assert_eq!(numbers(data, "t"), [Some(22_500.0), Some(25_230.0), None]);
    assert_eq!(numbers(data, "grp"), [Some(1.0), Some(2.0), Some(9.0)]);
    let format = |name: &str| data.variable(name).unwrap().format().to_string();
    assert_eq!(format("born"), "DATE11");
    assert_eq!(format("t"), "TIME8");
    // value labels as text
    let mut s = session(&dir);
    run(
        &mut s,
        "SAVE TRANSLATE OUTFILE='labels.xlsx' /TYPE=XLSX /CELLS=LABELS /KEEP=grp.\n\
         GET DATA /TYPE=XLSX /FILE='labels.xlsx' /READNAMES=OFF.",
    );
    assert_eq!(
        strings(s.dataset().unwrap(), "V1"),
        ["Yes", "No", "Missing"]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn types_and_options_that_are_refused() {
    let dir = temp_dir("refused");
    let mut s = session(&dir);
    let cases = [
        (
            "SAVE TRANSLATE OUTFILE='a.sas7bdat' /TYPE=SAS /VERSION=7.",
            "SAVE TRANSLATE /TYPE=SAS is not supported yet; write CSV, TAB or XLS /VERSION=12",
        ),
        (
            "SAVE TRANSLATE OUTFILE='a.xls' /TYPE=XLS /VERSION=8.",
            "Statscraft writes Excel 2007 and later workbooks: use /VERSION=12 and a .xlsx file",
        ),
        (
            "SAVE TRANSLATE OUTFILE='a.xlsx' /ENCODING='UTF8'.",
            "ENCODING applies to TYPE=CSV and TAB; a workbook is always Unicode",
        ),
        (
            "SAVE TRANSLATE OUTFILE='a.txt'.",
            "SAVE TRANSLATE needs /TYPE=CSV, TAB or XLS /VERSION=12",
        ),
        (
            "SAVE TRANSLATE OUTFILE='a.xlsx' /APPEND.",
            "subcommand APPEND of SAVE TRANSLATE is not supported yet",
        ),
        (
            "SAVE TRANSLATE OUTFILE='a.csv' /TEXTOPTIONS DELIMITER=',' DECIMAL=COMMA.",
            "the delimiter, qualifier and decimal mark must all differ",
        ),
    ];
    for (syntax, expected) in cases {
        assert_eq!(error(&mut s, syntax), expected, "{syntax}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
