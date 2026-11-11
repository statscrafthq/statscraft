//! DATA LIST with inline data and with a data file. The expected values agree with the
//! reference program; `reference_data.rs` repeats the comparison on saved files.

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::Session;

fn run(source: &str) -> Session {
    let mut session = Session::default();
    if let Err(errors) = session.run(source, None) {
        panic!("{errors:?}");
    }
    session
}

/// The values of a variable: numbers with `.` for system-missing, strings between `|`.
fn show(data: &Dataset, name: &str) -> String {
    match data.variable(name).unwrap().values() {
        Values::Numeric(values) => values
            .iter()
            .map(|v| v.map_or(".".to_string(), |v| v.to_string()))
            .collect::<Vec<_>>()
            .join(" "),
        Values::String(values) => values.iter().collect::<Vec<_>>().join("|"),
    }
}

fn format(data: &Dataset, name: &str) -> String {
    data.variable(name).unwrap().format().to_string()
}

#[test]
fn fixed_columns_imply_decimals_and_keep_string_blanks() {
    let source = "\
DATA LIST /id 1-3 sex 5-6 (A) age 7-8 op1 TO op3 10-12 rate 14-18 (PCT,2) cost 20-24 (DOLLAR).
BEGIN DATA
001 m 28 122   935 78788
002  f 9 2 2  1.5    12
  3 x 4a     -201  $1,0
END DATA.";
    let mut session = Session::default();
    let result = session.run(source, None).unwrap();
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "id"), "1 2 3");
    assert_eq!(show(data, "sex"), "m| f|x");
    assert_eq!(show(data, "age"), "28 9 .");
    assert_eq!(show(data, "op1"), "1 2 .");
    assert_eq!(show(data, "op2"), "2 . .");
    assert_eq!(show(data, "rate"), "9.35 1.5 -2.01");
    assert_eq!(show(data, "cost"), "78788 12 10");
    let formats: Vec<String> = ["id", "sex", "op1", "rate", "cost"]
        .iter()
        .map(|n| format(data, n))
        .collect();
    assert_eq!(formats, ["F3.0", "A2", "F1.0", "PCT7.2", "DOLLAR7.0"]);
    // the warning names the data line in the syntax
    let warnings = &result.commands[1].warnings;
    assert_eq!(
        warnings,
        &["line 5: age is system-missing: '4a' is not a number"]
    );
}

#[test]
fn several_records_per_case_and_format_lists() {
    let source = "\
DATA LIST RECORDS=2 /1 name (A6) score (T9, F4.1) /2 born 1-10 (ADATE) w (1X, COMMA6.2).
BEGIN DATA
Ann     123
01/31/1990 1,2345
  Bob   99.5
2/3/1985    7
END DATA.";
    let session = run(source);
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "name"), "Ann|  Bob");
    assert_eq!(show(data, "score"), "12.3 99.5");
    let days = |v: f64| v / 86400.0;
    let born = data.variable("born").unwrap().numeric().unwrap();
    assert_eq!(days(born.get(0).unwrap()), 148_763.0);
    assert_eq!(days(born.get(1).unwrap()), 146_940.0);
    assert_eq!(show(data, "w"), "123.45 0.07");
    assert_eq!(format(data, "w"), "COMMA8.2");
}

#[test]
fn list_reads_one_case_per_line() {
    let source = "\
DATA LIST LIST /a b (F2) s (A4).
BEGIN DATA
1 2 abc
3,,\"x y\"
 4 , 5 , 'q r'

6 7 toolong
8
9 10 z extra
END DATA.";
    let mut session = Session::default();
    let result = session.run(source, None).unwrap();
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "a"), "1 3 4 . 6 8 9");
    assert_eq!(show(data, "b"), "2 . 5 . 7 . 10");
    assert_eq!(show(data, "s"), "abc|x y|q r||tool||z");
    assert_eq!(format(data, "a"), "F2.0");
    assert_eq!(format(data, "b"), "F2.0");
    let warnings = &result.commands[1].warnings;
    assert_eq!(
        warnings,
        &[
            "line 6: 0 of 3 values; the others are missing",
            "line 7: s is cut to 4 bytes: toolong",
            "line 8: 1 of 3 values; the others are missing",
            "line 9: 4 values for 3 variables; the rest is ignored",
        ]
    );
}

#[test]
fn free_fills_cases_across_lines_and_drops_a_partial_last_case() {
    let source = "\
DATA LIST FREE (',') SKIP=1 /a b (F2) s (A3).
BEGIN DATA
a header line
1,2,abc,3
4,x,5,6
7,,
END DATA.";
    let mut session = Session::default();
    let result = session.run(source, None).unwrap();
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "a"), "1 3 5");
    assert_eq!(show(data, "b"), "2 4 6");
    assert_eq!(show(data, "s"), "abc|x|7");
    assert_eq!(
        result.commands[1].warnings,
        ["line 6: the last case has 1 of its 3 values and is left out"]
    );
}

#[test]
fn data_from_a_file() {
    let dir = std::env::temp_dir().join(format!("statscraft-inline-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("scores.txt");
    std::fs::write(&file, "1 10\n2 20\n3 x\n").unwrap();
    let source = format!("DATA LIST FILE='{}' FREE /id score.", file.display());
    let mut session = Session::default();
    let result = session.run(&source, None).unwrap();
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "score"), "10 20 .");
    let warning = &result.commands[0].warnings[0];
    assert!(
        warning.starts_with("record 3 of ")
            && warning.ends_with("score is system-missing: 'x' is not a number"),
        "{warning}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn inline_data_must_follow_its_data_list() {
    let mut session = Session::default();
    // transformations in between run after the data are read
    session
        .run(
            "DATA LIST FREE /x.\nCOMPUTE y = x * 2.\nBEGIN DATA\n1 2\nEND DATA.",
            None,
        )
        .unwrap();
    let y = session.dataset().unwrap().variable("y").unwrap();
    assert_eq!(
        y.numeric().unwrap().iter().collect::<Vec<_>>(),
        [Some(2.0), Some(4.0)]
    );
    // a command that reads the data cannot come before them
    let errors = session
        .run("DATA LIST FREE /x.\nLIST.\nBEGIN DATA\n1\nEND DATA.", None)
        .unwrap_err();
    assert_eq!(errors[0].line, Some(2));
    assert!(
        errors[0]
            .message
            .starts_with("the DATA LIST before this command has no data yet")
    );
    let errors = session.run("BEGIN DATA\n1\nEND DATA.", None).unwrap_err();
    assert!(
        errors[0]
            .message
            .starts_with("BEGIN DATA needs a DATA LIST or MATRIX DATA without FILE")
    );
}
