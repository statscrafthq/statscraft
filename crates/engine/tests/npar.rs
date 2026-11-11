//! NPAR TESTS and PARTIAL CORR. Expected numbers come from GNU PSPP (printed with ten
//! decimals; `reference_npar.rs` repeats those comparisons on more cases) where it follows the
//! manual, and else from an independent computation of the manual's formulas: a general
//! statistics library, the cases written out as often as their weight, and exact tests by
//! enumerating every arrangement of the small synthetic dataset below.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn fixture(name: &str) -> String {
    format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Runs the syntax on a new engine and returns the tables and the warnings.
fn run(source: &str) -> (Vec<Table>, Vec<String>) {
    let mut engine = Engine::default();
    let (commands, errors) = parse(source);
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

fn survey(syntax: &str) -> Vec<Table> {
    run(&format!(
        "GET FILE='{}'.\n{syntax}",
        fixture("survey_basic.sav")
    ))
    .0
}

/// Twelve cases: x and y measured twice, g two groups of six.
const SMALL: &str = "DATA LIST LIST /x y g.
BEGIN DATA
3 4 1
5 5 1
5 7 2
8 6 2
2 2 1
9 9 2
7 9 1
5 4 2
6 7 2
1 3 1
4 4 1
8 10 2
END DATA.";

fn small(syntax: &str) -> (Vec<Table>, Vec<String>) {
    run(&format!("{SMALL}\n{syntax}"))
}

fn num(table: &Table, row: usize, column: usize) -> f64 {
    match &table.rows[row][column] {
        Cell::Number(n) => *n,
        other => panic!("{} row {row} column {column}: {other:?}", table.title),
    }
}

fn empty(table: &Table, row: usize, column: usize) -> bool {
    table.rows[row][column] == Cell::Empty
}

/// The n-th table whose title starts with `title`.
fn nth<'t>(tables: &'t [Table], title: &str, n: usize) -> &'t Table {
    tables
        .iter()
        .filter(|t| t.title.starts_with(title))
        .nth(n)
        .unwrap_or_else(|| panic!("no table {title} #{n}"))
}

fn near(actual: f64, expected: f64, decimals: i32) {
    let allowed = 0.5 * 10f64.powi(-decimals) + 1e-12 * expected.abs();
    assert!(
        (actual - expected).abs() <= allowed,
        "{actual} is not {expected}"
    );
}

/// The label of a row, the text of its first label cell.
fn label(table: &Table, row: usize, column: usize) -> &str {
    match &table.rows[row][column] {
        Cell::Text(t) => t,
        _ => "",
    }
}

#[test]
fn chi_square_binomial_and_runs() {
    let tables = survey(
        "NPAR TESTS /CHISQUARE=region q1_satisfaction /CHISQUARE=q1_satisfaction (1,5) /EXPECTED=1 2 3 2 1
         /BINOMIAL=q5_1 /BINOMIAL(0.3)=q5_2 /BINOMIAL=q1_satisfaction(3) /BINOMIAL=q1_satisfaction(2,4)
         /RUNS(MEAN)=age /RUNS(MODE)=q1_satisfaction /RUNS(30)=age.",
    );
    let frequencies = nth(&tables, "Chi-Square Frequencies: q1_satisfaction", 0);
    assert_eq!(label(frequencies, 0, 0), "Very dissatisfied");
    near(num(frequencies, 0, 2), 35.8, 10);
    near(num(frequencies, 3, 3), 29.2, 10);
    let stats = nth(&tables, "Chi-Square Test Statistics", 0);
    near(num(stats, 0, 1), 5.75, 10);
    near(num(stats, 0, 2), 51.3072625698, 10);
    near(num(stats, 1, 2), 4.0, 10);
    near(num(stats, 2, 1), 0.2186125405771373, 12);
    let combined = nth(&tables, "Chi-Square Frequencies", 2);
    assert_eq!(combined.title, "Chi-Square Frequencies");
    near(num(combined, 1, 2), 39.7777777778, 10);
    near(
        num(nth(&tables, "Chi-Square Test Statistics", 1), 0, 1),
        49.9636871508,
        10,
    );

    let binomial = nth(&tables, "Binomial Test", 0);
    assert_eq!(label(binomial, 0, 2), "Selected");
    near(num(binomial, 0, 3), 58.0, 10);
    near(num(binomial, 1, 4), 0.71, 10);
    near(num(binomial, 0, 6), 2.579201364431024e-9, 15);
    let one_tailed = nth(&tables, "Binomial Test", 1);
    assert_eq!(one_tailed.columns[6], "Exact Sig. (1-tailed)");
    // P(X >= 80) of 200 cases at 0.3: the observed side is above the test proportion
    near(num(one_tailed, 0, 6), 0.0016368053245546304, 12);
    let cut = nth(&tables, "Binomial Test", 2);
    assert_eq!(label(cut, 0, 2), "<= 3");
    near(num(cut, 0, 4), 0.4245810056, 10);
    near(num(cut, 0, 6), 0.05167186194684383, 12);
    near(num(nth(&tables, "Binomial Test", 3), 2, 3), 89.0, 10);

    let runs = nth(&tables, "Runs Test", 0);
    near(num(runs, 0, 1), 50.845, 10);
    near(num(runs, 4, 1), 98.0, 10);
    near(num(runs, 5, 1), -0.4253340933, 10);
    near(num(nth(&tables, "Runs Test", 1), 5, 1), -0.6848145560, 10);
    let runs = nth(&tables, "Runs Test", 2);
    near(num(runs, 1, 1), 35.0, 10);
    near(num(runs, 5, 1), -0.4312267166, 10);
}

