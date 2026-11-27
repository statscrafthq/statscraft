//! DISCRIMINANT edge cases and numerical stability against R on the 60 invented cases of
//! discriminant_r.rs (groups of 18, 22 and 20; four predictors): a predictor in millions of
//! its own units (x2 times 1E8), one in millionths (x2 times 1E-8), two predictors shifted by
//! 1E6, a fourth group of one case, and two groups only. R solves the same problem on the
//! correlation form of the within-groups matrix (the eigenvalues of W^-1 B, canonical
//! coefficients scaled to unit pooled within-groups variance with the standardized coefficients
//! summing to a positive number, centroids and Wilks' lambda tests), so a scale cannot change a
//! number. The stepwise selection is held to the same rule: predictors in other units select the
//! same variables with the same statistics. Scripts in lab/oracles/edge-di.
//!
//! The tolerance test of a predictor judged the within-groups matrix by its largest element, so a
//! predictor in millionths was left out as redundant and one in millions left the others out;
//! both are now judged by their correlations.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g x1 x2 x3 x4.
BEGIN DATA
1 3.63 10.95 0.98 7.84
1 5.3 11.14 2.65 7.95
1 4.52 10.47 3.7 9.47
1 4.8 9.9 1.58 6.79
1 5.07 10.55 3.01 9.14
1 4.6 11.42 2.11 8.33
1 5.04 10.91 2.32 6.62
1 6.07 11.77 2.83 7.59
1 7.24 9.83 1.2 5.03
1 5.28 11.55 3.67 6.17
1 5.23 9.75 2.66 7.42
1 4.82 8.88 2.09 7.52
1 5.51 10.35 1.91 5.83
1 5.8 11.36 3.13 6.1
1 5.07 10.71 3.92 6.95
1 5.45 10.9 4.01 6.93
1 4.82 10.25 3.56 7.95
1 4.47 9.67 1.02 9.14
2 4.5 10.58 5.02 9.43
2 5.18 9.69 4.22 9.38
2 6.11 10.49 3.08 7.68
2 5.28 10.73 2.6 6.82
2 6.93 10.84 2.67 8.47
2 6.03 9.15 2.94 6.47
2 7.11 11.36 3.53 6.53
2 7.06 11.57 2.01 8.31
2 4.88 9.84 3.12 7.4
2 5.38 10.63 0.72 7.68
2 6.02 9.04 2.92 7.89
2 7.15 9.91 2.82 8.68
2 7.36 9.64 3.96 7.15
2 6.45 10.83 2.07 5.79
2 6.69 11.23 3.69 10.42
2 7.81 11.68 2.7 6.18
2 6.78 11.49 4.07 5.84
2 4.73 10.6 4.7 6.37
2 3.57 12.26 2.92 6.73
2 7.41 10.76 3.48 4.97
2 8.52 10.32 3.14 5.92
2 7.26 10.93 3.91 7.5
3 4.56 8.31 4.06 7.53
3 6.44 9.58 4.91 7.54
3 7.13 8.65 3.92 6.19
3 5.02 8.94 3.55 8.76
3 5.83 10.92 3.6 7.95
3 4.39 7.49 5.28 5.24
3 5.87 9.28 4.5 8.57
3 5.75 9.59 3.04 8.08
3 7.33 11.88 7.16 7.42
3 6.96 8.95 4.2 6.61
3 4.97 10.7 5.69 10.32
3 4.67 9.32 4.45 7.75
3 5.29 8.79 6.06 9.07
3 5.81 7.33 5.22 7.16
3 4.91 9.55 2.96 7.15
3 6.49 9.85 3.23 6.09
3 5.7 10.22 3.35 8.08
3 5.68 8.22 2.41 8.04
3 7 8.83 4.61 9.01
3 5.14 10.73 3.09 6.99
";

fn run(syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}END DATA.\n{syntax}"));
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

fn run_with(extra_case: &str, syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}{extra_case}END DATA.\n{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

/// The row headers joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                let shown = match cell {
                    Cell::Text(t) if !t.is_empty() => Some(t.clone()),
                    Cell::Number(n) => Some(format!("{n}")),
                    _ => None,
                };
                if let Some(t) = shown {
                    carried[level] = t;
                    for deeper in carried.iter_mut().skip(level + 1) {
                        deeper.clear();
                    }
                }
            }
            carried
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .collect()
}

fn value(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let r = row_keys(table)
        .iter()
        .position(|k| k == row)
        .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(table)));
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", table.column_titles()));
    match &table.rows[r][c] {
        Cell::Number(n) => *n,
        other => panic!("{title} / {row} / {column}: {other:?}"),
    }
}

