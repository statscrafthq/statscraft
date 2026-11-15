//! Parser tests of WEIGHTED KAPPA, FLEISS MULTIRATER KAPPA and RATIO STATISTICS.

use super::*;

fn one(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn err(source: &str) -> String {
    let (_, errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    format!("{:?}", errors[0])
}

fn names(list: &[&str]) -> Vec<VarItem> {
    list.iter().map(|n| VarItem::Name(n.to_string())).collect()
}

fn weighted(source: &str) -> WeightedKappa {
    match one(source) {
        Command::WeightedKappa(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn fleiss(source: &str) -> FleissKappa {
    match one(source) {
        Command::FleissKappa(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn ratio(source: &str) -> RatioStatistics {
    match one(source) {
        Command::RatioStatistics(c) => *c,
        other => panic!("{other:?}"),
    }
}

#[test]
fn weighted_kappa_defaults_and_one_list() {
    let c = weighted("WEIGHTED KAPPA r1 r2 r3.");
    assert_eq!(c.rows, names(&["r1", "r2", "r3"]));
    assert_eq!(c.columns, None);
    assert_eq!(c.level, 95.0);
    assert!(!c.include && !c.quadratic && !c.ignore_case && !c.lower && !c.descending);
    assert_eq!(c.tabulation, None);
}

#[test]
fn weighted_kappa_settings_in_both_spellings() {
    let c = weighted(
        "WEIGHTED KAPPA a b BY c /MISSING CLASSMISSING=INCLUDE \
         /CRITERIA ASYMPTOTIC_CILEVEL(90) WEIGHTING=QUADRATIC IGNORE_CASE(TRUE) \
         /PRINT TABULATION(a BY c) CASE(LOWER) ORDER=DESCENDING.",
    );
    assert_eq!(c.columns, Some(names(&["c"])));
    assert!(c.include && c.quadratic && c.ignore_case && c.lower && c.descending);
    assert_eq!(c.level, 90.0);
    assert_eq!(
        c.tabulation,
        Some(KappaTabulation::Pairs(names(&["a"]), names(&["c"])))
    );
    let c = weighted(
        "WEI KAP a b /CRI ASYMPTOTICCILEVEL=99 IGNORECASE=FALSE /MIS CLASSMISSING(TABLE) /PRI TAB(ALL).",
    );
    assert_eq!(c.level, 99.0);
    assert_eq!(c.tabulation, Some(KappaTabulation::All));
    assert!(!c.include);
}

#[test]
fn weighted_kappa_errors() {
    assert!(
        err("WEIGHTED KAPPA a b /CRITERIA ASYMPTOTICCILEVEL=100.").contains("between 0 and 100")
    );
    assert!(err("WEIGHTED KAPPA a b /CRITERIA WEIGHTING=CUBIC.").contains("CUBIC"));
    assert!(err("WEIGHTED KAPPA a b /PRINT TABULATION(a c).").contains("BY"));
    assert!(err("WEIGHTED KAPPA a b /STATISTICS ALL.").contains("unknown subcommand"));
}

#[test]
fn weight_still_parses_next_to_weighted_kappa() {
    assert!(matches!(one("WEIGHT BY w."), Command::Weight(_)));
}

#[test]
fn fleiss_kappa_list_settings_and_variables_keyword() {
    let c = fleiss("FLEISS MULTIRATER KAPPA j1 j2 j3.");
    assert_eq!(c.variables, names(&["j1", "j2", "j3"]));
    assert_eq!(c.level, 95.0);
    assert!(!c.category && !c.include && !c.ignore_case && !c.lower);
    let c = fleiss(
        "FLEISS MULTIRATER KAPPA /VARIABLES=j1 j2 /MISSING CLASSMISSING=INCLUDE \
         /CRITERIA IGNORECASE=TRUE ASYMPTOTICCILEVEL=80 /PRINT CATEGORY CASE(LOWER).",
    );
    assert_eq!(c.variables, names(&["j1", "j2"]));
    assert!(c.category && c.include && c.ignore_case && c.lower);
    assert_eq!(c.level, 80.0);
    assert!(err("FLEISS MULTIRATER KAPPA j1 j2 /PRINT TABULATION(ALL).").contains("CATEGORY"));
    assert!(err("FLEISS MULTIRATER KAPPA j1 j2 /MISSING CLASSMISSING=TABLE.").contains("TABLE"));
}

#[test]
fn ratio_statistics_variables_and_order() {
    let c = ratio("RATIO STATISTICS rent WITH income.");
    assert_eq!(
        (c.numerator.as_str(), c.denominator.as_str()),
        ("rent", "income")
    );
    assert_eq!(c.group, None);
    assert_eq!(c.print, None);
    let c = ratio("RATIO STATISTICS rent WITH income BY town (NOSORT) /MISSING=INCLUDE.");
    assert_eq!(c.group, Some(("town".to_string(), RatioOrder::NoSort)));
    assert!(c.include);
    let c = ratio("RATIO STATISTICS rent WITH income BY town.");
    assert_eq!(c.group, Some(("town".to_string(), RatioOrder::Ascending)));
}

#[test]
fn ratio_statistics_print_and_outfile() {
    let c = ratio(
        "RATIO STATISTICS a WITH b /PRINT=MEAN MEDIAN WGTMEAN CIN(90) BCOC((0.5,0.9) (1.1,1.4)) \
         WCOC(10 20) COD PRD /OUTFILE('out.sav')=MIN MAX RANGE STDDEV AAD MNCOV MDCOV BCOC(0.9,1.1) CIN.",
    );
    let print = c.print.unwrap();
    assert!(print.mean && print.median && print.wgtmean && print.cod && print.prd);
    assert!(!print.min && !print.aad);
    assert_eq!(print.cin, Some(90.0));
    assert_eq!(print.bcoc, vec![(0.5, 0.9), (1.1, 1.4)]);
    assert_eq!(print.wcoc, vec![10.0, 20.0]);
    let (file, out) = c.outfile.unwrap();
    assert_eq!(file, "out.sav");
    assert!(out.min && out.max && out.range && out.stddev && out.aad && out.mncov && out.mdcov);
    assert_eq!(out.bcoc, vec![(0.9, 1.1)]);
    assert_eq!(out.cin, Some(95.0));
    // an empty subcommand is ignored, a repeated one keeps the last
    let c = ratio("RATIO STATISTICS a WITH b /PRINT=MEAN /PRINT /PRINT=MEDIAN.");
    let print = c.print.unwrap();
    assert!(print.median && !print.mean);
}

#[test]
fn ratio_statistics_errors() {
    assert!(err("RATIO STATISTICS a b.").contains("WITH"));
    assert!(err("RATIO STATISTICS a WITH b /PRINT=CIN(100).").contains("100"));
    assert!(err("RATIO STATISTICS a WITH b /PRINT=WCOC(0).").contains("between 0 and 100"));
    assert!(err("RATIO STATISTICS a WITH b /PRINT=BCOC(2,1).").contains("backwards"));
    assert!(err("RATIO STATISTICS a WITH b /PRINT=MODE.").contains("MODE"));
}
