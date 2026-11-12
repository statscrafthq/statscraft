//! BEGIN CHART, a Statscraft extension: a chart given as JSON joins the output like a
//! procedure's chart. The chart JSON of every procedure reads back as the same chart, a
//! Vega-Lite subset translates exactly, and everything else is refused with a message that
//! names it, never with a panic.

use serde_json::{Value, json};
use statscraft_engine::chart::{ChartKind, Grouping, Orientation, Tier};
use statscraft_engine::picture::{Paper, chart_svg};
use statscraft_engine::plot::Plot;
use statscraft_engine::session::Session;
use statscraft_engine::{Cell, Chart, office};

/// Made-up orders of a bakery: shop (1 Dock, 2 Mill, 3 Park), loaves, the tip and the hour.
const DATA: &str = "DATA LIST LIST /shop loaves (F2) tip (F4.1) hour (F2).
BEGIN DATA
1 4 1.5 8
2 7 0.0 9
1 2 2.5 8
3 9 3.0 12
2 5 1.0 12
2 1 0.5 9
3 6 2.0 8
1 3 . 12
END DATA.
VALUE LABELS shop 1 'Dock' 2 'Mill' 3 'Park'.
";

fn block(json: &str) -> String {
    format!("BEGIN CHART\n{json}\nEND CHART.")
}

/// The chart a BEGIN CHART block gives, or the message of its error.
fn read(json: &str) -> Result<Chart, String> {
    let mut session = Session::default();
    match session.run(&block(json), None) {
        Ok(mut result) => {
            let command = result.commands.remove(0);
            assert_eq!(command.name, "BEGIN CHART");
            assert_eq!(command.document.as_deref(), Some("Document1"));
            let chart = command.charts.into_iter().next().expect("a chart");
            drawn(&chart);
            Ok(chart)
        }
        Err(errors) => Err(errors[0].message.clone()),
    }
}

fn refused(json: &str, message: &str) {
    match read(json) {
        Ok(chart) => panic!("{json} was read: {chart:?}"),
        Err(e) => assert!(e.contains(message), "{e}\nexpected: {message}"),
    }
}

/// Every way of showing a chart takes it without failing.
fn drawn(chart: &Chart) {
    let _ = chart.data_table();
    let _ = chart.vega_lite();
    let _ = chart_svg(chart, Paper::Light);
    let _ = chart_svg(chart, Paper::Dark);
    let _ = office::chart_workbook(chart);
    let _ = office::clipboard_package(chart);
}

