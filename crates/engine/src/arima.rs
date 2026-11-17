//! ARIMA: seasonal ARIMA models with regressors, estimated by exact maximum likelihood, with
//! fitted values, forecasts and prediction limits.
//!
//! Rules noted from the Command Syntax Reference and the Algorithms manual:
//! - The model: AR(B) SAR(B^s) [Delta (y_t - sum c_i x_it) - mu] = MA(B) SMA(B^s) a_t, every
//!   polynomial 1 - c_1 B - ..., Delta = (1 - B)^d (1 - B^s)^sd; mu only with CONSTANT.
//! - Exact maximum likelihood, also with missing values inside the series; Marquardt steps
//!   on the adjusted sum of squares SSQ' = SSQ |Omega|^(1/N). Stops after MXITER steps, when
//!   every estimate changes by less than PAREPS, when SSQ' changes by less than SSQPCT
//!   percent, or when the Marquardt constant passes MXLAMB.
//! - Initial values: AR from the extended Yule-Walker equations, MA from the cross
//!   covariances of an AR(p + q) fit, seasonal ones from the autocorrelations at the
//!   seasonal lags.
//! - Statistics: residual variance SSQ / (N - Np), log-likelihood -N ln(sigma) - SSQ' /
//!   (2 sigma^2) - N ln(2 pi) / 2, AIC -2L + 2 Np, SBC -2L + ln(N) Np.
//! - FIT: EXACT one-step least-squares predictions; CLS with past errors 0 and unknown past
//!   values at the mean; AUTOINIT starting from the first d + p + s (sd + sp) values. The
//!   first d + s sd fits are missing. Limits FIT -+ t(df) SEP; SEP is sigma for CLS and
//!   AUTOINIT, sigma (1 + psi_1^2 + ... + psi_(h-1)^2)^(1/2) for their forecasts. Residuals
//!   stay in the log metric; fits, limits and standard errors go back to the original one.
//!
//! Choices where the manual is silent:
//! - The likelihood is the Kalman filter's: the state holds the ARMA part of the differenced
//!   series and the last d + s sd values of the series, so it starts from the first such
//!   stretch of valid values (they must be valid) and takes later gaps exactly.
//! - Derivatives are central differences. The estimates' covariance is the inverse of the
//!   observed information: the second derivatives of (N / 2) ln SSQ', minus the exact
//!   log-likelihood with the variance profiled out. The Marquardt constant
//!   starts at 0.001, scales the diagonal of J'J and moves by factors of 10.
//! - A step to a nonstationary autoregressive part is refused like one that raises SSQ'.
//! - N counts the valid values after the first d + s sd.
//! - Back from logs, SEP is the standard error times the derivative of the inverse
//!   transformation at the fit (a first-order approximation).

use crate::descriptives::number;
use crate::dist::{t_p, t_quantile};
use crate::matrix::inverse;
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::timeseries::{Values, autocorrelations, read_series};
use crate::{Cell, Engine, Error, Table};
use statscraft_syntax::{Arima, ArimaApply, ArimaForecast, ArimaLog, TsetPrint};

/// The last ARIMA and its estimates, for APPLY.
#[derive(Debug, Clone)]
pub(crate) struct Previous {
    spec: Arima,
    estimates: Option<Estimates>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Estimates {
    pub(crate) ar: Vec<f64>,
    pub(crate) ma: Vec<f64>,
    pub(crate) sar: Vec<f64>,
    pub(crate) sma: Vec<f64>,
    pub(crate) reg: Vec<f64>,
    pub(crate) con: Option<f64>,
}

/// The structure of a model: lags, differencing, period, constant.
#[derive(Debug, Clone)]
pub(crate) struct Structure {
    pub(crate) ar: Vec<usize>,
    pub(crate) ma: Vec<usize>,
    pub(crate) sar: Vec<usize>,
    pub(crate) sma: Vec<usize>,
    pub(crate) d: usize,
    pub(crate) sd: usize,
    pub(crate) s: usize,
    pub(crate) regressors: usize,
    pub(crate) constant: bool,
}

impl Structure {
    /// The parameters in their order: AR, MA, SAR, SMA, regressors, constant.
    pub(crate) fn split<'a>(&self, theta: &'a [f64]) -> [&'a [f64]; 6] {
        let mut at = 0;
        let mut take = |n: usize| {
            let part = &theta[at..at + n];
            at += n;
            part
        };
        [
            take(self.ar.len()),
            take(self.ma.len()),
            take(self.sar.len()),
            take(self.sma.len()),
            take(self.regressors),
            take(usize::from(self.constant)),
        ]
    }

    /// The differencing polynomial's coefficients 1, -delta_1, ...
    pub(crate) fn difference(&self) -> Vec<f64> {
        let mut poly = vec![1.0];
        for _ in 0..self.d {
            poly = product(&poly, &[1.0, -1.0]);
        }
        for _ in 0..self.sd {
            let mut seasonal = vec![0.0; self.s + 1];
            (seasonal[0], seasonal[self.s]) = (1.0, -1.0);
            poly = product(&poly, &seasonal);
        }
        poly
    }

    /// The number of past values Delta reaches back.
    pub(crate) fn reach(&self) -> usize {
        self.d + self.s * self.sd
    }
}

/// The product of two polynomials in B.
fn product(a: &[f64], b: &[f64]) -> Vec<f64> {
    let mut out = vec![0.0; a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            out[i + j] += x * y;
        }
    }
    out
}

/// 1 - sum c_l B^(l step) for the lags and coefficients given.
fn lag_poly(lags: &[usize], coefficients: &[f64], step: usize) -> Vec<f64> {
    let top = lags.iter().max().map_or(0, |l| l * step);
    let mut poly = vec![0.0; top + 1];
    poly[0] = 1.0;
    for (l, c) in lags.iter().zip(coefficients) {
        poly[l * step] -= c;
    }
    poly
}

/// The ARMA part of a parameter vector: phi (x_t = sum phi_k x_(t-k) + ...) and the MA
/// coefficients in the form 1 + sum c_k B^k.
fn arma(st: &Structure, theta: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let [ar, ma, sar, sma, _, _] = st.split(theta);
    let ar_poly = product(&lag_poly(&st.ar, ar, 1), &lag_poly(&st.sar, sar, st.s));
    let ma_poly = product(&lag_poly(&st.ma, ma, 1), &lag_poly(&st.sma, sma, st.s));
    let phi = ar_poly[1..].iter().map(|c| -c).collect();
    (phi, ma_poly[1..].to_vec())
}

/// Whether x_t = sum phi_k x_(t-k) + noise is stationary (Schur-Cohn step-down).
fn stationary(phi: &[f64]) -> bool {
    let mut a: Vec<f64> = phi.to_vec();
    while let Some(&last) = a.last() {
        if last == 0.0 {
            a.pop();
            continue;
        }
        let k = a.len();
        let kappa = last;
        if kappa.abs() >= 1.0 {
            return false;
        }
        let scale = 1.0 - kappa * kappa;
        a = (0..k - 1)
            .map(|j| (a[j] + kappa * a[k - 2 - j]) / scale)
            .collect();
    }
    true
}

/// The psi weights psi_0 .. psi_(len - 1) of x_t = sum phi_k x_(t-k) + a_t + sum c_j a_(t-j).
fn psi_weights(phi: &[f64], c: &[f64], len: usize) -> Vec<f64> {
    let mut psi = vec![0.0; len];
    for j in 0..len {
        let mut v = if j == 0 {
            1.0
        } else {
            c.get(j - 1).copied().unwrap_or(0.0)
        };
        for (k, p) in phi.iter().enumerate() {
            if k < j {
                v += p * psi[j - 1 - k];
            }
        }
        psi[j] = v;
    }
    psi
}

