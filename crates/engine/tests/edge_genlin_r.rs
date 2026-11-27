//! Edge cases and numerical stability of GENLIN against R (`glm`, the covariance from its
//! fit) on ten invented cases: a Poisson log model with a constant covariate beside a real one
//! (left out, its parameter 0), a binomial logit model, a zero weight and a weight of two (cases
//! repeated as often as their weight), and the covariate with 10000 added (a column far from 0
//! is judged on the criterion of 1e-12 of its raw sum of squares, so 1e6 would be left out).
//! R fits the covariate shifted by its first value and maps the estimates and their covariance
//! back. The iteration stops by the parameter criterion, so the estimates agree to about 1e-6.
//! Scripts in lab/oracles/edge-gl.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y c x k w b (6F12.2).
BEGIN DATA.
2 3 1.2 7 1 0
3 5 2.1 7 2 0
7 4 3.3 7 1 1
1 6 0.8 7 1 0
4 2 1.9 7 0 1
9 7 3.1 7 1 1
5 3 2.5 7 1 0
6 8 2.9 7 1 1
3 4 1.5 7 1 0
8 5 3.6 7 1 1
END DATA.
";

const SHIFTED: &str = "DATA LIST FREE /y c x k w b (6F12.2).
BEGIN DATA.
2 3 10001.20 7 1 0
3 5 10002.10 7 2 0
7 4 10003.30 7 1 1
1 6 10000.80 7 1 0
4 2 10001.90 7 0 1
9 7 10003.10 7 1 1
5 3 10002.50 7 1 0
6 8 10002.90 7 1 1
3 4 10001.50 7 1 0
8 5 10003.60 7 1 1
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

/// The first cell of each row.
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
            (n - want).abs() <= 1e-6 + 1e-5 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn poisson_log_model() {
    let t = run("GENLIN y WITH x /MODEL x DISTRIBUTION=POISSON LINK=LOG /PRINT SOLUTION FIT.");
    check(
        &t,
        &[
            ("Parameter Estimates", "(Intercept)", "B", 0.0227886588686),
            (
                "Parameter Estimates",
                "(Intercept)",
                "Std. Error",
                0.522003117555,
            ),
            (
                "Parameter Estimates",
                "(Intercept)",
                "Hypothesis Test / Wald Chi-Square",
                0.00190586151829,
            ),
            (
                "Parameter Estimates",
                "(Intercept)",
                "Hypothesis Test / Sig.",
                0.965178474059,
            ),
            ("Parameter Estimates", "x", "B", 0.612860879106),
            ("Parameter Estimates", "x", "Std. Error", 0.183391145149),
            (
                "Parameter Estimates",
                "x",
                "Hypothesis Test / Wald Chi-Square",
                11.1677857832,
            ),
            (
                "Parameter Estimates",
                "x",
                "Hypothesis Test / Sig.",
                0.000832299001974,
            ),
            ("Goodness of Fit", "Deviance", "Value", 1.5806548714),
            ("Goodness of Fit", "Deviance", "df", 8.0),
            (
                "Goodness of Fit",
                "Pearson Chi-Square",
                "Value",
                1.59115848076,
            ),
            ("Goodness of Fit", "Log Likelihood", "Value", -17.1905655958),
        ],
    );
}

#[test]
fn binomial_logit_model() {
    let t = run("GENLIN b WITH x /MODEL x DISTRIBUTION=BINOMIAL LINK=LOGIT /PRINT SOLUTION FIT.");
    check(
        &t,
        &[
            ("Parameter Estimates", "(Intercept)", "B", 6.85256182928),
            (
                "Parameter Estimates",
                "(Intercept)",
                "Std. Error",
                4.10315791688,
            ),
            (
                "Parameter Estimates",
                "(Intercept)",
                "Hypothesis Test / Wald Chi-Square",
                2.78913452698,
            ),
            (
                "Parameter Estimates",
                "(Intercept)",
                "Hypothesis Test / Sig.",
                0.0949054720824,
            ),
            ("Parameter Estimates", "x", "B", -2.95085774925),
            ("Parameter Estimates", "x", "Std. Error", 1.70200873729),
            (
                "Parameter Estimates",
                "x",
                "Hypothesis Test / Wald Chi-Square",
                3.00588926822,
            ),
            (
                "Parameter Estimates",
                "x",
                "Hypothesis Test / Sig.",
                0.0829624401855,
            ),
            ("Goodness of Fit", "Deviance", "Value", 6.75612365945),
            ("Goodness of Fit", "Deviance", "df", 8.0),
            (
                "Goodness of Fit",
                "Pearson Chi-Square",
                "Value",
                6.18241636103,
            ),
            ("Goodness of Fit", "Log Likelihood", "Value", -3.37806182973),
        ],
    );
}

