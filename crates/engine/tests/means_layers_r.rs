//! MEANS against R on 54 invented cases with missing values (system-missing in the dependent and in
//! the controls, user-missing 999 in the dependent and 9 in a control) and frequency weights:
//! a table with two nested controls (every group of the first control with each level of the
//! second and a Total row, and the Total group), the cell statistics of such a table (mean, N,
//! standard deviation, the percents of the total N and sum, and of the N and sum in the category of
//! the first control), the three ways of treating missing values (TABLE leaves out a case missing
//! on any variable of the table, user-missing included; INCLUDE keeps the user-missing values;
//! DEPENDENT keeps those of the controls only), table lists with their own case counts, frequency
//! weights (every cell statistic, the ANOVA and eta from the cases written out as often as their
//! weight), and the test of linearity on the unweighted cases. Scripts in lab/oracles/means-r2.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /a b w y1 y2 (5F8.2).
BEGIN DATA.
1 2 2 28.9 10.4
2 1 2 25.4 .
3 1 1 44.4 16.1
1 2 1 31.6 17.5
2 1 1 . 28.5
3 1 1 38.8 3.1
1 1 1 39.6 3.7
2 2 1 999 5.4
3 1 1 42.2 9.6
1 2 2 34.1 9.2
. 2 1 46.9 4
3 1 3 38.2 14.8
1 1 3 18 17.2
2 . 2 38.3 8.4
3 2 1 45 2.3
1 1 1 17.3 13.8
2 2 2 43.8 15.2
3 2 1 34.8 28.2
1 2 2 34.8 17.3
2 2 1 37.5 15.8
3 2 1 . 8.5
1 2 2 31.1 19.4
2 2 1 45.5 17.3
3 2 1 46.1 17.2
1 2 2 26.8 2.2
2 1 1 34.1 16.9
3 1 1 31.5 15.5
1 1 1 36 10.3
2 1 1 39.2 10.8
3 . 1 28.5 6
1 2 2 33.9 57.9
2 1 1 40.7 3.1
3 9 3 40.9 6.4
1 2 1 31.5 9.2
2 2 1 35.5 3.6
3 2 3 46.9 2.3
1 2 2 25.6 21.4
2 2 3 33.9 3.4
3 1 2 34.8 25.8
. 2 1 37.1 23.2
2 2 2 40.3 9.8
3 2 1 29.6 14.2
1 1 2 23 2.7
2 1 2 26.9 3.3
3 1 3 38.8 18.4
1 2 2 39.9 2.3
2 2 1 50.3 7.1
3 2 2 44.6 3.1
1 2 1 39.3 14.3
2 1 2 34.8 .
3 2 1 34.4 6.8
1 1 3 32.2 2.9
2 1 1 32.8 4.6
3 2 1 48.9 13.8
END DATA.
MISSING VALUES y1 (999) b (9).
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
                let shown = match cell {
                    Cell::Text(t) if !t.is_empty() => Some(t.clone()),
                    Cell::Number(n) => Some(format!("{n}")),
                    _ => None,
                };
                if let Some(t) = shown {
                    carried[level] = t;
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

fn cell<'t>(tables: &'t [Table], title: &str, nth: usize, row: &str, column: &str) -> &'t Cell {
    let table = tables
        .iter()
        .filter(|t| t.title.starts_with(title))
        .nth(nth)
        .unwrap_or_else(|| panic!("no table {title} number {nth}"));
    let r = row_keys(table)
        .iter()
        .position(|k| k == row)
        .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(table)));
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", table.column_titles()));
    &table.rows[r][c]
}

fn value(tables: &[Table], title: &str, nth: usize, row: &str, column: &str) -> f64 {
    match cell(tables, title, nth, row, column) {
        Cell::Number(n) => *n,
        other => panic!("{title} / {row} / {column}: {other:?}"),
    }
}

fn near(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-8 + 1e-7 * expected.abs(),
        "{what}: {actual} is not {expected}"
    );
}

/// Included and excluded cases of the `nth` table of a list.
fn processed(t: &[Table], nth: usize, row: &str, included: f64, excluded: f64) {
    near(
        value(
            t,
            "Case Processing Summary",
            nth,
            row,
            "Cases / Included / N",
        ),
        included,
        row,
    );
    near(
        value(
            t,
            "Case Processing Summary",
            nth,
            row,
            "Cases / Excluded / N",
        ),
        excluded,
        row,
    );
}

