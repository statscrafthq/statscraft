//! DISCRIMINANT on synthetic data. The expected numbers come from an independent computation
//! from the raw values: group and pooled statistics by their definitions, Box's M by Box's
//! formulas, the canonical functions from the generalized symmetric eigenproblem of a general
//! numerical library, Fisher's functions and posteriors by the linear discriminant rule,
//! cross-validation by refitting without each case, the stepwise selection by trying every
//! variable at every step; weighted results by repeating each case as often as its weight. A
//! second statistics system's linear discriminant analysis agrees on the coefficients (up to
//! sign), the posteriors and the cross-validated table.

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Three groups of twelve; case 37 has group 4 (out of range), 38 no group, 39 misses x2.
const DATA: &str = "DATA LIST FREE /id g s w x1 x2 x3 x4.
BEGIN DATA.
1 1 1 1 6.6 1.4 3.8 5.2
2 2 1 2 4.9 5.6 3.6 3.9
3 3 1 1 7.2 7.8 5.4 6.5
4 1 1 2 4.6 3.2 4.5 6.6
5 2 1 1 7.3 3.9 2.4 4.9
6 3 1 2 5.7 5.8 6.9 4.6
7 1 1 1 7.0 1.8 5.8 6.0
8 2 1 2 6.9 4.4 5.1 5.2
9 3 1 1 6.0 6.0 5.0 5.6
10 1 1 2 5.1 3.2 4.1 5.3
11 2 1 1 8.1 4.7 4.8 5.8
12 3 1 2 6.4 7.8 4.1 5.3
13 1 1 1 3.9 0.5 2.7 5.2
14 2 1 2 8.6 4.3 6.0 5.0
15 3 1 1 5.2 5.7 3.6 4.5
16 1 1 2 4.9 3.0 3.7 8.1
17 2 1 1 7.0 4.9 7.3 6.6
18 3 1 2 5.7 5.6 6.4 5.7
19 1 1 1 4.4 3.2 3.9 5.3
20 2 1 2 9.9 4.2 3.6 5.9
21 3 1 1 5.6 6.7 5.3 5.9
22 1 1 2 4.8 1.7 3.7 5.5
23 2 1 1 7.0 3.8 1.9 5.6
24 3 1 2 7.9 5.4 5.8 6.5
25 1 1 1 5.9 4.9 5.0 5.7
26 2 1 2 8.5 3.8 3.3 4.8
27 3 1 1 7.0 6.5 5.4 6.0
28 1 2 2 3.6 2.2 3.4 5.0
29 2 2 1 7.0 2.0 2.9 6.4
30 3 2 2 3.6 7.7 3.6 5.5
31 1 2 1 4.8 2.9 4.6 7.4
32 2 2 2 6.8 3.9 3.6 4.9
33 3 2 1 6.6 4.3 4.4 5.1
34 1 2 2 6.7 3.6 5.4 8.2
35 2 2 1 6.6 4.6 3.0 1.7
36 3 2 2 8.1 4.8 2.3 7.2
37 4 1 1 6.1 4.2 4.4 5.5
38 . 1 1 5.2 3.3 4.1 6.2
39 2 1 1 6.6 . 4.0 5.0
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

fn run(syntax: &str) -> Vec<Table> {
    run_on(&mut Engine::default(), &format!("{DATA}{syntax}"))
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

/// The numbers after the row headers of the first row whose headers include `name`, from the
/// `nth` match on.
fn row_at(table: &Table, name: &str, nth: usize) -> Vec<f64> {
    let h = table.row_headers;
    let r = table
        .rows
        .iter()
        .filter(|r| r[..h].iter().any(|c| label(c) == name))
        .nth(nth)
        .unwrap_or_else(|| panic!("{}: no row {name}", table.title));
    r[h..]
        .iter()
        .map(|c| match c {
            Cell::Number(n) => *n,
            _ => f64::NAN,
        })
        .collect()
}

fn row(table: &Table, name: &str) -> Vec<f64> {
    row_at(table, name, 0)
}

fn assert_near(actual: &[f64], expected: &[f64], tolerance: f64, what: &str) {
    assert_eq!(actual.len(), expected.len(), "{what}: {actual:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert!(
            (a - e).abs() <= tolerance * e.abs().max(1.0),
            "{what}: ours {actual:?}, expected {expected:?}"
        );
    }
}

