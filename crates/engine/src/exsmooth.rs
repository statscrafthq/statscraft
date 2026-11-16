//! EXSMOOTH: exponential smoothing with no, linear, exponential or damped trend and no,
//! additive or multiplicative season.
//!
//! Rules noted from the Command Syntax Reference and the Algorithms manual:
//! - One-step forecasts: the forecast for case t is made at t - 1; the error is the case less
//!   it. Level S, trend T and seasonal factors I are updated by the recursions of the
//!   Algorithms chapter, one per model; GAMMA, DELTA and PHI enter only the models that use them.
//! - Start values: NN the mean of the series; seasonal models without trend the mean of the
//!   means of the complete periods; linear and damped trends from the first and last value
//!   (non-seasonal) or the first and last complete periods; exponential trends from ratios
//!   of the first two values or period means. Seasonal factors from SEASON's method (equal
//!   weights for an even period) unless SEASFACT gives them; INITIAL gives level and trend.
//! - GRID: every combination; the ten smallest sums of squared errors are shown (all under
//!   TSET PRINT=DETAILED) and the best one is saved as FIT and ERR.
//! - No missing values; seasonal models need four full periods.
//!
//! Choices where the manual is silent, or contradicts itself:
//! - The start values follow the printed formulas, also where they look inconsistent between
//!   models (LA starts from the first value, EM spreads its trend over k - 1); the printed
//!   index t - p - 1 of the DM forecast is read as t - p + 1, as in every other model.
//! - The error degrees of freedom are the number of cases less the number of smoothing
//!   parameters the model uses.
//! - Forecasts h steps ahead: level plus h trends (linear), level times trend to the h
//!   (exponential), level plus phi + ... + phi^h trends (damped), with the latest factor of
//!   the season.
//! - Multiplicative and exponential models need values above 0. ALPHA=0 is left out of the
//!   grid of EM (the manual wants it above 0 there).
//! - SEASFACT with a variable reads its values at the first period of cases of the series.

use crate::descriptives::number;
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::timeseries::{Values, read_series, seasonal_factors};
use crate::{Cell, Engine, Error, Table};
use statscraft_syntax::{
    Exsmooth, SeasonalFactors, Seasonality, Smoothing, Trend, TsetPrint, smoothing_model_name,
};

/// One set of smoothing parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Params {
    alpha: f64,
    gamma: f64,
    delta: f64,
    phi: f64,
}

#[derive(Debug, Clone, Copy)]
struct Model {
    trend: Trend,
    season: Seasonality,
    /// The period (seasonal models).
    p: usize,
}

impl Model {
    fn seasonal(&self) -> bool {
        self.season != Seasonality::None
    }
    fn uses_gamma(&self) -> bool {
        match self.trend {
            Trend::None => false,
            Trend::Damped => !self.seasonal(),
            _ => true,
        }
    }
    fn uses_phi(&self) -> bool {
        self.trend == Trend::Damped
    }
    fn description(&self) -> String {
        let trend = match self.trend {
            Trend::None => "no trend",
            Trend::Linear => "linear trend",
            Trend::Exponential => "exponential trend",
            Trend::Damped => "damped trend",
        };
        let season = match self.season {
            Seasonality::None => "no season",
            Seasonality::Additive => "additive season",
            Seasonality::Multiplicative => "multiplicative season",
        };
        format!(
            "{} ({trend}, {season})",
            smoothing_model_name(self.trend, self.season)
        )
    }
}

/// The start values: level, trend and the factors I_(1-p) .. I_0.
#[derive(Debug, Clone)]
struct Start {
    level: f64,
    trend: f64,
    factors: Vec<f64>,
}

/// The fit of one set of parameters.
struct Fit {
    sse: f64,
    /// The one-step forecast of every case, made at the case before.
    fits: Vec<f64>,
    /// Level and trend after each case (index 0: the start).
    levels: Vec<f64>,
    trends: Vec<f64>,
    /// I_(1-p) .. I_n.
    factors: Vec<f64>,
}

/// The mean of each complete period.
fn period_means(x: &[f64], p: usize) -> Vec<f64> {
    x.chunks_exact(p)
        .map(|c| c.iter().sum::<f64>() / p as f64)
        .collect()
}

