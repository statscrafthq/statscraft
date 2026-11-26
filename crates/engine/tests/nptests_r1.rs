//! NPTESTS, one sample and related samples, against R on 36 invented cases with ties and zero
//! differences (`gen.R` in lab/oracles/nptests-r1 makes the data, the other scripts there compute
//! the expected numbers). R's own `wilcox.test`, `binom.test`, `friedman.test`, `mcnemar.test`,
//! `chisq.test` and `ks.test` are the first check where they apply; the tests R lacks (Cochran's Q,
//! Kendall's W, marginal homogeneity, the runs test, the Lilliefors and Poisson K-S, the
//! pairwise comparisons) are the formulas of the manual's Nonparametric Tests chapter written out in
//! base R.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /a b c3 c4 x cnt ex un cat3 y1 y2 y3 y4 o1 o2 runv rv.
BEGIN DATA.
9 10 11 12 19.8 1 9.8 5.3 2 0 0 0 0 4 4 1 41
5 7 7 9 23.8 5 7.34 10.6 1 0 1 1 1 3 4 1 39
6 6 6 5 16.6 4 9.5 4.8 2 0 0 1 1 2 4 0 50
13 13 13 13 17.6 0 1.83 7.9 1 1 1 0 1 3 4 0 38
5 5 6 6 20 6 0.6 7 1 0 0 1 1 2 4 1 55
9 10 10 10 17.6 4 1.46 4.9 1 0 0 1 0 3 3 0 38
9 11 13 14 13.7 5 0.9 8.5 3 1 1 1 1 4 4 1 52
6 5 4 6 17.7 5 5.1 7.8 1 0 0 1 0 3 4 1 71
12 14 15 14 20 2 0.39 4 3 0 0 1 1 3 4 1 36
4 5 7 10 21.2 2 4.2 4.8 1 0 0 0 1 4 4 0 47
7 5 7 8 20.8 4 5.62 4 1 1 0 0 0 4 4 0 60
8 8 8 7 24.9 5 5.31 6.7 1 1 1 1 0 3 3 0 56
5 5 7 7 18 0 10.68 5.9 1 1 0 0 1 3 4 1 61
5 6 5 7 13.7 3 4.02 10.1 3 0 0 1 1 4 4 0 44
3 2 2 2 23.2 3 0.72 5.3 1 0 1 0 0 2 2 1 67
7 7 7 11 16.9 4 0.58 8.3 1 1 1 0 0 1 3 0 60
4 7 7 11 22.5 4 7.24 7.8 2 0 1 1 1 3 4 0 50
5 8 9 8 16.1 3 5.28 10.5 1 0 1 0 1 4 3 1 43
8 11 10 12 25.9 6 1.72 7.4 2 0 0 0 1 4 4 1 44
4 2 2 4 20 3 2.41 6.2 1 1 1 1 0 4 4 0 28
5 5 4 4 19.7 3 1.65 8.1 3 1 1 1 1 3 3 1 41
6 7 7 7 18.9 1 0.15 9.5 1 0 0 1 1 2 3 1 40
12 12 13 16 23.9 4 1.14 4.5 2 0 0 1 0 3 4 0 53
3 5 5 6 16.4 4 5.16 4.1 2 1 1 1 1 2 3 0 47
12 13 15 14 22.6 2 0.75 10.9 1 1 1 0 0 3 4 0 38
7 7 7 9 19 5 2.11 9.4 1 0 1 1 1 3 2 1 54
4 7 7 7 19.1 3 4.78 5.8 2 1 1 0 0 2 1 0 39
8 8 7 7 16.6 2 3.02 8.2 2 1 1 0 1 3 4 1 44
4 2 2 2 26.4 6 2.04 2.7 2 0 0 1 0 1 2 1 56
7 10 12 15 23.4 2 6.65 10.5 2 1 1 0 0 3 4 1 48
8 11 11 10 20 1 9.52 8.8 1 0 0 0 1 3 4 0 42
9 12 14 14 16.2 2 0.14 4.7 2 1 1 1 0 4 4 1 50
10 10 10 9 27.4 0 2.99 5.2 3 0 0 1 1 3 3 0 37
5 5 5 4 30.4 3 8.27 6.4 1 0 0 0 1 3 4 0 52
6 9 11 11 25.7 2 0.53 9.8 1 1 1 0 1 4 4 1 71
5 3 3 0 24.6 4 1.72 5.1 1 1 1 0 0 3 4 1 51
END DATA.
VARIABLE LEVEL a b c3 c4 x cnt ex un rv (SCALE) /cat3 y1 y2 y3 y4 runv (NOMINAL) /o1 o2 (ORDINAL).";

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

