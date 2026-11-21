//! MATRIX=IN and MATRIX=OUT of REGRESSION, PARTIAL CORR, RELIABILITY and ONEWAY on synthetic
//! data. Every procedure is checked by a round trip: the tables that need no cases come out
//! the same from the matrix its own run wrote as from the data; and the matrix of a MATRIX
//! DATA command with invented numbers gives what a textbook computation from the covariance
//! matrix gives (done in R).

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

pub fn run(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

pub fn run_error(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.0;
        }
    }
    panic!("no error");
}

/// The tables of `title` hold the same numbers (and the same text) in both runs.
pub fn same(a: &[Table], b: &[Table], title: &str) {
    let find = |t: &'_ [Table]| -> Vec<Vec<Cell>> {
        t.iter()
            .filter(|t| t.title == title)
            .flat_map(|t| t.rows.clone())
            .collect()
    };
    let (x, y) = (find(a), find(b));
    assert!(!x.is_empty(), "no table {title}");
    assert_eq!(x.len(), y.len(), "{title}: rows");
    for (r, (ra, rb)) in x.iter().zip(&y).enumerate() {
        for (c, (ca, cb)) in ra.iter().zip(rb).enumerate() {
            match (ca, cb) {
                (Cell::Number(p), Cell::Number(q)) => assert!(
                    (p - q).abs() <= 1e-9 * p.abs().max(1.0),
                    "{title} row {r} column {c}: {p} against {q}"
                ),
                (p, q) => assert_eq!(
                    format!("{p:?}"),
                    format!("{q:?}"),
                    "{title} row {r} column {c}"
                ),
            }
        }
    }
}

pub fn value(engine: &Engine, name: &str, case: usize) -> Option<f64> {
    let data = engine.dataset().expect("data");
    match data.variable(name).expect("variable").values() {
        Values::Numeric(v) => v.get(case),
        Values::String(_) => None,
    }
}

pub fn texts(engine: &Engine, name: &str) -> Vec<String> {
    let data = engine.dataset().expect("data");
    match data.variable(name).expect("variable").values() {
        Values::String(v) => (0..data.case_count())
            .map(|k| v.get(k).trim().to_string())
            .collect(),
        Values::Numeric(_) => panic!("{name} is numeric"),
    }
}

pub fn number(tables: &[Table], title: &str, row: usize, column: &str) -> f64 {
    let table = tables.iter().find(|t| t.title == title).expect("table");
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", table.column_titles()));
    match &table.rows[row][c] {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    }
}

const PAY: &str = "DATA LIST FREE /y x1 x2 g.
BEGIN DATA
12.1 3 5.5 1  9.4 2 4.1 1 15.0 5 6.2 1 11.2 4 5.0 1 13.8 4 7.1 1 10.3 3 4.4 1
 8.9 1 3.9 2 14.2 5 6.8 2 10.9 3 5.9 2 16.5 6 7.7 2 12.6 4 6.0 2  9.1 2 4.7 2
END DATA.
";

const PAY_GAPS: &str = "DATA LIST FREE /y x1 x2 g.
BEGIN DATA
12.1 3 5.5 1  9.4 2 4.1 1 15.0 5 . 1 11.2 4 5.0 1 13.8 . 7.1 1 10.3 3 4.4 1
 8.9 1 3.9 2 14.2 5 6.8 2 10.9 3 5.9 2 16.5 6 7.7 2 12.6 4 6.0 2  9.1 2 4.7 2
END DATA.
";

const EQUATION: &str = "/VARIABLES=y x1 x2 /DEPENDENT=y /METHOD=ENTER x1 x2.";

fn compare_round_trip(data: &str, lead: &str, equation: &str) {
    let direct = run(
        &mut Engine::default(),
        &format!("{data}REGRESSION {lead} {equation}"),
    );
    let mut engine = Engine::default();
    let _ = run(
        &mut engine,
        &format!("{data}REGRESSION MATRIX OUT(*) {lead} {equation}"),
    );
    // the matrix replaced the active dataset
    assert_eq!(texts(&engine, "ROWTYPE_")[0], "MEAN");
    let again = run(
        &mut engine,
        &format!("REGRESSION MATRIX IN(*) {lead} {equation}"),
    );
    for title in ["Model Summary", "ANOVA", "Coefficients"] {
        same(&direct, &again, title);
    }
}

