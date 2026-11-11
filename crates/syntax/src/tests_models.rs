//! Parser tests of REGRESSION and RELIABILITY.

use super::*;

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

fn regression(source: &str) -> Regression {
    match one(source) {
        Command::Regression(r) => *r,
        other => panic!("{other:?}"),
    }
}

fn reliability(source: &str) -> Reliability {
    match one(source) {
        Command::Reliability(r) => *r,
        other => panic!("{other:?}"),
    }
}

#[test]
fn regression_defaults_and_the_pasted_dialog_syntax() {
    let r = regression("REGRESSION /DEPENDENT=y /METHOD=ENTER a b.");
    assert_eq!(r.variables, None);
    assert_eq!(r.statistics, RegressionStatistics::DEFAULTS);
    assert_eq!(r.criteria, RegressionCriteria::default());
    assert_eq!(r.missing, RegressionMissing::Listwise);
    assert_eq!(
        r.methods,
        [RegressionMethod {
            kind: MethodKind::Enter,
            variables: vec![var("a"), var("b")]
        }]
    );
    let r = regression(
        "REGRESSION\n  /DESCRIPTIVES MEAN STDDEV CORR SIG N\n  /MISSING LISTWISE\n  \
         /STATISTICS COEFF OUTS CI(99) R ANOVA COLLIN TOL CHANGE ZPP\n  /CRITERIA=PIN(.05) POUT(.10)\n  \
         /NOORIGIN\n  /DEPENDENT y\n  /METHOD=ENTER a b\n  /SCATTERPLOT=(*ZRESID ,*ZPRED)\n  \
         /RESIDUALS DURBIN HISTOGRAM(ZRESID) NORMPROB(ZRESID)\n  /CASEWISE PLOT(ZRESID) OUTLIERS(3)\n  \
         /SAVE PRED ZPRED COOK LEVER.",
    );
    let s = r.statistics;
    assert!(s.coeff && s.outs && s.r && s.anova && s.collin && s.tol && s.change && s.zpp);
    assert_eq!(s.ci, Some(99.0));
    assert!(!s.bcov && !s.f);
    let d = r.descriptives.unwrap();
    assert!(d.mean && d.stddev && d.corr && d.sig && d.n && !d.cov);
    assert!(r.residuals.as_ref().unwrap().durbin);
    assert_eq!(r.casewise.as_ref().unwrap().limit, 3.0);
    assert_eq!(
        r.save,
        [
            (TempVar::Pred, None),
            (TempVar::Zpred, None),
            (TempVar::Cook, None),
            (TempVar::Lever, None)
        ]
    );
    assert_eq!(
        r.charts,
        ["SCATTERPLOT", "RESIDUALS HISTOGRAM", "RESIDUALS NORMPROB"]
    );
}

#[test]
fn regression_abbreviations_and_methods_without_the_method_keyword() {
    let r = regression(
        "REGR VAR=y a b c /STAT=DEF BCOV /CRIT=FIN(4) FOUT TOL(.01) MAXSTEPS(5) CIN(90) \
         /ORIG /DEP=y /ENT a /STEP b c /BACK /MISS=PAIR INCL /REGWGT=w.",
    );
    assert_eq!(
        r.variables,
        Some(vec![var("y"), var("a"), var("b"), var("c")])
    );
    assert!(r.statistics.bcov && r.statistics.r);
    assert_eq!(r.criteria.entry, Criterion::F(4.0));
    assert_eq!(r.criteria.removal, Criterion::F(2.71));
    assert_eq!(r.criteria.tolerance, 0.01);
    assert_eq!(r.criteria.max_steps, Some(5));
    assert_eq!(r.criteria.cin, 90.0);
    assert!(r.origin && r.include);
    assert_eq!(r.missing, RegressionMissing::Pairwise);
    assert_eq!(r.regwgt.as_deref(), Some("w"));
    let kinds: Vec<MethodKind> = r.methods.iter().map(|m| m.kind).collect();
    assert_eq!(
        kinds,
        [
            MethodKind::Enter,
            MethodKind::Stepwise,
            MethodKind::Backward
        ]
    );
    assert!(r.methods[2].variables.is_empty());
    // several methods in one METHOD subcommand
    let r = regression("REGRESSION /DEPENDENT=y /METHOD=ENTER a FORWARD b c.");
    assert_eq!(r.methods.len(), 2);
    assert_eq!(r.methods[1].variables, [var("b"), var("c")]);
}

#[test]
fn regression_statistics_all_and_save_names() {
    let r = regression(
        "REGRESSION /STATISTICS=ALL /DEPENDENT=y /METHOD=ENTER a \
         /SAVE=PRED(p) DFBETA(d) FITS /CASEWISE=ALL SRE MAH COOK SDR /RESID=DEFAULT ID(id).",
    );
    let s = r.statistics;
    assert!(s.selection && s.xtx && s.ses && s.label && s.bcov && !s.f);
    assert_eq!(s.ci, Some(95.0));
    assert_eq!(r.save[0], (TempVar::Pred, Some("p".to_string())));
    assert_eq!(r.save[1], (TempVar::Dfbeta, Some("d".to_string())));
    assert!(r.save.contains(&(TempVar::Covratio, None)));
    let c = r.casewise.unwrap();
    assert!(c.all && !c.dependent);
    assert_eq!(
        c.listed,
        [
            TempVar::Sresid,
            TempVar::Mahal,
            TempVar::Cook,
            TempVar::Sdresid
        ]
    );
    let residuals = r.residuals.unwrap();
    assert_eq!(residuals.outliers, [TempVar::Zresid]);
    assert_eq!(residuals.id.as_deref(), Some("id"));
}

