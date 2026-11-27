//! Conditional logic against R on 24 invented rows with system-missing and user-missing values:
//! the value of a logical expression (1, 0 or missing) for relations, arithmetic in relations
//! (0 times a missing value is 0), AND, OR and NOT with the precedence NOT, AND, OR, the
//! three-valued tables of the manual (true OR missing is true, false AND missing is false, the
//! others missing), the functions MISSING, SYSMIS, NMISS, VALUE, ANY and RANGE (missing when the
//! test or all the other arguments are, or when a bound is), strings compared as they are
//! written; IF, which runs only when the condition is true; SELECT IF, which keeps the cases
//! where it is true (two of them must both be true); DO IF, ELSE IF and ELSE, nested too, where a
//! condition that is missing sends the case to END IF, with numbers and strings. A user-missing
//! code is missing in a relation and a valid value to VALUE.
//! What R computes itself: `&`, `|`, `!` and the comparisons (the same three-valued tables as the
//! manual's), arithmetic, and the selection of rows. What follows the manual's rules, written
//! out in the R script because R has no such functions: MISSING, SYSMIS, NMISS, VALUE, ANY and
//! RANGE (including a range with its bounds the wrong way round, which the manual makes
//! missing), 0 times, over and mod a missing value, and the flow of control of DO IF.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    command: &'static str,
    /// The values of each variable after the run: numbers separated by spaces with `.` for
    /// missing, strings separated by `|`.
    columns: &'static [(&'static str, &'static str)],
}

const DATA: &str = r####"DATA LIST LIST /id x y z s t (F3 F3 F3 F3 A3 A3).
BEGIN DATA
1 0 . 1 'a' 'a'
2 1 . 2 'A' 'zz'
3 2 3 . 'b' 'b'
4 3 3 4 'a' ''
5 4 4 5 '' 'zz'
6 5 5 6 'ab' 'a'
7 6 . 7 'b' 'b'
8 7 1 . 'a' 'zz'
9 8 8 9 'A' 'a'
10 9 2 10 ' a' ''
11 10 10 0 'b' 'b'
12 . 4 2 'a' 'a'
13 99 4 3 'ab' 'zz'
14 98 . 4 '' 'a'
15 5 5 . 'A' 'b'
16 5 . 6 'b' ''
17 . 1 7 'a' 'a'
18 3 3 8 'ab' 'zz'
19 7 9 9 'a' 'b'
20 . 9 10 'b' 'a'
21 2 . 11 'A' 'zz'
22 9 2 . 'a' 'b'
23 0 1 0 '' 'a'
24 6 7 5 'b' ''
END DATA.
MISSING VALUES x (98, 99) t ('zz').
"####;

