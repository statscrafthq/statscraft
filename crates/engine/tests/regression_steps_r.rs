//! REGRESSION model building against R's lm on 60 invented cases (six predictors, three of them
//! real): stepwise selection by the probability of F (entry 0.05, removal 0.10, written out with
//! `add1` and `drop1`), forward and backward selection, the R square change and its F test, the
//! coefficient tables with confidence intervals, standardized coefficients, zero-order, partial
//! and part correlations, tolerance and VIF, the collinearity diagnostics (eigenvalues of the
//! unit-length scaled cross-products, condition indices and variance proportions) and the
//! Durbin-Watson statistic.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y x1 x2 x3 x4 x5 x6.
BEGIN DATA
7.77 1.9 0.84 3.12 0.06 0.7 0.11
1.94 0.02 1.11 -0.68 -0.52 -1.53 -0.79
2.4 0.54 -0.27 -0.67 -0.2 -2.33 -1.24
5.3 2.09 2.39 -0.52 -1.44 0.03 0.65
2.04 0.57 0.05 -1.93 -1.46 -2.7 1
3.46 0.75 -0.2 0.74 1.31 -1.58 0.04
2.93 0.05 0.38 0.77 -1.55 -0.01 0.79
-0.11 0.66 -0.86 -0.45 1.46 -2.27 0.3
0.6 0.69 0.03 -0.12 1.85 0.95 -0.71
4.83 0.97 1.14 0.71 1.83 -0.75 -0.88
0.38 -0.89 -1.74 0.84 1.15 -1.01 0.2
-0.93 -2.23 0.17 -0.52 1.6 0.48 -0.46
1.66 -0.05 -0.34 -0.35 0.76 -0.38 -0.9
1.17 -0.42 -1.08 0.11 1.01 -0.39 0.67
-0.33 -0.66 -0.26 -1.15 -0.79 -1.09 0.03
0.23 -0.65 0.08 -0.06 -0.23 0.87 1.58
5.61 1.49 2.88 0.7 0.07 1.04 -0.11
0.23 0.02 0.27 0.95 1.98 -1.83 1.41
2.61 0.24 0.05 0.83 0.21 -0.54 -0.94
-0.34 -1.08 -0.66 -0.97 0.88 0.11 1.32
2.06 -0.15 -1.16 1.55 1.08 -1.77 1.01
2.11 -0.96 -1.12 2.14 1.33 0.06 1.43
4.3 -0.05 -1.36 -0.16 0.11 0.57 0.02
1.16 0.03 0.55 -0.97 0.3 -0.16 -2.01
2.21 0.78 -0.83 -0.57 -0.98 0.57 -2.16
-1.58 -1.18 -0.87 -1.29 -0.91 1.77 -1.04
3.72 0.7 -0.15 -0.81 0.99 0.65 -0.23
2.32 -0.22 0.66 -0.06 -0.01 -2.09 -0.17
0.63 -0.33 -1.85 -0.91 -0.09 0.95 0.83
0.68 1.33 -1.58 -0.18 0.48 -0.65 -1.13
0.92 -0.65 0.6 -0.46 -1.34 -1.62 -0.94
1.77 1.2 2.04 -1.26 0.37 2.02 0.2
-1.4 -0.8 -0.59 -0.9 -0.7 -0.79 1.72
3.15 -0.88 -0.09 1 0.43 0.33 0.71
0.91 -0.73 -0.15 0.37 2.14 -2.04 0.09
6.03 0.13 -0.03 0.91 1.83 0.65 1.02
1.03 -0.64 -2.07 1.17 -0.07 0.44 -0.18
5.11 -1.07 -0.63 3.06 1.07 1.17 1.3
1.4 -0.44 -0.25 0.18 -0.55 -0.07 0.28
1.78 -0.02 0.69 1.36 0.86 -0.14 0.08
1.48 -0.06 -0.54 -0.21 -0.49 0.13 -0.29
2.89 0.37 0.86 1.98 0.95 -0.09 0.74
1.08 0.04 -0.29 0.92 0.41 1.79 -0.06
3.26 0.07 0.41 -0.06 -0.63 -1.2 0.17
8.53 1.82 2.15 2.17 1.31 0.68 0.59
1.69 0.83 0.58 -1.07 -0.24 -1.42 -0.37
3.94 -0.31 0.67 2.06 0.95 0.1 -2.11
1.16 0.24 0.56 -1.93 -1.04 2.12 0.59
2.62 0.24 0.31 -0.16 1.22 -0.58 0.9
3.11 0.97 0.75 0.63 -0.25 0.1 1.56
-1.34 -1.39 -0.37 -1.18 -0.26 0.87 -1.21
1.85 0.73 -0.75 -1.35 0.08 -1.04 -0.46
-2.82 -1.9 -1.6 -2 -0.85 0.77 1.07
3.38 1.15 -0.58 0.21 0.83 0.37 -0.14
1.31 -0.12 -0.28 -0.33 -2.08 0.81 0.25
1.28 0.97 1.67 -0.2 0.62 -0.21 -0.06
3.74 1.29 1.29 1.83 2.01 0.65 -1.36
-0.25 0.32 -1.14 -0.13 -1.33 -0.79 0.37
-1.75 -1.15 0.48 0.69 1.04 -1.8 1.05
4.16 1.45 2.2 0.19 0.84 -0.51 0.76
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-9 * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(s) => s,
        _ => "",
    }
}

