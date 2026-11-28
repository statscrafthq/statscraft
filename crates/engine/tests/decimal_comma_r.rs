//! Text files with a decimal comma, as Estonian files have it: GET DATA /TYPE=TXT, DATA LIST FREE,
//! LIST and FIXED under SET DECIMAL=COMMA and under the default point, on invented files.
//!
//! R reads the raw fields (`read.table` with a semicolon, a tab or a comma) and the conversion of a
//! field follows the manual, written out in the R script: the F and E formats take only the
//! decimal indicator of the setting (a comma under COMMA, a period under DOT) and a value in the
//! other form is system-missing, COMMA and DOLLAR take only a period, DOT only a comma, PCT a
//! trailing percent sign; fixed columns imply the decimals of the format when the number has no
//! decimal indicator.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    /// The text of the file.
    file: &'static str,
    /// `{FILE}` stands for the path of the file.
    command: &'static str,
    /// The variables the command makes, in order.
    names: &'static str,
    /// The values of each variable: numbers separated by spaces with `.` for missing, strings
    /// separated by `|`.
    columns: &'static [(&'static str, &'static str)],
}

const CASES: &[Case] = &[
    Case {
        label: r####"semicolons, a decimal comma in F and E, a period is no decimal indicator there"####,
        file: "id;price;ratio;exp;note\n1;3,5;-0,25;1,5E3;a\n2;,5;5,;2E-2;b\n3;1234,5;0,000123;-1,25e+2;\n4;;;;c\n5;10;0;7;d\n6;3.5;1.234,5;1,5;e\n",
        command: r####"SET DECIMAL=COMMA.
GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS=";" /FIRSTCASE=2
 /VARIABLES=id F3 price F8.2 ratio F10.6 exp E12 note A3."####,
        names: r####"id price ratio exp note"####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5 6"####),
            (r####"price"####, r####"3.5 0.5 1234.5 . 10 ."####),
            (r####"ratio"####, r####"-0.25 5 0.000123 . 0 ."####),
            (r####"exp"####, r####"1500 0.02 -125 . 7 1.5"####),
            (r####"note"####, r####"a|b||c|d|e"####),
        ],
    },
    Case {
        label: r####"the same file with the default decimal point: a comma is no decimal indicator"####,
        file: "id;price;ratio;exp;note\n1;3,5;-0,25;1,5E3;a\n2;,5;5,;2E-2;b\n3;1234,5;0,000123;-1,25e+2;\n4;;;;c\n5;10;0;7;d\n6;3.5;1.234,5;1,5;e\n",
        command: r####"SET DECIMAL=DOT.
GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS=";" /FIRSTCASE=2
 /VARIABLES=id F3 price F8.2 ratio F10.6 exp E12 note A3."####,
        names: r####"id price ratio exp note"####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5 6"####),
            (r####"price"####, r####". . . . 10 3.5"####),
            (r####"ratio"####, r####". . . . 0 ."####),
            (r####"exp"####, r####". 0.02 . . 7 ."####),
            (r####"note"####, r####"a|b||c|d|e"####),
        ],
    },
    Case {
        label: r####"COMMA, DOLLAR, PCT and DOT formats under a decimal comma: each keeps its own marks"####,
        file: "1.234,5;1,234.5;$1,234.5;12,5%;1.234,56\n.5;,5;$.5;5%;,5\n12;12;12;12;12\n",
        command: r####"SET DECIMAL=COMMA.
GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS=";"
 /VARIABLES=f F9.2 c COMMA9.2 d DOLLAR9.2 p PCT6.1 o DOT9.2."####,
        names: r####"f c d p o"####,
        columns: &[
            (r####"f"####, r####". . 12"####),
            (r####"c"####, r####"1234.5 5 12"####),
            (r####"d"####, r####"1234.5 0.5 12"####),
            (r####"p"####, r####"12.5 5 12"####),
            (r####"o"####, r####"1234.56 0.5 12"####),
        ],
    },
    Case {
        label: r####"the same formats under the default decimal point"####,
        file: "1.234,5;1,234.5;$1,234.5;12,5%;1.234,56\n.5;,5;$.5;5%;,5\n12;12;12;12;12\n",
        command: r####"SET DECIMAL=DOT.
GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS=";"
 /VARIABLES=f F9.2 c COMMA9.2 d DOLLAR9.2 p PCT6.1 o DOT9.2."####,
        names: r####"f c d p o"####,
        columns: &[
            (r####"f"####, r####". 0.5 12"####),
            (r####"c"####, r####"1234.5 5 12"####),
            (r####"d"####, r####"1234.5 0.5 12"####),
            (r####"p"####, r####". 5 12"####),
            (r####"o"####, r####"1234.56 0.5 12"####),
        ],
    },
    Case {
        label: r####"commas as delimiters, a decimal comma only inside qualifiers"####,
        file: "id,price,note\n1,\"3,5\",\"a,b\"\n2,\"10,25\",c\n3,,\"d\"\n",
        command: r####"SET DECIMAL=COMMA.
GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS="," /QUALIFIER='"' /FIRSTCASE=2
 /VARIABLES=id F2 price F6.2 note A4."####,
        names: r####"id price note"####,
        columns: &[
            (r####"id"####, r####"1 2 3"####),
            (r####"price"####, r####"3.5 10.25 ."####),
            (r####"note"####, r####"a,b|c|d"####),
        ],
    },
    Case {
        label: r####"tab separated with a decimal comma"####,
        file: "id\tprice\n1\t3,5\n2\t-0,5\n3\t\n4\t1000,25\n",
        command: r####"SET DECIMAL=COMMA.
GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS="\t" /FIRSTCASE=2
 /VARIABLES=id F2 price F8.2."####,
        names: r####"id price"####,
        columns: &[
            (r####"id"####, r####"1 2 3 4"####),
            (r####"price"####, r####"3.5 -0.5 . 1000.25"####),
        ],
    },
    Case {
        label: r####"AUTO makes a variable numeric from values with a decimal comma"####,
        file: "a;b;c\n3,5;4;,25\n2;2,5;-1\n",
        command: r####"SET DECIMAL=COMMA.
GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS=";" /FIRSTCASE=2
 /VARIABLES=a AUTO b AUTO c AUTO."####,
        names: r####"a b c"####,
        columns: &[
            (r####"a"####, r####"3.5 2"####),
            (r####"b"####, r####"4 2.5"####),
            (r####"c"####, r####"0.25 -1"####),
        ],
    },
    Case {
        label: r####"DATA LIST FREE with a semicolon and a decimal comma"####,
        file: "3,5;4,25;x\n-1,5;;y\n7;8,5;z\n",
        command: r####"SET DECIMAL=COMMA.
DATA LIST FILE='{FILE}' FREE(";") / a (F5.2) b (F5.2) c (A2)."####,
        names: r####"a b c"####,
        columns: &[
            (r####"a"####, r####"3.5 -1.5 7"####),
            (r####"b"####, r####"4.25 . 8.5"####),
            (r####"c"####, r####"x|y|z"####),
        ],
    },
    Case {
        label: r####"DATA LIST LIST with a semicolon and a decimal comma"####,
        file: "3,5;4,25;x\n-1,5;;y\n7;8,5;z\n",
        command: r####"SET DECIMAL=COMMA.
DATA LIST FILE='{FILE}' LIST(";") / a (F5.2) b (F5.2) c (A2)."####,
        names: r####"a b c"####,
        columns: &[
            (r####"a"####, r####"3.5 -1.5 7"####),
            (r####"b"####, r####"4.25 . 8.5"####),
            (r####"c"####, r####"x|y|z"####),
        ],
    },
    Case {
        label: r####"DATA LIST FIXED under a decimal comma: implied decimals when there is no comma"####,
        file: "  3,5 12345   ,25\n 12,5 1,5  7,125\n     99999  1.5\n",
        command: r####"SET DECIMAL=COMMA.
DATA LIST FILE='{FILE}' FIXED / a 1-5 (1) b 6-11 (2) c 12-17 (2)."####,
        names: r####"a b c"####,
        columns: &[
            (r####"a"####, r####"3.5 12.5 ."####),
            (r####"b"####, r####"123.45 1.5 999.99"####),
            (r####"c"####, r####"0.25 7.125 ."####),
        ],
    },
    Case {
        label: r####"GET DATA fixed columns under a decimal comma"####,
        file: "  3,5 12345   ,25\n 12,5 1,5  7,125\n     99999  1.5\n",
        command: r####"SET DECIMAL=COMMA.
GET DATA /TYPE=TXT /FILE='{FILE}' /ARRANGEMENT=FIXED
 /VARIABLES=a 0-4 F5.1 b 5-10 F6.2 c 11-16 F6.2."####,
        names: r####"a b c"####,
        columns: &[
            (r####"a"####, r####"3.5 12.5 ."####),
            (r####"b"####, r####"123.45 1.5 999.99"####),
            (r####"c"####, r####"0.25 7.125 ."####),
        ],
    },
];

fn show(engine: &Engine, name: &str) -> Option<String> {
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

fn run(case: &Case, path: &std::path::Path) -> Result<Engine, String> {
    std::fs::write(path, case.file).unwrap();
    let source = case.command.replace("{FILE}", &path.display().to_string());
    let (commands, errors) = parse(&source);
    if !errors.is_empty() {
        return Err(format!("{errors:?}"));
    }
    let mut engine = Engine::default();
    for command in &commands {
        engine
            .execute(&command.command)
            .map_err(|e| format!("line {}: {e}", command.first_line))?;
    }
    Ok(engine)
}

#[test]
fn a_decimal_comma_reads_as_r_and_the_manual_have_it() {
    let dir =
        std::env::temp_dir().join(format!("statscraft-decimal-comma-r-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = Vec::new();
    for (index, case) in CASES.iter().enumerate() {
        let engine = match run(case, &dir.join(format!("case{index}.txt"))) {
            Ok(engine) => engine,
            Err(error) => {
                failures.push(format!("{}: error {error}", case.label));
                continue;
            }
        };
        let names: Vec<&str> = engine.dataset().unwrap().variable_names().collect();
        if names.join(" ") != case.names {
            failures.push(format!(
                "{}: variables {}, wanted {}",
                case.label,
                names.join(" "),
                case.names
            ));
            continue;
        }
        for (name, want) in case.columns {
            let got = show(&engine, name).expect("the variable exists");
            if got != *want {
                failures.push(format!("{}: {name} is [{got}], R has [{want}]", case.label));
            }
        }
    }
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
}

// ---------------------------------------------------------------------------------------------
// Writing: SAVE TRANSLATE and WRITE with a decimal comma, read back by R.

const DATA: &str =
    "DATA LIST LIST /id (F2) x (F10.4) m (F10.2) y (F8.2) d (F10.2) p (F6.1) e (F12.3) name (A10).
BEGIN DATA
1 3.5 1234.5 0.5 1234.5 12.5 1500 'plain'
2 -0.25 -12345.678 -0.04 -0.5 100 0.000123 'a;b'
3 0.1 0.01 1000 12345.5 0 -123456789 'say \"x\"'
4 . . . . . . 'Zo\u{eb}'
5 1234567.891 99999.996 12 12 55.57 2.5 ''
END DATA.
FORMATS m (COMMA12.2) y (DOT10.2) d (DOLLAR10.2) p (PCT6.1).
";

/// The commands; `{OUT}` stands for the file each one writes.
const VARIANTS: &[(&str, &str)] = &[
    (
        "csv_set_comma",
        "SET DECIMAL=COMMA.\nSAVE TRANSLATE OUTFILE='{OUT}' /TYPE=CSV /FIELDNAMES /ENCODING='UTF8' BOM=NO.",
    ),
    (
        "csv_comma_pipe",
        "SAVE TRANSLATE OUTFILE='{OUT}' /TYPE=CSV /FIELDNAMES /TEXTOPTIONS DELIMITER='|' DECIMAL=COMMA /ENCODING='UTF8' BOM=NO.",
    ),
    (
        "csv_comma_variable",
        "SAVE TRANSLATE OUTFILE='{OUT}' /TYPE=CSV /FIELDNAMES /TEXTOPTIONS DECIMAL=COMMA FORMAT=VARIABLE /ENCODING='UTF8' BOM=NO.",
    ),
    (
        "csv_dot_under_comma",
        "SET DECIMAL=COMMA.\nSAVE TRANSLATE OUTFILE='{OUT}' /TYPE=CSV /FIELDNAMES /TEXTOPTIONS DECIMAL=DOT /ENCODING='UTF8' BOM=NO.",
    ),
    (
        "csv_set_comma_variable",
        "SET DECIMAL=COMMA.\nSAVE TRANSLATE OUTFILE='{OUT}' /TYPE=CSV /TEXTOPTIONS FORMAT=VARIABLE /ENCODING='UTF8' BOM=NO.",
    ),
    (
        "tab_set_comma",
        "SET DECIMAL=COMMA.\nSAVE TRANSLATE OUTFILE='{OUT}' /TYPE=TAB /FIELDNAMES /ENCODING='UTF8' BOM=NO.",
    ),
    (
        "write_set_comma",
        "SET DECIMAL=COMMA.\nWRITE OUTFILE='{OUT}' /id (F2) ' ' x (F10.4) ' ' m (COMMA12.2) ' ' y (DOT10.2) ' ' d (DOLLAR10.2) ' ' p (PCT6.1) ' ' e (F12.3).\nEXECUTE.",
    ),
    (
        "write_default",
        "WRITE OUTFILE='{OUT}' /id (F2) ' ' x (F10.4) ' ' m (COMMA12.2) ' ' y (DOT10.2) ' ' d (DOLLAR10.2) ' ' p (PCT6.1) ' ' e (F12.3).\nEXECUTE.",
    ),
];

#[test]
fn text_written_with_a_decimal_comma_is_what_r_reads() {
    let dir = std::env::temp_dir().join(format!("statscraft-dc-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let dump = std::env::var_os("DUMP_DIR").map(std::path::PathBuf::from);
    let mut failures = Vec::new();
    for (name, command) in VARIANTS {
        let mut session = statscraft_engine::session::Session::default();
        session.set_base_dir(Some(dir.clone()));
        let setup = format!("CD '{}'.\n{DATA}", dir.display());
        session
            .run(&setup, None)
            .unwrap_or_else(|e| panic!("{e:?}"));
        let out = dir.join(name);
        let source = command.replace("{OUT}", &out.display().to_string());
        if let Err(error) = session.run(&source, None) {
            failures.push(format!("{name}: {error:?}"));
            continue;
        }
        let text = String::from_utf8(std::fs::read(&out).unwrap()).unwrap();
        if let Some(dump) = &dump {
            std::fs::create_dir_all(dump).unwrap();
            std::fs::write(dump.join(name), &text).unwrap();
            continue;
        }
        match GOLDEN.iter().find(|(n, _)| n == name) {
            Some((_, want)) if *want == text => {}
            Some((_, want)) => failures.push(format!("{name}: wrote {text:?}, R checked {want:?}")),
            None => failures.push(format!("{name}: no text kept")),
        }
    }
    std::fs::remove_dir_all(&dir).ok();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// What the commands wrote when R read it back and found it right.
const GOLDEN: &[(&str, &str)] = &[
    (
        "csv_set_comma",
        "id;x;m;y;d;p;e;name\r\n1;3,5;1234,5;0,5;1234,5;12,5;1500;plain\r\n2;-0,25;-12345,678;-0,04;-0,5;100;0,000123;\"a;b\"\r\n3;0,1;0,01;1000;12345,5;0;-123456789;\"say \"\"x\"\"\"\r\n4;;;;;;;Zoë\r\n5;1234567,891;99999,996;12;12;55,57;2,5;\r\n",
    ),
    (
        "csv_comma_pipe",
        "id|x|m|y|d|p|e|name\r\n1|3,5|1234,5|0,5|1234,5|12,5|1500|plain\r\n2|-0,25|-12345,678|-0,04|-0,5|100|0,000123|a;b\r\n3|0,1|0,01|1000|12345,5|0|-123456789|\"say \"\"x\"\"\"\r\n4|||||||Zoë\r\n5|1234567,891|99999,996|12|12|55,57|2,5|\r\n",
    ),
    (
        "csv_dot_under_comma",
        "id,x,m,y,d,p,e,name\r\n1,3.5,1234.5,0.5,1234.5,12.5,1500,plain\r\n2,-0.25,-12345.678,-0.04,-0.5,100,0.000123,a;b\r\n3,0.1,0.01,1000,12345.5,0,-123456789,\"say \"\"x\"\"\"\r\n4,,,,,,,Zoë\r\n5,1234567.891,99999.996,12,12,55.57,2.5,\r\n",
    ),
    (
        "tab_set_comma",
        "id\tx\tm\ty\td\tp\te\tname\r\n1\t3,5\t1234,5\t0,5\t1234,5\t12,5\t1500\tplain\r\n2\t-0,25\t-12345,678\t-0,04\t-0,5\t100\t0,000123\ta;b\r\n3\t0,1\t0,01\t1000\t12345,5\t0\t-123456789\tsay \"x\"\r\n4\t\t\t\t\t\t\tZoë\r\n5\t1234567,891\t99999,996\t12\t12\t55,57\t2,5\t\r\n",
    ),
    (
        "csv_comma_variable",
        "id;x;m;y;d;p;e;name\r\n1;3,5000;1,234.50;,50;$1,234.50;12,5%;1500,000;plain\r\n2;-,2500;-12,345.68;-,04;-$.50;100,0%;,000;\"a;b\"\r\n3;,1000;.01;1.000,00;$12,345.50;,0%;-123456789,000;\"say \"\"x\"\"\"\r\n4;;;;;;;Zoë\r\n5;1234567,8910;100,000.00;12,00;$12.00;55,6%;2,500;\r\n",
    ),
    (
        "csv_set_comma_variable",
        "1;3,5000;1,234.50;,50;$1,234.50;12,5%;1500,000;plain\r\n2;-,2500;-12,345.68;-,04;-$.50;100,0%;,000;\"a;b\"\r\n3;,1000;.01;1.000,00;$12,345.50;,0%;-123456789,000;\"say \"\"x\"\"\"\r\n4;;;;;;;Zoë\r\n5;1234567,8910;100,000.00;12,00;$12.00;55,6%;2,500;\r\n",
    ),
    (
        "write_set_comma",
        " 1     3,5000     1,234.50        ,50  $1,234.50  12,5%     1500,000\n 2     -,2500   -12,345.68       -,04      -$.50 100,0%         ,000\n 3      ,1000          .01   1.000,00 $12,345.50    ,0% -123456789,0\n 4                                                                  \n 5 1234567,89   100,000.00      12,00     $12.00  55,6%        2,500\n",
    ),
    (
        "write_default",
        " 1     3.5000     1,234.50        ,50  $1,234.50  12.5%     1500.000\n 2     -.2500   -12,345.68       -,04      -$.50 100.0%         .000\n 3      .1000          .01   1.000,00 $12,345.50    .0% -123456789.0\n 4                                                                  \n 5 1234567.89   100,000.00      12,00     $12.00  55.6%        2.500\n",
    ),
];

fn session_with(dir: &std::path::Path, source: &str) -> statscraft_engine::session::Session {
    let mut session = statscraft_engine::session::Session::default();
    session.set_base_dir(Some(dir.to_path_buf()));
    let setup = format!("CD '{}'.\n{source}", dir.display());
    session
        .run(&setup, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    session
}

#[test]
fn a_delimiter_that_is_the_decimal_mark_is_refused() {
    let dir = std::env::temp_dir().join(format!("statscraft-dc-same-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut session = session_with(&dir, DATA);
    for text in [
        "SAVE TRANSLATE OUTFILE='a.csv' /TYPE=CSV /TEXTOPTIONS DELIMITER=',' DECIMAL=COMMA.",
        "SAVE TRANSLATE OUTFILE='a.csv' /TYPE=CSV /TEXTOPTIONS DELIMITER='.' DECIMAL=DOT.",
    ] {
        let error = session.run(text, None).unwrap_err();
        assert!(error[0].message.contains("must all differ"), "{error:?}");
    }
    assert!(!dir.join("a.csv").exists());
    std::fs::remove_dir_all(&dir).ok();
}

fn number_of(session: &mut statscraft_engine::session::Session, expr: &str) -> Vec<Option<f64>> {
    let text = format!("COMPUTE t = {expr}.");
    session.run(&text, None).unwrap_or_else(|e| panic!("{e:?}"));
    match session.dataset().unwrap().variable("t").unwrap().values() {
        Values::Numeric(v) => v.iter().collect(),
        Values::String(_) => panic!("{expr} is text"),
    }
}

fn text_of(session: &mut statscraft_engine::session::Session, expr: &str) -> String {
    static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let name = format!(
        "s{}",
        COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let text = format!("STRING {name} (A12).\nCOMPUTE {name} = {expr}.");
    session.run(&text, None).unwrap_or_else(|e| panic!("{e:?}"));
    match session.dataset().unwrap().variable(&name).unwrap().values() {
        Values::String(v) => v.iter().next().unwrap().trim().to_string(),
        Values::Numeric(_) => panic!("{expr} is a number"),
    }
}

#[test]
fn number_and_string_follow_the_decimal_indicator() {
    let dir = std::env::temp_dir().join(format!("statscraft-dc-fn-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = session_with(
        &dir,
        "DATA LIST FREE / x.\nBEGIN DATA\n1\nEND DATA.\nSET DECIMAL=COMMA.",
    );
    assert_eq!(number_of(&mut s, "NUMBER('3,5', F4.1)"), [Some(3.5)]);
    assert_eq!(number_of(&mut s, "NUMBER('3.5', F4.1)"), [None]);
    assert_eq!(number_of(&mut s, "NUMBER('1,5E2', E8)"), [Some(150.0)]);
    assert_eq!(number_of(&mut s, "NUMBER('12,5%', PCT6.1)"), [Some(12.5)]);
    assert_eq!(number_of(&mut s, "NUMBER('12.5%', PCT6.1)"), [None]);
    assert_eq!(
        number_of(&mut s, "NUMBER('1.234,5', DOT8.1)"),
        [Some(1234.5)]
    );
    assert_eq!(
        number_of(&mut s, "NUMBER('1,234.5', COMMA8.1)"),
        [Some(1234.5)]
    );
    assert_eq!(text_of(&mut s, "STRING(3.5, F4.1)"), "3,5");
    assert_eq!(text_of(&mut s, "STRING(1234.5, COMMA8.1)"), "1,234.5");
    assert_eq!(text_of(&mut s, "STRING(1234.5, DOT8.1)"), "1.234,5");
    assert_eq!(text_of(&mut s, "STRING(12.5, PCT6.1)"), "12,5%");
    s.run("SET DECIMAL=DOT.", None).unwrap();
    assert_eq!(number_of(&mut s, "NUMBER('3,5', F4.1)"), [None]);
    assert_eq!(number_of(&mut s, "NUMBER('3.5', F4.1)"), [Some(3.5)]);
    assert_eq!(text_of(&mut s, "STRING(3.5, F4.1)"), "3.5");
    std::fs::remove_dir_all(&dir).ok();
}

/// Doubles of every size written with a decimal comma read back to the same bits.
#[test]
fn doubles_written_with_a_decimal_comma_read_back_to_the_same_bits() {
    let dir = std::env::temp_dir().join(format!("statscraft-dc-bits-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut values: Vec<f64> = vec![0.1, 1.0 / 3.0, -2.5, 1e-7, 123456789.12345679, 1e15, 1e21];
    while values.len() < 300 {
        let value = f64::from_bits(next());
        if value.is_finite() && value.abs() > 1e-200 && value.abs() < 1e200 {
            values.push(value);
        }
    }
    let data: String = values.iter().map(|v| format!("{v:e}\n")).collect();
    let source = format!("DATA LIST FREE / x (F40).\nBEGIN DATA\n{data}END DATA.");
    let mut session = session_with(&dir, &source);
    session
        .run(
            "SET DECIMAL=COMMA.\nSAVE TRANSLATE OUTFILE='bits.csv' /TYPE=CSV /FIELDNAMES /ENCODING='UTF8' BOM=NO.\n\
             GET DATA /TYPE=TXT /FILE='bits.csv' /DELIMITERS=';' /FIRSTCASE=2 /VARIABLES=x F40.",
            None,
        )
        .unwrap_or_else(|e| panic!("{e:?}"));
    let read: Vec<f64> = match session.dataset().unwrap().variable("x").unwrap().values() {
        Values::Numeric(v) => v.iter().map(|v| v.unwrap_or(f64::NAN)).collect(),
        Values::String(_) => panic!("text"),
    };
    assert_eq!(read.len(), values.len());
    for (k, (a, b)) in values.iter().zip(&read).enumerate() {
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "value {k}: {a:e} came back as {b:e}"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}
