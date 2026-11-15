//! The POWER commands for means and proportions: the power of a planned test for given sample
//! sizes, or the smallest sample sizes that reach given powers. They need no data.
//!
//! Formulas from the Algorithms manual, in our words:
//! - t-tests: the power is the chance that a noncentral t passes the critical value of the
//!   central t. One sample and pairs: df n - 1, noncentrality (difference / sd) sqrt(n).
//!   Two groups with one sd: df n1 + n2 - 2, noncentrality difference / (sd sqrt(1/n1 + 1/n2));
//!   with two sds the Welch df and difference / sqrt(sd1^2/n1 + sd2^2/n2). A one-sided test
//!   uses the absolute noncentrality.
//! - One proportion, normal approximation: the z test at the null with the spread of the
//!   alternative, the continuity correction adding 1 / (2 sqrt(n)). Exact: the binomial
//!   rejection region at the null, its probability at the alternative.
//! - Two proportions: the z (chi-square) test with pooled or separate standard error and a
//!   continuity correction of (1/n1 + 1/n2) / 2, the t-test of the 0/1 values, the likelihood
//!   ratio statistic taken as normal, and Fisher's test by the arcsine approximation. Exact:
//!   every pair of outcomes is tested and the probabilities of those rejected are summed; for
//!   Fisher's test the conditional test at every total number of successes.
//! - Pairs (McNemar): the normal approximation in the difference and sum of the discordant
//!   shares; exact: the conditional binomial test of the discordant pairs averaged over their
//!   number.
//! - One-way analysis of variance: the overall F test has the noncentral F with J - 1 and N - J
//!   df and noncentrality sum n_j (mean_j - weighted mean)^2 / sd^2; a contrast c has the
//!   noncentral t with N - J df and noncentrality sum c_j mean_j / (sd sqrt(sum c_j^2 / n_j)); the
//!   pairwise comparisons are such contrasts, two-sided, at the adjusted significance level.
//! - Correlations: Fisher's z with the normal approximation. The test statistic has mean delta
//!   and variance nu under the alternative, the critical values +-z at the null. Without the bias
//!   adjustment delta is the z difference times sqrt(n - 3 - k) and nu is 1; with it the
//!   second-order terms in 1 / (n - 1 - k) of the mean and variance of z are added. A rank
//!   correlation has delta = z difference / sqrt(V) for the chosen variance V and nu = 1.
//! - Linear regression, q of p predictors tested: F with q and N - p - 1 df (N - p without the
//!   intercept). Fixed predictors: noncentrality N r^2 / (1 - r^2). Random predictors: the sample
//!   squared multiple partial correlation is a negative binomial mixture of beta variables, so
//!   the power is a sum of incomplete beta functions weighted by negative binomial terms.
//! - Sample size: bisection in the size from 2 (or 1) and 400, doubling the upper end until
//!   the power is passed, to 1e-9, then rounded up; the second group gets the ratio times the
//!   unrounded first size, rounded up. One-sided normal approximations of proportions use
//!   closed formulas. Exact powers rise in steps, so the search is repeated below its result
//!   until it settles, the smallest result is kept and the size is then raised one at a time
//!   until the power is reached.
//!
//! Choices where the manual is silent are listed in docs/differences.md under POWER.

use crate::chart::{Axis, ChartKind};
use crate::dist::{
    beta_i, f_quantile, ln_gamma, noncentral_f_cdf, noncentral_t_cdf, normal_cdf, normal_quantile,
    t_quantile,
};
use crate::npar_tests::{binomial_cdf, ln_choose};
use crate::plot::{Plot, Point, PointGroup};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_syntax::{
    PairwiseAdjust, Power, PowerKind, PowerPlot, ProportionTest, RankVariance,
};
use std::cell::Cell as Flag;
use std::time::{Duration, Instant};

/// Above this many trials the binomial enumeration gives way to the normal approximation.
const EXACT_LIMIT: f64 = 500.0;
/// Bisection gives up when the size passes this.
const MAX_SIZE: f64 = 2_147_483_647.0;

/// A planned test with everything its power depends on.
#[derive(Clone, Debug)]
struct Model {
    kind: PowerKind,
    alpha: f64,
    two_sided: bool,
    n1: f64,
    /// The second group of two independent groups.
    n2: f64,
    /// The mean difference (from the null value for one sample).
    diff: f64,
    /// The standard deviation; for pairs that of the differences.
    sd1: f64,
    /// The second group's standard deviation; equal to `sd1` with one SD.
    sd2: f64,
    equal: bool,
    /// One proportion: null and alternative. Two: p1 and p2. Pairs: the discordant shares
    /// a (success first, failure second) and b (the reverse).
    p: [f64; 2],
    /// Pairs given by their marginal success rates, kept for the charts.
    marginal: Option<[f64; 2]>,
    correlation: f64,
    method: ProportionTest,
    pooled: bool,
    continuity: bool,
    binomial: bool,
    /// ONEWAY ANOVA: the group sizes and means, and the contrast tested (empty for the overall
    /// F test). `sd1` is the pooled standard deviation.
    groups: Vec<f64>,
    means: Vec<f64>,
    contrast: Vec<f64>,
    /// Correlations (`p` holds the null and the alternative): the variables partialled out, the
    /// bias adjustment and the variance of a rank correlation.
    partialout: f64,
    bias: bool,
    variance: RankVariance,
    /// UNIVARIATE LINEAR: predictors in all and tested, the multiple partial correlation and,
    /// when given that way, the squared multiple correlations of the full and nested model.
    predictors: f64,
    tested: f64,
    intercept: bool,
    random: bool,
    partial: f64,
    squares: Option<[f64; 2]>,
}

/// The value of t with `df` exceeded with probability `upper`.
fn t_upper(upper: f64, df: f64) -> f64 {
    if upper <= 0.5 {
        t_quantile(upper, df)
    } else {
        -t_quantile(1.0 - upper, df)
    }
}

/// The standard normal value exceeded with probability `upper`.
fn z_upper(upper: f64) -> f64 {
    -normal_quantile(upper)
}

fn t_power(df: f64, ncp: f64, alpha: f64, two_sided: bool) -> f64 {
    if df.is_nan() || df <= 0.0 || ncp.is_nan() {
        return f64::NAN;
    }
    if two_sided {
        let t = t_upper(alpha / 2.0, df);
        1.0 - noncentral_t_cdf(t, df, ncp) + noncentral_t_cdf(-t, df, ncp)
    } else {
        1.0 - noncentral_t_cdf(t_upper(alpha, df), df, ncp.abs())
    }
}

/// P(R^2 > x) for the squared multiple partial correlation R^2 of a test of `u` predictors with
/// `v` error df, when predictors and outcome are jointly normal with population value `rho2`:
/// a negative binomial (u + v) / 2, 1 - rho2 mixture of beta (u / 2 + k, v / 2) variables, summed
/// from the largest weight outwards.
fn random_r2_upper(x: f64, u: f64, v: f64, rho2: f64) -> f64 {
    // P(beta(u/2 + k, v/2) > x), which rises to 1 with k
    let upper = |k: f64| beta_i(v / 2.0, u / 2.0 + k, 1.0 - x);
    if rho2 <= 0.0 {
        return upper(0.0);
    }
    let h = (u + v) / 2.0;
    let weight = |k: f64| {
        (ln_gamma(h + k) - ln_gamma(h) - ln_gamma(k + 1.0) + k * rho2.ln() + h * (1.0 - rho2).ln())
            .exp()
    };
    let top = ((h - 1.0) * rho2 / (1.0 - rho2)).floor().max(0.0);
    if top + 1.0 == top {
        return 1.0;
    }
    let (mut sum, mut weights) = (0.0, 0.0);
    for i in 0..=100_000 {
        let k = top - f64::from(i);
        let w = weight(k);
        sum += w * upper(k);
        weights += w;
        if k == 0.0 || (i > 5 && w < 1e-17 * weights) {
            break;
        }
    }
    for i in 1..=100_000 {
        let k = top + f64::from(i);
        let w = weight(k);
        let p = upper(k);
        if p > 1.0 - 1e-15 {
            // every later term is its weight alone: they make up the rest of the weight
            sum += (1.0 - weights).max(0.0);
            break;
        }
        sum += w * p;
        weights += w;
        if i > 5 && w < 1e-17 * weights {
            break;
        }
    }
    sum.clamp(0.0, 1.0)
}

/// The binomial probabilities of 0 to n successes.
fn binomial_pmf(n: usize, p: f64) -> Vec<f64> {
    let nf = n as f64;
    (0..=n)
        .map(|k| {
            let k = k as f64;
            let log = ln_choose(nf, k) + xlogy(k, p) + xlogy(nf - k, 1.0 - p);
            log.exp()
        })
        .collect()
}

/// x ln y, 0 when x is 0.
fn xlogy(x: f64, y: f64) -> f64 {
    if x == 0.0 { 0.0 } else { x * y.ln() }
}

/// x ln x, 0 when x is 0.
fn xlogx(x: f64) -> f64 {
    xlogy(x, x)
}

/// The largest k (from -1) whose lower tail is at most `level`, and the smallest k whose
/// lower tail reaches 1 - `level`, of the probabilities `pmf`.
fn critical(pmf: &[f64], level: f64) -> (i64, i64) {
    let slack = 1e-12;
    let mut below = -1;
    let mut sum = 0.0;
    for (k, p) in pmf.iter().enumerate() {
        sum += p;
        if sum <= level + slack {
            below = k as i64;
        } else {
            break;
        }
    }
    // the upper tail from the top, so it keeps its precision
    let mut above = pmf.len() as i64 - 1;
    let mut tail = 0.0;
    for k in (1..pmf.len()).rev() {
        tail += pmf[k];
        if tail <= level + slack {
            above = k as i64 - 1;
        } else {
            break;
        }
    }
    (below, above)
}

