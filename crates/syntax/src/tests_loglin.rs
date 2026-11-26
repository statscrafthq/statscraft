//! Parser tests of LOGLINEAR.

use super::*;

fn ll(source: &str) -> Loglinear {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    match parsed.remove(0).command {
        Command::Loglinear(l) => *l,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn variables_ranges_by_and_covariates() {
    let l = ll("LOGLINEAR a b (1,3) c (0 1) BY d (1,2) WITH x y.");
    assert_eq!(l.dependents.len(), 2);
    assert_eq!(l.dependents[0].variables.len(), 2);
    assert_eq!((l.dependents[1].min, l.dependents[1].max), (0.0, 1.0));
    assert_eq!(l.independents.len(), 1);
    assert_eq!(l.covariates.len(), 2);
    // no DESIGN: one saturated model, and not an added one
    assert_eq!(l.models.len(), 1);
    assert!(l.models[0].terms.is_none() && !l.implicit_design);
    assert!(err("LOGLINEAR a (3,1).").contains("minimum first"));
    assert!(err("LOGLINEAR a (1.5,3).").contains("whole numbers"));
    assert!(err("LOGLINEAR a.").contains("expected"));
}

#[test]
fn settings_carry_to_later_designs_and_designs_do_not() {
    let l = ll(
        "LOGLINEAR a b (1,3) /PRINT=ESTIM /DESIGN=a b /PRINT=ALL /CRITERIA=ITERATION(50) \
                CONVERGE(0.0001) /DESIGN=a BY b /PLOT=NORMPROB.",
    );
    assert_eq!(l.models.len(), 3);
    assert!(l.implicit_design, "the last subcommand is not DESIGN");
    let first = &l.models[0];
    assert!(first.print.estim && !first.print.cor && first.print.freq);
    assert_eq!(first.criteria, LoglinearCriteria::default());
    assert_eq!(first.terms.as_ref().unwrap().len(), 2);
    let second = &l.models[1];
    assert!(second.print.cor && second.print.design);
    assert_eq!(
        (second.criteria.iterate, second.criteria.converge),
        (50, 0.0001)
    );
    assert_eq!(second.terms.as_ref().unwrap()[0].len(), 2);
    let third = &l.models[2];
    assert!(
        third.terms.is_none(),
        "the added model is the saturated one"
    );
    assert!(third.plot_normal && !third.plot_resid);
    assert_eq!(third.criteria.iterate, 50);
}

#[test]
fn print_and_plot_keywords() {
    let l = ll(
        "LOGLINEAR a b (1,2) /PRINT=NONE /DESIGN /PRINT=COV DESIGN /DESIGN /PLOT=DEFAULT /DESIGN \
                /PLOT=NONE /DESIGN.",
    );
    assert!(l.models[0].print.none && !l.models[0].print.freq);
    let second = l.models[1].print;
    assert!(second.cor && second.design && !second.freq && !second.none);
    assert!(l.models[2].plot_resid && l.models[2].plot_normal);
    assert!(!l.models[3].plot_resid);
    assert!(err("LOGLINEAR a b (1,2) /PRINT=BOGUS.").contains("expected"));
}

#[test]
fn partitions_and_contrasts() {
    let l = ll(
        "LOGLINEAR a b (1,4) /CONTRAST(b)=POLYNOMIAL /DESIGN=a a BY b(1) \
                /CONTRAST(b)=SIMPLE(2) /CONTRAST(a)=BASIS SPECIAL(4*1, 1 -1 0 0, 0 1 -1 0, 0 0 1 -1) /DESIGN=a b.",
    );
    let first = &l.models[0];
    assert_eq!(first.contrasts.len(), 1);
    let term = &first.terms.as_ref().unwrap()[1];
    assert_eq!(term[1].category, Some(1));
    let second = &l.models[1];
    assert_eq!(second.contrasts.len(), 2);
    let b = second.contrasts.iter().find(|c| c.variable == "b").unwrap();
    assert_eq!(
        b.kind,
        LoglinearContrastKind::Named(Contrast::Simple(Some(2)))
    );
    match &second
        .contrasts
        .iter()
        .find(|c| c.variable == "a")
        .unwrap()
        .kind
    {
        LoglinearContrastKind::Special { values, basis } => {
            assert_eq!(values.len(), 16);
            assert!(*basis);
            assert_eq!(&values[..5], &[1.0, 1.0, 1.0, 1.0, 1.0]);
            assert_eq!(values[7], 0.0);
        }
        other => panic!("{other:?}"),
    }
    assert!(err("LOGLINEAR a (1,3) /CONTRAST(a)=SPECIAL(1 2 3).").contains("squared"));
    assert!(err("LOGLINEAR a (1,3) /CONTRAST(a)=BASIS SIMPLE.").contains("expected"));
}

#[test]
fn cell_weights_and_generalized_residuals() {
    let l = ll(
        "LOGLINEAR a b (1,2) /CWEIGHT=(0, 3*1) /GRESID=(1 -1 0 0) /GRESID=g /DESIGN=a b \
                /CWEIGHT=(4*1) /DESIGN=a b.",
    );
    let first = &l.models[0];
    assert_eq!(
        first.cweight,
        Some(CellWeights::Values(vec![0.0, 1.0, 1.0, 1.0]))
    );
    assert_eq!(first.gresid.len(), 2);
    assert_eq!(
        first.gresid[0],
        CellWeights::Values(vec![1.0, -1.0, 0.0, 0.0])
    );
    assert_eq!(first.gresid[1], CellWeights::Variable("g".into()));
    assert_eq!(l.models[1].gresid.len(), 2, "GRESID stays in force");
    assert_eq!(l.models[1].cweight, Some(CellWeights::Values(vec![1.0; 4])));
    // a variable on CWEIGHT allows no other CWEIGHT
    assert!(
        err("LOGLINEAR a b (1,2) /CWEIGHT=w /CWEIGHT=(4*1).").contains("cannot be given again")
    );
    assert!(err("LOGLINEAR a b (1,2) /CWEIGHT=(4*1) /CWEIGHT=w.").contains("only as the one"));
}

#[test]
fn missing_and_subcommand_errors() {
    assert!(ll("LOGLINEAR a b (1,2) /MISSING=INCLUDE.").include);
    assert!(!ll("LOGLINEAR a b (1,2) /MISSING=DEFAULT.").include);
    assert!(err("LOGLINEAR a b (1,2) /MISSING=INCLUDE /MISSING=EXCLUDE.").contains("only once"));
    assert!(err("LOGLINEAR a b (1,2) /BOGUS.").contains("unknown subcommand"));
    assert!(err("LOGLINEAR a b (1,2) /CRITERIA=ITERATE(0).").contains("whole number"));
    assert!(err("LOGLINEAR a b (1,2) /CRITERIA=DELTA(-1).").contains("not negative"));
}
