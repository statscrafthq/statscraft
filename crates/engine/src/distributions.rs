//! The CDF, IDF, PDF, NCDF, NPDF and SIG functions of one value, as the matrix language calls
//! them element by element.
//!
//! Rules, from the descriptions of the functions and the appendix on distributions of the
//! Algorithms manual:
//! - CDF gives P(X <= x) (the same as P(X < x) for the continuous ones), SIG the upper tail
//!   P(X > x), IDF the x with CDF(x) = p, PDF the density or the probability of x.
//! - Parameters: EXP and GAMMA take a rate (means 1/b and a/b); LNORMAL(a, b) has ln X normal
//!   with mean ln a and sd b; WEIBULL(a, b) has scale a and shape b; PARETO(a, b) threshold a
//!   and shape b; LAPLACE, LOGISTIC and CAUCHY a location and a scale; HALFNRM a location and
//!   a scale, x >= location; IGAUSS a mean and a shape; GEOM counts the trials up to the first
//!   success (from 1), NEGBIN those up to the r-th; HYPER(x, total, sample, hits); SRANGE and
//!   SMOD take the number of means (comparisons) and the degrees of freedom; BVNOR(x1, x2, r)
//!   is the standard bivariate normal with correlation r.
//! - The noncentral distributions take the noncentrality lambda; the noncentral chi-square,
//!   F and beta are Poisson mixtures with mean lambda / 2.
//!
//! Choices where the manual is silent: a parameter out of range, or a probability for IDF
//! outside (0, 1) (0 and 1 are allowed where the quantile is finite), gives None, which the
//! caller reports as an error.

use crate::dist::{
    beta_i, gamma_q, ln_gamma, maximum_modulus_cdf, maximum_modulus_quantile, noncentral_f_cdf,
    noncentral_t_cdf, normal_cdf, normal_quantile, solve, studentized_range_cdf,
    studentized_range_quantile,
};
use std::f64::consts::PI;

fn positive(x: f64) -> bool {
    x > 0.0
}

fn probability(x: f64) -> bool {
    (0.0..=1.0).contains(&x)
}

fn whole(x: f64) -> bool {
    x >= 0.0 && x.fract() == 0.0
}

fn normal_pdf(z: f64) -> f64 {
    (-z * z / 2.0).exp() / (2.0 * PI).sqrt()
}

/// ln C(n, k).
fn ln_choose(n: f64, k: f64) -> f64 {
    ln_gamma(n + 1.0) - ln_gamma(k + 1.0) - ln_gamma(n - k + 1.0)
}

fn chi_square_cdf(x: f64, df: f64) -> f64 {
    if x <= 0.0 {
        0.0
    } else {
        1.0 - gamma_q(df / 2.0, x / 2.0)
    }
}

fn chi_square_pdf(x: f64, df: f64) -> f64 {
    gamma_pdf(x, df / 2.0, 0.5)
}

fn gamma_pdf(x: f64, shape: f64, rate: f64) -> f64 {
    if x < 0.0 {
        return 0.0;
    }
    if x == 0.0 {
        return match shape {
            s if s < 1.0 => f64::INFINITY,
            1.0 => rate,
            _ => 0.0,
        };
    }
    (shape * rate.ln() + (shape - 1.0) * x.ln() - rate * x - ln_gamma(shape)).exp()
}

fn beta_pdf(x: f64, a: f64, b: f64) -> f64 {
    if !(0.0..=1.0).contains(&x) {
        return 0.0;
    }
    if x == 0.0 || x == 1.0 {
        let exponent = if x == 0.0 { a } else { b };
        return match exponent {
            e if e < 1.0 => f64::INFINITY,
            1.0 => (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b)).exp(),
            _ => 0.0,
        };
    }
    (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + (a - 1.0) * x.ln() + (b - 1.0) * (1.0 - x).ln())
        .exp()
}

fn f_cdf(x: f64, d1: f64, d2: f64) -> f64 {
    if x <= 0.0 {
        0.0
    } else {
        beta_i(d1 / 2.0, d2 / 2.0, d1 * x / (d1 * x + d2))
    }
}

