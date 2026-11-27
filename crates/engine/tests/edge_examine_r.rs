//! Edge cases of EXAMINE against R on nine invented cases: a constant variable, the first two and
//! three cases, groups of three, two and one case, a zero weight and a weight of two. The
//! descriptives (the 5 per cent trimmed mean with the fractional end weights of the algorithm,
//! the bias-adjusted skewness and kurtosis, empty where the manual's conditions fail), the
//! percentiles at (W + 1) p, Tukey's hinges (`fivenum`), the Shapiro-Wilk test (`shapiro.test`,
//! none below three cases or without spread) and the Lilliefors distance written out in R. Scripts
//! in lab/oracles/edge-ex.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /x c g w (4F8.2).
BEGIN DATA.
3 5 1 1
7 5 1 2
4 5 1 1
9 5 2 0
2 5 2 1
8 5 3 1
. 5 4 1
6 5 4 1
. 5 5 1
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
fn check(tables: &[Table], expected: &[(&str, &str, &str, f64)]) {
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
            (n - want).abs() <= 1e-8 + 1e-7 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn constant_variable() {
    let t = run(
        "EXAMINE VARIABLES=c /PLOT=NPPLOT /STATISTICS=DESCRIPTIVES /PERCENTILES(5,25,50,75,95).",
    );
    check(
        &t,
        &[
            ("Descriptives", "c / Mean", "Statistic", 5.0),
            ("Descriptives", "c / Mean", "Std. Error", 0.0),
            ("Descriptives", "c / 5% Trimmed Mean", "Statistic", 5.0),
            ("Descriptives", "c / Median", "Statistic", 5.0),
            ("Descriptives", "c / Variance", "Statistic", 0.0),
            ("Descriptives", "c / Std. Deviation", "Statistic", 0.0),
            ("Descriptives", "c / Minimum", "Statistic", 5.0),
            ("Descriptives", "c / Maximum", "Statistic", 5.0),
            ("Descriptives", "c / Range", "Statistic", 0.0),
            ("Descriptives", "c / Interquartile Range", "Statistic", 0.0),
            ("Descriptives", "c / Skewness", "Statistic", f64::NAN),
            ("Descriptives", "c / Skewness", "Std. Error", f64::NAN),
            ("Descriptives", "c / Kurtosis", "Statistic", f64::NAN),
            ("Descriptives", "c / Kurtosis", "Std. Error", f64::NAN),
            (
                "Percentiles",
                "c / Weighted Average at (W+1)p",
                "Percentiles / 5",
                5.0,
            ),
            (
                "Percentiles",
                "c / Tukey's Hinges",
                "Percentiles / 5",
                f64::NAN,
            ),
            (
                "Percentiles",
                "c / Weighted Average at (W+1)p",
                "Percentiles / 25",
                5.0,
            ),
            ("Percentiles", "c / Tukey's Hinges", "Percentiles / 25", 5.0),
            (
                "Percentiles",
                "c / Weighted Average at (W+1)p",
                "Percentiles / 50",
                5.0,
            ),
            ("Percentiles", "c / Tukey's Hinges", "Percentiles / 50", 5.0),
            (
                "Percentiles",
                "c / Weighted Average at (W+1)p",
                "Percentiles / 75",
                5.0,
            ),
            ("Percentiles", "c / Tukey's Hinges", "Percentiles / 75", 5.0),
            (
                "Percentiles",
                "c / Weighted Average at (W+1)p",
                "Percentiles / 95",
                5.0,
            ),
            (
                "Percentiles",
                "c / Tukey's Hinges",
                "Percentiles / 95",
                f64::NAN,
            ),
            (
                "Tests of Normality",
                "c",
                "Kolmogorov-Smirnov / Statistic",
                f64::NAN,
            ),
            ("Tests of Normality", "c", "Kolmogorov-Smirnov / df", 9.0),
            (
                "Tests of Normality",
                "c",
                "Shapiro-Wilk / Statistic",
                f64::NAN,
            ),
            ("Tests of Normality", "c", "Shapiro-Wilk / df", 9.0),
            ("Tests of Normality", "c", "Shapiro-Wilk / Sig.", f64::NAN),
        ],
    );
}

#[test]
fn two_cases() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 2).\nEXAMINE VARIABLES=x /PLOT=NPPLOT /STATISTICS=DESCRIPTIVES /PERCENTILES(5,25,50,75,95).",
    );
    check(
        &t,
        &[
            ("Descriptives", "x / Mean", "Statistic", 5.0),
            ("Descriptives", "x / Mean", "Std. Error", 2.0),
            ("Descriptives", "x / 5% Trimmed Mean", "Statistic", 5.0),
            ("Descriptives", "x / Median", "Statistic", 5.0),
            ("Descriptives", "x / Variance", "Statistic", 8.0),
            (
                "Descriptives",
                "x / Std. Deviation",
                "Statistic",
                2.82842712475,
            ),
            ("Descriptives", "x / Minimum", "Statistic", 3.0),
            ("Descriptives", "x / Maximum", "Statistic", 7.0),
            ("Descriptives", "x / Range", "Statistic", 4.0),
            ("Descriptives", "x / Interquartile Range", "Statistic", 4.0),
            ("Descriptives", "x / Skewness", "Statistic", f64::NAN),
            ("Descriptives", "x / Skewness", "Std. Error", f64::NAN),
            ("Descriptives", "x / Kurtosis", "Statistic", f64::NAN),
            ("Descriptives", "x / Kurtosis", "Std. Error", f64::NAN),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 5",
                3.0,
            ),
            (
                "Percentiles",
                "x / Tukey's Hinges",
                "Percentiles / 5",
                f64::NAN,
            ),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 25",
                3.0,
            ),
            ("Percentiles", "x / Tukey's Hinges", "Percentiles / 25", 3.0),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 50",
                5.0,
            ),
            ("Percentiles", "x / Tukey's Hinges", "Percentiles / 50", 5.0),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 75",
                7.0,
            ),
            ("Percentiles", "x / Tukey's Hinges", "Percentiles / 75", 7.0),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 95",
                7.0,
            ),
            (
                "Percentiles",
                "x / Tukey's Hinges",
                "Percentiles / 95",
                f64::NAN,
            ),
            (
                "Tests of Normality",
                "x",
                "Kolmogorov-Smirnov / Statistic",
                0.260249938907,
            ),
            ("Tests of Normality", "x", "Kolmogorov-Smirnov / df", 2.0),
            (
                "Tests of Normality",
                "x",
                "Shapiro-Wilk / Statistic",
                f64::NAN,
            ),
            ("Tests of Normality", "x", "Shapiro-Wilk / df", 2.0),
            ("Tests of Normality", "x", "Shapiro-Wilk / Sig.", f64::NAN),
        ],
    );
}

