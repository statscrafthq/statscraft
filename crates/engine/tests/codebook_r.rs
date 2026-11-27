//! CODEBOOK statistics against R on 80 invented cases: a nominal variable with a user-missing
//! code and system-missing values, two ordinal variables, two scale variables (one with a
//! user-missing code), and frequency weights with a zero weight. The counts and percents of the
//! valid and the missing values (percents of all the weighted cases, missing ones included), and
//! the mean, standard deviation and quartiles of the scale variables ((W + 1) p, as in
//! FREQUENCIES); a case with a zero weight is left out of everything. Scripts in
//! lab/oracles/codebook-r.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /q1 q2 age score sat w (6F8.2).
BEGIN DATA.
2 4 25 17.4 2 1
4 1 13 . 3 1
1 3 . 7.6 3 1
2 5 42 13.6 2 2
. 4 58 3.4 2 1
9 5 48 1.9 1 1
4 1 38 5.2 . 2
2 . 44 17.9 2 0
1 5 43 18.8 3 1
1 4 99 13.9 3 1
2 5 99 11.4 1 2
9 2 50 13.8 2 3
4 3 30 2.8 1 1
1 4 46 14.9 3 1
2 4 47 6.4 1 1
2 1 47 18.9 2 2
2 3 . 8.4 3 1
1 3 58 2 2 2
1 3 29 16.8 8 1
3 1 20 1.9 2 3
1 2 39 16.1 3 1
4 3 40 18.7 8 3
9 4 55 5.7 1 1
1 2 33 15.9 1 3
2 4 63 15.1 1 1
1 3 46 16.1 1 2
2 4 45 1.1 3 2
2 1 53 11.3 1 2
1 1 33 . 1 0
4 2 50 17.7 1 2
4 1 50 6.3 1 1
1 4 44 19.6 2 1
. 1 35 3 1 2
2 5 14 4.5 3 1
4 2 51 11.9 2 3
3 3 40 7.8 8 3
2 3 41 12.8 1 2
2 2 59 16.7 3 1
1 1 32 7.2 2 1
1 . 38 10.1 1 1
2 . 42 14.5 3 3
1 5 45 18.6 1 2
1 1 49 9.3 2 2
1 3 31 11.7 1 2
2 1 27 2.3 2 3
1 3 51 17.9 3 3
3 4 46 10.6 2 2
4 4 20 16.1 3 1
3 4 33 6.1 1 3
1 4 49 4 2 3
9 3 61 11.2 2 2
9 2 62 8.6 8 3
2 2 45 1.1 1 2
1 4 51 6.5 3 2
3 2 52 15.5 1 2
2 2 26 12.9 8 1
2 3 47 15.1 2 1
1 4 78 8.8 2 2
1 1 80 18.5 3 2
4 2 . 16.1 3 3
1 3 21 12.2 2 3
2 4 47 18.9 3 3
9 5 45 9.7 3 2
3 5 38 4 2 3
1 3 76 6.4 1 2
1 1 42 16.7 3 1
2 3 41 9.1 1 0
9 3 26 12.8 8 3
2 3 63 14.2 2 1
1 3 34 13.7 3 2
4 1 49 3.2 2 1
1 3 67 8.7 2 2
3 5 46 9.9 1 3
4 2 41 6 2 2
2 5 43 11 2 0
3 3 61 15.5 3 1
9 5 55 5.5 3 2
1 4 25 13.9 1 1
9 2 54 12.5 1 2
3 4 44 4.7 2 1
END DATA.
VARIABLE LEVEL q1 (NOMINAL) /q2 sat (ORDINAL) /age score w (SCALE).
MISSING VALUES q1 (9) /age (99) /sat (8).
VALUE LABELS q1 1 'one' 2 'two' 3 'three' 4 'four' 9 'refused' /q2 1 'a' 2 'b' 3 'c' 4 'd' 5 'e' /sat 1 'low' 2 'mid' 3 'high' 8 'unsure' /age 99 'unknown'.
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

