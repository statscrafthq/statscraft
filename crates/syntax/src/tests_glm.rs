//! Parser tests of LOGISTIC REGRESSION, PLUM, UNIANOVA and GLM.

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

fn logistic(source: &str) -> Logistic {
    match one(source) {
        Command::Logistic(l) => *l,
        other => panic!("{other:?}"),
    }
}

fn glm(source: &str) -> Glm {
    match one(source) {
        Command::Glm(g) => *g,
        other => panic!("{other:?}"),
    }
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn logistic_regression_defaults_and_the_pasted_dialog_syntax() {
    let l = logistic("LOGISTIC REGRESSION VARIABLES=bought WITH age income.");
    assert_eq!(l.dependent, "bought");
    assert_eq!(l.terms, [words(&["age"]), words(&["income"])]);
    assert_eq!(
        l.methods,
        [LogisticMethod {
            kind: LogisticMethodKind::Enter,
            terms: Vec::new()
        }]
    );
    assert_eq!(l.criteria, LogisticCriteria::default());
    assert!(!l.origin && !l.include && l.casewise.is_none() && l.save.is_empty());
    let l = logistic(
        "LOGISTIC REGRESSION VARIABLES bought\n  /METHOD=FSTEP(LR) age income region\n  \
         /CONTRAST (region)=Indicator(1)\n  /SAVE=PRED PGROUP COOK LEVER DFBETA(df)\n  \
         /CLASSPLOT\n  /CASEWISE OUTLIER(2)\n  /PRINT=GOODFIT CORR ITER(1) CI(95)\n  \
         /CRITERIA=PIN(0.05) POUT(0.10) ITERATE(20) CUT(0.5).",
    );
    assert!(l.terms.is_empty());
    assert_eq!(l.methods[0].kind, LogisticMethodKind::Fstep(Removal::Lr));
    assert_eq!(l.methods[0].terms.len(), 3);
    assert_eq!(
        l.contrasts,
        [("region".to_string(), Contrast::Indicator(Some(1)))]
    );
    assert_eq!(l.save[4], (LogisticTemp::Dfbeta, Some("df".into())));
    assert!(l.classplot && l.print.goodfit && l.print.corr);
    assert_eq!(l.print.iter, Some(1));
    assert_eq!(l.print.ci, Some(95.0));
    let cw = l.casewise.unwrap();
    assert_eq!(cw.outlier, Some(2.0));
    assert_eq!(cw.temps.len(), 5);
}

#[test]
fn logistic_regression_abbreviations_interactions_and_methods() {
    let l = logistic(
        "LOGI REG y WITH a b a BY b /CATE=b /CONT(b)=HELM /BSTEP(WALD) a b a BY b /METH=ENTER ALL \
         /NOCONST /MISS=INCLUDE /CRIT=BCON(1E-6) LCON(0.01) EPS(1E-10) /EXTERNAL.",
    );
    assert_eq!(l.terms[2], words(&["a", "b"]));
    assert_eq!(l.categorical, ["b"]);
    assert_eq!(l.contrasts[0].1, Contrast::Helmert);
    assert_eq!(l.methods[0].kind, LogisticMethodKind::Bstep(Removal::Wald));
    assert_eq!(l.methods[0].terms[2], words(&["a", "b"]));
    assert_eq!(l.methods[1].kind, LogisticMethodKind::Enter);
    assert!(l.methods[1].terms.is_empty());
    assert!(l.origin && l.include);
    assert_eq!(l.criteria.bcon, 1e-6);
    assert_eq!(l.criteria.lcon, 0.01);
    assert_eq!(l.criteria.eps, 1e-10);
    let l = logistic(
        "LOGISTIC REGRESSION y WITH g /CATEGORICAL=g /CONTRAST(g)=POLYNOMIAL(1,2,4) /PRINT=ALL.",
    );
    assert_eq!(
        l.contrasts[0].1,
        Contrast::Polynomial(Some(vec![1.0, 2.0, 4.0]))
    );
    assert!(l.print.corr && l.print.goodfit && l.print.ci == Some(95.0));
    let l = logistic(
        "LOGISTIC REGRESSION y WITH g /CATEGORICAL=g /CONTRAST(g)=SPECIAL(1 -1 0 0 1 -1).",
    );
    assert_eq!(
        l.contrasts[0].1,
        Contrast::Special(vec![1.0, -1.0, 0.0, 0.0, 1.0, -1.0])
    );
    for (source, method) in [
        ("/FSTEP", LogisticMethodKind::Fstep(Removal::Cond)),
        (
            "/METHOD=BSTEP(COND)",
            LogisticMethodKind::Bstep(Removal::Cond),
        ),
        (
            "/METHOD FSTEP (WALD)",
            LogisticMethodKind::Fstep(Removal::Wald),
        ),
    ] {
        let l = logistic(&format!("LOGISTIC REGRESSION y WITH a {source}."));
        assert_eq!(l.methods[0].kind, method, "{source}");
    }
}

#[test]
fn logistic_regression_errors() {
    for (source, message) in [
        (
            "LOGISTIC REGRESSION y.",
            "the independent variables follow WITH, or every METHOD names its own",
        ),
        (
            "LOGISTIC REGRESSION y z WITH a.",
            "LOGISTIC REGRESSION takes one dependent variable; the independent variables follow WITH",
        ),
        (
            "LOGISTIC REGRESSION y WITH a /CRITERIA=CUT(1).",
            "CUT must lie between 0 and 1, got 1",
        ),
        (
            "LOGISTIC REGRESSION y WITH a /CRITERIA=EPS(0.1).",
            "EPS must lie from 1E-12 to 0.05, got 0.1",
        ),
        (
            "LOGISTIC REGRESSION y WITH a /PRINT=CI(99.5).",
            "CI takes a whole number from 1 to 99, got 99.5",
        ),
        (
            "LOGISTIC REGRESSION y WITH a /SELECT=s EQ 1.",
            "subcommand SELECT of LOGISTIC REGRESSION is not supported yet",
        ),
        (
            "LOGISTIC REGRESSION y WITH a /SAVE=PRED PRED.",
            "a temporary variable is saved only once",
        ),
        (
            "LOGISTIC REGRESSION y WITH a /CONTRAST(a)=SIMPLE(0).",
            "the reference category is a whole number of at least 1, got 0",
        ),
    ] {
        assert_eq!(err(source).message, message, "{source}");
    }
}

#[test]
fn unianova_defaults_and_the_pasted_dialog_syntax() {
    let g = glm("UNIANOVA y BY a b WITH x.");
    assert_eq!(g.command, "UNIANOVA");
    assert_eq!(g.factors.len(), 2);
    assert_eq!(g.covariates.len(), 1);
    assert_eq!(g.sstype, 3);
    assert!(g.intercept && !g.include);
    assert_eq!((g.eps, g.alpha), (1e-8, 0.05));
    assert_eq!(g.design, None);
    let g = glm(
        "UNIANOVA y BY a b WITH x\n  /METHOD=SSTYPE(3)\n  /INTERCEPT=INCLUDE\n  \
         /SAVE=PRED RESID ZRESID COOK(c)\n  /PLOT=PROFILE(a*b) TYPE=LINE ERRORBAR=NO\n  \
         /EMMEANS=TABLES(a) WITH(x=MEAN) COMPARE ADJ(BONFERRONI)\n  \
         /EMMEANS=TABLES(a*b) WITH(x=10) COMPARE(b) ADJ(SIDAK)\n  \
         /POSTHOC=a(TUKEY BONFERRONI DUNNETT(1))\n  \
         /PRINT=ETASQ DESCRIPTIVE PARAMETER HOMOGENEITY OPOWER LOF\n  \
         /CRITERIA=ALPHA(.05)\n  /DESIGN=x a b a*b.",
    );
    assert_eq!(g.save[3], (GlmTemp::Cook, Some("c".into())));
    let GlmPlot::Profile(profile) = &g.plots[0] else {
        panic!("{:?}", g.plots);
    };
    assert_eq!(profile.factors, ["a", "b"]);
    assert!(!profile.bar && profile.error_bar.is_none() && !profile.mean_reference);
    assert_eq!(g.emmeans[0].factors, ["a"]);
    assert_eq!(g.emmeans[0].compare, Some(None));
    assert_eq!(g.emmeans[0].adjustment, Adjustment::Bonferroni);
    assert_eq!(g.emmeans[1].factors, ["a", "b"]);
    assert_eq!(g.emmeans[1].with, [("x".to_string(), Some(10.0))]);
    assert_eq!(g.emmeans[1].compare, Some(Some("b".into())));
    assert_eq!(g.posthoc[0].1.len(), 3);
    let p = g.print;
    assert!(p.etasq && p.descriptive && p.parameter && p.homogeneity && p.opower && p.lof);
    let labels: Vec<&str> = g
        .design
        .as_ref()
        .unwrap()
        .iter()
        .map(|t| t.label.as_str())
        .collect();
    assert_eq!(labels, ["x", "a", "b", "a*b"]);
}

#[test]
fn design_terms_with_interactions_and_nesting() {
    let g = glm(
        "UNIANOVA y BY a b c WITH x /DESIGN=INTERCEPT a BY b, b(a) c WITHIN b(a) x*x x*a(b) a(b(c)).",
    );
    let design = g.design.unwrap();
    let labels: Vec<&str> = design.iter().map(|t| t.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Intercept",
            "a*b",
            "b(a)",
            "c(b(a))",
            "x*x",
            "x*a(b)",
            "a(b(c))"
        ]
    );
    assert!(design[0].variables.is_empty());
    assert_eq!(design[3].variables, words(&["c", "b", "a"]));
    assert_eq!(design[4].variables, words(&["x", "x"]));
    let e = err("UNIANOVA y BY a b c /DESIGN=a(b)(c).");
    assert!(
        e.message.contains("nested in the one before"),
        "{}",
        e.message
    );
}

