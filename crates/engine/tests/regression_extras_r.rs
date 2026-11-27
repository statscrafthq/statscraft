//! REGRESSION options against R's `lm` on 60 invented cases (predictors x1, x2, x3 and x4, a
//! near copy of x1 + x2 / 2 whose tolerance is 2e-4, two dependent variables): the selection
//! criteria of STATISTICS=SELECTION (Akaike n ln(SSE / n) + 2k, Amemiya (1 - R2)(n + k) / (n - k),
//! Mallows SSE / MSE of the full equation + 2k - n, Schwarz n ln(SSE / n) + k ln n, with k the
//! number of coefficients and the constant), SES (the standard error of a standardized
//! coefficient, sqrt((1 - R2) / (df tolerance))), F in place of t, the swept correlation matrix
//! of XTX (R sweeps the correlation matrix of the entered variables and the dependent; the sign
//! convention of the swept row and column is the engine's, as the manual's formula is a picture
//! without a stated sign), the tolerance criterion (TOLERANCE(0.01) keeps x4 out of an ENTER
//! block and lists it in the excluded variables with its beta in, t, partial correlation,
//! tolerance and minimum tolerance), and a list of two dependent variables. Scripts in
//! lab/oracles/regression-r4.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y z x1 x2 x3 x4.
BEGIN DATA.
11.07 9.82 10.63 6.16 22.91 13.68
14.3 10.85 9.34 6.23 25.64 12.45
11.24 8.79 9.53 1.88 21.44 10.48
10.47 7.29 13.09 5.54 15.58 15.82
11.24 8.14 10.66 6.93 20.72 14.04
15.5 9.45 11.24 3.58 27.8 13
6.29 10.93 5.91 5.15 22.76 8.44
8.92 11.41 8.03 5.76 18.8 10.97
11.68 9.12 11.93 6.59 16.65 15.3
6.85 8.82 6.78 6.54 27.66 10.07
16.29 6.32 14.89 5.11 18.9 17.43
17.62 6.2 12.74 1.24 22.44 13.34
14.39 7.34 9.61 1.36 16.99 10.21
11.57 11.58 5.85 2.32 24.05 7.07
7.48 11.52 4.65 5.03 19.42 7.14
13.11 9.39 12.87 4.77 12.93 15.29
14.02 11.24 11.88 4.6 21.55 14.27
7.68 9.84 7.35 4.8 15.69 9.79
5.97 11.68 5.87 5.57 19.46 8.71
7.78 9.81 6.17 2.88 14.48 7.61
7.44 10.71 3.1 3.58 19.14 4.89
12.21 6.74 11.72 1.35 12.37 12.32
13.26 6.66 11.71 3.6 24.23 13.45
11.53 8.03 10.23 5.85 22.1 13.09
12.74 8.73 7.98 6.27 22.11 11.09
9.2 9.67 6.43 4.39 19.8 8.55
10.84 9.71 10.6 6.06 20.82 13.61
16.07 9.31 11.43 6.9 20.21 14.91
9.08 9.89 7.72 7.48 24.4 11.4
17.61 8.13 13.96 3.35 27.27 15.71
8.89 11.57 9.39 6.95 12.7 12.89
17.7 5.17 14.53 4.29 22.48 16.69
14.76 6.86 18.69 8.5 11.86 22.98
14.81 10.6 10.05 4.72 27.02 12.39
5.46 11.14 5.28 6.94 20.37 8.68
7.32 12.04 6.59 3.95 6.61 8.48
15.63 4.4 15.78 2.77 17.7 17.21
13.2 7.26 10.61 3.58 17.89 12.46
10.53 9.68 13.53 4.17 15.04 15.76
14.03 8.87 9.19 1.91 17.12 10.08
8.05 11.68 11.19 8.49 24.83 15.46
17.69 6.5 12.81 4.06 27.77 14.85
14.52 8.91 10.11 4.9 17.68 12.58
16.64 6.02 10.03 0.21 23.76 10.15
9.89 9.42 6.64 6.48 25.44 9.93
6 10.64 6.07 3.92 26.22 8.05
7.37 9.9 5.76 5.18 20.73 8.37
8.16 12.78 5.47 8.05 24.26 9.5
6.1 7.22 7.94 5.8 19.48 10.84
20.72 5.54 16.62 1.42 23.84 17.33
7.58 10.83 5.34 6.78 19.82 8.77
12.67 9.39 9.62 6.77 17.35 12.94
7.91 7.33 9.54 4.84 24.61 11.94
7.33 13.59 12.06 7.94 8.12 15.99
7.92 12.11 7.44 3.29 14.58 9.07
14.79 7.48 15.39 2.3 19.58 16.55
10.4 9.67 7.31 4.29 21.84 9.54
12.35 10.21 15.5 4.33 12.59 17.67
8.08 12.03 9.19 7.69 24.61 13.06
18.99 8.04 14.92 1.41 24.13 15.71
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

