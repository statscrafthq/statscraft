//! Charts of FREQUENCIES and CROSSTABS: their exact contents on a small made-up bike-share
//! survey, with weights, missing values, SET TNUMBERS and SPLIT FILE.

use statscraft_engine::chart::{ChartKind, Grouping, Statistic, Tier};
use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Chart, Engine};
use statscraft_syntax::parse;

/// trips per month, home zone (9 = no answer, user-missing), plan (m monthly, y yearly),
/// a weight and the survey wave (missing for the last case).
const DATA: &str = "DATA LIST LIST /trips zone (F2) plan (A1) wt (F3.1) wave (F1).
BEGIN DATA
3 1 m 1 1
5 1 y 2 1
8 2 m 1 1
12 2 m 1.5 1
2 3 y 1 1
7 9 y 1 2
15 1 m 0.5 2
9 2 y 1 2
. 3 m 1 2
4 2 y 2 .
END DATA.
VALUE LABELS zone 1 'North' 2 'South' 3 'West' 9 'No answer'.
MISSING VALUES zone (9).
";

/// The charts of the last command, and the warnings.
fn run(source: &str) -> (Vec<Chart>, Vec<String>) {
    let (commands, errors) = parse(&format!("{DATA}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut charts = Vec::new();
    for command in &commands {
        engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
        charts = engine.take_charts();
    }
    (charts, engine.warnings().to_vec())
}

fn labels(chart: &Chart) -> Vec<&str> {
    chart.categories.iter().map(|c| c.label.as_str()).collect()
}

fn values(chart: &Chart, series: usize) -> Vec<f64> {
    chart.series[series].values.clone()
}

fn close(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?} against {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 1e-9, "{got:?} against {want:?}");
    }
}

#[test]
fn bar_chart_counts_the_valid_values_in_table_order() {
    let (charts, warnings) = run("FREQUENCIES zone /BARCHART.");
    assert!(warnings.is_empty(), "{warnings:?}");
    let [chart] = &charts[..] else {
        panic!("{charts:?}")
    };
    assert_eq!(
        (chart.kind, chart.tier),
        (ChartKind::Bar, Tier::Presentation)
    );
    assert_eq!(chart.grouping, Some(Grouping::Clustered));
    assert_eq!(chart.title, "zone");
    assert_eq!(chart.category_axis.title, "zone");
    assert_eq!(chart.category_axis.variable.as_deref(), Some("zone"));
    assert_eq!(chart.value_axis.title, "Frequency");
    assert_eq!(chart.value_axis.format, "F8.0");
    // the missing value 9 has no bar
    assert_eq!(labels(chart), ["North", "South", "West"]);
    let codes: Vec<&Cell> = chart.categories.iter().map(|c| &c.value).collect();
    assert_eq!(
        codes,
        [&Cell::Number(1.0), &Cell::Number(2.0), &Cell::Number(3.0)]
    );
    assert_eq!(chart.series[0].statistic, Statistic::Count);
    close(&values(chart, 0), &[3.0, 4.0, 2.0]);
    assert_eq!(chart.kind_text(), "Simple bar chart");

    // percentages of the 9 valid cases; MIN leaves out North; DFREQ orders the bars
    let (charts, _) = run("FREQUENCIES zone /BARCHART=PERCENT MIN(2) /FORMAT=DFREQ.");
    assert_eq!(labels(&charts[0]), ["South", "West"]);
    assert_eq!(charts[0].series[0].statistic, Statistic::Percent);
    assert_eq!(charts[0].value_axis.format, "PCT8.1");
    close(&values(&charts[0], 0), &[400.0 / 9.0, 200.0 / 9.0]);

    // FREQ(n) sets the top of the axis; one below the highest bar is ignored
    let (charts, _) = run("FREQUENCIES zone /BARCHART=FREQ(10).");
    assert_eq!(charts[0].value_axis.maximum, Some(10.0));
    let (charts, _) = run("FREQUENCIES zone /BARCHART=FREQ(3).");
    assert_eq!(charts[0].value_axis.maximum, None);
}

#[test]
fn labels_follow_set_tnumbers_tvars_and_ovars() {
    let (charts, _) = run(
        "VARIABLE LABELS zone 'Home zone'.\nSET TNUMBERS=BOTH TVARS=NAMES OVARS=BOTH.\nFREQUENCIES zone /BARCHART.",
    );
    assert_eq!(labels(&charts[0]), ["1 North", "2 South", "3 West"]);
    assert_eq!(charts[0].category_axis.title, "zone");
    assert_eq!(charts[0].title, "zone Home zone");
}

