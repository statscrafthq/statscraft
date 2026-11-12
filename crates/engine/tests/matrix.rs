//! MATRIX ... END MATRIX: the examples of the manual and synthetic programs with exact expected
//! matrices. The expected values follow from the rules of the language: elementwise operators
//! with scalar expansion, the matrix product and power, the definitions of the functions, and
//! closed forms for the decompositions of small matrices (the symmetric 2 x 2 matrix
//! {a, b; b, a} has the eigenvalues a + b and a - b with the vectors (1, 1) and (-1, 1) over
//! the square root of 2).

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Table};
use std::path::PathBuf;

fn run(source: &str) -> (Session, Vec<Table>, Vec<String>) {
    let mut session = Session::default();
    let result = session
        .run(source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let tables = result.commands.iter().flat_map(|c| c.tables.clone());
    let warnings = result.commands.iter().flat_map(|c| c.warnings.clone());
    (session, tables.collect(), warnings.collect())
}

fn fails(source: &str) -> String {
    let errors = Session::default().run(source, None).unwrap_err();
    errors
        .iter()
        .map(|e| e.text())
        .collect::<Vec<_>>()
        .join(" | ")
}

/// The tables of a matrix program.
fn matrix(body: &str) -> Vec<Table> {
    run(&format!("MATRIX.\n{body}\nEND MATRIX.\n")).1
}

/// The numbers of a PRINT table, by rows.
fn numbers(table: &Table) -> Vec<Vec<f64>> {
    table
        .rows
        .iter()
        .map(|row| {
            row[1..]
                .iter()
                .map(|c| match c {
                    Cell::Number(n) => *n,
                    other => panic!("{}: not a number: {other:?}", table.title),
                })
                .collect()
        })
        .collect()
}

fn texts(table: &Table) -> Vec<Vec<String>> {
    table
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|c| match c {
                    Cell::Text(t) => t.clone(),
                    Cell::Number(n) => n.to_string(),
                    Cell::Empty => String::new(),
                })
                .collect()
        })
        .collect()
}

fn close(actual: &[Vec<f64>], expected: &[&[f64]], what: &str) {
    assert_eq!(actual.len(), expected.len(), "{what}: rows of {actual:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert_eq!(a.len(), e.len(), "{what}: columns of {actual:?}");
        for (x, y) in a.iter().zip(e.iter()) {
            assert!(
                (x - y).abs() <= 1e-10 * y.abs().max(1.0),
                "{what}: {actual:?} != {expected:?}"
            );
        }
    }
}

/// PRINTs each expression and checks the matrices it gives.
fn check(cases: &[(&str, &[&[f64]])]) {
    let body: String = cases
        .iter()
        .map(|(e, _)| format!("PRINT ({e}).\n"))
        .collect();
    let tables = matrix(&body);
    assert_eq!(tables.len(), cases.len());
    for ((expr, expected), table) in cases.iter().zip(&tables) {
        close(&numbers(table), expected, expr);
    }
}

