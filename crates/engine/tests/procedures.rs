//! DESCRIPTIVES, CROSSTABS, ADD FILES and MATCH FILES on the fixtures. The expected numbers
//! were printed by the reference program with ten decimals; `reference_procedures.rs` repeats
//! the comparison on many more cases when that program is at hand.

use statscraft_data::Dataset;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn fixture(name: &str) -> String {
    format!("{}/../../tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The tables of a CROSSTABS run after its Case Processing Summary, which comes first.
fn crosstabs(engine: &mut Engine, source: &str) -> Vec<Table> {
    let mut tables = run(engine, source);
    assert_eq!(tables[0].title, "Case Processing Summary");
    tables.remove(0);
    tables
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

    // a string variable gets no statistics, and a warning names it
    let mut engine = survey();
    let tables = run(&mut engine, "DESCRIPTIVES age plan.");
    assert_eq!(texts(&tables[0].rows[0])[0], "age");
    assert_eq!(texts(&tables[0].rows[1])[0], "Valid N (listwise)");
    assert_eq!(tables[0].caption, None);
    assert_eq!(
        engine.warnings(),
        ["no statistics for string variables: plan"]
    );
    // the next command starts without warnings
    run(&mut engine, "DESCRIPTIVES age.");
    assert!(engine.warnings().is_empty());
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
    let tables = crosstabs(
        &mut survey(),
        // names, as the expectations below give them
        "SET TVARS=NAMES OVARS=NAMES.\n\
         CROSSTABS /TABLES=gender BY q5_1 /CELLS=COUNT ROW EXPECTED ASRESID /STATISTICS=CHISQ.",
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
    let tables = crosstabs(
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
    let tables = crosstabs(
        &mut engine,
        "SET TVARS=NAMES OVARS=NAMES.\nSELECT IF resp_id <= 1012.\n\
         CROSSTABS q5_1 BY gender BY q5_2 /STATISTICS=CHISQ.",
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

    let tables = crosstabs(
        &mut engine,
        "CROSSTABS /TABLES=gender q5_1 BY region q5_2 /TABLES=plan BY q5_3 /CELLS=NONE.",
    );
    assert!(tables.is_empty());
    let tables = crosstabs(
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
    let tables = crosstabs(&mut engine, "CROSSTABS gender BY q5_1 /CELLS /STATISTICS.");
    let statistics: Vec<String> = tables[0].rows[..4]
        .iter()
        .map(|r| texts(r)[1].clone())
        .collect();
    assert_eq!(statistics, ["Count", "Row %", "Column %", "Total %"]);
    assert_eq!(tables[1].title, "Chi-Square Tests: gender by q5_1");
}

/// The Case Processing Summary comes first, one row per table: weighted counts of the cases
/// valid on every variable and of those missing on one, cases without a positive weight left
/// out. By hand: valid 2 + 1 = 3, missing 1 + 3 = 4, total 7.
#[test]
fn crosstabs_case_processing_summary() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        "DATA LIST FREE /r c w.\nBEGIN DATA\n1 1 2\n1 2 1\n2 . 1\n. 1 3\n2 2 0\n1 1 -1\n\
         END DATA.\nWEIGHT BY w.\nCROSSTABS /TABLES=r BY c /TABLES=c BY r BY w.",
    );
    let summary = &tables[0];
    assert_eq!(summary.title, "Case Processing Summary");
    assert_eq!(
        summary.columns,
        ["", "N", "Percent", "N", "Percent", "N", "Percent"]
    );
    assert_eq!(summary.column_groups[1], ["Cases", "Valid"]);
    let labels: Vec<String> = summary.rows.iter().map(|r| texts(r)[0].clone()).collect();
    assert_eq!(labels, ["r * c", "c * r * w"]);
    for row in &summary.rows {
        assert_row(
            row,
            1,
            &[
                Some(3.0),
                Some(300.0 / 7.0),
                Some(4.0),
                Some(400.0 / 7.0),
                Some(7.0),
                Some(100.0),
            ],
        );
    }
    // one summary for the command, before every crosstabulation
    assert_eq!(
        tables.iter().filter(|t| t.title == summary.title).count(),
        1
    );
}

/// Integer mode: every whole value of a row or column range is a category, empty or not; a
/// fraction is cut off (2.7 counts as 2); a value outside its range, or user-missing, makes the
/// case missing; TO on TABLES follows the order of VARIABLES (q before p), not the file's.
/// Cases (p, q, weight): (1,2,2) (2,1,1) (2.7,1,1) (3,1,1) (1,1,1) (6,1,1) (2,3,1) (0,2,1)
/// (4,9,1). For q by p the first five are valid (weight 6): q 1 by p = [1, 2, 1, 0, 0] and
/// q 2 by p = [2, 0, 0, 0, 0]. For p by p the seventh and the last are valid too (weight 8).
#[test]
fn crosstabs_integer_mode() {
    let mut engine = Engine::default();
    let tables = run(
        &mut engine,
        "DATA LIST FREE /p q w.\nBEGIN DATA\n1 2 2\n2 1 1\n2.7 1 1\n3 1 1\n1 1 1\n\
         6 1 1\n2 3 1\n0 2 1\n4 9 1\nEND DATA.\nMISSING VALUES q (3).\nWEIGHT BY w.\n\
         CROSSTABS VARIABLES=q p (1,5) /TABLES=q TO p BY p /CELLS=COUNT COLUMN /STATISTICS=CHISQ.",
    );
    let titled = |title: &str| tables.iter().find(|t| t.title == title).unwrap();
    let summary = titled("Case Processing Summary");
    let labels: Vec<String> = summary.rows.iter().map(|r| texts(r)[0].clone()).collect();
    assert_eq!(labels, ["q * p", "p * p"]);
    let shares = |valid: f64| {
        [
            Some(valid),
            Some(valid * 10.0),
            Some(10.0 - valid),
            Some(100.0 - valid * 10.0),
            Some(10.0),
            Some(100.0),
        ]
    };
    assert_row(&summary.rows[0], 1, &shares(6.0));
    assert_row(&summary.rows[1], 1, &shares(8.0));

    let q = titled("q by p");
    assert_eq!(q.columns[2..], ["1", "2", "3", "4", "5", "Total"]);
    let labels: Vec<String> = q
        .rows
        .iter()
        .step_by(2)
        .map(|r| texts(r)[0].clone())
        .collect();
    assert_eq!(labels, ["1", "2", "3", "4", "5", "Total"]);
    assert_row(
        &q.rows[0],
        2,
        &[
            Some(1.0),
            Some(2.0),
            Some(1.0),
            Some(0.0),
            Some(0.0),
            Some(4.0),
        ],
    );
    // a column without cases has no percentage
    let third = 100.0 / 3.0;
    assert_row(
        &q.rows[1],
        2,
        &[
            Some(third),
            Some(100.0),
            Some(100.0),
            None,
            None,
            Some(2.0 * third),
        ],
    );
    assert_row(
        &q.rows[4],
        2,
        &[
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(0.0),
        ],
    );
    assert_row(
        &q.rows[5],
        2,
        &[Some(0.0), Some(0.0), Some(0.0), None, None, Some(0.0)],
    );
    // the statistics use the values that occur: 2 rows by 3 columns
    let chi = titled("Chi-Square Tests: q by p");
    assert_eq!(texts(&chi.rows[0])[0], "Pearson Chi-Square");
    assert_row(&chi.rows[0][2..3], 0, &[Some(2.0)]);

    let p = titled("p by p");
    assert_row(
        &p.rows[6],
        2,
        &[
            Some(0.0),
            Some(0.0),
            Some(0.0),
            Some(1.0),
            Some(0.0),
            Some(1.0),
        ],
    );
    assert_row(
        &p.rows[10],
        2,
        &[
            Some(3.0),
            Some(3.0),
            Some(1.0),
            Some(1.0),
            Some(0.0),
            Some(8.0),
        ],
    );

    assert_eq!(
        error(&mut engine, "CROSSTABS VARIABLES=p (1,4) /TABLES=p BY q."),
        "q is not on VARIABLES; in integer mode TABLES uses only those"
    );
    assert!(
        error(
            &mut engine,
            "CROSSTABS VARIABLES=p (1,4) q (0,5000) /TABLES=p BY q."
        )
        .contains("at most 1000")
    );
}

/// The measures of association of the weighted 2 x 2 table [[3, 12], [9, 4]] (W = 28).
/// By hand: chi-square = 28 (12 - 108)^2 / (15 13 12 16) = 6.8923, phi = -root(6.8923 / 28)
/// (negative like the correlation), V = |phi|, CC = root(6.8923 / 34.8923) = 4/9; doubled
/// pairs P = 24 and Q = 216, so gamma = -192 / 240 = -0.8, tau-b = -192 / root(390 384),
/// tau-c = 2 (-192) / 28^2; lambda: rows 6/13, columns 5/12, symmetric 11/25; Goodman and
/// Kruskal's tau = phi^2. The standard errors, T and significances agree with GNU PSPP run
/// as a black box.
#[test]
fn crosstabs_measures_of_association() {
    let data = "DATA LIST FREE /a b n.\nBEGIN DATA\n1 1 3\n1 2 12\n2 1 9\n2 2 4\nEND DATA.\nWEIGHT BY n.\n";
    let mut engine = Engine::default();
    let tables = crosstabs(
        &mut engine,
        &format!(
            "{data}CROSSTABS a BY b /CELLS=NONE /STATISTICS=PHI CC LAMBDA BTAU CTAU GAMMA CORR."
        ),
    );
    let [symmetric, directional] = &tables[..] else {
        panic!("{tables:?}")
    };
    assert_eq!(symmetric.title, "Symmetric Measures: a by b");
    let chi = 28.0 * 96.0_f64.powi(2) / (15.0 * 13.0 * 12.0 * 16.0);
    let phi = -(chi / 28.0).sqrt();
    let tau_b = -192.0 / (390.0_f64 * 384.0).sqrt();
    let p = 0.00865674412724605;
    let rows: Vec<(String, Vec<Option<f64>>)> = symmetric
        .rows
        .iter()
        .map(|r| {
            (
                texts(r)[1].clone(),
                r[2..]
                    .iter()
                    .map(|c| match c {
                        Cell::Number(n) => Some(*n),
                        _ => None,
                    })
                    .collect(),
            )
        })
        .collect();
    let expected: [(&str, [Option<f64>; 4]); 9] = [
        ("Phi", [Some(phi), None, None, Some(p)]),
        ("Cramer's V", [Some(-phi), None, None, Some(p)]),
        (
            "Contingency Coefficient",
            [Some(4.0 / 9.0), None, None, Some(p)],
        ),
        (
            "Kendall's tau-b",
            [
                Some(tau_b),
                Some(0.1645027172548249),
                Some(-2.9833977696002307),
                Some(0.0028506724950433542),
            ],
        ),
        (
            "Kendall's tau-c",
            [
                Some(-384.0 / 784.0),
                Some(0.16417385685482314),
                Some(-2.9833977696002307),
                Some(0.0028506724950433542),
            ],
        ),
        (
            "Gamma",
            [
                Some(-0.8),
                Some(0.15874507866387544),
                Some(-2.9833977696002307),
                Some(0.0028506724950433542),
            ],
        ),
        (
            "Spearman Correlation",
            [
                Some(tau_b),
                Some(0.164502717254825),
                Some(-2.9137254363387344),
                Some(0.0072491951530327245),
            ],
        ),
        (
            "Pearson's R",
            [
                Some(tau_b),
                Some(0.164502717254825),
                Some(-2.9137254363387344),
                Some(0.0072491951530327245),
            ],
        ),
        // the case count's label stands in the first column
        ("", [Some(28.0), None, None, None]),
    ];
    assert_eq!(rows.len(), expected.len());
    for ((name, got), (want_name, want)) in rows.iter().zip(&expected) {
        assert_eq!(name, want_name);
        for (g, w) in got.iter().zip(want) {
            match (g, w) {
                (Some(g), Some(w)) => assert!((g - w).abs() < 1e-9, "{name}: {g} vs {w}"),
                (None, None) => {}
                other => panic!("{name}: {other:?}"),
            }
        }
    }
    assert_eq!(texts(&symmetric.rows[3])[0], "Ordinal by Ordinal");
    assert_eq!(texts(&symmetric.rows[8])[0], "N of Valid Cases");
    let lambdas: Vec<f64> = directional.rows[..3]
        .iter()
        .map(|r| number(&r[3]))
        .collect();
    for (got, want) in lambdas.iter().zip([11.0 / 25.0, 6.0 / 13.0, 5.0 / 12.0]) {
        assert!((got - want).abs() < 1e-12, "{lambdas:?}");
    }
    assert_eq!(
        directional
            .rows
            .iter()
            .map(|r| texts(r)[2].clone())
            .collect::<Vec<_>>(),
        [
            "Symmetric",
            "a Dependent",
            "b Dependent",
            "a Dependent",
            "b Dependent"
        ]
    );
    assert_eq!(texts(&directional.rows[0])[0], "Nominal by Nominal");
    for row in &directional.rows[3..] {
        assert!((number(&row[3]) - chi / 28.0).abs() < 1e-12);
    }

    // DVALUE shows the rows the other way round; the measures keep the ascending order
    let turned = crosstabs(
        &mut engine,
        "CROSSTABS a BY b /CELLS=NONE /STATISTICS=GAMMA /FORMAT=DVALUE.",
    );
    assert!((number(&turned[0].rows[0][2]) + 0.8).abs() < 1e-12);
    // a string variable has no correlations; a constant one no measures, only its count
    let strings = crosstabs(
        &mut engine,
        "STRING s (A1).\nCOMPUTE s = 'x'.\nIF a = 2 s = 'y'.\nCOMPUTE k = 1.\n\
         CROSSTABS /TABLES=s BY b /TABLES=k BY b /CELLS=NONE /STATISTICS=CORR GAMMA.",
    );
    let names: Vec<String> = strings[0]
        .rows
        .iter()
        .map(|r| texts(r)[1].clone())
        .collect();
    assert_eq!(names, ["Gamma", ""]);
    assert_eq!(strings[1].rows.len(), 1);
}

/// Kappa and the risk estimates of the 2 x 2 table [[3, 12], [9, 4]]. By hand: kappa =
/// (28 7 - 388) / (28^2 - 388) with 388 = 15 12 + 13 16; odds ratio 3 4 / (12 9) = 1/9; the
/// cohort risks 3 13 / (9 15) and 12 13 / (4 15), their 95% limits exp(+-z v) with v as the
/// manual gives it. The standard error of kappa is that of the delta method (GNU PSPP prints
/// another one).
#[test]
fn crosstabs_kappa_and_risk() {
    let data = "DATA LIST FREE /a b n.\nBEGIN DATA\n1 1 3\n1 2 12\n2 1 9\n2 2 4\nEND DATA.\nWEIGHT BY n.\n";
    let mut engine = Engine::default();
    let tables = crosstabs(
        &mut engine,
        &format!("{data}CROSSTABS a BY b /CELLS=NONE /STATISTICS=KAPPA RISK."),
    );
    let [symmetric, risk] = &tables[..] else {
        panic!("{tables:?}")
    };
    assert_eq!(
        texts(&symmetric.rows[0])[..2],
        ["Measure of Agreement", "Kappa"]
    );
    let kappa = -192.0 / 396.0;
    assert_row(
        &symmetric.rows[0],
        2,
        &[
            Some(kappa),
            Some(0.1654694117345023),
            Some(-2.6253204932555745),
            Some(0.008656744127246058),
        ],
    );
    let z = 1.959963984540054;
    let limits = |value: f64, v: f64| {
        [
            Some(value),
            Some(value * (-z * v).exp()),
            Some(value * (z * v).exp()),
        ]
    };
    let v_odds = (1.0 / 3.0 + 1.0 / 12.0 + 1.0 / 9.0 + 1.0 / 4.0_f64).sqrt();
    let v_first = (12.0 / (3.0 * 15.0) + 4.0 / (9.0 * 13.0_f64)).sqrt();
    let v_second = (3.0 / (12.0 * 15.0) + 9.0 / (4.0 * 13.0_f64)).sqrt();
    assert_eq!(texts(&risk.rows[0])[0], "Odds Ratio for a (1 / 2)");
    assert_row(&risk.rows[0], 1, &limits(1.0 / 9.0, v_odds));
    assert_eq!(texts(&risk.rows[1])[0], "For cohort b = 1");
    assert_row(&risk.rows[1], 1, &limits(39.0 / 135.0, v_first));
    assert_row(&risk.rows[2], 1, &limits(2.6, v_second));
    assert_eq!(texts(&risk.rows[3])[0], "N of Valid Cases");
    assert_eq!(risk.column_groups[2], ["95% Confidence Interval"]);

    // RISK alone makes no Symmetric Measures table; a larger table has only its count
    let more = crosstabs(
        &mut engine,
        "COMPUTE c = a + b.\nCROSSTABS a BY c /CELLS=NONE /STATISTICS=RISK.",
    );
    assert_eq!(more.len(), 1);
    assert_eq!(more[0].rows.len(), 1);
    assert!(more[0].caption.as_deref().unwrap().contains("2 x 2"));
    // kappa over a string and a number is not computed, and the table says why
    let mixed = crosstabs(
        &mut engine,
        "STRING s (A1).\nCOMPUTE s = 'x'.\nIF a = 2 s = 'y'.\n\
         CROSSTABS s BY b /CELLS=NONE /STATISTICS=KAPPA.",
    );
    assert_eq!(mixed[0].rows.len(), 1);
    assert!(mixed[0].caption.as_deref().unwrap().contains("same type"));
}

/// McNemar and the uncertainty coefficient. The 2 x 2 table [[3, 12], [9, 4]] has 12 and 9
/// off the diagonal: exact two-sided significance 2 sum_{i <= 9} C(21, i) / 2^21. The square
/// table [[12, 3, 1], [2, 9, 4], [1, 2, 11]]: McNemar-Bowker (3 - 2)^2 / 5 + 0 + (4 - 2)^2 / 6
/// on 3 degrees of freedom. The uncertainty coefficients are worked out from the entropies of
/// the totals and cells below; their standard errors agree with GNU PSPP.
#[test]
fn crosstabs_mcnemar_and_uncertainty() {
    let two = "DATA LIST FREE /a b n.\nBEGIN DATA\n1 1 3\n1 2 12\n2 1 9\n2 2 4\nEND DATA.\nWEIGHT BY n.\n";
    let mut engine = Engine::default();
    let tables = crosstabs(
        &mut engine,
        &format!("{two}CROSSTABS a BY b /CELLS=NONE /STATISTICS=MCNEMAR UC."),
    );
    let [tests, directional] = &tables[..] else {
        panic!("{tables:?}")
    };
    assert_eq!(texts(&tests.rows[0])[0], "McNemar Test");
    let exact: f64 = (0..=9u32)
        .map(|i| (0..i).fold(1.0, |c, k| c * f64::from(21 - k) / f64::from(k + 1)))
        .sum::<f64>()
        * 2.0
        / 2f64.powi(21);
    assert_row(&tests.rows[0], 1, &[None, None, None, Some(exact), None]);
    assert_eq!(texts(&tests.rows[1])[0], "N of Valid Cases");
    // the entropies of the rows (15, 13), the columns (12, 16) and the cells
    let h = |counts: &[f64]| {
        -counts
            .iter()
            .map(|n| n / 28.0 * (n / 28.0).ln())
            .sum::<f64>()
    };
    let (ux, uy, uxy) = (
        h(&[15.0, 13.0]),
        h(&[12.0, 16.0]),
        h(&[3.0, 12.0, 9.0, 4.0]),
    );
    let gain = ux + uy - uxy;
    let expected = [
        ("Symmetric", 2.0 * gain / (ux + uy), 0.1297093312),
        ("a Dependent", gain / ux, 0.1291746711),
        ("b Dependent", gain / uy, 0.1303272375),
    ];
    for (row, (which, value, ase)) in directional.rows.iter().zip(expected) {
        assert_eq!(texts(row)[2], which);
        assert!((number(&row[3]) - value).abs() < 1e-12, "{which}");
        assert!((number(&row[4]) - ase).abs() < 1e-9, "{which}");
        assert!((number(&row[5]) - 1.4372722438).abs() < 1e-9, "{which}");
    }
    assert_eq!(texts(&directional.rows[0])[1], "Uncertainty Coefficient");

    let square = "DATA LIST FREE /a b n.\nBEGIN DATA\n1 1 12\n1 2 3\n1 3 1\n2 1 2\n2 2 9\n2 3 4\n\
                  3 1 1\n3 2 2\n3 3 11\nEND DATA.\nWEIGHT BY n.\n";
    let tables = crosstabs(
        &mut engine,
        &format!("{square}CROSSTABS a BY b /CELLS=NONE /STATISTICS=MCNEMAR CHISQ."),
    );
    let bowker = tables[0]
        .rows
        .iter()
        .find(|r| texts(r)[0] == "McNemar-Bowker Test")
        .unwrap();
    let x = 1.0 / 5.0 + 4.0 / 6.0;
    assert_row(&bowker[..4], 1, &[Some(x), Some(3.0), Some(0.8334635315)]);
    // rows and columns with different values: no test, and the table says why
    let tables = crosstabs(
        &mut engine,
        "COMPUTE c = b + 1.\nCROSSTABS a BY c /CELLS=NONE /STATISTICS=MCNEMAR.",
    );
    assert_eq!(tables[0].rows.len(), 1);
    assert!(tables[0].caption.as_deref().unwrap().contains("square"));
}

/// Somers' d and eta of the 2 x 2 table [[3, 12], [9, 4]]: with doubled pair counts P - Q =
/// 24 - 216, d is -192 / 390 with b dependent (390 = 28^2 - 15^2 - 13^2), -192 / 384 with a
/// dependent and -192 / 387 symmetric; eta of two binary variables is |phi| both ways. The
/// directional standard errors and T agree with GNU PSPP; the symmetric one scales tau-b's.
#[test]
fn crosstabs_somers_d_and_eta() {
    let data = "DATA LIST FREE /a b n.\nBEGIN DATA\n1 1 3\n1 2 12\n2 1 9\n2 2 4\nEND DATA.\nWEIGHT BY n.\n";
    let mut engine = Engine::default();
    let tables = crosstabs(
        &mut engine,
        &format!("{data}CROSSTABS a BY b /CELLS=NONE /STATISTICS=ETA D."),
    );
    let [directional] = &tables[..] else {
        panic!("{tables:?}")
    };
    let phi = 96.0 / (15.0_f64 * 13.0 * 12.0 * 16.0).sqrt();
    let expected = [
        (
            "Ordinal by Ordinal",
            "Somers' d",
            "Symmetric",
            -192.0 / 387.0,
            Some(0.1644977745),
        ),
        ("", "", "a Dependent", -0.5, Some(0.1653594569)),
        ("", "", "b Dependent", -192.0 / 390.0, Some(0.1644768906)),
        ("Nominal by Interval", "Eta", "a Dependent", phi, None),
        ("", "", "b Dependent", phi, None),
    ];
    assert_eq!(directional.rows.len(), expected.len());
    for (row, (group, name, which, value, ase)) in directional.rows.iter().zip(expected) {
        assert_eq!(texts(row)[..3], [group, name, which]);
        assert!((number(&row[3]) - value).abs() < 1e-12, "{which}");
        match ase {
            Some(ase) => {
                assert!((number(&row[4]) - ase).abs() < 1e-9, "{which}");
                assert!((number(&row[5]) + 2.9833977696).abs() < 1e-9, "{which}");
            }
            None => assert_eq!(row[4..], [Cell::Empty, Cell::Empty, Cell::Empty]),
        }
    }
    // eta needs numbers
    let strings = crosstabs(
        &mut engine,
        "STRING s (A1).\nCOMPUTE s = 'x'.\nIF a = 2 s = 'y'.\nCROSSTABS s BY b /CELLS=NONE /STATISTICS=ETA.",
    );
    assert!(strings.is_empty() || strings[0].rows.is_empty());
}

/// CMH over three 2 x 2 strata. The common odds ratio by hand: (12 15/40 + 7 13/33 +
/// 20 9/46) / (8 5/40 + 9 4/33 + 6 11/46). Mantel-Haenszel's chi-square, the odds ratio and
/// its interval agree with R's mantelhaen.test (with continuity correction), run as a black
/// box; Cochran's, Breslow-Day's and Tarone's with an independent computation.
#[test]
fn crosstabs_cmh() {
    let data = "DATA LIST FREE /a b g n.\nBEGIN DATA\n1 1 1 12\n1 2 1 8\n2 1 1 5\n2 2 1 15\n\
                1 1 2 7\n1 2 2 9\n2 1 2 4\n2 2 2 13\n1 1 3 20\n1 2 3 6\n2 1 3 11\n2 2 3 9\n\
                END DATA.\nWEIGHT BY n.\n";
    let mut engine = Engine::default();
    let tables = crosstabs(
        &mut engine,
        &format!("{data}CROSSTABS a BY b BY g /CELLS=NONE /STATISTICS=CMH(1)."),
    );
    let [homogeneity, independence, estimate] = &tables[..] else {
        panic!("{tables:?}")
    };
    assert_eq!(
        homogeneity.title,
        "Tests of Homogeneity of the Odds Ratio: a by b by g"
    );
    assert_row(
        &homogeneity.rows[0],
        1,
        &[Some(0.4015963279279383), Some(2.0), Some(0.8180775324)],
    );
    assert_row(
        &homogeneity.rows[1],
        1,
        &[Some(0.40148793985485526), Some(2.0), Some(0.8181218685)],
    );
    assert_row(
        &independence.rows[0],
        1,
        &[Some(8.64623709221765), Some(1.0), Some(0.0032773753)],
    );
    assert_row(
        &independence.rows[1],
        1,
        &[Some(7.36154768693), Some(1.0), Some(0.0066633412)],
    );
    let theta = (12.0 * 15.0 / 40.0 + 7.0 * 13.0 / 33.0 + 20.0 * 9.0 / 46.0)
        / (8.0 * 5.0 / 40.0 + 9.0 * 4.0 / 33.0 + 6.0 * 11.0 / 46.0);
    let values: Vec<f64> = estimate.rows.iter().map(|r| number(&r[1])).collect();
    assert!((values[0] - theta).abs() < 1e-12);
    assert!((values[1] - theta.ln()).abs() < 1e-12);
    for (got, want) in values[4..6].iter().zip([1.44801696867, 6.93253673613]) {
        assert!((got - want).abs() < 1e-9, "{values:?}");
    }
    // another null value moves only the significance
    let other = crosstabs(
        &mut engine,
        "CROSSTABS a BY b BY g /CELLS=NONE /STATISTICS=CMH(3.168348281).",
    );
    assert!(number(&other[2].rows[3][1]) > 0.99);
    assert!(other[2].caption.as_deref().unwrap().contains("3.168348281"));
    // a stratum with an empty second row adds nothing to Breslow-Day, and leaves it a number
    let empty_row = crosstabs(
        &mut engine,
        &format!(
            "{data}IF g = 3 & a = 2 n = 0.
CROSSTABS a BY b BY g /CELLS=NONE /STATISTICS=CMH."
        ),
    );
    assert!(number(&empty_row[0].rows[0][1]).is_finite());
    // ALL: the chi-square table with McNemar, both measures tables, the risk estimate and CMH
    let all = crosstabs(
        &mut engine,
        &format!("{data}CROSSTABS a BY b BY g /CELLS=NONE /STATISTICS=ALL."),
    );
    let titles: Vec<&str> = all
        .iter()
        .map(|t| t.title.split(':').next().unwrap())
        .collect();
    assert_eq!(
        titles,
        [
            "Chi-Square Tests",
            "Symmetric Measures",
            "Directional Measures",
            "Risk Estimate",
            "Tests of Homogeneity of the Odds Ratio",
            "Tests of Conditional Independence",
            "Mantel-Haenszel Common Odds Ratio Estimate"
        ]
    );
    assert!(all[0].rows.iter().any(|r| texts(r)[1] == "McNemar Test"));
    // a variable with three values: no tests, and the table says why
    let three = crosstabs(
        &mut engine,
        "COMPUTE c = b + (g = 3).\nCROSSTABS a BY c BY g /CELLS=NONE /STATISTICS=CMH.",
    );
    assert_eq!(three.len(), 1);
    assert!(three[0].caption.as_deref().unwrap().contains("two values"));
}

#[test]
fn crosstabs_missing_values() {
    let mut engine = survey();
    let table = |engine: &mut Engine, syntax: &str| crosstabs(engine, syntax).remove(0);
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

// COUNT (manual, CROSSTABS): CASE makes every case weight whole before it is counted, CELL every
// cell's summed weight before anything is computed from it, each by ROUND or TRUNCATE; without
// COUNT the weights stay as they are.
#[test]
fn crosstabs_count_rounds_or_truncates_case_weights_or_cell_counts() {
    let data = "DATA LIST FREE /r c w.\nBEGIN DATA\n1 1 1.4\n1 1 1.4\n1 2 0.6\n2 1 2.5\n2 2 0.4\n\
                END DATA.\nWEIGHT BY w.\n";
    let counts = |count: &str| {
        let mut engine = Engine::default();
        let tables = crosstabs(&mut engine, &format!("{data}CROSSTABS r BY c {count}."));
        let rows = &tables[0].rows;
        (0..rows.len())
            .map(|i| rows[i][2..].iter().map(number).collect::<Vec<f64>>())
            .collect::<Vec<_>>()
    };
    let close = |count: &str, expected: &[&[f64]]| {
        let actual = counts(count);
        assert_eq!(actual.len(), expected.len(), "{count}: {actual:?}");
        for (a, e) in actual.iter().zip(expected) {
            assert_eq!(a.len(), e.len(), "{count}: {actual:?}");
            for (x, y) in a.iter().zip(*e) {
                assert!((x - y).abs() < 1e-9, "{count}: {actual:?}");
            }
        }
    };
    let asis: &[&[f64]] = &[&[2.8, 0.6, 3.4], &[2.5, 0.4, 2.9], &[5.3, 1.0, 6.3]];
    close("", asis);
    close("/COUNT ASIS", asis);
    // CELL ROUND is what the dialog writes; a keyword left out is CELL or ROUND
    let cell_round: &[&[f64]] = &[&[3.0, 1.0, 4.0], &[3.0, 0.0, 3.0], &[6.0, 1.0, 7.0]];
    close("/COUNT ROUND CELL", cell_round);
    close("/COUNT", cell_round);
    let cell_truncate: &[&[f64]] = &[&[2.0, 0.0, 2.0], &[2.0, 0.0, 2.0], &[4.0, 0.0, 4.0]];
    close("/COUNT CELL TRUNCATE", cell_truncate);
    // CASE ROUND: weights 1 1 1 3 0, so the last case drops out
    let case_round: &[&[f64]] = &[&[2.0, 1.0, 3.0], &[3.0, 0.0, 3.0], &[5.0, 1.0, 6.0]];
    close("/COUNT=CASE ROUND", case_round);
    // CASE TRUNCATE: weights 1 1 0 2 0, so column 2 has no case left
    close(
        "/COUNT CASE TRUNCATE",
        &[&[2.0, 2.0], &[2.0, 2.0], &[4.0, 4.0]],
    );
}
