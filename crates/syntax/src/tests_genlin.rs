//! Parser tests of GENLIN.

use super::*;

fn genlin(source: &str) -> Genlin {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    match parsed.remove(0).command {
        Command::Genlin(g) => *g,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn genlin_alone_fits_a_normal_intercept_model_with_the_default_output() {
    let g = genlin("GENLIN weight.");
    assert_eq!(
        g.response,
        GenlinResponse::Dependent {
            name: "weight".into(),
            reference: GenlinReference::Last,
            order: GenlinOrder::Ascending
        }
    );
    assert!(g.effects.is_empty() && g.intercept);
    assert_eq!(g.distribution, GenlinDistribution::Normal);
    assert_eq!(g.link, GenlinLink::Identity);
    assert_eq!(g.criteria, GenlinCriteria::default());
    assert_eq!(g.criteria.method, GenlinMethod::Fisher(1));
    assert_eq!(g.print, GenlinPrint::default());
    assert!(g.print.solution && !g.print.exponentiated && !g.print.covb);
}

#[test]
fn genlin_response_options_factors_and_covariates() {
    let g = genlin(
        "GENLIN passed (REFERENCE=FIRST) BY school class (ORDER=DESCENDING) WITH hours
  /MODEL school class hours school*hours DISTRIBUTION=BINOMIAL LINK=PROBIT INTERCEPT=NO.",
    );
    assert!(matches!(
        g.response,
        GenlinResponse::Dependent {
            reference: GenlinReference::First,
            ..
        }
    ));
    assert_eq!(g.factors, ["school", "class"]);
    assert_eq!(g.factor_order, GenlinOrder::Descending);
    assert_eq!(g.covariates, ["hours"]);
    assert_eq!(g.effects.len(), 4);
    assert_eq!(g.effects[3].label, "school*hours");
    assert!(!g.intercept);
    assert_eq!(g.link, GenlinLink::Probit);
    let g = genlin("GENLIN level (ORDER=DATA) (REFERENCE='low') /MODEL DISTRIBUTION=MULTINOMIAL.");
    assert!(matches!(
        g.response,
        GenlinResponse::Dependent {
            order: GenlinOrder::Data,
            reference: GenlinReference::Value(Value::String(_)),
            ..
        }
    ));
    assert_eq!(g.link, GenlinLink::CumLogit);
}

#[test]
fn genlin_events_over_trials_default_to_the_binomial_logit() {
    let g = genlin("GENLIN hits OF tries BY coach.");
    assert_eq!(
        g.response,
        GenlinResponse::Events {
            events: "hits".into(),
            trials: GenlinTrials::Variable("tries".into())
        }
    );
    assert_eq!(g.distribution, GenlinDistribution::Binomial);
    assert_eq!(g.link, GenlinLink::Logit);
    let g = genlin("GENLIN hits OF 10 /MODEL LINK=CLOGLOG.");
    assert!(matches!(
        g.response,
        GenlinResponse::Events {
            trials: GenlinTrials::Number(n),
            ..
        } if n == 10.0
    ));
    assert!(err("GENLIN hits OF 2.5.").contains("whole number"));
    assert!(err("GENLIN hits OF tries /MODEL DISTRIBUTION=POISSON.").contains("BINOMIAL"));
}

#[test]
fn genlin_default_links_and_allowed_pairs() {
    for (distribution, link) in [
        ("GAMMA", GenlinLink::Power(-1.0)),
        ("IGAUSS", GenlinLink::Power(-2.0)),
        ("POISSON", GenlinLink::Log),
        ("NEGBIN", GenlinLink::Log),
        ("MULTINOMIAL", GenlinLink::CumLogit),
    ] {
        let g = genlin(&format!("GENLIN y /MODEL DISTRIBUTION={distribution}."));
        assert_eq!(g.link, link, "{distribution}");
    }
    let g = genlin("GENLIN y /MODEL DISTRIBUTION=NEGBIN(MLE) LINK=NEGBIN.");
    assert_eq!(g.distribution, GenlinDistribution::NegativeBinomial(None));
    let g = genlin("GENLIN y /MODEL DISTRIBUTION=NEGBIN(0.5) LINK=POWER(0.5).");
    assert_eq!(
        g.distribution,
        GenlinDistribution::NegativeBinomial(Some(0.5))
    );
    assert_eq!(g.link, GenlinLink::Power(0.5));
    assert!(err("GENLIN y /MODEL DISTRIBUTION=NEGBIN(0) LINK=NEGBIN.").contains("cannot be used"));
    assert!(
        err("GENLIN y /MODEL LINK=LOGIT.")
            .contains("LINK=LOGIT cannot be used with DISTRIBUTION=NORMAL")
    );
    assert!(err("GENLIN y /MODEL DISTRIBUTION=MULTINOMIAL LINK=LOGIT.").contains("cannot be used"));
    assert!(err("GENLIN y /MODEL DISTRIBUTION=BINOMIAL LINK=CUMLOGIT.").contains("cannot be used"));
    let g = genlin("GENLIN y /MODEL DISTRIBUTION=BINOMIAL LINK=ODDSPOWER(0.5).");
    assert_eq!(g.link, GenlinLink::OddsPower(0.5));
    assert!(err("GENLIN y /MODEL DISTRIBUTION=TWEEDIE(1.5).").contains("not supported yet"));
}

#[test]
fn genlin_model_offset_scale_weight_and_order_of_parts() {
    let g = genlin(
        "GENLIN y BY a WITH x /MODEL a x a*x OFFSET=logt SCALEWEIGHT=w DISTRIBUTION=POISSON.",
    );
    assert_eq!(g.offset, Some(GenlinOffset::Variable("logt".into())));
    assert_eq!(g.scale_weight.as_deref(), Some("w"));
    let g = genlin("GENLIN y /MODEL OFFSET=-1.5.");
    assert_eq!(g.offset, Some(GenlinOffset::Value(-1.5)));
    assert!(
        err("GENLIN y BY a /MODEL DISTRIBUTION=POISSON a.")
            .contains("effects of MODEL come before")
    );
    assert!(err("GENLIN y /MODEL INTERCEPT=NO INTERCEPT=YES.").contains("only once"));
}

#[test]
fn genlin_criteria() {
    let g = genlin(
        "GENLIN y /CRITERIA ANALYSISTYPE=ALL(LR) CHECKSEP=0 CILEVEL=90 COVB=ROBUST
   HCONVERGE=1E-5(RELATIVE) LCONVERGE=1E-8 PCONVERGE=0 LIKELIHOOD=KERNEL MAXITERATIONS=50
   MAXSTEPHALVING=8 METHOD=NEWTON SCALE=DEVIANCE SINGULAR=1E-10.",
    );
    let c = g.criteria;
    assert!(c.type1 && c.type3 && c.lr && c.robust && c.kernel);
    assert_eq!(c.checksep, 0);
    assert_eq!(c.cilevel, 90.0);
    assert_eq!(
        c.hconverge,
        GenlinTolerance {
            value: 1e-5,
            relative: true
        }
    );
    assert_eq!(c.lconverge.value, 1e-8);
    assert_eq!(c.pconverge.value, 0.0);
    assert_eq!((c.maxiterations, c.maxstephalving), (50, 8));
    assert_eq!(c.method, GenlinMethod::Newton);
    assert_eq!(c.scale, Some(GenlinScale::Deviance));
    assert_eq!(c.singular, 1e-10);
    let g = genlin("GENLIN y /CRITERIA ANALYSISTYPE=1 METHOD=FISHER(3) SCALE=2.5.");
    assert!(g.criteria.type1 && !g.criteria.type3 && !g.criteria.lr);
    assert_eq!(g.criteria.method, GenlinMethod::Fisher(3));
    assert_eq!(g.criteria.scale, Some(GenlinScale::Fixed(2.5)));
    assert_eq!(
        genlin("GENLIN y /CRITERIA METHOD=FISHER.").criteria.method,
        GenlinMethod::FisherOnly
    );
    assert!(err("GENLIN y /CRITERIA PCONVERGE=0.").contains("at least one"));
    assert!(err("GENLIN y /CRITERIA CILEVEL=100.").contains("below 100"));
    assert!(err("GENLIN y /MODEL DISTRIBUTION=POISSON /CRITERIA SCALE=MLE.").contains("SCALE=MLE"));
    assert!(err("GENLIN y /CRITERIA CITYPE=PROFILE.").contains("not supported yet"));
    assert!(err("GENLIN y /CRITERIA INITIAL=1 2.").contains("not supported yet"));
}

#[test]
fn genlin_keywords_and_subcommands_are_written_in_full() {
    assert!(err("GENLIN y /MOD DISTRIBUTION=POISSON.").contains("unexpected"));
    assert!(err("GENLIN y /MODEL DISTRIBUTION=POIS.").contains("unexpected"));
    assert!(err("GENLIN y /CRITERIA MAXITER=5.").contains("unexpected"));
    assert!(err("GENLIN y /PRINT FIT /PRINT SOLUTION.").contains("only once"));
    assert!(err("GENLIN y BY a TO c.").contains("TO"));
}

#[test]
fn genlin_emmeans_rules() {
    let g = genlin(
        "GENLIN y BY a b WITH x /MODEL a b x
  /EMMEANS TABLES=a*b CONTROL=x(2.5) SCALE=TRANSFORMED COMPARE=a CONTRAST=SIMPLE(1) PADJUST=SIDAK
  /EMMEANS
  /EMMEANS TABLES=a BY b COMPARE=a*b.",
    );
    assert_eq!(g.emmeans.len(), 3);
    let e = &g.emmeans[0];
    assert_eq!(e.tables, ["a", "b"]);
    assert_eq!(e.control, [("x".to_string(), Some(2.5))]);
    assert!(e.transformed);
    assert_eq!(
        e.contrast,
        Some(GenlinContrast::Simple(Some(Value::Number(1.0))))
    );
    assert_eq!(e.padjust, GenlinAdjust::Sidak);
    assert!(g.emmeans[1].tables.is_empty());
    assert_eq!(
        g.emmeans[2].compare.as_deref(),
        Some(&["a".to_string(), "b".to_string()][..])
    );
    assert!(err("GENLIN y BY a /EMMEANS COMPARE=a.").contains("COMPARE needs TABLES"));
    assert!(err("GENLIN y BY a b /EMMEANS TABLES=a COMPARE=b.").contains("not on TABLES"));
    assert!(
        err("GENLIN y BY a /EMMEANS TABLES=a CONTRAST=HELMERT.").contains("CONTRAST needs COMPARE")
    );
    assert!(
        err("GENLIN y BY a b /EMMEANS TABLES=a*b COMPARE=a*b CONTRAST=HELMERT.")
            .contains("PAIRWISE")
    );
    let g = genlin("GENLIN y BY a /EMMEANS TABLES=a COMPARE=a CONTRAST=POLYNOMIAL(1 2 4).");
    assert_eq!(
        g.emmeans[0].contrast,
        Some(GenlinContrast::Polynomial(Some(vec![1.0, 2.0, 4.0])))
    );
    assert!(
        err("GENLIN y BY a /EMMEANS TABLES=a COMPARE=a CONTRAST=POLYNOMIAL(1 1 2).")
            .contains("different")
    );
}

#[test]
fn genlin_print_save_missing_and_unsupported_parts() {
    let g = genlin(
        "GENLIN y BY a /MISSING CLASSMISSING=INCLUDE /PRINT SOLUTION(EXPONENTIATED) COVB WORKINGCORR
  /SAVE XBPRED(eta) MEANPRED(:3) CIMEANPREDL(low:2) RESID COOK(cd).",
    );
    assert!(g.include);
    assert!(
        g.print.solution && g.print.exponentiated && g.print.covb && !g.print.fit && !g.print.cps
    );
    assert_eq!(g.save.len(), 5);
    assert_eq!(g.save[0].name.as_deref(), Some("eta"));
    assert_eq!((g.save[1].name.as_deref(), g.save[1].count), (None, 3));
    assert_eq!(
        (g.save[2].name.as_deref(), g.save[2].count),
        (Some("low"), 2)
    );
    assert_eq!(g.save[4].kind, GenlinSaveKind::Cook);
    let g = genlin("GENLIN y /PRINT NONE.");
    assert!(!g.print.solution && !g.print.fit && !g.print.modelinfo);
    assert!(err("GENLIN y /PRINT NONE FIT.").contains("NONE"));
    assert!(err("GENLIN y /PRINT LMATRIX.").contains("not supported yet"));
    assert!(err("GENLIN y /SAVE RESID RESID.").contains("only once"));
    assert!(
        err("GENLIN y /REPEATED SUBJECT=id.").contains("REPEATED of GENLIN is not supported yet")
    );
    assert!(err("GENLIN y /OUTFILE COVB='b.sav'.").contains("not supported yet"));
}
