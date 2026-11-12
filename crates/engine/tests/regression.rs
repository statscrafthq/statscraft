//! REGRESSION on synthetic data. The expected numbers come from an independent computation in a
//! general numerical library: textbook least squares with the design matrix, the hat matrix for
//! leverage and refits without each case for the deleted residuals, DFBETA, DFFIT and COVRATIO,
//! and F tests between refitted equations for the stepwise paths; not the sweep of the
//! Algorithms manual that the engine uses. PSPP comparisons are in reference_procedures.rs.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn run(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
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

fn run_error(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.0;
        }
    }
    panic!("no error");
}

/// The row headers of every row joined with " / ", an empty header taking the one above it as
/// long as the headers before it stay the same.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                let text = match cell {
                    Cell::Text(t) => t.clone(),
                    Cell::Number(n) => n.to_string(),
                    Cell::Empty => continue,
                };
                carried[level] = text;
                for deeper in carried.iter_mut().skip(level + 1) {
                    deeper.clear();
                }
            }
            let parts: Vec<&str> = carried
                .iter()
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .collect();
            parts.join(" / ")
        })
        .collect()
}

fn cell<'t>(tables: &'t [Table], title: &str, row: &str, column: &str) -> &'t Cell {
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
    &table.rows[r][c]
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0)
}

/// The syntax of each case of EXPECTED and SAVED, run after the data.
const CASES: &[(&str, &str)] = &[
    (
        "enter",
        "REGRESSION /STATISTICS=ALL /DEPENDENT=y /METHOD=ENTER x1 x2 x3 /RESIDUALS=DURBIN \
         /SAVE=PRED RESID ZPRED ZRESID SRESID DRESID SDRESID ADJPRED SEPRED MAHAL COOK LEVER \
         DFBETA SDBETA DFFIT SDFIT COVRATIO MCIN ICIN.",
    ),
    (
        "blocks",
        "REGRESSION /STATISTICS=DEFAULTS CHANGE TOL /DEPENDENT=y /METHOD=ENTER x1 /METHOD=ENTER x2 x3.",
    ),
    (
        "path-stepwise",
        "REGRESSION /DEPENDENT=y /METHOD=STEPWISE x1 x2 x3.",
    ),
    ("path-forward", "REGRESSION /DEP=y /FORWARD x1 TO x3."),
    (
        "path-backward",
        "REGRESSION /DEPENDENT=y /METHOD=BACKWARD x1 x2 x3.",
    ),
    (
        "path-backward-loose",
        "REGRESSION /CRITERIA=POUT(.5) /DEPENDENT=y /METHOD=BACKWARD x1 x2 x3.",
    ),
    (
        "path-stepwise-loose",
        "REGRESSION /CRITERIA=PIN(.5) POUT(.6) /DEPENDENT=y /METHOD=STEPWISE x1 x2 x3.",
    ),
    (
        "weighted",
        "WEIGHT BY w.\nREGRESSION /DEPENDENT=y /METHOD=ENTER x1 x2 x3.",
    ),
    (
        "regwgt",
        "REGRESSION /REGWGT=g /DEPENDENT=y /METHOD=ENTER x1 x2 x3 /SAVE=PRED RESID SEPRED LEVER SRESID.",
    ),
    (
        "origin",
        "REGRESSION /ORIGIN /DEPENDENT=y /METHOD=ENTER x1 x2 /SAVE=PRED LEVER MAHAL COOK.",
    ),
    (
        "listwise",
        "REGRESSION /DEPENDENT=y /METHOD=ENTER x1 x2 x3.",
    ),
    (
        "meansub",
        "REGRESSION /MISSING=MEANSUBSTITUTION /DEPENDENT=y /METHOD=ENTER x1 x2 x3.",
    ),
    (
        "pairwise",
        "REGRESSION /MISSING=PAIRWISE /DEPENDENT=y /METHOD=ENTER x1 x2 x3.",
    ),
    (
        "descriptives",
        "REGRESSION /DESCRIPTIVES=ALL /DEPENDENT=y /METHOD=ENTER x1 x2 x3.",
    ),
];

fn data_of(case: &str) -> &'static str {
    match case {
        "listwise" | "meansub" | "pairwise" => DATA_MISSING,
        _ => DATA,
    }
}

