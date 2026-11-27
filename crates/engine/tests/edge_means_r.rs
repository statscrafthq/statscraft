//! Edge cases of T-TEST and ONEWAY against R on ten invented cases: groups of one case, a group
//! with no valid value and a group value that does not occur, a constant variable, a zero weight,
//! one case alone and a missing member of a pair. R's t.test and aov on the cases written out as
//! often as their weight; where R has no result (the standard deviation of one case, t over a
//! zero spread, a group without cases) the cell is empty. An independent test with a group of one
//! case beside a larger one still gets the pooled row (its variance comes from the other group,
//! the separate-variances row stays empty) and, beside another single case, keeps its degrees of
//! freedom N1 + N2 - 2 and a mean difference; with a group of no cases nothing but the counts is
//! printed. Scripts in lab/oracles/edge-r2.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /y g z p q w h (7F8.2).
BEGIN DATA.
3 1 5 4 1 1 1
7 1 5 8 1 2 1
4 1 5 5 2 1 2
9 1 5 11 2 0 2
2 2 5 3 3 1 3
8 2 5 9 3 1 3
. 2 5 . 3 1 1
6 3 5 7 3 1 2
. 4 5 2 3 1 4
5 5 5 5 3 1 4
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
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

/// The row headers joined with " / ", an empty first header carried from the row above.
fn row_keys(table: &Table) -> Vec<String> {
    let one = (table.title.starts_with("One-Sample") && !table.title.contains("Effect"))
        || table.title.starts_with("Report");
    let mut carried = String::new();
    table
        .rows
        .iter()
        .map(|row| {
            let text = |c: &Cell| match c {
                Cell::Text(t) => t.clone(),
                Cell::Number(n) => format!("{n}"),
                Cell::Empty => String::new(),
            };
            let first = text(&row[0]);
            if !first.is_empty() {
                carried = first;
            }
            let second = if one { String::new() } else { text(&row[1]) };
            if second.is_empty() {
                carried.clone()
            } else {
                format!("{carried} / {second}")
            }
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
fn independent_groups_of_four_and_two() {
    let t = run("T-TEST GROUPS=g(1,2) /VARIABLES=y /ES DISPLAY(TRUE).");
    check(
        &t,
        &[
            ("Group Statistics", "y / 1", "N", 4.0),
            ("Group Statistics", "y / 1", "Mean", 5.75),
            ("Group Statistics", "y / 1", "Std. Deviation", 2.75378527364),
            (
                "Group Statistics",
                "y / 1",
                "Std. Error Mean",
                1.37689263682,
            ),
            ("Group Statistics", "y / 2", "N", 2.0),
            ("Group Statistics", "y / 2", "Mean", 5.0),
            ("Group Statistics", "y / 2", "Std. Deviation", 4.24264068712),
            ("Group Statistics", "y / 2", "Std. Error Mean", 3.0),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Mean Difference",
                0.75,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Mean Difference",
                0.75,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / df",
                4.0,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "Levene's Test for Equality of Variances / F",
                1.2,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "Levene's Test for Equality of Variances / Sig.",
                0.334852284194,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / t",
                0.271329378988,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                0.799564936236,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Std. Error Difference",
                2.76416804844,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Lower",
                -6.92456084803,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Upper",
                8.42456084803,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / t",
                0.227211881304,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / df",
                1.44430637522,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                0.848204845467,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Std. Error Difference",
                3.30088372006,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Lower",
                -20.2191337384,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Upper",
                21.7191337384,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Cohen's d",
                "Standardizer",
                3.1917863337,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Cohen's d",
                "Point Estimate",
                0.234978134996,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Hedges' correction",
                "Standardizer",
                4.00031093532,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Hedges' correction",
                "Point Estimate",
                0.18748542604,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Glass's delta",
                "Standardizer",
                4.24264068712,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Glass's delta",
                "Point Estimate",
                0.176776695297,
            ),
        ],
    );
}

#[test]
fn independent_groups_with_a_zero_weight() {
    let t = run("WEIGHT BY w.\nT-TEST GROUPS=g(1,2) /VARIABLES=y /ES DISPLAY(TRUE).");
    check(
        &t,
        &[
            ("Group Statistics", "y / 1", "N", 4.0),
            ("Group Statistics", "y / 1", "Mean", 5.25),
            ("Group Statistics", "y / 1", "Std. Deviation", 2.06155281281),
            ("Group Statistics", "y / 1", "Std. Error Mean", 1.0307764064),
            ("Group Statistics", "y / 2", "N", 2.0),
            ("Group Statistics", "y / 2", "Mean", 5.0),
            ("Group Statistics", "y / 2", "Std. Deviation", 4.24264068712),
            ("Group Statistics", "y / 2", "Std. Error Mean", 3.0),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Mean Difference",
                0.25,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Mean Difference",
                0.25,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / df",
                4.0,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "Levene's Test for Equality of Variances / F",
                16.6666666667,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "Levene's Test for Equality of Variances / Sig.",
                0.0150676985628,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / t",
                0.104115841259,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                0.922088966675,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Std. Error Difference",
                2.40117158904,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Lower",
                -6.41672110512,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Upper",
                6.91672110512,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / t",
                0.0788110406239,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / df",
                1.24426772485,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                0.947919656959,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Std. Error Difference",
                3.17214438511,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Lower",
                -25.4253084783,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Upper",
                25.9253084783,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Cohen's d",
                "Standardizer",
                2.7726341266,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Cohen's d",
                "Point Estimate",
                0.0901669634667,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Hedges' correction",
                "Standardizer",
                3.47498154847,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Hedges' correction",
                "Point Estimate",
                0.0719428280446,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Glass's delta",
                "Standardizer",
                4.24264068712,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Glass's delta",
                "Point Estimate",
                0.0589255650989,
            ),
        ],
    );
}

#[test]
fn independent_groups_of_one_case_each() {
    let t = run("T-TEST GROUPS=g(3,5) /VARIABLES=y.");
    check(
        &t,
        &[
            ("Group Statistics", "y / 3", "N", 1.0),
            ("Group Statistics", "y / 3", "Mean", 6.0),
            ("Group Statistics", "y / 3", "Std. Deviation", f64::NAN),
            ("Group Statistics", "y / 3", "Std. Error Mean", f64::NAN),
            ("Group Statistics", "y / 5", "N", 1.0),
            ("Group Statistics", "y / 5", "Mean", 5.0),
            ("Group Statistics", "y / 5", "Std. Deviation", f64::NAN),
            ("Group Statistics", "y / 5", "Std. Error Mean", f64::NAN),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Mean Difference",
                1.0,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Mean Difference",
                1.0,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / df",
                0.0,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / t",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Std. Error Difference",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / df",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Lower",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Upper",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / t",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Std. Error Difference",
                f64::NAN,
            ),
        ],
    );
}

