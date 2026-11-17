//! ORTHOPLAN, PLANCARDS and CONJOINT on invented plans and invented preferences.
//!
//! The expected utilities, standard errors, constants, importances and correlations come
//! from R 4 run as a black box: `lm` on the experimental profiles with sum-to-zero coding
//! (`contr.sum`) for DISCRETE factors, the value for LINEAR and the value and its square for
//! IDEAL, one row per profile and response; `vcov` for the standard errors; `cor` (Pearson and
//! Kendall) between the predictions and the observed preferences; ranks turned into 13 - rank.

use statscraft_data::Dataset;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Nine experimental profiles (an orthogonal plan of three 3-level factors and one 2-level
/// factor), three holdouts and three simulations. Price is coded 1 to 3 and recoded on FACTORS.
const PLAN: &str = "DATA LIST FREE /brand price sweet organic STATUS_.
BEGIN DATA.
1 1 1 0 0
1 2 2 1 0
1 3 3 0 0
2 1 2 0 0
2 2 3 0 0
2 3 1 1 0
3 1 3 1 0
3 2 1 0 0
3 3 2 0 0
1 3 2 1 1
3 1 3 0 1
2 2 1 1 1
1 1 2 1 2
2 3 3 0 2
3 2 2 1 2
END DATA.
VALUE LABELS brand 1 'Acme' 2 'Brio' 3 'Cora'.
DATASET NAME plan.
";

/// Four subjects' ranks of the twelve rated profiles, and a fifth with a missing rank.
const RANKS: [[u32; 12]; 4] = [
    [3, 7, 1, 9, 5, 11, 2, 8, 4, 10, 6, 12],
    [10, 2, 6, 1, 8, 3, 12, 5, 9, 4, 11, 7],
    [5, 1, 9, 4, 2, 8, 3, 7, 11, 6, 12, 10],
    [4, 6, 2, 12, 9, 7, 1, 3, 10, 8, 5, 11],
];

fn rank_data() -> String {
    let mut s = String::from("DATA LIST FREE /id r1 TO r12.\nBEGIN DATA.\n");
    for (i, r) in RANKS.iter().enumerate() {
        let r: Vec<String> = r.iter().map(u32::to_string).collect();
        s.push_str(&format!("{} {}\n", 101 + i, r.join(" ")));
    }
    s.push_str("105 1 2 3 4 5 6 7 8 9 10 11 .\nEND DATA.\n");
    s
}

/// The same preferences as profile numbers from the best down.
fn sequence_data() -> String {
    let mut s = String::from("DATA LIST FREE /id s1 TO s12.\nBEGIN DATA.\n");
    for (i, r) in RANKS.iter().enumerate() {
        let mut order: Vec<usize> = (1..=12).collect();
        order.sort_by_key(|&card| r[card - 1]);
        let o: Vec<String> = order.iter().map(usize::to_string).collect();
        s.push_str(&format!("{} {}\n", 101 + i, o.join(" ")));
    }
    s.push_str("END DATA.\n");
    s
}

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

fn close(got: f64, want: f64) {
    assert!((got - want).abs() < 1e-8, "got {got}, want {want}");
}

/// Column `col` of the rows of a table.
fn column(t: &Table, col: usize) -> Vec<f64> {
    t.rows.iter().map(|r| number(&r[col])).collect()
}

const MODEL: &str = "CONJOINT PLAN=plan /FACTORS=price 'Price' (LINEAR LESS 5 7 9) \
     sweet (IDEAL) organic (DISCRETE MORE) /SUBJECT=id";

#[test]
fn rankings_of_several_subjects_match_least_squares() {
    let mut engine = Engine::default();
    let source = format!(
        "{PLAN}{}DATASET DECLARE u.\n{MODEL} /RANK=r1 TO r12 /UTILITY=u.\n",
        rank_data()
    );
    let tables = run_on(&mut engine, &source);
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("subjects 105 skipped"))
    );
    let model = table(&tables, "Model Description");
    let models: Vec<&Cell> = model.rows.iter().map(|r| &r[1]).collect();
    assert_eq!(
        models,
        // DISCRETE factors named on FACTORS come before the others
        ["Discrete (more)", "Discrete", "Linear (less)", "Ideal"]
            .map(|s| Cell::Text(s.into()))
            .iter()
            .collect::<Vec<_>>()
    );
    let recoded = table(&tables, "Recoded Values: Price");
    assert_eq!(column(recoded, 1), [5.0, 7.0, 9.0]);

    // the summary: the fit to all four subjects
    let u = table(&tables, "Utilities");
    assert_eq!(u.rows[2][1], Cell::Text("Acme".into()));
    let want = [
        (-0.4375, 0.627723949138345),
        (0.4375, 0.627723949138345),
        (1.166666666666666, 0.836965265517793),
        (-0.75, 0.836965265517793),
        (-0.416666666666666, 0.836965265517793),
        (-1.5625, 1.81208295505899),
        (-2.1875, 2.53691613708259),
        (-2.8125, 3.26174931910618),
        (-1.66666666666667, 3.83545668328410),
        (-1.83333333333333, 5.22684640788760),
        (-0.499999999999995, 4.348999092141576),
        (10.83333333333332, 5.14240068470392),
    ];
    for (row, (value, se)) in u.rows.iter().zip(want) {
        close(number(&row[2]), value);
        close(number(&row[3]), se);
    }
    let b = column(table(&tables, "Coefficients"), 2);
    for (got, want) in b.iter().zip([-0.3125, -2.41666666666667, 0.75]) {
        close(*got, want);
    }
    // importance is the mean over the subjects
    let importance = column(table(&tables, "Importance Values"), 1);
    let want = [
        13.9431030628214,
        34.7202660406886,
        17.3108093002459,
        34.0258215962441,
    ];
    for (got, want) in importance.iter().zip(want) {
        assert!((got - want).abs() < 1e-8);
    }
    let r = table(&tables, "Correlations");
    close(number(&r.rows[0][1]), 0.895547325679290);
    close(number(&r.rows[0][2]), 0.000546735874434317);
    close(number(&r.rows[1][1]), 0.760638829255665);
    close(number(&r.rows[2][1]), 0.333333333333333);

    // one subject
    let u1 = table(&tables, "Utilities: Subject 101");
    close(number(&u1.rows[2][2]), 1.88888888888889);
    close(number(&u1.rows[4][3]), 1.133115447465063);
    close(number(&u1.rows[11][2]), 9.22222222222222);
    close(number(&u1.rows[11][3]), 6.96197786569801);
    let r2 = table(&tables, "Correlations: Subject 102");
    close(number(&r2.rows[0][1]), 0.785689310292365);
    close(number(&r2.rows[1][1]), 0.444444444444445);
    let i4 = column(table(&tables, "Importance Values: Subject 104"), 1);
    close(i4[1], 40.0);
    close(i4[2], 5.0);

    // reversals: price rises for 102, sweetness has a low point for 101 and 104, organic
    // falls for 101
    let rev = table(&tables, "Number of Reversals");
    assert_eq!(column(rev, 1), [1.0, 1.0, 2.0]);
    let summary = table(&tables, "Reversal Summary");
    assert_eq!(summary.rows[0][1], Cell::Text("Subjects 102, 104".into()));
    assert_eq!(summary.rows[1][1], Cell::Text("Subjects 101".into()));

    // simulations: per subject the best profile, BTL and logit, averaged
    let p = table(&tables, "Preference Probabilities of Simulations");
    let want = [
        [0.75, 0.403420484766033, 0.704730130721765],
        [0.25, 0.287076361637962, 0.173891371775905],
        [0.0, 0.309503153596004, 0.121378497502330],
    ];
    for (row, want) in p.rows.iter().zip(want) {
        for (cell, w) in row[1..].iter().zip(want) {
            close(number(cell), w);
        }
    }

    // the utility file: one case per subject
    run_on(&mut engine, "DATASET ACTIVATE u.\n");
    let data: &Dataset = engine.dataset().unwrap();
    let names: Vec<&str> = data.variable_names().collect();
    assert_eq!(
        &names[..10],
        [
            "id", "CONSTANT", "organic1", "organic2", "brand1", "brand2", "brand3", "price_L",
            "sweet_L", "sweet_Q"
        ]
    );
    assert_eq!(names[10], "SCORE1");
    assert_eq!(names.last(), Some(&"SIMUL15"));
    assert_eq!(data.case_count(), 4);
    let constant: Vec<Option<f64>> = data
        .variable("CONSTANT")
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect();
    close(constant[3].unwrap(), 24.49999999999998);
    let simul: Vec<Option<f64>> = data
        .variable("SIMUL13")
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect();
    close(simul[2].unwrap(), 12.44444444444445);
}