#[test]
fn three_cases() {
    let t = run(
        "COMPUTE k=$CASENUM.\nTEMPORARY.\nSELECT IF (k <= 3).\nEXAMINE VARIABLES=x /PLOT=NPPLOT /STATISTICS=DESCRIPTIVES /PERCENTILES(5,25,50,75,95).",
    );
    check(
        &t,
        &[
            ("Descriptives", "x / Mean", "Statistic", 4.66666666667),
            ("Descriptives", "x / Mean", "Std. Error", 1.20185042515),
            (
                "Descriptives",
                "x / 5% Trimmed Mean",
                "Statistic",
                4.62962962963,
            ),
            ("Descriptives", "x / Median", "Statistic", 4.0),
            ("Descriptives", "x / Variance", "Statistic", 4.33333333333),
            (
                "Descriptives",
                "x / Std. Deviation",
                "Statistic",
                2.08166599947,
            ),
            ("Descriptives", "x / Minimum", "Statistic", 3.0),
            ("Descriptives", "x / Maximum", "Statistic", 7.0),
            ("Descriptives", "x / Range", "Statistic", 4.0),
            ("Descriptives", "x / Interquartile Range", "Statistic", 4.0),
            ("Descriptives", "x / Skewness", "Statistic", 1.29334278073),
            ("Descriptives", "x / Skewness", "Std. Error", 1.22474487139),
            ("Descriptives", "x / Kurtosis", "Statistic", f64::NAN),
            ("Descriptives", "x / Kurtosis", "Std. Error", f64::NAN),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 5",
                3.0,
            ),
            (
                "Percentiles",
                "x / Tukey's Hinges",
                "Percentiles / 5",
                f64::NAN,
            ),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 25",
                3.0,
            ),
            ("Percentiles", "x / Tukey's Hinges", "Percentiles / 25", 3.5),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 50",
                4.0,
            ),
            ("Percentiles", "x / Tukey's Hinges", "Percentiles / 50", 4.0),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 75",
                7.0,
            ),
            ("Percentiles", "x / Tukey's Hinges", "Percentiles / 75", 5.5),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 95",
                7.0,
            ),
            (
                "Percentiles",
                "x / Tukey's Hinges",
                "Percentiles / 95",
                f64::NAN,
            ),
            (
                "Tests of Normality",
                "x",
                "Kolmogorov-Smirnov / Statistic",
                0.292279645813,
            ),
            ("Tests of Normality", "x", "Kolmogorov-Smirnov / df", 3.0),
            (
                "Tests of Normality",
                "x",
                "Shapiro-Wilk / Statistic",
                0.923076923077,
            ),
            ("Tests of Normality", "x", "Shapiro-Wilk / df", 3.0),
            (
                "Tests of Normality",
                "x",
                "Shapiro-Wilk / Sig.",
                0.463262874934,
            ),
        ],
    );
}

