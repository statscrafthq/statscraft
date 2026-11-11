//! TREE (CHAID and exhaustive CHAID) on synthetic frequency tables. The expected splits and
//! statistics come from an independent implementation of the manual's merging, splitting and
//! Bonferroni rules on the same data, with chi-square significances from a general statistics
//! library and, for an ordinal dependent, the row effects model fitted by maximum likelihood with
//! a general optimiser (the manual's iteration stops at CONVERGE=.001, so those numbers agree to
//! about 1e-3).

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// y (1 to 3) by a (nominal 1 to 4), b (ordinal 1 to 5, 9 user-missing) and c (1, 2); w counts.
const DATA1: &str = "DATA LIST FREE / a b c y w.
BEGIN DATA
1 1 1 1 14 1 1 1 2 9 1 1 1 3 5 1 1 2 1 14 1 1 2 2 6 1 1 2 3 1 1 2 1 1 13 1 2 1 3 1 1 2 2 1
14 1 2 2 2 10 1 2 2 3 3 1 3 1 1 16 1 3 1 2 8 1 3 1 3 1 1 3 2 1 9 1 3 2 2 4 1 3 2 3 1 1 4 1
1 6 1 4 1 2 9 1 4 1 3 5 1 4 2 1 11 1 4 2 2 4 1 4 2 3 5 1 5 1 1 13 1 5 1 2 4 1 5 1 3 1 1 5
2 1 10 1 5 2 2 2 1 9 1 1 7 1 9 1 2 2 1 9 1 3 2 1 9 2 1 11 1 9 2 2 6 1 9 2 3 2 2 1 1 1 8 2
1 1 2 5 2 1 2 1 12 2 1 2 2 4 2 1 2 3 2 2 2 1 1 16 2 2 1 2 11 2 2 2 1 5 2 2 2 2 6 2 2 2 3 1
2 3 1 1 8 2 3 1 2 8 2 3 1 3 3 2 3 2 1 5 2 3 2 2 5 2 3 2 3 3 2 4 1 1 6 2 4 1 2 1 2 4 1 3 1
2 4 2 1 13 2 4 2 2 6 2 4 2 3 5 2 5 1 1 12 2 5 1 2 10 2 5 1 3 1 2 5 2 1 9 2 5 2 2 5 2 5 2 3
1 2 9 1 1 5 2 9 1 2 6 2 9 1 3 1 2 9 2 1 3 2 9 2 2 6 2 9 2 3 2 3 1 1 1 4 3 1 1 2 6 3 1 1 3
7 3 1 2 1 2 3 1 2 2 7 3 1 2 3 12 3 2 1 1 3 3 2 1 2 2 3 2 1 3 10 3 2 2 1 5 3 2 2 2 2 3 2 2
3 6 3 3 1 1 2 3 3 1 2 4 3 3 1 3 2 3 3 2 1 9 3 3 2 2 4 3 3 2 3 9 3 4 1 1 17 3 4 1 2 3 3 4 1
3 6 3 4 2 1 13 3 4 2 2 10 3 4 2 3 3 3 5 1 1 13 3 5 1 2 4 3 5 1 3 3 3 5 2 1 4 3 5 2 2 3 3 5
2 3 5 3 9 1 1 12 3 9 1 2 8 3 9 1 3 9 3 9 2 1 8 3 9 2 2 6 3 9 2 3 8 4 1 1 1 8 4 1 1 2 5 4 1
1 3 10 4 1 2 1 2 4 1 2 2 1 4 1 2 3 5 4 2 1 1 1 4 2 1 2 5 4 2 1 3 11 4 2 2 1 2 4 2 2 2 5 4
2 2 3 17 4 3 1 1 8 4 3 1 2 3 4 3 1 3 6 4 3 2 1 12 4 3 2 2 9 4 3 2 3 8 4 4 1 1 12 4 4 1 2 5
4 4 1 3 6 4 4 2 1 9 4 4 2 2 3 4 4 2 3 3 4 5 1 1 8 4 5 1 2 8 4 5 1 3 3 4 5 2 1 10 4 5 2 2
12 4 5 2 3 2 4 9 1 1 11 4 9 1 2 2 4 9 1 3 7 4 9 2 1 10 4 9 2 2 8 4 9 2 3 6
END DATA.
MISSING VALUES b (9).
WEIGHT BY w.
";

