//! BAYES ONESAMPLE, BAYES RELATED, BAYES INDEPENDENT and BAYES CORRELATION: Bayes factors and
//! posterior distributions of a mean, a proportion, a Poisson rate, a mean difference and a
//! correlation. The linear models (BAYES ANOVA, REGRESSION, REPEATED) are in `models`.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - Case weights are frequencies rounded to whole numbers; a weight below 0.5 (or missing)
//!   drops the case. W is the sum of the rounded weights.
//! - One sample of a normal variable: t = (mean - null) / (sd / sqrt(W)) with W - 1 df. With a
//!   known variance BF01 compares the normal likelihood of the mean at the null value with the
//!   one where the mean is spread around it with variance g times the data variance over 1.
//!   Without it, a fixed g gives Rouder's closed form, and no g integrates g out against an
//!   inverse gamma (1/2, 1/2) prior, the JZS prior (a Cauchy prior on the effect size).
//! - The posterior of the mean: normal with a known variance (flat or normal prior on the
//!   mean); else a t whose degrees of freedom and scale come from the prior: flat mean and
//!   flat variance W - 3, 1/variance W - 1, 1/variance^2 W + 1; the conjugate priors (inverse
//!   chi-square, inverse gamma, gamma or chi-square on the precision, each with a normal mean
//!   prior worth KAPPA observations) the usual normal-gamma update. Intervals have equal tails.
//! - Binomial: BF01 is the ratio of the beta-binomial marginal likelihoods of the two beta
//!   priors; the posterior is Beta(a + y, b + W - y), with the mode at the edge or undefined
//!   when a shape parameter is at most 1. Poisson: the same with gamma priors (shape, rate)
//!   and the Gamma(a + y, b + W) posterior.
//! - Paired samples: the one-sample rules on the differences, first minus second.
//! - Two independent samples: the difference is group 2 minus group 1. Bayes factors from the
//!   pooled t with effective size W1 W2 / (W1 + W2): Rouder (JZS with a scale), Gonen (a normal
//!   prior on the effect size, through the noncentral t density) and the hyper-prior closed form
//!   (which gives BF10). Posteriors: known variances (normal), equal variances with a Jeffreys
//!   or inverse chi-square prior (t), else unequal variances by Patil's approximation to the
//!   Behrens-Fisher distribution.
//! - Correlation: BF10 of Wetzels and Wagenmakers under the JZS prior (BF01 is its inverse) or
//!   the fractional Bayes factor of Kang and others. The posterior density of rho is
//!   proportional to (1 - rho^2)^c (1 - rho^2)^((W-1)/2) / (1 - rho r)^(W - 3/2); the interval is
//!   the highest-density one. A correlation of 1 or -1 gets no Bayes factor.
//!
//! Where the manual approximates the posterior of rho by sampling, the mean, variance and
//! interval are computed here by quadrature of that density, so they need no seed. Integrals
//! use adaptive Gauss-Kronrod quadrature with the relative tolerance TOL and at most MAXITER
//! subintervals.

use crate::crosstabs::Key;
use crate::descriptives::{case_weights, number};
use crate::dist::{ln_gamma, solve, t_p};
use crate::distributions::evaluate;
use crate::means::is_valid;
use crate::random::{Random, time_seed};
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Values, Variable};
use statscraft_syntax::{
    Bayes, BayesCorrelation, BayesCriteria, BayesDistribution, BayesIndependent, BayesOneSample,
    BayesRelated, GroupSelect, GroupVariancePrior, IndependentFactor, MeanTest, NormalPrior,
    Success, Value, VariancePrior,
};
use std::collections::HashMap;
use std::f64::consts::PI;

mod models;

// ---------------------------------------------------------------------------------------------
// Quadrature

/// Abscissae of the 15-point Kronrod rule on [-1, 1], from the outside in.
const XGK: [f64; 8] = [
    0.991_455_371_120_812_6,
    0.949_107_912_342_758_5,
    0.864_864_423_359_769_1,
    0.741_531_185_599_394_4,
    0.586_087_235_467_691_1,
    0.405_845_151_377_397_2,
    0.207_784_955_007_898_5,
    0.0,
];
const WGK: [f64; 8] = [
    0.022_935_322_010_529_22,
    0.063_092_092_629_978_55,
    0.104_790_010_322_250_18,
    0.140_653_259_715_525_92,
    0.169_004_726_639_267_9,
    0.190_350_578_064_785_4,
    0.204_432_940_075_298_9,
    0.209_482_141_084_727_83,
];
/// The 7-point Gauss weights, at the odd Kronrod abscissae.
const WG: [f64; 4] = [
    0.129_484_966_168_869_7,
    0.279_705_391_489_276_7,
    0.381_830_050_505_118_9,
    0.417_959_183_673_469_4,
];

/// The Kronrod estimate of the integral of `f` over [a, b] and the error estimate.
fn kronrod(f: &dyn Fn(f64) -> f64, a: f64, b: f64) -> (f64, f64) {
    let (center, half) = ((a + b) / 2.0, (b - a) / 2.0);
    let fc = f(center);
    let (mut k, mut g) = (WGK[7] * fc, WG[3] * fc);
    for i in 0..7 {
        let pair = f(center - half * XGK[i]) + f(center + half * XGK[i]);
        k += WGK[i] * pair;
        if i % 2 == 1 {
            g += WG[i / 2] * pair;
        }
    }
    (k * half, ((k - g) * half).abs())
}

/// The integral of `f` over [a, b], halving the part with the largest error until the error
/// is below `tol` times the integral or there are `limit` parts.
fn adaptive(f: &dyn Fn(f64) -> f64, a: f64, b: f64, tol: f64, limit: usize) -> f64 {
    if a >= b {
        return 0.0;
    }
    let (k, e) = kronrod(f, a, b);
    let mut parts = vec![(a, b, k, e)];
    loop {
        let total: f64 = parts.iter().map(|p| p.2).sum();
        let error: f64 = parts.iter().map(|p| p.3).sum();
        if !total.is_finite()
            || error <= tol * total.abs()
            || error < 1e-300
            || parts.len() >= limit
        {
            return total;
        }
        let worst = (0..parts.len())
            .max_by(|&i, &j| parts[i].3.total_cmp(&parts[j].3))
            .unwrap_or(0);
        let (a, b, _, _) = parts.swap_remove(worst);
        let middle = (a + b) / 2.0;
        for (x, y) in [(a, middle), (middle, b)] {
            let (k, e) = kronrod(f, x, y);
            parts.push((x, y, k, e));
        }
    }
}

/// Where the mass of exp(`log_f`) lies: the largest log value, the peak and the range around
/// it beyond which the function is below e^-60 of the peak.
struct Peak {
    max: f64,
    low: f64,
    mode: f64,
    high: f64,
}

