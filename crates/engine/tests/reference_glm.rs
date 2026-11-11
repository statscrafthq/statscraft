//! LOGISTIC REGRESSION and GLM compared with GNU PSPP, run as a black box on the same files and
//! the same syntax. Ignored by default because they need PSPP:
//!
//! ```text
//! STATSCRAFT_PSPP=/path/to/pspp cargo test -p statscraft-engine --test reference_glm -- --ignored
//! ```
//!
//! PSPP's LOGISTIC REGRESSION knows VARIABLES with WITH, CATEGORICAL (indicator coding),
//! METHOD=ENTER, PRINT=CI, CRITERIA, MISSING and ORIGIN; its GLM knows factors without
//! covariates, METHOD=SSTYPE(1 to 3), INTERCEPT, MISSING and DESIGN, and shows no Intercept row.
//! The cases stay within that. PSPP prints with `SET FORMAT=F20.10`; each of our numbers must
//! round to what it printed. BCON is tight in both programs, so both estimates are converged.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs};

const SURVEY: &str = "survey_basic.sav";
/// The survey with `weight` as the weight variable.
const WEIGHTED: &str = "survey_basic_phpwriter.sav";
const PRECISE: &str =
    "FORMATS age monthly_spend q1_satisfaction q2_recommend q5_1 TO q5_5 (F20.10).";

static RUNS: AtomicUsize = AtomicUsize::new(0);