fn table<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn cell<'t>(t: &'t Table, label: &str) -> &'t [Cell] {
    t.rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == label))
        .unwrap_or_else(|| panic!("no row {label} in {}", t.title))
}

fn at(t: &Table, label: &str, column: usize) -> f64 {
    match &cell(t, label)[column] {
        Cell::Number(n) => *n,
        other => panic!("{label}: {other:?}"),
    }
}

fn near(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-8 + 1e-7 * expected.abs(),
        "{what}: {actual} is not {expected}"
    );
}

/// One row of a detail table against the numbers of the R script.
fn check(t: &Table, rows: &[(&str, f64)]) {
    for (label, want) in rows {
        near(at(t, label, 1), *want, &format!("{} {label}", t.title));
    }
}

#[test]
fn wilcoxon_sign_and_hodges_lehmann_with_zero_differences() {
    // a and b differ by zero in 12 of 36 cases and the absolute differences tie heavily
    let t = run("NPTESTS /RELATED TEST(a b) WILCOXON SIGN HODGES_LEHMAN.");
    let w = table(&t, "Related-Samples Wilcoxon Signed Rank Test");
    check(
        w,
        &[
            ("Total N", 36.0),
            ("Test Statistic", 59.0),
            ("Standard Error", 34.5470693402),
            ("Standardized Test Statistic", -2.63408739838),
            ("Asymptotic Sig. (2-sided test)", 0.00843637691561),
        ],
    );
    let sign = table(&t, "Related-Samples Sign Test");
    check(
        sign,
        &[
            ("Positive Differences", 6.0),
            ("Negative Differences", 18.0),
            ("Ties", 12.0),
            ("Exact Sig. (2-sided test)", 0.0226558446884),
        ],
    );
    // the manual drops the zero differences from the ranks only: its Hodges-Lehmann estimate
    // takes the Walsh averages of all 36 differences, while R's `wilcox.test(conf.int = TRUE)`
    // drops the zeros first (-1.183, limits -2.000 and -0.500), so the numbers are the
    // formulas written out in R
    let ci = table(&t, "Confidence Interval Summary");
    near(number(ci, 0, 2), -0.5, "HL estimate");
    near(number(ci, 0, 3), -1.5, "HL lower");
    near(number(ci, 0, 4), 0.0, "HL upper");
    // more than 25 untied pairs: the normal approximation with the continuity correction
    let t = run("NPTESTS /RELATED TEST(a c4) SIGN.");
    let sign = table(&t, "Related-Samples Sign Test");
    check(
        sign,
        &[
            ("Positive Differences", 9.0),
            ("Negative Differences", 24.0),
            ("Ties", 3.0),
            ("Standardized Test Statistic", 2.43708718338),
            ("Asymptotic Sig. (2-sided test)", 0.0148061091273),
        ],
    );
}

fn number(t: &Table, row: usize, column: usize) -> f64 {
    match &t.rows[row][column] {
        Cell::Number(n) => *n,
        other => panic!("row {row} column {column}: {other:?}"),
    }
}

/// A row of a pairwise table: statistic, standard error, standardized, sig., adjusted sig.
fn pair(t: &Table, label: &str, want: [f64; 5]) {
    let row = cell(t, label);
    for (i, w) in want.iter().enumerate() {
        match &row[i + 1] {
            Cell::Number(n) => near(*n, *w, &format!("{label} column {}", i + 1)),
            other => panic!("{label}: {other:?}"),
        }
    }
}

