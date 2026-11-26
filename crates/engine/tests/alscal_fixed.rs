//! ALSCAL /FILE with (FIXED): the coordinates and weights of the file stay as they are and the
//! rest is solved for them. Exact cases: data made from known coordinates and weights, with the
//! true ones fixed, give the others exactly and leave the fixed ones untouched, in the shape the
//! file gave them (no centering, turning or scaling of the output).

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// The places the data were made from.
const PLACES: [[f64; 2]; 7] = [
    [0.0, 0.0],
    [3.0, 1.0],
    [1.0, 4.0],
    [5.0, 5.0],
    [2.0, 2.5],
    [4.0, 0.5],
    [6.0, 3.0],
];

/// Three matrices of the places: the weights of the axes are 1 and 1, 2 and 0.5, 0.6 and 1.8.
const AXES: &str = "DATA LIST FREE /s1 TO s7.
BEGIN DATA
0 3.16227766 4.123105626 7.071067812 3.201562119 4.031128874 6.708203932
3.16227766 0 3.605551275 4.472135955 1.802775638 1.118033989 3.605551275
4.123105626 3.605551275 0 4.123105626 1.802775638 4.609772229 5.099019514
7.071067812 4.472135955 4.123105626 0 3.905124838 4.609772229 2.236067977
3.201562119 1.802775638 1.802775638 3.905124838 0 2.828427125 4.031128874
4.031128874 1.118033989 4.609772229 4.609772229 2.828427125 0 3.201562119
6.708203932 3.605551275 5.099019514 2.236067977 4.031128874 3.201562119 0
0 4.301162634 3.16227766 7.90569415 3.335416016 5.667892024 8.746427842
4.301162634 0 3.535533906 4 1.767766953 1.457737974 4.472135955
3.16227766 3.535533906 0 5.700877125 1.767766953 4.911720676 7.106335202
7.90569415 4 5.700877125 0 4.596194078 3.482097069 2
3.335416016 1.767766953 1.767766953 4.596194078 0 3.16227766 5.667892024
5.667892024 1.457737974 4.911720676 3.482097069 3.16227766 0 3.335416016
8.746427842 4.472135955 7.106335202 2 5.667892024 3.335416016 0
0 2.683281573 5.422176685 7.745966692 3.69459064 3.170173497 6.14817046
2.683281573 0 4.312771731 5.585696018 2.156385865 1.024695077 3.54964787
5.422176685 4.312771731 0 3.376388603 2.156385865 5.239274759 4.098780306
7.745966692 5.585696018 3.376388603 0 4.080441153 6.086871117 2.792848009
3.69459064 2.156385865 2.156385865 4.080441153 0 3.098386677 3.170173497
3.170173497 1.024695077 5.239274759 6.086871117 3.098386677 0 3.69459064
6.14817046 3.54964787 4.098780306 2.792848009 3.170173497 3.69459064 0
END DATA.
";

/// Three matrices of the places seen through metrics that turn the axes.
const TURNED: &str = "DATA LIST FREE /s1 TO s7.
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

/// One matrix of the places with weights of the axes that each place has as the first of a pair:
/// (1.6, 0.4), (0.5, 1.5), (1.2, 0.8), (0.7, 1.3), (1.5, 0.5), (0.6, 1.4), (0.9, 1.1).
const WEIGHTED: &str = "DATA LIST FREE /s1 TO s7.
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

/// Five row places and six column places, rows 0 to 4 of this list and then the columns.
const UNFOLD: &str = "DATA LIST FREE /c1 TO c6.
BEGIN DATA
3.605551275 7.071067812 5 6 2.236067977 5
1.414213562 4.123105626 5.656854249 2.236067977 2 2
3.16227766 3 2 6.403124237 4 2.828427125
3.605551275 1.414213562 6.08276253 4 5 2.236067977
2.236067977 4.472135955 2.236067977 5.830951895 2.236067977 3
END DATA.
";