fn f_pdf(x: f64, d1: f64, d2: f64) -> f64 {
    if x < 0.0 {
        return 0.0;
    }
    let y = d1 * x / (d1 * x + d2);
    // the density of the beta variable y times dy/dx
    beta_pdf(y, d1 / 2.0, d2 / 2.0) * d1 * d2 / (d1 * x + d2).powi(2)
}

fn t_cdf(x: f64, df: f64) -> f64 {
    let tail = beta_i(df / 2.0, 0.5, df / (df + x * x)) / 2.0;
    if x > 0.0 { 1.0 - tail } else { tail }
}

fn t_pdf(x: f64, df: f64) -> f64 {
    (ln_gamma((df + 1.0) / 2.0)
        - ln_gamma(df / 2.0)
        - 0.5 * (df * PI).ln()
        - (df + 1.0) / 2.0 * (1.0 + x * x / df).ln())
    .exp()
}

/// x with cdf(x) = p for an increasing cdf, searched upwards from `low`, or both ways when
/// `low` is -infinity.
fn quantile(p: f64, cdf: impl Fn(f64) -> f64, low: f64) -> f64 {
    let f = |x: f64| cdf(x) - p;
    let mut high = if low.is_finite() { low + 1.0 } else { 1.0 };
    let mut step = 1.0;
    while f(high) < 0.0 && high < 1e300 {
        step *= 2.0;
        high += step;
    }
    let mut bottom = if low.is_finite() { low } else { high - 2.0 };
    let mut step = 1.0;
    while !low.is_finite() && f(bottom) > 0.0 && bottom > -1e300 {
        step *= 2.0;
        bottom -= step;
    }
    solve(f, bottom, high)
}

/// A sum of Poisson(mean) weights times term(j), from the largest weight outwards.
fn poisson_mixture(mean: f64, term: impl Fn(f64) -> f64) -> f64 {
    if mean <= 0.0 {
        return term(0.0);
    }
    let weight = |j: f64| (-mean + j * mean.ln() - ln_gamma(j + 1.0)).exp();
    let top = mean.floor();
    // a mean this large has no next term in floating point, so the sum cannot be taken
    if top.is_nan() || top + 1.0 == top {
        return f64::NAN;
    }
    let mut sum = 0.0;
    for k in 0..=100_000 {
        let j = top - f64::from(k);
        let w = weight(j);
        sum += w * term(j);
        if j == 0.0 || (k > 5 && w < 1e-17) {
            break;
        }
    }
    for k in 1..=100_000 {
        let j = top + f64::from(k);
        let w = weight(j);
        sum += w * term(j);
        if k > 5 && w < 1e-17 {
            break;
        }
    }
    sum
}

/// P(X <= x, Y <= y) for standard normals with correlation r: the integral over the first of
/// its density times the conditional probability of the second.
fn bivariate_normal_cdf(x: f64, y: f64, r: f64) -> f64 {
    if r >= 1.0 {
        return normal_cdf(x.min(y));
    }
    if r <= -1.0 {
        return (normal_cdf(x) - normal_cdf(-y)).max(0.0);
    }
    let s = (1.0 - r * r).sqrt();
    let low = -40.0_f64;
    let high = x.min(40.0);
    if high <= low {
        return 0.0;
    }
    let f = |t: f64| normal_pdf(t) * normal_cdf((y - r * t) / s);
    // Simpson's rule on many short pieces: the integrand is smooth
    let pieces = 4000;
    let h = (high - low) / pieces as f64;
    let mut sum = f(low) + f(high);
    for k in 1..pieces {
        sum += f(low + k as f64 * h) * if k % 2 == 1 { 4.0 } else { 2.0 };
    }
    (sum * h / 3.0).clamp(0.0, 1.0)
}

fn inverse_gaussian_cdf(x: f64, mean: f64, shape: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    let a = (shape / x).sqrt();
    let first = normal_cdf(a * (x / mean - 1.0));
    let second = (2.0 * shape / mean + normal_cdf(-a * (x / mean + 1.0)).ln()).exp();
    (first + second).clamp(0.0, 1.0)
}

