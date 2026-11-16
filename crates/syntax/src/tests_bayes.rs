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

#[test]
fn anova_settings_and_abbreviations() {
    let Bayes::Anova(c) = bayes(
        "BAYES ANOVA yield BY plot /CRI CIL=90 TOL=0.0001 MAXITER=300 /DES REG=area\n\
         /INF ANA=BOTH /EST COMP=HYPER(4) /PRI TYPE=CONJ SHA=2 SCA=1.5 MEANV=3 4 5\n\
         VMAT=2; 0.5 3; 0 0 1 /PLOT MEANS=1 'b' c ERR=TRUE.",
    ) else {
        panic!()
    };
    assert_eq!((c.dependent.as_str(), c.factor.as_str()), ("yield", "plot"));
    assert_eq!(c.regwgt.as_deref(), Some("area"));
    assert_eq!(c.analysis, BayesAnalysis::Both);
    assert_eq!(c.g, GPrior::Hyper(4.0));
    assert!(c.prior.conjugate);
    assert_eq!((c.prior.shape, c.prior.scale), (Some(2.0), Some(1.5)));
    assert_eq!(c.prior.means, [3.0, 4.0, 5.0]);
    assert_eq!(
        c.prior.vmatrix,
        vec![vec![2.0], vec![0.5, 3.0], vec![0.0, 0.0, 1.0]]
    );
    assert_eq!(c.plot_means.len(), 3);
    assert!(c.plot_error);
    assert_eq!(c.criteria.maxiter, 300);
    let Bayes::Anova(c) = bayes("BAYES ANOVA yield BY plot.") else {
        panic!()
    };
    assert_eq!((c.g, c.criteria.maxiter), (GPrior::Jzs, 2000));
}

#[test]
fn anova_errors() {
    assert!(err("BAYES ANOVA yield plot.").contains("BY"));
    assert!(err("BAYES ANOVA yield BY plot /ESTBF COMPUTATION=ZELLNER.").contains("value of g"));
    assert!(err("BAYES ANOVA yield BY plot /ESTBF COMPUTATION=HYPER(2).").contains("above 2"));
    assert!(
        err("BAYES ANOVA yield BY plot /PRIOR TYPE=CONJUGATE SHAPEPARAM=1.").contains("MEANVECTOR")
    );
    assert!(err("BAYES ANOVA yield BY plot /PRIOR VMATRIX=1 2; 3.").contains("row 1"));
    assert!(err("BAYES ANOVA yield BY plot /PRIOR VMATRIX=1; 2 0.").contains("row 2"));
}

#[test]
fn regression_subcommands() {
    let Bayes::Regression(c) = bayes(
        "BAYES REGRESSION price BY region type WITH size age\n\
         /DESIGN FACTORS=region COVARIATES=size /ESTBF COMPUTATION=ZELLNER(3) COMPARE=FULL\n\
         /PREDICT REGRESSORS=north 120 /SAVE MEANPRED(pm) CIMEANPREDL MODE\n\
         /OUTFILE PARAMETER='coef.xml' /PLOT FACTORS=region MAX CATEGORY=3 BAYESPRED=TRUE\n\
         /FTEST LABEL='size only' VARIABLES=size TESTVALUES=0.5 /FTEST INTERCEPT=TRUE.",
    ) else {
        panic!()
    };
    assert_eq!((c.factors.len(), c.covariates.len()), (2, 2));
    assert_eq!(c.design_factors.as_ref().map(Vec::len), Some(1));
    assert_eq!(c.g, GPrior::Zellner(3.0));
    assert!(c.compare_full);
    assert_eq!(c.predict.len(), 2);
    assert_eq!(
        c.save,
        vec![
            (PredictStat::Mean, Some("pm".into())),
            (PredictStat::Lower, None),
            (PredictStat::Mode, None)
        ]
    );
    assert_eq!(c.outfile.as_deref(), Some("coef.xml"));
    assert_eq!(c.plot.max_category, 3);
    assert!(c.plot.predictive);
    assert_eq!(c.ftests.len(), 2);
    assert_eq!(c.ftests[0].label.as_deref(), Some("size only"));
    assert_eq!(c.ftests[0].values, [0.5]);
    assert!(c.ftests[1].intercept && c.ftests[1].variables.is_empty());
    let Bayes::Regression(c) = bayes("BAYES REGRESSION price WITH size /PLOT MAX_CATEGORY=4.")
    else {
        panic!()
    };
    assert!(c.factors.is_empty() && c.plot.max_category == 4);
}

#[test]
fn regression_errors() {
    assert!(err("BAYES REGRESSION price.").contains("BY"));
    assert!(err("BAYES REGRESSION price WITH size /FTEST VARIABLES=").contains("variable"));
    assert!(err("BAYES REGRESSION price WITH size /FTEST LABEL='x'.").contains("INTERCEPT=TRUE"));
    assert!(err("BAYES REGRESSION price WITH size /PLOT BAYESPRED=TRUE.").contains("PREDICT"));
    assert!(err("BAYES REGRESSION price WITH size /SAVE MEDIAN.").contains("MEANPRED"));
}

