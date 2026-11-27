//! Edge cases of CROSSTABS against R on ten invented cases: a square table with zero cells and a
//! missing value in either variable, a 2 x 2 table with an empty cell (Fisher, McNemar, the
//! continuity correction, the risk estimates), a column of one case, and a table that a zero
//! weight leaves with one column. The statistics (chi-squares, phi, V, C, gamma, tau-b, tau-c,
//! Spearman, Pearson, Somers' d, lambda, Goodman and Kruskal tau, the uncertainty coefficient,
//! kappa) are the formulas of the algorithms reference written out on the counts, with
//! `chisq.test`, `fisher.test` and `binom.test` where R has them; standard errors are not
//! compared. An odds ratio over a zero cell and a risk ratio of 0 have no interval, and the
//! interval of a risk ratio of 0 is empty. Scripts in lab/oracles/edge-r4.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /a b c w r s (6F8.2).
BEGIN DATA.
1 1 1 1 1 1
1 1 1 1 1 1
1 2 1 1 1 1
2 1 1 1 2 1
2 2 1 1 2 1
2 2 2 0 2 2
3 1 1 1 2 2
3 3 1 1 2 2
. 1 1 1 2 2
1 . 1 1 . .
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

/// The number of leading cells that name a row: two in the table of counts and in the symmetric
/// measures, three in the directional measures, one elsewhere.
fn headers(title: &str) -> usize {
    if title.starts_with("Directional") {
        3
    } else if title.starts_with("Symmetric") || (title.contains(" by ") && !title.contains(": ")) {
        2
    } else {
        1
    }
}

