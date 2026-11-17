//! IGRAPH: the numbers of its charts on small made-up workshop data, worked out by hand.

use statscraft_engine::chart::{ChartKind, Grouping, Orientation, Statistic};
use statscraft_engine::plot::{Plot, ReferenceLine};
use statscraft_engine::{Chart, Engine};
use statscraft_syntax::parse;

/// team (9 = unknown, user-missing), shift, pieces made, hours and the hall.
const DATA: &str = "DATA LIST LIST /team (F1) shift (A1) output (F4.0) hours (F4.1) hall (F1).
BEGIN DATA
1 a 10 1 1
1 a 14 2 1
1 b 20 3 2
1 b 22 4 2
2 a 6 1 1
2 a 8 2 2
2 b 30 3 1
2 b 26 4 2
2 b 25 2 1
9 a 40 5 1
END DATA.
VALUE LABELS team 1 'North' 2 'South' 9 'Unknown'.
MISSING VALUES team (9).
";

fn engine(source: &str) -> (Engine, Result<(), String>) {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            return (engine, Err(e.to_string()));
        }
    }
    (engine, Ok(()))
}

fn run(source: &str) -> Vec<Chart> {
    let (mut engine, result) = engine(&format!("{DATA}{source}"));
    result.unwrap_or_else(|e| panic!("{e}"));
    engine.take_charts()
}

fn one(source: &str) -> Chart {
    let mut charts = run(source);
    assert_eq!(charts.len(), 1, "{charts:?}");
    charts.remove(0)
}

fn error(source: &str) -> String {
    engine(&format!("{DATA}{source}")).1.expect_err("an error")
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
        assert!((g - w).abs() < 1e-9, "{got:?} against {want:?}");
    }
}

#[test]
fn clustered_bar_of_means_with_color() {
    let chart = one(
        "IGRAPH /VIEWNAME='Bar Chart' /X1=VAR(team) TYPE=CATEGORICAL /Y=VAR(output) TYPE=SCALE \
         /COLOR=VAR(shift) TYPE=CATEGORICAL /COORDINATE=VERTICAL /X1LENGTH=3.0 /YLENGTH=3.0 \
         /CHARTLOOK='NONE' /CATORDER VAR(team) (ASCENDING VALUES OMITEMPTY) \
         /BAR(MEAN) KEY=ON SHAPE=RECTANGLE BASELINE=AUTO.",
    );
    assert_eq!(
        (chart.kind, chart.grouping, chart.orientation),
        (
            ChartKind::Bar,
            Some(Grouping::Clustered),
            Some(Orientation::Vertical)
        )
    );
    // the unknown team is user-missing and left out
    assert_eq!(labels(&chart), ["North", "South"]);
    assert_eq!(names(&chart), ["a", "b"]);
    // North a: 10, 14; South a: 6, 8; North b: 20, 22; South b: 30, 26, 25
    close(&chart.series[0].values, &[12.0, 7.0]);
    close(&chart.series[1].values, &[21.0, 27.0]);
    assert_eq!(chart.series[0].statistic, Statistic::Mean);
    assert_eq!(chart.outline.as_deref(), Some("Bar Chart"));
    assert_eq!(chart.title, "Mean of output by team by shift");
}

#[test]
fn appearance_keywords_are_named_in_one_warning() {
    let (engine, result) = engine(&format!(
        "{DATA}IGRAPH /X1=VAR(team) /Y=$count /X1LENGTH=3 /CHARTLOOK='NONE' /EFFECT=THREE \
         /BAR KEY=ON LABEL INSIDE N BASELINE=AUTO."
    ));
    result.unwrap();
    let warnings: Vec<&String> = engine
        .warnings()
        .iter()
        .filter(|w| w.starts_with("IGRAPH"))
        .collect();
    assert_eq!(
        warnings,
        [
            "IGRAPH: X1LENGTH, CHARTLOOK, EFFECT=THREE, LABEL, BASELINE only change the look of \
             the chart and are ignored"
        ]
    );
}

