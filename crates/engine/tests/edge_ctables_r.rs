//! Edge cases of CTABLES against R on ten invented cases: a cell of one case, a cell with no case
//! (its only case has a zero weight), a constant scale variable, a case whose category is
//! missing (left out of the table and of every percentage), a zero weight and a weight of two
//! (cases repeated as often as their weight): counts, the three percentages, and the mean,
//! standard deviation, variance, sum, median (at (W + 1) p), minimum and maximum of a scale
//! variable, empty where a cell has too few cases. With 1000000000 added to the scale
//! variable the standard deviation, the variance and the range are those of the small values. Scripts in lab/oracles/edge-ct.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g h x c w (5F8.2).
BEGIN DATA.
1 1 3 5 1
1 1 7 5 2
1 2 4 5 1
2 1 9 5 0
2 2 2 5 1
2 2 8 5 1
3 1 5 5 1
3 2 6 5 1
. 1 4 5 1
1 . 3 5 1
END DATA.
VARIABLE LEVEL x c (SCALE) /g h (NOMINAL).
";

const SHIFTED: &str = "DATA LIST FREE /g h x c w (5F14.2).
BEGIN DATA.
1 1 1000000003.00 5 1
1 1 1000000007.00 5 2
1 2 1000000004.00 5 1
2 1 1000000009.00 5 0
2 2 1000000002.00 5 1
2 2 1000000008.00 5 1
3 1 1000000005.00 5 1
3 2 1000000006.00 5 1
. 1 1000000004.00 5 1
1 . 1000000003.00 5 1
END DATA.
VARIABLE LEVEL x c (SCALE) /g h (NOMINAL).
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

/// The leading text cells of each row (the category and, for a scale variable, its name)
/// joined with " / ".
fn row_keys(table: &Table) -> Vec<String> {
    table
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map_while(|c| match c {
                    Cell::Text(t) => Some(t.clone()),
                    _ => None,
                })
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" / ")
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
            (n - want).abs() <= 1e-8 + 1e-8 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn counts_and_percentages_with_an_empty_cell_and_a_missing_category() {
    let t = run("CTABLES /TABLE g BY h [COUNT, COLPCT.COUNT, ROWPCT.COUNT, TABLEPCT.COUNT].");
    check(
        &t,
        &[
            ("Custom Tables", "g / 1.00", "h / 1.00 / Count", 2.0),
            ("Custom Tables", "g / 1.00", "h / 1.00 / Column %", 50.0),
            (
                "Custom Tables",
                "g / 1.00",
                "h / 1.00 / Row %",
                66.6666666667,
            ),
            ("Custom Tables", "g / 1.00", "h / 1.00 / Table %", 25.0),
            ("Custom Tables", "g / 1.00", "h / 2.00 / Count", 1.0),
            ("Custom Tables", "g / 1.00", "h / 2.00 / Column %", 25.0),
            (
                "Custom Tables",
                "g / 1.00",
                "h / 2.00 / Row %",
                33.3333333333,
            ),
            ("Custom Tables", "g / 1.00", "h / 2.00 / Table %", 12.5),
            ("Custom Tables", "g / 2.00", "h / 1.00 / Count", 1.0),
            ("Custom Tables", "g / 2.00", "h / 1.00 / Column %", 25.0),
            (
                "Custom Tables",
                "g / 2.00",
                "h / 1.00 / Row %",
                33.3333333333,
            ),
            ("Custom Tables", "g / 2.00", "h / 1.00 / Table %", 12.5),
            ("Custom Tables", "g / 2.00", "h / 2.00 / Count", 2.0),
            ("Custom Tables", "g / 2.00", "h / 2.00 / Column %", 50.0),
            (
                "Custom Tables",
                "g / 2.00",
                "h / 2.00 / Row %",
                66.6666666667,
            ),
            ("Custom Tables", "g / 2.00", "h / 2.00 / Table %", 25.0),
            ("Custom Tables", "g / 3.00", "h / 1.00 / Count", 1.0),
            ("Custom Tables", "g / 3.00", "h / 1.00 / Column %", 25.0),
            ("Custom Tables", "g / 3.00", "h / 1.00 / Row %", 50.0),
            ("Custom Tables", "g / 3.00", "h / 1.00 / Table %", 12.5),
            ("Custom Tables", "g / 3.00", "h / 2.00 / Count", 1.0),
            ("Custom Tables", "g / 3.00", "h / 2.00 / Column %", 25.0),
            ("Custom Tables", "g / 3.00", "h / 2.00 / Row %", 50.0),
            ("Custom Tables", "g / 3.00", "h / 2.00 / Table %", 12.5),
        ],
    );
}

