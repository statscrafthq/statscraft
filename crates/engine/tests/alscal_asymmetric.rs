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

fn data_of(rows: &str) -> String {
    format!("DATA LIST FREE /s1 TO s7.\nBEGIN DATA\n{rows}\nEND DATA.\n")
}

/// Seven points in a plane: their exact distances.
const EXACT: &str = "0 3.16227766 4.123105626 7.071067812 3.201562119 4.031128874 6.708203932
3.16227766 0 3.605551275 4.472135955 1.802775638 1.118033989 3.605551275
4.123105626 3.605551275 0 4.123105626 1.802775638 4.609772229 5.099019514
7.071067812 4.472135955 4.123105626 0 3.905124838 4.609772229 2.236067977
3.201562119 1.802775638 1.802775638 3.905124838 0 2.828427125 4.031128874
4.031128874 1.118033989 4.609772229 4.609772229 2.828427125 0 3.201562119
6.708203932 3.605551275 5.099019514 2.236067977 4.031128874 3.201562119 0";

/// The distances, row i multiplied by 1, 2.5, 0.6, 1.8, 3, 0.9, 1.4.
const SCALED_ROWS: &str = "0 3.16227766 4.123105626 7.071067812 3.201562119 4.031128874 6.708203932
7.90569415 0 9.013878189 11.18033989 4.506939094 2.795084972 9.013878189
2.473863375 2.163330765 0 2.473863375 1.081665383 2.765863337 3.059411708
12.72792206 8.049844719 7.421590126 0 7.029224708 8.297590012 4.024922359
9.604686356 5.408326913 5.408326913 11.71537451 0 8.485281374 12.09338662
3.628015987 1.00623059 4.148795006 4.148795006 2.545584412 0 2.881405907
9.391485505 5.047771786 7.138627319 3.130495168 5.643580424 4.482186966 0";

/// Row i of squared distances turned by an affine function of its own.
const AFFINE_ROWS: &str =
    "0 3.240370349 4.183300133 7.106335202 3.278719262 4.092676386 6.745368782
4.795831523 0 5.385164807 6.557438524 3.082207001 2.34520788 5.385164807
3.082207001 2.738612788 0 3.082207001 1.620185175 3.409545424 3.741657387
8.671793355 5.495452666 5.069516742 0 4.80364445 5.663479496 2.774887385
5.722761571 3.4278273 3.4278273 6.910137481 0 5.099019514 7.123903424
4.123105626 2.236067977 4.582575695 4.582575695 3.224903099 0 3.492849839
7.355270219 3.962322551 5.594640292 2.469817807 4.427188724 3.521363372 0";

/// Row i of the distances raised to its own power (1, 2, 0.5, 1.5, 3, 1, 0.7).
const POWER_ROWS: &str = "0 3.16227766 4.123105626 7.071067812 3.201562119 4.031128874 6.708203932
10 0 13 20 3.25 1.25 13
2.030543185 1.898828922 0 2.030543185 1.342674807 2.147038013 2.258100864
18.80301547 9.45741609 8.372144029 0 7.717068989 9.897356206 3.343701525
32.81601172 5.859020823 5.859020823 59.55315378 0 22.627417 65.5058442
4.031128874 1.118033989 4.609772229 4.609772229 2.828427125 0 3.201562119
3.789870583 2.454031728 3.127812218 1.756465005 2.653375275 2.258152942 0";

/// How far the distances between the printed coordinates are from the exact ones, after the
/// best common scale.
fn shape_error(tables: &[Table]) -> f64 {
    let x: Vec<Vec<f64>> = table(tables, "Stimulus Coordinates")
        .rows
        .iter()
        .map(|r| r[1..].iter().map(number).collect())
        .collect();
    let exact: Vec<Vec<f64>> = EXACT
        .lines()
        .map(|l| l.split_whitespace().map(|v| v.parse().unwrap()).collect())
        .collect();
    let (mut dd, mut de) = (0.0, 0.0);
    let mut pairs = Vec::new();
    for i in 0..7 {
        for j in 0..i {
            let d: f64 = x[i]
                .iter()
                .zip(&x[j])
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                .sqrt();
            dd += d * d;
            de += d * exact[i][j];
            pairs.push((d, exact[i][j]));
        }
    }
    let scale = de / dd;
    pairs
        .iter()
        .map(|(d, e)| (d * scale - e).abs())
        .fold(0.0, f64::max)
}

