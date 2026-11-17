//! Parser tests of SIMPLAN, SIMRUN and BEGIN EXPR / END EXPR.

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

fn plan(source: &str) -> Simplan {
    match one(source) {
        Command::Simplan(p) => *p,
        other => panic!("{other:?}"),
    }
}

#[test]
fn simplan_inputs_distributions_and_options() {
    let p = plan(
        "SIMPLAN CREATE /MODEL FILE='m.json' CUSTOM=YES
          /TARGETOPTS TARGET=margin(FORMAT=DOLLAR,2) COMMENT='per week' TARGETFILTER=YES(MIN=0 MAX=900(EXCLUSIVE))
          /SIMINPUT INPUT=units(FORMAT=F,0) TYPE=MANUAL(LOCK=NO) DISTRIBUTION=TRIANGULAR(MIN=8 MAX=14 MODE=10)
          /SIMINPUT INPUT=cost(MAPTO=unit_cost) TYPE=AUTOFIT(GAMMA LNORMAL) MINVAL=0.5
          /FIXEDINPUT INPUT=price VALUE=12
          /CORRELATIONS VARORDER=units cost CORRMATRIX=1; -0.25, 1
          /AUTOFIT NCASES=50 FIT=KS BINS=20
          /STOPCRITERIA MAXCASES=5000 TARGET=margin TYPE=MEAN(THRESH=2 UNIT=PERCENT CILEVEL=99)
          /MISSING CLASSMISSING=INCLUDE
          /PLAN FILE='p.splan' DISPLAY=NO.",
    );
    assert_eq!(p.model, Some(("m.json".into(), true)));
    let t = &p.targets[0];
    assert_eq!(t.targets[0].format.as_deref(), Some("DOLLAR,2"));
    assert_eq!(t.comment.as_deref(), Some("per week"));
    let f = t.filter.expect("a filter");
    assert_eq!(
        (f.min, f.min_inclusive, f.max, f.max_inclusive),
        (Some(0.0), true, Some(900.0), false)
    );
    let d = p.inputs[0].distribution.as_ref().expect("given");
    assert_eq!(p.inputs[0].autofit, None);
    assert_eq!(d.family, "TRIANGULAR");
    assert_eq!(d.params[2], ("MODE".into(), vec![vec![10.0]]));
    assert_eq!(p.inputs[1].inputs[0].mapto.as_deref(), Some("unit_cost"));
    assert_eq!(
        p.inputs[1].autofit,
        Some(vec!["GAMMA".into(), "LNORMAL".into()])
    );
    assert_eq!(p.inputs[1].minval, Some(0.5));
    assert_eq!(p.fixed[0].values, [12.0]);
    let (order, matrix) = p.correlations.expect("correlations");
    assert_eq!(order, ["units", "cost"]);
    assert_eq!(matrix, [[1.0, -0.25], [-0.25, 1.0]]);
    assert_eq!((p.ncases, p.ks, p.bins), (Some(50), true, 20));
    assert_eq!(p.max_cases, 5000);
    assert_eq!(
        p.stop_rule,
        Some(StopRule::Mean {
            thresh: 2.0,
            percent: true,
            level: 99.0
        })
    );
    assert!(p.include_missing && !p.display);
}

#[test]
fn simplan_sensitivity_values_and_their_rules() {
    let p = plan(
        "SIMPLAN CREATE /SIMINPUT INPUT=a TYPE=MANUAL DISTRIBUTION=CATEGORICAL(CATS=1 2 3 PROBS=0.5 0.3 0.2; 0.2 0.3 0.5)
          /PLAN FILE='p.splan'.",
    );
    let d = p.inputs[0].distribution.as_ref().expect("given");
    assert_eq!(d.params[1].1.len(), 2);
    let p = plan(
        "SIMPLAN CREATE /SIMINPUT INPUT=a TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=1 STDDEV=1)
          /FIXEDINPUT INPUT=b ITERATIONS=RANGE(MIN=20 MAX=50 N=4) /PLAN FILE='p.splan'.",
    );
    assert_eq!(p.fixed[0].values, [20.0, 30.0, 40.0, 50.0]);
    assert!(
        err("SIMPLAN CREATE /SIMINPUT INPUT=a b TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=1 2 STDDEV=1) /PLAN FILE='p'.")
            .contains("single input")
    );
    assert!(
        err(
            "SIMPLAN CREATE /SIMINPUT INPUT=a TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=1 2 STDDEV=1)
             /FIXEDINPUT INPUT=b ITERATIONS=VALUES(1 2) /PLAN FILE='p'."
        )
        .contains("only one")
    );
    assert!(
        err("SIMPLAN CREATE /SIMINPUT INPUT=a TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=1 2 STDDEV=1 2) /PLAN FILE='p'.")
            .contains("only one parameter")
    );
}

