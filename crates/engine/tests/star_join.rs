//! STAR JOIN on data made up in the tests: lookups by key without sorting, several keys, the
//! same lookup twice, names that two sources bring, the dictionary of the result, files in and
//! out, and the errors.

use statscraft_data::{Dataset, Value, Values};
use statscraft_engine::Engine;
use statscraft_syntax::parse;

fn run(engine: &mut Engine, source: &str) {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            panic!("line {}: {error}", command.first_line);
        }
    }
}

/// The message of the error the last command of `source` ends with; the others must run.
fn error(engine: &mut Engine, source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let (last, before) = commands.split_last().unwrap();
    for command in before {
        engine.execute(&command.command).unwrap();
    }
    engine.execute(&last.command).unwrap_err().to_string()
}

/// Numbers separated by spaces with `.` for system-missing, strings by `|`.
fn show(data: &Dataset, name: &str) -> String {
    match data.variable(name).unwrap().values() {
        Values::Numeric(values) => {
            let text = values
                .iter()
                .map(|v| v.map_or(".".to_string(), |v| v.to_string()));
            text.collect::<Vec<_>>().join(" ")
        }
        Values::String(values) => values.iter().collect::<Vec<_>>().join("|"),
    }
}

fn names(data: &Dataset) -> Vec<&str> {
    data.variable_names().collect()
}

/// Cases in no key order: one region (5) and one size (4) no lookup holds, one region missing.
const FACTS: &str = "\
DATA LIST LIST /id region size (F2.0) name (A6).
BEGIN DATA
3 2 1 Ann
1 1 3 Bob
2 2 2 Cy
4 5 4 Dee
5 . 1 Eve
END DATA.
DATASET NAME facts.
DATA LIST LIST /region (F2.0) rname (A5).
BEGIN DATA
9 West
2 South
1 North
3 East
END DATA.
DATASET NAME regions.
DATA LIST LIST /size (F2.0) label (A6).
BEGIN DATA
3 large
1 small
2 medium
END DATA.
DATASET NAME sizes.
DATASET ACTIVATE facts.
";

fn facts() -> Engine {
    let mut engine = Engine::default();
    run(&mut engine, FACTS);
    engine
}

#[test]
fn every_case_stays_and_each_lookup_adds_its_fields_by_its_own_key() {
    let mut engine = facts();
    run(
        &mut engine,
        "STAR JOIN\n  /SELECT t1.id, t1.name, t2.rname, t3.label\n  /FROM * AS t1\n\
         \x20 /JOIN regions AS t2 ON t1.region = t2.region IN=inreg\n\
         \x20 /JOIN sizes AS t3 ON t1.size = t3.size\n  /OUTFILE FILE=*.",
    );
    let data = engine.dataset().unwrap();
    // the keys SELECT leaves out come first, the IN variable last
    assert_eq!(
        names(data),
        ["region", "size", "id", "name", "rname", "label", "inreg"]
    );
    assert_eq!(show(data, "id"), "3 1 2 4 5");
    assert_eq!(show(data, "region"), "2 1 2 5 .");
    assert_eq!(show(data, "rname"), "South|North|South||");
    assert_eq!(show(data, "label"), "small|large|medium||small");
    assert_eq!(show(data, "inreg"), "1 1 1 0 0");
    assert_eq!(data.variable("inreg").unwrap().format(), "F1.0");
    // the lookups are left as they were
    run(&mut engine, "DATASET ACTIVATE regions.");
    assert_eq!(engine.dataset().unwrap().case_count(), 4);
}

