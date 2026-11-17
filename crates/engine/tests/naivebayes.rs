//! NAIVEBAYES on invented data. The small example is worked by hand with fractions; the
//! selection examples come from a separate script that follows the published formulas
//! (smoothing 1/N, equal-width bins with empty ones merged, mean log-likelihood, forward
//! steps, pseudo-BIC and the test criterion).

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;
use std::f64::consts::LN_2;

/// y by a and b, b missing in the last case.
const HAND: &str = "DATA LIST FREE /y a b.
BEGIN DATA.
1 1 1
1 1 2
1 2 1
2 2 2
2 2 2
2 1 .
END DATA.
";

/// Twelve training cases: a goes with y, c a little, b and x hardly.
const SELECT: &str = "DATA LIST FREE /y a b c x t.
BEGIN DATA.
1 1 1 1 3.0 1
1 1 1 2 1.5 1
1 1 2 1 2.0 1
1 1 1 1 9.0 1
1 2 2 1 4.5 1
1 1 1 2 2.5 1
2 2 2 1 7.0 1
2 2 1 2 8.5 1
2 2 2 1 6.0 1
2 2 2 2 9.5 1
2 1 1 1 3.5 1
2 2 2 2 8.0 1
END DATA.
VARIABLE LEVEL x (SCALE).
";

/// The cases of SELECT and five test cases (t = 0).
const WITH_TEST: &str = "DATA LIST FREE /y a b c x t.
BEGIN DATA.
1 1 1 1 3.0 1
1 1 1 2 1.5 1
1 1 2 1 2.0 1
1 1 1 1 9.0 1
1 2 2 1 4.5 1
1 1 1 2 2.5 1
2 2 2 1 7.0 1
2 2 1 2 8.5 1
2 2 2 1 6.0 1
2 2 2 2 9.5 1
2 1 1 1 3.5 1
2 2 2 2 8.0 1
1 1 2 1 3.0 0
1 2 1 2 5.0 0
2 2 2 2 8.0 0
2 1 2 1 2.0 0
2 2 1 1 9.0 0
END DATA.
VARIABLE LEVEL x (SCALE).
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

fn fails(source: &str) -> String {
    let mut engine = Engine::default();
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.to_string();
        }
    }
    panic!("{source} ran without an error");
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
    (a - b).abs() <= 1e-12 * b.abs().max(1.0)
}

fn assert_all_close(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(close(*g, *w), "{got:?} != {want:?}");
    }
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

/// N = 6, so lambda = f = 1/6 and both priors (3 + 1/6) / (6 + 2/6) = 1/2. P(a=1 | y=1) =
/// (2 + 1/6) / (3 + 2/6) = 0.65 and P(a=1 | y=2) = 0.35; b has the same 0.65 and 0.35 for y=1
/// but for y=2 only two cases with b, both 2: P(b=1 | y=2) = (1/6) / (2 + 2/6) = 1/14. Case 1
/// (a 1, b 1): 0.65 * 0.65 against 0.35 / 14 gives 169/179; case 2: 0.65 * 0.35 against 0.35 *
/// 13/14 gives 7/17; case 3: 0.35 * 0.65 against 0.65 / 14 gives 49/59; cases 4 and 5: 0.35 *
/// 0.35 against 0.65 * 13/14 gives 343/2033; case 6 has no b and keeps 0.65.
#[test]
fn probabilities_and_classes_by_hand() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!("{HAND}NAIVEBAYES y BY a b /SUBSET NOSELECTION /SAVE PREDVAL PREDPROB."),
    );
    let first: Vec<f64> = saved(&engine, "PredictedProbability_1")
        .into_iter()
        .map(Option::unwrap)
        .collect();
    let want = [
        169.0 / 179.0,
        7.0 / 17.0,
        49.0 / 59.0,
        343.0 / 2033.0,
        343.0 / 2033.0,
        0.65,
    ];
    assert_all_close(&first, &want);
    let second = saved(&engine, "PredictedProbability_2");
    assert!(close(second[1].unwrap(), 10.0 / 17.0));
    let predicted: Vec<f64> = saved(&engine, "PredictedValue")
        .into_iter()
        .map(Option::unwrap)
        .collect();
    assert_eq!(predicted, [1.0, 2.0, 1.0, 2.0, 2.0, 1.0]);
    let classification = find(&tables, "Classification");
    assert_eq!(label(&classification.rows[0][0]), "Training");
    assert_eq!(column(classification, 2)[..2], [2.0, 1.0]);
    assert_eq!(column(classification, 3)[..2], [1.0, 2.0]);
    assert!(close(number(&classification.rows[2][4]), 400.0 / 6.0));
    let cps = find(&tables, "Case Processing Summary");
    assert_eq!(column(cps, 2), [6.0, 0.0, 6.0]);
    // NOSELECTION shows no selection tables
    assert!(
        tables
            .iter()
            .all(|t| t.title != "Predictor Selection Summary")
    );
}