/// The autocovariances gamma(0 .. len - 1) of a stationary ARMA with unit innovation variance.
fn autocovariances(phi: &[f64], c: &[f64], len: usize) -> Option<Vec<f64>> {
    let p = phi.len();
    let q = c.len();
    let psi = psi_weights(phi, c, q + 1);
    let theta = |j: usize| if j == 0 { 1.0 } else { c[j - 1] };
    // right side: sum_(j = k..q) theta_j psi_(j - k)
    let right = |k: usize| (k..=q).map(|j| theta(j) * psi[j - k]).sum::<f64>();
    // gamma(0..p) from gamma(k) - sum phi_j gamma(|k - j|) = right(k)
    let mut a = vec![vec![0.0; p + 1]; p + 1];
    let mut b = vec![0.0; p + 1];
    for k in 0..=p {
        a[k][k] += 1.0;
        for j in 1..=p {
            a[k][k.abs_diff(j)] -= phi[j - 1];
        }
        b[k] = right(k);
    }
    let (inv, _) = inverse(&a)?;
    let mut gamma: Vec<f64> = (0..=p)
        .map(|i| (0..=p).map(|j| inv[i][j] * b[j]).sum())
        .collect();
    for k in p + 1..len.max(p + 1) {
        let v = (1..=p).map(|j| phi[j - 1] * gamma[k - j]).sum::<f64>() + right(k);
        gamma.push(v);
    }
    gamma.truncate(len);
    (gamma[0] > 0.0 && gamma.iter().all(|g| g.is_finite())).then_some(gamma)
}

/// A state element as (coefficient, lag) terms in past series values and past innovations.
type Terms = (Vec<(f64, usize)>, Vec<(f64, usize)>);

/// The state space form of the ARMA part: dimension r, and the stationary covariance of
/// its state (unit innovation variance).
fn state_covariance(phi: &[f64], c: &[f64], r: usize) -> Option<Vec<Vec<f64>>> {
    let gamma = autocovariances(phi, c, 2 * r + 2)?;
    let psi = psi_weights(phi, c, 2 * r + 2);
    let phi_at = |k: usize| phi.get(k - 1).copied().unwrap_or(0.0);
    let c_at = |k: usize| {
        if k == 0 {
            1.0
        } else {
            c.get(k - 1).copied().unwrap_or(0.0)
        }
    };
    // element j (from 1): sum_(k=j..r) phi_k w_(t-(k-j+1)) + sum_(k=j-1..r-1) c_k a_(t-(k-j+1))
    let terms = |j: usize| -> Terms {
        if j == 1 {
            return (vec![(1.0, 0)], Vec::new());
        }
        let w = (j..=r).map(|k| (phi_at(k), k + 1 - j)).collect();
        let a = (j - 1..r).map(|k| (c_at(k), k + 1 - j)).collect();
        (w, a)
    };
    let all: Vec<_> = (1..=r).map(terms).collect();
    let cross = |(w1, a1): &Terms, (w2, a2): &Terms| {
        let mut v = 0.0;
        for (x, l1) in w1 {
            for (y, l2) in w2 {
                v += x * y * gamma[l1.abs_diff(*l2)];
            }
            for (y, l2) in a2 {
                // E[w_(t-l1) a_(t-l2)] = psi_(l2 - l1) when l2 >= l1
                if l2 >= l1 {
                    v += x * y * psi[l2 - l1];
                }
            }
        }
        for (x, l1) in a1 {
            for (y, l2) in w2 {
                if l1 >= l2 {
                    v += x * y * psi[l1 - l2];
                }
            }
            for (y, l2) in a2 {
                if l1 == l2 {
                    v += x * y;
                }
            }
        }
        v
    };
    Some(
        (0..r)
            .map(|i| (0..r).map(|j| cross(&all[i], &all[j])).collect())
            .collect(),
    )
}

/// The Kalman filter of z (the series less its regressors): the one-step prediction and its
/// variance (in units of sigma^2) of every case from `start + reach`, updating with the
/// valid values before `stop`.
struct Filtered {
    /// (case, prediction, variance, innovation if the case was valid)
    steps: Vec<(usize, f64, f64, Option<f64>)>,
}

fn kalman(
    st: &Structure,
    theta: &[f64],
    z: &[Option<f64>],
    start: usize,
    stop: usize,
) -> Option<Filtered> {
    let (phi, c) = arma(st, theta);
    if !stationary(&phi) {
        return None;
    }
    let mu = if st.constant {
        theta[theta.len() - 1]
    } else {
        0.0
    };
    let r = phi.len().max(c.len() + 1);
    let delta = st.difference();
    let reach = st.reach();
    let m = r + reach;
    let p0 = state_covariance(&phi, &c, r)?;
    // state: ARMA part x (r), then z_(t-1) .. z_(t-reach)
    let mut a = vec![0.0; m];
    let mut p = vec![vec![0.0; m]; m];
    for i in 0..r {
        p[i][..r].copy_from_slice(&p0[i]);
    }
    for k in 0..reach {
        a[r + k] = z[start + reach - 1 - k]?;
    }
    // observation row: z_t = mu + x_1 + sum delta_k z_(t-k)
    let mut zrow = vec![0.0; m];
    zrow[0] = 1.0;
    for k in 0..reach {
        zrow[r + k] = -delta[k + 1];
    }
    let phi_at = |k: usize| phi.get(k).copied().unwrap_or(0.0);
    let rvec: Vec<f64> = (0..r)
        .map(|k| {
            if k == 0 {
                1.0
            } else {
                c.get(k - 1).copied().unwrap_or(0.0)
            }
        })
        .collect();
    let mut steps = Vec::new();
    for (t, &value) in z.iter().enumerate().skip(start + reach) {
        let pz: Vec<f64> = (0..m)
            .map(|i| (0..m).map(|j| p[i][j] * zrow[j]).sum())
            .collect();
        let f = (0..m).map(|i| zrow[i] * pz[i]).sum::<f64>();
        let pred = mu + (0..m).map(|i| zrow[i] * a[i]).sum::<f64>();
        let observed = if t < stop { value } else { None };
        let innovation = observed.map(|v| v - pred);
        if let Some(v) = innovation {
            if f <= 1e-12 {
                return None;
            }
            for i in 0..m {
                a[i] += pz[i] * v / f;
            }
            for i in 0..m {
                for j in 0..m {
                    p[i][j] -= pz[i] * pz[j] / f;
                }
            }
        }
        steps.push((t, pred, f, innovation));
        // transition: x' = T x + R a, z lags shift with z_t = mu + zrow . state
        let zt = mu + (0..m).map(|i| zrow[i] * a[i]).sum::<f64>();
        let mut na = vec![0.0; m];
        for i in 0..r {
            na[i] = phi_at(i) * a[0] + if i + 1 < r { a[i + 1] } else { 0.0 };
        }
        if reach > 0 {
            na[r] = zt;
            for k in 1..reach {
                na[r + k] = a[r + k - 1];
            }
        }
        // A P A' through A applied to rows then columns
        let apply = |v: &[f64]| -> Vec<f64> {
            let mut out = vec![0.0; m];
            for i in 0..r {
                out[i] = phi_at(i) * v[0] + if i + 1 < r { v[i + 1] } else { 0.0 };
            }
            if reach > 0 {
                out[r] = (0..m).map(|j| zrow[j] * v[j]).sum();
                for k in 1..reach {
                    out[r + k] = v[r + k - 1];
                }
            }
            out
        };
        let rows: Vec<Vec<f64>> = p.iter().map(|row| apply(row)).collect();
        // rows[i][j] = (P A')[i][j]; now A (P A')
        let mut np = vec![vec![0.0; m]; m];
        for j in 0..m {
            let column: Vec<f64> = (0..m).map(|i| rows[i][j]).collect();
            let out = apply(&column);
            for i in 0..m {
                np[i][j] = out[i];
            }
        }
        for i in 0..r {
            for j in 0..r {
                np[i][j] += rvec[i] * rvec[j];
            }
        }
        a = na;
        p = np;
    }
    Some(Filtered { steps })
}

