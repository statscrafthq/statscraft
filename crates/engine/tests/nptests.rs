//! NPTESTS. Expected numbers come from a general statistics library run on the same invented
//! data (rank tests, exact binomial and sign tests, Clopper-Pearson, Jeffreys and
//! likelihood-ratio limits, Hodges-Lehmann estimates) or from the formulas worked by hand.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Twenty tastings: a strength reading, three panels, a flag, an ordinal mood, three timed
/// trials and three yes/no checks.
const DATA: &str = "DATA LIST LIST /brew panel flag mood t1 t2 t3 c1 c2 c3.
BEGIN DATA
4.2 1 0 2 11 13 12 0 1 1
5.1 1 1 3 14 15 17 1 1 1
3.8 1 0 1 9 12 11 0 0 1
6.0 1 1 4 16 15 19 1 1 0
4.9 1 0 2 12 14 15 0 1 1
5.5 2 1 3 13 17 16 1 1 1
6.3 2 1 4 15 18 20 1 0 1
7.1 2 0 3 17 16 21 0 1 1
5.8 2 1 2 14 19 18 1 1 0
6.6 2 1 4 18 20 22 1 1 1
7.4 3 0 3 16 21 19 0 1 1
8.0 3 1 4 19 22 24 1 1 1
6.9 3 1 2 15 20 23 1 0 1
7.7 3 0 3 18 19 22 0 1 1
8.5 3 1 4 20 23 25 1 1 1
4.4 1 0 1 10 11 14 0 0 0
5.0 2 1 2 13 16 15 1 1 1
7.9 3 1 3 17 22 21 1 1 1
3.9 1 0 1 8 10 13 0 0 1
6.1 2 0 3 15 17 18 0 1 1
END DATA.
VARIABLE LEVEL brew t1 t2 t3 (SCALE) /panel flag c1 c2 c3 (NOMINAL) /mood (ORDINAL).";

fn run(syntax: &str) -> (Vec<Table>, Vec<String>) {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}\n{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let (mut tables, mut warnings) = (Vec::new(), Vec::new());
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
        warnings.extend(engine.warnings().iter().cloned());
    }
    (tables, warnings)
}

fn error(syntax: &str) -> String {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}\n{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            return e.to_string();
        }
    }
    panic!("no error");
}

fn table<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

/// The number in the row labeled `row`, column `column` (1 is the first value column).
fn at(table: &Table, row: &str, column: usize) -> f64 {
    let found = table
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(t) if t == row))
        .unwrap_or_else(|| panic!("{}: no row {row}", table.title));
    match &found[column] {
        Cell::Number(n) => *n,
        other => panic!("{} {row}: {other:?}", table.title),
    }
}

fn number(table: &Table, row: usize, column: usize) -> f64 {
    match &table.rows[row][column] {
        Cell::Number(n) => *n,
        other => panic!("{} row {row} column {column}: {other:?}", table.title),
    }
}

fn text(table: &Table, row: usize, column: usize) -> &str {
    match &table.rows[row][column] {
        Cell::Text(t) => t,
        _ => "",
    }
}

fn near(actual: f64, expected: f64, decimals: i32) {
    let allowed = 0.5 * 10f64.powi(-decimals);
    assert!(
        (actual - expected).abs() <= allowed,
        "{actual} is not {expected}"
    );
}