#[test]
fn select_star_keeps_every_field_once_and_a_key_of_another_name_too() {
    let mut engine = facts();
    run(
        &mut engine,
        "DATA LIST LIST /code (F2.0) word (A5).\nBEGIN DATA\n1 one\n3 three\nEND DATA.\n\
         DATASET NAME words.\nDATASET ACTIVATE facts.\n\
         STAR JOIN /SELECT * /FROM * AS f /JOIN regions AS r ON f.region = r.region\n\
         \x20 /JOIN words AS w ON f.size = w.code /OUTFILE FILE=*.",
    );
    let data = engine.dataset().unwrap();
    assert_eq!(
        names(data),
        ["id", "region", "size", "name", "rname", "code", "word"]
    );
    assert_eq!(show(data, "code"), "1 3 . . 1");
    assert_eq!(show(data, "word"), "one|three|||one");
}

#[test]
fn one_lookup_serves_two_joins_and_its_missing_keys_match_nothing() {
    let mut engine = facts();
    // regions and sizes in one dataset; size is missing in its last case
    run(
        &mut engine,
        "DATA LIST LIST /region (F2.0) rname (A5) size (F2.0) label (A6).\nBEGIN DATA\n\
         1 North 2 medium\n2 South 1 small\n3 East . none\nEND DATA.\n\
         DATASET NAME both.\nDATASET ACTIVATE facts.\n\
         STAR JOIN /SELECT t1.id, t2.rname, t3.label /FROM * AS t1\n\
         \x20 /JOIN both AS t2 ON t1.region = t2.region\n\
         \x20 /JOIN both AS t3 ON t1.size = t3.size /OUTFILE FILE=*.",
    );
    let data = engine.dataset().unwrap();
    assert_eq!(names(data), ["region", "size", "id", "rname", "label"]);
    assert_eq!(show(data, "rname"), "South|North|South||");
    assert_eq!(show(data, "label"), "small||medium||small");
}

#[test]
fn several_keys_match_together_and_string_widths_may_differ() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        "DATA LIST LIST /first (A8) last (A10) n.\nBEGIN DATA\n\
         Ann Lee 1\nAnn Kask 2\nBob Lee 3\nEND DATA.\nDATASET NAME people.\n\
         DATA LIST LIST /first (A3) last (A4) city (A7).\nBEGIN DATA\n\
         Bob Lee Tartu\nAnn Lee Tallinn\nAnn Tamm Narva\nEND DATA.\nDATASET NAME towns.\n\
         DATASET ACTIVATE people.\n\
         STAR JOIN /SELECT p.first, p.last, p.n, t.city /FROM * AS p\n\
         \x20 /JOIN towns AS t ON p.first = t.first AND t.last = p.last /OUTFILE FILE=*.",
    );
    let data = engine.dataset().unwrap();
    assert_eq!(names(data), ["first", "last", "n", "city"]);
    assert_eq!(show(data, "city"), "Tallinn||Tartu");
    assert_eq!(data.variable("first").unwrap().format(), "A8");
}

#[test]
fn a_name_two_sources_bring_gets_its_alias_unless_as_names_it() {
    let mut engine = facts();
    run(
        &mut engine,
        "DATA LIST LIST /region (F2.0) name (A5).\nBEGIN DATA\n1 n1\n2 n2\nEND DATA.\n\
         DATASET NAME names.\nDATASET ACTIVATE facts.\nDATASET DECLARE out1.\n\
         STAR JOIN /SELECT a.id, a.name, b.name /FROM * AS a\n\
         \x20 /JOIN names AS b ON a.region = b.region /OUTFILE FILE=out1.\n\
         DATASET ACTIVATE out1.",
    );
    let data = engine.dataset().unwrap();
    assert_eq!(names(data), ["region", "id", "a.name", "b.name"]);
    assert_eq!(show(data, "b.name"), "n2|n1|n2||");
    run(
        &mut engine,
        "DATASET ACTIVATE facts.\nDATASET DECLARE out2.\n\
         STAR JOIN /SELECT a.id AS key, a.name, b.name AS other /FROM * AS a\n\
         \x20 /JOIN names AS b ON a.region = b.region /OUTFILE FILE=out2.\n\
         DATASET ACTIVATE out2.",
    );
    let data = engine.dataset().unwrap();
    assert_eq!(names(data), ["region", "key", "name", "other"]);
    let message = error(
        &mut engine,
        "DATASET ACTIVATE facts.\n\
         STAR JOIN /SELECT a.id, b.name AS id /FROM * AS a\n\
         \x20 /JOIN names AS b ON a.region = b.region /OUTFILE FILE=*.",
    );
    assert_eq!(
        message,
        "STAR JOIN would make two variables named id; give one another name with AS"
    );
}

