//! The data management commands given broken syntax: a command cut short, with a word dropped,
//! doubled, swapped or replaced by a random one, an unterminated quote, a number too large for
//! any field, and DO IF, LOOP, DO REPEAT, brackets and lists nested deeper than anyone writes.
//! Each must end in a result or a clear error within the time limit, never in a panic, and after
//! an error the active dataset is the one the run started with.

use statscraft_data::Dataset;
use statscraft_engine::session::Session;
use std::sync::mpsc;
use std::time::Duration;

/// A xorshift sequence: the same mutants on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const BASE: &str =
    "DATA LIST LIST /id (F3) g (F2) x (F6.2) y (F6.2) z (F6.2) s (A8) when (ADATE10).
BEGIN DATA
1 1 4.5 1 10 'ab' 01/02/2020
2 1 5.5 2 20 'cd' 03/04/2021
3 2 . 3 30 'ef' 12/31/1999
4 2 7.5 4 40 'gh' 02/29/2024
5 3 8.5 . 50 'ij' 06/30/2010
6 3 9.5 6 60 'kl' 07/07/2007
END DATA.
VARIABLE LABELS x 'Score' g 'Group'.
VALUE LABELS g 1 'one' 2 'two' 3 'three'.
MISSING VALUES z (60).
";

/// The commands the mutants are made of: one of each kind of data management command.
const SEEDS: &[&str] = &[
    "SORT CASES BY g (D) x.",
    "AGGREGATE OUTFILE=* MODE=ADDVARIABLES /BREAK=g /m=MEAN(x) /n=N.",
    "AGGREGATE OUTFILE=* /BREAK=g /sum=SUM(x y) /first=FIRST(s).",
    "COMPUTE total = SUM(x, y, z) / 2 + (id * 3).",
    "COMPUTE ratio = x / y.",
    "COMPUTE label = CONCAT(s, '-', STRING(id, F3)).",
    "IF (x > 5 AND y < 4) flag = 1.",
    "DO IF (x > 5).\nCOMPUTE big = 1.\nELSE IF (x > 4).\nCOMPUTE big = 2.\nELSE.\nCOMPUTE big = 3.\nEND IF.",
    "LOOP #i = 1 TO 3.\nCOMPUTE x = x + #i.\nEND LOOP.",
    "DO REPEAT v = x y z / w = a b c.\nCOMPUTE w = v * 2.\nEND REPEAT.",
    "VECTOR v = x TO z.\nLOOP #k = 1 TO 3.\nCOMPUTE v(#k) = v(#k) + 1.\nEND LOOP.",
    "RECODE x y (LOWEST THRU 5 = 1) (5 THRU HIGHEST = 2) (MISSING = 9) (ELSE = 0) INTO rx ry.",
    "RECODE s ('ab' = 'AB') ('cd' = 'CD') (ELSE = COPY).",
    "COUNT hits = x y z (4 THRU 9).",
    "STRING t (A5).\nCOMPUTE t = 'hello'.",
    "NUMERIC n1 n2 (F8.2).",
    "RENAME VARIABLES (x = score) (y = other).",
    "DELETE VARIABLES y z.",
    "VARIABLE LABELS y 'Why' z 'Zed'.",
    "VALUE LABELS y 1 'low' 2 'mid' 3 'high'.",
    "MISSING VALUES x y (LOWEST THRU 0, 99).",
    "FORMATS x y (F8.3) z (COMMA9.1).",
    "ALTER TYPE s (A12) id (F8.0).",
    "SELECT IF (g < 3).",
    "SAMPLE 0.5.",
    "WEIGHT BY g.",
    "SPLIT FILE BY g.",
    "FILTER BY id.",
    "SORT VARIABLES BY NAME (D).",
    "AUTORECODE VARIABLES=s /INTO s_code /PRINT.",
    "MATCH FILES /FILE=* /DROP=y z.",
    "ADD FILES /FILE=* /FILE=* /IN=copy.",
    "VARSTOCASES /MAKE v FROM x y z /INDEX=which /KEEP=id.",
    "CASESTOVARS /ID=g /INDEX=id /GROUPBY=VARIABLE.",
    "FLIP VARIABLES=x y z /NEWNAMES=s.",
    "COMPUTE year = XDATE.YEAR(when).",
    "COMPUTE m = MEAN.2(x, y, z).\nCOMPUTE d = DATEDIFF(when, DATE.DMY(1,1,2000), 'days').",
    "TEMPORARY.\nSELECT IF (x > 5).",
    "DATASET NAME other.\nDATASET ACTIVATE other.",
    "APPLY DICTIONARY FROM *.",
    "SORT CASES BY g.\nSAVE OUTFILE='/nonexistent-folder/out.sav'.",
];

