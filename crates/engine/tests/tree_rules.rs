//! TREE /RULES: the rules are run. The INTERNAL scoring rules of a tree, executed on the data it
//! was grown on, must give every case the node and the prediction that SAVE gives it, for each
//! method, a scale dependent, missing values with and without surrogates, and a test sample whose
//! cases the tree routed but did not grow on. The SQL and generic forms are checked as text on
//! small trees.

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
    assert!(errors.is_empty(), "{errors:?}\n{source}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}\n{source}", command.first_line),
        }
    }
    tables
}

fn engine() -> Engine {
    let mut engine = Engine::default();
    run(&mut engine, DATA);
    engine
}

fn values(engine: &Engine, name: &str) -> Vec<f64> {
    engine
        .dataset()
        .expect("data")
        .variable(name)
        .unwrap_or_else(|_| panic!("no variable {name}"))
        .numeric()
        .expect("numeric")
        .iter()
        .map(|v| v.unwrap_or(f64::NAN))
        .collect()
}

/// The lines of the Rules table.
fn rule_lines(tables: &[Table]) -> Vec<String> {
    let rules = tables
        .iter()
        .find(|t| t.title == "Rules")
        .expect("a Rules table");
    rules
        .rows
        .iter()
        .map(|r| match &r[0] {
            Cell::Text(t) => t.clone(),
            other => panic!("{other:?}"),
        })
        .collect()
}

const SMALL: &str = "/GROWTHLIMIT MAXDEPTH=3 MINPARENTSIZE=30 MINCHILDSIZE=12";

/// Grows the tree with SAVE, runs the INTERNAL scoring rules it prints, and compares the node
/// and the prediction of every case that SAVE fills. Returns the number of cases the rules left
/// without a node.
fn round_trip(tree: &str, rules: &str, scale: bool) -> usize {
    let mut engine = engine();
    run(&mut engine, "WEIGHT BY w.");
    let tables = run(
        &mut engine,
        &format!(
            "{tree} /SAVE NODEID(saved_node) PREDVAL(saved_value) /RULES {rules} SYNTAX=INTERNAL TYPE=SCORING \
             /PRINT NONE /TREE DISPLAY=NONE."
        ),
    );
    let lines = rule_lines(&tables);
    let program = lines.join("\n");
    run(&mut engine, &program);
    let (saved_node, saved_value) = (
        values(&engine, "saved_node"),
        values(&engine, "saved_value"),
    );
    let (node, value) = (values(&engine, "NodeID"), values(&engine, "PredictedValue"));
    let mut unassigned = 0;
    let mut compared = 0;
    for i in 0..node.len() {
        if saved_node[i].is_nan() {
            continue;
        }
        if node[i].is_nan() {
            unassigned += 1;
            continue;
        }
        compared += 1;
        assert_eq!(node[i], saved_node[i], "node of case {i}\n{program}");
        if scale {
            assert!(
                (value[i] - saved_value[i]).abs() < 1e-9,
                "value of case {i}"
            );
        } else {
            assert_eq!(value[i], saved_value[i], "value of case {i}");
        }
    }
    assert!(compared > 100, "only {compared} cases compared\n{program}");
    unassigned
}

const PREDICTORS: &str = "xm [s] z [s] x2 [s] o [o] g [n]";

#[test]
fn chaid_rules_give_every_case_its_node() {
    for method in ["CHAID", "EXHAUSTIVECHAID"] {
        let tree = format!("TREE y3 [n] BY {PREDICTORS} /METHOD TYPE={method} {SMALL}");
        assert_eq!(round_trip(&tree, "NODES=TERMINAL", false), 0, "{method}");
    }
}

#[test]
fn crt_rules_follow_the_surrogates_of_cases_with_missing_values() {
    let tree = format!("TREE y3 [n] BY {PREDICTORS} /METHOD TYPE=CRT {SMALL}");
    assert_eq!(round_trip(&tree, "NODES=TERMINAL", false), 0);
    // with every surrogate, then none (the larger side takes the cases with no value)
    let tree = format!("TREE y3 [n] BY {PREDICTORS} /METHOD TYPE=CRT MAXSURROGATES=0 {SMALL}");
    assert_eq!(round_trip(&tree, "NODES=TERMINAL", false), 0);
    let tree =
        format!("TREE y [n] BY xm [s] z [s] gm [n] o [o] /METHOD TYPE=CRT MAXSURROGATES=2 {SMALL}");
    assert_eq!(round_trip(&tree, "NODES=TERMINAL", false), 0);
}