fn folder(case: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-matrix-{}-{case}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn cases(data: &Dataset) -> Vec<String> {
    (0..data.case_count())
        .map(|case| {
            let fields: Vec<String> = data
                .variables()
                .map(|v| match v.values() {
                    Values::Numeric(x) => x.get(case).map_or(".".into(), |x| x.to_string()),
                    Values::String(s) => s.get(case).trim_end().to_string(),
                })
                .collect();
            fields.join(" ")
        })
        .collect()
}

fn names(data: &Dataset) -> Vec<String> {
    data.variable_names().map(str::to_string).collect()
}

const R2: f64 = std::f64::consts::FRAC_1_SQRT_2;

#[test]
fn the_operator_examples_of_the_manual() {
    let tables = matrix(
        [removed]
         PRINT B.\nPRINT C.\nPRINT D.\nPRINT E.\nPRINT F.",
    );
    close(
        &numbers(&tables[0]),
        &[&[5.0, 6.0, 7.0], &[8.0, 9.0, 10.0]],
        "B",
    );
    close(
        &numbers(&tables[1]),
        &[&[1.0, 4.0, 9.0], &[16.0, 25.0, 36.0]],
        "C",
    );
    close(
        &numbers(&tables[2]),
        &[&[2.0, 4.0, 8.0], &[16.0, 32.0, 64.0]],
        "D",
    );
    close(
        &numbers(&tables[3]),
        &[&[1.0, 1.0, 1.0], &[1.0, 0.0, 0.0]],
        "E",
    );
    // C/2 = .5 2 4.5 8 12.5 18 against B = 5 6 7 8 9 10
    close(
        &numbers(&tables[4]),
        &[&[1.0, 1.0, 1.0], &[0.0, 0.0, 0.0]],
        "F",
    );
    assert_eq!(tables[0].title, "B");
    assert_eq!(tables[0].columns, ["", "col1", "col2", "col3"]);
    assert_eq!(texts(&tables[0])[1][0], "row2");
}

#[test]
fn the_compute_examples_of_the_manual() {
    let tables = matrix(
        "COMPUTE PI = 3.14159265.\nCOMPUTE R = {1, 2; 3, 4}.\nCOMPUTE RSQ = R * R.\n\
         COMPUTE EIGENVAL = {2.5; 1; .3}.\nCOMPUTE FLAGS = EIGENVAL >= 1.\n\
         COMPUTE OBS = {1; 2}.\nCOMPUTE PRED = {1.5; 1.5}.\nCOMPUTE ERR = OBS - PRED.\n\
         COMPUTE ESTIM = {OBS, PRED, ERR}.\n\
         PRINT PI.\nPRINT RSQ.\nPRINT FLAGS.\nPRINT ESTIM.",
    );
    close(
        &numbers(&tables[0]),
        &[&["3.14159265".parse::<f64>().unwrap()]],
        "PI",
    );
    close(&numbers(&tables[1]), &[&[7.0, 10.0], &[15.0, 22.0]], "RSQ");
    close(&numbers(&tables[2]), &[&[1.0], &[1.0], &[0.0]], "FLAGS");
    close(
        &numbers(&tables[3]),
        &[&[1.0, 1.5, -0.5], &[2.0, 1.5, 0.5]],
        "ESTIM",
    );
}

#[test]
fn the_first_example_of_the_manual_reads_a_file_and_prints_large_eigenvalues() {
    let dir = folder("eigen");
    // three blocks {2,1;1,2}, {5,2;2,2}, {.5,.1;.1,.5}: eigenvalues 3 1, 6 1, .6 .4
    let rows = [
        "2 1 0 0 0 0",
        "1 2 0 0 0 0",
        "0 0 5 2 0 0",
        "0 0 2 2 0 0",
        "0 0 0 0 .5 .1",
        "0 0 0 0 .1 .5",
    ];
    std::fs::write(dir.join("matrdata.txt"), rows.join("\n")).unwrap();
    let (_, tables, _) = run(&format!(
        "FILE HANDLE MATRDATA /NAME='{}'.\n\
         [removed]
        dir.join("matrdata.txt").display()
    ));
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Eigenvalue:", "Eigenvector:", "Eigenvalue:", "Eigenvector:"]
    );
    close(&numbers(&tables[0]), &[&[6.0]], "first eigenvalue");
    let (a, b) = (2.0 / 5f64.sqrt(), 1.0 / 5f64.sqrt());
    close(
        &numbers(&tables[1]),
        &[&[0.0, 0.0, a, b, 0.0, 0.0]],
        "first eigenvector",
    );
    close(&numbers(&tables[2]), &[&[3.0]], "second eigenvalue");
    close(
        &numbers(&tables[3]),
        &[&[R2, R2, 0.0, 0.0, 0.0, 0.0]],
        "second eigenvector",
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn strings_print_with_a_format_and_labels_name_rows_and_columns() {
    let tables = matrix(
        "COMPUTE LABELS={\"Observe\", \"Predict\", \"Error\"}.\nPRINT LABELS /FORMAT=A7.\n\
         PRINT {1, 2; 3, 4} /RLABELS=first, 'second' /CLABELS='x'.\n\
         COMPUTE NAMES = {'alpha'; 'beta'}.\nPRINT {5, 6} /CNAMES=NAMES /FORMAT=F4.1 /TITLE='Named'.\n\
         PRINT /TITLE='Only a title'.",
    );
    assert_eq!(texts(&tables[0]), [["row1", "Observe", "Predict", "Error"]]);
    assert_eq!(tables[1].columns, ["", "x", ""]);
    assert_eq!(texts(&tables[1])[0][0], "FIRST");
    assert_eq!(texts(&tables[1])[1][0], "second");
    assert_eq!(tables[2].title, "Named");
    assert_eq!(tables[2].columns, ["", "alpha", "beta"]);
    assert_eq!(tables[2].formats[0][1], "F4.1");
    assert_eq!(tables[3].title, "Only a title");
    assert!(tables[3].rows.is_empty());
}

#[test]
fn the_scaling_example_of_the_manual() {
    let tables = matrix(
        "COMPUTE M = {.0000000001357, 2.468, 3690000000}.\n\
         PRINT M /TITLE \"Default format\".\n\
         PRINT M /FORMAT \"E13\" /TITLE \"Explicit exponential format\".",
    );
    // a power of 10 lets the largest value show; small values become 0
    assert_eq!(
        tables[0].caption.as_deref(),
        Some("The values are shown divided by 10**9")
    );
    close(
        &numbers(&tables[0]),
        &[&[1.357e-19, 2.468e-9, 3.69]],
        "scaled",
    );
    assert_eq!(statscraft_engine::cell_text(&tables[0], 0, 1), ".000000000");
    assert_eq!(statscraft_engine::cell_text(&tables[0], 0, 2), ".000000002");
    assert_eq!(tables[1].caption, None);
    close(
        &numbers(&tables[1]),
        &[&[1.357e-10, 2.468, 3.69e9]],
        "exact",
    );
    assert_eq!(tables[1].formats[0][1], "E13");
}

#[test]
fn control_flow_and_the_break_example() {
    let tables = matrix(
        "COMPUTE VEC = {5; 8; 13; 21}.\nCOMPUTE TARGET = 13.\n\
         LOOP LOCATION = 1, NROW(VEC).\n+ DO IF (VEC(LOCATION) = TARGET).\n+     BREAK.\n+ END IF.\nEND LOOP.\n\
         PRINT LOCATION.\n\
         COMPUTE S = 0.\nLOOP I = 1 TO 10 BY 3.\nDO IF I = 4.\nCOMPUTE S = S + 100.\n\
         ELSE IF I > 6.\nCOMPUTE S = S + 1000.\nELSE.\nCOMPUTE S = S + 1.\nEND IF.\nEND LOOP.\n\
         PRINT {S, I}.\n\
         COMPUTE K = 0.\nLOOP IF K < 5.\nCOMPUTE K = K + 2.\nEND LOOP.\nPRINT K.\n\
         COMPUTE N = 0.\nLOOP.\nCOMPUTE N = N + 1.\nEND LOOP IF N >= 7.\nPRINT N.\n\
         COMPUTE D = 0.\nLOOP J = 5 TO 1 BY -2.\nCOMPUTE D = D * 10 + J.\nEND LOOP.\nPRINT D.\n\
         COMPUTE Z = 0.\nLOOP J = 1 TO 0.\nCOMPUTE Z = 1.\nEND LOOP.\nPRINT Z.",
    );
    close(&numbers(&tables[0]), &[&[3.0]], "LOCATION");
    // I = 1, 4, 7, 10: 1 + 100 + 1000 + 1000; the index keeps its last value
    close(&numbers(&tables[1]), &[&[2101.0, 10.0]], "S and I");
    close(&numbers(&tables[2]), &[&[6.0]], "K");
    close(&numbers(&tables[3]), &[&[7.0]], "N");
    close(&numbers(&tables[4]), &[&[531.0]], "D");
    close(&numbers(&tables[5]), &[&[0.0]], "Z");
}

#[test]
fn every_loop_stops_at_mxloops_with_a_warning() {
    let (_, tables, warnings) = run(
        "MATRIX.\nCOMPUTE N = 0.\nLOOP I = 1 TO 100.\nCOMPUTE N = N + 1.\nEND LOOP.\nPRINT {N, I}.\nEND MATRIX.",
    );
    close(&numbers(&tables[0]), &[&[40.0, 40.0]], "N and I");
    assert_eq!(
        warnings,
        ["the LOOP on line 3 stopped after 40 iterations, the limit SET MXLOOPS gives"]
    );
    let (_, tables, warnings) = run(
        "SET MXLOOPS=200.\nMATRIX.\nCOMPUTE N = 0.\nLOOP I = 1 TO 100.\nCOMPUTE N = N + 1.\nEND LOOP.\nPRINT N.\nEND MATRIX.",
    );
    close(&numbers(&tables[0]), &[&[100.0]], "N");
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn indexes_take_and_set_parts() {
    let tables = matrix(
        "COMPUTE M = MAKE(3, 3, 0).\nCOMPUTE M(2, :) = {1, 2, 3}.\nCOMPUTE M(:, 3) = 9.\n\
         COMPUTE M({1, 3}, {1, 2}) = {7, 8; 5, 6}.\nPRINT M.\n\
         CALL SETDIAG(M, {-1, -2}).\nPRINT M.\nCALL SETDIAG(M, 0).\nPRINT DIAG(M).\n\
         COMPUTE V = {10, 20, 30, 40}.\nPRINT V(2:3).\nPRINT V({4, 1}).\nCOMPUTE W = T(V).\n\
         PRINT W(2:3).\nCOMPUTE W(4) = -1.\nPRINT T(W).\nPRINT M(2:3, :).",
    );
    close(
        &numbers(&tables[0]),
        &[&[7.0, 8.0, 9.0], &[1.0, 2.0, 9.0], &[5.0, 6.0, 9.0]],
        "M",
    );
    close(
        &numbers(&tables[1]),
        &[&[-1.0, 8.0, 9.0], &[1.0, -2.0, 9.0], &[5.0, 6.0, 9.0]],
        "SETDIAG",
    );
    close(
        &numbers(&tables[2]),
        &[&[0.0], &[0.0], &[0.0]],
        "SETDIAG scalar",
    );
    close(&numbers(&tables[3]), &[&[20.0, 30.0]], "row vector part");
    close(&numbers(&tables[4]), &[&[40.0, 10.0]], "reordered");
    close(
        &numbers(&tables[5]),
        &[&[20.0], &[30.0]],
        "column vector part",
    );
    close(
        &numbers(&tables[6]),
        &[&[10.0, 20.0, 30.0, -1.0]],
        "set one element",
    );
    close(
        &numbers(&tables[7]),
        &[&[1.0, 0.0, 9.0], &[5.0, 6.0, 0.0]],
        "rows",
    );
}

#[test]
fn the_function_library() {
    let s = 2f64.sqrt();
    check(&[
        ("ABS({-1, 2})", &[&[1.0, 2.0]]),
        (
            "{ALL({1, 0}), ANY({1, 0}), ALL({1, -2}), ANY({0, 0})}",
            &[&[0.0, 1.0, 1.0, 0.0]],
        ),
        ("ARSIN(1) * 2 - ARTAN(1) * 4", &[&[0.0]]),
        ("COS(0) + SIN(0)", &[&[1.0]]),
        (
            "BLOCK({1}, {2, 3; 4, 5})",
            &[&[1.0, 0.0, 0.0], &[0.0, 2.0, 3.0], &[0.0, 4.0, 5.0]],
        ),
        ("CHOL({4, 2; 2, 5})", &[&[2.0, 1.0], &[0.0, 2.0]]),
        (
            "{CMAX({1, 5; 3, 2}); CMIN({1, 5; 3, 2}); CSUM({1, 5; 3, 2}); CSSQ({1, 5; 3, 2})}",
            &[&[3.0, 5.0], &[1.0, 2.0], &[4.0, 7.0], &[10.0, 29.0]],
        ),
        (
            "{RMAX({1, 5; 3, 2}), RMIN({1, 5; 3, 2}), RSUM({1, 5; 3, 2}), RSSQ({1, 5; 3, 2})}",
            &[&[5.0, 1.0, 6.0, 26.0], &[3.0, 2.0, 5.0, 13.0]],
        ),
        (
            "{MMAX({1, 5; 3, 2}), MMIN({1, 5; 3, 2}), MSUM({1, 5; 3, 2}), MSSQ({1, 5; 3, 2})}",
            &[&[5.0, 1.0, 11.0, 39.0]],
        ),
        (
            "DESIGN({3, 5; 1, 5; 3, 5; 2, 5})",
            &[
                &[0.0, 0.0, 1.0],
                &[1.0, 0.0, 0.0],
                &[0.0, 0.0, 1.0],
                &[0.0, 1.0, 0.0],
            ],
        ),
        ("{DET({1, 2; 3, 4}), DET({2, 4; 1, 2})}", &[&[-2.0, 0.0]]),
        ("DIAG({1, 2, 3; 4, 5, 6})", &[&[1.0], &[5.0]]),
        ("EVAL({2, 1; 1, 2})", &[&[3.0], &[1.0]]),
        ("EXP(0) + LN(1) + LG10(100)", &[&[3.0]]),
        (
            "GINV({1, 2; 3, 4; 5, 6})",
            &[
                &[-4.0 / 3.0, -1.0 / 3.0, 2.0 / 3.0],
                &[13.0 / 12.0, 1.0 / 3.0, -5.0 / 12.0],
            ],
        ),
        (
            "GRADE({3, 1, 3; 2, 5, 1})",
            &[&[4.0, 1.0, 5.0], &[3.0, 6.0, 2.0]],
        ),
        (
            "RNKORDER({3, 1, 3; 2, 5, 1})",
            &[&[4.5, 1.5, 4.5], &[3.0, 6.0, 1.5]],
        ),
        ("GSCH({3, 4, 1; 4, -3, 1})", &[&[0.6, 0.8], &[0.8, -0.6]]),
        ("IDENT(2, 3)", &[&[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0]]),
        ("IDENT(2)", &[&[1.0, 0.0], &[0.0, 1.0]]),
        ("INV({4, 7; 2, 6})", &[&[0.6, -0.7], &[-0.2, 0.4]]),
        ("KRONEKER({1, 2}, {1; 10})", &[&[1.0, 2.0], &[10.0, 20.0]]),
        (
            "MAGIC(3)",
            &[&[8.0, 1.0, 6.0], &[3.0, 5.0, 7.0], &[4.0, 9.0, 2.0]],
        ),
        ("MAKE(2, 2, 7)", &[&[7.0, 7.0], &[7.0, 7.0]]),
        ("MDIAG({1, 2})", &[&[1.0, 0.0], &[0.0, 2.0]]),
        ("MOD({-7, 7, 7.5}, 3)", &[&[-1.0, 1.0, 1.5]]),
        ("{NCOL({1, 2, 3}), NROW({1, 2, 3})}", &[&[3.0, 1.0]]),
        (
            "{RANK({1, 2; 2, 4}), RANK(IDENT(3)), RANK({1, 2, 3; 4, 5, 6})}",
            &[&[1.0, 3.0, 2.0]],
        ),
        (
            "RESHAPE({1:6}, 3, 2)",
            &[&[1.0, 2.0], &[3.0, 4.0], &[5.0, 6.0]],
        ),
        (
            "RND({-2.5, -1.4, 0.5, 1.5, 2.6})",
            &[&[-3.0, -1.0, 1.0, 2.0, 3.0]],
        ),
        ("TRUNC({-2.7, 2.7})", &[&[-2.0, 2.0]]),
        ("SOLVE({2, 1; 1, 3}, {3; 5})", &[&[0.8], &[1.4]]),
        ("SQRT({4, 2})", &[&[2.0, s]]),
        ("SSCP({1, 2; 3, 4})", &[&[10.0, 14.0], &[14.0, 20.0]]),
        ("SVAL({3, 0; 0, -4; 0, 0})", &[&[4.0], &[3.0]]),
        (
            "SWEEP({4, 2, 1; 2, 5, 3; 1, 3, 6}, 2)",
            &[&[3.2, -0.4, -0.2], &[0.4, 0.2, 0.6], &[-0.2, -0.6, 4.2]],
        ),
        ("SWEEP({0, 2; 3, 4}, 1)", &[&[0.0, 0.0], &[0.0, 4.0]]),
        ("TRACE({1, 2; 3, 4})", &[&[5.0]]),
        ("TRANSPOS({1, 2})", &[&[1.0], &[2.0]]),
        ("{1, 2; 3, 4} ** 2", &[&[7.0, 10.0], &[15.0, 22.0]]),
        ("{1, 2; 3, 4} ** -1", &[&[-2.0, 1.0], &[1.5, -0.5]]),
        ("{1, 2; 3, 4} ** 0", &[&[1.0, 0.0], &[0.0, 1.0]]),
        ("-2 ** 2", &[&[4.0]]),
        ("{1, 2; 3, 4} * {5; 6}", &[&[17.0], &[39.0]]),
        ("{1, 2} / {4, 8}", &[&[0.25, 0.25]]),
        (
            "{1:7:2; 2:11:3}",
            &[&[1.0, 3.0, 5.0, 7.0], &[2.0, 5.0, 8.0, 11.0]],
        ),
        ("{5:1:-2, 1.9:3.2}", &[&[5.0, 3.0, 1.0, 1.0, 2.0, 3.0]]),
        ("NOT {1, 0, -1}", &[&[0.0, 1.0, 1.0]]),
        ("{1, 0, 1} XOR {1, 1, 0}", &[&[0.0, 1.0, 1.0]]),
        ("{1, 2} <> {1, 3} OR 0", &[&[0.0, 1.0]]),
        ("CDFNORM(0) + PROBIT(.5)", &[&[0.5]]),
        ("CDF.UNIFORM({0, 1, 2}, 0, 4)", &[&[0.0, 0.25, 0.5]]),
        (
            "PDF.BINOM({0, 1, 2}, 2, {.5, .5, .5})",
            &[&[0.25, 0.5, 0.25]],
        ),
        ("IDF.EXP(1 - EXP(-2), 1)", &[&[2.0]]),
        (
            "CHICDF(2, 2) - CDF.CHISQ(2, 2) + TCDF(0, 3) + FCDF(0, 1, 1)",
            &[&[0.5]],
        ),
    ]);
}

#[test]
fn call_eigen_and_svd() {
    let tables = matrix(
        "CALL EIGEN({3, 1; 1, 3}, V, E).\nPRINT V.\nPRINT E.\n\
         CALL SVD({3, 0; 0, -4; 0, 0}, U, Q, W).\nPRINT U.\nPRINT Q.\nPRINT W.\nPRINT (U * Q * T(W)).",
    );
    close(
        &numbers(&tables[0]),
        &[&[R2, -R2], &[R2, R2]],
        "eigenvectors",
    );
    close(&numbers(&tables[1]), &[&[4.0], &[2.0]], "eigenvalues");
    // U is 3 x 3, Q has the shape of the matrix, V is 2 x 2
    assert_eq!(numbers(&tables[2]).len(), 3);
    close(
        &numbers(&tables[3]),
        &[&[4.0, 0.0], &[0.0, 3.0], &[0.0, 0.0]],
        "Q",
    );
    close(&numbers(&tables[4]), &[&[0.0, 1.0], &[1.0, 0.0]], "V");
    close(
        &numbers(&tables[5]),
        &[&[3.0, 0.0], &[0.0, -4.0], &[0.0, 0.0]],
        "U Q T(V)",
    );
}

#[test]
fn random_matrices_follow_set_seed() {
    let (session, tables, _) = run(
        "SET SEED=12345.\nMATRIX.\nPRINT UNIFORM(1, 3).\nEND MATRIX.\n\
         SET SEED=12345.\nINPUT PROGRAM.\nLOOP #i = 1 TO 3.\nCOMPUTE k = #i.\nEND CASE.\nEND LOOP.\nEND FILE.\nEND INPUT PROGRAM.\n\
         COMPUTE u = UNIFORM(1).\nEXECUTE.",
    );
    let drawn = numbers(&tables[0]);
    let data = session.dataset().unwrap();
    let Values::Numeric(u) = data.variable("u").unwrap().values() else {
        panic!();
    };
    let u: Vec<f64> = u.iter().map(|v| v.unwrap()).collect();
    // the matrix draws in the same order as COMPUTE does case by case
    assert_eq!(drawn[0], u);
    let tables = matrix(
        "COMPUTE N = NORMAL(200, 50).\nPRINT {NROW(N), NCOL(N), ABS(MSUM(N) / 10000) < .05}.\nPRINT (UNIFORM({1, 2}) < {1, 2}).",
    );
    close(&numbers(&tables[0]), &[&[200.0, 50.0, 1.0]], "NORMAL");
    close(&numbers(&tables[1]), &[&[1.0, 1.0]], "UNIFORM(max)");
}

#[test]
fn get_reads_cases_as_rows_with_the_missing_value_rules() {
    let data = "DATA LIST LIST /test1 test2 test3 (F8) name (A4).\nBEGIN DATA\n1 2 3 a\n4 5 6 b\n7 . 9 c\n10 11 12 d\nEND DATA.\n\
                MISSING VALUES test3 (12).\n";
    let (_, tables, warnings) = run(&format!(
        "{data}MATRIX.\nGET SCORES /VARIABLES = TEST1,TEST2,TEST3 /NAMES = VARNAMES /MISSING = ACCEPT /SYSMIS = -1.0.\n\
         PRINT SCORES /CNAMES=VARNAMES.\nGET B /VARIABLES=test1 TO test3 /MISSING=OMIT.\nPRINT B.\n\
         GET C /FILE=* /MISSING=99.\nPRINT C.\nGET D /MISSING=ACCEPT /SYSMIS=OMIT.\nPRINT D.\n\
         COMPUTE E = MAKE(4, 3, 0).\nGET E(:, 2) /VARIABLES=test2 /MISSING=0.\nPRINT E.\nEND MATRIX."
    ));
    close(
        &numbers(&tables[0]),
        &[
            &[1.0, 2.0, 3.0],
            &[4.0, 5.0, 6.0],
            &[7.0, -1.0, 9.0],
            &[10.0, 11.0, 12.0],
        ],
        "ACCEPT",
    );
    assert_eq!(tables[0].columns, ["", "test1", "test2", "test3"]);
    close(
        &numbers(&tables[1]),
        &[&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]],
        "OMIT",
    );
    close(
        &numbers(&tables[2]),
        &[
            &[1.0, 2.0, 3.0],
            &[4.0, 5.0, 6.0],
            &[7.0, 99.0, 9.0],
            &[10.0, 11.0, 99.0],
        ],
        "value",
    );
    close(
        &numbers(&tables[3]),
        &[&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0], &[10.0, 11.0, 12.0]],
        "SYSMIS=OMIT",
    );
    close(
        &numbers(&tables[4]),
        &[
            &[0.0, 2.0, 0.0],
            &[0.0, 5.0, 0.0],
            &[0.0, 0.0, 0.0],
            &[0.0, 11.0, 0.0],
        ],
        "part",
    );
    assert!(
        warnings
            .iter()
            .all(|w| w == "GET skips the string variable name"),
        "{warnings:?}"
    );
    assert_eq!(warnings.len(), 2);
    let error = fails(&format!(
        "{data}MATRIX.\nCOMPUTE X = 1.\nGET A /VARIABLES=test1 test2.\nEND MATRIX."
    ));
    assert_eq!(
        error,
        "line 11, column 1: MATRIX: test2 is system-missing in case 3; say how to treat it with MISSING=OMIT or a value"
    );
}

#[test]
fn save_writes_cases_to_the_active_dataset_and_files() {
    let dir = folder("save");
    let (session, _, warnings) = run(&format!(
        "CD '{}'.\nMATRIX.\nCOMPUTE A = {{1; 2}}.\nCOMPUTE B = {{3; 4}}.\nCOMPUTE X = {{5, 6, 7; 8, 9, 10}}.\n\
         COMPUTE Y = {{11, 12; 13, 14}}.\nSAVE {{A,B,X,Y}} /OUTFILE=* /VARIABLES = A,B,X1 TO X3,Y1,Y2.\n\
         SAVE {{0, 0, 0, 0, 0, 0, 0}}.\n\
         COMPUTE LAB = {{'one'; 'two'}}.\nCOMPUTE N = {{'label', 'v'}}.\n\
         SAVE {{LAB, {{10; 20}}}} /OUTFILE='str.sav' /NAMES=N /STRINGS=label.\n\
         SAVE {{1, 2}} /OUTFILE='both.sav' /VARIABLES=p /NAMES=N.\nEND MATRIX.",
        dir.display()
    ));
    let data = session.dataset().unwrap();
    assert_eq!(names(data), ["A", "B", "X1", "X2", "X3", "Y1", "Y2"]);
    assert_eq!(
        cases(data),
        ["1 3 5 6 7 11 12", "2 4 8 9 10 13 14", "0 0 0 0 0 0 0"]
    );
    let strings = Dataset::read_sav(dir.join("str.sav")).unwrap();
    assert_eq!(cases(&strings), ["one 10", "two 20"]);
    assert!(strings.variable("label").unwrap().is_string());
    // VARIABLES wins over NAMES; names it does not give are COLn
    let both = Dataset::read_sav(dir.join("both.sav")).unwrap();
    assert_eq!(names(&both), ["p", "COL2"]);
    assert_eq!(
        warnings,
        ["SAVE has both VARIABLES and NAMES; VARIABLES gives the names"]
    );
    let error = fails("MATRIX.\nSAVE {1, 2} /OUTFILE=*.\nSAVE {1, 2, 3}.\nEND MATRIX.");
    assert_eq!(
        error,
        "line 3, column 1: MATRIX: SAVE adds 3 columns to a file of 2 variables"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn msave_writes_a_matrix_file_that_mget_and_factor_read() {
    let dir = folder("msave");
    let (session, tables, _) = run(&format!(
        "CD '{}'.\nMATRIX.\n\
         COMPUTE M = {{1, 2, 3}}.\nCOMPUTE S = {{1, 1, 1}}.\nCOMPUTE C = {{1, .5, .5; .5, 1, .5; .5, .5, 1}}.\n\
         MSAVE M /TYPE=MEAN /OUTFILE=CORRMAT /VARIABLES=V1 TO V3.\nMSAVE S /TYPE STDDEV.\n\
         MSAVE MAKE(1,3,24) /TYPE N.\nMSAVE C /TYPE CORR.\nEND MATRIX.\n\
         GET FILE=CORRMAT.\nFACTOR /MATRIX=IN(COR=*) /PRINT=INITIAL EXTRACTION /CRITERIA=FACTORS(1).\n\
         MATRIX.\nMGET /FILE=CORRMAT /TYPE=CORR MEAN.\nPRINT CR.\nPRINT MN.\nEND MATRIX.",
        dir.display()
    ));
    let data = Dataset::read_sav(dir.join("CORRMAT")).unwrap();
    assert_eq!(names(&data), ["ROWTYPE_", "VARNAME_", "V1", "V2", "V3"]);
    assert_eq!(
        cases(&data),
        [
            "MEAN  1 2 3",
            "STDDEV  1 1 1",
            "N  24 24 24",
            "CORR V1 1 0.5 0.5",
            "CORR V2 0.5 1 0.5",
            "CORR V3 0.5 0.5 1"
        ]
    );
    drop(session);
    let created = tables
        .iter()
        .find(|t| t.title == "Matrix variables created by MGET")
        .unwrap();
    assert_eq!(texts(created), [["MN", "1", "3"], ["CR", "3", "3"]]);
    let cr = tables.iter().find(|t| t.title == "CR").unwrap();
    close(
        &numbers(cr),
        &[&[1.0, 0.5, 0.5], &[0.5, 1.0, 0.5], &[0.5, 0.5, 1.0]],
        "CR",
    );
    // a correlation matrix with every r = .5 has the first eigenvalue 1 + 2r = 2
    assert!(
        tables
            .iter()
            .any(|t| t.title.contains("Total Variance") || t.title.contains("Communalities"))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn msave_with_factors_and_mget_names_by_cell_and_split() {
    let dir = folder("factors");
    let (_, tables, warnings) = run(&format!(
        "CD '{}'.\nMATRIX.\n\
         MSAVE {{1, 2}} /TYPE=MEAN /OUTFILE='f.sav' /VARIABLES=V1, V2 /FNAMES=SEX, GROUP /FACTOR={{1,1}}.\n\
         MSAVE {{3, 4}} /TYPE=MEAN /FACTOR={{1,2}}.\nMSAVE {{5, 6}} /TYPE=MEAN /FACTOR={{2,1}}.\n\
         MSAVE {{1, .2; .2, 1}} /TYPE=CORR.\nEND MATRIX.\n\
         GET FILE='f.sav'.\nLIST.\n\
         MATRIX DATA VARIABLES=s ROWTYPE_ a b /SPLIT=s.\nBEGIN DATA\n1 MEAN 1 2\n1 N 5 5\n2 MEAN 3 4\n2 N 6 6\nEND DATA.\n\
         MATRIX.\nCOMPUTE MNS2 = 0.\nMGET.\nPRINT MNS1.\nPRINT NCS2.\nPRINT MNS2.\nEND MATRIX.\n\
         MATRIX.\nMGET /FILE='f.sav'.\nPRINT MNF3.\nPRINT CR.\nEND MATRIX.",
        dir.display()
    ));
    let list = tables.iter().find(|t| t.title == "List").unwrap();
    assert_eq!(
        list.columns,
        ["ROWTYPE_", "SEX", "GROUP", "VARNAME_", "V1", "V2"]
    );
    assert_eq!(list.rows.len(), 5);
    assert_eq!(texts(list)[3][1], ".");
    close(
        &numbers(tables.iter().find(|t| t.title == "MNS1").unwrap()),
        &[&[1.0, 2.0]],
        "MNS1",
    );
    close(
        &numbers(tables.iter().find(|t| t.title == "NCS2").unwrap()),
        &[&[6.0, 6.0]],
        "NCS2",
    );
    // MGET does not replace a variable that exists
    close(
        &numbers(tables.iter().find(|t| t.title == "MNS2").unwrap()),
        &[&[0.0]],
        "MNS2",
    );
    assert_eq!(
        warnings,
        ["MGET does not replace the existing matrix variable MNS2"]
    );
    close(
        &numbers(tables.iter().find(|t| t.title == "MNF3").unwrap()),
        &[&[5.0, 6.0]],
        "MNF3",
    );
    close(
        &numbers(tables.iter().find(|t| t.title == "CR").unwrap()),
        &[&[1.0, 0.2], &[0.2, 1.0]],
        "pooled CR",
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn read_and_write_text_files() {
    let dir = folder("text");
    let (_, tables, warnings) = run(&format!(
        "CD '{}'.\nMATRIX.\n\
         WRITE {{1.5, 2, 3; 4, 5, 6}} /OUTFILE='w.txt' /FIELD=1 TO 30 BY 10 /FORMAT=F10.2.\n\
         WRITE {{1, 2; 3, 4}} /FIELD=3 TO 20.\n\
         WRITE {{1, 2, 3; 4, 5, 6; 7, 8, 9}} /FIELD=1 TO 9 BY 3 /MODE=TRIANGULAR.\n\
         WRITE {{1, 2, 3, 4, 5}} /FIELD=1 TO 4 /FORMAT='2F'.\n\
         WRITE {{7}} /FIELD=1 TO 3 /HOLD.\nWRITE {{8}} /FIELD=5 TO 7.\n\
         WRITE {{'ab', 'cd'}} /OUTFILE='s.txt' /FIELD=1 TO 6 /FORMAT=A3.\n\
         WRITE {{0.1, -2.5e-7, 1e20}} /FIELD=1 TO 80.\n\
         END MATRIX.\n\
         MATRIX.\n\
         READ R /FILE='w.txt' /FIELD=1 TO 30 BY 10 /SIZE={{2, 3}}.\nPRINT R.\n\
         READ Q /FIELD=3 TO 20 /SIZE=2.\nPRINT Q.\n\
         READ T3 /FIELD=1 TO 9 BY 3 /SIZE={{3, 3}} /MODE=SYMMETRIC.\nPRINT T3.\n\
         READ F /FIELD=1 TO 4 /FORMAT='2F' /SIZE={{1, 5}}.\nPRINT F.\n\
         READ H /FIELD=5 TO 7 /SIZE=1 /REREAD.\nPRINT H.\nPRINT EOF('w.txt').\n\
         READ S /FILE='s.txt' /FIELD=1 TO 6 /FORMAT=A3 /SIZE={{1, 2}}.\nPRINT S /FORMAT=A3.\n\
         READ G /FIELD=1 TO 80 /SIZE={{1, 3}}.\nPRINT G /FORMAT=E10.2.\nPRINT EOF('s.txt').\n\
         READ X /FIELD=1 TO 80 /SIZE={{1, 3}}.\n\
         END MATRIX.",
        dir.display()
    ));
    let written = std::fs::read_to_string(dir.join("w.txt")).unwrap();
    assert_eq!(
        written,
        "      1.50      2.00      3.00\n      4.00      5.00      6.00\n  1 2\n  3 4\n  1\n  4  5\n  7  8  9\n 1 2\n 3 4\n 5\n7   8\n"
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("s.txt")).unwrap(),
        "ab cd \n0.1 -0.00000025 100000000000000000000\n"
    );
    close(
        &numbers(&tables[0]),
        &[&[1.5, 2.0, 3.0], &[4.0, 5.0, 6.0]],
        "R",
    );
    close(&numbers(&tables[1]), &[&[1.0], &[3.0]], "Q");
    close(
        &numbers(&tables[2]),
        &[&[1.0, 4.0, 7.0], &[4.0, 5.0, 8.0], &[7.0, 8.0, 9.0]],
        "symmetric",
    );
    close(
        &numbers(&tables[3]),
        &[&[1.0, 2.0, 3.0, 4.0, 5.0]],
        "repeat factor",
    );
    // REREAD starts with the record of 7 and 8; that record is the last one
    close(&numbers(&tables[4]), &[&[8.0]], "REREAD");
    close(&numbers(&tables[5]), &[&[1.0]], "EOF");
    assert_eq!(texts(&tables[6]), [["row1", "ab", "cd"]]);
    close(
        &numbers(&tables[7]),
        &[&[0.1, -2.5e-7, 1e20]],
        "free numbers",
    );
    close(&numbers(&tables[8]), &[&[1.0]], "EOF of s.txt");
    assert_eq!(
        warnings,
        ["READ reached the end of s.txt before the matrix was full; the rest is missing"]
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn display_and_release() {
    let (_, tables, warnings) = run(
        "MATRIX.\nCOMPUTE A = {1, 2}.\nCOMPUTE b = IDENT(3).\nDISPLAY.\nRELEASE A, C.\nDISPLAY DICTIONARY.\nEND MATRIX.",
    );
    assert_eq!(texts(&tables[0]), [["A", "1", "2"], ["b", "3", "3"]]);
    assert_eq!(
        texts(&tables[1]),
        [["Variables", "2"], ["Elements", "11"], ["Bytes", "88"]]
    );
    assert_eq!(texts(&tables[2]), [["b", "3", "3"]]);
    assert_eq!(warnings, ["RELEASE: the matrix variable C does not exist"]);
}

#[test]
fn errors_stop_the_program_at_their_statement() {
    let cases = [
        (
            "COMPUTE A = {1, 2} + {1; 2}.",
            "line 2, column 13: MATRIX: + needs matrices of the same size or a scalar, but they are 1x2 and 2x1",
        ),
        (
            "PRINT B.",
            "line 2, column 7: MATRIX: the matrix variable B does not exist",
        ),
        (
            "PRINT LN({1, -1}).",
            "line 2, column 7: MATRIX: the elements of LN must be positive, but one is -1",
        ),
        (
            "COMPUTE A = {1, 2}.\nPRINT A(3).",
            "line 3, column 9: MATRIX: index 3 is out of range: A has 2 elements",
        ),
        (
            "COMPUTE A = {1, 2; 3, 4}.\nPRINT A(1).",
            "line 3, column 7: MATRIX: A is a 2x2 matrix and needs two indexes, rows and columns",
        ),
        (
            "COMPUTE A = {1, 2; 3}.",
            "line 2, column 13: MATRIX: the rows of a matrix need the same number of columns, but they have 2 and 1",
        ),
        (
            "COMPUTE A = {{1; 2}, 3}.",
            "line 2, column 22: MATRIX: the parts of a row of a matrix need the same number of rows, but this one has 1 and the first 2",
        ),
        (
            "COMPUTE A = {1, 2} * {3, 4}.",
            "line 2, column 13: MATRIX: * needs as many columns in the first matrix as rows in the second, but they are 1x2 and 1x2",
        ),
        (
            "PRINT INV({1, 2; 2, 4}).",
            "line 2, column 7: MATRIX: INV needs a nonsingular matrix, but this one is singular",
        ),
        (
            "CALL EIGEN({1, 2; 3, 4}, V, E).",
            "line 2, column 12: MATRIX: EIGEN needs a symmetric matrix, but element (2, 1) differs from (1, 2)",
        ),
        (
            "DO IF {1, 1}.\nEND IF.",
            "line 2, column 7: MATRIX: the condition of DO IF must give a scalar, but it gives 1x2",
        ),
        (
            "COMPUTE X(1) = 2.",
            "line 2, column 9: MATRIX: the matrix variable X must exist before a part of it is set",
        ),
        (
            "PRINT CDF.NORMAL(1, 0, -1).",
            "line 2, column 7: MATRIX: CDF.NORMAL(1, 0, -1) is out of range of its parameters",
        ),
        (
            "PRINT MAGIC(2).",
            "line 2, column 7: MATRIX: the first argument of MAGIC must be a whole number of at least 3, got 2",
        ),
        (
            "LOOP I = 1 TO 3 BY 0.\nEND LOOP.",
            "line 2, column 20: MATRIX: the increment of LOOP must not be 0",
        ),
        (
            "GET A.",
            "line 2, column 1: MATRIX: no active dataset: load one with GET FILE first",
        ),
    ];
    for (body, expected) in cases {
        assert_eq!(
            fails(&format!("MATRIX.\n{body}\nEND MATRIX.")),
            expected,
            "{body}"
        );
    }
    // nothing is saved when the program stops
    let mut session = Session::default();
    let failed = session.run(
        "MATRIX.\nSAVE {1} /OUTFILE=*.\nPRINT NOPE.\nEND MATRIX.",
        None,
    );
    assert!(failed.is_err());
    assert!(session.dataset().is_none());
}

#[test]
fn macros_and_abbreviations_reach_the_program() {
    let tables = matrix("COMP A = {1, 2}.\nPRIN (A * 2).\nDISP DIC.");
    close(&numbers(&tables[0]), &[&[2.0, 4.0]], "abbreviated");
    let (_, tables, _) = run(
        "DEFINE !square (m = !TOKENS(1))\nCOMPUTE !m = !m * !m.\n!ENDDEFINE.\nMATRIX.\nCOMPUTE X = {1, 1; 0, 1}.\n!square m=X.\nPRINT X.\nEND MATRIX.",
    );
    close(&numbers(&tables[0]), &[&[1.0, 2.0], &[0.0, 1.0]], "macro");
}
