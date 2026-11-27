//! RESPONSE RATE against R on 70 invented cases (zones with a gap, a zone whose denominator sums
//! to 0 and one whose sum is negative, a missing denominator, a missing zone, a user-missing
//! numerator, a zero weight, frequency weights): the sums of the numerator and the denominator in
//! each zone, the rate, the cumulative sums and the cumulative rate, with the rows colour-coded
//! by MINRATE and MAXCOUNT (as a count and as a percent of the denominator total), with and
//! without weights. R writes the rules of the command out: a case missing on any of the three
//! variables is left out, a zone whose denominator sum is 0 or negative is left out of
//! everything, and the weights multiply both values. Scripts in lab/oracles/rr-r.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /zone buys mailed w (4F8.2).
BEGIN DATA.
12 1 -5 3
12 1 -5 3
8 9 0 3
8 . 0 1
2 12 39 1
8 7 0 2
6 18 51 1
2 10 46 2
5 10 . 2
10 18 38 2
5 8 26 1
6 10 29 3
3 13 47 2
10 8 16 2
10 13 26 1
6 13 56 1
4 10 36 1
4 15 49 3
8 . 0 1
3 20 52 1
1 2 14 3
5 99 29 1
1 4 14 1
5 16 48 1
5 10 25 2
3 8 45 3
2 16 60 1
2 9 40 1
4 13 41 1
. 10 21 3
4 4 25 2
8 16 0 2
1 6 58 1
6 8 15 3
6 8 14 2
10 15 36 1
1 3 24 3
10 14 31 2
8 26 0 3
3 8 40 1
6 5 12 3
5 11 31 3
2 4 30 3
1 9 47 0
4 15 40 1
4 6 33 3
8 14 0 1
4 16 55 3
3 16 53 1
3 7 46 1
3 8 30 1
1 5 20 2
10 19 35 2
2 0 12 1
3 8 32 1
2 6 29 1
8 19 0 1
1 7 36 2
6 15 41 1
1 6 14 1
8 4 0 1
8 6 0 1
3 7 32 3
1 12 60 1
4 7 46 1
5 12 37 1
3 4 19 2
2 3 36 3
4 13 57 2
5 6 15 2
END DATA.
MISSING VALUES buys (99).
";

fn run(syntax: &str) -> Table {
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
        .into_iter()
        .find(|t| t.title == "Response Rates")
        .expect("the table")
}

/// Each row: the zone, then the numerator sum, the denominator sum, the rate, the cumulative
/// numerator, the cumulative denominator and the cumulative rate, and whether the row is green.
fn check(table: &Table, coloured: bool, expected: &[(&str, [f64; 6], bool)]) {
    assert_eq!(table.rows.len(), expected.len(), "the zones kept");
    for (i, (zone, values, green)) in expected.iter().enumerate() {
        let row = &table.rows[i];
        let name = match &row[0] {
            Cell::Number(n) => format!("{n}"),
            Cell::Text(t) => t.trim().to_string(),
            Cell::Empty => String::new(),
        };
        assert_eq!(&name, zone);
        for (k, want) in values.iter().enumerate() {
            let Cell::Number(n) = &row[k + 1] else {
                panic!("zone {zone}, column {k}: {:?}", row[k + 1]);
            };
            assert!(
                (n - want).abs() <= 1e-8 + 1e-7 * want.abs(),
                "zone {zone}, column {k}: {n} is not {want}"
            );
        }
        let fill = table.decoration(i, 0).and_then(|d| d.fill.as_deref());
        let colour = match fill {
            Some("#c6efce") => Some(true),
            Some("#ffc7ce") => Some(false),
            _ => None,
        };
        // a table without limits has no colours
        if coloured {
            assert_eq!(colour, Some(*green), "zone {zone}");
        } else {
            assert_eq!(colour, None, "zone {zone}");
        }
    }
}

#[test]
fn sums_rates_and_cumulative_rates_agree_with_r() {
    let t = run("RESPONSE RATE /VARIABLES NUMERATOR=buys DENOMINATOR=mailed GROUP=zone.");
    check(
        &t,
        false,
        &[
            (
                "1",
                [
                    54.0,
                    287.0,
                    18.81533101,
                    53.999999998700005,
                    287.0,
                    18.81533101,
                ],
                true,
            ),
            (
                "2",
                [
                    60.0,
                    292.0,
                    20.54794521,
                    113.99999999430001,
                    579.0,
                    19.68911917,
                ],
                true,
            ),
            (
                "3",
                [99.0, 396.0, 25.0, 213.0000000375, 975.0, 21.84615385],
                true,
            ),
            (
                "4",
                [
                    99.0,
                    382.0,
                    25.91623037,
                    311.9999999516,
                    1357.0,
                    22.99189388,
                ],
                true,
            ),
            (
                "5",
                [
                    63.0,
                    182.0,
                    34.61538462,
                    374.99999992470003,
                    1539.0,
                    24.36647173,
                ],
                true,
            ),
            (
                "6",
                [
                    77.0,
                    218.0,
                    35.32110092,
                    451.99999993750004,
                    1757.0,
                    25.72566875,
                ],
                true,
            ),
            (
                "10",
                [
                    87.0,
                    182.0,
                    47.8021978,
                    539.0000000965999,
                    1939.0,
                    27.79783394,
                ],
                true,
            ),
        ],
    );
}

