//! TREE with VALIDATION TYPE=SPLITSAMPLE on an invented sample of 150 cases: y (1, 2) and y3 (1 to
//! 3) by x1, x2, o, g, xm (14 missing), z (6 missing), b, v, zv, g4 and gm (6 missing), with
//! tr (1 for two cases in three, the rest are the test sample) and w (a weight of 1 to 3).
//! The tree grown with a test sample must be the tree grown on the training cases alone (the
//! training cases selected, no validation), for every method: any difference means a test case
//! took part in growing. The risk of the test sample is computed here from the saved predictions.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE / y y3 x1 x2 o g xm z r b v zv g4 gm tr w.
BEGIN DATA
1 1 29.4 8 1 2 29.4 6 22.01 1 29.9 4.1 2 2 0 1
1 1 10.5 8 4 4 10.5 2 15.01 1 26.4 5.4 5 4 1 2
2 2 42.5 12 2 4 42.5 8 26.49 1 24.7 8 4 . 1 3
1 1 11.1 7 1 2 11.1 2 16.58 1 30.7 0.5 2 2 0 1
2 2 45.8 8 3 2 45.8 9 25.33 2 35.2 8 2 2 1 2
2 3 52 10 3 4 52 10 26.69 1 32.8 8 5 4 1 3
2 2 43.3 5 2 3 43.3 9 21.66 1 30.2 8 3 3 0 1
1 2 39.9 4 1 4 39.9 . 22.32 1 27.1 1.2 5 4 1 2
2 2 48.3 5 1 3 48.3 10 22.71 2 31.9 8 3 3 1 3
2 3 58.1 5 2 1 58.1 12 25.33 1 11.7 8 1 1 0 1
2 3 53.2 4 2 2 . 11 23.82 2 36 8 2 2 1 2
1 1 14.7 12 2 1 14.7 3 8.8 1 29.4 7.2 1 1 1 3
1 1 13.6 7 4 1 13.6 3 16.28 1 30.6 8.1 1 1 0 1
1 1 24.9 9 1 2 24.9 5 18.72 2 31 9.4 2 2 1 2
1 1 35.4 11 1 1 35.4 7 15.05 1 28.8 2.7 1 1 1 3
1 3 45.2 9 2 4 45.2 9 24.04 1 26.1 9.1 5 4 0 1
1 2 51.6 3 1 1 51.6 10 20.02 1 30.9 5 1 . 1 2
2 3 53.8 8 3 4 . 11 31.17 2 48.9 8 5 4 1 3
1 1 17.7 8 4 3 17.7 4 17.53 1 30.4 6.3 3 3 0 1
1 1 22.6 7 2 2 22.6 5 16.84 1 26.8 7.2 2 2 1 2
2 2 41.8 10 2 2 41.8 8 22.22 2 15.7 8 2 2 1 3
2 3 48.7 3 2 2 . 10 24.67 2 33.1 8 2 2 0 1
2 2 53 9 4 1 53 11 25.39 2 25.6 8 1 1 1 2
1 1 26 3 2 1 26 5 17.75 1 29.3 2.3 1 1 1 3
2 3 55.6 6 1 2 55.6 11 28.79 2 44.4 8 2 2 0 1
1 1 15.9 8 1 2 15.9 3 14.75 1 31.8 8.6 2 2 1 2
1 2 51.9 2 3 3 51.9 10 27.5 1 32.1 8.1 3 3 1 3
1 1 11.2 12 1 1 11.2 2 13.56 1 31.5 6.2 1 1 0 1
2 3 56.2 3 2 3 56.2 11 24.41 2 24.3 8 3 3 1 2
2 1 15.2 3 4 1 15.2 3 15.22 2 27.9 8 1 1 1 3
1 3 55.9 2 4 2 55.9 11 25.68 1 26.6 0.4 2 2 0 1
1 3 57.3 12 1 3 57.3 11 27.89 1 32.3 0.5 3 3 1 2
1 2 56.9 2 1 3 56.9 11 24.84 1 35 6.7 3 3 1 3
2 3 46.9 4 1 4 46.9 9 25.86 2 49 8 5 4 0 1
1 1 41 11 2 3 41 8 22.77 1 31.2 9.7 3 3 1 2
1 1 23.3 9 3 3 23.3 5 20.34 1 29.8 5.4 3 3 1 3
2 2 40.6 7 4 3 40.6 8 24.94 2 15.6 8 3 3 0 1
1 1 49.8 6 1 1 49.8 10 20.93 1 28.3 1.5 1 1 1 2
2 1 10.5 3 1 1 10.5 2 11.16 2 16.9 8 1 1 1 3
1 1 37.9 3 2 1 37.9 8 19.88 2 30.5 5.2 1 . 0 1
2 3 45.9 12 1 4 45.9 9 27.1 2 28.7 8 4 . 1 2
1 1 26.7 2 2 2 26.7 5 16.7 1 30.9 7.3 2 2 1 3
2 2 50.3 10 2 4 50.3 10 29.05 1 14.5 8 4 4 0 1
1 1 16.6 3 3 4 16.6 3 15.09 1 30.4 8.8 5 4 1 2
1 1 15.8 6 1 3 15.8 3 13.16 1 30 3.5 3 3 1 3
1 2 26.8 9 3 2 26.8 5 18.88 2 27.4 3 2 2 0 1
1 3 33 10 4 3 33 7 17.65 1 26 7.7 3 3 1 2
1 1 17.5 2 1 2 . 4 13.22 2 32.8 8 2 2 1 3
2 2 47.8 11 1 4 47.8 10 28.49 2 10.2 8 4 4 0 1
1 2 49.2 2 3 3 49.2 10 25.69 1 28.1 6.2 3 3 1 2
1 1 18.7 9 2 4 18.7 4 17.31 1 29.7 4.9 4 4 1 3
2 2 35 12 4 3 35 7 18.73 2 32.8 8 3 3 0 1
1 1 27.6 8 1 4 27.6 6 20.25 1 28.8 7.4 4 4 1 2
1 1 35.9 1 4 1 . 7 22.64 2 26.5 5.4 1 1 1 3
2 2 38.9 8 1 4 38.9 8 25.58 1 14 8 4 4 0 1
2 2 54 5 2 3 54 11 30.28 2 27.6 8 3 3 1 2
2 2 37.3 8 4 2 37.3 7 22.07 2 29.8 8 2 2 1 3
1 2 46.1 3 4 4 46.1 9 28.94 2 32.4 8.6 5 4 0 1
1 1 16.8 11 4 2 16.8 3 16.57 1 25.9 2.8 2 2 1 2
1 1 47.4 4 3 1 47.4 9 25.93 1 31.6 5.4 1 1 1 3
2 3 51.2 12 3 2 51.2 10 26.06 2 49.6 8 2 2 0 1
1 2 47.7 3 4 4 47.7 10 25.69 1 28.2 8.1 5 4 1 2
1 1 49.5 12 2 1 . 10 21.1 1 28.3 0.9 1 1 1 3
2 3 44.4 12 3 3 . 9 20.7 2 33.7 8 3 3 0 1
1 3 54 11 3 1 54 11 23.13 1 26.3 2.5 1 1 1 2
1 1 25 1 4 1 25 5 19.14 1 31 8.7 1 1 1 3
2 1 20.9 12 2 4 20.9 4 17.15 2 31.4 8 4 4 0 1
1 1 23.9 11 3 1 23.9 5 15.95 2 35.8 2.2 1 1 1 2
2 3 49.9 9 1 2 49.9 10 24.51 1 18.6 8 2 2 1 3
1 3 53.1 5 4 4 53.1 11 29.16 1 32.7 2.2 5 4 0 1
2 2 18 12 3 3 18 4 19.81 2 28.9 8 3 3 1 2
2 3 59.3 12 4 4 59.3 12 29.61 2 18.5 8 5 4 1 3
1 2 45.8 5 3 3 45.8 9 26.07 1 30.9 4.5 3 3 0 1
1 3 50.9 3 3 4 50.9 10 30.27 1 30.8 4.7 5 4 1 2
2 2 44.9 9 4 3 . 9 22.25 2 55.7 8 3 3 1 3
1 1 29.7 6 2 1 29.7 6 14.91 1 31.8 6.8 1 1 0 1
2 2 39.3 7 4 3 39.3 8 25.75 2 25.5 8 3 3 1 2
1 1 49.4 4 1 1 49.4 10 20.08 1 26.6 4.8 1 1 1 3
1 1 12.1 9 2 3 12.1 2 15.92 1 30.8 8.3 3 3 0 1
1 1 47.8 7 2 3 . 10 21.03 1 30.9 9.6 3 3 1 2
1 2 48.5 3 2 2 48.5 10 24.25 1 33.7 5.6 2 2 1 3
1 1 45 3 3 1 45 9 24.7 1 26.4 7.5 1 1 0 1
1 3 32.9 10 3 2 32.9 7 21.62 1 29.7 8.3 2 2 1 2
1 2 45.9 1 2 3 45.9 9 22.59 1 27.5 7.5 3 3 1 3
1 1 17.8 10 1 3 17.8 4 14.18 2 28.5 6.7 3 3 0 1
1 1 24.9 3 1 4 24.9 5 20.19 2 31.9 6.9 5 4 1 2
2 2 56.1 11 1 3 56.1 11 24.28 2 35.2 8 3 3 1 3
1 1 22.1 11 1 3 22.1 4 12.48 1 33.5 3.3 3 3 0 1
2 3 11.8 11 3 2 11.8 2 16.3 2 47.7 8 2 2 1 2
2 1 14.8 5 4 1 14.8 . 11.03 1 36.9 8 1 . 1 3
1 1 44.1 10 4 1 44.1 9 18.92 1 32.8 3.5 1 1 0 1
1 1 12.3 9 1 3 12.3 2 15.3 1 24.9 6.1 3 3 1 2
1 1 21.5 7 3 3 21.5 4 13.95 1 28.3 6.1 3 3 1 3
2 3 47.8 8 3 4 47.8 10 29.51 2 27.4 8 5 4 0 1
2 1 28.1 3 2 1 28.1 6 15.06 2 27.8 8 1 1 1 2
1 2 37.6 7 3 3 37.6 8 18.25 1 29.4 6.7 3 3 1 3
1 1 31 11 1 1 31 6 15.83 1 33.7 9.7 1 1 0 1
1 2 21.2 7 4 2 . 4 18.47 2 30.9 1 2 2 1 2
2 2 59.6 7 1 1 59.6 12 24.84 2 35.3 8 1 1 1 3
1 1 26.7 2 2 3 26.7 5 17.54 2 30.8 3.3 3 3 0 1
2 1 24.6 3 2 2 24.6 5 17.1 2 41.4 8 2 2 1 2
1 3 42.9 11 3 4 42.9 9 22.71 2 30.4 7.3 5 4 1 3
1 1 23.5 10 2 4 23.5 5 19.04 2 29.3 4 4 4 0 1
2 1 12 10 4 3 . 2 14.68 1 37.2 8 3 3 1 2
1 1 19.1 10 3 1 19.1 4 10.97 1 30 9.9 1 1 1 3
2 1 41.2 9 2 1 41.2 8 20.16 2 19.5 8 1 1 0 1
1 2 37.4 10 4 2 37.4 7 25.75 2 28.3 4.9 2 2 1 2
1 3 45 8 4 3 45 9 28.25 2 33.1 8.2 3 3 1 3
1 1 31.1 12 1 3 31.1 6 17.93 1 30.1 7.9 3 3 0 1
2 1 26.7 3 1 3 26.7 5 19.92 1 31.8 8 3 3 1 2
1 2 54.5 4 4 1 54.5 11 25.01 1 28.6 1.6 1 1 1 3
1 2 13.8 7 4 2 13.8 3 18.84 1 31 7.9 2 2 0 1
1 1 50.4 10 2 1 50.4 10 23.71 1 31.4 5.7 1 1 1 2
2 1 52.5 11 1 2 52.5 10 28.65 2 17.8 8 2 2 1 3
2 3 35.6 9 4 4 35.6 7 25.88 2 23.9 8 4 4 0 1
1 1 12.9 3 3 1 12.9 3 11.12 1 30.9 7.4 1 1 1 2
2 2 41.9 8 1 4 41.9 8 24.64 2 28.6 8 4 4 1 3
1 2 16.4 7 1 4 16.4 3 17.76 1 26.3 8.9 5 4 0 1
2 1 46.4 8 3 1 46.4 9 21.38 1 37.5 8 1 1 1 2
1 1 19 4 4 3 19 4 14.76 1 30.3 6.2 3 . 1 3
2 1 18.8 9 1 2 . 4 17.05 2 19.3 8 2 2 0 1
1 2 52.3 12 1 1 52.3 10 21.86 1 27.9 2.7 1 1 1 2
1 1 15.1 2 1 3 15.1 3 11.86 1 29.6 1.3 3 3 1 3
2 2 51.9 8 3 3 51.9 10 31.65 2 33.5 8 3 3 0 1
1 1 29.3 5 1 1 29.3 6 13.11 2 27.7 9 1 1 1 2
1 1 18.3 3 2 3 18.3 4 18.2 2 30.7 3.8 3 3 1 3
2 1 24.8 4 4 2 24.8 5 22.64 2 37.1 8 2 2 0 1
1 1 33.7 9 4 1 33.7 7 19.76 1 31.7 8.6 1 1 1 2
2 3 40.4 6 2 2 40.4 8 20.86 1 39.8 8 2 2 1 3
2 2 51.2 9 2 4 51.2 10 28.8 2 15.1 8 5 4 0 1
2 3 54.2 12 3 4 54.2 11 29.98 2 15.5 8 4 4 1 2
1 1 13.2 8 1 3 13.2 3 13.93 1 28.8 8.1 3 3 1 3
2 3 39.3 10 4 4 . 8 24.97 2 48.8 8 4 4 0 1
1 1 30.1 3 3 2 30.1 6 21.06 1 24.2 7.7 2 2 1 2
1 2 59.7 6 1 3 59.7 12 25.92 1 30.9 4.9 3 3 1 3
1 1 29.2 5 3 4 29.2 6 17.23 2 26.2 9.6 5 4 0 1
2 1 42 5 3 1 42 8 22.9 2 46.8 8 1 1 1 2
1 3 55 7 3 3 55 11 25.79 1 27.8 8 3 3 1 3
2 2 25.6 4 2 4 25.6 5 20.16 1 26.3 8 4 4 0 1
1 2 39.7 6 1 4 39.7 . 22.66 2 31.4 3 5 4 1 2
2 3 57.4 11 2 4 57.4 11 30.74 2 26.7 8 4 4 1 3
2 1 30.9 7 4 1 30.9 6 17.7 1 62.7 8 1 1 0 1
1 2 26.4 9 3 3 26.4 5 18.46 1 28.4 5 3 3 1 2
1 2 55.8 5 4 1 55.8 11 23.61 2 29.3 2.9 1 1 1 3
2 2 38.3 7 4 2 38.3 . 24.24 2 36.3 8 2 2 0 1
2 2 44.2 12 1 4 44.2 9 25.32 2 31.9 8 5 4 1 2
2 2 33.9 3 2 2 . . 22.31 2 30.4 8 2 2 1 3
1 1 45.7 11 1 3 45.7 . 23.08 1 29.9 3.6 3 3 0 1
2 3 59.4 7 3 3 59.4 12 29.38 2 34.2 8 3 3 1 2
2 2 53.2 8 2 3 53.2 11 28.96 2 41.6 8 3 3 1 3
END DATA.
";

