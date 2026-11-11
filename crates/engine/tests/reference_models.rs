//! REGRESSION and RELIABILITY compared with GNU PSPP, run as a black box on the same files and
//! the same syntax. Ignored by default because they need PSPP:
//!
//! ```text
//! STATSCRAFT_PSPP=/path/to/pspp cargo test -p statscraft-engine --test reference_models -- --ignored
//! ```
//!
//! PSPP's REGRESSION knows VARIABLES, DEPENDENT, METHOD=ENTER without a list, STATISTICS (R,
//! COEFF, ANOVA, CI, TOL; its BCOV table is labelled one variable off), ORIGIN and SAVE PRED
//! RESID; its RELIABILITY knows ALPHA, SPLIT(n), SCALE, MISSING and SUMMARY=TOTAL, without
//! weights. The cases stay
//! within that. As in reference_procedures.rs, PSPP prints with `SET FORMAT=F20.10` and each of
//! our numbers must round to what it printed. Where PSPP prints a 0 for the constant's Beta,
//! ours is empty, as the manual defines no Beta for it.

use statscraft_data::{Dataset, Values};
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs};

const SURVEY: &str = "survey_basic.sav";
/// The survey with `weight` as the weight variable.
const WEIGHTED: &str = "survey_basic_phpwriter.sav";
/// Display formats with ten decimals, so PSPP prints the statistics precisely enough.
const PRECISE: &str =
    "FORMATS age monthly_spend q1_satisfaction q2_recommend q5_1 TO q5_5 (F20.10).";

static RUNS: AtomicUsize = AtomicUsize::new(0);

