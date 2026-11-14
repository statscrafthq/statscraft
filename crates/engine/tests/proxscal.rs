//! PROXSCAL on synthetic proximities. The expected numbers come from a general statistics
//! package's majorization scaling (used as a black box) run to full convergence from the
//! classical scaling start; normalized raw stress is computed from its configuration and
//! transformed proximities. Exact cases: distances of points in a plane, plain and with
//! dimension weights per source, are recovered with stress 0.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Eight made-up places: their straight-line distances with some noise, rounded.
const PLACES: &str = "DATA LIST FREE /o1 TO o8.
BEGIN DATA.
0 4.92 4.54 3.93 1.74 6.96 1.91 1.98
4.92 0 4.9 1.61 4.51 10.63 3.82 4.64
4.54 4.9 0 4.62 6.57 5.91 5.6 2.8
3.93 1.61 4.62 0 2.63 10.83 1.89 2.62
1.74 4.51 6.57 2.63 0 8.17 0.84 3.13
6.96 10.63 5.91 10.83 8.17 0 10.55 7.51
1.91 3.82 5.6 1.89 0.84 10.55 0 2.03
1.98 4.64 2.8 2.62 3.13 7.51 2.03 0
END DATA.
";

/// Three judges' distances among seven items, each judge stretching the two axes of one
/// layout differently, with noise.
const JUDGES: &str = "DATA LIST FREE /a1 TO a7.
BEGIN DATA.
0 6.92 12.24 3.57 2.46 7.61 5.11
6.92 0 6.47 6.38 8.96 4.19 3.82
12.24 6.47 0 10.21 11.19 3.72 7.02
3.57 6.38 10.21 0 3.14 6.9 4.69
2.46 8.96 11.19 3.14 0 9.43 6.03
7.61 4.19 3.72 6.9 9.43 0 2.81
5.11 3.82 7.02 4.69 6.03 2.81 0
0 4.1 10.87 7.72 4.45 7.66 4.04
4.1 0 11.67 9.22 7.39 8.68 5.62
10.87 11.67 0 4.88 9.05 3.45 6.6
7.72 9.22 4.88 0 2.98 3.33 4.68
4.45 7.39 9.05 2.98 0 4.8 3.43
7.66 8.68 3.45 3.33 4.8 0 3.51
4.04 5.62 6.6 4.68 3.43 3.51 0
0 5.83 9.05 5.15 3.56 6.41 4.46
5.83 0 8.8 8.89 7.29 6.54 4.04
9.05 8.8 0 5.77 8.56 3.58 5.93
5.15 8.89 5.77 0 3.09 4.06 3.8
3.56 7.29 8.56 3.09 0 6.35 4.59
6.41 6.54 3.58 4.06 6.35 0 2.88
4.46 4.04 5.93 3.8 4.59 2.88 0
END DATA.
";

/// Run to convergence from the classical scaling start, as the oracle was.
const TIGHT: &str = "/INITIAL=TORGERSON /CRITERIA=MAXITER(100000) DIFFSTRESS(0) MINSTRESS(0)";

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

