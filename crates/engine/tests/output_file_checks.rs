//! An output file comes from outside: OUTPUT OPEN checks every table and chart it reads. What
//! procedures wrote opens without a warning; a crafted item (bad indexes, a cycle, a number
//! that is not finite, sizes past the caps) is kept, not drawn, and named in a warning, and
//! the rest of the file opens.

use serde_json::{Value, json};
use statscraft_engine::output_file::{self, Item};
use statscraft_engine::picture::{Paper, chart_svg};
use statscraft_engine::session::Session;
use statscraft_engine::{office, output_file::leaves};
use std::path::PathBuf;

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "statscraft-output-checks-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Made-up scores of ten players in three groups and two sides, and every kind of plot.
const SOURCE: &str = "DATA LIST LIST /score group (F2) side (A1) id (A2).
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
FREQUENCIES group /BARCHART.
CROSSTABS group BY side /BARCHART.
EXAMINE score BY group /ID=id /PLOT ALL.
COMPUTE a = score * score / 10 + group.
COMPUTE b = group * group + score / 4.
LIST.
CLUSTER score group /PLOT DENDROGRAM VICICLE HICICLE.
TREE side [n] BY group [n] /GROWTHLIMIT MINPARENTSIZE=2 MINCHILDSIZE=1 /DEPCATEGORIES TARGET=['l'].
DISCRIMINANT GROUPS=group(1,3) /VARIABLES score a b /PLOT ALL.
GRAPH /SCATTERPLOT=score WITH a BY side.
PPLOT score /DISTRIBUTION=UNIFORM.
COMPUTE high = side = 'l'.
ROC score BY high (1) /PLOT=CURVE(REFERENCE).
";

/// The output of `SOURCE`, saved, as JSON.
fn saved(dir: &std::path::Path) -> Value {
    let mut session = Session::default();
    let source = format!(
        "CD '{}'.\n{SOURCE}OUTPUT SAVE OUTFILE='all.scout'.",
        dir.display()
    );
    session.run(&source, None).unwrap();
    serde_json::from_str(&std::fs::read_to_string(dir.join("all.scout")).unwrap()).unwrap()
}

/// Every chart and table item of a file, depth first, to change.
fn items_mut(file: &mut Value) -> Vec<&mut Value> {
    fn walk<'a>(items: &'a mut Value, out: &mut Vec<&'a mut Value>) {
        for item in items.as_array_mut().unwrap() {
            if item["kind"] == "command" {
                walk(&mut item["items"], out);
            } else {
                out.push(item);
            }
        }
    }
    let mut out = Vec::new();
    walk(&mut file["items"], &mut out);
    out
}

fn chart_of<'a>(file: &'a mut Value, kind: &str) -> &'a mut Value {
    items_mut(file)
        .into_iter()
        .find(|i| i["kind"] == "chart" && i["chart"]["kind"] == kind)
        .map(|i| &mut i["chart"])
        .unwrap_or_else(|| panic!("no {kind} chart"))
}

fn opened(file: &Value) -> (Vec<Item>, Vec<String>) {
    output_file::read_checked(&file.to_string()).unwrap()
}

fn unknown_count(items: &[Item]) -> usize {
    leaves(items)
        .iter()
        .filter(|(_, i)| matches!(i, Item::Unknown(_)))
        .count()
}

