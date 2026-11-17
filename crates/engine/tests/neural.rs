//! MLP and RBF on invented data. The expected results do not depend on luck: a problem that
//! a straight line separates is classified perfectly, a seeded run repeats itself exactly, and
//! the RBF networks used here have least squares solutions worked out by hand (one normalized
//! unit predicts the training mean; two units on two distinct input points reproduce the two
//! group means).

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Two groups of a class a and b, apart along x + y; z is noise for nothing; part is the
/// partition by sign.
const DATA: &str = "DATA LIST FREE /x y z part (F8.2) grp (A1).
BEGIN DATA.
-2.1 -1.8 0.3 1 a
-1.7 -2.4 0.9 1 a
-2.6 -2.0 0.1 1 a
-1.9 -1.5 0.4 1 a
-2.3 -2.7 0.8 1 a
-1.4 -2.2 0.2 0 a
-2.8 -1.6 0.6 -1 a
-1.6 -1.9 0.5 1 a
2.2 1.9 0.7 1 b
1.8 2.5 0.2 1 b
2.6 2.1 0.9 1 b
1.5 1.7 0.3 1 b
2.4 2.8 0.1 1 b
1.9 1.4 0.6 0 b
2.9 2.2 0.4 -1 b
1.7 2.0 0.8 1 b
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

fn fails(engine: &mut Engine, source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    commands
        .iter()
        .find_map(|c| engine.execute(&c.command).err())
        .expect("an error")
        .to_string()
}

