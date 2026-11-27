//! Edge cases and numerical stability of UNIANOVA (Type III) against R on ten invented cases:
//! an unbalanced two-factor design with cells of one case, a covariate, three cases (one
//! residual degree of freedom), a zero weight and a weight of two, and the cases with
//! 1000000000 added to y and 1000 to the covariate (every row but the intercept is
//! unchanged). R: the columns of each term dropped from the sum-to-zero design matrix (`lm`
//! design, `qr` residuals), which is the Type III sum of squares when no cell is empty. Scripts
//! in lab/oracles/edge-un.
//! A covariate that is 1e6 plus a spread of a few is taken for a multiple of the constant by the
//! singularity criterion (1e-8 of the pivot's original size) and left out: not tested.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y a b v c w (6F14.2).
BEGIN DATA.
3 1 1 2 5 1
7 1 2 5 5 2
4 1 1 4 5 1
9 2 1 9 5 0
2 2 2 1 5 1
8 2 1 6 5 1
5 3 1 7 5 1
6 3 2 3 5 1
. 3 2 4 5 1
4 1 2 8 5 1
END DATA.
";

const SHIFTED: &str = "DATA LIST FREE /y a b v c w (6F14.2).
BEGIN DATA.
1000000003 1 1 1002 5 1
1000000007 1 2 1005 5 2
1000000004 1 1 1004 5 1
1000000009 2 1 1009 5 0
1000000002 2 2 1001 5 1
1000000008 2 1 1006 5 1
1000000005 3 1 1007 5 1
1000000006 3 2 1003 5 1
. 3 2 1004 5 1
1000000004 1 2 1008 5 1
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
fn unbalanced_two_factor_design_with_cells_of_one_case() {
    let t = run("UNIANOVA y BY a b /METHOD=SSTYPE(3) /DESIGN=a b a*b.");
    check(
        &t,
        &[
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Type III Sum of Squares",
                206.722222222,
            ),
            ("Tests of Between-Subjects Effects", "Intercept", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "F",
                112.757575758,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Sig.",
                0.00178466804262,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Type III Sum of Squares",
                1.65384615385,
            ),
            ("Tests of Between-Subjects Effects", "a", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Mean Square",
                0.826923076923,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "F",
                0.451048951049,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Sig.",
                0.67411600733,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Type III Sum of Squares",
                2.72222222222,
            ),
            ("Tests of Between-Subjects Effects", "b", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Mean Square",
                2.72222222222,
            ),
            ("Tests of Between-Subjects Effects", "b", "F", 1.48484848485),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Sig.",
                0.310100901082,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Type III Sum of Squares",
                31.1153846154,
            ),
            ("Tests of Between-Subjects Effects", "a * b", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Mean Square",
                15.5576923077,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "F",
                8.48601398601,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a * b",
                "Sig.",
                0.0582168407115,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Type III Sum of Squares",
                5.5,
            ),
            ("Tests of Between-Subjects Effects", "Error", "df", 3.0),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Mean Square",
                1.83333333333,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "Type III Sum of Squares",
                44.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "df",
                8.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Type III Sum of Squares",
                38.5,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "df",
                5.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "F",
                4.2,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Sig.",
                0.133554947979,
            ),
        ],
    );
}

#[test]
fn two_factors_and_a_covariate() {
    let t = run("UNIANOVA y BY a b WITH v /METHOD=SSTYPE(3) /DESIGN=a b v.");
    check(
        &t,
        &[
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Type III Sum of Squares",
                14.324547807,
            ),
            ("Tests of Between-Subjects Effects", "Intercept", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "F",
                2.63330255706,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Sig.",
                0.17996454195,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Type III Sum of Squares",
                3.86560378167,
            ),
            ("Tests of Between-Subjects Effects", "a", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Mean Square",
                1.93280189083,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "F",
                0.355309796164,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Sig.",
                0.721047564968,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Type III Sum of Squares",
                0.0593119738415,
            ),
            ("Tests of Between-Subjects Effects", "b", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Mean Square",
                0.0593119738415,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "F",
                0.010903406829,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Sig.",
                0.921862852487,
            ),
            (
                "Tests of Between-Subjects Effects",
                "v",
                "Type III Sum of Squares",
                14.8563235177,
            ),
            ("Tests of Between-Subjects Effects", "v", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "v",
                "Mean Square",
                14.8563235177,
            ),
            ("Tests of Between-Subjects Effects", "v", "F", 2.7310596631),
            (
                "Tests of Between-Subjects Effects",
                "v",
                "Sig.",
                0.17375776741,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Type III Sum of Squares",
                21.7590610977,
            ),
            ("Tests of Between-Subjects Effects", "Error", "df", 4.0),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Mean Square",
                5.43976527442,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "Type III Sum of Squares",
                44.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "df",
                8.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Type III Sum of Squares",
                22.2409389023,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "df",
                4.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "F",
                1.02214607526,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Sig.",
                0.491786502177,
            ),
        ],
    );
}

