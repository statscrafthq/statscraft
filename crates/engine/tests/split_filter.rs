//! SPLIT FILE, FILTER, TEMPORARY, N OF CASES and SAMPLE. The numbers agree with the reference
//! program; `reference_procedures.rs` repeats the comparison when it is at hand.

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::{RunResult, Session};
use statscraft_engine::{Cell, Table, cell_text};

/// g x f: two runs of g = 1 around a run of g = 2; f is 0 or missing in two cases.
const DATA: &str = "\
DATA LIST LIST /g x f.
BEGIN DATA
1 10 1
1 20 0
1 30 1
2 40 1
2 50 .
2 60 1
1 70 1
END DATA.
";

fn session() -> Session {
    let mut session = Session::default();
    session.run(DATA, None).unwrap();
    session
}

fn run(session: &mut Session, source: &str) -> RunResult {
    session
        .run(source, None)
        .unwrap_or_else(|errors| panic!("{errors:?}"))
}

fn tables(result: &RunResult) -> Vec<&Table> {
    result.commands.iter().flat_map(|c| &c.tables).collect()
}

/// A row of a table as text, cells separated by `|`.
fn row(table: &Table, r: usize) -> String {
    let cells: Vec<String> = (0..table.columns.len())
        .map(|c| cell_text(table, r, c))
        .collect();
    cells.join("|")
}

fn show(data: &Dataset, name: &str) -> String {
    match data.variable(name).unwrap().values() {
        Values::Numeric(values) => values
            .iter()
            .map(|v| v.map_or(".".into(), |v| format!("{}", (v * 100.0).round() / 100.0)))
            .collect::<Vec<_>>()
            .join(" "),
        Values::String(values) => values.iter().collect::<Vec<_>>().join("|"),
    }
}

#[test]
fn a_filter_leaves_out_zero_and_missing_cases_without_deleting_them() {
    let mut session = session();
    let result = run(&mut session, "FILTER BY f.\nDESCRIPTIVES x.");
    let table = tables(&result)[0];
    assert_eq!(row(table, 0), "x||5|42|23.87|10|70");
    assert_eq!(session.dataset().unwrap().case_count(), 7);
    assert_eq!(session.dataset().unwrap().filter_variable(), Some("f"));
    let result = run(&mut session, "FILTER OFF.\nDESCRIPTIVES x.");
    assert_eq!(row(tables(&result)[0], 0), "x||7|40|21.60|10|70");
}

#[test]
fn layered_split_groups_share_one_table() {
    let mut session = session();
    let result = run(&mut session, "SPLIT FILE BY g.\nDESCRIPTIVES x.");
    let table = tables(&result)[0];
    assert_eq!(table.columns[0], "g");
    assert_eq!(table.row_headers, 3);
    // groups are runs of adjacent cases: g = 1 comes twice
    assert_eq!(row(table, 0), "1|x||3|20|10|10|30");
    assert_eq!(row(table, 1), "|Valid N (listwise)||3||||");
    assert_eq!(row(table, 3), "2|x||3|50|10|40|60");
    assert_eq!(row(table, 6), "1|x||1|70||70|70");
}

#[test]
fn separate_split_groups_get_tables_of_their_own() {
    let mut session = session();
    let result = run(
        &mut session,
        "SPLIT FILE SEPARATE BY g.\nFILTER BY f.\nFREQUENCIES x.",
    );
    let titles: Vec<&str> = tables(&result).iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, ["x (g = 1)", "x (g = 2)", "x (g = 1)"]);
    assert_eq!(tables(&result)[1].rows.len(), 3);
}

#[test]
fn a_crosstab_of_split_groups_keeps_the_columns_of_every_group() {
    let mut session = session();
    let result = run(&mut session, "SPLIT FILE BY g.\nCROSSTABS x BY f.");
    let table = tables(&result)[0];
    assert_eq!(&table.columns[3..], ["0", "1", "Total"]);
    assert_eq!(row(table, 1), "|20|Count|1|0|1");
    // the second group has no case with f = 0
    assert_eq!(row(table, 4), "2|40|Count||1|1");
}