#[test]
fn one_sample_kolmogorov_smirnov() {
    let tables = survey(
        "NPAR TESTS /K-S(NORMAL 40 10)=age /K-S(UNIFORM)=age /K-S(POISSON)=q1_satisfaction
         /K-S(EXPONENTIAL)=monthly_spend /K-S(NORMAL)=age.",
    );
    let normal = nth(&tables, "One-Sample", 0);
    near(num(normal, 3, 2), 0.3913447461, 10);
    near(num(normal, 4, 2), 0.0581927987, 10);
    near(num(normal, 5, 2), -0.3913447461, 10);
    near(num(normal, 6, 2), 5.5344504745, 10);
    let uniform = nth(&tables, "One-Sample", 1);
    near(num(uniform, 2, 2), 84.0, 10);
    near(num(uniform, 6, 2), 0.5592571815, 10);
    // the manual's three terms of the series, within 1e-6 of the limiting distribution
    near(num(uniform, 7, 2), 0.9132189602521897, 6);
    // the Poisson compares the cdfs at each value and at the whole number below it
    let poisson = nth(&tables, "One-Sample", 2);
    near(num(poisson, 1, 2), 3.564245810055866, 10);
    near(num(poisson, 2, 2), 0.15099329380715343, 10);
    near(num(poisson, 4, 2), -0.13035680483901615, 10);
    near(num(poisson, 5, 2), 2.0201525894538936, 10);
    near(num(poisson, 6, 2), 0.0005705636712639746, 6);
    let exponential = nth(&tables, "One-Sample", 3);
    near(num(exponential, 2, 2), 0.1611506045786975, 10);
    near(num(exponential, 5, 2), 2.1680596489604316, 10);
    near(num(exponential, 6, 2), 0.000_165_29, 7);
    // estimated normal parameters: the Lilliefors significance, below 0.01 here
    let lilliefors = nth(&tables, "One-Sample", 4);
    near(num(lilliefors, 3, 2), 0.08496926166739543, 10);
    assert!(num(lilliefors, 7, 2) < 0.01);
    assert!(
        lilliefors
            .caption
            .as_deref()
            .unwrap()
            .contains("Lilliefors")
    );
}

