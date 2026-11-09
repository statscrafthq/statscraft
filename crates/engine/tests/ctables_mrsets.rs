//! MRSETS and multiple response sets in CTABLES.
//!
//! The rules below are our summary of the Command Syntax Reference (MRSETS, pages 1167-1169;
//! CTABLES "Category Variables and Multiple Response Sets" and "Specifying Summaries", pages
//! 482-490; "Missing Values in Summaries", Table 31, page 491; the MRSETS subcommand, page 503),
//! the Custom Tables manual (multiple response sets, pages 6-8 and 60-64) and the CTABLES
//! algorithms (case weights count cases, not responses):
//!
//! - MRSETS defines a dichotomy set (MDGROUP: one category per variable, a response where the
//!   variable holds the counted value) or a category set (MCGROUP: the values of its variables are
//!   the categories). A set name starts with $, a set has two or more variables of one type, a
//!   numeric counted value is a whole number, a string one fits the narrowest variable. A name
//!   defined again replaces the set. Sets are stored in the data file.
//! - In CTABLES a set stands where a categorical variable stands. A case counts once in each
//!   category it has a response in, and once in the set's total if it has any response.
//! - RESPONSES counts responses. For a dichotomy set it equals the count in every category; the
//!   totals differ. A value repeated in two variables of a category set is one response unless
//!   /MRSETS COUNTDUPLICATES=YES; COUNT never counts it twice.
//! - Percentages: base.COUNT uses cases (each case once per area), base.RESPONSES responses over
//!   responses, base.RESPONSES.COUNT responses over cases, base.COUNT.RESPONSES cases over
//!   responses.
//! - A case without a response counts in TOTALN only. A case whose responses all lie in
//!   categories that /CATEGORIES leaves out is not in the table.
//! - A set may carry the summaries while categorical variables are nested inside it; of two
//!   nested sets only the inner one may. The response functions need a set as the summarized
//!   variable, and a set has none of the scale functions.
//!
//! Every expected number is checked against a hand count of the fixture or computed again here
//! from the raw values, without the engine.

use statscraft_data::{Dataset, Values};
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn fixture(name: &str) -> String {
    format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn run_on(engine: &mut Engine, source: &str) -> Result<Vec<Table>, String> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for c in &commands {
        tables.extend(engine.execute(&c.command).map_err(|e| e.to_string())?);
    }
    Ok(tables)
}

const SETS: &str = "MRSETS /MDGROUP NAME=$q5 LABEL='Q5. Contact channels used' VARIABLES=q5_1 TO q5_5 VALUE=1\n\
                    /MCGROUP NAME=$q6 LABEL='Q6. Other providers mentioned' VARIABLES=q6_mention1 TO q6_mention3.\n";

/// survey_basic.sav with the sets of survey_basic_phpwriter.sav defined by MRSETS, unweighted.
fn engine() -> Engine {
    let mut engine = Engine::default();
    let source = format!("GET FILE='{}'.\n{SETS}", fixture("survey_basic.sav"));
    run_on(&mut engine, &source).unwrap();
    engine
}

fn table(engine: &mut Engine, source: &str) -> Table {
    run_on(engine, source).unwrap().remove(0)
}

fn error(engine: &mut Engine, source: &str) -> String {
    run_on(engine, source).unwrap_err()
}

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(t) => t,
        other => panic!("not text: {other:?}"),
    }
}

/// The numbers of one row; None for an empty cell.
fn numbers(t: &Table, row: usize) -> Vec<Option<f64>> {
    t.rows[row][t.row_headers..]
        .iter()
        .map(|c| match c {
            Cell::Number(n) => Some(*n),
            Cell::Empty => None,
            other => panic!("not a number: {other:?}"),
        })
        .collect()
}

fn close(actual: &[Option<f64>], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} vs {expected:?}");
    for (a, e) in actual.iter().zip(expected) {
        let a = a.unwrap_or_else(|| panic!("empty cell, expected {e}: {actual:?}"));
        assert!(
            (a - e).abs() < 1e-9 * e.abs().max(1.0),
            "{actual:?} vs {expected:?}"
        );
    }
}

