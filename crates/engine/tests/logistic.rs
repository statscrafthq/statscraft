//! LOGISTIC REGRESSION on synthetic data. The expected numbers come from an independent fit by
//! iteratively reweighted least squares in a general statistics package: the coefficients and
//! their covariance, the likelihoods of the nested models, score tests written out from their
//! matrix formula, the Hosmer-Lemeshow grouping rule written out separately, and the case
//! diagnostics from the package's hat values. PSPP comparisons are in reference_models.rs.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

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

fn run_error(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.0;
        }
    }
    panic!("no error");
}

/// The row headers of every row joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                let text = match cell {
                    Cell::Text(t) => t.clone(),
                    Cell::Number(n) => n.to_string(),
                    Cell::Empty => continue,
                };
                carried[level] = text;
                for deeper in carried.iter_mut().skip(level + 1) {
                    deeper.clear();
                }
            }
            let parts: Vec<&str> = carried
                .iter()
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .collect();
            parts.join(" / ")
        })
        .collect()
}

/// The cell of the last table with this title (block 0 and block 1 share titles; the block-0
/// rows say Step 0).
fn cell<'t>(tables: &'t [Table], title: &str, row: &str, column: &str) -> &'t Cell {
    for table in tables.iter().rev().filter(|t| t.title == title) {
        let Some(r) = row_keys(table).iter().position(|k| k == row) else {
            continue;
        };
        let c = table
            .column_titles()
            .iter()
            .position(|t| t == column)
            .unwrap_or_else(|| {
                panic!("{title}: no column {column} in {:?}", table.column_titles())
            });
        return &table.rows[r][c];
    }
    panic!("no table {title} with row {row}");
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn close(actual: f64, expected: f64) -> bool {
    (actual - expected).abs() <= 1e-7 * expected.abs().max(1.0)
}