/// The sum of `pmf` up to `k` and above `k`.
fn split(pmf: &[f64], k: i64) -> (f64, f64) {
    let cut = (k + 1).clamp(0, pmf.len() as i64) as usize;
    (pmf[..cut].iter().sum(), pmf[cut..].iter().sum())
}

impl Model {
    fn power(&self) -> f64 {
        let (alpha, two) = (self.alpha, self.two_sided);
        match self.kind {
            PowerKind::MeansOneSample | PowerKind::MeansRelated => t_power(
                self.n1 - 1.0,
                self.diff / self.sd1 * self.n1.sqrt(),
                alpha,
                two,
            ),
            PowerKind::MeansIndependent => {
                let (n1, n2) = (self.n1, self.n2);
                if self.equal {
                    let ncp = self.diff / (self.sd1 * (1.0 / n1 + 1.0 / n2).sqrt());
                    t_power(n1 + n2 - 2.0, ncp, alpha, two)
                } else {
                    let (v1, v2) = (self.sd1.powi(2) / n1, self.sd2.powi(2) / n2);
                    let df = (v1 + v2).powi(2) / (v1 * v1 / (n1 - 1.0) + v2 * v2 / (n2 - 1.0));
                    t_power(df, self.diff / (v1 + v2).sqrt(), alpha, two)
                }
            }
            PowerKind::ProportionsOneSample => {
                if self.binomial && self.n1 <= EXACT_LIMIT {
                    self.one_exact()
                } else {
                    self.one_normal()
                }
            }
            PowerKind::ProportionsIndependent => {
                if self.binomial && self.n1 + self.n2 <= EXACT_LIMIT {
                    self.two_exact()
                } else {
                    self.two_normal()
                }
            }
            PowerKind::ProportionsRelated => {
                if self.binomial && self.n1 <= EXACT_LIMIT {
                    self.pairs_exact()
                } else {
                    self.pairs_normal()
                }
            }
            PowerKind::OnewayAnova => self.anova_power(),
            PowerKind::PartialCorr | PowerKind::PearsonOneSample | PowerKind::SpearmanOneSample => {
                self.correlation_power()
            }
            PowerKind::UnivariateLinear => self.regression_power(),
        }
    }

    fn anova_power(&self) -> f64 {
        let n: f64 = self.groups.iter().sum();
        let df = n - self.groups.len() as f64;
        if self.contrast.is_empty() {
            let mean = dot(&self.groups, &self.means) / n;
            let lambda = self
                .groups
                .iter()
                .zip(&self.means)
                .map(|(g, m)| g * (m - mean).powi(2))
                .sum::<f64>()
                / self.sd1.powi(2);
            let df1 = self.groups.len() as f64 - 1.0;
            if df <= 0.0 {
                return f64::NAN;
            }
            1.0 - noncentral_f_cdf(f_quantile(self.alpha, df1, df), df1, df, lambda)
        } else {
            let spread: f64 = self
                .contrast
                .iter()
                .zip(&self.groups)
                .map(|(c, g)| c * c / g)
                .sum();
            let ncp = dot(&self.contrast, &self.means) / (self.sd1 * spread.sqrt());
            t_power(df, ncp, self.alpha, self.two_sided)
        }
    }

    /// Fisher's z test of a correlation, partial correlation or rank correlation.
    fn correlation_power(&self) -> f64 {
        let [r0, r1] = self.p;
        let n = self.n1;
        let k = self.partialout;
        let d = if r0 == r1 {
            0.0
        } else {
            r1.atanh() - r0.atanh()
        };
        if d.is_infinite() {
            // a correlation of 1 or -1 on one side only is told apart for certain
            return 1.0;
        }
        let r2 = r1 * r1;
        let (delta, nu) = if self.kind == PowerKind::SpearmanOneSample {
            let v = match self.variance {
                RankVariance::Bw => (1.0 + r2 / 2.0) / (n - 3.0),
                RankVariance::Fhp => 1.06 / (n - 3.0),
                RankVariance::Cc => 1.0 / (n - 2.0) + r1.atanh().abs() / (6.0 * n + 4.0 * n.sqrt()),
            };
            (d / v.sqrt(), 1.0)
        } else if self.bias {
            let v = 1.0 / (n - 3.0 - k);
            let l = n - 1.0 - k;
            let shift = r1 / (2.0 * l)
                * (1.0
                    + (5.0 + r2) / (4.0 * l)
                    + (11.0 + 2.0 * r2 + 3.0 * r2 * r2) / (8.0 * l * l))
                - r0 / (2.0 * l);
            let spread =
                1.0 + (4.0 - r2) / (2.0 * l) + (22.0 - 6.0 * r2 - 3.0 * r2 * r2) / (6.0 * l * l);
            ((d + shift) / v.sqrt(), spread / (l * v))
        } else {
            (d * (n - 3.0 - k).sqrt(), 1.0)
        };
        let sd = nu.sqrt();
        if self.two_sided {
            let z = z_upper(self.alpha / 2.0);
            1.0 - normal_cdf((delta + z) / sd) + normal_cdf((delta - z) / sd)
        } else {
            let z = z_upper(self.alpha);
            if r1 >= r0 {
                normal_cdf((delta - z) / sd)
            } else {
                1.0 - normal_cdf((delta + z) / sd)
            }
        }
    }

    fn regression_power(&self) -> f64 {
        let u = self.tested;
        let v = self.n1 - self.predictors - if self.intercept { 1.0 } else { 0.0 };
        if v <= 0.0 {
            return f64::NAN;
        }
        let rho2 = self.partial * self.partial;
        let f = f_quantile(self.alpha, u, v);
        if self.random {
            random_r2_upper(u * f / (u * f + v), u, v, rho2)
        } else {
            1.0 - noncentral_f_cdf(f, u, v, self.n1 * rho2 / (1.0 - rho2))
        }
    }

    fn one_normal(&self) -> f64 {
        let [p0, p1] = self.p;
        let n = self.n1;
        let (s0, s1) = ((p0 * (1.0 - p0)).sqrt(), (p1 * (1.0 - p1)).sqrt());
        let c = if self.continuity { 0.5 / n.sqrt() } else { 0.0 };
        let shift = n.sqrt() * (p0 - p1);
        if self.two_sided {
            let z = z_upper(self.alpha / 2.0);
            1.0 - normal_cdf((shift + z * s0 + c) / s1) + normal_cdf((shift - z * s0 - c) / s1)
        } else {
            let z = z_upper(self.alpha);
            if p1 >= p0 {
                1.0 - normal_cdf((shift + z * s0 + c) / s1)
            } else {
                normal_cdf((shift - z * s0 - c) / s1)
            }
        }
    }

    fn one_exact(&self) -> f64 {
        let n = self.n1.round() as usize;
        let [p0, p1] = self.p;
        let (null, alternative) = (binomial_pmf(n, p0), binomial_pmf(n, p1));
        if self.two_sided {
            let (low, high) = critical(&null, self.alpha / 2.0);
            split(&alternative, low).0 + split(&alternative, high).1
        } else if p1 >= p0 {
            split(&alternative, critical(&null, self.alpha).1).1
        } else {
            split(&alternative, critical(&null, self.alpha).0).0
        }
    }

    fn two_normal(&self) -> f64 {
        let (n1, n2) = (self.n1, self.n2);
        let [p1, p2] = self.p;
        let (q1, q2) = (1.0 - p1, 1.0 - p2);
        let d = p1 - p2;
        let level = if self.two_sided {
            self.alpha / 2.0
        } else {
            self.alpha
        };
        let z = z_upper(level);
        // Fisher's exact test asked for beyond the exact limit falls back to the z test
        let method = match self.method {
            ProportionTest::Fisher if self.binomial => ProportionTest::ChiSquare,
            other => other,
        };
        match method {
            ProportionTest::ChiSquare => {
                let sd = (p1 * q1 / n1 + p2 * q2 / n2).sqrt();
                let pbar = (n1 * p1 + n2 * p2) / (n1 + n2);
                let sd0 = if self.pooled {
                    (pbar * (1.0 - pbar) * (1.0 / n1 + 1.0 / n2)).sqrt()
                } else {
                    sd
                };
                let c = if self.continuity {
                    0.5 * (1.0 / n1 + 1.0 / n2)
                } else {
                    0.0
                };
                let upper = 1.0 - normal_cdf((d + z * sd0 + c) / sd);
                let lower = normal_cdf((d - z * sd0 - c) / sd);
                if self.two_sided {
                    upper + lower
                } else if d >= 0.0 {
                    lower
                } else {
                    upper
                }
            }
            ProportionTest::T => {
                let spread = if self.pooled {
                    n1 * p1 * q1 + n2 * p2 * q2
                } else {
                    n1 * p2 * q2 + n2 * p1 * q1
                };
                let ncp = (n1 * n2).sqrt() * d / spread.sqrt();
                t_power(n1 + n2 - 2.0, ncp, self.alpha, self.two_sided)
            }
            ProportionTest::LikelihoodRatio => {
                let pbar = (n1 * p1 + n2 * p2) / (n1 + n2);
                let qbar = 1.0 - pbar;
                let g2 = 2.0
                    * (n1 * p1 * (p1 / pbar).ln()
                        + n1 * q1 * (q1 / qbar).ln()
                        + n2 * p2 * (p2 / pbar).ln()
                        + n2 * q2 * (q2 / qbar).ln());
                let g = g2.max(0.0).sqrt();
                if self.two_sided {
                    1.0 - normal_cdf(g + z) + normal_cdf(g - z)
                } else {
                    normal_cdf(g - z)
                }
            }
            ProportionTest::Fisher => {
                // the arcsine approximation holds only with enough successes and failures
                let valid = if p1 > p2 {
                    n1 >= 0.5 / p1 && n2 >= 0.5 / q2
                } else if p2 > p1 {
                    n1 >= 0.5 / q1 && n2 >= 0.5 / p2
                } else {
                    true
                };
                if !valid {
                    return f64::NAN;
                }
                let scale = 2.0 * (n1 * n2).sqrt() / (n1 + n2).sqrt();
                let asin = |x: f64| x.sqrt().asin();
                let delta = if p1 > p2 {
                    scale * (asin(p2 + 0.5 / n2) - asin(p1 - 0.5 / n1))
                } else if p1 < p2 {
                    scale * (asin(p2 - 0.5 / n2) - asin(p1 + 0.5 / n1))
                } else {
                    0.0
                };
                if self.two_sided {
                    1.0 - normal_cdf(delta + z) + normal_cdf(delta - z)
                } else if p1 >= p2 {
                    1.0 - normal_cdf(delta + z)
                } else {
                    normal_cdf(delta - z)
                }
            }
        }
    }

