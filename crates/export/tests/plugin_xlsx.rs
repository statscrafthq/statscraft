//! A plugin's colours and comments in a workbook: fills and font colours in the styles, and
//! the comments as Excel notes (xl/commentsN.xml with its VML drawing).

use statscraft_engine::{Cell, Decoration, Table};
use statscraft_export::{Item, write_xlsx};
use std::io::Read;

/// A table of `rows` by `columns` numbers after one row label column, every number decorated.
fn decorated(rows: usize, columns: usize) -> Table {
    let mut table = Table {
        title: "Decorated".into(),
        row_headers: 1,
        columns: (0..=columns).map(|c| format!("c{c}")).collect(),
        rows: (0..rows)
            .map(|r| {
                let label = Cell::Text(format!("row {r}"));
                let numbers = (0..columns).map(|c| Cell::Number((r * columns + c) as f64));
                std::iter::once(label).chain(numbers).collect()
            })
            .collect(),
        ..Table::default()
    };
    let cells = (0..rows).flat_map(|r| (1..=columns).map(move |c| (r, c)));
    let decorations = cells.map(|(row, col)| Decoration {
        row,
        col,
        fill: Some(if col % 2 == 0 { "#a9ecf5" } else { "#df6b00" }.into()),
        text: (col % 2 == 0).then(|| "#094e56".into()),
        comment: Some(format!("cell {row} {col}\nlower 0.1, upper 0.3")),
    });
    table.set_decorations(decorations.collect());
    table
}

/// Writes the table and returns the workbook's parts by name.
fn written(test: &str, table: &Table) -> std::collections::HashMap<String, String> {
    let dir = std::env::temp_dir().join(format!("statscraft-plugin-{test}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.xlsx");
    write_xlsx(&path, [Item::Table(table)], false).unwrap();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let mut parts = std::collections::HashMap::new();
    for i in 0..zip.len() {
        let mut part = zip.by_index(i).unwrap();
        let mut text = String::new();
        part.read_to_string(&mut text).unwrap();
        parts.insert(part.name().unwrap().to_string(), text);
    }
    std::fs::remove_dir_all(&dir).unwrap();
    parts
}

#[test]
fn fills_text_colours_and_comments_are_written() {
    let parts = written("small", &decorated(2, 2));
    let styles = &parts["xl/styles.xml"];
    assert!(
        styles.contains("FFA9ECF5") && styles.contains("FFDF6B00"),
        "{styles}"
    );
    assert!(styles.contains("FF094E56"), "{styles}");
    let comments = &parts["xl/comments1.xml"];
    assert_eq!(comments.matches("<comment ").count(), 4);
    // B3 is the first number: title row, header row, then the body
    assert!(comments.contains("ref=\"B3\""), "{comments}");
    assert!(comments.contains("cell 0 1"), "{comments}");
    assert!(
        parts
            .keys()
            .any(|k| k.starts_with("xl/drawings/vmlDrawing"))
    );
}

/// 100 000 comments: about 2.5 s in a release build (some 20 s in a debug one), most of it the
/// notes' drawing; run with --ignored --release.
#[test]
#[ignore = "a timing check, for release builds"]
fn a_hundred_thousand_comments_are_written_fast() {
    let table = decorated(1000, 100);
    let dir = std::env::temp_dir().join(format!("statscraft-plugin-large-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.xlsx");
    let started = std::time::Instant::now();
    write_xlsx(&path, [Item::Table(&table)], false).unwrap();
    let took = started.elapsed();
    std::fs::remove_dir_all(&dir).unwrap();
    eprintln!("100 000 comments written in {took:?}");
    assert!(took.as_secs_f64() < 5.0, "{took:?}");
}