fn values(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => (0..data.case_count()).map(|k| v.get(k)).collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

/// y, x1, x2, g (three categories) and a non-integer weight w; 60 cases drawn from a logistic
/// model.
const DATA: &str = "DATA LIST FREE /y x1 x2 g w.
BEGIN DATA.
0 25.3 0.9 2 1
0 56.5 -1.96 1 1
1 61.0 0.3 1 2
0 46.6 -0.72 1 1
1 49.7 -0.32 3 2
1 57.3 0.61 3 1
0 57.5 -0.87 3 0.5
0 47.4 -1.53 1 1
1 54.5 1.07 1 0.5
1 52.6 -0.48 2 0.5
1 57.4 -0.26 1 0.5
0 48.9 -0.35 3 1.5
1 51.5 0.64 2 0.5
1 57.7 -1.28 1 1
0 42.2 0.11 3 1
0 46.1 -0.94 3 1.5
1 48.8 -0.48 3 2
0 46.4 0.17 1 2
1 70.5 -1.5 1 2
1 55.0 -0.4 2 1
1 69.7 -0.77 2 1
0 26.1 -0.71 2 2
0 30.7 0.87 3 1.5
0 45.0 0.39 2 2
1 50.8 1.22 1 1
0 57.5 -0.58 1 1
0 52.7 1.71 3 1.5
0 50.0 -1.56 1 1.5
0 33.0 0.01 3 1
0 47.3 0.72 1 1.5
1 64.1 -0.29 1 1.5
0 47.6 -0.88 2 1
1 59.1 -0.9 2 1
0 36.7 0.0 2 1
1 49.7 -1.08 1 0.5
0 35.9 -2.41 3 1
1 50.2 0.73 2 1.5
1 51.0 -2.07 2 1.5
1 56.1 -0.04 3 0.5
1 66.1 1.54 2 1
1 55.2 1.68 2 1
1 47.9 1.66 1 1
1 48.0 -0.65 2 2
0 34.0 -0.63 1 0.5
0 65.7 -0.97 3 2
0 62.3 0.45 2 1
1 41.1 -0.05 2 1.5
0 40.7 -1.54 1 0.5
1 68.6 -2.53 2 0.5
0 44.3 -0.41 3 1.5
1 57.3 -0.05 2 1.5
0 36.5 -0.32 1 1
0 37.8 0.35 2 0.5
0 48.2 -0.62 2 1.5
0 43.0 0.56 3 0.5
1 68.5 1.93 3 1
1 46.1 1.25 3 1
0 31.1 -1.84 3 1
1 59.8 0.48 2 2
0 43.0 0.01 3 0.5
END DATA.";

/// (case, table, row, column, value)
const EXPECTED: &[(&str, &str, &str, &str, f64)] = &[
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / x1",
        "B",
        0.1702392147451744,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / x1",
        "S.E.",
        0.047657272656125876,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / x1",
        "Wald",
        12.76029883057479,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / x1",
        "Sig.",
        0.0003540543275016911,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / x1",
        "Exp(B)",
        1.1855884276347937,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / x2",
        "B",
        0.6787935481782691,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / x2",
        "S.E.",
        0.36896118934649086,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / x2",
        "Wald",
        3.38465214007396,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / x2",
        "Sig.",
        0.06580606380672295,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / x2",
        "Exp(B)",
        1.971497779782181,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / g(1)",
        "B",
        0.592247016238253,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / g(2)",
        "S.E.",
        0.8783494822701224,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / Constant",
        "B",
        -9.27198248780323,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / g",
        "Wald",
        3.5218558727714027,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / g",
        "Sig.",
        0.1718852911820162,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / x1",
        "95% C.I.for EXP(B) / Lower",
        1.07986137534719,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 1 / x1",
        "95% C.I.for EXP(B) / Upper",
        1.3016670026646862,
    ),
    (
        "enter",
        "Model Summary",
        "Step 1",
        "-2 Log likelihood",
        53.18534573096454,
    ),
    (
        "enter",
        "Model Summary",
        "Step 1",
        "Cox & Snell R Square",
        0.3927171494409847,
    ),
    (
        "enter",
        "Model Summary",
        "Step 1",
        "Nagelkerke R Square",
        0.5238170160277873,
    ),
    (
        "enter",
        "Omnibus Tests of Model Coefficients",
        "Step 1 / Model",
        "Chi-square",
        29.925636918392982,
    ),
    (
        "enter",
        "Omnibus Tests of Model Coefficients",
        "Step 1 / Model",
        "Sig.",
        5.068040584337591e-06,
    ),
    (
        "enter",
        "Variables not in the Equation",
        "Step 0 / x1",
        "Score",
        19.009196873973607,
    ),
    (
        "enter",
        "Variables not in the Equation",
        "Step 0 / g",
        "Score",
        4.205949193947546,
    ),
    (
        "enter",
        "Variables not in the Equation",
        "Step 0 / g(2)",
        "Score",
        3.2575269704243395,
    ),
    (
        "enter",
        "Variables not in the Equation",
        "Step 0 / Overall Statistics",
        "Score",
        23.606569412125904,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 0 / Constant",
        "B",
        -0.06669137449867232,
    ),
    (
        "enter",
        "Variables in the Equation",
        "Step 0 / Constant",
        "S.E.",
        0.25834245322210986,
    ),
    (
        "enter",
        "Correlation Matrix",
        "Step 1 / x1",
        "Constant",
        -0.9679587223819358,
    ),
    (
        "enter",
        "Correlation Matrix",
        "Step 1 / g(2)",
        "g(1)",
        0.5490356076584136,
    ),
    (
        "enter",
        "Classification Table",
        "Step 1 / 0",
        "Predicted / 0",
        25.0,
    ),
    (
        "enter",
        "Classification Table",
        "Step 1 / 1",
        "Predicted / 1",
        23.0,
    ),
    (
        "enter",
        "Classification Table",
        "Step 1 / Overall Percentage",
        "Predicted / Percentage Correct",
        80.0,
    ),
    (
        "enter",
        "Hosmer and Lemeshow Test",
        "Step 1",
        "Chi-square",
        6.627412399356501,
    ),
    ("enter", "Hosmer and Lemeshow Test", "Step 1", "df", 8.0),
    (
        "weighted",
        "Variables in the Equation",
        "Step 1 / x1",
        "B",
        0.15110262108975053,
    ),
    (
        "weighted",
        "Variables in the Equation",
        "Step 1 / g(2)",
        "S.E.",
        0.7600668088702925,
    ),
    (
        "weighted",
        "Model Summary",
        "Step 1",
        "-2 Log likelihood",
        67.08644321233969,
    ),
    (
        "weighted",
        "Model Summary",
        "Step 1",
        "Nagelkerke R Square",
        0.46359528027407043,
    ),
    (
        "weighted",
        "Classification Table",
        "Step 1 / 1",
        "Predicted / 1",
        25.5,
    ),
    (
        "helmert",
        "Variables in the Equation",
        "Step 1 / g(1)",
        "B",
        -0.3450334540068471,
    ),
    (
        "helmert",
        "Variables in the Equation",
        "Step 1 / g(2)",
        "B",
        1.3824982110606427,
    ),
    (
        "interaction",
        "Variables in the Equation",
        "Step 1 / x1 by g(1)",
        "B",
        0.11194655349391205,
    ),
    (
        "interaction",
        "Variables in the Equation",
        "Step 1 / x1 by g(2)",
        "S.E.",
        0.09924128960912412,
    ),
    (
        "interaction",
        "Variables in the Equation",
        "Step 1 / g(1)",
        "B",
        -5.392384567909539,
    ),
    (
        "origin",
        "Variables in the Equation",
        "Step 1 / x2",
        "B",
        0.43791957126485653,
    ),
    (
        "origin",
        "Model Summary",
        "Step 1",
        "Nagelkerke R Square",
        0.07221221329168959,
    ),
    (
        "origin",
        "Omnibus Tests of Model Coefficients",
        "Step 1 / Model",
        "Chi-square",
        3.3408581577111107,
    ),
    (
        "fstep",
        "Variables not in the Equation",
        "Step 1 / x2",
        "Score",
        3.091328112793504,
    ),
    (
        "fstep",
        "Variables not in the Equation",
        "Step 1 / g",
        "Score",
        3.259608973180263,
    ),
    (
        "fstep",
        "Model Summary",
        "Step 2",
        "-2 Log likelihood",
        57.005528244540955,
    ),
    (
        "fstep",
        "Omnibus Tests of Model Coefficients",
        "Step 2 / Step",
        "Chi-square",
        3.20294287567242,
    ),
    (
        "fstep",
        "Omnibus Tests of Model Coefficients",
        "Step 3 / Block",
        "Chi-square",
        29.925636918392982,
    ),
    (
        "fstep",
        "Model if Term Removed",
        "Step 3 / x2",
        "Change in -2 Log Likelihood",
        3.7388472672117388,
    ),
    (
        "fstep",
        "Model if Term Removed",
        "Step 3 / g",
        "Change in -2 Log Likelihood",
        3.8201825135764125,
    ),
    (
        "fstep",
        "Model if Term Removed",
        "Step 3 / g",
        "Model Log Likelihood",
        -28.502764122270477,
    ),
    (
        "bstep",
        "Model if Term Removed",
        "Step 1 / g",
        "Change in -2 Log Likelihood",
        3.9223823070179407,
    ),
    (
        "bstep",
        "Model if Term Removed",
        "Step 1 / x2",
        "Change in -2 Log Likelihood",
        3.816078977721503,
    ),
    (
        "bstep",
        "Model if Term Removed",
        "Step 2 / x2",
        "Change in -2 Log Likelihood",
        3.2538142189382455,
    ),
];

