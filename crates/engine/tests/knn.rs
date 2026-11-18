//! KNN on small invented data. Every expected number is worked out by hand from the published
//! rules (distances, votes with their tie rules, Laplace probabilities, means and medians,
//! fold errors, forward selection and importance); the random partition follows the
//! documented congruential generator, stepped by hand from SET SEED.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// One covariate x; cases 1 to 6 train (part 1), 7 and 8 are held out. Class c follows x except
/// for case 8; y is a scale target; z mixes the classes; fold splits the training cases in two.
const DATA: &str = "DATA LIST FREE /id x z c y part fold focal (8F4.1).
BEGIN DATA.
1 0 0 1 10 1 1 0
2 1 5 1 12 1 2 0
3 2 1 1 14 1 1 0
4 5 6 2 30 1 2 0
5 6 2 2 34 1 1 0
6 7 7 2 38 1 2 0
7 1.4 3 1 13 0 . 1
8 5.2 4 1 31 0 . 0
END DATA.
FORMATS c (F1).
";

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

fn run(engine: &mut Engine, syntax: &str) -> Vec<Table> {
    run_on(engine, &format!("{DATA}{syntax}"))
}

fn fails(syntax: &str) -> String {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
    assert!(errors.is_empty(), "{errors:?}");
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.to_string();
        }
    }
    panic!("{syntax} ran without an error");
}

fn find<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables.iter().find(|t| t.title == title).unwrap_or_else(|| {
        panic!(
            "no table {title} in {:?}",
            tables.iter().map(|t| &t.title).collect::<Vec<_>>()
        )
    })
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn text(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => n.to_string(),
        Cell::Empty => String::new(),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

fn saved(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    let Values::Numeric(values) = data.variable(name).unwrap().values() else {
        panic!("{name} is not numeric")
    };
    (0..data.case_count()).map(|c| values.get(c)).collect()
}

fn assert_saved(engine: &Engine, name: &str, want: &[Option<f64>]) {
    let got = saved(engine, name);
    assert_eq!(got.len(), want.len());
    for (g, w) in got.iter().zip(want) {
        assert!(
            match (g, w) {
                (Some(g), Some(w)) => close(*g, *w),
                (None, None) => true,
                _ => false,
            },
            "{name}: {got:?} != {want:?}"
        );
    }
}

/// The row of `table` whose first cells read `labels`.
fn row<'t>(table: &'t Table, labels: &[&str]) -> &'t [Cell] {
    table
        .rows
        .iter()
        .find(|r| labels.iter().enumerate().all(|(i, l)| text(&r[i]) == *l))
        .unwrap_or_else(|| panic!("no row {labels:?} in {}", table.title))
}