fn numbers(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().expect("data");
    match data.variable(name).expect(name).values() {
        Values::Numeric(v) => v.iter().collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

fn texts(engine: &Engine, name: &str) -> Vec<String> {
    let data = engine.dataset().expect("data");
    match data.variable(name).expect(name).values() {
        Values::String(v) => v.iter().map(|s| s.trim_end().to_string()).collect(),
        Values::Numeric(_) => panic!("{name} is numeric"),
    }
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn cell(t: &Table, row: usize, col: usize) -> f64 {
    match &t.rows[row][col] {
        Cell::Number(n) => *n,
        other => panic!("{other:?} at {row},{col} of {}", t.title),
    }
}

#[test]
fn mlp_classifies_a_separable_problem_perfectly() {
    let mut e = Engine::default();
    let tables = run(
        &mut e,
        &format!(
            "{DATA}SET SEED=7.
MLP grp BY WITH x y /PARTITION VARIABLE=part
  /ARCHITECTURE AUTOMATIC=NO HIDDENLAYERS=1 (NUMUNITS=2)
  /PRINT CPS NETWORKINFO SUMMARY CLASSIFICATION SOLUTION IMPORTANCE /PLOT NETWORK ROC GAIN LIFT PREDICTED
  /SAVE PREDVAL PSEUDOPROB."
        ),
    );
    // 12 training, 2 testing, 2 holdout cases
    let cps = table(&tables, "Case Processing Summary");
    assert_eq!(
        [cell(cps, 0, 2), cell(cps, 1, 2), cell(cps, 2, 2)],
        [12.0, 2.0, 2.0]
    );
    let summary = table(&tables, "Model Summary");
    let incorrect: Vec<f64> = summary
        .rows
        .iter()
        .filter(|r| matches!(&r[1], Cell::Text(t) if t == "Percent Incorrect Predictions"))
        .map(|r| match r[2] {
            Cell::Number(n) => n,
            _ => panic!(),
        })
        .collect();
    assert_eq!(incorrect, vec![0.0, 0.0, 0.0]);
    assert_eq!(texts(&e, "MLP_PredictedValue"), texts(&e, "grp"));
    // softmax pseudo-probabilities sum to 1 and favour the observed group
    let (pa, pb) = (
        numbers(&e, "MLP_PseudoProbability_1"),
        numbers(&e, "MLP_PseudoProbability_2"),
    );
    for i in 0..16 {
        let (a, b) = (pa[i].unwrap(), pb[i].unwrap());
        assert!((a + b - 1.0).abs() < 1e-12);
        assert_eq!(a > b, i < 8);
    }
    let auc = table(&tables, "Area Under the Curve");
    assert_eq!(cell(auc, 0, 2), 1.0);
    let importance = table(&tables, "Independent Variable Importance");
    let total = cell(importance, 0, 1) + cell(importance, 1, 1);
    assert!((total - 1.0).abs() < 1e-12);
    assert!(table(&tables, "Parameter Estimates").rows.len() == 3 + 3);
    let charts = e.take_charts();
    assert_eq!(
        charts.len(),
        6,
        "importance, network, boxes, ROC, gains, lift"
    );
}

#[test]
fn a_seeded_run_repeats_itself_exactly() {
    let syntax = format!(
        "{DATA}SET SEED=4242.
MLP x (MLEVEL=S) BY grp WITH y z /CRITERIA TRAINING=MINIBATCH MINIBATCHSIZE=3
  /STOPPINGRULES MAXEPOCHS=30 /PRINT NONE /PLOT NONE /SAVE PREDVAL(xhat)."
    );
    let mut first = Engine::default();
    run(&mut first, &syntax);
    let mut second = Engine::default();
    run(&mut second, &syntax);
    let a = numbers(&first, "xhat");
    assert!(a.iter().all(Option::is_some));
    assert_eq!(a, numbers(&second, "xhat"));
}

#[test]
fn rbf_one_normalized_unit_predicts_the_training_mean() {
    let mut e = Engine::default();
    let tables = run(
        &mut e,
        &format!(
            "{DATA}RBF z (MLEVEL=S) WITH x y /PARTITION VARIABLE=part /ARCHITECTURE NUMUNITS=1
  /PRINT SUMMARY /PLOT NONE /SAVE PREDVAL."
        ),
    );
    // the training cases (part = 1) of z: their mean is the prediction for every case
    let z = numbers(&e, "z");
    let part = numbers(&e, "part");
    let train: Vec<f64> = (0..16)
        .filter(|&i| part[i] == Some(1.0))
        .map(|i| z[i].unwrap())
        .collect();
    let mean = train.iter().sum::<f64>() / train.len() as f64;
    for p in numbers(&e, "RBF_PredictedValue") {
        assert!((p.unwrap() - mean).abs() < 1e-12);
    }
    // standardized z predicted by 0: the training error is half of n - 1, the relative error 1
    let summary = table(&tables, "Model Summary");
    assert!((cell(summary, 0, 2) - 0.5 * (train.len() as f64 - 1.0)).abs() < 1e-9);
    assert!((cell(summary, 1, 2) - 1.0).abs() < 1e-12);
}

#[test]
fn rbf_two_units_on_two_points_give_the_group_means() {
    let mut e = Engine::default();
    run(
        &mut e,
        "DATA LIST FREE /x y.
BEGIN DATA.
0 1.0
0 1.4
0 0.6
0 1.0
10 5.2
10 4.8
10 5.0
10 5.0
END DATA.
RBF y (MLEVEL=S) WITH x /PARTITION TRAINING=1 TESTING=0 /ARCHITECTURE NUMUNITS=2 HIDDENFUNCTION=ORBF
  /PRINT SOLUTION /PLOT NONE /SAVE PREDVAL(yhat).",
    );
    let got = numbers(&e, "yhat");
    for (i, p) in got.iter().enumerate() {
        let want = if i < 4 { 1.0 } else { 5.0 };
        assert!((p.unwrap() - want).abs() < 1e-9, "{got:?}");
    }
}

#[test]
fn rbf_classifies_the_separable_problem_and_writes_its_model() {
    let dir = std::env::temp_dir().join(format!("neural-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("rbf.json");
    let mut e = Engine::default();
    let tables = run(
        &mut e,
        &format!(
            "{DATA}RBF grp BY WITH x y /PARTITION VARIABLE=part /ARCHITECTURE NUMUNITS=2
  /PRINT CLASSIFICATION /PLOT NONE /SAVE PREDVAL /OUTFILE MODEL='{}'.",
            file.display()
        ),
    );
    assert_eq!(texts(&e, "RBF_PredictedValue"), texts(&e, "grp"));
    let classification = table(&tables, "Classification");
    assert_eq!(cell(classification, 2, 4), 100.0);
    let model: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(model["format"], "statscraft-rbf-model");
    assert_eq!(model["dependent"], "grp");
    assert_eq!(model["network"]["centers"].as_array().unwrap().len(), 2);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn neural_refuses_bad_variables() {
    let mut e = Engine::default();
    run(&mut e, DATA);
    assert!(fails(&mut e, "MLP grp (MLEVEL=S) WITH x.").contains("string"));
    assert!(fails(&mut e, "MLP x WITH x y.").contains("predictor"));
    assert!(fails(&mut e, "MLP x BY grp WITH grp.").contains("both"));
    assert!(fails(&mut e, "MLP x WITH grp.").contains("string"));
    assert!(
        fails(
            &mut e,
            "MLP x (MLEVEL=S) WITH y /ARCHITECTURE AUTOMATIC=NO OUTPUTFUNCTION=SOFTMAX."
        )
        .contains("SOFTMAX")
    );
    assert!(fails(&mut e, "MLP x WITH y /PARTITION VARIABLE=y.").contains("partition"));
}
