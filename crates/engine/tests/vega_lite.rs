//! The Vega-Lite specifications of charts, checked against the published Vega-Lite v5 JSON
//! schema. `vega-lite/v5.json.gz` is that schema as served at
//! https://vega.github.io/schema/vega-lite/v5.json on 2026-11-12, written compactly and
//! compressed; it is BSD-3-Clause licensed (`vega-lite/LICENSE`).
//!
//! The check below covers the keywords the schema uses (references, types, anyOf, properties,
//! required, items, enum, const and the bounds); string formats are not checked.

use serde_json::{Value, json};
use statscraft_engine::Engine;
use statscraft_engine::chart::{Chart, ChartKind, Grouping, Orientation};
use statscraft_syntax::parse;
use std::io::Read;

fn schema() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/vega-lite/v5.json.gz");
    let file = std::fs::File::open(path).unwrap();
    let mut text = String::new();
    flate2::read::GzDecoder::new(file)
        .read_to_string(&mut text)
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Whether `value` meets `schema`; `root` resolves the references.
fn valid(value: &Value, schema: &Value, root: &Value) -> bool {
    let Some(rules) = schema.as_object() else {
        return schema.as_bool().unwrap_or(true);
    };
    if let Some(reference) = rules.get("$ref").and_then(Value::as_str) {
        let pointer = reference.trim_start_matches('#');
        if !valid(value, root.pointer(pointer).expect("known reference"), root) {
            return false;
        }
    }
    if let Some(choices) = rules.get("anyOf").and_then(Value::as_array)
        && !choices.iter().any(|choice| valid(value, choice, root))
    {
        return false;
    }
    if let Some(types) = rules.get("type") {
        let types: Vec<&str> = match types {
            Value::Array(list) => list.iter().filter_map(Value::as_str).collect(),
            other => vec![other.as_str().unwrap()],
        };
        let fits = |t: &str| match t {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.as_f64().is_some_and(|n| n.fract() == 0.0),
            "boolean" => value.is_boolean(),
            "null" => value.is_null(),
            _ => panic!("type {t}"),
        };
        if !types.into_iter().any(fits) {
            return false;
        }
    }
    if rules.get("const").is_some_and(|c| c != value) {
        return false;
    }
    if let Some(list) = rules.get("enum").and_then(Value::as_array)
        && !list.contains(value)
    {
        return false;
    }
    if let Some(n) = value.as_f64() {
        let bound = |key: &str| rules.get(key).and_then(Value::as_f64);
        if bound("minimum").is_some_and(|m| n < m) || bound("maximum").is_some_and(|m| n > m) {
            return false;
        }
    }
    if let Some(text) = value.as_str() {
        let length = text.chars().count() as u64;
        let bound = |key: &str| rules.get(key).and_then(Value::as_u64);
        if bound("minLength").is_some_and(|m| length < m)
            || bound("maxLength").is_some_and(|m| length > m)
        {
            return false;
        }
    }
    if let Some(items) = value.as_array() {
        let count = items.len() as u64;
        let bound = |key: &str| rules.get(key).and_then(Value::as_u64);
        if bound("minItems").is_some_and(|m| count < m)
            || bound("maxItems").is_some_and(|m| count > m)
        {
            return false;
        }
        if let Some(each) = rules.get("items")
            && !items.iter().all(|item| valid(item, each, root))
        {
            return false;
        }
    }
    if let Some(object) = value.as_object() {
        let properties = rules.get("properties").and_then(Value::as_object);
        if let Some(required) = rules.get("required").and_then(Value::as_array)
            && !required
                .iter()
                .all(|key| object.contains_key(key.as_str().unwrap()))
        {
            return false;
        }
        let bound = rules.get("minProperties").and_then(Value::as_u64);
        if bound.is_some_and(|m| (object.len() as u64) < m) {
            return false;
        }
        for (key, member) in object {
            let rule = match properties.and_then(|p| p.get(key)) {
                Some(rule) => rule,
                None => match rules.get("additionalProperties") {
                    Some(rule) => rule,
                    None => continue,
                },
            };
            if !valid(member, rule, root) {
                return false;
            }
        }
    }
    true
}

#[test]
fn the_check_tells_valid_from_invalid() {
    let root = schema();
    let good = json!({
        "data": { "values": [{ "a": 1 }] },
        "mark": { "type": "bar" },
        "encoding": { "x": { "field": "a", "type": "quantitative" } },
    });
    assert!(valid(&good, &root, &root));
    let mut wrong_mark = good.clone();
    wrong_mark["mark"]["type"] = "column".into();
    assert!(!valid(&wrong_mark, &root, &root));
    let mut unknown_member = good.clone();
    unknown_member["encoding"]["x"]["colour"] = "red".into();
    assert!(!valid(&unknown_member, &root, &root));
}

/// Every chart FREQUENCIES and CROSSTABS make, and the other kinds and layouts made from them.
fn charts() -> Vec<Chart> {
    let source = "DATA LIST LIST /score group (F2) side (A1).
BEGIN DATA
4 1 l
7 1 r
7 2 l
9 2 r
12 3 l
3 3 r
8 1 l
END DATA.
VALUE LABELS group 1 'Lower' 2 'Middle' 3 'Upper'.
FREQUENCIES group /BARCHART=PERCENT(80) /PIECHART.
FREQUENCIES score /HISTOGRAM=NORMAL FREQ(5).
CROSSTABS group BY side /BARCHART.
";
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut charts = Vec::new();
    for command in &commands {
        engine.execute(&command.command).unwrap();
        charts.extend(engine.take_charts());
    }
    assert_eq!(charts.len(), 4);
    let clustered = charts[3].clone();
    let mut other = |change: &dyn Fn(&mut Chart)| {
        let mut chart = clustered.clone();
        change(&mut chart);
        charts.push(chart);
    };
    other(&|c| c.grouping = Some(Grouping::Stacked));
    other(&|c| c.grouping = Some(Grouping::Percent));
    other(&|c| c.orientation = Some(Orientation::Horizontal));
    other(&|c| c.kind = ChartKind::Line);
    other(&|c| {
        c.kind = ChartKind::Area;
        c.grouping = Some(Grouping::Stacked);
    });
    other(&|c| c.kind = ChartKind::Scatter);
    charts
}