/// (case index, PRED RESID DEV LRESID SRESID ZRESID LEVER COOK DFBETA0..4)
const SAVED: &[(usize, [f64; 13])] = &[
    (
        0,
        [
            0.06064070009080462,
            -0.06064070009080462,
            -0.35371522995601606,
            -1.0645553837564248,
            -0.3700350216339249,
            -0.25407751525159555,
            0.08626163381806687,
            0.00609436254477327,
            -0.16029178093246688,
            0.0031496317323782886,
            -0.004223866430554129,
            -0.005688183895655247,
            -0.016091963407958924,
        ],
    ),
    (
        4,
        [
            0.2634123717674219,
            0.7365876282325781,
            1.6334224935831696,
            3.7963289016771884,
            1.7056537752335543,
            1.6722227428417509,
            0.08290293388696207,
            0.25278008035124544,
            0.5130918530320061,
            -0.0037206376622118875,
            -0.046878248431968396,
            -0.33358831503061326,
            -0.34768214173833667,
        ],
    ),
    (
        16,
        [
            0.21583085663756738,
            0.7841691433624326,
            1.7511483370523986,
            4.633257800015333,
            1.8237231168404384,
            1.906110647369489,
            0.0780060613965147,
            0.3073947876996277,
            0.6642981807756869,
            -0.006283853565487615,
            -0.07157185369373598,
            -0.3587032465858279,
            -0.3824333971908303,
        ],
    ),
];

