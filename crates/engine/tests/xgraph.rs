//! XGRAPH: the numbers of its charts on small made-up shop data, worked out by hand.

use statscraft_engine::chart::{ChartKind, Grouping, Orientation};
use statscraft_engine::plot::Plot;
use statscraft_engine::{Chart, Engine};
use statscraft_syntax::parse;

/// zone (9 = unknown, user-missing), kind of sale, spend (one missing), age of the buyer, site.
const DATA: &str = "DATA LIST LIST /zone (F1) kind (A1) spend (F4.0) age (F3.0) site (F1).
BEGIN DATA
1 a 10 21 1
1 a 20 25 1
1 b 30 33 2
1 b . 29 1
2 a 40 38 2
2 b 50 41 1
2 b 60 47 2
2 b 70 52 1
2 a 80 58 2
9 a 99 60 1
END DATA.
VALUE LABELS zone 1 'East' 2 'West' 9 'Unknown'.
MISSING VALUES zone (9).
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
        assert!((g - w).abs() < 1e-9, "{got:?} against {want:?}");
    }
}

#[test]
fn simple_bar_of_counts() {
    let chart = one("XGRAPH CHART=([COUNT] [BAR]) BY zone [c].");
    assert_eq!(chart.kind, ChartKind::Bar);
    // the unknown zone is user-missing and left out
    assert_eq!(labels(&chart), ["East", "West"]);
    close(&chart.series[0].values, &[4.0, 5.0]);
    let chart = one("XGRAPH CHART=([COUNT] [BAR]) BY zone [c] /MISSING REPORT=YES.");
    assert_eq!(labels(&chart), ["East", "West", "Unknown"]);
    close(&chart.series[0].values, &[4.0, 5.0, 1.0]);
}

#[test]
fn clustered_bar_of_means() {
    // East: a 10 20, b 30 (one missing); West: a 40 80, b 50 60 70
    for source in [
        "XGRAPH (spend [MEAN] [BAR]) BY zone[c] > kind[COLOR].",
        "XGRAPH (spend [MEAN] [BAR]) BY zone[c] BY kind[c].",
    ] {
        let chart = one(source);
        assert_eq!(chart.grouping, Some(Grouping::Clustered));
        assert_eq!(
            (labels(&chart), names(&chart)),
            (vec!["East", "West"], vec!["a", "b"])
        );
        close(&chart.series[0].values, &[15.0, 60.0]);
        close(&chart.series[1].values, &[30.0, 60.0]);
    }
    // a cluster and z: z gives a chart per category
    let charts = run("XGRAPH (spend [MEAN] [BAR]) BY zone > kind BY site.");
    assert_eq!(charts.len(), 2);
    assert!(charts[0].title.contains("site = 1"), "{}", charts[0].title);
    // site 1: East a 10 20, b missing; West b 50 70
    let site_one = &charts[0].series;
    assert_eq!((site_one[0].values[0], site_one[1].values[1]), (15.0, 60.0));
    assert!(site_one[0].values[1].is_nan() && site_one[1].values[0].is_nan());
}

#[test]
fn stacked_bar_of_percentages() {
    // nine cases with a known zone: East a 2 b 2, West a 2 b 3
    let chart = one("XGRAPH ([PCT] [BAR]) > kind [PATTERN] BY zone [c].");
    assert_eq!(chart.grouping, Some(Grouping::Stacked));
    assert_eq!(names(&chart), ["a", "b"]);
    close(&chart.series[0].values, &[200.0 / 9.0, 200.0 / 9.0]);
    close(&chart.series[1].values, &[200.0 / 9.0, 300.0 / 9.0]);
}

#[test]
fn separate_variables_with_the_placeholder() {
    // East ages 21 25 33 (29 has no spend), West 38 41 47 52 58
    let chart = one("XGRAPH ((spend + age) [MEAN] [BAR]) BY zone BY 1.");
    assert_eq!(names(&chart), ["Mean of spend", "Mean of age"]);
    close(&chart.series[1].values, &[79.0 / 3.0, 47.2]);
    let chart = one("XGRAPH ((spend + age) [MEAN] [BAR]) BY zone BY 1 /MISSING USE=VARIABLEWISE.");
    close(&chart.series[1].values, &[27.0, 47.2]);
    // the variables along the x axis, the zones in series
    let chart = one("XGRAPH (spend [MEAN] [BAR] + age [MEAN] [BAR]) BY 1 BY zone.");
    assert_eq!(
        (labels(&chart), names(&chart)),
        (vec!["Mean of spend", "Mean of age"], vec!["East", "West"])
    );
    close(&chart.series[0].values, &[20.0, 79.0 / 3.0]);
    close(&chart.series[1].values, &[60.0, 47.2]);
    assert_eq!(chart.legend.as_deref(), Some("zone"));
    // stacked variables
    let chart = one("XGRAPH ((spend + age) [SUM] [BAR]) > 1 BY zone.");
    assert_eq!(chart.grouping, Some(Grouping::Stacked));
    close(&chart.series[0].values, &[60.0, 300.0]);
    assert!(error("XGRAPH ((spend + age) [MEAN] [BAR]) BY zone.").contains("placeholder 1"));
}

