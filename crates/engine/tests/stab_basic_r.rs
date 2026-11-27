//! Numerical stability of FREQUENCIES, DESCRIPTIVES, MEANS, CORRELATIONS, T-TEST and ONEWAY
//! against R on 24 invented cases whose values are binary fractions (so exact in a double):
//! `big` and `big2` are 1000000000 plus a spread of about 3, `tiny` is the spread of `big` times
//! 1e-9. The standard deviation, variance, skewness and kurtosis of `big` must equal those of
//! `tiny` scaled, and R's two-pass formulas, to ten digits: a running mean or a sum of squares
//! about a rounded mean loses eight. The groups' sums of squares and Levene's statistic are
//! written out in R (lm's own check calls the fit perfect at this offset). Scripts in
//! lab/oracles/stab-r1.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g big big2 tiny off small yb (7F20.12).
BEGIN DATA.
1 1000000005.000000000000 1000000002.250000000000 0.000000006000 1000000.000000000000 0.000004250000 0.000000000000
2 1000000000.750000000000 1000000001.000000000000 0.000000001750 1000000.500000000000 0.000003000000 0.000000000000
3 1000000001.500000000000 1000000005.750000000000 0.000000002500 1000001.000000000000 0.000007750000 0.000000000000
1 1000000008.000000000000 1000000003.250000000000 0.000000009000 1000001.500000000000 0.000005250000 1.000000000000
2 1000000006.750000000000 1000000006.500000000000 0.000000007750 1000002.000000000000 0.000008500000 1.000000000000
3 1000000001.250000000000 1000000008.750000000000 0.000000002250 1000000.000000000000 0.000010750000 1.000000000000
1 1000000009.000000000000 1000000001.750000000000 0.000000010000 1000000.500000000000 0.000003750000 1.000000000000
2 1000000010.000000000000 1000000010.000000000000 0.000000011000 1000001.000000000000 0.000012000000 1.000000000000
3 1000000000.750000000000 1000000009.000000000000 0.000000001750 1000001.500000000000 0.000011000000 1.000000000000
1 1000000000.750000000000 1000000003.500000000000 0.000000001750 1000002.000000000000 0.000005500000 0.000000000000
2 1000000002.000000000000 1000000004.500000000000 0.000000003000 1000000.000000000000 0.000006500000 0.000000000000
3 1000000008.500000000000 1000000001.750000000000 0.000000009500 1000000.500000000000 0.000003750000 1.000000000000
1 1000000008.750000000000 1000000002.750000000000 0.000000009750 1000001.000000000000 0.000004750000 1.000000000000
2 1000000009.000000000000 1000000010.000000000000 0.000000010000 1000001.500000000000 0.000012000000 1.000000000000
3 1000000001.500000000000 1000000008.750000000000 0.000000002500 1000002.000000000000 0.000010750000 1.000000000000
1 1000000009.000000000000 1000000000.750000000000 0.000000010000 1000000.000000000000 0.000002750000 1.000000000000
2 1000000007.750000000000 1000000008.500000000000 0.000000008750 1000000.500000000000 0.000010500000 1.000000000000
3 1000000005.000000000000 1000000007.250000000000 0.000000006000 1000001.000000000000 0.000009250000 1.000000000000
1 1000000007.250000000000 1000000005.750000000000 0.000000008250 1000001.500000000000 0.000007750000 1.000000000000
2 1000000002.750000000000 1000000003.750000000000 0.000000003750 1000002.000000000000 0.000005750000 0.000000000000
3 1000000004.750000000000 1000000008.250000000000 0.000000005750 1000000.000000000000 0.000010250000 1.000000000000
1 1000000005.250000000000 1000000007.000000000000 0.000000006250 1000000.500000000000 0.000009000000 1.000000000000
2 1000000001.000000000000 1000000001.750000000000 0.000000002000 1000001.000000000000 0.000003750000 0.000000000000
3 1000000002.500000000000 1000000005.250000000000 0.000000003500 1000001.500000000000 0.000007250000 0.000000000000
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
            (n - want).abs() <= 1e-10 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn descriptives_of_values_far_from_zero_and_very_small() {
    let t = run(
        "DESCRIPTIVES VARIABLES=big big2 tiny /STATISTICS=MEAN STDDEV VARIANCE SEMEAN KURTOSIS SKEWNESS.",
    );
    check(
        &t,
        &[
            ("Descriptive Statistics", "big", "N", 24.0),
            ("Descriptive Statistics", "big", "Mean", 1000000004.94792),
            (
                "Descriptive Statistics",
                "big",
                "S.E. Mean",
                0.67716694021154,
            ),
            ("Descriptive Statistics", "big", "Std Dev", 3.31742694840007),
            ("Descriptive Statistics", "big", "Variance", 11.005321557971),
            (
                "Descriptive Statistics",
                "big",
                "Skewness",
                0.0388614164922998,
            ),
            (
                "Descriptive Statistics",
                "big",
                "S.E. Skew",
                0.472260842152198,
            ),
            (
                "Descriptive Statistics",
                "big",
                "Kurtosis",
                -1.66606919165558,
            ),
            (
                "Descriptive Statistics",
                "big",
                "S.E. Kurt",
                0.917777082601901,
            ),
            ("Descriptive Statistics", "big2", "N", 24.0),
            ("Descriptive Statistics", "big2", "Mean", 1000000005.32292),
            (
                "Descriptive Statistics",
                "big2",
                "S.E. Mean",
                0.620359432736397,
            ),
            (
                "Descriptive Statistics",
                "big2",
                "Std Dev",
                3.03912813465319,
            ),
            (
                "Descriptive Statistics",
                "big2",
                "Variance",
                9.23629981884058,
            ),
            (
                "Descriptive Statistics",
                "big2",
                "Skewness",
                0.0455250644012432,
            ),
            (
                "Descriptive Statistics",
                "big2",
                "S.E. Skew",
                0.472260842152198,
            ),
            (
                "Descriptive Statistics",
                "big2",
                "Kurtosis",
                -1.41020517050729,
            ),
            (
                "Descriptive Statistics",
                "big2",
                "S.E. Kurt",
                0.917777082601901,
            ),
            ("Descriptive Statistics", "tiny", "N", 24.0),
            (
                "Descriptive Statistics",
                "tiny",
                "Mean",
                5.94791666666667e-09,
            ),
            (
                "Descriptive Statistics",
                "tiny",
                "S.E. Mean",
                6.7716694021154e-10,
            ),
            (
                "Descriptive Statistics",
                "tiny",
                "Std Dev",
                3.31742694840007e-09,
            ),
            (
                "Descriptive Statistics",
                "tiny",
                "Variance",
                1.1005321557971e-17,
            ),
            (
                "Descriptive Statistics",
                "tiny",
                "Skewness",
                0.0388614164922999,
            ),
            (
                "Descriptive Statistics",
                "tiny",
                "S.E. Skew",
                0.472260842152198,
            ),
            (
                "Descriptive Statistics",
                "tiny",
                "Kurtosis",
                -1.66606919165558,
            ),
            (
                "Descriptive Statistics",
                "tiny",
                "S.E. Kurt",
                0.917777082601901,
            ),
        ],
    );
}