#[test]
fn stacked_counts_horizontal_and_titles() {
    let chart = one(
        "IGRAPH /X1=VAR(team) TYPE=CATEGORICAL TITLE='Team' /Y=$count TITLE='Workers' \
         /COLOR=VAR(shift) TYPE=CATEGORICAL TITLE='Shift' STACK /COORDINATE=HORIZONTAL \
         /TITLE='Workers\\nper team' /CAPTION='Made up.' /BAR.",
    );
    assert_eq!(chart.grouping, Some(Grouping::Stacked));
    assert_eq!(chart.orientation, Some(Orientation::Horizontal));
    close(&chart.series[0].values, &[2.0, 2.0]);
    close(&chart.series[1].values, &[2.0, 3.0]);
    assert_eq!(chart.series[0].statistic, Statistic::Count);
    assert_eq!(chart.title, "Workers per team");
    assert_eq!(chart.footnote.as_deref(), Some("Made up."));
    assert_eq!(chart.category_axis.title, "Team");
    assert_eq!(chart.value_axis.title, "Workers");
    assert_eq!(chart.legend.as_deref(), Some("Shift"));
    // NORMALIZE stacks to 100%
    let chart = one("IGRAPH /X1=VAR(team) /Y=$count /COLOR=VAR(shift) STACK /NORMALIZE /BAR.");
    assert_eq!(chart.grouping, Some(Grouping::Percent));
}

#[test]
fn pies_of_counts_and_sums() {
    let chart = one(
        "IGRAPH /SUMMARYVAR=$count /COLOR=VAR(team) TYPE=CATEGORICAL /PIE START 0 CW SLICE=INSIDE LABEL PCT N.",
    );
    assert_eq!(chart.kind, ChartKind::Pie);
    assert_eq!(labels(&chart), ["North", "South"]);
    close(&chart.series[0].values, &[4.0, 5.0]);
    // REPORT keeps the user-missing team as a slice
    let chart = one("IGRAPH /SUMMARYVAR=$count /COLOR=VAR(team) /MISSING=REPORT /PIE.");
    assert_eq!(labels(&chart), ["North", "South", "Unknown"]);
    close(&chart.series[0].values, &[4.0, 5.0, 1.0]);
    // INCLUDE makes it valid
    let chart = one("IGRAPH /SUMMARYVAR=$count /COLOR=VAR(team) /MISSING=INCLUDE /PIE.");
    close(&chart.series[0].values, &[4.0, 5.0, 1.0]);
    // sums of output: 66 and 95; with $PCT the shares of the 9 workers
    let chart = one("IGRAPH /SUMMARYVAR=VAR(output) /COLOR=VAR(team) /PIE (SUM).");
    close(&chart.series[0].values, &[66.0, 95.0]);
    let chart = one("IGRAPH /SUMMARYVAR=$pct /COLOR=VAR(team) /PIE.");
    close(&chart.series[0].values, &[400.0 / 9.0, 500.0 / 9.0]);
    // a pie per shift
    let charts = run("IGRAPH /SUMMARYVAR=$count /COLOR=VAR(team) /X1=VAR(shift) /PIE.");
    assert_eq!(charts.len(), 2);
    close(&charts[0].series[0].values, &[2.0, 2.0]);
    close(&charts[1].series[0].values, &[2.0, 3.0]);
    assert!(error("IGRAPH /SUMMARYVAR=VAR(output) /COLOR=VAR(team) /PIE.").contains("PIE (SUM)"));
}

#[test]
fn line_and_area_with_two_series() {
    let chart = one(
        "IGRAPH /X1=VAR(team) /Y=VAR(output) /COLOR=VAR(shift) /LINE(MEAN) STYLE=DOTLINE \
         DROPLINE=OFF INTERPOLATE=STRAIGHT BREAK=MISSING.",
    );
    assert_eq!(chart.kind, ChartKind::Line);
    assert_eq!(names(&chart), ["a", "b"]);
    close(&chart.series[0].values, &[12.0, 7.0]);
    close(&chart.series[1].values, &[21.0, 27.0]);
    // maxima by team: 22 and 30, the areas stacked
    let chart = one("IGRAPH /X1=VAR(team) /Y=VAR(output) /COLOR=VAR(shift) /AREA(MAXIMUM).");
    assert_eq!(
        (chart.kind, chart.grouping),
        (ChartKind::Area, Some(Grouping::Stacked))
    );
    close(&chart.series[0].values, &[14.0, 8.0]);
    close(&chart.series[1].values, &[22.0, 30.0]);
}

