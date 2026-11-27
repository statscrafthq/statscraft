//! PROBIT: dose-response models of response counts out of subject counts, with a probit or
//! logit link, an optional natural response rate, effective levels with fiducial limits,
//! relative median potency and a parallelism test.
//!
//! Rules noted from the Command Syntax Reference and the Algorithms manual:
//! - P_i = c + (1 - c) F(x_i'b + a_g), F the standard normal or logistic distribution, a_g the
//!   intercept of the case's factor level (one intercept without a factor), c the natural
//!   response rate (0 unless NATRES). Log-likelihood sum r ln P + (n - r) ln(1 - P).
//! - Start: slopes and intercepts 0; c from the control cases (a covariate of 0), else the
//!   lowest observed rate. Under a logarithm the control cases stay out of the fit.
//! - The covariance matrix is the inverse of the observed information. Pearson chi-square
//!   sum (r - nP)^2 / (nP(1 - P)) with df = cases - parameters.
//! - Effective level for rate p in level j: x = (T(p) - a_j)/b, with fiducial limits
//!   (Fieller); g = t^2 v(b) h / b^2. Without heterogeneity t = 1.96 and h = 1; with it
//!   (goodness-of-fit significance below P) t is Student's at 0.975 with the test's df and
//!   h = chi-square/df. Under a logarithm the levels also come back as base^x.
//! - Relative median potency of j against k: (a_k - a_j)/b, the median level of j less that of
//!   k (a ratio under a logarithm), limits by Fieller's theorem.
//! - Parallelism: the goodness-of-fit chi-square of the common-slope model minus those of each
//!   level fitted alone, on levels - 1 df.

use crate::chart::{Axis, ChartKind};
use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, normal_cdf, normal_p, normal_quantile, t_quantile};
use crate::matrix::{Matrix, cholesky, inverse};
use crate::npar_tests::text;
use crate::plot::{Plot, Point, PointGroup};
use crate::regression::{plain, table};
use crate::wls::numeric_column;
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_syntax::{NaturalResponse, Probit};

#[derive(Clone, Copy, PartialEq)]
enum Link {
    Probit,
    Logit,
}

impl Link {
    fn name(self) -> &'static str {
        match self {
            Link::Probit => "PROBIT",
            Link::Logit => "LOGIT",
        }
    }
    /// F, its density and the density's derivative at eta.
    fn at(self, eta: f64) -> (f64, f64, f64) {
        match self {
            Link::Probit => {
                let d = (-0.5 * eta * eta).exp() / (2.0 * std::f64::consts::PI).sqrt();
                (normal_cdf(eta), d, -eta * d)
            }
            Link::Logit => {
                let f = 1.0 / (1.0 + (-eta).exp());
                let d = f * (1.0 - f);
                (f, d, d * (1.0 - 2.0 * f))
            }
        }
    }
    /// The inverse of F.
    fn transform(self, p: f64) -> f64 {
        match self {
            Link::Probit => normal_quantile(p),
            Link::Logit => (p / (1.0 - p)).ln(),
        }
    }
}

/// A case in the fit: counts, transformed covariates and factor level.
struct Unit {
    case: usize,
    r: f64,
    n: f64,
    x: Vec<f64>,
    group: usize,
}

struct Fit {
    /// The slopes, then one intercept per level.
    theta: Vec<f64>,
    c: f64,
    /// Whether c is a parameter of the covariance matrix.
    c_free: bool,
    cov: Matrix,
    iterations: usize,
    converged: bool,
    /// The fitted P of every unit.
    p: Vec<f64>,
}

impl Fit {
    fn pearson(&self, units: &[Unit]) -> f64 {
        units
            .iter()
            .zip(&self.p)
            .map(|(u, p)| (u.r - u.n * p).powi(2) / (u.n * p * (1.0 - p)))
            .sum()
    }
}

struct Settings {
    link: Link,
    slopes: usize,
    groups: usize,
    /// Some(start) to estimate c, None to keep it fixed at `c`.
    estimate: Option<f64>,
    c: f64,
    max_iter: usize,
    tolerance: f64,
}