/// The syntax of each case of EXPECTED, run after the data. BCON is tight so that the estimates
/// are converged to the last digit compared.
const CASES: &[(&str, &str)] = &[
    (
        "enter",
        "LOGISTIC REGRESSION y WITH x1 x2 g /CATEGORICAL=g /PRINT=CI(95) CORR GOODFIT \
         /CRITERIA=BCON(1E-10) /SAVE=PRED RESID DEV LRESID SRESID ZRESID LEVER COOK DFBETA PGROUP.",
    ),
    (
        "weighted",
        "WEIGHT BY w.\nLOGISTIC REGRESSION y WITH x1 x2 g /CATEGORICAL=g /CRITERIA=BCON(1E-10).",
    ),
    (
        "helmert",
        "LOGISTIC REGRESSION y WITH x1 g /CATEGORICAL=g /CONTRAST(g)=HELMERT /CRITERIA=BCON(1E-10).",
    ),
    (
        "interaction",
        "LOGI REG y WITH x1 g x1 BY g /CATE=g /CRIT=BCON(1E-10).",
    ),
    (
        "origin",
        "LOGISTIC REGRESSION y WITH x1 x2 /ORIGIN /CRITERIA=BCON(1E-10).",
    ),
    (
        "fstep",
        "LOGISTIC REGRESSION y WITH x1 x2 g /CATEGORICAL=g /METHOD=FSTEP(LR) \
         /CRITERIA=PIN(.2) POUT(.25) BCON(1E-10).",
    ),
    (
        "bstep",
        "LOGISTIC REGRESSION y WITH x1 x2 g /CATEGORICAL=g /METHOD=BSTEP /CRITERIA=BCON(1E-10).",
    ),
];

