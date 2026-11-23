//! GENLIN: generalized linear models, fitted by maximum likelihood.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (GENLIN), in our words:
//! - A case is used when its frequency weight rounds to 1 or more, the response suits the
//!   distribution (above 0 for GAMMA and IGAUSS, a whole number of 0 or more for POISSON and
//!   NEGBIN, events a whole number from 0 to the trials, trials a whole number of 1 or more),
//!   every factor (user-missing values valid with CLASSMISSING=INCLUDE), covariate and offset is
//!   valid and the scale weight is above 0. A binary response has exactly two values among them.
//! - The model: g(mu) = x'b + offset, Var(y) = phi V(mu) / w with the scale weight w (times the
//!   trials for events/trials, whose response is the proportion). The design has a column for
//!   the intercept and one per level combination of each effect; a column that depends on the
//!   columns before it (sweeping X'WX, W the frequency times scale weights) is redundant and 0.
//! - A binary response models the first value (in ascending order) unless REFERENCE says
//!   otherwise; the other value is the reference, coded 0.
//! - Estimates: Newton steps on the log likelihood from weighted least squares start values,
//!   with Fisher scoring (the expected information) for the first METHOD iterations and the
//!   observed information after; a step is halved while the log likelihood falls. The scale is
//!   1 for the discrete distributions; MLE estimates it jointly (the default for the
//!   continuous ones), DEVIANCE and PEARSON fit with 1 and then divide the statistic by its
//!   degrees of freedom, a number fixes it. NEGBIN(MLE) estimates k jointly.
//! - Standard errors from the inverse of the information that the last iteration used, times
//!   the scale; ROBUST gives the sandwich form.
//! - Goodness of fit: deviance and Pearson chi-square (over subpopulations, the cases with the
//!   same predictor values, for a binary response), each also divided by the scale, on N - p
//!   degrees of freedom; the log likelihood (full or kernel) and AIC, AICC, BIC and CAIC from
//!   it, counting the scale and k when estimated. With DEVIANCE or PEARSON the log likelihood is
//!   taken at scale 1, and the adjusted one at the estimate.
//! - The omnibus test compares the model with the intercept-only model (or with no parameters
//!   when there is no intercept) by likelihood ratio. Type I tests add the effects in order to
//!   the model with no parameters; Type III tests constrain each effect to its Type III
//!   hypothesis. Wald or likelihood ratio chi-squares; with DEVIANCE or PEARSON the likelihood
//!   ratio is divided by the scale and an F is added.
//! - The ordinal multinomial model: link(P(y <= j)) = threshold_j - x'b + offset, without an
//!   intercept.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, f_p, ln_gamma, normal_cdf, normal_quantile};
use crate::glm::{
    Data, Fit, combinations, effects_of, emmeans_l, gef_basis, ginverse, intercept_term,
    key_value_text, l_prime_table, parameter_name, percent_title,
};
use crate::matrix::{Matrix, symmetric_eigen};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::shown::{Show, Shown};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Measure, Values, Variable};
use statscraft_syntax::{
    Contrast, Genlin, GenlinAdjust, GenlinContrast, GenlinCorrelation, GenlinCriteria,
    GenlinDistribution, GenlinEmmeans, GenlinLink, GenlinMethod, GenlinOffset, GenlinOrder,
    GenlinReference, GenlinResponse, GenlinSaveKind, GenlinScale, GenlinTrials, Term, Value,
};
use std::f64::consts::PI;

mod gee;

// ---------------------------------------------------------------------------------------------
// distributions and links

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Family {
    Normal,
    Binomial,
    Gamma,
    InverseGaussian,
    NegBin,
    Poisson,
}

fn digamma(mut x: f64) -> f64 {
    let mut r = 0.0;
    while x < 6.0 {
        r -= 1.0 / x;
        x += 1.0;
    }
    let f = 1.0 / (x * x);
    r + x.ln()
        - 0.5 / x
        - f * (1.0 / 12.0 - f * (1.0 / 120.0 - f * (1.0 / 252.0 - f * (1.0 / 240.0 - f / 132.0))))
}

fn trigamma(mut x: f64) -> f64 {
    let mut r = 0.0;
    while x < 6.0 {
        r += 1.0 / (x * x);
        x += 1.0;
    }
    let f = 1.0 / (x * x);
    r + 1.0 / x + f / 2.0 + f / x * (1.0 / 6.0 - f * (1.0 / 30.0 - f * (1.0 / 42.0 - f / 30.0)))
}

/// ln Gamma(y + r) - ln Gamma(r). For large r (k near 0) the two terms are huge and nearly
/// equal, so the difference of their Stirling series is taken term by term.
fn ln_gamma_ratio(y: f64, r: f64) -> f64 {
    if r > 1e4 {
        let s = y + r;
        y * s.ln() + (r - 0.5) * (y / r).ln_1p() - y + (1.0 / s - 1.0 / r) / 12.0
            - (1.0 / s.powi(3) - 1.0 / r.powi(3)) / 360.0
    } else {
        ln_gamma(y + r) - ln_gamma(r)
    }
}

/// digamma(y + r) - digamma(r), by the difference of the asymptotic series for large r.
fn digamma_difference(y: f64, r: f64) -> f64 {
    if r > 1e4 {
        let s = y + r;
        (y / r).ln_1p() - (1.0 / s - 1.0 / r) / 2.0 - (1.0 / (s * s) - 1.0 / (r * r)) / 12.0
    } else {
        digamma(y + r) - digamma(r)
    }
}

fn normal_density(z: f64) -> f64 {
    (-0.5 * z * z).exp() / (2.0 * PI).sqrt()
}

/// A power below this is the log.
fn tiny(alpha: f64) -> bool {
    alpha.abs() < 2.2e-16
}

/// g(mu).
pub(crate) fn link_value(link: GenlinLink, mu: f64, k: f64) -> f64 {
    use GenlinLink as L;
    match link {
        L::Identity => mu,
        L::Log => mu.ln(),
        L::Logit | L::CumLogit => (mu / (1.0 - mu)).ln(),
        L::Probit | L::CumProbit => normal_quantile(mu),
        L::Cloglog | L::CumCloglog => (-(-mu).ln_1p()).ln(),
        L::Nloglog | L::CumNloglog => -(-mu.ln()).ln(),
        L::Logc => (-mu).ln_1p(),
        L::NegativeBinomial => (k * mu / (1.0 + k * mu)).ln(),
        L::Power(a) if tiny(a) => mu.ln(),
        L::Power(a) => mu.powf(a),
        L::OddsPower(a) if tiny(a) => (mu / (1.0 - mu)).ln(),
        L::OddsPower(a) => ((mu / (1.0 - mu)).powf(a) - 1.0) / a,
        L::CumCauchit => (PI * (mu - 0.5)).tan(),
    }
}

/// The inverse link.
pub(crate) fn link_inverse(link: GenlinLink, eta: f64, k: f64) -> f64 {
    use GenlinLink as L;
    let logistic = |e: f64| 1.0 / (1.0 + (-e).exp());
    match link {
        L::Identity => eta,
        L::Log => eta.exp(),
        L::Logit | L::CumLogit => logistic(eta),
        L::Probit | L::CumProbit => normal_cdf(eta),
        L::Cloglog | L::CumCloglog => -(-eta.exp()).exp_m1(),
        L::Nloglog | L::CumNloglog => (-(-eta).exp()).exp(),
        L::Logc => -eta.exp_m1(),
        L::NegativeBinomial => {
            if eta >= 0.0 {
                f64::NAN
            } else {
                eta.exp() / (k * -eta.exp_m1())
            }
        }
        L::Power(a) if tiny(a) => eta.exp(),
        L::Power(a) => eta.powf(1.0 / a),
        L::OddsPower(a) if tiny(a) => logistic(eta),
        L::OddsPower(a) => {
            let t = (1.0 + a * eta).powf(1.0 / a);
            t / (1.0 + t)
        }
        L::CumCauchit => 0.5 + eta.atan() / PI,
    }
}

/// dg/dmu.
pub(crate) fn link_d1(link: GenlinLink, mu: f64, k: f64) -> f64 {
    use GenlinLink as L;
    match link {
        L::Identity => 1.0,
        L::Log => 1.0 / mu,
        L::Logit | L::CumLogit => 1.0 / (mu * (1.0 - mu)),
        L::Probit | L::CumProbit => 1.0 / normal_density(normal_quantile(mu)),
        L::Cloglog | L::CumCloglog => -1.0 / ((1.0 - mu) * (-mu).ln_1p()),
        L::Nloglog | L::CumNloglog => -1.0 / (mu * mu.ln()),
        L::Logc => -1.0 / (1.0 - mu),
        L::NegativeBinomial => 1.0 / (mu + k * mu * mu),
        L::Power(a) if tiny(a) => 1.0 / mu,
        L::Power(a) => a * mu.powf(a - 1.0),
        L::OddsPower(a) if tiny(a) => 1.0 / (mu * (1.0 - mu)),
        L::OddsPower(a) => mu.powf(a - 1.0) / (1.0 - mu).powf(a + 1.0),
        L::CumCauchit => PI / (PI * (mu - 0.5)).cos().powi(2),
    }
}

/// d2g/dmu2.
fn link_d2(link: GenlinLink, mu: f64, k: f64) -> f64 {
    use GenlinLink as L;
    let d = link_d1(link, mu, k);
    match link {
        L::Identity => 0.0,
        L::Log => -d * d,
        L::Logit | L::CumLogit => d * d * (2.0 * mu - 1.0),
        L::Probit | L::CumProbit => d * d * normal_quantile(mu),
        L::Cloglog | L::CumCloglog => -d * d * (1.0 + (-mu).ln_1p()),
        L::Nloglog | L::CumNloglog => d * d * (1.0 + mu.ln()),
        L::Logc => -d * d,
        L::NegativeBinomial => -d * d * (1.0 + 2.0 * k * mu),
        L::Power(a) if tiny(a) => -d * d,
        L::Power(a) => d * (a - 1.0) / mu,
        L::OddsPower(a) if tiny(a) => d * d * (2.0 * mu - 1.0),
        L::OddsPower(a) => d * ((a - 1.0) / mu + (a + 1.0) / (1.0 - mu)),
        L::CumCauchit => 2.0 * PI * PI * (PI * (mu - 0.5)).tan() / (PI * (mu - 0.5)).cos().powi(2),
    }
}

pub(crate) fn variance(family: Family, mu: f64, k: f64) -> f64 {
    match family {
        Family::Normal => 1.0,
        Family::Binomial => mu * (1.0 - mu),
        Family::Gamma => mu * mu,
        Family::InverseGaussian => mu * mu * mu,
        Family::NegBin => mu + k * mu * mu,
        Family::Poisson => mu,
    }
}

fn variance_d1(family: Family, mu: f64, k: f64) -> f64 {
    match family {
        Family::Normal => 0.0,
        Family::Binomial => 1.0 - 2.0 * mu,
        Family::Gamma => 2.0 * mu,
        Family::InverseGaussian => 3.0 * mu * mu,
        Family::NegBin => 1.0 + 2.0 * k * mu,
        Family::Poisson => 1.0,
    }
}

pub(crate) fn mu_valid(family: Family, mu: f64) -> bool {
    mu.is_finite()
        && match family {
            Family::Normal => true,
            Family::Binomial => mu > 0.0 && mu < 1.0,
            _ => mu > 0.0,
        }
}

/// x ln(x / y), 0 at x = 0.
fn xlog(x: f64, y: f64) -> f64 {
    if x == 0.0 { 0.0 } else { x * (x / y).ln() }
}

/// The deviance of one case of prior weight 1.
fn unit_deviance(family: Family, y: f64, mu: f64, k: f64) -> f64 {
    match family {
        Family::Normal => (y - mu).powi(2),
        Family::InverseGaussian => (y - mu).powi(2) / (y * mu * mu),
        Family::Gamma => 2.0 * (-(y / mu).ln() + (y - mu) / mu),
        Family::NegBin if k > 0.0 => {
            2.0 * (xlog(y, mu) - (y + 1.0 / k) * ((1.0 + k * y) / (1.0 + k * mu)).ln())
        }
        Family::NegBin | Family::Poisson => 2.0 * (xlog(y, mu) - (y - mu)),
        Family::Binomial => 2.0 * (xlog(y, mu) + xlog(1.0 - y, 1.0 - mu)),
    }
}

/// The log likelihood of one case (without its frequency): prior weight `a` (the scale weight
/// times the trials), scale weight `omega`, trials `m`.
#[allow(clippy::too_many_arguments)]
fn case_loglik(
    family: Family,
    y: f64,
    mu: f64,
    phi: f64,
    a: f64,
    omega: f64,
    m: f64,
    k: f64,
    full: bool,
) -> f64 {
    let full = if full { 1.0 } else { 0.0 };
    match family {
        Family::Normal => {
            -0.5 * (a * (y - mu).powi(2) / phi + (phi / a).ln() + full * (2.0 * PI).ln())
        }
        Family::InverseGaussian => {
            -0.5 * (a * (y - mu).powi(2) / (phi * y * mu * mu)
                + (phi * y.powi(3) / a).ln()
                + full * (2.0 * PI).ln())
        }
        Family::Gamma => {
            let s = a / phi;
            s * (s * y / mu).ln() - s * y / mu - ln_gamma(s) - full * y.ln()
        }
        Family::NegBin if k > 0.0 => {
            let r = 1.0 / k;
            let kernel = if y == 0.0 {
                -(k * mu).ln_1p() / k
            } else {
                y * (k * mu).ln() - (y + r) * (k * mu).ln_1p() + ln_gamma_ratio(y, r)
            };
            a / phi * (kernel - full * ln_gamma(y + 1.0))
        }
        Family::NegBin | Family::Poisson => {
            let kernel = if y == 0.0 { -mu } else { y * mu.ln() - mu };
            a / phi * (kernel - full * ln_gamma(y + 1.0))
        }
        Family::Binomial => {
            let kernel = a / phi
                * (if y > 0.0 { y * mu.ln() } else { 0.0 }
                    + if y < 1.0 {
                        (1.0 - y) * (-mu).ln_1p()
                    } else {
                        0.0
                    });
            let r = (y * m).round();
            let choose = ln_gamma(m + 1.0) - ln_gamma(r + 1.0) - ln_gamma(m - r + 1.0);
            kernel + full * omega / phi * choose
        }
    }
}

// ---------------------------------------------------------------------------------------------
// fitting the models other than the multinomial

/// The cases of a fit. For the binomial, `y` is the proportion and `m` the trials.
struct Cases {
    y: Vec<f64>,
    m: Vec<f64>,
    f: Vec<f64>,
    omega: Vec<f64>,
    offset: Vec<f64>,
}

impl Cases {
    fn n(&self) -> usize {
        self.y.len()
    }

