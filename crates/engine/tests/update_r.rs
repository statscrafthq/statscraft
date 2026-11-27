//! UPDATE against R on invented files: a primary file with unique keys and transaction files with
//! duplicate keys, new keys and new variables, a user-missing code and two key variables.
//! R (dplyr: `group_by`, `summarise`, `full_join`, `coalesce`) works the result out from the manual's
//! rules, written out in the R script: a transaction value replaces the primary's only when it is
//! not missing (system-missing, a user-missing code, or blank for a string); the duplicates of a
//! transaction file are applied in order, so the last non-missing value of each variable wins; of
//! several transaction files the last with a value wins; a key only in a transaction file adds a
//! case; variables only in a transaction file are added, missing for the other cases; the result
//! is in the order of the keys; IN is 1 where the file holds the key.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    setup: &'static str,
    command: &'static str,
    /// Names that must appear in this order (not necessarily next to each other).
    order: &'static [&'static str],
    /// Every variable the result has, in any order.
    variables: &'static [&'static str],
    /// The values of each variable: numbers separated by spaces with `.` for missing, strings
    /// separated by `|`.
    columns: &'static [(&'static str, &'static str)],
}

const SETUP: &str = r####"DATA LIST LIST /k (F8) a (F8) b (F8) s (A4) c (F8.1).
BEGIN DATA
1 10 100 'aa' 1.5
2 20 . 'bb' .
4 . 400 '' 4.5
5 50 500 'ee' 5.5
8 80 800 'hh' .
END DATA.
DATASET NAME pm.
DATA LIST LIST /k (F8) a (F8) s (A4) d (F8).
BEGIN DATA
1 . 'x' 7
2 21 '' .
2 . 'y' 8
3 31 'z' 9
4 41 '' .
4 42 'w' .
6 . '' 66
8 88 'hh2' .
END DATA.
DATASET NAME t1.
DATA LIST LIST /k (F8) a (F8) b (F8) d (F8) e (F8).
BEGIN DATA
2 22 . . 1
3 . 333 99 2
5 55 . . .
9 99 999 9 9
END DATA.
DATASET NAME t2.
DATA LIST LIST /k (F8) u (F8) v (F8).
BEGIN DATA
1 1 10
2 99 20
3 3 30
4 99 40
5 5 50
END DATA.
DATASET NAME pu.
MISSING VALUES u v (99).
DATA LIST LIST /k (F8) u (F8) v (F8).
BEGIN DATA
1 99 .
2 7 99
3 . 31
3 99 .
6 99 60
END DATA.
DATASET NAME tu.
MISSING VALUES u v (99).
DATA LIST LIST /k1 (F8) k2 (A2) v (F8) w (F8).
BEGIN DATA
1 'a' 1 10
1 'b' 2 .
2 'a' 3 30
3 'c' 4 40
END DATA.
DATASET NAME pk.
DATA LIST LIST /k1 (F8) k2 (A2) v (F8) x (F8.1).
BEGIN DATA
1 'a' . 0.5
1 'a' 11 .
1 'b' 22 2.5
2 'a' . 3.5
2 'b' 5 4.5
4 'a' 6 5.5
END DATA.
DATASET NAME tk.
"####;