/// The words a mutant may put in.
const WORDS: &[&str] = &[
    "BY",
    "TO",
    "THRU",
    "LOWEST",
    "HIGHEST",
    "MISSING",
    "ELSE",
    "END",
    "IF",
    "LOOP",
    "REPEAT",
    "INTO",
    "WITH",
    "ALL",
    "x",
    "y",
    "z",
    "s",
    "g",
    "id",
    "nosuch",
    "#t",
    "$sysmis",
    "SYSMIS",
    "/",
    "(",
    ")",
    "=",
    ",",
    ".",
    "'",
    "\"",
    "(D)",
    "(A)",
    "F8.2",
    "A0",
    "F0",
    "A99999999",
    "99999999999999999999",
    "1e999",
    "-1",
    "0",
    "4294967296",
    "2147483648",
    "0.0000000001",
    "OUTFILE=*",
    "/BREAK=",
    "NAME",
    "MODE=ADDVARIABLES",
    "**",
    "+",
    "-",
    "AND",
    "OR",
    "NOT",
];

fn tokens(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() || matches!(c, '_' | '#' | '$' | '@') {
            word.push(c);
        } else {
            if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
            out.push(c.to_string());
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// One mutant of `seed`: how it was made, and the text.
fn mutate(seed: &str, rng: &mut Rng) -> (String, String) {
    let mut toks = tokens(seed);
    let how = rng.below(9);
    let at = rng.below(toks.len());
    let name = match how {
        0 => {
            // cut short, in the middle of a word or after it
            let cut = rng.below(seed.len().max(1));
            let cut = (0..=cut)
                .rev()
                .find(|&i| seed.is_char_boundary(i))
                .unwrap_or(0);
            return ("cut".into(), seed[..cut].to_string());
        }
        1 => {
            toks.remove(at);
            "drop"
        }
        2 => {
            let t = toks[at].clone();
            toks.insert(at, t);
            "double"
        }
        3 => {
            toks[at] = WORDS[rng.below(WORDS.len())].to_string();
            "replace"
        }
        4 => {
            let w = WORDS[rng.below(WORDS.len())].to_string();
            toks.insert(at, w);
            "insert"
        }
        5 => {
            let next = (at + 1).min(toks.len() - 1);
            toks.swap(at, next);
            "swap"
        }
        6 => {
            toks.insert(
                at,
                ["'", "\"", "(", ")", "[", "{"][rng.below(6)].to_string(),
            );
            "bracket"
        }
        // (a huge bound of an indexed LOOP is a loop that runs as long as it is asked to)
        7 if !seed.contains("LOOP") => {
            for t in toks
                .iter_mut()
                .filter(|t| t.chars().all(|c| c.is_ascii_digit()))
            {
                *t = ["99999999999999999999999", "4294967296", "2147483648", "0"][rng.below(4)]
                    .into();
            }
            "huge"
        }
        _ => {
            return (
                "unterminated".into(),
                format!("{} '{}", seed.trim_end_matches('.'), "text"),
            );
        }
    };
    (name.to_string(), toks.concat())
}

fn base() -> Session {
    let mut session = Session::default();
    session.run(BASE, None).unwrap_or_else(|e| panic!("{e:?}"));
    session
}

/// What a run of `source` came to.
enum Ended {
    Ok,
    Error,
    Panic(String),
    Hang,
}

/// Runs `source` on the base dataset on a thread with the stack of a run's thread (2 MiB, as the
/// window's worker has); the dataset after an error must be the one before.
fn run_on_base(source: String, limit: Duration) -> Ended {
    let (send, receive) = mpsc::channel();
    std::thread::Builder::new()
        .stack_size(2 << 20)
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut session = base();
                let before: Dataset = session.dataset().unwrap().clone();
                match session.run(&source, None) {
                    Ok(_) => Ended::Ok,
                    Err(errors) => {
                        let text = errors
                            .iter()
                            .map(|e| e.text())
                            .collect::<Vec<_>>()
                            .join("; ");
                        match session.dataset() {
                            Some(after) if *after == before => Ended::Error,
                            _ => Ended::Panic(format!("the dataset changed after: {text}")),
                        }
                    }
                }
            }));
            let ended = result.unwrap_or_else(|panic| {
                let text = panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();
                Ended::Panic(format!("panic: {text}"))
            });
            let _ = send.send(ended);
        })
        .unwrap();
    receive.recv_timeout(limit).unwrap_or(Ended::Hang)
}