#[test]
fn independent_one_case_against_four() {
    let t = run("T-TEST GROUPS=g(3,1) /VARIABLES=y /ES DISPLAY(TRUE).");
    check(
        &t,
        &[
            ("Group Statistics", "y / 3", "N", 1.0),
            ("Group Statistics", "y / 3", "Mean", 6.0),
            ("Group Statistics", "y / 3", "Std. Deviation", f64::NAN),
            ("Group Statistics", "y / 3", "Std. Error Mean", f64::NAN),
            ("Group Statistics", "y / 1", "N", 4.0),
            ("Group Statistics", "y / 1", "Mean", 5.75),
            ("Group Statistics", "y / 1", "Std. Deviation", 2.75378527364),
            (
                "Group Statistics",
                "y / 1",
                "Std. Error Mean",
                1.37689263682,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Mean Difference",
                0.25,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Mean Difference",
                0.25,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / df",
                3.0,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / t",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Std. Error Difference",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / df",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Lower",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Upper",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / t",
                0.0811997942941,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                0.940396895285,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Std. Error Difference",
                3.07882553365,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Lower",
                -9.54819694418,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Upper",
                10.0481969442,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "Levene's Test for Equality of Variances / F",
                4.86,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "Levene's Test for Equality of Variances / Sig.",
                0.11468444242,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Cohen's d",
                "Standardizer",
                2.75378527364,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Cohen's d",
                "Point Estimate",
                0.0907841299003,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Hedges' correction",
                "Standardizer",
                3.80566680378,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Hedges' correction",
                "Point Estimate",
                0.0656915102899,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Glass's delta",
                "Standardizer",
                2.75378527364,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Glass's delta",
                "Point Estimate",
                0.0907841299003,
            ),
        ],
    );
}

#[test]
fn independent_two_cases_against_one() {
    let t = run("T-TEST GROUPS=g(2,5) /VARIABLES=y /ES DISPLAY(TRUE).");
    check(
        &t,
        &[
            ("Group Statistics", "y / 2", "N", 2.0),
            ("Group Statistics", "y / 2", "Mean", 5.0),
            ("Group Statistics", "y / 2", "Std. Deviation", 4.24264068712),
            ("Group Statistics", "y / 2", "Std. Error Mean", 3.0),
            ("Group Statistics", "y / 5", "N", 1.0),
            ("Group Statistics", "y / 5", "Mean", 5.0),
            ("Group Statistics", "y / 5", "Std. Deviation", f64::NAN),
            ("Group Statistics", "y / 5", "Std. Error Mean", f64::NAN),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Mean Difference",
                0.0,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Mean Difference",
                0.0,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / df",
                1.0,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / t",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Std. Error Difference",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / df",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Lower",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Upper",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / t",
                0.0,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                1.0,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Std. Error Difference",
                5.19615242271,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Lower",
                -66.0233765233,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / 95% Confidence Interval / Upper",
                66.0233765233,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "Levene's Test for Equality of Variances / F",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "Levene's Test for Equality of Variances / Sig.",
                0.0,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Cohen's d",
                "Standardizer",
                4.24264068712,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Cohen's d",
                "Point Estimate",
                0.0,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Hedges' correction",
                "Standardizer",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Hedges' correction",
                "Point Estimate",
                0.0,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Glass's delta",
                "Standardizer",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Glass's delta",
                "Point Estimate",
                f64::NAN,
            ),
        ],
    );
}

