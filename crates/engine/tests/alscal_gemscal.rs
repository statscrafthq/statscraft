//! ALSCAL with MODEL=GEMSCAL: every matrix has a metric of its own on the group space, of rank
//! q (the directions). The data are made from known coordinates and metrics, so that the exact
//! recovery of the model is the check: matrices that turn the axes are fitted by GEMSCAL and
//! not by INDSCAL, and data of fewer directions need DIRECTIONS.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Seven places and three matrices: the first looks at the plane as it is, the second weighs the
/// axes 2 and 0.5 after a turn of 30 degrees, the third 0.6 and 1.8 after a turn of -50 degrees.
const FULL: &str = "DATA LIST FREE /s1 TO s7.
BEGIN DATA
0 3.16227766 4.123105626 7.071067812 3.201562119 4.031128874 6.708203932
3.16227766 0 3.605551275 4.472135955 1.802775638 1.118033989 3.605551275
4.123105626 3.605551275 0 4.123105626 1.802775638 4.609772229 5.099019514
7.071067812 4.472135955 4.123105626 0 3.905124838 4.609772229 2.236067977
3.201562119 1.802775638 1.802775638 3.905124838 0 2.828427125 4.031128874
4.031128874 1.118033989 4.609772229 4.609772229 2.828427125 0 3.201562119
6.708203932 3.605551275 5.099019514 2.236067977 4.031128874 3.201562119 0
0 4.404215517 4.563020099 9.745560663 4.29696876 5.368130607 9.474053298
4.404215517 0 2.565301418 5.558084638 1.282650709 1.092808742 5.091093069
4.563020099 2.565301418 0 5.663139803 1.282650709 3.421088992 5.916486244
9.745560663 5.558084638 5.663139803 0 5.46228302 5.018906402 1.589630079
4.29696876 1.282650709 1.282650709 5.46228302 0 2.191768139 5.368130607
5.368130607 1.092808742 3.421088992 5.018906402 2.191768139 0 4.29696876
9.474053298 5.091093069 5.916486244 1.589630079 5.368130607 4.29696876 0
0 4.047075384 4.854301558 9.46278144 4.239625158 4.848145407 8.836568788
4.047075384 0 2.826382785 5.674846919 1.413191392 0.9936080858 4.817837726
4.854301558 2.826382785 0 5.16622791 1.413191392 3.571107442 5.271782169
9.46278144 5.674846919 5.16622791 0 5.239254648 5.367711376 1.823155142
4.239625158 1.413191392 1.413191392 5.239254648 0 2.20746977 4.848145407
4.848145407 0.9936080858 3.571107442 5.367711376 2.20746977 0 4.239625158
8.836568788 4.817837726 5.271782169 1.823155142 4.848145407 4.239625158 0
END DATA.
";

/// Three matrices of one direction each, at 0, 60 and 120 degrees, with weights 1.5, 1.2, 2.
const ONE_DIRECTION: &str = "DATA LIST FREE /s1 TO s7.
BEGIN DATA
0 3.674234614 1.224744871 6.123724357 2.449489743 4.898979486 7.348469228
3.674234614 0 2.449489743 2.449489743 1.224744871 1.224744871 3.674234614
1.224744871 2.449489743 0 4.898979486 1.224744871 3.674234614 6.123724357
6.123724357 2.449489743 4.898979486 0 3.674234614 1.224744871 1.224744871
2.449489743 1.224744871 1.224744871 3.674234614 0 2.449489743 4.898979486
4.898979486 1.224744871 3.674234614 1.224744871 2.449489743 0 2.449489743
7.348469228 3.674234614 6.123724357 1.224744871 4.898979486 2.449489743 0
0 2.591850971 4.34245575 7.482029278 3.46715336 2.665231879 6.132385239
2.591850971 0 1.750604779 4.890178307 0.8753023896 0.07338090848 3.540534269
4.34245575 1.750604779 0 3.139573528 0.8753023896 1.677223871 1.789929489
7.482029278 4.890178307 3.139573528 0 4.014875918 4.816797399 1.349644039
3.46715336 0.8753023896 0.8753023896 4.014875918 0 0.8019214811 2.665231879
2.665231879 0.07338090848 1.677223871 4.816797399 0.8019214811 0 3.46715336
6.132385239 3.540534269 1.789929489 1.349644039 2.665231879 3.46715336 0
0 0.8965754722 4.191872704 2.588190451 1.647648616 2.216054689 0.5684060729
0.8965754722 0 5.088448177 3.484765923 2.544224088 1.319479217 0.3281693992
4.191872704 5.088448177 0 1.603682253 2.544224088 6.407927393 4.760278777
2.588190451 3.484765923 1.603682253 0 0.9405418349 4.80424514 3.156596524
1.647648616 2.544224088 2.544224088 0.9405418349 0 3.863703305 2.216054689
2.216054689 1.319479217 6.407927393 4.80424514 3.863703305 0 1.647648616
0.5684060729 0.3281693992 4.760278777 3.156596524 2.216054689 1.647648616 0
END DATA.
";

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

const TIGHT: &str = "CONVERGE(0) ITER(3000) STRESSMIN(0)";

fn command(model: &str, extra: &str) -> String {
    format!(
        "ALSCAL VARIABLES=s1 TO s7 /LEVEL=RATIO /MODEL={model} /CRITERIA=DIMENS(2) {extra} {TIGHT}."
    )
}

#[test]
fn matrices_that_turn_the_axes_are_fitted_by_the_generalized_model_only() {
    let gem = run(FULL, &command("GEMSCAL", ""));
    assert!(final_sstress(&gem) < 1e-4, "{}", final_sstress(&gem));
    let indscal = run(FULL, &command("INDSCAL", ""));
    assert!(
        final_sstress(&indscal) > 0.005,
        "{}",
        final_sstress(&indscal)
    );
}

