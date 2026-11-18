//! RESPONSE RATE on small invented data, every expected number worked out by hand.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Groups by zone after the missing and empty ones are left out:
/// 1: 6 + 4 of 50 + 50 (10%), 2: 3 + 1 of 30 + 10 (10%), 3: 2 of 40 (5%).
/// Cumulative: 10/100 = 10%, 14/140 = 10%, 16/180 = 8.888...%.
/// Zone 4 has a denominator sum of 0 and is left out; the case with buys missing, the one with
/// zone missing and the one with mailed user-missing (99) are left out.
const ZONES: &str = "DATA LIST FREE /zone buys mailed.
BEGIN DATA.
3 2 40
1 6 50
2 3 30
1 4 50
2 1 10
4 0 0
3 . 20
. 5 5
3 1 99
END DATA.
MISSING VALUES mailed (99).
";

/// Weighted, string groups. The case with weight 0 is left out. Weighted sums:
/// a: 3 of 10 (30%), b: 2*1 + 2 = 4 of 2*10 + 10 = 30 (13.33%), c: 3*1 = 3 of 3*4 = 12 (25%).
/// Cumulative: 3/10 = 30%, 7/40 = 17.5%, 10/52 = 19.2307...%.
const AREAS: &str = "DATA LIST FREE /area (A1) n d w.
BEGIN DATA.
b 1 10 2
a 3 10 1
b 2 10 1
a 1 10 0
c 1 4 3
END DATA.
WEIGHT BY w.
";

fn run(source: &str) -> (Engine, Vec<Table>) {
    let mut engine = Engine::default();
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    (engine, tables)
}

fn fails(source: &str) -> String {
    let mut engine = Engine::default();
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.to_string();
        }
    }
    panic!("{source} ran without an error");
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn label(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        other => format!("{other:?}"),
    }
}

fn assert_rows(table: &Table, want: &[(&str, [f64; 6])]) {
    assert_eq!(table.rows.len(), want.len(), "{:?}", table.rows);
    for (row, (name, values)) in table.rows.iter().zip(want) {
        assert_eq!(label(&row[0]).trim(), *name);
        for (cell, want) in row[1..].iter().zip(values) {
            let got = number(cell);
            assert!((got - want).abs() < 1e-9, "{got} != {want} in {row:?}");
        }
    }
}

/// The fill of every row, G for green and R for red, as one string.
fn fills(table: &Table) -> String {
    (0..table.rows.len())
        .map(|r| {
            let fills: Vec<&str> = (0..7)
                .map(|c| table.decoration(r, c).unwrap().fill.as_deref().unwrap())
                .collect();
            assert!(fills.iter().all(|f| *f == fills[0]));
            match fills[0] {
                "#c6efce" => 'G',
                "#ffc7ce" => 'R',
                other => panic!("{other}"),
            }
        })
        .collect()
}

fn zones(subcommands: &str) -> (Engine, Table) {
    let (engine, tables) = run(&format!(
        "{ZONES}RESPONSE RATE /VARIABLES NUMERATOR=buys DENOMINATOR=mailed GROUP=zone {subcommands}."
    ));
    assert_eq!(tables.len(), 1);
    (engine, tables.into_iter().next().unwrap())
}

#[test]
fn sums_rates_and_cumulative_rates_by_ascending_group() {
    let (mut engine, table) = zones("");
    assert_eq!(table.title, "Response Rates");
    assert_rows(
        &table,
        &[
            ("1", [10.0, 100.0, 10.0, 10.0, 100.0, 10.0]),
            ("2", [4.0, 40.0, 10.0, 14.0, 140.0, 10.0]),
            ("3", [2.0, 40.0, 5.0, 16.0, 180.0, 1600.0 / 180.0]),
        ],
    );
    // no limits: no colours and no caption
    assert!(table.decorations.is_empty() && table.caption.is_none());
    let warnings = engine.warnings().join(" | ");
    assert!(warnings.contains("is 0 or negative: 4"), "{warnings}");
    let charts = engine.take_charts();
    assert_eq!(charts.len(), 2);
    assert_eq!(charts[0].categories.len(), 3);
    assert_eq!(charts[0].series.len(), 2);
    assert_eq!(charts[0].series[0].values, [10.0, 10.0, 5.0]);
    assert_eq!(charts[1].series.len(), 1);
    assert_eq!(charts[1].series[0].values, [100.0, 140.0, 180.0]);
}

#[test]
fn limits_colour_the_rows_and_draw_reference_lines() {
    // a cumulative rate equal to the minimum keeps it
    let (_, table) = zones("/MINRATE=10");
    assert_eq!(fills(&table), "GGR");
    assert!(table.caption.as_deref().unwrap().contains("at least 10%"));
    // a cumulative sum equal to the maximum keeps it
    let (mut engine, table) = zones("/MAXCOUNT COUNT=140");
    assert_eq!(fills(&table), "GGR");
    let charts = engine.take_charts();
    assert_eq!(charts[1].series[1].values, [140.0; 3]);
    // both: the rate holds in every row, the count only in the first
    let (_, table) = zones("/MINRATE RATE=8 /MAXCOUNT=100");
    assert_eq!(fills(&table), "GRR");
    // both: the count holds in every row, the rate in the first two
    let (_, table) = zones("/MAXCOUNT=180 /MINRATE=9");
    assert_eq!(fills(&table), "GGR");
    // 80% of 180 is 144
    let (_, table) = zones("/MAXCOUNT PERCENT=80");
    assert_eq!(fills(&table), "GGR");
    assert!(table.caption.as_deref().unwrap().contains("at most 144"));
    // 1 / 20 * 100 = 5%
    let (mut engine, table) = zones("/MINRATE FORMULA=1, 20");
    assert_eq!(fills(&table), "GGG");
    let charts = engine.take_charts();
    assert_eq!(charts[0].series[2].values, [5.0; 3]);
}

#[test]
fn weights_and_string_groups() {
    let (_, tables) = run(&format!(
        "{AREAS}RESPONSE RATE /VARIABLES NUMERATOR=n DENOMINATOR=d GROUP=area /MINRATE RATE=19."
    ));
    assert_rows(
        &tables[0],
        &[
            ("a", [3.0, 10.0, 30.0, 3.0, 10.0, 30.0]),
            ("b", [4.0, 30.0, 40.0 / 3.0, 7.0, 40.0, 17.5]),
            ("c", [3.0, 12.0, 25.0, 10.0, 52.0, 1000.0 / 52.0]),
        ],
    );
    assert_eq!(fills(&tables[0]), "GRG");
}

#[test]
fn errors() {
    let error = fails(&format!(
        "{AREAS}RESPONSE RATE /VARIABLES NUMERATOR=area DENOMINATOR=d GROUP=n."
    ));
    assert!(error.contains("must be numeric"), "{error}");
    let error = fails(
        "DATA LIST FREE /g n d.\nBEGIN DATA.\n1 1 0\n2 1 -3\n2 1 2\nEND DATA.\n\
         RESPONSE RATE /VARIABLES NUMERATOR=n DENOMINATOR=d GROUP=g.",
    );
    assert!(error.contains("no group with a positive sum"), "{error}");
}