fn shown(source: &str) -> String {
    let flat = source.replace('\n', " / ");
    flat.chars().take(160).collect()
}

fn troubles(inputs: Vec<(String, String)>, limit: Duration) -> Vec<String> {
    let mut out = Vec::new();
    for (label, source) in inputs {
        if std::env::var("DM_SYNTAX_TRACE").is_ok() {
            eprintln!("@@ {label}");
        }
        match run_on_base(source.clone(), limit) {
            Ended::Ok | Ended::Error => {}
            Ended::Panic(text) => out.push(format!("{label}: {text}: {}", shown(&source))),
            Ended::Hang => {
                out.push(format!(
                    "{label}: no end within {limit:?}: {}",
                    shown(&source)
                ));
                // the thread still runs: stop here rather than pile more on
                break;
            }
        }
    }
    out
}

#[test]
fn the_seeds_are_valid_commands() {
    let mut bad = Vec::new();
    for seed in SEEDS {
        let (_, errors) = statscraft_syntax::parse(seed);
        if let Some(e) = errors.first() {
            bad.push(format!("{}: {}", shown(seed), e.message));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn broken_data_management_syntax_gives_an_error_and_leaves_the_dataset() {
    let mut rng = Rng(0xd15c_0001);
    let per_seed = std::env::var("DM_SYNTAX_MUTANTS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(25);
    let mut inputs = Vec::new();
    for seed in SEEDS {
        inputs.push(("seed".to_string(), seed.to_string()));
        for _ in 0..per_seed {
            let (how, text) = mutate(seed, &mut rng);
            // a loop with a bound of millions of steps runs as long as it is asked to
            let long_number = text
                .split(|c: char| !c.is_ascii_digit())
                .any(|digits| digits.len() >= 6);
            if seed.contains("LOOP") && long_number {
                continue;
            }
            inputs.push((how, text));
        }
    }
    let found = troubles(inputs, Duration::from_secs(20));
    assert!(
        found.is_empty(),
        "{} troubles:\n{}",
        found.len(),
        found.join("\n")
    );
}

/// Structures nested far deeper than anyone writes.
fn nested() -> Vec<(String, String)> {
    let mut out = Vec::new();
    // (60,000 levels take minutes in a debug build; DM_SYNTAX_FULL=1 asks for them)
    let sizes: &[usize] = if std::env::var("DM_SYNTAX_FULL").is_ok() {
        &[50, 120, 1_500, 60_000]
    } else {
        &[50, 120, 1_500]
    };
    for &n in sizes {
        let deep = |open: &str, close: &str| {
            format!("{}COMPUTE q = 1.\n{}", open.repeat(n), close.repeat(n))
        };
        out.push((format!("DO IF x{n}"), deep("DO IF (x > 0).\n", "END IF.\n")));
        out.push((
            format!("LOOP x{n}"),
            deep("LOOP #i = 1 TO 1.\n", "END LOOP.\n"),
        ));
        out.push((
            format!("DO REPEAT x{n}"),
            deep("DO REPEAT a = x.\n", "END REPEAT.\n"),
        ));
        out.push((
            format!("parentheses x{n}"),
            format!("COMPUTE q = {}1{}.", "(".repeat(n), ")".repeat(n)),
        ));
        out.push((
            format!("functions x{n}"),
            format!("COMPUTE q = {}x{}.", "ABS(".repeat(n), ")".repeat(n)),
        ));
        out.push((
            format!("ELSE IF chain x{n}"),
            format!(
                "DO IF (x > 1).\nCOMPUTE q = 1.\n{}END IF.",
                "ELSE IF (x > 2).\nCOMPUTE q = 2.\n".repeat(n)
            ),
        ));
        out.push((
            format!("long expression x{n}"),
            format!("COMPUTE q = {}1.", "x + ".repeat(n)),
        ));
        out.push((
            format!("long list x{n}"),
            format!("SORT CASES BY {}.", "x ".repeat(n)),
        ));
        out.push((
            format!("long string x{n}"),
            format!("COMPUTE s = '{}'.", "a".repeat(n * 10)),
        ));
        out.push((format!("many commands x{n}"), "COMPUTE q = 1.\n".repeat(n)));
    }
    out
}

#[test]
fn nesting_far_deeper_than_anyone_writes_gives_an_error_and_leaves_the_dataset() {
    let found = troubles(nested(), Duration::from_secs(60));
    assert!(
        found.is_empty(),
        "{} troubles:\n{}",
        found.len(),
        found.join("\n")
    );
}