fn row_labels(t: &Table, column: usize) -> Vec<&str> {
    t.rows.iter().map(|r| text(&r[column])).collect()
}

/// A numeric value straight from the data.
fn value(data: &Dataset, name: &str, case: usize) -> Option<f64> {
    match data.variable(name).unwrap().values() {
        Values::Numeric(values) => values.get(case),
        Values::String(_) => panic!("{name} is a string"),
    }
}

const Q5: [&str; 5] = ["q5_1", "q5_2", "q5_3", "q5_4", "q5_5"];
const Q6: [&str; 3] = ["q6_mention1", "q6_mention2", "q6_mention3"];

/// Per case the values given in the q6 variables, missing ones left out.
fn mentions(data: &Dataset, case: usize) -> Vec<f64> {
    Q6.iter().filter_map(|v| value(data, v, case)).collect()
}

fn chosen(data: &Dataset, case: usize) -> Vec<usize> {
    (0..5)
        .filter(|k| value(data, Q5[*k], case) == Some(1.0))
        .collect()
}

#[test]
fn mrsets_defines_the_sets_the_data_file_holds_and_they_survive_save_and_get() {
    let mut engine = engine();
    let reference = Dataset::read_sav(fixture("survey_basic_phpwriter.sav")).unwrap();
    assert_eq!(
        engine.dataset().unwrap().multiple_response_sets(),
        reference.multiple_response_sets()
    );

    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("mrsets_roundtrip.sav");
    let saved = format!("SAVE OUTFILE='{0}'.\nGET FILE='{0}'.", path.display());
    run_on(&mut engine, &saved).unwrap();
    assert_eq!(
        engine.dataset().unwrap().multiple_response_sets(),
        reference.multiple_response_sets()
    );

    let t = table(&mut engine, "MRSETS /DISPLAY NAME=ALL.");
    assert_eq!(t.title, "Multiple Response Sets");
    assert_eq!(
        t.columns,
        ["Name", "Label", "Type", "Counted value", "Variables"]
    );
    assert_eq!(text(&t.rows[0][0]), "$q5");
    assert_eq!(text(&t.rows[0][2]), "Multiple dichotomy");
    assert_eq!(text(&t.rows[0][3]), "1");
    assert_eq!(text(&t.rows[0][4]), "q5_1 q5_2 q5_3 q5_4 q5_5");
    assert_eq!(text(&t.rows[1][2]), "Multiple category");
    assert_eq!(t.rows[1][3], Cell::Empty);
}

#[test]
fn mrsets_runs_in_order_replaces_by_name_and_deletes() {
    let mut engine = engine();
    let tables = run_on(
        &mut engine,
        "MRSETS /DISPLAY NAME=[$Q6] /MCGROUP NAME=$Q6 VARIABLES=q6_mention1 q6_mention2\n\
         /DISPLAY NAME=[$q6] /DELETE NAME=[$q5] /DISPLAY NAME=ALL /DELETE NAME=ALL /DISPLAY NAME=ALL.",
    )
    .unwrap();
    assert_eq!(
        text(&tables[0].rows[0][4]),
        "q6_mention1 q6_mention2 q6_mention3"
    );
    // the same name in other case replaces the set; it has no label now
    assert_eq!(text(&tables[1].rows[0][0]), "$Q6");
    assert_eq!(text(&tables[1].rows[0][1]), "");
    assert_eq!(text(&tables[1].rows[0][4]), "q6_mention1 q6_mention2");
    assert_eq!(tables[2].rows.len(), 1);
    assert!(tables[3].rows.is_empty());
    assert_eq!(
        tables[3].caption.as_deref(),
        Some("The dataset has no multiple response sets.")
    );
}