const ALL: &str = "DISCRIMINANT GROUPS=g(1,3) /VARIABLES=x1 TO x4 \
                   /STATISTICS=MEAN STDDEV UNIVF BOXM COEFF RAW TABLE CROSSVALID CORR COV.";

#[test]
fn direct_entry_functions_and_classification() {
    let tables = run(ALL);
    let summary = find(&tables, "Analysis Case Processing Summary");
    assert_eq!(row(summary, "Valid"), [36.0, 100.0 * 36.0 / 39.0]);
    assert_eq!(row(summary, "Missing or out-of-range group codes")[0], 2.0);
    assert_eq!(
        row(summary, "At least one missing discriminating variable")[0],
        1.0
    );
    let stats = find(&tables, "Group Statistics");
    // group 2, x1: mean, deviation, valid N
    assert_near(
        &row_at(stats, "x1", 1),
        &[7.383333333333333, 1.2568599642350653, 12.0, 12.0],
        1e-12,
        "group 2 x1",
    );
    assert_near(
        &row_at(stats, "x4", 3),
        &[5.627777777777777, 1.1621681980827563, 36.0, 36.0],
        1e-12,
        "total x4",
    );
    let univ = find(&tables, "Tests of Equality of Group Means");
    assert_near(
        &row(univ, "x2"),
        &[
            0.33445539910690736,
            32.83393224944125,
            2.0,
            33.0,
            1.4176874147684e-08,
        ],
        1e-9,
        "univariate x2",
    );
    let pooled = find(&tables, "Pooled Within-Groups Matrices");
    assert_near(
        &row_at(pooled, "x1", 0),
        &[
            1.4532070707070706,
            -0.18505050505050494,
            0.32707070707070707,
            0.5310353535353535,
        ],
        1e-12,
        "pooled covariance",
    );
    assert_near(
        &row_at(pooled, "x4", 1),
        &[
            0.3983693483526168,
            0.032099966271534726,
            0.20719969093235888,
            1.0,
        ],
        1e-12,
        "pooled correlation",
    );
    let logdet = find(&tables, "Log Determinants");
    assert_near(
        &row(logdet, "1"),
        &[4.0, -1.026957757452688],
        1e-10,
        "log det 1",
    );
    assert_near(
        &row(logdet, "Pooled within-groups"),
        &[4.0, 0.8365086193004155],
        1e-10,
        "pooled log det",
    );
    let boxm = find(&tables, "Test Results");
    let values: Vec<f64> = boxm.rows.iter().map(|r| number(&r[2])).collect();
    assert_near(
        &values,
        &[
            32.71807083096961,
            1.3433160834082682,
            20.0,
            3909.0282813633016,
            0.13994143765058062,
        ],
        1e-8,
        "Box's M",
    );
    let eigen = find(&tables, "Eigenvalues");
    assert_near(
        &row(eigen, "1"),
        &[
            2.9138780839083385,
            78.25386063271817,
            78.25386063271817,
            0.8628435198705834,
        ],
        1e-10,
        "eigenvalue 1",
    );
    assert_near(
        &row(eigen, "2"),
        &[
            0.8097440611824016,
            21.746139367281828,
            100.0,
            0.668906329831645,
        ],
        1e-10,
        "eigenvalue 2",
    );
    let wilks = find(&tables, "Wilks' Lambda");
    assert_near(
        &row(wilks, "1 through 2"),
        &[
            0.14118077008657776,
            61.66799580016302,
            8.0,
            2.1917840004104653e-10,
        ],
        1e-9,
        "Wilks 1",
    );
    assert_near(
        &row(wilks, "2"),
        &[
            0.5525643219111586,
            18.685341127460035,
            3.0,
            0.0003175647160399631,
        ],
        1e-9,
        "Wilks 2",
    );
    let std = find(
        &tables,
        "Standardized Canonical Discriminant Function Coefficients",
    );
    let expected = [
        [0.7030585942833292, -0.7727891462392267],
        [0.9391442180504332, 0.4636078335177056],
        [-0.2600410771071125, 0.22287199608822303],
        [-0.4242655267516863, 0.5789572801173724],
    ];
    for (k, name) in ["x1", "x2", "x3", "x4"].iter().enumerate() {
        assert_near(&row(std, name), &expected[k], 1e-9, name);
    }
    let structure = find(&tables, "Structure Matrix");
    // ordered by the function of the largest correlation: x2 for 1, then x1, x4, x3 for 2
    let order: Vec<String> = structure.rows.iter().map(|r| label(&r[0])).collect();
    assert_eq!(order, ["x2", "x1", "x4", "x3"]);
    assert_near(
        &row(structure, "x2"),
        &[0.7504006367487801, 0.6566259039221685],
        1e-9,
        "structure x2",
    );
    assert_near(
        &row(structure, "x3"),
        &[0.07037662948972559, 0.31211648954752513],
        1e-9,
        "structure x3",
    );
    let raw = find(&tables, "Canonical Discriminant Function Coefficients");
    assert_near(
        &row(raw, "x1"),
        &[0.5832137713049457, -0.641057909065369],
        1e-9,
        "raw x1",
    );
    assert_near(
        &row(raw, "(Constant)"),
        &[-4.40495339255817, -1.548329852084209],
        1e-9,
        "constants",
    );
    let centroids = find(&tables, "Functions at Group Centroids");
    assert_near(
        &row(centroids, "1"),
        &[-2.279534971670981, 0.20130837067402108],
        1e-9,
        "centroid 1",
    );
    assert_near(
        &row(centroids, "3"),
        &[1.4704827713466209, 0.9400207346299474],
        1e-9,
        "centroid 3",
    );
    let fisher = find(&tables, "Classification Function Coefficients");
    assert_near(
        &row(fisher, "x4"),
        &[3.727361195397932, 1.8393847567406958, 2.675337413989151],
        1e-9,
        "Fisher x4",
    );
    assert_near(
        &row(fisher, "(Constant)"),
        &[-23.296026205953538, -33.18245948400689, -39.86302689802242],
        1e-9,
        "Fisher constants",
    );
    let priors = find(&tables, "Prior Probabilities for Groups");
    assert_near(&row(priors, "2"), &[1.0 / 3.0, 12.0, 12.0], 1e-12, "priors");
    let results = find(&tables, "Classification Results");
    // original counts, with the two ungrouped cases (37 and 38) classified as 2 and 1
    assert_eq!(row_at(results, "1", 0), [11.0, 0.0, 1.0, 12.0]);
    assert_eq!(row_at(results, "2", 0), [1.0, 9.0, 2.0, 12.0]);
    assert_eq!(row_at(results, "3", 0), [0.0, 2.0, 10.0, 12.0]);
    assert_eq!(row(results, "Ungrouped cases"), [1.0, 1.0, 0.0, 2.0]);
    assert_near(
        &row_at(results, "1", 1),
        &[100.0 * 11.0 / 12.0, 0.0, 100.0 / 12.0, 100.0],
        1e-12,
        "percent",
    );
    // cross-validated
    assert_eq!(row_at(results, "2", 2), [1.0, 9.0, 2.0, 12.0]);
    let caption = results.caption.clone().unwrap();
    assert!(
        caption.contains("83.3% of original grouped cases correctly classified")
            && caption.contains("83.3% of cross-validated"),
        "{caption}"
    );
}

