//! FIT: goodness-of-fit statistics of residual series.
//!
//! From the Algorithms manual, for residuals e and observed values x over n cases:
//! ME = sum e / n; MPE = 100 / n sum e / x; MAE = sum |e| / n; MAPE = 100 / n sum |e / x|;
//! SSE = sum e squared; MSE = SSE / DFE; RMS = sqrt(MSE); DW = sum (e_t - e_t-1) squared / SSE.
//! DFE is what DFE gives, or n - DFH, or n when neither is given.
//!
//! Choices where the manuals are silent:
//! - A case with a missing residual is left out. The percent statistics also leave out a case
//!   whose observed value is missing or 0 and divide by the cases they use.
//! - DW sums over the pairs of consecutive cases that both have a residual, over the SSE of all
//!   cases that have one.
//! - The statistics cover the USE period (the whole series without USE). When the PREDICT period
//!   starts inside the series, its cases get a column of their own, with DFE equal to its cases.
//! - Without names, the last ERR_n a procedure saved in the session and the series its label
//!   names; the observed series is then used only if it is in the dataset.
//! - Under SPLIT FILE the groups are blocks of rows of the one table, as for the other procedures.

use crate::descriptives::number;
use crate::npar_tests::text;
use crate::regression::table;
use crate::timeseries::read_series;
use crate::{Cell, Engine, Error, Table};
use statscraft_syntax::{Fit, FitFreedom};

/// The observed series a saved error series' label names: "Error of x, ..." or
/// "Residual of x, ...", up to the first comma or " from ".
pub(crate) fn observed_of(label: &str) -> Option<String> {
    let rest = label
        .strip_prefix("Error of ")
        .or_else(|| label.strip_prefix("Residual of "))?;
    let end = [rest.find(", "), rest.find(" from ")]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(rest.len());
    Some(rest[..end].trim().to_string()).filter(|s| !s.is_empty())
}

struct Stats {
    n: usize,
    me: f64,
    mpe: Option<f64>,
    mae: f64,
    mape: Option<f64>,
    sse: f64,
    dfe: f64,
    mse: Option<f64>,
    dw: Option<f64>,
}

/// The statistics of the residuals `e` (None: missing) with the observed values `x`; `dfe` is
/// the error degrees of freedom given, or the number to take from the cases (`Hypothesis`).
fn stats(
    e: &[Option<f64>],
    x: Option<&[Option<f64>]>,
    freedom: Option<(bool, f64)>,
) -> Option<Stats> {
    let valid: Vec<f64> = e.iter().flatten().copied().collect();
    let n = valid.len();
    if n == 0 {
        return None;
    }
    let nf = n as f64;
    let sse: f64 = valid.iter().map(|v| v * v).sum();
    let percent: Vec<f64> = x.map_or_else(Vec::new, |x| {
        e.iter()
            .zip(x)
            .filter_map(|(e, x)| match (e, x) {
                (Some(e), Some(x)) if *x != 0.0 => Some(e / x),
                _ => None,
            })
            .collect()
    });
    let mean_percent = |f: fn(f64) -> f64| {
        (!percent.is_empty())
            .then(|| 100.0 * percent.iter().map(|v| f(*v)).sum::<f64>() / percent.len() as f64)
    };
    let dfe = match freedom {
        None => nf,
        Some((false, v)) => v,
        Some((true, h)) => nf - h,
    };
    let steps: f64 = e
        .windows(2)
        .filter_map(|w| Some((w[1]? - w[0]?).powi(2)))
        .sum();
    Some(Stats {
        n,
        me: valid.iter().sum::<f64>() / nf,
        mpe: mean_percent(|v| v),
        mae: valid.iter().map(|v| v.abs()).sum::<f64>() / nf,
        mape: mean_percent(f64::abs),
        sse,
        dfe,
        mse: (dfe > 0.0).then(|| sse / dfe),
        dw: (sse > 0.0).then(|| steps / sse),
    })
}