#[test]
fn glm_methods_criteria_and_matrices() {
    let g = glm(
        "GLM y BY a b /METH=SSTYPE(1) /INTER=EXCLUDE /MISS=INCLUDE /CRIT=EPS(1E-10) ALPHA(.01) \
         /LMATRIX='a1 vs a2' a 1 -1 0 a*b 1/2 1/2 -1/2 -1/2 0 0; a 0 1 -1 \
         /LMATRIX ALL 0 1 -1 0 0 0 0 0 0 0 0 0 /KMATRIX=0;0 /CONTRAST(a)=REPEATED /REGWGT=w.",
    );
    assert_eq!(g.command, "GLM");
    assert_eq!(g.sstype, 1);
    assert!(!g.intercept && g.include);
    assert_eq!((g.eps, g.alpha), (1e-10, 0.01));
    assert_eq!(g.lmatrix[0].label.as_deref(), Some("a1 vs a2"));
    assert_eq!(g.lmatrix[0].rows.len(), 2);
    assert_eq!(g.lmatrix[0].rows[0][1].1, [0.5, 0.5, -0.5, -0.5, 0.0, 0.0]);
    assert_eq!(g.lmatrix[1].rows[0][0].0, None);
    assert_eq!(g.kmatrix, Some(vec![0.0, 0.0]));
    assert_eq!(g.contrasts, [("a".to_string(), Contrast::Repeated)]);
    assert_eq!(g.regwgt.as_deref(), Some("w"));
    let g = glm(
        "GLM t1 t2 t3 t4 BY g /WSFACTOR=time 2 SIMPLE(1) cond 2 /WSDESIGN=time cond time*cond.",
    );
    assert_eq!(g.dependents.len(), 4);
    assert_eq!(g.wsfactors[0].contrast, Contrast::Simple(Some(1)));
    assert_eq!(g.wsfactors[1].contrast, Contrast::Polynomial(None));
    assert_eq!(g.wsdesign.unwrap().len(), 3);
}

