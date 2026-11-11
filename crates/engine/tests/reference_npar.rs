//! NPAR TESTS compared with GNU PSPP, run as a black box on the same file and the same syntax.
//! Ignored by default because they need PSPP:
//!
//! ```text
//! STATSCRAFT_PSPP=/path/to/pspp cargo test -p statscraft-engine --test reference_npar -- --ignored
//! ```
//!
//! PSPP starts with `SET FORMAT=F20.10`, so most statistics come with ten decimals. Tables are
//! paired in order (PSPP writes the Descriptive Statistics table last, we first, so that one is
//! paired by title). Every number PSPP prints must be found among the numbers of our paired
//! table, within half a unit of the last decimal PSPP shows. The layouts differ (several of
//! ours follow the variables in columns where PSPP has them in rows), and the unit tests in
//! npar.rs check where each number stands. Labels, "." and NaN cells are skipped.
//!
//! Only what both programs do by the manual is compared. PSPP differs where Statscraft follows
//! the manual: the exact sign and McNemar tests above 25 changes (the manual uses the normal
//! and chi-square approximations), the continuity correction of the two-sample median test, the
//! median itself (the middle case or the mean of the two middle cases), the Lilliefors
//! significance of K-S with estimated normal parameters, the Poisson and exponential K-S, the
//! Jonckheere-Terpstra statistic (checked against a direct count in npar.rs), the exact Wilcoxon
//! test with ties, the K-S significance for Z of 1 and more, weights that are not whole numbers
//! (rounded, as documented), and the ranks of the Mann-Whitney and Wilcoxon tests with weights
//! and of Cochran's Q with weights (npar.rs checks those against the cases written out as often
//! as their weight). PSPP has no
//! two-sample K-S, W-W, MOSES or MH test and no PARTIAL CORR; those are checked against
//! independent computations in npar.rs.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs};

static RUNS: AtomicUsize = AtomicUsize::new(0);

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

/// PSPP's tables: title and rows.
fn pspp_tables(output: &str) -> Vec<(String, Vec<Vec<String>>)> {
    let mut tables = Vec::new();
    let mut current: Option<(String, Vec<Vec<String>>)> = None;
    for record in csv_records(output) {
        match (record[0].strip_prefix("Table: "), &mut current) {
            (Some(title), _) => {
                tables.extend(current.take());
                current = Some((title.to_string(), Vec::new()));
            }
            (None, Some(_)) if record.len() == 1 && record[0].is_empty() => {
                tables.extend(current.take())
            }
            (None, Some(_)) if record[0].starts_with("Footnote:") => {}
            (None, Some((_, rows))) => rows.push(record),
            (None, None) => {}
        }
    }
    tables.extend(current);
    tables
}

/// The value of a numeric cell and its number of decimals.
fn pspp_number(text: &str) -> Option<(f64, i32)> {
    let text = text.trim();
    let value: f64 = text.parse().ok()?;
    if !value.is_finite() {
        return None;
    }
    let (mantissa, exponent) = match text.split_once(['E', 'e']) {
        Some((m, e)) => (m, e.parse::<i32>().ok()?),
        None => (text, 0),
    };
    let decimals = mantissa.split_once('.').map_or(0, |(_, d)| d.len() as i32) - exponent;
    Some((value, decimals))
}