#[test]
fn mrsets_rejects_what_the_rules_forbid_and_changes_nothing() {
    let mut engine = engine();
    for (source, message) in [
        (
            "MRSETS /MDGROUP NAME=$x VARIABLES=q5_1 VALUE=1.",
            "/MDGROUP: $x: a set needs at least two variables",
        ),
        (
            "MRSETS /MDGROUP NAME=$x VARIABLES=q5_1 q5_2 VALUE='1'.",
            "/MDGROUP: q5_1 is numeric, so VALUE is a number, not a quoted text",
        ),
        (
            "MRSETS /MDGROUP NAME=$x VARIABLES=plan q3_comment VALUE=1.",
            "/MDGROUP: plan is a string variable, so VALUE is a quoted text",
        ),
        (
            "MRSETS /MDGROUP NAME=$x VARIABLES=q5_1 q5_2 VALUE=0.5.",
            "/MDGROUP: $x: the counted value of numeric variables must be a whole number, not 0.5",
        ),
        (
            "MRSETS /MCGROUP NAME=$x VARIABLES=q5_1 plan.",
            "/MCGROUP: $x: q5_1 and plan differ in type; the variables of a set are all numeric or all strings",
        ),
        (
            "MRSETS /MCGROUP NAME=$x VARIABLES=q5_1 nope.",
            "/MCGROUP: unknown variable nope",
        ),
        // the first subcommand worked, but the run is all or nothing
        (
            "MRSETS /DELETE NAME=ALL /DELETE NAME=[$q5].",
            "/DELETE: unknown multiple response set $q5",
        ),
        (
            "MRSETS /DISPLAY NAME=[$nope].",
            "/DISPLAY: unknown multiple response set $nope",
        ),
    ] {
        assert_eq!(error(&mut engine, source), message, "{source}");
    }
    assert_eq!(engine.dataset().unwrap().multiple_response_sets().len(), 2);
}

#[test]
fn dichotomy_set_counts_cases_per_variable_and_its_total_counts_cases_with_any_response() {
    let mut engine = engine();
    let t = table(
        &mut engine,
        "CTABLES /TABLE $q5 [COUNT, RESPONSES, COLPCT.COUNT, COLPCT.RESPONSES, COLPCT.RESPONSES.COUNT,\n\
         COLPCT.COUNT.RESPONSES, COLPCT.VALIDN, COLPCT.TOTALN]\n\
         /CATEGORIES VARIABLES=$q5 TOTAL=YES.",
    );
    assert_eq!(text(&t.rows[0][0]), "Q5. Contact channels used");
    assert_eq!(
        row_labels(&t, 1),
        [
            "Q5. Which of these channels have you used to contact us? Phone",
            "Q5. Which of these channels have you used to contact us? Email",
            "Q5. Which of these channels have you used to contact us? Live chat",
            "Q5. Which of these channels have you used to contact us? Mobile app",
            "Q5. Which of these channels have you used to contact us? In-store visit",
            "Total",
        ]
    );
    assert_eq!(
        &t.columns[2..],
        [
            "Count",
            "Responses",
            "Column %",
            "Column Responses %",
            "Column Responses % (Base: Count)",
            "Column Count % (Base: Responses)",
            "Column Valid N %",
            "Column Total N %",
        ]
    );
    // hand count of the fixture: cases with 1 in q5_1 .. q5_5; 193 cases have at least one,
    // 7 have none, 482 responses in all
    let counts = [58.0, 80.0, 101.0, 112.0, 131.0];
    for (row, n) in counts.iter().enumerate() {
        close(
            &numbers(&t, row),
            &[
                *n,
                *n,
                100.0 * n / 193.0,
                100.0 * n / 482.0,
                100.0 * n / 193.0,
                100.0 * n / 482.0,
                100.0 * n / 193.0,
                // the 7 cases without a response are in the TOTALN base only
                100.0 * n / 200.0,
            ],
        );
    }
    close(
        &numbers(&t, 5),
        &[
            193.0,
            482.0,
            100.0,
            100.0,
            100.0 * 482.0 / 193.0,
            100.0 * 193.0 / 482.0,
            100.0,
            100.0,
        ],
    );
    assert_eq!(t.formats[0][3], "F40.0");
    assert_eq!(t.formats[0][5], "PCT40.1");
}