fn run(data: &str, syntax: &str) -> Vec<Table> {
    run_on(&mut Engine::default(), &format!("{data}{syntax}"))
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn stress(tables: &[Table], title: &str) -> f64 {
    number(&table(tables, title).rows[0][1])
}

#[test]
fn ratio_interval_and_ordinal_match_the_oracle() {
    for (transformation, expected) in [
        ("RATIO", 0.00606356615487),
        ("INTERVAL", 0.00478114289802),
        ("ORDINAL", 0.000654203648541),
        ("ORDINAL(UNTIE)", 0.000654203648541),
    ] {
        let tables = run(
            PLACES,
            &format!(
                "PROXSCAL o1 TO o8 /TRANSFORMATION={transformation} {TIGHT} /PRINT=STRESS DISTANCES."
            ),
        );
        let got = stress(&tables, "Stress and Fit Measures");
        assert!((got - expected).abs() < 1e-9, "{transformation}: {got}");
        if transformation == "RATIO" {
            let d = &table(&tables, "Distances").rows[0];
            let want = [
                0.9415108611,
                0.8387428320,
                0.6752347323,
                0.3241593821,
                1.3406314887,
                0.4232884163,
                0.3264373571,
            ];
            for (cell, w) in d[2..].iter().zip(want) {
                assert!((number(cell) - w).abs() < 1e-6, "{cell:?} against {w}");
            }
        }
    }
}

#[test]
fn fit_measures_follow_from_the_normalized_raw_stress() {
    let tables = run(PLACES, &format!("PROXSCAL o1 TO o8 {TIGHT}."));
    let rows = &table(&tables, "Stress and Fit Measures").rows;
    let raw = number(&rows[0][1]);
    // at the optimum the configuration needs no dilation, so Stress-I is the root
    assert!((number(&rows[1][1]) - raw.sqrt()).abs() < 1e-8);
    assert!((number(&rows[4][1]) - (1.0 - raw)).abs() < 1e-12);
    assert!((number(&rows[5][1]) - (1.0 - raw).sqrt()).abs() < 1e-12);
}

#[test]
fn several_dimensionalities_from_three_down_to_one() {
    let tables = run(
        PLACES,
        &format!("PROXSCAL o1 TO o8 {TIGHT} /CRITERIA=DIMENSIONS(1,3) /PLOT=STRESS COMMON."),
    );
    for (p, expected) in [
        (3, 0.00558796122604),
        (2, 0.00606356615487),
        (1, 0.0412238009455),
    ] {
        let title = format!("Stress and Fit Measures ({p} Dimensions)");
        let got = stress(&tables, &title);
        assert!((got - expected).abs() < 1e-9, "{p}: {got}");
    }
    assert_eq!(
        table(&tables, "Final Coordinates (1 Dimensions)").columns,
        ["", "Dimension 1"]
    );
}

#[test]
fn the_default_simplex_start_reaches_the_same_solution() {
    let tables = run(
        PLACES,
        "PROXSCAL o1 TO o8 /CRITERIA=MAXITER(100000) DIFFSTRESS(0) MINSTRESS(0).",
    );
    let got = stress(&tables, "Stress and Fit Measures");
    assert!((got - 0.00606356615487).abs() < 1e-9, "{got}");
}

#[test]
fn coordinates_are_on_principal_axes_with_the_first_object_positive() {
    let tables = run(PLACES, &format!("PROXSCAL o1 TO o8 {TIGHT}."));
    let rows = &table(&tables, "Final Coordinates").rows;
    let z: Vec<[f64; 2]> = rows
        .iter()
        .map(|r| [number(&r[1]), number(&r[2])])
        .collect();
    let cross: f64 = z.iter().map(|p| p[0] * p[1]).sum();
    let (s1, s2): (f64, f64) = (
        z.iter().map(|p| p[0] * p[0]).sum(),
        z.iter().map(|p| p[1] * p[1]).sum(),
    );
    assert!(cross.abs() < 1e-8 && s1 > s2);
    assert!(z[0][0] > 0.0 && z[0][1] > 0.0);
    let mean: f64 = z.iter().map(|p| p[0]).sum::<f64>() / 8.0;
    assert!(mean.abs() < 1e-10);
}

#[test]
fn identity_model_over_three_sources() {
    let tables = run(JUDGES, &format!("PROXSCAL a1 TO a7 {TIGHT}."));
    let got = stress(&tables, "Stress and Fit Measures");
    assert!((got - 0.0320940915991).abs() < 1e-9, "{got}");
    let summary = &table(&tables, "Case Processing Summary").rows;
    assert_eq!(number(&summary[0][1]), 3.0);
    assert_eq!(number(&summary[2][1]), 63.0);
}

#[test]
fn individual_differences_fit_at_least_as_well_as_the_oracle() {
    // the oracle stops a little short of the minimum; ours may only be lower
    for (model, oracle) in [
        ("WEIGHTED", 0.00430864972441),
        ("GENERALIZED", 0.00372292249071),
    ] {
        let tables = run(
            JUDGES,
            &format!("PROXSCAL a1 TO a7 /MODEL={model} {TIGHT} /PRINT=STRESS WEIGHTS INDIVIDUAL."),
        );
        let got = stress(&tables, "Stress and Fit Measures");
        assert!(
            got <= oracle + 1e-12 && got > oracle - 1e-6,
            "{model}: {got}"
        );
    }
}

/// Seven points of a plane: the distances of every source after stretching its axes.
fn stretched(weights: &[(f64, f64)]) -> String {
    let points = [
        (0.0, 0.0),
        (4.0, 1.0),
        (1.0, 5.0),
        (6.0, 4.0),
        (2.0, 2.5),
        (5.0, 7.0),
        (3.0, 0.5),
    ];
    let mut data = String::from("DATA LIST FREE /p1 TO p7.\nBEGIN DATA.\n");
    for &(wx, wy) in weights {
        for a in points {
            let row: Vec<String> = points
                .iter()
                .map(|b| {
                    (((a.0 - b.0) * wx).powi(2) + ((a.1 - b.1) * wy).powi(2))
                        .sqrt()
                        .to_string()
                })
                .collect();
            data.push_str(&row.join(" "));
            data.push('\n');
        }
    }
    data.push_str("END DATA.\n");
    data
}

#[test]
fn exact_weighted_distances_are_recovered() {
    let data = stretched(&[(2.0, 1.0), (1.5, 1.0)]);
    let tables = run(
        &data,
        &format!("PROXSCAL p1 TO p7 /MODEL=WEIGHTED {TIGHT} /PRINT=STRESS WEIGHTS."),
    );
    let got = stress(&tables, "Stress and Fit Measures");
    assert!(got < 1e-10, "{got}");
    let w = &table(&tables, "Dimension Weights").rows;
    let ratio = |r: &Vec<Cell>| number(&r[1]) / number(&r[2]);
    // the relative stretch of the two sources is what the data fix
    assert!((ratio(&w[0]) / ratio(&w[1]) - 2.0 / 1.5).abs() < 1e-5);
}

#[test]
fn a_proximities_matrix_file_is_scaled_back_to_its_points() {
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        "DATA LIST FREE /x y.\nBEGIN DATA.\n0 0\n4 1\n1 5\n6 4\n2 2.5\n5 7\nEND DATA.\n\
         PROXIMITIES x y /PRINT=NONE /MATRIX=OUT(*).\n\
         PROXSCAL /MATRIX=IN(*) /INITIAL=TORGERSON /CRITERIA=MAXITER(10000) DIFFSTRESS(0) MINSTRESS(0) /PRINT=STRESS DISTANCES.",
    );
    assert!(stress(&tables, "Stress and Fit Measures") < 1e-12);
    let d = &table(&tables, "Distances").rows;
    // distances of the points up to one factor: (0,0)-(4,1) against (0,0)-(1,5)
    let ratio = number(&d[0][2]) / number(&d[0][3]);
    assert!((ratio - 17f64.sqrt() / 26f64.sqrt()).abs() < 1e-8);
}

