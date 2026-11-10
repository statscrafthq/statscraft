//! Scratch variables, LAG and $CASENUM. `reference_data.rs` compares the same rules with the
//! reference program on whole files.

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::Session;

fn run(session: &mut Session, source: &str) {
    if let Err(errors) = session.run(source, None) {
        panic!("{errors:?}");
    }
}

fn error(session: &mut Session, source: &str) -> String {
    session.run(source, None).unwrap_err()[0].message.clone()
}

fn show(data: &Dataset, name: &str) -> String {
    match data.variable(name).unwrap().values() {
        Values::Numeric(values) => values
            .iter()
            .map(|v| v.map_or(".".to_string(), |v| v.to_string()))
            .collect::<Vec<_>>()
            .join(" "),
        Values::String(values) => values.iter().collect::<Vec<_>>().join("|"),
    }
}

fn data(source: &str) -> Session {
    let mut session = Session::default();
    run(&mut session, source);
    session
}

const THREE: &str = "DATA LIST FREE /Var1.\nBEGIN DATA\n2 2 2\nEND DATA.\n";

#[test]
fn a_scratch_variable_starts_at_zero_and_carries_from_case_to_case() {
    // the example of the manual: a new variable starts missing in every case, a scratch
    // variable only once
    let session = data(&format!(
        "{THREE}COMPUTE Var2 = Var1 + Var2.\nCOMPUTE #s = Var1 + #s.\nCOMPUTE Var4 = #s."
    ));
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "Var2"), ". . .");
    assert_eq!(show(data, "Var4"), "2 4 6");
    assert!(data.variable_names().all(|n| !n.starts_with('#')));
}

#[test]
fn a_scratch_variable_assigned_before_use_needs_no_earlier_case() {
    let session = data(&format!(
        "{THREE}COMPUTE #t = Var1 * $CASENUM.\nCOMPUTE t = #t + 1."
    ));
    assert_eq!(show(session.dataset().unwrap(), "t"), "3 5 7");
}

#[test]
fn a_value_kept_inside_do_if_fills_the_cases_after_it() {
    let source = "DATA LIST LIST /x.\nBEGIN DATA\n5\n.\n.\n7\n.\nEND DATA.\n\
        DO IF NOT MISSING(x).\nCOMPUTE #last = x.\nEND IF.\nCOMPUTE filled = #last.\n\
        IF MISSING(x) x = LAG(x).";
    let session = data(source);
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "filled"), "5 5 5 7 7");
    // LAG sees the earlier case as it ended the pass, so the fill runs on
    assert_eq!(show(data, "x"), "5 5 5 7 7");
}

#[test]
fn lag_counts_the_kept_cases_and_casenum_every_case_read() {
    let source = "DATA LIST FREE /x.\nBEGIN DATA\n1 2 3 4 5\nEND DATA.\n\
        COMPUTE #n = #n + 1.\nSELECT IF x <> 2.\nCOMPUTE n = #n.\nCOMPUTE c = $CASENUM.\n\
        COMPUTE back = LAG(x).\nCOMPUTE back2 = LAG(x, 2).";
    let session = data(source);
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "n"), "1 3 4 5");
    assert_eq!(show(data, "c"), "1 3 4 5");
    assert_eq!(show(data, "back"), ". 1 3 4");
    assert_eq!(show(data, "back2"), ". . 1 3");
}

#[test]
fn lag_of_a_string_is_blank_at_first() {
    let source = "DATA LIST LIST /s (A3).\nBEGIN DATA\nab\ncd\nEND DATA.\n\
        STRING prev (A3).\nCOMPUTE prev = LAG(s).";
    let session = data(source);
    assert_eq!(show(session.dataset().unwrap(), "prev"), "|ab");
}

#[test]
fn scratch_variables_end_with_the_pass() {
    let mut session = data(THREE);
    assert_eq!(
        error(&mut session, "COMPUTE #a = 1.\nEXECUTE.\nCOMPUTE b = #a."),
        "unknown variable #a"
    );
    assert_eq!(
        error(&mut session, "COMPUTE #a = 1.\nDESCRIPTIVES #a."),
        "unknown variable #a"
    );
    assert_eq!(
        error(&mut session, "COMPUTE #a = 1.\nTEMPORARY.\nCOMPUTE b = #a."),
        "unknown variable #a"
    );
    // nor does the next run see them
    run(&mut session, "COMPUTE #a = 1.");
    assert_eq!(
        error(&mut session, "COMPUTE b = #a."),
        "unknown variable #a"
    );
}

#[test]
fn scratch_variables_have_no_labels_weights_or_splits() {
    let mut session = data(THREE);
    let message = |session: &mut Session, command: &str| {
        error(session, &format!("COMPUTE #a = 1.\n{command}"))
    };
    assert_eq!(
        message(&mut session, "WEIGHT BY #a."),
        "WEIGHT cannot name the scratch variable #a"
    );
    assert_eq!(
        message(&mut session, "VARIABLE LABELS #a 'A'."),
        "VARIABLE LABELS cannot name the scratch variable #a"
    );
    assert_eq!(
        message(&mut session, "MISSING VALUES #a (9)."),
        "MISSING VALUES cannot name the scratch variable #a"
    );
    assert_eq!(
        message(&mut session, "SPLIT FILE BY #a."),
        "SPLIT FILE cannot name the scratch variable #a"
    );
    assert_eq!(
        message(&mut session, "COUNT n = Var1 TO #a (1)."),
        "a TO list cannot hold scratch and other variables at once (Var1 TO #a)"
    );
    // ALL leaves scratch variables out
    run(&mut session, "COMPUTE #a = 2.\nCOUNT n = ALL (2).");
    assert_eq!(show(session.dataset().unwrap(), "n"), "1 1 1");
}

#[test]
fn numeric_and_string_declare_scratch_variables_that_keep_their_value() {
    let source = format!(
        "{THREE}NUMERIC #n.\nSTRING #s (A4).\nCOMPUTE #n = #n + Var1.\n\
         IF ($CASENUM = 2) #s = 'two'.\nCOMPUTE n = #n.\nSTRING s (A4).\nCOMPUTE s = #s."
    );
    let session = data(&source);
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "n"), "2 4 6");
    assert_eq!(show(data, "s"), "|two|two");
}

#[test]
fn an_error_case_by_case_names_the_case() {
    let mut session = data(THREE);
    let message = error(
        &mut session,
        "COMPUTE #a = #a + 1.\nCOMPUTE b = (#a > 1) AND Var1.",
    );
    assert!(
        message.starts_with("case 1: the right operand of AND must be 0 or 1"),
        "{message}"
    );
}
