//! FILE TYPE MIXED, GROUPED and NESTED against R on invented fixed-width records. R cuts the same lines
//! with `substr` (as `read.fwf` does) and builds the cases by the manual's rules, written out in
//! the R script: mixed, every record of a listed type is a case, the variables of the other types are
//! missing and a record of an unlisted type is skipped (also with the same variable name read from
//! other columns on each type); grouped, the records that follow each other with the same case id
//! are one case, a missing type leaves its variables missing and of a duplicated type the last
//! record is kept; nested, the lowest record builds a case and the values of the higher records
//! are spread to every case under them (SPREAD=NO: to the first only), of two records of one
//! type in a row the last is kept (DUPLICATE=NOWARN) or each builds a case (DUPLICATE=CASE).

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    command: &'static str,
    columns: &'static [(&'static str, &'static str)],
}

const CASES: &[Case] = &[
    Case {
        label: r####"FILE TYPE MIXED: two record types make cases, a third is skipped, the variables of the other type are missing"####,
        command: r####"FILE TYPE MIXED RECORD=rec 1 (A).
RECORD TYPE 'P'.
DATA LIST /name 3-7 (A) age 9-10 score 12-14.
RECORD TYPE 'V'.
DATA LIST /code 3-4 amount 6-9.
END FILE TYPE.

BEGIN DATA
P Ann   34 120
V 07 0450
P Bo    19  95
X zzzzzzzzzz
V 12 1000
V 03 0075
P Cy    61 140
END DATA.
"####,
        columns: &[
            (r####"rec"####, r####"P|V|P|V|V|P"####),
            (r####"name"####, r####"Ann||Bo|||Cy"####),
            (r####"age"####, r####"34 . 19 . . 61"####),
            (r####"score"####, r####"120 . 95 . . 140"####),
            (r####"code"####, r####". 7 . 12 3 ."####),
            (r####"amount"####, r####". 450 . 1000 75 ."####),
        ],
    },
    Case {
        label: r####"FILE TYPE MIXED: the same variable name on two types, read from different columns"####,
        command: r####"FILE TYPE MIXED RECORD=rec 1 (A).
RECORD TYPE 'A'.
DATA LIST /v 3-4 w 6.
RECORD TYPE 'B'.
DATA LIST /v 3 w 5-7.
END FILE TYPE.

BEGIN DATA
A 12 7
B 3 100
A 40 9
B 5 250
B 8   5
END DATA.
"####,
        columns: &[
            (r####"rec"####, r####"A|B|A|B|B"####),
            (r####"v"####, r####"12 3 40 5 8"####),
            (r####"w"####, r####"7 100 9 250 5"####),
        ],
    },
    Case {
        label: r####"FILE TYPE GROUPED: a case is the records of one id, a missing record leaves its variables missing, the last of a duplicated type is kept"####,
        command: r####"FILE TYPE GROUPED RECORD=rt 6 CASE=id 1-4.
RECORD TYPE 1.
DATA LIST /a 8-9.
RECORD TYPE 2.
DATA LIST /b 8-10.
RECORD TYPE 3.
DATA LIST /c 8.
END FILE TYPE.

BEGIN DATA
0001 1 12
0001 2 345
0001 3 6
0002 1 77
0002 3 8
0003 2 90
0003 2 91
0003 1 5
0003 3 4
END DATA.
"####,
        columns: &[
            (r####"id"####, r####"1 2 3"####),
            (r####"a"####, r####"12 77 5"####),
            (r####"b"####, r####"345 . 91"####),
            (r####"c"####, r####"6 8 4"####),
        ],
    },
    Case {
        label: r####"FILE TYPE NESTED: the lowest record builds a case, the higher ones are spread to every case under them"####,
        command: r####"FILE TYPE NESTED RECORD=rt 1 .
RECORD TYPE 1 .
DATA LIST /hid 3-5 city 7-12 (A).
RECORD TYPE 2 .
DATA LIST /vno 3 make 5-8 (A).
RECORD TYPE 3.
DATA LIST /pno 3 age 5-6.
END FILE TYPE.

BEGIN DATA
1 101 Oslo
2 1 Saab
3 1 34
3 2 29
2 2 Volv
3 1 51
1 102 Rome
2 1 Fiat
3 1 40
3 2 11
3 3 8
2 2 Opel
3 1 70
END DATA.
"####,
        columns: &[
            (r####"hid"####, r####"101 101 101 102 102 102 102"####),
            (
                r####"city"####,
                r####"Oslo|Oslo|Oslo|Rome|Rome|Rome|Rome"####,
            ),
            (r####"vno"####, r####"1 1 2 1 1 1 2"####),
            (
                r####"make"####,
                r####"Saab|Saab|Volv|Fiat|Fiat|Fiat|Opel"####,
            ),
            (r####"pno"####, r####"1 2 1 1 2 3 1"####),
            (r####"age"####, r####"34 29 51 40 11 8 70"####),
        ],
    },
    Case {
        label: r####"SPREAD=NO on the top record: its values only in the first case under it"####,
        command: r####"FILE TYPE NESTED RECORD=rt 1 .
RECORD TYPE 1 SPREAD=NO.
DATA LIST /hid 3-5 city 7-12 (A).
RECORD TYPE 2 .
DATA LIST /vno 3 make 5-8 (A).
RECORD TYPE 3.
DATA LIST /pno 3 age 5-6.
END FILE TYPE.

BEGIN DATA
1 101 Oslo
2 1 Saab
3 1 34
3 2 29
2 2 Volv
3 1 51
1 102 Rome
2 1 Fiat
3 1 40
3 2 11
3 3 8
2 2 Opel
3 1 70
END DATA.
"####,
        columns: &[
            (r####"hid"####, r####"101 . . 102 . . ."####),
            (r####"city"####, r####"Oslo|||Rome|||"####),
            (r####"vno"####, r####"1 1 2 1 1 1 2"####),
            (
                r####"make"####,
                r####"Saab|Saab|Volv|Fiat|Fiat|Fiat|Opel"####,
            ),
            (r####"pno"####, r####"1 2 1 1 2 3 1"####),
            (r####"age"####, r####"34 29 51 40 11 8 70"####),
        ],
    },
    Case {
        label: r####"SPREAD=NO on the middle record"####,
        command: r####"FILE TYPE NESTED RECORD=rt 1 .
RECORD TYPE 1 .
DATA LIST /hid 3-5 city 7-12 (A).
RECORD TYPE 2 SPREAD=NO.
DATA LIST /vno 3 make 5-8 (A).
RECORD TYPE 3.
DATA LIST /pno 3 age 5-6.
END FILE TYPE.

BEGIN DATA
1 101 Oslo
2 1 Saab
3 1 34
3 2 29
2 2 Volv
3 1 51
1 102 Rome
2 1 Fiat
3 1 40
3 2 11
3 3 8
2 2 Opel
3 1 70
END DATA.
"####,
        columns: &[
            (r####"hid"####, r####"101 101 101 102 102 102 102"####),
            (
                r####"city"####,
                r####"Oslo|Oslo|Oslo|Rome|Rome|Rome|Rome"####,
            ),
            (r####"vno"####, r####"1 . 2 1 . . 2"####),
            (r####"make"####, r####"Saab||Volv|Fiat|||Opel"####),
            (r####"pno"####, r####"1 2 1 1 2 3 1"####),
            (r####"age"####, r####"34 29 51 40 11 8 70"####),
        ],
    },
    Case {
        label: r####"DUPLICATE=NOWARN: of two vehicle records in a row, the last one is kept"####,
        command: r####"FILE TYPE NESTED RECORD=rt 1 DUPLICATE=NOWARN.
RECORD TYPE 1 .
DATA LIST /hid 3-5 city 7-12 (A).
RECORD TYPE 2 .
DATA LIST /vno 3 make 5-8 (A).
RECORD TYPE 3.
DATA LIST /pno 3 age 5-6.
END FILE TYPE.

BEGIN DATA
1 201 Bern
2 1 Audi
3 1 22
2 2 Mini
2 3 Ford
3 1 31
3 2 58
END DATA.
"####,
        columns: &[
            (r####"hid"####, r####"201 201 201"####),
            (r####"vno"####, r####"1 3 3"####),
            (r####"make"####, r####"Audi|Ford|Ford"####),
            (r####"pno"####, r####"1 1 2"####),
            (r####"age"####, r####"22 31 58"####),
        ],
    },
    Case {
        label: r####"DUPLICATE=CASE: a case for the vehicle without people, with the higher values spread"####,
        command: r####"FILE TYPE NESTED RECORD=rt 1 DUPLICATE=CASE.
RECORD TYPE 1 .
DATA LIST /hid 3-5 city 7-12 (A).
RECORD TYPE 2 .
DATA LIST /vno 3 make 5-8 (A).
RECORD TYPE 3.
DATA LIST /pno 3 age 5-6.
END FILE TYPE.

BEGIN DATA
1 201 Bern
2 1 Audi
3 1 22
2 2 Mini
2 3 Ford
3 1 31
3 2 58
END DATA.
"####,
        columns: &[
            (r####"hid"####, r####"201 201 201 201"####),
            (r####"vno"####, r####"1 2 3 3"####),
            (r####"make"####, r####"Audi|Mini|Ford|Ford"####),
            (r####"pno"####, r####"1 . 1 2"####),
            (r####"age"####, r####"22 . 31 58"####),
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

#[test]
fn complex_files_agree_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let mut engine = Engine::default();
        let (commands, errors) = parse(&format!("{}\nEXECUTE.", case.command));
        if !errors.is_empty() {
            failures.push(format!("{}: {errors:?}", case.label));
            continue;
        }
        let mut failed = false;
        for command in &commands {
            if let Err(error) = engine.execute(&command.command) {
                failures.push(format!(
                    "{}: line {}: {error}",
                    case.label, command.first_line
                ));
                failed = true;
                break;
            }
        }
        if failed {
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