#[test]
fn weighted_pie_has_a_slice_for_the_missing_values() {
    let (charts, _) = run("WEIGHT BY wt.\nFREQUENCIES zone /PIECHART /FORMAT=NOTABLE.");
    let chart = &charts[0];
    assert_eq!(chart.kind, ChartKind::Pie);
    assert_eq!((chart.orientation, chart.grouping), (None, None));
    assert_eq!(labels(chart), ["North", "South", "West", "Missing"]);
    assert!(chart.categories[3].missing);
    // weighted counts that are not whole show with decimals
    assert_eq!(chart.value_axis.format, "F8.2");
    close(&values(chart, 0), &[3.5, 5.5, 2.0, 1.0]);

    // percentages of the whole pie, 12
    let (charts, _) = run("WEIGHT BY wt.\nFREQUENCIES zone /PIECHART=PERCENT.");
    close(
        &values(&charts[0], 0),
        &[350.0 / 12.0, 550.0 / 12.0, 200.0 / 12.0, 100.0 / 12.0],
    );
    let (charts, _) = run("WEIGHT BY wt.\nFREQUENCIES zone /PIECHART=NOMISSING.");
    assert_eq!(labels(&charts[0]), ["North", "South", "West"]);
    // INCLUDE makes 9 a valid value with a slice of its own
    let (charts, _) = run("FREQUENCIES zone /PIECHART /MISSING=INCLUDE.");
    assert_eq!(labels(&charts[0]), ["North", "South", "West", "No answer"]);
    close(&values(&charts[0], 0), &[3.0, 4.0, 2.0, 1.0]);
}

#[test]
fn histogram_bins_and_normal_curve() {
    let (charts, warnings) = run("FREQUENCIES trips plan /HISTOGRAM=NORMAL /FORMAT=NOTABLE.");
    // a string variable gets none
    assert_eq!(warnings, ["plan: a string variable gets no histogram"]);
    let [chart] = &charts[..] else {
        panic!("{charts:?}")
    };
    assert_eq!(
        (chart.kind, chart.tier),
        (ChartKind::Histogram, Tier::Statistical)
    );
    // 9 valid values from 2 to 15: Sturges gives 5 bins of 2.6, moved to 2, from 2
    assert_eq!(chart.categories.len(), 7);
    assert_eq!(labels(chart)[..2], ["2 to under 4", "4 to under 6"]);
    assert_eq!(chart.categories[6].bin, Some([14.0, 16.0]));
    assert_eq!(chart.categories[6].value, Cell::Number(15.0));
    close(&values(chart, 0), &[2.0, 2.0, 1.0, 2.0, 0.0, 1.0, 1.0]);
    let normal = chart.normal.as_ref().unwrap();
    // mean 65 / 9; variance (617 - 65^2 / 9) / 8
    let sd = ((617.0 - 65.0 * 65.0 / 9.0) / 8.0_f64).sqrt();
    close(
        &[normal.mean, normal.std_dev, normal.total],
        &[65.0 / 9.0, sd, 9.0],
    );
    assert_eq!(
        chart.footnote.as_deref(),
        Some("Mean = 7.22, Std. Dev. = 4.29, N = 9")
    );
    assert_eq!(chart.kind_text(), "Histogram with a normal curve");

    // MIN and MAX bound the bars, not the curve
    let (charts, _) = run("FREQUENCIES trips /HISTOGRAM=NORMAL MIN(5) MAX(12) FREQ(4).");
    let chart = &charts[0];
    assert_eq!(chart.categories[0].bin, Some([4.0, 6.0]));
    close(&values(chart, 0), &[1.0, 1.0, 2.0, 0.0, 1.0]);
    close(&[chart.normal.as_ref().unwrap().mean], &[65.0 / 9.0]);
    assert_eq!(chart.value_axis.maximum, Some(4.0));
    // without NORMAL there is no curve
    let (charts, _) = run("WEIGHT BY wt.\nFREQUENCIES trips /HISTOGRAM.");
    assert_eq!(charts[0].normal, None);
    close(&values(&charts[0], 0), &[2.0, 4.0, 1.0, 2.0, 0.0, 1.5, 0.5]);
}

