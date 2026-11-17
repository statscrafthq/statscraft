//! Parser tests of GENLINMIXED.

use super::*;

fn glmm(source: &str) -> GenLinMixed {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    match parsed.remove(0).command {
        Command::GenLinMixed(g) => *g,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn defaults_with_only_the_target() {
    let g = glmm("GENLINMIXED /FIELDS TARGET=score.");
    assert_eq!(g.target, "score");
    assert!(g.subjects.is_empty() && g.random.is_empty() && g.fixed.is_empty());
    assert!(g.intercept && g.distribution.is_none() && g.link.is_none());
    assert_eq!(g.df_method, DfMethod::Residual);
    assert_eq!((g.max_iterations, g.confidence, g.scoring), (100, 95.0, 0));
    assert_eq!((g.pconverge.value, g.singular), (1e-6, 1e-12));
    assert_eq!(g.target_order, GenlinOrder::Ascending);
}

#[test]
fn every_supported_keyword() {
    let g = glmm(
        "GENLINMIXED /DATA_STRUCTURE SUBJECTS=ward BY bed\n\
         /FIELDS TARGET=falls TRIALS=FIELD(nights) ANALYSIS_WEIGHT=w OFFSET=VALUE(0.5)\n\
         /TARGET_OPTIONS DISTRIBUTION=BINOMIAL LINK=PROBIT REFERENCE=1\n\
         /FIXED EFFECTS=shift age shift*age USE_INTERCEPT=FALSE\n\
         /RANDOM EFFECTS=age USE_INTERCEPT=TRUE SUBJECTS=ward COVARIANCE_TYPE=UNSTRUCTURED SOLUTION=TRUE\n\
         /RANDOM USE_INTERCEPT=TRUE SUBJECTS=ward*bed\n\
         /BUILD_OPTIONS TARGET_CATEGORY_ORDER=DESCENDING MAX_ITERATIONS=40 CONFIDENCE_LEVEL=90\n\
         DF_METHOD=SATTERTHWAITE PCONVERGE=1E-8(RELATIVE) LCONVERGE=0.001 SCORING=2 SINGULAR=1E-10\n\
         COVB=MODEL INPUTS_CATEGORY_ORDER=ASCENDING\n\
         /EMMEANS_OPTIONS SCALE=ORIGINAL PADJUST=LSD CONTROL=age(40).",
    );
    assert_eq!(g.subjects, ["ward", "bed"]);
    assert_eq!(g.trials, Some(GenlinTrials::Variable("nights".into())));
    assert_eq!(g.analysis_weight.as_deref(), Some("w"));
    assert_eq!(g.offset, Some(GenlinOffset::Value(0.5)));
    assert_eq!(g.distribution, Some(GenlinDistribution::Binomial));
    assert_eq!(g.link, Some(GenlinLink::Probit));
    assert_eq!(g.reference, Some(Value::Number(1.0)));
    let labels: Vec<&str> = g.fixed.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(labels, ["shift", "age", "shift*age"]);
    assert!(!g.intercept);
    assert_eq!(g.random.len(), 2);
    let r = &g.random[0];
    let labels: Vec<&str> = r.terms.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(labels, ["Intercept", "age"]);
    assert_eq!((r.covtype, r.solution), (CovType::Un, true));
    assert_eq!(g.random[1].subjects, ["ward", "bed"]);
    assert_eq!(g.random[1].covtype, CovType::Vc);
    assert_eq!(g.target_order, GenlinOrder::Descending);
    assert_eq!((g.max_iterations, g.confidence, g.scoring), (40, 90.0, 2));
    assert_eq!(g.df_method, DfMethod::Satterthwaite);
    assert!(g.pconverge.relative && g.pconverge.value == 1e-8);
    assert_eq!(g.lconverge.value, 0.001);
    assert_eq!(g.singular, 1e-10);
}

#[test]
fn power_link_and_the_other_spellings() {
    let g = glmm(
        "GENLINMIXED /FIELDS TARGET=cost ANALYSISWEIGHT=n OFFSET=lg TRIALS=NONE\n\
         /TARGET_OPTIONS DISTRIBUTION=GAMMA LINK=POWER LINK_PARAMETER=-1 /RANDOM USE_INTERCEPT=TRUE.",
    );
    assert_eq!(g.link, Some(GenlinLink::Power(-1.0)));
    assert_eq!(g.offset, Some(GenlinOffset::Variable("lg".into())));
    assert_eq!(g.analysis_weight.as_deref(), Some("n"));
    assert!(g.trials.is_none());
    let g = glmm("GENLINMIXED /FIELDS TARGET=cost /TARGET_OPTIONS LINK=POWER(0.5).");
    assert_eq!(g.link, Some(GenlinLink::Power(0.5)));
    let types = [
        ("DIAGONAL", CovType::Diag),
        ("AR1", CovType::Ar1),
        ("ARH1", CovType::Arh1),
        ("ARMA11", CovType::Arma11),
        ("COMPOUND_SYMMETRY", CovType::Cs),
        ("CSH", CovType::Csh),
        ("IDENTITY", CovType::Id),
        ("TOEPLITZ", CovType::Tp),
        ("VARIANCE_COMPONENTS", CovType::Vc),
    ];
    for (word, t) in types {
        let g = glmm(&format!(
            "GENLINMIXED /FIELDS TARGET=y /RANDOM EFFECTS=a COVARIANCE_TYPE={word}."
        ));
        assert_eq!(g.random[0].covtype, t, "{word}");
    }
}

#[test]
fn errors_name_what_is_wrong_or_not_supported_yet() {
    let cases = [
        ("GENLINMIXED /FIXED EFFECTS=a.", "TARGET"),
        (
            "GENLINMIXED /FIELDS TARGET=y /RANDOM SUBJECTS=id.",
            "USE_INTERCEPT=TRUE",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y /FIXED USE_INTERCEPT=TRUE USE_INTERCEPT=FALSE.",
            "only once",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y /FIXED EFFECTS=a /FIXED EFFECTS=b.",
            "only once",
        ),
        ("GENLINMIXED /FIELDS TARGET=y /FIXED.", "EFFECTS"),
        (
            "GENLINMIXED /FIELDS TARGET=y /TARGET_OPTIONS LINK=POWER.",
            "LINK_PARAMETER",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y /TARGET_OPTIONS DISTRIBUTION=MULTINOMIAL.",
            "not supported yet",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y /TARGET_OPTIONS DISTRIBUTION=NEGATIVE_BINOMIAL.",
            "not supported yet",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y /TARGET_OPTIONS LINK=CAUCHIT.",
            "not support",
        ),
        (
            "GENLINMIXED /DATA_STRUCTURE SUBJECTS=id REPEATED_MEASURES=t /FIELDS TARGET=y.",
            "not supported yet",
        ),
        (
            "GENLINMIXED /DATA_STRUCTURE COVARIANCE_TYPE=AR1 /FIELDS TARGET=y.",
            "SUBJECTS",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y /RANDOM EFFECTS=a GROUPING=g.",
            "not supported yet",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y /RANDOM EFFECTS=a COVARIANCE_TYPE=UN_AR1.",
            "repeated measures",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y /BUILD_OPTIONS COVB=ROBUST.",
            "not supported yet",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y /BUILD_OPTIONS INPUTS_CATEGORY_ORDER=DATA.",
            "not supported yet",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y /BUILD_OPTIONS CONFIDENCE_LEVEL=100.",
            "below 100",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y /EMMEANS TABLES=a.",
            "not supported yet",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y /SAVE PREDICTED_VALUES(p).",
            "not supported yet",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y /OUTFILE MODEL='m.xml'.",
            "not supported yet",
        ),
        (
            "GENLINMIXED /FIELDS TARGET=y TRIALS=VALUE(0).",
            "whole number",
        ),
    ];
    for (source, part) in cases {
        let message = err(source);
        assert!(message.contains(part), "{source}: {message}");
    }
}
