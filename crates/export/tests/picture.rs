//! Charts as pictures, from real runs on a small made-up survey of library visits: the SVG of a
//! few charts against files kept beside this test, and the PNG they render to.
//!
//! After a deliberate change to the drawing, `STATSCRAFT_BLESS=1 cargo test -p
//! statscraft-export --test picture` writes the files again; look at them before committing.

use statscraft_engine::Chart;
use statscraft_engine::session::Session;
use statscraft_export::picture::{self, Paper, Shape};

/// visits per month, branch, age group (9 = not given, user-missing).
const DATA: &str = "DATA LIST LIST /visits branch age (F2).
BEGIN DATA
4 1 1
7 1 2
2 2 1
11 2 3
6 3 2
9 1 3
3 2 2
14 3 3
5 1 1
8 2 2
1 3 9
10 1 2
END DATA.
VALUE LABELS branch 1 'Harbour' 2 'Old Town' 3 'Hillside'
 /age 1 'Under 30' 2 '30 to 59' 3 '60 and over' 9 'Not given'.
MISSING VALUES age (9).
";

fn charts(source: &str) -> Vec<Chart> {
    let result = Session::default()
        .run(&format!("{DATA}{source}"), None)
        .unwrap();
    result.commands.into_iter().flat_map(|c| c.charts).collect()
}

fn chart(source: &str) -> Chart {
    charts(source).remove(0)
}

/// The SVG of the chart against `tests/pictures/<name>.svg`.
fn golden(name: &str, chart: &Chart, paper: Paper) {
    let svg = picture::chart_svg(chart, paper).expect("a picture");
    let path = format!("{}/tests/pictures/{name}.svg", env!("CARGO_MANIFEST_DIR"));
    if std::env::var_os("STATSCRAFT_BLESS").is_some() {
        std::fs::write(&path, &svg).unwrap();
    }
    let want = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        svg == want,
        "{name}.svg differs; STATSCRAFT_BLESS=1 writes it again\n{svg}"
    );
}

#[test]
fn a_bar_chart_of_frequencies() {
    golden(
        "frequencies-bar",
        &chart("FREQUENCIES branch /BARCHART."),
        Paper::Light,
    );
}

#[test]
fn a_pie_chart_with_the_missing_slice_on_dark_paper() {
    let pie = chart("FREQUENCIES age /PIECHART.");
    golden("frequencies-pie-dark", &pie, Paper::Dark);
}

#[test]
fn a_histogram_with_its_normal_curve() {
    let histogram = chart("FREQUENCIES visits /HISTOGRAM=NORMAL /FORMAT=NOTABLE.");
    golden("frequencies-histogram", &histogram, Paper::Light);
}

#[test]
fn a_clustered_bar_chart_of_a_crosstabulation() {
    golden(
        "crosstabs-bar",
        &chart("CROSSTABS branch BY age /BARCHART."),
        Paper::Light,
    );
}

#[test]
fn every_kind_draws_in_both_papers() {
    let mut base = chart("CROSSTABS branch BY age /BARCHART.");
    use statscraft_engine::chart::{ChartKind, Grouping, Orientation};
    for kind in [
        ChartKind::Bar,
        ChartKind::Line,
        ChartKind::Area,
        ChartKind::Scatter,
    ] {
        for grouping in [Grouping::Clustered, Grouping::Stacked, Grouping::Percent] {
            for orientation in [Orientation::Vertical, Orientation::Horizontal] {
                base.kind = kind;
                base.grouping = Some(grouping);
                base.orientation = Some(orientation);
                for paper in [Paper::Light, Paper::Dark] {
                    let scene = picture::scene(&base, 500.0, paper).expect("drawn");
                    assert!(scene.height > 200.0, "{kind:?} {grouping:?}");
                    let marks = scene.shapes.iter().filter(|s| {
                        matches!(
                            s,
                            Shape::Rect { .. } | Shape::Band { .. } | Shape::Circle { .. }
                        )
                    });
                    assert!(marks.count() >= 3, "{kind:?} {grouping:?}");
                }
            }
        }
    }
}

#[test]
fn the_png_has_the_scene_s_size_twice_over_and_something_on_it() {
    let bar = chart("FREQUENCIES branch /BARCHART.");
    let scene = picture::scene(&bar, picture::FILE_WIDTH, Paper::Light).unwrap();
    let png = picture::chart_png(&bar, Paper::Light).unwrap().unwrap();
    assert_eq!(&png[1..4], b"PNG");
    // the header's width and height, big-endian after the IHDR tag
    let size = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    assert_eq!(size(16), (scene.width * 2.0).ceil() as u32);
    assert_eq!(size(20), (scene.height * 2.0).ceil() as u32);
    // white paper with petrol bars and text compresses to more than a blank page would
    let blank = picture::png(
        &picture::Scene {
            shapes: Vec::new(),
            ..scene.clone()
        },
        2.0,
    )
    .unwrap();
    assert!(
        png.len() > 2 * blank.len(),
        "{} against {}",
        png.len(),
        blank.len()
    );
}
