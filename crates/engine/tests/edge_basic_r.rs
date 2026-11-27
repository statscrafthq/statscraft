//! Edge cases of FREQUENCIES, DESCRIPTIVES, MEANS and CORRELATIONS against R on nine invented
//! cases: a constant variable, a variable with no valid value, groups of one case, a group with no
//! valid value, the first one, two and three cases alone, a zero weight and weighted cases beside
//! a missing value. R follows the formulas of the algorithms reference written out; where R gives
//! NaN (the standard deviation of one case, the skewness of a constant) the cell is empty. The
//! manual's own rule applies to the skewness and kurtosis (at least three and four weighted cases
//! and a positive variance, the standard errors under the same condition), which R's formulas
//! reproduce as NaN. A variable with no valid value still prints a sum of 0 in DESCRIPTIVES and
//! nothing in FREQUENCIES: that difference is left as it is, so the sum is not asserted. Scripts
//! in lab/oracles/edge-r1.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /x c m g w y (6F8.2).
BEGIN DATA.
3 5 . 1 1 2
7 5 . 1 2 5
4 5 . 1 1 4
9 5 . 2 0 9
2 5 . 2 1 1
8 5 . 3 1 6
. 5 . 4 1 .
6 5 . 4 1 7
. 5 . 5 1 3
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
    let mut carried = String::new();
    table
        .rows
        .iter()
        .map(|row| {
            let text = |c: &Cell| match c {
                Cell::Text(t) => Some(t.clone()),
                Cell::Number(n) if row.len() > 2 => Some(format!("{n}")),
                _ => None,
            };
            if let Some(first) = text(&row[0]) {
                carried = first;
            }
            match row.get(1).and_then(|c| match c {
                Cell::Text(t) => Some(t.clone()),
                _ => None,
            }) {
                Some(second) if !second.is_empty() => format!("{carried} / {second}"),
                _ => carried.clone(),
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
fn constant_and_all_missing_frequencies() {
    let t = run("FREQUENCIES VARIABLES=c m /STATISTICS=ALL /FORMAT=NOTABLE.");
    check(
        &t,
        &[
            ("Statistics", "N / Valid", "c", 9.0),
            ("Statistics", "N / Missing", "c", 0.0),
            ("Statistics", "Mean", "c", 5.0),
            ("Statistics", "Std. Error of Mean", "c", 0.0),
            ("Statistics", "Median", "c", 5.0),
            ("Statistics", "Mode", "c", 5.0),
            ("Statistics", "Std. Deviation", "c", 0.0),
            ("Statistics", "Variance", "c", 0.0),
            ("Statistics", "Range", "c", 0.0),
            ("Statistics", "Minimum", "c", 5.0),
            ("Statistics", "Maximum", "c", 5.0),
            ("Statistics", "Sum", "c", 45.0),
            ("Statistics", "Skewness", "c", f64::NAN),
            ("Statistics", "Std. Error of Skewness", "c", f64::NAN),
            ("Statistics", "Kurtosis", "c", f64::NAN),
            ("Statistics", "Std. Error of Kurtosis", "c", f64::NAN),
            ("Statistics", "N / Valid", "m", 0.0),
            ("Statistics", "N / Missing", "m", 9.0),
            ("Statistics", "Mean", "m", f64::NAN),
            ("Statistics", "Std. Error of Mean", "m", f64::NAN),
            ("Statistics", "Median", "m", f64::NAN),
            ("Statistics", "Mode", "m", f64::NAN),
            ("Statistics", "Std. Deviation", "m", f64::NAN),
            ("Statistics", "Variance", "m", f64::NAN),
            ("Statistics", "Skewness", "m", f64::NAN),
            ("Statistics", "Std. Error of Skewness", "m", f64::NAN),
            ("Statistics", "Kurtosis", "m", f64::NAN),
            ("Statistics", "Std. Error of Kurtosis", "m", f64::NAN),
            ("Statistics", "Range", "m", f64::NAN),
            ("Statistics", "Minimum", "m", f64::NAN),
            ("Statistics", "Maximum", "m", f64::NAN),
        ],
    );
}

#[test]
fn frequencies_with_a_zero_weight_and_missing_values() {
    let t = run(
        "WEIGHT BY w.\nFREQUENCIES VARIABLES=x y /STATISTICS=ALL /PERCENTILES=10 50 90 /FORMAT=NOTABLE.",
    );
    check(
        &t,
        &[
            ("Statistics", "N / Valid", "x", 7.0),
            ("Statistics", "N / Missing", "x", 2.0),
            ("Statistics", "Mean", "x", 5.28571428571),
            ("Statistics", "Std. Error of Mean", "x", 0.865042958314),
            ("Statistics", "Median", "x", 6.0),
            ("Statistics", "Mode", "x", 7.0),
            ("Statistics", "Std. Deviation", "x", 2.28868854109),
            ("Statistics", "Variance", "x", 5.2380952381),
            ("Statistics", "Skewness", "x", -0.371788941848),
            ("Statistics", "Std. Error of Skewness", "x", 0.793725393319),
            ("Statistics", "Kurtosis", "x", -1.68555371901),
            ("Statistics", "Std. Error of Kurtosis", "x", 1.58745078664),
            ("Statistics", "Range", "x", 6.0),
            ("Statistics", "Minimum", "x", 2.0),
            ("Statistics", "Maximum", "x", 8.0),
            ("Statistics", "Sum", "x", 37.0),
            ("Statistics", "Percentiles / 10", "x", 2.0),
            ("Statistics", "Percentiles / 50", "x", 6.0),
            ("Statistics", "Percentiles / 90", "x", 8.0),
            ("Statistics", "N / Valid", "y", 8.0),
            ("Statistics", "N / Missing", "y", 1.0),
            ("Statistics", "Mean", "y", 4.125),
            ("Statistics", "Std. Error of Mean", "y", 0.718070330817),
            ("Statistics", "Median", "y", 4.5),
            ("Statistics", "Mode", "y", 5.0),
            ("Statistics", "Std. Deviation", "y", 2.03100960116),
            ("Statistics", "Variance", "y", 4.125),
            ("Statistics", "Skewness", "y", -0.223802710872),
            ("Statistics", "Std. Error of Skewness", "y", 0.75210143309),
            ("Statistics", "Kurtosis", "y", -0.885950413223),
            ("Statistics", "Std. Error of Kurtosis", "y", 1.48088047897),
            ("Statistics", "Range", "y", 6.0),
            ("Statistics", "Minimum", "y", 1.0),
            ("Statistics", "Maximum", "y", 7.0),
            ("Statistics", "Sum", "y", 33.0),
            ("Statistics", "Percentiles / 10", "y", 1.0),
            ("Statistics", "Percentiles / 50", "y", 4.5),
            ("Statistics", "Percentiles / 90", "y", 7.0),
        ],
    );
}

#[test]
fn descriptives_of_one_case() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 1).\nDESCRIPTIVES VARIABLES=x y /STATISTICS=MEAN STDDEV VARIANCE SEMEAN KURTOSIS SKEWNESS.",
    );
    check(
        &t,
        &[
            ("Descriptive Statistics", "x", "N", 1.0),
            ("Descriptive Statistics", "x", "Mean", 3.0),
            ("Descriptive Statistics", "x", "S.E. Mean", f64::NAN),
            ("Descriptive Statistics", "x", "Std Dev", f64::NAN),
            ("Descriptive Statistics", "x", "Variance", f64::NAN),
            ("Descriptive Statistics", "x", "Skewness", f64::NAN),
            ("Descriptive Statistics", "x", "S.E. Skew", f64::NAN),
            ("Descriptive Statistics", "x", "Kurtosis", f64::NAN),
            ("Descriptive Statistics", "x", "S.E. Kurt", f64::NAN),
            ("Descriptive Statistics", "y", "N", 1.0),
            ("Descriptive Statistics", "y", "Mean", 2.0),
            ("Descriptive Statistics", "y", "S.E. Mean", f64::NAN),
            ("Descriptive Statistics", "y", "Std Dev", f64::NAN),
            ("Descriptive Statistics", "y", "Variance", f64::NAN),
            ("Descriptive Statistics", "y", "Skewness", f64::NAN),
            ("Descriptive Statistics", "y", "S.E. Skew", f64::NAN),
            ("Descriptive Statistics", "y", "Kurtosis", f64::NAN),
            ("Descriptive Statistics", "y", "S.E. Kurt", f64::NAN),
        ],
    );
}

