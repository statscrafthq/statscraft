//! Edge cases of NPAR TESTS against R on eight invented cases: a variable that is tied in every
//! case, paired variables with no difference at all, groups of one case, a zero weight and a
//! weight of two, for Mann-Whitney, Kruskal-Wallis, Wilcoxon, sign, runs, binomial, chi-square,
//! Friedman and median tests. R follows the formulas of the algorithms reference written out on
//! the cases repeated as often as their weight (`pwilcox`, `pbinom`, `pchisq`, `chisq.test`,
//! `binom.test`). Where no statistic exists (a variance of zero from ties, no differences, a cut
//! with no case on one side) the cell is empty. Two readings are the engine's: the sign test
//! with no difference at all has the probability 1, and the Mann-Whitney exact row is
//! `2 * pwilcox` capped at 1 whatever the ties. Scripts in lab/oracles/edge-r3.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g a b c d e w (7F8.2).
BEGIN DATA.
1 5 1 2 2 0 1
1 5 4 4 4 1 1
1 5 2 6 6 0 2
2 5 8 3 3 1 1
2 5 6 5 5 1 0
3 5 3 7 7 1 1
3 5 9 1 1 1 1
3 5 7 8 8 0 1
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

/// The row headers (the leading columns without a title) joined with " / ", an empty first
/// header carried from the row above.
fn row_keys(table: &Table) -> Vec<String> {
    let headers = table
        .column_titles()
        .iter()
        .take_while(|t| t.is_empty())
        .count();
    let mut carried = String::new();
    table
        .rows
        .iter()
        .map(|row| {
            let text = |c: &Cell| match c {
                Cell::Text(t) => t.clone(),
                Cell::Number(n) => format!("{n}"),
                Cell::Empty => String::new(),
            };
            let first = text(&row[0]);
            if !first.is_empty() {
                carried = first;
            }
            let second = if headers > 1 {
                text(&row[1])
            } else {
                String::new()
            };
            if second.is_empty() {
                carried.clone()
            } else {
                format!("{carried} / {second}")
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
fn mann_whitney_unweighted() {
    let t = run("NPAR TESTS /M-W=a b BY g(1,2).");
    check(
        &t,
        &[
            ("Ranks", "a / 1", "N", 3.0),
            ("Ranks", "a / 1", "Mean Rank", 3.0),
            ("Ranks", "a / 1", "Sum of Ranks", 9.0),
            ("Ranks", "a / 2", "N", 2.0),
            ("Ranks", "a / 2", "Mean Rank", 3.0),
            ("Ranks", "a / 2", "Sum of Ranks", 6.0),
            ("Mann-Whitney Test", "Mann-Whitney U", "a", 3.0),
            ("Mann-Whitney Test", "Z", "a", f64::NAN),
            ("Mann-Whitney Test", "Asymp. Sig. (2-tailed)", "a", f64::NAN),
            (
                "Mann-Whitney Test",
                "Exact Sig. [2*(1-tailed Sig.)]",
                "a",
                1.0,
            ),
            ("Ranks", "b / 1", "N", 3.0),
            ("Ranks", "b / 1", "Mean Rank", 2.0),
            ("Ranks", "b / 1", "Sum of Ranks", 6.0),
            ("Ranks", "b / 2", "N", 2.0),
            ("Ranks", "b / 2", "Mean Rank", 4.5),
            ("Ranks", "b / 2", "Sum of Ranks", 9.0),
            ("Mann-Whitney Test", "Mann-Whitney U", "b", 0.0),
            ("Mann-Whitney Test", "Z", "b", -1.73205080757),
            (
                "Mann-Whitney Test",
                "Asymp. Sig. (2-tailed)",
                "b",
                0.0832645166636,
            ),
            (
                "Mann-Whitney Test",
                "Exact Sig. [2*(1-tailed Sig.)]",
                "b",
                0.2,
            ),
            ("Mann-Whitney Test", "Wilcoxon W", "b", 6.0),
        ],
    );
}

#[test]
fn kruskal_wallis_unweighted() {
    let t = run("NPAR TESTS /K-W=a b BY g(1,3).");
    check(
        &t,
        &[
            ("Ranks", "a / 1", "Mean Rank", 4.5),
            ("Ranks", "a / 2", "Mean Rank", 4.5),
            ("Ranks", "a / 3", "Mean Rank", 4.5),
            ("Kruskal-Wallis Test", "Chi-Square", "a", f64::NAN),
            ("Kruskal-Wallis Test", "df", "a", f64::NAN),
            ("Kruskal-Wallis Test", "Asymp. Sig.", "a", f64::NAN),
            ("Ranks", "b / 1", "Mean Rank", 2.33333333333),
            ("Ranks", "b / 2", "Mean Rank", 6.0),
            ("Ranks", "b / 3", "Mean Rank", 5.66666666667),
            ("Kruskal-Wallis Test", "Chi-Square", "b", 3.77777777778),
            ("Kruskal-Wallis Test", "df", "b", 2.0),
            ("Kruskal-Wallis Test", "Asymp. Sig.", "b", 0.15123975969),
        ],
    );
}

#[test]
fn wilcoxon_unweighted() {
    let t = run("NPAR TESTS /WILCOXON=c b WITH d c (PAIRED).");
    check(
        &t,
        &[
            ("Ranks", "c - d / Negative Ranks", "N", 0.0),
            ("Ranks", "c - d / Positive Ranks", "N", 0.0),
            ("Ranks", "c - d / Ties", "N", 8.0),
            ("Ranks", "c - d / Negative Ranks", "Sum of Ranks", 0.0),
            ("Ranks", "c - d / Positive Ranks", "Sum of Ranks", 0.0),
            ("Ranks", "c - d / Negative Ranks", "Mean Rank", f64::NAN),
            ("Ranks", "c - d / Positive Ranks", "Mean Rank", f64::NAN),
            ("Wilcoxon Signed Ranks Test", "Z", "c - d", f64::NAN),
            (
                "Wilcoxon Signed Ranks Test",
                "Asymp. Sig. (2-tailed)",
                "c - d",
                f64::NAN,
            ),
            ("Ranks", "b - c / Negative Ranks", "N", 4.0),
            ("Ranks", "b - c / Positive Ranks", "N", 3.0),
            ("Ranks", "b - c / Ties", "N", 1.0),
            ("Ranks", "b - c / Negative Ranks", "Sum of Ranks", 13.0),
            ("Ranks", "b - c / Positive Ranks", "Sum of Ranks", 15.0),
            ("Ranks", "b - c / Negative Ranks", "Mean Rank", 3.25),
            ("Ranks", "b - c / Positive Ranks", "Mean Rank", 5.0),
            ("Wilcoxon Signed Ranks Test", "Z", "b - c", -0.170560573084),
            (
                "Wilcoxon Signed Ranks Test",
                "Asymp. Sig. (2-tailed)",
                "b - c",
                0.864569301687,
            ),
        ],
    );
}

#[test]
fn sign_unweighted() {
    let t = run("NPAR TESTS /SIGN=c b WITH d c (PAIRED).");
    check(
        &t,
        &[
            (
                "Sign Test Frequencies",
                "c - d / Negative Differences",
                "N",
                0.0,
            ),
            (
                "Sign Test Frequencies",
                "c - d / Positive Differences",
                "N",
                0.0,
            ),
            ("Sign Test Frequencies", "c - d / Ties", "N", 8.0),
            ("Sign Test", "Exact Sig. (2-tailed)", "c - d", 1.0),
            (
                "Sign Test Frequencies",
                "b - c / Negative Differences",
                "N",
                4.0,
            ),
            (
                "Sign Test Frequencies",
                "b - c / Positive Differences",
                "N",
                3.0,
            ),
            ("Sign Test Frequencies", "b - c / Ties", "N", 1.0),
            ("Sign Test", "Exact Sig. (2-tailed)", "b - c", 1.0),
        ],
    );
}

#[test]
fn runs_unweighted() {
    let t = run("NPAR TESTS /RUNS(MEDIAN)=a b.");
    check(
        &t,
        &[
            ("Runs Test", "Test Value", "a", 5.0),
            ("Runs Test", "Cases < Test Value", "a", 0.0),
            ("Runs Test", "Cases >= Test Value", "a", 8.0),
            ("Runs Test", "Number of Runs", "a", 1.0),
            ("Runs Test", "Z", "a", f64::NAN),
            ("Runs Test", "Asymp. Sig. (2-tailed)", "a", f64::NAN),
            ("Runs Test", "Test Value", "b", 5.0),
            ("Runs Test", "Cases < Test Value", "b", 4.0),
            ("Runs Test", "Cases >= Test Value", "b", 4.0),
            ("Runs Test", "Number of Runs", "b", 4.0),
            ("Runs Test", "Z", "b", -0.381881307913),
            ("Runs Test", "Asymp. Sig. (2-tailed)", "b", 0.702549405576),
        ],
    );
}

#[test]
fn binomial_unweighted() {
    let t = run("NPAR TESTS /BINOMIAL(0.5)=e.");
    check(
        &t,
        &[
            ("Binomial Test", "e / Group 1", "N", 3.0),
            ("Binomial Test", "e / Group 2", "N", 5.0),
            (
                "Binomial Test",
                "e / Group 1",
                "Exact Sig. (2-tailed)",
                0.7265625,
            ),
        ],
    );
}

#[test]
fn chi_square_unweighted() {
    let t = run("NPAR TESTS /CHISQUARE=g.");
    check(
        &t,
        &[
            ("Chi-Square Test Statistics", "Chi-Square", "g", 0.25),
            ("Chi-Square Test Statistics", "df", "g", 2.0),
            (
                "Chi-Square Test Statistics",
                "Asymp. Sig.",
                "g",
                0.882496902585,
            ),
            ("Chi-Square Frequencies: g", "1", "Observed N", 3.0),
            ("Chi-Square Frequencies: g", "2", "Observed N", 2.0),
            ("Chi-Square Frequencies: g", "3", "Observed N", 3.0),
        ],
    );
}

#[test]
fn friedman_unweighted() {
    let t = run("NPAR TESTS /FRIEDMAN=b c d.");
    check(
        &t,
        &[
            ("Ranks", "b", "Mean Rank", 1.875),
            ("Ranks", "c", "Mean Rank", 2.0625),
            ("Ranks", "d", "Mean Rank", 2.0625),
            ("Friedman Test", "N", "Value", 8.0),
            ("Friedman Test", "Chi-Square", "Value", 0.285714285714),
            ("Friedman Test", "df", "Value", 2.0),
            ("Friedman Test", "Asymp. Sig.", "Value", 0.86687789975),
        ],
    );
}

#[test]
fn friedman_tied_everywhere_unweighted() {
    let t = run("NPAR TESTS /FRIEDMAN=c d d.");
    check(
        &t,
        &[
            ("Ranks", "c", "Mean Rank", 1.5),
            ("Ranks", "d", "Mean Rank", 1.5),
            ("Friedman Test", "Chi-Square", "Value", f64::NAN),
            ("Friedman Test", "Asymp. Sig.", "Value", f64::NAN),
        ],
    );
}

#[test]
fn median_unweighted() {
    let t = run("NPAR TESTS /MEDIAN=b BY g(1,3).");
    check(
        &t,
        &[
            ("Median Test", "Median", "b", 5.0),
            ("Median Test Frequencies", "b / > Median", "g / 1", 0.0),
            ("Median Test Frequencies", "b / <= Median", "g / 1", 3.0),
            ("Median Test Frequencies", "b / > Median", "g / 2", 2.0),
            ("Median Test Frequencies", "b / <= Median", "g / 2", 0.0),
            ("Median Test Frequencies", "b / > Median", "g / 3", 2.0),
            ("Median Test Frequencies", "b / <= Median", "g / 3", 1.0),
            ("Median Test", "Chi-Square", "b", 5.33333333333),
            ("Median Test", "Asymp. Sig.", "b", 0.0694834512228),
        ],
    );
}

#[test]
fn mann_whitney_weighted() {
    let t = run("WEIGHT BY w.\nNPAR TESTS /M-W=a b BY g(1,2).");
    check(
        &t,
        &[
            ("Ranks", "a / 1", "N", 4.0),
            ("Ranks", "a / 1", "Mean Rank", 3.0),
            ("Ranks", "a / 1", "Sum of Ranks", 12.0),
            ("Ranks", "a / 2", "N", 1.0),
            ("Ranks", "a / 2", "Mean Rank", 3.0),
            ("Ranks", "a / 2", "Sum of Ranks", 3.0),
            ("Mann-Whitney Test", "Mann-Whitney U", "a", 2.0),
            ("Mann-Whitney Test", "Z", "a", f64::NAN),
            ("Mann-Whitney Test", "Asymp. Sig. (2-tailed)", "a", f64::NAN),
            (
                "Mann-Whitney Test",
                "Exact Sig. [2*(1-tailed Sig.)]",
                "a",
                1.0,
            ),
            ("Ranks", "b / 1", "N", 4.0),
            ("Ranks", "b / 1", "Mean Rank", 2.5),
            ("Ranks", "b / 1", "Sum of Ranks", 10.0),
            ("Ranks", "b / 2", "N", 1.0),
            ("Ranks", "b / 2", "Mean Rank", 5.0),
            ("Ranks", "b / 2", "Sum of Ranks", 5.0),
            ("Mann-Whitney Test", "Mann-Whitney U", "b", 0.0),
            ("Mann-Whitney Test", "Z", "b", -1.45095250022),
            (
                "Mann-Whitney Test",
                "Asymp. Sig. (2-tailed)",
                "b",
                0.146793087309,
            ),
            (
                "Mann-Whitney Test",
                "Exact Sig. [2*(1-tailed Sig.)]",
                "b",
                0.4,
            ),
            ("Mann-Whitney Test", "Wilcoxon W", "b", 10.0),
        ],
    );
}

#[test]
fn kruskal_wallis_weighted() {
    let t = run("WEIGHT BY w.\nNPAR TESTS /K-W=a b BY g(1,3).");
    check(
        &t,
        &[
            ("Ranks", "a / 1", "Mean Rank", 4.5),
            ("Ranks", "a / 2", "Mean Rank", 4.5),
            ("Ranks", "a / 3", "Mean Rank", 4.5),
            ("Kruskal-Wallis Test", "Chi-Square", "a", f64::NAN),
            ("Kruskal-Wallis Test", "df", "a", f64::NAN),
            ("Kruskal-Wallis Test", "Asymp. Sig.", "a", f64::NAN),
            ("Ranks", "b / 1", "Mean Rank", 2.75),
            ("Ranks", "b / 2", "Mean Rank", 7.0),
            ("Ranks", "b / 3", "Mean Rank", 6.0),
            ("Kruskal-Wallis Test", "Chi-Square", "b", 4.25903614458),
            ("Kruskal-Wallis Test", "df", "b", 2.0),
            ("Kruskal-Wallis Test", "Asymp. Sig.", "b", 0.11889457864),
        ],
    );
}

#[test]
fn wilcoxon_weighted() {
    let t = run("WEIGHT BY w.\nNPAR TESTS /WILCOXON=c b WITH d c (PAIRED).");
    check(
        &t,
        &[
            ("Ranks", "c - d / Negative Ranks", "N", 0.0),
            ("Ranks", "c - d / Positive Ranks", "N", 0.0),
            ("Ranks", "c - d / Ties", "N", 8.0),
            ("Ranks", "c - d / Negative Ranks", "Sum of Ranks", 0.0),
            ("Ranks", "c - d / Positive Ranks", "Sum of Ranks", 0.0),
            ("Ranks", "c - d / Negative Ranks", "Mean Rank", f64::NAN),
            ("Ranks", "c - d / Positive Ranks", "Mean Rank", f64::NAN),
            ("Wilcoxon Signed Ranks Test", "Z", "c - d", f64::NAN),
            (
                "Wilcoxon Signed Ranks Test",
                "Asymp. Sig. (2-tailed)",
                "c - d",
                f64::NAN,
            ),
            ("Ranks", "b - c / Negative Ranks", "N", 5.0),
            ("Ranks", "b - c / Positive Ranks", "N", 2.0),
            ("Ranks", "b - c / Ties", "N", 1.0),
            ("Ranks", "b - c / Negative Ranks", "Sum of Ranks", 15.0),
            ("Ranks", "b - c / Positive Ranks", "Sum of Ranks", 13.0),
            ("Ranks", "b - c / Negative Ranks", "Mean Rank", 3.0),
            ("Ranks", "b - c / Positive Ranks", "Mean Rank", 6.5),
            ("Wilcoxon Signed Ranks Test", "Z", "b - c", -0.170560573084),
            (
                "Wilcoxon Signed Ranks Test",
                "Asymp. Sig. (2-tailed)",
                "b - c",
                0.864569301687,
            ),
        ],
    );
}

#[test]
fn sign_weighted() {
    let t = run("WEIGHT BY w.\nNPAR TESTS /SIGN=c b WITH d c (PAIRED).");
    check(
        &t,
        &[
            (
                "Sign Test Frequencies",
                "c - d / Negative Differences",
                "N",
                0.0,
            ),
            (
                "Sign Test Frequencies",
                "c - d / Positive Differences",
                "N",
                0.0,
            ),
            ("Sign Test Frequencies", "c - d / Ties", "N", 8.0),
            ("Sign Test", "Exact Sig. (2-tailed)", "c - d", 1.0),
            (
                "Sign Test Frequencies",
                "b - c / Negative Differences",
                "N",
                5.0,
            ),
            (
                "Sign Test Frequencies",
                "b - c / Positive Differences",
                "N",
                2.0,
            ),
            ("Sign Test Frequencies", "b - c / Ties", "N", 1.0),
            ("Sign Test", "Exact Sig. (2-tailed)", "b - c", 0.453125),
        ],
    );
}

#[test]
fn runs_weighted() {
    let t = run("WEIGHT BY w.\nNPAR TESTS /RUNS(MEDIAN)=a b.");
    check(
        &t,
        &[
            ("Runs Test", "Test Value", "a", 5.0),
            ("Runs Test", "Cases < Test Value", "a", 0.0),
            ("Runs Test", "Cases >= Test Value", "a", 8.0),
            ("Runs Test", "Number of Runs", "a", 1.0),
            ("Runs Test", "Z", "a", f64::NAN),
            ("Runs Test", "Asymp. Sig. (2-tailed)", "a", f64::NAN),
            ("Runs Test", "Test Value", "b", 3.5),
            ("Runs Test", "Cases < Test Value", "b", 4.0),
            ("Runs Test", "Cases >= Test Value", "b", 4.0),
            ("Runs Test", "Number of Runs", "b", 6.0),
            ("Runs Test", "Z", "b", 0.381881307913),
            ("Runs Test", "Asymp. Sig. (2-tailed)", "b", 0.702549405576),
        ],
    );
}

#[test]
fn binomial_weighted() {
    let t = run("WEIGHT BY w.\nNPAR TESTS /BINOMIAL(0.5)=e.");
    check(
        &t,
        &[
            ("Binomial Test", "e / Group 1", "N", 4.0),
            ("Binomial Test", "e / Group 2", "N", 4.0),
            ("Binomial Test", "e / Group 1", "Exact Sig. (2-tailed)", 1.0),
        ],
    );
}

#[test]
fn chi_square_weighted() {
    let t = run("WEIGHT BY w.\nNPAR TESTS /CHISQUARE=g.");
    check(
        &t,
        &[
            ("Chi-Square Test Statistics", "Chi-Square", "g", 1.75),
            ("Chi-Square Test Statistics", "df", "g", 2.0),
            (
                "Chi-Square Test Statistics",
                "Asymp. Sig.",
                "g",
                0.416862019679,
            ),
            ("Chi-Square Frequencies: g", "1", "Observed N", 4.0),
            ("Chi-Square Frequencies: g", "2", "Observed N", 1.0),
            ("Chi-Square Frequencies: g", "3", "Observed N", 3.0),
        ],
    );
}

#[test]
fn friedman_weighted() {
    let t = run("WEIGHT BY w.\nNPAR TESTS /FRIEDMAN=b c d.");
    check(
        &t,
        &[
            ("Ranks", "b", "Mean Rank", 1.625),
            ("Ranks", "c", "Mean Rank", 2.1875),
            ("Ranks", "d", "Mean Rank", 2.1875),
            ("Friedman Test", "N", "Value", 8.0),
            ("Friedman Test", "Chi-Square", "Value", 2.57142857143),
            ("Friedman Test", "df", "Value", 2.0),
            ("Friedman Test", "Asymp. Sig.", "Value", 0.27645304663),
        ],
    );
}

#[test]
fn friedman_tied_everywhere_weighted() {
    let t = run("WEIGHT BY w.\nNPAR TESTS /FRIEDMAN=c d d.");
    check(
        &t,
        &[
            ("Ranks", "c", "Mean Rank", 1.5),
            ("Ranks", "d", "Mean Rank", 1.5),
            ("Friedman Test", "Chi-Square", "Value", f64::NAN),
            ("Friedman Test", "Asymp. Sig.", "Value", f64::NAN),
        ],
    );
}

#[test]
fn median_weighted() {
    let t = run("WEIGHT BY w.\nNPAR TESTS /MEDIAN=b BY g(1,3).");
    check(
        &t,
        &[
            ("Median Test", "Median", "b", 3.5),
            ("Median Test Frequencies", "b / > Median", "g / 1", 1.0),
            ("Median Test Frequencies", "b / <= Median", "g / 1", 3.0),
            ("Median Test Frequencies", "b / > Median", "g / 2", 1.0),
            ("Median Test Frequencies", "b / <= Median", "g / 2", 0.0),
            ("Median Test Frequencies", "b / > Median", "g / 3", 2.0),
            ("Median Test Frequencies", "b / <= Median", "g / 3", 1.0),
            ("Median Test", "Chi-Square", "b", 2.33333333333),
            ("Median Test", "Asymp. Sig.", "b", 0.311403223915),
        ],
    );
}
