//! GET DATA /TYPE=XLSX and SAVE TRANSLATE /TYPE=XLSX against R's writexl and readxl on invented data.
//!
//! Reading: a workbook that writexl wrote (kept in `tests/fixtures/xlsx`: numbers, text with blanks,
//! quotes, commas and accented letters, dates, date and time, true and false, empty cells, two
//! sheets) is read by readxl, and the engine's GET DATA gives the same numbers, text, instants
//! (seconds from 1582-10-14, as the date formats count them), and true and false as 1 and 0, for
//! the first sheet, a sheet by name and by index, and a cell range with and without names.
//! Writing: the engine's SAVE TRANSLATE files (every kind of value, with and without names, value
//! labels as text, a sheet name, KEEP and RENAME) are read by readxl, and what it finds is the
//! table of values the engine reads back: a system-missing value is an empty cell, a user-missing
//! code a value, the dates and times Excel dates. Empty text and an empty cell both read as empty
//! text; readxl calls both NA.
//! What readxl does that the engine does not copy: it guesses a column's type from the cells
//! and trims text (switched off here), and the engine's names follow its own rules.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct WriteCase {
    label: &'static str,
    /// The SAVE TRANSLATE run on `WRITE_DATA`; `{OUT}` stands for the folder of the files.
    command: &'static str,
    /// The GET DATA that reads the file back.
    readback: &'static str,
    columns: &'static [(&'static str, &'static str)],
}

struct ReadCase {
    label: &'static str,
    command: &'static str,
    columns: &'static [(&'static str, &'static str)],
}