    /// The prior weight: the scale weight times the trials.
    fn a(&self, i: usize) -> f64 {
        self.omega[i] * self.m[i]
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ScaleMode {
    Mle,
    Deviance,
    Pearson,
    Fixed(f64),
}

/// What every fit of one command shares.
struct Spec<'a> {
    family: Family,
    link: GenlinLink,
    /// k of NEGBIN; None estimates it.
    k: Option<f64>,
    scale: ScaleMode,
    criteria: &'a GenlinCriteria,
    full: bool,
}

struct GlmFit {
    beta: Vec<f64>,
    mu: Vec<f64>,
    k: f64,
    /// The scale the log likelihood is taken at: the estimate for MLE, the fixed value, or 1.
    phi: f64,
    /// The log likelihood at `phi` and `k` (without the subpopulation term of a binary
    /// response).
    ll: f64,
    converged: bool,
    hessian_ok: bool,
    halving_failed: bool,
    /// The inverse of the information of b (scale 1), and the variance of the log of an
    /// estimated scale or k.
    cov: Matrix,
    log_variance: Option<f64>,
    /// b at the start and after every accepted iteration of the final fit.
    history: Vec<Vec<f64>>,
    /// Whether the last information is the expected one (Fisher scoring) or the observed one.
    fisher: bool,
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// The largest change of an estimate for PCONVERGE: absolute, or relative to the old value.
fn largest_change(old: &[f64], new: &[f64], relative: bool) -> f64 {
    old.iter()
        .zip(new)
        .map(|(o, n)| {
            let d = (n - o).abs();
            if relative { d / (o.abs() + 1e-6) } else { d }
        })
        .fold(0.0, f64::max)
}

fn mat_vec(m: &Matrix, v: &[f64]) -> Vec<f64> {
    m.iter().map(|row| dot(row, v)).collect()
}

/// The derivatives of the log likelihood in b at scale 1: the score, the expected and the
/// observed information.
fn derivatives(
    c: &Cases,
    x: &[Vec<f64>],
    spec: &Spec,
    k: f64,
    mu: &[f64],
) -> (Vec<f64>, Matrix, Matrix) {
    let q = x.first().map_or(0, Vec::len);
    let mut s = vec![0.0; q];
    let mut expected = vec![vec![0.0; q]; q];
    let mut observed = vec![vec![0.0; q]; q];
    for i in 0..c.n() {
        let a = c.f[i] * c.a(i);
        let m = mu[i];
        let v = variance(spec.family, m, k);
        let g1 = link_d1(spec.link, m, k);
        let r = c.y[i] - m;
        let we = a / (v * g1 * g1);
        let wo = we
            + a * r * (v * link_d2(spec.link, m, k) + variance_d1(spec.family, m, k) * g1)
                / (v * v * g1.powi(3));
        let sc = a * r / (v * g1);
        for j in 0..q {
            let xj = x[i][j];
            if xj == 0.0 {
                continue;
            }
            s[j] += sc * xj;
            for l in 0..q {
                expected[j][l] += we * xj * x[i][l];
                observed[j][l] += wo * xj * x[i][l];
            }
        }
    }
    (s, expected, observed)
}

fn evaluate(
    c: &Cases,
    x: &[Vec<f64>],
    spec: &Spec,
    k: f64,
    phi: f64,
    beta: &[f64],
) -> Option<(Vec<f64>, Vec<f64>, f64)> {
    let mut eta = Vec::with_capacity(c.n());
    let mut mu = Vec::with_capacity(c.n());
    let mut ll = 0.0;
    for (i, xi) in x.iter().enumerate().take(c.n()) {
        let e = dot(xi, beta) + c.offset[i];
        let m = link_inverse(spec.link, e, k);
        if !mu_valid(spec.family, m) {
            return None;
        }
        let l = case_loglik(
            spec.family,
            c.y[i],
            m,
            phi,
            c.a(i),
            c.omega[i],
            c.m[i],
            k,
            spec.full,
        );
        if !l.is_finite() {
            return None;
        }
        ll += c.f[i] * l;
        eta.push(e);
        mu.push(m);
    }
    Some((eta, mu, ll))
}

/// Weighted least squares start values from fitted values `start_mu`.
fn start_values(
    c: &Cases,
    x: &[Vec<f64>],
    spec: &Spec,
    k: f64,
    start_mu: &dyn Fn(usize) -> f64,
) -> Vec<f64> {
    let q = x.first().map_or(0, Vec::len);
    let mut xwx = vec![vec![0.0; q]; q];
    let mut xwz = vec![0.0; q];
    for (i, xi) in x.iter().enumerate().take(c.n()) {
        let m = start_mu(i);
        let mut e = link_value(spec.link, m, k);
        if !e.is_finite() {
            e = 1.0;
        }
        let g1 = link_d1(spec.link, m, k);
        let mut w = c.f[i] * c.a(i) / (variance(spec.family, m, k) * g1 * g1);
        if !w.is_finite() {
            w = 0.0;
        }
        let mut z = e - c.offset[i];
        if spec.family == Family::Binomial {
            z += (c.y[i] - m) * g1;
        }
        if !z.is_finite() {
            w = 0.0;
            z = 0.0;
        }
        for j in 0..q {
            xwz[j] += w * xi[j] * z;
            for l in 0..q {
                xwx[j][l] += w * xi[j] * xi[l];
            }
        }
    }
    let (g, _) = ginverse(&xwx, 1e-12);
    mat_vec(&g, &xwz)
}

struct BetaFit {
    beta: Vec<f64>,
    eta: Vec<f64>,
    mu: Vec<f64>,
    ll: f64,
    converged: bool,
    hessian_ok: bool,
    halving_failed: bool,
    expected: Matrix,
    observed: Matrix,
    /// b at the start and after every accepted iteration (for PRINT HISTORY).
    history: Vec<Vec<f64>>,
}

/// The estimates of b for a fixed scale and k.
fn fit_beta(
    c: &Cases,
    x: &[Vec<f64>],
    spec: &Spec,
    k: f64,
    phi: f64,
    start: Option<&[f64]>,
) -> Option<BetaFit> {
    let q = x.first().map_or(0, Vec::len);
    let crit = spec.criteria;
    let mut beta = match start {
        Some(b) if b.len() == q && evaluate(c, x, spec, k, phi, b).is_some() => b.to_vec(),
        _ => {
            // the manual's start; then fitted values pulled toward the mean
            let total: f64 = (0..c.n()).map(|i| c.f[i] * c.a(i)).sum();
            let mean = (0..c.n()).map(|i| c.f[i] * c.a(i) * c.y[i]).sum::<f64>() / total;
            let binomial = spec.family == Family::Binomial;
            let first = |i: usize| {
                if binomial {
                    (c.y[i] * c.m[i] + 0.5) / (c.m[i] + 1.0)
                } else {
                    c.y[i]
                }
            };
            let tries: [&dyn Fn(usize) -> f64; 3] =
                [&first, &|i: usize| (first(i) + mean) / 2.0, &|_: usize| {
                    mean
                }];
            tries
                .iter()
                .map(|t| start_values(c, x, spec, k, *t))
                .find(|b| evaluate(c, x, spec, k, phi, b).is_some())?
        }
    };
    let (mut eta, mut mu, mut ll) = evaluate(c, x, spec, k, phi, &beta)?;
    let (mut s, mut expected, mut observed) = derivatives(c, x, spec, k, &mu);
    let mut history = vec![beta.clone()];
    let criteria_used = crit.lconverge.value > 0.0 || crit.hconverge.value > 0.0;
    let mut ll_model = if criteria_used {
        criteria_scale(c, spec, k, &mu, ll).1
    } else {
        ll
    };
    let mut converged = q == 0 || crit.maxiterations == 0;
    let mut halving_failed = false;
    let mut fisher = true;
    let mut newton_forced = false;
    let tol = crit.singular;
    for iteration in 0..crit.maxiterations {
        if q == 0 {
            break;
        }
        let use_fisher = !newton_forced
            && match crit.method {
                GenlinMethod::Fisher(n) => iteration < n,
                GenlinMethod::FisherOnly => true,
                GenlinMethod::Newton => false,
            };
        let solve = |h: &Matrix| -> Option<Vec<f64>> {
            let (g, rank) = ginverse(h, tol);
            let d = mat_vec(&g, &s);
            (rank == q && dot(&d, &s) >= 0.0).then_some(d)
        };
        let newton = if use_fisher { None } else { solve(&observed) };
        let (direction, used_fisher) = match newton {
            Some(d) => (d, false),
            None => (
                solve(&expected).unwrap_or_else(|| mat_vec(&ginverse(&expected, tol).0, &s)),
                true,
            ),
        };
        fisher = used_fisher;
        let mut step = 1.0;
        let mut accepted = None;
        for _ in 0..crit.maxstephalving {
            let next: Vec<f64> = beta
                .iter()
                .zip(&direction)
                .map(|(b, d)| b + step * d)
                .collect();
            if let Some((e, m, l)) = evaluate(c, x, spec, k, phi, &next)
                && l >= ll - 1e-10 * (1.0 + ll.abs())
            {
                accepted = Some((next, e, m, l));
                break;
            }
            step /= 2.0;
        }
        let Some((next, e, m, l)) = accepted else {
            halving_failed = true;
            break;
        };
        let change = largest_change(&beta, &next, crit.pconverge.relative);
        // the iterations work at scale 1; the criteria are on the model's own scale
        let (scale_now, l_model) = if criteria_used {
            criteria_scale(c, spec, k, &m, l)
        } else {
            (1.0, l)
        };
        let dl = if crit.lconverge.relative {
            (l_model - ll_model).abs() / (ll_model.abs() + 1e-6)
        } else {
            (l_model - ll_model).abs()
        };
        ll_model = l_model;
        beta = next;
        history.push(beta.clone());
        eta = e;
        mu = m;
        ll = l;
        (s, expected, observed) = derivatives(c, x, spec, k, &mu);
        let hc = if crit.hconverge.value > 0.0 {
            let h = if fisher { &expected } else { &observed };
            let quad = dot(&s, &mat_vec(&ginverse(h, tol).0, &s)) / scale_now;
            if crit.hconverge.relative {
                quad / (ll_model.abs() + 1e-6)
            } else {
                quad
            }
        } else {
            0.0
        };
        let met = |t: statscraft_syntax::GenlinTolerance, v: f64| t.value == 0.0 || v < t.value;
        if met(crit.pconverge, change) && met(crit.lconverge, dl) && met(crit.hconverge, hc) {
            // converged by Fisher scoring in a hybrid method: Newton-Raphson must converge too
            if fisher && !expected_information(crit) && !newton_forced {
                newton_forced = true;
                continue;
            }
            converged = true;
            break;
        }
    }
    // no iterations (MAXITERATIONS=0) leave the start values, which nothing claims converged
    let hessian_ok = q == 0 || crit.hconverge.value > 0.0 || crit.maxiterations == 0 || {
        let h = if fisher { &expected } else { &observed };
        let scale_now = criteria_phi(c, spec, &mu);
        dot(&s, &mat_vec(&ginverse(h, tol).0, &s)) / scale_now < 1e-4
    };
    Some(BetaFit {
        beta,
        eta,
        mu,
        ll,
        converged,
        hessian_ok,
        halving_failed,
        expected,
        observed,
        history,
    })
}

/// The score of the log likelihood in tau = ln k for NEGBIN at fixed b (through eta).
fn negbin_score(c: &Cases, spec: &Spec, eta: &[f64], k: f64) -> f64 {
    let mut s = 0.0;
    for (i, &e) in eta.iter().enumerate().take(c.n()) {
        let mu = link_inverse(spec.link, e, k);
        let y = c.y[i];
        let a = c.f[i] * c.a(i);
        let r = 1.0 / k;
        let mut d = y / k + (k * mu).ln_1p() / (k * k)
            - (y + r) * mu / (1.0 + k * mu)
            - digamma_difference(y, r) / (k * k);
        if spec.link == GenlinLink::NegativeBinomial {
            // mu moves with k at a fixed linear predictor: d mu / d k = -mu / k
            d -= (y - mu) / (mu + k * mu * mu) * mu / k;
        }
        s += a * d * k;
    }
    s
}

/// The deviance over the number of cases: the scale of a normal or inverse Gaussian model at
/// these means, and near it for the gamma.
fn scale_estimate(c: &Cases, family: Family, mu: &[f64]) -> f64 {
    let n: f64 = c.f.iter().sum();
    let d: f64 = (0..c.n())
        .map(|i| c.f[i] * c.a(i) * unit_deviance(family, c.y[i], mu[i], 0.0))
        .sum();
    d / n
}

/// The deviance of the means `mu` against that of the weighted mean of the response: below
/// 1E-20 of it the fit is exact up to rounding.
fn exact_fit(c: &Cases, family: Family, mu: &[f64]) -> bool {
    let total: f64 = (0..c.n()).map(|i| c.f[i] * c.a(i)).sum();
    let mean = (0..c.n()).map(|i| c.f[i] * c.a(i) * c.y[i]).sum::<f64>() / total;
    let deviance = |m: &dyn Fn(usize) -> f64| -> f64 {
        (0..c.n())
            .map(|i| c.f[i] * c.a(i) * unit_deviance(family, c.y[i], m(i), 0.0))
            .sum()
    };
    deviance(&|i| mu[i]) <= 1e-20 * deviance(&|_| mean)
}

/// What the Lagrange multiplier test of a fixed scale or k gives.
enum Lagrange {
    /// T = s^2 / A with one degree of freedom.
    Statistic(f64),
    /// The auxiliary regression of NEGBIN(0): alpha, its standard error and z.
    Dispersion(f64, f64, f64),
}

/// The Lagrange multiplier test of the scale (normal, gamma, inverse Gaussian) or of the
/// ancillary parameter k of the negative binomial, at the fitted b and the fixed value.
/// The statistic is s^2 / A with s the score and A the information of tau = ln(scale) or
/// ln(k) after taking out b; the second derivatives are the observed ones.
fn lagrange_test(
    c: &Cases,
    x: &[Vec<f64>],
    spec: &Spec,
    beta: &[f64],
    mu: &[f64],
    phi: f64,
    k: f64,
) -> Lagrange {
    let n = c.n();
    if spec.family == Family::NegBin && k == 0.0 {
        let z: Vec<f64> = (0..n)
            .map(|i| ((c.y[i] - mu[i]).powi(2) - c.y[i]) / mu[i])
            .collect();
        let sww: f64 = (0..n).map(|i| c.f[i] * mu[i] * mu[i]).sum();
        let total: f64 = c.f.iter().sum();
        let alpha = (0..n).map(|i| c.f[i] * mu[i] * z[i]).sum::<f64>() / sww;
        let see = (0..n)
            .map(|i| c.f[i] * (z[i] - alpha * mu[i]).powi(2))
            .sum::<f64>()
            / (total - 1.0);
        let se = (see / sww).sqrt();
        return Lagrange::Dispersion(alpha, se, alpha / se);
    }
    if spec.family != Family::NegBin {
        // the score and second derivative in tau = ln(phi); the b-tau cross term is the score
        // of b over phi, which is 0 at the estimates
        let (mut s, mut d2) = (0.0, 0.0);
        for (i, &m) in mu.iter().enumerate().take(n) {
            let y = c.y[i];
            if spec.family == Family::Gamma {
                let nu = c.a(i) / phi;
                let g = (nu * y / m).ln() + 1.0 - y / m - digamma(nu);
                s -= c.f[i] * nu * g;
                d2 += c.f[i] * nu * (g + 1.0 - nu * trigamma(nu));
            } else {
                let v = if spec.family == Family::Normal {
                    1.0
                } else {
                    y * m * m
                };
                let q = c.a(i) * (y - m).powi(2) / (phi * v);
                s += c.f[i] / 2.0 * (q - 1.0);
                d2 -= c.f[i] * q / 2.0;
            }
        }
        return Lagrange::Statistic(s * s / -d2);
    }
    // NEGBIN with k fixed: central differences of the exact first derivatives in tau, with b
    // fixed
    // shortcut: the manual lists the second derivatives in closed form; the differences
    // agree to about 1E-8, check the closed form if more digits are ever compared.
    let q = x.first().map_or(0, Vec::len);
    let first = |tau: f64| -> (f64, Vec<f64>) {
        let kk = tau.exp();
        let eta: Vec<f64> = (0..n).map(|i| dot(&x[i], beta) + c.offset[i]).collect();
        let mut sb = vec![0.0; q];
        for i in 0..n {
            let m = link_inverse(spec.link, eta[i], kk);
            let r = c.f[i] * c.a(i) * (c.y[i] - m)
                / (variance(spec.family, m, kk) * link_d1(spec.link, m, kk));
            for (j, v) in x[i].iter().enumerate() {
                sb[j] += r * v;
            }
        }
        (negbin_score(c, spec, &eta, kk), sb)
    };
    let tau = k.ln();
    let h = 1e-4;
    let (up, down) = (first(tau + h), first(tau - h));
    let d2 = (up.0 - down.0) / (2.0 * h);
    let cross: Vec<f64> =
        up.1.iter()
            .zip(&down.1)
            .map(|(a, b)| (a - b) / (2.0 * h))
            .collect();
    let (_, _, observed) = derivatives(c, x, spec, k, mu);
    let g = ginverse(&observed, spec.criteria.singular).0;
    let s = first(tau).0;
    Lagrange::Statistic(s * s / (-d2 - dot(&cross, &mat_vec(&g, &cross))))
}

/// The scale the convergence criteria use: the model's own (the current estimate for MLE, 1 for
/// an exact fit), not the scale 1 the iterations work at.
fn criteria_phi(c: &Cases, spec: &Spec, mu: &[f64]) -> f64 {
    match spec.scale {
        ScaleMode::Mle if !exact_fit(c, spec.family, mu) => scale_estimate(c, spec.family, mu),
        ScaleMode::Fixed(v) => v,
        _ => 1.0,
    }
}

/// The scale of `criteria_phi` and the log likelihood at it (`ll` is at the fixed scale or 1).
fn criteria_scale(c: &Cases, spec: &Spec, k: f64, mu: &[f64], ll: f64) -> (f64, f64) {
    let phi = criteria_phi(c, spec, mu);
    if spec.scale != ScaleMode::Mle || phi == 1.0 {
        return (phi, ll);
    }
    let at_phi = (0..c.n())
        .map(|i| {
            c.f[i]
                * case_loglik(
                    spec.family,
                    c.y[i],
                    mu[i],
                    phi,
                    c.a(i),
                    c.omega[i],
                    c.m[i],
                    k,
                    spec.full,
                )
        })
        .sum();
    (phi, at_phi)
}

/// The score of the log likelihood in tau = ln phi and its second derivative, at fixed mu.
fn scale_score(c: &Cases, family: Family, mu: &[f64], phi: f64) -> (f64, f64) {
    let (mut s, mut h) = (0.0, 0.0);
    for (i, &m) in mu.iter().enumerate().take(c.n()) {
        let f = c.f[i];
        let a = c.a(i);
        let y = c.y[i];
        match family {
            Family::Gamma => {
                let t = a / phi;
                let g = (t * y / m).ln() + 1.0 - y / m - digamma(t);
                s -= f * t * g;
                h += f * (t * g + t - t * t * trigamma(t));
            }
            _ => {
                let d = a * unit_deviance(family, y, m, 0.0) / phi;
                s += 0.5 * f * (d - 1.0);
                h -= 0.5 * f * d;
            }
        }
    }
    (s, h)
}

/// The full fit for the scale mode of `spec`.
fn fit_glm(c: &Cases, x: &[Vec<f64>], spec: &Spec, start: Option<&[f64]>) -> Option<GlmFit> {
    let fixed_phi = match spec.scale {
        ScaleMode::Fixed(v) => v,
        _ => 1.0,
    };
    let q = x.first().map_or(0, Vec::len);
    let fit_at = |k: f64, start: Option<&[f64]>| fit_beta(c, x, spec, k, fixed_phi, start);
    let (bf, k, log_variance_k) = match (spec.family, spec.k) {
        (Family::NegBin, None) => {
            // the profile likelihood in tau = ln k, by Newton steps on its derivative
            let mut tau: f64 = 0.0;
            let mut bf = fit_at(1.0, start)?;
            let h = 1e-4;
            let mut second = -1.0;
            let mut k_converged = spec.criteria.maxiterations == 0;
            for _ in 0..spec.criteria.maxiterations {
                let k = tau.exp();
                let g = negbin_score(c, spec, &bf.eta, k);
                if g.abs() <= 1e-9 * (1.0 + bf.ll.abs()) {
                    k_converged = true;
                    break;
                }
                let up = fit_at((tau + h).exp(), Some(&bf.beta))?;
                let down = fit_at((tau - h).exp(), Some(&bf.beta))?;
                second = (negbin_score(c, spec, &up.eta, (tau + h).exp())
                    - negbin_score(c, spec, &down.eta, (tau - h).exp()))
                    / (2.0 * h);
                let step = if second < 0.0 {
                    -g / second
                } else {
                    g.signum() * 0.5
                };
                let step = step.clamp(-2.0, 2.0);
                let mut next = tau + step;
                let mut accepted = None;
                for _ in 0..30 {
                    if let Some(f) = fit_at(next.exp(), Some(&bf.beta))
                        && f.ll >= bf.ll - 1e-10 * (1.0 + bf.ll.abs())
                    {
                        accepted = Some(f);
                        break;
                    }
                    next = tau + (next - tau) / 2.0;
                }
                let Some(f) = accepted else {
                    bf.halving_failed = true;
                    break;
                };
                let moved = (next - tau).abs();
                tau = next;
                bf = f;
                if moved < 1e-10 {
                    k_converged = true;
                    break;
                }
            }
            bf.converged &= k_converged;
            // the curvature of the profile at the final k, for the variance of ln k when there
            // is no joint information (no parameters)
            if q == 0
                && let (Some(up), Some(down)) = (
                    fit_at((tau + h).exp(), Some(&bf.beta)),
                    fit_at((tau - h).exp(), Some(&bf.beta)),
                )
            {
                second = (negbin_score(c, spec, &up.eta, (tau + h).exp())
                    - negbin_score(c, spec, &down.eta, (tau - h).exp()))
                    / (2.0 * h);
            }
            // with parameters the joint information gives the variance of ln k
            (
                bf,
                tau.exp(),
                (q == 0 && second < 0.0).then(|| -1.0 / second),
            )
        }
        (Family::NegBin, Some(k)) => (fit_at(k, start)?, k, None),
        _ => (fit_at(0.0, start)?, 0.0, None),
    };
    let mut phi = fixed_phi;
    let mut log_variance = log_variance_k;
    if spec.scale == ScaleMode::Mle {
        phi = scale_estimate(c, spec.family, &bf.mu);
        // a perfect fit has scale 0 and nothing to iterate on
        if spec.family == Family::Gamma && phi > 0.0 {
            let mut tau = phi.ln();
            for _ in 0..100 {
                let (s, h) = scale_score(c, spec.family, &bf.mu, tau.exp());
                if h >= 0.0 {
                    break;
                }
                let step = (-s / h).clamp(-3.0, 3.0);
                tau += step;
                if step.abs() < 1e-12 {
                    break;
                }
            }
            phi = tau.exp();
        }
        let (_, h) = scale_score(c, spec.family, &bf.mu, phi);
        log_variance = (phi > 0.0 && h < 0.0).then(|| -1.0 / h);
    }
    let ll = (0..c.n())
        .map(|i| {
            c.f[i]
                * case_loglik(
                    spec.family,
                    c.y[i],
                    bf.mu[i],
                    phi,
                    c.a(i),
                    c.omega[i],
                    c.m[i],
                    k,
                    spec.full,
                )
        })
        .sum();
    // the observed information unless every iteration is Fisher scoring; the expected one
    // where the observed is not positive definite
    let observed_ok = {
        let (g, rank) = ginverse(&bf.observed, spec.criteria.singular);
        rank == q && (0..q).all(|j| g[j][j] > 0.0)
    };
    let fisher = expected_information(spec.criteria) || !observed_ok;
    let info = if fisher { &bf.expected } else { &bf.observed };
    let mut cov = ginverse(info, spec.criteria.singular).0;
    if spec.family == Family::NegBin && spec.k.is_none() && q > 0 {
        // the joint information of b and tau: the tau column numerically from the scores
        let tau = k.ln();
        let h = 1e-5;
        let score = |t: f64| -> (Vec<f64>, f64) {
            let kk = t.exp();
            let mu: Vec<f64> = bf
                .eta
                .iter()
                .map(|&e| link_inverse(spec.link, e, kk))
                .collect();
            let (s, _, _) = derivatives(c, x, spec, kk, &mu);
            (s, negbin_score(c, spec, &bf.eta, kk))
        };
        let (su, tu) = score(tau + h);
        let (sd, td) = score(tau - h);
        let mut full = vec![vec![0.0; q + 1]; q + 1];
        for j in 0..q {
            full[j][..q].copy_from_slice(&info[j]);
            // the expected cross information is 0 unless mu moves with k, as it does under the
            // negative binomial link
            let cross = if fisher && spec.link != GenlinLink::NegativeBinomial {
                0.0
            } else {
                -(su[j] - sd[j]) / (2.0 * h)
            };
            full[j][q] = cross;
            full[q][j] = cross;
        }
        full[q][q] = -(tu - td) / (2.0 * h);
        let inv = ginverse(&full, spec.criteria.singular).0;
        cov = (0..q).map(|j| inv[j][..q].to_vec()).collect();
        log_variance = Some(inv[q][q]);
    }
    Some(GlmFit {
        beta: bf.beta,
        mu: bf.mu,
        k,
        phi,
        ll,
        converged: bf.converged,
        hessian_ok: bf.hessian_ok,
        halving_failed: bf.halving_failed,
        cov,
        log_variance,
        history: bf.history,
        fisher,
    })
}

/// What every profile likelihood interval of one fit shares.
#[derive(Clone, Copy)]
struct Profile<'a> {
    c: &'a Cases,
    x: &'a [Vec<f64>],
    /// The spec of the refits: the scale of DEVIANCE or PEARSON is fixed at the full fit's.
    spec: &'a Spec<'a>,
    /// The log likelihood of the full fit under that spec.
    ll_max: f64,
    /// The chi-square quantile (one df) of the confidence level.
    z2: f64,
    tolerance: f64,
}

/// The profile likelihood confidence interval of coefficient `j`: the values of b_j where twice
/// the drop of the log likelihood from `ll_max`, with the other coefficients (and an estimated
/// scale or k) refitted, equals `z2`, the chi-square quantile of one df. Each refit moves b_j
/// into the offset. A bound that is not found (the profile never falls far enough, or the model
/// stops being fittable first without the profile having crossed) is None.
fn profile_interval(setup: &Profile, j: usize, b: &[f64], se: f64) -> (Option<f64>, Option<f64>) {
    let Profile {
        c,
        x,
        spec,
        ll_max,
        z2,
        tolerance,
    } = *setup;
    let reduced: Vec<Vec<f64>> = x
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .filter(|(i, _)| *i != j)
                .map(|(_, v)| *v)
                .collect()
        })
        .collect();
    let mut start: Vec<f64> = b
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != j)
        .map(|(_, v)| *v)
        .collect();
    let mut drop = |value: f64| -> Option<f64> {
        let moved = Cases {
            y: c.y.clone(),
            m: c.m.clone(),
            f: c.f.clone(),
            omega: c.omega.clone(),
            offset: (0..c.n()).map(|i| c.offset[i] + value * x[i][j]).collect(),
        };
        let fit = fit_glm(&moved, &reduced, spec, Some(&start))?;
        if !fit.converged {
            return None;
        }
        start = fit.beta.clone();
        Some(2.0 * (ll_max - fit.ll) - z2)
    };
    profile_limits(b[j], se, tolerance, &mut drop)
}

/// The two values of a parameter, one each side of its estimate `center`, where `drop` (twice
/// the fall of the log likelihood less the chi-square quantile; negative inside) reaches 0: the
/// search steps out from the estimate by `step`, doubling it, until `drop` is positive, then
/// bisects. A side that never gets there, or where `drop` has no value, is None.
fn profile_limits(
    center: f64,
    step: f64,
    tolerance: f64,
    drop: &mut dyn FnMut(f64) -> Option<f64>,
) -> (Option<f64>, Option<f64>) {
    let mut bound = |direction: f64| -> Option<f64> {
        let mut inside = 0.0;
        let mut step = if step.is_finite() && step > 0.0 {
            step
        } else {
            1.0
        };
        let mut outside = None;
        for _ in 0..60 {
            let g = drop(center + direction * (inside + step))?;
            if g > 0.0 {
                outside = Some(inside + step);
                break;
            }
            inside += step;
            step *= 2.0;
        }
        let mut hi = outside?;
        let mut lo = inside;
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if (hi - lo).abs() <= tolerance * 1e-4 * (1.0 + (center + direction * mid).abs()) {
                break;
            }
            match drop(center + direction * mid) {
                Some(g) if g <= 0.0 => lo = mid,
                _ => hi = mid,
            }
        }
        Some(center + direction * 0.5 * (lo + hi))
    };
    (bound(-1.0), bound(1.0))
}

