//! Parser tests of NPAR TESTS and PARTIAL CORR.

use crate::*;

fn one(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn err(source: &str) -> SyntaxError {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0)
}

fn var(name: &str) -> VarItem {
    VarItem::Name(name.to_string())
}

fn npar(source: &str) -> NparTests {
    match one(source) {
        Command::NparTests(command) => *command,
        other => panic!("{other:?}"),
    }
}

#[test]
fn one_sample_tests() {
    let c = npar(
        "NPAR TESTS CHISQUARE=a b (1,5) /EXPECTED=12, 3*16, 18 /BINOMIAL(.667)=c(0,1) \
         /RUNS(MEDIAN)=d /RUNS(24.5)=d /K-S(UNIFORM)=e /K-S(NORMAL,0,1)=f /K-S(POISSON 3)=g.",
    );
    assert_eq!(
        c.tests[0],
        NparTest::ChiSquare {
            variables: vec![var("a"), var("b")],
            range: Some((1.0, 5.0)),
            expected: vec![12.0, 16.0, 16.0, 16.0, 18.0],
        }
    );
    assert_eq!(
        c.tests[1],
        NparTest::Binomial {
            p: 0.667,
            variables: vec![var("c")],
            split: BinomialSplit::Values(0.0, 1.0),
        }
    );
    assert!(matches!(
        c.tests[2],
        NparTest::Runs {
            cut: RunsCut::Median,
            ..
        }
    ));
    assert!(matches!(c.tests[3], NparTest::Runs { cut: RunsCut::Value(v), .. } if v == 24.5));
    assert!(matches!(
        &c.tests[4],
        NparTest::KolmogorovSmirnov { distribution: KsDistribution::Uniform, parameters, .. } if parameters.is_empty()
    ));
    assert!(matches!(
        &c.tests[5],
        NparTest::KolmogorovSmirnov { distribution: KsDistribution::Normal, parameters, .. } if parameters == &[0.0, 1.0]
    ));
    assert!(matches!(
        &c.tests[6],
        NparTest::KolmogorovSmirnov { distribution: KsDistribution::Poisson, parameters, .. } if parameters == &[3.0]
    ));
    // a cut point, and the default dichotomy
    let c = npar("NPAR TESTS BINOMIAL=x(3) /BINOMIAL=y.");
    assert!(matches!(
        c.tests[0],
        NparTest::Binomial { p, split: BinomialSplit::Cut(3.0), .. } if p == 0.5
    ));
    assert!(matches!(
        c.tests[1],
        NparTest::Binomial {
            split: BinomialSplit::Dichotomy,
            ..
        }
    ));
}

#[test]
fn long_names_and_abbreviations() {
    let c = npar(
        "NPAR TEST /CHI-SQUARE=a /KOLMOGOROV-SMIRNOV(EXPONENTIAL)=b /MANN-WHITNEY=c BY g(1,2) \
         /WALD-WOLFOWITZ=c BY g(1,2) /KRUSKAL-WALLIS=c BY g(1,4) /JONCKHEERE-TERPSTRA=c BY g(1,4) \
         /K-S=c BY g(0,1) /MOS(2)=c BY g(0,1) /MED(8.4)=c BY g(1,2).",
    );
    let groups: Vec<GroupTest> = c
        .tests
        .iter()
        .filter_map(|t| match t {
            NparTest::Groups { test, .. } => Some(*test),
            _ => None,
        })
        .collect();
    assert_eq!(
        groups,
        [
            GroupTest::MannWhitney,
            GroupTest::WaldWolfowitz,
            GroupTest::KruskalWallis,
            GroupTest::JonckheereTerpstra,
            GroupTest::KolmogorovSmirnov,
            GroupTest::Moses(Some(2.0)),
            GroupTest::Median(Some(8.4)),
        ]
    );
    assert!(matches!(c.tests[0], NparTest::ChiSquare { .. }));
    assert!(matches!(
        c.tests[1],
        NparTest::KolmogorovSmirnov {
            distribution: KsDistribution::Exponential,
            ..
        }
    ));
    let c = npar(
        "NPAR TESTS CHI=a /BIN=b /WIL=a WITH b /MCN=a b /COC=a b /FRI=a b /KEN=a b \
         /STAT /MISS=LIST INCL /METH=EXACT TIMER(5) /SAMP.",
    );
    assert_eq!(c.tests.len(), 7);
    assert!(c.descriptives && !c.quartiles && c.listwise && c.include && c.exact);
}

