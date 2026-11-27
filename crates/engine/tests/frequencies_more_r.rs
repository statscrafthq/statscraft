//! FREQUENCIES and DESCRIPTIVES options against R on 60 invented cases with missing values (system
//! and user-missing 99 in two variables) and integer weights: the statistics of three variables
//! with different case counts (default and MISSING=INCLUDE), the frequency table with its user-
//! missing and system-missing rows (percent of all cases, valid percent and cumulative percent of
//! the valid ones), the table orders (AFREQ, DFREQ, DVALUE, LIMIT), percentiles and ntiles added
//! together, grouped percentiles and the grouped median (values as midpoints with the manual's
//! cumulative midpoints formula; boundaries and a width with the interpolation inside the group),
//! and DESCRIPTIVES with each missing option, SORT and weighted z-scores saved over the
//! listwise cases. Scripts in lab/oracles/frequencies-r2.
//!
//! Where the manual is silent, a value on a group boundary belongs to the group above it, except
//! the last boundary, which closes the last group (the engine's rule, written the same way in R).

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /x y z w (4F8.2) s (A5).
BEGIN DATA.
70  4.1  1.2 2 red
41  6.5  3.9 2 green
.  4.4 14.0 2 green
49  3.1 14.7 1 red
50  3.1 12.0 1 red
53 23.3  4.8 1 blue
59 18.3 13.3 2 red
53 . 13.3 4 red
54 23.9 18.8 3 green
43 23.4 19.4 2 red
54 17.2 12.6 3 red
68 12.2 13.8 4 red
52  7.7  7.8 1 green
79 22.9 . 2 red
36 17.6 11.3 4 green
40  3.9 17.5 3 red
50  4.4  3.0 4 blue
55  7.4  7.7 2 red
57  3.7  7.8 1 blue
55 27.1 18.6 1 red
61 26.0 16.6 1 blue
. .  7.8 1 red
62 12.0  4.2 3 blue
37 24.6 11.8 2 red
41  4.0  4.8 1 grey
51  4.1 15.5 3 blue
22 38.8  3.0 2 red
55  5.0  2.4 2 red
37 15.7 14.5 3 red
44 12.3 99.0 4 grey
32 19.9 16.4 1 green
55  5.1 12.8 3 red
61  9.7 11.9 1 red
65  3.5  8.2 1 green
48 34.5 17.5 2 red
51 17.3  5.0 1 green
23 24.6 11.8 3 red
70 12.8 19.3 2 red
48  8.9  5.7 4 green
99 11.5  6.4 2 green
32  9.4  9.9 2 red
51 14.5  6.9 1 red
53  3.2 19.3 3 red
53  3.5  8.3 1 grey
60 21.2 17.0 2 red
54 59.2  5.0 3 red
53 25.7 10.5 4 green
61  4.8 14.7 1 blue
64  9.9 16.2 1 green
38 16.5 15.2 2 green
39 . . 1 green
39 11.2 10.8 3 red
47 11.5  8.2 2 red
54  3.2  3.9 2 red
42  9.1  6.5 2 blue
38 11.8 15.3 4 red
54 19.9  6.7 1 blue
55 11.8 19.9 1 red
71 17.5 11.3 1 red
53 20.3  4.2 1 blue
END DATA.
MISSING VALUES x z (99).
";

fn engine_run(syntax: &str) -> (Engine, Vec<Table>) {
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
    (engine, tables)
}

