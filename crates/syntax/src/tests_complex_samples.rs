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
           /SUM /RATIO DENOM=acres NUMERATOR=yield TTE=1.5 /RATIO NUMERATOR=ALL DENOMINATOR=c
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
    assert_eq!(c.ratios[1].numerators, [VarItem::All]);
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

fn model(source: &str) -> CsModel {
    match one(source) {
        Command::CsModel(m) => *m,
        other => panic!("{other:?}"),
    }
}

#[test]
fn csglm_every_subcommand() {
    let m = model(
        "CSGLM yield BY soil crop WITH rain /PLAN FILE='p.json' /MODEL soil crop soil*crop rain
           /INTERCEPT INCLUDE=YES SHOW=NO
           /CUSTOM LABEL='soils' LMATRIX=soil 1 -1 0; 1 soil 1/3 1/3 1/3 rain 2 KMATRIX=0; 10
           /CUSTOM LMATRIX=ALL 0 1 -1 0
           /EMMEANS TABLES=soil*crop OTHER=[rain(MEAN)] COMPARE=soil CONTRAST=SIMPLE(2)
           /EMMEANS
           /CRITERIA CILEVEL=90 DF=12 SINGULAR=1E-10 /STATISTICS PARAMETER SE DEFF DEFFSQRT
           /TEST TYPE=ADJF PADJUST=SEQSIDAK /DOMAIN VARIABLE=region('north')
           /MISSING CLASSMISSING=INCLUDE /PRINT SUMMARY COVB LMATRIX /SAVE PRED(p) RESID.",
    );
    assert!(!m.logistic);
    assert_eq!(m.dependent, "yield");
    assert_eq!(m.plan, "p.json");
    assert_eq!(m.model.as_ref().unwrap().len(), 4);
    assert!(!m.show_intercept);
    assert_eq!(m.custom.len(), 2);
    let c = &m.custom[0];
    assert_eq!(c.label.as_deref(), Some("soils"));
    assert_eq!(c.rows.len(), 2);
    // the leading 1 is the intercept's coefficient
    assert!(c.rows[1][0].0.as_ref().unwrap().variables.is_empty());
    assert_eq!(c.rows[1][1].1, vec![1.0 / 3.0; 3]);
    assert_eq!(c.k, Some(vec![0.0, 10.0]));
    assert!(m.custom[1].rows[0][0].0.is_none());
    assert_eq!(m.emmeans.len(), 2);
    assert_eq!(m.emmeans[0].tables, ["soil", "crop"]);
    assert_eq!(m.emmeans[0].other, vec![("rain".to_string(), None)]);
    assert_eq!(
        m.emmeans[0].contrast,
        CsContrast::Simple(Some(Value::Number(2.0)))
    );
    assert!(m.emmeans[1].tables.is_empty());
    assert_eq!(m.criteria.cilevel, 90.0);
    assert_eq!(m.criteria.df, Some(12.0));
    let s = m.statistics.unwrap();
    assert!(s.parameter && s.se && s.deff && s.deffsqrt && !s.ttest);
    assert_eq!(m.test, CsTestType::AdjF);
    assert_eq!(m.padjust, CsPadjust::SeqSidak);
    assert_eq!(
        m.domain,
        Some(("region".to_string(), Value::String("north".into())))
    );
    assert!(m.include);
    assert!(m.print.summary && m.print.covb && m.print.lmatrix && !m.print.sampleinfo);
    assert_eq!(
        m.save,
        vec![
            CsModelSave::Pred(Some("p".into())),
            CsModelSave::Resid(None)
        ]
    );
}

