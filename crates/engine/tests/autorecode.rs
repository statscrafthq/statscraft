//! AUTORECODE. The expected codes and labels agree with the reference program;
//! `reference_data.rs` repeats the comparison on whole files.

use statscraft_data::{MissingValues, Value, Values};
use statscraft_engine::cell_text;
use statscraft_engine::session::{RunResult, Session};

const DATA: &str = "\
DATA LIST LIST /s (A3) n (F5.1) m (F3) t (A2).
BEGIN DATA
b 2.5 1 x
a 10 9 y
\"\" 2.5 9 z
c . 98 x
a -1 99 w
b 7 3 \"\"
END DATA.
MISSING VALUES n (7) m (98 99) t ('w').
VALUE LABELS n 10 'ten' / s 'a' 'Alpha'.
VARIABLE LABELS n 'Number'.
";

fn run(source: &str) -> (Session, RunResult) {
    let mut session = Session::default();
    let result = session
        .run(&format!("{DATA}{source}"), None)
        .unwrap_or_else(|errors| panic!("{errors:?}"));
    (session, result)
}

fn codes(session: &Session, name: &str) -> String {
    let data = session.dataset().unwrap();
    let Values::Numeric(values) = data.variable(name).unwrap().values() else {
        panic!()
    };
    let shown: Vec<String> = values
        .iter()
        .map(|v| v.map_or(".".into(), |v| v.to_string()))
        .collect();
    shown.join(" ")
}

fn labels(session: &Session, name: &str) -> Vec<String> {
    let data = session.dataset().unwrap();
    let variable = data.variable(name).unwrap();
    variable
        .value_labels()
        .iter()
        .map(|(value, label)| match value {
            Value::Number(n) => format!("{n}={label}"),
            Value::String(s) => format!("{s}={label}"),
        })
        .collect()
}

#[test]
fn values_become_codes_in_ascending_order_with_missing_values_last() {
    let (session, result) = run("AUTORECODE VARIABLES=s n /INTO rs rn /PRINT.");
    assert_eq!(codes(&session, "rs"), "3 2 1 4 2 3");
    assert_eq!(codes(&session, "rn"), "2 3 2 . 1 4");
    assert_eq!(labels(&session, "rs"), ["2=Alpha", "3=b", "4=c"]);
    assert_eq!(
        labels(&session, "rn"),
        ["1=  -1.0", "2=   2.5", "3=ten", "4=   7.0"]
    );
    let data = session.dataset().unwrap();
    let rn = data.variable("rn").unwrap();
    assert_eq!(rn.label(), Some("Number"));
    assert_eq!(rn.format(), "F1.0");
    assert_eq!(*rn.missing_values(), MissingValues::Numbers(vec![4.0]));
    let table = &result.commands.last().unwrap().tables[1];
    assert_eq!(table.title, "Recoding n into rn");
    assert_eq!(
        (0..3).map(|c| cell_text(table, 2, c)).collect::<Vec<_>>(),
        ["10.0", "3", "ten"]
    );
}

#[test]
fn descending_numbers_from_the_highest_value() {
    let (session, _) = run("AUTORECODE m /INTO rm /DESCENDING.");
    assert_eq!(codes(&session, "rm"), "3 1 1 5 4 2");
    let data = session.dataset().unwrap();
    assert_eq!(
        *data.variable("rm").unwrap().missing_values(),
        MissingValues::Numbers(vec![4.0, 5.0])
    );
}

#[test]
fn group_shares_one_scheme_and_blank_can_be_missing() {
    let (session, _) = run("AUTORECODE s t /INTO g1 g2 /GROUP /BLANK=MISSING.");
    // a b c x y z are valid; w is missing as t defines it; the blank is missing too
    assert_eq!(codes(&session, "g1"), "2 1 7 3 1 2");
    assert_eq!(codes(&session, "g2"), "4 5 6 4 8 7");
    assert_eq!(labels(&session, "g1")[0], "1=Alpha");
    let data = session.dataset().unwrap();
    assert_eq!(
        *data.variable("g2").unwrap().missing_values(),
        MissingValues::Numbers(vec![7.0, 8.0])
    );
}

#[test]
fn autorecode_errors() {
    let mut session = Session::default();
    session.run(DATA, None).unwrap();
    let message = |session: &mut Session, source: &str| {
        session.run(source, None).unwrap_err()[0].message.clone()
    };
    assert_eq!(
        message(&mut session, "AUTORECODE s n /INTO a."),
        "2 variables to recode but 1 after INTO; give one new name per variable"
    );
    assert_eq!(
        message(&mut session, "AUTORECODE s /INTO n."),
        "n already exists: AUTORECODE needs new variables after INTO"
    );
    assert_eq!(
        message(&mut session, "AUTORECODE s n /INTO a b /GROUP."),
        "GROUP needs variables of one type: all numeric or all strings"
    );
}

/// A folder of its own for the template files of a test.
fn folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "statscraft-autorecode-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run_plain(source: &str) -> Session {
    let mut session = Session::default();
    session
        .run(source, None)
        .unwrap_or_else(|errors| panic!("{errors:?}"));
    session
}