#[test]
fn frequencies_of_one_case() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 1).\nFREQUENCIES VARIABLES=x y /STATISTICS=ALL /PERCENTILES=25 75 /FORMAT=NOTABLE.",
    );
    check(
        &t,
        &[
            ("Statistics", "Median", "x", 3.0),
            ("Statistics", "Mode", "x", 3.0),
            ("Statistics", "Range", "x", 0.0),
            ("Statistics", "Minimum", "x", 3.0),
            ("Statistics", "Maximum", "x", 3.0),
            ("Statistics", "Sum", "x", 3.0),
            ("Statistics", "Std. Deviation", "x", f64::NAN),
            ("Statistics", "Skewness", "x", f64::NAN),
            ("Statistics", "Kurtosis", "x", f64::NAN),
            ("Statistics", "Percentiles / 25", "x", 3.0),
            ("Statistics", "Percentiles / 75", "x", 3.0),
            ("Statistics", "Median", "y", 2.0),
            ("Statistics", "Mode", "y", 2.0),
            ("Statistics", "Range", "y", 0.0),
            ("Statistics", "Minimum", "y", 2.0),
            ("Statistics", "Maximum", "y", 2.0),
            ("Statistics", "Sum", "y", 2.0),
            ("Statistics", "Std. Deviation", "y", f64::NAN),
            ("Statistics", "Skewness", "y", f64::NAN),
            ("Statistics", "Kurtosis", "y", f64::NAN),
            ("Statistics", "Percentiles / 25", "y", 2.0),
            ("Statistics", "Percentiles / 75", "y", 2.0),
        ],
    );
}