/// y (1, 2) by x (scale, some system-missing) and g (1 to 3); w counts.
const DATA2: &str = "DATA LIST FREE / x g y w.
BEGIN DATA
0 1 1 1 1 1 1 2 1 1 2 1 1 2 1 4 1 3 1 6 2 1 1 8 2 1 2 1 2 2 1 15 2 2 2 3 2 3 1 6 3 1 1 13
3 1 2 6 3 2 1 9 3 2 2 3 3 3 1 12 3 3 2 2 4 1 1 8 4 1 2 4 4 2 1 16 4 2 2 1 4 3 1 9 4 3 2 11
5 1 1 5 5 1 2 7 5 2 1 10 5 2 2 2 5 3 1 10 5 3 2 8 6 1 1 10 6 1 2 6 6 2 1 7 6 2 2 7 6 3 1
12 6 3 2 5 7 1 1 6 7 1 2 8 7 2 1 11 7 2 2 6 7 3 1 7 7 3 2 5 8 1 1 2 8 1 2 6 8 2 1 5 8 2 2
7 8 3 1 1 8 3 2 12 9 1 2 5 9 2 1 1 9 2 2 6 9 3 1 3 9 3 2 5 10 1 2 7 10 2 1 1 10 2 2 4 10 3
1 1 10 3 2 8 11 1 2 3 11 2 2 4 11 3 2 5 12 1 2 4 12 2 2 2 12 3 2 3 13 1 2 3 13 2 2 4 13 3
2 5 14 3 2 1 15 1 2 1 15 2 2 1 15 3 2 1 16 1 2 1 16 2 2 1 17 1 2 1 18 2 2 2 . 1 1 2 . 1 2
1 . 2 1 3 . 2 2 2 . 3 1 1 . 3 2 2
END DATA.
WEIGHT BY w.
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

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(t) => t,
        Cell::Empty => "",
        other => panic!("not text: {other:?}"),
    }
}

fn relative(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance * expected.abs(),
        "{actual} is not {expected}"
    );
}

fn table<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn column(table: &Table, name: &str) -> usize {
    table.columns.iter().position(|c| c == name).expect(name)
}

/// Per node row of the Tree Table: (definition, total N, split statistic, adjusted significance).
/// Definition, total N and the split (chi-square, adjusted significance) of a node.
type Row = (String, f64, Option<(f64, f64)>);

fn nodes(tables: &[Table]) -> Vec<Row> {
    let tree = table(tables, "Tree Table");
    let (definition, chi, sig) = (
        column(tree, "Definition"),
        column(tree, "Chi-Square"),
        column(tree, "Adj. Sig."),
    );
    let total = tree.columns.len() - 7;
    tree.rows
        .iter()
        .map(|row| {
            let split = match &row[chi] {
                Cell::Number(c) => Some((*c, number(&row[sig]))),
                _ => None,
            };
            (
                text(&row[definition]).to_string(),
                number(&row[total]),
                split,
            )
        })
        .collect()
}

fn engine_with(data: &str) -> Engine {
    let mut engine = Engine::default();
    run(&mut engine, data);
    engine
}

