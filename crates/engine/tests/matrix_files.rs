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

// RELIABILITY: the scale statistics come from N, MEAN, STDDEV and CORR alone, so every table
// that does not look at cases comes out the same from the matrix of its own run.
const ITEMS: &str = "DATA LIST FREE /a b c d g.
BEGIN DATA
1 2 3 4 1  2 1 4 3 1  3 5 3 6 1  4 3 6 5 1  5 6 5 8 1  6 4 8 7 1
7 8 6 9 2  8 7 9 8 2  9 9 7 12 2 10 8 11 10 2 11 12 9 13 2 12 10 13 12 2
END DATA.
";

const SCALE_REQUEST: &str = "/VARIABLES=a b c d /STATISTICS=DESCRIPTIVES SCALE ANOVA HOTELLING /SUMMARY=TOTAL MEANS VARIANCE CORRELATIONS /ICC=MODEL(MIXED)";

#[test]
fn reliability_matrix_round_trip() {
    let mut engine = Engine::default();
    let direct = run(&mut engine, &format!("{ITEMS}RELIABILITY {SCALE_REQUEST}"));
    let _ = run(
        &mut engine,
        &format!("RELIABILITY {SCALE_REQUEST} /MATRIX=OUT(*) NOPRINT."),
    );
    let kinds = texts(&engine, "ROWTYPE_");
    assert_eq!(&kinds[..3], ["N", "MEAN", "STDDEV"], "{kinds:?}");
    assert_eq!(kinds.iter().filter(|k| *k == "CORR").count(), 4);
    let again = run(
        &mut engine,
        &format!("RELIABILITY {SCALE_REQUEST} /MATRIX=IN(*)."),
    );
    let mut compared = 0;
    for t in direct
        .iter()
        .filter(|t| t.title != "Case Processing Summary")
    {
        same(&direct, &again, &t.title);
        compared += 1;
    }
    assert!(compared >= 6, "{compared}");
}

#[test]
fn reliability_matrix_out_with_noprint_shows_nothing_and_without_it_the_analysis() {
    let mut engine = Engine::default();
    let quiet = run(
        &mut engine,
        &format!("{ITEMS}RELIABILITY /VARIABLES=a b c d /MATRIX=OUT(*) NOPRINT."),
    );
    assert!(quiet.is_empty());
    let mut engine = Engine::default();
    let shown = run(
        &mut engine,
        &format!("{ITEMS}RELIABILITY /VARIABLES=a b c d /MATRIX=OUT(*)."),
    );
    assert!(shown.iter().any(|t| t.title == "Reliability Statistics"));
}

#[test]
fn reliability_matrix_round_trip_under_split_file() {
    let mut engine = Engine::default();
    let direct = run(
        &mut engine,
        &format!("{ITEMS}SORT CASES BY g.\nSPLIT FILE LAYERED BY g.\nRELIABILITY {SCALE_REQUEST}"),
    );
    let _ = run(
        &mut engine,
        &format!("RELIABILITY {SCALE_REQUEST} /MATRIX=OUT(*) NOPRINT."),
    );
    assert_eq!(
        texts(&engine, "ROWTYPE_")
            .iter()
            .filter(|k| *k == "MEAN")
            .count(),
        2
    );
    let again = run(
        &mut engine,
        &format!("RELIABILITY {SCALE_REQUEST} /MATRIX=IN(*)."),
    );
    for t in direct
        .iter()
        .filter(|t| t.title == "Reliability Statistics")
    {
        same(&direct, &again, &t.title);
    }
}

#[test]
fn reliability_from_matrix_data_gives_the_textbook_alpha() {
    let t = run(
        &mut Engine::default(),
        "MATRIX DATA VARIABLES=ROWTYPE_ a b c d /FORMAT=FULL.
BEGIN DATA
N 40 40 40 40
MEAN 5 6 7 8
STDDEV 1 2 1.5 2.5
CORR 1 .5 .4 .3
CORR .5 1 .6 .2
CORR .4 .6 1 .5
CORR .3 .2 .5 1
END DATA.
RELIABILITY /VARIABLES=a b c d /MATRIX=IN(*).",
    );
    assert!(
        (number(&t, "Reliability Statistics", 0, "Cronbach's Alpha") - 0.679975801573).abs()
            < 1e-10
    );
    assert!(
        (number(
            &t,
            "Reliability Statistics",
            0,
            "Cronbach's Alpha Based on Standardized Items"
        ) - 0.740740740741)
            .abs()
            < 1e-10
    );
}