/// Rows `a / b` of y1 by a by b: mean, N, standard deviation (None where one case has none), then
/// when `percents` the percents of the total N and sum and of those in the category of a.
fn layered(t: &[Table], rows: &[(&str, [f64; 7])], percents: bool) {
    for (key, w) in rows {
        let at = |c: &str| value(t, "Report: y1 by a by b", 0, key, c);
        near(at("Mean"), w[0], key);
        near(at("N"), w[1], key);
        if w[2].is_finite() {
            near(at("Std. Deviation"), w[2], key);
        } else {
            assert_eq!(
                cell(t, "Report: y1 by a by b", 0, key, "Std. Deviation"),
                &Cell::Empty,
                "{key}"
            );
        }
        if percents {
            near(at("% of Total N"), w[3], key);
            near(at("% of Total Sum"), w[4], key);
            near(at("% of Total N in a"), w[5], key);
            near(at("% of Total Sum in a"), w[6], key);
        }
    }
}

#[test]
fn two_nested_controls_with_percents_and_each_missing_value_option() {
    let t = run("MEANS y1 BY a BY b /CELLS=MEAN COUNT STDDEV NPCT SPCT NPCT(a) SPCT(a).");
    processed(&t, 0, "y1 * a * b", 46.0, 8.0);
    layered(
        &t,
        &[
            (
                "1 / 1",
                [
                    27.68333333,
                    6.0,
                    9.540317954,
                    13.04347826,
                    10.1077101,
                    35.29411765,
                    31.72268908,
                ],
            ),
            (
                "1 / 2",
                [
                    32.5,
                    11.0,
                    4.55675323,
                    23.91304348,
                    21.75500517,
                    64.70588235,
                    68.27731092,
                ],
            ),
            (
                "1 / Total",
                [
                    30.8,
                    17.0,
                    6.859300256,
                    36.95652174,
                    31.86271527,
                    100.0,
                    100.0,
                ],
            ),
            (
                "2 / 1",
                [
                    33.41428571,
                    7.0,
                    5.715309432,
                    15.2173913,
                    14.23355443,
                    50.0,
                    44.9202996,
                ],
            ),
            (
                "2 / 2",
                [
                    40.97142857,
                    7.0,
                    5.88747662,
                    15.2173913,
                    17.45268667,
                    50.0,
                    55.0797004,
                ],
            ),
            (
                "2 / Total",
                [
                    37.19285714,
                    14.0,
                    6.815419654,
                    30.43478261,
                    31.6862411,
                    100.0,
                    100.0,
                ],
            ),
            (
                "3 / 1",
                [
                    38.38571429,
                    7.0,
                    4.308297642,
                    15.2173913,
                    16.35124445,
                    46.66666667,
                    44.85809683,
                ],
            ),
            (
                "3 / 2",
                [
                    41.2875,
                    8.0,
                    7.206036656,
                    17.39130435,
                    20.09979918,
                    53.33333333,
                    55.14190317,
                ],
            ),
            (
                "3 / Total",
                [
                    39.93333333,
                    15.0,
                    6.0136353,
                    32.60869565,
                    36.45104363,
                    100.0,
                    100.0,
                ],
            ),
            (
                "Total / 1",
                [
                    33.435,
                    20.0,
                    7.720463239,
                    43.47826087,
                    40.69250898,
                    43.47826087,
                    40.69250898,
                ],
            ),
            (
                "Total / 2",
                [
                    37.48461538,
                    26.0,
                    7.080152106,
                    56.52173913,
                    59.30749102,
                    56.52173913,
                    59.30749102,
                ],
            ),
            (
                "Total / Total",
                [35.72391304, 46.0, 7.558811923, 100.0, 100.0, 100.0, 100.0],
            ),
        ],
        true,
    );
    // INCLUDE: the 999 of y1 and the 9 of b are values (case counts: 48 of 54)
    let t = run("MEANS y1 BY a BY b /MISSING=INCLUDE.");
    processed(&t, 0, "y1 * a * b", 48.0, 6.0);
    layered(
        &t,
        &[
            (
                "1 / 1",
                [
                    27.68333333,
                    6.0,
                    9.540317954,
                    12.5,
                    6.190369708,
                    35.29411765,
                    31.72268908,
                ],
            ),
            (
                "1 / 2",
                [
                    32.5,
                    11.0,
                    4.55675323,
                    22.91666667,
                    13.32364341,
                    64.70588235,
                    68.27731092,
                ],
            ),
            (
                "1 / Total",
                [
                    30.8,
                    17.0,
                    6.859300256,
                    35.41666667,
                    19.51401312,
                    100.0,
                    100.0,
                ],
            ),
            (
                "2 / 1",
                [
                    33.41428571,
                    7.0,
                    5.715309432,
                    14.58333333,
                    8.717203339,
                    46.66666667,
                    15.39119563,
                ],
            ),
            (
                "2 / 2",
                [
                    160.725,
                    8.0,
                    338.7581048,
                    16.66666667,
                    47.92039356,
                    53.33333333,
                    84.60880437,
                ],
            ),
            (
                "2 / Total",
                [
                    101.3133333,
                    15.0,
                    248.4243629,
                    31.25,
                    56.6375969,
                    100.0,
                    100.0,
                ],
            ),
            (
                "3 / 1",
                [
                    38.38571429,
                    7.0,
                    4.308297642,
                    14.58333333,
                    10.01416219,
                    43.75,
                    41.99093608,
                ],
            ),
            (
                "3 / 2",
                [
                    41.2875,
                    8.0,
                    7.206036656,
                    16.66666667,
                    12.30992844,
                    50.0,
                    51.61744023,
                ],
            ),
            (
                "3 / 9",
                [
                    40.9,
                    1.0,
                    f64::NAN,
                    2.083333333,
                    1.524299344,
                    6.25,
                    6.391623691,
                ],
            ),
            (
                "3 / Total",
                [
                    39.99375,
                    16.0,
                    5.814747773,
                    33.33333333,
                    23.84838998,
                    100.0,
                    100.0,
                ],
            ),
            (
                "Total / 1",
                [
                    33.435,
                    20.0,
                    7.720463239,
                    41.66666667,
                    24.92173524,
                    41.66666667,
                    24.92173524,
                ],
            ),
            (
                "Total / 2",
                [
                    73.0962963,
                    27.0,
                    185.1739172,
                    56.25,
                    73.55396541,
                    56.25,
                    73.55396541,
                ],
            ),
            (
                "Total / 9",
                [
                    40.9,
                    1.0,
                    f64::NAN,
                    2.083333333,
                    1.524299344,
                    2.083333333,
                    1.524299344,
                ],
            ),
            (
                "Total / Total",
                [55.9, 48.0, 139.2196443, 100.0, 100.0, 100.0, 100.0],
            ),
        ],
        false,
    );
    // DEPENDENT: the 9 of b is a group, the 999 of y1 is missing
    let t = run("MEANS y1 BY a BY b /MISSING=DEPENDENT.");
    processed(&t, 0, "y1 * a * b", 47.0, 7.0);
    layered(
        &t,
        &[
            (
                "1 / 1",
                [
                    27.68333333,
                    6.0,
                    9.540317954,
                    12.76595745,
                    9.862249139,
                    35.29411765,
                    31.72268908,
                ],
            ),
            (
                "1 / 2",
                [
                    32.5,
                    11.0,
                    4.55675323,
                    23.40425532,
                    21.22669517,
                    64.70588235,
                    68.27731092,
                ],
            ),
            (
                "1 / Total",
                [
                    30.8,
                    17.0,
                    6.859300256,
                    36.17021277,
                    31.08894431,
                    100.0,
                    100.0,
                ],
            ),
            (
                "2 / 1",
                [
                    33.41428571,
                    7.0,
                    5.715309432,
                    14.89361702,
                    13.8878993,
                    50.0,
                    44.9202996,
                ],
            ),
            (
                "2 / 2",
                [
                    40.97142857,
                    7.0,
                    5.88747662,
                    14.89361702,
                    17.02885643,
                    50.0,
                    55.0797004,
                ],
            ),
            (
                "2 / Total",
                [
                    37.19285714,
                    14.0,
                    6.815419654,
                    29.78723404,
                    30.91675573,
                    100.0,
                    100.0,
                ],
            ),
            (
                "3 / 1",
                [
                    38.38571429,
                    7.0,
                    4.308297642,
                    14.89361702,
                    15.95416221,
                    43.75,
                    41.99093608,
                ],
            ),
            (
                "3 / 2",
                [
                    41.2875,
                    8.0,
                    7.206036656,
                    17.0212766,
                    19.61168507,
                    50.0,
                    51.61744023,
                ],
            ),
            (
                "3 / 9",
                [
                    40.9,
                    1.0,
                    f64::NAN,
                    2.127659574,
                    2.428452678,
                    6.25,
                    6.391623691,
                ],
            ),
            (
                "3 / Total",
                [
                    39.99375,
                    16.0,
                    5.814747773,
                    34.04255319,
                    37.99429996,
                    100.0,
                    100.0,
                ],
            ),
            (
                "Total / 1",
                [
                    33.435,
                    20.0,
                    7.720463239,
                    42.55319149,
                    39.70431065,
                    42.55319149,
                    39.70431065,
                ],
            ),
            (
                "Total / 2",
                [
                    37.48461538,
                    26.0,
                    7.080152106,
                    55.31914894,
                    57.86723667,
                    55.31914894,
                    57.86723667,
                ],
            ),
            (
                "Total / 9",
                [
                    40.9,
                    1.0,
                    f64::NAN,
                    2.127659574,
                    2.428452678,
                    2.127659574,
                    2.428452678,
                ],
            ),
            (
                "Total / Total",
                [35.83404255, 47.0, 7.514226409, 100.0, 100.0, 100.0, 100.0],
            ),
        ],
        false,
    );
}