/// k of the negative binomial by maximum likelihood of the model without random effects
/// (GENLINMIXED estimates it so and then keeps it fixed); None when the fit fails.
pub(crate) fn negbin_k(
    y: &[f64],
    weight: &[f64],
    offset: &[f64],
    x: &[Vec<f64>],
    link: GenlinLink,
    iterations: usize,
) -> Option<f64> {
    let n = y.len();
    let c = Cases {
        y: y.to_vec(),
        m: vec![1.0; n],
        f: vec![1.0; n],
        omega: weight.to_vec(),
        offset: offset.to_vec(),
    };
    let criteria = GenlinCriteria {
        maxiterations: iterations.min(u32::MAX as usize) as u32,
        ..GenlinCriteria::default()
    };
    let spec = Spec {
        family: Family::NegBin,
        link,
        k: None,
        scale: ScaleMode::Fixed(1.0),
        criteria: &criteria,
        full: true,
    };
    fit_glm(&c, x, &spec, None).map(|f| f.k)
}

/// Whether the standard errors come from the expected information: when every iteration
/// would be Fisher scoring.
fn expected_information(crit: &GenlinCriteria) -> bool {
    match crit.method {
        GenlinMethod::FisherOnly => true,
        GenlinMethod::Fisher(n) => n >= crit.maxiterations,
        GenlinMethod::Newton => false,
    }
}

// ---------------------------------------------------------------------------------------------
// the ordinal multinomial model

/// Cases of the ordinal model: the category (0-based) of each, frequency, scale weight, offset.
struct Ordinal {
    category: Vec<usize>,
    f: Vec<f64>,
    omega: Vec<f64>,
    offset: Vec<f64>,
    j: usize,
}

struct OrdinalFit {
    /// The thresholds, then b.
    b: Vec<f64>,
    ll: f64,
    converged: bool,
    hessian_ok: bool,
    halving_failed: bool,
    /// The inverse of the observed information.
    cov: Matrix,
    /// b at the start and after every accepted iteration.
    history: Vec<Vec<f64>>,
    /// The last score and the information the covariance comes from.
    gradient: Vec<f64>,
    information: Matrix,
}

/// The cumulative probabilities of one case.
fn cumulative(link: GenlinLink, b: &[f64], x: &[f64], offset: f64, k: usize) -> Vec<f64> {
    let xb = dot(x, &b[k..]);
    (0..k)
        .map(|j| link_inverse(link, b[j] - xb + offset, 0.0))
        .collect()
}

/// The log likelihood, its score and the expected information of the ordinal model.
fn ordinal_eval(
    o: &Ordinal,
    x: &[Vec<f64>],
    link: GenlinLink,
    b: &[f64],
    with_information: bool,
) -> Option<(f64, Vec<f64>, Matrix)> {
    let k = o.j - 1;
    let n = b.len();
    let mut ll = 0.0;
    let mut s = vec![0.0; n];
    let mut info =
        vec![vec![0.0; if with_information { n } else { 0 }]; if with_information { n } else { 0 }];
    for (i, xi) in x.iter().enumerate().take(o.category.len()) {
        let w = o.f[i] * o.omega[i];
        let xb = dot(xi, &b[k..]);
        let eta: Vec<f64> = (0..k).map(|j| b[j] - xb + o.offset[i]).collect();
        let gamma: Vec<f64> = eta.iter().map(|&e| link_inverse(link, e, 0.0)).collect();
        let dens: Vec<f64> = gamma.iter().map(|&g| 1.0 / link_d1(link, g, 0.0)).collect();
        // d pi_c for every category
        let mut dpis = Vec::with_capacity(o.j);
        let mut pis = Vec::with_capacity(o.j);
        for c in 0..o.j {
            let upper = if c < k { gamma[c] } else { 1.0 };
            let lower = if c > 0 { gamma[c - 1] } else { 0.0 };
            let mut d = vec![0.0; n];
            let du = if c < k { dens[c] } else { 0.0 };
            let dl = if c > 0 { dens[c - 1] } else { 0.0 };
            if c < k {
                d[c] += du;
            }
            if c > 0 {
                d[c - 1] -= dl;
            }
            for (t, xv) in xi.iter().enumerate() {
                d[k + t] = -(du - dl) * xv;
            }
            pis.push(upper - lower);
            dpis.push(d);
        }
        let c = o.category[i];
        if pis[c].is_nan() || pis[c] <= 0.0 {
            return None;
        }
        ll += w * pis[c].ln();
        for a in 0..n {
            s[a] += w * dpis[c][a] / pis[c];
        }
        for (pi, d) in pis.iter().zip(&dpis) {
            if *pi <= 0.0 || !with_information {
                continue;
            }
            for a in 0..n {
                if d[a] == 0.0 {
                    continue;
                }
                for bb in 0..n {
                    info[a][bb] += w * d[a] * d[bb] / pi;
                }
            }
        }
    }
    Some((ll, s, info))
}

/// The observed information, from central differences of the score.
fn ordinal_observed(o: &Ordinal, x: &[Vec<f64>], link: GenlinLink, b: &[f64]) -> Option<Matrix> {
    let n = b.len();
    let mut h = vec![vec![0.0; n]; n];
    for a in 0..n {
        let step = 1e-5 * (1.0 + b[a].abs());
        let mut up = b.to_vec();
        up[a] += step;
        let mut down = b.to_vec();
        down[a] -= step;
        let (_, su, _) = ordinal_eval(o, x, link, &up, false)?;
        let (_, sd, _) = ordinal_eval(o, x, link, &down, false)?;
        for bb in 0..n {
            h[bb][a] = -(su[bb] - sd[bb]) / (2.0 * step);
        }
    }
    // symmetric: the mean of the two numerical halves
    let upper = h.clone();
    for (a, row) in h.iter_mut().enumerate() {
        for (bb, v) in row.iter_mut().enumerate() {
            *v = (upper[a][bb] + upper[bb][a]) / 2.0;
        }
    }
    Some(h)
}

fn fit_ordinal(
    o: &Ordinal,
    x: &[Vec<f64>],
    link: GenlinLink,
    crit: &GenlinCriteria,
    phi: f64,
) -> Option<OrdinalFit> {
    let k = o.j - 1;
    let q = x.first().map_or(0, Vec::len);
    // thresholds at the cumulative shares, less the mean offset of the cases below; b at 0
    let total: f64 = (0..o.f.len()).map(|i| o.f[i] * o.omega[i]).sum();
    let mut b = vec![0.0; k + q];
    let (mut below, mut offset_sum) = (0.0, 0.0);
    for (j, slot) in b.iter_mut().take(k).enumerate() {
        for i in 0..o.category.len() {
            if o.category[i] == j {
                below += o.f[i] * o.omega[i];
                offset_sum += o.f[i] * o.omega[i] * o.offset[i];
            }
        }
        let share = (below / total).clamp(1e-6, 1.0 - 1e-6);
        let mean_offset = if below > 0.0 { offset_sum / below } else { 0.0 };
        *slot = link_value(link, share, 0.0) - mean_offset;
    }
    for j in 1..k {
        if b[j] <= b[j - 1] {
            b[j] = b[j - 1] + 1e-3;
        }
    }
    let (mut ll, mut s, mut info) = ordinal_eval(o, x, link, &b, true)?;
    let mut history = vec![b.clone()];
    let mut converged = crit.maxiterations == 0;
    let mut halving_failed = false;
    let mut newton_forced = false;
    for iteration in 0..crit.maxiterations {
        let use_fisher = !newton_forced
            && match crit.method {
                GenlinMethod::Fisher(n) => iteration < n,
                GenlinMethod::FisherOnly => true,
                GenlinMethod::Newton => false,
            };
        let solve = |h: &Matrix| -> Option<Vec<f64>> {
            let (g, rank) = ginverse(h, crit.singular);
            let d = mat_vec(&g, &s);
            (rank == b.len() && dot(&d, &s) >= 0.0).then_some(d)
        };
        let observed = if use_fisher {
            None
        } else {
            ordinal_observed(o, x, link, &b)
        };
        let direction = observed
            .as_ref()
            .and_then(solve)
            .or_else(|| solve(&info))
            .unwrap_or_else(|| mat_vec(&ginverse(&info, crit.singular).0, &s));
        let mut step = 1.0;
        let mut accepted = None;
        for _ in 0..crit.maxstephalving {
            let next: Vec<f64> = b
                .iter()
                .zip(&direction)
                .map(|(v, d)| v + step * d)
                .collect();
            if let Some((l, s2, _)) = ordinal_eval(o, x, link, &next, false)
                && l >= ll - 1e-10 * (1.0 + ll.abs())
            {
                accepted = Some((next, l, s2));
                break;
            }
            step /= 2.0;
        }
        let Some((next, l, s2)) = accepted else {
            halving_failed = true;
            break;
        };
        let change = largest_change(&b, &next, crit.pconverge.relative);
        // the iterations work at scale 1; the criteria are on the model's own scale
        let dl = if crit.lconverge.relative {
            (l - ll).abs() / (ll.abs() + 1e-6)
        } else {
            (l - ll).abs() / phi
        };
        b = next;
        history.push(b.clone());
        ll = l;
        s = s2;
        info = ordinal_eval(o, x, link, &b, true)?.2;
        let hc = if crit.hconverge.value > 0.0 {
            let quad = dot(&s, &mat_vec(&ginverse(&info, crit.singular).0, &s)) / phi;
            if crit.hconverge.relative {
                quad / (ll.abs() / phi + 1e-6)
            } else {
                quad
            }
        } else {
            0.0
        };
        let met = |t: statscraft_syntax::GenlinTolerance, v: f64| t.value == 0.0 || v < t.value;
        if met(crit.pconverge, change) && met(crit.lconverge, dl) && met(crit.hconverge, hc) {
            // converged by Fisher scoring in a hybrid method: Newton-Raphson must converge too
            if use_fisher && !expected_information(crit) && !newton_forced {
                newton_forced = true;
                continue;
            }
            converged = true;
            break;
        }
    }
    let hessian_ok = crit.hconverge.value > 0.0
        || b.is_empty()
        || crit.maxiterations == 0
        || dot(&s, &mat_vec(&ginverse(&info, crit.singular).0, &s)) / phi < 1e-4;
    let positive = |m: &Matrix| {
        let (g, rank) = ginverse(m, crit.singular);
        rank == m.len() && (0..m.len()).all(|j| g[j][j] > 0.0)
    };
    let information = match ordinal_observed(o, x, link, &b) {
        Some(h) if !expected_information(crit) && positive(&h) => h,
        _ => info,
    };
    let cov = ginverse(&information, crit.singular).0;
    Some(OrdinalFit {
        b,
        ll,
        converged,
        hessian_ok,
        halving_failed,
        cov,
        history,
        gradient: s,
        information,
    })
}

// ---------------------------------------------------------------------------------------------
// helpers

/// An orthonormal basis of the null space of the rows of `l` (p columns), as columns.
fn null_space(l: &Matrix, p: usize) -> Matrix {
    let mut ltl = vec![vec![0.0; p]; p];
    for row in l {
        for a in 0..p {
            for b in 0..p {
                ltl[a][b] += row[a] * row[b];
            }
        }
    }
    let pairs = symmetric_eigen(&ltl);
    let largest = pairs.iter().map(|(v, _)| v.abs()).fold(0.0, f64::max);
    let basis: Vec<Vec<f64>> = pairs
        .into_iter()
        .filter(|(v, _)| v.abs() <= 1e-9 * largest.max(1e-300))
        .map(|(_, vec)| vec)
        .collect();
    (0..p)
        .map(|r| basis.iter().map(|v| v[r]).collect())
        .collect()
}

/// The rows of `x` times the p x r matrix `basis`.
fn times(x: &[Vec<f64>], basis: &Matrix) -> Vec<Vec<f64>> {
    let r = basis.first().map_or(0, Vec::len);
    x.iter()
        .map(|row| {
            (0..r)
                .map(|j| row.iter().zip(basis).map(|(v, b)| v * b[j]).sum())
                .collect()
        })
        .collect()
}

/// The Wald chi-square of L b = 0 with its df (the rank of L V L').
fn wald(l: &Matrix, b: &[f64], v: &Matrix) -> (f64, usize) {
    if l.is_empty() {
        return (0.0, 0);
    }
    let lb: Vec<f64> = l.iter().map(|row| dot(row, b)).collect();
    let lv: Matrix = l
        .iter()
        .map(|row| {
            (0..b.len())
                .map(|j| (0..b.len()).map(|i| row[i] * v[i][j]).sum())
                .collect()
        })
        .collect();
    let lvl: Matrix = lv
        .iter()
        .map(|row| l.iter().map(|r2| dot(row, r2)).collect())
        .collect();
    let (g, rank) = ginverse(&lvl, 1e-10);
    (dot(&lb, &mat_vec(&g, &lb)), rank)
}

/// The Type I hypothesis rows: the rows of the forward Doolittle reduction of X'WX.
fn type1_rows(xwx: &Matrix, eps: f64) -> Vec<Option<Vec<f64>>> {
    let p = xwx.len();
    let mut a = xwx.clone();
    let mut out = Vec::with_capacity(p);
    for c in 0..p {
        let original = xwx[c][c];
        if !(original > 0.0 && a[c][c] > eps * original) {
            out.push(None);
            continue;
        }
        let pivot = a[c][c];
        let row: Vec<f64> = (0..p)
            .map(|j| if j < c { 0.0 } else { a[c][j] / pivot })
            .collect();
        for ar in a.iter_mut().skip(c + 1) {
            let factor = ar[c];
            if factor == 0.0 {
                continue;
            }
            for (v, rv) in ar[c..].iter_mut().zip(&row[c..]) {
                *v -= factor * rv;
            }
        }
        out.push(Some(row));
    }
    out
}

pub(crate) fn distribution_name(d: GenlinDistribution, k: Option<f64>) -> String {
    match d {
        GenlinDistribution::Normal => "Normal".into(),
        GenlinDistribution::Binomial => "Binomial".into(),
        GenlinDistribution::Gamma => "Gamma".into(),
        GenlinDistribution::InverseGaussian => "Inverse Gaussian".into(),
        GenlinDistribution::NegativeBinomial(_) => match k {
            Some(k) => format!("Negative binomial ({k})"),
            None => "Negative binomial (MLE)".into(),
        },
        GenlinDistribution::Poisson => "Poisson".into(),
        GenlinDistribution::Multinomial => "Ordinal Multinomial".into(),
    }
}

pub(crate) fn link_name(link: GenlinLink) -> String {
    use GenlinLink as L;
    match link {
        L::Identity => "Identity".into(),
        L::Cloglog => "Complementary log-log".into(),
        L::Log => "Log".into(),
        L::Logc => "Log complement".into(),
        L::Logit => "Logit".into(),
        L::NegativeBinomial => "Negative binomial".into(),
        L::Nloglog => "Negative log-log".into(),
        L::OddsPower(a) => format!("Odds power({a})"),
        L::Power(a) => format!("Power({a})"),
        L::Probit => "Probit".into(),
        L::CumCauchit => "Cumulative Cauchit".into(),
        L::CumCloglog => "Cumulative complementary log-log".into(),
        L::CumLogit => "Cumulative logit".into(),
        L::CumNloglog => "Cumulative negative log-log".into(),
        L::CumProbit => "Cumulative probit".into(),
    }
}

/// Adjusted significances of `p` (one family) and the matching critical values for the
/// intervals at level `alpha`.
fn adjust(p: &[f64], alpha: f64, method: GenlinAdjust) -> (Vec<f64>, Vec<f64>) {
    let r = p.len().max(1) as f64;
    let z = |a: f64| normal_quantile(1.0 - a / 2.0);
    match method {
        GenlinAdjust::Lsd => (p.to_vec(), vec![z(alpha); p.len()]),
        GenlinAdjust::Bonferroni => (
            p.iter().map(|v| (v * r).min(1.0)).collect(),
            vec![z(alpha / r); p.len()],
        ),
        GenlinAdjust::Sidak => (
            p.iter()
                .map(|v| (1.0 - (1.0 - v).powf(r)).min(1.0))
                .collect(),
            vec![z(1.0 - (1.0 - alpha).powf(1.0 / r)); p.len()],
        ),
        GenlinAdjust::SeqBonferroni | GenlinAdjust::SeqSidak => {
            let sidak = method == GenlinAdjust::SeqSidak;
            let mut order: Vec<usize> = (0..p.len()).collect();
            order.sort_by(|a, b| p[*a].total_cmp(&p[*b]));
            let mut adjusted = vec![0.0; p.len()];
            let mut critical = vec![0.0; p.len()];
            let mut previous = 0.0_f64;
            let mut previous_z = 0.0;
            for (rank, &i) in order.iter().enumerate() {
                let left = r - rank as f64;
                let own = if sidak {
                    1.0 - (1.0 - p[i]).powf(left)
                } else {
                    left * p[i]
                };
                let own_z = if sidak {
                    z(1.0 - (1.0 - alpha).powf(1.0 / left))
                } else {
                    z(alpha / left)
                };
                if rank == 0 || own >= previous {
                    previous = own;
                    previous_z = own_z;
                }
                adjusted[i] = previous.min(1.0);
                critical[i] = previous_z;
            }
            (adjusted, critical)
        }
    }
}

fn adjust_name(a: GenlinAdjust) -> &'static str {
    match a {
        GenlinAdjust::Lsd => "Least Significant Difference",
        GenlinAdjust::Bonferroni => "Bonferroni",
        GenlinAdjust::SeqBonferroni => "Sequential Bonferroni",
        GenlinAdjust::Sidak => "Sidak",
        GenlinAdjust::SeqSidak => "Sequential Sidak",
    }
}

pub(crate) fn value_matches(variable: &Variable, key: &Key, value: &Value) -> bool {
    match (key, value) {
        (Key::Number(n), Value::Number(v)) => n == v,
        (Key::Text(t), Value::String(s)) => t.trim_end() == s.trim_end(),
        (Key::Number(_), Value::String(s)) => key_value_text(variable, key) == s.trim(),
        _ => false,
    }
}

// ---------------------------------------------------------------------------------------------
// the command

/// One parameter row: its name and where it sits in the estimates (None: redundant).
struct Parameter {
    name: String,
    at: Option<usize>,
}

