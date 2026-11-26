//! KEYED DATA LIST on made-up files: a keyed file of parts, a direct-access file of rates, a
//! sample of every third record in an input program, an update of existing variables, an
//! encoding. The expected values are read off the file lines with R (substr, startsWith and
//! readLines) as the rules say.

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::{RunResult, Session};
use std::path::PathBuf;

fn folder(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-keyed-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn ran(dir: &std::path::Path, source: &str) -> (Session, RunResult) {
    let mut session = Session::default();
    let source = format!("CD '{}'.\n{source}", dir.display());
    let result = session
        .run(&source, None)
        .unwrap_or_else(|errors| panic!("{errors:?}"));
    (session, result)
}

fn fails(dir: &std::path::Path, source: &str) -> String {
    let mut session = Session::default();
    let source = format!("CD '{}'.\n{source}", dir.display());
    match session.run(&source, None) {
        Ok(_) => panic!("{source} ran"),
        Err(errors) => errors[0].message.clone(),
    }
}

fn numbers(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => v.iter().collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

fn strings(data: &Dataset, name: &str) -> Vec<String> {
    match data.variable(name).unwrap().values() {
        Values::String(v) => v.iter().map(|s| s.trim_end().to_string()).collect(),
        Values::Numeric(_) => panic!("{name} is numeric"),
    }
}

/// A keyed file of parts: the code in columns 1-4, the name in 6-15 and the price in 17-21.
const PARTS: &str = "A100 Hex bolt   00.45
A200 Washer     00.05
B150 Wing nut   00.30
C900 Spanner    12.80
";

/// A direct-access file of rates by region number: the rate in columns 1-4 and a name in 6-12.
const RATES: &str = "0.21 North
0.19 South
0.25 East
0.18 West
0.23 Centre
0.20 Islands
";

#[test]
fn a_string_key_picks_the_record_that_begins_with_it() {
    let dir = folder("parts");
    std::fs::write(dir.join("parts.txt"), PARTS).unwrap();
    let (session, _) = ran(
        &dir,
        "FILE HANDLE parts /NAME='parts.txt'.\n\
         DATA LIST LIST /order (F2) code (A4).\n\
         BEGIN DATA\n1 'B150'\n2 'A100'\n3 'Z999'\n4 'C900'\n5 'A100'\nEND DATA.\n\
         KEYED DATA LIST FILE=parts KEY=code IN=known NOTABLE /part 6-15 (A) price 17-21 (F,2).\n\
         EXECUTE.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(
        strings(data, "part"),
        ["Wing nut", "Hex bolt", "", "Spanner", "Hex bolt"]
    );
    assert_eq!(
        numbers(data, "price"),
        [Some(0.3), Some(0.45), None, Some(12.8), Some(0.45)]
    );
    assert_eq!(
        numbers(data, "known"),
        [Some(1.0), Some(1.0), Some(0.0), Some(1.0), Some(1.0)]
    );
    // the display format has room for the decimal point
    assert_eq!(data.variable("price").unwrap().format(), "F6.2");
}

#[test]
fn a_numeric_key_is_the_number_of_the_record() {
    let dir = folder("rates");
    std::fs::write(dir.join("rates.txt"), RATES).unwrap();
    let (session, _) = ran(
        &dir,
        "FILE HANDLE rates /NAME='rates.txt'.\n\
         DATA LIST LIST /shop (F2) region (F2).\n\
         BEGIN DATA\n1 3\n2 5\n3 9\n4 .\n5 1.5\n6 0\n7 6\nEND DATA.\n\
         KEYED DATA LIST FILE=rates KEY=region IN=ok /rate 1-4 (F,2) place 6-12 (A).\n\
         EXECUTE.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(
        numbers(data, "rate"),
        [Some(0.25), Some(0.23), None, None, None, None, Some(0.2)]
    );
    assert_eq!(strings(data, "place")[0], "East");
    assert_eq!(strings(data, "place")[6], "Islands");
    // a missing key, a number that is not whole and one out of range find no record
    assert_eq!(
        numbers(data, "ok"),
        [
            Some(1.0),
            Some(1.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(1.0)
        ]
    );
}

#[test]
fn a_variable_that_exists_is_given_the_record_where_there_is_one() {
    let dir = folder("update");
    std::fs::write(dir.join("rates.txt"), RATES).unwrap();
    let (session, _) = ran(
        &dir,
        "FILE HANDLE rates /NAME='rates.txt'.\n\
         DATA LIST LIST /shop (F2) region (F2) rate (F4.2).\n\
         BEGIN DATA\n1 2 -1\n2 8 -1\nEND DATA.\n\
         KEYED DATA LIST FILE=rates KEY=region IN=ok /rate 1-4 (F,2).\n\
         EXECUTE.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(numbers(data, "rate"), [Some(0.19), Some(-1.0)]);
}

#[test]
fn a_scratch_key_and_flag_select_the_cases_that_have_a_record() {
    let dir = folder("scratch");
    std::fs::write(dir.join("parts.txt"), PARTS).unwrap();
    let (session, _) = ran(
        &dir,
        "FILE HANDLE parts /NAME='parts.txt'.\n\
         DATA LIST LIST /order (F2) qty (F3).\n\
         BEGIN DATA\n1 10\n2 5\n3 7\n4 12\nEND DATA.\n\
         STRING #key (A4).\n\
         DO IF qty >= 10.\n\
         COMPUTE #key = 'C900'.\n\
         ELSE IF qty = 5.\n\
         COMPUTE #key = 'A200'.\n\
         ELSE.\n\
         COMPUTE #key = 'Q000'.\n\
         END IF.\n\
         KEYED DATA LIST FILE=parts KEY=#key IN=#found /price 17-21 (F,2).\n\
         SELECT IF #found.\n\
         EXECUTE.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(numbers(data, "order"), [Some(1.0), Some(2.0), Some(4.0)]);
    assert_eq!(numbers(data, "price"), [Some(12.8), Some(0.05), Some(12.8)]);
}

#[test]
fn an_input_program_reads_every_third_record_by_number() {
    let dir = folder("sample");
    std::fs::write(dir.join("rates.txt"), RATES).unwrap();
    let (session, _) = ran(
        &dir,
        "FILE HANDLE rates /NAME='rates.txt'.\n\
         INPUT PROGRAM.\n\
         COMPUTE #next = #next + 3.\n\
         DO IF #next > 8.\n\
         END FILE.\n\
         ELSE.\n\
         KEYED DATA LIST FILE=rates KEY=#next IN=#there NOTABLE /rate 1-4 (F,2) place 6-12 (A).\n\
         DO IF #there.\n\
         END CASE.\n\
         END IF.\n\
         END IF.\n\
         END INPUT PROGRAM.\n\
         EXECUTE.",
    );
    let data = session.dataset().unwrap();
    // records 3 and 6
    assert_eq!(numbers(data, "rate"), [Some(0.25), Some(0.2)]);
    assert_eq!(strings(data, "place"), ["East", "Islands"]);
}

#[test]
fn an_encoding_decodes_the_file() {
    let dir = folder("encoding");
    // Windows-1252: 0xE9 is e with an acute accent
    std::fs::write(dir.join("names.txt"), b"1 Ren\xe9e\n2 Zo\xeb\n").unwrap();
    let (session, _) = ran(
        &dir,
        "DATA LIST LIST /n (F1).\nBEGIN DATA\n2\n1\nEND DATA.\n\
         KEYED DATA LIST FILE='names.txt' KEY=n IN=ok ENCODING='1252' /who 3-8 (A).\nEXECUTE.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(strings(data, "who"), ["Zo\u{eb}", "Ren\u{e9}e"]);
}

#[test]
fn what_the_command_needs_is_checked() {
    let dir = folder("errors");
    std::fs::write(dir.join("rates.txt"), RATES).unwrap();
    let head = "DATA LIST LIST /region (F2) tag (A2).\nBEGIN DATA\n1 'a'\nEND DATA.\n";
    for (command, wanted) in [
        (
            "KEYED DATA LIST KEY=region IN=ok /rate 1-4.",
            "needs FILE=handle",
        ),
        (
            "KEYED DATA LIST FILE='rates.txt' IN=ok /rate 1-4.",
            "needs KEY=variable",
        ),
        (
            "KEYED DATA LIST FILE='rates.txt' KEY=region /rate 1-4.",
            "needs IN=variable",
        ),
        (
            "KEYED DATA LIST FILE='rates.txt' KEY=nothere IN=ok /rate 1-4.",
            "the key variable nothere does not exist",
        ),
        (
            "KEYED DATA LIST FILE='rates.txt' KEY=region IN=ok /tag 1-4.",
            "tag is a string in the dataset and a number in the record",
        ),
        (
            "KEYED DATA LIST FILE='rates.txt' KEY=region IN=region /rate 1-4.",
            "other than the IN variable",
        ),
        (
            "KEYED DATA LIST FILE='missing.txt' KEY=region IN=ok /rate 1-4.\nEXECUTE.",
            "cannot read missing.txt",
        ),
    ] {
        let message = fails(&dir, &format!("{head}{command}"));
        assert!(message.contains(wanted), "{command}: {message}");
    }
}

#[test]
fn inside_a_loop_outside_an_input_program_it_is_refused() {
    let dir = folder("loop");
    std::fs::write(dir.join("rates.txt"), RATES).unwrap();
    let message = fails(
        &dir,
        "DATA LIST LIST /region (F2).\nBEGIN DATA\n1\nEND DATA.\n\
         LOOP #i = 1 TO 2.\n\
         KEYED DATA LIST FILE='rates.txt' KEY=region IN=ok /rate 1-4 (F,2).\n\
         END LOOP.\nEXECUTE.",
    );
    assert!(message.contains("cannot"), "{message}");
}
