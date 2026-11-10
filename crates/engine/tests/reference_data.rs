//! The data management commands compared with GNU PSPP, run as a black box on the same
//! fixture and the same syntax. Ignored by default because they need the PSPP program:
//!
//! ```text
//! STATSCRAFT_PSPP=/path/to/pspp cargo test -p statscraft-engine --test reference_data -- --ignored
//! ```

use statscraft_data::{Dataset, Values};
use statscraft_engine::Engine;
use statscraft_syntax::parse;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, fs};

/// Runs `commands` after `GET` of the fixture in both programs and compares the datasets they
/// end with, and every file in `files` that the commands write. In `commands` a file to write
/// is named `'{out}name.sav'`.
fn compare(case: &str, fixture: &str, commands: &str, files: &[&str]) {
    compare_with(case, fixture, commands, files, &[]);
}

/// `compare` with text files written to the folder of the run first, as (name, content). In
/// `commands` the folder is `{dir}`.
fn compare_with(
    case: &str,
    fixture: &str,
    commands: &str,
    files: &[&str],
    inputs: &[(&str, &str)],
) {
    let Some(pspp) = env::var_os("STATSCRAFT_PSPP") else {
        eprintln!("{case}: skipped, STATSCRAFT_PSPP is not set");
        return;
    };
    let dir = env::temp_dir().join(format!(
        "statscraft-reference-{}-{case}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    fs::copy(source.join(fixture), dir.join("data.sav")).unwrap();
    for (name, content) in inputs {
        fs::write(dir.join(name), content).unwrap();
    }
    let job = |get: &str, out: &str, folder: &str| {
        format!("GET FILE='{get}'.\n{commands}\nSAVE OUTFILE='{{out}}final.sav'.\n")
            .replace("{out}", out)
            .replace("{dir}", folder)
    };

    // PSPP is a Windows program on the development machine: relative paths only
    fs::write(dir.join("job.sps"), job("data.sav", "pspp_", "")).unwrap();
    let status = Command::new(&pspp)
        .args(["job.sps", "-o", "pspp.csv", "-O", "format=csv"])
        .current_dir(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", PathBuf::from(&pspp).display()));
    let log = fs::read_to_string(dir.join("pspp.csv")).unwrap_or_default();
    assert!(status.success(), "{case}: PSPP failed\n{log}");

    let absolute = |name: &str| dir.join(name).to_str().unwrap().to_string();
    let folder = format!("{}/", dir.display());
    let (parsed, errors) = parse(&job(&absolute("data.sav"), &absolute("ours_"), &folder));
    assert!(errors.is_empty(), "{case}: {errors:?}");
    let mut engine = Engine::default();
    for command in &parsed {
        if let Err(error) = engine.execute(&command.command) {
            panic!("{case}: line {}: {error}", command.first_line);
        }
    }

    for file in files.iter().chain(&["final.sav"]) {
        let read = |prefix: &str| Dataset::read_sav(dir.join(format!("{prefix}{file}"))).unwrap();
        let differences = differences(&read("pspp_"), &read("ours_"));
        assert!(
            differences.is_empty(),
            "{case}: {file} differs from PSPP ({} differences, files kept in {})\n{}",
            differences.len(),
            dir.display(),
            differences[..differences.len().min(25)].join("\n")
        );
    }
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
            "weight variable".to_string(),
            format!("{:?}", described.weight_variable),
        ),
    ];
    for v in &described.variables {
        let fields = [
            ("type", format!("{} {:?}", v.kind, v.width)),
            ("format", v.format.clone()),
            ("label", format!("{:?}", v.label)),
            ("measure", v.measure.to_string()),
            ("value labels", {
                // PSPP writes the labels in value order, we keep the order of the file
                let mut labels: Vec<_> = v.value_labels.iter().collect();
                labels.sort_by_key(|(value, _)| value.as_str());
                format!("{labels:?}")
            }),
            (
                "missing values",
                format!("{:?} {:?}", v.missing_values, v.missing_range),
            ),
        ];
        lines.extend(fields.map(|(field, value)| (format!("{} {field}", v.name), value)));
    }
    lines
}

/// Dictionary differences, or when there are none the cells that differ.
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
        let mut differ = |case: usize, a_value: String, b_value: String| {
            let (name, case) = (a.name(), case + 1);
            found.push(format!(
                "{name} case {case}: PSPP {a_value}, ours {b_value}"
            ));
        };
        match (a.values(), b.values()) {
            (Values::Numeric(x), Values::Numeric(y)) => {
                for (case, (x, y)) in x.iter().zip(y.iter()).enumerate() {
                    // PSPP stores NaN for the MEDIAN of a group without valid values
                    let x = x.filter(|x| !x.is_nan());
                    let close = match (x, y) {
                        (Some(x), Some(y)) => (x - y).abs() <= 1e-9 * x.abs().max(y.abs()).max(1.0),
                        _ => x == y,
                    };
                    if !close {
                        differ(case, format!("{x:?}"), format!("{y:?}"));
                    }
                }
            }
            (Values::String(x), Values::String(y)) => {
                for (case, (x, y)) in x.iter().zip(y.iter()).enumerate() {
                    if x != y {
                        differ(case, format!("{x:?}"), format!("{y:?}"));
                    }
                }
            }
            _ => unreachable!("the types were compared above"),
        }
    }
    found
}