#[test]
fn descriptives_of_two_cases() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 2).\nDESCRIPTIVES VARIABLES=x y /STATISTICS=MEAN STDDEV VARIANCE SEMEAN KURTOSIS SKEWNESS.",
    );
    check(
        &t,
        &[
            ("Descriptive Statistics", "x", "N", 2.0),
            ("Descriptive Statistics", "x", "Mean", 5.0),
            ("Descriptive Statistics", "x", "S.E. Mean", 2.0),
            ("Descriptive Statistics", "x", "Std Dev", 2.82842712475),
            ("Descriptive Statistics", "x", "Variance", 8.0),
            ("Descriptive Statistics", "x", "Skewness", f64::NAN),
            ("Descriptive Statistics", "x", "S.E. Skew", f64::NAN),
            ("Descriptive Statistics", "x", "Kurtosis", f64::NAN),
            ("Descriptive Statistics", "x", "S.E. Kurt", f64::NAN),
            ("Descriptive Statistics", "y", "N", 2.0),
            ("Descriptive Statistics", "y", "Mean", 3.5),
            ("Descriptive Statistics", "y", "S.E. Mean", 1.5),
            ("Descriptive Statistics", "y", "Std Dev", 2.12132034356),
            ("Descriptive Statistics", "y", "Variance", 4.5),
            ("Descriptive Statistics", "y", "Skewness", f64::NAN),
            ("Descriptive Statistics", "y", "S.E. Skew", f64::NAN),
            ("Descriptive Statistics", "y", "Kurtosis", f64::NAN),
            ("Descriptive Statistics", "y", "S.E. Kurt", f64::NAN),
        ],
    );
}

#[test]
fn frequencies_of_two_cases() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 2).\nFREQUENCIES VARIABLES=x y /STATISTICS=ALL /PERCENTILES=25 75 /FORMAT=NOTABLE.",
    );
    check(
        &t,
        &[
            ("Statistics", "Median", "x", 5.0),
            ("Statistics", "Mode", "x", 3.0),
            ("Statistics", "Range", "x", 4.0),
            ("Statistics", "Minimum", "x", 3.0),
            ("Statistics", "Maximum", "x", 7.0),
            ("Statistics", "Sum", "x", 10.0),
            ("Statistics", "Std. Deviation", "x", 2.82842712475),
            ("Statistics", "Skewness", "x", f64::NAN),
            ("Statistics", "Kurtosis", "x", f64::NAN),
            ("Statistics", "Percentiles / 25", "x", 3.0),
            ("Statistics", "Percentiles / 75", "x", 7.0),
            ("Statistics", "Median", "y", 3.5),
            ("Statistics", "Mode", "y", 2.0),
            ("Statistics", "Range", "y", 3.0),
            ("Statistics", "Minimum", "y", 2.0),
            ("Statistics", "Maximum", "y", 5.0),
            ("Statistics", "Sum", "y", 7.0),
            ("Statistics", "Std. Deviation", "y", 2.12132034356),
            ("Statistics", "Skewness", "y", f64::NAN),
            ("Statistics", "Kurtosis", "y", f64::NAN),
            ("Statistics", "Percentiles / 25", "y", 2.0),
            ("Statistics", "Percentiles / 75", "y", 5.0),
        ],
    );
}

