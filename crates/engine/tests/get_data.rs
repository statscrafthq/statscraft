//! GET DATA /TYPE=TXT on made-up text files.

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::Session;

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-getdata-{tag}-{}", std::process::id()));
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

#[test]
fn delimited_text_with_a_qualifier_header_and_formats() {
    let dir = temp_dir("delimited");
    // a header line, a qualified text with a comma and a line break, a datetime, an empty value
    std::fs::write(
        dir.join("survey.dat"),
        "id,started,comment,score\n\
         1,'2024-03-05 10:30:15','fine, thanks',4\n\
         2,'2024-03-06 08:00:00','two\nlines',\n\
         3,'2024-03-07 09:15:00','it''s ok',5\n",
    )
    .unwrap();
    let mut session = Session::default();
    session.set_base_dir(Some(dir.clone()));
    let source = format!(
        "CD '{}'.\n\
         GET DATA /TYPE=TXT /FILE='survey.dat' /DELCASE=LINE /DELIMITERS=\",\" /QUALIFIER=\"'\"\n\
           /ARRANGEMENT=DELIMITED /FIRSTCASE=2 /IMPORTCASE=ALL\n\
           /VARIABLES=id F3 started DATETIME23.2 comment A20 score F1.",
        dir.display()
    );
    session
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let data = session.dataset().unwrap();
    assert_eq!(data.case_count(), 3);
    assert_eq!(numbers(data, "id"), [Some(1.0), Some(2.0), Some(3.0)]);
    assert_eq!(
        strings(data, "comment"),
        ["fine, thanks", "two\nlines", "it's ok"]
    );
    assert_eq!(numbers(data, "score"), [Some(4.0), None, Some(5.0)]);
    let started = numbers(data, "started");
    assert_eq!(
        started[1].unwrap() - started[0].unwrap(),
        86_400.0 - 2.0 * 3600.0 - 30.0 * 60.0 - 15.0
    );
    assert_eq!(data.variable("started").unwrap().format(), "DATETIME23.2");
    assert_eq!(data.variable("id").unwrap().format(), "F3.0");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn fixed_columns_auto_formats_skips_and_first_cases() {
    let dir = temp_dir("fixed");
    std::fs::write(
        dir.join("fixed.txt"),
        "01Ann  x12.5\n02Bo   y 3\n03Cy   z  \n",
    )
    .unwrap();
    let file = dir.join("fixed.txt");
    let mut session = Session::default();
    let source = format!(
        "GET DATA /TYPE=TXT /FILE='{}' /ARRANGEMENT=FIXED /IMPORTCASE=FIRST 2\n\
           /VARIABLES=id 0-1 F2 name 2-6 AUTO flag 7-7 X amount 8-11 AUTO.",
        file.display()
    );
    session
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let data = session.dataset().unwrap();
    assert_eq!(data.case_count(), 2);
    assert_eq!(strings(data, "name"), ["Ann", "Bo"]);
    assert_eq!(data.variable("name").unwrap().format(), "A3");
    assert!(data.variable("flag").is_err());
    assert_eq!(numbers(data, "amount"), [Some(12.5), Some(3.0)]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn values_per_case_across_lines_and_other_types_refused() {
    let dir = temp_dir("delcase");
    std::fs::write(dir.join("v.txt"), "1 2 3\n4 5 6 7\n").unwrap();
    let file = dir.join("v.txt");
    let mut session = Session::default();
    let source = format!(
        "GET DATA /TYPE=TXT /FILE='{}' /DELCASE=VARIABLES 2 /DELIMITERS=\" \" /VARIABLES=a F1 b F1.",
        file.display()
    );
    session
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let data = session.dataset().unwrap();
    assert_eq!(numbers(data, "a"), [Some(1.0), Some(3.0), Some(5.0)]);
    assert_eq!(numbers(data, "b"), [Some(2.0), Some(4.0), Some(6.0)]);
    let error = session
        .run("GET DATA /TYPE=XLSX /FILE='a.xlsx' /SHEET=INDEX 1.", None)
        .unwrap_err();
    assert!(
        error[0].message.contains("TYPE=XLSX is not supported yet"),
        "{error:?}"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