#[test]
fn reliability_matrix_in_refuses_what_needs_cases_and_what_the_file_lacks() {
    let error = run_error(&format!(
        "{ITEMS}RELIABILITY /VARIABLES=a b c d /MATRIX=OUT(*) NOPRINT.\nRELIABILITY /VARIABLES=a b c d /STATISTICS=TUKEY /MATRIX=IN(*)."
    ));
    assert!(
        error.contains("Tukey") && error.contains("not available"),
        "{error}"
    );
    let error = run_error(
        "MATRIX DATA VARIABLES=ROWTYPE_ a b /FORMAT=FULL.
BEGIN DATA
MEAN 5 6
STDDEV 1 2
CORR 1 .5
CORR .5 1
END DATA.
RELIABILITY /VARIABLES=a b /MATRIX=IN(*).",
    );
    assert!(error.contains("no N record"), "{error}");
}

#[test]
fn a_matrix_file_with_grouping_variables_is_refused_by_the_correlation_type_readers() {
    let file = "MATRIX DATA VARIABLES=ROWTYPE_ f a b /FACTORS=f /FORMAT=FULL.
BEGIN DATA
N . 40 40
MEAN . 5 6
STDDEV . 1 2
CORR . 1 .5
CORR . .5 1
END DATA.
";
    for command in [
        "RELIABILITY /VARIABLES=a b /MATRIX=IN(*).",
        "PARTIAL CORR MATRIX=IN(*) /VARIABLES=a BY b(1).",
        "REGRESSION MATRIX=IN(*) /VARIABLES=a b /DEPENDENT=a /METHOD=ENTER b.",
    ] {
        let error = run_error(&format!("{file}{command}"));
        assert!(
            error.contains("between ROWTYPE_ and VARNAME_"),
            "{command}: {error}"
        );
    }
}

// ONEWAY: MATRIX=OUT keeps each group's mean, standard deviation and count, which is all the
// analysis of variance, the robust tests, the contrasts and the post hoc tests use.
const GROUPS: &str = "DATA LIST FREE /y1 y2 g.
BEGIN DATA
12 5 1  14 7 1  11 6 1  15 9 1  13 5 1
18 8 2  16 9 2  19 11 2  17 7 2  21 10 2  20 12 2
25 14 3  22 11 3  24 15 3  27 13 3
END DATA.
";

const ONEWAY_REQUEST: &str = "y1 y2 BY g /STATISTICS=DESCRIPTIVES EFFECTS WELCH BROWNFORSYTHE /POLYNOMIAL=2 /CONTRAST=1 0 -1 /POSTHOC=TUKEY GH LSD";

#[test]
fn oneway_matrix_round_trip() {
    let mut engine = Engine::default();
    let direct = run(&mut engine, &format!("{GROUPS}ONEWAY {ONEWAY_REQUEST}."));
    let _ = run(
        &mut engine,
        &format!("ONEWAY {ONEWAY_REQUEST} /MATRIX=OUT(*)."),
    );
    let kinds = texts(&engine, "ROWTYPE_");
    assert_eq!(&kinds[..3], ["MEAN", "STDDEV", "N"], "{kinds:?}");
    assert_eq!(kinds.len(), 9);
    let names: Vec<&str> = engine.dataset().unwrap().variable_names().collect();
    assert_eq!(names, ["ROWTYPE_", "g", "VARNAME_", "y1", "y2"]);
    let again = run(
        &mut engine,
        &format!("ONEWAY {ONEWAY_REQUEST} /MATRIX=IN(*)."),
    );
    let mut compared = 0;
    for t in direct.iter().filter(|t| t.title != "Descriptives") {
        same(&direct, &again, &t.title);
        compared += 1;
    }
    assert!(compared >= 6, "{compared}");
    // the descriptives agree except for the minimum and maximum, which a matrix cannot give
    let (a, b) = (
        direct.iter().find(|t| t.title == "Descriptives").unwrap(),
        again.iter().find(|t| t.title == "Descriptives").unwrap(),
    );
    assert_eq!(a.rows.len(), b.rows.len());
    for (ra, rb) in a.rows.iter().zip(&b.rows) {
        for c in 0..ra.len() {
            let name = &a.column_titles()[c];
            if name == "Minimum" || name == "Maximum" {
                assert!(matches!(rb[c], Cell::Empty), "{name}");
            } else {
                match (&ra[c], &rb[c]) {
                    // the matrix keeps the variance, so a standard deviation may differ in the
                    // last bit
                    (Cell::Number(p), Cell::Number(q)) => {
                        assert!((p - q).abs() <= 1e-12 * p.abs().max(1.0), "{name}: {p} {q}")
                    }
                    (p, q) => assert_eq!(format!("{p:?}"), format!("{q:?}"), "{name}"),
                }
            }
        }
    }
}

