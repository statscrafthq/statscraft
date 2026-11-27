//! CTABLES summary statistics against R on 90 invented cases (a with three categories, b with two
//! and a user-missing 9, c with two, a scale x with missing values, frequency weights; system-
//! missing values in a): counts and the row, column and table percentages with totals, a nested
//! row variable (a > c) by b, the Valid N and Total N bases (a Total N takes in the cases with a
//! missing category), and the scale statistics of x in each category of a and in the total: mean,
//! median, mode, minimum, maximum, range, sum, standard deviation, variance, standard error of
//! the mean, the 25th and 75th percentiles (PTILE is APTILE of the manual: R's `quantile` type 2, the averaged
//! empirical distribution), Valid N, Total N and Missing, then the same with frequency weights from R on the
//! cases written out as often as their weight. Scripts in lab/oracles/ctables-r.
//!
//! On record: a case whose a is system-missing counts in the Total N of the total, as the manual's
//! definition of Total N says (non-missing, user-missing and system-missing values), so the total
//! row of x has a Total N of 90, a Valid N of 85 (valid x and valid a) and a Missing of 3 (the cases
//! missing x, whatever their a); and the Total N percentage of the Total
//! column takes the cases of the row category whose b is user-missing as well.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /a b c x y w (6F8.2).
BEGIN DATA.
3 1 2 49 23.5 2
2 2 2 48 2 3
3 1 2 59 31.7 1
2 9 2 46 5 3
2 2 1 56 . 1
3 1 1 58 2.7 1
3 1 1 56 4.6 3
1 1 2 . 16.3 1
1 2 1 35 3.4 3
3 1 2 40 24.2 2
1 2 2 40 13.9 1
. 2 1 42 20.2 3
3 2 2 63 9.5 3
1 1 1 40 2.8 1
1 2 2 47 1.4 2
2 2 2 64 15 2
2 2 2 50 2.2 1
2 2 1 52 5.3 2
3 2 1 57 2.5 2
1 1 1 . 2.9 3
1 2 2 37 25.5 3
1 1 2 21 14 1
3 2 1 59 33.4 2
2 1 2 48 2.6 1
1 2 2 55 15 3
3 1 1 72 4.1 1
1 1 1 45 1.5 1
1 2 1 44 7.7 1
3 1 1 62 1.5 3
2 1 1 52 1.9 1
1 9 2 52 1.2 2
3 1 1 69 2.2 1
1 2 1 54 65 2
1 1 1 65 15.5 2
2 1 1 51 32.5 3
2 1 1 49 3 1
2 1 1 53 8.2 3
2 1 2 64 8.6 3
2 2 2 55 6.1 3
1 2 1 56 13.6 2
2 1 2 55 5.6 1
1 2 1 49 19.4 3
1 2 2 50 23.6 3
1 1 1 41 2.6 3
3 1 2 65 8.3 3
2 1 2 62 1.7 1
1 1 1 36 11.9 1
1 2 2 45 23.3 1
3 2 2 63 36.9 2
1 1 2 38 . 1
3 2 1 52 6.4 2
2 1 2 71 21 3
2 1 2 37 11.4 2
1 1 2 49 5.9 1
. 1 1 43 16.9 1
1 1 1 39 3.4 1
3 1 2 34 2.9 2
3 1 2 58 13.5 1
1 2 1 53 4.6 3
2 1 1 40 35.7 1
2 1 1 68 4.9 2
1 1 2 72 8.6 3
1 1 1 51 28.7 1
3 1 2 . 6.9 1
1 1 2 31 4.5 1
3 2 1 55 7.8 2
2 1 2 54 4.9 2
3 2 2 51 1.9 2
2 1 1 43 7.3 2
2 9 1 65 6.2 2
1 1 1 48 23.5 1
1 1 1 30 23.6 1
3 2 1 67 6.1 1
1 1 2 35 17.3 1
1 2 1 52 1.7 1
2 2 1 70 20 1
3 2 1 63 13.5 1
1 2 2 38 1.8 3
1 1 1 50 9.4 1
1 1 1 38 2.9 2
3 1 1 58 24.5 3
1 1 2 51 3.5 1
2 2 2 66 3.9 2
3 1 2 65 1.4 1
2 2 2 60 13.9 1
1 2 2 50 1.4 1
1 2 1 32 20.1 2
2 1 2 74 4.7 1
3 1 1 62 5.2 2
1 1 2 63 4.8 2
END DATA.
VARIABLE LEVEL a b c (NOMINAL) /x y w (SCALE).
MISSING VALUES b (9).
VALUE LABELS a 1 'A1' 2 'A2' 3 'A3' /b 1 'B1' 2 'B2' 9 'B9' /c 1 'C1' 2 'C2'.
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

