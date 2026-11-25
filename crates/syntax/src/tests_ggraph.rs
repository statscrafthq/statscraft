//! Parser tests of GGRAPH and BEGIN GPL.

use super::*;

fn commands(source: &str) -> Vec<Command> {
    let (parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    parsed.into_iter().map(|p| p.command).collect()
}

fn ggraph(source: &str) -> Ggraph {
    match commands(source).remove(0) {
        Command::Ggraph(g) => *g,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors[0].message.clone()
}

#[test]
fn reads_the_graph_dataset_and_the_block_after_it() {
    let source = "GGRAPH /GRAPHDATASET NAME=\"pots\" VARIABLES=kiln[NAME=\"k\" LEVEL=NOMINAL] \
                  MEAN(glaze) PTILE(glaze, 90)[NAME=\"top\"] COUNT()\n\
                  MISSING=VARIABLEWISE REPORTMISSING=YES CASELIMIT=50\n\
                  /GRAPHSPEC SOURCE=INLINE LABEL=\"Kilns\" DEFAULTTEMPLATE=NO.\n\
                  BEGIN GPL\n\
                  SOURCE: s=userSource(id(\"pots\"))\n\
                  DATA: k=col(source(s), name(\"k\"),\n\
                  \x20 unit.category())\n\
                  ELEMENT: interval.stack(position(k*top), color(\"Top\"))\n\
                  GUIDE: what is this\n\
                  END GPL.";
    let parsed = commands(source);
    assert_eq!(parsed.len(), 2);
    let Command::Ggraph(g) = &parsed[0] else {
        panic!("{:?}", parsed[0]);
    };
    let set = &g.datasets[0];
    assert_eq!(
        (
            set.name.as_str(),
            set.listwise,
            set.report_missing,
            set.case_limit
        ),
        ("pots", false, true, 50)
    );
    assert_eq!(
        set.items,
        [
            GraphItem::Variables {
                item: VarItem::Name("kiln".into()),
                name: Some("k".into()),
                scale: Some(false),
            },
            GraphItem::Function {
                stat: GraphStat::Graph(GraphFunction::Mean),
                variable: Some("glaze".into()),
                name: "MEAN_glaze".into(),
                bounds: None,
            },
            GraphItem::Function {
                stat: GraphStat::Graph(GraphFunction::Ptile(90.0)),
                variable: Some("glaze".into()),
                name: "top".into(),
                bounds: None,
            },
            GraphItem::Function {
                stat: GraphStat::Graph(GraphFunction::Count),
                variable: None,
                name: "COUNT".into(),
                bounds: None,
            },
        ]
    );
    assert_eq!(g.specs[0].label.as_deref(), Some("Kilns"));
    assert_eq!(g.skipped, ["DEFAULTTEMPLATE"]);
    let Command::BeginGpl(program) = &parsed[1] else {
        panic!("{:?}", parsed[1]);
    };
    let kinds: Vec<(&str, u32, bool)> = program
        .statements
        .iter()
        .map(|s| (s.kind.as_str(), s.line, s.body.is_some()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("SOURCE", 5, true),
            ("DATA", 6, true),
            ("ELEMENT", 8, true),
            ("GUIDE", 9, false)
        ]
    );
    let element = program.statements[2].body.as_ref().unwrap();
    let name = |n: &str| GplExpr::Name(n.into());
    assert_eq!(
        *element,
        GplExpr::Call(
            "interval.stack".into(),
            vec![
                GplExpr::Call(
                    "position".into(),
                    vec![GplExpr::Op('*', Box::new(name("k")), Box::new(name("top")))]
                ),
                GplExpr::Call("color".into(), vec![GplExpr::Str("Top".into())]),
            ]
        )
    );
}

#[test]
fn reads_the_algebra_with_its_precedence() {
    let program = read_gpl(&[(
        1,
        "ELEMENT: x(position(a+b*c/d), size(-2), at(400px))".into(),
    )]);
    let body = program.statements[0].body.as_ref().unwrap();
    let name = |n: &str| Box::new(GplExpr::Name(n.into()));
    let nest = GplExpr::Op('/', name("c"), name("d"));
    let cross = GplExpr::Op('*', name("b"), Box::new(nest));
    let blend = GplExpr::Op('+', name("a"), Box::new(cross));
    assert_eq!(
        *body,
        GplExpr::Call(
            "x".into(),
            vec![
                GplExpr::Call("position".into(), vec![blend]),
                GplExpr::Call("size".into(), vec![GplExpr::Number(-2.0)]),
                GplExpr::Call("at".into(), vec![GplExpr::Name("400px".into())]),
            ]
        )
    );
}

#[test]
fn transform_and_function_names() {
    let g = ggraph(
        "GGRAPH /GRAPHDATASET NAME=\"g\" VARIABLES=MEAN(a) PIN(b, 1, 2.5) a TO c ALL \
         TRANSFORM=VARSTOCASES(SUMMARY=\"m\" INDEX=\"i\")\n/GRAPHSPEC SOURCE=INLINE /FRAME INNER=YES.",
    );
    let set = &g.datasets[0];
    assert_eq!(
        set.to_cases,
        Some(ToCases {
            summary: "m".into(),
            index: "i".into(),
            high: "#HIGH".into(),
            low: "#LOW".into(),
        })
    );
    let GraphItem::Function { name, .. } = &set.items[1] else {
        panic!("{:?}", set.items);
    };
    assert_eq!(name, "PIN_b_1_2.5");
    assert_eq!(
        set.items[2],
        GraphItem::Variables {
            item: VarItem::Range("a".into(), "c".into()),
            name: None,
            scale: None,
        }
    );
    assert_eq!(g.skipped, ["subcommand FRAME"]);
}

#[test]
fn errors_name_what_is_not_supported() {
    let base =
        |rest: &str| format!("GGRAPH /GRAPHDATASET NAME=\"g\" {rest} /GRAPHSPEC SOURCE=INLINE.");
    assert_eq!(
        err(&base("VARIABLES=a RESPONSES(b)")),
        "the function RESPONSES of GGRAPH is not supported yet"
    );
    assert_eq!(
        err(&base("VARIABLES=a MEANCI(b, 49)")),
        "the confidence level of MEANCI lies from 50 to below 100"
    );
    assert_eq!(
        err(&base("VARIABLES=a COUNTCI(100)")),
        "the confidence level of COUNTCI lies from 50 to below 100"
    );
    assert_eq!(
        err(&base("VARIABLES=a MEANSD(b, 1.5)")),
        "the multiplier of MEANSD is a whole number above 0"
    );
    assert_eq!(
        err(&base("VARIABLES=a MEANSE(b, 0)")),
        "the multiplier of MEANSE is a whole number above 0"
    );
    assert_eq!(
        err(&base("VARIABLES=a CASEVALUE(b) COUNT()")),
        "CASEVALUE does not mix with the summary functions in VARIABLES"
    );
    assert_eq!(
        err(&base("VARIABLES=GPTILE(b, 100)")),
        "the percentile of GPTILE lies above 0 and below 100"
    );
    assert_eq!(
        err(&base("VARIABLES=PTILE(b, 100)")),
        "the percentile of PTILE lies above 0 and below 100"
    );
    assert_eq!(
        err(&base("VARIABLES=a DATAFILTER=a(VALUES=1)")),
        "DATAFILTER of GGRAPH is not supported yet"
    );
    assert_eq!(
        err(&base("VARIABLES=$set COUNT()")),
        "a multiple response set in VARIABLES of GGRAPH is not supported yet"
    );
    assert_eq!(
        err("GGRAPH /GRAPHDATASET NAME=\"g\" VARIABLES=a /GRAPHSPEC SOURCE=GPLFILE(\"x.gpl\")."),
        "SOURCE=GPLFILE of GGRAPH is not supported yet"
    );
    assert_eq!(
        err("GGRAPH /GRAPHDATASET VARIABLES=a /GRAPHSPEC SOURCE=INLINE."),
        "GRAPHDATASET needs NAME=\"name\""
    );
    assert_eq!(err("END GPL."), "END GPL without BEGIN GPL");
}

#[test]
fn error_interval_functions_name_their_ends() {
    let g = ggraph(
        "GGRAPH /GRAPHDATASET NAME=\"g\" VARIABLES=kiln MEANCI(glaze, 90) MEANSD(glaze, 2)[NAME=\"m\" \
         HIGH=\"up\" LOW=\"down\"] COUNTCI(99.5) MEDIANCI(clay, 50) MEANSE(clay, 3)[NAME=\"se\"] \
         TRANSFORM=VARSTOCASES(SUMMARY=\"v\" INDEX=\"i\" HIGH=\"hi\" LOW=\"lo\")\n\
         /GRAPHSPEC SOURCE=INLINE.",
    );
    let set = &g.datasets[0];
    let ends = |k: usize| match &set.items[k] {
        GraphItem::Function {
            stat,
            name,
            bounds: Some(b),
            ..
        } => (*stat, name.clone(), b.high.clone(), b.low.clone()),
        other => panic!("{other:?}"),
    };
    let text = |a: &str, b: &str, c: &str| (a.to_string(), b.to_string(), c.to_string());
    let (stat, name, high, low) = ends(1);
    assert_eq!(stat, GraphStat::Error(ErrorFn::MeanCi(90.0)));
    assert_eq!(
        (name, high, low),
        text(
            "MEANCI_glaze_90",
            "MEANCI_glaze_90_HIGH",
            "MEANCI_glaze_90_LOW"
        )
    );
    let (stat, name, high, low) = ends(2);
    assert_eq!(stat, GraphStat::Error(ErrorFn::MeanSd(2.0)));
    assert_eq!((name, high, low), text("m", "up", "down"));
    let (stat, name, high, low) = ends(3);
    assert_eq!(stat, GraphStat::Error(ErrorFn::CountCi(99.5)));
    assert_eq!(
        (name, high, low),
        text("COUNTCI_99.5", "COUNTCI_99.5_HIGH", "COUNTCI_99.5_LOW")
    );
    assert_eq!(ends(4).0, GraphStat::Error(ErrorFn::MedianCi(50.0)));
    // NAME renames the value only; the ends keep the names of the function
    let (_, name, high, low) = ends(5);
    assert_eq!(
        (name, high, low),
        text("se", "MEANSE_clay_3_HIGH", "MEANSE_clay_3_LOW")
    );
    let to = set.to_cases.as_ref().expect("VARSTOCASES");
    assert_eq!((to.high.as_str(), to.low.as_str()), ("hi", "lo"));
    // HIGH and LOW belong to the error functions
    let message = err(
        "GGRAPH /GRAPHDATASET NAME=\"g\" VARIABLES=kiln MEAN(glaze)[HIGH=\"x\"] /GRAPHSPEC SOURCE=INLINE.",
    );
    assert_eq!(message, "unexpected word HIGH");
}
