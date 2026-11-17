//! Parser tests of PLS.

use super::*;

fn pls(source: &str) -> Pls {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    match parsed.remove(0).command {
        Command::Pls(n) => *n,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn pls_dependents_with_levels_references_and_predictors() {
    let p = pls(
        "PLS sales MLEVEL=S rank MLEVEL=N REFERENCE=FIRST a TO c REFERENCE='x' BY shop WITH price.",
    );
    assert_eq!(p.dependents.len(), 3);
    assert_eq!(p.dependents[0].level, Some(PlsLevel::Scale));
    assert_eq!(p.dependents[1].level, Some(PlsLevel::Nominal));
    assert_eq!(p.dependents[1].reference, Some(PlsReference::First));
    assert_eq!(p.dependents[2].item, VarItem::Range("a".into(), "c".into()));
    assert_eq!(
        p.dependents[2].reference,
        Some(PlsReference::Value(Value::String("x".into())))
    );
    assert_eq!(p.factors, vec![VarItem::Name("shop".into())]);
    assert_eq!(p.covariates, vec![VarItem::Name("price".into())]);
    assert_eq!(p.latent_factors, 5);
    assert!(p.model.is_none() && p.id.is_none() && !p.empty_by && !p.empty_with);
}

#[test]
fn pls_all_settings_and_empty_by() {
    let p = pls("PLS y1 y2 ALL MLEVEL=O REFERENCE=3 BY WITH x.");
    assert_eq!(p.dependents.len(), 2);
    assert_eq!(p.all_level, Some(PlsLevel::Ordinal));
    assert_eq!(
        p.all_reference,
        Some(PlsReference::Value(Value::Number(3.0)))
    );
    assert!(p.empty_by && !p.empty_with);
    let p = pls("PLS ALL WITH.");
    assert_eq!(p.dependents[0].item, VarItem::All);
    assert!(p.empty_with);
}

#[test]
fn pls_subcommands() {
    let p = pls(
        "PLS y BY a b WITH x /ID VARIABLE=caseid /MODEL a b a*b a BY x x \
                 /OUTDATASET CASES=c1 LATENTFACTORS=f1 PREDICTORS=p1 /CRITERIA LATENTFACTORS=3.",
    );
    assert_eq!(p.id.as_deref(), Some("caseid"));
    let labels: Vec<&str> = p
        .model
        .as_ref()
        .unwrap()
        .iter()
        .map(|t| t.label.as_str())
        .collect();
    assert_eq!(labels, ["a", "b", "a * b", "a * x", "x"]);
    assert_eq!(p.out_cases.as_deref(), Some("c1"));
    assert_eq!(p.out_factors.as_deref(), Some("f1"));
    assert_eq!(p.out_predictors.as_deref(), Some("p1"));
    assert_eq!(p.latent_factors, 3);
}

#[test]
fn pls_rules_of_the_syntax() {
    assert!(err("PLS y WITH x /MODEL a(b).").contains("nested"));
    assert!(err("PLS y WITH x /MODEL a WITHIN b.").contains("nested"));
    assert!(err("PLS y WITH x /MODEL a*a.").contains("twice"));
    assert!(err("PLS y WITH x /ID VARIABLE=i /ID VARIABLE=j.").contains("more than once"));
    assert!(err("PLS y WITH x /OUTDATASET CASES='out.sav'.").contains("not a file"));
    assert!(err("PLS y WITH x /OUTDATASET CASES=a CASES=b.").contains("more than once"));
    assert!(err("PLS y WITH x /OUTDATASET CASES=a PREDICTORS=a.").contains("twice"));
    assert!(err("PLS y WITH x /CRITERIA LATENTFACTORS=0.").contains("positive whole"));
    assert!(err("PLS y WITH x /CRITERIA LATENTFACTORS=2.5.").contains("positive whole"));
    assert!(err("PLS y WITH x /CRITERIA.").contains("empty"));
    assert!(err("PLS y MLEVEL=S MLEVEL=N WITH x.").contains("more than once"));
    assert!(err("PLS y MLEVEL=X WITH x.").contains("unexpected"));
    // keywords are written in full
    assert!(err("PLS y WITH x /CRIT LATENTFACTORS=2.").contains("unknown subcommand"));
}