#[test]
fn independent_group_without_valid_cases() {
    let t = run("T-TEST GROUPS=g(3,4) /VARIABLES=y /ES DISPLAY(TRUE).");
    check(
        &t,
        &[
            ("Group Statistics", "y / 3", "N", 1.0),
            ("Group Statistics", "y / 3", "Mean", 6.0),
            ("Group Statistics", "y / 3", "Std. Deviation", f64::NAN),
            ("Group Statistics", "y / 3", "Std. Error Mean", f64::NAN),
            ("Group Statistics", "y / 4", "N", 0.0),
            ("Group Statistics", "y / 4", "Mean", f64::NAN),
            ("Group Statistics", "y / 4", "Std. Deviation", f64::NAN),
            ("Group Statistics", "y / 4", "Std. Error Mean", f64::NAN),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / t",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Std. Error Difference",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Mean Difference",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / t",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Std. Error Difference",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Mean Difference",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / df",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Cohen's d",
                "Standardizer",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Cohen's d",
                "Point Estimate",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Hedges' correction",
                "Standardizer",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Hedges' correction",
                "Point Estimate",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Glass's delta",
                "Standardizer",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Glass's delta",
                "Point Estimate",
                f64::NAN,
            ),
        ],
    );
}

#[test]
fn independent_group_value_that_does_not_occur() {
    let t = run("T-TEST GROUPS=g(1,9) /VARIABLES=y /ES DISPLAY(TRUE).");
    check(
        &t,
        &[
            ("Group Statistics", "y / 1", "N", 4.0),
            ("Group Statistics", "y / 1", "Mean", 5.75),
            ("Group Statistics", "y / 1", "Std. Deviation", 2.75378527364),
            (
                "Group Statistics",
                "y / 1",
                "Std. Error Mean",
                1.37689263682,
            ),
            ("Group Statistics", "y / 9", "N", 0.0),
            ("Group Statistics", "y / 9", "Mean", f64::NAN),
            ("Group Statistics", "y / 9", "Std. Deviation", f64::NAN),
            ("Group Statistics", "y / 9", "Std. Error Mean", f64::NAN),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / t",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Std. Error Difference",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / Mean Difference",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / t",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Std. Error Difference",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances not assumed",
                "t-test for Equality of Means / Mean Difference",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "y / Equal variances assumed",
                "t-test for Equality of Means / df",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Cohen's d",
                "Standardizer",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Cohen's d",
                "Point Estimate",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Hedges' correction",
                "Standardizer",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Hedges' correction",
                "Point Estimate",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Glass's delta",
                "Standardizer",
                f64::NAN,
            ),
            (
                "Independent Samples Effect Sizes",
                "y / Glass's delta",
                "Point Estimate",
                f64::NAN,
            ),
        ],
    );
}

#[test]
fn independent_constant_variable() {
    let t = run("T-TEST GROUPS=g(1,2) /VARIABLES=z.");
    check(
        &t,
        &[
            ("Group Statistics", "z / 1", "N", 4.0),
            ("Group Statistics", "z / 1", "Mean", 5.0),
            ("Group Statistics", "z / 1", "Std. Deviation", 0.0),
            ("Group Statistics", "z / 1", "Std. Error Mean", 0.0),
            ("Group Statistics", "z / 2", "N", 3.0),
            ("Group Statistics", "z / 2", "Mean", 5.0),
            ("Group Statistics", "z / 2", "Std. Deviation", 0.0),
            ("Group Statistics", "z / 2", "Std. Error Mean", 0.0),
            (
                "Independent Samples Test",
                "z / Equal variances assumed",
                "t-test for Equality of Means / Mean Difference",
                0.0,
            ),
            (
                "Independent Samples Test",
                "z / Equal variances not assumed",
                "t-test for Equality of Means / Mean Difference",
                0.0,
            ),
            (
                "Independent Samples Test",
                "z / Equal variances assumed",
                "t-test for Equality of Means / df",
                5.0,
            ),
            (
                "Independent Samples Test",
                "z / Equal variances assumed",
                "t-test for Equality of Means / t",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "z / Equal variances assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "z / Equal variances not assumed",
                "t-test for Equality of Means / t",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "z / Equal variances not assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                f64::NAN,
            ),
            (
                "Independent Samples Test",
                "z / Equal variances assumed",
                "Levene's Test for Equality of Variances / F",
                f64::NAN,
            ),
        ],
    );
}

