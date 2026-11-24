//! NPAR TESTS against R on 24 invented cases (two groups, three groups with ties, paired
//! values with three zero differences, three related measures, three related yes/no columns):
//! Mann-Whitney (U, W, tie-corrected Z, `wilcox.test`), Wilcoxon signed ranks, the exact sign,
//! McNemar and binomial tests (`binom.test`), Kruskal-Wallis (`kruskal.test`), Friedman and
//! Kendall's W (`friedman.test`), Cochran's Q, the chi-square test, the median test
//! (`chisq.test` on the two counts), Jonckheere-Terpstra, the runs test and both
//! Kolmogorov-Smirnov tests. The last three are written out in R from the manual's formulas.
//! The Kolmogorov-Smirnov significance uses the manual's constants rounded to seven digits
//! (1.233701 and 2.506628), so it agrees with the exact series only to 1e-5.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g x k y pre post b1 b2 f1 f2 f3 c1 c2 c3.
BEGIN DATA
1 59 1 25 34 34 0 0 9 12 13 1 0 0
1 54 1 25 42 44 0 1 9 11 15 0 0 1
1 60 1 22 27 30 0 0 7 11 8 1 1 1
1 60 1 26 25 26 0 0 10 11 14 1 1 0
1 60 1 13 26 34 0 0 9 12 10 1 1 1
1 62 1 22 24 29 0 0 10 13 12 0 1 0
1 55 1 16 26 26 1 0 8 7 11 1 1 0
1 49 2 24 21 23 1 1 8 9 12 0 0 1
1 39 2 25 35 32 1 1 9 15 13 1 0 1
1 40 2 32 29 35 0 1 11 14 13 0 1 1
2 44 2 24 36 41 1 1 8 7 11 1 0 0
2 63 2 28 30 25 0 1 10 5 11 1 1 1
2 66 2 28 40 43 1 1 9 11 12 1 1 1
2 62 2 24 35 41 0 0 10 10 13 1 0 0
2 30 2 25 33 34 1 1 13 13 16 0 0 1
2 58 3 31 31 32 0 0 8 10 9 1 0 1
2 51 3 25 40 41 0 1 8 12 9 1 1 1
2 63 3 29 29 34 1 1 11 12 15 1 0 1
2 45 3 31 18 15 1 0 11 12 15 1 1 1
2 41 3 39 34 34 1 0 9 13 10 1 1 0
2 69 3 23 33 32 0 0 11 11 12 0 0 1
2 62 3 30 17 22 0 0 8 8 11 0 0 1
2 68 3 31 37 41 1 0 8 9 9 1 0 1
2 57 3 32 23 17 0 1 13 11 13 1 1 1
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables.extend(engine.execute(&command.command).expect("runs"));
    }
    tables
}

fn near(got: f64, want: f64, tolerance: f64, what: &str) {
    assert!(
        (got - want).abs() <= tolerance * want.abs().max(1e-3),
        "{what}: {got} vs {want}"
    );
}