/// The number in `column` of the row of model `model` (rows of a model follow its first row)
/// that has `label` in its second cell, or in its first when `label` is empty.
fn at(table: &Table, model: usize, label: &str, column: &str) -> f64 {
    let at = table
        .column_titles()
        .iter()
        .position(|c| c == column || c.ends_with(&format!("/ {column}")))
        .unwrap_or_else(|| panic!("no column {column} in {:?}", table.column_titles()));
    let mut current = 0;
    for row in &table.rows {
        if let Cell::Number(n) = &row[0] {
            current = *n as usize;
        }
        if current == model && (label.is_empty() || text(&row[1]) == label) {
            return match row[at] {
                Cell::Number(n) => n,
                ref other => panic!("{label} {column}: {other:?}"),
            };
        }
    }
    panic!("no row {label} of model {model}");
}

#[test]
fn stepwise_selection_agrees_with_r() {
    let t = run(
        "REGRESSION /STATISTICS=COEFF OUTS CI R ANOVA CHANGE /CRITERIA=PIN(.05) POUT(.10) \
                 /DEPENDENT=y /METHOD=STEPWISE x1 TO x6.",
    );
    // x1 enters, then x3; no other variable gets in and none leaves
    let entered = table(&t, "Variables Entered/Removed");
    assert_eq!(entered.rows.len(), 2);
    assert_eq!(text(&entered.rows[0][1]), "x1");
    assert_eq!(text(&entered.rows[1][1]), "x3");
    let summary = table(&t, "Model Summary");
    for (model, r, r2, adjusted, se, change, f, df2, p) in [
        (
            1,
            0.660817868558,
            0.436680255406,
            0.426967846017,
            1.67505960565,
            0.436680255406,
            44.9610635108,
            58.0,
            9.14681227058e-09,
        ),
        (
            2,
            0.8155775457,
            0.665166733116,
            0.653418197435,
            1.30269770444,
            0.22848647771,
            38.89616271,
            57.0,
            5.87403468356e-08,
        ),
    ] {
        near(at(summary, model, "", "R"), r, "R");
        near(at(summary, model, "", "R Square"), r2, "R square");
        near(
            at(summary, model, "", "Adjusted R Square"),
            adjusted,
            "adjusted",
        );
        near(
            at(summary, model, "", "Std. Error of the Estimate"),
            se,
            "standard error",
        );
        near(at(summary, model, "", "R Square Change"), change, "change");
        near(at(summary, model, "", "F Change"), f, "F change");
        near(at(summary, model, "", "df2"), df2, "df2");
        near(
            at(summary, model, "", "Sig. F Change"),
            p,
            "p of the change",
        );
    }
    let anova = table(&t, "ANOVA");
    near(at(anova, 2, "Regression", "F"), 56.6169905105, "F");
    let coefficients = table(&t, "Coefficients");
    for (model, label, b, se, lower, upper) in [
        (
            1,
            "(Constant)",
            1.83664884391,
            0.217373041426,
            1.40152942631,
            2.27176826152,
        ),
        (
            1,
            "x1",
            1.5802614529,
            0.235673449644,
            1.1085097882,
            2.0520131176,
        ),
        (
            2,
            "(Constant)",
            1.7365116311,
            0.1698122938,
            1.3964683783,
            2.0765548839,
        ),
        (
            2,
            "x1",
            1.4107766459,
            0.1852874862,
            1.0397448548,
            1.781808437,
        ),
        (
            2,
            "x3",
            0.9183030848,
            0.1472423242,
            0.6234554164,
            1.2131507531,
        ),
    ] {
        near(at(coefficients, model, label, "B"), b, label);
        near(at(coefficients, model, label, "Std. Error"), se, label);
        near(at(coefficients, model, label, "Lower Bound"), lower, label);
        near(at(coefficients, model, label, "Upper Bound"), upper, label);
    }
    // what the first step left out: x2 and x3
    let excluded = table(&t, "Excluded Variables");
    for (label, beta, tvalue, p, partial, tolerance) in [
        (
            "x2",
            0.201916822689,
            1.75346301456,
            0.0848985684504,
            0.226230444233,
            0.707150842237,
        ),
        (
            "x3",
            0.483228232132,
            6.23667882049,
            5.87403468356e-08,
            0.636872892948,
            0.978488900467,
        ),
    ] {
        near(at(excluded, 1, label, "Beta In"), beta, label);
        near(at(excluded, 1, label, "t"), tvalue, label);
        near(at(excluded, 1, label, "Sig."), p, label);
        near(
            at(excluded, 1, label, "Partial Correlation"),
            partial,
            label,
        );
        near(
            at(excluded, 1, label, "Minimum Tolerance"),
            tolerance,
            label,
        );
    }
}

