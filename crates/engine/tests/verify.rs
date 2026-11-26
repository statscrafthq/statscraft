//! VERIFY on invented monthly data: the cases and periods table, the flags on the date
//! variables, and the report on one series. The expected numbers are counted by hand from the
//! data lines below.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Twelve months of a small shop's takings; the fifth month is missing and the ninth was
/// entered as -1 (user-missing).
const SHOP: &str = "DATA LIST LIST /takings (F5.0).
BEGIN DATA
210
190
205
220
.
240
255
250
-1
230
215
260
END DATA.
MISSING VALUES takings (-1).
";

fn run(source: &str) -> (Vec<Table>, Vec<String>) {
    let (commands, errors) = parse(&format!("{SHOP}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables = engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
    }
    (tables, engine.warnings().to_vec())
}

fn error(source: &str) -> String {
    let (commands, errors) = parse(&format!("{SHOP}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            return e.to_string();
        }
    }
    panic!("no error")
}

fn find<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables.iter().find(|t| t.title == title).unwrap_or_else(|| {
        panic!(
            "no table {title}: {:?}",
            tables.iter().map(|t| &t.title).collect::<Vec<_>>()
        )
    })
}

fn text(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        Cell::Number(v) => v.to_string(),
        Cell::Empty => String::new(),
    }
}

/// Row `label` of a table as texts.
fn row(table: &Table, label: &str) -> Vec<String> {
    table
        .rows
        .iter()
        .find(|r| text(&r[0]) == label)
        .unwrap_or_else(|| panic!("no row {label}"))
        .iter()
        .map(text)
        .collect()
}

#[test]
fn clean_file_has_no_problem() {
    let (tables, warnings) = run("DATE YEAR 2020 MONTH 1.\nVERIFY.");
    assert!(warnings.is_empty(), "{warnings:?}");
    let cases = find(&tables, "Cases and periods");
    assert_eq!(
        row(cases, "File, first case"),
        ["File, first case", "1", "JAN 2020"]
    );
    assert_eq!(
        row(cases, "File, last case"),
        ["File, last case", "12", "DEC 2020"]
    );
    // without USE the period is the whole file, and without PREDICT there is none
    assert_eq!(row(cases, "USE, first case")[1], "1");
    assert_eq!(row(cases, "USE, last case")[1], "12");
    assert!(
        cases
            .rows
            .iter()
            .all(|r| !text(&r[0]).starts_with("PREDICT"))
    );
    let dates = find(&tables, "Date variables");
    for name in ["YEAR_", "MONTH_"] {
        assert_eq!(row(dates, name)[1..], ["0", "no problem"], "{name}");
    }
    // a quarter follows its month and is nested in the year
    let (tables, _) = run("DATE YEAR 2020 QUARTER MONTH.\nVERIFY.");
    let dates = find(&tables, "Date variables");
    for name in ["YEAR_", "QUARTER_", "MONTH_"] {
        assert_eq!(row(dates, name)[1..], ["0", "no problem"], "{name}");
    }
    assert!(tables.iter().all(|t| !t.title.starts_with("Series")));
}

#[test]
fn use_and_predict_periods_by_case_and_date() {
    let (tables, _) = run("DATE YEAR 2020 MONTH 1.\nUSE 3 THRU 10.\nPREDICT 11 THRU 14.\nVERIFY.");
    let cases = find(&tables, "Cases and periods");
    assert_eq!(row(cases, "USE, first case")[1..], ["3", "MAR 2020"]);
    assert_eq!(row(cases, "USE, last case")[1..], ["10", "OCT 2020"]);
    assert_eq!(row(cases, "PREDICT, first case")[1..], ["11", "NOV 2020"]);
    // the last forecast lies past the file: DATE would give it February of the next year
    assert_eq!(row(cases, "PREDICT, last case")[1..], ["14", "FEB 2021"]);
}

