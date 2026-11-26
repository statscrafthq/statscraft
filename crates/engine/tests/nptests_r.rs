//! NPTESTS, independent samples, against R on 63 invented readings in three groups of 20, 25 and
//! 18 with many ties (`y` is whole numbers, `y2` has one decimal): the scripts are in
//! lab/oracles/nptests-r. R has no function for most of these tests, so the expected numbers are
//! the formulas of the manual's Nonparametric Tests chapter written out in base R, with R's own
//! `kruskal.test`, `wilcox.test` and `chisq.test` as a second check where they apply (the
//! Kruskal-Wallis statistic and p, the Mann-Whitney p, the median test chi-square).
//!
//! One difference is on record: the manual lists steps for the fewest and the most runs of the
//! Wald-Wolfowitz test that, written out in R, disagree with an exhaustive enumeration of the
//! arrangements of the tied values in 17 of 301 random cases (in both directions). The engine
//! gives the true bounds, which the unit test of `run_bounds` checks against enumeration; the
//! steps as printed agree with them on `y` below and not on `y2`, so only `y` is compared here.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g y y2.
BEGIN DATA.
1 10 2.6
1 10 5.3
1 12 5.1
1 10 4.7
1 5 2.9
1 11 4.4
1 8 5.7
1 12 7.4
1 12 7.9
1 8 4.8
1 10 4.3
1 10 6.8
1 14 7.5
1 11 6.7
1 12 2.7
1 10 9.1
1 10 5.7
1 11 2.8
1 10 6.4
1 9 4.2
2 10 8.7
2 12 4.2
2 13 5.2
2 13 6.9
2 12 3.4
2 12 6.7
2 13 4.1
2 17 4.9
2 7 7.7
2 9 4.6
2 16 5.1
2 12 2.8
2 11 8.9
2 14 4.5
2 11 6.2
2 15 3.8
2 10 4.9
2 12 7.3
2 12 5.3
2 10 4.6
2 10 3.3
2 11 9
2 15 1.1
2 12 5.7
2 14 4.2
3 15 5.9
3 13 4.7
3 11 8.2
3 17 4.6
3 13 4.9
3 12 6.8
3 9 5
3 14 5.7
3 13 4.7
3 11 6.6
3 11 6.5
3 12 6.9
3 11 9.2
3 8 5.9
3 12 5.1
3 13 5.9
3 14 6.1
3 15 8.7
END DATA.
VARIABLE LEVEL y y2 (SCALE) /g (NOMINAL).";

fn run(syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}\n{syntax}"));
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

fn table<'t>(tables: &'t [Table], title: &str) -> Vec<&'t Table> {
    let found: Vec<&Table> = tables.iter().filter(|t| t.title == title).collect();
    assert!(!found.is_empty(), "no table {title}");
    found
}

/// The value in the row labelled `label`, column `column` (1 is the first field).
fn at(t: &Table, label: &str, column: usize) -> f64 {
    let row = t
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == label))
        .unwrap_or_else(|| panic!("no row {label} in {}", t.title));
    match &row[column] {
        Cell::Number(n) => *n,
        other => panic!("{label}: {other:?}"),
    }
}

fn number(t: &Table, row: usize, column: usize) -> f64 {
    match &t.rows[row][column] {
        Cell::Number(n) => *n,
        other => panic!("row {row} column {column}: {other:?}"),
    }
}

fn near(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance * (1.0 + expected.abs()),
        "{what}: {actual} is not {expected}"
    );
}

#[test]
fn k_sample_tests_with_ties() {
    let t = run("NPTESTS /INDEPENDENT TEST(y) GROUP(g) KRUSKAL_WALLIS JONCKHEERE_TERPSTRA MEDIAN.");
    let kw = table(&t, "Independent-Samples Kruskal-Wallis Test")[0];
    near(at(kw, "Test Statistic", 1), 11.8941029929, 1e-10, "KW H");
    near(at(kw, "Asymptotic Sig.", 1), 0.00261353518674, 1e-9, "KW p");
    let median = table(&t, "Independent-Samples Median Test")[0];
    assert_eq!(at(median, "Median", 1), 12.0);
    near(
        at(median, "Test Statistic", 1),
        9.77931818182,
        1e-10,
        "median chi-square",
    );
    near(
        at(median, "Asymptotic Sig.", 1),
        0.00752398703341,
        1e-9,
        "median p",
    );
    // the Jonckheere-Terpstra variance with its correction for ties
    let jt = table(&t, "Independent-Samples Jonckheere-Terpstra Test")[0];
    assert_eq!(at(jt, "Test Statistic", 1), 906.0);
    near(at(jt, "Standard Error", 1), 77.9940223157, 1e-10, "JT sd");
    near(
        at(jt, "Standardized Test Statistic", 1),
        3.21819535072,
        1e-10,
        "JT z",
    );
    near(
        at(jt, "Asymptotic Sig. (1-sided test)", 1),
        0.000644999633604,
        1e-9,
        "JT p1",
    );
    near(
        at(jt, "Asymptotic Sig. (2-sided test)", 1),
        0.00128999926721,
        1e-9,
        "JT p2",
    );
}