/// A new variable of SAVE: its name, label, and values (numbers or texts).
enum SavedValues {
    Numbers(Vec<Option<f64>>, &'static str),
    Texts(Vec<String>),
}

struct Saved {
    name: String,
    given: bool,
    label: String,
    values: SavedValues,
}

/// The fitted model in the form the tables need.
struct Result_ {
    /// The estimates (the thresholds first for the multinomial) and their covariance.
    b: Vec<f64>,
    cov: Matrix,
    /// The log likelihood at the scale the tests use: the estimate (MLE), the fixed value, or 1
    /// (DEVIANCE, PEARSON).
    ll: f64,
    /// The scale shown and used for the standard errors.
    phi: f64,
    k: f64,
    log_variance: Option<f64>,
}

impl Engine {
    pub(crate) fn genlin(&mut self, command: &Genlin) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let crit = &command.criteria;
        let distribution = command.distribution;
        let multinomial = distribution == GenlinDistribution::Multinomial;
        let family = match distribution {
            GenlinDistribution::Normal => Family::Normal,
            GenlinDistribution::Binomial => Family::Binomial,
            GenlinDistribution::Gamma => Family::Gamma,
            GenlinDistribution::InverseGaussian => Family::InverseGaussian,
            GenlinDistribution::NegativeBinomial(_) => Family::NegBin,
            GenlinDistribution::Poisson | GenlinDistribution::Multinomial => Family::Poisson,
        };
        let fixed_k = match distribution {
            GenlinDistribution::NegativeBinomial(k) => k,
            _ => Some(0.0),
        };
        let continuous = matches!(
            family,
            Family::Normal | Family::Gamma | Family::InverseGaussian
        ) && !multinomial;
        let scale = match crit.scale {
            Some(GenlinScale::Mle) => ScaleMode::Mle,
            Some(GenlinScale::Deviance) => ScaleMode::Deviance,
            Some(GenlinScale::Pearson) => ScaleMode::Pearson,
            Some(GenlinScale::Fixed(v)) => ScaleMode::Fixed(v),
            None if continuous => ScaleMode::Mle,
            None => ScaleMode::Fixed(1.0),
        };
        let repeated = command.repeated.as_ref();
        if repeated.is_some() && multinomial {
            return Err(Error(
                "GENLIN: REPEATED with DISTRIBUTION=MULTINOMIAL (ordinal generalized estimating equations) is not supported yet".into(),
            ));
        }
        // the variables, each in one role
        let (response_names, events) = match &command.response {
            GenlinResponse::Dependent { name, .. } => (vec![name.clone()], false),
            GenlinResponse::Events { events, trials } => {
                let mut v = vec![events.clone()];
                if let GenlinTrials::Variable(t) = trials {
                    v.push(t.clone());
                }
                (v, true)
            }
        };
        let offset_name = match &command.offset {
            Some(GenlinOffset::Variable(n)) => Some(n.clone()),
            _ => None,
        };
        let mut seen: Vec<String> = Vec::new();
        for n in response_names
            .iter()
            .chain(&command.factors)
            .chain(&command.covariates)
            .chain(&offset_name)
            .chain(&command.scale_weight)
        {
            let v = data.variable(n)?;
            if seen.iter().any(|s| s.eq_ignore_ascii_case(v.name())) {
                return Err(Error(format!(
                    "{} has more than one role in GENLIN (response, factor, covariate, offset or scale weight)",
                    v.name()
                )));
            }
            seen.push(v.name().to_string());
        }
        if let Some(w) = data.weight_variable()
            && let Some(n) = seen.iter().find(|n| n.eq_ignore_ascii_case(w))
        {
            return Err(Error(format!(
                "GENLIN cannot use the WEIGHT variable {n} in the model; it is the frequency weight"
            )));
        }
        let lookup = |names: &[String]| -> Result<Vec<Variable>, Error> {
            names.iter().map(|n| Ok(data.variable(n)?)).collect()
        };
        let subject_vars = lookup(repeated.map_or(&[][..], |r| &r.subject))?;
        let within_vars = lookup(repeated.map_or(&[][..], |r| &r.within))?;
        for v in subject_vars.iter().chain(&within_vars) {
            let other_role = response_names
                .iter()
                .chain(&command.covariates)
                .chain(&offset_name)
                .chain(&command.scale_weight)
                .any(|n| n.eq_ignore_ascii_case(v.name()));
            if other_role {
                return Err(Error(format!(
                    "{} cannot be a SUBJECT or WITHINSUBJECT variable of GENLIN: it is the response, a covariate, the offset or the scale weight",
                    v.name()
                )));
            }
        }
        let dependent = data.variable(&response_names[0])?;
        let trials_var = match &command.response {
            GenlinResponse::Events {
                trials: GenlinTrials::Variable(t),
                ..
            } => Some(data.variable(t)?),
            _ => None,
        };
        let factor_vars: Vec<Variable> = command
            .factors
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let cov_vars: Vec<Variable> = command
            .covariates
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let offset_var = offset_name.as_ref().map(|n| data.variable(n)).transpose()?;
        let weight_var = command
            .scale_weight
            .as_ref()
            .map(|n| data.variable(n))
            .transpose()?;
        let categorical = multinomial || (family == Family::Binomial && !events);
        let must_be_numeric = cov_vars
            .iter()
            .chain(&offset_var)
            .chain(&weight_var)
            .chain(&trials_var)
            .chain((!categorical).then_some(&dependent));
        for v in must_be_numeric {
            if v.is_string() {
                return Err(Error(format!(
                    "GENLIN needs a numeric variable here, but {} is a string variable",
                    v.name()
                )));
            }
        }
        let factor_names: Vec<String> = factor_vars.iter().map(|v| v.name().to_string()).collect();
        let covariate_names: Vec<String> = cov_vars.iter().map(|v| v.name().to_string()).collect();
        let numeric = |v: &Variable, k: usize| -> Option<f64> {
            if !crate::means::is_valid(v, k, false) {
                return None;
            }
            match v.values() {
                Values::Numeric(values) => values.get(k),
                Values::String(_) => None,
            }
        };
        let cases = data.case_count();
        let weights = case_weights(data)?;
        let freq = |k: usize| -> f64 {
            match &weights {
                Some(w) if w[k].is_finite() && w[k] >= 0.5 => w[k].round(),
                Some(_) => 0.0,
                None => 1.0,
            }
        };
        let factor_keys: Vec<Vec<Option<Key>>> = factor_vars
            .iter()
            .map(|v| categories(v, command.include))
            .collect();
        let dep_keys = categories(&dependent, false);
        // user-missing values of the subject variables are always missing
        let subject_keys: Vec<Vec<Option<Key>>> =
            subject_vars.iter().map(|v| categories(v, false)).collect();
        let within_keys: Vec<Vec<Option<Key>>> =
            within_vars.iter().map(|v| categories(v, false)).collect();
        // everything but the response
        let predictors_valid: Vec<bool> = (0..cases)
            .map(|k| {
                factor_keys.iter().all(|col| col[k].is_some())
                    && cov_vars.iter().all(|v| numeric(v, k).is_some())
                    && offset_var.as_ref().is_none_or(|v| numeric(v, k).is_some())
            })
            .collect();
        let others_valid = |k: usize| {
            predictors_valid[k]
                && subject_keys
                    .iter()
                    .chain(&within_keys)
                    .all(|col| col[k].is_some())
                && freq(k) >= 1.0
                && weight_var
                    .as_ref()
                    .is_none_or(|v| numeric(v, k).is_some_and(|w| w > 0.0))
        };
        let whole = |v: f64| v.fract() == 0.0;
        let response_value = |k: usize| -> Option<(f64, f64)> {
            // (y, trials); y the proportion for events/trials
            match &command.response {
                GenlinResponse::Events { trials, .. } => {
                    let r = numeric(&dependent, k)?;
                    let m = match trials {
                        GenlinTrials::Number(n) => *n,
                        GenlinTrials::Variable(_) => numeric(trials_var.as_ref()?, k)?,
                    };
                    (r >= 0.0 && whole(r) && m >= 1.0 && whole(m) && m >= r).then_some((r / m, m))
                }
                GenlinResponse::Dependent { .. } if categorical => {
                    dep_keys[k].as_ref().map(|_| (0.0, 1.0))
                }
                GenlinResponse::Dependent { .. } => {
                    let y = numeric(&dependent, k)?;
                    let ok = match family {
                        Family::Gamma | Family::InverseGaussian => y > 0.0,
                        Family::Poisson | Family::NegBin => y >= 0.0 && whole(y),
                        _ => true,
                    };
                    ok.then_some((y, 1.0))
                }
            }
        };
        let rows: Vec<usize> = (0..cases)
            .filter(|&k| others_valid(k) && response_value(k).is_some())
            .collect();
        let mut used = vec![false; cases];
        for &k in &rows {
            used[k] = true;
        }
        // + 0.0 turns the -0.0 of an empty sum into 0
        let excluded: f64 = (0..cases)
            .filter(|&k| freq(k) >= 1.0 && !used[k])
            .map(freq)
            .sum::<f64>()
            + 0.0;
        if rows.is_empty() {
            return Err(Error(
                "GENLIN: no case has valid values on every variable and a positive weight".into(),
            ));
        }
        let sorted = |col: &[Option<Key>], order: GenlinOrder| -> Vec<Key> {
            let mut found: Vec<Key> = Vec::new();
            for &k in &rows {
                let key = col[k].as_ref().expect("valid");
                if !found.iter().any(|l| l.compare(key).is_eq()) {
                    found.push(key.clone());
                }
            }
            match order {
                GenlinOrder::Ascending => found.sort_by(|a, b| a.compare(b)),
                GenlinOrder::Descending => found.sort_by(|a, b| b.compare(a)),
                GenlinOrder::Data => {}
            }
            found
        };
        let levels: Vec<Vec<Key>> = factor_keys
            .iter()
            .map(|c| sorted(c, command.factor_order))
            .collect();
        let level_of = |keys: &[Key], key: &Key| keys.iter().position(|l| l.compare(key).is_eq());
        // the categories of a categorical response; for a binary one the modeled value first
        let mut categories_of: Vec<Key> = Vec::new();
        if categorical {
            let order = match &command.response {
                GenlinResponse::Dependent { order, .. } if multinomial => *order,
                _ => GenlinOrder::Ascending,
            };
            categories_of = sorted(&dep_keys, order);
            if multinomial && categories_of.len() < 2 {
                return Err(Error(format!(
                    "GENLIN: the multinomial dependent {} needs at least two values among the cases used",
                    dependent.name()
                )));
            }
            if !multinomial {
                if categories_of.len() != 2 {
                    return Err(Error(format!(
                        "GENLIN: the binomial dependent {} needs exactly two values among the cases used; it has {}",
                        dependent.name(),
                        categories_of.len()
                    )));
                }
                let reference = match &command.response {
                    GenlinResponse::Dependent { reference, .. } => reference,
                    GenlinResponse::Events { .. } => &GenlinReference::Last,
                };
                let reference_at = match reference {
                    GenlinReference::Last => 1,
                    GenlinReference::First => 0,
                    GenlinReference::Value(v) => categories_of
                        .iter()
                        .position(|k| value_matches(&dependent, k, v))
                        .ok_or_else(|| {
                            Error(format!(
                                "GENLIN: the REFERENCE value is not a value of {} among the cases used",
                                dependent.name()
                            ))
                        })?,
                };
                if reference_at == 0 {
                    categories_of.swap(0, 1);
                }
            }
        }
        let labels = Shown::labels(shown.currencies.clone());
        let n_rows = rows.len();
        let d = Data {
            factors: factor_names.clone(),
            factor_texts: factor_vars.iter().map(|v| shown.variable(v)).collect(),
            values: factor_vars
                .iter()
                .zip(&levels)
                .map(|(v, ls)| ls.iter().map(|k| key_value_text(v, k)).collect())
                .collect(),
            labels: factor_vars
                .iter()
                .zip(&levels)
                .map(|(v, ls)| ls.iter().map(|k| labels.value(v, k)).collect())
                .collect(),
            labelled: factor_vars.iter().any(|v| !v.value_labels().is_empty())
                && shown.values != Show::Plain,
            covariates: covariate_names.clone(),
            level: rows
                .iter()
                .map(|&k| {
                    factor_keys
                        .iter()
                        .zip(&levels)
                        .map(|(col, ls)| {
                            level_of(ls, col[k].as_ref().expect("valid")).expect("a level")
                        })
                        .collect()
                })
                .collect(),
            cov: rows
                .iter()
                .map(|&k| {
                    cov_vars
                        .iter()
                        .map(|v| numeric(v, k).expect("valid"))
                        .collect()
                })
                .collect(),
            y: vec![Vec::new(); n_rows],
            f: rows.iter().map(|&k| freq(k)).collect(),
            g: rows
                .iter()
                .map(|&k| {
                    weight_var
                        .as_ref()
                        .map_or(1.0, |v| numeric(v, k).expect("valid"))
                })
                .collect(),
            case: rows.clone(),
        };
        let offset_of = |k: usize| -> f64 {
            match &command.offset {
                Some(GenlinOffset::Value(v)) => *v,
                Some(GenlinOffset::Variable(_)) => offset_var
                    .as_ref()
                    .and_then(|v| numeric(v, k))
                    .unwrap_or(0.0),
                None => 0.0,
            }
        };
        let category: Vec<usize> = rows
            .iter()
            .map(|&k| {
                dep_keys[k]
                    .as_ref()
                    .and_then(|key| level_of(&categories_of, key))
                    .unwrap_or(0)
            })
            .collect();
        let c = Cases {
            y: rows
                .iter()
                .zip(&category)
                .map(|(&k, &cat)| {
                    if categorical {
                        // the modeled value is 1, the reference 0
                        f64::from(u8::from(cat == 0))
                    } else {
                        response_value(k).expect("valid").0
                    }
                })
                .collect(),
            m: rows
                .iter()
                .map(|&k| response_value(k).expect("valid").1)
                .collect(),
            f: d.f.clone(),
            omega: d.g.clone(),
            offset: rows.iter().map(|&k| offset_of(k)).collect(),
        };
        // the design
        let mut terms: Vec<Term> = command
            .effects
            .iter()
            .filter(|t| !t.variables.is_empty())
            .cloned()
            .collect();
        if command.intercept || multinomial {
            terms.insert(0, intercept_term());
        }
        let effects = effects_of(&terms, &factor_names, &covariate_names)?;
        let design = Fit::new(&d, effects, &d.y, crit.singular);
        let kept: Vec<usize> = (0..design.p)
            .filter(|&j| !design.redundant[j])
            .filter(|&j| !(multinomial && design.effects[0].is_intercept() && j == 0))
            .collect();
        let x: Vec<Vec<f64>> = design
            .x
            .iter()
            .map(|row| kept.iter().map(|&j| row[j]).collect())
            .collect();
        let px = kept.len();
        let has_intercept = command.intercept && !multinomial;
        let n_total: f64 = c.f.iter().sum();
        let mut warnings: Vec<String> = Vec::new();
        // INITIAL: a start value for every parameter, redundant ones too; the fit takes those
        // of the kept columns
        let start_values: Option<Vec<f64>> = match &command.initial {
            Some(_) if multinomial => {
                warnings.push(
                    "GENLIN: INITIAL is not used for the ordinal multinomial model; the initial values are computed".into(),
                );
                None
            }
            Some(_) if family == Family::Normal && command.link == GenlinLink::Identity => {
                warnings.push(
                    "GENLIN: INITIAL is ignored for the normal distribution with the identity link, which needs no iteration".into(),
                );
                None
            }
            Some(values) => {
                if values.len() != design.p {
                    return Err(Error(format!(
                        "GENLIN: INITIAL gives {} values and the model has {} parameters (redundant ones included)",
                        values.len(),
                        design.p
                    )));
                }
                Some(kept.iter().map(|&j| values[j]).collect())
            }
            None => None,
        };
        // the subjects of the estimating equations; the keys of each row of the fit
        let row_keys = |keys: &[Vec<Option<Key>>]| -> Vec<Vec<Key>> {
            rows.iter()
                .map(|&k| {
                    keys.iter()
                        .map(|col| col[k].clone().expect("valid"))
                        .collect()
                })
                .collect()
        };
        let subjects = match repeated {
            Some(r) => {
                let within = (!within_vars.is_empty()).then(|| row_keys(&within_keys));
                if !r.sort && within.is_some() {
                    warnings.push("WITHINSUBJECT is ignored with SORT=NO: the cases of each subject are taken in file order (a case with a missing WITHINSUBJECT value is still left out)".into());
                }
                // the positions are the WITHINSUBJECT values of every case with valid subject
                // and within values, also one left out for another reason
                let every: Vec<Vec<Key>> = (0..cases)
                    .filter(|&k| within.is_some() && r.sort && freq(k) >= 1.0)
                    .filter(|&k| {
                        subject_keys
                            .iter()
                            .chain(&within_keys)
                            .all(|col| col[k].is_some())
                    })
                    .map(|k| {
                        within_keys
                            .iter()
                            .map(|col| col[k].clone().expect("valid"))
                            .collect()
                    })
                    .collect();
                // without sorting a subject is a run of equal values in the whole file, so a
                // case left out between two runs still separates them
                let subject_of_row = if r.sort {
                    row_keys(&subject_keys)
                } else {
                    // a case with a missing subject value or no weight is not used and does not
                    // end a run
                    let mut run = vec![0.0; cases];
                    let mut last: Option<usize> = None;
                    let mut number = 0.0;
                    for k in 0..cases {
                        if freq(k) < 1.0 || subject_keys.iter().any(|col| col[k].is_none()) {
                            continue;
                        }
                        if let Some(l) = last {
                            let same = subject_keys.iter().all(|col| {
                                col[l]
                                    .as_ref()
                                    .expect("valid")
                                    .compare(col[k].as_ref().expect("valid"))
                                    .is_eq()
                            });
                            if !same {
                                number += 1.0;
                            }
                        }
                        run[k] = number;
                        last = Some(k);
                    }
                    rows.iter().map(|&k| vec![Key::Number(run[k])]).collect()
                };
                let s = gee::subjects(
                    &subject_of_row,
                    within
                        .as_deref()
                        .filter(|_| r.sort)
                        .map(|w| (w, &every[..])),
                    r.sort,
                )
                .map_err(Error)?;
                // a frequency weight copies a whole subject
                if s.members
                    .iter()
                    .any(|m| m.iter().any(|&(i, _)| c.f[i] != c.f[m[0].0]))
                {
                    return Err(Error(
                        "GENLIN: REPEATED: the frequency weight (WEIGHT) must be the same for every case of a subject".into(),
                    ));
                }
                Some(s)
            }
            None => None,
        };
        let spec = Spec {
            family,
            link: command.link,
            k: fixed_k,
            scale,
            criteria: crit,
            full: !crit.kernel,
        };
        // subpopulations: the cases with the same factor levels, covariates and offset
        let mut order: Vec<usize> = (0..n_rows).collect();
        let pattern = |a: &usize, b: &usize| {
            d.level[*a].cmp(&d.level[*b]).then_with(|| {
                d.cov[*a]
                    .iter()
                    .chain(std::iter::once(&c.offset[*a]))
                    .zip(d.cov[*b].iter().chain(std::iter::once(&c.offset[*b])))
                    .map(|(x, y)| (x + 0.0).total_cmp(&(y + 0.0)))
                    .find(|o| o.is_ne())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        };
        order.sort_by(pattern);
        let mut sub_of = vec![0; n_rows];
        let mut subs = 0;
        for (at, &r) in order.iter().enumerate() {
            if at > 0 && pattern(&order[at - 1], &r).is_ne() {
                subs += 1;
            }
            sub_of[r] = subs;
        }
        let ns = subs + 1;
        let j_categories = categories_of.len();
        let ordinal = Ordinal {
            category: category.clone(),
            f: c.f.clone(),
            omega: c.omega.clone(),
            offset: c.offset.clone(),
            j: j_categories,
        };
        let ord_link = command.link;
        // a fit of the model on the columns `xs`; the log likelihood at the tests' scale
        let fixed_phi = match scale {
            ScaleMode::Fixed(v) => v,
            _ => 1.0,
        };
        // the reduced and constrained models stop on the log likelihood alone: LCONVERGE, or
        // 1E-4 absolute when it is 0
        let mut refit_criteria = *crit;
        refit_criteria.pconverge.value = 0.0;
        refit_criteria.hconverge.value = 0.0;
        if crit.lconverge.value == 0.0 {
            refit_criteria.lconverge = statscraft_syntax::GenlinTolerance {
                value: 1e-4,
                relative: false,
            };
        }
        let refit_spec = Spec {
            criteria: &refit_criteria,
            ..spec
        };
        let refit = |xs: &[Vec<f64>]| -> Option<f64> {
            if multinomial {
                fit_ordinal(&ordinal, xs, ord_link, &refit_criteria, fixed_phi)
                    .map(|f| f.ll / fixed_phi)
            } else {
                fit_glm(&c, xs, &refit_spec, None).map(|f| f.ll)
            }
        };
        let failed = || {
            Error(
                "GENLIN: the model could not be fitted; no estimates give every case a valid mean"
                    .into(),
            )
        };
        let mut glm_fit;
        let mut ordinal_fit = None;
        let result = if multinomial {
            glm_fit = None;
            let f = fit_ordinal(&ordinal, &x, ord_link, crit, fixed_phi).ok_or_else(failed)?;
            if !f.converged {
                warnings.push(not_converged(crit, f.halving_failed));
            } else if !f.hessian_ok {
                warnings.push(HESSIAN_WARNING.into());
            }
            let r = Result_ {
                ll: f.ll / fixed_phi,
                b: f.b.clone(),
                cov: f.cov.clone(),
                phi: fixed_phi,
                k: 0.0,
                log_variance: None,
            };
            ordinal_fit = Some(f);
            r
        } else {
            let f = fit_glm(&c, &x, &spec, start_values.as_deref()).ok_or_else(failed)?;
            if !f.converged {
                warnings.push(not_converged(crit, f.halving_failed));
            } else if !f.hessian_ok {
                warnings.push(HESSIAN_WARNING.into());
            }
            let r = Result_ {
                b: f.beta.clone(),
                cov: f.cov.clone(),
                ll: f.ll,
                phi: f.phi,
                k: f.k,
                log_variance: f.log_variance,
            };
            glm_fit = Some(f);
            r
        };
        let mut result = result;
        // goodness of fit
        let k_estimated = family == Family::NegBin && fixed_k.is_none() && !multinomial;
        let binary = family == Family::Binomial && !events && !multinomial;
        let mut deviance = 0.0;
        let mut pearson = 0.0;
        let mut constant = 0.0;
        let df_fit;
        let probabilities: Vec<Vec<f64>> = if multinomial {
            (0..n_rows)
                .map(|i| {
                    cell_probabilities(&cumulative(
                        ord_link,
                        &result.b,
                        &x[i],
                        c.offset[i],
                        j_categories - 1,
                    ))
                })
                .collect()
        } else {
            Vec::new()
        };
        // complete separation: every case gets its observed response with a probability above
        // 0.99 (the quasi-complete case is not checked)
        if crit.checksep > 0 && (family == Family::Binomial || multinomial) {
            // a proportion strictly between 0 and 1 cannot be separated
            let observed = (0..n_rows).map(|i| match &glm_fit {
                Some(f) if c.y[i] == 1.0 => f.mu[i],
                Some(f) if c.y[i] == 0.0 => 1.0 - f.mu[i],
                Some(_) => 0.0,
                None => probabilities[i][category[i]],
            });
            if observed.fold(1.0, f64::min) > 0.99 {
                warnings.push("There is complete separation in the data: the predictors give every case its observed response with a probability above 0.99. The estimates are not unique and may be very large".into());
            }
        }
        if multinomial {
            let mut r = vec![vec![0.0; j_categories]; ns];
            let mut pi = vec![Vec::new(); ns];
            for i in 0..n_rows {
                r[sub_of[i]][category[i]] += c.f[i] * c.omega[i];
                pi[sub_of[i]] = probabilities[i].clone();
            }
            for s in 0..ns {
                let m: f64 = r[s].iter().sum();
                for j in 0..j_categories {
                    let e = m * pi[s][j];
                    deviance += 2.0 * xlog(r[s][j], e);
                    if e > 0.0 {
                        pearson += (r[s][j] - e).powi(2) / e;
                    }
                }
                constant += ln_gamma(m + 1.0) - r[s].iter().map(|v| ln_gamma(v + 1.0)).sum::<f64>();
            }
            df_fit = (ns * (j_categories - 1)) as f64 - (j_categories - 1 + px) as f64;
        } else {
            let f = glm_fit.as_ref().expect("fitted");
            if binary {
                let mut r1 = vec![0.0; ns];
                let mut m = vec![0.0; ns];
                let mut mu = vec![0.0; ns];
                for i in 0..n_rows {
                    r1[sub_of[i]] += c.f[i] * c.omega[i] * c.y[i];
                    m[sub_of[i]] += c.f[i] * c.omega[i];
                    mu[sub_of[i]] = f.mu[i];
                }
                for s in 0..ns {
                    let y = r1[s] / m[s];
                    deviance += 2.0 * m[s] * (xlog(y, mu[s]) + xlog(1.0 - y, 1.0 - mu[s]));
                    pearson += m[s] * (y - mu[s]).powi(2) / (mu[s] * (1.0 - mu[s]));
                    constant +=
                        ln_gamma(m[s] + 1.0) - ln_gamma(r1[s] + 1.0) - ln_gamma(m[s] - r1[s] + 1.0);
                }
                df_fit = ns as f64 - px as f64;
            } else {
                for i in 0..n_rows {
                    let a = c.f[i] * c.a(i);
                    deviance += a * unit_deviance(family, c.y[i], f.mu[i], f.k);
                    pearson += a * (c.y[i] - f.mu[i]).powi(2) / variance(family, f.mu[i], f.k);
                }
                df_fit = n_total - px as f64 - f64::from(u8::from(k_estimated));
            }
        }
        if crit.kernel {
            constant = 0.0;
        }
        match scale {
            ScaleMode::Deviance | ScaleMode::Pearson => {
                let value = if scale == ScaleMode::Deviance {
                    deviance
                } else {
                    pearson
                };
                if df_fit <= 0.0 {
                    return Err(Error(
                        "GENLIN: SCALE=DEVIANCE or PEARSON needs more cases (or subpopulations) than parameters".into(),
                    ));
                }
                result.phi = value / df_fit;
            }
            _ => {}
        }
        let phi = result.phi;
        let scale_estimated = !matches!(scale, ScaleMode::Fixed(_));
        let exact = match &glm_fit {
            Some(f) => exact_fit(&c, family, &f.mu),
            None => deviance <= 0.0,
        };
        if scale_estimated && (exact || phi.is_nan() || phi <= 0.0) {
            return Err(Error(
                "GENLIN: the model fits every case exactly, so the scale cannot be estimated; fix it with CRITERIA SCALE=number".into(),
            ));
        }
        if k_estimated {
            result.log_variance = result.log_variance.map(|v| v * phi);
            if result.k < 1e-6 {
                warnings.push(format!(
                    "The negative binomial k ({:.3e}) is at the boundary 0: the counts are not overdispersed, a Poisson model fits as well, and the standard error and interval of k are not reliable",
                    result.k
                ));
            }
        }
        // generalized estimating equations from the fit so far
        let gee_fit = match (repeated, &subjects, glm_fit.as_mut()) {
            (Some(r), Some(s), Some(f)) => {
                let set = gee::Settings::of(
                    r,
                    (family, command.link, result.k),
                    scale == ScaleMode::Mle,
                    crit.singular,
                );
                let g =
                    gee::fit(&c, &x, s, &set, (&result.b, phi), result.b.len()).map_err(Error)?;
                if g.halving_failed {
                    warnings.push("The generalized estimating equations stopped: no shortened step kept every mean valid; the results may not be final".into());
                } else if !g.converged {
                    warnings.push(format!(
                        "The generalized estimating equations did not converge within {} iterations (REPEATED MAXITERATIONS); the results may not be final",
                        r.maxiterations
                    ));
                } else if !g.hessian_ok {
                    warnings.push("The Hessian convergence criterion of the generalized estimating equations (1E-4, REPEATED HCONVERGE=0) is not met after the estimates converged; the results may not be final".into());
                }
                match g.adjusted {
                    gee::Adjusted::Ridge => warnings.push("The estimated working correlation matrix was not positive definite; it was moved toward the identity matrix until it was".into()),
                    gee::Adjusted::Clamp => warnings.push("The estimated AR(1) correlation was above 0.999 in size; it was set to 0.999 with its sign so that the matrix is safely positive definite".into()),
                    gee::Adjusted::No => {}
                }
                // H of the independent structure at its estimates (those of the plain model) and
                // at the scale of the quasi-likelihood, for QIC
                let phi_q = match scale {
                    ScaleMode::Fixed(v) => v,
                    _ => 1.0,
                };
                let independent = command.print.fit.then(|| {
                    gee::independent_information(&c, &x, &set, &result.b, phi_q)
                        .expect("the means of the plain model are valid")
                });
                f.beta = g.beta.clone();
                f.mu = g.mu.clone();
                result.b = g.beta.clone();
                result.phi = g.phi;
                result.cov = if r.robust {
                    g.robust.clone()
                } else {
                    g.model.clone()
                };
                result.log_variance = None;
                Some((g, independent, phi_q))
            }
            _ => None,
        };
        let phi = result.phi;
        let gee = gee_fit.is_some();
        // the covariance at the scale, robust if asked
        if !gee {
            result.cov = result
                .cov
                .iter()
                .map(|r| r.iter().map(|v| v * phi).collect())
                .collect();
        }
        if crit.robust && !gee {
            let q = result.b.len();
            let mut meat = vec![vec![0.0; q]; q];
            for i in 0..n_rows {
                let s: Vec<f64> = if multinomial {
                    ordinal_case_score(&ordinal, &x, ord_link, &result.b, i)
                        .iter()
                        .map(|v| v / phi)
                        .collect()
                } else {
                    let f = glm_fit.as_ref().expect("fitted");
                    let m = f.mu[i];
                    let sc = c.a(i) * (c.y[i] - m)
                        / (phi * variance(family, m, f.k) * link_d1(command.link, m, f.k));
                    x[i].iter().map(|v| v * sc).collect()
                };
                for a in 0..q {
                    for bb in 0..q {
                        meat[a][bb] += c.f[i] * s[a] * s[bb];
                    }
                }
            }
            let m1 = crate::matrix::multiply(&result.cov, &meat);
            result.cov = crate::matrix::multiply(&m1, &result.cov);
        }
        let ll_shown = |ll_at_phi: f64| ll_at_phi + constant / phi;
        // the log likelihood shown: at scale 1 for DEVIANCE and PEARSON
        let ll_one = result.ll + constant / fixed_phi;
        let ll_adjusted = match (scale, &glm_fit) {
            (ScaleMode::Deviance | ScaleMode::Pearson, Some(f)) if !gee => Some(
                (0..n_rows)
                    .map(|i| {
                        c.f[i]
                            * case_loglik(
                                family,
                                c.y[i],
                                f.mu[i],
                                phi,
                                c.a(i),
                                c.omega[i],
                                c.m[i],
                                f.k,
                                spec.full,
                            )
                    })
                    .sum::<f64>()
                    + constant / phi,
            ),
            (ScaleMode::Deviance | ScaleMode::Pearson, None) => Some(ll_shown(result.ll / phi)),
            _ => None,
        };
        // the divisor of likelihood ratio statistics
        let lr_scale = match scale {
            ScaleMode::Deviance | ScaleMode::Pearson => phi,
            _ => 1.0,
        };
        let model_label = {
            let mut parts: Vec<String> = Vec::new();
            if multinomial {
                parts.push("(Threshold)".into());
            }
            for e in &design.effects {
                if e.is_intercept() {
                    if !multinomial {
                        parts.push("(Intercept)".into());
                    }
                } else {
                    parts.push(e.label.clone());
                }
            }
            parts.join(", ")
        };
        let dep_text = shown.variable(&dependent);
        let caption = format!("Dependent Variable: {dep_text}. Model: {model_label}.");
        let print = command.print;
        let mut tables = Vec::new();
        // Model Information
        if print.modelinfo {
            let mut out = vec![];
            if events {
                out.push(vec![
                    text("Events Variable"),
                    text(shown.variable(&dependent)),
                ]);
                match &command.response {
                    GenlinResponse::Events {
                        trials: GenlinTrials::Number(n),
                        ..
                    } => out.push(vec![text("Number of Trials"), Cell::Number(*n)]),
                    _ => out.push(vec![
                        text("Trials Variable"),
                        text(
                            trials_var
                                .as_ref()
                                .map(|v| shown.variable(v))
                                .unwrap_or_default(),
                        ),
                    ]),
                }
            } else {
                out.push(vec![text("Dependent Variable"), text(dep_text.clone())]);
            }
            out.push(vec![
                text("Probability Distribution"),
                text(distribution_name(distribution, fixed_k)),
            ]);
            out.push(vec![text("Link Function"), text(link_name(command.link))]);
            match &command.offset {
                Some(GenlinOffset::Variable(_)) => out.push(vec![
                    text("Offset Variable"),
                    text(
                        offset_var
                            .as_ref()
                            .map(|v| shown.variable(v))
                            .unwrap_or_default(),
                    ),
                ]),
                Some(GenlinOffset::Value(v)) => {
                    out.push(vec![text("Offset Value"), Cell::Number(*v)])
                }
                None => {}
            }
            if let Some(v) = &weight_var {
                out.push(vec![text("Scale Weight Variable"), text(shown.variable(v))]);
            }
            if let Some(r) = repeated {
                let effect = |vars: &[Variable]| {
                    vars.iter()
                        .map(|v| shown.variable(v))
                        .collect::<Vec<_>>()
                        .join(" * ")
                };
                out.push(vec![text("Subject Effect"), text(effect(&subject_vars))]);
                if !within_vars.is_empty() && r.sort {
                    out.push(vec![
                        text("Within-Subject Effect"),
                        text(effect(&within_vars)),
                    ]);
                }
                out.push(vec![
                    text("Working Correlation Matrix Structure"),
                    text(correlation_name(
                        &r.correlation,
                        subjects.as_ref().map_or(0, |s| s.dim),
                    )),
                ]);
            }
            let note = binary.then(|| {
                format!(
                    "The procedure models {} as the response, treating {} as the reference category.",
                    shown.value(&dependent, &categories_of[0]),
                    shown.value(&dependent, &categories_of[1])
                )
            });
            tables.push(table(
                "Model Information",
                1,
                plain(&["", "Value"]),
                out,
                note,
            ));
        }
        // Case Processing Summary
        if print.cps {
            let total = n_total + excluded;
            let row = |name: &str, n: f64| {
                vec![
                    text(name),
                    Cell::Number(n),
                    number((total > 0.0).then(|| 100.0 * n / total)),
                ]
            };
            tables.push(table(
                "Case Processing Summary",
                1,
                plain(&["", "N", "Percent"]),
                vec![
                    row("Included", n_total),
                    row("Excluded", excluded),
                    row("Total", total),
                ],
                None,
            ));
            if let (Some(r), Some(s)) = (repeated, &subjects) {
                let distinct = |mut found: Vec<&Key>| {
                    found.sort_by(|a, b| a.compare(b));
                    found.dedup_by(|a, b| a.compare(b).is_eq());
                    found.len() as f64
                };
                let levels_of = |keys: &[Vec<Option<Key>>], at: usize| {
                    distinct(
                        rows.iter()
                            .map(|&k| keys[at][k].as_ref().expect("valid"))
                            .collect(),
                    )
                };
                // the within-subject levels of the positions, which count every valid case
                let within_levels =
                    |at: usize| distinct(s.positions.iter().map(|p| &p[at]).collect());
                let mut out = Vec::new();
                for (at, v) in subject_vars.iter().enumerate() {
                    out.push(vec![
                        text(if at == 0 {
                            "Levels of the Subject Effect"
                        } else {
                            ""
                        }),
                        text(shown.variable(v)),
                        Cell::Number(levels_of(&subject_keys, at)),
                    ]);
                }
                if r.sort {
                    for (at, v) in within_vars.iter().enumerate() {
                        out.push(vec![
                            text(if at == 0 {
                                "Levels of the Within-Subject Effect"
                            } else {
                                ""
                            }),
                            text(shown.variable(v)),
                            Cell::Number(within_levels(at)),
                        ]);
                    }
                }
                let sizes = s.members.iter().map(Vec::len);
                // a frequency weight copies whole subjects
                out.push(vec![
                    text("Number of Subjects"),
                    Cell::Empty,
                    Cell::Number(s.members.iter().map(|m| c.f[m[0].0]).sum()),
                ]);
                out.push(vec![
                    text("Measurements per Subject"),
                    text("Minimum"),
                    Cell::Number(sizes.clone().min().unwrap_or(0) as f64),
                ]);
                out.push(vec![
                    Cell::Empty,
                    text("Maximum"),
                    Cell::Number(sizes.max().unwrap_or(0) as f64),
                ]);
                out.push(vec![
                    text("Working Correlation Matrix Dimension"),
                    Cell::Empty,
                    Cell::Number(s.dim as f64),
                ]);
                for row in &mut out {
                    if matches!(&row[0], Cell::Text(t) if t.is_empty()) {
                        row[0] = Cell::Empty;
                    }
                }
                tables.push(table(
                    "Correlated Data Summary",
                    2,
                    plain(&["", "", "N"]),
                    out,
                    None,
                ));
            }
        }
        if print.descriptives {
            let mut out: Vec<Vec<Cell>> = Vec::new();
            let mut block = |kind: &str, v: &Variable, ls: &[Key], of: &dyn Fn(usize) -> usize| {
                let mut n = vec![0.0; ls.len()];
                for r in 0..n_rows {
                    n[of(r)] += c.f[r];
                }
                for (l, key) in ls.iter().enumerate() {
                    out.push(vec![
                        text(if l == 0 { kind } else { "" }),
                        text(if l == 0 {
                            shown.variable(v)
                        } else {
                            String::new()
                        }),
                        text(shown.value(v, key)),
                        Cell::Number(n[l]),
                        Cell::Number(100.0 * n[l] / n_total),
                    ]);
                }
                out.push(vec![
                    Cell::Empty,
                    Cell::Empty,
                    text("Total"),
                    Cell::Number(n_total),
                    Cell::Number(100.0),
                ]);
            };
            if categorical {
                block("Dependent Variable", &dependent, &categories_of, &|r| {
                    category[r]
                });
            }
            for (fi, v) in factor_vars.iter().enumerate() {
                block(if fi == 0 { "Factor" } else { "" }, v, &levels[fi], &|r| {
                    d.level[r][fi]
                });
            }
            if !out.is_empty() {
                // an empty header cell carries the one above
                for row in &mut out {
                    for cell in row.iter_mut().take(2) {
                        if matches!(cell, Cell::Text(t) if t.is_empty()) {
                            *cell = Cell::Empty;
                        }
                    }
                }
                tables.push(table(
                    "Categorical Variable Information",
                    3,
                    plain(&["", "", "", "N", "Percent"]),
                    out,
                    None,
                ));
            }
            let mut out: Vec<Vec<Cell>> = Vec::new();
            let mut describe = |kind: &str, v: &Variable, values: &dyn Fn(usize) -> f64| {
                let (mut lo, mut hi, mut sum, mut sum2) =
                    (f64::INFINITY, f64::NEG_INFINITY, 0.0, 0.0);
                for (r, f) in c.f.iter().enumerate().take(n_rows) {
                    let val = values(r);
                    lo = lo.min(val);
                    hi = hi.max(val);
                    sum += f * val;
                }
                let mean = sum / n_total;
                for (r, f) in c.f.iter().enumerate().take(n_rows) {
                    sum2 += f * (values(r) - mean).powi(2);
                }
                out.push(vec![
                    text(kind),
                    text(shown.variable(v)),
                    Cell::Number(n_total),
                    Cell::Number(lo),
                    Cell::Number(hi),
                    Cell::Number(mean),
                    number((n_total > 1.0).then(|| (sum2 / (n_total - 1.0)).sqrt())),
                ]);
            };
            if !categorical {
                if events {
                    describe("Dependent Variable", &dependent, &|r| c.y[r] * c.m[r]);
                    if let Some(t) = &trials_var {
                        describe("", t, &|r| c.m[r]);
                    }
                } else {
                    describe("Dependent Variable", &dependent, &|r| c.y[r]);
                }
            }
            for (ci, v) in cov_vars.iter().enumerate() {
                describe(if ci == 0 { "Covariate" } else { "" }, v, &|r| d.cov[r][ci]);
            }
            if let Some(v) = &offset_var {
                describe("Offset", v, &|r| c.offset[r]);
            }
            if !out.is_empty() {
                for row in &mut out {
                    if matches!(&row[0], Cell::Text(t) if t.is_empty()) {
                        row[0] = Cell::Empty;
                    }
                }
                tables.push(table(
                    "Continuous Variable Information",
                    2,
                    plain(&["", "", "N", "Minimum", "Maximum", "Mean", "Std. Deviation"]),
                    out,
                    None,
                ));
            }
        }
        // the number of estimated parameters for the information criteria
        let d_params = if multinomial {
            (j_categories - 1 + px) as f64
        } else {
            px as f64
                + f64::from(u8::from(scale == ScaleMode::Mle))
                + f64::from(u8::from(k_estimated))
        };
        let n_ic: f64 = if events {
            (0..n_rows).map(|i| c.f[i] * c.m[i]).sum()
        } else {
            n_total
        };
        if let (true, Some((g, independent, phi_q))) = (print.fit, &gee_fit) {
            // the quasi-likelihood at a scale fixed on CRITERIA, or else 1
            let phi_q = *phi_q;
            let q: f64 = (0..n_rows)
                .map(|i| {
                    c.f[i] * c.a(i) / phi_q
                        * gee::quasi(family, c.y[i], g.mu[i], result.k, !crit.kernel)
                })
                .sum();
            // trace(H C) of two symmetric matrices
            let trace: Option<f64> = independent.as_ref().map(|h| {
                h.iter()
                    .flatten()
                    .zip(g.robust.iter().flatten())
                    .map(|(a, b)| a * b)
                    .sum()
            });
            tables.push(table(
                "Goodness of Fit",
                1,
                plain(&["", "Value"]),
                vec![
                    vec![
                        text("Quasi-likelihood under Independence Model Criterion (QIC)"),
                        number(trace.map(|t| -2.0 * q + 2.0 * t)),
                    ],
                    vec![
                        text("Corrected Quasi-likelihood under Independence Model Criterion (QICC)"),
                        Cell::Number(-2.0 * q + 2.0 * px as f64),
                    ],
                ],
                Some(format!(
                    "{caption} Smaller values are better. Both criteria use the {} quasi-likelihood under independence at a scale of {}.",
                    if crit.kernel { "kernel of the" } else { "full" },
                    phi_q,
                )),
            ));
        } else if print.fit {
            let ll = ll_one;
            let aic = -2.0 * ll + 2.0 * d_params;
            let aicc = if n_ic - d_params - 1.0 > 0.0 {
                -2.0 * ll + 2.0 * d_params * n_ic / (n_ic - d_params - 1.0)
            } else {
                f64::NAN
            };
            let bic = -2.0 * ll + d_params * n_ic.ln();
            let caic = -2.0 * ll + d_params * (n_ic.ln() + 1.0);
            let per = |v: f64| number((df_fit > 0.0).then(|| v / df_fit));
            let mut out = vec![
                vec![
                    text("Deviance"),
                    Cell::Number(deviance),
                    Cell::Number(df_fit),
                    per(deviance),
                ],
                vec![
                    text("Scaled Deviance"),
                    Cell::Number(deviance / phi),
                    Cell::Number(df_fit),
                    Cell::Empty,
                ],
                vec![
                    text("Pearson Chi-Square"),
                    Cell::Number(pearson),
                    Cell::Number(df_fit),
                    per(pearson),
                ],
                vec![
                    text("Scaled Pearson Chi-Square"),
                    Cell::Number(pearson / phi),
                    Cell::Number(df_fit),
                    Cell::Empty,
                ],
                vec![
                    text("Log Likelihood"),
                    Cell::Number(ll),
                    Cell::Empty,
                    Cell::Empty,
                ],
            ];
            if let Some(adjusted) = ll_adjusted {
                out.push(vec![
                    text("Adjusted Log Likelihood"),
                    Cell::Number(adjusted),
                    Cell::Empty,
                    Cell::Empty,
                ]);
            }
            for (name, v) in [
                ("Akaike's Information Criterion (AIC)", aic),
                ("Finite Sample Corrected AIC (AICC)", aicc),
                ("Bayesian Information Criterion (BIC)", bic),
                ("Consistent AIC (CAIC)", caic),
            ] {
                out.push(vec![
                    text(name),
                    number(v.is_finite().then_some(v)),
                    Cell::Empty,
                    Cell::Empty,
                ]);
            }
            let mut note = format!(
                "{caption} Information criteria are in smaller-is-better form. The {} log likelihood function is displayed and used in computing information criteria.",
                if crit.kernel { "kernel of the" } else { "full" }
            );
            if ll_adjusted.is_some() {
                note.push_str(" The log likelihood is at a scale of 1; the adjusted log likelihood is at the estimated scale and is used in the omnibus test.");
            }
            if binary {
                note.push_str(" The deviance and Pearson chi-square are computed over the subpopulations of cases with the same values of the predictors.");
            }
            tables.push(table(
                "Goodness of Fit",
                1,
                plain(&["", "Value", "df", "Value/df"]),
                out,
                Some(note),
            ));
        }
        // the Lagrange multiplier test of a fixed scale or k
        if print.lagrange && repeated.is_none() {
            let honored = !multinomial
                && match family {
                    Family::Normal | Family::Gamma | Family::InverseGaussian => {
                        !matches!(scale, ScaleMode::Mle)
                    }
                    Family::NegBin => fixed_k.is_some(),
                    _ => false,
                };
            match (&glm_fit, honored) {
                (Some(f), true) => {
                    let negbin = family == Family::NegBin;
                    let result_k = if negbin { f.k } else { 0.0 };
                    let value = if negbin { result_k } else { phi };
                    let label = if negbin {
                        "Ancillary parameter (k)"
                    } else {
                        "Scale parameter"
                    };
                    let tested = lagrange_test(&c, &x, &spec, &f.beta, &f.mu, phi, result_k);
                    let (columns, row) = match tested {
                        Lagrange::Statistic(t) => (
                            plain(&["", "Fixed Value", "Lagrange Multiplier", "df", "Sig."]),
                            vec![
                                text(label),
                                Cell::Number(value),
                                Cell::Number(t),
                                Cell::Number(1.0),
                                number(Some(chi_square_p(t.max(0.0), 1.0))),
                            ],
                        ),
                        Lagrange::Dispersion(alpha, se, z) => (
                            plain(&[
                                "",
                                "Alpha",
                                "Std. Error",
                                "z",
                                "Sig. (k > 0)",
                                "Sig. (k < 0)",
                                "Sig. (k not 0)",
                            ]),
                            vec![
                                text("Over- or under-dispersion"),
                                Cell::Number(alpha),
                                Cell::Number(se),
                                Cell::Number(z),
                                number(Some(1.0 - normal_cdf(z))),
                                number(Some(normal_cdf(z))),
                                number(Some(2.0 * (1.0 - normal_cdf(z.abs())))),
                            ],
                        ),
                    };
                    tables.push(table(
                        "Lagrange Multiplier Test",
                        0,
                        columns,
                        vec![row],
                        Some(format!(
                            "{caption} Tests the {} fixed on the command.",
                            if negbin && result_k == 0.0 {
                                "ancillary parameter k = 0 (equidispersion) by the regression of ((y - mu)^2 - y) / mu on mu"
                            } else if negbin {
                                "ancillary parameter k"
                            } else {
                                "scale parameter"
                            }
                        )),
                    ));
                }
                _ => warnings.push(
                    "GENLIN: PRINT LAGRANGE needs the normal, gamma or inverse Gaussian distribution with a fixed scale (a number, DEVIANCE or PEARSON) or NEGBIN with a fixed k; it is ignored".into(),
                ),
            }
        }
        // the parameters and their names
        let mut parameters: Vec<Parameter> = Vec::new();
        if multinomial {
            for (j, key) in categories_of.iter().take(j_categories - 1).enumerate() {
                parameters.push(Parameter {
                    name: format!(
                        "[{} = {}]",
                        dependent.name(),
                        key_value_text(&dependent, key)
                    ),
                    at: Some(j),
                });
            }
        }
        let offset_b = if multinomial { j_categories - 1 } else { 0 };
        for e in &design.effects {
            if e.is_intercept() && multinomial {
                continue;
            }
            for (cl, cell) in e.cells.iter().enumerate() {
                let col = e.start + cl;
                let name = if e.is_intercept() {
                    "(Intercept)".to_string()
                } else {
                    parameter_name(&d, e, cell)
                };
                parameters.push(Parameter {
                    name,
                    at: kept.iter().position(|&k| k == col).map(|p| p + offset_b),
                });
            }
        }
        let b_x: Vec<f64> = result.b[offset_b..].to_vec();
        let cov_x: Matrix = result.cov[offset_b..]
            .iter()
            .map(|r| r[offset_b..].to_vec())
            .collect();
        // PRINT HISTORY: the iterations of the final fit, then its last gradient and Hessian
        if let Some(every) = print.history {
            // the estimates of every iteration, their log likelihoods (none for the estimating
            // equations), the last gradient, minus the information of the last step, and its kind
            type Run = (
                Vec<Vec<f64>>,
                Option<Vec<Option<f64>>>,
                Vec<f64>,
                Matrix,
                &'static str,
            );
            let run: Option<Run> = match (&glm_fit, &ordinal_fit, &gee_fit) {
                (_, _, Some((g, _, _))) => Some((
                    g.history.clone(),
                    None,
                    g.score.clone(),
                    g.information.clone(),
                    "expected",
                )),
                (Some(f), _, None) => {
                    let (s_one, expected, observed) = derivatives(&c, &x, &spec, f.k, &f.mu);
                    let lls = f
                        .history
                        .iter()
                        .map(|b| {
                            evaluate(&c, &x, &spec, f.k, f.phi, b)
                                .map(|e| e.2 + constant / fixed_phi)
                        })
                        .collect();
                    let (info, kind) = if f.fisher {
                        (expected, "expected")
                    } else {
                        (observed, "observed")
                    };
                    let gradient = s_one.iter().map(|v| v / f.phi).collect();
                    let hessian = info
                        .iter()
                        .map(|r| r.iter().map(|v| v / f.phi).collect())
                        .collect();
                    Some((f.history.clone(), Some(lls), gradient, hessian, kind))
                }
                (_, Some(f), None) => {
                    let lls = f
                        .history
                        .iter()
                        .map(|b| {
                            ordinal_eval(&ordinal, &x, ord_link, b, false)
                                .map(|e| e.0 / fixed_phi + constant / fixed_phi)
                        })
                        .collect();
                    let gradient = f.gradient.iter().map(|v| v / fixed_phi).collect();
                    let hessian = f
                        .information
                        .iter()
                        .map(|r| r.iter().map(|v| v / fixed_phi).collect())
                        .collect();
                    Some((f.history.clone(), Some(lls), gradient, hessian, "observed"))
                }
                _ => None,
            };
            if let Some((history, lls, gradient, information, kind)) = run {
                let shown: Vec<(&str, usize)> = parameters
                    .iter()
                    .filter_map(|p| p.at.map(|a| (p.name.as_str(), a)))
                    .collect();
                let names = |mut columns: Vec<(String, String)>| {
                    columns.extend(
                        shown
                            .iter()
                            .map(|(name, _)| (String::new(), name.to_string())),
                    );
                    columns
                };
                let mut head = vec!["Iteration"];
                if lls.is_some() {
                    head.push("Log Likelihood");
                }
                let last = history.len() - 1;
                let rows: Vec<Vec<Cell>> = history
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| i % every as usize == 0 || *i == last)
                    .map(|(i, b)| {
                        let mut row = vec![text(i.to_string())];
                        if let Some(lls) = &lls {
                            row.push(number(lls[i]));
                        }
                        row.extend(shown.iter().map(|(_, a)| Cell::Number(b[*a])));
                        row
                    })
                    .collect();
                tables.push(table(
                    "Iteration History",
                    1,
                    names(plain(&head)),
                    rows,
                    Some(format!(
                        "{caption} Iteration 0 is the start and the last iteration is always shown. {}",
                        if gee {
                            "The estimating equations have no likelihood, so none is shown."
                        } else if multinomial {
                            "The log likelihood is the one of the Goodness of Fit table."
                        } else {
                            "The log likelihood is the one of the Goodness of Fit table, at the final scale and k, which the iterations keep fixed."
                        }
                    )),
                ));
                let gradient_rows = shown
                    .iter()
                    .map(|(name, a)| vec![text(*name), Cell::Number(gradient[*a])])
                    .collect();
                tables.push(table(
                    "Last Evaluation of the Gradient Vector",
                    1,
                    plain(&["", "Gradient"]),
                    gradient_rows,
                    Some(caption.clone()),
                ));
                let hessian_rows = shown
                    .iter()
                    .map(|(name, a)| {
                        let mut row = vec![text(*name)];
                        row.extend(
                            shown
                                .iter()
                                .map(|(_, b)| Cell::Number(-information[*a][*b])),
                        );
                        row
                    })
                    .collect();
                tables.push(table(
                    "Last Evaluation of the Hessian Matrix",
                    1,
                    names(plain(&[""])),
                    hessian_rows,
                    Some(format!(
                        "{caption} The {kind} second derivatives of the {} in the parameters, at the final scale.",
                        if gee { "generalized estimating equations" } else { "log likelihood" }
                    )),
                ));
            }
        }
        // the hypothesis rows of each effect over the kept columns
        let restrict = |row: &[f64]| -> Vec<f64> { kept.iter().map(|&j| row[j]).collect() };
        let tested: Vec<usize> = (0..design.effects.len())
            .filter(|&e| !(multinomial && design.effects[e].is_intercept()))
            .collect();
        if print.summary && !gee {
            // omnibus test
            let base: Vec<Vec<f64>> = if has_intercept {
                let at = kept.iter().position(|&j| j == 0);
                x.iter()
                    .map(|row| at.map(|a| vec![row[a]]).unwrap_or_default())
                    .collect()
            } else {
                vec![Vec::new(); n_rows]
            };
            let ll0 = refit(&base);
            let df0 = if has_intercept {
                px.saturating_sub(1)
            } else {
                px
            } as f64;
            let blank = if ll0.is_none() { NO_SMALLER_FIT } else { "" };
            let row = match ll0 {
                Some(l0) => {
                    let chi = 2.0 * (result.ll - l0) / lr_scale;
                    vec![
                        Cell::Number(chi),
                        Cell::Number(df0),
                        number((df0 > 0.0).then(|| chi_square_p(chi.max(0.0), df0))),
                    ]
                }
                None => vec![Cell::Empty, Cell::Empty, Cell::Empty],
            };
            tables.push(table(
                "Omnibus Test",
                0,
                plain(&["Likelihood Ratio Chi-Square", "df", "Sig."]),
                vec![row],
                Some(format!(
                    "{caption} Compares the fitted model against the {} model.{blank}",
                    if multinomial {
                        "thresholds-only"
                    } else if has_intercept {
                        "intercept-only"
                    } else {
                        "empty"
                    }
                )),
            ));
        }
        // the analysis types of the effect tests (and of their L matrices)
        let mut types: Vec<i32> = Vec::new();
        if crit.type1 {
            types.push(1);
        }
        if crit.type3 {
            types.push(3);
        }
        // the Type I rows come from X'WX, W the frequency times the scale weight times the trials
        let type1_of = || {
            let p = design.p;
            let mut xwx = vec![vec![0.0; p]; p];
            for (i, row) in design.x.iter().enumerate() {
                let w = c.f[i] * c.a(i);
                for (a, out) in xwx.iter_mut().enumerate() {
                    if row[a] != 0.0 {
                        for (v, xb) in out.iter_mut().zip(row) {
                            *v += w * row[a] * xb;
                        }
                    }
                }
            }
            type1_rows(&xwx, crit.singular)
        };
        // the rows of L over every parameter that test effect `e` by the Type I or III rule,
        // keeping those that touch an estimated parameter
        let l_rows = |t: i32, e: usize, type1: &Option<Vec<Option<Vec<f64>>>>| -> Matrix {
            let all: Matrix = match type1 {
                Some(rows) => design.effects[e]
                    .range()
                    .filter_map(|c| rows[c].clone())
                    .collect(),
                None => design.type34(e, false),
            };
            let _ = t;
            all.into_iter()
                .filter(|r| restrict(r).iter().any(|v| v.abs() > 1e-12))
                .collect()
        };
        if print.summary {
            let mut columns: Vec<(String, String)> = plain(&["Source"]);
            let mut values: Vec<Vec<Cell>> = tested
                .iter()
                .map(|&e| {
                    vec![text(if design.effects[e].is_intercept() {
                        "(Intercept)".to_string()
                    } else {
                        design.effects[e].label.clone()
                    })]
                })
                .collect();
            let f_too = crit.lr && matches!(scale, ScaleMode::Deviance | ScaleMode::Pearson);
            let mut blank_lr = false;
            for &t in &types {
                let group = format!("Type {}", if t == 1 { "I" } else { "III" });
                let first = if crit.lr && repeated.is_some() {
                    "Generalized Score Chi-Square"
                } else if crit.lr {
                    "Likelihood Ratio Chi-Square"
                } else {
                    "Wald Chi-Square"
                };
                for name in [first, "df", "Sig."] {
                    columns.push((group.clone(), name.into()));
                }
                if f_too {
                    for name in ["F", "df1", "df2", "Sig. (F)"] {
                        columns.push((group.clone(), name.into()));
                    }
                }
                let type1 = (t == 1).then(type1_of);
                // the log likelihood of the model of the first effects, by their number
                let mut prefix_ll: std::collections::HashMap<usize, Option<f64>> =
                    std::collections::HashMap::new();
                for (slot, &e) in tested.iter().enumerate() {
                    let effect = &design.effects[e];
                    let l: Matrix = l_rows(t, e, &type1).iter().map(|r| restrict(r)).collect();
                    let (chi, df) = if crit.lr
                        && let (Some(r), Some(sub), Some((g, _, _))) =
                            (repeated, &subjects, &gee_fit)
                    {
                        // REPEATED: the generalized score statistic of L b = 0
                        let basis = null_space(&l, px);
                        let df = px - basis.first().map_or(0, Vec::len);
                        let set = gee::Settings::of(
                            r,
                            (family, command.link, result.k),
                            scale == ScaleMode::Mle,
                            crit.singular,
                        );
                        let chi =
                            gee::score_statistic(&c, &x, sub, &set, &l, &basis, &g.beta, g.phi)
                                .map_err(Error)?;
                        (chi, df)
                    } else if crit.lr {
                        let (pair, df) = if t == 1 {
                            // the model of the effects up to this one, with and without it
                            let columns_up_to = |last: usize| -> Vec<Vec<f64>> {
                                let cols: Vec<usize> = (0..px)
                                    .filter(|&p| {
                                        design.effects[..last]
                                            .iter()
                                            .any(|f| f.range().contains(&kept[p]))
                                    })
                                    .collect();
                                x.iter()
                                    .map(|row| cols.iter().map(|&p| row[p]).collect())
                                    .collect()
                            };
                            let df = effect.range().filter(|c| kept.contains(c)).count();
                            let mut nested = |last: usize| -> Option<f64> {
                                if last == design.effects.len() {
                                    return Some(result.ll);
                                }
                                *prefix_ll
                                    .entry(last)
                                    .or_insert_with(|| refit(&columns_up_to(last)))
                            };
                            (nested(e + 1).zip(nested(e)), df)
                        } else {
                            // the model constrained to the Type III hypothesis
                            let basis = null_space(&l, px);
                            let df = px - basis.first().map_or(0, Vec::len);
                            (refit(&times(&x, &basis)).map(|l0| (result.ll, l0)), df)
                        };
                        (pair.map(|(lf, ls)| 2.0 * (lf - ls) / lr_scale), df)
                    } else {
                        let (chi, df) = wald(&l, &b_x, &cov_x);
                        (Some(chi), df)
                    };
                    let df = df as f64;
                    let ok = df > 0.0 && chi.is_some_and(f64::is_finite);
                    blank_lr |= crit.lr && df > 0.0 && chi.is_none();
                    values[slot].push(number(chi.filter(|_| ok)));
                    values[slot].push(Cell::Number(df));
                    values[slot].push(number(
                        chi.filter(|_| ok).map(|v| chi_square_p(v.max(0.0), df)),
                    ));
                    if f_too {
                        let fv = chi.filter(|_| ok).map(|v| v / df);
                        values[slot].push(number(fv));
                        values[slot].push(Cell::Number(df));
                        values[slot].push(Cell::Number(df_fit));
                        values[slot].push(number(fv.map(|v| f_p(v.max(0.0), df, df_fit))));
                    }
                }
            }
            tables.push(table(
                "Tests of Model Effects",
                1,
                columns,
                values,
                Some(format!(
                    "{caption}{}",
                    if blank_lr { NO_SMALLER_FIT } else { "" }
                )),
            ));
        }
        if print.lmatrix || print.gef {
            if multinomial {
                warnings.push(
                    "GENLIN: PRINT LMATRIX and GEF are not supported yet for the ordinal multinomial model; they are ignored".into(),
                );
            } else {
                if print.lmatrix {
                    for &t in &types {
                        let roman = if t == 1 { "I" } else { "III" };
                        let type1 = (t == 1).then(type1_of);
                        for &e in &tested {
                            let rows = l_rows(t, e, &type1);
                            if rows.is_empty() {
                                continue;
                            }
                            let label = if design.effects[e].is_intercept() {
                                "(Intercept)".to_string()
                            } else {
                                design.effects[e].label.clone()
                            };
                            tables.push(l_prime_table(
                                &d,
                                &design,
                                &format!("Contrast Coefficients (L' Matrix): Type {roman} {label}"),
                                format!("{caption} Each column is a row of L, the hypothesis the Type {roman} test of {label} tests."),
                                (1..=rows.len()).map(|i| format!("L{i}")).collect(),
                                &rows,
                            ));
                        }
                    }
                }
                if print.gef {
                    let basis = gef_basis(&design.h(), &design.redundant);
                    tables.push(l_prime_table(
                        &d,
                        &design,
                        "General Estimable Function",
                        format!("{caption} Each column is a free coefficient; a parameter's coefficient in any estimable function is the combination of them in its row."),
                        (1..=basis.len()).map(|i| format!("L{i}")).collect(),
                        &basis,
                    ));
                }
            }
        }
        // parameter estimates
        let z = normal_quantile(0.5 + crit.cilevel / 200.0);
        let level = percent_title(1.0 - crit.cilevel / 100.0);
        let mut profiles: Vec<Option<(Option<f64>, Option<f64>)>> = vec![None; parameters.len()];
        let mut profiled = false;
        // the profile intervals of the scale and of an estimated NEGBIN k
        let mut scale_limits: Option<(Option<f64>, Option<f64>)> = None;
        let mut k_limits: Option<(Option<f64>, Option<f64>)> = None;
        if print.solution
            && let (Some(tolerance), false, None) = (crit.profile, multinomial, repeated)
        {
            let full_scale = match scale {
                ScaleMode::Deviance | ScaleMode::Pearson => ScaleMode::Fixed(result.phi),
                other => other,
            };
            let profile_spec = Spec {
                scale: full_scale,
                ..spec
            };
            if let Some(full) = fit_glm(&c, &x, &profile_spec, None) {
                let setup = Profile {
                    c: &c,
                    x: &x,
                    spec: &profile_spec,
                    ll_max: full.ll,
                    z2: z * z,
                    tolerance,
                };
                for (pi, par) in parameters.iter().enumerate() {
                    if let Some(i) = par.at {
                        let se = result.cov[i][i].sqrt();
                        profiles[pi] = Some(profile_interval(&setup, i, &result.b, se));
                    }
                }
                // ln of the scale (or of k) is the parameter: hold a value, refit the coefficients
                let z2 = z * z;
                let tolerance_of_limits = tolerance;
                let ancillary_limits = |centre: f64, variance: f64, hold_k: bool| {
                    let mut start = result.b.clone();
                    let mut drop = |t: f64| -> Option<f64> {
                        let held = if hold_k {
                            Spec {
                                k: Some(t.exp()),
                                ..profile_spec
                            }
                        } else {
                            Spec {
                                scale: ScaleMode::Fixed(t.exp()),
                                ..profile_spec
                            }
                        };
                        let fit = fit_glm(&c, &x, &held, Some(&start))?;
                        if !fit.converged {
                            return None;
                        }
                        start = fit.beta.clone();
                        Some(2.0 * (full.ll - fit.ll) - z2)
                    };
                    let (lower, upper) = profile_limits(
                        centre.ln(),
                        variance.sqrt(),
                        tolerance_of_limits,
                        &mut drop,
                    );
                    (lower.map(f64::exp), upper.map(f64::exp))
                };
                let positive = result.log_variance.filter(|v| *v > 0.0);
                if let (true, Some(lv)) = (
                    scale == ScaleMode::Mle && family != Family::NegBin && !multinomial,
                    positive,
                ) {
                    scale_limits = Some(ancillary_limits(phi, lv, false));
                }
                if let (true, Some(lv)) = (family == Family::NegBin && k_estimated, positive) {
                    k_limits = Some(ancillary_limits(result.k, lv, true));
                }
                profiled = true;
            }
        }
        if print.solution && crit.profile.is_some() && !profiled {
            warnings.push(
                "CITYPE=PROFILE is for the models other than the ordinal multinomial one and without REPEATED, and needs the model to refit; the intervals are Wald intervals".into(),
            );
        }
        let kind = if profiled {
            "Profile Likelihood"
        } else {
            "Wald"
        };
        if print.solution {
            let mut columns = plain(&["Parameter", "B", "Std. Error"]);
            let ci = format!("{level} {kind} Confidence Interval");
            columns.push((ci.clone(), "Lower".into()));
            columns.push((ci, "Upper".into()));
            for name in ["Wald Chi-Square", "df", "Sig."] {
                columns.push(("Hypothesis Test".into(), name.into()));
            }
            if print.exponentiated {
                columns.push((String::new(), "Exp(B)".into()));
                let ci = format!("{level} {kind} Confidence Interval for Exp(B)");
                columns.push((ci.clone(), "Lower".into()));
                columns.push((ci, "Upper".into()));
            }
            let width = columns.len();
            let mut out = Vec::new();
            let mut any_redundant = false;
            for (pi, par) in parameters.iter().enumerate() {
                let head = if multinomial && pi < j_categories - 1 {
                    text(format!("Threshold {}", par.name))
                } else {
                    text(par.name.clone())
                };
                let mut row = vec![head];
                match par.at {
                    Some(i) => {
                        let b = result.b[i];
                        let v = result.cov[i][i];
                        let ok = v > 0.0;
                        let se = v.sqrt();
                        let wald = (b / se).powi(2);
                        let (lower, upper) = match profiles[pi] {
                            Some(limits) => limits,
                            None => (ok.then_some(b - z * se), ok.then_some(b + z * se)),
                        };
                        row.extend([
                            Cell::Number(b),
                            number(ok.then_some(se)),
                            number(lower),
                            number(upper),
                            number(ok.then_some(wald)),
                            Cell::Number(1.0),
                            number(ok.then(|| chi_square_p(wald, 1.0))),
                        ]);
                        if print.exponentiated {
                            row.extend([
                                Cell::Number(b.exp()),
                                number(lower.map(f64::exp)),
                                number(upper.map(f64::exp)),
                            ]);
                        }
                    }
                    None => {
                        any_redundant = true;
                        row.push(Cell::Number(0.0));
                        row.resize(width, Cell::Empty);
                        if print.exponentiated {
                            row[width - 3] = Cell::Number(1.0);
                        }
                    }
                }
                out.push(row);
            }
            let mut notes = vec![caption.clone()];
            if any_redundant {
                notes.push("A parameter of 0 without a standard error is set to zero because it is redundant.".into());
            }
            let ancillary =
                |name: &str,
                 value: f64,
                 log_variance: Option<f64>,
                 limits: Option<(Option<f64>, Option<f64>)>| {
                    let mut row = vec![text(name), Cell::Number(value)];
                    match log_variance.filter(|v| *v > 0.0) {
                        Some(lv) => {
                            let s = lv.sqrt();
                            let (lower, upper) = limits.unwrap_or((
                                Some(value * (-z * s).exp()),
                                Some(value * (z * s).exp()),
                            ));
                            for v in [Some(value * s), lower, upper].map(|v| v.unwrap_or(f64::NAN))
                            {
                                // an estimate at the boundary of its range can have no finite bound
                                row.push(if v.is_finite() {
                                    Cell::Number(v)
                                } else {
                                    Cell::Empty
                                });
                            }
                        }
                        None => row.extend([Cell::Empty, Cell::Empty, Cell::Empty]),
                    }
                    row.resize(width, Cell::Empty);
                    row
                };
            out.push(ancillary(
                "(Scale)",
                phi,
                if scale == ScaleMode::Mle {
                    result.log_variance
                } else {
                    None
                },
                scale_limits,
            ));
            notes.push(match scale {
                ScaleMode::Mle if gee => {
                    "(Scale) is computed from the Pearson residuals of the estimating equations; it stays at the plain model's estimate when no iteration ran."
                        .into()
                }
                ScaleMode::Deviance | ScaleMode::Pearson if gee => format!(
                    "(Scale) is computed from the {} of the generalized linear model that gives the start values, and held there.",
                    if scale == ScaleMode::Deviance {
                        "deviance"
                    } else {
                        "Pearson chi-square"
                    }
                ),
                ScaleMode::Mle => "(Scale) is a maximum likelihood estimate.".into(),
                ScaleMode::Deviance => "(Scale) is computed from the deviance.".into(),
                ScaleMode::Pearson => "(Scale) is computed from the Pearson chi-square.".into(),
                ScaleMode::Fixed(_) => "(Scale) is fixed at the displayed value.".into(),
            });
            if family == Family::NegBin && !multinomial {
                out.push(ancillary(
                    "(Negative binomial)",
                    result.k,
                    if k_estimated {
                        result.log_variance
                    } else {
                        None
                    },
                    k_limits,
                ));
                notes.push(if k_estimated && gee {
                    "(Negative binomial) is the maximum likelihood estimate of the generalized linear model that gives the start values, held there.".into()
                } else if k_estimated {
                    "(Negative binomial) is a maximum likelihood estimate.".into()
                } else {
                    "(Negative binomial) is fixed at the displayed value.".into()
                });
            }
            match repeated {
                Some(r) if !r.robust => {
                    notes.push("The standard errors are model-based.".into());
                }
                Some(_) => {
                    notes.push("The standard errors are robust (sandwich) estimates.".into());
                }
                None if crit.robust => {
                    notes.push("The standard errors are robust (sandwich) estimates.".into());
                }
                None => {}
            }
            tables.push(table(
                "Parameter Estimates",
                1,
                columns,
                out,
                Some(notes.join(" ")),
            ));
        }
        if print.covb || print.corb {
            let names: Vec<String> = parameters
                .iter()
                .enumerate()
                .filter(|(_, p)| p.at.is_some())
                .map(|(pi, p)| {
                    if multinomial && pi < j_categories - 1 {
                        format!("Threshold {}", p.name)
                    } else {
                        p.name.clone()
                    }
                })
                .collect();
            let size = names.len();
            let matrix = |title: &str, value: &dyn Fn(usize, usize) -> Option<f64>| {
                let mut columns = plain(&[""]);
                columns.extend(names.iter().map(|n| (String::new(), n.clone())));
                let rows = (0..size)
                    .map(|i| {
                        let mut row = vec![text(names[i].clone())];
                        row.extend((0..size).map(|j| number(value(i, j))));
                        row
                    })
                    .collect();
                table(title, 1, columns, rows, Some(caption.clone()))
            };
            if print.covb {
                tables.push(matrix("Covariances of Parameter Estimates", &|i, j| {
                    Some(result.cov[i][j])
                }));
            }
            if print.corb {
                tables.push(matrix("Correlations of Parameter Estimates", &|i, j| {
                    let v = result.cov[i][i] * result.cov[j][j];
                    (v > 0.0).then(|| result.cov[i][j] / v.sqrt())
                }));
            }
        }
        if let (true, Some((g, _, _)), Some(r), Some(s)) =
            (print.workingcorr, &gee_fit, repeated, &subjects)
        {
            let names: Vec<String> = (0..s.dim)
                .map(|u| match s.positions.get(u) {
                    Some(keys) => format!(
                        "[{}]",
                        within_vars
                            .iter()
                            .zip(keys)
                            .map(|(v, k)| format!("{}={}", shown.variable(v), key_value_text(v, k)))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    None => format!("Measurement {}", u + 1),
                })
                .collect();
            let mut columns = plain(&[""]);
            columns.extend(names.iter().map(|n| (String::new(), n.clone())));
            let out = (0..s.dim)
                .map(|u| {
                    let mut row = vec![text(names[u].clone())];
                    row.extend(g.r[u].iter().map(|v| Cell::Number(*v)));
                    row
                })
                .collect();
            tables.push(table(
                "Working Correlation Matrix",
                1,
                columns,
                out,
                Some(format!(
                    "{caption} Working correlation structure: {}.",
                    correlation_name(&r.correlation, s.dim)
                )),
            ));
        }
        // estimated marginal means
        if !command.emmeans.is_empty() {
            if multinomial {
                warnings.push("EMMEANS is ignored: the ordinal multinomial model has no estimated marginal means here".into());
            } else {
                let h = design.h();
                for e in &command.emmeans {
                    tables.extend(emmeans_tables(
                        EmmContext {
                            e,
                            d: &d,
                            design: &design,
                            h: &h,
                            kept: &kept,
                            b: &b_x,
                            cov: &cov_x,
                            link: command.link,
                            k: result.k,
                            level: &level,
                            alpha: 1.0 - crit.cilevel / 100.0,
                            caption: &caption,
                            factor_vars: &factor_vars,
                            levels: &levels,
                            shown: &shown,
                            lmatrix: print.lmatrix,
                        },
                        &mut warnings,
                    )?);
                }
            }
        }
        // SAVE
        let mut saved: Vec<Saved> = Vec::new();
        if !command.save.is_empty() {
            // the design row of every case with valid predictors and levels that were fitted
            let rows_of: Vec<Option<Vec<f64>>> = (0..cases)
                .map(|kc| {
                    if !predictors_valid[kc] {
                        return None;
                    }
                    let mut lv = Vec::new();
                    for (col, ls) in factor_keys.iter().zip(&levels) {
                        lv.push(level_of(ls, col[kc].as_ref()?)?);
                    }
                    let cv: Vec<f64> = cov_vars
                        .iter()
                        .map(|v| numeric(v, kc))
                        .collect::<Option<_>>()?;
                    let mut full = vec![0.0; design.p];
                    for e in &design.effects {
                        let mine: Vec<usize> = e.factors.iter().map(|&f| lv[f]).collect();
                        let at = e.cells.iter().position(|c| *c == mine)?;
                        full[e.start + at] = e.covariates.iter().map(|&c| cv[c]).product();
                    }
                    Some(restrict(&full))
                })
                .collect();
            let fitted_row: Vec<Option<usize>> = {
                let mut v = vec![None; cases];
                for (i, &k) in rows.iter().enumerate() {
                    v[k] = Some(i);
                }
                v
            };
            let trials_of: Vec<Option<f64>> = (0..cases)
                .map(|kc| match &command.response {
                    GenlinResponse::Events { trials, .. } => {
                        let m = match trials {
                            GenlinTrials::Number(n) => Some(*n),
                            GenlinTrials::Variable(_) => numeric(trials_var.as_ref()?, kc),
                        }?;
                        (m >= 1.0 && m.fract() == 0.0).then_some(m)
                    }
                    GenlinResponse::Dependent { .. } => Some(1.0),
                })
                .collect();
            let save_ctx = SaveContext {
                trials: &trials_of,
                command,
                multinomial,
                binary,
                events,
                family,
                result: &result,
                glm: glm_fit.as_ref(),
                cases: &c,
                x: &x,
                rows_of: &rows_of,
                fitted_row: &fitted_row,
                offset_of: &offset_of,
                z,
                categories: &categories_of,
                dependent: &dependent,
                j: j_categories,
                px,
                ordinal: &ordinal,
                gee,
            };
            saved = save_columns(&save_ctx, &mut warnings);
        }
        for w in warnings {
            self.warn(w);
        }
        if !saved.is_empty() {
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                for s in &saved {
                    let mut name = s.name.clone();
                    if data.variable_index(&name).is_some() {
                        if s.given {
                            return Err(Error(format!(
                                "GENLIN /SAVE: variable {name} already exists"
                            )));
                        }
                        let mut n = 1;
                        while data.variable_index(&format!("{name}_{n}")).is_some() {
                            n += 1;
                        }
                        name = format!("{name}_{n}");
                    }
                    match &s.values {
                        SavedValues::Numbers(values, format) => {
                            data.set_numeric_values(&name, values.clone())?;
                            data.set_format(&name, format)?;
                            data.set_measure(&name, Measure::Scale)?;
                        }
                        SavedValues::Texts(values) => {
                            let width = values.iter().map(String::len).max().unwrap_or(1).max(1);
                            data.set_string_values(&name, width, values.clone())?;
                            data.set_measure(&name, Measure::Nominal)?;
                        }
                    }
                    data.set_variable_label(&name, Some(&s.label))?;
                }
                Ok(())
            })?;
        }
        Ok(tables)
    }
}