#[test]
fn sequences_give_the_same_utilities_as_ranks() {
    let ranks = run_on(
        &mut Engine::default(),
        &format!(
            "{PLAN}{}{MODEL} /RANK=r1 TO r12 /PRINT=ANALYSIS SUMMARYONLY.\n",
            rank_data()
        ),
    );
    let sequences = run_on(
        &mut Engine::default(),
        &format!(
            "{PLAN}{}{MODEL} /SEQUENCE=s1 TO s12 /PRINT=ANALYSIS SUMMARYONLY.\n",
            sequence_data()
        ),
    );
    let a = column(table(&ranks, "Utilities"), 2);
    let b = column(table(&sequences, "Utilities"), 2);
    a.iter().zip(&b).for_each(|(x, y)| close(*x, *y));
    assert!(sequences.iter().all(|t| !t.title.contains("Subject")));
    assert!(sequences.iter().all(|t| !t.title.contains("Simulation")));
}

#[test]
fn scores_without_subject_are_one_group_of_discrete_factors() {
    let source = format!(
        "{PLAN}DATA LIST FREE /s1 TO s12.\nBEGIN DATA.\n7 3 9 2 5 1 8 4 6 3 7 5\n\
         6 4 8 3 6 2 9 3 5 4 6 6\nEND DATA.\nCONJOINT PLAN=plan /SCORE=s1 TO s12 /PRINT=ANALYSIS.\n"
    );
    let tables = run_on(&mut Engine::default(), &source);
    let u = table(&tables, "Utilities");
    let want = [
        1.11111111111111,
        -1.888888888888888,
        0.777777777777778,
        0.777777777777778,
        -0.888888888888889,
        0.11111111111111,
        -1.222222222222222,
        -1.222222222222222,
        2.44444444444444,
        0.416666666666667,
        -0.416666666666667,
        4.916666666666667,
    ];
    column(u, 2)
        .iter()
        .zip(want)
        .for_each(|(g, w)| close(*g, w));
    close(number(&u.rows[2][3]), 0.288675134594813);
    close(number(&u.rows[9][3]), 0.216506350946110);
    let importance = column(table(&tables, "Importance Values"), 1);
    let want = [
        32.72727272727271,
        18.18181818181819,
        40.00000000000001,
        9.09090909090909,
    ];
    importance.iter().zip(want).for_each(|(g, w)| close(*g, w));
    let r = table(&tables, "Correlations");
    close(number(&r.rows[0][1]), 0.984953168106399);
    // R gives 0.9574 on its unrounded predictions, where float noise splits two tied ones
    close(number(&r.rows[1][1]), 0.971008312455224);
    close(number(&r.rows[2][1]), 0.333333333333333);
}

