//! Parser tests of CSPLAN, CSDESCRIPTIVES, CSTABULATE and the complex samples models.

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
    assert!(
        err(&format!("{base} /DESIGN /METHOD TYPE=SIMPLE_WOR."))
            .contains("belongs to sample plans")
    );
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

fn sample(source: &str) -> Csplan {
    let p = plan(source);
    assert_eq!(p.mode, CsplanMode::Sample);
    p
}

#[test]
fn csplan_sample_two_stages_with_defaults_and_analysis_side() {
    let p = sample(
        "CSPLAN SAMPLE /PLAN FILE='mills.json' /PLANVARS PREVIOUSWEIGHT=w0 /PRINT MATRIX
           /DESIGN STAGELABEL='mills' STRATA=river CLUSTER=mill /METHOD TYPE=SIMPLE_WOR
           /SIZE MATRIX=river; 'north' 2; 'south' 3
           /STAGEVARS POPSIZE WEIGHT(mill_w) INCLPROB(p_mill)
           /DESIGN CLUSTER=sack /METHOD TYPE=SIMPLE_SYSTEMATIC /RATE VALUE=0.2 MINSIZE=1 MAXSIZE=4
           /DESIGN /METHOD TYPE=SIMPLE_WR /SIZE VARIABLE=k.",
    );
    assert_eq!(p.weight.as_deref(), Some("SampleWeight_Final_"));
    assert_eq!(p.previous_weight.as_deref(), Some("w0"));
    assert!(!p.print_plan && p.print_matrix);
    let first = p.stages[0].sample.as_ref().unwrap();
    assert_eq!(first.method, CsMethod::SimpleWor);
    assert_eq!(first.vars.inclprob, "p_mill");
    assert_eq!(first.vars.cumweight, "SampleWeightCumulative_1_");
    assert_eq!(first.vars.popsize.as_deref(), Some("PopulationSize_1_"));
    assert_eq!(first.vars.weight.as_deref(), Some("mill_w"));
    assert_eq!(first.vars.index, None);
    // SIMPLE_WOR is analyzed as EQUAL_WOR with the inclusion probability CSSELECT writes
    assert_eq!(p.stages[0].estimator, CsEstimator::EqualWor);
    assert_eq!(
        p.stages[0].inclprob,
        Some(CsRate::Variable("p_mill".into()))
    );
    let second = p.stages[1].sample.as_ref().unwrap();
    assert_eq!(second.rate, Some(CsRate::Value(0.2)));
    assert_eq!((second.minsize, second.maxsize), (Some(1.0), Some(4.0)));
    assert_eq!(p.stages[1].estimator, CsEstimator::Wr);
    assert_eq!(p.stages[1].inclprob, None);
    let third = p.stages[2].sample.as_ref().unwrap();
    assert_eq!(third.size, Some(CsRate::Variable("k".into())));
    assert_eq!(third.vars.index.as_deref(), Some("Index_3_"));
}

#[test]
fn csplan_sample_notes_and_estimation_wr() {
    let p = sample(
        "CSPLAN SAMPLE /PLAN FILE='a' /PLANVARS ANALYSISWEIGHT=w SAMPLEWEIGHT=fw
           /DESIGN /METHOD TYPE=SIMPLE_WOR ESTIMATION=WR /MOS VARIABLE=m /SIZE VALUE=4
           /STAGEVARS INDEX.",
    );
    assert_eq!(p.weight.as_deref(), Some("fw"));
    assert_eq!(p.stages[0].estimator, CsEstimator::Wr);
    assert_eq!(p.stages[0].sample.as_ref().unwrap().vars.index, None);
    assert_eq!(p.ignored.len(), 3, "{:?}", p.ignored);
}

