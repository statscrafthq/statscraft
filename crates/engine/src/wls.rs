//! WLS: weighted least squares with weights from a power of a source variable or from a
//! weight variable; and the least-squares fit CURVEFIT shares.
//!
//! Rules noted from the Command Syntax Reference and the Algorithms manual:
//! - A case's weight is the source value to the power minus delta; delta defaults to 1 and the
//!   source to the first independent, with a warning when neither SOURCE, DELTA nor WEIGHT is
//!   given. Powers outside -6.5 to 7.5 move to the nearer end.
//! - The fit is ordinary least squares with weights c g (c the case weight, g the regression
//!   weight); degrees of freedom count the case weights.
//! - Log-likelihood: 0.5 (-n ln 2 pi - n ln s^2 + sum c ln g - (n - p)), s^2 the mean square
//!   error and p the number of coefficients. The power with the largest one is reported in
//!   full (the first on a tie) and its weights saved.
//! - Listwise deletion over every named variable; user-missing values are missing.

use crate::descriptives::{case_weights, number};
use crate::dist::{f_p, t_p};
use crate::matrix::{Matrix, inverse};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure};
use statscraft_syntax::Wls;

/// A column of numbers, NaN where the value is missing.
pub(crate) fn numeric_column(
    data: &Dataset,
    name: &str,
    include: bool,
    command: &str,
) -> Result<Vec<f64>, Error> {
    let v = data.variable(name)?;
    if v.is_string() {
        return Err(Error(format!(
            "{command} needs numeric variables, but {name} is a string variable"
        )));
    }
    let values = v.numeric()?;
    Ok((0..data.case_count())
        .map(|k| {
            values
                .get(k)
                .filter(|x| include || !v.is_user_missing(*x))
                .unwrap_or(f64::NAN)
        })
        .collect())
}

/// A weighted least-squares fit, computed on the predictors centered (with a constant) and
/// scaled to unit sums of squares, which keeps powers of a long series well conditioned.
pub(crate) struct LeastSquares {
    pub constant: bool,
    /// The weighted means of the predictors (0 without a constant).
    means: Vec<f64>,
    /// Root weighted sums of squares about the means.
    scales: Vec<f64>,
    scale_y: f64,
    /// The inverse of the scaled cross-product matrix of the predictors.
    rinv: Matrix,
    /// The slopes, then the constant.
    pub slopes: Vec<f64>,
    pub intercept: f64,
    /// The sum of the weights c g, and of the case weights c.
    sum_w: f64,
    pub n: f64,
    pub sse: f64,
    pub sst: f64,
    /// The smallest tolerance of a predictor.
    pub tolerance: f64,
}

impl LeastSquares {
    pub fn df_reg(&self) -> f64 {
        self.slopes.len() as f64
    }
    pub fn df_res(&self) -> f64 {
        self.n - self.slopes.len() as f64 - f64::from(u8::from(self.constant))
    }
    pub fn mse(&self) -> f64 {
        self.sse / self.df_res()
    }
    pub fn r_square(&self) -> f64 {
        1.0 - self.sse / self.sst
    }
    pub fn f(&self) -> (f64, f64) {
        let f = (self.sst - self.sse) / self.df_reg() / self.mse();
        (f, f_p(f, self.df_reg(), self.df_res()))
    }
    /// The covariance of slopes a and b over the mean square error.
    pub(crate) fn slope_cov(&self, a: usize, b: usize) -> f64 {
        self.rinv[a][b] / (self.scales[a] * self.scales[b])
    }
    pub fn slope_se(&self, a: usize) -> f64 {
        (self.mse() * self.slope_cov(a, a)).sqrt()
    }
    pub fn intercept_se(&self) -> f64 {
        let k = self.slopes.len();
        let quad: f64 = (0..k)
            .flat_map(|a| (0..k).map(move |b| (a, b)))
            .map(|(a, b)| self.means[a] * self.means[b] * self.slope_cov(a, b))
            .sum();
        (self.mse() * (1.0 / self.sum_w + quad)).sqrt()
    }
    /// The standardized coefficient of slope a and its standard error.
    pub fn beta(&self, a: usize) -> (f64, f64) {
        let ratio = self.scales[a] / self.scale_y;
        (self.slopes[a] * ratio, self.slope_se(a) * ratio)
    }
    /// The prediction at predictor values `x` and its leverage x'(X'WX)^-1 x.
    pub fn predict(&self, x: &[f64]) -> (f64, f64) {
        let k = self.slopes.len();
        let fit = self.intercept + (0..k).map(|a| self.slopes[a] * x[a]).sum::<f64>();
        let d: Vec<f64> = (0..k).map(|a| x[a] - self.means[a]).collect();
        let quad: f64 = (0..k)
            .flat_map(|a| (0..k).map(move |b| (a, b)))
            .map(|(a, b)| d[a] * d[b] * self.slope_cov(a, b))
            .sum();
        let base = if self.constant { 1.0 / self.sum_w } else { 0.0 };
        (fit, base + quad)
    }
}

