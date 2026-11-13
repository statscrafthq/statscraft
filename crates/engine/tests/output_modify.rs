//! OUTPUT MODIFY on invented orchard data: which items stay, what their titles and labels
//! become, cell texts after FORMAT and REPLACE, row order after SORT, hidden items in files.

use statscraft_engine::output_file::{Item, leaves};
use statscraft_engine::session::{RunResult, Session};
use statscraft_engine::{Cell, Table, cell_text};
use std::path::PathBuf;

const ORCHARD: &str = "DATA LIST LIST /variety (A6) yield height.
BEGIN DATA
apple 12 3.5
pear 7 4.0
apple 15 3.0
plum 4 2.5
pear 9 4.5
apple 6 3.2
END DATA.
DESCRIPTIVES yield height.
FREQUENCIES variety.
MEANS yield BY variety.
FREQUENCIES yield.";

fn run(session: &mut Session, source: &str) -> RunResult {
    session
        .run(source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"))
}

fn warnings(result: &RunResult) -> Vec<String> {
    let all = result.commands.iter().flat_map(|c| c.warnings.clone());
    all.collect()
}

fn fails(session: &mut Session, source: &str) -> String {
    let errors = session.run(source, None).unwrap_err();
    let texts: Vec<String> = errors.iter().map(|e| e.text()).collect();
    texts.join(" | ")
}

fn orchard() -> Session {
    let mut s = Session::default();
    run(&mut s, ORCHARD);
    s
}

/// The items of Document1, each as the window shows it.
fn shown(s: &Session) -> Vec<Table> {
    let items = s.output_items("Document1").unwrap();
    leaves(items).iter().map(|(_, i)| i.shown().0).collect()
}

fn titles(s: &Session) -> Vec<String> {
    shown(s).into_iter().map(|t| t.title).collect()
}

fn table(s: &Session, title: &str) -> Table {
    let found = shown(s).into_iter().find(|t| t.title == title);
    found.unwrap_or_else(|| panic!("no table {title} in {:?}", titles(s)))
}

/// The texts of one column, as the window shows them.
fn column(t: &Table, c: usize) -> Vec<String> {
    (0..t.rows.len()).map(|r| cell_text(t, r, c)).collect()
}

fn headings(s: &Session) -> Vec<String> {
    let items = s.output_items("Document1").unwrap();
    let commands = items.iter().filter_map(|i| match i {
        Item::Command(c) => Some(c.heading().to_string()),
        _ => None,
    });
    commands.collect()
}

#[test]
fn items_are_picked_by_command_run_label_and_instance() {
    let mut s = orchard();
    let all = [
        "Descriptive Statistics",
        "variety",
        "Case Processing Summary",
        "Report: yield by variety",
        "yield",
    ];
    assert_eq!(titles(&s), all);
    // the second run of FREQUENCIES, and the first table of MEANS
    run(
        &mut s,
        "OUTPUT MODIFY /SELECT TABLES /IF COMMANDS=['Frequencies(2)'] /DELETEOBJECT DELETE=YES\n\
         /SELECT TABLES /IF COMMANDS=['means'] INSTANCES=[1] /DELETE DELETE=YES.",
    );
    assert_eq!(titles(&s), [all[0], all[1], all[3]]);
    // labels compare with case
    run(
        &mut s,
        "OUTP MOD /SEL TABLES /IF LABELS=[CONTAINS('Yield')] /OBJ OUTLINELABEL='no'\n\
         /SELECT TABLES /IF LABELS=[ENDS('variety') BEGINS('Desc')] VISIBLEONLY=YES\n\
         /OBJECTPROPERTIES OUTLINELABEL=')LABEL!'.",
    );
    let outlines: Vec<Option<String>> = shown(&s).into_iter().map(|t| t.outline).collect();
    assert_eq!(
        outlines,
        [
            Some("Descriptive Statistics!".into()),
            Some("variety!".into()),
            Some("Report: yield by variety!".into())
        ]
    );
    // LAST is the last command of the document; its heading goes with its last table
    run(
        &mut s,
        "OUTPUT MODIFY /SELECT TABLES /IF COMMANDS=[LAST] /DELETEOBJECT DELETE=YES.",
    );
    assert_eq!(titles(&s), [all[0], all[1]]);
    assert_eq!(headings(&s), ["DESCRIPTIVES", "FREQUENCIES"]);
    // a heading deleted takes its command's items with it; nothing else is picked
    let result = run(
        &mut s,
        "OUTPUT MODIFY /SELECT HEADINGS /IF COMMANDS=['FREQUENCIES(LAST)'] /DELETEOBJECT DELETE=YES\n\
         /SELECT LOGS NOTES /DELETEOBJECT DELETE=YES.",
    );
    assert_eq!(titles(&s), [all[0]]);
    let modified = &result.commands[0].modified;
    assert_eq!(modified[0].document, "Document1");
    assert_eq!(modified[0].items.len(), 1);
    assert_eq!(
        (modified[0].items[0].position, modified[0].items[0].deleted),
        (1, true)
    );
    // the JSON of the run tells agents the same
    let json = result.to_json(false);
    assert_eq!(
        json["commands"][0]["modified"][0]["items"][0]["position"],
        1
    );
    assert_eq!(
        json["commands"][0]["modified"][0]["items"][0]["deleted"],
        true
    );
}

#[test]
fn indexes_number_titles_labels_and_headings() {
    let mut s = orchard();
    run(
        &mut s,
        "OUTPUT MODIFY /SELECT TABLES /IF COMMANDS=['Frequencies' 'Means']\n\
         /INDEXING INDEXTYPE=ROMANCAP INDEXSTART='III' /TABLE TABLETITLE=')INDEX. )LABEL'\n\
         /SELECT TABLES /IF COMMANDS=['descriptives'] /INDEXING INDEXTYPE=LETTERLOW INDEXSTART='z'\n\
         /OBJECTPROPERTIES OUTLINELABEL='Part )INDEX'\n\
         /SELECT OUTLINEHEADERS /OBJECTPROPERTIES OUTLINELABEL='Step )INDEX: )LABEL'.",
    );
    assert_eq!(
        titles(&s),
        [
            "Descriptive Statistics",
            "III. variety",
            "IV. Case Processing Summary",
            "V. Report: yield by variety",
            "VI. yield"
        ]
    );
    assert_eq!(shown(&s)[0].outline.as_deref(), Some("Part z"));
    assert_eq!(
        headings(&s),
        [
            "Step 1: DESCRIPTIVES",
            "Step 2: FREQUENCIES",
            "Step 3: MEANS",
            "Step 4: FREQUENCIES"
        ]
    );
    // a renamed heading still answers to its command name
    run(
        &mut s,
        "OUTPUT MODIFY /SELECT HEADINGS /IF COMMANDS=['MEANS'] /TEXTS TEXT='Yield by variety'.",
    );
    assert_eq!(headings(&s)[2], "Yield by variety");
    // letters run on past z
    run(
        &mut s,
        "OUTPUT MODIFY /SELECT TABLES /INDEXING INDEXTYPE=LETTERCAP INDEXSTART='Y'\n\
         /OBJECTPROPERTIES OUTLINELABEL=')INDEX'.",
    );
    let outlines: Vec<String> = shown(&s).into_iter().filter_map(|t| t.outline).collect();
    assert_eq!(outlines, ["Y", "Z", "AA", "AB", "AC"]);
}

#[test]
fn table_cells_take_formats_replacements_and_hidden_rows() {
    let mut s = orchard();
    let before = table(&s, "Descriptive Statistics");
    let mean = before.columns.iter().position(|c| c == "Mean").unwrap();
    let maximum = before.columns.iter().position(|c| c == "Maximum").unwrap();
    let n = before.columns.iter().position(|c| c == "N").unwrap();
    assert_eq!(column(&before, mean)[..2], ["8.83", "3.45"]);
    run(
        &mut s,
        "OUTPUT MODIFY /SELECT TABLES /IF SUBTYPES=['descriptive statistics']\n\
         /TABLECELLS SELECT=[MEAN] SELECTDIMENSION=COLUMNS FORMAT=F8.3\n\
         /TABLECELLS SELECT=['Maximum'] SELECTDIMENSION=COLUMNS SELECTCONDITION=['x>10'] REPLACE='high'\n\
         /TABLECELLS SELECT=['Std Dev'] SELECTDIMENSION=COLUMNS SELECTCONDITION=[TOP(1)]\n\
           APPLYTO=ROWDATA FORMAT=F6.1\n\
         /TABLECELLS SELECT=['Valid N (listwise)' 'Missing N (listwise)'] SELECTDIMENSION=ROWS HIDE=YES.",
    );
    let after = table(&s, "Descriptive Statistics");
    assert_eq!(column(&after, 0), ["yield", "height"]);
    // the yield row has the largest standard deviation: all its numbers get one decimal
    assert_eq!(column(&after, mean), ["8.8", "3.450"]);
    assert_eq!(column(&after, n), ["6.0", "6"]);
    assert_eq!(column(&after, maximum), ["high", "4.50"]);
}

#[test]
fn sort_orders_rows_and_transpose_turns_the_table() {
    let mut s = orchard();
    let report = "Report: yield by variety";
    assert_eq!(
        column(&table(&s, report), 0),
        ["apple", "pear", "plum", "Total"]
    );
    run(
        &mut s,
        "OUTPUT MODIFY /SELECT TABLES /IF LABELS=[BEGINS('Report')] /TABLE SORT=COLLABEL('Mean').",
    );
    assert_eq!(
        column(&table(&s, report), 0),
        ["plum", "pear", "apple", "Total"]
    );
    run(
        &mut s,
        "OUTPUT MODIFY /SELECT TABLES /IF LABELS=[BEGINS('Report')]\n\
         /TABLE SORT=COLPOSITION(1) SORTDIRECTION=DESCENDING.",
    );
    assert_eq!(
        column(&table(&s, report), 0),
        ["plum", "pear", "apple", "Total"]
    );
    run(
        &mut s,
        "OUTPUT MODIFY /SELECT TABLES /IF LABELS=[BEGINS('Report')] /TABLE SORT=COLLABEL('Mean')\n\
         SORTDIRECTION=DESCENDING TRANSPOSE=YES.",
    );
    let turned = table(&s, report);
    assert_eq!(turned.columns[1..], ["apple", "pear", "plum", "Total"]);
    assert_eq!(cell_text(&turned, 0, 0), "Mean");
    assert_eq!(turned.rows[0][1..4], [11.0, 8.0, 4.0].map(Cell::Number));
    // the frequency table has nested row labels: it stays, with a warning
    let result = run(
        &mut s,
        "OUTPUT MODIFY /SELECT TABLES /IF COMMANDS=['FREQUENCIES(1)'] /TABLE SORT=COLPOSITION(5).",
    );
    let w = warnings(&result);
    assert!(
        w[0].contains("variety is not sorted: its rows have nested labels"),
        "{w:?}"
    );
}

#[test]
fn hidden_items_stay_in_the_file_and_leave_visible_exports() {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("statscraft-output-modify-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = orchard();
    run(
        &mut s,
        &format!(
            "CD '{}'.\nOUTPUT MODIFY /SELECT TABLES /IF COMMANDS=['frequencies'] \
             /OBJECTPROPERTIES VISIBLE=NO.\n\
             OUTPUT EXPORT /CONTENTS EXPORT=VISIBLE /CSV DOCUMENTFILE='seen.csv'.\n\
             OUTPUT EXPORT /CSV DOCUMENTFILE='all.csv'.\n\
             OUTPUT SAVE OUTFILE='orchard.scout'.",
            dir.display()
        ),
    );
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
    let seen = read("seen.csv");
    assert!(!seen.contains("\nvariety\n") && seen.contains("Descriptive Statistics"));
    assert!(
        !seen.starts_with("variety") && !seen.contains("\nyield\n"),
        "{seen}"
    );
    assert!(read("all.csv").contains("\nvariety\n"));
    let file = read("orchard.scout");
    assert_eq!(file.matches("\"hidden\": true").count(), 2);
    // read back: VISIBLEONLY skips the hidden ones, VISIBLE=YES shows them again
    run(
        &mut s,
        "OUTPUT OPEN FILE='orchard.scout' NAME=back.\n\
         OUTPUT MODIFY NAME=back /SELECT TABLES /IF VISIBLEONLY=YES /DELETEOBJECT DELETE=YES.",
    );
    let back = s.output_items("back").unwrap();
    let left: Vec<String> = leaves(back)
        .iter()
        .map(|(_, i)| i.shown().0.title)
        .collect();
    assert_eq!(left, ["variety", "yield"]);
    run(
        &mut s,
        "OUTPUT MODIFY NAME=ALL /SELECT HEADINGS /OBJECTPROPERTIES VISIBLE=YES.",
    );
    assert!(shown(&s).iter().all(|t| !t.hidden));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn page_titles_text_and_the_report() {
    let mut s = Session::default();
    run(&mut s, "TITLE 'Orchard survey'.\nECHO 'note'.");
    let result = run(
        &mut s,
        "OUTPUT MODIFY /REPORT PRINTREPORT=YES /SELECT PAGETITLE /TEXTS TEXT=')LABEL, draft'\n\
         /SELECT ALL EXCEPT (PAGETITLE HEADINGS OUTLINEHEADERS) /DELETEOBJECT DELETE=YES.",
    );
    let items = shown(&s);
    assert_eq!(cell_text(&items[0], 0, 0), "Orchard survey, draft");
    // the report is the last item now; ECHO went
    let report = &result.commands[0].tables[0];
    assert_eq!(report.title, "Output Changes");
    let rows: Vec<Vec<String>> = (0..report.rows.len())
        .map(|r| (0..4).map(|c| cell_text(report, r, c)).collect())
        .collect();
    assert_eq!(
        rows,
        [
            ["Document1", "Title", "TITLE", "text"],
            ["Document1", "Echo", "ECHO", "deleted"]
        ]
    );
    assert_eq!(titles(&s), ["Title", "Output Changes"]);
}

#[test]
fn what_changes_nothing_says_so_and_wrong_syntax_is_refused() {
    let mut s = orchard();
    let result = run(
        &mut s,
        "OUTPUT MODIFY /SELECT TABLES /TABLECELLS SELECT=[BODY] FONT='Serif' TEXTCOLOR=RGB(9, 9, 9)\n\
         STYLE=BOLD ITALIC BACKGROUNDCOLOR=YELLOW /TABLE TLOOK=NONE /OBJECTPROPERTIES SIZE=PERCENT(50, PROPORTIONAL).",
    );
    let w = warnings(&result);
    assert_eq!(w.len(), 6, "{w:?}");
    assert!(w[0].contains("/TABLECELLS FONT changes nothing"), "{w:?}");
    assert!(
        w.iter().any(|w| w.contains("/TABLE TLOOK changes nothing")),
        "{w:?}"
    );
    let refused = [
        (
            "OUTPUT MODIFY /TABLE TRANSPOSE=YES.",
            "every block starts with /SELECT",
        ),
        (
            "OUTPUT MODIFY /SELECT TABLES /TAB TRANSPOSE=YES.",
            "more than one keyword",
        ),
        (
            "OUTPUT MODIFY /SELECT TABLES /TABLECELLS SELECT=[BODY] SELECTCONDITION=[PARENT('a')].",
            "PARENT and CHILD",
        ),
        (
            "OUTPUT MODIFY /SELECT TABLES /REPORT PRINTREPORT=YES.",
            "before the first /SELECT",
        ),
        (
            "OUTPUT MODIFY /SELECT TABLES /INDEXING INDEXTYPE=ROMANLOW INDEXSTART='iiii'.",
            "roman numeral",
        ),
        (
            "OUTPUT MODIFY /SELECT TABLES /TABLECELLS SELECT=[BODY] SELECTCONDITION=['y>1'].",
            "is no condition",
        ),
        (
            "OUTPUT MODIFY NAME=nowhere /SELECT TABLES.",
            "no open output document is named",
        ),
        ("OUTPUT MODIFY.", "needs /SELECT"),
    ];
    for (source, message) in refused {
        let error = fails(&mut s, source);
        assert!(error.contains(message), "{source}: {error}");
    }
}
