//! Distribution functions shared by the procedures: gamma and beta functions, the normal,
//! t, chi-square and F distributions, the studentized range and the noncentral t.

/// The significance of a chi-square value: the probability of a larger one.
pub(crate) fn chi_square_p(value: f64, df: f64) -> f64 {
    gamma_q(df / 2.0, value / 2.0)
}

/// ln Γ(x) for x > 0 (Lanczos, g = 7, nine terms; relative error near 1e-15).
pub(crate) fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        // reflection keeps the series in its accurate range
        return (std::f64::consts::PI / (std::f64::consts::PI * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let t = x + 7.5;
    let sum = G[1..]
        .iter()
        .enumerate()
        .fold(G[0], |sum, (i, g)| sum + g / (x + i as f64 + 1.0));
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + sum.ln()
}

/// The regularized upper incomplete gamma function Q(a, x) = Γ(a, x) / Γ(a): a power series
/// below x = a + 1, a continued fraction above.
pub(crate) fn gamma_q(a: f64, x: f64) -> f64 {
    const EPSILON: f64 = 1e-16;
    const TINY: f64 = 1e-300;
    if x.is_nan() || a <= 0.0 {
        return f64::NAN;
    }
    if x <= 0.0 {
        return 1.0;
    }
    let front = (-x + a * x.ln() - ln_gamma(a)).exp();
    if x < a + 1.0 {
        let (mut term, mut sum, mut ap) = (1.0 / a, 1.0 / a, a);
        for _ in 0..10_000 {
            ap += 1.0;
            term *= x / ap;
            sum += term;
            if term.abs() < sum.abs() * EPSILON {
                break;
            }
        }
        return (1.0 - sum * front).max(0.0);
    }
    // modified Lentz evaluation
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..10_000 {
        let an = -(i as f64) * (i as f64 - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < TINY {
            d = TINY;
        }
        c = b + an / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let step = d * c;
        h *= step;
        if (step - 1.0).abs() < EPSILON {
            break;
        }
    }
    (front * h).min(1.0)
}

/// The complementary error function: erfc(x) = Q(1/2, x^2) for x >= 0.
pub(crate) fn erfc(x: f64) -> f64 {
    let q = gamma_q(0.5, x * x);
    if x < 0.0 { 2.0 - q } else { q }
}

/// Two-sided significance of a standard normal z.
pub(crate) fn normal_p(z: f64) -> f64 {
    erfc(z.abs() / std::f64::consts::SQRT_2)
}

/// Two-sided significance of Student's t with `df` degrees of freedom: I_x(df/2, 1/2) at
/// x = df / (df + t^2).
pub(crate) fn t_p(t: f64, df: f64) -> f64 {
    beta_i(df / 2.0, 0.5, df / (df + t * t))
}

/// The regularized incomplete beta function I_x(a, b), by its continued fraction on the side of
/// x where it converges fast.
pub(crate) fn beta_i(a: f64, b: f64, x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let front =
        (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x < (a + 1.0) / (a + b + 2.0) {
        front * beta_fraction(a, b, x) / a
    } else {
        1.0 - front * beta_fraction(b, a, 1.0 - x) / b
    }
}

/// The continued fraction of I_x(a, b), evaluated by the modified Lentz method.
fn beta_fraction(a: f64, b: f64, x: f64) -> f64 {
    const TINY: f64 = 1e-300;
    let guard = |v: f64| if v.abs() < TINY { TINY } else { v };
    let mut c = 1.0;
    let mut d = 1.0 / guard(1.0 - (a + b) * x / (a + 1.0));
    let mut h = d;
    for m in 1..100_000 {
        let m = m as f64;
        let even = m * (b - m) * x / ((a + 2.0 * m - 1.0) * (a + 2.0 * m));
        d = 1.0 / guard(1.0 + even * d);
        c = guard(1.0 + even / c);
        h *= d * c;
        let odd = -(a + m) * (a + b + m) * x / ((a + 2.0 * m) * (a + 2.0 * m + 1.0));
        d = 1.0 / guard(1.0 + odd * d);
        c = guard(1.0 + odd / c);
        let step = d * c;
        h *= step;
        if (step - 1.0).abs() < 1e-15 {
            break;
        }
    }
    h
}

/// The standard normal distribution function by Hart's rational approximation (as given by
/// West, 2005), accurate to about 1e-15 and much faster than `erfc`; the studentized range
/// needs it many thousand times.
pub(crate) fn normal_cdf(x: f64) -> f64 {
    let z = x.abs();
    let tail = if z > 37.0 {
        0.0
    } else if z < 7.071_067_811_865_47 {
        let top = [
            3.526_249_659_989_11e-2,
            0.700_383_064_443_688,
            6.373_962_203_531_65,
            33.912_866_078_383,
            112.079_291_497_871,
            221.213_596_169_931,
            220.206_867_912_376,
        ];
        let bottom = [
            8.838_834_764_831_84e-2,
            1.755_667_163_182_64,
            16.064_177_579_207,
            86.780_732_202_946_1,
            296.564_248_779_674,
            637.333_633_378_831,
            793.826_512_519_948,
            440.413_735_824_752,
        ];
        let horner = |c: &[f64]| c.iter().fold(0.0, |acc, c| acc * z + c);
        (-z * z / 2.0).exp() * horner(&top) / horner(&bottom)
    } else {
        let fraction = (1..=4)
            .rev()
            .fold(z + 0.65, |acc, k| z + f64::from(k) / acc);
        (-z * z / 2.0).exp() / fraction / 2.506_628_274_631
    };
    if x > 0.0 { 1.0 - tail } else { tail }
}

/// The root of a monotone `f` that changes sign between `low` and `high`, by the Illinois
/// variant of regula falsi, to a few units in the last place.
pub(crate) fn solve(f: impl Fn(f64) -> f64, mut low: f64, mut high: f64) -> f64 {
    let (mut f_low, mut f_high) = (f(low), f(high));
    if f_low == 0.0 {
        return low;
    }
    if f_high == 0.0 || f_low.signum() == f_high.signum() {
        return high;
    }
    let mut side = 0;
    // the evaluated point nearest the root, which may stay on one side of the bracket
    let mut best = if f_low.abs() < f_high.abs() {
        (f_low.abs(), low)
    } else {
        (f_high.abs(), high)
    };
    for _ in 0..300 {
        let mut x = (low * f_high - high * f_low) / (f_high - f_low);
        if !x.is_finite() || x <= low.min(high) || x >= low.max(high) {
            x = (low + high) / 2.0;
        }
        let fx = f(x);
        if fx == 0.0 {
            return x;
        }
        if fx.abs() < best.0 {
            best = (fx.abs(), x);
        }
        if fx.signum() == f_low.signum() {
            low = x;
            f_low = fx;
            if side == -1 {
                f_high /= 2.0;
            }
            side = -1;
        } else {
            high = x;
            f_high = fx;
            if side == 1 {
                f_low /= 2.0;
            }
            side = 1;
        }
        if (high - low).abs() <= 4.0 * f64::EPSILON * (low.abs() + high.abs()) + 1e-300 {
            break;
        }
    }
    best.1
}

/// A point above `start` where the increasing `f` is no longer negative, by doubling.
fn bracket(f: &impl Fn(f64) -> f64, start: f64) -> f64 {
    let mut high = start;
    while f(high) < 0.0 && high < 1e300 {
        high *= 2.0;
    }
    high
}

/// The standard normal quantile: the x with Φ(x) = p, for 0 < p < 1. Solved on the lower half
/// by `erfc`, which keeps its relative precision far in the tail (1 - p is exact above one
/// half), in a narrow bracket around the rational start of Abramowitz and Stegun 26.2.23 (error
/// below 4.5e-4), comparing logarithms so the equation stays well scaled.
pub(crate) fn normal_quantile(p: f64) -> f64 {
    if p > 0.5 {
        return -normal_quantile(1.0 - p);
    }
    if p == 0.5 {
        return 0.0;
    }
    let t = (-2.0 * p.ln()).sqrt();
    let start = -(t
        - (2.515517 + 0.802853 * t + 0.010328 * t * t)
            / (1.0 + 1.432788 * t + 0.189269 * t * t + 0.001308 * t * t * t));
    solve(
        |x| (erfc(-x / std::f64::consts::SQRT_2) / 2.0 / p).ln(),
        start - 1e-3,
        (start + 1e-3).min(0.0),
    )
}

/// The value of Student's t with `df` degrees of freedom exceeded with probability `upper`
/// (at most 1/2).
pub(crate) fn t_quantile(upper: f64, df: f64) -> f64 {
    let f = |t: f64| upper - t_p(t, df) / 2.0;
    solve(f, 0.0, bracket(&f, 4.0))
}

/// The probability of an F with `d1` and `d2` degrees of freedom above `f`.
pub(crate) fn f_p(f: f64, d1: f64, d2: f64) -> f64 {
    if f.is_nan() {
        return f64::NAN;
    }
    if f <= 0.0 {
        return 1.0;
    }
    beta_i(d2 / 2.0, d1 / 2.0, d2 / (d2 + d1 * f))
}

/// The value of F with `d1` and `d2` degrees of freedom exceeded with probability `upper`.
pub(crate) fn f_quantile(upper: f64, d1: f64, d2: f64) -> f64 {
    let f = |x: f64| upper - f_p(x, d1, d2);
    solve(f, 0.0, bracket(&f, 4.0))
}

/// Nodes and weights of 16-point Gauss-Legendre quadrature on [-1, 1], found by Newton's
/// method on the Legendre polynomial.
fn gauss_legendre() -> &'static [(f64, f64); 16] {
    static NODES: std::sync::OnceLock<[(f64, f64); 16]> = std::sync::OnceLock::new();
    NODES.get_or_init(|| {
        const N: usize = 16;
        let mut nodes = [(0.0, 0.0); N];
        for (i, node) in nodes.iter_mut().enumerate() {
            let mut x = (std::f64::consts::PI * (i as f64 + 0.75) / (N as f64 + 0.5)).cos();
            let mut derivative = 1.0;
            for _ in 0..100 {
                let (mut p0, mut p1) = (1.0, x);
                for k in 2..=N {
                    let k = k as f64;
                    (p0, p1) = (p1, ((2.0 * k - 1.0) * x * p1 - (k - 1.0) * p0) / k);
                }
                derivative = N as f64 * (x * p1 - p0) / (x * x - 1.0);
                let step = p1 / derivative;
                x -= step;
                if step.abs() < 1e-16 {
                    break;
                }
            }
            *node = (x, 2.0 / ((1.0 - x * x) * derivative * derivative));
        }
        nodes
    })
}

/// The integral of `f` from `a` to `b` by 16-point Gauss-Legendre on `pieces` equal parts.
fn integrate(f: impl Fn(f64) -> f64, a: f64, b: f64, pieces: usize) -> f64 {
    let width = (b - a) / pieces as f64;
    let mut sum = 0.0;
    for piece in 0..pieces {
        let middle = a + width * (piece as f64 + 0.5);
        for &(x, w) in gauss_legendre() {
            sum += w * f(middle + x * width / 2.0);
        }
    }
    sum * width / 2.0
}

/// P(the range of `k` standard normals < w).
fn normal_range_cdf(w: f64, k: f64) -> f64 {
    if w <= 0.0 {
        return 0.0;
    }
    let density = |z: f64| (-z * z / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let inner = |z: f64| {
        let inside = (normal_cdf(z) - normal_cdf(z - w)).max(0.0);
        density(z) * inside.powf(k - 1.0)
    };
    (k * integrate(inner, -8.5, 8.5, 12)).min(1.0)
}

/// The distribution function of the studentized range of `k` means with `df` degrees of
/// freedom: P(Q < q), where Q is the range of k standard normals over an independent
/// chi / sqrt(df).
pub(crate) fn studentized_range_cdf(q: f64, k: f64, df: f64) -> f64 {
    if q <= 0.0 {
        return 0.0;
    }
    over_scale(df, |s| normal_range_cdf(q * s, k))
}

/// E[f(s)] for s = chi / sqrt(df), the scale of a statistic studentized with `df` degrees of
/// freedom; f(1) above 50000 df, where s is 1 to a few parts in 1e3 and the integral to 1e-10.
fn over_scale(df: f64, f: impl Fn(f64) -> f64) -> f64 {
    if df > 50_000.0 {
        return f(1.0);
    }
    // the density of s, integrated where it is not negligible
    let half = df / 2.0;
    let log_density = |s: f64| {
        std::f64::consts::LN_2 + half * half.ln() - ln_gamma(half) + (df - 1.0) * s.ln()
            - half * s * s
    };
    let mode = ((df - 1.0).max(0.0) / df).sqrt().max(0.05);
    let peak = log_density(mode);
    let step = 0.05 / df.sqrt();
    let mut high = mode;
    while log_density(high) > peak - 40.0 {
        high += step;
    }
    let mut low = mode;
    while low > 0.0 && log_density(low) > peak - 40.0 {
        low -= step;
    }
    let low = low.max(0.0);
    let outer = |s: f64| {
        if s <= 0.0 {
            0.0
        } else {
            log_density(s).exp() * f(s)
        }
    };
    integrate(outer, low, high, 16).clamp(0.0, 1.0)
}

/// The value of the studentized range of `k` means with `df` degrees of freedom exceeded with
/// probability `upper`.
pub(crate) fn studentized_range_quantile(upper: f64, k: f64, df: f64) -> f64 {
    let f = |q: f64| studentized_range_cdf(q, k, df) - (1.0 - upper);
    solve(f, 0.0, bracket(&f, 4.0))
}

/// The distribution function of the studentized maximum modulus of `r` independent standard
/// normals with `df` degrees of freedom: P(max |z_i| / s < m).
pub(crate) fn maximum_modulus_cdf(m: f64, r: f64, df: f64) -> f64 {
    if m <= 0.0 {
        return 0.0;
    }
    over_scale(df, |s| (2.0 * normal_cdf(m * s) - 1.0).max(0.0).powf(r))
}

/// The value of the studentized maximum modulus of `r` normals with `df` degrees of freedom
/// exceeded with probability `upper`.
pub(crate) fn maximum_modulus_quantile(upper: f64, r: f64, df: f64) -> f64 {
    let f = |m: f64| maximum_modulus_cdf(m, r, df) - (1.0 - upper);
    solve(f, 0.0, bracket(&f, 4.0))
}

/// Dunnett's many-to-one distribution: P(max T_i < d), or P(max |T_i| < d) when `two_sided`,
/// for t statistics with `df` degrees of freedom whose normal parts have the product
/// correlations lambda_i lambda_j. Each z_i = lambda_i z0 + sqrt(1 - lambda_i^2) e_i with
/// independent standard normals, so the probability is a double integral over z0 and the scale.
pub(crate) fn dunnett_cdf(d: f64, lambdas: &[f64], df: f64, two_sided: bool) -> f64 {
    if two_sided && d <= 0.0 {
        return 0.0;
    }
    let density = |z: f64| (-z * z / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let given_scale = |s: f64| {
        let x = d * s;
        let inner = |z: f64| {
            let product: f64 = lambdas
                .iter()
                .map(|&l| {
                    let c = (1.0 - l * l).sqrt();
                    let upper = normal_cdf((x - l * z) / c);
                    if two_sided {
                        (upper - normal_cdf((-x - l * z) / c)).max(0.0)
                    } else {
                        upper
                    }
                })
                .product();
            density(z) * product
        };
        integrate(inner, -8.5, 8.5, 24)
    };
    over_scale(df, given_scale)
}

/// The value of Dunnett's statistic (see `dunnett_cdf`) exceeded with probability `upper`.
pub(crate) fn dunnett_quantile(upper: f64, lambdas: &[f64], df: f64, two_sided: bool) -> f64 {
    let f = |d: f64| dunnett_cdf(d, lambdas, df, two_sided) - (1.0 - upper);
    solve(f, 0.0, bracket(&f, 4.0))
}

/// The distribution function of the noncentral t with `df` degrees of freedom and
/// noncentrality `delta`: P(T < t), a sum of Poisson-weighted incomplete beta functions taken
/// from the largest term outwards.
pub(crate) fn noncentral_t_cdf(t: f64, df: f64, delta: f64) -> f64 {
    if t < 0.0 {
        return 1.0 - noncentral_t_cdf(-t, df, -delta);
    }
    let base = normal_cdf(-delta);
    if t == 0.0 || delta == 0.0 && t == 0.0 {
        return base;
    }
    let x = t * t / (t * t + df);
    let lambda = delta * delta / 2.0;
    if lambda == 0.0 {
        return base + beta_i(0.5, df / 2.0, x) / 2.0;
    }
    let term = |j: f64| {
        let p = (-lambda + j * lambda.ln() - ln_gamma(j + 1.0)).exp();
        let q = delta.signum() * (-lambda + (j + 0.5) * lambda.ln() - ln_gamma(j + 1.5)).exp();
        p * beta_i(j + 0.5, df / 2.0, x) + q * beta_i(j + 1.0, df / 2.0, x)
    };
    let top = lambda.floor();
    let mut sum = 0.0;
    let mut j = top;
    loop {
        let value = term(j);
        sum += value;
        if j == 0.0 || (j < top - 5.0 && value.abs() < 1e-17 * sum.abs()) {
            break;
        }
        j -= 1.0;
    }
    let mut j = top + 1.0;
    loop {
        let value = term(j);
        sum += value;
        if (j > top + 5.0 && value.abs() < 1e-17 * sum.abs()) || j > top + 100_000.0 {
            break;
        }
        j += 1.0;
    }
    (base + sum / 2.0).clamp(0.0, 1.0)
}

/// The noncentrality at which a noncentral t with `df` degrees of freedom lies below `t` with
/// probability `p`.
pub(crate) fn noncentral_t_parameter(t: f64, df: f64, p: f64) -> f64 {
    // the probability falls as the noncentrality grows
    let f = |delta: f64| noncentral_t_cdf(t, df, delta) - p;
    let mut width = 2.0 + t.abs();
    while (f(t - width) < 0.0 || f(t + width) > 0.0) && width < 1e6 {
        width *= 2.0;
    }
    solve(f, t - width, t + width)
}

/// Hedges' correction factor J(k) = Γ(k/2) / (sqrt(k/2) Γ((k-1)/2)).
pub(crate) fn hedges_j(k: f64) -> f64 {
    (ln_gamma(k / 2.0) - 0.5 * (k / 2.0).ln() - ln_gamma((k - 1.0) / 2.0)).exp()
}

#[cfg(test)]
// the reference values keep the 17 digits they were printed with
#[allow(clippy::excessive_precision)]
mod tests {
    //! Expected values were printed with 17 digits by R, and for the studentized range by
    //! SciPy, whose values are closer than those of R (accurate to about 1e-8 there).

    use super::*;

    fn close(a: f64, b: f64, tolerance: f64) -> bool {
        (a - b).abs() <= tolerance * b.abs().max(1e-300)
    }

    const REFERENCE_F: [(f64, f64, f64, f64); 4] = [
        (3.238_716_003_1, 4.0, 255.0, 0.012_917_694_334_636_508),
        (1.5, 2.0, 7.0, 0.286_974_389_101_187_89),
        (0.2, 10.0, 30.0, 0.994_712_556_221_092_34),
        (25.0, 1.0, 3.5, 0.010_550_178_991_867_779),
    ];
    const F_95_4_255: f64 = 2.407_043_029_050_528_9;
    const T_995_1023: f64 = 2.624_745_048_245_882_3;
    const REFERENCE_PTUKEY: [(f64, f64, f64, f64); 8] = [
        (3.5, 5.0, 20.0, 0.863_497_648_429_593_2),
        (2.0, 3.0, 5.0, 0.597_637_569_004_219_7),
        (4.0, 10.0, 100.0, 0.858_752_524_814_786_1),
        (1.5, 2.0, 2.0, 0.6),
        (6.0, 4.0, 1000.0, 0.999_858_355_153_680_4),
        (3.0, 5.0, 1e6, 0.789_123_495_036_462),
        (0.5, 4.0, 12.0, 0.015_893_721_091_501_91),
        (10.0, 20.0, 3.0, 0.931_231_804_233_975_9),
    ];
    const REFERENCE_QTUKEY: [(f64, f64, f64, f64); 4] = [
        (0.05, 5.0, 255.0, 3.885_405_337_696_172),
        (0.05, 3.0, 10.0, 3.876_776_750_013_158),
        (0.01, 10.0, 30.0, 5.756_254_907_238_702),
        (0.05, 5.0, 102.26, 3.927_343_188_349_768_7),
    ];
    const REFERENCE_NCT: [(f64, f64, f64, f64); 6] = [
        (2.0, 10.0, 1.0, 0.807_611_562_530_311_08),
        (-1.0, 5.0, 0.5, 0.082_444_091_056_970_814),
        (3.0, 20.0, 4.0, 0.173_991_399_810_558_47),
        (0.5, 3.0, -2.0, 0.992_199_698_923_555_97),
        (10.0, 50.0, 8.0, 0.917_147_041_785_598_36),
        (1.2, 7.5, 0.0, 0.866_665_538_603_389_02),
    ];
    const HEDGES_10: f64 = 0.922_745_608_053_087_03;

    #[test]
    fn normal_cdf_agrees_with_erfc() {
        for i in -400..=400 {
            let x = f64::from(i) / 40.0;
            let slow = erfc(-x / std::f64::consts::SQRT_2) / 2.0;
            assert!((normal_cdf(x) - slow).abs() <= 1e-16 + 1e-12 * slow, "{x}");
        }
        assert!(close(normal_cdf(-12.0), 1.776_482_112_077_678_8e-33, 1e-9));
    }

    #[test]
    fn f_distribution_known_values() {
        for (f, d1, d2, p) in REFERENCE_F {
            assert!(close(f_p(f, d1, d2), p, 1e-11), "{f} {d1} {d2}");
        }
        assert!(close(f_quantile(0.05, 4.0, 255.0), F_95_4_255, 1e-11));
        assert_eq!(f_p(0.0, 3.0, 9.0), 1.0);
    }

    #[test]
    fn t_quantile_known_values() {
        assert!(close(
            t_quantile(0.025, 10.0),
            2.228_138_851_986_522_2,
            1e-12
        ));
        assert!(close(t_quantile(0.005, 102.3), T_995_1023, 1e-12));
        assert!(close(t_quantile(0.05, 1.0), 6.313_751_514_675_042_6, 1e-12));
        // with many degrees of freedom the log gamma terms of the beta function are near 360,
        // so their rounding limits the quantile to about 1e-13
        let got = t_quantile(0.025, 199.0);
        assert!(close(got, 1.971_956_544_251_751_9, 1e-13), "{got}");
    }

    #[test]
    fn normal_quantile_known_values() {
        for (p, x) in [
            (0.975, 1.959_963_984_540_054),
            (0.95, 1.644_853_626_951_472_2),
            (0.01, -2.326_347_874_040_840_8),
            (1e-10, -6.361_340_902_404_056),
            (0.5, 0.0),
        ] {
            let got = normal_quantile(p);
            assert!((got - x).abs() <= 1e-14 * x.abs().max(1.0), "{p}: {got}");
        }
        for i in 1..1000 {
            let p = f64::from(i) / 1000.0;
            assert!((normal_cdf(normal_quantile(p)) - p).abs() < 1e-15, "{p}");
        }
    }

    #[test]
    fn studentized_range_known_values() {
        for (q, k, df, p) in REFERENCE_PTUKEY {
            let got = studentized_range_cdf(q, k, df);
            assert!((got - p).abs() < 2e-10, "{q} {k} {df}: {got} vs {p}");
        }
        for (upper, k, df, q) in REFERENCE_QTUKEY {
            let got = studentized_range_quantile(upper, k, df);
            assert!(close(got, q, 1e-9), "{upper} {k} {df}: {got} vs {q}");
        }
    }

    #[test]
    fn noncentral_t_known_values() {
        for (t, df, delta, p) in REFERENCE_NCT {
            let got = noncentral_t_cdf(t, df, delta);
            assert!((got - p).abs() < 1e-12, "{t} {df} {delta}: {got} vs {p}");
        }
        // the parameter search inverts the distribution function
        let delta = noncentral_t_parameter(2.5, 30.0, 0.975);
        assert!((noncentral_t_cdf(2.5, 30.0, delta) - 0.975).abs() < 1e-12);
        assert!(close(hedges_j(10.0), HEDGES_10, 1e-13));
    }

    // Studentized maximum modulus and Dunnett's distribution: R's integrate() of the same
    // integrals at a relative tolerance of 1e-13 (cross-checked with mvtnorm's pmvt to 1e-8),
    // and the critical values of Dunnett's published tables to their two decimals.
    const REFERENCE_SMM: [(f64, f64, f64, f64); 5] = [
        (2.5, 3.0, 10.0, 0.913_685_721_036_227_25),
        (3.0, 10.0, 20.0, 0.936_095_528_946_572_22),
        (2.0, 1.0, 5.0, 0.898_060_521_170_141_9),
        (3.5, 6.0, 3.0, 0.860_426_876_678_561_99),
        (2.9, 45.0, 120.0, 0.824_155_097_138_753_01),
    ];
    const REFERENCE_QSMM: [(f64, f64, f64); 3] = [
        (10.0, 20.0, 3.113_968_958_470_938_5),
        (3.0, 10.0, 2.828_855_230_224_835_5),
        (6.0, 60.0, 2.716_088_737_385_315_3),
    ];

    #[test]
    fn maximum_modulus_known_values() {
        for (m, r, df, p) in REFERENCE_SMM {
            let got = maximum_modulus_cdf(m, r, df);
            assert!((got - p).abs() < 1e-10, "{m} {r} {df}: {got} vs {p}");
        }
        for (r, df, m) in REFERENCE_QSMM {
            let got = maximum_modulus_quantile(0.05, r, df);
            assert!(close(got, m, 1e-9), "{r} {df}: {got} vs {m}");
        }
        // one modulus is |t|
        assert!((maximum_modulus_cdf(2.0, 1.0, 5.0) - (1.0 - t_p(2.0, 5.0))).abs() < 1e-10);
    }

    #[test]
    fn dunnett_known_values() {
        let equal = [0.5f64.sqrt(); 3];
        let unequal = [
            0.674_199_862_463_242_04,
            0.745_355_992_499_929_9,
            0.804_399_666_539_843_74,
        ];
        let cases = [
            (2.5, &equal, 10.0, true, 0.923_464_387_108_830_16),
            (2.0, &equal, 10.0, false, 0.914_481_751_627_406_52),
            (2.3, &unequal, 45.0, true, 0.934_436_573_826_292_08),
            (1.9, &unequal, 45.0, false, 0.924_430_925_307_719_75),
        ];
        for (d, lambdas, df, two, p) in cases {
            let got = dunnett_cdf(d, lambdas, df, two);
            assert!((got - p).abs() < 1e-10, "{d} {df} {two}: {got} vs {p}");
        }
        // with correlations 1/2 the k statistics all lie below 0 with probability 1 / (k + 1)
        for k in 1..=6 {
            let got = dunnett_cdf(0.0, &vec![0.5f64.sqrt(); k], 7.0, false);
            assert!((got - 1.0 / (k as f64 + 1.0)).abs() < 1e-10, "{k}: {got}");
        }
        // Dunnett's tables, equal groups, infinite df: two-sided then one-sided, 2 to 5 groups
        let table = [(2.21, 1.92), (2.35, 2.06), (2.44, 2.16), (2.51, 2.23)];
        for (p, (two, one)) in table.into_iter().enumerate() {
            let lambdas = vec![0.5f64.sqrt(); p + 2];
            let got = dunnett_quantile(0.05, &lambdas, 1e9, true);
            assert!((got - two).abs() <= 0.005, "{p}: {got}");
            let got = dunnett_quantile(0.05, &lambdas, 1e9, false);
            assert!((got - one).abs() <= 0.005, "{p}: {got}");
        }
        // 10 df, three groups: 2.76 and 2.34 in the tables, 2.758996978 and 2.337559070 in R
        let got = dunnett_quantile(0.05, &equal, 10.0, true);
        assert!(close(got, 2.758_996_978_373_59, 1e-9), "{got}");
        let got = dunnett_quantile(0.05, &equal, 10.0, false);
        assert!(close(got, 2.337_559_069_576_3, 1e-9), "{got}");
    }
}
