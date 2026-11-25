//! GGRAPH: the numbers of its charts on a small made-up orchard harvest, with a weight,
//! missing values in both roles and the MISSING and REPORTMISSING options.

use statscraft_engine::chart::{ChartKind, Grouping, Orientation, Statistic};
use statscraft_engine::plot::Plot;
use statscraft_engine::{Chart, Engine};
use statscraft_syntax::parse;

/// field (9 = unknown, user-missing; one case system-missing), crop (a apple, p pear), the
/// yield in crates (one missing), the rain in centimeters and a weight (2 for the third case).
const DATA: &str = "DATA LIST LIST /field (F1) crop (A1) yield (F5.1) rain (F4.1) wt (F2).
BEGIN DATA
1 a 10 2 1
1 p 14 3 1
1 a 12 4 2
2 a 20 5 1
2 p 8 1 1
3 p 6 2 1
3 a . 3 1
9 a 30 6 1
. p 16 7 1
END DATA.
VALUE LABELS field 1 'North' 2 'South' 3 'East' 9 'Unknown' /crop 'a' 'Apple' 'p' 'Pear'.
MISSING VALUES field (9).
VARIABLE LEVEL field crop (NOMINAL) yield rain (SCALE).
WEIGHT BY wt.
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

/// The chart of the last command, with its warnings.
fn chart(source: &str) -> (Chart, Vec<String>) {
    let (mut engine, result) = engine(source);
    result.unwrap_or_else(|e| panic!("{e}"));
    let warnings = engine.warnings().to_vec();
    let mut charts = engine.take_charts();
    assert_eq!(charts.len(), 1, "{charts:?}");
    (charts.remove(0), warnings)
}

fn one(source: &str) -> Chart {
    chart(source).0
}

fn error(source: &str) -> String {
    engine(source).1.expect_err("an error")
}

/// GGRAPH with the dataset "h" of `variables`, then a program reading it with `rest`.
fn program(variables: &str, rest: &str) -> String {
    format!(
        "GGRAPH /GRAPHDATASET NAME=\"h\" VARIABLES={variables}\n /GRAPHSPEC SOURCE=INLINE.\n\
         BEGIN GPL\nSOURCE: s=userSource(id(\"h\"))\n{rest}\nEND GPL.\n"
    )
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

const FIELD: &str = "DATA: field=col(source(s), name(\"field\"), unit.category())";
const CROP: &str = "DATA: crop=col(source(s), name(\"crop\"), unit.category())";

#[test]
fn simple_bar_of_counts_weighs_cases_and_leaves_out_missing_fields() {
    let source = program(
        "field COUNT()",
        &format!(
            "{FIELD}\nDATA: count=col(source(s), name(\"COUNT\"))\n\
             GUIDE: axis(dim(1), label(\"Field\"))\nGUIDE: axis(dim(2), label(\"Crates\"))\n\
             GUIDE: text.title(label(\"Harvest\"))\n\
             SCALE: linear(dim(2), include(0))\n\
             ELEMENT: interval(position(field*count), shape.interior(shape.square))"
        ),
    );
    let (chart, warnings) = chart(&source);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        (chart.kind, chart.grouping),
        (ChartKind::Bar, Some(Grouping::Clustered))
    );
    assert_eq!(chart.title, "Harvest");
    assert_eq!(
        (
            chart.category_axis.title.as_str(),
            chart.value_axis.title.as_str()
        ),
        ("Field", "Crates")
    );
    assert_eq!(labels(&chart), ["North", "South", "East"]);
    assert_eq!(chart.series.len(), 1);
    assert_eq!(chart.series[0].statistic, Statistic::Count);
    // North weighs 1 + 1 + 2
    close(&chart.series[0].values, &[4.0, 2.0, 2.0]);
    // REPORTMISSING makes the user-missing field a category; system-missing stays out
    let reported = one(&source.replace("COUNT()", "COUNT() REPORTMISSING=YES"));
    assert_eq!(labels(&reported), ["North", "South", "East", "Unknown"]);
    assert!(reported.categories[3].missing);
    close(&reported.series[0].values, &[4.0, 2.0, 2.0, 1.0]);
    // at the scale level a variable reports no missing values
    let scale = source.replace(
        "field COUNT()",
        "field[LEVEL=SCALE] COUNT() REPORTMISSING=YES",
    );
    assert_eq!(labels(&one(&scale)), ["North", "South", "East"]);
}

