//! TSMODEL and TSAPPLY on invented series (random draws made for these tests). The ARIMA
//! numbers come from a general statistics package run as a black box (exact maximum
//! likelihood; its moving-average sign is the opposite of ours). The smoothing fits are checked
//! against our EXSMOOTH recursions, and the fit measures are recomputed here from the saved
//! fitted values with the documented formulas.

use statscraft_engine::chart::ChartKind;
use statscraft_engine::plot::Plot;
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
        "{}ARIMA y /MODEL=(1,0,1) /MXITER=200 /SSQPCT=1E-12 /PAREPS=1E-9.\n",
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
            "TSMODEL /MODEL DEPENDENT=y OUTPMML='m.xml'.",
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

/// An invented predictor (an AR(1) series around 50) and a dependent made from it by the
/// transfer function (1 + 1.5 B) / (1 - 0.6 B) plus AR(1) noise, random draws rounded.
const TF_X: &str = "50 49.8 53 50.5 50.7 53.4 55.7 54.6 55 53.8 57.3 55.7 55.4 55.5 55.5 52.3 \
53.4 53 50.3 52.1 46.3 54.9 50.8 48.9 53.3 51.1 51.6 49.5 50.5 51.6 49.4 49.8 51 51.4 50.8 50.4 \
51.2 49.8 49.6 51.5 50.3 52.6 50.7 52.7 48.4 48.5 55.1 56.2 49.8 49.4 55.5 55.9 58.5 53.7 53.8 \
53.9 51.1 54.9 53.4 49.3 47.5 52 57.8 50 51.8 50.5 53.7 48.7 50.9 51.1 47.7 53.3 49.9 56.3 54.1 \
47.2 49 49.9 54.3 57.6";
const TF_Y: &str = "13.14 15.31 17.33 19.96 17.47 19.96 28.7 35.12 37.19 38.82 40.67 45.81 \
46.24 47.51 48.4 44.55 37.5 33.32 30.99 25 20.04 16.93 23.31 16.69 16.25 21.68 22.26 20.18 16.56 \
16.1 18.03 14.44 14.54 17.75 18.36 16.94 16.14 16.09 15.24 14.76 16.35 16.75 20.43 22.3 21.84 \
12.27 15.36 30.03 31.76 22.61 23.53 34.38 42.97 46.25 43.03 40.53 34.69 32.07 35.22 29.86 20.19 \
17.12 25.49 32.06 27.57 24.26 23.09 22.4 17.78 19.37 15.19 12.37 18.74 22.26 33.35 28.55 17.55 \
15.4 18.32 31.05";

/// An invented AR(1) series around 30 with an additive outlier planted at case 25 and a level
/// shift of 4 from case 55 (random draws, rounded).
const SHIFTED: &str = "27.79 28.65 29.19 29.56 30.06 30.62 31.33 32.77 31.54 31.68 30.59 31.81 \
32.69 30.47 28.7 29.49 29.03 28.11 30.88 31.73 28.5 28.7 29.05 28.77 35.53 29.22 29.77 29.8 29.98 \
30.89 30.45 29.69 29.14 29.28 30.52 30.11 29.1 30.22 30.5 30.8 29.09 30.43 32.55 31.78 28.62 \
27.28 28.5 28.3 30.73 29.12 29.96 30 29 29.51 33.59 34.22 33.69 33.37 33.86 35.78 33.25 33.29 \
34.09 34.32 33.91 36.3 34.94 34.52 32.01 33.93 34.71 34.29 33.64 33.66 33.74 34.34 34.3 34.87 \
34.43 32.04 33.9 34.78 34.34 30.76 30.87 32.69 32.87 34.41 36.64 35.24";

fn two_columns(x: &str, y: &str) -> String {
    let rows: Vec<String> = x
        .split_whitespace()
        .zip(y.split_whitespace())
        .map(|(a, b)| format!("{a} {b}"))
        .collect();
    format!(
        "DATA LIST FREE /x y.\nBEGIN DATA\n{}\nEND DATA.\n",
        rows.join("\n")
    )
}

fn arima_parameter(tables: &[Table], name: &str) -> f64 {
    parameter(tables, "ARIMA Model Parameters", name).0
}

fn near(got: f64, wanted: f64) -> bool {
    close(got, wanted, 1e-6 * wanted.abs().max(1.0))
}

