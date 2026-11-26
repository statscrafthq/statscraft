//! SAVE TRANSLATE /TYPE=SAS /VERSION=X: the bytes of transport files made up here, and what
//! R's foreign::read.xport makes of them.

use statscraft_engine::session::Session;
use std::path::{Path, PathBuf};

const DATA: &str =
    "DATA LIST LIST /id (F3) name (A6) amount (F8.2) born (ADATE10) clock (F8) stamp (F8).
BEGIN DATA
1 'Ann' 118.625 01/02/1960 3661 86400
2 'Bobby' . 12/31/1999 . .
3 'Zo\u{eb}' -0.5 01/01/1960 0 0
END DATA.
VARIABLE LABELS amount 'Amount paid'.
MISSING VALUES id (3).
FORMATS born (DATE11) clock (TIME8) stamp (DATETIME20).
";

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-xpt-{tag}-{}", std::process::id()));
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

fn be16(b: &[u8], i: usize) -> i16 {
    i16::from_be_bytes([b[i], b[i + 1]])
}

#[test]
fn a_transport_file_has_the_headers_descriptors_and_hexadecimal_numbers() {
    let dir = temp_dir("layout");
    let mut s = session(&dir, DATA);
    run(
        &mut s,
        "SAVE TRANSLATE OUTFILE='wave2.xpt' /TYPE=SAS /VERSION=X.",
    );
    let b = std::fs::read(dir.join("wave2.xpt")).unwrap();
    assert_eq!(b.len() % 80, 0);
    let text = |i: usize, n: usize| String::from_utf8_lossy(&b[i..i + n]).to_string();
    assert_eq!(
        text(0, 80),
        format!(
            "HEADER RECORD*******LIBRARY HEADER RECORD!!!!!!!{:<32}",
            "000000000000000000000000000000"
        )
    );
    assert_eq!(text(80, 32), "SAS     SAS     SASLIB  6.06    ");
    assert_eq!(
        text(240, 80),
        format!(
            "HEADER RECORD*******MEMBER  HEADER RECORD!!!!!!!{:<32}",
            "000000000000000001600000000140"
        )
    );
    assert_eq!(
        text(320, 80),
        format!(
            "HEADER RECORD*******DSCRPTR HEADER RECORD!!!!!!!{:<32}",
            "000000000000000000000000000000"
        )
    );
    assert_eq!(text(400, 16), "SAS     WAVE2   ");
    assert_eq!(
        text(560, 80),
        format!(
            "HEADER RECORD*******NAMESTR HEADER RECORD!!!!!!!{:<32}",
            "000000000600000000000000000000"
        )
    );
    // six descriptors of 140 bytes follow the NAMESTR header (record 8), padded to whole records
    let d = |n: usize| &b[640 + 140 * n..640 + 140 * (n + 1)];
    let fields = [
        // type, length, name, label, format, format width, position
        (1, 8, "id", "id", "", 12, 0),
        (2, 6, "name", "name", "", 0, 8),
        (1, 8, "amount", "Amount paid", "", 12, 14),
        (1, 8, "born", "born", "DATE", 9, 22),
        (1, 8, "clock", "clock", "TIME", 8, 30),
        (1, 8, "stamp", "stamp", "DATETIME", 16, 38),
    ];
    for (n, (kind, length, name, label, format, width, at)) in fields.iter().enumerate() {
        let f = d(n);
        let s = |a: usize, z: usize| String::from_utf8_lossy(&f[a..z]).trim_end().to_string();
        assert_eq!(be16(f, 0), *kind, "{name}");
        assert_eq!(be16(f, 4), *length, "{name}");
        assert_eq!(be16(f, 6), n as i16 + 1, "{name}");
        assert_eq!(s(8, 16), *name);
        assert_eq!(s(16, 56), *label);
        assert_eq!(s(56, 64), *format);
        assert_eq!(be16(f, 64), *width, "{name}");
        assert_eq!(
            i32::from_be_bytes([f[84], f[85], f[86], f[87]]),
            *at,
            "{name}"
        );
    }
    let start = (640 + 140 * 6usize).next_multiple_of(80);
    assert_eq!(
        text(start, 48),
        "HEADER RECORD*******OBS     HEADER RECORD!!!!!!!"
    );
    // three observations of 46 bytes each, numbers in hexadecimal floating point: 118.625 is 0x42 76 A0,
    // 01/02/1960 is SAS day 1, and the datetimes are seconds since 1960-01-01, so 1582-10-15 is
    // far before it
    let obs = &b[start + 80..];
    let num = |o: usize, hex: [u8; 8]| assert_eq!(&obs[o..o + 8], &hex, "at {o}");
    let missing = [0x2E, 0, 0, 0, 0, 0, 0, 0];
    num(0, [0x41, 0x10, 0, 0, 0, 0, 0, 0]);
    assert_eq!(&obs[8..14], b"Ann   ");
    num(14, [0x42, 0x76, 0xA0, 0, 0, 0, 0, 0]);
    num(22, [0x41, 0x10, 0, 0, 0, 0, 0, 0]);
    num(30, [0x43, 0xE4, 0xD0, 0, 0, 0, 0, 0]);
    num(38, [0xC9, 0x2C, 0x58, 0x3A, 0x50, 0, 0, 0]);
    // 2: a missing number is `.`; 1999-12-31 is SAS day 14609
    num(46, [0x41, 0x20, 0, 0, 0, 0, 0, 0]);
    assert_eq!(&obs[54..60], b"Bobby ");
    num(60, missing);
    num(68, [0x44, 0x39, 0x11, 0, 0, 0, 0, 0]);
    num(76, missing);
    num(84, missing);
    // 3: a user-missing number is `.` too; the text is Windows Latin-1
    num(92, missing);
    assert_eq!(&obs[100..106], b"Zo\xeb   ");
    num(106, [0xC0, 0x80, 0, 0, 0, 0, 0, 0]);
    num(114, [0; 8]);
    num(122, [0; 8]);
    num(130, [0xC9, 0x2C, 0x58, 0x4F, 0x68, 0, 0, 0]);
    // padded with blanks to a whole record
    assert!(obs[138..].iter().all(|&x| x == b' '));
    std::fs::remove_dir_all(&dir).unwrap();
}

