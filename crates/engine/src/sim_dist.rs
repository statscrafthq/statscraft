//! The distributions of SIMPLAN and SIMRUN: their distribution and quantile functions, the
//! fit to data and the goodness of fit.
//!
//! Rules, from the simulation algorithms:
//! - Maximum likelihood with frequency weights: normal (σ with divisor W), lognormal (A the
//!   exponential of the mean log, B the root mean squared log deviation), exponential (rate 1
//!   over the mean), uniform (the extremes), Poisson (the mean), categorical (the weighted
//!   shares). Beta, gamma, two-parameter Weibull and the negative binomial (failures before the
//!   r-th success) have no closed form: we solve their likelihood equations (the profile
//!   equation in one parameter where one exists, Newton's method with step halving for beta).
//! - Triangular: the extremes, the mode by the method of moments, 3 z̄ - 1 on the 0 to 1 scale,
//!   kept inside the extremes. Binomial: the method of moments, N rounded and P = mean / N; no
//!   fit when the variance is not below the mean.
//! - Continuous fits are compared by Anderson-Darling or Kolmogorov-Smirnov on F(x), with the
//!   p-value formulas and critical-value tables of the algorithms; discrete fits by chi-square on
//!   the classes 0 to N (binomial), 0 to the maximum with the last class taking the upper tail
//!   (Poisson, negative binomial) or the categories, df = classes - 1 - parameters.

use crate::dist::{beta_i, chi_square_p, gamma_q, ln_gamma, normal_cdf, normal_quantile, solve};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Dist {
    Bernoulli(f64),
    Beta(f64, f64),
    Binom(f64, f64),
    /// Categories in ascending order with their probabilities.
    Categorical(Vec<f64>, Vec<f64>),
    /// The quantiles at 0, 1/B, ..., 1: the distribution function joins them linearly.
    Empirical(Vec<f64>),
    /// The rate.
    Exp(f64),
    /// Shape and rate.
    Gamma(f64, f64),
    /// The median and the standard deviation of the logarithm.
    LNormal(f64, f64),
    /// Successes r, success probability p; `trials` counts trials rather than failures.
    NegBin(f64, f64, bool),
    Normal(f64, f64),
    Poisson(f64),
    /// Minimum, maximum, mode.
    Triangular(f64, f64, f64),
    Uniform(f64, f64),
    UserRanges(Vec<f64>, Vec<f64>, Vec<f64>),
    /// Scale, shape, origin.
    Weibull(f64, f64, f64),
}

/// The parameters of each distribution, in the order the plan keeps them.
pub(crate) const PARAMS: [(&str, &[&str]); 15] = [
    ("BERNOULLI", &["PROB"]),
    ("BETA", &["SHAPE1", "SHAPE2"]),
    ("BINOM", &["N", "PROB"]),
    ("CATEGORICAL", &["CATS", "PROBS"]),
    ("EMPIRICAL", &["POINTS"]),
    ("EXP", &["SCALE"]),
    ("GAMMA", &["SHAPE", "SCALE"]),
    ("LNORMAL", &["A", "B"]),
    ("NEGBIN", &["THRESHOLD", "PROB"]),
    ("NORMAL", &["MEAN", "STDDEV"]),
    ("POISSON", &["MEAN"]),
    ("TRIANGULAR", &["MIN", "MAX", "MODE"]),
    ("UNIFORM", &["MIN", "MAX"]),
    ("USER_RANGES", &["MIN", "MAX", "PROBS"]),
    ("WEIBULL", &["A", "B", "C"]),
];

/// Parameters as the plan keeps them: name and values.
pub(crate) type Params = Vec<(String, Vec<f64>)>;

