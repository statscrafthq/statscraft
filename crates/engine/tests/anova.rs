//! ANOVA on synthetic data. The expected numbers come from an independent computation from the
//! raw values: explicit design matrices (sum-to-zero coding for the regression approach, dummy
//! coding for the others), weighted least squares in a general numerical library, and each sum
//! of squares as the difference of the error sums of squares of the two models the manual's
//! tables name; MCA deviations from the dummy-coded main-effects fits, centered.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// a in 1 to 3 and b in 1 to 2, unbalanced; case 25 has a = 4, case 26 misses y.
const DATA: &str = "DATA LIST FREE /id a b c y y2 w.
BEGIN DATA.
1 1 1 45.0 14.4 5.4 1
2 1 1 29.0 12.4 4.7 1
3 1 1 20.2 13.5 5.2 2
4 1 1 38.7 14.1 6.1 1
5 1 2 30.2 10.5 6.4 1
6 1 2 42.1 11.0 5.9 2
7 1 2 44.9 11.3 4.6 1
8 2 1 26.4 13.3 6.7 1
9 2 1 21.4 14.0 3.9 2
10 2 1 56.7 17.0 6.5 1
11 2 1 39.9 15.2 5.4 1
12 2 1 47.7 17.4 5.6 2
13 2 2 20.1 14.7 7.2 1
14 2 2 30.7 14.8 7.9 1
15 2 2 53.9 16.6 9.2 2
16 2 2 23.7 15.0 7.9 1
17 3 1 54.9 20.9 7.9 1
18 3 1 22.4 16.4 7.7 2
19 3 1 26.0 17.3 7.0 1
20 3 2 59.1 19.2 8.5 1
21 3 2 45.5 16.2 9.3 2
22 3 2 37.6 15.4 9.0 1
23 3 2 23.9 16.1 7.9 1
24 3 2 46.9 17.4 7.2 2
25 4 1 30.0 15.0 7.0 1
26 2 2 35.0 . 6.0 1
END DATA.
";

fn run_on(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
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

fn run(syntax: &str) -> Vec<Table> {
    run_on(&mut Engine::default(), &format!("{DATA}{syntax}"))
}

fn label(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => n.to_string(),
        Cell::Empty => String::new(),
    }
}

/// The numbers of the row: `group` is the first row header, `name` the second ("" for none).
/// Under a group, a named row is looked up after the group's first row.
fn source(table: &Table, group: &str, name: &str) -> Vec<f64> {
    let start = table
        .rows
        .iter()
        .position(|r| label(&r[0]) == group)
        .unwrap_or_else(|| panic!("{}: no group {group}", table.title));
    let r = table.rows[start..]
        .iter()
        .find(|r| label(&r[1]) == name)
        .unwrap_or_else(|| panic!("{}: no row {group} {name}", table.title));
    r[2..]
        .iter()
        .map(|c| match c {
            Cell::Number(n) => *n,
            _ => f64::NAN,
        })
        .collect()
}

/// Sum of squares, df, F and Sig.
fn check(table: &Table, group: &str, name: &str, expected: [f64; 4]) {
    let r = source(table, group, name);
    let ours = [r[0], r[1], r[3], r[4]];
    for (a, e) in ours.iter().zip(expected) {
        assert!(
            (a - e).abs() <= 1e-8 * e.abs().max(1.0),
            "{group} {name}: ours {ours:?}, expected {expected:?}"
        );
    }
}

fn anova_tables(tables: &[Table]) -> Vec<&Table> {
    tables.iter().filter(|t| t.title == "ANOVA").collect()
}

#[test]
fn the_regression_approach_adjusts_every_effect_for_all_others() {
    let tables = run("ANOVA VARIABLES=y y2 BY a(1,3) b(1,2) WITH c /STATISTICS=REG.");
    let t = anova_tables(&tables);
    assert_eq!(t.len(), 2);
    let y = t[0];
    check(
        y,
        "Covariates",
        "(Combined)",
        [
            27.910919302945707,
            1.0,
            45.785419521213775,
            3.2943423852312407e-06,
        ],
    );
    check(
        y,
        "Main Effects",
        "(Combined)",
        [
            98.32943227073504,
            3.0,
            53.766941639494654,
            6.922286342845108e-09,
        ],
    );
    check(
        y,
        "Main Effects",
        "a",
        [
            92.22323279806108,
            2.0,
            75.64206964003779,
            3.447059281760241e-09,
        ],
    );
    check(
        y,
        "Main Effects",
        "b",
        [
            14.63298888551498,
            1.0,
            24.004137151507315,
            0.00013532109887463712,
        ],
    );
    check(
        y,
        "2-Way Interactions",
        "a * b",
        [
            13.408640712002802,
            2.0,
            10.997850582146214,
            0.0008613379772053237,
        ],
    );
    let model = source(y, "Model", "");
    assert!((model[0] - 133.70633596961233).abs() < 1e-8 && model[1] == 6.0);
    let residual = source(y, "Residual", "");
    assert!((residual[0] - 10.363247363720964).abs() < 1e-8 && residual[1] == 17.0);
    let total = source(y, "Total", "");
    assert!((total[0] - 144.0695833333333).abs() < 1e-8 && total[1] == 23.0);
    // the coefficient of the covariate in the full model
    assert!((source(y, "Covariates", "c")[5] - 0.09148261626293545).abs() < 1e-10);
    // the second dependent, on the same cases (case 26 misses y)
    check(
        t[1],
        "Main Effects",
        "a",
        [
            20.709790633538482,
            2.0,
            15.857516143138794,
            0.00012992130571902856,
        ],
    );
    check(
        t[1],
        "Covariates",
        "c",
        [
            1.9884005771498963,
            1.0,
            3.045042299956227,
            0.09903256275779973,
        ],
    );
}

