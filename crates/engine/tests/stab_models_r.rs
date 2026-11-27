//! Numerical stability of REGRESSION and LOGISTIC REGRESSION against R on 24 invented cases (the
//! data of stab_basic_r) with a dependent and predictors at 1e9, 1e6 and 1e-6: the slopes, the
//! intercept at the size of the offsets, their standard errors, the R square, the ANOVA, and for
//! the logistic fit the equation, -2 log likelihood and omnibus chi-square. R fits the columns
//! shifted by their first value and scaled, then maps the coefficients and their covariance
//! back to the original columns. The regression agrees to 1e-7; the logistic iteration stops
//! when the estimates move by less than 0.001, so it agrees to about 1e-5. A perfect fit at this
//! scale (the dependent an exact linear function of the predictors) has no negative residual sum
//! of squares and no F. Scripts in lab/oracles/stab-r2.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g big big2 tiny off small yb (7F20.12).
BEGIN DATA.
1 1000000005.000000000000 1000000002.250000000000 0.000000006000 1000000.000000000000 0.000004250000 0.000000000000
2 1000000000.750000000000 1000000001.000000000000 0.000000001750 1000000.500000000000 0.000003000000 0.000000000000
3 1000000001.500000000000 1000000005.750000000000 0.000000002500 1000001.000000000000 0.000007750000 0.000000000000
1 1000000008.000000000000 1000000003.250000000000 0.000000009000 1000001.500000000000 0.000005250000 1.000000000000
2 1000000006.750000000000 1000000006.500000000000 0.000000007750 1000002.000000000000 0.000008500000 1.000000000000
3 1000000001.250000000000 1000000008.750000000000 0.000000002250 1000000.000000000000 0.000010750000 1.000000000000
1 1000000009.000000000000 1000000001.750000000000 0.000000010000 1000000.500000000000 0.000003750000 1.000000000000
2 1000000010.000000000000 1000000010.000000000000 0.000000011000 1000001.000000000000 0.000012000000 1.000000000000
3 1000000000.750000000000 1000000009.000000000000 0.000000001750 1000001.500000000000 0.000011000000 1.000000000000
1 1000000000.750000000000 1000000003.500000000000 0.000000001750 1000002.000000000000 0.000005500000 0.000000000000
2 1000000002.000000000000 1000000004.500000000000 0.000000003000 1000000.000000000000 0.000006500000 0.000000000000
3 1000000008.500000000000 1000000001.750000000000 0.000000009500 1000000.500000000000 0.000003750000 1.000000000000
1 1000000008.750000000000 1000000002.750000000000 0.000000009750 1000001.000000000000 0.000004750000 1.000000000000
2 1000000009.000000000000 1000000010.000000000000 0.000000010000 1000001.500000000000 0.000012000000 1.000000000000
3 1000000001.500000000000 1000000008.750000000000 0.000000002500 1000002.000000000000 0.000010750000 1.000000000000
1 1000000009.000000000000 1000000000.750000000000 0.000000010000 1000000.000000000000 0.000002750000 1.000000000000
2 1000000007.750000000000 1000000008.500000000000 0.000000008750 1000000.500000000000 0.000010500000 1.000000000000
3 1000000005.000000000000 1000000007.250000000000 0.000000006000 1000001.000000000000 0.000009250000 1.000000000000
1 1000000007.250000000000 1000000005.750000000000 0.000000008250 1000001.500000000000 0.000007750000 1.000000000000
2 1000000002.750000000000 1000000003.750000000000 0.000000003750 1000002.000000000000 0.000005750000 0.000000000000
3 1000000004.750000000000 1000000008.250000000000 0.000000005750 1000000.000000000000 0.000010250000 1.000000000000
1 1000000005.250000000000 1000000007.000000000000 0.000000006250 1000000.500000000000 0.000009000000 1.000000000000
2 1000000001.000000000000 1000000001.750000000000 0.000000002000 1000001.000000000000 0.000003750000 0.000000000000
3 1000000002.500000000000 1000000005.250000000000 0.000000003500 1000001.500000000000 0.000007250000 0.000000000000
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
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

/// The row headers (the leading columns titled "Step", "Observed" or nothing) joined with " / ", an empty
/// one carried from the row above.
fn row_keys(table: &Table) -> Vec<String> {
    let count = table
        .column_titles()
        .iter()
        .take_while(|t| t.is_empty() || ["Step", "Observed", "Model"].contains(&t.as_str()))
        .count();
    let mut carried = vec![String::new(); count];
    table
        .rows
        .iter()
        .map(|row| {
            for (slot, cell) in carried.iter_mut().zip(row) {
                let text = match cell {
                    Cell::Text(t) => t.clone(),
                    Cell::Number(n) => format!("{n}"),
                    Cell::Empty => String::new(),
                };
                if !text.is_empty() {
                    *slot = text;
                }
            }
            carried.join(" / ")
        })
        .collect()
}

