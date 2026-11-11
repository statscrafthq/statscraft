//! RELIABILITY on synthetic data. The expected numbers come from an independent computation in a
//! general numerical library from the raw values: alpha of the item sums, item-total statistics
//! by dropping each item, squared multiple correlations and Guttman's lambda 6 by regressing each
//! item on the others, the two-way ANOVA from cell means, Tukey's test from the interaction
//! contrast, Friedman's test and Cochran's Q by their usual formulas, Hotelling's T-squared from
//! the difference scores, and the intraclass correlations with the confidence intervals of
//! McGraw and Wong (1996). The parallel models are checked against the manual's formulas with
//! the determinant of the covariance matrix. PSPP comparisons are in reference_procedures.rs.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn run(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
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

/// The row headers of every row joined with " / ", an empty header taking the one above it as
/// long as the headers before it stay the same.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                let text = match cell {
                    Cell::Text(t) => t.clone(),
                    Cell::Number(n) => n.to_string(),
                    Cell::Empty => continue,
                };
                carried[level] = text;
                for deeper in carried.iter_mut().skip(level + 1) {
                    deeper.clear();
                }
            }
            let parts: Vec<&str> = carried
                .iter()
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .collect();
            parts.join(" / ")
        })
        .collect()
}

fn find<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables.iter().find(|t| t.title == title).unwrap_or_else(|| {
        panic!(
            "no table {title} in {:?}",
            tables.iter().map(|t| &t.title).collect::<Vec<_>>()
        )
    })
}

fn cell<'t>(tables: &'t [Table], title: &str, row: &str, column: &str) -> &'t Cell {
    let table = find(tables, title);
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

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0)
}

const CASES: &[(&str, &str)] = &[
    (
        "alpha",
        "RELIABILITY /VARIABLES=a1 TO a4 /STATISTICS=DESCRIPTIVE SCALE /SUMMARY=TOTAL.",
    ),
    ("summary", "RELIABILITY /VARIABLES=a1 TO a4 /SUMMARY=ALL."),
    ("matrices", "RELI VAR=a1 a2 a3 a4 /STAT=CORR COV."),
    (
        "split",
        "RELIABILITY /VARIABLES=a1 TO a4 /MODEL=SPLIT /STATISTICS=SCALE.",
    ),
    (
        "split-five",
        "RELIABILITY /VARIABLES=a1 TO a5 /MODEL=SPLIT(1) /STATISTICS=SCALE.",
    ),
    ("guttman", "RELIABILITY /VARIABLES=a1 TO a4 /MODEL=GUTTMAN."),
    (
        "parallel",
        "RELIABILITY /VARIABLES=a1 TO a4 /MODEL=PARALLEL.",
    ),
    (
        "strict",
        "RELIABILITY /VARIABLES=a1 TO a4 /MODEL=STRICTPARALLEL.",
    ),
    (
        "anova",
        "RELIABILITY /VARIABLES=a1 TO a4 /STATISTICS=ANOVA TUKEY HOTELLING.",
    ),
    (
        "friedman",
        "RELIABILITY /VARIABLES=r1 TO r4 /STATISTICS=ANOVA FRIEDMAN.",
    ),
    (
        "cochran",
        "RELIABILITY /VARIABLES=b1 TO b4 /STATISTICS=COCHRAN ANOVA.",
    ),
    (
        "icc-oneway",
        "RELIABILITY /VARIABLES=a1 TO a4 /ICC=MODEL(ONEWAY).",
    ),
    (
        "icc-mixed",
        "RELIABILITY /VARIABLES=a1 TO a4 /ICC=MODEL(MIXED) TYPE(CONSISTENCY) CIN=95 TESTVAL=0.",
    ),
    (
        "icc-random-absolute",
        "RELIABILITY /VARIABLES=a1 TO a4 /ICC=MODEL(RANDOM) TYPE(ABSOLUTE) CIN=90 TESTVAL=0.2.",
    ),
    (
        "icc-consistency-test",
        "RELIABILITY /VARIABLES=a1 TO a4 /ICC=MODEL(RANDOM) TESTVAL=.3.",
    ),
    (
        "weighted",
        "WEIGHT BY w.\nRELIABILITY /VARIABLES=a1 TO a4 /SUMMARY=TOTAL.",
    ),
];