impl Dist {
    pub(crate) fn new(family: &str, params: &Params, trials: bool) -> Result<Dist, String> {
        let names = PARAMS
            .iter()
            .find(|(f, _)| *f == family)
            .ok_or_else(|| format!("unknown distribution {family}"))?
            .1;
        if let Some((name, _)) = params.iter().find(|(n, _)| !names.contains(&n.as_str())) {
            return Err(format!("{family} has no parameter {name}"));
        }
        let list = |name: &str| -> Result<Vec<f64>, String> {
            match params.iter().find(|(n, _)| n == name) {
                Some((_, v)) => Ok(v.clone()),
                None if family == "WEIBULL" && name == "C" => Ok(vec![0.0]),
                None => Err(format!("{family} needs {name}=")),
            }
        };
        let one = |name: &str| -> Result<f64, String> {
            match list(name)?[..] {
                [v] => Ok(v),
                _ => Err(format!("{family} takes one value of {name}")),
            }
        };
        let check = |ok: bool, what: &str| match ok {
            true => Ok(()),
            false => Err(format!("{family}: {what}")),
        };
        let p01 = |v: f64| (0.0..=1.0).contains(&v);
        let d = match family {
            "BERNOULLI" => Dist::Bernoulli(one("PROB")?),
            "BETA" => Dist::Beta(one("SHAPE1")?, one("SHAPE2")?),
            "BINOM" => Dist::Binom(one("N")?, one("PROB")?),
            "CATEGORICAL" | "USER_RANGES" => {
                let probs = list("PROBS")?;
                let total: f64 = probs.iter().sum();
                check(
                    probs.iter().all(|&p| p >= 0.0) && (total - 1.0).abs() < 1e-6,
                    "the probabilities must sum to 1",
                )?;
                if family == "USER_RANGES" {
                    let (min, max) = (list("MIN")?, list("MAX")?);
                    check(
                        min.len() == probs.len() && max.len() == probs.len(),
                        "MIN, MAX and PROBS need as many values",
                    )?;
                    check(
                        min.iter().zip(&max).all(|(a, b)| a <= b),
                        "each MIN must not exceed its MAX",
                    )?;
                    Dist::UserRanges(min, max, probs)
                } else {
                    let cats = list("CATS")?;
                    check(
                        cats.len() == probs.len(),
                        "CATS and PROBS need as many values",
                    )?;
                    let mut pairs: Vec<(f64, f64)> = cats.into_iter().zip(probs).collect();
                    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
                    check(
                        pairs.windows(2).all(|w| w[0].0 != w[1].0),
                        "a category is given twice",
                    )?;
                    Dist::Categorical(
                        pairs.iter().map(|p| p.0).collect(),
                        pairs.iter().map(|p| p.1).collect(),
                    )
                }
            }
            "EMPIRICAL" => Dist::Empirical(list("POINTS")?),
            "EXP" => Dist::Exp(one("SCALE")?),
            "GAMMA" => Dist::Gamma(one("SHAPE")?, one("SCALE")?),
            "LNORMAL" => Dist::LNormal(one("A")?, one("B")?),
            "NEGBIN" => Dist::NegBin(one("THRESHOLD")?, one("PROB")?, trials),
            "NORMAL" => Dist::Normal(one("MEAN")?, one("STDDEV")?),
            "POISSON" => Dist::Poisson(one("MEAN")?),
            "TRIANGULAR" => Dist::Triangular(one("MIN")?, one("MAX")?, one("MODE")?),
            "UNIFORM" => Dist::Uniform(one("MIN")?, one("MAX")?),
            _ => Dist::Weibull(one("A")?, one("B")?, one("C")?),
        };
        match d {
            Dist::Bernoulli(p) => check(p01(p), "PROB must be from 0 to 1")?,
            Dist::Beta(a, b) | Dist::Gamma(a, b) | Dist::Weibull(a, b, _) => {
                check(a > 0.0 && b > 0.0, "the parameters must be positive")?
            }
            Dist::Binom(n, p) => check(
                n >= 0.0 && n.fract() == 0.0 && p01(p),
                "N must be a whole number and PROB from 0 to 1",
            )?,
            Dist::Exp(r) | Dist::Poisson(r) => check(r > 0.0, "the parameter must be positive")?,
            Dist::LNormal(a, b) | Dist::Normal(a, b) => check(
                b > 0.0 && (family == "NORMAL" || a > 0.0),
                "the parameters must be positive (the mean of NORMAL may be any number)",
            )?,
            Dist::NegBin(r, p, _) => check(
                r > 0.0 && p > 0.0 && p <= 1.0 && (!trials || r.fract() == 0.0),
                "THRESHOLD must be positive (whole with TYPE=TRIALS) and PROB above 0 up to 1",
            )?,
            Dist::Triangular(a, b, m) => check(a <= m && m <= b && a < b, "MIN <= MODE <= MAX")?,
            Dist::Uniform(a, b) => check(a < b, "MIN must be below MAX")?,
            Dist::Empirical(ref q) => check(
                q.len() >= 2 && q.windows(2).all(|w| w[0] <= w[1]),
                "the points must ascend",
            )?,
            _ => {}
        }
        Ok(d)
    }

