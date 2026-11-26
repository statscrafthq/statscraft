//! FIT on invented residual and observed series, against R: the means, sums and the Durbin-Watson
//! statistic written out from the formulas, with a missing residual, an observed value of 0, the
//! three ways to give the degrees of freedom and a PREDICT period inside the series; and the
//! default series after ARIMA.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Fourteen residuals (two missing) and observed values (one 0), and a second residual series.
const SERIES: &str = "DATA LIST FREE /e x e2.
BEGIN DATA
0.8 20 .
-1.2 22.5 0.5
. 19 -0.4
0.4 0 0.9
1.9 24 -1.1
-0.7 21 0.2
0.0 23 0.7
2.2 25 -0.3
-1.6 22 1.4
0.3 20.5 -0.8
-0.9 21.5 0.1
1.1 24.5 -0.6
. 23 0.3
0.6 26 1.0
END DATA.
";

fn run_on(data: &str, syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

fn error(data: &str, syntax: &str) -> String {
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            return e.to_string();
        }
    }
    panic!("no error");
}

fn fit_table(tables: &[Table]) -> &Table {
    tables
        .iter()
        .find(|t| t.title == "Fit Statistics")
        .expect("table")
}

/// The value of the row `label` in column number `column` (1 is the first series).
fn value(t: &Table, label: &str, column: usize) -> Option<f64> {
    let row = t
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == label))
        .unwrap_or_else(|| panic!("no row {label}"));
    match &row[column] {
        Cell::Number(n) => Some(*n),
        Cell::Empty => None,
        other => panic!("{label}: {other:?}"),
    }
}

/// me, mpe, mae, mape, sse, mse, rms, dw, dfe, n against the values of R.
fn check(t: &Table, column: usize, want: [f64; 10]) {
    let labels = [
        "Mean Error",
        "Mean Percent Error",
        "Mean Absolute Error",
        "Mean Absolute Percent Error",
        "Sum of Squared Errors",
        "Mean Square Error",
        "Root Mean Square Error",
        "Durbin-Watson",
        "Error Degrees of Freedom",
        "N of Cases",
    ];
    for (label, want) in labels.iter().zip(want) {
        let got = value(t, label, column).unwrap_or_else(|| panic!("{label} is empty"));
        assert!(
            (got - want).abs() <= 1e-9 * want.abs().max(1.0),
            "{label}: {got} vs {want}"
        );
    }
}

#[test]
fn the_statistics_agree_with_r() {
    // DFE the cases, DFH 2, DFE 9.5
    let t = run_on(SERIES, "FIT ERRORS=e /OBS=x.");
    check(
        fit_table(&t),
        1,
        [
            0.241666666667,
            0.804739006896,
            0.975,
            4.46390999799,
            16.21,
            1.350833333333,
            1.162253558107,
            2.58050586058,
            12.0,
            12.0,
        ],
    );
    let t = run_on(SERIES, "FIT e /OBS=x /DFH=2.");
    check(
        fit_table(&t),
        1,
        [
            0.241666666667,
            0.804739006896,
            0.975,
            4.46390999799,
            16.21,
            1.621,
            1.273184982632,
            2.58050586058,
            10.0,
            12.0,
        ],
    );
    let t = run_on(SERIES, "FIT VARIABLES=e /OBS=x /DFE=9.5.");
    check(
        fit_table(&t),
        1,
        [
            0.241666666667,
            0.804739006896,
            0.975,
            4.46390999799,
            16.21,
            1.706315789474,
            1.30626023038,
            2.58050586058,
            9.5,
            12.0,
        ],
    );
    // two residual series, each with its observed series and its DFH; the second has one missing
    let t = run_on(SERIES, "FIT ERRORS=e e2 /OBS=x x /DFH=2 3.");
    let table = fit_table(&t);
    check(
        table,
        2,
        [
            0.146153846154,
            0.329776720247,
            0.638461538462,
            2.70311257149,
            7.11,
            0.711,
            0.843208159353,
            2.7805907173,
            10.0,
            13.0,
        ],
    );
}

