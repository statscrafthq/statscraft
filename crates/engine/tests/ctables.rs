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
    run_all(file, source).remove(0)
}

fn run_all(file: &str, source: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let source = format!("GET FILE='{}'.\n{source}", fixture(file));
    run_on(&mut engine, &source).unwrap()
}

fn all_numbers(t: &Table) -> Vec<Vec<Option<f64>>> {
    (0..t.rows.len()).map(|r| numbers(t, r)).collect()
}

// Layers (Command Syntax Reference, CTABLES, TABLE subcommand and the summary functions table;
// Custom Tables manual, "Layers" and "Percentages"): `rows BY columns BY layers` gives a table for
// every layer category, nested layers a table for every combination, stacked layers one for every
// category of each. Within a layer every percentage but TABLEPCT is taken as if the layer were
// the whole table: LAYERPCT is the percentage of the layer, LAYERROWPCT and LAYERCOLPCT run across
// the subtables of the layer. TABLEPCT is of the whole table, all layers together.
const LAYERED: &str = "[COUNT, COLPCT, ROWPCT, SUBTABLEPCT, LAYERPCT, LAYERROWPCT, LAYERCOLPCT]";

#[test]
fn a_layer_is_the_table_of_its_cases() {
    for file in ["survey_basic.sav", "survey_basic_phpwriter.sav"] {
        let layers = run_all(
            file,
            &format!("CTABLES /TABLE q5_2 > gender {LAYERED} BY region BY q5_1."),
        );
        assert_eq!(layers.len(), 2);
        for (k, layer) in layers.iter().enumerate() {
            let alone = run(
                file,
                &format!(
                    "SELECT IF q5_1 = {k}.\nCTABLES /TABLE q5_2 > gender {LAYERED} BY region."
                ),
            );
            assert_eq!(all_numbers(layer), all_numbers(&alone), "{file} layer {k}");
            assert_eq!(
                (&layer.columns, &layer.rows.len()),
                (&alone.columns, &alone.rows.len())
            );
        }
        let label = ["Not selected", "Selected"];
        for (layer, label) in layers.iter().zip(label) {
            assert!(
                layer.title.starts_with("Custom Tables (Q5. "),
                "{}",
                layer.title
            );
            assert!(
                layer.title.ends_with(&format!("Phone: {label})")),
                "{}",
                layer.title
            );
        }
    }
}

#[test]
fn table_percentages_run_over_all_layers_and_a_total_layer_is_the_whole_table() {
    let tables = run_all(
        "survey_basic.sav",
        "CTABLES /TABLE gender [TABLEPCT, LAYERPCT, COLPCT] BY region BY q5_1 > q5_2\n\
         /CATEGORIES VARIABLES=q5_2 TOTAL=YES.",
    );
    // two categories of q5_1 by two of q5_2 and its total
    assert_eq!(tables.len(), 6);
    assert!(
        tables[2].title.ends_with("Email: Total)"),
        "{}",
        tables[2].title
    );
    let sum = |picked: &[usize], column: usize| -> f64 {
        picked
            .iter()
            .flat_map(|&t| (0..3).map(move |r| (t, r)))
            .map(|(t, r)| {
                numbers(&tables[t], r)
                    .iter()
                    .skip(column)
                    .step_by(3)
                    .flatten()
                    .sum::<f64>()
            })
            .sum()
    };
    // TABLEPCT adds up to 100 over the four layers of plain categories, LAYERPCT in each (the
    // cells are rounded to 6 decimals)
    assert!((sum(&[0, 1, 3, 4], 0) - 100.0).abs() < 1e-4);
    for t in [0, 1, 2, 3, 4, 5] {
        assert!((sum(&[t], 1) - 100.0).abs() < 1e-4, "layer {t}");
    }
    // the total of q5_2 within q5_1 = 0 is the layer q5_1 = 0 of a table without q5_2
    let outer = run_all(
        "survey_basic.sav",
        "CTABLES /TABLE gender [TABLEPCT, LAYERPCT, COLPCT] BY region BY q5_1.",
    );
    assert_eq!(all_numbers(&tables[2]), all_numbers(&outer[0]));
    assert_eq!(all_numbers(&tables[5]), all_numbers(&outer[1]));
}