#[test]
fn classification_by_partition_variable() {
    // k = 3 on x alone. Training cases are predicted from the other five: all right. Case 7
    // (x 1.4) has neighbors 2, 3, 1 at 0.4, 0.6, 1.4, all class 1: probability (3+1)/(3+2).
    // Case 8 (x 5.2) has neighbors 4, 5, 6, class 2, but is class 1.
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        "KNN c (MLEVEL=N) WITH x /PARTITION VARIABLE=part /RESCALE COVARIATE=NONE
          /FOCALCASES VARIABLE=focal /SAVE PREDVAL PREDPROB.",
    );
    let cps = find(&tables, "Case Processing Summary");
    assert_eq!(number(&row(cps, &["Sample", "Training"])[2]), 6.0);
    assert_eq!(number(&row(cps, &["Sample", "Holdout"])[2]), 2.0);
    assert_eq!(number(&row(cps, &["Valid"])[2]), 8.0);
    let errors = find(&tables, "Error Summary");
    assert_eq!(number(&row(errors, &["Training"])[2]), 0.0);
    assert_eq!(number(&row(errors, &["Holdout"])[2]), 50.0);
    let classes = find(&tables, "Classification Table");
    let held = row(classes, &["Holdout", "1"]);
    assert_eq!(
        held[2..].iter().map(number).collect::<Vec<_>>(),
        [1.0, 1.0, 50.0]
    );
    let overall = row(classes, &["Holdout", "Overall Percent"]);
    assert_eq!(
        overall[2..].iter().map(number).collect::<Vec<_>>(),
        [50.0, 50.0, 50.0]
    );
    let near = find(&tables, "Nearest Neighbors and Distances");
    assert_eq!(near.rows.len(), 1);
    let cells: Vec<f64> = near.rows[0].iter().map(number).collect();
    let want = [7.0, 2.0, 3.0, 1.0, 0.4, 0.6, 1.4];
    assert!(
        cells.iter().zip(want).all(|(g, w)| close(*g, w)),
        "{cells:?}"
    );
    let one = Some(1.0);
    let two = Some(2.0);
    assert_saved(
        &engine,
        "KNN_PredictedValue",
        &[one, one, one, two, two, two, one, two],
    );
    // case 1: neighbors 2, 3, 4 give 2 votes to class 1 and 1 to class 2
    let p1 = saved(&engine, "KNN_Probability_1");
    assert!(close(p1[0].unwrap(), 3.0 / 5.0) && close(p1[6].unwrap(), 4.0 / 5.0));
    assert!(close(
        saved(&engine, "KNN_Probability_2")[7].unwrap(),
        4.0 / 5.0
    ));
}

#[test]
fn scale_target_mean_and_median() {
    // Neighbors as in the classification test; training sums of squares by hand:
    // mean: (26/3)^2 + 6^2 + (10/3)^2 + (4/3)^2 + (20/3)^2 + 12^2 = 2812/9;
    // median: 4^2 + 2^2 + 2^2 + 4^2 + 4^2 + 8^2 = 120; holdout 1 + 9 both ways.
    for (median, training) in [("MEAN", 2812.0 / 9.0), ("MEDIAN", 120.0)] {
        let mut engine = Engine::default();
        let tables = run(
            &mut engine,
            &format!(
                "KNN y (MLEVEL=S) WITH x /PARTITION VARIABLE=part /RESCALE COVARIATE=NONE
                  /CRITERIA PREDICTED={median} /SAVE PREDVAL(guess)."
            ),
        );
        let errors = find(&tables, "Error Summary");
        assert!(close(number(&row(errors, &["Training"])[2]), training));
        assert!(close(number(&row(errors, &["Holdout"])[2]), 10.0));
        assert!(tables.iter().all(|t| t.title != "Classification Table"));
        let want: [f64; 2] = if median == "MEAN" {
            [56.0 / 3.0, 12.0]
        } else {
            [14.0, 12.0]
        };
        let got = saved(&engine, "guess");
        assert!(close(got[0].unwrap(), want[0]) && close(got[6].unwrap(), want[1]));
    }
}

#[test]
fn euclidean_and_city_block_with_rescaling() {
    // Rescaled x: 0.2 x - 1, z: 0.5 z - 1, g coded (1,0) or (0,1). From case 1: case 3 differs
    // by (1, 1, 0, 0), case 5 by (1.5, 0, 0, 0), case 4 by (0.4, 0.5, 1, 1), case 2 by
    // (2, 2, 1, 1). Euclid: 3 at sqrt 2, then 5 at 1.5; city block: 5 at 1.5, then 3 at 2.
    let data = "DATA LIST FREE /g (A1) x z f.
BEGIN DATA.
a 0 0 1
b 10 4 0
a 5 2 0
b 2 1 0
a 7.5 0 0
END DATA.
";
    for (metric, want) in [
        ("EUCLID", [1.0, 3.0, 5.0, 2f64.sqrt(), 1.5]),
        ("CITYBLOCK", [1.0, 5.0, 3.0, 1.5, 2.0]),
    ] {
        let mut engine = Engine::default();
        let tables = run_on(
            &mut engine,
            &format!(
                "{data}KNN BY g WITH x z /PARTITION TRAINING=1 HOLDOUT=0 /FOCALCASES VARIABLE=f
                  /MODEL METRIC={metric} NEIGHBORS=FIXED(K=2)."
            ),
        );
        let near = find(&tables, "Nearest Neighbors and Distances");
        let cells: Vec<f64> = near.rows[0].iter().map(number).collect();
        assert!(
            cells.iter().zip(want).all(|(g, w)| close(*g, w)),
            "{metric}: {cells:?}"
        );
        assert!(tables.iter().all(|t| t.title != "Error Summary"));
    }
}

