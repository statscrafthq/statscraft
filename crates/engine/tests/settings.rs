//! SET settings that change results: BLANKS, EPOCH, FUZZBITS and SCALEMIN, on made-up data.
//! Date values are seconds from 14 October 1582; the expected ones are worked out by hand.

use statscraft_data::{Dataset, Measure};
use statscraft_engine::session::Session;

const DAY: f64 = 86_400.0;

fn run(source: &str) -> Session {
    let mut session = Session::default();
    session
        .run(source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    session
}

fn numbers(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    let variable = data.variable(name).unwrap();
    variable.numeric().unwrap().iter().collect()
}

/// The days of a date variable's values, from 14 October 1582.
fn days(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    let values = numbers(data, name).into_iter();
    values.map(|v| v.map(|v| v / DAY)).collect()
}

fn error(source: &str) -> String {
    let errors = Session::default().run(source, None).unwrap_err();
    errors[0].message.clone()
}

#[test]
fn blanks_gives_completely_blank_numeric_fields_its_value() {
    let session = run("SET BLANKS=-9.\n\
         DATA LIST FIXED /x 1-3 d 4-14 (DATE) s 15-16 (A).\n\
         BEGIN DATA\n  1 1-JAN-2001ab\n\n  .\nEND DATA.\n\
         DATASET NAME fixed.\n\
         DATA LIST LIST (',') /a b c.\n\
         BEGIN DATA\n1,,3\n4, ,6\n7\nEND DATA.\n\
         DATASET NAME list.\n\
         DATA LIST FREE /a b.\n\
         BEGIN DATA\n1,,3,4\nEND DATA.\n\
         COMPUTE n = NUMBER('   ', F3).\n\
         COMPUTE m = NUMBER(' . ', F3).\n\
         DATASET NAME free.");
    let mut session = session;
    session.run("DATASET ACTIVATE fixed.", None).unwrap();
    let data = session.dataset().unwrap();
    // a lone period is not blank: it stays system-missing
    assert_eq!(numbers(data, "x"), [Some(1.0), Some(-9.0), None]);
    // dates are numeric variables too, and columns past the end of the line are blank
    assert_eq!(
        days(data, "d"),
        [Some(152_751.0), Some(-9.0 / DAY), Some(-9.0 / DAY)]
    );
    let s = data.variable("s").unwrap();
    assert_eq!(s.string().unwrap().get(1).trim(), "");
    session.run("DATASET ACTIVATE list.", None).unwrap();
    let data = session.dataset().unwrap();
    assert_eq!(numbers(data, "b"), [Some(-9.0), Some(-9.0), None]);
    // a value missing at the end of a line is not a blank field
    assert_eq!(numbers(data, "c"), [Some(3.0), Some(6.0), None]);
    session.run("DATASET ACTIVATE free.", None).unwrap();
    let data = session.dataset().unwrap();
    assert_eq!(numbers(data, "b"), [Some(-9.0), Some(4.0)]);
    assert_eq!(numbers(data, "n"), [Some(-9.0), Some(-9.0)]);
    assert_eq!(numbers(data, "m"), [None, None]);
    // BLANKS is no missing value
    assert!(!data.variable("b").unwrap().is_user_missing(-9.0));
}

#[test]
fn blanks_applies_to_get_data_and_input_programs() {
    let dir = std::env::temp_dir().join(format!("statscraft-settings-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "1,,3\n4,  ,6\n").unwrap();
    let mut session = run(&format!(
        "SET BLANKS=0.\nCD '{}'.\n\
         GET DATA /TYPE=TXT /FILE='a.txt' /DELIMITERS=\",\" /VARIABLES=a F1 b F1 c F1.",
        dir.display()
    ));
    let data = session.dataset().unwrap();
    assert_eq!(numbers(data, "b"), [Some(0.0), Some(0.0)]);
    session
        .run(
            "INPUT PROGRAM.\nDATA LIST FIXED /x 1-2.\nEND INPUT PROGRAM.\n\
             BEGIN DATA\n 5\n\nEND DATA.",
            None,
        )
        .unwrap();
    assert_eq!(
        numbers(session.dataset().unwrap(), "x"),
        [Some(5.0), Some(0.0)]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn epoch_sets_the_century_of_two_digit_years() {
    let session = run("SET EPOCH=1900.\n\
         DATA LIST LIST /d (DATE11) a (ADATE8).\n\
         BEGIN DATA\n1-JAN-05 01/01/85\nEND DATA.\n\
         COMPUTE f = DATE.DMY(1,1,85).\n\
         COMPUTE g = NUMBER('1-JAN-05', DATE9).\n\
         COMPUTE m = DATE.MOYR(2,30).\n\
         COMPUTE r = DATE.YRDAY(5,32).\n\
         COMPUTE y = YRMODA(85,1,1).");
    let data = session.dataset().unwrap();
    assert_eq!(days(data, "d"), [Some(117_687.0)]); // 1 January 1905
    assert_eq!(days(data, "a"), [Some(146_907.0)]); // 1 January 1985
    assert_eq!(days(data, "f"), [Some(146_907.0)]);
    assert_eq!(days(data, "g"), [Some(117_687.0)]);
    assert_eq!(days(data, "m"), [Some(126_849.0)]); // 1 February 1930
    assert_eq!(days(data, "r"), [Some(117_718.0)]); // 1 February 1905
    // YRMODA keeps its own rule: 0 to 99 are in the 1900s
    assert_eq!(numbers(data, "y"), [Some(146_907.0)]);

    let session = run("SET EPOCH=1980.\n\
         DATA LIST LIST /d (DATE11) a (ADATE8).\n\
         BEGIN DATA\n1-JAN-05 01/01/85\nEND DATA.\n\
         COMPUTE f = DATE.DMY(1,1,79).\n\
         COMPUTE g = NUMBER('1-JAN-80', DATE9).\n\
         COMPUTE y = YRMODA(5,1,1).");
    let data = session.dataset().unwrap();
    assert_eq!(days(data, "d"), [Some(154_212.0)]); // 1 January 2005
    assert_eq!(days(data, "a"), [Some(146_907.0)]);
    assert_eq!(days(data, "f"), [Some(181_240.0)]); // 1 January 2079
    assert_eq!(days(data, "g"), [Some(145_080.0)]); // 1 January 1980
    assert_eq!(numbers(data, "y"), [Some(117_687.0)]);
}

#[test]
fn epoch_applies_to_get_data_dates() {
    let dir = std::env::temp_dir().join(format!("statscraft-epoch-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("d.txt"), "1/1/05\n").unwrap();
    let session = run(&format!(
        "SET EPOCH=1900.\nCD '{}'.\n\
         GET DATA /TYPE=TXT /FILE='d.txt' /DELIMITERS=\",\" /VARIABLES=d ADATE8.",
        dir.display()
    ));
    assert_eq!(days(session.dataset().unwrap(), "d"), [Some(117_687.0)]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn fuzzbits_is_the_default_slack_of_rnd_and_trunc() {
    // 0.285 * 100 is 28.499999999999996 and 0.29 * 100 is 28.999999999999996
    let source = "DATA LIST LIST /x.\nBEGIN DATA\n0.285\n0.29\nEND DATA.\n\
                  COMPUTE r = RND(x * 100).\nCOMPUTE t = TRUNC(x * 100).\n\
                  COMPUTE r6 = RND(x * 100, 1, 6).\nCOMPUTE t6 = TRUNC(x * 100, 1, 6).";
    let session = run(source);
    let data = session.dataset().unwrap();
    assert_eq!(numbers(data, "r"), [Some(29.0), Some(29.0)]);
    assert_eq!(numbers(data, "t"), [Some(28.0), Some(29.0)]);
    let session = run(&format!("SET FUZZBITS=0.\n{source}"));
    let data = session.dataset().unwrap();
    assert_eq!(numbers(data, "r"), [Some(28.0), Some(29.0)]);
    assert_eq!(numbers(data, "t"), [Some(28.0), Some(28.0)]);
    // a third argument still wins over the setting
    assert_eq!(numbers(data, "r6"), [Some(29.0), Some(29.0)]);
    assert_eq!(numbers(data, "t6"), [Some(28.0), Some(29.0)]);
}

#[test]
fn scalemin_sets_how_many_values_make_a_new_variable_scale() {
    let source = "DATA LIST LIST /a b.\nBEGIN DATA\n1 1\n2 2\n3 2\nEND DATA.";
    let measures = |session: &Session| {
        let data = session.dataset().unwrap();
        ["a", "b"].map(|name| data.variable(name).unwrap().measure())
    };
    assert_eq!(measures(&run(source)), [Measure::Nominal, Measure::Nominal]);
    let session = run(&format!("SET SCALEMIN=3.\n{source}"));
    assert_eq!(measures(&session), [Measure::Scale, Measure::Nominal]);
    let session = run(&format!("SET SCALEMIN=2.\n{source}"));
    assert_eq!(measures(&session), [Measure::Scale, Measure::Scale]);
}

#[test]
fn settings_that_change_results_take_only_valid_values() {
    assert_eq!(
        error("SET BLANKS=x."),
        "SET BLANKS takes a number or SYSMIS, got x"
    );
    assert_eq!(
        error("SET EPOCH=99."),
        "SET EPOCH takes a year from 1582 to 9999 or AUTOMATIC, got 99"
    );
    assert_eq!(
        error("SET FUZZBITS=21."),
        "SET FUZZBITS takes a whole number from 0 to 20, got 21"
    );
    assert_eq!(
        error("SET SCALEMIN=0."),
        "SET SCALEMIN takes a whole number from 1, got 0"
    );
    // the defaults come back by name, without a warning
    let mut session = run("SET BLANKS=1 EPOCH=1900.");
    let report = session
        .run(
            "SET BLANKS=SYSMIS EPOCH=AUTOMATIC FUZZBITS=6 SCALEMIN=24.",
            None,
        )
        .unwrap();
    let warnings: Vec<&String> = report.commands.iter().flat_map(|c| &c.warnings).collect();
    assert!(warnings.is_empty(), "{warnings:?}");
}