#[test]
fn related_and_group_tests() {
    let c = npar(
        "NPAR TESTS WILCOXON=A B WITH C D (PAIRED) /SIGN=N1,M1 WITH N2,M2 (PAIRED) /MH=V1 V2 V3 \
         /MCNEMAR=V1 WITH V2 /COCHRAN=RV1 TO RV3 /M-W=V1 BY V2(1,2) /STATISTICS=QUARTILES \
         /METHOD=MC CIN(99) SAMPLES(10000).",
    );
    let NparTest::Paired { test, pairs } = &c.tests[0] else {
        panic!()
    };
    assert_eq!(*test, PairedTest::Wilcoxon);
    assert!(pairs.paired && pairs.right.as_deref() == Some(&[var("C"), var("D")][..]));
    assert!(
        matches!(&c.tests[1], NparTest::Paired { test: PairedTest::Sign, pairs } if pairs.left.len() == 2)
    );
    assert!(matches!(
        &c.tests[2],
        NparTest::Paired { test: PairedTest::MarginalHomogeneity, pairs } if pairs.right.is_none()
    ));
    assert!(matches!(
        &c.tests[4],
        NparTest::Related { test: RelatedTest::Cochran, variables } if variables == &[VarItem::Range("RV1".into(), "RV3".into())]
    ));
    assert_eq!(
        c.tests[5],
        NparTest::Groups {
            test: GroupTest::MannWhitney,
            variables: vec![var("V1")],
            group: "V2".into(),
            values: (1.0, 2.0),
        }
    );
    assert!(c.quartiles && !c.descriptives && c.exact);
    // ALL is both, and the KS_SIM settings are accepted
    let c =
        npar("NPAR TESTS K-S(NORMAL)=a /KS_SIM CIN(99) SAMPLES(10000) NONORMAL /STATISTICS=ALL.");
    assert!(c.descriptives && c.quartiles);
}

#[test]
fn npar_errors() {
    assert_eq!(
        err("NPAR TESTS /STATISTICS=ALL.").message,
        "NPAR TESTS needs at least one test subcommand, such as /M-W=var BY group(1,2)"
    );
    assert_eq!(
        err("NPAR TESTS EXPECTED=1 2 /CHISQUARE=a.").message,
        "EXPECTED applies to the CHISQUARE before it, and there is none"
    );
    assert_eq!(
        err("NPAR TESTS WILCOXON=a b (PAIRED).").message,
        "(PAIRED) needs a list after WITH to pair with"
    );
    assert_eq!(err("NPAR TESTS M-W=a BY g(1).").message, "unexpected ')'");
    assert_eq!(err("NPAR TESTS M-W=a g(1,2).").message, "unexpected '('");
    assert_eq!(
        err("NPAR TESTS BINOMIAL(1.5)=a.").message,
        "the test proportion must lie between 0 and 1, got 1.5"
    );
    assert_eq!(
        err("NPAR TESTS K-S(NORMAL 3)=a.").expected,
        ["a second parameter"]
    );
    assert_eq!(
        err("NPAR TESTS K-S(NORMAL 0 -1)=a.").message,
        "the standard deviation must be positive"
    );
    assert_eq!(err("NPAR TESTS K-S(POISSON)a.").expected, ["'='"]);
    assert_eq!(
        err("NPAR TESTS M-W=a BY g(1,2) /MISSING=LISTWISE /MISSING=INCLUDE.").message,
        "MISSING may be given only once in NPAR TESTS"
    );
    assert_eq!(
        err("NPAR TESTS M-W=a BY g(1,2) /FOO.").message,
        "unknown subcommand FOO of NPAR TESTS"
    );
    assert_eq!(err("NPAR TESTS RUNS=a.").message, "unexpected '='");
}

fn partial(source: &str) -> PartialCorr {
    match one(source) {
        Command::PartialCorr(command) => command,
        other => panic!("{other:?}"),
    }
}

#[test]
fn partial_corr() {
    let c = partial("PARTIAL CORR VARIABLES=PUBTRANS MECHANIC BY NETPURSE(1).");
    assert_eq!(
        c.lists,
        [PartialList {
            rows: vec![var("PUBTRANS"), var("MECHANIC")],
            columns: None,
            controls: vec![var("NETPURSE")],
            orders: vec![1],
        }]
    );
    assert!(!c.one_tailed && !c.pairwise && !c.include && c.format == PartialFormat::Matrix);
    let c = partial(
        "PARTIAL CORR RENT FOOD WITH TEACHER BY NETSALRY, NETPRICE (1,2) / a b BY c \
         /SIG=ONETAIL /STAT=CORR DESCRIPTIVES /FORMAT=CONDENSED SERIAL /MISSING=ANALYSIS INCLUDE.",
    );
    assert_eq!(c.lists.len(), 2);
    assert_eq!(c.lists[0].columns, Some(vec![var("TEACHER")]));
    assert_eq!(c.lists[0].orders, [1, 2]);
    assert!(c.lists[1].orders.is_empty());
    assert!(c.one_tailed && c.zero_order && c.descriptives && !c.bad_corr);
    assert_eq!(c.format, PartialFormat::Serial);
    assert!(c.pairwise && c.include);
    let c = partial("PARTIAL CORR /VARIABLES=a b BY c /STATISTICS=ALL /STATISTICS=NONE BADCORR.");
    assert!(!c.zero_order && !c.descriptives && c.bad_corr);
}

#[test]
fn partial_corr_errors() {
    assert_eq!(err("PARTIAL CORR VARIABLES=a b.").expected, ["WITH", "BY"]);
    assert_eq!(
        err("PARTIAL CORR VARIABLES=a b BY c (0).").message,
        "an order is a whole number from 1 to 100, got 0"
    );
    assert_eq!(
        err("PARTIAL CORR VARIABLES=a b BY c /MATRIX=OUT(*).").message,
        "subcommand MATRIX of PARTIAL CORR is not supported yet"
    );
    assert_eq!(
        err("PARTIAL CORR /SIGNIFICANCE=ONETAIL.").expected,
        ["VARIABLES=varlist BY controls"]
    );
}
