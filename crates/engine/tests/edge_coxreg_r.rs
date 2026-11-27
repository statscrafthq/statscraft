//! Edge cases and numerical stability of COXREG against R (`survival::coxph`, Breslow ties) on
//! twelve invented cases: tied event times, a constant covariate entered beside a real one (left
//! out), a zero weight and a weight of two (cases repeated as often as their weight), the
//! covariate with 1000000 added and the times with 1000000000 added: the partial likelihood
//! does not change, so the coefficient, its standard error and the likelihood are those of the
//! unshifted data. The iteration stops by the parameter criterion, so the estimates agree to
//! about 1e-5. Scripts in lab/oracles/edge-cx.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /t e x k w (5F14.2).
BEGIN DATA.
5 1 1.2 7 1
8 1 2.1 7 2
8 0 3.3 7 1
12 1 0.8 7 1
15 0 1.9 7 1
3 1 3.1 7 1
7 1 2.5 7 0
7 1 2.9 7 1
9 0 1.5 7 1
20 1 3.6 7 1
11 1 1.1 7 1
13 1 2.2 7 1
END DATA.
";

const SHIFTED_X: &str = "DATA LIST FREE /t e x k w (5F14.2).
BEGIN DATA.
5 1 1000001.20 7 1
8 1 1000002.10 7 2
8 0 1000003.30 7 1
12 1 1000000.80 7 1
15 0 1000001.90 7 1
3 1 1000003.10 7 1
7 1 1000002.50 7 0
7 1 1000002.90 7 1
9 0 1000001.50 7 1
20 1 1000003.60 7 1
11 1 1000001.10 7 1
13 1 1000002.20 7 1
END DATA.
";

const SHIFTED_T: &str = "DATA LIST FREE /t e x k w (5F14.2).
BEGIN DATA.
1000000005.00 1 1.2 7 1
1000000008.00 1 2.1 7 2
1000000008.00 0 3.3 7 1
1000000012.00 1 0.8 7 1
1000000015.00 0 1.9 7 1
1000000003.00 1 3.1 7 1
1000000007.00 1 2.5 7 0
1000000007.00 1 2.9 7 1
1000000009.00 0 1.5 7 1
1000000020.00 1 3.6 7 1
1000000011.00 1 1.1 7 1
1000000013.00 1 2.2 7 1
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

/// The row headers (the response category and the parameter, or the model) joined with " / ",
/// the first carried from the row above.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = String::new();
    table
        .rows
        .iter()
        .map(|row| {
            let text = |c: &Cell| match c {
                Cell::Text(t) => t.clone(),
                Cell::Number(n) => format!("{n}"),
                Cell::Empty => String::new(),
            };
            let first = text(&row[0]);
            if !first.is_empty() {
                carried = first;
            }
            match row.get(1) {
                Some(Cell::Text(second)) if !second.is_empty() => format!("{carried} / {second}"),
                _ => carried.clone(),
            }
        })
        .collect()
}

/// Each expectation: the table title, the row, the column and the value (NaN: an empty cell).
fn check(tables: &[Table], expected: &[(&str, &str, &str, f64)]) {
    for (title, row, column, want) in expected {
        // the block 0 tables come first: the last one of a title is the fitted step
        let t = tables
            .iter()
            .rev()
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
            (n - want).abs() <= 1e-6 + 1e-5 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn tied_event_times() {
    let t = run("COXREG t /STATUS=e(1) /METHOD=ENTER x /PRINT=CI(95).");
    check(
        &t,
        &[
            (
                "Variables in the Equation",
                "Step 1 / x",
                "B",
                -0.232750569632,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "SE",
                0.411302590475,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Wald",
                0.320227493093,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Sig.",
                0.571470962014,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Exp(B)",
                0.792351185326,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "95% CI for Exp(B) / Lower",
                0.35384764717,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "95% CI for Exp(B) / Upper",
                1.77426755811,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                "Step 1",
                "-2 Log Likelihood",
                30.9933734467,
            ),
        ],
    );
}

#[test]
fn zero_weight_and_weight_of_two() {
    let t = run("WEIGHT BY w.
COXREG t /STATUS=e(1) /METHOD=ENTER x /PRINT=CI(95).");
    check(
        &t,
        &[
            (
                "Variables in the Equation",
                "Step 1 / x",
                "B",
                -0.270064759621,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "SE",
                0.42270859741,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Wald",
                0.408181834158,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Sig.",
                0.522893779989,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Exp(B)",
                0.763330059771,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "95% CI for Exp(B) / Lower",
                0.333351301207,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "95% CI for Exp(B) / Upper",
                1.74792412101,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                "Step 1",
                "-2 Log Likelihood",
                30.9222004423,
            ),
        ],
    );
}

#[test]
fn covariate_near_one_million() {
    let t = run_on(
        SHIFTED_X,
        "COXREG t /STATUS=e(1) /METHOD=ENTER x /PRINT=CI(95).",
    );
    check(
        &t,
        &[
            (
                "Variables in the Equation",
                "Step 1 / x",
                "B",
                -0.232750569649,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "SE",
                0.411302590481,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Wald",
                0.32022749313,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Sig.",
                0.571470961992,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Exp(B)",
                0.792351185313,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "95% CI for Exp(B) / Lower",
                0.35384764716,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "95% CI for Exp(B) / Upper",
                1.7742675581,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                "Step 1",
                "-2 Log Likelihood",
                30.9933734467,
            ),
        ],
    );
}

#[test]
fn times_near_one_billion() {
    let t = run_on(
        SHIFTED_T,
        "COXREG t /STATUS=e(1) /METHOD=ENTER x /PRINT=CI(95).",
    );
    check(
        &t,
        &[
            (
                "Variables in the Equation",
                "Step 1 / x",
                "B",
                -0.232750569632,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "SE",
                0.411302590475,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Wald",
                0.320227493093,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Sig.",
                0.571470962014,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Exp(B)",
                0.792351185326,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "95% CI for Exp(B) / Lower",
                0.35384764717,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "95% CI for Exp(B) / Upper",
                1.77426755811,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                "Step 1",
                "-2 Log Likelihood",
                30.9933734467,
            ),
        ],
    );
}

#[test]
fn constant_covariate_is_left_out() {
    let t = run("COXREG t /STATUS=e(1) /METHOD=ENTER x k.");
    let table = t
        .iter()
        .find(|t| t.title == "Variables in the Equation")
        .unwrap();
    let keys = row_keys(table);
    assert!(
        keys.contains(&"Step 1 / x".to_string()) && !keys.iter().any(|k| k.ends_with("/ k")),
        "{keys:?}"
    );
}
