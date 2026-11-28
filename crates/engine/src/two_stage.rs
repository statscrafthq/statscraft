//! 2SLS: two-stage least squares for one or more structural equations.
//!
//! Rules noted from the Command Syntax Reference and the Algorithms manual:
//! - Each equation y = Z b + e is estimated with the instruments X (every INSTRUMENTS list,
//!   and the constant unless NOCONSTANT): b = (Z'X (X'X)^-1 X'Z)^-1 Z'X (X'X)^-1 X'y. A
//!   predictor that is also an instrument stands for itself; with only predictors as
//!   instruments the estimates are those of ordinary least squares.
//! - The instruments must be at least as many as the predictors of the longest equation.
//! - R square comes from sweeping the correlations of the predictors projected on the
//!   instruments, with the dependent; the residual sum of squares uses the observed
//!   predictors: (y - Z b)'(y - Z b). The rest follows ordinary regression.
//! - Case weights count; a case missing on any variable of the command is left out of every
//!   equation.
//! - TSET NEWVAR saves FIT_n and ERR_n per equation; SAVE PRED and RESID override it and take
//!   free numbers. PRINT=COV (or TSET PRINT=DETAILED) adds the covariance of the estimates.
//!
//! Choices where the manual is silent:
//! - The ANOVA table puts R square times the total sum of squares in the regression row and
//!   the residual sum of squares of the observed predictors in the residual row, so the two
//!   need not add up to the total; F is their ratio of mean squares.
//! - The covariance of the estimates is the residual mean square times
//!   (Z'X (X'X)^-1 X'Z)^-1. Tolerance is that of the projected predictors.
//! - Fitted values come for every case whose predictors are valid; residuals where the
//!   dependent is valid too.

use crate::descriptives::{case_weights, number};
use crate::dist::{f_p, t_p};
use crate::matrix::inverse_scaled;
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::timeseries::read_series;
use crate::{Cell, Engine, Error, Table};
use statscraft_syntax::{NewVar, TsetPrint, TwoStage};

type Matrix = Vec<Vec<f64>>;

/// A' W B over the rows `used`.
fn cross(a: &[Vec<f64>], b: &[Vec<f64>], w: &[f64], used: &[usize]) -> Matrix {
    a.iter()
        .map(|ca| {
            b.iter()
                .map(|cb| used.iter().map(|&k| w[k] * ca[k] * cb[k]).sum())
                .collect()
        })
        .collect()
}

fn product(a: &Matrix, b: &Matrix) -> Matrix {
    let inner = b.len();
    a.iter()
        .map(|row| {
            (0..b.first().map_or(0, Vec::len))
                .map(|j| (0..inner).map(|k| row[k] * b[k][j]).sum())
                .collect()
        })
        .collect()
}

fn transpose(a: &Matrix) -> Matrix {
    (0..a.first().map_or(0, Vec::len))
        .map(|j| a.iter().map(|row| row[j]).collect())
        .collect()
}

/// The estimates of one equation.
pub(crate) struct Estimates {
    pub beta: Vec<f64>,
    /// (Z'X (X'X)^-1 X'Z)^-1.
    pub inverse: Matrix,
    /// The predictors projected on the instruments, per column.
    pub projected: Vec<Vec<f64>>,
}