/// The form a dialog writes: a quoted dataset name, field names with periods, the key left out
/// of SELECT; the commands after it name the key first.
#[test]
fn the_pasted_form_with_a_quoted_dataset_and_names_with_periods() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        "DATA LIST LIST /intnr k9.1 (F2.0) mudel.1 (A8).\nBEGIN DATA\n20 7 Golf\n10 8 Polo\nEND DATA.\n\
         DATASET NAME DataSet4.\n\
         DATA LIST LIST /respondentkey intnr k1 k9.\nBEGIN DATA\n1 10 4 1\n2 20 5 2\n3 30 6 1\nEND DATA.\n\
         DATASET NAME mudel.\nDATASET ACTIVATE mudel.\n\
         STAR JOIN\n  /SELECT t0.RespondentKey, t0.K1, t0.K9,\n    t1.K9.1, t1.Mudel.1\n\
         \x20 /FROM * AS t0\n  /JOIN 'DataSet4' AS t1\n    ON t0.INTNR=t1.INTNR\n  /OUTFILE FILE=*.\n\
         ADD FILES FILE * /KEEP intnr TO k9 k9.1 mudel.1 ALL.",
    );
    let data = engine.dataset().unwrap();
    assert_eq!(
        names(data),
        ["intnr", "respondentkey", "k1", "k9", "k9.1", "mudel.1"]
    );
    assert_eq!(show(data, "k9.1"), "8 7 .");
    assert_eq!(show(data, "mudel.1"), "Polo|Golf|");
}

#[test]
fn a_key_takes_its_dictionary_from_the_first_file_with_one_and_documents_add_up() {
    let mut engine = Engine::default();
    run(
        &mut engine,
        "DATA LIST LIST /region (F2.0) rname (A5).\nBEGIN DATA\n1 North\n2 South\nEND DATA.\n\
         VARIABLE LABELS region 'Region of the case'.\nVALUE LABELS region 1 'N' 2 'S'.\n\
         DOCUMENT Regions from the 2025 list.\nFILE LABEL Regions.\n\
         DATASET NAME regions.\n\
         DATA LIST LIST /id region.\nBEGIN DATA\n1 2\n2 1\nEND DATA.\n\
         DOCUMENT Cases of the test.\nFILE LABEL Cases.\nWEIGHT BY id.\n\
         STAR JOIN /SELECT t1.id, t2.rname /FROM * AS t1\n\
         \x20 /JOIN regions AS t2 ON t1.region = t2.region /OUTFILE FILE=*.",
    );
    let data = engine.dataset().unwrap();
    let region = data.variable("region").unwrap();
    assert_eq!(region.label(), Some("Region of the case"));
    assert_eq!(
        region.value_labels(),
        [
            (Value::Number(1.0), "N".to_string()),
            (Value::Number(2.0), "S".to_string())
        ]
    );
    let documents = data.documents().join("\n");
    assert!(documents.contains("Cases of the test"), "{documents}");
    assert!(
        documents.contains("Regions from the 2025 list"),
        "{documents}"
    );
    assert_eq!(data.file_label(), None);
    // the weight of FROM stays
    assert_eq!(data.weight_variable(), Some("id"));
}