fn value(t: &Table, row: &str, column: &str) -> f64 {
    let r = row_keys(t)
        .iter()
        .position(|k| k == row)
        .unwrap_or_else(|| panic!("no row {row} in {:?}", row_keys(t)));
    let c = t
        .column_titles()
        .iter()
        .position(|k| k == column)
        .unwrap_or_else(|| panic!("no column {column} in {:?}", t.column_titles()));
    match &t.rows[r][c] {
        Cell::Number(n) => *n,
        other => panic!("{row} / {column}: {other:?}"),
    }
}

fn near(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-8 + 1e-7 * expected.abs(),
        "{what}: {actual} is not {expected}"
    );
}

fn first(tables: &[Table]) -> &Table {
    tables
        .iter()
        .find(|t| t.title == "Custom Tables")
        .expect("a table")
}

#[test]
fn counts_percentages_and_bases_agree_with_r() {
    let t = run(
        "CTABLES /TABLE a [COUNT, ROWPCT.COUNT, COLPCT.COUNT, TABLEPCT.COUNT] BY b /CATEGORIES VARIABLES=a b TOTAL=YES.",
    );
    let t = first(&t);
    // per row of a: B1 count, B2 count, total count, row % B1, B2, column % B1, B2, table % total, table % B1, B2, total
    let counts: [[f64; 11]; 3] = [
        [
            21.0,
            16.0,
            37.0,
            56.75675676,
            43.24324324,
            41.17647059,
            47.05882353,
            43.52941176,
            24.70588235,
            18.82352941,
            43.52941176,
        ],
        [
            15.0,
            9.0,
            24.0,
            62.5,
            37.5,
            29.41176471,
            26.47058824,
            28.23529412,
            17.64705882,
            10.58823529,
            28.23529412,
        ],
        [
            15.0,
            9.0,
            24.0,
            62.5,
            37.5,
            29.41176471,
            26.47058824,
            28.23529412,
            17.64705882,
            10.58823529,
            28.23529412,
        ],
    ];
    let total = [51.0, 34.0, 85.0, 60.0];
    for (i, row) in ["A1", "A2", "A3"].iter().enumerate() {
        let r = format!("a / {row}");
        let w = counts[i];
        for (column, want) in [
            ("b / B1 / Count", w[0]),
            ("b / B2 / Count", w[1]),
            ("b / Total / Count", w[2]),
            ("b / B1 / Row %", w[3]),
            ("b / B2 / Row %", w[4]),
            ("b / B1 / Column %", w[5]),
            ("b / B2 / Column %", w[6]),
            ("b / Total / Column %", w[2] / total[2] * 100.0),
            ("b / B1 / Table %", w[8]),
            ("b / B2 / Table %", w[9]),
            ("b / Total / Table %", w[10]),
        ] {
            near(value(t, &r, column), want, &format!("{r} {column}"));
        }
    }
    near(
        value(t, "a / Total", "b / B1 / Count"),
        total[0],
        "total B1",
    );
    near(
        value(t, "a / Total", "b / B2 / Count"),
        total[1],
        "total B2",
    );
    near(
        value(t, "a / Total", "b / Total / Count"),
        total[2],
        "grand total",
    );
    near(
        value(t, "a / Total", "b / B1 / Row %"),
        total[3],
        "total row % B1",
    );
    // nested rows: a > c by b, counts of each cell and of the Total rows
    let t = run("CTABLES /TABLE a > c BY b [COUNT] /CATEGORIES VARIABLES=a b c TOTAL=YES.");
    let t = first(&t);
    let nested: [(&str, [f64; 3]); 6] = [
        ("a / A1 / c / C1", [12.0, 8.0, 20.0]),
        ("a / A1 / c / C2", [9.0, 8.0, 17.0]),
        ("a / A2 / c / C1", [7.0, 3.0, 10.0]),
        ("a / A2 / c / C2", [8.0, 6.0, 14.0]),
        ("a / A3 / c / C1", [7.0, 6.0, 13.0]),
        ("a / A3 / c / C2", [8.0, 3.0, 11.0]),
    ];
    for (row, w) in nested {
        for (column, want) in ["b / B1 / Count", "b / B2 / Count", "b / Total / Count"]
            .iter()
            .zip(w)
        {
            near(value(t, row, column), want, &format!("{row} {column}"));
        }
    }
    for (row, w) in [
        ("a / Total / c / C1", [26.0, 17.0, 43.0]),
        ("a / Total / c / C2", [25.0, 17.0, 42.0]),
    ] {
        for (column, want) in ["b / B1 / Count", "b / B2 / Count", "b / Total / Count"]
            .iter()
            .zip(w)
        {
            near(value(t, row, column), want, &format!("{row} {column}"));
        }
    }
    // Valid N and Total N bases: the Total N of the column B1 takes in the case with a missing a
    let t = run(
        "CTABLES /TABLE a [COUNT, ROWPCT.VALIDN, COLPCT.TOTALN] BY b /CATEGORIES VARIABLES=a b TOTAL=YES.",
    );
    let t = first(&t);
    for (i, row) in ["A1", "A2", "A3"].iter().enumerate() {
        let r = format!("a / {row}");
        let w = [
            [40.38461538, 45.71428571, 42.22222222],
            [28.84615385, 25.71428571, 28.88888889],
            [28.84615385, 25.71428571, 26.66666667],
        ][i];
        near(value(t, &r, "b / B1 / Row Valid N %"), counts[i][3], &r);
        near(value(t, &r, "b / B1 / Column Total N %"), w[0], &r);
        near(value(t, &r, "b / B2 / Column Total N %"), w[1], &r);
        near(value(t, &r, "b / Total / Column Total N %"), w[2], &r);
    }
}