    /// A distribution of whole numbers.
    pub(crate) fn discrete(&self) -> bool {
        matches!(
            self,
            Dist::Bernoulli(_)
                | Dist::Binom(..)
                | Dist::Categorical(..)
                | Dist::NegBin(..)
                | Dist::Poisson(_)
        )
    }

    pub(crate) fn cdf(&self, x: f64) -> f64 {
        let clamp = |v: f64| v.clamp(0.0, 1.0);
        match self {
            Dist::Bernoulli(p) => match x {
                x if x < 0.0 => 0.0,
                x if x < 1.0 => 1.0 - p,
                _ => 1.0,
            },
            Dist::Beta(a, b) => beta_i(*a, *b, x),
            Dist::Binom(n, p) => match x.floor() {
                k if k < 0.0 => 0.0,
                k if k >= *n => 1.0,
                k => beta_i(n - k, k + 1.0, 1.0 - p),
            },
            Dist::Categorical(cats, probs) => cats
                .iter()
                .zip(probs)
                .filter(|(c, _)| **c <= x)
                .map(|(_, p)| p)
                .sum(),
            Dist::Empirical(q) => {
                let b = (q.len() - 1) as f64;
                match q.iter().position(|&v| v > x) {
                    None => 1.0,
                    Some(0) => 0.0,
                    Some(i) => (i as f64 - 1.0 + (x - q[i - 1]) / (q[i] - q[i - 1])) / b,
                }
            }
            Dist::Exp(r) => clamp(1.0 - (-r * x).exp()),
            Dist::Gamma(a, r) if x > 0.0 => 1.0 - gamma_q(*a, r * x),
            Dist::LNormal(a, b) if x > 0.0 => normal_cdf((x / a).ln() / b),
            Dist::Gamma(..) | Dist::LNormal(..) => 0.0,
            Dist::NegBin(r, p, trials) => {
                let k = (x - if *trials { *r } else { 0.0 }).floor();
                if k < 0.0 {
                    0.0
                } else {
                    beta_i(*r, k + 1.0, *p)
                }
            }
            Dist::Normal(m, s) => normal_cdf((x - m) / s),
            Dist::Poisson(l) if x >= 0.0 => gamma_q(x.floor() + 1.0, *l),
            Dist::Poisson(_) => 0.0,
            Dist::Triangular(a, b, m) => match x {
                x if x <= *a => 0.0,
                x if x < *m => (x - a).powi(2) / ((b - a) * (m - a)),
                x if x < *b => 1.0 - (b - x).powi(2) / ((b - a) * (b - m)),
                _ => 1.0,
            },
            Dist::Uniform(a, b) => clamp((x - a) / (b - a)),
            Dist::UserRanges(min, max, probs) => (0..probs.len())
                .map(|i| match max[i] > min[i] {
                    true => probs[i] * clamp((x - min[i]) / (max[i] - min[i])),
                    false => probs[i] * f64::from(u8::from(x >= min[i])),
                })
                .sum(),
            Dist::Weibull(a, b, c) => clamp(1.0 - (-((x - c).max(0.0) / a).powf(*b)).exp()),
        }
    }

