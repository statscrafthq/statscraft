//! Parser tests of XGRAPH.

use super::*;

fn xgraph(source: &str) -> Xgraph {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    match parsed.remove(0).command {
        Command::Xgraph(xgraph) => *xgraph,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors[0].message.clone()
}

fn var(name: &str, scale: Option<bool>) -> XgraphDim {
    XgraphDim::Var {
        name: name.into(),
        scale,
    }
}

#[test]
fn the_chart_expression() {
    let g = xgraph(
        "XGRAPH CHART=(spend [MEAN] [BAR]) > tier [PATTERN] BY shop[c] > day[COLOR] BY zone.",
    );
    assert_eq!(
        g.y,
        [XgraphY {
            variables: vec!["spend".into()],
            stat: Some(XgraphStat::Summary(GraphFunction::Mean)),
        }]
    );
    assert_eq!(g.element, XgraphElement::Bar);
    assert_eq!(g.stack, Some(var("tier", None)));
    let x = g.x.unwrap();
    assert_eq!(
        (x.dim, x.cluster),
        (var("shop", Some(false)), Some(var("day", None)))
    );
    assert_eq!(g.z.unwrap().dim, var("zone", None));
    assert!(g.listwise && !g.report);

    // blended variables, one function over a list, a count, the placeholder and the cases
    let g = xgraph("XGRAPH ((a + b) [PTILE(25)] [LINE] + c [PIN(1, 5)] [LINE]) BY 1 BY g[s].");
    assert_eq!(g.y[0].variables, ["a", "b"]);
    assert_eq!(
        g.y[0].stat,
        Some(XgraphStat::Summary(GraphFunction::Ptile(25.0)))
    );
    assert_eq!(
        g.y[1].stat,
        Some(XgraphStat::Summary(GraphFunction::Pin(1.0, 5.0)))
    );
    assert_eq!(
        (g.element, g.x.unwrap().dim),
        (XgraphElement::Line, XgraphDim::One)
    );
    let g = xgraph("XGRAPH (v [VALUE]) BY $CASENUM [LABEL=who] BY 1.");
    assert_eq!(g.y[0].stat, Some(XgraphStat::Value));
    assert_eq!(
        g.x.unwrap().dim,
        XgraphDim::Cases {
            label: Some("who".into())
        }
    );
    let g = xgraph("XGRAPH ([CSUM] [BAR] + [VALIDN] [BAR]) BY k.");
    assert_eq!(g.y[0].stat, Some(XgraphStat::Summary(GraphFunction::Cusum)));
    assert_eq!(g.y[1].stat, Some(XgraphStat::Summary(GraphFunction::N)));
    assert_eq!(
        xgraph("XGRAPH ([HISTOBAR]) BY age.").element,
        XgraphElement::Histobar
    );
}

#[test]
fn subcommands() {
    let g = xgraph(
        "XGRAPH ([HISTOBAR]) BY age[s] BY sex[c] /BIN START=10 SIZE=WIDTH(5) /DISTRIBUTION \
         TYPE=NORMAL /COORDINATE SPLIT=YES /MISSING USE=VARIABLEWISE REPORT=YES /PANEL \
         COLVAR=site /TEMPLATE FILE='look.sgt' /TITLES TITLE='One' 'Two' SUBTITLE='Sub' \
         FOOTNOTE='Foot' /DISPLAY DOT=FLAT /ERRORBAR SE(1).",
    );
    assert_eq!(g.bin, Some((Some(10.0), XgraphBinSize::Width(5.0))));
    assert!(g.normal && g.split && g.report && !g.listwise && g.template);
    assert_eq!(g.panel.unwrap().columns, ["site"]);
    assert_eq!(g.title, ["One", "Two"]);
    assert_eq!(
        (g.subtitle.unwrap(), g.footnote),
        ("Sub".into(), vec!["Foot".into()])
    );
    assert_eq!(g.dot, Some(XgraphDot::Flat));
    assert_eq!(g.interval, Some(GraphInterval::Se(1.0)));
    let g = xgraph("XGRAPH ([HISTOBAR]) BY age /BIN START=AUTO SIZE=COUNT(4) /ERRORBAR CI(90).");
    assert_eq!(g.bin, Some((None, XgraphBinSize::Count(4))));
    assert_eq!(g.interval, Some(GraphInterval::Ci(90.0)));
}

#[test]
fn errors() {
    assert!(
        err("XGRAPH (a [MEAN] [BAR]) BY g /TITLES TITLE='x' /TITLES TITLE='y'.")
            .contains("given twice")
    );
    err("XGRAPH (a [MEAN BAR]) BY g.");
    assert!(err("XGRAPH (a [MEAN] [SUM]) BY g.").contains("one function"));
    assert!(err("XGRAPH (a [MEAN] [BAR] + b [MEAN] [POINT]) BY g.").contains("same one"));
    assert!(err("XGRAPH (a [GPTILE(5)]) BY g.").contains("not supported yet"));
    assert!(err("XGRAPH (a [PTILE(100)]) BY g.").contains("between 0 and 100"));
    assert!(err("XGRAPH (a [NIN(5, 1)]) BY g.").contains("lower bound first"));
    assert!(err("XGRAPH (a [MEAN]) > s BY g > h BY z > k.").contains("two clusters"));
    assert!(err("XGRAPH ([HISTOBAR]) BY age /BIN SIZE=COUNT(2.5).").contains("whole number"));
    // keywords are written in full
    err("XGRAPH (a [MEAN]) BY g /MISSING USE=LIST.");
    err("XGRAPH (a [MEAN]) BY g /TITLES.");
}