/// Each expectation: the table title, the row, the column and the value (NaN: an empty cell).
fn check(tables: &[Table], expected: &[(&str, &str, &str, f64)], rel: f64) {
    for (title, row, column, want) in expected {
        // the tables of step 0 come first: the last one of a title is the fitted step
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
            (n - want).abs() <= rel * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn regression_with_offsets_of_1e9_and_1e6() {
    let t = run("REGRESSION /DEPENDENT=big2 /METHOD=ENTER big off.");
    check(
        &t,
        &[
            ("Model Summary", "1", "R", 0.202188682708976),
            ("Model Summary", "1", "R Square", 0.040880263415591),
            (
                "Model Summary",
                "1",
                "Adjusted R Square",
                -0.0504644734019717,
            ),
            (
                "Model Summary",
                "1",
                "Std. Error of the Estimate",
                3.11486834800142,
            ),
            (
                "ANOVA",
                "1 / Regression",
                "Sum of Squares",
                8.68439450033031,
            ),
            ("ANOVA", "1 / Residual", "Sum of Squares", 203.750501333003),
            ("ANOVA", "1 / Total", "Sum of Squares", 212.434895833333),
            ("ANOVA", "1 / Regression", "F", 0.447538247301962),
            ("ANOVA", "1 / Regression", "Sig.", 0.645155929416167),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / B",
                985258352.651144,
            ),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / Std. Error",
                197670681.542059,
            ),
            ("Coefficients", "1 / (Constant)", "t", 4.98434236663219),
            (
                "Coefficients",
                "1 / (Constant)",
                "Sig.",
                6.21956672051841e-05,
            ),
            (
                "Coefficients",
                "1 / big",
                "Unstandardized Coefficients / B",
                0.0138647855471665,
            ),
            (
                "Coefficients",
                "1 / big",
                "Unstandardized Coefficients / Std. Error",
                0.19754480371535,
            ),
            ("Coefficients", "1 / big", "t", 0.0701855239237011),
            ("Coefficients", "1 / big", "Sig.", 0.944710074659524),
            (
                "Coefficients",
                "1 / big",
                "Standardized Coefficients / Beta",
                0.0151344106500488,
            ),
            (
                "Coefficients",
                "1 / off",
                "Unstandardized Coefficients / B",
                0.876866215674543,
            ),
            (
                "Coefficients",
                "1 / off",
                "Unstandardized Coefficients / Std. Error",
                0.928474906036569,
            ),
            ("Coefficients", "1 / off", "t", 0.944415632531922),
            ("Coefficients", "1 / off", "Sig.", 0.355697325278418),
            (
                "Coefficients",
                "1 / off",
                "Standardized Coefficients / Beta",
                0.203648462076051,
            ),
        ],
        1e-7,
    );
}

#[test]
fn regression_with_predictors_at_1e_minus_6_and_1e6() {
    let t = run("REGRESSION /DEPENDENT=tiny /METHOD=ENTER small off.");
    check(
        &t,
        &[
            ("Model Summary", "1", "R", 0.134128268138819),
            ("Model Summary", "1", "R Square", 0.017990392313919),
            (
                "Model Summary",
                "1",
                "Adjusted R Square",
                -0.0755343322276125,
            ),
            (
                "Model Summary",
                "1",
                "Std. Error of the Estimate",
                3.44043618932287e-09,
            ),
            (
                "ANOVA",
                "1 / Regression",
                "Sum of Squares",
                4.55377120448076e-18,
            ),
            (
                "ANOVA",
                "1 / Residual",
                "Sum of Squares",
                2.48568624628853e-16,
            ),
            ("ANOVA", "1 / Total", "Sum of Squares", 2.53122395833333e-16),
            ("ANOVA", "1 / Regression", "F", 0.192359746602942),
            ("ANOVA", "1 / Regression", "Sig.", 0.826447435832667),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / B",
                0.000641054990741877,
            ),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / Std. Error",
                0.00103768539412712,
            ),
            ("Coefficients", "1 / (Constant)", "t", 0.617773936464739),
            ("Coefficients", "1 / (Constant)", "Sig.", 0.543366346753672),
            (
                "Coefficients",
                "1 / small",
                "Unstandardized Coefficients / B",
                1.69145629173229e-05,
            ),
            (
                "Coefficients",
                "1 / small",
                "Unstandardized Coefficients / Std. Error",
                0.000240997886340648,
            ),
            ("Coefficients", "1 / small", "t", 0.070185523923701),
            ("Coefficients", "1 / small", "Sig.", 0.944710074659524),
            (
                "Coefficients",
                "1 / small",
                "Standardized Coefficients / Beta",
                0.0154956009120832,
            ),
            (
                "Coefficients",
                "1 / off",
                "Unstandardized Coefficients / B",
                -6.41048552350949e-10,
            ),
            (
                "Coefficients",
                "1 / off",
                "Unstandardized Coefficients / Std. Error",
                1.03768475551841e-09,
            ),
            ("Coefficients", "1 / off", "t", -0.617768112080139),
            ("Coefficients", "1 / off", "Sig.", 0.543370113642027),
            (
                "Coefficients",
                "1 / off",
                "Standardized Coefficients / Beta",
                -0.136391204137928,
            ),
        ],
        1e-7,
    );
}

