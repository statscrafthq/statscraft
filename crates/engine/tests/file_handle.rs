//! FILE HANDLE: a handle stands for its path, quoted or not, whole or as the first part of a
//! path, in every command that takes a file, and lasts for the session.

use statscraft_engine::session::Session;

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "statscraft-file-handle-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(session: &mut Session, source: &str) -> statscraft_engine::session::RunResult {
    session
        .run(source, None)
        .unwrap_or_else(|errors| panic!("{errors:?}"))
}

fn ids(session: &Session) -> Vec<Option<f64>> {
    let data = session.dataset().unwrap();
    data.variable("id")
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect()
}

#[test]
fn a_handle_stands_for_a_file_or_a_folder_in_every_file_command() {
    let dir = temp_dir("commands");
    let d = dir.display();
    std::fs::write(dir.join("ids.txt"), "1\n2\n3\n").unwrap();
    std::fs::write(dir.join("more.csv"), "4\n5\n").unwrap();
    std::fs::write(dir.join("label.sps"), "VARIABLE LABELS id 'Case id'.\n").unwrap();
    let mut s = Session::default();
    run(
        &mut s,
        &format!(
            "FILE HANDLE folder /NAME='{d}'.\nFILE HAND ids /NAME='{d}/ids.txt' /MODE=CHARACTER.\n\
             FILE HANDLE out /NAME='{d}/ids.sav'.\n\
             DATA LIST LIST FILE=ids /id.\nSAVE OUTFILE=out.\n\
             GET DATA /TYPE=TXT /FILE='folder/more.csv' /VARIABLES=id F1.\n\
             SAVE OUTFILE='folder/more.sav'.\n\
             GET FILE='out'.\nADD FILES /FILE=* /FILE='folder/more.sav'.\n\
             MATCH FILES /FILE=* /TABLE=out /BY id.\nINSERT FILE='folder/label.sps'.\n\
             FILE HANDLE lab /NAME='{d}/label.sps'.\nINCLUDE lab.\nAPPLY DICTIONARY FROM out."
        ),
    );
    assert_eq!(
        ids(&s),
        [Some(1.0), Some(2.0), Some(3.0), Some(4.0), Some(5.0)]
    );
    let data = s.dataset().unwrap();
    assert_eq!(data.variable("id").unwrap().label(), Some("Case id"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn handles_last_from_run_to_run_and_a_dataset_comes_first() {
    let dir = temp_dir("session");
    let d = dir.display();
    let mut s = Session::default();
    run(
        &mut s,
        &format!(
            "DATA LIST LIST /id.\nBEGIN DATA\n7\nEND DATA.\nSAVE OUTFILE='{d}/seven.sav'.\n\
             FILE HANDLE seven /NAME='{d}/seven.sav' /RECFORM=FIXED /LRECL=80."
        ),
    );
    run(&mut s, "GET FILE=seven.");
    assert_eq!(ids(&s), [Some(7.0)]);
    // a dataset of the same name comes before the handle
    run(
        &mut s,
        "DATA LIST LIST /id.\nBEGIN DATA\n8\nEND DATA.\nDATASET NAME seven.\n\
         DATA LIST LIST /id.\nBEGIN DATA\n9\nEND DATA.\nADD FILES /FILE=* /FILE=seven.",
    );
    assert_eq!(ids(&s), [Some(9.0), Some(8.0)]);
    let settings = s.settings_syntax();
    assert!(
        settings.starts_with("FILE HANDLE SEVEN /NAME='"),
        "{settings}"
    );
    // a session put back from its settings knows the handle
    let mut next = Session::default();
    next.restore_settings(&settings).unwrap();
    run(&mut next, "GET FILE='seven'.");
    assert_eq!(ids(&next), [Some(7.0)]);
    let shown = run(&mut next, "SHOW HANDLES.");
    let table = &shown.commands[0].tables[0];
    assert!(format!("{:?}", table.rows[0][1]).contains("SEVEN = "));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_relative_name_starts_from_the_folder_of_the_cd_before_it() {
    let dir = temp_dir("relative");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    let d = dir.display();
    let mut s = Session::default();
    run(
        &mut s,
        &format!(
            "CD '{d}/sub'.\nFILE HANDLE here /NAME='x.sav'.\nCD '{d}'.\n\
             DATA LIST LIST /id.\nBEGIN DATA\n1\nEND DATA.\nSAVE OUTFILE=here."
        ),
    );
    assert!(dir.join("sub/x.sav").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn binary_modes_are_refused() {
    let mut s = Session::default();
    let errors = s
        .run("FILE HANDLE b /NAME='x.dat' /MODE=IMAGE /LRECL=16.", None)
        .unwrap_err();
    assert!(errors[0].message.contains("MODE=IMAGE"), "{errors:?}");
    let errors = s.run("FILE HANDLE b /MODE=CHARACTER.", None).unwrap_err();
    assert!(
        errors[0].expected.iter().any(|e| e.contains("NAME")),
        "{errors:?}"
    );
}