#[test]
fn the_experimental_approach_and_the_order_of_covariates() {
    let t = run("ANOVA y BY a(1,3) b(1,2) WITH c /METHOD=EXPERIMENTAL /STATISTICS=REG.");
    let y = anova_tables(&t)[0];
    check(
        y,
        "Covariates",
        "(Combined)",
        [
            28.81348555463451,
            1.0,
            47.26600043761877,
            2.692301087253124e-06,
        ],
    );
    check(
        y,
        "Main Effects",
        "(Combined)",
        [
            91.48420970297502,
            3.0,
            50.0239455313025,
            1.1995247580923624e-08,
        ],
    );
    check(
        y,
        "Main Effects",
        "a",
        [
            86.94918947227544,
            2.0,
            71.31626647276862,
            5.398634066991792e-09,
        ],
    );
    check(
        y,
        "Main Effects",
        "b",
        [
            11.438427177548157,
            1.0,
            18.76373835281101,
            0.00045299035668421354,
        ],
    );
    check(
        y,
        "2-Way Interactions",
        "(Combined)",
        [
            13.40864071200281,
            2.0,
            10.99785058214622,
            0.0008613379772053209,
        ],
    );
    assert!((source(y, "Covariates", "c")[5] - 0.08839633609543517).abs() < 1e-10);

    let t = run(
        "ANOVA y BY a(1,3) b(1,2) WITH c /METHOD=EXPERIMENTAL /COVARIATES=AFTER /STATISTICS=REG.",
    );
    let y = anova_tables(&t)[0];
    check(
        y,
        "Covariates",
        "(Combined)",
        [
            20.87085006015878,
            1.0,
            34.23680228504218,
            1.9252736784095914e-05,
        ],
    );
    check(
        y,
        "Main Effects",
        "(Combined)",
        [
            99.42684519745076,
            3.0,
            54.367011582161375,
            6.3586912568937514e-09,
        ],
    );
    check(
        y,
        "Main Effects",
        "a",
        [
            96.9564285307841,
            2.0,
            79.52426624464533,
            2.349310141093578e-09,
        ],
    );
    assert!((source(y, "Covariates", "c")[5] - 0.07629571269222879).abs() < 1e-10);

    let t = run("ANOVA y BY a(1,3) b(1,2) WITH c /METHOD=EXPERIMENTAL /COVARIATES=WITH.");
    let y = anova_tables(&t)[0];
    // with WITH both combined rows are the covariates and main effects together
    check(
        y,
        "Covariates",
        "(Combined)",
        [
            120.29769525760955,
            4.0,
            49.334459257881576,
            3.892523559072913e-09,
        ],
    );
    check(
        y,
        "Main Effects",
        "(Combined)",
        [
            120.29769525760955,
            4.0,
            49.334459257881576,
            3.892523559072913e-09,
        ],
    );
    check(
        y,
        "Covariates",
        "c",
        [
            20.87085006015878,
            1.0,
            34.23680228504218,
            1.9252736784095914e-05,
        ],
    );
    check(
        y,
        "Main Effects",
        "a",
        [
            86.94918947227544,
            2.0,
            71.31626647276862,
            5.398634066991792e-09,
        ],
    );
}

#[test]
fn the_hierarchical_approach_follows_the_order_of_the_factors() {
    let t = run("ANOVA y BY a(1,3) b(1,2) WITH c /METHOD=HIERARCHICAL.");
    let y = anova_tables(&t)[0];
    check(
        y,
        "Main Effects",
        "a",
        [
            80.04578252542687,
            2.0,
            65.65404912054825,
            1.009054145593871e-08,
        ],
    );
    check(
        y,
        "Main Effects",
        "b",
        [
            11.438427177548157,
            1.0,
            18.76373835281101,
            0.00045299035668421354,
        ],
    );
    let t = run("ANOVA y BY a(1,3) b(1,2) WITH c /METHOD=HIER /COVARIATES=AFTER.");
    let y = anova_tables(&t)[0];
    check(
        y,
        "Main Effects",
        "a",
        [
            90.21369047619046,
            2.0,
            73.99383051803277,
            4.078292519240781e-09,
        ],
    );
    check(
        y,
        "Main Effects",
        "b",
        [
            9.213154721260302,
            1.0,
            15.113373710418585,
            0.0011831187641265645,
        ],
    );
    check(
        y,
        "Covariates",
        "c",
        [
            20.87085006015878,
            1.0,
            34.23680228504218,
            1.9252736784095914e-05,
        ],
    );
}

