//! The .sav reader and writer against R's haven, on invented data with labels, user-missing
//! codes, string widths and dates.
//!
//! Reading: three files written by haven (case by case compression, zlib blocks and none) from
//! one data frame: variable labels, value labels for numbers and for strings, discrete missing
//! codes, a missing range with one more code, system-missing values, F, COMMA, PCT, DATE,
//! DATETIME and TIME formats and their values (seconds since 1582-10-14), strings of width 1,
//! 8, 30 and 300 with empty values, and text with letters outside ASCII. Each file must read the
//! way haven reads it.
//! Writing: the same data are built in the engine from syntax and saved three ways. haven
//! reads each of the three files the way it reads its own, which is the same table of facts, and
//! the files are kept in `tests/fixtures/haven` (`statscraft_made*`): a fresh write must give the same
//! bytes, apart from the product name and the creation date and time in the header (not for the
//! ZSAV file, whose zlib blocks depend on the compressor; it is held to the facts). The table of
//! facts is what haven 2.5.5 gave for each of the six files. Set `UPDATE_FIXTURES=1` to write the
//! `statscraft_made*` files again, and have haven read the new files before keeping them.

use statscraft_data::{MissingValues, Value, Values};
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Var {
    name: &'static str,
    label: &'static str,
    format: &'static str,
    /// `value=label` joined by `;`, in the order of the values.
    labels: &'static str,
    /// `n:` discrete numbers, `r:` low, high and an optional extra code, `s:` strings.
    missing: &'static str,
    /// Numbers separated by spaces with `.` for system-missing, strings separated by `|`.
    values: &'static str,
}

const FACTS: &[Var] = &[
    Var {
        name: r#"id"#,
        label: r#"Row id"#,
        format: r#"F8.0"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"1 2 3 4 5 6 7 8 9 10 11 12"#,
    },
    Var {
        name: r#"score"#,
        label: r#"Score from 0 to 10"#,
        format: r#"F5.1"#,
        labels: r#"0=Low;5=Mid;10=High"#,
        missing: r#"n:98,99"#,
        values: r#"3 7.5 0 98 . 10 5 99 2.5 5 . 10"#,
    },
    Var {
        name: r#"band"#,
        label: r#"Band, with a range of missing codes"#,
        format: r#"F2.0"#,
        labels: r#"1=One;2=Two;3=Three;7=Not asked"#,
        missing: r#"r:90,99,7"#,
        values: r#"1 2 95 7 3 99 2 1 7 90 3 2"#,
    },
    Var {
        name: r#"amt"#,
        label: r#"Amount"#,
        format: r#"COMMA10.2"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"1234.5 0.25 -17.125 . 99999.99 5 10.5 0 1000000 3.14159 2 8"#,
    },
    Var {
        name: r#"share"#,
        label: r#"Share"#,
        format: r#"PCT8.1"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"0.1 12.5 100 . 33.3 0 7 8 9 10 11 12"#,
    },
    Var {
        name: r#"when"#,
        label: r#"Visit date"#,
        format: r#"DATE11"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"13924656000 13927334400 13929753600 . 13956278400 13959216000 13925088000 13925174400 13925260800 13925347200 13925433600 13925520000"#,
    },
    Var {
        name: r#"stamp"#,
        label: r#"Stamp"#,
        format: r#"DATETIME20"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"13928661015 13928664615 13928747415 . 13928751076 13928661020 13928661021 13928661022 13928661023 13928661024 13928661025 13928661026"#,
    },
    Var {
        name: r#"tm"#,
        label: r#"Time of day"#,
        format: r#"TIME8"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"0 59 3600 . 45296 86399 1 2 3 4 5 6"#,
    },
    Var {
        name: r#"s1"#,
        label: r#"One char"#,
        format: r#"A1"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"a|b||a|b|a||b|a|b|a|b"#,
    },
    Var {
        name: r#"s8"#,
        label: r#"Short text"#,
        format: r#"A8"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"kiwi|plum||fig|pear|date|kiwi|lime|01234567|x|y|z"#,
    },
    Var {
        name: r#"s30"#,
        label: r#"Text of 30"#,
        format: r#"A30"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"a longer sentence here||short|x|y||||closing remark|z|w|v"#,
    },
    Var {
        name: r#"s300"#,
        label: r#"Text of 300"#,
        format: r#"A300"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghij||tiny|abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcde|abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdef||x||y||z|"#,
    },
    Var {
        name: r#"lab"#,
        label: r#"Coded text"#,
        format: r#"A1"#,
        labels: r#"a=Alpha;b=Bravo"#,
        missing: r#""#,
        values: r#"a|b|a|b|a|b|a|b|a|b|a|b"#,
    },
    Var {
        name: r#"uni"#,
        label: r#"Näide õäöü, label"#,
        format: r#"F3.0"#,
        labels: r#"1=Üüks;2=Kaks õ"#,
        missing: r#""#,
        values: r#"1 2 3 4 5 6 7 8 9 10 11 12"#,
    },
    Var {
        name: r#"a_name_of_33_characters_xxxxxxxxx"#,
        label: r#"A long variable label that goes on, A long variable label that goes on, A long variable label that goes on, A long variable label that goes on, A long variable label that goes on, A long variable label that goes on, A long variable label that goes on"#,
        format: r#"F6.2"#,
        labels: r#"-1.5=Minus one and a half;0=Zero;2=Two, spelled out in a long value label that is well past the sixty byte limit of the old files"#,
        missing: r#""#,
        values: r#"-1.5 0 2 2 -1.5 0 7 7 2 0 -1.5 7"#,
    },
    Var {
        name: r#"txtna"#,
        label: r#"Text with missing codes"#,
        format: r#"A8"#,
        labels: r#"fine=Fine;n/a=Not available"#,
        missing: r#"s:n/a,-"#,
        values: r#"ok|n/a|ok|-|fine|n/a|ok||-|fine|ok|ok"#,
    },
];

