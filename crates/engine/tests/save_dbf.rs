//! SAVE TRANSLATE /TYPE=DB3 and DB4: the bytes of files made up here, and what a dBASE reader
//! (R's foreign::read.dbf) and GET TRANSLATE make of them.

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::Session;
use std::path::{Path, PathBuf};

const DATA: &str =
    "DATA LIST LIST /id (F3) name (A6) amount (F8.2) born (ADATE10) elapsed (F5) big (F4).
BEGIN DATA
1 'Ann' 1.5 01/02/2020 22500 12
2 'Bobby' . 12/31/1999 . 12345
3 'Zoe' -0.004 02/29/2024 7 -5
END DATA.
FORMATS born (DATE11) elapsed (TIME8).
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-dbf-{tag}-{}", std::process::id()));
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

fn numbers(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => v.iter().collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

fn u16_at(b: &[u8], i: usize) -> usize {
    usize::from(u16::from_le_bytes([b[i], b[i + 1]]))
}

/// A field descriptor: name, type, length, decimals.
fn field(b: &[u8], n: usize) -> (String, char, usize, usize) {
    let d = &b[32 + 32 * n..64 + 32 * n];
    let end = d[..11].iter().position(|&x| x == 0).unwrap_or(11);
    (
        String::from_utf8(d[..end].to_vec()).unwrap(),
        char::from(d[11]),
        usize::from(d[16]),
        usize::from(d[17]),
    )
}

#[test]
fn a_dbase_iii_file_has_the_layout_and_the_values_of_its_print_formats() {
    let dir = temp_dir("layout");
    let mut s = session(&dir, DATA);
    run(&mut s, "SAVE TRANSLATE OUTFILE='a.dbf' /TYPE=DB3.");
    let b = std::fs::read(dir.join("a.dbf")).unwrap();
    assert_eq!(b[0], 0x03);
    assert_eq!(u32::from_le_bytes([b[4], b[5], b[6], b[7]]), 3);
    // AMOUNT is 9 wide: the data list gave it F9.2
    // header: 32 + 6 fields of 32 + the terminator; record: the flag and the field widths
    assert_eq!(u16_at(&b, 8), 225);
    assert_eq!(u16_at(&b, 10), 1 + 3 + 6 + 9 + 8 + 8 + 4);
    let want = [
        ("ID", 'N', 3, 0),
        ("NAME", 'C', 6, 0),
        ("AMOUNT", 'N', 9, 2),
        ("BORN", 'D', 8, 0),
        ("ELAPSED", 'N', 8, 0),
        ("BIG", 'N', 4, 0),
    ];
    for (n, (name, kind, length, decimals)) in want.iter().enumerate() {
        assert_eq!(field(&b, n), (name.to_string(), *kind, *length, *decimals));
    }
    assert_eq!(b[224], 0x0D);
    let records = std::str::from_utf8(&b[225..b.len() - 1]).unwrap();
    // a missing number is asterisks over the field, one that does not fit too (12345 in 4
    // places), and a value that rounds to zero keeps no minus sign
    let record = |parts: [&str; 6]| format!(" {}", parts.concat());
    assert_eq!(
        records,
        [
            record(["  1", "Ann   ", "     1.50", "20200102", "   22500", "  12"]),
            record(["  2", "Bobby ", "*********", "19991231", "********", "****"]),
            record(["  3", "Zoe   ", "     0.00", "20240229", "       7", "  -5"]),
        ]
        .concat()
    );
    assert_eq!(*b.last().unwrap(), 0x1A);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_file_reads_back_with_the_values_that_fit() {
    let dir = temp_dir("back");
    let mut s = session(&dir, DATA);
    run(&mut s, "SAVE TRANSLATE OUTFILE='a.dbf' /TYPE=DB4.");
    assert_eq!(std::fs::read(dir.join("a.dbf")).unwrap()[0], 0x04);
    run(&mut s, "GET TRANSLATE FILE='a.dbf' /TYPE=DBF.");
    let d = s.dataset().unwrap();
    assert_eq!(numbers(d, "ID"), [Some(1.0), Some(2.0), Some(3.0)]);
    assert_eq!(numbers(d, "AMOUNT"), [Some(1.5), None, Some(0.0)]);
    assert_eq!(numbers(d, "ELAPSED"), [Some(22500.0), None, Some(7.0)]);
    assert_eq!(numbers(d, "BIG"), [Some(12.0), None, Some(-5.0)]);
    // seconds from 1582-10-14: 2020-01-02, 1999-12-31, 2024-02-29
    assert_eq!(
        numbers(d, "BORN"),
        [
            Some(13797302400.0),
            Some(13165977600.0),
            Some(13928544000.0)
        ]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

fn warnings(s: &mut Session, syntax: &str) -> Vec<String> {
    let result = s
        .run(syntax, None)
        .unwrap_or_else(|e| panic!("{syntax}: {e:?}"));
    result
        .commands
        .iter()
        .flat_map(|c| c.warnings.clone())
        .collect()
}

#[test]
fn names_become_capitals_of_at_most_ten_bytes_and_stay_unique() {
    let dir = temp_dir("names");
    let data = "DATA LIST LIST /first.name very_long_name_a very_long_name_b @code (F1).\nBEGIN DATA\n1 2 3 4\nEND DATA.\n";
    let mut s = session(&dir, data);
    run(&mut s, "SAVE TRANSLATE OUTFILE='n.dbf' /TYPE=DB3.");
    let b = std::fs::read(dir.join("n.dbf")).unwrap();
    let names: Vec<String> = (0..4).map(|n| field(&b, n).0).collect();
    assert_eq!(names, ["FIRST_NAME", "VERY_LONG_", "VERY_LONG2", "V_CODE"]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn text_is_latin_1_blank_padded_and_a_lost_character_is_a_question_mark() {
    let dir = temp_dir("text");
    let data = "DATA LIST LIST /s (A6).\nBEGIN DATA\n'Zo\u{eb}'\n'\u{416}x'\n'.'\nEND DATA.\n";
    let mut s = session(&dir, data);
    let w = warnings(&mut s, "SAVE TRANSLATE OUTFILE='t.dbf' /TYPE=DB4.");
    assert_eq!(w.len(), 1, "{w:?}");
    assert!(
        w[0].contains("s: characters outside Windows Latin-1"),
        "{w:?}"
    );
    let b = std::fs::read(dir.join("t.dbf")).unwrap();
    assert_eq!(field(&b, 0), ("S".into(), 'C', 6, 0));
    // the flag of each record, then the six bytes of the text
    assert_eq!(&b[65..b.len() - 1], b" Zo\xeb    ?x     .     ");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn recode_user_missing_limits_overwrite_and_refused_options() {
    let dir = temp_dir("rules");
    let data = "DATA LIST LIST /n (F2) s (A3).\nBEGIN DATA\n1 'a'\n99 'zz'\nEND DATA.\nMISSING VALUES n (99) s ('zz').\n";
    let mut s = session(&dir, data);
    run(
        &mut s,
        "SAVE TRANSLATE OUTFILE='r.dbf' /TYPE=DB3 /MISSING=RECODE.",
    );
    let b = std::fs::read(dir.join("r.dbf")).unwrap();
    // flag, N (2), C (3): the user-missing number is asterisks and the string blanks
    assert_eq!(&b[97..b.len() - 1], b"  1a   **   ");
    // an existing file needs REPLACE
    let e = s
        .run("SAVE TRANSLATE OUTFILE='r.dbf' /TYPE=DB3.", None)
        .unwrap_err();
    assert!(e[0].message.contains("exists; add /REPLACE"), "{e:?}");
    run(&mut s, "SAVE TRANSLATE OUTFILE='r.dbf' /TYPE=DB3 /REPLACE.");
    // dBASE II, a type taken from the extension and ENCODING are refused
    for (syntax, wanted) in [
        (
            "SAVE TRANSLATE OUTFILE='x.dbf' /TYPE=DB2.",
            "SAVE TRANSLATE /TYPE=DB2 is not supported yet",
        ),
        (
            "SAVE TRANSLATE OUTFILE='x.dbf'.",
            "SAVE TRANSLATE needs /TYPE=CSV, TAB, STATA, SAS /VERSION=X, DB3, DB4 or XLS /VERSION=12",
        ),
        (
            "SAVE TRANSLATE OUTFILE='x.dbf' /TYPE=DB4 /ENCODING='UTF8'.",
            "a dBASE file holds Windows Latin-1 text",
        ),
    ] {
        let e = s.run(syntax, None).unwrap_err();
        assert!(e[0].message.contains(wanted), "{syntax}: {e:?}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_dbase_iii_file_holds_128_fields_and_dbase_iv_255() {
    let dir = temp_dir("limits");
    let names: Vec<String> = (1..=130).map(|i| format!("v{i}")).collect();
    let data = format!(
        "DATA LIST LIST /{} (F1).\nBEGIN DATA\n{}\nEND DATA.\n",
        names.join(" "),
        vec!["1"; 130].join(" ")
    );
    let mut s = session(&dir, &data);
    let w = warnings(&mut s, "SAVE TRANSLATE OUTFILE='l3.dbf' /TYPE=DB3.");
    assert_eq!(w, ["the file holds 128 variables; the last 2 are left out"]);
    let b = std::fs::read(dir.join("l3.dbf")).unwrap();
    assert_eq!(u16_at(&b, 8), 32 + 32 * 128 + 1);
    let w = warnings(&mut s, "SAVE TRANSLATE OUTFILE='l4.dbf' /TYPE=DB4.");
    assert!(w.is_empty(), "{w:?}");
    let b = std::fs::read(dir.join("l4.dbf")).unwrap();
    assert_eq!(u16_at(&b, 8), 32 + 32 * 130 + 1);
    // the widest number: 19 places for dBASE III, 20 for dBASE IV
    let mut s = session(
        &dir,
        "DATA LIST LIST /w (F25.2).\nBEGIN DATA\n1\nEND DATA.\n",
    );
    run(&mut s, "SAVE TRANSLATE OUTFILE='w3.dbf' /TYPE=DB3.");
    run(&mut s, "SAVE TRANSLATE OUTFILE='w4.dbf' /TYPE=DB4.");
    assert_eq!(field(&std::fs::read(dir.join("w3.dbf")).unwrap(), 0).2, 19);
    assert_eq!(field(&std::fs::read(dir.join("w4.dbf")).unwrap(), 0).2, 20);
    std::fs::remove_dir_all(&dir).unwrap();
}
