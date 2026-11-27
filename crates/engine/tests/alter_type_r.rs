//! ALTER TYPE against R on ten invented rows in twelve runs: a number becomes text as its print format writes it (the
//! digits of the format, left in the width, cut when too long), text becomes a number as R's `as.numeric`
//! reads it once the blanks are trimmed (anything else is system-missing), AMIN is the longest value in
//! bytes (a letter with an accent counts two), plus n bytes or n percent rounded, an input format limits the
//! change to the variables it fits, the last mention of a variable wins, and TO and ALL lists.
//! What R computes itself: the numbers (`as.numeric`), the text of a number (`sprintf`, after the
//! guide's rounding of a half away from zero is done), the widths (`nchar` in bytes). A half of a
//! percent width goes up (the guide only says rounded to an integer). What follows the manual, written out in the R script: the
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
        label: r####"AMIN + 30 percent: 6.5 goes up to 7"####,
        command: r####"ALTER TYPE s2 (A=AMIN + 30%)."####,
        columns: &[(r####"s2"####, r####"a|bb|ccc||dddd|e|ff|ggg|hhéh|i"####)],
        formats: &[(r####"s2"####, r####"A7"####)],
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
        label: r####"a TO list"####,
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

const PRINT_VALUES: &[(&str, usize, &str, &str)] = &[
    (r####"n1"####, 1, r####"12.50"####, r####"12.50"####),
    (r####"n1"####, 2, r####"-3.25"####, r####"-3.25"####),
    (r####"n1"####, 3, r####".00"####, r####".00"####),
    (r####"n1"####, 4, r####"1234.50"####, r####"1234.50"####),
    (r####"n1"####, 5, r####"."####, r####""####),
    (r####"n1"####, 6, r####"7.00"####, r####"7.00"####),
    (r####"n1"####, 7, r####"100.13"####, r####"100.13"####),
    (r####"n1"####, 8, r####".50"####, r####".50"####),
    (r####"n1"####, 9, r####"99999.99"####, r####"99999.99"####),
    (r####"n1"####, 10, r####"-.04"####, r####"-.04"####),
    (r####"n2"####, 1, r####"1"####, r####"1"####),
    (r####"n2"####, 2, r####"22"####, r####"22"####),
    (r####"n2"####, 3, r####"333"####, r####"333"####),
    (r####"n2"####, 4, r####"."####, r####""####),
    (r####"n2"####, 5, r####"5"####, r####"5"####),
    (r####"n2"####, 6, r####"66"####, r####"66"####),
    (r####"n2"####, 7, r####"7"####, r####"7"####),
    (r####"n2"####, 8, r####"88"####, r####"88"####),
    (r####"n2"####, 9, r####"9"####, r####"9"####),
    (r####"n2"####, 10, r####"1000"####, r####"1000"####),
    (r####"s1"####, 1, r####"12"####, r####"12.00"####),
    (r####"s1"####, 2, r####" 7"####, r####"7.00"####),
    (r####"s1"####, 3, r####"abc"####, r####"."####),
    (r####"s1"####, 4, r####""####, r####"."####),
    (r####"s1"####, 5, r####"3.5"####, r####"3.50"####),
    (r####"s1"####, 6, r####"1e3"####, r####"1000.00"####),
    (r####"s1"####, 7, r####"-4"####, r####"-4.00"####),
    (r####"s1"####, 8, r####"0012"####, r####"12.00"####),
    (r####"s1"####, 9, r####"12abc"####, r####"."####),
    (r####"s1"####, 10, r####"+8"####, r####"8.00"####),
    (r####"s2"####, 5, r####"dddd"####, r####"ddd"####),
    (r####"s2"####, 9, r####"hhéh"####, r####"hh"####),
];

fn tables_of(source: &str) -> Result<Vec<statscraft_engine::Table>, String> {
    let (commands, errors) = parse(source);
    if !errors.is_empty() {
        return Err(format!("{errors:?}"));
    }
    let mut engine = Engine::default();
    let mut last = Vec::new();
    for command in &commands {
        last = engine
            .execute(&command.command)
            .map_err(|e| format!("line {}: {e}", command.first_line))?;
    }
    Ok(last)
}

fn text(cell: &statscraft_engine::Cell) -> String {
    match cell {
        statscraft_engine::Cell::Text(t) => t.clone(),
        statscraft_engine::Cell::Number(n) => n.to_string(),
        statscraft_engine::Cell::Empty => String::new(),
    }
}

#[test]
fn the_report_of_altered_values_agrees_with_r() {
    let source =
        format!("{DATA}ALTER TYPE n1 (A8) n2 (A6) s1 (F8.2) s2 (A3) /PRINT=ALTEREDVALUES.");
    let tables = tables_of(&source).unwrap();
    // ALTEREDVALUES alone: no list of formats
    assert_eq!(tables.len(), 1);
    let table = &tables[0];
    assert_eq!(table.title, "Altered Values");
    assert_eq!(
        table.columns,
        ["Variable", "Case Number", "Old Value", "New Value"]
    );
    let got: Vec<[String; 4]> = table
        .rows
        .iter()
        .map(|r| [text(&r[0]), text(&r[1]), text(&r[2]), text(&r[3])])
        .collect();
    let want: Vec<[String; 4]> = PRINT_VALUES
        .iter()
        .map(|(v, i, o, n)| [v.to_string(), i.to_string(), o.to_string(), n.to_string()])
        .collect();
    assert_eq!(got, want);
}

#[test]
fn the_report_of_altered_types_is_the_default_and_none_turns_it_off() {
    let change = "ALTER TYPE n1 (A8) n2 (A6) s1 (F8.2) s2 (A3) s2 (A10 = A3)";
    let tables = tables_of(&format!("{DATA}{change}.")).unwrap();
    assert_eq!(tables.len(), 1);
    assert_eq!(tables[0].title, "Altered Types");
    assert_eq!(tables[0].columns, ["Variable", "Old Format", "New Format"]);
    let rows: Vec<Vec<String>> = tables[0]
        .rows
        .iter()
        .map(|r| r.iter().map(text).collect())
        .collect();
    // the format of the data as DATA LIST gives it: room for the decimal point
    let expect = [
        ["n1", "F9.2", "A8"],
        ["n2", "F5.0", "A6"],
        ["s1", "A8", "F8.2"],
        ["s2", "A10", "A3"],
    ];
    assert_eq!(rows, expect.map(|r| r.map(String::from).to_vec()));
    // both reports in the order of the guide's syntax chart
    let both = tables_of(&format!(
        "{DATA}{change} /PRINT=ALTEREDTYPES ALTEREDVALUES."
    ))
    .unwrap();
    assert_eq!(
        both.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
        ["Altered Types", "Altered Values"]
    );
    // NONE: no table; NONE with another keyword is refused; a variable the change leaves as it was is not listed
    assert!(
        tables_of(&format!("{DATA}{change} /PRINT=NONE."))
            .unwrap()
            .is_empty()
    );
    assert!(tables_of(&format!("{DATA}{change} /PRINT=NONE ALTEREDVALUES.")).is_err());
    let same = tables_of(&format!("{DATA}ALTER TYPE s2 (A10).")).unwrap();
    assert!(same.is_empty());
}

#[test]
fn the_report_of_values_holds_the_first_25_of_each_variable() {
    let rows: String = (1..=30).map(|i| format!("{i}\n")).collect();
    let source = format!(
        "DATA LIST LIST /n (F3).\nBEGIN DATA\n{rows}END DATA.\nALTER TYPE n (A5) /PRINT=ALTEREDVALUES."
    );
    let tables = tables_of(&source).unwrap();
    let table = &tables[0];
    assert_eq!(table.rows.len(), 25);
    assert_eq!(text(&table.rows[0][1]), "1");
    assert_eq!(text(&table.rows[24][1]), "25");
}

#[test]
fn a_format_that_stays_is_not_reported_and_an_empty_print_is_refused() {
    // n2 is F5.0 already: ALTER TYPE n2 (F5) changes nothing and lists nothing
    assert!(
        tables_of(&format!("{DATA}ALTER TYPE n2 (F5)."))
            .unwrap()
            .is_empty()
    );
    let error = tables_of(&format!("{DATA}ALTER TYPE n2 (F6) /PRINT=.")).unwrap_err();
    assert!(error.contains("ALTEREDTYPES"), "{error}");
}