#[test]
fn descriptives_of_three_cases() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 3).\nDESCRIPTIVES VARIABLES=x y /STATISTICS=MEAN STDDEV VARIANCE SEMEAN KURTOSIS SKEWNESS.",
    );
    check(
        &t,
        &[
            ("Descriptive Statistics", "x", "N", 3.0),
            ("Descriptive Statistics", "x", "Mean", 4.66666666667),
            ("Descriptive Statistics", "x", "S.E. Mean", 1.20185042515),
            ("Descriptive Statistics", "x", "Std Dev", 2.08166599947),
            ("Descriptive Statistics", "x", "Variance", 4.33333333333),
            ("Descriptive Statistics", "x", "Skewness", 1.29334278073),
            ("Descriptive Statistics", "x", "S.E. Skew", 1.22474487139),
            ("Descriptive Statistics", "x", "Kurtosis", f64::NAN),
            ("Descriptive Statistics", "x", "S.E. Kurt", f64::NAN),
            ("Descriptive Statistics", "y", "N", 3.0),
            ("Descriptive Statistics", "y", "Mean", 3.66666666667),
            ("Descriptive Statistics", "y", "S.E. Mean", 0.881917103688),
            ("Descriptive Statistics", "y", "Std Dev", 1.52752523165),
            ("Descriptive Statistics", "y", "Variance", 2.33333333333),
            ("Descriptive Statistics", "y", "Skewness", -0.935219529583),
            ("Descriptive Statistics", "y", "S.E. Skew", 1.22474487139),
            ("Descriptive Statistics", "y", "Kurtosis", f64::NAN),
            ("Descriptive Statistics", "y", "S.E. Kurt", f64::NAN),
        ],
    );
}

#[test]
fn frequencies_of_three_cases() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 3).\nFREQUENCIES VARIABLES=x y /STATISTICS=ALL /PERCENTILES=25 75 /FORMAT=NOTABLE.",
    );
    check(
        &t,
        &[
            ("Statistics", "Median", "x", 4.0),
            ("Statistics", "Mode", "x", 3.0),
            ("Statistics", "Range", "x", 4.0),
            ("Statistics", "Minimum", "x", 3.0),
            ("Statistics", "Maximum", "x", 7.0),
            ("Statistics", "Sum", "x", 14.0),
            ("Statistics", "Std. Deviation", "x", 2.08166599947),
            ("Statistics", "Skewness", "x", 1.29334278073),
            ("Statistics", "Kurtosis", "x", f64::NAN),
            ("Statistics", "Percentiles / 25", "x", 3.0),
            ("Statistics", "Percentiles / 75", "x", 7.0),
            ("Statistics", "Median", "y", 4.0),
            ("Statistics", "Mode", "y", 2.0),
            ("Statistics", "Range", "y", 3.0),
            ("Statistics", "Minimum", "y", 2.0),
            ("Statistics", "Maximum", "y", 5.0),
            ("Statistics", "Sum", "y", 11.0),
            ("Statistics", "Std. Deviation", "y", 1.52752523165),
            ("Statistics", "Skewness", "y", -0.935219529583),
            ("Statistics", "Kurtosis", "y", f64::NAN),
            ("Statistics", "Percentiles / 25", "y", 2.0),
            ("Statistics", "Percentiles / 75", "y", 5.0),
        ],
    );
}