impl Engine {
    pub(crate) fn fit(&mut self, command: &Fit) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let (errors, observed): (Vec<String>, Vec<Option<String>>) = if command.errors.is_empty() {
            let Some((name, source)) = self.last_error.clone() else {
                return Err(Error(
                    "FIT names no residual series, and no procedure has saved an ERR_n series in this session"
                        .into(),
                ));
            };
            if data.variable(&name).is_err() {
                return Err(Error(format!(
                    "FIT: the last residual series saved, {name}, is not in the active dataset: name the series"
                )));
            }
            (
                vec![name],
                vec![source.filter(|s| data.variable(s).is_ok())],
            )
        } else {
            let errors = self.expand(&command.errors)?;
            let observed = if command.observed.is_empty() {
                vec![None; errors.len()]
            } else {
                let observed = self.expand(&command.observed)?;
                if observed.len() != errors.len() {
                    return Err(Error(format!(
                        "OBS names {} series for {} residual series: give one for each, repeating a series that several residual series come from",
                        observed.len(),
                        errors.len()
                    )));
                }
                observed.into_iter().map(Some).collect()
            };
            (errors, observed)
        };
        let freedom: Vec<Option<(bool, f64)>> = match &command.freedom {
            None => vec![None; errors.len()],
            Some(FitFreedom::Error(v) | FitFreedom::Hypothesis(v)) if v.len() != errors.len() => {
                return Err(Error(format!(
                    "DFE and DFH name {} values for {} residual series: give one for each",
                    v.len(),
                    errors.len()
                )));
            }
            Some(FitFreedom::Error(v)) => v.iter().map(|d| Some((false, *d))).collect(),
            Some(FitFreedom::Hypothesis(v)) => v.iter().map(|d| Some((true, *d))).collect(),
        };
        let periods = self.periods("FIT")?;
        let data = self.active()?;
        let series = data.case_count() - self.ts_added.min(data.case_count());
        let include = self.tset.include;
        // the USE period, and the PREDICT period when it starts inside the series
        let mut spans = vec![("USE period", periods.first, periods.last)];
        if self.ts_predict.is_some() && periods.origin < series {
            spans.push((
                "PREDICT period",
                periods.origin,
                periods.end.min(series - 1),
            ));
        }
        let mut columns: Vec<(String, String)> = vec![(String::new(), String::new())];
        let mut results: Vec<Option<Stats>> = Vec::new();
        for (i, name) in errors.iter().enumerate() {
            let (e, _, _) = read_series(data, name, include, "FIT")?;
            let x = match &observed[i] {
                Some(o) => Some(read_series(data, o, include, "FIT")?.0),
                None => None,
            };
            for (k, (label, first, last)) in spans.iter().enumerate() {
                let (first, last) = (*first, (*last).min(e.len().saturating_sub(1)));
                if first > last {
                    continue;
                }
                columns.push(match spans.len() {
                    1 => (String::new(), name.clone()),
                    _ => (name.clone(), (*label).to_string()),
                });
                // a PREDICT period has no model to take parameters from
                let freedom = if k == 0 { freedom[i] } else { None };
                results.push(stats(
                    &e[first..=last],
                    x.as_deref().map(|x| &x[first..=last]),
                    freedom,
                ));
            }
        }
        let any_percent = results.iter().flatten().any(|s| s.mpe.is_some());
        type Pick = fn(&Stats) -> Option<f64>;
        let rows: [(&str, Pick, bool); 9] = [
            ("Mean Error", |s| Some(s.me), false),
            ("Mean Percent Error", |s| s.mpe, true),
            ("Mean Absolute Error", |s| Some(s.mae), false),
            ("Mean Absolute Percent Error", |s| s.mape, true),
            ("Sum of Squared Errors", |s| Some(s.sse), false),
            ("Mean Square Error", |s| s.mse, false),
            ("Root Mean Square Error", |s| s.mse.map(f64::sqrt), false),
            ("Durbin-Watson", |s| s.dw, false),
            ("Error Degrees of Freedom", |s| Some(s.dfe), false),
        ];
        let mut out: Vec<Vec<Cell>> = Vec::new();
        for (label, pick, percent) in rows {
            if percent && !any_percent {
                continue;
            }
            let mut row = vec![text(label)];
            row.extend(results.iter().map(|r| match r.as_ref().and_then(pick) {
                Some(v) => number(Some(v)),
                None => Cell::Empty,
            }));
            out.push(row);
        }
        let mut row = vec![text("N of Cases")];
        row.extend(results.iter().map(|r| match r {
            Some(s) => Cell::Number(s.n as f64),
            None => Cell::Number(0.0),
        }));
        out.push(row);
        let observed_names: Vec<&str> = observed.iter().flatten().map(String::as_str).collect();
        let caption = (!observed_names.is_empty()).then(|| {
            format!(
                "Percent errors use the observed series {}.",
                observed_names.join(", ")
            )
        });
        Ok(vec![table("Fit Statistics", 1, columns, out, caption)])
    }
}