#[test]
fn friedman_kendall_and_the_comparisons_with_ties() {
    let t = run("NPTESTS /RELATED TEST(a b c3 c4) FRIEDMAN KENDALL.\n\
         NPTESTS /RELATED TEST(a b c3 c4) FRIEDMAN(COMPARE=STEPWISE) /CRITERIA ALPHA=0.2.");
    let f = table(
        &t,
        "Related-Samples Friedman's Two-Way Analysis of Variance by Ranks",
    );
    check(
        f,
        &[
            ("Test Statistic", 15.7844522968),
            ("Degrees of Freedom", 3.0),
            ("Asymptotic Sig.", 0.00125540021596),
        ],
    );
    let k = table(&t, "Related-Samples Kendall's Coefficient of Concordance");
    check(
        k,
        &[
            ("Kendall's W", 0.146152336082),
            ("Test Statistic", 15.7844522968),
        ],
    );
    let p = table(&t, "Pairwise Comparisons of a, b, c3, c4");
    pair(
        p,
        "a-b",
        [
            -0.444444444444,
            0.304290309725,
            -1.46059348668,
            0.144127034816,
            0.864762208896,
        ],
    );
    pair(
        p,
        "a-c3",
        [
            -0.763888888889,
            0.304290309725,
            -2.51039505523,
            0.0120596161775,
            0.0723576970649,
        ],
    );
    pair(
        p,
        "a-c4",
        [
            -1.01388888889,
            0.304290309725,
            -3.33197889149,
            0.000862307964837,
            0.00517384778902,
        ],
    );
    pair(
        p,
        "b-c3",
        [
            -0.319444444444,
            0.304290309725,
            -1.04980156855,
            0.293809353948,
            1.0,
        ],
    );
    pair(
        p,
        "b-c4",
        [
            -0.569444444444,
            0.304290309725,
            -1.87138540481,
            0.0612916804263,
            0.367750082558,
        ],
    );
    pair(
        p,
        "c3-c4",
        [-0.25, 0.304290309725, -0.821583836258, 0.411313791776, 1.0],
    );
    // average ranks 1.944, 2.389, 2.708, 2.958; at 0.05 the subsets are a | b c3 | c4 (the test
    // of b and c3 has 3.556, p 0.0593), at 0.2 they are a | b | c3 c4 (1.5, p 0.2207)
    let steps = t
        .iter()
        .filter(|t| t.title == "Homogeneous Subsets of a, b, c3, c4")
        .collect::<Vec<_>>();
    assert_eq!(steps.len(), 1);
    let s = steps[0];
    assert_eq!(s.columns, ["Sample", "Subset 1", "Subset 2", "Subset 3"]);
    near(at(s, "a", 1), 1.94444444444, "a");
    near(at(s, "c3", 3), 2.70833333333, "c3");
    near(at(s, "c4", 3), 2.95833333333, "c4");
    near(at(s, "Test Statistic", 3), 1.5, "stepwise statistic");
    near(at(s, "Sig.", 3), 0.22067136192, "stepwise sig");
    let t = run("NPTESTS /RELATED TEST(a b c3 c4) FRIEDMAN(COMPARE=STEPWISE).");
    let s = table(&t, "Homogeneous Subsets of a, b, c3, c4");
    assert_eq!(s.columns, ["Sample", "Subset 1", "Subset 2", "Subset 3"]);
    near(at(s, "b", 2), 2.38888888889, "b");
    near(at(s, "c3", 2), 2.70833333333, "c3");
    near(at(s, "Test Statistic", 2), 3.55555555556, "statistic");
    near(at(s, "Sig.", 2), 0.0593464387919, "sig");
}

