//! GRAPH: the numbers of its charts on a small made-up set of cafe receipts, with weights,
//! missing values in both roles, the MISSING options and SPLIT FILE.

use statscraft_engine::chart::{ChartKind, Grouping, Statistic, Tier};
use statscraft_engine::plot::Plot;
use statscraft_engine::{Cell, Chart, Engine};
use statscraft_syntax::parse;

/// store (9 = unknown, user-missing; one case system-missing), time of day (m morning,
/// e evening), the basket and the tip in euros (one tip missing), and a weight (0 for the
/// last case).
const DATA: &str = "DATA LIST LIST /store (F1) daypart (A1) basket (F5.1) tip (F4.1) wt (F3.1).
BEGIN DATA
1 m 12 1 1
1 e 20 2.5 2
2 m 8 0.5 1
2 m 15 . 1
2 e 30 4 1
3 e 10 1.5 0.5
9 m 25 3 1
. e 18 2 1
3 m 6 0 0
END DATA.
VALUE LABELS store 1 'Harbor' 2 'Market' 3 'Station' 9 'Unknown'.
MISSING VALUES store (9).
";

fn engine(source: &str) -> (Engine, Result<(), String>) {
    let (commands, errors) = parse(&format!("{DATA}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            return (engine, Err(e.to_string()));
        }
    }
    (engine, Ok(()))
}

/// The charts of the last command.
fn run(source: &str) -> Vec<Chart> {
    let (mut engine, result) = engine(source);
    result.unwrap_or_else(|e| panic!("{e}"));
    engine.take_charts()
}

fn one(source: &str) -> Chart {
    let mut charts = run(source);
    assert_eq!(charts.len(), 1, "{charts:?}");
    charts.remove(0)
}

fn error(source: &str) -> String {
    engine(source).1.expect_err("an error")
}

fn labels(chart: &Chart) -> Vec<&str> {
    chart.categories.iter().map(|c| c.label.as_str()).collect()
}

fn names(chart: &Chart) -> Vec<&str> {
    chart.series.iter().map(|s| s.name.as_str()).collect()
}

fn close(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?} against {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(
            (g - w).abs() < 1e-9 || (g.is_nan() && w.is_nan()),
            "{got:?} against {want:?}"
        );
    }
}

#[test]
fn counts_by_one_variable_leave_out_missing_categories() {
    let chart = one("GRAPH /BAR=COUNT BY store.");
    assert_eq!(
        (chart.kind, chart.tier, chart.grouping),
        (
            ChartKind::Bar,
            Tier::Presentation,
            Some(Grouping::Clustered)
        )
    );
    assert_eq!(chart.title, "Count by store");
    assert_eq!(chart.category_axis.title, "store");
    assert_eq!(chart.category_axis.variable.as_deref(), Some("store"));
    assert_eq!(chart.value_axis.title, "Count");
    assert_eq!(chart.value_axis.format, "F8.0");
    assert_eq!(labels(&chart), ["Harbor", "Market", "Station"]);
    assert_eq!(chart.categories[0].value, Cell::Number(1.0));
    assert_eq!(names(&chart), ["Count"]);
    assert_eq!(chart.series[0].statistic, Statistic::Count);
    close(&chart.series[0].values, &[2.0, 3.0, 2.0]);
    // a bare variable is the same chart
    assert_eq!(one("GRAPH /BAR=store."), chart);
}

#[test]
fn weights_count_and_percentages_are_of_the_whole_chart() {
    let chart = one("WEIGHT BY wt.\nGRAPH /BAR=COUNT BY store.");
    // the Station case with weight 0 is left out
    close(&chart.series[0].values, &[3.0, 3.0, 0.5]);
    assert_eq!(chart.value_axis.format, "F8.2");
    let chart = one("WEIGHT BY wt.\nGRAPH /BAR=PCT BY store.");
    close(
        &chart.series[0].values,
        &[300.0 / 6.5, 300.0 / 6.5, 50.0 / 6.5],
    );
    assert_eq!(chart.value_axis.format, "PCT8.1");
    assert_eq!(chart.series[0].statistic, Statistic::Percent);
    // the mean of the baskets weighted: Harbor (12 + 40) / 3
    let chart = one("WEIGHT BY wt.\nGRAPH /BAR=MEAN(basket) BY store.");
    close(&chart.series[0].values, &[52.0 / 3.0, 53.0 / 3.0, 10.0]);
}

