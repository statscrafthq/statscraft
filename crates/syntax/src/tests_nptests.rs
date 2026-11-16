//! Parser tests of NPTESTS.

use crate::*;

fn np(source: &str) -> Nptests {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    match parsed.remove(0).command {
        Command::Nptests(command) => *command,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors[0].message.clone()
}

fn var(name: &str) -> VarItem {
    VarItem::Name(name.to_string())
}

#[test]
fn defaults_and_automatic_choice() {
    let c = np("NPTESTS /ONESAMPLE TEST(hue taste).");
    let one = c.one_sample.unwrap();
    assert_eq!(one.fields, vec![var("hue"), var("taste")]);
    assert!(one.automatic());
    assert_eq!((c.alpha, c.cilevel, c.seed), (0.05, 95.0, Some(2_000_000)));
    assert!(!c.listwise && !c.include);
}

#[test]
fn one_sample_keywords() {
    let c = np("NPTESTS /ONESAMPLE TEST(a TO c) \
         CHISQUARE(EXPECTED=CUSTOM(FREQUENCIES=2 1 1 CATEGORIES=1 2 'x')) \
         BINOMIAL(TESTVALUE=0.3 SUCCESSCATEGORICAL=LIST(4 5) SUCCESSCONTINUOUS=CUTPOINT(12.5) JEFFREYS LIKELIHOOD) \
         KOLMOGOROV_SMIRNOV(NSAMPLES=500 MC_CILEVEL=95 NORMAL=SAMPLE(SIMULATION=FALSE) UNIFORM=CUSTOM(MAX=9 MIN=1) POISSON=CUSTOM(MEAN=2.5)) \
         RUNS(GROUPCATEGORICAL=LIST(1) GROUPCONTINUOUS=CUTPOINT(SAMPLEMEAN)) WILCOXON(TESTVALUE=40) \
         /MISSING SCOPE=LISTWISE USERMISSING=INCLUDE /CRITERIA ALPHA=0.1 CILEVEL=90 SEED=RANDOM.");
    let one = c.one_sample.unwrap();
    assert_eq!(
        one.chisquare,
        Some(Some(vec![
            (Value::Number(1.0), 2.0),
            (Value::Number(2.0), 1.0),
            (Value::String("x".into()), 1.0)
        ]))
    );
    let b = one.binomial.unwrap();
    assert_eq!(b.test_value, 0.3);
    assert_eq!(
        b.success,
        Some(vec![Value::Number(4.0), Value::Number(5.0)])
    );
    assert_eq!(b.cut, Some(12.5));
    assert!(!b.clopper_pearson && b.jeffreys && b.likelihood);
    let ks = one.ks.unwrap();
    assert_eq!((ks.samples, ks.mc_cilevel), (500, 95.0));
    assert_eq!(ks.normal, Some(NpParameters::Sample { simulation: false }));
    // MIN and MAX in either order
    assert_eq!(ks.uniform, Some(NpParameters::Custom(vec![1.0, 9.0])));
    assert_eq!(ks.poisson, Some(2.5));
    assert_eq!(ks.exponential, None);
    let runs = one.runs.unwrap();
    assert_eq!(runs.group, Some(vec![Value::Number(1.0)]));
    assert_eq!(runs.cut, NpCut::Mean);
    assert_eq!(one.wilcoxon, Some(40.0));
    assert!(c.listwise && c.include);
    assert_eq!((c.alpha, c.cilevel, c.seed), (0.1, 90.0, None));
}

#[test]
fn ks_alone_is_normal_from_the_sample() {
    let c = np("NPTESTS /ONESAMPLE TEST(v) KOLMOGOROV_SMIRNOV.");
    let ks = c.one_sample.unwrap().ks.unwrap();
    assert_eq!(ks.normal, Some(NpParameters::Sample { simulation: true }));
    assert_eq!(ks.samples, 1000);
}

#[test]
fn independent_and_related_keywords() {
    let c = np(
        "NPTESTS /INDEPENDENT TEST(y) GROUP(g) MANN_WHITNEY HODGES_LEHMAN MOSES(TRIMOUTLIERS=2) \
         KRUSKAL_WALLIS(COMPARE=STEPWISE) MEDIAN(TESTVALUE=10 COMPARE=NONE) JONCKHEERE_TERPSTRA(ORDER=DESCENDING) \
         /RELATED TEST(p q r) MCNEMAR(SUCCESS=LIST(1)) SIGN COCHRAN(COMPARE=NONE) KENDALL /CRITERIA SEED(77).",
    );
    let ind = c.independent.unwrap();
    assert_eq!(ind.group, "g");
    assert!(ind.mann_whitney && ind.hodges_lehmann && !ind.ks && !ind.automatic());
    assert_eq!(ind.moses, Some(Some(2.0)));
    assert_eq!(ind.kruskal_wallis, Some(NpCompare::Stepwise));
    assert_eq!(ind.median, Some((Some(10.0), NpCompare::None)));
    assert_eq!(ind.jonckheere, Some((true, NpCompare::Pairwise)));
    let rel = c.related.unwrap();
    assert_eq!(rel.mcnemar, Some(Some(vec![Value::Number(1.0)])));
    assert!(rel.sign && !rel.wilcoxon);
    assert_eq!(rel.cochran, Some((None, NpCompare::None)));
    assert_eq!(rel.kendall, Some(NpCompare::Pairwise));
    assert_eq!(c.seed, Some(77));
}

#[test]
fn errors() {
    assert!(err("NPTESTS /MISSING SCOPE=LISTWISE.").contains("ONESAMPLE, INDEPENDENT or RELATED"));
    assert!(err("NPTESTS /ONESAMPLE TEST(a) /ONESAMPLE TEST(b).").contains("only once"));
    assert!(err("NPTESTS /ONESAMPLE TEST(a) RUNS RUNS.").contains("only once"));
    assert!(err("NPTESTS /ONESAMPLE TEST(a) WILCOXON.").contains("TESTVALUE"));
    assert!(err("NPTESTS /INDEPENDENT TEST(a) MANN_WHITNEY.").contains("GROUP"));
    assert!(err("NPTESTS /ONESAMPLE TEST(a) /CRITERIA ALPHA=2.").contains("ALPHA"));
    assert!(err("NPTESTS /ONESAMPLE TEST(a) /CRITERIA.").contains("empty"));
    // names are written in full
    assert!(!err("NPTESTS /ONES TEST(a).").is_empty());
    assert!(
        err("NPTESTS /ONESAMPLE TEST(a) CHISQUARE(EXPECTED=CUSTOM(FREQUENCIES=1 2 CATEGORIES=1)).")
            .contains("2 frequencies for 1 categories")
    );
}