fn run(engine: &mut Engine, source: &str) -> Vec<Table> {
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

fn error(engine: &mut Engine, source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.0;
        }
    }
    panic!("no error")
}

fn engine() -> Engine {
    let mut engine = Engine::default();
    run(&mut engine, DATA);
    engine
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

fn close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs().max(1e-300),
        "{actual} is not {expected}"
    );
}

const SMALL: &str = "/GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12";

/// The same tree with a test sample and on the training cases alone: every cell of the tree
/// table (and the surrogates) is the same.
fn same_as_training_alone(command: &str) {
    let printing = "/PRINT TREETABLE SURROGATES RISK /TREE DISPLAY=NONE";
    let mut validated = engine();
    run(&mut validated, "WEIGHT BY w.");
    let a = run(
        &mut validated,
        &format!("{command} /VALIDATION TYPE=SPLITSAMPLE(tr) {printing}."),
    );
    let mut alone = engine();
    run(&mut alone, "WEIGHT BY w.\nSELECT IF tr = 1.");
    let b = run(&mut alone, &format!("{command} {printing}."));
    let (a_tree, b_tree) = (table(&a, "Tree Table"), table(&b, "Tree Table"));
    assert!(
        a_tree.rows.len() > 3,
        "{command} grew {} nodes",
        a_tree.rows.len()
    );
    assert_eq!(a_tree.rows, b_tree.rows, "{command}");
    // the test is not blind: a tree grown on every case is another tree
    let mut everything = engine();
    run(&mut everything, "WEIGHT BY w.");
    let c = run(&mut everything, &format!("{command} {printing}."));
    assert_ne!(
        table(&c, "Tree Table").rows,
        b_tree.rows,
        "{command}: all cases"
    );
    if let (Some(x), Some(y)) = (
        a.iter().find(|t| t.title == "Surrogates"),
        b.iter().find(|t| t.title == "Surrogates"),
    ) {
        assert_eq!(x.rows, y.rows, "{command} surrogates");
    }
}

