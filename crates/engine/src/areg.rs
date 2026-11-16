//! AREG: linear regression whose errors follow a first-order autoregressive process, by
//! Prais-Winsten, Cochrane-Orcutt or exact maximum likelihood.
//!
//! Rules noted from the Command Syntax Reference and the Algorithms manual:
//! - The model y_t = a + sum b_i x_ti + u_t, u_t = rho u_(t-1) + e_t; a only with CONSTANT
//!   (the default unless TSET NOCONSTANT).
//! - Cochrane-Orcutt: from a start rho (RHO, default 0), least squares on y_t - rho y_(t-1)
//!   against (1 - rho) and x_t - rho x_(t-1) for t = 2 .. n; the residuals u of the original
//!   equation give the next rho = sum u_t u_(t-1) (t from 2) / sum u_t^2 (t from 1).
//!   Prais-Winsten adds the first case scaled by sqrt(1 - rho^2). Iteration stops when every
//!   estimate changes by less than TSET CNVERGE or after MXITER iterations.
//! - At each iteration: rho, its standard error sqrt((1 - rho^2) / (n - 1 - p*)), the
//!   Durbin-Watson statistic of e_1 = sqrt(1 - rho^2) u_1, e_t = u_t - rho u_(t-1), and the
//!   mean square error sum e_t^2 (t from 2) / (n - 2 - p*); p* counts the coefficients.
//!   Prais-Winsten's error degrees of freedom are n - 1 - p*.
//! - FIT: yhat_1 at the first case, yhat_t + rho u_(t-1) after it; ERR: y_t - FIT_t, the
//!   first one times sqrt(1 - rho^2). SEP: sqrt(MSE) sqrt(1 / (1 - rho^2) + h_1) at the first
//!   case, sqrt(MSE) sqrt(1 + h_t) after it, h_t = x_t (X*'X*)^-1 x_t' with X* the transformed
//!   design. Limits FIT -+ t SEP, with n - 1 - p* degrees of freedom (Prais-Winsten n - p*).
//! - PW and CO skip missing values at the ends of the series and stop at missing values
//!   inside it; ML takes missing values anywhere.
//! - ML is the ARIMA model (1,0,0) with the regressors, estimated by exact likelihood.
//!
//! Choices where the manual is silent:
//! - Iteration k takes the rho of the residuals of iteration k - 1 and refits the
//!   coefficients at it; the final table shows that rho and those coefficients, their
//!   standard errors sqrt(MSE) times the roots of the diagonal of (X*'X*)^-1.
//! - Prais-Winsten's mean square error includes the first, scaled residual, as the weighted
//!   least squares it rests on does.
//! - R square is that of the transformed regression (about the mean of the transformed
//!   dependent with a constant, about 0 without).
//! - Forecasts h steps past the last value add rho^h u_last to yhat; their standard error is
//!   sqrt(MSE) sqrt(1 + rho^2 + ... + rho^(2(h-1)) + h_t). The file is not extended, as the
//!   regressors are needed.

use crate::arima::Estimates;
use crate::descriptives::number;
use crate::dist::{f_p, t_p, t_quantile};
use crate::matrix::inverse;
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::timeseries::{Values, read_series};
use crate::{Cell, Engine, Error, Table};
use statscraft_syntax::{Areg, AregMethod, Arima, ArimaApply, TsetPrint};

/// The last AREG with its estimates, for APPLY.
#[derive(Debug, Clone)]
pub(crate) struct Previous {
    spec: Areg,
    rho: f64,
    /// The regressors' coefficients, then the constant when there is one.
    coefficients: Vec<f64>,
}

/// What APPLY INITIAL or FIT passes on to the run.
#[derive(Debug, Clone)]
pub(crate) struct Start {
    rho: f64,
    coefficients: Vec<f64>,
    fixed: bool,
}

/// Least squares on the transformed equation at one rho.
struct Fit {
    beta: Vec<f64>,
    /// (X*'X*)^-1.
    inverse: Vec<Vec<f64>>,
    /// The total sum of squares of the transformed dependent about its mean (with a
    /// constant) or about 0.
    sst: f64,
    rows: usize,
}