const READ_CASES: &[ReadCase] = &[
    ReadCase {
        label: r####"the first sheet with names: numbers, text, dates, date and time, true and false, empty cells"####,
        command: r####"GET DATA /TYPE=XLSX /FILE='{FIX}/wave.xlsx'."####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5 6 7 8"####),
            (
                r####"score"####,
                r####"12.5 7 . 20.25 0.3 -3.75 1234567.891 0"####,
            ),
            (
                r####"name"####,
                r####"Ann Lee|  Bob|Zoë Õie||say "hi"|a,b;c||x y"####,
            ),
            (
                r####"born"####,
                r####"13797216000 13165977600 13928544000 . 10015488000 14129683200 12703478400 13219372800"####,
            ),
            (
                r####"at"####,
                r####"13929013800 13929062399 . 13134528001 13928587200 13180428428 13928976000 13506084610"####,
            ),
            (r####"ok"####, r####"1 0 . 1 1 0 0 1"####),
            (
                r####"big"####,
                r####"12345678901.2345 -9.5e+12 5 1e-07 123456789 0.000123 98765432101234 3"####,
            ),
            (r####"note"####, r####"a||b|c||d|e|f"####),
        ],
    },
    ReadCase {
        label: r####"a sheet by name"####,
        command: r####"GET DATA /TYPE=XLSX /FILE='{FIX}/wave.xlsx' /SHEET=NAME 'Wave 2'."####,
        columns: &[
            (r####"code"####, r####"10 20 30"####),
            (r####"label"####, r####"low|mid|high"####),
        ],
    },
    ReadCase {
        label: r####"a sheet by index"####,
        command: r####"GET DATA /TYPE=XLSX /FILE='{FIX}/wave.xlsx' /SHEET=INDEX 2."####,
        columns: &[
            (r####"code"####, r####"10 20 30"####),
            (r####"label"####, r####"low|mid|high"####),
        ],
    },
    ReadCase {
        label: r####"a cell range without names"####,
        command: r####"GET DATA /TYPE=XLSX /FILE='{FIX}/wave.xlsx' /SHEET=NAME 'Wave 1' /CELLRANGE=RANGE 'B2:D6' /READNAMES=OFF."####,
        columns: &[
            (r####"V1"####, r####"12.5 7 . 20.25 0.3"####),
            (r####"V2"####, r####"Ann Lee|  Bob|Zoë Õie||say "hi""####),
            (
                r####"V3"####,
                r####"13797216000 13165977600 13928544000 . 10015488000"####,
            ),
        ],
    },
    ReadCase {
        label: r####"a cell range whose first row holds the names"####,
        command: r####"GET DATA /TYPE=XLSX /FILE='{FIX}/wave.xlsx' /CELLRANGE=RANGE 'F1:G5'."####,
        columns: &[
            (r####"ok"####, r####"1 0 . 1"####),
            (r####"big"####, r####"12345678901.2345 -9.5e+12 5 1e-07"####),
        ],
    },
];

const WRITE_DATA: &str = r####"DATA LIST LIST /id (F3) score (F8.2) name (A12) born (DATE11) at (DATETIME20) t (TIME8) grp (F1) big (F20.8).
BEGIN DATA
1 12.5 'Ann Lee' '15-JAN-2020' '05-MAR-2024 10:30:00' '06:15:00' 1 12345678901.2345
2 . ' lead' '31-DEC-1999' . '07:00:30' 2 -9.5e+12
3 20.25 'Zoë; Õie' . '01-JAN-1999 00:00:01' . 3 5
4 99 '' '01-MAR-1900' '29-FEB-2024 23:59:59' '00:00:00' 1 1e-07
5 -3.75 'say "hi"' '29-FEB-2024' '15-JUN-2000 06:07:08' '23:59:59' 2 123456789
6 0.1 'a,b' '15-JUL-2030' '10-OCT-2010 10:10:10' '12:00:00' 9 0.000123
END DATA.
MISSING VALUES score (99) grp (9).
VALUE LABELS grp 1 'Yes' 2 'No' 9 'Missing'.
"####;

const WRITE_CASES: &[WriteCase] = &[
    WriteCase {
        label: r####"every kind of value with the names in the first row: system-missing is an empty cell, a user-missing code a value"####,
        command: r####"SAVE TRANSLATE OUTFILE='{OUT}/w1.xlsx' /TYPE=XLSX /FIELDNAMES."####,
        readback: r####"GET DATA /TYPE=XLSX /FILE='{OUT}/w1.xlsx'."####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5 6"####),
            (r####"score"####, r####"12.5 . 20.25 99 -3.75 0.1"####),
            (
                r####"name"####,
                r####"Ann Lee| lead|Zoë; Õie||say "hi"|a,b"####,
            ),
            (
                r####"born"####,
                r####"13798425600 13165977600 . 10015488000 13928544000 14129683200"####,
            ),
            (
                r####"at"####,
                r####"13929013800 . 13134528001 13928630399 13180428428 13506084610"####,
            ),
            (r####"t"####, r####"22500 25230 . 0 86399 43200"####),
            (r####"grp"####, r####"1 2 3 1 2 9"####),
            (
                r####"big"####,
                r####"12345678901.2345 -9.5e+12 5 1e-07 123456789 0.000123"####,
            ),
        ],
    },
    WriteCase {
        label: r####"no names: the first row is the first case"####,
        command: r####"SAVE TRANSLATE OUTFILE='{OUT}/w2.xlsx' /TYPE=XLSX."####,
        readback: r####"GET DATA /TYPE=XLSX /FILE='{OUT}/w2.xlsx' /READNAMES=OFF."####,
        columns: &[
            (r####"V1"####, r####"1 2 3 4 5 6"####),
            (r####"V2"####, r####"12.5 . 20.25 99 -3.75 0.1"####),
            (
                r####"V3"####,
                r####"Ann Lee| lead|Zoë; Õie||say "hi"|a,b"####,
            ),
            (
                r####"V4"####,
                r####"13798425600 13165977600 . 10015488000 13928544000 14129683200"####,
            ),
            (
                r####"V5"####,
                r####"13929013800 . 13134528001 13928630399 13180428428 13506084610"####,
            ),
            (r####"V6"####, r####"22500 25230 . 0 86399 43200"####),
            (r####"V7"####, r####"1 2 3 1 2 9"####),
            (
                r####"V8"####,
                r####"12345678901.2345 -9.5e+12 5 1e-07 123456789 0.000123"####,
            ),
        ],
    },
    WriteCase {
        label: r####"value labels instead of values; a code without a label stays a number as text"####,
        command: r####"SAVE TRANSLATE OUTFILE='{OUT}/w3.xlsx' /TYPE=XLSX /FIELDNAMES /CELLS=LABELS /KEEP=id grp."####,
        readback: r####"GET DATA /TYPE=XLSX /FILE='{OUT}/w3.xlsx'."####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5 6"####),
            (r####"grp"####, r####"Yes|No|3|Yes|No|Missing"####),
        ],
    },
    WriteCase {
        label: r####"a chosen sheet name, KEEP (the file keeps the order of the data, not of KEEP) and RENAME"####,
        command: r####"SAVE TRANSLATE OUTFILE='{OUT}/w4.xlsx' /TYPE=XLSX /FIELDNAMES /EXCELOPTIONS SHEET='Wave 3' /KEEP=born name id /RENAME=(born=day) (name=who)."####,
        readback: r####"GET DATA /TYPE=XLSX /FILE='{OUT}/w4.xlsx' /SHEET=NAME 'Wave 3'."####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5 6"####),
            (
                r####"who"####,
                r####"Ann Lee| lead|Zoë; Õie||say "hi"|a,b"####,
            ),
            (
                r####"day"####,
                r####"13798425600 13165977600 . 10015488000 13928544000 14129683200"####,
            ),
        ],
    },
];

fn show_text(engine: &Engine, name: &str) -> Option<String> {
    let data = engine.dataset()?;
    Some(match data.variable(name).ok()?.values() {
        Values::Numeric(values) => {
            let text = values
                .iter()
                .map(|v| v.map_or(".".into(), |v| v.to_string()));
            text.collect::<Vec<_>>().join(" ")
        }
        Values::String(values) => values
            .iter()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("|"),
    })
}

/// The same values: numbers to 12 significant digits (R shows 15), anything else exactly.
fn same(got: &str, want: &str) -> bool {
    let (got_numbers, want_numbers): (Vec<&str>, Vec<&str>) =
        (got.split(' ').collect(), want.split(' ').collect());
    let number = |t: &&str| *t == "." || t.parse::<f64>().is_ok();
    if got_numbers.len() == want_numbers.len() && got_numbers.iter().all(number) {
        return got_numbers.iter().zip(&want_numbers).all(|(g, w)| {
            match (g.parse::<f64>(), w.parse::<f64>()) {
                (Ok(g), Ok(w)) => (g - w).abs() <= 1e-12 * w.abs().max(1.0),
                _ => g == w,
            }
        });
    }
    got == want
}

fn fixtures() -> String {
    format!("{}/tests/fixtures/xlsx", env!("CARGO_MANIFEST_DIR"))
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
fn reading_workbooks_agrees_with_readxl() {
    let mut failures = Vec::new();
    for case in READ_CASES {
        let mut engine = Engine::default();
        let source = case.command.replace("{FIX}", &fixtures());
        if let Err(error) = execute_all(&mut engine, &source) {
            failures.push(format!("{}: error {error}", case.label));
            continue;
        }
        for (name, want) in case.columns {
            let got = show_text(&engine, name).unwrap_or_else(|| "(no such variable)".into());
            if !same(&got, want) {
                failures.push(format!(
                    "{}: {name} is [{got}], readxl has [{want}]",
                    case.label
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Set `XLSX_DIR` to a folder to keep the files there, for R to read with readxl.
#[test]
fn written_workbooks_hold_what_readxl_finds_in_them() {
    let kept = std::env::var_os("XLSX_DIR").map(std::path::PathBuf::from);
    let dir = kept.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("statscraft-xlsx-{}", std::process::id()))
    });
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.display().to_string();
    let mut failures = Vec::new();
    for case in WRITE_CASES {
        let mut engine = Engine::default();
        let source = format!("{WRITE_DATA}{}", case.command.replace("{OUT}", &out));
        if let Err(error) = execute_all(&mut engine, &source) {
            failures.push(format!("{}: error {error}", case.label));
            continue;
        }
        let mut again = Engine::default();
        if let Err(error) = execute_all(&mut again, &case.readback.replace("{OUT}", &out)) {
            failures.push(format!("{}: read back: {error}", case.label));
            continue;
        }
        // the columns come in the order readxl found them
        let order: Vec<String> = again
            .dataset()
            .map(|d| d.variable_names().map(str::to_string).collect())
            .unwrap_or_default();
        let wanted: Vec<&str> = case.columns.iter().map(|(n, _)| *n).collect();
        if order != wanted {
            failures.push(format!(
                "{}: the columns are {order:?}, readxl has {wanted:?}",
                case.label
            ));
        }
        for (name, want) in case.columns {
            let got = show_text(&again, name).unwrap_or_else(|| "(no such variable)".into());
            if !same(&got, want) {
                failures.push(format!(
                    "{}: {name} is [{got}], readxl has [{want}]",
                    case.label
                ));
            }
        }
    }
    if kept.is_none() {
        std::fs::remove_dir_all(&dir).ok();
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