#[test]
fn table_lists_have_their_own_cases() {
    let t = run("MEANS y1 y2 BY a /y2 BY b /CELLS=MEAN COUNT.");
    processed(&t, 0, "y1 * a", 49.0, 5.0);
    processed(&t, 0, "y2 * a", 50.0, 4.0);
    processed(&t, 1, "y2 * b", 49.0, 5.0);
    for (title, nth, rows) in [
        (
            "Report: y1 by a",
            0,
            [(30.8, 17.0), (37.26666667, 15.0), (39.31764706, 17.0)],
        ),
        (
            "Report: y2 by a",
            0,
            [
                (13.62941176, 17.0),
                (10.21333333, 15.0),
                (11.78333333, 18.0),
            ],
        ),
    ] {
        for (g, (mean, n)) in ["1", "2", "3"].iter().zip(rows) {
            near(value(&t, title, nth, g, "Mean"), mean, g);
            near(value(&t, title, nth, g, "N"), n, g);
        }
    }
    for (g, (mean, n)) in ["1", "2"]
        .iter()
        .zip([(11.63684211, 19.0), (12.74333333, 30.0)])
    {
        near(value(&t, "Report: y2 by b", 0, g, "Mean"), mean, g);
        near(value(&t, "Report: y2 by b", 0, g, "N"), n, g);
    }
}