/// The scaled residuals whose sum of squares is SSQ', with SSQ and N; None for parameters
/// outside the stationary region.
fn scaled_residuals(
    st: &Structure,
    theta: &[f64],
    y: &[Option<f64>],
    x: &[Vec<Option<f64>>],
    start: usize,
    stop: usize,
) -> Option<(Vec<f64>, f64, usize)> {
    let z = adjusted(st, theta, y, x);
    let filtered = kalman(st, theta, &z, start, stop)?;
    let used: Vec<(f64, f64)> = filtered
        .steps
        .iter()
        .filter_map(|&(_, _, f, v)| Some((v?, f)))
        .collect();
    let n = used.len();
    if n == 0 {
        return None;
    }
    let log_det = used.iter().map(|(_, f)| f.ln()).sum::<f64>();
    let scale = (log_det / (2.0 * n as f64)).exp();
    let ssq = used.iter().map(|(v, f)| v * v / f).sum::<f64>();
    let r = used.iter().map(|(v, f)| v / f.sqrt() * scale).collect();
    Some((r, ssq, n))
}

/// The series less the regressors times their coefficients.
fn adjusted(st: &Structure, theta: &[f64], y: &[Option<f64>], x: &[Vec<Option<f64>>]) -> Values {
    let [_, _, _, _, reg, _] = st.split(theta);
    (0..y.len())
        .map(|t| {
            let mut v = y[t]?;
            for (c, column) in reg.iter().zip(x) {
                v -= c * column[t]?;
            }
            Some(v)
        })
        .collect()
}

/// Solves a small linear system; None when singular.
fn solve(a: &[Vec<f64>], b: &[f64]) -> Option<Vec<f64>> {
    let (inv, _) = inverse(&a.to_vec())?;
    Some(
        (0..b.len())
            .map(|i| (0..b.len()).map(|j| inv[i][j] * b[j]).sum())
            .collect(),
    )
}

/// Yule-Walker AR(k) coefficients from rho_1 .. rho_k (rho_0 = 1).
fn yule_walker(rho: &[f64], k: usize) -> Option<Vec<f64>> {
    let at = |l: usize| {
        if l == 0 {
            1.0
        } else {
            rho.get(l - 1).copied().unwrap_or(0.0)
        }
    };
    let a: Vec<Vec<f64>> = (0..k)
        .map(|i| (0..k).map(|j| at(i.abs_diff(j))).collect())
        .collect();
    let b: Vec<f64> = (1..=k).map(at).collect();
    solve(&a, &b)
}

/// Initial AR (p) and MA (q) coefficients of consecutive orders from the autocorrelations
/// rho_1, rho_2, ... (of the series at the step that matters).
fn initial_arma(rho: &[f64], p: usize, q: usize) -> (Vec<f64>, Vec<f64>) {
    // only rho_1 .. rho_(p+q) are used, the others are 0
    let at = |l: isize| -> f64 {
        let l = l.unsigned_abs();
        if l == 0 {
            1.0
        } else if l <= p + q {
            rho.get(l - 1).copied().unwrap_or(0.0)
        } else {
            0.0
        }
    };
    // extended Yule-Walker: rho_(q+i) = sum_j phi_j rho_(q+i-j)
    let phi: Vec<f64> = if p == 0 {
        Vec::new()
    } else {
        let a: Vec<Vec<f64>> = (1..=p)
            .map(|i| (1..=p).map(|j| at((q + i) as isize - j as isize)).collect())
            .collect();
        let b: Vec<f64> = (1..=p).map(|i| at((q + i) as isize)).collect();
        solve(&a, &b).unwrap_or_else(|| vec![0.0; p])
    };
    if q == 0 {
        return (phi, Vec::new());
    }
    let long: Vec<f64> = {
        let r: Vec<f64> = (1..=p + q).map(|l| at(l as isize)).collect();
        yule_walker(&r, p + q).unwrap_or_else(|| vec![0.0; p + q])
    };
    let lp = |i: usize| if i == 0 { -1.0 } else { long[i - 1] };
    let denominator: f64 = (0..=p + q)
        .flat_map(|i| (0..=p + q).map(move |j| (i, j)))
        .map(|(i, j)| lp(i) * lp(j) * at(i as isize - j as isize))
        .sum();
    let theta = (1..=q)
        .map(|l| {
            let li = l as isize;
            let mut v = at(li);
            v -= (1..=p + q)
                .map(|j| long[j - 1] * at(li + j as isize))
                .sum::<f64>();
            v -= (1..=p)
                .map(|i| phi[i - 1] * at(li - i as isize))
                .sum::<f64>();
            for i in 1..=p {
                for j in 1..=p + q {
                    v += phi[i - 1] * long[j - 1] * at(li + j as isize - i as isize);
                }
            }
            // lambda_l = -theta_l sigma^2
            -v / denominator
        })
        .collect();
    (phi, theta)
}

/// The coefficients at `lags` of consecutive-order initial values, or 0 where unusable.
fn pick(values: &[f64], lags: &[usize]) -> Vec<f64> {
    lags.iter()
        .map(|&l| {
            values
                .get(l - 1)
                .copied()
                .filter(|v| v.is_finite())
                .unwrap_or(0.0)
        })
        .collect()
}

/// The ordinary least squares coefficients of y on the columns (rows with any missing value
/// left out).
pub(crate) fn least_squares(y: &[Option<f64>], columns: &[Vec<Option<f64>>]) -> Option<Vec<f64>> {
    let k = columns.len();
    let rows: Vec<(f64, Vec<f64>)> = (0..y.len())
        .filter_map(|t| {
            let row: Option<Vec<f64>> = columns.iter().map(|c| c[t]).collect();
            Some((y[t]?, row?))
        })
        .collect();
    if rows.len() <= k {
        return None;
    }
    let mut a = vec![vec![0.0; k]; k];
    let mut b = vec![0.0; k];
    for (v, row) in &rows {
        for i in 0..k {
            b[i] += row[i] * v;
            for j in 0..k {
                a[i][j] += row[i] * row[j];
            }
        }
    }
    solve(&a, &b)
}

/// `x` differenced by the polynomial 1, -delta_1, ...
pub(crate) fn differenced(x: &[Option<f64>], poly: &[f64]) -> Values {
    (0..x.len())
        .map(|t| {
            let mut v = 0.0;
            for (k, c) in poly.iter().enumerate() {
                if *c != 0.0 {
                    v += c * x[t.checked_sub(k)?]?;
                }
            }
            Some(v)
        })
        .collect()
}