#[test]
fn cochran_mcnemar_and_marginal_homogeneity() {
    let t = run(
        "NPTESTS /RELATED TEST(y1 y2 y3 y4) COCHRAN(SUCCESS=LIST(1)) /CRITERIA ALPHA=0.8.\n\
         NPTESTS /RELATED TEST(y1 y2 y3 y4) COCHRAN(SUCCESS=LIST(1) COMPARE=STEPWISE) /CRITERIA ALPHA=0.8.\n\
         NPTESTS /RELATED TEST(y1 y2) MCNEMAR(SUCCESS=LIST(1)).\n\
         NPTESTS /RELATED TEST(o1 o2) MARGINAL_HOMOGENEITY.",
    );
    let q = table(&t, "Related-Samples Cochran's Q Test");
    check(
        q,
        &[
            ("Test Statistic", 1.40366972477),
            ("Degrees of Freedom", 3.0),
            ("Asymptotic Sig.", 0.704674753171),
        ],
    );
    let p = table(&t, "Pairwise Comparisons of y1, y2, y3, y4");
    let se = 0.118395480116;
    pair(
        p,
        "y1-y2",
        [-0.0833333333333, se, -0.703855698305, 0.481522643022, 1.0],
    );
    pair(
        p,
        "y1-y4",
        [-0.138888888889, se, -1.17309283051, 0.240758581272, 1.0],
    );
    pair(p, "y2-y3", [0.0, se, 0.0, 1.0, 1.0]);
    pair(
        p,
        "y3-y4",
        [-0.0555555555556, se, -0.469237132203, 0.638900146627, 1.0],
    );
    // column sums 16, 19, 19, 21: y1 stands alone (0.2568 < 0.8), then y2 y3 y4 hold
    let s = t
        .iter()
        .find(|t| t.title == "Homogeneous Subsets of y1, y2, y3, y4")
        .expect("subsets");
    near(at(s, "y1", 1), 16.0 / 36.0, "y1");
    near(at(s, "y4", 2), 21.0 / 36.0, "y4");
    near(at(s, "Test Statistic", 2), 0.285714285714, "Q of y2 y3 y4");
    near(at(s, "Sig.", 2), 0.86687789975, "sig");
    let m = table(&t, "Related-Samples McNemar Change Test");
    check(
        m,
        &[
            ("Success Then Failure", 2.0),
            ("Failure Then Success", 5.0),
            ("Exact Sig. (2-sided test)", 0.453125),
        ],
    );
    let h = table(&t, "Related-Samples Marginal Homogeneity Test");
    check(
        h,
        &[
            ("Off-Diagonal Cases", 22.0),
            ("Test Statistic", 58.0),
            ("Mean of the Statistic", 67.5),
            ("Standard Error", 2.78388218142),
            ("Standardized Test Statistic", -3.41250073851),
            ("Asymptotic Sig. (2-sided test)", 0.00064369752547),
        ],
    );
    // a and e = 12 - c4 as yes/no: 19 and 8 discordant pairs, past the 25 of the exact test;
    // with equal discordant counts (a and b at 5) the exact p is capped at 1
    let t = run(
        "COMPUTE e = 12 - c4.\nEXECUTE.\nVARIABLE LEVEL a b e (NOMINAL).\n\
         NPTESTS /RELATED TEST(a e) MCNEMAR(SUCCESS=LIST(6 7 8 9 10 11 12 13)).\n\
         NPTESTS /RELATED TEST(a b) MCNEMAR(SUCCESS=LIST(5)).",
    );
    let both: Vec<&Table> = t
        .iter()
        .filter(|t| t.title == "Related-Samples McNemar Change Test")
        .collect();
    check(
        both[0],
        &[
            ("Success Then Failure", 19.0),
            ("Failure Then Success", 8.0),
            ("Test Statistic", 3.7037037037),
            ("Asymptotic Sig.", 0.0542918283669),
        ],
    );
    check(
        both[1],
        &[
            ("Success Then Failure", 4.0),
            ("Failure Then Success", 4.0),
            ("Exact Sig. (2-sided test)", 1.0),
        ],
    );
}

