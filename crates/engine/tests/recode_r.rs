//! RECODE, COUNT and AUTORECODE against R on invented data.
//!
//! RECODE: value lists, ranges with LO and HI (which take in user-missing values but not the
//! system-missing one), MISSING, SYSMIS, ELSE (which also takes the system-missing value), COPY
//! and SYSMIS as new values, the first matching specification wins, a recoded user-missing value
//! is valid, an unmatched value stays when recoding in place and is missing (or blank) when
//! recoding into a new variable, several variables with one list of specifications or each with
//! its own, strings into strings, strings into numbers, numbers into strings, and CONVERT before
//! and after other specifications.
//! COUNT: values, ranges, MISSING and SYSMIS, a declared missing code counted by its value,
//! different values for different variables, strings, TO, and a target that already exists.
//! AUTORECODE: the ascending or descending order of the values, user-missing values after the
//! others, system-missing kept, BLANK, GROUP (the user-missing values of the first variable that
//! has them) and a saved template applied to another variable (new values after the template's).

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    setup: &'static str,
    /// `{TMP}` stands for a folder made for the run.
    command: &'static str,
    /// The values of each variable: numbers separated by spaces with `.` for missing, strings
    /// separated by `|`.
    columns: &'static [(&'static str, &'static str)],
}

const MAIN: &str = r#"DATA LIST LIST /a (F8.1) b (F8) cc (F8) s1 (A4) s2 (A6) n1 (A3) tt (A2).
BEGIN DATA
3 . 2 'kiwi' 'ab' '12' 'aa'
0 3 2 'plum' 'cd' '3.5' 'bb'
-9 5 7 'fig' 'ab' '7' 'cc'
. 1 5 'kiwi' 'ef' 'x' 'aa'
5.5 4 . '' 'cd' '' 'dd'
7 9 0 'pear' '' '-4' 'bb'
1 2 -1 'plum' 'ef' '0.5' 'cc'
12 -9 4 'fig' 'ab' '9' 'aa'
-2.5 8 4 'date' 'gh' 'abc' 'ee'
4 3 1 'kiwi' 'cd' '8' 'bb'
2 . 3 'plum' 'ab' '' 'dd'
99 6 8 'pear' 'ef' '2' 'cc'
6 0 8 'fig' 'gh' '6' 'aa'
7 5 2 'date' 'cd' '10' 'ff'
END DATA.
MISSING VALUES a (-9, 99) b (-9).
"#;
const AUTO: &str = r#"DATA LIST LIST /v1 (F8) w1 (A5) w2 (A2).
BEGIN DATA
30 'pear' 'x'
10 'fig' 'b'
20 '' 'zz'
10 'pear' 'b'
-9 'apple' 'x'
. 'fig' 'a'
30 '' 'k'
99 'kiwi' 'a'
END DATA.
MISSING VALUES v1 (-9, 99).
"#;
const GROUPDATA: &str = r#"DATA LIST LIST /p1 (A2) p2 (A2).
BEGIN DATA
'aa' 'bb'
'bb' 'ee'
'cc' 'ff'
'dd' 'aa'
'aa' 'gg'
END DATA.
MISSING VALUES p1 ('cc') p2 ('gg').
"#;
const TEMPLATE: &str = r#"DATA LIST LIST /x1 (A5).
BEGIN DATA
'rose'
'lily'
'tulip'
'rose'
END DATA.
"#;

