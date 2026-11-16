//! Parser tests of TWOSTEP CLUSTER (and DMCLUSTER) and DETECTANOMALY.

use super::*;

fn one(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

fn twostep(source: &str) -> Twostep {
    match one(source) {
        Command::Twostep(t) => *t,
        other => panic!("{other:?}"),
    }
}

fn anomaly(source: &str) -> Detectanomaly {
    match one(source) {
        Command::Detectanomaly(d) => *d,
        other => panic!("{other:?}"),
    }
}

#[test]
fn twostep_defaults_and_options() {
    let t = twostep("TWOSTEP CLUSTER /CONTINUOUS VARIABLES=a b /PRINT COUNT.");
    assert_eq!(t.continuous.len(), 2);
    assert!(t.categorical.is_empty());
    assert_eq!((t.threshold, t.branch, t.levels), (0.0, 8, 3));
    assert_eq!(
        t.count,
        ClusterCount::Auto {
            max: 15,
            aic: false
        }
    );
    assert!(!t.euclidean && t.view && t.print_count && !t.print_ic);
    assert_eq!(t.save, None);

    let t = twostep(
        "TWOSTEP CLUSTER /CAT VAR=r s /CONT VAR=a /CRIT INITTHRESHOLD(0.5) MXBRANCH(4) MXLEV(2) \
         /HANDLENOISE 25 /MISSING INCLUDE /NOSTANDARDIZE VARIABLES=a /NUMCLUSTERS AUTO 6 AIC \
         /PRINT IC SUMMARY /VIEWMODEL DISPLAY=NO EVALUATIONFIELDS=z /SAVE CLUSTER VARIABLE=grp \
         /MEMALLOCATE 16 /OUTFILE MODEL='m.xml' STATE='s.xml'.",
    );
    assert_eq!((t.threshold, t.branch, t.levels), (0.5, 4, 2));
    assert_eq!(t.noise, 25.0);
    assert!(t.include && !t.view && t.print_ic && t.print_summary && !t.print_count);
    assert_eq!(t.count, ClusterCount::Auto { max: 6, aic: true });
    assert_eq!(t.unstandardized, [VarItem::Name("a".into())]);
    assert_eq!(t.evaluation, [VarItem::Name("z".into())]);
    assert_eq!(t.save, Some(Some("grp".into())));
    assert_eq!(t.memory, 16);
    assert_eq!(t.model_file.as_deref(), Some("m.xml"));

    // the last of a repeated subcommand counts; an empty one changes nothing
    let t = twostep(
        "DMCLUSTER /CONTINUOUS VARIABLES=a /NUMCLUSTERS FIXED=3 /NUMCLUSTERS FIXED 4 /PRINT /DISTANCE EUCLIDEAN.",
    );
    assert_eq!(t.count, ClusterCount::Fixed(4));
    assert!(t.euclidean);
}

#[test]
fn twostep_errors() {
    assert!(err("TWOSTEP CLUSTER /PRINT COUNT.").contains("needs CATEGORICAL"));
    assert!(
        err("TWOSTEP CLUSTER /CATEGORICAL VARIABLES=r /DISTANCE EUCLIDEAN.")
            .contains("only continuous")
    );
    assert!(err("TWOSTEP CLUSTER /CONTINUOUS VARIABLES=a /MEMALLOCATE 2.").contains("at least 4"));
    assert!(err("TWOSTEP CLUSTER /CONTINUOUS VARIABLES=a /HANDLENOISE 120.").contains("0 to 100"));
    assert!(
        err("TWOSTEP CLUSTER /CONTINUOUS VARIABLES=a /CRITERIA MXBRANCH(1).")
            .contains("at least 2")
    );
}

#[test]
fn detectanomaly_defaults_and_options() {
    let d = anomaly("DETECTANOMALY.");
    assert!(d.categorical.is_none() && d.scale.is_none() && d.id.is_none());
    assert!(!d.apply && !d.missing_proportion);
    assert_eq!((d.min_peers, d.max_peers, d.reasons), (1, 15, 1));
    assert_eq!((d.ml_weight, d.percent, d.cutpoint), (6.0, 5.0, Some(2.0)));
    assert!(d.print_list && !d.print_cps);

    let d = anomaly(
        "DETECTANOMALY /VARIABLES CATEGORICAL=r s SCALE=a TO c ID=key EXCEPT=s \
         /HANDLEMISSING APPLY=YES CREATEMISPROPVAR=YES \
         /CRITERIA MINNUMPEERS=2 MAXNUMPEERS=4 MLWEIGHT=3 NUMREASONS=2 NUMANOMALOUSCASES=7 ANOMALYCUTPOINT=NONE \
         /SAVE ANOMALY(ai) PEERID REASONVAR(why) /OUTFILE MODEL='x.xml' /PRINT NONE NORMS.",
    );
    assert_eq!(d.categorical.as_ref().unwrap().len(), 2);
    assert_eq!(d.scale, Some(vec![VarItem::Range("a".into(), "c".into())]));
    assert_eq!(d.id.as_deref(), Some("key"));
    assert_eq!(d.except, [VarItem::Name("s".into())]);
    assert!(d.apply && d.missing_proportion);
    assert_eq!(
        (d.min_peers, d.max_peers, d.reasons, d.count),
        (2, 4, 2, Some(7))
    );
    assert_eq!((d.ml_weight, d.cutpoint), (3.0, None));
    assert_eq!(
        d.save,
        [
            (AnomalySave::Anomaly, Some("ai".into())),
            (AnomalySave::PeerId, None),
            (AnomalySave::ReasonVar, Some("why".into())),
        ]
    );
    // NONE with another keyword: the other keyword counts
    assert!(d.print_norms && !d.print_list);
    let d = anomaly("DETECTANOMALY /PRINT NONE.");
    assert!(!d.print_list && !d.print_norms && !d.print_cps);
}

#[test]
fn detectanomaly_errors() {
    assert!(err("DETECTANOMALY /PRINT CPS /PRINT NORMS.").contains("once"));
    assert!(err("DETECTANOMALY /CRITERIA NUMREASONS=1 NUMREASONS=2.").contains("twice"));
    assert!(err("DETECTANOMALY /CRITERIA MINNUMPEERS=5 MAXNUMPEERS=3.").contains("larger than"));
    assert!(err("DETECTANOMALY /VARIABLES EXCEPT=ALL.").contains("ALL"));
    // keywords in full and equals signs required
    assert!(!err("DETECTANOMALY /CRITERIA NUMREAS=2.").is_empty());
    assert!(!err("DETECTANOMALY /HANDLEMISSING APPLY YES.").is_empty());
    assert!(!err("DETECTANOMALY /CRITERIA PCTANOMALOUSCASES=150.").is_empty());
}