#[test]
fn regression_errors() {
    assert_eq!(
        err("REGRESSION /DEPENDENT=y /METHOD=TEST(a)(b).").message,
        "TEST in REGRESSION /METHOD is not supported yet"
    );
    assert_eq!(
        err("REGRESSION /MATRIX=IN(*) /DEPENDENT=y /METHOD=ENTER a.").message,
        "subcommand MATRIX of REGRESSION is not supported yet"
    );
    assert!(
        err("REGRESSION /METHOD=ENTER a.")
            .expected
            .contains(&"/DEPENDENT=varlist".to_string())
    );
    assert!(
        err("REGRESSION /DEPENDENT=y.")
            .expected
            .contains(&"/METHOD=ENTER varlist".to_string())
    );
    assert_eq!(
        err("REGRESSION /DEPENDENT=y /DEPENDENT=z /METHOD=ENTER a.").message,
        "DEPENDENT can be given only once"
    );
    assert_eq!(
        err("REGRESSION /DEPENDENT=y /METHOD=ENTER a /SAVE=MCIN(m).").message,
        "MCIN and ICIN save two variables each and take no name"
    );
    assert_eq!(
        err("REGRESSION /DEPENDENT=y /METHOD=ENTER a /CASEWISE=PLOT(COOK).").message,
        "COOK cannot be used in CASEWISE PLOT"
    );
    assert!(
        err("REGRESSION /DEPENDENT=y /METHOD=REMOVE.")
            .hint
            .is_some()
    );
    assert_eq!(
        err("REGRESSION /CRITERIA=PIN(2) /DEPENDENT=y /METHOD=ENTER a.").message,
        "a probability must lie above 0 and at most 1, got 2"
    );
}

#[test]
fn reliability_defaults_and_keywords() {
    let r = reliability("RELIABILITY /VARIABLES=a b c.");
    assert_eq!(r.variables, [var("a"), var("b"), var("c")]);
    assert_eq!(r.model, ReliabilityModel::Alpha);
    assert_eq!(r.scale, None);
    assert_eq!(r.anova, None);
    assert_eq!(r.summary, ReliabilitySummary::default());
    let r = reliability(
        "RELIABILITY\n  /VARIABLES=a b c d\n  /SCALE('ALL VARIABLES') ALL\n  /MODEL=SPLIT(2)\n  \
         /STATISTICS=DESCRIPTIVE SCALE CORR COV ANOVA FRIEDMAN TUKEY HOTELLING\n  \
         /SUMMARY=TOTAL MEANS VARIANCE COV CORR\n  /ICC=MODEL(RANDOM) TYPE(ABSOLUTE) CIN=90 TESTVAL=0.5\n  \
         /METHOD=COVARIANCE /MISSING=INCLUDE.",
    );
    assert_eq!(
        r.scale,
        Some(("ALL VARIABLES".to_string(), vec![VarItem::All]))
    );
    assert_eq!(r.model, ReliabilityModel::Split(Some(2)));
    assert!(r.descriptive && r.scale_statistics && r.correlations && r.covariances);
    assert!(r.tukey && r.hotelling && r.covariance_method && r.include);
    assert_eq!(r.anova, Some(AnovaTest::Friedman));
    assert_eq!(
        r.summary,
        ReliabilitySummary {
            means: true,
            variances: true,
            covariances: true,
            correlations: true,
            total: true
        }
    );
    assert_eq!(
        r.icc,
        Some(Icc {
            model: IccModel::Random,
            absolute: true,
            level: 90.0,
            test_value: 0.5
        })
    );
    // FRIEDMAN without ANOVA counts for nothing; statistics add up over subcommands
    let r = reliability("RELI VAR=a b /STAT=COCHRAN /STAT=DESC /SUMM=ALL /ICC.");
    assert_eq!(r.anova, None);
    assert!(r.descriptive && r.summary.total && r.summary.correlations);
    assert_eq!(r.icc.unwrap().model, IccModel::Mixed);
    let r = reliability("RELIABILITY /VARIABLES=a b /STATISTICS=ALL /MODEL=STRICTPARALLEL.");
    assert_eq!(r.anova, Some(AnovaTest::F));
    assert!(r.tukey && r.hotelling);
    assert_eq!(r.model, ReliabilityModel::StrictParallel);
}

#[test]
fn reliability_errors() {
    assert_eq!(
        err("RELIABILITY /VARIABLES=a b /MODEL=OMEGA.").message,
        "OMEGA in RELIABILITY /MODEL is not supported yet"
    );
    assert_eq!(
        err("RELIABILITY /VARIABLES=a b /MATRIX=OUT(*).").message,
        "subcommand MATRIX of RELIABILITY is not supported yet"
    );
    assert_eq!(
        err("RELIABILITY /VARIABLES=a b /SCALE(x) a /SCALE(y) b.").message,
        "SCALE can be given only once"
    );
    assert_eq!(
        err("RELIABILITY /VARIABLES=a b /ICC=CIN=100.").message,
        "CIN must lie between 0 and 100, got 100"
    );
    assert_eq!(
        err("RELIABILITY /VARIABLES=a b /MODEL=SPLIT(1.5).").message,
        "SPLIT takes a positive whole number of items, got 1.5"
    );
}