fn run(syntax: &str) -> Vec<Table> {
    engine_run(syntax).1
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

fn value(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let r = row_keys(table)
        .iter()
        .position(|k| k == row)
        .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(table)));
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", table.column_titles()));
    match &table.rows[r][c] {
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

const STATS: [&str; 15] = [
    "N / Valid",
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

#[test]
fn statistics_of_variables_with_their_own_missing_values_agree_with_r() {
    // R's order: valid, missing, then W, mean, se, median, mode, sd, variance, skewness and its se,
    // kurtosis and its se, range, minimum, maximum, sum
    let want: [(&str, &str, [f64; 17]); 6] = [
        (
            "DEFAULT",
            "x",
            [
                57.0,
                3.0,
                57.0,
                50.73684211,
                1.528043271,
                53.0,
                53.0,
                11.53647371,
                133.0902256,
                -0.1942260644,
                0.3163268814,
                0.2896507711,
                0.6231339036,
                57.0,
                22.0,
                79.0,
                2892.0,
            ],
        ),
        (
            "DEFAULT",
            "y",
            [
                57.0,
                3.0,
                57.0,
                14.1877193,
                1.399655503,
                11.8,
                3.1,
                10.56716731,
                111.6650251,
                1.681810443,
                0.3163268814,
                4.734130342,
                0.6231339036,
                56.1,
                3.1,
                59.2,
                808.7,
            ],
        ),
        (
            "DEFAULT",
            "z",
            [
                57.0,
                3.0,
                57.0,
                10.89298246,
                0.6984401689,
                11.3,
                7.8,
                5.273107638,
                27.80566416,
                0.003656993647,
                0.3163268814,
                -1.17266723,
                0.6231339036,
                18.7,
                1.2,
                19.9,
                620.9,
            ],
        ),
        (
            "INCLUDE",
            "x",
            [
                58.0,
                2.0,
                58.0,
                51.56896552,
                1.716633699,
                53.0,
                53.0,
                13.07349276,
                170.9162129,
                0.5851149944,
                0.3137199326,
                2.324625607,
                0.6181358368,
                77.0,
                22.0,
                99.0,
                2991.0,
            ],
        ),
        (
            "INCLUDE",
            "y",
            [
                57.0,
                3.0,
                57.0,
                14.1877193,
                1.399655503,
                11.8,
                3.1,
                10.56716731,
                111.6650251,
                1.681810443,
                0.3163268814,
                4.734130342,
                0.6231339036,
                56.1,
                3.1,
                59.2,
                808.7,
            ],
        ),
        (
            "INCLUDE",
            "z",
            [
                58.0,
                2.0,
                58.0,
                12.41206897,
                1.666919662,
                11.55,
                7.8,
                12.69488193,
                161.1600272,
                5.701147813,
                0.3137199326,
                39.09598601,
                0.6181358368,
                97.8,
                1.2,
                99.0,
                719.9,
            ],
        ),
    ];
    for (mode, variable, w) in want {
        let options = if mode == "INCLUDE" {
            " /MISSING=INCLUDE"
        } else {
            ""
        };
        let t = run(&format!(
            "FREQUENCIES x y z /STATISTICS=ALL /FORMAT=NOTABLE{options}."
        ));
        near(
            value(&t, "Statistics", "N / Missing", variable),
            w[1],
            &format!("{mode} {variable} missing"),
        );
        for (label, k) in STATS
            .iter()
            .zip([0usize, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])
        {
            near(
                value(&t, "Statistics", label, variable),
                w[k],
                &format!("{mode} {variable} {label}"),
            );
        }
    }
}

#[test]
fn the_frequency_table_lists_user_missing_and_system_missing_rows() {
    let t = run("FREQUENCIES z.");
    let table = t.iter().find(|t| t.title == "z").expect("table of z");
    let rows: [[f64; 5]; 43] = [
        [1.2, 1.0, 1.666666667, 1.754385965, 1.754385965],
        [2.4, 1.0, 1.666666667, 1.754385965, 3.50877193],
        [3.0, 2.0, 3.333333333, 3.50877193, 7.01754386],
        [3.9, 2.0, 3.333333333, 3.50877193, 10.52631579],
        [4.2, 2.0, 3.333333333, 3.50877193, 14.03508772],
        [4.8, 2.0, 3.333333333, 3.50877193, 17.54385965],
        [5.0, 2.0, 3.333333333, 3.50877193, 21.05263158],
        [5.7, 1.0, 1.666666667, 1.754385965, 22.80701754],
        [6.4, 1.0, 1.666666667, 1.754385965, 24.56140351],
        [6.5, 1.0, 1.666666667, 1.754385965, 26.31578947],
        [6.7, 1.0, 1.666666667, 1.754385965, 28.07017544],
        [6.9, 1.0, 1.666666667, 1.754385965, 29.8245614],
        [7.7, 1.0, 1.666666667, 1.754385965, 31.57894737],
        [7.8, 3.0, 5.0, 5.263157895, 36.84210526],
        [8.2, 2.0, 3.333333333, 3.50877193, 40.35087719],
        [8.3, 1.0, 1.666666667, 1.754385965, 42.10526316],
        [9.9, 1.0, 1.666666667, 1.754385965, 43.85964912],
        [10.5, 1.0, 1.666666667, 1.754385965, 45.61403509],
        [10.8, 1.0, 1.666666667, 1.754385965, 47.36842105],
        [11.3, 2.0, 3.333333333, 3.50877193, 50.87719298],
        [11.8, 2.0, 3.333333333, 3.50877193, 54.38596491],
        [11.9, 1.0, 1.666666667, 1.754385965, 56.14035088],
        [12.0, 1.0, 1.666666667, 1.754385965, 57.89473684],
        [12.6, 1.0, 1.666666667, 1.754385965, 59.64912281],
        [12.8, 1.0, 1.666666667, 1.754385965, 61.40350877],
        [13.3, 2.0, 3.333333333, 3.50877193, 64.9122807],
        [13.8, 1.0, 1.666666667, 1.754385965, 66.66666667],
        [14.0, 1.0, 1.666666667, 1.754385965, 68.42105263],
        [14.5, 1.0, 1.666666667, 1.754385965, 70.1754386],
        [14.7, 2.0, 3.333333333, 3.50877193, 73.68421053],
        [15.2, 1.0, 1.666666667, 1.754385965, 75.43859649],
        [15.3, 1.0, 1.666666667, 1.754385965, 77.19298246],
        [15.5, 1.0, 1.666666667, 1.754385965, 78.94736842],
        [16.2, 1.0, 1.666666667, 1.754385965, 80.70175439],
        [16.4, 1.0, 1.666666667, 1.754385965, 82.45614035],
        [16.6, 1.0, 1.666666667, 1.754385965, 84.21052632],
        [17.0, 1.0, 1.666666667, 1.754385965, 85.96491228],
        [17.5, 2.0, 3.333333333, 3.50877193, 89.47368421],
        [18.6, 1.0, 1.666666667, 1.754385965, 91.22807018],
        [18.8, 1.0, 1.666666667, 1.754385965, 92.98245614],
        [19.3, 2.0, 3.333333333, 3.50877193, 96.49122807],
        [19.4, 1.0, 1.666666667, 1.754385965, 98.24561404],
        [19.9, 1.0, 1.666666667, 1.754385965, 100.0],
    ];
    let valid: Vec<&Vec<Cell>> = table
        .rows
        .iter()
        .filter(|r| matches!(&r[0], Cell::Text(s) if s == "valid"))
        .collect();
    assert_eq!(valid.len(), rows.len());
    for (row, want) in valid.iter().zip(rows) {
        for (k, w) in want.iter().enumerate() {
            match &row[[1usize, 3, 4, 5, 6][k]] {
                Cell::Number(n) => near(*n, *w, "table cell"),
                other => panic!("{other:?}"),
            }
        }
    }
    let user = [1.0, 1.666666667];
    let system = [2.0, 3.333333333];
    let missing: Vec<&Vec<Cell>> = table
        .rows
        .iter()
        .filter(|r| matches!(&r[0], Cell::Text(s) if s == "missing"))
        .collect();
    assert_eq!(missing.len(), 2);
    assert_eq!(missing[0][1], Cell::Number(99.0));
    assert_eq!(missing[0][3], Cell::Number(user[0]));
    near(
        match &missing[0][4] {
            Cell::Number(n) => *n,
            other => panic!("{other:?}"),
        },
        user[1],
        "user-missing percent",
    );
    assert_eq!(missing[0][5], Cell::Empty);
    assert_eq!(missing[1][3], Cell::Number(system[0]));
    near(
        match &missing[1][4] {
            Cell::Number(n) => *n,
            other => panic!("{other:?}"),
        },
        system[1],
        "system-missing percent",
    );
    // MISSING=INCLUDE: 99 is a value, so the percents of the valid cases are over 58
    let t = run("FREQUENCIES x /MISSING=INCLUDE.");
    let table = t.iter().find(|t| t.title == "x").expect("table of x");
    let include = [33.0, 1.724137931];
    let valid: Vec<&Vec<Cell>> = table
        .rows
        .iter()
        .filter(|r| matches!(&r[0], Cell::Text(s) if s == "valid"))
        .collect();
    assert_eq!(valid.len(), include[0] as usize);
    let last = valid[valid.len() - 1];
    assert_eq!(last[1], Cell::Number(99.0));
    near(
        match &last[5] {
            Cell::Number(n) => *n,
            other => panic!("{other:?}"),
        },
        include[1],
        "valid percent of 99",
    );
    near(
        match &last[6] {
            Cell::Number(n) => *n,
            other => panic!("{other:?}"),
        },
        100.0,
        "cumulative percent",
    );
}

#[test]
fn table_orders_percentiles_with_ntiles_and_grouped_percentiles_agree_with_r() {
    // the table of s in each order, as the values read down the rows and their frequencies
    let order = |syntax: &str| -> Vec<(String, f64)> {
        let t = run(syntax);
        let table = t.iter().find(|t| t.title == "s").expect("table of s");
        table
            .rows
            .iter()
            .filter(|r| matches!(&r[0], Cell::Text(s) if s == "valid"))
            .map(|r| match (&r[1], &r[3]) {
                (Cell::Text(v), Cell::Number(n)) => (v.clone(), *n),
                other => panic!("{other:?}"),
            })
            .collect()
    };
    let (blue, green, grey, red) = (10.0, 14.0, 3.0, 33.0);
    assert_eq!(
        order("FREQUENCIES s /FORMAT=AFREQ."),
        [
            ("grey".into(), grey),
            ("blue".into(), blue),
            ("green".into(), green),
            ("red".into(), red)
        ]
    );
    assert_eq!(
        order("FREQUENCIES s /FORMAT=DFREQ."),
        [
            ("red".into(), red),
            ("green".into(), green),
            ("blue".into(), blue),
            ("grey".into(), grey)
        ]
    );
    assert_eq!(
        order("FREQUENCIES s /FORMAT=DVALUE."),
        [
            ("red".into(), red),
            ("grey".into(), grey),
            ("green".into(), green),
            ("blue".into(), blue)
        ]
    );
    // LIMIT(4) keeps the table of the four-valued s and leaves out the table of x
    let t = run("FREQUENCIES s x /FORMAT=LIMIT(4).");
    assert!(t.iter().any(|t| t.title == "s") && !t.iter().any(|t| t.title == "x"));
    // PERCENTILES and NTILES add up: 25 50 90 with quartiles, 10 with quintiles
    let t = run("FREQUENCIES z /PERCENTILES=25 50 90 /NTILES=4 /FORMAT=NOTABLE.");
    for (p, w) in ["25", "50", "75", "90"]
        .iter()
        .zip([6.45, 11.3, 15.25, 18.64])
    {
        near(
            value(&t, "Statistics", &format!("Percentiles / {p}"), "z"),
            w,
            p,
        );
    }
    let t = run("FREQUENCIES z /PERCENTILES=10 /NTILES=5 /FORMAT=NOTABLE.");
    for (p, w) in ["10", "20", "40", "60", "80"]
        .iter()
        .zip([3.9, 5.0, 8.22, 12.76, 16.28])
    {
        near(
            value(&t, "Statistics", &format!("Percentiles / {p}"), "z"),
            w,
            p,
        );
    }
    // grouped: the values of x as midpoints, the boundaries 30 to 70, and z in groups 0.1 wide
    let percentiles = ["10", "25", "50", "75", "90"];
    for (spec, data, variable, want) in [
        (
            "/GROUPED=x",
            "x",
            "x",
            [36.8, 41.83333333, 52.57142857, 56.5, 64.8],
        ),
        (
            "/GROUPED=x (30, 40, 50, 60, 70)",
            "x",
            "x",
            [34.11111111, 43.25, 53.125, 59.0625, 66.3],
        ),
        (
            "/GROUPED=z (0.1)",
            "z",
            "z",
            [3.935, 6.475, 11.325, 15.225, 18.58],
        ),
        (
            "/GROUPED=z",
            "z",
            "z",
            [4.005, 6.475, 11.425, 15.225, 18.45333333],
        ),
    ] {
        let t = run(&format!(
            "FREQUENCIES {data} {spec} /PERCENTILES=10 25 50 75 90 /FORMAT=NOTABLE."
        ));
        for (p, w) in percentiles.iter().zip(want) {
            near(
                value(&t, "Statistics", &format!("Percentiles / {p}"), variable),
                w,
                &format!("{spec} {p}"),
            );
        }
    }
    // the grouped median of z in groups 0.1 wide is its 50th grouped percentile
    let t = run("FREQUENCIES z /STATISTICS=MEDIAN /GROUPED=z (0.1) /FORMAT=NOTABLE.");
    near(
        value(&t, "Statistics", "Median", "z"),
        [3.935, 6.475, 11.325, 15.225, 18.58][2],
        "grouped median",
    );
}

#[test]
fn descriptives_missing_options_sort_and_weighted_z_scores_agree_with_r() {
    // each variable on its own: N, mean, sd, minimum, maximum
    let t = run("DESCRIPTIVES x y z.");
    for (v, w) in ["x", "y", "z"].iter().zip([
        [57.0, 50.73684211, 11.53647371, 22.0, 79.0],
        [57.0, 14.1877193, 10.56716731, 3.1, 59.2],
        [57.0, 10.89298246, 5.273107638, 1.2, 19.9],
    ]) {
        let row = v.to_string();
        for (c, k) in [
            ("N", 0),
            ("Mean", 1),
            ("Std Dev", 2),
            ("Minimum", 3),
            ("Maximum", 4),
        ] {
            near(
                value(&t, "Descriptive Statistics", &row, c),
                w[k],
                &format!("{v} {c}"),
            );
        }
    }
    near(
        value(&t, "Descriptive Statistics", "Valid N (listwise)", "N"),
        53.0,
        "listwise N",
    );
    // listwise
    let t = run("DESCRIPTIVES x y z /MISSING=LISTWISE.");
    for (v, w) in ["x", "y", "z"].iter().zip([
        [53.0, 50.50943396, 11.14868506, 22.0, 71.0],
        [53.0, 14.29433962, 10.80433344, 3.1, 59.2],
        [53.0, 10.93207547, 5.392008816, 1.2, 19.9],
    ]) {
        for (c, k) in [
            ("N", 0),
            ("Mean", 1),
            ("Std Dev", 2),
            ("Minimum", 3),
            ("Maximum", 4),
        ] {
            near(
                value(&t, "Descriptive Statistics", v, c),
                w[k],
                &format!("listwise {v} {c}"),
            );
        }
    }
    near(
        value(&t, "Descriptive Statistics", "Missing N (listwise)", "N"),
        7.0,
        "listwise missing",
    );
    // include: 99 is a value
    let t = run("DESCRIPTIVES x y z /MISSING=VARIABLE INCLUDE.");
    for (v, w) in ["x", "y", "z"].iter().zip([
        [58.0, 51.56896552, 13.07349276, 22.0, 99.0],
        [57.0, 14.1877193, 10.56716731, 3.1, 59.2],
        [58.0, 12.41206897, 12.69488193, 1.2, 99.0],
    ]) {
        for (c, k) in [
            ("N", 0),
            ("Mean", 1),
            ("Std Dev", 2),
            ("Minimum", 3),
            ("Maximum", 4),
        ] {
            near(
                value(&t, "Descriptive Statistics", v, c),
                w[k],
                &format!("include {v} {c}"),
            );
        }
    }
    // SORT: by the standard deviation ascending, by name descending
    let order = |syntax: &str| -> Vec<String> {
        let t = run(syntax);
        let table = t
            .iter()
            .find(|t| t.title == "Descriptive Statistics")
            .unwrap();
        row_keys(table)
            .into_iter()
            .filter(|k| ["x", "y", "z"].contains(&k.as_str()))
            .collect()
    };
    assert_eq!(
        order("DESCRIPTIVES x y z /SORT=STDDEV(A)."),
        ["z", "y", "x"]
    );
    assert_eq!(order("DESCRIPTIVES x y z /SORT=NAME(D)."), ["z", "y", "x"]);
    assert_eq!(order("DESCRIPTIVES x y z /SORT=MEAN(D)."), ["x", "y", "z"]);
    // weighted, listwise: the z-scores of the listwise cases use the weighted mean and sd; a case
    // missing on any variable gets none
    let (engine, t) = engine_run("WEIGHT BY w.\nDESCRIPTIVES x y z /MISSING=LISTWISE /SAVE.");
    near(
        value(&t, "Descriptive Statistics", "Valid N (listwise)", "N"),
        109.0,
        "weighted listwise N",
    );
    let columns = ["Zx", "Zy", "Zz"];
    let zs = [
        (
            (49.29357798, 11.31231937),
            [1.830431174, -0.7331456717, -0.02595205918, 0.06244714239],
        ),
        (
            (14.73302752, 11.36673228),
            [-0.9354515668, -0.7243090906, -1.023427599, -1.023427599],
        ),
        (
            (11.04587156, 5.332300805),
            [-1.84645839, -1.340110362, 0.6852817525, 0.178933724],
        ),
    ];
    for (name, (mean_sd, z)) in columns.iter().zip(zs) {
        let data = engine.dataset().unwrap();
        let Values::Numeric(col) = data.variable(name).unwrap().values() else {
            panic!("{name} is a string")
        };
        near(
            value(&t, "Descriptive Statistics", &name[1..], "Mean"),
            mean_sd.0,
            name,
        );
        near(
            value(&t, "Descriptive Statistics", &name[1..], "Std Dev"),
            mean_sd.1,
            name,
        );
        for (case, want) in [0usize, 1, 3, 4].iter().zip(z) {
            near(
                col.get(*case).expect("a z-score"),
                want,
                &format!("{name} case {case}"),
            );
        }
        // case 3 is missing x, case 8 is missing y, case 14 is missing z: none gets a score
        assert_eq!(col.get(2), None);
        assert_eq!(col.get(7), None);
    }
}