#[test]
fn every_statistic_matches_the_independent_fit() {
    let mut failures = Vec::new();
    for &(case, syntax) in CASES {
        let mut engine = Engine::default();
        let tables = run(&mut engine, &format!("{DATA}\n{syntax}"));
        for &(c, title, row, column, expected) in EXPECTED.iter().filter(|e| e.0 == case) {
            let actual = number(cell(&tables, title, row, column));
            if !close(actual, expected) {
                failures.push(format!(
                    "{c}: {title} / {row} / {column}: {actual} vs {expected}"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn saved_case_values_match_the_independent_diagnostics() {
    let mut engine = Engine::default();
    run(&mut engine, &format!("{DATA}\n{}", CASES[0].1));
    let names = [
        "PRE_1", "RES_1", "DEV_1", "LRE_1", "SRE_1", "ZRE_1", "LEV_1", "COO_1", "DFB0_1", "DFB1_1",
        "DFB2_1", "DFB3_1", "DFB4_1",
    ];
    for &(case, expected) in SAVED {
        for (name, want) in names.iter().zip(expected) {
            let got = values(&engine, name)[case].unwrap();
            assert!(close(got, want), "{name}[{case}]: {got} vs {want}");
        }
    }
    // the predicted group is the original value, 1 at a probability of at least the cut value
    let pred = values(&engine, "PRE_1");
    let group = values(&engine, "PGR_1");
    for (p, g) in pred.iter().zip(&group) {
        assert_eq!(g.unwrap(), f64::from(u8::from(p.unwrap() >= 0.5)));
    }
}

#[test]
fn stepwise_paths_follow_the_entry_and_removal_tests() {
    // forward with LR: x1, then x2 (score p .079), then g (p .15); nothing leaves at POUT .25
    let mut engine = Engine::default();
    let tables = run(&mut engine, &format!("{DATA}\n{}", CASES[5].1));
    let caption = |title: &str| {
        tables
            .iter()
            .rfind(|t| t.title == title)
            .and_then(|t| t.caption.clone())
            .unwrap()
    };
    let entered = caption("Variables in the Equation");
    assert!(entered.contains("entered on step 1: x1."), "{entered}");
    assert!(entered.contains("entered on step 2: x2."), "{entered}");
    assert!(entered.contains("entered on step 3: g."), "{entered}");
    // backward with the conditional statistic: g (p .141) leaves at POUT .10, x2 (p .071) stays
    let mut engine = Engine::default();
    let tables = run(&mut engine, &format!("{DATA}\n{}", CASES[6].1));
    let note = tables
        .iter()
        .rfind(|t| t.title == "Variables in the Equation")
        .and_then(|t| t.caption.clone())
        .unwrap();
    assert!(note.contains("removed on step 2: g."), "{note}");
    assert!(!note.contains("step 3"), "{note}");
    // SUMMARY keeps the last step only
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{DATA}\nLOGISTIC REGRESSION y WITH x1 x2 g /CATEGORICAL=g /METHOD=BSTEP /PRINT=SUMMARY."
        ),
    );
    let summary = tables.iter().rfind(|t| t.title == "Model Summary").unwrap();
    assert_eq!(summary.rows.len(), 1);
    assert_eq!(row_keys(summary), ["Step 2"]);
}

#[test]
fn blocks_build_on_each_other() {
    // ENTER x1, then ENTER x2 g: the block chi-square of block 2 is against block 1
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        &format!(
            "{DATA}\nLOGISTIC REGRESSION y WITH x1 x2 g /CATEGORICAL=g /METHOD=ENTER x1 /METHOD=ENTER x2 g \
             /CRITERIA=BCON(1E-10)."
        ),
    );
    let omnibus: Vec<&Table> = tables
        .iter()
        .filter(|t| t.title == "Omnibus Tests of Model Coefficients")
        .collect();
    assert_eq!(omnibus.len(), 2);
    let get = |t: &Table, row: &str| {
        let r = row_keys(t).iter().position(|k| k == row).unwrap();
        number(&t.rows[r][2])
    };
    let block = get(omnibus[1], "Step 1 / Block");
    let model = get(omnibus[1], "Step 1 / Model");
    let first = get(omnibus[0], "Step 1 / Model");
    assert!(close(block + first, model), "{block} + {first} vs {model}");
    // the final model is the full one
    let full = EXPECTED
        .iter()
        .find(|e| e.0 == "enter" && e.3 == "-2 Log likelihood")
        .unwrap()
        .4;
    assert!(close(
        number(cell(
            &tables,
            "Model Summary",
            "Step 1",
            "-2 Log likelihood"
        )),
        full
    ));
}

#[test]
fn string_dependent_missing_values_and_casewise_list() {
    let source = "DATA LIST FREE /r (A8) x.
BEGIN DATA.
yes 1 no 2 yes 3 no 4 yes 5 no 6 yes 7 yes 8 no 9 yes 10 no . maybe 12 no 13 yes 14 no 15
END DATA.
MISSING VALUES r ('maybe').
LOGISTIC REGRESSION r WITH x /CASEWISE /SAVE=PRED PGROUP.";
    let mut engine = Engine::default();
    let tables = run(&mut engine, source);
    // the values in alphabetical order: no is 0, yes is 1
    let encoding = tables
        .iter()
        .find(|t| t.title == "Dependent Variable Encoding")
        .unwrap();
    assert_eq!(row_keys(encoding), ["no", "yes"]);
    let included = "Selected Cases / Included in Analysis";
    assert_eq!(
        number(cell(&tables, "Case Processing Summary", included, "N")),
        13.0
    );
    let missing = "Selected Cases / Missing Cases";
    assert_eq!(
        number(cell(&tables, "Case Processing Summary", missing, "N")),
        2.0
    );
    let casewise = tables.iter().find(|t| t.title == "Casewise List").unwrap();
    assert_eq!(casewise.rows.len(), 13);
    // the case missing on the dependent still gets a prediction; the one missing x does not
    let pred = values(&engine, "PRE_1");
    assert!(pred[11].is_some() && pred[10].is_none());
    let data = engine.dataset().unwrap();
    let Values::String(groups) = data.variable("PGR_1").unwrap().values() else {
        panic!("PGR_1 is numeric");
    };
    for (k, p) in pred.iter().enumerate() {
        let group = groups.get(k).trim_end().to_string();
        match p {
            Some(p) if *p >= 0.5 => assert_eq!(group, "yes"),
            Some(_) => assert_eq!(group, "no"),
            None => assert_eq!(group, ""),
        }
    }
}

#[test]
fn errors_name_the_problem() {
    let three = format!("{DATA}\nCOMPUTE t = g.\nLOGISTIC REGRESSION t WITH x1.");
    assert_eq!(
        run_error(&three),
        "LOGISTIC REGRESSION needs a dependent variable with exactly two values among the cases used; t has 3"
    );
    let contrast = format!("{DATA}\nLOGISTIC REGRESSION y WITH x1 /CONTRAST(x1)=HELMERT.");
    assert_eq!(
        run_error(&contrast),
        "CONTRAST names x1, which is not a categorical variable of the model"
    );
    let reference =
        format!("{DATA}\nLOGISTIC REGRESSION y WITH g /CATEGORICAL=g /CONTRAST(g)=SIMPLE(4).");
    assert_eq!(
        run_error(&reference),
        "CONTRAST(g): the reference category 4 is beyond the 3 categories"
    );
}
