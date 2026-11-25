//! Parser tests of SPCHART.

use super::*;

fn spchart(source: &str) -> Spchart {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    match parsed.remove(0).command {
        Command::Spchart(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn xr_is_the_default_chart_and_takes_both_data_layouts() {
    let c = spchart("SPCHART fill BY batch.");
    assert_eq!(c.kind, SpchartKind::Xr);
    assert_eq!(
        c.data,
        SpchartData::Units {
            process: "fill".into(),
            by: "batch".into()
        }
    );
    assert_eq!((c.sigmas, c.minsample, c.xbar_only), (3.0, 2, false));
    let c = spchart("SPCHART /XS=w1 w2 w3 BY shift (XBARONLY) /SIGMAS=2.5 /MINSAMPLE=3.");
    assert_eq!(c.kind, SpchartKind::Xs);
    assert!(c.xbar_only);
    assert_eq!(
        c.data,
        SpchartData::Subgroups {
            variables: vec![
                VarItem::Name("w1".into()),
                VarItem::Name("w2".into()),
                VarItem::Name("w3".into())
            ],
            by: Some("shift".into())
        }
    );
    assert_eq!((c.sigmas, c.minsample), (2.5, 3));
    // a variable named like a chart subcommand, followed by BY, is the XR process
    let c = spchart("SPCHART c BY batch.");
    assert_eq!(c.kind, SpchartKind::Xr);
}

#[test]
fn attribute_charts_read_counts_or_units() {
    let c = spchart("SPCHART /P=COUNT(bad) N(lot_size) BY week.");
    assert_eq!(c.kind, SpchartKind::P);
    assert_eq!(
        c.data,
        SpchartData::Counts {
            count: "bad".into(),
            size: SpchartSize::Variable("lot_size".into()),
            by: Some("week".into())
        }
    );
    let c = spchart("SPCHART /U=COUNT(flaws) N(20).");
    assert!(matches!(
        c.data,
        SpchartData::Counts {
            size: SpchartSize::Constant(20.0),
            by: None,
            ..
        }
    ));
    let c = spchart("SPCHART /NP=result BY week /NONCONFORM='fail'.");
    assert_eq!(c.conform, Some((true, Value::String("fail".into()))));
    assert!(err("SPCHART /P=result BY week.").contains("CONFORM"));
}

#[test]
fn statistics_rules_and_options() {
    let c = spchart(
        "SPCHART TITLE='Line 3' 'Night shift' /IR=temp BY hour /SPAN=3 /RULES \
         /STATISTICS=CP CPK CP CZOUT MEAN /CAPSIGMA=MRBAR /LSL=4 /USL=9 /TARGET=6.5 \
         /MISSING=REPORT INCLUDE /ID=stamp.",
    );
    assert_eq!(c.title, ["Line 3", "Night shift"]);
    assert_eq!(c.span, Some(3));
    assert_eq!(c.rules, SPCHART_RULES.to_vec());
    assert_eq!(c.statistics, ["CP", "CPK", "CZOUT", "MEAN"]);
    assert_eq!(c.capsigma, Some(CapSigma::Mrbar));
    assert_eq!((c.lsl, c.usl, c.target), (Some(4.0), Some(9.0), Some(6.5)));
    assert!(c.report && c.include);
    assert_eq!(c.id.as_deref(), Some("stamp"));
    // keywords shortened to three letters where only one keyword starts so
    let c = spchart("SPCHART /I=temp /RUL=TRU ALT /STAT=STD CAPS.");
    assert_eq!(c.rules, ["TRUP", "ALTERNATING"]);
    assert_eq!(c.statistics, ["STDDEV", "CAPSIGMA"]);
}

#[test]
fn cpchart_options() {
    let c = spchart("SPCHART /CPCHART=len /LSL=1 /USL=2 /NORMAL=BOTH /REFERENCE=SL SIGMAS.");
    assert_eq!(c.kind, SpchartKind::Capability);
    assert!(matches!(c.data, SpchartData::Individuals { .. }));
    assert_eq!(c.normal, SpchartNormal::Both);
    assert!(c.reference_limits && c.reference_sigmas);
    let c = spchart("SPCHART /CPCHART=a b c /LSL=1 /USL=2 /REFERENCE=NONE.");
    assert!(matches!(c.data, SpchartData::Subgroups { by: None, .. }));
    assert!(!c.reference_limits && !c.reference_sigmas);
}

#[test]
fn rule_breaking_specifications_are_errors() {
    assert!(err("SPCHART /XR=a BY b /IR=c.").contains("only one chart"));
    assert!(err("SPCHART /XR=a.").contains("BY variable"));
    assert!(err("SPCHART /XR=a BY b /SPAN=3.").contains("SPAN"));
    assert!(err("SPCHART /C=a BY b /CONFORM=1.").contains("CONFORM"));
    assert!(err("SPCHART /XR=a BY b /NORMAL=BOTH.").contains("CPCHART only"));
    assert!(err("SPCHART /CPCHART=a /LSL=1.").contains("LSL and USL"));
    assert!(err("SPCHART /CPCHART=a /LSL=1 /USL=2 /CAPSIGMA=RBAR.").contains("subgroups"));
    assert!(err("SPCHART /IR=a /CAPSIGMA=RBAR /STATISTICS=CP.").contains("MRBAR"));
    assert!(err("SPCHART /XR=a BY b /CAPSIGMA=RBAR SBAR.").contains("one estimator"));
    assert!(err("SPCHART /XR=a BY b /RULES /RULES=UCL.").contains("only once"));
    assert!(err("SPCHART /XR=a BY b /SIGMAS=0.5.").contains("SIGMAS"));
    assert!(err("SPCHART /XR=a BY b /LSL=5 /USL=4.").contains("below"));
    assert!(err("SPCHART /CPCHART=a /LSL=1 /USL=2 /REFERENCE=NONE SL.").contains("NONE"));
    assert!(!spchart("SPCHART /XR=a BY b.").template);
    assert!(spchart("SPCHART /TEMPLATE='c.sgt' /XR=a BY b.").template);
    assert!(err("SPCHART /TITLE='x'.").contains("chart subcommand"));
    // a CAPSIGMA that does not fit an IR chart without capability statistics is allowed
    spchart("SPCHART /IR=a /CAPSIGMA=RBAR.");
}
