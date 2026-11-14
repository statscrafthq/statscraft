//! Parser tests of MVA.

use super::*;

fn mva(source: &str) -> Mva {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    match parsed.remove(0).command {
        Command::Mva(m) => *m,
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
fn mva_defaults() {
    let m = mva("MVA VARIABLES=age income.");
    assert_eq!(m.variables, names(&["age", "income"]));
    assert!(m.univariate && !m.listwise && !m.pairwise);
    assert_eq!(m.maxcat, 25);
    assert_eq!(
        (m.ttest, m.crosstab, m.mismatch, m.em, m.regression),
        (None, None, None, None, None)
    );
    // VARIABLES= may be left out; ALL takes the whole file
    assert_eq!(mva("MVA age income.").variables, m.variables);
    assert_eq!(mva("MVA ALL.").variables, vec![VarItem::All]);
}

#[test]
fn mva_description_subcommands() {
    let m = mva(
        "MVA VAR=a b c region /CATEGORICAL=region /MAXCAT=8 /ID=respid /NOUNI \
         /TTEST PERCENT=0 NOT NODF PROB NOCOUNTS NOMEANS /CROSSTAB PERCENT=10 \
         /MISMATCH NOSORT PERCENT=2 /DPATTERN SORT=region(DESCENDING) a DESCRIBE=region \
         /MPATTERN NOSORT DESCRIBE=a b /TPATTERN DESCRIBE=region PERCENT=0 /LISTWISE /PAIRWISE.",
    );
    assert_eq!(m.categorical, names(&["region"]));
    assert_eq!(
        (m.maxcat, m.id.as_deref(), m.univariate),
        (8, Some("respid"), false)
    );
    let t = m.ttest.unwrap();
    assert_eq!(
        (t.percent, t.t, t.df, t.prob, t.counts, t.means),
        (0.0, false, false, true, false, false)
    );
    assert_eq!(m.crosstab, Some(10.0));
    assert_eq!(m.mismatch, Some((2.0, true)));
    let d = m.dpattern.unwrap();
    assert_eq!(
        d.sort,
        vec![("region".to_string(), true), ("a".to_string(), false)]
    );
    assert_eq!(d.describe, names(&["region"]));
    let mp = m.mpattern.unwrap();
    assert!(mp.nosort);
    assert_eq!(mp.describe, names(&["a", "b"]));
    let tp = m.tpattern.unwrap();
    assert_eq!((tp.percent, tp.nosort), (0.0, false));
    assert!(m.listwise && m.pairwise);
    // TPATTERN keeps 1 percent by default, TTEST 5
    let m = mva("MVA a b /TPATTERN /TTEST.");
    assert_eq!(m.tpattern.unwrap().percent, 1.0);
    assert_eq!(m.ttest.unwrap(), MvaTtest::default());
}

#[test]
fn mva_estimation_subcommands() {
    let m = mva(
        "MVA a b c d /EM a b WITH c d (TOLERANCE=0.01 CONVERGENCE=1E-6 ITERATIONS=100 TDF=4 \
         OUTFILE='filled.sav') /REGRESSION WITH c (FLIMIT=2.5 NPREDICTORS=1, ADDTYPE=T(7) \
         OUTFILE 'reg.sav').",
    );
    let em = m.em.unwrap();
    assert_eq!(
        (em.predicted, em.predictors),
        (names(&["a", "b"]), names(&["c", "d"]))
    );
    assert_eq!(
        (em.tolerance, em.convergence, em.iterations),
        (0.01, 1e-6, 100)
    );
    assert_eq!(
        (em.tdf, em.outfile.as_deref()),
        (Some(4.0), Some("filled.sav"))
    );
    let r = m.regression.unwrap();
    assert!(r.predicted.is_empty());
    assert_eq!(r.predictors, names(&["c"]));
    assert_eq!(
        (r.flimit, r.npredictors, r.addtype),
        (2.5, Some(1), Some(AddType::T(7.0)))
    );
    assert_eq!(r.outfile.as_deref(), Some("reg.sav"));
    let m = mva("MVA a b /EM (LAMBDA=3 PROPORTION=0.1) /REGRESSION (ADDTYPE=T).");
    let em = m.em.unwrap();
    assert_eq!((em.lambda, em.proportion), (Some(3.0), Some(0.1)));
    assert_eq!(m.regression.unwrap().addtype, Some(AddType::T(5.0)));
    let m = mva("MVA a b /EM /REGRESSION a (ADDTYPE=NONE).");
    assert_eq!(m.em.unwrap(), MvaEstimate::default());
    assert_eq!(m.regression.unwrap().addtype, Some(AddType::None));
}

#[test]
fn mva_errors() {
    assert!(err("MVA a b /TTEST /ID=x.").contains("must come before"));
    assert!(err("MVA a b /TTEST PERCENT=120.").contains("from 0 to 100"));
    assert!(err("MVA a b /EM (TDF=1).").contains("at least 2"));
    assert!(err("MVA a b /EM (TDF=3 LAMBDA=2 PROPORTION=0.2).").contains("not both"));
    assert!(err("MVA a b /EM (LAMBDA=2).").contains("both LAMBDA and PROPORTION"));
    assert!(err("MVA a b /REGRESSION (ADDTYPE=UNIFORM).").contains("RESIDUAL"));
    assert!(err("MVA a b /EM (FLIMIT=3).").contains("TOLERANCE"));
    assert!(err("MVA a b /PLOT.").contains("unknown subcommand"));
    // WITH and DESCRIBE are reserved inside MVA
    assert!(!err("MVA a describe.").is_empty());
}