fn start_values(
    x: &[f64],
    m: &Model,
    phi: f64,
    factors: &[f64],
    given: Option<(f64, f64)>,
) -> Start {
    let n = x.len();
    let pf = m.p as f64;
    let means = if m.seasonal() {
        period_means(x, m.p)
    } else {
        Vec::new()
    };
    let k = means.len();
    let (level, trend) = match (m.trend, m.seasonal()) {
        (Trend::None, false) => (x.iter().sum::<f64>() / n as f64, 0.0),
        (Trend::None, true) => (means.iter().sum::<f64>() / k as f64, 0.0),
        (Trend::Linear, false) => {
            let t = (x[n - 1] - x[0]) / (n as f64 - 1.0);
            (x[0] - t / 2.0, t)
        }
        (Trend::Linear, true) => {
            let t = (means[k - 1] - means[0]) / ((k as f64 - 1.0) * pf);
            match m.season {
                // as printed: the first value for LA, the first period's mean for LM
                Seasonality::Additive => (x[0] - pf / 2.0 * t, t),
                _ => (means[0] - pf / 2.0 * t, t),
            }
        }
        (Trend::Exponential, false) => {
            let t = x[1] / x[0];
            (x[0] / t.sqrt(), t)
        }
        (Trend::Exponential, true) => {
            let spread = match m.season {
                Seasonality::Additive => pf,
                _ => k as f64 - 1.0,
            };
            let t = ((means[1].ln() - means[0].ln()) / spread).exp();
            ((means[0].ln() - pf / 2.0 * t.ln()).exp(), t)
        }
        (Trend::Damped, false) => {
            let t = (x[n - 1] - x[0]) / ((n as f64 - 1.0) * phi);
            (x[0] - t / 2.0, t)
        }
        (Trend::Damped, true) => {
            let t = (means[k - 1] - means[0]) / ((k as f64 - 1.0) * pf * phi);
            match m.season {
                Seasonality::Additive => (means[0] - pf / 2.0 * t, t),
                _ => (means[0] - pf / 2.0 * t * phi, t),
            }
        }
    };
    let (level, trend) = match given {
        Some((s, t)) => (s, if m.trend == Trend::None { trend } else { t }),
        None => (level, trend),
    };
    Start {
        level,
        trend,
        factors: factors.to_vec(),
    }
}

/// The level-and-trend part of the forecast `h` steps after a case with level `s` and trend
/// `t`.
fn ahead(m: &Model, q: &Params, s: f64, t: f64, h: usize) -> f64 {
    match m.trend {
        Trend::None => s,
        Trend::Linear => s + h as f64 * t,
        Trend::Exponential => s * t.powi(h as i32),
        Trend::Damped => s + (1..=h).map(|i| q.phi.powi(i as i32)).sum::<f64>() * t,
    }
}

fn with_season(m: &Model, value: f64, factor: f64) -> f64 {
    match m.season {
        Seasonality::None => value,
        Seasonality::Additive => value + factor,
        Seasonality::Multiplicative => value * factor,
    }
}

