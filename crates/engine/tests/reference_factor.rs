//! FACTOR and QUICK CLUSTER compared with GNU PSPP, run as a black box on the same synthetic
//! data and the same syntax. Ignored by default because they need PSPP:
//!
//! ```text
//! STATSCRAFT_PSPP=/path/to/pspp cargo test -p statscraft-engine --test reference_factor -- --ignored
//! ```
//!
//! PSPP's FACTOR knows PC and PAF, VARIMAX, EQUAMAX, QUARTIMAX and PROMAX (its default power is
//! 5, the manual's 4, so the cases give it). Its standard deviations and covariances divide by
//! N where the manual's statistics divide by N - 1, so those are left out. PSPP does not reflect
//! unrotated or oblique factors to a positive sum, so each column is compared after matching its
//! sign, and its Rotation Sums of Squared Loadings for PROMAX are not filled in. Its QUICK
//! CLUSTER numbers the clusters in another order, so clusters are matched by their final
//! centers. PSPP prints with `SET FORMAT=F20.10` and each of our numbers must round to what it
//! printed. With three factors each program stops the rotation by its own rule, so that case
//! iterates to a tight criterion in both; PSPP does not order rotated factors by their sums of
//! squares, so the three-factor case is one where the orders agree.

use statscraft_data::{Dataset, Values};
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs};

/// The data of factor.rs: two factors in six items, two missing values, whole weights.
const DATA: &str = "DATA LIST FREE /id g w x1 x2 x3 x4 x5 x6.
BEGIN DATA.
1 1 2 5 5 6 2 2 3
2 1 1 5 4 5 4 4 3
3 1 3 1 2 1 5 4 4
4 1 2 5 5 5 4 3 .
5 1 1 4 4 4 5 5 6
6 1 2 6 4 6 5 5 3
7 1 1 5 5 4 5 4 6
8 1 1 4 5 5 6 6 6
9 1 1 4 3 5 6 6 7
10 1 2 4 4 3 3 3 4
11 1 3 5 5 5 3 2 3
12 1 3 6 7 5 6 6 7
13 1 3 6 6 5 7 6 6
14 1 3 3 2 4 3 4 2
15 1 3 5 3 4 5 4 4
16 1 2 3 1 3 3 2 4
17 1 1 3 3 3 5 5 5
18 1 2 . 4 2 7 6 6
19 1 1 3 5 4 4 4 5
20 1 1 5 5 4 2 5 2
21 2 2 5 4 4 6 5 4
22 2 1 6 6 7 5 6 5
23 2 2 5 6 6 4 6 4
24 2 2 3 4 4 1 3 4
25 2 1 5 7 4 7 6 7
26 2 3 2 2 1 3 2 4
27 2 2 4 4 5 1 4 2
28 2 1 3 5 5 4 4 4
29 2 3 4 5 4 2 4 4
30 2 2 3 4 4 4 5 5
31 2 2 3 3 3 4 5 4
32 2 2 4 3 3 2 4 2
33 2 1 6 6 5 2 3 4
34 2 1 4 6 5 5 4 5
35 2 2 2 2 2 3 2 3
36 2 2 5 5 5 5 3 2
37 2 1 5 4 5 4 3 3
38 2 1 4 5 4 4 3 4
39 2 3 3 4 3 5 4 5
40 2 2 5 4 4 5 5 4
END DATA.
";

const ANY: usize = usize::MAX;

static RUNS: AtomicUsize = AtomicUsize::new(0);

