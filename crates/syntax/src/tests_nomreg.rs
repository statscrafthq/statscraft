//! Parser tests of NOMREG.

use super::*;

fn nomreg(source: &str) -> Nomreg {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    match parsed.remove(0).command {
        Command::Nomreg(n) => *n,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn nomreg_defaults_last_category_as_base_and_cps_mfi_printed() {
    let n = nomreg("NOMREG choice.");
    assert_eq!(n.dependent, "choice");
    assert_eq!(n.base, NomregBase::Last);
    assert_eq!(n.order, NomregOrder::Ascending);
    assert!(n.intercept && !n.include && !n.fullfactorial);
    assert_eq!(n.model, None);
    assert_eq!(n.criteria, NomregCriteria::default());
    assert_eq!(n.criteria.mxiter, 100);
    assert_eq!(n.criteria.pconverge, 1e-6);
    let p = n.print;
    assert!(p.cps && p.mfi && !p.parameter && !p.lrt && !p.fit && !p.summary);
    assert_eq!(n.scale, NomregScale::Value(1.0));
}

#[test]
fn nomreg_base_order_factors_covariates_and_abbreviations() {
    let n = nomreg("NOMREG pick (ORDER=DESCENDING BASE='tram') BY zone shift WITH km age.");
    assert_eq!(n.base, NomregBase::Value(Value::String("tram".into())));
    assert_eq!(n.order, NomregOrder::Descending);
    assert_eq!(n.factors.len(), 2);
    assert_eq!(n.covariates.len(), 2);
    let n = nomreg(
        "NOM pick (BASE=FIR ORD=DATA) WITH km /CRIT=CIN(90) MXIT(50) PCON(0) /PRI=PAR LRT FIT SUM CLASS IC KERN COVB CORB.",
    );
    assert_eq!(n.base, NomregBase::First);
    assert_eq!(n.order, NomregOrder::Data);
    assert_eq!(n.criteria.cin, 90.0);
    assert_eq!(n.criteria.mxiter, 50);
    assert_eq!(n.criteria.pconverge, 0.0);
    let p = n.print;
    assert!(p.parameter && p.lrt && p.fit && p.summary && p.classtable && p.ic && p.kernel);
    assert!(p.covb && p.corb && !p.cps && !p.mfi);
    let n = nomreg("NOMREG pick (BASE=3) BY zone.");
    assert_eq!(n.base, NomregBase::Value(Value::Number(3.0)));
}

#[test]
fn nomreg_model_terms_intercept_and_save() {
    let n = nomreg(
        "NOMREG pick BY zone shift WITH km /MODEL=zone shift zone*shift km BY zone /INTERCEPT=EXCLUDE\n\
         /SAVE=ESTPROB(pr:3) PREDCAT(guess) PCPROB ACPROB(hit) /SCALE=PEARSON /SUBPOP=zone shift km /MISSING=INCLUDE.",
    );
    let labels: Vec<&str> = n
        .model
        .as_ref()
        .unwrap()
        .iter()
        .map(|t| t.label.as_str())
        .collect();
    assert_eq!(labels, ["zone", "shift", "zone*shift", "km*zone"]);
    assert!(!n.intercept && n.include);
    assert_eq!(
        n.save,
        [
            NomregSave::Estprob {
                root: Some("pr".into()),
                count: 3
            },
            NomregSave::Predcat(Some("guess".into())),
            NomregSave::Pcprob(None),
            NomregSave::Acprob(Some("hit".into())),
        ]
    );
    assert_eq!(n.scale, NomregScale::Pearson);
    assert_eq!(n.subpop.as_ref().map(Vec::len), Some(3));
    let n = nomreg("NOMREG pick BY zone /SAVE=ESTPROB(:2) /PRINT=NONE PARAMETER /SCALE=2.5.");
    assert_eq!(
        n.save,
        [NomregSave::Estprob {
            root: None,
            count: 2
        }]
    );
    assert!(!n.print.parameter && !n.print.cps);
    assert_eq!(n.scale, NomregScale::Value(2.5));
    let n = nomreg("NOMREG pick BY zone shift /FULLFACTORIAL.");
    assert!(n.fullfactorial);
    let n = nomreg("NOMREG pick BY zone shift /MODEL=INTERCEPT zone.");
    assert_eq!(n.model.unwrap().len(), 1);
}

#[test]
fn nomreg_errors_name_what_is_not_supported() {
    assert!(err("NOMREG pick BY zone /MODEL=zone | FORWARD=zone.").contains("stepwise"));
    assert!(err("NOMREG pick BY zone /STEPWISE=PIN(0.1).").contains("not supported yet"));
    assert!(err("NOMREG pick BY zone /TEST=ALL 1 0.").contains("not supported yet"));
    assert!(err("NOMREG pick BY zone /OUTFILE=MODEL('m.xml').").contains("not supported yet"));
    assert!(err("NOMREG pick BY zone /PRINT=HISTORY(2).").contains("HISTORY"));
    assert!(err("NOMREG pick BY zone /PRINT=CELLPROB.").contains("not supported yet"));
    assert!(err("NOMREG pick zone.").contains("one dependent"));
    assert!(err("NOMREG pick BY zone /FULLFACTORIAL /MODEL=zone.").contains("FULLFACTORIAL"));
    assert!(err("NOMREG pick BY zone /CRITERIA=CIN(100).").contains("CIN"));
    assert!(err("NOMREG pick BY zone /CRITERIA=DELTA(1).").contains("below 1"));
    assert!(err("NOMREG pick BY zone /SCALE=0.").contains("positive"));
    assert!(err("NOMREG pick BY zone /SAVE=PCPROB PCPROB.").contains("only once"));
}