#[test]
fn report_and_include_keep_missing_categories() {
    let chart = one("GRAPH /BAR=COUNT BY store /MISSING=REPORT.");
    assert_eq!(
        labels(&chart),
        ["Harbor", "Market", "Station", "Unknown", "System missing"]
    );
    let missing: Vec<bool> = chart.categories.iter().map(|c| c.missing).collect();
    assert_eq!(missing, [false, false, false, true, true]);
    close(&chart.series[0].values, &[2.0, 3.0, 2.0, 1.0, 1.0]);
    let chart = one("GRAPH /BAR=COUNT BY store /MISSING=INCLUDE.");
    assert_eq!(labels(&chart), ["Harbor", "Market", "Station", "Unknown"]);
    assert!(!chart.categories[3].missing);
}

#[test]
fn summary_functions_of_one_category() {
    let values = |function: &str| {
        one(&format!("GRAPH /BAR={function} BY store.")).series[0]
            .values
            .clone()
    };
    close(&values("MEAN(basket)"), &[16.0, 53.0 / 3.0, 8.0]);
    close(&values("SUM(basket)"), &[32.0, 53.0, 16.0]);
    close(&values("MEDIAN(basket)"), &[16.0, 15.0, 8.0]);
    close(&values("MIN(basket)"), &[12.0, 8.0, 6.0]);
    close(&values("MAX(basket)"), &[20.0, 30.0, 10.0]);
    close(&values("N(tip)"), &[2.0, 2.0, 2.0]);
    close(&values("NLT(basket 10)"), &[0.0, 1.0, 1.0]);
    close(&values("PGT(basket 10)"), &[100.0, 200.0 / 3.0, 0.0]);
    close(&values("PIN(basket, 8, 15)"), &[50.0, 200.0 / 3.0, 50.0]);
    // (13 - 16)^2 + ... : Harbor 12 and 20 have variance 32
    close(&values("VARIANCE(basket)")[..1], &[32.0]);
    close(&values("STDDEV(basket)")[..1], &[32f64.sqrt()]);
    // along the categories: 2, 2 + 3, 2 + 3 + 2 cases
    close(&values("CUFREQ"), &[2.0, 5.0, 7.0]);
    close(&values("CUPCT"), &[200.0 / 7.0, 500.0 / 7.0, 100.0]);
    close(&values("CUSUM(basket)"), &[32.0, 85.0, 101.0]);
    let chart = one("GRAPH /BAR=MEAN(basket) BY store.");
    assert_eq!(chart.title, "Mean of basket by store");
    assert_eq!(chart.value_axis.title, "Mean of basket");
    assert_eq!(chart.series[0].statistic, Statistic::Mean);
    assert_eq!(chart.series[0].variable.as_deref(), Some("basket"));
}

#[test]
fn listwise_and_variable_deletion() {
    // the Market case without a tip drops out of both means, or only out of the tips
    let chart = one("GRAPH /BAR=MEAN(basket tip) BY store.");
    assert_eq!(names(&chart), ["Mean of basket", "Mean of tip"]);
    assert_eq!(chart.value_axis.title, "Mean");
    close(&chart.series[0].values, &[16.0, 19.0, 8.0]);
    close(&chart.series[1].values, &[1.75, 2.25, 0.75]);
    let chart = one("GRAPH /BAR=MEAN(basket) MEAN(tip) BY store /MISSING=VARIABLE.");
    close(&chart.series[0].values, &[16.0, 53.0 / 3.0, 8.0]);
    close(&chart.series[1].values, &[1.75, 2.25, 0.75]);
}

#[test]
fn two_by_variables_make_a_series_per_value_of_the_second() {
    let chart = one("GRAPH /BAR(STACKED)=COUNT BY store BY daypart.");
    assert_eq!(chart.grouping, Some(Grouping::Stacked));
    assert_eq!(chart.legend.as_deref(), Some("daypart"));
    assert_eq!(names(&chart), ["e", "m"]);
    assert_eq!(chart.series[1].variable.as_deref(), Some("daypart"));
    assert_eq!(chart.series[1].value, Cell::Text("m".into()));
    close(&chart.series[0].values, &[1.0, 1.0, 1.0]);
    close(&chart.series[1].values, &[1.0, 2.0, 1.0]);
    assert_eq!(chart.title, "Count by store by daypart");
    // percentages of all seven cases
    let chart = one("GRAPH /BAR=PCT BY store BY daypart.");
    let total: f64 = chart.series.iter().flat_map(|s| s.values.clone()).sum();
    assert!((total - 100.0).abs() < 1e-9);
    // GROUPED and SIMPLE follow the data
    let chart = one("GRAPH /BAR(SIMPLE)=COUNT BY store BY daypart.");
    assert_eq!(
        (chart.series.len(), chart.grouping),
        (2, Some(Grouping::Clustered))
    );
}