#[test]
fn binomial_with_intervals_and_one_sample_wilcoxon() {
    let (tables, _) = run(
        "NPTESTS /ONESAMPLE TEST(flag brew) BINOMIAL(CLOPPERPEARSON JEFFREYS LIKELIHOOD) \
         WILCOXON(TESTVALUE=6).",
    );
    let summary = table(&tables, "Hypothesis Test Summary");
    // flag: 9 cases of 0 (the first value met) out of 20
    assert_eq!(text(summary, 0, 0), "The proportion of flag = 0 is 0.5.");
    near(number(summary, 0, 2), 0.8238029, 7);
    assert_eq!(text(summary, 0, 3), "Exact");
    assert_eq!(text(summary, 0, 4), "Retain the null hypothesis.");
    let intervals = table(&tables, "Confidence Interval Summary");
    near(number(intervals, 0, 2), 0.45, 12);
    near(number(intervals, 0, 3), 0.2305779, 7);
    near(number(intervals, 0, 4), 0.6847219, 7);
    near(number(intervals, 1, 3), 0.2510234, 7);
    near(number(intervals, 1, 4), 0.6623350, 7);
    near(number(intervals, 2, 3), 0.2480296, 6);
    near(number(intervals, 2, 4), 0.6640606, 6);
    // brew, a scale field, is cut at the midpoint of 3.8 and 8.5: 11 at or below
    let binomial = table(&tables, "One-Sample Binomial Test");
    assert_eq!(at(binomial, "Successes", 2), 11.0);
    let wilcoxon = table(&tables, "One-Sample Wilcoxon Signed Rank Test");
    assert_eq!(at(wilcoxon, "Test Statistic", 1), 99.0);
    near(
        at(wilcoxon, "Asymptotic Sig. (2-sided test)", 1),
        0.8720924,
        7,
    );
}

#[test]
fn automatic_one_sample_choice() {
    let (tables, warnings) = run("NPTESTS /ONESAMPLE TEST(flag panel brew mood).");
    let summary = table(&tables, "Hypothesis Test Summary");
    let tests: Vec<&str> = (0..summary.rows.len())
        .map(|r| text(summary, r, 1))
        .collect();
    assert_eq!(
        tests,
        [
            "One-Sample Binomial Test",
            "One-Sample Chi-Square Test",
            "One-Sample Kolmogorov-Smirnov Test",
            "One-Sample Chi-Square Test",
        ]
    );
    // panel: 7, 7 and 6 cases against 20/3 each
    near(number(summary, 1, 2), 0.9512294, 7);
    assert_eq!(text(summary, 2, 3), "Monte Carlo");
    let ks = table(&tables, "One-Sample Kolmogorov-Smirnov Test (Normal)");
    near(at(ks, "Test Statistic", 1), 0.09615185, 8);
    let p = at(ks, "Monte Carlo Sig. (2-sided test)", 1);
    assert!(p > 0.5 && p <= 1.0, "{p}");
    assert!(at(ks, "Monte Carlo Sig. Lower Limit", 1) <= p);
    assert!(warnings.is_empty(), "{warnings:?}");
    // the same seed gives the same draws
    let (again, _) = run("NPTESTS /ONESAMPLE TEST(brew).");
    let ks2 = table(&again, "One-Sample Kolmogorov-Smirnov Test (Normal)");
    assert_eq!(at(ks2, "Monte Carlo Sig. (2-sided test)", 1), p);
}

#[test]
fn kolmogorov_smirnov_choices() {
    let (tables, _) = run(
        "NPTESTS /ONESAMPLE TEST(brew) KOLMOGOROV_SMIRNOV(NORMAL=SAMPLE(SIMULATION=FALSE) \
         UNIFORM=CUSTOM(MIN=3 MAX=9) POISSON=CUSTOM(MEAN=6)).",
    );
    let summary = table(&tables, "Hypothesis Test Summary");
    assert_eq!(text(summary, 0, 3), "Lilliefors");
    assert_eq!(text(summary, 0, 2), ">0.200");
    let uniform = table(&tables, "One-Sample Kolmogorov-Smirnov Test (Uniform)");
    near(at(uniform, "Test Statistic", 1), 0.1333333, 7);
    near(
        at(uniform, "Asymptotic Sig. (2-sided test)", 1),
        0.8691636,
        4,
    );
    // brew has fractions: no Poisson test
    assert!(
        tables
            .iter()
            .all(|t| t.title != "One-Sample Kolmogorov-Smirnov Test (Poisson)")
    );
}