fn binomial_cdf(x: f64, n: f64, p: f64) -> f64 {
    let k = x.floor();
    if k < 0.0 {
        return 0.0;
    }
    if k >= n {
        return 1.0;
    }
    // P(X <= k) = I_{1-p}(n - k, k + 1)
    beta_i(n - k, k + 1.0, 1.0 - p)
}

fn poisson_cdf(x: f64, mean: f64) -> f64 {
    let k = x.floor();
    if k < 0.0 { 0.0 } else { gamma_q(k + 1.0, mean) }
}

fn hyper_pdf(x: f64, total: f64, sample: f64, hits: f64) -> f64 {
    let low = (sample + hits - total).max(0.0);
    let high = sample.min(hits);
    if x.fract() != 0.0 || x < low || x > high {
        return 0.0;
    }
    (ln_choose(hits, x) + ln_choose(total - hits, sample - x) - ln_choose(total, sample)).exp()
}

/// P(X <= k) of the hypergeometric. The terms fall on both sides of the mode, so the tail
/// on the far side of k from the mode is summed outwards from k until the terms no longer
/// count: a billion terms from the lowest value would take minutes.
fn hyper_cdf(k: f64, total: f64, sample: f64, hits: f64) -> f64 {
    let low = (sample + hits - total).max(0.0);
    let high = sample.min(hits);
    if k < low {
        return 0.0;
    }
    if k >= high {
        return 1.0;
    }
    let tail = |from: f64, step: f64| {
        let mut sum = 0.0;
        let mut j = from;
        while (low..=high).contains(&j) {
            let p = hyper_pdf(j, total, sample, hits);
            sum += p;
            // the next value is no different at this size, or its term adds nothing
            if p == 0.0 || p < 1e-17 * sum || j + step == j {
                break;
            }
            j += step;
        }
        sum
    };
    let mode = ((sample + 1.0) * (hits + 1.0) / (total + 2.0)).floor();
    if k < mode {
        tail(k, -1.0).min(1.0)
    } else {
        (1.0 - tail(k + 1.0, 1.0)).max(0.0)
    }
}

/// P(X = x) of the negative binomial: the trials up to the r-th success.
fn negbin_pdf(x: f64, r: f64, p: f64) -> f64 {
    if x.fract() != 0.0 || x < r {
        return 0.0;
    }
    if p == 1.0 {
        return f64::from(u8::from(x == r));
    }
    (ln_choose(x - 1.0, r - 1.0) + r * p.ln() + (x - r) * (1.0 - p).ln()).exp()
}

fn noncentral_t_pdf(x: f64, df: f64, delta: f64) -> f64 {
    if x == 0.0 {
        return (ln_gamma((df + 1.0) / 2.0) - ln_gamma(df / 2.0)).exp() / (df * PI).sqrt()
            * (-delta * delta / 2.0).exp();
    }
    let wider = noncentral_t_cdf(x * (1.0 + 2.0 / df).sqrt(), df + 2.0, delta);
    (df / x * (wider - noncentral_t_cdf(x, df, delta))).max(0.0)
}

