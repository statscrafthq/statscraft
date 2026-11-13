//! The Statscraft output file (.scout): the golden file of every version, round trips through
//! OUTPUT SAVE, OUTPUT OPEN and OUTPUT EXPORT, deletion as the output window does it, and the
//! files of another program left alone.

use statscraft_engine::output_file::{self, Item};
use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table};
use std::path::PathBuf;
use std::sync::Arc;

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "statscraft-output-file-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Made-up library visits: branch (1 Harbor, 2 Old Town, 3 Hillside) and visits per month.
const DATA: &str = "DATA LIST LIST /branch visits (F2).
BEGIN DATA
1 4
2 7
1 2
3 9
2 5
2 1
END DATA.
VALUE LABELS branch 1 'Harbor' 2 'Old Town' 3 'Hillside'.
";

fn run(session: &mut Session, source: &str) -> Vec<String> {
    let result = session
        .run(source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    result
        .commands
        .into_iter()
        .flat_map(|c| c.warnings)
        .collect()
}

fn titles(items: &[Item]) -> Vec<String> {
    let leaves = output_file::leaves(items).into_iter();
    leaves.map(|(_, item)| item.shown().0.title).collect()
}

fn golden(version: u32) -> String {
    let path = format!(
        "{}/tests/output/v{version}.scout",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).unwrap()
}

/// The text with the line that names the writing program taken out.
fn without_writer(text: &str) -> String {
    let lines = text.lines().filter(|l| !l.contains("\"written_by\""));
    lines.collect::<Vec<_>>().join("\n")
}

#[test]
fn the_version_1_golden_file_reads_and_writes_back_unchanged() {
    let text = golden(1);
    let items = output_file::read(&text).unwrap();
    assert_eq!(items.len(), 3);
    let Item::Command(command) = &items[0] else {
        panic!("{:?}", items[0]);
    };
    assert_eq!(command.command, "FREQUENCIES");
    assert_eq!(command.syntax, "FREQUENCIES branch\n  /BARCHART.");
    assert_eq!(command.warnings, ["branch: 2 cases have no value"]);
    let Item::Table(table) = &command.items[0] else {
        panic!()
    };
    assert_eq!(table.rows[1][3], Cell::Number(53.333333333333336));
    assert_eq!(table.rows[2][3], Cell::Number(100.0));
    assert_eq!(table.caption.as_deref(), Some("Visits in March"));
    assert_eq!(table.formats[0][3], "F5.1");
    let Item::Chart(chart) = &command.items[1] else {
        panic!()
    };
    assert_eq!(chart.series[0].values[0], 14.0);
    // a number that is not finite is written as null and reads back as NaN
    assert!(chart.series[0].values[1].is_nan());
    assert!(matches!(&items[1], Item::Unknown(v) if v["kind"] == "note"));
    let Item::Table(cells) = &items[2] else {
        panic!()
    };
    assert_eq!(
        cells.rows[0],
        [
            Cell::Text("Row \"one\"".into()),
            Cell::Number(-0.000125),
            Cell::Empty,
            Cell::Text("Infinity".into())
        ]
    );
    assert_eq!(cells.rows[1][1], Cell::Number(12345678901234.0));
    assert_eq!(
        titles(&items),
        [
            "Library branch",
            "Library branch",
            "An output item of kind note, which this version of Statscraft cannot show; it is \
             kept and saved as it is",
            "Every kind of cell"
        ]
    );
    // written again, the file is the same, the item of the unknown kind included
    let written = output_file::write(&items);
    assert_eq!(without_writer(&written), without_writer(&text));
    let again: serde_json::Value = serde_json::from_str(&written).unwrap();
    let before: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(again["items"][1], before["items"][1]);
}

#[test]
fn every_kind_of_cell_survives_a_write_and_a_read() {
    let table = Table {
        title: "Cells".into(),
        row_headers: 1,
        columns: vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()],
        column_groups: vec![vec![]; 5],
        rows: vec![vec![
            Cell::Text("x".into()),
            Cell::Number(0.1 + 0.2),
            Cell::Empty,
            Cell::Number(f64::NAN),
            Cell::Number(f64::NEG_INFINITY),
        ]],
        formats: vec![vec![
            String::new(),
            "F8.3".into(),
            String::new(),
            String::new(),
            "F8.0".into(),
        ]],
        caption: None,
        outline: None,
        hidden: false,
    };
    let text = output_file::write(&[Item::Table(Arc::new(table))]);
    let items = output_file::read(&text).unwrap();
    let Item::Table(read) = &items[0] else {
        panic!()
    };
    assert_eq!(read.rows[0][1], Cell::Number(0.1 + 0.2));
    assert_eq!(read.rows[0][2], Cell::Empty);
    // not finite: kept as the text the table showed
    let (Cell::Text(nan), Cell::Text(minus)) = (&read.rows[0][3], &read.rows[0][4]) else {
        panic!("{:?}", read.rows[0])
    };
    assert!(!nan.is_empty() && !minus.is_empty(), "{nan} {minus}");
    assert_eq!(output_file::write(&items), text);
}