/// The name of a structure; m of MDEPENDENT as used, at most the dimension `dim` less 1.
fn correlation_name(c: &GenlinCorrelation, dim: usize) -> String {
    match c {
        GenlinCorrelation::Independent => "Independent".into(),
        GenlinCorrelation::Ar1 => "AR(1)".into(),
        GenlinCorrelation::Exchangeable => "Exchangeable".into(),
        GenlinCorrelation::MDependent(m) => {
            format!("M-dependent ({})", (*m as usize).min(dim.saturating_sub(1)))
        }
        GenlinCorrelation::Unstructured => "Unstructured".into(),
        GenlinCorrelation::Fixed(_) => "Fixed".into(),
    }
}

const NO_SMALLER_FIT: &str = " A blank likelihood ratio test has a smaller model that gives no valid means, such as one without parameters under an inverse or power link.";

const HESSIAN_WARNING: &str = "The Hessian convergence criterion (1E-4) is not met after the estimates converged; the results may not be final";

fn not_converged(crit: &GenlinCriteria, halving_failed: bool) -> String {
    if halving_failed {
        "The log likelihood could not be increased by step-halving (MAXSTEPHALVING) before the convergence criteria were met; the results may not be final".into()
    } else {
        format!(
            "The estimates did not converge within {} iterations (MAXITERATIONS); the results may not be final",
            crit.maxiterations
        )
    }
}

