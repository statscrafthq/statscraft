//! CTABLES on the survey fixtures. The expected numbers agree with the reference program on
//! the same files (see reference_ctables.rs).

use statscraft_data::{Dataset, Measure};
use statscraft_engine::{Cell, Engine, Table, format_value};
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

fn run(file: &str, source: &str) -> Table {
    let mut engine = Engine::default();
    let source = format!("GET FILE='{}'.\n{source}", fixture(file));
    run_on(&mut engine, &source).unwrap().remove(0)
}

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(t) => t,
        other => panic!("not text: {other:?}"),
    }
}

/// The numbers of one row, rounded to 6 decimals; None for an empty cell.
fn numbers(t: &Table, row: usize) -> Vec<Option<f64>> {
    t.rows[row][t.row_headers..]
        .iter()
        .map(|c| match c {
            Cell::Number(n) => Some((n * 1e6).round() / 1e6),
            Cell::Empty => None,
            other => panic!("not a number: {other:?}"),
        })
        .collect()
}

#[test]
fn one_way_counts() {
    let t = run("survey_basic.sav", "CTABLES /TABLE gender.");
    assert_eq!(t.title, "Custom Tables");
    assert_eq!(t.row_headers, 2);
    assert_eq!(t.columns, ["", "", "Count"]);
    let labels: Vec<&str> = t.rows.iter().map(|r| text(&r[1])).collect();
    assert_eq!(
        labels,
        ["Male", "Female", "Other or prefer to self-describe"]
    );
    assert_eq!(text(&t.rows[2][0]), "Gender");
    let counts: Vec<Option<f64>> = (0..3).flat_map(|r| numbers(&t, r)).collect();
    assert_eq!(counts, [Some(106.0), Some(88.0), Some(6.0)]);
    assert_eq!(t.formats[0][2], "F40.0");
}

#[test]
fn two_way_counts_with_header_levels() {
    let t = run("survey_basic.sav", "CTABLES /TABLE gender BY region.");
    assert_eq!(t.columns.len(), 7);
    assert_eq!(t.columns[2], "Count");
    assert_eq!(t.column_groups[2], ["Region of residence", "North"]);
    assert_eq!(t.column_groups[6], ["Region of residence", "Central"]);
    assert_eq!(numbers(&t, 0), [23.0, 23.0, 15.0, 27.0, 18.0].map(Some));
    // an empty labelled category shows zero
    assert_eq!(numbers(&t, 2), [0.0, 1.0, 2.0, 0.0, 3.0].map(Some));
}

#[test]
fn nesting_and_stacking() {
    let t = run("survey_basic.sav", "CTABLES /TABLE gender > region.");
    assert_eq!((t.row_headers, t.rows.len()), (4, 15));
    let labels: Vec<&str> = t.rows[5][..4].iter().map(text).collect();
    assert_eq!(labels, ["Gender", "Female", "Region of residence", "North"]);
    assert_eq!(numbers(&t, 5), [Some(11.0)]);

    let t = run("survey_basic.sav", "CTABLES /TABLE gender + region.");
    assert_eq!((t.row_headers, t.rows.len()), (2, 8));
    assert_eq!(text(&t.rows[3][0]), "Region of residence");
    assert_eq!(numbers(&t, 3), [Some(34.0)]);
}

#[test]
fn totals_and_percentages() {
    let t = run(
        "survey_basic.sav",
        "CTABLES /TABLE gender [COUNT, COLPCT.COUNT, ROWPCT.COUNT] BY region\n\
         /CATEGORIES VARIABLES=gender region TOTAL=YES.",
    );
    assert_eq!(t.rows.len(), 4);
    assert_eq!(text(&t.rows[3][1]), "Total");
    assert_eq!(t.column_groups.last().unwrap()[1], "Total");
    // Male: North count, column %, row %, ..., Total count, column % (none), row %
    let male = numbers(&t, 0);
    assert_eq!(&male[..3], [Some(23.0), Some(67.647059), Some(21.698113)]);
    assert_eq!(&male[15..], [Some(106.0), None, Some(100.0)]);
    let total = numbers(&t, 3);
    assert_eq!(&total[..3], [Some(34.0), Some(100.0), None]);
    assert_eq!(total[15], Some(200.0));
    assert_eq!(t.formats[0][3], "PCT40.1");
    assert_eq!(format_value(67.647059, &t.formats[0][3], false), "67.6%");
}

#[test]
fn missing_categories_and_valid_percentages() {
    let t = run(
        "survey_basic.sav",
        "CTABLES /TABLE q1_satisfaction [COUNT, COLPCT.COUNT, COLPCT.VALIDN] BY gender\n\
         /CATEGORIES VARIABLES=q1_satisfaction MISSING=INCLUDE TOTAL=YES.",
    );
    assert_eq!(text(&t.rows[5][1]), "Don't know");
    let male: Vec<Option<f64>> = (0..8).map(|r| numbers(&t, r)[1]).collect();
    assert_eq!(male[0], Some(4.716981));
    // the valid percentage leaves out the user-missing categories
    assert_eq!(numbers(&t, 0)[2], Some(5.208333));
    assert_eq!(numbers(&t, 5)[2], Some(0.0));
    assert_eq!(numbers(&t, 7)[..3], [Some(106.0), Some(100.0), Some(100.0)]);
}

