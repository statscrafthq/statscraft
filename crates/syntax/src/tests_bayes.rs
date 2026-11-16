//! Parser tests of the BAYES commands.

use super::*;

fn bayes(source: &str) -> Bayes {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    match parsed.remove(0).command {
        Command::Bayes(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    format!("{:?}", errors[0])
}

#[test]
fn onesample_normal_defaults_and_data() {
    let Bayes::OneSample(c) = bayes(
        "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=pace lap ANALYSIS=BOTH\n\
         /DATA VARIABLES=lap VARIANCE=4 NULLVALUE=12 ALTERGVALUE=0.5.",
    ) else {
        panic!()
    };
    assert_eq!(c.distribution, BayesDistribution::Normal);
    assert_eq!(c.analysis, BayesAnalysis::Both);
    assert_eq!(c.variables.len(), 2);
    assert!(!c.listwise);
    assert_eq!(c.criteria.ci, 0.95);
    assert_eq!(c.criteria.tol, 1e-6);
    assert_eq!(c.prior.variance, VariancePrior::Diffuse);
    assert_eq!(c.prior.kappa, 1.0);
    let d = &c.data[0];
    assert_eq!(
        (d.test.variance, d.test.null, d.test.g),
        (Some(4.0), 12.0, Some(0.5))
    );
}

#[test]
fn abbreviations_and_priors() {
    let Bayes::OneSample(c) = bayes(
        "BAYES ONESAMPLE /INF DIS=NOR VARIABLES=pace ANA=BAYESF /MIS SCO=LIS\n\
         /CRI CIL=90 MET=MCA SEE=17 MCS=5000 /PRI PRE=GAMMA(2 3) MEA=NORMAL(5) KAP=2.",
    ) else {
        panic!()
    };
    assert!(c.listwise);
    assert_eq!(c.analysis, BayesAnalysis::BayesFactor);
    assert!((c.criteria.ci - 0.9).abs() < 1e-12);
    assert!(c.criteria.monte_carlo);
    assert_eq!((c.criteria.seed, c.criteria.mcsamples), (Some(17), 5000));
    assert_eq!(c.prior.variance, VariancePrior::Gamma(2.0, 3.0));
    assert_eq!(c.prior.mean, Some((5.0, None)));
    assert_eq!(c.prior.kappa, 2.0);
}

#[test]
fn binomial_success_and_priors() {
    let Bayes::OneSample(c) = bayes(
        "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=BINOMIAL VARIABLES=answer\n\
         /DATA VARIABLES=answer SUCCESS=LEVEL(yes 'maybe') NULLPRIORPARAM=1 1 ALTERPRIORPARAM=3 3\n\
         /PRIOR TYPE=CONJUGATE(0 0).",
    ) else {
        panic!()
    };
    let d = &c.data[0];
    assert_eq!(
        d.success,
        Some(Success::Level(vec![
            Value::String("yes".into()),
            Value::String("maybe".into())
        ]))
    );
    assert_eq!(
        (d.null_prior, d.alter_prior),
        (Some((1.0, 1.0)), Some((3.0, 3.0)))
    );
    assert_eq!(c.prior.conjugate, Some((0.0, 0.0)));
}

#[test]
fn onesample_errors() {
    for (source, expected) in [
        ("BAYES ONESAMPLE /INFERENCE VARIABLES=x.", "DISTRIBUTION"),
        (
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL.",
            "VARIABLES",
        ),
        (
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=x /PRIOR VARDIST=JEF.",
            "could mean",
        ),
        (
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=x /PRIOR VARDIST=DIFFUSE PRECDIST=CHISQ(2).",
            "not both",
        ),
        (
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=POISSON VARIABLES=x /DATA VARIABLES=x SUCCESS=LAST.",
            "does not apply",
        ),
        (
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=POISSON VARIABLES=x /DATA VARIABLES=x ALTERPRIORPARAM=1 0.",
            "both above 0",
        ),
        (
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=BINOMIAL VARIABLES=x /PRIOR TYPE=CONJUGATE(0 1).",
            "or both 0",
        ),
        (
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=x /CRITERIA TOL=0.01.",
            "at most 0.001",
        ),
        (
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=x /CRITERIA MCSAMPLES=10.",
            "from 1000",
        ),
        (
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=x /DATA VARIANCE=1.",
            "VARIABLES",
        ),
    ] {
        let e = err(source);
        assert!(e.contains(expected), "{source}: {e}");
    }
}

#[test]
fn related_pairs() {
    let Bayes::Related(c) = bayes(
        "BAYES RELATED /PAIR VARIABLES=before after NULLVALUE=-1 /PAIR VARIABLES=left right VARIANCE=2\n\
         /INFERENCE DISTRIBUTION=NORMAL ANALYSIS=BOTH /PRIOR VARDIST=JEFFREYS4.",
    ) else {
        panic!()
    };
    assert_eq!(c.pairs.len(), 2);
    assert_eq!(
        (c.pairs[0].first.as_str(), c.pairs[0].test.null),
        ("before", -1.0)
    );
    assert_eq!(c.pairs[1].test.variance, Some(2.0));
    assert_eq!(c.prior.variance, VariancePrior::Jeffreys4);
    assert!(err("BAYES RELATED /INFERENCE ANALYSIS=BOTH.").contains("PAIR"));
    assert!(err("BAYES RELATED /PAIR VARIABLES=a b c.").contains("two variable names"));
}

#[test]
fn independent_settings() {
    let Bayes::Independent(c) = bayes(
        "BAYES INDEPENDENT /INFERENCE VARIABLES=score GROUP=site SELECT=LEVEL(north south) DATAVAR=1 2\n\
         /ESTBF COMPUTATION=GONEN(0.3 2) /PRIOR EQUALDATAVAR=TRUE VARDIST=INVCHISQ(3 1)\n\
         MEANDIST=NORMAL(1 2 3 4).",
    ) else {
        panic!()
    };
    assert_eq!(c.group, "site");
    assert_eq!(
        c.select,
        GroupSelect::Levels(Value::String("north".into()), Value::String("south".into()))
    );
    assert_eq!(c.datavar, Some((1.0, 2.0)));
    assert_eq!(c.factor, IndependentFactor::Gonen(0.3, 2.0));
    assert!(c.equal_variances);
    assert_eq!(c.variance, GroupVariancePrior::InvChiSquare(3.0, 1.0));
    assert_eq!(c.mean, Some([1.0, 2.0, 3.0, 4.0]));
    let Bayes::Independent(c) = bayes(
        "BAYES INDEPENDENT /INFERENCE VARIABLES=score GROUP=age SELECT=CUTPOINT(40) /ESTBF COMPUTATION=HYPER.",
    ) else {
        panic!()
    };
    assert_eq!(c.select, GroupSelect::Cutpoint(40.0));
    assert_eq!(c.factor, IndependentFactor::Hyper(-0.75));
    let Bayes::Independent(c) = bayes(
        "BAYES INDEPENDENT /INFERENCE VARIABLES=score GROUP=age SELECT=MIDPOINT /ESTBF COMPUTATION=GONEN(2).",
    ) else {
        panic!()
    };
    assert_eq!(c.factor, IndependentFactor::Gonen(0.0, 2.0));
    for (source, expected) in [
        (
            "BAYES INDEPENDENT /INFERENCE VARIABLES=s SELECT=MIDPOINT.",
            "GROUP",
        ),
        (
            "BAYES INDEPENDENT /INFERENCE VARIABLES=s GROUP=g.",
            "SELECT",
        ),
        (
            "BAYES INDEPENDENT /INFERENCE VARIABLES=s GROUP=g SELECT=MIDPOINT /ESTBF COMPUTATION=HYPER(-0.3).",
            "at most -0.5",
        ),
        (
            "BAYES INDEPENDENT /INFERENCE VARIABLES=s GROUP=g SELECT=LEVEL(1 2 3).",
            "two values",
        ),
        (
            "BAYES INDEPENDENT /INFERENCE VARIABLES=s GROUP=g SELECT=MIDPOINT /CRITERIA METHOD=MCA.",
            "unexpected",
        ),
    ] {
        let e = err(source);
        assert!(e.contains(expected), "{source}: {e}");
    }
}

#[test]
fn correlation_settings() {
    let Bayes::Correlation(c) = bayes(
        "BAYES CORRELATION /MISSING SCOPE=LISTWISE /INFERENCE VARIABLES=a TO c CVALUE=-1.5 MAXPLOTS=0\n\
         /ESTBF TYPE=FRACTION(0.25) NULLVALUE=0.2 /CRITERIA POSTSAMPLES=2000.",
    ) else {
        panic!()
    };
    assert!(c.listwise);
    assert_eq!((c.c, c.maxplots), (-1.5, 0));
    assert_eq!((c.fraction, c.null), (Some(0.25), 0.2));
    assert_eq!(c.criteria.tol, 1e-4);
    assert_eq!(c.criteria.postsamples, 2000);
    let Bayes::Correlation(c) =
        bayes("BAYES CORRELATION /INFERENCE VARIABLES=a b /ESTBF TYPE=FRACTION.")
    else {
        panic!()
    };
    assert_eq!(c.fraction, Some(0.5));
    assert!(err("BAYES CORRELATION /ESTBF NULLVALUE=1.").contains("between -1 and 1"));
    assert!(
        err("BAYES CORRELATION /INFERENCE VARIABLES=a b MAXPLOTS=51.").contains("from 0 to 50")
    );
}