#[test]
fn summary_functions_of_igraph() {
    let values = |function: &str| {
        let chart = one(&format!(
            "IGRAPH /X1=VAR(team) /Y=VAR(output) /BAR({function})."
        ));
        chart.series[0].values.clone()
    };
    // North 10 14 20 22, South 6 8 30 26 25
    close(&values("FIRST"), &[10.0, 6.0]);
    close(&values("LAST"), &[22.0, 25.0]);
    close(&values("SUMAV"), &[66.0, 95.0]);
    close(
        &values("SUMSQ"),
        &[
            100.0 + 196.0 + 400.0 + 484.0,
            36.0 + 64.0 + 900.0 + 676.0 + 625.0,
        ],
    );
    close(&values("NGE(20)"), &[2.0, 3.0]);
    close(&values("NLE(10)"), &[1.0, 2.0]);
    close(&values("NEQ(26)"), &[0.0, 1.0]);
    close(&values("PGE(25)"), &[0.0, 60.0]);
    // squared deviations 91 and 496 over n - 1
    close(
        &values("SEMEAN"),
        &[
            (91.0f64 / 3.0).sqrt() / 2.0,
            (124.0f64).sqrt() / 5f64.sqrt(),
        ],
    );
    close(&values("PTILE(50)"), &[17.0, 25.0]);
}

#[test]
fn box_plot_with_outliers_and_extremes() {
    let source = "DATA LIST LIST /g (F1) v (F3.0).
BEGIN DATA
1 1
1 2
1 3
1 4
1 5
1 6
1 7
1 8
1 20
1 60
2 10
2 14
2 20
2 22
END DATA.
";
    let box_of = |extra: &str| {
        let (mut engine, result) = engine(&format!(
            "{source}IGRAPH /X1=VAR(g) TYPE=CATEGORICAL /Y=VAR(v) TYPE=SCALE /BOX {extra}."
        ));
        result.unwrap();
        let mut charts = engine.take_charts();
        assert_eq!(charts.len(), 1);
        let chart = charts.remove(0);
        assert_eq!(chart.kind, ChartKind::Boxplot);
        let Some(Plot::Boxes { boxes }) = chart.plot else {
            panic!()
        };
        boxes
    };
    let boxes = box_of("OUTLIERS=ON EXTREME=ON MEDIAN=ON WHISKER=T CAPWIDTH (45)");
    // group 1: hinges at depths 3, 5.5 and 8 give 3, 5.5 and 8; a box length is 5, so 20 lies
    // between 1.5 and 3 lengths above (outlier) and 60 further (extreme)
    let b = &boxes[0];
    assert_eq!(b.label, "1");
    assert_eq!(
        [
            b.lower_whisker,
            b.lower_hinge,
            b.median,
            b.upper_hinge,
            b.upper_whisker
        ],
        [1.0, 3.0, 5.5, 8.0, 8.0]
    );
    assert_eq!(b.n, 10.0);
    assert_eq!(
        (b.outliers[0].value, b.outliers[0].case.as_str()),
        (20.0, "9")
    );
    assert_eq!(
        (b.extremes[0].value, b.extremes[0].case.as_str()),
        (60.0, "10")
    );
    // group 2: hinges at depths 1.5, 2.5 and 3.5
    let b = &boxes[1];
    assert_eq!(
        [
            b.lower_whisker,
            b.lower_hinge,
            b.median,
            b.upper_hinge,
            b.upper_whisker
        ],
        [10.0, 12.0, 17.0, 21.0, 22.0]
    );
    let boxes = box_of("OUTLIERS=OFF EXTREME=OFF");
    assert!(boxes[0].outliers.is_empty() && boxes[0].extremes.is_empty());
    assert_eq!(boxes[0].upper_whisker, 8.0);
}

#[test]
fn clustered_boxes_follow_catorder() {
    let chart = one("IGRAPH /X1=VAR(team) /Y=VAR(output) /COLOR=VAR(shift) \
         /CATORDER VAR(team) (DESCENDING VALUES) /BOX.");
    let Some(Plot::Boxes { boxes }) = &chart.plot else {
        panic!()
    };
    let cells: Vec<(&str, Option<&str>, f64)> = boxes
        .iter()
        .map(|b| (b.label.as_str(), b.group.as_deref(), b.median))
        .collect();
    assert_eq!(
        cells,
        [
            ("South", Some("a"), 7.0),
            ("South", Some("b"), 26.0),
            ("North", Some("a"), 12.0),
            ("North", Some("b"), 21.0)
        ]
    );
    assert_eq!(chart.legend.as_deref(), Some("shift"));
}