#[test]
fn every_statistic_matches_an_independent_computation() {
    let mut checked = 0;
    for (case, syntax) in CASES {
        let tables = run(&mut Engine::default(), &format!("{DATA}\n{syntax}"));
        let mut problems = Vec::new();
        for (c, title, row, column, expected) in EXPECTED.iter().filter(|e| e.0 == *case) {
            let actual = number(cell(&tables, title, row, column));
            if !close(actual, *expected) {
                problems.push(format!(
                    "{c}: {title} / {row} / {column}: {actual} is not {expected}"
                ));
            }
            checked += 1;
        }
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
    assert!(checked > 200, "{checked}");
}

const DATA: &str = "DATA LIST FREE /a1 a2 a3 a4 a5 b1 b2 b3 b4 r1 r2 r3 r4 w.
BEGIN DATA.
5 5 4 4 4 0 1 1 1 2 1 4 3 3
3 4 2 3 4 1 0 1 1 1 4 2 3 1
3 3 3 4 4 0 1 1 1 3 1 2 4 3
1 1 1 1 1 0 0 0 0 1.5 1.5 3 4 2
3 3 3 3 4 0 0 1 1 4 2 3 1 2
3 3 3 3 3 0 1 1 1 1 2 4 3 3
3 4 1 4 3 0 0 1 1 4 3 2 1 1
3 3 3 4 2 0 1 1 1 2 2 2 4 2
4 3 3 4 2 0 1 1 1 1 2 3 4 3
1 1 2 3 1 0 0 1 1 1 2 3 4 1
3 3 2 3 2 0 0 1 0 2 3 1 4 2
3 2 2 3 2 0 1 1 0 1 2 3 4 1
END DATA.";
const KENDALL_W: f64 = 0.21376811594202896;
const TUKEY_POWER: f64 = 0.6439628482972182;
const GRAND_MEAN: f64 = 2.875;
/// (case, table, row, column, value)
const EXPECTED: &[(&str, &str, &str, &str, f64)] = &[
    (
        "alpha",
        "Reliability Statistics",
        "",
        "Cronbach's Alpha",
        0.8822055137844612,
    ),
    (
        "alpha",
        "Reliability Statistics",
        "",
        "Cronbach's Alpha Based on Standardized Items",
        0.8835626136519095,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a1",
        "Scale Mean if Item Deleted",
        8.583333333333334,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a1",
        "Scale Variance if Item Deleted",
        6.083333333333332,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a1",
        "Corrected Item-Total Correlation",
        0.9042053270860062,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a1",
        "Cronbach's Alpha if Item Deleted",
        0.7808219178082192,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a2",
        "Scale Mean if Item Deleted",
        8.583333333333334,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a2",
        "Scale Variance if Item Deleted",
        6.265151515151513,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a2",
        "Corrected Item-Total Correlation",
        0.7667305109159659,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a2",
        "Cronbach's Alpha if Item Deleted",
        0.845223700120919,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a3",
        "Scale Mean if Item Deleted",
        9.083333333333334,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a3",
        "Scale Variance if Item Deleted",
        8.083333333333334,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a3",
        "Corrected Item-Total Correlation",
        0.6244653661971121,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a3",
        "Cronbach's Alpha if Item Deleted",
        0.8912839737582006,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a4",
        "Scale Mean if Item Deleted",
        8.25,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a4",
        "Scale Variance if Item Deleted",
        7.840909090909091,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a4",
        "Corrected Item-Total Correlation",
        0.7216459982982678,
    ),
    (
        "alpha",
        "Item-Total Statistics",
        "a4",
        "Cronbach's Alpha if Item Deleted",
        0.8608695652173913,
    ),
    ("alpha", "Item Statistics", "a1", "Mean", 2.9166666666666665),
    (
        "alpha",
        "Item Statistics",
        "a1",
        "Std. Deviation",
        1.0836246694508318,
    ),
    ("alpha", "Item Statistics", "a2", "Mean", 2.9166666666666665),
    (
        "alpha",
        "Item Statistics",
        "a2",
        "Std. Deviation",
        1.164500152881315,
    ),
    ("alpha", "Item Statistics", "a3", "Mean", 2.4166666666666665),
    (
        "alpha",
        "Item Statistics",
        "a3",
        "Std. Deviation",
        0.90033663737852,
    ),
    ("alpha", "Item Statistics", "a4", "Mean", 3.25),
    (
        "alpha",
        "Item Statistics",
        "a4",
        "Std. Deviation",
        0.8660254037844386,
    ),
    ("alpha", "Scale Statistics", "", "Mean", 11.5),
    (
        "alpha",
        "Scale Statistics",
        "",
        "Variance",
        12.090909090909092,
    ),
    (
        "alpha",
        "Scale Statistics",
        "",
        "Std. Deviation",
        3.4771984543464143,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a1",
        "Scale Mean if Item Deleted",
        8.583333333333334,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a1",
        "Scale Variance if Item Deleted",
        6.083333333333332,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a1",
        "Corrected Item-Total Correlation",
        0.9042053270860062,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a1",
        "Cronbach's Alpha if Item Deleted",
        0.7808219178082192,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a1",
        "Squared Multiple Correlation",
        0.8484245839872416,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a2",
        "Scale Mean if Item Deleted",
        8.583333333333334,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a2",
        "Scale Variance if Item Deleted",
        6.265151515151513,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a2",
        "Corrected Item-Total Correlation",
        0.7667305109159659,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a2",
        "Cronbach's Alpha if Item Deleted",
        0.845223700120919,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a2",
        "Squared Multiple Correlation",
        0.7752550707980442,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a3",
        "Scale Mean if Item Deleted",
        9.083333333333334,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a3",
        "Scale Variance if Item Deleted",
        8.083333333333334,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a3",
        "Corrected Item-Total Correlation",
        0.6244653661971121,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a3",
        "Cronbach's Alpha if Item Deleted",
        0.8912839737582006,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a3",
        "Squared Multiple Correlation",
        0.5528414139028536,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a4",
        "Scale Mean if Item Deleted",
        8.25,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a4",
        "Scale Variance if Item Deleted",
        7.840909090909091,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a4",
        "Corrected Item-Total Correlation",
        0.7216459982982678,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a4",
        "Cronbach's Alpha if Item Deleted",
        0.8608695652173913,
    ),
    (
        "summary",
        "Item-Total Statistics",
        "a4",
        "Squared Multiple Correlation",
        0.5213156595754926,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Item Means",
        "Mean",
        2.875,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Item Means",
        "Minimum",
        2.4166666666666665,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Item Means",
        "Maximum",
        3.25,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Item Means",
        "Range",
        0.8333333333333335,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Item Means",
        "Maximum / Minimum",
        1.3448275862068966,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Item Means",
        "Variance",
        0.1180555555555556,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Item Variances",
        "Mean",
        1.0227272727272727,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Item Variances",
        "Minimum",
        0.75,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Item Variances",
        "Maximum",
        1.3560606060606062,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Item Variances",
        "Range",
        0.6060606060606062,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Item Variances",
        "Maximum / Minimum",
        1.8080808080808082,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Item Variances",
        "Variance",
        0.08448117539026632,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Inter-Item Covariances",
        "Mean",
        0.6666666666666669,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Inter-Item Covariances",
        "Minimum",
        0.43181818181818193,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Inter-Item Covariances",
        "Maximum",
        1.0833333333333335,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Inter-Item Covariances",
        "Range",
        0.6515151515151516,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Inter-Item Covariances",
        "Maximum / Minimum",
        2.508771929824561,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Inter-Item Covariances",
        "Variance",
        0.04714500375657402,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Inter-Item Correlations",
        "Mean",
        0.6548244657320395,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Inter-Item Correlations",
        "Minimum",
        0.4696723052817422,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Inter-Item Correlations",
        "Maximum",
        0.8585066684456933,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Inter-Item Correlations",
        "Range",
        0.38883436316395115,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Inter-Item Correlations",
        "Maximum / Minimum",
        1.8278843755343446,
    ),
    (
        "summary",
        "Summary Item Statistics",
        "Inter-Item Correlations",
        "Variance",
        0.016280546729183918,
    ),
    (
        "matrices",
        "Inter-Item Correlation Matrix",
        "a1",
        "a1",
        0.9999999999999998,
    ),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a1",
        "a1",
        1.1742424242424243,
    ),
    (
        "matrices",
        "Inter-Item Correlation Matrix",
        "a1",
        "a2",
        0.8585066684456932,
    ),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a1",
        "a2",
        1.0833333333333335,
    ),
    (
        "matrices",
        "Inter-Item Correlation Matrix",
        "a1",
        "a3",
        0.6910862762215662,
    ),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a1",
        "a3",
        0.6742424242424244,
    ),
    (
        "matrices",
        "Inter-Item Correlation Matrix",
        "a1",
        "a4",
        0.7023212455599,
    ),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a1",
        "a4",
        0.6590909090909092,
    ),
    (
        "matrices",
        "Inter-Item Correlation Matrix",
        "a2",
        "a1",
        0.8585066684456933,
    ),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a2",
        "a1",
        1.0833333333333335,
    ),
    ("matrices", "Inter-Item Correlation Matrix", "a2", "a2", 1.0),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a2",
        "a2",
        1.3560606060606062,
    ),
    (
        "matrices",
        "Inter-Item Correlation Matrix",
        "a2",
        "a3",
        0.4696723052817422,
    ),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a2",
        "a3",
        0.49242424242424254,
    ),
    (
        "matrices",
        "Inter-Item Correlation Matrix",
        "a2",
        "a4",
        0.6535444634207008,
    ),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a2",
        "a4",
        0.6590909090909092,
    ),
    (
        "matrices",
        "Inter-Item Correlation Matrix",
        "a3",
        "a1",
        0.691086276221566,
    ),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a3",
        "a1",
        0.6742424242424244,
    ),
    (
        "matrices",
        "Inter-Item Correlation Matrix",
        "a3",
        "a2",
        0.4696723052817422,
    ),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a3",
        "a2",
        0.49242424242424254,
    ),
    ("matrices", "Inter-Item Correlation Matrix", "a3", "a3", 1.0),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a3",
        "a3",
        0.8106060606060608,
    ),
    (
        "matrices",
        "Inter-Item Correlation Matrix",
        "a3",
        "a4",
        0.5538158354626346,
    ),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a3",
        "a4",
        0.43181818181818193,
    ),
    (
        "matrices",
        "Inter-Item Correlation Matrix",
        "a4",
        "a1",
        0.7023212455599,
    ),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a4",
        "a1",
        0.6590909090909092,
    ),
    (
        "matrices",
        "Inter-Item Correlation Matrix",
        "a4",
        "a2",
        0.6535444634207009,
    ),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a4",
        "a2",
        0.6590909090909092,
    ),
    (
        "matrices",
        "Inter-Item Correlation Matrix",
        "a4",
        "a3",
        0.5538158354626347,
    ),
    (
        "matrices",
        "Inter-Item Covariance Matrix",
        "a4",
        "a3",
        0.43181818181818193,
    ),
    ("matrices", "Inter-Item Correlation Matrix", "a4", "a4", 1.0),
    ("matrices", "Inter-Item Covariance Matrix", "a4", "a4", 0.75),
    (
        "split",
        "Reliability Statistics",
        "Cronbach's Alpha / Part 1 / Value",
        "Value",
        0.9225806451612903,
    ),
    (
        "split",
        "Reliability Statistics",
        "Cronbach's Alpha / Part 2 / Value",
        "Value",
        0.7125000000000001,
    ),
    (
        "split",
        "Reliability Statistics",
        "Cronbach's Alpha / Part 1 / N of Items",
        "Value",
        2.0,
    ),
    (
        "split",
        "Reliability Statistics",
        "Cronbach's Alpha / Part 2 / N of Items",
        "Value",
        2.0,
    ),
    (
        "split",
        "Reliability Statistics",
        "Correlation Between Forms",
        "Value",
        0.736381738309777,
    ),
    (
        "split",
        "Reliability Statistics",
        "Spearman-Brown Coefficient / Equal Length",
        "Value",
        0.8481795472308794,
    ),
    (
        "split",
        "Reliability Statistics",
        "Spearman-Brown Coefficient / Unequal Length",
        "Value",
        0.8481795472308793,
    ),
    (
        "split",
        "Reliability Statistics",
        "Guttman Split-Half Coefficient",
        "Value",
        0.8220551378446117,
    ),
    (
        "split",
        "Scale Statistics",
        "Part 1",
        "Mean",
        5.833333333333333,
    ),
    (
        "split",
        "Scale Statistics",
        "Part 2",
        "Variance",
        2.4242424242424248,
    ),
    (
        "split",
        "Scale Statistics",
        "Both Parts",
        "Variance",
        12.090909090909092,
    ),
    (
        "split-five",
        "Reliability Statistics",
        "Cronbach's Alpha / Part 1 / Value",
        "Value",
        0.8822055137844612,
    ),
    (
        "split-five",
        "Reliability Statistics",
        "Cronbach's Alpha / Part 1 / N of Items",
        "Value",
        4.0,
    ),
    (
        "split-five",
        "Reliability Statistics",
        "Cronbach's Alpha / Part 2 / N of Items",
        "Value",
        1.0,
    ),
    (
        "split-five",
        "Reliability Statistics",
        "Correlation Between Forms",
        "Value",
        0.7018917899201756,
    ),
    (
        "split-five",
        "Reliability Statistics",
        "Spearman-Brown Coefficient / Equal Length",
        "Value",
        0.8248371536630971,
    ),
    (
        "split-five",
        "Reliability Statistics",
        "Spearman-Brown Coefficient / Unequal Length",
        "Value",
        0.874104040218539,
    ),
    (
        "split-five",
        "Reliability Statistics",
        "Guttman Split-Half Coefficient",
        "Value",
        0.5914149443561205,
    ),
    ("split-five", "Scale Statistics", "Part 1", "Mean", 11.5),
    (
        "split-five",
        "Scale Statistics",
        "Part 2",
        "Variance",
        1.3333333333333333,
    ),
    (
        "split-five",
        "Scale Statistics",
        "Both Parts",
        "Variance",
        19.06060606060606,
    ),
    (
        "guttman",
        "Reliability Statistics",
        "Lambda / 1",
        "Value",
        0.6616541353383458,
    ),
    (
        "guttman",
        "Reliability Statistics",
        "Lambda / 2",
        "Value",
        0.8926796324630824,
    ),
    (
        "guttman",
        "Reliability Statistics",
        "Lambda / 3",
        "Value",
        0.8822055137844611,
    ),
    (
        "guttman",
        "Reliability Statistics",
        "Lambda / 4",
        "Value",
        0.8220551378446117,
    ),
    (
        "guttman",
        "Reliability Statistics",
        "Lambda / 5",
        "Value",
        0.8992178475474274,
    ),
    (
        "guttman",
        "Reliability Statistics",
        "Lambda / 6",
        "Value",
        0.9004014722910096,
    ),
    (
        "parallel",
        "Reliability Statistics",
        "Common Variance",
        "Value",
        1.0227272727272727,
    ),
    (
        "parallel",
        "Reliability Statistics",
        "True Variance",
        "Value",
        0.6666666666666669,
    ),
    (
        "parallel",
        "Reliability Statistics",
        "Error Variance",
        "Value",
        0.35606060606060586,
    ),
    (
        "parallel",
        "Reliability Statistics",
        "Common Inter-Item Correlation",
        "Value",
        0.651851851851852,
    ),
    (
        "parallel",
        "Reliability Statistics",
        "Reliability of Scale",
        "Value",
        0.8822055137844612,
    ),
    (
        "parallel",
        "Reliability Statistics",
        "Reliability of Scale (Unbiased)",
        "Value",
        0.9036226930963774,
    ),
    (
        "parallel",
        "Test for Goodness of Fit",
        "Chi-Square / Value",
        "Value",
        8.155575499925188,
    ),
    (
        "parallel",
        "Test for Goodness of Fit",
        "Chi-Square / df",
        "Value",
        8.0,
    ),
    (
        "parallel",
        "Test for Goodness of Fit",
        "Chi-Square / Sig.",
        "Value",
        0.4184225955435699,
    ),
    (
        "parallel",
        "Test for Goodness of Fit",
        "Log of Determinant of / Unconstrained Matrix",
        "Value",
        -2.8721654413308633,
    ),
    (
        "parallel",
        "Test for Goodness of Fit",
        "Log of Determinant of / Constrained Matrix",
        "Value",
        -1.9918034683254457,
    ),
    (
        "strict",
        "Reliability Statistics",
        "Common Mean",
        "Value",
        2.875,
    ),
    (
        "strict",
        "Reliability Statistics",
        "Common Variance",
        "Value",
        1.1112689393939394,
    ),
    (
        "strict",
        "Reliability Statistics",
        "Error Variance",
        "Value",
        0.4444444444444444,
    ),
    (
        "strict",
        "Reliability Statistics",
        "True Variance",
        "Value",
        0.666824494949495,
    ),
    (
        "strict",
        "Reliability Statistics",
        "Common Inter-Item Correlation",
        "Value",
        0.5733560573782134,
    ),
    (
        "strict",
        "Reliability Statistics",
        "Reliability of Scale",
        "Value",
        0.8431495405179615,
    ),
    (
        "strict",
        "Reliability Statistics",
        "Reliability of Scale (Unbiased)",
        "Value",
        0.8823621553884712,
    ),
    (
        "strict",
        "Test for Goodness of Fit",
        "Chi-Square / Value",
        "Value",
        16.93734536915873,
    ),
    (
        "strict",
        "Test for Goodness of Fit",
        "Log of Determinant of / Constrained Matrix",
        "Value",
        -1.1327492675272175,
    ),
    ("anova", "ANOVA", "Between People", "Sum of Squares", 33.25),
    ("anova", "ANOVA", "Between People", "df", 11.0),
    (
        "anova",
        "ANOVA",
        "Between People",
        "Mean Square",
        3.022727272727273,
    ),
    (
        "anova",
        "ANOVA",
        "Within People / Between Items",
        "Sum of Squares",
        4.250000000000002,
    ),
    (
        "anova",
        "ANOVA",
        "Within People / Between Items",
        "F",
        3.9787234042553212,
    ),
    (
        "anova",
        "ANOVA",
        "Within People / Between Items",
        "Sig.",
        0.015910177837632714,
    ),
    (
        "anova",
        "ANOVA",
        "Within People / Residual",
        "Sum of Squares",
        11.749999999999998,
    ),
    ("anova", "ANOVA", "Within People / Residual", "df", 33.0),
    (
        "anova",
        "ANOVA",
        "Within People / Total",
        "Sum of Squares",
        16.0,
    ),
    ("anova", "ANOVA", "Total", "Sum of Squares", 49.25),
    ("anova", "ANOVA", "Total", "df", 47.0),
    (
        "anova",
        "ANOVA",
        "Within People / Nonadditivity",
        "Sum of Squares",
        0.045149638802888435,
    ),
    (
        "anova",
        "ANOVA",
        "Within People / Nonadditivity",
        "F",
        0.12343502027860735,
    ),
    (
        "anova",
        "ANOVA",
        "Within People / Nonadditivity",
        "Sig.",
        0.7276411990845448,
    ),
    (
        "anova",
        "ANOVA",
        "Within People / Balance",
        "Sum of Squares",
        11.70485036119711,
    ),
    ("anova", "ANOVA", "Within People / Balance", "df", 32.0),
    (
        "anova",
        "Hotelling's T-Squared Test",
        "",
        "Hotelling's T-Squared",
        13.854961832061075,
    ),
    (
        "anova",
        "Hotelling's T-Squared Test",
        "",
        "F",
        3.778625954198475,
    ),
    ("anova", "Hotelling's T-Squared Test", "", "df2", 9.0),
    (
        "anova",
        "Hotelling's T-Squared Test",
        "",
        "Sig.",
        0.052661543849763584,
    ),
    (
        "friedman",
        "ANOVA",
        "Within People / Between Items",
        "Friedman's Chi-Square",
        7.695652173913043,
    ),
    (
        "friedman",
        "ANOVA",
        "Within People / Between Items",
        "Sig.",
        0.05273878507524288,
    ),
    (
        "cochran",
        "ANOVA",
        "Within People / Between Items",
        "Cochran's Q",
        18.405405405405407,
    ),
    (
        "cochran",
        "ANOVA",
        "Within People / Between Items",
        "Sig.",
        0.00036278411025726653,
    ),
    (
        "icc-oneway",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "Intraclass Correlation",
        0.5918840579710145,
    ),
    (
        "icc-oneway",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "95% Confidence Interval / Lower Bound",
        0.3178454046734991,
    ),
    (
        "icc-oneway",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "95% Confidence Interval / Upper Bound",
        0.8329799839948199,
    ),
    (
        "icc-oneway",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "F Test with True Value 0 / Value",
        6.801136363636364,
    ),
    (
        "icc-oneway",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "F Test with True Value 0 / df2",
        36.0,
    ),
    (
        "icc-oneway",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "F Test with True Value 0 / Sig.",
        4.988531734799693e-06,
    ),
    (
        "icc-oneway",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "Intraclass Correlation",
        0.8529657477025899,
    ),
    (
        "icc-oneway",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "95% Confidence Interval / Lower Bound",
        0.6508103661295406,
    ),
    (
        "icc-oneway",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "95% Confidence Interval / Upper Bound",
        0.9522655380494732,
    ),
    (
        "icc-oneway",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "F Test with True Value 0 / Value",
        6.801136363636364,
    ),
    (
        "icc-oneway",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "F Test with True Value 0 / df2",
        36.0,
    ),
    (
        "icc-oneway",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "F Test with True Value 0 / Sig.",
        4.988531734799693e-06,
    ),
    (
        "icc-mixed",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "Intraclass Correlation",
        0.6518518518518519,
    ),
    (
        "icc-mixed",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "95% Confidence Interval / Lower Bound",
        0.3864324275256496,
    ),
    (
        "icc-mixed",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "95% Confidence Interval / Upper Bound",
        0.8634543422509966,
    ),
    (
        "icc-mixed",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "F Test with True Value 0 / Value",
        8.489361702127662,
    ),
    (
        "icc-mixed",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "F Test with True Value 0 / df2",
        33.0,
    ),
    (
        "icc-mixed",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "F Test with True Value 0 / Sig.",
        7.644642194091045e-07,
    ),
    (
        "icc-mixed",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "Intraclass Correlation",
        0.8822055137844612,
    ),
    (
        "icc-mixed",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "95% Confidence Interval / Lower Bound",
        0.7158484950520076,
    ),
    (
        "icc-mixed",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "95% Confidence Interval / Upper Bound",
        0.9619688436151007,
    ),
    (
        "icc-mixed",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "F Test with True Value 0 / Value",
        8.489361702127662,
    ),
    (
        "icc-mixed",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "F Test with True Value 0 / df2",
        33.0,
    ),
    (
        "icc-mixed",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "F Test with True Value 0 / Sig.",
        7.644642194091045e-07,
    ),
    (
        "icc-random-absolute",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "Intraclass Correlation",
        0.6000000000000001,
    ),
    (
        "icc-random-absolute",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "90% Confidence Interval / Lower Bound",
        0.3705530033924307,
    ),
    (
        "icc-random-absolute",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "90% Confidence Interval / Upper Bound",
        0.807394493860649,
    ),
    (
        "icc-random-absolute",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "F Test with True Value 0.2 / Value",
        3.776025236593061,
    ),
    (
        "icc-random-absolute",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "F Test with True Value 0.2 / df2",
        34.1601267045235,
    ),
    (
        "icc-random-absolute",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "F Test with True Value 0.2 / Sig.",
        0.0013586808788041554,
    ),
    (
        "icc-random-absolute",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "Intraclass Correlation",
        0.8571428571428572,
    ),
    (
        "icc-random-absolute",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "90% Confidence Interval / Lower Bound",
        0.7019182578371297,
    ),
    (
        "icc-random-absolute",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "90% Confidence Interval / Upper Bound",
        0.943718533159912,
    ),
    (
        "icc-random-absolute",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "F Test with True Value 0.2 / Value",
        6.47027027027027,
    ),
    (
        "icc-random-absolute",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "F Test with True Value 0.2 / df2",
        35.8095204124551,
    ),
    (
        "icc-random-absolute",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "F Test with True Value 0.2 / Sig.",
        8.742619942828291e-06,
    ),
    (
        "icc-consistency-test",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "Intraclass Correlation",
        0.6518518518518519,
    ),
    (
        "icc-consistency-test",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "95% Confidence Interval / Lower Bound",
        0.3864324275256496,
    ),
    (
        "icc-consistency-test",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "95% Confidence Interval / Upper Bound",
        0.8634543422509966,
    ),
    (
        "icc-consistency-test",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "F Test with True Value 0.3 / Value",
        3.1276595744680855,
    ),
    (
        "icc-consistency-test",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "F Test with True Value 0.3 / df2",
        33.0,
    ),
    (
        "icc-consistency-test",
        "Intraclass Correlation Coefficient",
        "Single Measures",
        "F Test with True Value 0.3 / Sig.",
        0.005466117731258465,
    ),
    (
        "icc-consistency-test",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "Intraclass Correlation",
        0.8822055137844612,
    ),
    (
        "icc-consistency-test",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "95% Confidence Interval / Lower Bound",
        0.7158484950520076,
    ),
    (
        "icc-consistency-test",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "95% Confidence Interval / Upper Bound",
        0.9619688436151007,
    ),
    (
        "icc-consistency-test",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "F Test with True Value 0.3 / Value",
        5.942553191489362,
    ),
    (
        "icc-consistency-test",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "F Test with True Value 0.3 / df2",
        33.0,
    ),
    (
        "icc-consistency-test",
        "Intraclass Correlation Coefficient",
        "Average Measures",
        "F Test with True Value 0.3 / Sig.",
        3.08735636624854e-05,
    ),
    (
        "weighted",
        "Reliability Statistics",
        "",
        "Cronbach's Alpha",
        0.9180392156862744,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a1",
        "Scale Mean if Item Deleted",
        9.041666666666666,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a1",
        "Scale Variance if Item Deleted",
        6.215579710144928,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a1",
        "Corrected Item-Total Correlation",
        0.9218746298257962,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a1",
        "Cronbach's Alpha if Item Deleted",
        0.8533955115126785,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a2",
        "Scale Mean if Item Deleted",
        9.125,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a2",
        "Scale Variance if Item Deleted",
        6.548913043478261,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a2",
        "Corrected Item-Total Correlation",
        0.8297654181839925,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a2",
        "Cronbach's Alpha if Item Deleted",
        0.8896265560165975,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a3",
        "Scale Mean if Item Deleted",
        9.5,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a3",
        "Scale Variance if Item Deleted",
        7.826086956521739,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a3",
        "Corrected Item-Total Correlation",
        0.769823489592391,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a3",
        "Cronbach's Alpha if Item Deleted",
        0.9090277777777778,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a4",
        "Scale Mean if Item Deleted",
        8.833333333333334,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a4",
        "Scale Variance if Item Deleted",
        7.884057971014494,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a4",
        "Corrected Item-Total Correlation",
        0.7550967469702594,
    ),
    (
        "weighted",
        "Item-Total Statistics",
        "a4",
        "Cronbach's Alpha if Item Deleted",
        0.9133731617647058,
    ),
    ("weighted", "Case Processing Summary", "Valid", "N", 24.0),
];

