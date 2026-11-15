//! BAYES ONESAMPLE, RELATED, INDEPENDENT and CORRELATION on invented seedling data. Expected
//! values were computed in a general statistics package run as a black box: its t, normal,
//! beta and gamma distributions, numerical integration of the formulas, and an independent
//! implementation of the JZS Bayes factors of the t-test and of regression.

use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table};

/// Fourteen seedlings under two light settings: growth in cm, leaf count, whether they
/// flowered, and the height of a stem before and after pruning.
const SEEDLINGS: &str = "DATA LIST FREE /light growth leaves bloom before after.
BEGIN DATA
1 4.2 3 0 10.1 9.4
1 5.1 4 1 11.3 10.2
1 3.8 2 0 9.8 9.9
1 4.9 5 1 12.0 10.8
1 5.6 3 1 10.5 9.7
1 4.4 4 0 11.1 10.6
1 3.9 2 0 9.6 9.1
2 6.1 6 1 10.9 9.8
2 5.8 5 1 12.4 11.5
2 6.7 7 1 11.0 10.1
2 5.2 4 0 10.2 9.9
2 6.3 6 1 11.7 10.4
2 7.0 8 1 12.2 11.0
2 5.9 5 0 10.6 10.2
END DATA.
";

fn run(source: &str) -> (Vec<Table>, Vec<String>) {
    let source = format!("{SEEDLINGS}{source}");
    let result = Session::default()
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let command = result.commands.into_iter().last().unwrap();
    (command.tables, command.warnings)
}

fn fails(source: &str) -> String {
    let source = format!("{SEEDLINGS}{source}");
    let errors = Session::default().run(&source, None).unwrap_err();
    errors
        .iter()
        .map(|e| e.text())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn find<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables.iter().find(|t| t.title == title).unwrap_or_else(|| {
        panic!(
            "no table {title}: {:?}",
            tables.iter().map(|t| &t.title).collect::<Vec<_>>()
        )
    })
}

/// The number in `row` under the column whose full title is `column`.
fn at(table: &Table, row: usize, column: &str) -> f64 {
    let i = table
        .column_titles()
        .iter()
        .position(|c| c == column)
        .unwrap_or_else(|| panic!("no column {column} in {:?}", table.column_titles()));
    match &table.rows[row][i] {
        Cell::Number(n) => *n,
        other => panic!("{other:?} is no number"),
    }
}

fn close(got: f64, expected: f64, tolerance: f64) {
    assert!(
        (got - expected).abs() <= tolerance * expected.abs().max(1e-300),
        "{got} != {expected}"
    );
}

/// Mode, mean, variance and the interval of a posterior row.
fn posterior(table: &Table, row: usize, expected: [f64; 5]) {
    let ci = table.column_groups.last().unwrap()[0].clone();
    let columns = [
        "Mode".to_string(),
        "Mean".into(),
        "Variance".into(),
        format!("{ci} / Lower"),
        format!("{ci} / Upper"),
    ];
    for (column, value) in columns.iter().zip(expected) {
        close(at(table, row, column), value, 1e-7);
    }
}

#[test]
fn one_sample_normal_bayes_factors() {
    let (tables, _) = run(
        "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=growth growth ANALYSIS=BAYESFACTOR\n\
         /DATA VARIABLES=growth NULLVALUE=5.",
    );
    let t = find(&tables, "Bayes Factor for a One-Sample Mean");
    close(at(t, 0, "t"), 1.2806018721, 1e-9);
    close(at(t, 0, "Bayes Factor"), 2.38001970605, 1e-6);
    assert_eq!(at(t, 0, "Test Value"), 5.0);
    assert_eq!(tables.len(), 1);
    let (tables, _) = run(
        "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=growth ANALYSIS=BAYESFACTOR\n\
         /DATA VARIABLES=growth NULLVALUE=5 ALTERGVALUE=0.5.",
    );
    close(at(&tables[0], 0, "Bayes Factor"), 1.37384210173, 1e-9);
    let (tables, _) = run(
        "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=growth ANALYSIS=BAYESFACTOR\n\
         /DATA VARIABLES=growth NULLVALUE=5 VARIANCE=1.2.",
    );
    close(at(&tables[0], 0, "Bayes Factor"), 1.9879036795, 1e-9);
}