/// Finds the peak on a grid of 400 steps from `from` to `to`, refined by golden section.
fn locate(log_f: &dyn Fn(f64) -> f64, from: f64, to: f64) -> Option<Peak> {
    let step = (to - from) / 400.0;
    let grid: Vec<(f64, f64)> = (0..=400)
        .map(|i| {
            let x = from + step * i as f64;
            (x, log_f(x))
        })
        .collect();
    let best = (0..grid.len())
        .filter(|&i| !grid[i].1.is_nan())
        .max_by(|&i, &j| grid[i].1.total_cmp(&grid[j].1))?;
    if grid[best].1 == f64::NEG_INFINITY {
        return None;
    }
    let (mut a, mut b) = (grid[best].0 - step, grid[best].0 + step);
    let ratio = (5f64.sqrt() - 1.0) / 2.0;
    for _ in 0..100 {
        let (c, d) = (b - ratio * (b - a), a + ratio * (b - a));
        if log_f(c) >= log_f(d) {
            b = d;
        } else {
            a = c;
        }
    }
    let refined = ((a + b) / 2.0).clamp(from, to);
    let (mode, max) = if log_f(refined) > grid[best].1 {
        (refined, log_f(refined))
    } else {
        grid[best]
    };
    let kept: Vec<usize> = (0..grid.len())
        .filter(|&i| grid[i].1 > max - 60.0)
        .collect();
    let first = kept.first().copied().unwrap_or(best).saturating_sub(1);
    let last = (kept.last().copied().unwrap_or(best) + 1).min(grid.len() - 1);
    Some(Peak {
        max,
        low: grid[first].0.min(mode),
        mode,
        high: grid[last].0.max(mode),
    })
}

/// The integral of `g(x) exp(log_f(x) - peak.max)` over the peak's range.
fn around(peak: &Peak, f: &dyn Fn(f64) -> f64, tol: f64, limit: usize) -> f64 {
    adaptive(f, peak.low, peak.mode, tol, limit) + adaptive(f, peak.mode, peak.high, tol, limit)
}

/// The logarithm of the integral of exp(`log_f`) from `from` to `to`.
fn log_integral(log_f: &dyn Fn(f64) -> f64, from: f64, to: f64, tol: f64, limit: usize) -> f64 {
    let Some(peak) = locate(log_f, from, to) else {
        return f64::NEG_INFINITY;
    };
    let f = |x: f64| (log_f(x) - peak.max).exp();
    peak.max + around(&peak, &f, tol, limit).ln()
}

/// ln(1 + e^x) without overflow.
fn softplus(x: f64) -> f64 {
    if x > 0.0 {
        x + (-x).exp().ln_1p()
    } else {
        x.exp().ln_1p()
    }
}

/// ln(1 - tanh(z)^2), exact far into the tails.
fn ln_sech2(z: f64) -> f64 {
    let z = z.abs();
    2.0 * (2f64.ln() - z - (-2.0 * z).exp().ln_1p())
}

fn ln_beta(a: f64, b: f64) -> f64 {
    ln_gamma(a) + ln_gamma(b) - ln_gamma(a + b)
}

/// The settings the integrals need.
#[derive(Clone, Copy)]
struct Quadrature {
    tol: f64,
    limit: usize,
}

// ---------------------------------------------------------------------------------------------
// Bayes factors

/// ln of the t likelihood factor (1 + t^2 / ((1 + n g) df))^(-(df+1)/2) (1 + n g)^(-1/2).
fn t_factor(t: f64, df: f64, n: f64, g: f64) -> f64 {
    let spread = (n * g).ln_1p();
    -0.5 * spread - (df + 1.0) / 2.0 * (t * t / (spread.exp() * df)).ln_1p()
}

/// BF01 of a t-test with a fixed g.
fn bf_fixed_g(t: f64, df: f64, n: f64, g: f64) -> f64 {
    (t_factor(t, df, n, 0.0) - t_factor(t, df, n, g)).exp()
}

/// BF01 of a t-test with the JZS prior of scale `r`: g follows an inverse gamma (1/2, r^2/2).
/// `draws` asks for a Monte Carlo average over that many draws of g instead of quadrature.
fn bf_jzs(t: f64, df: f64, n: f64, r: f64, q: Quadrature, draws: Option<(usize, u64)>) -> f64 {
    let null = t_factor(t, df, n, 0.0);
    let alternative = match draws {
        Some((count, seed)) => {
            let mut random = Random::default();
            random.seed = seed;
            let mut sum = 0.0;
            for _ in 0..count {
                let z = random.normal();
                sum += t_factor(t, df, n, r * r / (z * z)).exp();
            }
            (sum / count as f64).ln()
        }
        None => {
            // in u = ln g, with the prior density times g
            let log_f = |u: f64| {
                t_factor(t, df, n, u.exp()) + r.ln()
                    - 0.5 * (2.0 * PI).ln()
                    - 0.5 * u
                    - r * r / (2.0 * u.exp())
            };
            log_integral(&log_f, -40.0, 60.0, q.tol, q.limit)
        }
    };
    (null - alternative).exp()
}

/// BF01 with a known variance: the mean `difference` from the null over the variance of the
/// data `variance`, `w` cases and the spread `g`.
fn bf_known(difference: f64, variance: f64, w: f64, g: f64) -> f64 {
    let shrink = w * g / (1.0 + w * g);
    ((1.0 + w * g).ln() / 2.0 - 0.5 * difference * difference * w / variance * shrink).exp()
}

/// BF10 of a correlation `r` from `w` cases under the JZS prior.
fn bf10_correlation(r: f64, w: f64, q: Quadrature) -> f64 {
    let log_f = |u: f64| {
        (w - 2.0) / 2.0 * softplus(u)
            - (w - 1.0) / 2.0 * ((1.0 - r * r) * u.exp()).ln_1p()
            - 0.5 * u
            - w / (2.0 * u.exp())
    };
    let integral = log_integral(&log_f, -40.0, 60.0, q.tol, q.limit);
    (0.5 * (w / 2.0).ln() - 0.5 * PI.ln() + integral).exp()
}

/// ln of the integral over V of the fractional Bayes factor at `rho` with exponent `n` - 1.
/// With V = e^u the integrand is (2 cosh(u/2) - 2 r rho)^-(n-1), even in u.
fn ln_fraction_inner(r: f64, rho: f64, n: f64, q: Quadrature) -> f64 {
    let base = (2.0 - 2.0 * r * rho).ln();
    let log_f = |u: f64| {
        let u = u.abs();
        // ln(2 cosh(u/2) - 2 r rho), kept finite for large u
        let level = u / 2.0 + ((-u).exp() + 1.0 - 2.0 * r * rho * (-u / 2.0).exp()).ln();
        -(n - 1.0) * (level - base)
    };
    let end = 2.0 * (60.0 / (n - 1.0) + 2.0);
    let f = |u: f64| log_f(u).exp();
    -(n - 1.0) * base + (2.0 * adaptive(&f, 0.0, end, q.tol, q.limit)).ln()
}

/// ln I1 and ln I2 of the fractional Bayes factor with exponent `n`.
fn fraction_parts(r: f64, rho0: f64, n: f64, q: Quadrature) -> (f64, f64) {
    let i1 = (n - 1.0) / 2.0 * (1.0 - rho0 * rho0).ln() + ln_fraction_inner(r, rho0, n, q);
    // over rho = tanh(z): (1 - rho^2)^((n-3)/2) d rho = (1 - rho^2)^((n-1)/2) dz
    let log_f = |z: f64| (n - 1.0) / 2.0 * ln_sech2(z) + ln_fraction_inner(r, z.tanh(), n, q);
    let reach = 20.0 + 60.0 / (n - 1.0);
    (i1, log_integral(&log_f, -reach, reach, q.tol, q.limit))
}