#[test]
fn one_sample_of_eight_cases() {
    let t = run("T-TEST /VARIABLES=y /TESTVAL=5 /ES DISPLAY(TRUE).");
    check(
        &t,
        &[
            ("One-Sample Statistics", "y", "N", 8.0),
            ("One-Sample Statistics", "y", "Mean", 5.5),
            (
                "One-Sample Statistics",
                "y",
                "Std. Deviation",
                2.44948974278,
            ),
            (
                "One-Sample Statistics",
                "y",
                "Std. Error Mean",
                0.866025403784,
            ),
            ("One-Sample Test (Test Value = 5)", "y", "df", 7.0),
            (
                "One-Sample Test (Test Value = 5)",
                "y",
                "Mean Difference",
                0.5,
            ),
            ("One-Sample Test (Test Value = 5)", "y", "t", 0.57735026919),
            (
                "One-Sample Test (Test Value = 5)",
                "y",
                "Sig. (2-tailed)",
                0.581788234592,
            ),
            (
                "One-Sample Test (Test Value = 5)",
                "y",
                "95% Confidence Interval / Lower",
                -1.54782467228,
            ),
            (
                "One-Sample Test (Test Value = 5)",
                "y",
                "95% Confidence Interval / Upper",
                2.54782467228,
            ),
            (
                "One-Sample Effect Sizes",
                "y / Cohen's d",
                "Standardizer",
                2.44948974278,
            ),
            (
                "One-Sample Effect Sizes",
                "y / Cohen's d",
                "Point Estimate",
                0.204124145232,
            ),
            (
                "One-Sample Effect Sizes",
                "y / Hedges' correction",
                "Standardizer",
                2.75780423777,
            ),
            (
                "One-Sample Effect Sizes",
                "y / Hedges' correction",
                "Point Estimate",
                0.181303659321,
            ),
        ],
    );
}

#[test]
fn one_sample_constant_variable() {
    let t = run("T-TEST /VARIABLES=z /TESTVAL=5.");
    check(
        &t,
        &[
            ("One-Sample Statistics", "z", "N", 10.0),
            ("One-Sample Statistics", "z", "Mean", 5.0),
            ("One-Sample Statistics", "z", "Std. Deviation", 0.0),
            ("One-Sample Statistics", "z", "Std. Error Mean", 0.0),
            ("One-Sample Test (Test Value = 5)", "z", "df", 9.0),
            (
                "One-Sample Test (Test Value = 5)",
                "z",
                "Mean Difference",
                0.0,
            ),
            ("One-Sample Test (Test Value = 5)", "z", "t", f64::NAN),
            (
                "One-Sample Test (Test Value = 5)",
                "z",
                "Sig. (2-tailed)",
                f64::NAN,
            ),
        ],
    );
}

#[test]
fn one_sample_of_one_case() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 1).\nT-TEST /VARIABLES=y /TESTVAL=5.",
    );
    check(
        &t,
        &[
            ("One-Sample Statistics", "y", "N", 1.0),
            ("One-Sample Statistics", "y", "Mean", 3.0),
            ("One-Sample Statistics", "y", "Std. Deviation", f64::NAN),
            ("One-Sample Statistics", "y", "Std. Error Mean", f64::NAN),
            ("One-Sample Test (Test Value = 5)", "y", "df", 0.0),
            (
                "One-Sample Test (Test Value = 5)",
                "y",
                "Mean Difference",
                -2.0,
            ),
            ("One-Sample Test (Test Value = 5)", "y", "t", f64::NAN),
            (
                "One-Sample Test (Test Value = 5)",
                "y",
                "Sig. (2-tailed)",
                f64::NAN,
            ),
        ],
    );
}

#[test]
fn paired_with_a_missing_member() {
    let t = run("T-TEST PAIRS=y WITH p (PAIRED).");
    check(
        &t,
        &[
            ("Paired Samples Statistics", "Pair 1 / y", "N", 8.0),
            ("Paired Samples Statistics", "Pair 1 / y", "Mean", 5.5),
            (
                "Paired Samples Statistics",
                "Pair 1 / y",
                "Std. Deviation",
                2.44948974278,
            ),
            ("Paired Samples Correlations", "Pair 1 / y & p", "N", 8.0),
            (
                "Paired Samples Correlations",
                "Pair 1 / y & p",
                "Correlation",
                0.984309132775,
            ),
            (
                "Paired Samples Correlations",
                "Pair 1 / y & p",
                "Sig.",
                9.54456032094e-06,
            ),
            (
                "Paired Samples Test",
                "Pair 1 / y - p",
                "Paired Differences / Mean",
                -1.0,
            ),
            (
                "Paired Samples Test",
                "Pair 1 / y - p",
                "Paired Differences / Std. Deviation",
                0.534522483825,
            ),
            ("Paired Samples Test", "Pair 1 / y - p", "df", 7.0),
            ("Paired Samples Test", "Pair 1 / y - p", "t", -5.29150262213),
            (
                "Paired Samples Test",
                "Pair 1 / y - p",
                "Sig. (2-tailed)",
                0.00113378310336,
            ),
            (
                "Paired Samples Test",
                "Pair 1 / y - p",
                "Paired Differences / 95% Confidence Interval / Lower",
                -1.44687197956,
            ),
            (
                "Paired Samples Test",
                "Pair 1 / y - p",
                "Paired Differences / 95% Confidence Interval / Upper",
                -0.553128020441,
            ),
        ],
    );
}