#[test]
fn rules_without_surrogates_leave_the_cases_with_missing_values_out() {
    let tree = format!("TREE y3 [n] BY {PREDICTORS} /METHOD TYPE=CRT {SMALL}");
    // they agree wherever they assign a node, and miss some cases
    assert!(round_trip(&tree, "NODES=TERMINAL SURROGATES=EXCLUDE", false) > 0);
}

#[test]
fn quest_rules_with_a_nominal_predictor_of_five_categories() {
    let tree = format!(
        "TREE y3 [n] BY xm [s] z [s] x2 [s] o [o] g4 [n] /METHOD TYPE=QUEST /QUEST ALPHASPLIT=.2 {SMALL}"
    );
    assert_eq!(round_trip(&tree, "NODES=TERMINAL", false), 0);
}

#[test]
fn rules_of_a_scale_dependent_predict_the_node_mean() {
    for method in ["CRT", "CHAID"] {
        let tree = format!("TREE r [s] BY {PREDICTORS} /METHOD TYPE={method} {SMALL}");
        assert_eq!(round_trip(&tree, "NODES=TERMINAL", true), 0, "{method}");
    }
}

#[test]
fn rules_for_all_nodes_end_with_the_terminal_node() {
    for method in ["CHAID", "CRT"] {
        let tree = format!("TREE y3 [n] BY {PREDICTORS} /METHOD TYPE={method} {SMALL}");
        assert_eq!(round_trip(&tree, "NODES=ALL", false), 0, "{method}");
    }
}

#[test]
fn rules_of_a_tree_grown_on_a_training_sample_score_the_test_cases_too() {
    for method in ["CHAID", "CRT", "QUEST"] {
        let tree = format!(
            "TREE y3 [n] BY xm [s] z [s] x2 [s] o [o] g4 [n] /METHOD TYPE={method} {SMALL} \
             /VALIDATION TYPE=SPLITSAMPLE(tr)"
        );
        assert_eq!(round_trip(&tree, "NODES=TERMINAL", false), 0, "{method}");
    }
}

#[test]
fn a_category_only_the_test_cases_have_follows_the_surrogate_or_the_larger_side() {
    // category 4 of g is in the test cases only: the rules must send it where the tree does
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
                 /VALIDATION TYPE=SPLITSAMPLE(tr) /SAVE NODEID(saved) /RULES NODES=TERMINAL /PRINT NONE /TREE DISPLAY=NONE."
            ),
        );
        let program = rule_lines(&tables).join("\n");
        run(&mut engine, &program);
        let (saved, node) = (values(&engine, "saved"), values(&engine, "NodeID"));
        assert_eq!(saved.len(), 150);
        for i in 0..150 {
            assert_eq!(node[i], saved[i], "{method}: case {i}\n{program}");
        }
    }
}

#[test]
fn selection_rules_pick_the_cases_of_the_nodes() {
    let mut engine = engine();
    let tables = run(
        &mut engine,
        &format!(
            "TREE y3 [n] BY {PREDICTORS} /METHOD TYPE=CRT {SMALL} /RULES TYPE=SELECTION /PRINT NONE /TREE DISPLAY=NONE."
        ),
    );
    run(&mut engine, &rule_lines(&tables).join("\n"));
    // every terminal node together holds every case: the filter keeps all of them
    let filter = values(&engine, "filter_$");
    assert_eq!(filter.len(), 150);
    assert!(filter.iter().all(|&f| f == 1.0), "{filter:?}");
}

const TINY: &str = "DATA LIST FREE / y a.
BEGIN DATA
2 3
2 3
2 2
2 3
1 1
1 1
1 3
1 1
1 2
1 1
2 1
1 1
2 3
2 1
2 2
1 1
1 1
2 2
1 1
2 2
END DATA.
VALUE LABELS y 1 'no' 2 'yes'.
VALUE LABELS a 1 'Low' 2 'Mid' 3 'High'.
VARIABLE LABELS a 'Spending level'.
";

fn tiny_rules(clause: &str) -> Vec<String> {
    let mut engine = Engine::default();
    run(&mut engine, TINY);
    let tables = run(
        &mut engine,
        &format!(
            "TREE y [n] BY a [n] /GROWTHLIMIT MINPARENTSIZE=6 MINCHILDSIZE=3 /RULES {clause} /PRINT NONE /TREE DISPLAY=NONE."
        ),
    );
    rule_lines(&tables)
}

