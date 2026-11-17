//! Parser tests of AIM and SELECTPRED.

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

fn aim(source: &str) -> Aim {
    match one(source) {
        Command::Aim(a) => *a,
        other => panic!("{other:?}"),
    }
}

fn selectpred(source: &str) -> Selectpred {
    match one(source) {
        Command::Selectpred(s) => *s,
        other => panic!("{other:?}"),
    }
}

fn names(list: &[VarItem]) -> Vec<String> {
    list.iter()
        .map(|item| match item {
            VarItem::Name(n) => n.clone(),
            other => format!("{other:?}"),
        })
        .collect()
}

#[test]
fn aim_defaults_and_options() {
    let a = aim("AIM seg /CONTINUOUS x y.");
    assert_eq!(a.group, "seg");
    assert_eq!(names(&a.continuous), ["x", "y"]);
    assert!(a.categorical.is_empty());
    assert!(a.bonferroni && a.reference_lines && !a.hide_not_significant && !a.include);
    assert_eq!(a.ci, 95.0);
    assert!(!a.category_plot && !a.error_bar);
    assert_eq!((a.cluster_plot, a.importance), (None, None));

    let a = aim(
        "AIM seg /CAT r /CONT x /CRIT ADJ=NONE CI=90 HIDENOTSIG=YES SHOWREF=NO /MISS INCLUDE \
         /PLOT CATEGORY CLUSTER (TYPE = PIE) ERRORBAR IMPORTANCE (X = VARIABLE, Y = PVALUE).",
    );
    assert_eq!(names(&a.categorical), ["r"]);
    assert!(!a.bonferroni && !a.reference_lines && a.hide_not_significant && a.include);
    assert_eq!(a.ci, 90.0);
    assert!(a.category_plot && a.error_bar);
    assert_eq!(a.cluster_plot, Some(true));
    assert_eq!(
        a.importance,
        Some(AimImportance {
            by_variable: true,
            pvalue: true
        })
    );
    let a = aim("AIM seg /CONTINUOUS x /PLOT CLUSTER IMPORTANCE /PLOT IMPORTANCE(Y=TEST).");
    // the last PLOT replaces the first
    assert_eq!(a.cluster_plot, None);
    assert_eq!(
        a.importance,
        Some(AimImportance {
            by_variable: false,
            pvalue: false
        })
    );
}

#[test]
fn aim_errors() {
    assert!(err("AIM seg.").contains("CATEGORICAL or a CONTINUOUS"));
    assert!(err("AIM seg /CONTINUOUS x /CRITERIA CI=100.").contains("below 100"));
    let e = err("AIM seg /CONTINUOUS x /PLOT CLUSTER(TYPE=LINE).");
    assert!(e.contains("LINE"), "{e}");
    assert!(err("AIM seg /CONTINUOUS x /SAVE.").contains("unknown subcommand SAVE"));
}

#[test]
fn selectpred_defaults() {
    let s = selectpred("SELECTPRED y.");
    assert_eq!(s.dependent, "y");
    assert_eq!((s.level, s.factors, s.covariates), (None, None, None));
    assert!(s.screening && !s.include && !s.plot);
    assert_eq!(
        (
            s.pct_missing,
            s.pct_equal,
            s.pct_unequal,
            s.min_sd,
            s.min_cv
        ),
        (70.0, 95.0, 90.0, 0.0, 0.001)
    );
    assert_eq!(
        (s.size, s.p_cutoff, s.timer, s.show_unselected),
        (None, 0.05, 5.0, 0)
    );
    assert_eq!(s.ranking, Ranking::PearsonChiSquare);
    assert!(s.print_cps && s.print_excluded && s.print_selected && s.print_summary);
}

#[test]
fn selectpred_options() {
    let s = selectpred(
        "SELECTPRED y (MLEVEL=N) BY a b WITH c TO e /EXCEPT VARIABLES=b \
         /SCREENING STATUS=OFF PCTMISSING=50 PCTEQUAL=80 PCTUNEQUAL=60 STDDEV=0.5 CV=0.01 \
         /CRITERIA SIZE=4 PCUTOFF=1 RANKING=LAMBDA TIMER=0 SHOWUNSELECTED=2 \
         /MISSING USERMISSING=INCLUDE /PRINT CPS SUMMARY(LRCHISQ CRAMERSV) /PLOT SUMMARY.",
    );
    assert_eq!(s.level, Some(MeasurementLevel::Nominal));
    assert_eq!(names(s.factors.as_deref().unwrap()), ["a", "b"]);
    assert_eq!(
        s.covariates,
        Some(vec![VarItem::Range("c".into(), "e".into())])
    );
    assert_eq!(names(&s.except), ["b"]);
    assert!(!s.screening && s.include && s.plot);
    assert_eq!(
        (
            s.pct_missing,
            s.pct_equal,
            s.pct_unequal,
            s.min_sd,
            s.min_cv
        ),
        (50.0, 80.0, 60.0, 0.5, 0.01)
    );
    assert_eq!(
        (s.size, s.p_cutoff, s.timer, s.show_unselected),
        (Some(4), 1.0, 0.0, 2)
    );
    assert_eq!(s.ranking, Ranking::Lambda);
    assert!(s.print_cps && s.print_summary && !s.print_excluded && !s.print_selected);
    assert_eq!(
        s.summary_criteria,
        [Ranking::LikelihoodRatio, Ranking::CramersV]
    );
    // BY alone is kept as an empty list, for the warning when it runs
    let s = selectpred("SELECTPRED y BY WITH x /PRINT NONE /MISSING.");
    assert_eq!(s.factors, Some(Vec::new()));
    assert!(!s.print_cps && !s.print_summary);
}

#[test]
fn selectpred_errors() {
    assert!(err("SELECTPRED y /CRITERIA SIZE=3 SIZE=4.").contains("twice"));
    assert!(err("SELECTPRED y /PRINT CPS /PRINT SUMMARY.").contains("twice"));
    assert!(err("SELECTPRED y /PRINT NONE CPS.").contains("NONE"));
    let e = err("SELECTPRED y /CRITERIA SIZE 3.");
    assert!(e.contains("number 3"), "{e}");
    assert!(err("SELECTPRED y /CRIT SIZE=3.").contains("unknown subcommand CRIT"));
    assert!(err("SELECTPRED y /CRITERIA RANK=LAMBDA.").contains("RANK"));
    assert!(err("SELECTPRED y /SCREENING PCTMISSING=0.").contains("above 0"));
    assert!(err("SELECTPRED y /CRITERIA PCUTOFF=1.5.").contains("at most 1"));
    assert!(err("SELECTPRED y (MLEVEL=X).").contains("X"));
}

#[test]
fn selectpred_is_never_shortened() {
    // SELECT alone stays an unknown command rather than a short SELECTPRED
    assert_eq!(err("SELECT y."), "unknown command SELECT");
    assert!(err("SELECTP y.").contains("unknown command"));
}
