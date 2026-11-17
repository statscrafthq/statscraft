//! TSMODEL and TSAPPLY: exponential smoothing and ARIMA models of time series, chosen by hand
//! or by the expert modeler, with fit measures, forecasts and model files.
//!
//! Rules noted from the Command Syntax Reference and the Algorithms manual:
//! - Each dependent series of a MODEL block gets one model, named PREFIX_k with k counting the
//!   models of that prefix. The estimation period is the USE range less the missing values at
//!   its ends; forecasts start after it and end with PREDICT. Missing values inside are kept.
//! - Smoothing: the recursions of the seven methods are those of EXSMOOTH (the same updates
//!   written another way); a missing value moves the states on without an update. The weights
//!   minimize the sum of squared one-step errors. Start states come from smoothing the series
//!   backwards from its last value, with the documented backward start (last value as level,
//!   minus the fitted slope as trend, seasonal states from seasonal means or regressions).
//! - ARIMA: exact maximum likelihood by the ARIMA engine; the transformation applies to the
//!   dependent series. Predictors are regressors at lag 0.
//! - Fit measures on the original series, k parameters, n valid residuals: MSE = SSE / (n - k),
//!   RMSE, MAPE and MaxAPE in percent, MAE, MaxAE, normalized BIC ln(MSE) + k ln(n) / n,
//!   R squared about the mean, stationary R squared against the mean of the transformed series
//!   differenced as the model is (Brown and Holt twice, the other smoothing methods once, the
//!   seasonal ones also seasonally). Ljung-Box Q on the noise residuals with K - m degrees of
//!   freedom, m the parameters other than the constant and the predictors.
//! - Expert modeler: the smoothing expert fits every method that suits the series (no Brown
//!   when seasonal, no multiplicative Winters unless all values are above 0) and keeps the one
//!   with the smallest normalized BIC; the ARIMA expert works on a non-seasonal series when
//!   n < 3s; with both types, a series shorter than max(20, 3s) gets the smoothing expert and
//!   otherwise the smaller normalized BIC wins. Predictors that the ARIMA expert keeps make its
//!   model win; it drops insignificant predictors one at a time from the last.
//!
//! Choices where the manuals are silent (see docs/differences.md):
//! - A single lag value is an order (AR=2: lags 1 and 2); two or more values are the lags.
//! - The ARIMA expert takes the transformation from the slope of the log standard deviations
//!   on the log means of blocks of the series, the differencing orders with the smallest
//!   variance, the ARMA orders (0 to 2, seasonal 0 to 1) by normalized BIC, then drops terms
//!   with |t| below 2 one at a time.
//! - Brown's method is Holt's with alpha (2 - alpha) and alpha / (2 - alpha) as weights.
//! - Ljung-Box uses K = 18 lags (fewer for a short series). Limits are normal ones on the
//!   transformed scale, taken back; the smoothing forecast errors add psi weights of the
//!   additive form (also for multiplicative Winters, an approximation).
//! - The model file is our own JSON; it keeps the estimation values so that TSAPPLY without
//!   reestimation repeats the forecasts exactly, lined up with the active file by case number.

use crate::arima::{
    Controls, Estimates, Structure, differenced, estimate, exact_prediction, hessian,
    initial_values, least_squares,
};
use crate::chart::{Axis, Category, Series, Statistic};
use crate::descriptives::number;
use crate::dist::{chi_square_p, normal_quantile, t_p};
use crate::exsmooth::{Model as EsModel, Params, Start, ahead, forecast, smooth};
use crate::matrix::inverse;
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::timeseries::{Values, autocorrelations, partial_autocorrelations, read_series};
use crate::{Cell, Chart, Engine, Error, Table};
use serde::{Deserialize, Serialize};
use statscraft_syntax::{
    ES_TYPES, EsType, FilterCount, FitMeasure, Seasonality, Trend, TsArima, TsMethod, TsOutput,
    TsSave, TsTransform, Tsapply, Tsmodel,
};
use std::collections::HashMap;

const FORMAT: &str = "statscraft-time-series-models";
const LJUNG_BOX_LAGS: usize = 18;
const MEASURE_NAMES: [&str; 8] = [
    "Stationary R-squared",
    "R-squared",
    "RMSE",
    "MAPE",
    "MaxAPE",
    "MAE",
    "MaxAE",
    "Normalized BIC",
];

/// The place of a measure in the stored fit measures (the order of MEASURE_NAMES).
fn slot(m: FitMeasure) -> usize {
    match m {
        FitMeasure::Srsquare => 0,
        FitMeasure::Rsquare => 1,
        FitMeasure::Rmse => 2,
        FitMeasure::Mape => 3,
        FitMeasure::Maxape => 4,
        FitMeasure::Mae => 5,
        FitMeasure::Maxae => 6,
        FitMeasure::Normbic => 7,
    }
}

/// The form of a model.
#[derive(Debug, Clone)]
enum Kind {
    Es(EsType),
    Arima(Structure),
}

/// The form of a model as the model file keeps it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Form {
    Exsmooth {
        method: String,
    },
    Arima {
        ar: Vec<usize>,
        ma: Vec<usize>,
        sar: Vec<usize>,
        sma: Vec<usize>,
        d: usize,
        sd: usize,
        constant: bool,
    },
}

/// One model of a model file.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SavedModel {
    id: String,
    dependent: String,
    predictors: Vec<String>,
    form: Form,
    transform: String,
    season: usize,
    parameters: Vec<f64>,
    /// The first case of the estimation period, counted from 1, and the values from there.
    first_case: usize,
    values: Vec<Option<f64>>,
    predictor_values: Vec<Values>,
    /// In the order of MEASURE_NAMES.
    fit: Vec<Option<f64>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ModelFile {
    format: String,
    version: u32,
    models: Vec<SavedModel>,
}

/// A model fitted to one series.
struct Fitted {
    kind: Kind,
    transform: TsTransform,
    season: usize,
    predictors: Vec<String>,
    params: Vec<f64>,
    se: Vec<Option<f64>>,
    labels: Vec<String>,
    /// Valid residuals and parameters.
    n: usize,
    k: usize,
    /// The dependent series over the estimation period (original scale).
    y: Values,
    x: Vec<Values>,
    /// Over the estimation period and the forecasts after it (original scale).
    fit: Values,
    lower: Values,
    upper: Values,
    /// Noise residuals over the estimation period (transformed scale).
    resid: Values,
    stats: [Option<f64>; 8],
    /// Ljung-Box statistic, degrees of freedom and significance.
    q: [Option<f64>; 3],
}

impl Fitted {
    fn est_len(&self) -> usize {
        self.y.len()
    }

    fn type_name(&self) -> String {
        let transform = match self.transform {
            TsTransform::None => "",
            TsTransform::Sqrt => ", square root",
            TsTransform::Ln => ", natural log",
        };
        let name = match &self.kind {
            Kind::Es(t) => es_name(*t).to_string(),
            Kind::Arima(st) => {
                let top = |l: &[usize]| l.iter().max().copied().unwrap_or(0);
                let mut name = format!("ARIMA({},{},{})", top(&st.ar), st.d, top(&st.ma));
                if st.s > 1 {
                    name.push_str(&format!("({},{},{})", top(&st.sar), st.sd, top(&st.sma)));
                }
                name
            }
        };
        format!("{name}{transform}")
    }

    fn saved(&self, id: &str, dependent: &str, first: usize) -> SavedModel {
        let form = match &self.kind {
            Kind::Es(t) => Form::Exsmooth {
                method: ES_TYPES
                    .iter()
                    .find(|(_, e)| e == t)
                    .map(|(n, _)| n.to_string())
                    .expect("every method has a name"),
            },
            Kind::Arima(st) => Form::Arima {
                ar: st.ar.clone(),
                ma: st.ma.clone(),
                sar: st.sar.clone(),
                sma: st.sma.clone(),
                d: st.d,
                sd: st.sd,
                constant: st.constant,
            },
        };
        let est = self.est_len();
        SavedModel {
            id: id.into(),
            dependent: dependent.into(),
            predictors: self.predictors.clone(),
            form,
            transform: transform_name(self.transform).into(),
            season: self.season,
            parameters: self.params.clone(),
            first_case: first + 1,
            values: self.y.clone(),
            predictor_values: self.x.iter().map(|c| c[..est].to_vec()).collect(),
            fit: self.stats.to_vec(),
        }
    }
}