#[test]
fn frequencies_of_values_far_from_zero_and_very_small() {
    let t =
        run("FREQUENCIES VARIABLES=big tiny /STATISTICS=ALL /PERCENTILES=25 75 /FORMAT=NOTABLE.");
    check(
        &t,
        &[
            ("Statistics", "Mean", "big", 1000000004.94792),
            ("Statistics", "Std. Error of Mean", "big", 0.67716694021154),
            ("Statistics", "Median", "big", 1000000005.0),
            ("Statistics", "Std. Deviation", "big", 3.31742694840007),
            ("Statistics", "Variance", "big", 11.005321557971),
            ("Statistics", "Skewness", "big", 0.0388614164922998),
            ("Statistics", "Kurtosis", "big", -1.66606919165558),
            ("Statistics", "Range", "big", 9.25),
            ("Statistics", "Minimum", "big", 1000000000.75),
            ("Statistics", "Maximum", "big", 1000000010.0),
            ("Statistics", "Sum", "big", 24000000118.75),
            ("Statistics", "Percentiles / 25", "big", 1000000001.5),
            ("Statistics", "Percentiles / 75", "big", 1000000008.375),
            ("Statistics", "Mean", "tiny", 5.94791666666667e-09),
            (
                "Statistics",
                "Std. Error of Mean",
                "tiny",
                6.7716694021154e-10,
            ),
            ("Statistics", "Median", "tiny", 6e-09),
            ("Statistics", "Std. Deviation", "tiny", 3.31742694840007e-09),
            ("Statistics", "Variance", "tiny", 1.1005321557971e-17),
            ("Statistics", "Skewness", "tiny", 0.0388614164922999),
            ("Statistics", "Kurtosis", "tiny", -1.66606919165558),
            ("Statistics", "Range", "tiny", 9.25e-09),
            ("Statistics", "Minimum", "tiny", 1.75e-09),
            ("Statistics", "Maximum", "tiny", 1.1e-08),
            ("Statistics", "Sum", "tiny", 1.4275e-07),
            ("Statistics", "Percentiles / 25", "tiny", 2.5e-09),
            ("Statistics", "Percentiles / 75", "tiny", 9.375e-09),
        ],
    );
}

