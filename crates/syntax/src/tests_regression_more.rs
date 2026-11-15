//! Parser tests of PROBIT, CURVEFIT and WLS.

use super::*;

fn one(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn probit(source: &str) -> Probit {
    match one(source) {
        Command::Probit(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn curvefit(source: &str) -> Curvefit {
    match one(source) {
        Command::Curvefit(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn wls(source: &str) -> Wls {
    match one(source) {
        Command::Wls(c) => *c,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    let e = errors.remove(0);
    format!("{} (expected {})", e.message, e.expected.join(", "))
}

#[test]
fn probit_defaults_and_either_order_of_by_and_with() {
    let a = probit("PROBIT hatched OF eggs WITH dose BY strain(1,3).");
    let b = probit("PROBIT hatched OF eggs BY strain (1 3) WITH dose.");
    assert_eq!(a, b);
    assert_eq!(a.factor, Some(("strain".into(), 1.0, 3.0)));
    assert!(a.probit && !a.logit);
    assert_eq!(a.log_base, Some(10.0));
    assert!(a.freq && a.ci && a.rmp && !a.parall);
    assert_eq!(a.natres, NaturalResponse::None);
    assert_eq!((a.heterogeneity, a.step_limit), (0.15, 0.1));
}

#[test]
fn probit_subcommands() {
    let c = probit(
        "PROBIT r OF n WITH x y /MOD=BOTH /LOG /CRIT=CONVERGE(1E-10) ITER(80) P(0) STEPLIMIT(2) \
         /NATRES /PRINT=PARALL /MISSING=INCLUDE.",
    );
    assert!(c.probit && c.logit);
    assert_eq!(c.log_base, Some(std::f64::consts::E));
    assert_eq!(
        (c.optol, c.iterate, c.heterogeneity),
        (Some(1e-10), Some(80), 0.0)
    );
    assert_eq!(c.step_limit, 2.0);
    assert_eq!(c.natres, NaturalResponse::Estimate);
    assert!(!c.freq && !c.ci && !c.rmp && c.parall);
    assert!(c.include);
    let c = probit("PROBIT r OF n WITH x /MODEL=LOGIT /LOG=NONE /NATRES=0.2 /PRINT=NONE.");
    assert!(!c.probit && c.logit);
    assert_eq!(c.log_base, None);
    assert_eq!(c.natres, NaturalResponse::Fixed(0.2));
    assert!(!c.freq && !c.ci && !c.rmp && !c.parall);
    let c = probit("PROBIT r OF n WITH x /LOG=2 /PRINT=ALL.");
    assert_eq!(c.log_base, Some(2.0));
    assert!(c.freq && c.ci && c.rmp && c.parall);
    let c = probit("PROBIT r OF n WITH x /PRINT /MODEL.");
    assert!(c.freq && c.ci && c.rmp && !c.parall && c.probit);
}

#[test]
fn probit_errors() {
    assert!(err("PROBIT r n WITH x.").contains("OF"));
    assert!(err("PROBIT r OF n BY g(1,2).").contains("WITH"));
    assert!(err("PROBIT r OF n WITH x BY g(2,1).").contains("smaller first"));
    assert!(err("PROBIT r OF n WITH x BY g(1.5,3).").contains("whole numbers"));
    assert!(err("PROBIT r OF n WITH x /MODEL=PROBIT /MODEL=LOGIT.").contains("only once"));
    assert!(err("PROBIT r OF n WITH x /LOG=1.").contains("other than 1"));
    assert!(err("PROBIT r OF n WITH x /NATRES=1.").contains("below 1"));
    assert!(err("PROBIT r OF n WITH x /CRITERIA=P(2).").contains("from 0 to 1"));
    assert!(err("PROBIT r OF n WITH x /CRITERIA=ITERATE(2.5).").contains("whole number"));
    assert!(err("PROBIT r OF n WITH x /PLOT=NONE.").contains("unknown subcommand"));
}

#[test]
fn curvefit_models_and_options() {
    let c = curvefit("CURVEFIT VARIABLES=visits.");
    assert_eq!(c.models, vec![CurveModel::Linear]);
    assert_eq!(c.independent, None);
    assert!(c.constant && c.plot && !c.anova);
    assert_eq!(c.cin, 95.0);
    let c = curvefit(
        "CURVEFIT visits sales WITH week /MODEL=LGS QUA S EXPONENTIAL /MODEL=CUB lin S LGSTIC \
         /UPPER=500 /NOCONSTANT /CIN=90 /PLOT=NONE /ID=shop /PRINT=ANOVA /SAVE=PRED CIN \
         /TEMPLATE='look.sgt'.",
    );
    assert_eq!(c.dependents.len(), 2);
    assert_eq!(c.independent.as_deref(), Some("week"));
    // the last MODEL counts, in the canonical order
    assert_eq!(
        c.models,
        vec![
            CurveModel::Linear,
            CurveModel::Cubic,
            CurveModel::S,
            CurveModel::Logistic
        ]
    );
    assert_eq!((c.upper_bound, c.cin), (Some(500.0), 90.0));
    assert!(!c.constant && !c.plot && c.anova && c.template);
    assert_eq!(c.id.as_deref(), Some("shop"));
    assert!(c.save_pred && !c.save_resid && c.save_cin);
    let c = curvefit("CURVEFIT /MODEL=ALL /VARIABLES=y /UPPERBOUND=NO.");
    assert_eq!(c.models.len(), 11);
    assert_eq!(c.upper_bound, None);
}

#[test]
fn curvefit_errors() {
    assert!(err("CURVEFIT VARIABLES=y WITH a b.").contains("one independent"));
    assert!(err("CURVEFIT y /CIN=100.").contains("below 100"));
    assert!(err("CURVEFIT y /UPPERBOUND=-2.").contains("positive"));
    assert!(err("CURVEFIT y /MODEL=SPLINE.").contains("unexpected"));
    assert!(err("CURVEFIT y /SAVE.").contains("unexpected"));
    assert!(err("CURVEFIT y /APPLY.").contains("not supported"));
    assert!(err("CURVEFIT APPLY /MODEL=CUBIC.").contains("not supported"));
    assert!(err("CURVEFIT /MODEL=CUBIC.").contains("VARIABLES"));
}

#[test]
fn wls_powers_and_options() {
    let c = wls("WLS rent WITH area rooms.");
    assert_eq!(c.dependent, "rent");
    assert_eq!(c.independents.len(), 2);
    assert!(c.deltas.is_empty() && c.source.is_none() && c.constant && !c.print_all);
    let c = wls(
        "WLS VARIABLES=rent WITH area /SOURCE=rooms /POWER=0 TO 1 BY 0.5 /DELTA=2 BY 1 TO 4, -1 \
         /NOCONSTANT /PRINT=ALL /SAVE=WEIGHT.",
    );
    assert_eq!(c.source.as_deref(), Some("rooms"));
    assert_eq!(c.deltas, vec![0.0, 0.5, 1.0, 2.0, 3.0, 4.0, -1.0]);
    assert!(!c.constant && c.print_all && c.save_weight);
    let c = wls("WLS rent WITH area /WEIGHT=w.");
    assert_eq!(c.weight.as_deref(), Some("w"));
}

#[test]
fn wls_errors() {
    assert!(err("WLS rent area WITH rooms.").contains("one dependent"));
    assert!(err("WLS rent.").contains("one dependent"));
    assert!(err("WLS rent WITH area /WEIGHT=w /SOURCE=area.").contains("not both"));
    assert!(err("WLS rent WITH area /DELTA=2 TO 1 BY 0.5.").contains("grid"));
    assert!(err("WLS rent WITH area /DELTA=0 TO 200 BY 1.").contains("150"));
    assert!(err("WLS rent WITH area /APPLY.").contains("not supported"));
}
