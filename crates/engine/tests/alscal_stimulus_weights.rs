//! ALSCAL with MODEL=ASCAL and AINDS: every stimulus has weights of the dimensions as the first
//! of a pair, so that the distance from one to another is not the way back. The data are made
//! from known coordinates and weights, so that the exact recovery of the model is the check.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Seven places with the weights of the two axes each of them has as the first of a pair:
/// d(i, j)^2 = v_i1 (x_i1 - x_j1)^2 + v_i2 (x_i2 - x_j2)^2, the weights of each axis averaging 1.
const ONE: &str = "DATA LIST FREE /s1 TO s7.
BEGIN DATA
0 3.847076812 2.828427125 7.071067812 2.983286778 5.069516742 7.823042886
2.449489743 0 3.937003937 5.099019514 1.968501969 0.9354143467 3.240370349
3.741657387 3.464101615 0 4.472135955 1.732050808 4.538722287 5.54977477
7.071067812 4.857983121 3.535533906 0 3.798025803 5.198557492 2.42899156
3.020761493 1.620185175 1.620185175 4.077376608 0 2.828427125 4.911720676
3.154362059 0.9746794345 4.748684028 5.380520421 2.828427125 0 3.339161571
6.503845017 3.535533906 4.857983121 2.302172887 3.830796262 3.236510467 0
END DATA.
";

/// The same with a second judge who weighs the axes 2 and 0.5, and a third, 0.6 and 1.8.
const THREE: &str = "DATA LIST FREE /s1 TO s7.
BEGIN DATA
0 3.847076812 2.828427125 7.071067812 2.983286778 5.069516742 7.823042886
2.449489743 0 3.937003937 5.099019514 1.968501969 0.9354143467 3.240370349
3.741657387 3.464101615 0 4.472135955 1.732050808 4.538722287 5.54977477
7.071067812 4.857983121 3.535533906 0 3.798025803 5.198557492 2.42899156
3.020761493 1.620185175 1.620185175 4.077376608 0 2.828427125 4.911720676
3.154362059 0.9746794345 4.748684028 5.380520421 2.828427125 0 3.339161571
6.503845017 3.535533906 4.857983121 2.302172887 3.830796262 3.236510467 0
0 5.385164807 2.529822128 9.219544457 3.748332963 7.158910532 10.81665383
3.122498999 0 3.278719262 4 1.639359631 1.089724736 3.464101615
2.966479395 3.633180425 0 6.228964601 1.816590212 5.14781507 7.771743691
7.158910532 4 4.801041554 0 4.081972562 3.816084381 2
3.682729966 1.887458609 1.887458609 5.344389582 0 3.605551275 6.932712312
4.401704215 1.17260394 4.401704215 3.921096785 2.75680975 0 3.029026246
8.351646544 4.289522118 6.749074011 2 5.379358698 3.261518051 0
0 3.059411708 3.532704347 6.480740698 2.887905816 3.942080669 6.406246951
2.323790008 0 5.049752469 6.6633325 2.524876235 0.9874208829 3.674234614
4.874423043 3.979949748 0 3.6 1.989974874 4.911211663 4.409081537
8.306623863 6.254598308 3.009983389 0 4.290104894 6.914115996 3.127299154
3.03726851 1.710263138 1.710263138 3.704726711 0 2.683281573 3.824264635
2.527844932 0.9949874371 5.8403767 7.168681887 3.39411255 0 4.146082488
6.104096985 3.574912586 3.934463115 2.908607914 3.022416252 3.812479508 0
END DATA.
";

/// The stimulus weights the data were made with, and the coordinates.
const WEIGHTS: [[f64; 2]; 7] = [
    [1.6, 0.4],
    [0.5, 1.5],
    [1.2, 0.8],
    [0.7, 1.3],
    [1.5, 0.5],
    [0.6, 1.4],
    [0.9, 1.1],
];

