//! RMV on an invented hourly series with leading, inner and trailing gaps, against R: approx()
//! for LINT, loops over the valid values on either side for MEAN and MEDIAN (with a span and
//! with ALL), the mean for SMEAN and lm on the case position for TREND.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// 24 readings; `.` is system-missing.
const SERIES: &str = "DATA LIST FREE /x.
BEGIN DATA
. . 41.2 43.8 40.5 . . . 47.9 46.1 49.3 50.8 . 53.4 52.2 . 55.9 57.1 . 58.6 61.3 60.2 . .
END DATA.
";

fn run_on(data: &str, syntax: &str) -> (Engine, Vec<Table>) {
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    (engine, tables)
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

fn column(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    let Values::Numeric(v) = data.variable(name).unwrap().values() else {
        panic!("{name} is not numeric")
    };
    (0..data.case_count()).map(|c| v.get(c)).collect()
}

const NA: f64 = f64::NAN;

fn check(engine: &Engine, name: &str, want: &[f64]) {
    let got = column(engine, name);
    assert_eq!(got.len(), want.len(), "{name}");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        match g {
            None => assert!(w.is_nan(), "{name}[{i}] is missing, R has {w}"),
            Some(g) => assert!(
                (g - w).abs() <= 1e-9 * w.abs().max(1.0),
                "{name}[{i}]: {g} vs {w}"
            ),
        }
    }
}

#[test]
fn every_function_agrees_with_r() {
    let (engine, tables) = run_on(
        SERIES,
        "RMV lint = LINT(x) /mean2 = MEAN(x) /mean3 = MEAN(x, 3) /med2 = MEDIAN(x, 2) /med3 = MEDIAN(x 3)\n\
         /medall = MEDIAN(x, ALL) /smean = SMEAN(x) /trend = TREND(x).",
    );
    check(
        &engine,
        "lint",
        &[
            NA, NA, 41.2, 43.8, 40.5, 42.35, 44.2, 46.05, 47.9, 46.1, 49.3, 50.8, 52.1, 53.4, 52.2,
            54.05, 55.9, 57.1, 57.85, 58.6, 61.3, 60.2, NA, NA,
        ],
    );
    check(
        &engine,
        "mean2",
        &[
            NA, NA, 41.2, 43.8, 40.5, 44.575, 44.575, 44.575, 47.9, 46.1, 49.3, 50.8, 51.425, 53.4,
            52.2, 54.65, 55.9, 57.1, 58.225, 58.6, 61.3, 60.2, NA, NA,
        ],
    );
    check(
        &engine,
        "mean3",
        &[
            NA,
            NA,
            41.2,
            43.8,
            40.5,
            44.8,
            44.8,
            44.8,
            47.9,
            46.1,
            49.3,
            50.8,
            51.28333333,
            53.4,
            52.2,
            54.66666667,
            55.9,
            57.1,
            57.55,
            58.6,
            61.3,
            60.2,
            NA,
            NA,
        ],
    );
    check(
        &engine,
        "med2",
        &[
            NA, NA, 41.2, 43.8, 40.5, 44.95, 44.95, 44.95, 47.9, 46.1, 49.3, 50.8, 51.5, 53.4,
            52.2, 54.65, 55.9, 57.1, 57.85, 58.6, 61.3, 60.2, NA, NA,
        ],
    );
    check(
        &engine,
        "med3",
        &[
            NA, NA, 41.2, 43.8, 40.5, 44.95, 44.95, 44.95, 47.9, 46.1, 49.3, 50.8, 51.5, 53.4,
            52.2, 54.65, 55.9, 57.1, 57.85, 58.6, 61.3, 60.2, NA, NA,
        ],
    );
    check(
        &engine,
        "medall",
        &[
            51.5, 51.5, 41.2, 43.8, 40.5, 51.5, 51.5, 51.5, 47.9, 46.1, 49.3, 50.8, 51.5, 53.4,
            52.2, 51.5, 55.9, 57.1, 51.5, 58.6, 61.3, 60.2, 51.5, 51.5,
        ],
    );
    let m = 51.30714286;
    check(
        &engine,
        "smean",
        &[
            m, m, 41.2, 43.8, 40.5, m, m, m, 47.9, 46.1, 49.3, 50.8, m, 53.4, 52.2, m, 55.9, 57.1,
            m, 58.6, 61.3, 60.2, m, m,
        ],
    );
    check(
        &engine,
        "trend",
        &[
            38.54047718,
            39.61073658,
            41.2,
            43.8,
            40.5,
            43.89177417,
            44.96203357,
            46.03229297,
            47.9,
            46.1,
            49.3,
            50.8,
            51.38358996,
            53.4,
            52.2,
            54.59436815,
            55.9,
            57.1,
            57.80514634,
            58.6,
            61.3,
            60.2,
            62.08618394,
            63.15644333,
        ],
    );
    // replaced, first and last valid case, valid cases, function
    let t = &tables[0];
    assert_eq!(t.title, "Result Variables");
    let row = |name: &str| -> Vec<Cell> {
        t.rows
            .iter()
            .find(|r| matches!(&r[0], Cell::Text(s) if s == name))
            .unwrap_or_else(|| panic!("no row {name}"))
            .clone()
    };
    let numbers = |name: &str| -> Vec<f64> {
        row(name)[1..5]
            .iter()
            .map(|c| match c {
                Cell::Number(n) => *n,
                other => panic!("{other:?}"),
            })
            .collect()
    };
    assert_eq!(numbers("lint"), [6.0, 3.0, 22.0, 20.0]);
    assert_eq!(numbers("mean3"), [6.0, 3.0, 22.0, 20.0]);
    assert_eq!(numbers("medall"), [10.0, 1.0, 24.0, 24.0]);
    assert_eq!(row("mean3")[5], Cell::Text("MEAN(x,3)".into()));
    assert_eq!(row("smean")[5], Cell::Text("SMEAN(x)".into()));
    assert_eq!(row("medall")[5], Cell::Text("MEDIAN(x,ALL)".into()));
}

