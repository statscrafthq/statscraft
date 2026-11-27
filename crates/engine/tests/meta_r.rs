//! The metadata of variables through a .sav file against R's haven and labelled: VARIABLE LABELS (a
//! later one replaces), VALUE LABELS (which deletes the labels the variable had) and ADD VALUE
//! LABELS (which keeps them and changes the ones it names), for numbers and for strings, MISSING
//! VALUES (each command replaces; () clears; discrete codes, a range, a range with one more code,
//! LO THRU and THRU HI), RENAME VARIABLES and DELETE VARIABLES (the metadata goes with the
//! variable), and variables made by COMPUTE and RECODE (none).
//! The engine saves each case; the test compares the dictionary the session holds and the one it
//! reads back from the file with the table below, which an R model of the rules (from the manual)
//! gave. haven and labelled were shown the very files the engine saved (set `META_DIR` to keep them
//! for that check) and read the same table; the test itself does not run R. What R has no
//! counterpart for: the measurement level and the custom attributes, which haven does not give,
//! and the display width and alignment, which it gives only when they are not the default.

use statscraft_data::{MissingValues, Value};
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct MetaCase {
    label: &'static str,
    command: &'static str,
    file: &'static str,
    /// name, variable label, `value=label` joined by `;` in the order of the values, missing
    /// values as `n:` codes, `r:` low, high and an optional extra code (LO and HI for the
    /// ends) or `s:` strings.
    vars: &'static [(&'static str, &'static str, &'static str, &'static str)],
}

const META_DATA: &str = r####"DATA LIST LIST /q1 (F2) q2 (F2) q3 (F2) s (A6) q4 (F2) q5 (F2).
BEGIN DATA
1 2 3 'a' -1 4
2 9 8 'n/a' 0 5
3 1 99 'b' 6 99
END DATA.
"####;

