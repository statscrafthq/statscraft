//! Edge cases and numerical stability of FACTOR (principal components of the correlation matrix,
//! one component kept) against R on ten invented cases: a variable that is twice another (a
//! singular correlation matrix: a zero eigenvalue and no KMO or Bartlett), two variables, a zero
//! weight and a weight of two (cases repeated as often as their weight), a constant variable
//! (an error: no variance), and two variables with 1000000000 added (the same solution as the
//! small values). The eigenvalues, the share of variance, the communalities, the loadings (their
//! signs are arbitrary, so the absolute value), the KMO measure and Bartlett's test of
//! sphericity (the KMO of R's psych package and the determinant formula). Scripts in lab/oracles/edge-fa.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /a b c d k w (6F14.2).
BEGIN DATA.
3 2 4 1 5 1
5 4 3 2 5 2
4 4 6 2 5 1
9 8 7 3 5 0
2 3 2 4 5 1
8 6 7 3 5 1
6 7 5 5 5 1
6 7 8 6 5 1
5 4 4 5 5 1
7 8 8 6 5 1
END DATA.
COMPUTE a2=2*a.
";

const SHIFTED: &str = "DATA LIST FREE /a9 b9 c d k w (6F14.2).
BEGIN DATA.
1000000003.00 1000000002.00 4 1 5 1
1000000005.00 1000000004.00 3 2 5 2
1000000004.00 1000000004.00 6 2 5 1
1000000009.00 1000000008.00 7 3 5 0
1000000002.00 1000000003.00 2 4 5 1
1000000008.00 1000000006.00 7 3 5 1
1000000006.00 1000000007.00 5 5 5 1
1000000006.00 1000000007.00 8 6 5 1
1000000005.00 1000000004.00 4 5 5 1
1000000007.00 1000000008.00 8 6 5 1
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    run_on(DATA, syntax)
}

fn run_on(data: &str, syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{data}{syntax}"));
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

/// The first cell of each row (carried from the row above when empty), with the second when it
/// is text (a row of two labels).
fn row_keys(table: &Table) -> Vec<String> {
    let text = |c: &Cell| match c {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => format!("{n}"),
        Cell::Empty => String::new(),
    };
    let mut carried = String::new();
    table
        .rows
        .iter()
        .map(|row| {
            let first = text(&row[0]);
            if !first.is_empty() {
                carried = first;
            }
            match row.get(1) {
                Some(Cell::Text(second)) if !second.is_empty() => format!("{carried} / {second}"),
                _ => carried.clone(),
            }
        })
        .collect()
}

/// Each expectation: the table title, the row, the column and the value (NaN: an empty cell).
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
        let absolute = column.ends_with("(absolute)");
        let column = column.trim_end_matches(" (absolute)");
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
        let n = if absolute { &n.abs() } else { n };
        assert!(
            (n - want).abs() <= 1e-7 + 1e-6 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn four_variables_with_kmo_and_bartlett() {
    let t = run("FACTOR VARIABLES=a b c d /PRINT=INITIAL EXTRACTION KMO /CRITERIA=FACTORS(1).");
    check(
        &t,
        &[
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Total",
                2.89553873474,
            ),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / % of Variance",
                72.3884683685,
            ),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Cumulative %",
                72.3884683685,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Total",
                0.783180424687,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / % of Variance",
                19.5795106172,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Cumulative %",
                91.9679789857,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / Total",
                0.263321820643,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / % of Variance",
                6.58304551608,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / Cumulative %",
                98.5510245017,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / Total",
                0.0579590199304,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / % of Variance",
                1.44897549826,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / Cumulative %",
                100.0,
            ),
            ("Communalities", "a", "Initial", 1.0),
            ("Communalities", "a", "Extraction", 0.77856344391),
            (
                "Component Matrix",
                "a",
                "Component / 1 (absolute)",
                0.882362422086,
            ),
            ("Communalities", "b", "Initial", 1.0),
            ("Communalities", "b", "Extraction", 0.945981331501),
            (
                "Component Matrix",
                "b",
                "Component / 1 (absolute)",
                0.972615716252,
            ),
            ("Communalities", "c", "Initial", 1.0),
            ("Communalities", "c", "Extraction", 0.783274942276),
            (
                "Component Matrix",
                "c",
                "Component / 1 (absolute)",
                0.885028215525,
            ),
            ("Communalities", "d", "Initial", 1.0),
            ("Communalities", "d", "Extraction", 0.387719017053),
            (
                "Component Matrix",
                "d",
                "Component / 1 (absolute)",
                0.622670873779,
            ),
            (
                "KMO and Bartlett's Test",
                "Kaiser-Meyer-Olkin Measure of Sampling Adequacy.",
                "Value",
                0.597020395023,
            ),
            (
                "KMO and Bartlett's Test",
                "Bartlett's Test of Sphericity / Approx. Chi-Square",
                "Value",
                22.9847258033,
            ),
            (
                "KMO and Bartlett's Test",
                "Bartlett's Test of Sphericity / df",
                "Value",
                6.0,
            ),
            (
                "KMO and Bartlett's Test",
                "Bartlett's Test of Sphericity / Sig.",
                "Value",
                0.000801610508707,
            ),
        ],
    );
}