#[test]
fn a_plan_that_is_not_orthogonal_shows_cramers_v() {
    // the first holdout counted as experimental breaks the balance
    let plan = PLAN.replace("1 3 2 1 1\n", "1 3 2 1 0\n");
    let source = format!(
        "{plan}DATA LIST FREE /s1 TO s12.\nBEGIN DATA.\n7 3 9 2 5 1 8 4 6 3 7 5\nEND DATA.\n\
         CONJOINT PLAN=plan /SCORE=s1 TO s12.\n"
    );
    let mut engine = Engine::default();
    let tables = run_on(&mut engine, &source);
    let v = table(&tables, "Cramer's V");
    assert!(number(&v.rows[1][1]) > 0.0);
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("not orthogonal"))
    );
}

fn values(data: &Dataset, name: &str) -> Vec<f64> {
    let v = data.variable(name).unwrap();
    v.numeric().unwrap().iter().map(|x| x.unwrap()).collect()
}

const FACTORS: &str = "ORTHOPLAN FACTORS=brand 'Brand' ('Acme' 'Brio' 'Cora') \
     price 'Price' (5 '5 euro' 7 '7 euro' 9 '9 euro') size (1 2 3) organic (0 'no' 1 'yes')";

#[test]
fn orthoplan_builds_a_small_orthogonal_plan_with_holdouts() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!("SET SEED=4321.\n{FACTORS} /HOLDOUT=3.\n"),
    );
    let data = engine.dataset().unwrap().clone();
    let status = values(&data, "STATUS_");
    assert_eq!(status.iter().filter(|s| **s == 0.0).count(), 9);
    assert_eq!(&status[9..], [1.0, 1.0, 1.0]);
    assert_eq!(
        values(&data, "CARD_"),
        (1..=12).map(f64::from).collect::<Vec<_>>()
    );
    let names = ["brand", "price", "size", "organic"];
    let columns: Vec<Vec<f64>> = names
        .iter()
        .map(|n| values(&data, n)[..9].to_vec())
        .collect();
    // every pair of factors is uncorrelated in the experimental profiles
    for a in 0..4 {
        for b in 0..a {
            let (x, y) = (&columns[a], &columns[b]);
            let mx = x.iter().sum::<f64>() / 9.0;
            let my = y.iter().sum::<f64>() / 9.0;
            let cov: f64 = x.iter().zip(y).map(|(p, q)| (p - mx) * (q - my)).sum();
            assert!(cov.abs() < 1e-12, "{a} {b}");
        }
    }
    // the three-level factors are balanced, every level is used
    for c in &columns[..3] {
        let mut sorted = c.clone();
        sorted.sort_by(f64::total_cmp);
        assert_eq!(sorted.iter().filter(|v| **v == sorted[0]).count(), 3);
    }
    assert_eq!(columns[1].iter().filter(|v| **v == 7.0).count(), 3);
    assert!(columns[3].contains(&0.0) && columns[3].contains(&1.0));
    // holdouts repeat neither the plan nor each other
    let row = |i: usize| names.map(|n| values(&data, n)[i]);
    for h in 9..12 {
        assert!((0..h).all(|i| row(i) != row(h)));
    }
    let brand = data.variable("brand").unwrap();
    assert_eq!(brand.label(), Some("Brand"));
    assert_eq!(brand.value_labels().len(), 3);
    // the same seed gives the same plan
    run_on(
        &mut engine,
        &format!("SET SEED=4321.\n{FACTORS} /HOLDOUT=3 /REPLACE.\n"),
    );
    assert_eq!(
        values(engine.dataset().unwrap(), "price"),
        values(&data, "price")
    );
}

