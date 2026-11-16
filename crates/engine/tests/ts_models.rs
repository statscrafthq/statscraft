//! USE, PREDICT, EXSMOOTH and ARIMA on invented series. The ARIMA estimates, forecasts and
//! their standard errors come from an independent general statistics package run as a black
//! box (exact maximum likelihood; its moving-average sign is the opposite of ours). The
//! smoothing numbers are the published recursions computed by hand.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Sixty daily readings of a water tank level that drifts around 50.
const TANK: &str = "50.4 49.5 48 45.6 45.1 45.5 46.2 45.4 45.1 46.3 47.6 50.2 53.5 54.6 53.6 49.3 52 \
56.6 55.4 57.3 56.8 53.1 55 55.1 50.7 48.3 45.2 46 43.8 44 43.8 43.6 48.8 54.6 54.8 57.8 54.7 51.5 \
52.2 48.5 48.6 46.3 48.1 47.9 46.1 47.7 50.5 51.8 53.5 50.3 47.8 49.2 47.9 47.9 49.4 52.5 52.4 48.6 \
48.2 47.1";

/// Six years of monthly bookings at a ski rental shop.
const RENTALS: &str = "114.7 117.4 122.1 125.9 130.8 134.6 137.6 135.9 132.1 129.6 125.5 123.2 \
126 128.4 133.2 136.5 141 144.8 146.9 143.7 138.9 135 130.1 127.5 129.7 132.3 136.7 140.6 145 148.3 \
151.1 148.8 144.5 141.2 136.4 133.9 136.3 138.9 143.6 147 151.6 155.5 158 155.6 150.8 147 142.9 \
140.3 142.4 144.7 149 152.4 156.7 160.1 162.9 160.2 155.5 152 146.9 144.3 146.3 148.1 151.8 154.4 \
158.2 161.3 162.9 159.8 154.7 150.5 145.7 142.6";

/// Forty-eight weeks of bakery sales and the hours of sunshine.
const BAKERY: &str = "18.2 4.7 12.7 1.4 29.1 9.1 15.4 2.2 29.3 9.6 24.3 6.9 24.1 6.6 21 5.6 \
25.1 6.6 19.8 4.2 31.6 9.8 14.2 1.5 23.3 6.6 16.6 3.7 16.8 3.7 17.6 4.8 15.3 2.9 27.2 8.1 17.5 4 \
11.6 1 20.7 5.7 17.1 4.9 16.2 3.6 23.6 6.8 24 7.2 13.5 2.3 27.2 8.3 13.3 1.2 23.9 6.9 21.6 4.9 \
18.8 3.5 10.5 0.1 21.5 4.9 15.7 2.6 20.8 5 10.6 0.3 18.1 4.2 12.1 1.6 17.1 3.8 15.4 2.8 18.1 3.9 \
26.5 7.6 27.8 8.6 22.4 6.5 27.4 8.6 18.4 3.9 25.4 7.8 24.1 8.2";

struct Run {
    tables: Vec<Table>,
    warnings: Vec<String>,
    engine: Engine,
}

fn data(names: &str, values: &str) -> String {
    format!("DATA LIST FREE /{names}.\nBEGIN DATA\n{values}\nEND DATA.\n")
}

fn run(source: &str) -> Run {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables = engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
    }
    let warnings = engine.warnings().to_vec();
    Run {
        tables,
        warnings,
        engine,
    }
}

fn error(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            return e.to_string();
        }
    }
    panic!("no error")
}

fn find<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables.iter().find(|t| t.title == title).unwrap_or_else(|| {
        panic!(
            "no table {title}: {:?}",
            tables.iter().map(|t| &t.title).collect::<Vec<_>>()
        )
    })
}

fn num(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("not a number: {other:?}"),
    }
}

/// The estimate of the row named `name`.
fn estimate(tables: &[Table], name: &str) -> (f64, f64) {
    let t = find(tables, "Parameter Estimates");
    let row = t
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == name))
        .unwrap_or_else(|| panic!("no row {name}"));
    (num(&row[1]), num(&row[2]))
}

fn statistic(tables: &[Table], name: &str) -> f64 {
    let t = find(tables, "Model Statistics");
    let row = t
        .rows
        .iter()
        .find(|r| matches!(&r[0], Cell::Text(s) if s == name))
        .unwrap_or_else(|| panic!("no row {name}"));
    num(&row[1])
}

fn column(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().expect("a dataset");
    let v = data.variable(name).expect("the variable");
    let values = v.numeric().expect("numeric");
    (0..data.case_count()).map(|k| values.get(k)).collect()
}