#[test]
fn repeated_settings() {
    let Bayes::Repeated(c) = bayes(
        "BAYES REPEATED w1 TO w4 /CRITERIA SEED=12 MCS=5000 /ESTBF COMPUTATION=ROUDER\n\
         /DESIGN REGWGT=trust /PLOT MEANS=w1 w2 /INFERENCE ANALYSIS=BAYESFACTOR.",
    ) else {
        panic!()
    };
    assert_eq!(c.variables.len(), 1);
    assert!(c.rouder);
    assert_eq!((c.criteria.seed, c.criteria.mcsamples), (Some(12), 5000));
    assert_eq!(c.plot_means.len(), 2);
    let Bayes::Repeated(c) = bayes("BAYES REPEATED a b.") else {
        panic!()
    };
    assert!(!c.rouder && c.criteria.mcsamples == 30_000);
    assert!(err("BAYES REPEATED a b /CRITERIA TOL=0.001.").contains("SEED"));
}

#[test]
fn loglinear_defaults() {
    let Bayes::Loglinear(c) = bayes("BAYES LOGLINEAR /TABLE route BY class.") else {
        panic!()
    };
    assert_eq!((c.row.as_str(), c.column.as_str()), ("route", "class"));
    assert_eq!(c.analysis, BayesAnalysis::Posterior);
    assert_eq!(
        (c.model, c.margin, c.prior),
        (
            CountModel::Multinomial,
            FixedMargin::Total,
            TablePrior::Conjugate
        )
    );
    assert_eq!(c.cells, vec![CrosstabsCell::Count]);
    assert!(c.chisq && !c.lrt && !c.suppress_table && !c.criteria.descending);
    assert_eq!((c.criteria.postsamples, c.criteria.seed), (10_000, None));
}

#[test]
fn loglinear_settings_and_spellings() {
    let Bayes::Loglinear(c) = bayes(
        "BAYES LOGLINEAR /CRITERIA CILEVEL=90 SEED=31 TOL=0.0001 MAXITER=300 POSTSAMPLES=2000\n\
         ORDER=DESCENDING /INFERENCE ANALYSIS=BOTH /TABLE route BY class\n\
         /ESTBF MODEL=POISSON FIXMARGINS=COLUMN PRIORTYPE=MIXTURE_DIRICHLET\n\
         SHAPEPARAMS=1 2; 3 4 SCALEPARAM=0.25\n\
         /PRINT SUPPRESSTABLE=TRUE CELLS=ALL STATISTICS=LRT.",
    ) else {
        panic!()
    };
    assert_eq!(c.criteria.ci, 0.9);
    assert_eq!((c.criteria.seed, c.criteria.maxiter), (Some(31), 300));
    assert!(c.criteria.descending && c.suppress_table);
    assert_eq!(
        (c.model, c.margin),
        (CountModel::Poisson, FixedMargin::Column)
    );
    assert_eq!(c.prior, TablePrior::MixtureDirichlet);
    assert_eq!(c.shapes, vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
    assert_eq!(c.scale, Some(0.25));
    assert_eq!(c.cells.len(), 5);
    assert!(!c.chisq && c.lrt);
    // two words, the underscore, abbreviations and a bare FIX MARGIN
    let Bayes::Loglinear(c) = bayes(
        "BAYES LOG /TAB a BY b /ESTBF MOD=NONPAR FIX MARGIN=ROW PRIOR TYPE=INTR LAMBDAS=2 0.5\n\
         /PRINT SUPPRESS_TABLE=FALSE CELLS=EXP ROW STATISTICS= /CRITERIA ORDER=TRUE.",
    ) else {
        panic!()
    };
    assert_eq!(
        (c.model, c.margin),
        (CountModel::Nonparametric, FixedMargin::Row)
    );
    assert_eq!(c.prior, TablePrior::Intrinsic);
    assert_eq!(c.lambdas, vec![2.0, 0.5]);
    assert_eq!(c.cells, vec![CrosstabsCell::Expected, CrosstabsCell::Row]);
    assert!(c.chisq && !c.criteria.descending);
    let Bayes::Loglinear(c) =
        bayes("BAYES LOGLINEAR /TABLE a BY b /ESTBF FIX_MARGIN= MODEL=POISSON.")
    else {
        panic!()
    };
    assert_eq!(c.margin, FixedMargin::Total);
}

#[test]
fn loglinear_errors() {
    assert!(err("BAYES LOGLINEAR /ESTBF MODEL=POISSON.").contains("TABLE"));
    assert!(err("BAYES LOGLINEAR /TABLE a BY a.").contains("two different"));
    assert!(err("BAYES LOGLINEAR /TABLE a b.").contains("BY"));
    assert!(
        err("BAYES LOGLINEAR /TABLE a BY b /ESTBF MODEL=NONPARAMETRIC.").contains("ROW or COLUMN")
    );
    assert!(
        err("BAYES LOGLINEAR /TABLE a BY b /ESTBF MODEL=NONPARAMETRIC FIX_MARGIN=TOTAL.")
            .contains("ROW or COLUMN")
    );
    assert!(err("BAYES LOGLINEAR /TABLE a BY b /ESTBF SHAPEPARAMS=1 0.").contains("above 0"));
    assert!(err("BAYES LOGLINEAR /TABLE a BY b /ESTBF LAMBDAS=1; 2.").contains("without ';'"));
    assert!(err("BAYES LOGLINEAR /TABLE a BY b /CRITERIA POSTSAMPLES=10.").contains("POSTSAMPLES"));
    assert!(err("BAYES LOGLINEAR /TABLE a BY b /CRITERIA MCSAMPLES=1000.").contains("POSTSAMPLES"));
}