fn smooth(x: &[f64], m: &Model, start: &Start, q: &Params) -> Fit {
    let n = x.len();
    let p = if m.seasonal() { m.p } else { 0 };
    let (a, g, d, phi) = (q.alpha, q.gamma, q.delta, q.phi);
    // I_j sits at j + p - 1 for j from 1 - p
    let mut factors = start.factors.clone();
    let mut levels = vec![start.level];
    let mut trends = vec![start.trend];
    let mut fits = Vec::with_capacity(n);
    let mut sse = 0.0;
    let factor = |f: &[f64], j: usize| if p == 0 { 0.0 } else { f[j] };
    for t in 1..=n {
        let (s, tr) = (levels[t - 1], trends[t - 1]);
        // I_(t-p) is at index t - 1
        let i_old = factor(&factors, t - 1);
        let forecast = with_season(m, ahead(m, q, s, tr, 1), i_old);
        fits.push(forecast);
        let e = x[t - 1] - forecast;
        sse += e * e;
        let per = |e: f64| match m.season {
            Seasonality::Multiplicative => e / i_old,
            _ => e,
        };
        let (s_new, t_new) = match m.trend {
            Trend::None => (s + a * per(e), 0.0),
            Trend::Linear => (s + tr + a * per(e), tr + a * g * per(e)),
            Trend::Exponential => {
                let t_new = match m.season {
                    Seasonality::Multiplicative => tr + a * g * e / (i_old * s),
                    _ => tr + a * g * e / s,
                };
                (s * tr + a * per(e), t_new)
            }
            Trend::Damped if !m.seasonal() => (s + phi * tr + a * e, phi * tr + a * g * e),
            Trend::Damped => (
                s + phi * tr + a * (2.0 - a) * per(e),
                phi * tr + a * (a - phi + 1.0) * per(e),
            ),
        };
        if p > 0 {
            let weight = match m.trend {
                Trend::Damped => d * (1.0 - a * (2.0 - a)),
                _ => d * (1.0 - a),
            };
            let i_new = match m.season {
                Seasonality::Multiplicative => i_old + weight * e / s_new,
                _ => i_old + weight * e,
            };
            factors.push(i_new);
        }
        levels.push(s_new);
        trends.push(t_new);
    }
    Fit {
        sse,
        fits,
        levels,
        trends,
        factors,
    }
}

/// The forecast `h` steps after case `origin` (0: before the first case).
fn forecast(m: &Model, q: &Params, fit: &Fit, origin: usize, h: usize) -> f64 {
    let value = ahead(m, q, fit.levels[origin], fit.trends[origin], h);
    if !m.seasonal() {
        return value;
    }
    let p = m.p;
    // the latest factor of the season of case origin + h: I_j with j <= origin
    let mut j = origin + h;
    while j > origin {
        j -= p;
    }
    // I_j sits at j + p - 1; j >= 1 - p
    with_season(m, value, fit.factors[j + p - 1])
}

/// The values of a GRID or a single value.
fn values(given: Option<Smoothing>, default_grid: (f64, f64, f64)) -> Vec<f64> {
    match given {
        None => vec![0.1],
        Some(Smoothing::Value(v)) => vec![v],
        Some(Smoothing::Grid(range)) => {
            let (from, to, step) = range.unwrap_or(default_grid);
            let mut out = Vec::new();
            let mut i = 0;
            loop {
                let v = ((from + i as f64 * step) * 1e10).round() / 1e10;
                if v > to + 1e-9 {
                    break;
                }
                out.push(v);
                i += 1;
            }
            out
        }
    }
}

impl Engine {
    /// The command over the previous EXSMOOTH for APPLY; keeps the result.
    pub(crate) fn exsmooth_spec(&mut self, command: &Exsmooth) -> Result<Exsmooth, Error> {
        let mut given = match &command.apply {
            None => command.clone(),
            Some(Some(model)) => {
                return Err(Error(format!(
                    "EXSMOOTH APPLY of the named model '{model}' is not supported yet; APPLY \
                     without a name uses the previous EXSMOOTH"
                )));
            }
            Some(None) => {
                let Some(previous) = &self.ts_models.exsmooth else {
                    return Err(Error(
                        "EXSMOOTH APPLY: there is no earlier EXSMOOTH to apply".into(),
                    ));
                };
                let mut given = (**previous).clone();
                if !command.variables.is_empty() {
                    given.variables.clone_from(&command.variables);
                }
                // seasonal factors are never carried over
                given.seasfact.clone_from(&command.seasfact);
                macro_rules! over {
                    ($($field:ident),*) => {
                        $(if command.$field.is_some() {
                            given.$field = command.$field.clone();
                        })*
                    };
                }
                over!(model, period, alpha, gamma, delta, phi, initial);
                given
            }
        };
        given.apply = None;
        // calculated start values are not part of the model
        if given.initial == Some(None) {
            given.initial = None;
        }
        self.ts_models.exsmooth = Some(Box::new(given.clone()));
        Ok(given)
    }

