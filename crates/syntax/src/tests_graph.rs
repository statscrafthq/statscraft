//! Parser tests of GRAPH.

use super::*;

fn graph(source: &str) -> Graph {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    match parsed.remove(0).command {
        Command::Graph(graph) => *graph,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> SyntaxError {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0)
}

fn var(name: &str) -> VarItem {
    VarItem::Name(name.to_string())
}

fn by(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| n.to_string()).collect()
}

fn summary(function: GraphFunction, names: &[&str]) -> GraphSummary {
    GraphSummary {
        function,
        variables: names.iter().map(|n| var(n)).collect(),
    }
}

#[test]
fn count_functions_and_a_bare_variable() {
    let g = graph("GRAPH /BAR=COUNT BY store.");
    assert_eq!(
        g.chart,
        GraphChart::Bar {
            stacked: false,
            data: GraphData::Summary {
                functions: vec![summary(GraphFunction::Count, &[])],
                by: by(&["store"]),
            },
        }
    );
    assert!(g.listwise && !g.report && !g.include);
    assert!(g.title.is_empty() && g.subtitle.is_none() && g.footnote.is_empty());
    // a bare variable is COUNT BY that variable
    assert_eq!(graph("GRAPH /BAR=store.").chart, g.chart);
    let g = graph("graph /bar(sta)=pct by store by daypart.");
    let GraphChart::Bar { stacked, data } = g.chart else {
        panic!()
    };
    assert!(stacked);
    assert_eq!(
        data,
        GraphData::Summary {
            functions: vec![summary(GraphFunction::Pct, &[])],
            by: by(&["store", "daypart"]),
        }
    );
    for (word, function) in [
        ("CUPCT", GraphFunction::Cupct),
        ("CUF", GraphFunction::Cufreq),
        ("COU", GraphFunction::Count),
    ] {
        let g = graph(&format!("GRAPH /LINE(MULTIPLE)={word} BY week BY store."));
        let GraphChart::Line { area: false, data } = g.chart else {
            panic!()
        };
        let GraphData::Summary { functions, .. } = data else {
            panic!()
        };
        assert_eq!(functions[0].function, function);
    }
}

#[test]
fn summary_functions_with_lists_numbers_and_abbreviations() {
    let g = graph(
        "GRAPH /LINE(AREA)=MIN(basket) MAX(basket) MEA(tip wait) PTILE(basket,25) PLT(wait 5) \
         PIN(tip, 1, 3) N(tip) GMED(basket) BY week.",
    );
    let GraphChart::Line { area: true, data } = g.chart else {
        panic!()
    };
    assert_eq!(
        data,
        GraphData::Summary {
            functions: vec![
                summary(GraphFunction::Minimum, &["basket"]),
                summary(GraphFunction::Maximum, &["basket"]),
                summary(GraphFunction::Mean, &["tip", "wait"]),
                summary(GraphFunction::Ptile(25.0), &["basket"]),
                summary(GraphFunction::Plt(5.0), &["wait"]),
                summary(GraphFunction::Pin(1.0, 3.0), &["tip"]),
                summary(GraphFunction::N, &["tip"]),
                summary(GraphFunction::Gmedian, &["basket"]),
            ],
            by: by(&["week"]),
        }
    );
    // no BY at all: one bar per function
    let g = graph("GRAPH /PIE=SUM(tip) SUM(basket).");
    assert_eq!(
        g.chart,
        GraphChart::Pie(GraphData::Summary {
            functions: vec![
                summary(GraphFunction::Sum, &["tip"]),
                summary(GraphFunction::Sum, &["basket"]),
            ],
            by: Vec::new(),
        })
    );
    let g = graph("GRAPH /BAR=VALUE(tip basket) BY receipt.");
    assert_eq!(
        g.chart,
        GraphChart::Bar {
            stacked: false,
            data: GraphData::Values {
                variables: vec![var("tip"), var("basket")],
                by: Some("receipt".into()),
            },
        }
    );
}

#[test]
fn titles_missing_and_the_first_subcommand_without_slash() {
    let g = graph(
        "GRAPH TITLE='Tips' 'by week' /SUB='spring' /FOOT 'made up' \
         /MISSING=VARIABLE REPORT INCLUDE /HIST(NORMAL)=tip.",
    );
    assert_eq!(g.title, ["Tips", "by week"]);
    assert_eq!(g.subtitle.as_deref(), Some("spring"));
    assert_eq!(g.footnote, ["made up"]);
    assert!(!g.listwise && g.report && g.include);
    assert_eq!(
        g.chart,
        GraphChart::Histogram {
            variable: "tip".into(),
            normal: true,
        }
    );
    let g = graph("GRAPH /HISTOGRAM=tip /MISSING=REPORT LISTWISE NOREPORT.");
    assert!(g.listwise && !g.report);
}