fn descriptor_names(b: &[u8], count: usize) -> Vec<String> {
    (0..count)
        .map(|n| {
            let f = &b[640 + 140 * n..640 + 140 * (n + 1)];
            String::from_utf8_lossy(&f[8..16]).trim_end().to_string()
        })
        .collect()
}

#[test]
fn names_have_eight_bytes_and_the_data_set_is_named_after_the_file() {
    let dir = temp_dir("names");
    let data = "DATA LIST LIST /first.name very_long_name_a Very_Long_Name_B @code (F1).\nBEGIN DATA\n1 2 3 4\nEND DATA.\n";
    let mut s = session(&dir, data);
    run(
        &mut s,
        "SAVE TRANSLATE OUTFILE='2nd wave.xpt' /TYPE=SAS /VERSION=X.",
    );
    let b = std::fs::read(dir.join("2nd wave.xpt")).unwrap();
    assert_eq!(
        descriptor_names(&b, 4),
        ["first_na", "very_lon", "Very_Lo2", "_code"]
    );
    // a file name that starts with a digit gets a V
    assert_eq!(String::from_utf8_lossy(&b[408..416]), "V2ND_WAV");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_lost_character_a_long_text_and_the_refused_forms() {
    let dir = temp_dir("rules");
    let long = "x".repeat(250);
    let data = format!("DATA LIST LIST /s (A250).\nBEGIN DATA\n'\u{416}{long}'\nEND DATA.\n");
    let mut s = session(&dir, &data);
    let result = s
        .run("SAVE TRANSLATE OUTFILE='t.xpt' /TYPE=SAS /VERSION=X.", None)
        .unwrap();
    let warnings: Vec<String> = result
        .commands
        .iter()
        .flat_map(|c| c.warnings.clone())
        .collect();
    assert_eq!(
        warnings,
        [
            "s: text is cut to 200 bytes",
            "s: characters outside Windows Latin-1 are written as ?"
        ]
    );
    let b = std::fs::read(dir.join("t.xpt")).unwrap();
    let obs = &b[880..];
    assert_eq!(&obs[..4], b"?xxx");
    assert!(obs[..200].iter().skip(1).all(|&x| x == b'x'));
    for (syntax, wanted) in [
        (
            "SAVE TRANSLATE OUTFILE='x.xpt' /TYPE=SAS /VERSION=9.",
            "use /TYPE=SAS /VERSION=X",
        ),
        (
            "SAVE TRANSLATE OUTFILE='x.xpt' /TYPE=SAS.",
            "use /TYPE=SAS /VERSION=X",
        ),
        (
            "SAVE TRANSLATE OUTFILE='x.xpt' /TYPE=SAS /VERSION=X /ENCODING='UTF8'.",
            "a SAS transport file holds Windows Latin-1 text",
        ),
    ] {
        let e = s.run(syntax, None).unwrap_err();
        assert!(e[0].message.contains(wanted), "{syntax}: {e:?}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}