#[test]
fn summaries_with_cells_of_one_case_and_none() {
    let t =
        run("CTABLES /TABLE g BY h > x [MEAN, STDDEV, SUM, MEDIAN, VARIANCE, MINIMUM, MAXIMUM].");
    check(
        &t,
        &[
            ("Custom Tables", "g / 1.00", "h / 1.00 / x / Mean", 5.0),
            (
                "Custom Tables",
                "g / 1.00",
                "h / 1.00 / x / Std Deviation",
                2.82842712475,
            ),
            ("Custom Tables", "g / 1.00", "h / 1.00 / x / Sum", 10.0),
            ("Custom Tables", "g / 1.00", "h / 1.00 / x / Median", 5.0),
            ("Custom Tables", "g / 1.00", "h / 1.00 / x / Variance", 8.0),
            ("Custom Tables", "g / 1.00", "h / 1.00 / x / Minimum", 3.0),
            ("Custom Tables", "g / 1.00", "h / 1.00 / x / Maximum", 7.0),
            ("Custom Tables", "g / 1.00", "h / 2.00 / x / Mean", 4.0),
            (
                "Custom Tables",
                "g / 1.00",
                "h / 2.00 / x / Std Deviation",
                f64::NAN,
            ),
            ("Custom Tables", "g / 1.00", "h / 2.00 / x / Sum", 4.0),
            ("Custom Tables", "g / 1.00", "h / 2.00 / x / Median", 4.0),
            (
                "Custom Tables",
                "g / 1.00",
                "h / 2.00 / x / Variance",
                f64::NAN,
            ),
            ("Custom Tables", "g / 1.00", "h / 2.00 / x / Minimum", 4.0),
            ("Custom Tables", "g / 1.00", "h / 2.00 / x / Maximum", 4.0),
            ("Custom Tables", "g / 2.00", "h / 1.00 / x / Mean", 9.0),
            (
                "Custom Tables",
                "g / 2.00",
                "h / 1.00 / x / Std Deviation",
                f64::NAN,
            ),
            ("Custom Tables", "g / 2.00", "h / 1.00 / x / Sum", 9.0),
            ("Custom Tables", "g / 2.00", "h / 1.00 / x / Median", 9.0),
            (
                "Custom Tables",
                "g / 2.00",
                "h / 1.00 / x / Variance",
                f64::NAN,
            ),
            ("Custom Tables", "g / 2.00", "h / 1.00 / x / Minimum", 9.0),
            ("Custom Tables", "g / 2.00", "h / 1.00 / x / Maximum", 9.0),
            ("Custom Tables", "g / 2.00", "h / 2.00 / x / Mean", 5.0),
            (
                "Custom Tables",
                "g / 2.00",
                "h / 2.00 / x / Std Deviation",
                4.24264068712,
            ),
            ("Custom Tables", "g / 2.00", "h / 2.00 / x / Sum", 10.0),
            ("Custom Tables", "g / 2.00", "h / 2.00 / x / Median", 5.0),
            ("Custom Tables", "g / 2.00", "h / 2.00 / x / Variance", 18.0),
            ("Custom Tables", "g / 2.00", "h / 2.00 / x / Minimum", 2.0),
            ("Custom Tables", "g / 2.00", "h / 2.00 / x / Maximum", 8.0),
            ("Custom Tables", "g / 3.00", "h / 1.00 / x / Mean", 5.0),
            (
                "Custom Tables",
                "g / 3.00",
                "h / 1.00 / x / Std Deviation",
                f64::NAN,
            ),
            ("Custom Tables", "g / 3.00", "h / 1.00 / x / Sum", 5.0),
            ("Custom Tables", "g / 3.00", "h / 1.00 / x / Median", 5.0),
            (
                "Custom Tables",
                "g / 3.00",
                "h / 1.00 / x / Variance",
                f64::NAN,
            ),
            ("Custom Tables", "g / 3.00", "h / 1.00 / x / Minimum", 5.0),
            ("Custom Tables", "g / 3.00", "h / 1.00 / x / Maximum", 5.0),
            ("Custom Tables", "g / 3.00", "h / 2.00 / x / Mean", 6.0),
            (
                "Custom Tables",
                "g / 3.00",
                "h / 2.00 / x / Std Deviation",
                f64::NAN,
            ),
            ("Custom Tables", "g / 3.00", "h / 2.00 / x / Sum", 6.0),
            ("Custom Tables", "g / 3.00", "h / 2.00 / x / Median", 6.0),
            (
                "Custom Tables",
                "g / 3.00",
                "h / 2.00 / x / Variance",
                f64::NAN,
            ),
            ("Custom Tables", "g / 3.00", "h / 2.00 / x / Minimum", 6.0),
            ("Custom Tables", "g / 3.00", "h / 2.00 / x / Maximum", 6.0),
        ],
    );
}

