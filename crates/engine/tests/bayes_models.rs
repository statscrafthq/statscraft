//! BAYES ANOVA, REGRESSION and REPEATED on invented kiln data, and the charts of the BAYES
//! commands. Expected values were computed in a general statistics package run as a black box:
//! numerical integration of the Bayes factor formulas, its least squares fits and t, F, gamma
//! and normal distributions, and an independent implementation of the JZS regression factor
//! and of the mixed-model factor of repeated measures.

use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Chart, Table};

/// Fifteen test tiles from three kilns: the fuel, the firing temperature, the hardness, three
/// repeated scratch scores and a reliability weight.
const KILNS: &str = "DATA LIST FREE /kiln fuel (A4) temp hard t1 t2 t3 rw.
BEGIN DATA
1 gas 610 41.2 12.1 13.0 13.4 1
1 wood 655 44.0 11.4 12.2 13.1 2
1 gas 590 39.5 13.0 13.3 14.2 1
1 wood 640 43.1 10.8 11.9 12.0 0.5
1 gas 620 42.7 12.6 12.9 13.9 1
2 wood 700 47.9 11.9 13.1 13.0 1.5
2 gas 680 46.2 12.4 13.8 14.6 1
2 wood 720 49.4 10.9 11.6 12.8 1
2 gas 665 45.0 13.3 14.1 14.0 2
2 wood 690 47.1 12.0 12.4 13.7 1
3 gas 560 38.8 11.1 12.5 12.2 1
3 wood 600 41.9 12.8 13.6 14.9 0.5
3 gas 575 40.3 11.7 12.0 13.3 1
3 wood 615 42.5 12.2 13.5 13.6 1
3 gas 585 39.9 13.5 13.9 14.4 1
END DATA.
";

struct Run {
    tables: Vec<Table>,
    warnings: Vec<String>,
    charts: Vec<Chart>,
}

fn run(source: &str) -> Run {
    let source = format!("{KILNS}{source}");
    let result = Session::default()
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let command = result.commands.into_iter().last().unwrap();
    Run {
        tables: command.tables,
        warnings: command.warnings,
        charts: command.charts,
    }
}