const WRITTEN: &str = r####"DATA LIST LIST /id (F8.0) score (F5.1) band (F2.0) amt (COMMA10.2) share (PCT8.1) when (DATE11) stamp (DATETIME20) tm (TIME8) s1 (A1) s8 (A8) s30 (A30) s300 (A300) lab (A1) uni (F3.0) a_name_of_33_characters_xxxxxxxxx (F6.2) txtna (A8).
BEGIN DATA
1 3 1 1234.5 0.1 '15-JAN-2024' '01-MAR-2024 08:30:15' '00:00:00' 'a' 'kiwi' 'a longer sentence here' 'abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghij' 'a' 1 -1.5 'ok'
2 7.5 2 0.25 12.5 '15-FEB-2024' '01-MAR-2024 09:30:15' '00:00:59' 'b' 'plum' '' '' 'b' 2 0 'n/a'
3 0 95 -17.125 100 '14-MAR-2024' '02-MAR-2024 08:30:15' '01:00:00' '' '' 'short' 'tiny' 'a' 3 2 'ok'
4 98 7 . . . . . 'a' 'fig' 'x' 'abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcde' 'b' 4 2 '-'
5 . 3 99999.99 33.3 '15-JAN-2025' '02-MAR-2024 09:31:16' '12:34:56' 'b' 'pear' 'y' 'abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdef' 'a' 5 -1.5 'fine'
6 10 99 5 0 '18-FEB-2025' '01-MAR-2024 08:30:20' '23:59:59' 'a' 'date' '' '' 'b' 6 0 'n/a'
7 5 2 10.5 7 '20-JAN-2024' '01-MAR-2024 08:30:21' '00:00:01' '' 'kiwi' '' 'x' 'a' 7 7 'ok'
8 99 1 0 8 '21-JAN-2024' '01-MAR-2024 08:30:22' '00:00:02' 'b' 'lime' '' '' 'b' 8 7 ''
9 2.5 7 1000000 9 '22-JAN-2024' '01-MAR-2024 08:30:23' '00:00:03' 'a' '01234567' 'closing remark' 'y' 'a' 9 2 '-'
10 5 90 3.14159 10 '23-JAN-2024' '01-MAR-2024 08:30:24' '00:00:04' 'b' 'x' 'z' '' 'b' 10 0 'fine'
11 . 3 2 11 '24-JAN-2024' '01-MAR-2024 08:30:25' '00:00:05' 'a' 'y' 'w' 'z' 'a' 11 -1.5 'ok'
12 10 2 8 12 '25-JAN-2024' '01-MAR-2024 08:30:26' '00:00:06' 'b' 'z' 'v' '' 'b' 12 7 'ok'
END DATA.
FORMATS id (F8.0) score (F5.1) band (F2.0) amt (COMMA10.2) share (PCT8.1) uni (F3.0) a_name_of_33_characters_xxxxxxxxx (F6.2).
VARIABLE LABELS id 'Row id'
 /score 'Score from 0 to 10'
 /band 'Band, with a range of missing codes'
 /amt 'Amount'
 /share 'Share'
 /when 'Visit date'
 /stamp 'Stamp'
 /tm 'Time of day'
 /s1 'One char'
 /s8 'Short text'
 /s30 'Text of 30'
 /s300 'Text of 300'
 /lab 'Coded text'
 /uni 'Näide õäöü, label'
 /a_name_of_33_characters_xxxxxxxxx 'A long variable label that goes on, A long variable label that goes on, A long variable label that goes on, A long variable label that goes on, A long variable label that goes on, A long variable label that goes on, A long variable label that goes on'
 /txtna 'Text with missing codes'.