fn es_name(t: EsType) -> &'static str {
    match t {
        EsType::Simple => "Simple",
        EsType::SimpleSeasonal => "Simple Seasonal",
        EsType::Holt => "Holt",
        EsType::Brown => "Brown",
        EsType::DampedTrend => "Damped Trend",
        EsType::WintersAdditive => "Winters' Additive",
        EsType::WintersMultiplicative => "Winters' Multiplicative",
    }
}

fn transform_name(t: TsTransform) -> &'static str {
    match t {
        TsTransform::None => "NONE",
        TsTransform::Sqrt => "SQRT",
        TsTransform::Ln => "LN",
    }
}

fn seasonal_es(t: EsType) -> bool {
    matches!(
        t,
        EsType::SimpleSeasonal | EsType::WintersAdditive | EsType::WintersMultiplicative
    )
}

fn es_model(t: EsType, s: usize) -> EsModel {
    let (trend, season) = match t {
        EsType::Simple => (Trend::None, Seasonality::None),
        EsType::SimpleSeasonal => (Trend::None, Seasonality::Additive),
        EsType::Holt | EsType::Brown => (Trend::Linear, Seasonality::None),
        EsType::DampedTrend => (Trend::Damped, Seasonality::None),
        EsType::WintersAdditive => (Trend::Linear, Seasonality::Additive),
        EsType::WintersMultiplicative => (Trend::Linear, Seasonality::Multiplicative),
    };
    let p = if season == Seasonality::None { 0 } else { s };
    EsModel { trend, season, p }
}

/// The weights of a method in their order, as labels.
fn es_labels(t: EsType) -> &'static [&'static str] {
    match t {
        EsType::Simple => &["Alpha (Level)"],
        EsType::SimpleSeasonal => &["Alpha (Level)", "Delta (Season)"],
        EsType::Holt => &["Alpha (Level)", "Gamma (Trend)"],
        EsType::Brown => &["Alpha (Level and Trend)"],
        EsType::DampedTrend => &["Alpha (Level)", "Gamma (Trend)", "Phi (Trend damping)"],
        EsType::WintersAdditive | EsType::WintersMultiplicative => {
            &["Alpha (Level)", "Gamma (Trend)", "Delta (Season)"]
        }
    }
}

/// The weights as the EXSMOOTH recursions take them.
fn es_params(t: EsType, v: &[f64]) -> Params {
    let mut q = Params {
        alpha: v[0],
        gamma: 0.0,
        delta: 0.0,
        phi: 1.0,
    };
    match t {
        EsType::Simple => {}
        EsType::SimpleSeasonal => q.delta = v[1],
        EsType::Holt => q.gamma = v[1],
        // Brown's double smoothing is Holt's method with these weights
        EsType::Brown => (q.alpha, q.gamma) = (v[0] * (2.0 - v[0]), v[0] / (2.0 - v[0])),
        EsType::DampedTrend => (q.gamma, q.phi) = (v[1], v[2]),
        EsType::WintersAdditive | EsType::WintersMultiplicative => {
            (q.gamma, q.delta) = (v[1], v[2]);
        }
    }
    q
}

/// The start states for forward smoothing: smoothing backwards from the last value, then
/// stepping once more to before the first case.
fn backcast(x: &[f64], m: &EsModel, q: &Params) -> Start {
    let n = x.len();
    let s = m.p.max(1);
    let valid = |t: usize| (!x[t].is_nan()).then_some(x[t]);
    let y: Values = (0..n).map(valid).collect();
    let time: Values = (1..=n).map(|t| Some(t as f64)).collect();
    let ones: Values = vec![Some(1.0); n];
    let last = y.iter().rev().flatten().next().copied().unwrap_or(0.0);
    let slope = || least_squares(&y, &[time.clone(), ones.clone()]).map_or(0.0, |b| b[0]);
    // seasonal states by season position (case t from 0 is at t mod s)
    let (trend, states): (f64, Vec<f64>) = match (m.trend, m.season) {
        (Trend::None, Seasonality::None) => (0.0, Vec::new()),
        (_, Seasonality::None) => (-slope(), Vec::new()),
        (Trend::None, _) => {
            let valid: Vec<f64> = y.iter().flatten().copied().collect();
            let mean = valid.iter().sum::<f64>() / valid.len().max(1) as f64;
            let states = (0..s)
                .map(|k| {
                    let v: Vec<f64> = (k..n).step_by(s).filter_map(|t| y[t]).collect();
                    v.iter().sum::<f64>() / v.len().max(1) as f64 - mean
                })
                .collect();
            (0.0, states)
        }
        (_, Seasonality::Additive) => {
            let mut columns = vec![time.clone()];
            columns.extend((0..s).map(|k| (0..n).map(|t| Some(f64::from(t % s == k))).collect()));
            let b = least_squares(&y, &columns).unwrap_or_else(|| vec![0.0; s + 1]);
            let mean = b[1..].iter().sum::<f64>() / s as f64;
            (-b[0], b[1..].iter().map(|v| v - mean).collect())
        }
        (_, Seasonality::Multiplicative) => {
            let lines: Vec<(f64, f64)> = (0..s)
                .map(|k| {
                    let own: Values = (0..n).map(|t| y[t].filter(|_| t % s == k)).collect();
                    match least_squares(&own, &[ones.clone(), time.clone()]) {
                        Some(b) => (b[0], b[1]),
                        None => {
                            let v: Vec<f64> = own.iter().flatten().copied().collect();
                            (v.iter().sum::<f64>() / v.len().max(1) as f64, 0.0)
                        }
                    }
                })
                .collect();
            let mean_mu = lines.iter().map(|l| l.0).sum::<f64>() / s as f64;
            let mean_beta = lines.iter().map(|l| l.1).sum::<f64>() / s as f64;
            let states = lines
                .iter()
                .map(|(mu, beta)| (mu + beta) / (mean_mu + mean_beta))
                .collect();
            (-mean_beta, states)
        }
    };
    let reversed: Vec<f64> = x.iter().rev().copied().collect();
    let p = m.p;
    let back = Start {
        level: last,
        trend,
        // reversed case j (from 0) is original case n - 1 - j
        factors: (0..p).map(|j| states[(n - 1 - j) % s]).collect(),
    };
    let fit = smooth(&reversed, m, &back, q);
    let t = fit.trends[n];
    Start {
        level: ahead(m, q, fit.levels[n], t, 1),
        trend: -(if m.trend == Trend::Damped {
            q.phi * t
        } else {
            t
        }),
        // the latest backward factor of original case j + 1 is I_(n - j) of the reversed run
        factors: (0..p).map(|j| fit.factors[n - j + p - 1]).collect(),
    }
}

/// Minimizes `f` over [0, 1]^d: the best of a coarse grid, then a compass search.
fn minimize(f: &dyn Fn(&[f64]) -> f64, d: usize) -> Vec<f64> {
    let grid = [0.1, 0.3, 0.5, 0.7, 0.9];
    let mut best = vec![0.5; d];
    let mut best_f = f64::INFINITY;
    for i in 0..grid.len().pow(d as u32) {
        let point: Vec<f64> = (0..d)
            .map(|j| grid[i / grid.len().pow(j as u32) % grid.len()])
            .collect();
        let v = f(&point);
        if v < best_f {
            (best, best_f) = (point, v);
        }
    }
    let mut step = 0.1;
    while step > 1e-7 {
        let mut moved = false;
        for j in 0..d {
            for sign in [1.0, -1.0] {
                let mut p = best.clone();
                p[j] = (p[j] + sign * step).clamp(0.0, 1.0);
                let v = f(&p);
                if v < best_f {
                    (best, best_f, moved) = (p, v, true);
                }
            }
        }
        if !moved {
            step /= 2.0;
        }
    }
    best
}

/// Standard errors from the observed information of (n / 2) ln f, f a sum of squares.
fn standard_errors(f: &dyn Fn(&[f64]) -> Option<f64>, at: &[f64], n: usize) -> Vec<Option<f64>> {
    let half = n as f64 / 2.0;
    let g = |t: &[f64]| f(t).map(|v| half * v.ln());
    let k = at.len();
    hessian(&g, at)
        .and_then(|h| inverse(&h).map(|(inv, _)| inv))
        .map_or(vec![None; k], |inv| {
            (0..k)
                .map(|i| (inv[i][i] > 0.0).then(|| inv[i][i].sqrt()))
                .collect()
        })
}

/// The one-step fits over the estimation period and forecasts after it, with the standard
/// deviation of each (transformed scale).
struct Core {
    params: Vec<f64>,
    se: Vec<Option<f64>>,
    labels: Vec<String>,
    k: usize,
    /// Parameters counted off the Ljung-Box degrees of freedom.
    m: usize,
    fit: Values,
    sd: Values,
}

