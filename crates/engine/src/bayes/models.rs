//! BAYES ANOVA, BAYES REGRESSION and BAYES REPEATED: Bayes factors and posteriors of linear
//! models, and the charts of prior, likelihood and posterior densities.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - Each case has the rounded frequency weight f (dropped below 0.5) times the regression
//!   weight q of REGWGT (dropped at 0 or below, or missing). N is the sum of f.
//! - The Bayes factor BF10 of a model with p regressors against the model with the intercept
//!   only depends on N, p and the weighted R square. With a fixed g (ZELLNER) it is
//!   (1 + g)^((N-p-1)/2) (1 + g (1 - R2))^(-(N-1)/2). JZS integrates that over g with an inverse
//!   gamma (1/2, N/2) prior, ROUDER(s) with an inverse gamma (1/2, N s^2/2) prior, and HYPER(a)
//!   with the density (a - 2)/2 (1 + g)^(-a/2). BAYES ANOVA is the model of the factor's
//!   groups (p = groups - 1).
//! - Posteriors: the reference prior 1/variance gives a t posterior of the coefficients with
//!   N - P df (P coefficients) around the least squares estimates, and an inverse gamma
//!   ((N - P)/2, SSE/2) posterior of the error variance. The conjugate prior (inverse gamma
//!   (a0, b0) on the variance, normal (m0, variance V0) on the coefficients) updates to
//!   V1 = (V0^-1 + A'WA)^-1, m1 = V1 (V0^-1 m0 + A'Wy), a1 = a0 + N/2 and
//!   b1 = b0 + (m0'V0^-1 m0 + y'Wy - m1'V1^-1 m1)/2; the coefficients are then t with 2 a1 df,
//!   center m1 and scale b1/a1 V1. BAYES ANOVA uses one mean per group as its coefficients.
//! - A prediction at the regressors x is t with the same df, center x m1 and scale
//!   b1/a1 (1 + x V1 x'). The Bayesian F test of k coefficients L theta = c is
//!   (L m1 - c)'(L V1 L')^-1 (L m1 - c) / (2 b1) times 2 a1 / k, with k and 2 a1 df.
//! - Factors of BAYES REGRESSION get one indicator per category in ascending order; the last
//!   one is redundant once the intercept is in, and redundant coefficients are set to 0.
//! - Repeated measures: BIC approximation of Nathoo and Masson from the total, between-subject
//!   and condition sums of squares, or Rouder's mixed-model factor by Monte Carlo over the two
//!   g of the conditions and the subjects. The posterior of the condition means is the normal
//!   large-sample approximation.
//!
//! Choices where the manuals are silent or inconsistent are listed in docs/differences.md:
//! COMPARE=FULL takes the ratio of the factors against the null model (the manual's own
//! formulas do not converge for JZS); Rouder's factor uses the closed form of its integrand.

use super::*;
use crate::chart::{Axis, Chart, ChartKind};
use crate::crosstabs::categories;
use crate::dist::f_p;
use crate::matrix::{Matrix, cholesky, inverse, sweep, symmetric_eigen};
use crate::plot::{Plot, Point, PointGroup};
use statscraft_data::Measure;
use statscraft_syntax::{
    BayesAnova, BayesRegression, BayesRepeated, GPrior, LinearPrior, PredictStat, VarItem,
};
use std::cmp::Ordering;

/// Tables, warnings, charts.
pub(super) type Output = (Vec<Table>, Vec<String>, Vec<Chart>);

/// New variables of SAVE: name, label and the value of every case.
pub(super) type Saved = Vec<(String, String, Vec<Option<f64>>)>;

// ---------------------------------------------------------------------------------------------
// The linear model

/// One case: regressors, response, weight q f and frequency f.
struct Row {
    x: Vec<f64>,
    y: f64,
    w: f64,
    f: f64,
}

/// The weighted least squares fit on the columns that are not redundant.
struct Fit {
    /// The columns kept, in order.
    kept: Vec<usize>,
    xtx: Matrix,
    xty: Vec<f64>,
    yty: f64,
    /// N, the sum of the frequencies.
    n: f64,
    /// (A'WA)^-1 and the coefficients on the kept columns.
    inv: Matrix,
    coef: Vec<f64>,
    sse: f64,
    /// The weighted sum of squares around the weighted mean.
    sst: f64,
}

impl Fit {
    /// Fits on the columns of `columns` (indices into each row's x).
    fn new(rows: &[Row], columns: &[usize]) -> Option<Fit> {
        let m = columns.len();
        let mut cross = vec![vec![0.0; m]; m];
        let mut xty = vec![0.0; m];
        let (mut yty, mut n, mut sw, mut swy) = (0.0, 0.0, 0.0, 0.0);
        for r in rows {
            for (i, &a) in columns.iter().enumerate() {
                xty[i] += r.w * r.x[a] * r.y;
                for (j, &b) in columns.iter().enumerate() {
                    cross[i][j] += r.w * r.x[a] * r.x[b];
                }
            }
            yty += r.w * r.y * r.y;
            n += r.f;
            sw += r.w;
            swy += r.w * r.y;
        }
        // a column whose residual on the earlier ones is below 1e-10 of its size is redundant
        let mut swept = cross.clone();
        let mut kept_at = Vec::new();
        for i in 0..m {
            if cross[i][i] > 0.0 && swept[i][i] > 1e-10 * cross[i][i] {
                sweep(&mut swept, i);
                kept_at.push(i);
            }
        }
        let xtx: Matrix = kept_at
            .iter()
            .map(|&i| kept_at.iter().map(|&j| cross[i][j]).collect())
            .collect();
        let xty: Vec<f64> = kept_at.iter().map(|&i| xty[i]).collect();
        let (inv, _) = inverse(&xtx)?;
        let coef: Vec<f64> = inv
            .iter()
            .map(|row| row.iter().zip(&xty).map(|(a, b)| a * b).sum())
            .collect();
        let fitted: f64 = coef.iter().zip(&xty).map(|(a, b)| a * b).sum();
        Some(Fit {
            kept: kept_at.iter().map(|&i| columns[i]).collect(),
            xtx,
            xty,
            yty,
            n,
            inv,
            coef,
            sse: (yty - fitted).max(0.0),
            sst: yty - swy * swy / sw,
        })
    }

    fn r_square(&self) -> f64 {
        1.0 - self.sse / self.sst
    }
}

/// ln BF10 of a model with `p` regressors and `r2` against the intercept-only model.
fn ln_bf10(prior: GPrior, n: f64, p: f64, r2: f64, q: Quadrature) -> f64 {
    let rest = (1.0 - r2).ln();
    // ln of the factor at g = e^u
    let at = |u: f64| (n - p - 1.0) / 2.0 * softplus(u) - (n - 1.0) / 2.0 * softplus(u + rest);
    match prior {
        GPrior::Zellner(g) => at(g.ln()),
        GPrior::Jzs | GPrior::Rouder(_) => {
            let s = if let GPrior::Rouder(s) = prior {
                s
            } else {
                1.0
            };
            let b = n * s * s / 2.0;
            // the inverse gamma (1/2, b) density times g, over u = ln g
            let log_f = |u: f64| at(u) + 0.5 * b.ln() - 0.5 * PI.ln() - 0.5 * u - b * (-u).exp();
            log_integral(&log_f, -40.0, 60.0, q.tol, q.limit)
        }
        GPrior::Hyper(a) => {
            let log_f = |u: f64| at(u) - a / 2.0 * softplus(u) + u;
            ((a - 2.0) / 2.0).ln() + log_integral(&log_f, -40.0, 60.0, q.tol, q.limit)
        }
    }
}