#[test]
fn a_variable_that_is_twice_another() {
    let t = run("FACTOR VARIABLES=a a2 b c /PRINT=INITIAL EXTRACTION /CRITERIA=FACTORS(1).");
    check(
        &t,
        &[
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Total",
                3.51404770271,
            ),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / % of Variance",
                87.8511925677,
            ),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Cumulative %",
                87.8511925677,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Total",
                0.331277949393,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / % of Variance",
                8.28194873482,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Cumulative %",
                96.1331413025,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / Total",
                0.1546743479,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / % of Variance",
                3.8668586975,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / Cumulative %",
                100.0,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / Total",
                0.0,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / % of Variance",
                0.0,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / Cumulative %",
                100.0,
            ),
            ("Communalities", "a", "Initial", 1.0),
            ("Communalities", "a", "Extraction", 0.934687029741),
            (
                "Component Matrix",
                "a",
                "Component / 1 (absolute)",
                0.966792133678,
            ),
            ("Communalities", "a2", "Initial", 1.0),
            ("Communalities", "a2", "Extraction", 0.934687029741),
            (
                "Component Matrix",
                "a2",
                "Component / 1 (absolute)",
                0.966792133678,
            ),
            ("Communalities", "b", "Initial", 1.0),
            ("Communalities", "b", "Extraction", 0.883101923482),
            (
                "Component Matrix",
                "b",
                "Component / 1 (absolute)",
                0.939735028336,
            ),
            ("Communalities", "c", "Initial", 1.0),
            ("Communalities", "c", "Extraction", 0.761571719743),
            (
                "Component Matrix",
                "c",
                "Component / 1 (absolute)",
                0.872680766227,
            ),
        ],
    );
}

#[test]
fn two_variables() {
    let t = run("FACTOR VARIABLES=a b /PRINT=INITIAL EXTRACTION /CRITERIA=FACTORS(1).");
    check(
        &t,
        &[
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Total",
                1.86289380591,
            ),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / % of Variance",
                93.1446902954,
            ),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Cumulative %",
                93.1446902954,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Total",
                0.137106194092,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / % of Variance",
                6.85530970462,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Cumulative %",
                100.0,
            ),
            ("Communalities", "a", "Initial", 1.0),
            ("Communalities", "a", "Extraction", 0.931446902954),
            (
                "Component Matrix",
                "a",
                "Component / 1 (absolute)",
                0.965114968775,
            ),
            ("Communalities", "b", "Initial", 1.0),
            ("Communalities", "b", "Extraction", 0.931446902954),
            (
                "Component Matrix",
                "b",
                "Component / 1 (absolute)",
                0.965114968775,
            ),
        ],
    );
}