#[test]
fn error_bars_alone_and_on_bars() {
    let chart = one(
        "IGRAPH /X1=VAR(team) /Y=VAR(output) /ERRORBAR SD(2.0) CAPWIDTH (45) CAPSTYLE=T SYMBOL=ON.",
    );
    assert_eq!(chart.kind, ChartKind::ErrorBar);
    // North: mean 16.5, squares 91 over 3; South: mean 19, squares 496 over 4
    close(&chart.series[0].values, &[16.5, 19.0]);
    let bars = chart.error_bars.as_ref().unwrap();
    assert_eq!(bars.label, "2 SD");
    close(
        &bars.margins[0],
        &[2.0 * (91.0f64 / 3.0).sqrt(), 2.0 * 124f64.sqrt()],
    );
    // a 95% interval: t with 3 and 4 degrees of freedom times the standard error
    let chart = one("IGRAPH /X1=VAR(team) /Y=VAR(output) /ERRORBAR CI(95).");
    let margins = &chart.error_bars.as_ref().unwrap().margins[0];
    let want = [
        3.182446305284263 * (91.0f64 / 3.0).sqrt() / 2.0,
        2.7764451051977987 * 124f64.sqrt() / 5f64.sqrt(),
    ];
    assert!((margins[0] - want[0]).abs() < 1e-6 && (margins[1] - want[1]).abs() < 1e-6);
    // on a bar chart of means: one standard error
    let chart = one("IGRAPH /X1=VAR(team) /Y=VAR(output) /BAR(MEAN) /ERRORBAR SE(1).");
    assert_eq!(chart.kind, ChartKind::Bar);
    close(
        &chart.error_bars.as_ref().unwrap().margins[0],
        &[(91.0f64 / 3.0).sqrt() / 2.0, 124f64.sqrt() / 5f64.sqrt()],
    );
}

#[test]
fn histograms_of_counts_percentages_and_running_totals() {
    let source = "DATA LIST LIST /v (F2.0).
BEGIN DATA
1
2
2
3
7
8
END DATA.
";
    let histogram = |command: &str| {
        let (mut engine, result) = engine(&format!("{source}{command}"));
        result.unwrap();
        engine.take_charts().remove(0)
    };
    // 6 values: four bins by Sturges, width 1.75 moved to 2, from 0
    let chart = histogram(
        "IGRAPH /X1=VAR(v) TYPE=SCALE /Y=$count /HISTOGRAM SHAPE=HISTOGRAM CUM=OFF X1INTERVAL AUTO X1START=0 CURVE=ON.",
    );
    assert_eq!(chart.kind, ChartKind::Histogram);
    close(&chart.series[0].values, &[1.0, 3.0, 0.0, 1.0, 1.0]);
    assert!(chart.normal.is_some());
    let chart = histogram("IGRAPH /X1=VAR(v) /Y=$pct /HISTOGRAM.");
    let sixth = 100.0 / 6.0;
    close(&chart.series[0].values, &[sixth, 50.0, 0.0, sixth, sixth]);
    assert_eq!(chart.value_axis.title, "Percent");
    let chart = histogram("IGRAPH /X1=VAR(v) /Y=$pct /HISTOGRAM CUM.");
    close(
        &chart.series[0].values,
        &[sixth, 4.0 * sixth, 4.0 * sixth, 5.0 * sixth, 100.0],
    );
    assert_eq!(chart.value_axis.title, "Cumulative Percent");
}

#[test]
fn scatter_with_fit_lines() {
    let source = "DATA LIST LIST /x (F2.0) y (F2.0) g (A1).
BEGIN DATA
1 3 p
2 5 p
3 4 q
4 8 q
END DATA.
";
    let scatter = |command: &str| {
        let (mut engine, result) = engine(&format!("{source}{command}"));
        result.unwrap();
        engine.take_charts().remove(0)
    };
    let lines = |chart: &Chart| match &chart.plot {
        Some(Plot::Points { lines, .. }) => lines.clone(),
        _ => panic!(),
    };
    let line = |intercept: f64, slope: f64| ReferenceLine { intercept, slope };
    // x mean 2.5, y mean 5; Sxx = 5, Sxy = 7: slope 1.4, intercept 1.5
    let chart = scatter(
        "IGRAPH /X1=VAR(x) TYPE=SCALE /Y=VAR(y) TYPE=SCALE /COORDINATE=VERTICAL /SCATTER COINCIDENT=NONE \
         /FITLINE METHOD=REGRESSION LINEAR LINE=TOTAL SPIKE=OFF.",
    );
    assert_eq!(chart.kind, ChartKind::Scatter);
    let got = lines(&chart);
    assert_eq!(got.len(), 1);
    assert!((got[0].slope - 1.4).abs() < 1e-12 && (got[0].intercept - 1.5).abs() < 1e-12);
    assert_eq!(chart.footnote.as_deref(), Some("Fit line: y = 1.5 + 1.4 x"));
    // a line per group: p through (1, 3) and (2, 5), q through (3, 4) and (4, 8)
    let chart = scatter(
        "IGRAPH /X1=VAR(x) /Y=VAR(y) /COLOR=VAR(g) /SCATTER /FITLINE METHOD=REGRESSION LINEAR LINE=MEFFECT.",
    );
    assert_eq!(lines(&chart), [line(1.0, 2.0), line(-8.0, 4.0)]);
    // through the origin: 57 / 30; the mean of y: 5
    let chart = scatter("IGRAPH /X1=VAR(x) /Y=VAR(y) /SCATTER /FITLINE METHOD=ORIGIN LINEAR.");
    assert_eq!(lines(&chart), [line(0.0, 1.9)]);
    let chart = scatter("IGRAPH /X1=VAR(x) /Y=VAR(y) /SCATTER /FITLINE METHOD=MEAN.");
    assert_eq!(lines(&chart), [line(5.0, 0.0)]);
}