#[test]
fn clustered_bar_of_means_with_color() {
    let source = program(
        "field crop MEAN(yield)",
        &format!(
            "{FIELD}\n{CROP}\nDATA: mean=col(source(s), name(\"MEAN_yield\"))\n\
             COORD: rect(dim(1,2), cluster(3,0))\n\
             GUIDE: legend(aesthetic(aesthetic.color.interior), label(\"Crop\"))\n\
             ELEMENT: interval(position(crop*mean*field), color.interior(crop))"
        ),
    );
    let chart = one(&source);
    assert_eq!(chart.kind_text(), "Clustered bar chart");
    assert_eq!(labels(&chart), ["North", "South", "East"]);
    assert_eq!(names(&chart), ["Apple", "Pear"]);
    assert_eq!(chart.legend.as_deref(), Some("Crop"));
    assert_eq!(chart.value_axis.title, "Mean of yield");
    assert_eq!(chart.series[0].statistic, Statistic::Mean);
    // North apples: 10 and 12 twice; East apples have no valid yield
    close(&chart.series[0].values, &[34.0 / 3.0, 20.0, f64::NAN]);
    close(&chart.series[1].values, &[14.0, 8.0, 6.0]);
    // COORD transpose lays the bars across
    let across = one(&source.replace("cluster(3,0)", "cluster(3,0), transpose()"));
    assert_eq!(across.orientation, Some(Orientation::Horizontal));
}

#[test]
fn missing_follows_listwise_or_variablewise() {
    let rest = format!(
        "{FIELD}\nDATA: count=col(source(s), name(\"COUNT\"))\n\
         ELEMENT: interval(position(field*count))"
    );
    // listwise: the East case without a yield leaves COUNT too
    let listwise = one(&program("field COUNT() MEAN(yield)", &rest));
    close(&listwise.series[0].values, &[4.0, 2.0, 1.0]);
    let variablewise = one(&program(
        "field COUNT() MEAN(yield) MISSING=VARIABLEWISE",
        &rest,
    ));
    close(&variablewise.series[0].values, &[4.0, 2.0, 2.0]);
}

#[test]
fn stacked_bar_of_counts() {
    let chart = one(&program(
        "field crop COUNT()",
        &format!(
            "{FIELD}\n{CROP}\nDATA: count=col(source(s), name(\"COUNT\"))\n\
             ELEMENT: interval.stack(position(field*count), color(crop))"
        ),
    ));
    assert_eq!(chart.kind_text(), "Stacked bar chart");
    assert_eq!(names(&chart), ["Apple", "Pear"]);
    assert_eq!(chart.legend.as_deref(), Some("crop"));
    close(&chart.series[0].values, &[3.0, 1.0, 1.0]);
    close(&chart.series[1].values, &[1.0, 1.0, 1.0]);
}

#[test]
fn line_of_means_and_lines_named_by_color() {
    let chart = one(&program(
        "field MEAN(rain) MEDIAN(rain)[NAME=\"mid\"]",
        &format!(
            "{FIELD}\nDATA: mean=col(source(s), name(\"MEAN_rain\"))\n\
             DATA: mid=col(source(s), name(\"mid\"))\n\
             ELEMENT: line(position(field*mean), color(\"Mean\"), missing.wings())\n\
             ELEMENT: line(position(field*mid), color(\"Median\"))"
        ),
    ));
    assert_eq!(chart.kind, ChartKind::Line);
    assert_eq!(names(&chart), ["Mean", "Median"]);
    // North: 2, 3 and 4 twice
    close(&chart.series[0].values, &[13.0 / 4.0, 3.0, 2.5]);
    close(&chart.series[1].values, &[3.5, 3.0, 2.5]);
    assert_eq!(chart.title, "Mean of rain by field");
}

// GPTILE: the percentile of values taken as the midpoints of their intervals, interpolated on the
// cumulative weights; North holds 2, 3 and 4 (twice), the cumulative weights at the midpoints
// are 0.5, 1.5 and 3 of 4, so half the total (2) is a third of the way from 3 to 4.
#[test]
fn grouped_percentile_interpolates_the_midpoints() {
    let chart = one(&program(
        "field GPTILE(rain, 50)[NAME=\"gp\"]",
        &format!(
            "{FIELD}\nDATA: gp=col(source(s), name(\"gp\"))\n\
             ELEMENT: line(position(field*gp), missing.wings())"
        ),
    ));
    close(&chart.series[0].values, &[10.0 / 3.0, 3.0, 2.5]);
}

