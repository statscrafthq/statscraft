//! SAVE CODEPAGE: the bytes of data files written in a code page, and what a reader makes of
//! them (our own reader, and R's haven reading the same files, which gave the same text).

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::Session;
use std::path::{Path, PathBuf};

const DATA: &str = "DATA LIST LIST /id (F2) name (A12) city (A10) amount (F6.2).
BEGIN DATA
1 'Zo\u{eb}' 'K\u{f6}ln' 10.5
2 'Ren\u{e9}e' 'Lyon' 20
3 'Ana' 'Z\u{fc}rich' 7.25
END DATA.
VARIABLE LABELS name 'Caf\u{e9} owner'.
VALUE LABELS city 'Lyon' 'Cit\u{e9} de Lyon'.
FILE LABEL Pr\u{fc}fung.
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("statscraft-codepage-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn session(dir: &Path, data: &str) -> Session {
    let mut session = Session::default();
    session.set_base_dir(Some(dir.to_path_buf()));
    let source = format!("CD '{}'.\n{data}", dir.display());
    session
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    session
}

fn run(session: &mut Session, syntax: &str) {
    session
        .run(syntax, None)
        .unwrap_or_else(|e| panic!("{syntax}: {e:?}"));
}

fn error(session: &mut Session, syntax: &str) -> String {
    session.run(syntax, None).unwrap_err()[0].message.clone()
}

fn contains(bytes: &[u8], needle: &[u8]) -> bool {
    bytes.windows(needle.len()).any(|w| w == needle)
}

fn strings(data: &Dataset, name: &str) -> Vec<String> {
    match data.variable(name).unwrap().values() {
        Values::String(v) => v.iter().map(str::to_string).collect(),
        Values::Numeric(_) => panic!("{name} is numeric"),
    }
}

