//! SAVE TRANSLATE to CSV and TAB, and WRITE, read back by R.
//!
//! The data are invented: six cases with a name that holds a comma, a quote, a semicolon, a
//! leading blank and non-ASCII letters, numbers from 0.000123 to 1234567.891 and negative ones,
//! system-missing and user-missing values, a labelled and an unlabelled code, a date, a time, a
//! date with a time, and amounts with dollar, comma and percent formats. The text each command
//! writes is kept below; the R script (`read.csv`, `read.csv2`, `read.delim` and `read.fwf`)
//! reads every one of them back and checks that it gives the data that were written: the same
//! numbers to the last digit, the same strings, the dates as the same instants, the qualifiers
//! undone, and for WRITE the same text as R's own formatting of each value.
//! Set `DUMP_DIR` to a folder to write the files there for that check instead of comparing.
//! The kept text and the hash are a snapshot of what R and haven were shown when the tests were
//! written: the test itself runs neither, so it tells that a file changed; after a change that is
//! meant, write the files again, have R and haven read them, and keep the new text.
//!
//! The portable file (EXPORT) is read by haven (`read_por`): the names, labels, value labels,
//! user-missing codes, formats and values it finds are the table of facts, and the engine's own
//! data and its reading of the file agree with it. The file itself is not kept (it holds the
//! format's marker word); a hash of its bytes, without the creation date and time and the product
//! version, says it is the file haven read. Text goes in by the file's character table: a letter
//! with an accent has no code in it and is written as ? with a warning, a symbol the character set
//! has (plus-minus, degree, less-or-equal, the dagger) is written as its byte in the table, and
//! haven reads the text as it stands (the UTF-8 bytes of earlier versions were not in the table,
//! and haven made replacement characters of them).

use statscraft_data::{Dataset, MissingValues, Value, Values};
use statscraft_engine::Engine;
use statscraft_engine::session::Session;
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST LIST /id (F3) name (A14) score (F9.3) cnt (F2) grp (F1) when (DATE11)
  at (TIME8) stamp (DATETIME20) money (F10.2) pct (F6.1) cm (F10.1).
BEGIN DATA
1 'Ann Lee' 12.5 1 1 '15-JAN-2024' '00:00:00' '15-JAN-2024 08:30:00' 1234.5 12.5 1234.5
2 'Smith, Bo' -0.1 2 2 . '13:45:10' '29-FEB-2024 23:59:59' 0.5 100 -0.5
3 'say \"hi\"' 1234567.891 3 3 '29-FEB-2024' '23:59:59' '01-JAN-1999 00:00:01' -99.99 0.1 12
4 '' 0.000123 99 1 '01-JAN-1999' '01:02:03' . 1000000 . 7777777.7
5 ' lead pad' . 5 2 '31-DEC-2030' . '31-DEC-2030 12:00:00' 12 55.5 .
6 'Zo\u{eb}; \u{d5}ie' 7 . 1 '07-JUL-2007' '12:00:00' '07-JUL-2007 07:07:07' . 0 0.1
END DATA.
VARIABLE LABELS id 'Case id' /score 'Score, scaled' /grp 'Group'.
VALUE LABELS grp 1 'low' 2 'high'.
MISSING VALUES cnt (99).
FORMATS money (DOLLAR10.2) pct (PCT6.1) cm (COMMA10.1) score (F9.3).
";

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

