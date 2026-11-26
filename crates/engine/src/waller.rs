//! The Bayesian t value of the Waller-Duncan test.
//!
//! The rule (Waller and Duncan 1969) compares a mean difference with `t_B s sqrt(2/n)`, s^2 the
//! error mean square and n the common group size. `t_B` minimises the Bayes risk of the additive
//! loss over the pairs: for the difference d of two means, deciding "no difference" costs `d` when
//! d > 0, deciding "greater" costs `-k d` when d <= 0, k the ratio of the seriousness of a Type 1
//! to a Type 2 error. The means have a normal prior with an unknown variance and the variances
//! (sigma^2 of the cases, eta = sigma^2 + n sigma_mu^2 of the means) have the prior 1 / (sigma^2
//! eta) with eta > sigma^2.
//!
//! Worked out from that, with u = sigma^2 / eta, F the F ratio of the one-way analysis, q the
//! number of groups less 1 and f the error df: u has the density u^(q/2 - 1) (f + q F u)^(-(f + q) / 2) on (0, 1),
//! and the standardised difference, given u, is a t variable with f + q df, location (1 - u) t and
//! scale sqrt((1 - u)(f + q F u) / (f + q)), t the observed difference over s sqrt(2/n). The
//! Bayes rule says "different" when the posterior mean of d+ is above k times that of d-, the
//! averages over u taken by Gauss-Legendre rules, and `t_B` is the t where the two are equal.
//! The expected value of the positive part of a shifted and scaled t variable is closed form.
//!
//! Checked against the published values: the printed critical value of a worked example (error
//! df 12, 7 groups, F 10.4495, k 100: 2.12034, here 2.120344) and the 8 by 14 table of t_B for
//! k = 100, F = 1.2 with q from 8 to 100 and f from 4 to 120 (112 values to two decimals, the
//! test lists them), the table as printed in the literature (Steel and Torrie) and quoted in the
//! documentation of an R package; no program that computes it was run to compare with.

use crate::dist::{ln_gamma, solve, t_p};
use std::f64::consts::PI;

/// The nodes and weights of the n-point Gauss-Legendre rule on [-1, 1].
fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    let mut rule = Vec::with_capacity(n);
    for i in 0..n {
        let mut x = (PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
        let mut derivative = 1.0;
        for _ in 0..100 {
            let (mut p0, mut p1) = (1.0, x);
            for k in 2..=n {
                let kf = k as f64;
                (p0, p1) = (p1, ((2.0 * kf - 1.0) * x * p1 - (kf - 1.0) * p0) / kf);
            }
            derivative = n as f64 * (x * p1 - p0) / (x * x - 1.0);
            let step = p1 / derivative;
            x -= step;
            if step.abs() < 1e-15 {
                break;
            }
        }
        rule.push((x, 2.0 / ((1.0 - x * x) * derivative * derivative)));
    }
    rule
}

/// The density of Student's t with `nu` df.
fn t_density(z: f64, nu: f64) -> f64 {
    (ln_gamma((nu + 1.0) / 2.0) - ln_gamma(nu / 2.0)).exp() / (nu * PI).sqrt()
        * (1.0 + z * z / nu).powf(-(nu + 1.0) / 2.0)
}

/// The distribution function of Student's t with `nu` df.
fn t_cdf(z: f64, nu: f64) -> f64 {
    let both = t_p(z.abs(), nu);
    if z < 0.0 {
        both / 2.0
    } else {
        1.0 - both / 2.0
    }
}