fn check(tables: &[Table], what: &str, expected: &[(&str, &str, &str, f64)]) {
    for (title, row, column, want) in expected {
        let got = value(tables, title, row, column);
        assert!(
            (got - want).abs() <= 1e-7 * want.abs() + 1e-12,
            "{what}: {title} / {row} / {column}: {got} is not {want}"
        );
    }
}

const REQUEST: &str =
    "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 /PRIORS=EQUAL /STATISTICS=COEFF RAW.";

#[test]
fn the_unchanged_data_agrees_with_r() {
    let t = run(REQUEST);
    check(
        &t,
        "base",
        &[
            ("Eigenvalues", "1", "Eigenvalue", 0.960903364948),
            ("Eigenvalues", "1", "Canonical Correlation", 0.700022116324),
            (
                "Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 1",
                0.209053820499,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 1",
                0.208732628725,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 1",
                -0.791073785794,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 1",
                -0.735914054384,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 1",
                0.702275529332,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 1",
                0.737892490248,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 1",
                0.0857961112874,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 1",
                0.107885065852,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "(Constant)",
                "Function / 1",
                3.84744921473,
            ),
            (
                "Functions at Group Centroids",
                "1",
                "Function / 1",
                -1.00053496009,
            ),
            (
                "Functions at Group Centroids",
                "2",
                "Function / 1",
                -0.363883472299,
            ),
            (
                "Functions at Group Centroids",
                "3",
                "Function / 1",
                1.30075328361,
            ),
            ("Eigenvalues", "2", "Eigenvalue", 0.23804721492),
            ("Eigenvalues", "2", "Canonical Correlation", 0.438493286567),
            (
                "Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 2",
                0.957742009486,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 2",
                0.95627052786,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 2",
                0.297172898375,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 2",
                0.276451724761,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 2",
                0.0573389318617,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 2",
                0.0602469621287,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 2",
                0.13768533958,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 2",
                0.173133626975,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "(Constant)",
                "Function / 2",
                -9.76827574618,
            ),
            (
                "Functions at Group Centroids",
                "1",
                "Function / 2",
                -0.528841630513,
            ),
            (
                "Functions at Group Centroids",
                "2",
                "Function / 2",
                0.598173265391,
            ),
            (
                "Functions at Group Centroids",
                "3",
                "Function / 2",
                -0.182033124469,
            ),
            (
                "Wilks' Lambda",
                "1 through 2",
                "Wilks' Lambda",
                0.41191404537,
            ),
            ("Wilks' Lambda", "1 through 2", "Chi-square", 49.2252021423),
            ("Wilks' Lambda", "1 through 2", "Sig.", 5.75606899637e-08),
            ("Wilks' Lambda", "2", "Wilks' Lambda", 0.807723637636),
            ("Wilks' Lambda", "2", "Chi-square", 11.8512097936),
            ("Wilks' Lambda", "2", "Sig.", 0.00791062723188),
        ],
    );
}

#[test]
fn a_predictor_in_millions_of_units_agrees_with_r() {
    let t = run(&format!("COMPUTE x2 = x2 * 1E8.\n{REQUEST}"));
    check(
        &t,
        "big",
        &[
            ("Eigenvalues", "1", "Eigenvalue", 0.960903364948),
            ("Eigenvalues", "1", "Canonical Correlation", 0.700022116324),
            (
                "Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 1",
                0.209053820499,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 1",
                0.208732628725,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 1",
                -7.91073785794e-09,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 1",
                -0.735914054384,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 1",
                0.702275529332,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 1",
                0.737892490248,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 1",
                0.0857961112874,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 1",
                0.107885065852,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "(Constant)",
                "Function / 1",
                3.84744921473,
            ),
            (
                "Functions at Group Centroids",
                "1",
                "Function / 1",
                -1.00053496009,
            ),
            (
                "Functions at Group Centroids",
                "2",
                "Function / 1",
                -0.363883472299,
            ),
            (
                "Functions at Group Centroids",
                "3",
                "Function / 1",
                1.30075328361,
            ),
            ("Eigenvalues", "2", "Eigenvalue", 0.23804721492),
            ("Eigenvalues", "2", "Canonical Correlation", 0.438493286567),
            (
                "Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 2",
                0.957742009486,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 2",
                0.95627052786,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 2",
                2.97172898375e-09,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 2",
                0.276451724761,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 2",
                0.0573389318617,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 2",
                0.0602469621287,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 2",
                0.13768533958,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 2",
                0.173133626975,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "(Constant)",
                "Function / 2",
                -9.76827574618,
            ),
            (
                "Functions at Group Centroids",
                "1",
                "Function / 2",
                -0.528841630513,
            ),
            (
                "Functions at Group Centroids",
                "2",
                "Function / 2",
                0.598173265391,
            ),
            (
                "Functions at Group Centroids",
                "3",
                "Function / 2",
                -0.182033124469,
            ),
            (
                "Wilks' Lambda",
                "1 through 2",
                "Wilks' Lambda",
                0.41191404537,
            ),
            ("Wilks' Lambda", "1 through 2", "Chi-square", 49.2252021423),
            ("Wilks' Lambda", "1 through 2", "Sig.", 5.75606899637e-08),
            ("Wilks' Lambda", "2", "Wilks' Lambda", 0.807723637636),
            ("Wilks' Lambda", "2", "Chi-square", 11.8512097936),
            ("Wilks' Lambda", "2", "Sig.", 0.00791062723188),
        ],
    );
}