#[test]
fn a_variable_can_replace_itself_and_names_can_be_numbered() {
    let data = "DATA LIST FREE /a b.\nBEGIN DATA\n1 10\n. .\n3 30\n. 40\nEND DATA.\n";
    let (engine, _) = run_on(data, "RMV c1 TO c2 = LINT(a b) /a = LINT(a).");
    check(&engine, "c1", &[1.0, 2.0, 3.0, NA]);
    check(&engine, "c2", &[10.0, 20.0, 30.0, 40.0]);
    check(&engine, "a", &[1.0, 2.0, 3.0, NA]);
    // the originals are untouched when the new names differ
    check(&engine, "b", &[10.0, NA, 30.0, 40.0]);
    // a second equation can use a variable an earlier one made
    let (engine, _) = run_on(data, "RMV c = LINT(a) /d = SMEAN(c).");
    check(&engine, "d", &[1.0, 2.0, 3.0, 2.0]);
}

#[test]
fn user_missing_values_count_as_missing_unless_tset_includes_them() {
    let data =
        "DATA LIST FREE /v.\nMISSING VALUES v (-99).\nBEGIN DATA\n4 -99 8 6 -99 10\nEND DATA.\n";
    let (engine, _) = run_on(data, "RMV f = LINT(v).");
    check(&engine, "f", &[4.0, 6.0, 8.0, 6.0, 8.0, 10.0]);
    let (engine, _) = run_on(data, "TSET MISSING=INCLUDE.\nRMV f = LINT(v).");
    check(&engine, "f", &[4.0, -99.0, 8.0, 6.0, -99.0, 10.0]);
}

/// Each split group is a series of its own: the result equals running each group alone, and
/// the case position of TREND starts again in each.
#[test]
fn split_file_groups_are_series_of_their_own() {
    let g1 = "1 5\n1 .\n1 9\n1 .\n1 13";
    let g2 = "2 20\n2 .\n2 16\n2 14\n2 .";
    let whole =
        format!("DATA LIST FREE /g y.\nBEGIN DATA\n{g1}\n{g2}\nEND DATA.\nSPLIT FILE BY g.\n");
    let alone = |rows: &str| {
        let (e, _) = run_on(
            &format!("DATA LIST FREE /g y.\nBEGIN DATA\n{rows}\nEND DATA.\n"),
            "RMV l = LINT(y) /t = TREND(y).",
        );
        (column(&e, "l"), column(&e, "t"))
    };
    let (e, _) = run_on(&whole, "RMV l = LINT(y) /t = TREND(y).");
    let (l1, t1) = alone(g1);
    let (l2, t2) = alone(g2);
    assert_eq!(column(&e, "l"), [l1, l2].concat());
    assert_eq!(column(&e, "t"), [t1, t2].concat());
}

#[test]
fn a_gap_with_too_few_valid_values_stays_and_trend_needs_two() {
    let data = "DATA LIST FREE /v.\nBEGIN DATA\n. 5 . 7 .\nEND DATA.\n";
    let (e, _) = run_on(data, "RMV m = MEAN(v, 2) /n = MEAN(v, 1) /t = TREND(v).");
    check(&e, "m", &[NA, 5.0, NA, 7.0, NA]);
    check(&e, "n", &[NA, 5.0, 6.0, 7.0, NA]);
    check(&e, "t", &[4.0, 5.0, 6.0, 7.0, 8.0]);
    let (e, _) = run_on(
        "DATA LIST FREE /v.\nBEGIN DATA\n. 5 .\nEND DATA.\n",
        "RMV t = TREND(v).",
    );
    check(&e, "t", &[NA, 5.0, NA]);
}

#[test]
fn rmv_is_checked() {
    let data = "DATA LIST FREE /a b.\nBEGIN DATA\n1 2\n. 3\nEND DATA.\n";
    assert!(error(data, "RMV c = LINT(a b).").contains("makes 2 series here, but 1 new names"));
    assert!(error(data, "RMV c c = LINT(a b).").contains("named twice"));
    let (_, errors) = parse(&format!("{data}RMV c = LINT(a, 2)."));
    assert!(
        errors[0]
            .message
            .contains("LINT takes only a list of variables"),
        "{errors:?}"
    );
    let (_, errors) = parse(&format!("{data}RMV c = MEAN(a, 0)."));
    assert!(
        errors[0]
            .message
            .contains("the span takes a whole number of at least 1"),
        "{errors:?}"
    );
    let (_, errors) = parse(&format!("{data}RMV c = SMEAN(a ALL)."));
    assert!(
        errors[0].message.contains("SMEAN takes only a list"),
        "{errors:?}"
    );
    let (_, errors) = parse(&format!("{data}RMV c = FIT(a)."));
    assert!(!errors.is_empty());
}