fn close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

#[test]
fn arima_1_0_1_with_a_constant_matches_the_reference() {
    let r = run(&format!(
        "{}ARIMA level /MODEL=(1,0,1) /MXITER=50 /SSQPCT=1E-9 /PAREPS=1E-7.\n",
        data("level", TANK)
    ));
    let (ar, ar_se) = estimate(&r.tables, "AR1");
    let (ma, _) = estimate(&r.tables, "MA1");
    let (mu, _) = estimate(&r.tables, "Constant");
    assert!(close(ar, 0.691261, 2e-4), "{ar}");
    assert!(close(ma, -0.361073, 2e-4), "{ma}");
    assert!(close(mu, 49.69789, 2e-3), "{mu}");
    // the standard errors agree up to the degrees of freedom
    assert!(close(ar_se, 0.114101, 2e-3), "{ar_se}");
    // residual sum of squares: n times the reference's variance 4.479391
    let ssq = statistic(&r.tables, "Residual sum of squares");
    assert!(close(ssq, 60.0 * 4.479391, 2e-2), "{ssq}");
    assert_eq!(statistic(&r.tables, "Residual df"), 57.0);
    let fit = column(&r.engine, "FIT_1");
    let err = column(&r.engine, "ERR_1");
    assert_eq!(fit.len(), 60);
    // the first residual of the reference, which divides it by its standard error
    let sigma = (ssq / 57.0).sqrt();
    let sep = column(&r.engine, "SEP_1");
    let standardized = err[0].unwrap() / (sep[0].unwrap() / sigma);
    assert!(close(standardized, 0.3974398, 2e-3), "{standardized}");
}

#[test]
fn arima_forecasts_after_the_series_with_predict() {
    let r = run(&format!(
        "{}PREDICT THRU 63.\nARIMA level /MODEL=(1,0,1) /MXITER=50 /SSQPCT=1E-9 /PAREPS=1E-7.\n",
        data("level", TANK)
    ));
    let fit = column(&r.engine, "FIT_1");
    let sep = column(&r.engine, "SEP_1");
    let err = column(&r.engine, "ERR_1");
    let level = column(&r.engine, "level");
    assert_eq!(fit.len(), 63);
    let expected = [47.29811, 48.03901, 48.55117];
    let se = [2.116457, 3.072443, 3.436604];
    // our standard errors use n - 3 degrees of freedom, the reference n
    let scale = (60.0f64 / 57.0).sqrt();
    for k in 0..3 {
        assert!(
            close(fit[60 + k].unwrap(), expected[k], 2e-3),
            "{k}: {:?}",
            fit[60 + k]
        );
        assert!(
            close(sep[60 + k].unwrap(), se[k] * scale, 3e-3),
            "{k}: {:?}",
            sep[60 + k]
        );
        assert_eq!(err[60 + k], None);
        assert_eq!(level[60 + k], None);
    }
    let lcl = column(&r.engine, "LCL_1");
    let ucl = column(&r.engine, "UCL_1");
    let mid = (lcl[61].unwrap() + ucl[61].unwrap()) / 2.0;
    assert!(close(mid, fit[61].unwrap(), 1e-9));
}

#[test]
fn arima_differenced_without_constant() {
    let r = run(&format!(
        "{}ARIMA level /MODEL=(0,1,1) NOCONSTANT /MXITER=50 /SSQPCT=1E-9 /PAREPS=1E-7.\n",
        data("level", TANK)
    ));
    let (ma, ma_se) = estimate(&r.tables, "MA1");
    assert!(close(ma, -0.213598, 2e-4), "{ma}");
    assert!(close(ma_se, 0.143525, 2e-3), "{ma_se}");
    let ssq = statistic(&r.tables, "Residual sum of squares");
    assert!(close(ssq, 59.0 * 5.240921, 3e-2), "{ssq}");
    let fit = column(&r.engine, "FIT_1");
    // the first fit is missing: the differencing starts from the first value
    assert_eq!(fit[0], None);
    assert!(fit[1].is_some());
}