#[test]
fn two_and_k_related_samples() {
    let tables = survey(
        "NPAR TESTS /WILCOXON=q5_1 q5_2 WITH q5_3 q5_4 (PAIRED) /SIGN=q1_satisfaction WITH q2_recommend
         /MCNEMAR=q5_1 q5_2 /FRIEDMAN=q5_1 q5_2 q5_3 /KENDALL=q5_1 q5_2 q5_3 /COCHRAN=q5_1 q5_2 q5_3
         /MH=q1_satisfaction WITH region /MH=q5_1 WITH q5_2.",
    );
    let ranks = nth(&tables, "Ranks", 0);
    assert_eq!(label(ranks, 0, 0), "q5_1 - q5_3");
    near(num(ranks, 0, 2), 72.0, 10);
    near(num(ranks, 1, 4), 1479.0, 10);
    near(num(ranks, 4, 3), 47.5, 10);
    let wilcoxon = nth(&tables, "Wilcoxon Signed Ranks Test", 0);
    near(num(wilcoxon, 0, 1), -4.2786599179, 10);
    near(num(wilcoxon, 0, 2), -3.3005479880, 10);
    // 151 differences: Z with the continuity correction, as the manual's formula has it
    let sign = nth(&tables, "Sign Test", 1);
    assert_eq!(sign.title, "Sign Test");
    near(num(sign, 0, 1), 9.928219197008145, 10);
    let crosstab = nth(&tables, "q5_1 & q5_2", 0);
    near(num(crosstab, 0, 2), 54.0, 10);
    let mcnemar = nth(&tables, "McNemar Test", 0);
    near(num(mcnemar, 1, 1), 5.127906976744186, 10);
    near(num(mcnemar, 2, 1), 0.023544102412552798, 9);
    let friedman = nth(&tables, "Friedman Test", 0);
    near(num(friedman, 1, 1), 18.3708609272, 10);
    near(num(nth(&tables, "Ranks", 1), 0, 1), 1.8375, 10);
    let kendall = nth(&tables, "Kendall's W Test", 0);
    near(num(kendall, 1, 1), 0.0459271523, 10);
    near(num(kendall, 2, 1), 18.3708609272, 10);
    let cochran = nth(&tables, "Cochran Test", 0);
    near(num(cochran, 1, 1), 18.3708609272, 10);
    near(num(nth(&tables, "Cochran Frequencies", 0), 2, 2), 101.0, 10);
    let mh = nth(&tables, "Marginal Homogeneity Test", 0);
    near(num(mh, 1, 1), 146.0, 10);
    near(num(mh, 2, 1), 519.0, 10);
    near(num(mh, 3, 1), 477.5, 10);
    near(num(mh, 4, 1), 12.459935794377111, 10);
    near(num(mh, 5, 1), 3.3306752687062815, 10);
    // two values: the McNemar test instead
    assert_eq!(
        tables.iter().filter(|t| t.title == "McNemar Test").count(),
        2
    );
}

#[test]
fn independent_samples() {
    let tables = survey(
        "NPAR TESTS /M-W=age BY gender(1,2) /K-S=age monthly_spend BY gender(1,2)
         /K-W=age BY region(1,4) /MEDIAN=age BY region(1,4) /MEDIAN=age BY gender(2,1)
         /J-T=age BY region(1,4).",
    );
    let ranks = nth(&tables, "Ranks", 0);
    assert_eq!(label(ranks, 0, 1), "Male");
    near(num(ranks, 0, 3), 98.6650943396, 10);
    near(num(ranks, 1, 4), 8456.5, 10);
    let mw = nth(&tables, "Mann-Whitney Test", 0);
    near(num(mw, 0, 1), 4540.5, 10);
    near(num(mw, 1, 1), 8456.5, 10);
    near(num(mw, 2, 1), -0.3172945146, 10);
    // two-sample K-S: F of the larger group (male) less F of the other
    let ks = nth(&tables, "Two-Sample Kolmogorov-Smirnov Test", 0);
    near(num(ks, 0, 2), 0.1031303602058319, 10);
    near(num(ks, 3, 2), 0.715121669836788, 10);
    near(num(ks, 3, 3), 1.0107122324365867, 10);
    near(
        num(nth(&tables, "Kruskal-Wallis Test", 0), 0, 1),
        7.5407717711,
        10,
    );
    let median = nth(&tables, "Median Test", 1);
    assert_eq!(median.title, "Median Test");
    near(num(median, 1, 1), 50.5, 10);
    near(num(median, 2, 1), 6.4799526540, 10);
    // two-sample: chi-square with the continuity correction
    let two = nth(&tables, "Median Test", 3);
    near(num(two, 2, 1), 0.02079759862778731, 10);
    near(num(two, 4, 1), 0.8853317028471128, 9);
    let groups = nth(&tables, "Median Test Frequencies", 1);
    assert_eq!(groups.columns[2], "Female");
    near(num(groups, 0, 2), 43.0, 10);
    // J-T: 5075.5 pairs in order, counted one by one in the check script
    let jt = nth(&tables, "Jonckheere-Terpstra Test", 0);
    near(num(jt, 0, 1), 4.0, 10);
    near(num(jt, 0, 3), 5075.5, 10);
    near(num(jt, 0, 4), 4400.5, 10);
    near(num(jt, 0, 5), 308.5277918453945, 9);
    near(num(jt, 0, 6), 2.1878093897558744, 9);
    near(num(jt, 0, 7), 0.028683489634294183, 9);
}