#[test]
fn paired_constant_variable_with_itself() {
    let t = run("T-TEST PAIRS=z WITH z (PAIRED).");
    check(
        &t,
        &[
            ("Paired Samples Statistics", "Pair 1 / z", "N", 10.0),
            ("Paired Samples Statistics", "Pair 1 / z", "Mean", 5.0),
            (
                "Paired Samples Statistics",
                "Pair 1 / z",
                "Std. Deviation",
                0.0,
            ),
            ("Paired Samples Correlations", "Pair 1 / z & z", "N", 10.0),
            (
                "Paired Samples Correlations",
                "Pair 1 / z & z",
                "Correlation",
                f64::NAN,
            ),
            (
                "Paired Samples Correlations",
                "Pair 1 / z & z",
                "Sig.",
                f64::NAN,
            ),
            (
                "Paired Samples Test",
                "Pair 1 / z - z",
                "Paired Differences / Mean",
                0.0,
            ),
            (
                "Paired Samples Test",
                "Pair 1 / z - z",
                "Paired Differences / Std. Deviation",
                0.0,
            ),
            ("Paired Samples Test", "Pair 1 / z - z", "df", 9.0),
            ("Paired Samples Test", "Pair 1 / z - z", "t", f64::NAN),
            (
                "Paired Samples Test",
                "Pair 1 / z - z",
                "Sig. (2-tailed)",
                f64::NAN,
            ),
        ],
    );
}

#[test]
fn paired_one_case() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 1).\nT-TEST PAIRS=y WITH p (PAIRED).",
    );
    check(
        &t,
        &[
            ("Paired Samples Statistics", "Pair 1 / y", "N", 1.0),
            ("Paired Samples Statistics", "Pair 1 / y", "Mean", 3.0),
            (
                "Paired Samples Statistics",
                "Pair 1 / y",
                "Std. Deviation",
                f64::NAN,
            ),
            ("Paired Samples Correlations", "Pair 1 / y & p", "N", 1.0),
            (
                "Paired Samples Correlations",
                "Pair 1 / y & p",
                "Correlation",
                f64::NAN,
            ),
            (
                "Paired Samples Correlations",
                "Pair 1 / y & p",
                "Sig.",
                f64::NAN,
            ),
            (
                "Paired Samples Test",
                "Pair 1 / y - p",
                "Paired Differences / Mean",
                -1.0,
            ),
            (
                "Paired Samples Test",
                "Pair 1 / y - p",
                "Paired Differences / Std. Deviation",
                f64::NAN,
            ),
            ("Paired Samples Test", "Pair 1 / y - p", "df", 0.0),
            ("Paired Samples Test", "Pair 1 / y - p", "t", f64::NAN),
            (
                "Paired Samples Test",
                "Pair 1 / y - p",
                "Sig. (2-tailed)",
                f64::NAN,
            ),
        ],
    );
}