#[test]
fn cslogistic_reference_odds_ratios_criteria_and_save() {
    let m = model(
        "CSLOGISTIC owner (LOW) BY soil WITH rain /PLAN FILE='p.json'
           /ODDSRATIOS FACTOR=[soil('clay')] CONTROL=[rain(10)]
           /ODDSRATIOS COVARIATE=[rain(1 5)]
           /CRITERIA CHKSEP=0 LCONVERGE=[1E-8 ABSOLUTE] MXITER=50 MXSTEP=3 PCONVERGE=[0]
           /STATISTICS EXP CINTERVAL /PRINT HISTORY(5) CLASSTABLE /SAVE PREDPROB(pr:2) PREDVAL(v).",
    );
    assert!(m.logistic);
    assert_eq!(m.reference, CsReference::Low);
    assert_eq!(
        m.oddsratios[0].factors,
        vec![(
            "soil".to_string(),
            CsReference::Value(Value::String("clay".into()))
        )]
    );
    assert_eq!(
        m.oddsratios[0].control,
        vec![("rain".to_string(), CsControl::Value(Value::Number(10.0)))]
    );
    assert_eq!(
        m.oddsratios[1].covariates,
        vec![("rain".to_string(), vec![1.0, 5.0])]
    );
    assert_eq!(m.criteria.chksep, 0);
    assert_eq!(m.criteria.lconverge, (1e-8, false));
    assert_eq!(m.criteria.pconverge, (0.0, true));
    assert_eq!(m.criteria.mxiter, 50);
    assert_eq!(m.criteria.mxstep, 3);
    assert_eq!(m.print.history, Some(5));
    assert!(m.print.classtable && !m.print.summary);
    assert_eq!(
        m.save,
        vec![
            CsModelSave::PredProb(Some("pr".into()), 2),
            CsModelSave::PredVal(Some("v".into()))
        ]
    );
    let m = model("CSLOGISTIC owner (3) /PLAN FILE='p.json' /SAVE PREDPROB(:4).");
    assert_eq!(m.reference, CsReference::Value(Value::Number(3.0)));
    assert_eq!(m.save, vec![CsModelSave::PredProb(None, 4)]);
}

#[test]
fn model_syntax_errors() {
    let cases = [
        ("CSGLM y BY a.", "needs PLAN FILE"),
        ("CSGLM y x /PLAN FILE='p'.", "one dependent variable"),
        (
            "CSGLM y BY a /PLAN FILE='p' /MODEL a /INTERCEPT INCLUDE=ONLY.",
            "cannot go with MODEL",
        ),
        (
            "CSGLM y /PLAN FILE='p' /INTERCEPT INCLUDE=NO.",
            "needs factors",
        ),
        ("CSGLM y BY a /PLAN FILE='p' /CUSTOM LABEL='x'.", "LMATRIX"),
        (
            "CSGLM y BY a /PLAN FILE='p' /CUSTOM LMATRIX=a 1 -1; a 1 0 KMATRIX=0.",
            "1 values for 2 rows",
        ),
        (
            "CSGLM y BY a /PLAN FILE='p' /PRINT GEF.",
            "not supported yet",
        ),
        (
            "CSGLM y BY a /PLAN FILE='p' /OUTFILE COVB='c.sav'.",
            "not supported yet",
        ),
        (
            "CSGLM y BY a /PLAN FILE='p' /JOINTPROB FILE='j'.",
            "not supported yet",
        ),
        (
            "CSGLM y BY a /PLAN FILE='p' /ODDSRATIOS FACTOR=[a].",
            "unknown subcommand",
        ),
        (
            "CSLOGISTIC y BY a /PLAN FILE='p' /EMMEANS.",
            "unknown subcommand",
        ),
        (
            "CSGLM y BY a /PLAN FILE='p' /CRITERIA CILEVEL=100.",
            "below 100",
        ),
        ("CSGLM y BY a /PLAN FILE='p' /PLAN FILE='q'.", "only once"),
        (
            "CSGLM y BY a /PLAN FILE='p' /EMMEANS COMPARE=a.",
            "needs TABLES",
        ),
        (
            "CSLOGISTIC y BY a WITH x /PLAN FILE='p' /ODDSRATIOS FACTOR=[a] COVARIATE=[x].",
            "not both",
        ),
        (
            "CSLOGISTIC y /PLAN FILE='p' /CRITERIA MXSTEP=0.",
            "positive whole number",
        ),
        (
            "CSGLM y /PLAN FILE='p' /CRITERIA MXITER=5.",
            "unexpected word MXITER",
        ),
        ("CSGLM y /PLAN FILE='p' /SAVE PRED(a) RESID(A).", "twice"),
        ("CSGLM y /PLAN FILE='p' /STATISTICS SE SE.", "only once"),
        (
            "CSGLM y BY a /PLAN FILE='p' /CUSTOM LMATRIX=1 2 a 1 -1.",
            "one coefficient for the intercept",
        ),
    ];
    for (source, expected) in cases {
        let message = err(source);
        assert!(message.contains(expected), "{source}: {message}");
    }
}
