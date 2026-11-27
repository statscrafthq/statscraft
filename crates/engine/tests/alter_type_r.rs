//! ALTER TYPE against R on ten invented cases: a number becomes text as its print format writes it (the
//! digits of the format, left in the width, cut when too long), text becomes a number as R's `as.numeric`
//! reads it once the blanks are trimmed (anything else is system-missing), AMIN is the longest value in
//! bytes (a letter with an accent counts two), plus n bytes or n percent rounded, an input format limits the
//! change to the variables it fits, the last mention of a variable wins, and TO and ALL lists.
//! What R computes itself: the numbers (`as.numeric`), the text of a number (`sprintf`), the
//! widths (`nchar` in bytes, `round`). What follows the manual, written out in the R script: the
//! rules of the command itself (which variables a list and an input format pick).

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    command: &'static str,
    columns: &'static [(&'static str, &'static str)],
    /// The format of each variable afterwards.
    formats: &'static [(&'static str, &'static str)],
}

const DATA: &str = r####"DATA LIST LIST /n1 (F8.2) n2 (F5) s1 (A8) s2 (A10).
BEGIN DATA
12.5 1 '12' 'a'
-3.25 22 ' 7' 'bb'
0 333 'abc' 'ccc'
1234.5 . '' ''
. 5 '3.5' 'dddd'
7 66 '1e3' 'e'
100.125 7 '-4' 'ff'
0.5 88 '0012' 'ggg'
99999.99 9 '12abc' 'hhéh'
-0.04 1000 '+8' 'i'
END DATA.
"####;