#[test]
fn saved_scores_probabilities_and_classes() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{DATA}VALUE LABELS g 1 'low' 2 'mid' 3 'high'.\n\
             DISCRIMINANT GROUPS=g(1,3) /VARIABLES=x1 TO x4 /SAVE=CLASS SCORES PROBS \
             /ANALYSIS=x1 x2 /SAVE CLASS=cls SCORES=sc PROBS=pr /PRIORS=.5 .25 .25."
        ),
    );
    let data = engine.dataset().unwrap();
    let values = |name: &str| match data.variable(name).unwrap().values() {
        Values::Numeric(v) => (0..39).map(|i| v.get(i)).collect::<Vec<_>>(),
        Values::String(_) => panic!(),
    };
    let names: Vec<&str> = data.variables().map(|v| v.name()).collect();
    assert_eq!(
        names[8..16],
        [
            "DSC_1", "DSC1_1", "DSC2_1", "DSC1_2", "DSC2_2", "DSC3_2", "cls", "sc1"
        ]
    );
    let near = |a: Option<f64>, e: f64| assert!((a.unwrap() - e).abs() < 1e-9, "{a:?} {e}");
    near(values("DSC1_1")[0], -2.0976200937918463);
    near(values("DSC2_1")[0], -1.790935956864401);
    near(values("DSC1_2")[0], 0.9191476829224842);
    near(values("DSC3_2")[0], 0.0002807672617665338);
    assert_eq!(values("DSC_1")[0], Some(1.0));
    // the ungrouped cases are classified; case 39 misses x2 and gets nothing
    assert_eq!(values("DSC_1")[36], Some(2.0));
    assert_eq!(values("DSC_1")[37], Some(1.0));
    near(values("DSC2_2")[36], 0.5401253722324487);
    assert_eq!(values("DSC_1")[38], None);
    let class = data.variable("DSC_1").unwrap();
    assert_eq!(class.value_labels().len(), 3);
    assert_eq!(class.label(), Some("Predicted group for analysis 1"));
    // the second analysis has its own names; case 39 misses x2, used there too
    assert_eq!(values("cls")[38], None);
    assert!(data.variable("pr3").is_ok() && data.variable("sc2").is_ok());
}