    /// The smallest x with cdf(x) >= u, for 0 < u < 1.
    pub(crate) fn quantile(&self, u: f64) -> f64 {
        match self {
            Dist::Beta(a, b) => solve(|x| beta_i(*a, *b, x) - u, 0.0, 1.0),
            Dist::Gamma(a, r) => {
                let mut high = (a + 10.0 * a.sqrt() + 10.0) / r;
                while self.cdf(high) < u {
                    high *= 2.0;
                }
                solve(|x| self.cdf(x) - u, 0.0, high)
            }
            Dist::Categorical(cats, probs) => {
                let mut total = 0.0;
                for (c, p) in cats.iter().zip(probs) {
                    total += p;
                    if total >= u {
                        return *c;
                    }
                }
                *cats.last().expect("categories")
            }
            // one value: every quantile is that value (q.len() - 2 would underflow)
            Dist::Empirical(q) if q.len() == 1 => q[0],
            Dist::Empirical(q) => {
                let at = u * (q.len() - 1) as f64;
                let i = (at.floor() as usize).min(q.len() - 2);
                q[i] + (at - i as f64) * (q[i + 1] - q[i])
            }
            Dist::Exp(r) => -(1.0 - u).ln() / r,
            Dist::LNormal(a, b) => a * (b * normal_quantile(u)).exp(),
            Dist::Normal(m, s) => m + s * normal_quantile(u),
            Dist::Triangular(a, b, m) => match (m - a) / (b - a) {
                f if u < f => a + (u * (b - a) * (m - a)).sqrt(),
                _ => b - ((1.0 - u) * (b - a) * (b - m)).sqrt(),
            },
            Dist::Uniform(a, b) => a + u * (b - a),
            Dist::UserRanges(min, max, probs) => {
                let mut below = 0.0;
                for i in 0..probs.len() {
                    if below + probs[i] >= u && probs[i] > 0.0 {
                        return min[i] + (u - below) / probs[i] * (max[i] - min[i]);
                    }
                    below += probs[i];
                }
                *max.last().expect("ranges")
            }
            Dist::Weibull(a, b, c) => c + a * (-(1.0 - u).ln()).powf(1.0 / b),
            // whole numbers: from a normal guess down and up to the smallest k with cdf >= u
            _ => {
                let (mean, sd) = match self {
                    Dist::Bernoulli(p) => (*p, 0.5),
                    Dist::Binom(n, p) => (n * p, (n * p * (1.0 - p)).sqrt()),
                    Dist::NegBin(r, p, t) => (
                        r * (1.0 - p) / p + if *t { *r } else { 0.0 },
                        (r * (1.0 - p)).sqrt() / p,
                    ),
                    Dist::Poisson(l) => (*l, l.sqrt()),
                    _ => unreachable!("continuous distributions return above"),
                };
                let low = if let Dist::NegBin(r, _, true) = self {
                    *r
                } else {
                    0.0
                };
                let mut k = (mean + sd * normal_quantile(u)).floor().max(low);
                while k > low && self.cdf(k - 1.0) >= u {
                    k -= 1.0;
                }
                while self.cdf(k) < u && k < 1e15 {
                    k += 1.0;
                }
                k
            }
        }
    }

    /// The probability of the whole number `k` of a discrete distribution.
    fn mass(&self, k: f64) -> f64 {
        self.cdf(k) - self.cdf(k - 1.0)
    }
}

/// Weighted mean, sample variance (divisor W - 1) and total weight.
fn moments(x: &[f64], w: &[f64]) -> (f64, f64, f64) {
    let total: f64 = w.iter().sum();
    let mean = x.iter().zip(w).map(|(x, w)| x * w).sum::<f64>() / total;
    let ss: f64 = x.iter().zip(w).map(|(x, w)| w * (x - mean).powi(2)).sum();
    (mean, ss / (total - 1.0), total)
}

/// ψ(x), the derivative of ln Γ, for x > 0: recurrence up to 12, then the asymptotic series.
pub(crate) fn digamma(mut x: f64) -> f64 {
    let mut shift = 0.0;
    while x < 12.0 {
        shift -= 1.0 / x;
        x += 1.0;
    }
    let f = 1.0 / (x * x);
    shift + x.ln() - 0.5 / x - f * (1.0 / 12.0 - f * (1.0 / 120.0 - f * (1.0 / 252.0 - f / 240.0)))
}

/// ψ'(x) for x > 0, the same way.
fn trigamma(mut x: f64) -> f64 {
    let mut shift = 0.0;
    while x < 12.0 {
        shift += 1.0 / (x * x);
        x += 1.0;
    }
    let f = 1.0 / (x * x);
    shift
        + 1.0 / x
        + f / 2.0
        + (1.0 / 6.0 - f * (1.0 / 30.0 - f * (1.0 / 42.0 - f / 30.0))) / (x * x * x)
}