#[test]
fn glm_errors() {
    for (source, message) in [
        (
            "UNIANOVA y BY a /METHOD=SSTYPE(5).",
            "SSTYPE is 1, 2, 3 or 4, got 5",
        ),
        (
            "UNIANOVA y BY a /CRITERIA=ALPHA(1).",
            "ALPHA must lie between 0 and 1, got 1",
        ),
        (
            "UNIANOVA y BY a /RANDOM=a.",
            "subcommand RANDOM of UNIANOVA is not supported yet",
        ),
        (
            "UNIANOVA y BY a /PRINT=GEF.",
            "GEF in PRINT is not supported yet",
        ),
        (
            "UNIANOVA y BY a b /EMMEANS=TABLES(a*b) COMPARE.",
            "COMPARE needs the factor to compare when TABLES crosses factors",
        ),
        (
            "UNIANOVA y BY a /EMMEANS=TABLES(OVERALL) COMPARE.",
            "COMPARE cannot follow TABLES(OVERALL)",
        ),
        (
            "UNIANOVA y BY a /POSTHOC=a(TUKEY) VS b.",
            "VS in POSTHOC is not supported yet; the tests use the error of the model",
        ),
        (
            "UNIANOVA y BY a /POSTHOC=a(WALLER).",
            "WALLER in POSTHOC is not supported yet",
        ),
        (
            "UNIANOVA y BY a /LMATRIX=a 1/-2.",
            "a fraction needs a positive denominator, as in 1/3 or -1/3",
        ),
        (
            "GLM y1 y2 /WSFACTOR=t 1.",
            "a within-subjects factor has a whole number of at least 2 levels, got 1",
        ),
        (
            "UNIANOVA y BY a /CONTRAST(a)=SPECIAL(1 -1).",
            "SPECIAL in CONTRAST is not supported yet; LMATRIX tests any contrast",
        ),
    ] {
        assert_eq!(err(source).message, message, "{source}");
    }
}

