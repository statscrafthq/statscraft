//! Text files, .sav files and workbooks with letters outside ASCII, against R (iconv, read.csv2,
//! read.fwf, haven, writexl, readxl) on six invented rows of Estonian names and towns.
//!
//! Reading, from files R wrote (kept in `tests/fixtures/multibyte`): GET DATA and DATA LIST of a
//! delimited file in UTF-8, UTF-8 with a byte order mark, windows-1257, UTF-16 in both byte orders,
//! and (with no encoding given) a windows-1257 file that is not UTF-8, which is read as
//! windows-1252, as R reads those bytes by that code page; fixed columns of a code page file and a
//! UTF-16 file count letters and those of a UTF-8 file count bytes (the guide's rule); a .sav file
//! that haven wrote with strings of 2, 6, 400 bytes (the width is the longest value in bytes) and a
//! variable label; a workbook with a sheet named with an accented letter.
//! Writing: SAVE TRANSLATE to CSV in UTF-8 (with and without a mark), windows-1257, windows-1252
//! and UTF-16 in both byte orders gives the bytes R's iconv gives for the same text; a letter the
//! code page lacks is an error. The .sav, .por and .xlsx files the engine writes are kept when
//! `MB_DIR` names a folder, for `lab/oracles/multibyte-r/check_files.R` to read with haven and
//! readxl (a portable file has no code for these letters: they are `?`, with a warning).

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

const READ_ID: &str = r####"1 2 3 4 5 6"####;
const READ_NAME: &str = r####"Õie|Šarlota|Žanna|Jüri|€uro|Ääre"####;
const READ_CITY: &str = r####"Tähtvere|Pärnu|Ülenurme|Võru|Põlva|Kärdla"####;
const READ_NAME_1252: &str = r####"Õie|Ðarlota|Þanna|Jüri|€uro|Ääre"####;
const READ_CITY_1252: &str = r####"Tähtvere|Pärnu|Ülenurme|Võru|Põlva|Kärdla"####;
const FW_NAME: &str = r####"Õie|Jüri|Žanna|Ääre|naine|Ülle"####;
const FW_CITY: &str = r####"Tartu|Pärnu|Ülenurme|Kärdla|Võru|Põlva"####;

struct ReadCase {
    label: &'static str,
    command: &'static str,
    columns: &'static [(&'static str, &'static str)],
}

const ALL: &[(&str, &str)] = &[("id", READ_ID), ("name", READ_NAME), ("city", READ_CITY)];
const AS_1252: &[(&str, &str)] = &[
    ("id", READ_ID),
    ("name", READ_NAME_1252),
    ("city", READ_CITY_1252),
];
const FIXED: &[(&str, &str)] = &[("id", READ_ID), ("name", FW_NAME), ("city", FW_CITY)];