fn setup(case: &str) -> Option<(PathBuf, PathBuf)> {
    let Some(pspp) = env::var_os("STATSCRAFT_PSPP") else {
        eprintln!("{case}: skipped, STATSCRAFT_PSPP is not set");
        return None;
    };
    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    let dir = env::temp_dir().join(format!("statscraft-factor-{}-{run}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
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

fn pspp_tables(output: &str) -> Vec<(String, Vec<Vec<String>>)> {
    let mut tables = Vec::new();
    let mut current: Option<(String, Vec<Vec<String>>)> = None;
    for record in csv_records(output) {
        if let Some(title) = record[0].strip_prefix("Table: ") {
            tables.extend(current.take());
            current = Some((title.to_string(), Vec::new()));
        } else if record.len() == 1 && record[0].is_empty() {
            tables.extend(current.take());
        } else if let Some((_, rows)) = current.as_mut()
            && !record[0].starts_with("Caption:")
        {
            rows.push(record);
        }
    }
    tables.extend(current);
    tables
}

/// The value and the decimals PSPP printed; None for a blank cell.
fn pspp_number(text: &str) -> Option<(f64, i32)> {
    let text = text.trim();
    let text = text.strip_suffix('%').unwrap_or(text);
    let value: f64 = text.parse().ok()?;
    let decimals = text.split_once('.').map_or(0, |(_, d)| d.len() as i32);
    Some((value, decimals))
}

/// How a PSPP table lines up with ours: our title, PSPP's header rows and header columns, and
/// whether columns are compared after matching their signs.
fn layout(title: &str, word: &str) -> Option<(String, usize, usize, bool)> {
    Some(match title {
        "Descriptive Statistics" => (title.into(), 1, 1, false),
        "KMO and Bartlett's Test" => (title.into(), 0, 2, false),
        "Correlation Matrix" => (title.into(), 1, 2, false),
        "Anti-Image Matrices" => ("Anti-image Matrices".into(), 1, 2, false),
        "Communalities" => (title.into(), 1, 1, false),
        "Total Variance Explained" => (title.into(), 2, 1, false),
        "Component Matrix" | "Factor Matrix" | "Pattern Matrix" | "Structure Matrix" => {
            (title.into(), 2, 1, true)
        }
        "Rotated Component Matrix" | "Rotated Factor Matrix" => (title.into(), 2, 1, true),
        "Factor Correlation Matrix" => (format!("{word} Correlation Matrix"), 1, 1, true),
        _ => return None,
    })
}

/// A cell left out: table, row (`ANY` for every row) and column.
type Skip = (&'static str, usize, usize);

/// `skip` holds the cells left out.
fn compare_factor(case: &str, syntax: &str, skip: &[Skip]) {
    let Some((program, dir)) = setup(case) else {
        return;
    };
    let syntax = format!("{DATA}{syntax}\n");
    let output = pspp(case, &program, &dir, &syntax);
    let mine = ours(case, &dir, &syntax);
    let word = if mine.iter().any(|t| t.title == "Component Matrix") {
        "Component"
    } else {
        "Factor"
    };
    let mut problems = Vec::new();
    let mut compared = 0;
    for (title, rows) in pspp_tables(&output) {
        let Some((our_title, header_rows, header_columns, signed)) = layout(&title, word) else {
            continue;
        };
        let table = mine
            .iter()
            .find(|t| t.title == our_title)
            .unwrap_or_else(|| panic!("{case}: we have no {our_title}"));
        let data = &rows[header_rows..];
        assert_eq!(data.len(), table.rows.len(), "{case}: {title}: rows");
        let width = data[0].len() - header_columns;
        let value = |r: usize, c: usize| -> Option<f64> {
            match &table.rows[r][table.row_headers + c] {
                Cell::Number(n) => Some(*n),
                _ => None,
            }
        };
        // the sign of each column: the one that makes the largest PSPP value agree
        let signs: Vec<f64> = (0..width)
            .map(|c| {
                if !signed {
                    return 1.0;
                }
                let (mut best, mut sign) = (0.0, 1.0);
                for (r, row) in data.iter().enumerate() {
                    if let (Some((p, _)), Some(o)) =
                        (pspp_number(&row[header_columns + c]), value(r, c))
                        && p.abs() > best
                    {
                        best = p.abs();
                        sign = if p * o < 0.0 { -1.0 } else { 1.0 };
                    }
                }
                sign
            })
            .collect();
        for (r, row) in data.iter().enumerate() {
            for c in 0..width {
                if skip.contains(&(title.as_str(), ANY, c))
                    || skip.contains(&(title.as_str(), r, c))
                {
                    continue;
                }
                let Some((expected, decimals)) = pspp_number(&row[header_columns + c]) else {
                    continue;
                };
                let what = format!("{title}: row {} column {}", r + 1, c + 1);
                // the correlations of oblique factors: PSPP leaves the signs as they come
                let expected = if title == "Factor Correlation Matrix" && r != c {
                    expected.abs()
                } else {
                    expected * signs[c]
                };
                let actual =
                    value(r, c).map(|v| match title == "Factor Correlation Matrix" && r != c {
                        true => v.abs(),
                        false => v,
                    });
                match actual {
                    Some(n) => {
                        // 1e-8: the iterative cases agree to about ten digits
                        let allowed = (0.5 * 10f64.powi(-decimals) * (1.0 + 1e-9)).max(1e-8);
                        if (n - expected).abs() > allowed {
                            problems.push(format!(
                                "{what}: PSPP {}, ours {n}",
                                row[header_columns + c]
                            ));
                        }
                        compared += 1;
                    }
                    None => problems.push(format!(
                        "{what}: PSPP {}, ours empty",
                        row[header_columns + c]
                    )),
                }
            }
        }
    }
    assert!(
        compared > 20,
        "{case}: only {compared} numbers compared\n{output}"
    );
    assert!(
        problems.is_empty(),
        "{case}: {} differences (files kept in {})\n{}",
        problems.len(),
        dir.display(),
        problems.join("\n")
    );
    eprintln!("{case}: {compared} numbers agree");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn factor_tables() {
    // the standard deviations divide by N in PSPP
    let sd = [("Descriptive Statistics", ANY, 1)];
    let cases: [(&str, &str, &[Skip]); 7] = [
        (
            "pc-varimax",
            "FACTOR VARIABLES=x1 TO x6 /PRINT=UNIVARIATE INITIAL EXTRACTION ROTATION CORRELATION SIG KMO AIC \
             /FORMAT=SORT /CRITERIA=FACTORS(2) /EXTRACTION=PC /ROTATION=VARIMAX.",
            &sd,
        ),
        (
            "pc-mineigen",
            "FACTOR VARIABLES=x1 TO x6 /CRITERIA=MINEIGEN(0.4) ITERATE(200) RCONVERGE(1E-10) /EXTRACTION=PC \
             /ROTATION=VARIMAX.",
            &[],
        ),
        (
            "paf-quartimax",
            "FACTOR VARIABLES=x1 TO x6 /CRITERIA=FACTORS(2) /EXTRACTION=PAF /ROTATION=QUARTIMAX.",
            &[],
        ),
        (
            "paf-equamax",
            "FACTOR VARIABLES=x1 TO x6 /CRITERIA=FACTORS(2) ITERATE(200) ECONVERGE(1E-10) /EXTRACTION=PAF \
             /ROTATION=EQUAMAX.",
            &[],
        ),
        (
            "pc-promax",
            "FACTOR VARIABLES=x1 TO x6 /CRITERIA=FACTORS(2) /EXTRACTION=PC /ROTATION=PROMAX(4).",
            &[
                ("Total Variance Explained", ANY, 6),
                ("Total Variance Explained", ANY, 7),
                ("Total Variance Explained", ANY, 8),
            ],
        ),
        (
            "weighted-pairwise",
            "WEIGHT BY w.\nFACTOR VARIABLES=x1 TO x6 /MISSING=PAIRWISE /PRINT=INITIAL EXTRACTION ROTATION KMO \
             /CRITERIA=FACTORS(2) /EXTRACTION=PC /ROTATION=VARIMAX.",
            // Bartlett's test with PAIRWISE: ours takes the smallest pair count as N, PSPP
            // another count
            &[
                ("KMO and Bartlett's Test", 1, 0),
                ("KMO and Bartlett's Test", 3, 0),
            ],
        ),
        (
            "include-analysis",
            // PSPP's LISTWISE looks at the ANALYSIS variables only, the manual's at all of
            // VARIABLES; ANALYSIS keeps the variables with missing values so both agree
            "MISSING VALUES x5 (6).\nFACTOR VARIABLES=x1 TO x5 /MISSING=LISTWISE INCLUDE /ANALYSIS=x1 x2 x4 x5 \
             /PRINT=UNIVARIATE INITIAL EXTRACTION ROTATION /CRITERIA=FACTORS(2) /EXTRACTION=PC /ROTATION=VARIMAX.",
            &sd,
        ),
    ];
    for (case, syntax, skip) in cases {
        compare_factor(&format!("factor-{case}"), syntax, skip);
    }
}

fn numeric(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => (0..data.case_count()).map(|k| v.get(k)).collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

/// The final centers agree once the clusters are matched, and so do the counts, the saved
/// cluster of every case and its distance.
#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn quick_cluster_centers_and_saved_values() {
    for (case, prefix, criteria) in [
        ("kmeans", "", "CLUSTERS(3) MXITER(20)"),
        ("noinitial", "", "CLUSTERS(3) NOINITIAL MXITER(20)"),
        // PSPP's weighted centers are not the weighted means of their cases, and PSPP stops on
        // an internal error with PAIRWISE, so both are checked against an independent
        // computation in factor.rs only
        ("two", "", "CLUSTERS(2) MXITER(20)"),
        ("four", "", "CLUSTERS(4) MXITER(30)"),
    ] {
        let Some((program, dir)) = setup(case) else {
            return;
        };
        let job = |out: &str| {
            format!(
                "{DATA}{prefix}QUICK CLUSTER x1 x2 x4 x5 /CRITERIA={criteria} /SAVE=CLUSTER(cl) DISTANCE(di).\n\
                 SAVE OUTFILE='{{dir}}{out}.sav'.\n"
            )
        };
        let output = pspp(case, &program, &dir, &job("pspp"));
        let mine = ours(case, &dir, &job("ours"));
        let theirs = pspp_tables(&output);
        let (_, rows) = theirs
            .iter()
            .find(|(t, _)| t == "Final Cluster Centers")
            .expect("PSPP's final centers");
        let k = rows[0].len() - 1;
        let pspp_center = |j: usize| -> Vec<f64> {
            rows[2..]
                .iter()
                .map(|r| r[1 + j].parse().unwrap())
                .collect()
        };
        let table = mine
            .iter()
            .find(|t| t.title == "Final Cluster Centers")
            .unwrap();
        let our_center = |j: usize| -> Vec<f64> {
            table
                .rows
                .iter()
                .map(|r| match r[1 + j] {
                    Cell::Number(n) => n,
                    _ => f64::NAN,
                })
                .collect()
        };
        // PSPP's cluster j is our cluster map[j]
        let map: Vec<usize> = (0..k)
            .map(|j| {
                let p = pspp_center(j);
                (0..k)
                    .find(|&o| {
                        our_center(o)
                            .iter()
                            .zip(&p)
                            .all(|(a, b)| (a - b).abs() < 1e-9)
                    })
                    .unwrap_or_else(|| {
                        panic!("{case}: no center of ours is PSPP's {p:?}\n{output}")
                    })
            })
            .collect();
        let read = |name: &str| Dataset::read_sav(dir.join(format!("{name}.sav"))).unwrap();
        let (a, b) = (read("pspp"), read("ours"));
        let (pc, oc) = (numeric(&a, "cl"), numeric(&b, "cl"));
        let (pd, od) = (numeric(&a, "di"), numeric(&b, "di"));
        // PSPP writes the saved values of the clustered cases to the first cases of the file,
        // so after case 18, which misses x1, they sit one case early; ours stay with their case
        // and case 18 gets none, as LISTWISE says
        let theirs: Vec<(f64, f64)> = pc
            .iter()
            .zip(&pd)
            .filter_map(|(c, d)| Some((c.unwrap_or(f64::NAN), (*d)?)))
            .collect();
        let mine: Vec<(f64, f64)> = oc
            .iter()
            .zip(&od)
            .filter_map(|(c, d)| Some(((*c)?, (*d)?)))
            .collect();
        assert_eq!(od[17], None, "{case}");
        let mut compared = 0;
        for (i, (&(p, pdist), &(o, odist))) in theirs.iter().zip(&mine).enumerate() {
            assert_eq!(
                map[p as usize - 1] + 1,
                o as usize,
                "{case}: clustered case {}",
                i + 1
            );
            assert!(
                (pdist - odist).abs() <= 1e-9 * pdist.max(1.0),
                "{case}: clustered case {}: distance PSPP {pdist}, ours {odist}",
                i + 1
            );
            compared += 2;
        }
        assert!(compared > 70, "{case}: {compared}");
        eprintln!("{case}: {} centers and {compared} saved values agree", k);
        fs::remove_dir_all(&dir).unwrap();
    }
}