#[test]
fn lines_areas_and_pies() {
    let chart = one("GRAPH /LINE=MEAN(basket) BY store.");
    assert_eq!((chart.kind, chart.grouping), (ChartKind::Line, None));
    let chart = one("GRAPH /LINE(AREA)=MEAN(basket) MEAN(tip) BY store.");
    assert_eq!(
        (chart.kind, chart.grouping),
        (ChartKind::Area, Some(Grouping::Stacked))
    );
    // without BY a slice per function; the case without a tip is left out of both
    let chart = one("GRAPH /PIE=SUM(basket) SUM(tip).");
    assert_eq!(
        (chart.kind, chart.orientation, chart.grouping),
        (ChartKind::Pie, None, None)
    );
    assert_eq!(labels(&chart), ["Sum of basket", "Sum of tip"]);
    close(&chart.series[0].values, &[129.0, 14.5]);
    assert_eq!(chart.value_axis.title, "Sum");
    let chart = one("GRAPH /PIE=store.");
    close(&chart.series[0].values, &[2.0, 3.0, 2.0]);
}

#[test]
fn value_draws_every_case() {
    let chart = one("GRAPH /BAR=VALUE(tip).");
    assert_eq!(chart.category_axis.title, "Case Number");
    assert_eq!(
        labels(&chart),
        ["1", "2", "3", "4", "5", "6", "7", "8", "9"]
    );
    close(
        &chart.series[0].values,
        &[1.0, 2.5, 0.5, f64::NAN, 4.0, 1.5, 3.0, 2.0, 0.0],
    );
    // the BY variable names the cases; a case with weight 0 is not drawn
    let chart = one("WEIGHT BY wt.\nGRAPH /BAR=VALUE(basket tip) BY store.");
    assert_eq!(labels(&chart).len(), 8);
    assert_eq!(labels(&chart)[..3], ["Harbor", "Harbor", "Market"]);
    assert_eq!(names(&chart), ["basket", "tip"]);
    assert_eq!(chart.value_axis.title, "Value");
}

#[test]
fn histogram_with_normal_curve_and_titles() {
    let chart = one(
        "GRAPH /HISTOGRAM(NORMAL)=basket /TITLE='Basket' 'sizes' /SUBTITLE='Spring' \
         /FOOTNOTE='Made-up receipts.'",
    );
    assert_eq!(chart.kind, ChartKind::Histogram);
    assert_eq!(chart.title, "Basket sizes");
    assert_eq!(chart.subtitle.as_deref(), Some("Spring"));
    let footnote = chart.footnote.as_deref().unwrap();
    assert!(
        footnote.starts_with("Made-up receipts. Mean = "),
        "{footnote}"
    );
    let normal = chart.normal.as_ref().unwrap();
    assert_eq!(normal.total, 9.0);
    assert!((normal.mean - 144.0 / 9.0).abs() < 1e-9);
    let total: f64 = chart.series[0].values.iter().sum();
    assert_eq!(total, 9.0);
    assert!(chart.data_table().caption.unwrap().contains("Spring."));
    let spec = chart.vega_lite();
    assert_eq!(spec["title"]["subtitle"][0], "Spring");
}