#[test]
fn csplan_sample_pps_methods_mos_and_their_estimators() {
    let p = sample(
        "CSPLAN SAMPLE /PLAN FILE='a' /DESIGN STRATA=r CLUSTER=c /METHOD TYPE=PPS_WOR
           /SIZE VALUE=3 /MOS VARIABLE=acres MIN=2 MAX=40 /DESIGN /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=2.",
    );
    let first = p.stages[0].sample.as_ref().unwrap();
    assert_eq!(first.method, CsMethod::PpsWor);
    assert_eq!(
        first.mos,
        Some(CsMos {
            variable: Some("acres".into()),
            min: Some(2.0),
            max: Some(40.0),
        })
    );
    assert_eq!(p.stages[0].estimator, CsEstimator::UnequalWor);
    assert_eq!(p.stages[0].inclprob, None);
    let p = sample(
        "CSPLAN SAMPLE /PLAN FILE='a' /DESIGN CLUSTER=c /METHOD TYPE=PPS_MURTHY
           /RATE VALUE=0.5 /MOS SOURCE=FROMDATA.",
    );
    let first = p.stages[0].sample.as_ref().unwrap();
    assert_eq!((first.size.as_ref(), first.rate.as_ref()), (None, None));
    assert_eq!(first.mos.as_ref().unwrap().variable, None);
    assert!(p.ignored[0].contains("two units"), "{:?}", p.ignored);
    for (method, estimator) in [
        ("PPS_WR", CsEstimator::Wr),
        ("PPS_SYSTEMATIC", CsEstimator::Wr),
        ("PPS_CHROMY", CsEstimator::Wr),
        ("PPS_SAMPFORD", CsEstimator::UnequalWor),
        ("PPS_BREWER", CsEstimator::UnequalWor),
    ] {
        let p = sample(&format!(
            "CSPLAN SAMPLE /PLAN FILE='a' /DESIGN CLUSTER=c /METHOD TYPE={method} /SIZE VALUE=2 /MOS VARIABLE=m."
        ));
        assert_eq!(p.stages[0].estimator, estimator, "{method}");
    }
    let p = sample(
        "CSPLAN SAMPLE /PLAN FILE='a' /DESIGN CLUSTER=c /METHOD TYPE=PPS_WOR ESTIMATION=WR
           /SIZE VALUE=2 /MOS VARIABLE=m.",
    );
    assert_eq!(p.stages[0].estimator, CsEstimator::Wr);
    let base = "CSPLAN SAMPLE /PLAN FILE='a'";
    for (rest, expected) in [
        (
            "/DESIGN CLUSTER=c /METHOD TYPE=PPS_WOR /SIZE VALUE=2",
            "needs MOS",
        ),
        (
            "/DESIGN /METHOD TYPE=PPS_SAMPFORD /SIZE VALUE=2 /MOS VARIABLE=m",
            "needs CLUSTER",
        ),
        (
            "/DESIGN CLUSTER=c /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=2 /DESIGN /METHOD TYPE=PPS_WR /SIZE VALUE=1",
            "only in the first stage",
        ),
        (
            "/DESIGN CLUSTER=c /METHOD TYPE=PPS_WOR /SIZE VALUE=2 /MOS MIN=3",
            "one of VARIABLE",
        ),
        (
            "/DESIGN CLUSTER=c /METHOD TYPE=PPS_WOR /SIZE VALUE=2 /MOS VARIABLE=m MIN=5 MAX=2",
            "above its MAX",
        ),
        (
            "/DESIGN /METHOD TYPE=PPS_WR /SIZE VALUE=2 /MOS SOURCE=FROMDATA",
            "needs CLUSTER",
        ),
        (
            "/DESIGN CLUSTER=c /METHOD TYPE=PPS_WOR /SIZE VALUE=2 /MOS VARIABLE=c",
            "more than one role",
        ),
    ] {
        let message = err(&format!("{base} {rest}."));
        assert!(message.contains(expected), "{rest}: {message}");
    }
}