fn es_core(
    t: EsType,
    s: usize,
    z: &[Option<f64>],
    horizon: usize,
    fixed: Option<&[f64]>,
) -> Result<Core, String> {
    let m = es_model(t, s);
    let x: Vec<f64> = z.iter().map(|v| v.unwrap_or(f64::NAN)).collect();
    let n = x.len();
    let valid = z.iter().flatten().count();
    let d = es_labels(t).len();
    if m.seasonal() && n < 2 * s {
        return Err(format!(
            "needs at least two full seasons ({}) for {}",
            2 * s,
            es_name(t)
        ));
    }
    if valid <= d + 1 {
        return Err(format!("has too few valid values for {}", es_name(t)));
    }
    if t == EsType::WintersMultiplicative && z.iter().flatten().any(|v| *v <= 0.0) {
        return Err("needs values above 0 for Winters' multiplicative method".into());
    }
    let sse = |v: &[f64]| {
        let q = es_params(t, v);
        smooth(&x, &m, &backcast(&x, &m, &q), &q).sse
    };
    let params = match fixed {
        Some(v) => v.to_vec(),
        None => minimize(&sse, d),
    };
    let q = es_params(t, &params);
    let fit = smooth(&x, &m, &backcast(&x, &m, &q), &q);
    if !fit.sse.is_finite() {
        return Err(format!("gives no finite fit for {}", es_name(t)));
    }
    let sigma = (fit.sse / (valid - d) as f64).sqrt();
    let se = standard_errors(
        &|v: &[f64]| Some(sse(v)).filter(|s| s.is_finite()),
        &params,
        valid,
    );
    // forecast errors: psi weights of the error-correction form
    let psi = |j: usize| {
        let trend = match m.trend {
            Trend::Linear => j as f64,
            Trend::Damped => (1..=j).map(|i| q.phi.powi(i as i32)).sum(),
            _ => 0.0,
        };
        let season = if m.seasonal() && j.is_multiple_of(s) {
            q.delta * (1.0 - q.alpha)
        } else {
            0.0
        };
        q.alpha + q.alpha * q.gamma * trend + season
    };
    let mut fits: Values = fit.fits.iter().map(|v| Some(*v)).collect();
    let mut sd: Values = vec![Some(sigma); n];
    let mut sum: f64 = 1.0;
    for h in 1..=horizon {
        fits.push(Some(forecast(&m, &q, &fit, n, h)));
        sd.push(Some(sigma * sum.sqrt()));
        sum += psi(h).powi(2);
    }
    Ok(Core {
        params,
        se,
        labels: es_labels(t).iter().map(|l| l.to_string()).collect(),
        k: d,
        m: d,
        fit: fits,
        sd,
    })
}

fn arima_core(
    st: &Structure,
    z: &[Option<f64>],
    x: &[Values],
    est: usize,
    predictors: &[String],
    fixed: Option<&[f64]>,
) -> Result<Core, String> {
    let reach = st.reach();
    if (0..reach).any(|t| t >= est || z[t].is_none() || x.iter().any(|c| c[t].is_none())) {
        return Err(format!(
            "needs its first {reach} values (and those of its predictors) valid, as the \
             differencing starts from them"
        ));
    }
    let y_used = z[..est].to_vec();
    let x_used: Vec<Values> = x.iter().map(|c| c[..est].to_vec()).collect();
    let theta0 = match fixed {
        Some(v) => v.to_vec(),
        None => initial_values(st, &y_used, &x_used, &Estimates::default()),
    };
    let controls = Controls {
        mxiter: if fixed.is_some() { 0 } else { 50 },
        mxlamb: 1e9,
        ssqpct: 1e-6,
        pareps: 1e-6,
    };
    let e = estimate(st, &y_used, &x_used, 0, theta0, &controls).map_err(str::to_string)?;
    let k = e.theta.len();
    if e.n <= k {
        return Err(format!("has {} values, too few for {k} parameters", e.n));
    }
    let sigma = (e.ssq_plain / (e.n - k) as f64).sqrt();
    let prediction =
        exact_prediction(st, &e.theta, z, x, 0, est).ok_or("the estimates give no valid model")?;
    let mut labels = Vec::new();
    for (name, lags) in [
        ("AR", &st.ar),
        ("MA", &st.ma),
        ("AR, Seasonal", &st.sar),
        ("MA, Seasonal", &st.sma),
    ] {
        labels.extend(lags.iter().map(|l| format!("{name} Lag {l}")));
    }
    labels.extend(predictors.iter().map(|p| format!("Numerator Lag 0 ({p})")));
    if st.constant {
        labels.push("Constant".into());
    }
    let se = match &e.covariance {
        Some(c) => (0..k).map(|i| Some(c[i][i].sqrt())).collect(),
        None => vec![None; k],
    };
    Ok(Core {
        se,
        labels,
        k,
        m: st.ar.len() + st.ma.len() + st.sar.len() + st.sma.len(),
        fit: prediction.fit,
        sd: prediction.se.iter().map(|v| v.map(|v| v * sigma)).collect(),
        params: e.theta,
    })
}

/// Fits a model of the series `y` (original scale, the estimation period) with predictors `x`
/// over the estimation period and the `horizon` forecasts after it.
#[allow(clippy::too_many_arguments)]
fn fit_model(
    kind: Kind,
    transform: TsTransform,
    season: usize,
    y: &[Option<f64>],
    x: Vec<Values>,
    predictors: Vec<String>,
    horizon: usize,
    fixed: Option<&[f64]>,
    level: f64,
) -> Result<Fitted, String> {
    let est = y.len();
    let forward = |v: f64| match transform {
        TsTransform::None => Some(v),
        TsTransform::Sqrt => (v >= 0.0).then(|| v.sqrt()),
        TsTransform::Ln => (v > 0.0).then(|| v.ln()),
    };
    let back = |v: f64| match transform {
        TsTransform::None => v,
        TsTransform::Sqrt => v.max(0.0).powi(2),
        TsTransform::Ln => v.exp(),
    };
    let mut z: Values = Vec::with_capacity(est + horizon);
    for v in y {
        match v.map(forward) {
            Some(None) => {
                return Err(format!(
                    "has values the {} transformation cannot take",
                    transform_name(transform)
                ));
            }
            other => z.push(other.flatten()),
        }
    }
    let core = match &kind {
        Kind::Es(t) => es_core(*t, season, &z, horizon, fixed)?,
        Kind::Arima(st) => {
            z.resize(est + horizon, None);
            arima_core(st, &z, &x, est, &predictors, fixed)?
        }
    };
    z.truncate(est);
    let zq = normal_quantile(1.0 - (1.0 - level / 100.0) / 2.0);
    let fit: Values = core.fit.iter().map(|v| v.map(back)).collect();
    let lower = (0..fit.len())
        .map(|t| Some(back(core.fit[t]? - zq * core.sd[t]?)))
        .collect();
    let upper = (0..fit.len())
        .map(|t| Some(back(core.fit[t]? + zq * core.sd[t]?)))
        .collect();
    let resid: Values = (0..est).map(|t| Some(z[t]? - core.fit[t]?)).collect();

    // fit measures on the original series
    let pairs: Vec<(f64, f64)> = (0..est).filter_map(|t| Some((y[t]?, fit[t]?))).collect();
    let n = pairs.len();
    let k = core.k;
    let sse: f64 = pairs.iter().map(|(a, b)| (a - b).powi(2)).sum();
    let mse = sse / (n as f64 - k as f64);
    let mean = pairs.iter().map(|p| p.0).sum::<f64>() / n as f64;
    let sst: f64 = pairs.iter().map(|(a, _)| (a - mean).powi(2)).sum();
    let percent: Vec<f64> = pairs
        .iter()
        .filter(|(a, _)| *a != 0.0)
        .map(|(a, b)| 100.0 * ((a - b) / a).abs())
        .collect();
    let absolute: Vec<f64> = pairs.iter().map(|(a, b)| (a - b).abs()).collect();
    let mean_of = |v: &[f64]| (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64);
    let max_of = |v: &[f64]| v.iter().copied().reduce(f64::max);
    // the stationary part: the transformed series differenced as the model is
    let (d, sd, s) = match &kind {
        Kind::Es(t) => (
            if matches!(t, EsType::Holt | EsType::Brown) {
                2
            } else {
                1
            },
            usize::from(seasonal_es(*t)),
            season,
        ),
        Kind::Arima(st) => (st.d, st.sd, st.s),
    };
    let poly = Structure {
        ar: Vec::new(),
        ma: Vec::new(),
        sar: Vec::new(),
        sma: Vec::new(),
        d,
        sd,
        s,
        regressors: 0,
        constant: false,
    }
    .difference();
    let dz = differenced(&z, &poly);
    let both: Vec<(f64, f64)> = (0..est).filter_map(|t| Some((resid[t]?, dz[t]?))).collect();
    let dmean = both.iter().map(|p| p.1).sum::<f64>() / both.len().max(1) as f64;
    let srsq = 1.0
        - both.iter().map(|p| p.0 * p.0).sum::<f64>()
            / both.iter().map(|p| (p.1 - dmean).powi(2)).sum::<f64>();
    let finite = |v: f64| v.is_finite().then_some(v);
    let stats = [
        finite(srsq),
        finite(1.0 - sse / sst),
        finite(mse.sqrt()),
        mean_of(&percent),
        max_of(&percent),
        mean_of(&absolute),
        max_of(&absolute),
        finite(mse.ln() + k as f64 * (n as f64).ln() / n as f64),
    ];
    // Ljung-Box on the noise residuals
    let valid = resid.iter().flatten().count();
    let lags = LJUNG_BOX_LAGS.min(valid.saturating_sub(1));
    let (r, _) = autocorrelations(&resid, lags);
    let nf = valid as f64;
    let qv = nf
        * (nf + 2.0)
        * (1..=lags)
            .map(|j| r[j - 1].powi(2) / (nf - j as f64))
            .sum::<f64>();
    let df = lags as f64 - core.m as f64;
    let q = match df > 0.0 && qv.is_finite() {
        true => [Some(qv), Some(df), Some(chi_square_p(qv, df))],
        false => [finite(qv), None, None],
    };
    Ok(Fitted {
        kind,
        transform,
        season,
        predictors,
        params: core.params,
        se: core.se,
        labels: core.labels,
        n,
        k,
        y: y.to_vec(),
        x,
        fit,
        lower,
        upper,
        resid,
        stats,
        q,
    })
}