#[test]
fn scatterplot_groups_and_labels() {
    let chart = one("GRAPH /SCATTERPLOT=basket WITH tip BY daypart BY store (NAME).");
    assert_eq!(chart.kind, ChartKind::Scatter);
    assert_eq!(chart.title, "tip by basket");
    assert_eq!(chart.legend.as_deref(), Some("daypart"));
    let Some(Plot::Points { groups, .. }) = &chart.plot else {
        panic!("{chart:?}")
    };
    let group_names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(group_names, ["e", "m"]);
    let evening: Vec<(f64, f64, Option<&str>)> = groups[0]
        .points
        .iter()
        .map(|p| (p.x, p.y, p.label.as_deref()))
        .collect();
    // the system-missing store has no label
    assert_eq!(
        evening,
        [
            (20.0, 2.5, Some("Harbor")),
            (30.0, 4.0, Some("Market")),
            (10.0, 1.5, Some("Station")),
            (18.0, 2.0, None),
        ]
    );
    // the morning case without a tip is not drawn; a user-missing store still labels
    assert_eq!(groups[1].points.len(), 4);
    assert_eq!(groups[1].points[2].label.as_deref(), Some("Unknown"));
    let chart = one("GRAPH /SCATTERPLOT=basket WITH tip BY store (IDENTIFY).");
    let Some(Plot::Points { groups, .. }) = &chart.plot else {
        panic!()
    };
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].points.len(), 8);
    assert!(groups[0].points.iter().all(|p| p.label.is_none()));
}

#[test]
fn split_files_give_a_chart_per_group() {
    let charts = run("SORT CASES BY daypart.\nSPLIT FILE BY daypart.\nGRAPH /BAR=COUNT BY store.");
    let titles: Vec<&str> = charts.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Count by store (daypart = e)",
            "Count by store (daypart = m)"
        ]
    );
    close(&charts[0].series[0].values, &[1.0, 1.0, 1.0]);
    close(&charts[1].series[0].values, &[1.0, 2.0, 1.0]);
}

#[test]
fn charts_that_cannot_be_made() {
    assert_eq!(
        error("GRAPH /BAR=MEAN(daypart) BY store."),
        "GRAPH: daypart is a string variable; a summary function needs a numeric one"
    );
    assert_eq!(
        error("GRAPH /PIE=COUNT BY store BY daypart."),
        "GRAPH: a pie takes one function by one BY variable, or summary functions without BY"
    );
    assert_eq!(
        error("GRAPH /BAR=MEAN(basket tip) BY store BY daypart."),
        "GRAPH: with two BY variables the chart takes one function of one variable"
    );
    assert_eq!(
        error("SELECT IF store = 7.\nGRAPH /BAR=store."),
        "GRAPH: no valid cases to chart"
    );
    assert!(error("GRAPH /BAR=MEAN(nothing) BY store.").contains("nothing"));
}

/// Student's t exceeded with probability 0.025, for 1 and 2 degrees of freedom.
const T1: f64 = 12.706204736174698;
const T2: f64 = 4.302652729749464;

fn near(got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?} against {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!(
            (g - w).abs() < 1e-6 || (g.is_nan() && w.is_nan()),
            "{got:?} against {want:?}"
        );
    }
}

fn margins(chart: &Chart) -> &[Vec<f64>] {
    &chart.error_bars.as_ref().expect("error bars").margins
}

#[test]
fn error_bars_of_means() {
    // baskets: Harbor 12 and 20, Market 8, 15 and 30, Station 10 and 6
    let chart = one("GRAPH /ERRORBAR(STDDEV 1)=basket BY store.");
    assert_eq!(chart.kind, ChartKind::ErrorBar);
    assert_eq!(chart.kind_text(), "Simple error bar chart");
    assert_eq!(chart.title, "Mean of basket by store");
    close(&chart.series[0].values, &[16.0, 53.0 / 3.0, 8.0]);
    // squared deviations 32 (Harbor), 758 / 3 (Market) and 8 (Station), over n - 1
    close(
        &margins(&chart)[0],
        &[32f64.sqrt(), (379.0f64 / 3.0).sqrt(), 8f64.sqrt()],
    );
    assert_eq!(chart.error_bars.as_ref().unwrap().label, "1 SD");
    let chart = one("GRAPH /ERRORBAR(STERROR 2)=basket BY store.");
    close(&margins(&chart)[0], &[8.0, 2.0 * 379f64.sqrt() / 3.0, 4.0]);
    assert_eq!(chart.error_bars.as_ref().unwrap().label, "2 SE");
    let chart = one("GRAPH /ERRORBAR=basket BY store.");
    near(
        &margins(&chart)[0],
        &[T1 * 4.0, T2 * 379f64.sqrt() / 3.0, T1 * 2.0],
    );
    assert_eq!(chart.error_bars.as_ref().unwrap().label, "95% CI");
    // weighted: Harbor 12 once and 20 twice, variance 64 / 3 over W - 1 = 2, so the standard
    // error is 8 / 3; Station keeps a weight of 0.5, too little for a spread
    let chart = one("WEIGHT BY wt.\nGRAPH /ERRORBAR(SE 1)=basket BY store.");
    close(
        &margins(&chart)[0],
        &[8.0 / 3.0, 379f64.sqrt() / 3.0, f64::NAN],
    );
    // the picture draws the points; the office chart is a line chart of points
    assert!(chart.data_table().columns.len() == 3);
}