#[test]
fn crosstabs_bar_chart_clusters_rows_and_sums_layers() {
    let (charts, _) = run("CROSSTABS zone BY plan /BARCHART.");
    let [chart] = &charts[..] else {
        panic!("{charts:?}")
    };
    assert_eq!(chart.title, "zone by plan");
    assert_eq!(chart.legend.as_deref(), Some("plan"));
    assert_eq!(chart.value_axis.title, "Count");
    assert_eq!(labels(chart), ["North", "South", "West"]);
    let names: Vec<&str> = chart.series.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["m", "y"]);
    assert_eq!(chart.series[1].value, Cell::Text("y".into()));
    assert_eq!(chart.series[1].variable.as_deref(), Some("plan"));
    close(&values(chart, 0), &[2.0, 2.0, 1.0]);
    close(&values(chart, 1), &[1.0, 2.0, 1.0]);
    assert_eq!(chart.kind_text(), "Clustered bar chart");

    // a layer variable is summed over; the case without a wave drops out as in the table
    let (charts, _) = run("CROSSTABS zone BY plan BY wave /BARCHART /FORMAT=DVALUE.");
    assert_eq!(charts.len(), 1);
    assert_eq!(labels(&charts[0]), ["West", "South", "North"]);
    close(&values(&charts[0], 1), &[1.0, 1.0, 1.0]);

    let (charts, _) = run("WEIGHT BY wt.\nCROSSTABS zone BY plan /BARCHART /CELLS=NONE.");
    close(&values(&charts[0], 0), &[1.5, 2.5, 1.0]);
    close(&values(&charts[0], 1), &[2.0, 3.0, 1.0]);
    // no chart without BARCHART
    assert!(run("CROSSTABS zone BY plan.").0.is_empty());
}

#[test]
fn split_file_gives_a_chart_per_group() {
    let (charts, _) =
        run("SORT CASES BY plan.\nSPLIT FILE SEPARATE BY plan.\nFREQUENCIES zone /BARCHART.");
    let titles: Vec<&str> = charts.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, ["zone (plan = m)", "zone (plan = y)"]);
    close(&values(&charts[0], 0), &[2.0, 2.0, 1.0]);
    close(&values(&charts[1], 0), &[1.0, 2.0, 1.0]);
}

#[test]
fn data_table_shows_the_numbers_with_a_caption() {
    let (charts, _) = run("CROSSTABS zone BY plan /BARCHART.");
    let table = charts[0].data_table();
    assert_eq!(table.columns, ["zone", "m", "y"]);
    assert_eq!(table.column_groups[1], ["plan"]);
    assert_eq!(table.rows[1][2], Cell::Number(2.0));
    assert_eq!(table.caption.as_deref(), Some("Clustered bar chart."));
    let (charts, _) = run("FREQUENCIES trips /HISTOGRAM.");
    let caption = charts[0].data_table().caption.unwrap();
    assert_eq!(caption, "Histogram. Mean = 7.22, Std. Dev. = 4.29, N = 9");
}

#[test]
fn json_carries_charts_only_when_there_are_some() {
    let mut session = Session::default();
    let source = format!("{DATA}FREQUENCIES zone /BARCHART.");
    let result = session.run(&source, None).unwrap();
    let json: serde_json::Value = serde_json::from_str(&result.output(true, false)).unwrap();
    let commands = json["commands"].as_array().unwrap();
    assert!(
        commands[..commands.len() - 1]
            .iter()
            .all(|c| c.get("charts").is_none())
    );
    let chart = &commands[commands.len() - 1]["charts"][0];
    assert_eq!(chart["kind"], "bar");
    assert_eq!(chart["tier"], "presentation");
    assert_eq!(chart["categories"][1]["label"], "South");
    assert_eq!(
        chart["series"][0]["values"],
        serde_json::json!([3.0, 4.0, 2.0])
    );
    assert!(chart.get("vega_lite").is_none());
    // the description reads back
    let back: Chart = serde_json::from_value(chart.clone()).unwrap();
    assert_eq!(back, result.commands.last().unwrap().charts[0]);
    let with = result.to_json(true);
    let spec = &with["commands"][commands.len() - 1]["charts"][0]["vega_lite"];
    assert_eq!(spec["mark"]["type"], "bar");
    // Markdown: the data table with its caption, and the specification on request
    let text = result.output(false, false);
    assert!(text.contains("| zone | Frequency |"), "{text}");
    assert!(text.contains("| South | 4 |"), "{text}");
    assert!(text.contains("Simple bar chart."), "{text}");
    assert!(!text.contains("```json"));
    assert!(result.output_with(false, false, true).contains("```json"));
}