/// Initial values of every parameter.
pub(crate) fn initial_values(
    st: &Structure,
    y: &[Option<f64>],
    x: &[Vec<Option<f64>>],
    given: &Estimates,
) -> Vec<f64> {
    let poly = st.difference();
    let dy = differenced(y, &poly);
    let dx: Vec<Values> = x.iter().map(|c| differenced(c, &poly)).collect();
    let mut columns = dx.clone();
    if st.constant {
        columns.push(vec![Some(1.0); y.len()]);
    }
    let ls = match columns.is_empty() {
        true => Vec::new(),
        false => least_squares(&dy, &columns).unwrap_or_else(|| vec![0.0; columns.len()]),
    };
    let reg: Vec<f64> = match given.reg.len() == st.regressors {
        true => given.reg.clone(),
        false => ls[..st.regressors].to_vec(),
    };
    // the differenced series less the regressors
    let w: Values = (0..y.len())
        .map(|t| {
            let mut v = dy[t]?;
            for (c, column) in reg.iter().zip(&dx) {
                v -= c * column[t]?;
            }
            Some(v)
        })
        .collect();
    let valid: Vec<f64> = w.iter().flatten().copied().collect();
    let mean = valid.iter().sum::<f64>() / valid.len().max(1) as f64;
    let con = given.con.unwrap_or(mean);
    let varies = valid
        .iter()
        .any(|v| (v - mean).abs() > 1e-12 * (1.0 + mean.abs()));
    let (p, q) = (
        st.ar.iter().max().copied().unwrap_or(0),
        st.ma.iter().max().copied().unwrap_or(0),
    );
    let (sp, sq) = (
        st.sar.iter().max().copied().unwrap_or(0),
        st.sma.iter().max().copied().unwrap_or(0),
    );
    let longest = (p + q)
        .max(st.s * (sp + sq))
        .min(valid.len().saturating_sub(1));
    let rho = match varies && longest > 0 {
        true => autocorrelations(&w, longest).0,
        false => Vec::new(),
    };
    let (phi, theta) = initial_arma(&rho, p, q);
    let seasonal_rho: Vec<f64> = (1..=sp + sq)
        .map(|k| rho.get(k * st.s.max(1) - 1).copied().unwrap_or(0.0))
        .collect();
    let (sphi, stheta) = initial_arma(&seasonal_rho, sp, sq);
    let choose = |given: &[f64], lags: &[usize], computed: &[f64]| match given.len() == lags.len() {
        true => given.to_vec(),
        false => pick(computed, lags),
    };
    let mut ar = choose(&given.ar, &st.ar, &phi);
    let mut sar = choose(&given.sar, &st.sar, &sphi);
    let mut ma = choose(&given.ma, &st.ma, &theta);
    let mut sma = choose(&given.sma, &st.sma, &stheta);
    // computed values outside the stationary or invertible region start at 0
    let unusable = |lags: &[usize], v: &[f64]| {
        let poly = lag_poly(lags, v, 1);
        let phi: Vec<f64> = poly[1..].iter().map(|c| -c).collect();
        !stationary(&phi)
    };
    if given.ar.len() != st.ar.len() && unusable(&st.ar, &ar) {
        ar.fill(0.0);
    }
    if given.sar.len() != st.sar.len() && unusable(&st.sar, &sar) {
        sar.fill(0.0);
    }
    if given.ma.len() != st.ma.len() && unusable(&st.ma, &ma) {
        ma.fill(0.0);
    }
    if given.sma.len() != st.sma.len() && unusable(&st.sma, &sma) {
        sma.fill(0.0);
    }
    let mut theta0 = Vec::new();
    for part in [ar, ma, sar, sma, reg] {
        theta0.extend(part);
    }
    if st.constant {
        theta0.push(con);
    }
    theta0
}

/// One line of the iteration history: SSQ', the Marquardt constant and the estimates.
struct Step {
    ssq: f64,
    lambda: f64,
    theta: Vec<f64>,
}

/// The derivatives of the scaled residuals by central differences.
fn jacobian(
    residuals: &dyn Fn(&[f64]) -> Option<Vec<f64>>,
    theta: &[f64],
    n: usize,
) -> Option<Vec<Vec<f64>>> {
    let k = theta.len();
    let mut j = vec![vec![0.0; k]; n];
    for i in 0..k {
        let h = 1e-6 * theta[i].abs().max(1e-2);
        let mut up = theta.to_vec();
        let mut down = theta.to_vec();
        up[i] += h;
        down[i] -= h;
        let (ru, rd) = match (residuals(&up), residuals(&down)) {
            (Some(ru), Some(rd)) => (ru, rd),
            // at the edge of the region: one-sided
            (Some(ru), None) => (ru, residuals(theta)?),
            (None, Some(rd)) => (residuals(theta)?, rd),
            (None, None) => return None,
        };
        let width = if residuals(&up).is_some() && residuals(&down).is_some() {
            2.0 * h
        } else {
            h
        };
        for t in 0..n {
            j[t][i] = (ru[t] - rd[t]) / width;
        }
    }
    Some(j)
}

/// The matrix of second derivatives of `f` at `theta` by central differences; None where `f`
/// is not defined.
pub(crate) fn hessian(f: &dyn Fn(&[f64]) -> Option<f64>, theta: &[f64]) -> Option<Vec<Vec<f64>>> {
    let k = theta.len();
    let h: Vec<f64> = theta.iter().map(|t| 1e-4 * t.abs().max(1e-1)).collect();
    let at = |moves: &[(usize, f64)]| {
        let mut t = theta.to_vec();
        for &(i, m) in moves {
            t[i] += m * h[i];
        }
        f(&t)
    };
    let centre = f(theta)?;
    let mut out = vec![vec![0.0; k]; k];
    for i in 0..k {
        out[i][i] = (at(&[(i, 1.0)])? - 2.0 * centre + at(&[(i, -1.0)])?) / (h[i] * h[i]);
        for j in 0..i {
            let v = (at(&[(i, 1.0), (j, 1.0)])?
                - at(&[(i, 1.0), (j, -1.0)])?
                - at(&[(i, -1.0), (j, 1.0)])?
                + at(&[(i, -1.0), (j, -1.0)])?)
                / (4.0 * h[i] * h[j]);
            out[i][j] = v;
            out[j][i] = v;
        }
    }
    Some(out)
}

fn cross(j: &[Vec<f64>], r: &[f64]) -> (Vec<Vec<f64>>, Vec<f64>) {
    let k = j.first().map_or(0, Vec::len);
    let mut a = vec![vec![0.0; k]; k];
    let mut g = vec![0.0; k];
    for (row, v) in j.iter().zip(r) {
        for i in 0..k {
            g[i] += row[i] * v;
            for l in 0..k {
                a[i][l] += row[i] * row[l];
            }
        }
    }
    (a, g)
}

/// The fitted values, standard errors (sigma units) and whether each is a forecast, over
/// every case of the series from where the model can start.
pub(crate) struct Prediction {
    pub(crate) fit: Values,
    pub(crate) se: Values,
}

/// The stopping rules of the Marquardt iterations.
pub(crate) struct Controls {
    pub(crate) mxiter: usize,
    pub(crate) mxlamb: f64,
    pub(crate) ssqpct: f64,
    pub(crate) pareps: f64,
}