/// A string target is saved as a string; PREDPROB(root:1) saves the first category only.
#[test]
fn string_target_and_probability_count() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        "DATA LIST FREE /y (A3) a b.\nBEGIN DATA.\nyes 1 1\nyes 1 2\nyes 2 1\nno 2 2\nno 2 2\nno 1 .\nEND DATA.\n\
         NAIVEBAYES y BY a b /SUBSET NOSELECTION /SAVE PREDVAL(guess) PREDPROB(p:1).",
    );
    let data = engine.dataset().unwrap();
    let Values::String(guess) = data.variable("guess").unwrap().values() else {
        panic!()
    };
    let guesses: Vec<&str> = (0..6).map(|c| guess.get(c).trim_end()).collect();
    assert_eq!(guesses, ["yes", "no", "yes", "no", "no", "yes"]);
    // "no" sorts first: case 1 has 1 - 169/179
    assert!(close(saved(&engine, "p_1")[0].unwrap(), 10.0 / 179.0));
    assert!(data.variable("p_2").is_err());
}

/// Forward steps over a, b, c and binned x (BINS=3: bounds 1.5 + 8/3 and 1.5 + 16/3, no empty
/// bin): a first, then c, x and b. The pseudo-BIC adds ln(12) / 24 per predictor and is lowest
/// after the first step.
#[test]
fn selection_order_and_pseudo_bic() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!("{SELECT}NAIVEBAYES y BY a b c WITH x /CRITERIA BINS=3 /SAVE PREDPROB."),
    );
    let summary = find(&tables, "Predictor Selection Summary");
    let added: Vec<String> = summary.rows.iter().map(|r| label(&r[1])).collect();
    assert_eq!(added, ["Initial subset", "a", "c", "x", "b"]);
    assert_eq!(column(summary, 2), [0.0, 1.0, 2.0, 3.0, 4.0]);
    assert_all_close(
        &column(summary, 3),
        &[
            -LN_2,
            -0.4508453134828209,
            -0.4488667658250826,
            -0.4884041908762557,
            -0.5548889826460645,
        ],
    );
    assert_eq!(summary.columns[4], "Pseudo-BIC");
    assert_all_close(
        &column(summary, 4),
        &[
            LN_2,
            0.5543830905573209,
            0.6559423199740826,
            0.7990175220997557,
            0.9690400909440645,
        ],
    );
    assert_eq!(
        summary.caption.as_deref(),
        Some("The subset of step 1 is the model.")
    );
    let selected = find(&tables, "Selected Predictors");
    assert_eq!(selected.rows.len(), 1);
    assert_eq!(label(&selected.rows[0][1]), "a");
    // with a alone: P(a=1 | y=1) = (5 + 1/12) / (6 + 2/12) = 61/74 and P(a=1 | y=2) = 13/74
    assert!(close(
        saved(&engine, "PredictedProbability_1")[0].unwrap(),
        61.0 / 74.0
    ));
}

/// EXACTSIZE stops after that many steps and keeps the last subset; MAXSIZE stops early but
/// still picks by the criterion; FORCE starts every subset with its predictors.
#[test]
fn exact_and_maximum_sizes_and_forced_predictors() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!("{SELECT}NAIVEBAYES y BY a b c WITH x /SUBSET EXACTSIZE=2 /CRITERIA BINS=3."),
    );
    let summary = find(&tables, "Predictor Selection Summary");
    assert_eq!(summary.rows.len(), 3);
    assert_eq!(summary.columns.len(), 4);
    assert_eq!(
        label(&find(&tables, "Selected Predictors").rows[0][1]),
        "a c"
    );

    let tables = run(
        &mut Engine::default(),
        &format!(
            "{SELECT}NAIVEBAYES y BY a b c WITH x /SUBSET MAXSIZE=3 (BESTSUBSET=PSEUDOBIC) /CRITERIA BINS=3."
        ),
    );
    assert_eq!(find(&tables, "Predictor Selection Summary").rows.len(), 4);
    assert_eq!(label(&find(&tables, "Selected Predictors").rows[0][1]), "a");

    let tables = run(
        &mut Engine::default(),
        &format!(
            "{SELECT}NAIVEBAYES y BY a c /FORCE COVARIATES=x /SUBSET EXACTSIZE=1 /CRITERIA BINS=3."
        ),
    );
    let summary = find(&tables, "Predictor Selection Summary");
    assert_eq!(column(summary, 2), [1.0, 2.0]);
    assert_all_close(
        &column(summary, 3),
        &[-0.53276475931356, -0.49080910054485893],
    );
    let selected = find(&tables, "Selected Predictors");
    assert_eq!(label(&selected.rows[0][1]), "a");
    assert_eq!(label(&selected.rows[1][1]), "x");
}