#[test]
fn every_statistic_matches_an_independent_computation() {
    let mut checked = 0;
    for (case, syntax) in CASES {
        let mut engine = Engine::default();
        let tables = run(&mut engine, &format!("{}\n{syntax}", data_of(case)));
        let mut problems = Vec::new();
        for (c, title, row, column, expected) in EXPECTED.iter().filter(|e| e.0 == *case) {
            let actual = number(cell(&tables, title, row, column));
            if !close(actual, *expected) {
                problems.push(format!(
                    "{c}: {title} / {row} / {column}: {actual} is not {expected}"
                ));
            }
            checked += 1;
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
        let data = engine.dataset().unwrap();
        for (_, name, k, expected) in SAVED.iter().filter(|s| s.0 == *case) {
            let variable = data
                .variable(name)
                .unwrap_or_else(|_| panic!("{case}: no {name}"));
            let Values::Numeric(values) = variable.values() else {
                panic!("{name} is not numeric")
            };
            let actual = values.get(*k).unwrap();
            assert!(
                close(actual, *expected),
                "{case}: {name}[{k}]: {actual} is not {expected}"
            );
            checked += 1;
        }
    }
    assert!(checked > 300, "{checked}");
}

const DATA: &str = "DATA LIST FREE /y x1 x2 x3 w g.
BEGIN DATA.
9.1 15.1 13.7 -1.7 1 0.67
3.8 8.6 9.5 0.5 2 1.7
9.1 10.1 10.6 -2.1 3 1.25
10.1 11.2 7.5 -0.7 2 0.8
1.3 7.6 12.1 -1.2 1 1.81
6.5 10 10.3 1.5 1 0.9
6.7 10 9.2 1.8 2 0.86
-2.3 4.7 11.4 -0.3 2 1.06
5.6 13.1 11.5 0.8 2 1.08
10.9 11.8 8 -0.2 3 0.85
5.2 8.1 8.2 0.6 1 0.76
12 9.5 5.2 -0.8 1 1.1
4.3 11.5 12.8 -1.7 3 1.76
2 9.2 8.8 -1.8 3 1.68
6.6 9.3 8.2 0.4 3 1.46
4.5 5.6 9.9 2.2 1 1.24
END DATA.";
const DATA_MISSING: &str = "DATA LIST FREE /y x1 x2 x3 w g.
BEGIN DATA.
9.1 15.1 13.7 -1.7 1 0.67
3.8 8.6 . 0.5 2 1.7
9.1 10.1 10.6 -2.1 3 1.25
10.1 11.2 7.5 -0.7 2 0.8
. 7.6 12.1 -1.2 1 1.81
6.5 10 10.3 1.5 1 0.9
6.7 10 9.2 1.8 2 0.86
-2.3 4.7 11.4 -0.3 2 1.06
5.6 13.1 11.5 0.8 2 1.08
10.9 11.8 8 . 3 0.85
5.2 8.1 8.2 0.6 1 0.76
12 9.5 5.2 -0.8 1 1.1
4.3 11.5 12.8 -1.7 3 1.76
2 9.2 8.8 -1.8 3 1.68
6.6 9.3 8.2 0.4 3 1.46
4.5 5.6 9.9 2.2 1 1.24
END DATA.";

/// (case, table, row, column, value)
const EXPECTED: &[(&str, &str, &str, &str, f64)] = &[
    ("enter", "Model Summary", "1", "R", 0.8594694760043731),
    (
        "enter",
        "Model Summary",
        "1",
        "R Square",
        0.7386877801832316,
    ),
    (
        "enter",
        "Model Summary",
        "1",
        "Adjusted R Square",
        0.6733597252290395,
    ),
    (
        "enter",
        "Model Summary",
        "1",
        "Std. Error of the Estimate",
        2.163140396770354,
    ),
    (
        "enter",
        "Model Summary",
        "1",
        "Change Statistics / R Square Change",
        0.7386877801832316,
    ),
    (
        "enter",
        "Model Summary",
        "1",
        "Change Statistics / F Change",
        11.307359153754046,
    ),
    (
        "enter",
        "Model Summary",
        "1",
        "Change Statistics / Sig. F Change",
        0.0008223518759960406,
    ),
    (
        "enter",
        "Model Summary",
        "1",
        "Selection Criteria / Akaike Information Criterion",
        28.08704054377205,
    ),
    (
        "enter",
        "Model Summary",
        "1",
        "Selection Criteria / Amemiya Prediction Criterion",
        0.4355203663612808,
    ),
    (
        "enter",
        "Model Summary",
        "1",
        "Selection Criteria / Mallows' Prediction Criterion",
        4.0,
    ),
    (
        "enter",
        "Model Summary",
        "1",
        "Selection Criteria / Schwarz Bayesian Criterion",
        31.177395432731174,
    ),
    (
        "enter",
        "ANOVA",
        "1 / Regression",
        "Sum of Squares",
        158.72738348632234,
    ),
    ("enter", "ANOVA", "1 / Regression", "df", 3.0),
    (
        "enter",
        "ANOVA",
        "1 / Regression",
        "Mean Square",
        52.90912782877411,
    ),
    ("enter", "ANOVA", "1 / Regression", "F", 11.307359153754048),
    (
        "enter",
        "ANOVA",
        "1 / Regression",
        "Sig.",
        0.0008223518759960398,
    ),
    (
        "enter",
        "ANOVA",
        "1 / Residual",
        "Sum of Squares",
        56.15011651367766,
    ),
    ("enter", "ANOVA", "1 / Residual", "df", 12.0),
    (
        "enter",
        "ANOVA",
        "1 / Residual",
        "Mean Square",
        4.679176376139805,
    ),
    ("enter", "ANOVA", "1 / Total", "Sum of Squares", 214.8775),
    ("enter", "ANOVA", "1 / Total", "df", 15.0),
    (
        "enter",
        "Coefficients",
        "1 / (Constant)",
        "Unstandardized Coefficients / B",
        5.403268279728634,
    ),
    (
        "enter",
        "Coefficients",
        "1 / (Constant)",
        "Unstandardized Coefficients / Std. Error",
        3.1816816279797777,
    ),
    (
        "enter",
        "Coefficients",
        "1 / (Constant)",
        "t",
        1.6982429141282316,
    ),
    (
        "enter",
        "Coefficients",
        "1 / (Constant)",
        "Sig.",
        0.11521802715765722,
    ),
    (
        "enter",
        "Coefficients",
        "1 / (Constant)",
        "95.0% Confidence Interval for B / Lower Bound",
        -1.5290204712180948,
    ),
    (
        "enter",
        "Coefficients",
        "1 / (Constant)",
        "95.0% Confidence Interval for B / Upper Bound",
        12.335557030675362,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x1",
        "Unstandardized Coefficients / B",
        1.0978567604617804,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x1",
        "Unstandardized Coefficients / Std. Error",
        0.22834817379945943,
    ),
    ("enter", "Coefficients", "1 / x1", "t", 4.807819314666134),
    (
        "enter",
        "Coefficients",
        "1 / x1",
        "Sig.",
        0.000427807846650022,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x1",
        "95.0% Confidence Interval for B / Lower Bound",
        0.6003288297573063,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x1",
        "95.0% Confidence Interval for B / Upper Bound",
        1.5953846911662546,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x1",
        "Standardized Coefficients / Beta",
        0.7541605885284631,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x1",
        "Standardized Coefficients / Std. Error",
        0.1568612585393787,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x1",
        "Correlations / Zero-order",
        0.6242728708954568,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x1",
        "Correlations / Partial",
        0.8113365085483811,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x1",
        "Correlations / Part",
        0.7094754537324464,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x1",
        "Collinearity Statistics / Tolerance",
        0.8850077684816141,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x1",
        "Collinearity Statistics / VIF",
        1.1299335843296328,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x2",
        "Unstandardized Coefficients / B",
        -1.0285742830556672,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x2",
        "Unstandardized Coefficients / Std. Error",
        0.26364724666639044,
    ),
    ("enter", "Coefficients", "1 / x2", "t", -3.9013276112729045),
    (
        "enter",
        "Coefficients",
        "1 / x2",
        "Sig.",
        0.002105013588471662,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x2",
        "95.0% Confidence Interval for B / Lower Bound",
        -1.6030122865978345,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x2",
        "95.0% Confidence Interval for B / Upper Bound",
        -0.4541362795135,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x2",
        "Standardized Coefficients / Beta",
        -0.5958487822247581,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x2",
        "Standardized Coefficients / Std. Error",
        0.15272974781790954,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x2",
        "Correlations / Zero-order",
        -0.45378632784611506,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x2",
        "Correlations / Partial",
        -0.7477656514563296,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x2",
        "Correlations / Part",
        -0.5757071961342963,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x2",
        "Collinearity Statistics / Tolerance",
        0.9335362870256183,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x2",
        "Collinearity Statistics / VIF",
        1.0711956395247846,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x3",
        "Unstandardized Coefficients / B",
        0.10219557036471871,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x3",
        "Unstandardized Coefficients / Std. Error",
        0.4409683557559922,
    ),
    ("enter", "Coefficients", "1 / x3", "t", 0.23175261678247988),
    (
        "enter",
        "Coefficients",
        "1 / x3",
        "Sig.",
        0.8206350396536416,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x3",
        "95.0% Confidence Interval for B / Lower Bound",
        -0.8585919406320193,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x3",
        "95.0% Confidence Interval for B / Upper Bound",
        1.0629830813614567,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x3",
        "Standardized Coefficients / Beta",
        0.03650276606408701,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x3",
        "Standardized Coefficients / Std. Error",
        0.15750746019989087,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x3",
        "Correlations / Zero-order",
        -0.06854950023765556,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x3",
        "Correlations / Partial",
        0.06675200122023602,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x3",
        "Correlations / Part",
        0.034199037481267014,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x3",
        "Collinearity Statistics / Tolerance",
        0.8777608692840363,
    ),
    (
        "enter",
        "Coefficients",
        "1 / x3",
        "Collinearity Statistics / VIF",
        1.1392624517605467,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Correlations / x1",
        "x1",
        1.0,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Covariances / x1",
        "x1",
        0.052142888477548124,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Correlations / x1",
        "x2",
        -0.14135256594394913,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Covariances / x1",
        "x2",
        -0.008509900446819431,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Correlations / x1",
        "x3",
        0.2802376967567845,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Covariances / x1",
        "x3",
        0.028218343960255902,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Correlations / x2",
        "x1",
        -0.14135256594394632,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Covariances / x2",
        "x1",
        -0.008509900446819263,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Correlations / x2",
        "x2",
        1.0,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Covariances / x2",
        "x2",
        0.06950987067476852,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Correlations / x2",
        "x3",
        0.16734829450172584,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Covariances / x2",
        "x3",
        0.019455928259080133,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Correlations / x3",
        "x1",
        0.2802376967567854,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Covariances / x3",
        "x1",
        0.028218343960255993,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Correlations / x3",
        "x2",
        0.16734829450172523,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Covariances / x3",
        "x2",
        0.019455928259080064,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Correlations / x3",
        "x3",
        1.0,
    ),
    (
        "enter",
        "Coefficient Correlations",
        "1 / Covariances / x3",
        "x3",
        0.1944530907781433,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 1",
        "Eigenvalue",
        2.976803855655061,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 1",
        "Condition Index",
        1.0,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 1",
        "Variance Proportions / (Constant)",
        0.003169369575972829,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 1",
        "Variance Proportions / x1",
        0.006103182442822684,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 1",
        "Variance Proportions / x2",
        0.004583389409958525,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 1",
        "Variance Proportions / x3",
        0.006050270881362366,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 2",
        "Eigenvalue",
        0.9608772477071019,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 2",
        "Condition Index",
        1.7601154485841468,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 2",
        "Variance Proportions / (Constant)",
        0.0004702169431824816,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 2",
        "Variance Proportions / x1",
        0.0001186293054857161,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 2",
        "Variance Proportions / x2",
        0.0002744612394133363,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 2",
        "Variance Proportions / x3",
        0.8770034437090959,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 3",
        "Eigenvalue",
        0.04356354576802829,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 3",
        "Condition Index",
        8.266344474963942,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 3",
        "Variance Proportions / (Constant)",
        0.012726853213616695,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 3",
        "Variance Proportions / x1",
        0.7574950774048511,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 3",
        "Variance Proportions / x2",
        0.3717070388765343,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 3",
        "Variance Proportions / x3",
        0.020740910144656014,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 4",
        "Eigenvalue",
        0.018755350869810276,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 4",
        "Condition Index",
        12.598316455910302,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 4",
        "Variance Proportions / (Constant)",
        0.983633560267228,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 4",
        "Variance Proportions / x1",
        0.23628311084684048,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 4",
        "Variance Proportions / x2",
        0.623435110474094,
    ),
    (
        "enter",
        "Collinearity Diagnostics",
        "1 / 4",
        "Variance Proportions / x3",
        0.09620537526488566,
    ),
    (
        "enter",
        "Model Summary",
        "1",
        "Durbin-Watson",
        1.9876311738888286,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Predicted Value",
        "Minimum",
        -1.1932104440450215,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Predicted Value",
        "Maximum",
        10.402564775934303,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Predicted Value",
        "Mean",
        5.962499999999993,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Predicted Value",
        "Std. Deviation",
        3.252971805250548,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Residual",
        "Minimum",
        -4.268144758430648,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Residual",
        "Maximum",
        3.725876537763366,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Residual",
        "Mean",
        7.216449660063518e-15,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Residual",
        "Std. Deviation",
        1.9347715888217514,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Cook's Distance",
        "Minimum",
        0.00037099099163193246,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Cook's Distance",
        "Maximum",
        0.4209134313811591,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Cook's Distance",
        "Mean",
        0.10417023635349426,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Cook's Distance",
        "Std. Deviation",
        0.1372786968949242,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Centered Leverage Value",
        "Minimum",
        0.02185486773801598,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Centered Leverage Value",
        "Maximum",
        0.42072252017080747,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Centered Leverage Value",
        "Mean",
        0.18750000000000044,
    ),
    (
        "enter",
        "Residuals Statistics",
        "Centered Leverage Value",
        "Std. Deviation",
        0.11823693658740893,
    ),
    (
        "blocks",
        "Model Summary",
        "1",
        "Change Statistics / R Square Change",
        0.38971661733605534,
    ),
    (
        "blocks",
        "Model Summary",
        "2",
        "Change Statistics / R Square Change",
        0.3489711628471762,
    ),
    (
        "blocks",
        "Model Summary",
        "2",
        "Change Statistics / F Change",
        8.012740386007374,
    ),
    (
        "blocks",
        "Model Summary",
        "2",
        "Change Statistics / df1",
        2.0,
    ),
    (
        "blocks",
        "Model Summary",
        "2",
        "Change Statistics / df2",
        12.0,
    ),
    (
        "blocks",
        "Model Summary",
        "2",
        "Change Statistics / Sig. F Change",
        0.006162671904119055,
    ),
    (
        "blocks",
        "Excluded Variables",
        "1 / x2",
        "Beta In",
        -0.6017721619151075,
    ),
    (
        "blocks",
        "Excluded Variables",
        "1 / x2",
        "t",
        -4.150380023199715,
    ),
    (
        "blocks",
        "Excluded Variables",
        "1 / x2",
        "Sig.",
        0.0011404128733209639,
    ),
    (
        "blocks",
        "Excluded Variables",
        "1 / x2",
        "Partial Correlation",
        -0.7549183957600455,
    ),
    (
        "blocks",
        "Excluded Variables",
        "1 / x2",
        "Collinearity Statistics / Tolerance",
        0.9604336656332845,
    ),
    (
        "blocks",
        "Excluded Variables",
        "1 / x2",
        "Collinearity Statistics / Minimum Tolerance",
        0.9604336656332841,
    ),
    (
        "blocks",
        "Excluded Variables",
        "1 / x3",
        "Beta In",
        0.13933631890443052,
    ),
    (
        "blocks",
        "Excluded Variables",
        "1 / x3",
        "t",
        0.6200918972179936,
    ),
    (
        "blocks",
        "Excluded Variables",
        "1 / x3",
        "Sig.",
        0.5459139120419761,
    ),
    (
        "blocks",
        "Excluded Variables",
        "1 / x3",
        "Partial Correlation",
        0.16949416582659546,
    ),
    (
        "blocks",
        "Excluded Variables",
        "1 / x3",
        "Collinearity Statistics / Tolerance",
        0.9030512267733525,
    ),
    (
        "blocks",
        "Excluded Variables",
        "1 / x3",
        "Collinearity Statistics / Minimum Tolerance",
        0.9030512267733525,
    ),
    (
        "path-stepwise",
        "Model Summary",
        "1",
        "R Square",
        0.38971661733605534,
    ),
    (
        "path-stepwise",
        "Model Summary",
        "2",
        "R Square",
        0.7375182060185865,
    ),
    (
        "path-forward",
        "Model Summary",
        "1",
        "R Square",
        0.38971661733605534,
    ),
    (
        "path-forward",
        "Model Summary",
        "2",
        "R Square",
        0.7375182060185865,
    ),
    (
        "path-backward",
        "Model Summary",
        "1",
        "R Square",
        0.7386877801832316,
    ),
    (
        "path-backward",
        "Model Summary",
        "2",
        "R Square",
        0.7375182060185865,
    ),
    (
        "path-backward-loose",
        "Model Summary",
        "1",
        "R Square",
        0.7386877801832316,
    ),
    (
        "path-backward-loose",
        "Model Summary",
        "2",
        "R Square",
        0.7375182060185865,
    ),
    (
        "path-stepwise-loose",
        "Model Summary",
        "1",
        "R Square",
        0.38971661733605534,
    ),
    (
        "path-stepwise-loose",
        "Model Summary",
        "2",
        "R Square",
        0.7375182060185865,
    ),
    (
        "weighted",
        "Model Summary",
        "1",
        "R Square",
        0.6859407951698749,
    ),
    ("weighted", "ANOVA", "1 / Residual", "df", 27.0),
    (
        "weighted",
        "Coefficients",
        "1 / (Constant)",
        "Unstandardized Coefficients / B",
        4.411520234282246,
    ),
    (
        "weighted",
        "Coefficients",
        "1 / (Constant)",
        "Unstandardized Coefficients / Std. Error",
        2.5970089414381987,
    ),
    (
        "weighted",
        "Coefficients",
        "1 / x1",
        "Unstandardized Coefficients / B",
        1.148502956798451,
    ),
    (
        "weighted",
        "Coefficients",
        "1 / x1",
        "Unstandardized Coefficients / Std. Error",
        0.1792796609215327,
    ),
    (
        "weighted",
        "Coefficients",
        "1 / x2",
        "Unstandardized Coefficients / B",
        -1.0031377056224358,
    ),
    (
        "weighted",
        "Coefficients",
        "1 / x2",
        "Unstandardized Coefficients / Std. Error",
        0.21071552756721368,
    ),
    (
        "weighted",
        "Coefficients",
        "1 / x3",
        "Unstandardized Coefficients / B",
        0.023485714564897453,
    ),
    (
        "weighted",
        "Coefficients",
        "1 / x3",
        "Unstandardized Coefficients / Std. Error",
        0.32573852802442,
    ),
    (
        "regwgt",
        "Model Summary",
        "1",
        "R Square",
        0.6942469587161695,
    ),
    (
        "regwgt",
        "ANOVA",
        "1 / Residual",
        "Sum of Squares",
        74.13831416172563,
    ),
    (
        "regwgt",
        "Coefficients",
        "1 / (Constant)",
        "Unstandardized Coefficients / B",
        5.16303946751596,
    ),
    (
        "regwgt",
        "Coefficients",
        "1 / (Constant)",
        "Unstandardized Coefficients / Std. Error",
        3.6017317562283773,
    ),
    (
        "regwgt",
        "Coefficients",
        "1 / x1",
        "Unstandardized Coefficients / B",
        1.0675659751248336,
    ),
    (
        "regwgt",
        "Coefficients",
        "1 / x1",
        "Unstandardized Coefficients / Std. Error",
        0.2567145615527284,
    ),
    (
        "regwgt",
        "Coefficients",
        "1 / x2",
        "Unstandardized Coefficients / B",
        -0.988732057161163,
    ),
    (
        "regwgt",
        "Coefficients",
        "1 / x2",
        "Unstandardized Coefficients / Std. Error",
        0.2863266256927884,
    ),
    (
        "regwgt",
        "Coefficients",
        "1 / x3",
        "Unstandardized Coefficients / B",
        0.2351430266742076,
    ),
    (
        "regwgt",
        "Coefficients",
        "1 / x3",
        "Unstandardized Coefficients / Std. Error",
        0.4639040338663481,
    ),
    (
        "origin",
        "Model Summary",
        "1",
        "R Square",
        0.9073760325936233,
    ),
    (
        "origin",
        "Model Summary",
        "1",
        "Adjusted R Square",
        0.8941440372498553,
    ),
    ("origin", "ANOVA", "1 / Residual", "df", 14.0),
    ("origin", "ANOVA", "1 / Total", "df", 16.0),
    (
        "origin",
        "ANOVA",
        "1 / Total",
        "Sum of Squares",
        783.6999999999999,
    ),
    (
        "origin",
        "Coefficients",
        "1 / x1",
        "Unstandardized Coefficients / B",
        1.3014823553071082,
    ),
    (
        "origin",
        "Coefficients",
        "1 / x1",
        "Unstandardized Coefficients / Std. Error",
        0.19482304287984592,
    ),
    (
        "origin",
        "Coefficients",
        "1 / x2",
        "Unstandardized Coefficients / B",
        -0.6993339342853302,
    ),
    (
        "origin",
        "Coefficients",
        "1 / x2",
        "Unstandardized Coefficients / Std. Error",
        0.19482364765271157,
    ),
    (
        "listwise",
        "Model Summary",
        "1",
        "R Square",
        0.6728205874075637,
    ),
    (
        "listwise",
        "Coefficients",
        "1 / (Constant)",
        "Unstandardized Coefficients / B",
        5.504421895224375,
    ),
    (
        "listwise",
        "Coefficients",
        "1 / x1",
        "Unstandardized Coefficients / B",
        1.0626568347475964,
    ),
    (
        "listwise",
        "Coefficients",
        "1 / x2",
        "Unstandardized Coefficients / B",
        -1.000112430024015,
    ),
    (
        "listwise",
        "Coefficients",
        "1 / x3",
        "Unstandardized Coefficients / B",
        0.12536280150184842,
    ),
    (
        "meansub",
        "Model Summary",
        "1",
        "R Square",
        0.60948271396338,
    ),
    (
        "meansub",
        "Coefficients",
        "1 / (Constant)",
        "Unstandardized Coefficients / B",
        5.7674782945487175,
    ),
    (
        "meansub",
        "Coefficients",
        "1 / x1",
        "Unstandardized Coefficients / B",
        0.9280273201836062,
    ),
    (
        "meansub",
        "Coefficients",
        "1 / x2",
        "Unstandardized Coefficients / B",
        -0.8678292933165985,
    ),
    (
        "meansub",
        "Coefficients",
        "1 / x3",
        "Unstandardized Coefficients / B",
        -0.12155328295484491,
    ),
    ("meansub", "ANOVA", "1 / Residual", "df", 12.0),
    (
        "pairwise",
        "Model Summary",
        "1",
        "R Square",
        0.6516304995733453,
    ),
    (
        "pairwise",
        "Coefficients",
        "1 / (Constant)",
        "Unstandardized Coefficients / B",
        5.605880565076658,
    ),
    (
        "pairwise",
        "Coefficients",
        "1 / x1",
        "Unstandardized Coefficients / B",
        0.986058503140499,
    ),
    (
        "pairwise",
        "Coefficients",
        "1 / x1",
        "Standardized Coefficients / Beta",
        0.6928444159115141,
    ),
    (
        "pairwise",
        "Coefficients",
        "1 / x2",
        "Unstandardized Coefficients / B",
        -0.9089743577943831,
    ),
    (
        "pairwise",
        "Coefficients",
        "1 / x2",
        "Standardized Coefficients / Beta",
        -0.5571178244905586,
    ),
    (
        "pairwise",
        "Coefficients",
        "1 / x3",
        "Unstandardized Coefficients / B",
        -0.13528547458430054,
    ),
    (
        "pairwise",
        "Coefficients",
        "1 / x3",
        "Standardized Coefficients / Beta",
        -0.05116030435780797,
    ),
    ("pairwise", "ANOVA", "1 / Residual", "df", 10.0),
    (
        "descriptives",
        "Descriptive Statistics",
        "y",
        "Mean",
        5.9625,
    ),
    (
        "descriptives",
        "Descriptive Statistics",
        "y",
        "Std. Deviation",
        3.784860191165146,
    ),
    (
        "descriptives",
        "Descriptive Statistics",
        "y",
        "Variance",
        14.325166666666666,
    ),
    ("descriptives", "Descriptive Statistics", "y", "N", 16.0),
    (
        "descriptives",
        "Correlations",
        "y / Pearson Correlation",
        "y",
        1.0,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Covariance",
        "y",
        14.32516666666666,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Sum of Squares and Cross-products",
        "y",
        214.8775,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Pearson Correlation",
        "x1",
        0.6242728708954568,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Covariance",
        "x1",
        6.1431666666666676,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Sum of Squares and Cross-products",
        "x1",
        92.1475,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Sig. (1-tailed)",
        "x1",
        0.004871246471207895,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Pearson Correlation",
        "x2",
        -0.45378632784611506,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Covariance",
        "x2",
        -3.7657500000000006,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Sum of Squares and Cross-products",
        "x2",
        -56.48625000000001,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Sig. (1-tailed)",
        "x2",
        0.03873777124650426,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Pearson Correlation",
        "x3",
        -0.06854950023765556,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Covariance",
        "x3",
        -0.3507499999999998,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Sum of Squares and Cross-products",
        "x3",
        -5.261249999999999,
    ),
    (
        "descriptives",
        "Correlations",
        "y / Sig. (1-tailed)",
        "x3",
        0.4004232942303384,
    ),
    (
        "descriptives",
        "Descriptive Statistics",
        "x1",
        "Mean",
        9.7125,
    ),
    (
        "descriptives",
        "Descriptive Statistics",
        "x1",
        "Std. Deviation",
        2.5999679485203915,
    ),
    (
        "descriptives",
        "Descriptive Statistics",
        "x1",
        "Variance",
        6.759833333333334,
    ),
    ("descriptives", "Descriptive Statistics", "x1", "N", 16.0),
    (
        "descriptives",
        "Correlations",
        "x1 / Pearson Correlation",
        "y",
        0.6242728708954568,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Covariance",
        "y",
        6.1431666666666676,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Sum of Squares and Cross-products",
        "y",
        92.1475,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Sig. (1-tailed)",
        "y",
        0.004871246471207895,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Pearson Correlation",
        "x1",
        1.0,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Covariance",
        "x1",
        6.759833333333334,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Sum of Squares and Cross-products",
        "x1",
        101.39750000000001,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Pearson Correlation",
        "x2",
        0.1989128813493885,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Covariance",
        "x2",
        1.1339166666666665,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Sum of Squares and Cross-products",
        "x2",
        17.008749999999996,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Sig. (1-tailed)",
        "x2",
        0.23009340248066357,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Pearson Correlation",
        "x3",
        -0.31136597955885825,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Covariance",
        "x3",
        -1.0944166666666664,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Sum of Squares and Cross-products",
        "x3",
        -16.416249999999998,
    ),
    (
        "descriptives",
        "Correlations",
        "x1 / Sig. (1-tailed)",
        "x3",
        0.12021684649808977,
    ),
    (
        "descriptives",
        "Descriptive Statistics",
        "x2",
        "Mean",
        9.80625,
    ),
    (
        "descriptives",
        "Descriptive Statistics",
        "x2",
        "Std. Deviation",
        2.1925536861538117,
    ),
    (
        "descriptives",
        "Descriptive Statistics",
        "x2",
        "Variance",
        4.807291666666667,
    ),
    ("descriptives", "Descriptive Statistics", "x2", "N", 16.0),
    (
        "descriptives",
        "Correlations",
        "x2 / Pearson Correlation",
        "y",
        -0.45378632784611506,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Covariance",
        "y",
        -3.7657500000000006,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Sum of Squares and Cross-products",
        "y",
        -56.48625000000001,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Sig. (1-tailed)",
        "y",
        0.03873777124650426,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Pearson Correlation",
        "x1",
        0.19891288134938853,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Covariance",
        "x1",
        1.1339166666666665,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Sum of Squares and Cross-products",
        "x1",
        17.008749999999996,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Sig. (1-tailed)",
        "x1",
        0.23009340248066357,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Pearson Correlation",
        "x2",
        0.9999999999999998,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Covariance",
        "x2",
        4.807291666666667,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Sum of Squares and Cross-products",
        "x2",
        72.109375,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Pearson Correlation",
        "x3",
        -0.21778627054600255,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Covariance",
        "x3",
        -0.6455416666666667,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Sum of Squares and Cross-products",
        "x3",
        -9.683125,
    ),
    (
        "descriptives",
        "Correlations",
        "x2 / Sig. (1-tailed)",
        "x3",
        0.2088908880367477,
    ),
    (
        "descriptives",
        "Descriptive Statistics",
        "x3",
        "Mean",
        -0.16874999999999996,
    ),
    (
        "descriptives",
        "Descriptive Statistics",
        "x3",
        "Std. Deviation",
        1.351896815589119,
    ),
    (
        "descriptives",
        "Descriptive Statistics",
        "x3",
        "Variance",
        1.8276250000000003,
    ),
    ("descriptives", "Descriptive Statistics", "x3", "N", 16.0),
    (
        "descriptives",
        "Correlations",
        "x3 / Pearson Correlation",
        "y",
        -0.06854950023765558,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Covariance",
        "y",
        -0.3507499999999998,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Sum of Squares and Cross-products",
        "y",
        -5.261249999999999,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Sig. (1-tailed)",
        "y",
        0.4004232942303384,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Pearson Correlation",
        "x1",
        -0.31136597955885825,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Covariance",
        "x1",
        -1.0944166666666664,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Sum of Squares and Cross-products",
        "x1",
        -16.416249999999998,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Sig. (1-tailed)",
        "x1",
        0.12021684649808977,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Pearson Correlation",
        "x2",
        -0.21778627054600255,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Covariance",
        "x2",
        -0.6455416666666667,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Sum of Squares and Cross-products",
        "x2",
        -9.683125,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Sig. (1-tailed)",
        "x2",
        0.2088908880367477,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Pearson Correlation",
        "x3",
        1.0,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Covariance",
        "x3",
        1.8276250000000003,
    ),
    (
        "descriptives",
        "Correlations",
        "x3 / Sum of Squares and Cross-products",
        "x3",
        27.414375000000003,
    ),
];

/// (case, variable, case index, value)
const SAVED: &[(&str, &str, usize, f64)] = &[
    ("enter", "PRE_1", 0, 7.715705215218858),
    ("enter", "RES_1", 0, 1.384294784781142),
    ("enter", "ZPR_1", 0, 0.5389549372635372),
    ("enter", "ZRE_1", 0, 0.6399468045846416),
    ("enter", "SRE_1", 0, 0.8902092138878999),
    ("enter", "DRE_1", 0, 2.6787057076068503),
    ("enter", "SDR_1", 0, 0.881928789685054),
    ("enter", "ADJ_1", 0, 6.421294292393086),
    ("enter", "SEP_1", 0, 1.5036899284101037),
    ("enter", "MAH_1", 0, 6.310837802562112),
    ("enter", "COO_1", 0, 0.18525407295195523),
    ("enter", "LEV_1", 0, 0.42072252017080747),
    ("enter", "DFF_1", 0, 1.2944109228257714),
    ("enter", "SDF_1", 0, 0.852815947741881),
    ("enter", "COV_1", 0, 2.08528270356024),
    ("enter", "LMCI_1", 0, 4.439446307363258),
    ("enter", "UMCI_1", 0, 10.991964123074457),
    ("enter", "LICI_1", 0, 1.975759472807911),
    ("enter", "UICI_1", 0, 13.455650957629803),
    ("enter", "DFB0_1", 0, -2.07158863707814),
    ("enter", "SDB0_1", 0, -0.6450423587591464),
    ("enter", "DFB1_1", 0, 0.11711384731829089),
    ("enter", "SDB1_1", 0, 0.508103446144985),
    ("enter", "DFB2_1", 0, 0.11164101675954474),
    ("enter", "SDB2_1", 0, 0.4195096716615149),
    ("enter", "DFB3_1", 0, -0.040057962230559546),
    ("enter", "SDB3_1", 0, -0.08999592858103246),
    ("enter", "PRE_1", 5, 5.940814124420141),
    ("enter", "RES_1", 5, 0.5591858755798587),
    ("enter", "ZPR_1", 5, -0.00666648126025846),
    ("enter", "ZRE_1", 5, 0.2585065104487638),
    ("enter", "SRE_1", 5, 0.2881004020423437),
    ("enter", "DRE_1", 5, 0.6945458989903956),
    ("enter", "SDR_1", 5, 0.27679406310691773),
    ("enter", "ADJ_1", 5, 5.805454101009643),
    ("enter", "SEP_1", 5, 0.9549473823719677),
    ("enter", "MAH_1", 5, 1.9858494202607502),
    ("enter", "COO_1", 5, 0.0050229859696265195),
    ("enter", "LEV_1", 5, 0.13238996135071668),
    ("enter", "DFF_1", 5, 0.1353600234104979),
    ("enter", "SDF_1", 5, 0.13618330967450565),
    ("enter", "COV_1", 5, 1.710966500831581),
    ("enter", "LMCI_1", 5, 3.8601625160546007),
    ("enter", "UMCI_1", 5, 8.021465732785682),
    ("enter", "LICI_1", 5, 0.788900323713408),
    ("enter", "UICI_1", 5, 11.092727925126875),
    ("enter", "DFB0_1", 5, -0.12511351978631424),
    ("enter", "SDB0_1", 5, -0.0377798673228717),
    ("enter", "DFB1_1", 5, 0.008591127072808291),
    ("enter", "SDB1_1", 5, 0.036146437979879646),
    ("enter", "DFB2_1", 5, 0.00955034860294246),
    ("enter", "SDB2_1", 5, 0.03480237566084313),
    ("enter", "DFB3_1", 5, 0.050795787905616785),
    ("enter", "SDB3_1", 5, 0.11067083390231468),
    ("enter", "PRE_1", 11, 10.402564775934303),
    ("enter", "RES_1", 11, 1.5974352240656966),
    ("enter", "ZPR_1", 11, 1.364925686957293),
    ("enter", "ZRE_1", 11, 0.7384796781802625),
    ("enter", "SRE_1", 11, 0.9671652197458164),
    ("enter", "DRE_1", 11, 2.7399806769926456),
    ("enter", "SDR_1", 11, 0.96433809186808),
    ("enter", "ADJ_1", 11, 9.260019323007354),
    ("enter", "SEP_1", 11, 1.3968433383386534),
    ("enter", "MAH_1", 11, 5.317354984128865),
    ("enter", "COO_1", 11, 0.16725980236442126),
    ("enter", "LEV_1", 11, 0.3544903322752577),
    ("enter", "DFF_1", 11, 1.1425454529269494),
    ("enter", "SDF_1", 11, 0.8155572199918882),
    ("enter", "COV_1", 11, 1.7558808060121915),
    ("enter", "LMCI_1", 11, 7.359104589332167),
    ("enter", "UMCI_1", 11, 13.44602496253644),
    ("enter", "LICI_1", 11, 4.792239063290634),
    ("enter", "UICI_1", 11, 16.012890488577973),
    ("enter", "DFB0_1", 11, 1.9897479900909651),
    ("enter", "SDB0_1", 11, 0.6235481656721007),
    ("enter", "DFB1_1", 11, 0.006034602221579588),
    ("enter", "SDB1_1", 11, 0.026349947672612447),
    ("enter", "DFB2_1", 11, -0.19362017476166393),
    ("enter", "SDB2_1", 11, -0.7322443326211732),
    ("enter", "DFB3_1", 11, -0.12786706311453777),
    ("enter", "SDB3_1", 11, -0.2891211858699399),
    ("regwgt", "PRE_1", 0, 7.337913363446865),
    ("regwgt", "RES_1", 0, 1.7620866365531347),
    ("regwgt", "SEP_1", 0, 1.7284799663657628),
    ("regwgt", "LEV_1", 0, 0.28869747023676906),
    ("regwgt", "SRE_1", 0, 0.7057645978208722),
    ("regwgt", "PRE_1", 7, -1.1614888090368423),
    ("regwgt", "RES_1", 7, -1.1385111909631576),
    ("regwgt", "SEP_1", 7, 1.4416930253739597),
    ("regwgt", "LEV_1", 7, 0.3007588502994423),
    ("regwgt", "SRE_1", 7, -0.5879242559884563),
    ("origin", "PRE_1", 2, 5.732032085177292),
    ("origin", "LEV_1", 2, 0.06977011414932434),
    ("origin", "MAH_1", 2, 1.1163218263891894),
    ("origin", "COO_1", 2, 0.088196136735291),
];

fn data_engine(data: &str) -> Engine {
    let mut engine = Engine::default();
    run(&mut engine, data);
    engine
}

fn titles(tables: &[Table]) -> Vec<&str> {
    tables.iter().map(|t| t.title.as_str()).collect()
}

#[test]
fn default_statistics_give_the_four_tables_and_charts_warn() {
    let mut engine = data_engine(DATA);
    let tables = run(
        &mut engine,
        "REGRESSION /DEPENDENT=y /METHOD=ENTER x1 /METHOD=ENTER x2 /SCATTERPLOT=(*ZRESID ,*ZPRED) \
         /RESIDUALS HISTOGRAM(ZRESID) NORMPROB(ZRESID).",
    );
    assert_eq!(
        titles(&tables),
        [
            "Variables Entered/Removed",
            "Model Summary",
            "ANOVA",
            "Coefficients",
            "Excluded Variables",
            "Residuals Statistics"
        ]
    );
    assert!(engine.warnings().is_empty(), "{:?}", engine.warnings());
    // the histogram, the P-P plot and the scatterplot
    assert_eq!(engine.take_charts().len(), 3);
    // model 2 holds every variable, so only model 1 lists x2 as excluded
    assert_eq!(row_keys(&tables[4]), ["1 / x2"]);
    let entered = &tables[0];
    assert_eq!(entered.rows[1][1], Cell::Text("x2".into()));
    assert_eq!(entered.rows[1][3], Cell::Text("Enter".into()));
}

#[test]
fn remove_takes_variables_out_in_one_model() {
    let tables = run(
        &mut data_engine(DATA),
        "REGRESSION /VARIABLES=y x1 x2 x3 /DEPENDENT=y /METHOD=ENTER /METHOD=REMOVE x3 x2.",
    );
    let entered = &tables[0];
    assert_eq!(row_keys(entered), ["1", "2"]);
    assert_eq!(entered.rows[1][2], Cell::Text("x2, x3".into()));
    assert_eq!(entered.rows[1][3], Cell::Text("Remove".into()));
    let after = number(cell(&tables, "Model Summary", "2", "R Square"));
    let one = run(
        &mut data_engine(DATA),
        "REGRESSION /DEPENDENT=y /METHOD=ENTER x1.",
    );
    assert!(close(
        after,
        number(cell(&one, "Model Summary", "1", "R Square"))
    ));
    let stepwise = run(
        &mut data_engine(DATA),
        "REGRESSION /CRITERIA=FIN(4) FOUT(3) /DEPENDENT=y /METHOD=STEPWISE x1 x2 x3.",
    );
    assert_eq!(
        stepwise[0].rows[0][3],
        Cell::Text("Stepwise (Criteria: F-to-enter >= 4.000, F-to-remove <= 3.000)".into())
    );
}

#[test]
fn pin_above_pout_moves_pout_with_a_warning() {
    let mut engine = data_engine(DATA);
    run(
        &mut engine,
        "REGRESSION /CRITERIA=PIN(.2) POUT(.1) /DEPENDENT=y /METHOD=STEPWISE x1 x2 x3.",
    );
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("POUT is set to 0.2")),
        "{:?}",
        engine.warnings()
    );
}