/// Each row of the data is its own measurement scale, so the fit is exact only when the
/// disparities of every row are found on their own; one scale for the whole matrix cannot.
#[test]
fn a_row_of_its_own_scale_is_fitted_by_conditioning_on_rows() {
    for (rows, level) in [(SCALED_ROWS, "RATIO"), (AFFINE_ROWS, "INTERVAL")] {
        let data = data_of(rows);
        let by = |condition: &str| {
            run(
                &data,
                &format!(
                    "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /LEVEL={level} /CONDITION={condition} /CRITERIA=DIMENS(2) CONVERGE(0) ITER(400) STRESSMIN(0)."
                ),
            )
        };
        let row = by("ROW");
        assert!(
            final_sstress(&row) < 1e-5,
            "{level}: {}",
            final_sstress(&row)
        );
        assert!(shape_error(&row) < 1e-3, "{level}: {}", shape_error(&row));
        let matrix = by("MATRIX");
        assert!(
            final_sstress(&matrix) > 0.01,
            "{level}: {}",
            final_sstress(&matrix)
        );
    }
}

/// Every row has its own increasing function of the distances: the order within a row is all
/// the data say.
#[test]
fn rows_that_differ_by_increasing_functions_fit_at_the_ordinal_level() {
    let data = data_of(POWER_ROWS);
    let by = |condition: &str| {
        run(
            &data,
            &format!(
                "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /LEVEL=ORDINAL /CONDITION={condition} /CRITERIA=DIMENS(2) CONVERGE(0) ITER(400) STRESSMIN(0)."
            ),
        )
    };
    let row = by("ROW");
    assert!(final_sstress(&row) < 1e-3, "{}", final_sstress(&row));
    assert!(final_sstress(&by("MATRIX")) > final_sstress(&row) * 5.0);
}

#[test]
fn row_conditionality_with_equal_rows_agrees_with_the_matrix() {
    // the exact distances in both triangles: every row has the same scale
    let data = data_of(EXACT);
    let command = |condition: &str| {
        format!(
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /LEVEL=RATIO /CONDITION={condition} /CRITERIA=DIMENS(2) CONVERGE(0) ITER(400) STRESSMIN(0)."
        )
    };
    for condition in ["ROW", "MATRIX", "UNCONDITIONAL"] {
        let tables = run(&data, &command(condition));
        assert!(final_sstress(&tables) < 1e-6, "{condition}");
        assert!(shape_error(&tables) < 1e-3, "{condition}");
    }
}

#[test]
fn the_header_names_the_conditionality_and_a_symmetric_matrix_has_no_rows() {
    let tables = run(
        SYM,
        "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /CONDITION=ROW /PRINT=HEADER.",
    );
    let options = table(&tables, "Analysis Options");
    let row = |name: &str| {
        options
            .rows
            .iter()
            .find(|r| r[0] == Cell::Text(name.into()))
            .map(|r| r[1].clone())
    };
    assert_eq!(row("Conditionality"), Some(Cell::Text("Row".into())));
    assert_eq!(row("Shape"), Some(Cell::Text("Asymmetric".into())));
    let (_, errors) = parse(&format!("{SYM}ALSCAL VARIABLES=s1 TO s7 /CONDITION=ROW."));
    assert!(
        errors
            .iter()
            .any(|e| e.message.contains("CONDITION=ROW needs SHAPE=ASYMMETRIC")),
        "{errors:?}"
    );
}

#[test]
fn the_shapes_and_models_not_yet_known_are_still_refused() {
    for (syntax, wanted) in [
        (
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=RECTANGULAR.",
            "SHAPE=RECTANGULAR",
        ),
        (
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /MODEL=ASCAL.",
            "MODEL=ASCAL",
        ),
    ] {
        let (_, errors) = parse(&format!("{SYM}{syntax}"));
        assert!(
            errors.iter().any(|e| e.message.contains(wanted)),
            "{syntax}: {errors:?}"
        );
    }
}