#[test]
fn orthoplan_minimum_and_simulation_cases() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!("SET SEED=99.\n{FACTORS} /MINIMUM=10 /HOLDOUT=2 /MIXHOLD=YES.\n"),
    );
    let data = engine.dataset().unwrap();
    let status = values(data, "STATUS_");
    assert_eq!(status.len(), 18);
    assert_eq!(status.iter().filter(|s| **s == 1.0).count(), 2);
    // cases already there become simulation profiles after the plan
    let source = "DATA LIST FREE /speed seats.\nVALUE LABELS speed 1 'slow' 2 'fast' 3 'faster'\n\
                  /seats 2 'two' 4 'four'.\nBEGIN DATA.\n3 4\n1 2\nEND DATA.\n\
                  DATASET DECLARE p.\nORTHOPLAN /OUTFILE=p.\nDATASET ACTIVATE p.\n";
    let tables = run_on(&mut engine, source);
    assert_eq!(number(&table(&tables, "Orthogonal Plan").rows[1][1]), 2.0);
    let data = engine.dataset().unwrap();
    let status = values(data, "STATUS_");
    assert_eq!(status, [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 2.0]);
    assert_eq!(&values(data, "speed")[6..], [3.0, 1.0]);
    assert_eq!(&values(data, "CARD_")[6..], [7.0, 8.0]);
}

