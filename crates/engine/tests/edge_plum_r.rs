//! Edge cases and numerical stability of PLUM (logit link) against R (`MASS::polr`, the
//! covariance from its Hessian) on fifteen invented cases: a constant covariate entered beside a
//! real one (left out, its parameter 0), a zero weight and a weight of two (cases repeated as
//! often as their weight), and the covariate with 1000000 added: the slope, the fit and the
//! pseudo R squares are those of the unshifted covariate and each threshold moves by the slope
//! times 1000000. R fits the covariate shifted by its first value and maps the estimates and
//! their covariance back. The iteration stops by the log-likelihood criterion, so the estimates
//! agree to about 1e-5. Scripts in lab/oracles/edge-pl.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y x g k w (5F12.2).
BEGIN DATA.
1 1.5 1 7 1
1 2.1 1 7 2
2 3.2 1 7 1
1 2.8 2 7 1
3 4.1 2 7 0
2 3.7 2 7 1
3 3.0 1 7 1
2 2.9 2 7 1
1 4.8 1 7 1
1 1.9 2 7 1
2 3.3 1 7 1
3 4.4 2 7 1
2 2.2 1 7 1
3 3.9 1 7 2
1 3.4 2 7 1
END DATA.
";

const SHIFTED: &str = "DATA LIST FREE /y x g k w (5F12.2).
BEGIN DATA.
1 1000001.50 1 7 1
1 1000002.10 1 7 2
2 1000003.20 1 7 1
1 1000002.80 2 7 1
3 1000004.10 2 7 0
2 1000003.70 2 7 1
3 1000003.00 1 7 1
2 1000002.90 2 7 1
1 1000004.80 1 7 1
1 1000001.90 2 7 1
2 1000003.30 1 7 1
3 1000004.40 2 7 1
2 1000002.20 1 7 1
3 1000003.90 1 7 2
1 1000003.40 2 7 1
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
            (n - want).abs() <= 1e-6 + 1e-5 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn one_covariate() {
    let t = run("PLUM y WITH x /PRINT=PARAMETER FIT SUMMARY.");
    check(
        &t,
        &[
            (
                "Model Fitting Information",
                "Intercept Only",
                "-2 Log Likelihood",
                32.5556583941,
            ),
            (
                "Model Fitting Information",
                "Final",
                "-2 Log Likelihood",
                28.4536341823,
            ),
            (
                "Model Fitting Information",
                "Final",
                "Chi-Square",
                4.10202421176,
            ),
            ("Pseudo R-Square", "Cox and Snell", "Value", 0.239263534063),
            ("Pseudo R-Square", "Nagelkerke", "Value", 0.270090153315),
            ("Pseudo R-Square", "McFadden", "Value", 0.126000345688),
            (
                "Parameter Estimates",
                "Threshold / [y=1]",
                "Estimate",
                3.42445636801,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=1]",
                "Std. Error",
                2.15952360667,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=1]",
                "Wald",
                2.51459103272,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=1]",
                "Sig.",
                0.112796892781,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=2]",
                "Estimate",
                5.18902631099,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=2]",
                "Std. Error",
                2.40561606667,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=2]",
                "Wald",
                4.65285066846,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=2]",
                "Sig.",
                0.0310020222638,
            ),
            (
                "Parameter Estimates",
                "Location / x",
                "Estimate",
                1.27990606153,
            ),
            (
                "Parameter Estimates",
                "Location / x",
                "Std. Error",
                0.697330702845,
            ),
            ("Parameter Estimates", "Location / x", "Wald", 3.36882628725),
            (
                "Parameter Estimates",
                "Location / x",
                "Sig.",
                0.0664410743356,
            ),
        ],
    );
}