#[test]
fn groups_of_three_two_and_one_case() {
    let t =
        run("EXAMINE VARIABLES=x BY g /PLOT=NONE /STATISTICS=DESCRIPTIVES /PERCENTILES(25,50).");
    check(
        &t,
        &[
            ("Descriptives", "x / 1 / Mean", "Statistic", 4.66666666667),
            ("Descriptives", "x / 1 / Mean", "Std. Error", 1.20185042515),
            (
                "Descriptives",
                "x / 1 / 5% Trimmed Mean",
                "Statistic",
                4.62962962963,
            ),
            ("Descriptives", "x / 1 / Median", "Statistic", 4.0),
            (
                "Descriptives",
                "x / 1 / Variance",
                "Statistic",
                4.33333333333,
            ),
            (
                "Descriptives",
                "x / 1 / Std. Deviation",
                "Statistic",
                2.08166599947,
            ),
            ("Descriptives", "x / 1 / Minimum", "Statistic", 3.0),
            ("Descriptives", "x / 1 / Maximum", "Statistic", 7.0),
            ("Descriptives", "x / 1 / Range", "Statistic", 4.0),
            (
                "Descriptives",
                "x / 1 / Interquartile Range",
                "Statistic",
                4.0,
            ),
            (
                "Descriptives",
                "x / 1 / Skewness",
                "Statistic",
                1.29334278073,
            ),
            (
                "Descriptives",
                "x / 1 / Skewness",
                "Std. Error",
                1.22474487139,
            ),
            ("Descriptives", "x / 1 / Kurtosis", "Statistic", f64::NAN),
            ("Descriptives", "x / 1 / Kurtosis", "Std. Error", f64::NAN),
            (
                "Percentiles",
                "x / 1 / Weighted Average at (W+1)p",
                "Percentiles / 25",
                3.0,
            ),
            (
                "Percentiles",
                "x / 1 / Tukey's Hinges",
                "Percentiles / 25",
                3.5,
            ),
            (
                "Percentiles",
                "x / 1 / Weighted Average at (W+1)p",
                "Percentiles / 50",
                4.0,
            ),
            (
                "Percentiles",
                "x / 1 / Tukey's Hinges",
                "Percentiles / 50",
                4.0,
            ),
            ("Descriptives", "x / 2 / Mean", "Statistic", 5.5),
            ("Descriptives", "x / 2 / Mean", "Std. Error", 3.5),
            ("Descriptives", "x / 2 / 5% Trimmed Mean", "Statistic", 5.5),
            ("Descriptives", "x / 2 / Median", "Statistic", 5.5),
            ("Descriptives", "x / 2 / Variance", "Statistic", 24.5),
            (
                "Descriptives",
                "x / 2 / Std. Deviation",
                "Statistic",
                4.94974746831,
            ),
            ("Descriptives", "x / 2 / Minimum", "Statistic", 2.0),
            ("Descriptives", "x / 2 / Maximum", "Statistic", 9.0),
            ("Descriptives", "x / 2 / Range", "Statistic", 7.0),
            (
                "Descriptives",
                "x / 2 / Interquartile Range",
                "Statistic",
                7.0,
            ),
            ("Descriptives", "x / 2 / Skewness", "Statistic", f64::NAN),
            ("Descriptives", "x / 2 / Skewness", "Std. Error", f64::NAN),
            ("Descriptives", "x / 2 / Kurtosis", "Statistic", f64::NAN),
            ("Descriptives", "x / 2 / Kurtosis", "Std. Error", f64::NAN),
            (
                "Percentiles",
                "x / 2 / Weighted Average at (W+1)p",
                "Percentiles / 25",
                2.0,
            ),
            (
                "Percentiles",
                "x / 2 / Tukey's Hinges",
                "Percentiles / 25",
                2.0,
            ),
            (
                "Percentiles",
                "x / 2 / Weighted Average at (W+1)p",
                "Percentiles / 50",
                5.5,
            ),
            (
                "Percentiles",
                "x / 2 / Tukey's Hinges",
                "Percentiles / 50",
                5.5,
            ),
            ("Descriptives", "x / 3 / Mean", "Statistic", 8.0),
            ("Descriptives", "x / 3 / Mean", "Std. Error", f64::NAN),
            ("Descriptives", "x / 3 / 5% Trimmed Mean", "Statistic", 8.0),
            ("Descriptives", "x / 3 / Median", "Statistic", 8.0),
            ("Descriptives", "x / 3 / Variance", "Statistic", f64::NAN),
            (
                "Descriptives",
                "x / 3 / Std. Deviation",
                "Statistic",
                f64::NAN,
            ),
            ("Descriptives", "x / 3 / Minimum", "Statistic", 8.0),
            ("Descriptives", "x / 3 / Maximum", "Statistic", 8.0),
            ("Descriptives", "x / 3 / Range", "Statistic", 0.0),
            (
                "Descriptives",
                "x / 3 / Interquartile Range",
                "Statistic",
                0.0,
            ),
            ("Descriptives", "x / 3 / Skewness", "Statistic", f64::NAN),
            ("Descriptives", "x / 3 / Skewness", "Std. Error", f64::NAN),
            ("Descriptives", "x / 3 / Kurtosis", "Statistic", f64::NAN),
            ("Descriptives", "x / 3 / Kurtosis", "Std. Error", f64::NAN),
            (
                "Percentiles",
                "x / 3 / Weighted Average at (W+1)p",
                "Percentiles / 25",
                8.0,
            ),
            (
                "Percentiles",
                "x / 3 / Tukey's Hinges",
                "Percentiles / 25",
                8.0,
            ),
            (
                "Percentiles",
                "x / 3 / Weighted Average at (W+1)p",
                "Percentiles / 50",
                8.0,
            ),
            (
                "Percentiles",
                "x / 3 / Tukey's Hinges",
                "Percentiles / 50",
                8.0,
            ),
            ("Descriptives", "x / 4 / Mean", "Statistic", 6.0),
            ("Descriptives", "x / 4 / Mean", "Std. Error", f64::NAN),
            ("Descriptives", "x / 4 / 5% Trimmed Mean", "Statistic", 6.0),
            ("Descriptives", "x / 4 / Median", "Statistic", 6.0),
            ("Descriptives", "x / 4 / Variance", "Statistic", f64::NAN),
            (
                "Descriptives",
                "x / 4 / Std. Deviation",
                "Statistic",
                f64::NAN,
            ),
            ("Descriptives", "x / 4 / Minimum", "Statistic", 6.0),
            ("Descriptives", "x / 4 / Maximum", "Statistic", 6.0),
            ("Descriptives", "x / 4 / Range", "Statistic", 0.0),
            (
                "Descriptives",
                "x / 4 / Interquartile Range",
                "Statistic",
                0.0,
            ),
            ("Descriptives", "x / 4 / Skewness", "Statistic", f64::NAN),
            ("Descriptives", "x / 4 / Skewness", "Std. Error", f64::NAN),
            ("Descriptives", "x / 4 / Kurtosis", "Statistic", f64::NAN),
            ("Descriptives", "x / 4 / Kurtosis", "Std. Error", f64::NAN),
            (
                "Percentiles",
                "x / 4 / Weighted Average at (W+1)p",
                "Percentiles / 25",
                6.0,
            ),
            (
                "Percentiles",
                "x / 4 / Tukey's Hinges",
                "Percentiles / 25",
                6.0,
            ),
            (
                "Percentiles",
                "x / 4 / Weighted Average at (W+1)p",
                "Percentiles / 50",
                6.0,
            ),
            (
                "Percentiles",
                "x / 4 / Tukey's Hinges",
                "Percentiles / 50",
                6.0,
            ),
        ],
    );
}

