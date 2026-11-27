//! The warnings and ORDERED=NO of FILE TYPE GROUPED and NESTED against R on invented records. R cuts the
//! lines with `substr` and applies the rules of the guide, written out in the R script: the cases (by
//! case id for grouped, by the records above for nested; the last of a duplicated record is kept) and how
//! many warnings of each kind there are. Grouped, every case without a record of a defined type, every
//! second record of a type in one case, every record that comes after a later type of its case
//! (ORDERED=YES) and every record of an undefined type is one warning (MISSING, DUPLICATE, ORDERED and
//! WILD turn them off); nested, every record of the lowest type with no record of a higher type above it
//! (MISSING=WARN) and every second record of a higher type read again before any record under the first
//! (DUPLICATE=WARN) is one, both off by default. The cases are the same with the warnings on or off.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    command: &'static str,
    columns: &'static [(&'static str, &'static str)],
    /// A piece of the message and how many warnings have it.
    warnings: &'static [(&'static str, usize)],
}

const CASES: &[Case] = &[
    Case {
        label: r####"GROUPED with every default: missing records, a second record, an out-of-order record and an undefined type are each warned"####,
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
0002 2 77
0002 1 8
0002 3 9
0003 1 5
0004 1 3
0004 2 40
0004 2 41
0004 3 7
0005 9 99
0005 3 2
END DATA.
"####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5"####),
            (r####"a"####, r####"12 8 5 3 ."####),
            (r####"b"####, r####"345 77 . 41 ."####),
            (r####"c"####, r####"6 9 . 7 2"####),
        ],
        warnings: &[
            (r####"has no record of type"####, 4),
            (r####"second record of this type"####, 1),
            (r####"out of the order"####, 1),
            (r####"is not defined"####, 1),
        ],
    },
    Case {
        label: r####"GROUPED with MISSING=NOWARN"####,
        command: r####"FILE TYPE GROUPED RECORD=rt 6 CASE=id 1-4 MISSING=NOWARN.
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
0002 2 77
0002 1 8
0002 3 9
0003 1 5
0004 1 3
0004 2 40
0004 2 41
0004 3 7
0005 9 99
0005 3 2
END DATA.
"####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5"####),
            (r####"a"####, r####"12 8 5 3 ."####),
            (r####"b"####, r####"345 77 . 41 ."####),
            (r####"c"####, r####"6 9 . 7 2"####),
        ],
        warnings: &[
            (r####"has no record of type"####, 0),
            (r####"second record of this type"####, 1),
            (r####"out of the order"####, 1),
            (r####"is not defined"####, 1),
        ],
    },
    Case {
        label: r####"GROUPED with DUPLICATE=NOWARN"####,
        command: r####"FILE TYPE GROUPED RECORD=rt 6 CASE=id 1-4 DUPLICATE=NOWARN.
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
0002 2 77
0002 1 8
0002 3 9
0003 1 5
0004 1 3
0004 2 40
0004 2 41
0004 3 7
0005 9 99
0005 3 2
END DATA.
"####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5"####),
            (r####"a"####, r####"12 8 5 3 ."####),
            (r####"b"####, r####"345 77 . 41 ."####),
            (r####"c"####, r####"6 9 . 7 2"####),
        ],
        warnings: &[
            (r####"has no record of type"####, 4),
            (r####"second record of this type"####, 0),
            (r####"out of the order"####, 1),
            (r####"is not defined"####, 1),
        ],
    },
    Case {
        label: r####"GROUPED with ORDERED=NO"####,
        command: r####"FILE TYPE GROUPED RECORD=rt 6 CASE=id 1-4 ORDERED=NO.
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
0002 2 77
0002 1 8
0002 3 9
0003 1 5
0004 1 3
0004 2 40
0004 2 41
0004 3 7
0005 9 99
0005 3 2
END DATA.
"####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5"####),
            (r####"a"####, r####"12 8 5 3 ."####),
            (r####"b"####, r####"345 77 . 41 ."####),
            (r####"c"####, r####"6 9 . 7 2"####),
        ],
        warnings: &[
            (r####"has no record of type"####, 4),
            (r####"second record of this type"####, 1),
            (r####"out of the order"####, 0),
            (r####"is not defined"####, 1),
        ],
    },
    Case {
        label: r####"GROUPED with WILD=NOWARN"####,
        command: r####"FILE TYPE GROUPED RECORD=rt 6 CASE=id 1-4 WILD=NOWARN.
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
0002 2 77
0002 1 8
0002 3 9
0003 1 5
0004 1 3
0004 2 40
0004 2 41
0004 3 7
0005 9 99
0005 3 2
END DATA.
"####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5"####),
            (r####"a"####, r####"12 8 5 3 ."####),
            (r####"b"####, r####"345 77 . 41 ."####),
            (r####"c"####, r####"6 9 . 7 2"####),
        ],
        warnings: &[
            (r####"has no record of type"####, 4),
            (r####"second record of this type"####, 1),
            (r####"out of the order"####, 1),
            (r####"is not defined"####, 0),
        ],
    },
    Case {
        label: r####"GROUPED with every warning off"####,
        command: r####"FILE TYPE GROUPED RECORD=rt 6 CASE=id 1-4 WILD=NOWARN MISSING=NOWARN DUPLICATE=NOWARN ORDERED=NO.
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
0002 2 77
0002 1 8
0002 3 9
0003 1 5
0004 1 3
0004 2 40
0004 2 41
0004 3 7
0005 9 99
0005 3 2
END DATA.
"####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5"####),
            (r####"a"####, r####"12 8 5 3 ."####),
            (r####"b"####, r####"345 77 . 41 ."####),
            (r####"c"####, r####"6 9 . 7 2"####),
        ],
        warnings: &[
            (r####"has no record of type"####, 0),
            (r####"second record of this type"####, 0),
            (r####"out of the order"####, 0),
            (r####"is not defined"####, 0),
        ],
    },
    Case {
        label: r####"NESTED with the defaults: no warning"####,
        command: r####"FILE TYPE NESTED RECORD=rt 1.
RECORD TYPE 1.
DATA LIST /hid 3-5 city 7-12 (A).
RECORD TYPE 2.
DATA LIST /vno 3 make 5-9 (A).
RECORD TYPE 3.
DATA LIST /pno 3 age 5-6.
END FILE TYPE.
BEGIN DATA
1 101 Oslo
3 1 34
2 1 Saab
3 1 51
2 2 Volv
2 3 Opel
3 1 70
1 102 Rome
3 1 40
END DATA.
"####,
        columns: &[
            (r####"hid"####, r####"101 101 101 102"####),
            (r####"vno"####, r####". 1 3 ."####),
            (r####"age"####, r####"34 51 70 40"####),
        ],
        warnings: &[
            (r####"no record of type"####, 0),
            (r####"second record of this type"####, 0),
        ],
    },
    Case {
        label: r####"NESTED with MISSING=WARN: a record with no record above it"####,
        command: r####"FILE TYPE NESTED RECORD=rt 1 MISSING=WARN.
RECORD TYPE 1.
DATA LIST /hid 3-5 city 7-12 (A).
RECORD TYPE 2.
DATA LIST /vno 3 make 5-9 (A).
RECORD TYPE 3.
DATA LIST /pno 3 age 5-6.
END FILE TYPE.
BEGIN DATA
1 101 Oslo
3 1 34
2 1 Saab
3 1 51
2 2 Volv
2 3 Opel
3 1 70
1 102 Rome
3 1 40
END DATA.
"####,
        columns: &[
            (r####"hid"####, r####"101 101 101 102"####),
            (r####"vno"####, r####". 1 3 ."####),
            (r####"age"####, r####"34 51 70 40"####),
        ],
        warnings: &[
            (r####"no record of type"####, 2),
            (r####"second record of this type"####, 0),
        ],
    },
    Case {
        label: r####"NESTED with DUPLICATE=WARN: a record of a type read again before any record under it"####,
        command: r####"FILE TYPE NESTED RECORD=rt 1 DUPLICATE=WARN.
RECORD TYPE 1.
DATA LIST /hid 3-5 city 7-12 (A).
RECORD TYPE 2.
DATA LIST /vno 3 make 5-9 (A).
RECORD TYPE 3.
DATA LIST /pno 3 age 5-6.
END FILE TYPE.
BEGIN DATA
1 101 Oslo
3 1 34
2 1 Saab
3 1 51
2 2 Volv
2 3 Opel
3 1 70
1 102 Rome
3 1 40
END DATA.
"####,
        columns: &[
            (r####"hid"####, r####"101 101 101 102"####),
            (r####"vno"####, r####". 1 3 ."####),
            (r####"age"####, r####"34 51 70 40"####),
        ],
        warnings: &[
            (r####"no record of type"####, 0),
            (r####"second record of this type"####, 1),
        ],
    },
    Case {
        label: r####"NESTED with both"####,
        command: r####"FILE TYPE NESTED RECORD=rt 1 MISSING=WARN DUPLICATE=WARN.
RECORD TYPE 1.
DATA LIST /hid 3-5 city 7-12 (A).
RECORD TYPE 2.
DATA LIST /vno 3 make 5-9 (A).
RECORD TYPE 3.
DATA LIST /pno 3 age 5-6.
END FILE TYPE.
BEGIN DATA
1 101 Oslo
3 1 34
2 1 Saab
3 1 51
2 2 Volv
2 3 Opel
3 1 70
1 102 Rome
3 1 40
END DATA.
"####,
        columns: &[
            (r####"hid"####, r####"101 101 101 102"####),
            (r####"vno"####, r####". 1 3 ."####),
            (r####"age"####, r####"34 51 70 40"####),
        ],
        warnings: &[
            (r####"no record of type"####, 2),
            (r####"second record of this type"####, 1),
        ],
    },
];

fn show(engine: &Engine, name: &str) -> Option<String> {
    let data = engine.dataset()?;
    Some(match data.variable(name).ok()?.values() {
        Values::Numeric(values) => values
            .iter()
            .map(|v| v.map_or(".".into(), |v| v.to_string()))
            .collect::<Vec<_>>()
            .join(" "),
        Values::String(values) => values
            .iter()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("|"),
    })
}

#[test]
fn file_type_warnings_agree_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let (commands, errors) = parse(case.command);
        if !errors.is_empty() {
            failures.push(format!("{}: {errors:?}", case.label));
            continue;
        }
        let mut engine = Engine::default();
        let mut failed = None;
        for command in &commands {
            if let Err(e) = engine.execute(&command.command) {
                failed = Some(format!("line {}: {e}", command.first_line));
                break;
            }
        }
        if let Some(error) = failed {
            failures.push(format!("{}: error {error}", case.label));
            continue;
        }
        for (name, want) in case.columns {
            let got = show(&engine, name).unwrap_or_else(|| "(no such variable)".into());
            if got != *want {
                failures.push(format!("{}: {name} is [{got}], R has [{want}]", case.label));
            }
        }
        for (piece, want) in case.warnings {
            let got = engine
                .warnings()
                .iter()
                .filter(|w| w.contains(piece))
                .count();
            if got != *want {
                failures.push(format!(
                    "{}: {got} warnings with \"{piece}\", R counts {want}: {:?}",
                    case.label,
                    engine.warnings()
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
