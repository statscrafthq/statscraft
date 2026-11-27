//! QUANTILE REGRESSION numerical stability against R (`quantreg::rq`, the Barrodale-Roberts
//! simplex) on 48 invented cases (y on x1 and x2): x2 in hundred millions of its unit (times
//! 1E8) and in hundred-millionths (times 1E-8), at the median and at the first quartile. R fits
//! standardized columns and maps the coefficients back (a quantile regression is equivariant to a
//! linear change of its columns). The standard errors of the engine's own estimator (the
//! sparsity from the residual quantiles, checked elsewhere against the algorithm's formulas) must
//! move with the units as the coefficients do, so the scaled runs are held to the unscaled
//! standard errors times the scale. The simplex used to stop on "the design has no full set of
//! independent cases" for a column in hundred millions, and gave an empty standard error for
//! one in hundred-millionths. Scripts in lab/oracles/edge-qr.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g x1 x2 z1 y y2 dose n d w.
BEGIN DATA
1 4.62 11.53 -0.23 2.42 4.29 0.93 20 2 1.72
2 4.68 11.67 1.2 2.62 4.13 4.61 20 14 1.64
3 6.0 12.84 1.44 4.81 5.49 7.07 20 18 0.68
1 3.05 14.78 0.62 1.02 5.63 3.43 20 9 1.13
2 3.68 12.84 1.47 1.65 3.23 5.57 20 18 1.81
3 4.8 8.72 -0.37 4.71 4.87 3.23 20 8 1.54
1 4.9 12.86 0.86 4.05 3.92 2.97 20 6 1.05
2 4.47 10.4 0.71 3.68 5.15 5.18 20 10 0.76
3 5.6 11.23 1.5 3.59 5.98 7.85 20 19 1.87
1 7.23 7.98 0.88 4.01 4.73 3.65 20 7 0.92
2 5.25 6.46 -1.56 4.62 5.04 5.09 20 15 1.5
3 4.15 11.35 -2.08 4.31 5.45 5.45 20 15 1.0
1 3.72 7.48 2.67 4.37 4.59 1.29 20 3 1.7
2 1.11 15.11 0.99 1.18 4.59 3.9 20 15 0.52
3 4.8 6.22 0.42 6.53 3.88 6.64 20 19 1.76
1 4.72 13.07 -1.08 2.21 6.3 2.7 20 5 1.1
2 5.31 10.17 -0.24 4.88 3.83 6.13 20 17 1.74
3 4.06 4.52 -1.07 4.76 1.33 2.22 20 6 0.9
1 4.87 7.98 -0.17 1.28 3.65 4.09 20 11 0.68
2 5.06 11.57 -0.41 5.9 4.72 0.63 20 0 0.54
3 6.74 10.08 -0.81 6.13 5.77 2.87 20 4 1.9
1 6.07 10.01 0.79 3.71 5.59 3.3 20 6 1.61
2 6.15 9.65 -0.47 4.32 4.27 1.75 20 2 1.29
3 4.47 8.05 0.63 5.41 3.95 7.76 20 20 1.89
1 6.35 4.75 0.74 6.02 5.17 4.42 20 10 1.37
2 5.55 8.37 0.46 4.81 4.56 2.76 20 4 1.13
3 4.59 8.18 0.48 3.15 3.05 7.77 20 19 1.09
1 4.97 14.99 -1.24 1.6 2.64 7.49 20 19 1.15
2 6.62 11.03 -1.56 6.6 6.05 7.23 20 19 1.63
3 6.34 13.68 -0.71 4.68 5.63 2.25 20 1 1.63
1 5.01 11.1 1.24 3.45 4.04 7.13 20 19 0.8
2 8.74 10.04 0.75 8.17 8.28 4.84 20 12 1.06
3 6.09 7.56 -0.11 7.84 4.8 4.97 20 19 1.33
1 4.76 9.16 -0.34 4.36 6.13 3.57 20 8 0.74
2 6.89 10.35 -1.39 3.71 6.9 3.28 20 6 0.74
3 6.11 10.47 -0.69 3.65 6.04 6.72 20 20 1.37
1 3.04 11.98 -1.01 1.69 5.89 4.17 20 15 1.07
2 5.3 11.55 -0.16 5.85 2.55 5.99 20 14 0.89
3 3.69 9.9 0.62 3.63 4.87 5.6 20 15 1.65
1 7.99 8.25 0.22 6.4 5.93 6.65 20 19 1.04
2 5.32 13.92 -0.57 2.72 4.84 1.51 20 3 1.3
3 3.8 8.16 0.41 5.71 3.96 0.59 20 1 1.18
1 5.11 11.4 0.03 3.45 4.58 5.71 20 15 0.99
2 7.14 10.45 2.18 4.53 4.74 5.49 20 15 0.56
3 4.25 6.02 -0.53 3.55 4.35 3.23 20 8 0.81
1 5.8 8.67 0.95 5.3 5.33 2.62 20 5 0.8
2 5.32 1.74 1.16 6.77 1.43 1.88 20 2 0.63
3 5.14 10.52 -0.15 4.67 5.23 7.26 20 20 1.19
END DATA.
";

fn run(transform: &str, quantile: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!(
        "{DATA}{transform}QUANTILE REGRESSION y WITH x1 x2 /CRITERIA QUANTILE={quantile}."
    ));
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

/// The coefficient and standard error of each parameter row.
fn estimates(tables: &[Table]) -> Vec<(f64, f64)> {
    let table = tables
        .iter()
        .find(|t| t.title == "Parameter Estimates")
        .expect("Parameter Estimates");
    let titles = table.column_titles();
    let at = |name: &str| titles.iter().position(|t| t == name).expect("column");
    let (b, se) = (at("Coefficient"), at("Std. Error"));
    table
        .rows
        .iter()
        .map(|r| match (&r[b], &r[se]) {
            (Cell::Number(b), Cell::Number(se)) => (*b, *se),
            other => panic!("not numbers: {other:?}"),
        })
        .collect()
}

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-7 * want.abs() + 1e-14,
        "{what}: {got} is not {want}"
    );
}

/// The coefficients of R for the three parameters, and the scale of x2 against the unscaled run.
fn check(transform: &str, quantile: &str, scale: f64, r: [f64; 3]) {
    let plain = estimates(&run("", quantile));
    let got = estimates(&run(transform, quantile));
    let factors = [1.0, 1.0, scale];
    for i in 0..3 {
        near(got[i].0, r[i], &format!("coefficient {i}"));
        near(
            got[i].1,
            plain[i].1 * factors[i],
            &format!("standard error {i}"),
        );
    }
}

#[test]
fn the_unchanged_data_agrees_with_r() {
    check(
        "",
        "0.5",
        1.0,
        [3.93917913211, 0.636015254739, -0.3176898203],
    );
}

#[test]
fn a_predictor_in_hundred_millions_of_units_agrees_with_r() {
    check(
        "COMPUTE x2 = x2 * 1E8.\n",
        "0.5",
        1e-08,
        [3.93917913211, 0.636015254739, -3.176898203e-09],
    );
}

#[test]
fn a_predictor_in_hundred_millionths_of_a_unit_agrees_with_r() {
    check(
        "COMPUTE x2 = x2 * 1E-8.\n",
        "0.5",
        100000000.0,
        [3.93917913211, 0.636015254739, -31768982.03],
    );
}

#[test]
fn the_first_quartile_agrees_with_r() {
    check(
        "",
        "0.25",
        1.0,
        [4.09196111237, 0.504953571471, -0.328632718668],
    );
}