#[test]
fn every_kind_of_chart_gives_a_valid_specification() {
    let root = schema();
    for chart in charts() {
        let spec = chart.vega_lite();
        assert_eq!(spec["$schema"], statscraft_engine::vega_lite::SCHEMA);
        assert!(
            valid(&spec, &root, &root),
            "{}: {}",
            chart.kind_text(),
            serde_json::to_string_pretty(&spec).unwrap()
        );
    }
}

#[test]
fn specifications_hold_the_numbers_and_the_layout() {
    let charts = charts();
    // percentages of 7 valid cases, the axis up to 80
    let bars = charts[0].vega_lite();
    assert_eq!(bars["data"]["values"][0]["category"], "Lower");
    assert!((bars["data"]["values"][0]["value"].as_f64().unwrap() - 300.0 / 7.0).abs() < 1e-9);
    assert_eq!(bars["encoding"]["y"]["scale"]["domainMax"], 80.0);
    assert_eq!(
        bars["encoding"]["x"]["sort"],
        json!(["Lower", "Middle", "Upper"])
    );
    assert!(bars["encoding"].get("color").is_none());

    let pie = charts[1].vega_lite();
    assert_eq!(pie["mark"]["type"], "arc");
    assert_eq!(pie["encoding"]["theta"]["field"], "value");

    // bins of 2 from 2: 3 | 4 | 7 7 | 8 9 | | 12
    let histogram = charts[2].vega_lite();
    let bars = &histogram["layer"][0];
    assert_eq!(bars["encoding"]["x"]["bin"]["step"], 2.0);
    let counts: Vec<f64> = bars["data"]["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["value"].as_f64().unwrap())
        .collect();
    assert_eq!(counts, [1.0, 1.0, 2.0, 2.0, 0.0, 1.0]);
    let curve = histogram["layer"][1]["data"]["values"].as_array().unwrap();
    assert_eq!(curve.len(), 101);
    assert_eq!(
        histogram["title"]["subtitle"],
        charts[2].footnote.clone().unwrap()
    );

    let clustered = charts[3].vega_lite();
    assert_eq!(clustered["encoding"]["color"]["title"], "side");
    assert_eq!(clustered["encoding"]["xOffset"]["field"], "series");
    assert_eq!(clustered["encoding"]["y"]["stack"], Value::Null);
    assert_eq!(charts[4].vega_lite()["encoding"]["y"]["stack"], "zero");
    assert_eq!(charts[5].vega_lite()["encoding"]["y"]["stack"], "normalize");
    let horizontal = charts[6].vega_lite();
    assert_eq!(horizontal["encoding"]["y"]["field"], "category");
    assert_eq!(horizontal["encoding"]["yOffset"]["field"], "series");
}

/// Every statistical plot the procedures make.
fn plots() -> Vec<Chart> {
    let source = "DATA LIST LIST /score group (F2) side (A1) id (A2).
BEGIN DATA
4 1 l a1
7 1 r a2
7 2 l a3
9 2 r a4
12 3 l a5
3 3 r a6
8 1 l a7
30 1 r a8
-2 2 l a9
5 3 r b1
END DATA.
EXAMINE score BY group /ID=id /PLOT ALL.
EXAMINE score BY side /PLOT BOXPLOT /COMPARE VARIABLES.
ONEWAY score BY group /PLOT MEANS.
COMPUTE a = score * score / 10 + group.
COMPUTE b = group * group + score / 4.
FACTOR /VARIABLES score group a b /CRITERIA FACTORS(3) /PLOT EIGEN ROTATION.
REGRESSION /DEPENDENT score /METHOD ENTER group a /RESIDUALS /SCATTERPLOT (*ZRESID, *ZPRED) /PARTIALPLOT.
CLUSTER score group /PLOT DENDROGRAM VICICLE HICICLE.
TREE side [n] BY group [n] /GROWTHLIMIT MINPARENTSIZE=2 MINCHILDSIZE=1.
";
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut charts = Vec::new();
    for command in &commands {
        engine.execute(&command.command).unwrap();
        charts.extend(engine.take_charts());
    }
    charts
}

#[test]
fn every_statistical_plot_gives_a_valid_specification() {
    let root = schema();
    let charts = plots();
    let kinds: std::collections::BTreeSet<String> = charts.iter().map(|c| c.kind_text()).collect();
    for kind in [
        "Boxplot",
        "Stem-and-leaf plot",
        "Normal Q-Q plot",
        "Detrended normal Q-Q plot",
        "Spread-versus-level plot",
        "Histogram with a normal curve",
        "Line chart",
        "Scatter plot",
        "Normal P-P plot",
        "Dendrogram",
        "Icicle plot",
        "Tree diagram",
    ] {
        assert!(kinds.contains(kind), "{kind} missing from {kinds:?}");
    }
    for chart in charts {
        let spec = chart.vega_lite();
        assert!(
            valid(&spec, &root, &root),
            "{}: {}",
            chart.kind_text(),
            serde_json::to_string_pretty(&spec).unwrap()
        );
        // the data table has a row per box, line or point
        assert!(!chart.data_table().rows.is_empty(), "{}", chart.title);
    }
}