#[test]
fn transfer_function_matches_the_profiled_likelihood() {
    let (tables, _) = run(&format!(
        "{}TSMODEL /MODELDETAILS PRINT=PARAMETERS /MODEL DEPENDENT=y INDEPENDENT=x \
         /ARIMA AR=1 MA=0 /TRANSFERFUNCTION NUM=1 DENOM=1.\n",
        two_columns(TF_X, TF_Y)
    ));
    // the other package's exact likelihood, the numerator columns built for each denominator
    // value and the likelihood maximized over that value; presample values of x are its first
    for (name, wanted) in [
        ("AR Lag 1", 0.374290183),
        ("Numerator Lag 0 (x)", 1.024190098),
        ("Numerator Lag 1 (x)", -1.549640526),
        ("Denominator Lag 1 (x)", 0.576290350),
        ("Constant", -290.672385610),
    ] {
        let got = arima_parameter(&tables, name);
        assert!(near(got, wanted), "{name}: {got} {wanted}");
    }
    assert_eq!(model_type(&tables), "ARIMA(1,0,0)");
}

#[test]
fn a_prepared_predictor_enters_logged_differenced_and_delayed() {
    let (tables, _) = run(&format!(
        "{}TSMODEL /MODELDETAILS PRINT=PARAMETERS /MODEL DEPENDENT=y INDEPENDENT=x \
         /ARIMA AR=1 MA=0 /TRANSFERFUNCTION VARIABLES=x TRANSFORM=LN DIFF=1 DELAY=1.\n",
        two_columns(TF_X, TF_Y)
    ));
    // the other package with the prepared column as a regressor, its two leading gaps set to
    // its first value
    for (name, wanted) in [
        ("AR Lag 1", 0.900409281),
        (
            "Numerator Lag 0 (x, natural log, difference 1, delay 1)",
            25.404128252,
        ),
        ("Constant", 24.354732841),
    ] {
        let got = arima_parameter(&tables, name);
        assert!(near(got, wanted), "{name}: {got} {wanted}");
    }
}

fn outlier_rows(tables: &[Table]) -> Vec<(String, String, f64)> {
    find(tables, "Outliers")
        .rows
        .iter()
        .map(|r| (texts(&r[1]), texts(&r[2]), num(&r[3])))
        .collect()
}

#[test]
fn given_outliers_are_regressors_of_their_shape() {
    let (tables, _) = run(&format!(
        "{}TSMODEL /MODELDETAILS PRINT=PARAMETERS /MODEL DEPENDENT=y /ARIMA AR=1 MA=0 \
         /OUTLIER LOCATION=[25] TYPE=ADDITIVE /OUTLIER LOCATION=[55] TYPE=LEVELSHIFT \
         /OUTLIER LOCATION=[84] TYPE=TRANSIENT.\n",
        data("y", SHIFTED)
    ));
    // the other package with a pulse, a step and a decay of 0.7 per case as regressors
    let rows = outlier_rows(&tables);
    let wanted = [
        ("25", "Additive", 6.303905702),
        ("55", "Level Shift", 4.357054481),
        ("84", "Transient", -3.447156896),
    ];
    assert_eq!(rows.len(), 3);
    for ((period, kind, value), (p, k, w)) in rows.iter().zip(wanted) {
        assert_eq!((period.as_str(), kind.as_str()), (p, k));
        assert!(near(*value, w), "{kind}: {value} {w}");
    }
    assert!(near(arima_parameter(&tables, "AR Lag 1"), 0.443731160));
    assert!(near(arima_parameter(&tables, "Constant"), 29.888873239));
    let statistics = &find(&tables, "Model Statistics").rows[0];
    assert_eq!(num(statistics.last().unwrap()), 3.0);
}

#[test]
fn an_innovational_outlier_passes_through_the_noise_model() {
    let (tables, _) = run(&format!(
        "{}TSMODEL /MODELDETAILS PRINT=PARAMETERS /MODEL DEPENDENT=y /ARIMA AR=1 MA=0 \
         /OUTLIER LOCATION=[25] TYPE=INNOVATIONAL.\n",
        data("y", SHIFTED)
    ));
    // the other package's exact likelihood of y less k phi^(t - 25) from case 25, maximized
    // over phi, the mean and k directly
    let rows = outlier_rows(&tables);
    assert_eq!(rows[0].1, "Innovational");
    assert!(near(rows[0].2, 6.242765430), "{}", rows[0].2);
    assert!(near(arima_parameter(&tables, "AR Lag 1"), 0.793542207));
    assert!(near(arima_parameter(&tables, "Constant"), 31.275280827));
}

