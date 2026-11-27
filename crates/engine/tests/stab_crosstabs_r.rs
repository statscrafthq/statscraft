//! Numerical stability of CROSSTABS against R on the ten invented cases of edge_crosstabs_r with
//! every case weighted 250000 (counts in the millions, zero cells and missing values kept): the
//! chi-squares, phi, V, C, gamma, tau-b, tau-c, Spearman, Pearson, Somers' d, lambda, Goodman
//! and Kruskal tau, the uncertainty coefficient, kappa and the risk estimates, from the
//! formulas of the algorithms reference written out in R, to nine digits. The significances
//! underflow and are not compared. Scripts in lab/oracles/stab-r3.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /a b c w r s (6F8.2).
BEGIN DATA.
1 1 1 250000 1 1
1 1 1 250000 1 1
1 2 1 250000 1 1
2 1 1 250000 2 1
2 2 1 250000 2 1
2 2 2 250000 2 2
3 1 1 250000 2 2
3 3 1 250000 2 2
. 1 1 250000 2 2
1 . 1 250000 . .
END DATA.
WEIGHT BY w.
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
            (n - want).abs() <= 1e-9 * want.abs(),
            "{title} / {row} / {column}: {n} is not {want}"
        );
    }
}

#[test]
fn weighted_square_table_with_counts_in_the_millions() {
    let t = run(
        "CROSSTABS /TABLES=a BY b /STATISTICS=CHISQ PHI CC LAMBDA UC GAMMA D BTAU CTAU CORR KAPPA /CELLS=COUNT EXPECTED.",
    );
    check(
        &t,
        &[
            ("a by b", "1 / Count", "b / 1", 500000.0),
            ("a by b", "1 / Expected", "b / 1", 375000.0),
            ("a by b", "1 / Count", "b / 2", 250000.0),
            ("a by b", "1 / Expected", "b / 2", 281250.0),
            ("a by b", "1 / Count", "b / 3", 0.0),
            ("a by b", "1 / Expected", "b / 3", 93750.0),
            ("a by b", "2 / Count", "b / 1", 250000.0),
            ("a by b", "2 / Expected", "b / 1", 375000.0),
            ("a by b", "2 / Count", "b / 2", 500000.0),
            ("a by b", "2 / Expected", "b / 2", 281250.0),
            ("a by b", "2 / Count", "b / 3", 0.0),
            ("a by b", "2 / Expected", "b / 3", 93750.0),
            ("a by b", "3 / Count", "b / 1", 250000.0),
            ("a by b", "3 / Expected", "b / 1", 250000.0),
            ("a by b", "3 / Count", "b / 2", 0.0),
            ("a by b", "3 / Expected", "b / 2", 187500.0),
            ("a by b", "3 / Count", "b / 3", 250000.0),
            ("a by b", "3 / Expected", "b / 3", 62500.0),
            ("a by b", "1 / Count", "Total", 750000.0),
            ("a by b", "1 / Expected", "Total", 750000.0),
            ("a by b", "2 / Count", "Total", 750000.0),
            ("a by b", "2 / Expected", "Total", 750000.0),
            ("a by b", "3 / Count", "Total", 500000.0),
            ("a by b", "3 / Expected", "Total", 500000.0),
            ("a by b", "Total / Count", "b / 1", 1000000.0),
            ("a by b", "Total / Count", "b / 2", 750000.0),
            ("a by b", "Total / Count", "b / 3", 250000.0),
            ("a by b", "Total / Count", "Total", 2000000.0),
            (
                "Chi-Square Tests: a by b",
                "N of Valid Cases",
                "Value",
                2000000.0,
            ),
            (
                "Chi-Square Tests: a by b",
                "Pearson Chi-Square",
                "Value",
                1194444.44444,
            ),
            ("Chi-Square Tests: a by b", "Pearson Chi-Square", "df", 4.0),
            (
                "Chi-Square Tests: a by b",
                "Likelihood Ratio",
                "Value",
                1294569.32603,
            ),
            (
                "Chi-Square Tests: a by b",
                "Linear-by-Linear Association",
                "Value",
                279569.752688,
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
fn weighted_two_by_two_table_with_counts_in_the_millions() {
    let t = run(
        "CROSSTABS /TABLES=r BY s /STATISTICS=CHISQ PHI CC LAMBDA UC GAMMA D BTAU CTAU CORR KAPPA RISK MCNEMAR /CELLS=COUNT EXPECTED.",
    );
    check(
        &t,
        &[
            ("r by s", "1 / Count", "s / 1", 750000.0),
            ("r by s", "1 / Expected", "s / 1", 416666.666667),
            ("r by s", "1 / Count", "s / 2", 0.0),
            ("r by s", "1 / Expected", "s / 2", 333333.333333),
            ("r by s", "2 / Count", "s / 1", 500000.0),
            ("r by s", "2 / Expected", "s / 1", 833333.333333),
            ("r by s", "2 / Count", "s / 2", 1000000.0),
            ("r by s", "2 / Expected", "s / 2", 666666.666667),
            ("r by s", "1 / Count", "Total", 750000.0),
            ("r by s", "1 / Expected", "Total", 750000.0),
            ("r by s", "2 / Count", "Total", 1500000.0),
            ("r by s", "2 / Expected", "Total", 1500000.0),
            ("r by s", "Total / Count", "s / 1", 1250000.0),
            ("r by s", "Total / Count", "s / 2", 1000000.0),
            ("r by s", "Total / Count", "Total", 2250000.0),
            (
                "Chi-Square Tests: r by s",
                "N of Valid Cases",
                "Value",
                2250000.0,
            ),
            (
                "Chi-Square Tests: r by s",
                "Pearson Chi-Square",
                "Value",
                900000.0,
            ),
            ("Chi-Square Tests: r by s", "Pearson Chi-Square", "df", 1.0),
            (
                "Chi-Square Tests: r by s",
                "Likelihood Ratio",
                "Value",
                1181784.5898,
            ),
            (
                "Chi-Square Tests: r by s",
                "Linear-by-Linear Association",
                "Value",
                899999.6,
            ),
            (
                "Chi-Square Tests: r by s",
                "Continuity Correction",
                "Value",
                899997.300002,
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
                2.99321816272,
            ),
            (
                "Risk Estimate: r by s",
                "For cohort s = 1",
                "95% Confidence Interval / Upper",
                3.00679720312,
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
