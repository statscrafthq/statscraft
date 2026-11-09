//! DESCRIPTIVES, CROSSTABS, ADD FILES and MATCH FILES compared with GNU PSPP, run as a black
//! box on the same files and the same syntax. Ignored by default because they need PSPP:
//!
//! ```text
//! STATSCRAFT_PSPP=/path/to/pspp cargo test -p statscraft-engine --test reference_procedures -- --ignored
//! ```
//!
//! Tables: PSPP writes formatted numbers, so its job starts with `SET FORMAT=F20.10`, which
//! gives most statistics ten decimals. Each number of ours must round to what PSPP printed: it
//! may differ by at most half a unit of the last decimal PSPP shows (percentages have one
//! decimal, residuals two and significances three whatever the setting). A cell PSPP shows as a
//! value label is skipped; a blank or "." cell must be empty in ours. Rows are matched in
//! order, except in the chi-square table, where PSPP leaves out some rows (see `chi_square`).
//!
//! Datasets (DESCRIPTIVES /SAVE and the merges): both programs save their result and the two
//! files must agree on the dictionary and on every value. A second input file is made by our
//! own commands first (the `prepare` syntax), so both programs merge the very same files.
//! In syntax, `{dir}` is the folder of the run; PSPP gets relative paths there.

use statscraft_data::{Dataset, Values};
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs};

const SURVEY: &str = "survey_basic.sav";
/// The survey with `weight` as the weight variable.
const WEIGHTED: &str = "survey_basic_phpwriter.sav";
const EDGE: &str = "edge_cases.sav";

static RUNS: AtomicUsize = AtomicUsize::new(0);