#[test]
fn oneway_groups_of_one_case_and_an_empty_group() {
    let t = run("ONEWAY y BY g /STATISTICS=DESCRIPTIVES HOMOGENEITY.");
    check(
        &t,
        &[
            ("Descriptives", "y / 1", "N", 4.0),
            ("Descriptives", "y / 1", "Mean", 5.75),
            ("Descriptives", "y / 1", "Std. Deviation", 2.75378527364),
            ("Descriptives", "y / 1", "Std. Error", 1.37689263682),
            ("Descriptives", "y / 1", "Minimum", 3.0),
            ("Descriptives", "y / 1", "Maximum", 9.0),
            (
                "Descriptives",
                "y / 1",
                "95% Confidence Interval for Mean / Lower Bound",
                1.36811311517,
            ),
            (
                "Descriptives",
                "y / 1",
                "95% Confidence Interval for Mean / Upper Bound",
                10.1318868848,
            ),
            ("Descriptives", "y / 2", "N", 2.0),
            ("Descriptives", "y / 2", "Mean", 5.0),
            ("Descriptives", "y / 2", "Std. Deviation", 4.24264068712),
            ("Descriptives", "y / 2", "Std. Error", 3.0),
            ("Descriptives", "y / 2", "Minimum", 2.0),
            ("Descriptives", "y / 2", "Maximum", 8.0),
            (
                "Descriptives",
                "y / 2",
                "95% Confidence Interval for Mean / Lower Bound",
                -33.1186142085,
            ),
            (
                "Descriptives",
                "y / 2",
                "95% Confidence Interval for Mean / Upper Bound",
                43.1186142085,
            ),
            ("Descriptives", "y / 3", "N", 1.0),
            ("Descriptives", "y / 3", "Mean", 6.0),
            ("Descriptives", "y / 3", "Std. Deviation", f64::NAN),
            ("Descriptives", "y / 3", "Std. Error", f64::NAN),
            ("Descriptives", "y / 3", "Minimum", 6.0),
            ("Descriptives", "y / 3", "Maximum", 6.0),
            (
                "Descriptives",
                "y / 3",
                "95% Confidence Interval for Mean / Lower Bound",
                f64::NAN,
            ),
            (
                "Descriptives",
                "y / 3",
                "95% Confidence Interval for Mean / Upper Bound",
                f64::NAN,
            ),
            ("Descriptives", "y / 5", "N", 1.0),
            ("Descriptives", "y / 5", "Mean", 5.0),
            ("Descriptives", "y / 5", "Std. Deviation", f64::NAN),
            ("Descriptives", "y / 5", "Std. Error", f64::NAN),
            ("Descriptives", "y / 5", "Minimum", 5.0),
            ("Descriptives", "y / 5", "Maximum", 5.0),
            (
                "Descriptives",
                "y / 5",
                "95% Confidence Interval for Mean / Lower Bound",
                f64::NAN,
            ),
            (
                "Descriptives",
                "y / 5",
                "95% Confidence Interval for Mean / Upper Bound",
                f64::NAN,
            ),
            ("Descriptives", "y / Total", "N", 8.0),
            ("Descriptives", "y / Total", "Mean", 5.5),
            ("Descriptives", "y / Total", "Std. Deviation", 2.44948974278),
            ("Descriptives", "y / Total", "Std. Error", 0.866025403784),
            ("Descriptives", "y / Total", "Minimum", 2.0),
            ("Descriptives", "y / Total", "Maximum", 9.0),
            (
                "Descriptives",
                "y / Total",
                "95% Confidence Interval for Mean / Lower Bound",
                3.45217532772,
            ),
            (
                "Descriptives",
                "y / Total",
                "95% Confidence Interval for Mean / Upper Bound",
                7.54782467228,
            ),
            ("ANOVA", "y / Between Groups", "Sum of Squares", 1.25),
            ("ANOVA", "y / Between Groups", "df", 3.0),
            ("ANOVA", "y / Between Groups", "Mean Square", 0.416666666667),
            ("ANOVA", "y / Within Groups", "Sum of Squares", 40.75),
            ("ANOVA", "y / Within Groups", "df", 4.0),
            ("ANOVA", "y / Within Groups", "Mean Square", 10.1875),
            ("ANOVA", "y / Total", "Sum of Squares", 42.0),
            ("ANOVA", "y / Total", "df", 7.0),
            ("ANOVA", "y / Between Groups", "F", 0.040899795501),
            ("ANOVA", "y / Between Groups", "Sig.", 0.987393174106),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Mean",
                "Levene Statistic",
                5.4,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Mean",
                "df1",
                3.0,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Mean",
                "df2",
                4.0,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Mean",
                "Sig.",
                0.0684731239304,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Median",
                "Levene Statistic",
                4.90909090909,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Median",
                "df1",
                3.0,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Median",
                "df2",
                4.0,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Median",
                "Sig.",
                0.0791829017408,
            ),
        ],
    );
}