#[test]
fn one_sample_wilcoxon_chi_square_binomial_and_runs() {
    let t = run("NPTESTS /ONESAMPLE TEST(x) WILCOXON(TESTVALUE=20).\n\
         NPTESTS /ONESAMPLE TEST(cat3) CHISQUARE.\n\
         NPTESTS /ONESAMPLE TEST(cat3) CHISQUARE(EXPECTED=CUSTOM(FREQUENCIES=5 3 2 CATEGORIES=1 2 3)).\n\
         NPTESTS /ONESAMPLE TEST(cnt) BINOMIAL(TESTVALUE=0.6 SUCCESSCONTINUOUS=CUTPOINT(4) CLOPPERPEARSON JEFFREYS LIKELIHOOD).\n\
         NPTESTS /ONESAMPLE TEST(y1) BINOMIAL(TESTVALUE=0.4 SUCCESSCATEGORICAL=LIST(1)).\n\
         NPTESTS /ONESAMPLE TEST(runv) RUNS.\n\
         NPTESTS /ONESAMPLE TEST(rv) RUNS(GROUPCONTINUOUS=CUTPOINT(SAMPLEMEDIAN)).\n\
         NPTESTS /ONESAMPLE TEST(rv) RUNS(GROUPCONTINUOUS=CUTPOINT(SAMPLEMEAN)).\n\
         NPTESTS /ONESAMPLE TEST(rv) RUNS(GROUPCONTINUOUS=CUTPOINT(52)).");
    // three of the 36 values equal the test value and drop out of the ranks
    let w = table(&t, "One-Sample Wilcoxon Signed Rank Test");
    check(
        w,
        &[
            ("Test Statistic", 305.0),
            ("Standard Error", 53.4696175412),
            ("Standardized Test Statistic", 0.766790597827),
            ("Asymptotic Sig. (2-sided test)", 0.443206027405),
        ],
    );
    let chi: Vec<&Table> = t
        .iter()
        .filter(|t| t.title == "One-Sample Chi-Square Test")
        .collect();
    check(
        chi[0],
        &[
            ("Test Statistic", 9.5),
            ("Asymptotic Sig.", 0.00865169520312),
        ],
    );
    check(
        chi[1],
        &[
            ("Test Statistic", 0.898148148148),
            ("Asymptotic Sig.", 0.638218821475),
        ],
    );
    // 36 cases: the normal approximation with the continuity correction, one- and two-sided
    let bin: Vec<&Table> = t
        .iter()
        .filter(|t| t.title == "One-Sample Binomial Test")
        .collect();
    check(
        bin[0],
        &[
            ("Successes", 28.0),
            ("Asymptotic Sig. (1-sided test)", 0.0223630809639),
            ("Asymptotic Sig. (2-sided test)", 0.0447261619278),
        ],
    );
    check(
        bin[1],
        &[
            ("Successes", 16.0),
            ("Asymptotic Sig. (1-sided test)", 0.354117495926),
            ("Asymptotic Sig. (2-sided test)", 0.708234991852),
        ],
    );
    let ci = table(&t, "Confidence Interval Summary");
    near(number(ci, 0, 3), 0.608482350843, "CP lower");
    near(number(ci, 0, 4), 0.898849469655, "CP upper");
    near(number(ci, 1, 3), 0.624016680123, "Jeffreys lower");
    near(number(ci, 1, 4), 0.888921891841, "Jeffreys upper");
    near(number(ci, 2, 3), 0.625708038264, "LR lower");
    near(number(ci, 2, 4), 0.891649042376, "LR upper");
    // 36 cases are under 50, so the runs statistic has the continuity correction of 0.5
    let runs: Vec<&Table> = t
        .iter()
        .filter(|t| t.title == "One-Sample Runs Test")
        .collect();
    check(
        runs[0],
        &[
            ("Number of Runs", 21.0),
            ("Standard Error", 2.94743783153),
            ("Standardized Test Statistic", 0.527765348913),
            ("Asymptotic Sig. (2-sided test)", 0.597662210168),
        ],
    );
    check(
        runs[1],
        &[
            ("Test Value", 47.5),
            ("Number of Runs", 20.0),
            ("Standard Error", 2.95683228183),
            ("Standardized Test Statistic", 0.169099885399),
            ("Asymptotic Sig. (2-sided test)", 0.86571807507),
        ],
    );
    check(
        runs[2],
        &[
            ("Test Value", 48.4166666667),
            ("Number of Runs", 20.0),
            ("Standard Error", 2.94743783153),
            ("Standardized Test Statistic", 0.188487624612),
            ("Asymptotic Sig. (2-sided test)", 0.850494415783),
        ],
    );
    check(
        runs[3],
        &[
            ("Number of Runs", 17.0),
            ("Standard Error", 2.4964701712),
            ("Standardized Test Statistic", 0.0890145713679),
            ("Asymptotic Sig. (2-sided test)", 0.929070329814),
        ],
    );
    // the exact binomial test of the first 24 cases, p0 away from one half: twice the smaller tail
    let t = run("SELECT IF ($CASENUM <= 24).\n\
         NPTESTS /ONESAMPLE TEST(cnt) BINOMIAL(TESTVALUE=0.3 SUCCESSCONTINUOUS=CUTPOINT(4)).");
    let b = table(&t, "One-Sample Binomial Test");
    check(
        b,
        &[
            ("Successes", 18.0),
            ("Exact Sig. (1-sided test)", 7.06177572374e-06),
            ("Exact Sig. (2-sided test)", 1.41235514475e-05),
        ],
    );
}

