//! LAG, $CASENUM, N OF CASES and SAMPLE with SELECT IF, SORT CASES and TEMPORARY against R on invented
//! cases. LAG, $CASENUM and N OF CASES are worked out in R with vector indexing from the manual's rules:
//! LAG counts the cases that remain after SELECT IF, also when it is written before it, and it is
//! worked out after the other transformations of the same run (an EXECUTE between makes it see the
//! earlier value); the first n cases of LAG(x, n) are system-missing or blank; $CASENUM is the position
//! of the case in the file as it is when the command runs; N OF CASES keeps the first n cases that
//! survive SELECT IF whichever comes first, a larger later N OF CASES is ignored and TEMPORARY gives
//! the cases back. SAMPLE is random, so what is checked is the rules: n FROM m gives exactly n cases
//! from the first m (of those SELECT IF kept, when it came first), in file order without repeats; a
//! proportion gives a count inside R's binomial limits (a share of 50 in 8000 on 2000 cases too);
//! SAMPLE before SELECT IF gives a count inside the hypergeometric limits; N OF CASES with SAMPLE
//! gives n whichever comes first; TEMPORARY SAMPLE leaves every case in the file.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    command: &'static str,
    columns: &'static [(&'static str, &'static str)],
}

struct SampleCase {
    label: &'static str,
    command: &'static str,
    min: usize,
    max: usize,
    max_id: Option<usize>,
    even: bool,
}

const DATA: &str = r####"DATA LIST LIST /id (F3) x (F3) s (A3).
BEGIN DATA
1 5 'a'
2 3 'bb'
3 . ''
4 8 'dd'
5 1 'e'
6 2 'f'
7 . 'g'
8 9 ''
9 4 'ii'
10 6 'j'
11 7 'k'
12 2 'l'
13 5 'm'
14 . 'nn'
END DATA.
"####;

