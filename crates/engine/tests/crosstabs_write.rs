//! CROSSTABS /WRITE through PROCEDURE OUTPUT: the records of the cells, 80-column fixed fields as
//! the manual lays them out (split-file group, table number and frequency as integers, then the
//! values of the row, column and control variables in 8-column fields), checked by hand on a few
//! cases small enough to count.

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

fn folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "statscraft-xtabs-write-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const DATA: &str = "DATA LIST FREE / x y z.
BEGIN DATA
1 1 1
1 1 2
1 1 1
1 2 1
2 1 2
2 1 2
2 2 1
. 1 1
END DATA.
";

fn lines(path: &std::path::Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(String::from)
        .collect()
}

#[test]
fn cells_are_written_a_column_at_a_time_with_the_print_format_of_each_value() {
    let dir = folder("cells");
    let file = dir.join("cells.txt");
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{DATA}PROCEDURE OUTPUT OUTFILE='{}'.\nCROSSTABS x BY y /WRITE=CELLS.",
            file.display()
        ),
    );
    // the summary of the file comes first
    assert_eq!(tables[0].title, "Output File Summary");
    assert_eq!(tables[0].rows[1][1], Cell::Number(4.0));
    // column y = 1: x = 1 three cases, x = 2 two; column y = 2: x = 1 one, x = 2 one. The case
    // with a missing x is no cell. Values print as F8.2.
    assert_eq!(
        lines(&file),
        [
            "   1   1       3    1.00    1.00",
            "   1   1       2    2.00    1.00",
            "   1   1       1    1.00    2.00",
            "   1   1       1    2.00    2.00",
        ]
    );
}

#[test]
fn integer_mode_all_writes_the_empty_cells_and_every_layer() {
    let dir = folder("all");
    let file = dir.join("all.txt");
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{DATA}PROCEDURE OUTPUT OUTFILE='{}'.\nCROSSTABS VARIABLES=x y z (1,2) /TABLES=x BY y BY z /WRITE=ALL.",
            file.display()
        ),
    );
    assert_eq!(tables[0].rows[1][1], Cell::Number(8.0));
    // layer z = 1 first (the last control variable changes slowest), columns y = 1, 2, rows x = 1, 2
    assert_eq!(
        lines(&file),
        [
            "   1   1       2       1       1       1",
            "   1   1       0       2       1       1",
            "   1   1       1       1       2       1",
            "   1   1       1       2       2       1",
            "   1   1       1       1       1       2",
            "   1   1       2       2       1       2",
            "   1   1       0       1       2       2",
            "   1   1       0       2       2       2",
        ]
    );
    // CELLS in the same table leaves the three empty cells out
    run(
        &mut engine,
        "CROSSTABS VARIABLES=x y z (1,2) /TABLES=x BY y BY z /WRITE=CELLS.",
    );
    assert_eq!(lines(&file).len(), 5);
}

#[test]
fn a_split_file_numbers_its_groups_and_every_table_has_a_number() {
    let dir = folder("split");
    let file = dir.join("split.txt");
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "{DATA}PROCEDURE OUTPUT OUTFILE='{}'.\nSORT CASES BY z.\nSPLIT FILE BY z.\nCROSSTABS /TABLES=x BY y /TABLES=y BY x /WRITE=CELLS.",
            file.display()
        ),
    );
    let all = lines(&file);
    // group 1 (z = 1): table 1 then table 2; group 2 (z = 2) after it
    let key = |l: &String| (l[0..4].trim().to_string(), l[4..8].trim().to_string());
    let order: Vec<(String, String)> = all.iter().map(key).collect();
    assert_eq!(order.first().unwrap(), &("1".to_string(), "1".to_string()));
    assert_eq!(order.last().unwrap(), &("2".to_string(), "2".to_string()));
    assert!(order.iter().any(|(g, t)| g == "1" && t == "2"));
    assert!(order.iter().any(|(g, t)| g == "2" && t == "1"));
    // every record is 4 + 4 + 8 + 2 * 8 = 32 columns wide
    assert!(all.iter().all(|l| l.len() == 32));
}

#[test]
fn write_needs_a_procedure_output_and_the_manuals_limits() {
    let mut engine = Engine::default();
    run(&mut engine, DATA);
    let (commands, errors) = parse("CROSSTABS x BY y /WRITE.");
    assert!(errors.is_empty());
    let error = engine.execute(&commands[0].command).unwrap_err();
    assert!(error.0.contains("PROCEDURE OUTPUT"), "{error}");
    // ALL is for integer mode only
    let (_, errors) = parse("CROSSTABS x BY y /WRITE=ALL.");
    assert!(errors[0].message.contains("integer mode"));
    let dir = folder("long");
    run(
        &mut engine,
        &format!(
            "STRING s (A12).\nCOMPUTE s = 'abcdefghijkl'.\nPROCEDURE OUTPUT OUTFILE='{}/long.txt'.",
            dir.display()
        ),
    );
    let (commands, _) = parse("CROSSTABS s BY y /WRITE=CELLS.");
    let error = engine.execute(&commands[0].command).unwrap_err();
    assert!(error.0.contains("long string"), "{error}");
}

#[test]
fn layout_words_of_format_are_read_and_say_they_change_nothing() {
    let mut engine = Engine::default();
    run(&mut engine, DATA);
    let tables = run(&mut engine, "CROSSTABS x BY y /FORMAT=NOBOX INDEX AVALUE.");
    assert!(!tables.is_empty());
    assert!(engine.warnings()[0].contains("NOBOX INDEX"));
}