const ROW_POINTS: [[f64; 2]; 5] = [[0.0, 0.0], [4.0, 1.0], [2.0, 5.0], [6.0, 4.0], [1.0, 3.0]];
const COLUMN_POINTS: [[f64; 2]; 6] = [
    [3.0, 2.0],
    [5.0, 5.0],
    [0.0, 5.0],
    [6.0, 0.0],
    [2.0, 1.0],
    [4.0, 3.0],
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

fn grid(tables: &[Table], title: &str) -> Vec<Vec<f64>> {
    table(tables, title)
        .rows
        .iter()
        .map(|r| r[1..].iter().map(number).collect())
        .collect()
}

/// A configuration/weights file as a dataset: a TYPE_ and the two dimensions.
fn file(name: &str, kind: &str, rows: &[[f64; 2]]) -> String {
    let lines: Vec<String> = rows
        .iter()
        .map(|r| format!("{kind} {} {}", r[0], r[1]))
        .collect();
    format!(
        "DATA LIST FREE /TYPE_ (A8) DIM1 DIM2.\nBEGIN DATA\n{}\nEND DATA.\nDATASET NAME {name}.\n",
        lines.join("\n")
    )
}

const TIGHT: &str = "CONVERGE(0) ITER(3000) STRESSMIN(0)";

fn same(a: &[Vec<f64>], b: &[[f64; 2]], tolerance: f64, what: &str) {
    assert_eq!(a.len(), b.len(), "{what}");
    for (x, y) in a.iter().zip(b) {
        for d in 0..2 {
            assert!(
                (x[d] - y[d]).abs() < tolerance,
                "{what}: {x:?} against {y:?}"
            );
        }
    }
}

#[test]
fn fixed_coordinates_stay_and_the_weights_are_solved_for_them() {
    let config = file("start", "CON", &PLACES);
    let tables = run(
        &format!("{config}{AXES}"),
        &format!(
            "ALSCAL VARIABLES=s1 TO s7 /LEVEL=RATIO /MODEL=INDSCAL /FILE=start CONFIG(FIXED) /CRITERIA=DIMENS(2) {TIGHT}."
        ),
    );
    assert!(final_sstress(&tables) < 1e-6, "{}", final_sstress(&tables));
    // as the file gave them: not centered, turned or scaled
    same(
        &grid(&tables, "Stimulus Coordinates"),
        &PLACES,
        1e-12,
        "coordinates",
    );
    // the weights of the axes against each other: 1 : 1, 2 : 0.5 and 0.6 : 1.8
    let w = grid(&tables, "Subject Weights");
    let ratio = |k: usize| w[k][0] / w[k][1];
    assert!((ratio(0) - 1.0).abs() < 1e-5, "{}", ratio(0));
    assert!((ratio(1) - 4.0).abs() < 1e-4, "{}", ratio(1));
    assert!((ratio(2) - 1.0 / 3.0).abs() < 1e-5, "{}", ratio(2));
}

#[test]
fn fixed_coordinates_that_fit_badly_stay_where_the_file_put_them() {
    let wrong = [
        [0.0, 0.0],
        [1.0, 3.0],
        [3.0, 1.0],
        [2.0, 2.0],
        [5.0, 5.0],
        [4.0, 0.0],
        [1.0, 6.0],
    ];
    let config = file("start", "CON", &wrong);
    let exact = AXES.lines().take(9).collect::<Vec<_>>().join("\n");
    let one = format!("{exact}\nEND DATA.\n");
    let command = |how: &str| {
        format!(
            "ALSCAL VARIABLES=s1 TO s7 /LEVEL=RATIO /FILE=start CONFIG({how}) /CRITERIA=DIMENS(2) {TIGHT}."
        )
    };
    let fixed = run(&format!("{config}{one}"), &command("FIXED"));
    same(
        &grid(&fixed, "Stimulus Coordinates"),
        &wrong,
        1e-12,
        "fixed",
    );
    let initial = run(&format!("{config}{one}"), &command("INITIAL"));
    assert!(
        final_sstress(&initial) < 1e-5,
        "{}",
        final_sstress(&initial)
    );
    assert!(final_sstress(&fixed) > 0.05, "{}", final_sstress(&fixed));
}

#[test]
fn fixed_subject_weights_stay_and_the_coordinates_are_solved() {
    // the true weights, each matrix on a scale of its own (the fit rescales the disparities)
    let weights = [[1.0, 1.0], [4.0, 1.0], [0.6, 1.8]];
    let config = file("start", "SUB", &weights);
    let tables = run(
        &format!("{config}{AXES}"),
        &format!(
            "ALSCAL VARIABLES=s1 TO s7 /LEVEL=RATIO /MODEL=INDSCAL /FILE=start SUBJWGHT(FIXED) /CRITERIA=DIMENS(2) {TIGHT}."
        ),
    );
    assert!(final_sstress(&tables) < 1e-5, "{}", final_sstress(&tables));
    let w = grid(&tables, "Subject Weights");
    for (found, given) in w.iter().zip(&weights) {
        assert!((found[0] - given[0]).abs() < 1e-12 && (found[1] - given[1]).abs() < 1e-12);
    }
}

#[test]
fn fixed_stimulus_weights_stay_and_the_coordinates_are_solved() {
    let v = [
        [1.6, 0.4],
        [0.5, 1.5],
        [1.2, 0.8],
        [0.7, 1.3],
        [1.5, 0.5],
        [0.6, 1.4],
        [0.9, 1.1],
    ];
    let config = file("start", "STI", &v);
    let tables = run(
        &format!("{config}{WEIGHTED}"),
        &format!(
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /LEVEL=RATIO /MODEL=ASCAL /FILE=start STIMWGHT(FIXED) /CRITERIA=DIMENS(2) {TIGHT}."
        ),
    );
    assert!(final_sstress(&tables) < 1e-5, "{}", final_sstress(&tables));
    same(&grid(&tables, "Stimulus Weights"), &v, 1e-12, "weights");
}

#[test]
fn the_rows_of_an_unfolding_can_be_fixed_and_the_columns_found_in_their_frame() {
    let mut rows = ROW_POINTS.to_vec();
    // the columns start somewhere else
    rows.extend(COLUMN_POINTS.iter().map(|c| [c[0] * 0.5 + 1.0, c[1] * 0.3]));
    let lines: Vec<String> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| format!("{} {} {}", if i < 5 { "ROW" } else { "COL" }, r[0], r[1]))
        .collect();
    let config = format!(
        "DATA LIST FREE /TYPE_ (A8) DIM1 DIM2.\nBEGIN DATA\n{}\nEND DATA.\nDATASET NAME start.\n",
        lines.join("\n")
    );
    let tables = run(
        &format!("{config}{UNFOLD}"),
        &format!(
            "ALSCAL VARIABLES=c1 TO c6 /SHAPE=RECTANGULAR /LEVEL=RATIO /FILE=start ROWCONF(FIXED) COLCONF(INITIAL) /CRITERIA=DIMENS(2) {TIGHT}."
        ),
    );
    assert!(final_sstress(&tables) < 1e-5, "{}", final_sstress(&tables));
    same(
        &grid(&tables, "Row Stimulus Coordinates"),
        &ROW_POINTS,
        1e-12,
        "rows",
    );
    // the rows pin the frame, so the columns are where they were made
    same(
        &grid(&tables, "Column Stimulus Coordinates"),
        &COLUMN_POINTS,
        1e-3,
        "columns",
    );
}