#[test]
fn what_procedures_wrote_opens_without_a_warning_and_draws() {
    let dir = folder("clean");
    let file = saved(&dir);
    let (items, warnings) = opened(&file);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(unknown_count(&items), 0);
    let kinds: Vec<String> = leaves(&items)
        .iter()
        .filter_map(|(_, i)| match i {
            Item::Chart(c) => Some(c.kind_text()),
            _ => None,
        })
        .collect();
    for kind in [
        "Dendrogram",
        "Icicle plot",
        "Tree diagram",
        "Territorial map",
        "Stem-and-leaf plot",
    ] {
        assert!(kinds.iter().any(|k| k == kind), "{kind} in {kinds:?}");
    }
    for (_, item) in leaves(&items) {
        if let Item::Chart(chart) = item {
            let _ = chart_svg(chart, Paper::Light);
            let _ = chart.vega_lite();
            let _ = office::chart_workbook(chart);
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The warnings of a file after `change`, and how many items stay unread.
#[track_caller]
fn crafted(dir: &std::path::Path, change: impl FnOnce(&mut Value)) -> (usize, Vec<String>) {
    let mut file = saved(dir);
    let all = leaves(&opened(&file).0).len();
    change(&mut file);
    let (items, warnings) = opened(&file);
    // the rest of the file opens
    assert_eq!(leaves(&items).len(), all);
    for (_, item) in leaves(&items) {
        let shown = item.shown();
        if let Some(chart) = shown.1 {
            let _ = chart_svg(&chart, Paper::Dark);
            let _ = chart.vega_lite();
        }
    }
    (unknown_count(&items), warnings)
}

#[track_caller]
fn one_warning(dir: &std::path::Path, change: impl FnOnce(&mut Value), says: &[&str]) {
    let (unread, warnings) = crafted(dir, change);
    assert_eq!((unread, warnings.len()), (1, 1), "{warnings:?}");
    for part in says {
        assert!(
            warnings[0].contains(part),
            "{}\nexpected: {part}",
            warnings[0]
        );
    }
}

#[test]
fn bad_indexes_in_merges_and_trees_are_not_drawn() {
    let dir = folder("indexes");
    let merge = |field: &'static str, value: Value| {
        move |file: &mut Value| chart_of(file, "dendrogram")["plot"]["merges"][0][field] = value
    };
    one_warning(
        &dir,
        merge("left", json!(99)),
        &["the chart \"", "of CLUSTER", "merges join items"],
    );
    one_warning(&dir, merge("right", json!(10)), &["merges join items"]);
    one_warning(
        &dir,
        |file| chart_of(file, "icicle")["plot"]["order"][0] = json!(1000),
        &["order lists every item"],
    );
    // more merges than a clustering of the items can have
    one_warning(
        &dir,
        |file| {
            let plot = &mut chart_of(file, "icicle")["plot"];
            let first = plot["merges"][0].clone();
            for _ in 0..20 {
                plot["merges"].as_array_mut().unwrap().push(first.clone());
            }
        },
        &["at most one fewer than items"],
    );
    // a node that would be its own child (its id given twice), a root with a parent, and a
    // node whose parent comes after it
    one_warning(
        &dir,
        |file| {
            let nodes = chart_of(file, "tree_diagram")["plot"]["nodes"]
                .as_array_mut()
                .unwrap();
            let root = nodes[0]["id"].clone();
            nodes.push(json!({"id": root, "parent": root, "statistics": []}));
        },
        &["of TREE", "is given twice"],
    );
    one_warning(
        &dir,
        |file| {
            let nodes = &mut chart_of(file, "tree_diagram")["plot"]["nodes"];
            nodes[0]["parent"] = nodes[0]["id"].clone();
        },
        &["root"],
    );
    one_warning(
        &dir,
        |file| {
            let nodes = chart_of(file, "tree_diagram")["plot"]["nodes"]
                .as_array_mut()
                .unwrap();
            let root = nodes[0]["id"].as_u64().unwrap();
            nodes.push(json!({"id": root + 1000, "parent": root + 2000, "statistics": []}));
            nodes.push(json!({"id": root + 2000, "parent": root, "statistics": []}));
        },
        &["comes after its parent"],
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn numbers_that_are_not_finite_where_a_drawing_needs_them_are_not_drawn() {
    let dir = folder("nan");
    one_warning(
        &dir,
        |file| chart_of(file, "scatter")["plot"]["groups"][0]["points"][0]["x"] = Value::Null,
        &["invalid type: null"],
    );
    one_warning(
        &dir,
        |file| chart_of(file, "boxplot")["plot"]["boxes"][0]["median"] = Value::Null,
        &["of EXAMINE"],
    );
    // a value of a series may be missing: null reads as a gap
    let (unread, warnings) = crafted(&dir, |file| {
        chart_of(file, "bar")["series"][0]["values"][0] = Value::Null;
    });
    assert_eq!((unread, warnings.len()), (0, 0), "{warnings:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sizes_past_the_caps_and_tables_whose_parts_do_not_fit_are_not_shown() {
    let dir = folder("sizes");
    one_warning(
        &dir,
        |file| {
            let chart = chart_of(file, "bar");
            let category = chart["categories"][0].clone();
            let many = vec![category; 100_001];
            chart["categories"] = json!(many);
        },
        &["categories: give 1 to 100000"],
    );
    one_warning(
        &dir,
        |file| chart_of(file, "bar")["title"] = json!("x".repeat(40_000)),
        &["longer than 32768 characters"],
    );
    one_warning(
        &dir,
        |file| chart_of(file, "bar")["value_axis"]["format"] = json!("F999999999.2"),
        &["is not a display format"],
    );
    fn table(file: &mut Value) -> &mut Value {
        let items = items_mut(file);
        items.into_iter().find(|i| i["kind"] == "table").unwrap()
    }
    one_warning(
        &dir,
        |file| {
            table(file)["rows"][0]
                .as_array_mut()
                .unwrap()
                .push(json!(1))
        },
        &["the table \"", "cells for"],
    );
    one_warning(
        &dir,
        |file| {
            let t = table(file);
            let rows = t["rows"].as_array().unwrap().len();
            t["formats"] = json!(vec![vec!["F8.2"]; rows + 3]);
        },
        &["formats do not have the shape"],
    );
    one_warning(
        &dir,
        |file| table(file)["row_headers"] = json!(500),
        &["headers do not fit"],
    );
    one_warning(
        &dir,
        |file| {
            let t = table(file);
            t["columns"] = json!(vec![""; 10_001]);
            t["rows"] = json!([]);
        },
        &["more than a table may hold"],
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unknown_fields_of_a_newer_version_are_dropped_and_the_item_opens() {
    let dir = folder("fields");
    let (unread, warnings) = crafted(&dir, |file| {
        chart_of(file, "bar")["colour"] = json!("red");
        let items = items_mut(file);
        let table = items.into_iter().find(|i| i["kind"] == "table").unwrap();
        table["shading"] = json!({"rows": 2});
    });
    assert_eq!((unread, warnings.len()), (0, 0), "{warnings:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn output_open_warns_about_an_item_it_does_not_show_and_opens_the_rest() {
    let dir = folder("open");
    let mut file = saved(&dir);
    let all = leaves(&opened(&file).0).len();
    chart_of(&mut file, "dendrogram")["plot"]["merges"][0]["left"] = json!(99);
    std::fs::write(dir.join("crafted.scout"), file.to_string()).unwrap();
    let mut session = Session::default();
    let source = format!("CD '{}'.\nOUTPUT OPEN FILE='crafted.scout'.", dir.display());
    let result = session.run(&source, None).unwrap();
    let open = &result.commands[1];
    assert_eq!(open.warnings.len(), 1, "{:?}", open.warnings);
    assert!(
        open.warnings[0].starts_with("crafted.scout: the chart \""),
        "{:?}",
        open.warnings
    );
    let name = session.designated_output().unwrap().to_string();
    let items = session.output_items(&name).unwrap();
    assert_eq!(leaves(items).len(), all);
    // saved again, the item is written back as it was read
    let again = format!(
        "CD '{}'.\nOUTPUT SAVE OUTFILE='again.scout'.",
        dir.display()
    );
    session.run(&again, None).unwrap();
    let text = std::fs::read_to_string(dir.join("again.scout")).unwrap();
    assert!(text.contains("\"left\": 99"), "kept as it was");
    let _ = std::fs::remove_dir_all(&dir);
}
