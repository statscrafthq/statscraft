//! The POWER commands for means and proportions, on invented study plans. Expected powers were
//! computed in a general statistics package run as a black box (its t-test and two-proportion
//! power functions, and its t, normal, binomial and hypergeometric distributions).

use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Chart, Table};

struct Run {
    table: Table,
    charts: Vec<Chart>,
    warnings: Vec<String>,
}

fn run(source: &str) -> Run {
    let result = Session::default()
        .run(source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let command = result.commands.into_iter().last().unwrap();
    Run {
        table: command
            .tables
            .into_iter()
            .find(|t| t.title == "Power Analysis")
            .unwrap_or_else(|| panic!("no table: {source}")),
        charts: command.charts,
        warnings: command.warnings,
    }
}

fn fails(source: &str) -> String {
    let errors = Session::default().run(source, None).unwrap_err();
    errors
        .iter()
        .map(|e| e.text())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// The number in `row` under the column titled `column`.
fn at(table: &Table, row: usize, column: &str) -> f64 {
    let i = table
        .columns
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("no column {column} in {:?}", table.columns));
    match &table.rows[row][i] {
        Cell::Number(n) => *n,
        other => panic!("{other:?} is no number"),
    }
}

fn close(got: f64, expected: f64, tolerance: f64) {
    assert!((got - expected).abs() < tolerance, "{got} != {expected}");
}

#[test]
fn one_sample_mean_power_and_size() {
    // a reading test: mean 1.5 above the norm, sd 4
    let r = run("POWER MEANS ONESAMPLE /PARAMETERS N=25 SD=4 MEAN=11.5 NULL=10.");
    close(at(&r.table, 0, "Power"), 0.4364995963, 1e-8);
    close(at(&r.table, 0, "Effect Size"), 0.375, 1e-12);
    assert_eq!(r.table.column_groups[2], vec!["Test Assumptions"]);
    assert!(r.table.caption.as_ref().unwrap().starts_with("Two-sided"));

    let r = run("POWER MEANS ONESAMPLE /PARAMETERS N=25 SD=4 MEAN=1.5 TEST=DIRECTIONAL.");
    close(at(&r.table, 0, "Power"), 0.5702133975, 1e-8);

    // the continuous roots are 57.76 and 109.15
    let r = run("POWER MEANS ONESAMPLE /PARAMETERS POWER=0.8 SD=4 MEAN=1.5.");
    assert_eq!(at(&r.table, 0, "N"), 58.0);
    assert!(at(&r.table, 0, "Actual Power") >= 0.8);
    assert_eq!(at(&r.table, 0, "Power"), 0.8);
    let r = run("POWER MEANS ONESAMPLE /PARAMETERS POWER=0.9 SD=4 MEAN=1.5 SIGNIFICANCE=0.01.");
    assert_eq!(at(&r.table, 0, "N"), 110.0);
}

#[test]
fn independent_means_equal_and_separate_variances() {
    let r = run("POWER MEANS INDEPENDENT /PARAMETERS N=30 45 SD=10 MEAN=6.");
    close(at(&r.table, 0, "Power"), 0.7095330388, 1e-8);
    let r = run("POWER MEANS INDEPENDENT /PARAMETERS N=30 45 SD=8 12 MEAN=20 14.");
    close(at(&r.table, 0, "Power"), 0.7270145539, 1e-8);
    close(at(&r.table, 0, "Mean Difference"), 6.0, 1e-12);
    let r = run("POWER MEANS INDEPENDENT /PARAMETERS N=30 45 SD=8 12 MEAN=6 TEST=DIRECTIONAL.");
    close(at(&r.table, 0, "Power"), 0.8235608338, 1e-8);

    // equal groups: the root is 44.59 per group
    let r = run("POWER MEANS INDEPENDENT /PARAMETERS POWER=0.8 SD=10 MEAN=6.");
    assert_eq!((at(&r.table, 0, "N1"), at(&r.table, 0, "N2")), (45.0, 45.0));
    // twice as many in group 2: the root 33.36 gives 34 and 67
    let r = run("POWER MEANS INDEPENDENT /PARAMETERS POWER=0.8 SD=10 MEAN=6 NRATIO=2.");
    assert_eq!((at(&r.table, 0, "N1"), at(&r.table, 0, "N2")), (34.0, 67.0));
    close(at(&r.table, 0, "Actual Power"), 0.8055974624, 1e-8);
}

#[test]
fn paired_means_combine_two_sds() {
    let r =
        run("POWER MEANS RELATED /PARAMETERS NPAIRS=40 SD=6.5 7 RHO=0.7 MEAN=3 TEST=DIRECTIONAL.");
    close(at(&r.table, 0, "Std. Deviation"), 5.2488093888, 1e-8);
    close(at(&r.table, 0, "Power"), 0.9716909975, 1e-8);
}

#[test]
fn one_proportion_normal_and_exact() {
    let base = "POWER PROPORTIONS ONESAMPLE /PARAMETERS NULL=0.3 ALTERNATIVE=0.42";
    let r = run(&format!("{base} N=120."));
    close(at(&r.table, 0, "Power"), 0.8005571099, 1e-8);
    let r = run(&format!("{base} N=120 CONTINUITY=TRUE."));
    close(at(&r.table, 0, "Power"), 0.7737125157, 1e-8);
    let r = run(&format!("{base} N=120 ESTIMATE=BINOMIAL."));
    close(at(&r.table, 0, "Power"), 0.7638396582, 1e-8);
    assert!(r.table.caption.as_ref().unwrap().contains("Exact binomial"));

    // the closed formula for a one-sided test gives 94.93
    let r = run(&format!("{base} POWER=0.8 TEST=DIRECTIONAL."));
    assert_eq!(at(&r.table, 0, "N"), 95.0);
    // exact power rises in steps; the smallest size that reaches 0.8 is 127
    let r = run(&format!("{base} POWER=0.8 ESTIMATE=BINOMIAL."));
    assert_eq!(at(&r.table, 0, "N"), 127.0);
    assert!(at(&r.table, 0, "Actual Power") >= 0.8);
    let r = run(&format!("{base} N=126 ESTIMATE=BINOMIAL."));
    assert!(at(&r.table, 0, "Power") < 0.8);

    let r = run(&format!("{base} N=700 ESTIMATE=BINOMIAL."));
    assert!(
        r.warnings.iter().any(|w| w.contains("500")),
        "{:?}",
        r.warnings
    );
}

#[test]
fn two_proportions_by_method() {
    let base = "POWER PROPORTIONS INDEPENDENT /PARAMETERS PROPORTIONS=0.22 0.35";
    let r = run(&format!("{base} N=100 100."));
    close(at(&r.table, 0, "Power"), 0.5307917818, 1e-8);
    close(at(&r.table, 0, "Risk Difference"), -0.13, 1e-12);
    close(at(&r.table, 0, "Risk Ratio"), 0.22 / 0.35, 1e-12);
    // the continuous root is 188.10 per group
    let r = run(&format!("{base} POWER=0.8."));
    assert_eq!(
        (at(&r.table, 0, "N1"), at(&r.table, 0, "N2")),
        (189.0, 189.0)
    );

    let exact =
        "POWER PROPORTIONS INDEPENDENT /PARAMETERS PROPORTIONS=0.2 0.5 N=30 40 ESTIMATE=BINOMIAL";
    let r = run(&format!("{exact}."));
    close(at(&r.table, 0, "Power"), 0.7646134945, 1e-8);
    let r = run(&format!("{exact} METHOD=FISHER."));
    close(at(&r.table, 0, "Power"), 0.6625424009, 1e-8);
    // the other methods, separate variances for the t-test
    for (method, estimate, expected) in [
        ("T", "NORMAL", 0.7846248969),
        ("T", "BINOMIAL", 0.7709128201),
        ("LRT", "NORMAL", 0.7465668222),
        ("LRT", "BINOMIAL", 0.7646136521),
        ("FISHER", "NORMAL", 0.6667415318),
    ] {
        let r = run(&format!(
            "POWER PROPORTIONS INDEPENDENT /PARAMETERS PROPORTIONS=0.2 0.5 N=30 40 \
             METHOD={method} ESTIMATE={estimate} POOLED=FALSE."
        ));
        close(at(&r.table, 0, "Power"), expected, 1e-8);
    }
    let r = run(&format!(
        "{base} POWER=0.8 TEST=DIRECTIONAL CONTINUITY=TRUE NRATIO=1.5."
    ));
    // closed formula with continuity correction: 137 and 206
    assert_eq!(
        (at(&r.table, 0, "N1"), at(&r.table, 0, "N2")),
        (137.0, 206.0)
    );
    // exact chi-square: 38 per group gives 0.7997, 39 gives 0.8128
    let r = run(
        "POWER PROPORTIONS INDEPENDENT /PARAMETERS PROPORTIONS=0.2 0.5 POWER=0.8 ESTIMATE=BINOMIAL.",
    );
    assert_eq!((at(&r.table, 0, "N1"), at(&r.table, 0, "N2")), (39.0, 39.0));
    close(at(&r.table, 0, "Actual Power"), 0.8128375881, 1e-8);
}

#[test]
fn paired_proportions_normal_and_exact() {
    let base = "POWER PROPORTIONS RELATED /PARAMETERS NPAIRS=80 VALUES=PROPORTIONS(0.1 0.25)";
    let r = run(&format!("{base}."));
    close(at(&r.table, 0, "Power"), 0.6248476534, 1e-8);
    close(at(&r.table, 0, "Risk Difference"), 0.15, 1e-12);
    let r = run(&format!("{base} ESTIMATE=BINOMIAL."));
    close(at(&r.table, 0, "Power"), 0.5561467851, 1e-8);
    // the same pairs as counts
    let r = run("POWER PROPORTIONS RELATED /PARAMETERS NPAIRS=80 VALUES=COUNTS(8 20).");
    close(at(&r.table, 0, "Power"), 0.6248476534, 1e-8);
    // marginal rates 0.4 and 0.55 with correlation 0.5
    let r = run(
        "POWER PROPORTIONS RELATED /PARAMETERS POWER=0.9 VALUES=PROPORTIONS(0.4 0.55) \
         MARGINAL=TRUE CORRELATION=0.5 /PLOT CORRELATION MARGINAL.",
    );
    // the continuous root is 120.14
    assert_eq!(at(&r.table, 0, "N Pairs"), 121.0);
    close(at(&r.table, 0, "Actual Power"), 0.9020679734, 1e-8);
    assert_eq!(r.charts.len(), 2);
    assert!(
        fails(
            "POWER PROPORTIONS RELATED /PARAMETERS NPAIRS=50 VALUES=PROPORTIONS(0.1 0.9) \
         MARGINAL=TRUE CORRELATION=0.9."
        )
        .contains("cannot have the correlation")
    );
}

#[test]
fn power_lists_and_charts() {
    let r = run(
        "POWER MEANS ONESAMPLE /PARAMETERS POWER=0.6 TO 0.9 BY 0.1 0.7 SD=4 MEAN=1.5 /PLOT N(5 9) ES.",
    );
    assert_eq!(r.table.rows.len(), 4);
    let sizes: Vec<f64> = (0..4).map(|i| at(&r.table, i, "N")).collect();
    assert!(sizes.windows(2).all(|w| w[0] < w[1]), "{sizes:?}");
    assert_eq!(r.charts.len(), 1);
    assert_eq!(r.warnings.len(), 3, "{:?}", r.warnings);

    let r = run(
        "POWER MEANS ONESAMPLE /PARAMETERS N=20 SD=4 MEAN=2 /PLOT N ES(0 2) N_XAXIS(5 30) ES_YAXIS.",
    );
    assert_eq!(r.charts.len(), 3);
    assert_eq!(r.charts[0].title, "Power by sample size");
    assert!(r.charts[2].kind_text().contains("three dimensions"));
    assert!(
        fails("POWER MEANS ONESAMPLE /PARAMETERS N=20 SD=4 MEAN=2 /PLOT ES(0 7).")
            .contains("0 to 5")
    );

    let r = run(
        "POWER PROPORTIONS INDEPENDENT /PARAMETERS PROPORTIONS=0.3 0.45 N=60 60 \
         /PLOT RISK_DIFF RISK_RATIO ODDS_RATIO(0.5 3) NRATIO N1_XAXIS N2_YAXIS P1_YAXIS P2_XAXIS.",
    );
    assert_eq!(r.charts.len(), 6);
}

#[test]
fn errors_without_an_effect() {
    assert!(
        fails("POWER MEANS ONESAMPLE /PARAMETERS POWER=0.8 SD=2 MEAN=5 NULL=5.")
            .contains("no difference")
    );
}

/// The tables of a run by title.
fn tables(source: &str) -> (Vec<Table>, Vec<String>, usize) {
    let result = Session::default()
        .run(source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let command = result.commands.into_iter().last().unwrap();
    (command.tables, command.warnings, command.charts.len())
}

fn titled<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no {title} table"))
}

#[test]
fn oneway_anova_overall_contrast_and_pairs() {
    // four teaching methods: sizes 12 15 10 13, mean scores 20 23 25 21, sd 4.5
    let base = "POWER ONEWAY ANOVA /PARAMETERS GROUP_SIZES=12 15 10 13 GROUP_MEANS=20 23 25 21 \
                POOLED_SD=4.5";
    let (t, _, _) = tables(&format!(
        "{base} /CONTRAST COEFFICIENTS=-3 1 1 1 PAIRWISE=TRUE."
    ));
    let main = titled(&t, "Power Analysis");
    close(at(main, 0, "Power"), 0.6210297429, 1e-8);
    close(at(main, 0, "Effect Size"), 0.4932866177, 1e-8);
    assert_eq!(at(main, 0, "N"), 50.0);
    close(at(main, 1, "Power"), 0.5018431608, 1e-8);
    close(at(main, 1, "Effect Size"), 2.0, 1e-12);
    let pairs = titled(&t, "Pairwise Differences");
    assert_eq!(pairs.rows.len(), 6);
    close(at(pairs, 0, "Power"), 0.1632150407, 1e-8);
    close(at(pairs, 1, "Power"), 0.4438666227, 1e-8);
    close(at(pairs, 5, "Power"), 0.2727987973, 1e-8);
    close(at(pairs, 0, "Sig."), 0.05 / 6.0, 1e-12);

    let (t, _, _) = tables(&format!(
        "{base} /CONTRAST COEFFICIENTS=-3 1 1 1 TEST=DIRECTIONAL PAIRWISE=TRUE ADJUST=SIDAK."
    ));
    close(
        at(titled(&t, "Power Analysis"), 1, "Power"),
        0.6300333417,
        1e-8,
    );
    let pairs = titled(&t, "Pairwise Differences");
    close(at(pairs, 1, "Power"), 0.4469266595, 1e-8);
    close(at(pairs, 1, "Sig."), 0.0085124446, 1e-9);

    // coefficients adding up to -2: the last becomes -1, with a warning
    let (t, warnings, _) = tables(&format!("{base} /CONTRAST COEFFICIENTS=1 1 -1 -3."));
    close(
        at(titled(&t, "Power Analysis"), 1, "Power"),
        0.2075702859,
        1e-8,
    );
    assert!(
        warnings.iter().any(|w| w.contains("add up to -2")),
        "{warnings:?}"
    );
}

#[test]
fn oneway_anova_group_sizes() {
    // weights 1 2 1.5 1: the roots 70.49 and 90.39 give the groups below
    let (t, _, charts) = tables(
        "POWER ONEWAY ANOVA /PARAMETERS POWER=0.8 0.9 GROUP_WEIGHTS=1 2 1.5 1 \
         GROUP_MEANS=20 23 25 21 POOLED_SD=4.5 \
         /CONTRAST COEFFICIENTS=-3 1 1 1 TEST=DIRECTIONAL PAIRWISE=TRUE /PLOT TOTAL_N.",
    );
    let main = titled(&t, "Power Analysis");
    assert_eq!(main.rows.len(), 4);
    assert_eq!((at(main, 0, "N"), at(main, 1, "N")), (72.0, 92.0));
    close(at(main, 0, "Actual Power"), 0.8117686106, 1e-8);
    close(at(main, 1, "Actual Power"), 0.9077402190, 1e-8);
    // the one-sided contrast: root 96.34
    assert_eq!(at(main, 2, "N"), 99.0);
    close(at(main, 2, "Actual Power"), 0.8095502337, 1e-8);
    let pairs = titled(&t, "Pairwise Differences");
    assert_eq!(at(pairs, 0, "N"), 231.0);
    close(at(pairs, 0, "Actual Power"), 0.8057468771, 1e-8);
    let sizes = titled(&t, "Group Sizes");
    let groups: Vec<f64> = ["Group 1", "Group 2", "Group 3", "Group 4", "Total"]
        .iter()
        .map(|c| at(sizes, 0, c))
        .collect();
    assert_eq!(groups, [13.0, 26.0, 20.0, 13.0, 72.0]);
    assert_eq!(at(sizes, 4, "Group 2"), 84.0);
    assert_eq!(charts, 1);
}

#[test]
fn pearson_correlation_with_and_without_bias() {
    let base = "POWER PEARSON ONESAMPLE /PARAMETERS NULL=0.1 ALTERNATIVE=0.45";
    let r = run(&format!("{base} N=60."));
    close(at(&r.table, 0, "Power"), 0.8329439505, 1e-8);
    let r = run(&format!("{base} N=60 ADJUST_BIAS=FALSE."));
    close(at(&r.table, 0, "Power"), 0.8268857288, 1e-8);
    let r =
        run("POWER PEARSON ONESAMPLE /PARAMETERS NULL=0.1 ALTERNATIVE=-0.2 N=60 TEST=DIRECTIONAL.");
    close(at(&r.table, 0, "Power"), 0.7462987310, 1e-8);
    // roots 62.83 and 63.77
    let r = run(&format!("{base} POWER=0.85."));
    assert_eq!(at(&r.table, 0, "N"), 63.0);
    close(at(&r.table, 0, "Actual Power"), 0.8509908330, 1e-8);
    let r = run(&format!("{base} POWER=0.85 ADJUST BIAS=FALSE."));
    assert_eq!(at(&r.table, 0, "N"), 64.0);
    close(at(&r.table, 0, "Actual Power"), 0.8512996307, 1e-8);
    // one-sided without the bias term: ((z.90 + z.99) / atanh 0.3)^2 + 3 = 138.87
    let r = run(
        "POWER PEARSON ONESAMPLE /PARAMETERS ALTERNATIVE=0.3 POWER=0.9 SIGNIFICANCE=0.01 \
         TEST=DIRECTIONAL ADJUST_BIAS=FALSE.",
    );
    assert_eq!(at(&r.table, 0, "N"), 139.0);
    // a perfect alternative is found for certain
    let r = run("POWER PEARSON ONESAMPLE /PARAMETERS ALTERNATIVE=1 N=10.");
    assert_eq!(at(&r.table, 0, "Power"), 1.0);
}

#[test]
fn spearman_variances_and_partial_correlation() {
    for (variance, power, n, actual) in [
        ("BW", 0.7980822563, 51.0, 0.8063063870),
        ("FHP", 0.8053900232, 50.0, 0.8053900232),
        ("CC", 0.8128035565, 49.0, 0.8047170880),
    ] {
        let base =
            format!("POWER SPEARMAN ONESAMPLE /PARAMETERS ALTERNATIVE=0.4 VARIANCE={variance}");
        let r = run(&format!("{base} N=50."));
        close(at(&r.table, 0, "Power"), power, 1e-8);
        let r = run(&format!("{base} POWER=0.8."));
        assert_eq!(at(&r.table, 0, "N"), n, "{variance}");
        close(at(&r.table, 0, "Actual Power"), actual, 1e-8);
    }
    // one-sided: (1 + 0.4^2 / 2) ((z.80 + z.95) / atanh 0.4)^2 + 3 = 40.20
    let r = run("POWER SPEARMAN ONESAMPLE /PARAMETERS ALTERNATIVE=0.4 POWER=0.8 TEST=DIRECTIONAL.");
    assert_eq!(at(&r.table, 0, "N"), 41.0);

    let base = "POWER PARTIALCORR /PARAMETERS PARTIALOUT=3 NULL=0.05 ALTERNATIVE=0.38";
    let r = run(&format!(
        "{base} N=70 /PLOT PARTIALOUT N DIFF NULL_XAXIS ALTERNATIVE_YAXIS."
    ));
    close(at(&r.table, 0, "Power"), 0.8054235128, 1e-8);
    assert_eq!(at(&r.table, 0, "Partialled Out"), 3.0);
    assert_eq!(r.charts.len(), 4);
    // root 90.80
    let r = run(&format!("{base} POWER=0.9."));
    assert_eq!(at(&r.table, 0, "N"), 91.0);
    close(at(&r.table, 0, "Actual Power"), 0.9006734929, 1e-8);
}

#[test]
fn linear_regression_fixed_and_random_predictors() {
    // 6 predictors, 2 of them tested, partial correlation 0.3. The random power was checked
    // twice: as the noncentral F power averaged over a chi-square noncentrality, and as the
    // negative binomial mixture of beta distributions.
    let base =
        "POWER UNIVARIATE LINEAR /PARAMETERS TOTAL_PREDICTORS=6 TEST_PREDICTORS=2 PARTIAL_CORR=0.3";
    let r = run(&format!("{base} N=80."));
    close(at(&r.table, 0, "Power"), 0.6933496902, 1e-8);
    let r = run(&format!(
        "{base} N=80 MODEL=RANDOM /PLOT N PARTIAL_CORR N_XAXIS PARTIAL_CORR_YAXIS."
    ));
    close(at(&r.table, 0, "Power"), 0.6570845366, 1e-8);
    assert_eq!(r.charts.len(), 3);
    // roots 100.60 and 107.12
    let r = run(&format!("{base} POWER=0.8."));
    assert_eq!(at(&r.table, 0, "N"), 101.0);
    close(at(&r.table, 0, "Actual Power"), 0.8017070642, 1e-8);
    let r = run(&format!("{base} POWER=0.8 MODEL=RANDOM."));
    assert_eq!(at(&r.table, 0, "N"), 108.0);
    close(at(&r.table, 0, "Actual Power"), 0.8036606746, 1e-8);
    // squared multiple correlations 0.42 and 0.35, no intercept: partial r^2 = 0.07 / 0.65
    let full = "POWER UNIVARIATE LINEAR /PARAMETERS TOTAL_PREDICTORS=6 TEST_PREDICTORS=2 \
                FULL_MODEL=0.42 NESTED_MODEL=0.35 INTERCEPT=FALSE N=80";
    let r = run(&format!("{full}."));
    close(at(&r.table, 0, "Power"), 0.7835358078, 1e-8);
    close(at(&r.table, 0, "Full Model R Square"), 0.42, 1e-12);
    let r = run(&format!("{full} MODEL=RANDOM."));
    close(at(&r.table, 0, "Power"), 0.7525770242, 1e-8);
}