#[test]
fn arima_airline_model() {
    let r = run(&format!(
        "{}ARIMA bookings /MODEL=(0,1,1)(0,1,1) 12 NOCONSTANT /MXITER=50 /SSQPCT=1E-9 /PAREPS=1E-7.\n",
        data("bookings", RENTALS)
    ));
    let (ma, _) = estimate(&r.tables, "MA1");
    let (sma, _) = estimate(&r.tables, "SMA1");
    assert!(close(ma, -0.621122, 1e-3), "{ma}");
    // the likelihood is flat here: ours is a little higher at 0.7771
    assert!(close(sma, 0.773304, 5e-3), "{sma}");
    let fit = column(&r.engine, "FIT_1");
    assert_eq!(fit[12], None);
    assert!(fit[13].is_some());
}

#[test]
fn arima_with_a_regressor() {
    let r = run(&format!(
        "{}ARIMA sales WITH sun /MODEL=(1,0,0) /MXITER=50 /SSQPCT=1E-9 /PAREPS=1E-7.\n",
        data("sales sun", BAKERY)
    ));
    let (ar, _) = estimate(&r.tables, "AR1");
    let (sun, sun_se) = estimate(&r.tables, "sun");
    let (mu, _) = estimate(&r.tables, "Constant");
    assert!(close(ar, 0.511369, 5e-4), "{ar}");
    assert!(close(sun, 2.055648, 5e-4), "{sun}");
    assert!(close(mu, 9.748525, 2e-3), "{mu}");
    assert!(close(sun_se, 0.043216, 1e-3), "{sun_se}");
}

#[test]
fn arima_with_missing_values_inside_the_series() {
    // cases 20, 21 and 40 missing
    let mut values: Vec<&str> = TANK.split_whitespace().collect();
    for k in [19, 20, 39] {
        values[k] = ".";
    }
    let tank = values.join(" ");
    let r = run(&format!(
        "{}ARIMA level /MODEL=(1,0,1) /MXITER=50 /SSQPCT=1E-9 /PAREPS=1E-7.\n",
        data("level", &tank)
    ));
    let (ar, _) = estimate(&r.tables, "AR1");
    let (ma, _) = estimate(&r.tables, "MA1");
    assert!(close(ar, 0.642298, 5e-4), "{ar}");
    assert!(close(ma, -0.504445, 5e-4), "{ma}");
    // a missing case still gets a fit, but no residual
    let fit = column(&r.engine, "FIT_1");
    let err = column(&r.engine, "ERR_1");
    assert!(fit[19].is_some() && err[19].is_none());
    let d = run(&format!(
        "{}ARIMA level /MODEL=(0,1,1) NOCONSTANT /MXITER=50 /SSQPCT=1E-9 /PAREPS=1E-7.\n",
        data("level", &tank)
    ));
    let (ma, _) = estimate(&d.tables, "MA1");
    assert!(close(ma, -0.365979, 5e-4), "{ma}");
}

#[test]
fn arima_apply_fit_reuses_the_estimates() {
    let r = run(&format!(
        "{}ARIMA level /MODEL=(1,0,1) /MXITER=50 /SSQPCT=1E-9 /PAREPS=1E-7.\nTSET NEWVAR=ALL.\nARIMA APPLY FIT.\n",
        data("level", TANK)
    ));
    let first = column(&r.engine, "FIT_1");
    let second = column(&r.engine, "FIT_2");
    for (a, b) in first.iter().zip(&second) {
        assert!(close(a.unwrap(), b.unwrap(), 1e-9));
    }
    assert_eq!(find(&r.tables, "Iteration History").rows.len(), 1);
}

#[test]
fn arima_errors() {
    let tank = data("level", TANK);
    assert!(
        error(&format!("{tank}ARIMA level /MODEL=(0,0,0)(0,1,1).\n")).contains("needs a period")
    );
    assert!(error(&format!("{tank}ARIMA level /P=1 /AR=0.5 0.2.\n")).contains("AR gives 2"));
    let bakery = data("sales sun", BAKERY).replacen("4.7", ".", 1);
    assert!(
        error(&format!("{bakery}ARIMA sales WITH sun /MODEL=(0,1,0).\n"))
            .contains("regressor sun must be valid")
    );
    assert!(
        error(&format!("{tank}ARIMA level /P=1 /AR=1.5 /MXITER=0.\n")).contains("nonstationary")
    );
}

