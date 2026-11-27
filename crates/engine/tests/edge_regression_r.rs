//! Edge cases of REGRESSION against R (`lm`) on six invented cases: a predictor that is an exact
//! multiple of another (left out, as the tolerance rule does), a constant predictor, a perfect
//! fit, as many cases as coefficients (no residual degrees of freedom, so no standard errors,
//! adjusted R square or F), and a zero weight beside a weight of two (frequency weights, so the
//! cases count as often as their weight). Where R has no value the cell is empty; the t of a
//! perfect fit is infinite and prints empty, with the significance 0. Scripts in
//! lab/oracles/edge-r5.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y x1 x2 x3 k m w (7F8.2).
BEGIN DATA.
3 1 2 5 7 . 1
5 2 4 3 7 . 1
4 3 6 8 7 . 2
8 4 8 1 7 . 1
6 5 10 9 7 . 0
9 6 12 2 7 . 1
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

/// The row headers (the leading columns titled "Model" or nothing) joined with " / ", an empty
/// one carried from the row above.
fn row_keys(table: &Table) -> Vec<String> {
    let count = table
        .column_titles()
        .iter()
        .take_while(|t| t.is_empty() || *t == "Model")
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
fn predictor_that_is_a_multiple_of_another() {
    let t = run("REGRESSION /DEPENDENT=y /METHOD=ENTER x1 x2 x3.");
    check(
        &t,
        &[
            ("Model Summary", "1", "R", 0.994007011523),
            ("Model Summary", "1", "R Square", 0.988049938958),
            ("Model Summary", "1", "Adjusted R Square", 0.980083231596),
            (
                "Model Summary",
                "1",
                "Std. Error of the Estimate",
                0.326935249097,
            ),
            ("ANOVA", "1 / Regression", "df", 2.0),
            ("ANOVA", "1 / Residual", "df", 3.0),
            ("ANOVA", "1 / Total", "df", 5.0),
            ("ANOVA", "1 / Regression", "Sum of Squares", 26.512673362),
            ("ANOVA", "1 / Total", "Sum of Squares", 26.8333333333),
            ("ANOVA", "1 / Residual", "Sum of Squares", 0.320659971306),
            ("ANOVA", "1 / Residual", "Mean Square", 0.106886657102),
            ("ANOVA", "1 / Regression", "F", 124.022371365),
            ("ANOVA", "1 / Regression", "Sig.", 0.00130633687316),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / B",
                3.96700143472,
            ),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / Std. Error",
                0.379874524955,
            ),
            ("Coefficients", "1 / (Constant)", "t", 10.4429256876),
            ("Coefficients", "1 / (Constant)", "Sig.", 0.00187434867212),
            (
                "Coefficients",
                "1 / x1",
                "Unstandardized Coefficients / B",
                1.01578192253,
            ),
            (
                "Coefficients",
                "1 / x1",
                "Standardized Coefficients / Beta",
                0.820317905288,
            ),
            (
                "Coefficients",
                "1 / x1",
                "Unstandardized Coefficients / Std. Error",
                0.078320482433,
            ),
            ("Coefficients", "1 / x1", "t", 12.9695565064),
            ("Coefficients", "1 / x1", "Sig.", 0.000989641506002),
            (
                "Coefficients",
                "1 / x3",
                "Unstandardized Coefficients / B",
                -0.361908177905,
            ),
            (
                "Coefficients",
                "1 / x3",
                "Standardized Coefficients / Beta",
                -0.510223488696,
            ),
            (
                "Coefficients",
                "1 / x3",
                "Unstandardized Coefficients / Std. Error",
                0.0448636924019,
            ),
            ("Coefficients", "1 / x3", "t", -8.06683887415),
            ("Coefficients", "1 / x3", "Sig.", 0.00397962144375),
            ("Excluded Variables", "1 / x2", "Beta In", f64::NAN),
            ("Excluded Variables", "1 / x2", "t", f64::NAN),
            ("Excluded Variables", "1 / x2", "Sig.", f64::NAN),
            (
                "Excluded Variables",
                "1 / x2",
                "Partial Correlation",
                f64::NAN,
            ),
            (
                "Excluded Variables",
                "1 / x2",
                "Collinearity Statistics / Minimum Tolerance",
                f64::NAN,
            ),
        ],
    );
}

