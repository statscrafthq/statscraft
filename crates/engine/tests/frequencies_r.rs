//! FREQUENCIES and DESCRIPTIVES statistics against R on 36 invented values with a tie, one
//! missing value and case weights: N, mean, standard error, median, mode, standard deviation,
//! variance, the bias-adjusted skewness and kurtosis with their standard errors, range,
//! minimum, maximum, sum, the percentiles and ntiles (R's quantile type 6, the weighted
//! average at (W + 1) p), the same statistics for frequency weights (R on the cases repeated
//! by their weights), the weighted frequency table, and the z scores DESCRIPTIVES saves.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /x w g (3F8.1).
BEGIN DATA
15.5 1 1
19.8 2 1
51.7 3 1
14.2 1 3
20.2 1 3
20.2 2 3
18.1 2 2
29.0 1 1
. 2 3
17.4 1 3
10.9 1 2
12.3 2 3
14.4 1 3
24.7 1 2
29.1 2 2
12.1 1 3
21.4 2 1
21.4 1 1
34.2 1 2
18.7 1 3
24.1 2 3
11.1 2 3
21.4 2 1
23.2 2 2
16.1 2 1
19.5 1 3
13.8 1 1
13.0 1 2
12.3 1 1
25.4 2 2
35.8 1 3
22.7 1 3
47.3 1 3
17.4 1 3
9.4 1 3
29.0 2 2
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

