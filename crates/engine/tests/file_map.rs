//! /MAP of GET and SAVE: the list of the variables with their names on the other side, as the
//! options before each /MAP leave them.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn run(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(t) => t,
        other => panic!("{other:?}"),
    }
}

fn pairs(table: &Table) -> Vec<(&str, &str)> {
    table
        .rows
        .iter()
        .map(|r| (text(&r[0]), text(&r[1])))
        .collect()
}

#[test]
fn save_lists_the_file_names_after_each_rename_and_drop_before_the_map() {
    let dir = std::env::temp_dir().join(format!("statscraft-file-map-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "DATA LIST FREE / a b c.\nBEGIN DATA\n1 2 3\nEND DATA.\n\
             SAVE OUTFILE='{}/m.sav' /RENAME=(a=x) /MAP /DROP=c /MAP.",
            dir.display()
        ),
    );
    assert_eq!(tables.len(), 2);
    assert_eq!(tables[0].title, "Variables in the file");
    assert_eq!(tables[0].columns, ["Name in the file", "Variable"]);
    assert_eq!(pairs(&tables[0]), [("x", "a"), ("b", "b"), ("c", "c")]);
    assert_eq!(pairs(&tables[1]), [("x", "a"), ("b", "b")]);
    // the file was written with the options, and GET maps it back
    let tables = run(
        &mut engine,
        &format!("GET FILE='{}/m.sav' /RENAME=(x=p) /MAP.", dir.display()),
    );
    assert_eq!(tables[0].title, "Variables in the active dataset");
    assert_eq!(tables[0].columns, ["Variable", "Name in the file"]);
    assert_eq!(pairs(&tables[0]), [("p", "x"), ("b", "b")]);
    // without MAP no table
    let tables = run(&mut engine, &format!("GET FILE='{}/m.sav'.", dir.display()));
    assert!(tables.is_empty());
}