/// Each expectation: the table (one per variable, titled by its name), the row, the column and the value.
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
fn counts_percents_and_statistics_unweighted_agree_with_r() {
    let t = run(
        "CODEBOOK q1 q2 sat age score /VARINFO=LABEL /STATISTICS=COUNT PERCENT MEAN STDDEV QUARTILES.",
    );
    check(
        &t,
        &[
            ("q1", "Valid Values / 1", "Count", 27.0),
            ("q1", "Valid Values / 1", "Percent", 33.75),
            ("q1", "Valid Values / 2", "Count", 22.0),
            ("q1", "Valid Values / 2", "Percent", 27.5),
            ("q1", "Valid Values / 3", "Count", 9.0),
            ("q1", "Valid Values / 3", "Percent", 11.25),
            ("q1", "Valid Values / 4", "Count", 11.0),
            ("q1", "Valid Values / 4", "Percent", 13.75),
            ("q1", "Missing Values / 9", "Count", 9.0),
            ("q1", "Missing Values / 9", "Percent", 11.25),
            ("q1", "Missing Values / System", "Count", 2.0),
            ("q1", "Missing Values / System", "Percent", 2.5),
            ("q2", "Valid Values / 1", "Count", 14.0),
            ("q2", "Valid Values / 1", "Percent", 17.5),
            ("q2", "Valid Values / 2", "Count", 13.0),
            ("q2", "Valid Values / 2", "Percent", 16.25),
            ("q2", "Valid Values / 3", "Count", 21.0),
            ("q2", "Valid Values / 3", "Percent", 26.25),
            ("q2", "Valid Values / 4", "Count", 18.0),
            ("q2", "Valid Values / 4", "Percent", 22.5),
            ("q2", "Valid Values / 5", "Count", 11.0),
            ("q2", "Valid Values / 5", "Percent", 13.75),
            ("q2", "Missing Values / System", "Count", 3.0),
            ("q2", "Missing Values / System", "Percent", 3.75),
            ("sat", "Valid Values / 1", "Count", 25.0),
            ("sat", "Valid Values / 1", "Percent", 31.25),
            ("sat", "Valid Values / 2", "Count", 26.0),
            ("sat", "Valid Values / 2", "Percent", 32.5),
            ("sat", "Valid Values / 3", "Count", 22.0),
            ("sat", "Valid Values / 3", "Percent", 27.5),
            ("sat", "Missing Values / 8", "Count", 6.0),
            ("sat", "Missing Values / 8", "Percent", 7.5),
            ("sat", "Missing Values / System", "Count", 1.0),
            ("sat", "Missing Values / System", "Percent", 1.25),
            (
                "age",
                "Central Tendency and Dispersion / Mean",
                "Value",
                44.16,
            ),
            (
                "age",
                "Central Tendency and Dispersion / Standard Deviation",
                "Value",
                13.66494825,
            ),
            (
                "age",
                "Central Tendency and Dispersion / Percentile 25",
                "Value",
                35.0,
            ),
            (
                "age",
                "Central Tendency and Dispersion / Percentile 50",
                "Value",
                45.0,
            ),
            (
                "age",
                "Central Tendency and Dispersion / Percentile 75",
                "Value",
                51.0,
            ),
            ("age", "Missing Values / 99", "Count", 2.0),
            ("age", "Missing Values / 99", "Percent", 2.5),
            ("age", "Missing Values / System", "Count", 3.0),
            ("age", "Missing Values / System", "Percent", 3.75),
            (
                "score",
                "Central Tendency and Dispersion / Mean",
                "Value",
                10.87948718,
            ),
            (
                "score",
                "Central Tendency and Dispersion / Standard Deviation",
                "Value",
                5.403759129,
            ),
            (
                "score",
                "Central Tendency and Dispersion / Percentile 25",
                "Value",
                6.25,
            ),
            (
                "score",
                "Central Tendency and Dispersion / Percentile 50",
                "Value",
                11.35,
            ),
            (
                "score",
                "Central Tendency and Dispersion / Percentile 75",
                "Value",
                15.6,
            ),
            ("score", "Missing Values / System", "Count", 2.0),
            ("score", "Missing Values / System", "Percent", 2.5),
        ],
    );
}