#[test]
fn category_set_counts_values_across_its_variables() {
    let mut engine = engine();
    let t = table(
        &mut engine,
        "CTABLES /TABLE $q6 [COUNT, RESPONSES, COLPCT.COUNT, COLPCT.TOTALN] /CATEGORIES VARIABLES=$q6 TOTAL=YES.",
    );
    assert_eq!(
        row_labels(&t, 1),
        [
            "Norvale Telecom",
            "Bluefinch Mobile",
            "Harborline",
            "Summit and Pine",
            "Quillnet",
            "Other provider",
            "Total"
        ]
    );
    // hand count: cases naming each provider in any of the three mentions; 171 cases name
    // one at least, 29 none (all three system-missing), and no case names one twice
    let counts = [58.0, 57.0, 37.0, 62.0, 45.0, 52.0];
    for (row, n) in counts.iter().enumerate() {
        close(
            &numbers(&t, row),
            &[*n, *n, 100.0 * n / 171.0, 100.0 * n / 200.0],
        );
    }
    close(&numbers(&t, 6), &[171.0, 311.0, 100.0, 100.0]);
}

#[test]
fn counts_ignore_repeated_values_and_responses_count_them_only_when_asked() {
    let mut engine = engine();
    // make the second mention repeat the first one for every tenth respondent
    run_on(
        &mut engine,
        "IF (MOD(resp_id, 10) = 0) q6_mention2 = q6_mention1.",
    )
    .unwrap();
    let data = engine.dataset().unwrap().clone();
    let (mut count, mut once, mut all) = ([0.0; 6], [0.0; 6], [0.0; 6]);
    let mut repeats = 0;
    for case in 0..data.case_count() {
        let given = mentions(&data, case);
        for v in 1..=6 {
            let n = given.iter().filter(|g| **g == v as f64).count() as f64;
            count[v - 1] += (n > 0.0) as u8 as f64;
            once[v - 1] += (n > 0.0) as u8 as f64;
            all[v - 1] += n;
            repeats += (n > 1.0) as usize;
        }
    }
    assert!(repeats > 5, "the change must repeat some values: {repeats}");
    let source =
        "/TABLE $q6 [COUNT, RESPONSES, COLPCT.RESPONSES] /CATEGORIES VARIABLES=$q6 TOTAL=YES.";
    let plain = table(&mut engine, &format!("CTABLES {source}"));
    let repeated = table(
        &mut engine,
        &format!("CTABLES /MRSETS COUNTDUPLICATES=YES {source}"),
    );
    let sum = |a: &[f64; 6]| a.iter().sum::<f64>();
    for v in 0..6 {
        close(
            &numbers(&plain, v),
            &[count[v], once[v], 100.0 * once[v] / sum(&once)],
        );
        close(
            &numbers(&repeated, v),
            &[count[v], all[v], 100.0 * all[v] / sum(&all)],
        );
    }
    assert_eq!(numbers(&plain, 6)[1], Some(sum(&once)));
    assert_eq!(numbers(&repeated, 6)[1], Some(sum(&all)));
    assert_eq!(numbers(&plain, 6)[0], numbers(&repeated, 6)[0]);
}

#[test]
fn a_dichotomy_category_counts_like_its_variable_with_the_counted_value() {
    let mut engine = engine();
    let set = table(&mut engine, "CTABLES /TABLE $q5 BY region.");
    for (k, name) in Q5.iter().enumerate() {
        let single = table(
            &mut engine,
            &format!("CTABLES /TABLE {name} BY region /CATEGORIES VARIABLES={name} [1]."),
        );
        assert_eq!(numbers(&set, k), numbers(&single, 0), "{name}");
    }
}