const CASES: &[Case] = &[
    Case {
        label: r####"a logical expression gives 1, 0 or missing"####,
        command: r####"COMPUTE e01 = x > 5.
COMPUTE e02 = x <= y.
COMPUTE e03 = x = y.
COMPUTE e04 = x <> y.
COMPUTE e05 = x EQ 3.
COMPUTE e06 = NOT (x > 5).
COMPUTE e07 = x > 5 AND y < 5.
COMPUTE e08 = x > 5 OR y < 5.
COMPUTE e09 = x > 5 OR y < 5 AND z > 4.
COMPUTE e10 = (x > 5 OR y < 5) AND z > 4.
COMPUTE e11 = NOT x > 5 OR y < 5.
COMPUTE e12 = NOT x > 5 AND NOT y < 5.
COMPUTE e13 = x + y > 8.
COMPUTE e14 = x * y = 0.
COMPUTE e15 = x ** 2 >= 25.
COMPUTE e16 = MISSING(x).
COMPUTE e17 = SYSMIS(x).
COMPUTE e18 = NOT MISSING(y).
COMPUTE e19 = NMISS(x, y, z) = 1.
COMPUTE e20 = VALUE(x) > 50.
COMPUTE e21 = ANY(x, 1, 3, 5).
COMPUTE e22 = ANY(y, x, z).
COMPUTE e23 = RANGE(x, 3, 8).
COMPUTE e24 = RANGE(y, x, z).
COMPUTE e25 = RANGE(x, 0, 2, 7, 9).
COMPUTE e26 = s = 'a'.
COMPUTE e27 = s = 'a '.
COMPUTE e28 = s > 'a'.
COMPUTE e29 = s <> ''.
COMPUTE e30 = ANY(s, 'a', 'b').
COMPUTE e31 = s = 'a' OR x > 7.
COMPUTE e32 = s = 'b' AND y > 3."####,
        columns: &[
            (
                r####"e01"####,
                r####"0 0 0 0 0 0 1 1 1 1 1 . . . 0 0 . 0 1 . 0 1 0 1"####,
            ),
            (
                r####"e02"####,
                r####". . 1 1 1 1 . 0 1 0 1 . . . 1 . . 1 1 . . 0 1 1"####,
            ),
            (
                r####"e03"####,
                r####". . 0 1 1 1 . 0 1 0 1 . . . 1 . . 1 0 . . 0 0 0"####,
            ),
            (
                r####"e04"####,
                r####". . 1 0 0 0 . 1 0 1 0 . . . 0 . . 0 1 . . 1 1 1"####,
            ),
            (
                r####"e05"####,
                r####"0 0 0 1 0 0 0 0 0 0 0 . . . 0 0 . 1 0 . 0 0 0 0"####,
            ),
            (
                r####"e06"####,
                r####"1 1 1 1 1 1 0 0 0 0 0 . . . 1 1 . 1 0 . 1 0 1 0"####,
            ),
            (
                r####"e07"####,
                r####"0 0 0 0 0 0 . 1 0 1 0 . . . 0 0 . 0 0 0 0 1 0 0"####,
            ),
            (
                r####"e08"####,
                r####". . 1 1 1 0 1 1 1 1 1 1 1 . 0 . 1 1 1 . . 1 1 1"####,
            ),
            (
                r####"e09"####,
                r####"0 0 . 0 1 0 1 1 1 1 1 . . . 0 . 1 1 1 . . 1 0 1"####,
            ),
            (
                r####"e10"####,
                r####"0 0 . 0 1 0 1 . 1 1 0 0 0 0 0 . 1 1 1 . . . 0 1"####,
            ),
            (
                r####"e11"####,
                r####"1 1 1 1 1 1 . 1 0 1 0 1 1 . 1 1 1 1 0 . 1 1 1 0"####,
            ),
            (
                r####"e12"####,
                r####". . 0 0 0 1 0 0 0 0 0 0 0 . 1 . 0 0 0 . . 0 0 0"####,
            ),
            (
                r####"e13"####,
                r####". . 0 0 0 1 . 0 1 1 1 . . . 1 . . 0 1 . . 1 0 1"####,
            ),
            (
                r####"e14"####,
                r####"1 . 0 0 0 0 . 0 0 0 0 . . . 0 . . 0 0 . . 0 1 0"####,
            ),
            (
                r####"e15"####,
                r####"0 0 0 0 0 1 1 1 1 1 1 . . . 1 1 . 0 1 . 0 1 0 1"####,
            ),
            (
                r####"e16"####,
                r####"0 0 0 0 0 0 0 0 0 0 0 1 1 1 0 0 1 0 0 1 0 0 0 0"####,
            ),
            (
                r####"e17"####,
                r####"0 0 0 0 0 0 0 0 0 0 0 1 0 0 0 0 1 0 0 1 0 0 0 0"####,
            ),
            (
                r####"e18"####,
                r####"0 0 1 1 1 1 0 1 1 1 1 1 1 0 1 0 1 1 1 1 0 1 1 1"####,
            ),
            (
                r####"e19"####,
                r####"1 1 1 0 0 0 1 1 0 0 0 1 1 0 1 1 1 0 0 1 1 1 0 0"####,
            ),
            (
                r####"e20"####,
                r####"0 0 0 0 0 0 0 0 0 0 0 . 1 1 0 0 . 0 0 . 0 0 0 0"####,
            ),
            (
                r####"e21"####,
                r####"0 1 0 1 0 1 0 0 0 0 0 . . . 1 1 . 1 0 . 0 0 0 0"####,
            ),
            (
                r####"e22"####,
                r####". . 0 1 1 1 . 0 1 0 1 0 0 . 1 . 0 1 1 0 . 0 0 0"####,
            ),
            (
                r####"e23"####,
                r####"0 0 0 1 1 1 1 1 1 0 0 . . . 1 1 . 1 1 . 0 0 0 1"####,
            ),
            (
                r####"e24"####,
                r####". . . 1 1 1 . . 1 0 . . . . . . . 1 1 . . . 0 ."####,
            ),
            (
                r####"e25"####,
                r####"1 1 1 0 0 0 0 1 1 1 0 . . . 0 0 . 0 1 . 1 1 1 0"####,
            ),
            (
                r####"e26"####,
                r####"1 0 0 1 0 0 0 1 0 0 0 1 0 0 0 0 1 0 1 0 0 1 0 0"####,
            ),
            (
                r####"e27"####,
                r####"1 0 0 1 0 0 0 1 0 0 0 1 0 0 0 0 1 0 1 0 0 1 0 0"####,
            ),
            (
                r####"e28"####,
                r####"0 0 1 0 0 1 1 0 0 0 1 0 1 0 0 1 0 1 0 1 0 0 0 1"####,
            ),
            (
                r####"e29"####,
                r####"1 1 1 1 0 1 1 1 1 1 1 1 1 0 1 1 1 1 1 1 1 1 0 1"####,
            ),
            (
                r####"e30"####,
                r####"1 0 1 1 0 0 1 1 0 0 1 1 0 0 0 1 1 0 1 1 0 1 0 1"####,
            ),
            (
                r####"e31"####,
                r####"1 0 0 1 0 0 0 1 1 1 1 1 . . 0 0 1 0 1 . 0 1 0 0"####,
            ),
            (
                r####"e32"####,
                r####"0 0 0 0 0 0 . 0 0 0 1 0 0 0 0 . 0 0 0 1 0 0 0 1"####,
            ),
        ],
    },
    Case {
        label: r####"a user-missing string is valid, a user-missing number is missing, 0 times a missing value is 0"####,
        command: r####"COMPUTE b1 = t <> 'a'.
COMPUTE b2 = t = 'zz'.
COMPUTE b3 = MISSING(t).
COMPUTE b4 = ANY(t, 'zz', 'a').
COMPUTE xc = x.
COMPUTE m1 = x * y.
COMPUTE m2 = x / y.
COMPUTE m3 = y / x.
COMPUTE m4 = MOD(x, y).
COMPUTE m5 = 0 * y.
COMPUTE m6 = 0 / z.
STRING tc (A3).
COMPUTE tc = t.
DO IF t <> 'a'.
COMPUTE r = 1.
END IF."####,
        columns: &[
            (
                r####"b1"####,
                r####"0 1 1 1 1 0 1 1 0 1 1 0 1 0 1 1 0 1 1 0 1 1 0 1"####,
            ),
            (
                r####"b2"####,
                r####"0 1 0 0 1 0 0 1 0 0 0 0 1 0 0 0 0 1 0 0 1 0 0 0"####,
            ),
            (
                r####"b3"####,
                r####"0 1 0 0 1 0 0 1 0 0 0 0 1 0 0 0 0 1 0 0 1 0 0 0"####,
            ),
            (
                r####"b4"####,
                r####"1 1 0 0 1 1 0 1 1 0 0 1 1 1 0 0 1 1 0 1 1 0 1 0"####,
            ),
            (
                r####"xc"####,
                r####"0 1 2 3 4 5 6 7 8 9 10 . . . 5 5 . 3 7 . 2 9 0 6"####,
            ),
            (
                r####"m1"####,
                r####"0 . 6 9 16 25 . 7 64 18 100 . . . 25 . . 9 63 . . 18 0 42"####,
            ),
            (
                r####"m2"####,
                r####"0 . 0.666666666666667 1 1 1 . 7 1 4.5 1 . . . 1 . . 1 0.777777777777778 . . 4.5 0 0.857142857142857"####,
            ),
            (
                r####"m3"####,
                r####". . 1.5 1 1 1 . 0.142857142857143 1 0.222222222222222 1 . . . 1 . . 1 1.28571428571429 . . 0.222222222222222 . 1.16666666666667"####,
            ),
            (
                r####"m4"####,
                r####"0 . 2 0 0 0 . 0 0 1 0 . . . 0 . . 0 7 . . 1 0 6"####,
            ),
            (
                r####"m5"####,
                r####"0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0"####,
            ),
            (
                r####"m6"####,
                r####"0 0 0 0 0 0 0 0 0 0 . 0 0 0 0 0 0 0 0 0 0 0 . 0"####,
            ),
            (
                r####"tc"####,
                r####"a|zz|b||zz|a|b|zz|a||b|a|zz|a|b||a|zz|b|a|zz|b|a|"####,
            ),
            (
                r####"r"####,
                r####". 1 1 1 1 . 1 1 . 1 1 . 1 . 1 1 . 1 1 . 1 1 . 1"####,
            ),
        ],
    },
    Case {
        label: r####"IF runs only when the condition is true"####,
        command: r####"COMPUTE w = 0.
IF (x > 5 AND y < 5) v1 = 1.
IF (x > 5 AND y < 5) w = w + 1.
IF (x > 5 OR y < 5) v2 = 2.
IF (x > 5 OR y < 5) w = w + 10.
IF (NOT (x > 5)) v3 = 3.
IF (NOT (x > 5)) w = w + 100.
IF (ANY(x, 1, 3, 5)) v4 = 4.
IF (ANY(x, 1, 3, 5)) w = w + 1000.
IF (MISSING(y) OR z > 8) v5 = 5.
IF (MISSING(y) OR z > 8) w = w + 10000."####,
        columns: &[
            (
                r####"v1"####,
                r####". . . . . . . 1 . 1 . . . . . . . . . . . 1 . ."####,
            ),
            (
                r####"v2"####,
                r####". . 2 2 2 . 2 2 2 2 2 2 2 . . . 2 2 2 . . 2 2 2"####,
            ),
            (
                r####"v3"####,
                r####"3 3 3 3 3 3 . . . . . . . . 3 3 . 3 . . 3 . 3 ."####,
            ),
            (
                r####"v4"####,
                r####". 4 . 4 . 4 . . . . . . . . 4 4 . 4 . . . . . ."####,
            ),
            (
                r####"v5"####,
                r####"5 5 . . . . 5 . 5 5 . . . 5 . 5 . . 5 5 5 . . ."####,
            ),
            (
                r####"w"####,
                r####"10100 11100 110 1110 110 1100 10010 11 10010 10011 10 10 10 10000 1100 11100 10 1110 10010 10000 10100 11 110 10"####,
            ),
        ],
    },
    Case {
        label: r####"SELECT IF with OR over missing values"####,
        command: r####"SELECT IF (x > 5 OR y < 5)."####,
        columns: &[(
            r####"id"####,
            r####"3 4 5 7 8 9 10 11 12 13 17 18 19 22 23 24"####,
        )],
    },
    Case {
        label: r####"SELECT IF with NOT and OR"####,
        command: r####"SELECT IF (NOT x > 5 OR y < 5)."####,
        columns: &[(
            r####"id"####,
            r####"1 2 3 4 5 6 8 10 12 13 15 16 17 18 21 22 23"####,
        )],
    },
    Case {
        label: r####"SELECT IF with NMISS and VALUE"####,
        command: r####"SELECT IF (NMISS(x, y, z) = 0 OR VALUE(x) > 90)."####,
        columns: &[(r####"id"####, r####"4 5 6 9 10 11 13 14 18 19 23 24"####)],
    },
    Case {
        label: r####"SELECT IF with ANY over variables"####,
        command: r####"SELECT IF (ANY(y, x, z))."####,
        columns: &[(r####"id"####, r####"4 5 6 9 11 15 18 19"####)],
    },
    Case {
        label: r####"SELECT IF with RANGE"####,
        command: r####"SELECT IF (RANGE(x, 2, 6) AND NOT MISSING(z))."####,
        columns: &[(r####"id"####, r####"4 5 6 7 16 18 21 24"####)],
    },
    Case {
        label: r####"two SELECT IF commands must both be true"####,
        command: r####"SELECT IF (x > 2).
SELECT IF (y < 8)."####,
        columns: &[(r####"id"####, r####"4 5 6 8 10 15 18 22 24"####)],
    },
    Case {
        label: r####"SELECT IF on a string"####,
        command: r####"SELECT IF (s = 'a' OR s = 'b')."####,
        columns: &[(r####"id"####, r####"1 3 4 7 8 11 12 16 17 19 20 22 24"####)],
    },
    Case {
        label: r####"DO IF and ELSE"####,
        command: r####"DO IF (x > 5).
COMPUTE r = 1.
ELSE.
COMPUTE r = 2.
END IF."####,
        columns: &[(
            r####"r"####,
            r####"2 2 2 2 2 2 1 1 1 1 1 . . . 2 2 . 2 1 . 2 1 2 1"####,
        )],
    },
    Case {
        label: r####"DO IF with ELSE IF and ELSE"####,
        command: r####"DO IF (x = 0).
COMPUTE r = 10.
ELSE IF (x <= 5).
COMPUTE r = 20.
ELSE IF (y > 4).
COMPUTE r = 30.
ELSE.
COMPUTE r = 40.
END IF."####,
        columns: &[(
            r####"r"####,
            r####"10 20 20 20 20 20 . 40 30 40 30 . . . 20 20 . 20 30 . 20 40 10 30"####,
        )],
    },
    Case {
        label: r####"nested DO IF"####,
        command: r####"DO IF (x > 3).
DO IF (y > 4).
COMPUTE r = 1.
ELSE.
COMPUTE r = 2.
END IF.
ELSE IF (z > 5).
COMPUTE r = 3.
ELSE.
COMPUTE r = 4.
END IF."####,
        columns: &[(
            r####"r"####,
            r####"4 4 . 4 2 1 . 2 1 2 1 . . . 1 . . 3 1 . 3 2 4 1"####,
        )],
    },
    Case {
        label: r####"DO IF into a string, blank when the condition is missing"####,
        command: r####"STRING lab (A9).
DO IF (x = 0).
COMPUTE lab = 'zero'.
ELSE IF (x <= 9).
COMPUTE lab = 'low'.
ELSE.
COMPUTE lab = 'high'.
END IF."####,
        columns: &[(
            r####"lab"####,
            r####"zero|low|low|low|low|low|low|low|low|low|high||||low|low||low|low||low|low|zero|low"####,
        )],
    },
    Case {
        label: r####"DO IF with OR over a missing value"####,
        command: r####"DO IF (x > 5 OR y < 3).
COMPUTE r = 1.
ELSE.
COMPUTE r = 0.
END IF."####,
        columns: &[(
            r####"r"####,
            r####". . 0 0 0 0 1 1 1 1 1 . . . 0 . 1 0 1 . . 1 1 1"####,
        )],
    },
    Case {
        label: r####"DO IF without ELSE leaves the other cases as they are"####,
        command: r####"COMPUTE r = -1.
DO IF (x > 5).
COMPUTE r = 1.
COMPUTE q = r + 1.
END IF."####,
        columns: &[
            (
                r####"r"####,
                r####"-1 -1 -1 -1 -1 -1 1 1 1 1 1 -1 -1 -1 -1 -1 -1 -1 1 -1 -1 1 -1 1"####,
            ),
            (
                r####"q"####,
                r####". . . . . . 2 2 2 2 2 . . . . . . . 2 . . 2 . 2"####,
            ),
        ],
    },
    Case {
        label: r####"DO IF with AND and NOT over missing values"####,
        command: r####"DO IF (NOT MISSING(x) AND y > 2).
COMPUTE r = 1.
ELSE IF (SYSMIS(y)).
COMPUTE r = 2.
ELSE.
COMPUTE r = 3.
END IF."####,
        columns: &[(
            r####"r"####,
            r####". . 1 1 1 1 . 3 1 3 1 3 3 2 1 . 3 1 1 3 . 3 3 1"####,
        )],
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

fn run(case: &Case) -> Result<Engine, String> {
    let source = format!("{DATA}{}\nEXECUTE.", case.command);
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
fn conditional_logic_agrees_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let engine = match run(case) {
            Ok(engine) => engine,
            Err(error) => {
                failures.push(format!("{}: error {error}", case.label));
                continue;
            }
        };
        for (name, want) in case.columns {
            let got = show(&engine, name).unwrap_or_else(|| "(no such variable)".into());
            if !same(&got, want) {
                failures.push(format!("{}: {name} is [{got}], R has [{want}]", case.label));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} column checks failed in {} cases:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
}