#[test]
fn zero_weight_and_weight_of_two() {
    let t = run("WEIGHT BY w.
GENLIN y WITH x /MODEL x DISTRIBUTION=POISSON LINK=LOG /PRINT SOLUTION FIT.");
    check(
        &t,
        &[
            ("Parameter Estimates", "(Intercept)", "B", -0.112738548546),
            (
                "Parameter Estimates",
                "(Intercept)",
                "Std. Error",
                0.548582013966,
            ),
            (
                "Parameter Estimates",
                "(Intercept)",
                "Hypothesis Test / Wald Chi-Square",
                0.0422339546537,
            ),
            (
                "Parameter Estimates",
                "(Intercept)",
                "Hypothesis Test / Sig.",
                0.837174456589,
            ),
            ("Parameter Estimates", "x", "B", 0.651198255137),
            ("Parameter Estimates", "x", "Std. Error", 0.191193909979),
            (
                "Parameter Estimates",
                "x",
                "Hypothesis Test / Wald Chi-Square",
                11.6005437659,
            ),
            (
                "Parameter Estimates",
                "x",
                "Hypothesis Test / Sig.",
                0.000659325387517,
            ),
            ("Goodness of Fit", "Deviance", "Value", 1.49282904671),
            ("Goodness of Fit", "Deviance", "df", 8.0),
            (
                "Goodness of Fit",
                "Pearson Chi-Square",
                "Value",
                1.53999933555,
            ),
            ("Goodness of Fit", "Log Likelihood", "Value", -17.0096989008),
        ],
    );
}

#[test]
fn covariate_near_ten_thousand_keeps_its_slope() {
    let t = run_on(
        SHIFTED,
        "GENLIN y WITH x /MODEL x DISTRIBUTION=POISSON LINK=LOG /PRINT SOLUTION FIT.",
    );
    check(
        &t,
        &[
            ("Parameter Estimates", "(Intercept)", "B", -6128.5860024),
            (
                "Parameter Estimates",
                "(Intercept)",
                "Std. Error",
                1834.41308412,
            ),
            (
                "Parameter Estimates",
                "(Intercept)",
                "Hypothesis Test / Wald Chi-Square",
                11.1615957984,
            ),
            (
                "Parameter Estimates",
                "(Intercept)",
                "Hypothesis Test / Sig.",
                0.000835080602141,
            ),
            ("Parameter Estimates", "x", "B", 0.612860879106),
            ("Parameter Estimates", "x", "Std. Error", 0.183391142726),
            (
                "Parameter Estimates",
                "x",
                "Hypothesis Test / Wald Chi-Square",
                11.1677860783,
            ),
            (
                "Parameter Estimates",
                "x",
                "Hypothesis Test / Sig.",
                0.000832298869577,
            ),
            ("Goodness of Fit", "Deviance", "Value", 1.5806548714),
            ("Goodness of Fit", "Deviance", "df", 8.0),
            (
                "Goodness of Fit",
                "Pearson Chi-Square",
                "Value",
                1.59115848076,
            ),
            ("Goodness of Fit", "Log Likelihood", "Value", -17.1905655958),
        ],
    );
}

#[test]
fn constant_covariate_is_left_out() {
    let t = run("GENLIN y WITH x k /MODEL x k DISTRIBUTION=POISSON LINK=LOG /PRINT SOLUTION.");
    let table = t.iter().find(|t| t.title == "Parameter Estimates").unwrap();
    let keys = row_keys(table);
    let k = keys.iter().position(|k| k == "k").unwrap();
    // the parameter of a redundant column is set to 0 and has no standard error
    assert_eq!(table.rows[k][1], Cell::Number(0.0));
    assert_eq!(table.rows[k][2], Cell::Empty);
}