#[test]
fn priors_and_separate_covariances_change_the_posteriors() {
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!(
            "{DATA}DISCRIMINANT GROUPS=g(1,3) /VARIABLES=x1 TO x4 /PRIORS=.5 2*.25 \
             /SAVE=PROBS /STATISTICS=COEFF TABLE."
        ),
    );
    let fisher = find(&tables, "Classification Function Coefficients");
    assert_near(
        &row(fisher, "(Constant)"),
        &[-22.89056109784537, -33.47014155645867, -40.1507089704742],
        1e-9,
        "Fisher constants with priors",
    );
    let data = engine.dataset().unwrap();
    let p = |name: &str| match data.variable(name).unwrap().values() {
        Values::Numeric(v) => v.get(1).unwrap(),
        Values::String(_) => panic!(),
    };
    assert_near(
        &[p("DSC1_1"), p("DSC2_1"), p("DSC3_1")],
        &[0.005383664250156457, 0.2624143667277548, 0.7322019690220888],
        1e-9,
        "posterior of case 2",
    );
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{DATA}DISCRIMINANT GROUPS=g(1,3) /VARIABLES=x1 TO x4 /SAVE=PROBS /CLASSIFY=SEPARATE."
        ),
    );
    let data = engine.dataset().unwrap();
    let p = |name: &str| match data.variable(name).unwrap().values() {
        Values::Numeric(v) => v.get(1).unwrap(),
        Values::String(_) => panic!(),
    };
    assert_near(
        &[p("DSC1_1"), p("DSC2_1"), p("DSC3_1")],
        &[
            0.004504779673455485,
            0.15829248841298876,
            0.8372027319135558,
        ],
        1e-9,
        "separate posterior of case 2",
    );
    // priors that do not sum to 1 are rescaled with a warning
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{DATA}DISCRIMINANT GROUPS=g(1,3) /VARIABLES=x1 TO x4 /PRIORS=2 1 1 /STATISTICS=TABLE."
        ),
    );
    assert!(engine.warnings().iter().any(|w| w.contains("rescaled")));
}