#[test]
fn logistic_with_a_predictor_at_1e6() {
    let t = run("LOGISTIC REGRESSION VARIABLES=yb /METHOD=ENTER off.");
    check(
        &t,
        &[
            (
                "Omnibus Tests of Model Coefficients",
                "Step 1 / Model",
                "Chi-square",
                0.0436396244009245,
            ),
            (
                "Model Summary",
                "Step 1",
                "-2 Log likelihood",
                30.5090404537501,
            ),
            (
                "Variables in the Equation",
                "Step 1 / off",
                "B",
                -0.130964761817868,
            ),
            (
                "Variables in the Equation",
                "Step 1 / off",
                "S.E.",
                0.627338709787528,
            ),
            (
                "Variables in the Equation",
                "Step 1 / off",
                "Wald",
                0.0435817577232375,
            ),
            (
                "Variables in the Equation",
                "Step 1 / off",
                "Sig.",
                0.834633691228743,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "B",
                130965.581834945,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "S.E.",
                627339.324019635,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "Wald",
                0.0435822181433946,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "Sig.",
                0.834632830339724,
            ),
        ],
        1e-5,
    );
}

#[test]
fn logistic_with_a_predictor_at_1e9() {
    let t = run("LOGISTIC REGRESSION VARIABLES=yb /METHOD=ENTER big.");
    check(
        &t,
        &[
            (
                "Omnibus Tests of Model Coefficients",
                "Step 1 / Model",
                "Chi-square",
                11.5153320866666,
            ),
            (
                "Model Summary",
                "Step 1",
                "-2 Log likelihood",
                19.0373479914844,
            ),
            (
                "Variables in the Equation",
                "Step 1 / big",
                "B",
                0.616232376353264,
            ),
            (
                "Variables in the Equation",
                "Step 1 / big",
                "S.E.",
                0.248378131643476,
            ),
            (
                "Variables in the Equation",
                "Step 1 / big",
                "Wald",
                6.15548549598044,
            ),
            (
                "Variables in the Equation",
                "Step 1 / big",
                "Sig.",
                0.0131005151544054,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "B",
                -616232378.063915,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "S.E.",
                248378132.385441,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "Wald",
                6.15548549337967,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "Sig.",
                0.0131005151736689,
            ),
        ],
        1e-5,
    );
}

#[test]
fn logistic_with_a_predictor_at_1e_minus_6() {
    let t = run("LOGISTIC REGRESSION VARIABLES=yb /METHOD=ENTER small.");
    check(
        &t,
        &[
            (
                "Omnibus Tests of Model Coefficients",
                "Step 1 / Model",
                "Chi-square",
                5.03641546751972,
            ),
            (
                "Model Summary",
                "Step 1",
                "-2 Log likelihood",
                25.5162646106313,
            ),
            (
                "Variables in the Equation",
                "Step 1 / small",
                "B",
                368774.11479943,
            ),
            (
                "Variables in the Equation",
                "Step 1 / small",
                "S.E.",
                185043.740069767,
            ),
            (
                "Variables in the Equation",
                "Step 1 / small",
                "Wald",
                3.97165999982233,
            ),
            (
                "Variables in the Equation",
                "Step 1 / small",
                "Sig.",
                0.0462721348245378,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "B",
                -1.811656936368,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "S.E.",
                1.26156474226771,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "Wald",
                2.06220975213697,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "Sig.",
                0.150991071407245,
            ),
        ],
        1e-5,
    );
}

#[test]
fn perfect_fit_at_this_scale_has_no_negative_residual_and_no_f() {
    // big2 = 1000000000 - 2 + 1000000 * small exactly: rounding must not leave the residual
    // sum of squares below 0 (R's own residuals are 1e-8 here, a perfect fit all the same)
    let t = run("REGRESSION /DEPENDENT=big2 /METHOD=ENTER big off small.");
    let anova = t.iter().find(|t| t.title == "ANOVA").unwrap();
    let keys = row_keys(anova);
    let residual = keys.iter().position(|k| k == "1 / Residual").unwrap();
    let regression = keys.iter().position(|k| k == "1 / Regression").unwrap();
    let columns = anova.column_titles();
    let sum_of_squares = columns.iter().position(|c| c == "Sum of Squares").unwrap();
    let f = columns.iter().position(|c| c == "F").unwrap();
    let Cell::Number(ss) = anova.rows[residual][sum_of_squares] else {
        panic!("{:?}", anova.rows[residual]);
    };
    assert!((0.0..1e-9).contains(&ss), "{ss}");
    assert_eq!(anova.rows[regression][f], Cell::Empty);
}