#[test]
fn pairwise_comparisons_with_ties() {
    let t = run("NPTESTS /INDEPENDENT TEST(y) GROUP(g) KRUSKAL_WALLIS JONCKHEERE_TERPSTRA MEDIAN.");
    let pairwise = table(&t, "Pairwise Comparisons of y by g");
    assert_eq!(pairwise.len(), 3);
    // Kruskal-Wallis (Dunn): the difference of mean ranks, the tie-corrected standard error,
    // the two-sided p and the Bonferroni adjustment
    let dunn = [
        (
            -15.315,
            5.43262991027,
            -2.81907662642,
            0.0048162019274,
            0.0144486057822,
        ),
        (
            -18.5416666667,
            5.88341405409,
            -3.15151483411,
            0.00162425911884,
            0.00487277735653,
        ),
        (
            -3.22666666667,
            5.59779302389,
            -0.576417644042,
            0.564332921254,
            1.0,
        ),
    ];
    // the median test of each pair on its own median, Yates' correction at more than 30 cases
    let median = [
        (5.31361607143, 0.021159393916, 0.0634781817481),
        (5.07405851229, 0.0242864578711, 0.0728593736133),
        (0.365712345679, 0.545350733958, 1.0),
    ];
    // Jonckheere-Terpstra: Mann-Whitney U of each pair, one-sided
    let jt = [
        (372.0, 42.9763853409, 2.83876829176, 0.00226440147851),
        (285.5, 33.7986254739, 3.12142871258, 0.000899879081136),
        (248.5, 40.1435340031, 0.585399382082, 0.279139602079),
    ];
    for (r, (stat, se, z, p, adjusted)) in dunn.iter().enumerate() {
        let tab = pairwise[0];
        near(number(tab, r, 1), *stat, 1e-9, "Dunn difference");
        near(number(tab, r, 2), *se, 1e-9, "Dunn se");
        near(number(tab, r, 3), *z, 1e-9, "Dunn z");
        near(number(tab, r, 4), *p, 1e-8, "Dunn p");
        near(number(tab, r, 5), *adjusted, 1e-8, "Dunn adjusted p");
    }
    for (r, (stat, p, adjusted)) in median.iter().enumerate() {
        let tab = pairwise[1];
        near(number(tab, r, 1), *stat, 1e-9, "median chi-square");
        near(number(tab, r, 4), *p, 1e-8, "median p");
        near(number(tab, r, 5), *adjusted, 1e-8, "median adjusted p");
    }
    for (r, (u, se, z, p)) in jt.iter().enumerate() {
        let tab = pairwise[2];
        near(number(tab, r, 1), *u, 1e-9, "JT U");
        near(number(tab, r, 2), *se, 1e-9, "JT se");
        near(number(tab, r, 3), *z, 1e-9, "JT z");
        near(number(tab, r, 4), *p, 1e-8, "JT p");
        near(number(tab, r, 5), (3.0 * p).min(1.0), 1e-8, "JT adjusted p");
    }
}

#[test]
fn stepwise_subsets_with_ties() {
    // the mean ranks are 20.625, 35.94 and 39.17; the first sample stands apart, and the
    // Kruskal-Wallis test of the other two (kruskal.test on them) is 0.342692436542, p 0.558279204158
    let t = run("NPTESTS /INDEPENDENT TEST(y) GROUP(g) KRUSKAL_WALLIS(COMPARE=STEPWISE).");
    let subsets = table(&t, "Homogeneous Subsets of y by g")[0];
    assert_eq!(subsets.columns, ["Sample", "Subset 1", "Subset 2"]);
    near(number(subsets, 0, 1), 20.625, 1e-12, "mean rank 1");
    assert_eq!(subsets.rows[0][2], Cell::Empty);
    near(number(subsets, 1, 2), 35.94, 1e-12, "mean rank 2");
    near(number(subsets, 2, 2), 39.1666666667, 1e-10, "mean rank 3");
    near(
        number(subsets, 3, 2),
        0.342692436542,
        1e-10,
        "subset statistic",
    );
    near(number(subsets, 4, 2), 0.558279204158, 1e-9, "subset p");
}

