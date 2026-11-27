//! Edge cases of RELIABILITY (model ALPHA) against R on nine invented cases: a constant item kept
//! in the scale (the manual's cheap method, used when only TOTAL is asked for) and dropped from
//! it (the covariance method, used once COVARIANCE is asked for), two items, three cases, a zero
//! weight and a weight of two, and a case with a missing item left out. Alpha from the
//! covariance matrix, the standardized alpha from the mean inter-item correlation, the item-total
//! statistics by deleting each item; where an item has no variance its correlation is empty, and
//! the standardized alpha is empty while any item in the scale has none. Scripts in
//! lab/oracles/edge-rl. The same cases with 1000000000 added to every item must give the same
//! alpha and item-total statistics, the means shifted.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /a b c d k m w (7F8.2).
BEGIN DATA.
3 2 4 1 5 . 1
5 4 3 2 5 . 2
4 4 6 2 5 . 1
9 8 7 3 5 . 0
2 3 2 4 5 . 1
8 6 7 3 5 . 1
. 7 5 5 5 . 1
6 7 8 6 5 . 1
5 4 4 5 5 . 1
END DATA.
";

const SHIFTED: &str = "DATA LIST FREE /a b c d (4F14.2).
BEGIN DATA.
1000000003 1000000002 1000000004 1000000001
1000000005 1000000004 1000000003 1000000002
1000000004 1000000004 1000000006 1000000002
1000000009 1000000008 1000000007 1000000003
1000000002 1000000003 1000000002 1000000004
1000000008 1000000006 1000000007 1000000003
. 1000000007 1000000005 1000000005
1000000006 1000000007 1000000008 1000000006
1000000005 1000000004 1000000004 1000000005
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    run_on(DATA, syntax)
}

