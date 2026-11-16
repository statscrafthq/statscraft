//! Parser tests of TSET, TSHOW, CREATE, ACF, PACF, CCF and SEASON.

use super::*;

fn command(source: &str) -> Command {
    let (mut parsed, errors) = parse(source);
    assert!(errors.is_empty(), "{source}: {errors:?}");
    assert_eq!(parsed.len(), 1, "{source}");
    parsed.remove(0).command
}

fn err(source: &str) -> String {
    let (_, mut errors) = parse(source);
    assert_eq!(errors.len(), 1, "{source}: {errors:?}");
    errors.remove(0).message
}

fn correlogram(source: &str) -> Correlogram {
    match command(source) {
        Command::Correlogram(c) => *c,
        other => panic!("{other:?}"),
    }
}

#[test]
fn tset_items_with_and_without_slashes() {
    let Command::Tset(items) =
        command("TSET PERIOD 7 NEWVARS NONE /MXAUTO=30 MISS=INCLUDE CIN DEFAULT NOCONSTANT.")
    else {
        panic!()
    };
    assert_eq!(
        items,
        vec![
            TsetItem::Period(Some(7)),
            TsetItem::NewVar(Some(NewVar::None)),
            TsetItem::MxAuto(Some(30)),
            TsetItem::Include(Some(true)),
            TsetItem::Cin(None),
            TsetItem::Constant(Some(false)),
        ]
    );
    let mut settings = TimeSettings::default();
    for item in &items {
        settings.apply(item);
    }
    assert_eq!(
        (settings.period, settings.mxauto, settings.cin),
        (Some(7), 30, 95.0)
    );
    settings.apply(&TsetItem::Default);
    assert_eq!(settings, TimeSettings::default());
    assert!(err("TSET CIN=100.").contains("below 100"));
    assert!(err("TSET MXAUTO=0.").contains("at least 1"));
    assert!(matches!(command("TSHOW."), Command::Tshow));
}

#[test]
fn create_equations() {
    let Command::Create(eq) = command(
        "CREATE lag2 TO lag4 = LAG(price, 2, 4) /sm = MA(price cost, 5 1) /c = CSUM(price).",
    ) else {
        panic!()
    };
    assert_eq!(eq[0].targets, vec!["lag2", "lag3", "lag4"]);
    assert_eq!(eq[0].function, SeriesFunction::Lag);
    assert_eq!(eq[0].numbers, vec![2, 4]);
    assert_eq!(eq[1].sources.len(), 2);
    assert_eq!(eq[1].numbers, vec![5, 1]);
    assert_eq!(eq[2].numbers, Vec::<u32>::new());
    assert!(err("CREATE d = DIFF(price).").contains("one number"));
    assert!(err("CREATE c = CSUM(price, 2).").contains("no number"));
    assert!(err("CREATE l = LAG(price, 4, 2).").contains("lower order"));
    assert!(err("CREATE m = MA(price, 3, 5).").contains("minimum span"));
    assert!(err("CREATE m = MA(price, 0).").contains("at least 1"));
}

#[test]
fn correlogram_subcommands() {
    let c = correlogram(
        "ACF VARIABLES=sales /DIFFERENCE /SDIFF=2 /PER=12 /LN /SEASONAL /MXAUTO=20 /SERROR=MA /PACF.",
    );
    assert_eq!(c.kind, CorrelogramKind::Acf);
    assert_eq!((c.diff, c.sdiff, c.period), (Some(1), Some(2), Some(12)));
    assert_eq!(
        (c.ln, c.seasonal, c.max_lag),
        (Some(true), Some(true), Some(20))
    );
    assert_eq!((c.ma_se, c.pacf), (Some(true), Some(true)));
    let c = correlogram("PACF sales /NOLOG /DIFF=0.");
    assert_eq!(
        (c.kind, c.ln, c.diff),
        (CorrelogramKind::Pacf, Some(false), Some(0))
    );
    let c = correlogram("CCF a b WITH c /MXCROSS=3.");
    assert_eq!(c.with, Some(vec![VarItem::Name("c".into())]));
    assert_eq!(c.max_lag, Some(3));
    let c = correlogram("ACF APPLY /NOLOG.");
    assert_eq!(c.apply, Some(None));
    let c = correlogram("ACF x /APPLY='MOD_2'.");
    assert_eq!(c.apply, Some(Some("MOD_2".into())));
    assert!(err("PACF x /SERROR=MA.").contains("SERROR"));
    assert!(err("ACF x /PERIOD=1.").contains("at least 2"));
    assert!(err("ACF /LN.").contains("needs VARIABLES"));
}

#[test]
fn season_subcommands() {
    let Command::Season(s) =
        command("SEASON VARIABLES=x y /MODEL=ADDITIVE MULTIPLICATIVE /MA=EQUAL /PERIOD=6.")
    else {
        panic!()
    };
    assert_eq!(s.variables.len(), 2);
    assert_eq!(
        (s.additive, s.centered, s.period),
        (Some(true), Some(false), Some(6))
    );
    let Command::Season(s) = command("SEASON x /MODEL.") else {
        panic!()
    };
    assert_eq!(s.additive, Some(false));
}