/// A single lag value is an order; two or more are the lags.
fn lag_list(v: &[u32]) -> Vec<usize> {
    let mut lags: Vec<usize> = match v {
        [one] => (1..=*one as usize).collect(),
        _ => v.iter().filter(|&&l| l > 0).map(|&l| l as usize).collect(),
    };
    lags.sort_unstable();
    lags.dedup();
    lags
}

fn structure(a: &TsArima, season: usize, regressors: usize) -> Structure {
    let mut st = Structure {
        ar: lag_list(&a.ar),
        ma: lag_list(&a.ma),
        sar: lag_list(&a.sar),
        sma: lag_list(&a.sma),
        d: a.d as usize,
        sd: a.sd as usize,
        s: 1,
        regressors,
        constant: a.constant,
    };
    if !st.sar.is_empty() || !st.sma.is_empty() || st.sd > 0 {
        st.s = season;
    }
    st
}

/// The Box-Cox power nearest to the one the blocks of the series suggest: 1, 1/2 or 0, from
/// the slope of the log standard deviations on the log means.
fn expert_transform(y: &[Option<f64>], s: usize) -> TsTransform {
    let valid: Vec<f64> = y.iter().flatten().copied().collect();
    if valid.iter().any(|v| *v <= 0.0) {
        return TsTransform::None;
    }
    let block = if s > 1 { s } else { 4 };
    let points: Vec<(f64, f64)> = valid
        .chunks_exact(block)
        .filter_map(|c| {
            let mean = c.iter().sum::<f64>() / c.len() as f64;
            let var = c.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (c.len() - 1) as f64;
            (var > 0.0).then(|| (mean.ln(), var.sqrt().ln()))
        })
        .collect();
    if points.len() < 3 {
        return TsTransform::None;
    }
    let mx = points.iter().map(|p| p.0).sum::<f64>() / points.len() as f64;
    let my = points.iter().map(|p| p.1).sum::<f64>() / points.len() as f64;
    let sxx: f64 = points.iter().map(|p| (p.0 - mx).powi(2)).sum();
    if sxx <= 0.0 {
        return TsTransform::None;
    }
    let slope = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum::<f64>() / sxx;
    match 1.0 - slope {
        l if l <= 0.25 => TsTransform::Ln,
        l if l <= 0.75 => TsTransform::Sqrt,
        _ => TsTransform::None,
    }
}

/// The differencing orders (d up to 2, seasonal up to 1) that leave the smallest variance.
fn expert_differencing(z: &[Option<f64>], s: usize) -> (usize, usize) {
    let mut best = (0, 0, f64::INFINITY);
    for sd in 0..=usize::from(s > 1) {
        for d in 0..=2 {
            let st = Structure {
                ar: Vec::new(),
                ma: Vec::new(),
                sar: Vec::new(),
                sma: Vec::new(),
                d,
                sd,
                s,
                regressors: 0,
                constant: false,
            };
            let v: Vec<f64> = differenced(z, &st.difference())
                .into_iter()
                .flatten()
                .collect();
            if v.len() < 3 {
                continue;
            }
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (v.len() - 1) as f64;
            if var < best.2 * (1.0 - 1e-9) {
                best = (d, sd, var);
            }
        }
    }
    (best.0, best.1)
}

fn bic(f: &Fitted) -> f64 {
    f.stats[7].unwrap_or(f64::INFINITY)
}

/// The smoothing expert: the method with the smallest normalized BIC.
fn expert_es(y: &[Option<f64>], s: usize, horizon: usize, level: f64) -> Option<Fitted> {
    let seasonal = s > 1 && y.len() >= 2 * s;
    let positive = y.iter().flatten().all(|v| *v > 0.0);
    let mut methods = vec![EsType::Simple, EsType::Holt, EsType::DampedTrend];
    if seasonal {
        methods.extend([EsType::SimpleSeasonal, EsType::WintersAdditive]);
        if positive {
            methods.push(EsType::WintersMultiplicative);
        }
    } else {
        methods.push(EsType::Brown);
    }
    let season = if seasonal { s } else { 1 };
    methods
        .into_iter()
        .filter_map(|t| {
            fit_model(
                Kind::Es(t),
                TsTransform::None,
                season,
                y,
                Vec::new(),
                Vec::new(),
                horizon,
                None,
                level,
            )
            .ok()
        })
        .min_by(|a, b| bic(a).total_cmp(&bic(b)))
}

/// The ARIMA expert; predictors that stay make the model a transfer function model.
fn expert_arima(
    y: &[Option<f64>],
    x: &[Values],
    names: &[String],
    s: usize,
    seasonal: bool,
    horizon: usize,
    level: f64,
) -> Option<Fitted> {
    let n = y.len();
    let s = if seasonal && s > 1 && n >= 3 * s {
        s
    } else {
        1
    };
    let transform = expert_transform(y, s);
    let z: Values = y
        .iter()
        .map(|v| {
            v.map(|v| match transform {
                TsTransform::None => v,
                TsTransform::Sqrt => v.sqrt(),
                TsTransform::Ln => v.ln(),
            })
        })
        .collect();
    let (d, sd) = expert_differencing(&z, s);
    let fit = |st: &Structure, keep: &[usize]| {
        let mut st = st.clone();
        st.regressors = keep.len();
        fit_model(
            Kind::Arima(st),
            transform,
            s,
            y,
            keep.iter().map(|&i| x[i].clone()).collect(),
            keep.iter().map(|&i| names[i].clone()).collect(),
            horizon,
            None,
            level,
        )
        .ok()
    };
    let all: Vec<usize> = (0..names.len()).collect();
    let mut best: Option<(Structure, Fitted)> = None;
    let seasonal_orders = if s > 1 { 1 } else { 0 };
    for p in 0..=2 {
        for q in 0..=2 {
            for sp in 0..=seasonal_orders {
                for sq in 0..=seasonal_orders {
                    let st = Structure {
                        ar: (1..=p).collect(),
                        ma: (1..=q).collect(),
                        sar: (1..=sp).collect(),
                        sma: (1..=sq).collect(),
                        d,
                        sd,
                        s,
                        regressors: 0,
                        constant: d + sd < 2,
                    };
                    if let Some(f) = fit(&st, &all)
                        && best.as_ref().is_none_or(|b| bic(&f) < bic(&b.1))
                    {
                        best = Some((st, f));
                    }
                }
            }
        }
    }
    let (mut st, mut model) = best?;
    let mut keep = all;
    // drop terms with |t| below 2, the last insignificant predictor first, else the weakest
    loop {
        let t = |i: usize| match model.se[i] {
            Some(se) if se > 0.0 => (model.params[i] / se).abs(),
            _ => 0.0,
        };
        let arma = st.ar.len() + st.ma.len() + st.sar.len() + st.sma.len();
        let weak: Vec<usize> = (0..model.params.len()).filter(|&i| t(i) < 2.0).collect();
        let Some(&drop) = weak
            .iter()
            .rev()
            .find(|&&i| i >= arma && i < arma + keep.len())
            .or_else(|| weak.iter().min_by(|&&a, &&b| t(a).total_cmp(&t(b))))
        else {
            break;
        };
        let mut i = drop;
        for lags in [&mut st.ar, &mut st.ma, &mut st.sar, &mut st.sma] {
            if i < lags.len() {
                lags.remove(i);
                i = usize::MAX;
                break;
            }
            i -= lags.len();
        }
        if i != usize::MAX {
            if i < keep.len() {
                keep.remove(i);
            } else {
                st.constant = false;
            }
        }
        match fit(&st, &keep) {
            Some(f) => model = f,
            None => break,
        }
    }
    Some(model)
}

