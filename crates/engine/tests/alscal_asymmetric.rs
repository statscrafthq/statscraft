//! ALSCAL with SHAPE=ASYMMETRIC: both triangles of a matrix are data, the distances stay
//! symmetric. Checks that need no outside program: a symmetric matrix in both triangles gives
//! the symmetric fit, the transposed matrix gives the same fit, and the S-stress and the fit
//! tables agree with the formulas written out from the coordinates and the optimally scaled
//! data the tables show.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Seven made-up places, as a symmetric matrix of distances with noise.
const SYM: &str = "DATA LIST FREE /s1 TO s7.
BEGIN DATA
0.0 3.3 4.3 7.2 3.2 4.0 6.6
3.3 0.0 3.7 4.5 1.7 1.0 3.5
4.3 3.7 0.0 4.1 1.7 4.5 5.0
7.2 4.5 4.1 0.0 3.8 4.5 2.3
3.2 1.7 1.7 3.8 0.0 2.9 4.1
4.0 1.0 4.5 4.5 2.9 0.0 3.4
6.6 3.5 5.0 2.3 4.1 3.4 0.0
END DATA.
";

/// The same places as an asymmetric matrix: the distance from one to another differs by a
/// little from the way back.
const ASYM: &str = "DATA LIST FREE /s1 TO s7.
BEGIN DATA
0.0 2.9 4.6 7.4 2.9 4.4 7.2
3.6 0.0 3.5 4.9 1.6 1.1 4.1
4.7 3.6 0.0 4.6 1.8 4.0 5.5
7.3 4.8 3.8 0.0 4.4 4.2 2.2
3.0 1.9 1.8 3.6 0.0 3.0 3.6
3.7 1.2 5.0 4.1 2.8 0.0 3.0
6.6 3.4 5.7 2.3 3.6 3.5 0.0
END DATA.
";

/// The transpose of ASYM.
const ASYM_T: &str = "DATA LIST FREE /s1 TO s7.
BEGIN DATA
0.0 3.6 4.7 7.3 3.0 3.7 6.6
2.9 0.0 3.6 4.8 1.9 1.2 3.4
4.6 3.5 0.0 3.8 1.8 5.0 5.7
7.4 4.9 4.6 0.0 3.6 4.1 2.3
2.9 1.6 1.8 4.4 0.0 2.8 3.6
4.4 1.1 4.0 4.2 3.0 0.0 3.5
7.2 4.1 5.5 2.2 3.6 3.0 0.0
END DATA.
";

/// SYM again and a second judge, whose first axis counts more: two matrices.
const TWO: &str = "DATA LIST FREE /s1 TO s7.
BEGIN DATA
0.0 3.3 4.3 7.2 3.2 4.0 6.6
3.3 0.0 3.7 4.5 1.7 1.0 3.5
4.3 3.7 0.0 4.1 1.7 4.5 5.0
7.2 4.5 4.1 0.0 3.8 4.5 2.3
3.2 1.7 1.7 3.8 0.0 2.9 4.1
4.0 1.0 4.5 4.5 2.9 0.0 3.4
6.6 3.5 5.0 2.3 4.1 3.4 0.0
0.0 4.3 2.9 7.8 3.2 5.7 8.6
4.3 0.0 3.4 3.7 1.8 1.4 4.5
2.9 3.4 0.0 5.8 1.6 4.9 7.0
7.8 3.7 5.8 0.0 4.6 3.1 2.0
3.2 1.8 1.6 4.6 0.0 3.2 5.6
5.7 1.4 4.9 3.1 3.2 0.0 3.3
8.6 4.5 7.0 2.0 5.6 3.3 0.0
END DATA.
";

const TIGHT: &str = "CONVERGE(0) ITER(200) STRESSMIN(0)";

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

/// Every number of the table, row by row, blanks left out.
fn numbers(tables: &[Table], title: &str) -> Vec<f64> {
    table(tables, title)
        .rows
        .iter()
        .flat_map(|r| r.iter())
        .filter_map(|c| match c {
            Cell::Number(n) => Some(*n),
            _ => None,
        })
        .collect()
}

fn same(a: &[f64], b: &[f64], tolerance: f64, what: &str) {
    assert_eq!(a.len(), b.len(), "{what}");
    for (x, y) in a.iter().zip(b) {
        assert!((x - y).abs() < tolerance, "{what}: {x} against {y}");
    }
}

const TABLES: [&str; 2] = [
    "Stimulus Coordinates",
    "Stress and Squared Correlation (RSQ) in Distances",
];

/// The last S-stress of the iteration history; how many iterations run before the roundoff
/// stops them is not part of the fit.
fn final_sstress(tables: &[Table]) -> f64 {
    let rows = &table(tables, "Iteration History").rows;
    number(&rows[rows.len() - 1][1])
}

#[test]
fn a_symmetric_matrix_in_both_triangles_gives_the_symmetric_fit() {
    for level in [
        "ORDINAL",
        "ORDINAL(UNTIE)",
        "NOMINAL",
        "INTERVAL(2)",
        "RATIO",
    ] {
        let command = |shape: &str| {
            format!(
                "ALSCAL VARIABLES=s1 TO s7 /SHAPE={shape} /LEVEL={level} /CRITERIA=DIMENS(2) {TIGHT}."
            )
        };
        let symmetric = run(SYM, &command("SYMMETRIC"));
        let asymmetric = run(SYM, &command("ASYMMETRIC"));
        for title in TABLES {
            same(
                &numbers(&asymmetric, title),
                &numbers(&symmetric, title),
                1e-6,
                &format!("{level} {title}"),
            );
        }
        same(
            &[final_sstress(&asymmetric)],
            &[final_sstress(&symmetric)],
            1e-8,
            level,
        );
    }
}

