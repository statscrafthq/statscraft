//! Parser tests of OPTIMAL BINNING.

use super::*;

fn binning(source: &str) -> OptimalBinning {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    match parsed.remove(0).command {
        Command::OptimalBinning(b) => *b,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn defaults_follow_the_guide() {
    let b = binning("OPTIMAL BINNING /VARIABLES GUIDE=y BIN=a b.");
    assert_eq!(b.guide.as_deref(), Some("y"));
    assert_eq!(b.bin.len(), 2);
    assert_eq!(b.method, BinningMethod::Mdlp);
    assert_eq!(b.preprocess, Some(1000));
    assert!(!b.save && b.into.is_empty() && !b.listwise && b.rules.is_none());
    assert!(!b.lower_observed && !b.upper_observed && !b.lower_exclusive);
    assert_eq!(b.force_merge, 0.0);
    assert!(b.print_endpoints && !b.print_descriptives && !b.print_entropy);
    // without a guide equal frequency into 10 bins, and nothing to preprocess
    let b = binning("OPTIMAL BINNING /VARIABLES BIN=a.");
    assert_eq!(b.method, BinningMethod::EqualFreq(10));
    assert_eq!(b.preprocess, None);
}

#[test]
fn every_option() {
    let b = binning(
        "OPTIMAL BINNING /VARIABLES BIN=a TO c GUIDE=y SAVE=YES (INTO=x1 TO x3)
         /CRITERIA PREPROCESS=EQUALFREQ(BINS=50) METHOD=MDLP LOWEREND=OBSERVED UPPEREND=OBSERVED
           LOWERLIMIT=EXCLUSIVE FORCEMERGE=0.25
         /MISSING SCOPE=LISTWISE /OUTFILE RULES='rules.sps' /PRINT DESCRIPTIVES ENTROPY.",
    );
    assert!(b.save);
    assert_eq!(b.into, vec![VarItem::Range("x1".into(), "x3".into())]);
    assert_eq!(b.preprocess, Some(50));
    assert!(b.lower_observed && b.upper_observed && b.lower_exclusive && b.listwise);
    assert_eq!(b.force_merge, 0.25);
    assert_eq!(b.rules.as_deref(), Some("rules.sps"));
    assert!(!b.print_endpoints && b.print_descriptives && b.print_entropy);
    // METHOD=EQUALFREQ drops the guide and the preprocessing
    let b = binning(
        "OPTIMAL BINNING /VARIABLES GUIDE=y BIN=a /CRITERIA METHOD=EQUALFREQ(BINS=4) PREPROCESS=NONE.",
    );
    assert_eq!(b.method, BinningMethod::EqualFreq(4));
    assert!(b.guide.is_none() && b.preprocess.is_none());
    let b = binning("OPTIMAL BINNING /VARIABLES GUIDE=y BIN=a SAVE=NO /PRINT NONE /.");
    assert!(!b.save && !b.print_endpoints);
    // an empty PRINT is ignored and keeps the end points
    let b = binning("OPTIMAL BINNING /VARIABLES BIN=a /PRINT /MISSING SCOPE=PAIRWISE.");
    assert!(b.print_endpoints && !b.listwise);
}

#[test]
fn errors() {
    assert!(err("OPTIMAL BINNING /PRINT ENDPOINTS.").contains("VARIABLES"));
    assert!(err("OPTIMAL BINNING /VARIABLES GUIDE=y.").contains("BIN"));
    assert!(err("OPTIMAL BINNING /VARIABLES BIN=a /CRITERIA METHOD=MDLP.").contains("guide"));
    assert!(err("OPTIMAL BINNING /VARIABLES BIN=a /VARIABLES BIN=b.").contains("once"));
    assert!(err("OPTIMAL BINNING /VARIABLES BIN=a /PRINT NONE ENTROPY.").contains("NONE"));
    assert!(
        err("OPTIMAL BINNING /VARIABLES BIN=a /CRITERIA FORCEMERGE=1 FORCEMERGE=2.")
            .contains("twice")
    );
    // BINS above 1, words in full, equals signs required
    assert!(
        !err("OPTIMAL BINNING /VARIABLES BIN=a /CRITERIA METHOD=EQUALFREQ(BINS=1).").is_empty()
    );
    assert!(!err("OPTIMAL BINNING /VAR BIN=a.").is_empty());
    assert!(!err("OPTIMAL BINNING /VARIABLES BIN a.").is_empty());
    assert!(!err("OPTIMAL BINNING /VARIABLES BIN=a /CRITERIA FORCEMERGE=-1.").is_empty());
}