#[test]
fn three_cases_leave_one_residual_degree_of_freedom() {
    let t = run("COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 3).\nUNIANOVA y BY b /DESIGN=b.");
    check(
        &t,
        &[
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Type III Sum of Squares",
                73.5,
            ),
            ("Tests of Between-Subjects Effects", "Intercept", "df", 1.0),
            ("Tests of Between-Subjects Effects", "Intercept", "F", 147.0),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Sig.",
                0.0523889328216,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Type III Sum of Squares",
                8.16666666667,
            ),
            ("Tests of Between-Subjects Effects", "b", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Mean Square",
                8.16666666667,
            ),
            ("Tests of Between-Subjects Effects", "b", "F", 16.3333333333),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Sig.",
                0.154420958311,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Type III Sum of Squares",
                0.5,
            ),
            ("Tests of Between-Subjects Effects", "Error", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Mean Square",
                0.5,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "Type III Sum of Squares",
                8.66666666667,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "df",
                2.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Type III Sum of Squares",
                8.16666666667,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "df",
                1.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "F",
                16.3333333333,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Sig.",
                0.154420958311,
            ),
        ],
    );
}

#[test]
fn zero_weight_and_weight_of_two() {
    let t = run("WEIGHT BY w.\nUNIANOVA y BY a b /METHOD=SSTYPE(3) /DESIGN=a b.");
    check(
        &t,
        &[
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Type III Sum of Squares",
                198.868353345,
            ),
            ("Tests of Between-Subjects Effects", "Intercept", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "F",
                30.7024826216,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Intercept",
                "Sig.",
                0.00262796829915,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Type III Sum of Squares",
                0.413636363636,
            ),
            ("Tests of Between-Subjects Effects", "a", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Mean Square",
                0.206818181818,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "F",
                0.0319298245614,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Sig.",
                0.96877039899,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Type III Sum of Squares",
                0.113636363636,
            ),
            ("Tests of Between-Subjects Effects", "b", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Mean Square",
                0.113636363636,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "F",
                0.0175438596491,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Sig.",
                0.899791094848,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Type III Sum of Squares",
                32.3863636364,
            ),
            ("Tests of Between-Subjects Effects", "Error", "df", 5.0),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Mean Square",
                6.47727272727,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "Type III Sum of Squares",
                32.8888888889,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "df",
                8.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Type III Sum of Squares",
                0.502525252525,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "df",
                3.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "F",
                0.025860948668,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Sig.",
                0.993675229388,
            ),
        ],
    );
}

#[test]
fn values_near_one_billion_leave_every_effect_but_the_intercept_unchanged() {
    let t = run_on(
        SHIFTED,
        "UNIANOVA y BY a b WITH v /METHOD=SSTYPE(3) /DESIGN=a b v.",
    );
    check(
        &t,
        &[
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Type III Sum of Squares",
                3.86560378167,
            ),
            ("Tests of Between-Subjects Effects", "a", "df", 2.0),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Mean Square",
                1.93280189083,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "F",
                0.355309796164,
            ),
            (
                "Tests of Between-Subjects Effects",
                "a",
                "Sig.",
                0.721047564968,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Type III Sum of Squares",
                0.0593119738415,
            ),
            ("Tests of Between-Subjects Effects", "b", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Mean Square",
                0.0593119738415,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "F",
                0.010903406829,
            ),
            (
                "Tests of Between-Subjects Effects",
                "b",
                "Sig.",
                0.921862852487,
            ),
            (
                "Tests of Between-Subjects Effects",
                "v",
                "Type III Sum of Squares",
                14.8563235177,
            ),
            ("Tests of Between-Subjects Effects", "v", "df", 1.0),
            (
                "Tests of Between-Subjects Effects",
                "v",
                "Mean Square",
                14.8563235177,
            ),
            ("Tests of Between-Subjects Effects", "v", "F", 2.7310596631),
            (
                "Tests of Between-Subjects Effects",
                "v",
                "Sig.",
                0.17375776741,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Type III Sum of Squares",
                21.7590610977,
            ),
            ("Tests of Between-Subjects Effects", "Error", "df", 4.0),
            (
                "Tests of Between-Subjects Effects",
                "Error",
                "Mean Square",
                5.43976527442,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "Type III Sum of Squares",
                44.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Total",
                "df",
                8.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Type III Sum of Squares",
                22.2409389023,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "df",
                4.0,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "F",
                1.02214607526,
            ),
            (
                "Tests of Between-Subjects Effects",
                "Corrected Model",
                "Sig.",
                0.491786502177,
            ),
        ],
    );
}
