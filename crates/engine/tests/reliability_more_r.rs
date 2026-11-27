//! RELIABILITY against R on 45 invented cases of six items: the split model in two equal parts
//! (SPLIT(3)) and in unequal parts of 4 and 2 items (SPLIT(2): the last 2 items are the second part), with the alpha of each part, the
//! correlation between the forms, the Spearman-Brown coefficients (equal length 2R / (1 + R) and
//! the unequal length formula of the algorithms reference) and Guttman's split-half coefficient;
//! the item statistics, the inter-item correlation and covariance matrices, the summary item
//! statistics of each part and of both parts (the mean, minimum, maximum, range, ratio and
//! variance of the item means, the item variances and the off-diagonal covariances and
//! correlations, over the items of the part; the variance of the covariances and correlations over
//! all ordered pairs with the denominator of the manual, as in reliability_r.rs), the item-total statistics (scale mean and variance
//! without the item, corrected item-total correlation, squared multiple correlation from `lm` of
//! the item on the others, alpha if deleted) and the scale statistics of each part. Scripts in
//! lab/oracles/reliability-r3.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /i1 i2 i3 i4 i5 i6.
BEGIN DATA.
1 -0.5 1.5 0 3.1 1.6
1.7 1.7 1.5 1.5 2.6 1
4.7 4 5.2 2.6 4.7 3.6
1.8 2.1 1.9 2.4 1.3 1.8
2.2 3.6 3.1 4 2.3 1.4
1.9 3 1.1 1.2 1.3 1.1
2.2 2.9 1.8 2.8 1.2 3.1
0 1 0.8 0.7 1.5 1.7
2.3 3.7 4.2 2.8 1.8 3.6
4.4 5.4 2.5 3.3 1.8 4.2
1.2 4 1 4.2 2.1 2
4 3.6 2.8 6.2 1.7 2.9
3.8 2.1 3.1 4.1 3.1 3.4
3.4 3.6 2.5 4.2 3.6 3.6
3.6 2.6 2.8 1.6 2.6 3.2
2.4 0.3 0.6 2.2 1.2 0.9
2.1 2.3 4 1.4 2.9 3.2
5.5 3.5 5.5 3.4 5.7 2.7
2.7 2.9 3.7 3.5 5.2 5
2.9 4.8 4.3 2.8 4.2 4.4
2.1 1.8 2.5 2.7 0.8 3.3
2.8 3.4 3.5 3 2.8 3.3
2.3 4.1 2.1 2.4 3.3 4.9
2.5 0.7 1.7 4.6 2 2.5
2.7 3.3 3.3 4.6 2.8 3.6
1.8 3.7 1.9 2.3 3.3 4.2
3.7 3.5 4 4.6 2.2 4.9
3.7 3.8 3.1 3.6 3.9 1.6
4.4 3.8 4 3.7 3.6 4.8
2.5 4.5 2.7 1.3 3.9 4.6
2.8 4.2 4.2 3 3.4 2.3
2.6 2.8 2.3 2.7 3.9 1.3
1.8 2.2 1.9 1.3 2.4 2.7
2.5 2.8 1.8 3.1 4.7 1.3
2.6 1.6 2.1 2.7 1.3 2.2
3.8 2.7 3.7 3.2 4.4 4.2
3 2.5 3.1 3.5 1.6 2.4
1.6 2.7 0.7 2.1 2.8 1.9
2.1 1.8 3.6 3.7 4.2 3.1
2.7 1.5 2.8 4.2 3.2 2.8
2.9 4 1.2 3.9 4.2 2.6
3.3 2.3 3.4 2.6 3.3 4.4
3.6 3.7 3.1 2.7 4.8 3.8
2.7 4.3 3.8 2.4 2.5 4.4
2.8 3.1 2.2 2.5 3.5 2.1
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