/// The values of the template file as text: "source=target" for each pair.
fn template(path: &std::path::Path) -> Vec<String> {
    let data = statscraft_data::Dataset::read_sav(path).unwrap();
    let Values::Numeric(targets) = data.variable("Target_").unwrap().values() else {
        panic!("Target_ is a string")
    };
    let sources: Vec<String> = match data.variable("Source_").unwrap().values() {
        Values::String(v) => v.iter().map(|s| s.trim_end().to_string()).collect(),
        Values::Numeric(v) => v.iter().map(|n| n.unwrap().to_string()).collect(),
    };
    sources
        .iter()
        .zip(targets.iter())
        .map(|(s, t)| format!("{s}={}", t.unwrap()))
        .collect()
}

// A saved template holds the valid values of its variable only (a user-missing value is left
// out); a template applied to other data keeps its codes and gives a value it lacks the next code.
#[test]
fn a_template_is_saved_and_applied_with_new_values_after_its_own() {
    let dir = folder("scheme");
    let file = dir.join("grade_template.sat");
    run_plain(&format!(
        "DATA LIST FREE /grade (a1) tier (a1).\nBEGIN DATA\nk t\nm u\nz v\nEND DATA.\n\
         MISSING VALUES grade ('z') tier ('v').\n\
         AUTORECODE VARIABLES=grade tier /INTO grade_n tier_n /SAVE TEMPLATE='{}'.",
        file.display()
    ));
    assert_eq!(template(&file), ["k=1", "m=2"]);

    let session = run_plain(&format!(
        "DATA LIST FREE /size (a1).\nBEGIN DATA\nk m q\nEND DATA.\n\
         AUTORECODE VARIABLES=size /INTO size_n /SAVE TEMPLATE='{}'.\n\
         DATA LIST FREE /width (a1).\nBEGIN DATA\nk m z\nEND DATA.\n\
         AUTORECODE VARIABLES=width /INTO width_n /APPLY TEMPLATE='{}'.",
        file.display(),
        file.display()
    ));
    assert_eq!(codes(&session, "width_n"), "1 2 4");
    assert_eq!(labels(&session, "width_n"), ["1=k", "2=m", "3=q", "4=z"]);
    std::fs::remove_dir_all(dir).ok();
}

// Applying a template and saving to the same file adds the new values to the template; the
// user-missing values of the first variable that has any follow the valid codes.
#[test]
fn applying_and_saving_the_same_template_extends_it() {
    let dir = folder("extend");
    let file = dir.join("products.sat");
    run_plain(&format!(
        "DATA LIST FREE /p (F2).\nBEGIN DATA\n10 30\nEND DATA.\n\
         AUTORECODE VARIABLES=p /INTO code /SAVE TEMPLATE='{}'.",
        file.display()
    ));
    assert_eq!(template(&file), ["10=1", "30=2"]);
    let session = run_plain(&format!(
        "DATA LIST FREE /p (F2) q (F2).\nBEGIN DATA\n20 5\n30 99\n10 20\n40 5\nEND DATA.\n\
         MISSING VALUES p (40).\n\
         AUTORECODE VARIABLES=p q /INTO cp cq /APPLY TEMPLATE='{}' /SAVE TEMPLATE='{}'.",
        file.display(),
        file.display()
    ));
    // template: 10=1, 30=2; then the other valid values 5, 20, 99 (3, 4, 5), then 40, missing (6)
    assert_eq!(codes(&session, "cp"), "4 2 1 6");
    assert_eq!(codes(&session, "cq"), "3 5 4 3");
    let data = session.dataset().unwrap();
    assert_eq!(
        *data.variable("cp").unwrap().missing_values(),
        MissingValues::Numbers(vec![6.0])
    );
    assert_eq!(template(&file), ["10=1", "30=2", "5=3", "20=4", "99=5"]);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn a_template_must_exist_and_match_the_type_of_the_variables() {
    let dir = folder("errors");
    let file = dir.join("numbers.sat");
    run_plain(&format!(
        "DATA LIST FREE /n (F2).\nBEGIN DATA\n1 2\nEND DATA.\n\
         AUTORECODE VARIABLES=n /INTO c /SAVE TEMPLATE='{}'.",
        file.display()
    ));
    let mut session = Session::default();
    let errors = session
        .run(
            &format!(
                "DATA LIST FREE /s (A2).\nBEGIN DATA\na\nEND DATA.\n\
                 AUTORECODE VARIABLES=s /INTO c /APPLY TEMPLATE='{}'.",
                file.display()
            ),
            None,
        )
        .unwrap_err();
    assert!(
        errors[0].message.contains("the template holds numbers"),
        "{errors:?}"
    );
    let errors = session
        .run(
            &format!(
                "DATA LIST FREE /s (A2).\nBEGIN DATA\na\nEND DATA.\n\
                 AUTORECODE VARIABLES=s /INTO d /APPLY TEMPLATE='{}'.",
                dir.join("nosuch.sat").display()
            ),
            None,
        )
        .unwrap_err();
    assert!(errors[0].message.contains("APPLY TEMPLATE"), "{errors:?}");
    std::fs::remove_dir_all(dir).ok();
}