#[test]
fn monte_carlo_factor_is_close_and_repeatable() {
    let source = "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=growth ANALYSIS=BAYESFACTOR\n\
         /DATA VARIABLES=growth NULLVALUE=5 /CRITERIA METHOD=MCA SEED=4242 MCSAMPLES=200000.";
    let first = at(&run(source).0[0], 0, "Bayes Factor");
    close(first, 2.38001970605, 0.01);
    assert_eq!(first, at(&run(source).0[0], 0, "Bayes Factor"));
}

#[test]
fn one_sample_normal_posteriors() {
    let cases = [
        (
            "",
            [5.35, 5.35, 0.107896825397, 4.69604726579, 6.00395273421],
        ),
        (
            "/PRIOR VARDIST=JEFFREYS2",
            [5.35, 5.35, 0.0882792207792, 4.75955183166, 5.94044816834],
        ),
        (
            "/PRIOR VARDIST=JEFFREYS4",
            [5.35, 5.35, 0.0746978021978, 4.8076807562, 5.8923192438],
        ),
        (
            "/PRIOR VARDIST=INVGAMMA(3 2) MEANDIST=NORMAL(5) KAPPA=2",
            [
                5.30625,
                5.30625,
                0.0618381076389,
                4.81414696434,
                5.79835303566,
            ],
        ),
        // the same prior as precision Gamma(3, 2) and as inverse chi-square(6, 2/3)
        (
            "/PRIOR PRECDIST=GAMMA(3 2) MEANDIST=NORMAL(5 9) KAPPA=2",
            [
                5.30625,
                5.30625,
                0.0618381076389,
                4.81414696434,
                5.79835303566,
            ],
        ),
        (
            "/PRIOR VARDIST=INVCHISQ(6 0.6666666666666666) MEANDIST=NORMAL(5) KAPPA=2",
            [
                5.30625,
                5.30625,
                0.0618381076389,
                4.81414696434,
                5.79835303566,
            ],
        ),
        // a conjugate variance prior without a normal mean prior falls back to the flat priors
        (
            "/PRIOR VARDIST=INVGAMMA(3 2)",
            [5.35, 5.35, 0.107896825397, 4.69604726579, 6.00395273421],
        ),
        (
            "/DATA VARIABLES=growth VARIANCE=1.2 /PRIOR MEANDIST=NORMAL(5 0.5)",
            [
                5.2987804878,
                5.2987804878,
                0.0731707317073,
                4.76860848483,
                5.82895249078,
            ],
        ),
    ];
    for (extra, expected) in cases {
        let (tables, _) = run(&format!(
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=growth {extra}."
        ));
        let t = find(&tables, "Posterior Distribution of the Mean");
        assert_eq!(at(t, 0, "N"), 14.0);
        posterior(t, 0, expected);
    }
}

#[test]
fn binomial_and_poisson() {
    let (tables, _) = run(
        "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=BINOMIAL VARIABLES=bloom growth ANALYSIS=BOTH\n\
         /DATA VARIABLES=bloom NULLPRIORPARAM=2 2 ALTERPRIORPARAM=1 1\n\
         /DATA VARIABLES=growth SUCCESS=CUTPOINT(5.5) ALTERPRIORPARAM=1 1\n\
         /PRIOR TYPE=CONJUGATE(1 1).",
    );
    let t = find(&tables, "Bayes Factor for a Binomial Proportion");
    assert_eq!((at(t, 0, "N"), at(t, 0, "Successes")), (14.0, 8.0));
    close(at(t, 0, "Bayes Factor"), 1.38970588235, 1e-10);
    assert_eq!(at(t, 1, "Successes"), 7.0);
    posterior(
        find(&tables, "Posterior Distribution of the Proportion"),
        0,
        [
            0.571428571429,
            0.5625,
            0.0144761029412,
            0.322869766206,
            0.787333270481,
        ],
    );
    let (tables, _) = run(
        "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=POISSON VARIABLES=leaves ANALYSIS=BOTH\n\
         /DATA VARIABLES=leaves ALTERPRIORPARAM=1 0.5 /PRIOR TYPE=CONJUGATE(0.5 0).",
    );
    let t = find(&tables, "Bayes Factor for a Poisson Rate");
    assert_eq!(at(t, 0, "Total"), 64.0);
    close(at(t, 0, "Bayes Factor"), 0.0540760092445, 1e-9);
    posterior(
        find(&tables, "Posterior Distribution of the Rate"),
        0,
        [
            4.53571428571,
            4.60714285714,
            0.329081632653,
            3.55189960619,
            5.797542002,
        ],
    );
    let (tables, warnings) = run(
        "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=POISSON VARIABLES=growth\n\
         /PRIOR TYPE=CONJUGATE(1 1).",
    );
    assert!(
        tables[0].rows.is_empty() && warnings[0].contains("not counts"),
        "{warnings:?}"
    );
}

