//! Edge cases and numerical stability of NONPAR CORR against R on nine invented cases: a constant
//! variable, a variable with no valid value, ties, the first three and two cases, a zero weight
//! and a weight of two, and the same nine cases with 1000000000 added to x, y and z (ranks do
//! not change). Kendall's tau-b with its normal significance corrected for ties (`cor.test`,
//! `exact = FALSE`) and Spearman's rho with its t significance on n - 2 degrees of freedom; no
//! significance for one or two pairs, none for a constant variable; a Spearman rho of 1 or -1
//! has the significance 0. Scripts in lab/oracles/edge-nc.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /x y z c m w (6F14.2).
BEGIN DATA.
3 2 9 5 . 1
7 5 8 5 . 2
4 4 7 5 . 1
9 9 6 5 . 0
2 1 5 5 . 1
8 6 4 5 . 1
. 7 3 5 . 1
6 7 2 5 . 1
5 . 1 5 . 1
END DATA.
";

const SHIFTED: &str = "DATA LIST FREE /x y z c m w (6F14.2).
BEGIN DATA.
1000000003 1000000002 1000000009 5 . 1
1000000007 1000000005 1000000008 5 . 2
1000000004 1000000004 1000000007 5 . 1
1000000009 1000000009 1000000006 5 . 0
1000000002 1000000001 1000000005 5 . 1
1000000008 1000000006 1000000004 5 . 1
. 1000000007 1000000003 5 . 1
1000000006 1000000007 1000000002 5 . 1
1000000005 . 1000000001 5 . 1
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

