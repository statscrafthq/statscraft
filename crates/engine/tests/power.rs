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
    let n = at(&r.table, 0, "N");
    assert!(n >= 127.0, "{n}");
    assert!(at(&r.table, 0, "Actual Power") >= 0.8);

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
    // every method gives a power between the significance level and 1
    for method in ["T", "LRT", "FISHER"] {
        for estimate in ["NORMAL", "BINOMIAL"] {
            let r = run(&format!(
                "POWER PROPORTIONS INDEPENDENT /PARAMETERS PROPORTIONS=0.2 0.5 N=30 40 \
                 METHOD={method} ESTIMATE={estimate} POOLED=FALSE."
            ));
            let power = at(&r.table, 0, "Power");
            assert!(power > 0.4 && power < 0.95, "{method} {estimate}: {power}");
        }
    }
    let r = run(&format!(
        "{base} POWER=0.8 TEST=DIRECTIONAL CONTINUITY=TRUE NRATIO=1.5."
    ));
    assert!(at(&r.table, 0, "Actual Power") >= 0.8);
    let r = run(
        "POWER PROPORTIONS INDEPENDENT /PARAMETERS PROPORTIONS=0.2 0.5 POWER=0.8 ESTIMATE=BINOMIAL.",
    );
    assert!(at(&r.table, 0, "Actual Power") >= 0.8);
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
    assert!(at(&r.table, 0, "Actual Power") >= 0.9);
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