#[test]
fn chaid_merges_and_splits_a_nominal_dependent() {
    let mut engine = engine_with(DATA1);
    let tables = run(
        &mut engine,
        "TREE y [n] BY a [n] b [o] c [n] /DEPCATEGORIES TARGET=[3]
          /PRINT MODELSUMMARY CLASSIFICATION RISK TREETABLE /TREE DISPLAY=NONE.",
    );
    let found = nodes(&tables);
    assert_eq!(found.len(), 5);
    // a: 1 with 2 and 3 with 4 merge; S(4, 2) = 7 ways
    let (chi, sig) = found[0].2.unwrap();
    relative(chi, 69.9770198351, 1e-10);
    relative(sig, 4.46458661415e-15, 1e-8);
    assert_eq!(found[1].0, "a = 1; 2");
    assert_eq!(found[1].1, 424.0);
    assert_eq!(found[2].0, "a = 3; 4");
    // b in node 2: the missing category floats into 3-5; C(4, 0) + 2 C(4, 1) = 9 ways
    let (chi, sig) = found[2].2.unwrap();
    relative(chi, 46.3853909645, 1e-10);
    relative(sig, 7.61698045198e-10, 1e-8);
    assert_eq!(found[3].0, "b = 1; 2");
    assert_eq!(found[4].0, "b = 3; 4; 5; <missing>");
    assert_eq!((found[3].1, found[4].1), (138.0, 336.0));
    assert!(found[1].2.is_none() && found[3].2.is_none());

    let summary = table(&tables, "Model Summary");
    let value = |name: &str| {
        let row = summary.rows.iter().find(|r| text(&r[1]) == name).unwrap();
        row[2].clone()
    };
    assert_eq!(text(&value("Independent Variables Included")), "a, b");
    assert_eq!(number(&value("Number of Nodes")), 5.0);
    assert_eq!(number(&value("Number of Terminal Nodes")), 3.0);
    assert_eq!(number(&value("Depth")), 2.0);

    // nodes 1 and 4 predict 1, node 3 predicts 3
    let classification = table(&tables, "Classification");
    let counts: Vec<Vec<f64>> = classification.rows[..3]
        .iter()
        .map(|r| r[1..4].iter().map(number).collect())
        .collect();
    assert_eq!(
        counts,
        [[398.0, 0.0, 27.0], [229.0, 0.0, 33.0], [133.0, 0.0, 78.0]]
    );
    relative(
        number(&classification.rows[3][4]),
        100.0 * 476.0 / 898.0,
        1e-12,
    );
    let risk = table(&tables, "Risk");
    let r = 422.0 / 898.0;
    relative(number(&risk.rows[0][0]), r, 1e-12);
    relative(
        number(&risk.rows[0][1]),
        (r * (1.0 - r) / 898.0).sqrt(),
        1e-12,
    );

    // gains for category 3: nodes by index, then cumulative
    let gains = table(&tables, "Gain Summary for Nodes");
    let order: Vec<&str> = gains.rows.iter().map(|r| text(&r[0])).collect();
    assert_eq!(order, ["3", "4", "1"]);
    let share = 211.0 / 898.0;
    relative(number(&gains.rows[0][5]), 100.0 * 78.0 / 138.0, 1e-12);
    relative(
        number(&gains.rows[0][6]),
        100.0 * 78.0 / 138.0 / share,
        1e-12,
    );
    relative(number(&gains.rows[1][10]), 100.0 * 164.0 / 211.0, 1e-12);
    relative(
        number(&gains.rows[1][12]),
        100.0 * 164.0 / 474.0 / share,
        1e-12,
    );
    relative(number(&gains.rows[2][12]), 100.0, 1e-12);
}

#[test]
fn exhaustive_chaid_with_likelihood_ratio() {
    let mut engine = engine_with(DATA1);
    let tables = run(
        &mut engine,
        "TREE y [n] BY a [n] b [o] c [n] /METHOD TYPE=EXHAUSTIVECHAID /CHAID CHISQUARE=LR
          /PRINT TREETABLE /TREE DISPLAY=NONE.",
    );
    let found = nodes(&tables);
    assert_eq!(found.len(), 5);
    // nominal a: 4 (16 - 1) / 6 = 10 pairs over the merges; ordinal b with missing: 6 * 5 / 2
    let (chi, sig) = found[0].2.unwrap();
    relative(chi, 73.5960951563, 1e-10);
    relative(sig, 1.04426564855e-15, 1e-8);
    let (chi, sig) = found[2].2.unwrap();
    relative(chi, 46.7656767911, 1e-10);
    relative(sig, 1.0496719035e-09, 1e-8);
    assert_eq!(found[4].0, "b = 3; 4; 5; <missing>");
}