/// The category probabilities from the J-1 cumulative ones.
fn cell_probabilities(gamma: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(gamma.len() + 1);
    let mut lower = 0.0;
    for &g in gamma.iter().chain(std::iter::once(&1.0)) {
        out.push(g - lower);
        lower = g;
    }
    out
}

/// The score of case i (at scale 1) in the ordinal model, without its frequency.
fn ordinal_case_score(
    o: &Ordinal,
    x: &[Vec<f64>],
    link: GenlinLink,
    b: &[f64],
    i: usize,
) -> Vec<f64> {
    let one = Ordinal {
        category: vec![o.category[i]],
        f: vec![1.0],
        omega: vec![o.omega[i]],
        offset: vec![o.offset[i]],
        j: o.j,
    };
    ordinal_eval(&one, std::slice::from_ref(&x[i]), link, b, false)
        .map(|(_, s, _)| s)
        .unwrap_or_else(|| vec![0.0; b.len()])
}

// ---------------------------------------------------------------------------------------------
// estimated marginal means

struct EmmContext<'a> {
    e: &'a GenlinEmmeans,
    d: &'a Data,
    design: &'a Fit,
    h: &'a Matrix,
    kept: &'a [usize],
    b: &'a [f64],
    cov: &'a Matrix,
    link: GenlinLink,
    k: f64,
    level: &'a str,
    alpha: f64,
    caption: &'a str,
    factor_vars: &'a [Variable<'a>],
    levels: &'a [Vec<Key>],
    shown: &'a Shown,
    /// PRINT LMATRIX: also the coefficients of each mean.
    lmatrix: bool,
}

