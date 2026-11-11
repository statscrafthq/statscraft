//! TITLE, SUBTITLE, ECHO, NEW FILE, CLEAR TRANSFORMATIONS, ERASE, SYSFILE INFO, PERMISSIONS,
//! HOST, DATE, PRINT, PRINT EJECT, PRINT SPACE, WRITE, PRINT FORMATS, WRITE FORMATS and XSAVE,
//! on made-up data.

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::{RunResult, Session};
use statscraft_engine::{Cell, Table};
use std::path::PathBuf;

const DATA: &str = "\
DATA LIST LIST /id (F3) name (A6) score (F6.2).
BEGIN DATA
1 Ann 12.5
2 Bob -3
3 Cy .
END DATA.
";

fn session() -> Session {
    let mut session = Session::default();
    run(&mut session, DATA);
    session
}

fn run(session: &mut Session, source: &str) -> RunResult {
    session
        .run(source, None)
        .unwrap_or_else(|errors| panic!("{errors:?}"))
}

fn fails(session: &mut Session, source: &str) -> String {
    match session.run(source, None) {
        Ok(_) => panic!("{source} ran"),
        Err(errors) => errors[0].message.clone(),
    }
}

fn tables(result: &RunResult) -> Vec<&Table> {
    result.commands.iter().flat_map(|c| &c.tables).collect()
}

fn text(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => n.to_string(),
        Cell::Empty => String::new(),
    }
}

/// The pages of text output, each as its lines.
fn pages(result: &RunResult) -> Vec<Vec<String>> {
    tables(result)
        .into_iter()
        .filter(|t| t.title == "Text output")
        .map(|t| t.rows.iter().map(|r| text(&r[0])).collect())
        .collect()
}