    pub(crate) fn exsmooth(&mut self, spec: &Exsmooth) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let names = self.expand(&spec.variables)?;
        let (trend, season) = spec.model.unwrap_or((Trend::None, Seasonality::None));
        let mut m = Model {
            trend,
            season,
            p: 0,
        };
        if m.seasonal() {
            let Some(p) = self.ts_period(spec.period) else {
                return Err(Error(
                    "EXSMOOTH: a seasonal model needs a period: give PERIOD, TSET PERIOD or DATE \
                     variables"
                        .into(),
                ));
            };
            if p < 2 {
                return Err(Error(
                    "EXSMOOTH: a seasonal model needs a period of at least 2".into(),
                ));
            }
            m.p = p;
        }
        let periods = self.periods("EXSMOOTH")?;
        let include = self.tset.include;
        let detailed = self.tset.print == TsetPrint::Detailed;
        let given_initial = spec.initial.flatten();

        // the parameter grid
        let alphas: Vec<f64> = values(spec.alpha, (0.0, 1.0, 0.1))
            .into_iter()
            .filter(|&a| {
                !(a == 0.0
                    && m.trend == Trend::Exponential
                    && m.season == Seasonality::Multiplicative)
            })
            .collect();
        if alphas.is_empty() {
            return Err(Error("EXSMOOTH: ALPHA must be above 0 for EM".into()));
        }
        let gammas = match m.uses_gamma() {
            true => values(spec.gamma, (0.0, 1.0, 0.2)),
            false => vec![f64::NAN],
        };
        let deltas = match m.seasonal() {
            true => values(spec.delta, (0.0, 1.0, 0.2)),
            false => vec![f64::NAN],
        };
        let phis = match m.uses_phi() {
            true => values(spec.phi, (0.1, 0.9, 0.2)),
            false => vec![f64::NAN],
        };
        let mut grid = Vec::new();
        for &alpha in &alphas {
            for &gamma in &gammas {
                for &delta in &deltas {
                    for &phi in &phis {
                        grid.push(Params {
                            alpha,
                            gamma,
                            delta,
                            phi,
                        });
                    }
                }
            }
        }
        let used =
            1 + usize::from(m.uses_gamma()) + usize::from(m.seasonal()) + usize::from(m.uses_phi());