/// The row headers joined with " / ", numbers as they print, an empty header taking the one above.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                let shown = match cell {
                    Cell::Text(t) if !t.is_empty() => Some(t.clone()),
                    Cell::Number(n) => Some(format!("{n}")),
                    _ => None,
                };
                if let Some(t) = shown {
                    carried[level] = t;
                    for deeper in carried.iter_mut().skip(level + 1) {
                        deeper.clear();
                    }
                }
            }
            carried
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .collect()
}

/// Each expectation: the table title, which table of that title (counting from the first; one
/// per dependent variable), the row, the column and the value.
fn check(tables: &[Table], expected: &[(&str, usize, &str, &str, f64)]) {
    for (title, nth, row, column, want) in expected {
        let t = tables
            .iter()
            .filter(|t| t.title == *title)
            .nth(*nth)
            .unwrap_or_else(|| panic!("no table {title} number {nth}"));
        let r = row_keys(t)
            .iter()
            .position(|k| k == row)
            .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(t)));
        let c = t
            .column_titles()
            .iter()
            .position(|k| k == column)
            .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", t.column_titles()));
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
fn selection_criteria_standard_errors_of_betas_swept_matrix_and_f_agree_with_r() {
    let t = run(
        "REGRESSION /STATISTICS=COEFF R SELECTION SES XTX F /DEPENDENT=y /METHOD=ENTER x1 /METHOD=ENTER x2 x3.",
    );
    check(
        &t,
        &[
            (
                "Model Summary",
                0,
                "1",
                "Selection Criteria / Akaike Information Criterion",
                115.3616613,
            ),
            (
                "Model Summary",
                0,
                "1",
                "Selection Criteria / Amemiya Prediction Criterion",
                0.4693222232,
            ),
            (
                "Model Summary",
                0,
                "1",
                "Selection Criteria / Mallows' Prediction Criterion",
                45.5236753,
            ),
            (
                "Model Summary",
                0,
                "1",
                "Selection Criteria / Schwarz Bayesian Criterion",
                119.5503505,
            ),
            (
                "Coefficients",
                0,
                "1 / x1",
                "Standardized Coefficients / Std. Error",
                0.0870040992,
            ),
            ("Coefficients", 0, "1 / x1", "F", 74.10539994),
            (
                "Model Summary",
                0,
                "2",
                "Selection Criteria / Akaike Information Criterion",
                83.66524126,
            ),
            (
                "Model Summary",
                0,
                "2",
                "Selection Criteria / Amemiya Prediction Criterion",
                0.2767706705,
            ),
            (
                "Model Summary",
                0,
                "2",
                "Selection Criteria / Mallows' Prediction Criterion",
                4.0,
            ),
            (
                "Model Summary",
                0,
                "2",
                "Selection Criteria / Schwarz Bayesian Criterion",
                92.04261951,
            ),
            (
                "Coefficients",
                0,
                "2 / x1",
                "Standardized Coefficients / Std. Error",
                0.06728228388,
            ),
            ("Coefficients", 0, "2 / x1", "F", 116.251647),
            (
                "Coefficients",
                0,
                "2 / x2",
                "Standardized Coefficients / Std. Error",
                0.06682330797,
            ),
            ("Coefficients", 0, "2 / x2", "F", 25.02183463),
            (
                "Coefficients",
                0,
                "2 / x3",
                "Standardized Coefficients / Std. Error",
                0.06630096181,
            ),
            ("Coefficients", 0, "2 / x3", "F", 18.45022189),
            ("Swept Correlation Matrix", 0, "x1", "x1", 1.046794322),
            ("Swept Correlation Matrix", 0, "x1", "x2", 0.1829667779),
            ("Swept Correlation Matrix", 0, "x1", "x3", 0.1290147115),
            ("Swept Correlation Matrix", 0, "x1", "y", -0.7254379686),
            ("Swept Correlation Matrix", 0, "x2", "x1", 0.1829667779),
            ("Swept Correlation Matrix", 0, "x2", "x2", 1.032561314),
            ("Swept Correlation Matrix", 0, "x2", "x3", 0.04666051504),
            ("Swept Correlation Matrix", 0, "x2", "y", 0.3342624142),
            ("Swept Correlation Matrix", 0, "x3", "x1", 0.1290147115),
            ("Swept Correlation Matrix", 0, "x3", "x2", 0.04666051504),
            ("Swept Correlation Matrix", 0, "x3", "x3", 1.016481702),
            ("Swept Correlation Matrix", 0, "x3", "y", -0.2847873047),
            ("Swept Correlation Matrix", 0, "y", "x1", 0.7254379686),
            ("Swept Correlation Matrix", 0, "y", "x2", -0.3342624142),
            ("Swept Correlation Matrix", 0, "y", "x3", 0.2847873047),
            ("Swept Correlation Matrix", 0, "y", "y", 0.2421743367),
        ],
    );
}

