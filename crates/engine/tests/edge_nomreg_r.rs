//! Edge cases and numerical stability of NOMREG against R (`nnet::multinom`, the covariance from
//! its Hessian) on fifteen invented cases: a constant covariate entered beside a real one (left
//! out, its parameter 0), a zero weight and a weight of two (cases repeated as often as their
//! weight), and the covariate with 1000000 added: the slope, the fit and the pseudo R squares
//! are those of the unshifted covariate and the intercept moves by the slope times 1000000.
//! R fits the covariate shifted by its first value and maps the coefficients and their
//! covariance back. The iteration stops by the log-likelihood criterion, so the estimates agree
//! to about 1e-5. Scripts in lab/oracles/edge-nm.

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
    let t = run("NOMREG y (BASE=FIRST) WITH x /PRINT=MFI PARAMETER SUMMARY.");
    check(
        &t,
        &[
            (
                "Model Fitting Information",
                "Intercept Only",
                "Model Fitting Criteria / -2 Log Likelihood",
                32.555658389,
            ),
            (
                "Model Fitting Information",
                "Final",
                "Model Fitting Criteria / -2 Log Likelihood",
                28.5714068205,
            ),
            (
                "Model Fitting Information",
                "Final",
                "Likelihood Ratio Tests / Chi-Square",
                3.98425156848,
            ),
            ("Pseudo R-Square", "Cox and Snell", "Value", 0.233267094688),
            ("Pseudo R-Square", "Nagelkerke", "Value", 0.263321135071),
            ("Pseudo R-Square", "McFadden", "Value", 0.122382767409),
            ("Parameter Estimates", "2 / Intercept", "B", -1.47294241835),
            (
                "Parameter Estimates",
                "2 / Intercept",
                "Std. Error",
                2.24638885034,
            ),
            (
                "Parameter Estimates",
                "2 / Intercept",
                "Wald",
                0.429933878792,
            ),
            (
                "Parameter Estimates",
                "2 / Intercept",
                "Sig.",
                0.512021394692,
            ),
            ("Parameter Estimates", "2 / x", "B", 0.44442227742),
            ("Parameter Estimates", "2 / x", "Std. Error", 0.740887712077),
            ("Parameter Estimates", "2 / x", "Wald", 0.359821290928),
            ("Parameter Estimates", "2 / x", "Sig.", 0.548605502688),
            ("Parameter Estimates", "2 / x", "Exp(B)", 1.55958892578),
            ("Parameter Estimates", "3 / Intercept", "B", -6.03980535555),
            (
                "Parameter Estimates",
                "3 / Intercept",
                "Std. Error",
                3.60424001134,
            ),
            (
                "Parameter Estimates",
                "3 / Intercept",
                "Wald",
                2.80813820807,
            ),
            (
                "Parameter Estimates",
                "3 / Intercept",
                "Sig.",
                0.0937871630715,
            ),
            ("Parameter Estimates", "3 / x", "B", 1.69436010052),
            ("Parameter Estimates", "3 / x", "Std. Error", 1.02241079775),
            ("Parameter Estimates", "3 / x", "Wald", 2.74637967885),
            ("Parameter Estimates", "3 / x", "Sig.", 0.0974749099027),
            ("Parameter Estimates", "3 / x", "Exp(B)", 5.44316177419),
        ],
    );
}