#[test]
fn means_by_groups_of_one_case_and_an_empty_group() {
    let t = run("MEANS TABLES=x BY g /CELLS=COUNT MEAN STDDEV SUM MEDIAN SKEW MIN MAX.");
    // group 5 has no valid x: no row
    let table = t.iter().find(|t| t.title == "Report: x by g").unwrap();
    assert!(
        !row_keys(table).contains(&"5".to_string()),
        "{:?}",
        row_keys(table)
    );
    check(
        &t,
        &[
            ("Report: x by g", "1", "N", 3.0),
            ("Report: x by g", "1", "Mean", 4.66666666667),
            ("Report: x by g", "1", "Std. Deviation", 2.08166599947),
            ("Report: x by g", "1", "Sum", 14.0),
            ("Report: x by g", "1", "Median", 4.0),
            ("Report: x by g", "1", "Skewness", 1.29334278073),
            ("Report: x by g", "1", "Minimum", 3.0),
            ("Report: x by g", "1", "Maximum", 7.0),
            ("Report: x by g", "2", "N", 2.0),
            ("Report: x by g", "2", "Mean", 5.5),
            ("Report: x by g", "2", "Std. Deviation", 4.94974746831),
            ("Report: x by g", "2", "Sum", 11.0),
            ("Report: x by g", "2", "Median", 5.5),
            ("Report: x by g", "2", "Skewness", f64::NAN),
            ("Report: x by g", "2", "Minimum", 2.0),
            ("Report: x by g", "2", "Maximum", 9.0),
            ("Report: x by g", "3", "N", 1.0),
            ("Report: x by g", "3", "Mean", 8.0),
            ("Report: x by g", "3", "Std. Deviation", f64::NAN),
            ("Report: x by g", "3", "Sum", 8.0),
            ("Report: x by g", "3", "Median", 8.0),
            ("Report: x by g", "3", "Skewness", f64::NAN),
            ("Report: x by g", "3", "Minimum", 8.0),
            ("Report: x by g", "3", "Maximum", 8.0),
            ("Report: x by g", "4", "N", 1.0),
            ("Report: x by g", "4", "Mean", 6.0),
            ("Report: x by g", "4", "Std. Deviation", f64::NAN),
            ("Report: x by g", "4", "Sum", 6.0),
            ("Report: x by g", "4", "Median", 6.0),
            ("Report: x by g", "4", "Skewness", f64::NAN),
            ("Report: x by g", "4", "Minimum", 6.0),
            ("Report: x by g", "4", "Maximum", 6.0),
            ("Report: x by g", "Total", "N", 7.0),
            ("Report: x by g", "Total", "Mean", 5.57142857143),
            ("Report: x by g", "Total", "Std. Deviation", 2.63673679998),
            ("Report: x by g", "Total", "Sum", 39.0),
            ("Report: x by g", "Total", "Median", 6.0),
            ("Report: x by g", "Total", "Skewness", -0.112218409877),
            ("Report: x by g", "Total", "Minimum", 2.0),
            ("Report: x by g", "Total", "Maximum", 9.0),
        ],
    );
}

#[test]
fn means_with_a_zero_weight_leaving_one_case() {
    let t =
        run("WEIGHT BY w.\nMEANS TABLES=x BY g /CELLS=COUNT MEAN STDDEV SUM MEDIAN SKEW MIN MAX.");
    check(
        &t,
        &[
            ("Report: x by g", "1", "N", 4.0),
            ("Report: x by g", "1", "Mean", 5.25),
            ("Report: x by g", "1", "Std. Deviation", 2.06155281281),
            ("Report: x by g", "1", "Sum", 21.0),
            ("Report: x by g", "1", "Median", 5.5),
            ("Report: x by g", "1", "Skewness", -0.199735220618),
            ("Report: x by g", "1", "Minimum", 3.0),
            ("Report: x by g", "1", "Maximum", 7.0),
            ("Report: x by g", "2", "N", 1.0),
            ("Report: x by g", "2", "Mean", 2.0),
            ("Report: x by g", "2", "Std. Deviation", f64::NAN),
            ("Report: x by g", "2", "Sum", 2.0),
            ("Report: x by g", "2", "Median", 2.0),
            ("Report: x by g", "2", "Skewness", f64::NAN),
            ("Report: x by g", "2", "Minimum", 2.0),
            ("Report: x by g", "2", "Maximum", 2.0),
            ("Report: x by g", "3", "N", 1.0),
            ("Report: x by g", "3", "Mean", 8.0),
            ("Report: x by g", "3", "Std. Deviation", f64::NAN),
            ("Report: x by g", "3", "Sum", 8.0),
            ("Report: x by g", "3", "Median", 8.0),
            ("Report: x by g", "3", "Skewness", f64::NAN),
            ("Report: x by g", "3", "Minimum", 8.0),
            ("Report: x by g", "3", "Maximum", 8.0),
            ("Report: x by g", "4", "N", 1.0),
            ("Report: x by g", "4", "Mean", 6.0),
            ("Report: x by g", "4", "Std. Deviation", f64::NAN),
            ("Report: x by g", "4", "Sum", 6.0),
            ("Report: x by g", "4", "Median", 6.0),
            ("Report: x by g", "4", "Skewness", f64::NAN),
            ("Report: x by g", "4", "Minimum", 6.0),
            ("Report: x by g", "4", "Maximum", 6.0),
            ("Report: x by g", "Total", "N", 7.0),
            ("Report: x by g", "Total", "Mean", 5.28571428571),
            ("Report: x by g", "Total", "Std. Deviation", 2.28868854109),
            ("Report: x by g", "Total", "Sum", 37.0),
            ("Report: x by g", "Total", "Median", 6.0),
            ("Report: x by g", "Total", "Skewness", -0.371788941848),
            ("Report: x by g", "Total", "Minimum", 2.0),
            ("Report: x by g", "Total", "Maximum", 8.0),
        ],
    );
}