#[test]
fn oneway_matrix_round_trip_under_split_file() {
    let data = "DATA LIST FREE /y g s.
BEGIN DATA
1 1 1  3 1 1  2 1 1  6 2 1  7 2 1  5 2 1  4 1 2  6 1 2  5 1 2  9 2 2  12 2 2  10 2 2
END DATA.
";
    let mut engine = Engine::default();
    let direct = run(
        &mut engine,
        &format!("{data}SORT CASES BY s.\nSPLIT FILE LAYERED BY s.\nONEWAY y BY g."),
    );
    let _ = run(&mut engine, "ONEWAY y BY g /MATRIX=OUT(*).");
    assert_eq!(
        texts(&engine, "ROWTYPE_")
            .iter()
            .filter(|k| *k == "MEAN")
            .count(),
        4
    );
    let again = run(&mut engine, "ONEWAY y BY g /MATRIX=IN(*).");
    same(&direct, &again, "ANOVA");
}

#[test]
fn oneway_from_matrix_data_with_the_pooled_variance_gives_the_textbook_anova() {
    let t = run(
        &mut Engine::default(),
        "MATRIX DATA VARIABLES=ROWTYPE_ g y /FACTORS=g.
BEGIN DATA
MEAN 1 10
MEAN 2 12
MEAN 3 15
N 1 10
N 2 12
N 3 8
MSE . 4
DFE . 27
END DATA.
ONEWAY y BY g /POSTHOC=TUKEY /MATRIX=IN(*).",
    );
    let anova = |row: usize, column: &str| number(&t, "ANOVA", row, column);
    assert!((anova(0, "Sum of Squares") - 111.4666666667).abs() < 1e-8);
    assert!((anova(0, "F") - 13.9333333333).abs() < 1e-8);
    assert!((anova(0, "Sig.") - 0.0000696218).abs() < 1e-9);
    assert!((anova(1, "Sum of Squares") - 108.0).abs() < 1e-9);
    assert_eq!(anova(1, "df"), 27.0);
    // Tukey of group 1 against 3: the difference is -5, the p value from the studentized range
    let table = t
        .iter()
        .find(|t| t.title.starts_with("Multiple Comparisons"))
        .expect("post hoc");
    let sig = table
        .column_titles()
        .iter()
        .position(|c| c == "Sig.")
        .unwrap();
    let found = table.rows.iter().any(|r| match r[sig] {
        Cell::Number(p) => (p - 0.0000427422).abs() < 1e-9,
        _ => false,
    });
    assert!(found, "{:?}", table.rows);
}

#[test]
fn oneway_matrix_in_refuses_homogeneity_and_needs_the_variance() {
    let file = "MATRIX DATA VARIABLES=ROWTYPE_ g y /FACTORS=g.
BEGIN DATA
MEAN 1 10
MEAN 2 12
N 1 10
N 2 12
END DATA.
";
    let error = run_error(&format!(
        "{file}ONEWAY y BY g /STATISTICS=HOMOGENEITY /MATRIX=IN(*)."
    ));
    assert!(
        error.contains("homogeneity") && error.contains("not available"),
        "{error}"
    );
    let error = run_error(&format!("{file}ONEWAY y BY g /MATRIX=IN(*)."));
    assert!(
        error.contains("no STDDEV records") && error.contains("MSE and DFE"),
        "{error}"
    );
    let error = run_error(&format!("{file}ONEWAY y BY nosuch /MATRIX=IN(*)."));
    assert!(error.contains("nosuch"), "{error}");
}

#[test]
fn oneway_matrix_in_refuses_a_second_grouping_variable_and_a_variance_of_nothing() {
    let error = run_error(
        "MATRIX DATA VARIABLES=ROWTYPE_ s g y /FACTORS=s g.
BEGIN DATA
MEAN 1 1 10
MEAN 1 2 12
N 1 1 10
N 1 2 12
MSE . . 4
DFE . . 20
END DATA.
ONEWAY y BY g /MATRIX=IN(*).",
    );
    assert!(
        error.contains("more than one variable between ROWTYPE_ and VARNAME_"),
        "{error}"
    );
    // STDDEV records with nothing in them do not count as a variance
    let error = run_error(
        "MATRIX DATA VARIABLES=ROWTYPE_ g y /FACTORS=g.
BEGIN DATA
MEAN 1 10
MEAN 2 12
STDDEV 1 .
STDDEV 2 .
N 1 10
N 2 12
END DATA.
ONEWAY y BY g /MATRIX=IN(*).",
    );
    assert!(error.contains("no STDDEV records"), "{error}");
}