#[test]
fn clustered_error_bars() {
    let chart = one("GRAPH /ERRORBAR(CI 90)=basket tip BY daypart.");
    assert_eq!(chart.kind_text(), "Clustered error bar chart");
    assert_eq!(labels(&chart), ["e", "m"]);
    assert_eq!(names(&chart), ["Mean of basket", "Mean of tip"]);
    assert_eq!(margins(&chart).len(), 2);
    assert_eq!(chart.error_bars.as_ref().unwrap().label, "90% CI");
    let chart = one("GRAPH /ERRORBAR(STDDEV 2)=basket BY daypart BY store.");
    assert_eq!(names(&chart), ["Harbor", "Market", "Station"]);
    // evening Harbor has one case: no spread, no bar
    assert!(margins(&chart)[0][0].is_nan());
    // several variables without BY: a point per variable
    let chart = one("GRAPH /ERRORBAR=basket tip.");
    assert_eq!(labels(&chart), ["Mean of basket", "Mean of tip"]);
}

#[test]
fn pareto_bars_descend_with_the_cumulative_percent() {
    // Market 3, Harbor 2, Station 2: equal counts keep their order by value
    let chart = one("GRAPH /PARETO=store.");
    assert_eq!(chart.kind, ChartKind::Bar);
    assert_eq!(labels(&chart), ["Market", "Harbor", "Station"]);
    assert_eq!(names(&chart), ["Count", "Cumulative Percent"]);
    close(&chart.series[0].values, &[3.0, 2.0, 2.0]);
    close(&chart.series[1].values, &[300.0 / 7.0, 500.0 / 7.0, 100.0]);
    let second = chart.second_axis.as_ref().expect("a second axis");
    assert_eq!(
        (second.maximum, second.format.as_str()),
        (Some(100.0), "PCT8.1")
    );
    assert_eq!(chart.kind_text(), "Bar chart with a line on a second axis");
    let table = chart.data_table();
    assert_eq!(table.formats[0][2], "PCT8.1");
    let chart = one("GRAPH /PARETO(NOCUM)=store.");
    assert!(chart.second_axis.is_none());
    assert_eq!(names(&chart), ["Count"]);
    // sums of baskets: Market 23 + 30, Harbor 12 + 20, Station 6 + 10
    let chart = one("GRAPH /PARETO(STACKED)=SUM(basket) BY store BY daypart.");
    assert_eq!(chart.grouping, Some(Grouping::Stacked));
    assert_eq!(labels(&chart), ["Market", "Harbor", "Station"]);
    assert_eq!(names(&chart), ["e", "m", "Cumulative Percent"]);
    close(&chart.series[0].values, &[30.0, 20.0, 10.0]);
    close(&chart.series[1].values, &[23.0, 12.0, 6.0]);
    close(
        &chart.series[2].values,
        &[5300.0 / 101.0, 8500.0 / 101.0, 100.0],
    );
    assert_eq!(
        chart.kind_text(),
        "Stacked bar chart with a line on a second axis"
    );
    // a bar per case, largest first; the missing tip counts as nothing
    let chart = one("GRAPH /PARETO=VALUE(tip).");
    assert_eq!(
        labels(&chart),
        ["5", "7", "2", "8", "6", "1", "3", "4", "9"]
    );
    // tips add up to 14.5, the first two to 7
    close(&chart.series[1].values[1..2], &[700.0 / 14.5]);
    // Vega-Lite: the line on its own scale
    let spec = chart.vega_lite();
    assert_eq!(spec["resolve"]["scale"]["y"], "independent");
    assert_eq!(spec["layer"][1]["transform"][0]["filter"], "datum.second");
}