#[test]
fn chaid_with_a_test_sample_grows_the_tree_of_the_training_cases() {
    same_as_training_alone(&format!(
        "TREE y3 [n] BY xm [s] z [s] x2 [s] o [o] g [n] /METHOD TYPE=CHAID {SMALL}"
    ));
}

#[test]
fn exhaustive_chaid_with_a_test_sample_grows_the_tree_of_the_training_cases() {
    same_as_training_alone(&format!(
        "TREE y3 [n] BY xm [s] z [s] x2 [s] o [o] g [n] /METHOD TYPE=EXHAUSTIVECHAID {SMALL}"
    ));
}

#[test]
fn crt_with_a_test_sample_grows_the_tree_of_the_training_cases() {
    same_as_training_alone(&format!(
        "TREE y3 [n] BY xm [s] z [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT {SMALL}"
    ));
    same_as_training_alone(&format!(
        "TREE y [n] BY xm [s] z [s] x2 [s] o [o] gm [n] /METHOD TYPE=CRT /CRT IMPURITY=TWOING /PRIORS EQUAL {SMALL}"
    ));
}

#[test]
fn quest_with_a_test_sample_grows_the_tree_of_the_training_cases() {
    same_as_training_alone(&format!(
        "TREE y3 [n] BY xm [s] z [s] x2 [s] o [o] g4 [n] /METHOD TYPE=QUEST /QUEST ALPHASPLIT=.2 {SMALL}"
    ));
    same_as_training_alone(&format!(
        "TREE y [n] BY v [s] zv [s] x2 [s] b [n] /METHOD TYPE=QUEST {SMALL}"
    ));
}