/// The result of exact maximum likelihood estimation.
pub(crate) struct Estimation {
    pub(crate) theta: Vec<f64>,
    history: Vec<Step>,
    reason: String,
    pub(crate) ssq_adj: f64,
    pub(crate) ssq_plain: f64,
    /// The valid values after the first d + s sd.
    pub(crate) n: usize,
    pub(crate) covariance: Option<Vec<Vec<f64>>>,
}

/// Estimates a model from `theta` by Marquardt steps on SSQ', over the cases from `start`
/// to the end of `y_used`.
pub(crate) fn estimate(
    st: &Structure,
    y_used: &[Option<f64>],
    x_used: &[Values],
    start: usize,
    mut theta: Vec<f64>,
    c: &Controls,
) -> Result<Estimation, &'static str> {
    let used_len = y_used.len();
    let k = theta.len();
    let (mxiter, mxlamb, ssqpct, pareps) = (c.mxiter, c.mxlamb, c.ssqpct, c.pareps);
    let residuals = |t: &[f64]| -> Option<Vec<f64>> {
        scaled_residuals(st, t, y_used, x_used, start, used_len).map(|r| r.0)
    };
    let ssq_of = |t: &[f64]| residuals(t).map(|r| r.iter().map(|v| v * v).sum::<f64>());
    let Some(mut ssq) = ssq_of(&theta) else {
        return Err("the initial values give a nonstationary model; give other AR or SAR values");
    };
    let mut lambda = 0.001;
    let mut history = vec![Step {
        ssq,
        lambda,
        theta: theta.clone(),
    }];
    let mut reason = if mxiter == 0 {
        "MXITER=0: the initial values are the final estimates".to_string()
    } else {
        format!("the maximum of {mxiter} iterations was reached")
    };
    'iterate: for _ in 0..mxiter {
        let Some(r) = residuals(&theta) else { break };
        let Some(j) = jacobian(&residuals, &theta, r.len()) else {
            break;
        };
        let (a, g) = cross(&j, &r);
        loop {
            let mut damped = a.clone();
            for i in 0..k {
                damped[i][i] += lambda * a[i][i].max(1e-12);
            }
            let step = solve(&damped, &g.iter().map(|v| -v).collect::<Vec<_>>());
            let candidate: Option<(Vec<f64>, f64)> = step.and_then(|s| {
                let t: Vec<f64> = theta.iter().zip(&s).map(|(a, b)| a + b).collect();
                let new = ssq_of(&t)?;
                (new < ssq).then_some((t, new))
            });
            match candidate {
                Some((t, new)) => {
                    let change = theta
                        .iter()
                        .zip(&t)
                        .map(|(a, b)| (a - b).abs())
                        .fold(0.0, f64::max);
                    let relative = (ssq - new) / ssq * 100.0;
                    theta = t;
                    ssq = new;
                    lambda /= 10.0;
                    history.push(Step {
                        ssq,
                        lambda,
                        theta: theta.clone(),
                    });
                    if change < pareps {
                        reason = format!("no estimate changed by {pareps} or more (PAREPS)");
                        break 'iterate;
                    }
                    if relative < ssqpct {
                        reason = format!(
                            "the adjusted sum of squares changed by less than {ssqpct} percent (SSQPCT)"
                        );
                        break 'iterate;
                    }
                    break;
                }
                None => {
                    lambda *= 10.0;
                    if lambda > mxlamb {
                        reason = format!("the Marquardt constant passed {mxlamb:e} (MXLAMB)");
                        break 'iterate;
                    }
                }
            }
        }
    }
    let (r, ssq_plain, n) = scaled_residuals(st, &theta, y_used, x_used, start, used_len)
        .ok_or("the estimates give no valid model")?;
    let ssq_adj: f64 = r.iter().map(|v| v * v).sum();
    // the observed information: second derivatives of minus the exact log-likelihood
    // with the variance profiled out, (N / 2) ln SSQ'
    let covariance: Option<Vec<Vec<f64>>> = match k {
        0 => Some(Vec::new()),
        _ => {
            let half_n = n as f64 / 2.0;
            let f = |t: &[f64]| ssq_of(t).map(|v| half_n * v.ln());
            hessian(&f, &theta).and_then(|h| {
                let (inv, _) = inverse(&h)?;
                (0..k).all(|i| inv[i][i] > 0.0).then_some(inv)
            })
        }
    };
    Ok(Estimation {
        theta,
        history,
        reason,
        ssq_adj,
        ssq_plain,
        n,
        covariance,
    })
}

/// EXACT: the one-step predictions and their standard errors (sigma units) of every case
/// from `start`, forecasts from `origin` on, with the regressors added back.
pub(crate) fn exact_prediction(
    st: &Structure,
    theta: &[f64],
    y: &[Option<f64>],
    x: &[Values],
    start: usize,
    origin: usize,
) -> Option<Prediction> {
    let z = adjusted(st, theta, y, x);
    let filtered = kalman(st, theta, &z, start, origin)?;
    let mut fit = vec![None; y.len()];
    let mut se = vec![None; y.len()];
    let [_, _, _, _, reg, _] = st.split(theta);
    for (t, pred, f, _) in filtered.steps {
        let mut v = pred;
        for (c, column) in reg.iter().zip(x) {
            match column[t] {
                Some(xv) => v += c * xv,
                None => {
                    v = f64::NAN;
                    break;
                }
            }
        }
        if v.is_finite() {
            fit[t] = Some(v);
            se[t] = Some(f.sqrt());
        }
    }
    Some(Prediction { fit, se })
}

impl Engine {
    /// The command over the previous successful ARIMA for APPLY.
    pub(crate) fn arima_spec(&mut self, command: &Arima) -> Result<Arima, Error> {
        let given = match &command.apply {
            None => command.clone(),
            Some((Some(model), _)) => {
                return Err(Error(format!(
                    "ARIMA APPLY of the named model '{model}' is not supported yet; APPLY \
                     without a name uses the previous ARIMA"
                )));
            }
            Some((None, how)) => {
                let Some(previous) = &self.ts_models.arima else {
                    return Err(Error(
                        "ARIMA APPLY: there is no earlier ARIMA to apply".into(),
                    ));
                };
                let mut given = previous.spec.clone();
                // the initial values come anew unless INITIAL or FIT carry the estimates
                (
                    given.ar, given.ma, given.sar, given.sma, given.reg, given.con,
                ) = (None, None, None, None, None, None);
                if *how != ArimaApply::Specifications {
                    let Some(e) = &previous.estimates else {
                        return Err(Error(
                            "ARIMA APPLY: the earlier ARIMA has no estimates to apply".into(),
                        ));
                    };
                    given.ar = Some(e.ar.clone());
                    given.ma = Some(e.ma.clone());
                    given.sar = Some(e.sar.clone());
                    given.sma = Some(e.sma.clone());
                    given.reg = Some(e.reg.clone());
                    given.con = e.con;
                }
                if *how == ArimaApply::Fit {
                    given.mxiter = Some(0);
                }
                if !command.dependent.is_empty() {
                    given.dependent.clone_from(&command.dependent);
                    given.regressors.clone_from(&command.regressors);
                }
                if command.orders.is_some() {
                    given.orders = command.orders;
                    given.seasonal_orders = command.seasonal_orders;
                    given.period = command.period;
                }
                macro_rules! over {
                    ($($field:ident),*) => {
                        $(if command.$field.is_some() {
                            given.$field = command.$field.clone();
                        })*
                    };
                }
                over!(
                    constant, log, p, q, sp, sq, d, sd, ar, ma, sar, sma, reg, con, mxiter, mxlamb,
                    ssqpct, pareps, cinpct, forecast
                );
                given
            }
        };
        let mut given = given;
        given.apply = None;
        Ok(given)
    }