/// The transformed rows at `rho`: Cochrane-Orcutt drops the first case, Prais-Winsten scales it.
fn transformed(
    y: &[f64],
    x: &[Vec<f64>],
    rho: f64,
    prais: bool,
    constant: bool,
) -> (Vec<f64>, Vec<Vec<f64>>) {
    let row = |t: usize| -> Vec<f64> {
        let mut r: Vec<f64> = x.iter().map(|c| c[t] - rho * c[t - 1]).collect();
        if constant {
            r.push(1.0 - rho);
        }
        r
    };
    let (mut ys, mut xs) = (Vec::new(), Vec::new());
    if prais {
        let s = (1.0 - rho * rho).sqrt();
        ys.push(s * y[0]);
        let mut r: Vec<f64> = x.iter().map(|c| s * c[0]).collect();
        if constant {
            r.push(s);
        }
        xs.push(r);
    }
    for t in 1..y.len() {
        ys.push(y[t] - rho * y[t - 1]);
        xs.push(row(t));
    }
    (ys, xs)
}

fn least_squares(ys: &[f64], xs: &[Vec<f64>], constant: bool) -> Option<Fit> {
    let k = xs.first()?.len();
    let mut xtx = vec![vec![0.0; k]; k];
    let mut xty = vec![0.0; k];
    for (r, yv) in xs.iter().zip(ys) {
        for i in 0..k {
            xty[i] += r[i] * yv;
            for j in 0..k {
                xtx[i][j] += r[i] * r[j];
            }
        }
    }
    let (inv, _) = inverse(&xtx)?;
    let beta: Vec<f64> = (0..k)
        .map(|i| (0..k).map(|j| inv[i][j] * xty[j]).sum())
        .collect();
    let mean = if constant {
        ys.iter().sum::<f64>() / ys.len() as f64
    } else {
        0.0
    };
    let sst = ys.iter().map(|v| (v - mean).powi(2)).sum();
    Some(Fit {
        beta,
        inverse: inv,
        sst,
        rows: ys.len(),
    })
}

/// The residuals of the original equation.
fn residuals(y: &[f64], x: &[Vec<f64>], beta: &[f64], constant: bool) -> Vec<f64> {
    (0..y.len())
        .map(|t| {
            let mut fit: f64 = x.iter().zip(beta).map(|(c, b)| b * c[t]).sum();
            if constant {
                fit += beta[x.len()];
            }
            y[t] - fit
        })
        .collect()
}

fn next_rho(u: &[f64]) -> f64 {
    let num: f64 = u.windows(2).map(|w| w[0] * w[1]).sum();
    let den: f64 = u.iter().map(|v| v * v).sum();
    if den > 0.0 { num / den } else { 0.0 }
}

/// Rho's standard error, the Durbin-Watson statistic and the mean square error.
struct Statistics {
    se_rho: f64,
    dw: f64,
    mse: f64,
}

fn statistics(u: &[f64], rho: f64, pstar: usize, prais: bool) -> Statistics {
    let n = u.len() as f64;
    let p = pstar as f64;
    let mut e = vec![(1.0 - rho * rho).sqrt() * u[0]];
    e.extend(u.windows(2).map(|w| w[1] - rho * w[0]));
    let sum_e2: f64 = e.iter().map(|v| v * v).sum();
    let dw = e.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f64>() / sum_e2;
    let mse = if prais {
        sum_e2 / (n - 1.0 - p)
    } else {
        (sum_e2 - e[0] * e[0]) / (n - 2.0 - p)
    };
    Statistics {
        se_rho: ((1.0 - rho * rho) / (n - 1.0 - p)).sqrt(),
        dw,
        mse,
    }
}

impl Engine {
    /// The command over the previous AREG for APPLY; INITIAL and FIT pass its estimates on.
    pub(crate) fn areg_spec(&mut self, command: &Areg) -> Result<Areg, Error> {
        self.ts_models.areg_start = None;
        let given = match &command.apply {
            None => command.clone(),
            Some((Some(model), _)) => {
                return Err(Error(format!(
                    "AREG APPLY of the named model '{model}' is not supported yet; APPLY \
                     without a name uses the previous AREG"
                )));
            }
            Some((None, how)) => {
                let Some(previous) = &self.ts_models.areg else {
                    return Err(Error(
                        "AREG APPLY: there is no earlier AREG to apply".into(),
                    ));
                };
                let mut given = previous.spec.clone();
                if !command.dependent.is_empty() {
                    given.dependent.clone_from(&command.dependent);
                    given.regressors.clone_from(&command.regressors);
                }
                if *how != ArimaApply::Specifications {
                    self.ts_models.areg_start = Some(Start {
                        // a RHO given now wins over the earlier estimate
                        rho: command.rho.unwrap_or(previous.rho),
                        coefficients: previous.coefficients.clone(),
                        fixed: *how == ArimaApply::Fit,
                    });
                }
                if *how == ArimaApply::Fit {
                    given.mxiter = Some(0);
                }
                macro_rules! over {
                    ($($field:ident),*) => {
                        $(if command.$field.is_some() {
                            given.$field = command.$field.clone();
                        })*
                    };
                }
                over!(method, constant, rho, mxiter);
                given
            }
        };
        let mut given = given;
        given.apply = None;
        Ok(given)
    }

