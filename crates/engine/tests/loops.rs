//! DO REPEAT, LOOP with BREAK and VECTOR. `reference_data.rs` compares the same commands with
//! the reference program on whole files.

use statscraft_data::{Dataset, Values};
use statscraft_engine::cell_text;
use statscraft_engine::session::{RunResult, Session};

fn run(session: &mut Session, source: &str) -> RunResult {
    session
        .run(source, None)
        .unwrap_or_else(|errors| panic!("{errors:?}"))
}

fn error(session: &mut Session, source: &str) -> String {
    session.run(source, None).unwrap_err()[0].text()
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

fn session(data: &str) -> Session {
    let mut session = Session::default();
    run(
        &mut session,
        &format!("DATA LIST LIST /x y z.\nBEGIN DATA\n{data}\nEND DATA."),
    );
    session
}

#[test]
fn the_index_holds_the_counter_unless_break_or_end_loop_if_ends_the_loop() {
    // the example of the manual, one case
    let mut session = session("1 1 1");
    run(
        &mut session,
        "LOOP a = 1 TO 3.\nCOMPUTE a = 0.\nEND LOOP.\n\
         [removed]
         NUMERIC #d.\nLOOP d = 1 TO 3.\nCOMPUTE d = 0.\nCOMPUTE #d = #d + 1.\n\
         DO IF #d = 3.\nBREAK.\nEND IF.\nEND LOOP.\n\
         LOOP e = 3 TO 1.\nCOMPUTE e = 0.\nEND LOOP.",
    );
    let data = session.dataset().unwrap();
    let values: Vec<String> = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|n| show(data, n))
        .collect();
    assert_eq!(values, ["4", "0", "4", "0", "3"]);
}