#[test]
fn zero_weight_and_weight_of_two() {
    let t = run("WEIGHT BY w.\nNOMREG y (BASE=FIRST) WITH x /PRINT=MFI PARAMETER SUMMARY.");
    check(
        &t,
        &[
            (
                "Model Fitting Information",
                "Intercept Only",
                "Model Fitting Criteria / -2 Log Likelihood",
                34.2953630116,
            ),
            (
                "Model Fitting Information",
                "Final",
                "Model Fitting Criteria / -2 Log Likelihood",
                29.8016139727,
            ),
            (
                "Model Fitting Information",
                "Final",
                "Likelihood Ratio Tests / Chi-Square",
                4.49374903885,
            ),
            ("Pseudo R-Square", "Cox and Snell", "Value", 0.244865435832),
            ("Pseudo R-Square", "Nagelkerke", "Value", 0.277388844779),
            ("Pseudo R-Square", "McFadden", "Value", 0.131030805457),
            ("Parameter Estimates", "2 / Intercept", "B", -2.06911494619),
            (
                "Parameter Estimates",
                "2 / Intercept",
                "Std. Error",
                2.22632861269,
            ),
            (
                "Parameter Estimates",
                "2 / Intercept",
                "Wald",
                0.863755249653,
            ),
            (
                "Parameter Estimates",
                "2 / Intercept",
                "Sig.",
                0.352689941037,
            ),
            ("Parameter Estimates", "2 / x", "B", 0.606819473688),
            ("Parameter Estimates", "2 / x", "Std. Error", 0.743861071384),
            ("Parameter Estimates", "2 / x", "Wald", 0.665480533849),
            ("Parameter Estimates", "2 / x", "Sig.", 0.414631750264),
            ("Parameter Estimates", "2 / x", "Exp(B)", 1.8345871571),
            ("Parameter Estimates", "3 / Intercept", "B", -6.30309096341),
            (
                "Parameter Estimates",
                "3 / Intercept",
                "Std. Error",
                3.49352581065,
            ),
            (
                "Parameter Estimates",
                "3 / Intercept",
                "Wald",
                3.25521169375,
            ),
            (
                "Parameter Estimates",
                "3 / Intercept",
                "Sig.",
                0.071196743342,
            ),
            ("Parameter Estimates", "3 / x", "B", 1.771295135),
            ("Parameter Estimates", "3 / x", "Std. Error", 1.00918914715),
            ("Parameter Estimates", "3 / x", "Wald", 3.08060997059),
            ("Parameter Estimates", "3 / x", "Sig.", 0.0792308065075),
            ("Parameter Estimates", "3 / x", "Exp(B)", 5.87846183501),
        ],
    );
}

#[test]
fn covariate_near_one_million_keeps_its_slope() {
    let t = run_on(
        SHIFTED,
        "NOMREG y (BASE=FIRST) WITH x /PRINT=MFI PARAMETER SUMMARY.",
    );
    check(
        &t,
        &[
            (
                "Model Fitting Information",
                "Intercept Only",
                "Model Fitting Criteria / -2 Log Likelihood",
                32.555658389,
            ),
            (
                "Model Fitting Information",
                "Final",
                "Model Fitting Criteria / -2 Log Likelihood",
                28.5714068207,
            ),
            (
                "Model Fitting Information",
                "Final",
                "Likelihood Ratio Tests / Chi-Square",
                3.98425156834,
            ),
            ("Pseudo R-Square", "Cox and Snell", "Value", 0.233267094681),
            ("Pseudo R-Square", "Nagelkerke", "Value", 0.263321135062),
            ("Pseudo R-Square", "McFadden", "Value", 0.122382767405),
            ("Parameter Estimates", "2 / Intercept", "B", -444423.750307),
            (
                "Parameter Estimates",
                "2 / Intercept",
                "Std. Error",
                740889.872378,
            ),
            (
                "Parameter Estimates",
                "2 / Intercept",
                "Wald",
                0.359821577584,
            ),
            (
                "Parameter Estimates",
                "2 / Intercept",
                "Sig.",
                0.548605343433,
            ),
            ("Parameter Estimates", "2 / x", "B", 0.444422277364),
            ("Parameter Estimates", "2 / x", "Std. Error", 0.740887712067),
            ("Parameter Estimates", "2 / x", "Wald", 0.359821290848),
            ("Parameter Estimates", "2 / x", "Sig.", 0.548605502733),
            ("Parameter Estimates", "2 / x", "Exp(B)", 1.55958892569),
            ("Parameter Estimates", "3 / Intercept", "B", -1694366.14027),
            (
                "Parameter Estimates",
                "3 / Intercept",
                "Std. Error",
                1022414.31483,
            ),
            (
                "Parameter Estimates",
                "3 / Intercept",
                "Wald",
                2.74638036344,
            ),
            (
                "Parameter Estimates",
                "3 / Intercept",
                "Sig.",
                0.0974748681589,
            ),
            ("Parameter Estimates", "3 / x", "B", 1.69436010046),
            ("Parameter Estimates", "3 / x", "Std. Error", 1.02241079773),
            ("Parameter Estimates", "3 / x", "Wald", 2.74637967875),
            ("Parameter Estimates", "3 / x", "Sig.", 0.0974749099085),
            ("Parameter Estimates", "3 / x", "Exp(B)", 5.44316177385),
        ],
    );
}

#[test]
fn constant_covariate_is_left_out() {
    let t = run("NOMREG y (BASE=FIRST) WITH x k /PRINT=PARAMETER.");
    let table = t.iter().find(|t| t.title == "Parameter Estimates").unwrap();
    let keys = row_keys(table);
    let k = keys.iter().position(|k| k == "2 / k").unwrap();
    // the parameter of a redundant column is set to 0 and has no standard error
    assert_eq!(table.rows[k][2], Cell::Number(0.0));
    assert_eq!(table.rows[k][3], Cell::Empty);
}