#[test]
fn correlations_of_a_constant_and_an_all_missing_variable() {
    let t = run("CORRELATIONS VARIABLES=x c m y /PRINT=TWOTAIL.");
    check(
        &t,
        &[
            ("Correlations", "x / Pearson Correlation", "x", 1.0),
            ("Correlations", "x / Sig. (2-tailed)", "x", f64::NAN),
            ("Correlations", "x / N", "x", 7.0),
            ("Correlations", "x / Pearson Correlation", "c", f64::NAN),
            ("Correlations", "x / Sig. (2-tailed)", "c", f64::NAN),
            ("Correlations", "x / N", "c", 7.0),
            ("Correlations", "x / Pearson Correlation", "m", f64::NAN),
            ("Correlations", "x / Sig. (2-tailed)", "m", f64::NAN),
            ("Correlations", "x / N", "m", 0.0),
            (
                "Correlations",
                "x / Pearson Correlation",
                "y",
                0.917677307388,
            ),
            ("Correlations", "x / Sig. (2-tailed)", "y", 0.00357129570122),
            ("Correlations", "x / N", "y", 7.0),
            ("Correlations", "c / Pearson Correlation", "x", f64::NAN),
            ("Correlations", "c / Sig. (2-tailed)", "x", f64::NAN),
            ("Correlations", "c / N", "x", 7.0),
            ("Correlations", "c / Pearson Correlation", "c", f64::NAN),
            ("Correlations", "c / Sig. (2-tailed)", "c", f64::NAN),
            ("Correlations", "c / N", "c", 9.0),
            ("Correlations", "c / Pearson Correlation", "m", f64::NAN),
            ("Correlations", "c / Sig. (2-tailed)", "m", f64::NAN),
            ("Correlations", "c / N", "m", 0.0),
            ("Correlations", "c / Pearson Correlation", "y", f64::NAN),
            ("Correlations", "c / Sig. (2-tailed)", "y", f64::NAN),
            ("Correlations", "c / N", "y", 8.0),
            ("Correlations", "m / Pearson Correlation", "x", f64::NAN),
            ("Correlations", "m / Sig. (2-tailed)", "x", f64::NAN),
            ("Correlations", "m / N", "x", 0.0),
            ("Correlations", "m / Pearson Correlation", "c", f64::NAN),
            ("Correlations", "m / Sig. (2-tailed)", "c", f64::NAN),
            ("Correlations", "m / N", "c", 0.0),
            ("Correlations", "m / Pearson Correlation", "m", f64::NAN),
            ("Correlations", "m / Sig. (2-tailed)", "m", f64::NAN),
            ("Correlations", "m / N", "m", 0.0),
            ("Correlations", "m / Pearson Correlation", "y", f64::NAN),
            ("Correlations", "m / Sig. (2-tailed)", "y", f64::NAN),
            ("Correlations", "m / N", "y", 0.0),
            (
                "Correlations",
                "y / Pearson Correlation",
                "x",
                0.917677307388,
            ),
            ("Correlations", "y / Sig. (2-tailed)", "x", 0.00357129570122),
            ("Correlations", "y / N", "x", 7.0),
            ("Correlations", "y / Pearson Correlation", "c", f64::NAN),
            ("Correlations", "y / Sig. (2-tailed)", "c", f64::NAN),
            ("Correlations", "y / N", "c", 8.0),
            ("Correlations", "y / Pearson Correlation", "m", f64::NAN),
            ("Correlations", "y / Sig. (2-tailed)", "m", f64::NAN),
            ("Correlations", "y / N", "m", 0.0),
            ("Correlations", "y / Pearson Correlation", "y", 1.0),
            ("Correlations", "y / Sig. (2-tailed)", "y", f64::NAN),
            ("Correlations", "y / N", "y", 8.0),
        ],
    );
}