#[test]
fn k_by_cross_validation_folds() {
    // Folds 1 = cases 1, 3, 5 and 2 = cases 2, 4, 6, predicted from the other fold. Errors of
    // fold 1 and 2: k=1 0 and 0; k=2 0 and 2/3 (two-way ties go to class 1: equal training
    // counts, lower value); k=3 2/3 and 2/3. Means 0, 1/3, 2/3: k = 1.
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        "KNN c (MLEVEL=N) WITH x /PARTITION VARIABLE=part /RESCALE COVARIATE=NONE
          /MODEL NEIGHBORS=AUTO(KMIN=1, KMAX=3) /CROSSVALIDATION VARIABLE=fold.",
    );
    let log = find(&tables, "k Selection Error Log");
    let errors: Vec<f64> = log.rows.iter().map(|r| number(&r[1])).collect();
    let want = [0.0, 100.0 / 3.0, 200.0 / 3.0];
    assert!(
        errors.iter().zip(want).all(|(g, w)| close(*g, w)),
        "{errors:?}"
    );
    let summary = find(&tables, "Model Summary");
    assert_eq!(
        text(&row(summary, &["Number of Nearest Neighbors (k)"])[1]),
        "1"
    );
}

#[test]
fn feature_selection_and_importance() {
    // k = 1 without rescaling. Alone, x errs on no training case and z on half (cases 2, 4, 5
    // find a case of the other class nearest). With both, only case 5 errs (cases 3 and 4 are
    // equally near; 3 comes first): 1/6.
    let selection = |rule: &str| {
        let mut engine = Engine::default();
        run(
            &mut engine,
            &format!(
                "KNN c (MLEVEL=N) WITH x z /PARTITION VARIABLE=part /RESCALE COVARIATE=NONE
                  /MODEL NEIGHBORS=FIXED(K=1) FEATURES=AUTO /CRITERIA NUMFEATURES={rule}."
            ),
        )
    };
    let tables = selection("AUTO");
    let log = find(&tables, "Feature Selection Error Log");
    assert_eq!(log.rows.len(), 2);
    assert_eq!(text(&log.rows[0][1]), "x");
    assert_eq!(number(&log.rows[0][3]), 0.0);
    assert_eq!(text(&log.rows[1][1]), "z");
    assert!(close(number(&log.rows[1][3]), 100.0 / 6.0));
    // importance: without x the error is 1/2, without z 0; plus 1/2 each, then scaled
    let importance = find(&tables, "Feature Importance");
    assert!(close(number(&row(importance, &["x"])[1]), 2.0 / 3.0));
    assert!(close(number(&row(importance, &["z"])[1]), 1.0 / 3.0));
    // a fixed count of one, and the error ratio rule that stops once the error is 0
    for rule in ["FIXED(1)", "ERRORRATIO"] {
        let tables = selection(rule);
        assert_eq!(find(&tables, "Feature Selection Error Log").rows.len(), 1);
        let summary = find(&tables, "Model Summary");
        assert_eq!(text(&row(summary, &["Features"])[1]), "x");
    }
    // forcing z: x is added and brings the error from 1/2 to 1/6
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        "KNN c (MLEVEL=N) WITH x z /PARTITION VARIABLE=part /RESCALE COVARIATE=NONE
          /MODEL NEIGHBORS=FIXED(K=1) FEATURES=AUTO(FORCE=z) /CRITERIA NUMFEATURES=FIXED(1).",
    );
    let log = find(&tables, "Feature Selection Error Log");
    assert_eq!(text(&log.rows[0][1]), "x");
    assert_eq!(number(&log.rows[0][2]), 2.0);
    // k from 1 to 2 with selection: x alone errs on no case with either k, so k = 1 stays
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        "KNN c (MLEVEL=N) WITH x z /PARTITION VARIABLE=part /RESCALE COVARIATE=NONE
          /MODEL NEIGHBORS=AUTO(KMIN=1, KMAX=2) FEATURES=AUTO /CRITERIA NUMFEATURES=FIXED(1).",
    );
    let summary = find(&tables, "Model Summary");
    assert_eq!(
        text(&row(summary, &["Number of Nearest Neighbors (k)"])[1]),
        "1"
    );
    assert_eq!(find(&tables, "Feature Selection Error Log").rows.len(), 2);
}