/// Two-stage least squares of column `y` on the columns `z` with instruments `x`, weights `w`.
/// With a constant column among both the predictors and the instruments the model is the same
/// after centring every other column (and y) on its weighted mean, and the centred cross-products
/// keep their digits where raw ones of columns far from 0 would not; the slopes stay, the
/// constant takes the means back, and so do the predictors' fitted values.
pub(crate) fn estimate(
    y: &[f64],
    z: &[Vec<f64>],
    x: &[Vec<f64>],
    w: &[f64],
    used: &[usize],
) -> Option<Estimates> {
    let Some(&first) = used.first() else {
        return estimate_raw(y, z, x, w, used);
    };
    let constant = |c: &[f64]| used.iter().all(|&k| c[k] == c[first]);
    // a column of zeros is constant too, but is no intercept
    let intercept = |c: &Vec<f64>| constant(c) && c[first] != 0.0;
    let (Some(cz), true) = (z.iter().position(intercept), x.iter().any(intercept)) else {
        return estimate_raw(y, z, x, w, used);
    };
    let total: f64 = used.iter().map(|&k| w[k]).sum();
    let mean = |c: &[f64]| used.iter().map(|&k| w[k] * c[k]).sum::<f64>() / total;
    let centre = |cols: &[Vec<f64>]| -> Vec<Vec<f64>> {
        cols.iter()
            .map(|c| {
                if constant(c) {
                    c.clone()
                } else {
                    let m = mean(c);
                    c.iter().map(|v| v - m).collect()
                }
            })
            .collect()
    };
    let means: Vec<f64> = z
        .iter()
        .map(|c| if constant(c) { 0.0 } else { mean(c) })
        .collect();
    let my = mean(y);
    let centred_y: Vec<f64> = y.iter().map(|v| v - my).collect();
    let mut e = estimate_raw(&centred_y, &centre(z), &centre(x), w, used)?;
    // beta = A beta* + (my / kappa) on the constant: the constant column holds kappa
    let kappa = z[cz][first];
    let q = z.len();
    let mut a: Matrix = (0..q)
        .map(|i| (0..q).map(|j| f64::from(u8::from(i == j))).collect())
        .collect();
    for j in 0..q {
        if j != cz {
            a[cz][j] = -means[j] / kappa;
        }
    }
    let star = e.beta.clone();
    e.beta = (0..q)
        .map(|i| (0..q).map(|j| a[i][j] * star[j]).sum::<f64>())
        .collect();
    e.beta[cz] += my / kappa;
    e.inverse = product(&product(&a, &e.inverse), &transpose(&a));
    for (col, m) in e.projected.iter_mut().zip(&means) {
        for v in col.iter_mut() {
            *v += m;
        }
    }
    Some(e)
}

fn estimate_raw(
    y: &[f64],
    z: &[Vec<f64>],
    x: &[Vec<f64>],
    w: &[f64],
    used: &[usize],
) -> Option<Estimates> {
    let (xx_inv, _) = inverse_scaled(&cross(x, x, w, used))?;
    let zx = cross(z, x, w, used);
    let xy = cross(x, &[y.to_vec()], w, used);
    // the first stage: coefficients of each predictor on the instruments
    let gamma = product(&xx_inv, &transpose(&zx));
    let middle = product(&zx, &gamma);
    let (inv, _) = inverse_scaled(&middle)?;
    let right = product(&product(&zx, &xx_inv), &xy);
    let beta: Vec<f64> = product(&inv, &right).iter().map(|r| r[0]).collect();
    let rows = y.len();
    let projected = (0..z.len())
        .map(|j| {
            (0..rows)
                .map(|k| x.iter().enumerate().map(|(i, c)| c[k] * gamma[i][j]).sum())
                .collect()
        })
        .collect();
    Some(Estimates {
        beta,
        inverse: inv,
        projected,
    })
}

impl Engine {
    /// The command over the previous 2SLS for APPLY.
    pub(crate) fn two_stage_spec(&mut self, command: &TwoStage) -> Result<TwoStage, Error> {
        let mut given = match &command.apply {
            None => command.clone(),
            Some(Some(model)) => {
                return Err(Error(format!(
                    "2SLS APPLY of the named model '{model}' is not supported yet; APPLY \
                     without a name uses the previous 2SLS"
                )));
            }
            Some(None) => {
                let Some(previous) = &self.ts_models.two_stage else {
                    return Err(Error(
                        "2SLS APPLY: there is no earlier 2SLS to apply".into(),
                    ));
                };
                let mut given = (**previous).clone();
                if !command.equations.is_empty() {
                    given.equations.clone_from(&command.equations);
                }
                if command.instruments.is_some() {
                    given.instruments.clone_from(&command.instruments);
                }
                if !command.endogenous.is_empty() {
                    given.endogenous.clone_from(&command.endogenous);
                }
                if command.constant.is_some() {
                    given.constant = command.constant;
                }
                given.print_cov |= command.print_cov;
                given.save_pred = command.save_pred;
                given.save_resid = command.save_resid;
                given
            }
        };
        given.apply = None;
        Ok(given)
    }