#[test]
fn simplan_rejects_what_the_rules_forbid() {
    assert!(
        err(
            "SIMPLAN CREATE /SIMINPUT INPUT=a DISTRIBUTION=NORMAL(MEAN=1 STDDEV=1) /PLAN FILE='p'."
        )
        .contains("TYPE=MANUAL")
    );
    assert!(
        err("SIMPLAN CREATE /SIMINPUT INPUT=a TYPE=MANUAL /PLAN FILE='p'.")
            .contains("DISTRIBUTION")
    );
    assert!(
        err("SIMPLAN CREATE /SIMINPUT INPUT=a TYPE=AUTOFIT(NORMAL POISSON) /PLAN FILE='p'.")
            .contains("mix")
    );
    assert!(
        err("SIMPLAN CREATE /SIMINPUT INPUT=a /PLAN FILE='p' /PLAN FILE='q'.").contains("once")
    );
    assert!(err("SIMPLAN CREATE /PLAN FILE='p'.").contains("SIMINPUT"));
    assert!(err("SIMPLAN CREATE /SIMINPUT INPUT=a /CORRELATIONS VARORDER=a b CORRMATRIX=1; 2, 1 /PLAN FILE='p'.").contains("row 2"));
    assert!(err("SIMPLAN VIEW /SIMINPUT INPUT=a /PLAN FILE='p'.").contains("PLAN only"));
    assert!(matches!(one("SIMPLAN VIEW /PLAN FILE='p.splan'."), Command::Simplan(p) if p.view));
}

#[test]
fn simrun_options() {
    let Command::Simrun(r) = one(
        "SIMRUN /PLAN FILE='p.splan' /CRITERIA REPRESULTS=TRUE SEED=4711
          /DISTRIBUTION DISPLAY=PDF CDF CDFORDER=DESCENDING SCALE=MEAN PCT(10 90) REFLINES(LOWER=LO)
          /BOXPLOT DISPLAY=YES /SCATTERPLOT DISPLAY=YES ROWSIZE=3
          /TORNADO TYPE=CORR DELTASTDDEV(0.5) VARIANCE
          /PRINT DESCRIPTIVES=YES(CILEVEL=90) PERCENTILES=YES(NTILES=4 PTILES=10 90) ASSOCIATIONS=YES
          /OPTIONS VIZSTYLESHEET=\"plain\"[LOCATION=LOCAL]
          /OUTFILE FILE=*.",
    ) else {
        panic!()
    };
    assert_eq!(r.seed, Some(4711.0));
    assert!(r.pdf && r.cdf && r.cdf_descending && !r.cdf_ascending);
    assert!(r.boxplot && r.scatter);
    assert!(r.tornado_corr && r.tornado_variance);
    assert_eq!(r.tornado_delta, Some(0.5));
    assert_eq!(r.descriptives, Some(90.0));
    assert_eq!((r.ntiles, r.ptiles.clone()), (Some(4), vec![10.0, 90.0]));
    assert!(r.associations);
    assert_eq!(r.outfile.as_deref(), Some("*"));
    let Command::Simrun(r) = one("SIMRUN /PLAN FILE='p' /CRITERIA REPRESULTS=FALSE.") else {
        panic!()
    };
    assert_eq!(r.seed, None);
    assert!(err("SIMRUN /CRITERIA SEED=1.").contains("PLAN"));
}

#[test]
fn expression_block_commands() {
    assert_eq!(
        one("BEGIN EXPR /OUTFILE PREPXML='m.json'."),
        Command::BeginExpr("m.json".into())
    );
    assert_eq!(one("END EXPR."), Command::EndExpr);
}