/// Fits y on the rows of `x` with weights `c g` (`c` the case weights). None when a predictor
/// does not vary or the predictors are linearly dependent.
pub(crate) fn least_squares(
    x: &[Vec<f64>],
    y: &[f64],
    c: &[f64],
    g: &[f64],
    constant: bool,
) -> Option<LeastSquares> {
    let k = x.first().map_or(0, Vec::len);
    let w: Vec<f64> = c.iter().zip(g).map(|(c, g)| c * g).collect();
    let sum_w: f64 = w.iter().sum();
    let n: f64 = c.iter().sum();
    let mean = |f: &dyn Fn(usize) -> f64| match constant {
        true => (0..y.len()).map(|i| w[i] * f(i)).sum::<f64>() / sum_w,
        false => 0.0,
    };
    let means: Vec<f64> = (0..k).map(|a| mean(&|i| x[i][a])).collect();
    let mean_y = mean(&|i| y[i]);
    let ss = |f: &dyn Fn(usize) -> f64| (0..y.len()).map(|i| w[i] * f(i)).sum::<f64>();
    let scales: Vec<f64> = (0..k)
        .map(|a| ss(&|i| (x[i][a] - means[a]).powi(2)).sqrt())
        .collect();
    let sst = ss(&|i| (y[i] - mean_y).powi(2));
    if scales.iter().any(|s| *s <= 0.0 || !s.is_finite()) || sst <= 0.0 {
        return None;
    }
    let z = |i: usize, a: usize| (x[i][a] - means[a]) / scales[a];
    let r: Matrix = (0..k)
        .map(|a| (0..k).map(|b| ss(&|i| z(i, a) * z(i, b))).collect())
        .collect();
    let ry: Vec<f64> = (0..k).map(|a| ss(&|i| z(i, a) * (y[i] - mean_y))).collect();
    let (rinv, _) = inverse(&r)?;
    let tolerance = (0..k).map(|a| 1.0 / rinv[a][a]).fold(1.0, f64::min);
    let slopes: Vec<f64> = (0..k)
        .map(|a| (0..k).map(|b| rinv[a][b] * ry[b]).sum::<f64>() / scales[a])
        .collect();
    let intercept = mean_y - (0..k).map(|a| slopes[a] * means[a]).sum::<f64>();
    let sse = ss(&|i| {
        let fit = intercept + (0..k).map(|a| slopes[a] * x[i][a]).sum::<f64>();
        (y[i] - fit).powi(2)
    });
    Some(LeastSquares {
        constant,
        means,
        scales,
        scale_y: sst.sqrt(),
        rinv,
        slopes,
        intercept,
        sum_w,
        n,
        sse,
        sst,
        tolerance,
    })
}

/// The analysis of variance of a fit.
pub(crate) fn anova_table(ls: &LeastSquares, caption: Option<String>) -> Table {
    let (f, sig) = ls.f();
    let ssr = ls.sst - ls.sse;
    let rows = vec![
        vec![
            text("Regression"),
            Cell::Number(ssr),
            Cell::Number(ls.df_reg()),
            number(Some(ssr / ls.df_reg())),
            number(Some(f)),
            number(Some(sig)),
        ],
        vec![
            text("Residual"),
            Cell::Number(ls.sse),
            Cell::Number(ls.df_res()),
            number(Some(ls.mse())),
            Cell::Empty,
            Cell::Empty,
        ],
        vec![
            text("Total"),
            Cell::Number(ls.sst),
            Cell::Number(ls.df_reg() + ls.df_res()),
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
        ],
    ];
    let columns = plain(&["", "Sum of Squares", "df", "Mean Square", "F", "Sig."]);
    table("ANOVA", 1, columns, rows, caption)
}

/// R, R square, adjusted R square and the standard error of a fit, plus `extra` rows.
pub(crate) fn summary_table(
    ls: &LeastSquares,
    extra: Vec<(&str, f64)>,
    caption: Option<String>,
) -> Table {
    let r2 = ls.r_square();
    let adjusted = 1.0 - (1.0 - r2) * (ls.df_reg() + ls.df_res()) / ls.df_res();
    let mut stats = vec![
        ("Multiple R", r2.sqrt()),
        ("R Square", r2),
        ("Adjusted R Square", adjusted),
        ("Std. Error of the Estimate", ls.mse().sqrt()),
    ];
    stats.extend(extra);
    let rows = stats
        .into_iter()
        .map(|(name, v)| vec![text(name), number(Some(v))])
        .collect();
    table("Model Summary", 1, plain(&["", "Value"]), rows, caption)
}

