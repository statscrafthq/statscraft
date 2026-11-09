//! DESCRIPTIVES, CROSSTABS, ADD FILES and MATCH FILES on the fixtures. The expected numbers
//! were printed by the reference program with ten decimals; `reference_procedures.rs` repeats
//! the comparison on many more cases when that program is at hand.

use statscraft_data::Dataset;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn fixture(name: &str) -> String {
    format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn run(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

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

fn survey() -> Engine {
    loaded("survey_basic.sav")
}

/// The survey with `weight` as the weight variable.
fn weighted() -> Engine {
    loaded("survey_basic_phpwriter.sav")
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

/// Every number of `row` (from column `from` on) against the expected values, which the
/// reference program printed with ten decimals; None is an empty cell.
fn assert_row(row: &[Cell], from: usize, expected: &[Option<f64>]) {
    assert_eq!(row.len() - from, expected.len(), "{row:?}");
    for (cell, want) in row[from..].iter().zip(expected) {
        match want {
            Some(want) => {
                let got = number(cell);
                assert!(
                    (got - want).abs() <= 6e-11 * want.abs().max(1.0),
                    "{got} vs {want} in {row:?}"
                );
            }
            None => assert_eq!(*cell, Cell::Empty, "{row:?}"),
        }
    }
}

fn texts(row: &[Cell]) -> Vec<String> {
    row.iter()
        .map(|cell| match cell {
            Cell::Text(t) => t.clone(),
            _ => String::new(),
        })
        .collect()
}

#[test]
fn descriptives_all_statistics() {
    let tables = run(
        &mut survey(),
        "DESCRIPTIVES VARIABLES=age monthly_spend q1_satisfaction /STATISTICS=ALL.",
    );
    let table = &tables[0];
    assert_eq!(table.title, "Descriptive Statistics");
    assert_eq!(table.row_headers, 2);
    assert_eq!(
        table.columns,
        [
            "Variable",
            "Label",
            "N",
            "Mean",
            "S.E. Mean",
            "Std Dev",
            "Variance",
            "Kurtosis",
            "S.E. Kurt",
            "Skewness",
            "S.E. Skew",
            "Range",
            "Minimum",
            "Maximum",
            "Sum"
        ]
    );
    assert_eq!(texts(&table.rows[0])[..2], ["age", "Age in years"]);
    assert_row(
        &table.rows[0],
        2,
        &[
            Some(200.0),
            Some(50.845),
            Some(1.3595575969),
            Some(19.2270479237),
            Some(369.6793718593),
            Some(-1.2109488364),
            Some(0.3422023892),
            Some(0.0063257326),
            Some(0.1719247963),
            Some(66.0),
            Some(18.0),
            Some(84.0),
            Some(10169.0),
        ],
    );
    // user-missing 98 and 99 are left out
    assert_row(
        &table.rows[2],
        2,
        &[
            Some(179.0),
            Some(3.5642458101),
            Some(0.0823382422),
            Some(1.1016106012),
            Some(1.2135459168),
            Some(-0.4556467221),
            Some(0.3612139930),
            Some(-0.4964096789),
            Some(0.1815739587),
            Some(4.0),
            Some(1.0),
            Some(5.0),
            Some(638.0),
        ],
    );
    // also with MISSING=VARIABLE the row counts the cases valid on all three variables
    assert_eq!(texts(&table.rows[3])[0], "Valid N (listwise)");
    assert_eq!(number(&table.rows[3][2]), 161.0);
    assert_eq!(texts(&table.rows[4])[0], "Missing N (listwise)");
    assert_eq!(number(&table.rows[4][2]), 39.0);
    assert_eq!(table.rows[4][3], Cell::Empty);
}

#[test]
fn descriptives_weighted_and_listwise() {
    let tables = run(
        &mut weighted(),
        "DESCRIPTIVES age monthly_spend q1_satisfaction /STATISTICS=ALL.
DESCRIPTIVES age monthly_spend q1_satisfaction /MISSING=LISTWISE.",
    );
    assert_row(
        &tables[0].rows[1],
        2,
        &[
            Some(233.822),
            Some(184.5153862340),
            Some(7.4578892929),
            Some(114.0403700140),
            Some(13005.2059929220),
            Some(-1.1812897062),
            Some(0.3170357301),
            Some(0.1980371621),
            Some(0.1591741652),
            Some(387.82),
            Some(5.87),
            Some(393.69),
            Some(43143.75664),
        ],
    );
    let listwise = &tables[1];
    assert_eq!(
        listwise.columns[2..],
        ["N", "Mean", "Std Dev", "Minimum", "Maximum"]
    );
    assert_row(
        &listwise.rows[0],
        2,
        &[
            Some(206.6982),
            Some(49.9815687800),
            Some(19.6101878776),
            Some(18.0),
            Some(84.0),
        ],
    );
    assert!((number(&listwise.rows[3][2]) - 206.6982).abs() < 1e-9);
    assert!((number(&listwise.rows[4][2]) - 53.2359).abs() < 1e-9);
}

#[test]
fn descriptives_edge_cases() {
    let tables = run(
        &mut loaded("edge_cases.sav"),
        "DESCRIPTIVES all_missing three_missing /STATISTICS=MEAN STDDEV SUM KURTOSIS.",
    );
    let rows = &tables[0].rows;
    // nothing valid: N and Sum are 0, the rest is empty
    assert_row(&rows[0], 2, &[Some(0.0), None, None, None, None, Some(0.0)]);
    assert_row(
        &rows[1],
        2,
        &[
            Some(8.0),
            Some(2.625),
            Some(1.4078859532),
            Some(-0.5644996348),
            Some(1.4808804790),
            Some(21.0),
        ],
    );
    // all_missing has no valid value, so no case is valid on both variables
    assert_eq!(number(&rows[2][2]), 0.0);
    assert_eq!(number(&rows[3][2]), 12.0);

    // without spread there is no skewness or kurtosis, and no standard error of either
    let tables = run(
        &mut survey(),
        "COMPUTE same = 4.\nDESCRIPTIVES same /STATISTICS=SKEWNESS KURTOSIS.",
    );
    assert_row(
        &tables[0].rows[0],
        2,
        &[Some(200.0), None, None, None, None],
    );

    // a string variable gets no statistics; the caption names it
    let tables = run(&mut survey(), "DESCRIPTIVES age plan.");
    assert_eq!(texts(&tables[0].rows[0])[0], "age");
    assert_eq!(texts(&tables[0].rows[1])[0], "Valid N (listwise)");
    assert_eq!(
        tables[0].caption.as_deref(),
        Some("No statistics for string variables: plan.")
    );
}

#[test]
fn descriptives_save_z_scores() {
    let mut engine = survey();
    run(
        &mut engine,
        "RENAME VARIABLES (resp_id = Zage).
DESCRIPTIVES age monthly_spend (spend_z) q1_satisfaction /SAVE.
DESCRIPTIVES age /SAVE.",
    );
    let data = engine.dataset().unwrap();
    let names: Vec<&str> = data.variable_names().collect();
    assert_eq!(
        names[19..],
        ["ZSC001", "spend_z", "Zq1_satisfaction", "ZSC002"]
    );
    // Zage is taken, so age gets the first name of the other series and its name in the label
    let z = data.variable("ZSC001").unwrap();
    assert_eq!(z.label(), Some("Zscore(age) Age in years"));
    let label = |name: &str| data.variable(name).unwrap().label().map(str::to_string);
    let q1_label = label("q1_satisfaction").unwrap();
    assert_eq!(
        label("Zq1_satisfaction"),
        Some(format!("Zscore: {q1_label}"))
    );
    assert_eq!(z.format(), "F8.2");
    let values: Vec<Option<f64>> = z.numeric().unwrap().iter().collect();
    assert!((values[0].unwrap() - (49.0 - 50.845) / 19.2270479237).abs() < 1e-9);
    let q1: Vec<Option<f64>> = data
        .variable("Zq1_satisfaction")
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect();
    // case 8 answered 99, which is user-missing
    assert_eq!(q1[7], None);
    assert!((q1[0].unwrap() - (3.0 - 3.5642458101) / 1.1016106012).abs() < 1e-9);

    // without SAVE only the variables with a name in parentheses get a z-score
    let mut engine = survey();
    run(&mut engine, "DESCRIPTIVES age (age_z) monthly_spend.");
    let data = engine.dataset().unwrap();
    let names: Vec<&str> = data.variable_names().collect();
    assert_eq!(names[19..], ["age_z"]);
    let z = data.variable("age_z").unwrap();
    assert_eq!(z.label(), Some("Zscore: Age in years"));
    let first = z.numeric().unwrap().get(0).unwrap();
    assert!((first - (49.0 - 50.845) / 19.2270479237).abs() < 1e-9);

    // the numbered names run ZSC001 to ZSC099, then STDZ01 to STDZ09
    let mut engine = survey();
    let mut taken = String::from("NUMERIC Zage");
    for i in 1..100 {
        taken.push_str(&format!(" ZSC{i:03}"));
    }
    run(&mut engine, &format!("{taken}.\nDESCRIPTIVES age /SAVE."));
    assert!(engine.dataset().unwrap().variable("STDZ01").is_ok());
}

#[test]
fn descriptives_errors() {
    let mut engine = survey();
    assert_eq!(
        error(&mut engine, "DESCRIPTIVES plan."),
        "DESCRIPTIVES needs a numeric variable, but plan is a string variable"
    );
    assert_eq!(
        error(&mut engine, "DESCRIPTIVES age (gender) /SAVE."),
        "the z-score name gender is already in use; choose another name"
    );
    assert_eq!(
        error(&mut engine, "DESCRIPTIVES agee."),
        "unknown variable agee"
    );
    assert_eq!(engine.dataset().unwrap().variable_count(), 19);
}

#[test]
fn crosstabs_cells_and_chi_square() {
    let tables = run(
        &mut survey(),
        "CROSSTABS /TABLES=gender BY q5_1 /CELLS=COUNT ROW EXPECTED ASRESID /STATISTICS=CHISQ.",
    );
    let [table, tests] = &tables[..] else {
        panic!("{tables:?}")
    };
    assert_eq!(table.title, "gender by q5_1");
    assert_eq!(table.row_headers, 2);
    assert_eq!(
        table.column_titles(),
        [
            "gender",
            "Statistic",
            "q5_1 / Not selected",
            "q5_1 / Selected",
            "Total"
        ]
    );
    assert_eq!(texts(&table.rows[0])[..2], ["Male", "Count"]);
    assert_row(&table.rows[0], 2, &[Some(78.0), Some(28.0), Some(106.0)]);
    assert_row(
        &table.rows[1],
        2,
        &[Some(7800.0 / 106.0), Some(2800.0 / 106.0), Some(100.0)],
    );
    // in the margins the expected counts add up to the observed count
    assert_row(&table.rows[2], 2, &[Some(75.26), Some(30.74), Some(106.0)]);
    let adjusted = 2.74 / (75.26_f64 * (1.0 - 0.53) * (1.0 - 0.71)).sqrt();
    assert_row(&table.rows[3], 2, &[Some(adjusted), Some(-adjusted), None]);
    // the Total row has no residual line
    let last: Vec<String> = table.rows.iter().map(|r| texts(r)[1].clone()).collect();
    assert_eq!(last[last.len() - 3..], ["Count", "Row %", "Expected"]);
    assert_row(
        &table.rows[last.len() - 3],
        2,
        &[Some(142.0), Some(58.0), Some(200.0)],
    );
    assert_row(
        &table.rows[last.len() - 1],
        2,
        &[Some(142.0), Some(58.0), Some(200.0)],
    );

    assert_eq!(tests.title, "Chi-Square Tests: gender by q5_1");
    assert_eq!(
        tests.columns,
        ["Statistic", "Value", "df", "Asymptotic Sig. (2-tailed)"]
    );
    let names: Vec<String> = tests.rows.iter().map(|r| texts(r)[0].clone()).collect();
    assert_eq!(
        names,
        [
            "Pearson Chi-Square",
            "Likelihood Ratio",
            "Linear-by-Linear Association",
            "N of Valid Cases"
        ]
    );
    assert_row(&tests.rows[0][..3], 1, &[Some(1.7499627207), Some(2.0)]);
    assert!((number(&tests.rows[0][3]) - 0.417).abs() < 5e-4);
    assert_row(&tests.rows[1][..3], 1, &[Some(1.6343712972), Some(2.0)]);
    assert_row(&tests.rows[2][..3], 1, &[Some(1.2470820473), Some(1.0)]);
    assert_row(&tests.rows[3], 1, &[Some(200.0), None, None]);
}

#[test]
fn crosstabs_two_by_two_weighted() {
    let tables = run(
        &mut weighted(),
        "CROSSTABS q5_2 BY q5_1 /CELLS=COUNT EXPECTED /STATISTICS=CHISQ.",
    );
    // weighted counts are not rounded
    assert_row(
        &tables[0].rows[0],
        2,
        &[Some(112.0302), Some(43.367), Some(155.3972)],
    );
    assert_row(
        &tables[0].rows[1],
        2,
        &[Some(110.1751938106), Some(45.2220061894), Some(155.3972)],
    );
    let tests = &tables[1];
    assert_eq!(tests.columns.len(), 6);
    let row = |name: &str| {
        tests
            .rows
            .iter()
            .find(|r| texts(r)[0] == name)
            .unwrap_or_else(|| panic!("{name}"))
    };
    assert_row(
        &row("Pearson Chi-Square")[..3],
        1,
        &[Some(0.2668664061), Some(1.0)],
    );
    assert_row(
        &row("Likelihood Ratio")[..3],
        1,
        &[Some(0.2659156680), Some(1.0)],
    );
    assert_row(
        &row("Continuity Correction")[..3],
        1,
        &[Some(0.1423920488), Some(1.0)],
    );
    assert_row(
        &row("Linear-by-Linear Association")[..3],
        1,
        &[Some(0.2658397366), Some(1.0)],
    );
    // Fisher's test rounds the weighted counts: [[112, 43], [72, 32]]
    let fisher = row("Fisher's Exact Test");
    assert!((number(&fisher[4]) - 0.6752983930639885).abs() < 1e-9);
    assert!((number(&fisher[5]) - 0.348299045751045).abs() < 1e-9);
    assert_eq!(fisher[1], Cell::Empty);
    assert_row(&row("N of Valid Cases")[..2], 1, &[Some(259.9341)]);
}

#[test]
fn crosstabs_layers_and_several_tables() {
    let mut engine = survey();
    let tables = run(
        &mut engine,
        "SELECT IF resp_id <= 1012.\nCROSSTABS q5_1 BY gender BY q5_2 /STATISTICS=CHISQ.",
    );
    let table = &tables[0];
    assert_eq!(table.row_headers, 3);
    assert_eq!(
        table.columns,
        [
            "q5_2",
            "q5_1",
            "Statistic",
            "Male",
            "Female",
            "Other or prefer to self-describe",
            "Total"
        ]
    );
    assert_eq!(table.column_groups[3], ["gender"]);
    assert!(table.column_groups[6].is_empty());
    // in the first layer nobody is in the third column: its cells stay blank
    assert_eq!(
        texts(&table.rows[0])[..3],
        ["Not selected", "Not selected", "Count"]
    );
    assert_row(&table.rows[0], 3, &[Some(1.0), Some(3.0), None, Some(4.0)]);
    assert_eq!(texts(&table.rows[3])[..2], ["Selected", "Not selected"]);
    assert_row(
        &table.rows[3],
        3,
        &[Some(2.0), Some(3.0), Some(0.0), Some(5.0)],
    );
    // the 2 x 2 layer gets Fisher's test, the 2 x 3 layer does not
    let names: Vec<String> = tables[1].rows.iter().map(|r| texts(r)[1].clone()).collect();
    assert_eq!(
        names.iter().filter(|n| *n == "Fisher's Exact Test").count(),
        1
    );

    let tables = run(
        &mut engine,
        "CROSSTABS /TABLES=gender q5_1 BY region q5_2 /TABLES=plan BY q5_3 /CELLS=NONE.",
    );
    assert!(tables.is_empty());
    let tables = run(
        &mut engine,
        "CROSSTABS /TABLES=gender q5_1 BY region q5_2 /FORMAT=DVALUE.",
    );
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "gender by region",
            "gender by q5_2",
            "q5_1 by region",
            "q5_1 by q5_2"
        ]
    );
    // DVALUE turns the rows round and leaves the columns ascending
    assert_eq!(
        texts(&tables[1].rows[0])[0],
        "Other or prefer to self-describe"
    );
    assert_eq!(tables[1].columns[2..4], ["Not selected", "Selected"]);

    // CELLS alone shows the count and the three percentages, STATISTICS alone the chi-square
    let tables = run(&mut engine, "CROSSTABS gender BY q5_1 /CELLS /STATISTICS.");
    let statistics: Vec<String> = tables[0].rows[..4]
        .iter()
        .map(|r| texts(r)[1].clone())
        .collect();
    assert_eq!(statistics, ["Count", "Row %", "Column %", "Total %"]);
    assert_eq!(tables[1].title, "Chi-Square Tests: gender by q5_1");
}

#[test]
fn crosstabs_missing_values() {
    let mut engine = survey();
    let table = |engine: &mut Engine, syntax: &str| run(engine, syntax).remove(0);
    let excluded = table(&mut engine, "CROSSTABS q1_satisfaction BY q6_mention2.");
    let total = excluded.rows.last().unwrap();
    assert_eq!(number(total.last().unwrap()), 97.0);
    let included = table(
        &mut engine,
        "CROSSTABS q1_satisfaction BY q6_mention2 /MISSING=INCLUDE.",
    );
    assert_eq!(number(included.rows.last().unwrap().last().unwrap()), 107.0);
    assert_eq!(texts(&included.rows[6])[0], "Refused");
}

#[test]
fn add_files() {
    let dir = std::env::temp_dir().join(format!("statscraft-add-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let b = dir.join("b.sav");
    let mut engine = survey();
    run(
        &mut engine,
        &format!(
            "SELECT IF resp_id > 1195.
VARIABLE LABELS age 'Age, second file' gender ''.
STRING plan_long (A30).
COMPUTE plan_long = CONCAT(RTRIM(plan), ' plan').
DELETE VARIABLES q3_comment.
SAVE OUTFILE='{}'.
GET FILE='{}'.
SELECT IF resp_id <= 1003.
VARIABLE LABELS gender ''.
ADD FILES /FILE=* /FILE='{}' /KEEP=resp_id age gender q3_comment plan_long.",
            b.display(),
            fixture("survey_basic_phpwriter.sav"),
            b.display()
        ),
    );
    let data = engine.dataset().unwrap();
    assert_eq!(data.case_count(), 8);
    assert_eq!(data.weight_variable(), None);
    let ids: Vec<Option<f64>> = data
        .variable("resp_id")
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect();
    assert_eq!(ids[2..4], [Some(1003.0), Some(1196.0)]);
    // the first file's label stays; a label the first file lacks comes from a later one
    assert_eq!(data.variable("age").unwrap().label(), Some("Age in years"));
    assert_eq!(data.variable("gender").unwrap().label(), None);
    let comment = data.variable("q3_comment").unwrap();
    assert_eq!(comment.string().unwrap().get(7), "");
    let long = data.variable("plan_long").unwrap();
    assert_eq!(long.string().unwrap().get(0), "");
    assert!(long.string().unwrap().get(3).ends_with(" plan"));

    run(
        &mut engine,
        &format!(
            "GET FILE='{}'.\nSTRING age2 (A3).\nRENAME VARIABLES (age age2 = n age).\nSAVE OUTFILE='{}'.",
            b.display(),
            b.display()
        ),
    );
    let mut engine = survey();
    assert_eq!(
        error(
            &mut engine,
            &format!("ADD FILES /FILE=* /FILE='{}'.", b.display())
        ),
        format!(
            "age is numeric in FILE=* but a string in FILE='{}'; rename one of them with /RENAME or change its type",
            b.display()
        )
    );
    assert_eq!(
        error(&mut engine, "ADD FILES /FILE=* /FILE=* /BY region."),
        "FILE=* is not sorted by region: case 4 has a lower key than case 3; sort the active dataset with SORT CASES BY region first"
    );

    // files weighted by the same variable stay weighted; one unweighted file turns it off
    let mut engine = weighted();
    run(&mut engine, "ADD FILES /FILE=* /FILE=*.");
    assert_eq!(engine.dataset().unwrap().weight_variable(), Some("weight"));
    run(
        &mut engine,
        &format!("ADD FILES /FILE=* /FILE='{}'.", fixture("survey_basic.sav")),
    );
    assert_eq!(engine.dataset().unwrap().weight_variable(), None);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn match_files() {
    let dir = std::env::temp_dir().join(format!("statscraft-match-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = |name: &str| dir.join(name).display().to_string();
    let mut engine = survey();
    run(
        &mut engine,
        &format!(
            "AGGREGATE /OUTFILE='{}' /BREAK=region /n = N.
SELECT IF resp_id >= 1002 AND resp_id <= 1005.
COMPUTE age = age + 1000.
COMPUTE extra = resp_id * 2.
SAVE OUTFILE='{}'.
GET FILE='{}'.
SELECT IF resp_id <= 1003 OR resp_id = 1007.
MATCH FILES /FILE=* /FILE='{}' /BY resp_id /KEEP=resp_id age extra.",
            path("lookup.sav"),
            path("b.sav"),
            fixture("survey_basic.sav"),
            path("b.sav")
        ),
    );
    let column = |engine: &Engine, name: &str| -> Vec<Option<f64>> {
        let data = engine.dataset().unwrap();
        data.variable(name)
            .unwrap()
            .numeric()
            .unwrap()
            .iter()
            .collect()
    };
    assert_eq!(
        column(&engine, "resp_id"),
        [1001.0, 1002.0, 1003.0, 1004.0, 1005.0, 1007.0].map(Some)
    );
    // a shared variable takes its value from the first file that has the case
    assert_eq!(
        column(&engine, "age"),
        [49.0, 59.0, 75.0, 1036.0, 1068.0, 25.0].map(Some)
    );
    assert_eq!(
        column(&engine, "extra"),
        [
            None,
            Some(2004.0),
            Some(2006.0),
            Some(2008.0),
            Some(2010.0),
            None
        ]
    );

    let mut engine = survey();
    run(
        &mut engine,
        &format!(
            "SELECT IF resp_id <= 1006.\nSORT CASES BY region.\nMATCH FILES /FILE=* /TABLE='{}' /BY region.",
            path("lookup.sav")
        ),
    );
    assert_eq!(
        column(&engine, "region"),
        [2.0, 2.0, 3.0, 3.0, 4.0, 4.0].map(Some)
    );
    assert_eq!(
        column(&engine, "n"),
        [47.0, 47.0, 30.0, 30.0, 43.0, 43.0].map(Some)
    );

    let mut engine = survey();
    assert_eq!(
        error(
            &mut engine,
            &format!(
                "MATCH FILES /FILE=* /TABLE='{}' /BY region.",
                path("lookup.sav")
            )
        ),
        "FILE=* is not sorted by region: case 4 has a lower key than case 3; sort the active dataset with SORT CASES BY region first"
    );
    run(&mut engine, "SORT CASES BY gender.");
    assert_eq!(
        error(
            &mut engine,
            &format!(
                "MATCH FILES /FILE=* /TABLE='{}' /BY gender.",
                path("lookup.sav")
            )
        ),
        format!(
            "the BY variable gender is not in TABLE='{}'; every file needs the key variables",
            path("lookup.sav")
        )
    );

    // a table may not hold a key twice
    let mut engine = survey();
    run(
        &mut engine,
        &format!(
            "GET FILE='{}'.\nADD FILES /FILE=* /FILE=* /BY region.\nSAVE OUTFILE='{}'.
GET FILE='{}'.\nSORT CASES BY region.",
            path("lookup.sav"),
            path("twice.sav"),
            fixture("survey_basic.sav")
        ),
    );
    assert_eq!(
        error(
            &mut engine,
            &format!(
                "MATCH FILES /FILE=* /TABLE='{}' /BY region.",
                path("twice.sav")
            )
        ),
        format!(
            "TABLE='{}' holds the same key twice (cases 1 and 2); a table may hold each key once",
            path("twice.sav")
        )
    );

    // without BY the n-th cases meet; the longer file decides the case count
    let mut engine = survey();
    run(
        &mut engine,
        &format!(
            "MATCH FILES /FILE='{}' /RENAME=(resp_id = id) /FILE=* /KEEP=id resp_id.",
            path("b.sav")
        ),
    );
    let data = engine.dataset().unwrap();
    assert_eq!(data.case_count(), 200);
    assert_eq!(
        column(&engine, "id")[..5],
        [Some(1002.0), Some(1003.0), Some(1004.0), Some(1005.0), None]
    );
    let _ = Dataset::read_sav(path("b.sav")).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}