fn near(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-9 * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

/// The value of the statistics table's row `label` (and `sub`, for the percentiles).
fn statistic(tables: &[Table], nth: usize, label: &str, sub: Option<&str>) -> f64 {
    let table = tables
        .iter()
        .filter(|t| t.title == "Statistics")
        .nth(nth)
        .expect("a statistics table");
    let text = |c: &Cell| match c {
        Cell::Text(s) => Some(s.clone()),
        _ => None,
    };
    let row = table
        .rows
        .iter()
        .find(|r| {
            text(&r[0]).as_deref() == Some(label)
                && sub.is_none_or(|s| text(&r[1]).as_deref() == Some(s))
        })
        .unwrap_or_else(|| panic!("no row {label} {sub:?}"));
    match row[row.len() - 1] {
        Cell::Number(n) => n,
        ref other => panic!("{label}: {other:?}"),
    }
}

const STATISTICS: [&str; 15] = [
    "Valid",
    "Mean",
    "Std. Error of Mean",
    "Median",
    "Mode",
    "Std. Deviation",
    "Variance",
    "Skewness",
    "Std. Error of Skewness",
    "Kurtosis",
    "Std. Error of Kurtosis",
    "Range",
    "Minimum",
    "Maximum",
    "Sum",
];
const PERCENTILES: [&str; 11] = [
    "5", "10", "20", "25", "40", "50", "60", "75", "80", "90", "95",
];

fn check(tables: &[Table], nth: usize, statistics: [f64; 15], percentiles: [f64; 11]) {
    for (label, want) in STATISTICS.iter().zip(statistics) {
        let sub = (*label == "Valid").then_some("Valid");
        let name = if *label == "Valid" { "N" } else { label };
        near(statistic(tables, nth, name, sub), want, label);
    }
    for (p, want) in PERCENTILES.iter().zip(percentiles) {
        near(statistic(tables, nth, "Percentiles", Some(p)), want, p);
    }
}

const COMMAND: &str = "FREQUENCIES x /FORMAT=NOTABLE /STATISTICS=ALL /PERCENTILES=5 10 25 50 75 90 95 \
                       /NTILES=5.";

#[test]
fn the_statistics_of_unweighted_cases_agree_with_r() {
    let t = run(COMMAND);
    check(
        &t,
        0,
        [
            35.0,
            21.337142857143,
            1.623778587973,
            19.8,
            21.4,
            9.60640367654,
            92.282991596639,
            1.539080677418,
            0.397694044332,
            2.75589556509,
            0.777794391104,
            42.3,
            9.4,
            51.7,
            746.8,
        ],
        [
            10.6, 11.7, 13.16, 14.2, 17.68, 19.8, 21.4, 24.7, 28.28, 34.84, 48.18,
        ],
    );
}

#[test]
fn the_statistics_of_weighted_cases_agree_with_r_on_the_repeated_cases() {
    let t = run(&format!("WEIGHT BY w.\n{COMMAND}"));
    check(
        &t,
        0,
        [
            50.0,
            22.428,
            1.470327021795,
            20.2,
            21.4,
            10.39678207673,
            108.093077551020,
            1.538702503097,
            0.336600708549,
            2.332518672017,
            0.661908374514,
            42.3,
            9.4,
            51.7,
            1121.4,
        ],
        [
            11.01, 12.12, 13.88, 15.225, 19.02, 20.2, 21.4, 25.4, 29.0, 35.64, 51.7,
        ],
    );
}

#[test]
fn descriptives_gives_the_same_statistics_and_saves_z_scores() {
    let t = run("DESCRIPTIVES x /STATISTICS=ALL /SAVE.");
    let table = t
        .iter()
        .find(|t| t.title == "Descriptive Statistics")
        .expect("the table");
    let want = [
        ("N", 35.0),
        ("Mean", 21.337142857143),
        ("S.E. Mean", 1.623778587973),
        ("Std Dev", 9.60640367654),
        ("Variance", 92.282991596639),
        ("Kurtosis", 2.75589556509),
        ("S.E. Kurt", 0.777794391104),
        ("Skewness", 1.539080677418),
        ("S.E. Skew", 0.397694044332),
        ("Range", 42.3),
        ("Minimum", 9.4),
        ("Maximum", 51.7),
        ("Sum", 746.8),
    ];
    for (column, value) in want {
        let at = table
            .column_titles()
            .iter()
            .position(|c| c == column)
            .unwrap_or_else(|| panic!("no column {column}"));
        match table.rows[0][at] {
            Cell::Number(n) => near(n, value, column),
            ref other => panic!("{column}: {other:?}"),
        }
    }
    // the weights count in DESCRIPTIVES too
    let t = run("WEIGHT BY w.\nDESCRIPTIVES x /STATISTICS=MEAN STDDEV SKEWNESS KURTOSIS.");
    let table = t
        .iter()
        .find(|t| t.title == "Descriptive Statistics")
        .expect("the table");
    for (column, value) in [
        ("N", 50.0),
        ("Mean", 22.428),
        ("Std Dev", 10.39678207673),
        ("Skewness", 1.538702503097),
        ("Kurtosis", 2.332518672017),
    ] {
        let at = table
            .column_titles()
            .iter()
            .position(|c| c == column)
            .unwrap_or_else(|| panic!("no column {column}"));
        match table.rows[0][at] {
            Cell::Number(n) => near(n, value, column),
            ref other => panic!("{column}: {other:?}"),
        }
    }
}

#[test]
fn the_weighted_frequency_table_counts_the_weights() {
    let t = run("WEIGHT BY w.\nFREQUENCIES g.");
    let table = t.iter().find(|t| t.title == "g").expect("the table");
    let number = |row: usize, column: usize| match table.rows[row][column] {
        Cell::Number(n) => n,
        ref other => panic!("{other:?}"),
    };
    // 16, 14 and 22 weighted cases of the three groups, 52 in all
    for (row, count, cumulative) in [(0, 16.0, 16.0), (1, 14.0, 30.0), (2, 22.0, 52.0)] {
        near(number(row, 3), count, "frequency");
        near(number(row, 4), count / 52.0 * 100.0, "percent");
        near(
            number(row, 6),
            cumulative / 52.0 * 100.0,
            "cumulative percent",
        );
    }
    // the first values of x: 9.4 once, 10.9 once, 11.1 twice, with the missing case apart
    let t = run("WEIGHT BY w.\nFREQUENCIES x.");
    let table = t.iter().find(|t| t.title == "x").expect("the table");
    for (row, count, cumulative) in [(0, 1.0, 1.0), (1, 1.0, 2.0), (2, 2.0, 4.0), (3, 1.0, 5.0)] {
        match (
            &table.rows[row][3],
            &table.rows[row][5],
            &table.rows[row][6],
        ) {
            (Cell::Number(f), Cell::Number(valid), Cell::Number(c)) => {
                near(*f, count, "frequency");
                near(*valid, count / 50.0 * 100.0, "valid percent");
                near(*c, cumulative / 50.0 * 100.0, "cumulative percent");
            }
            other => panic!("{other:?}"),
        }
    }
}
