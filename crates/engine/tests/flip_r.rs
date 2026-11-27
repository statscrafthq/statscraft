//! FLIP against R's `t()` on six invented cases with a string variable, system-missing values and a
//! user-missing code: every variable, a subset (VARIABLES), the names from a string variable
//! (upper case) and from a number (K_ and the value), a second FLIP that takes the names from the
//! CASE_LBL of the first and gives the data back, SELECT IF (obeyed) and TEMPORARY (not).
//! What R computes itself: the transposed values (`t()` of the matrix of the variables, a string
//! variable and a user-missing code as NA). What follows the manual's words, written out in the R
//! script: the names (CASE_LBL, VAR001 to VARn, K_, upper case) and what a second FLIP names.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct FlipCase {
    label: &'static str,
    command: &'static str,
    columns: &'static [(&'static str, &'static str)],
}

const FLIP_DATA: &str = r####"DATA LIST LIST /id (F3) name (A5) a b c w.
BEGIN DATA
1 'ann' 5 2.5 . 10
2 'bo' 7 . 3 99
3 'cy' . 1 4 30
4 'di' 2 2 2 99
5 'ed' 9 8 . 50
6 'fy' 1 . 6 60
END DATA.
MISSING VALUES w (99).
"####;

const FLIP_CASES: &[FlipCase] = &[
    FlipCase {
        label: r####"FLIP of every variable: CASE_LBL holds the variable names, a string variable and a user-missing code are system-missing"####,
        command: r####"FLIP."####,
        columns: &[
            (r####"CASE_LBL"####, r####"id|name|a|b|c|w"####),
            (r####"VAR001"####, r####"1 . 5 2.5 . 10"####),
            (r####"VAR002"####, r####"2 . 7 . 3 ."####),
            (r####"VAR003"####, r####"3 . . 1 4 30"####),
            (r####"VAR004"####, r####"4 . 2 2 2 ."####),
            (r####"VAR005"####, r####"5 . 9 8 . 50"####),
            (r####"VAR006"####, r####"6 . 1 . 6 60"####),
        ],
    },
    FlipCase {
        label: r####"FLIP VARIABLES leaves the others out"####,
        command: r####"FLIP VARIABLES=a b c."####,
        columns: &[
            (r####"CASE_LBL"####, r####"a|b|c"####),
            (r####"VAR001"####, r####"5 2.5 ."####),
            (r####"VAR002"####, r####"7 . 3"####),
            (r####"VAR003"####, r####". 1 4"####),
            (r####"VAR004"####, r####"2 2 2"####),
            (r####"VAR005"####, r####"9 8 ."####),
            (r####"VAR006"####, r####"1 . 6"####),
        ],
    },
    FlipCase {
        label: r####"NEWNAMES takes the names from a string variable (upper case) and the variable is no case"####,
        command: r####"FLIP VARIABLES=a b c w /NEWNAMES=name."####,
        columns: &[
            (r####"CASE_LBL"####, r####"a|b|c|w"####),
            (r####"ANN"####, r####"5 2.5 . 10"####),
            (r####"BO"####, r####"7 . 3 ."####),
            (r####"CY"####, r####". 1 4 30"####),
            (r####"DI"####, r####"2 2 2 ."####),
            (r####"ED"####, r####"9 8 . 50"####),
            (r####"FY"####, r####"1 . 6 60"####),
        ],
    },
    FlipCase {
        label: r####"NEWNAMES of a number: the names begin with K_"####,
        command: r####"FLIP VARIABLES=a b c /NEWNAMES=id."####,
        columns: &[
            (r####"CASE_LBL"####, r####"a|b|c"####),
            (r####"K_1"####, r####"5 2.5 ."####),
            (r####"K_2"####, r####"7 . 3"####),
            (r####"K_3"####, r####". 1 4"####),
            (r####"K_4"####, r####"2 2 2"####),
            (r####"K_5"####, r####"9 8 ."####),
            (r####"K_6"####, r####"1 . 6"####),
        ],
    },
    FlipCase {
        label: r####"a second FLIP: the CASE_LBL of the first gives the names and the values come back as they were"####,
        command: r####"FLIP VARIABLES=a b c w /NEWNAMES=name.
FLIP."####,
        columns: &[
            (r####"CASE_LBL"####, r####"ANN|BO|CY|DI|ED|FY"####),
            (r####"a"####, r####"5 7 . 2 9 1"####),
            (r####"b"####, r####"2.5 . 1 2 8 ."####),
            (r####"c"####, r####". 3 4 2 . 6"####),
            (r####"w"####, r####"10 . 30 . 50 60"####),
        ],
    },
    FlipCase {
        label: r####"SELECT IF is obeyed, TEMPORARY is not (the selection stays)"####,
        command: r####"TEMPORARY.
SELECT IF (id > 2).
FLIP VARIABLES=a b c."####,
        columns: &[
            (r####"CASE_LBL"####, r####"a|b|c"####),
            (r####"VAR001"####, r####". 1 4"####),
            (r####"VAR002"####, r####"2 2 2"####),
            (r####"VAR003"####, r####"9 8 ."####),
            (r####"VAR004"####, r####"1 . 6"####),
        ],
    },
    FlipCase {
        label: r####"SELECT IF with OR, then FLIP of two variables"####,
        command: r####"SELECT IF (id = 3 OR id = 6).
FLIP VARIABLES=a c."####,
        columns: &[
            (r####"CASE_LBL"####, r####"a|c"####),
            (r####"VAR001"####, r####". 4"####),
            (r####"VAR002"####, r####"1 6"####),
        ],
    },
];

fn show(engine: &Engine, name: &str) -> Option<String> {
    let data = engine.dataset()?;
    // the manual's names are in capitals, the engine keeps the case it was given
    let found = data
        .variable_names()
        .find(|n| n.eq_ignore_ascii_case(name))?;
    Some(match data.variable(found).ok()?.values() {
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
fn flip_agrees_with_r() {
    let mut failures = Vec::new();
    for case in FLIP_CASES {
        let mut engine = Engine::default();
        if let Err(error) = execute_all(&mut engine, &format!("{FLIP_DATA}{}", case.command)) {
            failures.push(format!("{}: error {error}", case.label));
            continue;
        }
        // the variables of the result are the columns, in order, and no others
        let names: Vec<String> = engine
            .dataset()
            .map(|d| d.variable_names().map(str::to_lowercase).collect())
            .unwrap_or_default();
        let wanted: Vec<String> = case.columns.iter().map(|(n, _)| n.to_lowercase()).collect();
        if names != wanted {
            failures.push(format!(
                "{}: the variables are {names:?}, R has {wanted:?}",
                case.label
            ));
            continue;
        }
        for (name, want) in case.columns {
            let got = show(&engine, name).unwrap_or_else(|| "(no such variable)".into());
            if !same(&got, want) {
                failures.push(format!("{}: {name} is [{got}], R has [{want}]", case.label));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