#[test]
fn the_text_of_the_file_is_in_the_code_page_and_reads_back() {
    let dir = temp_dir("latin1");
    let mut s = session(&dir, DATA);
    run(&mut s, "SAVE CODEPAGE OUTFILE='l.sav' /ENCODING='1252'.");
    let b = std::fs::read(dir.join("l.sav")).unwrap();
    // one byte for each of the letters with accents, not the two of UTF-8
    assert!(contains(&b, b"Zo\xeb"), "Zoe with a diaeresis");
    assert!(contains(&b, b"K\xf6ln"));
    assert!(contains(&b, b"Caf\xe9 owner"));
    assert!(contains(&b, b"Pr\xfcfung"));
    assert!(!contains(&b, "\u{eb}".as_bytes()), "no UTF-8 left");
    // the file names its code page: 1252 in the integer information record and the encoding
    assert!(contains(&b, b"windows-1252"));
    assert!(contains(&b, &1252_i32.to_le_bytes()));
    // our reader decodes it by that name
    run(&mut s, "GET FILE='l.sav'.");
    let data = s.dataset().unwrap();
    assert_eq!(strings(data, "name"), ["Zo\u{eb}", "Ren\u{e9}e", "Ana"]);
    assert_eq!(strings(data, "city")[0], "K\u{f6}ln");
    assert_eq!(
        data.variable("name").unwrap().label(),
        Some("Caf\u{e9} owner")
    );
    assert_eq!(
        data.variable("city").unwrap().value_labels()[0].1,
        "Cit\u{e9} de Lyon"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_baltic_code_page_has_its_own_bytes() {
    let dir = temp_dir("baltic");
    let data = "DATA LIST LIST /city (A12).\nBEGIN DATA\n'\u{17d}aliakalnis'\n'\u{160}iauliai'\nEND DATA.\n";
    let mut s = session(&dir, data);
    run(
        &mut s,
        "SAVE CODEPAGE OUTFILE='b.sav' /ENCODING='windows-1257'.",
    );
    let b = std::fs::read(dir.join("b.sav")).unwrap();
    // Z with a caron is 0xDE and S with a caron 0xD0 in Windows-1257
    assert!(contains(&b, b"\xdealiakalnis"));
    assert!(contains(&b, b"\xd0iauliai"));
    assert!(contains(&b, &1257_i32.to_le_bytes()));
    run(&mut s, "GET FILE='b.sav'.");
    assert_eq!(
        strings(s.dataset().unwrap(), "city"),
        ["\u{17d}aliakalnis", "\u{160}iauliai"]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_code_page_is_a_number_a_name_or_the_locale() {
    let dir = temp_dir("names");
    let mut s = session(&dir, DATA);
    for (i, spelling) in [
        "'1252'",
        "'windows-1252'",
        "'cp1252'",
        "'iso-8859-1'",
        "LOCALE",
        "",
    ]
    .iter()
    .enumerate()
    {
        let encoding = if spelling.is_empty() {
            String::new()
        } else {
            format!("/ENCODING={spelling}")
        };
        run(
            &mut s,
            &format!("SAVE CODEPAGE OUTFILE='n{i}.sav' {encoding}."),
        );
        let b = std::fs::read(dir.join(format!("n{i}.sav"))).unwrap();
        assert!(contains(&b, b"Zo\xeb"), "{spelling}");
    }
    assert!(
        error(&mut s, "SAVE CODEPAGE OUTFILE='x.sav' /ENCODING='klingon'.")
            .contains("unknown encoding klingon")
    );
    assert!(
        error(&mut s, "SAVE CODEPAGE OUTFILE='x.sav' /ENCODING='UTF16'.")
            .contains("cannot write a file in the encoding")
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn text_the_code_page_lacks_stops_the_command_and_writes_nothing() {
    let dir = temp_dir("lacks");
    let data = "DATA LIST LIST /word (A8).\nBEGIN DATA\n'\u{416}ukov'\nEND DATA.\n";
    let mut s = session(&dir, data);
    let message = error(&mut s, "SAVE CODEPAGE OUTFILE='c.sav' /ENCODING='1252'.");
    assert!(message.contains("cannot hold"), "{message}");
    assert!(!dir.join("c.sav").exists());
    // the Cyrillic code page holds it
    run(&mut s, "SAVE CODEPAGE OUTFILE='c.sav' /ENCODING='1251'.");
    assert!(contains(
        &std::fs::read(dir.join("c.sav")).unwrap(),
        b"\xc6ukov"
    ));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn keep_drop_and_unselected_shape_the_file_and_the_active_dataset_stays() {
    let dir = temp_dir("shape");
    let mut s = session(&dir, DATA);
    run(
        &mut s,
        "COMPUTE keep = id > 1.\nFILTER BY keep.\nSAVE CODEPAGE OUTFILE='k.sav' /ENCODING='1252' /UNSELECTED=DELETE /KEEP=city id /DROP=id.",
    );
    // the active dataset is as it was: its variables, its cases, UTF-8
    let before = s.dataset().unwrap();
    assert_eq!(before.case_count(), 3);
    assert_eq!(before.variable_names().count(), 5);
    run(&mut s, "FILTER OFF.\nGET FILE='k.sav'.");
    let data = s.dataset().unwrap();
    assert_eq!(data.variable_names().collect::<Vec<_>>(), ["city"]);
    assert_eq!(strings(data, "city"), ["Lyon", "Z\u{fc}rich"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_password_is_refused_and_none_is_not() {
    let dir = temp_dir("password");
    let mut s = session(&dir, DATA);
    run(
        &mut s,
        "SAVE CODEPAGE OUTFILE='p.sav' /PASSPROTECT PASSWORD=NONE ENCRYPTEDPW=NO.",
    );
    let message = error(
        &mut s,
        "SAVE CODEPAGE OUTFILE='q.sav' /PASSPROTECT PASSWORD='secret'.",
    );
    assert!(message.contains("does not encrypt"), "{message}");
    assert!(!dir.join("q.sav").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn what_the_syntax_needs_is_checked() {
    let dir = temp_dir("syntax");
    let mut s = session(&dir, DATA);
    for (syntax, wanted) in [
        ("SAVE CODEPAGE.", "needs OUTFILE"),
        (
            "SAVE CODEPAGE OUTFILE='x.sav' /ENCODING='1252' /ENCODING='1257'.",
            "given only once",
        ),
        (
            "SAVE CODEPAGE OUTFILE='x.sav' /RENAME=(id = number).",
            "subcommand RENAME of SAVE CODEPAGE",
        ),
        (
            "SAVE CODEPAGE OUTFILE='x.sav' /COMPRESSED.",
            "subcommand COMPRESSED of SAVE CODEPAGE",
        ),
    ] {
        let message = error(&mut s, syntax);
        assert!(message.contains(wanted), "{syntax}: {message}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