#[test]
fn zero_weight_and_weight_of_two() {
    let t = run(
        "WEIGHT BY w.\nEXAMINE VARIABLES=x /PLOT=NPPLOT /STATISTICS=DESCRIPTIVES /PERCENTILES(5,25,50,75,95).",
    );
    check(
        &t,
        &[
            ("Descriptives", "x / Mean", "Statistic", 5.28571428571),
            ("Descriptives", "x / Mean", "Std. Error", 0.865042958314),
            (
                "Descriptives",
                "x / 5% Trimmed Mean",
                "Statistic",
                5.31746031746,
            ),
            ("Descriptives", "x / Median", "Statistic", 6.0),
            ("Descriptives", "x / Variance", "Statistic", 5.2380952381),
            (
                "Descriptives",
                "x / Std. Deviation",
                "Statistic",
                2.28868854109,
            ),
            ("Descriptives", "x / Minimum", "Statistic", 2.0),
            ("Descriptives", "x / Maximum", "Statistic", 8.0),
            ("Descriptives", "x / Range", "Statistic", 6.0),
            ("Descriptives", "x / Interquartile Range", "Statistic", 4.0),
            ("Descriptives", "x / Skewness", "Statistic", -0.371788941848),
            ("Descriptives", "x / Skewness", "Std. Error", 0.793725393319),
            ("Descriptives", "x / Kurtosis", "Statistic", -1.68555371901),
            ("Descriptives", "x / Kurtosis", "Std. Error", 1.58745078664),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 5",
                2.0,
            ),
            (
                "Percentiles",
                "x / Tukey's Hinges",
                "Percentiles / 5",
                f64::NAN,
            ),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 25",
                3.0,
            ),
            ("Percentiles", "x / Tukey's Hinges", "Percentiles / 25", 3.5),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 50",
                6.0,
            ),
            ("Percentiles", "x / Tukey's Hinges", "Percentiles / 50", 6.0),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 75",
                7.0,
            ),
            ("Percentiles", "x / Tukey's Hinges", "Percentiles / 75", 7.0),
            (
                "Percentiles",
                "x / Weighted Average at (W+1)p",
                "Percentiles / 95",
                8.0,
            ),
            (
                "Percentiles",
                "x / Tukey's Hinges",
                "Percentiles / 95",
                f64::NAN,
            ),
            (
                "Tests of Normality",
                "x",
                "Kolmogorov-Smirnov / Statistic",
                0.201650462546,
            ),
            ("Tests of Normality", "x", "Kolmogorov-Smirnov / df", 7.0),
            (
                "Tests of Normality",
                "x",
                "Shapiro-Wilk / Statistic",
                0.918940540192,
            ),
            ("Tests of Normality", "x", "Shapiro-Wilk / df", 7.0),
            (
                "Tests of Normality",
                "x",
                "Shapiro-Wilk / Sig.",
                0.461206545896,
            ),
        ],
    );
}