    fn two_exact(&self) -> f64 {
        let (n1, n2) = (self.n1.round() as usize, self.n2.round() as usize);
        let [p1, p2] = self.p;
        let (b1, b2) = (binomial_pmf(n1, p1), binomial_pmf(n2, p2));
        let (f1, f2) = (n1 as f64, n2 as f64);
        let n = f1 + f2;
        let level = if self.two_sided {
            self.alpha / 2.0
        } else {
            self.alpha
        };
        // the direction of a one-sided test
        let up = p1 >= p2;
        if self.method == ProportionTest::Fisher {
            let mut power = 0.0;
            for m in 0..=n1 + n2 {
                let low = m.saturating_sub(n2);
                let high = m.min(n1);
                let total = ln_choose(n, m as f64);
                let null: Vec<f64> = (low..=high)
                    .map(|i| {
                        (ln_choose(f1, i as f64) + ln_choose(f2, (m - i) as f64) - total).exp()
                    })
                    .collect();
                let (below, above) = critical(&null, level);
                for (k, i) in (low..=high).enumerate() {
                    let k = k as i64;
                    let reject = if self.two_sided {
                        k <= below || k > above
                    } else if up {
                        k > above
                    } else {
                        k <= below
                    };
                    if reject {
                        // the conditional probability times that of m is the joint one
                        power += b1[i] * b2[m - i];
                    }
                }
            }
            return power.min(1.0);
        }
        let z = z_upper(level);
        let t = t_upper(level, n - 2.0);
        let c = if self.continuity {
            0.5 * (1.0 / f1 + 1.0 / f2)
        } else {
            0.0
        };
        let mut power = 0.0;
        for (x1, w1) in b1.iter().enumerate() {
            for (x2, w2) in b2.iter().enumerate() {
                // all successes or all failures would leave nothing to compare
                let (y1, y2) = match x1 + x2 {
                    0 => (0.001, 0.001),
                    s if s == n1 + n2 => (f1 - 0.001, f2 - 0.001),
                    _ => (x1 as f64, x2 as f64),
                };
                let (h1, h2) = (y1 / f1, y2 / f2);
                let d = h1 - h2;
                let signed = if self.two_sided {
                    d.abs()
                } else if up {
                    d
                } else {
                    -d
                };
                let reject = match self.method {
                    ProportionTest::ChiSquare => {
                        let se = if self.pooled {
                            let pbar = (y1 + y2) / n;
                            (pbar * (1.0 - pbar) * (1.0 / f1 + 1.0 / f2)).sqrt()
                        } else {
                            (h1 * (1.0 - h1) / f1 + h2 * (1.0 - h2) / f2).sqrt()
                        };
                        (signed - c) / se > z
                    }
                    ProportionTest::T => {
                        let spread = if self.pooled {
                            f1 * h1 * (1.0 - h1) + f2 * h2 * (1.0 - h2)
                        } else {
                            f1 * h2 * (1.0 - h2) + f2 * h1 * (1.0 - h1)
                        };
                        (f1 * f2).sqrt() * signed / (spread * n / (n - 2.0)).sqrt() > t
                    }
                    _ => {
                        let g2 = 2.0
                            * (xlogx(y1) + xlogx(y2) + xlogx(f1 - y1) + xlogx(f2 - y2) + xlogx(n)
                                - xlogx(y1 + y2)
                                - xlogx(n - y1 - y2)
                                - xlogx(f1)
                                - xlogx(f2));
                        let side = self.two_sided || (up && h1 >= h2) || (!up && h1 < h2);
                        side && g2.max(0.0).sqrt() > z
                    }
                };
                if reject {
                    power += w1 * w2;
                }
            }
        }
        power.min(1.0)
    }

    fn pairs_normal(&self) -> f64 {
        let [a, b] = self.p;
        let (pd, pt) = (b - a, a + b);
        let n = self.n1;
        let sd = (pt - pd * pd).sqrt();
        if self.two_sided {
            let z = z_upper(self.alpha / 2.0);
            1.0 - normal_cdf((pd * n.sqrt() + z * pt.sqrt()) / sd)
                + normal_cdf((pd * n.sqrt() - z * pt.sqrt()) / sd)
        } else {
            let z = z_upper(self.alpha);
            if pd >= 0.0 {
                normal_cdf((pd * n.sqrt() - z * pt.sqrt()) / sd)
            } else {
                1.0 - normal_cdf((pd * n.sqrt() + z * pt.sqrt()) / sd)
            }
        }
    }

    fn pairs_exact(&self) -> f64 {
        let n = self.n1.round() as usize;
        let [a, b] = self.p;
        let pt = a + b;
        // the share of the discordant pairs that are of the first kind
        let p = a / pt;
        let counts = binomial_pmf(n, pt);
        let level = if self.two_sided {
            self.alpha / 2.0
        } else {
            self.alpha
        };
        let mut power = 0.0;
        for (m, w) in counts.iter().enumerate() {
            if *w == 0.0 {
                continue;
            }
            let (below, above) = critical(&binomial_pmf(m, 0.5), level);
            let mf = m as f64;
            let lower = binomial_cdf(below as f64, mf, p);
            let upper = 1.0 - binomial_cdf(above as f64, mf, p);
            power += w * if self.two_sided {
                lower + upper
            } else if b <= a {
                upper
            } else {
                lower
            };
        }
        power.min(1.0)
    }

    /// The same test with the sample size set from the first (or only) group's size.
    fn sized(&self, n1: f64, ratio: f64) -> Model {
        let mut m = self.clone();
        m.n1 = n1;
        m.n2 = n1 * ratio;
        if !m.groups.is_empty() {
            // the groups keep their proportions
            let total: f64 = self.groups.iter().sum();
            m.groups = self.groups.iter().map(|g| g * n1 / total).collect();
        }
        m
    }

    /// The smallest sample size the test allows.
    fn least_size(&self) -> f64 {
        match self.kind {
            PowerKind::ProportionsOneSample => 1.0,
            PowerKind::PartialCorr => self.partialout + 4.0,
            PowerKind::PearsonOneSample => 4.0,
            PowerKind::SpearmanOneSample if self.variance == RankVariance::Cc => 3.0,
            PowerKind::SpearmanOneSample => 4.0,
            PowerKind::UnivariateLinear => self.predictors + if self.intercept { 2.0 } else { 1.0 },
            _ => 2.0,
        }
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Bisection for the size where `f` (power minus the target, rising with the size) crosses 0,
/// from `x1` and `x2`, doubling `x2` until it is passed. None when it never is.
fn bisect(f: &dyn Fn(f64) -> f64, mut x1: f64, mut x2: f64) -> Option<f64> {
    let mut f2 = f(x2);
    if f2 == 0.0 {
        return Some(x2);
    }
    let mut f1 = f(x1);
    if f1 >= 0.0 {
        return Some(x1);
    }
    while f1 * f2 > 0.0 {
        x1 = x2;
        x2 *= 2.0;
        if x2 > MAX_SIZE {
            return None;
        }
        f1 = f2;
        f2 = f(x2);
        if f2 == 0.0 {
            return Some(x2);
        }
    }
    if f2.is_nan() || f1.is_nan() {
        return None;
    }
    let mut root = x1;
    let mut step = x2 - x1;
    loop {
        step *= 0.5;
        let middle = root + step;
        let value = f(middle);
        if value <= 0.0 {
            root = middle;
        }
        if step.abs() < 1e-9 || value == 0.0 {
            return Some(root);
        }
    }
}

/// The bisection in whole sizes, for exact powers: the result is one above the last size
/// below the target, or a size that hits it.
fn bisect_whole(f: &dyn Fn(f64) -> f64, mut x1: f64, mut x2: f64) -> Option<f64> {
    let mut f2 = f(x2);
    if f2 == 0.0 {
        return Some(x2);
    }
    let mut f1 = f(x1);
    if f1 >= 0.0 {
        return Some(x1);
    }
    while f1 * f2 > 0.0 {
        x1 = x2;
        x2 *= 2.0;
        if x2 > MAX_SIZE {
            return None;
        }
        f1 = f2;
        f2 = f(x2);
        if f2 == 0.0 {
            return Some(x2);
        }
    }
    if f2.is_nan() || f1.is_nan() {
        return None;
    }
    let mut root = x1;
    let mut step = x2 - x1;
    loop {
        step *= 0.5;
        let whole = step.floor();
        if whole == 0.0 {
            // the steps left all land on the root itself
            return Some(if f(root) == 0.0 { root } else { root + 1.0 });
        }
        let value = f(root + whole);
        if value <= 0.0 {
            root += whole;
        }
        if value == 0.0 {
            return Some(root);
        }
    }
}

/// Repeats `search` (from 1 up to the last result) until a result comes back, keeps the
/// smallest, then raises it one at a time until `reached` holds.
fn settle(
    first: f64,
    search: &dyn Fn(f64) -> Option<f64>,
    reached: &dyn Fn(f64) -> bool,
) -> Option<f64> {
    let mut pool = vec![first.ceil()];
    let mut last = first.ceil();
    for _ in 0..1000 {
        let next = search(last)?.ceil();
        if pool.contains(&next) {
            break;
        }
        pool.push(next);
        last = next;
    }
    let mut n = pool.iter().copied().fold(f64::INFINITY, f64::min);
    while !reached(n) {
        n += 1.0;
        if n > MAX_SIZE {
            return None;
        }
    }
    Some(n)
}

/// The sizes found for one power: n1 (and n2), or None when no size reaches it.
type Sizes = Option<(f64, f64)>;

struct Search<'a> {
    model: &'a Model,
    ratio: f64,
    deadline: Option<Instant>,
    timed_out: Flag<bool>,
}

impl Search<'_> {
    fn power(&self, n1: f64) -> f64 {
        self.model.sized(n1, self.ratio).power()
    }