#[test]
fn the_three_forms_of_the_rules_of_a_small_tree() {
    // generic: sentences, with the labels of the variable and its values, or without them
    assert_eq!(
        tiny_rules("SYNTAX=GENERIC"),
        [
            "Node 1: if Spending level is Low then the predicted y is no with probability 0.8",
            "Node 2: if Spending level is Mid or High then the predicted y is yes with probability 0.8",
        ]
    );
    assert_eq!(
        tiny_rules("SYNTAX=GENERIC LABELS=NO"),
        [
            "Node 1: if a is 1 then the predicted y is no with probability 0.8",
            "Node 2: if a is 2 or 3 then the predicted y is yes with probability 0.8",
        ]
    );
    assert_eq!(
        tiny_rules("SYNTAX=GENERIC TYPE=SELECTION"),
        ["Select the cases for which (Spending level is Low) or (Spending level is Mid or High)"]
    );
    // SQL: a CASE for each value of a node; the first node also takes the values no category
    // holds, as the tree sends them
    let unknown = "(a IN (1) OR (a IS NULL OR a NOT IN (1, 2, 3)))";
    assert_eq!(
        tiny_rules("SYNTAX=SQL"),
        [
            "/* Node 1 */".to_string(),
            format!(
                "SELECT CASE WHEN {unknown} THEN 1 END AS NodeID, CASE WHEN {unknown} THEN 1 END AS PredictedValue, CASE WHEN {unknown} THEN 0.8 END AS PredictedProbability;"
            ),
            "/* Node 2 */".to_string(),
            "SELECT CASE WHEN a IN (2, 3) THEN 2 END AS NodeID, CASE WHEN a IN (2, 3) THEN 2 END AS PredictedValue, CASE WHEN a IN (2, 3) THEN 0.8 END AS PredictedProbability;".to_string(),
        ]
    );
    assert_eq!(
        tiny_rules("SYNTAX=SQL TYPE=SELECTION"),
        [format!("WHERE ({unknown} OR a IN (2, 3))")]
    );
    // the command language: DO IF blocks that compute the node, the value and the probability
    let lines = tiny_rules("SYNTAX=INTERNAL");
    assert_eq!(lines[0], "/* Node 1 */.");
    assert_eq!(
        lines[1],
        "DO IF (ANY(a, 1) OR (SYSMIS(a) OR NOT ANY(a, 1, 2, 3)))."
    );
    assert_eq!(
        &lines[2..5],
        [
            "COMPUTE NodeID = 1.",
            "COMPUTE PredictedValue = 1.",
            "COMPUTE PredictedProbability = 0.8."
        ]
    );
    assert_eq!(lines[5], "END IF.");
    assert_eq!(lines[7], "DO IF ANY(a, 2, 3).");
    assert_eq!(lines.last().map(String::as_str), Some("EXECUTE."));
    assert_eq!(
        tiny_rules("SYNTAX=INTERNAL NODES=ALL TYPE=SELECTION"),
        [
            "COMPUTE filter_$ = (1 = 1).",
            "FILTER BY filter_$.",
            "EXECUTE."
        ]
    );
}

#[test]
fn string_values_are_quoted_and_a_string_dependent_is_declared() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        "DATA LIST FREE / y (A5) r (A6).\nBEGIN DATA\n\
         up north\nup north\nup north\nup north\nup south\ndown south\ndown south\ndown south\ndown south\ndown north\n\
         up north\nup north\nup north\ndown south\ndown south\ndown south\nup south\nup south\nup south\nup north\n\
         END DATA.",
    );
    let tables = run(
        &mut engine,
        "TREE y [n] BY r [n] /GROWTHLIMIT MINPARENTSIZE=6 MINCHILDSIZE=3 /RULES /PRINT NONE /TREE DISPLAY=NONE.",
    );
    let lines = rule_lines(&tables);
    assert_eq!(lines[0], "STRING PredictedValue (A4).");
    assert!(
        lines.iter().any(|l| l.contains("ANY(r, 'north')")),
        "{lines:?}"
    );
    assert!(
        lines.iter().any(|l| l == "COMPUTE PredictedValue = 'up'."),
        "{lines:?}"
    );
    // the rules run
    run(&mut engine, &lines.join("\n"));
}

#[test]
fn rules_that_are_not_ready_say_so() {
    let mut engine = engine();
    let (commands, _) =
        parse("TREE y [n] BY x1 [s] /RULES NODES=TOPN(2) /DEPCATEGORIES TARGET=[1].");
    let error = engine.execute(&commands[0].command).expect_err("an error");
    assert!(error.0.contains("not supported yet"), "{error}");
}