#[test]
fn counts_percents_and_statistics_weighted_agree_with_r() {
    let t = run(
        "WEIGHT BY w.\nCODEBOOK q1 q2 sat age score /VARINFO=LABEL /STATISTICS=COUNT PERCENT MEAN STDDEV QUARTILES.",
    );
    check(
        &t,
        &[
            ("q1", "Valid Values / 1", "Count", 45.0),
            ("q1", "Valid Values / 1", "Percent", 32.14285714),
            ("q1", "Valid Values / 2", "Count", 32.0),
            ("q1", "Valid Values / 2", "Percent", 22.85714286),
            ("q1", "Valid Values / 3", "Count", 21.0),
            ("q1", "Valid Values / 3", "Percent", 15.0),
            ("q1", "Valid Values / 4", "Count", 20.0),
            ("q1", "Valid Values / 4", "Percent", 14.28571429),
            ("q1", "Missing Values / 9", "Count", 19.0),
            ("q1", "Missing Values / 9", "Percent", 13.57142857),
            ("q1", "Missing Values / System", "Count", 3.0),
            ("q1", "Missing Values / System", "Percent", 2.142857143),
            ("q2", "Valid Values / 1", "Count", 23.0),
            ("q2", "Valid Values / 1", "Percent", 16.42857143),
            ("q2", "Valid Values / 2", "Count", 28.0),
            ("q2", "Valid Values / 2", "Percent", 20.0),
            ("q2", "Valid Values / 3", "Count", 38.0),
            ("q2", "Valid Values / 3", "Percent", 27.14285714),
            ("q2", "Valid Values / 4", "Count", 28.0),
            ("q2", "Valid Values / 4", "Percent", 20.0),
            ("q2", "Valid Values / 5", "Count", 19.0),
            ("q2", "Valid Values / 5", "Percent", 13.57142857),
            ("q2", "Missing Values / System", "Count", 4.0),
            ("q2", "Missing Values / System", "Percent", 2.857142857),
            ("sat", "Valid Values / 1", "Count", 41.0),
            ("sat", "Valid Values / 1", "Percent", 29.28571429),
            ("sat", "Valid Values / 2", "Count", 47.0),
            ("sat", "Valid Values / 2", "Percent", 33.57142857),
            ("sat", "Valid Values / 3", "Count", 36.0),
            ("sat", "Valid Values / 3", "Percent", 25.71428571),
            ("sat", "Missing Values / 8", "Count", 14.0),
            ("sat", "Missing Values / 8", "Percent", 10.0),
            ("sat", "Missing Values / System", "Count", 2.0),
            ("sat", "Missing Values / System", "Percent", 1.428571429),
            (
                "age",
                "Central Tendency and Dispersion / Mean",
                "Value",
                44.45454545,
            ),
            (
                "age",
                "Central Tendency and Dispersion / Standard Deviation",
                "Value",
                13.5671154,
            ),
            (
                "age",
                "Central Tendency and Dispersion / Percentile 25",
                "Value",
                35.75,
            ),
            (
                "age",
                "Central Tendency and Dispersion / Percentile 50",
                "Value",
                45.0,
            ),
            (
                "age",
                "Central Tendency and Dispersion / Percentile 75",
                "Value",
                51.0,
            ),
            ("age", "Missing Values / 99", "Count", 3.0),
            ("age", "Missing Values / 99", "Percent", 2.142857143),
            ("age", "Missing Values / System", "Count", 5.0),
            ("age", "Missing Values / System", "Percent", 3.571428571),
            (
                "score",
                "Central Tendency and Dispersion / Mean",
                "Value",
                10.7381295,
            ),
            (
                "score",
                "Central Tendency and Dispersion / Standard Deviation",
                "Value",
                5.416137932,
            ),
            (
                "score",
                "Central Tendency and Dispersion / Percentile 25",
                "Value",
                6.1,
            ),
            (
                "score",
                "Central Tendency and Dispersion / Percentile 50",
                "Value",
                11.4,
            ),
            (
                "score",
                "Central Tendency and Dispersion / Percentile 75",
                "Value",
                15.5,
            ),
            ("score", "Missing Values / System", "Count", 1.0),
            ("score", "Missing Values / System", "Percent", 0.7142857143),
        ],
    );
}