#[test]
fn loop_bounds_conditions_and_limits_per_case() {
    let mut session = session("1 5 2\n3 0 1\n4 . 2\n7 3 -1");
    let result = run(
        &mut session,
        "COMPUTE f = 1.\nLOOP #i = 1 TO y.\nCOMPUTE f = f * #i.\nEND LOOP.\n\
         COMPUTE g = 0.\nLOOP #i = 1 TO 10 BY z IF g < 4.\nCOMPUTE g = g + #i.\nEND LOOP.\n\
         COMPUTE h = 0.\nLOOP.\nCOMPUTE h = h + 1.\nDO IF h >= x.\nBREAK.\nEND IF.\n\
         COMPUTE h = h + 0.5.\nEND LOOP.\n\
         COMPUTE w = 0.\nLOOP.\nCOMPUTE w = w + 1.\nEND LOOP IF w >= 50.\n\
         LOOP #i = 1 TO 2 BY 0.\nEND LOOP.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "f"), "120 1 1 6");
    assert_eq!(show(data, "g"), "4 6 4 0");
    assert_eq!(show(data, "h"), "1 4 4 7");
    // a loop without an index stops after 40 iterations
    assert_eq!(show(data, "w"), "40 40 40 40");
    let warnings = &result.commands.last().unwrap().warnings;
    assert_eq!(warnings, &["a LOOP step of 0 runs no iteration"]);
    // SET MXLOOPS changes the limit
    run(
        &mut session,
        "SET MXLOOPS=60.\nCOMPUTE w = 0.\nLOOP.\nCOMPUTE w = w + 1.\nEND LOOP IF w >= 50.",
    );
    assert_eq!(show(session.dataset().unwrap(), "w"), "50 50 50 50");
    assert!(error(&mut session, "SET MXLOOPS=0.").contains("whole number from 1"));
}

#[test]
fn a_loop_that_carries_scratch_values_runs_case_by_case() {
    let mut session = session("1 2 3\n4 5 6");
    run(
        &mut session,
        "LOOP #i = 1 TO 3.\nCOMPUTE #total = #total + #i.\nEND LOOP.\nCOMPUTE t = #total.",
    );
    assert_eq!(show(session.dataset().unwrap(), "t"), "6 12");
}

#[test]
fn loop_errors() {
    let mut session = session("1 2 3");
    assert_eq!(
        error(&mut session, "END LOOP."),
        "line 1: END LOOP: END LOOP without LOOP"
    );
    assert_eq!(
        error(&mut session, "LOOP #i = 1 TO 2.\nCOMPUTE x = 1."),
        "LOOP without END LOOP: close the loop with END LOOP in the same run"
    );
    assert_eq!(
        error(&mut session, "BREAK."),
        "line 1: BREAK: BREAK is only allowed inside LOOP"
    );
    let message = error(&mut session, "LOOP #i = 1 TO 2.\nFREQUENCIES x.\nEND LOOP.");
    assert!(
        message.contains("cannot be used inside DO IF or LOOP"),
        "{message}"
    );
}

#[test]
fn vectors_read_and_assign_elements() {
    let mut session = session("1 2 3\n4 5 6\n7 8 .");
    let result = run(
        &mut session,
        "VECTOR v = x TO z / n (3, F4.1).\nLOOP #i = 1 TO 3.\nCOMPUTE n(#i) = v(#i) * 10.\n\
         END LOOP.\nCOMPUTE last = v(3).\nCOMPUTE bad = v(x).\nIF (y > 4) n(x) = 0.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "n1"), "10 40 70");
    assert_eq!(show(data, "n2"), "20 50 80");
    assert_eq!(show(data, "n3"), "30 60 .");
    assert_eq!(data.variable("n1").unwrap().format(), "F4.1");
    assert_eq!(show(data, "last"), "3 6 .");
    // an index outside the vector reads missing and assigns nothing
    assert_eq!(show(data, "bad"), "1 . .");
    assert_eq!(
        result.commands.last().unwrap().warnings,
        ["an index of n is missing or outside 1 to 3; those cases are not assigned"]
    );
    // a vector ends with the data pass
    run(&mut session, "EXECUTE.");
    let message = error(&mut session, "COMPUTE w = v(1).");
    assert!(message.contains("unknown function V"), "{message}");
}

#[test]
fn vector_errors() {
    let mut session = session("1 2 3");
    run(&mut session, "STRING s (A3).");
    let message = error(&mut session, "VECTOR m = z TO s.");
    assert!(
        message.contains("all numeric or all strings of one width"),
        "{message}"
    );
    let message = error(&mut session, "COMPUTE nothing(1) = 2.");
    assert!(message.contains("nothing is not a vector"), "{message}");
}

#[test]
fn do_repeat_steps_through_its_lists_together() {
    let mut session = session("1 2 3\n4 5 6");
    let result = run(
        &mut session,
        "DO REPEAT v = x TO z / r = r1 TO r3 / k = 10 20 30.\nCOMPUTE r = v + k.\nEND REPEAT PRINT.\n\
         DO REPEAT s = s1 s2 / t = 'a' \"it's\".\nSTRING s (A4).\nCOMPUTE s = t.\nEND REPEAT.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "r1"), "11 14");
    assert_eq!(show(data, "r3"), "33 36");
    assert_eq!(show(data, "s2"), "it's|it's");
    let table = &result.commands[0].tables[0];
    assert_eq!(table.title, "Commands generated by DO REPEAT");
    let rows: Vec<String> = (0..table.rows.len())
        .map(|r| cell_text(table, r, 0))
        .collect();
    assert_eq!(
        rows,
        [
            "COMPUTE r1 = x + 10",
            "COMPUTE r2 = y + 20",
            "COMPUTE r3 = z + 30"
        ]
    );
    // the DO REPEAT spans the lines up to its END REPEAT
    assert_eq!(
        (result.commands[0].first_line, result.commands[0].last_line),
        (1, 3)
    );
}

#[test]
fn do_repeat_errors() {
    let mut session = session("1 2 3");
    assert_eq!(
        error(
            &mut session,
            "DO REPEAT a = x y / b = 1 TO 3.\nCOMPUTE a = b.\nEND REPEAT."
        ),
        "line 1: DO REPEAT: the replacement lists of DO REPEAT differ in length: a has 2 items, b has 3"
    );
    assert_eq!(
        error(
            &mut session,
            "DO REPEAT a = x new1.\nCOMPUTE a = 1.\nEND REPEAT."
        ),
        "line 1: DO REPEAT: the list of a mixes existing and new variables; give one kind"
    );
    let message = error(
        &mut session,
        "DO REPEAT a = x y.\nCOMPUTE a = .\nEND REPEAT.",
    );
    assert!(message.starts_with("line 2, column 13: "), "{message}");
    let message = error(&mut session, "DO REPEAT a = x y.\nCOMPUTE a = 1.");
    assert_eq!(
        message,
        "line 1, column 1: DO REPEAT without END REPEAT; hint: end the commands to repeat with END REPEAT"
    );
    let message = error(&mut session, "END REPEAT.");
    assert!(
        message.contains("END REPEAT without DO REPEAT"),
        "{message}"
    );
}
