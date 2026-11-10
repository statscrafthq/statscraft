//! Macros end to end: a table library brought in with INCLUDE, its macros called on data, and
//! the session commands such libraries use. All syntax and data here are made up for the test.

use statscraft_engine::session::{RunResult, Session};
use statscraft_engine::{Table, cell_text};

const DATA: &str = "\
DATA LIST LIST /q1 q2 grp age.
BEGIN DATA
1 2 1 23
2 1 1 35
1 1 2 41
2 2 2 55
1 2 1 62
2 1 2 19
1 1 1 33
2 2 2 47
END DATA.
VALUE LABELS q1 q2 1 'Yes' 2 'No' / grp 1 'North' 2 'South'.
VARIABLE LEVEL q1 q2 grp (NOMINAL).
";

/// A library in the style of real table libraries: positional arguments ended by '-', a
/// default, a loop that walks a variable list with !HEAD and !TAIL and builds a CTABLES
/// axis with !CONCAT, an !IF on a flag, a caption macro and a filter macro over !CMDEND.
const LIBRARY: &str = "\
* A made-up table library.
DEFINE !caption()
  'All respondents'.
!ENDDEFINE.

define tab (!positional !charend('-') / !positional !charend('-')
  /!positional !default('Y') !tokens(1)).
!let !stat = ' [COUNT F40.0]'
!if (!3 = 'Y' | !3 = 'y') !then !let !stat = \" [COUNT F40.0, COLPCT.COUNT '%' PCT40.0]\" !ifend
!let !rows = !concat('total', !stat)
!let !rest = !1
!do !i = 1 !to 50
!if (!length(!rest) = 0) !then !break !ifend
!let !rows = !concat(!rows, ' + ', !head(!rest), !stat)
!let !rest = !tail(!rest)
!doend
CTABLES
  /TABLE !rows BY !2
  /TITLES CAPTION= !caption
  /CATEGORIES VARIABLES=!2 EMPTY=INCLUDE.
!ENDDEFINE.

DEFINE !only (!POSITIONAL !CMDEND)
USE ALL.
COMPUTE keep = (!1).
FILTER BY keep.
EXECUTE.
!ENDDEFINE.
";

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-macros-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(session: &mut Session, source: &str) -> RunResult {
    session
        .run(source, None)
        .unwrap_or_else(|errors| panic!("{errors:?}"))
}

fn tables(result: &RunResult) -> Vec<&Table> {
    result.commands.iter().flat_map(|c| &c.tables).collect()
}

fn row(table: &Table, r: usize) -> String {
    let cells: Vec<String> = (0..table.columns.len())
        .map(|c| cell_text(table, r, c))
        .collect();
    cells.join("|")
}

