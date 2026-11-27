//! Edge cases of LOGISTIC REGRESSION against R (`glm`, binomial) on eight invented cases: a
//! constant predictor entered beside a real one (left out as a linear combination of the
//! constant) and a zero weight beside a weight of two (frequency weights, so the cases count as
//! often as their weight): the omnibus test, the three summary statistics, the equation with
//! B, its standard error, Wald, significance and Exp(B), and the classification table. The
//! iteration stops when the estimates move by less than 0.001, so the values agree with the
//! converged fit of R to about 1e-5. Scripts in lab/oracles/edge-r6.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y x s k m c w (7F8.2).
BEGIN DATA.
0 1 0 7 . 1 1
0 2 0 7 . 1 1
1 3 1 7 . 0 2
0 4 0 7 . 1 1
1 5 1 7 . 0 1
1 6 1 7 . 1 0
1 2.5 0 7 . 1 1
0 3.5 1 7 . 0 1
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
        .take_while(|t| t.is_empty() || ["Step", "Observed"].contains(&t.as_str()))
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
            (n - want).abs() <= 1e-6 + 1e-5 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn constant_predictor_beside_a_real_one() {
    let t = run("LOGISTIC REGRESSION VARIABLES=y /METHOD=ENTER k x.");
    check(
        &t,
        &[
            (
                "Omnibus Tests of Model Coefficients",
                "Step 1 / Model",
                "Chi-square",
                2.17332244135,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                "Step 1 / Model",
                "df",
                1.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                "Step 1 / Model",
                "Sig.",
                0.14042256445,
            ),
            (
                "Model Summary",
                "Step 1",
                "-2 Log likelihood",
                8.91703244761,
            ),
            (
                "Model Summary",
                "Step 1",
                "Cox & Snell R Square",
                0.23789070755,
            ),
            (
                "Model Summary",
                "Step 1",
                "Nagelkerke R Square",
                0.317187610067,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "B",
                -2.72315189311,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "S.E.",
                2.2972204353,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "Wald",
                1.4052008073,
            ),
            ("Variables in the Equation", "Step 1 / Constant", "df", 1.0),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "Sig.",
                0.235854723201,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "Exp(B)",
                0.0656674511122,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "B",
                0.814766452836,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "S.E.",
                0.652822627546,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Wald",
                1.55767152151,
            ),
            ("Variables in the Equation", "Step 1 / x", "df", 1.0),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Sig.",
                0.212006480215,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Exp(B)",
                2.25864810989,
            ),
            ("Classification Table", "Step 1 / 0", "Predicted / 0", 2.0),
            ("Classification Table", "Step 1 / 0", "Predicted / 1", 2.0),
            (
                "Classification Table",
                "Step 1 / 0",
                "Predicted / Percentage Correct",
                50.0,
            ),
            ("Classification Table", "Step 1 / 1", "Predicted / 0", 2.0),
            ("Classification Table", "Step 1 / 1", "Predicted / 1", 2.0),
            (
                "Classification Table",
                "Step 1 / 1",
                "Predicted / Percentage Correct",
                50.0,
            ),
            (
                "Classification Table",
                "Step 1 / Overall Percentage",
                "Predicted / Percentage Correct",
                50.0,
            ),
        ],
    );
}

#[test]
fn zero_weight_and_weight_of_two() {
    let t = run("WEIGHT BY w.
LOGISTIC REGRESSION VARIABLES=y /METHOD=ENTER x.");
    check(
        &t,
        &[
            (
                "Omnibus Tests of Model Coefficients",
                "Step 1 / Model",
                "Chi-square",
                0.899085015655,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                "Step 1 / Model",
                "df",
                1.0,
            ),
            (
                "Omnibus Tests of Model Coefficients",
                "Step 1 / Model",
                "Sig.",
                0.343027170587,
            ),
            (
                "Model Summary",
                "Step 1",
                "-2 Log likelihood",
                10.1912698733,
            ),
            (
                "Model Summary",
                "Step 1",
                "Cox & Snell R Square",
                0.106300443599,
            ),
            (
                "Model Summary",
                "Step 1",
                "Nagelkerke R Square",
                0.141733924798,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "B",
                -1.89124688747,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "S.E.",
                2.27398533161,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "Wald",
                0.691705349945,
            ),
            ("Variables in the Equation", "Step 1 / Constant", "df", 1.0),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "Sig.",
                0.405584933022,
            ),
            (
                "Variables in the Equation",
                "Step 1 / Constant",
                "Exp(B)",
                0.15088355668,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "B",
                0.630415629157,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "S.E.",
                0.715766463656,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Wald",
                0.775731184923,
            ),
            ("Variables in the Equation", "Step 1 / x", "df", 1.0),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Sig.",
                0.378449883061,
            ),
            (
                "Variables in the Equation",
                "Step 1 / x",
                "Exp(B)",
                1.87839113117,
            ),
            ("Classification Table", "Step 1 / 0", "Predicted / 0", 2.0),
            ("Classification Table", "Step 1 / 0", "Predicted / 1", 2.0),
            (
                "Classification Table",
                "Step 1 / 0",
                "Predicted / Percentage Correct",
                50.0,
            ),
            ("Classification Table", "Step 1 / 1", "Predicted / 0", 1.0),
            ("Classification Table", "Step 1 / 1", "Predicted / 1", 3.0),
            (
                "Classification Table",
                "Step 1 / 1",
                "Predicted / Percentage Correct",
                75.0,
            ),
            (
                "Classification Table",
                "Step 1 / Overall Percentage",
                "Predicted / Percentage Correct",
                62.5,
            ),
        ],
    );
}