#[test]
fn scatter_plot_of_cases_grouped_by_color() {
    let chart = one(&program(
        "rain yield crop",
        &format!(
            "{CROP}\nDATA: rain=col(source(s), name(\"rain\"))\n\
             DATA: yield=col(source(s), name(\"yield\"))\n\
             ELEMENT: point(position(rain*yield), color(crop))"
        ),
    ));
    assert_eq!(chart.kind, ChartKind::Scatter);
    let Some(Plot::Points { groups, .. }) = &chart.plot else {
        panic!("{:?}", chart.plot);
    };
    let points =
        |g: usize| -> Vec<(f64, f64)> { groups[g].points.iter().map(|p| (p.x, p.y)).collect() };
    assert_eq!(groups.len(), 2);
    assert_eq!(
        (groups[0].name.as_str(), groups[1].name.as_str()),
        ("Apple", "Pear")
    );
    // every case is a row, missing fields included; the apple without a yield is left out
    assert_eq!(
        points(0),
        [(2.0, 10.0), (4.0, 12.0), (5.0, 20.0), (6.0, 30.0)]
    );
    assert_eq!(
        points(1),
        [(3.0, 14.0), (1.0, 8.0), (2.0, 6.0), (7.0, 16.0)]
    );
    assert_eq!(chart.legend.as_deref(), Some("crop"));
}

#[test]
fn histogram_from_a_summary_of_bins_with_its_normal_curve() {
    let (chart, warnings) = chart(&program(
        "yield",
        "DATA: yield=col(source(s), name(\"yield\"))\n\
         GUIDE: axis(dim(2), label(\"Frequency\"))\n\
         ELEMENT: interval(position(summary.count(bin.rect(yield))))\n\
         ELEMENT: line(position(density.normal(yield)))",
    ));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(chart.kind, ChartKind::Histogram);
    // 9 weighted values from 6 to 30: five bins of 4.8, rounded to 5, from 5
    assert_eq!(
        labels(&chart),
        [
            "5 to under 10",
            "10 to under 15",
            "15 to under 20",
            "20 to under 25",
            "25 to under 30",
            "30 to under 35"
        ]
    );
    close(&chart.series[0].values, &[2.0, 4.0, 1.0, 1.0, 0.0, 1.0]);
    assert_eq!(chart.value_axis.title, "Frequency");
    let normal = chart.normal.expect("normal curve");
    close(
        &[normal.mean, normal.total],
        &[
            (6.0 + 8.0 + 10.0 + 24.0 + 14.0 + 16.0 + 20.0 + 30.0) / 9.0,
            9.0,
        ],
    );
}

#[test]
fn summaries_in_the_program_and_a_pie() {
    // unaggregated data summed up by the program: the mean rain by crop
    let means = one(&program(
        "crop rain",
        &format!(
            "{CROP}\nDATA: rain=col(source(s), name(\"rain\"))\n\
             ELEMENT: interval(position(summary.mean(crop*rain)))"
        ),
    ));
    // apples 2, 4 twice, 5, 3, 6; pears 3, 1, 2, 7
    close(&means.series[0].values, &[24.0 / 6.0, 13.0 / 4.0]);
    let pie = one(&program(
        "field COUNT()",
        &format!(
            "{FIELD}\nDATA: count=col(source(s), name(\"COUNT\"))\nCOORD: polar.theta(startAngle(0))\n\
             ELEMENT: interval.stack(position(summary.percent(count)), color(field))"
        ),
    ));
    assert_eq!(pie.kind, ChartKind::Pie);
    assert_eq!(labels(&pie), ["North", "South", "East"]);
    close(&pie.series[0].values, &[50.0, 25.0, 25.0]);
}

#[test]
fn separate_variables_become_rows() {
    let chart = one(&program(
        "MEAN(yield) MEAN(rain) TRANSFORM=VARSTOCASES(SUMMARY=\"m\" INDEX=\"v\")",
        "DATA: v=col(source(s), name(\"v\"), unit.category())\n\
         DATA: m=col(source(s), name(\"m\"))\nELEMENT: interval(position(v*m))",
    ));
    assert_eq!(labels(&chart), ["Mean of yield", "Mean of rain"]);
    // listwise: the case without a yield leaves both means
    close(&chart.series[0].values, &[128.0 / 9.0, 34.0 / 9.0]);
}