#[test]
fn stacked_layers_and_summaries_in_the_layers() {
    let tables = run_all(
        "survey_basic.sav",
        "CTABLES /TABLE gender BY region BY q5_1 + q5_2.",
    );
    assert_eq!(tables.len(), 4);
    assert!(
        tables[2].title.contains("Email: Not selected"),
        "{}",
        tables[2].title
    );
    // SLABELS POSITION=LAYER: a table for every summary, in the layers' order
    let plain = run(
        "survey_basic.sav",
        "CTABLES /TABLE gender [COUNT, COLPCT] BY region.",
    );
    let tables = run_all(
        "survey_basic.sav",
        "CTABLES /TABLE gender [COUNT, COLPCT] BY region /SLABELS POSITION=LAYER.",
    );
    assert_eq!(tables.len(), 2);
    assert_eq!(tables[1].title, "Custom Tables (Column %)");
    for (s, table) in tables.iter().enumerate() {
        assert_eq!(table.columns.len(), 2 + 5);
        for r in 0..3 {
            let every: Vec<Option<f64>> =
                numbers(&plain, r).into_iter().skip(s).step_by(2).collect();
            assert_eq!(numbers(table, r), every, "summary {s} row {r}");
        }
    }
}

#[test]
fn tests_run_in_every_layer() {
    let tables = run_all(
        "survey_basic.sav",
        "CTABLES /TABLE gender BY region BY q5_1 /SIGTEST TYPE=CHISQUARE.",
    );
    let titles: Vec<&str> = tables.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles.len(), 4, "{titles:?}");
    assert!(titles[1].starts_with("Pearson Chi-Square Tests (Q5. "));
    for k in 0..2 {
        let alone = run_all(
            "survey_basic.sav",
            &format!(
                "SELECT IF q5_1 = {k}.\nCTABLES /TABLE gender BY region /SIGTEST TYPE=CHISQUARE."
            ),
        );
        // chi-square, df and significance
        assert_eq!(
            tables[2 * k + 1].rows[0][2..5],
            alone[1].rows[0][2..5],
            "layer {k}"
        );
    }
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
    // Male: North count, column %, row %, ..., Total count, column %, row %. A total column has
    // column percentages and a total row has row percentages (the manual's Figure 25 shows the
    // column percentages of a Total column): their base is every category of the totaled
    // variable.
    let male = numbers(&t, 0);
    assert_eq!(&male[..3], [Some(23.0), Some(67.647059), Some(21.698113)]);
    assert_eq!(&male[15..], [Some(106.0), Some(53.0), Some(100.0)]);
    let total = numbers(&t, 3);
    assert_eq!(&total[..3], [Some(34.0), Some(100.0), Some(17.0)]);
    assert_eq!(&total[15..], [Some(200.0), Some(100.0), Some(100.0)]);
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