    pub(crate) fn two_stage(&mut self, spec: &TwoStage) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let include = self.tset.include;
        let constant = spec.constant.unwrap_or(self.tset.constant);
        let print_cov = spec.print_cov || self.tset.print == TsetPrint::Detailed;
        let mut instruments: Vec<String> = Vec::new();
        for name in self.expand(spec.instruments.as_deref().unwrap_or_default())? {
            if !instruments.contains(&name) {
                instruments.push(name);
            }
        }
        let mut equations = Vec::new();
        for e in &spec.equations {
            let dependent =
                self.expand(&[statscraft_syntax::VarItem::Name(e.dependent.clone())])?;
            let mut predictors = Vec::new();
            for name in self.expand(&e.predictors)? {
                if !predictors.contains(&name) {
                    predictors.push(name);
                }
            }
            if predictors.contains(&dependent[0]) {
                return Err(Error(format!(
                    "2SLS: {} is both the dependent variable and a predictor",
                    dependent[0]
                )));
            }
            equations.push((dependent[0].clone(), predictors));
        }
        let longest = equations.iter().map(|(_, p)| p.len()).max().unwrap_or(0);
        if instruments.len() < longest {
            return Err(Error(format!(
                "2SLS: the {} instruments are fewer than the {longest} predictors of the longest \
                 equation",
                instruments.len()
            )));
        }
        // every variable of the command, for listwise deletion
        let mut all: Vec<String> = instruments.clone();
        for (y, z) in &equations {
            for name in std::iter::once(y).chain(z) {
                if !all.contains(name) {
                    all.push(name.clone());
                }
            }
        }
        let cases = data.case_count();
        let mut columns = std::collections::HashMap::new();
        for name in &all {
            let (values, _, _) = read_series(data, name, include, "2SLS")?;
            columns.insert(name.clone(), values);
        }
        let weights = case_weights(data)?.unwrap_or_else(|| vec![1.0; cases]);
        let used: Vec<usize> = (0..cases)
            .filter(|&k| weights[k] > 0.0 && all.iter().all(|n| columns[n][k].is_some()))
            .collect();
        let n: f64 = used.iter().map(|&k| weights[k]).sum();
        let filled = |name: &str| -> Vec<f64> {
            columns[name]
                .iter()
                .map(|v| v.unwrap_or(f64::NAN))
                .collect()
        };
        let mut x: Vec<Vec<f64>> = instruments.iter().map(|n| filled(n)).collect();
        if constant {
            x.push(vec![1.0; cases]);
        }
        let label = |name: &str| shown.named(data, name);
        let mut tables = vec![table(
            "Model Description",
            1,
            plain(&["", "Specification"]),
            {
                let mut rows: Vec<Vec<Cell>> = equations
                    .iter()
                    .enumerate()
                    .map(|(i, (y, z))| {
                        vec![
                            text(format!("Equation {}", i + 1)),
                            text(format!(
                                "{} with {}",
                                label(y),
                                z.iter().map(|v| label(v)).collect::<Vec<_>>().join(", ")
                            )),
                        ]
                    })
                    .collect();
                rows.push(vec![
                    text("Instruments"),
                    text(
                        instruments
                            .iter()
                            .map(|v| label(v))
                            .chain(constant.then(|| "Constant".to_string()))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                ]);
                let endogenous: Vec<String> = equations
                    .iter()
                    .flat_map(|(y, z)| std::iter::once(y).chain(z))
                    .filter(|v| !instruments.contains(v))
                    .fold(Vec::new(), |mut acc, v| {
                        if !acc.contains(v) {
                            acc.push(v.clone());
                        }
                        acc
                    });
                rows.push(vec![
                    text("Endogenous"),
                    text(
                        endogenous
                            .iter()
                            .map(|v| label(v))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                ]);
                rows.push(vec![text("Cases used"), number(Some(n))]);
                rows
            },
            Some("Variables not named as instruments are endogenous.".into()),
        )];
        let mut saves = Vec::new();
        let mut skipped = Vec::new();
        for (index, (dependent, predictors)) in equations.iter().enumerate() {
            let y = filled(dependent);
            // as in REGRESSION: a dependent without variance gets no equation
            if !used.is_empty() && used.iter().all(|&c| y[c] == y[used[0]]) {
                skipped.push(format!(
                    "2SLS: {dependent} has no variance in the cases of the analysis; equation {} is left out",
                    index + 1
                ));
                continue;
            }
            let mut z: Vec<Vec<f64>> = predictors.iter().map(|n| filled(n)).collect();
            if constant {
                z.push(vec![1.0; cases]);
            }
            let p = predictors.len();
            let k = z.len();
            let df_res = n - k as f64;
            if df_res <= 0.0 {
                return Err(Error(format!(
                    "2SLS: {} cases are too few for the {k} coefficients of equation {}",
                    n + 0.0,
                    index + 1
                )));
            }
            let est = estimate(&y, &z, &x, &weights, &used).ok_or_else(|| {
                Error(format!(
                    "2SLS: equation {} cannot be estimated: the instruments or the predictors \
                     projected on them are linearly dependent",
                    index + 1
                ))
            })?;
            let fit_of = |cols: &[Vec<f64>], k_: usize| -> f64 {
                cols.iter().zip(&est.beta).map(|(c, b)| c[k_] * b).sum()
            };
            let wsum =
                |f: &dyn Fn(usize) -> f64| used.iter().map(|&c| weights[c] * f(c)).sum::<f64>();
            let mean = |v: &[f64]| wsum(&|c| v[c]) / n;
            let y_mean = mean(&y);
            let center = if constant { y_mean } else { 0.0 };
            let sst = wsum(&|c| (y[c] - center).powi(2));
            let sse = wsum(&|c| (y[c] - fit_of(&z, c)).powi(2));
            let mean_residual = wsum(&|c| y[c] - fit_of(&z, c)) / n;
            // the second stage: the projected predictors' fit about the same center
            let ssr = wsum(&|c| (fit_of(&est.projected, c) - center).powi(2));
            let r2 = if sst > 0.0 { ssr / sst } else { f64::NAN };
            let mse = sse / df_res;
            let df_total = if constant { n - 1.0 } else { n };
            let adjusted = 1.0 - (1.0 - r2) * df_total / df_res;
            let title = format!(
                "Equation {}: {} with {}",
                index + 1,
                label(dependent),
                predictors
                    .iter()
                    .map(|v| label(v))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            tables.push(table(
                "Model Summary",
                1,
                plain(&["", "Value"]),
                vec![
                    vec![text("Multiple R"), number(Some(r2.sqrt()))],
                    vec![text("R Square"), number(Some(r2))],
                    vec![text("Adjusted R Square"), number(Some(adjusted))],
                    vec![text("Std. Error of the Estimate"), number(Some(mse.sqrt()))],
                    vec![text("Mean of the Residuals"), number(Some(mean_residual))],
                ],
                Some(format!("{title}.")),
            ));
            let reg = r2 * sst;
            let f = (reg / p as f64) / mse;
            tables.push(table(
                "ANOVA",
                1,
                plain(&["", "Sum of Squares", "df", "Mean Square", "F", "Sig."]),
                vec![
                    vec![
                        text("Regression"),
                        number(Some(reg)),
                        Cell::Number(p as f64),
                        number(Some(reg / p as f64)),
                        number(Some(f)),
                        number(Some(f_p(f, p as f64, df_res))),
                    ],
                    vec![
                        text("Residual"),
                        number(Some(sse)),
                        Cell::Number(df_res),
                        number(Some(mse)),
                        Cell::Empty,
                        Cell::Empty,
                    ],
                ],
                Some(format!(
                    "{title}. The residuals use the observed predictors, the regression sum of \
                     squares the predicted ones, so the two need not add up to the total."
                )),
            ));
            // tolerance of each projected predictor among the others
            let projected_corr = {
                let sd: Vec<f64> = (0..p)
                    .map(|j| {
                        let m = if constant {
                            mean(&est.projected[j])
                        } else {
                            0.0
                        };
                        wsum(&|c| (est.projected[j][c] - m).powi(2)).sqrt()
                    })
                    .collect();
                let cov: Matrix = (0..p)
                    .map(|i| {
                        (0..p)
                            .map(|j| {
                                let (mi, mj) = if constant {
                                    (mean(&est.projected[i]), mean(&est.projected[j]))
                                } else {
                                    (0.0, 0.0)
                                };
                                wsum(&|c| (est.projected[i][c] - mi) * (est.projected[j][c] - mj))
                                    / (sd[i] * sd[j])
                            })
                            .collect()
                    })
                    .collect();
                inverse_scaled(&cov).map(|(inv, _)| inv)
            };
            let sd_y = wsum(&|c| (y[c] - center).powi(2)).sqrt();
            let rows = (0..k)
                .map(|j| {
                    let name = if j < p {
                        label(&predictors[j])
                    } else {
                        "Constant".to_string()
                    };
                    let se = (mse * est.inverse[j][j]).sqrt();
                    let t = est.beta[j] / se;
                    let beta_std = (j < p).then(|| {
                        let m = if constant { mean(&z[j]) } else { 0.0 };
                        est.beta[j] * wsum(&|c| (z[j][c] - m).powi(2)).sqrt() / sd_y
                    });
                    let tolerance = (j < p)
                        .then(|| projected_corr.as_ref().map(|inv| 1.0 / inv[j][j]))
                        .flatten();
                    vec![
                        text(name),
                        number(Some(est.beta[j])),
                        number(Some(se)),
                        number(beta_std),
                        number(Some(t)),
                        number(Some(t_p(t, df_res))),
                        number(tolerance),
                    ]
                })
                .collect();
            tables.push(table(
                "Coefficients",
                1,
                plain(&["", "B", "Std. Error", "Beta", "t", "Sig.", "Tolerance"]),
                rows,
                Some(format!("{title}.")),
            ));
            let names: Vec<String> = (0..k)
                .map(|j| {
                    if j < p {
                        label(&predictors[j])
                    } else {
                        "Constant".to_string()
                    }
                })
                .collect();
            let mut header = vec![""];
            header.extend(names.iter().map(String::as_str));
            let matrix = |f: &dyn Fn(usize, usize) -> f64| -> Vec<Vec<Cell>> {
                (0..k)
                    .map(|i| {
                        let mut row = vec![text(names[i].clone())];
                        row.extend((0..k).map(|j| number(Some(f(i, j)))));
                        row
                    })
                    .collect()
            };
            let cov = |i: usize, j: usize| mse * est.inverse[i][j];
            tables.push(table(
                "Correlation Matrix of the Estimates",
                1,
                plain(&header),
                matrix(&|i, j| cov(i, j) / (cov(i, i) * cov(j, j)).sqrt()),
                Some(format!("{title}.")),
            ));
            if print_cov {
                tables.push(table(
                    "Covariance Matrix of the Estimates",
                    1,
                    plain(&header),
                    matrix(&cov),
                    Some(format!("{title}.")),
                ));
            }
            // fits wherever the predictors are valid
            let fits: Vec<Option<f64>> = (0..cases)
                .map(|c| {
                    let v = fit_of(&z, c);
                    v.is_finite().then_some(v)
                })
                .collect();
            let errors: Vec<Option<f64>> = (0..cases)
                .map(|c| Some(y[c] - fits[c]?).filter(|v| v.is_finite()))
                .collect();
            saves.push((
                vec![
                    format!("Fit of {dependent} from 2SLS equation {}", index + 1),
                    format!("Error of {dependent} from 2SLS equation {}", index + 1),
                ],
                fits,
                errors,
            ));
        }
        for warning in skipped {
            self.warn(warning);
        }
        // SAVE overrides TSET NEWVAR and takes free numbers
        let (roots, newvar): (Vec<&str>, NewVar) = match (spec.save_pred, spec.save_resid) {
            (false, false) => (vec!["FIT", "ERR"], self.tset.newvar),
            (pred, resid) => (
                [("FIT", pred), ("ERR", resid)]
                    .iter()
                    .filter(|(_, w)| *w)
                    .map(|(r, _)| *r)
                    .collect(),
                NewVar::All,
            ),
        };
        let items = saves
            .into_iter()
            .map(|(labels, fits, errors)| {
                let pairs = [(labels[0].clone(), fits), (labels[1].clone(), errors)];
                pairs
                    .into_iter()
                    .zip(["FIT", "ERR"])
                    .filter(|(_, root)| roots.contains(root))
                    .map(|(pair, _)| pair)
                    .unzip()
            })
            .collect();
        self.save_series_as("2SLS", &roots, items, newvar)?;
        // a model APPLY can reuse, kept only when the run succeeds
        self.ts_models.two_stage = Some(Box::new(spec.clone()));
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn just_identified_equals_the_instrumental_variable_ratio() {
        // y on z with one instrument x and a constant: b = cov(x, y) / cov(x, z)
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let z = vec![2.0, 1.0, 4.0, 3.0, 6.0];
        let y = vec![3.0, 2.5, 6.0, 4.0, 9.0];
        let w = vec![1.0; 5];
        let used: Vec<usize> = (0..5).collect();
        let one = vec![1.0; 5];
        let e = estimate(&y, &[z.clone(), one.clone()], &[x.clone(), one], &w, &used).unwrap();
        let mean = |v: &[f64]| v.iter().sum::<f64>() / 5.0;
        let cov = |a: &[f64], b: &[f64]| {
            let (ma, mb) = (mean(a), mean(b));
            a.iter()
                .zip(b)
                .map(|(p, q)| (p - ma) * (q - mb))
                .sum::<f64>()
        };
        let b = cov(&x, &y) / cov(&x, &z);
        assert!((e.beta[0] - b).abs() < 1e-12);
        assert!((e.beta[1] - (mean(&y) - b * mean(&z))).abs() < 1e-12);
    }
}