#[test]
fn weights_are_rounded_frequencies() {
    // doubling every case through a weight of 2.4 is the same as listing it twice
    let (weighted, _) = run("COMPUTE w = 2.4.\nWEIGHT BY w.\n\
         BAYES ONESAMPLE /INFERENCE DISTRIBUTION=BINOMIAL VARIABLES=bloom ANALYSIS=BAYESFACTOR\n\
         /DATA VARIABLES=bloom ALTERPRIORPARAM=1 1.");
    let t = &weighted[0];
    assert_eq!((at(t, 0, "N"), at(t, 0, "Successes")), (28.0, 16.0));
    let (dropped, _) = run("COMPUTE w = 0.4 + (light = 2).\nWEIGHT BY w.\n\
         BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=growth.");
    assert_eq!(at(&dropped[0], 0, "N"), 7.0);
}

#[test]
fn related_samples() {
    let (tables, _) = run("BAYES RELATED /PAIR VARIABLES=before after NULLVALUE=0.5\n\
         /INFERENCE ANALYSIS=BOTH /PRIOR VARDIST=JEFFREYS2.");
    let t = find(&tables, "Bayes Factor for Related Samples");
    close(at(t, 0, "Mean Difference"), 0.771428571429, 1e-11);
    close(at(t, 0, "t"), 2.48659118169, 1e-10);
    close(at(t, 0, "Bayes Factor"), 0.446767955361, 1e-6);
    assert!(matches!(&t.rows[0][0], Cell::Text(s) if s.contains("before") && s.contains("after")));
    posterior(
        find(&tables, "Posterior Distribution of the Mean Difference"),
        0,
        [
            0.771428571429,
            0.771428571429,
            0.0140816326531,
            0.535609437789,
            1.00724770507,
        ],
    );
}

#[test]
fn independent_bayes_factors() {
    let base = "BAYES INDEPENDENT /INFERENCE VARIABLES=growth GROUP=light SELECT=LEVEL(1 2) ANALYSIS=BAYESFACTOR";
    let factor = |estbf: &str| {
        let (tables, _) = run(&format!("{base} {estbf}."));
        let t = find(&tables, "Bayes Factor for Independent Samples");
        close(at(t, 0, "Mean Difference"), 1.58571428571, 1e-11);
        close(at(t, 0, "t"), 4.69340063655, 1e-10);
        close(at(t, 0, "Sig. (2-tailed)"), 0.000520161494157, 1e-8);
        at(t, 0, "Bayes Factor")
    };
    close(factor(""), 0.0166760717025, 1e-6);
    close(
        factor("/ESTBF COMPUTATION=ROUDER(0.5)"),
        0.0269535612287,
        1e-6,
    );
    close(
        factor("/ESTBF COMPUTATION=GONEN(0.4 2)"),
        0.00852423851717,
        1e-7,
    );
    close(factor("/ESTBF COMPUTATION=GONEN(2)"), 0.0123715914648, 1e-9);
    close(factor("/ESTBF COMPUTATION=HYPER"), 56.4459106799, 1e-9);
    // a cut point at 1.5 forms the same groups
    let (tables, _) =
        run("BAYES INDEPENDENT /INFERENCE VARIABLES=growth GROUP=light SELECT=CUTPOINT(1.5).");
    let stats = find(&tables, "Group Statistics");
    assert_eq!((at(stats, 0, "N"), at(stats, 1, "N")), (7.0, 7.0));
}