#[test]
fn zero_weight_and_weight_of_two() {
    let t = run("WEIGHT BY w.
FACTOR VARIABLES=a b c d /PRINT=INITIAL EXTRACTION /CRITERIA=FACTORS(1).");
    check(
        &t,
        &[
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Total",
                3.02122525725,
            ),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / % of Variance",
                75.5306314312,
            ),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Cumulative %",
                75.5306314312,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Total",
                0.627103685408,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / % of Variance",
                15.6775921352,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Cumulative %",
                91.2082235664,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / Total",
                0.28546828281,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / % of Variance",
                7.13670707024,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / Cumulative %",
                98.3449306366,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / Total",
                0.0662027745362,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / % of Variance",
                1.6550693634,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / Cumulative %",
                100.0,
            ),
            ("Communalities", "a", "Initial", 1.0),
            ("Communalities", "a", "Extraction", 0.742149779677),
            (
                "Component Matrix",
                "a",
                "Component / 1 (absolute)",
                0.861481154569,
            ),
            ("Communalities", "b", "Initial", 1.0),
            ("Communalities", "b", "Extraction", 0.944016796356),
            (
                "Component Matrix",
                "b",
                "Component / 1 (absolute)",
                0.971605267769,
            ),
            ("Communalities", "c", "Initial", 1.0),
            ("Communalities", "c", "Extraction", 0.759629595663),
            (
                "Component Matrix",
                "c",
                "Component / 1 (absolute)",
                0.87156732136,
            ),
            ("Communalities", "d", "Initial", 1.0),
            ("Communalities", "d", "Extraction", 0.57542908555),
            (
                "Component Matrix",
                "d",
                "Component / 1 (absolute)",
                0.758570422275,
            ),
        ],
    );
}

#[test]
fn two_variables_near_one_billion() {
    let t = run_on(
        SHIFTED,
        "FACTOR VARIABLES=a9 b9 c d /PRINT=INITIAL EXTRACTION KMO /CRITERIA=FACTORS(1).",
    );
    check(
        &t,
        &[
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Total",
                2.89553873474,
            ),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / % of Variance",
                72.3884683685,
            ),
            (
                "Total Variance Explained",
                "1",
                "Initial Eigenvalues / Cumulative %",
                72.3884683685,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Total",
                0.783180424687,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / % of Variance",
                19.5795106172,
            ),
            (
                "Total Variance Explained",
                "2",
                "Initial Eigenvalues / Cumulative %",
                91.9679789857,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / Total",
                0.263321820643,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / % of Variance",
                6.58304551608,
            ),
            (
                "Total Variance Explained",
                "3",
                "Initial Eigenvalues / Cumulative %",
                98.5510245017,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / Total",
                0.0579590199304,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / % of Variance",
                1.44897549826,
            ),
            (
                "Total Variance Explained",
                "4",
                "Initial Eigenvalues / Cumulative %",
                100.0,
            ),
            ("Communalities", "a9", "Initial", 1.0),
            ("Communalities", "a9", "Extraction", 0.77856344391),
            (
                "Component Matrix",
                "a9",
                "Component / 1 (absolute)",
                0.882362422086,
            ),
            ("Communalities", "b9", "Initial", 1.0),
            ("Communalities", "b9", "Extraction", 0.945981331501),
            (
                "Component Matrix",
                "b9",
                "Component / 1 (absolute)",
                0.972615716252,
            ),
            ("Communalities", "c", "Initial", 1.0),
            ("Communalities", "c", "Extraction", 0.783274942276),
            (
                "Component Matrix",
                "c",
                "Component / 1 (absolute)",
                0.885028215525,
            ),
            ("Communalities", "d", "Initial", 1.0),
            ("Communalities", "d", "Extraction", 0.387719017053),
            (
                "Component Matrix",
                "d",
                "Component / 1 (absolute)",
                0.622670873779,
            ),
            (
                "KMO and Bartlett's Test",
                "Kaiser-Meyer-Olkin Measure of Sampling Adequacy.",
                "Value",
                0.597020395023,
            ),
            (
                "KMO and Bartlett's Test",
                "Bartlett's Test of Sphericity / Approx. Chi-Square",
                "Value",
                22.9847258033,
            ),
            (
                "KMO and Bartlett's Test",
                "Bartlett's Test of Sphericity / df",
                "Value",
                6.0,
            ),
            (
                "KMO and Bartlett's Test",
                "Bartlett's Test of Sphericity / Sig.",
                "Value",
                0.000801610508707,
            ),
        ],
    );
}

#[test]
fn a_constant_variable_is_an_error() {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!(
        "{DATA}FACTOR VARIABLES=a b c k /PRINT=INITIAL EXTRACTION /CRITERIA=FACTORS(1)."
    ));
    assert!(errors.is_empty(), "{errors:?}");
    let mut message = String::new();
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            message = error.to_string();
        }
    }
    assert!(message.contains("no variance"), "{message}");
}