#[test]
fn independent_samples_with_comparisons() {
    let (tables, _) = run(
        "NPTESTS /INDEPENDENT TEST(brew) GROUP(panel) KRUSKAL_WALLIS JONCKHEERE_TERPSTRA \
         MEDIAN(COMPARE=STEPWISE).",
    );
    let kw = table(&tables, "Independent-Samples Kruskal-Wallis Test");
    near(at(kw, "Test Statistic", 1), 15.03537, 5);
    near(at(kw, "Asymptotic Sig.", 1), 0.0005433879, 9);
    let jt = table(&tables, "Independent-Samples Jonckheere-Terpstra Test");
    assert_eq!(at(jt, "Test Statistic", 1), 128.0);
    let pairwise = tables
        .iter()
        .find(|t| t.title == "Pairwise Comparisons of brew by panel")
        .unwrap();
    assert_eq!(text(pairwise, 0, 0), "1-2");
    near(number(pairwise, 0, 1), -6.0, 10);
    near(number(pairwise, 0, 2), 3.162278, 6);
    near(number(pairwise, 0, 4), 0.05777957, 8);
    near(number(pairwise, 0, 5), 3.0 * 0.05777957, 7);
    // every panel's median lies on its own side of the others
    let subsets = table(&tables, "Homogeneous Subsets of brew by panel");
    assert_eq!(subsets.columns.len(), 4);
}