#[test]
fn csplan_sample_rules_give_errors() {
    let base = "CSPLAN SAMPLE /PLAN FILE='a'";
    let cases = [
        ("/DESIGN /METHOD TYPE=PPS_WOR /SIZE VALUE=2", "PPS_WOR"),
        ("/DESIGN /SIZE VALUE=2", "needs a METHOD"),
        ("/DESIGN /METHOD TYPE=SIMPLE_WOR", "needs SIZE or RATE"),
        (
            "/DESIGN /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=2 /RATE VALUE=0.1",
            "cannot both",
        ),
        (
            "/DESIGN /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=2.5",
            "whole number",
        ),
        (
            "/DESIGN /METHOD TYPE=SIMPLE_WOR /RATE VALUE=1.5",
            "at most 1",
        ),
        (
            "/DESIGN /METHOD TYPE=SIMPLE_WOR /RATE VALUE=0.5 MINSIZE=5 MAXSIZE=2",
            "MINSIZE is above",
        ),
        (
            "/DESIGN CLUSTER=c /METHOD TYPE=SIMPLE_WR /SIZE VALUE=2 /DESIGN /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=1",
            "only the last stage",
        ),
        (
            "/DESIGN CLUSTER=c /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=2 /DESIGN /METHOD TYPE=SIMPLE_WOR ESTIMATION=WR /SIZE VALUE=1",
            "first stage",
        ),
        (
            "/DESIGN /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=2 /DESIGN /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=1",
            "needs CLUSTER",
        ),
        (
            "/DESIGN /METHOD TYPE=SIMPLE_WOR /ESTIMATOR TYPE=WR",
            "analysis plans",
        ),
        (
            "/SRSESTIMATOR TYPE=WR /DESIGN /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=1",
            "analysis plans",
        ),
        (
            "/DESIGN STRATA=r /METHOD TYPE=SIMPLE_WOR /SIZE MATRIX=q; 1 2",
            "not a stratum variable",
        ),
        (
            "/DESIGN STRATA=r /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=1 /STAGEVARS WEIGHT(r)",
            "more than one role",
        ),
        (
            "/DESIGN /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=1 /STAGEVARS RATE RATE",
            "only once",
        ),
        (
            "/DESIGN /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=1 /METHOD TYPE=SIMPLE_WR",
            "only once",
        ),
        ("/DESIGN /METHOD TYPE=SIMPLE /SIZE VALUE=1", "unexpected"),
    ];
    for (rest, expected) in cases {
        let message = err(&format!("{base} {rest}."));
        assert!(message.contains(expected), "{rest}: {message}");
    }
    assert!(err("CSPLAN SAMPLE /PLAN FILE='a'.").contains("needs a DESIGN"));
}

fn select(source: &str) -> Csselect {
    match one(source) {
        Command::Csselect(c) => *c,
        other => panic!("{other:?}"),
    }
}

#[test]
fn csselect_parses_every_subcommand() {
    let c = select(
        "CSSELECT /PLAN FILE='mills.json' /CRITERIA STAGES=2 3 SEED=4242 /CLASSMISSING INCLUDE
           /DATA RENAMEVARS PRESORTED /SAMPLEFILE OUTFILE='drawn.sav' DROP=notes KEEP=ALL
           /JOINTPROB OUTFILE='joint.sav' /PRINT CPS.",
    );
    assert_eq!(c.plan, "mills.json");
    assert_eq!(c.stages, [2, 3]);
    assert_eq!(c.seed, Some(4242));
    assert!(c.include && c.renamevars);
    assert_eq!(c.samplefile.as_deref(), Some("drawn.sav"));
    assert_eq!(c.subsets.len(), 2);
    assert!(!c.subsets[0].0 && c.subsets[1].0);
    assert_eq!(c.jointprob.as_deref(), Some("joint.sav"));
    assert!(!c.print_selection && c.print_cps);
    let d = select("CSSELECT /PLAN FILE='p' /CRITERIA SEED=RANDOM.");
    assert!(d.seed.is_none() && d.stages.is_empty() && d.print_selection);
}

