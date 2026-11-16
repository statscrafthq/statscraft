//! Parser tests of TSET, TSHOW, CREATE, ACF, PACF, CCF, SEASON, PREDICT, EXSMOOTH, ARIMA,
//! SPECTRA, AREG and 2SLS.

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

#[test]
fn predict_ranges() {
    assert_eq!(
        command("PREDICT 61 THRU 72."),
        Command::Predict(UseRange {
            first: Some(CasePoint::Case(61)),
            last: Some(CasePoint::Case(72)),
            permanent: false
        })
    );
    assert_eq!(
        command("PREDICT THRU END."),
        Command::Predict(UseRange {
            first: None,
            last: None,
            permanent: false
        })
    );
    assert!(err("PREDICT.").contains("expected"));
}

#[test]
fn exsmooth_subcommands() {
    let Command::Exsmooth(e) = command(
        "EXSMOOTH VARIABLES=rooms beds /MOD=WINTERS NN /PER=4 /SEASFACT=(0.9 1.1 1.2 0.8) \
         /ALPHA=GRID /GAMMA=GRID(0.1, 0.5, 0.2) /DELTA=0.3 /PHI=0.5 /INITIAL=(100, 2).",
    ) else {
        panic!()
    };
    assert_eq!(e.variables.len(), 2);
    assert_eq!(e.model, Some((Trend::Linear, Seasonality::Multiplicative)));
    assert_eq!(e.period, Some(4));
    assert_eq!(
        e.seasfact,
        Some(SeasonalFactors::Values(vec![0.9, 1.1, 1.2, 0.8]))
    );
    assert_eq!(e.alpha, Some(Smoothing::Grid(None)));
    assert_eq!(e.gamma, Some(Smoothing::Grid(Some((0.1, 0.5, 0.2)))));
    assert_eq!(e.delta, Some(Smoothing::Value(0.3)));
    assert_eq!(e.initial, Some(Some((100.0, 2.0))));
    let Command::Exsmooth(e) = command("EXSMOOTH APPLY /SEASFACT=SAF_1 /INITIAL=CALCULATE.") else {
        panic!()
    };
    assert_eq!(e.apply, Some(None));
    assert_eq!(e.seasfact, Some(SeasonalFactors::Variable("SAF_1".into())));
    assert_eq!(e.initial, Some(None));
    assert!(err("EXSMOOTH y /PHI=1.").contains("PHI is above 0 and below 1"));
    assert!(err("EXSMOOTH y /ALPHA=GRID(0.5 0.1 0.1).").contains("ends at or above its start"));
    assert!(err("EXSMOOTH y /INITIAL=(1).").contains("two values"));
    assert!(err("EXSMOOTH y /MODEL=XX.").contains("expected"));
}

#[test]
fn arima_subcommands() {
    let Command::Arima(a) = command(
        "ARIMA calls WITH promo price /MODEL=(1,1,1)(0,1,1) 7 NOCONSTANT LN /P=(2) /SQ=2 \
         /MA=0.3 /SMA=0.4 0.1 /CON=2 /MXITER=20 /PAREPS=0.0001 /CINPCT=90 /FORECAST=CLS.",
    ) else {
        panic!()
    };
    assert_eq!(a.dependent.len(), 1);
    assert_eq!(a.regressors.len(), 2);
    assert_eq!(a.orders, Some([1, 1, 1]));
    assert_eq!(a.seasonal_orders, Some([0, 1, 1]));
    assert_eq!(a.period, Some(7));
    assert_eq!(a.constant, Some(false));
    assert_eq!(a.log, Some(ArimaLog::Ln));
    assert_eq!(a.p, Some(vec![2]));
    assert_eq!(a.sq, Some(vec![1, 2]));
    assert_eq!(a.sma, Some(vec![0.4, 0.1]));
    assert_eq!(a.con, Some(2.0));
    assert_eq!(a.forecast, Some(ArimaForecast::Cls));
    let Command::Arima(a) = command("ARIMA other /APPLY INITIAL.") else {
        panic!()
    };
    assert_eq!(a.apply, Some((None, ArimaApply::Initial)));
    let Command::Arima(a) = command("ARIMA APPLY FIT /MODEL=LOG.") else {
        panic!()
    };
    assert_eq!(
        (a.apply, a.log),
        (Some((None, ArimaApply::Fit)), Some(ArimaLog::Log10))
    );
    assert!(err("ARIMA a b /MODEL=(1,0,0).").contains("one dependent"));
    assert!(err("ARIMA a /MODEL=(1,0,0) 12.").contains("after the seasonal orders"));
    assert!(err("ARIMA a /CINPCT=100.").contains("below 100"));
}

