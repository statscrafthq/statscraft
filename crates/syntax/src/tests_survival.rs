//! Parser tests of KM and SURVIVAL.

use super::*;
use crate::ast::{Bound, MissingValue, Value};

fn command(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn km(source: &str) -> Km {
    match command(source) {
        Command::Km(k) => *k,
        other => panic!("{other:?}"),
    }
}

fn survival(source: &str) -> Survival {
    match command(source) {
        Command::Survival(s) => *s,
        other => panic!("{other:?}"),
    }
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

#[test]
fn km_defaults_and_the_other_name() {
    let k = km("KM weeks /STATUS=outcome(1).");
    assert_eq!((k.time.as_str(), k.factor.as_deref()), ("weeks", None));
    assert_eq!(k.event, vec![MissingValue::Value(Value::Number(1.0))]);
    assert!(k.lost.is_empty() && k.plots.is_empty() && k.percentiles.is_empty());
    assert!(k.table && k.mean && k.tests.is_empty() && k.compare.is_empty());
    assert_eq!(km("K-M weeks /STATUS=outcome(1).").time, "weeks");
}

#[test]
fn km_subcommands() {
    let k = km(
        "KM weeks BY plan /STATUS=outcome EVENT(1, 3 THRU 5) LOST(9) /STRATA=region \
         /ID=client /PLOT /PLOT=HAZ OMS /PRINT=MEAN /PERCENTILES=(10 90) /TEST=BRESLOW TARONE \
         /COMPARE=PAIRWISE STRATA /COMPARE /TREND=(1 2 4) \
         /SAVE=SURVIVAL(s_hat) SE CUMEVENT.",
    );
    assert_eq!(k.factor.as_deref(), Some("plan"));
    assert_eq!(
        k.event[1],
        MissingValue::Range(Bound::Number(3.0), Bound::Number(5.0))
    );
    assert_eq!(k.lost, vec![MissingValue::Value(Value::Number(9.0))]);
    assert_eq!(
        (k.strata.as_deref(), k.id.as_deref()),
        (Some("region"), Some("client"))
    );
    assert_eq!(
        k.plots,
        [KmPlot::Survival, KmPlot::Hazard, KmPlot::OneMinusSurvival]
    );
    assert!(!k.table && k.mean);
    assert_eq!(k.percentiles, [10.0, 90.0]);
    assert_eq!(k.tests, [KmTest::Breslow, KmTest::Tarone]);
    assert_eq!(k.compare.len(), 2);
    assert!(k.compare[0].pairwise && k.compare[0].per_stratum);
    assert!(!k.compare[1].pairwise && !k.compare[1].per_stratum);
    assert_eq!(k.trend, Some(vec![1.0, 2.0, 4.0]));
    assert_eq!(
        k.save,
        [
            (KmSave::Survival, Some("s_hat".into())),
            (KmSave::Se, None),
            (KmSave::CumEvent, None)
        ]
    );
    let k = km("KM t /STATUS=s('done') /PERCENTILES /TREND.");
    assert_eq!(
        k.event,
        vec![MissingValue::Value(Value::String("done".into()))]
    );
    assert_eq!(k.percentiles, [25.0, 50.0, 75.0]);
    assert_eq!(k.trend, Some(Vec::new()));
}

#[test]
fn km_errors() {
    assert!(err("KM t.").contains("needs STATUS"));
    assert!(err("KM t /STATUS=s.").contains("unexpected end"));
    assert!(err("KM t /STATUS=s(1) /PERCENTILES=100.").contains("below 100"));
    assert!(err("KM t /STATUS=s(1) /SAVE=SE SE.").contains("at most once"));
    assert!(err("KM t /STATUS=s(1) /PLOT=DENSITY.").contains("unexpected"));
}

#[test]
fn survival_tables_controls_and_intervals() {
    let s = survival(
        "SURVIVAL TABLES=weeks days BY plan region(1,3) promo(0 1) BY site(1,2) \
         /INTERVAL=THRU 20 BY 6 THRU 30 BY 4 /STATUS=outcome(1) FOR weeks /STATUS=done(2,3).",
    );
    assert_eq!(s.variables.len(), 2);
    assert_eq!(s.controls.len(), 2);
    let first: Vec<(i64, i64)> = s.controls[0].iter().map(|c| (c.min, c.max)).collect();
    assert_eq!(first, [(1, 3), (1, 3), (0, 1)]);
    assert_eq!(s.controls[1][0].variable, VarItem::Name("site".into()));
    assert_eq!(s.intervals, [(20.0, 6.0), (30.0, 4.0)]);
    assert_eq!(
        s.status[0].applies_to,
        Some(vec![VarItem::Name("weeks".into())])
    );
    assert_eq!((s.status[1].min, s.status[1].max), (2.0, 3.0));
    assert!(s.status[1].applies_to.is_none());
    assert!(s.table && s.plots.is_none() && s.compare.is_none());
}

#[test]
fn survival_options() {
    let s = survival(
        "SURV weeks BY plan(1,3) /INT=THRU 30 BY 5 /STATUS=outcome(1) /PLOTS(SURVIVAL,HAZARD)=weeks \
         /PRINT=NOTABLE /COMPARE /CALCULATE=APPROXIMATE PAIRWISE /MISSING=LISTWISE INCLUDE.",
    );
    let (kinds, selection) = s.plots.unwrap();
    assert_eq!(kinds, [LifePlot::Survival, LifePlot::Hazard]);
    assert_eq!(selection, vec![vec![VarItem::Name("weeks".into())]]);
    assert!(!s.table && s.compare == Some(Vec::new()));
    assert!(s.approximate && s.pairwise && !s.compare_only);
    assert!(s.listwise && s.include);
    // PLOTS alone draws every function; CALCULATE without COMPARE changes nothing
    let s = survival("SURVIVAL weeks /INTERVAL=THRU 9 BY 3 /STATUS=s(1) /PLOTS /CALC=PAIRWISE.");
    assert_eq!(s.plots.unwrap().0.len(), 5);
    assert!(!s.pairwise);
}

#[test]
fn survival_commas_and_a_variable_named_table() {
    let s = survival("SURVIVAL table BY a(1,2), b(1,3) /INTERVAL=THRU 9 BY 3 /STATUS=s(1).");
    assert_eq!(s.variables, vec![VarItem::Name("table".into())]);
    assert_eq!(s.controls[0].len(), 2);
    let s = survival("SURVIVAL TABLE weeks BY a(1,2) /INTERVAL=THRU 9 BY 3 /STATUS=s(1).");
    assert_eq!(s.variables, vec![VarItem::Name("weeks".into())]);
    assert!(km("KM t /STATUS=a(1) /STATUS=b(1).").status_repeated);
    let k = km("KM t /STATUS=a(1) /PRINT.");
    assert!(k.table && k.mean);
    assert!(survival("SURVIVAL t /INTERVAL=THRU 9 BY 3 /STATUS=s(1) /PRINT.").table);
}

#[test]
fn survival_errors() {
    assert!(err("SURVIVAL t /STATUS=s(1).").contains("needs INTERVAL"));
    assert!(err("SURVIVAL t /INTERVAL=THRU 9 BY 3.").contains("needs STATUS"));
    assert!(err("SURVIVAL t BY g /INTERVAL=THRU 9 BY 3 /STATUS=s(1).").contains("unexpected '/'"));
    assert!(
        err("SURVIVAL t /INTERVAL=THRU 9 BY 3 THRU 5 BY 1 /STATUS=s(1).").contains("ascending")
    );
    assert!(
        err("SURVIVAL t BY g(1,2) /INTERVAL=THRU 9 BY 3 /STATUS=s(1) /CALCULATE=COMPARE.")
            .contains("needs COMPARE")
    );
    assert!(err("SURVIVAL t BY g(1.5,2) /INTERVAL=THRU 9 BY 3 /STATUS=s(1).").contains("whole"));
}
