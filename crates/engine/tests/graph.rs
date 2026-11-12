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