#[test]
fn the_metrics_are_those_the_data_were_made_with() {
    let tables = run(FULL, &command("GEMSCAL", ""));
    // the group space may be turned and stretched, which changes every metric alike, and the
    // disparities of every matrix are rescaled on their own: what stays is the ratio of the
    // generalized eigenvalues of a metric against the first one, 2 : 0.5 for the second matrix
    // and 1.8 : 0.6 for the third (the first is the plane as it is)
    let w = &table(&tables, "Subject Weights").rows;
    let metric = |k: usize| -> [[f64; 2]; 2] {
        let t = &table(&tables, &format!("Subject Directions: Matrix {}", k + 1)).rows;
        let mut m = [[0.0; 2]; 2];
        for a in 0..2 {
            for b in 0..2 {
                m[a][b] = (0..2)
                    .map(|d| number(&w[k][d + 1]) * number(&t[a][d + 1]) * number(&t[b][d + 1]))
                    .sum();
            }
        }
        m
    };
    let m1 = metric(0);
    // det(M - lambda M1) = 0, a quadratic in lambda
    let roots = |m: [[f64; 2]; 2]| -> [f64; 2] {
        let a = m1[0][0] * m1[1][1] - m1[0][1] * m1[0][1];
        let b = -(m[0][0] * m1[1][1] + m[1][1] * m1[0][0] - 2.0 * m[0][1] * m1[0][1]);
        let c = m[0][0] * m[1][1] - m[0][1] * m[0][1];
        let d = (b * b - 4.0 * a * c).sqrt();
        let (r1, r2) = ((-b + d) / (2.0 * a), (-b - d) / (2.0 * a));
        [r1.max(r2), r1.min(r2)]
    };
    let second = roots(metric(1));
    let third = roots(metric(2));
    assert!((second[0] / second[1] - 4.0).abs() < 2e-3, "{second:?}");
    assert!((third[0] / third[1] - 3.0).abs() < 2e-3, "{third:?}");
    // the directions are unit vectors, orthogonal for a full metric
    for k in 1..=3 {
        let t = table(&tables, &format!("Subject Directions: Matrix {k}"));
        let col = |b: usize| -> Vec<f64> { t.rows.iter().map(|r| number(&r[b + 1])).collect() };
        let dot = |a: &[f64], b: &[f64]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
        let (c0, c1) = (col(0), col(1));
        assert!((dot(&c0, &c0) - 1.0).abs() < 1e-9);
        assert!((dot(&c1, &c1) - 1.0).abs() < 1e-9);
        assert!(dot(&c0, &c1).abs() < 1e-9);
    }
}

#[test]
fn matrices_of_one_direction_are_fitted_with_directions_of_one() {
    let tables = run(
        ONE_DIRECTION,
        &command("GEMSCAL", "/CRITERIA=DIRECTIONS(1)"),
    );
    assert!(final_sstress(&tables) < 1e-3, "{}", final_sstress(&tables));
    let w = &table(&tables, "Subject Weights");
    assert_eq!(w.columns, ["Matrix", "Direction 1"]);
}

#[test]
fn the_header_the_outfile_and_the_file_know_the_directions() {
    let tables = run(
        &format!("{FULL}DATASET DECLARE conf.\n"),
        &format!(
            "{} /OUTFILE=conf /PRINT=HEADER.\nALSCAL VARIABLES=s1 TO s7 /LEVEL=RATIO /MODEL=GEMSCAL /FILE=conf /CRITERIA=DIMENS(2) CONVERGE(0) ITER(2) STRESSMIN(0).",
            command("GEMSCAL", "").trim_end_matches('.')
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
        Some(Cell::Text("Generalized Euclidean (GEMSCAL)".into()))
    );
    let histories: Vec<&Table> = tables
        .iter()
        .filter(|t| t.title == "Iteration History")
        .collect();
    assert_eq!(histories.len(), 2);
    // the second run starts from the coordinates and the weights of the first, its directions
    // lost (they are not read): the coordinates alone give a fit that the metrics then repair
    assert!(number(&histories[1].rows[1][1]) < 0.2);
}

#[test]
fn the_model_is_checked() {
    for (syntax, wanted) in [
        (
            "ALSCAL VARIABLES=s1 TO s7 /MODEL=GEMSCAL /CRITERIA=DIMENS(1).",
            "MODEL=GEMSCAL needs at least 2 dimensions",
        ),
        (
            "ALSCAL VARIABLES=s1 TO s7 /MODEL=GEMSCAL /CRITERIA=DIRECTIONS(3).",
            "DIRECTIONS cannot exceed the number of dimensions",
        ),
        (
            "ALSCAL VARIABLES=s1 TO s7 /MODEL=GEMSCAL /CRITERIA=DIRECTIONS(0).",
            "DIRECTIONS takes a whole number from 1 to 6",
        ),
    ] {
        let (_, errors) = parse(&format!("{FULL}{syntax}"));
        assert!(
            errors.iter().any(|e| e.message.contains(wanted)),
            "{syntax}: {errors:?}"
        );
    }
}

#[test]
fn one_matrix_is_the_euclidean_model_and_the_directions_of_the_axes_are_individual_differences() {
    // GEMSCAL on one matrix: a warning and the Euclidean fit
    let one = format!(
        "DATA LIST FREE /s1 TO s7.\nBEGIN DATA\n{}\nEND DATA.\n",
        FULL.lines().skip(2).take(7).collect::<Vec<_>>().join("\n")
    );
    let tables = run(&one, &command("GEMSCAL", ""));
    assert!(tables.iter().all(|t| t.title != "Subject Weights"));
    assert!(final_sstress(&tables) < 1e-4);
}