/// The value of `name` (such as "CDF.NORMAL") at the arguments `p`, the first being the
/// quantile or probability. None when a parameter is out of range.
pub(crate) fn evaluate(name: &str, p: &[f64]) -> Option<f64> {
    let (family, distribution) = name.split_once('.')?;
    let x = p[0];
    let value = match (family, distribution, &p[1..]) {
        (_, "NORMAL", &[mean, sd]) if positive(sd) => {
            let z = (x - mean) / sd;
            match family {
                "CDF" => normal_cdf(z),
                "PDF" => normal_pdf(z) / sd,
                "IDF" => mean + sd * idf_normal(x)?,
                _ => return None,
            }
        }
        (_, "T", &[df]) if positive(df) => match family {
            "CDF" => t_cdf(x, df),
            "PDF" => t_pdf(x, df),
            "IDF" => {
                check_p(x)?;
                quantile(x, |t| t_cdf(t, df), f64::NEG_INFINITY)
            }
            _ => return None,
        },
        (_, "CHISQ", &[df]) if positive(df) => match family {
            "CDF" => chi_square_cdf(x, df),
            "SIG" => {
                if x <= 0.0 {
                    1.0
                } else {
                    gamma_q(df / 2.0, x / 2.0)
                }
            }
            "PDF" => chi_square_pdf(x, df),
            "IDF" => {
                check_p(x)?;
                quantile(x, |v| chi_square_cdf(v, df), 0.0)
            }
            _ => return None,
        },
        (_, "F", &[d1, d2]) if positive(d1) && positive(d2) => match family {
            "CDF" => f_cdf(x, d1, d2),
            "SIG" => 1.0 - f_cdf(x, d1, d2),
            "PDF" => f_pdf(x, d1, d2),
            "IDF" => {
                check_p(x)?;
                quantile(x, |v| f_cdf(v, d1, d2), 0.0)
            }
            _ => return None,
        },
        (_, "BETA", &[a, b]) if positive(a) && positive(b) => match family {
            "CDF" => beta_i(a, b, x),
            "PDF" => beta_pdf(x, a, b),
            "IDF" => {
                check_p(x)?;
                solve(|v| beta_i(a, b, v) - x, 0.0, 1.0)
            }
            _ => return None,
        },
        (_, "GAMMA", &[shape, rate]) if positive(shape) && positive(rate) => {
            let cdf = |v: f64| {
                if v <= 0.0 {
                    0.0
                } else {
                    1.0 - gamma_q(shape, rate * v)
                }
            };
            match family {
                "CDF" => cdf(x),
                "PDF" => gamma_pdf(x, shape, rate),
                "IDF" => {
                    check_p(x)?;
                    quantile(x, cdf, 0.0)
                }
                _ => return None,
            }
        }
        (_, "EXP", &[rate]) if positive(rate) => match family {
            "CDF" => 1.0 - (-rate * x.max(0.0)).exp(),
            "PDF" => {
                if x < 0.0 {
                    0.0
                } else {
                    rate * (-rate * x).exp()
                }
            }
            "IDF" => {
                check_p(x)?;
                -(1.0 - x).ln() / rate
            }
            _ => return None,
        },
        (_, "UNIFORM", &[min, max]) if min <= max => match family {
            "CDF" => ((x - min) / (max - min)).clamp(0.0, 1.0),
            "PDF" => {
                if (min..=max).contains(&x) {
                    1.0 / (max - min)
                } else {
                    0.0
                }
            }
            "IDF" => {
                check_p(x)?;
                min + x * (max - min)
            }
            _ => return None,
        },
        (_, "CAUCHY", &[location, scale]) if positive(scale) => {
            let z = (x - location) / scale;
            match family {
                "CDF" => 0.5 + z.atan() / PI,
                "PDF" => 1.0 / (PI * scale * (1.0 + z * z)),
                "IDF" => {
                    check_p(x)?;
                    location + scale * (PI * (x - 0.5)).tan()
                }
                _ => return None,
            }
        }
        (_, "LAPLACE", &[mean, scale]) if positive(scale) => {
            let z = (x - mean) / scale;
            match family {
                "CDF" => {
                    if z < 0.0 {
                        z.exp() / 2.0
                    } else {
                        1.0 - (-z).exp() / 2.0
                    }
                }
                "PDF" => (-z.abs()).exp() / (2.0 * scale),
                "IDF" => {
                    check_p(x)?;
                    if x <= 0.5 {
                        mean + scale * (2.0 * x).ln()
                    } else {
                        mean - scale * (2.0 * (1.0 - x)).ln()
                    }
                }
                _ => return None,
            }
        }
        (_, "LOGISTIC", &[mean, scale]) if positive(scale) => {
            let z = (x - mean) / scale;
            match family {
                "CDF" => 1.0 / (1.0 + (-z).exp()),
                "PDF" => {
                    let e = (-z.abs()).exp();
                    e / (scale * (1.0 + e).powi(2))
                }
                "IDF" => {
                    check_p(x)?;
                    mean + scale * (x / (1.0 - x)).ln()
                }
                _ => return None,
            }
        }
        (_, "LNORMAL", &[a, b]) if positive(a) && positive(b) => match family {
            "CDF" => {
                if x <= 0.0 {
                    0.0
                } else {
                    normal_cdf((x.ln() - a.ln()) / b)
                }
            }
            "PDF" => {
                if x <= 0.0 {
                    0.0
                } else {
                    normal_pdf((x.ln() - a.ln()) / b) / (x * b)
                }
            }
            "IDF" => a * (b * idf_normal(x)?).exp(),
            _ => return None,
        },
        (_, "HALFNRM", &[location, scale]) if positive(scale) => {
            let z = (x - location) / scale;
            match family {
                "CDF" => {
                    if z <= 0.0 {
                        0.0
                    } else {
                        2.0 * normal_cdf(z) - 1.0
                    }
                }
                "PDF" => {
                    if z < 0.0 {
                        0.0
                    } else {
                        2.0 * normal_pdf(z) / scale
                    }
                }
                "IDF" => {
                    check_p(x)?;
                    if x == 1.0 {
                        return None;
                    }
                    location + scale * normal_quantile((1.0 + x) / 2.0)
                }
                _ => return None,
            }
        }
        (_, "PARETO", &[threshold, shape]) if positive(threshold) && positive(shape) => {
            match family {
                "CDF" => {
                    if x <= threshold {
                        0.0
                    } else {
                        1.0 - (threshold / x).powf(shape)
                    }
                }
                "PDF" => {
                    if x < threshold {
                        0.0
                    } else {
                        shape * threshold.powf(shape) / x.powf(shape + 1.0)
                    }
                }
                "IDF" => {
                    check_p(x)?;
                    threshold / (1.0 - x).powf(1.0 / shape)
                }
                _ => return None,
            }
        }
        (_, "WEIBULL", &[scale, shape]) if positive(scale) && positive(shape) => match family {
            "CDF" => {
                if x <= 0.0 {
                    0.0
                } else {
                    1.0 - (-(x / scale).powf(shape)).exp()
                }
            }
            "PDF" => {
                if x < 0.0 {
                    0.0
                } else {
                    shape / scale * (x / scale).powf(shape - 1.0) * (-(x / scale).powf(shape)).exp()
                }
            }
            "IDF" => {
                check_p(x)?;
                scale * (-(1.0 - x).ln()).powf(1.0 / shape)
            }
            _ => return None,
        },
        (_, "IGAUSS", &[mean, shape]) if positive(mean) && positive(shape) => match family {
            "CDF" => inverse_gaussian_cdf(x, mean, shape),
            "PDF" => {
                if x <= 0.0 {
                    0.0
                } else {
                    (shape / (2.0 * PI * x.powi(3))).sqrt()
                        * (-shape * (x - mean).powi(2) / (2.0 * mean * mean * x)).exp()
                }
            }
            "IDF" => {
                check_p(x)?;
                quantile(x, |v| inverse_gaussian_cdf(v, mean, shape), 0.0)
            }
            _ => return None,
        },
        (_, "SRANGE", &[k, df]) if k >= 1.0 && df >= 1.0 => match family {
            "CDF" => studentized_range_cdf(x, k, df),
            "IDF" => {
                check_p(x)?;
                studentized_range_quantile(1.0 - x, k, df)
            }
            _ => return None,
        },
        (_, "SMOD", &[k, df]) if k >= 1.0 && df >= 1.0 => match family {
            "CDF" => maximum_modulus_cdf(x, k, df),
            "IDF" => {
                check_p(x)?;
                maximum_modulus_quantile(1.0 - x, k, df)
            }
            _ => return None,
        },
        (_, "BVNOR", &[y, r]) if (-1.0..=1.0).contains(&r) => match family {
            "CDF" => bivariate_normal_cdf(x, y, r),
            "PDF" => {
                let s = 1.0 - r * r;
                if s == 0.0 {
                    return None;
                }
                (-(x * x - 2.0 * r * x * y + y * y) / (2.0 * s)).exp() / (2.0 * PI * s.sqrt())
            }
            _ => return None,
        },
        (_, "BERNOULLI", &[prob]) if probability(prob) => match family {
            "CDF" => {
                if x < 0.0 {
                    0.0
                } else if x < 1.0 {
                    1.0 - prob
                } else {
                    1.0
                }
            }
            "PDF" => match x {
                0.0 => 1.0 - prob,
                1.0 => prob,
                _ => 0.0,
            },
            _ => return None,
        },
        (_, "BINOM", &[n, prob]) if whole(n) && probability(prob) => match family {
            "CDF" => binomial_cdf(x, n, prob),
            "PDF" => {
                if x.fract() != 0.0 || x < 0.0 || x > n {
                    0.0
                } else if prob == 0.0 || prob == 1.0 {
                    f64::from(u8::from(x == n * prob))
                } else {
                    (ln_choose(n, x) + x * prob.ln() + (n - x) * (1.0 - prob).ln()).exp()
                }
            }
            _ => return None,
        },
        (_, "GEOM", &[prob]) if prob > 0.0 && prob <= 1.0 => match family {
            "CDF" => {
                if x < 1.0 {
                    0.0
                } else {
                    1.0 - (1.0 - prob).powf(x.floor())
                }
            }
            "PDF" => {
                if x.fract() != 0.0 || x < 1.0 {
                    0.0
                } else {
                    prob * (1.0 - prob).powf(x - 1.0)
                }
            }
            _ => return None,
        },
        (_, "NEGBIN", &[r, prob]) if whole(r) && r >= 1.0 && prob > 0.0 && prob <= 1.0 => {
            match family {
                "CDF" => {
                    let k = x.floor();
                    if k < r {
                        0.0
                    } else {
                        // r-th success within k trials: at least r successes in k trials
                        1.0 - binomial_cdf(r - 1.0, k, prob)
                    }
                }
                "PDF" => negbin_pdf(x, r, prob),
                _ => return None,
            }
        }
        (_, "POISSON", &[mean]) if positive(mean) => match family {
            "CDF" => poisson_cdf(x, mean),
            "PDF" => {
                if x.fract() != 0.0 || x < 0.0 {
                    0.0
                } else {
                    (x * mean.ln() - mean - ln_gamma(x + 1.0)).exp()
                }
            }
            _ => return None,
        },
        (_, "HYPER", &[total, sample, hits])
            if whole(total) && whole(sample) && whole(hits) && sample <= total && hits <= total =>
        {
            match family {
                "PDF" => hyper_pdf(x, total, sample, hits),
                "CDF" => hyper_cdf(x.floor(), total, sample, hits),
                _ => return None,
            }
        }
        _ => return noncentral(family, distribution, x, &p[1..]),
    };
    value.is_finite().then_some(value).or_else(|| {
        // a density may be infinite at the edge of its range
        (family == "PDF" || family == "NPDF").then_some(value)
    })
}