fn run(data: &str, syntax: &str) -> Vec<Table> {
    let (commands, errors) = parse(&format!("{data}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
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

fn final_sstress(tables: &[Table]) -> f64 {
    let rows = &table(tables, "Iteration History").rows;
    number(&rows[rows.len() - 1][1])
}

fn columns(tables: &[Table], title: &str) -> Vec<Vec<f64>> {
    let rows = &table(tables, title).rows;
    let p = rows[0].len() - 1;
    (0..p)
        .map(|a| rows.iter().map(|r| number(&r[a + 1])).collect())
        .collect()
}

const TIGHT: &str = "CONVERGE(0) ITER(3000) STRESSMIN(0)";

fn command(model: &str, level: &str) -> String {
    format!(
        "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /MODEL={model} /LEVEL={level} /CRITERIA=DIMENS(2) {TIGHT} /PRINT=HEADER."
    )
}

/// The stimulus weights found (a mean of 1 per axis) against the true ones: the axes may come
/// in the other order.
fn weights_match(found: &[Vec<f64>], tolerance: f64) -> bool {
    let truth = |a: usize| -> Vec<f64> { WEIGHTS.iter().map(|w| w[a]).collect() };
    let close = |x: &[f64], y: &[f64]| x.iter().zip(y).all(|(p, q)| (p - q).abs() < tolerance);
    (close(&found[0], &truth(0)) && close(&found[1], &truth(1)))
        || (close(&found[0], &truth(1)) && close(&found[1], &truth(0)))
}

#[test]
fn distances_with_stimulus_weights_are_fitted_exactly_and_the_weights_recovered() {
    let tables = run(ONE, &command("ASCAL", "RATIO"));
    assert!(final_sstress(&tables) < 1e-5, "{}", final_sstress(&tables));
    let v = columns(&tables, "Stimulus Weights");
    assert!(weights_match(&v, 1e-2), "{v:?}");
    // a mean of 1 for every axis
    for axis in &v {
        assert!((axis.iter().sum::<f64>() / 7.0 - 1.0).abs() < 1e-9);
    }
    // the model the plain Euclidean one cannot match
    let euclid = run(ONE, &command("EUCLID", "RATIO"));
    assert!(final_sstress(&euclid) > 0.01, "{}", final_sstress(&euclid));
}

#[test]
fn individual_differences_with_stimulus_weights_are_fitted_exactly() {
    let tables = run(THREE, &command("AINDS", "RATIO"));
    assert!(final_sstress(&tables) < 1e-4, "{}", final_sstress(&tables));
    let v = columns(&tables, "Stimulus Weights");
    assert!(weights_match(&v, 2e-2), "{v:?}");
    // the judges weigh the axes 1 : 1, 2 : 0.5 and 0.6 : 1.8, so the ratio of the two weights of
    // a judge to that of the first is 4 and 1/3 (or the inverses, the axes the other way)
    let w = &table(&tables, "Subject Weights").rows;
    let ratio = |k: usize| number(&w[k][1]) / number(&w[k][2]);
    let (second, third) = (ratio(1) / ratio(0), ratio(2) / ratio(0));
    let direct = (second - 4.0).abs() < 0.05 && (third - 1.0 / 3.0).abs() < 0.01;
    let turned = (second - 0.25).abs() < 0.01 && (third - 3.0).abs() < 0.05;
    assert!(direct || turned, "{second} {third}");
}

#[test]
fn stimulus_weights_equal_to_one_are_the_euclidean_model() {
    // symmetric distances need no stimulus weights: ASCAL finds weights of 1
    let data = "DATA LIST FREE /s1 TO s7.\nBEGIN DATA\n0.0 3.3 4.3 7.2 3.2 4.0 6.6
3.3 0.0 3.7 4.5 1.7 1.0 3.5
4.3 3.7 0.0 4.1 1.7 4.5 5.0
7.2 4.5 4.1 0.0 3.8 4.5 2.3
3.2 1.7 1.7 3.8 0.0 2.9 4.1
4.0 1.0 4.5 4.5 2.9 0.0 3.4
6.6 3.5 5.0 2.3 4.1 3.4 0.0\nEND DATA.\n";
    let tables = run(data, &command("ASCAL", "RATIO"));
    let euclid = run(data, &command("EUCLID", "RATIO"));
    assert!(final_sstress(&tables) <= final_sstress(&euclid) + 1e-9);
}

#[test]
fn the_header_the_outfile_and_the_file_know_the_stimulus_weights() {
    let tables = run(
        &format!("{ONE}DATASET DECLARE conf.\n"),
        &format!(
            "{} /OUTFILE=conf.\nALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /MODEL=ASCAL /LEVEL=RATIO /FILE=conf /CRITERIA=DIMENS(2) CONVERGE(0) ITER(2) STRESSMIN(0).",
            command("ASCAL", "RATIO").trim_end_matches('.')
        ),
    );
    let options = table(&tables, "Analysis Options");
    let model = options
        .rows
        .iter()
        .find(|r| r[0] == Cell::Text("Model".into()))
        .map(|r| r[1].clone());
    assert_eq!(
        model,
        Some(Cell::Text("Asymmetric Euclidean (ASCAL)".into()))
    );
    // the second run starts where the first stopped, the weights included
    let histories: Vec<&Table> = tables
        .iter()
        .filter(|t| t.title == "Iteration History")
        .collect();
    assert_eq!(histories.len(), 2);
    assert!(number(&histories[1].rows[0][1]) < 1e-5);
}

#[test]
fn the_model_is_checked() {
    for (syntax, wanted) in [
        (
            "ALSCAL VARIABLES=s1 TO s7 /MODEL=ASCAL.",
            "MODEL=ASCAL needs SHAPE=ASYMMETRIC",
        ),
        (
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /MODEL=AINDS /CRITERIA=DIMENS(1).",
            "MODEL=AINDS needs at least 2 dimensions",
        ),
        (
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /CONDITION=ROW /MODEL=ASCAL.",
            "MODEL=ASCAL cannot be used with CONDITION=ROW",
        ),
        (
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /FILE=x STIMWGHT.",
            "STIMWGHT of FILE is used by MODEL=ASCAL and AINDS only",
        ),
    ] {
        let (_, errors) = parse(&format!("{ONE}{syntax}"));
        assert!(
            errors.iter().any(|e| e.message.contains(wanted)),
            "{syntax}: {errors:?}"
        );
    }
}

#[test]
fn ainds_with_one_matrix_is_ascal() {
    let tables = run(ONE, &command("AINDS", "RATIO"));
    assert!(table(&tables, "Stimulus Weights").rows.len() == 7);
    assert!(tables.iter().all(|t| t.title != "Subject Weights"));
}