#[test]
fn ordinal_dependent_uses_the_row_effects_model() {
    let mut engine = engine_with(DATA1);
    let tables = run(
        &mut engine,
        "TREE y [o] BY a [n] b [o] c [n] /PRINT TREETABLE /TREE DISPLAY=NONE.",
    );
    let found = nodes(&tables);
    assert_eq!(found.len(), 6);
    let (chi, sig) = found[0].2.unwrap();
    relative(chi, 59.3772882501, 1e-5);
    relative(sig, 9.11138704993e-14, 1e-3);
    // b splits node 2 three ways; the missing category joins 3; C(4, 1) + 3 C(4, 2) = 22 ways
    let (chi, sig) = found[2].2.unwrap();
    relative(chi, 53.3595152762, 1e-4);
    relative(sig, 5.69575316403e-11, 1e-3);
    let definitions: Vec<&str> = found[3..].iter().map(|n| n.0.as_str()).collect();
    assert_eq!(definitions, ["b = 1; 2", "b = 3; <missing>", "b = 4; 5"]);
    assert_eq!(found[4].1, 171.0);
}

#[test]
fn growth_limits_and_alpha_levels() {
    let mut engine = engine_with(DATA1);
    let tables = run(
        &mut engine,
        "TREE y [n] BY a [n] b [o] c [n] /PRINT TREETABLE /TREE DISPLAY=NONE
          /GROWTHLIMIT MINCHILDSIZE=150 MINPARENTSIZE=300 /CHAID ALPHAMERGE=0.5.",
    );
    let found = nodes(&tables);
    // a keeps three groups at ALPHAMERGE .5; S(4, 3) = 6; nodes under 300 cases are not split
    let (chi, sig) = found[0].2.unwrap();
    relative(chi, 74.7402705888, 1e-10);
    relative(sig, 1.35675376238e-14, 1e-8);
    let definitions: Vec<&str> = found[1..4].iter().map(|n| n.0.as_str()).collect();
    assert_eq!(definitions, ["a = 1", "a = 2", "a = 3; 4"]);
    assert!(found[1].2.is_none() && found[2].2.is_none());
    // in node 3 the groups 1-2 and 3+missing are under 150 cases and join
    let (chi, sig) = found[3].2.unwrap();
    relative(chi, 30.1129103367, 1e-10);
    relative(sig, 2.60199892502e-06, 1e-8);
    assert_eq!(found[4].0, "b = 1; 2; 3; <missing>");
    assert_eq!(found[5].0, "b = 4; 5");

    // MAXDEPTH=1 stops below the root; a strict ALPHASPLIT keeps the root whole
    let tables = run(
        &mut engine,
        "TREE y [n] BY a [n] b [o] c [n] /PRINT TREETABLE /TREE DISPLAY=NONE /GROWTHLIMIT MAXDEPTH=1.",
    );
    assert_eq!(nodes(&tables).len(), 3);
    let tables = run(
        &mut engine,
        "TREE y [n] BY a [n] b [o] c [n] /PRINT TREETABLE /TREE DISPLAY=NONE /CHAID ALPHASPLIT=1E-16.",
    );
    assert_eq!(nodes(&tables).len(), 1);
}

#[test]
fn force_splits_the_root_by_its_variable() {
    let mut engine = engine_with(DATA1);
    let tables = run(
        &mut engine,
        "TREE y [n] BY a [n] b [o] FORCE = c [n] /PRINT TREETABLE /TREE DISPLAY=NONE.",
    );
    let found = nodes(&tables);
    assert_eq!(found.len(), 11);
    // c is not significant, yet splits the root
    let (chi, sig) = found[0].2.unwrap();
    relative(chi, 1.51891661648, 1e-10);
    relative(sig, 0.467919826685, 1e-9);
    let (chi, sig) = found[1].2.unwrap();
    relative(chi, 39.2821373171, 1e-10);
    relative(sig, 2.06580965531e-08, 1e-8);
    let (chi, sig) = found[6].2.unwrap();
    relative(chi, 27.6809103754, 1e-10);
    relative(sig, 8.77827525032e-06, 1e-8);
}