#[test]
fn weighted_counts_with_a_zero_weight_and_a_weight_of_two() {
    let t = run("WEIGHT BY w.
CTABLES /TABLE g BY h [COUNT, COLPCT.COUNT].");
    check(
        &t,
        &[
            ("Custom Tables", "g / 1.00", "h / 1.00 / Count", 3.0),
            ("Custom Tables", "g / 1.00", "h / 1.00 / Column %", 75.0),
            ("Custom Tables", "g / 1.00", "h / 2.00 / Count", 1.0),
            ("Custom Tables", "g / 1.00", "h / 2.00 / Column %", 25.0),
            ("Custom Tables", "g / 2.00", "h / 1.00 / Count", 0.0),
            ("Custom Tables", "g / 2.00", "h / 1.00 / Column %", 0.0),
            ("Custom Tables", "g / 2.00", "h / 2.00 / Count", 2.0),
            ("Custom Tables", "g / 2.00", "h / 2.00 / Column %", 50.0),
            ("Custom Tables", "g / 3.00", "h / 1.00 / Count", 1.0),
            ("Custom Tables", "g / 3.00", "h / 1.00 / Column %", 25.0),
            ("Custom Tables", "g / 3.00", "h / 2.00 / Count", 1.0),
            ("Custom Tables", "g / 3.00", "h / 2.00 / Column %", 25.0),
        ],
    );
}

#[test]
fn weighted_summaries() {
    let t = run("WEIGHT BY w.
CTABLES /TABLE g BY h > x [MEAN, STDDEV, SUM, MEDIAN].");
    check(
        &t,
        &[
            (
                "Custom Tables",
                "g / 1.00",
                "h / 1.00 / x / Mean",
                5.66666666667,
            ),
            (
                "Custom Tables",
                "g / 1.00",
                "h / 1.00 / x / Std Deviation",
                2.30940107676,
            ),
            ("Custom Tables", "g / 1.00", "h / 1.00 / x / Sum", 17.0),
            ("Custom Tables", "g / 1.00", "h / 1.00 / x / Median", 7.0),
            ("Custom Tables", "g / 1.00", "h / 2.00 / x / Mean", 4.0),
            (
                "Custom Tables",
                "g / 1.00",
                "h / 2.00 / x / Std Deviation",
                f64::NAN,
            ),
            ("Custom Tables", "g / 1.00", "h / 2.00 / x / Sum", 4.0),
            ("Custom Tables", "g / 1.00", "h / 2.00 / x / Median", 4.0),
            ("Custom Tables", "g / 2.00", "h / 1.00 / x / Mean", f64::NAN),
            (
                "Custom Tables",
                "g / 2.00",
                "h / 1.00 / x / Std Deviation",
                f64::NAN,
            ),
            ("Custom Tables", "g / 2.00", "h / 1.00 / x / Sum", f64::NAN),
            (
                "Custom Tables",
                "g / 2.00",
                "h / 1.00 / x / Median",
                f64::NAN,
            ),
            ("Custom Tables", "g / 2.00", "h / 2.00 / x / Mean", 5.0),
            (
                "Custom Tables",
                "g / 2.00",
                "h / 2.00 / x / Std Deviation",
                4.24264068712,
            ),
            ("Custom Tables", "g / 2.00", "h / 2.00 / x / Sum", 10.0),
            ("Custom Tables", "g / 2.00", "h / 2.00 / x / Median", 5.0),
            ("Custom Tables", "g / 3.00", "h / 1.00 / x / Mean", 5.0),
            (
                "Custom Tables",
                "g / 3.00",
                "h / 1.00 / x / Std Deviation",
                f64::NAN,
            ),
            ("Custom Tables", "g / 3.00", "h / 1.00 / x / Sum", 5.0),
            ("Custom Tables", "g / 3.00", "h / 1.00 / x / Median", 5.0),
            ("Custom Tables", "g / 3.00", "h / 2.00 / x / Mean", 6.0),
            (
                "Custom Tables",
                "g / 3.00",
                "h / 2.00 / x / Std Deviation",
                f64::NAN,
            ),
            ("Custom Tables", "g / 3.00", "h / 2.00 / x / Sum", 6.0),
            ("Custom Tables", "g / 3.00", "h / 2.00 / x / Median", 6.0),
        ],
    );
}

#[test]
fn constant_scale_variable() {
    let t = run("CTABLES /TABLE g > c [MEAN, STDDEV, VARIANCE, SUM, MEDIAN].");
    check(
        &t,
        &[
            ("Custom Tables", "g / 1.00 / c", "Mean", 5.0),
            ("Custom Tables", "g / 1.00 / c", "Std Deviation", 0.0),
            ("Custom Tables", "g / 1.00 / c", "Variance", 0.0),
            ("Custom Tables", "g / 1.00 / c", "Sum", 20.0),
            ("Custom Tables", "g / 1.00 / c", "Median", 5.0),
            ("Custom Tables", "g / 2.00 / c", "Mean", 5.0),
            ("Custom Tables", "g / 2.00 / c", "Std Deviation", 0.0),
            ("Custom Tables", "g / 2.00 / c", "Variance", 0.0),
            ("Custom Tables", "g / 2.00 / c", "Sum", 15.0),
            ("Custom Tables", "g / 2.00 / c", "Median", 5.0),
            ("Custom Tables", "g / 3.00 / c", "Mean", 5.0),
            ("Custom Tables", "g / 3.00 / c", "Std Deviation", 0.0),
            ("Custom Tables", "g / 3.00 / c", "Variance", 0.0),
            ("Custom Tables", "g / 3.00 / c", "Sum", 10.0),
            ("Custom Tables", "g / 3.00 / c", "Median", 5.0),
        ],
    );
}

#[test]
fn scale_variable_near_one_billion_keeps_its_spread() {
    let t = run_on(SHIFTED, "CTABLES /TABLE g BY h > x [STDDEV, VARIANCE].");
    check(
        &t,
        &[
            (
                "Custom Tables",
                "g / 1.00",
                "h / 1.00 / x / Std Deviation",
                2.82842712475,
            ),
            ("Custom Tables", "g / 1.00", "h / 1.00 / x / Variance", 8.0),
            (
                "Custom Tables",
                "g / 1.00",
                "h / 2.00 / x / Std Deviation",
                f64::NAN,
            ),
            (
                "Custom Tables",
                "g / 1.00",
                "h / 2.00 / x / Variance",
                f64::NAN,
            ),
            (
                "Custom Tables",
                "g / 2.00",
                "h / 1.00 / x / Std Deviation",
                f64::NAN,
            ),
            (
                "Custom Tables",
                "g / 2.00",
                "h / 1.00 / x / Variance",
                f64::NAN,
            ),
            (
                "Custom Tables",
                "g / 2.00",
                "h / 2.00 / x / Std Deviation",
                4.24264068712,
            ),
            ("Custom Tables", "g / 2.00", "h / 2.00 / x / Variance", 18.0),
            (
                "Custom Tables",
                "g / 3.00",
                "h / 1.00 / x / Std Deviation",
                f64::NAN,
            ),
            (
                "Custom Tables",
                "g / 3.00",
                "h / 1.00 / x / Variance",
                f64::NAN,
            ),
            (
                "Custom Tables",
                "g / 3.00",
                "h / 2.00 / x / Std Deviation",
                f64::NAN,
            ),
            (
                "Custom Tables",
                "g / 3.00",
                "h / 2.00 / x / Variance",
                f64::NAN,
            ),
        ],
    );
}
