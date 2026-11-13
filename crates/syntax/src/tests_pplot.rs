//! Parser tests of PPLOT and ROC.

use super::*;

fn command(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn pplot(source: &str) -> Pplot {
    match command(source) {
        Command::Pplot(p) => *p,
        other => panic!("{other:?}"),
    }
}

fn roc(source: &str) -> Roc {
    match command(source) {
        Command::Roc(r) => *r,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn pplot_leaves_out_what_is_not_given() {
    let p = pplot("PPLOT VARIABLES=load_kg.");
    assert_eq!(p.variables, vec![VarItem::Name("load_kg".into())]);
    assert_eq!(p.distribution, None);
    assert_eq!((p.fraction, p.ties, p.pp, p.plot), (None, None, None, None));
    // VARIABLES may be left out, and NPPLOT and abbreviations name the same command
    assert_eq!(pplot("NPPLOT load_kg.").variables, p.variables);
    assert_eq!(pplot("PPL VAR=load_kg.").variables, p.variables);
}

#[test]
fn pplot_subcommands() {
    let p = pplot(
        "PPLOT a b /DIST=WEIBUL(2, 1.5) /FRACTION=VW RANKIT /TIES=BREAK /TYPE=P-P \
         /PLOT=DETRENDED /STANDARDIZE /DIFF /SDIFF=2 /PERIOD=7 /LN /NOSTAND.",
    );
    assert_eq!(
        p.distribution,
        Some((TestDistribution::Weibull, vec![2.0, 1.5]))
    );
    assert_eq!(p.fraction, Some(RankFraction::Vw));
    assert_eq!(p.ties, Some(RankTies::Condense));
    assert_eq!(p.pp, Some(true));
    assert_eq!(p.plot, Some(PplotPlot::Detrended));
    assert_eq!(p.standardize, Some(false));
    assert_eq!(
        (p.diff, p.sdiff, p.period, p.ln),
        (Some(1), Some(2), Some(7), Some(true))
    );
    assert_eq!(pplot("PPLOT a /TYPE=Q-Q.").pp, Some(false));
    let d = pplot("PPLOT a /DISTRIBUTION=CHI(3).").distribution;
    assert_eq!(d, Some((TestDistribution::ChiSquare, vec![3.0])));
    let d = pplot("PPLOT a /DISTRIBUTION.").distribution;
    assert_eq!(d, Some((TestDistribution::Normal, vec![])));
}

#[test]
fn pplot_apply_without_variables() {
    let p = pplot("PPLOT APPLY /FRACTION=TUKEY.");
    assert_eq!(p.apply, Some(None));
    assert!(p.variables.is_empty());
    assert_eq!(
        pplot("PPLOT z /APPLY='MOD_1'.").apply,
        Some(Some("MOD_1".into()))
    );
}

#[test]
fn pplot_errors() {
    assert!(err("PPLOT a /DISTRIBUTION=STUDENT.").contains("degrees of freedom"));
    assert!(err("PPLOT a /DISTRIBUTION=NORMAL(0, -1).").contains("scale"));
    assert!(err("PPLOT a /DISTRIBUTION=UNIFORM(5, 2).").contains("minimum"));
    assert!(err("PPLOT a /DISTRIBUTION=GAMMA(2).").contains("2 parameters"));
    assert!(err("PPLOT a /TEMPLATE='x.sgt'.").contains("not supported yet"));
    assert!(err("PPLOT a /DIFF=0.").contains("positive whole number"));
    assert!(err("PPLOT /FRACTION=VW.").contains("VARIABLES"));
}

#[test]
fn roc_defaults_and_options() {
    let r = roc("ROC score BY ill (1).");
    assert_eq!(r.tests, vec![VarItem::Name("score".into())]);
    assert_eq!(
        (r.state.as_str(), &r.positive),
        ("ill", &Value::Number(1.0))
    );
    assert!(r.curve && !r.reference && !r.se && !r.coordinates && !r.include);
    assert_eq!(r.ci, 95.0);
    let r = roc(
        "ROC s1 TO s3 BY grade ('yes') /MISSING=INCLUDE /CRITERIA=CUTOFF(EXCLUDE) TESTPOS(SMALL) \
         CI(90) DISTRIBUTION(NEGEXPO) /PLOT=CURVE(REFERENCE) /PRINT=SE(MODELQUALITY) COORDINATES.",
    );
    assert_eq!(r.positive, Value::String("yes".into()));
    assert!(r.include && r.exclude_cutoff && r.small_positive && r.negexpo);
    assert!(r.reference && r.se && r.model_quality && r.coordinates);
    assert_eq!(r.ci, 90.0);
    assert!(!roc("ROC s BY g(0) /PLOT=NONE.").curve);
}

#[test]
fn roc_errors() {
    assert!(err("ROC s BY g /PLOT=NONE.").contains("unexpected '/'"));
    assert!(err("ROC s BY g(1) /CRITERIA=CI(100).").contains("between 0 and 100"));
    assert!(err("ROC s BY g(1) /PRINT=AREA.").contains("unexpected"));
}