#[test]
fn scatterplots_with_groups_and_point_labels() {
    let g = graph("GRAPH /SCATTERPLOT(BIV)=wait WITH tip BY store BY receipt (NAME).");
    assert_eq!(
        g.chart,
        GraphChart::Scatter {
            x: "wait".into(),
            y: "tip".into(),
            control: Some("store".into()),
            names: Some("receipt".into()),
        }
    );
    let g = graph("GRAPH /SCAT=wait WITH tip BY receipt (IDENTIFY).");
    assert_eq!(
        g.chart,
        GraphChart::Scatter {
            x: "wait".into(),
            y: "tip".into(),
            control: None,
            names: None,
        }
    );
}

#[test]
fn error_bars_with_their_intervals() {
    let g = graph("GRAPH /ERRORBAR(CI 90)=tip BY store.");
    assert_eq!(
        g.chart,
        GraphChart::ErrorBar {
            interval: GraphInterval::Ci(90.0),
            variables: vec![var("tip")],
            by: by(&["store"]),
        }
    );
    // no keyword is a 95% interval; the number may stand in parentheses
    let interval = |source: &str| match graph(source).chart {
        GraphChart::ErrorBar { interval, .. } => interval,
        other => panic!("{other:?}"),
    };
    assert_eq!(interval("GRAPH /ERRORBAR=tip."), GraphInterval::Ci(95.0));
    assert_eq!(interval("GRAPH /ERR(CI)=tip."), GraphInterval::Ci(95.0));
    assert_eq!(
        interval("GRAPH /ERRORBAR(CI(99))=tip."),
        GraphInterval::Ci(99.0)
    );
    assert_eq!(
        interval("GRAPH /ERRORBAR(STERROR)=tip."),
        GraphInterval::Se(2.0)
    );
    assert_eq!(
        interval("graph /errorbar(ste 1.5)=tip."),
        GraphInterval::Se(1.5)
    );
    assert_eq!(
        interval("GRAPH /ERRORBAR(SE(3))=tip."),
        GraphInterval::Se(3.0)
    );
    assert_eq!(
        interval("GRAPH /ERRORBAR(STD)=tip."),
        GraphInterval::StdDev(2.0)
    );
    assert_eq!(
        interval("GRAPH /ERRORBAR(STDDEV 1)=tip."),
        GraphInterval::StdDev(1.0)
    );
    let g = graph("GRAPH /ERRORBAR(STDDEV 1)=tip basket BY daypart.");
    let GraphChart::ErrorBar {
        variables, by: b, ..
    } = g.chart
    else {
        panic!()
    };
    assert_eq!(
        (variables, b),
        (vec![var("tip"), var("basket")], by(&["daypart"]))
    );
    let g = graph("GRAPH /ERRORBAR=tip BY store BY daypart.");
    let GraphChart::ErrorBar { by: b, .. } = g.chart else {
        panic!()
    };
    assert_eq!(b, by(&["store", "daypart"]));
}

#[test]
fn pareto_charts_with_and_without_the_cumulative_line() {
    let g = graph("GRAPH /PARETO=store.");
    assert_eq!(
        g.chart,
        GraphChart::Pareto {
            cumulative: true,
            data: GraphData::Summary {
                functions: vec![summary(GraphFunction::Count, &[])],
                by: by(&["store"]),
            },
        }
    );
    let g = graph("GRAPH /PAR(NOCUM)(STACKED)=SUM(tip) BY store BY daypart.");
    assert_eq!(
        g.chart,
        GraphChart::Pareto {
            cumulative: false,
            data: GraphData::Summary {
                functions: vec![summary(GraphFunction::Sum, &["tip"])],
                by: by(&["store", "daypart"]),
            },
        }
    );
    let g = graph("GRAPH /PARETO(SIMPLE)(CUM)=VALUE(tip).");
    let GraphChart::Pareto { cumulative, data } = g.chart else {
        panic!()
    };
    assert!(cumulative);
    assert!(matches!(data, GraphData::Values { .. }));
    assert_eq!(
        err("GRAPH /PARETO=MEAN(tip) BY store.").message,
        "PARETO takes COUNT, SUM or VALUE"
    );
    assert_eq!(
        err("GRAPH /PARETO=PCT BY store.").message,
        "PARETO takes COUNT, SUM or VALUE"
    );
}

#[test]
fn ranges_drop_lines_differences_and_high_low_charts() {
    let data = GraphData::Summary {
        functions: vec![
            summary(GraphFunction::Minimum, &["tip"]),
            summary(GraphFunction::Maximum, &["tip"]),
        ],
        by: by(&["week"]),
    };
    let chart = |source: &str| graph(source).chart;
    assert_eq!(
        chart("GRAPH /BAR(RANGE)=MIN(tip) MAX(tip) BY week."),
        GraphChart::Range(data.clone())
    );
    assert_eq!(
        chart("GRAPH /LINE(DROP)=MIN(tip) MAX(tip) BY week."),
        GraphChart::Drop(data.clone())
    );
    assert_eq!(
        chart("GRAPH /LINE(DIFF)=MIN(tip) MAX(tip) BY week."),
        GraphChart::Difference(data.clone())
    );
    assert_eq!(
        chart("GRAPH /HILO=MIN(tip) MAX(tip) BY week."),
        GraphChart::Hilo(data.clone())
    );
    assert_eq!(
        chart("GRAPH /HIL(SIM)=MIN(tip) MAX(tip) BY week."),
        GraphChart::Hilo(data)
    );
    assert!(matches!(
        chart("GRAPH /HILO(GROUPED)=VALUE(a b c)."),
        GraphChart::Hilo(GraphData::Values { .. })
    ));
}