/// What one unit gives at theta and c.
struct UnitTerms {
    p: f64,
    /// dP by each parameter.
    dp: Vec<f64>,
    /// d eta by each parameter.
    z: Vec<f64>,
    ll: f64,
    /// The density of F at eta and its derivative.
    d: f64,
    d2: f64,
}

fn evaluate(s: &Settings, theta: &[f64], c: f64, u: &Unit) -> UnitTerms {
    let k = s.slopes;
    let eta: f64 = (0..k).map(|j| theta[j] * u.x[j]).sum::<f64>() + theta[k + u.group];
    let (f, d, d2) = s.link.at(eta);
    let p = c + (1.0 - c) * f;
    let size = k + s.groups + usize::from(s.estimate.is_some());
    let mut z = vec![0.0; size];
    z[..k].copy_from_slice(&u.x[..k]);
    z[k + u.group] = 1.0;
    let mut dp: Vec<f64> = z.iter().map(|z| (1.0 - c) * d * z).collect();
    if s.estimate.is_some() {
        dp[size - 1] = 1.0 - f;
    }
    let ll = if u.r > 0.0 { u.r * p.ln() } else { 0.0 }
        + if u.n > u.r {
            (u.n - u.r) * (1.0 - p).ln()
        } else {
            0.0
        };
    UnitTerms {
        p,
        dp,
        z,
        ll,
        d,
        d2,
    }
}

/// The score and the observed and expected information at theta and c.
fn information(
    s: &Settings,
    units: &[Unit],
    theta: &[f64],
    c: f64,
) -> (Vec<f64>, Matrix, Matrix, f64) {
    let k = s.slopes;
    let size = k + s.groups + usize::from(s.estimate.is_some());
    let mut score = vec![0.0; size];
    let mut observed = vec![vec![0.0; size]; size];
    let mut expected = vec![vec![0.0; size]; size];
    let mut ll = 0.0;
    for u in units {
        let UnitTerms {
            p,
            dp,
            z,
            ll: l,
            d,
            d2,
        } = evaluate(s, theta, c, u);
        ll += l;
        let a1 = (u.r - u.n * p) / (p * (1.0 - p));
        let a2 = u.r / (p * p) + (u.n - u.r) / ((1.0 - p) * (1.0 - p));
        let e = u.n / (p * (1.0 - p));
        for a in 0..size {
            score[a] += a1 * dp[a];
            for b in 0..size {
                // the second derivative of P: (1 - c) f' z z' on the model parameters, -f z
                // against c
                let second = match (s.estimate.is_some(), a == size - 1, b == size - 1) {
                    (true, true, true) => 0.0,
                    (true, true, false) => -d * z[b],
                    (true, false, true) => -d * z[a],
                    _ => (1.0 - c) * d2 * z[a] * z[b],
                };
                observed[a][b] += a2 * dp[a] * dp[b] - a1 * second;
                expected[a][b] += e * dp[a] * dp[b];
            }
        }
    }
    (score, observed, expected, ll)
}

fn log_likelihood(s: &Settings, units: &[Unit], theta: &[f64], c: f64) -> f64 {
    units.iter().map(|u| evaluate(s, theta, c, u).ll).sum()
}

