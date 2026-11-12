//! SET TVARS, TNUMBERS, OVARS and ONUMBERS: the same tables under each setting, with the
//! exact header and label text. The data are made up for the test.

use statscraft_engine::session::{RunResult, Session};
use statscraft_engine::{Table, cell_text};

const DATA: &str = "\
DATA LIST LIST /sex q1 score.
BEGIN DATA
1 1 10
1 2 12
2 1 14
2 2 18
1 1 11
2 2 16
1 2 13
2 1 15
END DATA.
VARIABLE LABELS sex 'Gender' q1 'Satisfied' score 'Test score'.
VALUE LABELS sex 1 'Male' 2 'Female' / q1 1 'Yes' 2 'No'.
VARIABLE LEVEL sex q1 (NOMINAL) score (SCALE).
";

fn tables(settings: &str, source: &str) -> Vec<Table> {
    let mut session = Session::default();
    session.run(DATA, None).unwrap();
    if !settings.is_empty() {
        session.run(settings, None).unwrap();
    }
    let result: RunResult = session
        .run(source, None)
        .unwrap_or_else(|errors| panic!("{errors:?}"));
    result.commands.into_iter().flat_map(|c| c.tables).collect()
}

/// A row of a table as text, cells separated by `|`.
fn row(table: &Table, r: usize) -> String {
    let cells: Vec<String> = (0..table.columns.len())
        .map(|c| cell_text(table, r, c))
        .collect();
    cells.join("|")
}

const NAMES: &str = "SET TVARS=NAMES TNUMBERS=VALUES.";
const BOTH: &str = "SET TVARS=BOTH TNUMBERS=BOTH OVARS=BOTH ONUMBERS=BOTH.";

#[test]
fn frequencies_keep_the_value_and_drop_the_label_column_for_values() {
    let columns = |labels: bool| {
        let mut c = vec!["Category", "Value", "Label", "Frequency", "Percent"];
        c.extend(["Valid Percent", "Cumulative Percent"]);
        if !labels {
            c.remove(2);
        }
        c
    };
    for (settings, title, labels, first) in [
        ("", "Satisfied", true, "valid|1|Yes|4|50|50|50"),
        (NAMES, "Satisfied", false, "valid|1|4|50|50|50"),
        (BOTH, "q1 Satisfied", true, "valid|1|Yes|4|50|50|50"),
        ("SET OVARS=NAMES.", "q1", true, "valid|1|Yes|4|50|50|50"),
    ] {
        let t = &tables(settings, "FREQUENCIES q1.")[0];
        assert_eq!(t.title, title, "{settings}");
        assert_eq!(t.columns, columns(labels), "{settings}");
        assert_eq!(row(t, 0), first, "{settings}");
    }
}

#[test]
fn crosstabs_headers_and_categories() {
    for (settings, title, columns, group, first) in [
        (
            "",
            "Satisfied by Gender",
            ["Satisfied", "Statistic", "Male", "Female", "Total"],
            "Gender",
            "Yes|Count|2|2|4",
        ),
        (
            NAMES,
            "Satisfied by Gender",
            ["q1", "Statistic", "1", "2", "Total"],
            "sex",
            "1|Count|2|2|4",
        ),
        (
            BOTH,
            "q1 Satisfied by sex Gender",
            ["q1 Satisfied", "Statistic", "1 Male", "2 Female", "Total"],
            "sex Gender",
            "1 Yes|Count|2|2|4",
        ),
    ] {
        let t = &tables(settings, "CROSSTABS q1 BY sex.")[0];
        assert_eq!(t.title, title, "{settings}");
        assert_eq!(t.columns, columns, "{settings}");
        assert_eq!(t.column_groups[2], [group], "{settings}");
        assert_eq!(row(t, 0), first, "{settings}");
    }
}

#[test]
fn means_report_and_summary() {
    for (settings, title, control, summary, first) in [
        (
            "",
            "Report: Test score by Gender",
            "Gender",
            "Test score * Gender|8|100|0|0|8|100",
            "Male|11.50|4|1.29",
        ),
        (
            NAMES,
            "Report: Test score by Gender",
            "sex",
            "score * sex|8|100|0|0|8|100",
            "1|11.50|4|1.29",
        ),
        (
            BOTH,
            "Report: score Test score by sex Gender",
            "sex Gender",
            "score Test score * sex Gender|8|100|0|0|8|100",
            "1 Male|11.50|4|1.29",
        ),
    ] {
        let all = tables(settings, "MEANS score BY sex.");
        assert_eq!(row(&all[0], 0), summary, "{settings}");
        let t = &all[1];
        assert_eq!(t.title, title, "{settings}");
        assert_eq!(t.columns, [control, "Mean", "N", "Std. Deviation"]);
        assert_eq!(row(t, 0), first, "{settings}");
        assert_eq!(row(t, 2), "Total|13.62|8|2.67", "{settings}");
    }
}