/// The root of a decreasing `f` above 0, bracketed by halving and doubling from `start`.
fn positive_root(f: impl Fn(f64) -> f64, start: f64) -> Option<f64> {
    let (mut low, mut high) = (start, start);
    while f(low) <= 0.0 && low > 1e-12 {
        low /= 2.0;
    }
    while f(high) >= 0.0 && high < 1e12 {
        high *= 2.0;
    }
    let root = solve(&f, low, high);
    (root.is_finite() && f(low) > 0.0 && f(high) < 0.0).then_some(root)
}

/// The maximum likelihood (or, where the algorithms say so, moment) parameters of `family` for
/// the values `x` with weights `w`; None when the family cannot describe the data.
pub(crate) fn fit(family: &str, x: &[f64], w: &[f64], iterations: usize) -> Option<Params> {
    let (mean, var, total) = moments(x, w);
    let min = x.iter().copied().fold(f64::INFINITY, f64::min);
    let max = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let whole = x.iter().all(|v| v.fract() == 0.0 && *v >= 0.0);
    let wsum = |f: &dyn Fn(f64) -> f64| x.iter().zip(w).map(|(x, w)| w * f(*x)).sum::<f64>();
    let named = |values: &[f64]| -> Params {
        let names = PARAMS.iter().find(|(f, _)| *f == family).expect("known").1;
        names
            .iter()
            .zip(values)
            .map(|(n, v)| (n.to_string(), vec![*v]))
            .collect()
    };
    if total <= 1.0 || !mean.is_finite() {
        return None;
    }
    let values = match family {
        "NORMAL" if var > 0.0 => vec![mean, (var * (total - 1.0) / total).sqrt()],
        "LNORMAL" if min > 0.0 => {
            let lm = wsum(&|v| v.ln()) / total;
            let s = (wsum(&|v| (v.ln() - lm).powi(2)) / total).sqrt();
            vec![lm.exp(), s]
        }
        "EXP" if min >= 0.0 && mean > 0.0 => vec![1.0 / mean],
        "UNIFORM" if min < max => vec![min, max],
        "TRIANGULAR" if min < max => {
            let z = (mean - min) / (max - min);
            vec![
                min,
                max,
                min + (max - min) * (3.0 * z - 1.0).clamp(0.0, 1.0),
            ]
        }
        "GAMMA" if min > 0.0 && var > 0.0 => {
            let s = mean.ln() - wsum(&|v| v.ln()) / total;
            let shape = positive_root(|a| a.ln() - digamma(a) - s, mean * mean / var)?;
            vec![shape, shape / mean]
        }
        "WEIBULL" if min > 0.0 && var > 0.0 => {
            // on x / max the powers stay finite; the equation does not change
            let t: Vec<f64> = x.iter().map(|v| v / max).collect();
            let lt = t.iter().zip(w).map(|(t, w)| w * t.ln()).sum::<f64>() / total;
            let sums = |g: f64| {
                t.iter().zip(w).fold((0.0, 0.0), |(a, b), (t, w)| {
                    (a + w * t.powf(g), b + w * t.powf(g) * t.ln())
                })
            };
            let shape = positive_root(
                |g| {
                    let (a, b) = sums(g);
                    lt + 1.0 / g - b / a
                },
                1.0,
            )?;
            vec![max * (sums(shape).0 / total).powf(1.0 / shape), shape, 0.0]
        }
        "BETA" if min > 0.0 && max < 1.0 && var > 0.0 => {
            let (l1, l2) = (wsum(&|v| v.ln()), wsum(&|v| (1.0 - v).ln()));
            let ll = |a: f64, b: f64| {
                total * (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b))
                    + (a - 1.0) * l1
                    + (b - 1.0) * l2
            };
            let k = (mean * (1.0 - mean) / var - 1.0).max(0.01);
            let (mut a, mut b) = (mean * k, (1.0 - mean) * k);
            for _ in 0..iterations {
                let (d, da, db) = (digamma(a + b), digamma(a), digamma(b));
                let g = [total * (d - da) + l1, total * (d - db) + l2];
                let t = trigamma(a + b);
                let h = [
                    total * (t - trigamma(a)),
                    total * t,
                    total * (t - trigamma(b)),
                ];
                let det = h[0] * h[2] - h[1] * h[1];
                let step = [
                    (h[2] * g[0] - h[1] * g[1]) / det,
                    (h[0] * g[1] - h[1] * g[0]) / det,
                ];
                let mut f = 1.0;
                let before = ll(a, b);
                while f > 1e-10 {
                    let (na, nb) = (a - f * step[0], b - f * step[1]);
                    if na > 0.0 && nb > 0.0 && ll(na, nb) >= before {
                        (a, b) = (na, nb);
                        break;
                    }
                    f /= 2.0;
                }
                if f <= 1e-10 || (step[0].abs() < 1e-12 * a && step[1].abs() < 1e-12 * b) {
                    break;
                }
            }
            vec![a, b]
        }
        "POISSON" if whole && mean > 0.0 => vec![mean],
        "BINOM" if whole && var < mean => {
            let n = (mean / (1.0 - var / mean) + 0.5).floor().max(max);
            vec![n, mean / n]
        }
        "NEGBIN" if whole && var > mean => {
            let r = positive_root(
                |r| wsum(&|v| digamma(v + r)) - total * digamma(r) + total * (r / (r + mean)).ln(),
                mean * mean / (var - mean),
            )?;
            vec![r, r / (r + mean)]
        }
        "CATEGORICAL" => {
            let mut pairs: Vec<(f64, f64)> = x.iter().copied().zip(w.iter().copied()).collect();
            pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
            let (mut cats, mut probs): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
            for (v, w) in pairs {
                match cats.last() {
                    Some(&c) if c == v => *probs.last_mut().expect("paired") += w / total,
                    _ => {
                        cats.push(v);
                        probs.push(w / total);
                    }
                }
            }
            return Some(vec![("CATS".into(), cats), ("PROBS".into(), probs)]);
        }
        _ => return None,
    };
    values.iter().all(|v| v.is_finite()).then(|| named(&values))
}