const CASES: &[Case] = &[
    Case {
        label: r####"one transaction file: duplicate keys apply in order, a missing value or blank does not overwrite, a new key adds a case, a new variable is added"####,
        setup: SETUP,
        command: r####"UPDATE FILE=pm /FILE=t1 /BY k."####,
        order: &[
            r####"k"####,
            r####"a"####,
            r####"b"####,
            r####"s"####,
            r####"c"####,
            r####"d"####,
        ],
        variables: &[
            r####"k"####,
            r####"a"####,
            r####"b"####,
            r####"s"####,
            r####"c"####,
            r####"d"####,
        ],
        columns: &[
            (r####"k"####, r####"1 2 3 4 5 6 8"####),
            (r####"a"####, r####"10 21 31 42 50 . 88"####),
            (r####"b"####, r####"100 . . 400 500 . 800"####),
            (r####"s"####, r####"x|y|z|w|ee||hh2"####),
            (r####"c"####, r####"1.5 . . 4.5 5.5 . ."####),
            (r####"d"####, r####"7 8 9 . . 66 ."####),
        ],
    },
    Case {
        label: r####"two transaction files: the last file with a value wins"####,
        setup: SETUP,
        command: r####"UPDATE FILE=pm /FILE=t1 /FILE=t2 /BY k."####,
        order: &[
            r####"k"####,
            r####"a"####,
            r####"b"####,
            r####"s"####,
            r####"c"####,
            r####"d"####,
            r####"e"####,
        ],
        variables: &[
            r####"k"####,
            r####"a"####,
            r####"b"####,
            r####"s"####,
            r####"c"####,
            r####"d"####,
            r####"e"####,
        ],
        columns: &[
            (r####"k"####, r####"1 2 3 4 5 6 8 9"####),
            (r####"a"####, r####"10 22 31 42 55 . 88 99"####),
            (r####"b"####, r####"100 . 333 400 500 . 800 999"####),
            (r####"s"####, r####"x|y|z|w|ee||hh2|"####),
            (r####"c"####, r####"1.5 . . 4.5 5.5 . . ."####),
            (r####"d"####, r####"7 8 99 . . 66 . 9"####),
            (r####"e"####, r####". 1 2 . . . . 9"####),
        ],
    },
    Case {
        label: r####"the second transaction file alone"####,
        setup: SETUP,
        command: r####"UPDATE FILE=pm /FILE=t2 /BY k."####,
        order: &[
            r####"k"####,
            r####"a"####,
            r####"b"####,
            r####"s"####,
            r####"c"####,
            r####"d"####,
            r####"e"####,
        ],
        variables: &[
            r####"k"####,
            r####"a"####,
            r####"b"####,
            r####"s"####,
            r####"c"####,
            r####"d"####,
            r####"e"####,
        ],
        columns: &[
            (r####"k"####, r####"1 2 3 4 5 8 9"####),
            (r####"a"####, r####"10 22 . . 55 80 99"####),
            (r####"b"####, r####"100 . 333 400 500 800 999"####),
            (r####"s"####, r####"aa|bb|||ee|hh|"####),
            (r####"c"####, r####"1.5 . . 4.5 5.5 . ."####),
            (r####"d"####, r####". . 99 . . . 9"####),
            (r####"e"####, r####". 1 2 . . . 9"####),
        ],
    },
    Case {
        label: r####"IN on the primary file and on each transaction file"####,
        setup: SETUP,
        command: r####"UPDATE FILE=pm /IN=inm /FILE=t1 /IN=in1 /FILE=t2 /IN=in2 /BY k."####,
        order: &[
            r####"k"####,
            r####"a"####,
            r####"b"####,
            r####"s"####,
            r####"c"####,
            r####"d"####,
            r####"e"####,
            r####"inm"####,
            r####"in1"####,
            r####"in2"####,
        ],
        variables: &[
            r####"k"####,
            r####"a"####,
            r####"b"####,
            r####"s"####,
            r####"c"####,
            r####"d"####,
            r####"e"####,
            r####"inm"####,
            r####"in1"####,
            r####"in2"####,
        ],
        columns: &[
            (r####"k"####, r####"1 2 3 4 5 6 8 9"####),
            (r####"a"####, r####"10 22 31 42 55 . 88 99"####),
            (r####"b"####, r####"100 . 333 400 500 . 800 999"####),
            (r####"s"####, r####"x|y|z|w|ee||hh2|"####),
            (r####"c"####, r####"1.5 . . 4.5 5.5 . . ."####),
            (r####"d"####, r####"7 8 99 . . 66 . 9"####),
            (r####"e"####, r####". 1 2 . . . . 9"####),
            (r####"inm"####, r####"1 1 0 1 1 0 1 0"####),
            (r####"in1"####, r####"1 1 1 1 0 1 1 0"####),
            (r####"in2"####, r####"0 1 1 0 1 0 0 1"####),
        ],
    },
    Case {
        label: r####"RENAME of a transaction variable to the name of a primary variable updates it"####,
        setup: SETUP,
        command: r####"UPDATE FILE=pm /FILE=t2 /RENAME=(e = c) /BY k."####,
        order: &[
            r####"k"####,
            r####"a"####,
            r####"b"####,
            r####"s"####,
            r####"c"####,
            r####"d"####,
        ],
        variables: &[
            r####"k"####,
            r####"a"####,
            r####"b"####,
            r####"s"####,
            r####"c"####,
            r####"d"####,
        ],
        columns: &[
            (r####"k"####, r####"1 2 3 4 5 8 9"####),
            (r####"a"####, r####"10 22 . . 55 80 99"####),
            (r####"b"####, r####"100 . 333 400 500 800 999"####),
            (r####"s"####, r####"aa|bb|||ee|hh|"####),
            (r####"c"####, r####"1.5 1 2 4.5 5.5 . 9"####),
            (r####"d"####, r####". . 99 . . . 9"####),
        ],
    },
    Case {
        label: r####"KEEP chooses and orders the variables"####,
        setup: SETUP,
        command: r####"UPDATE FILE=pm /FILE=t1 /FILE=t2 /BY k /KEEP=k s a d."####,
        order: &[r####"k"####, r####"s"####, r####"a"####, r####"d"####],
        variables: &[r####"k"####, r####"s"####, r####"a"####, r####"d"####],
        columns: &[
            (r####"k"####, r####"1 2 3 4 5 6 8 9"####),
            (r####"s"####, r####"x|y|z|w|ee||hh2|"####),
            (r####"a"####, r####"10 22 31 42 55 . 88 99"####),
            (r####"d"####, r####"7 8 99 . . 66 . 9"####),
        ],
    },
    Case {
        label: r####"DROP leaves variables out"####,
        setup: SETUP,
        command: r####"UPDATE FILE=pm /FILE=t1 /BY k /DROP=b c."####,
        order: &[r####"k"####, r####"a"####, r####"s"####, r####"d"####],
        variables: &[r####"k"####, r####"a"####, r####"s"####, r####"d"####],
        columns: &[
            (r####"k"####, r####"1 2 3 4 5 6 8"####),
            (r####"a"####, r####"10 21 31 42 50 . 88"####),
            (r####"s"####, r####"x|y|z|w|ee||hh2"####),
            (r####"d"####, r####"7 8 9 . . 66 ."####),
        ],
    },
    Case {
        label: r####"a user-missing code does not overwrite and is overwritten"####,
        setup: SETUP,
        command: r####"UPDATE FILE=pu /FILE=tu /BY k."####,
        order: &[r####"k"####, r####"u"####, r####"v"####],
        variables: &[r####"k"####, r####"u"####, r####"v"####],
        columns: &[
            (r####"k"####, r####"1 2 3 4 5 6"####),
            (r####"u"####, r####"1 7 3 99 5 ."####),
            (r####"v"####, r####"10 20 31 40 50 60"####),
        ],
    },
    Case {
        label: r####"two key variables, a number and a string"####,
        setup: SETUP,
        command: r####"UPDATE FILE=pk /FILE=tk /BY k1 k2."####,
        order: &[
            r####"k1"####,
            r####"k2"####,
            r####"v"####,
            r####"w"####,
            r####"x"####,
        ],
        variables: &[
            r####"k1"####,
            r####"k2"####,
            r####"v"####,
            r####"w"####,
            r####"x"####,
        ],
        columns: &[
            (r####"k1"####, r####"1 1 2 2 3 4"####),
            (r####"k2"####, r####"a|b|a|b|c|a"####),
            (r####"v"####, r####"11 22 3 5 4 6"####),
            (r####"w"####, r####"10 . 30 . 40 ."####),
            (r####"x"####, r####"0.5 2.5 3.5 4.5 . 5.5"####),
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
        Values::String(values) => values.iter().collect::<Vec<_>>().join("|"),
    })
}

fn run(case: &Case) -> Result<Engine, String> {
    let source = format!("{}{}", case.setup, case.command);
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
fn update_agrees_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let engine = match run(case) {
            Ok(engine) => engine,
            Err(error) => {
                failures.push(format!("{}: error {error}", case.label));
                continue;
            }
        };
        let names: Vec<String> = engine
            .dataset()
            .unwrap()
            .variable_names()
            .map(str::to_string)
            .collect();
        let mut got = names.clone();
        got.sort();
        let mut want: Vec<&str> = case.variables.to_vec();
        want.sort();
        if got != want {
            failures.push(format!(
                "{}: variables {} (wanted {})",
                case.label,
                names.join(" "),
                case.variables.join(" ")
            ));
            continue;
        }
        let position = |name: &str| names.iter().position(|n| n == name).unwrap();
        let positions: Vec<usize> = case.order.iter().map(|n| position(n)).collect();
        if positions.windows(2).any(|w| w[0] >= w[1]) {
            failures.push(format!(
                "{}: order {} (wanted {})",
                case.label,
                names.join(" "),
                case.order.join(" ")
            ));
        }
        for (name, want) in case.columns {
            let got = show(&engine, name).expect("the variable exists");
            if got != *want {
                failures.push(format!("{}: {name} is [{got}], R has [{want}]", case.label));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
}