#[test]
fn a_set_crossed_with_a_variable_agrees_with_a_count_of_the_raw_values() {
    let mut engine = engine();
    let t = table(
        &mut engine,
        "CTABLES /TABLE $q5 [COUNT, COLPCT.COUNT, ROWPCT.COUNT, COLPCT.RESPONSES, ROWPCT.TOTALN] BY gender\n\
         /CATEGORIES VARIABLES=$q5 gender TOTAL=YES.",
    );
    let data = engine.dataset().unwrap().clone();
    // [category or 5 for the total][gender 0..2 or 3 for the total]
    let (mut count, mut responses) = ([[0.0; 4]; 6], [[0.0; 4]; 6]);
    let mut all_cases = [0.0; 4];
    for case in 0..data.case_count() {
        let g = value(&data, "gender", case).unwrap() as usize - 1;
        let picked = chosen(&data, case);
        for column in [g, 3] {
            all_cases[column] += 1.0;
            for &k in &picked {
                count[k][column] += 1.0;
                responses[k][column] += 1.0;
                responses[5][column] += 1.0;
            }
            if !picked.is_empty() {
                count[5][column] += 1.0;
            }
        }
    }
    for row in 0..6 {
        let mut expected = Vec::new();
        for column in 0..4 {
            let n = count[row][column];
            // a percentage whose area a total decides stays empty, as for plain variables
            let column_pct = (column < 3).then(|| 100.0 * n / count[5][column]);
            let response_pct =
                (column < 3).then(|| 100.0 * responses[row][column] / responses[5][column]);
            // the set decides the row area, so a row holds only the cases of its category
            let row_pct = (row < 5).then(|| 100.0 * n / count[row][3]);
            expected.extend([Some(n), column_pct, row_pct, response_pct, row_pct]);
        }
        let actual = numbers(&t, row);
        assert_eq!(actual.len(), expected.len());
        for (a, e) in actual.iter().zip(&expected) {
            assert_eq!(
                a.is_some(),
                e.is_some(),
                "row {row}: {actual:?} vs {expected:?}"
            );
            close(&[*a].map(|a| a.or(Some(0.0))), &[e.unwrap_or(0.0)]);
        }
    }
    // COLPCT.TOTALN of the set's total row in a column is 100: every case of the column
    let t = table(
        &mut engine,
        "CTABLES /TABLE $q5 [COLPCT.TOTALN] BY gender /CATEGORIES VARIABLES=$q5 TOTAL=YES.",
    );
    let expected: Vec<f64> = (0..3).map(|g| 100.0 * count[0][g] / all_cases[g]).collect();
    close(&numbers(&t, 0), &expected);
    close(&numbers(&t, 5), &[100.0, 100.0, 100.0]);
}

#[test]
fn weights_weigh_cases_and_every_response_of_a_case() {
    // the second writer's file defines the sets and weights by `weight`
    let mut engine = Engine::default();
    let source = format!(
        "GET FILE='{}'.\nCTABLES /TABLE $q5 [COUNT, UCOUNT, RESPONSES, URESPONSES, COLPCT.COUNT]\n\
         /CATEGORIES VARIABLES=$q5 TOTAL=YES.",
        fixture("survey_basic_phpwriter.sav")
    );
    let t = run_on(&mut engine, &source).unwrap().remove(0);
    let data = engine.dataset().unwrap().clone();
    let (mut weighted, mut cases, mut any, mut any_cases, mut responses) =
        ([0.0; 5], [0.0; 5], 0.0, 0.0, 0.0);
    for case in 0..data.case_count() {
        let w = value(&data, "weight", case).unwrap();
        let picked = chosen(&data, case);
        for &k in &picked {
            weighted[k] += w;
            cases[k] += 1.0;
            responses += w;
        }
        if !picked.is_empty() {
            any += w;
            any_cases += 1.0;
        }
    }
    for k in 0..5 {
        close(
            &numbers(&t, k),
            &[
                weighted[k],
                cases[k],
                weighted[k],
                cases[k],
                100.0 * weighted[k] / any,
            ],
        );
    }
    close(&numbers(&t, 5), &[any, any_cases, responses, 482.0, 100.0]);
}