/// The row headers joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                if let Cell::Text(t) = cell
                    && !t.is_empty()
                {
                    carried[level] = t.clone();
                    for deeper in carried.iter_mut().skip(level + 1) {
                        deeper.clear();
                    }
                }
            }
            carried
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .collect()
}

/// Each expectation: the table title, the row, the column and the value.
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
            // the variance of a single value is not defined
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
fn split_half_in_equal_parts_the_matrices_and_the_summaries_agree_with_r() {
    let t = run(
        "RELIABILITY VARIABLES=i1 TO i6 /STATISTICS=CORR COV DESCRIPTIVES SCALE /MODEL=SPLIT(3) /SUMMARY=TOTAL MEANS VARIANCE COV CORR.",
    );
    check(
        &t,
        &[
            (
                "Reliability Statistics",
                "Cronbach's Alpha / Part 1 / Value",
                "Value",
                0.7550281033,
            ),
            (
                "Reliability Statistics",
                "Cronbach's Alpha / Part 1 / N of Items",
                "Value",
                3.0,
            ),
            (
                "Reliability Statistics",
                "Cronbach's Alpha / Part 2 / Value",
                "Value",
                0.4219113405,
            ),
            (
                "Reliability Statistics",
                "Cronbach's Alpha / Part 2 / N of Items",
                "Value",
                3.0,
            ),
            (
                "Reliability Statistics",
                "Cronbach's Alpha / Total N of Items",
                "Value",
                6.0,
            ),
            (
                "Reliability Statistics",
                "Correlation Between Forms",
                "Value",
                0.7447547973,
            ),
            (
                "Reliability Statistics",
                "Spearman-Brown Coefficient / Equal Length",
                "Value",
                0.8537071208,
            ),
            (
                "Reliability Statistics",
                "Spearman-Brown Coefficient / Unequal Length",
                "Value",
                0.8537071208,
            ),
            (
                "Reliability Statistics",
                "Guttman Split-Half Coefficient",
                "Value",
                0.8489051329,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "Mean",
                2.793333333,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "Minimum",
                2.724444444,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "Maximum",
                2.92,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "Range",
                0.1955555556,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "Maximum / Minimum",
                1.07177814,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "Variance",
                0.01206419753,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "N of Items",
                3.0,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "Mean",
                1.299531987,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "Minimum",
                1.049616162,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "Maximum",
                1.464363636,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "Range",
                0.4147474747,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "Maximum / Minimum",
                1.395142043,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "Variance",
                0.04843335459,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "N of Items",
                3.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "Mean",
                0.658537037,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "Minimum",
                0.5685909091,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "Maximum",
                0.8057020202,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "Range",
                0.2371111111,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "Maximum / Minimum",
                1.417015305,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "Variance",
                0.01320873424,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "N of Items",
                3.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "Mean",
                0.5164191024,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "Minimum",
                0.4222942514,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "Maximum",
                0.6683354532,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "Range",
                0.2460412018,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "Maximum / Minimum",
                1.582629768,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "Variance",
                0.01411116907,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "N of Items",
                3.0,
            ),
            ("Scale Statistics", "Part 1", "Mean", 8.38),
            ("Scale Statistics", "Part 1", "Variance", 7.849818182),
            ("Scale Statistics", "Part 1", "Std. Deviation", 2.801752698),
            ("Scale Statistics", "Part 1", "N of Items", 3.0),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "Mean",
                2.945185185,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "Minimum",
                2.917777778,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "Maximum",
                2.968888889,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "Range",
                0.05111111111,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "Maximum / Minimum",
                1.017517136,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "Variance",
                0.0006633744856,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "N of Items",
                3.0,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "Mean",
                1.420262626,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "Minimum",
                1.391949495,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "Maximum",
                1.453919192,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "Range",
                0.06196969697,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "Maximum / Minimum",
                1.044520076,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "Variance",
                0.0009814750536,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "N of Items",
                3.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "Mean",
                0.2779107744,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "Minimum",
                0.1234292929,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "Maximum",
                0.4138282828,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "Range",
                0.2903989899,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "Maximum / Minimum",
                3.352755841,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "Variance",
                0.01707308732,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "N of Items",
                3.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "Mean",
                0.1955151346,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "Minimum",
                0.08676336108,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "Maximum",
                0.2885255088,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "Range",
                0.2017621477,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "Maximum / Minimum",
                3.325430288,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "Variance",
                0.008290267844,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "N of Items",
                3.0,
            ),
            ("Scale Statistics", "Part 2", "Mean", 8.835555556),
            ("Scale Statistics", "Part 2", "Variance", 5.928252525),
            ("Scale Statistics", "Part 2", "Std. Deviation", 2.434800305),
            ("Scale Statistics", "Part 2", "N of Items", 3.0),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "Mean",
                2.869259259,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "Minimum",
                2.724444444,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "Maximum",
                2.968888889,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "Range",
                0.2444444444,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "Maximum / Minimum",
                1.089722675,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "Variance",
                0.01200872428,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "N of Items",
                6.0,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "Mean",
                1.359897306,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "Minimum",
                1.049616162,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "Maximum",
                1.464363636,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "Range",
                0.4147474747,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "Maximum / Minimum",
                1.395142043,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "Variance",
                0.02413869807,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "N of Items",
                6.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "Mean",
                0.5259895623,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "Minimum",
                0.1234292929,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "Maximum",
                0.8057020202,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "Range",
                0.6822727273,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "Maximum / Minimum",
                6.527640247,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "Variance",
                0.03187975063,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "N of Items",
                6.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "Mean",
                0.3934145943,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "Minimum",
                0.08676336108,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "Maximum",
                0.6683354532,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "Range",
                0.5815720921,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "Maximum / Minimum",
                7.702968682,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "Variance",
                0.02058723686,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "N of Items",
                6.0,
            ),
            ("Scale Statistics", "Both Parts", "Mean", 17.21555556),
            ("Scale Statistics", "Both Parts", "Variance", 23.93907071),
            (
                "Scale Statistics",
                "Both Parts",
                "Std. Deviation",
                4.892756964,
            ),
            ("Scale Statistics", "Both Parts", "N of Items", 6.0),
            ("Item Statistics", "i1", "Mean", 2.735555556),
            ("Item Statistics", "i1", "Std. Deviation", 1.024507766),
            ("Item Statistics", "i1", "N", 45.0),
            ("Inter-Item Correlation Matrix", "i1", "i1", 1.0),
            ("Inter-Item Correlation Matrix", "i1", "i2", 0.4586276025),
            ("Inter-Item Correlation Matrix", "i1", "i3", 0.6683354532),
            ("Inter-Item Correlation Matrix", "i1", "i4", 0.4951044687),
            ("Inter-Item Correlation Matrix", "i1", "i5", 0.4351367104),
            ("Inter-Item Correlation Matrix", "i1", "i6", 0.4190492113),
            ("Inter-Item Covariance Matrix", "i1", "i1", 1.049616162),
            ("Inter-Item Covariance Matrix", "i1", "i2", 0.5685909091),
            ("Inter-Item Covariance Matrix", "i1", "i3", 0.8057020202),
            ("Inter-Item Covariance Matrix", "i1", "i4", 0.5984444444),
            ("Inter-Item Covariance Matrix", "i1", "i5", 0.537540404),
            ("Inter-Item Covariance Matrix", "i1", "i6", 0.5106767677),
            (
                "Item-Total Statistics",
                "i1",
                "Scale Mean if Item Deleted",
                14.48,
            ),
            (
                "Item-Total Statistics",
                "i1",
                "Scale Variance if Item Deleted",
                16.84754545,
            ),
            (
                "Item-Total Statistics",
                "i1",
                "Corrected Item-Total Correlation",
                0.7183905615,
            ),
            (
                "Item-Total Statistics",
                "i1",
                "Squared Multiple Correlation",
                0.581555819,
            ),
            (
                "Item-Total Statistics",
                "i1",
                "Cronbach's Alpha if Item Deleted",
                0.7224923212,
            ),
            ("Item Statistics", "i2", "Mean", 2.92),
            ("Item Statistics", "i2", "Std. Deviation", 1.210108936),
            ("Item Statistics", "i2", "N", 45.0),
            ("Inter-Item Correlation Matrix", "i2", "i1", 0.4586276025),
            ("Inter-Item Correlation Matrix", "i2", "i2", 1.0),
            ("Inter-Item Correlation Matrix", "i2", "i3", 0.4222942514),
            ("Inter-Item Correlation Matrix", "i2", "i4", 0.3049092501),
            ("Inter-Item Correlation Matrix", "i2", "i5", 0.3081844527),
            ("Inter-Item Correlation Matrix", "i2", "i6", 0.4864302138),
            ("Inter-Item Covariance Matrix", "i2", "i1", 0.5685909091),
            ("Inter-Item Covariance Matrix", "i2", "i2", 1.464363636),
            ("Inter-Item Covariance Matrix", "i2", "i3", 0.6013181818),
            ("Inter-Item Covariance Matrix", "i2", "i4", 0.4353181818),
            ("Inter-Item Covariance Matrix", "i2", "i5", 0.4496818182),
            ("Inter-Item Covariance Matrix", "i2", "i6", 0.7001818182),
            (
                "Item-Total Statistics",
                "i2",
                "Scale Mean if Item Deleted",
                14.29555556,
            ),
            (
                "Item-Total Statistics",
                "i2",
                "Scale Variance if Item Deleted",
                16.96452525,
            ),
            (
                "Item-Total Statistics",
                "i2",
                "Corrected Item-Total Correlation",
                0.5527650942,
            ),
            (
                "Item-Total Statistics",
                "i2",
                "Squared Multiple Correlation",
                0.3323078221,
            ),
            (
                "Item-Total Statistics",
                "i2",
                "Cronbach's Alpha if Item Deleted",
                0.7566896876,
            ),
            ("Item Statistics", "i3", "Mean", 2.724444444),
            ("Item Statistics", "i3", "Std. Deviation", 1.176697141),
            ("Item Statistics", "i3", "N", 45.0),
            ("Inter-Item Correlation Matrix", "i3", "i1", 0.6683354532),
            ("Inter-Item Correlation Matrix", "i3", "i2", 0.4222942514),
            ("Inter-Item Correlation Matrix", "i3", "i3", 1.0),
            ("Inter-Item Correlation Matrix", "i3", "i4", 0.2882977168),
            ("Inter-Item Correlation Matrix", "i3", "i5", 0.4758388707),
            ("Inter-Item Correlation Matrix", "i3", "i6", 0.5524653093),
            ("Inter-Item Covariance Matrix", "i3", "i1", 0.8057020202),
            ("Inter-Item Covariance Matrix", "i3", "i2", 0.6013181818),
            ("Inter-Item Covariance Matrix", "i3", "i3", 1.384616162),
            ("Inter-Item Covariance Matrix", "i3", "i4", 0.4002373737),
            ("Inter-Item Covariance Matrix", "i3", "i5", 0.6751414141),
            ("Inter-Item Covariance Matrix", "i3", "i6", 0.7732777778),
            (
                "Item-Total Statistics",
                "i3",
                "Scale Mean if Item Deleted",
                14.49111111,
            ),
            (
                "Item-Total Statistics",
                "i3",
                "Scale Variance if Item Deleted",
                16.04310101,
            ),
            (
                "Item-Total Statistics",
                "i3",
                "Corrected Item-Total Correlation",
                0.6907683383,
            ),
            (
                "Item-Total Statistics",
                "i3",
                "Squared Multiple Correlation",
                0.5654941526,
            ),
            (
                "Item-Total Statistics",
                "i3",
                "Cronbach's Alpha if Item Deleted",
                0.7221432228,
            ),
            ("Item Statistics", "i4", "Mean", 2.917777778),
            ("Item Statistics", "i4", "Std. Deviation", 1.179809093),
            ("Item Statistics", "i4", "N", 45.0),
            ("Inter-Item Correlation Matrix", "i4", "i1", 0.4951044687),
            ("Inter-Item Correlation Matrix", "i4", "i2", 0.3049092501),
            ("Inter-Item Correlation Matrix", "i4", "i3", 0.2882977168),
            ("Inter-Item Correlation Matrix", "i4", "i4", 1.0),
            ("Inter-Item Correlation Matrix", "i4", "i5", 0.08676336108),
            ("Inter-Item Correlation Matrix", "i4", "i6", 0.211256534),
            ("Inter-Item Covariance Matrix", "i4", "i1", 0.5984444444),
            ("Inter-Item Covariance Matrix", "i4", "i2", 0.4353181818),
            ("Inter-Item Covariance Matrix", "i4", "i3", 0.4002373737),
            ("Inter-Item Covariance Matrix", "i4", "i4", 1.391949495),
            ("Inter-Item Covariance Matrix", "i4", "i5", 0.1234292929),
            ("Inter-Item Covariance Matrix", "i4", "i6", 0.2964747475),
            (
                "Item-Total Statistics",
                "i4",
                "Scale Mean if Item Deleted",
                14.29777778,
            ),
            (
                "Item-Total Statistics",
                "i4",
                "Scale Variance if Item Deleted",
                18.83931313,
            ),
            (
                "Item-Total Statistics",
                "i4",
                "Corrected Item-Total Correlation",
                0.3620286824,
            ),
            (
                "Item-Total Statistics",
                "i4",
                "Squared Multiple Correlation",
                0.2784220904,
            ),
            (
                "Item-Total Statistics",
                "i4",
                "Cronbach's Alpha if Item Deleted",
                0.8009765738,
            ),
            ("Item Statistics", "i5", "Mean", 2.948888889),
            ("Item Statistics", "i5", "Std. Deviation", 1.205785716),
            ("Item Statistics", "i5", "N", 45.0),
            ("Inter-Item Correlation Matrix", "i5", "i1", 0.4351367104),
            ("Inter-Item Correlation Matrix", "i5", "i2", 0.3081844527),
            ("Inter-Item Correlation Matrix", "i5", "i3", 0.4758388707),
            ("Inter-Item Correlation Matrix", "i5", "i4", 0.08676336108),
            ("Inter-Item Correlation Matrix", "i5", "i5", 1.0),
            ("Inter-Item Correlation Matrix", "i5", "i6", 0.2885255088),
            ("Inter-Item Covariance Matrix", "i5", "i1", 0.537540404),
            ("Inter-Item Covariance Matrix", "i5", "i2", 0.4496818182),
            ("Inter-Item Covariance Matrix", "i5", "i3", 0.6751414141),
            ("Inter-Item Covariance Matrix", "i5", "i4", 0.1234292929),
            ("Inter-Item Covariance Matrix", "i5", "i5", 1.453919192),
            ("Inter-Item Covariance Matrix", "i5", "i6", 0.4138282828),
            (
                "Item-Total Statistics",
                "i5",
                "Scale Mean if Item Deleted",
                14.26666667,
            ),
            (
                "Item-Total Statistics",
                "i5",
                "Scale Variance if Item Deleted",
                18.08590909,
            ),
            (
                "Item-Total Statistics",
                "i5",
                "Corrected Item-Total Correlation",
                0.4289509092,
            ),
            (
                "Item-Total Statistics",
                "i5",
                "Squared Multiple Correlation",
                0.2778557327,
            ),
            (
                "Item-Total Statistics",
                "i5",
                "Cronbach's Alpha if Item Deleted",
                0.7865546312,
            ),
            ("Item Statistics", "i6", "Mean", 2.968888889),
            ("Item Statistics", "i6", "Std. Deviation", 1.189503759),
            ("Item Statistics", "i6", "N", 45.0),
            ("Inter-Item Correlation Matrix", "i6", "i1", 0.4190492113),
            ("Inter-Item Correlation Matrix", "i6", "i2", 0.4864302138),
            ("Inter-Item Correlation Matrix", "i6", "i3", 0.5524653093),
            ("Inter-Item Correlation Matrix", "i6", "i4", 0.211256534),
            ("Inter-Item Correlation Matrix", "i6", "i5", 0.2885255088),
            ("Inter-Item Correlation Matrix", "i6", "i6", 1.0),
            ("Inter-Item Covariance Matrix", "i6", "i1", 0.5106767677),
            ("Inter-Item Covariance Matrix", "i6", "i2", 0.7001818182),
            ("Inter-Item Covariance Matrix", "i6", "i3", 0.7732777778),
            ("Inter-Item Covariance Matrix", "i6", "i4", 0.2964747475),
            ("Inter-Item Covariance Matrix", "i6", "i5", 0.4138282828),
            ("Inter-Item Covariance Matrix", "i6", "i6", 1.414919192),
            (
                "Item-Total Statistics",
                "i6",
                "Scale Mean if Item Deleted",
                14.24666667,
            ),
            (
                "Item-Total Statistics",
                "i6",
                "Scale Variance if Item Deleted",
                17.13527273,
            ),
            (
                "Item-Total Statistics",
                "i6",
                "Corrected Item-Total Correlation",
                0.5472138618,
            ),
            (
                "Item-Total Statistics",
                "i6",
                "Squared Multiple Correlation",
                0.3833098555,
            ),
            (
                "Item-Total Statistics",
                "i6",
                "Cronbach's Alpha if Item Deleted",
                0.7579984461,
            ),
        ],
    );
}

