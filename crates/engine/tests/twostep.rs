//! TWOSTEP CLUSTER and DETECTANOMALY on synthetic data. The expected numbers come from an
//! independent implementation of the published formulas (pre-clustering tree, log-likelihood
//! distance, brute-force joining, BIC and AIC with the two-stage count, noise critical values,
//! anomaly index); the small example is also worked through by hand, formula by formula.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Shop customers: id, spend, visits, region (1 to 3) and plan (a or b); case 31 misses spend.
const DATA: &str = "DATA LIST FREE /id spend visits region (F1) plan (A1).
BEGIN DATA.
1 21.0 2.8 1 b
2 53.6 7.4 2 b
3 92.2 4.0 3 b
4 12.9 1.4 1 a
5 61.0 9.3 2 b
6 96.0 4.2 3 b
7 24.1 2.0 1 a
8 59.3 7.1 2 b
9 97.2 4.5 3 b
10 19.5 2.0 1 b
11 58.4 9.2 2 b
12 105.5 5.0 3 b
13 7.0 3.9 1 a
14 61.5 11.5 2 b
15 100.5 4.8 3 b
16 13.9 4.6 1 a
17 52.2 7.2 2 b
18 97.9 1.6 3 b
19 17.4 3.4 1 a
20 51.4 7.8 2 b
21 93.4 3.2 3 b
22 15.3 0.9 1 a
23 67.8 8.7 3 b
24 115.5 4.7 3 b
25 11.3 2.4 1 a
26 50.8 8.1 2 b
27 97.8 2.9 3 b
28 24.4 2.7 1 a
29 74.2 7.5 3 a
30 99.6 1.6 3 b
31 . 5.0 2 a
END DATA.
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

fn label(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => n.to_string(),
        Cell::Empty => String::new(),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

fn column(table: &Table, index: usize) -> Vec<f64> {
    table.rows.iter().map(|r| number(&r[index])).collect()
}

fn saved(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    let Values::Numeric(values) = data.variable(name).unwrap().values() else {
        panic!("{name} is not numeric")
    };
    (0..data.case_count()).map(|c| values.get(c)).collect()
}

fn assert_all_close(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(close(*g, *w), "{got:?} != {want:?}");
    }
}

#[test]
fn mixed_variables_by_bic() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        "TWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend visits /CATEGORICAL VARIABLES=region plan \
         /PRINT IC COUNT SUMMARY /SAVE CLUSTER.",
    );
    let auto = find(&tables, "Auto-Clustering");
    assert_eq!(auto.rows.len(), 15);
    let bic = [
        166.1519603076016,
        113.1762552392389,
        98.02250937607636,
        111.76971820356297,
        127.30270859295615,
        148.23373657591634,
        170.70588893226883,
        193.3297749933661,
        216.3227949252944,
        239.66160526174886,
        263.07314115834714,
        286.5076414191922,
        310.11068429054393,
        333.803258475942,
        357.52019026109014,
    ];
    assert_all_close(&column(auto, 1), &bic);
    assert!(close(number(&auto.rows[2][4]), 3.8725234262780606));
    assert!(close(number(&auto.rows[4][4]), 2.876042425654297));
    assert!(close(
        number(&auto.rows[2][3]),
        (bic[2] - bic[1]) / (bic[1] - bic[0])
    ));
    assert!(
        auto.caption
            .as_ref()
            .unwrap()
            .contains("Chosen: 3 clusters")
    );
    let summary = find(&tables, "Model Summary");
    assert_eq!(number(&summary.rows[2][1]), 3.0);

    let counts = find(&tables, "Cluster Distribution");
    assert_eq!(column(counts, 2), [10.0, 8.0, 12.0, 30.0, 1.0, 31.0]);
    assert!(close(number(&counts.rows[0][4]), 1000.0 / 31.0));
    let centroids = find(&tables, "Centroids");
    assert!(close(number(&centroids.rows[0][1]), 16.68));
    assert!(close(number(&centroids.rows[0][2]), 5.660349419917074));
    assert!(close(number(&centroids.rows[2][2]), 12.743625857659193));
    let plan = find(&tables, "plan");
    // cluster 1: 8 of plan a, 2 of plan b
    assert_eq!(number(&plan.rows[0][1]), 8.0);
    assert!(close(number(&plan.rows[0][4]), 20.0));

    let numbers: Vec<Option<f64>> = [
        1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 2, 3, 1, 3, 3, 1, 2, 3, 1, 3, 3,
    ]
    .iter()
    .map(|&n| Some(f64::from(n)))
    .chain([None])
    .collect();
    assert_eq!(saved(&engine, "TSC_1"), numbers);
    // a second run saves under the next free name
    run_on(
        &mut engine,
        "TWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend /VIEWMODEL DISPLAY=NO /SAVE CLUSTER.",
    );
    assert!(engine.dataset().unwrap().variable("TSC_2").is_ok());
}

