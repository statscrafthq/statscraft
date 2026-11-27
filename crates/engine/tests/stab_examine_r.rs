//! Numerical stability of EXAMINE against R on the 24 cases of stab_basic_r (`big` is 1000000000
//! plus a spread of about 3, `tiny` is the same spread times 1e-9): the descriptives, the
//! percentiles and the Shapiro-Wilk test to nine digits, the Lilliefors distance to six (it uses
//! the mean as a double, which at 1e9 carries an error of 1e-7). R works on the values shifted
//! by their first one. The M-estimators are not compared: the manual stops the iteration at a
//! change of 0.005 of the estimate, which at an offset of 1e9 is far above the spread, so they
//! are not translation invariant, as specified. Scripts in lab/oracles/stab-ex.

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

/// The row headers (the columns before the first one titled Statistic, Case Number or with a
/// slash) joined with " / ", the empty ones left out.
fn row_keys(table: &Table) -> Vec<String> {
    let titles = table.column_titles();
    let count = titles
        .iter()
        .position(|t| t.contains(" / ") || t == "Statistic" || t == "Case Number")
        .unwrap_or(1);
    table
        .rows
        .iter()
        .map(|row| {
            row[..count]
                .iter()
                .map(|c| match c {
                    Cell::Text(t) => t.clone(),
                    Cell::Number(n) => format!("{n}"),
                    Cell::Empty => String::new(),
                })
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .collect()
}

/// Each expectation: the table title, the row, the column and the value (NaN: an empty cell).
/// Where several tables share a title (one per group), the first that has the row is used.
fn check(tables: &[Table], expected: &[(&str, &str, &str, f64)], rel: f64) {
    for (title, row, column, want) in expected {
        let (t, r) = tables
            .iter()
            .filter(|t| t.title == *title)
            .find_map(|t| row_keys(t).iter().position(|k| k == row).map(|r| (t, r)))
            .unwrap_or_else(|| panic!("no row {row} in a table {title}"));
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
            (n - want).abs() <= rel * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn examine_of_values_far_from_zero_and_very_small() {
    let t = run(
        "EXAMINE VARIABLES=big tiny /PLOT=NPPLOT /STATISTICS=DESCRIPTIVES /PERCENTILES(5,25,50,75,95).",
    );
    check(
        &t,
        &[
            ("Descriptives", "big / Mean", "Statistic", 1000000004.94792),
            ("Descriptives", "big / Mean", "Std. Error", 0.67716694021154),
            (
                "Descriptives",
                "big / 5% Trimmed Mean",
                "Statistic",
                1000000004.90972,
            ),
            ("Descriptives", "big / Median", "Statistic", 1000000005.0),
            (
                "Descriptives",
                "big / Variance",
                "Statistic",
                11.005321557971,
            ),
            (
                "Descriptives",
                "big / Std. Deviation",
                "Statistic",
                3.31742694840007,
            ),
            ("Descriptives", "big / Range", "Statistic", 9.25),
            (
                "Descriptives",
                "big / Interquartile Range",
                "Statistic",
                6.875,
            ),
            (
                "Descriptives",
                "big / Skewness",
                "Statistic",
                0.0388614164922998,
            ),
            (
                "Descriptives",
                "big / Kurtosis",
                "Statistic",
                -1.66606919165558,
            ),
            (
                "Percentiles",
                "big / Weighted Average at (W+1)p",
                "Percentiles / 5",
                1000000000.75,
            ),
            (
                "Percentiles",
                "big / Weighted Average at (W+1)p",
                "Percentiles / 25",
                1000000001.5,
            ),
            (
                "Percentiles",
                "big / Weighted Average at (W+1)p",
                "Percentiles / 50",
                1000000005.0,
            ),
            (
                "Percentiles",
                "big / Weighted Average at (W+1)p",
                "Percentiles / 75",
                1000000008.375,
            ),
            (
                "Percentiles",
                "big / Weighted Average at (W+1)p",
                "Percentiles / 95",
                1000000009.75,
            ),
            (
                "Tests of Normality",
                "big",
                "Shapiro-Wilk / Statistic",
                0.880596326383405,
            ),
            (
                "Tests of Normality",
                "big",
                "Shapiro-Wilk / Sig.",
                0.00854315266919227,
            ),
            (
                "Descriptives",
                "tiny / Mean",
                "Statistic",
                5.94791666666667e-09,
            ),
            (
                "Descriptives",
                "tiny / Mean",
                "Std. Error",
                6.7716694021154e-10,
            ),
            (
                "Descriptives",
                "tiny / 5% Trimmed Mean",
                "Statistic",
                5.90972222222222e-09,
            ),
            ("Descriptives", "tiny / Median", "Statistic", 6e-09),
            (
                "Descriptives",
                "tiny / Variance",
                "Statistic",
                1.1005321557971e-17,
            ),
            (
                "Descriptives",
                "tiny / Std. Deviation",
                "Statistic",
                3.31742694840007e-09,
            ),
            ("Descriptives", "tiny / Range", "Statistic", 9.25e-09),
            (
                "Descriptives",
                "tiny / Interquartile Range",
                "Statistic",
                6.875e-09,
            ),
            (
                "Descriptives",
                "tiny / Skewness",
                "Statistic",
                0.0388614164922999,
            ),
            (
                "Descriptives",
                "tiny / Kurtosis",
                "Statistic",
                -1.66606919165558,
            ),
            (
                "Percentiles",
                "tiny / Weighted Average at (W+1)p",
                "Percentiles / 5",
                1.75e-09,
            ),
            (
                "Percentiles",
                "tiny / Weighted Average at (W+1)p",
                "Percentiles / 25",
                2.5e-09,
            ),
            (
                "Percentiles",
                "tiny / Weighted Average at (W+1)p",
                "Percentiles / 50",
                6e-09,
            ),
            (
                "Percentiles",
                "tiny / Weighted Average at (W+1)p",
                "Percentiles / 75",
                9.375e-09,
            ),
            (
                "Percentiles",
                "tiny / Weighted Average at (W+1)p",
                "Percentiles / 95",
                1.075e-08,
            ),
            (
                "Tests of Normality",
                "tiny",
                "Shapiro-Wilk / Statistic",
                0.880596326383405,
            ),
            (
                "Tests of Normality",
                "tiny",
                "Shapiro-Wilk / Sig.",
                0.00854315266919206,
            ),
        ],
        1e-9,
    );
}

#[test]
fn examine_lilliefors_distance_far_from_zero_and_very_small() {
    let t = run("EXAMINE VARIABLES=big tiny /PLOT=NPPLOT /STATISTICS=NONE.");
    check(
        &t,
        &[
            (
                "Tests of Normality",
                "big",
                "Kolmogorov-Smirnov / Statistic",
                0.162852963239965,
            ),
            (
                "Tests of Normality",
                "tiny",
                "Kolmogorov-Smirnov / Statistic",
                0.162852963239965,
            ),
        ],
        1e-6,
    );
}