/// The fit on covariates centred and scaled to unit spread, so that their units and their
/// distance from 0 cannot upset the Newton steps: with x = m + sd x*, the slopes b* = sd b and the
/// level intercepts g* = g + sum m b of the scaled model give b = b* / sd and g = g* - sum m b,
/// and the covariance matrix follows by the same linear map. The probabilities do not change.
fn fit(s: &Settings, units: &[Unit]) -> Result<Fit, Error> {
    let k = s.slopes;
    if k == 0 || units.is_empty() {
        return fit_scaled(s, units);
    }
    let count = units.len() as f64;
    let centre: Vec<f64> = (0..k)
        .map(|j| units.iter().map(|u| u.x[j]).sum::<f64>() / count)
        .collect();
    let spread: Vec<f64> = (0..k)
        .map(|j| {
            let sd = (units
                .iter()
                .map(|u| (u.x[j] - centre[j]).powi(2))
                .sum::<f64>()
                / count)
                .sqrt();
            if sd > 0.0 && sd.is_finite() { sd } else { 1.0 }
        })
        .collect();
    let scaled: Vec<Unit> = units
        .iter()
        .map(|u| Unit {
            case: u.case,
            r: u.r,
            n: u.n,
            x: (0..k).map(|j| (u.x[j] - centre[j]) / spread[j]).collect(),
            group: u.group,
        })
        .collect();
    let mut f = fit_scaled(s, &scaled)?;
    let size = f.theta.len() + usize::from(f.c_free);
    // theta = T theta*
    let mut t = vec![vec![0.0; size]; size];
    for (j, row) in t.iter_mut().enumerate().take(k) {
        row[j] = 1.0 / spread[j];
    }
    for g in 0..s.groups {
        t[k + g][k + g] = 1.0;
        for j in 0..k {
            t[k + g][j] = -centre[j] / spread[j];
        }
    }
    if f.c_free {
        t[size - 1][size - 1] = 1.0;
    }
    let params: Vec<f64> = (0..f.theta.len())
        .map(|a| (0..f.theta.len()).map(|b| t[a][b] * f.theta[b]).sum())
        .collect();
    f.theta = params;
    f.cov = crate::matrix::multiply(
        &crate::matrix::multiply(&t, &f.cov),
        &crate::matrix::transpose(&t),
    );
    Ok(f)
}

/// Maximum likelihood by Newton steps (the expected information where the observed one is
/// not positive definite), halved until the log-likelihood does not fall.
fn fit_scaled(s: &Settings, units: &[Unit]) -> Result<Fit, Error> {
    let params = s.slopes + s.groups;
    let mut theta = vec![0.0; params];
    let mut c = s.estimate.unwrap_or(s.c);
    let free = s.estimate.is_some();
    let mut ll = log_likelihood(s, units, &theta, c);
    let (mut iterations, mut converged) = (0, false);
    let singular =
        || Error("PROBIT cannot estimate the model: the information matrix is singular".into());
    while iterations < s.max_iter {
        iterations += 1;
        let (score, observed, expected, _) = information(s, units, &theta, c);
        let matrix = if cholesky(&observed).is_some() {
            observed
        } else {
            expected
        };
        let (inv, _) = inverse(&matrix).ok_or_else(singular)?;
        let step: Vec<f64> = inv
            .iter()
            .map(|row| row.iter().zip(&score).map(|(a, b)| a * b).sum())
            .collect();
        let mut t = 1.0;
        let (mut next_theta, mut next_c, mut next_ll);
        loop {
            next_theta = (0..params)
                .map(|j| theta[j] + t * step[j])
                .collect::<Vec<_>>();
            next_c = if free {
                (c + t * step[params]).clamp(0.0, 1.0 - 1e-10)
            } else {
                c
            };
            next_ll = log_likelihood(s, units, &next_theta, next_c);
            if next_ll.is_finite() && next_ll >= ll - 1e-12 * ll.abs() || t < 1e-10 {
                break;
            }
            t /= 2.0;
        }
        // no step along this direction helps: keep the last point, not converged
        if !(next_ll.is_finite() && next_ll >= ll - 1e-12 * ll.abs()) {
            break;
        }
        let change = (0..params)
            .map(|j| (next_theta[j] - theta[j]).abs() / theta[j].abs().max(1.0))
            .fold((next_c - c).abs(), f64::max);
        let ll_change = (next_ll - ll).abs() / ll.abs().max(1.0);
        (theta, c, ll) = (next_theta, next_c, next_ll);
        if change <= s.tolerance.sqrt() && ll_change <= s.tolerance.sqrt() {
            converged = true;
            break;
        }
    }
    // a rate at its bound 0 is no longer a parameter of the covariance matrix
    let c_free = free && c > 1e-8;
    let fixed = Settings {
        estimate: c_free.then_some(c),
        c,
        ..*s
    };
    let (_, observed, _, _) = information(&fixed, units, &theta, c);
    let (cov, _) = inverse(&observed).ok_or_else(singular)?;
    let p = units
        .iter()
        .map(|u| evaluate(&fixed, &theta, c, u).p)
        .collect();
    Ok(Fit {
        theta,
        c,
        c_free,
        cov,
        iterations,
        converged,
        p,
    })
}

