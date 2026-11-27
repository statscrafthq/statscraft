//! NPAR TESTS subcommands that had no check against R, on 30 invented cases: CHISQUARE with
//! EXPECTED ratios and a range (`chisq.test`), BINOMIAL with a test proportion other than one half
//! (one-tailed), with a cut point and with a pair of values (`pbinom`, `binom.test`), RUNS cut at
//! the mode and at a value (the manual's formula with the continuity correction), K-S against the
//! uniform, exponential, Poisson and normal with given parameters (the manual's distances and the
//! first terms of the Smirnov series with the constants this part of the manual prints), MH
//! (marginal homogeneity with the values as scores), STATISTICS=DESCRIPTIVES QUARTILES (R's
//! `quantile` type 6), and METHOD=EXACT for Mann-Whitney (the distribution of the rank sum with
//! ties by dynamic programming), Wilcoxon (the same for signed ranks) and the sign test
//! (`binom.test`), MOSES (the span and the trimmed span) and W-W (the true bounds of the number
//! of runs with ties by brute force, the normal and exact probabilities). Scripts in
//! lab/oracles/npar-r2.
//!
//! Two readings are on record. The first Mann-Whitney exact row, "Exact Sig. [2*(1-tailed Sig.)]",
//! is the algorithm of the manual without ties (`2 * pwilcox`), and the rows after it come from the
//! distribution of the tied ranks. A Moses span that is not a whole number (tied ranks give halves)
//! is rounded up from a half, where R's `round` goes to the even number.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g y u e k x cat4 b pre post m1 m2.
BEGIN DATA.
1 22 9.5 2.87 2 41 2 1 23.8 25.7 1 1
1 18.4 4 13.28 2 41 4 1 29.1 39.8 2 3
1 18.4 10 7.63 1 59 3 2 25.7 25 1 1
1 18.4 2.1 3.22 0 36 3 1 33.1 35.4 1 1
1 26.6 10.9 5.78 5 33 4 2 31.7 32.1 1 1
1 22.5 2.6 10.16 4 44 2 1 33.4 35.5 1 2
1 20.8 3.1 0.39 2 58 3 1 30.8 35.5 3 3
1 24.4 4.6 7.72 2 41 4 2 36.4 37.9 2 2
1 19.2 11.4 1.63 5 63 3 1 22.8 21.4 1 1
1 18.5 8.1 0.99 1 42 3 1 22.2 26.2 2 1
1 18.8 3.1 1.36 3 46 2 2 26.7 29.7 2 1
1 20.2 9.7 1.33 6 58 4 1 30 34 3 2
1 12.5 11.5 1.04 4 46 2 2 26.2 31.5 1 1
2 23.8 6.1 1.3 4 54 4 2 38.2 36.7 3 3
2 37.9 5.1 3.81 2 59 3 2 30.4 34.3 3 3
2 29.8 11.1 26.84 1 52 3 1 32.7 37.2 2 2
2 29.8 4.8 0.58 5 55 2 1 21.1 14.3 3 2
2 20.7 8.7 0.7 3 39 4 2 18.7 23.2 1 1
2 28 8.1 5.66 6 63 4 1 35.9 40.8 2 3
2 20.7 5.9 3.65 2 61 2 1 21.9 21.2 2 1
2 11.8 5.8 1.8 4 52 4 2 24.4 26.4 1 1
2 25.4 6.7 1.48 6 55 2 1 16.4 17.8 2 2
2 19.3 5.4 2.06 6 36 4 1 25.2 25.4 1 2
2 31.6 6.2 4.08 4 49 3 2 28.7 29.7 2 2
2 32.6 4.3 0.85 4 63 1 2 44.4 46.8 1 1
2 26.6 6.8 0.32 2 56 3 1 30.8 38 3 3
2 13.5 4 8.01 4 52 1 1 28.5 30.6 2 2
2 23.8 4.5 5.41 3 46 2 2 28.3 30 1 1
2 15.1 9.3 3.31 4 53 2 2 19.2 19.5 1 1
2 20.8 10.9 2.83 4 39 2 1 39.2 38.5 3 2
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