    /// Power at n1 minus `target`, or 1 once the time is up so every search ends at once.
    fn gap(&self, n1: f64, target: f64) -> f64 {
        if let Some(deadline) = self.deadline
            && Instant::now() > deadline
        {
            self.timed_out.set(true);
            return 1.0;
        }
        self.power(n1) - target
    }

    fn find(&self, target: f64) -> Sizes {
        let m = self.model;
        let r = self.ratio;
        let z_beta = z_upper(1.0 - target);
        let level = if m.two_sided { m.alpha / 2.0 } else { m.alpha };
        let z = z_upper(level);
        let continuous = |x1: f64| bisect(&|n| self.gap(n, target), x1, 400.0);
        match m.kind {
            PowerKind::MeansOneSample | PowerKind::MeansRelated => {
                continuous(2.0).map(|n| (n.ceil(), 0.0))
            }
            PowerKind::MeansIndependent => {
                continuous(2.0).map(|n| (n.ceil(), (n * r).ceil().max(2.0)))
            }
            PowerKind::ProportionsOneSample if m.binomial => {
                let f = |n: f64| self.gap(n, target);
                let first = bisect_whole(&f, 1.0, 400.0)?;
                let n = settle(first, &|top| bisect_whole(&f, 1.0, top), &|n| {
                    self.timed_out.get() || self.power(n) >= target
                })?;
                Some((n, 0.0))
            }
            PowerKind::ProportionsOneSample => {
                if m.two_sided {
                    return continuous(1.0).map(|n| (n.ceil(), 0.0));
                }
                let [p0, p1] = m.p;
                let (s0, s1) = ((p0 * (1.0 - p0)).sqrt(), (p1 * (1.0 - p1)).sqrt());
                let b = -z_beta * s1 - z * s0;
                let a = p0 - p1;
                let n = if m.continuity {
                    ((b - (b * b + 2.0 * a.abs()).sqrt()) / (2.0 * a)).powi(2)
                } else {
                    (b / a).powi(2)
                };
                Some((n.ceil().max(1.0), 0.0))
            }
            PowerKind::ProportionsIndependent if m.binomial => {
                // the exact power at a fractional size uses n1 rounded up and n2 rounded
                let whole = |n: f64| {
                    let n1 = n.ceil().max(1.0);
                    (n1, (n1 * r).round().max(1.0))
                };
                let f = |n: f64| {
                    if let Some(deadline) = self.deadline
                        && Instant::now() > deadline
                    {
                        self.timed_out.set(true);
                        return 1.0;
                    }
                    let (n1, n2) = whole(n);
                    let mut s = m.clone();
                    (s.n1, s.n2) = (n1, n2);
                    s.power() - target
                };
                let first = bisect(&f, 2.0, 400.0)?;
                let n1 = settle(first, &|top| bisect(&f, 1.0, top), &|n| {
                    self.timed_out.get() || f(n) >= 0.0
                })?;
                Some(whole(n1))
            }
            PowerKind::ProportionsIndependent => {
                let [p1, p2] = m.p;
                let d = p1 - p2;
                if m.method == ProportionTest::ChiSquare && !m.two_sided {
                    let a = r * p1 * (1.0 - p1) + p2 * (1.0 - p2);
                    let b = (p1 + r * p2) * (1.0 - p1 + r * (1.0 - p2));
                    let n1 = match (m.pooled, m.continuity) {
                        (true, false) => {
                            (z_beta * a.sqrt() + z * b.sqrt() / (1.0 + r).sqrt()).powi(2)
                                / (r * d * d)
                        }
                        (false, false) => (z_beta + z).powi(2) * a / (r * d * d),
                        (pooled, true) => {
                            let c = if pooled {
                                z * b.sqrt() / (r * (r + 1.0)).sqrt() + a.sqrt() / r.sqrt() * z_beta
                            } else {
                                a.sqrt() / r.sqrt() * (z_beta + z)
                            };
                            let root = (c + (c * c + 4.0 * d.abs() * (1.0 + r) / (2.0 * r)).sqrt())
                                / (2.0 * d);
                            root * root
                        }
                    };
                    return Some((n1.ceil().max(1.0), (n1 * r).ceil().max(1.0)));
                }
                let start = if m.method == ProportionTest::Fisher {
                    if p1 > p2 {
                        (0.5 / p1).max(0.5 / (r * (1.0 - p2)))
                    } else {
                        (0.5 / (1.0 - p1)).max(0.5 / (r * p2))
                    }
                } else {
                    2.0
                };
                let n = bisect(&|n| self.gap(n, target), start, start + 400.0)?;
                Some((n.ceil(), (n * r).ceil().max(1.0)))
            }
            PowerKind::ProportionsRelated if m.binomial => {
                let f = |n: f64| self.gap(n, target);
                let first = bisect_whole(&f, 1.0, 400.0)?;
                let n = settle(first, &|top| bisect_whole(&f, 1.0, top), &|n| {
                    self.timed_out.get() || self.power(n) >= target
                })?;
                Some((n, 0.0))
            }
            PowerKind::ProportionsRelated => {
                if m.two_sided {
                    return continuous(1.0).map(|n| (n.ceil(), 0.0));
                }
                let [a, b] = m.p;
                let (pd, pt) = (b - a, a + b);
                let n = ((z_beta * (pt - pd * pd).sqrt() + z * pt.sqrt()) / pd).powi(2);
                Some((n.ceil().max(1.0), 0.0))
            }
            PowerKind::PartialCorr
            | PowerKind::PearsonOneSample
            | PowerKind::SpearmanOneSample
            | PowerKind::UnivariateLinear => {
                // the starting points of the search as the Algorithms manual gives them
                let (x1, x2) = match m.kind {
                    PowerKind::PartialCorr => (m.partialout + 3.0, m.partialout + 400.0),
                    PowerKind::UnivariateLinear => (m.least_size(), m.predictors + 400.0),
                    _ => (3.0, 400.0),
                };
                bisect(&|n| self.gap(n, target), x1, x2)
                    .map(|n| (n.ceil().max(m.least_size()), 0.0))
            }
            // the groups are searched by anova_sizes
            PowerKind::OnewayAnova => None,
        }
    }
}

/// The group sizes of a one-way design with the groups in the proportions `shares` that reach
/// `target`: the total is searched as a continuous value, then each group is rounded up, to at
/// least 2. None when no size reaches it.
fn anova_sizes(m: &Model, shares: &[f64], target: f64) -> Option<Vec<f64>> {
    let at = |n: f64| {
        let mut s = m.clone();
        s.groups = shares.iter().map(|r| r * n).collect();
        s.power() - target
    };
    let j = shares.len() as f64;
    let root = bisect(&at, 2.0 * j, 2.0 * j + 400.0)?;
    Some(shares.iter().map(|r| (root * r).ceil().max(2.0)).collect())
}

fn fail(message: String) -> Error {
    Error(message)
}

/// The discordant shares of pairs from the marginal success rates and their correlation.
fn discordant(first: f64, second: f64, rho: f64) -> [f64; 2] {
    let a = first * (1.0 - second) - rho * (first * (1.0 - first) * second * (1.0 - second)).sqrt();
    [a, a + second - first]
}

fn feasible_pairs([a, b]: [f64; 2]) -> bool {
    a >= 0.0 && b >= 0.0 && a + b <= 1.0 + 1e-12 && a + b > 0.0
}

fn odds(p: f64) -> f64 {
    p / (1.0 - p)
}

/// The proportion with odds `ratio` times those of `p`.
fn with_odds_ratio(p: f64, ratio: f64) -> f64 {
    let o = ratio * odds(p);
    o / (1.0 + o)
}

/// The three ways to compare two proportions: difference, ratio and odds ratio of `p` over
/// `q`.
fn comparisons(p: f64, q: f64) -> [f64; 3] {
    [p - q, p / q, odds(p) / odds(q)]
}

fn number(v: f64) -> Cell {
    if v.is_finite() {
        Cell::Number(v)
    } else {
        Cell::Empty
    }
}

/// What a chart axis is, how it changes the model, and its default range.
struct AxisSpec {
    title: &'static str,
    whole: bool,
    range: (f64, f64),
}