#[test]
fn regression_matrix_round_trip_listwise() {
    compare_round_trip(PAY, "", EQUATION);
}

#[test]
fn regression_matrix_round_trip_through_the_origin() {
    let mut engine = Engine::default();
    let direct = run(&mut engine, &format!("{PAY}REGRESSION /ORIGIN {EQUATION}"));
    let _ = run(
        &mut engine,
        &format!("REGRESSION MATRIX=OUT(*) /ORIGIN {EQUATION}"),
    );
    let kinds = texts(&engine, "ROWTYPE_");
    assert!(
        kinds.contains(&"OCORR".to_string()) && !kinds.contains(&"CORR".to_string()),
        "{kinds:?}"
    );
    let again = run(
        &mut engine,
        &format!("REGRESSION MATRIX=IN(*) /ORIGIN {EQUATION}"),
    );
    for title in ["Model Summary", "ANOVA", "Coefficients"] {
        same(&direct, &again, title);
    }
}

#[test]
fn regression_matrix_round_trip_pairwise_writes_the_matrix_of_counts() {
    let mut engine = Engine::default();
    let direct = run(
        &mut engine,
        &format!("{PAY_GAPS}REGRESSION /MISSING=PAIRWISE {EQUATION}"),
    );
    let _ = run(
        &mut engine,
        &format!("REGRESSION MATRIX=OUT(*) /MISSING=PAIRWISE {EQUATION}"),
    );
    // pairwise counts differ, so N comes as a matrix with a record for each variable
    let kinds = texts(&engine, "ROWTYPE_");
    assert_eq!(kinds.iter().filter(|k| *k == "N").count(), 3, "{kinds:?}");
    let again = run(
        &mut engine,
        &format!("REGRESSION MATRIX=IN(*) /MISSING=PAIRWISE {EQUATION}"),
    );
    for title in ["Model Summary", "ANOVA", "Coefficients"] {
        same(&direct, &again, title);
    }
}

#[test]
fn regression_matrix_round_trip_under_split_file() {
    let split = "SORT CASES BY g.\nSPLIT FILE LAYERED BY g.\n";
    let mut engine = Engine::default();
    let direct = run(&mut engine, &format!("{PAY}{split}REGRESSION {EQUATION}"));
    let _ = run(&mut engine, &format!("REGRESSION MATRIX=OUT(*) {EQUATION}"));
    // the split variable leads, and each group has its own records
    let data = engine.dataset().unwrap();
    assert_eq!(data.variable_names().next().unwrap(), "g");
    assert_eq!(
        texts(&engine, "ROWTYPE_")
            .iter()
            .filter(|k| *k == "MEAN")
            .count(),
        2
    );
    let again = run(&mut engine, &format!("REGRESSION MATRIX=IN(*) {EQUATION}"));
    for title in ["Model Summary", "ANOVA", "Coefficients"] {
        same(&direct, &again, title);
    }
}