VALUE LABELS score 0 'Low' 5 'Mid' 10 'High'
 /band 1 'One' 2 'Two' 3 'Three' 7 'Not asked'
 /lab 'a' 'Alpha' 'b' 'Bravo'
 /uni 1 'Üüks' 2 'Kaks õ'
 /a_name_of_33_characters_xxxxxxxxx -1.5 'Minus one and a half' 0 'Zero' 2 'Two, spelled out in a long value label that is well past the sixty byte limit of the old files'
 /txtna 'n/a' 'Not available' 'fine' 'Fine'.
MISSING VALUES score (98, 99) /band (90 THRU 99, 7) /txtna ('n/a', '-').
"####;

fn fixture(name: &str) -> String {
    format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn run(engine: &mut Engine, source: &str) -> Result<(), String> {
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

/// What the engine holds, in the words of the table of facts.
fn differences(engine: &Engine, what: &str) -> Vec<String> {
    let mut out = Vec::new();
    let data = engine.dataset().unwrap();
    let names: Vec<&str> = data.variable_names().collect();
    let wanted: Vec<&str> = FACTS.iter().map(|v| v.name).collect();
    if names != wanted {
        out.push(format!("{what}: variables {names:?}, haven has {wanted:?}"));
        return out;
    }
    for fact in FACTS {
        let v = data.variable(fact.name).unwrap();
        let mut check = |part: &str, got: String, want: &str| {
            if got != want {
                out.push(format!(
                    "{what}: {} {part} is [{got}], haven has [{want}]",
                    fact.name
                ));
            }
        };
        check("label", v.label().unwrap_or("").to_string(), fact.label);
        check("format", v.format().to_string(), fact.format);
        let number = |x: f64| x.to_string();
        let mut labels: Vec<(Value, String)> = v.value_labels().to_vec();
        labels.sort_by(|a, b| match (&a.0, &b.0) {
            (Value::Number(x), Value::Number(y)) => x.total_cmp(y),
            (Value::String(x), Value::String(y)) => x.cmp(y),
            _ => std::cmp::Ordering::Equal,
        });
        let text: Vec<String> = labels
            .iter()
            .map(|(value, label)| match value {
                Value::Number(x) => format!("{}={label}", number(*x)),
                Value::String(s) => format!("{s}={label}"),
            })
            .collect();
        check("value labels", text.join(";"), fact.labels);
        let missing = match v.missing_values() {
            MissingValues::None => String::new(),
            MissingValues::Numbers(n) => {
                let n: Vec<String> = n.iter().map(|&x| number(x)).collect();
                format!("n:{}", n.join(","))
            }
            MissingValues::Range { low, high, extra } => {
                let extra = extra.map(|x| format!(",{}", number(x))).unwrap_or_default();
                format!("r:{},{}{extra}", number(*low), number(*high))
            }
            MissingValues::Strings(s) => format!("s:{}", s.join(",")),
        };
        check("missing values", missing, fact.missing);
        let values = match v.values() {
            Values::Numeric(values) => {
                let text = values.iter().map(|x| x.map_or(".".into(), number));
                text.collect::<Vec<_>>().join(" ")
            }
            Values::String(values) => values.iter().collect::<Vec<_>>().join("|"),
        };
        check("values", values, fact.values);
        if let Some(width) = v.string_width() {
            check("width", format!("A{width}"), fact.format);
        }
    }
    out
}

#[test]
fn reading_agrees_with_haven() {
    let mut failures = Vec::new();
    for name in [
        "haven/haven_made.sav",
        "haven/haven_made_plain.sav",
        "haven/haven_made_zsav.zsav",
    ] {
        let mut engine = Engine::default();
        match run(&mut engine, &format!("GET FILE='{}'.", fixture(name))) {
            Ok(()) => failures.extend(differences(&engine, name)),
            Err(error) => failures.push(format!("{name}: {error}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The bytes of a file without the product name and the creation date and time.
fn masked(mut bytes: Vec<u8>) -> Vec<u8> {
    bytes[4..64].fill(0);
    bytes[92..109].fill(0);
    bytes
}

#[test]
fn writing_agrees_with_haven() {
    let dir = std::env::temp_dir().join(format!("statscraft-haven-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let update = std::env::var_os("UPDATE_FIXTURES").is_some();
    let mut engine = Engine::default();
    run(&mut engine, WRITTEN).unwrap();
    let mut failures = differences(&engine, "built from syntax");
    for (name, option) in [
        ("haven/statscraft_made.sav", "COMPRESSED"),
        ("haven/statscraft_made_plain.sav", "UNCOMPRESSED"),
        ("haven/statscraft_made_zsav.zsav", "ZCOMPRESSED"),
    ] {
        let path = dir.join(name.rsplit('/').next().unwrap());
        let save = format!("SAVE OUTFILE='{}' /{option}.", path.display());
        if let Err(error) = run(&mut engine, &save) {
            failures.push(format!("{name}: {error}"));
            continue;
        }
        let written = std::fs::read(&path).unwrap();
        if update {
            std::fs::write(fixture(name), &written).unwrap();
        } else if option != "ZCOMPRESSED" {
            // the zlib blocks belong to the compressor, so the ZSAV file is held to its facts only
            let kept = std::fs::read(fixture(name)).unwrap();
            if masked(written) != masked(kept) {
                failures.push(format!(
                    "{name}: the bytes differ from the file haven read; if the writer changed on \
                     purpose, run with UPDATE_FIXTURES=1 and have haven read the new files again"
                ));
            }
        }
        let mut again = Engine::default();
        match run(&mut again, &format!("GET FILE='{}'.", path.display())) {
            Ok(()) => failures.extend(differences(&again, &format!("{name} read back"))),
            Err(error) => failures.push(format!("{name} read back: {error}")),
        }
    }
    std::fs::remove_dir_all(&dir).ok();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