#[test]
fn table_form_and_upper_triangle_give_the_square_form_result() {
    let square = run(PLACES, &format!("PROXSCAL o1 TO o8 {TIGHT}."));
    let want = stress(&square, "Stress and Fit Measures");
    // the upper triangle of the same matrix
    let upper = run(PLACES, &format!("PROXSCAL o1 TO o8 /SHAPE=UPPER {TIGHT}."));
    assert!((stress(&upper, "Stress and Fit Measures") - want).abs() < 1e-12);
    // one proximity per case
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{PLACES}COMPUTE row = $CASENUM.\n\
             VARSTOCASES /MAKE prox FROM o1 TO o8 /INDEX=col.\n\
             SELECT IF col < row.\n"
        ),
    );
    let tables = run_on(
        &mut engine,
        &format!("PROXSCAL prox /TABLE=row BY col {TIGHT}."),
    );
    assert!((stress(&tables, "Stress and Fit Measures") - want).abs() < 1e-12);
}

#[test]
fn outfile_common_and_charts() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{PLACES}DATASET NAME places.\nDATASET DECLARE space.\n\
             PROXSCAL o1 TO o8 {TIGHT} /PLOT=COMMON TRANSFORMATIONS RESIDUALS /OUTFILE=COMMON(space).\n\
             DATASET ACTIVATE space.\n"
        ),
    );
    let data = engine.dataset().unwrap();
    assert_eq!(data.case_count(), 8);
    let names: Vec<&str> = data.variable_names().collect();
    assert_eq!(names, ["DIM_1", "DIM_2"]);
    match data.variable("DIM_1").unwrap().values() {
        Values::Numeric(v) => assert!(v.get(0).unwrap() > 0.0),
        _ => panic!("numeric"),
    }
}

#[test]
fn ratio_with_similarities_falls_back_to_interval() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!("{PLACES}PROXSCAL o1 TO o8 /PROXIMITIES=SIMILARITIES."),
    );
    assert!(engine.warnings().iter().any(|w| w.contains("INTERVAL")));
}

#[test]
fn table_sources_are_numbered_from_the_codes_present() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{PLACES}COMPUTE row = $CASENUM.\n\
             VARSTOCASES /MAKE prox FROM o1 TO o8 /INDEX=col.\n\
             SELECT IF col < row.\n\
             COMPUTE judge = 1.\n"
        ),
    );
    // a second copy of the matrix as source 3: there is no source 2
    let tables = run_on(
        &mut engine,
        &format!(
            "DATASET NAME first.\nDATASET COPY second.\nDATASET ACTIVATE second.\n\
             COMPUTE judge = 3.\nADD FILES /FILE=first /FILE=*.\nEXECUTE.\n\
             PROXSCAL prox /TABLE=row BY col BY judge {TIGHT}."
        ),
    );
    let summary = &table(&tables, "Case Processing Summary").rows;
    assert_eq!(number(&summary[0][1]), 2.0);
}