#[test]
fn glm_plot_keywords() {
    let g = glm(
        "UNIANOVA y BY a b c WITH x /PLOT=SPREADLEVEL RESIDUALS PROFILE(a a BY b a*b*c) \
         WITH(x=2.5) TYPE=BAR MEANREFERENCE=YES ERRORBAR=SE(1.5) YAXIS=0 /PLOT=PROFILE(b).",
    );
    assert_eq!(g.plots.len(), 6);
    assert_eq!(g.plots[..2], [GlmPlot::SpreadLevel, GlmPlot::Residuals]);
    let profiles: Vec<&Profile> = g
        .plots
        .iter()
        .filter_map(|p| match p {
            GlmPlot::Profile(p) => Some(p),
            _ => None,
        })
        .collect();
    assert_eq!(profiles[2].factors, ["a", "b", "c"]);
    assert_eq!(profiles[1].with, [("x".to_string(), Some(2.5))]);
    assert!(
        profiles[0].bar
            && profiles[0].mean_reference
            && profiles[0].error_bar == Some(ErrorBar::Se(1.5))
    );
    assert!(!profiles[3].bar && profiles[3].with.is_empty());
    assert!(
        err("UNIANOVA y BY a /PLOT=PROFILE(a) ERRORBAR=SE(0).")
            .message
            .contains("positive")
    );
}

fn plum(source: &str) -> Plum {
    match one(source) {
        Command::Plum(p) => *p,
        other => panic!("{other:?}"),
    }
}