#[test]
fn scale_predictors_are_cut_by_rank() {
    let mut engine = engine_with(DATA2);
    // five groups from the weighted average ranks: break points 3, 4, 6 and 8
    let tables = run(
        &mut engine,
        "TREE y [n] BY x [s] g [n] /CHAID INTERVALS=5 /PRINT TREETABLE /TREE DISPLAY=NONE
          /GROWTHLIMIT MINPARENTSIZE=60 MINCHILDSIZE=20.",
    );
    let found = nodes(&tables);
    let (chi, sig) = found[0].2.unwrap();
    relative(chi, 110.593507894, 1e-10);
    relative(sig, 1.79898470798e-22, 1e-8);
    let definitions: Vec<&str> = found[1..].iter().map(|n| n.0.as_str()).collect();
    assert_eq!(
        definitions,
        ["x = <= 3", "x = (3, 6]; <missing>", "x = (6, 8]", "x = > 8"]
    );
    let tables = run(
        &mut engine,
        "TREE y [n] BY x [s] g [n] /CHAID INTERVALS=x (5) /METHOD TYPE=EXHAUSTIVECHAID
          /PRINT TREETABLE /TREE DISPLAY=NONE /GROWTHLIMIT MINPARENTSIZE=60 MINCHILDSIZE=20.",
    );
    relative(nodes(&tables)[0].2.unwrap().1, 1.22658048271e-22, 1e-8);
}

#[test]
fn save_adds_node_prediction_and_probabilities() {
    let mut engine = engine_with(DATA1);
    run(
        &mut engine,
        "COMPUTE NodeID = 0.
         TREE y [n] BY a [n] b [o] c [n] /PRINT NONE /TREE DISPLAY=NONE
          /SAVE NODEID PREDVAL(guess) PREDPROB(p).",
    );
    let data = engine.dataset().unwrap();
    let values = |name: &str| -> Vec<Option<f64>> {
        data.variable(name)
            .unwrap()
            .numeric()
            .unwrap()
            .iter()
            .collect()
    };
    // NodeID exists, so the node goes to NodeID_1
    let node = values("NodeID_1");
    let guess = values("guess");
    let (p1, p3) = (values("p_1"), values("p_3"));
    // the first row: a = 1, in node 1; a row with a = 3 and b = 1 is in node 3; b = 9 in node 4
    assert_eq!((node[0], guess[0]), (Some(1.0), Some(1.0)));
    relative(p1[0].unwrap(), 240.0 / 424.0, 1e-12);
    let a = values("a");
    let b = values("b");
    let row = (0..a.len())
        .find(|&i| a[i] == Some(3.0) && b[i] == Some(1.0))
        .unwrap();
    assert_eq!((node[row], guess[row]), (Some(3.0), Some(3.0)));
    relative(p3[row].unwrap(), 78.0 / 138.0, 1e-12);
    let row = (0..a.len())
        .find(|&i| a[i] == Some(3.0) && b[i] == Some(9.0))
        .unwrap();
    assert_eq!(node[row], Some(4.0));
}

#[test]
fn what_is_not_supported_says_so() {
    let mut engine = engine_with(DATA1);
    let message = error(&mut engine, "TREE y [s] BY a [n] /PRINT NONE.");
    assert!(message.contains("scale dependent variable"), "{message}");
    let (_, errors) = parse("TREE y BY a /METHOD TYPE=CRT.");
    assert_eq!(errors[0].message, "METHOD TYPE=CRT is not supported yet");
    // user-missing values of a nominal predictor are categories of their own with VALID
    let tables = run(
        &mut engine,
        "TREE y [n] BY a [n] b [n] /MISSING NOMINALMISSING=VALID /PRINT TREETABLE /TREE DISPLAY=NONE.",
    );
    let found = nodes(&tables);
    let (chi, sig) = found[2].2.unwrap();
    relative(chi, 53.245553082, 1e-10);
    relative(sig, 6.81372960151e-09, 1e-8);
    assert_eq!(found[4].0, "b = 3; 9");
    assert!(engine.warnings().is_empty(), "{:?}", engine.warnings());
}