fn numbers(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => v.iter().collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

fn strings(data: &Dataset, name: &str) -> Vec<String> {
    match data.variable(name).unwrap().values() {
        Values::String(v) => v.iter().map(str::to_string).collect(),
        Values::Numeric(_) => panic!("{name} is numeric"),
    }
}

fn folder(case: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-output-{}-{case}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn titles_and_echo_show_where_they_stand() {
    let mut s = session();
    let long = "x".repeat(70);
    let result = run(
        &mut s,
        &format!("TITLE 'Wave ''2'''.\nSUBTITLE ' '.\nECHO 'a' + 'b'.\nTITLE {long}."),
    );
    let shown: Vec<(String, String)> = tables(&result)
        .iter()
        .map(|t| (t.title.clone(), text(&t.rows[0][0])))
        .collect();
    assert_eq!(
        shown,
        [
            ("Title".into(), "Wave '2'".into()),
            ("Echo".into(), "ab".into()),
            ("Title".into(), "x".repeat(60)),
        ]
    );
}

#[test]
fn new_file_empties_the_active_dataset_and_keeps_a_named_one() {
    let mut s = session();
    run(&mut s, "DATASET NAME first.\nNEW FILE.");
    assert!(s.dataset().is_none());
    let (named, active) = s.named_datasets();
    assert_eq!(named.len(), 1);
    assert_eq!(named[0].0, "first");
    assert_eq!(active, None);
    run(&mut s, "DATASET ACTIVATE first.");
    assert_eq!(s.dataset().unwrap().case_count(), 3);
    // the pending transformations never run
    let result = run(&mut s, "PRINT /id.\nNEW FILE.");
    assert!(pages(&result).is_empty());
}

#[test]
fn clear_transformations_goes_back_to_the_last_data_pass() {
    let mut s = session();
    run(
        &mut s,
        "COMPUTE kept = id * 2.\nFREQUENCIES id.\nCOMPUTE gone = 1.\n\
         VARIABLE LABELS id 'Number'.\nRECODE kept (2 = 20).\nTEMPORARY.\nSELECT IF id = 1.\n\
         PRINT /id.\nCLEAR TRANSFORMATIONS.\nLIST.",
    );
    let data = s.dataset().unwrap();
    assert_eq!(data.case_count(), 3);
    assert!(data.variable("gone").is_err());
    assert_eq!(data.variable("id").unwrap().label(), None);
    assert_eq!(numbers(data, "kept"), [Some(2.0), Some(4.0), Some(6.0)]);
    let result = run(&mut s, "CLEAR TRANSFORMATIONS.");
    assert_eq!(
        result.commands[0].warnings,
        ["there are no transformations to clear"]
    );
    // the end of a run is a data pass
    run(&mut s, "COMPUTE later = 1.");
    run(&mut s, "CLEAR TRANSFORMATIONS.");
    assert!(s.dataset().unwrap().variable("later").is_ok());
}

#[test]
fn erase_and_permissions_work_on_files() {
    let dir = folder("files");
    let file = dir.join("old.txt");
    std::fs::write(&file, "x").unwrap();
    let mut s = Session::default();
    let path = file.display();
    run(
        &mut s,
        &format!("PERMISSIONS FILE='{path}' /PERMISSIONS READONLY."),
    );
    assert!(std::fs::metadata(&file).unwrap().permissions().readonly());
    run(
        &mut s,
        &format!("PERMISSIONS FILE='{path}' /PERMISSIONS WRITEABLE."),
    );
    assert!(!std::fs::metadata(&file).unwrap().permissions().readonly());
    run(&mut s, &format!("ERASE FILE='{path}'."));
    assert!(!file.exists());
    assert!(fails(&mut s, &format!("ERASE FILE='{path}'.")).ends_with("no such file"));
    assert!(
        fails(
            &mut s,
            &format!("PERMISSIONS FILE='{path}' /PERMISSIONS READONLY.")
        )
        .ends_with("no such file")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn host_is_refused() {
    let mut s = Session::default();
    let message = fails(&mut s, "HOST COMMAND=['echo hi'].");
    assert!(message.starts_with("HOST is refused"), "{message}");
}

#[test]
fn sysfile_info_shows_a_file_dictionary_without_opening_it() {
    let dir = folder("sysfile");
    let file = dir.join("info.sav");
    let mut s = session();
    run(
        &mut s,
        &format!(
            "FILE LABEL Wave two.\nVALUE LABELS id 1 'one'.\nSAVE OUTFILE='{}'.\nNEW FILE.",
            file.display()
        ),
    );
    let result = run(&mut s, &format!("SYSFILE INFO '{}'.", file.display()));
    assert!(s.dataset().is_none());
    let tables = tables(&result);
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "File information",
            "Variable information",
            "Variable values"
        ]
    );
    let info: Vec<(String, String)> = tables[0]
        .rows
        .iter()
        .map(|r| (text(&r[0]), text(&r[1])))
        .collect();
    assert!(info.contains(&("Label".into(), "Wave two".into())));
    assert!(info.contains(&("Cases".into(), "3".into())));
    assert!(info.contains(&("Variables".into(), "3".into())));
    assert_eq!(tables[1].rows.len(), 3);
    std::fs::remove_dir_all(dir).unwrap();
}

fn dates(source: &str, cases: usize) -> (Dataset, Vec<(String, String)>) {
    let mut s = Session::default();
    let mut data = String::from("DATA LIST LIST /x.\nBEGIN DATA\n");
    for case in 0..cases {
        data.push_str(&format!("{case}\n"));
    }
    data.push_str("END DATA.\n");
    let result = run(&mut s, &format!("{data}{source}"));
    let report = tables(&result)
        .last()
        .unwrap()
        .rows
        .iter()
        .map(|r| (text(&r[0]), text(&r[1])))
        .collect();
    (s.dataset().unwrap().clone(), report)
}

#[test]
fn date_counts_years_and_months() {
    let (data, report) = dates("DATE Y 1960 M.", 14);
    assert_eq!(
        report,
        [
            ("YEAR_".into(), "YEAR, not periodic".into()),
            ("MONTH_".into(), "MONTH, period 12".into()),
            ("DATE_".into(), "DATE. FORMAT: \"MMM YYYY\"".into()),
        ]
    );
    let year = numbers(&data, "YEAR_");
    assert_eq!(year[11], Some(1960.0));
    assert_eq!(year[12], Some(1961.0));
    let month = numbers(&data, "MONTH_");
    assert_eq!(month[..3], [Some(1.0), Some(2.0), Some(3.0)]);
    assert_eq!(month[12..], [Some(1.0), Some(2.0)]);
    let text = strings(&data, "DATE_");
    assert_eq!(text[0], "JAN 1960");
    assert_eq!(text[13], "FEB 1961");
    assert_eq!(data.variable("YEAR_").unwrap().format(), "F4.0");
}

#[test]
fn date_infers_the_month_from_the_quarter() {
    let (data, report) = dates("DATE Y 1950 Q 2 M.", 10);
    assert_eq!(report[1], ("QUARTER_".into(), "QUARTER, period 4".into()));
    assert_eq!(
        numbers(&data, "MONTH_")[..4],
        [Some(4.0), Some(5.0), Some(6.0), Some(7.0)]
    );
    assert_eq!(
        numbers(&data, "QUARTER_")[..4],
        [Some(2.0), Some(2.0), Some(2.0), Some(3.0)]
    );
    assert_eq!(numbers(&data, "YEAR_")[9], Some(1951.0));
    assert_eq!(strings(&data, "DATE_")[9], "JAN 1951");
}

#[test]
fn date_adds_a_cycle_above_a_periodic_highest_keyword() {
    let (data, report) = dates("DATE DAY 1 5 HOUR 3 8.", 7);
    let names: Vec<&str> = report.iter().map(|r| r.0.as_str()).collect();
    assert_eq!(names, ["CYCLE_", "DAY_", "HOUR_", "DATE_"]);
    assert_eq!(report[3].1, "DATE. FORMAT: \"CCCC D H\"");
    assert_eq!(
        numbers(&data, "HOUR_"),
        [3.0, 4.0, 5.0, 6.0, 7.0, 0.0, 1.0].map(Some)
    );
    assert_eq!(
        numbers(&data, "DAY_"),
        [1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0].map(Some)
    );
    assert_eq!(strings(&data, "DATE_")[5], "   1 2 0");

    let (data, report) = dates("DATE OBS 9 17.", 10);
    assert_eq!(report[1], ("OBS_".into(), "OBS, period 17".into()));
    assert_eq!(numbers(&data, "OBS_")[8..], [Some(17.0), Some(1.0)]);
    assert_eq!(numbers(&data, "CYCLE_")[8..], [Some(1.0), Some(2.0)]);
    assert_eq!(strings(&data, "DATE_")[9], "   2  1");
}

#[test]
fn date_steps_by_and_replaces_earlier_date_variables() {
    let (data, report) = dates("DATE Y 2000.\nDATE DAY HOUR 1 24 BY 2.", 13);
    assert!(data.variable("YEAR_").is_err());
    assert_eq!(report[1], ("HOUR_".into(), "HOUR, period 24 by 2".into()));
    let hours = numbers(&data, "HOUR_");
    assert_eq!(hours[..3], [Some(1.0), Some(3.0), Some(5.0)]);
    assert_eq!(hours[11..], [Some(23.0), Some(1.0)]);
    assert_eq!(numbers(&data, "DAY_")[11..], [Some(1.0), Some(2.0)]);
    assert_eq!(strings(&data, "DATE_")[12], "   2  1");

    let (data, _) = dates("DATE W H 1 168.", 170);
    assert_eq!(numbers(&data, "HOUR_")[166..168], [Some(167.0), Some(0.0)]);
    assert_eq!(numbers(&data, "WEEK_")[166..168], [Some(1.0), Some(2.0)]);
}

#[test]
fn date_rejects_conflicts_and_bad_steps() {
    let mut s = session();
    assert!(fails(&mut s, "DATE Q 1 M 4.").contains("conflict"));
    assert!(fails(&mut s, "DATE DAY HOUR 1 24 BY 5.").contains("does not divide"));
    assert!(fails(&mut s, "DATE Y M 13.").contains("from 1 to 12"));
    assert!(fails(&mut s, "DATE DAY HOUR 24 24.").contains("from 0 to 23"));
}

#[test]
fn print_shows_each_case_when_the_data_are_read() {
    let mut s = session();
    let result = run(
        &mut s,
        "PRINT /id name score.\nWRITE /id name score.\nPRINT /id (F5.1) name * score (COMMA8.2) 'x'.\nEXECUTE.",
    );
    assert_eq!(result.commands[3].tables.len(), 1);
    assert_eq!(
        pages(&result),
        [[
            "  1 Ann      12.50 ",
            "  1Ann     12.50",
            "  1.0Ann       12.50x",
            "  2 Bob      -3.00 ",
            "  2Bob     -3.00",
            "  2.0Bob       -3.00x",
            "  3 Cy         .   ",
            "  3Cy           ",
            "  3.0Cy          .  x",
        ]]
    );
}

#[test]
fn print_places_texts_columns_and_format_lists() {
    let mut s = session();
    let result = run(
        &mut s,
        "DO IF $CASENUM = 1.\nPRINT EJECT /'Name' 1 'Score' 12.\nEND IF.\n\
         PRINT / name 1-6 score 12-18 (2) / id id (T3, F3, 2X, F3.1) * id 12.\n\
         PRINT SPACE id - 2.\nEXECUTE.",
    );
    assert_eq!(
        pages(&result),
        [vec![
            "Name       Score",
            "Ann          12.50",
            "    1  1.0 1",
            "",
            "Bob          -3.00",
            "    2  2.0 2",
            "Cy             .  ",
            "    3  3.0 3",
            "",
        ]]
    );
    assert_eq!(
        result.commands[5].warnings,
        ["the expression on PRINT SPACE gave -1; one line is printed"]
    );
}

#[test]
fn print_eject_starts_a_page_and_print_space_adds_lines() {
    let mut s = session();
    let result = run(&mut s, "PRINT EJECT /'Case' id.\nPRINT SPACE.\nLIST.");
    assert_eq!(
        pages(&result),
        [
            vec!["Case  1 ", ""],
            vec!["Case  2 ", ""],
            vec!["Case  3 ", ""],
        ]
    );
    // the pass at the end of the run shows with the last command
    let result = run(&mut s, "COMPUTE y = 1.\nPRINT /id.");
    assert_eq!(result.commands[1].tables.len(), 1);
    assert_eq!(pages(&result), [["  1 ", "  2 ", "  3 "]]);
}

#[test]
fn print_runs_case_by_case_with_scratch_values_and_loops() {
    let mut s = session();
    let result = run(
        &mut s,
        "COMPUTE #sum = #sum + id.\nPRINT /#sum (F3).\n\
         LOOP #i = 1 TO id.\nPRINT /'loop' #i (F2).\nEND LOOP.\nEXECUTE.",
    );
    assert_eq!(
        pages(&result),
        [[
            "  1", "loop 1", "  3", "loop 1", "loop 2", "  6", "loop 1", "loop 2", "loop 3",
        ]]
    );
}

#[test]
fn print_table_records_and_errors() {
    let mut s = session();
    let result = run(&mut s, "PRINT RECORDS=1 TABLE /id name /score.\nEXECUTE.");
    let layout = tables(&result)
        .into_iter()
        .find(|t| t.title == "Writing 2 records")
        .unwrap();
    let rows: Vec<Vec<String>> = layout
        .rows
        .iter()
        .map(|r| r.iter().map(text).collect())
        .collect();
    assert_eq!(
        rows,
        [
            ["id", "1", "1-3", "F3.0"],
            ["name", "1", "5-10", "A6"],
            ["score", "2", "1-7", "F7.2"],
        ]
    );
    // the warning comes with the command that reads the data
    assert_eq!(
        result.commands[1].warnings,
        ["RECORDS=1 does not match the 2 lines given; RECORDS is ignored"]
    );
    assert!(fails(&mut s, "PRINT /id name (F3).\nEXECUTE.").contains("2 variables but 1 formats"));
    assert!(fails(&mut s, "PRINT /name (F3).\nEXECUTE.").contains("give it an A format"));
    assert!(fails(&mut s, "PRINT /id score 1-5.\nEXECUTE.").contains("shared equally"));
    assert!(fails(&mut s, "PRINT /nothing.\nEXECUTE.").contains("unknown variable"));
}

#[test]
fn write_to_files_in_encodings() {
    let dir = folder("write");
    let mut s = session();
    let file = |name: &str| dir.join(name).display().to_string();
    run(
        &mut s,
        &format!(
            "WRITE OUTFILE='{0}' /id name score.\nPRINT OUTFILE='{1}' /id.\nPRINT EJECT OUTFILE='{1}' /'h'.\n\
             PRINT SPACE OUTFILE='{1}'.\nWRITE OUTFILE='{2}' ENCODING='UTF16LE' /name.\n\
             WRITE OUTFILE='{3}' ENCODING='UTF8' BOM=NO /name.\nEXECUTE.",
            file("w.txt"),
            file("p.txt"),
            file("u16.txt"),
            file("u8.txt")
        ),
    );
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
    assert_eq!(
        read("w.txt"),
        "  1Ann     12.50\n  2Bob     -3.00\n  3Cy           \n"
    );
    assert_eq!(read("p.txt"), "   1 \n1h\n \n   2 \n1h\n \n   3 \n1h\n \n");
    assert_eq!(read("u8.txt"), "Ann   \nBob   \nCy    \n");
    let bytes = std::fs::read(dir.join("u16.txt")).unwrap();
    assert_eq!(bytes[..4], [0xFF, 0xFE, b'A', 0]);
    assert_eq!(bytes.len(), 2 + 3 * 7 * 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn print_and_write_formats_set_one_format_each() {
    let dir = folder("formats");
    let mut s = session();
    let saved = dir.join("f.sav").display().to_string();
    let out = dir.join("w.txt").display().to_string();
    let result = run(
        &mut s,
        &format!(
            "PRINT FORMATS score (F5.1).\nWRITE FORMATS id (F5.1).\nDISPLAY VARIABLES.\n\
             SAVE OUTFILE='{saved}'.\nGET FILE='{saved}'.\nWRITE OUTFILE='{out}' /id score.\nEXECUTE."
        ),
    );
    let display: Vec<(String, String, String)> = tables(&result)[0]
        .rows
        .iter()
        .map(|r| (text(&r[0]), text(&r[2]), text(&r[3])))
        .collect();
    assert_eq!(
        display,
        [
            ("id".into(), "F3.0".into(), "F5.1".into()),
            ("name".into(), "A6".into(), "A6".into()),
            ("score".into(), "F5.1".into(), "F7.2".into()),
        ]
    );
    let data = s.dataset().unwrap();
    assert_eq!(data.variable("id").unwrap().format(), "F3.0");
    assert_eq!(data.variable("id").unwrap().write_format(), "F5.1");
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "  1.0  12.50\n  2.0  -3.00\n  3.0       \n"
    );
    // FORMATS sets both again
    run(&mut s, "FORMATS id (F4).");
    assert_eq!(
        s.dataset().unwrap().variable("id").unwrap().write_format(),
        "F4.0"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn xsave_writes_the_cases_as_they_pass() {
    let dir = folder("xsave");
    let file = dir.join("x.sav").display().to_string();
    let mut s = session();
    let result = run(
        &mut s,
        &format!(
            "COMPUTE #t = id * 10.\nCOMPUTE big = #t.\nXSAVE OUTFILE='{file}' /DROP=score \
             /RENAME=(name=who) /MAP.\nSELECT IF id < 3.\nEXECUTE."
        ),
    );
    let map = tables(&result)
        .into_iter()
        .find(|t| t.title == "Variables in the file")
        .unwrap();
    let rows: Vec<(String, String)> = map
        .rows
        .iter()
        .map(|r| (text(&r[0]), text(&r[1])))
        .collect();
    assert_eq!(
        rows,
        [
            ("id".into(), "id".into()),
            ("who".into(), "name".into()),
            ("big".into(), "big".into())
        ]
    );
    let saved = Dataset::read_sav(&file).unwrap();
    let names: Vec<&str> = saved.variable_names().collect();
    assert_eq!(names, ["id", "who", "big"]);
    assert_eq!(numbers(&saved, "big"), [Some(10.0), Some(20.0), Some(30.0)]);
    assert_eq!(strings(&saved, "who"), ["Ann", "Bob", "Cy"]);
    assert_eq!(s.dataset().unwrap().case_count(), 2);
    // temporary changes reach the file; the dataset keeps its own
    run(
        &mut s,
        &format!("TEMPORARY.\nCOMPUTE id = id + 100.\nXSAVE OUTFILE='{file}'.\nEXECUTE."),
    );
    let saved = Dataset::read_sav(&file).unwrap();
    assert_eq!(numbers(&saved, "id"), [Some(101.0), Some(102.0)]);
    assert_eq!(numbers(s.dataset().unwrap(), "id"), [Some(1.0), Some(2.0)]);
    assert!(
        fails(
            &mut s,
            &format!("XSAVE OUTFILE='{file}' /UNCOMPRESSED.\nEXECUTE.")
        )
        .contains("not supported yet")
    );
    std::fs::remove_dir_all(dir).unwrap();
}