/// The values of a saved variable.
fn values(engine: &Engine, name: &str) -> Vec<f64> {
    engine
        .dataset()
        .expect("data")
        .variable(name)
        .expect("saved variable")
        .numeric()
        .expect("numeric")
        .iter()
        .map(|v| v.unwrap_or(f64::NAN))
        .collect()
}

/// The risk of the test cases computed from the saved predictions, and the one in the table.
fn test_risk_checks(command: &str, equal_priors: bool) {
    let mut engine = engine();
    run(&mut engine, "WEIGHT BY w.");
    let tables = run(
        &mut engine,
        &format!(
            "{command} /VALIDATION TYPE=SPLITSAMPLE(tr) /PRINT RISK CLASSIFICATION MODELSUMMARY \
             /SAVE NODEID(node) PREDVAL(guess) /TREE DISPLAY=NONE."
        ),
    );
    let (y, guess, tr, w) = (
        values(&engine, "y3"),
        values(&engine, "guess"),
        values(&engine, "tr"),
        values(&engine, "w"),
    );
    // every case lands in a terminal node and gets a prediction
    assert!(values(&engine, "node").iter().all(|n| n.is_finite()));
    assert!(guess.iter().all(|g| g.is_finite()));
    let risk = table(&tables, "Risk");
    let rows: Vec<(&Cell, &Cell, &Cell)> =
        risk.rows.iter().map(|r| (&r[0], &r[1], &r[2])).collect();
    assert_eq!(rows.len(), 2);
    for (k, (sample, want)) in [(1.0, "Training"), (0.0, "Test")].into_iter().enumerate() {
        assert!(matches!(rows[k].0, Cell::Text(t) if t == want));
        let in_sample: Vec<usize> = (0..y.len()).filter(|&i| tr[i] == sample).collect();
        let total: f64 = in_sample.iter().map(|&i| w[i]).sum();
        let estimate = if equal_priors {
            // the share misclassified of each class, times its prior of a third
            (1..=3)
                .map(|j| {
                    let class: Vec<usize> = in_sample
                        .iter()
                        .copied()
                        .filter(|&i| y[i] == j as f64)
                        .collect();
                    let n: f64 = class.iter().map(|&i| w[i]).sum();
                    let wrong: f64 = class
                        .iter()
                        .filter(|&&i| guess[i] != y[i])
                        .map(|&i| w[i])
                        .sum();
                    wrong / n / 3.0
                })
                .sum()
        } else {
            let wrong: f64 = in_sample
                .iter()
                .filter(|&&i| guess[i] != y[i])
                .map(|&i| w[i])
                .sum();
            wrong / total
        };
        close(number(rows[k].1), estimate, 1e-12);
        assert!(number(rows[k].2) > 0.0);
    }
    // the classification table counts each sample's cases
    let classification = table(&tables, "Classification");
    assert_eq!(classification.row_headers, 2);
    let n = |sample: f64| -> f64 {
        (0..y.len())
            .filter(|&i| tr[i] == sample)
            .map(|i| w[i])
            .sum()
    };
    let mut seen = 0.0;
    for row in classification
        .rows
        .iter()
        .filter(|r| matches!(&r[1], Cell::Text(t) if t != "Overall Percentage"))
    {
        seen += (2..5).map(|k| number(&row[k])).sum::<f64>();
    }
    close(seen, n(1.0) + n(0.0), 1e-12);
    let summary = table(&tables, "Model Summary");
    let find = |name: &str| {
        summary
            .rows
            .iter()
            .find(|r| matches!(&r[1], Cell::Text(t) if t == name))
    };
    close(
        number(&find("Cases in Training Sample").expect("row")[2]),
        n(1.0),
        1e-12,
    );
    close(
        number(&find("Cases in Test Sample").expect("row")[2]),
        n(0.0),
        1e-12,
    );
}

#[test]
fn the_test_risk_follows_the_saved_predictions() {
    let base = format!("TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT {SMALL}");
    test_risk_checks(&base, false);
    test_risk_checks(&format!("{base} /PRIORS EQUAL"), true);
    test_risk_checks(
        &format!("TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CHAID {SMALL}"),
        false,
    );
}

#[test]
fn only_the_test_sample_with_output_testsample() {
    let mut engine = engine();
    let tables = run(
        &mut engine,
        &format!(
            "TREE y3 [n] BY x1 [s] x2 [s] /METHOD TYPE=CRT {SMALL} /VALIDATION TYPE=SPLITSAMPLE(tr) \
             OUTPUT=TESTSAMPLE /PRINT RISK CLASSIFICATION /DEPCATEGORIES TARGET=[1] /GAIN CATEGORYTABLE=YES /TREE DISPLAY=NONE."
        ),
    );
    assert_eq!(table(&tables, "Risk").rows.len(), 1);
    assert!(tables.iter().all(|t| !t.title.contains("Training")));
    assert!(
        tables
            .iter()
            .any(|t| t.title == "Gain Summary for Nodes (Test Sample)")
    );
}

#[test]
fn both_samples_get_their_gain_tables() {
    let mut engine = engine();
    let tables = run(
        &mut engine,
        &format!(
            "TREE y3 [n] BY x1 [s] x2 [s] /METHOD TYPE=CRT {SMALL} /VALIDATION TYPE=SPLITSAMPLE(tr) \
             /DEPCATEGORIES TARGET=[1] /GAIN CATEGORYTABLE=YES /PLOT GAIN /TREE DISPLAY=NONE."
        ),
    );
    let training = table(&tables, "Gain Summary for Nodes (Training Sample)");
    let test = table(&tables, "Gain Summary for Nodes (Test Sample)");
    // the same terminal nodes in the same order
    let nodes = |t: &Table| {
        t.rows
            .iter()
            .map(|r| format!("{:?}", r[0]))
            .collect::<Vec<_>>()
    };
    assert_eq!(nodes(training), nodes(test));
}

#[test]
fn a_percent_makes_a_reproducible_random_split() {
    let command = format!(
        "SET SEED=2468.\nTREE y [n] BY x1 [s] x2 [s] /METHOD TYPE=CRT {SMALL} /VALIDATION TYPE=SPLITSAMPLE(60) \
         /PRINT MODELSUMMARY /TREE DISPLAY=NONE."
    );
    let sizes = |source: &str| {
        let mut engine = engine();
        let tables = run(&mut engine, source);
        let summary = table(&tables, "Model Summary");
        let get = |name: &str| {
            summary
                .rows
                .iter()
                .find(|r| matches!(&r[1], Cell::Text(t) if t == name))
                .map(|r| number(&r[2]))
                .expect("row")
        };
        (get("Cases in Training Sample"), get("Cases in Test Sample"))
    };
    let first = sizes(&command);
    assert_eq!(first, sizes(&command));
    // 150 cases of weight 1 to 3, 60 percent to train: a plausible share
    let total = first.0 + first.1;
    assert!((0.4..0.8).contains(&(first.0 / total)), "{first:?}");
}