#[test]
fn without_observed_series_there_are_no_percent_statistics() {
    let t = run_on(SERIES, "FIT e.");
    let table = fit_table(&t);
    assert!(
        !table
            .rows
            .iter()
            .any(|r| matches!(&r[0], Cell::Text(s) if s.contains("Percent")))
    );
    assert!((value(table, "Sum of Squared Errors", 1).unwrap() - 16.21).abs() < 1e-9);
}

#[test]
fn a_predict_period_inside_the_series_has_columns_of_its_own() {
    // USE 1 to 10 and PREDICT 11 to 14: R's statistics of the two parts; the PREDICT period's
    // degrees of freedom are its own cases
    let t = run_on(SERIES, "USE 1 THRU 10.\nPREDICT 11 THRU 14.\nFIT e /OBS=x.");
    let table = fit_table(&t);
    assert_eq!(table.columns.len(), 3);
    check(
        table,
        1,
        [
            0.233333333333,
            0.780085920177,
            1.011111111111,
            4.764934405026,
            13.83,
            1.536666666667,
            1.239623598786,
            2.631236442516,
            9.0,
            9.0,
        ],
    );
    check(
        table,
        2,
        [
            0.266666666667,
            0.870480571477,
            0.866666666667,
            3.661178245896,
            2.38,
            0.793333333333,
            0.890692614392,
            1.680672268908,
            3.0,
            3.0,
        ],
    );
}

#[test]
fn without_names_it_takes_the_last_saved_error_series_and_its_observed_series() {
    let data = "DATA LIST FREE /y.\nBEGIN DATA\n5.1 5.9 6.4 6.2 7.1 7.8 7.5 8.4 9.0 8.8 9.7 10.1\nEND DATA.\n";
    let named = run_on(
        data,
        "TSET NEWVAR=ALL.\nARIMA y /MODEL=(1,0,0).\nFIT ERRORS=ERR_1 /OBS=y.",
    );
    let default = run_on(data, "TSET NEWVAR=ALL.\nARIMA y /MODEL=(1,0,0).\nFIT.");
    assert_eq!(fit_table(&named).rows, fit_table(&default).rows);
    assert!(value(fit_table(&default), "Mean Percent Error", 1).is_some());
    // the later series is the one: a second ARIMA saves ERR_2
    let second = run_on(
        data,
        "TSET NEWVAR=ALL.\nARIMA y /MODEL=(1,0,0).\nARIMA y /MODEL=(0,1,1).\nFIT.",
    );
    let wanted = run_on(
        data,
        "TSET NEWVAR=ALL.\nARIMA y /MODEL=(1,0,0).\nARIMA y /MODEL=(0,1,1).\nFIT ERRORS=ERR_2 /OBS=y.",
    );
    assert_eq!(fit_table(&second).rows, fit_table(&wanted).rows);
}

#[test]
fn split_groups_are_blocks_of_one_table() {
    let data = "DATA LIST FREE /g e.\nBEGIN DATA\n1 1\n1 -2\n1 3\n2 0.5\n2 0.5\n2 -1\nEND DATA.\nSPLIT FILE BY g.\n";
    let t = run_on(data, "FIT e.");
    let tables: Vec<&Table> = t.iter().filter(|t| t.title == "Fit Statistics").collect();
    assert_eq!(tables.len(), 1);
    // each group is a block of rows led by its split value
    let sse: Vec<f64> = tables[0]
        .rows
        .iter()
        .filter(|r| matches!(&r[1], Cell::Text(s) if s == "Sum of Squared Errors"))
        .map(|r| match &r[2] {
            Cell::Number(n) => *n,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(sse.len(), 2);
    assert!((sse[0] - 14.0).abs() < 1e-12);
    assert!((sse[1] - 1.5).abs() < 1e-12);
}

#[test]
fn fit_is_checked() {
    assert!(error(SERIES, "FIT.").contains("no procedure has saved an ERR_n series"));
    assert!(error(SERIES, "FIT e e2 /OBS=x.").contains("OBS names 1 series for 2 residual series"));
    assert!(
        error(SERIES, "FIT e e2 /DFE=3.")
            .contains("DFE and DFH name 1 values for 2 residual series")
    );
    assert!(error(SERIES, "FIT nosuch.").contains("nosuch"));
    let (_, errors) = parse(&format!("{SERIES}FIT /OBS=x."));
    assert!(
        errors[0].message.contains("OBS needs the residual series"),
        "{errors:?}"
    );
}
