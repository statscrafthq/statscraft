//! Expressions and the commands that use them. The expected values are worked out by hand from
//! the first twelve cases of `survey_basic.sav` (listed in `survey_basic_phpwriter.input.json`)
//! and agree with the reference program; the counts over all 200 cases come from it.

use statscraft_engine::Engine;
use statscraft_syntax::parse;

fn fixture(name: &str) -> String {
    format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn try_run(engine: &mut Engine, source: &str) -> Result<(), String> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    for parsed in &commands {
        engine.execute(&parsed.command).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn loaded(name: &str) -> Engine {
    let mut engine = Engine::default();
    try_run(&mut engine, &format!("GET FILE='{}'.", fixture(name))).unwrap();
    engine
}

/// The survey after running `source`.
fn survey(source: &str) -> Engine {
    let mut engine = loaded("survey_basic.sav");
    try_run(&mut engine, source).unwrap();
    engine
}

/// The error of the last command of `source`, run on the survey.
fn error(source: &str) -> String {
    try_run(&mut loaded("survey_basic.sav"), source).unwrap_err()
}

fn numbers(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let variable = engine.dataset().unwrap().variable(name).unwrap();
    variable.numeric().unwrap().iter().collect()
}

/// The value of a numeric variable in a case, counted from 1 as in the data listing.
fn number(engine: &Engine, name: &str, case: usize) -> Option<f64> {
    numbers(engine, name)[case - 1]
}

fn text(engine: &Engine, name: &str, case: usize) -> String {
    let variable = engine.dataset().unwrap().variable(name).unwrap();
    variable.string().unwrap().get(case - 1).to_string()
}

/// How many cases have this value.
fn count(engine: &Engine, name: &str, value: Option<f64>) -> usize {
    let values = numbers(engine, name);
    values.iter().filter(|v| **v == value).count()
}

fn close(actual: Option<f64>, expected: f64) {
    let actual = actual.expect("a value, not missing");
    assert!(
        (actual - expected).abs() <= 1e-12 * expected.abs(),
        "{actual} vs {expected}"
    );
}

/// One COMPUTE of a constant expression, read from the first case.
fn constant(expr: &str) -> Option<f64> {
    number(&survey(&format!("COMPUTE r = {expr}.")), "r", 1)
}

fn constant_text(expr: &str) -> String {
    let engine = survey(&format!("STRING s (A20).\nCOMPUTE s = {expr}."));
    text(&engine, "s", 1)
}

#[test]
fn a_user_missing_value_is_missing_in_an_expression() {
    let engine = survey(
        "COMPUTE r = q1_satisfaction + 1.
COMPUTE copy = q1_satisfaction.
COMPUTE raw = VALUE(q1_satisfaction) + 1.
COMPUTE neg = -q2_recommend.",
    );
    // case 8 has the user-missing code 99, case 3 has 99 in q2_recommend
    let first: Vec<Option<f64>> = numbers(&engine, "r")[..8].to_vec();
    let expected = [4.0, 5.0, 5.0, 5.0, 4.0, 6.0, 6.0].map(Some);
    assert_eq!(first[..7], expected);
    assert_eq!(first[7], None);
    assert_eq!(number(&engine, "copy", 8), None);
    assert_eq!(count(&engine, "copy", None), 21);
    assert_eq!(number(&engine, "raw", 8), Some(100.0));
    assert_eq!(number(&engine, "neg", 1), Some(-6.0));
    assert_eq!(number(&engine, "neg", 3), None);
    let data = engine.dataset().unwrap();
    assert_eq!(data.variable("r").unwrap().format(), "F8.2");
    // the source keeps its code
    assert_eq!(number(&engine, "q1_satisfaction", 8), Some(99.0));
}

#[test]
fn arithmetic_with_missing_and_zero() {
    let engine = survey(
        "COMPUTE product = q2_recommend * monthly_spend.
COMPUTE ratio = q5_1 / monthly_spend.
COMPUTE by_zero = age / q5_1.
COMPUTE zeros = q5_1 / q5_3.
COMPUTE power = monthly_spend ** 0.
COMPUTE sum = q2_recommend + monthly_spend.
COMPUTE root = SQRT(age - 50).
COMPUTE log = LN(q2_recommend).",
    );
    // case 12: q2_recommend 0, monthly_spend system-missing, q5_1 0
    assert_eq!(number(&engine, "product", 12), Some(0.0));
    assert_eq!(number(&engine, "product", 3), None);
    assert_eq!(number(&engine, "ratio", 12), Some(0.0));
    assert_eq!(number(&engine, "sum", 12), None);
    // a power is not among the operations that survive a missing operand
    assert_eq!(number(&engine, "power", 12), None);
    assert_eq!(number(&engine, "by_zero", 1), Some(49.0));
    assert_eq!(number(&engine, "by_zero", 2), None);
    assert_eq!(number(&engine, "zeros", 1), None);
    assert_eq!(number(&engine, "zeros", 2), Some(0.0));
    // case 7 is 0 / 0
    assert_eq!(number(&engine, "zeros", 7), None);
    assert_eq!(number(&engine, "root", 1), None);
    assert_eq!(number(&engine, "root", 2), Some(3.0));
    assert_eq!(number(&engine, "root", 8), Some(0.0));
    assert_eq!(number(&engine, "log", 12), None);

    assert_eq!(constant("2 + 3 * 4 - 6 / 4"), Some(12.5));
    assert_eq!(constant("-2 ** 2"), Some(-4.0));
    assert_eq!(constant("2 ** 3 ** 2"), Some(64.0));
    assert_eq!(constant("0 ** 0"), None);
    assert_eq!(constant("0 ** -1"), None);
    assert_eq!(constant("(-8) ** 0.5"), None);
    assert_eq!(constant("0 ** $SYSMIS"), None);
    assert_eq!(constant("$SYSMIS ** 0"), None);
    assert_eq!(constant("5 ** 0"), Some(1.0));
    assert_eq!(constant("0 * $SYSMIS"), Some(0.0));
    assert_eq!(constant("0 / $SYSMIS"), Some(0.0));
    // a divisor of 0 is a domain error, also when the dividend is 0
    assert_eq!(constant("0 / 0"), None);
    assert_eq!(constant("1 / 0"), None);
    assert_eq!(constant("10 ** 200 * 10 ** 200"), None);
}

#[test]
fn comparisons_and_three_valued_logic() {
    let engine = survey(
        "COMPUTE high = q1_satisfaction > 3.
COMPUTE and_false = q1_satisfaction > 3 AND q5_1 = 1.
COMPUTE and_true = q1_satisfaction > 3 AND q5_5 = 1.
COMPUTE or_true = q1_satisfaction > 3 OR q5_5 = 1.
COMPUTE or_false = q1_satisfaction > 3 OR q5_1 = 1.
COMPUTE inverse = NOT (q1_satisfaction > 3).
COMPUTE both = q1_satisfaction > 3 AND q2_recommend > 5.
COMPUTE either = q1_satisfaction > 3 | q2_recommend > 5.",
    );
    assert_eq!(numbers(&engine, "high")[..3], [0.0, 1.0, 1.0].map(Some));
    assert_eq!(
        [
            count(&engine, "high", Some(0.0)),
            count(&engine, "high", Some(1.0)),
            count(&engine, "high", None)
        ],
        [76, 103, 21]
    );
    // case 8: q1_satisfaction is missing, q5_1 is 0, q5_5 is 1
    assert_eq!(number(&engine, "and_false", 8), Some(0.0));
    assert_eq!(number(&engine, "and_true", 8), None);
    assert_eq!(number(&engine, "or_true", 8), Some(1.0));
    assert_eq!(number(&engine, "or_false", 8), None);
    assert_eq!(numbers(&engine, "inverse")[..2], [1.0, 0.0].map(Some));
    assert_eq!(number(&engine, "inverse", 8), None);
    assert_eq!(
        [
            count(&engine, "both", Some(0.0)),
            count(&engine, "both", Some(1.0)),
            count(&engine, "both", None)
        ],
        [97, 74, 29]
    );
    assert_eq!(
        [
            count(&engine, "either", Some(0.0)),
            count(&engine, "either", Some(1.0)),
            count(&engine, "either", None)
        ],
        [10, 174, 16]
    );

    assert_eq!(constant("3 >= 3 AND 2 <> 2"), Some(0.0));
    assert_eq!(constant("1 LT 2 OR $SYSMIS"), Some(1.0));
    assert_eq!(constant("$SYSMIS = $SYSMIS"), None);
    let message = error("COMPUTE r = age AND 1.");
    assert!(
        message.contains("the left operand of AND must be 0 or 1")
            && message.contains("49 in case 1"),
        "{message}"
    );
}

#[test]
fn arithmetic_functions() {
    assert_eq!(constant("RND(2.5)"), Some(3.0));
    assert_eq!(constant("RND(-2.5)"), Some(-3.0));
    assert_eq!(constant("RND(2.4)"), Some(2.0));
    close(constant("RND(1.2345, 0.01)"), 1.23);
    assert_eq!(constant("RND(7, 5)"), Some(5.0));
    assert_eq!(constant("RND(7, 0)"), None);
    // a value that earlier arithmetic left a hair below the half still rounds up
    assert_eq!(constant("RND(0.49999999999999994)"), Some(1.0));
    assert_eq!(constant("RND(0.4999)"), Some(0.0));
    assert_eq!(constant("RND(0.5 - 2 ** -40)"), Some(0.0));
    assert_eq!(constant("RND(0.5 - 2 ** -40, 1, 20)"), Some(1.0));
    // the sum is 1.5 less 2^-49: 0 and 3 fuzz bits round down, 4 round up
    let sum = "9.62 - 5.82 - 9.21 + 6.91";
    assert_eq!(constant(&format!("RND({sum}, 1, 0)")), Some(1.0));
    assert_eq!(constant(&format!("RND({sum}, 1, 3)")), Some(1.0));
    assert_eq!(constant(&format!("RND({sum}, 1, 4)")), Some(2.0));
    assert_eq!(constant(&format!("RND({sum})")), Some(2.0));
    // in steps of 0.1 it falls 1.8e-14 short of 15 steps, which 4 fuzz bits cover (15 * 2^-49)
    close(constant(&format!("TRUNC({sum}, 0.1, 0)")), 1.4);
    close(constant(&format!("TRUNC({sum}, 0.1, 3)")), 1.4);
    close(constant(&format!("TRUNC({sum}, 0.1, 4)")), 1.5);
    close(constant(&format!("TRUNC({sum}, 0.1)")), 1.5);
    // the fuzz is relative: a large value a few units short of a half still rounds up
    assert_eq!(constant("RND(1000000.5 - 2 ** -32)"), Some(1_000_001.0));
    assert_eq!(constant("TRUNC(-1.99)"), Some(-1.0));
    assert_eq!(constant("TRUNC(1.99)"), Some(1.0));
    assert_eq!(constant("TRUNC(57, 10)"), Some(50.0));
    assert_eq!(constant("MOD(7, 3)"), Some(1.0));
    assert_eq!(constant("MOD(-7, 3)"), Some(-1.0));
    assert_eq!(constant("MOD(7, 0)"), None);
    assert_eq!(constant("MOD(0, $SYSMIS)"), Some(0.0));
    assert_eq!(constant("ABS(-3.5)"), Some(3.5));
    assert_eq!(constant("SQRT(16)"), Some(4.0));
    assert_eq!(constant("SQRT(-1)"), None);
    assert_eq!(constant("EXP(0)"), Some(1.0));
    assert_eq!(constant("LN(0)"), None);
    assert_eq!(constant("LG10(1000)"), Some(3.0));
    assert_eq!(constant("LG10(-1)"), None);
    assert_eq!(constant("ABS($SYSMIS)"), None);

    let engine =
        survey("COMPUTE cents = TRUNC(monthly_spend * 100).\nCOMPUTE whole = RND(monthly_spend).");
    // 78.28 * 100 is 7828.000000000001 and 113.80 * 100 is 11379.999999999998
    assert_eq!(number(&engine, "cents", 1), Some(7828.0));
    assert_eq!(number(&engine, "cents", 5), Some(11380.0));
    assert_eq!(number(&engine, "whole", 1), Some(78.0));
    assert_eq!(number(&engine, "whole", 12), None);
}

#[test]
fn statistical_functions_skip_missing_arguments() {
    let engine = survey(
        "COMPUTE channels = SUM(q5_1 TO q5_5).
COMPUTE mean = MEAN(q1_satisfaction, q2_recommend, monthly_spend).
COMPUTE mean2 = MEAN.2(q1_satisfaction, q2_recommend, monthly_spend).
COMPUTE mean3 = MEAN.3(q1_satisfaction, q2_recommend, monthly_spend).
COMPUTE low = MIN(q6_mention1 TO q6_mention3).
COMPUTE top = MAX(q6_mention1, q6_mention2, q6_mention3).
COMPUTE sd = SD(q6_mention1 TO q6_mention3).
COMPUTE var = VARIANCE(q6_mention1 TO q6_mention3).
COMPUTE sd3 = SD.3(q6_mention1 TO q6_mention3).
COMPUTE cv = CFVAR(q6_mention1 TO q6_mention3).
COMPUTE valid = NVALID(q1_satisfaction, q2_recommend, monthly_spend).
COMPUTE gaps = NMISS(q6_mention1 TO q6_mention3).",
    );
    assert_eq!(numbers(&engine, "channels")[..3], [4.0, 2.0, 3.0].map(Some));
    let total: f64 = numbers(&engine, "channels").iter().flatten().sum();
    assert_eq!(total, 482.0);
    close(number(&engine, "mean", 1), (3.0 + 6.0 + 78.28) / 3.0);
    // case 3: q2_recommend is user-missing
    close(number(&engine, "mean", 3), (4.0 + 278.21) / 2.0);
    close(number(&engine, "mean2", 3), (4.0 + 278.21) / 2.0);
    assert_eq!(number(&engine, "mean3", 3), None);
    assert_eq!(count(&engine, "mean2", None), 4);
    // case 8 mentions 1, 6, 2; case 1 mentions 5, 3; case 3 only 1; case 11 nothing
    assert_eq!(number(&engine, "low", 8), Some(1.0));
    assert_eq!(number(&engine, "top", 8), Some(6.0));
    assert_eq!(number(&engine, "low", 11), None);
    close(number(&engine, "sd", 8), 7f64.sqrt());
    close(number(&engine, "var", 8), 7.0);
    close(number(&engine, "sd", 1), 2f64.sqrt());
    assert_eq!(number(&engine, "sd", 3), None);
    assert_eq!(number(&engine, "sd3", 1), None);
    close(number(&engine, "sd3", 8), 7f64.sqrt());
    close(number(&engine, "cv", 8), 7f64.sqrt() / 3.0);
    assert_eq!(numbers(&engine, "valid")[..3], [3.0, 3.0, 2.0].map(Some));
    assert_eq!(number(&engine, "gaps", 11), Some(3.0));
    assert_eq!(number(&engine, "gaps", 8), Some(0.0));

    assert_eq!(constant("SUM(1, $SYSMIS, 3)"), Some(4.0));
    assert_eq!(constant("SUM($SYSMIS)"), None);
    assert_eq!(constant("SUM.2(1, $SYSMIS)"), None);
    assert_eq!(constant("CFVAR(-1, 1)"), None);
}

#[test]
fn missing_value_functions() {
    let engine = survey(
        "COMPUTE any_missing = MISSING(q1_satisfaction).
COMPUTE system = SYSMIS(q1_satisfaction).
COMPUTE no_spend = SYSMIS(monthly_spend).
COMPUTE sum_missing = MISSING(q1_satisfaction + q2_recommend).
COMPUTE raw = VALUE(q2_recommend).",
    );
    // case 8 has the user-missing code 99, case 12 has no monthly_spend
    assert_eq!(number(&engine, "any_missing", 8), Some(1.0));
    assert_eq!(number(&engine, "any_missing", 1), Some(0.0));
    assert_eq!(count(&engine, "any_missing", Some(1.0)), 21);
    assert_eq!(number(&engine, "system", 8), Some(0.0));
    assert_eq!(number(&engine, "no_spend", 12), Some(1.0));
    assert_eq!(number(&engine, "no_spend", 1), Some(0.0));
    assert_eq!(number(&engine, "sum_missing", 3), Some(1.0));
    assert_eq!(number(&engine, "raw", 3), Some(99.0));

    // MISSING also takes a string variable and sees its user-missing values, which every
    // other part of an expression treats as valid
    let first = text(&survey(""), "plan", 1);
    let first = first.trim_end();
    let engine = survey(&format!(
        "MISSING VALUES plan ('{first}').
COMPUTE missing = MISSING(plan).
COMPUTE same = plan = '{first}'.
COMPUTE literal = MISSING('{first}')."
    ));
    assert_eq!(number(&engine, "missing", 1), Some(1.0));
    assert_eq!(number(&engine, "same", 1), Some(1.0));
    assert_eq!(number(&engine, "literal", 1), Some(0.0));
    assert_eq!(numbers(&engine, "missing"), numbers(&engine, "same"));
}

#[test]
fn any_and_range() {
    let engine = survey(
        "COMPUTE odd_region = ANY(region, 1, 3, 5).
COMPUTE unhappy = ANY(q1_satisfaction, 1, 2, 98).
COMPUTE third = ANY(3, q6_mention1, q6_mention2, q6_mention3).
COMPUTE young_or_old = RANGE(age, 18, 29, 60, 99).
COMPUTE basic = ANY(plan, 'BASIC', 'PLUS').
COMPUTE early = RANGE(plan, 'A', 'F').",
    );
    // regions 2, 4, 4, 3; plans BASIC, PREMIUM
    assert_eq!(
        numbers(&engine, "odd_region")[..4],
        [0.0, 0.0, 0.0, 1.0].map(Some)
    );
    assert_eq!(number(&engine, "unhappy", 8), None);
    assert_eq!(number(&engine, "unhappy", 12), Some(1.0));
    // mentions 5, 3 in case 1; 4, 1 in case 2; none in case 11
    assert_eq!(number(&engine, "third", 1), Some(1.0));
    assert_eq!(number(&engine, "third", 2), Some(0.0));
    assert_eq!(number(&engine, "third", 11), None);
    // ages 49, 59, 75 and 25 in case 7
    assert_eq!(
        numbers(&engine, "young_or_old")[..3],
        [0.0, 0.0, 1.0].map(Some)
    );
    assert_eq!(number(&engine, "young_or_old", 7), Some(1.0));
    assert_eq!(numbers(&engine, "basic")[..2], [1.0, 0.0].map(Some));
    assert_eq!(numbers(&engine, "early")[..2], [1.0, 0.0].map(Some));

    assert_eq!(constant("RANGE(3, 1, 5)"), Some(1.0));
    assert_eq!(constant("RANGE(3, 4, 5, $SYSMIS, 9)"), Some(0.0));
    assert_eq!(constant("RANGE(3, $SYSMIS, 9)"), None);
    // a range that runs backwards makes the result missing
    assert_eq!(constant("RANGE(3, 1, 5, 4, 2)"), None);
    assert_eq!(constant("ANY($SYSMIS, 1, 2)"), None);
    assert_eq!(constant("ANY(2, 1, $SYSMIS)"), Some(0.0));
}

#[test]
fn string_functions() {
    let engine = survey(
        "STRING joined (A30) / part rest none lower trimmed short (A12).
COMPUTE joined = CONCAT(plan, '/', RTRIM(plan), '!').
COMPUTE part = SUBSTR(plan, 2, 3).
COMPUTE rest = SUBSTR(plan, 4).
COMPUTE none = SUBSTR(plan, 0).
COMPUTE lower = LOWER(plan).
COMPUTE trimmed = LTRIM(STRING(age, F8.2)).
COMPUTE short = UPCASE(SUBSTR(q3_comment, 1, 6)).
COMPUTE width = LENGTH(plan).
COMPUTE used = LENGTH(RTRIM(plan)).
COMPUTE at = INDEX(plan, 'S').
COMPUTE at2 = INDEX(plan, 'SIC').
COMPUTE is_basic = plan = 'BASIC'.
COMPUTE before = plan < 'PLUS'.",
    );
    // plans: BASIC, PREMIUM, ..., ENTERPRISE in case 6, PLUS in case 7
    assert_eq!(text(&engine, "joined", 1), "BASIC     /BASIC!");
    assert_eq!(text(&engine, "part", 1), "ASI");
    assert_eq!(text(&engine, "rest", 2), "MIUM");
    assert_eq!(text(&engine, "none", 1), "");
    assert_eq!(text(&engine, "lower", 6), "enterprise");
    assert_eq!(text(&engine, "trimmed", 1), "49.00");
    assert_eq!(text(&engine, "short", 1), "PLEASE");
    assert_eq!(number(&engine, "width", 1), Some(10.0));
    assert_eq!(number(&engine, "used", 1), Some(5.0));
    assert_eq!(number(&engine, "used", 6), Some(10.0));
    assert_eq!(number(&engine, "at", 1), Some(3.0));
    assert_eq!(number(&engine, "at", 2), Some(0.0));
    assert_eq!(number(&engine, "at", 7), Some(4.0));
    assert_eq!(number(&engine, "at2", 1), Some(3.0));
    assert_eq!(numbers(&engine, "is_basic")[..2], [1.0, 0.0].map(Some));
    assert_eq!(numbers(&engine, "before")[..2], [1.0, 0.0].map(Some));

    assert_eq!(constant("'a' < 'B'"), Some(0.0));
    assert_eq!(constant("'ab' = 'ab   '"), Some(1.0));
    assert_eq!(constant("LENGTH('')"), Some(0.0));
    assert_eq!(constant_text("CONCAT('[', RTRIM('ab  '), ']')"), "[ab]");
    assert_eq!(constant_text("CONCAT('[', LTRIM('  ab'), ']')"), "[ab]");
    // a value longer than its variable is cut
    assert_eq!(
        constant_text("'abcdefghijklmnopqrstuvwxyz'"),
        "abcdefghijklmnopqrst"
    );
}

#[test]
fn strings_count_bytes_for_width_and_characters_for_positions() {
    let mut engine = loaded("unicode.sav");
    try_run(
        &mut engine,
        "STRING first (A12).
COMPUTE bytes = LENGTH(RTRIM(tervitus)).
COMPUTE characters = CHAR.LENGTH(RTRIM(tervitus)).
COMPUTE width = LENGTH(tervitus).
COMPUTE first = SUBSTR(tervitus, 1, 2).
COMPUTE comma = INDEX(tervitus, ',').",
    )
    .unwrap();
    // case 2 is five CJK characters of three bytes each; case 1 is Cyrillic with a comma
    assert_eq!(number(&engine, "bytes", 2), Some(15.0));
    assert_eq!(number(&engine, "characters", 2), Some(5.0));
    assert_eq!(number(&engine, "width", 2), Some(44.0));
    assert_eq!(text(&engine, "first", 2).chars().count(), 2);
    assert_eq!(text(&engine, "first", 2).len(), 6);
    assert_eq!(number(&engine, "comma", 1), Some(12.0));
}

#[test]
fn number_to_string_in_f_format() {
    let engine = survey("STRING spend (A8).\nCOMPUTE spend = STRING(monthly_spend, F8.2).");
    assert_eq!(text(&engine, "spend", 1), "   78.28");
    assert_eq!(text(&engine, "spend", 12), "     .");

    for (expr, expected) in [
        ("STRING(0.5, F5.0)", "    1"),
        ("STRING(-0.5, F3.1)", "-.5"),
        ("STRING(0, F8.2)", "     .00"),
        ("STRING(0, F3.0)", "  0"),
        ("STRING(-0.001, F8.2)", "     .00"),
        // too wide for two decimals: fewer decimals, then scientific notation, then asterisks
        ("STRING(1234567.891, F8.2)", " 1234568"),
        ("STRING(123456789, F8.2)", "1.2E+008"),
        ("STRING(-1234567890123, F8.0)", " -1E+012"),
        ("STRING(123456, F3.0)", "***"),
        ("STRING(9.99, F3.2)", " 10"),
        // values stored a little below a half: the reference program rounds some of them up
        ("STRING(0.285, F8.2)", "     .29"),
        ("STRING(1.005, F8.2)", "    1.00"),
        ("STRING(99999.995, F9.2)", "100000.00"),
        ("STRING(2.1895, F6.3)", " 2.189"),
        ("STRING(98.85, F4.2)", "98.9"),
        ("STRING(12.449, F4.2)", "12.4"),
    ] {
        assert_eq!(constant_text(expr), expected, "{expr}");
    }
    let message = error("COMPUTE r = LENGTH(STRING(age, COMMA8.2)).");
    assert!(message.contains("F format such as F8.2"), "{message}");
}

#[test]
fn string_to_number_in_f_format() {
    for (expr, expected) in [
        ("NUMBER('12345', F8.0)", Some(12345.0)),
        // only the width is read; without a decimal point the last digits are decimals
        ("NUMBER('12345', F3.0)", Some(123.0)),
        ("NUMBER('12345', F8.2)", Some(123.45)),
        ("NUMBER('12.5', F8.2)", Some(12.5)),
        ("NUMBER('1.5e3', F8.2)", Some(1500.0)),
        ("NUMBER('1+2', F5)", Some(100.0)),
        ("NUMBER(' -12 ', F8.0)", Some(-12.0)),
        ("NUMBER('- 5', F8.0)", Some(-5.0)),
        ("NUMBER('.5', F8.0)", Some(0.5)),
        ("NUMBER('  ', F5)", None),
        ("NUMBER('abc', F8.2)", None),
        ("NUMBER('12abc', F8.0)", None),
        ("NUMBER('1.2.3', F8.0)", None),
        ("NUMBER('1 2', F8.0)", None),
    ] {
        assert_eq!(constant(expr), expected, "{expr}");
    }
    let engine = survey("COMPUTE back = NUMBER(STRING(monthly_spend, F8.2), F8.2).");
    assert_eq!(number(&engine, "back", 1), Some(78.28));
    assert_eq!(number(&engine, "back", 12), None);
}

#[test]
fn dates_are_seconds_since_1582() {
    let engine = survey(
        "COMPUTE y = XDATE.YEAR(signup_date).
COMPUTE m = XDATE.MONTH(signup_date).
COMPUTE d = XDATE.MDAY(signup_date).
COMPUTE same = DATE.DMY(d, m, y) = signup_date.",
    );
    // signup dates: 2023-11-01, 2021-02-27
    assert_eq!(
        [
            number(&engine, "y", 1),
            number(&engine, "m", 1),
            number(&engine, "d", 1)
        ],
        [2023.0, 11.0, 1.0].map(Some)
    );
    assert_eq!(
        [
            number(&engine, "y", 2),
            number(&engine, "m", 2),
            number(&engine, "d", 2)
        ],
        [2021.0, 2.0, 27.0].map(Some)
    );
    assert_eq!(count(&engine, "same", Some(1.0)), 200);

    assert_eq!(constant("DATE.DMY(1, 11, 2023)"), Some(13_918_176_000.0));
    assert_eq!(constant("DATE.MDY(11, 1, 2023)"), Some(13_918_176_000.0));
    assert_eq!(constant("DATE.DMY(15, 10, 1582)"), Some(86400.0));
    assert_eq!(constant("DATE.DMY(14, 10, 1582)"), None);
    // day 0 is the last day of the month before, 29 February 2023 is 1 March
    assert_eq!(constant("DATE.DMY(0, 1, 2000)"), Some(13_165_977_600.0));
    assert_eq!(constant("DATE.DMY(29, 2, 2023)"), Some(13_897_008_000.0));
    assert_eq!(constant("DATE.DMY(15, 13, 2000)"), Some(13_198_896_000.0));
    assert_eq!(constant("DATE.DMY(32, 1, 2000)"), None);
    assert_eq!(constant("DATE.DMY(1, 14, 2000)"), None);
    assert_eq!(constant("DATE.DMY(1, 0, 2000)"), None);
    assert_eq!(constant("DATE.MDY(0, 1, 2000)"), None);
    assert_eq!(constant("DATE.DMY(1.5, 1, 2000)"), None);
    assert_eq!(constant("XDATE.YEAR(86400)"), Some(1582.0));
    assert_eq!(constant("XDATE.YEAR(0)"), None);
    assert_eq!(constant("XDATE.MDAY(13918262399)"), Some(1.0));
}

#[test]
fn if_assigns_only_where_the_condition_is_true() {
    let engine = survey(
        "IF (age >= 65) senior = 1.
IF (age > 50) age = 0.
IF (q1_satisfaction > 3) happy = 1.
IF (q1_satisfaction <= 3) happy = 0.
STRING note (A8).
IF (gender = 1) note = 'male'.",
    );
    // ages 49, 59, 75; case 8 has no valid q1_satisfaction
    assert_eq!(numbers(&engine, "senior")[..3], [None, None, Some(1.0)]);
    assert_eq!(numbers(&engine, "age")[..3], [49.0, 0.0, 0.0].map(Some));
    assert_eq!(numbers(&engine, "happy")[..2], [0.0, 1.0].map(Some));
    assert_eq!(number(&engine, "happy", 8), None);
    assert_eq!(count(&engine, "happy", None), 21);
    assert_eq!(text(&engine, "note", 1), "male");
    assert_eq!(text(&engine, "note", 3), "");
}

#[test]
fn select_if_keeps_the_cases_where_the_condition_is_true() {
    let engine = survey("SELECT IF q1_satisfaction > 3.");
    assert_eq!(engine.dataset().unwrap().case_count(), 103);
    assert_eq!(number(&engine, "resp_id", 1), Some(1002.0));
    let engine = survey("SELECT IF age > 40.\nSELECT IF resp_id <= 1003.");
    assert_eq!(numbers(&engine, "age"), [49.0, 59.0, 75.0].map(Some));
}

#[test]
fn string_and_numeric_declare_variables() {
    let engine = survey("STRING code1 code2 (A8) / memo (A40).\nNUMERIC score (F5.1) / flag.");
    let data = engine.dataset().unwrap();
    assert_eq!(data.variable("code2").unwrap().format(), "A8");
    assert_eq!(data.variable("memo").unwrap().string_width(), Some(40));
    assert_eq!(text(&engine, "memo", 1), "");
    assert_eq!(data.variable("score").unwrap().format(), "F5.1");
    assert_eq!(data.variable("flag").unwrap().format(), "F8.2");
    assert_eq!(count(&engine, "score", None), 200);

    let mut engine = loaded("survey_basic.sav");
    let message = try_run(&mut engine, "NUMERIC fresh age.").unwrap_err();
    assert_eq!(message, "variable age already exists");
    assert!(engine.dataset().unwrap().variable("fresh").is_err());
    assert_eq!(error("STRING plan (A5)."), "variable plan already exists");
}

#[test]
fn do_if_runs_the_first_true_branch_and_stops_at_a_missing_condition() {
    let engine = survey(
        "DO IF (q1_satisfaction > 3).
COMPUTE g = 1.
ELSE IF (q2_recommend > 5).
COMPUTE g = 2.
ELSE IF (monthly_spend > 100).
COMPUTE g = 3.
ELSE.
COMPUTE g = 4.
END IF.",
    );
    // case 5: q1 is 3 and q2 is user-missing; case 8: q1 is user-missing;
    // case 12: q1 is 2, q2 is 0 and monthly_spend is missing
    let first: Vec<Option<f64>> = numbers(&engine, "g")[..5].to_vec();
    assert_eq!(first, [Some(2.0), Some(1.0), Some(1.0), Some(1.0), None]);
    assert_eq!(number(&engine, "g", 8), None);
    assert_eq!(number(&engine, "g", 12), None);
    let counts = [Some(1.0), Some(2.0), Some(3.0), Some(4.0), None].map(|v| count(&engine, "g", v));
    assert_eq!(counts, [103, 55, 4, 4, 34]);
}

#[test]
fn do_if_blocks_nest_and_keep_their_cases() {
    let mut engine = loaded("survey_basic.sav");
    assert!(engine.active_cases().is_none());
    try_run(&mut engine, "DO IF (age < 30).").unwrap();
    // ages 49 and, in case 7, 25
    let active = engine.active_cases().unwrap();
    assert_eq!((active.len(), active[0], active[6]), (200, false, true));
    try_run(
        &mut engine,
        "COMPUTE age = 99.
ELSE IF (age > 60).
COMPUTE g = 2.
DO IF (gender = 2).
COMPUTE h = 1.
ELSE.
IF (region = 4) h = 2.
END IF.
ELSE.
COMPUTE g = 3.
END IF.",
    )
    .unwrap();
    assert!(engine.active_cases().is_none());
    engine.finish().unwrap();
    // case 7 was 25: it took the first branch, and its new age does not take it into the second
    assert_eq!(number(&engine, "age", 7), Some(99.0));
    assert_eq!(number(&engine, "g", 7), None);
    // case 1: 49; case 3: 75, female; case 6: 83, female; case 12: 84
    assert_eq!(number(&engine, "g", 1), Some(3.0));
    assert_eq!(number(&engine, "g", 3), Some(2.0));
    assert_eq!(number(&engine, "h", 3), Some(1.0));
    assert_eq!(number(&engine, "h", 1), None);
}

#[test]
fn do_if_errors() {
    assert_eq!(error("ELSE."), "ELSE without DO IF");
    assert_eq!(error("ELSE IF (age > 1)."), "ELSE IF without DO IF");
    assert_eq!(error("END IF."), "END IF without DO IF");
    let message = error("DO IF (age > 1).\nELSE.\nELSE IF (age > 2).");
    assert!(message.starts_with("ELSE IF after ELSE"), "{message}");
    let message = error("DO IF (age > 1).\nSELECT IF age > 2.");
    assert!(message.contains("inside DO IF or LOOP"), "{message}");

    let mut engine = loaded("survey_basic.sav");
    try_run(&mut engine, "DO IF (age > 1).\nCOMPUTE x = 1.").unwrap();
    let message = engine.finish().unwrap_err().to_string();
    assert!(message.starts_with("DO IF without END IF"), "{message}");
    // a failed DO IF opens no block
    let mut engine = loaded("survey_basic.sav");
    try_run(&mut engine, "DO IF (nothing > 1).").unwrap_err();
    engine.finish().unwrap();
}

#[test]
fn a_value_error_is_reported_only_for_the_cases_that_use_it() {
    // region is 1 to 5, so it is no valid operand of AND except where it is 1
    let message = error("IF (region AND q5_1) x = 1.");
    assert!(message.contains("must be 0 or 1"), "{message}");
    survey("DO IF (region = 1).\nIF (region AND q5_1) x = 1.\nEND IF.");
    survey("IF (region = 1) x = region AND q5_1.");
    let message = error("IF (age) x = 1.");
    assert!(message.contains("a condition must be 0 or 1"), "{message}");
}

#[test]
fn errors_name_what_is_wrong() {
    for (source, expected) in [
        ("COMPUTE r = agee + 1.", "unknown variable agee"),
        (
            "COMPUTE r = SUM(age TO nowhere).",
            "unknown variable nowhere",
        ),
        ("COMPUTE r = FOO(age).", "unknown function FOO"),
        (
            "COMPUTE r = LNGAMMA(age).",
            "function LNGAMMA is not supported yet",
        ),
        (
            "COMPUTE r = CDF.NORMAL(age, 0, 1).",
            "function CDF.NORMAL is not supported yet",
        ),
        (
            "COMPUTE r = $JDATE.",
            "system variable $JDATE is not supported yet",
        ),
        (
            "COMPUTE r = MOD(age).",
            "wrong number of arguments for MOD: expected MOD(number, divisor), got 1",
        ),
        (
            "COMPUTE r = MEAN.4(age, region).",
            "MEAN.4 needs at least 4 arguments, got 2",
        ),
        (
            "COMPUTE r = SD(age).",
            "SD needs at least 2 arguments, got 1",
        ),
        (
            "COMPUTE r = ABS(plan).",
            "argument 1 of ABS must be a number, but variable plan is a string",
        ),
        (
            "COMPUTE r = LENGTH(age).",
            "argument 1 of LENGTH must be a string, but variable age is a number",
        ),
        (
            "COMPUTE r = plan + 1.",
            "the left operand of + must be a number, but variable plan is a string",
        ),
        (
            "COMPUTE r = age = 'x'.",
            "= cannot compare a number with a string: variable age is a number, 'x' is a string",
        ),
        (
            "COMPUTE r = SYSMIS(plan).",
            "argument 1 of SYSMIS must be a number, but variable plan is a string",
        ),
        (
            "COMPUTE r = VALUE(age + 1).",
            "the argument of VALUE must be a variable name",
        ),
        (
            "COMPUTE r = SUM(q5_5 TO q5_1).",
            "the first variable must come before the second",
        ),
        (
            "SELECT IF plan.",
            "a condition must be a number, but variable plan is a string",
        ),
        (
            "COMPUTE label = 'x'.",
            "declare it first, for example STRING label (A20)",
        ),
        (
            "COMPUTE plan = 1.",
            "plan is a string variable, but the expression gives a number",
        ),
        (
            "COMPUTE age = 'x'.",
            "age is a numeric variable, but the expression gives a string",
        ),
    ] {
        let message = error(source);
        assert!(message.contains(expected), "{source}: {message}");
    }
    let message = try_run(&mut Engine::default(), "COMPUTE r = 1.").unwrap_err();
    assert!(message.contains("GET FILE"), "{message}");
    // a type error is found even when there is no case to evaluate
    let mut empty = loaded("empty.sav");
    let message = try_run(&mut empty, "COMPUTE r = name + 1.").unwrap_err();
    assert!(message.contains("variable name is a string"), "{message}");
    try_run(&mut empty, "COMPUTE r = id + 1.\nSELECT IF r > 1.").unwrap();
}

// COUNT inside DO IF is our extension: the reference program rejects it there.
// gender is 1 1 2 3 2 2 2 2 2 1 1 2 and region 2 4 4 3 2 3 2 5 4 2 2 2 in the first twelve cases.
#[test]
fn recode_and_count_change_only_the_cases_of_the_open_branch() {
    let engine = survey(
        "DO IF (gender = 1).
RECODE region (1 THRU 5=0).
COUNT c = q5_1 (1).
END IF.",
    );
    let first = |name: &str| numbers(&engine, name)[..12].to_vec();
    let some = |values: [f64; 12]| values.map(Some).to_vec();
    assert_eq!(
        first("region"),
        some([0., 0., 4., 3., 2., 3., 2., 5., 4., 0., 0., 2.])
    );
    let c = first("c");
    assert_eq!(
        (c[0], c[1], c[2], c[9]),
        (Some(1.0), Some(0.0), None, Some(0.0))
    );
}

// age is 49 59 75 in the first three cases and plan BASIC PREMIUM BASIC.
#[test]
fn casenum_counts_cases_from_one_so_one_case_can_be_changed() {
    let engine = survey(
        "COMPUTE n = $CASENUM.
IF ($CASENUM = 2) age = 33.
IF ($casenum = 3) plan = 'GOLD'.
IF ($CASENUM = 1) monthly_spend = $SYSMIS.",
    );
    assert_eq!(
        numbers(&engine, "n")[..3],
        [Some(1.0), Some(2.0), Some(3.0)]
    );
    assert_eq!(number(&engine, "n", 200), Some(200.0));
    let age: Vec<_> = (1..=3).map(|case| number(&engine, "age", case)).collect();
    assert_eq!(age, [Some(49.0), Some(33.0), Some(75.0)]);
    assert_eq!(text(&engine, "plan", 3), "GOLD");
    assert_eq!(text(&engine, "plan", 1), "BASIC");
    assert_eq!(number(&engine, "monthly_spend", 1), None);
    assert_eq!(number(&engine, "monthly_spend", 2), Some(127.1));
}

#[test]
fn a_function_name_may_be_shortened_to_three_letters_that_start_only_it() {
    let mut engine = Engine::default();
    let data = "DATA LIST FREE /x.\nBEGIN DATA\n1 . 3\nEND DATA.\n";
    let syntax = "COMPUTE y = 0.\nIF (sys(x)) y = 1.\nCOMPUTE n = NVAL(x, 2).\n\
                  COMPUTE m = mea(x, 5).";
    try_run(&mut engine, &format!("{data}{syntax}")).unwrap();
    assert_eq!(numbers(&engine, "y"), [Some(0.0), Some(1.0), Some(0.0)]);
    assert_eq!(numbers(&engine, "n"), [Some(2.0), Some(1.0), Some(2.0)]);
    assert_eq!(numbers(&engine, "m"), [Some(3.0), Some(5.0), Some(4.0)]);
    for (syntax, expected) in [
        // STR starts STRING and STRUNC, SY is too short
        ("COMPUTE z = STR(x, F2.0).", "unknown function STR"),
        ("COMPUTE z = SY(x).", "unknown function SY"),
    ] {
        let message = try_run(&mut engine, syntax).unwrap_err();
        assert_eq!(message, expected, "{syntax}");
    }
}