#[test]
fn a_category_only_the_test_cases_have_is_routed_not_lost() {
    // category 4 of g is in the test cases only: CHAID codes it as missing, CRT sends it down the
    // side of the categories it does not name
    let mut source = String::from("DATA LIST FREE / y g x tr.\nBEGIN DATA\n");
    for i in 0..120 {
        let g = i % 3 + 1;
        let y = if (g == 1) == (i % 7 != 0) { 1 } else { 2 };
        source.push_str(&format!("{y} {g} {} 1\n", (i * 7) % 23));
    }
    for i in 0..30 {
        source.push_str(&format!("{} 4 {} 0\n", i % 2 + 1, (i * 5) % 23));
    }
    source.push_str("END DATA.\n");
    for method in ["CHAID", "CRT", "QUEST"] {
        let mut engine = Engine::default();
        run(&mut engine, &source);
        let tables = run(
            &mut engine,
            &format!(
                "TREE y [n] BY g [n] x [s] /METHOD TYPE={method} /GROWTHLIMIT MINPARENTSIZE=20 MINCHILDSIZE=8 \
                 /VALIDATION TYPE=SPLITSAMPLE(tr) /PRINT CLASSIFICATION /SAVE NODEID(node) /TREE DISPLAY=NONE."
            ),
        );
        let node = values(&engine, "node");
        assert_eq!(node.len(), 150);
        assert!(node.iter().all(|n| n.is_finite()), "{method}");
        let classification = table(&tables, "Classification");
        let counted: f64 = classification
            .rows
            .iter()
            .filter(|r| {
                matches!(&r[0], Cell::Text(t) if t == "Test")
                    && matches!(&r[1], Cell::Text(t) if t != "Overall Percentage")
            })
            .map(|r| (2..4).map(|k| number(&r[k])).sum::<f64>())
            .sum();
        assert_eq!(counted, 30.0, "{method}");
    }
}

#[test]
fn pruning_by_the_test_sample_never_does_worse_on_it() {
    let base = "TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /GROWTHLIMIT MAXDEPTH=5 MINPARENTSIZE=10 MINCHILDSIZE=5 \
         /VALIDATION TYPE=SPLITSAMPLE(tr) /PRINT RISK TREETABLE /TREE DISPLAY=NONE";
    let result = |extra: &str| {
        let mut engine = engine();
        let tables = run(&mut engine, &format!("{base} {extra}."));
        let risk = table(&tables, "Risk");
        (
            number(&risk.rows[1][1]),
            table(&tables, "Tree Table").rows.len(),
        )
    };
    let (grown, grown_nodes) = result("/METHOD TYPE=CRT");
    let (pruned, pruned_nodes) = result("/METHOD TYPE=CRT PRUNE=SE(0)");
    assert!(pruned_nodes <= grown_nodes);
    assert!(pruned <= grown + 1e-12, "{pruned} against {grown}");
}

#[test]
fn what_validation_cannot_use_is_refused() {
    let mut engine = engine();
    let message = error(
        &mut engine,
        "TREE y [n] BY x1 [s] /VALIDATION TYPE=SPLITSAMPLE(y).",
    );
    assert!(message.contains("cannot be the dependent"), "{message}");
    run(&mut engine, "COMPUTE everyone = 1.");
    let message = error(
        &mut engine,
        "TREE y [n] BY x1 [s] /VALIDATION TYPE=SPLITSAMPLE(everyone).",
    );
    assert!(message.contains("test sample has no cases"), "{message}");
    run(&mut engine, "COMPUTE none = 0.");
    let message = error(
        &mut engine,
        "TREE y [n] BY x1 [s] /VALIDATION TYPE=SPLITSAMPLE(none).",
    );
    assert!(
        message.contains("training sample has fewer than two"),
        "{message}"
    );
}

/// The fold of each case as the engine draws it: one uniform number per case in order, the
/// cases sorted by it, the k-th in that order in fold k mod V.
fn folds(seed: u32, count: usize, v: usize) -> Vec<usize> {
    let mut engine = engine();
    run(
        &mut engine,
        &format!("SET SEED={seed}.\nCOMPUTE u = UNIFORM(1)."),
    );
    let u = values(&engine, "u");
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by(|&a, &b| u[a].total_cmp(&u[b]));
    let mut fold = vec![0; count];
    for (rank, &i) in order.iter().enumerate() {
        fold[i] = rank % v;
    }
    fold
}

/// The data with the fold of each case as a last column.
fn data_with_fold(fold: &[usize]) -> String {
    let at = DATA.find("BEGIN DATA\n").expect("data") + "BEGIN DATA\n".len();
    let end = DATA.find("END DATA.").expect("end");
    let rows: Vec<String> = DATA[at..end]
        .lines()
        .zip(fold)
        .map(|(row, f)| format!("{row} {f}"))
        .collect();
    format!(
        "DATA LIST FREE / y y3 x1 x2 o g xm z r b v zv g4 gm tr w fold.\nBEGIN DATA\n{}\nEND DATA.\n",
        rows.join("\n")
    )
}

/// The cross-validated risk of `command` against the same trees grown fold by fold with a split
/// sample (the fold held out as the test sample).
fn cross_validation_equals_the_folds(command: &str, v: usize, equal_priors: bool) {
    cross_validation_with_costs(command, v, equal_priors, &UNIT_COSTS);
}

/// The cost of assigning class i (row) to a case of class j (column).
type CostMatrix = [[f64; 3]; 3];
const UNIT_COSTS: CostMatrix = [[0.0, 1.0, 1.0], [1.0, 0.0, 1.0], [1.0, 1.0, 0.0]];

