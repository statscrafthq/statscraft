//! OUTPUT NEW, NAME, ACTIVATE, CLOSE, DISPLAY and EXPORT: which tables each document gets,
//! and the files OUTPUT EXPORT writes, read back cell by cell.

use calamine::{Data, Reader};
use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table};
use std::path::PathBuf;

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "statscraft-output-docs-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(session: &mut Session, source: &str) -> (Vec<Table>, Vec<String>) {
    let result = session
        .run(source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let tables = result.commands.iter().flat_map(|c| c.tables.clone());
    let warnings = result.commands.iter().flat_map(|c| c.warnings.clone());
    (tables.collect(), warnings.collect())
}

fn fails(session: &mut Session, source: &str) -> String {
    let errors = session.run(source, None).unwrap_err();
    errors
        .iter()
        .map(|e| e.text())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// The rows of OUTPUT DISPLAY: name, tables, designated.
fn documents(session: &mut Session) -> Vec<(String, f64, bool)> {
    let (tables, _) = run(session, "OUTPUT DISPLAY.");
    tables[0]
        .rows
        .iter()
        .map(|r| match (&r[0], &r[1], &r[2]) {
            (Cell::Text(name), Cell::Number(n), Cell::Text(d)) => (name.clone(), *n, d == "Yes"),
            other => panic!("{other:?}"),
        })
        .collect()
}

fn doc(name: &str, tables: f64, designated: bool) -> (String, f64, bool) {
    (name.to_string(), tables, designated)
}

#[test]
fn tables_go_to_the_designated_document() {
    assert!(documents(&mut Session::default()).is_empty());
    let mut s = Session::default();
    run(
        &mut s,
        "ECHO 'one'.\nOUTPUT NAME males.\nOUTPUT NEW NAME=females.\nECHO 'two'.",
    );
    run(
        &mut s,
        "ECHO 'three'.\nOUTPUT ACTIVATE males.\nECHO 'four'.",
    );
    // the display itself goes to the designated document after it is shown
    assert_eq!(
        documents(&mut s),
        [doc("males", 2.0, true), doc("females", 2.0, false)]
    );
    // a name another document has passes over; the other gets a new one
    run(&mut s, "OUTPUT NEW NAME=females.");
    assert_eq!(
        documents(&mut s),
        [
            doc("males", 3.0, false),
            doc("Document2", 2.0, false),
            doc("females", 0.0, true)
        ]
    );
    let (_, warnings) = run(&mut s, "OUTPUT ACTIVATE nobody.\nOUTPUT CLOSE nobody.");
    assert!(warnings[0].contains("still go to females"), "{warnings:?}");
    assert!(warnings[1].contains("nothing was closed"), "{warnings:?}");
    run(&mut s, "OUTPUT CLOSE *.\nOUTPUT CLOSE Document2.");
    // nothing is designated: the next table, here that of OUTPUT DISPLAY, starts a new document
    assert_eq!(documents(&mut s), [doc("males", 3.0, false)]);
    assert_eq!(
        documents(&mut s),
        [doc("males", 3.0, false), doc("Document3", 1.0, true)]
    );
    run(&mut s, "OUTPUT CLOSE ALL.\nECHO 'x'.");
    assert_eq!(documents(&mut s), [doc("Document4", 1.0, true)]);
}

#[test]
fn export_writes_text_csv_and_html() {
    let dir = folder("text");
    let mut s = Session::default();
    run(
        &mut s,
        &format!(
            "CD '{}'.\nECHO 'hello'.\nOUTPUT NEW NAME=second.\nECHO 'a, \"b\"'.\nECHO 'the third'.",
            dir.display()
        ),
    );
    run(
        &mut s,
        "OUTPUT EXPORT NAME=Document1 /TEXT DOCUMENTFILE='one.txt' /CSV DOCUMENTFILE='one.csv'.\n\
         OUTPUT EXPORT /TEXT DOCUMENTFILE='two.txt' COLUMNWIDTH=5 ROWBORDER='=' COLUMNBORDER=':'\n\
         /HTML DOCUMENTFILE='two.htm' STYLING=NO.\n\
         OUTPUT EXPORT /TEXT DOCUMENTFILE='tab.txt' SEPARATOR=TAB ENCODING=UTF16.",
    );
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
    assert_eq!(
        read("one.txt"),
        "Echo\n---------\n| Text  |\n---------\n| hello |\n---------\n"
    );
    assert_eq!(read("one.csv"), "Echo\nText\nhello\n");
    assert_eq!(
        read("two.txt"),
        "Echo\n========\n: Text :\n========\n: a,   :\n: \"b\"  :\n========\n\n\
         Echo\n=========\n: Text  :\n=========\n: the   :\n: third :\n=========\n"
    );
    let html = read("two.htm");
    assert!(html.contains("<h2>ECHO</h2>"), "{html}");
    assert!(
        html.contains("<td class=\"text\">a, &quot;b&quot;</td>"),
        "{html}"
    );
    assert!(!html.contains("<style>"), "{html}");
    let bytes = std::fs::read(dir.join("tab.txt")).unwrap();
    assert_eq!(&bytes[..2], [0xFF, 0xFE]);
    let units: Vec<u16> = bytes[2..]
        .chunks(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    assert_eq!(
        String::from_utf16(&units).unwrap(),
        "Echo\nText\na, \"b\"\n\nEcho\nText\nthe third\n"
    );
    // a document that does not exist
    let e = fails(
        &mut s,
        "OUTPUT EXPORT NAME=nobody /CSV DOCUMENTFILE='x.csv'.",
    );
    assert!(e.contains("no open output document is named nobody"), "{e}");
    std::fs::remove_dir_all(dir).unwrap();
}

/// The text of every cell of a sheet by its position (row, column) from 0.
fn sheet(path: &PathBuf, name: &str) -> Vec<(u32, u32, String)> {
    let mut book = calamine::open_workbook_auto(path).unwrap();
    let range = book.worksheet_range(name).unwrap();
    let (top, left) = range.start().unwrap_or((0, 0));
    let mut out = Vec::new();
    for (r, row) in range.rows().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            let text = match cell {
                Data::Empty => continue,
                Data::String(s) => s.clone(),
                Data::Float(f) => f.to_string(),
                Data::Int(i) => i.to_string(),
                other => format!("{other:?}"),
            };
            out.push((top + r as u32, left + c as u32, text));
        }
    }
    out
}