#[test]
fn aic_with_a_smaller_maximum() {
    let tables = run(
        &mut Engine::default(),
        "TWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend visits /CATEGORICAL VARIABLES=region plan \
         /NUMCLUSTERS AUTO 6 AIC /PRINT IC.",
    );
    let auto = find(&tables, "Auto-Clustering");
    assert_eq!(auto.columns[1], "AIC");
    assert_all_close(
        &column(auto, 1),
        &[
            156.3435786359665,
            93.55949189596872,
            68.59736436117109,
            72.53619151702262,
            78.2608002347807,
            89.3834465461058,
        ],
    );
}

/// x = 0 1 5 6 with categories a a b b, worked by hand: the two pairs join at distance
/// ln(1 + 0.25 / s) with s = 26/3, then at 2 ln((s + 6.5) / (s + 0.25)) + 4 ln 2.
#[test]
fn hand_worked_example() {
    let data = "DATA LIST FREE /x (F2) c (A1).\nBEGIN DATA.\n0 a 1 a 5 b 6 b\nEND DATA.\n";
    let tables = run_on(
        &mut Engine::default(),
        &format!(
            "{data}TWOSTEP CLUSTER /CONTINUOUS VARIABLES=x /CATEGORICAL VARIABLES=c \
             /NOSTANDARDIZE VARIABLES=x /PRINT IC COUNT /SAVE CLUSTER VARIABLE=grp.\n\
             TWOSTEP CLUSTER /CONTINUOUS VARIABLES=x /CATEGORICAL VARIABLES=c \
             /NOSTANDARDIZE VARIABLES=x /NUMCLUSTERS AUTO AIC /PRINT IC."
        ),
    );
    let autos: Vec<&Table> = tables
        .iter()
        .filter(|t| t.title == "Auto-Clustering")
        .collect();
    assert_all_close(
        &column(autos[0], 1),
        &[
            20.580460676994413,
            17.069454905414965,
            21.17146211813357,
            25.273469330852173,
        ],
    );
    assert!(close(number(&autos[0].rows[1][4]), 134.8531243300411));
    assert_all_close(
        &column(autos[1], 1),
        &[
            22.42157759363474,
            20.751688738695623,
            26.694812868054555,
            32.63793699741349,
        ],
    );
    assert_eq!(
        column(find(&tables, "Cluster Distribution"), 2),
        [2.0, 2.0, 4.0, 0.0, 4.0]
    );
}