fn ours(dir: &Path, syntax: &str) -> Vec<Table> {
    let syntax = syntax.replace("{dir}", &format!("{}/", dir.display()));
    let (parsed, errors) = parse(&syntax);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &parsed {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

/// Runs `syntax` after GET of the survey in both programs and checks every number of PSPP's
/// tables against ours, leaving out PSPP rows and columns whose label is in `skip`. Returns
/// the number of numbers compared.
fn compare(case: &str, syntax: &str, skip: &[&str]) -> usize {
    let Some(program) = env::var_os("STATSCRAFT_PSPP") else {
        eprintln!("{case}: skipped, STATSCRAFT_PSPP is not set");
        return 0;
    };
    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    let dir = env::temp_dir().join(format!("statscraft-npar-{}-{run}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    fs::copy(source.join("survey_basic.sav"), dir.join("data.sav")).unwrap();
    let syntax = format!("GET FILE='{{dir}}data.sav'.\n{syntax}\n");
    let job = format!("SET FORMAT=F20.10.\n{}", syntax.replace("{dir}", ""));
    fs::write(dir.join("job.sps"), job).unwrap();
    let status = Command::new(PathBuf::from(program))
        .args(["job.sps", "-o", "pspp.csv", "-O", "format=csv"])
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("PSPP runs");
    let output = fs::read_to_string(dir.join("pspp.csv")).unwrap_or_default();
    assert!(
        status.success() && !output.contains("error:"),
        "{case}: PSPP failed\n{output}"
    );
    let mut theirs = pspp_tables(&output);
    let mut mine = ours(&dir, &syntax);
    let mut pairs = Vec::new();
    if let Some(i) = theirs.iter().position(|t| t.0 == "Descriptive Statistics") {
        let j = mine
            .iter()
            .position(|t| t.title == "Descriptive Statistics")
            .expect("our descriptives");
        pairs.push((theirs.remove(i), mine.remove(j)));
    }
    assert_eq!(
        theirs.len(),
        mine.len(),
        "{case}: PSPP made {} tables, we {}: {:?}\n{output}",
        theirs.len(),
        mine.len(),
        mine.iter().map(|t| &t.title).collect::<Vec<_>>()
    );
    pairs.extend(theirs.into_iter().zip(mine));
    let mut problems = Vec::new();
    let mut compared = 0;
    for ((title, rows), table) in &pairs {
        let numbers: Vec<f64> = table
            .rows
            .iter()
            .flatten()
            .filter_map(|c| match c {
                Cell::Number(n) => Some(*n),
                _ => None,
            })
            .collect();
        let header = rows.first().cloned().unwrap_or_default();
        for (r, row) in rows.iter().enumerate() {
            if row
                .first()
                .is_some_and(|label| skip.contains(&label.as_str()))
            {
                continue;
            }
            // the first column holds labels, also when a category shows as its value
            for (c, cell) in row.iter().enumerate().skip(1) {
                if header.get(c).is_some_and(|h| skip.contains(&h.as_str())) {
                    continue;
                }
                let Some((value, decimals)) = pspp_number(cell) else {
                    continue;
                };
                compared += 1;
                let allowed = 0.5 * 10f64.powi(-decimals) * (1.0 + 1e-9) + 1e-12 * value.abs();
                if !numbers.iter().any(|n| (n - value).abs() <= allowed) {
                    problems.push(format!(
                        "{title} / {}: PSPP {cell} (row {r}, column {c}) not in our {}",
                        table.title, table.title
                    ));
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

/// Integer weights, so rounding them changes nothing and both programs weight alike.
const WHOLE_WEIGHTS: &str = "COMPUTE w2 = RND(weight * 2).\nWEIGHT BY w2.";

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn npar_tests_against_pspp() {
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "chi-square",
            "NPAR TESTS /CHISQUARE=region q1_satisfaction q5_1.",
            &[],
        ),
        (
            "chi-square-range-expected",
            "NPAR TESTS /CHISQUARE=q1_satisfaction q2_recommend (1,5) /EXPECTED=1 2 3 2 1.",
            &[],
        ),
        (
            "chi-square-weighted",
            &format!(
                "{WHOLE_WEIGHTS}\nNPAR TESTS /CHISQUARE=region (2,4) /CHISQUARE=q2_recommend."
            ),
            &[],
        ),
        (
            "binomial",
            "NPAR TESTS /BINOMIAL=q5_1 /BINOMIAL(0.3)=q5_2 /BINOMIAL(.6)=q5_3 \
             /BINOMIAL=q1_satisfaction(3) /BINOMIAL(.25)=q1_satisfaction(2,4) /BINOMIAL=q5_4 q5_5.",
            &[],
        ),
        (
            "binomial-weighted",
            &format!("{WHOLE_WEIGHTS}\nNPAR TESTS /BINOMIAL(0.45)=q5_1 q5_3 /BINOMIAL=age(40)."),
            &[],
        ),
        (
            "runs",
            "NPAR TESTS /RUNS(MEAN)=age monthly_spend /RUNS(MEDIAN)=age q2_recommend /RUNS(MODE)=q1_satisfaction /RUNS(30)=age.",
            &[],
        ),
        (
            "runs-small",
            "SELECT IF resp_id < 1040.\nNPAR TESTS /RUNS(MEAN)=age /RUNS(4)=q1_satisfaction.",
            &[],
        ),
        (
            "ks-given-parameters",
            "NPAR TESTS /K-S(NORMAL 40 10)=age /K-S(NORMAL 180 100)=monthly_spend /K-S(UNIFORM)=age \
             /K-S(UNIFORM 0 400)=monthly_spend.",
            // for Z of 1 and more PSPP's significance is not the manual's Smirnov series (which
            // agrees with the limiting Kolmogorov distribution); below 1 the two agree
            &["Asymp. Sig. (2-tailed)"],
        ),
        (
            "mann-whitney",
            "NPAR TESTS /M-W=age monthly_spend q1_satisfaction BY gender(1,2) /M-W=q2_recommend BY region(3,5).",
            &[],
        ),
        (
            "kruskal-wallis",
            "NPAR TESTS /K-W=age monthly_spend q2_recommend BY region(1,5) /K-W=age BY region(2,4).",
            &[],
        ),
        (
            "median-k-sample",
            "NPAR TESTS /MEDIAN=age monthly_spend BY region(1,5) /MEDIAN(45)=age BY region(2,4).",
            &["Median"],
        ),
        (
            "wilcoxon",
            "NPAR TESTS /WILCOXON=q5_1 q5_2 WITH q5_3 q5_4 (PAIRED) /WILCOXON=q1_satisfaction q2_recommend age.",
            &[],
        ),
        (
            "sign-mcnemar-small",
            "SELECT IF resp_id < 1030.\nNPAR TESTS /SIGN=q5_1 q5_2 WITH q5_3 q5_4 /MCNEMAR=q5_1 WITH q5_3 q5_4 /METHOD=EXACT.",
            &[],
        ),
        (
            "related",
            "NPAR TESTS /FRIEDMAN=q5_1 TO q5_5 /KENDALL=q1_satisfaction q2_recommend /COCHRAN=q5_1 TO q5_4 \
             /FRIEDMAN=age q2_recommend monthly_spend.",
            &[],
        ),
        (
            "related-weighted-listwise",
            &format!(
                "{WHOLE_WEIGHTS}\nNPAR TESTS /FRIEDMAN=q1_satisfaction q2_recommend /KENDALL=q5_1 q5_2 q5_3 \
                 /MISSING=LISTWISE."
            ),
            &[],
        ),
        (
            "descriptives-include",
            "NPAR TESTS /M-W=q1_satisfaction BY gender(1,2) /STATISTICS=DESCRIPTIVES /MISSING=INCLUDE.",
            &[],
        ),
    ];
    let mut total = 0;
    for (case, syntax, skip) in cases {
        total += compare(case, syntax, skip);
    }
    eprintln!("compared {total} numbers in {} cases", cases.len());
}