/// The row headers joined with " / ", an empty one carried from the row above.
fn row_keys(table: &Table) -> Vec<String> {
    let count = headers(&table.title);
    let mut carried = vec![String::new(); count];
    table
        .rows
        .iter()
        .map(|row| {
            for (slot, cell) in carried.iter_mut().zip(row) {
                let text = match cell {
                    Cell::Text(t) => t.clone(),
                    Cell::Number(n) => format!("{n}"),
                    Cell::Empty => String::new(),
                };
                if !text.is_empty() {
                    *slot = text;
                }
            }
            carried.join(" / ")
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
fn square_table_with_zero_cells_and_missing_values() {
    let t = run(
        "CROSSTABS /TABLES=a BY b /STATISTICS=CHISQ PHI CC LAMBDA UC GAMMA D BTAU CTAU CORR KAPPA /CELLS=COUNT EXPECTED.",
    );
    check(
        &t,
        &[
            ("a by b", "1 / Count", "b / 1", 2.0),
            ("a by b", "1 / Expected", "b / 1", 1.5),
            ("a by b", "1 / Count", "b / 2", 1.0),
            ("a by b", "1 / Expected", "b / 2", 1.125),
            ("a by b", "1 / Count", "b / 3", 0.0),
            ("a by b", "1 / Expected", "b / 3", 0.375),
            ("a by b", "2 / Count", "b / 1", 1.0),
            ("a by b", "2 / Expected", "b / 1", 1.5),
            ("a by b", "2 / Count", "b / 2", 2.0),
            ("a by b", "2 / Expected", "b / 2", 1.125),
            ("a by b", "2 / Count", "b / 3", 0.0),
            ("a by b", "2 / Expected", "b / 3", 0.375),
            ("a by b", "3 / Count", "b / 1", 1.0),
            ("a by b", "3 / Expected", "b / 1", 1.0),
            ("a by b", "3 / Count", "b / 2", 0.0),
            ("a by b", "3 / Expected", "b / 2", 0.75),
            ("a by b", "3 / Count", "b / 3", 1.0),
            ("a by b", "3 / Expected", "b / 3", 0.25),
            ("a by b", "1 / Count", "Total", 3.0),
            ("a by b", "1 / Expected", "Total", 3.0),
            ("a by b", "2 / Count", "Total", 3.0),
            ("a by b", "2 / Expected", "Total", 3.0),
            ("a by b", "3 / Count", "Total", 2.0),
            ("a by b", "3 / Expected", "Total", 2.0),
            ("a by b", "Total / Count", "b / 1", 4.0),
            ("a by b", "Total / Count", "b / 2", 3.0),
            ("a by b", "Total / Count", "b / 3", 1.0),
            ("a by b", "Total / Count", "Total", 8.0),
            ("Chi-Square Tests: a by b", "N of Valid Cases", "Value", 8.0),
            (
                "Chi-Square Tests: a by b",
                "Pearson Chi-Square",
                "Value",
                4.77777777778,
            ),
            ("Chi-Square Tests: a by b", "Pearson Chi-Square", "df", 4.0),
            (
                "Chi-Square Tests: a by b",
                "Pearson Chi-Square",
                "Asymptotic Sig. (2-tailed)",
                0.31086803469,
            ),
            (
                "Chi-Square Tests: a by b",
                "Likelihood Ratio",
                "Value",
                5.17827730413,
            ),
            (
                "Chi-Square Tests: a by b",
                "Likelihood Ratio",
                "Asymptotic Sig. (2-tailed)",
                0.269489349703,
            ),
            (
                "Chi-Square Tests: a by b",
                "Linear-by-Linear Association",
                "Value",
                0.978494623656,
            ),
            (
                "Chi-Square Tests: a by b",
                "Linear-by-Linear Association",
                "Asymptotic Sig. (2-tailed)",
                0.322570742123,
            ),
            (
                "Symmetric Measures: a by b",
                "Nominal by Nominal / Phi",
                "Value",
                0.772801541291,
            ),
            (
                "Symmetric Measures: a by b",
                "Nominal by Nominal / Cramer's V",
                "Value",
                0.546453210359,
            ),
            (
                "Symmetric Measures: a by b",
                "Nominal by Nominal / Contingency Coefficient",
                "Value",
                0.611484295365,
            ),
            (
                "Symmetric Measures: a by b",
                "Ordinal by Ordinal / Gamma",
                "Value",
                0.428571428571,
            ),
            (
                "Symmetric Measures: a by b",
                "Ordinal by Ordinal / Kendall's tau-b",
                "Value",
                0.300375704593,
            ),
            (
                "Symmetric Measures: a by b",
                "Ordinal by Ordinal / Kendall's tau-c",
                "Value",
                0.28125,
            ),
            (
                "Symmetric Measures: a by b",
                "Ordinal by Ordinal / Spearman Correlation",
                "Value",
                0.310529501704,
            ),
            (
                "Symmetric Measures: a by b",
                "Interval by Interval / Pearson's R",
                "Value",
                0.373878250553,
            ),
            (
                "Directional Measures: a by b",
                "Ordinal by Ordinal / Somers' d / Symmetric",
                "Value",
                0.3,
            ),
            (
                "Directional Measures: a by b",
                "Ordinal by Ordinal / Somers' d / a Dependent",
                "Value",
                0.315789473684,
            ),
            (
                "Directional Measures: a by b",
                "Ordinal by Ordinal / Somers' d / b Dependent",
                "Value",
                0.285714285714,
            ),
            (
                "Symmetric Measures: a by b",
                "Measure of Agreement / Kappa",
                "Value",
                0.414634146341,
            ),
            (
                "Directional Measures: a by b",
                "Nominal by Nominal / Lambda / Symmetric",
                "Value",
                0.333333333333,
            ),
            (
                "Directional Measures: a by b",
                "Nominal by Nominal / Lambda / a Dependent",
                "Value",
                0.4,
            ),
            (
                "Directional Measures: a by b",
                "Nominal by Nominal / Lambda / b Dependent",
                "Value",
                0.25,
            ),
            (
                "Directional Measures: a by b",
                "Nominal by Nominal / Goodman and Kruskal tau / a Dependent",
                "Value",
                0.269841269841,
            ),
            (
                "Directional Measures: a by b",
                "Nominal by Nominal / Goodman and Kruskal tau / b Dependent",
                "Value",
                0.228070175439,
            ),
            (
                "Directional Measures: a by b",
                "Nominal by Nominal / Uncertainty Coefficient / Symmetric",
                "Value",
                0.314749052507,
            ),
            (
                "Directional Measures: a by b",
                "Nominal by Nominal / Uncertainty Coefficient / a Dependent",
                "Value",
                0.299060865181,
            ),
            (
                "Directional Measures: a by b",
                "Nominal by Nominal / Uncertainty Coefficient / b Dependent",
                "Value",
                0.332174310771,
            ),
        ],
    );
}

#[test]
fn two_by_two_table_with_an_empty_cell() {
    let t = run(
        "CROSSTABS /TABLES=r BY s /STATISTICS=CHISQ PHI CC LAMBDA UC GAMMA D BTAU CTAU CORR KAPPA RISK MCNEMAR /CELLS=COUNT EXPECTED.",
    );
    check(
        &t,
        &[
            ("r by s", "1 / Count", "s / 1", 3.0),
            ("r by s", "1 / Expected", "s / 1", 1.66666666667),
            ("r by s", "1 / Count", "s / 2", 0.0),
            ("r by s", "1 / Expected", "s / 2", 1.33333333333),
            ("r by s", "2 / Count", "s / 1", 2.0),
            ("r by s", "2 / Expected", "s / 1", 3.33333333333),
            ("r by s", "2 / Count", "s / 2", 4.0),
            ("r by s", "2 / Expected", "s / 2", 2.66666666667),
            ("r by s", "1 / Count", "Total", 3.0),
            ("r by s", "1 / Expected", "Total", 3.0),
            ("r by s", "2 / Count", "Total", 6.0),
            ("r by s", "2 / Expected", "Total", 6.0),
            ("r by s", "Total / Count", "s / 1", 5.0),
            ("r by s", "Total / Count", "s / 2", 4.0),
            ("r by s", "Total / Count", "Total", 9.0),
            ("Chi-Square Tests: r by s", "N of Valid Cases", "Value", 9.0),
            (
                "Chi-Square Tests: r by s",
                "Pearson Chi-Square",
                "Value",
                3.6,
            ),
            ("Chi-Square Tests: r by s", "Pearson Chi-Square", "df", 1.0),
            (
                "Chi-Square Tests: r by s",
                "Pearson Chi-Square",
                "Asymptotic Sig. (2-tailed)",
                0.0577795711236,
            ),
            (
                "Chi-Square Tests: r by s",
                "Likelihood Ratio",
                "Value",
                4.72713835921,
            ),
            (
                "Chi-Square Tests: r by s",
                "Likelihood Ratio",
                "Asymptotic Sig. (2-tailed)",
                0.0296902440759,
            ),
            (
                "Chi-Square Tests: r by s",
                "Linear-by-Linear Association",
                "Value",
                3.2,
            ),
            (
                "Chi-Square Tests: r by s",
                "Linear-by-Linear Association",
                "Asymptotic Sig. (2-tailed)",
                0.0736382701203,
            ),
            (
                "Chi-Square Tests: r by s",
                "Continuity Correction",
                "Value",
                1.40625,
            ),
            (
                "Chi-Square Tests: r by s",
                "Continuity Correction",
                "Asymptotic Sig. (2-tailed)",
                0.235679913429,
            ),
            (
                "Chi-Square Tests: r by s",
                "Fisher's Exact Test",
                "Exact Sig. (2-sided)",
                0.166666666667,
            ),
            (
                "Chi-Square Tests: r by s",
                "Fisher's Exact Test",
                "Exact Sig. (1-sided)",
                0.119047619048,
            ),
            (
                "Chi-Square Tests: r by s",
                "McNemar Test",
                "Exact Sig. (2-sided)",
                0.5,
            ),
            (
                "Risk Estimate: r by s",
                "Odds Ratio for r (1 / 2)",
                "Value",
                f64::NAN,
            ),
            ("Risk Estimate: r by s", "For cohort s = 1", "Value", 3.0),
            (
                "Risk Estimate: r by s",
                "For cohort s = 1",
                "95% Confidence Interval / Lower",
                0.967564252522,
            ),
            (
                "Risk Estimate: r by s",
                "For cohort s = 1",
                "95% Confidence Interval / Upper",
                9.30170784684,
            ),
            ("Risk Estimate: r by s", "For cohort s = 2", "Value", 0.0),
            (
                "Risk Estimate: r by s",
                "For cohort s = 2",
                "95% Confidence Interval / Lower",
                f64::NAN,
            ),
            (
                "Risk Estimate: r by s",
                "For cohort s = 2",
                "95% Confidence Interval / Upper",
                f64::NAN,
            ),
            (
                "Symmetric Measures: r by s",
                "Nominal by Nominal / Phi",
                "Value",
                0.632455532034,
            ),
            (
                "Symmetric Measures: r by s",
                "Nominal by Nominal / Cramer's V",
                "Value",
                0.632455532034,
            ),
            (
                "Symmetric Measures: r by s",
                "Nominal by Nominal / Contingency Coefficient",
                "Value",
                0.534522483825,
            ),
            (
                "Symmetric Measures: r by s",
                "Ordinal by Ordinal / Gamma",
                "Value",
                1.0,
            ),
            (
                "Symmetric Measures: r by s",
                "Ordinal by Ordinal / Kendall's tau-b",
                "Value",
                0.632455532034,
            ),
            (
                "Symmetric Measures: r by s",
                "Ordinal by Ordinal / Kendall's tau-c",
                "Value",
                0.592592592593,
            ),
            (
                "Symmetric Measures: r by s",
                "Ordinal by Ordinal / Spearman Correlation",
                "Value",
                0.632455532034,
            ),
            (
                "Symmetric Measures: r by s",
                "Interval by Interval / Pearson's R",
                "Value",
                0.632455532034,
            ),
            (
                "Directional Measures: r by s",
                "Ordinal by Ordinal / Somers' d / Symmetric",
                "Value",
                0.631578947368,
            ),
            (
                "Directional Measures: r by s",
                "Ordinal by Ordinal / Somers' d / r Dependent",
                "Value",
                0.6,
            ),
            (
                "Directional Measures: r by s",
                "Ordinal by Ordinal / Somers' d / s Dependent",
                "Value",
                0.666666666667,
            ),
            (
                "Symmetric Measures: r by s",
                "Measure of Agreement / Kappa",
                "Value",
                0.571428571429,
            ),
            (
                "Directional Measures: r by s",
                "Nominal by Nominal / Lambda / Symmetric",
                "Value",
                0.428571428571,
            ),
            (
                "Directional Measures: r by s",
                "Nominal by Nominal / Lambda / r Dependent",
                "Value",
                0.333333333333,
            ),
            (
                "Directional Measures: r by s",
                "Nominal by Nominal / Lambda / s Dependent",
                "Value",
                0.5,
            ),
            (
                "Directional Measures: r by s",
                "Nominal by Nominal / Goodman and Kruskal tau / r Dependent",
                "Value",
                0.4,
            ),
            (
                "Directional Measures: r by s",
                "Nominal by Nominal / Goodman and Kruskal tau / s Dependent",
                "Value",
                0.4,
            ),
            (
                "Directional Measures: r by s",
                "Nominal by Nominal / Uncertainty Coefficient / Symmetric",
                "Value",
                0.396862275335,
            ),
            (
                "Directional Measures: r by s",
                "Nominal by Nominal / Uncertainty Coefficient / r Dependent",
                "Value",
                0.412589084132,
            ),
            (
                "Directional Measures: r by s",
                "Nominal by Nominal / Uncertainty Coefficient / s Dependent",
                "Value",
                0.382290373553,
            ),
        ],
    );
}

#[test]
fn three_by_two_table_with_a_column_of_one_case() {
    let t = run(
        "CROSSTABS /TABLES=a BY c /STATISTICS=CHISQ PHI CC LAMBDA UC GAMMA D BTAU CTAU CORR /CELLS=COUNT EXPECTED.",
    );
    check(
        &t,
        &[
            ("a by c", "1 / Count", "c / 1", 4.0),
            ("a by c", "1 / Expected", "c / 1", 3.55555555556),
            ("a by c", "1 / Count", "c / 2", 0.0),
            ("a by c", "1 / Expected", "c / 2", 0.444444444444),
            ("a by c", "2 / Count", "c / 1", 2.0),
            ("a by c", "2 / Expected", "c / 1", 2.66666666667),
            ("a by c", "2 / Count", "c / 2", 1.0),
            ("a by c", "2 / Expected", "c / 2", 0.333333333333),
            ("a by c", "3 / Count", "c / 1", 2.0),
            ("a by c", "3 / Expected", "c / 1", 1.77777777778),
            ("a by c", "3 / Count", "c / 2", 0.0),
            ("a by c", "3 / Expected", "c / 2", 0.222222222222),
            ("a by c", "1 / Count", "Total", 4.0),
            ("a by c", "1 / Expected", "Total", 4.0),
            ("a by c", "2 / Count", "Total", 3.0),
            ("a by c", "2 / Expected", "Total", 3.0),
            ("a by c", "3 / Count", "Total", 2.0),
            ("a by c", "3 / Expected", "Total", 2.0),
            ("a by c", "Total / Count", "c / 1", 8.0),
            ("a by c", "Total / Count", "c / 2", 1.0),
            ("a by c", "Total / Count", "Total", 9.0),
            ("Chi-Square Tests: a by c", "N of Valid Cases", "Value", 9.0),
            (
                "Chi-Square Tests: a by c",
                "Pearson Chi-Square",
                "Value",
                2.25,
            ),
            ("Chi-Square Tests: a by c", "Pearson Chi-Square", "df", 2.0),
            (
                "Chi-Square Tests: a by c",
                "Pearson Chi-Square",
                "Asymptotic Sig. (2-tailed)",
                0.324652467358,
            ),
            (
                "Chi-Square Tests: a by c",
                "Likelihood Ratio",
                "Value",
                2.45989271541,
            ),
            (
                "Chi-Square Tests: a by c",
                "Likelihood Ratio",
                "Asymptotic Sig. (2-tailed)",
                0.292308257347,
            ),
            (
                "Chi-Square Tests: a by c",
                "Linear-by-Linear Association",
                "Value",
                0.08,
            ),
            (
                "Chi-Square Tests: a by c",
                "Linear-by-Linear Association",
                "Asymptotic Sig. (2-tailed)",
                0.77729741079,
            ),
            (
                "Symmetric Measures: a by c",
                "Nominal by Nominal / Phi",
                "Value",
                0.5,
            ),
            (
                "Symmetric Measures: a by c",
                "Nominal by Nominal / Cramer's V",
                "Value",
                0.5,
            ),
            (
                "Symmetric Measures: a by c",
                "Nominal by Nominal / Contingency Coefficient",
                "Value",
                0.4472135955,
            ),
            (
                "Symmetric Measures: a by c",
                "Ordinal by Ordinal / Gamma",
                "Value",
                0.333333333333,
            ),
            (
                "Symmetric Measures: a by c",
                "Ordinal by Ordinal / Kendall's tau-b",
                "Value",
                0.138675049056,
            ),
            (
                "Symmetric Measures: a by c",
                "Ordinal by Ordinal / Kendall's tau-c",
                "Value",
                0.0987654320988,
            ),
            (
                "Symmetric Measures: a by c",
                "Ordinal by Ordinal / Spearman Correlation",
                "Value",
                0.146385010942,
            ),
            (
                "Symmetric Measures: a by c",
                "Interval by Interval / Pearson's R",
                "Value",
                0.1,
            ),
            (
                "Directional Measures: a by c",
                "Ordinal by Ordinal / Somers' d / Symmetric",
                "Value",
                0.117647058824,
            ),
            (
                "Directional Measures: a by c",
                "Ordinal by Ordinal / Somers' d / a Dependent",
                "Value",
                0.25,
            ),
            (
                "Directional Measures: a by c",
                "Ordinal by Ordinal / Somers' d / c Dependent",
                "Value",
                0.0769230769231,
            ),
            (
                "Directional Measures: a by c",
                "Nominal by Nominal / Lambda / Symmetric",
                "Value",
                0.166666666667,
            ),
            (
                "Directional Measures: a by c",
                "Nominal by Nominal / Lambda / a Dependent",
                "Value",
                0.2,
            ),
            (
                "Directional Measures: a by c",
                "Nominal by Nominal / Lambda / c Dependent",
                "Value",
                0.0,
            ),
            (
                "Directional Measures: a by c",
                "Nominal by Nominal / Goodman and Kruskal tau / a Dependent",
                "Value",
                0.134615384615,
            ),
            (
                "Directional Measures: a by c",
                "Nominal by Nominal / Goodman and Kruskal tau / c Dependent",
                "Value",
                0.25,
            ),
            (
                "Directional Measures: a by c",
                "Nominal by Nominal / Uncertainty Coefficient / Symmetric",
                "Value",
                0.193887733029,
            ),
            (
                "Directional Measures: a by c",
                "Nominal by Nominal / Uncertainty Coefficient / a Dependent",
                "Value",
                0.128821050546,
            ),
            (
                "Directional Measures: a by c",
                "Nominal by Nominal / Uncertainty Coefficient / c Dependent",
                "Value",
                0.391766434454,
            ),
        ],
    );
}

#[test]
fn table_left_with_one_column_by_a_zero_weight() {
    let t = run(
        "WEIGHT BY w.\nCROSSTABS /TABLES=a BY c /STATISTICS=CHISQ PHI CC /CELLS=COUNT EXPECTED.",
    );
    // a single column has no test: only the count of valid cases is printed
    let tests = t
        .iter()
        .find(|t| t.title == "Chi-Square Tests: a by c")
        .unwrap();
    assert_eq!(tests.rows.len(), 1);
    let measures = t
        .iter()
        .find(|t| t.title == "Symmetric Measures: a by c")
        .unwrap();
    assert_eq!(measures.rows.len(), 1);
    check(
        &t,
        &[
            ("a by c", "1 / Count", "c / 1", 4.0),
            ("a by c", "1 / Expected", "c / 1", 4.0),
            ("a by c", "2 / Count", "c / 1", 2.0),
            ("a by c", "2 / Expected", "c / 1", 2.0),
            ("a by c", "3 / Count", "c / 1", 2.0),
            ("a by c", "3 / Expected", "c / 1", 2.0),
            ("a by c", "1 / Count", "Total", 4.0),
            ("a by c", "1 / Expected", "Total", 4.0),
            ("a by c", "2 / Count", "Total", 2.0),
            ("a by c", "2 / Expected", "Total", 2.0),
            ("a by c", "3 / Count", "Total", 2.0),
            ("a by c", "3 / Expected", "Total", 2.0),
            ("a by c", "Total / Count", "c / 1", 8.0),
            ("a by c", "Total / Count", "Total", 8.0),
            ("Chi-Square Tests: a by c", "N of Valid Cases", "Value", 8.0),
        ],
    );
}