fn axis_spec(m: &Model, key: &str) -> AxisSpec {
    let spec = |title, whole, range| AxisSpec {
        title,
        whole,
        range,
    };
    let size = |n: f64, least: f64| (least, (2.0 * n).max(least + 10.0).round());
    let open = (0.01, 0.99);
    let ratio_range = |v: f64| {
        let v = if v.is_finite() && v > 0.0 { v } else { 1.0 };
        (v.min(1.0) / 2.0, v.max(1.0) * 2.0)
    };
    let correlation = m.kind.correlation();
    match key {
        "N" => spec("Sample size", true, size(m.n1, m.least_size())),
        "TOTAL_N" => {
            let total: f64 = m.groups.iter().sum();
            let least = anova_total_range(m).0;
            spec(
                "Total sample size",
                true,
                (least, (2.0 * total).max(least + 10.0).round()),
            )
        }
        "POOLED_SD" => spec(
            "Pooled standard deviation",
            false,
            (m.sd1 / 2.0, 2.0 * m.sd1),
        ),
        "PARTIALOUT" => spec(
            "Variables partialled out",
            true,
            (0.0, (m.n1 - 4.0).max(0.0)),
        ),
        "PARTIAL_CORR" => spec("Multiple partial correlation", false, (0.0, 0.99)),
        "NULL" if correlation => spec("Null correlation", false, (-0.99, 0.99)),
        "ALTERNATIVE" if correlation => spec("Alternative correlation", false, (-0.99, 0.99)),
        "DIFF" if correlation => spec(
            "Alternative minus null",
            false,
            (-0.99 - m.p[0], 0.99 - m.p[0]),
        ),
        "NPAIRS" => spec("Number of pairs", true, size(m.n1, 2.0)),
        "N1" => spec("Group 1 size", true, size(m.n1, 1.0)),
        "N2" => spec("Group 2 size", true, size(m.n2, 1.0)),
        "ES" => {
            let unequal = m.kind == PowerKind::MeansIndependent && !m.equal;
            let current = if unequal {
                m.diff.abs()
            } else {
                (m.diff / m.sd1).abs()
            };
            let high = if current > 0.0 { 2.0 * current } else { 1.0 };
            let high = if unequal { high } else { high.min(5.0) };
            let low = if m.two_sided { 0.0 } else { -high };
            let title = if unequal {
                "Mean difference"
            } else {
                "Effect size"
            };
            spec(title, false, (low, high))
        }
        "NRATIO" => spec("Group size ratio (n2 / n1)", false, (0.2, 5.0)),
        "NULL" => spec("Null proportion", false, open),
        "ALTERNATIVE" => spec("Alternative proportion", false, open),
        "DIFF" => spec(
            "Alternative minus null",
            false,
            (0.01 - m.p[0], 0.99 - m.p[0]),
        ),
        "P1" => spec("Group 1 proportion", false, open),
        "P2" => spec("Group 2 proportion", false, open),
        "RISK_DIFF" => {
            let base = m.p[if m.kind == PowerKind::ProportionsRelated {
                0
            } else {
                1
            }];
            spec("Risk difference", false, (0.01 - base, 0.99 - base))
        }
        "RISK_RATIO" => {
            let [_, r, _] = comparisons_of(m);
            spec("Risk ratio", false, ratio_range(r))
        }
        "ODDS_RATIO" => {
            let [_, _, o] = comparisons_of(m);
            spec("Odds ratio", false, ratio_range(o))
        }
        "CORRELATION" => spec("Correlation of the pairs", false, (-0.99, 0.99)),
        _ => spec("", false, open),
    }
}

/// Risk difference, ratio and odds ratio as the table shows them.
fn comparisons_of(m: &Model) -> [f64; 3] {
    match m.kind {
        PowerKind::ProportionsRelated => comparisons(m.p[1], m.p[0]),
        _ => comparisons(m.p[0], m.p[1]),
    }
}

/// The model with the quantity `key` set to `v`; None when that is not a possible test.
fn set(m: &Model, key: &str, v: f64) -> Option<Model> {
    let mut s = m.clone();
    let related = m.kind == PowerKind::ProportionsRelated;
    // the proportion that moves, and the one it is measured from
    let (moving, base) = if related { (1, 0) } else { (0, 1) };
    match key {
        "N" | "NPAIRS" | "N1" => s.n1 = v,
        "TOTAL_N" => s = m.sized(v, 0.0),
        "POOLED_SD" => s.sd1 = v,
        "PARTIALOUT" => s.partialout = v,
        "PARTIAL_CORR" => s.partial = v,
        "N2" => s.n2 = v,
        "ES" => {
            s.diff = if m.kind == PowerKind::MeansIndependent && !m.equal {
                v
            } else {
                v * m.sd1
            }
        }
        "NRATIO" => {
            let total = m.n1 + m.n2;
            s.n1 = total / (1.0 + v);
            s.n2 = total - s.n1;
            if m.kind == PowerKind::ProportionsIndependent {
                s.n1 = s.n1.round();
                s.n2 = total - s.n1;
                if s.n1 < 1.0 || s.n2 < 1.0 {
                    return None;
                }
            } else if s.n1 < 2.0 || s.n2 < 2.0 {
                return None;
            }
        }
        "NULL" => s.p[0] = v,
        "ALTERNATIVE" => s.p[1] = v,
        "DIFF" => s.p[1] = m.p[0] + v,
        "P1" => s.p[0] = v,
        "P2" => s.p[1] = v,
        "RISK_DIFF" => s.p[moving] = m.p[base] + v,
        "RISK_RATIO" => s.p[moving] = m.p[base] * v,
        "ODDS_RATIO" => s.p[moving] = with_odds_ratio(m.p[base], v),
        "CORRELATION" => {
            let [first, second] = m.marginal?;
            s.p = discordant(first, second, v);
        }
        _ => return None,
    }
    let ok = match m.kind {
        PowerKind::ProportionsOneSample | PowerKind::ProportionsIndependent => {
            s.p.iter().all(|p| *p > 0.0 && *p < 1.0)
        }
        PowerKind::ProportionsRelated => feasible_pairs(s.p),
        PowerKind::OnewayAnova => s.groups.iter().all(|g| *g >= 2.0) && s.sd1 > 0.0,
        PowerKind::PartialCorr | PowerKind::PearsonOneSample | PowerKind::SpearmanOneSample => {
            s.p.iter().all(|r| r.abs() <= 1.0) && s.n1 >= s.least_size()
        }
        PowerKind::UnivariateLinear => s.partial.abs() < 1.0 && s.n1 >= s.least_size(),
        _ => true,
    };
    ok.then_some(s)
}

/// Evenly spaced points from `low` to `high`, whole numbers when `whole`, about `count`.
fn grid(low: f64, high: f64, whole: bool, count: f64) -> Vec<f64> {
    if whole {
        let step = ((high - low) / count).ceil().max(1.0);
        let mut points = Vec::new();
        let mut v = low;
        while v <= high {
            points.push(v);
            v += step;
        }
        if points.last() != Some(&high) {
            points.push(high);
        }
        points
    } else {
        (0..=count as usize)
            .map(|i| low + (high - low) * i as f64 / count)
            .collect()
    }
}

/// The checks the manual puts on a chart range.
fn check_range(m: &Model, key: &str, (low, high): (f64, f64)) -> Result<(), Error> {
    let bad = |rule: &str| {
        Err(fail(format!(
            "{}: the {key} range {low} to {high} must {rule}",
            m.kind.name()
        )))
    };
    match key {
        "ES" if m.kind == PowerKind::MeansIndependent && !m.equal => {
            if low < 0.0 || high > 5000.0 {
                return bad("lie from 0 to 5000");
            }
        }
        "ES" => {
            let bottom = if m.two_sided { 0.0 } else { -5.0 };
            if low < bottom || high > 5.0 {
                return bad(&format!("lie from {bottom} to 5"));
            }
        }
        "NRATIO" if m.kind == PowerKind::MeansIndependent && (low < 0.2 || high > 5.0) => {
            return bad("lie from 0.2 to 5");
        }
        "NRATIO" | "RISK_RATIO" | "ODDS_RATIO" if low <= 0.0 => return bad("be above 0"),
        "NPAIRS" => {
            let top = if m.kind == PowerKind::MeansRelated {
                5000.0
            } else {
                2500.0
            };
            if low < 2.0 || high > top {
                return bad(&format!("lie from 2 to {top}"));
            }
        }
        "N" if low < axis_spec(m, "N").range.0 => {
            return bad(&format!("start at {} or more", axis_spec(m, "N").range.0));
        }
        "N1" | "N2" if low < 1.0 => return bad("start at 1 or more"),
        "TOTAL_N" => {
            let (least, most) = anova_total_range(m);
            if low < least || high > most {
                return bad(&format!("lie from {least} to {most}"));
            }
        }
        "POOLED_SD" if low <= 0.0 => return bad("be above 0"),
        _ => {}
    }
    Ok(())
}

/// The totals a TOTAL_N chart may span: every group at least 2 and at most 5000.
fn anova_total_range(m: &Model) -> (f64, f64) {
    let total: f64 = m.groups.iter().sum();
    let smallest = m.groups.iter().copied().fold(f64::INFINITY, f64::min);
    let largest = m.groups.iter().copied().fold(0.0, f64::max);
    (
        (2.0 * total / smallest).ceil(),
        (5000.0 / largest * total).floor().min(MAX_SIZE),
    )
}

fn line_chart(m: &Model, request: &PowerPlot, range: Option<(f64, f64)>) -> Chart {
    let key = request.x.as_str();
    let spec = axis_spec(m, key);
    let (low, high) = range.unwrap_or(spec.range);
    let points = grid(low, high, spec.whole, 100.0)
        .into_iter()
        .filter_map(|v| {
            let power = set(m, key, v)?.power();
            power.is_finite().then_some(Point {
                x: v,
                y: power,
                z: None,
                label: None,
            })
        })
        .collect();
    let mut chart = Chart::plot(
        ChartKind::Line,
        format!("Power by {}", spec.title.to_lowercase()),
        Axis::titled(spec.title),
        Axis::titled("Power"),
        Plot::Points {
            groups: vec![PointGroup {
                name: "Power".into(),
                points,
            }],
            lines: Vec::new(),
            z_axis: None,
        },
    );
    chart.footnote = Some(format!(
        "{}, other assumptions as in the table",
        m.kind.name()
    ));
    chart
}