fn cross_validation_with_costs(command: &str, v: usize, equal_priors: bool, costs: &CostMatrix) {
    let seed = 13579;
    let fold = folds(seed, 150, v);
    let with_fold = data_with_fold(&fold);
    let printing = "/PRINT RISK CLASSIFICATION TREETABLE /TREE DISPLAY=NONE";
    let mut engine = Engine::default();
    run(&mut engine, &with_fold);
    run(&mut engine, "WEIGHT BY w.");
    let crossed = run(
        &mut engine,
        &format!("SET SEED={seed}.\n{command} /VALIDATION TYPE=CROSSVALIDATION({v}) {printing}."),
    );
    let plain = run(&mut engine, &format!("{command} {printing}."));
    // the tree and its resubstitution risk are those without validation
    assert_eq!(
        table(&crossed, "Tree Table").rows,
        table(&plain, "Tree Table").rows
    );
    let risk = table(&crossed, "Risk");
    assert_eq!(risk.rows.len(), 2);
    assert!(matches!(&risk.rows[0][0], Cell::Text(t) if t == "Resubstitution"));
    assert!(matches!(&risk.rows[1][0], Cell::Text(t) if t == "Cross-Validation"));
    close(
        number(&risk.rows[0][1]),
        number(&table(&plain, "Risk").rows[0][0]),
        1e-12,
    );
    // the class weights missed by the tree of each fold, from its test-sample classification
    let classes = 3;
    let (mut wrong, mut squared) = (vec![0.0; classes], vec![0.0; classes]);
    for f in 0..v {
        run(&mut engine, &format!("COMPUTE keep = (fold <> {f})."));
        let tables = run(
            &mut engine,
            &format!("{command} /VALIDATION TYPE=SPLITSAMPLE(keep) OUTPUT=TESTSAMPLE {printing}."),
        );
        let classification = table(&tables, "Classification");
        for (j, row) in classification.rows.iter().take(classes).enumerate() {
            // the cases of class j assigned each class, with the cost of that
            for i in 0..classes {
                let count = number(&row[2 + i]);
                wrong[j] += count * costs[i][j];
                squared[j] += count * costs[i][j] * costs[i][j];
            }
        }
        run(&mut engine, "DELETE VARIABLES keep.");
    }
    let totals: Vec<f64> = (1..=3)
        .map(|class| {
            let (y3, w) = (values(&engine, "y3"), values(&engine, "w"));
            (0..150)
                .filter(|&i| y3[i] == class as f64)
                .map(|i| w[i])
                .sum()
        })
        .collect();
    let n: f64 = totals.iter().sum();
    let (estimate, variance) = if equal_priors {
        (
            (0..classes)
                .map(|j| wrong[j] / totals[j] / 3.0)
                .sum::<f64>(),
            (0..classes)
                .map(|j| {
                    (1.0 / 3.0 / totals[j]).powi(2) * (squared[j] - wrong[j] * wrong[j] / totals[j])
                })
                .sum::<f64>(),
        )
    } else {
        let r = wrong.iter().sum::<f64>() / n;
        (r, (squared.iter().sum::<f64>() - n * r * r) / (n * n))
    };
    close(number(&risk.rows[1][1]), estimate, 1e-12);
    close(number(&risk.rows[1][2]), variance.sqrt(), 1e-9);
}

#[test]
fn cross_validation_of_chaid_is_the_pooled_misses_of_its_fold_trees() {
    cross_validation_equals_the_folds(
        &format!("TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CHAID {SMALL}"),
        5,
        false,
    );
}

#[test]
fn cross_validation_of_crt_with_equal_priors() {
    cross_validation_equals_the_folds(
        &format!("TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT /PRIORS EQUAL {SMALL}"),
        4,
        true,
    );
}

#[test]
fn cross_validation_of_quest_with_a_nominal_predictor() {
    cross_validation_equals_the_folds(
        &format!(
            "TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=QUEST /PRIORS EQUAL {SMALL}"
        ),
        3,
        true,
    );
}

#[test]
fn cross_validation_with_force_is_ignored() {
    let mut engine = engine();
    let tables = run(
        &mut engine,
        &format!(
            "TREE y3 [n] BY x1 [s] x2 [s] FORCE=x1 /VALIDATION TYPE=CROSSVALIDATION(5) /PRINT RISK {SMALL}."
        ),
    );
    assert_eq!(table(&tables, "Risk").rows.len(), 1);
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("CROSSVALIDATION is ignored"))
    );
}

#[test]
fn cross_validation_prunes_each_fold_tree_by_its_own_cases() {
    // not the test sample's risk as in a split sample: a fold tree has only its learning cases
    let mut engine = engine();
    let tables = run(
        &mut engine,
        "SET SEED=99.\nTREE y3 [n] BY x1 [s] x2 [s] o [o] /METHOD TYPE=CRT PRUNE=SE(1) \
         /VALIDATION TYPE=CROSSVALIDATION(5) /PRINT RISK MODELSUMMARY /GROWTHLIMIT MAXDEPTH=5 MINPARENTSIZE=10 MINCHILDSIZE=5 /TREE DISPLAY=NONE.",
    );
    let risk = table(&tables, "Risk");
    assert_eq!(risk.rows.len(), 2);
    assert!(number(&risk.rows[1][1]) > 0.0 && number(&risk.rows[1][1]) < 1.0);
    let summary = table(&tables, "Model Summary");
    assert!(
        summary
            .rows
            .iter()
            .any(|r| matches!(&r[2], Cell::Text(t) if t == "Cross-validation, 5 folds"))
    );
}

const SCALE_COMMAND: &str = "TREE r [s] BY xm [s] z [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT";

#[test]
fn crt_of_a_scale_dependent_with_a_test_sample_grows_the_tree_of_the_training_cases() {
    same_as_training_alone(&format!("{SCALE_COMMAND} {SMALL}"));
}

#[test]
fn the_test_risk_of_a_scale_dependent_is_the_mean_squared_deviation_from_the_training_means() {
    let mut engine = engine();
    run(&mut engine, "WEIGHT BY w.");
    let tables = run(
        &mut engine,
        &format!(
            "{SCALE_COMMAND} {SMALL} /VALIDATION TYPE=SPLITSAMPLE(tr) /PRINT RISK MODELSUMMARY \
             /SAVE PREDVAL(guess) /TREE DISPLAY=NONE."
        ),
    );
    let (r, guess, tr, w) = (
        values(&engine, "r"),
        values(&engine, "guess"),
        values(&engine, "tr"),
        values(&engine, "w"),
    );
    let risk = table(&tables, "Risk");
    assert_eq!(risk.rows.len(), 2);
    for (k, sample) in [1.0, 0.0].into_iter().enumerate() {
        let cases: Vec<usize> = (0..r.len()).filter(|&i| tr[i] == sample).collect();
        let n: f64 = cases.iter().map(|&i| w[i]).sum();
        let squares: f64 = cases
            .iter()
            .map(|&i| w[i] * (r[i] - guess[i]).powi(2))
            .sum();
        let fourth: f64 = cases
            .iter()
            .map(|&i| w[i] * (r[i] - guess[i]).powi(4))
            .sum();
        let estimate = squares / n;
        close(number(&risk.rows[k][1]), estimate, 1e-9);
        close(
            number(&risk.rows[k][2]),
            ((fourth - n * estimate * estimate) / (n * n)).sqrt(),
            1e-6,
        );
    }
}