#[test]
fn unsupported_statements_warn_and_the_chart_is_drawn() {
    let (chart, warnings) = chart(&program(
        "field COUNT()",
        &format!(
            "{FIELD}\nDATA: count=col(source(s), name(\"COUNT\"))\n\
             PAGE: begin(scale(400px,300px))\n\
             GUIDE: form.line(position(*,2))\n\
             ELEMENT: interval(position(field*count), label(count))\n\
             ELEMENT: edge(position(field*count))"
        ),
    ));
    close(&chart.series[0].values, &[4.0, 2.0, 2.0]);
    assert_eq!(
        warnings,
        [
            "GGRAPH: line 23: PAGE is not supported yet and is skipped",
            "GGRAPH: line 24: GUIDE could not be read and is skipped",
            "GGRAPH: line 25: ELEMENT interval: label is not supported yet and is skipped",
            "GGRAPH: line 26: ELEMENT edge is not supported yet and is skipped",
        ]
    );
}

#[test]
fn charts_that_cannot_be_drawn_are_errors() {
    let no_element = program(
        "field COUNT()",
        &format!("{FIELD}\nELEMENT: edge(position(field))"),
    );
    assert_eq!(
        error(&no_element),
        "GGRAPH: the chart program has no ELEMENT that can be drawn; see the warnings"
    );
    let unknown = program("field COUNT()", "DATA: x=col(source(s), name(\"nope\"))");
    assert_eq!(
        error(&unknown),
        "GGRAPH: line 21: graph dataset \"h\" has no column \"nope\""
    );
    assert_eq!(
        error("BEGIN GPL\nELEMENT: point(position(a*b))\nEND GPL."),
        "BEGIN GPL belongs right after a GGRAPH command with GRAPHSPEC SOURCE=INLINE"
    );
}

/// The values of the series of a dataset's column drawn as points by field.
fn column(variables: &str, name: &str) -> Vec<f64> {
    one(&program(
        variables,
        &format!(
            "{FIELD}\nDATA: v=col(source(s), name(\"{name}\"))\n\
             ELEMENT: point(position(field*v))"
        ),
    ))
    .series[0]
        .values
        .clone()
}

#[test]
fn mean_interval_has_a_value_and_two_ends() {
    // North counts 10, 14 and 12 twice (n = 4), South 20 and 8, East only 6; t quantiles of 3 and
    // 1 degrees of freedom, checked in R
    let vars = "field MEANCI(yield, 95)[NAME=\"m\" HIGH=\"hi\" LOW=\"lo\"]";
    close(&column(vars, "m"), &[12.0, 14.0, 6.0]);
    // East has no interval, so it has no point at its ends
    close(&column(vars, "hi"), &[14.598456527250, 90.237228417048]);
    close(&column(vars, "lo"), &[9.401543472750, -62.237228417048]);
    // without qualifiers the names are the function's, with _HIGH and _LOW after them
    let hi = column("field MEANCI(yield, 90)", "MEANCI_yield_90_HIGH");
    close(&hi[..1], &[13.921513198196]);
    // k standard deviations and k standard errors: sd 1.632993161855, se 0.816496580928
    let sd = 1.632993161855;
    let hi = column("field MEANSD(yield, 2)", "MEANSD_yield_2_HIGH");
    close(&hi[..1], &[12.0 + 2.0 * sd]);
    let lo = column("field MEANSE(yield, 3)", "MEANSE_yield_3_LOW");
    close(&lo[..1], &[12.0 - 3.0 * sd / 2.0]);
}