#[test]
fn one_sample_kolmogorov_smirnov_distributions() {
    let t = run(
        "NPTESTS /ONESAMPLE TEST(ex) KOLMOGOROV_SMIRNOV(EXPONENTIAL=CUSTOM(MEAN=4)).\n\
         NPTESTS /ONESAMPLE TEST(un) KOLMOGOROV_SMIRNOV(UNIFORM=CUSTOM(MIN=2 MAX=11)).\n\
         NPTESTS /ONESAMPLE TEST(cnt) KOLMOGOROV_SMIRNOV(POISSON=CUSTOM(MEAN=3)).\n\
         NPTESTS /ONESAMPLE TEST(cnt) KOLMOGOROV_SMIRNOV(POISSON=CUSTOM(MEAN=3.13888888888889)).\n\
         NPTESTS /ONESAMPLE TEST(x) KOLMOGOROV_SMIRNOV(NORMAL=CUSTOM(MEAN=20 SD=4)).\n\
         NPTESTS /ONESAMPLE TEST(x) KOLMOGOROV_SMIRNOV(NORMAL=SAMPLE(SIMULATION=FALSE)).\n\
         NPTESTS /ONESAMPLE TEST(ex) KOLMOGOROV_SMIRNOV(NORMAL=SAMPLE(SIMULATION=FALSE)).",
    );
    let pick = |title: &str, nth: usize| -> &Table {
        t.iter()
            .filter(|t| t.title == title)
            .nth(nth)
            .expect("table")
    };
    // the statistic is the larger of the two one-sided distances taken at each distinct value,
    // and the p-value is the first three terms of the Smirnov series
    let e = pick("One-Sample Kolmogorov-Smirnov Test (Exponential)", 0);
    check(
        e,
        &[
            ("Test Statistic", 0.0861849347067),
            ("Asymptotic Sig. (2-sided test)", 0.951935430387),
        ],
    );
    let u = pick("One-Sample Kolmogorov-Smirnov Test (Uniform)", 0);
    check(
        u,
        &[
            ("Test Statistic", 0.194444444444),
            ("Asymptotic Sig. (2-sided test)", 0.131419728341),
        ],
    );
    // the Poisson distances use the empirical distribution one step below each value
    let p = pick("One-Sample Kolmogorov-Smirnov Test (Poisson)", 0);
    check(
        p,
        &[
            ("Test Statistic", 0.0916763332267),
            ("Asymptotic Sig. (2-sided test)", 0.922758532671),
        ],
    );
    let p = pick("One-Sample Kolmogorov-Smirnov Test (Poisson)", 1);
    check(
        p,
        &[
            ("Test Statistic", 0.0605919809954),
            ("Asymptotic Sig. (2-sided test)", 0.999390837224),
        ],
    );
    let n = pick("One-Sample Kolmogorov-Smirnov Test (Normal)", 0);
    check(
        n,
        &[
            ("Test Statistic", 0.109224574425),
            ("Asymptotic Sig. (2-sided test)", 0.783683942894),
        ],
    );
    // parameters from the data: the Lilliefors significance (formula, then the table of
    // critical values; a statistic under the 0.2 point is reported as above 0.2)
    let l = pick("One-Sample Kolmogorov-Smirnov Test (Normal)", 1);
    check(
        l,
        &[
            ("Test Statistic", 0.139884996417),
            ("Lilliefors Sig. (2-sided test)", 0.0724611582033),
        ],
    );
    let l = pick("One-Sample Kolmogorov-Smirnov Test (Normal)", 2);
    check(
        l,
        &[
            ("Test Statistic", 0.171408566571),
            ("Lilliefors Sig. (2-sided test)", 0.00904773462341),
        ],
    );
}