/// The empirical distribution of continuous values: their weighted quantiles at 0, 1/B, ..., 1.
pub(crate) fn empirical(x: &[f64], w: &[f64], bins: usize) -> Params {
    let mut pairs: Vec<(f64, f64)> = x.iter().copied().zip(w.iter().copied()).collect();
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = w.iter().sum();
    let mut cum = 0.0;
    let mut cums = Vec::with_capacity(pairs.len());
    for (_, w) in &pairs {
        cum += w;
        cums.push(cum);
    }
    let points = (0..=bins)
        .map(|k| {
            let at = total * k as f64 / bins as f64;
            let i = cums.partition_point(|&c| c < at - 1e-9 * total);
            pairs[i.min(pairs.len() - 1)].0
        })
        .collect();
    vec![("POINTS".into(), points)]
}

/// One tested distribution: its parameters, the statistic and its p-value when known.
pub(crate) struct Fitted {
    pub family: &'static str,
    pub params: Params,
    pub statistic: f64,
    pub p: Option<f64>,
}

/// The upper-tail probability by linear interpolation in a table of critical values that rise
/// as the probabilities fall; beyond the table its first or last probability.
fn table_p(z: f64, probs: &[f64], critical: &[f64]) -> f64 {
    if z <= critical[0] {
        return probs[0];
    }
    for i in 1..critical.len() {
        if z <= critical[i] {
            let f = (z - critical[i - 1]) / (critical[i] - critical[i - 1]);
            return probs[i - 1] + f * (probs[i] - probs[i - 1]);
        }
    }
    *probs.last().expect("a table")
}

/// The piecewise p-value formula of the algorithms: (upper bound, a, b, c, upper tail).
fn piecewise(z: f64, pieces: &[(f64, f64, f64, f64, bool)]) -> f64 {
    for &(bound, a, b, c, upper) in pieces {
        if z <= bound {
            let e = (a + b * z + c * z * z).exp();
            return if upper { e } else { 1.0 - e };
        }
    }
    0.0
}