#[test]
fn interval_element_draws_the_error_bars_of_the_points() {
    let chart = one(&program(
        "field MEANCI(yield, 95)[NAME=\"m\" HIGH=\"hi\" LOW=\"lo\"]",
        &format!(
            "{FIELD}\nDATA: m=col(source(s), name(\"m\"))\n\
             DATA: hi=col(source(s), name(\"hi\"))\nDATA: lo=col(source(s), name(\"lo\"))\n\
             ELEMENT: point(position(field*m))\n\
             ELEMENT: interval(position(region.spread.range(field*(lo+hi))), shape(shape.ibeam))"
        ),
    ));
    assert_eq!(chart.kind_text(), "Simple error bar chart");
    let bars = chart.error_bars.as_ref().expect("error bars");
    assert_eq!(bars.label, "95% CI");
    close(
        &bars.margins[0],
        &[2.598456527250, 76.237228417048, f64::NAN],
    );
    close(&bars.lower[0], &[2.598456527250, 76.237228417048, f64::NAN]);
    // the order of the two ends does not matter, and a bar can sit on bars
    let chart = one(&program(
        "field MEANCI(yield, 95)[NAME=\"m\" HIGH=\"hi\" LOW=\"lo\"]",
        &format!(
            "{FIELD}\nDATA: m=col(source(s), name(\"m\"))\n\
             DATA: hi=col(source(s), name(\"hi\"))\nDATA: lo=col(source(s), name(\"lo\"))\n\
             ELEMENT: interval(position(field*m))\n\
             ELEMENT: interval(position(region.spread.range(field*(hi+lo))))"
        ),
    ));
    assert_eq!(chart.kind_text(), "Simple bar chart");
    close(
        &chart.error_bars.as_ref().expect("error bars").margins[0],
        &[2.598456527250, 76.237228417048, f64::NAN],
    );
    // an interval element alone has no value to surround
    let message = error(&program(
        "field MEANCI(yield, 95)[NAME=\"m\" HIGH=\"hi\" LOW=\"lo\"]",
        &format!(
            "{FIELD}\nDATA: m=col(source(s), name(\"m\"))\n\
             DATA: hi=col(source(s), name(\"hi\"))\nDATA: lo=col(source(s), name(\"lo\"))\n\
             ELEMENT: interval(position(region.spread.range(field*(lo+hi))))"
        ),
    ));
    assert!(
        message.contains("add a point, bar or line ELEMENT"),
        "{message}"
    );
}

#[test]
fn separate_functions_with_intervals_become_rows_with_low_and_high_columns() {
    // 9 valid cases of yield (weights counted): mean 14.2222, t(8) interval checked in R; the
    // mean of rain has no interval, so its ends are missing
    let vars = "MEANCI(yield, 95) MEAN(rain) \
                TRANSFORM=VARSTOCASES(SUMMARY=\"m\" INDEX=\"k\" HIGH=\"up\" LOW=\"down\")";
    let rows = |name: &str| {
        one(&program(
            vars,
            &format!(
                "DATA: k=col(source(s), name(\"k\"), unit.category())\n\
                 DATA: v=col(source(s), name(\"{name}\"))\nELEMENT: point(position(k*v))"
            ),
        ))
        .series[0]
            .values
            .clone()
    };
    close(&rows("m"), &[14.222222222222, 3.777777777778]);
    close(&rows("up"), &[19.788803718338]);
    close(&rows("down"), &[8.655640726106]);
}

#[test]
fn median_and_count_intervals_agree_with_the_error_bars_of_graph() {
    // GRAPH's bars give how far each end lies below and above the value
    let graph = one("GRAPH /BAR(SIMPLE)=MEDIAN(yield) BY field /INTERVAL CI(90).");
    let bars = graph.error_bars.as_ref().expect("bars");
    let value = &graph.series[0].values;
    let up: Vec<f64> = (0..3).map(|k| value[k] + bars.margins[0][k]).collect();
    let down: Vec<f64> = (0..3).map(|k| value[k] - bars.lower[0][k]).collect();
    close(
        &column("field MEDIANCI(yield, 90)", "MEDIANCI_yield_90"),
        value,
    );
    close(
        &column("field MEDIANCI(yield, 90)", "MEDIANCI_yield_90_HIGH"),
        &up,
    );
    close(
        &column("field MEDIANCI(yield, 90)", "MEDIANCI_yield_90_LOW"),
        &down,
    );
    // COUNT of the cases of each field, a share of all the cases counted
    let graph = one("GRAPH /BAR(SIMPLE)=COUNT BY field /INTERVAL CI(95).");
    let bars = graph.error_bars.as_ref().expect("bars");
    let value = &graph.series[0].values;
    close(&column("field COUNTCI(95)", "COUNTCI_95"), value);
    let up: Vec<f64> = (0..3).map(|k| value[k] + bars.margins[0][k]).collect();
    let down: Vec<f64> = (0..3).map(|k| value[k] - bars.lower[0][k]).collect();
    close(&column("field COUNTCI(95)", "COUNTCI_95_HIGH"), &up);
    close(&column("field COUNTCI(95)", "COUNTCI_95_LOW"), &down);
}
