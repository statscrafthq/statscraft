//! Compares transformations with GNU PSPP, run as a black box on the same file.
//!
//! The tests are ignored by default. They run when STATSCRAFT_PSPP names the PSPP executable:
//!
//! ```text
//! STATSCRAFT_PSPP=/path/to/pspp cargo test -p statscraft-engine --test reference -- --ignored
//! ```
//!
//! `check(fixture, syntax)` loads the fixture in both programs, runs the syntax, saves both
//! results and compares them with our own reader: variable names, types and every value.
//! Keep the syntax ASCII, because PSPP reads a syntax file in the encoding of its locale.

use statscraft_data::{Dataset, Values};
use statscraft_engine::Engine;
use statscraft_syntax::parse;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static RUNS: AtomicUsize = AtomicUsize::new(0);

fn close(a: f64, b: f64) -> bool {
    a == b || (a - b).abs() <= 1e-9 * a.abs().max(b.abs())
}

fn run_ours(source: &str) {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}\n{source}");
    let mut engine = Engine::default();
    for parsed in &commands {
        if let Err(error) = engine.execute(&parsed.command) {
            panic!("line {}: {error}\n{source}", parsed.first_line);
        }
    }
    engine.finish().unwrap();
}

fn check(fixture: &str, syntax: &str) {
    let pspp = std::env::var("STATSCRAFT_PSPP").expect("STATSCRAFT_PSPP names the PSPP executable");
    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("statscraft-ref-{}-{run}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures");
    std::fs::copy(source.join(fixture), dir.join(fixture)).unwrap();

    // PSPP here may be a Windows program, so it gets relative paths and runs in the directory
    let job = format!("GET FILE='{fixture}'.\n{syntax}\nSAVE OUTFILE='pspp_out.sav'.\n");
    std::fs::write(dir.join("job.sps"), job).unwrap();
    let output = Command::new(&pspp)
        .current_dir(&dir)
        .args(["job.sps", "-o", "pspp_out.csv", "-O", "format=csv"])
        .output()
        .unwrap_or_else(|e| panic!("cannot run {pspp}: {e}"));
    let messages = String::from_utf8_lossy(&output.stderr);
    let Ok(reference) = Dataset::read_sav(dir.join("pspp_out.sav")) else {
        panic!("PSPP wrote no result for:\n{syntax}\n{messages}");
    };

    let ours = dir.join("ours.sav");
    run_ours(&format!(
        "GET FILE='{}'.\n{syntax}\nSAVE OUTFILE='{}'.\n",
        dir.join(fixture).display(),
        ours.display()
    ));
    let ours = Dataset::read_sav(&ours).unwrap();

    let names = |data: &Dataset| -> Vec<(String, bool)> {
        let variables = data.variables();
        variables
            .map(|v| (v.name().to_string(), v.is_string()))
            .collect()
    };
    assert_eq!(
        names(&ours),
        names(&reference),
        "variables after:\n{syntax}"
    );
    assert_eq!(
        ours.case_count(),
        reference.case_count(),
        "cases after:\n{syntax}"
    );
    let mut differences = Vec::new();
    for (mine, theirs) in ours.variables().zip(reference.variables()) {
        for case in 0..ours.case_count() {
            let (same, shown) = match (mine.values(), theirs.values()) {
                (Values::Numeric(a), Values::Numeric(b)) => {
                    let (a, b) = (a.get(case), b.get(case));
                    let same = match (a, b) {
                        (Some(a), Some(b)) => close(a, b),
                        _ => a == b,
                    };
                    (same, format!("{a:?} but PSPP has {b:?}"))
                }
                (Values::String(a), Values::String(b)) => {
                    let (a, b) = (a.get(case), b.get(case));
                    (a == b, format!("{a:?} but PSPP has {b:?}"))
                }
                _ => unreachable!("types were compared above"),
            };
            if !same {
                differences.push(format!("{} case {}: {shown}", mine.name(), case + 1));
            }
        }
    }
    assert!(
        differences.is_empty(),
        "{} differences after:\n{syntax}\n{}",
        differences.len(),
        differences[..differences.len().min(12)].join("\n")
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

fn survey(cases: &[&str]) {
    for syntax in cases {
        check("survey_basic.sav", syntax);
    }
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn arithmetic_and_missing_values() {
    survey(&[
        "COMPUTE r = q1_satisfaction + 1.",
        "COMPUTE r = q1_satisfaction.",
        "COMPUTE r = q2_recommend - monthly_spend.",
        "COMPUTE r = q2_recommend * monthly_spend.\nCOMPUTE r2 = monthly_spend * q5_1.",
        // PSPP makes 0 / 0 zero; the manual makes any divisor of 0 missing, so it is left out
        "IF (q5_1 <> 0 OR MISSING(q5_1)) r = q2_recommend / q5_1.\nCOMPUTE r2 = q5_1 / monthly_spend.\n\
         IF (q5_2 <> 0 OR MISSING(q5_2)) r3 = q5_1 / q5_2.",
        "COMPUTE r = age ** 2 - -3 / 4.\nCOMPUTE r2 = -age ** 2.\nCOMPUTE r3 = (2 ** 3) ** 2.",
        // PSPP makes 0 ** missing zero and missing ** 0 one, which the manual does not, so
        // missing operands of ** are left out
        "IF (NOT MISSING(q6_mention2)) r = q5_1 ** q6_mention2.\n\
         IF (NOT MISSING(q6_mention2)) r2 = q6_mention2 ** q5_1.\n\
         COMPUTE r3 = q5_1 ** q5_2.\nCOMPUTE r4 = (q5_1 - 2) ** 0.5.\n\
         COMPUTE r5 = q5_1 ** (q5_2 - 1).\nIF (NOT MISSING(monthly_spend)) r6 = monthly_spend ** 0.",
        "COMPUTE r = monthly_spend / 3 * 3.\nCOMPUTE r2 = -(age - 50) * (monthly_spend + 1).",
        "COMPUTE r = 10 ** 200 * 10 ** 200.\nCOMPUTE r2 = age / 7 + weight * 1e-3.",
        "COMPUTE r = $SYSMIS.\nCOMPUTE r2 = $SYSMIS + 1.",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn comparisons_and_logic() {
    survey(&[
        "COMPUTE r = q1_satisfaction >= 4.\nCOMPUTE r2 = age <> 49.\n\
         COMPUTE r3 = monthly_spend < 100.\nCOMPUTE r4 = q1_satisfaction = 99.\n\
         COMPUTE r5 = age LE 30.\nCOMPUTE r6 = age GT q2_recommend * 6.",
        "COMPUTE r = q5_1 AND q1_satisfaction > 3.\nCOMPUTE r2 = q5_1 OR q1_satisfaction > 3.",
        "COMPUTE r = NOT (q1_satisfaction > 3).\nCOMPUTE r2 = ~q5_1.",
        "COMPUTE r = q5_1 = 1 & q5_2 = 1 | q5_3 = 1.\n\
         COMPUTE r2 = q1_satisfaction > 3 AND q2_recommend > 5 OR monthly_spend > 200.",
        "COMPUTE r = (q1_satisfaction > 3) + (q2_recommend > 5).",
        "COMPUTE r = plan = 'BASIC'.\nCOMPUTE r2 = plan = 'BASIC     '.\n\
         COMPUTE r3 = plan < 'PLUS'.\nCOMPUTE r4 = plan >= 'ENTERPRISE  '.\n\
         COMPUTE r5 = 'a' < 'B'.\nCOMPUTE r6 = plan <> 'PLUS '.\nCOMPUTE r7 = q3_comment = ''.",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn arithmetic_functions() {
    survey(&[
        "COMPUTE r = ABS(age - 50).\nCOMPUTE r2 = SQRT(age - 50).\nCOMPUTE r3 = EXP(q2_recommend).",
        "COMPUTE r = LN(q2_recommend).\nCOMPUTE r2 = LG10(monthly_spend).\n\
         COMPUTE r3 = LN(age - 50).\nCOMPUTE r4 = EXP(age * 20).",
        "COMPUTE r = RND(monthly_spend).\nCOMPUTE r2 = RND(monthly_spend / 7).\n\
         COMPUTE r3 = RND(-monthly_spend).\nCOMPUTE r4 = RND(age - 50.5).",
        // PSPP's slack near a half does not grow with the value, so 188.45 in steps of 0.1 rounds
        // down there and up here; steps of 0.25 divide the cents without landing near a half
        "COMPUTE r = RND(monthly_spend, 5).\nCOMPUTE r2 = RND(monthly_spend, 0.25).\n\
         COMPUTE r3 = RND(monthly_spend, 0).\nCOMPUTE r4 = RND(monthly_spend, -2).\n\
         COMPUTE r5 = RND(age, q5_1).",
        "COMPUTE r = RND(0.5 - 2**-48).\nCOMPUTE r2 = RND(0.5 - 2**-46).\n\
         COMPUTE r3 = RND(-0.5 + 2**-48).\nCOMPUTE r4 = RND(2.5 - 2**-44, 1, 10).\n\
         COMPUTE r5 = RND(2.5 - 2**-44, 1, 2).\nCOMPUTE r6 = RND(q5_1 + 0.5 - 2**-49).",
        // PSPP's slack does not grow with the value, so 278.21 * 100 (27820.999999999996) stays
        // below 27821 there; quarters land exactly on whole numbers in both programs
        "COMPUTE r = TRUNC(monthly_spend).\nCOMPUTE r2 = TRUNC(-monthly_spend).\n\
         COMPUTE r3 = TRUNC(monthly_spend, 10).\nCOMPUTE r4 = TRUNC(monthly_spend * 4).\n\
         COMPUTE r5 = TRUNC(monthly_spend, 0.25).",
        "COMPUTE r = TRUNC(1 - 2**-48).\nCOMPUTE r2 = TRUNC(1 - 2**-46).\n\
         COMPUTE r3 = TRUNC(-1 + 2**-48).\nCOMPUTE r4 = TRUNC(1 - 2**-46, 1, 8).\n\
         COMPUTE r5 = TRUNC(age / 3, 0).",
        "COMPUTE r = MOD(age, 7).\nCOMPUTE r2 = MOD(-age, 7).\nCOMPUTE r3 = MOD(monthly_spend, 2.5).\n\
         COMPUTE r4 = MOD(age, q5_1).\nCOMPUTE r5 = MOD(q5_1, monthly_spend).\n\
         COMPUTE r6 = MOD(q5_1, q5_2).",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn statistical_functions() {
    survey(&[
        "COMPUTE r = SUM(q5_1 TO q5_5).\nCOMPUTE r2 = SUM(q1_satisfaction, q2_recommend).\n\
         COMPUTE r3 = SUM(q6_mention1, q6_mention2, q6_mention3).",
        "COMPUTE r = MEAN(q1_satisfaction, q2_recommend, monthly_spend).\n\
         COMPUTE r2 = MEAN.2(q1_satisfaction, q2_recommend, monthly_spend).\n\
         COMPUTE r3 = MEAN.3(q1_satisfaction, q2_recommend, monthly_spend).",
        "COMPUTE r = MIN(q6_mention1 TO q6_mention3).\nCOMPUTE r2 = MAX(q6_mention1 TO q6_mention3).\n\
         COMPUTE r3 = MAX(q1_satisfaction, age).\nCOMPUTE r4 = MIN(age).",
        "COMPUTE r = SD(q6_mention1 TO q6_mention3).\n\
         COMPUTE r2 = VARIANCE(q6_mention1 TO q6_mention3).\n\
         COMPUTE r3 = SD.3(q6_mention1 TO q6_mention3).\nCOMPUTE r4 = VARIANCE(q5_1 TO q5_5).",
        "COMPUTE r = CFVAR(q5_1 TO q5_5).\nCOMPUTE r2 = CFVAR(q1_satisfaction, q2_recommend, age).\n\
         COMPUTE r3 = CFVAR.3(q1_satisfaction, q2_recommend, age).",
        "COMPUTE r = SUM.2(q6_mention1 TO q6_mention3).\nCOMPUTE r2 = MIN.2(q6_mention1 TO q6_mention3).\n\
         COMPUTE r3 = MAX.3(q6_mention1 TO q6_mention3).\nCOMPUTE r4 = SUM(age + 1, monthly_spend * 2).",
        "COMPUTE r = NVALID(q1_satisfaction, q2_recommend, monthly_spend).\n\
         COMPUTE r2 = NMISS(q6_mention1 TO q6_mention3).\nCOMPUTE r3 = NVALID(q5_1 TO weight).\n\
         COMPUTE r4 = NMISS(q1_satisfaction + q2_recommend, age).",
        "COMPUTE r = MISSING(q1_satisfaction).\nCOMPUTE r2 = SYSMIS(q1_satisfaction).\n\
         COMPUTE r3 = MISSING(monthly_spend).\nCOMPUTE r4 = SYSMIS(monthly_spend).\n\
         COMPUTE r5 = MISSING(q1_satisfaction + q2_recommend).\nCOMPUTE r6 = SYSMIS(q2_recommend + 1).",
        "COMPUTE r = VALUE(q1_satisfaction).\nCOMPUTE r2 = VALUE(q2_recommend) + 1.\n\
         COMPUTE r3 = MEAN(VALUE(q1_satisfaction), VALUE(q2_recommend)).\n\
         COMPUTE r4 = VALUE(monthly_spend).",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn any_and_range() {
    survey(&[
        "COMPUTE r = ANY(q1_satisfaction, 1, 2, 98).\nCOMPUTE r2 = ANY(region, 1, 3, 5).",
        "COMPUTE r = ANY(q6_mention1, q6_mention2, q6_mention3).\n\
         COMPUTE r2 = ANY(3, q6_mention1, q6_mention2, q6_mention3).\n\
         COMPUTE r3 = ANY(q2_recommend, q1_satisfaction, 7).",
        "COMPUTE r = RANGE(age, 18, 29, 60, 99).\nCOMPUTE r2 = RANGE(q2_recommend, 0, 6).\n\
         COMPUTE r3 = RANGE(age, q1_satisfaction * 10, q2_recommend * 10).",
        "COMPUTE r = RANGE(3, q6_mention1, q6_mention2).\nCOMPUTE r2 = RANGE(3, 4, 2).\n\
         COMPUTE r3 = RANGE(3, 4, 2, 5, 6).\nCOMPUTE r4 = RANGE(3, 4, 2, 1, 5).\n\
         COMPUTE r5 = RANGE(3, q6_mention1, q6_mention2, q6_mention2, q6_mention3).",
        "COMPUTE r = ANY(plan, 'BASIC', 'PLUS').\nCOMPUTE r2 = RANGE(plan, 'A', 'F').\n\
         COMPUTE r3 = ANY(plan, 'BASIC   ', 'premium').\nCOMPUTE r4 = RANGE(plan, 'F', 'A', 'P', 'Q').",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn string_functions() {
    survey(&[
        "STRING s (A30) / t (A12).\nCOMPUTE s = CONCAT(plan, '/', plan).\n\
         COMPUTE t = CONCAT(RTRIM(plan), '-', 'x').",
        "STRING s1 s2 s3 s4 s5 (A12).\nCOMPUTE s1 = SUBSTR(plan, 2).\nCOMPUTE s2 = SUBSTR(plan, 2, 3).\n\
         COMPUTE s3 = SUBSTR(plan, 0).\nCOMPUTE s4 = SUBSTR(plan, 11).\nCOMPUTE s5 = SUBSTR(plan, 9, 5).",
        "STRING s1 s2 (A40).\nCOMPUTE s1 = SUBSTR(plan, q5_1 + 1, q6_mention1).\n\
         COMPUTE s2 = SUBSTR(q3_comment, 5, 20).",
        "COMPUTE r = LENGTH(plan).\nCOMPUTE r2 = LENGTH(RTRIM(plan)).\nCOMPUTE r3 = LENGTH(q3_comment).\n\
         COMPUTE r4 = LENGTH(RTRIM(q3_comment)).\nCOMPUTE r5 = LENGTH(CONCAT(RTRIM(plan), 'x')).\n\
         COMPUTE r6 = LENGTH('').",
        "STRING s (A200) / t (A10).\nCOMPUTE s = UPCASE(q3_comment).\nCOMPUTE t = LOWER(plan).",
        "STRING s (A20) / t (A200) / u (A10).\nCOMPUTE s = LTRIM(STRING(age, F8.2)).\n\
         COMPUTE t = LTRIM(SUBSTR(q3_comment, 7)).\nCOMPUTE u = CONCAT(RTRIM(SUBSTR(plan, 1, 3)), '!').",
        "COMPUTE r = INDEX(q3_comment, 'the').\nCOMPUTE r2 = INDEX(plan, 'S').\n\
         COMPUTE r3 = INDEX(plan, plan).\nCOMPUTE r4 = INDEX(q3_comment, RTRIM(LOWER(plan))).\n\
         COMPUTE r5 = INDEX(q3_comment, plan).\nCOMPUTE r6 = INDEX(plan, '').",
        "STRING s (A3) / t (A4).\nCOMPUTE s = plan.\nCOMPUTE t = 'abcdefgh'.\nIF (age > 50) s = 'old'.",
    ]);
    check(
        "unicode.sav",
        "STRING s (A60).\nCOMPUTE s = CONCAT(RTRIM(tervitus), '!').\n\
         COMPUTE r = LENGTH(RTRIM(tervitus)).\nCOMPUTE r2 = LENGTH(tervitus).\n\
         COMPUTE r3 = tervitus = 'Hello'.",
    );
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn number_and_string_conversion() {
    survey(&[
        "STRING s1 s2 s3 s4 (A12).\nCOMPUTE s1 = STRING(monthly_spend, F8.2).\n\
         COMPUTE s2 = STRING(monthly_spend, F5.0).\nCOMPUTE s3 = STRING(monthly_spend, F4.2).\n\
         COMPUTE s4 = STRING(age - 50, F3.1).",
        "STRING s1 s2 s3 s4 (A12).\nCOMPUTE s1 = STRING(monthly_spend * 1000000, F8.2).\n\
         COMPUTE s2 = STRING(-monthly_spend * 1000000, F8.2).\n\
         COMPUTE s3 = STRING(monthly_spend * 1e9, F12.0).\nCOMPUTE s4 = STRING(-monthly_spend * 1e9, F6.1).",
        "STRING s1 s2 s3 s4 (A14).\nCOMPUTE s1 = STRING(monthly_spend / 3, F12.6).\n\
         COMPUTE s2 = STRING(q1_satisfaction, F2.0).\nCOMPUTE s3 = STRING(weight, F6.3).\n\
         COMPUTE s4 = STRING(monthly_spend / 1000, F6.3).",
        "STRING s1 s2 s3 s4 s5 (A14).\nCOMPUTE s1 = STRING(signup_date, F12.0).\n\
         COMPUTE s2 = STRING((age - 50) / 1000, F5.2).\nCOMPUTE s3 = STRING(monthly_spend / 8, F8.2).\n\
         COMPUTE s4 = STRING(weight / 4, F7.3).\nCOMPUTE s5 = STRING((age - 50) / 8, F4.2).",
        "STRING s1 s2 s3 s4 (A14).\nCOMPUTE s1 = STRING(0.015, F8.2).\nCOMPUTE s2 = STRING(99999.995, F8.2).\n\
         COMPUTE s3 = STRING(-0.04, F3.2).\nCOMPUTE s4 = STRING(monthly_spend, F1.0).",
        "COMPUTE r = NUMBER(STRING(monthly_spend, F8.2), F8.2).\n\
         COMPUTE r2 = NUMBER(STRING(age, F3.0), F3.1).\nCOMPUTE r3 = NUMBER('12345', F3.0).\n\
         COMPUTE r4 = NUMBER('1.5e3', F8.2).\nCOMPUTE r5 = NUMBER(' -12 ', F8.2).\n\
         COMPUTE r6 = NUMBER('+3.', F8.1).\nCOMPUTE r7 = NUMBER('1+2', F5).\n\
         COMPUTE r8 = NUMBER('  ', F5).\nCOMPUTE r9 = NUMBER('- 5', F8.2).",
        "SELECT IF resp_id <= 1006.\nCOMPUTE r = NUMBER(plan, F8.2).\nCOMPUTE r2 = NUMBER('12abc', F5).\n\
         COMPUTE r3 = NUMBER('1.2.3', F5).\nCOMPUTE r4 = NUMBER('1e', F5).\nCOMPUTE r5 = NUMBER('1 2', F5).",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn dates() {
    survey(&[
        "COMPUTE y = XDATE.YEAR(signup_date).\nCOMPUTE m = XDATE.MONTH(signup_date).\n\
         COMPUTE d = XDATE.MDAY(signup_date).\nCOMPUTE back = DATE.DMY(d, m, y).\n\
         COMPUTE same = back = signup_date.",
        "COMPUTE r = DATE.MDY(region, q6_mention1, 2000 + age).\n\
         COMPUTE r2 = DATE.DMY(q5_1 * 31, q2_recommend + 3, 1990 + age).\n\
         COMPUTE r3 = XDATE.MONTH(r2).\nCOMPUTE r4 = XDATE.MDAY(r2).\nCOMPUTE r5 = XDATE.YEAR(r2).",
        "COMPUTE r = XDATE.YEAR(signup_date + 86400 * 365.25 * age).\n\
         COMPUTE r2 = XDATE.MDAY(signup_date - 86399).\nCOMPUTE r3 = XDATE.YEAR(age).\n\
         COMPUTE r4 = XDATE.YEAR(86400).\nCOMPUTE r5 = DATE.DMY(15, 10, 1582).",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn compute_if_and_select() {
    survey(&[
        "IF (age >= 65) senior = 1.",
        "IF (q1_satisfaction > 3) happy = 1.\nIF (q1_satisfaction <= 3) happy = 0.",
        "IF (age > 50) age = age - 100.\nIF (q1_satisfaction = 3) q1_satisfaction = 98.\n\
         COMPUTE r = q1_satisfaction.",
        "IF (MISSING(monthly_spend)) monthly_spend = 0.\nIF q5_1 r = monthly_spend * 2.",
        "COMPUTE a = age * 2.\nCOMPUTE a = a + 1.\nCOMPUTE age = a.\nCOMPUTE q1_satisfaction = 99.",
        "SELECT IF age > 40.",
        "SELECT IF q1_satisfaction > 3.\nCOMPUTE r = q1_satisfaction * 2.",
        "SELECT IF NOT MISSING(monthly_spend) AND ANY(region, 1, 2).\nSELECT IF plan = 'PLUS'.",
        "NUMERIC n1 n2 (F5.1) / n3.\nCOMPUTE n1 = 1.25.\nIF (age > 50) n3 = n1 + age.",
        "STRING s1 s2 (A8) / s3 (A2).\nCOMPUTE s3 = 'abc'.\nIF (age > 50) s1 = plan.",
    ]);
}

// The constant expressions of the normal tests in `transform.rs`.
#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn constants() {
    survey(&[
        // 0 ** $SYSMIS, $SYSMIS ** 0 and 0 / 0 are left out: PSPP gives 0, 1 and 0, the manual missing
        "COMPUTE r1 = 2 + 3 * 4 - 6 / 4.\n\
         COMPUTE r4 = 0 * $SYSMIS.\nCOMPUTE r6 = 1 / 0.\nCOMPUTE r7 = 0 ** -1.\n\
         COMPUTE r8 = 3 >= 3 AND 2 <> 2.\nCOMPUTE r9 = 1 LT 2 OR $SYSMIS.\nCOMPUTE r10 = $SYSMIS = $SYSMIS.",
        "COMPUTE r1 = RND(2.5).\nCOMPUTE r2 = RND(-2.5).\nCOMPUTE r3 = RND(1.2345, 0.01).\n\
         COMPUTE r4 = RND(7, 5).\nCOMPUTE r5 = RND(0.49999999999999994).\nCOMPUTE r6 = RND(0.5 - 2 ** -40).\n\
         COMPUTE r7 = RND(0.5 - 2 ** -40, 1, 20).\nCOMPUTE r8 = TRUNC(57, 10).\nCOMPUTE r9 = MOD(-7, 3).\n\
         COMPUTE r10 = MOD(7, 0).\nCOMPUTE r11 = ABS($SYSMIS).\nCOMPUTE r12 = LG10(1000).",
        "COMPUTE r1 = SUM(1, $SYSMIS, 3).\nCOMPUTE r2 = SUM($SYSMIS).\nCOMPUTE r3 = SUM.2(1, $SYSMIS).\n\
         COMPUTE r4 = CFVAR(-1, 1).\nCOMPUTE r5 = RANGE(3, 1, 5).\nCOMPUTE r6 = RANGE(3, 4, 5, $SYSMIS, 9).\n\
         COMPUTE r7 = RANGE(3, $SYSMIS, 9).\nCOMPUTE r8 = RANGE(3, 1, 5, 4, 2).\n\
         COMPUTE r9 = ANY($SYSMIS, 1, 2).\nCOMPUTE r10 = ANY(2, 1, $SYSMIS).",
        "STRING s1 s2 s3 s4 s5 s6 s7 s8 (A12).\nCOMPUTE s1 = STRING(0.5, F5.0).\n\
         COMPUTE s2 = STRING(-0.5, F3.1).\nCOMPUTE s3 = STRING(0, F3.0).\nCOMPUTE s4 = STRING(-0.001, F8.2).\n\
         COMPUTE s5 = STRING(-1234567890123, F8.0).\nCOMPUTE s6 = STRING(9.99, F3.2).\n\
         COMPUTE s7 = STRING(0.285, F8.2).\nCOMPUTE s8 = STRING(12.449, F4.2).",
        "STRING s1 s2 s3 s4 s5 s6 (A12).\nCOMPUTE s1 = STRING(1.005, F8.2).\n\
         COMPUTE s2 = STRING(99999.995, F9.2).\nCOMPUTE s3 = STRING(2.1895, F6.3).\n\
         COMPUTE s4 = STRING(98.85, F4.2).\nCOMPUTE s5 = STRING(123456789, F8.2).\n\
         COMPUTE s6 = STRING(123456, F3.0).",
        "COMPUTE r1 = NUMBER('12345', F8.0).\nCOMPUTE r2 = NUMBER('12.5', F8.2).\nCOMPUTE r3 = NUMBER('.5', F8.0).\n\
         COMPUTE r4 = DATE.DMY(1, 11, 2023).\nCOMPUTE r5 = DATE.DMY(0, 1, 2000).\n\
         COMPUTE r6 = DATE.DMY(29, 2, 2023).\nCOMPUTE r7 = DATE.DMY(15, 13, 2000).\n\
         COMPUTE r8 = XDATE.MDAY(13918262399).\nCOMPUTE r9 = XDATE.YEAR(0).",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn more_numeric_and_statistical_functions() {
    survey(&[
        "COMPUTE r1 = LNGAMMA(age / 10).\nCOMPUTE r2 = LNGAMMA(q5_1).\nCOMPUTE r3 = SIN(age).\n\
         COMPUTE r4 = COS(monthly_spend).\nCOMPUTE r5 = ARSIN((q1_satisfaction - 3) / 2).\n\
         COMPUTE r6 = ARTAN(monthly_spend - 100).",
        "COMPUTE r1 = MEDIAN(q6_mention1 TO q6_mention3).\nCOMPUTE r2 = MEDIAN(q5_1 TO q5_5, age).\n\
         COMPUTE r3 = MEDIAN.2(q6_mention1 TO q6_mention3).\n\
         COMPUTE r4 = MEDIAN(q1_satisfaction, q2_recommend).",
        "STRING s1 s2 (A10).\nCOMPUTE s1 = MIN(plan, 'C').\nCOMPUTE s2 = MAX(plan, 'C', 'BASICS').",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn distribution_functions() {
    // GNU PSPP counts the failures of NEGBIN and has its own BVNOR and PDF.LAPLACE, which the
    // matrix comparison already notes, and has no HALFNRM, IGAUSS, SMOD, SRANGE or noncentral
    // chi-square, F and t; they are left out here. Its NPDF.BETA is not the derivative of its
    // NCDF.BETA (ours is, and its NCDF.BETA agrees with ours), so NPDF.BETA is left out too.
    survey(&[
        "COMPUTE r1 = CDF.NORMAL(age, 45, 12).\nCOMPUTE r2 = PDF.NORMAL(age, 45, 12).\n\
         COMPUTE r3 = IDF.NORMAL(weight / 3, 0, 1).\nCOMPUTE r4 = CDFNORM((age - 45) / 12).\n\
         COMPUTE r5 = PROBIT(weight / 3).\nCOMPUTE r6 = CDF.NORMAL(age, 45, q5_1 - 1).",
        "COMPUTE r1 = CDF.T(age / 10 - 4, q6_mention1).\nCOMPUTE r2 = IDF.T(weight / 3, region).\n\
         COMPUTE r3 = PDF.T(q2_recommend - 5, 3).\nCOMPUTE r4 = CDF.CHISQ(monthly_spend / 20, region).\n\
         COMPUTE r5 = SIG.CHISQ(monthly_spend / 20, region).\nCOMPUTE r6 = IDF.CHISQ(weight / 3, 4).\n\
         COMPUTE r7 = PDF.CHISQ(age / 10, 3).",
        "COMPUTE r1 = CDF.F(monthly_spend / 50, region, 20).\nCOMPUTE r2 = SIG.F(monthly_spend / 50, 3, age).\n\
         COMPUTE r3 = IDF.F(weight / 3, 3, 100).\nCOMPUTE r4 = PDF.F(monthly_spend / 50, 4, 12).\n\
         COMPUTE r5 = CDF.BETA(weight / 3, region, 2).\nCOMPUTE r6 = IDF.BETA(weight / 3, 2, 3).\n\
         COMPUTE r7 = PDF.BETA(weight / 3, 2, q6_mention1).",
        "COMPUTE r1 = CDF.GAMMA(monthly_spend / 50, 2, 1.5).\nCOMPUTE r2 = IDF.GAMMA(weight / 3, 3, 2).\n\
         COMPUTE r3 = PDF.GAMMA(age / 20, region, 1).\nCOMPUTE r4 = CDF.EXP(age / 30, 0.5).\n\
         COMPUTE r5 = IDF.EXP(weight / 3, 2).\nCOMPUTE r6 = PDF.EXP(age / 30, 2).",
        "COMPUTE r1 = CDF.UNIFORM(age, 0, 100).\nCOMPUTE r2 = IDF.UNIFORM(weight / 3, 1, 5).\n\
         COMPUTE r3 = PDF.UNIFORM(age, 0, 100).\nCOMPUTE r4 = CDF.CAUCHY(age, 45, 10).\n\
         COMPUTE r5 = IDF.CAUCHY(weight / 3, 0, 2).\nCOMPUTE r6 = PDF.CAUCHY(age, 45, 10).\n\
         COMPUTE r7 = CDF.LAPLACE(age, 45, 10).\nCOMPUTE r8 = IDF.LAPLACE(weight / 3, 0, 2).",
        "COMPUTE r1 = CDF.LOGISTIC(age, 45, 10).\nCOMPUTE r2 = IDF.LOGISTIC(weight / 3, 0, 2).\n\
         COMPUTE r3 = PDF.LOGISTIC(age, 45, 10).\nCOMPUTE r4 = CDF.LNORMAL(monthly_spend, 100, 0.5).\n\
         COMPUTE r5 = IDF.LNORMAL(weight / 3, 100, 0.5).\nCOMPUTE r6 = PDF.LNORMAL(monthly_spend, 100, 0.5).",
        "COMPUTE r1 = CDF.PARETO(monthly_spend, 50, 2).\nCOMPUTE r2 = IDF.PARETO(weight / 3, 50, 2).\n\
         COMPUTE r3 = PDF.PARETO(monthly_spend, 50, 2).\nCOMPUTE r4 = CDF.WEIBULL(monthly_spend, 100, 2).\n\
         COMPUTE r5 = IDF.WEIBULL(weight / 3, 100, 2).\nCOMPUTE r6 = PDF.WEIBULL(monthly_spend, 100, 2).",
        "COMPUTE r1 = CDF.BINOM(q6_mention1, 6, 0.3).\nCOMPUTE r2 = PDF.BINOM(q6_mention1, 6, 0.3).\n\
         COMPUTE r3 = CDF.POISSON(region, 2.5).\nCOMPUTE r4 = PDF.POISSON(region, 2.5).\n\
         COMPUTE r5 = CDF.GEOM(region, 0.3).\nCOMPUTE r6 = PDF.GEOM(region, 0.3).\n\
         COMPUTE r7 = CDF.BERNOULLI(q5_1, 0.3).\nCOMPUTE r8 = PDF.BERNOULLI(q5_1, 0.3).",
        "COMPUTE r1 = CDF.HYPER(q6_mention1, 20, 6, 8).\nCOMPUTE r2 = PDF.HYPER(q6_mention1, 20, 6, 8).\n\
         COMPUTE r3 = NCDF.BETA(weight / 3, 2, 3, 1).",
        "COMPUTE r1 = CDF.NORMAL(0, 0, 0).\nCOMPUTE r2 = IDF.NORMAL(1.5, 0, 1).\n\
         COMPUTE r3 = PROBIT(0).\nCOMPUTE r4 = CDF.BINOM(1, 2.5, 0.5).\nCOMPUTE r5 = IDF.CHISQ(-0.1, 2).\n\
         COMPUTE r6 = CDF.CHISQ(-1, 2).\nCOMPUTE r7 = PDF.EXP(-1, 2).\nCOMPUTE r8 = CDF.UNIFORM(81, 18, 80).\n\
         COMPUTE r9 = CDF.POISSON(1.5, 2).\nCOMPUTE r10 = CDF.GEOM(0, 0.5).",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn more_date_functions() {
    // GNU PSPP takes TIME.HMS with three arguments only and accepts some the manual calls
    // invalid; the parts of negative times differ on purpose
    survey(&[
        "COMPUTE r1 = DATE.MOYR(region, 1990 + age).\nCOMPUTE r2 = DATE.QYR(q5_1 + 1, 2000 + age).\n\
         COMPUTE r3 = DATE.WKYR(age, 2004).\nCOMPUTE r4 = DATE.YRDAY(2003, age * 5).\n\
         COMPUTE r5 = YRMODA(1900 + age, region * 2, q6_mention1 * 5).\nCOMPUTE r6 = YRMODA(age, 1, 1).",
        "COMPUTE r1 = TIME.HMS(age, q6_mention1 * 9, MOD(monthly_spend, 60)).\nCOMPUTE r2 = TIME.DAYS(monthly_spend / 7).\n\
         COMPUTE r3 = TIME.HMS(-age, -region, 0).\nCOMPUTE r4 = CTIME.DAYS(signup_date).\n\
         COMPUTE r5 = CTIME.HOURS(monthly_spend * 1000).\nCOMPUTE r6 = CTIME.MINUTES(monthly_spend * 100).\n\
         COMPUTE r7 = CTIME.SECONDS(age).",
        "COMPUTE t = signup_date + monthly_spend * 1000.\nCOMPUTE r1 = XDATE.DATE(t).\n\
         COMPUTE r2 = XDATE.TIME(t).\nCOMPUTE r3 = XDATE.HOUR(t).\nCOMPUTE r4 = XDATE.MINUTE(t).\n\
         COMPUTE r5 = XDATE.SECOND(t).\nCOMPUTE r6 = XDATE.TDAY(t).\nCOMPUTE r7 = XDATE.JDAY(t).\n\
         COMPUTE r8 = XDATE.WEEK(t).\nCOMPUTE r9 = XDATE.WKDAY(t).\nCOMPUTE r10 = XDATE.QUARTER(t).",
        "COMPUTE later = DATESUM(signup_date, age * 7 + monthly_spend, 'days').\n\
         COMPUTE r1 = DATEDIFF(later, signup_date, 'years').\nCOMPUTE r2 = DATEDIFF(later, signup_date, 'quarters').\n\
         COMPUTE r3 = DATEDIFF(later, signup_date, 'months').\nCOMPUTE r4 = DATEDIFF(signup_date, later, 'months').\n\
         COMPUTE r5 = DATEDIFF(later, signup_date, 'weeks').\nCOMPUTE r6 = DATEDIFF(later, signup_date, 'days').\n\
         COMPUTE r7 = DATEDIFF(later, signup_date, 'hours').\nCOMPUTE r8 = DATEDIFF(signup_date, later, 'minutes').\n\
         COMPUTE r9 = DATEDIFF(later, signup_date, 'seconds').",
        "COMPUTE r1 = DATESUM(signup_date, age, 'months').\nCOMPUTE r2 = DATESUM(signup_date, -age, 'months', 'rollover').\n\
         COMPUTE r3 = DATESUM(signup_date, region + 0.7, 'years', 'closest').\n\
         COMPUTE r4 = DATESUM(signup_date, -region, 'quarters', 'rollover').\n\
         COMPUTE r5 = DATESUM(signup_date, monthly_spend, 'hours').\n\
         COMPUTE r6 = DATESUM(signup_date, -age, 'weeks').\nCOMPUTE r7 = DATESUM(signup_date, age, 'minutes').\n\
         COMPUTE r8 = DATESUM(signup_date, monthly_spend, 'seconds').",
        "COMPUTE d = DATE.DMY(31, region * 2 - 1, 2000 + age).\n\
         COMPUTE r1 = DATESUM(d, q6_mention1, 'months', 'rollover').\nCOMPUTE r2 = DATESUM(d, q6_mention1, 'months').\n\
         COMPUTE r3 = DATESUM(d, -q6_mention2, 'months', 'rollover').\nCOMPUTE r4 = DATESUM(d, -q6_mention2, 'months').\n\
         COMPUTE r5 = DATEDIFF(d, signup_date, 'months').\nCOMPUTE r6 = DATEDIFF(signup_date, d, 'years').",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn more_string_functions() {
    // GNU PSPP has no CHAR. functions (the byte forms here count characters too), NTRIM or
    // NORMALIZE, and gives back the string for a missing padding length (the manual: the null
    // string); those are left out. A length with a fraction is missing in both.
    survey(&[
        "COMPUTE r1 = INDEX(q3_comment, 'aeiou', 1).\nCOMPUTE r2 = RINDEX(q3_comment, 'aeiou', 1).\n\
         COMPUTE r3 = RINDEX(q3_comment, 'the').\nCOMPUTE r4 = INDEX(plan, 'ASIC', 2).\n\
         COMPUTE r5 = RINDEX(q3_comment, 'e').\nCOMPUTE r6 = INDEX(plan, 'ABC', 2).\n\
         COMPUTE r7 = RINDEX(plan, ' ').\nCOMPUTE r8 = INDEX(plan, 'S', q5_1).",
        "STRING s1 s2 s3 s4 s5 s6 (A20).\nCOMPUTE s1 = LPAD(plan, 14).\nCOMPUTE s2 = RPAD(RTRIM(plan), 15, 'ab').\n\
         COMPUTE s3 = LPAD(RTRIM(plan), TRUNC(age / 4), '*').\nCOMPUTE s4 = RPAD(RTRIM(plan), 9, '-').\n\
         COMPUTE s5 = LPAD(plan, 3, 'x').\nCOMPUTE s6 = CONCAT(LPAD(RTRIM(plan), region + 5, 'xy'), '|').",
        "STRING s1 s2 s3 s4 (A20).\nCOMPUTE s1 = LTRIM(plan, 'B').\nCOMPUTE s2 = RTRIM(RTRIM(plan), 'S').\n\
         COMPUTE s3 = CONCAT(RTRIM(plan, ' '), '|').\nCOMPUTE s4 = LTRIM(LOWER(plan), 'p').",
        "STRING s1 s2 s3 s4 s5 (A200).\nCOMPUTE s1 = REPLACE(q3_comment, 'the', 'THE').\n\
         COMPUTE s2 = REPLACE(q3_comment, ' ', '_', 2).\nCOMPUTE s3 = REPLACE(plan, 'S', '').\n\
         COMPUTE s4 = REPLACE(q3_comment, 'e', 'ee', region).\nCOMPUTE s5 = REPLACE(plan, '', 'x').",
        "STRING s1 s2 s3 (A20).\nCOMPUTE s1 = STRUNC(q3_comment, 7).\nCOMPUTE s2 = STRUNC(plan, TRUNC(age / 10)).\n\
         COMPUTE s3 = CONCAT(STRUNC(plan, 3), '|').\nCOMPUTE r1 = MBLEN.BYTE(plan, 2).\n\
         COMPUTE r2 = MBLEN.BYTE(q3_comment, region).\nCOMPUTE r3 = MBLEN.BYTE(plan, 11).",
        "STRING s1 s2 (A40).\nCOMPUTE s1 = VALUELABEL(region).\nCOMPUTE s2 = VALUELABEL(q1_satisfaction).",
        "STRING s1 s2 s3 s4 s5 (A20).\nCOMPUTE s1 = STRING(signup_date, DATE11).\n\
         COMPUTE s2 = STRING(signup_date, ADATE10).\nCOMPUTE s3 = STRING(monthly_spend * 1000, COMMA12.2).\n\
         COMPUTE s4 = STRING(age, N5).\nCOMPUTE s5 = STRING(monthly_spend, DOLLAR10.2).\n\
         COMPUTE r1 = NUMBER(s1, DATE11).\nCOMPUTE r2 = NUMBER(s2, ADATE10).\nCOMPUTE r3 = NUMBER(s3, COMMA12.2).\n\
         COMPUTE r4 = NUMBER(s4, N5).\nCOMPUTE r5 = NUMBER(s5, DOLLAR10.2).",
    ]);
    // STRUNC drops a cut character as the manual says, where GNU PSPP keeps its first bytes;
    // GNU PSPP's MBLEN.BYTE is 1 for every character of these UTF-8 strings, and its INDEX
    // counts bytes where ours counts characters
    check(
        "unicode.sav",
        "STRING s1 s2 (A60).\nCOMPUTE s1 = REPLACE(tervitus, ',', ';').\n\
         COMPUTE s2 = REPLACE(tervitus, 'o', '0', 1).",
    );
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn do_if_blocks() {
    survey(&[
        "DO IF (age < 30).\nCOMPUTE g = 1.\nELSE.\nCOMPUTE g = 2.\nEND IF.",
        "DO IF (gender = 1).\nRECODE region (1 THRU 5=0).\nELSE.\nRECODE region (2=20) INTO r2.\nEND IF.",
        "DO IF (q1_satisfaction > 3).\nCOMPUTE g = 1.\nELSE IF (q2_recommend > 5).\nCOMPUTE g = 2.\n\
         ELSE IF (monthly_spend > 100).\nCOMPUTE g = 3.\nELSE.\nCOMPUTE g = 4.\nEND IF.",
        "DO IF (age < 30).\nCOMPUTE age = 99.\nELSE IF (age > 60).\nCOMPUTE g = 2.\nELSE.\n\
         COMPUTE g = 3.\nEND IF.",
        "DO IF (region = 1).\nCOMPUTE g = 10.\nDO IF (gender = 1).\nCOMPUTE h = 1.\n\
         ELSE IF (q1_satisfaction > 2).\nCOMPUTE h = 2.\nELSE.\nCOMPUTE h = 3.\nEND IF.\n\
         COMPUTE g = g + h.\nELSE.\nIF (gender = 2) g = 20.\nEND IF.",
        "STRING tier (A6).\nDO IF (plan = 'BASIC').\nCOMPUTE tier = 'low'.\n\
         ELSE IF (ANY(plan, 'PLUS', 'PREMIUM')).\nCOMPUTE tier = 'mid'.\nIF (age > 50) tier = 'midold'.\n\
         ELSE.\nCOMPUTE tier = 'high'.\nEND IF.",
        "DO IF (MISSING(q1_satisfaction)).\nCOMPUTE q1_satisfaction = 3.\nEND IF.\n\
         DO IF (q2_recommend > 8).\nEND IF.\nCOMPUTE r = q1_satisfaction + q2_recommend.",
    ]);
}