fn caption(tables: &[Table], title: &str) -> String {
    find(tables, title).caption.clone().unwrap_or_default()
}

/// The number after `label` in a caption, up to the period that ends it.
fn captioned(text: &str, label: &str) -> f64 {
    let (_, rest) = text
        .split_once(label)
        .unwrap_or_else(|| panic!("no {label} in {text}"));
    let end = rest.find(". ").unwrap_or(rest.len() - 1);
    rest[..end].trim_end_matches('.').parse().unwrap()
}

#[test]
fn captions_hold_the_grand_mean_kendalls_w_and_tukeys_power() {
    let tables = run(
        &mut Engine::default(),
        &format!("{DATA}\nRELIABILITY /VARIABLES=a1 TO a4 /STATISTICS=ANOVA TUKEY."),
    );
    let text = caption(&tables, "ANOVA");
    assert!(close(captioned(&text, "Grand Mean = "), GRAND_MEAN));
    assert!(close(
        captioned(&text, "achieve additivity = "),
        TUKEY_POWER
    ));
    let tables = run(
        &mut Engine::default(),
        &format!("{DATA}\nRELIABILITY /VARIABLES=r1 TO r4 /STATISTICS=ANOVA FRIEDMAN."),
    );
    let text = caption(&tables, "ANOVA");
    assert!(close(captioned(&text, "concordance W = "), KENDALL_W));
}