fn cells(list: &[(u32, u32, &str)]) -> Vec<(u32, u32, String)> {
    list.iter()
        .map(|(r, c, t)| (*r, *c, t.to_string()))
        .collect()
}

#[test]
fn export_to_workbooks_and_their_sheets() {
    let dir = folder("xlsx");
    let book = dir.join("out.xlsx");
    let mut s = Session::default();
    run(
        &mut s,
        &format!("CD '{}'.\nECHO 'first'.\nECHO 'second'.", dir.display()),
    );
    run(&mut s, "OUTPUT EXPORT /XLSX DOCUMENTFILE='out.xlsx'.");
    let names = |path: &PathBuf| calamine::open_workbook_auto(path).unwrap().sheet_names();
    assert_eq!(names(&book), ["Tables"]);
    // one below the other, a blank row between
    assert_eq!(
        sheet(&book, "Tables"),
        cells(&[
            (0, 0, "Echo"),
            (1, 0, "Text"),
            (2, 0, "first"),
            (4, 0, "Echo"),
            (5, 0, "Text"),
            (6, 0, "second")
        ])
    );
    run(
        &mut s,
        "OUTPUT NEW.\nECHO 'third'.\n\
         OUTPUT EXPORT /XLSX DOCUMENTFILE='out.xlsx' OPERATION=CREATESHEET SHEET='Region 1'.\n\
         OUTPUT EXPORT /XLSX DOCUMENTFILE='out.xlsx' OPERATION=MODIFYSHEET SHEET='Tables'.\n\
         OUTPUT EXPORT /XLSX DOCUMENTFILE='out.xlsx' OPERATION=MODIFYSHEET SHEET='Tables' LOCATION=LASTROW.\n\
         OUTPUT EXPORT /XLSX DOCUMENTFILE='out.xlsx' OPERATION=MODIFYSHEET SHEET='New' LOCATION=STARTCELL('C3').",
    );
    assert_eq!(names(&book), ["Tables", "Region 1", "New"]);
    assert_eq!(
        sheet(&book, "Region 1"),
        cells(&[(0, 0, "Echo"), (1, 0, "Text"), (2, 0, "third")])
    );
    // after the last column with a blank column between, then below the last row
    let tables = sheet(&book, "Tables");
    assert_eq!(tables.len(), 12, "{tables:?}");
    for cell in cells(&[
        (0, 2, "Echo"),
        (1, 2, "Text"),
        (2, 2, "third"),
        (8, 0, "Echo"),
        (9, 0, "Text"),
        (10, 0, "third"),
    ]) {
        assert!(tables.contains(&cell), "{cell:?} in {tables:?}");
    }
    assert_eq!(
        sheet(&book, "New"),
        cells(&[(2, 2, "Echo"), (3, 2, "Text"), (4, 2, "third")])
    );
    // CREATESHEET replaces a sheet of the same name
    run(
        &mut s,
        "ECHO 'fourth'.\nOUTPUT EXPORT /XLSX DOCUMENTFILE='out.xlsx' OPERATION=CREATESHEET SHEET='region 1'.",
    );
    assert_eq!(names(&book), ["Tables", "Region 1", "New"]);
    assert_eq!(sheet(&book, "Region 1").len(), 6);
    // a workbook from elsewhere is not touched
    std::fs::copy(&book, dir.join("other.xlsx")).unwrap();
    let e = fails(
        &mut s,
        "OUTPUT EXPORT /XLSX DOCUMENTFILE='other.xlsx' OPERATION=MODIFYSHEET SHEET='Tables'.",
    );
    assert!(
        e.contains("was not written by OUTPUT EXPORT in this session"),
        "{e}"
    );
    // a check writes nothing
    s.check(
        "OUTPUT EXPORT /XLSX DOCUMENTFILE='checked.xlsx' /CSV DOCUMENTFILE='checked.csv'.",
        None,
    )
    .unwrap();
    assert!(!dir.join("checked.xlsx").exists() && !dir.join("checked.csv").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn notes_can_be_left_out() {
    let dir = folder("notes");
    let mut s = Session::default();
    run(
        &mut s,
        &format!(
            "CD '{}'.\nDATA LIST FREE /x.\nBEGIN DATA\n1 2 4\nEND DATA.\nCLUSTER x /METHOD=SINGLE.\n\
             OUTPUT EXPORT /CSV DOCUMENTFILE='with.csv' /HTML DOCUMENTFILE='with.htm'.\n\
             OUTPUT EXPORT /CSV DOCUMENTFILE='without.csv' NOTESCAPTIONS=NO\n\
             /HTML DOCUMENTFILE='without.htm' NOTESCAPTIONS=NO.",
            dir.display()
        ),
    );
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).unwrap();
    let (with, without) = (read("with.csv"), read("without.csv"));
    let notes: Vec<&str> = with.lines().filter(|l| !without.contains(l)).collect();
    assert!(notes.contains(&"Squared Euclidean Distance used"), "{with}");
    assert!(read("with.htm").contains("<p class=\"note\">Squared Euclidean Distance used</p>"));
    assert!(!read("without.htm").contains("class=\"note\""));
    std::fs::remove_dir_all(dir).unwrap();
}