#[test]
fn a_predictor_in_millionths_of_a_unit_agrees_with_r() {
    let t = run(&format!("COMPUTE x2 = x2 * 1E-8.\n{REQUEST}"));
    check(
        &t,
        "small",
        &[
            ("Eigenvalues", "1", "Eigenvalue", 0.960903364948),
            ("Eigenvalues", "1", "Canonical Correlation", 0.700022116324),
            (
                "Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 1",
                0.209053820499,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 1",
                0.208732628725,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 1",
                -79107378.5794,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 1",
                -0.735914054384,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 1",
                0.702275529332,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 1",
                0.737892490248,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 1",
                0.0857961112874,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 1",
                0.107885065852,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "(Constant)",
                "Function / 1",
                3.84744921473,
            ),
            (
                "Functions at Group Centroids",
                "1",
                "Function / 1",
                -1.00053496009,
            ),
            (
                "Functions at Group Centroids",
                "2",
                "Function / 1",
                -0.363883472299,
            ),
            (
                "Functions at Group Centroids",
                "3",
                "Function / 1",
                1.30075328361,
            ),
            ("Eigenvalues", "2", "Eigenvalue", 0.23804721492),
            ("Eigenvalues", "2", "Canonical Correlation", 0.438493286567),
            (
                "Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 2",
                0.957742009486,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 2",
                0.95627052786,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 2",
                29717289.8375,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 2",
                0.276451724761,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 2",
                0.0573389318617,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 2",
                0.0602469621287,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 2",
                0.13768533958,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 2",
                0.173133626975,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "(Constant)",
                "Function / 2",
                -9.76827574618,
            ),
            (
                "Functions at Group Centroids",
                "1",
                "Function / 2",
                -0.528841630513,
            ),
            (
                "Functions at Group Centroids",
                "2",
                "Function / 2",
                0.598173265391,
            ),
            (
                "Functions at Group Centroids",
                "3",
                "Function / 2",
                -0.182033124469,
            ),
            (
                "Wilks' Lambda",
                "1 through 2",
                "Wilks' Lambda",
                0.41191404537,
            ),
            ("Wilks' Lambda", "1 through 2", "Chi-square", 49.2252021423),
            ("Wilks' Lambda", "1 through 2", "Sig.", 5.75606899637e-08),
            ("Wilks' Lambda", "2", "Wilks' Lambda", 0.807723637636),
            ("Wilks' Lambda", "2", "Chi-square", 11.8512097936),
            ("Wilks' Lambda", "2", "Sig.", 0.00791062723188),
        ],
    );
}

