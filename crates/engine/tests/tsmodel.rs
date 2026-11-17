//! TSMODEL and TSAPPLY on invented series (random draws made for these tests). The ARIMA
//! numbers come from a general statistics package run as a black box (exact maximum
//! likelihood; its moving-average sign is the opposite of ours). The smoothing fits are checked
//! against our EXSMOOTH recursions, and the fit measures are recomputed here from the saved
//! fitted values with the documented formulas.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// An ARMA(1,1) series around 20.
const ARMA: &str = "17.5 17.6 20.8 20.4 20.5 19.3 21.9 22.4 17.6 17.7 16.3 19.4 20.5 19.5 18.8 \
17.6 19.6 18.3 18.3 16.5 17.0 18.1 18.8 19.2 21.4 21.8 21.6 22.3 24.0 22.5 20.3 21.2 19.1 20.3 \
22.6 23.5 24.4 20.1 18.5 17.7 19.5 19.7 20.8 20.3 22.2 20.0 21.4 22.1 21.0 21.1 21.5 21.8 22.6 \
21.3 20.0 19.5 21.2 20.2 20.1 17.4";

/// A linear trend with noise.
const TREND: &str = "10.9 11.8 14.0 15.0 13.4 17.2 19.5 16.6 16.6 21.5 19.9 20.8 20.5 23.3 21.8 \
23.1 23.7 25.0 27.2 26.8 26.7 30.0 32.5 31.2 28.6 31.1 33.3 33.7 34.3 37.5";

/// Four years of monthly values with a trend and a season.
const MONTHLY: &str = "95.0 97.3 98.0 104.0 105.7 111.8 112.4 109.4 106.8 105.1 103.5 100.2 \
98.8 102.5 105.7 109.9 111.8 115.7 117.3 116.1 113.5 112.0 105.9 106.5 106.2 108.8 110.4 115.3 \
118.7 122.7 122.7 121.5 121.6 117.4 114.3 113.3 111.2 115.1 119.0 121.9 126.0 128.8 130.0 128.9 \
124.0 120.5 118.6 117.4";

/// An AR(1) series around 50.
const AR1: &str = "49.2 49.4 48.7 46.6 52.9 51.5 48.7 50.2 48.8 47.9 47.9 48.7 47.0 48.8 45.4 \
48.5 48.0 47.9 45.8 47.1 46.3 51.1 52.5 53.9 48.7 48.3 46.8 51.8 52.1 50.2 49.6 50.1 49.6 47.5 \
49.2 51.6 49.4 49.1 48.1 48.2 46.1 47.2 50.1 47.5 48.2 46.2 44.1 46.4 50.7 49.6 48.3 51.3 51.6 \
54.2 51.6 51.0 50.3 49.4 45.7 49.0 50.3 49.5 50.3 52.4 48.4 48.0 47.7 49.8 51.7 51.4 52.4 51.1 \
49.9 47.8 49.6 49.5 51.4 50.8 52.3 51.5";

fn data(name: &str, values: &str) -> String {
    format!("DATA LIST FREE /{name}.\nBEGIN DATA\n{values}\nEND DATA.\n")
}

fn run_in(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        tables = engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
    }
    tables
}

fn run(source: &str) -> (Vec<Table>, Engine) {
    let mut engine = Engine::default();
    let tables = run_in(&mut engine, source);
    (tables, engine)
}