fn surface_chart(m: &Model, request: &PowerPlot) -> Chart {
    // DISCORDANT and MARGINAL name a surface over the two shares of pairs
    let (x, y, titles) = match (request.x.as_str(), &request.y) {
        ("DISCORDANT", _) => (
            "A",
            "B",
            [
                "Success first, failure second",
                "Failure first, success second",
            ],
        ),
        ("MARGINAL", _) => (
            "FIRST",
            "SECOND",
            ["First success rate", "Second success rate"],
        ),
        (x, Some(y)) => (
            x,
            y.as_str(),
            [axis_spec(m, x).title, axis_spec(m, y).title],
        ),
        (x, None) => (x, x, [""; 2]),
    };
    let range = |key: &str, given: Option<(f64, f64)>| {
        let spec = axis_spec(m, key);
        let (low, high) = given.unwrap_or(spec.range);
        grid(low, high, spec.whole, 20.0)
    };
    let (xs, ys) = (range(x, request.x_range), range(y, request.y_range));
    let mut points = Vec::new();
    for &u in &xs {
        for &v in &ys {
            let model = match (x, y) {
                ("A", _) => {
                    let mut s = m.clone();
                    s.p = [u, v];
                    feasible_pairs(s.p).then_some(s)
                }
                ("FIRST", _) => {
                    let mut s = m.clone();
                    s.p = discordant(u, v, m.correlation);
                    feasible_pairs(s.p).then_some(s)
                }
                _ => set(m, x, u).and_then(|s| set(&s, y, v)),
            };
            if let Some(power) = model.map(|s| s.power()).filter(|p| p.is_finite()) {
                points.push(Point {
                    x: u,
                    y: v,
                    z: Some(power),
                    label: None,
                });
            }
        }
    }
    let mut chart = Chart::plot(
        ChartKind::Scatter,
        format!(
            "Power by {} and {}",
            titles[0].to_lowercase(),
            titles[1].to_lowercase()
        ),
        Axis::titled(titles[0]),
        Axis::titled(titles[1]),
        Plot::Points {
            groups: vec![PointGroup {
                name: "Power".into(),
                points,
            }],
            lines: Vec::new(),
            z_axis: Some(Axis::titled("Power")),
        },
    );
    chart.footnote = Some(format!(
        "{}, other assumptions as in the table",
        m.kind.name()
    ));
    chart
}

impl Engine {
    pub(crate) fn power(&mut self, c: &Power) -> Result<Vec<Table>, Error> {
        let name = c.kind.name();
        let finding = !c.power.is_empty();
        for v in &c.repeated {
            self.warn(format!(
                "{name}: the power {v} is given more than once and is used once"
            ));
        }
        let mut model = Model {
            kind: c.kind,
            alpha: c.significance,
            two_sided: !c.directional,
            n1: c.n.first().copied().unwrap_or(f64::NAN),
            n2: c.n.get(1).copied().unwrap_or(f64::NAN),
            diff: 0.0,
            sd1: c.sd.first().copied().unwrap_or(f64::NAN),
            sd2: c.sd.last().copied().unwrap_or(f64::NAN),
            equal: true,
            p: [0.0; 2],
            marginal: None,
            correlation: c.correlation,
            method: c.method,
            pooled: c.pooled,
            continuity: c.continuity,
            binomial: c.binomial,
            groups: Vec::new(),
            means: Vec::new(),
            contrast: Vec::new(),
            partialout: c.partialout,
            bias: c.adjust_bias || c.kind == PowerKind::PartialCorr,
            variance: c.variance,
            predictors: c.total_predictors,
            tested: c.test_predictors,
            intercept: c.intercept,
            random: c.random,
            partial: 0.0,
            squares: None,
        };
        let mean_difference = match c.mean.as_slice() {
            [d] => *d,
            [m1, m2] => m1 - m2,
            _ => 0.0,
        };
        match c.kind {
            PowerKind::MeansOneSample => model.diff = c.mean[0] - c.null.unwrap_or(0.0),
            PowerKind::MeansIndependent => {
                model.diff = mean_difference;
                model.equal = model.sd1 == model.sd2;
            }
            PowerKind::MeansRelated => {
                model.diff = mean_difference;
                if let [s1, s2] = c.sd.as_slice() {
                    model.sd1 = (s1 * s1 + s2 * s2 - 2.0 * c.rho * s1 * s2).sqrt();
                }
            }
            PowerKind::ProportionsOneSample => {
                model.p = [c.null.unwrap_or(0.5), c.alternative.unwrap_or(f64::NAN)];
            }
            PowerKind::ProportionsIndependent => model.p = [c.proportions[0], c.proportions[1]],
            PowerKind::ProportionsRelated => {
                let mut values = [c.proportions[0], c.proportions[1]];
                if c.counts {
                    values = values.map(|v| v / model.n1);
                }
                if c.marginal {
                    if values.iter().any(|v| *v <= 0.0 || *v >= 1.0) {
                        return Err(fail(format!(
                            "{name}: marginal success rates must lie strictly between 0 and 1"
                        )));
                    }
                    model.marginal = Some(values);
                    model.p = discordant(values[0], values[1], c.correlation);
                    if !feasible_pairs(model.p) {
                        return Err(fail(format!(
                            "{name}: success rates {} and {} cannot have the correlation {}: \
                             a share of discordant pairs would fall outside 0 to 1",
                            values[0], values[1], c.correlation
                        )));
                    }
                } else {
                    model.p = values;
                    if !feasible_pairs(values) {
                        return Err(fail(format!(
                            "{name}: the two discordant shares must be at least 0, add up to at \
                             most 1 and not both be 0"
                        )));
                    }
                }
            }
            PowerKind::OnewayAnova => {
                model.means = c.mean.clone();
                model.groups = c.n.clone();
                if finding {
                    // the design proportions: GROUP_WEIGHTS, or equal groups
                    model.groups = if c.weights.is_empty() {
                        vec![1.0; c.mean.len()]
                    } else {
                        c.weights.clone()
                    };
                }
                model.n1 = model.groups.iter().sum();
                model.sd1 = c.sd[0];
            }
            PowerKind::PartialCorr | PowerKind::PearsonOneSample | PowerKind::SpearmanOneSample => {
                model.p = [c.null.unwrap_or(0.0), c.alternative.unwrap_or(f64::NAN)];
            }
            PowerKind::UnivariateLinear => {
                model.squares = c.full_nested.map(|(full, nested)| [full, nested]);
                model.partial = match model.squares {
                    Some([full, nested]) => ((full - nested) / (1.0 - nested)).sqrt(),
                    None => c.partial_corr.unwrap_or(f64::NAN),
                };
            }
        }
        let no_effect = match c.kind {
            PowerKind::MeansOneSample | PowerKind::MeansIndependent | PowerKind::MeansRelated => {
                model.diff == 0.0
            }
            PowerKind::OnewayAnova => model.means.iter().all(|m| *m == model.means[0]),
            PowerKind::UnivariateLinear => model.partial == 0.0,
            _ => model.p[0] == model.p[1],
        };
        if finding && no_effect {
            return Err(fail(format!(
                "{name}: with no difference to detect no sample size raises the power above the \
                 significance level"
            )));
        }
        if c.kind == PowerKind::OnewayAnova {
            return self.power_anova(c, model);
        }
        let mut warnings = Vec::new();
        let exact_note = |n: f64, warnings: &mut Vec<String>| {
            if c.binomial && n > EXACT_LIMIT {
                warnings.push(format!(
                    "{name}: ESTIMATE=BINOMIAL works up to {EXACT_LIMIT} trials; with {n} the \
                     normal approximation is used"
                ));
            }
        };
        // one row per power: the sizes, the power the test has, the power asked for
        let mut rows: Vec<(Sizes, f64, Option<f64>)> = Vec::new();
        if finding {
            let mut search_model = model.clone();
            if c.kind == PowerKind::ProportionsIndependent
                && c.binomial
                && c.method == ProportionTest::Fisher
            {
                warnings.push(format!(
                    "{name}: the sample size for Fisher's exact test is searched with the exact \
                     chi-square test (pooled, no continuity correction); the power shown is that \
                     of Fisher's test"
                ));
                search_model.method = ProportionTest::ChiSquare;
                search_model.pooled = true;
                search_model.continuity = false;
            }
            let search = Search {
                model: &search_model,
                ratio: c.ratio,
                deadline: c
                    .timer
                    .filter(|_| c.binomial)
                    .map(|minutes| Instant::now() + Duration::from_secs_f64(minutes * 60.0)),
                timed_out: Flag::new(false),
            };
            for &target in &c.power {
                let sizes = search.find(target);
                if search.timed_out.get() {
                    self.warn(format!(
                        "{name}: the TIMER limit of {} minutes was reached before the sample size \
                         was found; there are no results",
                        c.timer.unwrap_or(0.0)
                    ));
                    return Ok(Vec::new());
                }
                let actual = match sizes {
                    Some((n1, n2)) => {
                        let mut m = model.clone();
                        (m.n1, m.n2) = (n1, n2);
                        exact_note(n1 + if n2.is_nan() { 0.0 } else { n2 }, &mut warnings);
                        m.power()
                    }
                    None => {
                        warnings.push(format!("{name}: no sample size reaches the power {target}"));
                        f64::NAN
                    }
                };
                rows.push((sizes, actual, Some(target)));
            }
        } else {
            let total = model.n1 + if model.n2.is_nan() { 0.0 } else { model.n2 };
            exact_note(total, &mut warnings);
            let power = model.power();
            if power.is_nan() && c.method == ProportionTest::Fisher {
                warnings.push(format!(
                    "{name}: the arcsine approximation of Fisher's test needs n1 and n2 large \
                     enough for at least half a success and half a failure in the expected \
                     direction; the power is missing"
                ));
            }
            rows.push((Some((model.n1, model.n2)), power, None));
        }
        warnings.dedup();
        for w in warnings {
            self.warn(w);
        }
        let table = power_table(&model, &rows, c);
        // the charts start from the sizes in the table
        let mut base = model.clone();
        if let Some((Some((n1, n2)), _, _)) = rows.first() {
            (base.n1, base.n2) = (*n1, *n2);
        }
        let sizes: Vec<f64> = rows.iter().filter_map(|r| r.0.map(|s| s.0)).collect();
        self.power_charts(c, &base, &sizes)?;
        Ok(vec![table])
    }

