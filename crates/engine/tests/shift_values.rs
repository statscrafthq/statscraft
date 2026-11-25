//! SHIFT VALUES on an invented series of seven readings with a gap, against R: the vector
//! shifted by hand (`x[i + k]` inside the data, NA outside), also within split groups.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

const SERIES: &str = "DATA LIST FREE /x (F5.2) g f.
MISSING VALUES g (9).
VARIABLE LABELS x 'Reading'.
VALUE LABELS g 1 'north' 2 'south'.
BEGIN DATA
5.5 1 1
7 1 1
. 1 0
2.25 1 1
9 2 1
4 2 0
8.5 2 1
END DATA.
";

fn run_on(data: &str, syntax: &str) -> Engine {
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
    }
    engine
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
            Some(g) => assert!((g - w).abs() < 1e-12, "{name}[{i}]: {g} vs {w}"),
        }
    }
}

#[test]
fn lead_lag_and_shift_agree_with_the_vector_shifted_in_r() {
    let e = run_on(
        SERIES,
        "SHIFT VALUES VARIABLE=x RESULT=lead1 LEAD=1\n\
           /VARIABLE=x RESULT=lead2 LEAD=2\n\
           /VARIABLE=x RESULT=lag1 LAG=1\n\
           /VARIABLE=x RESULT=lag3 LAG=3\n\
           /VARIABLE=x RESULT=same SHIFT=0\n\
           /VARIABLE=x RESULT=back SHIFT=-1\n\
           /VARIABLE=x RESULT=ahead SHIFT=2\n\
           /VARIABLE=x RESULT=toofar LEAD=7\n\
           /VARIABLE=x RESULT=tooback SHIFT=-7.",
    );
    check(&e, "lead1", &[7.0, NA, 2.25, 9.0, 4.0, 8.5, NA]);
    check(&e, "lead2", &[NA, 2.25, 9.0, 4.0, 8.5, NA, NA]);
    check(&e, "lag1", &[NA, 5.5, 7.0, NA, 2.25, 9.0, 4.0]);
    check(&e, "lag3", &[NA, NA, NA, 5.5, 7.0, NA, 2.25]);
    check(&e, "same", &[5.5, 7.0, NA, 2.25, 9.0, 4.0, 8.5]);
    check(&e, "back", &[NA, 5.5, 7.0, NA, 2.25, 9.0, 4.0]);
    check(&e, "ahead", &[NA, 2.25, 9.0, 4.0, 8.5, NA, NA]);
    check(&e, "toofar", &[NA; 7]);
    check(&e, "tooback", &[NA; 7]);
}

#[test]
fn split_groups_are_shifted_apart_and_a_filter_is_ignored() {
    // R: tapply over the groups of the same vector
    let e = run_on(
        SERIES,
        "SPLIT FILE BY g.\nSHIFT VALUES VARIABLE=x RESULT=lead1 LEAD=1 /VARIABLE=x RESULT=lag2 LAG=2.",
    );
    check(&e, "lead1", &[7.0, NA, 2.25, NA, 4.0, 8.5, NA]);
    check(&e, "lag2", &[NA, NA, 5.5, 7.0, NA, NA, 9.0]);
    // cases the filter leaves out still count and still receive a value
    let e = run_on(
        SERIES,
        "FILTER BY f.\nSHIFT VALUES VARIABLE=x RESULT=lead1 LEAD=1.",
    );
    check(&e, "lead1", &[7.0, NA, 2.25, 9.0, 4.0, 8.5, NA]);
}

#[test]
fn the_result_keeps_the_dictionary_entry_without_custom_attributes() {
    let e = run_on(
        &format!("{SERIES}VARIABLE ATTRIBUTE VARIABLES=g ATTRIBUTE=source('field team').\n"),
        "SHIFT VALUES VARIABLE=g RESULT=g_next LEAD=1 /VARIABLE=x RESULT=x_prev LAG=1.",
    );
    let data = e.dataset().unwrap();
    let g = data.variable("g").unwrap();
    let next = data.variable("g_next").unwrap();
    assert_eq!(next.value_labels().len(), 2);
    assert_eq!(next.missing_values(), g.missing_values());
    assert_eq!(next.format(), g.format());
    assert_eq!(next.measure(), g.measure());
    assert!(!g.attributes().is_empty());
    assert!(next.attributes().is_empty());
    assert_eq!(next.label(), Some("g from 1 case(s) later"));
    assert_eq!(
        data.variable("x_prev").unwrap().label(),
        Some("x from 1 case(s) earlier")
    );
    assert_eq!(
        data.variable("x_prev").unwrap().format(),
        data.variable("x").unwrap().format()
    );
    // the first and last cases have no source
    check(&e, "g_next", &[1.0, 1.0, 1.0, 2.0, 2.0, 2.0, NA]);
}

#[test]
fn a_user_missing_value_is_carried_over_and_a_string_gets_an_empty_value() {
    let data = "DATA LIST FREE /r (F1) s (A3).\nMISSING VALUES r (9).\nBEGIN DATA\n0 aa\n9 bb\n1 cc\nEND DATA.\n";
    let e = run_on(
        data,
        "SHIFT VALUES VARIABLE=r RESULT=r1 LEAD=1 /VARIABLE=s RESULT=s1 LAG=1.",
    );
    check(&e, "r1", &[9.0, 1.0, NA]);
    let d = e.dataset().unwrap();
    let Values::String(s) = d.variable("s1").unwrap().values() else {
        panic!("s1 is not a string")
    };
    assert_eq!(
        (0..3).map(|i| s.get(i).to_string()).collect::<Vec<_>>(),
        ["", "aa", "bb"]
    );
}

#[test]
fn shift_values_is_checked() {
    assert!(error(SERIES, "SHIFT VALUES VARIABLE=x RESULT=g LEAD=1.").contains("g already exists"));
    assert!(
        error(
            SERIES,
            "SHIFT VALUES VARIABLE=x RESULT=a LEAD=1 /VARIABLE=a RESULT=x LAG=1."
        )
        .contains("x already exists")
    );
    assert!(error(SERIES, "SHIFT VALUES VARIABLE=nosuch RESULT=n LEAD=1.").contains("nosuch"));
    // a failed command leaves no new variable behind
    let (commands, _) = parse(&format!(
        "{SERIES}SHIFT VALUES VARIABLE=x RESULT=a LEAD=1 /VARIABLE=x RESULT=x LEAD=1.\n"
    ));
    let mut engine = Engine::default();
    let mut failed = false;
    for c in &commands {
        failed |= engine.execute(&c.command).is_err();
    }
    assert!(failed);
    assert!(engine.dataset().unwrap().variable("a").is_err());
    for (text, wanted) in [
        (
            "SHIFT VALUES VARIABLE=x RESULT=n LEAD=-1.",
            "unexpected '-'",
        ),
        ("SHIFT VALUES VARIABLE=x RESULT=n LAG=1.5.", "whole number"),
        ("SHIFT VALUES VARIABLE=x LEAD=1.", "needs RESULT="),
        ("SHIFT VALUES RESULT=n LEAD=1.", "needs VARIABLE="),
        ("SHIFT VALUES VARIABLE=x RESULT=n.", "needs LEAD="),
    ] {
        let (_, errors) = parse(&format!("{SERIES}{text}"));
        assert!(
            errors.first().is_some_and(|e| e.message.contains(wanted)),
            "{text}: {errors:?}"
        );
    }
}