#[test]
fn weighted_features() {
    // k = 2: alone, x errs on no training case and z on four (two-way ties go to class 1), so
    // the importance is 4/6 + 1/2 for x and 0 + 1/2 for z, scaled: 0.7 and 0.3. Case 7 at
    // (1.4, 3): case 2 at (1, 5) is sqrt(0.7 0.16 + 0.3 4) away, case 3 at (2, 1)
    // sqrt(0.7 0.36 + 0.3 4); city block 0.7 0.4 + 0.3 2 and 0.7 0.6 + 0.3 2.
    for (metric, d2, d3) in [
        ("EUCLID", 1.312f64.sqrt(), 1.452f64.sqrt()),
        ("CITYBLOCK", 0.88, 1.02),
    ] {
        let mut engine = Engine::default();
        let tables = run(
            &mut engine,
            &format!(
                "KNN c (MLEVEL=N) WITH x z /PARTITION VARIABLE=part /RESCALE COVARIATE=NONE
                  /FOCALCASES VARIABLE=focal /MODEL METRIC={metric} NEIGHBORS=FIXED(K=2)
                  /CRITERIA WEIGHTFEATURES=YES."
            ),
        );
        let importance = find(&tables, "Feature Importance");
        assert!(close(number(&row(importance, &["x"])[1]), 0.7));
        assert!(close(number(&row(importance, &["z"])[1]), 0.3));
        let near = find(&tables, "Nearest Neighbors and Distances");
        let cells: Vec<f64> = near.rows[0].iter().map(number).collect();
        let want = [7.0, 2.0, 3.0, d2, d3];
        assert!(
            cells.iter().zip(want).all(|(g, w)| close(*g, w)),
            "{metric}: {cells:?}"
        );
    }
}

#[test]
fn seeded_random_partition_and_folds() {
    // Seed 12345 gives the draws 0.3629, 0.7452, 0.8311, 0.2763, 0.1838, 0.7289, 0.0779,
    // 0.7343: below 0.7 trains.
    let syntax = "SET SEED=12345.
KNN c (MLEVEL=N) WITH x /SAVE PARTITION.";
    let mut engine = Engine::default();
    run(&mut engine, syntax);
    let (t, h) = (Some(1.0), Some(0.0));
    assert_saved(&engine, "KNN_Partition", &[t, h, h, t, t, h, t, h]);
    // folds: the same seed gives the same assignment, two folds of three
    let syntax = "SET SEED=777.
KNN c (MLEVEL=N) WITH x /PARTITION TRAINING=1 HOLDOUT=0 /MODEL NEIGHBORS=AUTO(KMIN=1, KMAX=2)
  /CROSSVALIDATION FOLDS=2 /SAVE FOLD.";
    let mut first = Engine::default();
    run(&mut first, syntax);
    let mut second = Engine::default();
    run(&mut second, syntax);
    let folds = saved(&first, "KNN_Fold");
    assert_eq!(folds, saved(&second, "KNN_Fold"));
    assert_eq!(folds.iter().filter(|f| **f == Some(1.0)).count(), 4);
    assert_eq!(folds.iter().filter(|f| **f == Some(2.0)).count(), 4);
}