const CASES: &[Case] = &[
    Case {
        label: r#"in place with ranges and COPY"#,
        setup: MAIN,
        command: r#"RECODE a (LO THRU 0=-1) (1 THRU 4=10) (5,7=20) (ELSE=COPY)."#,
        columns: &[(r#"a"#, r#"10 -1 -1 . 5.5 20 10 12 -1 10 10 99 6 20"#)],
    },
    Case {
        label: r#"into a new variable leaves the rest missing"#,
        setup: MAIN,
        command: r#"RECODE a (MISSING=0) (6 THRU HI=3) INTO a2."#,
        columns: &[
            (r#"a2"#, r#". . 0 0 . 3 . 3 . . . 0 3 3"#),
            (r#"a"#, r#"3 0 -9 . 5.5 7 1 12 -2.5 4 2 99 6 7"#),
        ],
    },
    Case {
        label: r#"two variables in place with SYSMIS"#,
        setup: MAIN,
        command: r#"RECODE a b (SYSMIS=77) (ELSE=COPY)."#,
        columns: &[
            (r#"a"#, r#"3 0 -9 77 5.5 7 1 12 -2.5 4 2 99 6 7"#),
            (r#"b"#, r#"77 3 5 1 4 9 2 -9 8 3 77 6 0 5"#),
        ],
    },
    Case {
        label: r#"separate specifications and ELSE reaching the system-missing value"#,
        setup: MAIN,
        command: r#"RECODE a (1,2=7) /b (3 THRU 5=8) (ELSE=0)."#,
        columns: &[
            (r#"a"#, r#"3 0 -9 . 5.5 7 7 12 -2.5 4 7 99 6 7"#),
            (r#"b"#, r#"0 8 8 0 8 0 0 0 0 8 0 0 0 8"#),
        ],
    },
    Case {
        label: r#"several variables into several targets"#,
        setup: MAIN,
        command: r#"RECODE a b (1 THRU 3=1) (4 THRU HI=2) INTO ra rb."#,
        columns: &[
            (r#"ra"#, r#"1 . . . 2 2 1 2 . 2 1 2 2 2"#),
            (r#"rb"#, r#". 1 2 1 2 2 1 . 2 1 . 2 . 2"#),
        ],
    },
    Case {
        label: r#"a value is recoded once, the first match wins"#,
        setup: MAIN,
        command: r#"RECODE a (3 THRU HI=1) (1 THRU 5=2) INTO a3."#,
        columns: &[(r#"a3"#, r#"1 . . . 1 1 2 1 . 1 2 1 1 1"#)],
    },
    Case {
        label: r#"negative decimal range, a new value of SYSMIS"#,
        setup: MAIN,
        command: r#"RECODE a (-2.5 THRU -0.5=SYSMIS) (0 THRU 2.5=100) (ELSE=COPY) INTO a4."#,
        columns: &[(r#"a4"#, r#"3 100 -9 . 5.5 7 100 12 . 4 100 99 6 7"#)],
    },
    Case {
        label: r#"a user-missing value is matched by its value and the new value is valid"#,
        setup: MAIN,
        command: r#"RECODE b (-9=5) INTO b2."#,
        columns: &[(r#"b2"#, r#". . . . . . . 5 . . . . . ."#)],
    },
    Case {
        label: r#"strings into strings"#,
        setup: MAIN,
        command: r#"STRING s1r (A6).
RECODE s1 ('kiwi','plum'='fruit') ('fig'='dried') (ELSE=COPY) INTO s1r."#,
        columns: &[(
            r#"s1r"#,
            r#"fruit|fruit|dried|fruit||pear|fruit|dried|date|fruit|fruit|pear|dried|date"#,
        )],
    },
    Case {
        label: r#"strings in place, unmatched stay"#,
        setup: MAIN,
        command: r#"RECODE s2 ('ab','cd'='low') ('gh'='high')."#,
        columns: &[(
            r#"s2"#,
            r#"low|low|low|ef|low||ef|low|high|low|low|ef|high|low"#,
        )],
    },
    Case {
        label: r#"string into number"#,
        setup: MAIN,
        command: r#"RECODE tt ('aa'=1) ('bb','cc'=2) INTO nt."#,
        columns: &[(r#"nt"#, r#"1 2 2 1 . 2 2 1 . 2 . 2 1 ."#)],
    },
    Case {
        label: r#"number into string"#,
        setup: MAIN,
        command: r#"STRING sa (A4).
RECODE a (1 THRU 3='low') (4 THRU HI='high') INTO sa."#,
        columns: &[(
            r#"sa"#,
            r#"low||||high|high|low|high||high|low|high|high|high"#,
        )],
    },
    Case {
        label: r#"CONVERT first"#,
        setup: MAIN,
        command: r#"RECODE n1 (CONVERT) INTO c1."#,
        columns: &[(r#"c1"#, r#"12 3.5 7 . . -4 0.5 9 . 8 . 2 6 10"#)],
    },
    Case {
        label: r#"a specification before CONVERT"#,
        setup: MAIN,
        command: r#"RECODE n1 ('x'=-1) (CONVERT) INTO c2."#,
        columns: &[(r#"c2"#, r#"12 3.5 7 -1 . -4 0.5 9 . 8 . 2 6 10"#)],
    },
    Case {
        label: r#"blank recoded before CONVERT, one after"#,
        setup: MAIN,
        command: r#"RECODE n1 (' '=-99) (CONVERT) ('abc'=-5) INTO c3."#,
        columns: &[(r#"c3"#, r#"12 3.5 7 . -99 -4 0.5 9 -5 8 -99 2 6 10"#)],
    },
    Case {
        label: r#"COUNT values and a range over three variables"#,
        setup: MAIN,
        command: r#"COUNT k1 = a b cc (1 THRU 4)."#,
        columns: &[(r#"k1"#, r#"2 2 0 1 1 0 2 1 1 3 2 0 0 1"#)],
    },
    Case {
        label: r#"COUNT MISSING, SYSMIS and declared codes"#,
        setup: MAIN,
        command: r#"COUNT k2 = a b (MISSING) /k3 = a b cc (SYSMIS) /k4 = a b cc (-9, 99)."#,
        columns: &[
            (r#"k2"#, r#"1 0 1 1 0 0 0 1 0 0 1 1 0 0"#),
            (r#"k3"#, r#"1 0 0 1 1 0 0 0 0 0 1 0 0 0"#),
            (r#"k4"#, r#"0 0 1 0 0 0 0 1 0 0 0 1 0 0"#),
        ],
    },
    Case {
        label: r#"COUNT different values for different variables"#,
        setup: MAIN,
        command: r#"COUNT k5 = a (LO THRU 0) b (5 THRU HI) cc (2)."#,
        columns: &[(r#"k5"#, r#"1 2 2 0 0 1 0 0 2 0 0 1 0 2"#)],
    },
    Case {
        label: r#"COUNT strings"#,
        setup: MAIN,
        command: r#"COUNT k6 = s1 s2 ('kiwi' 'ab') /k7 = s1 ('plum') s2 ('cd' 'ef')."#,
        columns: &[
            (r#"k6"#, r#"2 0 1 1 0 0 0 1 0 1 1 0 0 0"#),
            (r#"k7"#, r#"0 2 0 1 1 0 2 0 0 1 1 1 0 1"#),
        ],
    },
    Case {
        label: r#"COUNT replaces an existing target"#,
        setup: MAIN,
        command: r#"COMPUTE k8 = 100.
COUNT k8 = a (1, 2, 3)."#,
        columns: &[(r#"k8"#, r#"1 0 0 0 0 0 1 0 0 0 1 0 0 0"#)],
    },
    Case {
        label: r#"COUNT with TO"#,
        setup: MAIN,
        command: r#"COUNT k9 = a TO cc (2 3)."#,
        columns: &[(r#"k9"#, r#"2 2 0 0 0 0 1 0 0 1 2 0 0 1"#)],
    },
    Case {
        label: r#"AUTORECODE numeric with user-missing and system-missing"#,
        setup: AUTO,
        command: r#"AUTORECODE v1 /INTO r1."#,
        columns: &[(r#"r1"#, r#"3 1 2 1 4 . 3 5"#)],
    },
    Case {
        label: r#"AUTORECODE descending"#,
        setup: AUTO,
        command: r#"AUTORECODE v1 /INTO r1 /DESCENDING."#,
        columns: &[(r#"r1"#, r#"1 3 2 3 5 . 1 4"#)],
    },
    Case {
        label: r#"AUTORECODE strings, blank valid"#,
        setup: AUTO,
        command: r#"AUTORECODE w1 /INTO r2."#,
        columns: &[(r#"r2"#, r#"5 3 1 5 2 3 1 4"#)],
    },
    Case {
        label: r#"AUTORECODE strings, blank missing"#,
        setup: AUTO,
        command: r#"AUTORECODE w1 /INTO r2 /BLANK=MISSING."#,
        columns: &[(r#"r2"#, r#"4 2 5 4 1 2 5 3"#)],
    },
    Case {
        label: r#"AUTORECODE strings descending, blank missing"#,
        setup: AUTO,
        command: r#"AUTORECODE w1 /INTO r2 /BLANK=MISSING /DESCENDING."#,
        columns: &[(r#"r2"#, r#"1 3 5 1 4 3 5 2"#)],
    },
    Case {
        label: r#"AUTORECODE two variables apart"#,
        setup: AUTO,
        command: r#"AUTORECODE w1 w2 /INTO r2 r3."#,
        columns: &[
            (r#"r2"#, r#"5 3 1 5 2 3 1 4"#),
            (r#"r3"#, r#"4 2 5 2 4 1 3 1"#),
        ],
    },
    Case {
        label: r#"AUTORECODE grouped, the user-missing value of the first variable"#,
        setup: GROUPDATA,
        command: r#"AUTORECODE p1 p2 /INTO q1 q2 /GROUP."#,
        columns: &[(r#"q1"#, r#"1 2 7 3 1"#), (r#"q2"#, r#"2 4 5 1 6"#)],
    },
    Case {
        label: r#"AUTORECODE saves a template and applies it"#,
        setup: TEMPLATE,
        command: r#"AUTORECODE x1 /INTO y1 /SAVE TEMPLATE='{TMP}/flowers.sat'.
DATA LIST LIST /x2 (A5).
BEGIN DATA
lily
daisy
rose
iris
tulip
END DATA.
AUTORECODE x2 /INTO y2 /APPLY TEMPLATE='{TMP}/flowers.sat'."#,
        columns: &[(r#"y2"#, r#"1 4 2 5 3"#)],
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

fn run(case: &Case, folder: &str) -> Result<Engine, String> {
    let source = format!("{}{}", case.setup, case.command.replace("{TMP}", folder));
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
fn recode_count_and_autorecode_agree_with_r() {
    let folder = std::env::temp_dir().join(format!("statscraft-recode-r-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let folder_text = folder.to_str().unwrap().replace('\\', "/");
    let mut failures = Vec::new();
    for case in CASES {
        let engine = match run(case, &folder_text) {
            Ok(engine) => engine,
            Err(error) => {
                failures.push(format!("{}: error {error}", case.label));
                continue;
            }
        };
        for (name, want) in case.columns {
            let got = show(&engine, name).expect("the variable exists");
            if got != *want {
                failures.push(format!("{}: {name} is [{got}], R has [{want}]", case.label));
            }
        }
    }
    std::fs::remove_dir_all(&folder).ok();
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
}