#[test]
fn plum_defaults_and_abbreviations() {
    let p = plum("PLUM score.");
    assert_eq!(p.dependent, "score");
    assert!(p.factors.is_empty() && p.covariates.is_empty());
    assert_eq!(p.link, PlumLink::Logit);
    assert_eq!(p.criteria, PlumCriteria::default());
    assert_eq!(
        (p.location, p.scale, p.print, p.include),
        (None, None, None, false)
    );
    // keywords shortened to three letters, a repeated subcommand replacing the earlier one
    let p = plum(
        "PLUM score BY shift crew WITH hours /LIN=PRO /CRI=CIN(90) MXITER(40) PCON(0) LCON(1E-5) \
         DEL(0.5) BIA(0.1) /PRI=FIT /PRI=PAR TPAR HIS(3) /MIS=INC /SAV=EST(prob:2) PRED ACP(hit).",
    );
    assert_eq!(p.factors.len(), 2);
    assert_eq!(p.link, PlumLink::Probit);
    assert_eq!(p.criteria.cin, 90.0);
    assert_eq!(p.criteria.mxiter, 40);
    assert_eq!((p.criteria.pconverge, p.criteria.lconverge), (0.0, 1e-5));
    assert_eq!((p.criteria.delta, p.criteria.bias), (0.5, 0.1));
    let print = p.print.unwrap();
    assert!(!print.fit && print.parameter && print.tparallel);
    assert_eq!(print.history, Some(3));
    assert!(p.include);
    assert_eq!(
        p.save,
        [
            PlumSave::Estprob {
                root: Some("prob".into()),
                count: 2
            },
            PlumSave::Predcat(None),
            PlumSave::Acprob(Some("hit".into())),
        ]
    );
    assert_eq!(
        plum("PLUM score /SAVE=ESTPROB(:3).").save,
        [PlumSave::Estprob {
            root: None,
            count: 3
        }]
    );
}

#[test]
fn plum_location_and_scale() {
    let p = plum(
        "PLUM score BY shift crew WITH hours /LOCATION=hours shift crew shift*crew /SCALE=crew.",
    );
    let labels: Vec<String> = p
        .location
        .unwrap()
        .iter()
        .map(|t| t.label.clone())
        .collect();
    assert_eq!(labels, ["hours", "shift", "crew", "shift*crew"]);
    assert_eq!(p.scale.unwrap()[0].variables, words(&["crew"]));
    // an empty LOCATION is the default model; INTERCEPT alone leaves only the thresholds
    assert_eq!(plum("PLUM score BY shift /LOCATION.").location, None);
    assert_eq!(
        plum("PLUM score BY shift /LOCATION=INTERCEPT.").location,
        Some(Vec::new())
    );
    assert_eq!(plum("PLUM score BY shift /SCALE.").scale, None);
}

#[test]
fn plum_errors() {
    for (source, message) in [
        (
            "PLUM score other.",
            "PLUM takes one dependent variable; factors follow BY and covariates WITH",
        ),
        (
            "PLUM score /CRITERIA=CIN(100).",
            "CIN is at least 0 and below 100",
        ),
        (
            "PLUM score /CRITERIA=DELTA(1).",
            "DELTA is at least 0 and below 1",
        ),
        (
            "PLUM score /CRITERIA=MXSTEP(0).",
            "MXSTEP is a whole number, 1 or more",
        ),
        ("PLUM score /SCALE=INTERCEPT.", "SCALE takes no INTERCEPT"),
        (
            "PLUM score BY a /SCALE=a INTERCEPT.",
            "SCALE takes no INTERCEPT",
        ),
        (
            "PLUM score /SAVE=PCPROB PCPROB.",
            "each SAVE keyword is given only once",
        ),
        (
            "PLUM score BY a /TEST=a 1 -1.",
            "subcommand TEST of PLUM is not supported yet",
        ),
    ] {
        assert_eq!(err(source).message, message, "{source}");
    }
}