    /// The charts PLOT asks for, around `base`; `sizes` are the sizes found for several powers.
    fn power_charts(&mut self, c: &Power, base: &Model, sizes: &[f64]) -> Result<(), Error> {
        let name = c.kind.name();
        let mut charts = Vec::new();
        for request in &c.plots {
            if c.power.len() > 1 {
                let size_chart =
                    request.y.is_none() && matches!(request.x.as_str(), "N" | "NPAIRS" | "TOTAL_N");
                if !size_chart {
                    self.warn(format!(
                        "{name}: with several powers only the sample size chart is drawn; {} is \
                         left out",
                        request.x
                    ));
                    continue;
                }
                if request.x_range.is_some() {
                    self.warn(format!(
                        "{name}: with several powers the {} chart spans the sizes found; the \
                         range given is ignored",
                        request.x
                    ));
                }
                let low = sizes.iter().copied().fold(f64::INFINITY, f64::min);
                let high = sizes.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                if low.is_finite() {
                    charts.push(line_chart(base, request, Some((low, high.max(low + 1.0)))));
                }
                continue;
            }
            if base.n1.is_nan() {
                continue;
            }
            if let Some(range) = request.x_range {
                check_range(base, &request.x, range)?;
            }
            if let (Some(y), Some(range)) = (&request.y, request.y_range) {
                check_range(base, y, range)?;
            }
            let marginal_only = matches!(request.x.as_str(), "CORRELATION" | "MARGINAL");
            if marginal_only && base.marginal.is_none() {
                self.warn(format!(
                    "{name}: the {} chart needs MARGINAL=TRUE and is left out",
                    request.x
                ));
                continue;
            }
            let surface =
                request.y.is_some() || matches!(request.x.as_str(), "DISCORDANT" | "MARGINAL");
            charts.push(if surface {
                surface_chart(base, request)
            } else {
                line_chart(base, request, request.x_range)
            });
        }
        self.charts.extend(charts);
        Ok(())
    }

    /// POWER ONEWAY ANOVA: the overall test, the contrast and the pairwise comparisons, each
    /// with its own sizes when they are searched.
    fn power_anova(&mut self, c: &Power, model: Model) -> Result<Vec<Table>, Error> {
        let name = c.kind.name();
        let finding = !c.power.is_empty();
        let groups = c.mean.len();
        if c.empty_contrast {
            self.warn(format!(
                "{name}: CONTRAST has neither COEFFICIENTS nor PAIRWISE=TRUE and is ignored"
            ));
        }
        let mut overall = model.clone();
        overall.two_sided = true;
        let mut tests = vec![("Overall test".to_string(), overall)];
        if !c.contrast.is_empty() {
            let mut coefficients = c.contrast.clone();
            let sum: f64 = coefficients.iter().sum();
            let scale = coefficients.iter().fold(0.0, |a: f64, v| a.max(v.abs()));
            if sum.abs() > 1e-10 * scale {
                // the last coefficient takes up the difference, or the one before it when only
                // the last is not 0
                let last = groups - 1;
                let fix = if coefficients[..last].iter().all(|v| *v == 0.0) {
                    last - 1
                } else {
                    last
                };
                coefficients[fix] -= sum;
                self.warn(format!(
                    "{name}: the contrast coefficients add up to {sum}, not 0; coefficient {} is \
                     set to {} so that they do",
                    fix + 1,
                    coefficients[fix]
                ));
            }
            let mut m = model.clone();
            m.contrast = coefficients;
            tests.push(("Contrast test".to_string(), m));
        }
        let mut pairs = Vec::new();
        if c.pairwise {
            let comparisons = (groups * (groups - 1)) as f64;
            let alpha = match c.adjust {
                PairwiseAdjust::Bonferroni => 2.0 * c.significance / comparisons,
                PairwiseAdjust::Sidak => 1.0 - (1.0 - c.significance).powf(2.0 / comparisons),
                PairwiseAdjust::Lsd => c.significance,
            };
            for i in 0..groups {
                for j in i + 1..groups {
                    let mut m = model.clone();
                    m.alpha = alpha;
                    m.two_sided = true;
                    m.contrast = vec![0.0; groups];
                    (m.contrast[i], m.contrast[j]) = (1.0, -1.0);
                    pairs.push((format!("Group {} - Group {}", i + 1, j + 1), m));
                }
            }
        }
        let total: f64 = model.groups.iter().sum();
        let shares: Vec<f64> = model.groups.iter().map(|g| g / total).collect();
        let mut warnings = Vec::new();
        // per test and power: the group sizes, the power they give, the power asked for
        let mut solve = |(label, m): &(String, Model)| -> Vec<AnovaRow> {
            let effect = anova_effect(m);
            if !finding {
                return vec![AnovaRow {
                    label: label.clone(),
                    groups: Some(m.groups.clone()),
                    actual: m.power(),
                    asked: None,
                    effect,
                    alpha: m.alpha,
                }];
            }
            c.power
                .iter()
                .map(|&target| {
                    let sizes = anova_sizes(m, &shares, target);
                    let actual = match &sizes {
                        Some(sizes) => {
                            let mut s = m.clone();
                            s.groups = sizes.clone();
                            s.power()
                        }
                        None => {
                            warnings.push(format!(
                                "{name}: no sample size reaches the power {target} ({label})"
                            ));
                            f64::NAN
                        }
                    };
                    AnovaRow {
                        label: label.clone(),
                        groups: sizes,
                        actual,
                        asked: Some(target),
                        effect,
                        alpha: m.alpha,
                    }
                })
                .collect()
        };
        let main: Vec<AnovaRow> = tests.iter().flat_map(&mut solve).collect();
        let pairwise: Vec<AnovaRow> = pairs.iter().flat_map(&mut solve).collect();
        for w in warnings {
            self.warn(w);
        }
        let side = if c.directional {
            "one-sided, in the direction of the contrast"
        } else {
            "two-sided"
        };
        let mut caption = "Overall test: power from the noncentral F distribution.".to_string();
        if !c.contrast.is_empty() {
            caption.push_str(&format!(
                " Contrast test: power from the noncentral t distribution, {side}."
            ));
        }
        caption.push_str(" N is the total over the groups.");
        let mut tables = vec![anova_table(
            "Power Analysis",
            &main,
            model.sd1,
            finding,
            caption,
        )];
        if !pairwise.is_empty() {
            let how = match c.adjust {
                PairwiseAdjust::Bonferroni => "with the Bonferroni adjustment",
                PairwiseAdjust::Sidak => "with the Sidak adjustment",
                PairwiseAdjust::Lsd => "without adjustment (LSD)",
            };
            tables.push(anova_table(
                "Pairwise Differences",
                &pairwise,
                model.sd1,
                finding,
                format!(
                    "Two-sided tests of each pair of group means, {how}; Sig. is the level each \
                     test uses. Power from the noncentral t distribution."
                ),
            ));
        }
        if finding {
            tables.push(group_sizes_table(main.iter().chain(&pairwise), groups));
        }
        // the charts are those of the overall test, from the sizes in the table
        let mut base = tests[0].1.clone();
        if let Some(sizes) = &main[0].groups {
            base.groups = sizes.clone();
        }
        base.n1 = base.groups.iter().sum();
        let totals: Vec<f64> = main
            .iter()
            .filter(|r| r.label == "Overall test")
            .filter_map(|r| r.groups.as_ref().map(|g| g.iter().sum()))
            .collect();
        self.power_charts(c, &base, &totals)?;
        Ok(tables)
    }
}

/// One row of a one-way analysis of variance table.
struct AnovaRow {
    label: String,
    groups: Option<Vec<f64>>,
    actual: f64,
    asked: Option<f64>,
    effect: f64,
    alpha: f64,
}

/// The effect size of a one-way test: the root mean square standardized effect of the overall
/// test, the contrast over the standard deviation (unsigned for a two-sided test).
fn anova_effect(m: &Model) -> f64 {
    if m.contrast.is_empty() {
        let total: f64 = m.groups.iter().sum();
        let mean = dot(&m.groups, &m.means) / total;
        let squares: f64 = m.means.iter().map(|x| (x - mean).powi(2)).sum();
        (squares / (m.means.len() as f64 - 1.0)).sqrt() / m.sd1
    } else {
        let effect = dot(&m.contrast, &m.means) / m.sd1;
        if m.two_sided { effect.abs() } else { effect }
    }
}

fn anova_table(title: &str, rows: &[AnovaRow], sd: f64, finding: bool, caption: String) -> Table {
    let mut columns: Vec<String> = if finding {
        vec!["".into(), "N".into(), "Actual Power".into(), "Power".into()]
    } else {
        vec!["".into(), "Power".into(), "N".into()]
    };
    columns.extend(["Std. Deviation", "Effect Size", "Sig."].map(String::from));
    let mut groups = vec![Vec::new(); 3];
    groups.resize(columns.len(), vec!["Test Assumptions".to_string()]);
    let rows = rows
        .iter()
        .map(|r| {
            let n = r.groups.as_ref().map_or(f64::NAN, |g| g.iter().sum());
            let mut row = vec![Cell::Text(r.label.clone())];
            match r.asked {
                Some(target) => row.extend([number(n), number(r.actual), number(target)]),
                None => row.extend([number(r.actual), number(n)]),
            }
            row.extend([number(sd), number(r.effect), number(r.alpha)]);
            row
        })
        .collect();
    let mut caption = caption;
    if finding {
        caption.push_str(
            " The sizes are the smallest that reach the power asked for, with the groups in the \
             proportions of GROUP_WEIGHTS; Actual Power is the power they give.",
        );
    }
    Table {
        title: title.into(),
        row_headers: 1,
        columns,
        column_groups: groups,
        rows,
        caption: Some(caption),
        ..Table::default()
    }
}