#[test]
fn use_limits_the_estimation_and_predict_forecasts_after_it() {
    let r = run(&format!(
        "{}USE 1 THRU 50.\nPREDICT 51 THRU 55.\nEXSMOOTH level /ALPHA=0.3.\n",
        data("level", TANK)
    ));
    let fit = column(&r.engine, "FIT_1");
    let err = column(&r.engine, "ERR_1");
    assert_eq!(fit.len(), 60);
    // flat forecasts of simple smoothing from case 50 on
    for k in 51..55 {
        assert_eq!(fit[k], fit[50]);
        assert_eq!(err[k], None);
    }
    assert!(fit[49].is_some() && err[49].is_some());
    assert_eq!(fit[55], None);
    // the start level is the mean of the first 50 cases
    let values: Vec<f64> = TANK
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    let mean = values[..50].iter().sum::<f64>() / 50.0;
    let start = find(&r.tables, "Initial Values");
    assert!(close(num(&start.rows[0][1]), mean, 1e-9));
    assert!(close(fit[0].unwrap(), mean, 1e-9));
}

#[test]
fn use_filters_other_procedures_and_all_restores() {
    let r = run(&format!(
        "{}USE 11 THRU 20.\nDESCRIPTIVES level.\n",
        data("level", TANK)
    ));
    let t = find(&r.tables, "Descriptive Statistics");
    assert_eq!(num(&t.rows[0][2]), 10.0);
    let r = run(&format!(
        "{}USE 11 THRU 20.\nUSE ALL.\nDESCRIPTIVES level.\n",
        data("level", TANK)
    ));
    assert_eq!(
        num(&find(&r.tables, "Descriptive Statistics").rows[0][2]),
        60.0
    );
}

#[test]
fn predict_with_dates_adds_dated_cases() {
    let r = run(&format!(
        "{}DATE YEAR 2020 MONTH 1.\nPREDICT THRU YEAR 2026 MONTH 3.\nEXSMOOTH bookings /MODEL=NA.\n",
        data("bookings", RENTALS)
    ));
    let fit = column(&r.engine, "FIT_1");
    assert_eq!(fit.len(), 75);
    let year = column(&r.engine, "YEAR_");
    let month = column(&r.engine, "MONTH_");
    assert_eq!((year[74], month[74]), (Some(2026.0), Some(3.0)));
    assert!(fit[72..].iter().all(Option::is_some));
    let show = run(&format!(
        "{}DATE YEAR 2020 MONTH 1.\nUSE YEAR 2021 THRU YEAR 2024.\nPREDICT YEAR 2025.\nTSHOW.\n",
        data("bookings", RENTALS)
    ));
    let rows = &show.tables[0].rows;
    let row = |name: &str| {
        rows.iter()
            .find(|r| matches!(&r[0], Cell::Text(s) if s == name))
            .map(|r| r[1].clone())
    };
    assert_eq!(
        row("USE"),
        Some(Cell::Text("YEAR 2021 THRU YEAR 2024".into()))
    );
    assert_eq!(
        row("PREDICT"),
        Some(Cell::Text("YEAR 2025 THRU LAST".into()))
    );
}

#[test]
fn exsmooth_grid_lists_the_smallest_sums() {
    let r = run(&format!(
        "{}EXSMOOTH bookings /MODEL=LA /PERIOD=12 /ALPHA=GRID(0.1 0.5 0.1) /GAMMA=GRID /DELTA=0.2.\n",
        data("bookings", RENTALS)
    ));
    let t = find(&r.tables, "Smallest Sums of Squared Errors");
    // 5 alphas times 6 gammas, ten shown, sorted
    assert_eq!(t.rows.len(), 10);
    let sums: Vec<f64> = t.rows.iter().map(|r| num(&r[3])).collect();
    assert!(sums.windows(2).all(|w| w[0] <= w[1]));
    let all = run(&format!(
        "{}TSET PRINT=DETAILED.\nEXSMOOTH bookings /MODEL=LA /PERIOD=12 /ALPHA=GRID(0.1 0.5 0.1) /GAMMA=GRID /DELTA=0.2.\n",
        data("bookings", RENTALS)
    ));
    assert_eq!(
        find(&all.tables, "Smallest Sums of Squared Errors")
            .rows
            .len(),
        30
    );
    // the saved fit is the best one's: its errors square to the smallest sum
    let err = column(&r.engine, "ERR_1");
    let sse: f64 = err.iter().flatten().map(|e| e * e).sum();
    assert!(close(sse, sums[0], 1e-6 * sums[0]));
}