#[test]
fn regression_matrix_in_from_a_file_with_the_matrix_of_the_run() {
    let dir = std::env::temp_dir().join(format!("sc-regmat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("m.sav");
    let path = file.to_str().unwrap().replace('\\', "/");
    let mut engine = Engine::default();
    let direct = run(
        &mut engine,
        &format!("{PAY}REGRESSION MATRIX OUT('{path}') {EQUATION}"),
    );
    // the data stay active; the matrix is read from the file
    let again = run(
        &mut engine,
        &format!("REGRESSION MATRIX IN('{path}') {EQUATION}"),
    );
    for title in ["Model Summary", "ANOVA", "Coefficients"] {
        same(&direct, &again, title);
    }
    let _ = std::fs::remove_dir_all(dir);
}

// A matrix made by MATRIX DATA from invented summary numbers: the coefficients solve the
// normal equations of the covariance matrix (computed in R).
#[test]
fn regression_from_matrix_data_gives_the_textbook_solution() {
    let t = run(
        &mut Engine::default(),
        "MATRIX DATA VARIABLES=ROWTYPE_ y x1 x2 /FORMAT=FULL.
BEGIN DATA
MEAN 10 5 3
STDDEV 2 1.5 1
N 40 40 40
CORR 1 .6 .3
CORR .6 1 .2
CORR .3 .2 1
END DATA.
REGRESSION MATRIX=IN(*) /VARIABLES=y x1 x2 /DEPENDENT=y /METHOD=ENTER x1 x2.",
    );
    let coef = |row: usize, column: &str| number(&t, "Coefficients", row, column);
    let b = "Unstandardized Coefficients / B";
    let se = "Unstandardized Coefficients / Std. Error";
    for (row, expected_b, expected_se) in [
        (0, 5.125, 1.078809566179),
        (1, 0.75, 0.174191971235),
        (2, 0.375, 0.261287956853),
    ] {
        assert!((coef(row, b) - expected_b).abs() < 1e-9, "b {row}");
        assert!((coef(row, se) - expected_se).abs() < 1e-9, "se {row}");
    }
    assert!((number(&t, "Model Summary", 0, "R Square") - 0.39375).abs() < 1e-12);
    assert!((number(&t, "ANOVA", 0, "F") - 12.015463917526).abs() < 1e-9);
}

#[test]
fn regression_matrix_in_refuses_what_needs_cases() {
    for (extra, name) in [
        ("/SAVE PRED", "SAVE"),
        ("/RESIDUALS DURBIN", "RESIDUALS"),
        ("/CASEWISE", "CASEWISE"),
        ("/SCATTERPLOT=(y,x1)", "SCATTERPLOT"),
    ] {
        let error = run_error(&format!(
            "{PAY}REGRESSION MATRIX OUT(*) {EQUATION}\nREGRESSION MATRIX IN(*) /VARIABLES=y x1 x2 /DEPENDENT=y /METHOD=ENTER x1 x2 {extra}."
        ));
        assert!(
            error.contains(name) && error.contains("not available"),
            "{error}"
        );
    }
}

// PARTIAL CORR: the same round trips, a matrix written by REGRESSION read by it, and the
// first-order partial of invented correlations (computed in R).
const PARTIAL: &str = "/VARIABLES=x1 x2 BY y(1) /STATISTICS=DESCRIPTIVES CORR";

#[test]
fn partial_corr_matrix_out_holds_the_means_counts_and_partials_of_the_run() {
    for (data, missing, pairs) in [(PAY, "", 1), (PAY_GAPS, "/MISSING=ANALYSIS", 2)] {
        let mut engine = Engine::default();
        let direct = run(
            &mut engine,
            &format!("{data}PARTIAL CORR {PARTIAL} {missing}."),
        );
        let _ = run(
            &mut engine,
            &format!("PARTIAL CORR MATRIX=OUT(*) {PARTIAL} {missing}."),
        );
        let kinds = texts(&engine, "ROWTYPE_");
        let order: Vec<&str> = kinds.iter().map(String::as_str).collect();
        // the variables are the VARIABLES list, not the control; N is a record when all pairs
        // agree and a matrix of records when pairwise counts differ
        assert_eq!(order[..2], ["MEAN", "STDDEV"], "{kinds:?}");
        assert_eq!(
            order.iter().filter(|k| **k == "N").count(),
            if pairs == 1 { 1 } else { 2 }
        );
        assert_eq!(order.iter().filter(|k| **k == "PCORR").count(), 2);
        let coefficient = |t: &[Table], row: usize, column: usize| {
            let table = t.iter().find(|t| t.title == "Correlations").unwrap();
            match &table.rows[row][table.row_headers + column] {
                Cell::Number(n) => *n,
                other => panic!("{other:?}"),
            }
        };
        // the zero-order block has 9 rows (3 variables, 3 statistics each); the first row after it is x1 given y
        let pcorr = kinds.iter().position(|k| k == "PCORR").unwrap();
        let from_file = value(&engine, "x2", pcorr).unwrap();
        assert!((from_file - coefficient(&direct, 9, 1)).abs() < 1e-12);
        let mean = number(&direct, "Descriptive Statistics", 0, "Mean");
        assert!((value(&engine, "x1", 0).unwrap() - mean).abs() < 1e-12);
    }
}

#[test]
fn partial_corr_matrix_out_under_split_file_has_a_set_per_group() {
    let mut engine = Engine::default();
    let _ = run(
        &mut engine,
        &format!(
            "{PAY}SORT CASES BY g.\nSPLIT FILE LAYERED BY g.\nPARTIAL CORR MATRIX=OUT(*) {PARTIAL}."
        ),
    );
    assert_eq!(
        texts(&engine, "ROWTYPE_")
            .iter()
            .filter(|k| *k == "MEAN")
            .count(),
        2
    );
    assert_eq!(
        engine.dataset().unwrap().variable_names().next().unwrap(),
        "g"
    );
}

// A matrix of partials read again: the partial of a partial is the higher-order partial, so
// the coefficient equals the second-order partial of the data (the degrees of freedom cannot,
// the file does not say what was held fixed before).
#[test]
fn partial_corr_reads_its_own_matrix_as_correlations() {
    let data = "DATA LIST FREE /a b c d.
BEGIN DATA
1 2 3 4  2 1 4 3  3 5 3 6  4 3 6 5  5 6 5 8  6 4 8 7  7 8 6 9  8 7 9 8
9 9 7 12 10 8 11 10 11 12 9 13 12 10 13 12
END DATA.
";
    let mut engine = Engine::default();
    let direct = run(
        &mut engine,
        &format!("{data}PARTIAL CORR /VARIABLES=a b BY c d(2)."),
    );
    let _ = run(
        &mut engine,
        "PARTIAL CORR MATRIX=OUT(*) /VARIABLES=a b c BY d(1).",
    );
    let again = run(
        &mut engine,
        "PARTIAL CORR MATRIX=IN(*) /VARIABLES=a b BY c(1).",
    );
    let at = |t: &[Table]| {
        let table = t.iter().find(|t| t.title == "Correlations").unwrap();
        match &table.rows[0][table.row_headers + 1] {
            Cell::Number(n) => *n,
            other => panic!("{other:?}"),
        }
    };
    assert!(
        (at(&direct) - at(&again)).abs() < 1e-10,
        "{} {}",
        at(&direct),
        at(&again)
    );
}

#[test]
fn partial_corr_reads_the_matrix_regression_wrote() {
    let mut engine = Engine::default();
    let direct = run(
        &mut engine,
        &format!("{PAY}PARTIAL CORR /VARIABLES=x1 x2 BY y(1)."),
    );
    let _ = run(&mut engine, &format!("REGRESSION MATRIX=OUT(*) {EQUATION}"));
    let again = run(
        &mut engine,
        "PARTIAL CORR MATRIX=IN(*) /VARIABLES=x1 x2 BY y(1).",
    );
    same(&direct, &again, "Correlations");
}

#[test]
fn partial_corr_from_matrix_data_gives_the_textbook_partial() {
    let t = run(
        &mut Engine::default(),
        "MATRIX DATA VARIABLES=ROWTYPE_ x y z /FORMAT=FULL.
BEGIN DATA
N 30 30 30
CORR 1 .5 .4
CORR .5 1 .3
CORR .4 .3 1
END DATA.
PARTIAL CORR MATRIX=IN(*) /VARIABLES=x y BY z(1).",
    );
    let table = t.iter().find(|t| t.title == "Correlations").unwrap();
    // rows: x Correlation, Significance, df; the column of y is the third value column
    let at = |row: usize| match &table.rows[row][table.row_headers + 1] {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    };
    assert!((at(0) - 0.434633560328).abs() < 1e-10);
    assert!((at(1) - 0.018468425951).abs() < 1e-9);
    assert_eq!(at(2), 27.0);
}