/// The last number in the row of the `nth` table called `title` that has `label` in a cell.
fn stat(tables: &[Table], title: &str, nth: usize, label: &str) -> f64 {
    let table = tables
        .iter()
        .filter(|t| t.title == title)
        .nth(nth)
        .unwrap_or_else(|| panic!("no table {title}"));
    let row = table
        .rows
        .iter()
        .find(|r| r.iter().any(|c| matches!(c, Cell::Text(s) if s == label)))
        .unwrap_or_else(|| panic!("no row {label} in {title}"));
    row.iter()
        .rev()
        .find_map(|c| match c {
            Cell::Number(n) => Some(*n),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no number in row {label}"))
}

#[test]
fn two_independent_samples_agree_with_r() {
    let t = run(
        "NPAR TESTS /M-W=x BY g(1,2) /K-S=x BY g(1,2) /MEDIAN=x BY g(1,2) \
                 /MEDIAN(55)=x BY g(1,2).",
    );
    let mw = "Mann-Whitney Test";
    near(stat(&t, mw, 0, "Mann-Whitney U"), 52.0, 1e-12, "U");
    near(stat(&t, mw, 0, "Wilcoxon W"), 107.0, 1e-12, "W");
    near(stat(&t, mw, 0, "Z"), -1.05604026974, 1e-9, "Z");
    near(
        stat(&t, mw, 0, "Asymp. Sig. (2-tailed)"),
        0.290949817271,
        1e-9,
        "p",
    );
    // D = 0.4 and Z = sqrt(n1 n2 / (n1 + n2)) D; the p of the exact series is 0.308132967774
    let ks = "Two-Sample Kolmogorov-Smirnov Test";
    near(stat(&t, ks, 0, "Absolute"), 0.4, 1e-12, "D");
    near(
        stat(&t, ks, 0, "Kolmogorov-Smirnov Z"),
        0.966091783079,
        1e-9,
        "Z",
    );
    near(
        stat(&t, ks, 0, "Asymp. Sig. (2-tailed)"),
        0.308132967774,
        1e-5,
        "p",
    );
    // the median 58.5 splits the groups 5 and 5 above, 7 and 7 at or below: chi-square 0
    let median = "Median Test";
    near(stat(&t, median, 0, "Median"), 58.5, 1e-12, "median");
    near(stat(&t, median, 0, "Chi-Square"), 0.0, 1e-9, "chi-square 0");
    near(stat(&t, median, 0, "Asymp. Sig."), 1.0, 1e-9, "p 1");
    // cut at 55: 5 and 9 above, 5 and 5 below
    near(
        stat(&t, median, 1, "Chi-Square"),
        0.489795918367,
        1e-9,
        "chi-square",
    );
    near(stat(&t, median, 1, "Asymp. Sig."), 0.484018354, 1e-6, "p");
}

#[test]
fn two_related_samples_agree_with_r() {
    let t = run(
        "NPAR TESTS /WILCOXON=pre WITH post (PAIRED) /SIGN=pre WITH post (PAIRED) \
                 /MCNEMAR=b1 WITH b2 (PAIRED) /BINOMIAL(0.5)=b1(0,1).",
    );
    // 21 differences left after three zeros: rank sums 175 and 56
    near(
        stat(&t, "Ranks", 0, "Negative Ranks"),
        175.0,
        1e-12,
        "negative sum",
    );
    near(
        stat(&t, "Ranks", 0, "Positive Ranks"),
        56.0,
        1e-12,
        "positive sum",
    );
    let wilcoxon = "Wilcoxon Signed Ranks Test";
    near(stat(&t, wilcoxon, 0, "Z"), -2.07672241795, 1e-9, "Z");
    near(
        stat(&t, wilcoxon, 0, "Asymp. Sig. (2-tailed)"),
        0.0378271862847,
        1e-9,
        "p",
    );
    near(
        stat(&t, "Sign Test", 0, "Exact Sig. (2-tailed)"),
        0.0266036987305,
        1e-9,
        "sign",
    );
    // four and five changes: the exact p is 1
    near(
        stat(&t, "McNemar Test", 0, "Exact Sig. (2-tailed)"),
        1.0,
        1e-12,
        "McNemar",
    );
    near(
        stat(&t, "Binomial Test", 0, "Group 1"),
        0.541256189346,
        1e-9,
        "binomial",
    );
}

#[test]
fn several_samples_agree_with_r() {
    let t = run(
        "NPAR TESTS /K-W=y BY k(1,3) /J-T=y BY k(1,3) /FRIEDMAN=f1 f2 f3 \
                 /KENDALL=f1 f2 f3 /COCHRAN=c1 c2 c3 /CHISQUARE=k.",
    );
    let kw = "Kruskal-Wallis Test";
    near(stat(&t, kw, 0, "Chi-Square"), 9.58046732355, 1e-9, "H");
    near(
        stat(&t, kw, 0, "Asymp. Sig."),
        0.00831051530527,
        1e-9,
        "H p",
    );
    // the observed J-T statistic, its mean and standard deviation with the tie terms
    let jt = "Jonckheere-Terpstra Test";
    let table = t.iter().find(|t| t.title == jt).expect("the J-T table");
    let cell = |column: &str| {
        let at = table
            .column_titles()
            .iter()
            .position(|c| c == column)
            .unwrap_or_else(|| panic!("no column {column}"));
        match table.rows[0][at] {
            Cell::Number(n) => n,
            ref other => panic!("{column}: {other:?}"),
        }
    };
    near(cell("Observed J-T Statistic"), 155.5, 1e-12, "J-T");
    near(cell("Mean J-T Statistic"), 95.5, 1e-12, "mean");
    near(
        cell("Std. Deviation of J-T Statistic"),
        18.7297211375,
        1e-9,
        "sd",
    );
    near(cell("Std. J-T Statistic"), 3.20346467306, 1e-9, "z");
    near(cell("Asymp. Sig. (2-tailed)"), 0.00135784697799, 1e-8, "p");
    let friedman = "Friedman Test";
    near(
        stat(&t, friedman, 0, "Chi-Square"),
        23.8888888889,
        1e-9,
        "Friedman",
    );
    near(
        stat(&t, friedman, 0, "Asymp. Sig."),
        6.4952173484e-06,
        1e-8,
        "Friedman p",
    );
    let kendall = "Kendall's W Test";
    near(
        stat(&t, kendall, 0, "Kendall's W"),
        0.497685185185,
        1e-9,
        "W",
    );
    near(
        stat(&t, kendall, 0, "Chi-Square"),
        23.8888888889,
        1e-9,
        "W chi-square",
    );
    let cochran = "Cochran Test";
    near(
        stat(&t, cochran, 0, "Cochran's Q"),
        2.94117647059,
        1e-9,
        "Q",
    );
    near(
        stat(&t, cochran, 0, "Asymp. Sig."),
        0.229790274673,
        1e-9,
        "Q p",
    );
    let chi = "Chi-Square Test Statistics";
    near(stat(&t, chi, 0, "Chi-Square"), 0.25, 1e-12, "chi-square");
    near(
        stat(&t, chi, 0, "Asymp. Sig."),
        0.882496902585,
        1e-9,
        "chi-square p",
    );
}

#[test]
fn the_runs_and_one_sample_kolmogorov_smirnov_tests_agree_with_r() {
    let t = run("NPAR TESTS /RUNS(MEDIAN)=x /K-S(NORMAL,50,10)=x.");
    // 10 runs, mean 13, 12 cases each side, the 0.5 correction toward the mean
    let runs = "Runs Test";
    near(stat(&t, runs, 0, "Number of Runs"), 10.0, 1e-12, "runs");
    near(stat(&t, runs, 0, "Z"), -1.04355888742, 1e-9, "runs Z");
    near(
        stat(&t, runs, 0, "Asymp. Sig. (2-tailed)"),
        0.296689522482,
        1e-9,
        "runs p",
    );
    let ks = "One-Sample Kolmogorov-Smirnov Test";
    near(stat(&t, ks, 0, "Absolute"), 0.34136968111, 1e-9, "D");
    near(
        stat(&t, ks, 0, "Kolmogorov-Smirnov Z"),
        1.67236306475,
        1e-9,
        "Z",
    );
    near(
        stat(&t, ks, 0, "Asymp. Sig. (2-tailed)"),
        0.00744323798565,
        1e-5,
        "p",
    );
}