#[test]
fn constant_predictor() {
    let t = run("REGRESSION /DEPENDENT=y /METHOD=ENTER k x1.");
    check(
        &t,
        &[
            ("Model Summary", "1", "R", 0.853719873264),
            ("Model Summary", "1", "R Square", 0.728837622005),
            ("Model Summary", "1", "Adjusted R Square", 0.661047027507),
            (
                "Model Summary",
                "1",
                "Std. Error of the Estimate",
                1.34872073427,
            ),
            ("ANOVA", "1 / Regression", "df", 1.0),
            ("ANOVA", "1 / Residual", "df", 4.0),
            ("ANOVA", "1 / Total", "df", 5.0),
            ("ANOVA", "1 / Regression", "Sum of Squares", 19.5571428571),
            ("ANOVA", "1 / Total", "Sum of Squares", 26.8333333333),
            ("ANOVA", "1 / Residual", "Sum of Squares", 7.27619047619),
            ("ANOVA", "1 / Residual", "Mean Square", 1.81904761905),
            ("ANOVA", "1 / Regression", "F", 10.7513089005),
            ("ANOVA", "1 / Regression", "Sig.", 0.0305317712486),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / B",
                2.13333333333,
            ),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / Std. Error",
                1.25559067236,
            ),
            ("Coefficients", "1 / (Constant)", "t", 1.6990675228),
            ("Coefficients", "1 / (Constant)", "Sig.", 0.164534642572),
            (
                "Coefficients",
                "1 / x1",
                "Unstandardized Coefficients / B",
                1.05714285714,
            ),
            (
                "Coefficients",
                "1 / x1",
                "Standardized Coefficients / Beta",
                0.853719873264,
            ),
            (
                "Coefficients",
                "1 / x1",
                "Unstandardized Coefficients / Std. Error",
                0.32240592152,
            ),
            ("Coefficients", "1 / x1", "t", 3.27891886153),
            ("Coefficients", "1 / x1", "Sig.", 0.0305317712486),
            ("Excluded Variables", "1 / k", "Beta In", f64::NAN),
            ("Excluded Variables", "1 / k", "t", f64::NAN),
            ("Excluded Variables", "1 / k", "Sig.", f64::NAN),
            (
                "Excluded Variables",
                "1 / k",
                "Partial Correlation",
                f64::NAN,
            ),
            (
                "Excluded Variables",
                "1 / k",
                "Collinearity Statistics / Minimum Tolerance",
                f64::NAN,
            ),
        ],
    );
}

#[test]
fn perfect_fit() {
    let t = run("COMPUTE yp=2*x1+1.\nREGRESSION /DEPENDENT=yp /METHOD=ENTER x1.");
    check(
        &t,
        &[
            ("Model Summary", "1", "R", 1.0),
            ("Model Summary", "1", "R Square", 1.0),
            ("Model Summary", "1", "Adjusted R Square", 1.0),
            ("Model Summary", "1", "Std. Error of the Estimate", 0.0),
            ("ANOVA", "1 / Regression", "df", 1.0),
            ("ANOVA", "1 / Residual", "df", 4.0),
            ("ANOVA", "1 / Total", "df", 5.0),
            ("ANOVA", "1 / Regression", "Sum of Squares", 70.0),
            ("ANOVA", "1 / Total", "Sum of Squares", 70.0),
            ("ANOVA", "1 / Residual", "Sum of Squares", 0.0),
            ("ANOVA", "1 / Residual", "Mean Square", 0.0),
            ("ANOVA", "1 / Regression", "F", f64::NAN),
            ("ANOVA", "1 / Regression", "Sig.", 0.0),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / B",
                1.0,
            ),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / Std. Error",
                0.0,
            ),
            ("Coefficients", "1 / (Constant)", "t", f64::NAN),
            ("Coefficients", "1 / (Constant)", "Sig.", 0.0),
            (
                "Coefficients",
                "1 / x1",
                "Unstandardized Coefficients / B",
                2.0,
            ),
            (
                "Coefficients",
                "1 / x1",
                "Standardized Coefficients / Beta",
                1.0,
            ),
            (
                "Coefficients",
                "1 / x1",
                "Unstandardized Coefficients / Std. Error",
                0.0,
            ),
            ("Coefficients", "1 / x1", "t", f64::NAN),
            ("Coefficients", "1 / x1", "Sig.", 0.0),
        ],
    );
}