/// Fieller's limits for the ratio num / b, given var(num), var(b), cov(num, b), the critical
/// value t and the heterogeneity factor h. None when g reaches 1.
fn fieller(
    num: f64,
    b: f64,
    v_num: f64,
    v_b: f64,
    c_nb: f64,
    t: f64,
    h: f64,
) -> Option<(f64, f64)> {
    let m = num / b;
    let g = t * t * v_b * h / (b * b);
    if g >= 1.0 {
        return None;
    }
    let centre = m + g / (1.0 - g) * (m - c_nb / v_b);
    let radicand = h * (v_num - 2.0 * m * c_nb + m * m * v_b - g * (v_num - c_nb * c_nb / v_b));
    if radicand < 0.0 {
        return None;
    }
    let half = t / (b.abs() * (1.0 - g)) * radicand.sqrt();
    Some((centre - half, centre + half))
}

/// The rates of the confidence limits table: 0.01 to 0.09, 0.10 to 0.90 by 0.05, 0.91 to 0.99.
fn rates() -> Vec<f64> {
    let low = (1..=9).map(|i| f64::from(i) / 100.0);
    let mid = (0..=16).map(|i| f64::from(10 + 5 * i) / 100.0);
    let high = (91..=99).map(|i| f64::from(i) / 100.0);
    low.chain(mid).chain(high).collect()
}

impl Engine {
    pub(crate) fn probit(&mut self, command: &Probit) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let covariates = self.expand(&command.covariates)?;
        let include = command.include;
        let r = numeric_column(data, &command.response, include, "PROBIT")?;
        let n = numeric_column(data, &command.total, include, "PROBIT")?;
        let xs: Vec<Vec<f64>> = covariates
            .iter()
            .map(|name| numeric_column(data, name, include, "PROBIT"))
            .collect::<Result<_, _>>()?;
        let factor = match &command.factor {
            Some((name, lo, hi)) => {
                Some((numeric_column(data, name, include, "PROBIT")?, *lo, *hi))
            }
            None => None,
        };
        let weights = case_weights(data)?;
        let log_scale = command.log_base.map(f64::ln);
        let (mut missing, mut outside, mut no_log) = (0usize, 0usize, 0usize);
        let (mut control_r, mut control_n, mut controls) = (0.0, 0.0, 0usize);
        let mut units: Vec<Unit> = Vec::new();
        for k in 0..data.case_count() {
            let w = weights.as_ref().map_or(1.0, |w| w[k]);
            if w <= 0.0 {
                continue;
            }
            let level = factor.as_ref().map(|(f, _, _)| f[k].trunc());
            let low = factor.as_ref().map(|(_, lo, _)| *lo);
            let values = [r[k], n[k]].into_iter().chain(xs.iter().map(|x| x[k]));
            if values.chain(level).any(f64::is_nan) {
                missing += 1;
                continue;
            }
            if let (Some(level), Some((_, lo, hi))) = (level, &factor)
                && (level < *lo || level > *hi)
            {
                outside += 1;
                continue;
            }
            if r[k] < 0.0 || r[k] > n[k] {
                return Err(Error(format!(
                    "PROBIT: case {} has {} responses out of {} subjects; the responses must be from 0 to the subjects",
                    k + 1,
                    r[k],
                    n[k]
                )));
            }
            let x: Vec<f64> = xs.iter().map(|x| x[k]).collect();
            let control = x.contains(&0.0);
            if control {
                controls += 1;
                control_r += w * r[k];
                control_n += w * n[k];
            }
            let x = match log_scale {
                None => x,
                Some(_) if control => continue,
                Some(scale) => {
                    if x.iter().any(|v| *v < 0.0) {
                        no_log += 1;
                        continue;
                    }
                    x.iter().map(|v| v.ln() / scale).collect()
                }
            };
            if n[k] <= 0.0 {
                continue;
            }
            units.push(Unit {
                case: k,
                r: w * r[k],
                n: w * n[k],
                x,
                // the offset from the range's low end, which may be negative
                group: level.zip(low).map_or(0, |(l, lo)| (l - lo) as usize),
            });
        }
        // the factor levels that have cases, in order
        let mut levels: Vec<usize> = units.iter().map(|u| u.group).collect();
        levels.sort_unstable();
        levels.dedup();
        for u in &mut units {
            u.group = levels.binary_search(&u.group).expect("collected");
        }
        let p = covariates.len();
        let q = levels.len().max(1);
        let estimate_c = matches!(command.natres, NaturalResponse::Estimate);
        let s_count = p + q + usize::from(estimate_c);
        if units.len() <= s_count {
            return Err(Error(format!(
                "PROBIT needs more valid cases than its {s_count} parameters, but has {}",
                units.len()
            )));
        }
        let (responses, subjects) = units
            .iter()
            .fold((0.0, 0.0), |(r, n), u| (r + u.r, n + u.n));
        if responses == 0.0 || responses == subjects {
            return Err(Error(format!(
                "PROBIT: {} of the subjects responded, so the response curve cannot be estimated",
                if responses == 0.0 { "none" } else { "all" }
            )));
        }
        let mut warnings = Vec::new();
        let c_start = match command.natres {
            NaturalResponse::Estimate if control_n > 0.0 => Some(control_r / control_n),
            NaturalResponse::Estimate => {
                warnings.push(
                    "PROBIT: there is no control group; the natural response rate starts from the lowest observed rate"
                        .to_string(),
                );
                Some(units.iter().map(|u| u.r / u.n).fold(1.0, f64::min))
            }
            _ => None,
        }
        .map(|c| c.clamp(1e-6, 0.99));
        let c_fixed = match command.natres {
            NaturalResponse::Fixed(c) => c,
            _ => 0.0,
        };
        let factor_var = match &command.factor {
            Some((name, _, _)) => Some(data.variable(name)?),
            None => None,
        };
        let level_text = |g: usize| -> String {
            match &factor_var {
                Some(v) => shown.number(
                    v,
                    levels[g] as f64 + command.factor.as_ref().map_or(0.0, |f| f.1),
                ),
                None => String::new(),
            }
        };
        let names: Vec<String> = covariates.iter().map(|c| shown.named(data, c)).collect();
        let factor_text = factor_var.as_ref().map(|v| shown.variable(v));