const SURVEY: &str = "survey_basic.sav";
/// The survey with a weight variable set.
const WEIGHTED: &str = "survey_basic_phpwriter.sav";
const EDGE: &str = "edge_cases.sav";

fn compare_all(command: &str, cases: &[(&str, &str, &str)]) {
    for (case, fixture, commands) in cases {
        compare(&format!("{command}-{case}"), fixture, commands, &[]);
    }
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn recode() {
    compare_all(
        "recode",
        &[
            (
                "missing-values",
                SURVEY,
                "RECODE q1_satisfaction (99=7) (1 THRU 3=1) INTO a.
RECODE q2_recommend (90 THRU 100=50) (0 THRU 6=1) INTO b.
RECODE q2_recommend (LO THRU HI=1) INTO c.
RECODE monthly_spend (LOWEST THRU HIGHEST=1) INTO c2.
RECODE monthly_spend (100 THRU HI=1) (ELSE=2) INTO d.
RECODE q2_recommend (MISSING=-1) (ELSE=COPY) INTO e.
RECODE monthly_spend (MISSING=-1) (ELSE=COPY) INTO e2.
RECODE q2_recommend (SYSMIS=-1) (ELSE=COPY) INTO f.
RECODE monthly_spend (SYSMIS=-1) (100 THRU 150=SYSMIS) INTO f2.
RECODE q2_recommend (MISSING=COPY) (ELSE=0) INTO g.
RECODE q2_recommend (99=1) (MISSING=2) (98=3) INTO h.",
            ),
            (
                "in-place",
                SURVEY,
                "RECODE region (1,2=10) (3 THRU 4=20).
RECODE q2_recommend (MISSING=SYSMIS).
RECODE q5_1 TO q5_5 (1=0) (0=1).
RECODE age (LO THRU 30=1) (30 THRU 59=2) (60 THRU HI=3) (ELSE=SYSMIS).
RECODE monthly_spend (LO THRU 100=-1.5) (100 THRU 200=0) (SYSMIS=-9).
RECODE q1_satisfaction (98, 99=SYSMIS) (ELSE=COPY).
RECODE gender (3 THRU 2=9).",
            ),
            (
                // The original leaves an existing target unchanged where nothing matches and PSPP
                // clears it; with an explicit ELSE both programs agree, so the cases here use one.
                "into-existing-target",
                SURVEY,
                "RECODE gender (1=100) (ELSE=SYSMIS) INTO age.
RECODE q1_satisfaction (99=7) (98=8) (ELSE=SYSMIS) INTO q2_recommend.
RECODE q5_1 q5_2 (1=2) (ELSE=SYSMIS) INTO q5_2 q5_3.
RECODE q5_4 (1=1) (ELSE=SYSMIS) INTO q5_5.",
            ),
            (
                "strings",
                SURVEY,
                "RECODE plan ('BASIC'='B') ('plus'='P') ('PLUS   '='PP').
RECODE plan ('B'=1) ('PP'=2) INTO pn.
RECODE plan ('B'=1) ('PP'=2) (ELSE=SYSMIS) INTO pn2.
RECODE gender (1='M') (2='F') (ELSE='?') INTO plan.
RECODE plan ('M'='Male') (MISSING='none') (ELSE=COPY).",
            ),
            (
                "string-copy-into-a-narrower-target",
                SURVEY,
                "RECODE q3_comment (''='none') (ELSE=COPY) INTO plan.
RECODE plan ('none'='x') (ELSE=' ') INTO q3_comment.",
            ),
            (
                "groups-and-lists",
                SURVEY,
                "RECODE gender q5_1 (1=5) (ELSE=6) INTO m1 m2 / q5_2 (0=9)
  / q6_mention1 TO q6_mention3 (SYSMIS=0) (ELSE=COPY) INTO n1 TO n3.
RECODE m1 TO n3 (5 THRU 6=1).",
            ),
            (
                "decimals-and-dates",
                SURVEY,
                "RECODE monthly_spend (78.28=1) (127.1=2) (ELSE=3) INTO fl.
RECODE signup_date (LO THRU 13800000000=1) (ELSE=2) INTO sd.
RECODE weight (LO THRU 1=-0.5) (ELSE=COPY) INTO w2.",
            ),
            (
                "missing-definitions",
                EDGE,
                "RECODE range_plus_one (MISSING=1) (ELSE=0) INTO rm.
RECODE range_plus_one (-1=1) (95 THRU HI=2) (SYSMIS=3) INTO rv.
RECODE three_missing (97 THRU 98=1) (MISSING=2) (ELSE=3) INTO tm.
RECODE all_missing (SYSMIS=5).
RECODE extreme_numbers (LO THRU 0=1) (0 THRU HI=2) INTO en.
RECODE country_code ('EE'='FI') (ELSE=COPY).",
            ),
        ],
    );
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn count() {
    compare_all(
        "count",
        &[
            (
                "values-and-missing",
                SURVEY,
                "COUNT c1 = q5_1 TO q5_5 (1).
COUNT c2 = q1_satisfaction q2_recommend (99).
COUNT c3 = q1_satisfaction q2_recommend (90 THRU HI).
COUNT c4 = q1_satisfaction q2_recommend monthly_spend (MISSING).
COUNT c5 = q1_satisfaction q2_recommend monthly_spend q6_mention2 (SYSMIS).
COUNT c7 = q2_recommend (LO THRU 5, 10).
COUNT c8 = q2_recommend q1_satisfaction (6 THRU 4).
COUNT c10 = q2_recommend monthly_spend (LO THRU HI).",
            ),
            (
                "several-lists-and-strings",
                SURVEY,
                "COUNT c6 = q5_1 q5_2 (1) q6_mention1 q6_mention2 (1, 3 THRU 4) plan ('BASIC', 'PLUS  ')
  q3_comment ('').",
            ),
            (
                "targets",
                SURVEY,
                "COUNT age = gender (1).
COUNT c8 = gender (2) / c9 = q5_1 q5_1 (0 1).
COUNT c10 = c8 c9 (1 2).
COUNT c8 = c8 TO c10 (2 THRU 3) / c9 = c8 (1).",
            ),
            (
                "missing-definitions",
                EDGE,
                "COUNT m = range_plus_one three_missing all_missing (MISSING)
  / s = range_plus_one three_missing all_missing (SYSMIS)
  / r = range_plus_one (-1 90 THRU 95) / cc = country_code plan_name ('EE' 'FI' 'PLUS').",
            ),
        ],
    );
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn sort_cases() {
    compare_all(
        "sort",
        &[
            (
                "system-missing-and-direction",
                SURVEY,
                "SORT CASES BY q6_mention2 region (D).",
            ),
            ("string-key", SURVEY, "SORT CASES BY plan (D) q2_recommend."),
            (
                "ties-keep-their-order",
                SURVEY,
                "SORT CASES age (D).\nSORT CASES BY gender (A).",
            ),
            (
                "empty-strings-and-decimals",
                SURVEY,
                "SORT CASES BY q3_comment monthly_spend (DOWN).",
            ),
            (
                "to-list",
                SURVEY,
                "SORT CASES BY gender TO region (D) q5_1 (UP) resp_id (D).",
            ),
            ("weight-variable", WEIGHTED, "SORT CASES BY weight (D)."),
            (
                "key-given-twice",
                SURVEY,
                "SORT CASES BY region gender (D) region gender.",
            ),
            (
                "extreme-numbers",
                EDGE,
                "SORT CASES BY all_missing extreme_numbers (D).",
            ),
            (
                "missing-definitions",
                EDGE,
                "SORT CASES BY three_missing range_plus_one (D) plan_name.",
            ),
            (
                "non-ascii-text",
                "unicode.sav",
                "SORT CASES BY linn_õäöü (D).\nSORT CASES BY keel tervitus.",
            ),
        ],
    );
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn rename_delete_weight() {
    compare_all(
        "variables",
        &[
            (
                "rename-forms",
                WEIGHTED,
                "RENAME VARIABLES (gender age = age gender).
RENAME VARIABLES region = area.
RENAME VARIABLES (q5_1 TO q5_3 = ch_a ch_b ch_c) (plan = tariff).
RENAME VARIABLES q6_mention1 = m1 q6_mention2 = m2.
RENAME VARIABLES (weight = w).
RENAME VARIABLES (m1 = M1).",
            ),
            (
                "rename-at-once",
                WEIGHTED,
                "RENAME VARIABLES (q5_4 q5_5 = z1 TO z2) (gender = age) (age = gender).",
            ),
            (
                "rename-all",
                WEIGHTED,
                "RENAME VARIABLES (ALL = v01 TO v19).",
            ),
            (
                "delete",
                WEIGHTED,
                "DELETE VARIABLES q3_comment signup_date TO q5_5.\nDELETE VARIABLES age age.",
            ),
            (
                "delete-the-weight-variable",
                WEIGHTED,
                "DELETE VARIABLES weight.",
            ),
            ("weight-off", WEIGHTED, "WEIGHT OFF."),
            ("weight-by", SURVEY, "WEIGHT BY weight."),
            (
                "weight-follows-a-rename",
                WEIGHTED,
                "WEIGHT age.\nRENAME VARIABLES (age = years).\nDELETE VARIABLES gender weight.",
            ),
        ],
    );
}

/// Every function on a numeric variable with system-missing values (`monthly_spend`), on one
/// with user-missing values (`q1_satisfaction`), on a string and on a date.
const EVERY_FUNCTION: &str = "
  /s_sum = SUM(monthly_spend) /s_mean = MEAN(monthly_spend) /s_sd = SD(monthly_spend)
  /s_med = MEDIAN(monthly_spend) /s_min = MIN(monthly_spend) /s_max = MAX(monthly_spend)
  /s_first = FIRST(monthly_spend) /s_last = LAST(monthly_spend)
  /n_all = N /n_spend = N(monthly_spend) /nu_all = NU /nu_spend = NU(monthly_spend)
  /s_nmiss = NMISS(monthly_spend) /s_numiss = NUMISS(monthly_spend)
  /q1_mean 'Mean of q1' = MEAN(q1_satisfaction) /q1_sum = SUM(q1_satisfaction)
  /q1_sd = SD(q1_satisfaction) /q1_med = MEDIAN(q1_satisfaction)
  /q1_min = MIN(q1_satisfaction) /q1_max 'Highest q1' = MAX(q1_satisfaction)
  /q1_first = FIRST(q1_satisfaction) /q1_last = LAST(q1_satisfaction)
  /q1_n = N(q1_satisfaction) /q1_nu = NU(q1_satisfaction)
  /q1_nmiss = NMISS(q1_satisfaction) /q1_numiss = NUMISS(q1_satisfaction)
  /p_min = MIN(plan) /p_max = MAX(plan) /p_first = FIRST(plan) /p_last = LAST(plan)
  /p_n = N(plan) /p_nmiss = NMISS(plan) /c_first = FIRST(q3_comment) /c_max = MAX(q3_comment)
  /d_min = MIN(signup_date) /d_last = LAST(signup_date) /d_mean = MEAN(signup_date)
  /d_sd = SD(signup_date) /d_med = MEDIAN(signup_date)
  /a_mean r_mean = MEAN(age q2_recommend) /m1 TO m3 = MEAN(q5_1 TO q5_3).";

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn aggregate() {
    let every = |start: &str| format!("{start}{EVERY_FUNCTION}");
    let cases = [
        (
            "replace",
            SURVEY,
            every("AGGREGATE /OUTFILE=* /BREAK=region gender"),
        ),
        (
            "replace-weighted",
            WEIGHTED,
            every("AGGREGATE /OUTFILE=* /BREAK=region"),
        ),
        (
            "small-weighted-groups",
            WEIGHTED,
            every("AGGREGATE OUTFILE=* MODE=REPLACE /BREAK=age gender"),
        ),
        (
            "break-is-system-missing-or-user-missing",
            SURVEY,
            every("AGGREGATE /OUTFILE=* /BREAK=q6_mention2 q2_recommend"),
        ),
        (
            "string-break-descending",
            WEIGHTED,
            every("AGGREGATE /OUTFILE=* /BREAK=plan (D) q5_1"),
        ),
        ("no-break", WEIGHTED, every("AGGREGATE /OUTFILE=*")),
        (
            "break-variable-given-twice",
            SURVEY,
            every("AGGREGATE /OUTFILE=* /BREAK=gender region gender"),
        ),
        (
            // PSPP forms groups from neighbouring cases in this mode, so sort first
            "add-variables",
            SURVEY,
            every(
                "SORT CASES BY region gender.
AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=region gender",
            ),
        ),
        (
            "add-variables-weighted",
            WEIGHTED,
            every(
                "SORT CASES BY q6_mention2 (D).
AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=q6_mention2 (D)",
            ),
        ),
        (
            "add-variables-no-break",
            WEIGHTED,
            every("AGGREGATE /OUTFILE=* MODE=ADDVARIABLES"),
        ),
        (
            "target-takes-the-name-of-a-dropped-variable",
            SURVEY,
            "AGGREGATE /OUTFILE=* /BREAK=gender /age = MEAN(age) /region 'Cases' = N
  /plan = MAX(q1_satisfaction) /resp_id = FIRST(plan)."
                .to_string(),
        ),
        (
            // zero and negative weights; missing ones in the next case
            "unusable-weights",
            WEIGHTED,
            every(
                "RECODE weight (LO THRU 0.6 = 0) (0.6 THRU 0.7 = -1).
AGGREGATE /OUTFILE=* /BREAK=q6_mention2",
            ),
        ),
        (
            "missing-weights",
            WEIGHTED,
            every(
                "RECODE weight (LO THRU 0.8 = SYSMIS).
MISSING VALUES weight (2 THRU HI).
AGGREGATE /OUTFILE=* /BREAK=region q5_1 q5_2",
            ),
        ),
        (
            "nothing-valid",
            EDGE,
            "AGGREGATE /OUTFILE=* /BREAK=country_code /s = SUM(all_missing) /m = MEAN(all_missing)
  /sd = SD(all_missing) /mn = MIN(all_missing) /f = FIRST(all_missing) /n = N(all_missing)
  /nm = NMISS(all_missing) /t_min = MIN(three_missing) /r_max = MAX(range_plus_one)
  /r_n = N(range_plus_one) /e_first = FIRST(empty_text) /x_sum = SUM(extreme_numbers)
  /x_sd = SD(extreme_numbers) /x_med = MEDIAN(extreme_numbers)."
                .to_string(),
        ),
    ];
    for (case, fixture, commands) in &cases {
        compare(&format!("aggregate-{case}"), fixture, commands, &[]);
    }
    compare(
        "aggregate-to-a-file",
        WEIGHTED,
        &every("AGGREGATE /OUTFILE='{out}groups.sav' /BREAK=gender region"),
        &["groups.sav"],
    );
}

#[test]
#[ignore = "needs PSPP: set STATSCRAFT_PSPP"]
fn data_list() {
    compare_all(
        "data-list",
        &[
            (
                // PSPP makes an N variable scale whatever its values; here n3 has no value
                // below 10, which makes it scale by the rules of the manual too
                "fixed-columns",
                SURVEY,
                "DATA LIST /id 1-3 sex 5-6 (A) age 7-8 op1 TO op3 10-12 rate 14-18 (PCT,2)
  cost 20-24 (DOLLAR) c2 26-31 (COMMA,1) n3 33-35 (N) f2 37-41 (2) dt 43-53 (DATE).
BEGIN DATA
001 m 28 122   935 78788  1,234 012 12345  1-JAN-2001
002  f 9 2 2  1.5     12    9.5 1x2  -201 03-Feb-1999
  3 x 4a     -201  $1,0     -12      2 01 3 2 2001
004   10  1e -1.5  1.2e1    999 345   .20 3.II.1999
END DATA.",
            ),
            (
                "fixed-formats-and-records",
                SURVEY,
                "DATA LIST RECORDS=3 /1 name (A6) score (T9, F4.1) /3 born 1-10 (ADATE)
  w (1X, COMMA6.2) e 20-25 (E,1) d (1X, DOT8.1) s (1X, SDATE10).
BEGIN DATA
Ann     123
skipped record
01/31/1990 1,2345  12e1   1.234,5  2001/02/03
  Bob   99.5
second skipped record
2/3/1985        7  1+2    12,5     99.12.31
END DATA.",
            ),
            (
                "list",
                SURVEY,
                "DATA LIST LIST /a b (F2) s (A4) c (DOLLAR8.2) d (EDATE10).
BEGIN DATA
1 2 abc $1,234.5 31.12.1999
3,,\"x y\" 5 1/2/2003
 4 , 5 , 'q r' , 6 , 3.3.2003
6 7 toolong 8
9 10 z 11 4/4/2004
END DATA.",
            ),
            (
                "list-with-delimiters",
                SURVEY,
                "DATA LIST LIST (\",\") SKIP=1 /a (F2) b (F2) s (A4).
BEGIN DATA
header line
1,2,abc
3,,x y
 4 , 5 , \"q,r\"
6,,
,7
END DATA.",
            ),
            (
                "free",
                SURVEY,
                "DATA LIST FREE /a b * s (A3) c.
BEGIN DATA
1 2 abc 3
4 x 5 6
7 8 lmnop 9 10
END DATA.",
            ),
            (
                "free-with-tabs",
                SURVEY,
                "DATA LIST FREE (TAB) /a b (F3) c.
BEGIN DATA
1\t2\t3
4\t\t6\t7\t8
9
END DATA.",
            ),
        ],
    );
    // PSPP gives data read from text measurement levels by rules of its own (decimals in the
    // format make a variable scale, for one); the manual decides by the values, so these
    // values and formats lead both rules to the same levels
    compare_with(
        "data-list-file",
        SURVEY,
        "DATA LIST FILE='{dir}input.txt' NOTABLE /id 1-2 name 4-8 (A) score 10-13 (1).
SAVE OUTFILE='{out}fixed.sav'.
DATA LIST FILE='{dir}input.txt' FREE /a (F2) b (A3) c.",
        &["fixed.sav"],
        &[("input.txt", "10 Ann   123\n20 Bob   45.6\n30 Cy     -7\n")],
    );
}
