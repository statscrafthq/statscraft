//! RECODE, COUNT, SORT CASES, RENAME VARIABLES, DELETE VARIABLES, WEIGHT and AGGREGATE.
//! The expected values agree with the reference program on the same fixtures; the tests in
//! `reference_data.rs` repeat the comparison on whole files when that program is at hand.

use statscraft_data::{Dataset, Measure, MissingValues, Values};
use statscraft_engine::Engine;
use statscraft_syntax::parse;

fn fixture(name: &str) -> String {
    format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn run(engine: &mut Engine, source: &str) {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            panic!("line {}: {error}", command.first_line);
        }
    }
}

/// The message of the error that the one command in `source` ends with.
fn error(engine: &mut Engine, source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty() && commands.len() == 1, "{errors:?}");
    engine
        .execute(&commands[0].command)
        .unwrap_err()
        .to_string()
}

fn loaded(name: &str) -> Engine {
    let mut engine = Engine::default();
    run(&mut engine, &format!("GET FILE='{}'.", fixture(name)));
    engine
}

/// 200 cases in `resp_id` order, 1001 to 1200, without a weight variable.
fn survey() -> Engine {
    loaded("survey_basic.sav")
}

/// The same survey with `weight` as the weight variable.
fn weighted() -> Engine {
    loaded("survey_basic_phpwriter.sav")
}

/// The values of the first `cases` cases: numbers separated by spaces with `.` for
/// system-missing, strings separated by `|`.
fn show_in(data: &Dataset, name: &str, cases: usize) -> String {
    match data.variable(name).unwrap().values() {
        Values::Numeric(values) => {
            let cells = values.iter().take(cases);
            let text = cells.map(|v| v.map_or(".".to_string(), |v| v.to_string()));
            text.collect::<Vec<_>>().join(" ")
        }
        Values::String(values) => values.iter().take(cases).collect::<Vec<_>>().join("|"),
    }
}

fn show(engine: &Engine, name: &str, cases: usize) -> String {
    show_in(engine.dataset().unwrap(), name, cases)
}

fn column(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    data.variable(name)
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect()
}

fn names(engine: &Engine) -> Vec<&str> {
    engine.dataset().unwrap().variable_names().collect()
}

/// Expected values come from a listing with six decimals.
fn assert_close(engine: &Engine, name: &str, expected: &[f64]) {
    let actual = column(engine, name);
    assert_eq!(actual.len(), expected.len(), "{name}");
    for (case, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let actual = actual.unwrap_or_else(|| panic!("{name} case {case} is missing"));
        assert!(
            (actual - expected).abs() < 1e-6,
            "{name} case {case}: {actual} vs {expected}"
        );
    }
}

// First 14 cases of the survey:
// q1_satisfaction 3 4 4 4 3 5 5 99 3 3 99 2 4 5         (98 and 99 are user-missing)
// q2_recommend    6 10 99 97 98 8 7 7 8 8 7 0 8 7       (97 to 99 are user-missing)
// monthly_spend   78.28 127.1 278.21 206.93 113.8 143.01 148.61 198.93 70.38 294.05 120.96 . 56.91 188.22
#[test]
fn recode_matches_user_missing_values_by_value_and_system_missing_only_by_keyword() {
    let mut engine = survey();
    run(
        &mut engine,
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
RECODE q2_recommend (99=1) (MISSING=2) (98=3) INTO h.
RECODE q2_recommend (8 THRU 7=1) INTO i.",
    );
    let expected = [
        // a plain value and a range match user-missing values
        ("a", "1 . . . 1 . . 7 1 1 7 1 . ."),
        ("b", "1 . 50 50 50 . . . . . . 1 . ."),
        ("c", "1 1 1 1 1 1 1 1 1 1 1 1 1 1"),
        // but no range matches system-missing; ELSE and MISSING do
        ("c2", "1 1 1 1 1 1 1 1 1 1 1 . 1 1"),
        ("d", "2 1 1 1 1 1 1 1 2 1 1 2 2 1"),
        ("e", "6 10 -1 -1 -1 8 7 7 8 8 7 0 8 7"),
        (
            "e2",
            "78.28 127.1 278.21 206.93 113.8 143.01 148.61 198.93 70.38 294.05 120.96 -1 56.91 188.22",
        ),
        ("f", "6 10 99 97 98 8 7 7 8 8 7 0 8 7"),
        ("f2", ". . . . . . . . . . . -1 . ."),
        ("g", "0 0 99 97 98 0 0 0 0 0 0 0 0 0"),
        // the first matching specification wins
        ("h", ". . 1 2 2 . . . . . . . . ."),
        // a range written high THRU low is read as low THRU high
        ("i", ". . . . . 1 1 1 1 1 1 . 1 1"),
    ];
    for (name, values) in expected {
        assert_eq!(show(&engine, name, 14), values, "{name}");
    }
    let new = engine.dataset().unwrap().variable("a").unwrap();
    assert_eq!((new.format(), new.label()), ("F8.2", None));
}