/// OUTFILE writes the final clusters (MODEL) and the tree's subclusters (STATE) as JSON, in the
/// standardized units the clustering used, with the mean and standard deviation to get there.
#[test]
fn outfile_writes_model_and_state() {
    let dir = std::env::temp_dir().join(format!("statscraft-twostep-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let data = format!(
        "DATA LIST FREE /x (F2) c (A1).\nBEGIN DATA.\n0 a 1 a 5 b 6 b\nEND DATA.\nCD '{}'.\n",
        dir.display()
    );
    run_on(
        &mut Engine::default(),
        &format!(
            "{data}TWOSTEP CLUSTER /CONTINUOUS VARIABLES=x /CATEGORICAL VARIABLES=c \
             /NUMCLUSTERS FIXED=2 /VIEWMODEL DISPLAY=NO /OUTFILE MODEL='m.json' STATE='s.json'."
        ),
    );
    let read = |name: &str| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join(name)).unwrap()).unwrap()
    };
    let (model, state) = (read("m.json"), read("s.json"));
    let sd = (26.0f64 / 3.0).sqrt();
    assert_eq!(model["format"], "statscraft-twostep-model");
    assert_eq!(model["distance"], "likelihood");
    let x = &model["continuous"][0];
    assert_eq!(
        (x["name"].as_str(), x["mean"].as_f64()),
        (Some("x"), Some(3.0))
    );
    assert!(close(x["sd"].as_f64().unwrap(), sd));
    assert!(close(x["variance"].as_f64().unwrap(), 1.0));
    assert_eq!(
        model["categorical"][0]["categories"],
        serde_json::json!(["a", "b"])
    );
    let clusters = model["clusters"].as_array().unwrap();
    assert_eq!(clusters.len(), 2);
    for (cluster, centre, counts) in [
        (&clusters[0], 0.5, [2.0, 0.0]),
        (&clusters[1], 5.5, [0.0, 2.0]),
    ] {
        assert_eq!(cluster["cases"], 2);
        let summary = &cluster["summary"];
        assert_eq!(summary["n"].as_f64(), Some(2.0));
        assert!(close(
            summary["mean"][0].as_f64().unwrap(),
            (centre - 3.0) / sd
        ));
        assert!(close(summary["ss"][0].as_f64().unwrap(), 0.5 / (sd * sd)));
        assert_eq!(summary["counts"][0], serde_json::json!(counts));
    }
    assert_eq!(state["format"], "statscraft-twostep-state");
    assert_eq!(
        (state["max_branch"].as_u64(), state["max_level"].as_u64()),
        (Some(8), Some(3))
    );
    let subs = state["subclusters"].as_array().unwrap();
    let n: f64 = subs.iter().map(|s| s["n"].as_f64().unwrap()).sum();
    assert_eq!(n, 4.0);

    let missing = dir.join("no-such-folder").join("m.json");
    assert!(
        fails(&format!(
            "TWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend /OUTFILE MODEL='{}'.",
            missing.display()
        ))
        .contains("cannot write the model file")
    );
    assert!(
        fails("SPLIT FILE BY plan.\nTWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend /OUTFILE STATE='s.json'.")
            .contains("SPLIT FILE")
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn euclidean_fixed_count() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        "TWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend visits /DISTANCE EUCLIDEAN \
         /NUMCLUSTERS FIXED=3 /PRINT IC /SAVE CLUSTER VARIABLE=grp.",
    );
    let warnings = engine.warnings().to_vec();
    assert!(
        warnings.iter().any(|w| w.contains("PRINT IC")),
        "{warnings:?}"
    );
    let want: Vec<Option<f64>> = (0..30)
        .map(|i| Some(f64::from(i % 3 + 1)))
        .chain([None])
        .collect();
    assert_eq!(saved(&engine, "grp"), want);
}

/// A tree of at most 4 subclusters (MXBRANCH 2, one level below the root) fills and is rebuilt.
#[test]
fn small_tree_with_noise_and_case_order() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        "TWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend visits /CATEGORICAL VARIABLES=region \
         /CRITERIA MXBRANCH(2) MXLEVEL(1) /PRINT IC /SAVE CLUSTER VARIABLE=a.\n\
         TWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend visits /CATEGORICAL VARIABLES=region \
         /CRITERIA MXBRANCH(2) MXLEVEL(1) /SAVE CLUSTER VARIABLE=b.\n\
         TWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend visits /DISTANCE EUCLIDEAN /HANDLENOISE 25 \
         /CRITERIA MXBRANCH(2) MXLEVEL(1) /NUMCLUSTERS FIXED 3 /PRINT COUNT /SAVE CLUSTER VARIABLE=c.",
    );
    assert_all_close(
        &column(find(&tables, "Auto-Clustering"), 1),
        &[126.09890480264579, 88.42520227131592, 70.92680516697827],
    );
    // the same order gives the same clusters
    assert_eq!(saved(&engine, "a"), saved(&engine, "b"));
    let first: Vec<Option<f64>> = (0..30)
        .map(|i| Some(if i % 3 == 0 { 1.0 } else { 2.0 }))
        .chain([None])
        .collect();
    assert_eq!(saved(&engine, "a"), first);
    // Euclidean noise: cases farther than the critical distance go to -1
    let c = saved(&engine, "c");
    let noise: Vec<usize> = (0..31)
        .filter(|&i| c[i] == Some(-1.0))
        .map(|i| i + 1)
        .collect();
    assert_eq!(noise, [5, 11, 14, 18, 27, 30]);
    let counts = find(&tables, "Cluster Distribution");
    assert_eq!(label(&counts.rows[2][1]), "Outlier (-1)");
    assert_eq!(column(counts, 2), [10.0, 14.0, 6.0, 30.0, 1.0, 31.0]);
    let warnings = engine.warnings().to_vec();
    assert!(
        warnings.iter().any(|w| w.contains("fewer than the 3")),
        "{warnings:?}"
    );
}

#[test]
fn twostep_errors() {
    assert!(fails("TWOSTEP CLUSTER /CONTINUOUS VARIABLES=plan /PRINT COUNT.").contains("string"));
    assert!(
        fails(
            "TWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend /CATEGORICAL VARIABLES=spend /PRINT COUNT."
        )
        .contains("both")
    );
    assert!(
        fails("TWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend /VIEWMODEL DISPLAY=NO.")
            .contains("nothing to show")
    );
    assert!(
        fails("TWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend /SAVE CLUSTER VARIABLE=visits.")
            .contains("already exists")
    );
}