/// The commands; `{OUT}` stands for the file each one writes.
const VARIANTS: &[(&str, &str)] = &[
    (
        "csv_plain",
        "SAVE TRANSLATE OUTFILE='{OUT}' /TYPE=CSV /FIELDNAMES.",
    ),
    (
        "csv_comma_decimal",
        "SAVE TRANSLATE OUTFILE='{OUT}' /TYPE=CSV /FIELDNAMES /TEXTOPTIONS DECIMAL=COMMA.",
    ),
    (
        "csv_own_delimiter",
        "SAVE TRANSLATE OUTFILE='{OUT}' /TYPE=CSV /TEXTOPTIONS DELIMITER=';' QUALIFIER=\"'\" /ENCODING='UTF8' BOM=NO.",
    ),
    (
        "csv_labels",
        "SAVE TRANSLATE OUTFILE='{OUT}' /TYPE=CSV /FIELDNAMES VALUE=LABELS /CELLS=LABELS /ENCODING='UTF8' BOM=NO.",
    ),
    (
        "csv_display_formats",
        "SAVE TRANSLATE OUTFILE='{OUT}' /TYPE=CSV /FIELDNAMES /TEXTOPTIONS FORMAT=VARIABLE /ENCODING='UTF8' BOM=NO.",
    ),
    (
        "csv_user_missing_empty",
        "SAVE TRANSLATE OUTFILE='{OUT}' /TYPE=CSV /FIELDNAMES /MISSING=RECODE /ENCODING='UTF8' BOM=NO.",
    ),
    (
        "csv_keep_rename",
        "SAVE TRANSLATE OUTFILE='{OUT}' /TYPE=CSV /FIELDNAMES /KEEP=id name score /RENAME=(score = points) /ENCODING='UTF8' BOM=NO.",
    ),
    (
        "tab_plain",
        "SAVE TRANSLATE OUTFILE='{OUT}' /TYPE=TAB /FIELDNAMES /ENCODING='UTF8' BOM=NO.",
    ),
    (
        "tab_comma_decimal",
        "SAVE TRANSLATE OUTFILE='{OUT}' /TYPE=TAB /TEXTOPTIONS DECIMAL=COMMA /ENCODING='UTF8' BOM=NO.",
    ),
    (
        "write_columns",
        "WRITE OUTFILE='{OUT}' /id (F3) name (A14) score (F9.3) cnt (F3) when (DATE11) at (TIME8) money (DOLLAR14.2) pct (PCT6.1) cm (COMMA12.1).\nEXECUTE.",
    ),
    (
        "write_two_records",
        "WRITE OUTFILE='{OUT}' RECORDS=2 /1 id (F3) ' ' grp (F1) ' ' stamp (DATETIME20) /2 name (A14) '|' cnt (F2).\nEXECUTE.",
    ),
];