#[test]
fn predictors_shifted_by_a_million_agrees_with_r() {
    let t = run(&format!(
        "COMPUTE x1 = x1 + 1E6.\nCOMPUTE x3 = x3 + 1E6.\n{REQUEST}"
    ));
    check(
        &t,
        "shift",
        &[
            ("Eigenvalues", "1", "Eigenvalue", 0.960903365007),
            ("Eigenvalues", "1", "Canonical Correlation", 0.700022116335),
            (
                "Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 1",
                0.209053820507,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 1",
                0.208732628732,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 1",
                -0.791073785771,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 1",
                -0.735914054363,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 1",
                0.702275529343,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 1",
                0.737892490264,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 1",
                0.0857961112848,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 1",
                0.107885065849,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "(Constant)",
                "Function / 1",
                -911325.502401,
            ),
            (
                "Functions at Group Centroids",
                "1",
                "Function / 1",
                -1.00053496008,
            ),
            (
                "Functions at Group Centroids",
                "2",
                "Function / 1",
                -0.363883472274,
            ),
            (
                "Functions at Group Centroids",
                "3",
                "Function / 1",
                1.30075328362,
            ),
            ("Eigenvalues", "2", "Eigenvalue", 0.238047214926),
            ("Eigenvalues", "2", "Canonical Correlation", 0.438493286572),
            (
                "Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 2",
                0.957742009493,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 2",
                0.956270527866,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 2",
                0.29717289838,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 2",
                0.276451724766,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 2",
                0.0573389318385,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 2",
                0.0602469621047,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 2",
                0.137685339588,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 2",
                0.173133626985,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "(Constant)",
                "Function / 2",
                -1015090.70961,
            ),
            (
                "Functions at Group Centroids",
                "1",
                "Function / 2",
                -0.528841630549,
            ),
            (
                "Functions at Group Centroids",
                "2",
                "Function / 2",
                0.598173265364,
            ),
            (
                "Functions at Group Centroids",
                "3",
                "Function / 2",
                -0.182033124534,
            ),
            (
                "Wilks' Lambda",
                "1 through 2",
                "Wilks' Lambda",
                0.411914045356,
            ),
            ("Wilks' Lambda", "1 through 2", "Chi-square", 49.2252021443),
            ("Wilks' Lambda", "1 through 2", "Sig.", 5.75606899136e-08),
            ("Wilks' Lambda", "2", "Wilks' Lambda", 0.807723637632),
            ("Wilks' Lambda", "2", "Chi-square", 11.8512097939),
            ("Wilks' Lambda", "2", "Sig.", 0.00791062723087),
        ],
    );
}

#[test]
fn a_group_of_one_case_agrees_with_r() {
    let t = run_with(
        "4 5.5 10.5 2.5 7.5\n",
        "DISCRIMINANT /GROUPS=g(1,4) /VARIABLES=x1 x2 x3 x4 /PRIORS=EQUAL /STATISTICS=COEFF RAW.",
    );
    check(
        &t,
        "one",
        &[
            ("Eigenvalues", "1", "Eigenvalue", 0.975223532283),
            ("Eigenvalues", "1", "Canonical Correlation", 0.702657944079),
            (
                "Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 1",
                0.211495234595,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 1",
                0.211170291815,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 1",
                -0.787173233698,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 1",
                -0.732285478695,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 1",
                0.705392684819,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 1",
                0.741167736969,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 1",
                0.0841408795818,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 1",
                0.105803680357,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "(Constant)",
                "Function / 1",
                3.81046897069,
            ),
            (
                "Functions at Group Centroids",
                "1",
                "Function / 1",
                -0.987871201068,
            ),
            (
                "Functions at Group Centroids",
                "2",
                "Function / 1",
                -0.346314504596,
            ),
            (
                "Functions at Group Centroids",
                "3",
                "Function / 1",
                1.31488443021,
            ),
            (
                "Functions at Group Centroids",
                "4",
                "Function / 1",
                -0.897087883954,
            ),
            ("Eigenvalues", "2", "Eigenvalue", 0.238758174183),
            ("Eigenvalues", "2", "Canonical Correlation", 0.4390215694),
            (
                "Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 2",
                0.955243406968,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 2",
                0.953775764212,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 2",
                0.302970697327,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 2",
                0.281845256705,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 2",
                0.0568608776328,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 2",
                0.0597446626598,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 2",
                0.135271779259,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 2",
                0.170098674571,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "(Constant)",
                "Function / 2",
                -9.78998751113,
            ),
            (
                "Functions at Group Centroids",
                "1",
                "Function / 2",
                -0.521142708484,
            ),
            (
                "Functions at Group Centroids",
                "2",
                "Function / 2",
                0.603061597344,
            ),
            (
                "Functions at Group Centroids",
                "3",
                "Function / 2",
                -0.184426023825,
            ),
            (
                "Functions at Group Centroids",
                "4",
                "Function / 2",
                -0.198265912354,
            ),
            ("Eigenvalues", "3", "Eigenvalue", 0.00136111701788),
            ("Eigenvalues", "3", "Canonical Correlation", 0.0368682369605),
            (
                "Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 3",
                -0.384040554682,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 3",
                -0.383450512046,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 3",
                0.568300825828,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 3",
                0.528674533723,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 3",
                0.626564388249,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 3",
                0.658341544643,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 3",
                -0.493930663481,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 3",
                -0.621097405891,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "(Constant)",
                "Function / 3",
                -1.99066307252,
            ),
            (
                "Functions at Group Centroids",
                "1",
                "Function / 3",
                0.0113141596745,
            ),
            (
                "Functions at Group Centroids",
                "2",
                "Function / 3",
                0.00373943434109,
            ),
            (
                "Functions at Group Centroids",
                "3",
                "Function / 3",
                -0.000606298604276,
            ),
            (
                "Functions at Group Centroids",
                "4",
                "Function / 3",
                -0.27379645756,
            ),
            (
                "Wilks' Lambda",
                "1 through 3",
                "Wilks' Lambda",
                0.408137492566,
            ),
            ("Wilks' Lambda", "1 through 3", "Chi-square", 50.1844655068),
            ("Wilks' Lambda", "1 through 3", "Sig.", 1.29662435296e-06),
            (
                "Wilks' Lambda",
                "2 through 3",
                "Wilks' Lambda",
                0.806162779723,
            ),
            ("Wilks' Lambda", "2 through 3", "Chi-square", 12.0662974271),
            ("Wilks' Lambda", "2 through 3", "Sig.", 0.0605060278989),
            ("Wilks' Lambda", "3", "Wilks' Lambda", 0.998640733103),
            ("Wilks' Lambda", "3", "Chi-square", 0.0761707261173),
            ("Wilks' Lambda", "3", "Sig.", 0.962630764277),
        ],
    );
}