const META_CASES: &[MetaCase] = &[
    MetaCase {
        label: r####"variable labels: a later one replaces"####,
        command: r####"VARIABLE LABELS q1 'First'.
VARIABLE LABELS q1 'First, revised'.
VARIABLE LABELS q2 'Second item'.
VARIABLE LABELS s 'Group text'."####,
        file: r####"m1.sav"####,
        vars: &[
            (
                r####"q1"####,
                r####"First, revised"####,
                r####""####,
                r####""####,
            ),
            (
                r####"q2"####,
                r####"Second item"####,
                r####""####,
                r####""####,
            ),
            (r####"q3"####, r####""####, r####""####, r####""####),
            (
                r####"s"####,
                r####"Group text"####,
                r####""####,
                r####""####,
            ),
            (r####"q4"####, r####""####, r####""####, r####""####),
            (r####"q5"####, r####""####, r####""####, r####""####),
        ],
    },
    MetaCase {
        label: r####"VALUE LABELS deletes the labels the variable had, ADD VALUE LABELS keeps them and changes the ones named"####,
        command: r####"VALUE LABELS q1 1 'no' 2 'yes' 9 'unknown'.
ADD VALUE LABELS q1 3 'maybe' 1 'NO'.
VALUE LABELS q2 1 'a' 2 'b'.
VALUE LABELS q2 3 'c'.
ADD VALUE LABELS q3 5 'five'."####,
        file: r####"m2.sav"####,
        vars: &[
            (
                r####"q1"####,
                r####""####,
                r####"1=NO;2=yes;3=maybe;9=unknown"####,
                r####""####,
            ),
            (r####"q2"####, r####""####, r####"3=c"####, r####""####),
            (r####"q3"####, r####""####, r####"5=five"####, r####""####),
            (r####"s"####, r####""####, r####""####, r####""####),
            (r####"q4"####, r####""####, r####""####, r####""####),
            (r####"q5"####, r####""####, r####""####, r####""####),
        ],
    },
    MetaCase {
        label: r####"value labels of a string variable"####,
        command: r####"VALUE LABELS s 'x' 'ex' 'y' 'why'.
ADD VALUE LABELS s 'z' 'zed' 'x' 'EX'."####,
        file: r####"m3.sav"####,
        vars: &[
            (r####"q1"####, r####""####, r####""####, r####""####),
            (r####"q2"####, r####""####, r####""####, r####""####),
            (r####"q3"####, r####""####, r####""####, r####""####),
            (
                r####"s"####,
                r####""####,
                r####"x=EX;y=why;z=zed"####,
                r####""####,
            ),
            (r####"q4"####, r####""####, r####""####, r####""####),
            (r####"q5"####, r####""####, r####""####, r####""####),
        ],
    },
    MetaCase {
        label: r####"MISSING VALUES: each command replaces, () clears, ranges and a range with one more code"####,
        command: r####"MISSING VALUES q1 (9).
MISSING VALUES q1 (8, 9).
MISSING VALUES q2 (1 THRU 3).
MISSING VALUES q3 (1 THRU 3, 99).
MISSING VALUES s ('n/a').
MISSING VALUES q2 ().
MISSING VALUES q4 (LO THRU 0).
MISSING VALUES q5 (5 THRU HI)."####,
        file: r####"m4.sav"####,
        vars: &[
            (r####"q1"####, r####""####, r####""####, r####"n:8,9"####),
            (r####"q2"####, r####""####, r####""####, r####""####),
            (r####"q3"####, r####""####, r####""####, r####"r:1,3,99"####),
            (r####"s"####, r####""####, r####""####, r####"s:n/a"####),
            (r####"q4"####, r####""####, r####""####, r####"r:LO,0"####),
            (r####"q5"####, r####""####, r####""####, r####"r:5,HI"####),
        ],
    },
    MetaCase {
        label: r####"RENAME VARIABLES and DELETE VARIABLES keep the metadata of what remains, and new variables have none"####,
        command: r####"VARIABLE LABELS q1 'First'.
VALUE LABELS q1 1 'no' 2 'yes'.
MISSING VALUES q1 (9).
VALUE LABELS q3 1 'gone'.
RENAME VARIABLES (q1 = first).
DELETE VARIABLES q3.
ADD VALUE LABELS first 3 'maybe'.
COMPUTE q6 = first * 2.
RECODE first (1 = 10) (2 = 20) (ELSE = 0) INTO q7."####,
        file: r####"m5.sav"####,
        vars: &[
            (
                r####"first"####,
                r####"First"####,
                r####"1=no;2=yes;3=maybe"####,
                r####"n:9"####,
            ),
            (r####"q2"####, r####""####, r####""####, r####""####),
            (r####"s"####, r####""####, r####""####, r####""####),
            (r####"q4"####, r####""####, r####""####, r####""####),
            (r####"q5"####, r####""####, r####""####, r####""####),
            (r####"q6"####, r####""####, r####""####, r####""####),
            (r####"q7"####, r####""####, r####""####, r####""####),
        ],
    },
];

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

fn number(x: f64) -> String {
    if x < -1e300 {
        "LO".into()
    } else if x > 1e300 {
        "HI".into()
    } else {
        x.to_string()
    }
}

/// The metadata of every variable of the dataset in the form of the table.
fn facts(engine: &Engine) -> Vec<(String, String, String, String)> {
    let data = engine.dataset().unwrap();
    data.variable_names()
        .map(|name| {
            let v = data.variable(name).unwrap();
            let mut labels = v.value_labels().to_vec();
            labels.sort_by(|a, b| match (&a.0, &b.0) {
                (Value::Number(x), Value::Number(y)) => x.total_cmp(y),
                (Value::String(x), Value::String(y)) => x.cmp(y),
                _ => std::cmp::Ordering::Equal,
            });
            let labels: Vec<String> = labels
                .iter()
                .map(|(value, label)| match value {
                    Value::Number(x) => format!("{}={label}", number(*x)),
                    Value::String(s) => format!("{}={label}", s.trim_end()),
                })
                .collect();
            let missing = match v.missing_values() {
                MissingValues::None => String::new(),
                MissingValues::Numbers(n) => {
                    format!(
                        "n:{}",
                        n.iter().map(|&x| number(x)).collect::<Vec<_>>().join(",")
                    )
                }
                MissingValues::Range { low, high, extra } => {
                    let extra = extra.map(|x| format!(",{}", number(x))).unwrap_or_default();
                    format!("r:{},{}{extra}", number(*low), number(*high))
                }
                MissingValues::Strings(s) => format!(
                    "s:{}",
                    s.iter()
                        .map(|x| x.trim_end().to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            };
            (
                name.to_string(),
                v.label().unwrap_or("").to_string(),
                labels.join(";"),
                missing,
            )
        })
        .collect()
}

#[test]
fn variable_metadata_goes_through_a_sav_file_as_the_table_says() {
    let kept = std::env::var_os("META_DIR").map(std::path::PathBuf::from);
    let dir = kept.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("statscraft-meta-{}", std::process::id()))
    });
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = Vec::new();
    for case in META_CASES {
        let wanted: Vec<(String, String, String, String)> = case
            .vars
            .iter()
            .map(|(a, b, c, d)| (a.to_string(), b.to_string(), c.to_string(), d.to_string()))
            .collect();
        let path = dir.join(case.file);
        let mut engine = Engine::default();
        let source = format!(
            "{META_DATA}{}\nSAVE OUTFILE='{}'.",
            case.command,
            path.display()
        );
        if let Err(error) = execute_all(&mut engine, &source) {
            failures.push(format!("{}: error {error}", case.label));
            continue;
        }
        // the dictionary as the session holds it, and as the file gives it back
        let mut again = Engine::default();
        if let Err(error) = execute_all(&mut again, &format!("GET FILE='{}'.", path.display())) {
            failures.push(format!("{}: read back: {error}", case.label));
            continue;
        }
        for (what, got) in [
            ("in the session", facts(&engine)),
            ("read back", facts(&again)),
        ] {
            if got != wanted {
                failures.push(format!(
                    "{}: {what} the variables are\n{got:?}\nR has\n{wanted:?}",
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