/// What the commands wrote when R read it back and found it right.
const GOLDEN: &[(&str, &str)] = &[
    (
        "csv_plain",
        "\u{feff}id,name,score,cnt,grp,when,at,stamp,money,pct,cm\r\n1,Ann Lee,12.5,1,1,01/15/2024,00:00:00,01/15/2024 08:30:00,1234.5,12.5,1234.5\r\n2,\"Smith, Bo\",-0.1,2,2,,13:45:10,02/29/2024 23:59:59,0.5,100,-0.5\r\n3,\"say \"\"hi\"\"\",1234567.891,3,3,02/29/2024,23:59:59,01/01/1999 00:00:01,-99.99,0.1,12\r\n4,,0.000123,99,1,01/01/1999,01:02:03,,1000000,,7777777.7\r\n5, lead pad,,5,2,12/31/2030,,12/31/2030 12:00:00,12,55.5,\r\n6,Zoë; Õie,7,,1,07/07/2007,12:00:00,07/07/2007 07:07:07,,0,0.1\r\n",
    ),
    (
        "csv_comma_decimal",
        "\u{feff}id;name;score;cnt;grp;when;at;stamp;money;pct;cm\r\n1;Ann Lee;12,5;1;1;01/15/2024;00:00:00;01/15/2024 08:30:00;1234,5;12,5;1234,5\r\n2;Smith, Bo;-0,1;2;2;;13:45:10;02/29/2024 23:59:59;0,5;100;-0,5\r\n3;\"say \"\"hi\"\"\";1234567,891;3;3;02/29/2024;23:59:59;01/01/1999 00:00:01;-99,99;0,1;12\r\n4;;0,000123;99;1;01/01/1999;01:02:03;;1000000;;7777777,7\r\n5; lead pad;;5;2;12/31/2030;;12/31/2030 12:00:00;12;55,5;\r\n6;\"Zoë; Õie\";7;;1;07/07/2007;12:00:00;07/07/2007 07:07:07;;0;0,1\r\n",
    ),
    (
        "csv_own_delimiter",
        "1;Ann Lee;12.5;1;1;01/15/2024;00:00:00;01/15/2024 08:30:00;1234.5;12.5;1234.5\r\n2;Smith, Bo;-0.1;2;2;;13:45:10;02/29/2024 23:59:59;0.5;100;-0.5\r\n3;say \"hi\";1234567.891;3;3;02/29/2024;23:59:59;01/01/1999 00:00:01;-99.99;0.1;12\r\n4;;0.000123;99;1;01/01/1999;01:02:03;;1000000;;7777777.7\r\n5; lead pad;;5;2;12/31/2030;;12/31/2030 12:00:00;12;55.5;\r\n6;'Zoë; Õie';7;;1;07/07/2007;12:00:00;07/07/2007 07:07:07;;0;0.1\r\n",
    ),
    (
        "csv_labels",
        "Case id,name,\"Score, scaled\",cnt,Group,when,at,stamp,money,pct,cm\r\n1,Ann Lee,12.5,1,low,01/15/2024,00:00:00,01/15/2024 08:30:00,1234.5,12.5,1234.5\r\n2,\"Smith, Bo\",-0.1,2,high,,13:45:10,02/29/2024 23:59:59,0.5,100,-0.5\r\n3,\"say \"\"hi\"\"\",1234567.891,3,3,02/29/2024,23:59:59,01/01/1999 00:00:01,-99.99,0.1,12\r\n4,,0.000123,99,low,01/01/1999,01:02:03,,1000000,,7777777.7\r\n5, lead pad,,5,high,12/31/2030,,12/31/2030 12:00:00,12,55.5,\r\n6,Zoë; Õie,7,,low,07/07/2007,12:00:00,07/07/2007 07:07:07,,0,0.1\r\n",
    ),
    (
        "csv_display_formats",
        "id,name,score,cnt,grp,when,at,stamp,money,pct,cm\r\n1,Ann Lee,12.500,1,1,15-JAN-2024,00:00:00,15-JAN-2024 08:30:00,\"$1,234.50\",12.5%,\"1,234.5\"\r\n2,\"Smith, Bo\",-.100,2,2,,13:45:10,29-FEB-2024 23:59:59,$.50,100.0%,-.5\r\n3,\"say \"\"hi\"\"\",1234567.891,3,3,29-FEB-2024,23:59:59,01-JAN-1999 00:00:01,-$99.99,.1%,12.0\r\n4,,.000,99,1,01-JAN-1999,01:02:03,,\"$1,000,000.00\",,\"7,777,777.7\"\r\n5, lead pad,,5,2,31-DEC-2030,,31-DEC-2030 12:00:00,$12.00,55.5%,\r\n6,Zoë; Õie,7.000,,1,07-JUL-2007,12:00:00,07-JUL-2007 07:07:07,,.0%,.1\r\n",
    ),
    (
        "csv_user_missing_empty",
        "id,name,score,cnt,grp,when,at,stamp,money,pct,cm\r\n1,Ann Lee,12.5,1,1,01/15/2024,00:00:00,01/15/2024 08:30:00,1234.5,12.5,1234.5\r\n2,\"Smith, Bo\",-0.1,2,2,,13:45:10,02/29/2024 23:59:59,0.5,100,-0.5\r\n3,\"say \"\"hi\"\"\",1234567.891,3,3,02/29/2024,23:59:59,01/01/1999 00:00:01,-99.99,0.1,12\r\n4,,0.000123,,1,01/01/1999,01:02:03,,1000000,,7777777.7\r\n5, lead pad,,5,2,12/31/2030,,12/31/2030 12:00:00,12,55.5,\r\n6,Zoë; Õie,7,,1,07/07/2007,12:00:00,07/07/2007 07:07:07,,0,0.1\r\n",
    ),
    (
        "csv_keep_rename",
        "id,name,points\r\n1,Ann Lee,12.5\r\n2,\"Smith, Bo\",-0.1\r\n3,\"say \"\"hi\"\"\",1234567.891\r\n4,,0.000123\r\n5, lead pad,\r\n6,Zoë; Õie,7\r\n",
    ),
    (
        "tab_plain",
        "id\tname\tscore\tcnt\tgrp\twhen\tat\tstamp\tmoney\tpct\tcm\r\n1\tAnn Lee\t12.5\t1\t1\t01/15/2024\t00:00:00\t01/15/2024 08:30:00\t1234.5\t12.5\t1234.5\r\n2\tSmith, Bo\t-0.1\t2\t2\t\t13:45:10\t02/29/2024 23:59:59\t0.5\t100\t-0.5\r\n3\tsay \"hi\"\t1234567.891\t3\t3\t02/29/2024\t23:59:59\t01/01/1999 00:00:01\t-99.99\t0.1\t12\r\n4\t\t0.000123\t99\t1\t01/01/1999\t01:02:03\t\t1000000\t\t7777777.7\r\n5\t lead pad\t\t5\t2\t12/31/2030\t\t12/31/2030 12:00:00\t12\t55.5\t\r\n6\tZoë; Õie\t7\t\t1\t07/07/2007\t12:00:00\t07/07/2007 07:07:07\t\t0\t0.1\r\n",
    ),
    (
        "tab_comma_decimal",
        "1\tAnn Lee\t12,5\t1\t1\t01/15/2024\t00:00:00\t01/15/2024 08:30:00\t1234,5\t12,5\t1234,5\r\n2\tSmith, Bo\t-0,1\t2\t2\t\t13:45:10\t02/29/2024 23:59:59\t0,5\t100\t-0,5\r\n3\tsay \"hi\"\t1234567,891\t3\t3\t02/29/2024\t23:59:59\t01/01/1999 00:00:01\t-99,99\t0,1\t12\r\n4\t\t0,000123\t99\t1\t01/01/1999\t01:02:03\t\t1000000\t\t7777777,7\r\n5\t lead pad\t\t5\t2\t12/31/2030\t\t12/31/2030 12:00:00\t12\t55,5\t\r\n6\tZoë; Õie\t7\t\t1\t07/07/2007\t12:00:00\t07/07/2007 07:07:07\t\t0\t0,1\r\n",
    ),
    (
        "write_columns",
        "  1Ann Lee          12.500  115-JAN-202400:00:00     $1,234.50 12.5%     1,234.5\n  2Smith, Bo         -.100  2           13:45:10          $.50100.0%         -.5\n  3say \"hi\"      1234567.9  329-FEB-202423:59:59       -$99.99   .1%        12.0\n  4                   .000 9901-JAN-199901:02:03 $1,000,000.00       7,777,777.7\n  5 lead pad                531-DEC-2030                $12.00 55.5%            \n  6Zoë; Õie          7.000   07-JUL-200712:00:00                 .0%          .1\n",
    ),
    (
        "write_two_records",
        "  1 1 15-JAN-2024 08:30:00\nAnn Lee       | 1\n  2 2 29-FEB-2024 23:59:59\nSmith, Bo     | 2\n  3 3 01-JAN-1999 00:00:01\nsay \"hi\"      | 3\n  4 1                     \n              |99\n  5 2 31-DEC-2030 12:00:00\n lead pad     | 5\n  6 1 07-JUL-2007 07:07:07\nZoë; Õie      |  \n",
    ),
];