#[test]
fn unmatched_values_stay_unchanged_in_place_and_in_an_existing_target() {
    let mut engine = survey();
    assert_eq!(show(&engine, "region", 12), "2 4 4 3 2 3 2 5 4 2 2 2");
    run(&mut engine, "RECODE region (1,2=10) (3 THRU 4=20).");
    assert_eq!(
        show(&engine, "region", 12),
        "10 20 20 20 10 20 10 5 20 10 10 10"
    );
    let region = engine.dataset().unwrap().variable("region").unwrap();
    assert_eq!((region.format(), region.value_labels().len()), ("F1.0", 5));

    // gender is 1 1 2 3 2 2 2 2 2 1 1 2: an existing target keeps its value where nothing matches
    let ages = show(&engine, "age", 12);
    run(&mut engine, "RECODE gender (1=100) INTO age.");
    let expected: Vec<String> = ages
        .split(' ')
        .zip("1 1 2 3 2 2 2 2 2 1 1 2".split(' '))
        .map(|(age, gender)| {
            if gender == "1" {
                "100".into()
            } else {
                age.to_string()
            }
        })
        .collect();
    assert_eq!(show(&engine, "age", 12), expected.join(" "));

    // the second variable is recoded from what the first one just wrote
    let recoded = |source: &str, target: &str| -> String {
        source
            .split(' ')
            .zip(target.split(' '))
            .map(|(s, t)| if s == "1" { "2" } else { t })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let (q5_1, q5_2, q5_3) = (
        show(&engine, "q5_1", 12),
        show(&engine, "q5_2", 12),
        show(&engine, "q5_3", 12),
    );
    run(&mut engine, "RECODE q5_1 q5_2 (1=2) INTO q5_2 q5_3.");
    let new_q5_2 = recoded(&q5_1, &q5_2);
    assert_eq!(show(&engine, "q5_2", 12), new_q5_2);
    assert_eq!(show(&engine, "q5_3", 12), recoded(&new_q5_2, &q5_3));

    run(
        &mut engine,
        "RECODE q5_4 q5_5 (0=5) (1=6) INTO r1 TO r2 / q6_mention1 TO q6_mention3 (SYSMIS=0).",
    );
    assert_eq!(show(&engine, "q5_4", 6), "1 0 1 0 0 0");
    assert_eq!(show(&engine, "r1", 6), "6 5 6 5 5 5");
    assert_eq!(names(&engine)[19..], ["r1", "r2"]);
    assert_eq!(show(&engine, "q6_mention3", 6), "0 0 0 0 0 0");
}

#[test]
fn recode_strings() {
    let mut engine = survey();
    assert_eq!(
        show(&engine, "plan", 9),
        "BASIC|PREMIUM|BASIC|BASIC|BASIC|ENTERPRISE|PLUS|BASIC|PLUS"
    );
    // padding does not matter, case does
    run(
        &mut engine,
        "RECODE plan ('BASIC'='B') ('plus'='P') ('PLUS   '='PP').
RECODE plan ('B'=1) ('PP'=2) INTO plan_number.",
    );
    assert_eq!(
        show(&engine, "plan", 9),
        "B|PREMIUM|B|B|B|ENTERPRISE|PP|B|PP"
    );
    assert_eq!(show(&engine, "plan_number", 9), "1 . 1 1 1 . 2 1 2");

    run(
        &mut engine,
        "RECODE gender (1='M') (2='F') (ELSE='?') INTO plan.",
    );
    assert_eq!(show(&engine, "plan", 9), "M|M|F|?|F|F|F|F|F");

    // COPY cuts a longer value to the width of the target; without a match the target keeps its value
    let mut engine = survey();
    let comments = show(&engine, "q3_comment", 5);
    run(
        &mut engine,
        "RECODE q3_comment (''='none') (ELSE=COPY) INTO plan.
RECODE plan ('none'='x') INTO q3_comment.",
    );
    assert_eq!(
        show(&engine, "plan", 5),
        "Please add|Please add|none|The mobile|The suppor"
    );
    let mut expected: Vec<&str> = comments.split('|').collect();
    expected[2] = "x";
    assert_eq!(show(&engine, "q3_comment", 5), expected.join("|"));
}

#[test]
fn recode_errors_name_the_variable_and_the_fix() {
    let mut engine = survey();
    let before = engine.dataset().unwrap().describe();
    for (source, expected) in [
        (
            "RECODE plan ('BASIC'=1).",
            "INTO is required to recode the string variable plan into numeric values",
        ),
        (
            "RECODE gender (1='x').",
            "INTO is required to recode the numeric variable gender into string values",
        ),
        (
            "RECODE plan (1='x').",
            "plan is a string variable, so its values must be quoted strings (numbers, ranges and SYSMIS do not apply)",
        ),
        (
            "RECODE gender ('a'=1).",
            "gender is a numeric variable, so its values must be numbers, not quoted strings",
        ),
        (
            "RECODE gender (1='a') INTO label.",
            "the string target label does not exist; create it first with the STRING command, for example STRING label (A8)",
        ),
        (
            "RECODE gender (1=2) INTO plan.",
            "cannot store numeric values in the string variable plan",
        ),
        (
            "RECODE plan ('BASIC'='ABCDEFGHIJKLM').",
            "the output value 'ABCDEFGHIJKLM' needs 13 bytes but plan has width 10",
        ),
        (
            "RECODE gender age (1=2) INTO only_one.",
            "2 source variables (gender age) but 1 target variables after INTO (only_one); give one target per source",
        ),
        (
            "RECODE plan ('BASIC'=1) (ELSE=COPY) INTO n.",
            "the output values of plan must be all numbers or all strings (SYSMIS is a number, COPY has the type of the source variable)",
        ),
        ("RECODE nope (1=2).", "unknown variable nope"),
        // the first group is undone when a later one fails
        (
            "RECODE gender (1=9) INTO g2 / plan (1=2).",
            "plan is a string variable, so its values must be quoted strings (numbers, ranges and SYSMIS do not apply)",
        ),
    ] {
        assert_eq!(error(&mut engine, source), expected, "{source}");
    }
    let after = engine.dataset().unwrap().describe();
    assert_eq!(
        serde_json::to_value(before).unwrap(),
        serde_json::to_value(after).unwrap()
    );
}

#[test]
fn count_values_ranges_and_missing() {
    let mut engine = survey();
    run(
        &mut engine,
        "COUNT c1 = q5_1 TO q5_5 (1).
COUNT c2 = q1_satisfaction q2_recommend (99).
COUNT c3 = q1_satisfaction q2_recommend (90 THRU HI).
COUNT c4 = q1_satisfaction q2_recommend monthly_spend (MISSING).
COUNT c5 = q1_satisfaction q2_recommend monthly_spend q6_mention2 (SYSMIS).
COUNT c6 = q5_1 q5_2 (1) q6_mention1 q6_mention2 (1, 3 THRU 4) plan ('BASIC', 'PLUS  ').
COUNT c7 = q2_recommend (LO THRU 5, 10) / c9 = q5_1 q5_1 (0 1).
COUNT c10 = q2_recommend monthly_spend (LO THRU HI).
COUNT age = gender (1).",
    );
    let expected = [
        ("c1", "4 2 3 3 3 2 1 1 2 3 3 3"),
        // values and ranges count user-missing values, LO THRU HI does not reach system-missing
        ("c2", "0 0 1 0 0 0 0 1 0 0 1 0"),
        ("c3", "0 0 1 1 1 0 0 1 0 0 1 0"),
        ("c4", "0 0 1 1 1 0 0 1 0 0 1 1"),
        ("c5", "0 0 1 1 1 1 0 0 1 0 1 2"),
        ("c6", "4 3 3 4 3 1 2 2 3 3 0 2"),
        ("c7", "0 1 0 0 0 0 0 0 0 0 0 1"),
        // a variable listed twice counts twice
        ("c9", "2 2 2 2 2 2 2 2 2 2 2 2"),
        ("c10", "2 2 2 2 2 2 2 2 2 2 2 1"),
        ("age", "1 1 0 0 0 0 0 0 0 1 1 0"),
    ];
    for (name, values) in expected {
        assert_eq!(show(&engine, name, 12), values, "{name}");
    }
    let data = engine.dataset().unwrap();
    assert_eq!(data.variable("c1").unwrap().format(), "F8.2");
    // an existing target keeps its dictionary entry
    let age = data.variable("age").unwrap();
    assert_eq!((age.format(), age.label()), ("F3.0", Some("Age in years")));

    for (source, expected) in [
        (
            "COUNT plan = gender (1).",
            "the target plan is a string variable; a count needs a numeric or a new variable",
        ),
        (
            "COUNT n = plan (1).",
            "plan is a string variable, so its values must be quoted strings (numbers, ranges and SYSMIS do not apply)",
        ),
        (
            "COUNT n = gender ('a').",
            "gender is a numeric variable, so its values must be numbers, not quoted strings",
        ),
        ("COUNT n = nope (1).", "unknown variable nope"),
    ] {
        assert_eq!(error(&mut engine, source), expected, "{source}");
    }
}

#[test]
fn sort_cases_is_stable_and_puts_system_missing_lowest() {
    let mut engine = survey();
    run(&mut engine, "SORT CASES BY q6_mention2 region (D).");
    let ids = column(&engine, "resp_id");
    assert_eq!(
        [ids[0], ids[7], ids[14], ids[140]],
        [Some(1008.0), Some(1185.0), Some(1172.0), Some(1152.0)]
    );
    // 93 cases have no second mention: descending puts them last, ascending first
    let mentions = column(&engine, "q6_mention2");
    assert_eq!(mentions.iter().position(Option::is_none), Some(107));
    assert_eq!(mentions[0], Some(6.0));
    run(&mut engine, "SORT CASES BY q6_mention2.");
    let mentions = column(&engine, "q6_mention2");
    assert!(mentions[..93].iter().all(Option::is_none));
    assert_eq!((mentions[93], mentions[199]), (Some(1.0), Some(6.0)));

    // the file is in resp_id order, so equal keys must leave resp_id ascending
    let mut engine = survey();
    run(&mut engine, "SORT CASES gender (A).");
    let (genders, ids) = (column(&engine, "gender"), column(&engine, "resp_id"));
    assert_eq!(
        [ids[0], ids[106], ids[194]],
        [Some(1001.0), Some(1003.0), Some(1004.0)]
    );
    for case in 1..200 {
        assert!(genders[case - 1] <= genders[case]);
        assert!(genders[case - 1] < genders[case] || ids[case - 1] < ids[case]);
    }

    // a direction covers the variables before it, so age needs its own
    run(&mut engine, "SORT CASES BY plan (D) age (A) resp_id (D).");
    let data = engine.dataset().unwrap();
    let plans = data.variable("plan").unwrap().string().unwrap();
    let plans: Vec<&str> = plans.iter().collect();
    let (ages, ids) = (column(&engine, "age"), column(&engine, "resp_id"));
    assert_eq!((plans[0], plans[199]), ("PREMIUM", "BASIC"));
    for case in 1..200 {
        let key = |case: usize| (std::cmp::Reverse(plans[case]), ages[case]);
        assert!(key(case - 1) <= key(case));
        assert!(key(case - 1) < key(case) || ids[case - 1] > ids[case]);
    }
    assert_eq!((data.case_count(), data.variable_count()), (200, 19));

    assert_eq!(
        error(&mut engine, "SORT CASES BY nope."),
        "unknown variable nope"
    );
}

#[test]
fn rename_variables() {
    let mut engine = weighted();
    run(
        &mut engine,
        "RENAME VARIABLES (gender age = age gender) (q5_1 TO q5_3 = ch1 TO ch3).
RENAME VARIABLES region = area weight = w.",
    );
    assert_eq!(names(&engine)[..4], ["resp_id", "age", "gender", "area"]);
    assert_eq!(names(&engine)[10..13], ["ch1", "ch2", "ch3"]);
    // the values stayed in place: the variable now called age holds the gender codes
    assert_eq!(show(&engine, "age", 4), "1 1 2 3");
    let data = engine.dataset().unwrap();
    assert_eq!(data.weight_variable(), Some("w"));
    assert_eq!(data.multiple_response_sets()[0].variables[0], "ch1");
    // groups rename at the same moment
    run(
        &mut engine,
        "RENAME VARIABLES (age = gender) (gender = age).",
    );
    assert_eq!(show(&engine, "gender", 4), "1 1 2 3");

    let all = names(&engine).join(" ");
    for (source, expected) in [
        (
            "RENAME VARIABLES (age gender = x).",
            "2 old names (age gender) but 1 new names (x); give one new name per variable",
        ),
        (
            "RENAME VARIABLES (age = area).",
            "variable name already in use: area",
        ),
        (
            "RENAME VARIABLES (age area = x X).",
            "the new name X is given to two variables",
        ),
        (
            "RENAME VARIABLES (age = x) (AGE = y).",
            "age is renamed twice",
        ),
        (
            "RENAME VARIABLES (resp_id = id) (id = id2).",
            "unknown variable id",
        ),
        (
            "RENAME VARIABLES (age area = a1 TO b2).",
            "a1 TO b2 cannot name new variables: both names need the same stem and a number at the end, as in v1 TO v5 (at most 10000 names)",
        ),
    ] {
        assert_eq!(error(&mut engine, source), expected, "{source}");
        assert_eq!(names(&engine).join(" "), all, "{source} changed names");
    }

    run(&mut engine, "RENAME VARIABLES (ALL = v01 TO v19).");
    assert_eq!(names(&engine)[..2], ["v01", "v02"]);
    assert_eq!(names(&engine)[18], "v19");
    assert_eq!(engine.dataset().unwrap().weight_variable(), Some("v19"));
}

#[test]
fn delete_variables_and_weight() {
    let mut engine = weighted();
    run(
        &mut engine,
        "DELETE VARIABLES q3_comment signup_date TO q5_5 age age.",
    );
    assert_eq!(
        names(&engine),
        [
            "resp_id",
            "gender",
            "region",
            "q1_satisfaction",
            "q2_recommend",
            "monthly_spend",
            "plan",
            "q6_mention1",
            "q6_mention2",
            "q6_mention3",
            "weight"
        ]
    );
    assert_eq!(engine.dataset().unwrap().weight_variable(), Some("weight"));
    assert_eq!(
        error(&mut engine, "DELETE VARIABLES ALL."),
        "cannot delete every variable"
    );
    assert_eq!(
        error(&mut engine, "DELETE VARIABLES gender nope."),
        "unknown variable nope"
    );

    run(&mut engine, "WEIGHT OFF.");
    assert_eq!(engine.dataset().unwrap().weight_variable(), None);
    run(&mut engine, "WEIGHT BY Monthly_Spend.");
    assert_eq!(
        engine.dataset().unwrap().weight_variable(),
        Some("monthly_spend")
    );
    run(&mut engine, "WEIGHT weight.");
    assert_eq!(engine.dataset().unwrap().weight_variable(), Some("weight"));
    assert_eq!(
        error(&mut engine, "WEIGHT BY plan."),
        "plan is not a numeric variable"
    );
    assert_eq!(
        error(&mut engine, "WEIGHT BY nope."),
        "unknown variable: nope"
    );
    // deleting the weight variable turns weighting off
    run(&mut engine, "DELETE VARIABLES weight.");
    assert_eq!(engine.dataset().unwrap().weight_variable(), None);
}

const AGGREGATIONS: &str = "
  /s_sum = SUM(monthly_spend) /s_mean = MEAN(monthly_spend) /s_sd = SD(monthly_spend)
  /s_med = MEDIAN(monthly_spend) /s_min = MIN(monthly_spend) /s_max = MAX(monthly_spend)
  /s_first = FIRST(monthly_spend) /s_last = LAST(monthly_spend)
  /n = N /nu = NU /n_spend = N(monthly_spend) /nmiss = NMISS(monthly_spend)
  /numiss = NUMISS(monthly_spend)
  /q1_mean 'Mean of q1' = MEAN(q1_satisfaction) /q1_n = N(q1_satisfaction)
  /q1_max = MAX(q1_satisfaction) /q1_first = FIRST(q1_satisfaction)
  /p_min = MIN(plan) /p_max = MAX(plan) /p_first = FIRST(plan) /p_last = LAST(plan)
  /d_min = MIN(signup_date).";

#[test]
fn aggregate_replaces_the_dataset_with_one_case_per_group() {
    let mut engine = survey();
    run(
        &mut engine,
        &format!("AGGREGATE /OUTFILE=* /BREAK=region gender{AGGREGATIONS}"),
    );
    // the break variables come first, in BREAK order, and the groups in ascending order
    assert_eq!(names(&engine)[..4], ["region", "gender", "s_sum", "s_mean"]);
    assert_eq!(engine.dataset().unwrap().case_count(), 13);
    assert_eq!(show(&engine, "region", 13), "1 1 2 2 2 3 3 3 4 4 5 5 5");
    assert_eq!(show(&engine, "gender", 13), "1 2 1 2 3 1 2 3 1 2 1 2 3");

    // missing values, system and user, are left out
    let all = "23 11 23 23 1 15 13 2 27 16 18 25 3";
    assert_eq!(show(&engine, "n", 13), all);
    assert_eq!(show(&engine, "nu", 13), all);
    assert_eq!(
        show(&engine, "n_spend", 13),
        "21 9 22 20 1 13 13 2 23 13 18 23 3"
    );
    assert_eq!(show(&engine, "nmiss", 13), "2 2 1 3 0 2 0 0 4 3 0 2 0");
    assert_eq!(show(&engine, "numiss", 13), "2 2 1 3 0 2 0 0 4 3 0 2 0");
    assert_eq!(
        show(&engine, "q1_n", 13),
        "19 9 21 21 1 15 13 2 24 15 17 19 3"
    );
    assert_close(
        &engine,
        "s_sum",
        &[
            3490.6, 2243.48, 3394.96, 3373.12, 351.61, 1944.97, 2296.12, 325.49, 4635.16, 2539.44,
            3513.12, 5249.99, 565.17,
        ],
    );
    assert_close(
        &engine,
        "s_mean",
        &[
            166.219048, 249.275556, 154.316364, 168.656, 351.61, 149.613077, 176.624615, 162.745,
            201.528696, 195.341538, 195.173333, 228.260435, 188.39,
        ],
    );
    assert_close(
        &engine,
        "s_med",
        &[
            169.7, 251.49, 120.865, 140.93, 351.61, 116.13, 181.44, 162.745, 211.44, 209.67,
            179.44, 248.2, 194.3,
        ],
    );
    assert_close(
        &engine,
        "q1_mean",
        &[
            3.684211, 3.777778, 3.142857, 4.047619, 5.0, 3.466667, 3.230769, 4.0, 3.333333,
            3.533333, 4.0, 3.368421, 3.666667,
        ],
    );
    // the standard deviation of a single case is system-missing
    let sd = column(&engine, "s_sd");
    assert_eq!(sd[4], None);
    for (case, expected) in [
        (0, 98.774599),
        (1, 112.24802),
        (7, 62.487026),
        (12, 147.273963),
    ] {
        assert!((sd[case].unwrap() - expected).abs() < 1e-6, "s_sd {case}");
    }
    assert_eq!(show(&engine, "s_min", 2), "30.66 66.25");
    assert_eq!(show(&engine, "s_max", 2), "366.66 372.02");
    assert_eq!(show(&engine, "s_first", 2), "34.53 364.12");
    assert_eq!(show(&engine, "s_last", 2), "366.66 372.02");
    assert_eq!(show(&engine, "q1_max", 5), "5 5 4 5 5");
    assert_eq!(show(&engine, "q1_first", 3), "4 4 3");
    assert_eq!(show(&engine, "p_min", 5), "BASIC|BASIC|BASIC|BASIC|PLUS");
    assert_eq!(show(&engine, "p_max", 3), "PREMIUM|PREMIUM|PREMIUM");
    assert_eq!(show(&engine, "p_first", 3), "ENTERPRISE|BASIC|BASIC");
    assert_eq!(show(&engine, "p_last", 3), "PLUS|ENTERPRISE|PLUS");

    // computed targets get a plain format; MIN, MAX, FIRST and LAST copy the source variable
    let data = engine.dataset().unwrap();
    let entry = |name: &str| {
        let v = data.variable(name).unwrap();
        (v.format(), v.label())
    };
    assert_eq!(entry("region"), ("F1.0", Some("Region of residence")));
    assert_eq!(entry("s_sum"), ("F8.2", None));
    assert_eq!(entry("s_med"), ("F8.2", None));
    assert_eq!(entry("n"), ("F7.0", None));
    assert_eq!(entry("nmiss"), ("F7.0", None));
    assert_eq!(entry("q1_mean"), ("F8.2", Some("Mean of q1")));
    assert_eq!(entry("p_first"), ("A10", Some("Current subscription plan")));
    assert_eq!(
        entry("d_min"),
        ("DATE11", Some("Date the customer signed up"))
    );
    let q1_max = data.variable("q1_max").unwrap();
    assert_eq!(q1_max.format(), "F2.0");
    assert!(q1_max.label().unwrap().starts_with("Q1."));
    assert_eq!(q1_max.value_labels().len(), 7);
    assert_eq!(q1_max.measure(), Measure::Ordinal);
    assert_eq!(
        q1_max.missing_values(),
        &MissingValues::Numbers(vec![98.0, 99.0])
    );
}

#[test]
fn aggregate_is_weighted_when_a_weight_variable_is_set() {
    let mut engine = weighted();
    run(
        &mut engine,
        &format!("AGGREGATE /OUTFILE=* /BREAK=gender{AGGREGATIONS}"),
    );
    assert_close(&engine, "n", &[132.3074, 119.3866, 8.2401]);
    assert_close(&engine, "n_spend", &[121.1643, 104.4176, 8.2401]);
    assert_close(&engine, "nmiss", &[11.1431, 14.969, 0.0]);
    assert_close(&engine, "q1_n", &[118.8681, 103.569, 8.2401]);
    assert_close(&engine, "s_sum", &[20490.946774, 21167.475423, 1485.334443]);
    assert_close(&engine, "s_mean", &[169.117032, 202.719421, 180.256847]);
    assert_close(&engine, "s_sd", &[115.134361, 111.593461, 107.493698]);
    assert_close(&engine, "s_med", &[129.63095, 198.93, 194.3]);
    assert_close(&engine, "q1_mean", &[3.475771, 3.581882, 3.845099]);
    // NU and NUMISS count cases; MIN, MAX, FIRST and LAST ignore the weights
    assert_eq!(show(&engine, "nu", 3), "106 88 6");
    assert_eq!(show(&engine, "numiss", 3), "9 10 0");
    assert_eq!(show(&engine, "q1_max", 3), "5 5 5");
    assert_eq!(show(&engine, "p_first", 3), "BASIC|BASIC|BASIC");

    let data = engine.dataset().unwrap();
    let format = |name: &str| data.variable(name).unwrap().format();
    assert_eq!(
        [format("n"), format("nmiss"), format("nu"), format("numiss")],
        ["F8.2", "F8.2", "F7.0", "F7.0"]
    );
    // the weight variable is not among the break variables, so it is gone
    assert_eq!(data.weight_variable(), None);
    assert!(data.multiple_response_sets().is_empty());

    // a case with a missing, zero or negative weight counts for NU only
    let mut engine = weighted();
    run(
        &mut engine,
        "RECODE weight (LO THRU 1 = 0) (1.5 THRU 1.8 = -1) (2 THRU HI = SYSMIS).
AGGREGATE /OUTFILE=* /n = N /nu = NU /s = SUM(weight) /mx = MAX(resp_id) /m = MEAN(age).
COUNT positive = n s m (0 THRU HI).",
    );
    assert_eq!(show(&engine, "nu", 1), "200");
    assert_eq!(show(&engine, "mx", 1), "1200");
    assert_eq!(show(&engine, "positive", 1), "3");
    let n = column(&engine, "n")[0].unwrap();
    let s = column(&engine, "s")[0].unwrap();
    // the remaining weights lie between 1 and 2, so their squares add up to more than N
    assert!(n > 20.0 && n < 150.0 && s > n, "{n} {s}");
}

#[test]
fn aggregate_adds_group_values_to_every_case() {
    // the cases are not sorted by plan: each one still gets the value of its whole group
    let mut engine = survey();
    run(
        &mut engine,
        "AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=plan
  /plan_age 'Mean age of the plan' = MEAN(age) /plan_n = N /plan_first = FIRST(resp_id).",
    );
    assert_eq!(
        show(&engine, "plan", 7),
        "BASIC|PREMIUM|BASIC|BASIC|BASIC|ENTERPRISE|PLUS"
    );
    assert_eq!(show(&engine, "resp_id", 3), "1001 1002 1003");
    assert_eq!(show(&engine, "plan_n", 7), "83 34 83 83 83 22 61");
    assert_eq!(
        show(&engine, "plan_first", 7),
        "1001 1002 1001 1001 1001 1006 1007"
    );
    let ages = column(&engine, "plan_age");
    let expected = [
        49.807229, 54.264706, 49.807229, 49.807229, 49.807229, 57.0, 48.131148,
    ];
    for (case, expected) in expected.iter().enumerate() {
        assert!((ages[case].unwrap() - expected).abs() < 1e-6, "case {case}");
    }
    let data = engine.dataset().unwrap();
    assert_eq!((data.case_count(), data.variable_count()), (200, 22));
    assert_eq!(names(&engine)[19..], ["plan_age", "plan_n", "plan_first"]);
    assert_eq!(
        data.variable("plan_age").unwrap().label(),
        Some("Mean age of the plan")
    );
    assert_eq!(data.variable("plan_first").unwrap().format(), "F6.0");

    // without BREAK all cases form one group
    run(
        &mut engine,
        "AGGREGATE /OUTFILE=* MODE=ADDVARIABLES
  /all_mean = MEAN(age) /all_n = N /all_sd = SD(age) /all_max = MAX(age) /all_med = MEDIAN(q2_recommend).",
    );
    assert_eq!(show(&engine, "all_n", 2), "200 200");
    assert_eq!(show(&engine, "all_max", 2), "84 84");
    assert_eq!(show(&engine, "all_med", 2), "8 8");
    assert_eq!(show(&engine, "all_mean", 1), "50.845");
    assert!((column(&engine, "all_sd")[199].unwrap() - 19.227048).abs() < 1e-6);
}

#[test]
fn aggregate_to_a_file_leaves_the_active_dataset_alone() {
    let mut engine = survey();
    let path = std::env::temp_dir().join(format!("statscraft-agg-{}.sav", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let command = format!(
        "AGGREGATE /OUTFILE='{}' /BREAK=gender (D) /age = MEAN(age) /n 'Cases' = N.",
        path.display()
    );
    engine.dry_run = true;
    run(&mut engine, &command);
    assert!(!path.exists());
    engine.dry_run = false;
    run(&mut engine, &command);
    let data = engine.dataset().unwrap();
    assert_eq!((data.case_count(), data.variable_count()), (200, 19));

    let written = Dataset::read_sav(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    let written_names: Vec<&str> = written.variable_names().collect();
    assert_eq!(written_names, ["gender", "age", "n"]);
    assert_eq!(show_in(&written, "gender", 3), "3 2 1");
    assert_eq!(show_in(&written, "n", 3), "6 88 106");
    // a target may take the name of a variable that is not kept; it does not inherit from it
    let age = written.variable("age").unwrap();
    assert_eq!((age.format(), age.label()), ("F8.2", None));
    assert_eq!(written.variable("n").unwrap().label(), Some("Cases"));

    // without BREAK the result is a single case
    run(
        &mut engine,
        "AGGREGATE /OUTFILE=* /n = N /oldest = MAX(age).",
    );
    assert_eq!(names(&engine), ["n", "oldest"]);
    assert_eq!(show(&engine, "n", 9), "200");
    assert_eq!(show(&engine, "oldest", 9), "84");
}

#[test]
fn aggregate_errors() {
    let mut engine = survey();
    for (source, expected) in [
        (
            "AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=gender /age = MEAN(age).",
            "the target age already exists in the dataset; choose a new name or delete the variable first",
        ),
        (
            "AGGREGATE /OUTFILE=* /BREAK=gender /Gender = N.",
            "the target Gender has the name of a break variable; choose another name",
        ),
        (
            "AGGREGATE /OUTFILE=* /BREAK=gender /m = MEAN(age) /M = SUM(age).",
            "the target name M is used twice",
        ),
        (
            "AGGREGATE /OUTFILE=* /BREAK=gender /m = SUM(plan).",
            "SUM needs a numeric variable, plan is a string variable",
        ),
        (
            "AGGREGATE /OUTFILE=* /BREAK=gender /m = MEAN(q5_1 TO q5_3).",
            "1 target variables (m) but 3 source variables in MEAN(q5_1 q5_2 q5_3); give one target per source",
        ),
        (
            "AGGREGATE /OUTFILE='x.sav' MODE=ADDVARIABLES /m = MEAN(age).",
            "MODE=ADDVARIABLES adds to the active dataset, so it needs OUTFILE=* instead of a file",
        ),
        (
            "AGGREGATE /OUTFILE=* /BREAK=nope /m = MEAN(age).",
            "unknown variable nope",
        ),
    ] {
        assert_eq!(error(&mut engine, source), expected, "{source}");
    }
    let data = engine.dataset().unwrap();
    assert_eq!((data.case_count(), data.variable_count()), (200, 19));
}