fn run_on(data: &str, syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{data}{syntax}"));
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

/// The first cell of each row (the source).
fn row_keys(table: &Table) -> Vec<String> {
    table
        .rows
        .iter()
        .map(|row| match &row[0] {
            Cell::Text(t) => t.clone(),
            _ => String::new(),
        })
        .collect()
}

/// Each expectation: the table title, the row, the column and the value (NaN: an empty cell).
fn check(tables: &[Table], expected: &[(&str, &str, &str, f64)]) {
    for (title, row, column, want) in expected {
        let t = tables
            .iter()
            .find(|t| t.title == *title)
            .unwrap_or_else(|| panic!("no table {title}"));
        let r = row_keys(t)
            .iter()
            .position(|k| k == row)
            .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(t)));
        let c = t
            .column_titles()
            .iter()
            .position(|k| k == column)
            .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", t.column_titles()));
        if want.is_nan() {
            assert_eq!(t.rows[r][c], Cell::Empty, "{title} / {row} / {column}");
            continue;
        }
        let Cell::Number(n) = &t.rows[r][c] else {
            panic!("{title} / {row} / {column}: {:?}", t.rows[r][c]);
        };
        assert!(
            (n - want).abs() <= 1e-8 + 1e-7 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn constant_item_kept_by_the_cheap_method() {
    let t = run("RELIABILITY VARIABLES=a b c k /SCALE(SC)=ALL /MODEL=ALPHA /SUMMARY=TOTAL.");
    check(
        &t,
        &[
            (
                "Reliability Statistics",
                "",
                "Cronbach's Alpha",
                0.824355971897,
            ),
            ("Reliability Statistics", "", "N of Items", 4.0),
            (
                "Reliability Statistics",
                "",
                "Cronbach's Alpha Based on Standardized Items",
                f64::NAN,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Scale Mean if Item Deleted",
                14.875,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Scale Variance if Item Deleted",
                16.125,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Corrected Item-Total Correlation",
                0.857389304371,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Cronbach's Alpha if Item Deleted",
                0.671096345515,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Scale Mean if Item Deleted",
                15.375,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Scale Variance if Item Deleted",
                17.9821428571,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Corrected Item-Total Correlation",
                0.914879975295,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Cronbach's Alpha if Item Deleted",
                0.637537239325,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Scale Mean if Item Deleted",
                15.0,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Scale Variance if Item Deleted",
                18.5714285714,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Corrected Item-Total Correlation",
                0.795424176004,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Cronbach's Alpha if Item Deleted",
                0.703846153846,
            ),
            (
                "Item-Total Statistics",
                "k",
                "Scale Mean if Item Deleted",
                15.125,
            ),
            (
                "Item-Total Statistics",
                "k",
                "Scale Variance if Item Deleted",
                38.125,
            ),
            (
                "Item-Total Statistics",
                "k",
                "Corrected Item-Total Correlation",
                f64::NAN,
            ),
            (
                "Item-Total Statistics",
                "k",
                "Cronbach's Alpha if Item Deleted",
                0.927400468384,
            ),
        ],
    );
}

#[test]
fn constant_item_dropped_by_the_covariance_method() {
    let t =
        run("RELIABILITY VARIABLES=a b c k /SCALE(SC)=ALL /MODEL=ALPHA /SUMMARY=TOTAL COVARIANCE.");
    check(
        &t,
        &[
            (
                "Reliability Statistics",
                "",
                "Cronbach's Alpha",
                0.927400468384,
            ),
            ("Reliability Statistics", "", "N of Items", 3.0),
            (
                "Reliability Statistics",
                "",
                "Cronbach's Alpha Based on Standardized Items",
                0.929891945162,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Scale Mean if Item Deleted",
                9.875,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Scale Variance if Item Deleted",
                16.125,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Corrected Item-Total Correlation",
                0.857389304371,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Cronbach's Alpha if Item Deleted",
                0.894795127353,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Scale Mean if Item Deleted",
                10.375,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Scale Variance if Item Deleted",
                17.9821428571,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Corrected Item-Total Correlation",
                0.914879975295,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Cronbach's Alpha if Item Deleted",
                0.850049652433,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Scale Mean if Item Deleted",
                10.0,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Scale Variance if Item Deleted",
                18.5714285714,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Corrected Item-Total Correlation",
                0.795424176004,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Cronbach's Alpha if Item Deleted",
                0.938461538462,
            ),
        ],
    );
}

#[test]
fn two_items() {
    let t = run("RELIABILITY VARIABLES=a b /SCALE(SC)=ALL /MODEL=ALPHA /SUMMARY=TOTAL.");
    check(
        &t,
        &[
            (
                "Reliability Statistics",
                "",
                "Cronbach's Alpha",
                0.938461538462,
            ),
            ("Reliability Statistics", "", "N of Items", 2.0),
            (
                "Reliability Statistics",
                "",
                "Cronbach's Alpha Based on Standardized Items",
                0.943750046899,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Scale Mean if Item Deleted",
                4.75,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Scale Variance if Item Deleted",
                4.21428571429,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Corrected Item-Total Correlation",
                0.893491208333,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Cronbach's Alpha if Item Deleted",
                f64::NAN,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Scale Mean if Item Deleted",
                5.25,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Scale Variance if Item Deleted",
                5.64285714286,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Corrected Item-Total Correlation",
                0.893491208333,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Cronbach's Alpha if Item Deleted",
                f64::NAN,
            ),
        ],
    );
}

#[test]
fn three_cases() {
    let t = run(
        "COMPUTE kk=$CASENUM.\nTEMPORARY.\nSELECT IF (kk <= 3).\nRELIABILITY VARIABLES=a b c /SCALE(SC)=ALL /MODEL=ALPHA /SUMMARY=TOTAL.",
    );
    check(
        &t,
        &[
            (
                "Reliability Statistics",
                "",
                "Cronbach's Alpha",
                0.394736842105,
            ),
            ("Reliability Statistics", "", "N of Items", 3.0),
            (
                "Reliability Statistics",
                "",
                "Cronbach's Alpha Based on Standardized Items",
                0.489980972581,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Scale Mean if Item Deleted",
                7.66666666667,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Scale Variance if Item Deleted",
                4.33333333333,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Corrected Item-Total Correlation",
                0.240192230708,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Cronbach's Alpha if Item Deleted",
                0.307692307692,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Scale Mean if Item Deleted",
                8.33333333333,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Scale Variance if Item Deleted",
                2.33333333333,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Corrected Item-Total Correlation",
                0.755928946018,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Cronbach's Alpha if Item Deleted",
                -0.857142857143,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Scale Mean if Item Deleted",
                7.33333333333,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Scale Variance if Item Deleted",
                4.33333333333,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Corrected Item-Total Correlation",
                -0.0524142418361,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Cronbach's Alpha if Item Deleted",
                0.923076923077,
            ),
        ],
    );
}

#[test]
fn zero_weight_and_weight_of_two() {
    let t = run(
        "WEIGHT BY w.\nRELIABILITY VARIABLES=a b c d /SCALE(SC)=ALL /MODEL=ALPHA /SUMMARY=TOTAL.",
    );
    check(
        &t,
        &[
            (
                "Reliability Statistics",
                "",
                "Cronbach's Alpha",
                0.840236686391,
            ),
            ("Reliability Statistics", "", "N of Items", 4.0),
            (
                "Reliability Statistics",
                "",
                "Cronbach's Alpha Based on Standardized Items",
                0.84756359938,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Scale Mean if Item Deleted",
                12.0,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Scale Variance if Item Deleted",
                21.4285714286,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Corrected Item-Total Correlation",
                0.673721536853,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Cronbach's Alpha if Item Deleted",
                0.7975,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Scale Mean if Item Deleted",
                12.5,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Scale Variance if Item Deleted",
                20.2857142857,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Corrected Item-Total Correlation",
                0.942832753785,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Cronbach's Alpha if Item Deleted",
                0.694542253521,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Scale Mean if Item Deleted",
                12.125,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Scale Variance if Item Deleted",
                18.4107142857,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Corrected Item-Total Correlation",
                0.723559894482,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Cronbach's Alpha if Item Deleted",
                0.779825412221,
            ),
            (
                "Item-Total Statistics",
                "d",
                "Scale Mean if Item Deleted",
                13.625,
            ),
            (
                "Item-Total Statistics",
                "d",
                "Scale Variance if Item Deleted",
                25.6964285714,
            ),
            (
                "Item-Total Statistics",
                "d",
                "Corrected Item-Total Correlation",
                0.43042136729,
            ),
            (
                "Item-Total Statistics",
                "d",
                "Cronbach's Alpha if Item Deleted",
                0.892286309937,
            ),
        ],
    );
}

#[test]
fn items_near_one_billion_keep_their_digits() {
    let t = run_on(
        SHIFTED,
        "RELIABILITY VARIABLES=a b c d /SCALE(SC)=ALL /MODEL=ALPHA /SUMMARY=TOTAL.",
    );
    check(
        &t,
        &[
            (
                "Reliability Statistics",
                "",
                "Cronbach's Alpha",
                0.842743463433,
            ),
            ("Reliability Statistics", "", "N of Items", 4.0),
            (
                "Reliability Statistics",
                "",
                "Cronbach's Alpha Based on Standardized Items",
                0.831433369123,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Scale Mean if Item Deleted",
                3000000013.12,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Scale Variance if Item Deleted",
                23.8392857143,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Corrected Item-Total Correlation",
                0.760576276089,
            ),
            (
                "Item-Total Statistics",
                "a",
                "Cronbach's Alpha if Item Deleted",
                0.76404494382,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Scale Mean if Item Deleted",
                3000000013.62,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Scale Variance if Item Deleted",
                23.9821428571,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Corrected Item-Total Correlation",
                0.941416621304,
            ),
            (
                "Item-Total Statistics",
                "b",
                "Cronbach's Alpha if Item Deleted",
                0.679076693969,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Scale Mean if Item Deleted",
                3000000013.25,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Scale Variance if Item Deleted",
                25.6428571429,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Corrected Item-Total Correlation",
                0.764790991517,
            ),
            (
                "Item-Total Statistics",
                "c",
                "Cronbach's Alpha if Item Deleted",
                0.760445682451,
            ),
            (
                "Item-Total Statistics",
                "d",
                "Scale Mean if Item Deleted",
                3000000015.12,
            ),
            (
                "Item-Total Statistics",
                "d",
                "Scale Variance if Item Deleted",
                38.125,
            ),
            (
                "Item-Total Statistics",
                "d",
                "Corrected Item-Total Correlation",
                0.301500481066,
            ),
            (
                "Item-Total Statistics",
                "d",
                "Cronbach's Alpha if Item Deleted",
                0.927400468384,
            ),
        ],
    );
}