const CASES: &[Case] = &[
    Case {
        label: r####"a number becomes text as its print format writes it: left in the width, the digits of the format"####,
        command: r####"ALTER TYPE n1 (A8) n2 (A6)."####,
        columns: &[
            (
                r####"n1"####,
                r####"12.50|-3.25|.00|1234.50||7.00|100.13|.50|99999.99|-.04"####,
            ),
            (r####"n2"####, r####"1|22|333||5|66|7|88|9|1000"####),
        ],
        formats: &[
            (r####"n1"####, r####"A8"####),
            (r####"n2"####, r####"A6"####),
        ],
    },
    Case {
        label: r####"a number that does not fit the width is cut"####,
        command: r####"ALTER TYPE n1 (A4)."####,
        columns: &[(
            r####"n1"####,
            r####"12.5|-3.2|.00|1234||7.00|100.|.50|9999|-.04"####,
        )],
        formats: &[(r####"n1"####, r####"A4"####)],
    },
    Case {
        label: r####"text becomes a number: blanks trimmed, anything but a number is system-missing"####,
        command: r####"ALTER TYPE s1 (F8.2)."####,
        columns: &[(r####"s1"####, r####"12 7 . . 3.5 1000 -4 12 . 8"####)],
        formats: &[(r####"s1"####, r####"F8.2"####)],
    },
    Case {
        label: r####"a format with decimals and a wider width"####,
        command: r####"ALTER TYPE s1 (F12.4) n2 (F9.3)."####,
        columns: &[
            (r####"s1"####, r####"12 7 . . 3.5 1000 -4 12 . 8"####),
            (r####"n2"####, r####"1 22 333 . 5 66 7 88 9 1000"####),
        ],
        formats: &[
            (r####"s1"####, r####"F12.4"####),
            (r####"n2"####, r####"F9.3"####),
        ],
    },
    Case {
        label: r####"AMIN is the longest value in bytes"####,
        command: r####"ALTER TYPE s2 (A=AMIN)."####,
        columns: &[(r####"s2"####, r####"a|bb|ccc||dddd|e|ff|ggg|hhéh|i"####)],
        formats: &[(r####"s2"####, r####"A5"####)],
    },
    Case {
        label: r####"AMIN + 3 bytes"####,
        command: r####"ALTER TYPE s2 (A=AMIN + 3)."####,
        columns: &[(r####"s2"####, r####"a|bb|ccc||dddd|e|ff|ggg|hhéh|i"####)],
        formats: &[(r####"s2"####, r####"A8"####)],
    },
    Case {
        label: r####"AMIN + 50 percent is rounded"####,
        command: r####"ALTER TYPE s2 (A=AMIN + 50%)."####,
        columns: &[(r####"s2"####, r####"a|bb|ccc||dddd|e|ff|ggg|hhéh|i"####)],
        formats: &[(r####"s2"####, r####"A8"####)],
    },
    Case {
        label: r####"an input format limits the change to the variables it fits (A8 only, not A10)"####,
        command: r####"ALTER TYPE s1 s2 (A8 = A3)."####,
        columns: &[
            (r####"s1"####, r####"12| 7|abc||3.5|1e3|-4|001|12a|+8"####),
            (r####"s2"####, r####"a|bb|ccc||dddd|e|ff|ggg|hhéh|i"####),
        ],
        formats: &[
            (r####"s1"####, r####"A3"####),
            (r####"s2"####, r####"A10"####),
        ],
    },
    Case {
        label: r####"the last mention of a variable wins"####,
        command: r####"ALTER TYPE s1 (A6) s2 (A5) s1 (A3)."####,
        columns: &[
            (r####"s1"####, r####"12| 7|abc||3.5|1e3|-4|001|12a|+8"####),
            (r####"s2"####, r####"a|bb|ccc||dddd|e|ff|ggg|hhéh|i"####),
        ],
        formats: &[
            (r####"s1"####, r####"A3"####),
            (r####"s2"####, r####"A5"####),
        ],
    },
    Case {
        label: r####"TO lists and the format without a width of the input"####,
        command: r####"ALTER TYPE n1 TO n2 (F10.1)."####,
        columns: &[
            (
                r####"n1"####,
                r####"12.5 -3.25 0 1234.5 . 7 100.125 0.5 99999.99 -0.04"####,
            ),
            (r####"n2"####, r####"1 22 333 . 5 66 7 88 9 1000"####),
        ],
        formats: &[
            (r####"n1"####, r####"F10.1"####),
            (r####"n2"####, r####"F10.1"####),
        ],
    },
    Case {
        label: r####"an input format without a width takes every variable of that type: numbers to text"####,
        command: r####"ALTER TYPE ALL (F = A12)."####,
        columns: &[
            (
                r####"n1"####,
                r####"12.50|-3.25|.00|1234.50||7.00|100.13|.50|99999.99|-.04"####,
            ),
            (r####"n2"####, r####"1|22|333||5|66|7|88|9|1000"####),
            (
                r####"s1"####,
                r####"12| 7|abc||3.5|1e3|-4|0012|12abc|+8"####,
            ),
            (r####"s2"####, r####"a|bb|ccc||dddd|e|ff|ggg|hhéh|i"####),
        ],
        formats: &[
            (r####"n1"####, r####"A12"####),
            (r####"n2"####, r####"A12"####),
            (r####"s1"####, r####"A8"####),
            (r####"s2"####, r####"A10"####),
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
fn alter_type_agrees_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let mut engine = Engine::default();
        if let Err(error) = execute_all(&mut engine, &format!("{DATA}{}\nEXECUTE.", case.command)) {
            failures.push(format!("{}: error {error}", case.label));
            continue;
        }
        for (name, want) in case.columns {
            let got = show(&engine, name).unwrap_or_else(|| "(no such variable)".into());
            if !same(&got, want) {
                failures.push(format!("{}: {name} is [{got}], R has [{want}]", case.label));
            }
        }
        let data = engine.dataset().unwrap();
        for (name, want) in case.formats {
            let got = data
                .variable(name)
                .map(|v| v.format().to_string())
                .unwrap_or_default();
            if got != *want {
                failures.push(format!(
                    "{}: the format of {name} is {got}, wanted {want}",
                    case.label
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