#[test]
fn friedman_without_anova_is_ignored_and_tables_follow_the_default() {
    let tables = run(
        &mut Engine::default(),
        &format!("{DATA}\nRELIABILITY /VARIABLES=a1 TO a4 /STATISTICS=FRIEDMAN."),
    );
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Case Processing Summary", "Reliability Statistics"]
    );
    assert_eq!(caption(&tables, "Reliability Statistics"), "Scale: ALL.");
}

#[test]
fn the_scale_is_a_subset_and_cases_are_dropped_on_all_variables() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{DATA}\nRECODE a5 (4 = SYSMIS).\n\
             RELIABILITY /VARIABLES=a1 TO a5 /SCALE('First three') a1 a2 a3 /SUMMARY=TOTAL."
        ),
    );
    let summary = find(&tables, "Case Processing Summary");
    // the four cases where a5 is 4 are left out, a5 being on VARIABLES
    assert_eq!(
        (number(&summary.rows[0][1]), number(&summary.rows[1][1])),
        (8.0, 4.0)
    );
    assert_eq!(
        row_keys(find(&tables, "Item-Total Statistics")),
        ["a1", "a2", "a3"]
    );
    assert!(caption(&tables, "Reliability Statistics").starts_with("Scale: First three."));
    let (commands, _) = parse(&format!(
        "{DATA}\nRELIABILITY /VARIABLES=a1 a2 /SCALE(x) a1 a3."
    ));
    let mut engine = Engine::default();
    let mut error = None;
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            error = Some(e.0);
        }
    }
    assert_eq!(
        error.unwrap(),
        "a3 is on SCALE but not on VARIABLES of RELIABILITY"
    );
}