/// B, its standard error, Beta with its standard error, t and significance for every
/// predictor, then the constant.
fn coefficients_table(ls: &LeastSquares, names: &[String], caption: Option<String>) -> Table {
    let df = ls.df_res();
    let mut rows: Vec<Vec<Cell>> = names
        .iter()
        .enumerate()
        .map(|(a, name)| {
            let (b, se) = (ls.slopes[a], ls.slope_se(a));
            let (beta, beta_se) = ls.beta(a);
            vec![
                text(name.clone()),
                Cell::Number(b),
                number(Some(se)),
                number(Some(beta)),
                number(Some(beta_se)),
                number(Some(b / se)),
                number(Some(t_p(b / se, df))),
            ]
        })
        .collect();
    if ls.constant {
        let se = ls.intercept_se();
        let t = ls.intercept / se;
        rows.push(vec![
            text("(Constant)"),
            Cell::Number(ls.intercept),
            number(Some(se)),
            Cell::Empty,
            Cell::Empty,
            number(Some(t)),
            number(Some(t_p(t, df))),
        ]);
    }
    let columns = vec![
        (String::new(), String::new()),
        ("Unstandardized Coefficients".into(), "B".into()),
        ("Unstandardized Coefficients".into(), "Std. Error".into()),
        ("Standardized Coefficients".into(), "Beta".into()),
        ("Standardized Coefficients".into(), "Std. Error".into()),
        (String::new(), "t".into()),
        (String::new(), "Sig.".into()),
    ];
    table("Coefficients", 1, columns, rows, caption)
}

/// The log-likelihood of a weighted fit, with `g` the regression weights.
fn log_likelihood(ls: &LeastSquares, c: &[f64], g: &[f64]) -> f64 {
    let p = ls.slopes.len() as f64 + f64::from(u8::from(ls.constant));
    let log_weights: f64 = c.iter().zip(g).map(|(c, g)| c * g.ln()).sum();
    0.5 * (-ls.n * (2.0 * std::f64::consts::PI).ln() - ls.n * ls.mse().ln() + log_weights
        - (ls.n - p))
}

/// The powers WLS takes, at most this far from 0.
const POWER_RANGE: (f64, f64) = (-6.5, 7.5);