#[test]
fn weights_count_as_repeated_cases() {
    let tables = run(&format!("WEIGHT BY w.\n{ALL}"));
    let eigen = find(&tables, "Eigenvalues");
    assert_near(
        &row(eigen, "1"),
        &[
            3.538138327662894,
            79.8851516106771,
            79.8851516106771,
            0.8829752681610459,
        ],
        1e-10,
        "weighted eigenvalue",
    );
    let univ = find(&tables, "Tests of Equality of Group Means");
    assert_near(
        &row(univ, "x4"),
        &[
            0.8112978123460414,
            5.931121361293073,
            2.0,
            51.0,
            0.004831675680795532,
        ],
        1e-9,
        "weighted univariate",
    );
    let boxm = find(&tables, "Test Results");
    let values: Vec<f64> = boxm.rows.iter().map(|r| number(&r[2])).collect();
    assert_near(
        &values,
        &[
            55.69945230629214,
            2.465924913483964,
            20.0,
            9336.439448876008,
            0.0002846235815819628,
        ],
        1e-8,
        "weighted Box's M",
    );
    let raw = find(&tables, "Canonical Discriminant Function Coefficients");
    assert_near(
        &row(raw, "(Constant)"),
        &[-4.4919325106197805, -2.8362543224719445],
        1e-9,
        "weighted constants",
    );
    let results = find(&tables, "Classification Results");
    assert_eq!(row_at(results, "2", 0), [1.0, 14.0, 3.0, 18.0]);
    let stats = find(&tables, "Group Statistics");
    assert_eq!(row_at(stats, "x1", 0)[2..], [12.0, 18.0]);
}

#[test]
fn stepwise_wilks_enters_x2_x1_x4() {
    let tables =
        run("DISCRIMINANT GROUPS=g(1,3) /VARIABLES=x1 TO x4 /METHOD=WILKS /STATISTICS=FPAIR.");
    let steps = find(&tables, "Variables Entered/Removed");
    let entered: Vec<String> = steps.rows.iter().map(|r| label(&r[1])).collect();
    assert_eq!(entered, ["x2", "x1", "x4"]);
    let lambdas: Vec<f64> = steps.rows.iter().map(|r| number(&r[3])).collect();
    assert_near(
        &lambdas,
        &[0.3344553991069075, 0.20885891591048542, 0.15036701796728522],
        1e-10,
        "lambda by step",
    );
    let wilks = find(&tables, "Wilks' Lambda");
    // the last step: Rao's F with 6 and 62 degrees of freedom
    let last = &wilks.rows[2];
    assert_near(
        &last[6..9].iter().map(number).collect::<Vec<_>>(),
        &[16.314637577682536, 6.0, 62.0],
        1e-9,
        "Rao's F",
    );
    let not_in = find(&tables, "Variables Not in the Analysis");
    let last = not_in.rows.last().unwrap();
    assert_eq!(label(&last[1]), "x3");
    assert_near(
        &[number(&last[4]), number(&last[5])],
        &[0.9760091131824281, 0.14118077008657776],
        1e-9,
        "x3 at the end",
    );
    let inside = find(&tables, "Variables in the Analysis");
    let x1 = inside
        .rows
        .iter()
        .rev()
        .find(|r| label(&r[1]) == "x1")
        .unwrap();
    assert!((number(&x1[3]) - 14.110771643739783).abs() < 1e-8);
    // the canonical functions use the three variables
    let std = find(
        &tables,
        "Standardized Canonical Discriminant Function Coefficients",
    );
    assert_eq!(std.rows.len(), 3);
    let structure = find(&tables, "Structure Matrix");
    assert!(
        structure
            .caption
            .as_ref()
            .unwrap()
            .contains("Not used in the analysis: x3")
    );
    assert!(
        tables
            .iter()
            .any(|t| t.title == "Pairwise Group Comparisons")
    );
}