const GAPS: &str = "DATA LIST FREE /a b c d.
BEGIN DATA
-0.77 1.68 8.5 -0.96
-0.82 -0.31 3.9 1.58
-0.14 . 3.28 1.13
-0.28 0.17 5.1 -1.02
0.44 0.56 . -0.26
-1.19 -1.45 2.79 0.69
1.19 0.46 5.48 -1.24
-0.02 0.83 5.76 -0.18
-0.25 -0.02 . 1.31
-0.36 -0.19 7.4 -1.08
1.28 -0.04 5.56 0.23
-0.47 0.02 2.59 .
0.07 -0.61 5.93 -0.49
-0.27 0.13 5.7 -1.67
1.85 1.18 3.82 0.37
-0.84 -1.15 3.29 0.11
-0.08 -1.66 4.51 -0.05
-2.62 -2.16 8.08 -0.08
0.89 0.84 6.09 0.39
-0.71 0.96 8.13 -0.82
1.76 0.89 5.5 -0.61
0.18 -0.69 1.08 -0.4
-0.27 -1.21 3.38 1.29
0.93 -0.24 4 -1.39
-0.69 -0.87 5.61 0.8
END DATA.
";

fn column(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let rows = texts(engine, "ROWTYPE_").len();
    (0..rows).map(|k| value(engine, name, k)).collect()
}

fn near(got: Option<f64>, want: f64, what: &str) {
    let got = got.unwrap_or(f64::NAN);
    assert!(
        (got - want).abs() <= 1e-9 * want.abs().max(1.0),
        "{what}: {got} against {want}"
    );
}

// The numbers are those of cor(), colMeans() and sd() in R with pairwise (or complete) cases.
#[test]
fn correlations_matrix_out_pairwise_and_listwise() {
    let mut engine = Engine::default();
    let _ = run(
        &mut engine,
        &format!("{GAPS}CORRELATIONS VARIABLES=a b c d /MATRIX=OUT(*)."),
    );
    let kinds = texts(&engine, "ROWTYPE_");
    assert_eq!(
        kinds,
        [
            "MEAN", "STDDEV", "N", "N", "N", "N", "CORR", "CORR", "CORR", "CORR"
        ]
    );
    assert_eq!(texts(&engine, "VARNAME_")[2..6], ["a", "b", "c", "d"]);
    near(value(&engine, "c", 0), 5.02086956522, "mean of c");
    near(value(&engine, "b", 1), 0.9703428350, "sd of b");
    // the counts of the pairs
    near(value(&engine, "c", 3), 22.0, "n of b and c");
    near(value(&engine, "a", 2), 25.0, "n of a");
    near(value(&engine, "d", 5), 24.0, "n of d");
    // the correlations, the diagonal 1
    near(value(&engine, "b", 6), 0.5511196874, "r a b");
    near(value(&engine, "d", 8), -0.4197183028, "r c d");
    near(value(&engine, "c", 9), -0.4197183028, "r d c");
    near(value(&engine, "d", 9), 1.0, "r d d");
    let _ = run(
        &mut engine,
        &format!("{GAPS}CORRELATIONS VARIABLES=a b c d /MISSING=LISTWISE /MATRIX=OUT(*)."),
    );
    assert_eq!(
        texts(&engine, "ROWTYPE_"),
        ["MEAN", "STDDEV", "N", "CORR", "CORR", "CORR", "CORR"]
    );
    // one N for every variable, over the 21 cases with no gap; the means over those cases too
    assert!(column(&engine, "d")[2] == Some(21.0));
    near(value(&engine, "b", 0), -0.16380952381, "listwise mean of b");
    near(value(&engine, "c", 1), 1.8737595262, "listwise sd of c");
    near(value(&engine, "d", 5), -0.3781847540, "listwise r c d");
}