impl Engine {
    pub(crate) fn wls(&mut self, command: &Wls) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let independents = self.expand(&command.independents)?;
        let constant = command.constant.unwrap_or(self.tset.constant);
        let print_all = command
            .print_all
            .unwrap_or(self.tset.print == statscraft_syntax::TsetPrint::Detailed);
        // TSET PRINT=BRIEF: only the statistics of the best power
        let brief =
            command.print_all.is_none() && self.tset.print == statscraft_syntax::TsetPrint::Brief;
        // without SAVE, TSET NEWVAR decides: WGT_1 (CURRENT), the next free WGT_n (ALL) or none
        let newvar = self.tset.newvar;
        let save_weight = command.save_weight || newvar != statscraft_syntax::NewVar::None;
        let numbered_from_one =
            !command.save_weight && newvar == statscraft_syntax::NewVar::Current;
        let mut warnings = Vec::new();
        let source = match (&command.weight, &command.source) {
            (Some(_), _) => None,
            (None, Some(source)) => Some(source.clone()),
            (None, None) => {
                if command.deltas.is_empty() {
                    warnings.push(format!(
                        "WLS has neither SOURCE, DELTA nor WEIGHT: the weights are {} to the power -1",
                        independents[0]
                    ));
                }
                Some(independents[0].clone())
            }
        };
        let mut powers = if command.deltas.is_empty() {
            vec![1.0]
        } else {
            command.deltas.clone()
        };
        for power in &mut powers {
            let kept = power.clamp(POWER_RANGE.0, POWER_RANGE.1);
            if kept != *power {
                warnings.push(format!(
                    "the power {power} is outside -6.5 to 7.5; {kept} is used instead"
                ));
                *power = kept;
            }
        }
        let y = numeric_column(data, &command.dependent, false, "WLS")?;
        let xs: Vec<Vec<f64>> = independents
            .iter()
            .map(|n| numeric_column(data, n, false, "WLS"))
            .collect::<Result<_, _>>()?;
        let weight_name = command
            .weight
            .as_ref()
            .or(source.as_ref())
            .expect("one is set");
        let weight_values = numeric_column(data, weight_name, false, "WLS")?;
        let case_w = case_weights(data)?;
        let mut cases = Vec::new();
        let mut not_positive = 0usize;
        for k in 0..data.case_count() {
            let c = case_w.as_ref().map_or(1.0, |w| w[k]);
            let values = std::iter::once(y[k]).chain(xs.iter().map(|x| x[k]));
            if c <= 0.0 || weight_values[k].is_nan() || values.clone().any(f64::is_nan) {
                continue;
            }
            if weight_values[k] <= 0.0 {
                not_positive += 1;
                continue;
            }
            cases.push(k);
        }
        if not_positive > 0 {
            warnings.push(format!(
                "{not_positive} case(s) with {weight_name} not above 0 are left out: they give no weight"
            ));
        }
        let x: Vec<Vec<f64>> = cases
            .iter()
            .map(|&k| xs.iter().map(|x| x[k]).collect())
            .collect();
        let yv: Vec<f64> = cases.iter().map(|&k| y[k]).collect();
        let c: Vec<f64> = cases
            .iter()
            .map(|&k| case_w.as_ref().map_or(1.0, |w| w[k]))
            .collect();
        let base: Vec<f64> = cases.iter().map(|&k| weight_values[k]).collect();
        let parameters = xs.len() + usize::from(constant);
        if c.iter().sum::<f64>() <= parameters as f64 {
            return Err(Error(format!(
                "WLS needs more cases than its {parameters} coefficients"
            )));
        }
        let weights_for = |power: f64| -> Vec<f64> {
            match &command.weight {
                Some(_) => base.clone(),
                None => base.iter().map(|s| s.powf(-power)).collect(),
            }
        };
        let fit = |power: f64| -> Result<LeastSquares, Error> {
            least_squares(&x, &yv, &c, &weights_for(power), constant).ok_or_else(|| {
                Error(
                    "WLS cannot fit the model: a variable is constant or the independents are linearly dependent"
                        .into(),
                )
            })
        };
        let names: Vec<String> = independents.iter().map(|n| shown.named(data, n)).collect();
        let dependent = shown.named(data, &command.dependent);
        let mut tables = Vec::new();
        let (best, ls) = if command.weight.is_some() {
            (None, fit(1.0)?)
        } else {
            let mut values: Vec<(f64, LeastSquares, f64)> = Vec::new();
            for &power in &powers {
                let ls = fit(power)?;
                let ll = log_likelihood(&ls, &c, &weights_for(power));
                values.push((power, ls, ll));
            }
            let best =
                (0..values.len()).fold(0, |b, i| if values[i].2 > values[b].2 { i } else { b });
            let rows = values
                .iter()
                .map(|(power, _, ll)| vec![Cell::Number(*power), number(Some(*ll))])
                .collect();
            let source_text = shown.named(data, weight_name);
            if !brief {
                tables.push(table(
                    "Log-Likelihood Values",
                    1,
                    plain(&["Power", "Log-likelihood"]),
                    rows,
                    Some(format!(
                        "Source variable: {source_text}. The power {} gives the largest log-likelihood; the tables below use it.",
                        values[best].0
                    )),
                ));
            }
            if print_all {
                for (power, ls, _) in &values {
                    tables.push(coefficients_table(
                        ls,
                        &names,
                        Some(format!("Dependent variable: {dependent}. Power {power}.")),
                    ));
                }
            }
            let (power, ls, _) = values.swap_remove(best);
            (Some(power), ls)
        };
        let g = weights_for(best.unwrap_or(1.0));
        let ll = log_likelihood(&ls, &c, &g);
        let weighting = match best {
            Some(power) => format!("{} to the power {}", shown.named(data, weight_name), -power),
            None => shown.named(data, weight_name),
        };
        let caption = Some(format!(
            "Dependent variable: {dependent}. Weights: {weighting}."
        ));
        tables.push(summary_table(
            &ls,
            vec![("Log-likelihood Function Value", ll)],
            caption.clone(),
        ));
        tables.push(anova_table(&ls, caption.clone()));
        tables.push(coefficients_table(&ls, &names, caption));
        for warning in warnings {
            self.warn(warning);
        }
        if save_weight {
            match best {
                None if command.save_weight => {
                    self.warn("WLS: SAVE=WEIGHT saves nothing when WEIGHT gives the weights")
                }
                None => {}
                Some(power) => {
                    let source = weight_name.clone();
                    let dependent = command.dependent.clone();
                    self.all_or_nothing(|engine| {
                        let data = engine.active_mut()?;
                        let values: Vec<Option<f64>> = weight_values
                            .iter()
                            .map(|s| (*s > 0.0).then(|| s.powf(-power)))
                            .collect();
                        let name = match numbered_from_one {
                            true => "WGT_1".to_string(),
                            false => (1..)
                                .map(|n| format!("WGT_{n}"))
                                .find(|name| data.variable_index(name).is_none())
                                .expect("a free name"),
                        };
                        data.set_numeric_values(&name, values)?;
                        data.set_variable_label(
                            &name,
                            Some(&format!(
                                "WLS weight for {dependent}: {source} to the power {}",
                                -power
                            )),
                        )?;
                        data.set_format(&name, "F11.5")?;
                        data.set_measure(&name, Measure::Scale)?;
                        Ok(())
                    })?;
                }
            }
        }
        Ok(tables)
    }
}