const CASES: &[Case] = &[
    Case {
        label: r####"LAG of a number with a missing value, of a string, and with a longer lag"####,
        command: r####"COMPUTE l1 = LAG(x).
COMPUTE l2 = LAG(x, 2).
COMPUTE l5 = LAG(id, 5).
STRING t (A3).
COMPUTE t = LAG(s, 2).
COMPUTE lall = LAG(id, 14)."####,
        columns: &[
            (r####"l1"####, r####". 5 3 . 8 1 2 . 9 4 6 7 2 5"####),
            (r####"l2"####, r####". . 5 3 . 8 1 2 . 9 4 6 7 2"####),
            (r####"l5"####, r####". . . . . 1 2 3 4 5 6 7 8 9"####),
            (r####"t"####, r####"||a|bb||dd|e|f|g||ii|j|k|l"####),
            (r####"lall"####, r####". . . . . . . . . . . . . ."####),
        ],
    },
    Case {
        label: r####"LAG counts the cases that remain after SELECT IF, also when LAG comes first"####,
        command: r####"COMPUTE before = LAG(id).
SELECT IF (id <> 3 AND id <> 7).
COMPUTE after = LAG(id, 2)."####,
        columns: &[
            (r####"id"####, r####"1 2 4 5 6 8 9 10 11 12 13 14"####),
            (r####"before"####, r####". 1 2 4 5 6 8 9 10 11 12 13"####),
            (r####"after"####, r####". . 1 2 4 5 6 8 9 10 11 12"####),
        ],
    },
    Case {
        label: r####"LAG written before a SELECT IF has no value in the first case that is kept"####,
        command: r####"COMPUTE p = LAG(x).
SELECT IF (id > 4)."####,
        columns: &[
            (r####"id"####, r####"5 6 7 8 9 10 11 12 13 14"####),
            (r####"p"####, r####". 1 2 . 9 4 6 7 2 5"####),
        ],
    },
    Case {
        label: r####"LAG is worked out after the other transformations of the same run"####,
        command: r####"COMPUTE lagvar = LAG(x).
COMPUTE x = x * 2."####,
        columns: &[
            (
                r####"lagvar"####,
                r####". 10 6 . 16 2 4 . 18 8 12 14 4 10"####,
            ),
            (r####"x"####, r####"10 6 . 16 2 4 . 18 8 12 14 4 10 ."####),
        ],
    },
    Case {
        label: r####"with an EXECUTE between, LAG sees the value as it was"####,
        command: r####"COMPUTE lagvar = LAG(x).
EXECUTE.
COMPUTE x = x * 2."####,
        columns: &[
            (r####"lagvar"####, r####". 5 3 . 8 1 2 . 9 4 6 7 2 5"####),
            (r####"x"####, r####"10 6 . 16 2 4 . 18 8 12 14 4 10 ."####),
        ],
    },
    Case {
        label: r####"$CASENUM counts the cases in file order"####,
        command: r####"COMPUTE n = $CASENUM.
COMPUTE both = $CASENUM + id."####,
        columns: &[
            (r####"n"####, r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14"####),
            (
                r####"both"####,
                r####"2 4 6 8 10 12 14 16 18 20 22 24 26 28"####,
            ),
        ],
    },
    Case {
        label: r####"$CASENUM after SORT CASES is the position in the sorted file"####,
        command: r####"SORT CASES BY x (D).
COMPUTE n = $CASENUM."####,
        columns: &[
            (r####"id"####, r####"8 4 11 10 1 13 9 2 6 12 5 3 7 14"####),
            (r####"n"####, r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14"####),
        ],
    },
    Case {
        label: r####"$CASENUM after SELECT IF numbers the cases that remain"####,
        command: r####"SELECT IF (id <> 3 AND id <> 7).
COMPUTE n = $CASENUM."####,
        columns: &[
            (r####"id"####, r####"1 2 4 5 6 8 9 10 11 12 13 14"####),
            (r####"n"####, r####"1 2 3 4 5 6 7 8 9 10 11 12"####),
        ],
    },
    Case {
        label: r####"N OF CASES keeps the first n"####,
        command: r####"N OF CASES 5."####,
        columns: &[(r####"id"####, r####"1 2 3 4 5"####)],
    },
    Case {
        label: r####"N OF CASES before SELECT IF: the first n cases that are selected"####,
        command: r####"N OF CASES 3.
SELECT IF (x > 2)."####,
        columns: &[(r####"id"####, r####"1 2 4"####)],
    },
    Case {
        label: r####"N OF CASES after SELECT IF gives the same"####,
        command: r####"SELECT IF (x > 2).
N OF CASES 3."####,
        columns: &[(r####"id"####, r####"1 2 4"####)],
    },
    Case {
        label: r####"a later N OF CASES with a larger number is ignored, a smaller one lowers the limit"####,
        command: r####"N OF CASES 6.
N OF CASES 9.
N OF CASES 4."####,
        columns: &[(r####"id"####, r####"1 2 3 4"####)],
    },
    Case {
        label: r####"N OF CASES larger than the file keeps everything"####,
        command: r####"N OF CASES 100."####,
        columns: &[(r####"id"####, r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14"####)],
    },
    Case {
        label: r####"TEMPORARY N OF CASES limits the next procedure only and the cases come back"####,
        command: r####"TEMPORARY.
N OF CASES 4.
COMPUTE tmp = 1.
DESCRIPTIVES id.
COMPUTE seen = 1."####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14"####),
            (r####"seen"####, r####"1 1 1 1 1 1 1 1 1 1 1 1 1 1"####),
        ],
    },
];

const SAMPLES: &[SampleCase] = &[
    SampleCase {
        label: r####"SAMPLE n FROM m takes exactly n cases from the first m"####,
        command: r####"SAMPLE 40 FROM 300."####,
        min: 40,
        max: 40,
        max_id: Some(300),
        even: false,
    },
    SampleCase {
        label: r####"SAMPLE n FROM m with m the size of the file"####,
        command: r####"SAMPLE 100 FROM 2000."####,
        min: 100,
        max: 100,
        max_id: Some(2000),
        even: false,
    },
    SampleCase {
        label: r####"SAMPLE n FROM m with m beyond the file takes the same share of the cases"####,
        command: r####"SAMPLE 50 FROM 8000."####,
        min: 0,
        max: 39,
        max_id: Some(2000),
        even: false,
    },
    SampleCase {
        label: r####"a proportion keeps about that share of the cases"####,
        command: r####"SAMPLE 0.25."####,
        min: 387,
        max: 619,
        max_id: Some(2000),
        even: false,
    },
    SampleCase {
        label: r####"a proportion of 0.9"####,
        command: r####"SAMPLE 0.9."####,
        min: 1715,
        max: 1876,
        max_id: Some(2000),
        even: false,
    },
    SampleCase {
        label: r####"SELECT IF before SAMPLE: the sample is drawn from the selected cases"####,
        command: r####"SELECT IF (MOD(id, 2) = 0).
SAMPLE 30 FROM 200."####,
        min: 30,
        max: 30,
        max_id: Some(400),
        even: true,
    },
    SampleCase {
        label: r####"SAMPLE before SELECT IF keeps a hypergeometric share"####,
        command: r####"SAMPLE 200 FROM 400.
SELECT IF (MOD(id, 2) = 0)."####,
        min: 70,
        max: 130,
        max_id: Some(400),
        even: true,
    },
    SampleCase {
        label: r####"N OF CASES with SAMPLE: the first n of the sample, whichever comes first"####,
        command: r####"N OF CASES 7.
SAMPLE 20 FROM 100."####,
        min: 7,
        max: 7,
        max_id: Some(100),
        even: false,
    },
    SampleCase {
        label: r####"N OF CASES after SAMPLE"####,
        command: r####"SAMPLE 20 FROM 100.
N OF CASES 7."####,
        min: 7,
        max: 7,
        max_id: Some(100),
        even: false,
    },
    SampleCase {
        label: r####"a second SAMPLE acts on the first one's sample"####,
        command: r####"SAMPLE 100 FROM 400.
SAMPLE 10 FROM 100."####,
        min: 10,
        max: 10,
        max_id: Some(400),
        even: false,
    },
    SampleCase {
        label: r####"TEMPORARY SAMPLE does not change the cases that stay"####,
        command: r####"TEMPORARY.
SAMPLE 10 FROM 100.
DESCRIPTIVES id.
SELECT IF (id <= 2000)."####,
        min: 2000,
        max: 2000,
        max_id: Some(2000),
        even: false,
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
fn lag_casenum_and_n_of_cases_agree_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let mut engine = Engine::default();
        let source = format!("{DATA}{}\nEXECUTE.", case.command);
        if let Err(error) = execute_all(&mut engine, &source) {
            failures.push(format!("{}: error {error}", case.label));
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

/// 2000 cases made in an input program, then the commands and a seed.
fn sampled(seed: u32, command: &str) -> Result<Vec<usize>, String> {
    let source = format!(
        "INPUT PROGRAM.\nLOOP #i = 1 TO 2000.\nCOMPUTE id = #i.\nEND CASE.\nEND LOOP.\nEND FILE.\n\
         END INPUT PROGRAM.\nSET SEED={seed}.\n{command}\nEXECUTE."
    );
    let mut engine = Engine::default();
    execute_all(&mut engine, &source)?;
    let text = show(&engine, "id").ok_or("no id")?;
    Ok(text
        .split(' ')
        .filter(|t| !t.is_empty())
        .map(|t| t.parse::<f64>().unwrap() as usize)
        .collect())
}

#[test]
fn sampling_follows_the_rules_of_the_manual() {
    let mut failures = Vec::new();
    for case in SAMPLES {
        for seed in [11, 2024, 70001] {
            let ids = match sampled(seed, case.command) {
                Ok(ids) => ids,
                Err(error) => {
                    failures.push(format!("{} (seed {seed}): error {error}", case.label));
                    continue;
                }
            };
            let what = format!("{} (seed {seed})", case.label);
            if ids.len() < case.min || ids.len() > case.max {
                failures.push(format!(
                    "{what}: {} cases, R allows {} to {}",
                    ids.len(),
                    case.min,
                    case.max
                ));
            }
            if ids.windows(2).any(|w| w[0] >= w[1]) {
                failures.push(format!("{what}: not in file order without repeats"));
            }
            if let Some(max_id) = case.max_id
                && ids.iter().any(|&i| i > max_id)
            {
                failures.push(format!("{what}: a case beyond {max_id}"));
            }
            if case.even && ids.iter().any(|i| i % 2 == 1) {
                failures.push(format!(
                    "{what}: an odd case that SELECT IF should have left out"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn n_of_cases_gives_the_same_cases_whichever_comes_first() {
    for seed in [3, 99] {
        let one = sampled(seed, "N OF CASES 7.\nSAMPLE 20 FROM 100.").unwrap();
        let two = sampled(seed, "SAMPLE 20 FROM 100.\nN OF CASES 7.").unwrap();
        assert_eq!(one.len(), 7);
        assert_eq!(one, two, "seed {seed}");
    }
}