#[test]
fn values_of_cases() {
    let chart = one("XGRAPH (spend [VALUE]) BY $CASENUM [LABEL=kind].");
    assert_eq!(labels(&chart).len(), 10);
    assert_eq!(labels(&chart)[..3], ["a", "a", "b"]);
    assert!(chart.series[0].values[3].is_nan());
    // a variable on the other axis gives a chart per category
    let charts = run("XGRAPH (spend [VALUE] [BAR]) BY zone BY $CASENUM.");
    assert_eq!(charts.len(), 2);
    close(&charts[1].series[0].values, &[40.0, 50.0, 60.0, 70.0, 80.0]);
}

#[test]
fn dot_plot() {
    // site 1 six cases, site 2 four
    let points = |source: &str| {
        let chart = one(source);
        let Some(Plot::Points { groups, .. }) = chart.plot else {
            panic!("{chart:?}")
        };
        groups[0]
            .points
            .iter()
            .map(|p| (p.x, p.y))
            .collect::<Vec<_>>()
    };
    // its own kind, so every export captions it as a dot plot, not a scatter plot
    let chart = one("XGRAPH ([POINT]) BY site.");
    assert_eq!(chart.kind, ChartKind::Dot);
    assert_eq!(chart.kind_text(), "Dot plot");
    assert!(
        chart
            .data_table()
            .caption
            .is_some_and(|c| c.starts_with("Dot plot."))
    );
    assert_eq!(chart.vega_lite()["description"], "Dot plot");
    let up = points("XGRAPH ([POINT]) BY site.");
    assert_eq!(up.len(), 10);
    assert_eq!(up[5], (1.0, 6.0));
    assert_eq!(up[9], (2.0, 4.0));
    let centered = points("XGRAPH ([POINT]) BY site /DISPLAY DOT=SYMMETRIC.");
    assert_eq!((centered[0], centered[5]), ((1.0, -2.5), (1.0, 2.5)));
    assert_eq!(centered[6], (2.0, -1.5));
    assert!(
        points("XGRAPH ([POINT]) BY site /DISPLAY DOT=FLAT.")
            .iter()
            .all(|p| p.1 == 0.0)
    );
    assert!(error("XGRAPH ([POINT]) BY kind.").contains("string variable"));
}

#[test]
fn line_chart() {
    let chart = one("XGRAPH (spend [SUM] [LINE]) BY zone.");
    assert_eq!(chart.kind, ChartKind::Line);
    close(&chart.series[0].values, &[60.0, 300.0]);
    assert!(error("XGRAPH (spend [SUM] [LINE]) > kind BY zone.").contains("do not stack"));
}

#[test]
fn box_plot_by_category() {
    let chart = one("XGRAPH (spend [BOX]) BY kind.");
    assert_eq!(chart.kind, ChartKind::Boxplot);
    let Some(Plot::Boxes { boxes }) = chart.plot else {
        panic!()
    };
    // a: 10 20 40 80 99, hinges at depth 2; b: 30 50 60 70, hinges at depth 1.5
    let five = |i: usize| {
        let b = &boxes[i];
        [
            b.lower_whisker,
            b.lower_hinge,
            b.median,
            b.upper_hinge,
            b.upper_whisker,
        ]
    };
    assert_eq!(five(0), [10.0, 20.0, 40.0, 80.0, 99.0]);
    assert_eq!(five(1), [30.0, 40.0, 55.0, 65.0, 70.0]);
    assert!(error("XGRAPH (spend [MEAN] [BOX]) BY kind.").contains("without a function"));
}