#[test]
fn correlations_of_values_far_from_zero_and_very_small() {
    let t = run("CORRELATIONS VARIABLES=big big2 tiny /PRINT=TWOTAIL.");
    check(
        &t,
        &[
            (
                "Correlations",
                "big / Pearson Correlation",
                "big2",
                -0.0120051686526866,
            ),
            (
                "Correlations",
                "big / Sig. (2-tailed)",
                "big2",
                0.955600550211704,
            ),
            ("Correlations", "big / Pearson Correlation", "tiny", 1.0),
            ("Correlations", "big / Sig. (2-tailed)", "tiny", f64::NAN),
            (
                "Correlations",
                "big2 / Pearson Correlation",
                "big",
                -0.0120051686526866,
            ),
            (
                "Correlations",
                "big2 / Sig. (2-tailed)",
                "big",
                0.955600550211704,
            ),
            (
                "Correlations",
                "big2 / Pearson Correlation",
                "tiny",
                -0.0120051686526867,
            ),
            (
                "Correlations",
                "big2 / Sig. (2-tailed)",
                "tiny",
                0.955600550211704,
            ),
            ("Correlations", "tiny / Pearson Correlation", "big", 1.0),
            ("Correlations", "tiny / Sig. (2-tailed)", "big", f64::NAN),
            (
                "Correlations",
                "tiny / Pearson Correlation",
                "big2",
                -0.0120051686526867,
            ),
            (
                "Correlations",
                "tiny / Sig. (2-tailed)",
                "big2",
                0.955600550211704,
            ),
        ],
    );
}

#[test]
fn means_by_group_of_values_far_from_zero() {
    let t = run(
        "MEANS TABLES=big BY g /CELLS=COUNT MEAN STDDEV SUM MEDIAN VARIANCE SKEW KURT MIN MAX.",
    );
    check(
        &t,
        &[
            ("Report: big by g", "1", "N", 8.0),
            ("Report: big by g", "1", "Mean", 1000000006.625),
            ("Report: big by g", "1", "Std. Deviation", 2.85669639368875),
            ("Report: big by g", "1", "Sum", 8000000053.0),
            ("Report: big by g", "1", "Median", 1000000007.625),
            ("Report: big by g", "1", "Variance", 8.16071428571429),
            ("Report: big by g", "1", "Skewness", -1.39255896101329),
            ("Report: big by g", "1", "Kurtosis", 1.73927957519547),
            ("Report: big by g", "1", "Minimum", 1000000000.75),
            ("Report: big by g", "1", "Maximum", 1000000009.0),
            ("Report: big by g", "2", "N", 8.0),
            ("Report: big by g", "2", "Mean", 1000000005.0),
            ("Report: big by g", "2", "Std. Deviation", 3.77491721763537),
            ("Report: big by g", "2", "Sum", 8000000040.0),
            ("Report: big by g", "2", "Median", 1000000004.75),
            ("Report: big by g", "2", "Variance", 14.25),
            ("Report: big by g", "2", "Skewness", 0.127473790755132),
            ("Report: big by g", "2", "Kurtosis", -2.13696082311041),
            ("Report: big by g", "2", "Minimum", 1000000000.75),
            ("Report: big by g", "2", "Maximum", 1000000010.0),
            ("Report: big by g", "3", "N", 8.0),
            ("Report: big by g", "3", "Mean", 1000000003.21875),
            ("Report: big by g", "3", "Std. Deviation", 2.66738736540887),
            ("Report: big by g", "3", "Sum", 8000000025.75),
            ("Report: big by g", "3", "Median", 1000000002.0),
            ("Report: big by g", "3", "Variance", 7.11495535714286),
            ("Report: big by g", "3", "Skewness", 1.23791543846271),
            ("Report: big by g", "3", "Kurtosis", 0.947612080276819),
            ("Report: big by g", "3", "Minimum", 1000000000.75),
            ("Report: big by g", "3", "Maximum", 1000000008.5),
            ("Report: big by g", "Total", "N", 24.0),
            ("Report: big by g", "Total", "Mean", 1000000004.94792),
            (
                "Report: big by g",
                "Total",
                "Std. Deviation",
                3.31742694840007,
            ),
            ("Report: big by g", "Total", "Sum", 24000000118.75),
            ("Report: big by g", "Total", "Median", 1000000005.0),
            ("Report: big by g", "Total", "Variance", 11.005321557971),
            ("Report: big by g", "Total", "Skewness", 0.0388614164922998),
            ("Report: big by g", "Total", "Kurtosis", -1.66606919165558),
            ("Report: big by g", "Total", "Minimum", 1000000000.75),
            ("Report: big by g", "Total", "Maximum", 1000000010.0),
        ],
    );
}

