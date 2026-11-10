//! DESCRIPTIVES, CROSSTABS, MEANS, T-TEST, ONEWAY, CORRELATIONS, EXAMINE, ADD FILES and MATCH FILES
//! compared with GNU PSPP, run as a black
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
            "Summary" | "Mapping of Variables to Z-scores" | "Settings" | "Split Values"
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
    // in scientific notation such as 2.1411656982E-005 the exponent moves the last digit
    let (mantissa, exponent) = match text.split_once(['E', 'e']) {
        Some((m, e)) => (m, e.parse::<i32>().ok()?),
        None => (text, 0),
    };
    let decimals = mantissa.split_once('.').map_or(0, |(_, d)| d.len() as i32) - exponent;
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

/// How a table of MEANS, T-TEST, ONEWAY or CORRELATIONS lines up with PSPP's: the header rows
/// PSPP writes, and for every compared column its index in PSPP's rows and in ours.
struct Layout {
    header_rows: usize,
    columns: Vec<(usize, usize)>,
}

/// The same columns from `first` on in both programs, `count` of them.
fn straight(header_rows: usize, first: usize, count: usize) -> Layout {
    Layout {
        header_rows,
        columns: (first..first + count).map(|c| (c, c)).collect(),
    }
}

/// None for one of our tables that PSPP does not make (effect sizes, MEANS ANOVA, the robust
/// tests); they are left out before the tables are paired.
fn layout(title: &str, ours: &Table) -> Option<Layout> {
    let values = ours.columns.len() - ours.row_headers;
    Some(match title {
        "Case Processing Summary" => straight(3, 1, 6),
        t if t.starts_with("Report") => straight(1, ours.row_headers, values),
        "One-Sample Statistics" => straight(1, 1, 4),
        t if t.starts_with("One-Sample Test") => straight(3, 1, 6),
        "Group Statistics" => straight(1, 2, 4),
        "Independent Samples Test" => straight(3, 2, 9),
        // PSPP shows N before the mean, we follow the manual's order
        "Paired Samples Statistics" => Layout {
            header_rows: 1,
            columns: vec![(2, 3), (3, 2), (4, 4), (5, 5)],
        },
        "Paired Samples Correlations" => straight(1, 2, 3),
        "Paired Samples Test" => straight(3, 2, 8),
        "Descriptive Statistics" => straight(1, 1, 3),
        "Correlations" => straight(1, 2, values),
        "Descriptives" => straight(2, 2, 8),
        "Test of Homogeneity of Variances" => straight(1, 1, 4),
        "ANOVA" => straight(1, 2, 5),
        "Contrast Coefficients" => straight(2, 1, values),
        "Contrast Tests" => straight(1, 2, 6),
        t if t.starts_with("Multiple Comparisons") => straight(2, 3, 5),
        _ => return None,
    })
}

/// Compares the rows of one table cell by cell through `layout`.
fn positional(pspp: &[Vec<String>], ours: &Table, layout: &Layout) -> Vec<String> {
    let data = &pspp[layout.header_rows.min(pspp.len())..];
    if data.len() != ours.rows.len() {
        return vec![format!(
            "rows: PSPP {}, ours {}",
            data.len(),
            ours.rows.len()
        )];
    }
    let mut problems = Vec::new();
    let mut test = String::new();
    for (r, (row, mine)) in data.iter().zip(&ours.rows).enumerate() {
        if !row[0].is_empty() {
            test = row[0].clone();
        }
        for &(p, o) in &layout.columns {
            // PSPP shows the pooled standard error in the Games-Howell rows; the manual's
            // test uses the separate variances, as its interval in both programs does
            if test == "Games-Howell" && p == 4 {
                continue;
            }
            let (Some(cell), Some(our)) = (row.get(p), mine.get(o)) else {
                problems.push(format!("row {r}: column {p} or {o} is missing"));
                continue;
            };
            // where the manual leaves a statistic undefined (a standard deviation of one case,
            // skewness below W = 3, kurtosis below W = 4) PSPP prints NaN, infinity or a
            // standard error of 0; ours is empty
            let undefined = matches!(cell.as_str(), "NaN" | "+Infinity" | "-Infinity")
                || (ours.columns[o].starts_with("Std. Error of K")
                    || ours.columns[o].starts_with("Std. Error of S"))
                    && pspp_number(cell) == Some(Some((0.0, 10)));
            if undefined && *our == Cell::Empty {
                continue;
            }
            // PSPP's studentized range is good to about 1e-8; ours agrees with a more precise
            // reference to 1e-10 (see dist.rs), so intervals built on it get that much room
            if matches!(test.as_str(), "Tukey HSD" | "Games-Howell")
                && let (Some(Some((value, _))), Cell::Number(number)) = (pspp_number(cell), our)
                && (number - value).abs() <= 1e-8 * value.abs() + 5e-8
            {
                continue;
            }
            // a value PSPP writes with an exponent is mostly a difference of two near numbers,
            // such as an interval bound at 0, where both programs keep about 1e-14 of them
            if cell.contains('E')
                && let (Some(Some((value, _))), Cell::Number(number)) = (pspp_number(cell), our)
                && (number - value).abs() <= 1e-13
            {
                continue;
            }
            let what = format!("row {} column {p}", r + 1);
            problems.extend(check_cell(&what, cell, our).err());
        }
    }
    problems
}