#[test]
fn a_table_library_runs_from_include_to_tables() {
    let dir = temp_dir("library");
    std::fs::write(dir.join("tables.sps"), LIBRARY).unwrap();
    let mut session = Session::default();
    session.set_base_dir(Some(dir.clone()));
    run(&mut session, DATA);
    let result = run(
        &mut session,
        "INCLUDE FILE='tables.sps'.\n\
         COMPUTE total = 1.\n\
         VARIABLE LEVEL total (NOMINAL).\n\
         tab q1 q2 - grp -.\n\
         tab q1 - grp - n.\n\
         !only age < 50.\n\
         tab q1 - grp -.\n\
         FILTER OFF.",
    );
    let tables = tables(&result);
    assert_eq!(tables.len(), 3);
    // counts and column percentages of q1 and q2 by grp, with the total row first
    let first = tables[0];
    assert_eq!(first.caption.as_deref(), Some("All respondents"));
    assert_eq!(row(first, 0), "total|1.00|4|100%|4|100%");
    assert_eq!(row(first, 1), "q1|Yes|3|75%|1|25%");
    assert_eq!(row(first, 4), "q2|No|2|50%|2|50%");
    // 'n' leaves the percentages out
    assert_eq!(tables[1].columns.len(), 2 + 2);
    // the filter macro keeps the 6 cases below 50, 3 in each group
    assert_eq!(row(tables[2], 0), "total|1.00|3|100%|3|100%");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn settings_are_kept_shown_preserved_and_restored() {
    let mut session = Session::default();
    run(&mut session, DATA);
    let result = run(
        &mut session,
        "SET FORMAT=F5.1 TNUMBERS=BOTH LOCALE='en_UK'.\n\
         COMPUTE y = q1 * 2.\n\
         PRESERVE LOCALE.\n\
         SET FORMAT F3.0.\n\
         COMPUTE z = 1.\n\
         RESTORE.\n\
         SHOW FORMAT LOCALE N.\n\
         CACHE.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(data.variable("y").unwrap().format(), "F5.1");
    assert_eq!(data.variable("z").unwrap().format(), "F3.0");
    assert_eq!(data.variable("q1").unwrap().format(), "F8.2");
    let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings[0].contains("TNUMBERS only changes how output looks"));
    assert!(warnings[1].contains("LOCALE ignored"));
    let show = tables(&result)[0];
    assert_eq!(row(show, 0), "FORMAT|F5.1");
    assert_eq!(row(show, 1), "LOCALE|en_UK");
    assert_eq!(row(show, 2), "N|8");
    let error = session.run("SET FORMAT=A8.", None).unwrap_err();
    assert!(error[0].message.contains("simple F format"));
    let error = session.run("SET WIBBLE=1.", None).unwrap_err();
    assert_eq!(error[0].message, "unknown SET subcommand WIBBLE");
}

#[test]
fn named_datasets_stay_open_while_another_is_active() {
    let mut session = Session::default();
    run(&mut session, DATA);
    run(
        &mut session,
        "DATASET NAME whole.\n\
         DATASET COPY north.\n\
         DATASET ACTIVATE north.\n\
         SELECT IF grp = 1.\n\
         DATASET DECLARE later.",
    );
    assert_eq!(session.dataset().unwrap().case_count(), 4);
    let result = run(&mut session, "DATASET ACTIVATE whole.\nDATASET DISPLAY.");
    assert_eq!(session.dataset().unwrap().case_count(), 8);
    let list = tables(&result)[0];
    assert_eq!(row(list, 0), "whole|8|4|yes");
    assert_eq!(row(list, 1), "later|||declared");
    assert_eq!(row(list, 2), "north|4|4|");
    // a new data source parks the named active dataset
    run(
        &mut session,
        "DATA LIST LIST /x.\nBEGIN DATA\n1\nEND DATA.\nDATASET NAME small.",
    );
    run(&mut session, "DATASET ACTIVATE north.");
    assert_eq!(session.dataset().unwrap().case_count(), 4);
    let error = session.run("DATASET ACTIVATE later.", None).unwrap_err();
    assert!(error[0].message.contains("holds no data yet"));
    let error = session.run("DATASET ACTIVATE nothing.", None).unwrap_err();
    assert!(error[0].message.contains("no dataset named nothing"));
    run(&mut session, "DATASET CLOSE ALL.");
    assert!(session.run("DATASET ACTIVATE small.", None).is_err());
}

#[test]
fn file_commands_take_dataset_names() {
    let mut session = Session::default();
    run(&mut session, DATA);
    run(
        &mut session,
        "DATASET NAME whole.\n\
         DATASET DECLARE counts.\n\
         AGGREGATE /OUTFILE=counts /BREAK=grp /n = N.\n\
         DATA LIST LIST /grp code.\nBEGIN DATA\n1 10\n2 20\nEND DATA.\n\
         DATASET NAME codes.\n\
         DATASET ACTIVATE whole.\n\
         SORT CASES BY grp.\n\
         MATCH FILES /FILE=* /TABLE='codes' /BY grp.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(data.case_count(), 8);
    let code = data.variable("code").unwrap();
    let codes: Vec<Option<f64>> = code.numeric().unwrap().iter().collect();
    assert_eq!(codes[..4], [Some(10.0); 4]);
    assert_eq!(codes[4..], [Some(20.0); 4]);
    // the declared dataset got the aggregated cases; ADD FILES reads it by name, unquoted
    run(
        &mut session,
        "ADD FILES /FILE=counts /FILE=counts.\nDATASET NAME both.",
    );
    assert_eq!(session.dataset().unwrap().case_count(), 4);
    let error = session
        .run("AGGREGATE /OUTFILE=both /BREAK=grp /m = N.", None)
        .unwrap_err();
    assert!(error[0].message.contains("OUTFILE=*"), "{error:?}");
}

#[test]
fn display_and_list_show_the_dictionary_and_the_cases() {
    let mut session = Session::default();
    run(&mut session, DATA);
    let result = run(
        &mut session,
        "VARIABLE LABELS age 'Age'.\n\
         MISSING VALUES age (99).\n\
         DISPLAY SORTED LABELS /VARIABLES=q2 age.\n\
         DISPLAY DICTIONARY /VARIABLES=age.\n\
         LIST VARIABLES=grp age /CASES=FROM 2 TO 6 BY 2 /FORMAT=NUMBERED.\n\
         USE ALL.",
    );
    let tables = tables(&result);
    assert_eq!(row(tables[0], 0), "age|4|Age");
    assert_eq!(row(tables[0], 1), "q2|2|");
    assert_eq!(
        row(tables[1], 0),
        "age|4|Age|scale|input||right|F8.2|F8.2|99"
    );
    let list = tables[2];
    assert_eq!(list.columns, ["Case", "grp", "age"]);
    assert_eq!(row(list, 0), "2|1.00|35.00");
    assert_eq!(row(list, 2), "6|2.00|19.00");
    assert_eq!(list.rows.len(), 3);
}