fn noncentral(family: &str, distribution: &str, x: f64, p: &[f64]) -> Option<f64> {
    let value = match (family, distribution, p) {
        ("NCDF", "CHISQ", &[df, lambda]) if positive(df) && lambda >= 0.0 => {
            poisson_mixture(lambda / 2.0, |j| chi_square_cdf(x, df + 2.0 * j))
        }
        ("NPDF", "CHISQ", &[df, lambda]) if positive(df) && lambda >= 0.0 => {
            poisson_mixture(lambda / 2.0, |j| chi_square_pdf(x, df + 2.0 * j))
        }
        ("NCDF", "F", &[d1, d2, lambda]) if positive(d1) && positive(d2) && lambda >= 0.0 => {
            noncentral_f_cdf(x, d1, d2, lambda)
        }
        ("NPDF", "F", &[d1, d2, lambda]) if positive(d1) && positive(d2) && lambda >= 0.0 => {
            if x < 0.0 {
                0.0
            } else {
                let y = d1 * x / (d1 * x + d2);
                let jacobian = d1 * d2 / (d1 * x + d2).powi(2);
                poisson_mixture(lambda / 2.0, |j| beta_pdf(y, d1 / 2.0 + j, d2 / 2.0)) * jacobian
            }
        }
        ("NCDF", "BETA", &[a, b, lambda]) if positive(a) && positive(b) && lambda >= 0.0 => {
            poisson_mixture(lambda / 2.0, |j| beta_i(a + j, b, x))
        }
        ("NPDF", "BETA", &[a, b, lambda]) if positive(a) && positive(b) && lambda >= 0.0 => {
            poisson_mixture(lambda / 2.0, |j| beta_pdf(x, a + j, b))
        }
        ("NCDF", "T", &[df, delta]) if positive(df) => noncentral_t_cdf(x, df, delta),
        ("NPDF", "T", &[df, delta]) if positive(df) => noncentral_t_pdf(x, df, delta),
        _ => return None,
    };
    Some(value.clamp(0.0, f64::INFINITY))
}