#[test]
fn csselect_rules_give_errors() {
    let cases = [
        ("CSSELECT /CRITERIA SEED=3.", "needs PLAN"),
        (
            "CSSELECT /PLAN FILE='p' /CRITERIA STAGES=1 3.",
            "consecutive",
        ),
        (
            "CSSELECT /PLAN FILE='p' /CRITERIA SEED=0.",
            "positive whole number",
        ),
        (
            "CSSELECT /PLAN FILE='p' /SELECTRULE OUTFILE='r.txt'.",
            "not supported yet",
        ),
        ("CSSELECT /PLAN FILE='p' /PRINT.", "needs SELECTION or CPS"),
        (
            "CSSELECT /PLAN FILE='p' /SAMPLEFILE KEEP=a.",
            "needs OUTFILE",
        ),
        (
            "CSSELECT /PLAN FILE='p' /SAMPLEFILE OUTFILE='x' /JOINTPROB OUTFILE='x'.",
            "same file",
        ),
        ("CSSELECT /PLAN FILE='p' /PLAN FILE='q'.", "only once"),
    ];
    for (source, expected) in cases {
        let message = err(source);
        assert!(message.contains(expected), "{source}: {message}");
    }
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
        err("CSDESCRIPTIVES /PLAN FILE='p' /JOINTPROB FILE='j' /JOINTPROB FILE='k' /SUM.")
            .contains("only once")
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
    let m = model(
        "CSLOGISTIC owner (3) /PLAN FILE='p.json' /SAVE PREDPROB(:4) /TEST TYPE=ADJC PADJUST=SEQS.",
    );
    assert_eq!(m.test, CsTestType::AdjChiSquare);
    assert_eq!(m.padjust, CsPadjust::SeqSidak);
    assert_eq!(m.reference, CsReference::Value(Value::Number(3.0)));
    assert_eq!(m.save, vec![CsModelSave::PredProb(None, 4)]);
    // a PREDPROB root may equal the PREDVAL name: its variables get suffixes
    let m = model("CSLOGISTIC owner /PLAN FILE='p.json' /SAVE PREDPROB(p) PREDVAL(p).");
    assert_eq!(m.save.len(), 2);
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
            "CSGLM y BY a /PLAN FILE='p' /OUTFILE COVB='c.sav' CORB='d.sav'.",
            "one of COVB and CORB",
        ),
        (
            "CSGLM y BY a /PLAN FILE='p' /OUTFILE MODEL='m' PARAMETER='q'.",
            "one of MODEL and PARAMETER",
        ),
        ("CSGLM y BY a /PLAN FILE='p' /OUTFILE.", "unexpected end"),
        (
            "CSGLM y BY a /PLAN FILE='p' /OUTFILE COVB 'c'.",
            "unexpected word COVB",
        ),
        (
            "CSGLM y BY a /PLAN FILE='p' /OUTFILE COVB='c' /OUTFILE MODEL='m'.",
            "only once",
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
        (
            "CSGLM y /PLAN FILE='p' /TEST TYPE=ADJ.",
            "more than one keyword",
        ),
        (
            "CSGLM y /PLAN FILE='p' /TEST PADJUST=SEQ.",
            "more than one keyword",
        ),
        (
            "CSGLM y /PLAN FILE='p' /STATISTICS DEF.",
            "more than one keyword",
        ),
        (
            "CSLOGISTIC y /PLAN FILE='p' /SAVE PRED.",
            "more than one keyword",
        ),
    ];
    for (source, expected) in cases {
        let message = err(source);
        assert!(message.contains(expected), "{source}: {message}");
    }
}

#[test]
fn csordinal_subcommands_and_defaults() {
    let m = model(
        "CSORDINAL grade (DESCENDING) BY school WITH hours /PLAN FILE='p.json' /MODEL school hours
           /LINK FUNCTION=CLOGLOG /CRITERIA METHOD=FISHER(4) MXITER=50 CHKSEP=0
           /STATISTICS PARAMETER EXP SE /TEST TYPE=ADJF /DOMAIN VARIABLE=area(2)
           /MISSING CLASSMISSING=INCLUDE /PRINT HISTORY(2) CLASSTABLE.",
    );
    assert_eq!(m.name(), "CSORDINAL");
    assert!(m.logistic && !m.show_intercept && m.include);
    assert_eq!(
        m.ordinal,
        Some(CsOrdinal {
            descending: true,
            link: PlumLink::Cloglog,
            nonparallel: None,
        })
    );
    assert_eq!(m.criteria.fisher, 4);
    assert_eq!(m.criteria.mxiter, 50);
    assert_eq!(m.model.as_ref().map(Vec::len), Some(2));
    assert!(m.statistics.is_some_and(|s| s.exp && s.se && !s.ttest));
    assert_eq!(m.print.history, Some(2));
    assert!(m.print.classtable && !m.print.summary);
    // the defaults: ascending, logit, Newton-Raphson
    let m = model("CSORDINAL grade /PLAN FILE='p.json' /CRITERIA METHOD=FISHER.");
    assert_eq!(
        m.ordinal,
        Some(CsOrdinal {
            descending: false,
            link: PlumLink::Logit,
            nonparallel: None,
        })
    );
    assert_eq!(m.criteria.fisher, u32::MAX);
    assert!(m.print.sampleinfo && m.print.variableinfo && m.print.summary);
    let m = model("CSORDINAL grade (ASCENDING) /PLAN FILE='p.json' /CRITERIA METHOD=NEWTON.");
    assert_eq!(m.criteria.fisher, 0);
}