#[test]
fn exsmooth_seasonal_start_values_by_hand() {
    // two periods of 2 repeated: means 2.5 and 4.5 per period
    let r = run(
        "DATA LIST FREE /y.\nBEGIN DATA\n1 4 3 6 5 8 7 10\nEND DATA.\n\
         EXSMOOTH y /MODEL=LA /PERIOD=2 /SEASFACT=(-1 1) /ALPHA=0.5 /GAMMA=0.5 /DELTA=0.5.\n",
    );
    let t = find(&r.tables, "Initial Values");
    // T0 = (m_4 - m_1) / ((k - 1) p) = (8.5 - 2.5) / 6 = 1; S0 = X_1 - p/2 T0 = 0
    assert_eq!(num(&t.rows[0][1]), 0.0);
    assert_eq!(num(&t.rows[1][1]), 1.0);
    assert_eq!(num(&t.rows[2][1]), -1.0);
    let fit = column(&r.engine, "FIT_1");
    // X^_0 = S0 + T0 + I_(1-p) = 0
    assert_eq!(fit[0], Some(0.0));
    // e_1 = 1; S_1 = 0 + 1 + 0.5 = 1.5; T_1 = 1 + 0.25 = 1.25; I_1 = -1 + 0.5 * 0.5 = -0.75
    // X^_1 = S_1 + T_1 + I_0 = 1.5 + 1.25 + 1 = 3.75
    assert_eq!(fit[1], Some(3.75));
}

#[test]
fn exsmooth_errors_and_apply() {
    let rentals = data("bookings", RENTALS);
    assert!(error(&format!("{rentals}EXSMOOTH bookings /MODEL=NA.\n")).contains("needs a period"));
    assert!(
        error(&format!(
            "{rentals}EXSMOOTH bookings /MODEL=NA /PERIOD=12 /SEASFACT=(1 2).\n"
        ))
        .contains("SEASFACT gives 2 values")
    );
    assert!(
        error("DATA LIST FREE /y.\nBEGIN DATA\n1 2 . 4\nEND DATA.\nEXSMOOTH y.\n")
            .contains("missing values")
    );
    let r = run(&format!(
        "{rentals}EXSMOOTH bookings /ALPHA=0.4.\nEXSMOOTH APPLY /ALPHA=0.2.\n"
    ));
    let t = find(&r.tables, "Smallest Sums of Squared Errors");
    assert_eq!(num(&t.rows[0][0]), 0.2);
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
}

#[test]
fn arima_fit_methods_by_hand() {
    // AR(1) with phi 0.5 and mean 10, not estimated
    let tiny = "DATA LIST FREE /y.\nBEGIN DATA\n12 8 11\nEND DATA.\nPREDICT THRU 5.\n";
    let fixed = "/MODEL=(1,0,0) /AR=0.5 /CON=10 /MXITER=0";
    let cls = run(&format!("{tiny}ARIMA y {fixed} /FORECAST=CLS.\n"));
    let fit = column(&cls.engine, "FIT_1");
    assert_eq!(
        fit,
        vec![Some(10.0), Some(11.0), Some(9.0), Some(10.5), Some(10.25)]
    );
    let sep = column(&cls.engine, "SEP_1");
    let sigma = sep[0].unwrap();
    assert!(close(sep[4].unwrap(), sigma * 1.25f64.sqrt(), 1e-12));
    let auto = run(&format!("{tiny}ARIMA y {fixed} /FORECAST=AUTOINIT.\n"));
    let fit = column(&auto.engine, "FIT_1");
    assert_eq!(
        fit,
        vec![None, Some(11.0), Some(9.0), Some(10.5), Some(10.25)]
    );
    let exact = run(&format!("{tiny}ARIMA y {fixed}.\n"));
    let fit = column(&exact.engine, "FIT_1");
    assert_eq!(fit[..3], [Some(10.0), Some(11.0), Some(9.0)]);
    let sep = column(&exact.engine, "SEP_1");
    // the first prediction has the variance of the process, 1 / (1 - 0.25) innovations
    assert!(close(
        sep[0].unwrap() / sep[1].unwrap(),
        (4.0f64 / 3.0).sqrt(),
        1e-12
    ));
    // logs: the fit goes back to the original metric
    let logged = run(&format!(
        "{tiny}ARIMA y /MODEL=(0,0,0) LN /CON=2.302585092994046 /MXITER=0.\n"
    ));
    let fit = column(&logged.engine, "FIT_1");
    assert!(close(fit[0].unwrap(), 10.0, 1e-9));
    let err = column(&logged.engine, "ERR_1");
    assert!(close(err[0].unwrap(), (12.0f64 / 10.0).ln(), 1e-9));
}