/// Whether x, the first argument of a CDF, PDF, NCDF, NPDF or SIG function, is a value the
[removed]
/// whole numbers only. The parameters are left to `evaluate`.
pub(crate) fn x_in_range(name: &str, p: &[f64]) -> bool {
    let Some((family, distribution)) = name.split_once('.') else {
        return true;
    };
    if family == "IDF" {
        return true;
    }
    let x = p[0];
    let whole = |low: f64, high: f64| x.fract() == 0.0 && x >= low && x <= high;
    match (distribution, &p[1..]) {
        ("BETA", _) => (0.0..=1.0).contains(&x),
        (
            "CHISQ" | "EXP" | "F" | "GAMMA" | "IGAUSS" | "LNORMAL" | "SMOD" | "SRANGE" | "WEIBULL",
            _,
        ) => x >= 0.0,
        ("HALFNRM" | "PARETO", &[low, ..]) => x >= low,
        ("UNIFORM", &[low, high, ..]) => (low..=high).contains(&x),
        ("BERNOULLI", _) => whole(0.0, 1.0),
        ("BINOM", &[n, ..]) => whole(0.0, n),
        ("GEOM", _) => whole(1.0, f64::INFINITY),
        ("NEGBIN", &[r, ..]) => whole(r, f64::INFINITY),
        ("POISSON", _) => whole(0.0, f64::INFINITY),
        ("HYPER", &[total, sample, hits]) => {
            whole((sample + hits - total).max(0.0), sample.min(hits))
        }
        _ => true,
    }
}