const READ_CASES: &[ReadCase] = &[
    ReadCase {
        label: "GET DATA of a windows-1257 file with its encoding",
        command: "GET DATA /TYPE=TXT /FILE='{FIX}/ee_1257.csv' /ENCODING='windows-1257' /DELIMITERS=';' /FIRSTCASE=2 /VARIABLES=id F2 name A24 city A24.",
        columns: ALL,
    },
    ReadCase {
        label: "GET DATA of a windows-1257 file with no encoding is read as windows-1252",
        command: "GET DATA /TYPE=TXT /FILE='{FIX}/ee_1257.csv' /DELIMITERS=';' /FIRSTCASE=2 /VARIABLES=id F2 name A24 city A24.",
        columns: AS_1252,
    },
    ReadCase {
        label: "GET DATA of a UTF-8 file",
        command: "GET DATA /TYPE=TXT /FILE='{FIX}/ee_utf8.csv' /DELIMITERS=';' /FIRSTCASE=2 /VARIABLES=id F2 name A24 city A24.",
        columns: ALL,
    },
    ReadCase {
        label: "GET DATA of a UTF-8 file with a byte order mark",
        command: "GET DATA /TYPE=TXT /FILE='{FIX}/ee_bom.csv' /DELIMITERS=';' /FIRSTCASE=2 /VARIABLES=id F2 name A24 city A24.",
        columns: ALL,
    },
    ReadCase {
        label: "GET DATA of a UTF-16 file, little endian",
        command: "GET DATA /TYPE=TXT /FILE='{FIX}/ee_utf16le.csv' /ENCODING='UTF16LE' /DELIMITERS=';' /FIRSTCASE=2 /VARIABLES=id F2 name A24 city A24.",
        columns: ALL,
    },
    ReadCase {
        label: "GET DATA of a UTF-16 file, big endian",
        command: "GET DATA /TYPE=TXT /FILE='{FIX}/ee_utf16be.csv' /ENCODING='UTF16BE' /DELIMITERS=';' /FIRSTCASE=2 /VARIABLES=id F2 name A24 city A24.",
        columns: ALL,
    },
    ReadCase {
        label: "DATA LIST of a windows-1257 file with its encoding",
        command: "DATA LIST FILE='{FIX}/ee_1257.csv' ENCODING='1257' LIST (';') SKIP=1 /id (F2) name (A24) city (A24).",
        columns: ALL,
    },
    ReadCase {
        label: "DATA LIST of a UTF-16 file",
        command: "DATA LIST FILE='{FIX}/ee_utf16le.csv' ENCODING='UTF16LE' LIST (';') SKIP=1 /id (F2) name (A24) city (A24).",
        columns: ALL,
    },
    ReadCase {
        label: "DATA LIST through a file handle that has the encoding",
        command: "FILE HANDLE eh /NAME='{FIX}/ee_1257.csv' /ENCODING='windows-1257'.\nDATA LIST FILE=eh LIST (';') SKIP=1 /id (F2) name (A24) city (A24).",
        columns: ALL,
    },
    ReadCase {
        label: "DATA LIST in a UTF-8 file: columns are bytes",
        command: "DATA LIST FILE='{FIX}/fw_utf8.txt' FIXED /id 1 (F1) name 2-7 (A6) city 8-17 (A10).",
        columns: FIXED,
    },
    ReadCase {
        label: "DATA LIST in a windows-1257 file: columns are letters",
        command: "DATA LIST FILE='{FIX}/fw_1257.txt' ENCODING='windows-1257' FIXED /id 1 (F1) name 2-7 (A6) city 8-17 (A10).",
        columns: FIXED,
    },
    ReadCase {
        label: "DATA LIST in a UTF-16 file: columns are letters",
        command: "DATA LIST FILE='{FIX}/fw_utf16le.txt' ENCODING='UTF16LE' FIXED /id 1 (F1) name 2-7 (A6) city 8-17 (A10).",
        columns: FIXED,
    },
    ReadCase {
        label: "GET DATA fixed columns of a windows-1257 file are letters",
        command: "GET DATA /TYPE=TXT /FILE='{FIX}/fw_1257.txt' /ENCODING='windows-1257' /ARRANGEMENT=FIXED /FIRSTCASE=1 /VARIABLES=/1 id 0-0 F1 name 1-6 A6 city 7-16 A10.",
        columns: FIXED,
    },
    ReadCase {
        label: "a sheet named with an accented letter, by name",
        command: "GET DATA /TYPE=XLSX /FILE='{FIX}/ee_sheets.xlsx' /SHEET=NAME 'Õpilased'.",
        columns: ALL,
    },
    ReadCase {
        label: "the same sheet by index",
        command: "GET DATA /TYPE=XLSX /FILE='{FIX}/ee_sheets.xlsx' /SHEET=INDEX 1.",
        columns: ALL,
    },
];

fn fixtures() -> String {
    format!("{}/tests/fixtures/multibyte", env!("CARGO_MANIFEST_DIR"))
}

fn show(engine: &Engine, name: &str) -> Option<String> {
    let data = engine.dataset()?;
    Some(match data.variable(name).ok()?.values() {
        Values::Numeric(values) => values
            .iter()
            .map(|v| v.map_or(".".into(), |v| v.to_string()))
            .collect::<Vec<_>>()
            .join(" "),
        Values::String(values) => values
            .iter()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("|"),
    })
}

fn execute_all(engine: &mut Engine, source: &str) -> Result<(), String> {
    let (commands, errors) = parse(source);
    if !errors.is_empty() {
        return Err(format!("{errors:?}"));
    }
    for command in &commands {
        engine
            .execute(&command.command)
            .map_err(|e| format!("line {}: {e}", command.first_line))?;
    }
    Ok(())
}