#[test]
fn the_covariance_method_drops_items_without_variance() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{DATA}\nCOMPUTE c = 3.\nRELIABILITY /VARIABLES=a1 a2 c /STATISTICS=CORR /SUMMARY=TOTAL."
        ),
    );
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("c has no variance and is dropped")),
        "{:?}",
        engine.warnings()
    );
    assert_eq!(
        row_keys(find(&tables, "Item-Total Statistics")),
        ["a1", "a2"]
    );
    // without the covariance method the item stays
    let tables = run(
        &mut Engine::default(),
        &format!("{DATA}\nCOMPUTE c = 3.\nRELIABILITY /VARIABLES=a1 a2 c /SUMMARY=TOTAL."),
    );
    assert_eq!(
        row_keys(find(&tables, "Item-Total Statistics")),
        ["a1", "a2", "c"]
    );
}

#[test]
fn a_negative_alpha_says_why() {
    let tables = run(
        &mut Engine::default(),
        &format!("{DATA}\nCOMPUTE m = 6 - a1.\nRELIABILITY /VARIABLES=a1 m b1."),
    );
    let alpha = number(cell(
        &tables,
        "Reliability Statistics",
        "",
        "Cronbach's Alpha",
    ));
    assert!(alpha < 0.0);
    assert!(caption(&tables, "Reliability Statistics").contains("negative"));
}

#[test]
fn unsupported_models_and_matrices_say_so() {
    let (_, errors) = parse("RELIABILITY /VARIABLES=a b c /MODEL=OMEGA.");
    assert_eq!(
        errors[0].message,
        "OMEGA in RELIABILITY /MODEL is not supported yet"
    );
    let (_, errors) = parse("RELIABILITY /VARIABLES=a b c /MATRIX=OUT(*).");
    assert_eq!(
        errors[0].message,
        "subcommand MATRIX of RELIABILITY is not supported yet"
    );
    let (_, errors) = parse("RELIABILITY /MODEL=ALPHA.");
    assert!(errors[0].hint.as_ref().unwrap().contains("VARIABLES"));
}