#[test]
fn range_drop_difference_and_high_low_charts() {
    let chart = one("GRAPH /BAR(RANGE)=MIN(basket) MAX(basket) BY store.");
    assert_eq!((chart.kind, chart.grouping), (ChartKind::RangeBar, None));
    close(&chart.series[0].values, &[12.0, 8.0, 6.0]);
    close(&chart.series[1].values, &[20.0, 30.0, 10.0]);
    let spec = chart.vega_lite();
    assert_eq!(spec["mark"]["type"], "bar");
    assert_eq!(spec["data"]["values"][1]["high"], 30.0);
    // a second BY variable with two values gives the pair
    let chart = one("GRAPH /BAR(RANGE)=SUM(basket) BY store BY daypart.");
    assert_eq!(names(&chart), ["e", "m"]);
    let chart = one("GRAPH /LINE(DROP)=MIN(basket) MEAN(basket) MAX(basket) BY daypart.");
    assert_eq!(chart.kind, ChartKind::DropLine);
    assert_eq!(chart.series.len(), 3);
    let spec = chart.vega_lite();
    assert_eq!(spec["layer"][0]["mark"]["type"], "rule");
    // evening baskets 20, 30, 10 and 18; morning 12, 8, 15, 25 and 6
    assert_eq!(spec["layer"][0]["data"]["values"][0]["low"], 10.0);
    assert_eq!(spec["layer"][0]["data"]["values"][1]["high"], 25.0);
    let chart = one("GRAPH /LINE(DIFFERENCE)=SUM(basket) BY store BY daypart.");
    assert_eq!(chart.kind, ChartKind::Difference);
    let chart = one("GRAPH /HILO=MAX(basket) MIN(basket) MEAN(basket) BY store.");
    assert_eq!(chart.kind, ChartKind::HighLow);
    assert_eq!(chart.kind_text(), "High-low-close chart");
    close(&chart.series[2].values, &[16.0, 53.0 / 3.0, 8.0]);
    assert_eq!(
        error("GRAPH /BAR(RANGE)=MEAN(basket) BY store."),
        "GRAPH: BAR(RANGE) takes its ends in pairs: an even number of functions or variables, \
         or a second BY variable with two values; this chart has 1"
    );
    assert_eq!(
        error("GRAPH /LINE(DIFFERENCE)=MIN(basket) MEAN(basket) MAX(basket) BY store."),
        "GRAPH: LINE(DIFFERENCE) needs two series: two functions, or a second BY variable with \
         two values; this chart has 3"
    );
    assert_eq!(
        error("GRAPH /HILO=MAX(basket) MIN(basket) MAX(tip) MIN(tip) BY store."),
        "GRAPH: HILO with 4 series (GROUPED) is not supported yet; give two or three"
    );
}

#[test]
fn panels_give_a_chart_per_combination() {
    let charts = run("GRAPH /BAR=COUNT BY store /PANEL COLVAR=daypart.");
    let titles: Vec<&str> = charts.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Count by store (daypart = e)",
            "Count by store (daypart = m)"
        ]
    );
    close(&charts[0].series[0].values, &[1.0, 1.0, 1.0]);
    close(&charts[1].series[0].values, &[1.0, 2.0, 1.0]);
    // weights hold within a panel; the Station morning case weighs nothing
    let charts = run("WEIGHT BY wt.\nGRAPH /BAR=COUNT BY store /PANEL ROWVAR=daypart.");
    close(&charts[0].series[0].values, &[2.0, 1.0, 0.5]);
    assert_eq!(labels(&charts[1]), ["Harbor", "Market"]);
    // rows before columns; missing panel values only with REPORT
    let charts = run("GRAPH /BAR=COUNT BY daypart /PANEL COLVAR=store ROWVAR=daypart.");
    assert_eq!(charts.len(), 6);
    assert_eq!(
        charts[1].title,
        "Count by daypart (daypart = e, store = Market)"
    );
    let charts = run("GRAPH /HISTOGRAM=basket /PANEL COLVAR=store /MISSING=REPORT.");
    assert_eq!(charts.len(), 5);
    assert!(charts[4].title.ends_with("(store = System missing)"));
    // a panel without a valid value is left out with a warning
    let (mut engine, result) = engine("GRAPH /BAR=MEAN(tip) BY daypart /PANEL COLVAR=basket.");
    result.unwrap();
    assert_eq!(engine.take_charts().len(), 8);
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.starts_with("basket = 15: ")),
        "{:?}",
        engine.warnings()
    );
}
