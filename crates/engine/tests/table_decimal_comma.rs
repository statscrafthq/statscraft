//! Tables under SET DECIMAL=COMMA: the numbers a procedure makes show with a decimal comma where
//! the formats that take the setting's mark show them, in the window, in Markdown and in the
//! exports, which all read `cell_text`. The numbers themselves stay raw in JSON.

use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table, cell_text};

const DATA: &str = "DATA LIST LIST /a b g.
BEGIN DATA
1.5 2.25 1
2.5 3.5 2
3.25 1.75 1
4.5 5.5 2
5.75 4.25 1
6.5 6.125 2
END DATA.
";

const COMMANDS: &str = "DESCRIPTIVES a b.
FREQUENCIES a /FORMAT=LIMIT(10).
CORRELATIONS a b.
T-TEST GROUPS=g(1,2) /VARIABLES=a.
LIST a b.
REGRESSION /DEPENDENT=b /METHOD=ENTER a.
ONEWAY a BY g.
";

fn tables(setting: &str) -> Vec<Table> {
    let mut session = Session::default();
    let source = format!("{DATA}{setting}{COMMANDS}");
    let result = session
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    result.commands.into_iter().flat_map(|c| c.tables).collect()
}

/// Every number of every table as text.
fn numbers(tables: &[Table]) -> Vec<String> {
    let mut texts = Vec::new();
    for table in tables {
        for (r, row) in table.rows.iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                if matches!(cell, Cell::Number(_)) {
                    texts.push(cell_text(table, r, c));
                }
            }
        }
    }
    texts
}

#[test]
fn every_number_of_every_table_shows_a_comma_where_it_showed_a_period() {
    let dot = numbers(&tables(""));
    let comma = numbers(&tables("SET DECIMAL=COMMA.\n"));
    assert_eq!(dot.len(), comma.len());
    assert!(dot.len() > 100, "{} numbers", dot.len());
    let mut changed = 0;
    for (d, c) in dot.iter().zip(&comma) {
        // a format that takes the mark shows the comma where the period was; the others (COMMA,
        // DOT, DOLLAR) keep their own marks
        if d != c {
            assert_eq!(&d.replace('.', ","), c, "{d:?} against {c:?}");
            changed += 1;
        }
    }
    assert!(changed > 50, "{changed} numbers changed");
    // no number with a decimal point is left, other than in the formats that own it
    let periods = comma.iter().filter(|t| t.contains('.')).count();
    assert!(
        periods <= dot.len() / 10,
        "{periods} numbers still have a period"
    );
}

#[test]
fn a_few_cells_by_name() {
    let comma = tables("SET DECIMAL=COMMA.\n");
    let descriptives = &comma[0];
    let at = |table: &Table, row: usize, name: &str| {
        let column = table.columns.iter().position(|c| c == name).unwrap();
        cell_text(table, row, column)
    };
    assert_eq!(at(descriptives, 1, "Mean"), "3,90");
    assert_eq!(at(descriptives, 1, "Minimum"), "1,75");
    assert_eq!(at(descriptives, 0, "Minimum"), "1,50");
    // a count has no mark at all
    assert_eq!(at(descriptives, 0, "N"), "6");
    // the numbers in JSON stay raw
    let json = serde_json::to_value(descriptives).unwrap();
    assert_eq!(json["decimal_comma"], true);
    let raw = json["rows"][1].as_array().unwrap();
    assert!(
        raw.iter()
            .any(|v| v.as_f64().is_some_and(|x| (x - 23.375 / 6.0).abs() < 1e-12))
    );
}

#[test]
fn tables_made_before_the_setting_and_after_it_is_reset_keep_the_period() {
    let mut session = Session::default();
    let run = |session: &mut Session, source: &str| -> Vec<Table> {
        let result = session
            .run(source, None)
            .unwrap_or_else(|e| panic!("{e:?}"));
        result.commands.into_iter().flat_map(|c| c.tables).collect()
    };
    run(&mut session, DATA);
    let before = run(&mut session, "DESCRIPTIVES b.");
    run(&mut session, "SET DECIMAL=COMMA.");
    let during = run(&mut session, "DESCRIPTIVES b.");
    run(&mut session, "SET DECIMAL=DOT.");
    let after = run(&mut session, "DESCRIPTIVES b.");
    let mean = |tables: &[Table]| {
        let table = &tables[0];
        let column = table.columns.iter().position(|c| c == "Mean").unwrap();
        cell_text(table, 0, column)
    };
    assert_eq!(mean(&before), "3.90");
    assert_eq!(mean(&during), "3,90");
    assert_eq!(mean(&after), "3.90");
    // the flag is not written when it is off
    let json = serde_json::to_value(&before[0]).unwrap();
    assert!(json.get("decimal_comma").is_none());
}

#[test]
fn the_markdown_of_a_run_shows_the_comma() {
    let mut session = Session::default();
    let result = session
        .run(
            &format!(
                "{DATA}SET DECIMAL=COMMA.\nDESCRIPTIVES a b.\nFREQUENCIES a /FORMAT=LIMIT(10)."
            ),
            None,
        )
        .unwrap();
    let text = result.output(false, false);
    assert!(text.contains("| 3,90 |"), "{text}");
    assert!(text.contains("| 16,67 |"), "{text}");
    assert!(!text.contains("3.90") && !text.contains("16.67"), "{text}");
}