/// The fractional BF01 of the null correlation `rho0`, with the fraction `b` of `w` cases.
fn bf01_fraction(r: f64, w: f64, b: f64, rho0: f64, q: Quadrature) -> f64 {
    let (full1, full2) = fraction_parts(r, rho0, w, q);
    let (part1, part2) = fraction_parts(r, rho0, b * w, q);
    (full1 + part2 - full2 - part1).exp()
}

// ---------------------------------------------------------------------------------------------
// Posteriors

/// Mode, mean, variance and the interval of a posterior; NaN where not defined.
#[derive(Clone, Copy)]
struct Posterior {
    mode: f64,
    mean: f64,
    variance: f64,
    lower: f64,
    upper: f64,
}

const NONE: Posterior = Posterior {
    mode: f64::NAN,
    mean: f64::NAN,
    variance: f64::NAN,
    lower: f64::NAN,
    upper: f64::NAN,
};

fn idf(name: &str, p: f64, parameters: &[f64]) -> f64 {
    let mut arguments = vec![p];
    arguments.extend_from_slice(parameters);
    evaluate(name, &arguments).unwrap_or(f64::NAN)
}

fn normal_posterior(mean: f64, variance: f64, alpha: f64) -> Posterior {
    let sd = variance.sqrt();
    Posterior {
        mode: mean,
        mean,
        variance,
        lower: idf("IDF.NORMAL", alpha / 2.0, &[mean, sd]),
        upper: idf("IDF.NORMAL", 1.0 - alpha / 2.0, &[mean, sd]),
    }
}

/// A t posterior with `df` degrees of freedom, location `center` and squared scale `scale2`.
fn t_posterior(df: f64, center: f64, scale2: f64, alpha: f64) -> Posterior {
    if !(df > 0.0 && scale2 > 0.0) {
        return NONE;
    }
    let half = idf("IDF.T", 1.0 - alpha / 2.0, &[df]) * scale2.sqrt();
    Posterior {
        mode: center,
        mean: center,
        variance: if df > 2.0 {
            df / (df - 2.0) * scale2
        } else {
            f64::NAN
        },
        lower: center - half,
        upper: center + half,
    }
}

/// The posterior of a normal mean from `w` cases with mean `mean` and sum of squared
/// deviations `ss`.
fn mean_posterior(
    w: f64,
    mean: f64,
    ss: f64,
    known: Option<f64>,
    prior: &NormalPrior,
    alpha: f64,
) -> Posterior {
    if let Some(variance) = known {
        if w <= 0.0 {
            return NONE;
        }
        return match prior.mean {
            Some((m0, Some(v0))) => {
                let v = 1.0 / (1.0 / v0 + w / variance);
                normal_posterior(v * (m0 / v0 + w * mean / variance), v, alpha)
            }
            _ => normal_posterior(mean, variance / w, alpha),
        };
    }
    let flat = |df: f64| t_posterior(df, mean, ss / (w * df), alpha);
    // the shape and rate of the gamma prior on the precision
    let conjugate = match prior.variance {
        VariancePrior::Diffuse => return flat(w - 3.0),
        VariancePrior::Jeffreys2 => return flat(w - 1.0),
        VariancePrior::Jeffreys4 => return flat(w + 1.0),
        VariancePrior::InvChiSquare(df, scale) => (df / 2.0, df * scale / 2.0),
        // the second value of both enters the posterior as the rate of the precision
        VariancePrior::InvGamma(shape, b) | VariancePrior::Gamma(shape, b) => (shape, b),
        VariancePrior::ChiSquare(df) => (df / 2.0, 0.5),
    };
    let Some((m0, _)) = prior.mean else {
        // a conjugate variance prior needs the normal mean prior; else the flat priors
        return flat(w - 3.0);
    };
    let (a0, b0) = conjugate;
    let kappa = prior.kappa + w;
    let center = (prior.kappa * m0 + w * mean) / kappa;
    let a = a0 + w / 2.0;
    let b = b0 + ss / 2.0 + prior.kappa * w * (mean - m0).powi(2) / (2.0 * kappa);
    t_posterior(2.0 * a, center, b / (a * kappa), alpha)
}

fn beta_posterior(a: f64, b: f64, alpha: f64) -> Posterior {
    if !(a > 0.0 && b > 0.0) {
        return NONE;
    }
    let mode = if a > 1.0 && b > 1.0 {
        (a - 1.0) / (a + b - 2.0)
    } else if (a < 1.0 && b >= 1.0) || (a == 1.0 && b > 1.0) {
        0.0
    } else if (a >= 1.0 && b < 1.0) || (a > 1.0 && b == 1.0) {
        1.0
    } else {
        // two modes at 0 and 1, or a flat posterior
        f64::NAN
    };
    Posterior {
        mode,
        mean: a / (a + b),
        variance: a * b / ((a + b).powi(2) * (a + b + 1.0)),
        lower: idf("IDF.BETA", alpha / 2.0, &[a, b]),
        upper: idf("IDF.BETA", 1.0 - alpha / 2.0, &[a, b]),
    }
}

fn gamma_posterior(shape: f64, rate: f64, alpha: f64) -> Posterior {
    if !(shape > 0.0 && rate > 0.0) {
        return NONE;
    }
    Posterior {
        mode: if shape >= 1.0 {
            (shape - 1.0) / rate
        } else {
            f64::NAN
        },
        mean: shape / rate,
        variance: shape / (rate * rate),
        lower: idf("IDF.GAMMA", alpha / 2.0, &[shape, rate]),
        upper: idf("IDF.GAMMA", 1.0 - alpha / 2.0, &[shape, rate]),
    }
}

/// The posterior of a correlation `r` from `w` cases with the prior (1 - rho^2)^c, with the
/// highest-density interval.
fn correlation_posterior(r: f64, w: f64, c: f64, alpha: f64, q: Quadrature) -> Posterior {
    let a = c + (w - 1.0) / 2.0;
    let power = w - 1.5;
    if a <= -1.0 || r.abs() >= 1.0 {
        return NONE;
    }
    // the log density over rho, and over z = atanh(rho) with the Jacobian 1 - rho^2
    let log_rho = |rho: f64, sech2: f64| a * sech2 - power * (1.0 - rho * r).ln();
    let log_z = |z: f64| {
        let sech2 = ln_sech2(z);
        log_rho(z.tanh(), sech2) + sech2
    };
    let reach = 20.0 + 60.0 / (a + 1.0);
    let Some(peak) = locate(&log_z, -reach, reach) else {
        return NONE;
    };
    let density = |z: f64| (log_z(z) - peak.max).exp();
    let total = around(&peak, &density, q.tol, q.limit);
    let moment = |k: i32| {
        around(
            &peak,
            &|z: f64| z.tanh().powi(k) * density(z),
            q.tol,
            q.limit,
        )
    };
    let mean = moment(1) / total;
    let variance = moment(2) / total - mean * mean;
    let mode = if a > 0.0 {
        solve(
            |rho| -2.0 * a * rho * (1.0 - rho * r) + power * r * (1.0 - rho * rho),
            -1.0,
            1.0,
        )
    } else {
        f64::NAN
    };
    let (mut lower, mut upper) = (f64::NAN, f64::NAN);
    if mode.is_finite() {
        // the density over rho at z, relative to its value at the mode
        let z_mode = mode.atanh();
        let top = log_rho(mode, (1.0 - mode * mode).ln());
        let relative = |z: f64| log_rho(z.tanh(), ln_sech2(z)) - top;
        let cdf = |z: f64| adaptive(&density, peak.low, z, q.tol, q.limit) / total;
        // the end of the region where the density is above e^level of its top, between the
        // mode and `bound`: the bound itself when the density there is still above
        let end = |level: f64, bound: f64| {
            if relative(bound) >= level {
                bound
            } else {
                solve(|z| relative(z) - level, bound, z_mode)
            }
        };
        let ends = |level: f64| (end(level, peak.low), end(level, peak.high));
        let level = solve(
            |level| {
                let (low, high) = ends(level);
                cdf(high) - cdf(low) - (1.0 - alpha)
            },
            -60.0,
            0.0,
        );
        let (low, high) = ends(level);
        (lower, upper) = (low.tanh(), high.tanh());
    }
    Posterior {
        mode,
        mean,
        variance,
        lower,
        upper,
    }
}