#[test]
fn panels_by_column_and_row_variables() {
    let g =
        graph("GRAPH /BAR=store /PANEL COLVAR=daypart COLOP=NEST ROWVAR=size zone ROWOP=CROSS.");
    assert_eq!(
        g.panel,
        Some(GraphPanel {
            columns: by(&["daypart"]),
            rows: by(&["size", "zone"]),
        })
    );
    let g = graph("GRAPH /PANEL ROWVAR=zone /HISTOGRAM=tip.");
    assert_eq!(
        g.panel,
        Some(GraphPanel {
            columns: Vec::new(),
            rows: by(&["zone"]),
        })
    );
    assert_eq!(graph("GRAPH /BAR=store.").panel, None);
    assert!(
        err("GRAPH /BAR=store /PANEL COLOP=NEST.")
            .expected
            .contains(&"COLVAR".to_string())
    );
    assert!(
        err("GRAPH /BAR=store /PANEL COLVAR=zone COLOP=SIDE.")
            .expected
            .contains(&"NEST".to_string())
    );
}

#[test]
fn unsupported_parts_name_themselves() {
    for (source, what) in [
        ("GRAPH /BAR=store /INTERVAL CI(95).", "subcommand INTERVAL"),
        (
            "GRAPH /BAR=store /TEMPLATE='old.sgt'.",
            "subcommand TEMPLATE",
        ),
        (
            "GRAPH /SCATTERPLOT(MATRIX)=tip wait basket.",
            "SCATTERPLOT(MATRIX)",
        ),
        (
            "GRAPH /SCATTERPLOT(XYZ)=tip WITH wait WITH basket.",
            "SCATTERPLOT(XYZ)",
        ),
        (
            "GRAPH /SCATTERPLOT=tip WITH wait WITH basket.",
            "a third axis (XYZ scatterplots)",
        ),
    ] {
        assert_eq!(
            err(source).message,
            format!("{what} of GRAPH is not supported yet"),
            "{source}"
        );
    }
}

#[test]
fn mistakes_are_errors() {
    let message = |source: &str| err(source).message;
    assert!(message("GRAPH /TITLE='x'.").starts_with("unexpected end of command"));
    assert_eq!(
        message("GRAPH /BAR=store /PIE=store."),
        "GRAPH draws one chart; give only one of BAR, LINE, PIE, HISTOGRAM, SCATTERPLOT, \
         ERRORBAR, PARETO and HILO"
    );
    assert_eq!(
        message("GRAPH /BAR=COUNT BY a BY b BY c."),
        "at most 2 BY variables are allowed"
    );
    assert_eq!(
        message("GRAPH /BAR=PTILE(tip 100) BY week."),
        "PTILE needs a percentile between 0 and 100, got 100"
    );
    assert_eq!(
        message("GRAPH /BAR=NIN(tip 5 2) BY week."),
        "NIN needs its lower bound first, got 5 and 2"
    );
    assert_eq!(
        message("GRAPH /BAR=PGT(tip) BY week."),
        "PGT needs a number after its variables"
    );
    assert_eq!(
        message("GRAPH /BAR=COUNT(tip) BY week."),
        "a count function takes no variables; write it as COUNT BY var"
    );
    assert_eq!(
        message("GRAPH /HISTOGRAM=tip wait."),
        "HISTOGRAM takes one variable"
    );
    assert_eq!(
        message("GRAPH /TITLE='a' 'b' 'c' /BAR=store."),
        "TITLE takes at most 2 lines"
    );
    assert_eq!(
        message("GRAPH /SUBTITLE='a' 'b' /BAR=store."),
        "SUBTITLE takes one line"
    );
    let long = "x".repeat(73);
    assert_eq!(
        message(&format!("GRAPH /TITLE='{long}' /BAR=store.")),
        "a line of TITLE has at most 72 characters"
    );
    assert_eq!(
        message("GRAPH /BAR=store /COLORS=red."),
        "unknown subcommand COLORS of GRAPH"
    );
    assert_eq!(
        message("GRAPH /ERRORBAR(CI 40)=tip."),
        "CI needs a level from 50 to 99.9 percent, got 40"
    );
    assert_eq!(
        message("GRAPH /ERRORBAR(STDDEV 0)=tip."),
        "STDDEV needs a number above 0, got 0"
    );
    let error = err("GRAPH /BAR=MEAN(tip) FOO(tip) BY week.");
    assert!(error.expected.contains(&"MEDIAN".to_string()), "{error:?}");
}