fn method_name(prior: GPrior) -> String {
    match prior {
        GPrior::Jzs => "JZS".into(),
        GPrior::Zellner(g) => format!("Zellner, g = {g}"),
        GPrior::Hyper(a) => format!("hyper-g, a = {a}"),
        GPrior::Rouder(s) => format!("Rouder, scale {s}"),
    }
}

/// The normal-gamma posterior: coefficients t with 2a df, center `theta`, scale b/a `v`.
struct Linear {
    theta: Vec<f64>,
    v: Matrix,
    a: f64,
    b: f64,
}

impl Linear {
    fn reference(fit: &Fit) -> Linear {
        Linear {
            theta: fit.coef.clone(),
            v: fit.inv.clone(),
            a: (fit.n - fit.kept.len() as f64) / 2.0,
            b: fit.sse / 2.0,
        }
    }

    /// The conjugate update with prior mean `m0` and matrix `v0` on the kept columns.
    fn conjugate(fit: &Fit, m0: &[f64], v0: &Matrix, a0: f64, b0: f64) -> Option<Linear> {
        let (v0inv, _) = inverse(v0)?;
        let precision: Matrix = v0inv
            .iter()
            .zip(&fit.xtx)
            .map(|(a, b)| a.iter().zip(b).map(|(x, y)| x + y).collect())
            .collect();
        let (v, _) = inverse(&precision)?;
        let prior_part = mat_vec(&v0inv, m0);
        let right: Vec<f64> = prior_part
            .iter()
            .zip(&fit.xty)
            .map(|(a, b)| a + b)
            .collect();
        let theta = mat_vec(&v, &right);
        let b =
            b0 + (dot(m0, &prior_part) + fit.yty - dot(&theta, &mat_vec(&precision, &theta))) / 2.0;
        Some(Linear {
            theta,
            v,
            a: a0 + fit.n / 2.0,
            b,
        })
    }

    fn coefficient(&self, i: usize, alpha: f64) -> Posterior {
        t_posterior(
            2.0 * self.a,
            self.theta[i],
            self.b / self.a * self.v[i][i],
            alpha,
        )
    }

    fn error_variance(&self, alpha: f64) -> Posterior {
        inverse_gamma_posterior(self.a, self.b, alpha)
    }

    /// The center and squared scale of the prediction at `x` (kept columns).
    fn prediction(&self, x: &[f64]) -> (f64, f64) {
        let spread = 1.0 + dot(x, &mat_vec(&self.v, x));
        (dot(x, &self.theta), self.b / self.a * spread)
    }