// ---------------------------------------------------------------------------------------------
// Data

struct Run<'d> {
    data: &'d Dataset,
    weights: Option<Vec<f64>>,
    shown: Shown,
    warnings: Vec<String>,
    charts: Vec<crate::chart::Chart>,
    alpha: f64,
    ci: String,
    quadrature: Quadrature,
}

impl Run<'_> {
    fn new<'d>(
        engine: &Engine,
        data: &'d Dataset,
        criteria: &BayesCriteria,
    ) -> Result<Run<'d>, Error> {
        let percent = 100.0 * criteria.ci;
        let shown = if (percent - percent.round()).abs() < 1e-9 {
            format!("{percent:.0}")
        } else {
            format!("{}", (percent * 1e6).round() / 1e6)
        };
        Ok(Run {
            data,
            weights: case_weights(data)?,
            shown: engine.shown(),
            warnings: Vec::new(),
            charts: Vec::new(),
            alpha: 1.0 - criteria.ci,
            ci: format!("{shown}% Credible Interval"),
            quadrature: Quadrature {
                tol: criteria.tol,
                limit: criteria.maxiter,
            },
        })
    }

    /// The rounded frequency weight of a case: 0 below 0.5.
    fn weight(&self, case: usize) -> f64 {
        let w = self.weights.as_ref().map_or(1.0, |w| w[case]);
        if w < 0.5 { 0.0 } else { w.round() }
    }

    /// The cases that count and are valid on every variable of `variables`.
    fn cases(&self, variables: &[&Variable]) -> Vec<usize> {
        (0..self.data.case_count())
            .filter(|&c| self.weight(c) > 0.0 && variables.iter().all(|v| is_valid(v, c, false)))
            .collect()
    }

    fn label(&self, variable: &Variable) -> Cell {
        Cell::Text(self.shown.variable(variable))
    }
}

fn numeric(variable: &Variable, case: usize) -> f64 {
    match variable.values() {
        Values::Numeric(values) => values.get(case).unwrap_or(f64::NAN),
        Values::String(_) => f64::NAN,
    }
}

fn text(variable: &Variable, case: usize) -> String {
    match variable.values() {
        Values::String(values) => values.get(case).trim_end().to_string(),
        Values::Numeric(_) => String::new(),
    }
}

/// Weighted count, mean and sum of squared deviations.
#[derive(Clone, Copy)]
struct Moments {
    w: f64,
    mean: f64,
    ss: f64,
}

impl Moments {
    fn of(pairs: impl Iterator<Item = (f64, f64)> + Clone) -> Moments {
        let w: f64 = pairs.clone().map(|(_, w)| w).sum();
        let mean = pairs.clone().map(|(x, w)| x * w).sum::<f64>() / w;
        let ss = pairs.map(|(x, w)| w * (x - mean).powi(2)).sum();
        Moments { w, mean, ss }
    }

    fn variance(&self) -> f64 {
        self.ss / (self.w - 1.0)
    }
}

fn table(title: &str, columns: &[&str], rows: Vec<Vec<Cell>>) -> Table {
    Table {
        title: title.into(),
        row_headers: columns.iter().take_while(|c| c.is_empty()).count(),
        columns: columns.iter().map(|c| c.to_string()).collect(),
        rows,
        ..Table::default()
    }
}

/// The columns of a posterior table after `lead` label columns and `before` others.
fn posterior_table(run: &Run, title: &str, columns: &[&str], rows: Vec<Vec<Cell>>) -> Table {
    let mut all = columns.to_vec();
    all.extend(["Mode", "Mean", "Variance", "Lower", "Upper"]);
    let mut t = table(title, &all, rows);
    t.column_groups = (0..all.len())
        .map(|i| {
            if i + 2 >= all.len() {
                vec![run.ci.clone()]
            } else {
                Vec::new()
            }
        })
        .collect();
    t
}

fn posterior_cells(p: &Posterior) -> Vec<Cell> {
    [p.mode, p.mean, p.variance, p.lower, p.upper]
        .map(|v| number(Some(v)))
        .to_vec()
}

const BF01: &str =
    "Bayes factor BF01: the evidence for the null hypothesis against the alternative.";

// ---------------------------------------------------------------------------------------------
// The commands

impl Engine {
    pub(crate) fn bayes(&mut self, command: &Bayes) -> Result<Vec<Table>, Error> {
        let (tables, warnings, charts) = match command {
            Bayes::OneSample(c) => self.bayes_onesample(c)?,
            Bayes::Related(c) => self.bayes_related(c)?,
            Bayes::Independent(c) => self.bayes_independent(c)?,
            Bayes::Correlation(c) => self.bayes_correlation(c)?,
            Bayes::Anova(c) => self.bayes_anova(c)?,
            Bayes::Regression(c) => {
                let (output, saved) = self.bayes_regression(c)?;
                self.save_predictions(saved)?;
                output
            }
            Bayes::Repeated(c) => self.bayes_repeated(c)?,
        };
        for warning in warnings {
            self.warn(warning);
        }
        self.charts.extend(charts);
        Ok(tables)
    }

    /// A known variance needs both values of MEANDIST=NORMAL.
    /// A known variance needs both values of MEANDIST=NORMAL. With an unknown variance and a
    /// flat or Jeffreys variance prior the normal mean prior is not used: a warning says so.
    fn check_mean_prior(
        run: &mut Run,
        prior: &NormalPrior,
        tests: &[&MeanTest],
        unknown: bool,
    ) -> Result<(), Error> {
        if matches!(prior.mean, Some((_, None))) && tests.iter().any(|t| t.variance.is_some()) {
            return Err(Error(
                "with a known VARIANCE, MEANDIST=NORMAL needs both the mean and the variance"
                    .into(),
            ));
        }
        let flat = matches!(
            prior.variance,
            VariancePrior::Diffuse | VariancePrior::Jeffreys2 | VariancePrior::Jeffreys4
        );
        if unknown && flat && prior.mean.is_some() {
            run.warnings.push(
                "MEANDIST=NORMAL is used only with a known variance or a conjugate VARDIST or PRECDIST; the flat mean prior is used".into(),
            );
        }
        Ok(())
    }

