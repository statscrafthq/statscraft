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
}

#[test]
fn dictionary_commands_change_the_dataset_and_are_summarised() {
    let mut engine = loaded("survey_basic.sav");
    let before = engine.dataset().unwrap().describe();
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
    let changes = Changes::between(Some(&before), Some(&data.describe()));
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