// The weighted counts equal those of FREQUENCIES on the same file, which agree with the reference
// program: Male 132.3074, Female 119.3866, Other 8.2401. Case weights are not rounded.
#[test]
fn weighted_counts_use_the_case_weights_as_they_are() {
    let t = run(
        "survey_basic_phpwriter.sav",
        "CTABLES /TABLE gender [COUNT, UCOUNT].",
    );
    let close = |got: Option<f64>, want: f64| (got.unwrap() - want).abs() < 5e-5;
    for (row, (weighted, cases)) in [(132.3074, 106.0), (119.3866, 88.0), (8.2401, 6.0)]
        .into_iter()
        .enumerate()
    {
        let cells = numbers(&t, row);
        assert!(close(cells[0], weighted), "{cells:?}");
        assert_eq!(cells[1], Some(cases));
    }

    // weighted mean and median differ from the unweighted ones
    let t = run(
        "survey_basic_phpwriter.sav",
        "CTABLES /TABLE age [MEAN, UMEAN, VALIDN, UVALIDN] BY gender.",
    );
    let cells = numbers(&t, 0);
    assert!(close(cells[2], 132.3074), "{cells:?}");
    assert_eq!(cells[3], Some(106.0));
    assert_ne!(cells[0], cells[1]);
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

#[test]
fn percentiles_use_the_averaged_empirical_distribution() {
    // values 1 to 4: a percentile that falls on a whole case is the mean of two neighbours
    let t = run(
        "survey_basic.sav",
        "SELECT IF resp_id <= 1004.\nCOMPUTE x = resp_id - 1000.\n\
         CTABLES /TABLE x [S] [PTILE 25, PTILE 30, MEDIAN, PTILE 75, PTILE 100].",
    );
    assert_eq!(numbers(&t, 0), [1.5, 2.0, 2.5, 3.5, 4.0].map(Some));

    // weights 1, 1 and 0.5: the median is at 1.25 of 2.5, inside the second value
    let t = run(
        "survey_basic.sav",
        "SELECT IF resp_id <= 1003.\nCOMPUTE x = resp_id - 1000.\nCOMPUTE w = 1.\n\
         IF (x = 3) w = 0.5.\nWEIGHT BY w.\nCTABLES /TABLE x [S] [MEDIAN, PTILE 40].",
    );
    assert_eq!(numbers(&t, 0), [2.0, 1.5].map(Some));
}

#[test]
fn a_case_without_weight_is_left_out_also_unweighted() {
    let t = run(
        "survey_basic.sav",
        "COMPUTE w = 1.\nIF (gender = 1) w = 0.\nWEIGHT BY w.\nCTABLES /TABLE gender [COUNT, UCOUNT].",
    );
    assert_eq!(numbers(&t, 0), [0.0, 0.0].map(Some));
    assert_eq!(numbers(&t, 1), [88.0, 88.0].map(Some));
}

#[test]
fn a_count_without_cases_is_zero_also_beside_a_scale_variable() {
    let t = run(
        "survey_basic.sav",
        "CTABLES /TABLE gender > monthly_spend [MEAN, COUNT] BY region.",
    );
    // nobody of the third gender lives in the North: no mean, a count of 0
    assert_eq!(numbers(&t, 2)[..2], [None, Some(0.0)]);
    let t = run(
        "survey_basic.sav",
        "CTABLES /FORMAT EMPTY=BLANK /TABLE gender > monthly_spend [MEAN, COUNT] BY region.",
    );
    assert_eq!(numbers(&t, 2)[..2], [None, None]);
}

#[test]
fn the_level_of_new_variables_follows_the_default_rules() {
    let t = run(
        "survey_basic.sav",
        "COMPUTE small = region.\nCOMPUTE big = region + 10.\nCOMPUTE day = region.\n\
         FORMATS day (DATE11).\nCTABLES /TABLE small + big + day.",
    );
    // few whole numbers are categorical, unless none is below 10 or the format is a date
    assert_eq!(t.rows.len(), 7);
    assert_eq!(t.columns[2..], ["Count", "Mean"]);
}

#[test]
fn a_value_named_twice_takes_its_last_place() {
    let labels =
        |t: &Table| -> Vec<String> { t.rows.iter().map(|r| text(&r[1]).to_string()).collect() };
    let t = run(
        "survey_basic.sav",
        "CTABLES /TABLE region /CATEGORIES VARIABLES=region [1, 3, 1].",
    );
    assert_eq!(labels(&t), ["East", "North"]);
    let t = run(
        "survey_basic.sav",
        "CTABLES /TABLE region /CATEGORIES VARIABLES=region [3, 1 THRU 5].",
    );
    let labels = labels(&t);
    assert_eq!(labels.len(), 5);
    assert_eq!((labels[0].as_str(), labels[2].as_str()), ("North", "East"));
}

// The total of a variable is the area of all its categories, so the Total block of an outer
// variable holds the same numbers as the table without that variable, percentages included.
#[test]
fn the_total_of_an_outer_variable_is_the_table_without_it() {
    let summaries = "[COUNT, COLPCT, ROWPCT, SUBTABLEPCT, TABLEPCT, LAYERCOLPCT]";
    let nested = run(
        "survey_basic.sav",
        &format!(
            "CTABLES /TABLE region > gender {summaries} BY q5_1\n\
             /CATEGORIES VARIABLES=region q5_1 TOTAL=YES."
        ),
    );
    let flat = run(
        "survey_basic.sav",
        &format!("CTABLES /TABLE gender {summaries} BY q5_1 /CATEGORIES VARIABLES=q5_1 TOTAL=YES."),
    );
    assert_eq!(nested.rows.len(), 18);
    for g in 0..3 {
        assert_eq!(numbers(&nested, 15 + g), numbers(&flat, g), "gender {g}");
    }
}

/// The raw values of a numeric variable; None for system-missing, user-missing codes kept.
fn raw(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    data.variable(name)
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect()
}

fn percent(part: usize, whole: usize) -> Option<f64> {
    Some((100.0 * part as f64 / whole as f64 * 1e6).round() / 1e6)
}

// Missing values in summaries (Command Syntax Reference, CTABLES, Table 31, page 491): a case whose
// value is user-missing and not shown, or system-missing, counts in TOTALN and the .TOTALN
// percentages but not in COUNT; a case whose valid value an explicit list leaves out is not in the
// table at all, not even in TOTALN.
#[test]
fn hidden_missing_values_count_in_totaln_but_values_left_out_do_not() {
    let data = Dataset::read_sav(fixture("survey_basic.sav")).unwrap();
    let (gender, q1) = (raw(&data, "gender"), raw(&data, "q1_satisfaction"));
    let n = data.case_count();
    let count = |f: &dyn Fn(usize) -> bool| (0..n).filter(|&i| f(i)).count();
    let shown = |i: usize| q1[i].is_some_and(|v| v <= 3.0);
    let in_table = |i: usize| shown(i) || q1[i].is_some_and(|v| v >= 98.0);
    assert!(
        count(&|i| in_table(i) && !shown(i)) > 0,
        "the fixture has hidden missing values"
    );

    let t = run(
        "survey_basic.sav",
        "CTABLES /TABLE q1_satisfaction [COUNT, COLPCT, COLPCT.TOTALN, TABLEPCT.TOTALN] BY gender\n\
         /CATEGORIES VARIABLES=q1_satisfaction [1, 2, 3] TOTAL=YES.",
    );
    let all = count(&in_table);
    for (row, k) in [1.0, 2.0, 3.0].into_iter().enumerate() {
        for g in 0..3 {
            let is_g = |i: usize| gender[i] == Some(g as f64 + 1.0);
            let cell = count(&|i| is_g(i) && q1[i] == Some(k));
            let expected = [
                Some(cell as f64),
                percent(cell, count(&|i| is_g(i) && shown(i))),
                percent(cell, count(&|i| is_g(i) && in_table(i))),
                percent(cell, all),
            ];
            assert_eq!(
                numbers(&t, row)[g * 4..g * 4 + 4],
                expected,
                "row {row} gender {g}"
            );
        }
    }
    // the total row counts the shown values; its Total N, the base of the .TOTALN
    // percentages, holds the hidden missing ones as well
    for g in 0..3 {
        let is_g = |i: usize| gender[i] == Some(g as f64 + 1.0);
        let (valid, total) = (
            count(&|i| is_g(i) && shown(i)),
            count(&|i| is_g(i) && in_table(i)),
        );
        let expected = [
            Some(valid as f64),
            Some(100.0),
            Some(100.0),
            percent(total, all),
        ];
        assert_eq!(
            numbers(&t, 3)[g * 4..g * 4 + 4],
            expected,
            "total, gender {g}"
        );
    }

    // TOTALN of a scale variable in the total of the categorical variable
    let t = run(
        "survey_basic.sav",
        "CTABLES /TABLE age [COUNT, TOTALN, VALIDN] BY q1_satisfaction\n\
         /CATEGORIES VARIABLES=q1_satisfaction [1, 2, 3] TOTAL=YES.",
    );
    let (valid, total) = (count(&shown) as f64, all as f64);
    assert_eq!(numbers(&t, 0)[9..], [Some(valid), Some(total), Some(valid)]);

    // system-missing values of a categorical variable in the columns count in the row's TOTALN
    let q6 = raw(&data, "q6_mention1");
    let t = run(
        "survey_basic.sav",
        "CTABLES /TABLE gender [COUNT, ROWPCT.TOTALN] BY q6_mention1 [C]\n\
         /CATEGORIES VARIABLES=q6_mention1 TOTAL=YES.",
    );
    for g in 0..3 {
        let is_g = |i: usize| gender[i] == Some(g as f64 + 1.0);
        let row = numbers(&t, g);
        for k in 0..6 {
            let cell = count(&|i| is_g(i) && q6[i] == Some(k as f64 + 1.0));
            assert_eq!(
                row[k * 2 + 1],
                percent(cell, count(&is_g)),
                "gender {g} q6 {k}"
            );
        }
        let valid = count(&|i| is_g(i) && q6[i].is_some());
        assert_eq!(row[12..], [Some(valid as f64), Some(100.0)]);
    }
}