#[test]
fn independent_t_test_of_values_far_from_zero_and_very_small() {
    let t = run("T-TEST GROUPS=g(1,2) /VARIABLES=big tiny.");
    check(
        &t,
        &[
            ("Group Statistics", "big / 1", "Mean", 1000000006.625),
            (
                "Group Statistics",
                "big / 1",
                "Std. Deviation",
                2.85669639368875,
            ),
            (
                "Group Statistics",
                "big / 1",
                "Std. Error Mean",
                1.00999469588423,
            ),
            ("Group Statistics", "big / 2", "Mean", 1000000005.0),
            (
                "Group Statistics",
                "big / 2",
                "Std. Deviation",
                3.77491721763537,
            ),
            (
                "Group Statistics",
                "big / 2",
                "Std. Error Mean",
                1.33463478150391,
            ),
            (
                "Independent Samples Test",
                "big / Equal variances assumed",
                "t-test for Equality of Means / t",
                0.970891076316861,
            ),
            (
                "Independent Samples Test",
                "big / Equal variances assumed",
                "t-test for Equality of Means / df",
                14.0,
            ),
            (
                "Independent Samples Test",
                "big / Equal variances assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                0.348078664422712,
            ),
            (
                "Independent Samples Test",
                "big / Equal variances assumed",
                "t-test for Equality of Means / Mean Difference",
                1.625,
            ),
            (
                "Independent Samples Test",
                "big / Equal variances assumed",
                "t-test for Equality of Means / Std. Error Difference",
                1.67372019337591,
            ),
            (
                "Independent Samples Test",
                "big / Equal variances not assumed",
                "t-test for Equality of Means / t",
                0.970891076316861,
            ),
            (
                "Independent Samples Test",
                "big / Equal variances not assumed",
                "t-test for Equality of Means / df",
                13.0374692693102,
            ),
            (
                "Independent Samples Test",
                "big / Equal variances not assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                0.349271182572618,
            ),
            (
                "Independent Samples Test",
                "big / Equal variances assumed",
                "Levene's Test for Equality of Variances / F",
                2.83940740740741,
            ),
            (
                "Independent Samples Test",
                "big / Equal variances assumed",
                "Levene's Test for Equality of Variances / Sig.",
                0.114129245619704,
            ),
            ("Group Statistics", "tiny / 1", "Mean", 7.625e-09),
            (
                "Group Statistics",
                "tiny / 1",
                "Std. Deviation",
                2.85669639368875e-09,
            ),
            (
                "Group Statistics",
                "tiny / 1",
                "Std. Error Mean",
                1.00999469588423e-09,
            ),
            ("Group Statistics", "tiny / 2", "Mean", 6e-09),
            (
                "Group Statistics",
                "tiny / 2",
                "Std. Deviation",
                3.77491721763537e-09,
            ),
            (
                "Group Statistics",
                "tiny / 2",
                "Std. Error Mean",
                1.33463478150391e-09,
            ),
            (
                "Independent Samples Test",
                "tiny / Equal variances assumed",
                "t-test for Equality of Means / t",
                0.970891076316861,
            ),
            (
                "Independent Samples Test",
                "tiny / Equal variances assumed",
                "t-test for Equality of Means / df",
                14.0,
            ),
            (
                "Independent Samples Test",
                "tiny / Equal variances assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                0.348078664422712,
            ),
            (
                "Independent Samples Test",
                "tiny / Equal variances assumed",
                "t-test for Equality of Means / Mean Difference",
                1.625e-09,
            ),
            (
                "Independent Samples Test",
                "tiny / Equal variances assumed",
                "t-test for Equality of Means / Std. Error Difference",
                1.67372019337591e-09,
            ),
            (
                "Independent Samples Test",
                "tiny / Equal variances not assumed",
                "t-test for Equality of Means / t",
                0.970891076316861,
            ),
            (
                "Independent Samples Test",
                "tiny / Equal variances not assumed",
                "t-test for Equality of Means / df",
                13.0374692693102,
            ),
            (
                "Independent Samples Test",
                "tiny / Equal variances not assumed",
                "t-test for Equality of Means / Sig. (2-tailed)",
                0.349271182572617,
            ),
            (
                "Independent Samples Test",
                "tiny / Equal variances assumed",
                "Levene's Test for Equality of Variances / F",
                2.83940740740741,
            ),
            (
                "Independent Samples Test",
                "tiny / Equal variances assumed",
                "Levene's Test for Equality of Variances / Sig.",
                0.114129245619704,
            ),
        ],
    );
}