const SCALE: [&str; 15] = [
    "x / Mean",
    "x / Median",
    "x / Mode",
    "x / Minimum",
    "x / Maximum",
    "x / Range",
    "x / Sum",
    "x / Std Deviation",
    "x / Variance",
    "x / Std Error of Mean",
    "x / Percentile 25",
    "x / Percentile 75",
    "x / Valid N",
    "x / Total N",
    "x / Missing",
];

#[test]
fn the_summary_statistics_of_a_scale_variable_agree_with_r() {
    let t = run(
        "CTABLES /TABLE a BY x [MEAN, MEDIAN, MODE, MINIMUM, MAXIMUM, RANGE, SUM, STDDEV, VARIANCE, SEMEAN, PTILE 25, PTILE 75, VALIDN, TOTALN, MISSING] /CATEGORIES VARIABLES=a TOTAL=YES.",
    );
    let t = first(&t);
    let rows: [(&str, [f64; 15]); 4] = [
        (
            "a / A1",
            [
                45.33333333,
                46.0,
                38.0,
                21.0,
                72.0,
                51.0,
                1632.0,
                10.50985932,
                110.4571429,
                1.751643219,
                38.0,
                51.5,
                36.0,
                38.0,
                2.0,
            ],
        ),
        (
            "a / A2",
            [
                55.88461538,
                54.5,
                48.0,
                37.0,
                74.0,
                37.0,
                1453.0,
                9.840028143,
                96.82615385,
                1.929788289,
                49.0,
                64.0,
                26.0,
                26.0,
                0.0,
            ],
        ),
        (
            "a / A3",
            [
                58.13043478,
                59.0,
                58.0,
                34.0,
                72.0,
                38.0,
                1337.0,
                8.750606981,
                76.57312253,
                1.824627687,
                55.0,
                63.0,
                23.0,
                24.0,
                1.0,
            ],
        ),
        (
            "a / Total",
            [
                52.02352941,
                52.0,
                52.0,
                21.0,
                74.0,
                53.0,
                4422.0,
                11.35359862,
                128.9042017,
                1.231470674,
                45.0,
                62.0,
                85.0,
                90.0,
                3.0,
            ],
        ),
    ];
    for (row, w) in rows {
        for (column, want) in SCALE.iter().zip(w) {
            near(value(t, row, column), want, &format!("{row} {column}"));
        }
    }
    // frequency weights: the statistics of the cases written out as often as their weight
    let t = run(
        "CTABLES /WEIGHT VARIABLE=w /TABLE a BY x [MEAN, STDDEV, VARIANCE, SUM, VALIDN, MEDIAN, PTILE 75, COUNT].",
    );
    let t = first(&t);
    let weighted: [(&str, [f64; 8]); 3] = [
        (
            "a / A1",
            [
                46.75806452,
                10.86532382,
                118.0552618,
                2899.0,
                62.0,
                48.5,
                53.0,
                66.0,
            ],
        ),
        (
            "a / A2",
            [
                55.79166667,
                9.602655513,
                92.21099291,
                2678.0,
                48.0,
                54.0,
                64.0,
                48.0,
            ],
        ),
        (
            "a / A3",
            [
                57.37209302,
                8.414905301,
                70.81063123,
                2467.0,
                43.0,
                58.0,
                63.0,
                44.0,
            ],
        ),
    ];
    for (row, w) in weighted {
        for (column, want) in [
            "x / Mean",
            "x / Std Deviation",
            "x / Variance",
            "x / Sum",
            "x / Valid N",
            "x / Median",
            "x / Percentile 75",
            "x / Count",
        ]
        .iter()
        .zip(w)
        {
            near(value(t, row, column), want, &format!("{row} {column}"));
        }
    }
}