#[test]
fn correlations_with_a_zero_weight() {
    let t = run("WEIGHT BY w.\nCORRELATIONS VARIABLES=x y /PRINT=TWOTAIL.");
    check(
        &t,
        &[
            ("Correlations", "x / Pearson Correlation", "x", 1.0),
            ("Correlations", "x / Sig. (2-tailed)", "x", f64::NAN),
            ("Correlations", "x / N", "x", 7.0),
            (
                "Correlations",
                "x / Pearson Correlation",
                "y",
                0.86608007538,
            ),
            ("Correlations", "x / Sig. (2-tailed)", "y", 0.0117132126268),
            ("Correlations", "x / N", "y", 7.0),
            (
                "Correlations",
                "y / Pearson Correlation",
                "x",
                0.86608007538,
            ),
            ("Correlations", "y / Sig. (2-tailed)", "x", 0.0117132126268),
            ("Correlations", "y / N", "x", 7.0),
            ("Correlations", "y / Pearson Correlation", "y", 1.0),
            ("Correlations", "y / Sig. (2-tailed)", "y", f64::NAN),
            ("Correlations", "y / N", "y", 8.0),
        ],
    );
}

#[test]
fn correlations_of_two_cases() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 2).\nCORRELATIONS VARIABLES=x y /PRINT=TWOTAIL.",
    );
    check(
        &t,
        &[
            ("Correlations", "x / Pearson Correlation", "x", 1.0),
            ("Correlations", "x / Sig. (2-tailed)", "x", f64::NAN),
            ("Correlations", "x / N", "x", 2.0),
            ("Correlations", "x / Pearson Correlation", "y", 1.0),
            ("Correlations", "x / Sig. (2-tailed)", "y", f64::NAN),
            ("Correlations", "x / N", "y", 2.0),
            ("Correlations", "y / Pearson Correlation", "x", 1.0),
            ("Correlations", "y / Sig. (2-tailed)", "x", f64::NAN),
            ("Correlations", "y / N", "x", 2.0),
            ("Correlations", "y / Pearson Correlation", "y", 1.0),
            ("Correlations", "y / Sig. (2-tailed)", "y", f64::NAN),
            ("Correlations", "y / N", "y", 2.0),
        ],
    );
}

#[test]
fn correlations_of_one_case() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 1).\nCORRELATIONS VARIABLES=x y /PRINT=TWOTAIL.",
    );
    check(
        &t,
        &[
            ("Correlations", "x / Pearson Correlation", "x", f64::NAN),
            ("Correlations", "x / Sig. (2-tailed)", "x", f64::NAN),
            ("Correlations", "x / N", "x", 1.0),
            ("Correlations", "x / Pearson Correlation", "y", f64::NAN),
            ("Correlations", "x / Sig. (2-tailed)", "y", f64::NAN),
            ("Correlations", "x / N", "y", 1.0),
            ("Correlations", "y / Pearson Correlation", "x", f64::NAN),
            ("Correlations", "y / Sig. (2-tailed)", "x", f64::NAN),
            ("Correlations", "y / N", "x", 1.0),
            ("Correlations", "y / Pearson Correlation", "y", f64::NAN),
            ("Correlations", "y / Sig. (2-tailed)", "y", f64::NAN),
            ("Correlations", "y / N", "y", 1.0),
        ],
    );
}