/// The row headers (the coefficient, the variable and the statistic) joined with " / ", the
/// first two carried from the row above.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = [String::new(), String::new()];
    table
        .rows
        .iter()
        .map(|row| {
            for (slot, cell) in carried.iter_mut().zip(row) {
                if let Cell::Text(t) = cell
                    && !t.is_empty()
                {
                    slot.clone_from(t);
                }
            }
            let last = match &row[2] {
                Cell::Text(t) => t.clone(),
                _ => String::new(),
            };
            format!("{} / {} / {}", carried[0], carried[1], last)
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
            (n - want).abs() <= 1e-8 + 1e-7 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn constant_and_empty_variables_with_ties() {
    let t = run("NONPAR CORR VARIABLES=x y z c m /PRINT=BOTH TWOTAIL.");
    check(
        &t,
        &[
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "x", 8.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / x / N", "x", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "y",
                0.809523809524,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "y",
                0.0106740181183,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "y", 7.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "y",
                0.892857142857,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "y",
                0.00680718740894,
            ),
            ("Correlations", "Spearman's rho / x / N", "y", 7.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "z",
                -0.0714285714286,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "z",
                0.804570948017,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "z", 8.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "z",
                -0.166666666667,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "z",
                0.693238811728,
            ),
            ("Correlations", "Spearman's rho / x / N", "z", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "c", 8.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / x / N", "c", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "m", 0.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / x / N", "m", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "x",
                0.809523809524,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "x",
                0.0106740181183,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "x", 7.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "x",
                0.892857142857,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "x",
                0.00680718740894,
            ),
            ("Correlations", "Spearman's rho / y / N", "x", 7.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "y", 8.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / y / N", "y", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "z",
                -0.400066132099,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "z",
                0.170239954629,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "z", 8.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "z",
                -0.514979292623,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "z",
                0.191549313416,
            ),
            ("Correlations", "Spearman's rho / y / N", "z", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "c", 8.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / y / N", "c", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "m", 0.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / y / N", "m", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "x",
                -0.0714285714286,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "x",
                0.804570948017,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "x", 8.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "x",
                -0.166666666667,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "x",
                0.693238811728,
            ),
            ("Correlations", "Spearman's rho / z / N", "x", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "y",
                -0.400066132099,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "y",
                0.170239954629,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "y", 8.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "y",
                -0.514979292623,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "y",
                0.191549313416,
            ),
            ("Correlations", "Spearman's rho / z / N", "y", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "z",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "z", 9.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "z",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / z / N", "z", 9.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "c", 9.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / z / N", "c", 9.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "m", 0.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / z / N", "m", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / c / Correlation Coefficient",
                "x",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / c / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / c / N", "x", 8.0),
            (
                "Correlations",
                "Spearman's rho / c / Correlation Coefficient",
                "x",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / c / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / c / N", "x", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / c / Correlation Coefficient",
                "y",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / c / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / c / N", "y", 8.0),
            (
                "Correlations",
                "Spearman's rho / c / Correlation Coefficient",
                "y",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / c / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / c / N", "y", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / c / Correlation Coefficient",
                "z",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / c / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / c / N", "z", 9.0),
            (
                "Correlations",
                "Spearman's rho / c / Correlation Coefficient",
                "z",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / c / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / c / N", "z", 9.0),
            (
                "Correlations",
                "Kendall's tau_b / c / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / c / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / c / N", "c", 9.0),
            (
                "Correlations",
                "Spearman's rho / c / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / c / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / c / N", "c", 9.0),
            (
                "Correlations",
                "Kendall's tau_b / c / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / c / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / c / N", "m", 0.0),
            (
                "Correlations",
                "Spearman's rho / c / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / c / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / c / N", "m", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / m / Correlation Coefficient",
                "x",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / m / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / m / N", "x", 0.0),
            (
                "Correlations",
                "Spearman's rho / m / Correlation Coefficient",
                "x",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / m / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / m / N", "x", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / m / Correlation Coefficient",
                "y",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / m / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / m / N", "y", 0.0),
            (
                "Correlations",
                "Spearman's rho / m / Correlation Coefficient",
                "y",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / m / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / m / N", "y", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / m / Correlation Coefficient",
                "z",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / m / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / m / N", "z", 0.0),
            (
                "Correlations",
                "Spearman's rho / m / Correlation Coefficient",
                "z",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / m / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / m / N", "z", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / m / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / m / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / m / N", "c", 0.0),
            (
                "Correlations",
                "Spearman's rho / m / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / m / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / m / N", "c", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / m / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / m / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / m / N", "m", 0.0),
            (
                "Correlations",
                "Spearman's rho / m / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / m / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / m / N", "m", 0.0),
        ],
    );
}

#[test]
fn three_cases() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 3).\nNONPAR CORR VARIABLES=x y z /PRINT=BOTH TWOTAIL.",
    );
    check(
        &t,
        &[
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "x", 3.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / x / N", "x", 3.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "y",
                0.117185087198,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "y", 3.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "y",
                0.0,
            ),
            ("Correlations", "Spearman's rho / x / N", "y", 3.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "z",
                -0.333333333333,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "z",
                0.601508134441,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "z", 3.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "z",
                -0.5,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "z",
                0.666666666667,
            ),
            ("Correlations", "Spearman's rho / x / N", "z", 3.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "x",
                0.117185087198,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "x", 3.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "x",
                0.0,
            ),
            ("Correlations", "Spearman's rho / y / N", "x", 3.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "y", 3.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / y / N", "y", 3.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "z",
                -0.333333333333,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "z",
                0.601508134441,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "z", 3.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "z",
                -0.5,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "z",
                0.666666666667,
            ),
            ("Correlations", "Spearman's rho / y / N", "z", 3.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "x",
                -0.333333333333,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "x",
                0.601508134441,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "x", 3.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "x",
                -0.5,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "x",
                0.666666666667,
            ),
            ("Correlations", "Spearman's rho / z / N", "x", 3.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "y",
                -0.333333333333,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "y",
                0.601508134441,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "y", 3.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "y",
                -0.5,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "y",
                0.666666666667,
            ),
            ("Correlations", "Spearman's rho / z / N", "y", 3.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "z",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "z", 3.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "z",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / z / N", "z", 3.0),
        ],
    );
}

#[test]
fn two_cases() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 2).\nNONPAR CORR VARIABLES=x y z /PRINT=BOTH TWOTAIL.",
    );
    check(
        &t,
        &[
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "x", 2.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / x / N", "x", 2.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "y", 2.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / x / N", "y", 2.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "z",
                -1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "z", 2.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "z",
                -1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / x / N", "z", 2.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "x", 2.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / y / N", "x", 2.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "y", 2.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / y / N", "y", 2.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "z",
                -1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "z", 2.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "z",
                -1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / y / N", "z", 2.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "x",
                -1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "x", 2.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "x",
                -1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / z / N", "x", 2.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "y",
                -1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "y", 2.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "y",
                -1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / z / N", "y", 2.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "z",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "z", 2.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "z",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / z / N", "z", 2.0),
        ],
    );
}

#[test]
fn zero_weight_and_weight_of_two() {
    let t = run("WEIGHT BY w.\nNONPAR CORR VARIABLES=x y z /PRINT=BOTH TWOTAIL.");
    check(
        &t,
        &[
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "x", 8.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / x / N", "x", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "y",
                0.7,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "y",
                0.0315144211326,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "y", 7.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "y",
                0.781818181818,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "y",
                0.0378228918442,
            ),
            ("Correlations", "Spearman's rho / x / N", "y", 7.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "z",
                -0.037037037037,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "z",
                0.900032130542,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "z", 8.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "z",
                -0.10843373494,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "z",
                0.798274813333,
            ),
            ("Correlations", "Spearman's rho / x / N", "z", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "x",
                0.7,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "x",
                0.0315144211326,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "x", 7.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "x",
                0.781818181818,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "x",
                0.0378228918442,
            ),
            ("Correlations", "Spearman's rho / y / N", "x", 7.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "y", 8.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / y / N", "y", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "z",
                -0.528395949267,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "z",
                0.0763570041784,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "z", 8.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "z",
                -0.678800345392,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "z",
                0.0641695587559,
            ),
            ("Correlations", "Spearman's rho / y / N", "z", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "x",
                -0.037037037037,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "x",
                0.900032130542,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "x", 8.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "x",
                -0.10843373494,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "x",
                0.798274813333,
            ),
            ("Correlations", "Spearman's rho / z / N", "x", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "y",
                -0.528395949267,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "y",
                0.0763570041784,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "y", 8.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "y",
                -0.678800345392,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "y",
                0.0641695587559,
            ),
            ("Correlations", "Spearman's rho / z / N", "y", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "z",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "z", 9.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "z",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / z / N", "z", 9.0),
        ],
    );
}

#[test]
fn values_near_one_billion_rank_as_the_small_ones() {
    let t = run_on(
        SHIFTED,
        "NONPAR CORR VARIABLES=x y z c m /PRINT=BOTH TWOTAIL.",
    );
    check(
        &t,
        &[
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "x", 8.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "x",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / x / N", "x", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "y",
                0.809523809524,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "y",
                0.0106740181183,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "y", 7.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "y",
                0.892857142857,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "y",
                0.00680718740894,
            ),
            ("Correlations", "Spearman's rho / x / N", "y", 7.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "z",
                -0.0714285714286,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "z",
                0.804570948017,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "z", 8.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "z",
                -0.166666666667,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "z",
                0.693238811728,
            ),
            ("Correlations", "Spearman's rho / x / N", "z", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "c", 8.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / x / N", "c", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / x / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / x / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / x / N", "m", 0.0),
            (
                "Correlations",
                "Spearman's rho / x / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / x / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / x / N", "m", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "x",
                0.809523809524,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "x",
                0.0106740181183,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "x", 7.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "x",
                0.892857142857,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "x",
                0.00680718740894,
            ),
            ("Correlations", "Spearman's rho / y / N", "x", 7.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "y", 8.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "y",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / y / N", "y", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "z",
                -0.400066132099,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "z",
                0.170239954629,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "z", 8.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "z",
                -0.514979292623,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "z",
                0.191549313416,
            ),
            ("Correlations", "Spearman's rho / y / N", "z", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "c", 8.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / y / N", "c", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / y / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / y / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / y / N", "m", 0.0),
            (
                "Correlations",
                "Spearman's rho / y / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / y / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / y / N", "m", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "x",
                -0.0714285714286,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "x",
                0.804570948017,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "x", 8.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "x",
                -0.166666666667,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "x",
                0.693238811728,
            ),
            ("Correlations", "Spearman's rho / z / N", "x", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "y",
                -0.400066132099,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "y",
                0.170239954629,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "y", 8.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "y",
                -0.514979292623,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "y",
                0.191549313416,
            ),
            ("Correlations", "Spearman's rho / z / N", "y", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "z",
                1.0,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "z", 9.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "z",
                1.0,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / z / N", "z", 9.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "c", 9.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / z / N", "c", 9.0),
            (
                "Correlations",
                "Kendall's tau_b / z / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / z / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / z / N", "m", 0.0),
            (
                "Correlations",
                "Spearman's rho / z / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / z / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / z / N", "m", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / c / Correlation Coefficient",
                "x",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / c / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / c / N", "x", 8.0),
            (
                "Correlations",
                "Spearman's rho / c / Correlation Coefficient",
                "x",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / c / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / c / N", "x", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / c / Correlation Coefficient",
                "y",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / c / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / c / N", "y", 8.0),
            (
                "Correlations",
                "Spearman's rho / c / Correlation Coefficient",
                "y",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / c / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / c / N", "y", 8.0),
            (
                "Correlations",
                "Kendall's tau_b / c / Correlation Coefficient",
                "z",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / c / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / c / N", "z", 9.0),
            (
                "Correlations",
                "Spearman's rho / c / Correlation Coefficient",
                "z",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / c / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / c / N", "z", 9.0),
            (
                "Correlations",
                "Kendall's tau_b / c / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / c / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / c / N", "c", 9.0),
            (
                "Correlations",
                "Spearman's rho / c / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / c / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / c / N", "c", 9.0),
            (
                "Correlations",
                "Kendall's tau_b / c / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / c / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / c / N", "m", 0.0),
            (
                "Correlations",
                "Spearman's rho / c / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / c / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / c / N", "m", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / m / Correlation Coefficient",
                "x",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / m / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / m / N", "x", 0.0),
            (
                "Correlations",
                "Spearman's rho / m / Correlation Coefficient",
                "x",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / m / Sig. (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / m / N", "x", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / m / Correlation Coefficient",
                "y",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / m / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / m / N", "y", 0.0),
            (
                "Correlations",
                "Spearman's rho / m / Correlation Coefficient",
                "y",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / m / Sig. (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / m / N", "y", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / m / Correlation Coefficient",
                "z",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / m / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / m / N", "z", 0.0),
            (
                "Correlations",
                "Spearman's rho / m / Correlation Coefficient",
                "z",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / m / Sig. (2-tailed)",
                "z",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / m / N", "z", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / m / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / m / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / m / N", "c", 0.0),
            (
                "Correlations",
                "Spearman's rho / m / Correlation Coefficient",
                "c",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / m / Sig. (2-tailed)",
                "c",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / m / N", "c", 0.0),
            (
                "Correlations",
                "Kendall's tau_b / m / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Kendall's tau_b / m / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Kendall's tau_b / m / N", "m", 0.0),
            (
                "Correlations",
                "Spearman's rho / m / Correlation Coefficient",
                "m",
                f64::NAN,
            ),
            (
                "Correlations",
                "Spearman's rho / m / Sig. (2-tailed)",
                "m",
                f64::NAN,
            ),
            ("Correlations", "Spearman's rho / m / N", "m", 0.0),
        ],
    );
}
