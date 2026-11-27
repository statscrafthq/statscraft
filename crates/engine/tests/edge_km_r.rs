//! Edge cases and numerical stability of KM against R (`survival::survfit` and `survdiff`) on ten
//! invented cases: events and censored times tied, groups of five, a zero weight and a weight of
//! two (cases repeated as often as their weight), and the times with 1000000000 added (the
//! means and medians move by 1000000000, nothing else changes). The mean of a group is the area
//! under its curve up to its own largest time, its standard error that of `survfit`; the median
//! the first time the curve is at or under one half; the log-rank test from `survdiff`. Scripts
//! in lab/oracles/edge-km.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /t e g w (4F14.2).
BEGIN DATA.
5 1 1 1
8 1 1 2
8 0 1 1
12 1 1 1
15 0 1 1
3 1 2 1
7 1 2 0
7 1 2 1
9 0 2 1
20 1 2 1
END DATA.
";

const SHIFTED: &str = "DATA LIST FREE /t e g w (4F14.2).
BEGIN DATA.
1000000005.00 1 1 1
1000000008.00 1 1 2
1000000008.00 0 1 1
1000000012.00 1 1 1
1000000015.00 0 1 1
1000000003.00 1 2 1
1000000007.00 1 2 0
1000000007.00 1 2 1
1000000009.00 0 2 1
1000000020.00 1 2 1
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

/// The first cell of each row (the group or the test).
fn row_keys(table: &Table) -> Vec<String> {
    table
        .rows
        .iter()
        .map(|row| match &row[0] {
            Cell::Text(t) => t.clone(),
            Cell::Number(n) => format!("{n}"),
            Cell::Empty => String::new(),
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
fn two_groups_with_ties() {
    let t = run("KM t BY g /STATUS=e(1) /PRINT=MEAN /TEST=LOGRANK.");
    check(
        &t,
        &[
            ("Case Processing Summary", "1", "Total N", 5.0),
            ("Case Processing Summary", "1", "N of Events", 3.0),
            ("Case Processing Summary", "1", "Censored / N", 2.0),
            (
                "Means and Medians for Survival Time",
                "1",
                "Mean / Estimate",
                10.7,
            ),
            (
                "Means and Medians for Survival Time",
                "1",
                "Mean / Std. Error",
                1.7137677789,
            ),
            (
                "Means and Medians for Survival Time",
                "1",
                "Median / Estimate",
                12.0,
            ),
            ("Case Processing Summary", "2", "Total N", 5.0),
            ("Case Processing Summary", "2", "N of Events", 4.0),
            ("Case Processing Summary", "2", "Censored / N", 1.0),
            (
                "Means and Medians for Survival Time",
                "2",
                "Mean / Estimate",
                11.4,
            ),
            (
                "Means and Medians for Survival Time",
                "2",
                "Mean / Std. Error",
                3.20749123148,
            ),
            (
                "Means and Medians for Survival Time",
                "2",
                "Median / Estimate",
                7.0,
            ),
            (
                "Overall Comparisons",
                "Log Rank (Mantel-Cox)",
                "Chi-Square",
                0.11039588027,
            ),
            ("Overall Comparisons", "Log Rank (Mantel-Cox)", "df", 1.0),
            (
                "Overall Comparisons",
                "Log Rank (Mantel-Cox)",
                "Sig.",
                0.739693881166,
            ),
        ],
    );
}

#[test]
fn zero_weight_and_weight_of_two() {
    let t = run("WEIGHT BY w.
KM t BY g /STATUS=e(1) /PRINT=MEAN /TEST=LOGRANK.");
    check(
        &t,
        &[
            ("Case Processing Summary", "1", "Total N", 6.0),
            ("Case Processing Summary", "1", "N of Events", 4.0),
            ("Case Processing Summary", "1", "Censored / N", 2.0),
            (
                "Means and Medians for Survival Time",
                "1",
                "Mean / Estimate",
                10.25,
            ),
            (
                "Means and Medians for Survival Time",
                "1",
                "Mean / Std. Error",
                1.48604620834,
            ),
            (
                "Means and Medians for Survival Time",
                "1",
                "Median / Estimate",
                8.0,
            ),
            ("Case Processing Summary", "2", "Total N", 4.0),
            ("Case Processing Summary", "2", "N of Events", 3.0),
            ("Case Processing Summary", "2", "Censored / N", 1.0),
            (
                "Means and Medians for Survival Time",
                "2",
                "Mean / Estimate",
                12.5,
            ),
            (
                "Means and Medians for Survival Time",
                "2",
                "Mean / Std. Error",
                3.81608438062,
            ),
            (
                "Means and Medians for Survival Time",
                "2",
                "Median / Estimate",
                7.0,
            ),
            (
                "Overall Comparisons",
                "Log Rank (Mantel-Cox)",
                "Chi-Square",
                0.00013621232913,
            ),
            ("Overall Comparisons", "Log Rank (Mantel-Cox)", "df", 1.0),
            (
                "Overall Comparisons",
                "Log Rank (Mantel-Cox)",
                "Sig.",
                0.990688097678,
            ),
        ],
    );
}

#[test]
fn times_near_one_billion_keep_their_digits() {
    let t = run_on(SHIFTED, "KM t BY g /STATUS=e(1) /PRINT=MEAN /TEST=LOGRANK.");
    check(
        &t,
        &[
            ("Case Processing Summary", "1", "Total N", 5.0),
            ("Case Processing Summary", "1", "N of Events", 3.0),
            ("Case Processing Summary", "1", "Censored / N", 2.0),
            (
                "Means and Medians for Survival Time",
                "1",
                "Mean / Estimate",
                1000000010.7,
            ),
            (
                "Means and Medians for Survival Time",
                "1",
                "Mean / Std. Error",
                1.7137677789,
            ),
            (
                "Means and Medians for Survival Time",
                "1",
                "Median / Estimate",
                1000000012.0,
            ),
            ("Case Processing Summary", "2", "Total N", 5.0),
            ("Case Processing Summary", "2", "N of Events", 4.0),
            ("Case Processing Summary", "2", "Censored / N", 1.0),
            (
                "Means and Medians for Survival Time",
                "2",
                "Mean / Estimate",
                1000000011.4,
            ),
            (
                "Means and Medians for Survival Time",
                "2",
                "Mean / Std. Error",
                3.20749123148,
            ),
            (
                "Means and Medians for Survival Time",
                "2",
                "Median / Estimate",
                1000000007.0,
            ),
            (
                "Overall Comparisons",
                "Log Rank (Mantel-Cox)",
                "Chi-Square",
                0.11039588027,
            ),
            ("Overall Comparisons", "Log Rank (Mantel-Cox)", "df", 1.0),
            (
                "Overall Comparisons",
                "Log Rank (Mantel-Cox)",
                "Sig.",
                0.739693881166,
            ),
        ],
    );
}