fn fails(source: &str) -> String {
    let source = format!("{KILNS}{source}");
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
fn anova_factors_of_every_g_prior() {
    for (estbf, expected) in [
        ("", 340.277808081),
        ("/ESTBF COMPUTATION=ZELLNER(2)", 55.3794079835),
        ("/ESTBF COMPUTATION=HYPER", 182.252411796),
        ("/ESTBF COMP=ROUDER(0.5)", 264.891250105),
    ] {
        let r = run(&format!(
            "BAYES ANOVA hard BY kiln /INFERENCE ANALYSIS=BAYESFACTOR {estbf}."
        ));
        assert_eq!(r.tables.len(), 1);
        let t = find(&r.tables, "ANOVA");
        close(at(t, 0, "Bayes Factor"), expected, 1e-6);
        close(
            at(t, 0, "Sum of Squares") / at(t, 2, "Sum of Squares"),
            0.777427678935,
            1e-9,
        );
        assert_eq!(at(t, 1, "df"), 12.0);
    }
    // regression weights enter R square; N stays the number of cases
    let r = run("BAYES ANOVA hard BY kiln /INF ANA=BAYESF /DESIGN REGWGT=rw.");
    let t = &r.tables[0];
    close(at(t, 0, "Bayes Factor"), 191.31362794, 1e-6);
    assert!(t.caption.as_ref().unwrap().contains("0.7535"));
}

#[test]
fn anova_posteriors_and_charts() {
    let r = run("BAYES ANOVA hard BY kiln /PLOT MEANS=2 ERRORVAR=TRUE.");
    let t = find(&r.tables, "Posterior Distribution of the Means of hard");
    posterior(t, 0, [42.1, 42.1, 0.65552, 40.4896425968, 43.7103574032]);
    posterior(t, 2, [40.68, 40.68, 0.65552, 39.0696425968, 42.2903574032]);
    posterior(
        t,
        3,
        [
            2.34114285714,
            3.2776,
            2.68566544,
            1.4044852245,
            7.44268257843,
        ],
    );
    assert_eq!(at(t, 1, "N"), 5.0);
    // a reference prior has no prior curve: likelihood and posterior coincide
    assert_eq!(r.charts.len(), 2);
    let r = run(
        "BAYES ANOVA hard BY kiln /PRIOR TYPE=CONJUGATE SHAPEPARAM=2 SCALEPARAM=3 \
         MEANVECTOR=40 45 40 /PLOT MEANS=1 7.",
    );
    let t = &r.tables[0];
    posterior(
        t,
        0,
        [41.75, 41.75, 0.456683006536, 40.4120825888, 43.0879174112],
    );
    posterior(
        t,
        1,
        [
            46.7666666667,
            46.7666666667,
            0.456683006536,
            45.4287492554,
            48.1045840779,
        ],
    );
    posterior(
        t,
        3,
        [
            2.21817460317,
            2.74009803922,
            1.0010849686,
            1.41791072708,
            5.23006573455,
        ],
    );
    assert_eq!(r.charts.len(), 1);
    assert!(
        r.warnings.iter().any(|w| w.contains("7 is not a group")),
        "{:?}",
        r.warnings
    );
    let json = serde_json::to_string(&r.charts[0]).unwrap();
    for curve in ["Prior", "Likelihood", "Posterior"] {
        assert!(json.contains(curve), "{json}");
    }
    // a VMATRIX that is not positive definite falls back to the identity
    let r = run(
        "BAYES ANOVA hard BY kiln /PRIOR TYPE=CONJUGATE SHAPEPARAM=2 SCALEPARAM=3 \
         MEANVECTOR=40 45 40 VMATRIX=1; 2 1; 0 0 1.",
    );
    posterior(
        &r.tables[0],
        0,
        [41.75, 41.75, 0.456683006536, 40.4120825888, 43.0879174112],
    );
    assert!(r.warnings.iter().any(|w| w.contains("identity")));
    assert!(
        fails("BAYES ANOVA hard BY kiln /PRIOR TYPE=CONJUGATE SHAPEPARAM=2 SCALEPARAM=3 MEANVECTOR=40 45.")
            .contains("3 values")
    );
}

#[test]
fn regression_factor_comparison_and_posterior() {
    let r = run(
        "BAYES REGRESSION hard BY fuel WITH temp /INFERENCE ANALYSIS=BOTH /ESTBF COMPARE=FULL.",
    );
    let t = find(&r.tables, "ANOVA");
    close(at(t, 0, "Bayes Factor"), 42304674.6814, 1e-6);
    assert_eq!(at(t, 0, "df"), 2.0);
    let t = find(&r.tables, "Bayes Factors Against the Full Model");
    // without fuel, without temp, the intercept only
    close(at(t, 0, "R Square"), 0.971290989807, 1e-9);
    close(at(t, 0, "Bayes Factor"), 10.8071781657, 1e-6);
    close(at(t, 1, "R Square"), 0.298015172387, 1e-9);
    close(at(t, 1, "Bayes Factor"), 4.18418897796e-08, 1e-6);
    close(at(t, 2, "Bayes Factor"), 2.36380496371e-08, 1e-6);
    let t = find(&r.tables, "Posterior Distribution of the Parameters");
    // intercept, gas, wood (redundant), temp, error variance
    posterior(
        t,
        0,
        [
            3.4447093596,
            3.4447093596,
            6.9861942164,
            -1.81243167753,
            8.70185039674,
        ],
    );
    posterior(
        t,
        1,
        [
            -0.310176444245,
            -0.310176444245,
            0.144976531645,
            -1.06749422525,
            0.447141336757,
        ],
    );
    assert_eq!((at(t, 2, "Mode"), at(t, 2, "Mean")), (0.0, 0.0));
    assert_eq!(t.rows[2][3], Cell::Empty);
    posterior(
        t,
        3,
        [
            0.0631573667712,
            0.0631573667712,
            1.59080744041e-05,
            0.0552243548364,
            0.0710903787059,
        ],
    );
    posterior(
        t,
        4,
        [
            0.28318502985,
            0.39645904179,
            0.0392949429542,
            0.169886766632,
            0.900268124051,
        ],
    );
    for (estbf, expected) in [
        ("COMPUTATION=ZELLNER(2)", 504.999579324),
        ("COMPUTATION=HYPER(3)", 14460841.6511),
        ("COMPUTATION=ROUDER(0.5)", 22261678.0273),
    ] {
        let r = run(&format!(
            "BAYES REGRESSION hard BY fuel WITH temp /INFERENCE ANALYSIS=BAYESFACTOR /ESTBF {estbf}."
        ));
        close(at(&r.tables[0], 0, "Bayes Factor"), expected, 1e-6);
    }
}

#[test]
fn regression_conjugate_prediction_tests_and_save() {
    let r = run(
        "BAYES REGRESSION hard BY fuel WITH temp /PRIOR TYPE=CONJUGATE SHAPEPARAM=2 \
         SCALEPARAM=3 MEANVECTOR=0 0 0 0.",
    );
    let t = &r.tables[0];
    posterior(
        t,
        0,
        [
            0.18247985513,
            0.18247985513,
            0.587458244374,
            -1.33495613842,
            1.69991584868,
        ],
    );
    posterior(
        t,
        3,
        [
            0.0680534081909,
            0.0680534081909,
            1.5333326509e-06,
            0.0656018620498,
            0.070504954332,
        ],
    );
    posterior(
        t,
        4,
        [
            0.505135390458,
            0.623990776448,
            0.0519152652123,
            0.322894729627,
            1.19102044229,
        ],
    );

    let source = "BAYES REGRESSION hard BY fuel WITH temp /PREDICT REGRESSORS=gas 650 \
         /FTEST VARIABLES=temp /FTEST LABEL='level' INTERCEPT=TRUE VARIABLES=fuel TESTVALUES=30 2 0 \
         /SAVE MEANPRED VARIANCE(predvar) /PLOT COVARIATES=temp FACTORS=fuel MAX CATEGORY=1 \
         INTERCEPT=TRUE ERRORVAR=TRUE BAYESPRED=TRUE.\n\
         DESCRIPTIVES MEANPRED_1 predvar.";
    let full = format!("{KILNS}{source}");
    let result = Session::default().run(&full, None).unwrap();
    let bayes = &result.commands[result.commands.len() - 2];
    let t = find(&bayes.tables, "Predictive Distribution");
    posterior(
        t,
        0,
        [
            44.1868213166,
            44.1868213166,
            0.470680151431,
            42.8222625524,
            45.5513800808,
        ],
    );
    let t = find(&bayes.tables, "Bayesian F Tests");
    close(at(t, 0, "F"), 300.892707148, 1e-8);
    close(at(t, 0, "Sig."), 7.29943105959e-10, 1e-5);
    assert_eq!(t.rows[1][0], Cell::Text("level".into()));
    close(at(t, 1, "F"), 184.772377888, 1e-8);
    assert_eq!((at(t, 1, "df1"), at(t, 1, "df2")), (2.0, 12.0));
    // intercept, temp, fuel = gas, error variance, prediction
    assert_eq!(bayes.charts.len(), 5);
    assert_eq!(result.commands.last().unwrap().tables.len(), 1);

    let r = run(
        "BAYES REGRESSION hard BY fuel WITH temp /SAVE MEANPRED VARIANCE.\nLIST MEANPRED_1 VARIANCE_1 /CASES=FROM 1 TO 1.",
    );
    // LIST shows the values with five decimals (F11.5)
    assert_eq!(
        r.tables[0].rows[0][..],
        [Cell::Text("41.66053".into()), Cell::Text(".44602".into())]
    );

    assert!(
        fails("BAYES REGRESSION hard BY fuel WITH temp /PREDICT REGRESSORS=coal 650.")
            .contains("not a category")
    );
    assert!(
        fails("BAYES REGRESSION hard BY fuel WITH temp /FTEST VARIABLES=kiln.")
            .contains("not in the model")
    );
    assert!(
        fails("BAYES REGRESSION hard BY fuel WITH temp /DESIGN COVARIATES=kiln.")
            .contains("not among")
    );
}

#[test]
fn repeated_bic_rouder_and_posterior() {
    let r = run("BAYES REPEATED t1 t2 t3 /INFERENCE ANALYSIS=BOTH /PLOT MEANS=t2.");
    let t = find(&r.tables, "Bayes Factor for Repeated Measures");
    close(at(t, 0, "Bayes Factor"), 38105097709.4, 1e-8);
    close(at(t, 0, "F"), 68.1417500352, 1e-9);
    close(at(t, 0, "Sig."), 1.74540382147e-11, 1e-5);
    let t = find(&r.tables, "Posterior Distribution of the Means");
    posterior(
        t,
        1,
        [12.92, 12.92, 0.0420404938272, 12.5181333233, 13.3218666767],
    );
    assert_eq!(r.charts.len(), 1);

    // Monte Carlo against a two-dimensional integration of the same factor
    let r = run(
        "BAYES REPEATED t1 t2 t3 /INFERENCE ANALYSIS=BAYESFACTOR /ESTBF COMPUTATION=ROUDER \
         /CRITERIA SEED=4242 MCSAMPLES=200000.",
    );
    close(at(&r.tables[0], 0, "Bayes Factor"), 431897783.227, 0.03);
    let r = run(
        "BAYES REPEATED t1 t2 t3 /INF ANA=BAYESF /ESTBF COMP=ROUDER /DESIGN REGWGT=rw \
         /CRITERIA SEED=99 MCSAMPLES=200000.",
    );
    close(at(&r.tables[0], 0, "Bayes Factor"), 6567142762.04, 0.03);
    let r = run("BAYES REPEATED t1 t2 t3 /INF ANA=BAYESF /DESIGN REGWGT=rw.");
    close(at(&r.tables[0], 0, "Bayes Factor"), 224168782517.0, 1e-8);
}

#[test]
fn correlation_charts_follow_maxplots() {
    let r = run("BAYES CORRELATION /INFERENCE VARIABLES=temp hard t1 MAXPLOTS=2.");
    assert_eq!(r.charts.len(), 2);
    let json = serde_json::to_value(&r.charts[0]).unwrap();
    let text = json.to_string();
    assert!(
        text.contains("Prior") && text.contains("Posterior"),
        "{text}"
    );
    let r = run("BAYES CORRELATION /INFERENCE VARIABLES=temp hard t1 ANALYSIS=BAYESFACTOR.");
    assert!(r.charts.is_empty());
}