#[test]
fn the_outlier_search_finds_the_planted_outliers() {
    let (tables, _) = run(&format!(
        "{}TSMODEL /MODELDETAILS PRINT=PARAMETERS /MODEL DEPENDENT=y /ARIMA AR=1 MA=0 \
         /AUTOOUTLIER DETECT=ON.\n",
        data("y", SHIFTED)
    ));
    // the same search written in the other package (median absolute deviation, critical
    // value 3.1 for 90 values, joint refits by its exact likelihood) finds the same two
    let rows = outlier_rows(&tables);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!((rows[0].0.as_str(), rows[0].1.as_str()), ("25", "Additive"));
    assert_eq!(
        (rows[1].0.as_str(), rows[1].1.as_str()),
        ("55", "Level Shift")
    );
    assert!(near(rows[0].2, 6.3244811731), "{}", rows[0].2);
    assert!(near(rows[1].2, 4.0843592539), "{}", rows[1].2);
    assert!(near(arima_parameter(&tables, "AR Lag 1"), 0.4623626483));
    // the expert modeler searches its ARIMA model the same way
    let (tables, _) = run(&format!(
        "{}TSMODEL /MODELDETAILS PRINT=PARAMETERS /MODEL DEPENDENT=y \
         /EXPERTMODELER TYPE=[ARIMA] /AUTOOUTLIER DETECT=ON.\n",
        data("y", SHIFTED)
    ));
    let rows = outlier_rows(&tables);
    assert!(rows.iter().any(|r| r.0 == "25"), "{rows:?}");
    // a series without outliers gets none
    let (tables, _) = run(&format!(
        "{}TSMODEL /MODELDETAILS PRINT=PARAMETERS /MODEL DEPENDENT=y /ARIMA AR=1 MA=0 \
         /AUTOOUTLIER DETECT=ON.\n",
        data("y", AR1)
    ));
    assert!(!tables.iter().any(|t| t.title == "Outliers"));
}