#[test]
fn files_in_and_out_and_passprotect() {
    let dir = std::env::temp_dir().join(format!("statscraft-star-join-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = |name: &str| dir.join(name).display().to_string();
    let mut engine = facts();
    run(
        &mut engine,
        &format!(
            "DATASET ACTIVATE regions.\nSAVE OUTFILE='{}'.\nDATASET ACTIVATE facts.\n\
             STAR JOIN /SELECT t1.id, t2.rname /FROM * AS t1\n\
             \x20 /JOIN '{}' PASSWORD='not needed' AS t2 ON t1.region = t2.region\n\
             \x20 /OUTFILE FILE='{}' /PASSPROTECT PASSWORD=NONE.",
            path("regions.sav"),
            path("regions.sav"),
            path("merged.sav")
        ),
    );
    // the active dataset is the one it was
    assert_eq!(
        names(engine.dataset().unwrap()),
        ["id", "region", "size", "name"]
    );
    let merged = Dataset::read_sav(path("merged.sav")).unwrap();
    assert_eq!(names(&merged), ["region", "id", "rname"]);
    assert_eq!(show(&merged, "rname"), "South|North|South||");
    let message = error(
        &mut engine,
        &format!(
            "STAR JOIN /SELECT t1.id /FROM * AS t1 /JOIN regions AS t2 ON t1.region = t2.region\n\
             \x20 /OUTFILE FILE='{}' /PASSPROTECT PASSWORD='secret' ENCRYPTEDPW=NO.",
            path("locked.sav")
        ),
    );
    assert_eq!(
        message,
        format!(
            "/PASSPROTECT: Statscraft cannot write encrypted data files, so {} was not written; \
             leave PASSPROTECT out or give PASSWORD=NONE",
            path("locked.sav")
        )
    );
    assert!(!dir.join("locked.sav").exists());
    // into a declared dataset, where PASSPROTECT does not apply
    run(
        &mut engine,
        "DATASET DECLARE joined.\n\
         STAR JOIN /SELECT t1.id /FROM * AS t1 /JOIN sizes AS t2 ON t1.size = t2.size\n\
         \x20 /OUTFILE FILE=joined /PASSPROTECT PASSWORD='secret'.\nDATASET ACTIVATE joined.",
    );
    assert_eq!(names(engine.dataset().unwrap()), ["size", "id"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn errors_name_the_key_and_the_fix() {
    let mut engine = facts();
    let message = error(
        &mut engine,
        "DATA LIST LIST /region (F2.0) rname (A5).\nBEGIN DATA\n1 North\n2 South\n1 Again\nEND DATA.\n\
         DATASET NAME twice.\nDATASET ACTIVATE facts.\n\
         STAR JOIN /SELECT t1.id /FROM * AS t1 /JOIN twice AS t2 ON t1.region = t2.region\n\
         \x20 /OUTFILE FILE=*.",
    );
    assert_eq!(
        message,
        "the keys of t2 (region) must be unique there, but cases 1 and 3 hold the same key"
    );
    let message = error(
        &mut engine,
        "DATASET ACTIVATE facts.\n\
         STAR JOIN /SELECT t1.id /FROM * AS t1 /JOIN regions AS t2 ON t1.name = t2.region\n\
         \x20 /OUTFILE FILE=*.",
    );
    assert_eq!(
        message,
        "the key t1.name is a string but t2.region is numeric; a key pair must be of one type"
    );
    let message = error(
        &mut engine,
        "STAR JOIN /SELECT t1.id, t2.nothing /FROM * AS t1 /JOIN regions AS t2\n\
         \x20 ON t1.region = t2.region /OUTFILE FILE=*.",
    );
    assert_eq!(message, "t2.nothing: regions has no variable nothing");
    let message = error(
        &mut engine,
        "TEMPORARY.\nSTAR JOIN /SELECT t1.id /FROM * AS t1 /JOIN regions AS t2\n\
         \x20 ON t1.region = t2.region /OUTFILE FILE=*.",
    );
    assert_eq!(
        message,
        "STAR JOIN cannot follow TEMPORARY: run a procedure or EXECUTE first"
    );
}
