//! APPLY DICTIONARY against a file R's haven wrote (`tests/fixtures/apply/src.sav`: variable labels, value
//! labels, a discrete missing code, a missing range and a format on six variables) and a target the
//! engine builds from syntax. R holds a model of the manual's rules, written out in the R script:
//! variables match by name and type (a number with a number, a string with a string); a defined
//! attribute of the source replaces the target's and an empty one does not (the empty label and the
//! missing value labels of one variable leave the target's as they are); VALLABELS replaces the
//! value labels by default and MERGE joins them (the source wins on a value both label); a
//! variable of another type only gets the label; NEWVARS makes the variables the target lacks;
//! SOURCE with one variable and TARGET apply it to every target variable. The table is what that
//! model gives; haven and labelled read the very files the engine saved to the same table (set
//! `APPLY_DIR` to keep them for that check; the test itself does not run R).

use statscraft_data::{MissingValues, Value};
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    command: &'static str,
    file: &'static str,
    /// name, variable label, value labels, missing values, format
    vars: &'static [(
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
    )],
}

const TARGET: &str = r####"DATA LIST LIST /a b d (F5) c (A3) f (A3) g (F2).
BEGIN DATA
1 1 1 'x' 'p' 4
2 2 2 'y' 'q' 5
END DATA.
VARIABLE LABELS a 'Old alpha' /b 'Beta (target)'.
VALUE LABELS a 1 'uno' 3 'tres' /b 5 'five'.
FORMATS a b (F5.0)."####;