#[test]
fn text_written_is_read_back_by_r() {
    let dir = std::env::temp_dir().join(format!("statscraft-textout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dump = std::env::var_os("DUMP_DIR").map(std::path::PathBuf::from);
    let mut session = Session::default();
    session.set_base_dir(Some(dir.clone()));
    let setup = format!("CD '{}'.\n{DATA}", dir.display());
    session
        .run(&setup, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let mut failures = Vec::new();
    for (name, command) in VARIANTS {
        let out = dir.join(name);
        let source = command.replace("{OUT}", &out.display().to_string());
        if let Err(error) = session.run(&source, None) {
            failures.push(format!("{name}: {error:?}"));
            continue;
        }
        let bytes = std::fs::read(&out).unwrap();
        if let Some(dump) = &dump {
            std::fs::create_dir_all(dump).unwrap();
            std::fs::write(dump.join(name), &bytes).unwrap();
            continue;
        }
        let text = String::from_utf8(bytes).unwrap();
        match GOLDEN.iter().find(|(n, _)| n == name) {
            Some((_, want)) if *want == text => {}
            Some((_, want)) => failures.push(format!("{name}: wrote {text:?}, R checked {want:?}")),
            None => failures.push(format!("{name}: no text kept")),
        }
    }
    std::fs::remove_dir_all(&dir).ok();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

const PORTABLE_FACTS: &[Var] = &[
    Var {
        name: r#"ID"#,
        label: r#"Case id ≤ 6 †"#,
        format: r#"F3.0"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"1 2 3 4 5 6"#,
    },
    Var {
        name: r#"NAME"#,
        label: r#""#,
        format: r#"A14"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"Ann Lee|Smith, Bo|say "hi"|| lead #1 ±2°|Zo?; ?ie"#,
    },
    Var {
        name: r#"SCORE"#,
        label: r#"Score, scaled"#,
        format: r#"F9.3"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"12.5 -0.1 1234567.891 0.000123 . 7"#,
    },
    Var {
        name: r#"CNT"#,
        label: r#""#,
        format: r#"F2.0"#,
        labels: r#""#,
        missing: r#"n:99"#,
        values: r#"1 2 3 99 5 ."#,
    },
    Var {
        name: r#"GRP"#,
        label: r#"Group"#,
        format: r#"F1.0"#,
        labels: r#"1=low;2=high ±1"#,
        missing: r#""#,
        values: r#"1 2 3 1 2 1"#,
    },
    Var {
        name: r#"WHEN"#,
        label: r#""#,
        format: r#"DATE11"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"13924656000 . 13928544000 13134528000 14144284800 13403145600"#,
    },
    Var {
        name: r#"AT"#,
        label: r#""#,
        format: r#"TIME8"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"0 49510 86399 3723 . 43200"#,
    },
    Var {
        name: r#"STAMP"#,
        label: r#""#,
        format: r#"DATETIME20"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"13924686600 13928630399 13134528001 . 14144328000 13403171227"#,
    },
    Var {
        name: r#"MONEY"#,
        label: r#""#,
        format: r#"DOLLAR10.2"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"1234.5 0.5 -99.99 1000000 12 ."#,
    },
    Var {
        name: r#"PCT"#,
        label: r#""#,
        format: r#"PCT6.1"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"12.5 100 0.1 . 55.5 0"#,
    },
    Var {
        name: r#"CM"#,
        label: r#""#,
        format: r#"COMMA10.1"#,
        labels: r#""#,
        missing: r#""#,
        values: r#"1234.5 -0.5 12 7777777.7 . 0.1"#,
    },
];

/// What the engine holds, in the words of a table of facts.
fn differences(data: &Dataset, facts: &[Var], what: &str) -> Vec<String> {
    let mut out = Vec::new();
    let names: Vec<&str> = data.variable_names().collect();
    let wanted: Vec<&str> = facts.iter().map(|v| v.name).collect();
    // a portable file keeps names in capitals, a dataset keeps the case it was given
    let alike = names.len() == wanted.len()
        && names
            .iter()
            .zip(&wanted)
            .all(|(a, b)| a.eq_ignore_ascii_case(b));
    if !alike {
        out.push(format!("{what}: variables {names:?}, haven has {wanted:?}"));
        return out;
    }
    for fact in facts {
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

/// The portable file: the same cases, with a # and symbols in a name, a label and a value label.
/// A letter outside ASCII has no code in the file's character table and is written as ?; a #
/// and the symbols plus-minus, degree, less-or-equal and the dagger have one.
fn portable_data() -> String {
    DATA.replace(" lead pad", " lead #1 \u{b1}2\u{b0}")
        .replace("'Case id'", "'Case id \u{2264} 6 \u{2020}'")
        .replace("2 'high'", "2 'high \u{b1}1'")
}

/// The bytes of a portable file as text without line breaks, and without the creation date and
/// time and the product text with its version, which follow the marker at character 464.
fn portable_normalized(bytes: &[u8]) -> Vec<u8> {
    // the file's symbols are bytes from 0x80 on, so this works on bytes
    let text: Vec<u8> = bytes
        .iter()
        .copied()
        .filter(|c| *c != b'\n' && *c != b'\r')
        .collect();
    let (head, rest) = text.split_at(464);
    // A8/yyyymmdd, 6/hhmmss, then 1 and a base 30 length and / and that many characters
    let rest = rest.strip_prefix(b"A8/").unwrap()[8..]
        .strip_prefix(b"6/")
        .unwrap()[6..]
        .strip_prefix(b"1")
        .unwrap();
    // the length is written in base 30, as many digits as it needs, up to the slash
    let slash = rest.iter().position(|&c| c == b'/').unwrap();
    let length = rest[..slash].iter().fold(0, |sum, &c| {
        sum * 30 + char::from(c).to_digit(30).unwrap() as usize
    });
    [head, &rest[slash + 1 + length..]].concat()
}

fn fnv(text: &[u8]) -> u64 {
    text.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[test]
fn the_portable_file_is_read_by_haven() {
    let dir = std::env::temp_dir().join(format!("statscraft-por-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut session = Session::default();
    session.set_base_dir(Some(dir.clone()));
    let path = dir.join("out.por");
    let source = format!(
        "CD '{}'.\n{}EXPORT OUTFILE='{}'.",
        dir.display(),
        portable_data(),
        path.display()
    );
    let result = session
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let warnings: Vec<String> = result
        .commands
        .iter()
        .flat_map(|c| c.warnings.clone())
        .collect();
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("no code for 2 characters")),
        "{warnings:?}"
    );
    let bytes = std::fs::read(&path).unwrap();
    if let Some(dump) = std::env::var_os("DUMP_DIR") {
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dump).unwrap();
        std::fs::write(std::path::Path::new(&dump).join("por"), &bytes).unwrap();
        println!("hash {}", fnv(&portable_normalized(&bytes)));
        return;
    }
    // the data as haven read the file, and as the engine reads it back
    // the dataset keeps its letters; what the file holds, as haven reads it, is what comes back
    let names: Vec<&str> = match session
        .dataset()
        .unwrap()
        .variable("name")
        .unwrap()
        .values()
    {
        Values::String(v) => v.iter().collect(),
        Values::Numeric(_) => panic!("name is a string"),
    };
    assert_eq!(
        names,
        [
            "Ann Lee",
            "Smith, Bo",
            "say \"hi\"",
            "",
            " lead #1 \u{b1}2\u{b0}",
            "Zo\u{eb}; \u{d5}ie"
        ]
    );
    let mut failures = Vec::new();
    let mut again = Engine::default();
    let (commands, errors) = parse(&format!("IMPORT FILE='{}'.", path.display()));
    assert!(errors.is_empty(), "{errors:?}");
    let imported = again.execute(&commands[0].command);
    std::fs::remove_dir_all(&dir).ok();
    imported.unwrap();
    failures.extend(differences(
        again.dataset().unwrap(),
        PORTABLE_FACTS,
        "read back",
    ));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    // the same bytes as the file haven read
    assert_eq!(
        fnv(&portable_normalized(&bytes)),
        PORTABLE_HASH,
        "the portable file changed; have haven read the new file again and update the facts"
    );
}

/// The hash of the portable file as haven read it.
const PORTABLE_HASH: u64 = 6702505346293743288;