#[test]
fn weights_and_maxorders() {
    let t = run("WEIGHT BY w.\nANOVA y BY a(1,3) b(1,2) WITH c.");
    let y = anova_tables(&t)[0];
    check(
        y,
        "Main Effects",
        "a",
        [
            111.7541616236652,
            2.0,
            99.85326421418691,
            1.1996484043265742e-12,
        ],
    );
    let residual = source(y, "Residual", "");
    assert!((residual[0] - 13.989798243344186).abs() < 1e-8 && residual[1] == 25.0);
    assert_eq!(source(y, "Total", "")[1], 31.0);
    // MAXORDERS=NONE pools the interaction into the residual
    let t = run("ANOVA y BY a(1,3) b(1,2) /MAXORDERS=NONE /METHOD=EXPERIMENTAL.");
    let y = anova_tables(&t)[0];
    assert!(!y.rows.iter().any(|r| label(&r[0]).contains("Interactions")));
    assert_eq!(source(y, "Model", "")[1], 3.0);
    assert_eq!(source(y, "Residual", "")[1], 20.0);
}

#[test]
fn mca_cell_means_and_goodness_of_fit() {
    let t = run("ANOVA y BY a(1,3) b(1,2) WITH c /METHOD=HIERARCHICAL /STATISTICS=ALL.");
    let mca = t.iter().find(|t| t.title == "MCA").unwrap();
    // a = 1: N, predicted means (unadjusted, factors, factors and covariates), deviations
    let grand = 15.170833333333334;
    let row: Vec<f64> = mca.rows[0][2..]
        .iter()
        .map(|c| match c {
            Cell::Number(n) => *n,
            _ => f64::NAN,
        })
        .collect();
    let expected = [
        7.0,
        grand - 2.713690476190477,
        grand - 2.803626095368159,
        grand - 2.720698355926109,
        -2.713690476190477,
        -2.803626095368159,
        -2.720698355926109,
    ];
    for (a, e) in row.iter().zip(expected) {
        assert!((a - e).abs() < 1e-9, "MCA a = 1: {row:?}");
    }
    let summary = t.iter().find(|t| t.title == "Factor Summary").unwrap();
    let b: Vec<f64> = summary.rows[1][1..]
        .iter()
        .map(|c| match c {
            Cell::Number(n) => *n,
            _ => f64::NAN,
        })
        .collect();
    for (a, e) in b
        .iter()
        .zip([0.13094802548345175, 0.25695036549706873, 0.2871777107341548])
    {
        assert!((a - e).abs() < 1e-9, "factor summary b: {b:?}");
    }
    let fit = t
        .iter()
        .find(|t| t.title == "Model Goodness of Fit")
        .unwrap();
    let r2 = |r: usize| match fit.rows[r][2] {
        Cell::Number(n) => n,
        _ => f64::NAN,
    };
    assert!((r2(0) - 0.6901307194552455).abs() < 1e-10);
    assert!((r2(1) - 0.8349971761858794).abs() < 1e-10);
    let means = t.iter().find(|t| t.title == "Cell Means").unwrap();
    let cell = means.rows.iter().find(|r| label(&r[1]) == "2, 2").unwrap();
    assert_eq!(cell[2..], [Cell::Number(15.275), Cell::Number(4.0)]);
    // with covariates entered first, the experimental MCA leaves out the factor-adjusted values
    let t = run("ANOVA y BY a(1,3) b(1,2) WITH c /METHOD=EXPERIMENTAL /STATISTICS=MCA.");
    let mca = t.iter().find(|t| t.title == "MCA").unwrap();
    assert_eq!(mca.columns.len(), 7);
}

#[test]
fn warnings_and_errors() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!("{DATA}ANOVA y BY a(1,3) b(1,2) /STATISTICS=MCA."),
    );
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("not available"))
    );
    // the regression approach needs the lowest category
    let mut engine = Engine::default();
    let (commands, _) = parse(&format!("{DATA}ANOVA y BY a(0,3) b(1,2).\n"));
    let mut last = Ok(Vec::new());
    for c in &commands {
        last = engine.execute(&c.command);
    }
    assert!(last.unwrap_err().to_string().contains("lowest value"));
    // an empty cell drops the interaction with a warning
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{DATA}SELECT IF NOT(a = 3 AND b = 2).\nANOVA y BY a(1,3) b(1,2) /METHOD=EXPERIMENTAL."
        ),
    );
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("not of full rank"))
    );
}
