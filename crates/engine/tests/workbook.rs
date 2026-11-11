//! GET DATA /TYPE=XLSX on workbooks written in the test.

use rust_xlsxwriter::{ExcelDateTime, Format, Workbook};
use statscraft_data::{Dataset, Values};
use statscraft_engine::session::{RunResult, Session};
use std::path::{Path, PathBuf};

fn temp_dir(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("statscraft-workbook-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
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

/// Seconds from 1582-10-14 of a day, as the date formats count them.
fn date(y: i64, m: i64, d: i64) -> f64 {
    // days from 1970-01-01, then the 141 428 days from 1582-10-14 to 1970-01-01
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    ((era * 146_097 + doe - 719_468 + 141_428) * 86_400) as f64
}

fn run(dir: &Path, syntax: &str) -> (Session, RunResult) {
    let mut session = Session::default();
    session.set_base_dir(Some(dir.to_path_buf()));
    let source = format!("CD '{}'.\n{syntax}", dir.display());
    let result = session
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    (session, result)
}

fn error(dir: &Path, syntax: &str) -> String {
    let mut session = Session::default();
    let source = format!("CD '{}'.\n{syntax}", dir.display());
    let errors = session.run(&source, None).unwrap_err();
    errors[0].message.clone()
}

/// id, name, score, born, hour of `at`, hour of `start`
type Row<'a> = (f64, &'a str, Option<f64>, (u16, u8, u8), u16, u16);

/// A survey sheet with every kind of cell, a blank row, an empty column and a second sheet.
fn survey(path: &Path) {
    let mut book = Workbook::new();
    let day = Format::new().set_num_format("yyyy-mm-dd");
    let moment = Format::new().set_num_format("yyyy-mm-dd hh:mm");
    let clock = Format::new().set_num_format("hh:mm:ss");
    let sheet = book.add_worksheet().set_name("Wave 1").unwrap();
    let head = [
        "id",
        "First name",
        "score",
        "born",
        "at",
        "start",
        "mixed",
        "ok",
        "1st",
        "",
        "id",
    ];
    for (c, h) in head.iter().enumerate() {
        if !h.is_empty() {
            sheet.write_string(0, c as u16, *h).unwrap();
        }
    }
    let rows: [Row; 3] = [
        (1.0, "Ann", Some(1.5), (2020, 1, 1), 10, 6),
        (2.0, "  Bob", Some(2.0), (1999, 12, 31), 11, 7),
        (3.0, "Cy  ", None, (2024, 2, 29), 12, 8),
    ];
    for (k, (id, name, score, (y, m, d), hour, start)) in rows.into_iter().enumerate() {
        // row 3 of the sheet (index 2) stays blank
        let r = if k == 0 { 1 } else { k as u32 + 2 };
        sheet.write_number(r, 0, id).unwrap();
        sheet.write_string(r, 1, name).unwrap();
        if let Some(score) = score {
            sheet.write_number(r, 2, score).unwrap();
        }
        let born = ExcelDateTime::from_ymd(y, m, d).unwrap();
        sheet.write_datetime_with_format(r, 3, &born, &day).unwrap();
        let at = ExcelDateTime::from_ymd(2024, 3, 5)
            .unwrap()
            .and_hms(hour, 30, 0)
            .unwrap();
        sheet
            .write_datetime_with_format(r, 4, &at, &moment)
            .unwrap();
        let start = ExcelDateTime::from_hms(start, 15, 0).unwrap();
        sheet
            .write_datetime_with_format(r, 5, &start, &clock)
            .unwrap();
        match k {
            1 => sheet.write_string(r, 6, "x").unwrap(),
            _ => sheet.write_number(r, 6, id * 10.0).unwrap(),
        };
        sheet.write_boolean(r, 7, k != 1).unwrap();
        sheet.write_number(r, 8, 0.5 * id).unwrap();
        sheet.write_number(r, 10, 100.0 + id).unwrap();
    }
    let other = book.add_worksheet().set_name("Notes").unwrap();
    other.write_string(1, 1, "a").unwrap();
    other.write_string(1, 2, "b").unwrap();
    other.write_number(2, 1, 7.0).unwrap();
    other.write_number(2, 2, 8.0).unwrap();
    book.save(path).unwrap();
}

#[test]
fn a_sheet_with_names_numbers_strings_dates_and_times() {
    let dir = temp_dir("types");
    survey(&dir.join("survey.xlsx"));
    let (session, result) = run(&dir, "GET DATA /TYPE=XLSX /FILE='survey.xlsx'.");
    let data = session.dataset().unwrap();
    assert_eq!(
        names(data),
        [
            "id",
            "First_name",
            "score",
            "born",
            "at",
            "start",
            "mixed",
            "ok",
            "V1st",
            "id_2"
        ]
    );
    // the blank row is left out
    assert_eq!(data.case_count(), 3);
    assert_eq!(numbers(data, "id"), [Some(1.0), Some(2.0), Some(3.0)]);
    assert_eq!(strings(data, "First_name"), ["Ann", "  Bob", "Cy"]);
    assert_eq!(data.variable("First_name").unwrap().string_width(), Some(5));
    assert_eq!(numbers(data, "score"), [Some(1.5), Some(2.0), None]);
    assert_eq!(
        numbers(data, "born"),
        [
            Some(date(2020, 1, 1)),
            Some(date(1999, 12, 31)),
            Some(date(2024, 2, 29))
        ]
    );
    let at = date(2024, 3, 5);
    assert_eq!(
        numbers(data, "at"),
        [
            Some(at + 10.5 * 3600.0),
            Some(at + 11.5 * 3600.0),
            Some(at + 12.5 * 3600.0)
        ]
    );
    assert_eq!(
        numbers(data, "start"),
        [Some(22_500.0), Some(26_100.0), Some(29_700.0)]
    );
    // a string column: numbers are written as short as they read back
    assert_eq!(strings(data, "mixed"), ["10", "x", "30"]);
    assert_eq!(numbers(data, "ok"), [Some(1.0), Some(0.0), Some(1.0)]);
    assert_eq!(
        numbers(data, "id_2"),
        [Some(101.0), Some(102.0), Some(103.0)]
    );
    let format = |name: &str| data.variable(name).unwrap().format().to_string();
    assert_eq!(format("id"), "F8.0");
    assert_eq!(format("score"), "F8.2");
    assert_eq!(format("born"), "DATE11");
    assert_eq!(format("at"), "DATETIME20");
    assert_eq!(format("start"), "TIME8");
    assert_eq!(format("V1st"), "F8.2");
    assert!(result.commands[1].warnings.is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn sheet_range_names_and_spaces() {
    let dir = temp_dir("range");
    survey(&dir.join("survey.xlsx"));
    // the second sheet by name and by number, from its first used cell
    for sheet in ["NAME 'Notes'", "NAME notes", "INDEX 2"] {
        let (session, _) = run(
            &dir,
            &format!("GET DATA /TYPE=XLSX /FILE='survey.xlsx' /SHEET={sheet}."),
        );
        let data = session.dataset().unwrap();
        assert_eq!(names(data), ["a", "b"]);
        assert_eq!(numbers(data, "b"), [Some(8.0)]);
    }
    // a range without names: V and the column's number in the range
    let (session, _) = run(
        &dir,
        "GET DATA /TYPE=XLSX /FILE='survey.xlsx' /CELLRANGE=RANGE 'A2:B4' /READNAMES=OFF\n\
         /LEADINGSPACES IGNORE=YES /TRAILINGSPACES IGNORE=YES.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(names(data), ["V1", "V2"]);
    assert_eq!(numbers(data, "V1"), [Some(1.0), Some(2.0)]);
    assert_eq!(strings(data, "V2"), ["Ann", "Bob"]);
    // without trailing spaces in the width, Bob is the longest
    assert_eq!(data.variable("V2").unwrap().string_width(), Some(3));
    // ASSUMEDSTRWIDTH cuts longer values, with a warning
    let (session, result) = run(
        &dir,
        "GET DATA /TYPE=XLSX /FILE='survey.xlsx' /CELLRANGE=RANGE 'B1:B5' /ASSUMEDSTRWIDTH=3.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(strings(data, "First_name"), ["Ann", "  B", "Cy"]);
    assert_eq!(
        result.commands[1].warnings,
        ["row 4: First_name is cut to 3 bytes"]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn datatypemin_makes_a_mostly_numeric_column_numeric() {
    let dir = temp_dir("typemin");
    survey(&dir.join("survey.xlsx"));
    let (session, result) = run(
        &dir,
        "GET DATA /TYPE=XLSX /FILE='survey.xlsx' /DATATYPEMIN PERCENTAGE=60.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(numbers(data, "mixed"), [Some(10.0), None, Some(30.0)]);
    assert_eq!(
        result.commands[1].warnings,
        ["row 4: mixed is system-missing: the cell holds no number"]
    );
    // 2 of 3 is less than 70 percent: a string
    let (session, _) = run(
        &dir,
        "GET DATA /TYPE=XLSX /FILE='survey.xlsx' /DATATYPEMIN PERCENTAGE=70.",
    );
    assert_eq!(
        strings(session.dataset().unwrap(), "mixed"),
        ["10", "x", "30"]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn hidden_rows_and_columns_can_be_left_out() {
    let dir = temp_dir("hidden");
    let path = dir.join("hidden.xlsx");
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    for (c, h) in ["a", "b", "c"].iter().enumerate() {
        sheet.write_string(0, c as u16, *h).unwrap();
        for r in 1..4 {
            sheet
                .write_number(r, c as u16, f64::from(r) * 10.0 + c as f64)
                .unwrap();
        }
    }
    sheet.set_row_hidden(2).unwrap();
    sheet.set_column_hidden(1).unwrap();
    book.save(&path).unwrap();
    let (session, _) = run(
        &dir,
        "GET DATA /TYPE=XLSX /FILE='hidden.xlsx' /HIDDEN IGNORE=YES.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(names(data), ["a", "c"]);
    assert_eq!(numbers(data, "c"), [Some(12.0), Some(32.0)]);
    let (session, _) = run(
        &dir,
        "GET DATA /TYPE=XLSM /FILE='hidden.xlsx' /HIDDEN IGNORE=NO.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(names(data), ["a", "b", "c"]);
    assert_eq!(data.case_count(), 3);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn errors_name_the_sheet_and_the_subcommand() {
    let dir = temp_dir("errors");
    survey(&dir.join("survey.xlsx"));
    let message = error(
        &dir,
        "GET DATA /TYPE=XLSX /FILE='survey.xlsx' /SHEET=INDEX 3.",
    );
    assert!(
        message.contains("there is no sheet 3; the workbook has 2"),
        "{message}"
    );
    let message = error(
        &dir,
        "GET DATA /TYPE=XLSX /FILE='survey.xlsx' /SHEET=NAME 'Wave 2'.",
    );
    assert!(
        message.contains("the sheets are Wave 1, Notes"),
        "{message}"
    );
    let message = error(
        &dir,
        "GET DATA /TYPE=XLSX /FILE='survey.xlsx' /VARIABLES=a F8.",
    );
    assert_eq!(message, "VARIABLES does not apply to GET DATA /TYPE=XLSX");
    let message = error(
        &dir,
        "GET DATA /TYPE=XLS /FILE='survey.xls' /HIDDEN IGNORE=YES.",
    );
    assert_eq!(message, "HIDDEN does not apply to GET DATA /TYPE=XLS");
    let message = error(
        &dir,
        "GET DATA /TYPE=TXT /FILE='a.csv' /SHEET=INDEX 1 /VARIABLES=a F8.",
    );
    assert_eq!(message, "SHEET does not apply to GET DATA /TYPE=TXT");
    let message = error(
        &dir,
        "GET DATA /TYPE=XLSX /FILE='survey.xlsx' /CELLRANGE=RANGE 'B4:A1'.",
    );
    assert_eq!(message, "'B4:A1' is not a range such as 'A1:K14'");
    std::fs::remove_dir_all(&dir).unwrap();
}