#[test]
fn csordinal_syntax_errors() {
    let base = "CSORDINAL y BY a WITH x /PLAN FILE='p'";
    let m = model(&format!("{base} /JOINTPROB FILE='j.sav'."));
    assert_eq!(m.jointprob.as_deref(), Some("j.sav"));
    for command in ["CSGLM", "CSLOGISTIC", "CSORDINAL"] {
        let m = model(&format!(
            "{command} y BY a /PLAN FILE='p' /OUTFILE MODEL='m.json' CORB=est."
        ));
        assert_eq!(
            m.outfile,
            CsOutfile {
                matrix: Some(("est".into(), true)),
                model: Some(("m.json".into(), true)),
            }
        );
        let m = model(&format!(
            "{command} y BY a /PLAN FILE='p' /OUTFILE COVB='c.sav' PARAMETER='e.json'."
        ));
        assert_eq!(
            m.outfile,
            CsOutfile {
                matrix: Some(("c.sav".into(), false)),
                model: Some(("e.json".into(), false)),
            }
        );
    }
    let cases = [
        ("/INTERCEPT INCLUDE=NO", "unknown subcommand INTERCEPT"),
        ("/LINK FUNCTION=LOGLOG", "unexpected"),
        ("/LINK LOGIT", "unexpected word LOGIT"),
        ("/LINK FUNCTION=PROBIT /LINK FUNCTION=LOGIT", "only once"),
        ("/CRITERIA METHOD=FISHER(-1)", "whole number"),
        ("/PRINT GEF", "not supported yet"),
        ("/CUSTOM KMATRIX=1", "CSORDINAL CUSTOM needs LMATRIX="),
        ("/NONPARALLEL", "needs TEST, PARAMETER or COVB"),
        ("/NONPARALLEL TEST TEST", "only once"),
        ("/NONPARALLEL TEST /NONPARALLEL COVB", "only once"),
        ("/SAVE PRED", "short for more than one keyword"),
        ("/SAVE RESID", "unexpected"),
        ("/SAVE PREDVAL(v) OBSVALPROB(v)", "SAVE names V twice"),
    ];
    for (rest, expected) in cases {
        let message = err(&format!("{base} {rest}."));
        assert!(message.contains(expected), "{rest}: {message}");
    }
    let message = err("CSORDINAL y (LOW) /PLAN FILE='p'.");
    assert!(message.contains("unexpected word LOW"), "{message}");
    assert!(err("CSLOGISTIC y /PLAN FILE='p' /CRITERIA METHOD=NEWTON.").contains("unexpected"));
    assert!(err("CSLOGISTIC y /PLAN FILE='p' /LINK FUNCTION=LOGIT.").contains("unknown"));
}

#[test]
fn csordinal_custom_oddsratios_nonparallel_and_save() {
    let m = model(
        "CSORDINAL y BY a WITH x /PLAN FILE='p'
           /CUSTOM LABEL='t' LMATRIX=1 0 a 0 -1; a 1 -1; ALL 0 1/2 0 0 1 KMATRIX=0; 1; 2
           /ODDSRATIOS FACTOR=[a(LOW)] CONTROL=[x(2)] /ODDSRATIOS COVARIATE=[x(1 5)]
           /NONPARALLEL PARAMETER, COVB
           /SAVE CUMPROB(cp:2) PREDPROB PREDVAL(pv) PREDVALPROB OBSVALPROB(ov).",
    );
    let custom = &m.custom[0];
    assert_eq!(custom.rows.len(), 3);
    // the leading numbers of a row are the threshold coefficients
    let (lead, values) = &custom.rows[0][0];
    assert!(lead.as_ref().is_some_and(|t| t.variables.is_empty()));
    assert_eq!(values, &vec![1.0, 0.0]);
    assert_eq!(custom.rows[2][0].1, vec![0.0, 0.5, 0.0, 0.0, 1.0]);
    assert_eq!(custom.k, Some(vec![0.0, 1.0, 2.0]));
    assert_eq!(m.oddsratios.len(), 2);
    assert_eq!(
        m.ordinal.and_then(|o| o.nonparallel),
        Some(CsNonparallel {
            test: false,
            parameter: true,
            covb: true,
        })
    );
    assert_eq!(
        m.save,
        vec![
            CsModelSave::CumProb(Some("cp".into()), 2),
            CsModelSave::PredProb(None, 25),
            CsModelSave::PredVal(Some("pv".into())),
            CsModelSave::PredValProb(None),
            CsModelSave::ObsValProb(Some("ov".into())),
        ]
    );
    // CSLOGISTIC keeps one intercept coefficient and its SAVE keywords
    let message = err("CSLOGISTIC y BY a /PLAN FILE='p' /CUSTOM LMATRIX=1 0 a 1 -1.");
    assert!(
        message.contains("one coefficient for the intercept"),
        "{message}"
    );
    assert!(err("CSLOGISTIC y /PLAN FILE='p' /SAVE CUMPROB.").contains("unexpected"));
    assert!(err("CSLOGISTIC y /PLAN FILE='p' /NONPARALLEL TEST.").contains("unknown"));
}