#[test]
fn paired_t_test_of_values_far_from_zero() {
    let t = run("T-TEST PAIRS=big WITH big2 (PAIRED).");
    check(
        &t,
        &[
            (
                "Paired Samples Test",
                "Pair 1 / big - big2",
                "Paired Differences / Mean",
                -0.375,
            ),
            (
                "Paired Samples Test",
                "Pair 1 / big - big2",
                "Paired Differences / Std. Deviation",
                4.52589169691166,
            ),
            (
                "Paired Samples Test",
                "Pair 1 / big - big2",
                "t",
                -0.405912785836431,
            ),
            (
                "Paired Samples Test",
                "Pair 1 / big - big2",
                "Sig. (2-tailed)",
                0.68855834130908,
            ),
            (
                "Paired Samples Correlations",
                "Pair 1 / big & big2",
                "Correlation",
                -0.0120051686526866,
            ),
        ],
    );
}

#[test]
fn oneway_of_values_far_from_zero() {
    let t = run("ONEWAY big BY g /STATISTICS=DESCRIPTIVES HOMOGENEITY.");
    check(
        &t,
        &[
            (
                "ANOVA",
                "big / Between Groups",
                "Sum of Squares",
                46.4427083333334,
            ),
            (
                "ANOVA",
                "big / Within Groups",
                "Sum of Squares",
                206.6796875,
            ),
            ("ANOVA", "big / Total", "Sum of Squares", 253.122395833333),
            ("ANOVA", "big / Between Groups", "F", 2.35944055944056),
            ("ANOVA", "big / Between Groups", "Sig.", 0.119029839834103),
            (
                "Descriptives",
                "big / 1",
                "Std. Deviation",
                2.85669639368875,
            ),
            ("Descriptives", "big / 1", "Mean", 1000000006.625),
            (
                "Descriptives",
                "big / 2",
                "Std. Deviation",
                3.77491721763537,
            ),
            ("Descriptives", "big / 2", "Mean", 1000000005.0),
            (
                "Descriptives",
                "big / 3",
                "Std. Deviation",
                2.66738736540887,
            ),
            ("Descriptives", "big / 3", "Mean", 1000000003.21875),
            (
                "Test of Homogeneity of Variances",
                "big / Based on Mean",
                "Levene Statistic",
                2.03071850699844,
            ),
        ],
    );
}
