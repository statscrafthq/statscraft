//! The dictionary commands, on made-up data.

use statscraft_data::{Dataset, Value, Values};
use statscraft_engine::session::{RunResult, Session};

const DATA: &str = "\
DATA LIST LIST /id (F3) score (F5.1) name (A12) code (A6).
BEGIN DATA
1 10.5 Anna 12
2 20 Bob x7
3 . 'Christopher' '1,250'
END DATA.
VALUE LABELS score 10.5 'low' 20 'high'.
";

fn session() -> Session {
    let mut session = Session::default();
    run(&mut session, DATA);
    session
}

fn run(session: &mut Session, source: &str) -> RunResult {
    session
        .run(source, None)
        .unwrap_or_else(|errors| panic!("{errors:?}"))
}

fn strings(data: &Dataset, name: &str) -> Vec<String> {
    match data.variable(name).unwrap().values() {
        Values::String(v) => v.iter().map(str::to_string).collect(),
        Values::Numeric(_) => panic!("{name} is numeric"),
    }
}

fn numbers(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => v.iter().collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

#[test]
fn labels_documents_and_display_settings() {
    let mut s = session();
    run(
        &mut s,
        "ADD VALUE LABELS score 20 'top' 30 'max' / name 'Bob' 'B'.\n\
         FILE LABEL Wave 2, cleaned.\n\
         DOCUMENT First line\n  second line.\n\
         ADD DOCUMENT 'Third' ''.\n\
         VARIABLE ROLE /TARGET score /NONE id.\n\
         VARIABLE WIDTH id (4) / name (20).\n\
         VARIABLE ALIGNMENT name (CENTER).",
    );
    let data = s.dataset().unwrap();
    let labels = data.variable("score").unwrap().value_labels().to_vec();
    assert_eq!(
        labels,
        [
            (Value::Number(10.5), "low".to_string()),
            (Value::Number(20.0), "top".to_string()),
            (Value::Number(30.0), "max".to_string()),
        ]
    );
    assert_eq!(data.file_label(), Some("Wave 2, cleaned"));
    let documents = data.documents();
    assert_eq!(documents[0], "First line");
    assert_eq!(documents[1], "  second line");
    assert!(documents[2].starts_with("(Entered 20"));
    assert_eq!(documents[3..5], ["Third", ""]);
    let score = data.variable("score").unwrap();
    assert_eq!(score.role(), Some("target"));
    let name = data.variable("name").unwrap();
    assert_eq!(
        (name.display_width(), name.alignment()),
        (Some(20), Some("center"))
    );
    run(&mut s, "DROP DOCUMENTS.");
    assert!(s.dataset().unwrap().documents().is_empty());
}

#[test]
fn attributes_set_arrays_and_delete() {
    let mut s = session();
    run(
        &mut s,
        "VARIABLE ATTRIBUTE VARIABLES=id score ATTRIBUTE=Source('panel') Note[2]('two')\n\
         /VARIABLES=name ATTRIBUTE=Source('typed').\n\
         DATAFILE ATTRIBUTE ATTRIBUTE=Version('1') Rev[1]('a') Rev[3]('c').\n\
         DATAFILE ATTRIBUTE DELETE=Rev[1] Version.",
    );
    let data = s.dataset().unwrap();
    let id = data.variable("id").unwrap().attributes().to_vec();
    assert_eq!(id[0], ("Source".to_string(), vec!["panel".to_string()]));
    assert_eq!(
        id[1],
        ("Note".to_string(), vec![String::new(), "two".to_string()])
    );
    assert_eq!(
        data.attributes(),
        [("Rev".to_string(), vec![String::new(), "c".to_string()])]
    );
}

#[test]
fn alter_type_changes_widths_and_types() {
    let mut s = session();
    run(
        &mut s,
        "ALTER TYPE name (A4) code (A=AMIN + 1).\n\
         ALTER TYPE id (A3) score (F8.3).",
    );
    let data = s.dataset().unwrap();
    assert_eq!(strings(data, "name"), ["Anna", "Bob", "Chri"]);
    assert_eq!(data.variable("code").unwrap().format(), "A6");
    assert_eq!(strings(data, "id"), ["1", "2", "3"]);
    assert_eq!(data.variable("score").unwrap().format(), "F8.3");
    run(&mut s, "ALTER TYPE code (F8.0).");
    let data = s.dataset().unwrap();
    assert_eq!(numbers(data, "code"), [Some(12.0), None, Some(1250.0)]);
    // an input format limits the change: score is numeric, so A=A2 leaves it alone
    run(&mut s, "ALTER TYPE score name (A=A2).");
    let data = s.dataset().unwrap();
    assert_eq!(strings(data, "name"), ["An", "Bo", "Ch"]);
    assert_eq!(data.variable("score").unwrap().format(), "F8.3");
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-dict-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn save_and_get_keep_drop_and_rename_and_apply_dictionary() {
    let dir = temp_dir("files");
    let path = |f: &str| dir.join(f).to_string_lossy().into_owned();
    let mut s = session();
    run(
        &mut s,
        &format!(
            "VARIABLE LABELS score 'Test score'.\n\
             MISSING VALUES score (99).\n\
             SAVE OUTFILE='{}' /DROP=code /RENAME=(score = points) /KEEP=points ALL.",
            path("a.sav")
        ),
    );
    // the active dataset stays as it was
    assert_eq!(s.dataset().unwrap().variable_count(), 4);
    let saved = Dataset::read_sav(path("a.sav")).unwrap();
    let names: Vec<&str> = saved.variable_names().collect();
    assert_eq!(names, ["points", "id", "name"]);
    run(
        &mut s,
        &format!("GET FILE='{}' /KEEP=id points.", path("a.sav")),
    );
    let names: Vec<String> = s
        .dataset()
        .unwrap()
        .variable_names()
        .map(str::to_string)
        .collect();
    assert_eq!(names, ["id", "points"]);
    // the dictionary of a.sav goes onto a dataset without labels
    run(
        &mut s,
        &format!(
            "DATA LIST LIST /points id.\nBEGIN DATA\n1 2\nEND DATA.\n\
             APPLY DICTIONARY FROM '{}' /VARINFO VARLABEL MISSING.",
            path("a.sav")
        ),
    );
    let points = s.dataset().unwrap().variable("points").unwrap();
    assert_eq!(points.label(), Some("Test score"));
    assert!(points.value_labels().is_empty());
    assert_eq!(
        *points.missing_values(),
        statscraft_data::MissingValues::Numbers(vec![99.0])
    );
    // one source variable onto several targets
    run(
        &mut s,
        "VALUE LABELS points 1 'one'.\nAPPLY DICTIONARY FROM * /SOURCE VARIABLES=points /TARGET VARIABLES=id.",
    );
    let id = s.dataset().unwrap().variable("id").unwrap();
    assert_eq!(id.value_labels(), [(Value::Number(1.0), "one".to_string())]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn aggregate_accepts_presorted() {
    let mut s = session();
    run(&mut s, "AGGREGATE /OUTFILE=* /PRESORTED /BREAK=id /n = N.");
    assert_eq!(s.dataset().unwrap().case_count(), 3);
}