#[test]
fn two_groups_agrees_with_r() {
    let t = run(
        "SELECT IF g < 3.\nDISCRIMINANT /GROUPS=g(1,2) /VARIABLES=x1 x2 x3 x4 /PRIORS=EQUAL /STATISTICS=COEFF RAW.",
    );
    check(
        &t,
        "two",
        &[
            ("Eigenvalues", "1", "Eigenvalue", 0.454513845552),
            ("Eigenvalues", "1", "Canonical Correlation", 0.559003631014),
            (
                "Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 1",
                0.891888439524,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x1",
                "Function / 1",
                0.931828155253,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 1",
                -0.0261534017673,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x2",
                "Function / 1",
                -0.0209022884331,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 1",
                0.51452664633,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x3",
                "Function / 1",
                0.498540561212,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 1",
                0.197496432072,
            ),
            (
                "Standardized Canonical Discriminant Function Coefficients",
                "x4",
                "Function / 1",
                0.256281905407,
            ),
            (
                "Canonical Discriminant Function Coefficients",
                "(Constant)",
                "Function / 1",
                -7.82574933573,
            ),
            (
                "Functions at Group Centroids",
                "1",
                "Function / 1",
                -0.726457897076,
            ),
            (
                "Functions at Group Centroids",
                "2",
                "Function / 1",
                0.594374643062,
            ),
            ("Wilks' Lambda", "1", "Wilks' Lambda", 0.687514940513),
            ("Wilks' Lambda", "1", "Chi-square", 13.4881818488),
            ("Wilks' Lambda", "1", "Sig.", 0.00912113680289),
        ],
    );
}

// Predictors in other units select the same variables in the same steps with the same statistics.
#[test]
fn stepwise_selection_does_not_depend_on_the_units() {
    let request = "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x1 x2 x3 x4 /METHOD=WILKS /PIN=0.15 /POUT=0.2 /PRIORS=EQUAL /STATISTICS=COEFF RAW.";
    let plain = run(request);
    let scaled = run(&format!(
        "COMPUTE x2 = x2 * 1E6.\nCOMPUTE x4 = x4 * 1E-6.\n{request}"
    ));
    for title in [
        "Variables in the Analysis",
        "Variables Not in the Analysis",
        "Eigenvalues",
        "Wilks' Lambda",
    ] {
        let a = plain.iter().find(|t| t.title == title).expect("table");
        let b = scaled.iter().find(|t| t.title == title).expect("table");
        assert_eq!(a.rows.len(), b.rows.len(), "{title}");
        for (ra, rb) in a.rows.iter().zip(&b.rows) {
            for (ca, cb) in ra.iter().zip(rb) {
                match (ca, cb) {
                    (Cell::Number(x), Cell::Number(y)) => assert!(
                        (x - y).abs() <= 1e-8 * x.abs().max(1.0),
                        "{title}: {x} is not {y}"
                    ),
                    (x, y) => assert_eq!(format!("{x:?}"), format!("{y:?}"), "{title}"),
                }
            }
        }
    }
}