fn check_p(p: f64) -> Option<()> {
    (0.0..=1.0).contains(&p).then_some(())
}

fn idf_normal(p: f64) -> Option<f64> {
    (p > 0.0 && p < 1.0).then(|| normal_quantile(p))
}

#[cfg(test)]
mod tests {
    use super::evaluate;

    fn near(name: &str, args: &[f64], expected: f64) {
        let value = evaluate(name, args).unwrap();
        assert!(
            (value - expected).abs() <= 1e-9 * expected.abs().max(1.0),
            "{name}{args:?} = {value}, expected {expected}"
        );
    }

    // values from the closed forms or printed with 10 digits or more by R
    #[test]
    fn continuous() {
        near("CDF.NORMAL", &[1.96, 0.0, 1.0], 0.9750021048517795);
        near("IDF.NORMAL", &[0.975, 10.0, 2.0], 13.919927969080115);
        near("PDF.NORMAL", &[0.0, 0.0, 1.0], 0.3989422804014327);
        near("CDF.T", &[2.0, 5.0], 0.9490302605850709);
        near("IDF.T", &[0.95, 10.0], 1.8124611228107335);
        near("PDF.T", &[1.0, 3.0], 0.20674833578317203);
        near("CDF.CHISQ", &[3.0, 2.0], 0.7768698398515702);
        near("IDF.CHISQ", &[0.95, 3.0], 7.814727903251178);
        near("SIG.CHISQ", &[3.84, 1.0], 0.05004352124870519);
        near("PDF.CHISQ", &[2.0, 3.0], 0.2075537487102974);
        near("CDF.F", &[3.0, 2.0, 10.0], 0.904632568359375);
        near("IDF.F", &[0.95, 3.0, 100.0], 2.6955342548881385);
        near("PDF.F", &[1.0, 4.0, 6.0], 0.41472);
        near("CDF.BETA", &[0.3, 2.0, 3.0], 0.3483);
        near("PDF.BETA", &[0.3, 2.0, 3.0], 1.764);
        near("IDF.BETA", &[0.3483, 2.0, 3.0], 0.3);
        near("CDF.GAMMA", &[2.0, 2.0, 1.0], 0.5939941502901619);
        near("PDF.GAMMA", &[2.0, 2.0, 1.0], 0.2706705664732254);
        near("IDF.GAMMA", &[0.5939941502901619, 2.0, 1.0], 2.0);
        near("CDF.EXP", &[1.0, 2.0], 1.0 - (-2.0_f64).exp());
        near(
            "CDF.WEIBULL",
            &[2.0, 1.5, 2.0],
            1.0 - (-(2.0_f64 / 1.5).powi(2)).exp(),
        );
        near("IDF.WEIBULL", &[0.5, 1.0, 1.0], std::f64::consts::LN_2);
        near("CDF.LNORMAL", &[2.0, 1.0, 1.0], 0.7558914042144173);
        near("CDF.LOGISTIC", &[1.0, 0.0, 1.0], 0.7310585786300049);
        near(
            "CDF.LAPLACE",
            &[1.0, 0.0, 1.0],
            1.0 - (-1.0_f64).exp() / 2.0,
        );
        near("CDF.CAUCHY", &[1.0, 0.0, 1.0], 0.75);
        near("CDF.PARETO", &[4.0, 2.0, 3.0], 0.875);
        near("CDF.UNIFORM", &[0.25, 0.0, 2.0], 0.125);
        near("CDF.HALFNRM", &[1.0, 0.0, 1.0], 0.6826894921370859);
        near("CDF.IGAUSS", &[1.0, 1.0, 2.0], 0.6276978381552528);
        near("CDF.BVNOR", &[0.0, 0.0, 0.5], 1.0 / 3.0);
        near("NCDF.CHISQ", &[3.0, 2.0, 1.0], 0.6206436532195437);
        near("NCDF.T", &[1.0, 10.0, 1.0], 0.49024005139545046);
        near("NCDF.F", &[2.0, 3.0, 10.0, 2.0], 0.631360852827701);
        near("NCDF.BETA", &[0.4, 2.0, 3.0, 1.0], 0.42946121234784);
        near("NPDF.BETA", &[0.4, 2.0, 3.0, 1.0], 1.6902034399412835);
        near("NPDF.F", &[1.0, 3.0, 10.0, 2.0], 0.35969443195475725);
        near("NPDF.CHISQ", &[2.0, 3.0, 1.5], 0.15496299662495927);
        near("NPDF.T", &[1.0, 10.0, 1.0], 0.37984052618871944);
        near("CDF.BVNOR", &[1.0, -0.5, -0.4], 0.22185866541475938);
        near("PDF.BVNOR", &[0.5, 0.2, 0.3], 0.1470334226549344);
        near("IDF.IGAUSS", &[0.5, 1.0, 2.0], 0.8043390412960014);
        near("CDF.SRANGE", &[3.5, 3.0, 10.0], 0.9228966891615896);
    }

    #[test]
    fn discrete() {
        near("PDF.BINOM", &[3.0, 10.0, 0.3], 0.2668279320000001);
        near("CDF.BINOM", &[3.0, 10.0, 0.3], 0.6496107184000002);
        near("PDF.POISSON", &[2.0, 3.0], 0.22404180765538775);
        near("CDF.POISSON", &[2.0, 3.0], 0.42319008112684353);
        near("PDF.GEOM", &[3.0, 0.5], 0.125);
        near("CDF.GEOM", &[3.0, 0.5], 0.875);
        near("PDF.HYPER", &[1.0, 10.0, 3.0, 4.0], 0.5);
        near("CDF.HYPER", &[1.0, 10.0, 3.0, 4.0], 2.0 / 3.0);
        near("PDF.NEGBIN", &[5.0, 2.0, 0.5], 0.125);
        near("CDF.NEGBIN", &[5.0, 2.0, 0.5], 0.8125);
        near("CDF.BERNOULLI", &[0.0, 0.3], 0.7);
        assert_eq!(evaluate("CDF.NORMAL", &[1.0, 0.0, -1.0]), None);
    }
}
