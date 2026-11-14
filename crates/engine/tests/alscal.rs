//! ALSCAL on synthetic proximities. The expected S-stress values are minima found by a
//! general-purpose optimizer (used as a black box) of S-stress written directly as a function
//! of the coordinates: at the ratio level one minus the squared cosine of the squared
//! distances and the squared data, at the ordinal level one minus the share of the squared
//! distances that their monotone regression on the data keeps, started from classical
//! scaling. Exact cases: distances of points in a plane are recovered with S-stress 0.

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

const TIGHT: &str = "CONVERGE(0) ITER(5000) STRESSMIN(0)";

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

/// The last S-stress of an iteration history.
fn sstress(tables: &[Table], title: &str) -> f64 {
    let rows = &table(tables, title).rows;
    number(&rows[rows.len() - 1][1])
}

fn distance(z: &[Vec<f64>], i: usize, j: usize) -> f64 {
    z[i].iter()
        .zip(&z[j])
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn coordinates(tables: &[Table], title: &str) -> Vec<Vec<f64>> {
    table(tables, title)
        .rows
        .iter()
        .map(|r| r[1..].iter().map(number).collect())
        .collect()
}

#[test]
fn ratio_level_in_one_to_three_dimensions_matches_the_oracle() {
    let tables = run(
        PLACES,
        &format!("ALSCAL VARIABLES=o1 TO o8 /LEVEL=RATIO /CRITERIA=DIMENS(1,3) {TIGHT}."),
    );
    for (p, expected, ratio) in [
        (3, 0.0955766469312, 1.12924538),
        (2, 0.102447767905, 1.12861516),
        (1, 0.207845498174, 1.872254479),
    ] {
        let got = sstress(&tables, &format!("Iteration History ({p} Dimensions)"));
        assert!((got - expected).abs() < 1e-9, "{p}: {got}");
        let z = coordinates(&tables, &format!("Stimulus Coordinates ({p} Dimensions)"));
        let r = distance(&z, 0, 1) / distance(&z, 0, 2);
        assert!((r - ratio).abs() < 1e-6, "{p}: {r}");
    }
}

#[test]
fn ordinal_level_matches_the_oracle() {
    let tables = run(
        PLACES,
        &format!("ALSCAL VARIABLES=o1 TO o8 /CRITERIA={TIGHT}."),
    );
    let got = sstress(&tables, "Iteration History");
    assert!((got - 0.0245250511136).abs() < 1e-9, "{got}");
    let fit = &table(&tables, "Stress and Squared Correlation (RSQ) in Distances").rows[0];
    assert!(number(&fit[1]) > 0.0 && number(&fit[2]) > 0.99);
}

#[test]
fn indscal_matches_the_oracle() {
    let tables = run(
        JUDGES,
        &format!("ALSCAL VARIABLES=a1 TO a7 /LEVEL=RATIO /MODEL=INDSCAL /CRITERIA={TIGHT}."),
    );
    let got = sstress(&tables, "Iteration History");
    assert!((got - 0.120964109186).abs() < 1e-8, "{got}");
    // the weights of one dimension over the other, relative between the judges, are fixed by
    // the data: the oracle's are 3.9699754, 0.2531404 and 0.5974620 (its first dimension is
    // our second)
    let w = &table(&tables, "Subject Weights").rows;
    let ratio = |k: usize| number(&w[k][2]) / number(&w[k][1]);
    assert!((ratio(0) / ratio(1) - 3.9699754 / 0.2531404).abs() < 1e-3);
    assert!((ratio(2) / ratio(1) - 0.5974620 / 0.2531404).abs() < 1e-4);
    let averaged = &table(&tables, "Stress and Squared Correlation (RSQ) in Distances").rows[3];
    assert_eq!(averaged[0], Cell::Text("Averaged".into()));
}

#[test]
fn exact_distances_are_recovered_at_every_level() {
    let points = [
        (0.0, 0.0),
        (4.0, 1.0),
        (1.0, 5.0),
        (6.0, 4.0),
        (2.0, 2.5),
        (5.0, 7.0),
    ];
    let mut data = String::from("DATA LIST FREE /p1 TO p6.\nBEGIN DATA.\n");
    for a in points {
        let row: Vec<String> = points
            .iter()
            .map(|b: &(f64, f64)| {
                ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2))
                    .sqrt()
                    .to_string()
            })
            .collect();
        data.push_str(&row.join(" "));
        data.push('\n');
    }
    data.push_str("END DATA.\n");
    for level in ["RATIO", "INTERVAL", "ORDINAL"] {
        let tables = run(
            &data,
            &format!("ALSCAL VARIABLES=p1 TO p6 /LEVEL={level} /CRITERIA={TIGHT}."),
        );
        let got = sstress(&tables, "Iteration History");
        assert!(got < 1e-6, "{level}: {got}");
        if level != "ORDINAL" {
            let z = coordinates(&tables, "Stimulus Coordinates");
            let r = distance(&z, 0, 1) / distance(&z, 0, 2);
            assert!(
                (r - 17f64.sqrt() / 26f64.sqrt()).abs() < 1e-6,
                "{level}: {r}"
            );
        }
    }
}

#[test]
fn coordinates_are_centered_principal_axes_with_mean_square_one() {
    let tables = run(PLACES, "ALSCAL VARIABLES=o1 TO o8 /PLOT.");
    let z = coordinates(&tables, "Stimulus Coordinates");
    let ss: f64 = z.iter().flatten().map(|v| v * v).sum();
    assert!((ss - 16.0).abs() < 1e-9);
    let cross: f64 = z.iter().map(|r| r[0] * r[1]).sum();
    assert!(cross.abs() < 1e-9);
    assert!(z[0][0] > 0.0 && z[0][1] > 0.0);
    // the default stops early: at most 30 iterations
    assert!(table(&tables, "Iteration History").rows.len() <= 30);
}

#[test]
fn a_proximities_matrix_and_the_outfile() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        "DATA LIST FREE /x y.\nBEGIN DATA.\n0 0\n4 1\n1 5\n6 4\n2 2.5\n5 7\nEND DATA.\n\
         PROXIMITIES x y /PRINT=NONE /MATRIX=OUT(*).\n\
         DATASET NAME dist.\nDATASET DECLARE conf.\n\
         ALSCAL /MATRIX=IN(*) /LEVEL=RATIO /CRITERIA=CONVERGE(0) ITER(500) STRESSMIN(0) /OUTFILE=conf.\n\
         DATASET ACTIVATE conf.\n",
    );
    let data = engine.dataset().unwrap();
    let names: Vec<&str> = data.variable_names().collect();
    assert_eq!(names, ["TYPE_", "DIMENS", "MATNUM", "DIM1", "DIM2"]);
    assert_eq!(data.case_count(), 6);
}

#[test]
fn matrix_input_takes_the_items_that_variables_names() {
    let tables = run_on(
        &mut Engine::default(),
        "DATA LIST FREE /x y.\nBEGIN DATA.\n0 0\n4 1\n1 5\n6 4\n2 2.5\n5 7\nEND DATA.\n\
         PROXIMITIES x y /PRINT=NONE /MATRIX=OUT(*).\n\
         ALSCAL VARIABLES=VAR1 TO VAR4 VAR6 /MATRIX=IN(*) /LEVEL=RATIO.\n",
    );
    assert_eq!(coordinates(&tables, "Stimulus Coordinates").len(), 5);
}
