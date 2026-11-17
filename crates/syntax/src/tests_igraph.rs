//! Parser tests of IGRAPH.

use super::*;

fn igraph(source: &str) -> Igraph {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    match parsed.remove(0).command {
        Command::Igraph(igraph) => *igraph,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors[0].message.clone()
}

#[test]
fn roles_and_a_bar_of_means() {
    let g = igraph(
        "IGRAPH /VIEWNAME='Bars' /X1 = VAR(region) TYPE = CATEGORICAL TITLE='Region' \
         /Y = VAR(sales) TYPE = SCALE (MIN=0 MAX=50) /COLOR = VAR(kind) TYPE = CATEGORICAL \
         LEGEND=ON STACK /COORDINATE = HORIZONTAL /X1LENGTH=3.0 /CHARTLOOK='NONE' \
         /CATORDER VAR(region) (DESCENDING COUNT OMITEMPTY) /BAR(MEAN) KEY=ON SHAPE=RECTANGLE \
         BASELINE=AUTO.",
    );
    assert_eq!(g.viewname.as_deref(), Some("Bars"));
    let x1 = g.x1.unwrap();
    assert_eq!(
        (x1.var, x1.scale, x1.title.as_deref()),
        (IgraphVar::Var("region".into()), Some(false), Some("Region"))
    );
    let y = g.y.unwrap();
    assert_eq!((y.scale, y.max), (Some(true), Some(50.0)));
    assert_eq!(g.color.unwrap().stack, Some(true));
    assert!(g.horizontal);
    assert_eq!(
        g.orders,
        [IgraphOrder {
            variable: "region".into(),
            by: IgraphOrderBy::Count,
            descending: true,
        }]
    );
    assert_eq!(g.elements, [IgraphElement::Bar(Some(GraphFunction::Mean))]);
    assert_eq!(
        g.ignored,
        ["MIN", "X1LENGTH", "CHARTLOOK", "SHAPE", "BASELINE"]
    );
    assert!(g.listwise && !g.report && !g.include);
}

#[test]
fn built_in_dimensions_and_functions_with_numbers() {
    let g = igraph("IGRAPH /X1=VAR(a) /Y=$count /BAR LABEL INSIDE N.");
    assert_eq!(g.y.unwrap().var, IgraphVar::Count);
    assert_eq!(g.elements, [IgraphElement::Bar(None)]);
    let g =
        igraph("IGRAPH /SUMMARYVAR=$pct /COLOR=VAR(a) /PIE START 180 CW SLICE=INSIDE LABEL PCT N.");
    assert_eq!(g.summary_var.unwrap().var, IgraphVar::Pct);
    for (text, function) in [
        ("NGT(5)", GraphFunction::Ngt(5.0)),
        ("PIN(1, 4)", GraphFunction::Pin(1.0, 4.0)),
        ("PTILE(90)", GraphFunction::Ptile(90.0)),
        ("SUM", GraphFunction::Sum),
        ("SUMSQ", GraphFunction::Sumsq),
        ("MED", GraphFunction::Median),
    ] {
        let g = igraph(&format!("IGRAPH /X1=VAR(a) /Y=VAR(b) /LINE ({text})."));
        assert_eq!(g.elements, [IgraphElement::Line(Some(function))], "{text}");
    }
    assert!(err("IGRAPH /Y=VAR(b) /BAR (NIN(5, 1)).").contains("lower bound first"));
}

#[test]
fn element_keywords() {
    let g = igraph(
        "IGRAPH /X1=VAR(a) /Y=VAR(b) /BOX OUTLIERS=OFF EXTREME=ON MEDIAN=ON WHISKER=T \
         CAPWIDTH (45) /ERRORBAR SE(2.0) DIRECTION BOTH CAPSTYLE=NONE.",
    );
    assert_eq!(
        g.elements,
        [
            IgraphElement::Box {
                outliers: false,
                extremes: true
            },
            IgraphElement::ErrorBar(GraphInterval::Se(2.0)),
        ]
    );
    let g = igraph("IGRAPH /X1=VAR(a) /Y=$pct /HISTOGRAM CUM X1INTERVAL WIDTH=5 CURVE=OFF.");
    assert_eq!(
        g.elements,
        [IgraphElement::Histogram {
            cumulative: true,
            curve: false
        }]
    );
    assert_eq!(g.ignored, ["X1INTERVAL"]);
    let g = igraph(
        "IGRAPH /X1=VAR(a) /Y=VAR(b) /POINTLABEL=VAR(id) ALL /SCATTER COINCIDENT=JITTER \
         /FITLINE METHOD=ORIGIN LINEAR LINE=TOTAL MEFFECT SPIKE=OFF.",
    );
    assert_eq!(g.point_label.as_deref(), Some("id"));
    assert_eq!(
        g.elements,
        [
            IgraphElement::Scatter,
            IgraphElement::FitLine {
                method: IgraphFit::Origin,
                total: true,
                groups: true
            }
        ]
    );
    assert!(err("IGRAPH /Y=VAR(b) /ERRORBAR CI(120).").contains("between 0 and 100"));
    err("IGRAPH /Y=VAR(b) /BAR FOO.");
    assert!(
        err("IGRAPH /X1=VAR(a) /Y=VAR(b) /SCATTER /FITLINE INTERVAL(95)=MEAN.")
            .contains("INTERVAL")
    );
}

#[test]
fn panels_missing_and_texts() {
    let g = igraph(
        "igraph /x1=var(a) /y=$count /panel var(p) q /missing=variablewise report include \
         /title='One\\nTwo' /subtitle='Sub' /caption='Note' /bar.",
    );
    assert_eq!(g.panel, ["p", "q"]);
    assert!(!g.listwise && g.report && g.include);
    assert_eq!(g.title.as_deref(), Some("One\\nTwo"));
    assert_eq!(
        (g.subtitle.as_deref(), g.caption.as_deref()),
        (Some("Sub"), Some("Note"))
    );
    // an empty IGRAPH parses
    assert!(igraph("IGRAPH.").elements.is_empty());
}