/// Anderson-Darling (or Kolmogorov-Smirnov) of continuous values against `d`, with the
/// p-value where the algorithms give one.
fn continuous_fit(family: &str, d: &Dist, x: &[f64], w: &[f64], ks: bool) -> (f64, Option<f64>) {
    let mut y: Vec<(f64, f64)> = x
        .iter()
        .zip(w)
        .map(|(x, w)| (d.cdf(*x).clamp(1e-15, 1.0 - 1e-15), *w))
        .collect();
    y.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = w.iter().sum();
    let (mut before, mut ad, mut dn) = (0.0, -total, 0.0f64);
    for (y, w) in &y {
        let after = before + w;
        ad += w * (2.0 * before + w) * ((1.0 - y).ln() - y.ln()) / total - 2.0 * w * (1.0 - y).ln();
        dn = dn
            .max((y - after / total).abs())
            .max((y - before / total).abs());
        before = after;
    }
    let root = total.sqrt();
    let p = match (ks, family) {
        (false, "UNIFORM") if ad < 2.0 => {
            let g = 2.00012
                + (0.247105
                    - (0.0649821 - (0.0347962 - (0.0116720 - 0.00168691 * ad) * ad) * ad) * ad)
                    * ad;
            Some(1.0 - (-1.2337141 / ad).exp() / ad.sqrt() * g)
        }
        (false, "UNIFORM") => Some(
            (-(1.0776
                - (2.30695 - (0.43424 - (0.082433 - (0.008056 - 0.0003146 * ad) * ad) * ad) * ad)
                    * ad)
                .exp())
            .exp(),
        ),
        (false, "NORMAL" | "LNORMAL") => Some(piecewise(
            ad * (1.0 + 0.75 / total + 2.25 / (total * total)),
            &[
                (0.2, -13.436, 101.14, -223.73, false),
                (0.34, -8.318, 42.796, -59.938, false),
                (0.6, 0.9177, -4.279, -1.38, true),
                (153.467, 1.2937, -5.709, 0.0186, true),
            ],
        )),
        (false, "EXP") => Some(piecewise(
            ad * (1.0 + 0.6 / total),
            &[
                (0.26, -12.2204, 67.459, -110.3, false),
                (0.51, -6.1327, 20.218, -18.663, false),
                (0.95, 0.9209, -3.353, 0.3, true),
                (10.03, 0.731, -3.009, 0.15, true),
            ],
        )),
        (false, "WEIBULL") => Some(table_p(
            ad * (1.0 + 0.2 / root),
            &[0.25, 0.10, 0.05, 0.025, 0.01],
            &[0.474, 0.637, 0.757, 0.877, 1.038],
        )),
        (false, "GAMMA") => {
            let shape = if let Dist::Gamma(a, _) = d { *a } else { 1.0 };
            let row: [f64; 6] = match shape {
                s if s <= 1.0 => [0.486, 0.657, 0.786, 0.917, 1.092, 1.227],
                s if s <= 8.0 => [0.473, 0.637, 0.759, 0.883, 1.048, 1.173],
                _ => [0.470, 0.631, 0.752, 0.873, 1.035, 1.159],
            };
            Some(table_p(ad, &[0.25, 0.10, 0.05, 0.025, 0.01, 0.005], &row))
        }
        (true, "UNIFORM") => {
            let z = root * dn;
            let sum: f64 = (1..200)
                .map(|i| 2.0 * (-1f64).powi(i - 1) * (-2.0 * (f64::from(i) * z).powi(2)).exp())
                .sum();
            Some(sum.clamp(0.0, 1.0))
        }
        (true, "NORMAL" | "LNORMAL") => Some(table_p(
            dn * (root - 0.01 + 0.85 / root),
            &[0.15, 0.10, 0.05, 0.025, 0.01],
            &[0.775, 0.819, 0.895, 0.995, 1.035],
        )),
        (true, "EXP") => Some(table_p(
            (dn - 0.2 / total) * (root + 0.26 + 0.5 / root),
            &[0.15, 0.10, 0.05, 0.025, 0.01],
            &[0.926, 0.995, 1.094, 1.184, 1.298],
        )),
        (true, "WEIBULL") => Some(table_p(
            root * dn,
            &[0.10, 0.05, 0.025, 0.01],
            &[1.372, 1.477, 1.557, 1.671],
        )),
        (true, "GAMMA") => Some(table_p(
            dn * (root + 0.3 / root),
            &[0.25, 0.20, 0.15, 0.10, 0.05, 0.025, 0.01, 0.005],
            &[0.74, 0.780, 0.800, 0.858, 0.928, 0.990, 1.069, 1.13],
        )),
        _ => None,
    };
    (if ks { dn } else { ad }, p)
}