#[test]
fn sets_nest_and_stack_like_variables() {
    let mut engine = engine();
    let data = engine.dataset().unwrap().clone();
    // a set inside a variable: Female (2) and Email (q5_2)
    let t = table(&mut engine, "CTABLES /TABLE gender > $q5.");
    assert_eq!(t.row_headers, 4);
    assert_eq!(text(&t.rows[6][1]), "Female");
    let female_email = (0..data.case_count())
        .filter(|&c| value(&data, "gender", c) == Some(2.0) && chosen(&data, c).contains(&1))
        .count() as f64;
    close(&numbers(&t, 6), &[female_email]);

    // the set carries the summaries with a variable nested inside it
    let t = table(
        &mut engine,
        "CTABLES /TABLE $q5 [COUNT, RESPONSES] > gender /CATEGORIES VARIABLES=gender TOTAL=YES.",
    );
    assert_eq!(&t.columns[4..], ["Count", "Responses"]);
    // Email > Total of gender
    close(&numbers(&t, 7), &[80.0, 80.0]);

    // two sets crossed: cases with both the channel and the provider
    let t = table(&mut engine, "CTABLES /TABLE $q5 BY $q6.");
    let phone_norvale = (0..data.case_count())
        .filter(|&c| chosen(&data, c).contains(&0) && mentions(&data, c).contains(&1.0))
        .count() as f64;
    assert_eq!(numbers(&t, 0)[0], Some(phone_norvale));

    // stacked with a variable
    let t = table(&mut engine, "CTABLES /TABLE $q5 + gender.");
    assert_eq!(t.rows.len(), 8);
    assert_eq!(text(&t.rows[5][0]), "Gender");
    close(&numbers(&t, 4), &[131.0]);
}

#[test]
fn categories_of_a_dichotomy_set_are_its_variables_in_any_order() {
    let mut engine = engine();
    let t = table(
        &mut engine,
        "CTABLES /TABLE $q5 [COUNT, COLPCT.COUNT] /CATEGORIES VARIABLES=$q5 [q5_3, Q5_1] TOTAL=YES LABEL='Any' POSITION=BEFORE.",
    );
    assert_eq!(row_labels(&t, 1)[0], "Any");
    assert!(row_labels(&t, 1)[1].ends_with("Live chat"));
    assert!(row_labels(&t, 1)[2].ends_with("Phone"));
    // the base is the cases with a response in one of the two listed channels; a case whose
    // only responses are in left-out channels is not in the table
    let data = engine.dataset().unwrap().clone();
    let base = (0..data.case_count())
        .filter(|&c| chosen(&data, c).iter().any(|k| [0, 2].contains(k)))
        .count() as f64;
    close(&numbers(&t, 0), &[base, 100.0]);
    close(&numbers(&t, 1), &[101.0, 100.0 * 101.0 / base]);

    let t = table(
        &mut engine,
        "CTABLES /TABLE $q5 /CATEGORIES VARIABLES=$q5 ORDER=D.",
    );
    assert!(row_labels(&t, 1)[0].ends_with("In-store visit"));
    let t = table(
        &mut engine,
        "CTABLES /TABLE $q5 /CATEGORIES VARIABLES=$q5 KEY=LABEL.",
    );
    assert!(row_labels(&t, 1)[0].ends_with("Email"));

    // EMPTY=EXCLUDE leaves out a variable that never holds the counted value
    run_on(
        &mut engine,
        "COMPUTE never = 0.\nMRSETS /MDGROUP NAME=$e VARIABLES=q5_1 never VALUE=1.",
    )
    .unwrap();
    assert_eq!(table(&mut engine, "CTABLES /TABLE $e.").rows.len(), 2);
    let t = table(
        &mut engine,
        "CTABLES /TABLE $e /CATEGORIES VARIABLES=$e EMPTY=EXCLUDE.",
    );
    assert_eq!(t.rows.len(), 1);
}