#[test]
fn tsapply_keeps_transfer_functions_and_outliers() {
    let dir = std::env::temp_dir().join(format!("statscraft-tsmodel-tf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("tf.json");
    let file = file.display();
    // the last five values of y are missing: forecasts from the predictor's values there
    let y: Vec<&str> = TF_Y.split_whitespace().take(75).chain(["."; 5]).collect();
    let source = two_columns(TF_X, &y.join(" "));
    let mut engine = Engine::default();
    let original = run_in(
        &mut engine,
        &format!(
            "{source}TSMODEL /MODELDETAILS PRINT=FORECASTS /MODEL DEPENDENT=y INDEPENDENT=x \
             OUTFILE='{file}' /ARIMA AR=1 MA=0 /TRANSFERFUNCTION NUM=1 DENOM=1 \
             /OUTLIER LOCATION=[20] TYPE=ADDITIVE.\n"
        ),
    );
    let forecasts = find(&original, "Forecast");
    assert_eq!(forecasts.rows.len(), 5);
    let applied = run_in(
        &mut engine,
        &format!("TSAPPLY /MODELDETAILS PRINT=[PARAMETERS FORECASTS] /MODEL FILE='{file}'.\n"),
    );
    let again = find(&applied, "Forecast");
    assert_eq!(again.rows.len(), 5);
    for (a, b) in forecasts.rows.iter().zip(&again.rows) {
        for c in 2..5 {
            assert!(close(num(&a[c]), num(&b[c]), 1e-9));
        }
    }
    assert_eq!(outlier_rows(&applied)[0].1, "Additive");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn transfer_and_outlier_errors() {
    let base = two_columns(TF_X, TF_Y);
    for (syntax, wanted) in [
        (
            "TSMODEL /MODEL DEPENDENT=y INDEPENDENT=x /ARIMA AR=1 MA=0 \
             /TRANSFERFUNCTION NUMSEASONAL=1.",
            "season length",
        ),
        (
            "TSMODEL /MODEL DEPENDENT=y /ARIMA AR=1 MA=0 /AUTOOUTLIER DETECT=ON \
             TYPE=[SEASONALADDITIVE].",
            "season length",
        ),
        (
            "TSMODEL /MODEL DEPENDENT=y /ARIMA AR=1 MA=0 /OUTLIER LOCATION=[200] TYPE=ADDITIVE.",
            "no model",
        ),
    ] {
        let message = error(&format!("{base}{syntax}\n"));
        assert!(message.contains(wanted), "{syntax}: {message}");
    }
}

/// MODELSUMMARY PLOT over two models: the histogram of a measure holds each model's value once
/// (checked against the Model Statistics table), and the boxplot of the residual ACF at a lag is
/// the box of the two models' coefficients in the Residual ACF table; with two values the hinges
/// are the two values and the median their mean.
#[test]
fn model_summary_charts_show_the_models_together() {
    let source = format!(
        "{}TSMODEL /MODELSUMMARY PLOT=[SRSQUARE RMSE RESIDACF RESIDPACF] \
         /MODELSTATISTICS MODELFIT=[SRSQUARE RMSE] /MODELDETAILS PRINT=[RESIDACF] \
         /MODEL DEPENDENT=x /ARIMA AR=1 MA=0 /MODEL DEPENDENT=y /ARIMA AR=1 MA=0.\n",
        two_columns(TF_X, TF_Y)
    );
    let mut engine = Engine::default();
    let tables = run_in(&mut engine, &source);
    let charts = engine.take_charts();
    let titles: Vec<&str> = charts.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Histogram of Stationary R-squared",
            "Histogram of RMSE",
            "Residual ACF by Lag",
            "Residual PACF by Lag"
        ]
    );
    let stats = find(&tables, "Model Statistics");
    // the value of a measure in each model's row
    let values = |name: &str| -> Vec<f64> {
        let at = stats.columns.iter().position(|c| c == name).expect(name);
        stats.rows.iter().map(|r| num(&r[at])).collect()
    };
    for (chart, name) in [(&charts[0], "Stationary R-squared"), (&charts[1], "RMSE")] {
        assert_eq!(chart.kind, ChartKind::Histogram);
        let v = values(name);
        assert_eq!(v.len(), 2);
        assert_eq!(chart.series[0].values.iter().sum::<f64>(), 2.0, "{name}");
        // each value lies in a bin that holds a count, a bin keeping its lower bound
        for x in v {
            let counted = chart
                .categories
                .iter()
                .zip(&chart.series[0].values)
                .any(|(c, n)| {
                    let [lo, hi] = c.bin.expect("bounds");
                    *n > 0.0 && lo <= x && x < hi + 1e-9
                });
            assert!(counted, "{name} {x}");
        }
    }
    let acf = find(&tables, "Residual ACF");
    let lag_one: Vec<f64> = acf
        .rows
        .iter()
        .filter(|r| num(&r[1]) == 1.0)
        .map(|r| num(&r[2]))
        .collect();
    assert_eq!(lag_one.len(), 2);
    let Some(Plot::Boxes { boxes }) = &charts[2].plot else {
        panic!("no boxes");
    };
    let b = &boxes[0];
    assert_eq!((b.label.as_str(), b.n), ("1", 2.0));
    let (lo, hi) = (lag_one[0].min(lag_one[1]), lag_one[0].max(lag_one[1]));
    assert!(close(b.lower_hinge, lo, 1e-12) && close(b.upper_hinge, hi, 1e-12));
    assert!(close(b.median, (lo + hi) / 2.0, 1e-12));
    // one box per lag of the table
    let lags = acf.rows.iter().map(|r| num(&r[1]) as usize).max().unwrap();
    assert_eq!(boxes.len(), lags);
}

/// Without reestimation there are no residuals: the boxplots are left out with a warning and
/// the histograms of the models' fit measures stay.
#[test]
fn model_summary_residual_plots_need_reestimation() {
    let dir = std::env::temp_dir().join(format!("statscraft-tsplots-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("models.json");
    let file = file.display();
    let first: String = series(AR1)[..70]
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let mut engine = Engine::default();
    run_in(
        &mut engine,
        &format!(
            "{}PREDICT THRU 76.\nTSMODEL /MODEL DEPENDENT=y OUTFILE='{file}' /ARIMA AR=1 MA=0.\n",
            data("y", &first)
        ),
    );
    engine.take_charts();
    run_in(
        &mut engine,
        &format!(
            "{}PREDICT THRU 76.\nTSAPPLY /MODELSUMMARY PLOT=[RMSE RESIDACF] /MODEL FILE='{file}'.\n",
            data("y", AR1)
        ),
    );
    let charts = engine.take_charts();
    let titles: Vec<&str> = charts.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, ["Histogram of RMSE"]);
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("REESTIMATE=YES"))
    );
    std::fs::remove_dir_all(&dir).ok();
}