#[test]
fn anomalies_with_reasons_and_saved_variables() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        "DETECTANOMALY /VARIABLES SCALE=spend visits CATEGORICAL=region plan ID=id \
         /CRITERIA NUMREASONS=2 PCTANOMALOUSCASES=10 ANOMALYCUTPOINT=1 \
         /PRINT CPS ANOMALYLIST NORMS ANOMALYSUMMARY REASONSUMMARY \
         /SAVE ANOMALY PEERID PEERSIZE REASONVAR REASONMEASURE REASONVALUE(val) REASONNORM.",
    );
    let index = find(&tables, "Anomaly Case Index List");
    assert_eq!(column(index, 0), [29.0, 10.0, 1.0]);
    assert_all_close(
        &column(index, 2),
        &[3.3527438367398434, 2.00214981204756, 1.9487162766085475],
    );
    let peers = find(&tables, "Anomaly Case Peer ID List");
    assert_eq!(column(peers, 2), [2.0, 1.0, 1.0]);
    assert_eq!(column(peers, 3), [10.0, 10.0, 10.0]);
    let first = find(&tables, "Anomaly Case Reason List: Reason 1");
    assert_eq!(label(&first.rows[0][2]), "plan");
    assert!(close(number(&first.rows[0][3]), 0.4838462750826757));
    assert_eq!(label(&first.rows[0][4]), "a");
    assert_eq!(label(&first.rows[0][5]), "b");
    let second = find(&tables, "Anomaly Case Reason List: Reason 2");
    assert_eq!(label(&second.rows[0][2]), "region");
    assert!(close(number(&second.rows[0][3]), 0.35498623735195023));
    assert!(close(number(&second.rows[2][3]), 0.051918012398148236));
    let cps = find(&tables, "Case Processing Summary");
    assert_eq!(column(cps, 2), [10.0, 10.0, 10.0, 30.0, 1.0, 31.0]);
    let norms = find(&tables, "Scale Variable Norms");
    assert!(close(number(&norms.rows[0][2]), 16.68));
    assert!(close(number(&norms.rows[0][3]), 59.019999999999996));
    let categorical = find(&tables, "Categorical Variable Norms");
    assert_eq!(label(&categorical.rows[3][2]), "a");
    assert_eq!(number(&categorical.rows[4][2]), 8.0);
    let summary = find(&tables, "Anomaly Index Summary");
    assert_eq!(number(&summary.rows[0][1]), 3.0);
    assert!(close(number(&summary.rows[0][3]), 3.3527438367398434));
    let reasons = find(&tables, "Reason Summary: Reason 1");
    assert_eq!(label(&reasons.rows[0][0]), "plan");
    assert_eq!(number(&reasons.rows[0][1]), 3.0);

    let anomaly = saved(&engine, "AnomalyIndex");
    assert!(close(anomaly[13].unwrap(), 1.5085792660254713));
    assert_eq!(anomaly[30], None);
    assert_eq!(saved(&engine, "PeerId")[28], Some(2.0));
    assert!(close(
        saved(&engine, "ReasonMeasure_2")[13].unwrap(),
        0.11576306648541639
    ));
    let data = engine.dataset().unwrap();
    let Values::String(why) = data.variable("ReasonVar_1").unwrap().values() else {
        panic!()
    };
    assert_eq!(why.get(13).trim_end(), "visits");
    let Values::String(value) = data.variable("val_1").unwrap().values() else {
        panic!()
    };
    assert_eq!(value.get(28).trim_end(), "a");
}

#[test]
fn missing_values_handled_and_counted() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        "MISSING VALUES region (9).\nDO IF id = 5.\nCOMPUTE region = 9.\nEND IF.\n\
         DETECTANOMALY /VARIABLES SCALE=spend visits CATEGORICAL=region plan \
         /HANDLEMISSING APPLY=YES CREATEMISPROPVAR=YES /CRITERIA PCTANOMALOUSCASES=10 ANOMALYCUTPOINT=NONE \
         /SAVE ANOMALY.",
    );
    let index = find(&tables, "Anomaly Case Index List");
    assert_eq!(column(index, 0), [10.0, 31.0, 1.0]);
    assert_all_close(
        &column(index, 1),
        &[1.9861177025525845, 1.9467007302810753, 1.9325930404902758],
    );
    let reasons = find(&tables, "Anomaly Case Reason List: Reason 1");
    assert_eq!(label(&reasons.rows[1][1]), "Missing Proportion");
    assert!(close(number(&reasons.rows[1][2]), 0.45839936615748467));
    assert!(close(
        saved(&engine, "AnomalyIndex")[4].unwrap(),
        1.7784802376839788
    ));
}