#[test]
fn fixed_coordinates_with_the_generalized_model() {
    let config = file("start", "CON", &PLACES);
    let tables = run(
        &format!("{config}{TURNED}"),
        &format!(
            "ALSCAL VARIABLES=s1 TO s7 /LEVEL=RATIO /MODEL=GEMSCAL /FILE=start CONFIG(FIXED) /CRITERIA=DIMENS(2) {TIGHT}."
        ),
    );
    assert!(final_sstress(&tables) < 1e-5, "{}", final_sstress(&tables));
    same(
        &grid(&tables, "Stimulus Coordinates"),
        &PLACES,
        1e-12,
        "coordinates",
    );
}

#[test]
fn what_fixed_needs_is_checked() {
    for (syntax, wanted) in [
        (
            "ALSCAL VARIABLES=s1 TO s7 /FILE=f SUBJWGHT(FIXED).",
            "SUBJWGHT(FIXED) needs MODEL=INDSCAL or AINDS",
        ),
        (
            "ALSCAL VARIABLES=s1 TO s7 /MODEL=GEMSCAL /FILE=f SUBJWGHT(FIXED).",
            "not supported with MODEL=GEMSCAL",
        ),
        (
            "ALSCAL VARIABLES=s1 TO s7 /FILE=f COLCONF(FIXED) /SHAPE=ASYMMETRIC.",
            "COLCONF of FILE needs SHAPE=RECTANGULAR",
        ),
        (
            "ALSCAL VARIABLES=s1 TO s7 /SHAPE=ASYMMETRIC /MODEL=ASCAL /FILE=f CONFIG(FIXED) STIMWGHT(SOLID).",
            "unexpected word SOLID",
        ),
    ] {
        let (_, errors) = parse(&format!("{AXES}{syntax}"));
        assert!(
            errors.iter().any(|e| e.message.contains(wanted)),
            "{syntax}: {errors:?}"
        );
    }
    // a file without the coordinates that are to stay is an error
    let nothing = "DATA LIST FREE /TYPE_ (A8) DIM1 DIM2.\nBEGIN DATA\nSUB 1 1\nEND DATA.\nDATASET NAME start.\n";
    let (commands, errors) = parse(&format!(
        "{nothing}{AXES}ALSCAL VARIABLES=s1 TO s7 /FILE=start CONFIG(FIXED)."
    ));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let failure = commands
        .iter()
        .find_map(|c| engine.execute(&c.command).err());
    assert!(
        failure
            .expect("an error")
            .to_string()
            .contains("configuration rows")
    );
}