/// Rows of the series that the window covers, with their labels.
struct Place {
    /// The case of the first estimation value (from 0) in the active file.
    first: usize,
}

/// One model with its name and where its series sits in the active file.
struct Run {
    id: String,
    dependent: String,
    model: Fitted,
    place: Place,
    /// False under TSAPPLY without reestimation: only forecasts are new.
    estimated: bool,
}

fn summary_row(name: String, values: &[f64]) -> Vec<Cell> {
    let mut row = vec![text(name)];
    let n = values.len();
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let mean = values.iter().sum::<f64>() / n as f64;
    let sd = (n > 1)
        .then(|| (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt());
    // the weighted average at (n + 1) p
    let percentile = |p: f64| {
        let at = (n as f64 + 1.0) * p / 100.0;
        let k = at.floor() as usize;
        Some(match k {
            0 => sorted[0],
            k if k >= n => sorted[n - 1],
            k => sorted[k - 1] + (at - k as f64) * (sorted[k] - sorted[k - 1]),
        })
    };
    row.extend([
        number(Some(mean)),
        number(sd),
        number(sorted.first().copied()),
        number(sorted.last().copied()),
    ]);
    row.extend([5.0, 10.0, 25.0, 50.0, 75.0, 90.0, 95.0].map(|p| number(percentile(p))));
    row
}

const SUMMARY_COLUMNS: [&str; 12] = [
    "",
    "Mean",
    "Std. Deviation",
    "Minimum",
    "Maximum",
    "5",
    "10",
    "25",
    "50",
    "75",
    "90",
    "95",
];

impl Engine {
    /// The season length: SEASONLENGTH, else the period of the DATE variables, else 1.
    fn ts_season(&self, out: &TsOutput) -> Result<usize, Error> {
        Ok(out
            .season_length
            .or_else(|| crate::date::date_period(self.active().ok()?))
            .unwrap_or(1))
    }

    /// The USE range and the last case the forecasts reach (it may be past the file).
    fn ts_window(&self, command: &str) -> Result<(usize, usize, usize), Error> {
        let periods = self.periods(command)?;
        let data = self.active()?;
        let end = match self.split_group {
            Some(_) => periods.end,
            None => self
                .predict_bounds(data, periods.last, None)?
                .map_or(periods.last, |(_, end)| end),
        };
        Ok((periods.first, periods.last, end.max(periods.last)))
    }

    pub(crate) fn tsmodel(&mut self, c: &Tsmodel) -> Result<Vec<Table>, Error> {
        let out = &c.output;
        let season = self.ts_season(out)?;
        let (first, last, end) = self.ts_window("TSMODEL")?;
        let mut runs: Vec<Run> = Vec::new();
        let mut counters: HashMap<String, usize> = HashMap::new();
        let mut files: Vec<(String, Vec<SavedModel>)> = Vec::new();
        let mut warnings = Vec::new();
        let level = out.cilevel;
        for block in &c.models {
            let data = self.active()?;
            let dependents = self.expand(&block.dependent)?;
            let independents = self.expand(&block.independent)?;
            let seasonal_needed = match &block.method {
                TsMethod::Exsmooth(t, _) => seasonal_es(*t),
                TsMethod::Arima(a) => a.sd > 0 || a.sar.iter().chain(&a.sma).any(|&v| v > 0),
                TsMethod::Expert { .. } => false,
            };
            if seasonal_needed && season < 2 {
                return Err(Error(
                    "TSMODEL: a seasonal model needs a season length: give SEASONLENGTH or DATE \
                     variables with a period"
                        .into(),
                ));
            }
            match &block.method {
                TsMethod::Exsmooth(..) if !independents.is_empty() => {
                    return Err(Error(
                        "TSMODEL: EXSMOOTH takes no INDEPENDENT variables".into(),
                    ));
                }
                TsMethod::Expert { arima: false, .. } if !independents.is_empty() => {
                    return Err(Error(
                        "TSMODEL: EXPERTMODELER TYPE=[EXSMOOTH] takes no INDEPENDENT variables"
                            .into(),
                    ));
                }
                TsMethod::Arima(a)
                    if independents.is_empty()
                        && !a.constant
                        && [&a.ar, &a.ma, &a.sar, &a.sma]
                            .iter()
                            .all(|l| lag_list(l).is_empty()) =>
                {
                    return Err(Error(
                        "TSMODEL: the ARIMA model has no parameter: give an AR or MA lag, a \
                         constant or a predictor"
                            .into(),
                    ));
                }
                _ => {}
            }
            let mut fitted_here = Vec::new();
            for dep in &dependents {
                let (all, _, _) = read_series(data, dep, out.include_user_missing, "TSMODEL")?;
                let Some(a) = (first..=last).find(|&t| all[t].is_some()) else {
                    warnings.push(format!(
                        "TSMODEL: {dep} has no valid values; it is not modeled"
                    ));
                    continue;
                };
                let b = (first..=last)
                    .rev()
                    .find(|&t| all[t].is_some())
                    .expect("a is valid");
                let y = all[a..=b].to_vec();
                let horizon = end - b;
                let mut names: Vec<String> = Vec::new();
                let mut x: Vec<Values> = Vec::new();
                for name in independents.iter().filter(|n| *n != dep) {
                    let (column, _, _) =
                        read_series(data, name, out.include_user_missing, "TSMODEL")?;
                    let values: Values = (a..=end)
                        .map(|t| column.get(t).copied().flatten())
                        .collect();
                    if values[..y.len()].iter().any(Option::is_none) {
                        if matches!(block.method, TsMethod::Arima(_)) {
                            warnings.push(format!(
                                "TSMODEL: the predictor {name} has missing values in the \
                                 estimation period, so {dep} is not modeled"
                            ));
                            names.clear();
                            x.clear();
                            names.push(String::new());
                            break;
                        }
                        warnings.push(format!(
                            "TSMODEL: the predictor {name} has missing values in the estimation \
                             period; the model of {dep} leaves it out"
                        ));
                        continue;
                    }
                    names.push(name.clone());
                    x.push(values);
                }
                if names.first().is_some_and(String::is_empty) {
                    continue;
                }
                let result = match &block.method {
                    TsMethod::Exsmooth(t, transform) => fit_model(
                        Kind::Es(*t),
                        *transform,
                        if seasonal_es(*t) { season } else { 1 },
                        &y,
                        Vec::new(),
                        Vec::new(),
                        horizon,
                        None,
                        level,
                    ),
                    TsMethod::Arima(spec) => {
                        let st = structure(spec, season, names.len());
                        fit_model(
                            Kind::Arima(st),
                            spec.transform,
                            season,
                            &y,
                            x,
                            names,
                            horizon,
                            None,
                            level,
                        )
                    }
                    TsMethod::Expert {
                        arima,
                        exsmooth,
                        seasonal,
                    } => {
                        let s = if *seasonal { season } else { 1 };
                        let n = y.len();
                        let short = n < 20.max(3 * s);
                        let univariate = short || names.is_empty();
                        let (x, names) = match univariate {
                            true => (Vec::new(), Vec::new()),
                            false => (x, names),
                        };
                        // a smoothing model also competes with an ARIMA model that ends
                        // without predictors
                        let es = (*exsmooth)
                            .then(|| expert_es(&y, s, horizon, level))
                            .flatten();
                        let ar = (*arima && !(short && *exsmooth))
                            .then(|| expert_arima(&y, &x, &names, s, *seasonal, horizon, level))
                            .flatten();
                        match (es, ar) {
                            (_, Some(a)) if !a.predictors.is_empty() => Ok(a),
                            (Some(e), Some(a)) => Ok(if bic(&a) < bic(&e) { a } else { e }),
                            (Some(m), None) | (None, Some(m)) => Ok(m),
                            (None, None) => Err("fits no model".to_string()),
                        }
                    }
                };
                let model = match result {
                    Ok(m) => m,
                    Err(e) => {
                        warnings.push(format!("TSMODEL: {dep} {e}; it is not modeled"));
                        continue;
                    }
                };
                let counter = counters.entry(block.prefix.clone()).or_default();
                *counter += 1;
                let id = format!("{}_{}", block.prefix, counter);
                fitted_here.push(model.saved(&id, dep, a));
                runs.push(Run {
                    id,
                    dependent: dep.clone(),
                    model,
                    place: Place { first: a },
                    estimated: true,
                });
            }
            if let Some(file) = &block.outfile {
                match files.iter_mut().find(|(f, _)| f == file) {
                    Some((_, models)) => models.extend(fitted_here),
                    None => files.push((file.clone(), fitted_here)),
                }
            }
        }
        if self.split_group.is_some() && !files.is_empty() {
            warnings.push("TSMODEL: OUTFILE is ignored under SPLIT FILE".into());
            files.clear();
        }
        for (file, models) in files {
            self.write_models(&file, models, "TSMODEL")?;
        }
        for w in warnings {
            self.warn(w);
        }
        self.ts_report("TSMODEL", out, runs, end)
    }

    fn write_models(&self, file: &str, models: Vec<SavedModel>, who: &str) -> Result<(), Error> {
        let value = serde_json::to_value(ModelFile {
            format: FORMAT.into(),
            version: 1,
            models,
        })
        .expect("a model file serializes");
        self.write_json(who, file, "model", &value)
    }

    pub(crate) fn tsapply(&mut self, c: &Tsapply) -> Result<Vec<Table>, Error> {
        let out = &c.output;
        if self.split_group.is_some() {
            return Err(Error("TSAPPLY cannot run under SPLIT FILE".into()));
        }
        let season = self.ts_season(out)?;
        let given_season = out.season_length.is_some() || season > 1;
        let (first, last, end) = match out.reestimate {
            true => self.ts_window("TSAPPLY")?,
            // without reestimation USE does not count: forecasts run to the end of PREDICT,
            // else of the file
            false => {
                let data = self.active()?;
                let last = data.case_count().saturating_sub(1);
                let end = self
                    .predict_bounds(data, 0, None)?
                    .map_or(last, |(_, end)| end);
                (0, last, end)
            }
        };
        let mut warnings = Vec::new();
        let mut runs: Vec<Run> = Vec::new();
        for spec in &c.files {
            let path = self.path(&spec.file);
            let text = std::fs::read_to_string(&path).map_err(|e| {
                Error(format!(
                    "TSAPPLY cannot read the model file {}: {e}",
                    path.display()
                ))
            })?;
            let file: ModelFile = serde_json::from_str(&text)
                .ok()
                .filter(|f: &ModelFile| f.format == FORMAT && f.version == 1)
                .ok_or_else(|| {
                    Error(format!(
                        "TSAPPLY: {} is not a model file of TSMODEL",
                        path.display()
                    ))
                })?;
            let picked: Vec<SavedModel> = file
                .models
                .into_iter()
                .filter(|m| spec.keep.as_ref().is_none_or(|k| k.contains(&m.id)))
                .filter(|m| !spec.drop.contains(&m.id))
                .collect();
            if picked.is_empty() {
                return Err(Error(format!(
                    "TSAPPLY: no model of {} is left to apply",
                    path.display()
                )));
            }
            let mut applied = Vec::new();
            for saved in picked {
                if runs.iter().any(|r| r.id == saved.id) {
                    warnings.push(format!(
                        "TSAPPLY: the model {} is in more than one file; the first one is used",
                        saved.id
                    ));
                    continue;
                }
                match self.apply_model(&saved, out, season, given_season, (first, last, end)) {
                    Ok(run) => {
                        applied.push(run.model.saved(&run.id, &run.dependent, run.place.first));
                        runs.push(run);
                    }
                    Err(e) => warnings.push(format!("TSAPPLY: {e}; the model is not applied")),
                }
            }
            if let Some(outfile) = &spec.outfile {
                self.write_models(outfile, applied, "TSAPPLY")?;
            }
        }
        for w in warnings {
            self.warn(w);
        }
        self.ts_report("TSAPPLY", out, runs, end)
    }

    fn apply_model(
        &self,
        saved: &SavedModel,
        out: &TsOutput,
        season: usize,
        given_season: bool,
        (first, last, end): (usize, usize, usize),
    ) -> Result<Run, String> {
        let data = self.active().map_err(|e| e.0)?;
        let id = &saved.id;
        let read = |name: &str| {
            read_series(data, name, out.include_user_missing, "TSAPPLY")
                .map(|r| r.0)
                .map_err(|_| format!("the model {id} needs the numeric variable {name}"))
        };
        let all = read(&saved.dependent)?;
        let columns: Vec<Values> = saved
            .predictors
            .iter()
            .map(|p| read(p))
            .collect::<Result<_, _>>()?;
        if saved.season > 1 && given_season && season != saved.season {
            return Err(format!(
                "the model {id} has a season length of {}, the active file {season}; give \
                 SEASONLENGTH to match",
                saved.season
            ));
        }
        let kind = match &saved.form {
            Form::Exsmooth { method } => Kind::Es(
                ES_TYPES
                    .iter()
                    .find(|(n, _)| n == method)
                    .map(|(_, t)| *t)
                    .ok_or_else(|| format!("the model {id} has an unknown method"))?,
            ),
            Form::Arima {
                ar,
                ma,
                sar,
                sma,
                d,
                sd,
                constant,
            } => Kind::Arima(Structure {
                ar: ar.clone(),
                ma: ma.clone(),
                sar: sar.clone(),
                sma: sma.clone(),
                d: *d,
                sd: *sd,
                s: if sar.is_empty() && sma.is_empty() && *sd == 0 {
                    1
                } else {
                    saved.season
                },
                regressors: saved.predictors.len(),
                constant: *constant,
            }),
        };
        let transform = match saved.transform.as_str() {
            "SQRT" => TsTransform::Sqrt,
            "LN" => TsTransform::Ln,
            _ => TsTransform::None,
        };
        let (a, y, fixed) = if out.reestimate {
            let a = (first..=last)
                .find(|&t| all[t].is_some())
                .ok_or_else(|| format!("{} has no valid values", saved.dependent))?;
            let b = (first..=last)
                .rev()
                .find(|&t| all[t].is_some())
                .expect("a is valid");
            (a, all[a..=b].to_vec(), None)
        } else {
            (
                saved.first_case - 1,
                saved.values.clone(),
                Some(saved.parameters.as_slice()),
            )
        };
        let est_end = a + y.len() - 1;
        if !out.reestimate && end <= est_end {
            return Err(format!(
                "the forecasts of {id} would start after the end of PREDICT"
            ));
        }
        let horizon = end.saturating_sub(est_end);
        let x: Vec<Values> = columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                (a..=a + y.len() - 1 + horizon)
                    .enumerate()
                    .map(|(t, case)| match (fixed, t < y.len()) {
                        (Some(_), true) => saved.predictor_values[i].get(t).copied().flatten(),
                        _ => c.get(case).copied().flatten(),
                    })
                    .collect()
            })
            .collect();
        let mut model = fit_model(
            kind,
            transform,
            saved.season,
            &y,
            x,
            saved.predictors.clone(),
            horizon,
            fixed,
            out.cilevel,
        )
        .map_err(|e| format!("{} {e}", saved.dependent))?;
        if !out.reestimate {
            for (i, v) in saved.fit.iter().enumerate().take(8) {
                model.stats[i] = *v;
            }
        }
        Ok(Run {
            id: id.clone(),
            dependent: saved.dependent.clone(),
            model,
            place: Place { first: a },
            estimated: out.reestimate,
        })
    }

    /// The tables, charts and saved variables of the models.
    fn ts_report(
        &mut self,
        who: &str,
        out: &TsOutput,
        runs: Vec<Run>,
        end: usize,
    ) -> Result<Vec<Table>, Error> {
        if runs.is_empty() {
            return Err(Error(format!("{who}: no model could be estimated")));
        }
        if self.active()?.weight_variable().is_some() {
            self.warn(format!("{who} does not use case weights"));
        }
        let estimated = runs.iter().all(|r| r.estimated);
        if !estimated
            && (out.summary_acf
                || out.summary_pacf
                || out.residual_acf
                || out.residual_pacf
                || out.plot_acf
                || out.plot_pacf
                || out.series_plot[0]
                || out.series_plot[2]
                || out.series_plot[4]
                || out.save.iter().any(|(s, _)| *s == TsSave::Nresidual))
        {
            self.warn(format!(
                "{who}: residual autocorrelations, observed and fitted values and residuals need \
                 REESTIMATE=YES; they are left out"
            ));
        }
        if out.summary_plots {
            self.warn(format!(
                "{who}: the charts of MODELSUMMARY are not supported yet"
            ));
        }
        let data = self.active()?;
        let shown = self.shown();
        let cases = data.case_count();
        let mut charts = Vec::new();
        // the case labels: DATE_, else the case numbers; past the file from the DATE command
        let date = data.variable("DATE_").ok();
        let future: Vec<String> = match (&self.date_spec, end >= cases) {
            (Some(spec), true) => crate::date::date_columns(spec, end + 1)
                .map(|c| c.1)
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let case_number = |k: usize| {
            self.case_numbers
                .as_ref()
                .and_then(|c| c.get(k).copied())
                .unwrap_or(k)
                + 1
        };
        let label = |k: usize| match (&date, future.get(k)) {
            (Some(v), _) if k < cases => crate::cluster::value_text(v, k).trim().to_string(),
            (_, Some(t)) => t.trim().to_string(),
            _ => case_number(k).to_string(),
        };
        let mut tables = Vec::new();
        tables.push(table(
            "Model Description",
            1,
            plain(&["Model ID", "Series", "Model Type"]),
            runs.iter()
                .map(|r| {
                    vec![
                        text(r.id.clone()),
                        text(shown.named(data, &r.dependent)),
                        text(r.model.type_name()),
                    ]
                })
                .collect(),
            None,
        ));
        if out.summary_fit {
            let rows = (0..8)
                .filter_map(|i| {
                    let v: Vec<f64> = runs.iter().filter_map(|r| r.model.stats[i]).collect();
                    (!v.is_empty()).then(|| summary_row(MEASURE_NAMES[i].into(), &v))
                })
                .collect();
            tables.push(table(
                "Model Fit",
                1,
                plain(&SUMMARY_COLUMNS),
                rows,
                Some("Percentiles 5 to 95 across the models.".into()),
            ));
        }
        let acf_of = |m: &Fitted, partial: bool| -> Vec<f64> {
            let valid = m.resid.iter().flatten().count();
            let lags = out.maxacflags.min(valid.saturating_sub(1));
            let (r, _) = autocorrelations(&m.resid, lags);
            match partial {
                true => partial_autocorrelations(&r),
                false => r,
            }
        };
        for (wanted, partial, title) in [
            (out.summary_acf, false, "Residual ACF Summary"),
            (out.summary_pacf, true, "Residual PACF Summary"),
        ] {
            if !wanted || !estimated {
                continue;
            }
            let all: Vec<Vec<f64>> = runs.iter().map(|r| acf_of(&r.model, partial)).collect();
            let top = all.iter().map(Vec::len).max().unwrap_or(0);
            let rows = (0..top)
                .map(|l| {
                    let v: Vec<f64> = all.iter().filter_map(|a| a.get(l).copied()).collect();
                    summary_row(format!("Lag {}", l + 1), &v)
                })
                .collect();
            tables.push(table(title, 1, plain(&SUMMARY_COLUMNS), rows, None));
        }

        // the sets of models the per-model output covers
        let measure = slot(out.filter_fit);
        let higher_better = measure < 2;
        let mut order: Vec<usize> = (0..runs.len()).collect();
        order.sort_by(|&a, &b| {
            let (x, y) = (runs[a].model.stats[measure], runs[b].model.stats[measure]);
            let key = |v: Option<f64>| v.unwrap_or(f64::NAN);
            match higher_better {
                true => key(y).total_cmp(&key(x)),
                false => key(x).total_cmp(&key(y)),
            }
        });
        let count = |c: FilterCount| match c {
            FilterCount::N(n) => n.min(runs.len()),
            FilterCount::Pct(p) => {
                ((runs.len() as f64 * p / 100.0).ceil() as usize).min(runs.len())
            }
        };
        let measure_name = MEASURE_NAMES[measure];
        let mut sets: Vec<(Vec<usize>, Option<String>)> = Vec::new();
        if out.best.is_none() && out.worst.is_none() {
            sets.push(((0..runs.len()).collect(), None));
        }
        if let Some(c) = out.best {
            sets.push((
                order[..count(c)].to_vec(),
                Some(format!("Best-fitting models by {measure_name}.")),
            ));
        }
        if let Some(c) = out.worst {
            let n = count(c);
            sets.push((
                order[runs.len() - n..].iter().rev().copied().collect(),
                Some(format!("Worst-fitting models by {measure_name}.")),
            ));
        }
        let zq = out.cilevel;
        for (set, note) in &sets {
            if out.statistics {
                let mut columns = vec!["Model", "Number of Predictors"];
                columns.extend(out.statistics_fit.iter().map(|m| MEASURE_NAMES[slot(*m)]));
                columns.extend([
                    "Ljung-Box Q(18) Statistics",
                    "DF",
                    "Sig.",
                    "Number of Outliers",
                ]);
                let rows = set
                    .iter()
                    .map(|&i| {
                        let m = &runs[i].model;
                        let mut row = vec![
                            text(format!(
                                "{}-{}",
                                shown.named(data, &runs[i].dependent),
                                runs[i].id
                            )),
                            Cell::Number(m.predictors.len() as f64),
                        ];
                        row.extend(out.statistics_fit.iter().map(|f| number(m.stats[slot(*f)])));
                        row.extend(m.q.iter().map(|v| number(*v)));
                        row.push(Cell::Number(0.0));
                        row
                    })
                    .collect();
                tables.push(table(
                    "Model Statistics",
                    1,
                    plain(&columns),
                    rows,
                    note.clone(),
                ));
            }
            if out.parameters {
                for arima in [false, true] {
                    let rows: Vec<Vec<Cell>> = set
                        .iter()
                        .filter(|&&i| matches!(runs[i].model.kind, Kind::Arima(_)) == arima)
                        .flat_map(|&i| {
                            let m = &runs[i].model;
                            let df = m.n as f64 - m.k as f64;
                            let name =
                                format!("{}-{}", shown.named(data, &runs[i].dependent), runs[i].id);
                            (0..m.params.len()).map(move |j| {
                                let t = m.se[j].map(|s| m.params[j] / s);
                                vec![
                                    text(name.clone()),
                                    text(m.labels[j].clone()),
                                    number(Some(m.params[j])),
                                    number(m.se[j]),
                                    number(t),
                                    number(t.map(|t| t_p(t, df))),
                                ]
                            })
                        })
                        .collect();
                    if rows.is_empty() {
                        continue;
                    }
                    let title = match arima {
                        true => "ARIMA Model Parameters",
                        false => "Exponential Smoothing Model Parameters",
                    };
                    tables.push(table(
                        title,
                        2,
                        plain(&["Model", "Parameter", "Estimate", "SE", "t", "Sig."]),
                        rows,
                        note.clone(),
                    ));
                }
            }
            for (wanted, partial, title) in [
                (out.residual_acf, false, "Residual ACF"),
                (out.residual_pacf, true, "Residual PACF"),
            ] {
                if !wanted || !estimated {
                    continue;
                }
                let mut rows = Vec::new();
                for &i in set {
                    let m = &runs[i].model;
                    let r = acf_of(m, partial);
                    let n = m.resid.iter().flatten().count() as f64;
                    let mut sum = 0.0;
                    for (l, v) in r.iter().enumerate() {
                        let se = match partial {
                            true => 1.0 / n.sqrt(),
                            false => ((1.0 + 2.0 * sum) / n).sqrt(),
                        };
                        sum += v * v;
                        rows.push(vec![
                            text(format!(
                                "{}-{}",
                                shown.named(data, &runs[i].dependent),
                                runs[i].id
                            )),
                            Cell::Number((l + 1) as f64),
                            number(Some(*v)),
                            number(Some(se)),
                        ]);
                    }
                }
                tables.push(table(
                    title,
                    2,
                    plain(&["Model", "Lag", "Coefficient", "Std. Error"]),
                    rows,
                    note.clone(),
                ));
            }
            if out.forecasts {
                let mut rows = Vec::new();
                for &i in set {
                    let r = &runs[i];
                    let est = r.model.est_len();
                    for t in est..r.model.fit.len() {
                        rows.push(vec![
                            text(format!("{}-{}", shown.named(data, &r.dependent), r.id)),
                            text(label(r.place.first + t)),
                            number(r.model.fit[t]),
                            number(r.model.upper[t]),
                            number(r.model.lower[t]),
                        ]);
                    }
                }
                let mut caption = format!("Limits at {zq}% confidence.");
                if let Some(note) = note {
                    caption = format!("{note} {caption}");
                }
                if rows.is_empty() {
                    caption = format!(
                        "There are no forecasts: PREDICT ends inside the estimation period. {caption}"
                    );
                }
                tables.push(table(
                    "Forecast",
                    2,
                    plain(&["Model", "Period", "Forecast", "UCL", "LCL"]),
                    rows,
                    Some(caption),
                ));
            }
            for &i in set {
                let r = &runs[i];
                let name = format!("{}-{}", shown.named(data, &r.dependent), r.id);
                let est = r.model.est_len();
                let len = r.model.fit.len();
                let flags = out.series_plot;
                if flags.iter().any(|f| *f) {
                    let categories = (0..len)
                        .map(|t| Category {
                            label: label(r.place.first + t),
                            value: Cell::Number(case_number(r.place.first + t) as f64),
                            missing: false,
                            bin: None,
                        })
                        .collect();
                    let part = |v: &Values, from: usize, to: usize| -> Vec<f64> {
                        (0..len)
                            .map(|t| match t >= from && t < to {
                                true => v[t].unwrap_or(f64::NAN),
                                false => f64::NAN,
                            })
                            .collect()
                    };
                    let series = |name: &str, values: Vec<f64>| Series {
                        name: name.into(),
                        statistic: Statistic::Value,
                        variable: None,
                        value: Cell::Empty,
                        values,
                    };
                    let (all, _, _) =
                        read_series(data, &r.dependent, out.include_user_missing, who)?;
                    let observed: Values = (0..len)
                        .map(|t| all.get(r.place.first + t).copied().flatten())
                        .collect();
                    let mut lines = Vec::new();
                    let fit_shown = r.estimated;
                    if flags[0] && fit_shown {
                        lines.push(series("Observed", part(&observed, 0, len)));
                    }
                    if flags[2] && fit_shown {
                        lines.push(series("Fit", part(&r.model.fit, 0, est)));
                    }
                    if flags[1] {
                        lines.push(series("Forecast", part(&r.model.fit, est, len)));
                    }
                    let (mut from, mut to) = (len, 0);
                    if flags[4] && fit_shown {
                        (from, to) = (0, est);
                    }
                    if flags[3] {
                        (from, to) = (from.min(est), len);
                    }
                    if from < to {
                        lines.push(series("UCL", part(&r.model.upper, from, to)));
                        lines.push(series("LCL", part(&r.model.lower, from, to)));
                    }
                    charts.push(Chart::lines(
                        name.clone(),
                        Axis::titled(if date.is_some() {
                            "Date"
                        } else {
                            "Case Number"
                        }),
                        Axis::titled(shown.named(data, &r.dependent)),
                        categories,
                        lines,
                    ));
                }
                for (wanted, partial, title) in [
                    (out.plot_acf, false, "Residual ACF"),
                    (out.plot_pacf, true, "Residual PACF"),
                ] {
                    if !wanted || !r.estimated {
                        continue;
                    }
                    let values = acf_of(&r.model, partial);
                    let n = r.model.resid.iter().flatten().count() as f64;
                    let limit = 2.0 / n.sqrt();
                    let categories = (1..=values.len())
                        .map(|l| Category {
                            label: l.to_string(),
                            value: Cell::Number(l as f64),
                            missing: false,
                            bin: None,
                        })
                        .collect();
                    let series = |name: &str, values: Vec<f64>| Series {
                        name: name.into(),
                        statistic: Statistic::Value,
                        variable: None,
                        value: Cell::Empty,
                        values,
                    };
                    let k = values.len();
                    charts.push(Chart::lines(
                        format!("{title}: {name}"),
                        Axis::titled("Lag"),
                        Axis::titled("Coefficient"),
                        categories,
                        vec![
                            series("Coefficient", values),
                            series("Upper Limit", vec![limit; k]),
                            series("Lower Limit", vec![-limit; k]),
                        ],
                    ));
                }
            }
        }

        self.charts.extend(charts);
        // saved variables
        if !out.save.is_empty() {
            if self.in_temporary() {
                self.warn(format!("{who}: SAVE is ignored after TEMPORARY"));
            } else {
                self.ts_save(out, &runs)?;
            }
        }
        Ok(tables)
    }

    fn ts_save(&mut self, out: &TsOutput, runs: &[Run]) -> Result<(), Error> {
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            let cases = data.case_count();
            for r in runs {
                let est = r.model.est_len();
                for (what, root) in &out.save {
                    let (default, label, values, from) = match what {
                        TsSave::Predicted => ("Predicted", "Predicted value", &r.model.fit, 0),
                        TsSave::Lcl => ("LCL", "Lower confidence limit", &r.model.lower, 0),
                        TsSave::Ucl => ("UCL", "Upper confidence limit", &r.model.upper, 0),
                        TsSave::Nresidual => ("NResidual", "Noise residual", &r.model.resid, 0),
                    };
                    if !r.estimated && *what == TsSave::Nresidual {
                        continue;
                    }
                    let from = if r.estimated { from } else { est };
                    let mut column: Values = vec![None; cases];
                    for (t, v) in values.iter().enumerate().skip(from) {
                        if let Some(slot) = column.get_mut(r.place.first + t) {
                            *slot = *v;
                        }
                    }
                    let base: String = format!(
                        "{}_{}_{}",
                        root.as_deref().unwrap_or(default),
                        r.dependent,
                        r.id
                    )
                    .chars()
                    .take(60)
                    .collect();
                    let name = std::iter::once(base.clone())
                        .chain((1..).map(|i| format!("{base}_{i}")))
                        .find(|n| data.variable_index(n).is_none())
                        .expect("a free name");
                    data.set_numeric_values(&name, column)?;
                    data.set_variable_label(
                        &name,
                        Some(&format!("{label} of {}, {}", r.dependent, r.id)),
                    )?;
                    data.set_format(&name, "F11.5")?;
                    data.set_measure(&name, statscraft_data::Measure::Scale)?;
                }
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_lag_values_are_orders() {
        assert_eq!(lag_list(&[2]), vec![1, 2]);
        assert_eq!(lag_list(&[0]), Vec::<usize>::new());
        assert_eq!(lag_list(&[3, 1, 3]), vec![1, 3]);
        assert_eq!(lag_list(&[0, 2]), vec![2]);
    }

    #[test]
    fn brown_is_holt_with_tied_weights() {
        let q = es_params(EsType::Brown, &[0.4]);
        assert!((q.alpha - 0.64).abs() < 1e-12);
        assert!((q.gamma - 0.25).abs() < 1e-12);
    }

    #[test]
    fn minimize_finds_an_interior_minimum() {
        let f = |v: &[f64]| (v[0] - 0.37).powi(2) + (v[1] - 0.81).powi(2);
        let m = minimize(&f, 2);
        assert!((m[0] - 0.37).abs() < 1e-6 && (m[1] - 0.81).abs() < 1e-6);
    }

    #[test]
    fn differencing_by_the_smallest_variance() {
        // a random walk made of invented steps
        let steps = [
            1.0, -2.0, 3.0, 1.0, -1.0, 2.0, -3.0, 1.0, 2.0, -1.0, 1.0, -2.0,
        ];
        let mut level = 50.0;
        let walk: Values = steps
            .iter()
            .cycle()
            .take(48)
            .map(|s| {
                level += s * 3.0;
                Some(level)
            })
            .collect();
        assert_eq!(expert_differencing(&walk, 1), (1, 0));
        let flat: Values = steps.iter().map(|s| Some(10.0 + s)).collect();
        assert_eq!(expert_differencing(&flat, 1), (0, 0));
    }
}