#[test]
fn neighbors_only_and_unscored_holdout() {
    // without a target only neighbors are found; AUTO is dropped with a warning
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        "KNN WITH x /PARTITION VARIABLE=part /RESCALE COVARIATE=NONE /FOCALCASES VARIABLE=focal
          /MODEL NEIGHBORS=AUTO.",
    );
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("NEIGHBORS=AUTO"))
    );
    assert_eq!(
        find(&tables, "Nearest Neighbors and Distances").rows[0].len(),
        7
    );
    assert!(tables.iter().all(|t| t.title != "Error Summary"));
    // a holdout case of a category no training case has is not scored
    let data = "DATA LIST FREE /g y part.
BEGIN DATA.
1 1 1
1 1 1
2 2 1
2 2 1
3 1 0
1 1 0
END DATA.
";
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{data}KNN y (MLEVEL=N) BY g /PARTITION VARIABLE=part /MODEL NEIGHBORS=FIXED(K=1)
              /SAVE PREDVAL PARTITION."
        ),
    );
    assert!(engine.warnings().iter().any(|w| w.contains("not scored")));
    let pred = saved(&engine, "KNN_PredictedValue");
    assert_eq!(pred[4], None);
    assert_eq!(pred[5], Some(1.0));
    // PARTITION repeats the partition variable and is not saved
    assert!(engine.dataset().unwrap().variable("KNN_Partition").is_err());
}

#[test]
fn errors() {
    assert!(fails("KNN c WITH x /MODEL FEATURES=AUTO(FORCE=x).").contains("FORCE"));
    assert!(
        fails("KNN c WITH x /PARTITION VARIABLE=part /MODEL NEIGHBORS=FIXED(K=6).")
            .contains("neighbors")
    );
    assert!(
        fails("COMPUTE none = 0.\nKNN c WITH x /FOCALCASES VARIABLE=none.").contains("no positive")
    );
    assert!(fails("KNN c BY x WITH x.").contains("both"));
    assert!(fails("KNN c WITH c.").contains("target"));
    assert!(fails("KNN c WITH x /PARTITION VARIABLE=x.").contains("partition"));
}

/// OUTFILE writes the JSON model and, for each focal case, its neighbors and distances (case 7:
/// cases 2, 3, 1 at 0.4, 0.6, 1.4). A string target is saved as text.
#[test]
fn outfiles_and_string_target() {
    let dir = std::env::temp_dir().join(format!("statscraft-knn-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut engine = Engine::default();
    run(
        &mut engine,
        &format!(
            "CD '{}'.
STRING s (A3).
RECODE c (1='low') (2='hi') INTO s.
DATASET DECLARE near.
KNN s WITH x /PARTITION VARIABLE=part /RESCALE COVARIATE=NONE /FOCALCASES VARIABLE=focal
  /SAVE PREDVAL /OUTFILE MODEL='knn.json' FOCALCASES=near.",
            dir.display()
        ),
    );
    let data = engine.dataset().unwrap();
    let Values::String(values) = data.variable("KNN_PredictedValue").unwrap().values() else {
        panic!("not text")
    };
    let got: Vec<&str> = (0..8).map(|c| values.get(c).trim_end()).collect();
    assert_eq!(got, ["low", "low", "low", "hi", "hi", "hi", "low", "hi"]);
    let model: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("knn.json")).unwrap()).unwrap();
    assert_eq!(model["format"], "statscraft-knn-model");
    assert_eq!(model["k"], 3);
    assert_eq!(
        model["target"]["categories"],
        serde_json::json!(["hi", "low"])
    );
    assert_eq!(model["training"].as_array().unwrap().len(), 6);
    let (named, _) = engine.named_datasets();
    let near = named
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case("near"))
        .and_then(|(_, d)| d.as_ref())
        .unwrap();
    assert_eq!(near.case_count(), 1);
    let value = |name: &str| {
        let Values::Numeric(v) = near.variable(name).unwrap().values() else {
            panic!("{name}")
        };
        v.get(0).unwrap()
    };
    assert_eq!(value("KNN_FocalCase_CaseNumber"), 7.0);
    assert_eq!(value("KNN_NearestNeighbor_CaseNumber_3"), 1.0);
    assert!(close(value("KNN_NearestNeighbor_Distance_2"), 0.6));
}

