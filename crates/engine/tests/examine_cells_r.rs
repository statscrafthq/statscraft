//! EXAMINE with a factor against R on 38 invented values in three groups (12, 15 and 11, one
//! extreme value, ties) and a second factor that crosses them: the descriptives of every cell
//! with the 90 percent interval, the percentiles by the weighted average and Tukey's hinges,
//! the three highest and lowest values with their case numbers, the tests of normality (the
//! Kolmogorov-Smirnov statistic with the Lilliefors significance of the manual's formulas, and
//! `shapiro.test`), the four Levene tests (on the data, and on the data raised to a power or
//! logged when SPREADLEVEL names one) with the manual's adjusted degrees of freedom, the
//! spread-versus-level slope, intercept and points, and the boxplot statistics (Tukey's hinges,
//! the whiskers at the last values inside 1.5 steps, the extremes past 3). Scripts in
//! lab/oracles/examine-r2.
//!
//! Where values tie, the extreme values table lists the later case first among the highest and the
//! earlier case first among the lowest; the manual is silent and the engine's order is kept.
//!
//! One choice is on record: the significance between the critical values for 0.2 and 0.1 uses the
//! critical value at 0.2 of the published table with four decimals (0.2004 for 12 cases), not the
//! three decimals of the table the manual prints for NPTESTS (0.200), which would move the
//! significance of the 12-case group by 0.0012.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g h y.
BEGIN DATA.
1 1 9.7
1 2 7.5
1 1 6.2
1 2 6.2
1 1 4.9
1 2 10.7
1 1 5.7
1 2 9
1 1 6.8
1 2 13.1
1 1 6.9
1 2 5.4
2 1 8.3
2 2 11
2 1 5.4
2 2 6.7
2 1 12
2 2 20.7
2 1 19.6
2 2 20.7
2 1 24.6
2 2 9.3
2 1 6.4
2 2 15.6
2 1 17.3
2 2 6.2
2 1 140.2
3 2 16.6
3 1 24.2
3 2 16.5
3 1 38.6
3 2 5.1
3 1 35.5
3 2 25.1
3 1 21.8
3 2 32.9
3 1 18.1
3 2 30.5
END DATA.
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
                if let Cell::Text(t) = cell {
                    carried[level] = t.clone();
                    for deeper in carried.iter_mut().skip(level + 1) {
                        deeper.clear();
                    }
                } else if let Cell::Number(n) = cell {
                    carried[level] = format!("{n}");
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

/// The `nth` table with this title and one of its cells.
fn cell<'t>(tables: &'t [Table], title: &str, nth: usize, row: &str, column: &str) -> &'t Cell {
    let table = tables
        .iter()
        .filter(|t| t.title == title)
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