#[test]
fn the_full_model_with_correlations_collinearity_and_durbin_watson_agrees_with_r() {
    let t = run(
        "REGRESSION /STATISTICS=COEFF R ANOVA COLLIN ZPP /RESIDUALS=DURBIN /DEPENDENT=y \
                 /METHOD=ENTER x1 TO x6.",
    );
    let summary = table(&t, "Model Summary");
    near(at(summary, 1, "", "R Square"), 0.695493131492, "R square");
    near(
        at(summary, 1, "", "Adjusted R Square"),
        0.661020655812,
        "adjusted",
    );
    near(
        at(summary, 1, "", "Std. Error of the Estimate"),
        1.28833079345,
        "standard error",
    );
    near(
        at(summary, 1, "", "Durbin-Watson"),
        1.66726562263,
        "Durbin-Watson",
    );
    let anova = table(&t, "ANOVA");
    near(
        at(anova, 1, "Regression", "Sum of Squares"),
        200.921492965,
        "regression SS",
    );
    near(
        at(anova, 1, "Residual", "Sum of Squares"),
        87.9692003679,
        "residual SS",
    );
    near(at(anova, 1, "Regression", "F"), 20.1753171993, "F");
    near(at(anova, 1, "Regression", "Sig."), 4.04459736379e-12, "p");
    let coefficients = table(&t, "Coefficients");
    // B, standard error, Beta, t, p, zero-order, partial, part, VIF
    let expected = [
        (
            "x1",
            [
                1.21560826769,
                0.219916826454,
                0.508330860685,
                5.527581892187,
                1.01058196345e-06,
                0.660817868558,
                0.604715558621,
                0.41898241646,
                1.4719781334,
            ],
        ),
        (
            "x2",
            [
                0.315290659472,
                0.186972535407,
                0.152906861573,
                1.686293972461,
                9.76180259994e-02,
                0.500390803605,
                0.22565586878,
                0.127818553795,
                1.43108737405,
            ],
        ),
        (
            "x3",
            [
                0.94890130943,
                0.169570005498,
                0.499329589357,
                5.595926630092,
                7.89316831869e-07,
                0.569753389505,
                0.60942623773,
                0.424162845081,
                1.38582808281,
            ],
        ),
        (
            "x4",
            [
                -0.144835361143,
                0.186768614833,
                -0.0676205136099,
                -0.775480191215,
                0.441502504666,
                0.19028138351,
                -0.105921169103,
                -0.0587802353306,
                1.32340962623,
            ],
        ),
        (
            "x5",
            [
                0.169082951162,
                0.148721468113,
                0.0873609139714,
                1.136910180536,
                0.260690055669,
                0.131794420914,
                0.154296465036,
                0.0861760864026,
                1.02768685988,
            ],
        ),
        (
            "x6",
            [
                0.00858123370007,
                0.183939346762,
                0.00363137280208,
                0.046652518078,
                0.9629655218,
                -0.0208334236664,
                0.00640808461943,
                0.00353619089495,
                1.05455751244,
            ],
        ),
    ];
    let columns = [
        "B",
        "Std. Error",
        "Beta",
        "t",
        "Sig.",
        "Zero-order",
        "Partial",
        "Part",
        "VIF",
    ];
    for (label, values) in expected {
        for (column, want) in columns.iter().zip(values) {
            near(
                at(coefficients, 1, label, column),
                want,
                &format!("{label} {column}"),
            );
        }
    }
    // eigenvalues of the scaled cross-products, condition indices and the variance
    // proportions of the constant, x1 and x2 on the first three dimensions
    let collinearity = table(&t, "Collinearity Diagnostics");
    let eigenvalues = [
        1.81739826665,
        1.46503022426,
        1.10561796901,
        0.932866037451,
        0.787911285686,
        0.480720240857,
        0.41045597608,
    ];
    let conditions = [
        1.0,
        1.11378602024,
        1.28210180183,
        1.39577499471,
        1.518750361,
        1.944369726,
        2.10422299925,
    ];
    for (i, row) in collinearity.rows.iter().enumerate() {
        let number = |c: usize| match row[c] {
            Cell::Number(n) => n,
            ref other => panic!("{other:?}"),
        };
        near(number(2), eigenvalues[i], "eigenvalue");
        near(number(3), conditions[i], "condition index");
    }
    let proportions = [
        [0.052975403, 0.0757148842, 0.0711572166],
        [0.0523066974, 0.1149080141, 0.1210892282],
        [0.1476271707, 0.0141779763, 0.0000191198],
    ];
    for (i, want) in proportions.iter().enumerate() {
        for (j, want) in want.iter().enumerate() {
            match collinearity.rows[i][4 + j] {
                Cell::Number(n) => {
                    assert!((n - want).abs() < 1e-9, "proportion {i} {j}: {n} vs {want}")
                }
                ref other => panic!("{other:?}"),
            }
        }
    }
}