/// A folder with the fixture as `data.sav`, or None without PSPP.
fn setup(case: &str, fixture: &str) -> Option<(PathBuf, PathBuf)> {
    let Some(pspp) = env::var_os("STATSCRAFT_PSPP") else {
        eprintln!("{case}: skipped, STATSCRAFT_PSPP is not set");
        return None;
    };
    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    let dir = env::temp_dir().join(format!(
        "statscraft-procedures-{}-{run}",
        std::process::id()
    ));
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

/// Runs the syntax in PSPP and returns its output, which `pspp.csv` also keeps.
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

/// The records of CSV text; quoted fields may hold commas, quotes and line breaks.
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

/// The tables of PSPP's CSV output: title and rows, without the ones we do not produce.
fn pspp_tables(output: &str) -> Vec<(String, Vec<Vec<String>>)> {
    let mut tables: Vec<(String, Vec<Vec<String>>)> = Vec::new();
    let mut current: Option<(String, Vec<Vec<String>>)> = None;
    for record in csv_records(output) {
        let title = record[0].strip_prefix("Table: ").map(str::to_string);
        match (title, &mut current) {
            (Some(title), _) => {
                tables.extend(current.take());
                current = Some((title, Vec::new()));
            }
            (None, Some(_)) if record.len() == 1 && record[0].is_empty() => {
                tables.extend(current.take());
            }
            (None, Some(_)) if record[0].starts_with("Footnote:") => {}
            (None, Some((_, rows))) => {
                // footnote markers such as "Refused[a]" are not part of the label
                let strip =
                    |cell: String| match cell.strip_suffix(']').and_then(|c| c.rsplit_once('[')) {
                        Some((label, marker)) if marker.len() == 1 => label.to_string(),
                        _ => cell,
                    };
                rows.push(record.into_iter().map(strip).collect());
            }
            (None, None) => {}
        }
    }
    tables.extend(current);
    tables.retain(|(title, _)| {
        !matches!(
            title.as_str(),
            "Summary" | "Mapping of Variables to Z-scores" | "Settings"
        )
    });
    tables
}

/// None for a cell that shows no number (a label); Some(None) for a blank or "." cell.
fn pspp_number(text: &str) -> Option<Option<(f64, i32)>> {
    let text = text.trim();
    if text.is_empty() || text == "." {
        return Some(None);
    }
    let text = text.strip_suffix('%').unwrap_or(text);
    let value: f64 = text.parse().ok()?;
    let decimals = text.split_once('.').map_or(0, |(_, d)| d.len() as i32);
    Some(Some((value, decimals)))
}

fn check_cell(what: &str, pspp: &str, ours: &Cell) -> Result<(), String> {
    let Some(expected) = pspp_number(pspp) else {
        return Ok(());
    };
    match (expected, ours) {
        (None, Cell::Empty) => Ok(()),
        (Some((value, decimals)), Cell::Number(number)) => {
            let allowed = 0.5 * 10f64.powi(-decimals) * (1.0 + 1e-9) + 1e-12 * value.abs();
            if (number - value).abs() <= allowed {
                Ok(())
            } else {
                Err(format!("{what}: PSPP {pspp}, ours {number}"))
            }
        }
        _ => Err(format!("{what}: PSPP {pspp:?}, ours {ours:?}")),
    }
}

fn text(cell: &Cell) -> String {
    match cell {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => n.to_string(),
        Cell::Empty => String::new(),
    }
}

/// DESCRIPTIVES: one row per variable and the two listwise rows, in the same order.
/// `listwise` is false for MISSING=VARIABLE, where PSPP counts every case as valid listwise
/// and the manual only the complete cases; the two listwise rows are then not compared.
fn descriptives(pspp: &[Vec<String>], ours: &Table, listwise: bool) -> Vec<String> {
    let mut problems = Vec::new();
    let header = &pspp[0];
    if header[1..] != ours.columns[2..] {
        return vec![format!(
            "columns: PSPP {:?}, ours {:?}",
            &header[1..],
            &ours.columns[2..]
        )];
    }
    if pspp.len() - 1 != ours.rows.len() {
        return vec![format!(
            "rows: PSPP {}, ours {}",
            pspp.len() - 1,
            ours.rows.len()
        )];
    }
    for (row, mine) in pspp[1..].iter().zip(&ours.rows) {
        if !listwise && row[0].ends_with("N (listwise)") {
            continue;
        }
        for (column, cell) in row[1..].iter().enumerate() {
            let what = format!("{} {}", row[0], header[column + 1]);
            problems.extend(check_cell(&what, cell, &mine[column + 2]).err());
        }
    }
    problems
}

/// The crosstab: two header rows, then the rows in our order.
fn crosstab(pspp: &[Vec<String>], ours: &Table) -> Vec<String> {
    let headers = ours.row_headers;
    let layers = headers - 2;
    let categories: Vec<&String> = pspp[1][2 * layers + 3..].iter().collect();
    let mine: Vec<&String> = ours.columns[headers..].iter().collect();
    let last = categories.len() - 1;
    if categories[..last] != mine[..mine.len() - 1] {
        return vec![format!("columns: PSPP {categories:?}, ours {mine:?}")];
    }
    if pspp.len() - 2 != ours.rows.len() {
        return vec![format!(
            "rows: PSPP {}, ours {}",
            pspp.len() - 2,
            ours.rows.len()
        )];
    }
    let mut problems = Vec::new();
    for (row, mine) in pspp[2..].iter().zip(&ours.rows) {
        let category = match row[2 * layers].as_str() {
            "Total" => "Total",
            _ => row[2 * layers + 1].as_str(),
        };
        let statistic = &row[2 * layers + 2];
        if !category.is_empty() && category != text(&mine[layers]) {
            problems.push(format!(
                "category: PSPP {category}, ours {:?}",
                mine[layers]
            ));
        }
        if *statistic != text(&mine[layers + 1]) {
            problems.push(format!(
                "statistic: PSPP {statistic}, ours {:?}",
                mine[layers + 1]
            ));
        }
        let last = row.len() - 2 * layers - 4;
        for (column, cell) in row[2 * layers + 3..].iter().enumerate() {
            // PSPP shows shares as the expected count of a total; the manual's expected counts
            // add up to the observed one there, so those cells are not compared
            if statistic == "Expected" && (text(&mine[layers]) == "Total" || column == last) {
                continue;
            }
            let what = format!("{category} {statistic} column {}", column + 1);
            problems.extend(check_cell(&what, cell, &mine[headers + column]).err());
        }
    }
    problems
}

/// The chi-square table, matched by layer and test name: PSPP leaves out the Pearson and
/// likelihood ratio rows when they are 0 and sometimes the row of the case count.
fn chi_square(pspp: &[Vec<String>], ours: &Table) -> Vec<String> {
    let layers = ours.row_headers - 1;
    let header = &pspp[0];
    let mut mine: HashMap<String, &Vec<Cell>> = HashMap::new();
    for row in &ours.rows {
        let key: Vec<String> = row[..=layers].iter().map(text).collect();
        mine.insert(key.join("|"), row);
    }
    let mut problems = Vec::new();
    let mut layer = vec![String::new(); layers];
    for row in &pspp[1..] {
        for (l, value) in layer.iter_mut().enumerate() {
            if !row[2 * l + 1].is_empty() {
                *value = row[2 * l + 1].clone();
            }
        }
        let name = &row[2 * layers];
        let key = format!(
            "{}{}{name}",
            layer.join("|"),
            if layers > 0 { "|" } else { "" }
        );
        let Some(found) = mine.get(&key) else {
            problems.push(format!("PSPP row {key} is missing in ours"));
            continue;
        };
        for (column, cell) in row[2 * layers + 1..].iter().enumerate() {
            // PSPP can sum a two-sided Fisher significance above 1
            if pspp_number(cell).flatten().is_some_and(|(v, _)| v > 1.0) {
                continue;
            }
            let what = format!("{key} {}", header[2 * layers + 1 + column]);
            problems.extend(check_cell(&what, cell, &found[layers + 1 + column]).err());
        }
    }
    problems
}

/// Runs `syntax` after GET of the fixture in both programs and compares the tables.
fn compare_tables(case: &str, fixture: &str, syntax: &str) {
    let Some((program, dir)) = setup(case, fixture) else {
        return;
    };
    let syntax = format!("GET FILE='{{dir}}data.sav'.\n{syntax}\n");
    let output = pspp(case, &program, &dir, &syntax);
    let theirs = pspp_tables(&output);
    let mine = ours(case, &dir, &syntax);
    assert_eq!(
        theirs.len(),
        mine.len(),
        "{case}: PSPP made {} tables, we {}\n{output}",
        theirs.len(),
        mine.len()
    );
    let mut problems = Vec::new();
    for ((title, rows), table) in theirs.iter().zip(&mine) {
        let found = match title.as_str() {
            "Descriptive Statistics" => descriptives(rows, table, syntax.contains("LISTWISE")),
            "Chi-Square Tests" => chi_square(rows, table),
            _ => crosstab(rows, table),
        };
        problems.extend(found.into_iter().map(|p| format!("{}: {p}", table.title)));
    }
    assert!(
        problems.is_empty(),
        "{case}: {} differences (files kept in {})\n{}",
        problems.len(),
        dir.display(),
        problems[..problems.len().min(25)].join("\n")
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// Runs `prepare` in our engine only (to write input files), then `syntax` after GET of the
/// fixture in both programs, and compares the datasets both end with.
fn compare_data(case: &str, fixture: &str, prepare: &str, syntax: &str) {
    let Some((program, dir)) = setup(case, fixture) else {
        return;
    };
    ours(case, &dir, prepare);
    let job = |out: &str| {
        format!("GET FILE='{{dir}}data.sav'.\n{syntax}\nSAVE OUTFILE='{{dir}}{out}final.sav'.\n")
    };
    let output = pspp(case, &program, &dir, &job("pspp_"));
    ours(case, &dir, &job("ours_"));
    let read = |prefix: &str| Dataset::read_sav(dir.join(format!("{prefix}final.sav"))).unwrap();
    let problems = differences(&read("pspp_"), &read("ours_"));
    assert!(
        problems.is_empty(),
        "{case}: {} differences (files kept in {})\n{}\n{output}",
        problems.len(),
        dir.display(),
        problems[..problems.len().min(25)].join("\n")
    );
    fs::remove_dir_all(&dir).unwrap();
}

/// What the two results must agree on in the dictionary, as (what, value) lines.
fn dictionary(data: &Dataset) -> Vec<(String, String)> {
    let described = data.describe();
    let names: Vec<&str> = data.variable_names().collect();
    let mut lines = vec![
        ("cases".to_string(), described.case_count.to_string()),
        ("variables".to_string(), names.join(" ")),
        (
            "weight".to_string(),
            format!("{:?}", described.weight_variable),
        ),
    ];
    for v in &described.variables {
        let mut labels: Vec<_> = v.value_labels.iter().collect();
        labels.sort_by_key(|(value, _)| value.as_str());
        let fields = [
            ("type", format!("{} {:?}", v.kind, v.width)),
            ("format", v.format.clone()),
            ("label", format!("{:?}", v.label)),
            ("measure", v.measure.to_string()),
            ("value labels", format!("{labels:?}")),
            (
                "missing",
                format!("{:?} {:?}", v.missing_values, v.missing_range),
            ),
        ];
        lines.extend(fields.map(|(field, value)| (format!("{} {field}", v.name), value)));
    }
    lines
}

/// Dictionary differences, or when there are none the values that differ.
fn differences(pspp: &Dataset, ours: &Dataset) -> Vec<String> {
    let mut found: Vec<String> = dictionary(pspp)
        .into_iter()
        .zip(dictionary(ours))
        .filter(|(a, b)| a != b)
        .map(|(a, b)| format!("{}: PSPP {}, ours {}", a.0, a.1, b.1))
        .collect();
    if !found.is_empty() {
        return found;
    }
    for (a, b) in pspp.variables().zip(ours.variables()) {
        for case in 0..pspp.case_count() {
            let (same, shown) = match (a.values(), b.values()) {
                (Values::Numeric(x), Values::Numeric(y)) => {
                    let (x, y) = (x.get(case), y.get(case));
                    let same = match (x, y) {
                        (Some(x), Some(y)) => (x - y).abs() <= 1e-9 * x.abs().max(1.0),
                        _ => x == y,
                    };
                    (same, format!("PSPP {x:?}, ours {y:?}"))
                }
                (Values::String(x), Values::String(y)) => {
                    let (x, y) = (x.get(case), y.get(case));
                    (x == y, format!("PSPP {x:?}, ours {y:?}"))
                }
                _ => unreachable!("the types were compared above"),
            };
            if !same {
                found.push(format!("{} case {}: {shown}", a.name(), case + 1));
            }
        }
    }
    found
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn descriptives_tables() {
    let three = "age monthly_spend q1_satisfaction";
    let cases = [
        ("default", SURVEY, format!("DESCRIPTIVES {three}.")),
        ("all", SURVEY, format!("DESCRIPTIVES VARIABLES={three} q2_recommend /STATISTICS=ALL.")),
        ("all-weighted", WEIGHTED, format!("DESCRIPTIVES /VARIABLES={three} q2_recommend /STATISTICS=ALL.")),
        ("listwise", SURVEY, format!("DESCRIPTIVES {three} /MISSING=LISTWISE /STATISTICS=ALL.")),
        ("listwise-weighted", WEIGHTED, format!("DESCRIPTIVES {three} /MISSING=LISTWISE /STATISTICS=ALL.")),
        ("include", SURVEY, "DESCRIPTIVES q1_satisfaction q2_recommend monthly_spend /MISSING=INCLUDE /STATISTICS=ALL.".into()),
        ("listwise-include-weighted", WEIGHTED, "DESCRIPTIVES q1_satisfaction q2_recommend age /MISSING=LISTWISE INCLUDE /STATISTICS=DEFAULT SUM.".into()),
        ("single-statistics", SURVEY, "DESCRIPTIVES age /STATISTICS=SUM.\nDESCRIPTIVES age /STATISTICS=SEMEAN VARIANCE.\nDESCRIPTIVES age monthly_spend /STATISTICS=KURTOSIS.\nDESCRIPTIVES weight /STATISTICS=SKEWNESS RANGE MIN.".into()),
        ("to-list-weighted", WEIGHTED, "DESCRIPTIVES q5_1 TO q5_5 q6_mention1 TO q6_mention3 /STATISTICS=ALL.".into()),
        ("edge-cases", EDGE, "DESCRIPTIVES all_missing three_missing range_plus_one /STATISTICS=ALL.".into()),
        ("small-groups", SURVEY, "SELECT IF region = 5 AND gender = 3.\nDESCRIPTIVES age q2_recommend monthly_spend /STATISTICS=ALL.".into()),
        ("three-cases", SURVEY, "SELECT IF resp_id <= 1003.\nDESCRIPTIVES age q2_recommend /STATISTICS=ALL.".into()),
        ("unusable-weights", WEIGHTED, "RECODE weight (LO THRU 0.6 = 0) (0.6 THRU 0.7 = -1).\nDESCRIPTIVES age monthly_spend /STATISTICS=ALL.".into()),
        ("missing-weights", WEIGHTED, "RECODE weight (LO THRU 0.8 = SYSMIS).\nMISSING VALUES weight (2 THRU HI).\nDESCRIPTIVES age monthly_spend q1_satisfaction /STATISTICS=ALL.".into()),
        ("computed", SURVEY, "COMPUTE spend_per_year = monthly_spend * 12.\nCOMPUTE big = age * 1e6 + 0.123.\nDESCRIPTIVES spend_per_year big /STATISTICS=ALL.".into()),
        ("weight-off", WEIGHTED, "WEIGHT OFF.\nDESCRIPTIVES age weight /STATISTICS=MEAN STDDEV VARIANCE.".into()),
    ];
    for (case, fixture, syntax) in &cases {
        compare_tables(&format!("descriptives-{case}"), fixture, syntax);
    }
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn descriptives_save() {
    let cases = [
        (
            "survey",
            SURVEY,
            "DESCRIPTIVES age monthly_spend q1_satisfaction q2_recommend /SAVE.",
        ),
        (
            "weighted",
            WEIGHTED,
            "DESCRIPTIVES age monthly_spend q1_satisfaction /SAVE.",
        ),
        (
            "names",
            SURVEY,
            "RENAME VARIABLES (resp_id = Zage).\nDESCRIPTIVES age monthly_spend (spend_z) /SAVE.\nDESCRIPTIVES age q2_recommend /SAVE.",
        ),
        (
            "include",
            WEIGHTED,
            "DESCRIPTIVES q1_satisfaction q2_recommend /MISSING=INCLUDE /SAVE.",
        ),
    ];
    // our z-score labels follow the manual (Zscore: plus the label) and PSPP's do not, so the
    // labels are cleared before the comparison
    let clear = |names: &str| {
        let labels: Vec<String> = names.split(' ').map(|n| format!("{n} ''")).collect();
        format!("\nVARIABLE LABELS {}.", labels.join(" "))
    };
    let labels = [
        clear("Zage Zmonthly_spend Zq1_satisfaction Zq2_recommend"),
        clear("Zage Zmonthly_spend Zq1_satisfaction"),
        clear("ZSC001 spend_z ZSC002 Zq2_recommend"),
        clear("Zq1_satisfaction Zq2_recommend"),
    ];
    for ((case, fixture, syntax), labels) in cases.into_iter().zip(labels) {
        let syntax = format!("{syntax}{labels}");
        compare_data(&format!("descriptives-save-{case}"), fixture, "", &syntax);
    }
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn crosstabs_tables() {
    let cases = [
        ("counts", SURVEY, "CROSSTABS /TABLES=gender BY region."),
        (
            "percentages-chisq",
            SURVEY,
            "CROSSTABS /TABLES=gender BY q5_1 /CELLS=COUNT ROW COLUMN TOTAL /STATISTICS=CHISQ.",
        ),
        (
            "residuals",
            SURVEY,
            "CROSSTABS /TABLES=gender BY region /CELLS=EXPECTED RESID SRESID ASRESID.",
        ),
        (
            "two-by-two",
            SURVEY,
            "CROSSTABS /TABLES=q5_2 BY q5_1 /CELLS=COUNT EXPECTED /STATISTICS=CHISQ.",
        ),
        (
            "two-by-two-weighted",
            WEIGHTED,
            "CROSSTABS /TABLES=q5_2 BY q5_1 /CELLS=COUNT EXPECTED ASRESID /STATISTICS=CHISQ.",
        ),
        (
            "weighted-all-cells",
            WEIGHTED,
            "CROSSTABS /TABLES=gender BY region /CELLS=COUNT ROW COLUMN TOTAL /STATISTICS=CHISQ.\nCROSSTABS /TABLES=gender BY region /CELLS=EXPECTED RESID SRESID ASRESID.",
        ),
        (
            "user-missing",
            SURVEY,
            "CROSSTABS /TABLES=q1_satisfaction BY q6_mention2 /STATISTICS=CHISQ /CELLS=COUNT ROW.",
        ),
        // PSPP turns the columns round with DVALUE too, the manual only the rows, so DVALUE is
        // tested in procedures.rs and not here
        (
            "include",
            SURVEY,
            "CROSSTABS /TABLES=q1_satisfaction BY q6_mention2 /MISSING=INCLUDE /CELLS=COUNT COLUMN /STATISTICS=CHISQ.",
        ),
        (
            "strings",
            SURVEY,
            "CROSSTABS /TABLES=plan BY gender /CELLS=COUNT ROW /STATISTICS=CHISQ.",
        ),
        (
            "several-variables",
            SURVEY,
            "CROSSTABS /TABLES=gender q5_1 BY region q5_2 /STATISTICS=CHISQ.",
        ),
        (
            "layer",
            SURVEY,
            "CROSSTABS /TABLES=q5_1 BY q5_2 BY gender /CELLS=COUNT ROW /STATISTICS=CHISQ.",
        ),
        (
            "two-layers",
            SURVEY,
            "CROSSTABS /TABLES=q5_1 BY q5_2 BY q5_3 BY gender /CELLS=COUNT TOTAL.",
        ),
        (
            "layers-differ",
            SURVEY,
            "SELECT IF resp_id <= 1012.\nCROSSTABS /TABLES=q5_1 BY gender BY q5_2 /CELLS=COUNT EXPECTED /STATISTICS=CHISQ.",
        ),
        (
            "layer-weighted",
            WEIGHTED,
            "CROSSTABS /TABLES=region BY q5_4 BY gender /CELLS=COUNT COLUMN SRESID /STATISTICS=CHISQ.",
        ),
        (
            "repeated-tables",
            SURVEY,
            "CROSSTABS /TABLES=q5_1 BY q5_2 /TABLES=gender BY q5_3 /STATISTICS=CHISQ.",
        ),
        (
            "missing-range-weighted",
            WEIGHTED,
            "CROSSTABS /TABLES=q2_recommend BY region /CELLS=COUNT ROW /STATISTICS=CHISQ.",
        ),
        (
            "unusable-weights",
            WEIGHTED,
            "RECODE weight (LO THRU 0.6 = 0) (0.6 THRU 0.7 = -1).\nCROSSTABS /TABLES=gender BY q5_5 /CELLS=COUNT EXPECTED /STATISTICS=CHISQ.",
        ),
        (
            "one-column",
            SURVEY,
            "SELECT IF gender = 3.\nCROSSTABS /TABLES=q5_1 BY gender /STATISTICS=CHISQ.",
        ),
    ];
    for (case, fixture, syntax) in cases {
        compare_tables(&format!("crosstabs-{case}"), fixture, syntax);
    }
}

/// Input files for the merges, written by our engine: `b.sav` (cases 1151 to 1200 with a
/// changed dictionary), `sorted_a.sav` and `sorted_b.sav` (halves sorted by region), `extra.sav`
/// (some cases, a new and a shared variable, sorted by resp_id), `lookup.sav` (one case per
/// region) and `plans.sav` (one case per plan, with a duplicate key).
const PREPARE: &str = "GET FILE='{dir}data.sav'.
SELECT IF resp_id > 1150.
VARIABLE LABELS age 'Age, second file' gender ''.
VALUE LABELS age 18 'eighteen' / gender.
MISSING VALUES age (49) gender (3).
FORMATS age (F8.3).
STRING plan_long (A30).
COMPUTE plan_long = CONCAT(RTRIM(plan), ' plan').
COMPUTE extra = age * 10.
DELETE VARIABLES q3_comment signup_date.
SAVE OUTFILE='{dir}b.sav'.
GET FILE='{dir}data.sav'.
SELECT IF resp_id <= 1100.
SORT CASES BY region.
SAVE OUTFILE='{dir}sorted_a.sav'.
GET FILE='{dir}data.sav'.
SELECT IF resp_id > 1100.
SORT CASES BY region.
SAVE OUTFILE='{dir}sorted_b.sav'.
GET FILE='{dir}data.sav'.
SELECT IF MOD(resp_id, 3) = 0 OR resp_id > 1190.
COMPUTE age = age + 1000.
COMPUTE bonus = resp_id / 100.
DELETE VARIABLES gender region TO q3_comment q5_1 TO weight.
SAVE OUTFILE='{dir}extra.sav'.
GET FILE='{dir}data.sav'.
AGGREGATE /OUTFILE='{dir}lookup.sav' /BREAK=region /mean_age 'Mean age' = MEAN(age) /n = N.
GET FILE='{dir}data.sav'.
AGGREGATE /OUTFILE='{dir}plans.sav' /BREAK=plan /plan_spend = MEAN(monthly_spend).
GET FILE='{dir}plans.sav'.
SORT CASES BY plan.
COMPUTE plan_spend = plan_spend * 2.
SAVE OUTFILE='{dir}plans2.sav'.
ADD FILES /FILE='{dir}plans.sav' /FILE='{dir}plans2.sav' /BY plan.
SAVE OUTFILE='{dir}plans_dup.sav'.
";

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn merges() {
    let cases = [
        (
            "add-two-files",
            SURVEY,
            "SELECT IF resp_id <= 1020.\nADD FILES /FILE=* /FILE='{dir}b.sav'.",
        ),
        (
            "add-dictionaries-meet",
            SURVEY,
            "VARIABLE LABELS region ''.\nADD FILES /FILE=* /FILE='{dir}b.sav'.",
        ),
        // PSPP drops the weight also when every file has it, the manual only when one file
        // lacks it; with the active file unweighted both turn it off
        (
            "add-second-file-first",
            WEIGHTED,
            "WEIGHT OFF.\nADD FILES /FILE='{dir}b.sav' /FILE=*.",
        ),
        (
            "add-by",
            SURVEY,
            "GET FILE='{dir}sorted_a.sav'.\nADD FILES /FILE=* /FILE='{dir}sorted_b.sav' /BY region.",
        ),
        (
            "add-rename-keep-drop",
            WEIGHTED,
            "ADD FILES /FILE=* /RENAME=(age gender = years sex) /FILE='{dir}b.sav' /RENAME=(age=years)\n  /DROP=q3_comment /KEEP=resp_id years sex extra plan_long ALL /DROP=weight.",
        ),
        (
            "add-three-files",
            SURVEY,
            "ADD FILES /FILE='{dir}b.sav' /FILE=* /FILE='{dir}b.sav' /KEEP=resp_id age extra.",
        ),
        (
            "match-by-key",
            SURVEY,
            "MATCH FILES /FILE=* /FILE='{dir}extra.sav' /BY resp_id.",
        ),
        (
            "match-other-file-first",
            WEIGHTED,
            "MATCH FILES /FILE='{dir}extra.sav' /FILE=* /BY resp_id.",
        ),
        (
            "match-table",
            SURVEY,
            "SORT CASES BY region.\nMATCH FILES /FILE=* /TABLE='{dir}lookup.sav' /BY region.",
        ),
        (
            "match-table-first",
            SURVEY,
            "SORT CASES BY region.\nMATCH FILES /TABLE='{dir}lookup.sav' /FILE=* /BY region.",
        ),
        (
            "match-duplicate-keys",
            SURVEY,
            "SORT CASES BY region.\nMATCH FILES /FILE=* /RENAME=(resp_id = id1) /FILE='{dir}sorted_b.sav' /BY region\n  /KEEP=region id1 resp_id age.",
        ),
        (
            "match-by-position",
            WEIGHTED,
            "MATCH FILES /FILE=* /FILE='{dir}b.sav' /RENAME=(age plan = age_b plan_b).",
        ),
        // PSPP takes the first case of a key a table holds twice; the manual forbids such a
        // table, so the comparison uses the one without duplicates
        (
            "match-string-key-table",
            SURVEY,
            "SORT CASES BY plan resp_id.\nMATCH FILES /FILE=* /TABLE='{dir}plans.sav' /BY plan /DROP=q3_comment.",
        ),
        (
            "match-two-keys",
            SURVEY,
            "SORT CASES BY region resp_id.\nMATCH FILES /FILE=* /FILE='{dir}sorted_b.sav' /RENAME=(age = age2) /BY region resp_id /KEEP=resp_id region age age2.",
        ),
        (
            "match-and-add",
            SURVEY,
            "MATCH FILES /FILE=* /FILE='{dir}extra.sav' /BY resp_id.\nADD FILES /FILE=* /FILE='{dir}extra.sav'.",
        ),
    ];
    for (case, fixture, syntax) in cases {
        compare_data(&format!("merge-{case}"), fixture, PREPARE, syntax);
    }
}