#[test]
fn oneway_constant_variable() {
    let t = run("ONEWAY z BY g /STATISTICS=DESCRIPTIVES HOMOGENEITY.");
    check(
        &t,
        &[
            ("Descriptives", "z / 1", "N", 4.0),
            ("Descriptives", "z / 1", "Mean", 5.0),
            ("Descriptives", "z / 1", "Std. Deviation", 0.0),
            ("Descriptives", "z / 1", "Std. Error", 0.0),
            ("Descriptives", "z / 1", "Minimum", 5.0),
            ("Descriptives", "z / 1", "Maximum", 5.0),
            (
                "Descriptives",
                "z / 1",
                "95% Confidence Interval for Mean / Lower Bound",
                5.0,
            ),
            (
                "Descriptives",
                "z / 1",
                "95% Confidence Interval for Mean / Upper Bound",
                5.0,
            ),
            ("Descriptives", "z / 2", "N", 3.0),
            ("Descriptives", "z / 2", "Mean", 5.0),
            ("Descriptives", "z / 2", "Std. Deviation", 0.0),
            ("Descriptives", "z / 2", "Std. Error", 0.0),
            ("Descriptives", "z / 2", "Minimum", 5.0),
            ("Descriptives", "z / 2", "Maximum", 5.0),
            (
                "Descriptives",
                "z / 2",
                "95% Confidence Interval for Mean / Lower Bound",
                5.0,
            ),
            (
                "Descriptives",
                "z / 2",
                "95% Confidence Interval for Mean / Upper Bound",
                5.0,
            ),
            ("Descriptives", "z / 3", "N", 1.0),
            ("Descriptives", "z / 3", "Mean", 5.0),
            ("Descriptives", "z / 3", "Std. Deviation", f64::NAN),
            ("Descriptives", "z / 3", "Std. Error", f64::NAN),
            ("Descriptives", "z / 3", "Minimum", 5.0),
            ("Descriptives", "z / 3", "Maximum", 5.0),
            (
                "Descriptives",
                "z / 3",
                "95% Confidence Interval for Mean / Lower Bound",
                f64::NAN,
            ),
            (
                "Descriptives",
                "z / 3",
                "95% Confidence Interval for Mean / Upper Bound",
                f64::NAN,
            ),
            ("Descriptives", "z / 4", "N", 1.0),
            ("Descriptives", "z / 4", "Mean", 5.0),
            ("Descriptives", "z / 4", "Std. Deviation", f64::NAN),
            ("Descriptives", "z / 4", "Std. Error", f64::NAN),
            ("Descriptives", "z / 4", "Minimum", 5.0),
            ("Descriptives", "z / 4", "Maximum", 5.0),
            (
                "Descriptives",
                "z / 4",
                "95% Confidence Interval for Mean / Lower Bound",
                f64::NAN,
            ),
            (
                "Descriptives",
                "z / 4",
                "95% Confidence Interval for Mean / Upper Bound",
                f64::NAN,
            ),
            ("Descriptives", "z / 5", "N", 1.0),
            ("Descriptives", "z / 5", "Mean", 5.0),
            ("Descriptives", "z / 5", "Std. Deviation", f64::NAN),
            ("Descriptives", "z / 5", "Std. Error", f64::NAN),
            ("Descriptives", "z / 5", "Minimum", 5.0),
            ("Descriptives", "z / 5", "Maximum", 5.0),
            (
                "Descriptives",
                "z / 5",
                "95% Confidence Interval for Mean / Lower Bound",
                f64::NAN,
            ),
            (
                "Descriptives",
                "z / 5",
                "95% Confidence Interval for Mean / Upper Bound",
                f64::NAN,
            ),
            ("Descriptives", "z / Total", "N", 10.0),
            ("Descriptives", "z / Total", "Mean", 5.0),
            ("Descriptives", "z / Total", "Std. Deviation", 0.0),
            ("Descriptives", "z / Total", "Std. Error", 0.0),
            ("Descriptives", "z / Total", "Minimum", 5.0),
            ("Descriptives", "z / Total", "Maximum", 5.0),
            (
                "Descriptives",
                "z / Total",
                "95% Confidence Interval for Mean / Lower Bound",
                5.0,
            ),
            (
                "Descriptives",
                "z / Total",
                "95% Confidence Interval for Mean / Upper Bound",
                5.0,
            ),
            (
                "ANOVA",
                "z / Between Groups",
                "Sum of Squares",
                1.18329135783e-30,
            ),
            ("ANOVA", "z / Between Groups", "df", 4.0),
            (
                "ANOVA",
                "z / Between Groups",
                "Mean Square",
                2.95822839458e-31,
            ),
            (
                "ANOVA",
                "z / Within Groups",
                "Sum of Squares",
                5.91645678916e-30,
            ),
            ("ANOVA", "z / Within Groups", "df", 5.0),
            (
                "ANOVA",
                "z / Within Groups",
                "Mean Square",
                1.18329135783e-30,
            ),
            ("ANOVA", "z / Total", "Sum of Squares", 7.09974814699e-30),
            ("ANOVA", "z / Total", "df", 9.0),
            ("ANOVA", "z / Between Groups", "F", f64::NAN),
            ("ANOVA", "z / Between Groups", "Sig.", f64::NAN),
            (
                "Test of Homogeneity of Variances",
                "z / Based on Mean",
                "Levene Statistic",
                f64::NAN,
            ),
            (
                "Test of Homogeneity of Variances",
                "z / Based on Mean",
                "df1",
                4.0,
            ),
            (
                "Test of Homogeneity of Variances",
                "z / Based on Mean",
                "df2",
                5.0,
            ),
            (
                "Test of Homogeneity of Variances",
                "z / Based on Mean",
                "Sig.",
                f64::NAN,
            ),
            (
                "Test of Homogeneity of Variances",
                "z / Based on Median",
                "Levene Statistic",
                f64::NAN,
            ),
            (
                "Test of Homogeneity of Variances",
                "z / Based on Median",
                "df1",
                4.0,
            ),
            (
                "Test of Homogeneity of Variances",
                "z / Based on Median",
                "df2",
                5.0,
            ),
            (
                "Test of Homogeneity of Variances",
                "z / Based on Median",
                "Sig.",
                f64::NAN,
            ),
        ],
    );
}

