use crate::ast::Command;
use crate::{Adp, AdpRescale, parse};

fn adp(source: &str) -> Adp {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    match &commands[0].command {
        Command::Adp(a) => (**a).clone(),
        other => panic!("{other:?}"),
    }
}

fn error(source: &str) -> String {
    let (_, errors) = parse(source);
    assert!(!errors.is_empty(), "{source} parsed");
    errors[0].message.clone()
}

#[test]
fn defaults() {
    let a = adp("ADP /FIELDS TARGET=y INPUT=a b /OUTFILE PREPXML='prep.sps'.");
    assert_eq!(a.target.as_deref(), Some("y"));
    assert_eq!(a.inputs.len(), 2);
    assert_eq!(a.outfile, "prep.sps");
    assert!(a.date_duration.is_some() && a.time_duration.is_some());
    assert_eq!(a.extract, [None, None, None, None, None, None]);
    assert_eq!(
        (a.max_missing, a.max_categories, a.max_single),
        (Some(50.0), Some(100), None)
    );
    assert_eq!((a.max_ordinal, a.min_continuous), (10, 5));
    assert_eq!((a.cutoff, a.outlier_to_missing), (3.0, false));
    assert!(a.replace_input.is_some() && a.replace_target.is_none());
    assert!(!a.reorder_input && !a.reorder_target);
    assert_eq!(a.rescale_input, AdpRescale::Zscore { mean: 0.0, sd: 1.0 });
    assert_eq!(a.rescale_target, Some((0.0, 1.0)));
    assert_eq!(
        (a.merge_supervised, a.binning, a.selection),
        (Some(0.05), Some(0.05), Some(0.05))
    );
    assert_eq!(a.merge_unsupervised, None);
    assert_eq!(
        (a.suffix_input.as_str(), a.suffix_target.as_str()),
        ("_transformed", "_transformed")
    );
}

#[test]
fn every_subcommand() {
    let a = adp("ADP /FIELDS INPUT=a TO c ANALYSISWEIGHT=w
  /PREPDATETIME DATEDURATION=YES(REFERENCE=YMD('2020-02-29') UNIT=MONTHS(SUFFIX='_mo'))
    TIMEDURATION=NO EXTRACTYEAR=YES EXTRACTSECOND=YES(SUFFIX='_s')
  /SCREENING PCTMISSING=NO UNIQUECAT=YES(MAXCAT=20) SINGLECAT=YES(MAXPCT=90)
  /ADJUSTLEVEL INPUT=NO MAXVALORDINAL=8 MINVALCONTINUOUS=3
  /OUTLIERHANDLING TARGET=NO CUTOFF=SD(2.5) REPLACEWITH=MISSING
  /REPLACEMISSING INPUT=YES(EXCLUDE(NOMINAL ORDINAL)) TARGET=YES
  /REORDERNOMINAL INPUT=YES TARGET=YES
  /RESCALE INPUT=MINMAX(MIN=-1 MAX=1) TARGET=NONE
  /TRANSFORM MERGESUPERVISED=NO MERGEUNSUPERVISED=YES(NOMINAL MINPCT=5) BINNING=NONE
    SELECTION=YES(PVALUE=0.1) CONSTRUCTION=NO
  /CRITERIA SUFFIX(TARGET='_t' INPUT='_p')
  /OUTFILE PREPXML='x.sps'.");
    assert_eq!(a.analysis_weight.as_deref(), Some("w"));
    let d = a.date_duration.unwrap();
    assert_eq!(
        (d.reference, d.unit, d.suffix.as_deref()),
        (Some([2020.0, 2.0, 29.0]), Some(1), Some("_mo"))
    );
    assert_eq!(a.time_duration, None);
    assert_eq!(a.extract[0].as_deref(), Some("_year"));
    assert_eq!(a.extract[5].as_deref(), Some("_s"));
    assert_eq!(
        (a.max_missing, a.max_categories, a.max_single),
        (None, Some(20), Some(90.0))
    );
    assert!(!a.adjust_input && a.adjust_target);
    assert_eq!((a.max_ordinal, a.min_continuous), (8, 3));
    assert!(a.outlier_input && !a.outlier_target && a.outlier_to_missing);
    assert_eq!(a.cutoff, 2.5);
    let r = a.replace_input.unwrap();
    assert!(r.continuous && !r.nominal && !r.ordinal);
    assert!(a.replace_target.is_some());
    assert!(a.reorder_input && a.reorder_target);
    assert_eq!(
        a.rescale_input,
        AdpRescale::MinMax {
            min: -1.0,
            max: 1.0
        }
    );
    assert_eq!(a.rescale_target, None);
    assert_eq!(
        (a.merge_supervised, a.binning, a.selection),
        (None, None, Some(0.1))
    );
    let (levels, minpct) = a.merge_unsupervised.unwrap();
    assert!(levels.nominal && !levels.ordinal);
    assert_eq!(minpct, 5.0);
    assert_eq!(
        (a.suffix_target.as_str(), a.suffix_input.as_str()),
        ("_t", "_p")
    );
}

#[test]
fn errors() {
    let base = "ADP /FIELDS INPUT=a /OUTFILE PREPXML='p.sps'";
    assert!(error("ADP /FIELDS INPUT=a.").contains("OUTFILE"));
    assert!(error("ADP /OUTFILE PREPXML='p.sps'.").contains("FIELDS"));
    assert!(error("ADP /FIELDS TARGET=y /OUTFILE PREPXML='p.sps'.").contains("INPUT"));
    assert!(error(&format!("{base} /FIELDS INPUT=b.")).contains("once"));
    assert!(error(&format!("{base} /SCREENING.")).contains("empty"));
    assert!(
        error(&format!("{base} /TRANSFORM CONSTRUCTION=YES(ROOT=f)."))
            .contains("not supported yet")
    );
    assert!(
        error(&format!(
            "{base} /ADJUSTLEVEL MAXVALORDINAL=3 MINVALCONTINUOUS=4."
        ))
        .contains("MINVALCONTINUOUS")
    );
    assert!(
        error(&format!("{base} /RESCALE INPUT=MINMAX(MIN=5 MAX=5).")).contains("MAX above MIN")
    );
    assert!(error(&format!("{base} /TRANSFORM SELECTION=YES(PVALUE=0).")).contains("above 0"));
    assert!(error(&format!("{base} /SCREENING PCTMISSING=YES(MAXPCT=101).")).contains("MAXPCT"));
    assert!(error(&format!("{base} /OUTLIERHANDLING CUTOFF=SD(0).")).contains("above 0"));
    assert!(error(&format!("{base} /REORDERNOMINAL INPUT=YES INPUT=NO.")).contains("twice"));
    assert!(
        error(&format!(
            "{base} /PREPDATETIME DATEDURATION=YES(REFERENCE=YMD('2020'))."
        ))
        .contains("three numbers")
    );
    // words in full
    assert!(!error(&format!("{base} /SCREEN PCTMISSING=NO.")).is_empty());
}
