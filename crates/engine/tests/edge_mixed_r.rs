//! Edge cases and numerical stability of MIXED (a random intercept by REML) against R
//! (`nlme::lme`) on sixteen invented cases: a subject with a single case, and y with 1000000000
//! added, which must give the same covariance parameters, likelihood, slope and standard errors
//! and an intercept that is 1000000000 higher. R fits y shifted by its first value. The degrees
//! of freedom are the engine's own (Satterthwaite), not the containment of nlme, so only the
//! estimates, their standard errors, the covariance parameters and the likelihood are compared.
//! Scripts in lab/oracles/edge-mx.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y x s t (4F14.2).
BEGIN DATA.
5.1 1.2 1 1
5.9 2.1 1 2
7.2 3.3 1 3
4.2 0.8 2 1
5.0 1.9 2 2
6.1 3.1 2 3
6.3 1.1 3 1
7.4 2.4 3 2
8.1 3.2 3 3
5.5 1.4 4 1
5.2 2.0 4 2
6.8 3.0 4 3
4.9 0.9 5 1
6.2 2.2 5 2
6.6 3.4 5 3
5.7 1.7 6 1
END DATA.
";

const SHIFTED: &str = "DATA LIST FREE /y x s t (4F14.2).
BEGIN DATA.
1000000005.10 1.2 1 1
1000000005.90 2.1 1 2
1000000007.20 3.3 1 3
1000000004.20 0.8 2 1
1000000005.00 1.9 2 2
1000000006.10 3.1 2 3
1000000006.30 1.1 3 1
1000000007.40 2.4 3 2
1000000008.10 3.2 3 3
1000000005.50 1.4 4 1
1000000005.20 2.0 4 2
1000000006.80 3.0 4 3
1000000004.90 0.9 5 1
1000000006.20 2.2 5 2
1000000006.60 3.4 5 3
1000000005.70 1.7 6 1
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    run_on(DATA, syntax)
}

fn run_on(data: &str, syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{data}{syntax}"));
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

/// The first cell of each row (the source).
fn row_keys(table: &Table) -> Vec<String> {
    table
        .rows
        .iter()
        .map(|row| match &row[0] {
            Cell::Text(t) => t.clone(),
            _ => String::new(),
        })
        .collect()
}

/// Each expectation: the table title, the row, the column and the value (NaN: an empty cell).
fn check(tables: &[Table], expected: &[(&str, &str, &str, f64)]) {
    for (title, row, column, want) in expected {
        let t = tables
            .iter()
            .find(|t| t.title == *title)
            .unwrap_or_else(|| panic!("no table {title}"));
        let r = row_keys(t)
            .iter()
            .position(|k| k == row)
            .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(t)));
        let c = t
            .column_titles()
            .iter()
            .position(|k| k == column)
            .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", t.column_titles()));
        if want.is_nan() {
            assert_eq!(t.rows[r][c], Cell::Empty, "{title} / {row} / {column}");
            continue;
        }
        let Cell::Number(n) = &t.rows[r][c] else {
            panic!("{title} / {row} / {column}: {:?}", t.rows[r][c]);
        };
        assert!(
            (n - want).abs() <= 1e-6 + 1e-6 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn random_intercept_with_a_subject_of_one_case() {
    let t =
        run("MIXED y WITH x /FIXED=x /RANDOM=INTERCEPT | SUBJECT(s) /METHOD=REML /PRINT=SOLUTION.");
    check(
        &t,
        &[
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Estimate",
                0.0742441785333,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = s]",
                "Estimate",
                0.37777384043,
            ),
            (
                "Information Criteria",
                "-2 Restricted Log Likelihood",
                "Value",
                21.6086082952,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                4.24648556483,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                0.309420913133,
            ),
            (
                "Estimates of Fixed Effects",
                "x",
                "Estimate",
                0.839750375011,
            ),
            (
                "Estimates of Fixed Effects",
                "x",
                "Std. Error",
                0.0800606143725,
            ),
        ],
    );
}

#[test]
fn y_near_one_billion_keeps_its_digits() {
    let t = run_on(
        SHIFTED,
        "MIXED y WITH x /FIXED=x /RANDOM=INTERCEPT | SUBJECT(s) /METHOD=REML /PRINT=SOLUTION.",
    );
    check(
        &t,
        &[
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Estimate",
                0.0742441782466,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = s]",
                "Estimate",
                0.377773784522,
            ),
            (
                "Information Criteria",
                "-2 Restricted Log Likelihood",
                "Value",
                21.6086075777,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                1000000004.25,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                0.309420897864,
            ),
            (
                "Estimates of Fixed Effects",
                "x",
                "Estimate",
                0.839750383161,
            ),
            (
                "Estimates of Fixed Effects",
                "x",
                "Std. Error",
                0.080060614202,
            ),
        ],
    );
}
