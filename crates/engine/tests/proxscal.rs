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

#[test]
fn table_codes_far_beyond_the_cases_are_an_error() {
    let mut engine = Engine::default();
    let (commands, _) = parse(
        "DATA LIST FREE /r c d.\nBEGIN DATA.\n20240101 20240102 1\n20240101 20240103 2\n20240102 20240103 3\nEND DATA.\n\
         PROXSCAL d /TABLE=r BY c.\n",
    );
    let last = commands
        .iter()
        .map(|c| engine.execute(&c.command))
        .last()
        .unwrap();
    assert!(
        last.unwrap_err()
            .to_string()
            .contains("number the objects from 1")
    );
}

#[test]
fn the_reduced_rank_model_never_raises_the_stress() {
    let tables = run(
        JUDGES,
        "PROXSCAL a1 TO a7 /MODEL=REDUCED(1) /INITIAL=TORGERSON \
         /CRITERIA=MAXITER(500) DIFFSTRESS(0) MINSTRESS(0) /PRINT=HISTORY STRESS.",
    );
    let history: Vec<f64> = table(&tables, "Iteration History")
        .rows
        .iter()
        .map(|r| number(&r[1]))
        .collect();
    assert!(
        history.windows(2).all(|w| w[1] <= w[0] + 1e-12),
        "{history:?}"
    );
    // rank 1 spaces fit worse than the generalized model's full rank
    let got = stress(&tables, "Stress and Fit Measures");
    assert!(got.is_finite() && got > 0.00372292249071, "{got}");
}

#[test]
fn the_reduced_rank_model_moves_the_common_space_with_few_sources() {
    let data = stretched(&[(2.0, 1.0), (1.0, 2.0)]);
    let tables = run(
        &data,
        "PROXSCAL p1 TO p7 /MODEL=REDUCED(1) /INITIAL=TORGERSON \
         /CRITERIA=DIMENSIONS(3) MAXITER(300) DIFFSTRESS(0) MINSTRESS(0) /PRINT=HISTORY STRESS WEIGHTS.",
    );
    let history: Vec<f64> = table(&tables, "Iteration History")
        .rows
        .iter()
        .map(|r| number(&r[1]))
        .collect();
    assert!(
        history.windows(2).all(|w| w[1] <= w[0] + 1e-12),
        "{history:?}"
    );
    assert!(
        history.last().unwrap() < &(history[0] - 1e-3),
        "{history:?}"
    );
    // the rank 1 weights draw on more than the first dimension of the common space
    let rows = &table(&tables, "Space Weights").rows;
    let beyond: f64 = rows
        .iter()
        .filter(|r| number(&r[1]) > 1.0)
        .map(|r| number(&r[2]).abs())
        .sum();
    assert!(beyond > 1e-3, "{rows:?}");
}

/// A data file of coordinates; `.` is a missing value.
fn coordinate_file(dir: &std::path::Path, name: &str, rows: &[&str]) -> String {
    let path = dir.join(name);
    let source = format!(
        "DATA LIST FREE /d1 d2.\nBEGIN DATA.\n{}\nEND DATA.\nSAVE OUTFILE='{}'.\n",
        rows.join("\n"),
        path.display()
    );
    run_on(&mut Engine::default(), &source);
    path.display().to_string()
}