#[test]
fn text_files_agree_with_r() {
    let mut failures = Vec::new();
    for case in READ_CASES {
        let mut engine = Engine::default();
        let source = case.command.replace("{FIX}", &fixtures());
        if let Err(error) = execute_all(&mut engine, &source) {
            failures.push(format!("{}: error {error}", case.label));
            continue;
        }
        for (name, want) in case.columns {
            let got = show(&engine, name).unwrap_or_else(|| "(no such variable)".into());
            if got != *want {
                failures.push(format!("{}: {name} is [{got}], R has [{want}]", case.label));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_sav_file_from_haven_keeps_its_text_and_widths_in_bytes() {
    let mut engine = Engine::default();
    let source = format!(
        "GET FILE='{}/haven_text.sav'.\nCOMPUTE bytes = LENGTH(RTRIM(s300)).\nCOMPUTE letters = CHAR.LENGTH(s300).",
        fixtures()
    );
    execute_all(&mut engine, &source).unwrap();
    let long = "õ".repeat(200);
    let last = format!("ž{}", "a".repeat(298));
    let facts: [(&str, String); 5] = [
        ("s1", "õ|ä|ö|ü".into()),
        ("s5", "Õie|Jüri|Žanna|kuu".into()),
        // haven's bytes of the text in R: 400 5 0 300
        ("bytes", "400 5 0 300".into()),
        ("letters", "200 5 0 299".into()),
        ("s300", format!("{long}|short||{last}")),
    ];
    for (name, want) in facts {
        assert_eq!(show(&engine, name).unwrap(), want, "{name}");
    }
    let data = engine.dataset().unwrap();
    // haven reads the formats as A2, A6 and A400, the longest value in bytes
    for (name, format) in [("s1", "A2"), ("s5", "A6"), ("s300", "A400")] {
        assert_eq!(data.variable(name).unwrap().format().to_string(), format);
    }
    assert_eq!(
        data.variable("s5").unwrap().label().unwrap_or(""),
        "Nimi õäöü"
    );
}

const TABLE: &str = "DATA LIST LIST /id (F2) name (A24) city (A24).
BEGIN DATA
1 'Õie' 'Tähtvere'
2 'Šarlota' 'Pärnu'
3 'Žanna' 'Ülenurme'
4 'Jüri' 'Võru'
5 '€uro' 'Põlva'
6 'Ääre' 'Kärdla'
END DATA.
";

fn written(dir: &std::path::Path, command: &str) -> Vec<u8> {
    let mut engine = Engine::default();
    let source = format!(
        "{TABLE}{}",
        command.replace("{OUT}", &dir.display().to_string())
    );
    execute_all(&mut engine, &source).unwrap_or_else(|e| panic!("{command}: {e}"));
    let file = command.split('\'').nth(1).unwrap().replace("{OUT}", "");
    std::fs::read(dir.join(file.trim_start_matches('/'))).unwrap()
}

#[test]
fn csv_files_have_the_bytes_r_gives() {
    let dir = std::env::temp_dir().join(format!("statscraft-mb-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let fix = std::path::PathBuf::from(fixtures());
    let head = "SAVE TRANSLATE OUTFILE='{OUT}/";
    let tail = "/TYPE=CSV /FIELDNAMES /TEXTOPTIONS DELIMITER=';'";
    let cases = [
        (
            "windows-1257",
            "o1.csv' ",
            " /ENCODING='windows-1257'",
            "ee_1257.csv",
        ),
        (
            "windows-1252",
            "o2.csv' ",
            " /ENCODING='windows-1252'",
            "ee_1252.csv",
        ),
        (
            "UTF-8 without a mark",
            "o3.csv' ",
            " /ENCODING='UTF8' BOM=NO",
            "ee_utf8.csv",
        ),
        (
            "UTF-8 with a mark",
            "o4.csv' ",
            " /ENCODING='UTF8' BOM=YES",
            "ee_bom.csv",
        ),
        (
            "UTF-16 little endian",
            "o5.csv' ",
            " /ENCODING='UTF16LE'",
            "ee_utf16le.csv",
        ),
        (
            "UTF-16 big endian",
            "o6.csv' ",
            " /ENCODING='UTF16BE'",
            "ee_utf16be.csv",
        ),
    ];
    let mut failures = Vec::new();
    for (label, file, encoding, reference) in cases {
        let command = format!("{head}{file}{tail}{encoding}.");
        let got = written(&dir, &command);
        let want = std::fs::read(fix.join(reference)).unwrap();
        if got != want {
            failures.push(format!("{label}: {got:02x?} is not R's {want:02x?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // a letter the code page lacks is an error, nothing is written
    let mut engine = Engine::default();
    let source = format!(
        "DATA LIST LIST /t (A12).\nBEGIN DATA\n'日本'\nEND DATA.\nSAVE TRANSLATE OUTFILE='{}/jp.csv' /TYPE=CSV /ENCODING='windows-1257'.",
        dir.display()
    );
    let error = execute_all(&mut engine, &source).unwrap_err();
    assert!(error.contains("windows-1257 cannot hold"), "{error}");
    assert!(!dir.join("jp.csv").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Set `MB_DIR` to a folder to keep the files there, for R to read with haven and readxl.
#[test]
fn sav_por_and_xlsx_files_are_written_for_r() {
    let kept = std::env::var_os("MB_DIR").map(std::path::PathBuf::from);
    let dir = kept.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("statscraft-mb2-{}", std::process::id()))
    });
    std::fs::create_dir_all(&dir).unwrap();
    let mut engine = Engine::default();
    let source = format!(
        "{TABLE}STRING s1 (A2).\nSTRING s300 (A400).\nCOMPUTE s1 = CHAR.SUBSTR(name, 1, 1).\nCOMPUTE s300 = CHAR.LPAD('õ', 200, 'õ').\nIF (id = 4) s300 = CONCAT('ž', CHAR.LPAD('a', 298, 'a')).\nVARIABLE LABELS name 'Nimi õäöü'.\nSAVE OUTFILE='{d}/text.sav'.\nSAVE CODEPAGE OUTFILE='{d}/text1257.sav' /ENCODING='windows-1257' /KEEP=id name city s1.\nSAVE TRANSLATE OUTFILE='{d}/sheets.xlsx' /TYPE=XLSX /FIELDNAMES /EXCELOPTIONS SHEET='Õpilased' /KEEP=id name city.",
        d = dir.display()
    );
    execute_all(&mut engine, &source).unwrap();
    // the portable file: ? with a warning, and what is left is read back the same
    let export = format!(
        "EXPORT OUTFILE='{}/text.por' /KEEP=id name city.",
        dir.display()
    );
    let (commands, _) = parse(&export);
    engine.execute(&commands[0].command).unwrap();
    assert!(engine.warnings().iter().any(|w| w.contains("no code for")));
    for file in ["text.sav", "text1257.sav", "sheets.xlsx", "text.por"] {
        assert!(dir.join(file).exists(), "{file}");
    }
    if kept.is_none() {
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn file_type_reads_a_code_page_file_with_its_encoding() {
    let dir = std::env::temp_dir().join(format!("statscraft-mb3-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // "A1Tähtvere" and "B2Põlva" in windows-1257: ä is e4 and õ is f5, as in R's iconv
    std::fs::write(dir.join("mixed.txt"), b"A1T\xe4htvere\nB2P\xf5lva\n").unwrap();
    let mut engine = Engine::default();
    let source = format!(
        "FILE TYPE MIXED FILE='{d}/mixed.txt' ENCODING='1257' RECORD=1 (A1).\nRECORD TYPE 'A'.\nDATA LIST /n 2 (F1) town 3-12 (A10).\nRECORD TYPE 'B'.\nDATA LIST /n 2 (F1) town 3-12 (A10).\nEND FILE TYPE.",
        d = dir.display()
    );
    execute_all(&mut engine, &source).unwrap();
    assert_eq!(show(&engine, "n").unwrap(), "1 2");
    assert_eq!(show(&engine, "town").unwrap(), "Tähtvere|Põlva");
    let _ = std::fs::remove_dir_all(&dir);
}