#[test]
fn split_half_in_unequal_parts_agrees_with_r() {
    let t = run(
        "RELIABILITY VARIABLES=i1 TO i6 /STATISTICS=SCALE /MODEL=SPLIT(2) /SUMMARY=MEANS VARIANCE COV CORR.",
    );
    check(
        &t,
        &[
            (
                "Reliability Statistics",
                "Cronbach's Alpha / Part 1 / Value",
                "Value",
                0.7508233468,
            ),
            (
                "Reliability Statistics",
                "Cronbach's Alpha / Part 1 / N of Items",
                "Value",
                4.0,
            ),
            (
                "Reliability Statistics",
                "Cronbach's Alpha / Part 2 / Value",
                "Value",
                0.447806139,
            ),
            (
                "Reliability Statistics",
                "Cronbach's Alpha / Part 2 / N of Items",
                "Value",
                2.0,
            ),
            (
                "Reliability Statistics",
                "Cronbach's Alpha / Total N of Items",
                "Value",
                6.0,
            ),
            (
                "Reliability Statistics",
                "Correlation Between Forms",
                "Value",
                0.6077816222,
            ),
            (
                "Reliability Statistics",
                "Spearman-Brown Coefficient / Equal Length",
                "Value",
                0.7560499682,
            ),
            (
                "Reliability Statistics",
                "Spearman-Brown Coefficient / Unequal Length",
                "Value",
                0.7732039502,
            ),
            (
                "Reliability Statistics",
                "Guttman Split-Half Coefficient",
                "Value",
                0.6794589632,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "Mean",
                2.824444444,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "Minimum",
                2.724444444,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "Maximum",
                2.92,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "Range",
                0.1955555556,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "Maximum / Minimum",
                1.07177814,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "Variance",
                0.01191440329,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 1",
                "N of Items",
                4.0,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "Mean",
                1.322636364,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "Minimum",
                1.049616162,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "Maximum",
                1.464363636,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "Range",
                0.4147474747,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "Maximum / Minimum",
                1.395142043,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "Variance",
                0.03442415203,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 1",
                "N of Items",
                4.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "Mean",
                0.5682685185,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "Minimum",
                0.4002373737,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "Maximum",
                0.8057020202,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "Range",
                0.4054646465,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "Maximum / Minimum",
                2.013060431,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "Variance",
                0.01896143565,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 1",
                "N of Items",
                4.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "Mean",
                0.4395947904,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "Minimum",
                0.2882977168,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "Maximum",
                0.6683354532,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "Range",
                0.3800377363,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "Maximum / Minimum",
                2.318212785,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "Variance",
                0.01765384927,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 1",
                "N of Items",
                4.0,
            ),
            ("Scale Statistics", "Part 1", "Mean", 11.29777778),
            ("Scale Statistics", "Part 1", "Variance", 12.10976768),
            ("Scale Statistics", "Part 1", "Std. Deviation", 3.479909148),
            ("Scale Statistics", "Part 1", "N of Items", 4.0),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "Mean",
                2.958888889,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "Minimum",
                2.948888889,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "Maximum",
                2.968888889,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "Range",
                0.02,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "Maximum / Minimum",
                1.006782216,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "Variance",
                0.0002,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Part 2",
                "N of Items",
                2.0,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "Mean",
                1.434419192,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "Minimum",
                1.414919192,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "Maximum",
                1.453919192,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "Range",
                0.039,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "Maximum / Minimum",
                1.027563412,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "Variance",
                0.0007605,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Part 2",
                "N of Items",
                2.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "Mean",
                0.4138282828,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "Minimum",
                0.4138282828,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "Maximum",
                0.4138282828,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "Range",
                0.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "Maximum / Minimum",
                1.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "Variance",
                0.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Part 2",
                "N of Items",
                2.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "Mean",
                0.2885255088,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "Minimum",
                0.2885255088,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "Maximum",
                0.2885255088,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "Range",
                0.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "Maximum / Minimum",
                1.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "Variance",
                0.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Part 2",
                "N of Items",
                2.0,
            ),
            ("Scale Statistics", "Part 2", "Mean", 5.917777778),
            ("Scale Statistics", "Part 2", "Variance", 3.696494949),
            ("Scale Statistics", "Part 2", "Std. Deviation", 1.922627096),
            ("Scale Statistics", "Part 2", "N of Items", 2.0),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "Mean",
                2.869259259,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "Minimum",
                2.724444444,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "Maximum",
                2.968888889,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "Range",
                0.2444444444,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "Maximum / Minimum",
                1.089722675,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "Variance",
                0.01200872428,
            ),
            (
                "Summary Item Statistics",
                "Item Means / Both Parts",
                "N of Items",
                6.0,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "Mean",
                1.359897306,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "Minimum",
                1.049616162,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "Maximum",
                1.464363636,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "Range",
                0.4147474747,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "Maximum / Minimum",
                1.395142043,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "Variance",
                0.02413869807,
            ),
            (
                "Summary Item Statistics",
                "Item Variances / Both Parts",
                "N of Items",
                6.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "Mean",
                0.5259895623,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "Minimum",
                0.1234292929,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "Maximum",
                0.8057020202,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "Range",
                0.6822727273,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "Maximum / Minimum",
                6.527640247,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "Variance",
                0.03187975063,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Covariances / Both Parts",
                "N of Items",
                6.0,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "Mean",
                0.3934145943,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "Minimum",
                0.08676336108,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "Maximum",
                0.6683354532,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "Range",
                0.5815720921,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "Maximum / Minimum",
                7.702968682,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "Variance",
                0.02058723686,
            ),
            (
                "Summary Item Statistics",
                "Inter-Item Correlations / Both Parts",
                "N of Items",
                6.0,
            ),
            ("Scale Statistics", "Both Parts", "Mean", 17.21555556),
            ("Scale Statistics", "Both Parts", "Variance", 23.93907071),
            (
                "Scale Statistics",
                "Both Parts",
                "Std. Deviation",
                4.892756964,
            ),
            ("Scale Statistics", "Both Parts", "N of Items", 6.0),
        ],
    );
}