#[test]
fn a_variable_that_breaks_the_tolerance_stays_out() {
    let mut engine = data_engine(DATA);
    let tables = run(
        &mut engine,
        "COMPUTE x4 = x1 + x2.\nREGRESSION /DEPENDENT=y /METHOD=ENTER x1 x2 x4.",
    );
    // the constant and two of the three variables
    assert_eq!(row_keys(&tables[3]).len(), 3, "{:?}", row_keys(&tables[3]));
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("not entered") && w.contains("tolerance")),
        "{:?}",
        engine.warnings()
    );
}

#[test]
fn saved_variables_get_names_labels_and_formats() {
    let mut engine = data_engine(DATA);
    run(
        &mut engine,
        "REGRESSION /DEPENDENT=y /METHOD=ENTER x1 x2 /SAVE=PRED RESID(r) DFBETA(dfb) MCIN.\n\
         REGRESSION /DEPENDENT=y /METHOD=ENTER x1 /SAVE=PRED.",
    );
    let data = engine.dataset().unwrap();
    for name in [
        "PRE_1", "r", "dfb0", "dfb1", "dfb2", "LMCI_1", "UMCI_1", "PRE_2",
    ] {
        assert!(data.variable_index(name).is_some(), "{name}");
    }
    let pre = data.variable("PRE_1").unwrap();
    assert_eq!(pre.label(), Some("Unstandardized Predicted Value"));
    assert_eq!(pre.format(), "F11.5");
    assert_eq!(data.variable("dfb1").unwrap().label(), Some("DFBETA x1"));
    assert_eq!(
        data.variable("LMCI_1").unwrap().label(),
        Some("95% Mean Confidence Interval Lower Bound for y")
    );
    let error = run_error(&format!(
        "{DATA}\nREGRESSION /DEPENDENT=y /METHOD=ENTER x1 /SAVE=PRED(x2)."
    ));
    assert_eq!(error, "REGRESSION /SAVE: variable x2 already exists");
}

