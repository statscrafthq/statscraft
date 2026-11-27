//! Edge cases and numerical stability of PARTIAL CORR against R on nine invented cases: control
//! for one and for two variables, the first four and three cases, a zero weight and a weight of
//! two, and the same cases with 1000000000 added to x, y, z and u. The partial correlation is
//! the correlation of the residuals of x and y on the controls (`lm`), the significance the t on
//! n - 2 - k degrees of freedom; no significance without a degree of freedom, and a correlation
//! of 1 or -1 (three cases, one control) has the significance 0 only where a degree remains.
//! The reading for a constant control, which the manual does not give, is the engine's: no
//! partial correlations. Scripts in lab/oracles/edge-pc.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /x y z u c m w (7F14.2).
BEGIN DATA.
3 2 9 1 5 . 1
7 5 8 4 5 . 2
4 4 7 2 5 . 1
9 9 6 7 5 . 0
2 1 5 3 5 . 1
8 6 4 9 5 . 1
. 7 3 5 5 . 1
6 7 2 8 5 . 1
5 . 1 6 5 . 1
END DATA.
";

const SHIFTED: &str = "DATA LIST FREE /x y z u c m w (7F14.2).
BEGIN DATA.
1000000003 1000000002 1000000009 1000000001 5 . 1
1000000007 1000000005 1000000008 1000000004 5 . 2
1000000004 1000000004 1000000007 1000000002 5 . 1
1000000009 1000000009 1000000006 1000000007 5 . 0
1000000002 1000000001 1000000005 1000000003 5 . 1
1000000008 1000000006 1000000004 1000000009 5 . 1
. 1000000007 1000000003 1000000005 5 . 1
1000000006 1000000007 1000000002 1000000008 5 . 1
1000000005 . 1000000001 1000000006 5 . 1
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
fn one_control() {
    let t = run("PARTIAL CORR VARIABLES=x y BY z /SIGNIFICANCE=TWOTAIL.");
    check(
        &t,
        &[
            ("Correlations", "z / x / Correlation", "x", 1.0),
            (
                "Correlations",
                "z / x / Significance (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "z / x / df", "x", 0.0),
            ("Correlations", "z / x / Correlation", "y", 0.921892385112),
            (
                "Correlations",
                "z / x / Significance (2-tailed)",
                "y",
                0.00891293980614,
            ),
            ("Correlations", "z / x / df", "y", 4.0),
            ("Correlations", "z / y / Correlation", "x", 0.921892385112),
            (
                "Correlations",
                "z / y / Significance (2-tailed)",
                "x",
                0.00891293980614,
            ),
            ("Correlations", "z / y / df", "x", 4.0),
            ("Correlations", "z / y / Correlation", "y", 1.0),
            (
                "Correlations",
                "z / y / Significance (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "z / y / df", "y", 0.0),
        ],
    );
}

#[test]
fn two_controls() {
    let t = run("PARTIAL CORR VARIABLES=x y BY z u /SIGNIFICANCE=TWOTAIL.");
    check(
        &t,
        &[
            ("Correlations", "z & u / x / Correlation", "x", 1.0),
            (
                "Correlations",
                "z & u / x / Significance (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "z & u / x / df", "x", 0.0),
            (
                "Correlations",
                "z & u / x / Correlation",
                "y",
                0.87891903259,
            ),
            (
                "Correlations",
                "z & u / x / Significance (2-tailed)",
                "y",
                0.0496476256513,
            ),
            ("Correlations", "z & u / x / df", "y", 3.0),
            (
                "Correlations",
                "z & u / y / Correlation",
                "x",
                0.87891903259,
            ),
            (
                "Correlations",
                "z & u / y / Significance (2-tailed)",
                "x",
                0.0496476256513,
            ),
            ("Correlations", "z & u / y / df", "x", 3.0),
            ("Correlations", "z & u / y / Correlation", "y", 1.0),
            (
                "Correlations",
                "z & u / y / Significance (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "z & u / y / df", "y", 0.0),
        ],
    );
}

#[test]
fn four_cases() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 4).\nPARTIAL CORR VARIABLES=x y BY z /SIGNIFICANCE=TWOTAIL.",
    );
    check(
        &t,
        &[
            ("Correlations", "z / x / Correlation", "x", 1.0),
            (
                "Correlations",
                "z / x / Significance (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "z / x / df", "x", 0.0),
            ("Correlations", "z / x / Correlation", "y", 0.963086824686),
            (
                "Correlations",
                "z / x / Significance (2-tailed)",
                "y",
                0.173512548945,
            ),
            ("Correlations", "z / x / df", "y", 1.0),
            ("Correlations", "z / y / Correlation", "x", 0.963086824686),
            (
                "Correlations",
                "z / y / Significance (2-tailed)",
                "x",
                0.173512548945,
            ),
            ("Correlations", "z / y / df", "x", 1.0),
            ("Correlations", "z / y / Correlation", "y", 1.0),
            (
                "Correlations",
                "z / y / Significance (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "z / y / df", "y", 0.0),
        ],
    );
}

#[test]
fn three_cases_leave_no_degree_of_freedom() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 3).\nPARTIAL CORR VARIABLES=x y BY z /SIGNIFICANCE=TWOTAIL.",
    );
    check(
        &t,
        &[
            ("Correlations", "z / x / Correlation", "x", 1.0),
            (
                "Correlations",
                "z / x / Significance (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "z / x / df", "x", 0.0),
            ("Correlations", "z / x / Correlation", "y", 1.0),
            (
                "Correlations",
                "z / x / Significance (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "z / x / df", "y", 0.0),
            ("Correlations", "z / y / Correlation", "x", 1.0),
            (
                "Correlations",
                "z / y / Significance (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "z / y / df", "x", 0.0),
            ("Correlations", "z / y / Correlation", "y", 1.0),
            (
                "Correlations",
                "z / y / Significance (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "z / y / df", "y", 0.0),
        ],
    );
}

#[test]
fn zero_weight_and_weight_of_two() {
    let t = run("WEIGHT BY w.\nPARTIAL CORR VARIABLES=x y BY z /SIGNIFICANCE=TWOTAIL.");
    check(
        &t,
        &[
            ("Correlations", "z / x / Correlation", "x", 1.0),
            (
                "Correlations",
                "z / x / Significance (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "z / x / df", "x", 0.0),
            ("Correlations", "z / x / Correlation", "y", 0.89951760306),
            (
                "Correlations",
                "z / x / Significance (2-tailed)",
                "y",
                0.0146377972261,
            ),
            ("Correlations", "z / x / df", "y", 4.0),
            ("Correlations", "z / y / Correlation", "x", 0.89951760306),
            (
                "Correlations",
                "z / y / Significance (2-tailed)",
                "x",
                0.0146377972261,
            ),
            ("Correlations", "z / y / df", "x", 4.0),
            ("Correlations", "z / y / Correlation", "y", 1.0),
            (
                "Correlations",
                "z / y / Significance (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "z / y / df", "y", 0.0),
        ],
    );
}

#[test]
fn values_near_one_billion_give_the_same_partial_correlations() {
    let t = run_on(
        SHIFTED,
        "PARTIAL CORR VARIABLES=x y BY z u /SIGNIFICANCE=TWOTAIL.",
    );
    check(
        &t,
        &[
            ("Correlations", "z & u / x / Correlation", "x", 1.0),
            (
                "Correlations",
                "z & u / x / Significance (2-tailed)",
                "x",
                f64::NAN,
            ),
            ("Correlations", "z & u / x / df", "x", 0.0),
            (
                "Correlations",
                "z & u / x / Correlation",
                "y",
                0.87891903259,
            ),
            (
                "Correlations",
                "z & u / x / Significance (2-tailed)",
                "y",
                0.0496476256513,
            ),
            ("Correlations", "z & u / x / df", "y", 3.0),
            (
                "Correlations",
                "z & u / y / Correlation",
                "x",
                0.87891903259,
            ),
            (
                "Correlations",
                "z & u / y / Significance (2-tailed)",
                "x",
                0.0496476256513,
            ),
            ("Correlations", "z & u / y / df", "x", 3.0),
            ("Correlations", "z & u / y / Correlation", "y", 1.0),
            (
                "Correlations",
                "z & u / y / Significance (2-tailed)",
                "y",
                f64::NAN,
            ),
            ("Correlations", "z & u / y / df", "y", 0.0),
        ],
    );
}