#[test]
fn cross_validation_of_a_scale_dependent_pools_the_fold_trees_squares() {
    let seed = 777;
    let v = 4;
    let fold = folds(seed, 150, v);
    let mut engine = Engine::default();
    run(&mut engine, &data_with_fold(&fold));
    run(&mut engine, "WEIGHT BY w.");
    let command = format!("{SCALE_COMMAND} {SMALL}");
    let printing = "/PRINT RISK /TREE DISPLAY=NONE";
    let crossed = run(
        &mut engine,
        &format!("SET SEED={seed}.\n{command} /VALIDATION TYPE=CROSSVALIDATION({v}) {printing}."),
    );
    let risk = table(&crossed, "Risk");
    assert_eq!(risk.rows.len(), 2);
    // each fold's tree tested on the fold: its squares and fourth powers add up
    let w = values(&engine, "w");
    let (mut squares, mut fourth) = (0.0, 0.0);
    for f in 0..v {
        run(&mut engine, &format!("COMPUTE keep = (fold <> {f})."));
        let tables = run(
            &mut engine,
            &format!("{command} /VALIDATION TYPE=SPLITSAMPLE(keep) OUTPUT=TESTSAMPLE {printing}."),
        );
        let test = table(&tables, "Risk");
        let n: f64 = (0..150).filter(|&i| fold[i] == f).map(|i| w[i]).sum();
        let (rf, se) = (number(&test.rows[0][1]), number(&test.rows[0][2]));
        squares += rf * n;
        fourth += se * se * n * n + n * rf * rf;
        run(&mut engine, "DELETE VARIABLES keep.");
    }
    let n: f64 = w.iter().sum();
    let estimate = squares / n;
    close(number(&risk.rows[1][1]), estimate, 1e-9);
    close(
        number(&risk.rows[1][2]),
        ((fourth - n * estimate * estimate) / (n * n)).sqrt(),
        1e-6,
    );
}

#[test]
fn cross_validation_with_costs_pools_the_cost_of_the_fold_trees_assignments() {
    // assigning class i to a case of class j costs costs[i][j]: here 5 for 1 to a 3, 2 for 2 to a 3,
    // 3 for 1 to a 2, as COSTS CUSTOM=3 1 [5] 3 2 [2] 2 1 [3] says
    let costs = [[0.0, 3.0, 5.0], [1.0, 0.0, 2.0], [1.0, 1.0, 0.0]];
    let tree = format!("TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT {SMALL}");
    let custom = "/COSTS CUSTOM=3 1 [5] 3 2 [2] 2 1 [3]";
    cross_validation_with_costs(&format!("{tree} {custom}"), 4, false, &costs);
    cross_validation_with_costs(&format!("{tree} /PRIORS EQUAL {custom}"), 4, true, &costs);
    let chaid = format!("TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CHAID {SMALL}");
    cross_validation_with_costs(&format!("{chaid} {custom}"), 3, false, &costs);
}

#[test]
fn the_test_risk_counts_the_cost_of_each_assignment() {
    let mut engine = engine();
    run(&mut engine, "WEIGHT BY w.");
    let tables = run(
        &mut engine,
        &format!(
            "TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT /COSTS CUSTOM=3 1 [5] 3 2 [2] 2 1 [3] {SMALL} \
             /VALIDATION TYPE=SPLITSAMPLE(tr) /PRINT RISK /SAVE PREDVAL(guess) /TREE DISPLAY=NONE."
        ),
    );
    let (y, guess, tr, w) = (
        values(&engine, "y3"),
        values(&engine, "guess"),
        values(&engine, "tr"),
        values(&engine, "w"),
    );
    let cost = |assigned: f64, actual: f64| match (assigned as usize, actual as usize) {
        (a, b) if a == b => 0.0,
        (1, 3) => 5.0,
        (2, 3) => 2.0,
        (1, 2) => 3.0,
        _ => 1.0,
    };
    let risk = table(&tables, "Risk");
    for (k, sample) in [1.0, 0.0].into_iter().enumerate() {
        let cases: Vec<usize> = (0..y.len()).filter(|&i| tr[i] == sample).collect();
        let n: f64 = cases.iter().map(|&i| w[i]).sum();
        let loss: f64 = cases.iter().map(|&i| w[i] * cost(guess[i], y[i])).sum();
        let squared: f64 = cases
            .iter()
            .map(|&i| w[i] * cost(guess[i], y[i]).powi(2))
            .sum();
        let estimate = loss / n;
        close(number(&risk.rows[k][1]), estimate, 1e-12);
        close(
            number(&risk.rows[k][2]),
            ((squared - n * estimate * estimate) / (n * n)).sqrt(),
            1e-9,
        );
    }
}

#[test]
fn chaid_of_a_scale_dependent_with_a_test_sample_grows_the_tree_of_the_training_cases() {
    for method in ["CHAID", "EXHAUSTIVECHAID"] {
        same_as_training_alone(&format!(
            "TREE r [s] BY o [o] g [n] b [n] g4 [n] x2 [s] /METHOD TYPE={method} /CHAID ALPHASPLIT=.2 \
             /GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=20 MINCHILDSIZE=8"
        ));
    }
}

fn text_of(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        other => format!("{other:?}"),
    }
}

/// The terminal nodes of a sample as (node, class counts), from the node each case was saved in.
fn terminal_counts(engine: &Engine, sample: Option<f64>) -> Vec<(usize, [f64; 3])> {
    let (node, y, w, tr) = (
        values(engine, "node"),
        values(engine, "y3"),
        values(engine, "w"),
        values(engine, "tr"),
    );
    let mut found: Vec<(usize, [f64; 3])> = Vec::new();
    let mut ids: Vec<usize> = node.iter().map(|&n| n as usize).collect();
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        let mut counts = [0.0; 3];
        for i in 0..node.len() {
            if node[i] as usize == id && sample.is_none_or(|s| tr[i] == s) {
                counts[y[i] as usize - 1] += w[i];
            }
        }
        found.push((id, counts));
    }
    found
}