#[test]
fn a_variable_below_the_tolerance_criterion_stays_out_and_is_listed_agree_with_r() {
    let t = run(
        "REGRESSION /CRITERIA=TOLERANCE(0.01) /STATISTICS=COEFF OUTS R TOL /DEPENDENT=y /METHOD=ENTER x1 x2 x3 x4.",
    );
    check(
        &t,
        &[
            ("Model Summary", 0, "1", "R", 0.8705318278),
            ("Model Summary", 0, "1", "R Square", 0.7578256633),
            ("Model Summary", 0, "1", "Adjusted R Square", 0.7448520381),
            (
                "Model Summary",
                0,
                "1",
                "Std. Error of the Estimate",
                1.944568522,
            ),
            (
                "Coefficients",
                0,
                "1 / (Constant)",
                "Unstandardized Coefficients / B",
                1.874128538,
            ),
            (
                "Coefficients",
                0,
                "1 / (Constant)",
                "Unstandardized Coefficients / Std. Error",
                1.59211066,
            ),
            ("Coefficients", 0, "1 / (Constant)", "t", 1.177134595),
            ("Coefficients", 0, "1 / (Constant)", "Sig.", 0.2441207903),
            (
                "Coefficients",
                0,
                "1 / x1",
                "Unstandardized Coefficients / B",
                0.814999599,
            ),
            (
                "Coefficients",
                0,
                "1 / x1",
                "Unstandardized Coefficients / Std. Error",
                0.07558886736,
            ),
            ("Coefficients", 0, "1 / x1", "t", 10.7820057),
            ("Coefficients", 0, "1 / x1", "Sig.", 2.782542493e-15),
            (
                "Coefficients",
                0,
                "1 / x2",
                "Unstandardized Coefficients / B",
                -0.6358466439,
            ),
            (
                "Coefficients",
                0,
                "1 / x2",
                "Unstandardized Coefficients / Std. Error",
                0.1271138312,
            ),
            ("Coefficients", 0, "1 / x2", "t", -5.002182987),
            ("Coefficients", 0, "1 / x2", "Sig.", 5.943297673e-06),
            (
                "Coefficients",
                0,
                "1 / x3",
                "Unstandardized Coefficients / B",
                0.2256912272,
            ),
            (
                "Coefficients",
                0,
                "1 / x3",
                "Unstandardized Coefficients / Std. Error",
                0.0525428809,
            ),
            ("Coefficients", 0, "1 / x3", "t", 4.295372148),
            ("Coefficients", 0, "1 / x3", "Sig.", 7.005902195e-05),
            ("Excluded Variables", 0, "1 / x4", "Beta In", -3.025759605),
            ("Excluded Variables", 0, "1 / x4", "t", -0.6620339148),
            ("Excluded Variables", 0, "1 / x4", "Sig.", 0.510713982),
            (
                "Excluded Variables",
                0,
                "1 / x4",
                "Partial Correlation",
                -0.08891506073,
            ),
            (
                "Excluded Variables",
                0,
                "1 / x4",
                "Collinearity Statistics / Tolerance",
                0.0002091269179,
            ),
            (
                "Excluded Variables",
                0,
                "1 / x4",
                "Collinearity Statistics / Minimum Tolerance",
                0.0002060090692,
            ),
        ],
    );
}