/// A peer group of one case: its index is the mean group size over its size, (10 / 2) / 1.
#[test]
fn single_case_peer_group() {
    let data = "DATA LIST FREE /x.\nBEGIN DATA.\n1 1 1 1 1 8 9 10 11 30\nEND DATA.\n";
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!(
            "{data}VARIABLE LEVEL x (SCALE).\nWEIGHT BY x.\nDETECTANOMALY /CRITERIA MINNUMPEERS=2 MAXNUMPEERS=2 PCTANOMALOUSCASES=100 ANOMALYCUTPOINT=NONE /SAVE ANOMALY."
        ),
    );
    assert_eq!(column(find(&tables, "Anomaly Case Index List"), 0)[0], 10.0);
    let index = saved(&engine, "AnomalyIndex");
    assert_eq!(index[9], Some(5.0));
    assert!(close(index[0].unwrap(), 0.8557153154601614));
    assert!(close(index[8].unwrap(), 1.707555452126092));
    let warnings = engine.warnings().to_vec();
    assert!(
        warnings.iter().any(|w| w.contains("weight")),
        "{warnings:?}"
    );
}

/// OUTFILE MODEL writes the inputs (with the mean that stands in for a missing value and the
/// categories) and the peer groups with their mean group deviation.
#[test]
fn detectanomaly_outfile_writes_the_model() {
    let dir = std::env::temp_dir().join(format!("statscraft-anomaly-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    run(
        &mut Engine::default(),
        &format!(
            "CD '{}'.\nDETECTANOMALY /VARIABLES SCALE=spend CATEGORICAL=region ID=id \
             /HANDLEMISSING APPLY=YES /OUTFILE MODEL='a.json'.",
            dir.display()
        ),
    );
    let text = std::fs::read_to_string(dir.join("a.json")).unwrap();
    let model: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(model["format"], "statscraft-anomaly-model");
    assert_eq!(model["missing_applied"], true);
    let spend: Vec<f64> = DATA
        .lines()
        .skip(2)
        .filter_map(|l| l.split(' ').nth(1)?.parse().ok())
        .collect();
    assert_eq!(spend.len(), 30);
    let variables = model["variables"].as_array().unwrap();
    assert_eq!(
        (variables[0]["name"].as_str(), variables[0]["kind"].as_str()),
        (Some("spend"), Some("scale"))
    );
    assert!(close(
        variables[0]["mean"].as_f64().unwrap(),
        spend.iter().sum::<f64>() / 30.0
    ));
    assert_eq!(
        variables[1]["categories"],
        serde_json::json!([1.0, 2.0, 3.0])
    );
    assert_eq!(variables[1]["missing_category"], false);
    let peers = model["peer_groups"].as_array().unwrap();
    let n: f64 = peers
        .iter()
        .map(|p| p["summary"]["n"].as_f64().unwrap())
        .sum();
    assert_eq!(n, 31.0);
    assert!(
        peers
            .iter()
            .all(|p| p["mean_deviation"].as_f64().unwrap() >= 0.0)
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn detectanomaly_errors_and_split_file() {
    assert!(fails("DETECTANOMALY /VARIABLES SCALE=spend CATEGORICAL=spend.").contains("both"));
    assert!(fails("DETECTANOMALY /VARIABLES SCALE=plan.").contains("string"));
    assert!(fails("DETECTANOMALY /VARIABLES SCALE=spend ID=spend.").contains("ID variable"));
    assert!(
        fails("DETECTANOMALY /VARIABLES SCALE=spend /SAVE ANOMALY(visits).")
            .contains("already exists")
    );
    // EXCEPT applies before the overlap check
    let mut engine = Engine::default();
    run(
        &mut engine,
        "SORT CASES BY region.\nSPLIT FILE BY region.\n\
         DETECTANOMALY /VARIABLES SCALE=spend visits CATEGORICAL=plan EXCEPT=visits ID=id /PRINT CPS.",
    );
    let warnings = engine.warnings().to_vec();
    assert!(
        warnings.iter().any(|w| w.contains("SPLIT FILE")),
        "{warnings:?}"
    );
}