#[test]
fn exact_tests_of_a_small_sample() {
    let (tables, _) = small(
        "NPAR TESTS /M-W=x BY g(1,2) /W-W=x BY g(1,2) /MOSES=x BY g(1,2) /K-S=x BY g(1,2)
         /MEDIAN=x BY g(2,1) /SIGN=x WITH y /RUNS(MEDIAN)=x /MH=x WITH y /BINOMIAL(.3)=x(4)
         /K-S(EXPONENTIAL)=x /K-S(POISSON)=x.",
    );
    let mw = nth(&tables, "Mann-Whitney Test", 0);
    near(num(mw, 0, 1), 4.0, 10);
    near(num(mw, 1, 1), 25.0, 10);
    near(num(mw, 2, 1), -2.2616509994203624, 10);
    assert_eq!(label(mw, 4, 0), "Exact Sig. [2*(1-tailed Sig.)]");
    near(num(mw, 4, 1), 0.025974025974025976, 12);
    // tied values across the groups: 4 to 6 runs, each with its exact lower tail
    let ww = nth(&tables, "Wald-Wolfowitz Test", 0);
    assert_eq!(label(ww, 0, 1), "Minimum Possible");
    near(num(ww, 0, 2), 4.0, 10);
    near(num(ww, 0, 3), -1.5138251770487459, 10);
    near(num(ww, 0, 4), 0.0670995670995671, 12);
    near(num(ww, 1, 2), 6.0, 10);
    near(num(ww, 1, 4), 0.3917748917748918, 12);
    assert!(empty(ww, 0, 5));
    let moses = nth(&tables, "Moses Test", 0);
    near(num(moses, 0, 2), 9.0, 10);
    near(num(moses, 1, 2), 0.2727272727272727, 12);
    near(num(moses, 2, 2), 5.0, 10);
    near(num(moses, 3, 2), 0.2727272727272727, 12);
    near(num(moses, 4, 2), 1.0, 10);
    let ks = nth(&tables, "Two-Sample", 0);
    near(num(ks, 0, 2), 2.0 / 3.0, 12);
    near(num(ks, 3, 2), 1.1547005383792515, 12);
    // 12 cases: Fisher's exact test
    let median = nth(&tables, "Median Test", 1);
    near(num(median, 1, 1), 5.0, 10);
    assert_eq!(label(median, 2, 0), "Exact Sig.");
    near(num(median, 2, 1), 0.24242424242424238, 12);
    near(num(nth(&tables, "Sign Test", 1), 0, 1), 0.2890625, 12);
    let runs = nth(&tables, "Runs Test", 0);
    near(num(runs, 4, 1), 6.0, 10);
    let mh = nth(&tables, "Marginal Homogeneity Test", 0);
    near(num(mh, 0, 1), 10.0, 10);
    near(num(mh, 5, 1), -1.4596008983995234, 10);
    near(num(mh, 6, 1), 0.14439979221307134, 10);
    near(
        num(nth(&tables, "Binomial Test", 0), 0, 6),
        0.5074842265649997,
        12,
    );
    near(
        num(nth(&tables, "One-Sample", 0), 2, 2),
        0.2832235183483186,
        12,
    );
    let poisson = nth(&tables, "One-Sample", 1);
    near(num(poisson, 2, 2), 0.08924681708056748, 12);
    near(num(poisson, 3, 2), 0.06155231373306455, 12);
    near(num(poisson, 5, 2), 0.30916004319469753, 12);
}