type Described = (
    &'static str,
    [f64; 16],
    [f64; 7],
    [f64; 3],
    [f64; 3],
    [f64; 3],
    [f64; 3],
    [f64; 3],
);
type Normality = (&'static str, f64, f64, [f64; 2]);
type Boxed = (&'static str, [f64; 6], &'static [f64]);

/// `(table number, row prefix)` of a cell: the first table of each title is all cases.
fn prefix(cell: &str) -> (usize, String) {
    if cell == "all" {
        (0, "y".into())
    } else {
        (1, format!("y / {cell}"))
    }
}

#[test]
fn descriptives_percentiles_extremes_and_normality_of_every_cell() {
    let cells: [Described; 4] = [
        (
            "all",
            [
                17.92105263,
                3.631653472,
                11.79411323,
                24.04799203,
                14.51959064,
                11.5,
                501.1784637,
                22.38701552,
                4.9,
                140.2,
                135.3,
                14.35,
                4.636444737,
                0.3828183996,
                25.11713058,
                0.7497003516,
            ],
            [5.09, 5.4, 6.625, 11.5, 20.975, 33.16, 43.68],
            [6.7, 11.5, 20.7],
            [140.2, 38.6, 35.5],
            [27.0, 31.0, 33.0],
            [4.9, 5.1, 5.4],
            [5.0, 32.0, 12.0],
        ),
        (
            "1",
            [
                7.675,
                0.7122441061,
                6.395891623,
                8.954108377,
                7.527777778,
                6.85,
                6.0875,
                2.467285958,
                4.9,
                13.1,
                8.2,
                3.7,
                1.086328476,
                0.6373020055,
                0.5654361706,
                1.232246474,
            ],
            [4.9, 5.05, 5.825, 6.85, 9.525, 12.38, 13.1],
            [5.95, 6.85, 9.35],
            [13.1, 10.7, 9.7],
            [10.0, 6.0, 1.0],
            [4.9, 5.4, 5.7],
            [5.0, 12.0, 7.0],
        ),
        (
            "2",
            [
                21.6,
                8.624555199,
                6.409483511,
                36.79051649,
                15.91111111,
                12.0,
                1115.744286,
                33.40275865,
                5.4,
                140.2,
                134.8,
                14.0,
                3.642072233,
                0.5801193511,
                13.72265388,
                1.120897077,
            ],
            [5.4, 5.88, 6.7, 12.0, 20.7, 70.84, 140.2],
            [7.5, 12.0, 20.15],
            [140.2, 24.6, 20.7],
            [27.0, 21.0, 20.0],
            [5.4, 6.2, 6.4],
            [15.0, 26.0, 23.0],
        ),
        (
            "3",
            [
                24.08181818,
                2.978584721,
                18.68324917,
                29.48038719,
                24.32979798,
                24.2,
                97.59163636,
                9.878847927,
                5.1,
                38.6,
                33.5,
                16.3,
                -0.3067938612,
                0.6606874726,
                -0.1870994227,
                1.279415789,
            ],
            [5.1, 7.38, 16.6, 24.2, 32.9, 37.98, 38.6],
            [17.35, 24.2, 31.7],
            [38.6, 35.5, 32.9],
            [31.0, 33.0, 36.0],
            [5.1, 16.5, 16.6],
            [32.0, 30.0, 28.0],
        ),
    ];
    let t = run(
        "EXAMINE VARIABLES=y BY g /CINTERVAL=90 /STATISTICS=DESCRIPTIVES EXTREME(3) /PERCENTILES /PLOT=NPPLOT.",
    );
    for (name, d, pct, hinges, high, high_cases, low, low_cases) in cells {
        let (nth, key) = prefix(name);
        let at = |row: &str, column: &str| {
            value(&t, "Descriptives", nth, &format!("{key} / {row}"), column)
        };
        let interval = "90% Confidence Interval for Mean";
        near(at("Mean", "Statistic"), d[0], name);
        near(at("Mean", "Std. Error"), d[1], name);
        near(
            at(&format!("{interval} / Lower Bound"), "Statistic"),
            d[2],
            name,
        );
        near(
            at(&format!("{interval} / Upper Bound"), "Statistic"),
            d[3],
            name,
        );
        near(at("5% Trimmed Mean", "Statistic"), d[4], name);
        near(at("Median", "Statistic"), d[5], name);
        near(at("Variance", "Statistic"), d[6], name);
        near(at("Std. Deviation", "Statistic"), d[7], name);
        near(at("Minimum", "Statistic"), d[8], name);
        near(at("Maximum", "Statistic"), d[9], name);
        near(at("Range", "Statistic"), d[10], name);
        near(at("Interquartile Range", "Statistic"), d[11], name);
        near(at("Skewness", "Statistic"), d[12], name);
        near(at("Skewness", "Std. Error"), d[13], name);
        near(at("Kurtosis", "Statistic"), d[14], name);
        near(at("Kurtosis", "Std. Error"), d[15], name);
        for (i, p) in ["5", "10", "25", "50", "75", "90", "95"].iter().enumerate() {
            let column = format!("Percentiles / {p}");
            near(
                value(
                    &t,
                    "Percentiles",
                    nth,
                    &format!("{key} / Weighted Average at (W+1)p"),
                    &column,
                ),
                pct[i],
                name,
            );
        }
        for (i, p) in ["25", "50", "75"].iter().enumerate() {
            let column = format!("Percentiles / {p}");
            near(
                value(
                    &t,
                    "Percentiles",
                    nth,
                    &format!("{key} / Tukey's Hinges"),
                    &column,
                ),
                hinges[i],
                name,
            );
        }
        for k in 0..3 {
            let row = |side: &str| format!("{key} / {side} / {}", k + 1);
            near(
                value(&t, "Extreme Values", nth, &row("Highest"), "Value"),
                high[k],
                name,
            );
            near(
                value(&t, "Extreme Values", nth, &row("Highest"), "Case Number"),
                high_cases[k],
                name,
            );
            near(
                value(&t, "Extreme Values", nth, &row("Lowest"), "Value"),
                low[k],
                name,
            );
            near(
                value(&t, "Extreme Values", nth, &row("Lowest"), "Case Number"),
                low_cases[k],
                name,
            );
        }
    }
    // Kolmogorov-Smirnov and Shapiro-Wilk of every cell; the group of 12 lies between the
    // critical values of 0.2 and 0.1, the group of 11 under the one of 0.2
    let normality: [Normality; 4] = [
        (
            "all",
            0.2804065692,
            3.974784927e-08,
            [0.4984084388, 3.126170127e-10],
        ),
        (
            "1",
            0.2066147103,
            0.1718038981,
            [0.8987554783, 0.1528592192],
        ),
        (
            "2",
            0.3975512654,
            5.34235608e-07,
            [0.4531518876, 1.507577963e-06],
        ),
        ("3", 0.1304889998, f64::NAN, [0.9689178346, 0.8753995596]),
    ];
    for (name, d, p, shapiro) in normality {
        let (nth, key) = prefix(name);
        let at = |column: &str| cell(&t, "Tests of Normality", nth, &key, column);
        let number = |c: &Cell| match c {
            Cell::Number(n) => *n,
            other => panic!("{other:?}"),
        };
        near(number(at("Kolmogorov-Smirnov / Statistic")), d, name);
        near(number(at("Shapiro-Wilk / Statistic")), shapiro[0], name);
        near(number(at("Shapiro-Wilk / Sig.")), shapiro[1], name);
        match name {
            "3" => assert_eq!(
                at("Kolmogorov-Smirnov / Sig."),
                &Cell::Text(">0.200".into())
            ),
            "1" => near(
                number(at("Kolmogorov-Smirnov / Sig.")),
                0.1730494304531777,
                "interpolated Lilliefors",
            ),
            _ => near(number(at("Kolmogorov-Smirnov / Sig.")), p, name),
        }
    }
}

#[test]
fn levene_tests_with_and_without_a_power_transformation() {
    let cases: [(&str, [f64; 9]); 3] = [
        (
            "/PLOT=SPREADLEVEL",
            [
                2.024049511,
                0.1472965574,
                1.137375626,
                0.3322252083,
                1.137375626,
                14.71406146,
                0.3472939086,
                1.267971861,
                0.2940101601,
            ],
        ),
        (
            "/PLOT=SPREADLEVEL(0.5)",
            [
                2.176611555,
                0.1285391401,
                1.735591882,
                0.1911255087,
                1.735591882,
                17.04737459,
                0.2060316969,
                1.903225465,
                0.164199592,
            ],
        ),
        (
            "/PLOT=SPREADLEVEL(0)",
            [
                2.51396526,
                0.09546317484,
                2.237565113,
                0.1217666272,
                2.237565113,
                25.10346079,
                0.1275751152,
                2.36145104,
                0.1091374509,
            ],
        ),
    ];
    for (plot, want) in cases {
        let t = run(&format!(
            "EXAMINE VARIABLES=y BY g /STATISTICS=NONE /PERCENTILES=NONE {plot}."
        ));
        let at = |row: &str, c: &str| {
            value(
                &t,
                "Test of Homogeneity of Variance",
                0,
                &format!("y / {row}"),
                c,
            )
        };
        near(at("Based on Mean", "Levene Statistic"), want[0], plot);
        near(at("Based on Mean", "Sig."), want[1], plot);
        near(at("Based on Median", "Levene Statistic"), want[2], plot);
        near(at("Based on Median", "Sig."), want[3], plot);
        near(
            at("Based on Median and with adjusted df", "Levene Statistic"),
            want[4],
            plot,
        );
        near(
            at("Based on Median and with adjusted df", "df2"),
            want[5],
            plot,
        );
        near(
            at("Based on Median and with adjusted df", "Sig."),
            want[6],
            plot,
        );
        near(
            at("Based on trimmed mean", "Levene Statistic"),
            want[7],
            plot,
        );
        near(at("Based on trimmed mean", "Sig."), want[8], plot);
        assert_eq!(at("Based on Mean", "df1"), 2.0);
        assert_eq!(at("Based on Mean", "df2"), 35.0);
    }
}

fn chart(syntax: &str, shape: &str) -> serde_json::Value {
    let (mut engine, _) = engine_run(syntax);
    engine
        .take_charts()
        .iter()
        .map(|c| serde_json::to_value(c).expect("a chart"))
        .find(|c| c["plot"]["shape"] == shape)
        .unwrap_or_else(|| panic!("no {shape} chart"))
}

#[test]
fn spread_versus_level_slope_intercept_and_points() {
    // slope, power (1 - slope), intercept, the three levels, the three spreads
    let cases: [(&str, [f64; 9]); 3] = [
        (
            "/PLOT=SPREADLEVEL",
            [
                1.135455744,
                -0.1354557443,
                -0.6286026197,
                1.924248652,
                2.48490665,
                3.186352633,
                1.30833282,
                2.63905733,
                2.791165108,
            ],
        ),
        (
            "/PLOT=SPREADLEVEL(0.5)",
            [
                0.360344072,
                0.639655928,
                0.1105222653,
                2.617233035,
                3.464101615,
                4.91934955,
                0.6727662875,
                1.961289445,
                1.661542402,
            ],
        ),
        (
            "/PLOT=SPREADLEVEL(0)",
            [
                0.1199373067,
                0.8800626933,
                0.4643427237,
                1.924222012,
                2.48490665,
                3.186352633,
                0.4919136043,
                1.128026174,
                0.6840699624,
            ],
        ),
    ];
    for (plot, want) in cases {
        let c = chart(
            &format!("EXAMINE VARIABLES=y BY g /STATISTICS=NONE /PERCENTILES=NONE {plot}."),
            "points",
        );
        let number = |v: &serde_json::Value| v.as_f64().expect("a number");
        let line = &c["plot"]["lines"][0];
        near(number(&line["slope"]), want[0], plot);
        near(number(&line["intercept"]), want[2], plot);
        let points = c["plot"]["groups"][0]["points"].as_array().expect("points");
        for (i, p) in points.iter().enumerate() {
            near(number(&p["x"]), want[3 + i], &format!("{plot} level {i}"));
            near(number(&p["y"]), want[6 + i], &format!("{plot} spread {i}"));
        }
        if plot == "/PLOT=SPREADLEVEL" {
            let note = c["footnote"].as_str().expect("a footnote");
            assert!(
                note.contains(&format!("power for transformation = {:.3}", want[1])),
                "{note}"
            );
        }
    }
}

#[test]
fn boxplot_statistics_of_every_cell() {
    let cells: [Boxed; 4] = [
        (
            "all",
            [38.0, 4.9, 6.7, 11.5, 20.7, 38.6],
            &[0.0, 1.0, 140.2, 27.0],
        ),
        ("1", [12.0, 4.9, 5.95, 6.85, 9.35, 13.1], &[0.0, 0.0]),
        (
            "2",
            [15.0, 5.4, 7.5, 12.0, 20.15, 24.6],
            &[0.0, 1.0, 140.2, 27.0],
        ),
        ("3", [11.0, 5.1, 17.35, 24.2, 31.7, 38.6], &[0.0, 0.0]),
    ];
    let (mut engine, _) =
        engine_run("EXAMINE VARIABLES=y BY g /STATISTICS=NONE /PERCENTILES=NONE /PLOT=BOXPLOT.");
    let charts: Vec<serde_json::Value> = engine
        .take_charts()
        .iter()
        .map(|c| serde_json::to_value(c).expect("a chart"))
        .collect();
    let number = |v: &serde_json::Value| v.as_f64().expect("a number");
    for (name, b, flags) in cells {
        // the first chart holds all cases, the second the three groups
        let (chart, index) = if name == "all" {
            (&charts[0], 0)
        } else {
            (&charts[1], name.parse::<usize>().unwrap() - 1)
        };
        let boxed = &chart["plot"]["boxes"][index];
        near(number(&boxed["n"]), b[0], name);
        near(number(&boxed["lower_whisker"]), b[1], name);
        near(number(&boxed["lower_hinge"]), b[2], name);
        near(number(&boxed["median"]), b[3], name);
        near(number(&boxed["upper_hinge"]), b[4], name);
        near(number(&boxed["upper_whisker"]), b[5], name);
        assert_eq!(
            boxed["outliers"].as_array().unwrap().len() as f64,
            flags[0],
            "{name} outliers"
        );
        let extremes = boxed["extremes"].as_array().unwrap();
        assert_eq!(extremes.len() as f64, flags[1], "{name} extremes");
        if flags[1] > 0.0 {
            near(number(&extremes[0]["value"]), flags[2], name);
            assert_eq!(extremes[0]["case"], format!("{}", flags[3]));
        }
    }
}

#[test]
fn two_factors_cross_into_cells() {
    let t = run(
        "EXAMINE VARIABLES=y BY g BY h /NOTOTAL /STATISTICS=DESCRIPTIVES /PERCENTILES=NONE /PLOT=NONE.",
    );
    let cells: [(u32, u32, [f64; 4]); 6] = [
        (1, 1, [6.0, 6.7, 1.645600194, 6.5]),
        (1, 2, [6.0, 8.65, 2.898792852, 8.25]),
        (2, 1, [8.0, 29.225, 45.34763342, 14.65]),
        (2, 2, [7.0, 12.88571429, 6.176144583, 11.0]),
        (3, 1, [5.0, 27.64, 8.928213707, 24.2]),
        (3, 2, [6.0, 21.11666667, 10.40200301, 20.85]),
    ];
    for (g, h, want) in cells {
        let key = format!("y / {g} / {h}");
        near(
            value(&t, "Case Processing Summary", 0, &key, "Cases / Valid / N"),
            want[0],
            &key,
        );
        near(
            value(&t, "Descriptives", 0, &format!("{key} / Mean"), "Statistic"),
            want[1],
            &key,
        );
        near(
            value(
                &t,
                "Descriptives",
                0,
                &format!("{key} / Std. Deviation"),
                "Statistic",
            ),
            want[2],
            &key,
        );
        near(
            value(
                &t,
                "Descriptives",
                0,
                &format!("{key} / Median"),
                "Statistic",
            ),
            want[3],
            &key,
        );
        near(
            value(
                &t,
                "Descriptives",
                0,
                &format!("{key} / Mean"),
                "Std. Error",
            ),
            want[2] / want[0].sqrt(),
            &key,
        );
    }
}