fn setup(case: &str, fixture: &str) -> Option<(PathBuf, PathBuf)> {
    let Some(pspp) = env::var_os("STATSCRAFT_PSPP") else {
        eprintln!("{case}: skipped, STATSCRAFT_PSPP is not set");
        return None;
    };
    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    let dir = env::temp_dir().join(format!("statscraft-glm-{}-{run}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    fs::copy(source.join(fixture), dir.join("data.sav")).unwrap();
    Some((PathBuf::from(pspp), dir))
}

fn ours(case: &str, dir: &Path, syntax: &str) -> Vec<Table> {
    let syntax = syntax.replace("{dir}", &format!("{}/", dir.display()));
    let (parsed, errors) = parse(&syntax);
    assert!(errors.is_empty(), "{case}: {errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &parsed {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("{case}: line {}: {error}", command.first_line),
        }
    }
    tables
}

fn pspp(case: &str, program: &Path, dir: &Path, syntax: &str) -> String {
    let job = format!("SET FORMAT=F20.10.\n{}", syntax.replace("{dir}", ""));
    fs::write(dir.join("job.sps"), job).unwrap();
    let status = Command::new(program)
        .args(["job.sps", "-o", "pspp.csv", "-O", "format=csv"])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", program.display()));
    let output = fs::read_to_string(dir.join("pspp.csv")).unwrap_or_default();
    assert!(status.success(), "{case}: PSPP failed\n{output}");
    assert!(
        !output.contains("error:"),
        "{case}: PSPP reported an error\n{output}"
    );
    output
}

fn csv_records(text: &str) -> Vec<Vec<String>> {
    let (mut records, mut record, mut field) = (Vec::new(), Vec::new(), String::new());
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            (true, '"') => quoted = false,
            (true, c) => field.push(c),
            (false, '"') => quoted = true,
            (false, ',') => record.push(std::mem::take(&mut field)),
            (false, '\n') => {
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            (false, '\r') => {}
            (false, c) => field.push(c),
        }
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    records
}

/// The tables of PSPP's CSV output: title and rows.
fn pspp_tables(output: &str) -> Vec<(String, Vec<Vec<String>>)> {
    let mut tables = Vec::new();
    let mut current: Option<(String, Vec<Vec<String>>)> = None;
    for record in csv_records(output) {
        if let Some(title) = record[0].strip_prefix("Table: ") {
            tables.extend(current.take());
            current = Some((title.to_string(), Vec::new()));
        } else if record.len() == 1 && record[0].is_empty() {
            tables.extend(current.take());
        } else if let Some((_, rows)) = current.as_mut() {
            rows.push(record);
        }
    }
    tables.extend(current);
    tables
}

/// None for a cell that shows no number; Some(None) for a blank cell.
fn pspp_number(text: &str) -> Option<Option<(f64, i32)>> {
    let text = text.trim();
    if text.is_empty() || text == "." {
        return Some(None);
    }
    let text = text.strip_suffix('%').unwrap_or(text);
    let value: f64 = text.parse().ok()?;
    let (mantissa, exponent) = match text.split_once(['E', 'e']) {
        Some((m, e)) => (m, e.parse::<i32>().ok()?),
        None => (text, 0),
    };
    let decimals = mantissa.split_once('.').map_or(0, |(_, d)| d.len() as i32) - exponent;
    Some(Some((value, decimals)))
}

/// One table compared: PSPP's title, ours, PSPP's header rows (None: the rows before the first
/// that starts with Step), the (PSPP, ours) column pairs
/// and how many of our first rows take part (None: all, after dropping the Intercept row PSPP
/// does not show).
struct Spec {
    pspp: &'static str,
    ours: &'static str,
    header: Option<usize>,
    pairs: &'static [(usize, usize)],
    rows: Option<usize>,
}

const LOGISTIC: &[Spec] = &[
    Spec {
        pspp: "Dependent Variable Encoding",
        ours: "Dependent Variable Encoding",
        header: Some(1),
        pairs: &[(1, 1)],
        rows: None,
    },
    Spec {
        pspp: "Case Processing Summary",
        ours: "Case Processing Summary",
        header: Some(1),
        pairs: &[(1, 2), (2, 3)],
        rows: Some(3),
    },
    Spec {
        pspp: "Model Summary",
        ours: "Model Summary",
        header: Some(1),
        pairs: &[(1, 1), (2, 2), (3, 3)],
        rows: None,
    },
    Spec {
        pspp: "Categorical Variables' Codings",
        ours: "Categorical Variables Codings",
        header: Some(2),
        pairs: &[(2, 2), (3, 3), (4, 4), (5, 5), (6, 6)],
        rows: None,
    },
    Spec {
        pspp: "Classification Table",
        ours: "Classification Table",
        header: None,
        pairs: &[(3, 2), (4, 3), (5, 4)],
        rows: None,
    },
    Spec {
        pspp: "Variables in the Equation",
        ours: "Variables in the Equation",
        header: None,
        pairs: &[
            (2, 2),
            (3, 3),
            (4, 4),
            (5, 5),
            (6, 6),
            (7, 7),
            (8, 8),
            (9, 9),
        ],
        rows: None,
    },
];

const GLM: &[Spec] = &[Spec {
    pspp: "Tests of Between-Subjects Effects",
    ours: "Tests of Between-Subjects Effects",
    header: Some(1),
    pairs: &[(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)],
    rows: None,
}];

/// Compares the tables of `specs`; returns the number of cells compared.
fn compare(case: &str, fixture: &str, syntax: &str, specs: &[Spec]) -> usize {
    let Some((program, dir)) = setup(case, fixture) else {
        return 0;
    };
    let syntax = format!("GET FILE='{{dir}}data.sav'.\n{PRECISE}\n{syntax}\n");
    let output = pspp(case, &program, &dir, &syntax);
    let theirs = pspp_tables(&output);
    let mine = ours(case, &dir, &syntax);
    let mut problems = Vec::new();
    let mut compared = 0;
    for spec in specs {
        let pspp_all: Vec<&(String, Vec<Vec<String>>)> =
            theirs.iter().filter(|(t, _)| t == spec.pspp).collect();
        // the last of our tables of that title: block 1 after block 0
        let our_all: Vec<&Table> = mine.iter().filter(|t| t.title == spec.ours).collect();
        let (Some((_, rows)), Some(table)) = (pspp_all.last(), our_all.last()) else {
            if !pspp_all.is_empty() || !our_all.is_empty() {
                problems.push(format!("{}: missing in PSPP or ours", spec.ours));
            }
            continue;
        };
        let header = spec.header.unwrap_or_else(|| {
            rows.iter()
                .take_while(|r| !r[0].starts_with("Step"))
                .count()
        });
        let data = &rows[header..];
        let our_rows: Vec<&Vec<Cell>> = table
            .rows
            .iter()
            .filter(|r| !matches!(&r[0], Cell::Text(t) if t == "Intercept"))
            .take(spec.rows.unwrap_or(usize::MAX))
            .collect();
        if data.len() != our_rows.len() {
            problems.push(format!(
                "{}: rows: PSPP {}, ours {}",
                spec.ours,
                data.len(),
                our_rows.len()
            ));
            continue;
        }
        for (r, (row, mine)) in data.iter().zip(&our_rows).enumerate() {
            for &(p, o) in spec.pairs {
                let Some(text) = row.get(p) else { continue };
                let Some(expected) = pspp_number(text) else {
                    continue;
                };
                let what = format!("{}: row {} column {p}", spec.ours, r + 1);
                match (expected, mine.get(o).unwrap_or(&Cell::Empty)) {
                    (None, Cell::Empty) => {}
                    (Some((value, decimals)), Cell::Number(n)) => {
                        let allowed =
                            0.5 * 10f64.powi(-decimals) * (1.0 + 1e-9) + 1e-9 * value.abs();
                        if (n - value).abs() > allowed {
                            problems.push(format!("{what}: PSPP {text}, ours {n}"));
                        }
                        compared += 1;
                    }
                    (_, other) => problems.push(format!("{what}: PSPP {text:?}, ours {other:?}")),
                }
            }
        }
    }
    assert!(
        problems.is_empty(),
        "{case}: {} differences (files kept in {})\n{}",
        problems.len(),
        dir.display(),
        problems.join("\n")
    );
    fs::remove_dir_all(&dir).unwrap();
    compared
}

/// One test, so that the PSPP runs do not overlap.
#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn logistic_regression_and_glm_tables() {
    logistic_regression_tables();
    glm_tables();
}