/// The node-by-node gain rows of target class `target` (0-based) from class counts: the size
/// share, response and index from the node probabilities under `priors`, and the cumulative
/// figures, in the order of `order` (node ids).
fn gain_rows(
    counts: &[(usize, [f64; 3])],
    priors: [f64; 3],
    target: usize,
    order: &[usize],
) -> Vec<Vec<f64>> {
    let totals: Vec<f64> = (0..3)
        .map(|j| counts.iter().map(|c| c.1[j]).sum())
        .collect();
    let figures = |node: usize| {
        let c = counts.iter().find(|c| c.0 == node).expect("a node").1;
        let n: f64 = c.iter().sum();
        let w: f64 = (0..3).map(|j| priors[j] * c[j] / totals[j]).sum();
        let a = priors[target] * c[target] / totals[target];
        (n, c[target], w, a)
    };
    let all_w: f64 = counts.iter().map(|c| figures(c.0).2).sum();
    let (mut sn, mut sg, mut sw, mut sa) = (0.0, 0.0, 0.0, 0.0);
    order
        .iter()
        .map(|&node| {
            let (n, g, w, a) = figures(node);
            (sn, sg, sw, sa) = (sn + n, sg + g, sw + w, sa + a);
            let mut row = Vec::new();
            for (n, g, w, a) in [(n, g, w, a), (sn, sg, sw, sa)] {
                row.extend([
                    n,
                    100.0 * w / all_w,
                    g,
                    100.0 * g / totals[target],
                    100.0 * a / w,
                    100.0 * (a / w) / priors[target],
                ]);
            }
            row
        })
        .collect()
}

#[test]
fn gain_tables_use_the_node_probabilities_when_the_priors_are_not_the_data() {
    let check = |priors_clause: &str, given: [f64; 3], validated: bool| {
        let mut engine = engine();
        run(&mut engine, "WEIGHT BY w.");
        let validation = if validated {
            "/VALIDATION TYPE=SPLITSAMPLE(tr)"
        } else {
            ""
        };
        let tables = run(
            &mut engine,
            &format!(
                "TREE y3 [n] BY x1 [s] x2 [s] o [o] g [n] /METHOD TYPE=CRT {priors_clause} {validation} {SMALL} \
                 /DEPCATEGORIES TARGET=[1 2] /GAIN CATEGORYTABLE=YES CUMULATIVE=YES /SAVE NODEID(node) /TREE DISPLAY=NONE."
            ),
        );
        // all zeros stands for priors from the data: each sample's own class shares
        let shares = |sample: Option<f64>| -> [f64; 3] {
            if given != [0.0; 3] {
                return given;
            }
            let counts = terminal_counts(&engine, sample);
            let totals: Vec<f64> = (0..3)
                .map(|j| counts.iter().map(|c| c.1[j]).sum())
                .collect();
            let n: f64 = totals.iter().sum();
            [totals[0] / n, totals[1] / n, totals[2] / n]
        };
        let samples: Vec<(&str, Option<f64>)> = if validated {
            vec![("Training", Some(1.0)), ("Test", Some(0.0))]
        } else {
            vec![("", None)]
        };
        for target in 0..2 {
            // the training sample's order of the nodes by response, highest first
            let training = terminal_counts(&engine, validated.then_some(1.0));
            let score = |node: usize| {
                let rows = gain_rows(&training, shares(validated.then_some(1.0)), target, &[node]);
                rows[0][4]
            };
            let mut order: Vec<usize> = training.iter().map(|c| c.0).collect();
            // scores within 1e-12 are tied, the nodes then keeping their order
            let key = |n: usize| (score(n) / 100.0 * 1e12).round() as i64;
            order.sort_by_key(|&n| std::cmp::Reverse(key(n)));
            for (label, sample) in &samples {
                let title = if validated {
                    format!("Gain Summary for Nodes ({label} Sample)")
                } else {
                    "Gain Summary for Nodes".to_string()
                };
                let found: Vec<&Table> = tables.iter().filter(|t| t.title == title).collect();
                let found = found[target];
                let counts = terminal_counts(&engine, *sample);
                let want = gain_rows(&counts, shares(*sample), target, &order);
                assert_eq!(found.rows.len(), want.len(), "{title}");
                for (row, want) in found.rows.iter().zip(&want) {
                    for (k, expected) in want.iter().enumerate() {
                        let got = number(&row[1 + k]);
                        assert!(
                            (got - expected).abs() <= 1e-9 * expected.abs().max(1e-300),
                            "{priors_clause} validated {validated} {title} target {target} column {k}: {got} against {expected}; got {:?} want {want:?}",
                            found
                                .rows
                                .iter()
                                .map(|r| text_of(&r[0]))
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
    };
    let third = 1.0 / 3.0;
    check("/PRIORS EQUAL", [third; 3], false);
    check("/PRIORS CUSTOM=1 [5] 2 [3] 3 [2]", [0.5, 0.3, 0.2], false);
    // a test sample: its own class totals, the training sample's priors
    check("/PRIORS EQUAL", [third; 3], true);
    // priors from the data are the counts: the figures the node counts give directly
    check("/PRIORS FROMDATA", [0.0; 3], false);
    check("/PRIORS FROMDATA", [0.0; 3], true);
}

#[test]
fn the_last_percentile_holds_every_case_whatever_the_priors() {
    // the 100 percent point: gain 100, the response the sample's score (the prior of the target),
    // index 100
    for (clause, response) in [
        ("/PRIORS EQUAL", 100.0 / 3.0),
        ("/PRIORS CUSTOM=1 [5] 2 [3] 3 [2]", 50.0),
    ] {
        let mut engine = engine();
        run(&mut engine, "WEIGHT BY w.");
        run(
            &mut engine,
            &format!(
                "TREE y3 [n] BY x1 [s] x2 [s] /METHOD TYPE=CRT {clause} {SMALL} /DEPCATEGORIES TARGET=[1] \
                 /PLOT GAIN RESPONSE INDEX INCREMENT=25 /TREE DISPLAY=NONE."
            ),
        );
        let charts = engine.take_charts();
        assert_eq!(charts.len(), 3);
        let last = |k: usize| *charts[k].series[0].values.last().expect("a value");
        close(last(0), 100.0, 1e-9);
        close(last(1), response, 1e-9);
        close(last(2), 100.0, 1e-9);
    }
}