fn setup(case: &str, fixture: &str) -> Option<(PathBuf, PathBuf)> {
    let Some(pspp) = env::var_os("STATSCRAFT_PSPP") else {
        eprintln!("{case}: skipped, STATSCRAFT_PSPP is not set");
        return None;
    };
    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    let dir = env::temp_dir().join(format!("statscraft-models-{}-{run}", std::process::id()));
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

/// The header rows PSPP writes and the pairs (PSPP column, our column) compared.
type Layout = (usize, Vec<(usize, usize)>);

/// None for a table PSPP does not make.
fn layout(ours: &Table) -> Option<Layout> {
    let by_name = |pspp: &[&str]| -> Vec<(usize, usize)> {
        pspp.iter()
            .enumerate()
            .filter(|(_, name)| !name.is_empty())
            .map(|(p, name)| {
                let o = ours.columns.iter().position(|c| c == name);
                (
                    p,
                    o.unwrap_or_else(|| panic!("{}: no column {name}", ours.title)),
                )
            })
            .collect()
    };
    Some(match ours.title.as_str() {
        "Model Summary" => (1, (0..4).map(|c| (c, c + 1)).collect()),
        "ANOVA" => (1, (1..6).map(|c| (c, c + 1)).collect()),
        "Coefficients" => {
            let mut names = vec!["", "B", "Std. Error", "Beta", "t", "Sig."];
            if ours.columns.iter().any(|c| c == "Lower Bound") {
                names.extend(["Lower Bound", "Upper Bound"]);
            }
            // PSPP's Tolerance is not 1 - R squared of the variable on the others (an
            // independent computation agrees with ours, see regression.rs), so it is left out
            (2, by_name(&names))
        }
        "Case Processing Summary" => (1, vec![(1, 1), (2, 2)]),
        "Reliability Statistics" if ours.row_headers == 3 => (0, vec![(3, 3)]),
        "Reliability Statistics" => (1, vec![(0, 0), (1, 2)]),
        "Item-Total Statistics" => (1, vec![(1, 1), (2, 2), (3, 3), (4, ours.columns.len() - 1)]),
        _ => return None,
    })
}

fn compare(case: &str, fixture: &str, syntax: &str) {
    let Some((program, dir)) = setup(case, fixture) else {
        return;
    };
    let syntax = format!("GET FILE='{{dir}}data.sav'.\n{PRECISE}\n{syntax}\n");
    let output = pspp(case, &program, &dir, &syntax);
    let theirs = pspp_tables(&output);
    let mine: Vec<(Table, Layout)> = ours(case, &dir, &syntax)
        .into_iter()
        .filter_map(|t| layout(&t).map(|l| (t, l)))
        .collect();
    assert_eq!(
        theirs.len(),
        mine.len(),
        "{case}: PSPP made {} tables, we {}\n{output}",
        theirs.len(),
        mine.len()
    );
    let mut problems = Vec::new();
    for ((_, rows), (table, (header_rows, columns))) in theirs.iter().zip(&mine) {
        let data = &rows[*header_rows..];
        if data.len() != table.rows.len() {
            problems.push(format!(
                "{}: rows: PSPP {}, ours {}",
                table.title,
                data.len(),
                table.rows.len()
            ));
            continue;
        }
        for (r, (row, mine)) in data.iter().zip(&table.rows).enumerate() {
            for &(p, o) in columns {
                // with ORIGIN PSPP takes C - p - 1 degrees of freedom for the adjusted R square
                // and the t of the intervals (its ANOVA has C - p, as the manual everywhere)
                let origin_df = match table.title.as_str() {
                    "Model Summary" => p == 2,
                    "Coefficients" => p == 6 || p == 7,
                    _ => false,
                };
                if case.contains("origin") && origin_df {
                    continue;
                }
                let Some(expected) = pspp_number(&row[p]) else {
                    continue;
                };
                let what = format!("{}: row {} column {p}", table.title, r + 1);
                match (expected, &mine[o]) {
                    (None, Cell::Empty) => {}
                    // the constant has no Beta; PSPP prints 0
                    (Some((v, _)), Cell::Empty) if v == 0.0 && row[0] == "(Constant)" => {}
                    (Some((value, decimals)), Cell::Number(n)) => {
                        let allowed =
                            0.5 * 10f64.powi(-decimals) * (1.0 + 1e-9) + 1e-12 * value.abs();
                        if (n - value).abs() > allowed {
                            problems.push(format!("{what}: PSPP {}, ours {n}", row[p]));
                        }
                    }
                    (_, other) => {
                        problems.push(format!("{what}: PSPP {:?}, ours {other:?}", row[p]))
                    }
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
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn regression_tables() {
    let cases = [
        (
            "ci-tolerance",
            SURVEY,
            "REGRESSION /VARIABLES=age q1_satisfaction q2_recommend /STATISTICS=R COEFF ANOVA CI(90) TOL \
             /DEPENDENT=monthly_spend /METHOD=ENTER.",
        ),
        (
            "weighted",
            WEIGHTED,
            "REGRESSION /VARIABLES=age q1_satisfaction q2_recommend /DEPENDENT=monthly_spend /METHOD=ENTER.",
        ),
        (
            "two-dependents",
            SURVEY,
            "REGRESSION /VARIABLES=age monthly_spend /DEPENDENT=q1_satisfaction q2_recommend /METHOD=ENTER.",
        ),
        (
            "two-dependents-weighted",
            WEIGHTED,
            "REGRESSION /VARIABLES=age q5_1 q5_2 /STATISTICS=R COEFF ANOVA CI TOL \
             /DEPENDENT=q2_recommend monthly_spend /METHOD=ENTER.",
        ),
        (
            "origin",
            SURVEY,
            "REGRESSION /VARIABLES=age q1_satisfaction /ORIGIN /DEPENDENT=monthly_spend /METHOD=ENTER.",
        ),
        (
            "origin-weighted",
            WEIGHTED,
            "REGRESSION /VARIABLES=q1_satisfaction q2_recommend /STATISTICS=R COEFF ANOVA CI TOL \
             /ORIGIN /DEPENDENT=age /METHOD=ENTER.",
        ),
        (
            "one-predictor",
            SURVEY,
            "REGRESSION /VARIABLES=q2_recommend /DEPENDENT=q1_satisfaction /METHOD=ENTER.",
        ),
    ];
    for (case, fixture, syntax) in cases {
        compare(&format!("regression-{case}"), fixture, syntax);
    }
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn reliability_tables() {
    // PSPP's RELIABILITY leaves the weight out, which the manual uses (reliability.rs checks
    // weights against replicated cases), so every case here is unweighted
    let cases = [
        (
            "alpha",
            SURVEY,
            "RELIABILITY /VARIABLES=q5_1 TO q5_5 /SUMMARY=TOTAL.",
        ),
        // PSPP takes SPLIT(n) as the size of the first part and the manual as the size of the
        // second, so the parts are of equal size here
        (
            "split",
            SURVEY,
            "RELIABILITY /VARIABLES=q5_1 TO q5_4 /MODEL=SPLIT(2) /SUMMARY=TOTAL.",
        ),
        (
            "missing",
            SURVEY,
            "RELIABILITY /VARIABLES=q1_satisfaction q2_recommend age /SUMMARY=TOTAL.",
        ),
        (
            "include",
            SURVEY,
            "RELIABILITY /VARIABLES=q1_satisfaction q2_recommend age /MISSING=INCLUDE /SUMMARY=TOTAL.",
        ),
        (
            "scale",
            SURVEY,
            "RELIABILITY /VARIABLES=q1_satisfaction q2_recommend age monthly_spend \
             /SCALE('part')=q1_satisfaction q2_recommend age /SUMMARY=TOTAL.",
        ),
    ];
    for (case, fixture, syntax) in cases {
        compare(&format!("reliability-{case}"), fixture, syntax);
    }
}

/// SAVE PRED RESID: PSPP names them PRED1 and RES1, the manual PRE_1 and RES_1. A case missing
/// on a predictor gets no predicted value in either program.
#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn regression_saved_values() {
    for (case, fixture, regression) in [
        (
            "save",
            SURVEY,
            "REGRESSION /VARIABLES=age monthly_spend /DEPENDENT=q1_satisfaction /METHOD=ENTER /SAVE=PRED RESID.",
        ),
        (
            "save-weighted",
            WEIGHTED,
            "REGRESSION /VARIABLES=age q2_recommend /DEPENDENT=monthly_spend /METHOD=ENTER /SAVE=PRED RESID.",
        ),
    ] {
        let Some((program, dir)) = setup(case, fixture) else {
            return;
        };
        let job = |out: &str| {
            format!("GET FILE='{{dir}}data.sav'.\n{regression}\nSAVE OUTFILE='{{dir}}{out}.sav'.\n")
        };
        pspp(case, &program, &dir, &job("pspp"));
        ours(case, &dir, &job("ours"));
        let read = |name: &str| Dataset::read_sav(dir.join(format!("{name}.sav"))).unwrap();
        let (theirs, mine) = (read("pspp"), read("ours"));
        let values = |data: &Dataset, name: &str| -> Vec<Option<f64>> {
            match data.variable(name).unwrap().values() {
                Values::Numeric(v) => (0..data.case_count()).map(|k| v.get(k)).collect(),
                Values::String(_) => panic!("{name} is a string"),
            }
        };
        let mut compared = 0;
        for (theirs_name, our_name) in [("PRED1", "PRE_1"), ("RES1", "RES_1")] {
            let (a, b) = (values(&theirs, theirs_name), values(&mine, our_name));
            for (k, (x, y)) in a.iter().zip(&b).enumerate() {
                // PSPP predicts from a system-missing predictor as if it were a number
                let x = x.filter(|x| x.abs() < 1e300);
                match (x, y) {
                    (Some(x), Some(y)) => {
                        assert!(
                            (x - y).abs() <= 1e-9 * x.abs().max(1.0),
                            "{case}: {our_name}[{k}]: PSPP {x}, ours {y}"
                        );
                        compared += 1;
                    }
                    (None, None) => {}
                    // PSPP takes a user-missing value of the dependent or a predictor as valid
                    // here; the manual needs valid values
                    (Some(_), None) => {}
                    _ => panic!("{case}: {our_name}[{k}]: PSPP {x:?}, ours {y:?}"),
                }
            }
        }
        assert!(compared > 100, "{case}: {compared}");
        fs::remove_dir_all(&dir).unwrap();
    }
}