#[test]
fn method_exact_adds_exact_significances() {
    let (tables, warnings) = small(
        "NPAR TESTS /M-W=x BY g(1,2) /WILCOXON=x WITH y /SIGN=x WITH y /RUNS(MEDIAN)=x
         /K-W=x BY g(1,2) /METHOD=EXACT.",
    );
    // Mann-Whitney given the tied ranks, every split of the 12 cases enumerated
    let mw = nth(&tables, "Mann-Whitney Test", 0);
    assert_eq!(label(mw, 5, 0), "Exact Sig. (2-tailed)");
    near(num(mw, 5, 1), 0.023809523809523808, 12);
    near(num(mw, 6, 1), 0.011904761904761904, 12);
    near(num(mw, 7, 1), 0.004329004329004329, 12);
    let wilcoxon = nth(&tables, "Wilcoxon Signed Ranks Test", 0);
    near(num(wilcoxon, 0, 1), -1.4433756729740645, 10);
    near(num(wilcoxon, 2, 1), 0.21875, 12);
    near(num(wilcoxon, 3, 1), 0.109375, 12);
    near(num(wilcoxon, 4, 1), 0.05859375, 12);
    let runs = nth(&tables, "Runs Test", 0);
    near(num(runs, 7, 1), 1.0, 12);
    near(num(runs, 8, 1), 0.2545454545454545, 12);
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("exact significance of the Kruskal-Wallis test is not computed"))
    );
}

#[test]
fn weights_repeat_cases() {
    let weighted = "COMPUTE w2 = RND(weight * 2).\nWEIGHT BY w2.\n";
    let tables = survey(&format!(
        "{weighted}NPAR TESTS /WILCOXON=q1_satisfaction WITH q2_recommend /M-W=age BY gender(2,1)
         /COCHRAN=q5_2 q5_5."
    ));
    near(
        num(nth(&tables, "Wilcoxon Signed Ranks Test", 0), 0, 1),
        -15.363751049660058,
        10,
    );
    let mw = nth(&tables, "Mann-Whitney Test", 0);
    near(num(mw, 0, 1), 30556.5, 10);
    near(num(mw, 3, 1), 0.6471174277713592, 10);
    near(num(nth(&tables, "Ranks", 1), 0, 4), 58997.5, 10);
    near(
        num(nth(&tables, "Cochran Test", 0), 1, 1),
        60.79136690647482,
        10,
    );
    // a weight below one half drops the case
    let (tables, _) =
        small("COMPUTE w = 1.\nIF (x = 9) w = 0.4.\nWEIGHT BY w.\nNPAR TESTS /RUNS(MEAN)=x.");
    near(num(nth(&tables, "Runs Test", 0), 3, 1), 11.0, 10);
}

#[test]
fn statistics_and_missing_values() {
    let tables =
        survey("NPAR TESTS /M-W=q1_satisfaction BY gender(1,2) /STATISTICS=ALL /MISSING=LISTWISE.");
    let descriptives = &tables[0];
    assert_eq!(descriptives.title, "Descriptive Statistics");
    assert_eq!(label(descriptives, 0, 0), "q1_satisfaction");
    near(num(descriptives, 0, 1), 179.0, 10);
    assert_eq!(descriptives.column_groups[6], ["Percentiles"]);
    near(num(descriptives, 1, 5), 3.0, 10);
    // INCLUDE makes 98 and 99 valid
    let tables = survey("NPAR TESTS /RUNS(MEAN)=q1_satisfaction /MISSING=INCLUDE /STATISTICS.");
    near(num(&tables[0], 0, 1), 200.0, 10);
    assert_eq!(tables[0].columns.len(), 6);
}

#[test]
fn errors_and_warnings() {
    let mut engine = Engine::default();
    let (commands, _) = parse(&format!(
        "GET FILE='{}'.\nNPAR TESTS /M-W=plan BY gender(1,2).\n\
         NPAR TESTS /SIGN=age q2_recommend WITH q1_satisfaction (PAIRED).\n\
         NPAR TESTS /CHISQUARE=q1_satisfaction /EXPECTED=1 2 3.\n\
         NPAR TESTS /MEDIAN=age BY gender(1,1).",
        fixture("survey_basic.sav")
    ));
    let messages: Vec<String> = commands
        .iter()
        .filter_map(|c| engine.execute(&c.command).err())
        .map(|e| e.0)
        .collect();
    assert_eq!(
        messages,
        [
            "NPAR TESTS needs numeric variables, but plan is a string variable",
            "(PAIRED) needs as many variables after WITH as before it, got 2 and 1",
            "CHISQUARE: EXPECTED gives 3 frequencies, but q1_satisfaction has 5 categories",
            "NPAR TESTS: the two values of gender must differ",
        ]
    );
    let (tables, warnings) = small(
        "NPAR TESTS /BINOMIAL=x /K-S(UNIFORM 2 8)=x /MCNEMAR=x WITH y /WILCOXON=ALL /COCHRAN=x y.",
    );
    assert!(tables.iter().all(|t| t.title != "Binomial Test"));
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("BINOMIAL: x has 9 values"))
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("K-S: x has values outside the uniform range"))
    );
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("MCNEMAR: x and y have 10 values together"))
    );
    assert!(warnings.iter().any(|w| w.contains("COCHRAN")));
    // ALL pairs each variable with every later one and never with itself
    assert_eq!(
        nth(&tables, "Wilcoxon Signed Ranks Test", 0).columns.len(),
        4
    );
}