// Only the last list without WITH is written; the active dataset is untouched without `*`.
#[test]
fn correlations_matrix_out_takes_the_last_square_list_and_can_go_to_a_file() {
    let mut engine = Engine::default();
    let _ = run(
        &mut engine,
        &format!("{GAPS}CORRELATIONS VARIABLES=a b /a WITH c d /b c d /MATRIX=OUT(*)."),
    );
    let names = texts(&engine, "VARNAME_");
    assert_eq!(names[names.len() - 3..], ["b", "c", "d"]);
    assert!(engine.dataset().unwrap().variable("a").is_err());
    let path = std::env::temp_dir().join("statscraft_corr_matrix_test.sav");
    let _ = std::fs::remove_file(&path);
    let mut engine = Engine::default();
    let _ = run(
        &mut engine,
        &format!(
            "{GAPS}CORRELATIONS VARIABLES=a b c d /MATRIX=OUT('{}').",
            path.display()
        ),
    );
    // the data stay, the file holds the matrix
    assert!(engine.dataset().unwrap().variable("a").is_ok());
    let t = run(&mut engine, &format!("GET FILE='{}'.", path.display()));
    let _ = t;
    assert_eq!(texts(&engine, "ROWTYPE_")[0], "MEAN");
    let _ = std::fs::remove_file(&path);
    // only OUT is accepted
    let (_, errors) = parse("CORRELATIONS VARIABLES=a b /MATRIX=IN(*).");
    assert!(errors[0].to_string().contains("OUT"));
}

// NONPAR CORR writes N and RHO, or N and TAUB with KENDALL alone; BOTH writes Spearman's only.
#[test]
fn nonpar_corr_matrix_out_writes_rho_or_taub() {
    let mut engine = Engine::default();
    let _ = run(
        &mut engine,
        &format!("{GAPS}NONPAR CORR VARIABLES=a b c d /MATRIX=OUT(*)."),
    );
    assert_eq!(
        texts(&engine, "ROWTYPE_"),
        ["N", "N", "N", "N", "RHO", "RHO", "RHO", "RHO"]
    );
    near(value(&engine, "b", 4), 0.4179169483, "rho a b");
    near(value(&engine, "d", 6), -0.4229249012, "rho c d");
    near(value(&engine, "c", 1), 22.0, "n b c");
    near(value(&engine, "d", 7), 1.0, "rho d d");
    let _ = run(
        &mut engine,
        &format!("{GAPS}NONPAR CORR VARIABLES=a b c d /PRINT=KENDALL /MATRIX=OUT(*)."),
    );
    assert_eq!(texts(&engine, "ROWTYPE_")[4], "TAUB");
    near(value(&engine, "b", 4), 0.3230495338, "tau a b");
    near(value(&engine, "d", 6), -0.2640692641, "tau c d");
    let _ = run(
        &mut engine,
        &format!("{GAPS}NONPAR CORR VARIABLES=a b c d /PRINT=BOTH /MATRIX=OUT(*)."),
    );
    assert_eq!(texts(&engine, "ROWTYPE_")[4], "RHO");
}

// A matrix written by CORRELATIONS is read by the procedures that read a correlation matrix.
#[test]
fn a_correlations_matrix_feeds_regression_and_partial_corr() {
    let direct = run(
        &mut Engine::default(),
        &format!("{PAY}REGRESSION /VARIABLES=y x1 x2 /DEPENDENT=y /METHOD=ENTER x1 x2."),
    );
    let mut engine = Engine::default();
    let _ = run(
        &mut engine,
        &format!("{PAY}CORRELATIONS VARIABLES=y x1 x2 /MATRIX=OUT(*)."),
    );
    let again = run(
        &mut engine,
        "REGRESSION MATRIX IN(*) /VARIABLES=y x1 x2 /DEPENDENT=y /METHOD=ENTER x1 x2.",
    );
    for title in ["Model Summary", "ANOVA", "Coefficients"] {
        same(&direct, &again, title);
    }
}

// Under SPLIT FILE a full set of records is written for every group.
#[test]
fn correlations_matrix_out_under_split_file_writes_a_set_per_group() {
    let mut engine = Engine::default();
    let _ = run(
        &mut engine,
        &format!(
            "{PAY}SORT CASES BY g.\nSPLIT FILE BY g.\nCORRELATIONS VARIABLES=y x1 x2 /MATRIX=OUT(*)."
        ),
    );
    let kinds = texts(&engine, "ROWTYPE_");
    assert_eq!(kinds.iter().filter(|k| *k == "CORR").count(), 6);
    assert_eq!(kinds.iter().filter(|k| *k == "MEAN").count(), 2);
    near(value(&engine, "g", 0), 1.0, "first split value");
    near(
        value(&engine, "g", kinds.len() - 1),
        2.0,
        "last split value",
    );
}