    pub(crate) fn arima(&mut self, spec: &Arima) -> Result<Vec<Table>, Error> {
        let (tables, estimates) = self.arima_model(spec, "ARIMA")?;
        // a model APPLY can reuse, kept only when the run succeeds
        self.ts_models.arima = Some(Previous {
            spec: spec.clone(),
            estimates: Some(estimates),
        });
        Ok(tables)
    }

    /// Fits an ARIMA model for `what` (ARIMA, or AREG with METHOD=ML) and saves its series.
    pub(crate) fn arima_model(
        &mut self,
        spec: &Arima,
        what: &str,
    ) -> Result<(Vec<Table>, Estimates), Error> {
        let data = self.active()?;
        let shown = self.shown();
        let dependent = self.expand(&spec.dependent)?;
        let [dependent] = dependent.as_slice() else {
            return Err(Error(format!("{what} takes one dependent series")));
        };
        let regressors = self.expand(&spec.regressors)?;
        let include = self.tset.include;
        let print = self.tset.print;
        let constant = spec.constant.unwrap_or(self.tset.constant);
        let level = spec.cinpct.unwrap_or(self.tset.cin);
        let log = spec.log.unwrap_or(ArimaLog::None);
        let method = spec.forecast.unwrap_or(ArimaForecast::Exact);
        let [p0, d0, q0] = spec.orders.unwrap_or([0; 3]);
        let [sp0, sd0, sq0] = spec.seasonal_orders.unwrap_or([0; 3]);
        let lags = |given: &Option<Vec<u32>>, order: u32| -> Vec<usize> {
            match given {
                Some(l) => l.iter().map(|&v| v as usize).collect(),
                None => (1..=order as usize).collect(),
            }
        };
        let mut st = Structure {
            ar: lags(&spec.p, p0),
            ma: lags(&spec.q, q0),
            sar: lags(&spec.sp, sp0),
            sma: lags(&spec.sq, sq0),
            d: spec.d.unwrap_or(d0) as usize,
            sd: spec.sd.unwrap_or(sd0) as usize,
            s: 1,
            regressors: regressors.len(),
            constant,
        };
        if !st.sar.is_empty() || !st.sma.is_empty() || st.sd > 0 {
            let Some(s) = self.ts_period(spec.period) else {
                return Err(Error(format!(
                    "{what}: a seasonal model needs a period: give it after the seasonal orders \
                     on MODEL, with TSET PERIOD or with DATE"
                )));
            };
            if s < 2 {
                return Err(Error(format!(
                    "{what}: the seasonal period must be at least 2"
                )));
            }
            st.s = s;
        }
        let periods = self.periods(what)?;
        // the series over the cases from the start of USE to the end of the forecasts
        let window = periods.first..=periods.end.max(periods.last);
        let transform = |v: f64| match log {
            ArimaLog::None => Ok(v),
            _ if v <= 0.0 => Err(Error(format!(
                "{what}: {dependent} has values not above 0, which cannot be logged"
            ))),
            ArimaLog::Ln => Ok(v.ln()),
            ArimaLog::Log10 => Ok(v.log10()),
        };
        let (all, _, _) = read_series(data, dependent, include, what)?;
        let y: Values = all[window.clone()]
            .iter()
            .map(|v| v.map(transform).transpose())
            .collect::<Result<_, _>>()?;
        let mut x = Vec::new();
        for name in &regressors {
            let (column, _, _) = read_series(data, name, include, what)?;
            x.push(column[window.clone()].to_vec());
        }
        let used_len = periods.last + 1 - periods.first;
        let origin = periods.origin - periods.first;
        // the model starts at the first valid value
        let Some(start) = y[..used_len].iter().position(Option::is_some) else {
            return Err(Error(format!("{what}: {dependent} has no valid values")));
        };
        let reach = st.reach();
        if (start..start + reach).any(|t| t >= used_len || y[t].is_none()) {
            return Err(Error(format!(
                "{what}: the first {reach} values of {dependent} from its first valid one must be \
                 valid, as the differencing starts from them"
            )));
        }
        if let Some(name) = regressors
            .iter()
            .zip(&x)
            .find(|(_, c)| (start..start + reach).any(|t| c[t].is_none()))
            .map(|(name, _)| name)
        {
            return Err(Error(format!(
                "{what}: the regressor {name} must be valid at the first {reach} values of \
                 {dependent}, as the differencing starts from them"
            )));
        }
        let given = Estimates {
            ar: spec.ar.clone().unwrap_or_default(),
            ma: spec.ma.clone().unwrap_or_default(),
            sar: spec.sar.clone().unwrap_or_default(),
            sma: spec.sma.clone().unwrap_or_default(),
            reg: spec.reg.clone().unwrap_or_default(),
            con: spec.con,
        };
        let check_count = |name: &str, values: &[f64], wanted: usize| {
            if !values.is_empty() && values.len() != wanted {
                return Err(Error(format!(
                    "{what}: {name} gives {} initial values for {wanted} parameters",
                    values.len()
                )));
            }
            Ok(())
        };
        check_count("AR", &given.ar, st.ar.len())?;
        check_count("MA", &given.ma, st.ma.len())?;
        check_count("SAR", &given.sar, st.sar.len())?;
        check_count("SMA", &given.sma, st.sma.len())?;
        check_count("REG", &given.reg, st.regressors)?;
        let y_used: Values = y[..used_len].to_vec();
        let x_used: Vec<Values> = x.iter().map(|c| c[..used_len].to_vec()).collect();
        let theta0 = initial_values(&st, &y_used, &x_used, &given);
        let mut warnings = Vec::new();
        let controls = Controls {
            mxiter: spec.mxiter.unwrap_or(10) as usize,
            mxlamb: spec.mxlamb.unwrap_or(1e9),
            ssqpct: spec.ssqpct.unwrap_or(0.001),
            pareps: spec.pareps.unwrap_or(self.tset.cnverge),
        };
        let Estimation {
            theta,
            history,
            reason,
            ssq_adj,
            ssq_plain,
            n,
            covariance,
        } = estimate(&st, &y_used, &x_used, start, theta0, &controls)
            .map_err(|e| Error(format!("{what}: {e}")))?;
        let k = theta.len();
        let df = n as f64 - k as f64;
        if df <= 0.0 {
            return Err(Error(format!(
                "{what}: {n} values are too few for {k} parameters"
            )));
        }
        let variance = ssq_plain / df;
        let sigma = variance.sqrt();
        let loglik = -(n as f64) * sigma.ln()
            - ssq_adj / (2.0 * variance)
            - n as f64 * (2.0 * std::f64::consts::PI).ln() / 2.0;
        let aic = -2.0 * loglik + 2.0 * k as f64;
        let sbc = -2.0 * loglik + (n as f64).ln() * k as f64;
        if covariance.is_none() {
            warnings.push(format!(
                "{what}: the estimates' covariance is singular, so no standard errors are given"
            ));
        }

        // names of the parameters
        let mut labels: Vec<String> = Vec::new();
        for (prefix, lags) in [
            ("AR", &st.ar),
            ("MA", &st.ma),
            ("SAR", &st.sar),
            ("SMA", &st.sma),
        ] {
            // AREG calls its AR1 rho
            labels.extend(lags.iter().map(|l| match (what, prefix) {
                ("AREG", "AR") => "Rho (AR1)".to_string(),
                _ => format!("{prefix}{l}"),
            }));
        }
        labels.extend(regressors.iter().map(|r| shown.named(data, r)));
        if st.constant {
            labels.push("Constant".into());
        }
        let label = shown.named(data, dependent);

        // tables
        let order_text = |lags: &[usize]| match lags {
            [] => "none".to_string(),
            l if l.iter().copied().eq(1..=l.len()) => format!("order {}", l.len()),
            l => format!(
                "lags {}",
                l.iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        let mut model_rows = vec![
            vec![text("Dependent series"), text(label.clone())],
            vec![
                text("Regressors"),
                text(if regressors.is_empty() {
                    "None".to_string()
                } else {
                    regressors
                        .iter()
                        .map(|r| shown.named(data, r))
                        .collect::<Vec<_>>()
                        .join(", ")
                }),
            ],
            vec![text("Autoregressive"), text(order_text(&st.ar))],
            vec![text("Differencing"), Cell::Number(st.d as f64)],
            vec![text("Moving average"), text(order_text(&st.ma))],
        ];
        if st.s > 1 {
            model_rows.extend([
                vec![text("Seasonal autoregressive"), text(order_text(&st.sar))],
                vec![text("Seasonal differencing"), Cell::Number(st.sd as f64)],
                vec![text("Seasonal moving average"), text(order_text(&st.sma))],
                vec![text("Period"), Cell::Number(st.s as f64)],
            ]);
        }
        model_rows.extend([
            vec![
                text("Constant"),
                text(if st.constant {
                    "Included"
                } else {
                    "Not included"
                }),
            ],
            vec![
                text("Transformation"),
                text(match log {
                    ArimaLog::None => "None",
                    ArimaLog::Ln => "Natural logarithm",
                    ArimaLog::Log10 => "Base 10 logarithm",
                }),
            ],
            vec![
                text("Forecasting method"),
                text(match method {
                    ArimaForecast::Exact => "Unconditional least squares (EXACT)",
                    ArimaForecast::Cls => "Conditional least squares (CLS)",
                    ArimaForecast::Autoinit => {
                        "Conditional least squares from the first values (AUTOINIT)"
                    }
                }),
            ],
            vec![
                text("Cases used"),
                text(format!("{} to {}", periods.first + 1, periods.last + 1)),
            ],
        ]);
        let mut tables = vec![table(
            "Model Description",
            1,
            plain(&["", "Specification"]),
            model_rows,
            None,
        )];
        if print != TsetPrint::Brief {
            let mut columns = vec!["Iteration", "Adjusted Sum of Squares", "Marquardt Constant"];
            let detailed = print == TsetPrint::Detailed;
            if detailed {
                columns.extend(labels.iter().map(String::as_str));
            }
            let rows = history
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let mut row = vec![
                        Cell::Number(i as f64),
                        number(Some(s.ssq)),
                        number(Some(s.lambda)),
                    ];
                    if detailed {
                        row.extend(s.theta.iter().map(|v| number(Some(*v))));
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
        }
        let rows = labels
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let se = covariance.as_ref().map(|c| c[i][i].sqrt());
                let t = se.map(|s| theta[i] / s);
                vec![
                    text(name.clone()),
                    number(Some(theta[i])),
                    number(se),
                    number(t),
                    number(t.map(|t| t_p(t, df))),
                ]
            })
            .collect();
        tables.push(table(
            "Parameter Estimates",
            1,
            plain(&["", "Estimate", "Std. Error", "t", "Approx Sig."]),
            rows,
            Some(format!(
                "Series: {label}. Moving-average coefficients enter as 1 - c B, so a positive \
                 estimate weighs the past error negatively."
            )),
        ));
        tables.push(table(
            "Model Statistics",
            1,
            plain(&["", "Value"]),
            vec![
                vec![text("Number of residuals"), Cell::Number(n as f64)],
                vec![text("Number of parameters"), Cell::Number(k as f64)],
                vec![text("Residual df"), Cell::Number(df)],
                vec![
                    text("Adjusted residual sum of squares"),
                    number(Some(ssq_adj)),
                ],
                vec![text("Residual sum of squares"), number(Some(ssq_plain))],
                vec![text("Residual variance"), number(Some(variance))],
                vec![text("Standard error"), number(Some(sigma))],
                vec![text("Log-likelihood"), number(Some(loglik))],
                vec![
                    text("Akaike's information criterion (AIC)"),
                    number(Some(aic)),
                ],
                vec![
                    text("Schwarz's Bayesian criterion (SBC)"),
                    number(Some(sbc)),
                ],
            ],
            Some(format!("Series: {label}.")),
        ));
        if print != TsetPrint::Brief
            && let Some(cov) = &covariance
            && k > 0
        {
            let mut columns = vec![""];
            columns.extend(labels.iter().map(String::as_str));
            let matrix = |f: &dyn Fn(usize, usize) -> f64| -> Vec<Vec<Cell>> {
                (0..k)
                    .map(|i| {
                        let mut row = vec![text(labels[i].clone())];
                        row.extend((0..k).map(|j| number(Some(f(i, j)))));
                        row
                    })
                    .collect()
            };
            tables.push(table(
                "Covariance Matrix",
                1,
                plain(&columns),
                matrix(&|i, j| cov[i][j]),
                Some(format!("Series: {label}.")),
            ));
            tables.push(table(
                "Correlation Matrix",
                1,
                plain(&columns),
                matrix(&|i, j| cov[i][j] / (cov[i][i] * cov[j][j]).sqrt()),
                Some(format!("Series: {label}.")),
            ));
        }

        // fitted values, forecasts and their standard errors (sigma units)
        let prediction = match method {
            ArimaForecast::Exact => exact_prediction(&st, &theta, &y, &x, start, origin)
                .ok_or_else(|| Error(format!("{what}: the estimates give no valid model")))?,
            _ => conditional(&st, &theta, &y, &x, start, origin, method),
        };
        let t_crit = t_quantile((1.0 - level / 100.0) / 2.0, df);
        let back = |v: f64| match log {
            ArimaLog::None => v,
            ArimaLog::Ln => v.exp(),
            ArimaLog::Log10 => 10f64.powf(v),
        };
        let slope = |v: f64| match log {
            ArimaLog::None => 1.0,
            ArimaLog::Ln => v.exp(),
            ArimaLog::Log10 => 10f64.powf(v) * std::f64::consts::LN_10,
        };
        let cases = data.case_count();
        let mut columns: Vec<Values> = vec![vec![None; cases]; 5];
        for (t, &yt) in y.iter().enumerate() {
            let case = periods.first + t;
            // forecasts stop at the end of the PREDICT period
            if t >= origin && case > periods.end {
                break;
            }
            let (Some(f), Some(s)) = (prediction.fit[t], prediction.se[t]) else {
                continue;
            };
            let sep = s * sigma;
            columns[0][case] = Some(back(f));
            if t < origin {
                columns[1][case] = yt.map(|v| v - f);
            }
            columns[2][case] = Some(back(f - t_crit * sep));
            columns[3][case] = Some(back(f + t_crit * sep));
            columns[4][case] = Some(sep * slope(f));
        }
        let name_model = if what == "AREG" {
            "AREG exact maximum likelihood".to_string()
        } else {
            let mut m = format!("ARIMA ({},{},{})", st.ar.len(), st.d, st.ma.len());
            if st.s > 1 {
                m.push_str(&format!(
                    "({},{},{}) {}",
                    st.sar.len(),
                    st.sd,
                    st.sma.len(),
                    st.s
                ));
            }
            if st.constant {
                m.push_str(" with constant");
            }
            m
        };
        let item_labels = vec![
            format!("Fit of {dependent}, {name_model}"),
            format!("Error of {dependent}, {name_model}"),
            format!("{level}% lower limit of {dependent}, {name_model}"),
            format!("{level}% upper limit of {dependent}, {name_model}"),
            format!("Standard error of the fit of {dependent}, {name_model}"),
        ];
        for w in warnings {
            self.warn(w);
        }
        self.save_series(
            what,
            &["FIT", "ERR", "LCL", "UCL", "SEP"],
            vec![(item_labels, columns)],
        )?;
        let [ar, ma, sar, sma, reg, con] = st.split(&theta);
        Ok((
            tables,
            Estimates {
                ar: ar.to_vec(),
                ma: ma.to_vec(),
                sar: sar.to_vec(),
                sma: sma.to_vec(),
                reg: reg.to_vec(),
                con: con.first().copied(),
            },
        ))
    }
}

/// CLS and AUTOINIT: fitted values by the conditional recursion, forecasts after `origin`.
fn conditional(
    st: &Structure,
    theta: &[f64],
    y: &[Option<f64>],
    x: &[Vec<Option<f64>>],
    start: usize,
    origin: usize,
    method: ArimaForecast,
) -> Prediction {
    let (phi, c) = arma(st, theta);
    let mu = if st.constant {
        theta[theta.len() - 1]
    } else {
        0.0
    };
    let [_, _, _, _, reg, _] = st.split(theta);
    let delta = st.difference();
    let reach = st.reach();
    let z = adjusted(st, theta, y, x);
    let len = y.len();
    let regression = |t: usize| -> Option<f64> {
        let mut v = 0.0;
        for (coef, column) in reg.iter().zip(x) {
            v += coef * column[t]?;
        }
        Some(v)
    };
    let mut fit: Values = vec![None; len];
    let mut se: Values = vec![None; len];
    // the full autoregressive polynomial with the differencing, for the psi weights
    let ar_poly: Vec<f64> = std::iter::once(1.0).chain(phi.iter().map(|v| -v)).collect();
    let full = product(&ar_poly, &delta);
    let full_phi: Vec<f64> = full[1..].iter().map(|v| -v).collect();
    let psi = psi_weights(&full_phi, &c, len + 1);
    // the series as known: observed before the origin, else its fit
    let mut known: Values = (0..len)
        .map(|t| if t < origin { z[t] } else { None })
        .collect();
    let mut errors = vec![0.0; len];
    match method {
        ArimaForecast::Cls => {
            // on the differenced series, unknown values at the mean
            let mut w: Values = vec![None; len];
            let mut w_hat = vec![0.0; len];
            for t in start + reach..len {
                let mut v = mu;
                for (k, p) in phi.iter().enumerate() {
                    let past = t.checked_sub(k + 1).filter(|&s| s >= start + reach);
                    let dev = past.and_then(|s| w[s]).map_or(0.0, |ws| ws - mu);
                    v += p * dev;
                }
                for (j, cj) in c.iter().enumerate() {
                    if let Some(s) = t.checked_sub(j + 1) {
                        v += cj * errors[s];
                    }
                }
                w_hat[t] = v;
                // z_t = w_t + sum delta_k z_(t-k)
                let lagged: Option<f64> = (1..delta.len())
                    .map(|k| known[t - k].map(|zv| -delta[k] * zv))
                    .sum();
                let Some(lagged) = lagged else { continue };
                let z_hat = v + lagged;
                match known[t] {
                    Some(observed) => {
                        let actual = observed - lagged;
                        errors[t] = actual - v;
                        w[t] = Some(actual);
                    }
                    None => {
                        w[t] = Some(v);
                        known[t] = Some(z_hat);
                    }
                }
                if let Some(r) = regression(t) {
                    fit[t] = Some(z_hat + r);
                }
            }
        }
        _ => {
            // on the series itself, from the first d + p + s (sd + sp) values
            let begin = start + full_phi.len();
            let constant = mu * (1.0 - phi.iter().sum::<f64>());
            for t in begin..len {
                let mut v = constant;
                let mut complete = true;
                for (k, p) in full_phi.iter().enumerate() {
                    match known[t - k - 1] {
                        Some(zv) => v += p * zv,
                        None if *p == 0.0 => {}
                        None => complete = false,
                    }
                }
                for (j, cj) in c.iter().enumerate() {
                    if let Some(s) = t.checked_sub(j + 1) {
                        v += cj * errors[s];
                    }
                }
                if !complete {
                    continue;
                }
                match known[t] {
                    Some(observed) => errors[t] = observed - v,
                    None => known[t] = Some(v),
                }
                if let Some(r) = regression(t) {
                    fit[t] = Some(v + r);
                }
            }
        }
    }
    for t in 0..len {
        if fit[t].is_some() {
            let h = if t < origin { 1 } else { t + 1 - origin };
            se[t] = Some(psi[..h].iter().map(|p| p * p).sum::<f64>().sqrt());
        }
    }
    Prediction { fit, se }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stationarity_of_ar_polynomials() {
        assert!(stationary(&[0.5]));
        assert!(!stationary(&[1.1]));
        assert!(stationary(&[0.5, 0.3]));
        assert!(!stationary(&[0.5, 0.6]));
    }

    #[test]
    fn autocovariances_of_an_arma_1_1() {
        // x_t = 0.5 x_(t-1) + a_t + 0.4 a_(t-1): gamma(0) = (1 + 2 phi c + c^2) / (1 - phi^2)
        let g = autocovariances(&[0.5], &[0.4], 3).unwrap();
        let g0 = (1.0 + 2.0 * 0.5 * 0.4 + 0.16) / 0.75;
        assert!((g[0] - g0).abs() < 1e-12);
        let g1 = 0.5 * g0 + 0.4;
        assert!((g[1] - g1).abs() < 1e-12);
        assert!((g[2] - 0.5 * g1).abs() < 1e-12);
    }

    #[test]
    fn state_covariance_is_stationary() {
        // P = T P T' + R R' for an ARMA(2, 2)
        let (phi, c) = (vec![0.6, -0.2], vec![0.3, 0.25]);
        let r = 3;
        let p = state_covariance(&phi, &c, r).unwrap();
        let t = |i: usize, j: usize| -> f64 {
            let mut v = if j == 0 {
                phi.get(i).copied().unwrap_or(0.0)
            } else {
                0.0
            };
            if j == i + 1 {
                v += 1.0;
            }
            v
        };
        let rv = [1.0, 0.3, 0.25];
        for i in 0..r {
            for j in 0..r {
                let mut v = rv[i] * rv[j];
                for (k, row) in p.iter().enumerate() {
                    for (l, pkl) in row.iter().enumerate() {
                        v += t(i, k) * pkl * t(j, l);
                    }
                }
                assert!((v - p[i][j]).abs() < 1e-10, "{i} {j}: {v} {}", p[i][j]);
            }
        }
    }

    #[test]
    fn initial_ma_of_a_known_autocorrelation() {
        // an MA(1) with theta 0.5 (1 - 0.5 B) has rho_1 = -0.4
        let (_, theta) = initial_arma(&[-0.4], 0, 1);
        assert!((theta[0] - 0.4 / 0.84).abs() < 1e-12);
    }
}