#[test]
fn scale_summary_by_category() {
    let t = run(
        "survey_basic.sav",
        "CTABLES /TABLE monthly_spend [MEAN, MEDIAN, VALIDN, MISSING, COUNT] BY region.",
    );
    assert_eq!(t.row_headers, 1);
    assert_eq!(text(&t.rows[0][0]), "Average monthly spend (EUR)");
    let north = &numbers(&t, 0)[..5];
    assert_eq!(
        north[1..],
        [Some(190.38), Some(30.0), Some(4.0), Some(34.0)]
    );
    assert_eq!(
        format_value(north[0].unwrap(), &t.formats[0][1], false),
        "191.14"
    );
    assert_eq!(t.formats[0][1], "F8.2");

    let t = run(
        "survey_basic.sav",
        "CTABLES /TABLE age [MEAN, STDDEV, MODE] BY gender.",
    );
    assert_eq!(
        numbers(&t, 0)[..3],
        [Some(51.311321), Some(19.770282), Some(73.0)]
    );
    // a statistic in the variable's format, F3.0
    assert_eq!(format_value(51.311321, &t.formats[0][1], false), "51");
}

#[test]
fn stacked_scale_variables_share_summary_columns() {
    let t = run(
        "survey_basic.sav",
        "CTABLES /TABLE age [MEAN, COUNT] + monthly_spend [MEAN F8.1, VALIDN] BY gender.",
    );
    let leaves: Vec<&str> = t.columns[1..4].iter().map(String::as_str).collect();
    assert_eq!(leaves, ["Mean", "Count", "Valid N"]);
    assert_eq!(numbers(&t, 0)[..3], [Some(51.311321), Some(106.0), None]);
    assert_eq!(numbers(&t, 1)[1..3], [None, Some(97.0)]);
    assert_eq!(
        (t.formats[0][1].as_str(), t.formats[1][1].as_str()),
        ("F3.0", "F8.1")
    );
}

#[test]
fn weighted_counts_use_rounded_case_weights() {
    let t = run(
        "survey_basic_phpwriter.sav",
        "CTABLES /TABLE gender [COUNT, UCOUNT, COLPCT.COUNT] BY region.",
    );
    assert_eq!(
        numbers(&t, 0)[..3],
        [Some(30.0), Some(23.0), Some(66.666667)]
    );
    // one case of weight 0.43: no weight, but the cell exists
    assert_eq!(numbers(&t, 2)[3..6], [Some(0.0), Some(1.0), Some(0.0)]);

    let t = run(
        "survey_basic_phpwriter.sav",
        "CTABLES /TABLE age [MEAN, MEDIAN, UMEAN, VALIDN] BY gender.",
    );
    assert_eq!(
        numbers(&t, 0)[..4],
        [Some(51.530303), Some(52.0), Some(51.311321), Some(132.0)]
    );
}

#[test]
fn titles_and_explicit_categories() {
    let t = run(
        "survey_basic.sav",
        "CTABLES /TABLE region /CATEGORIES VARIABLES=region [3, 1, 5] TOTAL=YES\n\
         /TITLES TITLE='Regions' CAPTION='Three of five' CORNER='Base: all'.",
    );
    assert_eq!(t.title, "Regions");
    assert_eq!(t.caption.as_deref(), Some("Three of five"));
    assert_eq!(t.columns[0], "Base: all");
    let labels: Vec<&str> = t.rows.iter().map(|r| text(&r[1])).collect();
    assert_eq!(labels, ["East", "North", "Central", "Total"]);
    assert_eq!(numbers(&t, 3), [Some(110.0)]);
}

#[test]
fn an_unknown_level_is_guessed_from_the_values() {
    let mut data = Dataset::read_sav(fixture("survey_basic.sav")).unwrap();
    data.set_measure("region", Measure::Unknown).unwrap();
    data.set_measure("age", Measure::Unknown).unwrap();
    let mut engine = Engine::with_dataset(Some(data));
    let t = run_on(&mut engine, "CTABLES /TABLE region + age.")
        .unwrap()
        .remove(0);
    // region has five whole values and is categorical, age has many and is scale
    assert_eq!(t.rows.len(), 6);
    assert_eq!(t.columns[2..], ["Count", "Mean"]);
}

#[test]
fn errors_name_the_variable_and_the_fix() {
    let mut engine = Engine::default();
    let load = format!("GET FILE='{}'.\n", fixture("survey_basic.sav"));
    let mut fails = |syntax: &str| run_on(&mut engine, &format!("{load}{syntax}")).unwrap_err();
    let e = fails("CTABLES /TABLE gender [MEAN] BY region.");
    assert!(
        e.contains("MEAN applies only to scale variables") && e.contains("gender [S]"),
        "{e}"
    );
    let e = fails("CTABLES /TABLE nope BY region.");
    assert!(e.contains("unknown variable nope"), "{e}");
    let e = fails("CTABLES /TABLE age > monthly_spend.");
    assert!(e.contains("stack them with +"), "{e}");
    let e = fails("CTABLES /TABLE plan [S].");
    assert!(e.contains("string variable"), "{e}");
}