#[test]
fn save_open_and_export_give_the_same_tables_and_new_output_is_appended() {
    let dir = folder("round");
    let mut first = Session::default();
    run(
        &mut first,
        &format!(
            "CD '{}'.\n{DATA}FREQUENCIES branch /BARCHART.\nDESCRIPTIVES visits.\n\
             OUTPUT SAVE OUTFILE='march.scout'.\nOUTPUT EXPORT /CSV DOCUMENTFILE='direct.csv'.",
            dir.display()
        ),
    );
    assert!(!first.output_documents()[0].unsaved);
    let text = std::fs::read_to_string(dir.join("march.scout")).unwrap();
    assert!(
        text.contains("\"syntax\": \"FREQUENCIES branch /BARCHART.\""),
        "{text}"
    );
    assert!(text.contains("\"kind\": \"chart\""), "{text}");

    // headless conversion: open, export
    let mut second = Session::default();
    let source = format!(
        "CD '{}'.\nOUTPUT OPEN FILE='march.scout' NAME=march.\n\
         OUTPUT EXPORT /CSV DOCUMENTFILE='opened.csv'.",
        dir.display()
    );
    let result = second.run(&source, None).unwrap();
    let open = &result.commands[1];
    assert_eq!(open.document.as_deref(), Some("march"));
    let (file, items) = open.opened.as_ref().unwrap();
    assert_eq!(file, "march.scout");
    assert_eq!(titles(items).len(), 3);
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
    assert_eq!(read("opened.csv"), read("direct.csv"));
    let listed = &second.output_documents()[0];
    assert_eq!(
        (listed.name.as_str(), listed.tables, listed.unsaved),
        ("march", 3, false)
    );
    assert_eq!(listed.file, Some(dir.join("march.scout")));

    // the opened document is designated: new tables join it, and saving keeps both
    run(
        &mut second,
        &format!("{DATA}MEANS visits BY branch.\nOUTPUT SAVE OUTFILE='march.scout'."),
    );
    let items = output_file::read(&read("march.scout")).unwrap();
    let all = titles(&items);
    assert_eq!(all.len(), 5, "{all:?}");
    assert_eq!(all, titles(second.output_items("march").unwrap()));
    let Item::Command(means) = items.last().unwrap() else {
        panic!()
    };
    assert_eq!(means.command, "MEANS");
    assert_eq!(means.syntax, "MEANS visits BY branch.");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_files_of_another_program_are_never_written_or_read() {
    let dir = folder("foreign");
    let theirs = dir.join("report.spv");
    std::fs::write(&theirs, b"PK\x03\x04 not ours").unwrap();
    let mut s = Session::default();
    let warnings = run(
        &mut s,
        &format!(
            "CD '{}'.\n{DATA}FREQUENCIES branch.\nOUTPUT SAVE OUTFILE='report.spv'.",
            dir.display()
        ),
    );
    assert!(
        warnings[0].contains("another program's output file"),
        "{warnings:?}"
    );
    assert!(warnings[0].contains("report.scout"), "{warnings:?}");
    assert_eq!(std::fs::read(&theirs).unwrap(), b"PK\x03\x04 not ours");
    assert!(dir.join("report.scout").exists());
    // OUTPUT OPEN of their name reads ours
    run(&mut s, "OUTPUT OPEN FILE='report.SPV' NAME=again.");
    assert_eq!(s.designated_output(), Some("again"));
    assert_eq!(s.output_documents()[1].tables, 1);
    std::fs::remove_file(dir.join("report.scout")).unwrap();
    let warnings = run(&mut s, "OUTPUT OPEN FILE='report.spv'.");
    assert!(warnings[0].contains("cannot read"), "{warnings:?}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_file_that_cannot_be_opened_leaves_the_designation_as_it_was() {
    let dir = folder("missing");
    std::fs::write(dir.join("other.scout"), "{\"format\": \"something else\"}").unwrap();
    let newer = "{\"format\": \"statscraft-output\", \"version\": 99, \"items\": []}";
    std::fs::write(dir.join("newer.scout"), newer).unwrap();
    let mut s = Session::default();
    run(
        &mut s,
        &format!("CD '{}'.\n{DATA}OUTPUT NEW NAME=main.", dir.display()),
    );
    let warnings = run(
        &mut s,
        "OUTPUT OPEN FILE='nowhere.scout'.\nOUTPUT OPEN FILE='other.scout'.\n\
         OUTPUT OPEN FILE='newer.scout'.\nFREQUENCIES branch.",
    );
    assert_eq!(warnings.len(), 3, "{warnings:?}");
    assert!(
        warnings[0].starts_with("cannot open nowhere.scout"),
        "{warnings:?}"
    );
    assert!(
        warnings[1].contains("not a Statscraft output file"),
        "{warnings:?}"
    );
    assert!(
        warnings[2].contains("newer Statscraft (output format 99)"),
        "{warnings:?}"
    );
    assert!(
        warnings[2].ends_with("the tables still go to main"),
        "{warnings:?}"
    );
    assert_eq!(s.output_documents().len(), 1);
    assert_eq!(s.output_documents()[0].tables, 1);
    // OUTPUT SAVE of a document that does not exist is an error
    let errors = s.run("OUTPUT SAVE NAME=nobody OUTFILE='x.scout'.", None);
    let text = errors.unwrap_err()[0].text();
    assert!(
        text.contains("no open output document is named nobody"),
        "{text}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn deleting_output_changes_the_document_and_shows_in_the_log() {
    let dir = folder("delete");
    let mut s = Session::default();
    run(
        &mut s,
        &format!(
            "CD '{}'.\n{DATA}FREQUENCIES branch /BARCHART.\nDESCRIPTIVES visits.\n\
             OUTPUT SAVE OUTFILE='d.scout'.",
            dir.display()
        ),
    );
    let before = titles(s.output_items("Document1").unwrap());
    assert_eq!(before.len(), 3, "{before:?}");
    // the chart and the descriptives table go; DESCRIPTIVES goes with its only table
    s.delete_output("Document1", &[1, 2]).unwrap();
    assert_eq!(titles(s.output_items("Document1").unwrap()), before[..1]);
    assert_eq!(s.output_items("Document1").unwrap().len(), 1);
    assert!(s.output_documents()[0].unsaved);
    let entry = s.log.last().unwrap();
    assert!(
        entry
            .syntax
            .starts_with("* The output window deleted 2 items from Document1: ")
    );
    assert_eq!(
        entry.result.as_deref(),
        Ok("deleted 2 items from Document1")
    );
    // the log entry is a comment: a script made from the log runs
    let syntax = entry.syntax.clone();
    run(&mut s, &format!("{syntax}\nSHOW N."));
    // a position the document does not have deletes nothing
    assert!(s.delete_output("Document1", &[9]).is_err());
    assert!(s.delete_output("nobody", &[0]).is_err());
    run(&mut s, "OUTPUT SAVE OUTFILE='d.scout'.");
    let text = std::fs::read_to_string(dir.join("d.scout")).unwrap();
    // the table of SHOW joined the one left
    assert_eq!(titles(&output_file::read(&text).unwrap()).len(), 2);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn undo_keeps_the_output() {
    let mut s = Session::default();
    run(&mut s, &format!("{DATA}FREQUENCIES branch."));
    run(&mut s, "COMPUTE twice = visits * 2.\nDESCRIPTIVES twice.");
    assert!(s.undo());
    assert!(s.dataset().unwrap().variable_names().all(|n| n != "twice"));
    assert_eq!(s.output_documents()[0].tables, 2);
}

#[test]
fn statistical_plots_keep_their_shapes() {
    let mut s = Session::default();
    let source = format!("{DATA}EXAMINE visits BY branch /PLOT BOXPLOT STEMLEAF NPPLOT /NOTOTAL.");
    let result = s.run(&source, None).unwrap();
    let charts: Vec<serde_json::Value> = result
        .commands
        .iter()
        .flat_map(|c| &c.charts)
        .map(|c| serde_json::to_value(c).unwrap())
        .collect();
    assert!(
        charts.iter().any(|c| c["plot"]["shape"].is_string()),
        "{charts:?}"
    );
    let text = output_file::write(s.output_items("Document1").unwrap());
    let items = output_file::read(&text).unwrap();
    let read: Vec<serde_json::Value> = output_file::leaves(&items)
        .into_iter()
        .filter_map(|(_, item)| match item {
            Item::Chart(c) => Some(serde_json::to_value(&**c).unwrap()),
            _ => None,
        })
        .collect();
    assert_eq!(read, charts);
}