#[test]
fn zero_weight_and_weight_of_two() {
    let t = run("WEIGHT BY w.\nPLUM y WITH x /PRINT=PARAMETER FIT SUMMARY.");
    check(
        &t,
        &[
            (
                "Model Fitting Information",
                "Intercept Only",
                "-2 Log Likelihood",
                34.2953630614,
            ),
            (
                "Model Fitting Information",
                "Final",
                "-2 Log Likelihood",
                29.4504089302,
            ),
            (
                "Model Fitting Information",
                "Final",
                "Chi-Square",
                4.84495413125,
            ),
            ("Pseudo R-Square", "Cox and Snell", "Value", 0.261260285501),
            ("Pseudo R-Square", "Nagelkerke", "Value", 0.29596128382),
            ("Pseudo R-Square", "McFadden", "Value", 0.141271405192),
            (
                "Parameter Estimates",
                "Threshold / [y=1]",
                "Estimate",
                3.81457127598,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=1]",
                "Std. Error",
                2.14268401934,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=1]",
                "Wald",
                3.16938652589,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=1]",
                "Sig.",
                0.0750306241619,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=2]",
                "Estimate",
                5.53886549191,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=2]",
                "Std. Error",
                2.39280400514,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=2]",
                "Wald",
                5.35830442032,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=2]",
                "Sig.",
                0.0206238163102,
            ),
            (
                "Parameter Estimates",
                "Location / x",
                "Estimate",
                1.38026458864,
            ),
            (
                "Parameter Estimates",
                "Location / x",
                "Std. Error",
                0.701559764168,
            ),
            ("Parameter Estimates", "Location / x", "Wald", 3.87075198629),
            (
                "Parameter Estimates",
                "Location / x",
                "Sig.",
                0.0491345022293,
            ),
        ],
    );
}

#[test]
fn covariate_near_one_million_keeps_its_slope() {
    let t = run_on(SHIFTED, "PLUM y WITH x /PRINT=PARAMETER FIT SUMMARY.");
    check(
        &t,
        &[
            (
                "Model Fitting Information",
                "Intercept Only",
                "-2 Log Likelihood",
                32.5556583941,
            ),
            (
                "Model Fitting Information",
                "Final",
                "-2 Log Likelihood",
                28.4536341826,
            ),
            (
                "Model Fitting Information",
                "Final",
                "Chi-Square",
                4.10202421154,
            ),
            ("Pseudo R-Square", "Cox and Snell", "Value", 0.239263534051),
            ("Pseudo R-Square", "Nagelkerke", "Value", 0.270090153302),
            ("Pseudo R-Square", "McFadden", "Value", 0.126000345681),
            (
                "Parameter Estimates",
                "Threshold / [y=1]",
                "Estimate",
                1279909.46737,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=1]",
                "Std. Error",
                697332.777764,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=1]",
                "Wald",
                3.3688241682,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=1]",
                "Sig.",
                0.0664411597991,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=2]",
                "Estimate",
                1279911.23194,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=2]",
                "Std. Error",
                697333.022601,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=2]",
                "Wald",
                3.36883109157,
            ),
            (
                "Parameter Estimates",
                "Threshold / [y=2]",
                "Sig.",
                0.066440880573,
            ),
            (
                "Parameter Estimates",
                "Location / x",
                "Estimate",
                1.27990604291,
            ),
            (
                "Parameter Estimates",
                "Location / x",
                "Std. Error",
                0.69733070015,
            ),
            ("Parameter Estimates", "Location / x", "Wald", 3.36882621529),
            (
                "Parameter Estimates",
                "Location / x",
                "Sig.",
                0.0664410772379,
            ),
        ],
    );
}

#[test]
fn constant_covariate_is_left_out() {
    let t = run("PLUM y WITH x k /PRINT=PARAMETER.");
    let table = t.iter().find(|t| t.title == "Parameter Estimates").unwrap();
    let keys = row_keys(table);
    let k = keys.iter().position(|k| k == "Location / k").unwrap();
    // the parameter of a redundant column is set to 0 and has no standard error
    assert_eq!(table.rows[k][2], Cell::Number(0.0));
    assert_eq!(table.rows[k][3], Cell::Empty);
}