#[test]
fn tsplot_and_caseplot_charts() {
    use statscraft_engine::chart::{ChartKind, Orientation};
    let small = "DATA LIST FREE /a b c switch.\nBEGIN DATA\n\
                 4 1 2 0  6 3 5 0  5 8 1 1  9 2 7 1\nEND DATA.\n";
    let r = run(&format!(
        "{small}TSPLOT a /LN /DIFF /FORMAT=REFERENCE /MARK=switch.\n"
    ));
    let charts = r.engine.clone().take_charts();
    let chart = &charts[0];
    assert_eq!(chart.kind, ChartKind::Line);
    assert_eq!(chart.orientation, Some(Orientation::Vertical));
    let v = &chart.series[0].values;
    assert!(v[0].is_nan());
    assert!(close(v[1], (6.0f64 / 4.0).ln(), 1e-12));
    // the reference line at the mean of the differences
    let mean = ((9.0f64 / 4.0).ln()) / 3.0;
    assert!(
        chart.series[1]
            .values
            .iter()
            .all(|r| close(*r, mean, 1e-12))
    );
    assert_eq!(
        chart.footnote.as_deref(),
        Some("Transformations: natural log, difference (1). Marked at 3.")
    );
    let r = run(&format!("{small}CASEPLOT a b c /FORMAT=HILO.\n"));
    let charts = r.engine.clone().take_charts();
    assert_eq!(charts[0].kind, ChartKind::HighLow);
    assert_eq!(charts[0].orientation, Some(Orientation::Horizontal));
    assert_eq!(charts[0].series[0].values, vec![4.0, 6.0, 8.0, 9.0]);
    assert_eq!(charts[0].series[1].values, vec![1.0, 3.0, 1.0, 2.0]);
    // APPLY keeps the format; dates label the cases
    let r = run(&format!(
        "{small}DATE YEAR 2001.\nCASEPLOT a b /FORMAT=JOIN.\nCASEPLOT c b /APPLY.\n"
    ));
    let charts = r.engine.clone().take_charts();
    assert_eq!(charts[0].kind, ChartKind::DropLine);
    assert_eq!(charts[0].categories[0].label, "2001");
    assert_eq!(charts[0].series[0].name, "c");
}

#[test]
fn arima_forecasts_inside_the_series_stop_at_the_predict_end() {
    let r = run(&format!(
        "{}PREDICT 51 THRU 55.\nARIMA level /MODEL=(1,0,0) /FORECAST=CLS.\n",
        data("level", TANK)
    ));
    let fit = column(&r.engine, "FIT_1");
    let err = column(&r.engine, "ERR_1");
    assert_eq!(fit.len(), 60);
    assert!(err[49].is_some() && err[50].is_none());
    assert!(fit[54].is_some() && fit[55].is_none());
    // the forecasts decay toward the mean from case 50 (50.3) on
    let (ar, _) = estimate(&r.tables, "AR1");
    let (mu, _) = estimate(&r.tables, "Constant");
    let expected = mu + ar * ar * (50.3 - mu);
    assert!(close(fit[51].unwrap(), expected, 1e-9));
}

#[test]
fn a_failed_arima_keeps_the_model_apply_reuses() {
    let source = format!(
        "{}ARIMA level /MODEL=(1,0,0).\nARIMA level /MODEL=(0,0,0)(0,1,1).\nTSET NEWVAR=ALL.\nARIMA APPLY FIT.\n",
        data("level", TANK)
    );
    let (commands, errors) = parse(&source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let results: Vec<bool> = commands
        .iter()
        .map(|c| engine.execute(&c.command).is_ok())
        .collect();
    // the seasonal model fails for want of a period; APPLY FIT uses the AR(1) estimates
    assert_eq!(results[results.len() - 3..], [false, true, true]);
    let first = column(&engine, "FIT_1");
    let second = column(&engine, "FIT_2");
    assert_eq!(first, second);
}

#[test]
fn exsmooth_seasonal_forecasts_from_early_in_the_series() {
    let r = run(&format!(
        "{}PREDICT 2 THRU 14.\nEXSMOOTH bookings /MODEL=NA /PERIOD=12 /ALPHA=0.2 /DELTA=0.1.\n",
        data("bookings", RENTALS)
    ));
    let fit = column(&r.engine, "FIT_1");
    let start = find(&r.tables, "Initial Values");
    // from case 1: the level after it plus the start factor of each later month
    let factor = |k: usize| num(&start.rows[1 + k][1]);
    let level_gap = fit[1].unwrap() - factor(1);
    for (k, f) in fit.iter().enumerate().take(12).skip(2) {
        assert!(close(f.unwrap() - factor(k), level_gap, 1e-9), "{k}");
    }
    assert_eq!(fit[14], None);
}