/// The Bayesian t value for the k-ratio `k`, the F ratio `f_ratio` of the one-way analysis, its
/// numerator df `q` (groups - 1) and the error df `f`.
pub(crate) fn waller_t(k: f64, f_ratio: f64, q: f64, f: f64) -> f64 {
    let nu = f + q;
    let rule = gauss_legendre(24);
    // the integration points: u, the weight (in logs) and the rule weight, over two ranges that
    // make the integrand analytic: u = v^2 up to 1/2 (the factor u^(q/2 - 1) becomes v^(q - 2),
    // with du = 2 v dv) and u = 1 - s^2 above it. The two parts nearly cancel at the root, so
    // each has to be exact to many digits.
    let mut nodes: Vec<(f64, f64, f64)> = Vec::new();
    let log_base = |u: f64| -nu / 2.0 * (f + q * f_ratio * u).ln();
    let panels = 8;
    let side = 0.5f64.sqrt();
    for (near_zero, p) in (0..panels).flat_map(|p| [(true, p), (false, p)]) {
        let (a, b) = (
            side * p as f64 / panels as f64,
            side * (p + 1) as f64 / panels as f64,
        );
        for &(x, weight) in &rule {
            let s = (a + b) / 2.0 + (b - a) / 2.0 * x;
            let w = weight * (b - a) / 2.0;
            let (u, log_weight) = if near_zero {
                let u = s * s;
                (u, 2.0f64.ln() + (q - 1.0) * s.ln() + log_base(u))
            } else {
                let u = 1.0 - s * s;
                (u, (q / 2.0 - 1.0) * u.ln() + (2.0 * s).ln() + log_base(u))
            };
            if u > 0.0 && u < 1.0 && log_weight.is_finite() {
                nodes.push((u, log_weight, w));
            }
        }
    }
    let reference = nodes.iter().map(|n| n.1).fold(f64::NEG_INFINITY, f64::max);
    // the posterior expected loss of "different" less that of "no difference", up to a factor
    let gain = |t: f64| -> f64 {
        nodes
            .iter()
            .map(|&(u, log_weight, w)| {
                let scale = ((1.0 - u) * (f + q * f_ratio * u) / nu).sqrt();
                let z = (1.0 - u) * t / scale;
                let tail = (nu + z * z) / (nu - 1.0) * t_density(z, nu);
                let value = scale * (z * (t_cdf(z, nu) + k * t_cdf(-z, nu)) + (1.0 - k) * tail);
                w * (log_weight - reference).exp() * value
            })
            .sum()
    };
    solve(gain, 0.0, 60.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_integrates_a_polynomial_exactly() {
        let integral: f64 = gauss_legendre(20)
            .iter()
            .map(|&(x, w)| w * (x.powi(6) + 3.0 * x * x))
            .sum();
        assert!((integral - (2.0 / 7.0 + 2.0)).abs() < 1e-13);
    }

    #[test]
    fn the_printed_critical_value_of_a_worked_example() {
        // error mean square 1.6452381, treatment mean square 17.1919048 (6 df), 12 error df
        let f = 17.1919048 / 1.6452381;
        let t = waller_t(100.0, f, 6.0, 12.0);
        assert!((t - 2.12034).abs() < 2e-5, "{t}");
        // 4 groups, error df 8, F 17.34478: 2.236 printed
        let t = waller_t(100.0, 17.34478, 3.0, 8.0);
        assert!((t - 2.236).abs() < 5e-4, "{t}");
    }

    #[test]
    fn the_published_table_for_k_100_and_f_1_2() {
        let q = [8.0, 10.0, 12.0, 14.0, 16.0, 20.0, 40.0, 100.0];
        let f = [
            4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 30.0, 40.0, 60.0, 120.0,
        ];
        let table = [
            [
                285, 291, 294, 296, 297, 298, 299, 299, 299, 300, 300, 300, 300, 300,
            ],
            [
                285, 293, 298, 301, 304, 305, 306, 307, 308, 309, 310, 310, 311, 312,
            ],
            [
                285, 295, 301, 305, 308, 310, 312, 313, 314, 316, 317, 319, 320, 321,
            ],
            [
                285, 296, 303, 308, 312, 314, 316, 318, 319, 321, 323, 325, 327, 329,
            ],
            [
                285, 297, 305, 311, 315, 318, 320, 322, 324, 326, 329, 331, 334, 336,
            ],
            [
                285, 299, 308, 314, 319, 323, 326, 328, 330, 333, 337, 340, 344, 347,
            ],
            [
                285, 302, 313, 322, 329, 335, 339, 343, 347, 352, 358, 364, 372, 379,
            ],
            [
                285, 304, 317, 328, 336, 344, 350, 355, 359, 367, 376, 386, 398, 411,
            ],
        ];
        let mut rounded_differently = 0;
        for (i, row) in table.iter().enumerate() {
            for (j, &hundredths) in row.iter().enumerate() {
                let printed = f64::from(hundredths) / 100.0;
                let t = waller_t(100.0, 1.2, q[i], f[j]);
                // two decimals; two entries lie within 5E-4 of a rounding edge and the table
                // rounds them the other way (its own integration error is of that size)
                assert!(
                    (t - printed).abs() < 0.0056,
                    "q {} f {}: {t} against {printed}",
                    q[i],
                    f[j]
                );
                if (t * 100.0).round() / 100.0 != printed {
                    rounded_differently += 1;
                }
            }
        }
        assert!(rounded_differently <= 2, "{rounded_differently}");
    }

    #[test]
    fn the_value_falls_as_f_grows_and_rises_with_the_k_ratio() {
        let t = |k: f64, f: f64| waller_t(k, f, 4.0, 20.0);
        assert!(t(100.0, 1.0) > t(100.0, 3.0) && t(100.0, 3.0) > t(100.0, 10.0));
        assert!(t(50.0, 2.0) < t(100.0, 2.0) && t(100.0, 2.0) < t(500.0, 2.0));
    }
}
