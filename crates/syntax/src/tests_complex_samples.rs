//! Parser tests of CSPLAN, CSDESCRIPTIVES and CSTABULATE.

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

fn plan(source: &str) -> Csplan {
    match one(source) {
        Command::Csplan(p) => *p,
        other => panic!("{other:?}"),
    }
}

#[test]
fn csplan_two_stage_plan_with_matrix_and_variable() {
    let p = plan(
        "CSPLAN ANALYSIS /PLAN FILE='crops.json' /PLANVARS ANALYSISWEIGHT=wt /PRINT MATRIX
           /DESIGN STAGELABEL='parishes' STRATA=county CLUSTER=parish /ESTIMATOR TYPE=EQUAL_WOR
           /INCLPROB MATRIX=county; 1 0.2; 2 0.25
           /DESIGN CLUSTER=farm /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VARIABLE=farms_total.",
    );
    assert_eq!(p.mode, CsplanMode::Analysis);
    assert_eq!(p.file, "crops.json");
    assert_eq!(p.weight.as_deref(), Some("wt"));
    assert!(!p.print_plan && p.print_matrix && !p.srs_wr);
    assert_eq!(p.stages.len(), 2);
    assert_eq!(p.stages[0].label.as_deref(), Some("parishes"));
    assert_eq!(p.stages[0].strata, ["county"]);
    assert_eq!(p.stages[0].clusters, ["parish"]);
    assert_eq!(
        p.stages[0].inclprob,
        Some(CsRate::Matrix {
            variables: vec!["county".into()],
            rows: vec![
                (vec![Value::Number(1.0)], 0.2),
                (vec![Value::Number(2.0)], 0.25)
            ],
        })
    );
    assert_eq!(p.stages[1].estimator, CsEstimator::EqualWor);
    assert_eq!(
        p.stages[1].popsize,
        Some(CsRate::Variable("farms_total".into()))
    );
}

#[test]
fn csplan_wr_ignores_popsize_with_a_note_and_view_takes_only_plan() {
    let p = plan(
        "CSPLAN ANALYSIS /PLAN FILE='a' /PLANVARS ANALYSISWEIGHT=wt /SRSESTIMATOR TYPE=WR
           /DESIGN /ESTIMATOR TYPE=WR /POPSIZE VALUE=500.",
    );
    assert!(p.srs_wr);
    assert_eq!(p.stages[0].popsize, None);
    assert_eq!(p.ignored.len(), 1);
    let v = plan("CSPLAN VIEW /PLAN FILE='a'.");
    assert_eq!(v.mode, CsplanMode::View);
    assert!(err("CSPLAN VIEW /PLAN FILE='a' /DESIGN.").contains("only PLAN and PRINT"));
}

#[test]
fn csplan_rules_give_errors() {
    let base = "CSPLAN ANALYSIS /PLAN FILE='a' /PLANVARS ANALYSISWEIGHT=wt";
    assert!(
        err(&format!("{base} /DESIGN /ESTIMATOR TYPE=EQUAL_WOR.")).contains("POPSIZE or INCLPROB")
    );
    assert!(err(&format!("{base} /DESIGN STRATA=r.")).contains("needs an ESTIMATOR"));
    assert!(
        err(&format!(
            "{base} /DESIGN STRATA=r /ESTIMATOR TYPE=WR /DESIGN /ESTIMATOR TYPE=WR."
        ))
        .contains("needs CLUSTER")
    );
    assert!(
        err(&format!(
            "{base} /DESIGN /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VALUE=10 /INCLPROB VALUE=0.1."
        ))
        .contains("cannot both")
    );
    assert!(
        err(&format!(
            "{base} /DESIGN /ESTIMATOR TYPE=EQUAL_WOR /INCLPROB VALUE=1.5."
        ))
        .contains("at most 1")
    );
    assert!(
        err(&format!(
            "{base} /DESIGN /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VALUE=2.5."
        ))
        .contains("whole number")
    );
    assert!(
        err(&format!("{base} /DESIGN STRATA=wt /ESTIMATOR TYPE=WR."))
            .contains("more than one role")
    );
    assert!(
        err(&format!(
            "{base} /DESIGN STRATA=r /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE MATRIX=q; 1 10."
        ))
        .contains("not a stratum variable")
    );
    assert!(err(&format!("{base} /DESIGN /ESTIMATOR TYPE=UNEQUAL_WOR.")).contains("needs CLUSTER"));
    assert!(err(&format!("{base} /DESIGN /METHOD TYPE=SIMPLE_WOR.")).contains("not supported yet"));
    assert!(err("CSPLAN SAMPLE /PLAN FILE='a'.").contains("not supported yet"));
    assert!(
        err(&format!(
            "{base} /DESIGN CLUSTER=v /ESTIMATOR TYPE=EQUAL_WOR /POPSIZE VARIABLE=psz
               /DESIGN /ESTIMATOR TYPE=EQUAL_WOR /INCLPROB MATRIX=psz; 100 0.2."
        ))
        .contains("not a stratum variable")
    );
    assert!(
        err("CSPLAN ANALYSIS /PLAN FILE='a' /DESIGN /ESTIMATOR TYPE=WR.")
            .contains("ANALYSISWEIGHT")
    );
    assert!(err(&format!("{base} /ESTIMATOR TYPE=WR.")).contains("give DESIGN first"));
    assert!(err("CSPLAN ANALYSIS /PLAN FILE 'a'.").contains("unexpected string"));
}

