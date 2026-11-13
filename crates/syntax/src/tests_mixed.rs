//! Parser tests of MIXED.

use super::*;

fn mixed(source: &str) -> Mixed {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    match parsed.remove(0).command {
        Command::Mixed(m) => *m,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn defaults_with_only_the_variable_list() {
    let m = mixed("MIXED gain.");
    assert_eq!(m.dependent, "gain");
    assert!(m.fixed.is_empty() && m.intercept && m.random.is_empty());
    assert_eq!(m.sstype, 3);
    assert!(!m.ml && !m.include && !m.residual_df);
    assert_eq!((m.cin, m.mxiter, m.mxstep, m.scoring), (95.0, 100, 10, 1));
    assert_eq!(m.singular, 1e-12);
    assert_eq!(m.pconverge.value, 1e-6);
    assert_eq!((m.lconverge.value, m.hconverge.value), (0.0, 0.0));
    assert!(m.repeated.is_none() && m.save.is_empty());
}

#[test]
fn fixed_random_and_repeated_with_abbreviations() {
    let m = mixed(
        "MIX gain BY plot crop WITH rain\n\
         /FIX=crop rain crop*rain | NOI SST(3)\n\
         /RAND=INTERCEPT rain | SUBJ(farm*plot) COV(UN)\n\
         /RANDOM=crop WITHIN plot\n\
         /REP=week | SUBJECT(farm) COVTYPE(AR1)\n\
         /METH=ML /MISS=INCLUDE /CRIT=CIN(90) MXIT(30) MXST(5) SCOR(3) SING(1E-10) PCON(1E-8, REL)\n\
         LCON(1E-9) HCON(0.001 ABSOLUTE) DFM(RESIDUAL)\n\
         /PRI=SOL TESTC /REGW=w /SAVE=FIXPRED(fx) PRED RESID SEFIXP DFFIXP(dfx).",
    );
    assert_eq!(
        m.factors,
        [VarItem::Name("plot".into()), VarItem::Name("crop".into())]
    );
    let labels: Vec<&str> = m.fixed.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(labels, ["crop", "rain", "crop*rain"]);
    assert!(!m.intercept);
    assert_eq!(m.random.len(), 2);
    assert_eq!(m.random[0].terms[0].label, "Intercept");
    assert_eq!(m.random[0].subject, ["farm", "plot"]);
    assert_eq!(m.random[0].covtype, CovType::Un);
    assert_eq!(m.random[1].terms[0].label, "crop(plot)");
    assert_eq!(m.random[1].covtype, CovType::Vc);
    let r = m.repeated.unwrap();
    assert_eq!(
        (r.variables, r.subject, r.covtype),
        (
            vec!["week".to_string()],
            vec!["farm".to_string()],
            CovType::Ar1
        )
    );
    assert!(m.ml && m.include && m.residual_df);
    assert_eq!(
        (m.cin, m.mxiter, m.mxstep, m.scoring, m.singular),
        (90.0, 30, 5, 3, 1e-10)
    );
    assert!(m.pconverge.relative && !m.hconverge.relative);
    assert_eq!((m.lconverge.value, m.hconverge.value), (1e-9, 0.001));
    assert!(m.print.solution && m.print.testcov);
    assert_eq!(m.regwgt.as_deref(), Some("w"));
    assert_eq!(m.save[0], (MixedTemp::Fixpred, Some("fx".into())));
    assert_eq!(m.save[4], (MixedTemp::Dffixp, Some("dfx".into())));
}

#[test]
fn emmeans_test_and_the_matrix_tables() {
    let m = mixed(
        "MIXED gain BY crop soil WITH rain /FIXED=crop soil rain\n\
         /EMM=TABLES(crop*soil) WITH(rain=2.5) COMPARE(soil) REFCAT(LAST) ADJ(SIDAK)\n\
         /EMMEANS=TABLES(crop) COMPARE REFCAT('oat')\n\
         /EMMEANS=TABLES(crop) COMPARE REFCAT('oat')\n\
         /EMMEANS=TABLES(OVERALL)\n\
         /TEST(0.5 2)='soil steps' soil 1 -1 0; soil 0 2 -2 DIVISOR=2\n\
         /TEST=ALL 1 1/3 1/3 1/3 0 0 0\n\
         /PRINT=G R COVB CORB.",
    );
    assert_eq!(m.emmeans.len(), 3, "the identical EMMEANS counts once");
    let once = mixed(
        "MIXED gain BY crop /EMMEANS=TABLES(crop) COMPARE /EMMEANS=TABLES(CROP) COMPARE(Crop).",
    );
    assert_eq!(once.emmeans.len(), 1, "names compare without case");
    let with = |a: &str, b: &str| {
        mixed(&format!("MIXED gain BY crop WITH x z /EMMEANS=TABLES(crop) WITH({a}) /EMMEANS=TABLES(crop) WITH({b})."))
            .emmeans
            .len()
    };
    assert_eq!(
        with("x=1 z=2", "z=2 x=1"),
        1,
        "the order of WITH does not matter"
    );
    assert_eq!(
        with("x=1", "x=1 X=2"),
        1,
        "the first value of a covariate counts"
    );
    assert_eq!(with("x=1 x=2", "x=2 x=1"), 2);
    assert_eq!(with("x=MEAN", "z=MEAN"), 1);
    let e = &m.emmeans[0];
    assert_eq!(e.factors, ["crop", "soil"]);
    assert_eq!(e.with, [("rain".to_string(), Some(2.5))]);
    assert_eq!(e.compare, Some(Some("soil".into())));
    assert_eq!(e.refcat, Some(RefCat::Last));
    assert_eq!(e.adjustment, Adjustment::Sidak);
    assert_eq!(
        m.emmeans[1].refcat,
        Some(RefCat::Value(Value::String("oat".into())))
    );
    assert!(m.emmeans[2].factors.is_empty());
    let t = &m.tests[0];
    assert_eq!(t.label.as_deref(), Some("soil steps"));
    assert_eq!(t.constants, [0.5, 2.0]);
    assert_eq!(t.rows.len(), 2);
    assert_eq!(t.rows[1][0].1, [0.0, 1.0, -1.0]);
    let all = &m.tests[1];
    assert_eq!(all.constants, [0.0]);
    assert!(all.rows[0][0].0.is_none());
    assert!((all.rows[0][0].1[1] - 1.0 / 3.0).abs() < 1e-15);
    assert!(m.print.g && m.print.r && m.print.covb && m.print.corb);
    // a REFCAT belongs to the COMPARE before it
    let m = mixed(
        "MIXED gain BY crop soil /EMMEANS=TABLES(crop*soil) COMPARE(crop) REFCAT(LAST) COMPARE(soil).",
    );
    assert_eq!(m.emmeans[0].refcat, None);
    assert!(err("MIXED gain BY crop /TEST(1 2)=crop 1 -1.").contains("2 constants for 1 rows"));
    assert!(err("MIXED gain BY crop /TEST=crop 1 -1 DIVISOR=0.").contains("DIVISOR"));
}

#[test]
fn random_with_the_same_effects_and_subjects_keeps_the_last() {
    let m = mixed(
        "MIXED gain /RANDOM=INTERCEPT | SUBJECT(farm) /RANDOM=INTERCEPT | SUBJECT(farm) COVTYPE(ID) /RANDOM=INTERCEPT.",
    );
    assert_eq!(m.random.len(), 2);
    assert_eq!(m.random[0].covtype, CovType::Id);
    assert!(m.random[1].subject.is_empty());
}

#[test]
fn what_is_not_supported_yet_says_so() {
    for (source, word) in [
        (
            "MIXED gain BY crop /RANDOM=plot /TEST=crop 1 -1 | plot 1 -1.",
            "random effects",
        ),
        ("MIXED gain /FIXED=| SSTYPE(1).", "SSTYPE(1)"),
        (
            "MIXED gain /CRITERIA=DFMETHOD(KENWARDROGER).",
            "KENWARDROGER",
        ),
        (
            "MIXED gain /RANDOM=INTERCEPT | COVTYPE(TPH).",
            "COVTYPE(TPH)",
        ),
        ("MIXED gain /RANDOM=INTERCEPT | SOLUTION.", "SOLUTION"),
        (
            "MIXED gain /REPEATED=week | SUBJECT(farm) COVTYPE(UN_AR1).",
            "UN_AR1",
        ),
        (
            "MIXED gain /REPEATED=week | SUBJECT(farm) KRONECKER(a).",
            "KRONECKER",
        ),
        ("MIXED gain /PRINT=SOLUTION CPS.", "CPS"),
        ("MIXED gain /PRINT=HIST(2).", "HISTORY"),
        ("MIXED gain /SAVE=SEPRED.", "SEPRED"),
    ] {
        let message = err(source);
        assert!(
            message.contains(word) && message.contains("not supported yet"),
            "{source}: {message}"
        );
    }
}

#[test]
fn errors_name_the_problem() {
    assert!(err("MIXED gain /REPEATED=week.").contains("SUBJECT"));
    assert!(err("MIXED gain /REPEATED=week | COVTYPE(AR1).").contains("SUBJECT"));
    assert!(err("MIXED gain /REPEATED=week | SUBJECT(week).").contains("both"));
    assert!(err("MIXED gain /RANDOM=a INTERCEPT.").contains("first"));
    assert!(err("MIXED gain /CRITERIA=CIN(100).").contains("below 100"));
    assert!(err("MIXED gain /RANDOM=INTERCEPT | SUBJECT(a*a).").contains("twice"));
    assert!(err("MIXED gain /PLOT=x.").contains("PLOT"));
}