#[test]
fn plancards_lists_and_cards() {
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!(
            "{PLAN}DATASET ACTIVATE plan.\nPLANCARDS FORMAT=BOTH /FACTORS=brand organic \
             /TITLE='Card )CARD' /FOOTER='Score:'.\n"
        ),
    );
    let list = table(&tables, "Card )CARD");
    assert_eq!(list.rows.len(), 12);
    assert_eq!(list.rows[9][0], Cell::Text("10 (Holdout)".into()));
    assert_eq!(list.rows[0][1], Cell::Text("Acme".into()));
    assert_eq!(table(&tables, "Simulation Profiles").rows.len(), 3);
    let card = table(&tables, "Card 12");
    assert_eq!(card.rows[0][1], Cell::Text("Brio".into()));
    assert_eq!(card.caption.as_deref(), Some("Score:"));
    assert!(tables.iter().all(|t| t.title != "Card 13"));
    let dir = std::env::temp_dir().join(format!("conjoint-cards-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("cards.txt");
    run_on(
        &mut engine,
        &format!(
            "PLANCARDS FORMAT=CARD /FACTORS=brand /TITLE='Card )CARD' /OUTFILE='{}'.\n",
            file.display()
        ),
    );
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.starts_with("Card 1\n\nbrand: Acme\n"));
    assert_eq!(text.matches("Card ").count(), 12);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_generated_plan_recovers_exact_utilities() {
    // scores made from known part-worths: brand +1, 0, -1; price -0.5 per euro; organic +-0.25
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!("SET SEED=7.\n{FACTORS} /REPLACE.\nDATASET NAME made.\n"),
    );
    let data = engine.dataset().unwrap().clone();
    let (brand, price, organic) = (
        values(&data, "brand"),
        values(&data, "price"),
        values(&data, "organic"),
    );
    let scores: Vec<String> = (0..brand.len())
        .map(|i| {
            let s = 20.0 + (2.0 - brand[i]) - 0.5 * price[i] + (organic[i] - 0.5) * 0.5;
            s.to_string()
        })
        .collect();
    let n = scores.len();
    let tables = run_on(
        &mut engine,
        &format!(
            "DATA LIST FREE /s1 TO s{n}.\nBEGIN DATA.\n{}\nEND DATA.\n\
             CONJOINT PLAN=made /FACTORS=price (LINEAR) /SCORE=s1 TO s{n}.\n",
            scores.join(" ")
        ),
    );
    let u = column(table(&tables, "Utilities"), 2);
    // brand, size, organic, price, constant
    let want = [
        1.0, 0.0, -1.0, 0.0, 0.0, 0.0, -0.25, 0.25, -2.5, -3.5, -4.5, 20.0,
    ];
    u.iter().zip(want).for_each(|(g, w)| close(*g, w));
}

/// IDEAL on a two-level factor cannot be fitted (its square repeats it), so the run stops with
/// that reason rather than failing later without one.
#[test]
fn ideal_needs_three_levels() {
    let mut engine = Engine::default();
    run_on(&mut engine, &format!("{PLAN}{}", rank_data()));
    let (commands, errors) =
        parse("CONJOINT PLAN=plan /FACTORS=organic (IDEAL) /SUBJECT=id /RANK=r1 TO r12.");
    assert!(errors.is_empty(), "{errors:?}");
    let error = engine
        .execute(&commands[0].command)
        .unwrap_err()
        .to_string();
    assert!(error.contains("need at least three"), "{error}");
}