#[test]
fn csdescriptives_parses_every_subcommand() {
    let c = match one(
        "CSDESCRIPTIVES /PLAN FILE='p.json' /SUMMARY VARIABLES=yield acres /MEAN TTEST=3, 4
           /SUM /RATIO DENOMINATOR=acres NUMERATOR=yield TTEST=1.5 /RATIO NUMERATOR=a b DENOMINATOR=c
           /STATISTICS COUNT POPSIZE CV DEFFSQRT CIN(90) /SUBPOP TABLE=organic BY county DISPLAY=SEPARATE
           /MISSING SCOPE=LISTWISE CLASSMISSING=INCLUDE.",
    ) {
        Command::Csdescriptives(c) => *c,
        other => panic!("{other:?}"),
    };
    assert_eq!(c.plan, "p.json");
    assert_eq!(c.summary.len(), 2);
    assert_eq!(c.mean, Some(vec![3.0, 4.0]));
    assert_eq!(c.sum, Some(vec![]));
    assert_eq!(c.ratios.len(), 2);
    assert_eq!(c.ratios[0].numerators, [VarItem::Name("yield".into())]);
    assert_eq!(c.ratios[0].test, Some(vec![1.5]));
    let s = c.statistics;
    assert!(s.count && s.popsize && s.cv && s.deffsqrt && !s.se && !s.deff);
    assert_eq!(s.cin, Some(90.0));
    let sub = c.subpop.unwrap();
    assert_eq!(sub.variables, ["organic", "county"]);
    assert!(sub.separate && c.listwise && c.include);
}

#[test]
fn csdescriptives_rules_give_errors() {
    assert!(
        err("CSDESCRIPTIVES /PLAN FILE='p' /SUMMARY VARIABLES=x.").contains("MEAN, SUM or RATIO")
    );
    assert!(err("CSDESCRIPTIVES /PLAN FILE='p' /MEAN.").contains("SUMMARY"));
    assert!(err("CSDESCRIPTIVES /SUMMARY VARIABLES=x /MEAN.").contains("PLAN FILE"));
    assert!(
        err("CSDESCRIPTIVES /PLAN FILE='p' /SUMMARY VARIABLES=x /MEAN /MEAN.")
            .contains("only once")
    );
    assert!(
        err("CSDESCRIPTIVES /PLAN FILE='p' /SUMMARY VARIABLES=x /MEAN /STATISTICS CIN(100).")
            .contains("below 100")
    );
    assert!(err("CSDESCRIPTIVES /PLAN FILE='p' /RATIO NUMERATOR=x.").contains("DENOMINATOR"));
    assert!(
        err("CSDESCRIPTIVES /PLAN FILE='p' /RATIO NUMERATOR=x DENOMINATOR=y /STATISTICS DEF.")
            .contains("unexpected")
    );
    assert!(
        err("CSDESCRIPTIVES /PLAN FILE='p' /JOINTPROB FILE='j' /RATIO NUMERATOR=x DENOMINATOR=y.")
            .contains("not supported yet")
    );
    assert!(
        err("CSDESCRIPTIVES /PLAN FILE='p' /SUMMARY VARIABLES=x /MEAN /SUBPOP TABLE=a BY a.")
            .contains("twice")
    );
}

#[test]
fn cstabulate_defaults_and_keywords() {
    let c = match one("CSTABULATE /PLAN FILE='p' /TABLES VARIABLES=a b.") {
        Command::Cstabulate(c) => *c,
        other => panic!("{other:?}"),
    };
    assert!(c.cells.popsize && !c.cells.rowpct && c.statistics.se && c.by.is_none());
    let c = match one(
        "CSTABULATE /PLAN FILE='p' /TABLES VARIABLES=a b BY c /CELLS ROWPCT TABLEPCT
           /STATISTICS CIN COUNT DEFF EXPECTED RESID ASRESID /TEST INDEPENDENCE ODDSRATIO RELRISK RISKDIFF
           /MISSING SCOPE=LISTWISE.",
    ) {
        Command::Cstabulate(c) => *c,
        other => panic!("{other:?}"),
    };
    assert_eq!(c.variables.len(), 2);
    assert_eq!(c.by.as_deref(), Some("c"));
    assert!(!c.cells.popsize && c.cells.rowpct && c.cells.tablepct);
    let s = c.statistics;
    assert!(
        !s.se
            && s.deff
            && !s.deffsqrt
            && s.count
            && s.expected
            && s.resid
            && s.asresid
            && s.cin == Some(95.0)
    );
    assert!(c.tests.independence && c.tests.oddsratio && c.tests.relrisk && c.tests.riskdiff);
    assert!(c.listwise);
    assert!(err("CSTABULATE /PLAN FILE='p' /TABLES VARIABLES=a /CELLS.").contains("at least one"));
    assert!(err("CSTABULATE /PLAN FILE='p'.").contains("TABLES"));
}