/// A number of the `nth` table with this title.
fn value(tables: &[Table], title: &str, nth: usize, row: &str, column: &str) -> f64 {
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

#[test]
fn chi_square_binomial_and_runs_agree_with_r() {
    let t = run("NPAR TESTS /CHISQUARE=cat4 /EXPECTED=1 3 3 2 /CHISQUARE=k (2,6).");
    // statistic, df, p, then the observed and the expected counts of the four categories
    let ratios = [
        1.45,
        3.0,
        0.6938574282,
        2.0,
        10.0,
        9.0,
        9.0,
        3.333333333,
        10.0,
        10.0,
        6.666666667,
    ];
    for (i, row) in ["1", "2", "3", "4"].iter().enumerate() {
        near(
            value(&t, "Chi-Square Frequencies: cat4", 0, row, "Observed N"),
            ratios[3 + i],
            row,
        );
        near(
            value(&t, "Chi-Square Frequencies: cat4", 0, row, "Expected N"),
            ratios[7 + i],
            row,
        );
    }
    near(
        value(&t, "Chi-Square Test Statistics", 0, "Chi-Square", "cat4"),
        ratios[0],
        "chi-square",
    );
    near(
        value(&t, "Chi-Square Test Statistics", 0, "Asymp. Sig.", "cat4"),
        ratios[2],
        "p",
    );
    // the range 2 to 6 takes 26 of the 30 cases, five equal categories
    let range = [5.538461538, 4.0, 0.236369309, 7.0, 3.0, 9.0, 3.0, 4.0, 26.0];
    for (i, row) in ["2", "3", "4", "5", "6"].iter().enumerate() {
        near(
            value(&t, "Chi-Square Frequencies", 0, row, "k / Observed N"),
            range[3 + i],
            row,
        );
        near(
            value(&t, "Chi-Square Frequencies", 0, row, "k / Expected N"),
            range[8] / 5.0,
            row,
        );
    }
    near(
        value(&t, "Chi-Square Test Statistics", 1, "Chi-Square", "k"),
        range[0],
        "range chi-square",
    );
    near(
        value(&t, "Chi-Square Test Statistics", 1, "Asymp. Sig.", "k"),
        range[2],
        "range p",
    );
    // BINOMIAL(0.3): one-tailed; a cut point; two values with the test proportion 0.4
    let t = run("NPAR TESTS /BINOMIAL(0.3)=b /BINOMIAL=x (50) /BINOMIAL(0.4)=cat4 (1,3).");
    let (b, x, c) = (
        [17.0, 13.0, 0.5666666667, 0.002124690751],
        [14.0, 16.0, 0.4666666667, 0.8555355519],
        [2.0, 9.0, 0.1818181818, 0.1189168128],
    );
    near(
        value(&t, "Binomial Test", 0, "b / Group 1", "N"),
        b[0],
        "b n1",
    );
    near(
        value(&t, "Binomial Test", 0, "b / Group 1", "Observed Prop."),
        b[2],
        "b observed",
    );
    near(
        value(
            &t,
            "Binomial Test",
            0,
            "b / Group 1",
            "Exact Sig. (1-tailed)",
        ),
        b[3],
        "b p",
    );
    near(
        value(&t, "Binomial Test", 1, "x / Group 1", "N"),
        x[0],
        "x n1",
    );
    near(
        value(
            &t,
            "Binomial Test",
            1,
            "x / Group 1",
            "Exact Sig. (2-tailed)",
        ),
        x[3],
        "x p",
    );
    near(
        value(&t, "Binomial Test", 2, "cat4 / Group 1", "N"),
        c[0],
        "cat4 n1",
    );
    near(
        value(&t, "Binomial Test", 2, "cat4 / Group 2", "N"),
        c[1],
        "cat4 n3",
    );
    near(
        value(
            &t,
            "Binomial Test",
            2,
            "cat4 / Group 1",
            "Exact Sig. (1-tailed)",
        ),
        c[3],
        "cat4 p",
    );
    // RUNS at the mode of k (4) and at the value 50
    let t = run("NPAR TESTS /RUNS(MODE)=k /RUNS(50)=x.");
    for (nth, variable, w) in [
        (0, "k", [4.0, 14.0, 16.0, 30.0, 16.0, 0.0, 1.0]),
        (
            1,
            "x",
            [50.0, 14.0, 16.0, 30.0, 17.0, 0.2115537716, 0.8324551778],
        ),
    ] {
        near(
            value(&t, "Runs Test", nth, "Test Value", variable),
            w[0],
            variable,
        );
        near(
            value(&t, "Runs Test", nth, "Cases < Test Value", variable),
            w[1],
            variable,
        );
        near(
            value(&t, "Runs Test", nth, "Cases >= Test Value", variable),
            w[2],
            variable,
        );
        near(
            value(&t, "Runs Test", nth, "Number of Runs", variable),
            w[4],
            variable,
        );
        near(value(&t, "Runs Test", nth, "Z", variable), w[5], variable);
        near(
            value(&t, "Runs Test", nth, "Asymp. Sig. (2-tailed)", variable),
            w[6],
            variable,
        );
    }
}

#[test]
fn one_sample_kolmogorov_smirnov_marginal_homogeneity_and_quartiles_agree_with_r() {
    let t = run(
        "NPAR TESTS /K-S(UNIFORM 2 12)=u /K-S(EXPONENTIAL 5)=e /K-S(POISSON 4)=k /K-S(NORMAL 50 10)=x.",
    );
    // |D|, positive, negative, Z, p
    for (nth, variable, w) in [
        (
            0,
            "u",
            [0.12, 0.12, -0.06666666667, 0.657267069, 0.7806718092],
        ),
        (
            1,
            "e",
            [
                0.1421969093,
                0.1421969093,
                -0.06199500047,
                0.7788445482,
                0.5789139944,
            ],
        ),
        (
            2,
            "k",
            [0.1378297315, 0.1378297315, 0.0, 0.7549245303, 0.6188832787],
        ),
        (
            3,
            "x",
            [
                0.1125930428,
                0.09680048459,
                -0.1125930428,
                0.6166974934,
                0.8414318275,
            ],
        ),
    ] {
        let title = "One-Sample Kolmogorov-Smirnov Test";
        near(
            value(
                &t,
                title,
                nth,
                "Most Extreme Differences / Absolute",
                variable,
            ),
            w[0],
            variable,
        );
        near(
            value(
                &t,
                title,
                nth,
                "Most Extreme Differences / Positive",
                variable,
            ),
            w[1],
            variable,
        );
        near(
            value(
                &t,
                title,
                nth,
                "Most Extreme Differences / Negative",
                variable,
            ),
            w[2],
            variable,
        );
        near(
            value(&t, title, nth, "Kolmogorov-Smirnov Z", variable),
            w[3],
            variable,
        );
        near(
            value(&t, title, nth, "Asymp. Sig. (2-tailed)", variable),
            w[4],
            variable,
        );
    }
    let t = run("NPAR TESTS /MH=m1 WITH m2 (PAIRED).");
    let mh = [3.0, 10.0, 21.0, 20.0, 1.58113883, 0.632455532, 0.5270892569];
    let title = "Marginal Homogeneity Test";
    near(
        value(&t, title, 0, "Distinct Values", "m1 & m2"),
        mh[0],
        "distinct",
    );
    near(
        value(&t, title, 0, "Off-Diagonal Cases", "m1 & m2"),
        mh[1],
        "off-diagonal",
    );
    near(
        value(&t, title, 0, "Observed MH Statistic", "m1 & m2"),
        mh[2],
        "statistic",
    );
    near(
        value(&t, title, 0, "Mean MH Statistic", "m1 & m2"),
        mh[3],
        "mean",
    );
    near(
        value(&t, title, 0, "Std. Deviation of MH Statistic", "m1 & m2"),
        mh[4],
        "sd",
    );
    near(
        value(&t, title, 0, "Std. MH Statistic", "m1 & m2"),
        mh[5],
        "z",
    );
    near(
        value(&t, title, 0, "Asymp. Sig. (2-tailed)", "m1 & m2"),
        mh[6],
        "p",
    );
    for (syntax, variable, w) in [
        (
            "NPAR TESTS /M-W=y BY g(1,2) /STATISTICS=DESCRIPTIVES QUARTILES.",
            "y",
            [
                30.0,
                22.39666667,
                6.09355133,
                11.8,
                37.9,
                18.475,
                20.8,
                26.6,
            ],
        ),
        (
            "NPAR TESTS /WILCOXON=m1 WITH m2 (PAIRED) /STATISTICS=DESCRIPTIVES QUARTILES.",
            "m1",
            [30.0, 1.8, 0.8051557999, 1.0, 3.0, 1.0, 2.0, 2.25],
        ),
    ] {
        let t = run(syntax);
        for (c, k) in [
            ("N", 0),
            ("Mean", 1),
            ("Std. Deviation", 2),
            ("Minimum", 3),
            ("Maximum", 4),
            ("Percentiles / 25th", 5),
            ("Percentiles / 50th (Median)", 6),
            ("Percentiles / 75th", 7),
        ] {
            near(
                value(&t, "Descriptive Statistics", 0, variable, c),
                w[k],
                &format!("{variable} {c}"),
            );
        }
    }
}

#[test]
fn exact_tests_moses_and_wald_wolfowitz_agree_with_r() {
    // Mann-Whitney: U, W, Z, the asymptotic p; the rows of METHOD=EXACT
    let t = run("NPAR TESTS /M-W=y BY g(1,2) /METHOD=EXACT.");
    let (mw, exact) = (
        [63.0, 154.0, -1.989951365, 0.04659629326, 0.04662684531],
        [0.02331342265, 0.9778735695, 0.04663093683, 0.001186992135],
    );
    let title = "Mann-Whitney Test";
    near(value(&t, title, 0, "Mann-Whitney U", "y"), mw[0], "U");
    near(value(&t, title, 0, "Wilcoxon W", "y"), mw[1], "W");
    near(value(&t, title, 0, "Z", "y"), mw[2], "Z");
    near(
        value(&t, title, 0, "Asymp. Sig. (2-tailed)", "y"),
        mw[3],
        "asymptotic p",
    );
    // low tail, high tail, two-tailed, point probability of the rank sum with ties
    near(
        value(&t, title, 0, "Exact Sig. (1-tailed)", "y"),
        exact[0],
        "one-tailed",
    );
    near(
        value(&t, title, 0, "Exact Sig. (2-tailed)", "y"),
        exact[2],
        "two-tailed",
    );
    near(
        value(&t, title, 0, "Point Probability", "y"),
        exact[3],
        "point probability",
    );
    // without ties the manual's algorithm: twice the lower tail of the distribution of U
    near(
        value(&t, title, 0, "Exact Sig. [2*(1-tailed Sig.)]", "y"),
        2.0 * [0.02403773886][0],
        "2 * pwilcox",
    );
    // Wilcoxon signed ranks and the sign test
    let t = run(
        "NPAR TESTS /WILCOXON=pre WITH post (PAIRED) /SIGN=pre WITH post (PAIRED) /METHOD=EXACT.",
    );
    let (wx, wz, sg) = (
        [
            0.0001016668975,
            0.9999034479,
            0.0002033337951,
            5.11482358e-06,
        ],
        [-3.507458349, 0.0004524091695],
        [
            0.001430906355,
            0.0007154531777,
            0.0005529960617,
            3.103761159,
        ],
    );
    near(
        value(&t, "Wilcoxon Signed Ranks Test", 0, "Z", "pre - post"),
        wz[0],
        "Wilcoxon Z",
    );
    near(
        value(
            &t,
            "Wilcoxon Signed Ranks Test",
            0,
            "Asymp. Sig. (2-tailed)",
            "pre - post",
        ),
        wz[1],
        "Wilcoxon asymptotic p",
    );
    near(
        value(
            &t,
            "Wilcoxon Signed Ranks Test",
            0,
            "Exact Sig. (1-tailed)",
            "pre - post",
        ),
        wx[0],
        "Wilcoxon one-tailed",
    );
    near(
        value(
            &t,
            "Wilcoxon Signed Ranks Test",
            0,
            "Exact Sig. (2-tailed)",
            "pre - post",
        ),
        wx[2],
        "Wilcoxon two-tailed",
    );
    near(
        value(
            &t,
            "Wilcoxon Signed Ranks Test",
            0,
            "Point Probability",
            "pre - post",
        ),
        wx[3],
        "Wilcoxon point",
    );
    near(
        value(&t, "Sign Test", 0, "Exact Sig. (2-tailed)", "pre - post"),
        sg[0],
        "sign two-tailed",
    );
    near(
        value(&t, "Sign Test", 0, "Exact Sig. (1-tailed)", "pre - post"),
        sg[1],
        "sign one-tailed",
    );
    near(
        value(&t, "Sign Test", 0, "Point Probability", "pre - post"),
        sg[2],
        "sign point",
    );
    near(
        value(&t, "Sign Test", 0, "Z", "pre - post"),
        sg[3],
        "sign Z",
    );
    // Moses: control group 1, the span of its ranks and the span less one case from each end
    let t = run("NPAR TESTS /MOSES=y BY g(1,2).");
    let moses = [22.5, 0.0473496585, 16.0, 0.01266384352, 1.0];
    near(
        value(
            &t,
            "Moses Test",
            0,
            "Observed Control Group Span / Span",
            "y",
        ),
        moses[0] + 0.5,
        "span rounded up from a half",
    );
    near(
        value(
            &t,
            "Moses Test",
            0,
            "Observed Control Group Span / Sig. (1-tailed)",
            "y",
        ),
        moses[1],
        "span p",
    );
    near(
        value(
            &t,
            "Moses Test",
            0,
            "Trimmed Control Group Span / Span",
            "y",
        ),
        moses[2],
        "trimmed span",
    );
    near(
        value(
            &t,
            "Moses Test",
            0,
            "Trimmed Control Group Span / Sig. (1-tailed)",
            "y",
        ),
        moses[3],
        "trimmed p",
    );
    near(
        value(&t, "Moses Test", 0, "Outliers Trimmed from each End", "y"),
        moses[4],
        "trimmed cases",
    );
    // Wald-Wolfowitz with ties: fewest and most possible runs
    let t = run("NPAR TESTS /W-W=y BY g(1,2).");
    for (row, w) in [
        ("Minimum Possible", [13.0, -0.8455008703, 0.1979694363]),
        ("Maximum Possible", [15.0, -0.08833591182, 0.4627633552]),
    ] {
        near(
            value(
                &t,
                "Wald-Wolfowitz Test",
                0,
                &format!("y / {row}"),
                "Number of Runs",
            ),
            w[0],
            row,
        );
        near(
            value(&t, "Wald-Wolfowitz Test", 0, &format!("y / {row}"), "Z"),
            w[1],
            row,
        );
        near(
            value(
                &t,
                "Wald-Wolfowitz Test",
                0,
                &format!("y / {row}"),
                "Exact Sig. (1-tailed)",
            ),
            w[2],
            row,
        );
    }
}