fn group_sizes_table<'a>(rows: impl Iterator<Item = &'a AnovaRow>, groups: usize) -> Table {
    let mut columns = vec!["".to_string(), "Power".to_string()];
    columns.extend((1..=groups).map(|g| format!("Group {g}")));
    columns.push("Total".into());
    let rows = rows
        .map(|r| {
            let mut row = vec![
                Cell::Text(r.label.clone()),
                number(r.asked.unwrap_or(f64::NAN)),
            ];
            match &r.groups {
                Some(sizes) => {
                    row.extend(sizes.iter().map(|g| number(*g)));
                    row.push(number(sizes.iter().sum()));
                }
                None => row.extend(vec![Cell::Empty; groups + 1]),
            }
            row
        })
        .collect();
    Table {
        title: "Group Sizes".into(),
        row_headers: 1,
        columns,
        rows,
        caption: Some("The group sizes behind each N of the tables above.".into()),
        ..Table::default()
    }
}

fn power_table(m: &Model, rows: &[(Sizes, f64, Option<f64>)], c: &Power) -> Table {
    let finding = !c.power.is_empty();
    let two_groups = matches!(
        m.kind,
        PowerKind::MeansIndependent | PowerKind::ProportionsIndependent
    );
    let mut columns = vec![String::new()];
    let mut size_columns: Vec<String> = if two_groups {
        vec!["N1".into(), "N2".into()]
    } else if matches!(
        m.kind,
        PowerKind::MeansRelated | PowerKind::ProportionsRelated
    ) {
        vec!["N Pairs".into()]
    } else {
        vec!["N".into()]
    };
    let shown_diff = if m.two_sided { m.diff.abs() } else { m.diff };
    let (assumption_names, assumptions): (Vec<&str>, Vec<f64>) = match m.kind {
        PowerKind::MeansOneSample | PowerKind::MeansRelated => (
            vec!["Std. Deviation", "Effect Size"],
            vec![m.sd1, shown_diff / m.sd1],
        ),
        PowerKind::MeansIndependent if m.equal => (
            vec!["Std. Deviation", "Effect Size"],
            vec![m.sd1, shown_diff / m.sd1],
        ),
        PowerKind::MeansIndependent => (
            vec!["Std. Deviation 1", "Std. Deviation 2", "Mean Difference"],
            vec![m.sd1, m.sd2, shown_diff],
        ),
        PowerKind::ProportionsOneSample
        | PowerKind::PearsonOneSample
        | PowerKind::SpearmanOneSample => (vec!["Null", "Alternative"], m.p.to_vec()),
        PowerKind::PartialCorr => (
            vec!["Partialled Out", "Null", "Alternative"],
            vec![m.partialout, m.p[0], m.p[1]],
        ),
        PowerKind::UnivariateLinear => match m.squares {
            Some([full, nested]) => (
                vec![
                    "Total Predictors",
                    "Test Predictors",
                    "Full Model R Square",
                    "Nested Model R Square",
                ],
                vec![m.predictors, m.tested, full, nested],
            ),
            None => (
                vec!["Total Predictors", "Test Predictors", "Partial Correlation"],
                vec![m.predictors, m.tested, m.partial],
            ),
        },
        _ => (
            vec!["Risk Difference", "Risk Ratio", "Odds Ratio"],
            comparisons_of(m).to_vec(),
        ),
    };
    let mut groups = vec![Vec::new()];
    if finding {
        columns.append(&mut size_columns);
        columns.push("Actual Power".into());
        groups.resize(columns.len(), Vec::new());
        columns.push("Power".into());
    } else {
        columns.push("Power".into());
        groups.push(Vec::new());
        columns.append(&mut size_columns);
    }
    columns.extend(assumption_names.iter().map(|s| s.to_string()));
    columns.push("Sig.".into());
    groups.resize(columns.len(), vec!["Test Assumptions".into()]);
    let label = match m.kind {
        PowerKind::MeansOneSample => "Test of the mean",
        PowerKind::MeansIndependent | PowerKind::MeansRelated => "Test of the mean difference",
        PowerKind::ProportionsOneSample => "Test of the proportion",
        PowerKind::PearsonOneSample => "Test of the correlation",
        PowerKind::SpearmanOneSample => "Test of the rank correlation",
        PowerKind::PartialCorr => "Test of the partial correlation",
        PowerKind::UnivariateLinear => "F test of the tested predictors",
        _ => "Test of the proportion difference",
    };
    let table_rows = rows
        .iter()
        .map(|(sizes, actual, asked)| {
            let mut row = vec![Cell::Text(label.into())];
            let size_cells: Vec<Cell> = match sizes {
                Some((n1, n2)) if two_groups => vec![number(*n1), number(*n2)],
                Some((n1, _)) => vec![number(*n1)],
                None => vec![Cell::Empty; if two_groups { 2 } else { 1 }],
            };
            match asked {
                Some(target) => {
                    row.extend(size_cells);
                    row.push(number(*actual));
                    row.push(number(*target));
                }
                None => {
                    row.push(number(*actual));
                    row.extend(size_cells);
                }
            }
            row.extend(assumptions.iter().map(|v| number(*v)));
            row.push(number(m.alpha));
            row
        })
        .collect();
    let mut notes = vec![if m.kind == PowerKind::UnivariateLinear {
        // an F test has no direction
        String::new()
    } else if m.two_sided {
        "Two-sided test.".to_string()
    } else {
        "One-sided test.".to_string()
    }];
    let exact = |n: f64| m.binomial && n <= EXACT_LIMIT;
    let total = |(n1, n2): (f64, f64)| n1 + if n2.is_nan() { 0.0 } else { n2 };
    let largest = rows
        .iter()
        .filter_map(|r| r.0.map(total))
        .fold(0.0, f64::max);
    notes.push(match m.kind {
        PowerKind::MeansOneSample | PowerKind::MeansRelated | PowerKind::MeansIndependent => {
            let mut text = "Power from the noncentral t distribution.".to_string();
            if m.kind == PowerKind::MeansIndependent {
                text.push_str(if m.equal {
                    " Equal group variances assumed."
                } else {
                    " Separate group variances, with Welch's degrees of freedom."
                });
            }
            if m.kind == PowerKind::MeansRelated {
                text.push_str(" The standard deviation is that of the differences within pairs.");
            }
            text
        }
        PowerKind::ProportionsOneSample | PowerKind::ProportionsRelated => {
            if exact(largest) && m.kind == PowerKind::ProportionsRelated {
                "Exact conditional binomial test of the discordant pairs.".into()
            } else if exact(largest) {
                "Exact binomial test.".into()
            } else if m.continuity && m.kind == PowerKind::ProportionsOneSample {
                "Normal approximation with continuity correction.".into()
            } else {
                "Normal approximation.".into()
            }
        }
        PowerKind::ProportionsIndependent => {
            let fisher_fallback =
                m.method == ProportionTest::Fisher && m.binomial && !exact(largest);
            let test = match m.method {
                ProportionTest::Fisher if !fisher_fallback => {
                    if exact(largest) {
                        "Fisher's exact test, exact power.".to_string()
                    } else {
                        "Fisher's exact test, arcsine approximation.".to_string()
                    }
                }
                ProportionTest::LikelihoodRatio => "Likelihood ratio test.".into(),
                ProportionTest::T => format!(
                    "t-test, {} variance.",
                    if m.pooled { "pooled" } else { "separate" }
                ),
                _ => format!(
                    "Chi-square test, {} standard error{}.",
                    if m.pooled { "pooled" } else { "separate" },
                    if m.continuity {
                        ", continuity correction"
                    } else {
                        ""
                    }
                ),
            };
            let how = if exact(largest) {
                " Binomial enumeration."
            } else {
                " Large-sample approximation."
            };
            test + how
        }
        PowerKind::PearsonOneSample | PowerKind::PartialCorr => format!(
            "Fisher's z with the normal approximation, {} the bias adjustment.",
            if m.bias { "with" } else { "without" }
        ),
        PowerKind::SpearmanOneSample => format!(
            "Fisher's z with the normal approximation; the variance of z after {}.",
            match m.variance {
                RankVariance::Bw => "Bonett and Wright",
                RankVariance::Fhp => "Fieller, Hartley and Pearson",
                RankVariance::Cc => "Caruso and Cliff",
            }
        ),
        PowerKind::UnivariateLinear => format!(
            "{} The model has {}.",
            if m.random {
                "Random predictors, jointly normal with the outcome: power from the distribution \
                 of the squared multiple partial correlation."
            } else {
                "Fixed predictors: power from the noncentral F distribution."
            },
            if m.intercept {
                "an intercept"
            } else {
                "no intercept"
            }
        ),
        PowerKind::OnewayAnova => String::new(),
    });
    notes.retain(|n| !n.is_empty());
    if finding {
        notes.push(
            "The sizes are the smallest that reach the power asked for; Actual Power is the \
             power they give."
                .into(),
        );
    }
    Table {
        title: "Power Analysis".into(),
        row_headers: 1,
        columns,
        column_groups: groups,
        rows: table_rows,
        caption: Some(notes.join(" ")),
        ..Table::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn critical_of(pmf: &[f64], level: f64) -> (i64, i64) {
        critical(pmf, level)
    }

    #[test]
    fn critical_values_of_a_fair_coin() {
        // ten tosses: P(X <= 1) = 11/1024 <= 0.025 < P(X <= 2) = 56/1024
        let pmf = binomial_pmf(10, 0.5);
        assert_eq!(critical_of(&pmf, 0.025), (1, 8));
        // P(X <= 0) = 1/1024 is above 0.0005, so nothing is rejected on the left
        assert_eq!(critical_of(&pmf, 0.0005), (-1, 10));
    }

    #[test]
    fn bisection_finds_a_square_root() {
        let root = bisect(&|x| x * x - 2000.0, 2.0, 400.0).unwrap();
        assert!((root - 2000f64.sqrt()).abs() < 1e-8);
        assert_eq!(bisect_whole(&|x| x - 37.5, 1.0, 400.0), Some(38.0));
        assert_eq!(bisect(&|_| -1.0, 2.0, 400.0), None);
    }
}