#[test]
fn two_independent_samples() {
    let (tables, _) = run("SELECT IF panel < 3.
         NPTESTS /INDEPENDENT TEST(brew) GROUP(panel) MANN_WHITNEY HODGES_LEHMAN WALD_WOLFOWITZ \
         KOLMOGOROV_SMIRNOV MOSES /CRITERIA CILEVEL=95.");
    let mw = table(&tables, "Independent-Samples Mann-Whitney U Test");
    // 45 of the 49 pairs have the first panel's reading below
    assert_eq!(at(mw, "Mann-Whitney U", 1), 45.0);
    near(at(mw, "Exact Sig. (2-sided test)", 1), 0.006993007, 9);
    let intervals = table(&tables, "Confidence Interval Summary");
    near(number(intervals, 0, 2), -1.5, 9);
    near(number(intervals, 0, 3), -2.3, 9);
    near(number(intervals, 0, 4), -0.6, 9);
    let moses = table(
        &tables,
        "Independent-Samples Moses Test of Extreme Reaction",
    );
    assert_eq!(at(moses, "Observed Control Group Span", 1), 10.0);
    assert_eq!(at(moses, "Records Trimmed from Each End", 1), 1.0);
    let ww = table(&tables, "Independent-Samples Wald-Wolfowitz Runs Test");
    assert_eq!(at(ww, "Number of Runs", 1), 6.0);
}

#[test]
fn related_samples() {
    let (tables, _) = run("NPTESTS /RELATED TEST(t1 t2 t3 c1 c2 c3).
         NPTESTS /RELATED TEST(t1 t2) WILCOXON SIGN HODGES_LEHMAN.");
    let friedman = table(
        &tables,
        "Related-Samples Friedman's Two-Way Analysis of Variance by Ranks",
    );
    near(at(friedman, "Test Statistic", 1), 25.9, 10);
    near(at(friedman, "Asymptotic Sig.", 1), 2.376219e-06, 12);
    let cochran = table(&tables, "Related-Samples Cochran's Q Test");
    near(at(cochran, "Test Statistic", 1), 4.666667, 6);
    near(at(cochran, "Asymptotic Sig.", 1), 0.09697197, 8);
    let pairwise = table(&tables, "Pairwise Comparisons of t1, t2, t3");
    near(number(pairwise, 0, 1), -1.15, 10);
    near(number(pairwise, 0, 2), 0.3162278, 7);
    near(number(pairwise, 0, 4), 0.0002762397, 10);
    let wilcoxon = table(&tables, "Related-Samples Wilcoxon Signed Rank Test");
    assert_eq!(at(wilcoxon, "Test Statistic", 1), 6.0);
    near(
        at(wilcoxon, "Asymptotic Sig. (2-sided test)", 1),
        0.0002002676,
        9,
    );
    let sign = table(&tables, "Related-Samples Sign Test");
    near(at(sign, "Exact Sig. (2-sided test)", 1), 0.0004024506, 10);
    let intervals = table(&tables, "Confidence Interval Summary");
    near(number(intervals, 0, 2), -2.5, 9);
    near(number(intervals, 0, 3), -3.5, 9);
    near(number(intervals, 0, 4), -2.0, 9);
}

#[test]
fn stepwise_subsets_of_related_fields() {
    let (tables, _) = run("NPTESTS /RELATED TEST(t1 t2 t3) FRIEDMAN(COMPARE=STEPWISE).");
    let subsets = table(&tables, "Homogeneous Subsets of t1, t2, t3");
    // t1 stands apart; t2 and t3 do not differ (Friedman of the two: 1.8, p 0.18)
    assert_eq!(subsets.columns, ["Sample", "Subset 1", "Subset 2"]);
    assert_eq!(text(subsets, 0, 0), "t1");
    assert_eq!(subsets.rows[1][1], Cell::Empty);
    near(number(subsets, 3, 2), 1.8, 10);
    near(number(subsets, 4, 2), 0.1797125, 7);
}

#[test]
fn weights_repeat_cases_and_missing_values() {
    let doubled = run("COMPUTE w = 2.
         WEIGHT BY w.
         NPTESTS /INDEPENDENT TEST(brew) GROUP(panel) KRUSKAL_WALLIS(COMPARE=NONE).")
    .0;
    let kw = table(&doubled, "Independent-Samples Kruskal-Wallis Test");
    assert_eq!(at(kw, "Total N", 1), 40.0);
    // the same as every case written twice
    near(at(kw, "Test Statistic", 1), 30.86208, 5);
    // a user-missing panel code drops cases unless USERMISSING=INCLUDE; a user-missing
    // reading stays missing either way
    let (tables, _) = run("MISSING VALUES panel (3) brew (8.5).
         NPTESTS /INDEPENDENT TEST(brew) GROUP(panel) /MISSING USERMISSING=INCLUDE.
         NPTESTS /INDEPENDENT TEST(brew) GROUP(panel).");
    let kw = tables
        .iter()
        .find(|t| t.title == "Independent-Samples Kruskal-Wallis Test")
        .unwrap();
    assert_eq!(at(kw, "Total N", 1), 19.0);
    let mw = table(&tables, "Independent-Samples Mann-Whitney U Test");
    assert_eq!(at(mw, "Total N", 1), 14.0);
}

#[test]
fn listwise_drops_cases_missing_anywhere() {
    let (tables, _) = run("RECODE t3 (25=SYSMIS).
         NPTESTS /ONESAMPLE TEST(brew) WILCOXON(TESTVALUE=6) /RELATED TEST(t2 t3) WILCOXON
         /MISSING SCOPE=LISTWISE.");
    let one = table(&tables, "One-Sample Wilcoxon Signed Rank Test");
    assert_eq!(at(one, "Total N", 1), 19.0);
}

#[test]
fn string_fields_and_errors() {
    let (tables, _) = run("STRING site (A4).
         RECODE panel (1='east') (2='west') (3='west') INTO site.
         VARIABLE LEVEL site (NOMINAL).
         NPTESTS /ONESAMPLE TEST(site) /INDEPENDENT TEST(brew) GROUP(site).");
    let summary = table(&tables, "Hypothesis Test Summary");
    assert_eq!(text(summary, 0, 0), "The proportion of site = east is 0.5.");
    assert_eq!(
        text(summary, 1, 1),
        "Independent-Samples Mann-Whitney U Test"
    );
    assert!(error("NPTESTS /INDEPENDENT TEST(t1) GROUP(brew).").contains("nominal or ordinal"));
    assert!(
        error("WEIGHT BY flag.\nNPTESTS /ONESAMPLE TEST(flag brew).").contains("weight variable")
    );
}