#[test]
fn oneway_with_a_zero_weight() {
    let t = run("WEIGHT BY w.\nONEWAY y BY g /STATISTICS=DESCRIPTIVES HOMOGENEITY.");
    check(
        &t,
        &[
            ("Descriptives", "y / 1", "N", 4.0),
            ("Descriptives", "y / 1", "Mean", 5.25),
            ("Descriptives", "y / 1", "Std. Deviation", 2.06155281281),
            ("Descriptives", "y / 1", "Std. Error", 1.0307764064),
            ("Descriptives", "y / 1", "Minimum", 3.0),
            ("Descriptives", "y / 1", "Maximum", 7.0),
            (
                "Descriptives",
                "y / 1",
                "95% Confidence Interval for Mean / Lower Bound",
                1.96960943386,
            ),
            (
                "Descriptives",
                "y / 1",
                "95% Confidence Interval for Mean / Upper Bound",
                8.53039056614,
            ),
            ("Descriptives", "y / 2", "N", 2.0),
            ("Descriptives", "y / 2", "Mean", 5.0),
            ("Descriptives", "y / 2", "Std. Deviation", 4.24264068712),
            ("Descriptives", "y / 2", "Std. Error", 3.0),
            ("Descriptives", "y / 2", "Minimum", 2.0),
            ("Descriptives", "y / 2", "Maximum", 8.0),
            (
                "Descriptives",
                "y / 2",
                "95% Confidence Interval for Mean / Lower Bound",
                -33.1186142085,
            ),
            (
                "Descriptives",
                "y / 2",
                "95% Confidence Interval for Mean / Upper Bound",
                43.1186142085,
            ),
            ("Descriptives", "y / 3", "N", 1.0),
            ("Descriptives", "y / 3", "Mean", 6.0),
            ("Descriptives", "y / 3", "Std. Deviation", f64::NAN),
            ("Descriptives", "y / 3", "Std. Error", f64::NAN),
            ("Descriptives", "y / 3", "Minimum", 6.0),
            ("Descriptives", "y / 3", "Maximum", 6.0),
            (
                "Descriptives",
                "y / 3",
                "95% Confidence Interval for Mean / Lower Bound",
                f64::NAN,
            ),
            (
                "Descriptives",
                "y / 3",
                "95% Confidence Interval for Mean / Upper Bound",
                f64::NAN,
            ),
            ("Descriptives", "y / 5", "N", 1.0),
            ("Descriptives", "y / 5", "Mean", 5.0),
            ("Descriptives", "y / 5", "Std. Deviation", f64::NAN),
            ("Descriptives", "y / 5", "Std. Error", f64::NAN),
            ("Descriptives", "y / 5", "Minimum", 5.0),
            ("Descriptives", "y / 5", "Maximum", 5.0),
            (
                "Descriptives",
                "y / 5",
                "95% Confidence Interval for Mean / Lower Bound",
                f64::NAN,
            ),
            (
                "Descriptives",
                "y / 5",
                "95% Confidence Interval for Mean / Upper Bound",
                f64::NAN,
            ),
            ("Descriptives", "y / Total", "N", 8.0),
            ("Descriptives", "y / Total", "Mean", 5.25),
            ("Descriptives", "y / Total", "Std. Deviation", 2.12132034356),
            ("Descriptives", "y / Total", "Std. Error", 0.75),
            ("Descriptives", "y / Total", "Minimum", 2.0),
            ("Descriptives", "y / Total", "Maximum", 8.0),
            (
                "Descriptives",
                "y / Total",
                "95% Confidence Interval for Mean / Lower Bound",
                3.47653181131,
            ),
            (
                "Descriptives",
                "y / Total",
                "95% Confidence Interval for Mean / Upper Bound",
                7.02346818869,
            ),
            ("ANOVA", "y / Between Groups", "Sum of Squares", 0.75),
            ("ANOVA", "y / Between Groups", "df", 3.0),
            ("ANOVA", "y / Between Groups", "Mean Square", 0.25),
            ("ANOVA", "y / Within Groups", "Sum of Squares", 30.75),
            ("ANOVA", "y / Within Groups", "df", 4.0),
            ("ANOVA", "y / Within Groups", "Mean Square", 7.6875),
            ("ANOVA", "y / Total", "Sum of Squares", 31.5),
            ("ANOVA", "y / Total", "df", 7.0),
            ("ANOVA", "y / Between Groups", "F", 0.0325203252033),
            ("ANOVA", "y / Between Groups", "Sig.", 0.99094648712),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Mean",
                "Levene Statistic",
                24.3333333333,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Mean",
                "df1",
                3.0,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Mean",
                "df2",
                4.0,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Mean",
                "Sig.",
                0.00497138666361,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Median",
                "Levene Statistic",
                16.2222222222,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Median",
                "df1",
                3.0,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Median",
                "df2",
                4.0,
            ),
            (
                "Test of Homogeneity of Variances",
                "y / Based on Median",
                "Sig.",
                0.0105377398267,
            ),
        ],
    );
}
