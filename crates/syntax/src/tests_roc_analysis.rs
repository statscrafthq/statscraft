//! Parser tests of ROC ANALYSIS.

use super::*;

fn roc(source: &str) -> RocAnalysis {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    match parsed.remove(0).command {
        Command::RocAnalysis(r) => *r,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn defaults_and_the_plain_roc_command_stay_apart() {
    let r = roc("ROC ANALYSIS a b BY sick (1).");
    assert_eq!(r.tests.len(), 2);
    assert_eq!(
        (r.state.as_str(), &r.positive),
        ("sick", &Value::Number(1.0))
    );
    assert_eq!(r.criteria, RocCriteria::default());
    assert_eq!(r.criteria.ci, 95.0);
    assert_eq!(r.design, RocDesign::default());
    assert_eq!(r.plot, RocPlot::default());
    assert!(r.plot.roc && !r.plot.pr);
    assert_eq!(r.print, RocPrint::default());
    assert!(r.warnings.is_empty());
    // ROC alone is still the simpler command; abbreviated words name ROC ANALYSIS
    let (parsed, _) = parse("ROC a BY sick (1).");
    assert!(matches!(parsed[0].command, Command::Roc(_)));
    assert_eq!(
        roc("ROC ANAL a BY s ('y').").positive,
        Value::String("y".into())
    );
}

#[test]
fn every_subcommand_in_both_spellings() {
    let r = roc("ROC ANALYSIS a b BY s (2)
        /MISSING USERMISSING=INCLUDE
        /CRITERIA CUTOFF=EXCLUDE TESTPOS(SMALL) CI=90 DISTRIBUTION(NEGEXPO)
        /DESIGN PAIR=TRUE GROUP=g SELECT=LEVEL('m' 'f')
        /PLOT CURVE=ROC(REFERENCE) PR PR_INTERPOLATE=FP MODELQUALITY(TRUE)
        /PRINT SE(TRUE) COORDINATES=ROC PR CLASSIFIER=TRUE.");
    assert!(r.include);
    let c = &r.criteria;
    assert!(c.exclude_cutoff && c.small_positive && c.negexpo && c.ci == 90.0);
    assert!(r.design.pair);
    assert_eq!(r.design.group.as_deref(), Some("g"));
    assert_eq!(
        r.design.select,
        Some(RocSelect::Levels(
            Value::String("m".into()),
            Value::String("f".into())
        ))
    );
    let p = &r.plot;
    assert!(p.roc && p.reference && p.pr && !p.pr_reference && p.interpolate_fp && p.model_quality);
    let q = &r.print;
    assert!(q.se && q.coordinates_roc && q.coordinates_pr && q.classifier);

    let r = roc(
        "ROC ANALYSIS a BY s (1) /DESIGN GROUP(age) SELECT(CUTPOINT(40)) /PLOT CURVE(NONE) PRINTERPOLATE=TP /PRINT COORDINATES(PR).",
    );
    assert_eq!(r.design.select, Some(RocSelect::Cutpoint(40.0)));
    assert!(!r.plot.roc && !r.plot.pr);
    assert!(r.print.coordinates_pr && !r.print.coordinates_roc);
    let r =
        roc("ROC ANALYSIS a BY s (1) /DESIGN GROUP=age SELECT=MIDPOINT /PLOT CURVE=PR(REFERENCE).");
    assert_eq!(r.design.select, Some(RocSelect::Midpoint));
    assert!(!r.plot.roc && r.plot.pr && r.plot.pr_reference);
}

#[test]
fn repeats_and_contradictions_warn_and_the_last_counts() {
    // a repeated subcommand starts from the defaults again
    let r = roc("ROC ANALYSIS a BY s (1) /PRINT SE=TRUE /PRINT CLASSIFIER=TRUE.");
    assert!(!r.print.se && r.print.classifier);
    assert_eq!(
        r.warnings,
        ["PRINT is given twice; only the last one counts"]
    );
    let r = roc("ROC ANALYSIS a BY s (1) /CRITERIA TESTPOS=SMALL TESTPOS=LARGE CI=90 CI=90.");
    assert!(!r.criteria.small_positive);
    assert_eq!(
        r.warnings,
        ["TESTPOS is given twice; the later value counts"]
    );
    // of NONE and a curve, the later one counts
    assert!(
        roc("ROC ANALYSIS a BY s (1) /PLOT CURVE=NONE ROC.")
            .plot
            .roc
    );
    assert!(
        !roc("ROC ANALYSIS a BY s (1) /PLOT CURVE=ROC NONE.")
            .plot
            .roc
    );
}

#[test]
fn errors_name_what_was_expected() {
    assert!(err("ROC ANALYSIS a BY s (1) /CRITERIA CI=100.").contains("between 0 and 100"));
    assert!(err("ROC ANALYSIS a BY s (1) /DESIGN SELECT=LEVEL(1).").contains("two group values"));
    assert!(err("ROC ANALYSIS a BY s (1) /PLOT CURVE=BOTH.").contains("BOTH"));
    assert!(err("ROC ANALYSIS a BY s (1) /OPTIONS.").contains("OPTIONS"));
    // the missing BY: the list stops at the parenthesis
    assert!(err("ROC ANALYSIS a s (1).").contains("'('"));
}