#[test]
fn every_dependent_gets_its_equation_and_saved_variables() {
    let mut engine = data_engine(DATA);
    let tables = run(
        &mut engine,
        "REGRESSION /DEPENDENT=y x3 /METHOD=ENTER x1 x2 /SAVE=PRED.",
    );
    let summaries: Vec<&Table> = tables
        .iter()
        .filter(|t| t.title == "Model Summary")
        .collect();
    assert_eq!(summaries.len(), 2);
    let caption = summaries[1].caption.as_ref().unwrap();
    assert!(caption.ends_with("Dependent variable: x3."), "{caption}");
    assert!(engine.dataset().unwrap().variable_index("PRE_2").is_some());
}

#[test]
fn casewise_and_outlier_tables_list_the_cases() {
    let tables = run(
        &mut data_engine(DATA),
        "REGRESSION /DEPENDENT=y /METHOD=ENTER x1 x2 x3 /RESIDUALS=OUTLIERS(ZRESID COOK) ID(w) \
         /CASEWISE=PLOT(ZRESID) OUTLIERS(1.5) DEPENDENT PRED COOK.",
    );
    let casewise = tables
        .iter()
        .find(|t| t.title == "Casewise Diagnostics")
        .unwrap();
    assert_eq!(
        casewise.columns,
        [
            "Case Number",
            "w",
            "Std. Residual",
            "y",
            "Predicted Value",
            "Cook's Distance"
        ]
    );
    assert!(!casewise.rows.is_empty());
    for row in &casewise.rows {
        assert!(number(&row[2]).abs() >= 1.5);
    }
    let outliers = tables
        .iter()
        .find(|t| t.title == "Outlier Statistics")
        .unwrap();
    assert_eq!(outliers.rows.len(), 20);
    assert!(number(&outliers.rows[0][4]).abs() >= number(&outliers.rows[1][4]).abs());
    // the largest standardized residual of the casewise table leads the outliers
    let largest = casewise
        .rows
        .iter()
        .map(|r| number(&r[2]).abs())
        .fold(0.0, f64::max);
    assert!(close(number(&outliers.rows[0][4]).abs(), largest));
}

#[test]
fn errors_and_unsupported_methods() {
    assert_eq!(
        run_error(&format!(
            "{DATA}\nSTRING s (A3).\nREGRESSION /DEPENDENT=y /METHOD=ENTER x1 s."
        )),
        "REGRESSION needs numeric variables, but s is a string variable"
    );
    let (_, errors) = parse("REGRESSION /DEPENDENT=y /METHOD=TEST(x1)(x2).");
    assert_eq!(
        errors[0].message,
        "TEST in REGRESSION /METHOD is not supported yet"
    );
    let (_, errors) = parse("REGRESSION /DEPENDENT=y /METHOD=ENTER x1 /SELECT=x2 EQ 1.");
    assert_eq!(
        errors[0].message,
        "subcommand SELECT of REGRESSION is not supported yet"
    );
    let (_, errors) = parse("REGRESSION /DEPENDENT=y /METHOD=ENTER.");
    assert!(
        errors[0].message.contains("needs a variable list"),
        "{:?}",
        errors[0]
    );
}