    /// The Bayesian F test of the kept coefficients `which` at the values `c`: F, df1, df2.
    fn f_test(&self, which: &[usize], c: &[f64]) -> Option<(f64, f64, f64)> {
        let k = which.len();
        let d: Vec<f64> = which
            .iter()
            .zip(c)
            .map(|(&i, c)| self.theta[i] - c)
            .collect();
        let sub: Matrix = which
            .iter()
            .map(|&i| which.iter().map(|&j| self.v[i][j]).collect())
            .collect();
        let (sub_inv, _) = inverse(&sub)?;
        let f = dot(&d, &mat_vec(&sub_inv, &d)) / (2.0 * self.b) * 2.0 * self.a / k as f64;
        Some((f, k as f64, 2.0 * self.a))
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn mat_vec(m: &Matrix, v: &[f64]) -> Vec<f64> {
    m.iter().map(|row| dot(row, v)).collect()
}

fn inverse_gamma_posterior(a: f64, b: f64, alpha: f64) -> Posterior {
    if !(a > 0.0 && b > 0.0) {
        return NONE;
    }
    Posterior {
        mode: b / (a + 1.0),
        mean: if a > 1.0 { b / (a - 1.0) } else { f64::NAN },
        variance: if a > 2.0 {
            b * b / ((a - 1.0).powi(2) * (a - 2.0))
        } else {
            f64::NAN
        },
        lower: 1.0 / idf("IDF.GAMMA", 1.0 - alpha / 2.0, &[a, b]),
        upper: 1.0 / idf("IDF.GAMMA", alpha / 2.0, &[a, b]),
    }
}

/// The prior of the coefficients on all `size` columns: mean, matrix and the inverse gamma
/// parameters. None for the reference prior. A VMATRIX of the wrong size or not positive
/// definite gives the identity with a warning.
fn prior_of(
    run: &mut Run,
    prior: &LinearPrior,
    size: usize,
) -> Result<Option<(Vec<f64>, Matrix, f64, f64)>, Error> {
    if !prior.conjugate {
        return Ok(None);
    }
    if prior.means.len() != size {
        return Err(Error(format!(
            "MEANVECTOR needs {size} values, one per parameter, but has {}",
            prior.means.len()
        )));
    }
    let identity: Matrix = (0..size)
        .map(|i| (0..size).map(|j| f64::from(u8::from(i == j))).collect())
        .collect();
    let v0 = if prior.vmatrix.is_empty() {
        identity
    } else {
        let full: Matrix = (0..prior.vmatrix.len())
            .map(|i| {
                (0..prior.vmatrix.len())
                    .map(|j| prior.vmatrix[i.max(j)][i.min(j)])
                    .collect()
            })
            .collect();
        if full.len() != size || cholesky(&full).is_none() {
            run.warnings.push(format!(
                "VMATRIX must be a positive definite {size} by {size} matrix; the identity is used"
            ));
            identity
        } else {
            full
        }
    };
    let (shape, scale) = (prior.shape.unwrap_or(1.0), prior.scale.unwrap_or(1.0));
    Ok(Some((prior.means.clone(), v0, shape, scale)))
}

/// The posterior on the kept columns, from the reference or the conjugate prior.
fn posterior_of(fit: &Fit, prior: &Option<(Vec<f64>, Matrix, f64, f64)>) -> Option<Linear> {
    match prior {
        None => Some(Linear::reference(fit)),
        Some((m0, v0, a0, b0)) => {
            let m: Vec<f64> = fit.kept.iter().map(|&i| m0[i]).collect();
            let v: Matrix = fit
                .kept
                .iter()
                .map(|&i| fit.kept.iter().map(|&j| v0[i][j]).collect())
                .collect();
            Linear::conjugate(fit, &m, &v, *a0, *b0)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Densities and charts

/// The density of a t with `df`, `center` and squared scale `scale2` at `x`.
fn t_density(x: f64, df: f64, center: f64, scale2: f64) -> f64 {
    (ln_gamma((df + 1.0) / 2.0)
        - ln_gamma(df / 2.0)
        - 0.5 * (df * PI * scale2).ln()
        - (df + 1.0) / 2.0 * ((x - center).powi(2) / (df * scale2)).ln_1p())
    .exp()
}

fn inverse_gamma_density(x: f64, a: f64, b: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    (a * b.ln() - ln_gamma(a) - (a + 1.0) * x.ln() - b / x).exp()
}

/// A density to draw: its name, the function and the range holding nearly all of its mass
/// (None for a prior that only follows the others).
type Curve = (&'static str, Box<dyn Fn(f64) -> f64>, Option<(f64, f64)>);

fn t_curve(
    name: &'static str,
    df: f64,
    center: f64,
    scale2: f64,
    own_range: bool,
) -> Option<Curve> {
    if !(df > 0.0 && scale2 > 0.0 && center.is_finite()) {
        return None;
    }
    let half = idf("IDF.T", 0.999, &[df]) * scale2.sqrt();
    let range = own_range.then_some((center - half, center + half));
    Some((
        name,
        Box::new(move |x| t_density(x, df, center, scale2)),
        range,
    ))
}

fn inverse_gamma_curve(name: &'static str, a: f64, b: f64, own_range: bool) -> Option<Curve> {
    if !(a > 0.0 && b > 0.0) {
        return None;
    }
    let range = own_range.then(|| {
        (
            1.0 / idf("IDF.GAMMA", 0.999, &[a, b]),
            1.0 / idf("IDF.GAMMA", 0.001, &[a, b]),
        )
    });
    Some((
        name,
        Box::new(move |x| inverse_gamma_density(x, a, b)),
        range,
    ))
}

/// A line chart of the densities over the union of their ranges, 201 points each.
fn density_chart(
    title: String,
    x_title: &str,
    curves: Vec<Option<Curve>>,
    footnote: &str,
) -> Option<Chart> {
    let curves: Vec<Curve> = curves.into_iter().flatten().collect();
    let (low, high) = curves
        .iter()
        .filter_map(|c| c.2)
        .filter(|(a, b)| a.is_finite() && b.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), (x, y)| {
            (a.min(x), b.max(y))
        });
    if !(low < high) {
        return None;
    }
    let groups = curves
        .iter()
        .map(|(name, f, _)| PointGroup {
            name: name.to_string(),
            points: (0..=200)
                .map(|i| low + (high - low) * i as f64 / 200.0)
                .map(|x| (x, f(x)))
                .filter(|(_, y)| y.is_finite())
                .map(|(x, y)| Point {
                    x,
                    y,
                    z: None,
                    label: None,
                })
                .collect(),
        })
        .collect();
    let mut chart = Chart::plot(
        ChartKind::Line,
        title,
        Axis::titled(x_title),
        Axis::titled("Density"),
        Plot::Points {
            groups,
            lines: Vec::new(),
            z_axis: None,
        },
    );
    if !footnote.is_empty() {
        chart.footnote = Some(footnote.into());
    }
    Some(chart)
}

const LIKELIHOOD_NOTE: &str =
    "The likelihood is scaled to a density: the posterior under the flat reference prior.";

/// The prior, likelihood and posterior chart of coefficient `i` (a kept column).
fn coefficient_chart(
    title: String,
    i: usize,
    column: usize,
    reference: &Linear,
    posterior: &Linear,
    prior: &Option<(Vec<f64>, Matrix, f64, f64)>,
) -> Option<Chart> {
    let prior_curve = prior.as_ref().and_then(|(m0, v0, a0, b0)| {
        t_curve(
            "Prior",
            2.0 * a0,
            m0[column],
            b0 / a0 * v0[column][column],
            false,
        )
    });
    let like =
        |l: &Linear, name, own| t_curve(name, 2.0 * l.a, l.theta[i], l.b / l.a * l.v[i][i], own);
    density_chart(
        title,
        "Value",
        vec![
            prior_curve,
            like(reference, "Likelihood", true),
            like(posterior, "Posterior", true),
        ],
        LIKELIHOOD_NOTE,
    )
}

fn error_chart(
    reference: &Linear,
    posterior: &Linear,
    prior: &Option<(Vec<f64>, Matrix, f64, f64)>,
) -> Option<Chart> {
    density_chart(
        "Error variance".into(),
        "Variance",
        vec![
            prior
                .as_ref()
                .and_then(|(_, _, a0, b0)| inverse_gamma_curve("Prior", *a0, *b0, false)),
            inverse_gamma_curve("Likelihood", reference.a, reference.b, true),
            inverse_gamma_curve("Posterior", posterior.a, posterior.b, true),
        ],
        LIKELIHOOD_NOTE,
    )
}

// ---------------------------------------------------------------------------------------------
// Shared pieces of the commands

/// The ANOVA table with the Bayes factor in the model row.
fn anova_table(names: [&str; 3], fit: &Fit, p: f64, bf: Option<f64>, method: &str) -> Table {
    let (ssm, sse, sst) = (fit.sst - fit.sse, fit.sse, fit.sst);
    let (df1, df2) = (p, fit.n - p - 1.0);
    let f = ssm / df1 / (sse / df2);
    let mut factor = vec![Cell::Empty; 3];
    factor[0] = number(bf);
    let rows = vec![
        vec![
            Cell::Text(names[0].into()),
            number(Some(ssm)),
            number(Some(df1)),
            number(Some(ssm / df1)),
            number(Some(f)),
            number(Some(f_p(f, df1, df2))),
            factor[0].clone(),
        ],
        vec![
            Cell::Text(names[1].into()),
            number(Some(sse)),
            number(Some(df2)),
            number(Some(sse / df2)),
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
        ],
        vec![
            Cell::Text(names[2].into()),
            number(Some(sst)),
            number(Some(fit.n - 1.0)),
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
            Cell::Empty,
        ],
    ];
    let columns = [
        "",
        "Sum of Squares",
        "df",
        "Mean Square",
        "F",
        "Sig.",
        "Bayes Factor",
    ];
    let mut t = table("ANOVA", &columns, rows);
    t.caption = Some(format!(
        "Bayes factor BF10 ({method}): the evidence for the model against the model with the intercept only. R Square = {:.4}.",
        fit.r_square()
    ));
    t
}

/// The Bayes factor of a fit with `p` regressors; NaN with a warning when it is undefined.
fn model_factor(run: &mut Run, prior: GPrior, fit: &Fit, p: f64) -> f64 {
    if fit.n <= p + 1.0 {
        run.warnings.push(format!(
            "no Bayes factor: it needs more than {} cases",
            p + 1.0
        ));
        return f64::NAN;
    }
    if !(fit.sse > 1e-12 * fit.sst) {
        run.warnings
            .push("no Bayes factor: the model fits the data perfectly or the dependent variable does not vary".into());
        return f64::NAN;
    }
    ln_bf10(prior, fit.n, p, fit.r_square(), run.quadrature).exp()
}

/// The sorted categories of `variable` among `cases`.
fn levels(variable: &Variable, cases: &[usize]) -> Vec<Key> {
    let keys = categories(variable, false);
    let mut found: Vec<Key> = Vec::new();
    for &k in cases {
        if let Some(key) = &keys[k]
            && !found.iter().any(|f| f.compare(key) == Ordering::Equal)
        {
            found.push(key.clone());
        }
    }
    found.sort_by(|a, b| a.compare(b));
    found
}

/// The position of the level a syntax value names, if any.
fn level_of(variable: &Variable, levels: &[Key], value: &Value) -> Option<usize> {
    let key = match (value, variable.is_string()) {
        (Value::Number(n), false) => Key::Number(*n),
        (Value::String(s), false) => Key::Number(s.trim().parse().ok()?),
        (Value::String(s), true) => Key::Text(s.trim_end().to_string()),
        (Value::Number(n), true) => Key::Text(n.to_string()),
    };
    levels.iter().position(|l| match (l, &key) {
        (Key::Text(a), Key::Text(b)) => a.trim_end() == b,
        _ => l.compare(&key) == Ordering::Equal,
    })
}

fn value_text(value: &Value) -> String {
    match value {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
    }
}

/// The regression weight of a case: 0 when missing or not above 0.
fn regression_weight(variable: Option<&Variable>, case: usize) -> f64 {
    variable.map_or(1.0, |v| {
        let q = numeric(v, case);
        if is_valid(v, case, false) && q > 0.0 {
            q
        } else {
            0.0
        }
    })
}

fn regwgt_variable<'d>(
    data: &'d Dataset,
    name: &Option<String>,
) -> Result<Option<Variable<'d>>, Error> {
    let Some(name) = name else {
        return Ok(None);
    };
    let v = data.variable(name)?;
    if v.is_string() {
        return Err(Error(format!(
            "REGWGT needs a numeric variable, but {} is a string variable",
            v.name()
        )));
    }
    Ok(Some(v))
}

fn numeric_only(command: &str, variables: &[&Variable]) -> Result<(), Error> {
    match variables.iter().find(|v| v.is_string()) {
        Some(v) => Err(Error(format!(
            "{command} needs a numeric {}, but it is a string variable",
            v.name()
        ))),
        None => Ok(()),
    }
}

fn prior_caption(prior: &LinearPrior) -> String {
    if prior.conjugate {
        "Conjugate prior: inverse gamma on the error variance, normal on the parameters.".into()
    } else {
        "Reference prior, proportional to 1 / variance.".into()
    }
}

// ---------------------------------------------------------------------------------------------
// The commands

impl Engine {
    pub(super) fn bayes_anova(&self, c: &BayesAnova) -> Result<Output, Error> {
        let data = self.active()?;
        let mut run = Run::new(self, data, &c.criteria)?;
        let dependent = data.variable(&c.dependent)?;
        let factor = data.variable(&c.factor)?;
        let weight = regwgt_variable(data, &c.regwgt)?;
        numeric_only("BAYES ANOVA", &[&dependent])?;
        let mut used = vec![&dependent, &factor];
        used.extend(weight.as_ref());
        let cases: Vec<usize> = run
            .cases(&used)
            .into_iter()
            .filter(|&k| regression_weight(weight.as_ref(), k) > 0.0)
            .collect();
        let groups = levels(&factor, &cases);
        if groups.len() < 2 {
            return Err(Error(format!(
                "BAYES ANOVA needs at least two groups of {} with valid cases",
                factor.name()
            )));
        }
        let keys = categories(&factor, false);
        let k = groups.len();
        let rows: Vec<Row> = cases
            .iter()
            .map(|&case| {
                let key = keys[case].as_ref().expect("valid case");
                let group = groups
                    .iter()
                    .position(|g| g.compare(key) == Ordering::Equal)
                    .unwrap_or(0);
                let f = run.weight(case);
                Row {
                    x: (0..k).map(|g| f64::from(u8::from(g == group))).collect(),
                    y: numeric(&dependent, case),
                    w: f * regression_weight(weight.as_ref(), case),
                    f,
                }
            })
            .collect();
        let all: Vec<usize> = (0..k).collect();
        let fit =
            Fit::new(&rows, &all).ok_or_else(|| Error("BAYES ANOVA: no valid cases".into()))?;
        let labels: Vec<String> = groups.iter().map(|g| run.shown.value(&factor, g)).collect();
        let mut out = Vec::new();
        if c.analysis.factor() {
            let p = k as f64 - 1.0;
            let bf = model_factor(&mut run, c.g, &fit, p);
            out.push(anova_table(
                ["Between Groups", "Within Groups", "Total"],
                &fit,
                p,
                Some(bf),
                &method_name(c.g),
            ));
        }
        let prior = prior_of(&mut run, &c.prior, k)?;
        let charts_wanted = !c.plot_means.is_empty() || c.plot_error;
        let mut charts = Vec::new();
        if c.analysis.posterior() || charts_wanted {
            if fit.n <= k as f64 && !c.prior.conjugate {
                run.warnings.push(format!(
                    "no posterior under the reference prior: it needs more than {k} cases"
                ));
            } else if let Some(post) = posterior_of(&fit, &prior) {
                let reference = Linear::reference(&fit);
                let mut table_rows = Vec::new();
                for (g, label) in labels.iter().enumerate() {
                    let n: f64 = rows.iter().filter(|r| r.x[g] == 1.0).map(|r| r.f).sum();
                    let mut row = vec![Cell::Text(label.clone()), Cell::Number(n)];
                    row.extend(posterior_cells(&post.coefficient(g, run.alpha)));
                    table_rows.push(row);
                }
                let mut row = vec![Cell::Text("Error variance".into()), Cell::Number(fit.n)];
                row.extend(posterior_cells(&post.error_variance(run.alpha)));
                table_rows.push(row);
                if c.analysis.posterior() {
                    let title = format!(
                        "Posterior Distribution of the Means of {}",
                        run.shown.variable(&dependent)
                    );
                    let mut t = posterior_table(&run, &title, &["", "N"], table_rows);
                    t.caption = Some(prior_caption(&c.prior));
                    out.push(t);
                }
                for value in &c.plot_means {
                    match level_of(&factor, &groups, value) {
                        Some(g) => charts.extend(coefficient_chart(
                            format!("Mean of {}", labels[g]),
                            g,
                            g,
                            &reference,
                            &post,
                            &prior,
                        )),
                        None => run.warnings.push(format!(
                            "PLOT MEANS: {} is not a group of {}",
                            value_text(value),
                            factor.name()
                        )),
                    }
                }
                if c.plot_error {
                    charts.extend(error_chart(&reference, &post, &prior));
                }
            }
        }
        run.charts.extend(charts);
        Ok((out, run.warnings, run.charts))
    }
}

impl Engine {
    pub(super) fn bayes_regression(&self, c: &BayesRegression) -> Result<(Output, Saved), Error> {
        let data = self.active()?;
        let mut run = Run::new(self, data, &c.criteria)?;
        let dependent = data.variable(&c.dependent)?;
        let pick = |design: &Option<Vec<VarItem>>, all: &[VarItem], what: &str| {
            let all = self.expand(all)?;
            let Some(items) = design else {
                return Ok(all);
            };
            let chosen = self.expand(items)?;
            match chosen.iter().find(|n| !all.contains(n)) {
                Some(n) => Err(Error(format!(
                    "DESIGN {what} names {n}, which is not among the {what} after {}",
                    if what == "FACTORS" { "BY" } else { "WITH" }
                ))),
                None => Ok(chosen),
            }
        };
        let factor_names = pick(&c.design_factors, &c.factors, "FACTORS")?;
        let covariate_names = pick(&c.design_covariates, &c.covariates, "COVARIATES")?;
        if factor_names.is_empty() && covariate_names.is_empty() {
            return Err(Error(
                "BAYES REGRESSION needs at least one factor or covariate in the model".into(),
            ));
        }
        let factors = self.variables(&factor_names)?;
        let covariates = self.variables(&covariate_names)?;
        let mut numeric_ones = vec![&dependent];
        numeric_ones.extend(covariates.iter());
        numeric_only("BAYES REGRESSION", &numeric_ones)?;
        let weight = regwgt_variable(data, &c.regwgt)?;
        let mut used = numeric_ones.clone();
        used.extend(factors.iter());
        used.extend(weight.as_ref());
        let cases: Vec<usize> = run
            .cases(&used)
            .into_iter()
            .filter(|&k| regression_weight(weight.as_ref(), k) > 0.0)
            .collect();
        let factor_levels: Vec<Vec<Key>> = factors.iter().map(|f| levels(f, &cases)).collect();
        let keys: Vec<Vec<Option<Key>>> = factors.iter().map(|f| categories(f, false)).collect();
        // the columns: intercept, the indicators of each factor, the covariates
        let mut names = vec!["(Intercept)".to_string()];
        let mut owner: Vec<Option<usize>> = vec![None];
        for (i, f) in factors.iter().enumerate() {
            for level in &factor_levels[i] {
                names.push(format!(
                    "{} = {}",
                    run.shown.variable(f),
                    run.shown.value(f, level)
                ));
                owner.push(Some(i));
            }
        }
        for (j, v) in covariates.iter().enumerate() {
            names.push(run.shown.variable(v));
            owner.push(Some(factors.len() + j));
        }
        let design_row = |case: usize| -> Option<Vec<f64>> {
            let mut x = vec![1.0];
            for (i, levels) in factor_levels.iter().enumerate() {
                let key = keys[i][case].as_ref()?;
                let at = levels
                    .iter()
                    .position(|l| l.compare(key) == Ordering::Equal)?;
                x.extend((0..levels.len()).map(|l| f64::from(u8::from(l == at))));
            }
            for v in &covariates {
                if !is_valid(v, case, false) {
                    return None;
                }
                x.push(numeric(v, case));
            }
            Some(x)
        };
        let rows: Vec<Row> = cases
            .iter()
            .filter_map(|&case| {
                let f = run.weight(case);
                Some(Row {
                    x: design_row(case)?,
                    y: numeric(&dependent, case),
                    w: f * regression_weight(weight.as_ref(), case),
                    f,
                })
            })
            .collect();
        let columns: Vec<usize> = (0..names.len()).collect();
        let fit = Fit::new(&rows, &columns)
            .ok_or_else(|| Error("BAYES REGRESSION: no valid cases".into()))?;
        let p = fit.kept.len() as f64 - 1.0;
        let mut out = Vec::new();
        if c.analysis.factor() {
            let bf = model_factor(&mut run, c.g, &fit, p);
            out.push(anova_table(
                ["Regression", "Residual", "Total"],
                &fit,
                p,
                Some(bf),
                &method_name(c.g),
            ));
            if c.compare_full {
                let terms: Vec<String> = factors
                    .iter()
                    .chain(covariates.iter())
                    .map(|v| run.shown.variable(v))
                    .collect();
                let mut rows_out = Vec::new();
                for (t, term) in terms.iter().enumerate() {
                    let fewer: Vec<usize> =
                        (0..names.len()).filter(|&i| owner[i] != Some(t)).collect();
                    let (r2, bf1) = match Fit::new(&rows, &fewer) {
                        Some(sub) => {
                            let p1 = sub.kept.len() as f64 - 1.0;
                            let ln1 = if p1 == 0.0 {
                                0.0
                            } else {
                                ln_bf10(c.g, sub.n, p1, sub.r_square(), run.quadrature)
                            };
                            (sub.r_square(), (ln1 - bf.ln()).exp())
                        }
                        None => (f64::NAN, f64::NAN),
                    };
                    rows_out.push(vec![
                        Cell::Text(format!("Without {term}")),
                        number(Some(r2)),
                        number(Some(bf1)),
                    ]);
                }
                rows_out.push(vec![
                    Cell::Text("Intercept only".into()),
                    Cell::Number(0.0),
                    number(Some(1.0 / bf)),
                ]);
                let mut t = table(
                    "Bayes Factors Against the Full Model",
                    &["", "R Square", "Bayes Factor"],
                    rows_out,
                );
                t.caption = Some(format!(
                    "Each Bayes factor ({}) weighs the smaller model against the full model: the ratio of their Bayes factors against the model with the intercept only.",
                    method_name(c.g)
                ));
                out.push(t);
            }
        }
        let prior = prior_of(&mut run, &c.prior, names.len())?;
        let post = posterior_of(&fit, &prior)
            .ok_or_else(|| Error("BAYES REGRESSION: the posterior matrix is singular".into()))?;
        let reference = Linear::reference(&fit);
        let kept_at = |column: usize| fit.kept.iter().position(|&k| k == column);
        if c.analysis.posterior() {
            let mut rows_out = Vec::new();
            for (column, name) in names.iter().enumerate() {
                let cells = match kept_at(column) {
                    Some(i) => posterior_cells(&post.coefficient(i, run.alpha)),
                    None => vec![
                        Cell::Number(0.0),
                        Cell::Number(0.0),
                        Cell::Empty,
                        Cell::Empty,
                        Cell::Empty,
                    ],
                };
                let mut row = vec![Cell::Text(name.clone())];
                row.extend(cells);
                rows_out.push(row);
            }
            let mut row = vec![Cell::Text("Error variance".into())];
            row.extend(posterior_cells(&post.error_variance(run.alpha)));
            rows_out.push(row);
            let mut t = posterior_table(
                &run,
                "Posterior Distribution of the Parameters",
                &[""],
                rows_out,
            );
            let mut caption = prior_caption(&c.prior);
            if fit.kept.len() < names.len() {
                caption.push_str(" A redundant parameter is set to 0.");
            }
            t.caption = Some(caption);
            out.push(t);
        }
        // the columns of a variable named on FTEST or PLOT
        let term_of = |name: &str| {
            factor_names
                .iter()
                .chain(covariate_names.iter())
                .position(|n| n == name)
        };
        let columns_of = |term: usize| -> Vec<usize> {
            (0..names.len())
                .filter(|&i| owner[i] == Some(term))
                .collect()
        };
        if !c.ftests.is_empty() {
            let mut rows_out = Vec::new();
            for (n, test) in c.ftests.iter().enumerate() {
                let mut tested = Vec::new();
                if test.intercept {
                    tested.push(0);
                }
                for name in self.expand(&test.variables)? {
                    let term = term_of(&name).ok_or_else(|| {
                        Error(format!("FTEST names {name}, which is not in the model"))
                    })?;
                    tested.extend(columns_of(term));
                }
                let values = if test.values.is_empty() {
                    vec![0.0; tested.len()]
                } else if test.values.len() == tested.len() {
                    test.values.clone()
                } else {
                    return Err(Error(format!(
                        "FTEST TESTVALUES needs {} values, one per tested parameter",
                        tested.len()
                    )));
                };
                let (which, c0): (Vec<usize>, Vec<f64>) = tested
                    .iter()
                    .zip(&values)
                    .filter_map(|(&col, &v)| Some((kept_at(col)?, v)))
                    .unzip();
                let result = if which.is_empty() {
                    None
                } else {
                    post.f_test(&which, &c0)
                };
                let label = test
                    .label
                    .clone()
                    .unwrap_or_else(|| format!("Test {}", n + 1));
                let mut row = vec![Cell::Text(label)];
                match result {
                    Some((f, df1, df2)) => row.extend([
                        number(Some(f)),
                        number(Some(df1)),
                        number(Some(df2)),
                        number(Some(f_p(f, df1, df2))),
                    ]),
                    None => row.extend(vec![Cell::Empty; 4]),
                }
                rows_out.push(row);
            }
            let mut t = table(
                "Bayesian F Tests",
                &["", "F", "df1", "df2", "Sig."],
                rows_out,
            );
            t.caption = Some("Each test asks whether the tested parameters equal their test values (0 unless given); redundant parameters are left out.".into());
            out.push(t);
        }
        let mut prediction = None;
        if !c.predict.is_empty() {
            let expected = factors.len() + covariates.len();
            if c.predict.len() != expected {
                return Err(Error(format!(
                    "PREDICT REGRESSORS needs {expected} values, one per factor and covariate of the model"
                )));
            }
            let mut x = vec![1.0];
            for (i, f) in factors.iter().enumerate() {
                let at = level_of(f, &factor_levels[i], &c.predict[i]).ok_or_else(|| {
                    Error(format!(
                        "PREDICT: {} is not a category of {}",
                        value_text(&c.predict[i]),
                        f.name()
                    ))
                })?;
                x.extend((0..factor_levels[i].len()).map(|l| f64::from(u8::from(l == at))));
            }
            for (j, v) in covariates.iter().enumerate() {
                match &c.predict[factors.len() + j] {
                    Value::Number(n) => x.push(*n),
                    Value::String(s) => {
                        return Err(Error(format!(
                            "PREDICT needs a number for the covariate {}, got {s}",
                            v.name()
                        )));
                    }
                }
            }
            let x: Vec<f64> = fit.kept.iter().map(|&i| x[i]).collect();
            let (center, scale2) = post.prediction(&x);
            let p = t_posterior(2.0 * post.a, center, scale2, run.alpha);
            let mut row = vec![Cell::Text(format!(
                "Predicted {}",
                run.shown.variable(&dependent)
            ))];
            row.extend(posterior_cells(&p));
            out.push(posterior_table(
                &run,
                "Predictive Distribution",
                &[""],
                vec![row],
            ));
            prediction = Some((center, scale2));
        }
        // SAVE: the predictive distribution of every case with valid regressors
        let mut saved = Vec::new();
        let mut taken: Vec<String> = Vec::new();
        for (stat, given) in &c.save {
            let keyword = PredictStat::KEYWORDS[*stat as usize];
            let name = match given {
                Some(name) => name.clone(),
                None => {
                    let mut n = 1;
                    while data.variable_index(&format!("{keyword}_{n}")).is_some()
                        || taken.contains(&format!("{keyword}_{n}"))
                    {
                        n += 1;
                    }
                    format!("{keyword}_{n}")
                }
            };
            if data.variable_index(&name).is_some() || taken.contains(&name) {
                return Err(Error(format!(
                    "BAYES REGRESSION /SAVE: variable {name} already exists"
                )));
            }
            taken.push(name.clone());
            let values = (0..data.case_count())
                .map(|case| {
                    let x = design_row(case)?;
                    let x: Vec<f64> = fit.kept.iter().map(|&i| x[i]).collect();
                    let (center, scale2) = post.prediction(&x);
                    let p = t_posterior(2.0 * post.a, center, scale2, run.alpha);
                    let v = match stat {
                        PredictStat::Mean => p.mean,
                        PredictStat::Variance => p.variance,
                        PredictStat::Mode => p.mode,
                        PredictStat::Lower => p.lower,
                        PredictStat::Upper => p.upper,
                    };
                    v.is_finite().then_some(v)
                })
                .collect();
            let what = match stat {
                PredictStat::Mean => "Mean",
                PredictStat::Variance => "Variance",
                PredictStat::Mode => "Mode",
                PredictStat::Lower => "Lower credible bound",
                PredictStat::Upper => "Upper credible bound",
            };
            saved.push((
                name,
                format!(
                    "{what} of the predictive distribution of {}",
                    dependent.name()
                ),
                values,
            ));
        }
        if c.outfile.is_some() {
            run.warnings.push(
                "OUTFILE: model files are not written yet; the command ran without it".into(),
            );
        }
        // PLOT
        let mut charts = Vec::new();
        let mut chart_of = |column: usize, title: String| {
            if let Some(i) = kept_at(column) {
                charts.extend(coefficient_chart(
                    title, i, column, &reference, &post, &prior,
                ));
            }
        };
        if c.plot.intercept {
            chart_of(0, "(Intercept)".into());
        }
        for name in self.expand(&c.plot.covariates)? {
            match covariate_names.iter().position(|n| *n == name) {
                Some(j) => chart_of(
                    names.len() - covariates.len() + j,
                    run.shown.variable(&covariates[j]),
                ),
                None => run.warnings.push(format!(
                    "PLOT COVARIATES: {name} is not a covariate of the model"
                )),
            }
        }
        for name in self.expand(&c.plot.factors)? {
            match factor_names.iter().position(|n| *n == name) {
                Some(i) => {
                    for column in columns_of(i).into_iter().take(c.plot.max_category) {
                        chart_of(column, names[column].clone());
                    }
                }
                None => run
                    .warnings
                    .push(format!("PLOT FACTORS: {name} is not a factor of the model")),
            }
        }
        if c.plot.error {
            charts.extend(error_chart(&reference, &post, &prior));
        }
        if c.plot.predictive
            && let Some((center, scale2)) = prediction
        {
            charts.extend(density_chart(
                format!("Prediction of {}", run.shown.variable(&dependent)),
                "Value",
                vec![t_curve(
                    "Posterior predictive",
                    2.0 * post.a,
                    center,
                    scale2,
                    true,
                )],
                "",
            ));
        }
        run.charts.extend(charts);
        Ok(((out, run.warnings, run.charts), saved))
    }

    /// Writes the variables of BAYES REGRESSION /SAVE.
    pub(super) fn save_predictions(&mut self, saved: Saved) -> Result<(), Error> {
        if saved.is_empty() {
            return Ok(());
        }
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            for (name, label, values) in saved {
                data.set_numeric_values(&name, values)?;
                data.set_variable_label(&name, Some(&label))?;
                data.set_format(&name, "F11.5")?;
                data.set_measure(&name, Measure::Scale)?;
            }
            Ok(())
        })
    }

    pub(super) fn bayes_repeated(&self, c: &BayesRepeated) -> Result<Output, Error> {
        let data = self.active()?;
        let mut run = Run::new(self, data, &c.criteria)?;
        let names = self.expand(&c.variables)?;
        if names.len() < 2 {
            return Err(Error("BAYES REPEATED needs at least two variables".into()));
        }
        let variables = self.variables(&names)?;
        let refs: Vec<&Variable> = variables.iter().collect();
        numeric_only("BAYES REPEATED", &refs)?;
        let weight = regwgt_variable(data, &c.regwgt)?;
        let mut used = refs.clone();
        used.extend(weight.as_ref());
        let cases: Vec<usize> = run
            .cases(&used)
            .into_iter()
            .filter(|&k| regression_weight(weight.as_ref(), k) > 0.0)
            .collect();
        if cases.len() < 2 {
            return Err(Error(
                "BAYES REPEATED needs at least two cases with valid values on every variable"
                    .into(),
            ));
        }
        let k = variables.len();
        let y: Vec<Vec<f64>> = cases
            .iter()
            .map(|&case| variables.iter().map(|v| numeric(v, case)).collect())
            .collect();
        let q: Vec<f64> = cases
            .iter()
            .map(|&case| regression_weight(weight.as_ref(), case))
            .collect();
        let f: Vec<f64> = cases.iter().map(|&case| run.weight(case)).collect();
        let w: Vec<f64> = q.iter().zip(&f).map(|(a, b)| a * b).collect();
        let total: f64 = w.iter().sum();
        let kf = k as f64;
        let grand = y
            .iter()
            .zip(&w)
            .map(|(row, w)| w * row.iter().sum::<f64>())
            .sum::<f64>()
            / (total * kf);
        let condition: Vec<f64> = (0..k)
            .map(|j| y.iter().zip(&w).map(|(row, w)| w * row[j]).sum::<f64>() / total)
            .collect();
        let sst: f64 = y
            .iter()
            .zip(&w)
            .map(|(row, w)| w * row.iter().map(|v| (v - grand).powi(2)).sum::<f64>())
            .sum();
        let sss: f64 = kf
            * y.iter()
                .zip(&w)
                .map(|(row, w)| w * (row.iter().sum::<f64>() / kf - grand).powi(2))
                .sum::<f64>();
        let ssc: f64 = total * condition.iter().map(|m| (m - grand).powi(2)).sum::<f64>();
        let sse = sst - ssc - sss;
        let mut out = Vec::new();
        if c.analysis.factor() {
            let (bf, how) = if c.rouder {
                if f.iter().any(|&f| f != 1.0) {
                    run.warnings.push(
                        "ROUDER uses the regression weights only; the case weights are ignored"
                            .into(),
                    );
                }
                let seed = c.criteria.seed.unwrap_or_else(time_seed);
                let draws = c.criteria.mcsamples;
                (
                    rouder_repeated(&y, &q, draws, seed),
                    format!("Rouder's method, {draws} Monte Carlo draws, seed {seed}"),
                )
            } else {
                (
                    bic_repeated(sst, sss, ssc, total, kf),
                    "BIC approximation".into(),
                )
            };
            let df_c = kf - 1.0;
            let df_s = total - 1.0;
            let df_e = df_c * df_s;
            let fv = ssc / df_c / (sse / df_e);
            let rows = vec![
                vec![
                    Cell::Text("Conditions".into()),
                    number(Some(ssc)),
                    number(Some(df_c)),
                    number(Some(ssc / df_c)),
                    number(Some(fv)),
                    number(Some(f_p(fv, df_c, df_e))),
                    number(Some(bf)),
                ],
                vec![
                    Cell::Text("Subjects".into()),
                    number(Some(sss)),
                    number(Some(df_s)),
                    number(Some(sss / df_s)),
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                ],
                vec![
                    Cell::Text("Error".into()),
                    number(Some(sse)),
                    number(Some(df_e)),
                    number(Some(sse / df_e)),
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                ],
            ];
            let columns = [
                "",
                "Sum of Squares",
                "df",
                "Mean Square",
                "F",
                "Sig.",
                "Bayes Factor",
            ];
            let mut t = table("Bayes Factor for Repeated Measures", &columns, rows);
            t.caption = Some(format!(
                "Bayes factor BF10 ({how}): the evidence for different condition means against equal ones."
            ));
            out.push(t);
        }
        // the large-sample normal posterior of the condition means
        let spread = kf * sss - sst + ssc;
        let diagonal = if spread > 0.0 {
            sse / (total * (kf - 1.0)) + spread / (total * kf * (kf - 1.0))
        } else {
            (sst - ssc) / (total * kf)
        };
        let variance = diagonal / total;
        let n: f64 = f.iter().sum();
        if c.analysis.posterior() {
            let rows = variables
                .iter()
                .zip(&condition)
                .map(|(v, &mean)| {
                    let mut row = vec![run.label(v), Cell::Number(n)];
                    row.extend(posterior_cells(&normal_posterior(
                        mean, variance, run.alpha,
                    )));
                    row
                })
                .collect();
            let mut t = posterior_table(
                &run,
                "Posterior Distribution of the Means",
                &["", "N"],
                rows,
            );
            t.caption =
                Some("Normal large-sample approximation; it does not depend on a prior.".into());
            out.push(t);
        }
        for name in self.expand(&c.plot_means)? {
            let Some(j) = names.iter().position(|n| *n == name) else {
                run.warnings.push(format!(
                    "PLOT MEANS: {name} is not one of the repeated variables"
                ));
                continue;
            };
            let (mean, sd) = (condition[j], variance.sqrt());
            let curve: Curve = (
                "Posterior",
                Box::new(move |x| {
                    (-0.5 * ((x - mean) / sd).powi(2)).exp() / (sd * (2.0 * PI).sqrt())
                }),
                Some((mean - 3.3 * sd, mean + 3.3 * sd)),
            );
            let chart = density_chart(
                format!("Mean of {}", run.shown.variable(&variables[j])),
                "Value",
                vec![Some(curve)],
                "",
            );
            run.charts.extend(chart);
        }
        Ok((out, run.warnings, run.charts))
    }
}

/// BF10 of the repeated measures by the BIC approximation of Nathoo and Masson.
fn bic_repeated(sst: f64, sss: f64, ssc: f64, w: f64, k: f64) -> f64 {
    let delta = if k * sss > sst {
        w * (k - 1.0) * ((sst - ssc - sss) / (sst - sss)).ln()
            + (k + 2.0) * (w * (sst - ssc) / sss).ln()
            - 3.0 * (w * sst / sss).ln()
    } else if k * sss > sst - ssc {
        w * (sss / w).ln() + w * (k - 1.0) * ((sst - ssc - sss) / (w * (k - 1.0))).ln()
            - w * k * (sst / (w * k)).ln()
            - 3.0 * (w * k).ln()
            + (k + 2.0) * (w * (sst - ssc) / sss).ln()
    } else {
        w * k * ((sst - ssc) / sst).ln() + (k - 1.0) * (w * k).ln()
    };
    (-delta / 2.0).exp()
}

/// BF10 of Rouder's mixed model: the mean over `draws` pairs of g (each 1 over a squared
/// standard normal) of the factor with conditions and subjects, over the mean of the factor
/// with subjects only. `y` holds the k measures of each subject, `q` its regression weight.
///
/// The conditions and the centered subject effects are orthogonal, so the determinant and
/// the quadratic form of the manual's matrices reduce to the eigenvalues of the weighted
/// subject block (all equal without weights).
fn rouder_repeated(y: &[Vec<f64>], q: &[f64], draws: usize, seed: u64) -> f64 {
    let (n, k) = (y.len(), y[0].len());
    let (nf, kf) = (n as f64, k as f64);
    let total: f64 = q.iter().sum();
    let s: Vec<f64> = q.iter().map(|q| q.sqrt()).collect();
    let center = y
        .iter()
        .zip(&s)
        .map(|(row, s)| s * row.iter().sum::<f64>())
        .sum::<f64>()
        / (nf * kf);
    let centered: f64 = y
        .iter()
        .zip(&s)
        .map(|(row, s)| row.iter().map(|v| (s * v - center).powi(2)).sum::<f64>())
        .sum();
    let weighted_mean = y
        .iter()
        .zip(q)
        .map(|(row, q)| q * row.iter().sum::<f64>())
        .sum::<f64>()
        / (kf * total);
    let weighted_ss = y
        .iter()
        .zip(q)
        .map(|(row, q)| q * row.iter().map(|v| v * v).sum::<f64>())
        .sum::<f64>()
        - kf * total * weighted_mean * weighted_mean;
    let sums: Vec<f64> = (0..k)
        .map(|j| y.iter().zip(q).map(|(row, q)| q * row[j]).sum())
        .collect();
    let mean_sum = sums.iter().sum::<f64>() / kf;
    let conditions: f64 = sums.iter().map(|c| (c - mean_sum).powi(2)).sum();
    // the subject block: K = D^1/2 (I - J/n) D^1/2 and the projection z of the data on it
    let r: Vec<f64> = y
        .iter()
        .zip(&s)
        .map(|(row, s)| row.iter().map(|v| s * v - center).sum())
        .collect();
    let r_mean = r.iter().sum::<f64>() / nf;
    let z: Vec<f64> = r.iter().zip(&s).map(|(r, s)| s * (r - r_mean)).collect();
    // (eigenvalue, squared projection, multiplicity)
    let blocks: Vec<(f64, f64, f64)> = if q.iter().all(|&w| w == q[0]) {
        vec![(q[0], z.iter().map(|v| v * v).sum(), nf - 1.0)]
    } else {
        let block: Matrix = (0..n)
            .map(|a| {
                (0..n)
                    .map(|b| s[a] * (f64::from(u8::from(a == b)) - 1.0 / nf) * s[b])
                    .collect()
            })
            .collect();
        symmetric_eigen(&block)
            .into_iter()
            .map(|(lambda, e)| (lambda.max(0.0), dot(&e, &z).powi(2), 1.0))
            .collect()
    };
    let power = (kf * total - 1.0) / 2.0;
    let ln_s = |g1: Option<f64>, g2: f64| {
        let mut det = 0.0;
        let mut quad = 0.0;
        if let Some(g1) = g1 {
            det += (kf - 1.0) * (total * g1).ln_1p();
            quad += conditions / (total + 1.0 / g1);
        }
        for &(lambda, u2, times) in &blocks {
            det += times * (kf * lambda * g2).ln_1p();
            quad += u2 * g2 / (kf * lambda * g2 + 1.0);
        }
        -0.5 * det + power * (weighted_ss.ln() - (centered - quad).ln())
    };
    let mut random = Random::default();
    random.seed = seed;
    let (mut full, mut aux) = (Vec::with_capacity(draws), Vec::with_capacity(draws));
    for _ in 0..draws {
        let g1 = 1.0 / random.normal().powi(2);
        let g2 = 1.0 / random.normal().powi(2);
        full.push(ln_s(Some(g1), g2));
        aux.push(ln_s(None, g2));
    }
    (log_mean_exp(&full) - log_mean_exp(&aux)).exp()
}

fn log_mean_exp(values: &[f64]) -> f64 {
    let top = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    top + (values.iter().map(|v| (v - top).exp()).sum::<f64>() / values.len() as f64).ln()
}

/// The prior, likelihood and posterior of a correlation `r` from `w` cases, with the prior
/// power `c`.
pub(super) fn correlation_chart(
    title: String,
    r: f64,
    w: f64,
    c: f64,
    q: Quadrature,
) -> Option<Chart> {
    let ln_prior = move |rho: f64| c * (1.0 - rho * rho).ln();
    let ln_like =
        move |rho: f64| (w - 1.0) / 2.0 * (1.0 - rho * rho).ln() - (w - 1.5) * (1.0 - rho * r).ln();
    // the log of the normalizing integral of exp(h) over rho, taken over z = atanh(rho)
    let normalizer = |h: &dyn Fn(f64) -> f64, power: f64| {
        let reach = 20.0 + 60.0 / (power + 1.0);
        log_integral(
            &|z: f64| h(z.tanh()) + ln_sech2(z),
            -reach,
            reach,
            q.tol,
            q.limit,
        )
    };
    let range = |p: Posterior| {
        let sd = p.variance.sqrt();
        (p.mean.is_finite() && sd.is_finite())
            .then(|| ((p.mean - 6.0 * sd).max(-1.0), (p.mean + 6.0 * sd).min(1.0)))
    };
    let alpha = 0.05;
    let like_norm = normalizer(&ln_like, (w - 1.0) / 2.0);
    let post_norm = normalizer(&|rho| ln_prior(rho) + ln_like(rho), c + (w - 1.0) / 2.0);
    let mut curves: Vec<Option<Curve>> = Vec::new();
    if c > -1.0 {
        let prior_norm = normalizer(&ln_prior, c);
        curves.push(Some((
            "Prior",
            Box::new(move |rho| (ln_prior(rho) - prior_norm).exp()),
            None,
        )));
    }
    curves.push(Some((
        "Likelihood",
        Box::new(move |rho| (ln_like(rho) - like_norm).exp()),
        range(correlation_posterior(r, w, 0.0, alpha, q)),
    )));
    curves.push(Some((
        "Posterior",
        Box::new(move |rho| (ln_prior(rho) + ln_like(rho) - post_norm).exp()),
        range(correlation_posterior(r, w, c, alpha, q)),
    )));
    density_chart(
        title,
        "rho",
        curves,
        "The likelihood is scaled to a density.",
    )
}
