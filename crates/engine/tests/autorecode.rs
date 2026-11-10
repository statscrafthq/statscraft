//! AUTORECODE. The expected codes and labels agree with the reference program;
//! `reference_data.rs` repeats the comparison on whole files.

use statscraft_data::{MissingValues, Value, Values};
use statscraft_engine::cell_text;
use statscraft_engine::session::{RunResult, Session};

const DATA: &str = "\
DATA LIST LIST /s (A3) n (F5.1) m (F3) t (A2).
BEGIN DATA
b 2.5 1 x
a 10 9 y
\"\" 2.5 9 z
c . 98 x
a -1 99 w
b 7 3 \"\"
END DATA.
MISSING VALUES n (7) m (98 99) t ('w').
VALUE LABELS n 10 'ten' / s 'a' 'Alpha'.
VARIABLE LABELS n 'Number'.
";

fn run(source: &str) -> (Session, RunResult) {
    let mut session = Session::default();
    let result = session
        .run(&format!("{DATA}{source}"), None)
        .unwrap_or_else(|errors| panic!("{errors:?}"));
    (session, result)
}

fn codes(session: &Session, name: &str) -> String {
    let data = session.dataset().unwrap();
    let Values::Numeric(values) = data.variable(name).unwrap().values() else {
        panic!()
    };
    let shown: Vec<String> = values
        .iter()
        .map(|v| v.map_or(".".into(), |v| v.to_string()))
        .collect();
    shown.join(" ")
}

fn labels(session: &Session, name: &str) -> Vec<String> {
    let data = session.dataset().unwrap();
    let variable = data.variable(name).unwrap();
    variable
        .value_labels()
        .iter()
        .map(|(value, label)| match value {
            Value::Number(n) => format!("{n}={label}"),
            Value::String(s) => format!("{s}={label}"),
        })
        .collect()
}

#[test]
fn values_become_codes_in_ascending_order_with_missing_values_last() {
    let (session, result) = run("AUTORECODE VARIABLES=s n /INTO rs rn /PRINT.");
    assert_eq!(codes(&session, "rs"), "3 2 1 4 2 3");
    assert_eq!(codes(&session, "rn"), "2 3 2 . 1 4");
    assert_eq!(labels(&session, "rs"), ["2=Alpha", "3=b", "4=c"]);
    assert_eq!(
        labels(&session, "rn"),
        ["1=  -1.0", "2=   2.5", "3=ten", "4=   7.0"]
    );
    let data = session.dataset().unwrap();
    let rn = data.variable("rn").unwrap();
    assert_eq!(rn.label(), Some("Number"));
    assert_eq!(rn.format(), "F1.0");
    assert_eq!(*rn.missing_values(), MissingValues::Numbers(vec![4.0]));
    let table = &result.commands.last().unwrap().tables[1];
    assert_eq!(table.title, "Recoding n into rn");
    assert_eq!(
        (0..3).map(|c| cell_text(table, 2, c)).collect::<Vec<_>>(),
        ["10.0", "3", "ten"]
    );
}

#[test]
fn descending_numbers_from_the_highest_value() {
    let (session, _) = run("AUTORECODE m /INTO rm /DESCENDING.");
    assert_eq!(codes(&session, "rm"), "3 1 1 5 4 2");
    let data = session.dataset().unwrap();
    assert_eq!(
        *data.variable("rm").unwrap().missing_values(),
        MissingValues::Numbers(vec![4.0, 5.0])
    );
}

#[test]
fn group_shares_one_scheme_and_blank_can_be_missing() {
    let (session, _) = run("AUTORECODE s t /INTO g1 g2 /GROUP /BLANK=MISSING.");
    // a b c x y z are valid; w is missing as t defines it; the blank is missing too
    assert_eq!(codes(&session, "g1"), "2 1 7 3 1 2");
    assert_eq!(codes(&session, "g2"), "4 5 6 4 8 7");
    assert_eq!(labels(&session, "g1")[0], "1=Alpha");
    let data = session.dataset().unwrap();
    assert_eq!(
        *data.variable("g2").unwrap().missing_values(),
        MissingValues::Numbers(vec![7.0, 8.0])
    );
}

#[test]
fn autorecode_errors() {
    let mut session = Session::default();
    session.run(DATA, None).unwrap();
    let message = |session: &mut Session, source: &str| {
        session.run(source, None).unwrap_err()[0].message.clone()
    };
    assert_eq!(
        message(&mut session, "AUTORECODE s n /INTO a."),
        "2 variables to recode but 1 after INTO; give one new name per variable"
    );
    assert_eq!(
        message(&mut session, "AUTORECODE s /INTO n."),
        "n already exists: AUTORECODE needs new variables after INTO"
    );
    assert_eq!(
        message(&mut session, "AUTORECODE s n /INTO a b /GROUP."),
        "GROUP needs variables of one type: all numeric or all strings"
    );
}