#[test]
fn blocks_backward_and_forward_selection_agree_with_r() {
    let t = run("REGRESSION /STATISTICS=R CHANGE /DEPENDENT=y /METHOD=ENTER x1 /METHOD=ENTER x2.");
    let summary = table(&t, "Model Summary");
    near(
        at(summary, 2, "", "R Square"),
        0.465511080388,
        "block 2 R square",
    );
    near(
        at(summary, 2, "", "R Square Change"),
        0.0288308250213,
        "change",
    );
    near(at(summary, 2, "", "F Change"), 3.07463254342, "F change");
    near(
        at(summary, 2, "", "Sig. F Change"),
        0.0848985684504,
        "p of the change",
    );
    // x6, x4 and x5 leave one by one; x2 stays at 0.094 (the limit is 0.10)
    let t = run(
        "REGRESSION /STATISTICS=R COEFF /CRITERIA=PIN(.05) POUT(.10) /DEPENDENT=y \
                 /METHOD=BACKWARD x1 TO x6.",
    );
    let entered = table(&t, "Variables Entered/Removed");
    let removed: Vec<&str> = entered.rows.iter().skip(1).map(|r| text(&r[2])).collect();
    assert_eq!(removed, ["x6", "x4", "x5"]);
    let summary = table(&t, "Model Summary");
    for (model, r2, adjusted, se) in [
        (1, 0.695493131492, 0.661020655812, 1.28833079345),
        (2, 0.695480626846, 0.667284388591, 1.27637226692),
        (4, 0.683119023078, 0.666143256457, 1.27855921372),
    ] {
        near(at(summary, model, "", "R Square"), r2, "R square");
        near(
            at(summary, model, "", "Adjusted R Square"),
            adjusted,
            "adjusted",
        );
        near(
            at(summary, model, "", "Std. Error of the Estimate"),
            se,
            "standard error",
        );
    }
    // forward selection stops where stepwise does: x1 and x3
    let t = run(
        "REGRESSION /STATISTICS=R COEFF /CRITERIA=PIN(.05) POUT(.10) /DEPENDENT=y \
                 /METHOD=FORWARD x1 TO x6.",
    );
    let entered = table(&t, "Variables Entered/Removed");
    assert_eq!(entered.rows.len(), 2);
    let summary = table(&t, "Model Summary");
    near(at(summary, 2, "", "R Square"), 0.665166733116, "R square");
}