#[test]
fn individual_differences_of_symmetric_matrices_in_both_triangles_are_the_symmetric_fit() {
    let command = |shape: &str| {
        format!(
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE={shape} /LEVEL=RATIO /MODEL=INDSCAL /CRITERIA=DIMENS(2) {TIGHT}."
        )
    };
    let symmetric = run(TWO, &command("SYMMETRIC"));
    let asymmetric = run(TWO, &command("ASYMMETRIC"));
    for title in [
        "Stimulus Coordinates",
        "Subject Weights",
        "Stress and Squared Correlation (RSQ) in Distances",
    ] {
        same(
            &numbers(&asymmetric, title),
            &numbers(&symmetric, title),
            1e-6,
            title,
        );
    }
    same(
        &[final_sstress(&asymmetric)],
        &[final_sstress(&symmetric)],
        1e-8,
        "S-stress",
    );
}

#[test]
fn the_transposed_matrix_gives_the_same_fit() {
    for level in ["ORDINAL", "RATIO"] {
        let command = format!(
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /LEVEL={level} /CRITERIA=DIMENS(2) {TIGHT}."
        );
        let a = run(ASYM, &command);
        let t = run(ASYM_T, &command);
        for title in TABLES {
            same(
                &numbers(&a, title),
                &numbers(&t, title),
                1e-8,
                &format!("{level} {title}"),
            );
        }
    }
}

/// S-stress, and stress and RSQ, from the coordinates and the optimally scaled data of the
/// tables, over every ordered pair of stimuli.
#[test]
fn the_fit_measures_run_over_every_ordered_pair() {
    let tables = run(
        ASYM,
        &format!(
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /LEVEL=ORDINAL /PRINT=DATA /CRITERIA=DIMENS(2) {TIGHT}."
        ),
    );
    let x: Vec<Vec<f64>> = table(&tables, "Stimulus Coordinates")
        .rows
        .iter()
        .map(|r| r[1..].iter().map(number).collect())
        .collect();
    let disparity = &table(&tables, "Optimally Scaled Data").rows;
    let (mut num, mut den) = (0.0, 0.0);
    let (mut d, mut s) = (Vec::new(), Vec::new());
    for i in 0..7 {
        for j in 0..7 {
            if i == j {
                continue;
            }
            let e: f64 = x[i].iter().zip(&x[j]).map(|(a, b)| (a - b).powi(2)).sum();
            let f = number(&disparity[i][j + 1]);
            num += (e - f * f).powi(2);
            den += f.powi(4);
            d.push(e.sqrt());
            s.push(f);
        }
    }
    let history = &table(&tables, "Iteration History").rows;
    let last = number(&history[history.len() - 1][1]);
    assert!((last - (num / den).sqrt()).abs() < 1e-9, "{last}");
    // stress formula 1 of the distances and the disparities, and their squared correlation
    let stress = (d.iter().zip(&s).map(|(a, b)| (a - b).powi(2)).sum::<f64>()
        / d.iter().map(|a| a * a).sum::<f64>())
    .sqrt();
    let n = d.len() as f64;
    let (md, ms) = (d.iter().sum::<f64>() / n, s.iter().sum::<f64>() / n);
    let cov: f64 = d.iter().zip(&s).map(|(a, b)| (a - md) * (b - ms)).sum();
    let rsq = cov * cov
        / (d.iter().map(|a| (a - md).powi(2)).sum::<f64>()
            * s.iter().map(|b| (b - ms).powi(2)).sum::<f64>());
    let row = &table(&tables, "Stress and Squared Correlation (RSQ) in Distances").rows[0];
    assert!((number(&row[1]) - stress).abs() < 1e-9, "stress");
    assert!((number(&row[2]) - rsq).abs() < 1e-9, "rsq");
    // the data table holds both triangles as given
    let raw = &table(&tables, "Raw Data").rows;
    assert_eq!(number(&raw[0][2]), 2.9);
    assert_eq!(number(&raw[1][1]), 3.6);
}

#[test]
fn an_asymmetric_matrix_is_not_the_symmetric_fit_of_its_lower_triangle() {
    let a = run(
        ASYM,
        &format!(
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /LEVEL=RATIO /CRITERIA=DIMENS(2) {TIGHT}."
        ),
    );
    let lower = run(
        ASYM,
        &format!("ALSCAL VARIABLES=s1 TO s7 /LEVEL=RATIO /CRITERIA=DIMENS(2) {TIGHT}."),
    );
    let (x, y) = (
        numbers(&a, "Stimulus Coordinates"),
        numbers(&lower, "Stimulus Coordinates"),
    );
    assert!(x.iter().zip(&y).any(|(p, q)| (p - q).abs() > 1e-3));
}

#[test]
fn the_models_and_conditions_not_yet_known_are_still_refused() {
    for (syntax, wanted) in [
        (
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=RECTANGULAR.",
            "SHAPE=RECTANGULAR",
        ),
        (
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /MODEL=ASCAL.",
            "MODEL=ASCAL",
        ),
        (
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /CONDITION=ROW.",
            "CONDITION=ROW",
        ),
    ] {
        let (_, errors) = parse(&format!("{SYM}{syntax}"));
        assert!(
            errors.iter().any(|e| e.message.contains(wanted)),
            "{syntax}: {errors:?}"
        );
    }
}