#[test]
fn partial_correlations() {
    let tables = survey(
        "PARTIAL CORR /VARIABLES=age q2_recommend monthly_spend BY q1_satisfaction /STATISTICS=ALL.",
    );
    let descriptives = nth(&tables, "Descriptive Statistics", 0);
    near(num(descriptives, 0, 1), 50.2, 10);
    near(num(descriptives, 2, 2), 115.00557348, 8);
    near(num(descriptives, 3, 3), 140.0, 10);
    let correlations = nth(&tables, "Correlations", 0);
    // zero-order block: four variables, three rows each
    assert_eq!(label(correlations, 0, 0), "-none-");
    near(num(correlations, 0, 4), -0.07433726884934834, 12);
    near(num(correlations, 1, 4), 0.38272128511317616, 10);
    near(num(correlations, 2, 4), 138.0, 10);
    assert_eq!(label(correlations, 12, 0), "q1_satisfaction");
    near(num(correlations, 12, 5), 0.120795624068679, 12);
    near(num(correlations, 13, 5), 0.15663168543971395, 10);
    near(num(correlations, 14, 5), 137.0, 10);
    assert!(empty(correlations, 12, 6));
    // pairwise: df from the smallest count of the correlations used; orders 1 and 2
    let tables = survey(
        "PARTIAL CORR age WITH q2_recommend BY q1_satisfaction region (1,2) /MISSING=ANALYSIS
         /SIGNIFICANCE=ONETAIL.",
    );
    let t = &tables[0];
    let expected = [
        (
            "q1_satisfaction",
            -0.08898182739937369,
            153.0,
            0.27088223385802956,
        ),
        ("region", -0.08768600500533276, 173.0, 0.24854610548459583),
        (
            "q1_satisfaction & region",
            -0.09170368869027606,
            152.0,
            0.2580019573153329,
        ),
    ];
    for (k, (controls, r, df, sig)) in expected.into_iter().enumerate() {
        assert_eq!(label(t, 3 * k, 0), controls);
        near(num(t, 3 * k, 3), r, 12);
        near(num(t, 3 * k + 1, 3), sig / 2.0, 10);
        near(num(t, 3 * k + 2, 3), df, 10);
    }
    assert_eq!(label(t, 1, 2), "Significance (1-tailed)");
    // CONDENSED: coefficients alone, stars for significance; SERIAL: each pair once
    let tables = survey(
        "PARTIAL CORR age monthly_spend q2_recommend BY q1_satisfaction /FORMAT=CONDENSED.
         PARTIAL CORR age monthly_spend q2_recommend BY q1_satisfaction /FORMAT=SERIAL.",
    );
    assert_eq!(tables[0].rows.len(), 3);
    assert_eq!(tables[0].rows[0][3], Cell::Number(1.0));
    let serial = &tables[1];
    assert!(empty(serial, 0, 3) && !empty(serial, 0, 4) && empty(serial, 3, 4));
}

#[test]
fn partial_corr_bad_correlations() {
    // c is constant among the complete cases, so its correlations cannot be computed
    let (tables, _) = run("DATA LIST LIST /a b c.
BEGIN DATA
1 2 5
2 1 5
3 4 5
4 3 5
END DATA.
PARTIAL CORR a b BY c /STATISTICS=BADCORR.");
    let t = &tables[0];
    assert_eq!(label(t, 0, 0), "-none-");
    assert!(empty(t, 0, 5));
    assert_eq!(label(t, 9, 0), "c");
    assert!(empty(t, 9, 4));
}