#[test]
fn missing_values_of_a_category_set() {
    let mut engine = engine();
    run_on(
        &mut engine,
        "MISSING VALUES q6_mention1 TO q6_mention3 (6).",
    )
    .unwrap();
    let data = engine.dataset().unwrap().clone();
    let (mut valid, mut any, mut none) = (0.0, 0.0, 0.0);
    for case in 0..data.case_count() {
        let given = mentions(&data, case);
        if given.iter().any(|v| *v != 6.0) {
            valid += 1.0;
        }
        if given.is_empty() || given.iter().all(|v| *v == 6.0) {
            none += 1.0;
        }
        if !given.is_empty() {
            any += 1.0;
        }
    }
    assert_eq!(valid + none, 200.0);
    let functions = "[COUNT, COLPCT.COUNT, COLPCT.VALIDN, COLPCT.TOTALN]";
    // excluded: a case that only named the missing value counts in TOTALN only
    let t = table(
        &mut engine,
        &format!("CTABLES /TABLE $q6 {functions} /CATEGORIES VARIABLES=$q6 TOTAL=YES."),
    );
    assert_eq!(t.rows.len(), 6);
    close(
        &numbers(&t, 0),
        &[58.0, 5800.0 / valid, 5800.0 / valid, 5800.0 / 200.0],
    );
    close(&numbers(&t, 5), &[valid, 100.0, 100.0, 100.0]);
    // included: the missing value is a category; it counts, but is not valid
    let t = table(
        &mut engine,
        &format!(
            "CTABLES /TABLE $q6 {functions} /CATEGORIES VARIABLES=$q6 MISSING=INCLUDE TOTAL=YES."
        ),
    );
    assert_eq!(text(&t.rows[5][1]), "Other provider");
    close(
        &numbers(&t, 0),
        &[58.0, 5800.0 / any, 5800.0 / valid, 5800.0 / 200.0],
    );
    close(&numbers(&t, 5), &[52.0, 5200.0 / any, 0.0, 5200.0 / 200.0]);
    close(&numbers(&t, 6), &[any, 100.0, 100.0, 100.0]);
}

#[test]
fn labels_of_a_set_follow_vlabels() {
    let mut engine = engine();
    let t = table(
        &mut engine,
        "CTABLES /VLABELS VARIABLES=$q6 DISPLAY=BOTH /TABLE $q6 + $q5.",
    );
    assert_eq!(text(&t.rows[0][0]), "$q6 Q6. Other providers mentioned");
    assert_eq!(text(&t.rows[6][0]), "Q5. Contact channels used");
    let t = table(
        &mut engine,
        "CTABLES /VLABELS VARIABLES=ALL DISPLAY=NAME /TABLE $q6.",
    );
    assert_eq!(text(&t.rows[0][0]), "$q6");
}

#[test]
fn mistakes_with_sets_name_the_rule() {
    let mut engine = engine();
    for (source, message) in [
        (
            "CTABLES /TABLE gender [RESPONSES].",
            "/TABLE: summary function RESPONSES applies only to multiple response sets, and gender is not one",
        ),
        (
            "CTABLES /TABLE $q5 BY gender [COLPCT.RESPONSES].",
            "/TABLE: summary function COLPCT.RESPONSES applies only to multiple response sets, and gender is not one",
        ),
        (
            "CTABLES /TABLE $q5 [MEAN].",
            "/TABLE: summary function MEAN applies only to scale variables, and $q5 is a multiple response set",
        ),
        (
            "CTABLES /TABLE $q5 [S].",
            "/TABLE: $q5 is a multiple response set, so it cannot be a scale variable; remove [S]",
        ),
        (
            "CTABLES /TABLE $q5 [COUNT] > $q6.",
            "/TABLE: summaries may follow only the innermost categorical variable of a nesting, or a multiple response set with no other set or summaries inside it, and $q5 is an outer one",
        ),
        (
            "CTABLES /TABLE $nope.",
            "/TABLE: unknown multiple response set $nope (define it with MRSETS)",
        ),
        (
            "CTABLES /TABLE $q6 /CATEGORIES VARIABLES=$q6 [q6_mention1].",
            "/CATEGORIES for $q6: a category set lists values; only a dichotomy set lists its variables by name",
        ),
        (
            "CTABLES /TABLE $q5 /CATEGORIES VARIABLES=$q5 [1].",
            "/CATEGORIES for $q5: the categories of a dichotomy set are its variables, so list them by name",
        ),
        (
            "CTABLES /TABLE $q5 /CATEGORIES VARIABLES=$q5 [q6_mention1].",
            "/CATEGORIES for $q5: q6_mention1 is not a variable of the set",
        ),
        (
            "CTABLES /TABLE gender /CATEGORIES VARIABLES=gender [male].",
            "/CATEGORIES for gender: male is a name, but only a multiple dichotomy set lists its categories by name",
        ),
        (
            "CTABLES /TABLE gender /CATEGORIES VARIABLES=$nope TOTAL=YES.",
            "/CATEGORIES: unknown multiple response set $nope",
        ),
    ] {
        assert_eq!(error(&mut engine, source), message, "{source}");
    }
}