    pub(crate) fn areg(&mut self, spec: &Areg) -> Result<Vec<Table>, Error> {
        let start = self.ts_models.areg_start.take();
        let method = spec.method.unwrap_or(AregMethod::Pw);
        let constant = spec.constant.unwrap_or(self.tset.constant);
        let regressors = self.expand(&spec.regressors)?;
        // the given estimates fit only a model of the same shape
        let start =
            start.filter(|s| s.coefficients.len() == regressors.len() + usize::from(constant));
        let rho0 = start.as_ref().map(|s| s.rho).or(spec.rho).unwrap_or(0.0);
        let mxiter = spec.mxiter.unwrap_or(10);
        if method == AregMethod::Ml {
            let (reg, con) = match &start {
                Some(s) => (
                    Some(s.coefficients[..regressors.len()].to_vec()),
                    constant.then(|| s.coefficients[regressors.len()]),
                ),
                None => (None, None),
            };
            let arima = Arima {
                dependent: spec.dependent.clone(),
                regressors: spec.regressors.clone(),
                orders: Some([1, 0, 0]),
                constant: Some(constant),
                ar: Some(vec![rho0]),
                reg,
                con,
                mxiter: Some(mxiter),
                ..Arima::default()
            };
            let (tables, e): (Vec<Table>, Estimates) = self.arima_model(&arima, "AREG")?;
            let mut coefficients = e.reg.clone();
            coefficients.extend(e.con);
            self.ts_models.areg = Some(Previous {
                spec: spec.clone(),
                rho: e.ar.first().copied().unwrap_or(0.0),
                coefficients,
            });
            return Ok(tables);
        }
        let prais = method == AregMethod::Pw;
        let what = if prais {
            "Prais-Winsten"
        } else {
            "Cochrane-Orcutt"
        };
        let data = self.active()?;
        let shown = self.shown();
        let include = self.tset.include;
        let print = self.tset.print;
        let level = self.tset.cin;
        let cnverge = self.tset.cnverge;
        let dependent = self.expand(&spec.dependent)?;
        let [dependent] = dependent.as_slice() else {
            return Err(Error("AREG takes one dependent series".into()));
        };
        let periods = self.periods("AREG")?;
        let window = periods.first..=periods.end.max(periods.last);
        let (all, _, _) = read_series(data, dependent, include, "AREG")?;
        let y_all: Values = all[window.clone()].to_vec();
        let mut x_all: Vec<Values> = Vec::new();
        for name in &regressors {
            let (column, _, _) = read_series(data, name, include, "AREG")?;
            x_all.push(column[window.clone()].to_vec());
        }
        let used_len = periods.last + 1 - periods.first;
        let origin = periods.origin - periods.first;
        let valid = |t: usize| y_all[t].is_some() && x_all.iter().all(|c| c[t].is_some());
        let Some(lo) = (0..used_len).find(|&t| valid(t)) else {
            return Err(Error(format!(
                "AREG: no case has valid values of {dependent} and every regressor"
            )));
        };
        let hi = (0..used_len)
            .rev()
            .find(|&t| valid(t))
            .expect("lo is valid");
        if (lo..=hi).any(|t| !valid(t)) {
            return Err(Error(format!(
                "AREG: {dependent} or a regressor has missing values inside the series; \
                 METHOD={} takes missing values only at the ends, METHOD=ML anywhere",
                if prais { "PW" } else { "CO" }
            )));
        }
        let y: Vec<f64> = (lo..=hi).map(|t| y_all[t].expect("valid")).collect();
        if y.iter().all(|&v| v == y[0]) {
            return Err(Error(format!(
                "AREG: {dependent} has the same value in every case of the series, so there is \
                 nothing to fit"
            )));
        }
        let x: Vec<Vec<f64>> = x_all
            .iter()
            .map(|c| (lo..=hi).map(|t| c[t].expect("valid")).collect())
            .collect();
        let n = y.len();
        let pstar = regressors.len() + usize::from(constant);
        let df_mse = n as f64 - pstar as f64 - if prais { 1.0 } else { 2.0 };
        if df_mse < 1.0 {
            return Err(Error(format!(
                "AREG: {n} cases are too few for {pstar} coefficients and rho"
            )));
        }
        let fit_at = |rho: f64| -> Result<Fit, Error> {
            let (ys, xs) = transformed(&y, &x, rho, prais, constant);
            least_squares(&ys, &xs, constant)
                .ok_or_else(|| Error("AREG: the regressors are linearly dependent".into()))
        };
        // iteration 0: the start rho with its least squares fit (or the applied estimates)
        let mut rho = rho0;
        let first = fit_at(rho)?;
        let mut beta = match &start {
            Some(s) if s.fixed => s.coefficients.clone(),
            _ => first.beta.clone(),
        };
        let initial = (rho, first);
        struct Row {
            rho: f64,
            stats: Statistics,
            beta: Vec<f64>,
        }
        let mut history = vec![Row {
            rho,
            stats: statistics(&residuals(&y, &x, &beta, constant), rho, pstar, prais),
            beta: beta.clone(),
        }];
        let mut reason = if mxiter == 0 {
            "MXITER=0: no iterations".to_string()
        } else {
            format!("the maximum of {mxiter} iterations was reached")
        };
        for _ in 0..mxiter {
            let u = residuals(&y, &x, &beta, constant);
            let new_rho = next_rho(&u);
            if new_rho.abs() >= 1.0 {
                reason = "rho reached 1 or more; the last valid estimates are kept".into();
                break;
            }
            let fit = fit_at(new_rho)?;
            let change = beta
                .iter()
                .zip(&fit.beta)
                .map(|(a, b)| (a - b).abs())
                .fold((new_rho - rho).abs(), f64::max);
            rho = new_rho;
            beta = fit.beta;
            history.push(Row {
                rho,
                stats: statistics(&residuals(&y, &x, &beta, constant), rho, pstar, prais),
                beta: beta.clone(),
            });
            if change < cnverge {
                reason = format!("no estimate changed by {cnverge} or more (TSET CNVERGE)");
                break;
            }
        }
        let final_fit = fit_at(rho)?;
        let u = residuals(&y, &x, &beta, constant);
        let stats = statistics(&u, rho, pstar, prais);

        // labels
        let label = shown.named(data, dependent);
        let mut names: Vec<String> = regressors.iter().map(|r| shown.named(data, r)).collect();
        if constant {
            names.push("Constant".into());
        }
        let mut tables = vec![table(
            "Model Description",
            1,
            plain(&["", "Specification"]),
            vec![
                vec![text("Dependent series"), text(label.clone())],
                vec![
                    text("Regressors"),
                    text(names[..regressors.len()].join(", ")),
                ],
                vec![text("Method"), text(what)],
                vec![
                    text("Constant"),
                    text(if constant { "Included" } else { "Not included" }),
                ],
                vec![text("Initial rho"), number(Some(rho0))],
                vec![
                    text("Cases used"),
                    text(format!(
                        "{} to {}",
                        periods.first + lo + 1,
                        periods.first + hi + 1
                    )),
                ],
            ],
            None,
        )];
        if print != TsetPrint::Brief {
            let mut columns = vec![
                "Iteration",
                "Rho",
                "Std. Error of Rho",
                "Durbin-Watson",
                "Mean Square Error",
            ];
            let detailed = print == TsetPrint::Detailed;
            if detailed {
                columns.extend(names.iter().map(String::as_str));
            }
            let rows = history
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let mut row = vec![
                        Cell::Number(i as f64),
                        number(Some(r.rho)),
                        number(Some(r.stats.se_rho)),
                        number(Some(r.stats.dw)),
                        number(Some(r.stats.mse)),
                    ];
                    if detailed {
                        row.extend(r.beta.iter().map(|v| number(Some(*v))));
                    }
                    row
                })
                .collect();
            tables.push(table(
                "Iteration History",
                0,
                plain(&columns),
                rows,
                Some(format!("Estimation stopped because {reason}.")),
            ));
            // the fit at the start and at the end
            // the transformed sum of squares of coefficients that APPLY FIT may have fixed
            let sse_of = |rho: f64, beta: &[f64]| -> f64 {
                let (ys, xs) = transformed(&y, &x, rho, prais, constant);
                ys.iter()
                    .zip(&xs)
                    .map(|(v, r)| (v - r.iter().zip(beta).map(|(a, b)| a * b).sum::<f64>()).powi(2))
                    .sum()
            };
            let summary = |fit: &Fit, rho: f64, beta: &[f64]| -> Vec<f64> {
                let s = statistics(&residuals(&y, &x, beta, constant), rho, pstar, prais);
                let r2 = 1.0 - sse_of(rho, beta) / fit.sst;
                let m = fit.rows as f64 - f64::from(u8::from(constant));
                let adjusted = 1.0 - (1.0 - r2) * m / (fit.rows - pstar) as f64;
                vec![r2.max(0.0).sqrt(), r2, adjusted, s.mse.sqrt(), s.dw]
            };
            let start_values = summary(&initial.1, initial.0, &history[0].beta);
            let end_values = summary(&final_fit, rho, &beta);
            let names_of = [
                "Multiple R",
                "R Square",
                "Adjusted R Square",
                "Std. Error of the Estimate",
                "Durbin-Watson",
            ];
            tables.push(table(
                "Model Fit Summary",
                1,
                plain(&[
                    "",
                    if initial.0 == 0.0 {
                        "Ordinary Least Squares"
                    } else {
                        "Initial"
                    },
                    "Final",
                ]),
                names_of
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        vec![
                            text(*n),
                            number(Some(start_values[i])),
                            number(Some(end_values[i])),
                        ]
                    })
                    .collect(),
                Some(format!(
                    "Series: {label}. R square is that of the transformed regression."
                )),
            ));
            let ssr = final_fit.sst - sse_of(rho, &beta);
            let df_reg = regressors.len() as f64;
            let sse = stats.mse * df_mse;
            let f = (ssr / df_reg) / stats.mse;
            tables.push(table(
                "ANOVA",
                1,
                plain(&["", "Sum of Squares", "df", "Mean Square", "F", "Sig."]),
                vec![
                    vec![
                        text("Regression"),
                        number(Some(ssr)),
                        Cell::Number(df_reg),
                        number(Some(ssr / df_reg)),
                        number(Some(f)),
                        number(Some(f_p(f, df_reg, df_mse))),
                    ],
                    vec![
                        text("Residual"),
                        number(Some(sse)),
                        Cell::Number(df_mse),
                        number(Some(stats.mse)),
                        Cell::Empty,
                        Cell::Empty,
                    ],
                ],
                Some(format!("Series: {label}. Final {what} estimates.")),
            ));
        }
        let t_row = |name: String, est: f64, se: f64| {
            let t = est / se;
            vec![
                text(name),
                number(Some(est)),
                number(Some(se)),
                number(Some(t)),
                number(Some(t_p(t, df_mse))),
            ]
        };
        let mut rows = vec![t_row("Rho (AR1)".into(), rho, stats.se_rho)];
        for (i, name) in names.iter().enumerate() {
            let se = (stats.mse * final_fit.inverse[i][i]).sqrt();
            rows.push(t_row(name.clone(), beta[i], se));
        }
        tables.push(table(
            "Parameter Estimates",
            1,
            plain(&["", "Estimate", "Std. Error", "t", "Approx Sig."]),
            rows,
            Some(format!("Series: {label}. {what} estimates.")),
        ));

        // fitted values, residuals, limits and standard errors over the window
        let cases = data.case_count();
        let mut columns: Vec<Values> = vec![vec![None; cases]; 5];
        let df_ci = n as f64 - pstar as f64 - if prais { 0.0 } else { 1.0 };
        let t_crit = t_quantile((1.0 - level / 100.0) / 2.0, df_ci);
        let root_mse = stats.mse.sqrt();
        let xrow = |t: usize| -> Option<Vec<f64>> {
            let mut r: Vec<f64> = x_all.iter().map(|c| c[t]).collect::<Option<_>>()?;
            if constant {
                r.push(1.0);
            }
            Some(r)
        };
        let leverage = |r: &[f64]| -> f64 {
            let k = r.len();
            (0..k)
                .map(|i| {
                    (0..k)
                        .map(|j| r[i] * final_fit.inverse[i][j] * r[j])
                        .sum::<f64>()
                })
                .sum()
        };
        // the last known residual of the original equation, and steps since it
        let mut last_u: Option<(f64, usize)> = None;
        for (t, &observed) in y_all.iter().enumerate().skip(lo) {
            let case = periods.first + t;
            if t >= origin && case > periods.end {
                break;
            }
            let forecast = t >= origin || t > hi;
            let Some(r) = xrow(t) else {
                if forecast {
                    last_u = last_u.map(|(u, s)| (u, s + 1));
                }
                continue;
            };
            let yhat: f64 = r.iter().zip(&beta).map(|(a, b)| a * b).sum();
            let h = leverage(&r);
            let (fit, sep, err) = if t == lo {
                let fit = yhat;
                let sep = root_mse * (1.0 / (1.0 - rho * rho) + h).sqrt();
                let err = observed.map(|v| (1.0 - rho * rho).sqrt() * (v - fit));
                (fit, sep, err)
            } else if !forecast {
                let (u_prev, _) = last_u.expect("set at the first case");
                let fit = yhat + rho * u_prev;
                (fit, root_mse * (1.0 + h).sqrt(), observed.map(|v| v - fit))
            } else {
                let Some((u_last, steps)) = last_u else {
                    continue;
                };
                let steps = steps + 1;
                let fit = yhat + rho.powi(steps as i32) * u_last;
                let spread: f64 = (0..steps).map(|j| rho.powi(2 * j as i32)).sum();
                (fit, root_mse * (spread + h).sqrt(), None)
            };
            columns[0][case] = Some(fit);
            columns[1][case] = err;
            columns[2][case] = Some(fit - t_crit * sep);
            columns[3][case] = Some(fit + t_crit * sep);
            columns[4][case] = Some(sep);
            last_u = match (forecast, observed) {
                (false, Some(v)) => Some((v - yhat, 0)),
                (_, _) => last_u.map(|(u, s)| (u, s + 1)),
            };
        }
        let model = format!("AREG {what}");
        let labels = vec![
            format!("Fit of {dependent}, {model}"),
            format!("Error of {dependent}, {model}"),
            format!("{level}% lower limit of {dependent}, {model}"),
            format!("{level}% upper limit of {dependent}, {model}"),
            format!("Standard error of the fit of {dependent}, {model}"),
        ];
        self.save_series(
            "AREG",
            &["FIT", "ERR", "LCL", "UCL", "SEP"],
            vec![(labels, columns)],
        )?;
        self.ts_models.areg = Some(Previous {
            spec: spec.clone(),
            rho,
            coefficients: beta,
        });
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rho_of_residuals_and_statistics_by_hand() {
        let u = [1.0, 2.0, -1.0, 0.5];
        // (2 - 2 - 0.5) / (1 + 4 + 1 + 0.25)
        assert!((next_rho(&u) - (-0.5 / 6.25)).abs() < 1e-12);
        let s = statistics(&u, 0.5, 1, false);
        // e = sqrt(.75), 1.5, -2, 1; MSE = (2.25 + 4 + 1) / (4 - 2 - 1)
        assert!((s.mse - 7.25).abs() < 1e-12);
        assert!((s.se_rho - (0.75f64 / 2.0).sqrt()).abs() < 1e-12);
        let e = [0.75f64.sqrt(), 1.5, -2.0, 1.0];
        let dw = ((e[1] - e[0]).powi(2) + 12.25 + 9.0) / (0.75 + 2.25 + 4.0 + 1.0);
        assert!((s.dw - dw).abs() < 1e-12);
    }

    #[test]
    fn prais_winsten_scales_the_first_case() {
        let y = [2.0, 3.0, 5.0];
        let x = vec![vec![1.0, 2.0, 4.0]];
        let (ys, xs) = transformed(&y, &x, 0.6, true, true);
        assert_eq!(ys.len(), 3);
        assert!((ys[0] - 1.6).abs() < 1e-12 && (xs[0][1] - 0.8).abs() < 1e-12);
        assert!((ys[1] - 1.8).abs() < 1e-12 && (xs[1][0] - 1.4).abs() < 1e-12);
        assert!((xs[1][1] - 0.4).abs() < 1e-12);
        let (ys, _) = transformed(&y, &x, 0.6, false, true);
        assert_eq!(ys.len(), 2);
    }
}