#[test]
fn series_values_and_missing_count() {
    let (tables, _) = run(
        "DATE YEAR 2020 MONTH 1.\nUSE 5 THRU 9.\nPREDICT 10 THRU 13.\nVERIFY VARIABLES=takings.",
    );
    let series = find(&tables, "Series takings");
    assert_eq!(row(series, "File, first case")[1..], ["1", "210"]);
    assert_eq!(row(series, "File, last case")[1..], ["12", "260"]);
    // the USE period begins and ends on a missing value
    assert_eq!(row(series, "USE, first case")[1..], ["5", "missing"]);
    assert_eq!(row(series, "USE, last case")[1..], ["9", "missing"]);
    assert_eq!(row(series, "PREDICT, first case")[1..], ["10", "230"]);
    assert_eq!(
        row(series, "PREDICT, last case")[1..],
        ["13", "not in the file"]
    );
    // one system-missing and one user-missing value
    assert_eq!(row(series, "Cases with a missing value")[2], "2");
    let (tables, _) = run("DATE YEAR 2020 MONTH 1.\nTSET MISSING=INCLUDE.\nVERIFY takings.");
    let series = find(&tables, "Series takings");
    assert_eq!(row(series, "Cases with a missing value")[2], "1");
    assert_eq!(row(series, "File, first case")[2], "210");
}

#[test]
fn a_dropped_case_makes_a_step_and_the_file_ends_early() {
    // case 5 removed: April is followed by June; the last two months go too
    let (tables, _) = run(
        "DATE YEAR 2020 MONTH 1.\nCOMPUTE n = $CASENUM.\nSELECT IF (n <> 5 AND n < 11).\nVERIFY.",
    );
    let cases = find(&tables, "Cases and periods");
    assert_eq!(row(cases, "File, last case")[1..], ["9", "OCT 2020"]);
    let dates = find(&tables, "Date variables");
    let month = row(dates, "MONTH_");
    assert_eq!(month[1], "0");
    assert!(
        month[2].contains("steps from 4 to 6 at case 5"),
        "{month:?}"
    );
    assert!(month[2].contains("ends at 10, not 12"), "{month:?}");
    assert!(!month[2].contains("begins"), "{month:?}");
    assert_eq!(row(dates, "YEAR_")[2], "no problem");
}

#[test]
fn a_file_that_starts_late_and_wraps_the_year() {
    // January and February removed: the file begins in March
    let (tables, _) =
        run("DATE YEAR 2020 MONTH 1.\nCOMPUTE n = $CASENUM.\nSELECT IF (n > 2).\nVERIFY.");
    let month = row(find(&tables, "Date variables"), "MONTH_");
    assert_eq!(month[2], "* begins at 3, not 1");
}

#[test]
fn year_changing_inside_a_cycle_is_not_nested() {
    let (tables, _) = run("DATE YEAR 2020 MONTH 1.\nIF (MONTH_ >= 7) YEAR_ = 2021.\nVERIFY.");
    let dates = find(&tables, "Date variables");
    let month = row(dates, "MONTH_");
    assert!(
        month[2].contains("not nested in YEAR_ at case 7"),
        "{month:?}"
    );
    // the year itself moves by one, so its own step is fine
    assert_eq!(row(dates, "YEAR_")[2], "no problem");
}

#[test]
fn invalid_date_values_are_counted() {
    let (tables, _) = run("DATE YEAR 2020 MONTH 1.\nIF (MONTH_ = 4) MONTH_ = 13.\nVERIFY.");
    let month = row(find(&tables, "Date variables"), "MONTH_");
    assert_eq!(month[1], "1");
    assert!(month[2].contains("steps from 3 to 13"), "{month:?}");
}

#[test]
fn without_date_variables_only_the_cases_are_listed() {
    let (tables, warnings) = run("VERIFY.");
    assert!(tables.iter().all(|t| t.title != "Date variables"));
    assert_eq!(
        row(find(&tables, "Cases and periods"), "File, last case")[1..],
        ["12", ""]
    );
    assert!(
        warnings.iter().any(|w| w.contains("no DATE variables")),
        "{warnings:?}"
    );
}

#[test]
fn the_series_must_be_a_numeric_variable() {
    assert!(error("DATE YEAR 2020 MONTH 1.\nVERIFY DATE_.").contains("numeric"));
    assert!(error("VERIFY nothing.").contains("nothing"));
}