#[test]
fn the_other_criteria_choose_the_first_variable() {
    // with one variable each criterion is a function of its group separation: all pick x2
    for (method, value) in [
        ("MAHAL", 2.0622891011876057),
        ("MAXMINF", 12.373734607125634),
        ("MINRESID", 1.4639822780560767),
        ("RAO", 65.66786449888248),
    ] {
        let tables = run(&format!(
            "DISCRIMINANT GROUPS=g(1,3) /VARIABLES=x1 TO x4 /METHOD={method} /MAXSTEPS=1."
        ));
        let steps = find(&tables, "Variables Entered/Removed");
        assert_eq!(steps.rows.len(), 1, "{method}");
        assert_eq!(label(&steps.rows[0][1]), "x2", "{method}");
        assert!((number(&steps.rows[0][3]) - value).abs() < 1e-9, "{method}");
    }
    // inclusion levels: x3 forced in first (even level), x4 never (level 0)
    let tables = run(
        "DISCRIMINANT GROUPS=g(1,3) /VARIABLES=x1 TO x4 /ANALYSIS=x3(2) x1 x2(1) x4(0) /METHOD=WILKS /HISTORY=NONE.",
    );
    assert!(
        !tables
            .iter()
            .any(|t| t.title == "Variables Entered/Removed")
    );
    let std = find(
        &tables,
        "Standardized Canonical Discriminant Function Coefficients",
    );
    let names: Vec<String> = std.rows.iter().map(|r| label(&r[0])).collect();
    assert!(names.contains(&"x3".to_string()) && !names.contains(&"x4".to_string()));
    // a high FIN enters nothing
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!("{DATA}DISCRIMINANT GROUPS=g(1,3) /VARIABLES=x1 TO x4 /METHOD=WILKS /FIN=100."),
    );
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("entered no variable"))
    );
}

#[test]
fn select_and_classify_options() {
    let tables =
        run("DISCRIMINANT GROUPS=g(1,3) /VARIABLES=x1 TO x4 /SELECT=s(1) /STATISTICS=TABLE.");
    let summary = find(&tables, "Analysis Case Processing Summary");
    assert_eq!(row(summary, "Valid")[0], 27.0);
    assert_eq!(row(summary, "Unselected")[0], 9.0);
    let results: Vec<&Table> = tables
        .iter()
        .filter(|t| t.title == "Classification Results")
        .collect();
    assert_eq!(results.len(), 2);
    assert!(
        results[1]
            .caption
            .as_ref()
            .unwrap()
            .starts_with("Cases Not Selected")
    );
    // the unselected cases: three per group
    let total: f64 = (1..=3)
        .map(|g| row_at(results[1], &g.to_string(), 0)[3])
        .sum();
    assert_eq!(total, 9.0);
    // UNCLASSIFIED classifies only the two cases without a group in range
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{DATA}DISCRIMINANT GROUPS=g(1,3) /VARIABLES=x1 TO x4 /SAVE=CLASS /CLASSIFY=UNCLASSIFIED MEANSUB."
        ),
    );
    let data = engine.dataset().unwrap();
    let Values::Numeric(v) = data.variable("DSC_1").unwrap().values() else {
        panic!()
    };
    let classified: Vec<usize> = (0..39).filter(|&i| v.get(i).is_some()).collect();
    assert_eq!(classified, [36, 37]);
}

#[test]
fn errors() {
    let mut engine = Engine::default();
    let (commands, _) = parse(&format!(
        "{DATA}DISCRIMINANT GROUPS=g(1,1) /VARIABLES=x1 x2.\n"
    ));
    let last = commands.last().unwrap();
    let mut result = Ok(Vec::new());
    for c in &commands {
        result = engine.execute(&c.command);
    }
    let _ = last;
    assert!(result.unwrap_err().to_string().contains("two groups"));
}
