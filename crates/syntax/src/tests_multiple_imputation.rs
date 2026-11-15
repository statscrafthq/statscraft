//! Parser tests of MULTIPLE IMPUTATION.

use super::*;

fn mi(source: &str) -> MultipleImputation {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    match parsed.remove(0).command {
        Command::MultipleImputation(m) => *m,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    let error = errors.remove(0);
    format!("{} {:?}", error.message, error.expected)
}

fn names(list: &[&str]) -> Vec<VarItem> {
    list.iter().map(|n| VarItem::Name(n.to_string())).collect()
}

#[test]
fn defaults() {
    let m = mi("MULTIPLE IMPUTATION rent hhsize /OUTFILE IMPUTATIONS=filled.");
    assert_eq!(m.variables, names(&["rent", "hhsize"]));
    assert_eq!(m.method, ImputeMethod::Auto);
    assert_eq!(
        (m.nimputations, m.pmm, m.twoway, m.maxpctmissing),
        (5, None, false, None)
    );
    assert_eq!(
        (m.maxcasedraws, m.maxparamdraws, m.maxiter, m.maxmodelparam),
        (50, 2, 10, 100)
    );
    assert_eq!(m.singular, 1e-12);
    assert!(m.overall && m.models && !m.patterns);
    assert_eq!((m.variable_summary, m.descriptives), (None, None));
    assert_eq!(m.imputations.as_deref(), Some("filled"));
    // the command name may be shortened word by word
    assert_eq!(
        mi("MULT IMPUTATION a b /OUTFILE IMPUTATIONS='x.sav'.").variables,
        names(&["a", "b"])
    );
}

#[test]
fn every_subcommand() {
    let m = mi("MULTIPLE IMPUTATION a TO c ALL \
         /IMPUTE METHOD=FCS NIMPUTATIONS=20 SCALEMODEL=PMM(3) INTERACTIONS=TWOWAY \
         MAXPCTMISSING=40 MAXCASEDRAWS=9 MAXPARAMDRAWS=4 MAXITER=25 SINGULAR=1E-8 MAXMODELPARAM=300 \
         /CONSTRAINTS a b (MIN=1 MAX=7 RND=0.5 ROLE=DEP) \
         /CONSTRAINTS c (MIN=NONE RND=NO ROLE=IND) /CONSTRAINTS d (MAX='12/31/2020') \
         /MISSINGSUMMARIES VARIABLES(MAXVARS=4 MINPCTMISSING=0) PATTERNS \
         /IMPUTATIONSUMMARIES DESCRIPTIVES(a c) \
         /ANALYSISWEIGHT w /OUTFILE IMPUTATIONS='out.sav' FCSITERATIONS=trace.");
    assert_eq!(
        m.variables,
        vec![VarItem::Range("a".into(), "c".into()), VarItem::All]
    );
    assert_eq!(m.method, ImputeMethod::Fcs);
    assert_eq!((m.nimputations, m.pmm, m.twoway), (20, Some(3), true));
    assert_eq!(m.maxpctmissing, Some(40.0));
    assert_eq!(
        (m.maxcasedraws, m.maxparamdraws, m.maxiter, m.maxmodelparam),
        (9, 4, 25, 300)
    );
    assert_eq!(m.singular, 1e-8);
    assert_eq!(m.given.len(), 10);
    assert_eq!(m.constraints.len(), 3);
    let first = &m.constraints[0];
    assert_eq!(first.variables, names(&["a", "b"]));
    assert_eq!(first.min, Some(Some(Value::Number(1.0))));
    assert_eq!(first.max, Some(Some(Value::Number(7.0))));
    assert_eq!(
        (first.rnd, first.role),
        (Some(Some(0.5)), Some(ImputeRole::Dependent))
    );
    let second = &m.constraints[1];
    assert_eq!((second.min.clone(), second.max.clone()), (Some(None), None));
    assert_eq!(
        (second.rnd, second.role),
        (Some(None), Some(ImputeRole::Predictor))
    );
    assert_eq!(
        m.constraints[2].max,
        Some(Some(Value::String("12/31/2020".into())))
    );
    assert!(!m.overall && m.patterns && !m.models);
    assert_eq!(m.variable_summary, Some((4, 0.0)));
    assert_eq!(m.descriptives, Some(names(&["a", "c"])));
    assert_eq!(m.analysis_weight.as_deref(), Some("w"));
    assert_eq!(m.imputations.as_deref(), Some("out.sav"));
    assert_eq!(m.fcs_iterations.as_deref(), Some("trace"));
    // NONE alone, and no OUTFILE needed without imputation
    let none = mi(
        "MULTIPLE IMPUTATION a b /IMPUTE METHOD=NONE /MISSINGSUMMARIES NONE \
         /IMPUTATIONSUMMARIES NONE.",
    );
    assert_eq!(none.method, ImputeMethod::None);
    assert!(!none.overall && !none.models);
    let plain = mi(
        "MULTIPLE IMPUTATION a b /IMPUTE SCALEMODEL=PMM /MISSINGSUMMARIES VARIABLES \
         /IMPUTATIONSUMMARIES MODELS DESCRIPTIVES /OUTFILE IMPUTATIONS=d.",
    );
    assert_eq!(plain.pmm, Some(5));
    assert_eq!(plain.variable_summary, Some((25, 10.0)));
    assert_eq!(plain.descriptives, Some(Vec::new()));
}

#[test]
fn rules_of_the_syntax() {
    // OUTFILE IMPUTATIONS is needed while imputing
    assert!(err("MULTIPLE IMPUTATION a b.").contains("needs /OUTFILE IMPUTATIONS="));
    assert!(err("MULTIPLE IMPUTATION a b /OUTFILE FCSITERATIONS=t.").contains("IMPUTATIONS="));
    // keywords in full and equals signs written
    assert!(
        err("MULTIPLE IMPUTATION a b /IMPUTE METH=FCS /OUTFILE IMPUTATIONS=d.").contains("METHOD")
    );
    assert!(
        err("MULTIPLE IMPUTATION a b /IMP METHOD=FCS /OUTFILE IMPUTATIONS=d.")
            .contains("unknown subcommand IMP")
    );
    assert!(
        err("MULTIPLE IMPUTATION a b /IMPUTE METHOD FCS /OUTFILE IMPUTATIONS=d.").contains("'='")
    );
    // once per subcommand, once per keyword
    assert!(
        err("MULTIPLE IMPUTATION a b /IMPUTE MAXITER=3 MAXITER=4 /OUTFILE IMPUTATIONS=d.")
            .contains("MAXITER is given twice")
    );
    assert!(
        err(
            "MULTIPLE IMPUTATION a b /IMPUTE MAXITER=3 /IMPUTE NIMPUTATIONS=2 \
         /OUTFILE IMPUTATIONS=d."
        )
        .contains("IMPUTE is given twice")
    );
    assert!(
        err("MULTIPLE IMPUTATION a b /CONSTRAINTS a (MIN=1 MIN=2) /OUTFILE IMPUTATIONS=d.")
            .contains("MIN is given twice")
    );
    // empty subcommands, NONE with others, ranges
    assert!(err("MULTIPLE IMPUTATION a b /IMPUTE /OUTFILE IMPUTATIONS=d.").contains("METHOD"));
    assert!(
        err("MULTIPLE IMPUTATION a b /MISSINGSUMMARIES /OUTFILE IMPUTATIONS=d.")
            .contains("a keyword")
    );
    assert!(
        err("MULTIPLE IMPUTATION a b /MISSINGSUMMARIES NONE PATTERNS /OUTFILE IMPUTATIONS=d.")
            .contains("NONE cannot stand")
    );
    assert!(
        err("MULTIPLE IMPUTATION a b /IMPUTATIONSUMMARIES MODELS NONE /OUTFILE IMPUTATIONS=d.")
            .contains("NONE cannot stand")
    );
    assert!(
        err("MULTIPLE IMPUTATION a b /CONSTRAINTS a () /OUTFILE IMPUTATIONS=d.")
            .contains("needs MIN, MAX, RND or ROLE")
    );
    assert!(
        err("MULTIPLE IMPUTATION a b /CONSTRAINTS a (MIN=5 MAX=5) /OUTFILE IMPUTATIONS=d.")
            .contains("must be below MAX")
    );
    assert!(
        err("MULTIPLE IMPUTATION a b /IMPUTE MAXPCTMISSING=100 /OUTFILE IMPUTATIONS=d.")
            .contains("above 0 and below 100")
    );
    assert!(
        err("MULTIPLE IMPUTATION a b /IMPUTE NIMPUTATIONS=0 /OUTFILE IMPUTATIONS=d.")
            .contains("positive whole number")
    );
    assert!(
        err("MULTIPLE IMPUTATION a b /IMPUTE SINGULAR=0 /OUTFILE IMPUTATIONS=d.")
            .contains("positive number")
    );
    assert!(
        err("MULTIPLE IMPUTATION a b /CONSTRAINTS a (ROLE=PRED) /OUTFILE IMPUTATIONS=d.")
            .contains("BOTH")
    );
}