    fn variables(&self, names: &[String]) -> Result<Vec<Variable<'_>>, Error> {
        let data = self.active()?;
        names.iter().map(|n| Ok(data.variable(n)?)).collect()
    }

    fn bayes_onesample(&self, c: &BayesOneSample) -> Result<models::Output, Error> {
        let data = self.active()?;
        let mut run = Run::new(self, data, &c.criteria)?;
        let names = self.expand(&c.variables)?;
        let variables = self.variables(&names)?;
        let kind = match c.distribution {
            BayesDistribution::Normal => "NORMAL",
            BayesDistribution::Binomial => "BINOMIAL",
            BayesDistribution::Poisson => "POISSON",
        };
        if c.distribution != BayesDistribution::Binomial
            && let Some(v) = variables.iter().find(|v| v.is_string())
        {
            return Err(Error(format!(
                "BAYES ONESAMPLE DISTRIBUTION={kind} needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        // the DATA settings of each variable
        let mut settings: HashMap<String, usize> = HashMap::new();
        for (i, d) in c.data.iter().enumerate() {
            for name in self.expand(&d.variables)? {
                if !names.contains(&name) {
                    return Err(Error(format!(
                        "BAYES ONESAMPLE /DATA names {name}, which is not on /INFERENCE VARIABLES"
                    )));
                }
                if settings.insert(name.clone(), i).is_some() {
                    return Err(Error(format!(
                        "BAYES ONESAMPLE names {name} on more than one DATA subcommand"
                    )));
                }
            }
        }
        let default_test = MeanTest {
            variance: None,
            null: 0.0,
            g: None,
        };
        if c.distribution != BayesDistribution::Normal {
            if c.analysis.factor()
                && let Some(v) = variables.iter().find(|v| {
                    settings
                        .get(v.name())
                        .is_none_or(|&i| c.data[i].alter_prior.is_none())
                })
            {
                return Err(Error(format!(
                    "a Bayes factor of {} needs ALTERPRIORPARAM on a DATA subcommand",
                    v.name()
                )));
            }
            if c.analysis.posterior() && c.prior.conjugate.is_none() {
                return Err(Error(format!(
                    "the posterior of DISTRIBUTION={kind} needs /PRIOR TYPE=CONJUGATE(a b)"
                )));
            }
        }
        let tests: Vec<&MeanTest> = c.data.iter().map(|d| &d.test).collect();
        if c.distribution == BayesDistribution::Normal && c.analysis.posterior() {
            let unknown = names.iter().any(|n| {
                settings
                    .get(n)
                    .is_none_or(|&i| c.data[i].test.variance.is_none())
            });
            Self::check_mean_prior(&mut run, &c.prior, &tests, unknown)?;
        }
        let all: Vec<&Variable> = variables.iter().collect();
        let complete = run.cases(&all);
        let seed = c.criteria.seed.unwrap_or_else(time_seed);
        let draws = c
            .criteria
            .monte_carlo
            .then_some((c.criteria.mcsamples, seed));
        let (mut factors, mut posteriors) = (Vec::new(), Vec::new());
        for variable in &variables {
            let cases = if c.listwise {
                complete.clone()
            } else {
                run.cases(&[variable])
            };
            let setting = settings.get(variable.name()).map(|&i| &c.data[i]);
            let label = run.label(variable);
            match c.distribution {
                BayesDistribution::Normal => {
                    let test = setting.map_or(&default_test, |d| &d.test);
                    let pairs = cases.iter().map(|&k| (numeric(variable, k), run.weight(k)));
                    let m = Moments::of(pairs);
                    let (factor, posterior) =
                        normal_rows(&mut run, label, m, test, &c.prior, draws);
                    factors.push(factor);
                    posteriors.push(posterior);
                }
                BayesDistribution::Binomial => {
                    let success = setting
                        .and_then(|d| d.success.clone())
                        .unwrap_or(Success::Last);
                    let is_success = successes(variable, &cases, &success)?;
                    let n: f64 = cases.iter().map(|&k| run.weight(k)).sum();
                    let y: f64 = cases
                        .iter()
                        .zip(&is_success)
                        .filter(|(_, s)| **s)
                        .map(|(&k, _)| run.weight(k))
                        .sum();
                    let (a0, b0) = setting.and_then(|d| d.null_prior).unwrap_or((2.0, 2.0));
                    if let Some((a1, b1)) = setting.and_then(|d| d.alter_prior) {
                        let bf = ln_beta(a0 + y, b0 + n - y) + ln_beta(a1, b1)
                            - ln_beta(a1 + y, b1 + n - y)
                            - ln_beta(a0, b0);
                        factors.push(vec![
                            label.clone(),
                            Cell::Number(n),
                            Cell::Number(y),
                            number(Some(y / n)),
                            number(Some(bf.exp())),
                        ]);
                    }
                    if let Some((a, b)) = c.prior.conjugate {
                        let p = if n >= 1.0 {
                            beta_posterior(a + y, b + n - y, run.alpha)
                        } else {
                            NONE
                        };
                        let mut row = vec![label, Cell::Number(n)];
                        row.extend(posterior_cells(&p));
                        posteriors.push(row);
                    }
                }
                BayesDistribution::Poisson => {
                    let counts = cases.iter().map(|&k| numeric(variable, k));
                    if counts.clone().any(|x| x < 0.0 || x.fract() != 0.0) {
                        run.warnings.push(format!(
                            "{} has values that are not counts (whole numbers 0 or above); it is left out",
                            variable.name()
                        ));
                        continue;
                    }
                    let n: f64 = cases.iter().map(|&k| run.weight(k)).sum();
                    let y: f64 = cases
                        .iter()
                        .map(|&k| run.weight(k) * numeric(variable, k))
                        .sum();
                    let (a0, b0) = setting.and_then(|d| d.null_prior).unwrap_or((2.0, 2.0));
                    if let Some((a1, b1)) = setting.and_then(|d| d.alter_prior) {
                        let bf = a0 * b0.ln()
                            + (a1 + y) * (b1 + n).ln()
                            + ln_gamma(a0 + y)
                            + ln_gamma(a1)
                            - a1 * b1.ln()
                            - (a0 + y) * (b0 + n).ln()
                            - ln_gamma(a1 + y)
                            - ln_gamma(a0);
                        factors.push(vec![
                            label.clone(),
                            Cell::Number(n),
                            Cell::Number(y),
                            number(Some(y / n)),
                            number(Some(bf.exp())),
                        ]);
                    }
                    if let Some((a, b)) = c.prior.conjugate {
                        let p = gamma_posterior(a + y, b + n, run.alpha);
                        let mut row = vec![label, Cell::Number(n)];
                        row.extend(posterior_cells(&p));
                        posteriors.push(row);
                    }
                }
            }
        }
        let mut out = Vec::new();
        let (factor_title, posterior_title, columns): (&str, &str, &[&str]) = match c.distribution {
            BayesDistribution::Normal => (
                "Bayes Factor for a One-Sample Mean",
                "Posterior Distribution of the Mean",
                &[
                    "",
                    "Test Value",
                    "N",
                    "Mean",
                    "Std. Deviation",
                    "Std. Error Mean",
                    "Bayes Factor",
                    "t",
                    "df",
                    "Sig. (2-tailed)",
                ],
            ),
            BayesDistribution::Binomial => (
                "Bayes Factor for a Binomial Proportion",
                "Posterior Distribution of the Proportion",
                &["", "N", "Successes", "Proportion", "Bayes Factor"],
            ),
            BayesDistribution::Poisson => (
                "Bayes Factor for a Poisson Rate",
                "Posterior Distribution of the Rate",
                &["", "N", "Total", "Mean", "Bayes Factor"],
            ),
        };
        if c.analysis.factor() {
            let mut t = table(factor_title, columns, factors);
            t.caption = Some(BF01.into());
            out.push(t);
        }
        if c.analysis.posterior() {
            out.push(posterior_table(
                &run,
                posterior_title,
                &["", "N"],
                posteriors,
            ));
        }
        Ok((out, run.warnings, run.charts))
    }

    fn bayes_related(&self, c: &BayesRelated) -> Result<models::Output, Error> {
        let data = self.active()?;
        let mut run = Run::new(self, data, &c.criteria)?;
        let mut pairs = Vec::new();
        for pair in &c.pairs {
            let (a, b) = (data.variable(&pair.first)?, data.variable(&pair.second)?);
            for v in [&a, &b] {
                if v.is_string() {
                    return Err(Error(format!(
                        "BAYES RELATED needs numeric variables, but {} is a string variable",
                        v.name()
                    )));
                }
            }
            pairs.push((a, b, &pair.test));
        }
        let tests: Vec<&MeanTest> = c.pairs.iter().map(|p| &p.test).collect();
        if c.analysis.posterior() {
            let unknown = tests.iter().any(|t| t.variance.is_none());
            Self::check_mean_prior(&mut run, &c.prior, &tests, unknown)?;
        }
        let all: Vec<&Variable> = pairs.iter().flat_map(|(a, b, _)| [a, b]).collect();
        let complete = run.cases(&all);
        let seed = c.criteria.seed.unwrap_or_else(time_seed);
        let draws = c
            .criteria
            .monte_carlo
            .then_some((c.criteria.mcsamples, seed));
        let (mut factors, mut posteriors) = (Vec::new(), Vec::new());
        for (a, b, test) in &pairs {
            let cases = if c.listwise {
                complete.clone()
            } else {
                run.cases(&[a, b])
            };
            let differences = cases
                .iter()
                .map(|&k| (numeric(a, k) - numeric(b, k), run.weight(k)));
            let m = Moments::of(differences);
            let label = Cell::Text(format!(
                "{} - {}",
                run.shown.variable(a),
                run.shown.variable(b)
            ));
            let (factor, posterior) = normal_rows(&mut run, label, m, test, &c.prior, draws);
            factors.push(factor);
            posteriors.push(posterior);
        }
        let mut out = Vec::new();
        if c.analysis.factor() {
            let columns = [
                "",
                "Test Value",
                "N",
                "Mean Difference",
                "Std. Deviation",
                "Std. Error Mean",
                "Bayes Factor",
                "t",
                "df",
                "Sig. (2-tailed)",
            ];
            let mut t = table("Bayes Factor for Related Samples", &columns, factors);
            t.caption = Some(BF01.into());
            out.push(t);
        }
        if c.analysis.posterior() {
            let title = "Posterior Distribution of the Mean Difference";
            out.push(posterior_table(&run, title, &["", "N"], posteriors));
        }
        Ok((out, run.warnings, run.charts))
    }

    fn bayes_independent(&self, c: &BayesIndependent) -> Result<models::Output, Error> {
        let data = self.active()?;
        let mut run = Run::new(self, data, &c.criteria)?;
        let names = self.expand(&c.variables)?;
        let variables = self.variables(&names)?;
        if let Some(v) = variables.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "BAYES INDEPENDENT needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let group = data.variable(&c.group)?;
        let mut all: Vec<&Variable> = variables.iter().collect();
        all.push(&group);
        let complete = run.cases(&all);
        // which group a case falls in: 1, 2 or none
        let split = match &c.select {
            GroupSelect::Levels(a, b) => {
                let matches = |value: &Value, case: usize| match (value, group.is_string()) {
                    (Value::String(s), true) => text(&group, case) == s.trim_end(),
                    (Value::Number(n), false) => numeric(&group, case) == *n,
                    (Value::String(s), false) => s.parse::<f64>() == Ok(numeric(&group, case)),
                    (Value::Number(n), true) => text(&group, case) == n.to_string(),
                };
                let (a, b) = (a.clone(), b.clone());
                Box::new(move |case: usize| {
                    if matches(&a, case) {
                        Some(0usize)
                    } else if matches(&b, case) {
                        Some(1)
                    } else {
                        None
                    }
                }) as Box<dyn Fn(usize) -> Option<usize> + '_>
            }
            select => {
                if group.is_string() {
                    return Err(Error(format!(
                        "SELECT=MIDPOINT and CUTPOINT need a numeric grouping variable, but {} is a string variable",
                        group.name()
                    )));
                }
                let cut = match select {
                    GroupSelect::Cutpoint(cut) => *cut,
                    _ => {
                        let values = run.cases(&[&group]).into_iter().map(|k| numeric(&group, k));
                        let low = values.clone().fold(f64::INFINITY, f64::min);
                        let high = values.fold(f64::NEG_INFINITY, f64::max);
                        (low + high) / 2.0
                    }
                };
                Box::new(move |case: usize| Some(usize::from(numeric(&group, case) >= cut)))
                    as Box<dyn Fn(usize) -> Option<usize> + '_>
            }
        };
        let group_labels: [String; 2] = match &c.select {
            GroupSelect::Levels(a, b) => [a, b].map(|v| {
                let key = match v {
                    Value::Number(n) => Key::Number(*n),
                    Value::String(s) => Key::Text(s.trim_end().to_string()),
                };
                run.shown.value(&group, &key)
            }),
            GroupSelect::Cutpoint(cut) => [format!("< {cut}"), format!(">= {cut}")],
            GroupSelect::Midpoint => [
                "Below the midpoint".into(),
                "At or above the midpoint".into(),
            ],
        };
        let (mut statistics, mut factors, mut posteriors) = (Vec::new(), Vec::new(), Vec::new());
        let q = run.quadrature;
        for variable in &variables {
            let cases = if c.listwise {
                complete.clone()
            } else {
                run.cases(&[variable, &group])
            };
            let mut groups = [Vec::new(), Vec::new()];
            for &k in &cases {
                if let Some(g) = split(k) {
                    groups[g].push((numeric(variable, k), run.weight(k)));
                }
            }
            let label = run.label(variable);
            if groups.iter().any(|g| g.is_empty()) {
                run.warnings.push(format!(
                    "{} is left out: at least one of the two groups has no valid cases",
                    variable.name()
                ));
                continue;
            }
            let [x, y] = groups.map(|g| Moments::of(g.into_iter()));
            for (m, name) in [(x, &group_labels[0]), (y, &group_labels[1])] {
                let sd = m.variance().sqrt();
                statistics.push(vec![
                    label.clone(),
                    Cell::Text(name.clone()),
                    Cell::Number(m.w),
                    number(Some(m.mean)),
                    number(Some(sd)),
                    number(Some(sd / m.w.sqrt())),
                ]);
            }
            let difference = y.mean - x.mean;
            let df = x.w + y.w - 2.0;
            let pooled = (x.ss + y.ss) / df;
            let se = (pooled * (1.0 / x.w + 1.0 / y.w)).sqrt();
            let t = difference / se;
            let n = x.w * y.w / (x.w + y.w);
            let enough = x.w > 1.0 && y.w > 1.0 && se > 0.0;
            let bf = if !enough {
                f64::NAN
            } else {
                match c.factor {
                    IndependentFactor::Rouder(r) => bf_jzs(t, df, n, r, q, None),
                    IndependentFactor::Gonen(mean, variance) => {
                        let spread = (1.0 + n * variance).sqrt();
                        let null = evaluate("PDF.T", &[t, df]).unwrap_or(f64::NAN);
                        let alternative =
                            evaluate("NPDF.T", &[t / spread, df, n.sqrt() * mean / spread])
                                .unwrap_or(f64::NAN);
                        null * spread / alternative
                    }
                    IndependentFactor::Hyper(a) => (ln_gamma(df / 2.0) + ln_gamma(a + 1.5)
                        - ln_gamma((df + 1.0) / 2.0)
                        - ln_gamma(a + 1.0)
                        + (df - 2.0 * a - 2.0) / 2.0 * (t * t / df).ln_1p())
                    .exp(),
                }
            };
            factors.push(vec![
                label.clone(),
                number(Some(difference)),
                number(Some(se)),
                number(Some(bf)),
                number(Some(t)),
                number(Some(df)),
                number(Some(t_p(t, df))),
            ]);
            if c.analysis.posterior() {
                let p = independent_posterior(&mut run, c, variable, x, y);
                let mut row = vec![label];
                row.extend(posterior_cells(&p));
                posteriors.push(row);
            }
        }
        let mut out = vec![table(
            "Group Statistics",
            &["", "", "N", "Mean", "Std. Deviation", "Std. Error Mean"],
            statistics,
        )];
        if c.analysis.factor() {
            let columns = [
                "",
                "Mean Difference",
                "Pooled Std. Error Difference",
                "Bayes Factor",
                "t",
                "df",
                "Sig. (2-tailed)",
            ];
            let mut t = table("Bayes Factor for Independent Samples", &columns, factors);
            t.caption = Some(match c.factor {
                IndependentFactor::Hyper(_) => {
                    "Bayes factor BF10: the evidence for the alternative hypothesis against the null. The difference is group 2 minus group 1.".into()
                }
                _ => format!("{BF01} The difference is group 2 minus group 1."),
            });
            out.push(t);
        }
        if c.analysis.posterior() {
            let title = "Posterior Distribution of the Mean Difference";
            let mut t = posterior_table(&run, title, &[""], posteriors);
            t.caption = Some("The difference is group 2 minus group 1.".into());
            out.push(t);
        }
        Ok((out, run.warnings, run.charts))
    }

    fn bayes_correlation(&self, c: &BayesCorrelation) -> Result<models::Output, Error> {
        let data = self.active()?;
        let mut run = Run::new(self, data, &c.criteria)?;
        let names = self.expand(&c.variables)?;
        let variables = self.variables(&names)?;
        if variables.len() < 2 {
            return Err(Error(
                "BAYES CORRELATION needs at least two variables".into(),
            ));
        }
        if let Some(v) = variables.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "BAYES CORRELATION needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let all: Vec<&Variable> = variables.iter().collect();
        let complete = run.cases(&all);
        let q = run.quadrature;
        let (mut factors, mut posteriors) = (Vec::new(), Vec::new());
        let mut plotted = 0;
        for (i, x) in variables.iter().enumerate() {
            for y in &variables[i + 1..] {
                let cases = if c.listwise {
                    complete.clone()
                } else {
                    run.cases(&[x, y])
                };
                let w: f64 = cases.iter().map(|&k| run.weight(k)).sum();
                let mean = |v: &Variable| {
                    cases
                        .iter()
                        .map(|&k| run.weight(k) * numeric(v, k))
                        .sum::<f64>()
                        / w
                };
                let (mx, my) = (mean(x), mean(y));
                let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
                for &k in &cases {
                    let (dx, dy, wk) = (numeric(x, k) - mx, numeric(y, k) - my, run.weight(k));
                    sxx += wk * dx * dx;
                    syy += wk * dy * dy;
                    sxy += wk * dx * dy;
                }
                let r = sxy / (sxx * syy).sqrt();
                let labels = [run.label(x), run.label(y)];
                let usable = w >= 2.0 && r.is_finite();
                if !usable {
                    run.warnings.push(format!(
                        "no inference on the correlation of {} and {}: it needs two cases and some spread in both",
                        x.name(),
                        y.name()
                    ));
                }
                let perfect = usable && r.abs() >= 1.0;
                if perfect && c.analysis.factor() {
                    run.warnings.push(format!(
                        "{} and {} correlate perfectly; there is no Bayes factor",
                        x.name(),
                        y.name()
                    ));
                }
                let bf = match c.fraction {
                    _ if !usable || perfect => f64::NAN,
                    None => 1.0 / bf10_correlation(r, w, q),
                    Some(b) if b * w > 1.0 => bf01_fraction(r, w, b, c.null, q),
                    Some(_) => {
                        run.warnings.push(format!(
                            "no fractional Bayes factor for {} and {}: the fraction times N must exceed 1",
                            x.name(),
                            y.name()
                        ));
                        f64::NAN
                    }
                };
                let mut row = labels.to_vec();
                row.extend([Cell::Number(w), number(Some(r)), number(Some(bf))]);
                factors.push(row);
                let p = if usable && c.analysis.posterior() {
                    correlation_posterior(r, w, c.c, run.alpha, q)
                } else {
                    NONE
                };
                // MAXPLOTS sets of prior, likelihood and posterior; not with Bayes factors only
                if usable && !perfect && c.analysis.posterior() && plotted < c.maxplots {
                    let title = format!(
                        "Correlation of {} and {}",
                        run.shown.variable(x),
                        run.shown.variable(y)
                    );
                    if let Some(chart) = models::correlation_chart(title, r, w, c.c, q) {
                        run.charts.push(chart);
                        plotted += 1;
                    }
                }
                let mut row = labels.to_vec();
                row.push(Cell::Number(w));
                row.extend(posterior_cells(&p));
                posteriors.push(row);
            }
        }
        let mut out = Vec::new();
        if c.analysis.factor() {
            let columns = ["", "", "N", "Pearson Correlation", "Bayes Factor"];
            let mut t = table("Bayes Factor for Pearson Correlations", &columns, factors);
            t.caption = Some(match c.fraction {
                None => format!("{BF01} JZS prior."),
                Some(b) => format!(
                    "{BF01} Fractional Bayes factor with fraction {b}, null value {}.",
                    c.null
                ),
            });
            out.push(t);
        }
        if c.analysis.posterior() {
            let title = "Posterior Distribution of the Correlation";
            let mut t = posterior_table(&run, title, &["", "", "N"], posteriors);
            t.caption = Some(format!(
                "The prior of rho is proportional to (1 - rho^2)^{}; the interval is the highest-density one.",
                c.c
            ));
            out.push(t);
        }
        Ok((out, run.warnings, run.charts))
    }
}

/// The Bayes factor row and the posterior row of a normal mean (or mean difference).
fn normal_rows(
    run: &mut Run,
    label: Cell,
    m: Moments,
    test: &MeanTest,
    prior: &NormalPrior,
    draws: Option<(usize, u64)>,
) -> (Vec<Cell>, Vec<Cell>) {
    let sd = m.variance().sqrt();
    let se = sd / m.w.sqrt();
    let df = m.w - 1.0;
    let t = (m.mean - test.null) / se;
    let bf = match test.variance {
        Some(variance) if m.w > 0.0 => {
            bf_known(m.mean - test.null, variance, m.w, test.g.unwrap_or(1.0))
        }
        Some(_) => f64::NAN,
        None if !(df > 0.0 && se > 0.0) => f64::NAN,
        None => match test.g {
            Some(g) => bf_fixed_g(t, df, m.w, g),
            None => bf_jzs(t, df, m.w, 1.0, run.quadrature, draws),
        },
    };
    let factor = vec![
        label.clone(),
        Cell::Number(test.null),
        Cell::Number(m.w),
        number(Some(m.mean)),
        number(Some(sd)),
        number(Some(se)),
        number(Some(bf)),
        number(Some(t)),
        number(Some(df)),
        number(Some(t_p(t, df))),
    ];
    let p = if m.w > 0.0 {
        mean_posterior(m.w, m.mean, m.ss, test.variance, prior, run.alpha)
    } else {
        NONE
    };
    let mut posterior = vec![label, Cell::Number(m.w)];
    posterior.extend(posterior_cells(&p));
    (factor, posterior)
}

/// Which cases of `cases` are successes.
fn successes(variable: &Variable, cases: &[usize], success: &Success) -> Result<Vec<bool>, Error> {
    if variable.is_string() {
        let values: Vec<String> = cases.iter().map(|&k| text(variable, k)).collect();
        let chosen = match success {
            Success::Last => values.iter().max().cloned().into_iter().collect(),
            Success::First => values.iter().min().cloned().into_iter().collect(),
            Success::Level(levels) => levels
                .iter()
                .map(|l| match l {
                    Value::String(s) => s.trim_end().to_string(),
                    Value::Number(n) => n.to_string(),
                })
                .collect::<Vec<_>>(),
            _ => {
                return Err(Error(format!(
                    "SUCCESS=MIDPOINT and CUTPOINT need a numeric variable, but {} is a string variable",
                    variable.name()
                )));
            }
        };
        return Ok(values.iter().map(|v| chosen.contains(v)).collect());
    }
    let values: Vec<f64> = cases.iter().map(|&k| numeric(variable, k)).collect();
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    Ok(match success {
        Success::Last => values.iter().map(|&v| v == high).collect(),
        Success::First => values.iter().map(|&v| v == low).collect(),
        Success::Midpoint => values.iter().map(|&v| v >= (low + high) / 2.0).collect(),
        Success::Cutpoint(cut) => values.iter().map(|&v| v >= *cut).collect(),
        Success::Level(levels) => {
            let numbers: Vec<f64> = levels
                .iter()
                .map(|l| match l {
                    Value::Number(n) => Ok(*n),
                    Value::String(s) => s.parse::<f64>().map_err(|_| {
                        Error(format!(
                            "SUCCESS=LEVEL needs numbers for the numeric variable {}, got {s}",
                            variable.name()
                        ))
                    }),
                })
                .collect::<Result<_, _>>()?;
            values.iter().map(|v| numbers.contains(v)).collect()
        }
    })
}

/// The posterior of the difference of two independent means, group 2 (`y`) minus group 1.
fn independent_posterior(
    run: &mut Run,
    c: &BayesIndependent,
    variable: &Variable,
    x: Moments,
    y: Moments,
) -> Posterior {
    let alpha = run.alpha;
    let difference = y.mean - x.mean;
    if let Some((vx, vy)) = c.datavar {
        return match c.mean {
            Some([mx0, my0, sx0, sy0]) => {
                let vxn = 1.0 / (1.0 / sx0 + x.w / vx);
                let vyn = 1.0 / (1.0 / sy0 + y.w / vy);
                let mxn = vxn * (mx0 / sx0 + x.mean * x.w / vx);
                let myn = vyn * (my0 / sy0 + y.mean * y.w / vy);
                normal_posterior(myn - mxn, vxn + vyn, alpha)
            }
            None => normal_posterior(difference, vy / y.w + vx / x.w, alpha),
        };
    }
    if c.mean.is_some() {
        run.warnings
            .push("MEANDIST=NORMAL needs DATAVAR; the flat mean priors are used".to_string());
    }
    let inverse = 1.0 / y.w + 1.0 / x.w;
    match (c.equal_variances, c.variance) {
        (true, GroupVariancePrior::Jeffreys) if x.w > 1.0 && y.w > 1.0 => {
            let df = x.w + y.w - 2.0;
            t_posterior(df, difference, (x.ss + y.ss) / df * inverse, alpha)
        }
        (true, GroupVariancePrior::InvChiSquare(df0, scale)) if x.w > 1.0 && y.w > 1.0 => {
            let df = df0 + x.w + y.w - 2.0;
            t_posterior(
                df,
                difference,
                (df0 * scale + x.ss + y.ss) / df * inverse,
                alpha,
            )
        }
        (true, GroupVariancePrior::Jeffreys | GroupVariancePrior::InvChiSquare(..)) => NONE,
        _ if x.w > 5.0 && y.w > 5.0 => {
            // Patil's approximation to the Behrens-Fisher distribution
            let (sx2, sy2) = (x.variance(), y.variance());
            let psi = ((sy2 / y.w).sqrt() / (sx2 / x.w).sqrt()).atan();
            let (sin2, cos2) = (psi.sin().powi(2), psi.cos().powi(2));
            let e1 = (y.w - 1.0) / (y.w - 3.0) * sin2 + (x.w - 1.0) / (x.w - 3.0) * cos2;
            let e2 = (y.w - 1.0).powi(2) / ((y.w - 3.0).powi(2) * (y.w - 5.0)) * sin2 * sin2
                + (x.w - 1.0).powi(2) / ((x.w - 3.0).powi(2) * (x.w - 5.0)) * cos2 * cos2;
            let e3 = 4.0 + e1 * e1 / e2;
            let e4 = (e1 * (e3 - 2.0) / e3).sqrt();
            t_posterior(e3, difference, e4 * e4 * (sx2 / x.w + sy2 / y.w), alpha)
        }
        _ => {
            run.warnings.push(format!(
                "no posterior for {} with unequal variances: each group needs more than five cases",
                variable.name()
            ));
            NONE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const Q: Quadrature = Quadrature {
        tol: 1e-10,
        limit: 2000,
    };

    #[test]
    fn quadrature_integrates_a_normal_density() {
        let log_f = |x: f64| -0.5 * (x - 3.0).powi(2) / 0.01;
        let got = log_integral(&log_f, -50.0, 50.0, 1e-12, 2000);
        assert!((got - (2.0 * PI * 0.01).sqrt().ln()).abs() < 1e-10, "{got}");
    }

    #[test]
    fn fixed_g_and_known_variance_factors_agree_in_the_limit() {
        // with a huge sample the t factor approaches the known-variance factor
        let bf = bf_fixed_g(2.0, 1e7, 1e7 + 1.0, 0.5);
        let known = bf_known(2.0 / (1e7f64 + 1.0).sqrt(), 1.0, 1e7 + 1.0, 0.5);
        assert!((bf / known - 1.0).abs() < 1e-5, "{bf} {known}");
    }

    #[test]
    fn correlation_posterior_of_a_flat_prior_is_symmetric_at_zero() {
        let p = correlation_posterior(0.0, 30.0, 0.0, 0.05, Q);
        assert!(p.mode.abs() < 1e-12 && p.mean.abs() < 1e-9);
        assert!((p.lower + p.upper).abs() < 1e-6);
    }
}
