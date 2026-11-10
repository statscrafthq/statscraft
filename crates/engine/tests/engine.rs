use statscraft_engine::{Cell, Changes, Engine, Table};
use statscraft_syntax::parse;

fn fixture(name: &str) -> String {
    format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn run(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    commands
        .iter()
        .flat_map(|c| engine.execute(&c.command).unwrap())
        .collect()
}

fn loaded(name: &str) -> Engine {
    let mut engine = Engine::default();
    run(&mut engine, &format!("GET FILE='{}'.", fixture(name)));
    engine
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

// Expected counts and percentages agree with the reference program on the same file:
// Male 106 (53.0%), Female 88 (44.0%), Other 6 (3.0%).
#[test]
fn frequencies_of_a_labelled_variable() {
    let tables = run(
        &mut loaded("survey_basic.sav"),
        "FREQUENCIES VARIABLES=gender.",
    );
    let t = &tables[0];
    assert_eq!(t.title, "gender: Gender");
    let counts: Vec<f64> = t.rows.iter().map(|r| number(&r[3])).collect();
    assert_eq!(counts, [106.0, 88.0, 6.0, 200.0]);
    assert_eq!(t.rows[0][2], Cell::Text("Male".into()));
    assert_eq!(number(&t.rows[0][4]), 53.0);
    assert_eq!(number(&t.rows[2][6]), 100.0);
}

#[test]
fn user_missing_values_are_reported_apart() {
    let tables = run(&mut loaded("survey_basic.sav"), "FREQ q1_satisfaction.");
    let rows = &tables[0].rows;
    let count = |category: &str| -> f64 {
        rows.iter()
            .filter(|r| r[0] == Cell::Text(category.into()))
            .map(|r| number(&r[3]))
            .sum()
    };
    assert_eq!(
        (count("valid"), count("missing"), count("total")),
        (179.0, 21.0, 200.0)
    );
    let missing_row = rows
        .iter()
        .find(|r| r[0] == Cell::Text("missing".into()))
        .unwrap();
    assert_eq!(
        missing_row[5],
        Cell::Empty,
        "missing rows have no valid percent"
    );
    let last_valid = rows
        .iter()
        .rfind(|r| r[0] == Cell::Text("valid".into()))
        .unwrap();
    assert!((number(&last_valid[6]) - 100.0).abs() < 1e-9);
}

// Weighted counts agree with the reference program on the same file:
// Male 132.3074, Female 119.3866, Other 8.2401, total 259.9341.
#[test]
fn weights_are_applied_when_the_file_has_a_weight_variable() {
    let tables = run(
        &mut loaded("survey_basic_phpwriter.sav"),
        "FREQUENCIES gender.",
    );
    let counts: Vec<f64> = tables[0].rows.iter().map(|r| number(&r[3])).collect();
    for (got, want) in counts.iter().zip([132.3074, 119.3866, 8.2401, 259.9341]) {
        assert!((got - want).abs() < 5e-5, "{got} vs {want}");
    }
    let unweighted = run(&mut loaded("survey_basic.sav"), "FREQUENCIES gender.");
    assert_eq!(number(&unweighted[0].rows.last().unwrap()[3]), 200.0);
    // a user-missing weight counts as 0, like a system-missing one
    let tables = run(
        &mut loaded("survey_basic_phpwriter.sav"),
        "MISSING VALUES weight (LO THRU 1).\nFREQUENCIES gender.",
    );
    let total = number(&tables[0].rows.last().unwrap()[3]);
    assert!(total > 0.0 && total < 259.9341, "{total}");
    let tables = run(
        &mut loaded("survey_basic_phpwriter.sav"),
        "MISSING VALUES weight (LO THRU HI).\nFREQUENCIES gender.",
    );
    assert_eq!(number(&tables[0].rows.last().unwrap()[3]), 0.0);
}

#[test]
fn dictionary_commands_change_the_dataset_and_are_summarised() {
    let mut engine = loaded("survey_basic.sav");
    let before = engine.dataset().unwrap().clone();
    run(
        &mut engine,
        "VARIABLE LABELS age 'Age at interview'.
VALUE LABELS gender 1 'M' 2 'F'.
MISSING VALUES age (LO THRU 17).
VARIABLE LEVEL q2_recommend (ORDINAL).
FORMATS monthly_spend (F10.1).",
    );
    let data = engine.dataset().unwrap();
    assert_eq!(
        data.variable("age").unwrap().label(),
        Some("Age at interview")
    );
    assert_eq!(data.variable("gender").unwrap().value_labels().len(), 2);
    assert_eq!(data.variable("monthly_spend").unwrap().format(), "F10.1");
    let changes = Changes::between(Some(&before), Some(data));
    assert_eq!(
        changes.variables_changed,
        ["gender", "age", "q2_recommend", "monthly_spend"]
    );
    assert!(changes.variables_added.is_empty() && changes.variables_removed.is_empty());
}

#[test]
fn variable_lists_expand_in_file_order() {
    let tables = run(&mut loaded("survey_basic.sav"), "FREQUENCIES q5_1 TO q5_3.");
    let names: Vec<&str> = tables
        .iter()
        .map(|t| t.title.split(':').next().unwrap())
        .collect();
    assert_eq!(names, ["q5_1", "q5_2", "q5_3"]);
}

#[test]
fn errors_are_specific() {
    let (commands, _) = parse("FREQUENCIES gender.");
    let error = Engine::default().execute(&commands[0].command).unwrap_err();
    assert!(error.to_string().contains("GET FILE"), "{error}");

    let mut engine = loaded("survey_basic.sav");
    let (commands, _) = parse("FREQUENCIES no_such_variable.\nFREQUENCIES age TO gender.");
    assert!(
        engine
            .execute(&commands[0].command)
            .unwrap_err()
            .to_string()
            .contains("no_such_variable")
    );
    assert!(
        engine
            .execute(&commands[1].command)
            .unwrap_err()
            .to_string()
            .contains("TO list")
    );
}

#[test]
fn dry_run_does_not_write_files() {
    let mut engine = loaded("survey_basic.sav");
    engine.dry_run = true;
    let path = std::env::temp_dir().join(format!("statscraft-dry-{}.sav", std::process::id()));
    run(&mut engine, &format!("SAVE OUTFILE='{}'.", path.display()));
    assert!(!path.exists());
}

#[test]
fn warnings_report_what_a_command_did_differently() {
    let mut engine = loaded("survey_basic.sav");
    // a range written high THRU low is read the other way round
    run(
        &mut engine,
        "RECODE age (60 THRU 18 = 1) (ELSE = 0) INTO young.",
    );
    assert_eq!(
        engine.warnings(),
        ["the range 60 THRU 18 has its high end first; it is read as 18 THRU 60"]
    );
    run(&mut engine, "COUNT n = q5_1 TO q5_5 (1 THRU 0).");
    assert_eq!(engine.warnings().len(), 1);
    run(&mut engine, "COUNT m = q5_1 TO q5_5 (0 THRU 1).");
    assert!(engine.warnings().is_empty());

    // MRSETS: equal variable labels and different labels of one value, ignoring case
    run(
        &mut engine,
        "VARIABLE LABELS q5_2 'Q5. Which of these channels have you used to contact us? PHONE'.
VALUE LABELS q5_3 0 'not selected' 1 'Ticked'.",
    );
    run(
        &mut engine,
        "MRSETS /MDGROUP NAME=$a VARIABLES=q5_1 TO q5_3 VALUE=1.",
    );
    assert_eq!(
        engine.warnings(),
        [
            "/MDGROUP $a: q5_1 and q5_2 have the same variable label, so their categories look alike",
            "/MDGROUP $a: q5_1 and q5_3 label the value 1 differently ('Selected' and 'Ticked')",
        ]
    );
    run(&mut engine, "MRSETS /MCGROUP NAME=$b VARIABLES=q5_1 q5_3.");
    assert_eq!(
        engine.warnings(),
        ["/MCGROUP $b: q5_1 and q5_3 label the value 1 differently ('Selected' and 'Ticked')"]
    );
    run(
        &mut engine,
        "MRSETS /MCGROUP NAME=$c VARIABLES=q6_mention1 TO q6_mention3.",
    );
    assert!(engine.warnings().is_empty());
}

#[test]
fn match_files_warns_about_repeated_keys_in_a_file() {
    let dir = std::env::temp_dir().join(format!("statscraft-warn-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let lookup = dir.join("regions.sav");
    let mut engine = loaded("survey_basic.sav");
    run(
        &mut engine,
        &format!(
            "AGGREGATE /OUTFILE='{}' /BREAK=region /n = N.\nSORT CASES BY region.",
            lookup.display()
        ),
    );
    let table = format!(
        "MATCH FILES /FILE=* /TABLE='{}' /BY region.",
        lookup.display()
    );
    run(&mut engine, &table);
    assert!(engine.warnings().is_empty());
    let file = format!(
        "MATCH FILES /FILE=* /FILE='{}' /BY region.",
        lookup.display()
    );
    run(&mut engine, &file);
    assert_eq!(
        engine.warnings(),
        ["FILE=* holds some keys more than once; cases with the same key are matched in order"]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