#[test]
fn as_many_cases_as_coefficients() {
    let t =
        run("TEMPORARY.\nSELECT IF ($CASENUM <= 3).\nREGRESSION /DEPENDENT=y /METHOD=ENTER x1 x3.");
    check(
        &t,
        &[
            ("Model Summary", "1", "R", 1.0),
            ("Model Summary", "1", "R Square", 1.0),
            ("Model Summary", "1", "Adjusted R Square", f64::NAN),
            ("Model Summary", "1", "Std. Error of the Estimate", f64::NAN),
            ("ANOVA", "1 / Regression", "df", 2.0),
            ("ANOVA", "1 / Residual", "df", 0.0),
            ("ANOVA", "1 / Total", "df", 2.0),
            ("ANOVA", "1 / Regression", "Sum of Squares", 2.0),
            ("ANOVA", "1 / Total", "Sum of Squares", 2.0),
            ("ANOVA", "1 / Regression", "F", f64::NAN),
            ("ANOVA", "1 / Regression", "Sig.", f64::NAN),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / B",
                4.0,
            ),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / Std. Error",
                f64::NAN,
            ),
            ("Coefficients", "1 / (Constant)", "t", f64::NAN),
            ("Coefficients", "1 / (Constant)", "Sig.", f64::NAN),
            (
                "Coefficients",
                "1 / x1",
                "Unstandardized Coefficients / B",
                1.14285714286,
            ),
            (
                "Coefficients",
                "1 / x1",
                "Standardized Coefficients / Beta",
                1.14285714286,
            ),
            (
                "Coefficients",
                "1 / x1",
                "Unstandardized Coefficients / Std. Error",
                f64::NAN,
            ),
            ("Coefficients", "1 / x1", "t", f64::NAN),
            ("Coefficients", "1 / x1", "Sig.", f64::NAN),
            (
                "Coefficients",
                "1 / x3",
                "Unstandardized Coefficients / B",
                -0.428571428571,
            ),
            (
                "Coefficients",
                "1 / x3",
                "Standardized Coefficients / Beta",
                -1.07854777647,
            ),
            (
                "Coefficients",
                "1 / x3",
                "Unstandardized Coefficients / Std. Error",
                f64::NAN,
            ),
            ("Coefficients", "1 / x3", "t", f64::NAN),
            ("Coefficients", "1 / x3", "Sig.", f64::NAN),
        ],
    );
}

#[test]
fn zero_weight_and_weight_of_two() {
    let t = run("WEIGHT BY w.\nREGRESSION /DEPENDENT=y /METHOD=ENTER x1 x3.");
    check(
        &t,
        &[
            ("Model Summary", "1", "R", 0.996717844271),
            ("Model Summary", "1", "R Square", 0.993446461089),
            ("Model Summary", "1", "Adjusted R Square", 0.989077435148),
            (
                "Model Summary",
                "1",
                "Std. Error of the Estimate",
                0.253856519761,
            ),
            ("ANOVA", "1 / Regression", "df", 2.0),
            ("ANOVA", "1 / Residual", "df", 3.0),
            ("ANOVA", "1 / Total", "df", 5.0),
            ("ANOVA", "1 / Regression", "Sum of Squares", 29.3066706021),
            ("ANOVA", "1 / Total", "Sum of Squares", 29.5),
            ("ANOVA", "1 / Residual", "Sum of Squares", 0.193329397875),
            ("ANOVA", "1 / Residual", "Mean Square", 0.064443132625),
            ("ANOVA", "1 / Regression", "F", 227.383969466),
            ("ANOVA", "1 / Regression", "Sig.", 0.000530534735458),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / B",
                4.27863046045,
            ),
            (
                "Coefficients",
                "1 / (Constant)",
                "Unstandardized Coefficients / Std. Error",
                0.362277336701,
            ),
            ("Coefficients", "1 / (Constant)", "t", 11.8103729574),
            ("Coefficients", "1 / (Constant)", "Sig.", 0.00130491789892),
            (
                "Coefficients",
                "1 / x1",
                "Unstandardized Coefficients / B",
                0.960743801653,
            ),
            (
                "Coefficients",
                "1 / x1",
                "Standardized Coefficients / Beta",
                0.681264817658,
            ),
            (
                "Coefficients",
                "1 / x1",
                "Unstandardized Coefficients / Std. Error",
                0.0720606117175,
            ),
            ("Coefficients", "1 / x1", "t", 13.3324402715),
            ("Coefficients", "1 / x1", "Sig.", 0.000912043159457),
            (
                "Coefficients",
                "1 / x3",
                "Unstandardized Coefficients / B",
                -0.4046635183,
            ),
            (
                "Coefficients",
                "1 / x3",
                "Standardized Coefficients / Beta",
                -0.502560954004,
            ),
            (
                "Coefficients",
                "1 / x3",
                "Unstandardized Coefficients / Std. Error",
                0.0411444843948,
            ),
            ("Coefficients", "1 / x3", "t", -9.83518263147),
            ("Coefficients", "1 / x3", "Sig.", 0.00223456058919),
        ],
    );
}