fn emmeans_tables(cx: EmmContext, warnings: &mut Vec<String>) -> Result<Vec<Table>, Error> {
    let e = cx.e;
    let d = cx.d;
    let find = |name: &str| -> Result<usize, Error> {
        d.factors
            .iter()
            .position(|v| v.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                Error(format!(
                    "GENLIN EMMEANS names {name}, which is not a factor"
                ))
            })
    };
    let factors: Vec<usize> = e.tables.iter().map(|n| find(n)).collect::<Result<_, _>>()?;
    let mut with = Vec::new();
    for (name, value) in &e.control {
        let Some(ci) = d
            .covariates
            .iter()
            .position(|n| n.eq_ignore_ascii_case(name))
        else {
            return Err(Error(format!(
                "GENLIN EMMEANS CONTROL names {name}, which is not a covariate"
            )));
        };
        if let Some(v) = value {
            with.push((ci, *v));
        }
    }
    let label_of = |f: usize, lv: usize| cx.shown.value(&cx.factor_vars[f], &cx.levels[f][lv]);
    let combos = combinations(
        &factors
            .iter()
            .map(|&f| d.values[f].len())
            .collect::<Vec<_>>(),
    );
    // the linear predictor of each cell and its covariance
    let full: Vec<Vec<f64>> = combos
        .iter()
        .map(|combo| {
            let fixed: Vec<(usize, usize)> =
                factors.iter().copied().zip(combo.iter().copied()).collect();
            emmeans_l(d, cx.design, &fixed, &with)
        })
        .collect();
    let ls: Vec<Option<Vec<f64>>> = full
        .iter()
        .map(|l| {
            cx.design
                .estimable(l, cx.h)
                .then(|| cx.kept.iter().map(|&j| l[j]).collect())
        })
        .collect();
    let r = combos.len();
    let mut est = vec![f64::NAN; r];
    let mut v = vec![vec![0.0; r]; r];
    for i in 0..r {
        let Some(li) = &ls[i] else { continue };
        let lv = mat_vec(cx.cov, li);
        est[i] = dot(li, cx.b);
        for j in 0..r {
            if let Some(lj) = &ls[j] {
                v[i][j] = dot(lj, &lv);
            }
        }
    }
    if !e.transformed {
        let slope: Vec<f64> = est
            .iter()
            .map(|&eta| {
                let mu = link_inverse(cx.link, eta, cx.k);
                1.0 / link_d1(cx.link, mu, cx.k)
            })
            .collect();
        for i in 0..r {
            for j in 0..r {
                v[i][j] *= slope[i] * slope[j];
            }
        }
        for x in &mut est {
            *x = link_inverse(cx.link, *x, cx.k);
        }
    }
    let z = normal_quantile(1.0 - cx.alpha / 2.0);
    let scale_note = if e.transformed {
        "The means are on the scale of the linear predictor."
    } else {
        "The means are on the original scale of the response."
    };
    let covariate_note = if d.covariates.is_empty() {
        String::new()
    } else {
        let values: Vec<String> = d
            .covariates
            .iter()
            .enumerate()
            .map(|(ci, n)| {
                let v = with
                    .iter()
                    .find(|(w, _)| *w == ci)
                    .map_or_else(|| d.covariate_mean(&[ci]), |(_, v)| *v);
                format!("{n} = {}", (v * 1e4).round() / 1e4)
            })
            .collect();
        format!(" Covariates are held at: {}.", values.join(", "))
    };
    let caption = format!("{} {scale_note}{covariate_note}", cx.caption);
    let ci_group = format!("{} Wald Confidence Interval", cx.level);
    let name = if factors.is_empty() {
        "Overall".to_string()
    } else {
        factors
            .iter()
            .map(|&f| d.factor_texts[f].clone())
            .collect::<Vec<_>>()
            .join(" * ")
    };
    let mut columns: Vec<(String, String)> = factors
        .iter()
        .map(|&f| (String::new(), d.factor_texts[f].clone()))
        .collect();
    columns.extend(plain(&["Mean", "Std. Error"]));
    columns.push((ci_group.clone(), "Lower".into()));
    columns.push((ci_group.clone(), "Upper".into()));
    let mut rows = Vec::new();
    for (i, combo) in combos.iter().enumerate() {
        let mut row: Vec<Cell> = factors
            .iter()
            .zip(combo)
            .enumerate()
            .map(|(at, (&f, &lv))| {
                if i == 0 || combo[at + 1..].iter().all(|&x| x == 0) {
                    text(label_of(f, lv))
                } else {
                    Cell::Empty
                }
            })
            .collect();
        if ls[i].is_some() {
            let se = v[i][i].max(0.0).sqrt();
            row.extend([
                Cell::Number(est[i]),
                Cell::Number(se),
                Cell::Number(est[i] - z * se),
                Cell::Number(est[i] + z * se),
            ]);
        } else {
            row.extend([Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]);
        }
        rows.push(row);
    }
    let mut out = vec![table(
        &format!("Estimated Marginal Means: {name}"),
        factors.len(),
        columns,
        rows,
        Some(if ls.iter().any(Option::is_none) {
            format!("{caption} A mean without a value is not estimable.")
        } else {
            caption.clone()
        }),
    )];
    if cx.lmatrix {
        let cells: Vec<String> = combos
            .iter()
            .map(|combo| {
                if factors.is_empty() {
                    return "Overall".to_string();
                }
                factors
                    .iter()
                    .zip(combo)
                    .map(|(&f, &lv)| format!("{}={}", d.factor_texts[f], label_of(f, lv)))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .collect();
        out.push(l_prime_table(
            d,
            cx.design,
            &format!("Contrast Coefficients (L' Matrix): Estimated Marginal Means {name}"),
            format!(
                "{} Each column is the row of L of one mean: the cell's parameters, the other factors averaged and the covariates held as in the means.",
                cx.caption
            ),
            cells,
            &full,
        ));
    }
    let Some(compare) = &e.compare else {
        return Ok(out);
    };
    let compared: Vec<usize> = compare
        .iter()
        .map(|n| {
            let f = find(n)?;
            factors.iter().position(|&g| g == f).ok_or_else(|| {
                Error(format!(
                    "GENLIN EMMEANS COMPARE names {n}, which is not on TABLES"
                ))
            })
        })
        .collect::<Result<_, _>>()?;
    let others: Vec<usize> = (0..factors.len())
        .filter(|a| !compared.contains(a))
        .collect();
    let other_combos = combinations(
        &others
            .iter()
            .map(|&a| d.values[factors[a]].len())
            .collect::<Vec<_>>(),
    );
    let compared_combos = combinations(
        &compared
            .iter()
            .map(|&a| d.values[factors[a]].len())
            .collect::<Vec<_>>(),
    );
    let kc = compared_combos.len();
    let index_of = |other: &[usize], mine: &[usize]| -> usize {
        let mut combo = vec![0; factors.len()];
        for (o, &a) in other.iter().zip(&others) {
            combo[a] = *o;
        }
        for (m, &a) in mine.iter().zip(&compared) {
            combo[a] = *m;
        }
        combos
            .iter()
            .position(|c| *c == combo)
            .expect("a combination")
    };
    let compared_label = |mine: &[usize]| -> String {
        mine.iter()
            .zip(&compared)
            .map(|(&lv, &a)| label_of(factors[a], lv))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let compared_name = compared
        .iter()
        .map(|&a| d.factor_texts[factors[a]].clone())
        .collect::<Vec<_>>()
        .join(" * ");
    // the contrast rows over the compared cells
    let pairwise = matches!(e.contrast, None | Some(GenlinContrast::Pairwise));
    let contrast_list: Vec<(String, Vec<f64>)> = if pairwise {
        Vec::new()
    } else {
        let f = factors[compared[0]];
        let k = d.values[f].len();
        let mut reference = |value: &Option<Value>| -> Option<u32> {
            let v = value.as_ref()?;
            match cx.levels[f]
                .iter()
                .position(|key| value_matches(&cx.factor_vars[f], key, v))
            {
                Some(at) => Some(at as u32 + 1),
                None => {
                    warnings.push(format!(
                        "The contrast reference value is not a level of {}; the last level is used",
                        d.factor_texts[f]
                    ));
                    None
                }
            }
        };
        let kind = match e.contrast.as_ref().expect("not pairwise") {
            GenlinContrast::Deviation(v) => Contrast::Deviation(reference(v)),
            GenlinContrast::Simple(v) => Contrast::Simple(reference(v)),
            GenlinContrast::Difference => Contrast::Difference,
            GenlinContrast::Helmert => Contrast::Helmert,
            GenlinContrast::Repeated => Contrast::Repeated,
            GenlinContrast::Polynomial(metric) => {
                let metric = match metric {
                    Some(m) if m.len() < k => {
                        warnings.push(format!(
                            "The POLYNOMIAL metric has {} values for the {k} levels of {}; the comparisons are left out",
                            m.len(),
                            d.factor_texts[f]
                        ));
                        return Ok(out);
                    }
                    Some(m) => {
                        if m.len() > k {
                            warnings.push(format!(
                                "The POLYNOMIAL metric has more values than {} has levels; the first {k} are used",
                                d.factor_texts[f]
                            ));
                        }
                        Some(m[..k].to_vec())
                    }
                    None => None,
                };
                Contrast::Polynomial(metric)
            }
            GenlinContrast::Pairwise => unreachable!("handled"),
        };
        let mut rows = crate::contrast::contrast_rows(&kind, k)
            .map_err(|m| Error(format!("GENLIN EMMEANS: {m}")))?;
        for (label, _) in &mut rows {
            for l in (0..k).rev() {
                *label = label.replace(&format!("Level {}", l + 1), &label_of(f, l));
            }
        }
        rows
    };
    let mut detail_rows = Vec::new();
    let mut overall_rows = Vec::new();
    for other in &other_combos {
        let cells: Vec<usize> = compared_combos
            .iter()
            .map(|mine| index_of(other, mine))
            .collect();
        let estimable = |c: &[f64]| {
            c.iter()
                .zip(&cells)
                .all(|(w, &i)| *w == 0.0 || ls[i].is_some())
        };
        let estimate = |c: &[f64]| -> (f64, f64) {
            let value: f64 = c
                .iter()
                .zip(&cells)
                .filter(|(w, _)| **w != 0.0)
                .map(|(w, &i)| w * est[i])
                .sum();
            let mut var = 0.0;
            for (a, &i) in cells.iter().enumerate() {
                for (b2, &j) in cells.iter().enumerate() {
                    if c[a] != 0.0 && c[b2] != 0.0 {
                        var += c[a] * c[b2] * v[i][j];
                    }
                }
            }
            (value, var.max(0.0).sqrt())
        };
        // the hypotheses of the family
        let family: Vec<(String, String, Vec<f64>)> = if pairwise {
            let mut list = Vec::new();
            for a in 0..kc {
                for b2 in (0..kc).filter(|&b2| b2 != a) {
                    let mut c = vec![0.0; kc];
                    c[a] = 1.0;
                    c[b2] = -1.0;
                    list.push((
                        compared_label(&compared_combos[a]),
                        compared_label(&compared_combos[b2]),
                        c,
                    ));
                }
            }
            list
        } else {
            contrast_list
                .iter()
                .map(|(l, c)| (l.clone(), String::new(), c.clone()))
                .collect()
        };
        // significance before adjustment, one per distinct hypothesis (a pair counts once)
        let tests: Vec<Option<(f64, f64, f64)>> = family
            .iter()
            .map(|(_, _, c)| {
                estimable(c).then(|| {
                    let (value, se) = estimate(c);
                    let chi = (value / se).powi(2);
                    (value, se, chi_square_p(chi, 1.0))
                })
            })
            .collect();
        let distinct: Vec<usize> = (0..family.len())
            .filter(|&i| {
                !pairwise || {
                    let c = &family[i].2;
                    let first = c.iter().position(|&w| w != 0.0).expect("a pair");
                    c[first] > 0.0
                }
            })
            .filter(|&i| tests[i].is_some())
            .collect();
        let pvalues: Vec<f64> = distinct
            .iter()
            .map(|&i| tests[i].expect("estimable").2)
            .collect();
        let (adjusted, critical) = adjust(&pvalues, cx.alpha, e.padjust);
        let adjusted_of = |i: usize| -> Option<(f64, f64)> {
            let c = &family[i].2;
            distinct
                .iter()
                .position(|&j| {
                    j == i || (pairwise && family[j].2.iter().zip(c).all(|(a, b2)| *a == -b2))
                })
                .map(|at| (adjusted[at], critical[at]))
        };
        let other_cells = |row: &mut Vec<Cell>, first: bool| {
            for (o, &a) in other.iter().zip(&others) {
                row.push(if first {
                    text(label_of(factors[a], *o))
                } else {
                    Cell::Empty
                });
            }
        };
        let mut last_i = String::new();
        for (fi, (li, lj, _)) in family.iter().enumerate() {
            let mut row = Vec::new();
            other_cells(&mut row, fi == 0);
            if pairwise {
                row.push(if *li != last_i {
                    text(li.clone())
                } else {
                    Cell::Empty
                });
                last_i = li.clone();
                row.push(text(lj.clone()));
            } else {
                row.push(text(li.clone()));
            }
            match (tests[fi], adjusted_of(fi)) {
                (Some((value, se, _)), Some((p, zc))) => {
                    if pairwise {
                        row.extend([
                            Cell::Number(value),
                            Cell::Number(se),
                            Cell::Number(1.0),
                            Cell::Number(p),
                        ]);
                    } else {
                        row.extend([
                            Cell::Number(value),
                            Cell::Number(0.0),
                            Cell::Number(se),
                            Cell::Number((value / se).powi(2)),
                            Cell::Number(1.0),
                            Cell::Number(p),
                        ]);
                    }
                    row.extend([Cell::Number(value - zc * se), Cell::Number(value + zc * se)]);
                }
                _ => {
                    let blanks = if pairwise { 6 } else { 8 };
                    row.extend(std::iter::repeat_n(Cell::Empty, blanks));
                }
            }
            detail_rows.push(row);
        }
        // the overall test: the differences from the last cell, or the contrast rows
        let c_rows: Matrix = if pairwise {
            let usable: Vec<usize> = (0..kc).filter(|&a| ls[cells[a]].is_some()).collect();
            match usable.split_last() {
                Some((&last, rest)) => rest
                    .iter()
                    .map(|&a| {
                        let mut c = vec![0.0; kc];
                        c[a] = 1.0;
                        c[last] = -1.0;
                        c
                    })
                    .collect(),
                None => Vec::new(),
            }
        } else {
            contrast_list
                .iter()
                .map(|(_, c)| c.clone())
                .filter(|c| estimable(c))
                .collect()
        };
        let cv: Vec<f64> = c_rows.iter().map(|c| estimate(c).0).collect();
        let cvc: Matrix = c_rows
            .iter()
            .map(|ca| {
                c_rows
                    .iter()
                    .map(|cb| {
                        let mut s = 0.0;
                        for (a, &i) in cells.iter().enumerate() {
                            for (b2, &j) in cells.iter().enumerate() {
                                if ca[a] != 0.0 && cb[b2] != 0.0 {
                                    s += ca[a] * cb[b2] * v[i][j];
                                }
                            }
                        }
                        s
                    })
                    .collect()
            })
            .collect();
        let (g, rank) = ginverse(&cvc, 1e-10);
        let chi = dot(&cv, &mat_vec(&g, &cv));
        let mut row = Vec::new();
        other_cells(&mut row, true);
        let ok = rank > 0;
        row.extend([
            number(ok.then_some(chi)),
            Cell::Number(rank as f64),
            number(ok.then(|| chi_square_p(chi.max(0.0), rank as f64))),
        ]);
        overall_rows.push(row);
    }
    let mut columns: Vec<(String, String)> = others
        .iter()
        .map(|&a| (String::new(), d.factor_texts[factors[a]].clone()))
        .collect();
    let adjustment = format!(
        "Adjustment for multiple comparisons: {}.",
        adjust_name(e.padjust)
    );
    if pairwise {
        columns.push((String::new(), format!("(I) {compared_name}")));
        columns.push((String::new(), format!("(J) {compared_name}")));
        columns.extend(plain(&[
            "Mean Difference (I-J)",
            "Std. Error",
            "df",
            "Sig.",
        ]));
        let group = format!("{} Wald Confidence Interval for Difference", cx.level);
        columns.push((group.clone(), "Lower".into()));
        columns.push((group, "Upper".into()));
        out.push(table(
            &format!("Pairwise Comparisons: {compared_name}"),
            others.len() + 2,
            columns.clone(),
            detail_rows,
            Some(format!(
                "{caption} Pairwise comparisons of estimated marginal means. {adjustment}"
            )),
        ));
        columns.truncate(others.len());
    } else {
        columns.push((String::new(), "Contrast".into()));
        columns.extend(plain(&[
            "Contrast Estimate",
            "Test Value",
            "Std. Error",
            "Wald Chi-Square",
            "df",
            "Sig.",
        ]));
        let group = format!("{} Wald Confidence Interval for Difference", cx.level);
        columns.push((group.clone(), "Lower".into()));
        columns.push((group, "Upper".into()));
        out.push(table(
            &format!("Individual Test Results: {compared_name}"),
            others.len() + 1,
            columns.clone(),
            detail_rows,
            Some(format!("{caption} {adjustment}")),
        ));
        columns.truncate(others.len());
    }
    columns.extend(plain(&["Wald Chi-Square", "df", "Sig."]));
    out.push(table(
        &format!("Overall Test Results: {compared_name}"),
        others.len(),
        columns,
        overall_rows,
        Some(format!(
            "{caption} The Wald chi-square tests the effect of {compared_name}, based on the linearly independent contrasts among the estimated marginal means."
        )),
    ));
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// SAVE

struct SaveContext<'a> {
    command: &'a Genlin,
    multinomial: bool,
    binary: bool,
    events: bool,
    family: Family,
    result: &'a Result_,
    glm: Option<&'a GlmFit>,
    cases: &'a Cases,
    x: &'a [Vec<f64>],
    /// The design row (kept columns) of every case of the dataset with valid predictors.
    rows_of: &'a [Option<Vec<f64>>],
    /// The row of the fit of each case of the dataset.
    fitted_row: &'a [Option<usize>],
    /// The number of trials of each case of the dataset, where valid.
    trials: &'a [Option<f64>],
    offset_of: &'a dyn Fn(usize) -> f64,
    z: f64,
    categories: &'a [Key],
    dependent: &'a Variable<'a>,
    j: usize,
    px: usize,
    ordinal: &'a Ordinal,
    /// Generalized estimating equations: no leverage, deviance or standardized residuals.
    gee: bool,
}

fn save_columns(s: &SaveContext, warnings: &mut Vec<String>) -> Vec<Saved> {
    use GenlinSaveKind as K;
    let link = s.command.link;
    let r = s.result;
    let n = s.rows_of.len();
    let mut out = Vec::new();
    let numbers =
        |name: &str, given: &Option<String>, label: &str, values: Vec<Option<f64>>| Saved {
            name: given.clone().unwrap_or_else(|| name.to_string()),
            given: given.is_some(),
            label: label.to_string(),
            // a value that cannot be computed (leverage 1, no parameters) is missing
            values: SavedValues::Numbers(
                values
                    .into_iter()
                    .map(|v| v.filter(|x| x.is_finite()))
                    .collect(),
                "F11.5",
            ),
        };
    if s.multinomial {
        let k = s.j - 1;
        let q = r.b.len();
        // eta_j and its standard error for case kc
        let eta = |kc: usize, j: usize| -> Option<(f64, f64)> {
            let x = s.rows_of[kc].as_ref()?;
            let value = r.b[j] - dot(x, &r.b[k..]) + (s.offset_of)(kc);
            let mut l = vec![0.0; q];
            l[j] = 1.0;
            for (t, xv) in x.iter().enumerate() {
                l[k + t] = -xv;
            }
            let var = dot(&l, &mat_vec(&r.cov, &l));
            Some((value, var.max(0.0).sqrt()))
        };
        for item in &s.command.save {
            let count = item.count.min(k);
            let root = |default: &str| item.name.clone().unwrap_or_else(|| default.to_string());
            let per_category = |default: &str,
                                label: &str,
                                value: &dyn Fn(f64, f64) -> Option<f64>|
             -> Vec<Saved> {
                (0..count)
                    .map(|j| Saved {
                        name: format!("{}_{}", root(default), j + 1),
                        given: item.name.is_some(),
                        label: format!(
                            "{label} for category {}",
                            key_value_text(s.dependent, &s.categories[j])
                        ),
                        values: SavedValues::Numbers(
                            (0..n)
                                .map(|kc| eta(kc, j).and_then(|(e, se)| value(e, se)))
                                .collect(),
                            "F11.5",
                        ),
                    })
                    .collect()
            };
            let z = s.z;
            match item.kind {
                K::XbPred => out.extend(per_category(
                    "XBPredicted",
                    "Predicted value of the linear predictor",
                    &|e, _| Some(e),
                )),
                K::XbStdError => out.extend(per_category(
                    "XBStandardError",
                    "Standard error of the linear predictor",
                    &|_, se| Some(se),
                )),
                K::MeanPred => out.extend(per_category(
                    "CumMeanPredicted",
                    "Predicted cumulative probability",
                    &|e, _| Some(link_inverse(link, e, 0.0)),
                )),
                K::CiMeanPredL => out.extend(per_category(
                    "CICumMeanPredictedLower",
                    "Lower bound of the cumulative probability",
                    &|e, se| Some(link_inverse(link, e - z * se, 0.0)).filter(|v| v.is_finite()),
                )),
                K::CiMeanPredU => out.extend(per_category(
                    "CICumMeanPredictedUpper",
                    "Upper bound of the cumulative probability",
                    &|e, se| Some(link_inverse(link, e + z * se, 0.0)).filter(|v| v.is_finite()),
                )),
                K::PredVal => {
                    let counts: Vec<f64> = (0..s.j)
                        .map(|cat| {
                            (0..s.ordinal.category.len())
                                .filter(|&i| s.ordinal.category[i] == cat)
                                .map(|i| s.ordinal.f[i])
                                .sum()
                        })
                        .collect();
                    let predicted: Vec<Option<usize>> = (0..n)
                        .map(|kc| {
                            let x = s.rows_of[kc].as_ref()?;
                            let probs = cell_probabilities(&cumulative(
                                link,
                                &r.b,
                                x,
                                (s.offset_of)(kc),
                                k,
                            ));
                            let mut best = 0;
                            for cat in 1..s.j {
                                let better = probs[cat] > probs[best] + 1e-12
                                    || ((probs[cat] - probs[best]).abs() <= 1e-12
                                        && counts[cat] > counts[best]);
                                if better {
                                    best = cat;
                                }
                            }
                            Some(best)
                        })
                        .collect();
                    out.push(category_column(item.name.clone(), &predicted, s.categories));
                }
                other => warnings.push(format!(
                    "SAVE {} is not available for the multinomial distribution; it is left out",
                    save_keyword(other)
                )),
            }
        }
        return out;
    }
    let glm = s.glm.expect("fitted");
    let k = r.k;
    let phi = r.phi;
    let eta = |kc: usize| -> Option<(f64, f64)> {
        let x = s.rows_of[kc].as_ref()?;
        let value = dot(x, &r.b) + (s.offset_of)(kc);
        let var = dot(x, &mat_vec(&r.cov, x));
        Some((value, var.max(0.0).sqrt()))
    };
    let trials = |kc: usize| -> Option<f64> { if s.events { s.trials[kc] } else { Some(1.0) } };
    // leverage from the expected information of the fit at scale 1
    let c = s.cases;
    let q = r.b.len();
    let mut xwx = vec![vec![0.0; q]; q];
    let weight = |i: usize| {
        let m = glm.mu[i];
        let g1 = link_d1(link, m, k);
        c.a(i) / (variance(s.family, m, k) * g1 * g1)
    };
    // no leverage for generalized estimating equations
    for i in (0..c.n()).filter(|_| !s.gee) {
        let w = c.f[i] * weight(i);
        for (a, row) in xwx.iter_mut().enumerate() {
            for (b, v) in row.iter_mut().enumerate() {
                *v += w * s.x[i][a] * s.x[i][b];
            }
        }
    }
    let inverse = if s.gee {
        Vec::new()
    } else {
        ginverse(&xwx, s.command.criteria.singular).0
    };
    let leverage = |i: usize| {
        if s.gee {
            f64::NAN
        } else {
            weight(i) * dot(&s.x[i], &mat_vec(&inverse, &s.x[i]))
        }
    };
    let residuals = |kc: usize| -> Option<(f64, f64, f64, f64)> {
        // raw, Pearson, deviance residual and leverage of a case of the fit
        let i = s.fitted_row[kc]?;
        let m = glm.mu[i];
        let y = c.y[i];
        let a = c.a(i);
        let raw = (y - m) * c.m[i];
        let pearson = (y - m) * (a / variance(s.family, m, k)).sqrt();
        let dev = (a * unit_deviance(s.family, y, m, k)).max(0.0).sqrt() * (y - m).signum();
        Some((raw, pearson, dev, leverage(i)))
    };
    let any_residual = s.command.save.iter().any(|i| {
        !matches!(
            i.kind,
            K::XbPred | K::XbStdError | K::MeanPred | K::CiMeanPredL | K::CiMeanPredU | K::PredVal
        )
    });
    let computed: Vec<Option<(f64, f64, f64, f64)>> = if any_residual {
        (0..n).map(residuals).collect()
    } else {
        vec![None; n]
    };
    let residuals = |kc: usize| computed[kc];
    let column = |f: &dyn Fn(usize) -> Option<f64>| -> Vec<Option<f64>> { (0..n).map(f).collect() };
    let std_pearson = |kc: usize| residuals(kc).map(|(_, p, _, h)| p / (phi * (1.0 - h)).sqrt());
    let std_deviance = |kc: usize| residuals(kc).map(|(_, _, dv, h)| dv / (phi * (1.0 - h)).sqrt());
    for item in &s.command.save {
        let given = &item.name;
        let z = s.z;
        if s.gee
            && matches!(
                item.kind,
                K::Leverage
                    | K::DevianceResid
                    | K::StdPearsonResid
                    | K::StdDevianceResid
                    | K::LikelihoodResid
                    | K::Cook
            )
        {
            warnings.push(format!(
                "SAVE {} is not available for generalized estimating equations; it is left out",
                save_keyword(item.kind)
            ));
            continue;
        }
        match item.kind {
            K::XbPred => out.push(numbers(
                "XBPredicted",
                given,
                "Predicted value of the linear predictor",
                column(&|kc| eta(kc).map(|e| e.0)),
            )),
            K::XbStdError => out.push(numbers(
                "XBStandardError",
                given,
                "Standard error of the linear predictor",
                column(&|kc| eta(kc).map(|e| e.1)),
            )),
            K::MeanPred => out.push(numbers(
                "MeanPredicted",
                given,
                "Predicted value of the mean of the response",
                column(&|kc| {
                    let (e, _) = eta(kc)?;
                    Some(link_inverse(link, e, k) * trials(kc)?).filter(|v| v.is_finite())
                }),
            )),
            K::CiMeanPredL | K::CiMeanPredU => {
                let upper = item.kind == K::CiMeanPredU;
                let values = column(&|kc| {
                    let (e, se) = eta(kc)?;
                    let a = link_inverse(link, e - z * se, k);
                    let b = link_inverse(link, e + z * se, k);
                    if !(a.is_finite() && b.is_finite()) {
                        return None;
                    }
                    Some(if upper { a.max(b) } else { a.min(b) } * trials(kc)?)
                });
                out.push(if upper {
                    numbers(
                        "CIMeanPredictedUpper",
                        given,
                        "Upper bound of the confidence interval for the mean",
                        values,
                    )
                } else {
                    numbers(
                        "CIMeanPredictedLower",
                        given,
                        "Lower bound of the confidence interval for the mean",
                        values,
                    )
                });
            }
            K::PredVal => {
                if !s.binary {
                    warnings.push("SAVE PREDVAL needs a binary binomial or a multinomial response; it is left out".into());
                    continue;
                }
                let predicted: Vec<Option<usize>> = (0..n)
                    .map(|kc| {
                        eta(kc).map(|(e, _)| {
                            if link_inverse(link, e, k) >= 0.5 {
                                0
                            } else {
                                1
                            }
                        })
                    })
                    .collect();
                out.push(category_column(given.clone(), &predicted, s.categories));
            }
            K::Leverage => out.push(numbers(
                "Leverage",
                given,
                "Leverage value",
                column(&|kc| residuals(kc).map(|r| r.3)),
            )),
            K::Resid => out.push(numbers(
                "Residual",
                given,
                "Raw residual",
                column(&|kc| residuals(kc).map(|r| r.0)),
            )),
            K::PearsonResid => out.push(numbers(
                "PearsonResidual",
                given,
                "Pearson residual",
                column(&|kc| residuals(kc).map(|r| r.1)),
            )),
            K::DevianceResid => out.push(numbers(
                "DevianceResidual",
                given,
                "Deviance residual",
                column(&|kc| residuals(kc).map(|r| r.2)),
            )),
            K::StdPearsonResid => out.push(numbers(
                "StdPearsonResidual",
                given,
                "Standardized Pearson residual",
                column(&std_pearson),
            )),
            K::StdDevianceResid => out.push(numbers(
                "StdDevianceResidual",
                given,
                "Standardized deviance residual",
                column(&std_deviance),
            )),
            K::LikelihoodResid => out.push(numbers(
                "LikelihoodResidual",
                given,
                "Likelihood residual",
                column(&|kc| {
                    let (_, _, dv, h) = residuals(kc)?;
                    let sp = std_pearson(kc)?;
                    let sd = std_deviance(kc)?;
                    Some(dv.signum() * (h * sp * sp + (1.0 - h) * sd * sd).sqrt())
                }),
            )),
            K::Cook => out.push(numbers(
                "CooksDistance",
                given,
                "Cook's distance",
                column(&|kc| {
                    let (_, _, _, h) = residuals(kc)?;
                    let sp = std_pearson(kc)?;
                    Some(h / (s.px as f64 * (1.0 - h)) * sp * sp)
                }),
            )),
        }
    }
    out
}

fn save_keyword(kind: GenlinSaveKind) -> &'static str {
    use GenlinSaveKind as K;
    match kind {
        K::XbPred => "XBPRED",
        K::XbStdError => "XBSTDERROR",
        K::MeanPred => "MEANPRED",
        K::CiMeanPredL => "CIMEANPREDL",
        K::CiMeanPredU => "CIMEANPREDU",
        K::PredVal => "PREDVAL",
        K::Leverage => "LEVERAGE",
        K::Resid => "RESID",
        K::PearsonResid => "PEARSONRESID",
        K::DevianceResid => "DEVIANCERESID",
        K::StdPearsonResid => "STDPEARSONRESID",
        K::StdDevianceResid => "STDDEVIANCERESID",
        K::LikelihoodResid => "LIKELIHOODRESID",
        K::Cook => "COOK",
    }
}

fn key_text(key: &Key) -> String {
    match key {
        Key::Number(n) => n.to_string(),
        Key::Text(t) => t.trim_end().to_string(),
    }
}

/// PREDVAL: the predicted category as its value, a number or a text.
fn category_column(name: Option<String>, predicted: &[Option<usize>], categories: &[Key]) -> Saved {
    let given = name.is_some();
    let name = name.unwrap_or_else(|| "PredictedValue".into());
    let label = "Predicted category value".to_string();
    let values = match categories.first() {
        Some(Key::Text(_)) => SavedValues::Texts(
            predicted
                .iter()
                .map(|p| p.map(|c| key_text(&categories[c])).unwrap_or_default())
                .collect(),
        ),
        _ => SavedValues::Numbers(
            predicted
                .iter()
                .map(|p| {
                    p.and_then(|c| match &categories[c] {
                        Key::Number(v) => Some(*v),
                        Key::Text(_) => None,
                    })
                })
                .collect(),
            "F8.2",
        ),
    };
    Saved {
        name,
        given,
        label,
        values,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_binomial_series_match_exact_sums_at_large_r() {
        // for a whole y both differences are finite sums over j below y
        for r in [2e4, 1e8, 1e12] {
            for y in [1.0, 7.0, 40.0] {
                let terms = (0..y as usize).map(|j| r + j as f64);
                let ln_sum: f64 = terms.clone().map(f64::ln).sum();
                let inverse_sum: f64 = terms.map(|t| 1.0 / t).sum();
                let ratio = ln_gamma_ratio(y, r);
                let digammas = digamma_difference(y, r);
                assert!(
                    (ratio - ln_sum).abs() <= 1e-13 * ln_sum,
                    "{r} {y} {ratio} {ln_sum}"
                );
                assert!(
                    (digammas - inverse_sum).abs() <= 1e-11 * inverse_sum,
                    "{r} {y} {digammas} {inverse_sum}"
                );
            }
        }
    }
}