#[test]
fn several_dependent_variables_are_fitted_each_agree_with_r() {
    let t = run("REGRESSION /DEPENDENT=y z /METHOD=ENTER x1 x2.");
    check(
        &t,
        &[
            (
                "Coefficients",
                0,
                "1 / (Constant)",
                "Unstandardized Coefficients / B",
                6.941939161,
            ),
            (
                "Coefficients",
                0,
                "1 / (Constant)",
                "Unstandardized Coefficients / Std. Error",
                1.221754195,
            ),
            ("Coefficients", 0, "1 / (Constant)", "t", 5.681944201),
            ("Coefficients", 0, "1 / (Constant)", "Sig.", 4.743899644e-07),
            (
                "Coefficients",
                0,
                "1 / x1",
                "Unstandardized Coefficients / B",
                0.774391056,
            ),
            (
                "Coefficients",
                0,
                "1 / x1",
                "Unstandardized Coefficients / Std. Error",
                0.08570965462,
            ),
            ("Coefficients", 0, "1 / x1", "t", 9.035050478),
            ("Coefficients", 0, "1 / x1", "Sig.", 1.353349396e-12),
            (
                "Coefficients",
                0,
                "1 / x2",
                "Unstandardized Coefficients / B",
                -0.6607143328,
            ),
            (
                "Coefficients",
                0,
                "1 / x2",
                "Unstandardized Coefficients / Std. Error",
                0.1451234085,
            ),
            ("Coefficients", 0, "1 / x2", "t", -4.552775735),
            ("Coefficients", 0, "1 / x2", "Sig.", 2.829881599e-05),
            ("Model Summary", 0, "1", "R", 0.823429966),
            ("Model Summary", 0, "1", "R Square", 0.6780369089),
            ("Model Summary", 0, "1", "Adjusted R Square", 0.6667399583),
            (
                "Model Summary",
                0,
                "1",
                "Std. Error of the Estimate",
                2.222382567,
            ),
            (
                "Coefficients",
                1,
                "1 / (Constant)",
                "Unstandardized Coefficients / B",
                10.64353726,
            ),
            (
                "Coefficients",
                1,
                "1 / (Constant)",
                "Unstandardized Coefficients / Std. Error",
                0.8158128926,
            ),
            ("Coefficients", 1, "1 / (Constant)", "t", 13.04654211),
            ("Coefficients", 1, "1 / (Constant)", "Sig.", 9.257938249e-19),
            (
                "Coefficients",
                1,
                "1 / x1",
                "Unstandardized Coefficients / B",
                -0.3234189453,
            ),
            (
                "Coefficients",
                1,
                "1 / x1",
                "Unstandardized Coefficients / Std. Error",
                0.05723167686,
            ),
            ("Coefficients", 1, "1 / x1", "t", -5.651047865),
            ("Coefficients", 1, "1 / x1", "Sig.", 5.323054062e-07),
            (
                "Coefficients",
                1,
                "1 / x2",
                "Unstandardized Coefficients / B",
                0.3776287698,
            ),
            (
                "Coefficients",
                1,
                "1 / x2",
                "Unstandardized Coefficients / Std. Error",
                0.09690455593,
            ),
            ("Coefficients", 1, "1 / x2", "t", 3.896914507),
            ("Coefficients", 1, "1 / x2", "Sig.", 0.000258724679),
            ("Model Summary", 1, "1", "R", 0.7050804585),
            ("Model Summary", 1, "1", "R Square", 0.4971384529),
            ("Model Summary", 1, "1", "Adjusted R Square", 0.4794941881),
            (
                "Model Summary",
                1,
                "1",
                "Std. Error of the Estimate",
                1.483971456,
            ),
        ],
    );
}