#[test]
fn z_scores_are_computed_per_group_and_missing_outside_the_filter() {
    let mut session = session();
    run(
        &mut session,
        "FILTER BY f.\nSPLIT FILE BY g.\nDESCRIPTIVES x /SAVE.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "Zx"), "-0.71 . 0.71 -0.71 . 0.71 .");
}

#[test]
fn split_and_filter_follow_renames_and_end_with_their_variables() {
    let mut session = session();
    run(
        &mut session,
        "SPLIT FILE BY g.\nFILTER BY f.\nRENAME VARIABLES (g f = group use).",
    );
    let data = session.dataset().unwrap();
    assert_eq!(data.split_file().unwrap().variables, ["group"]);
    assert_eq!(data.filter_variable(), Some("use"));
    run(&mut session, "DELETE VARIABLES use group.");
    let data = session.dataset().unwrap();
    assert!(data.split_file().is_none() && data.filter_variable().is_none());
}

#[test]
fn sort_cases_turns_the_split_off() {
    let mut session = session();
    let result = run(&mut session, "SPLIT FILE BY g.\nSORT CASES BY g.");
    assert!(session.dataset().unwrap().split_file().is_none());
    assert_eq!(
        result.commands[1].warnings,
        ["SORT CASES turns SPLIT FILE off; give SPLIT FILE again to split"]
    );
}

#[test]
fn a_warning_every_split_group_gives_shows_once() {
    let mut session = session();
    let result = run(
        &mut session,
        "SPLIT FILE BY g.\nEXAMINE x /PLOT BOXPLOT STEMLEAF NPPLOT.\nBEGIN PROGRAM PYTHON3.\nprint(1)\nEND PROGRAM.",
    );
    assert_eq!(
        result.commands[1].warnings,
        [
            "plots are not drawn yet: BOXPLOT, STEMLEAF, NPPLOT left out; the statistics and the Tests of Normality table are shown"
        ]
    );
    assert_eq!(
        result.commands[2].warnings,
        [
            "the program block was not run: Statscraft runs no Python or R code, so what the block would make (files, variables, commands) is missing"
        ]
    );
}

#[test]
fn temporary_changes_last_until_the_next_procedure() {
    let mut session = session();
    let result = run(
        &mut session,
        "TEMPORARY.\nSELECT IF x > 30.\nSPLIT FILE BY g.\nCOMPUTE y = x * 2.\nDESCRIPTIVES y.\nDESCRIPTIVES x.",
    );
    let tables = tables(&result);
    assert_eq!(row(tables[0], 0), "2|y||3|100|20|80|120");
    assert_eq!(row(tables[0], 3), "1|y||1|140||140|140");
    // the second procedure sees the permanent data again
    assert_eq!(row(tables[1], 0), "x||7|40|21.60|10|70");
    let data = session.dataset().unwrap();
    assert_eq!(data.case_count(), 7);
    assert!(data.variable("y").is_err() && data.split_file().is_none());
}

#[test]
fn variables_the_procedure_after_temporary_adds_stay() {
    let mut session = session();
    run(
        &mut session,
        "TEMPORARY.\nCOMPUTE x = x / 10.\nDESCRIPTIVES x /SAVE.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "x"), "10 20 30 40 50 60 70");
    assert_eq!(show(data, "Zx"), "-1.39 -0.93 -0.46 0 0.46 0.93 1.39");
}

#[test]
fn temporary_ends_with_the_run_and_refuses_sort_cases() {
    let mut session = session();
    run(&mut session, "TEMPORARY.\nCOMPUTE x = 0.");
    assert_eq!(
        show(session.dataset().unwrap(), "x"),
        "10 20 30 40 50 60 70"
    );
    let errors = session
        .run("TEMPORARY.\nSORT CASES BY x.", None)
        .unwrap_err();
    assert_eq!(
        errors[0].message,
        "SORT CASES cannot follow TEMPORARY: run a procedure or EXECUTE first"
    );
}

#[test]
fn n_of_cases_keeps_the_first_cases_after_selection() {
    let mut session = session();
    let result = run(
        &mut session,
        "N OF CASES 3.\nSELECT IF x > 10.\nDESCRIPTIVES x.",
    );
    assert_eq!(row(tables(&result)[0], 0), "x||3|30|10|20|40");
    assert_eq!(session.dataset().unwrap().case_count(), 3);
    // a larger limit later changes nothing; a temporary one only the next procedure
    run(&mut session, "N 5.\nEXECUTE.");
    assert_eq!(session.dataset().unwrap().case_count(), 3);
    let result = run(
        &mut session,
        "TEMPORARY.\nN 1.\nDESCRIPTIVES x.\nDESCRIPTIVES x.",
    );
    assert_eq!(row(tables(&result)[0], 0), "x||1|20||20|20");
    assert_eq!(row(tables(&result)[1], 0), "x||3|30|10|20|40");
}

#[test]
fn sample_draws_exactly_n_of_the_first_m_cases() {
    let mut session = session();
    run(&mut session, "SAMPLE 3 FROM 5.");
    let data = session.dataset().unwrap();
    // three of the first five cases; the cases after the fifth are left out too
    assert_eq!(data.case_count(), 3);
    let x = data.variable("x").unwrap().numeric().unwrap();
    assert!(x.iter().all(|v| v.unwrap() <= 50.0));
    // the same seed in every session: the sample repeats
    let mut again = self::session();
    run(&mut again, "SAMPLE 3 FROM 5.");
    assert_eq!(show(again.dataset().unwrap(), "x"), show(data, "x"));
}

#[test]
fn sample_inside_do_if_samples_only_that_branch() {
    let mut session = session();
    run(
        &mut session,
        "DO IF g = 2.\nSAMPLE 1 FROM 3.\nEND IF.\nCOMPUTE one = 1.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(data.case_count(), 5);
    assert_eq!(show(data, "g").matches('2').count(), 1);
}

#[test]
fn a_proportion_keeps_about_that_share() {
    let mut session = Session::default();
    let mut source = String::from("DATA LIST FREE /x.\nBEGIN DATA\n");
    source.extend((0..2000).map(|i| format!("{i} ")));
    source.push_str("\nEND DATA.\nSAMPLE .25.");
    run(&mut session, &source);
    let kept = session.dataset().unwrap().case_count();
    assert!((440..=560).contains(&kept), "{kept}");
}

#[test]
fn aggregate_ignores_the_split_and_uses_the_filtered_cases() {
    let mut session = session();
    run(
        &mut session,
        "SPLIT FILE BY g.\nFILTER BY f.\nAGGREGATE /BREAK=g /m = MEAN(x) /n = N.",
    );
    let data = session.dataset().unwrap();
    assert_eq!(show(data, "m"), "36.67 . 36.67 50 . 50 36.67");
    assert_eq!(show(data, "n"), "3 . 3 2 . 2 3");
}

#[test]
fn a_cell_from_each_group_lines_up_in_the_merged_table() {
    let mut session = session();
    let result = run(&mut session, "SPLIT FILE BY g.\nFREQUENCIES g.");
    let table = tables(&result)[0];
    assert!(matches!(&table.rows[0][0], Cell::Text(t) if t == "1"));
    assert_eq!(table.rows.len(), 6);
}