/// 20 cases drawn once in R (seed 4417); 15 train, 5 are held out. The expected numbers come
/// from R as a black box: the covariates rescaled to 2 (x - min) / (max - min) - 1 over the
/// training cases and g coded one column per category, then class::knn.cv (each training case
/// from the other training cases) and class::knn (the holdout cases) with k = 3, and the
/// neighbor distances from dist(). Votes v of class 1 give P(1) = (v + 1) / (3 + 2).
#[test]
fn defaults_agree_with_r() {
    let data = "DATA LIST FREE /id x1 x2 g cls part focal.
BEGIN DATA.
1 33.7 -0.24 2 1 1 1
2 46.3 3.58 1 1 1 0
3 51.1 5.78 2 2 1 0
4 47.6 2.15 1 1 1 0
5 40.7 3.91 2 1 1 0
6 11.7 0.28 1 1 1 0
7 31.2 4.66 2 1 1 0
8 19.8 3.92 1 1 1 0
9 21.1 7.82 2 2 1 0
10 11.5 -2.17 1 1 1 0
11 10.6 8.96 1 2 1 0
12 33.3 6.76 2 2 1 0
13 18 3.22 2 1 1 0
14 15.6 5.94 1 1 1 0
15 59.5 4.49 1 1 1 0
16 41.5 2.41 2 1 0 0
17 43.1 7.75 2 2 0 1
18 24.8 2.33 1 1 0 1
19 20.6 5.07 1 1 0 0
20 20.1 8.17 2 2 0 0
END DATA.
";
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!(
            "{data}KNN cls (MLEVEL=N) BY g WITH x1 x2 /PARTITION VARIABLE=part
              /FOCALCASES VARIABLE=focal /SAVE PREDVAL PREDPROB."
        ),
    );
    let classes = [1, 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 1, 2, 1, 1, 1, 2, 1, 1, 2];
    let votes = [3, 3, 2, 3, 1, 3, 2, 2, 2, 3, 2, 2, 1, 2, 2, 3, 1, 3, 2, 1];
    assert_saved(
        &engine,
        "KNN_PredictedValue",
        &classes.map(|c| Some(f64::from(c))),
    );
    assert_saved(
        &engine,
        "KNN_Probability_1",
        &votes.map(|v| Some((f64::from(v) + 1.0) / 5.0)),
    );
    let near = find(&tables, "Nearest Neighbors and Distances");
    let want: [[f64; 7]; 3] = [
        [
            1.0,
            5.0,
            7.0,
            13.0,
            0.798801268604739,
            0.886420185744829,
            0.8938071478106,
        ],
        [
            17.0,
            12.0,
            3.0,
            5.0,
            0.438523217999373,
            0.482051342883371,
            0.696973806350635,
        ],
        [
            18.0,
            8.0,
            6.0,
            14.0,
            0.351358057915086,
            0.650205570460603,
            0.749928862592391,
        ],
    ];
    assert_eq!(near.rows.len(), 3);
    for (row, want) in near.rows.iter().zip(want) {
        let cells: Vec<f64> = row.iter().map(number).collect();
        assert!(
            cells.iter().zip(want).all(|(g, w)| close(*g, w)),
            "{cells:?}"
        );
    }
}