#[test]
fn weighted_sums_and_rates_agree_with_r() {
    let t =
        run("WEIGHT BY w.\nRESPONSE RATE /VARIABLES NUMERATOR=buys DENOMINATOR=mailed GROUP=zone.");
    check(
        &t,
        false,
        &[
            (
                "1",
                [
                    67.0,
                    372.0,
                    18.01075269,
                    67.00000000680001,
                    372.0,
                    18.01075269,
                ],
                true,
            ),
            (
                "2",
                [84.0, 470.0, 17.87234043, 151.0000000298, 842.0, 17.93349169],
                true,
            ),
            (
                "3",
                [
                    146.0,
                    616.0,
                    23.7012987,
                    296.9999999946,
                    1458.0,
                    20.37037037,
                ],
                true,
            ),
            (
                "4",
                [
                    190.0,
                    738.0,
                    25.74525745,
                    486.9999999648,
                    2196.0,
                    22.17668488,
                ],
                true,
            ),
            (
                "5",
                [
                    101.0,
                    284.0,
                    35.56338028,
                    588.000000016,
                    2480.0,
                    23.70967742,
                ],
                true,
            ),
            (
                "6",
                [
                    131.0,
                    344.0,
                    38.08139535,
                    718.9999999056,
                    2824.0,
                    25.46033994,
                ],
                true,
            ),
            (
                "10",
                [
                    146.0,
                    302.0,
                    48.34437086,
                    864.9999998897999,
                    3126.0,
                    27.67114523,
                ],
                true,
            ),
        ],
    );
}

#[test]
fn minimum_cumulative_rate_colours_agree_with_r() {
    let t = run(
        "RESPONSE RATE /VARIABLES NUMERATOR=buys DENOMINATOR=mailed GROUP=zone /MINRATE RATE=20.",
    );
    check(
        &t,
        true,
        &[
            (
                "1",
                [
                    54.0,
                    287.0,
                    18.81533101,
                    53.999999998700005,
                    287.0,
                    18.81533101,
                ],
                false,
            ),
            (
                "2",
                [
                    60.0,
                    292.0,
                    20.54794521,
                    113.99999999430001,
                    579.0,
                    19.68911917,
                ],
                false,
            ),
            (
                "3",
                [99.0, 396.0, 25.0, 213.0000000375, 975.0, 21.84615385],
                true,
            ),
            (
                "4",
                [
                    99.0,
                    382.0,
                    25.91623037,
                    311.9999999516,
                    1357.0,
                    22.99189388,
                ],
                true,
            ),
            (
                "5",
                [
                    63.0,
                    182.0,
                    34.61538462,
                    374.99999992470003,
                    1539.0,
                    24.36647173,
                ],
                true,
            ),
            (
                "6",
                [
                    77.0,
                    218.0,
                    35.32110092,
                    451.99999993750004,
                    1757.0,
                    25.72566875,
                ],
                true,
            ),
            (
                "10",
                [
                    87.0,
                    182.0,
                    47.8021978,
                    539.0000000965999,
                    1939.0,
                    27.79783394,
                ],
                true,
            ),
        ],
    );
}

#[test]
fn maximum_count_colours_agree_with_r() {
    let t = run(
        "RESPONSE RATE /VARIABLES NUMERATOR=buys DENOMINATOR=mailed GROUP=zone /MAXCOUNT COUNT=800.",
    );
    check(
        &t,
        true,
        &[
            (
                "1",
                [
                    54.0,
                    287.0,
                    18.81533101,
                    53.999999998700005,
                    287.0,
                    18.81533101,
                ],
                true,
            ),
            (
                "2",
                [
                    60.0,
                    292.0,
                    20.54794521,
                    113.99999999430001,
                    579.0,
                    19.68911917,
                ],
                true,
            ),
            (
                "3",
                [99.0, 396.0, 25.0, 213.0000000375, 975.0, 21.84615385],
                false,
            ),
            (
                "4",
                [
                    99.0,
                    382.0,
                    25.91623037,
                    311.9999999516,
                    1357.0,
                    22.99189388,
                ],
                false,
            ),
            (
                "5",
                [
                    63.0,
                    182.0,
                    34.61538462,
                    374.99999992470003,
                    1539.0,
                    24.36647173,
                ],
                false,
            ),
            (
                "6",
                [
                    77.0,
                    218.0,
                    35.32110092,
                    451.99999993750004,
                    1757.0,
                    25.72566875,
                ],
                false,
            ),
            (
                "10",
                [
                    87.0,
                    182.0,
                    47.8021978,
                    539.0000000965999,
                    1939.0,
                    27.79783394,
                ],
                false,
            ),
        ],
    );
}