#[test]
fn error_bars_by_hand() {
    // East 10 20 30: sd 10; West 40 to 80 by 10: sd sqrt(250)
    let chart = one("XGRAPH (spend [MEAN] [BAR]) BY zone /ERRORBAR SE(1).");
    let bars = chart.error_bars.unwrap();
    close(
        &bars.margins[0],
        &[10.0 / 3f64.sqrt(), 250f64.sqrt() / 5f64.sqrt()],
    );
    // counts 4 and 5 of 9: the binomial spread sqrt(9 p (1 - p)) = sqrt(20 / 9) for both
    let chart = one("XGRAPH ([COUNT] [BAR]) BY zone /ERRORBAR SE(2).");
    let spread = 2.0 * (20f64 / 9.0).sqrt();
    close(&chart.error_bars.unwrap().margins[0], &[spread, spread]);
    let chart = one("XGRAPH ([PCT] [BAR]) BY zone /ERRORBAR STDDEV(1).");
    let spread = 100.0 * (20f64 / 81.0 / 9.0).sqrt();
    close(&chart.error_bars.unwrap().margins[0], &[spread, spread]);
    // Jeffreys: the ends are the 2.5 and 97.5% points of Beta(w + .5, 9 - w + .5), times 9
    let chart = one("XGRAPH ([COUNT] [BAR]) BY zone /ERRORBAR CI(95).");
    for (i, w) in [4.0, 5.0].into_iter().enumerate() {
        let (low, high) = (
            chart.error_bar(0, i).unwrap()[0],
            chart.error_bar(0, i).unwrap()[1],
        );
        let beta = |x: f64| statscraft_dist::beta_i(w + 0.5, 9.0 - w + 0.5, x / 9.0);
        assert!((beta(low) - 0.025).abs() < 1e-6 && (beta(high) - 0.975).abs() < 1e-6);
    }
    assert!(error("XGRAPH ([POINT]) BY site /ERRORBAR CI(95).").contains("ERRORBAR"));
}

#[test]
fn panels() {
    // site 1: a 3 b 3; site 2: a 2 b 2
    let charts = run("XGRAPH ([COUNT] [BAR]) BY kind /PANEL COLVAR=site.");
    assert_eq!(charts.len(), 2);
    assert!(charts[1].title.contains("site = 2"));
    close(&charts[0].series[0].values, &[3.0, 3.0]);
    close(&charts[1].series[0].values, &[2.0, 2.0]);
    assert!(
        error("XGRAPH ([COUNT] [BAR]) BY kind BY zone /COORDINATE SPLIT=YES /PANEL ROWVAR=site.")
            .contains("ROWVAR")
    );
}

#[test]
fn population_pyramids() {
    let chart = one("XGRAPH ([COUNT] [BAR]) BY kind BY zone /COORDINATE SPLIT=YES.");
    assert_eq!(chart.orientation, Some(Orientation::Horizontal));
    assert_eq!(names(&chart), ["East", "West"]);
    close(&chart.series[0].values, &[2.0, 2.0]);
    close(&chart.series[1].values, &[2.0, 3.0]);
    // a histogram per kind with the same bins of 10 from 20
    let charts = run(
        "XGRAPH ([HISTOBAR]) BY age[s] BY kind[c] /COORDINATE SPLIT=YES /BIN START=20 \
         SIZE=WIDTH(10) /DISTRIBUTION TYPE=NORMAL.",
    );
    assert_eq!(charts.len(), 2);
    assert_eq!(charts[0].kind, ChartKind::Histogram);
    assert_eq!(labels(&charts[0])[0], "20 to under 30");
    close(&charts[0].series[0].values, &[2.0, 1.0, 0.0, 1.0, 1.0]);
    close(&charts[1].series[0].values, &[1.0, 1.0, 2.0, 1.0]);
    assert!((charts[0].normal.as_ref().unwrap().mean - 40.4).abs() < 1e-9);
    // four bins from the smallest age, 21, to the largest, 60, which the last bin holds
    let chart = one("XGRAPH ([HISTOBAR]) BY age /BIN SIZE=COUNT(4).");
    close(&chart.series[0].values, &[3.0, 2.0, 2.0, 3.0]);
    assert!(error("XGRAPH ([HISTOBAR]) BY age[c].").contains("scale"));
}

#[test]
fn titles_and_warnings() {
    let (mut engine, result) = engine(
        "XGRAPH ([COUNT] [BAR]) BY zone /TEMPLATE FILE='look.sgt' /DISPLAY DOT=FLAT \
         /TITLES TITLE='Sales: )CHART' SUBTITLE='Two zones' FOOTNOTE='Made up'.",
    );
    result.unwrap();
    let warnings: Vec<&String> = engine
        .warnings()
        .iter()
        .filter(|w| w.starts_with("XGRAPH"))
        .collect();
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    let chart = engine.take_charts().remove(0);
    assert_eq!(chart.title, "Sales: zone");
    assert_eq!(chart.subtitle.as_deref(), Some("Two zones"));
    assert_eq!(chart.footnote.as_deref(), Some("Made up"));
}