/// With test cases the test criterion is the default: minus the test mean log-likelihood is
/// lowest for the empty subset, so the model keeps the priors only; PSEUDOBIC still picks a.
#[test]
fn test_data_criterion() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{WITH_TEST}NAIVEBAYES y BY a b c WITH x /TRAININGSAMPLE VARIABLE=t /CRITERIA BINS=3."
        ),
    );
    let summary = find(&tables, "Predictor Selection Summary");
    assert_eq!(summary.columns[4], "Test Mean Log-likelihood");
    assert_eq!(summary.columns[5], "Test Criterion");
    let test = [
        -LN_2,
        -0.8115610317155684,
        -0.9161767255063813,
        -1.0839012847920944,
        -0.862269940973684,
    ];
    assert_all_close(&column(summary, 4), &test);
    assert_all_close(&column(summary, 5), &test.map(|t| -t));
    let selected = find(&tables, "Selected Predictors");
    assert!(selected.rows.is_empty());
    let cps = find(&tables, "Case Processing Summary");
    assert_eq!(column(cps, 2), [12.0, 5.0, 0.0, 17.0]);
    let classification = find(&tables, "Classification");
    assert_eq!(classification.rows.len(), 6);
    assert_eq!(label(&classification.rows[3][0]), "Test");

    let tables = run(
        &mut Engine::default(),
        &format!(
            "{WITH_TEST}NAIVEBAYES y BY a b c WITH x /TRAININGSAMPLE VARIABLE=t \
             /SUBSET MAXSIZE=AUTO (BESTSUBSET=PSEUDOBIC) /CRITERIA BINS=3 /SAVE PREDPROB."
        ),
    );
    assert_eq!(label(&find(&tables, "Selected Predictors").rows[0][1]), "a");
}