#[test]
fn both_limits_colours_agree_with_r() {
    let t = run(
        "RESPONSE RATE /VARIABLES NUMERATOR=buys DENOMINATOR=mailed GROUP=zone /MINRATE RATE=20 /MAXCOUNT COUNT=800.",
    );
    check(
        &t,
        true,
        &[
            (
                "1",
                [
                    54.0,
                    287.0,
                    18.81533101,
                    53.999999998700005,
                    287.0,
                    18.81533101,
                ],
                false,
            ),
            (
                "2",
                [
                    60.0,
                    292.0,
                    20.54794521,
                    113.99999999430001,
                    579.0,
                    19.68911917,
                ],
                false,
            ),
            (
                "3",
                [99.0, 396.0, 25.0, 213.0000000375, 975.0, 21.84615385],
                false,
            ),
            (
                "4",
                [
                    99.0,
                    382.0,
                    25.91623037,
                    311.9999999516,
                    1357.0,
                    22.99189388,
                ],
                false,
            ),
            (
                "5",
                [
                    63.0,
                    182.0,
                    34.61538462,
                    374.99999992470003,
                    1539.0,
                    24.36647173,
                ],
                false,
            ),
            (
                "6",
                [
                    77.0,
                    218.0,
                    35.32110092,
                    451.99999993750004,
                    1757.0,
                    25.72566875,
                ],
                false,
            ),
            (
                "10",
                [
                    87.0,
                    182.0,
                    47.8021978,
                    539.0000000965999,
                    1939.0,
                    27.79783394,
                ],
                false,
            ),
        ],
    );
}

#[test]
fn maximum_count_as_a_percent_colours_agree_with_r() {
    let t = run(
        "RESPONSE RATE /VARIABLES NUMERATOR=buys DENOMINATOR=mailed GROUP=zone /MINRATE RATE=20 /MAXCOUNT PERCENT=80.",
    );
    check(
        &t,
        true,
        &[
            (
                "1",
                [
                    54.0,
                    287.0,
                    18.81533101,
                    53.999999998700005,
                    287.0,
                    18.81533101,
                ],
                false,
            ),
            (
                "2",
                [
                    60.0,
                    292.0,
                    20.54794521,
                    113.99999999430001,
                    579.0,
                    19.68911917,
                ],
                false,
            ),
            (
                "3",
                [99.0, 396.0, 25.0, 213.0000000375, 975.0, 21.84615385],
                true,
            ),
            (
                "4",
                [
                    99.0,
                    382.0,
                    25.91623037,
                    311.9999999516,
                    1357.0,
                    22.99189388,
                ],
                true,
            ),
            (
                "5",
                [
                    63.0,
                    182.0,
                    34.61538462,
                    374.99999992470003,
                    1539.0,
                    24.36647173,
                ],
                true,
            ),
            (
                "6",
                [
                    77.0,
                    218.0,
                    35.32110092,
                    451.99999993750004,
                    1757.0,
                    25.72566875,
                ],
                false,
            ),
            (
                "10",
                [
                    87.0,
                    182.0,
                    47.8021978,
                    539.0000000965999,
                    1939.0,
                    27.79783394,
                ],
                false,
            ),
        ],
    );
}

#[test]
fn weighted_limits_colours_agree_with_r() {
    let t = run(
        "WEIGHT BY w.\nRESPONSE RATE /VARIABLES NUMERATOR=buys DENOMINATOR=mailed GROUP=zone /MINRATE RATE=20 /MAXCOUNT COUNT=1500.",
    );
    check(
        &t,
        true,
        &[
            (
                "1",
                [
                    67.0,
                    372.0,
                    18.01075269,
                    67.00000000680001,
                    372.0,
                    18.01075269,
                ],
                false,
            ),
            (
                "2",
                [84.0, 470.0, 17.87234043, 151.0000000298, 842.0, 17.93349169],
                false,
            ),
            (
                "3",
                [
                    146.0,
                    616.0,
                    23.7012987,
                    296.9999999946,
                    1458.0,
                    20.37037037,
                ],
                true,
            ),
            (
                "4",
                [
                    190.0,
                    738.0,
                    25.74525745,
                    486.9999999648,
                    2196.0,
                    22.17668488,
                ],
                false,
            ),
            (
                "5",
                [
                    101.0,
                    284.0,
                    35.56338028,
                    588.000000016,
                    2480.0,
                    23.70967742,
                ],
                false,
            ),
            (
                "6",
                [
                    131.0,
                    344.0,
                    38.08139535,
                    718.9999999056,
                    2824.0,
                    25.46033994,
                ],
                false,
            ),
            (
                "10",
                [
                    146.0,
                    302.0,
                    48.34437086,
                    864.9999998897999,
                    3126.0,
                    27.67114523,
                ],
                false,
            ),
        ],
    );
}