#[test]
fn series_plot_subcommands() {
    let Command::SeriesPlot(t) = command(
        "TSPLOT VARIABLES=level flow /DIFF /SDIFF=2 /PER=7 /LN /ID=day /MARK=YEAR 2021 MONTH 4 \
         /SPLIT=SCALE /FORMAT=HILO.",
    ) else {
        panic!()
    };
    assert!(!t.casewise);
    assert_eq!(
        (t.diff, t.sdiff, t.period, t.ln),
        (Some(1), Some(2), Some(7), Some(true))
    );
    assert_eq!(
        t.mark,
        Some(PlotMark::Date(vec![
            (DateUnit::Year, 2021.0),
            (DateUnit::Month, 4.0)
        ]))
    );
    assert_eq!((t.split_scale, t.join), (Some(true), Some(PlotJoin::Hilo)));
    let Command::SeriesPlot(c) =
        command("CASEPLOT load /MARK=switch /FORMAT=LEFT REFERENCE(12.5).")
    else {
        panic!()
    };
    assert!(c.casewise);
    assert_eq!(c.mark, Some(PlotMark::Variable("switch".into())));
    assert_eq!(c.fill, Some(true));
    assert_eq!(c.reference, Some(PlotReference::Value(12.5)));
    let e = err("TSPLOT y /FORMAT=LEFT.");
    assert!(e.contains("unexpected word LEFT"), "{e}");
    assert!(err("CASEPLOT y /SPLIT=WIDE.").contains("WIDE"));
}

#[test]
fn spectra_subcommands() {
    let Command::Spectra(s) = command(
        "SPECTRA VARIABLES=flow rain /CENTER /CROSS /WINDOW=PARZEN(7) TUKEY \
         /PLOT=P S K BY PER /SAVE=FREQ(fq) S(dens) K(coh).",
    ) else {
        panic!()
    };
    assert_eq!(s.variables.len(), 2);
    assert_eq!((s.center, s.cross), (Some(true), Some(true)));
    // of several windows the first counts
    assert_eq!(
        s.window,
        Some(SpectraSmoothing::Named(SpectraWindow::Parzen, Some(7)))
    );
    let plot = s.plot.unwrap();
    assert_eq!(
        plot.items,
        vec![SpectraItem::P, SpectraItem::S, SpectraItem::K]
    );
    assert_eq!(plot.by, Some(SpectraBy::Period));
    assert_eq!(s.save.len(), 3);
    assert_eq!(s.save[1], (SpectraItem::S, "dens".to_string()));
    let Command::Spectra(s) = command("SPECTRA flow /WINDOW=0.2, 0.6, 0.2 /CENTER NO /PLOT=NONE.")
    else {
        panic!()
    };
    assert_eq!(
        s.window,
        Some(SpectraSmoothing::Weights(vec![0.2, 0.6, 0.2]))
    );
    assert_eq!(s.center, Some(false));
    assert!(s.plot.unwrap().items.is_empty());
    let Command::Spectra(s) = command("SPECTRA APPLY /WINDOW=DANIELL /PLOT=ALL.") else {
        panic!()
    };
    assert_eq!(s.apply, Some(None));
    assert!(s.plot.unwrap().all);
    assert!(err("SPECTRA flow /WINDOW=HANN(5).").contains("expected"));
    let message = err("SPECTRA flow /SAVE=P.");
    assert!(message.contains("expected"), "{message}");
}

#[test]
fn areg_subcommands() {
    let Command::Areg(a) =
        command("AREG load WITH temp hour /METH=CO /NOCONSTANT /RHO=-0.25 /MXITER=5.")
    else {
        panic!()
    };
    assert_eq!((a.dependent.len(), a.regressors.len()), (1, 2));
    assert_eq!(a.method, Some(AregMethod::Co));
    assert_eq!(
        (a.constant, a.rho, a.mxiter),
        (Some(false), Some(-0.25), Some(5))
    );
    let Command::Areg(a) = command("AREG load WITH other /APPLY INITIAL.") else {
        panic!()
    };
    assert_eq!(a.apply, Some((None, ArimaApply::Initial)));
    assert!(err("AREG load.").contains("regressor"));
    assert!(err("AREG load WITH temp /RHO=1.").contains("below 1"));
    assert!(err("AREG a b WITH c.").contains("one dependent"));
}

#[test]
fn two_stage_subcommands() {
    let Command::TwoStage(t) = command(
        "2SLS demand WITH price income /EQUATION=price WITH demand cost \
         /INSTRUMENTS=income /INSTRUMENTS cost lagprice /ENDOGENOUS=price demand \
         /NOCONSTANT /PRINT=COV /SAVE=PRED RESID.",
    ) else {
        panic!()
    };
    assert_eq!(t.equations.len(), 2);
    assert_eq!(t.equations[1].dependent, "price");
    assert_eq!(t.instruments.as_ref().map(Vec::len), Some(3));
    assert_eq!(t.endogenous.len(), 2);
    assert!(t.print_cov && t.save_pred && t.save_resid);
    assert_eq!(t.constant, Some(false));
    // the name starts with a digit; lower case works too
    let Command::TwoStage(t) = command("2sls y WITH x /INSTRUMENTS z.") else {
        panic!()
    };
    assert_eq!(t.equations[0].dependent, "y");
    assert!(err("2SLS y WITH x.").contains("INSTRUMENTS"));
    assert!(err("2SLS y x /INSTRUMENTS=z.").contains("WITH"));
}