        // Data Information
        let mut rows = vec![
            vec![text("Valid cases"), Cell::Number(units.len() as f64)],
            vec![text("Rejected: missing"), Cell::Number(missing as f64)],
        ];
        if factor.is_some() {
            rows.push(vec![
                text("Rejected: outside the factor range"),
                Cell::Number(outside as f64),
            ]);
        }
        if log_scale.is_some() {
            rows.push(vec![
                text("Rejected: logarithm undefined"),
                Cell::Number(no_log as f64),
            ]);
        }
        rows.push(vec![text("Control group"), Cell::Number(controls as f64)]);
        if let Some(f) = &factor_text {
            for g in 0..levels.len() {
                let count = units.iter().filter(|u| u.group == g).count();
                rows.push(vec![
                    text(format!("{f} = {}", level_text(g))),
                    Cell::Number(count as f64),
                ]);
            }
        }
        let mut tables = vec![table(
            "Data Information",
            1,
            plain(&["", "N of Cases"]),
            rows,
            log_scale.is_some().then(|| {
                "Control cases (a covariate of 0) stay out of the fit under a logarithm."
                    .to_string()
            }),
        )];

        let links: Vec<Link> = [(command.logit, Link::Logit), (command.probit, Link::Probit)]
            .into_iter()
            .filter_map(|(on, link)| on.then_some(link))
            .collect();
        let base_text = match command.log_base {
            Some(b) if (b - std::f64::consts::E).abs() < 1e-12 => {
                " Covariates enter as their natural logarithm.".to_string()
            }
            Some(b) => format!(" Covariates enter as their base {b} logarithm."),
            None => String::new(),
        };
        let single = p == 1;
        let mut charts = Vec::new();
        for link in links {
            let settings = Settings {
                link,
                slopes: p,
                groups: q,
                estimate: c_start,
                c: c_fixed,
                max_iter: command.iterate.unwrap_or_else(|| 50.max(3 * (s_count + 1))),
                tolerance: command.optol.unwrap_or(f64::EPSILON.powf(0.8)),
            };
            let model = link.name();
            let fit = fit(&settings, &units)?;
            if !fit.converged {
                warnings.push(format!(
                    "PROBIT ({model}): no convergence after {} iterations; the estimates are the last ones",
                    fit.iterations
                ));
            }
            if estimate_c && !fit.c_free {
                warnings.push(format!(
                    "PROBIT ({model}): the natural response rate is at its bound 0 and is treated as known"
                ));
            }
            let caption = |extra: &str| Some(format!("{model} model.{extra}"));
            tables.push(table(
                "Convergence Information",
                1,
                plain(&["", "Value"]),
                vec![
                    vec![
                        text("Number of Iterations"),
                        Cell::Number(fit.iterations as f64),
                    ],
                    vec![
                        text("Optimal Solution Found"),
                        text(if fit.converged { "Yes" } else { "No" }),
                    ],
                ],
                caption(""),
            ));
            // parameter names in the order of theta, then the natural response rate
            let mut params: Vec<String> = names.clone();
            for g in 0..q {
                params.push(match &factor_text {
                    Some(f) => format!("Intercept, {f} = {}", level_text(g)),
                    None => "Intercept".into(),
                });
            }
            let z = normal_quantile(0.975);
            let rows = (0..p + q)
                .map(|j| {
                    let (b, se) = (fit.theta[j], fit.cov[j][j].sqrt());
                    vec![
                        text(params[j].clone()),
                        Cell::Number(b),
                        number(Some(se)),
                        number(Some(b / se)),
                        number(Some(normal_p(b / se))),
                        number(Some(b - z * se)),
                        number(Some(b + z * se)),
                    ]
                })
                .collect();
            let columns = vec![
                (String::new(), "Parameter".into()),
                (String::new(), "Estimate".into()),
                (String::new(), "Std. Error".into()),
                (String::new(), "Z".into()),
                (String::new(), "Sig.".into()),
                ("95% Confidence Interval".into(), "Lower Bound".into()),
                ("95% Confidence Interval".into(), "Upper Bound".into()),
            ];
            tables.push(table(
                "Parameter Estimates",
                1,
                columns,
                rows,
                caption(&base_text),
            ));
            if command.natres != NaturalResponse::None {
                let se = fit.c_free.then(|| fit.cov[p + q][p + q].sqrt());
                tables.push(table(
                    "Natural Response Rate",
                    1,
                    plain(&["", "Estimate", "Std. Error"]),
                    vec![vec![
                        text("Natural Response"),
                        Cell::Number(fit.c),
                        number(se),
                    ]],
                    caption(match (command.natres, fit.c_free) {
                        (NaturalResponse::Fixed(_), _) => " The rate is fixed by NATRES.",
                        (_, false) => " The estimate is at its bound 0.",
                        _ => "",
                    }),
                ));
            }
            let mut cov_names = params.clone();
            if fit.c_free {
                cov_names.push("Natural Response".into());
            }
            let size = cov_names.len();
            let rows = (0..size)
                .map(|a| {
                    std::iter::once(text(cov_names[a].clone()))
                        .chain((0..size).map(|b| {
                            let v = fit.cov[a][b];
                            Cell::Number(if b > a {
                                v / (fit.cov[a][a] * fit.cov[b][b]).sqrt()
                            } else {
                                v
                            })
                        }))
                        .collect()
                })
                .collect();
            let mut columns = vec![(String::new(), String::new())];
            columns.extend(cov_names.iter().map(|n| (String::new(), n.clone())));
            tables.push(table(
                "Covariances and Correlations of Parameter Estimates",
                1,
                columns,
                rows,
                caption(" Covariances on and below the diagonal, correlations above it."),
            ));

            let chi = fit.pearson(&units);
            let df = (units.len() - s_count) as f64;
            let sig = chi_square_p(chi, df);
            let heterogeneity = df > 0.0
                && (command.heterogeneity >= 1.0
                    || (command.heterogeneity > 0.0 && sig < command.heterogeneity));
            let mut rows = vec![vec![
                text("Pearson Goodness-of-Fit Test"),
                Cell::Number(chi),
                Cell::Number(df),
                number(Some(sig)),
            ]];
            if command.parall && factor.is_some() && q > 1 {
                if single {
                    let alone = Settings {
                        groups: 1,
                        estimate: None,
                        c: fit.c,
                        ..settings
                    };
                    // a level that cannot be fitted on its own drops the test, not the command
                    let separate: Option<f64> = (0..q)
                        .map(|g| {
                            let own: Vec<Unit> = units
                                .iter()
                                .filter(|u| u.group == g)
                                .map(|u| Unit {
                                    case: u.case,
                                    r: u.r,
                                    n: u.n,
                                    x: u.x.clone(),
                                    group: 0,
                                })
                                .collect();
                            fit_alone(&alone, &own)
                        })
                        .sum();
                    match separate {
                        Some(separate) => {
                            let par = chi - separate;
                            rows.push(vec![
                                text("Parallelism Test"),
                                Cell::Number(par),
                                Cell::Number((q - 1) as f64),
                                number(Some(chi_square_p(par, (q - 1) as f64))),
                            ]);
                        }
                        None => warnings.push(format!(
                            "PROBIT ({model}): a factor level cannot be fitted on its own, so there is no parallelism test"
                        )),
                    }
                } else {
                    warnings.push("PROBIT: the parallelism test needs a single covariate".into());
                }
            }
            let het_note = if heterogeneity {
                format!(
                    " The goodness-of-fit significance is below {}, so the confidence limits use a heterogeneity factor.",
                    command.heterogeneity
                )
            } else {
                String::new()
            };
            tables.push(table(
                "Chi-Square Tests",
                1,
                plain(&["", "Chi-Square", "df", "Sig."]),
                rows,
                caption(&het_note),
            ));

            if command.freq {
                let mut columns = vec![(String::new(), "Number".to_string())];
                if let Some(f) = &factor_text {
                    columns.push((String::new(), f.clone()));
                }
                columns.extend(names.iter().map(|n| (String::new(), n.clone())));
                columns.extend(plain(&[
                    "Number of Subjects",
                    "Observed Responses",
                    "Expected Responses",
                    "Residual",
                    "Probability",
                ]));
                let rows = units
                    .iter()
                    .zip(&fit.p)
                    .enumerate()
                    .map(|(i, (u, prob))| {
                        let mut row = vec![Cell::Number((i + 1) as f64)];
                        if factor_text.is_some() {
                            row.push(text(level_text(u.group)));
                        }
                        row.extend(xs.iter().map(|x| Cell::Number(x[u.case])));
                        row.extend([
                            Cell::Number(u.n),
                            Cell::Number(u.r),
                            Cell::Number(u.n * prob),
                            Cell::Number(u.r - u.n * prob),
                            Cell::Number(*prob),
                        ]);
                        row
                    })
                    .collect();
                let mut t = table("Cell Counts and Residuals", 0, columns, rows, caption(""));
                t.row_headers = 1 + usize::from(factor_text.is_some());
                tables.push(t);
            }

            let (t_crit, h) = match heterogeneity {
                true => (t_quantile(0.025, df), chi / df),
                false => (z, 1.0),
            };
            let back = |v: f64| command.log_base.map_or(v, |b| b.powf(v));
            let triple = |est: f64, lim: Option<(f64, f64)>| -> Vec<Cell> {
                vec![
                    Cell::Number(est),
                    number(lim.map(|l| l.0)),
                    number(lim.map(|l| l.1)),
                ]
            };
            let both_scales = |est: f64, lim: Option<(f64, f64)>| -> Vec<Cell> {
                let mut cells = triple(back(est), lim.map(|(a, b)| (back(a), back(b))));
                if log_scale.is_some() {
                    cells.extend(triple(est, lim));
                }
                cells
            };
            let x_name = names.first().cloned().unwrap_or_default();
            let scale_columns = |columns: &mut Vec<(String, String)>| {
                let group = format!("95% Confidence Limits for {x_name}");
                columns.extend(
                    ["Estimate", "Lower Bound", "Upper Bound"]
                        .map(|c| (group.clone(), c.to_string())),
                );
                if log_scale.is_some() {
                    let group = format!("95% Confidence Limits for log({x_name})");
                    columns.extend(
                        ["Estimate", "Lower Bound", "Upper Bound"]
                            .map(|c| (group.clone(), c.to_string())),
                    );
                }
            };
            let (vb, b) = (fit.cov[0][0], fit.theta[0]);
            if command.ci && single {
                let mut rows = Vec::new();
                for g in 0..q {
                    let (a, va, cab) = (fit.theta[p + g], fit.cov[p + g][p + g], fit.cov[p + g][0]);
                    for rate in rates() {
                        let num = link.transform(rate) - a;
                        let lim = fieller(num, b, va, vb, -cab, t_crit, h);
                        let mut row = Vec::new();
                        if factor_text.is_some() {
                            row.push(text(level_text(g)));
                        }
                        row.push(Cell::Number(rate));
                        row.extend(both_scales(num / b, lim));
                        rows.push(row);
                    }
                }
                let mut columns = Vec::new();
                if let Some(f) = &factor_text {
                    columns.push((String::new(), f.clone()));
                }
                columns.push((String::new(), "Probability".to_string()));
                scale_columns(&mut columns);
                tables.push(table(
                    "Confidence Limits",
                    1 + usize::from(factor_text.is_some()),
                    columns,
                    rows,
                    caption(&het_note),
                ));
            }
            if command.rmp && single && q > 1 {
                let mut rows = Vec::new();
                for j in 0..q {
                    for k in j + 1..q {
                        let (aj, ak) = (p + j, p + k);
                        let num = fit.theta[ak] - fit.theta[aj];
                        let v11 = fit.cov[aj][aj] + fit.cov[ak][ak] - 2.0 * fit.cov[aj][ak];
                        let c_nb = fit.cov[ak][0] - fit.cov[aj][0];
                        let lim = fieller(num, b, v11, vb, c_nb, t_crit, h);
                        let mut row = vec![text(level_text(j)), text(level_text(k))];
                        row.extend(both_scales(num / b, lim));
                        rows.push(row);
                    }
                }
                let f = factor_text.clone().unwrap_or_default();
                let mut columns = vec![
                    (String::new(), format!("{f} (first)")),
                    (String::new(), format!("{f} (second)")),
                ];
                scale_columns(&mut columns);
                tables.push(table(
                    "Relative Median Potency Estimates",
                    2,
                    columns,
                    rows,
                    caption(&format!(
                        " The median level of the first group against the second.{het_note}"
                    )),
                ));
            }
            if single {
                let groups = (0..q)
                    .map(|g| PointGroup {
                        name: if factor_text.is_some() {
                            level_text(g)
                        } else {
                            "Observed".into()
                        },
                        points: units
                            .iter()
                            .filter(|u| u.group == g && u.r > 0.0 && u.r < u.n)
                            .map(|u| Point {
                                x: u.x[0],
                                y: link.transform(u.r / u.n),
                                z: None,
                                label: None,
                            })
                            .collect(),
                    })
                    .collect();
                let x_title = match command.log_base {
                    Some(_) => format!("log({x_name})"),
                    None => x_name.clone(),
                };
                charts.push(Chart::plot(
                    ChartKind::Scatter,
                    format!("{model}: transformed response rates"),
                    Axis::titled(x_title),
                    Axis::titled(format!("{model} of the observed rate")),
                    Plot::Points {
                        groups,
                        lines: Vec::new(),
                        z_axis: None,
                    },
                ));
            }
        }
        for warning in warnings {
            self.warn(warning);
        }
        self.charts.extend(charts);
        Ok(tables)
    }
}

/// The Pearson chi-square of one factor level fitted on its own.
fn fit_alone(s: &Settings, units: &[Unit]) -> Option<f64> {
    if units.len() <= s.slopes + 1 {
        return Some(0.0);
    }
    fit(s, units)
        .ok()
        .map(|f| f.pearson(units))
        .filter(|c| c.is_finite())
}