        let factor_source = match (&spec.seasfact, m.seasonal()) {
            (_, false) => None,
            (None, true) => Some("Computed by the seasonal decomposition method".to_string()),
            (Some(SeasonalFactors::Values(_)), true) => Some("Given on SEASFACT".to_string()),
            (Some(SeasonalFactors::Variable(v)), true) => Some(format!("Read from {v}")),
        };
        let mut model_rows = vec![
            vec![
                text("Series"),
                text(
                    names
                        .iter()
                        .map(|n| shown.named(data, n))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            ],
            vec![text("Model"), text(m.description())],
        ];
        if m.seasonal() {
            model_rows.push(vec![text("Period"), Cell::Number(m.p as f64)]);
        }
        if let Some(source) = &factor_source {
            model_rows.push(vec![text("Initial seasonal factors"), text(source.clone())]);
        }
        model_rows.push(vec![
            text("Cases used"),
            text(format!("{} to {}", periods.first + 1, periods.last + 1)),
        ]);
        let mut tables = vec![table(
            "Model Description",
            1,
            plain(&["", "Specification"]),
            model_rows,
            None,
        )];

        let cases = data.case_count();
        let mut saved = Vec::new();
        let mut warnings = Vec::new();
        for name in &names {
            let (all, _, _) = read_series(data, name, include, "EXSMOOTH")?;
            let Some(x): Option<Vec<f64>> =
                all[periods.first..=periods.last].iter().copied().collect()
            else {
                return Err(Error(format!(
                    "EXSMOOTH cannot use {name}: it has missing values; replace them first (RMV) \
                     or leave them out with USE"
                )));
            };
            let n = x.len();
            if m.seasonal() && n < 4 * m.p {
                return Err(Error(format!(
                    "EXSMOOTH needs at least four full periods ({}) of {name}, which has {n} cases",
                    4 * m.p
                )));
            }
            if n < 2 {
                return Err(Error(format!("EXSMOOTH needs at least 2 cases of {name}")));
            }
            let positive = m.season == Seasonality::Multiplicative || m.trend == Trend::Exponential;
            if positive && x.iter().any(|v| *v <= 0.0) {
                return Err(Error(format!(
                    "EXSMOOTH: the model {} needs values above 0, but {name} has others",
                    smoothing_model_name(m.trend, m.season)
                )));
            }
            // the initial seasonal factors, as ratios for the multiplicative season
            let factors: Vec<f64> = match (&spec.seasfact, m.seasonal()) {
                (_, false) => Vec::new(),
                (None, true) => {
                    let additive = m.season == Seasonality::Additive;
                    let (_, _, f) = seasonal_factors(&x, m.p, additive, m.p % 2 == 1);
                    match additive {
                        true => f,
                        false => f.iter().map(|v| v / 100.0).collect(),
                    }
                }
                (Some(SeasonalFactors::Values(v)), true) => {
                    if v.len() != m.p {
                        return Err(Error(format!(
                            "EXSMOOTH: SEASFACT gives {} values, but the period is {}",
                            v.len(),
                            m.p
                        )));
                    }
                    v.clone()
                }
                (Some(SeasonalFactors::Variable(v)), true) => {
                    let (column, _, _) = read_series(data, v, include, "EXSMOOTH")?;
                    let first: Option<Vec<f64>> = column[periods.first..periods.first + m.p]
                        .iter()
                        .copied()
                        .collect();
                    first.ok_or_else(|| {
                        Error(format!(
                            "EXSMOOTH: the seasonal factors in {v} are missing in the first period"
                        ))
                    })?
                }
            };
            if m.season == Seasonality::Multiplicative && factors.iter().any(|f| *f <= 0.0) {
                return Err(Error(
                    "EXSMOOTH: multiplicative seasonal factors must be above 0".into(),
                ));
            }
            let mut fits: Vec<(Params, Fit, Start)> = grid
                .iter()
                .map(|q| {
                    let start = start_values(&x, &m, q.phi, &factors, given_initial);
                    (*q, smooth(&x, &m, &start, q), start)
                })
                .collect();
            if fits.iter().any(|f| !f.1.sse.is_finite()) {
                warnings.push(format!(
                    "EXSMOOTH: some parameters give no finite fit of {name}; they are left out"
                ));
                fits.retain(|f| f.1.sse.is_finite());
            }
            if fits.is_empty() {
                return Err(Error(format!("EXSMOOTH: no parameters fit {name}")));
            }
            fits.sort_by(|a, b| a.1.sse.total_cmp(&b.1.sse));
            let label = shown.named(data, name);
            let (best_q, best, best_start) = &fits[0];

            // the start values of the best fit
            let mut rows = vec![vec![text("Series"), Cell::Number(best_start.level)]];
            if m.trend != Trend::None {
                rows.push(vec![text("Trend"), Cell::Number(best_start.trend)]);
            }
            for (i, f) in best_start.factors.iter().enumerate() {
                rows.push(vec![
                    text(format!("Seasonal factor {}", i + 1)),
                    Cell::Number(*f),
                ]);
            }
            tables.push(table(
                "Initial Values",
                1,
                plain(&["", "Value"]),
                rows,
                Some(format!(
                    "Series: {label}. For the parameters with the smallest sum of squared errors."
                )),
            ));
            let mut columns = vec!["Alpha"];
            if m.uses_gamma() {
                columns.push("Gamma");
            }
            if m.seasonal() {
                columns.push("Delta");
            }
            if m.uses_phi() {
                columns.push("Phi");
            }
            columns.push("Sum of Squared Errors");
            let shown_fits = if detailed {
                fits.len()
            } else {
                fits.len().min(10)
            };
            let rows = fits[..shown_fits]
                .iter()
                .map(|(q, f, _)| {
                    let mut row = vec![Cell::Number(q.alpha)];
                    if m.uses_gamma() {
                        row.push(Cell::Number(q.gamma));
                    }
                    if m.seasonal() {
                        row.push(Cell::Number(q.delta));
                    }
                    if m.uses_phi() {
                        row.push(Cell::Number(q.phi));
                    }
                    row.push(number(Some(f.sse)));
                    row
                })
                .collect();
            let dfe = n.saturating_sub(used);
            tables.push(table(
                "Smallest Sums of Squared Errors",
                0,
                plain(&columns),
                rows,
                Some(format!(
                    "Series: {label}. Error degrees of freedom: {dfe}. Sorted from the smallest sum."
                )),
            ));

            // fitted values and forecasts over the dataset
            let mut fit_column: Values = vec![None; cases];
            let mut err_column: Values = vec![None; cases];
            let origin = periods.origin - periods.first;
            for d in periods.first..periods.origin {
                let t = d - periods.first;
                fit_column[d] = Some(best.fits[t]);
                err_column[d] = Some(x[t] - best.fits[t]);
            }
            for (h, d) in (periods.origin..=periods.end).enumerate() {
                fit_column[d] = Some(forecast(&m, best_q, best, origin, h + 1));
            }
            let mut parameters = format!("A {}", best_q.alpha);
            if m.uses_gamma() {
                parameters.push_str(&format!(" G {}", best_q.gamma));
            }
            if m.seasonal() {
                parameters.push_str(&format!(" D {}", best_q.delta));
            }
            if m.uses_phi() {
                parameters.push_str(&format!(" P {}", best_q.phi));
            }
            let model = smoothing_model_name(m.trend, m.season);
            saved.push((
                vec![
                    format!("Fit of {name}, EXSMOOTH {model}, {parameters}"),
                    format!("Error of {name}, EXSMOOTH {model}, {parameters}"),
                ],
                vec![fit_column, err_column],
            ));
        }
        for w in warnings {
            self.warn(w);
        }
        self.save_series("EXSMOOTH", &["FIT", "ERR"], saved)?;
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(alpha: f64, gamma: f64, delta: f64, phi: f64) -> Params {
        Params {
            alpha,
            gamma,
            delta,
            phi,
        }
    }

    #[test]
    fn simple_smoothing_by_hand() {
        let x = [10.0, 12.0, 11.0, 15.0];
        let m = Model {
            trend: Trend::None,
            season: Seasonality::None,
            p: 0,
        };
        let start = start_values(&x, &m, 0.0, &[], None);
        assert_eq!(start.level, 12.0);
        let f = smooth(&x, &m, &start, &q(0.5, 0.0, 0.0, 0.0));
        // forecasts 12, 11, 11.5, 11.25
        assert_eq!(f.fits, vec![12.0, 11.0, 11.5, 11.25]);
        let sse = 4.0 + 1.0 + 0.25 + 3.75 * 3.75;
        assert!((f.sse - sse).abs() < 1e-12);
        assert_eq!(forecast(&m, &q(0.5, 0.0, 0.0, 0.0), &f, 4, 3), 13.125);
    }

    #[test]
    fn holt_forecasts_add_the_trend() {
        let x = [3.0, 5.0, 7.0, 9.0];
        let m = Model {
            trend: Trend::Linear,
            season: Seasonality::None,
            p: 0,
        };
        let start = start_values(&x, &m, 0.0, &[], None);
        // the printed start is the level half a step before the first case
        assert_eq!((start.level, start.trend), (2.0, 2.0));
        let f = smooth(&x, &m, &start, &q(0.5, 0.4, 0.0, 0.0));
        // forecast 4, error -1: level 2 + 2 - 0.5, trend 2 - 0.2
        assert_eq!(f.fits[0], 4.0);
        assert!((f.levels[1] - 3.5).abs() < 1e-12 && (f.trends[1] - 1.8).abs() < 1e-12);
        assert!((f.fits[1] - 5.3).abs() < 1e-12);
        let last = (f.levels[4], f.trends[4]);
        let ahead = forecast(&m, &q(0.5, 0.4, 0.0, 0.0), &f, 4, 2);
        assert!((ahead - (last.0 + 2.0 * last.1)).abs() < 1e-12);
    }

    #[test]
    fn grids_run_from_start_to_end() {
        assert_eq!(
            values(
                Some(Smoothing::Grid(Some((0.2, 0.6, 0.2)))),
                (0.0, 1.0, 0.1)
            ),
            vec![0.2, 0.4, 0.6]
        );
        assert_eq!(
            values(Some(Smoothing::Grid(None)), (0.0, 1.0, 0.1)).len(),
            11
        );
    }
}