#[test]
fn frequency_weights_and_the_linearity_test_agree_with_r() {
    let t = run(
        "WEIGHT BY w.\nMEANS y1 BY a /CELLS=MEAN COUNT STDDEV VARIANCE SUM SEMEAN SKEW SESKEW KURT SEKURT MIN MAX RANGE MEDIAN /STATISTICS=ANOVA.",
    );
    let columns = [
        "Mean",
        "N",
        "Std. Deviation",
        "Variance",
        "Sum",
        "Std. Error of Mean",
        "Skewness",
        "Std. Error of Skewness",
        "Kurtosis",
        "Std. Error of Kurtosis",
        "Minimum",
        "Maximum",
        "Range",
        "Median",
    ];
    let groups: [(&str, [f64; 14]); 4] = [
        (
            "1",
            [
                30.07,
                30.0,
                6.683234299,
                44.66562069,
                902.1,
                1.220186061,
                -0.5029326177,
                0.426892396,
                -0.4935840936,
                0.8327456184,
                17.3,
                39.9,
                22.6,
                31.55,
            ],
        ),
        (
            "2",
            [
                36.36086957,
                23.0,
                6.453028826,
                41.64158103,
                836.3,
                1.345549525,
                0.01870447931,
                0.4813366615,
                -0.1158330808,
                0.9347637988,
                25.4,
                50.3,
                24.9,
                35.5,
            ],
        ),
        (
            "3",
            [
                39.9037037,
                27.0,
                5.524663685,
                30.52190883,
                1077.4,
                1.063222022,
                -0.3244938718,
                0.4478520164,
                -0.5993254245,
                0.8720665112,
                28.5,
                48.9,
                20.4,
                38.8,
            ],
        ),
        (
            "Total",
            [
                35.1975,
                80.0,
                7.482350118,
                55.98556329,
                2815.8,
                0.8365521748,
                -0.3718182743,
                0.2689089812,
                -0.09151074795,
                0.5317855505,
                17.3,
                50.3,
                33.0,
                34.8,
            ],
        ),
    ];
    for (g, want) in groups {
        for (c, w) in columns.iter().zip(want) {
            near(
                value(&t, "Report: y1 by a", 0, g, c),
                w,
                &format!("{g} {c}"),
            );
        }
    }
    let anova = [
        1417.872088,
        2.0,
        18.16582497,
        3.446444391e-07,
        3004.987412,
        77.0,
        39.02581055,
        4422.8595,
        79.0,
    ];
    near(
        value(
            &t,
            "ANOVA Table",
            0,
            "Between Groups / (Combined)",
            "Sum of Squares",
        ),
        anova[0],
        "between",
    );
    near(
        value(&t, "ANOVA Table", 0, "Between Groups / (Combined)", "df"),
        anova[1],
        "between df",
    );
    near(
        value(&t, "ANOVA Table", 0, "Between Groups / (Combined)", "F"),
        anova[2],
        "F",
    );
    near(
        value(&t, "ANOVA Table", 0, "Between Groups / (Combined)", "Sig."),
        anova[3],
        "sig",
    );
    near(
        value(&t, "ANOVA Table", 0, "Within Groups", "Sum of Squares"),
        anova[4],
        "within",
    );
    near(
        value(&t, "ANOVA Table", 0, "Within Groups", "df"),
        anova[5],
        "within df",
    );
    near(
        value(&t, "ANOVA Table", 0, "Within Groups", "Mean Square"),
        anova[6],
        "within ms",
    );
    near(
        value(&t, "ANOVA Table", 0, "Total", "Sum of Squares"),
        anova[7],
        "total",
    );
    near(
        value(&t, "ANOVA Table", 0, "Total", "df"),
        anova[8],
        "total df",
    );
    // without weights: the linear term and the deviation from it
    let t = run("MEANS y1 BY a /CELLS=MEAN COUNT /STATISTICS=ANOVA LINEARITY.");
    let lin = [
        667.4129812,
        616.6776471,
        50.73533413,
        50.73533413,
        14.25989028,
        0.0004554092901,
        1.173190404,
        0.284391667,
        0.5012162768,
    ];
    near(
        value(
            &t,
            "ANOVA Table",
            0,
            "Between Groups / (Combined)",
            "Sum of Squares",
        ),
        lin[0],
        "combined",
    );
    near(
        value(
            &t,
            "ANOVA Table",
            0,
            "Between Groups / Linearity",
            "Sum of Squares",
        ),
        lin[1],
        "linear term",
    );
    near(
        value(&t, "ANOVA Table", 0, "Between Groups / Linearity", "F"),
        lin[4],
        "linearity F",
    );
    near(
        value(&t, "ANOVA Table", 0, "Between Groups / Linearity", "Sig."),
        lin[5],
        "linearity sig",
    );
    near(
        value(
            &t,
            "ANOVA Table",
            0,
            "Between Groups / Deviation from Linearity",
            "Sum of Squares",
        ),
        lin[2],
        "deviation",
    );
    near(
        value(
            &t,
            "ANOVA Table",
            0,
            "Between Groups / Deviation from Linearity",
            "F",
        ),
        lin[6],
        "deviation F",
    );
    near(
        value(
            &t,
            "ANOVA Table",
            0,
            "Between Groups / Deviation from Linearity",
            "Sig.",
        ),
        lin[7],
        "deviation sig",
    );
    near(
        value(&t, "Measures of Association", 0, "y1 * a", "Eta"),
        lin[8],
        "eta",
    );
}