#[test]
fn the_chart_json_of_procedures_reads_back_as_the_same_chart() {
    let mut session = Session::default();
    let source = format!(
        "{DATA}FREQUENCIES shop /BARCHART.\nFREQUENCIES shop /PIECHART PERCENT.\n\
         FREQUENCIES loaves /HISTOGRAM=NORMAL.\nCROSSTABS shop BY hour /BARCHART.\n\
         GRAPH /BAR(STACKED)=COUNT BY shop BY hour.\nGRAPH /LINE(AREA)=MEAN(loaves) MEAN(tip) BY shop.\n\
         GRAPH /ERRORBAR(STERROR 2)=loaves BY shop.\nGRAPH /PARETO=shop.\n\
         GRAPH /BAR(RANGE)=MIN(loaves) MAX(loaves) BY shop.\n\
         GRAPH /LINE(DIFFERENCE)=SUM(loaves) SUM(tip) BY shop.\n\
         GRAPH /SCATTERPLOT=loaves WITH tip BY shop.\nEXAMINE loaves BY shop /PLOT=BOXPLOT."
    );
    let result = session.run(&source, None).unwrap();
    let charts: Vec<Chart> = result.commands.into_iter().flat_map(|c| c.charts).collect();
    assert!(charts.len() >= 12, "{}", charts.len());
    for chart in charts {
        let written = serde_json::to_value(&chart).unwrap();
        let text = serde_json::to_string_pretty(&written).unwrap();
        let back = read(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        // compared as JSON: a value that is not finite (NaN) is null on both sides
        assert_eq!(serde_json::to_value(&back).unwrap(), written, "{text}");
    }
}

#[test]
fn the_chart_goes_to_the_designated_document_and_out_with_output_save_and_export() {
    let dir = std::env::temp_dir().join(format!("statscraft-chart-input-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let chart = json!({
        "mark": "bar",
        "title": "Loaves by shop",
        "data": {"values": [{"shop": "Dock", "loaves": 9}, {"shop": "Mill", "loaves": 13}]},
        "encoding": {
            "x": {"field": "shop", "type": "nominal"},
            "y": {"field": "loaves", "type": "quantitative"}
        }
    });
    let source = format!(
        "CD '{}'.\nOUTPUT NEW NAME=agent.\n{}\nOUTPUT SAVE OUTFILE='agent.scout'.\n\
         OUTPUT EXPORT /XLSX DOCUMENTFILE='agent.xlsx'.",
        dir.display(),
        block(&chart.to_string())
    );
    let mut session = Session::default();
    let result = session.run(&source, None).unwrap();
    assert_eq!(result.commands[2].document.as_deref(), Some("agent"));
    let items = session.output_items("agent").unwrap();
    let leaves = statscraft_engine::output_file::leaves(items);
    assert_eq!(leaves.len(), 1);
    assert_eq!(leaves[0].0, "BEGIN CHART");
    let saved = std::fs::read_to_string(dir.join("agent.scout")).unwrap();
    assert!(saved.contains("\"title\": \"Loaves by shop\""), "{saved}");
    assert!(dir.join("agent.xlsx").exists());
    // opened again, the chart is there
    let mut again = Session::default();
    let opened = format!("CD '{}'.\nOUTPUT OPEN FILE='agent.scout'.", dir.display());
    again.run(&opened, None).unwrap();
    let name = again.designated_output().unwrap().to_string();
    let items = again.output_items(&name).unwrap();
    assert_eq!(statscraft_engine::output_file::leaves(items).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_version_and_the_vega_lite_member_are_read_and_null_fields_are_left_out() {
    let chart = json!({
        "version": 1,
        "kind": "line",
        "tier": "presentation",
        "title": "Visitors",
        "subtitle": null,
        "category_axis": {"title": "Week", "format": ""},
        "value_axis": {"title": "Visitors", "format": "F8.0"},
        "categories": [
            {"label": "1", "value": 1, "missing": false},
            {"label": "2", "value": 2}
        ],
        "series": [{"name": "Visitors", "statistic": "count", "values": [40, null]}],
        "vega_lite": {"mark": "line"}
    });
    let read = read(&chart.to_string()).unwrap();
    assert_eq!(read.kind, ChartKind::Line);
    assert_eq!(read.categories[1].value, Cell::Number(2.0));
    assert_eq!(read.series[0].values[0], 40.0);
    assert!(read.series[0].values[1].is_nan());
}

fn vega(mark: Value, encoding: Value, values: Value) -> String {
    json!({
        "$schema": "https://vega.github.io/schema/vega-lite/v5.json",
        "width": 400,
        "mark": mark,
        "data": {"values": values},
        "encoding": encoding
    })
    .to_string()
}

fn sales() -> Value {
    json!([
        {"region": "South", "year": "2025", "sales": 12.5},
        {"region": "North", "year": "2025", "sales": 7},
        {"region": "North", "year": "2024", "sales": 5},
        {"region": "South", "year": "2024", "sales": null}
    ])
}

fn labels(chart: &Chart) -> Vec<&str> {
    chart.categories.iter().map(|c| c.label.as_str()).collect()
}

fn series(chart: &Chart) -> Vec<(&str, Vec<Option<f64>>)> {
    let values = |v: &[f64]| v.iter().map(|x| (!x.is_nan()).then_some(*x)).collect();
    chart
        .series
        .iter()
        .map(|s| (s.name.as_str(), values(&s.values)))
        .collect()
}

#[test]
fn vega_lite_bars_become_clustered_stacked_or_percent_bars() {
    let encoding = |extra: Value| {
        let mut e = json!({
            "x": {"field": "region", "type": "nominal", "title": "Region"},
            "y": {"field": "sales", "type": "quantitative"},
            "color": {"field": "year"}
        });
        e.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        e
    };
    let stacked = read(&vega(json!("bar"), encoding(json!({})), sales())).unwrap();
    assert_eq!(stacked.kind, ChartKind::Bar);
    assert_eq!(stacked.tier, Tier::Presentation);
    assert_eq!(stacked.orientation, Some(Orientation::Vertical));
    assert_eq!(stacked.grouping, Some(Grouping::Stacked));
    assert_eq!(stacked.title, "sales by Region");
    assert_eq!(stacked.category_axis.title, "Region");
    assert_eq!(stacked.value_axis.title, "sales");
    assert_eq!(stacked.value_axis.format, "F12.1");
    assert_eq!(stacked.legend.as_deref(), Some("year"));
    // ascending, as Vega-Lite sorts by default
    assert_eq!(labels(&stacked), ["North", "South"]);
    assert_eq!(stacked.categories[0].value, Cell::Text("North".into()));
    assert_eq!(
        series(&stacked),
        [
            ("2024", vec![Some(5.0), None]),
            ("2025", vec![Some(7.0), Some(12.5)])
        ]
    );
    let clustered = read(&vega(
        json!({"type": "bar", "tooltip": true}),
        encoding(json!({"xOffset": {"field": "year"}})),
        sales(),
    ))
    .unwrap();
    assert_eq!(clustered.grouping, Some(Grouping::Clustered));
    let y = json!({"field": "sales", "type": "quantitative", "stack": "normalize"});
    let percent = read(&vega(json!("bar"), encoding(json!({ "y": y })), sales())).unwrap();
    assert_eq!(percent.grouping, Some(Grouping::Percent));
}

#[test]
fn vega_lite_horizontal_bars_lines_areas_pies_and_points() {
    let rows = json!([{"team": "b", "goals": 3}, {"team": "a", "goals": 4}]);
    let horizontal = read(&vega(
        json!("bar"),
        json!({
            "y": {"field": "team", "type": "nominal", "sort": null},
            "x": {"field": "goals", "type": "quantitative", "title": "Goals"}
        }),
        rows.clone(),
    ))
    .unwrap();
    assert_eq!(horizontal.orientation, Some(Orientation::Horizontal));
    assert_eq!(horizontal.grouping, Some(Grouping::Clustered));
    // sort null keeps the order of the data
    assert_eq!(labels(&horizontal), ["b", "a"]);
    assert_eq!(series(&horizontal), [("Goals", vec![Some(3.0), Some(4.0)])]);
    assert_eq!(horizontal.value_axis.format, "F8.0");

    let weeks = json!([{"week": 2, "n": 5}, {"week": 1, "n": 4}, {"week": 10, "n": 6}]);
    let line = read(&vega(
        json!("line"),
        json!({"x": {"field": "week", "type": "quantitative"}, "y": {"field": "n", "type": "quantitative"}}),
        weeks.clone(),
    ))
    .unwrap();
    assert_eq!(line.kind, ChartKind::Line);
    assert_eq!(line.grouping, None);
    assert_eq!(labels(&line), ["1", "2", "10"]);
    assert_eq!(line.categories[2].value, Cell::Number(10.0));
    assert_eq!(
        series(&line),
        [("n", vec![Some(4.0), Some(5.0), Some(6.0)])]
    );

    let area = read(&vega(
        json!("area"),
        json!({
            "x": {"field": "year", "type": "ordinal", "sort": ["2025", "2024"]},
            "y": {"field": "sales", "type": "quantitative"},
            "color": {"field": "region", "sort": "descending"}
        }),
        sales(),
    ))
    .unwrap();
    assert_eq!(area.kind, ChartKind::Area);
    assert_eq!(area.grouping, Some(Grouping::Stacked));
    assert_eq!(labels(&area), ["2025", "2024"]);
    assert_eq!(
        series(&area),
        [
            ("South", vec![Some(12.5), None]),
            ("North", vec![Some(7.0), Some(5.0)])
        ]
    );

    let pie = read(
        &json!({
            "mark": {"type": "arc", "innerRadius": 0},
            "title": {"text": "Share", "subtitle": ["of all", "goals"]},
            "data": {"values": rows},
            "encoding": {
                "theta": {"field": "goals", "type": "quantitative"},
                "color": {"field": "team", "type": "nominal", "legend": {"orient": "bottom"}}
            }
        })
        .to_string(),
    )
    .unwrap();
    assert_eq!(pie.kind, ChartKind::Pie);
    assert_eq!(
        (pie.title.as_str(), pie.subtitle.as_deref()),
        ("Share", Some("of all goals"))
    );
    assert_eq!(labels(&pie), ["a", "b"]);
    assert_eq!(series(&pie), [("goals", vec![Some(4.0), Some(3.0)])]);

    let points = json!([
        {"x": 1, "y": 2, "g": "p"}, {"x": 1, "y": 3, "g": "p"},
        {"x": 2, "y": null, "g": "q"}, {"x": 3, "y": 1, "g": "q"}
    ]);
    let scatter = read(&vega(
        json!("point"),
        json!({
            "x": {"field": "x", "type": "quantitative"},
            "y": {"field": "y", "type": "quantitative"},
            "color": {"field": "g", "type": "nominal", "title": "Group"}
        }),
        points,
    ))
    .unwrap();
    assert_eq!(
        (scatter.kind, scatter.tier),
        (ChartKind::Scatter, Tier::Statistical)
    );
    assert_eq!(scatter.legend.as_deref(), Some("Group"));
    let Some(Plot::Points { groups, .. }) = &scatter.plot else {
        panic!("{scatter:?}")
    };
    let shown: Vec<(&str, Vec<[f64; 2]>)> = groups
        .iter()
        .map(|g| {
            (
                g.name.as_str(),
                g.points.iter().map(|p| [p.x, p.y]).collect(),
            )
        })
        .collect();
    // the point without y is left out; ties at one x stay apart
    assert_eq!(
        shown,
        [("p", vec![[1.0, 2.0], [1.0, 3.0]]), ("q", vec![[3.0, 1.0]])]
    );
}

#[test]
fn what_does_not_map_cleanly_is_refused_by_name() {
    let x = json!({"field": "region", "type": "nominal"});
    let y = json!({"field": "sales", "type": "quantitative"});
    let simple = |encoding: Value| vega(json!("bar"), encoding, sales());
    refused(
        &simple(
            json!({"x": x, "y": {"field": "sales", "type": "quantitative", "aggregate": "sum"}}),
        ),
        "aggregate in encoding y is not supported",
    );
    refused(
        &simple(json!({"x": {"field": "region", "type": "nominal", "bin": true}, "y": y})),
        "bin in encoding x is not supported",
    );
    refused(
        &simple(json!({"x": x, "y": y, "size": y})),
        "encoding size is not supported with mark bar",
    );
    refused(
        &vega(json!("tick"), json!({"x": x, "y": y}), sales()),
        "mark tick is not supported",
    );
    // two rows for North: nothing is added up
    refused(
        &simple(json!({"x": x, "y": y})),
        "rows 2 and 3 of data.values give the same category",
    );
    refused(
        &simple(json!({"x": x, "y": {"field": "region", "type": "nominal"}})),
        "one of x and y must be quantitative",
    );
    refused(&simple(json!({"x": x})), "mark bar needs x and y");
    refused(
        &simple(json!({"x": {"field": "region"}, "y": y})),
        "encoding x needs a type",
    );
    refused(
        &simple(json!({"x": {"field": "region", "type": "nominal", "sort": "-y"}, "y": y})),
        "sort \"-y\" in encoding x is not supported",
    );
    refused(
        &vega(json!("point"), json!({"x": x, "y": y}), sales()),
        "mark point needs quantitative x and y",
    );
    refused(
        &vega(json!("arc"), json!({"theta": y}), sales()),
        "mark arc needs theta (the values) and color (the slices)",
    );
    refused(
        &vega(
            json!("bar"),
            json!({"x": x, "y": {"field": "region", "type": "quantitative"}}),
            sales(),
        ),
        "row 1 of data.values: region is \"South\", not a number",
    );
    let with = |key: &str, value: Value| {
        let mut spec: Value = serde_json::from_str(&simple(json!({"x": x, "y": y}))).unwrap();
        spec[key] = value;
        spec.to_string()
    };
    refused(
        &with("transform", json!([{"filter": "true"}])),
        "transform is not supported",
    );
    refused(&with("layer", json!([])), "layer is not supported");
    refused(
        &with("data", json!({"url": "sales.csv"})),
        "data.url is not supported",
    );
    refused(
        &with(
            "$schema",
            json!("https://vega.github.io/schema/vega/v5.json"),
        ),
        "is not a Vega-Lite schema",
    );
}

fn bars() -> Value {
    json!({
        "kind": "bar",
        "tier": "presentation",
        "title": "Orders",
        "category_axis": {"title": "Shop"},
        "value_axis": {"title": "Count", "format": "F8.0"},
        "categories": [{"label": "Dock", "value": 1}, {"label": "Mill", "value": 2}],
        "series": [{"name": "Count", "statistic": "count", "values": [3, 4]}]
    })
}

#[test]
fn a_chart_that_would_not_draw_is_refused_by_name() {
    assert!(read(&bars().to_string()).is_ok());
    let with = |path: &[&str], value: Value| {
        let mut chart = bars();
        let mut at = &mut chart;
        for key in &path[..path.len() - 1] {
            at = match key.parse::<usize>() {
                Ok(i) => &mut at[i],
                Err(_) => &mut at[*key],
            };
        }
        at[path[path.len() - 1]] = value;
        chart.to_string()
    };
    refused(&with(&["colour"], json!("red")), "unknown field colour");
    refused(
        &with(&["categories", "0", "lable"], json!("x")),
        "unknown field categories[0].lable",
    );
    refused(&with(&["version"], json!(2)), "chart version 2");
    refused(&with(&["kind"], json!("donut")), "unknown variant `donut`");
    refused(
        &with(&["series", "0", "values"], json!([1])),
        "series Count: 1 values for 2 categories",
    );
    refused(
        &with(&["series", "0", "values"], json!([1, "many"])),
        "chart:",
    );
    refused(
        &with(&["categories"], json!([])),
        "categories: give 1 to 3000",
    );
    refused(
        &with(&["value_axis", "format"], json!("F99999.2")),
        "is not a display format",
    );
    refused(
        &with(&["title"], json!("x".repeat(1001))),
        "longer than 1000 characters",
    );
    refused(
        &with(&["kind"], json!("range_bar")),
        "range bar chart needs its series in pairs",
    );
    refused(
        &with(&["kind"], json!("histogram")),
        "a histogram needs a bin",
    );
    refused(
        &with(&["kind"], json!("boxplot")),
        "a chart of kind boxplot gives its data in plot",
    );
    refused(
        &with(&["second_axis"], json!({"title": "Cumulative"})),
        "second_axis: only a bar chart with two or more series",
    );
    refused(
        &with(
            &["error_bars"],
            json!({"label": "95% CI", "margins": [[1]]}),
        ),
        "error_bars: a list of margins per series",
    );
    refused(
        &with(
            &["plot"],
            json!({"shape": "merges", "items": [], "order": [], "merges": []}),
        ),
        "plot shape merges is not read",
    );
    let many: Vec<Value> = (0..3001)
        .map(|i| json!({"label": i.to_string(), "value": i}))
        .collect();
    refused(
        &with(&["categories"], json!(many)),
        "categories: give 1 to 3000",
    );
    // the place of a JSON error is that of the syntax: BEGIN CHART is line 1, bar is on line 3
    refused("{\n  \"kind\": bar\n}", "line 3, column 11: expected value");
    refused("[1, 2]", "the chart must be a JSON object");
    let mut session = Session::default();
    let errors = session.run("BEGIN CHART\nEND CHART.", None).unwrap_err();
    assert!(errors[0].message.contains("holds no chart"), "{errors:?}");
    let errors = session.run("BEGIN CHART\n{}", None).unwrap_err();
    assert!(
        errors[0].message.contains("BEGIN CHART without END CHART"),
        "{errors:?}"
    );
    let errors = session.run("END CHART.", None).unwrap_err();
    assert!(
        errors[0].message.contains("END CHART without BEGIN CHART"),
        "{errors:?}"
    );
}

#[test]
fn a_chart_needs_no_data_and_leaves_pending_transformations_alone() {
    let mut session = Session::default();
    let source = format!(
        "{DATA}COMPUTE more = loaves + 1.\n{}",
        block(&bars().to_string())
    );
    let result = session.run(&source, None).unwrap();
    assert_eq!(result.commands.last().unwrap().charts.len(), 1);
    assert!(session.dataset().unwrap().variable("more").is_ok());
}