/// Chi-square of whole numbers (or categories) against a discrete fit.
fn chi_square(d: &Dist, x: &[f64], w: &[f64], parameters: f64) -> (f64, Option<f64>) {
    let total: f64 = w.iter().sum();
    let classes: Vec<f64> = match d {
        Dist::Categorical(cats, _) => cats.clone(),
        Dist::Binom(n, _) => (0..=*n as usize).map(|k| k as f64).collect(),
        _ => {
            let max = x.iter().copied().fold(0.0, f64::max);
            (0..=max as usize).map(|k| k as f64).collect()
        }
    };
    let last = classes.len() - 1;
    let mut t = 0.0;
    for (i, &c) in classes.iter().enumerate() {
        let observed: f64 = x
            .iter()
            .zip(w)
            .filter(|(x, _)| **x == c)
            .map(|(_, w)| w)
            .sum();
        let p = match i == last && !matches!(d, Dist::Categorical(..) | Dist::Binom(..)) {
            true => 1.0 - d.cdf(c - 1.0),
            false => d.mass(c),
        };
        let expected = total * p;
        t += match expected > 0.0 {
            true => (observed - expected).powi(2) / expected,
            false if observed > 0.0 => f64::INFINITY,
            false => 0.0,
        };
    }
    let df = classes.len() as f64 - 1.0 - parameters;
    (t, (df > 0.0).then(|| chi_square_p(t, df)))
}

/// Fits every family of `families` to the values and orders them by the statistic, smallest
/// (best) first.
pub(crate) fn autofit(
    families: &[&'static str],
    x: &[f64],
    w: &[f64],
    ks: bool,
    iterations: usize,
) -> Vec<Fitted> {
    let mut out: Vec<Fitted> = families
        .iter()
        .filter_map(|&family| {
            let params = fit(family, x, w, iterations)?;
            let d = Dist::new(family, &params, false).ok()?;
            let (statistic, p) = match d.discrete() {
                true => {
                    let estimated = match &d {
                        Dist::Categorical(cats, _) => cats.len() as f64 - 1.0,
                        Dist::Poisson(_) => 1.0,
                        _ => 2.0,
                    };
                    chi_square(&d, x, w, estimated)
                }
                false => continuous_fit(family, &d, x, w, ks),
            };
            (!statistic.is_nan()).then_some(Fitted {
                family,
                params,
                statistic,
                p,
            })
        })
        .collect();
    out.sort_by(|a, b| a.statistic.total_cmp(&b.statistic));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digamma_and_trigamma_at_known_points() {
        // ψ(1) = -γ, ψ'(1) = π²/6, ψ(0.5) = -γ - 2 ln 2
        let euler = std::f64::consts::EULER_GAMMA;
        assert!((digamma(1.0) + euler).abs() < 1e-12);
        assert!((digamma(0.5) + euler + 2.0 * 2f64.ln()).abs() < 1e-12);
        assert!((trigamma(1.0) - std::f64::consts::PI.powi(2) / 6.0).abs() < 1e-10);
    }

    #[test]
    fn an_empirical_distribution_of_one_value() {
        let one = Dist::Empirical(vec![4.5]);
        assert_eq!(one.quantile(0.0), 4.5);
        assert_eq!(one.quantile(0.9), 4.5);
    }

    #[test]
    fn quantiles_invert_the_distribution_functions() {
        let ds = [
            Dist::Gamma(2.5, 0.5),
            Dist::Beta(2.0, 5.0),
            Dist::Weibull(3.0, 1.5, 1.0),
            Dist::Triangular(1.0, 4.0, 2.0),
            Dist::LNormal(2.0, 0.5),
            Dist::UserRanges(vec![0.0, 5.0], vec![1.0, 10.0], vec![0.25, 0.75]),
        ];
        for d in ds {
            for u in [0.01, 0.3, 0.5, 0.9, 0.999] {
                assert!((d.cdf(d.quantile(u)) - u).abs() < 1e-9, "{d:?} {u}");
            }
        }
        // whole numbers: the smallest k with F(k) >= u
        let poisson = Dist::Poisson(3.0);
        assert_eq!(poisson.quantile(0.5), 3.0);
        assert_eq!(poisson.quantile(0.05), 1.0);
        let negbin = Dist::NegBin(2.0, 0.5, true);
        assert_eq!(negbin.quantile(0.25), 2.0);
        assert_eq!(Dist::Binom(10.0, 0.3).quantile(0.6496), 3.0);
    }
}