fn error(source: &str) -> String {
    let (commands, errors) = parse(source);
    if let Some(e) = errors.first() {
        return e.to_string();
    }
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

fn texts(cell: &Cell) -> String {
    match cell {
        Cell::Text(s) => s.clone(),
        other => panic!("not text: {other:?}"),
    }
}

/// The estimate and standard error of a parameter row.
fn parameter(tables: &[Table], title: &str, name: &str) -> (f64, f64) {
    let row = find(tables, title)
        .rows
        .iter()
        .find(|r| texts(&r[1]) == name)
        .unwrap_or_else(|| panic!("no parameter {name}"));
    (num(&row[2]), num(&row[3]))
}

fn arima_estimate(tables: &[Table], name: &str) -> f64 {
    let row = find(tables, "Parameter Estimates")
        .rows
        .iter()
        .find(|r| texts(&r[0]) == name)
        .unwrap_or_else(|| panic!("no row {name}"));
    num(&row[1])
}

fn column(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().expect("a dataset");
    let v = data.variable(name).expect("the variable");
    let values = v.numeric().expect("numeric");
    (0..data.case_count()).map(|k| values.get(k)).collect()
}

fn series(values: &str) -> Vec<f64> {
    values
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect()
}

fn close(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
}

fn model_type(tables: &[Table]) -> String {
    texts(&find(tables, "Model Description").rows[0][2])
}

#[test]
fn arima_through_tsmodel_matches_arima_and_the_reference() {
    let source = format!(
        "{}TSMODEL /MODELDETAILS PRINT=[PARAMETERS FORECASTS] /MODEL DEPENDENT=y \
         /ARIMA AR=[1] MA=[1].\n",
        data("y", ARMA)
    );
    let (tables, _) = run(&source);
    let (ar, ar_se) = parameter(&tables, "ARIMA Model Parameters", "AR Lag 1");
    let (ma, _) = parameter(&tables, "ARIMA Model Parameters", "MA Lag 1");
    let (mu, _) = parameter(&tables, "ARIMA Model Parameters", "Constant");
    // the ARIMA command with the same stopping rules gives the same estimates
    let (arima, _) = run(&format!(
        "{}ARIMA y /MODEL=(1,0,1) /MXITER=50 /SSQPCT=1E-6 /PAREPS=1E-6.\n",
        data("y", ARMA)
    ));
    assert!(close(ar, arima_estimate(&arima, "AR1"), 1e-12));
    assert!(close(ma, arima_estimate(&arima, "MA1"), 1e-12));
    assert!(close(mu, arima_estimate(&arima, "Constant"), 1e-12));
    // the reference package: ar 0.5655324, ma -0.1161432 (sign flipped), mean 20.01653
    assert!(close(ar, 0.5655324, 2e-4), "{ar}");
    assert!(close(ma, -0.1161432, 2e-4), "{ma}");
    assert!(close(mu, 20.01653, 2e-3), "{mu}");
    assert!(close(ar_se, 0.1541656, 0.02), "{ar_se}");
    // no PREDICT: no forecasts
    assert!(find(&tables, "Forecast").rows.is_empty());
}

#[test]
fn arima_fit_measures_and_ljung_box() {
    let source = format!(
        "{}TSMODEL /MODELSTATISTICS MODELFIT=[SRSQUARE RSQUARE RMSE MAPE MAE MAXAPE MAXAE NORMBIC] \
         /SAVE PREDICTED NRESIDUAL(res) /MODEL DEPENDENT=y PREFIX='Lab' /ARIMA AR=1 MA=1.\n",
        data("y", ARMA)
    );
    let (tables, engine) = run(&source);
    let y = series(ARMA);
    let fit = column(&engine, "Predicted_y_Lab_1");
    let res = column(&engine, "res_y_Lab_1");
    let n = y.len() as f64;
    let k = 3.0;
    let e: Vec<f64> = y.iter().zip(&fit).map(|(a, f)| a - f.unwrap()).collect();
    for (a, r) in e.iter().zip(&res) {
        assert!(close(*a, r.unwrap(), 1e-9));
    }
    let sse: f64 = e.iter().map(|v| v * v).sum();
    let mean = y.iter().sum::<f64>() / n;
    let sst: f64 = y.iter().map(|v| (v - mean).powi(2)).sum();
    let mse = sse / (n - k);
    let ape: Vec<f64> = e
        .iter()
        .zip(&y)
        .map(|(e, y)| 100.0 * (e / y).abs())
        .collect();
    let expected = [
        1.0 - sse / sst, // stationary R squared: no differencing, so the plain one
        1.0 - sse / sst,
        mse.sqrt(),
        ape.iter().sum::<f64>() / n,
        e.iter().map(|v| v.abs()).sum::<f64>() / n,
        ape.iter().copied().fold(0.0, f64::max),
        e.iter().map(|v| v.abs()).fold(0.0, f64::max),
        mse.ln() + k * n.ln() / n,
    ];
    let row = &find(&tables, "Model Statistics").rows[0];
    assert_eq!(texts(&row[0]), "y-Lab_1");
    for (i, v) in expected.iter().enumerate() {
        assert!(
            close(num(&row[2 + i]), *v, 1e-9),
            "measure {i}: {:?} {v}",
            row[2 + i]
        );
    }
    // Ljung-Box Q(18) with 16 degrees of freedom, by hand from the saved residuals
    let r: Vec<f64> = res.iter().map(|v| v.unwrap()).collect();
    let rm = r.iter().sum::<f64>() / n;
    let c0: f64 = r.iter().map(|v| (v - rm).powi(2)).sum();
    let q: f64 = n
        * (n + 2.0)
        * (1..=18)
            .map(|k| {
                let ck: f64 = (k..r.len()).map(|t| (r[t] - rm) * (r[t - k] - rm)).sum();
                (ck / c0).powi(2) / (n - k as f64)
            })
            .sum::<f64>();
    assert!(close(num(&row[10]), q, 1e-9), "{:?} {q}", row[10]);
    assert_eq!(num(&row[11]), 16.0);
    // the reference package gives 7.0349 (p 0.9726) on its own residuals of the same fit, whose
    // first innovations differ a little from ours
    assert!(close(q, 7.0349, 0.5), "{q}");
    assert!(close(num(&row[12]), 0.9726, 0.02), "{:?}", row[12]);
    // one model: the summary's mean, minimum and maximum are its values
    let summary = &find(&tables, "Model Fit").rows[2];
    assert_eq!(texts(&summary[0]), "RMSE");
    assert!(close(num(&summary[1]), mse.sqrt(), 1e-9));
    assert!(close(num(&summary[3]), mse.sqrt(), 1e-9));
}

#[test]
fn exsmooth_through_tsmodel_runs_the_exsmooth_recursions() {
    let source = format!(
        "{}TSMODEL /MODELDETAILS PRINT=PARAMETERS /SAVE PREDICTED /MODEL DEPENDENT=y \
         /EXSMOOTH TYPE=HOLT.\n",
        data("y", TREND)
    );
    let (tables, engine) = run(&source);
    let (alpha, _) = parameter(
        &tables,
        "Exponential Smoothing Model Parameters",
        "Alpha (Level)",
    );
    let (gamma, _) = parameter(
        &tables,
        "Exponential Smoothing Model Parameters",
        "Gamma (Trend)",
    );
    assert!((0.0..=1.0).contains(&alpha) && (0.0..=1.0).contains(&gamma));
    let fit: Vec<f64> = column(&engine, "Predicted_y_Model_1")
        .into_iter()
        .map(Option::unwrap)
        .collect();
    let y = series(TREND);
    // the start states behind the first two fits: fit_1 = L0 + T0 and
    // fit_2 = fit_1 + T0 + alpha (1 + gamma) e_1
    let t0 = fit[1] - fit[0] - alpha * (1.0 + gamma) * (y[0] - fit[0]);
    let l0 = fit[0] - t0;
    // EXSMOOTH with those weights and start states makes the same fits
    let mut engine = Engine::default();
    run_in(
        &mut engine,
        &format!(
            "{}EXSMOOTH y /MODEL=HOLT /ALPHA={alpha} /GAMMA={gamma} /INITIAL=({l0},{t0}).\n",
            data("y", TREND)
        ),
    );
    for (a, b) in column(&engine, "FIT_1").iter().zip(&fit) {
        assert!(close(a.unwrap(), *b, 1e-9), "{a:?} {b}");
    }
    // the weights minimize the sum of squared one-step errors: nearby weights fit worse
    let sse = |a: f64, g: f64| {
        let (mut l, mut t, mut s) = (l0, t0, 0.0);
        for v in &y {
            let e = v - (l + t);
            s += e * e;
            (l, t) = (l + t + a * e, t + a * g * e);
        }
        s
    };
    let best = sse(alpha, gamma);
    for (da, dg) in [(0.01, 0.0), (-0.01, 0.0), (0.0, 0.01), (0.0, -0.01)] {
        let (a, g) = ((alpha + da).clamp(0.0, 1.0), (gamma + dg).clamp(0.0, 1.0));
        if (a, g) != (alpha, gamma) {
            // the start states move with the weights, so allow a little
            assert!(sse(a, g) >= best - 0.05 * best, "{a} {g}");
        }
    }
    assert_eq!(model_type(&tables), "Holt");
}

#[test]
fn simple_smoothing_starts_from_the_backward_pass() {
    // y = 4, 6, 5, 9; alpha fixed by TSAPPLY's file is not needed: check the start by hand
    // for the optimal alpha the command reports
    let source = format!(
        "{}TSMODEL /MODELDETAILS PRINT=PARAMETERS /SAVE PREDICTED /MODEL DEPENDENT=y \
         /EXSMOOTH TYPE=SIMPLE.\n",
        data("y", "4 6 5 9 7 8 6 7")
    );
    let (tables, engine) = run(&source);
    let (a, _) = parameter(
        &tables,
        "Exponential Smoothing Model Parameters",
        "Alpha (Level)",
    );
    let y = series("4 6 5 9 7 8 6 7");
    // backwards from the last value over the reversed series, then forwards
    let mut level = *y.last().unwrap();
    for v in y.iter().rev() {
        level += a * (v - level);
    }
    let fit = column(&engine, "Predicted_y_Model_1");
    assert!(close(fit[0].unwrap(), level, 1e-9));
    for t in 0..y.len() - 1 {
        level += a * (y[t] - level);
        assert!(close(fit[t + 1].unwrap(), level, 1e-9));
    }
}

#[test]
fn tsapply_repeats_the_saved_forecasts_on_extended_data() {
    let dir = std::env::temp_dir().join(format!("statscraft-tsmodel-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("models.json");
    let file = file.display();
    let values = series(AR1);
    let first: String = values[..70]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let mut engine = Engine::default();
    let original = run_in(
        &mut engine,
        &format!(
            "{}PREDICT THRU 76.\nTSMODEL /MODELDETAILS PRINT=FORECASTS \
             /MODEL DEPENDENT=y OUTFILE='{file}' /ARIMA AR=1 MA=0.\n",
            data("y", &first)
        ),
    );
    let forecasts = find(&original, "Forecast");
    assert_eq!(forecasts.rows.len(), 6);
    assert_eq!(texts(&forecasts.rows[0][1]), "71");
    // the extended file: TSAPPLY without reestimation repeats the forecasts
    let applied = run_in(
        &mut engine,
        &format!(
            "{}PREDICT THRU 76.\nTSAPPLY /MODELDETAILS PRINT=[PARAMETERS FORECASTS] \
             /MODEL FILE='{file}'.\n",
            data("y", AR1)
        ),
    );
    let again = find(&applied, "Forecast");
    assert_eq!(again.rows.len(), 6);
    for (a, b) in forecasts.rows.iter().zip(&again.rows) {
        for c in 2..5 {
            assert!(close(num(&a[c]), num(&b[c]), 1e-9));
        }
    }
    // reestimated on all 80 cases it agrees with TSMODEL on them
    let reestimated = run_in(
        &mut engine,
        &format!(
            "{}PREDICT THRU 82.\nTSAPPLY /AUXILIARY REESTIMATE=YES /MODELDETAILS PRINT=PARAMETERS \
             /MODEL FILE='{file}' KEEP=['Model_1'].\n",
            data("y", AR1)
        ),
    );
    let fresh = run_in(
        &mut engine,
        &format!(
            "{}TSMODEL /MODELDETAILS PRINT=PARAMETERS /MODEL DEPENDENT=y /ARIMA AR=1 MA=0.\n",
            data("y", AR1)
        ),
    );
    let title = "ARIMA Model Parameters";
    assert!(close(
        parameter(&reestimated, title, "AR Lag 1").0,
        parameter(&fresh, title, "AR Lag 1").0,
        1e-12
    ));
    assert!(
        error(&format!(
            "TSAPPLY /MODEL FILE='{file}' KEEP=['a'] DROP=['b'].\n"
        ))
        .contains("exclude")
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_expert_modeler_picks_clear_cut_models() {
    // a seasonal series with a trend: a seasonal smoothing method
    let (tables, _) = run(&format!(
        "{}DATE YEAR 2020 MONTH 1.\nTSMODEL /MODEL DEPENDENT=y /EXPERTMODELER TYPE=[EXSMOOTH].\n",
        data("y", MONTHLY)
    ));
    let chosen = model_type(&tables);
    assert!(chosen.starts_with("Winters"), "{chosen}");
    // without seasonal models the same series gets a trend method
    let (tables, _) = run(&format!(
        "{}DATE YEAR 2020 MONTH 1.\nTSMODEL /MODEL DEPENDENT=y \
         /EXPERTMODELER TYPE=[EXSMOOTH] TRYSEASONAL=NO.\n",
        data("y", MONTHLY)
    ));
    let chosen = model_type(&tables);
    assert!(
        !chosen.contains("Winters") && !chosen.contains("Seasonal"),
        "{chosen}"
    );
    // an autoregressive series of order 1: ARIMA(1,0,0)
    let (tables, _) = run(&format!(
        "{}TSMODEL /MODEL DEPENDENT=y /EXPERTMODELER TYPE=[ARIMA].\n",
        data("y", AR1)
    ));
    assert_eq!(model_type(&tables), "ARIMA(1,0,0)");
    // a short series goes to the smoothing expert even when ARIMA is allowed
    let (tables, _) = run(&format!(
        "{}TSMODEL /MODEL DEPENDENT=y.\n",
        data("y", "5 7 6 8 7 9 8 10 9 11 10 12")
    ));
    let chosen = model_type(&tables);
    assert!(!chosen.starts_with("ARIMA"), "{chosen}");
}

#[test]
fn tsmodel_errors_and_unsupported_parts() {
    let base = data("y", TREND);
    for (syntax, wanted) in [
        (
            "TSMODEL /MODEL DEPENDENT=y /OUTLIER LOCATION=[3] TYPE=ADDITIVE.",
            "not supported",
        ),
        (
            "TSMODEL /MODEL DEPENDENT=y /TRANSFERFUNCTION NUM=[1].",
            "not supported",
        ),
        (
            "TSMODEL /MODEL DEPENDENT=y /AUTOOUTLIER DETECT=ON.",
            "not supported",
        ),
        (
            "TSMODEL /MODEL DEPENDENT=y /ARIMA AR=0 MA=0 CONSTANT=NO.",
            "no parameter",
        ),
        (
            "TSMODEL /MODEL DEPENDENT=y /EXSMOOTH TYPE=WINTERSADDITIVE.",
            "season length",
        ),
        (
            "TSMODEL /MODEL DEPENDENT=y /SAVE PREDICTED.",
            "before the first",
        ),
        (
            "TSMODEL /MODEL DEPENDENT=y /ARIMA AR=1 MA=0 /EXSMOOTH TYPE=SIMPLE.",
            "one of",
        ),
        (
            "TSMODEL /AUXILIARY CILEVEL=100 /MODEL DEPENDENT=y.",
            "CILEVEL",
        ),
    ] {
        let message = error(&format!("{base}{syntax}\n"));
        assert!(message.contains(wanted), "{syntax}: {message}");
    }
}