#[test]
fn panels_give_a_chart_per_hall() {
    let charts = run("IGRAPH /X1=VAR(shift) /Y=$count /PANEL VAR(hall) /BAR.");
    assert_eq!(charts.len(), 2);
    // hall 1: a a a b b a; hall 2: b b a b
    assert!(
        charts[0].title.ends_with("(hall = 1)"),
        "{}",
        charts[0].title
    );
    close(&charts[0].series[0].values, &[4.0, 2.0]);
    close(&charts[1].series[0].values, &[1.0, 3.0]);
}

#[test]
fn category_orders() {
    let order = |spec: &str| {
        let chart = one(&format!(
            "IGRAPH /X1=VAR(team) /Y=$count /COLOR=VAR(shift) /CATORDER VAR(team) ({spec}) /CATORDER VAR(shift) (DESCENDING) /BAR."
        ));
        let series: Vec<String> = chart.series.iter().map(|s| s.name.clone()).collect();
        (
            labels(&chart).join(" "),
            series.join(" "),
            chart.series[0].values.clone(),
        )
    };
    // counts: North 4, South 5; series b first
    assert_eq!(
        order("DESCENDING VALUES"),
        ("South North".into(), "b a".into(), vec![3.0, 2.0])
    );
    assert_eq!(
        order("ASCENDING COUNT"),
        ("North South".into(), "b a".into(), vec![2.0, 3.0])
    );
    assert_eq!(order("DESCENDING COUNT").0, "South North");
    assert_eq!(order("DESCENDING LABEL").0, "South North");
    assert_eq!(order("OCCURRENCE").0, "North South");
}

#[test]
fn cases_drawn_one_by_one() {
    let chart = one("IGRAPH /X1=$case /Y=VAR(output) /CASELABEL=VAR(shift) /BAR.");
    assert_eq!(chart.categories.len(), 10);
    assert_eq!(chart.categories[2].label, "b");
    assert_eq!(chart.series[0].values[2], 20.0);
}

#[test]
fn what_cannot_be_drawn_is_an_error() {
    let parse_error = |source: &str| {
        let (_, errors) = parse(source);
        assert_eq!(errors.len(), 1, "{source}");
        errors[0].message.clone()
    };
    assert!(parse_error("IGRAPH /X1=VAR(a) /Y=VAR(b) /BAR /REFLINE b 3.").contains("REFLINE"));
    assert!(parse_error("IGRAPH /X1=VAR(a) /Y=VAR(b) /SPIKE CENTROID.").contains("SPIKE"));
    assert!(
        parse_error("IGRAPH /X1=VAR(a) /Y=VAR(b) /SCATTER /FITLINE METHOD=LLR.").contains("LLR")
    );
    assert!(error("IGRAPH /X1=VAR(team) /Y=VAR(output) /BAR /PIE.").contains("mix of elements"));
    assert!(error("IGRAPH /X1=VAR(team) /Y=VAR(output) /X2=VAR(hall) /BAR.").contains("X2"));
    assert!(error("IGRAPH /X1=VAR(team) /Y=$count /BAR (MEAN).").contains("no summary function"));
    assert!(error("IGRAPH /X1=VAR(output) /Y=VAR(hours) /HISTOGRAM.").contains("$COUNT"));
    // no element: a warning and no chart
    let (mut engine, result) = engine(&format!("{DATA}IGRAPH /X1=VAR(team) /Y=VAR(output)."));
    result.unwrap();
    assert!(engine.take_charts().is_empty());
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("nothing is drawn"))
    );
}