const CASES: &[Case] = &[
    Case {
        label: r####"APPLY DICTIONARY: labels, value labels (replaced), missing values and formats of matching variables; a type that differs gets the label only; an empty label or value labels do not overwrite"####,
        command: r####"APPLY DICTIONARY FROM '{SRC}/src.sav'."####,
        file: r####"p1.sav"####,
        vars: &[
            (
                r####"a"####,
                r####"Alpha"####,
                r####"1=low;2=high"####,
                r####"n:9"####,
                r####"F8.2"####,
            ),
            (
                r####"b"####,
                r####"Beta (target)"####,
                r####"5=five"####,
                r####"r:8,9"####,
                r####"F8.2"####,
            ),
            (
                r####"d"####,
                r####"Delta"####,
                r####""####,
                r####""####,
                r####"F8.2"####,
            ),
            (
                r####"c"####,
                r####"Gamma"####,
                r####"x=ex"####,
                r####""####,
                r####"A3"####,
            ),
            (
                r####"f"####,
                r####"Foxtrot"####,
                r####""####,
                r####""####,
                r####"A3"####,
            ),
            (
                r####"g"####,
                r####""####,
                r####""####,
                r####""####,
                r####"F2.0"####,
            ),
        ],
    },
    Case {
        label: r####"NEWVARS also makes the variables the target lacks"####,
        command: r####"APPLY DICTIONARY FROM '{SRC}/src.sav' /NEWVARS."####,
        file: r####"p2.sav"####,
        vars: &[
            (
                r####"a"####,
                r####"Alpha"####,
                r####"1=low;2=high"####,
                r####"n:9"####,
                r####"F8.2"####,
            ),
            (
                r####"b"####,
                r####"Beta (target)"####,
                r####"5=five"####,
                r####"r:8,9"####,
                r####"F8.2"####,
            ),
            (
                r####"d"####,
                r####"Delta"####,
                r####""####,
                r####""####,
                r####"F8.2"####,
            ),
            (
                r####"c"####,
                r####"Gamma"####,
                r####"x=ex"####,
                r####""####,
                r####"A3"####,
            ),
            (
                r####"f"####,
                r####"Foxtrot"####,
                r####""####,
                r####""####,
                r####"A3"####,
            ),
            (
                r####"g"####,
                r####""####,
                r####""####,
                r####""####,
                r####"F2.0"####,
            ),
            (
                r####"e"####,
                r####"Eps"####,
                r####"5=five"####,
                r####""####,
                r####"F8.2"####,
            ),
        ],
    },
    Case {
        label: r####"VARINFO VALLABELS=MERGE joins the value labels, the source winning on a value both label, and applies nothing else (the other items are the default only when VARINFO is not written)"####,
        command: r####"APPLY DICTIONARY FROM '{SRC}/src.sav' /VARINFO VALLABELS=MERGE."####,
        file: r####"p3.sav"####,
        vars: &[
            (
                r####"a"####,
                r####"Old alpha"####,
                r####"1=low;2=high;3=tres"####,
                r####""####,
                r####"F5.0"####,
            ),
            (
                r####"b"####,
                r####"Beta (target)"####,
                r####"5=five"####,
                r####""####,
                r####"F5.0"####,
            ),
            (
                r####"d"####,
                r####""####,
                r####""####,
                r####""####,
                r####"F5.0"####,
            ),
            (
                r####"c"####,
                r####""####,
                r####"x=ex"####,
                r####""####,
                r####"A3"####,
            ),
            (
                r####"f"####,
                r####""####,
                r####""####,
                r####""####,
                r####"A3"####,
            ),
            (
                r####"g"####,
                r####""####,
                r####""####,
                r####""####,
                r####"F2.0"####,
            ),
        ],
    },
    Case {
        label: r####"VARINFO with only MISSING and FORMATS leaves the labels"####,
        command: r####"APPLY DICTIONARY FROM '{SRC}/src.sav' /VARINFO MISSING FORMATS."####,
        file: r####"p5.sav"####,
        vars: &[
            (
                r####"a"####,
                r####"Old alpha"####,
                r####"1=uno;3=tres"####,
                r####"n:9"####,
                r####"F8.2"####,
            ),
            (
                r####"b"####,
                r####"Beta (target)"####,
                r####"5=five"####,
                r####"r:8,9"####,
                r####"F8.2"####,
            ),
            (
                r####"d"####,
                r####""####,
                r####""####,
                r####""####,
                r####"F8.2"####,
            ),
            (
                r####"c"####,
                r####""####,
                r####""####,
                r####""####,
                r####"A3"####,
            ),
            (
                r####"f"####,
                r####""####,
                r####""####,
                r####""####,
                r####"A3"####,
            ),
            (
                r####"g"####,
                r####""####,
                r####""####,
                r####""####,
                r####"F2.0"####,
            ),
        ],
    },
    Case {
        label: r####"one source variable on several target variables of its type"####,
        command: r####"APPLY DICTIONARY FROM '{SRC}/src.sav' /SOURCE VARIABLES=a /TARGET VARIABLES=b d."####,
        file: r####"p4.sav"####,
        vars: &[
            (
                r####"a"####,
                r####"Old alpha"####,
                r####"1=uno;3=tres"####,
                r####""####,
                r####"F5.0"####,
            ),
            (
                r####"b"####,
                r####"Alpha"####,
                r####"1=low;2=high"####,
                r####"n:9"####,
                r####"F8.2"####,
            ),
            (
                r####"d"####,
                r####"Alpha"####,
                r####"1=low;2=high"####,
                r####"n:9"####,
                r####"F8.2"####,
            ),
            (
                r####"c"####,
                r####""####,
                r####""####,
                r####""####,
                r####"A3"####,
            ),
            (
                r####"f"####,
                r####""####,
                r####""####,
                r####""####,
                r####"A3"####,
            ),
            (
                r####"g"####,
                r####""####,
                r####""####,
                r####""####,
                r####"F2.0"####,
            ),
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
fn facts(engine: &Engine) -> Vec<(String, String, String, String, String)> {
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
                v.format().to_string(),
            )
        })
        .collect()
}

fn fixtures() -> String {
    format!("{}/tests/fixtures/apply", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn apply_dictionary_follows_the_model_and_the_file_haven_wrote() {
    let kept = std::env::var_os("APPLY_DIR").map(std::path::PathBuf::from);
    let dir = kept.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("statscraft-apply-{}", std::process::id()))
    });
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = Vec::new();
    for case in CASES {
        let wanted: Vec<(String, String, String, String, String)> = case
            .vars
            .iter()
            .map(|(a, b, c, d, e)| {
                (
                    a.to_string(),
                    b.to_string(),
                    c.to_string(),
                    d.to_string(),
                    e.to_string(),
                )
            })
            .collect();
        let path = dir.join(case.file);
        let _ = std::fs::remove_file(&path);
        let mut engine = Engine::default();
        let source = format!(
            "{TARGET}\n{}\nSAVE OUTFILE='{}'.",
            case.command.replace("{SRC}", &fixtures()),
            path.display()
        );
        if let Err(error) = execute_all(&mut engine, &source) {
            failures.push(format!("{}: error {error}", case.label));
            continue;
        }
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
                    "{}: {what}\n{got:?}\nmodel\n{wanted:?}",
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