/// BINS=8 over x (1.5 to 9.5): bounds 2.5 to 8.5 by 1, the bin (4.5, 5.5] is empty and its
/// bounds give way to 5. OUTFILE writes them with the counts and probabilities.
#[test]
fn bins_merge_and_model_file() {
    let dir = std::env::temp_dir().join(format!("statscraft-naivebayes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    run(
        &mut Engine::default(),
        &format!(
            "{SELECT}CD '{}'.\nNAIVEBAYES y WITH x /SUBSET NOSELECTION /CRITERIA BINS=8 /OUTFILE MODEL='nb.json'.",
            dir.display()
        ),
    );
    let text = std::fs::read_to_string(dir.join("nb.json")).unwrap();
    let model: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(model["format"], "statscraft-naivebayes-model");
    // (6 + 1/12) / (12 + 2/12)
    assert!(close(model["priors"][0].as_f64().unwrap(), 0.5));
    let x = &model["predictors"][0];
    assert_eq!(
        x["bounds"],
        serde_json::json!([2.5, 3.5, 5.0, 6.5, 7.5, 8.5])
    );
    // bins of the cases: 1 0 0 6 2 0 | 4 5 3 6 1 5
    assert_eq!(
        x["counts"],
        serde_json::json!([
            [3.0, 0.0],
            [1.0, 1.0],
            [1.0, 0.0],
            [0.0, 1.0],
            [0.0, 1.0],
            [0.0, 2.0],
            [1.0, 1.0]
        ])
    );
    // (3 + 1/12) / (6 + 7/12)
    assert!(close(
        x["probabilities"][0][0].as_f64().unwrap(),
        37.0 / 79.0
    ));
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A random training sample is the same after the same SET SEED.
#[test]
fn seeded_training_sample_repeats() {
    let mut engine = Engine::default();
    let command = "NAIVEBAYES y BY a b c WITH x /TRAININGSAMPLE PERCENT=60 /SUBSET NOSELECTION /CRITERIA BINS=3";
    let first = run(
        &mut engine,
        &format!("{SELECT}SET SEED=20261117.\n{command} /SAVE PREDPROB(one).\n"),
    );
    let second = run(
        &mut engine,
        &format!("SET SEED=20261117.\n{command} /SAVE PREDPROB(two).\n"),
    );
    let cps = |tables: &[Table]| column(find(tables, "Case Processing Summary"), 2);
    assert_eq!(cps(&first), cps(&second));
    let (train, test) = (cps(&first)[0], cps(&first)[1]);
    assert!(
        train > 0.0 && test > 0.0 && train + test == 12.0,
        "{train} {test}"
    );
    assert_eq!(saved(&engine, "one_1"), saved(&engine, "two_1"));
}

/// Weights are rounded frequency weights: 1.6 counts as 2 and 0.4 leaves the case out, the
/// same as the first case twice and no sixth case.
#[test]
fn weights_round_to_whole_cases() {
    let mut weighted = Engine::default();
    let tables = run(
        &mut weighted,
        "DATA LIST FREE /y a b w.\nBEGIN DATA.\n1 1 1 1.6\n1 1 2 1\n1 2 1 1\n2 2 2 1\n2 2 2 1\n2 1 . 0.4\nEND DATA.\n\
         WEIGHT BY w.\nNAIVEBAYES y BY a b /SUBSET NOSELECTION /SAVE PREDPROB.",
    );
    assert_eq!(
        column(find(&tables, "Case Processing Summary"), 2),
        [6.0, 0.0, 6.0]
    );
    let mut copied = Engine::default();
    run(
        &mut copied,
        "DATA LIST FREE /y a b.\nBEGIN DATA.\n1 1 1\n1 1 1\n1 1 2\n1 2 1\n2 2 2\n2 2 2\nEND DATA.\n\
         NAIVEBAYES y BY a b /SUBSET NOSELECTION /SAVE PREDPROB.",
    );
    let w = saved(&weighted, "PredictedProbability_1");
    let c = saved(&copied, "PredictedProbability_1");
    assert!(close(w[1].unwrap(), c[2].unwrap()));
    assert!(close(w[3].unwrap(), c[4].unwrap()));
}

/// Without BY and WITH every other variable is a predictor by its level, except the
/// TRAININGSAMPLE variable and EXCEPT; a constant predictor is left out with a note.
#[test]
fn default_predictors_and_exclusions() {
    let tables = run(
        &mut Engine::default(),
        &format!(
            "{WITH_TEST}COMPUTE k = 1.\nNAIVEBAYES y /EXCEPT VARIABLES=b /TRAININGSAMPLE VARIABLE=t \
             /SUBSET NOSELECTION /CRITERIA BINS=3 /OUTFILE MODEL='{}'.",
            std::env::temp_dir()
                .join(format!("statscraft-nb-default-{}.json", std::process::id()))
                .display()
        ),
    );
    let excluded = find(&tables, "Excluded Predictors");
    assert_eq!(label(&excluded.rows[0][0]), "k");
    assert_eq!(label(&excluded.rows[0][2]), "Constant");
    let path =
        std::env::temp_dir().join(format!("statscraft-nb-default-{}.json", std::process::id()));
    let model: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    std::fs::remove_file(&path).unwrap();
    let names: Vec<&str> = model["predictors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["a", "c", "x"]);
}

#[test]
fn errors() {
    let weighted = format!("{HAND}WEIGHT BY a.\nNAIVEBAYES y BY b /TRAININGSAMPLE PERCENT=50.");
    assert!(fails(&weighted).contains("PERCENT"));
    assert!(fails(&format!("{SELECT}NAIVEBAYES x BY a.")).contains("nominal or ordinal"));
    assert!(
        fails(&format!("{SELECT}NAIVEBAYES y BY a b /SUBSET EXACTSIZE=2.")).contains("EXACTSIZE")
    );
    assert!(fails(&format!("{SELECT}NAIVEBAYES y BY a b /SUBSET MAXSIZE=3.")).contains("MAXSIZE"));
    assert!(
        fails(&format!(
            "{SELECT}NAIVEBAYES y BY a /SUBSET MAXSIZE=1 (BESTSUBSET=TESTDATA)."
        ))
        .contains("TESTDATA")
    );
    assert!(fails(&format!("{SELECT}NAIVEBAYES y BY a b WITH b.")).contains("both"));
    assert!(fails(&format!("{SELECT}NAIVEBAYES y BY a /FORCE FACTORS=y.")).contains("FORCE"));
    assert!(fails(&format!("{SELECT}NAIVEBAYES y BY a /SAVE PREDVAL(x).")).contains("exists"));
}