/// Runs `syntax` after GET of the fixture in both programs and compares every table PSPP
/// makes with ours, in order.
fn compare_statistics(case: &str, fixture: &str, syntax: &str) {
    let Some((program, dir)) = setup(case, fixture) else {
        return;
    };
    let syntax = format!("GET FILE='{{dir}}data.sav'.\n{syntax}\n");
    let output = pspp(case, &program, &dir, &syntax);
    let theirs = pspp_tables(&output);
    let mine: Vec<(Table, Layout)> = ours(case, &dir, &syntax)
        .into_iter()
        .filter_map(|t| layout(&t.title.clone(), &t).map(|l| (t, l)))
        .collect();
    assert_eq!(
        theirs.len(),
        mine.len(),
        "{case}: PSPP made {} tables, we {}: {:?}\n{output}",
        theirs.len(),
        mine.len(),
        mine.iter().map(|(t, _)| &t.title).collect::<Vec<_>>()
    );
    let mut problems = Vec::new();
    for ((_, rows), (table, layout)) in theirs.iter().zip(&mine) {
        let found = positional(rows, table, layout);
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

/// Display formats with ten decimals, so PSPP prints the statistics of these variables
/// (MEANS uses the format of the dependent) precisely enough.
const PRECISE: &str =
    "FORMATS age monthly_spend q1_satisfaction q2_recommend q5_1 TO q5_5 (F20.10).";

fn run_statistics(prefix: &str, cases: &[(&str, &str, &str)]) {
    for (case, fixture, syntax) in cases {
        compare_statistics(
            &format!("{prefix}-{case}"),
            fixture,
            &format!("{PRECISE}\n{syntax}"),
        );
    }
}

/// The cells PSPP knows; it has no MEDIAN, NPCT or SPCT.
const MEANS_CELLS: &str = "MEAN COUNT STDDEV SEMEAN SUM MIN MAX RANGE VARIANCE KURT SEKURT SKEW SESKEW FIRST LAST HARMONIC GEOMETRIC";

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn means_tables() {
    let all = format!("MEANS age BY gender BY region /CELLS={MEANS_CELLS}.");
    let all_weighted = format!("MEANS monthly_spend BY region BY q5_1 /CELLS={MEANS_CELLS}.");
    let no_controls = "MEANS TABLES=monthly_spend /CELLS=MEAN COUNT SUM HARMONIC GEOMETRIC.";
    run_statistics(
        "means",
        &[
            ("default", SURVEY, "MEANS TABLES=age BY gender."),
            ("all-cells", SURVEY, &all),
            ("all-cells-weighted", WEIGHTED, &all_weighted),
            (
                "missing-dependent",
                WEIGHTED,
                "MEANS monthly_spend BY region.",
            ),
            (
                "include",
                SURVEY,
                "MEANS q1_satisfaction BY gender /MISSING=INCLUDE.",
            ),
            // PSPP has no MISSING=TABLE; tablewise deletion is checked in procedures.rs
            (
                "include-nested-weighted",
                WEIGHTED,
                "MEANS q2_recommend BY region BY q5_1 /MISSING=INCLUDE /CELLS=MEAN COUNT STDDEV SUM.",
            ),
            (
                "dependent",
                SURVEY,
                "MEANS age BY q1_satisfaction /MISSING=DEPENDENT.",
            ),
            ("no-controls", WEIGHTED, no_controls),
            (
                "two-lists-weighted",
                WEIGHTED,
                "MEANS age BY gender /monthly_spend BY region /CELLS=MEAN COUNT VARIANCE.",
            ),
            // PSPP's MIN and MAX still see cases weighted 0 or less, which the manual makes
            // invisible, so they are not asked for here (procedures.rs checks them)
            (
                "unusable-weights",
                WEIGHTED,
                "RECODE weight (LO THRU 0.6 = 0) (0.6 THRU 0.7 = -1).\nMEANS age BY region /CELLS=MEAN COUNT STDDEV SUM.",
            ),
            (
                "string-control",
                SURVEY,
                "MEANS age BY plan /CELLS=MEAN COUNT STDDEV FIRST LAST.",
            ),
            (
                "nested-missing-weighted",
                WEIGHTED,
                "MEANS monthly_spend BY gender BY q6_mention1 /CELLS=MEAN COUNT STDDEV SEMEAN.",
            ),
        ],
    );
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn t_test_tables() {
    run_statistics(
        "t-test",
        &[
            (
                "one-sample",
                SURVEY,
                "T-TEST /TESTVAL=50 /VARIABLES=age monthly_spend.",
            ),
            (
                "one-sample-weighted-90",
                WEIGHTED,
                "T-TEST /TESTVAL=150 /VARIABLES=monthly_spend age /CRITERIA=CI(.90).",
            ),
            (
                "groups",
                SURVEY,
                "T-TEST GROUPS=gender(1,2) /VARIABLES=age monthly_spend.",
            ),
            (
                "groups-weighted",
                WEIGHTED,
                "T-TEST GROUPS=gender(1 2) /VARIABLES=age monthly_spend q2_recommend.",
            ),
            (
                "cut-point",
                WEIGHTED,
                "T-TEST GROUPS=age(50) /VARIABLES=monthly_spend q1_satisfaction.",
            ),
            (
                "default-groups",
                SURVEY,
                "T-TEST GROUPS=gender /VARIABLES=q1_satisfaction q2_recommend.",
            ),
            (
                "groups-listwise",
                WEIGHTED,
                "T-TEST GROUPS=q5_1(0,1) /VARIABLES=monthly_spend q1_satisfaction /MISSING=LISTWISE.",
            ),
            (
                "include",
                SURVEY,
                "T-TEST /TESTVAL=3 /VARIABLES=q1_satisfaction /MISSING=INCLUDE.",
            ),
            ("pairs-all", SURVEY, "T-TEST PAIRS=q5_1 q5_2 q5_3."),
            (
                "pairs-paired-weighted",
                WEIGHTED,
                "T-TEST PAIRS=q1_satisfaction q5_1 WITH age q5_2 (PAIRED).",
            ),
            (
                "pairs-with-listwise",
                WEIGHTED,
                "T-TEST PAIRS=q1_satisfaction WITH q2_recommend monthly_spend /MISSING=LISTWISE /CRITERIA=CI(.99).",
            ),
            (
                "groups-other-values",
                WEIGHTED,
                "T-TEST GROUPS=region(1,3) /VARIABLES=age.",
            ),
        ],
    );
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn oneway_tables() {
    run_statistics(
        "oneway",
        &[
            (
                "plain",
                SURVEY,
                "ONEWAY age BY region /STATISTICS=DESCRIPTIVES HOMOGENEITY.",
            ),
            (
                "weighted",
                WEIGHTED,
                "ONEWAY age monthly_spend BY region /STATISTICS=DESCRIPTIVES HOMOGENEITY.",
            ),
            (
                "pooled-tests",
                SURVEY,
                "ONEWAY monthly_spend BY region /POSTHOC=LSD BONFERRONI SIDAK.",
            ),
            (
                "tukey-scheffe-weighted",
                WEIGHTED,
                "ONEWAY age BY region /POSTHOC=TUKEY SCHEFFE.",
            ),
            (
                "games-howell-weighted",
                WEIGHTED,
                "ONEWAY monthly_spend BY region /POSTHOC=GH.",
            ),
            (
                "contrasts",
                SURVEY,
                "ONEWAY age BY region /CONTRAST=1 -1 0 0 0 /CONTRAST=2 1 0 -1 -2.",
            ),
            (
                "contrasts-weighted-repeat",
                WEIGHTED,
                "ONEWAY age monthly_spend BY gender /CONTRAST=-1 0.5 0.5.",
            ),
            (
                "listwise",
                WEIGHTED,
                "ONEWAY age monthly_spend BY region /MISSING=LISTWISE /STATISTICS=DESCRIPTIVES.",
            ),
            (
                "include",
                SURVEY,
                "ONEWAY q1_satisfaction BY region /MISSING=INCLUDE /STATISTICS=DESCRIPTIVES.",
            ),
            (
                "missing-factor",
                SURVEY,
                "ONEWAY age BY q1_satisfaction /POSTHOC=TUKEY LSD /STATISTICS=HOMOGENEITY.",
            ),
            (
                "alpha",
                WEIGHTED,
                "ONEWAY age BY gender /POSTHOC=LSD TUKEY ALPHA(0.01).",
            ),
        ],
    );
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn correlations_tables() {
    run_statistics(
        "correlations",
        &[
            (
                "square",
                SURVEY,
                "CORRELATIONS /VARIABLES=age monthly_spend q1_satisfaction.",
            ),
            (
                "square-weighted-all",
                WEIGHTED,
                "CORRELATIONS /VARIABLES=age monthly_spend q1_satisfaction /STATISTICS=ALL.",
            ),
            (
                "with",
                SURVEY,
                "CORRELATIONS /VARIABLES=age WITH monthly_spend q2_recommend.",
            ),
            (
                "listwise",
                SURVEY,
                "CORRELATIONS /VARIABLES=age monthly_spend q2_recommend /MISSING=LISTWISE.",
            ),
            (
                "one-tailed-weighted",
                WEIGHTED,
                "CORRELATIONS /VARIABLES=q5_1 TO q5_5 /PRINT=ONETAIL.",
            ),
            (
                "include",
                SURVEY,
                "CORRELATIONS /VARIABLES=q1_satisfaction q2_recommend age /MISSING=INCLUDE.",
            ),
            (
                "two-lists",
                WEIGHTED,
                "CORRELATIONS /VARIABLES=age monthly_spend /q5_1 q5_2 WITH q1_satisfaction.",
            ),
            (
                "listwise-xprod-weighted",
                WEIGHTED,
                "CORRELATIONS /VARIABLES=age monthly_spend q1_satisfaction /MISSING=LISTWISE /STATISTICS=XPROD.",
            ),
            // PSPP adds negative weights into the sums (the manual treats them as 0) and
            // makes every coefficient NaN with a missing weight, so only zero weights here
            (
                "unusable-weights",
                WEIGHTED,
                "RECODE weight (LO THRU 0.6 = 0).\nCORRELATIONS /VARIABLES=age monthly_spend q2_recommend /STATISTICS=XPROD.",
            ),
            // with LISTWISE PSPP computes the descriptives from the complete cases, as the
            // Algorithms manual says; the Command Syntax Reference keeps them variable by
            // variable, which we follow, so the comparison uses the default PAIRWISE
            (
                "descriptives",
                SURVEY,
                "CORRELATIONS /VARIABLES=age monthly_spend q1_satisfaction q2_recommend /STATISTICS=DESCRIPTIVES.",
            ),
        ],
    );
}

/// Runs EXAMINE in both programs and pairs the n-th table of a title in PSPP's output with the
/// n-th of ours (the two order the tables differently). PSPP has no M-estimators and no
/// Kolmogorov-Smirnov test, so those are checked in examine.rs; its Tests of Normality hold
/// Shapiro-Wilk alone, compared with our last three columns. Its default /STATISTICS shows no
/// Descriptives (the manual's default does), so every case asks for the tables it compares.
fn compare_examine(case: &str, fixture: &str, syntax: &str) {
    let Some((program, dir)) = setup(case, fixture) else {
        return;
    };
    let syntax = format!("GET FILE='{{dir}}data.sav'.\n{PRECISE}\n{syntax}\n");
    let output = pspp(case, &program, &dir, &syntax);
    let theirs = pspp_tables(&output);
    let mine = ours(case, &dir, &syntax);
    let mut problems = Vec::new();
    let titles = [
        "Case Processing Summary",
        "Descriptives",
        "Percentiles",
        "Extreme Values",
        "Tests of Normality",
    ];
    for title in titles {
        let pspp_tables: Vec<_> = theirs.iter().filter(|(t, _)| t == title).collect();
        let our_tables: Vec<_> = mine.iter().filter(|t| t.title == title).collect();
        assert_eq!(
            pspp_tables.len(),
            our_tables.len(),
            "{case}: {title}: PSPP made {}, we {}\n{output}",
            pspp_tables.len(),
            our_tables.len()
        );
        for ((_, rows), table) in pspp_tables.into_iter().zip(our_tables) {
            let first = table.row_headers;
            let values = table.columns.len() - first;
            let header_rows = match title {
                "Case Processing Summary" => 3,
                "Percentiles" | "Tests of Normality" => 2,
                _ => 1,
            };
            // with weights below 1 PSPP does not follow the manual's depths of Tukey's hinges
            // (an independent computation of the manual agrees with ours), so a weighted
            // case leaves their rows out
            let hinge = "Tukey's Hinges";
            let skip =
                |row: &[Cell]| fixture == WEIGHTED && row.contains(&Cell::Text(hinge.into()));
            let rows: Vec<Vec<String>> = rows
                .iter()
                .filter(|row| fixture != WEIGHTED || !row.iter().any(|c| c == hinge))
                .cloned()
                .collect();
            let table = Table {
                title: table.title.clone(),
                row_headers: first,
                columns: table.columns.clone(),
                rows: table.rows.iter().filter(|r| !skip(r)).cloned().collect(),
                ..Table::default()
            };
            let layout = match title {
                "Tests of Normality" => Layout {
                    header_rows,
                    columns: (0..3).map(|c| (first + c, first + 3 + c)).collect(),
                },
                _ => straight(header_rows, first, values),
            };
            let found = positional(&rows, &table, &layout);
            problems.extend(found.into_iter().map(|p| format!("{title}: {p}")));
        }
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

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn examine_tables() {
    let mut cases: Vec<(String, &str, String)> = vec![
        // PSPP gives NaN bounds for any /CINTERVAL, so other levels are checked in examine.rs
        ("descriptives".into(), SURVEY, "EXAMINE VARIABLES=age /STATISTICS=DESCRIPTIVES.".into()),
        ("descriptives-weighted".into(), WEIGHTED, "EXAMINE VARIABLES=monthly_spend /STATISTICS=DESCRIPTIVES.".into()),
        ("pairwise".into(), SURVEY, "EXAMINE age monthly_spend q2_recommend /MISSING=PAIRWISE /STATISTICS=DESCRIPTIVES.".into()),
        ("by-factor".into(), SURVEY, "EXAMINE monthly_spend BY region /STATISTICS=DESCRIPTIVES /PERCENTILES.".into()),
        ("by-factor-weighted".into(), WEIGHTED, "EXAMINE age monthly_spend BY region /MISSING=PAIRWISE /STATISTICS=DESCRIPTIVES /PERCENTILES.".into()),
        ("two-factors".into(), SURVEY, "EXAMINE age BY region BY q5_1 /STATISTICS=DESCRIPTIVES /PERCENTILES(25,50,75) /NOTOTAL.".into()),
        ("two-factors-weighted".into(), WEIGHTED, "EXAMINE monthly_spend BY q5_2 BY q5_1 /STATISTICS=DESCRIPTIVES /PERCENTILES(10,50,90).".into()),
        ("extremes-id".into(), SURVEY, "EXAMINE age monthly_spend BY gender /MISSING=PAIRWISE /STATISTICS=EXTREME(3) /ID=resp_id.".into()),
        ("extremes-ties".into(), WEIGHTED, "EXAMINE q2_recommend /STATISTICS=EXTREME.".into()),
        ("include".into(), SURVEY, "EXAMINE q1_satisfaction q2_recommend /MISSING=PAIRWISE INCLUDE /STATISTICS=DESCRIPTIVES /PERCENTILES.".into()),
        // listwise, the manual leaves a case missing on a factor out of the total as well,
        // PSPP keeps it there; NOTOTAL compares the cells, where both leave it out
        ("listwise-factor-missing".into(), SURVEY, "EXAMINE age BY q6_mention1 /STATISTICS=DESCRIPTIVES /PERCENTILES(25,50,75) /NOTOTAL.".into()),
        // PSPP still sees cases weighted 0 in the minimum and maximum, which the manual leaves
        // out, so this case compares the percentiles only
        // Shapiro-Wilk: one dependent per command, as PSPP shows only the first of several in
        // the total; groups of 3 to 11 cases use Royston's small-sample approximation
        ("normality".into(), SURVEY, "EXAMINE age /STATISTICS=NONE /PLOT=NPPLOT.\nEXAMINE monthly_spend BY gender /STATISTICS=NONE /PLOT=NPPLOT /NOTOTAL.".into()),
        ("normality-small".into(), SURVEY, "SELECT IF resp_id <= 1030.\nEXAMINE age /STATISTICS=NONE /PLOT=NPPLOT.\nEXAMINE monthly_spend /STATISTICS=NONE /PLOT=NPPLOT.".into()),
        // PSPP's Shapiro-Wilk is wrong for 3 to 5 cases (W far above 1 for 4, NaN for 5, 0.925
        // for 3 where the exact W is 0.983), so those are checked in examine.rs against SciPy
        ("normality-integer-weights".into(), WEIGHTED, "COMPUTE w2 = RND(weight * 3).\nWEIGHT BY w2.\nEXAMINE age BY region /STATISTICS=NONE /PLOT=NPPLOT /NOTOTAL.".into()),
        // with non-integer weights PSPP does not round them and expand the series as the manual
        // says (it prints W above 1), so only whole weights are compared here
        ("report".into(), SURVEY, "EXAMINE age BY q1_satisfaction /MISSING=REPORT /STATISTICS=DESCRIPTIVES /NOTOTAL.".into()),
        ("unusable-weights".into(), WEIGHTED, "RECODE weight (LO THRU 0.6 = 0).\nEXAMINE age BY region /STATISTICS=NONE /PERCENTILES.".into()),
    ];
    let percents = "(1,5,10,25,33.3,50,66.7,75,90,95,99)";
    for method in ["HAVERAGE", "WAVERAGE", "ROUND", "EMPIRICAL", "AEMPIRICAL"] {
        for (fixture, suffix) in [(SURVEY, ""), (WEIGHTED, "-weighted")] {
            cases.push((
                format!("percentiles-{}{suffix}", method.to_lowercase()),
                fixture,
                format!("EXAMINE age monthly_spend /MISSING=PAIRWISE /PERCENTILES{percents}={method} /STATISTICS=NONE."),
            ));
        }
    }
    for (case, fixture, syntax) in &cases {
        compare_examine(&format!("examine-{case}"), fixture, syntax);
    }
}

/// SPLIT FILE, FILTER, TEMPORARY and N OF CASES. PSPP shows split groups as separate tables
/// whatever the keyword, so ours use SEPARATE: one set of tables per group, in group order.
#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn split_and_filter_tables() {
    let cases = [
        (
            "split-sorted",
            "SORT CASES BY gender.\nSPLIT FILE SEPARATE BY gender.\nDESCRIPTIVES age monthly_spend.",
        ),
        // groups are runs of adjacent cases, so an unsorted file gives many small groups
        (
            "split-adjacent-runs",
            "SELECT IF resp_id <= 1040.\nSPLIT FILE SEPARATE BY region.\nDESCRIPTIVES age monthly_spend.",
        ),
        (
            "filter",
            "COMPUTE adult = age >= 40.\nFILTER BY adult.\nDESCRIPTIVES age monthly_spend /STATISTICS=ALL.",
        ),
        (
            "split-filter-crosstabs",
            "SORT CASES BY region.\nSPLIT FILE SEPARATE BY region.\nFILTER BY q5_1.\nCROSSTABS /TABLES=q1_satisfaction BY gender /CELLS=COUNT ROW /STATISTICS=CHISQ.",
        ),
        (
            "split-two-variables",
            "SORT CASES BY region gender.\nSPLIT FILE SEPARATE BY region gender.\nDESCRIPTIVES age.",
        ),
        (
            "temporary",
            "TEMPORARY.\nSELECT IF gender = 1.\nCOMPUTE age = age * 2.\nDESCRIPTIVES age.\nDESCRIPTIVES age.",
        ),
        // N OF CASES is not compared: PSPP ignores it after GET (it limits only cases that
        // DATA LIST reads), where the manual limits the active dataset; split_filter.rs checks it
    ];
    for (case, syntax) in cases {
        compare_tables(&format!("split-{case}"), SURVEY, syntax);
    }
    compare_data(
        "split-z-scores",
        SURVEY,
        "",
        "SORT CASES BY gender.\nSPLIT FILE BY gender.\nFILTER BY q5_1.\nDESCRIPTIVES age monthly_spend /SAVE.\nFILTER OFF.\nSPLIT FILE OFF.\n* the z-score labels differ as in descriptives_data.\nVARIABLE LABELS Zage '' Zmonthly_spend ''.",
    );
}

/// The cells of a Statistics table as (variable, statistic) to cell text, with the percentile
/// rows numbered in order. PSPP heads a column with the variable's label.
fn pspp_statistics(
    rows: &[Vec<String>],
    names: &HashMap<String, String>,
    out: &mut HashMap<(String, String), String>,
    percentiles: &mut Vec<String>,
) {
    let (mut statistic, mut n) = (String::new(), 0);
    for row in &rows[1..] {
        if !row[0].is_empty() {
            statistic = match row[0].as_str() {
                "S.E. Mean" => "Std. Error of Mean",
                "Std Dev" => "Std. Deviation",
                "S.E. Skew" => "Std. Error of Skewness",
                "S.E. Kurt" => "Std. Error of Kurtosis",
                other => other,
            }
            .to_string();
        }
        let key = match statistic.as_str() {
            "N" => format!("N {}", row[1]),
            "Percentiles" => {
                if percentiles.len() <= n {
                    percentiles.push(row[1].clone());
                }
                n += 1;
                format!("Percentiles #{}", n - 1)
            }
            _ => statistic.clone(),
        };
        for (column, cell) in row[2..].iter().enumerate() {
            let header = &rows[0][column + 2];
            let name = names.get(header).unwrap_or(header).clone();
            out.insert((name, key.clone()), cell.clone());
        }
    }
}

/// Runs FREQUENCIES in both programs and compares the Statistics cells PSPP shows with ours,
/// by variable and statistic, and the frequency tables in order. `pspp_syntax` replaces the
/// syntax for PSPP where it needs another spelling to mean what ours does; `{alg}` asks PSPP
/// for the manual's percentiles at (W + 1) p, which it computes only with
/// /ALGORITHM=COMPATIBLE (its default is another definition).
fn compare_frequencies(case: &str, fixture: &str, syntax: &str, pspp_syntax: Option<&str>) {
    let Some((program, dir)) = setup(case, fixture) else {
        return;
    };
    let job = |syntax: &str, alg: &str| {
        let syntax = syntax.replace("{alg}", alg);
        format!("GET FILE='{{dir}}data.sav'.\n{PRECISE}\n{syntax}\n")
    };
    let output = pspp(
        case,
        &program,
        &dir,
        &job(pspp_syntax.unwrap_or(syntax), " /ALGORITHM=COMPATIBLE"),
    );
    let theirs = pspp_tables(&output);
    let mine = ours(case, &dir, &job(syntax, ""));
    let data = Dataset::read_sav(dir.join("data.sav")).unwrap();
    let mut names: HashMap<String, String> = HashMap::new();
    for name in data.variable_names() {
        let label = data.variable(name).unwrap().label().map(str::to_string);
        names.insert(label.unwrap_or_else(|| name.to_string()), name.to_string());
    }

    let mut problems = Vec::new();
    let (mut pspp_cells, mut pspp_percentiles) = (HashMap::new(), Vec::new());
    for (_, rows) in theirs.iter().filter(|(t, _)| t == "Statistics") {
        pspp_statistics(rows, &names, &mut pspp_cells, &mut pspp_percentiles);
    }
    let mut our_cells: HashMap<(String, String), &Cell> = HashMap::new();
    let mut our_percentiles = Vec::new();
    for table in mine.iter().filter(|t| t.title == "Statistics") {
        let mut n = 0;
        for row in &table.rows {
            let statistic = text(&row[0]);
            let key = match statistic.as_str() {
                "N" => format!("N {}", text(&row[1])),
                "Percentiles" => {
                    if our_percentiles.len() <= n {
                        our_percentiles.push(text(&row[1]));
                    }
                    n += 1;
                    format!("Percentiles #{}", n - 1)
                }
                _ => statistic,
            };
            for (column, cell) in row[2..].iter().enumerate() {
                our_cells.insert((table.columns[column + 2].clone(), key.clone()), cell);
            }
        }
    }
    // PSPP adds the 0th and 100th percentiles to NTILES, which the manual does not
    let extra: Vec<usize> = pspp_percentiles
        .iter()
        .enumerate()
        .filter(|(_, p)| (*p == "0" || *p == "100") && !our_percentiles.contains(p))
        .map(|(i, _)| i)
        .collect();
    let renumber = |key: &str| -> Option<String> {
        let Some(i) = key.strip_prefix("Percentiles #") else {
            return Some(key.to_string());
        };
        let i: usize = i.parse().unwrap();
        let before = extra.iter().filter(|&&e| e < i).count();
        (!extra.contains(&i)).then(|| format!("Percentiles #{}", i - before))
    };
    // with weights that are not whole numbers PSPP's percentiles leave the manual's formula
    // (an independent computation of it agrees with ours, see frequencies.rs) and its mode is
    // not the value of the largest weight, so those rows are compared with whole weights only
    let fractional = fixture == WEIGHTED && !syntax.contains("WEIGHT BY");
    for ((variable, key), cell) in &pspp_cells {
        let Some(key) = renumber(key) else {
            continue;
        };
        if fractional && (key == "Median" || key == "Mode" || key.starts_with("Percentiles")) {
            continue;
        }
        let what = format!("{variable} {key}");
        match our_cells.get(&(variable.clone(), key)) {
            None => problems.push(format!("{what}: PSPP {cell}, missing in ours")),
            // where the manual leaves a statistic undefined PSPP may print infinity or NaN
            Some(Cell::Empty) if matches!(cell.as_str(), "NaN" | "+Infinity" | "-Infinity") => {}
            Some(our) => problems.extend(check_cell(&what, cell, our).err()),
        }
    }

    let pspp_frequencies: Vec<_> = theirs.iter().filter(|(t, _)| t != "Statistics").collect();
    let our_frequencies: Vec<&Table> = mine.iter().filter(|t| t.title != "Statistics").collect();
    assert_eq!(
        pspp_frequencies.len(),
        our_frequencies.len(),
        "{case}: PSPP made {} frequency tables, we {}\n{output}",
        pspp_frequencies.len(),
        our_frequencies.len()
    );
    for ((title, rows), table) in pspp_frequencies.iter().zip(&our_frequencies) {
        let data_rows = &rows[1..];
        if data_rows.len() != table.rows.len() {
            problems.push(format!(
                "{title}: rows: PSPP {}, ours {}",
                data_rows.len(),
                table.rows.len()
            ));
            continue;
        }
        for (r, (row, mine)) in data_rows.iter().zip(&table.rows).enumerate() {
            // the value or its label, as PSPP shows it, puts the rows in the same order
            let shown = row[1].trim();
            let same = match (pspp_number(shown), &mine[1]) {
                (Some(Some((value, _))), Cell::Number(n)) => (value - n).abs() < 1e-9,
                _ => shown.is_empty() || text(&mine[2]) == shown || text(&mine[1]) == shown,
            };
            if !same {
                problems.push(format!("{title} row {r}: PSPP {shown}, ours {:?}", mine[1]));
            }
            for column in 2..6 {
                let what = format!("{title} row {r} column {column}");
                problems.extend(check_cell(&what, &row[column], &mine[column + 1]).err());
            }
        }
    }
    assert!(
        problems.is_empty(),
        "{case}: {} differences (files kept in {})\n{}\n{output}",
        problems.len(),
        dir.display(),
        problems[..problems.len().min(25)].join("\n")
    );
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn frequencies_tables() {
    const WHOLE: &str = "COMPUTE w3 = RND(weight * 3).\nWEIGHT BY w3.";
    let percents = "/PERCENTILES=1 5 10 25 33.3 50 66.7 75 90 95 99";
    let counted = "gender region q1_satisfaction q2_recommend plan";
    let four = "age monthly_spend q1_satisfaction q2_recommend";
    let cases: Vec<(&str, &str, String, Option<String>)> = vec![
        ("counts", SURVEY, format!("FREQUENCIES {counted} /STATISTICS=MEAN."), None),
        ("counts-weighted", WEIGHTED, format!("FREQUENCIES {counted} /STATISTICS=MEAN."), None),
        ("all-statistics", SURVEY, format!("FREQUENCIES {four} /STATISTICS=ALL /FORMAT=NOTABLE{{alg}}."), None),
        ("all-statistics-weighted", WEIGHTED, format!("FREQUENCIES {four} /STATISTICS=ALL /FORMAT=NOTABLE{{alg}}."), None),
        ("percentiles", SURVEY, format!("FREQUENCIES age monthly_spend q2_recommend {percents} /STATISTICS=MEDIAN /FORMAT=NOTABLE{{alg}}."), None),
        ("percentiles-weighted", WEIGHTED, format!("{WHOLE}\nFREQUENCIES age monthly_spend q2_recommend {percents} /STATISTICS=MEDIAN MODE /FORMAT=NOTABLE{{alg}}."), None),
        ("ntiles", SURVEY, "FREQUENCIES age q2_recommend /NTILES=4 /NTILES=10 /STATISTICS=MEAN /FORMAT=NOTABLE{alg}.".into(), None),
        ("ntiles-weighted", WEIGHTED, format!("{WHOLE}\nFREQUENCIES age monthly_spend /NTILES=5 /PERCENTILES=50 99 /STATISTICS=MEAN /FORMAT=NOTABLE{{alg}}."), None),
        ("dvalue", SURVEY, "FREQUENCIES region q1_satisfaction plan /FORMAT=DVALUE /STATISTICS=MODE.".into(), None),
        ("afreq", SURVEY, "FREQUENCIES region q2_recommend q5_1 /FORMAT=AFREQ /STATISTICS=MEAN SUM.".into(), None),
        // PSPP takes the minimum, maximum and range from the first and last rows of the table,
        // wrong when they are not in ascending order of value, so those are left out here
        ("dfreq-weighted", WEIGHTED, "FREQUENCIES region q1_satisfaction /FORMAT=DFREQ /STATISTICS=SUM MEAN.".into(), None),
        ("limit", SURVEY, "FREQUENCIES age gender q1_satisfaction q5_1 /FORMAT=LIMIT(5) /STATISTICS=MEAN.".into(), None),
        // PSPP leaves user-missing values out of the statistics even with INCLUDE, where the
        // manual counts them; without missing values PSPP computes what INCLUDE means
        ("include", SURVEY, "FREQUENCIES q1_satisfaction q2_recommend /MISSING=INCLUDE /STATISTICS=ALL{alg}.".into(),
            Some("MISSING VALUES q1_satisfaction q2_recommend ().\nFREQUENCIES q1_satisfaction q2_recommend /STATISTICS=ALL{alg}.".into())),
        ("include-weighted", WEIGHTED, "FREQUENCIES q2_recommend /MISSING=INCLUDE /STATISTICS=MEAN MEDIAN STDDEV{alg}.".into(),
            Some("MISSING VALUES q2_recommend ().\nFREQUENCIES q2_recommend /STATISTICS=MEAN MEDIAN STDDEV{alg}.".into())),
        // PSPP ignores ORDER=VARIABLE and makes one Statistics table; the cells are compared
        // by variable, so both layouts line up
        ("order-variable", SURVEY, "FREQUENCIES gender region /ORDER=VARIABLE /STATISTICS=MEAN MODE.".into(), None),
        ("three-cases", SURVEY, "SELECT IF resp_id <= 1003.\nFREQUENCIES gender age region /STATISTICS=ALL{alg}.".into(), None),
        ("four-cases", SURVEY, "SELECT IF resp_id <= 1004.\nFREQUENCIES age monthly_spend /STATISTICS=ALL /PERCENTILES=10 50 90 /FORMAT=NOTABLE{alg}.".into(), None),
        ("unusable-weights", WEIGHTED, "RECODE weight (LO THRU 0.6 = 0) (0.6 THRU 0.7 = -1).\nFREQUENCIES age region /STATISTICS=ALL /FORMAT=LIMIT(6){alg}.".into(), None),
        ("abbreviations", SURVEY, "FRE VAR=region q5_2 /FOR=DFREQ /STA=SEM VAR KURT SEKURT SKEW SESKEW.".into(), None),
        ("strings-weighted", WEIGHTED, "FREQUENCIES plan gender /FORMAT=DFREQ /STATISTICS=NONE.".into(), None),
    ];
    for (case, fixture, syntax, pspp) in &cases {
        compare_frequencies(
            &format!("frequencies-{case}"),
            fixture,
            syntax,
            pspp.as_deref(),
        );
    }
}