#[test]
fn independent_posteriors() {
    let base = "BAYES INDEPENDENT /INFERENCE VARIABLES=growth GROUP=light SELECT=MIDPOINT";
    for (extra, expected) in [
        (
            "/PRIOR EQUALDATAVAR=TRUE VARDIST=JEFFREYS",
            [
                1.58571428571,
                1.58571428571,
                0.136979591837,
                0.849579679392,
                2.32184889204,
            ],
        ),
        (
            "/PRIOR EQUALDATAVAR=TRUE VARDIST=INVCHISQ(4 0.5)",
            [
                1.58571428571,
                1.58571428571,
                0.138658892128,
                0.847309759813,
                2.32411881162,
            ],
        ),
        (
            "",
            [
                1.58571428571,
                1.58571428571,
                0.171224489796,
                0.759310690722,
                2.41211788071,
            ],
        ),
        (
            "DATAVAR=0.5 0.8",
            [
                1.58571428571,
                1.58571428571,
                0.185714285714,
                0.741076031202,
                2.43035254023,
            ],
        ),
        (
            "DATAVAR=0.5 0.8 /PRIOR MEANDIST=NORMAL(4 6 2 3)",
            [
                1.59968364442,
                1.59968364442,
                0.179057260361,
                0.770321785984,
                2.42904550285,
            ],
        ),
    ] {
        let (tables, _) = run(&format!("{base} {extra}."));
        posterior(
            find(&tables, "Posterior Distribution of the Mean Difference"),
            0,
            expected,
        );
    }
}

#[test]
fn correlation_factors_and_posteriors() {
    let (tables, _) =
        run("BAYES CORRELATION /INFERENCE VARIABLES=growth leaves before ANALYSIS=BOTH.");
    let t = find(&tables, "Bayes Factor for Pearson Correlations");
    assert_eq!(t.rows.len(), 3);
    close(at(t, 0, "Pearson Correlation"), 0.905938830819, 1e-11);
    close(at(t, 0, "Bayes Factor"), 0.000303007826939, 1e-6);
    close(at(t, 1, "Bayes Factor"), 0.317572432927, 1e-6);
    let p = find(&tables, "Posterior Distribution of the Correlation");
    for (column, value) in [
        ("Mode", 0.899272928794),
        ("Mean", 0.848161203065),
        ("Variance", 0.00729462228518),
    ] {
        close(at(p, 0, column), value, 1e-6);
    }
    let ci = p.column_groups.last().unwrap()[0].clone();
    close(at(p, 0, &format!("{ci} / Lower")), 0.679427901316, 1e-5);
    close(at(p, 0, &format!("{ci} / Upper")), 0.970541420375, 1e-5);

    let (tables, _) = run(
        "BAYES CORRELATION /INFERENCE VARIABLES=growth leaves ANALYSIS=BAYESFACTOR\n\
         /ESTBF TYPE=FRACTION /CRITERIA TOL=0.00000001.",
    );
    close(at(&tables[0], 0, "Bayes Factor"), 0.00382673658619, 1e-6);
    let (tables, _) = run(
        "BAYES CORRELATION /INFERENCE VARIABLES=growth before ANALYSIS=BOTH CVALUE=-1\n\
         /ESTBF TYPE=FRACTION(0.3) NULLVALUE=0.2 /CRITERIA TOL=0.00000001.",
    );
    close(at(&tables[0], 0, "Bayes Factor"), 0.627159400691, 1e-6);
    let p = &tables[1];
    for (column, value) in [
        ("Mode", 0.664212700789),
        ("Mean", 0.571805593754),
        ("Variance", 0.0358529418498),
    ] {
        close(at(p, 0, column), value, 1e-6);
    }
}

#[test]
fn errors_and_warnings() {
    assert!(
        fails("BAYES ONESAMPLE /INFERENCE DISTRIBUTION=BINOMIAL VARIABLES=bloom.")
            .contains("TYPE=CONJUGATE")
    );
    assert!(
        fails(
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=BINOMIAL VARIABLES=bloom ANALYSIS=BAYESFACTOR."
        )
        .contains("ALTERPRIORPARAM")
    );
    assert!(
        fails("BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=growth /DATA VARIABLES=leaves.")
            .contains("not on /INFERENCE")
    );
    assert!(
        fails("BAYES RELATED /PAIR VARIABLES=before after VARIANCE=1 /PRIOR MEANDIST=NORMAL(0).")
            .contains("both the mean and the variance")
    );
    let (_, warnings) =
        run("BAYES INDEPENDENT /INFERENCE VARIABLES=growth GROUP=light SELECT=LEVEL(1 3).");
    assert!(warnings[0].contains("no valid cases"), "{warnings:?}");
}