#[test]
fn two_independent_samples_with_ties() {
    let t = run("SELECT IF g < 3.
         NPTESTS /INDEPENDENT TEST(y y2) GROUP(g) MANN_WHITNEY WALD_WOLFOWITZ KOLMOGOROV_SMIRNOV HODGES_LEHMAN MOSES /CRITERIA CILEVEL=95.");
    // Mann-Whitney: 20 times 25 pairs is above 400, so the normal approximation with the tie
    // correction; the p is also wilcox.test with exact = FALSE and no continuity correction
    let mw = table(&t, "Independent-Samples Mann-Whitney U Test")[0];
    for (column, u, se, z, p) in [
        (1, 372.0, 42.9763853409, 2.83876829176, 0.00452880295701),
        (2, 243.0, 43.7595588403, -0.159965049592, 0.872908606014),
    ] {
        assert_eq!(at(mw, "Mann-Whitney U", column), u);
        near(at(mw, "Standard Error", column), se, 1e-10, "MW se");
        near(
            at(mw, "Standardized Test Statistic", column),
            z,
            1e-10,
            "MW z",
        );
        near(
            at(mw, "Asymptotic Sig. (2-sided test)", column),
            p,
            1e-9,
            "MW p",
        );
    }
    // Kolmogorov-Smirnov: the largest difference, Smirnov's Z and its series p (R's exact
    // ks.test gives 0.0201 and 0.9810, which is a different p)
    let ks = table(&t, "Independent-Samples Kolmogorov-Smirnov Test")[0];
    for (column, positive, negative, z, p) in [
        (1, 0.0, -0.39, 1.3, 0.0680922218448),
        (2, 0.1, -0.12, 0.4, 0.997192326777),
    ] {
        near(
            at(ks, "Most Extreme Differences: Positive", column),
            positive,
            1e-12,
            "KS +",
        );
        near(
            at(ks, "Most Extreme Differences: Negative", column),
            negative,
            1e-12,
            "KS -",
        );
        near(at(ks, "Test Statistic", column), z, 1e-12, "KS Z");
        // the manual's constants are rounded to seven digits
        near(
            at(ks, "Asymptotic Sig. (2-sided test)", column),
            p,
            2e-7,
            "KS p",
        );
    }
    // Wald-Wolfowitz on y: 10 to 30 runs, z with the continuity correction below 50 cases
    let ww = table(&t, "Independent-Samples Wald-Wolfowitz Runs Test")[0];
    assert_eq!(at(ww, "Fewest Possible Runs", 1), 10.0);
    assert_eq!(at(ww, "Most Possible Runs", 1), 30.0);
    near(
        at(ww, "Standardized Test Statistic, Fewest Runs", 1),
        -3.88597626322,
        1e-10,
        "WW z min",
    );
    near(
        at(ww, "Standardized Test Statistic, Most Runs", 1),
        2.22298205451,
        1e-10,
        "WW z max",
    );
    near(
        at(ww, "Asymptotic Sig. (1-sided test), Fewest Runs", 1),
        5.0959732816e-05,
        1e-9,
        "WW p min",
    );
    near(
        at(ww, "Asymptotic Sig. (1-sided test), Most Runs", 1),
        0.986891494964,
        1e-9,
        "WW p max",
    );
    // Moses: the span of the first group's ranks and the exact probability, with 1 record
    // trimmed from each end (5 percent of 20); a trimmed span of 27.5 rounds to 28
    let moses = table(&t, "Independent-Samples Moses Test of Extreme Reaction")[0];
    for (column, span, p, trimmed, trimmed_p) in [
        (1, 40.0, 0.152202684818, 28.0, 0.0024108055421),
        (2, 44.0, 0.808080808081, 39.0, 0.628881422982),
    ] {
        assert_eq!(at(moses, "Observed Control Group Span", column), span);
        near(
            at(moses, "Exact Sig. (1-sided test), Observed", column),
            p,
            1e-9,
            "Moses p",
        );
        assert_eq!(at(moses, "Trimmed Control Group Span", column), trimmed);
        near(
            at(moses, "Exact Sig. (1-sided test), Trimmed", column),
            trimmed_p,
            1e-9,
            "Moses trimmed p",
        );
    }
    // Hodges-Lehmann: the median of the pairwise differences, limits at ranks 166 and 335 of the
    // 500 ordered differences for y and 165 and 336 for y2
    let intervals = table(&t, "Confidence Interval Summary")[0];
    for (row, estimate, lower, upper) in [(0, -2.0, -3.0, -1.0), (1, 0.1, -1.2, 1.3)] {
        near(number(intervals, row, 2), estimate, 1e-9, "HL estimate");
        near(number(intervals, row, 3), lower, 1e-9, "HL lower");
        near(number(intervals, row, 4), upper, 1e-9, "HL upper");
    }
}