fn folder(name: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("statscraft-proxscal-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Fixed coordinates for the first four places; the rest are free. The expected numbers are
/// the minimum of normalized raw stress over the free coordinates, found by BFGS from 60
/// random starts in base R (its own gradient, the proximities normalized to n(n-1)/2 squares),
/// with the fixed coordinates at the scale of the file.
const FIXED_PLACES: [&str; 8] = [
    "0.5 1.5", "5 4", "4.5 -2", "3.5 .", ". .", ". .", ". .", ". .",
];

fn close(got: f64, want: f64, what: &str) {
    assert!((got - want).abs() < 1e-6, "{what}: {got} against {want}");
}

fn coordinates(tables: &[Table], title: &str) -> Vec<[f64; 2]> {
    table(tables, title)
        .rows
        .iter()
        .map(|r| [number(&r[1]), number(&r[2])])
        .collect()
}

#[test]
fn fixed_coordinates_stay_and_the_free_ones_reach_the_minimum() {
    let dir = folder("fixed");
    let file = coordinate_file(&dir, "fixed.sav", &FIXED_PLACES);
    for initial in ["TORGERSON", "SIMPLEX"] {
        let tables = run(
            PLACES,
            &format!(
                "PROXSCAL o1 TO o8 /RESTRICTIONS=COORDINATES('{file}') /INITIAL={initial} \
                 /CRITERIA=MAXITER(100000) DIFFSTRESS(0) MINSTRESS(0) /PRINT=COMMON STRESS HISTORY."
            ),
        );
        let z = coordinates(&tables, "Final Coordinates");
        let want = [
            [0.5, 1.5],
            [5.0, 4.0],
            [4.5, -2.0],
            [3.5, 3.723730359987],
            [0.14900439135, 3.181169313738],
            [-0.5153453172, -6.816864090725],
            [1.25278937381, 3.587730851725],
            [2.29264213491, 0.736442387854],
        ];
        for (i, (got, w)) in z.iter().zip(want).enumerate() {
            // the majorization stops a little short of the R optimum in the last digits
            assert!(
                (got[0] - w[0]).abs() < 1e-5 && (got[1] - w[1]).abs() < 1e-5,
                "{initial} object {i}: {got:?} against {w:?}"
            );
        }
        // the fixed ones are exactly the file's
        for (got, w) in z.iter().zip(want).take(3) {
            assert!((got[0] - w[0]).abs() < 1e-12 && (got[1] - w[1]).abs() < 1e-12);
        }
        assert!((z[3][0] - 3.5).abs() < 1e-12);
        let got = stress(&tables, "Stress and Fit Measures");
        assert!((got - 0.007514758863).abs() < 1e-8, "{initial}: {got}");
        // the stress never rises
        let history: Vec<f64> = table(&tables, "Iteration History")
            .rows
            .iter()
            .map(|r| number(&r[1]))
            .collect();
        assert!(history.len() > 5);
        assert!(
            history.windows(2).all(|w| w[1] <= w[0] + 1e-12),
            "{history:?}"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// A lower dimensionality takes the first column of the file and keeps the axes.
#[test]
fn fewer_dimensions_use_the_first_columns_of_the_file() {
    let dir = folder("one");
    let file = coordinate_file(&dir, "one.sav", &FIXED_PLACES);
    let tables = run(
        PLACES,
        &format!(
            "PROXSCAL o1 TO o8 /RESTRICTIONS=COORDINATES('{file}') d1 /INITIAL=TORGERSON \
             /CRITERIA=DIMENSIONS(1) MAXITER(100000) DIFFSTRESS(0) MINSTRESS(0) /PRINT=COMMON STRESS."
        ),
    );
    // One dimension has several local minima. The solution from the classical start is the one
    // below; in R it is a stationary point of the same stress (gradient 1.5e-9, polished to the
    // same values), and the best of 60 random starts there is another, at 0.107106962198.
    let z: Vec<f64> = table(&tables, "Final Coordinates")
        .rows
        .iter()
        .map(|r| number(&r[1]))
        .collect();
    let want = [
        0.5,
        5.0,
        4.5,
        3.5,
        1.62896282265,
        -4.27785462579,
        2.18213302570,
        1.20683293895,
    ];
    for (i, (got, w)) in z.iter().zip(want).enumerate() {
        close(*got, w, &format!("object {i}"));
    }
    let got = stress(&tables, "Stress and Fit Measures");
    assert!((got - 0.111608665221).abs() < 1e-8, "{got}");
    // from two down to one: the 1-D solution keeps the same fixed column
    let both = run(
        PLACES,
        &format!(
            "PROXSCAL o1 TO o8 /RESTRICTIONS=COORDINATES('{file}') /INITIAL=TORGERSON \
             /CRITERIA=DIMENSIONS(1,2) MAXITER(100000) DIFFSTRESS(0) MINSTRESS(0) /PRINT=COMMON."
        ),
    );
    let z1 = &table(&both, "Final Coordinates (1 Dimensions)").rows;
    for (row, w) in z1.iter().zip([0.5, 5.0, 4.5, 3.5]) {
        close(number(&row[1]), w, "fixed in one dimension");
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// With every coordinate given nothing is estimated: the configuration is the file's and the
/// stress is that of the fixed distances against the normalized proximities.
#[test]
fn a_file_that_fixes_everything_gives_its_own_stress() {
    let dir = folder("all");
    let rows = [
        "0.2 0.1", "2.1 3.2", "4.3 0.9", "0.8 4.1", "3.2 2.8", "5.1 5.2", "0.1 5.4", "4.2 4.1",
    ];
    let file = coordinate_file(&dir, "all.sav", &rows);
    let tables = run(
        PLACES,
        &format!("PROXSCAL o1 TO o8 /RESTRICTIONS=COORDINATES('{file}') ALL /PRINT=COMMON STRESS."),
    );
    let z = coordinates(&tables, "Final Coordinates");
    let pts: Vec<[f64; 2]> = rows
        .iter()
        .map(|r| {
            let v: Vec<f64> = r.split(' ').map(|v| v.parse().unwrap()).collect();
            [v[0], v[1]]
        })
        .collect();
    for (got, w) in z.iter().zip(&pts) {
        close(got[0], w[0], "x");
        close(got[1], w[1], "y");
    }
    // 1 - rho^2 / (eta_dhat eta_x) from the lower triangle of PLACES, worked out here
    let delta: Vec<Vec<f64>> = PLACES
        .lines()
        .skip(2)
        .take(8)
        .map(|l| l.split_whitespace().map(|v| v.parse().unwrap()).collect())
        .collect();
    let (mut ee, mut ff, mut ef) = (0.0, 0.0, 0.0);
    for i in 0..8 {
        for j in 0..i {
            let d = ((pts[i][0] - pts[j][0]).powi(2) + (pts[i][1] - pts[j][1]).powi(2)).sqrt();
            ee += delta[i][j] * delta[i][j];
            ff += d * d;
            ef += delta[i][j] * d;
        }
    }
    let expected = 1.0 - ef * ef / (ee * ff);
    let got = stress(&tables, "Stress and Fit Measures");
    assert!((got - expected).abs() < 1e-9, "{got} against {expected}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn restrictions_need_the_identity_model_and_a_file_of_the_right_size() {
    let dir = folder("errors");
    let short = coordinate_file(&dir, "short.sav", &FIXED_PLACES[..5]);
    let seven = coordinate_file(&dir, "seven.sav", &FIXED_PLACES[..7]);
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!(
        "{PLACES}PROXSCAL o1 TO o8 /RESTRICTIONS=COORDINATES('{short}').\n\
         {JUDGES}PROXSCAL a1 TO a7 /MODEL=WEIGHTED /RESTRICTIONS=COORDINATES('{seven}').\n"
    ));
    assert!(errors.is_empty(), "{errors:?}");
    let mut messages = Vec::new();
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            messages.push(e.to_string());
        }
    }
    assert!(
        messages[0].contains("5 cases") && messages[0].contains("8 objects"),
        "{messages:?}"
    );
    assert!(messages[1].contains("MODEL=IDENTITY only"), "{messages:?}");
    std::fs::remove_dir_all(&dir).ok();
}

/// A data file of independent variables, one column each; `.` is a missing value.
fn variable_file(dir: &std::path::Path, name: &str, columns: &[(&str, [&str; 8])]) -> String {
    let path = dir.join(name);
    let names: Vec<&str> = columns.iter().map(|c| c.0).collect();
    let rows: Vec<String> = (0..8)
        .map(|i| columns.iter().map(|c| c.1[i]).collect::<Vec<_>>().join(" "))
        .collect();
    let source = format!(
        "DATA LIST FREE /{}.\nBEGIN DATA.\n{}\nEND DATA.\nSAVE OUTFILE='{}'.\n",
        names.join(" "),
        rows.join("\n"),
        path.display()
    );
    run_on(&mut Engine::default(), &source);
    path.display().to_string()
}

const V1: [&str; 8] = ["0.5", "5.2", "4.1", "3.9", "1.1", "-0.4", "1.4", "2.4"];
const V2: [&str; 8] = ["1.6", "4.1", "-1.7", "3.8", "3.3", "-6.5", "3.6", "0.9"];

fn column(tables: &[Table], title: &str, j: usize) -> Vec<f64> {
    table(tables, title)
        .rows
        .iter()
        .map(|r| number(&r[1 + j]))
        .collect()
}

#[test]
fn interval_variables_reach_the_minimum_of_a_linear_space() {
    let dir = folder("vars");
    let file = variable_file(&dir, "v.sav", &[("east", V1), ("north", V2)]);
    let tables = run(
        PLACES,
        &format!(
            "PROXSCAL o1 TO o8 /RESTRICTIONS=VARIABLES('{file}') /INITIAL=TORGERSON \
             /CRITERIA=MAXITER(100000) DIFFSTRESS(0) MINSTRESS(0) \
             /PRINT=COMMON STRESS VARIABLES CORRELATIONS HISTORY."
        ),
    );
    // base R: the minimum of normalized raw stress over Z = V C, BFGS from 40 starts, gradient 1e-10
    let got = stress(&tables, "Stress and Fit Measures");
    assert!((got - 0.008143361916).abs() < 1e-8, "{got}");
    // the space is the weighted sum of the transformed variables
    let z = [
        column(&tables, "Final Coordinates", 0),
        column(&tables, "Final Coordinates", 1),
    ];
    let weights = &table(&tables, "Regression Weights").rows;
    let q = [
        column(&tables, "Transformed Independent Variables", 0),
        column(&tables, "Transformed Independent Variables", 1),
    ];
    for d in 0..2 {
        for i in 0..8 {
            let sum: f64 = (0..2).map(|j| q[j][i] * number(&weights[j][1 + d])).sum();
            assert!((sum - z[d][i]).abs() < 1e-9, "dimension {d} object {i}");
        }
    }
    // an interval variable is its values, centered and scaled to a sum of squares of 8
    for (j, values) in [V1, V2].iter().enumerate() {
        let v: Vec<f64> = values.iter().map(|s| s.parse().unwrap()).collect();
        let mean = v.iter().sum::<f64>() / 8.0;
        let ss: f64 = v.iter().map(|x| (x - mean).powi(2)).sum();
        let sign = (q[j][1] * (v[1] - mean)).signum();
        for i in 0..8 {
            let want = sign * (v[i] - mean) * (8.0 / ss).sqrt();
            assert!((q[j][i] - want).abs() < 1e-9, "variable {j} object {i}");
        }
    }
    // correlations of a variable with the dimensions
    let r = &table(&tables, "Correlations").rows;
    for j in 0..2 {
        for d in 0..2 {
            let mq = q[j].iter().sum::<f64>() / 8.0;
            let mz = z[d].iter().sum::<f64>() / 8.0;
            let cov: f64 = (0..8).map(|i| (q[j][i] - mq) * (z[d][i] - mz)).sum();
            let sq: f64 = q[j].iter().map(|x| (x - mq).powi(2)).sum();
            let sz: f64 = z[d].iter().map(|x| (x - mz).powi(2)).sum();
            close(number(&r[j][1 + d]), cov / (sq * sz).sqrt(), "correlation");
        }
    }
    let history: Vec<f64> = table(&tables, "Iteration History")
        .rows
        .iter()
        .map(|r| number(&r[1]))
        .collect();
    assert!(
        history.windows(2).all(|w| w[1] <= w[0] + 1e-12),
        "{history:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// One variable for two dimensions: the second dimension is a free dummy variable, so one
/// direction of the space is the variable and the other is free.
#[test]
fn a_dimension_the_variables_leave_over_is_free() {
    let dir = folder("dummy");
    let file = variable_file(&dir, "v.sav", &[("east", V1), ("pad", V2)]);
    let tables = run(
        PLACES,
        &format!(
            "PROXSCAL o1 TO o8 /RESTRICTIONS=VARIABLES('{file}') east /INITIAL=TORGERSON \
             /CRITERIA=MAXITER(100000) DIFFSTRESS(0) MINSTRESS(0) /PRINT=COMMON STRESS VARIABLES."
        ),
    );
    // base R: Z = v b1' + u b2' with u free, 40 BFGS starts
    let got = stress(&tables, "Stress and Fit Measures");
    assert!((got - 0.007788004451).abs() < 1e-8, "{got}");
    assert_eq!(table(&tables, "Regression Weights").rows.len(), 1);
    std::fs::remove_dir_all(&dir).ok();
}

const REG: [&str; 8] = ["1", "3", "2", "3", "1", "2", "1", "2"];
const RANK: [&str; 8] = ["1", "3", "2", "4", "1", "4", "2", "3"];

fn variables_run(dir: &std::path::Path, columns: &[(&str, [&str; 8])], rest: &str) -> Vec<Table> {
    let file = variable_file(dir, "v.sav", columns);
    run(
        PLACES,
        &format!(
            "PROXSCAL o1 TO o8 /RESTRICTIONS=VARIABLES('{file}') {rest} /INITIAL=TORGERSON \
             /CRITERIA=MAXITER(100000) DIFFSTRESS(0) MINSTRESS(0) /PRINT=COMMON STRESS VARIABLES HISTORY."
        ),
    )
}

fn never_rises(tables: &[Table]) {
    let history: Vec<f64> = table(tables, "Iteration History")
        .rows
        .iter()
        .map(|r| number(&r[1]))
        .collect();
    assert!(history.len() > 5);
    assert!(
        history.windows(2).all(|w| w[1] <= w[0] + 1e-12),
        "{history:?}"
    );
}

/// A nominal and an ordinal variable: the quantifications are the same within a category and
/// rise with the ordinal values (ties kept), the stress never rises, and the solution is a local
/// minimum of the model in base R (Z = Y1[reg] b1' + Y2[rank] b2' with free nominal values and
/// squared increments for the ordinal ones): the stress evaluated there is the engine's and BFGS
/// from this point moves it by 1e-16. Random starts in R find lower local minima, 0.1103.
#[test]
fn nominal_and_ordinal_variables_are_quantified_in_their_classes() {
    let dir = folder("cat");
    let tables = variables_run(&dir, &[("reg", REG), ("rank", RANK)], "(NOMINAL ORDINAL)");
    never_rises(&tables);
    let q = [
        column(&tables, "Transformed Independent Variables", 0),
        column(&tables, "Transformed Independent Variables", 1),
    ];
    let same = |a: usize, b: usize, j: usize| (q[j][a] - q[j][b]).abs() < 1e-12;
    // reg: objects 1, 5, 7 | 3, 6, 8 | 2, 4
    assert!(same(0, 4, 0) && same(0, 6, 0) && same(2, 5, 0) && same(2, 7, 0) && same(1, 3, 0));
    // rank: 1 at objects 1, 5; 2 at 3, 7; 3 at 2, 8; 4 at 4, 6
    assert!(same(0, 4, 1) && same(2, 6, 1) && same(1, 7, 1) && same(3, 5, 1));
    assert!(q[1][0] <= q[1][2] && q[1][2] <= q[1][1] && q[1][1] <= q[1][3]);
    // normalized: mean 0, sum of squares 8
    for col in &q {
        assert!(col.iter().sum::<f64>().abs() < 1e-9);
        assert!((col.iter().map(|x| x * x).sum::<f64>() - 8.0).abs() < 1e-9);
    }
    let got = stress(&tables, "Stress and Fit Measures");
    assert!((got - 0.135826464171).abs() < 1e-8, "{got}");
    std::fs::remove_dir_all(&dir).ok();
}

/// UNTIE lets tied values part: it can only fit at least as well as KEEPTIES from the same
/// start, and a missing value takes the fitted target, so the variable still has all objects.
#[test]
fn untied_ordinal_missing_values_and_splines_work() {
    let dir = folder("cat2");
    let ties = variables_run(
        &dir,
        &[("rank", RANK), ("east", V1)],
        "(ORDINAL(KEEPTIES) INTERVAL)",
    );
    let untied = variables_run(
        &dir,
        &[("rank", RANK), ("east", V1)],
        "(ORDINAL(UNTIE) INTERVAL)",
    );
    never_rises(&ties);
    never_rises(&untied);
    let kept = column(&ties, "Transformed Independent Variables", 0);
    assert!((kept[0] - kept[4]).abs() < 1e-12);
    assert!(
        stress(&untied, "Stress and Fit Measures")
            <= stress(&ties, "Stress and Fit Measures") + 1e-9
    );
    let gap: [&str; 8] = ["1", "3", ".", "4", "1", "4", "2", "3"];
    let missing = variables_run(&dir, &[("rank", gap), ("east", V1)], "(ORDINAL INTERVAL)");
    never_rises(&missing);
    assert_eq!(
        table(&missing, "Transformed Independent Variables")
            .rows
            .len(),
        8
    );
    let spline = variables_run(
        &dir,
        &[("east", V1), ("north", V2)],
        "(SPLINE DEGREE=2 INKNOT=2 SPLINE)",
    );
    never_rises(&spline);
    // a monotone spline is a nondecreasing function of the values
    let q = column(&spline, "Transformed Independent Variables", 0);
    let x: Vec<f64> = V1.iter().map(|s| s.parse().unwrap()).collect();
    for i in 0..8 {
        for j in 0..8 {
            if x[i] < x[j] {
                assert!(q[i] <= q[j] + 1e-9, "objects {i} and {j}");
            }
        }
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn variables_with_fewer_dimensions_outfile_and_input() {
    let dir = folder("misc");
    let file = variable_file(&dir, "v.sav", &[("east", V1), ("north", V2)]);
    let out = dir.join("q.sav").display().to_string();
    let tables = run(
        PLACES,
        &format!(
            "PROXSCAL o1 TO o8 /RESTRICTIONS=VARIABLES('{file}') /CRITERIA=DIMENSIONS(1,2) \
             /PRINT=INPUT VARIABLES CORRELATIONS /OUTFILE=VARIABLES('{out}')."
        ),
    );
    assert_eq!(
        table(&tables, "Independent Variables").columns,
        ["", "east", "north"]
    );
    for suffix in ["(1 Dimensions)", "(2 Dimensions)"] {
        for title in [
            "Transformed Independent Variables",
            "Regression Weights",
            "Correlations",
        ] {
            table(&tables, &format!("{title} {suffix}"));
        }
    }
    // one dimension: both variables weigh the one axis; the file holds the last solution
    assert_eq!(
        table(&tables, "Regression Weights (1 Dimensions)")
            .columns
            .len(),
        2
    );
    let written = run(
        "",
        &format!("GET FILE='{out}'.\nDESCRIPTIVES VAR_1 VAR_2 /STATISTICS=MEAN STDDEV.\n"),
    );
    let rows = &table(&written, "Descriptive Statistics").rows;
    let labels: Vec<String> = rows.iter().map(|r| format!("{:?}", r[0])).collect();
    assert!(
        labels.iter().any(|l| l.contains("VAR_1")) && labels.iter().any(|l| l.contains("VAR_2"))
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn variable_restrictions_check_their_file() {
    let dir = folder("verr");
    let file = variable_file(&dir, "v.sav", &[("east", V1), ("north", V2)]);
    let short = coordinate_file(&dir, "short.sav", &FIXED_PLACES[..5]);
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!(
        "{PLACES}PROXSCAL o1 TO o8 /RESTRICTIONS=VARIABLES('{short}').\n\
         PROXSCAL o1 TO o8 /RESTRICTIONS=VARIABLES('{file}') (NOMINAL ORDINAL INTERVAL).\n\
         PROXSCAL o1 TO o8 /RESTRICTIONS=VARIABLES('{file}') FIRST(3).\n\
         {JUDGES}PROXSCAL a1 TO a7 /MODEL=WEIGHTED /RESTRICTIONS=VARIABLES('{file}').\n"
    ));
    assert!(errors.is_empty(), "{errors:?}");
    let mut messages = Vec::new();
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            messages.push(e.to_string());
        }
    }
    assert!(
        messages[0].contains("5 cases") && messages[0].contains("8 objects"),
        "{messages:?}"
    );
    assert!(
        messages[1].contains("3 transformations") && messages[1].contains("2 independent"),
        "{messages:?}"
    );
    assert!(messages[2].contains("FIRST(3)"), "{messages:?}");
    assert!(messages[3].contains("MODEL=IDENTITY only"), "{messages:?}");
    std::fs::remove_dir_all(&dir).ok();
}

/// A start from a file: the stress at the start is that of the file's configuration, and the
/// iterations then reach the same minimum as the other starts do (0.00606356615487, the ratio
/// oracle above).
#[test]
fn a_start_from_a_file_is_used_as_given() {
    let dir = folder("start");
    // the common space of a converged run, rotated and stretched: the start is the same
    // configuration up to similarity, so it starts at the minimum
    let converged = run(
        PLACES,
        "PROXSCAL o1 TO o8 /INITIAL=TORGERSON /CRITERIA=MAXITER(100000) DIFFSTRESS(0) MINSTRESS(0) \
         /PRINT=COMMON STRESS.",
    );
    let z = coordinates(&converged, "Final Coordinates");
    let rows: Vec<String> = z
        .iter()
        .map(|p| format!("{} {}", 3.0 * p[1], -3.0 * p[0]))
        .collect();
    let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
    let file = coordinate_file(&dir, "start.sav", &rows);
    let tables = run(
        PLACES,
        &format!(
            "PROXSCAL o1 TO o8 /INITIAL=('{file}') /CRITERIA=MAXITER(100000) DIFFSTRESS(0) MINSTRESS(0) \
             /PRINT=STRESS HISTORY."
        ),
    );
    let history: Vec<f64> = table(&tables, "Iteration History")
        .rows
        .iter()
        .map(|r| number(&r[1]))
        .collect();
    assert!(
        (history[0] - 0.00606356615487).abs() < 1e-7,
        "{}",
        history[0]
    );
    assert!((stress(&tables, "Stress and Fit Measures") - 0.00606356615487).abs() < 1e-9);
    // a rough start (the places on a line and a bend) still gets there
    let rough = coordinate_file(
        &dir,
        "rough.sav",
        &["0 0", "1 3", "2 0", "3 3", "4 0", "5 3", "6 0", "7 3"],
    );
    let tables = run(
        PLACES,
        &format!(
            "PROXSCAL o1 TO o8 /INITIAL=('{rough}') d1 d2 /CRITERIA=MAXITER(100000) DIFFSTRESS(0) \
             MINSTRESS(0) /PRINT=STRESS HISTORY."
        ),
    );
    let history: Vec<f64> = table(&tables, "Iteration History")
        .rows
        .iter()
        .map(|r| number(&r[1]))
        .collect();
    assert!(history[0] > 0.1, "{}", history[0]);
    assert!(history.windows(2).all(|w| w[1] <= w[0] + 1e-12));
    // a start from the file goes with a smaller dimensionality too (its first column)
    let one = run(
        PLACES,
        &format!("PROXSCAL o1 TO o8 /INITIAL=('{rough}') /CRITERIA=DIMENSIONS(1,2) /PRINT=STRESS."),
    );
    table(&one, "Stress and Fit Measures (1 Dimensions)");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_start_file_needs_every_coordinate() {
    let dir = folder("starterr");
    let file = coordinate_file(
        &dir,
        "gap.sav",
        &["0 0", "1 3", "2 .", "3 3", "4 0", "5 3", "6 0", "7 3"],
    );
    let short = coordinate_file(&dir, "short.sav", &["0 0", "1 3", "2 0"]);
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!(
        "{PLACES}PROXSCAL o1 TO o8 /INITIAL=('{file}').\nPROXSCAL o1 TO o8 /INITIAL=('{short}').\n"
    ));
    assert!(errors.is_empty(), "{errors:?}");
    let mut messages = Vec::new();
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            messages.push(e.to_string());
        }
    }
    assert!(
        messages[0].contains("missing value among the initial"),
        "{messages:?}"
    );
    assert!(
        messages[1].contains("3 cases") && messages[1].contains("8 objects"),
        "{messages:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

fn points_of(chart: &statscraft_engine::Chart) -> Vec<(f64, f64, Option<String>)> {
    match chart.plot.as_ref().expect("a plot") {
        statscraft_engine::plot::Plot::Points { groups, .. } => groups
            .iter()
            .flat_map(|g| g.points.iter().map(|p| (p.x, p.y, p.label.clone())))
            .collect(),
        other => panic!("not points: {other:?}"),
    }
}

/// The plots of the independent variables: the correlations of each variable with the two
/// dimensions are the points of the Correlations plot (the table's values), and a variable's
/// transformation plot has a point for each object with the original and the transformed value.
#[test]
fn the_plots_of_the_independent_variables() {
    let dir = folder("plots");
    let gap: [&str; 8] = ["1", "3", ".", "4", "1", "4", "2", "3"];
    let file = variable_file(&dir, "v.sav", &[("rank", gap), ("east", V1)]);
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!(
            "{PLACES}PROXSCAL o1 TO o8 /RESTRICTIONS=VARIABLES('{file}') (ORDINAL INTERVAL) \
             /PLOT=CORRELATIONS VARIABLES(rank) /PRINT=CORRELATIONS VARIABLES."
        ),
    );
    let charts = engine.take_charts();
    let titles: Vec<&str> = charts.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, ["Correlations", "Transformation Plot: rank"]);
    let table = table(&tables, "Correlations");
    let points = points_of(&charts[0]);
    assert_eq!(points.len(), 2);
    for (j, (x, y, label)) in points.iter().enumerate() {
        close(*x, number(&table.rows[j][1]), "x");
        close(*y, number(&table.rows[j][2]), "y");
        assert_eq!(label.as_deref(), Some(["rank", "east"][j]));
    }
    // the variable with a missing value has no point at that object
    let q = column(&tables, "Transformed Independent Variables", 0);
    let transformed = points_of(&charts[1]);
    assert_eq!(transformed.len(), 7);
    for (x, y, label) in &transformed {
        let object: usize = label.as_deref().unwrap()[1..].parse::<usize>().unwrap() - 1;
        assert!(object != 2);
        close(*y, q[object], "transformed value");
        let original = ["1", "3", ".", "4", "1", "4", "2", "3"][object];
        close(*x, original.parse().unwrap(), "original value");
    }
    // the default plots of a restricted run: the common space and the correlations
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!("{PLACES}PROXSCAL o1 TO o8 /RESTRICTIONS=VARIABLES('{file}')."),
    );
    let titles: Vec<String> = engine.take_charts().into_iter().map(|c| c.title).collect();
    assert_eq!(titles, ["Common Space", "Correlations"]);
    std::fs::remove_dir_all(&dir).ok();
}