#[test]
fn ctables_follow_the_settings_and_vlabels_wins() {
    for (settings, source, group, first) in [
        ("", "", ["Gender", "Male"], "Satisfied|Yes|2|2"),
        (NAMES, "", ["sex", "1.00"], "q1|1.00|2|2"),
        (
            BOTH,
            "",
            ["sex Gender", "1.00 Male"],
            "q1 Satisfied|1.00 Yes|2|2",
        ),
        // VLABELS decides for the variables it names; DEFAULT follows TVARS
        (
            BOTH,
            " /VLABELS VARIABLES=q1 DISPLAY=NAME /VLABELS VARIABLES=sex DISPLAY=DEFAULT",
            ["sex Gender", "1.00 Male"],
            "q1|1.00 Yes|2|2",
        ),
        (
            NAMES,
            " /VLABELS VARIABLES=q1 sex DISPLAY=LABEL",
            ["Gender", "1.00"],
            "Satisfied|1.00|2|2",
        ),
    ] {
        let syntax = format!("CTABLES /TABLE q1 BY sex{source}.");
        let t = &tables(settings, &syntax)[0];
        assert_eq!(t.column_groups[2], group, "{settings}{source}");
        assert_eq!(row(t, 0), first, "{settings}{source}");
    }
}

#[test]
fn regression_and_unianova_name_their_variables_and_levels() {
    for (settings, variable) in [("", "Satisfied"), (NAMES, "q1"), (BOTH, "q1 Satisfied")] {
        let all = tables(settings, "REGRESSION /DEPENDENT score /METHOD=ENTER q1.");
        assert_eq!(row(&all[0], 0), format!("1|{variable}||Enter"));
        let coefficients = all.iter().find(|t| t.title == "Coefficients").unwrap();
        assert_eq!(
            row(coefficients, 1),
            format!("|{variable}|2.25|1.82|0.45|1.24|0.26")
        );
    }
    for (settings, columns, first, source) in [
        (
            "",
            vec!["", "", "Value Label", "N"],
            "Gender|1|Male|4",
            "Gender",
        ),
        (NAMES, vec!["", "", "N"], "sex|1|4", "sex"),
        (
            BOTH,
            vec!["", "", "Value Label", "N"],
            "sex Gender|1|Male|4",
            "sex Gender",
        ),
    ] {
        let all = tables(settings, "UNIANOVA score BY sex.");
        assert_eq!(all[0].columns, columns, "{settings}");
        assert_eq!(row(&all[0], 0), first, "{settings}");
        let effects = &all[1];
        assert!(
            row(effects, 2).starts_with(&format!("{source}|36.12|1|")),
            "{settings}"
        );
    }
}

#[test]
fn split_groups_follow_titles_when_separate_and_tables_when_layered() {
    let titles = |settings: &str| -> Vec<String> {
        let source = "SORT CASES BY sex.\nSPLIT FILE SEPARATE BY sex.\nDESCRIPTIVES score.";
        tables(settings, source)
            .into_iter()
            .map(|t| t.title)
            .collect()
    };
    let expected = |a: &str, b: &str| [a, b].map(|g| format!("Descriptive Statistics ({g})"));
    assert_eq!(titles(""), expected("Gender = Male", "Gender = Female"));
    assert_eq!(
        titles("SET OVARS=NAMES ONUMBERS=VALUES."),
        expected("sex = 1", "sex = 2")
    );
    // the table settings leave the titles alone
    assert_eq!(titles(NAMES), expected("Gender = Male", "Gender = Female"));
    let source = "SORT CASES BY sex.\nSPLIT FILE LAYERED BY sex.\nDESCRIPTIVES score.";
    let t = &tables(NAMES, source)[0];
    assert_eq!(t.columns[..3], ["sex", "Variable", "N"]);
    assert!(row(t, 0).starts_with("1|score|4|"), "{}", row(t, 0));
}

#[test]
fn set_checks_the_values_and_the_session_keeps_them() {
    let mut session = Session::default();
    session.run(DATA, None).unwrap();
    let error = session.run("SET TVARS=VALUES.", None).unwrap_err();
    assert!(
        format!("{error:?}").contains("SET TVARS takes NAMES, LABELS, BOTH, got VALUES"),
        "{error:?}"
    );
    let error = session.run("SET ONUMBERS=NAMES.", None).unwrap_err();
    assert!(format!("{error:?}").contains("SET ONUMBERS takes VALUES, LABELS, BOTH"));
    let result = session
        .run(
            "SET TVARS=both TNUMBERS=VALUES.\nSHOW TVARS TNUMBERS OVARS.",
            None,
        )
        .unwrap();
    assert!(result.commands.iter().all(|c| c.warnings.is_empty()));
    let show = &result.commands[1].tables[0];
    let rows: Vec<String> = (0..show.rows.len()).map(|r| row(show, r)).collect();
    // followed now, so without the note that output does not follow them
    assert_eq!(rows, ["TVARS|BOTH", "TNUMBERS|VALUES", "OVARS|LABELS"]);
    let kept = session.settings_syntax();
    let mut next = Session::default();
    next.restore_settings(&kept).unwrap();
    next.run(DATA, None).unwrap();
    let t = &next.run("CROSSTABS q1 BY sex.", None).unwrap().commands[0].tables[0];
    assert_eq!(t.columns[..3], ["q1 Satisfied", "Statistic", "1"]);
}