fn logistic_regression_tables() {
    let cases = [
        (
            "categorical-ci",
            SURVEY,
            "LOGISTIC REGRESSION q5_1 WITH age monthly_spend region /CATEGORICAL=region /PRINT=CI(95) \
             /CRITERIA=BCON(0.0000000001).",
        ),
        (
            "weighted",
            WEIGHTED,
            "LOGISTIC REGRESSION q5_1 WITH age monthly_spend region /CATEGORICAL=region /PRINT=CI(90) \
             /CRITERIA=BCON(0.0000000001).",
        ),
        (
            "cut",
            SURVEY,
            "LOGISTIC REGRESSION q5_2 WITH age q2_recommend /CRITERIA=BCON(0.0000000001) CUT(0.4).",
        ),
        (
            "include",
            SURVEY,
            "LOGISTIC REGRESSION q5_4 WITH q1_satisfaction age /MISSING=INCLUDE \
             /CRITERIA=BCON(0.0000000001).",
        ),
        (
            "two-categorical",
            SURVEY,
            "LOGISTIC REGRESSION q5_3 WITH monthly_spend region gender /CATEGORICAL=region gender \
             /CRITERIA=BCON(0.0000000001).",
        ),
    ];
    let mut total = 0;
    for (case, fixture, syntax) in cases {
        total += compare(&format!("logistic-{case}"), fixture, syntax, LOGISTIC);
    }
    eprintln!("logistic regression: {total} numbers compared");
}

fn glm_tables() {
    // PSPP takes a non-integer weight as it is; the manual rounds frequency weights, so the
    // weighted file is left out. With an empty cell PSPP counts the df of an interaction as if
    // every cell had cases (region by gender has two empty cells), so the crossed designs here
    // have none. Its GLM with MISSING=INCLUDE gives a model sum of squares that its own ONEWAY
    // does not (ours agrees with that ONEWAY), so INCLUDE is compared in LOGISTIC REGRESSION only.
    let cases = [
        ("type3", "GLM monthly_spend BY region q5_1."),
        (
            "type2",
            "GLM monthly_spend BY region q5_1 /METHOD=SSTYPE(2).",
        ),
        (
            "type1",
            "GLM monthly_spend BY region q5_1 /METHOD=SSTYPE(1).",
        ),
        (
            "main-effects",
            "GLM monthly_spend BY region gender /DESIGN=region gender.",
        ),
        ("one-factor", "GLM q1_satisfaction BY region."),
        (
            "three-factors",
            "GLM monthly_spend BY region q5_1 q5_2 /DESIGN=region q5_1 q5_2 region*q5_1 q5_1*q5_2.",
        ),
    ];
    let mut total = 0;
    for (case, syntax) in cases {
        total += compare(&format!("glm-{case}"), SURVEY, syntax, GLM);
    }
    eprintln!("glm: {total} numbers compared");
}
