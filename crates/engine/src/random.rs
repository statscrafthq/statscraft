//! Random numbers: the two uniform generators that SET RNG chooses, and the distributions of
//! UNIFORM, NORMAL and the RV functions.
//!
//! Rules, from the descriptions of SET and of the generation of uniform random numbers:
//! - RNG=MC (the default) is the multiplicative congruential generator seed = 397204094 * seed
//!   mod (2^31 - 1), giving seed / 2^31. SET SEED sets its seed (a whole number from 1 to
//!   1,999,999,999; the default 2,000,000) and starts its sequence again.
//! - RNG=MT is the Mersenne Twister (MT19937). SET MTINDEX starts it from any number: the
//!   number's sign, size and 53 bits of mantissa become a key of three words for the standard
//!   initialisation by array. A draw is (a * 2^26 + b) / 2^53 from two 32-bit outputs a >> 5
//!   and b >> 6.
//! - SEED=RANDOM and MTINDEX=RANDOM take the time of day in milliseconds since midnight.
//! - Every function draws only for the cases it is evaluated for, in case order. A parameter
//!   outside its range (or missing) gives system-missing and draws nothing.

use crate::dist::normal_quantile;

const MC_MULTIPLIER: u64 = 397_204_094;
const MC_MODULUS: u64 = 2_147_483_647;
pub(crate) const DEFAULT_SEED: u64 = 2_000_000;

#[derive(Clone)]
pub(crate) struct Random {
    /// SET RNG=MT
    pub mt_active: bool,
    /// The state of the congruential generator; SHOW SEED shows it.
    pub seed: u64,
    mtindex: f64,
    /// Started from `mtindex` at its first draw.
    twister: Option<Box<Twister>>,
    /// The numbers the twister gave since it started.
    mt_draws: u64,
}

/// Where the generators stand, for a session to go on from in a later run. The settings
/// (SEED, MTINDEX, RNG) are kept with the other settings.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RandomState {
    /// The state of the congruential generator.
    pub seed: u64,
    /// The numbers the Mersenne Twister gave since MTINDEX started it.
    pub mt_draws: u64,
}

impl Default for Random {
    fn default() -> Self {
        Self {
            mt_active: false,
            seed: DEFAULT_SEED,
            mtindex: DEFAULT_SEED as f64,
            twister: None,
            mt_draws: 0,
        }
    }
}

/// The time of day in milliseconds since midnight, for SEED=RANDOM and MTINDEX=RANDOM.
pub(crate) fn time_seed() -> u64 {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    let ms = now.map_or(0, |d| d.as_millis() as u64) % 86_400_000;
    ms.max(1)
}

impl Random {
    pub fn set_mtindex(&mut self, index: f64) {
        self.mtindex = index;
        self.twister = None;
        self.mt_draws = 0;
    }

    pub fn state(&self) -> RandomState {
        RandomState {
            seed: self.seed,
            mt_draws: self.mt_draws,
        }
    }

    /// Goes on from `state`, after the settings put back SEED, MTINDEX and RNG.
    pub fn restore(&mut self, state: RandomState) {
        self.seed = state.seed;
        let mt_active = std::mem::replace(&mut self.mt_active, true);
        self.set_mtindex(self.mtindex);
        for _ in 0..state.mt_draws {
            self.uniform();
        }
        self.mt_active = mt_active;
    }

    /// A number from 0 to 1, from the generator SET RNG chose.
    pub fn uniform(&mut self) -> f64 {
        if self.mt_active {
            let index = self.mtindex;
            let twister = self
                .twister
                .get_or_insert_with(|| Box::new(Twister::from_index(index)));
            let a = f64::from(twister.next() >> 5);
            let b = f64::from(twister.next() >> 6);
            self.mt_draws += 1;
            (a * 67_108_864.0 + b) / 9_007_199_254_740_992.0
        } else {
            self.seed = MC_MULTIPLIER * self.seed % MC_MODULUS;
            self.seed as f64 / (MC_MODULUS + 1) as f64
        }
    }

    /// A uniform number above 0 and below 1, for logarithms and quantiles.
    fn open(&mut self) -> f64 {
        loop {
            let u = self.uniform();
            if u > 0.0 && u < 1.0 {
                return u;
            }
        }
    }

    pub fn normal(&mut self) -> f64 {
        normal_quantile(self.open())
    }

    /// Exponential with mean 1.
    fn exponential(&mut self) -> f64 {
        -self.open().ln()
    }

    /// Gamma with shape `a` and scale 1: algorithm GS (Ahrens and Dieter) below 1, Fishman's
    /// above 1, as the documented algorithms.
    fn gamma(&mut self, a: f64) -> f64 {
        if a == 1.0 {
            return self.exponential();
        }
        if a < 1.0 {
            let c = (std::f64::consts::E + a) / std::f64::consts::E;
            loop {
                let p = c * self.uniform();
                if p <= 1.0 {
                    let x = p.powf(1.0 / a);
                    if self.uniform() <= (-x).exp() {
                        return x;
                    }
                } else {
                    let x = -((c - p) / a).ln();
                    if x >= 0.0 && self.uniform() <= x.powf(a - 1.0) {
                        return x;
                    }
                }
            }
        }
        loop {
            let y = self.exponential();
            if self.open().ln() <= (a - 1.0) * (1.0 - y + y.ln()) {
                return a * y;
            }
        }
    }

    fn chi_square(&mut self, df: f64) -> f64 {
        2.0 * self.gamma(df / 2.0)
    }

    fn bernoulli(&mut self, p: f64) -> bool {
        p >= 1.0 || (p > 0.0 && self.uniform() <= p)
    }

    /// Trials up to and including the first success.
    fn geometric(&mut self, p: f64) -> f64 {
        if p >= 1.0 {
            return 1.0;
        }
        (self.open().ln() / (1.0 - p).ln()).ceil().max(1.0)
    }
}

/// The number of parameters of a random function, None for any other name.
pub(crate) fn arity(function: &str) -> Option<usize> {
    Some(match function {
        "UNIFORM" | "NORMAL" => 1,
        "RV.BERNOULLI" | "RV.CHISQ" | "RV.EXP" | "RV.GEOM" | "RV.POISSON" | "RV.T" => 1,
        "RV.BETA" | "RV.BINOM" | "RV.CAUCHY" | "RV.F" | "RV.GAMMA" | "RV.HALFNRM" | "RV.IGAUSS"
        | "RV.LAPLACE" | "RV.LOGISTIC" | "RV.LNORMAL" | "RV.NEGBIN" | "RV.NORMAL" | "RV.PARETO"
        | "RV.UNIFORM" | "RV.WEIBULL" => 2,
        "RV.HYPER" => 3,
        _ => return None,
    })
}

fn whole(x: f64) -> bool {
    x.fract() == 0.0 && x >= 0.0
}

// shortcut: the binomial, hypergeometric, negative binomial and Poisson draws take time in
// proportion to their counts; switch to the documented rejection algorithms if large
// counts get slow.
const MAX_COUNT: f64 = 1e8;

/// One value of `function` with parameters `p`, None where a parameter is out of range. The
/// parameters follow the documented definitions: EXP, GAMMA take a rate (mean 1/b, a/b);
/// LNORMAL(a, b) has ln X normal with mean ln a and sd b; WEIBULL(a, b) has scale a and shape b.
pub(crate) fn draw(random: &mut Random, function: &str, p: &[f64]) -> Option<f64> {
    let positive = |x: f64| x > 0.0;
    let probability = |x: f64| (0.0..=1.0).contains(&x);
    Some(match (function, p) {
        ("UNIFORM", &[max]) => max * random.uniform(),
        ("RV.UNIFORM", &[min, max]) if min <= max => min + (max - min) * random.uniform(),
        ("NORMAL", &[sd]) if positive(sd) => sd * random.normal(),
        ("RV.NORMAL", &[mean, sd]) if positive(sd) => mean + sd * random.normal(),
        ("RV.HALFNRM", &[mean, sd]) if positive(sd) => mean + sd * random.normal().abs(),
        ("RV.LNORMAL", &[a, b]) if positive(a) && positive(b) => a * (b * random.normal()).exp(),
        ("RV.EXP", &[rate]) if positive(rate) => random.exponential() / rate,
        ("RV.GAMMA", &[shape, rate]) if positive(shape) && positive(rate) => {
            random.gamma(shape) / rate
        }
        ("RV.CHISQ", &[df]) if positive(df) => random.chi_square(df),
        ("RV.BETA", &[a, b]) if positive(a) && positive(b) => {
            let x = random.gamma(a);
            x / (x + random.gamma(b))
        }
        ("RV.F", &[d1, d2]) if positive(d1) && positive(d2) => {
            (random.chi_square(d1) / d1) / (random.chi_square(d2) / d2)
        }
        ("RV.T", &[1.0]) => (std::f64::consts::PI * (random.open() - 0.5)).tan(),
        ("RV.T", &[df]) if positive(df) => {
            let z = random.normal();
            z / (random.chi_square(df) / df).sqrt()
        }
        ("RV.CAUCHY", &[location, scale]) if positive(scale) => {
            location + scale * (std::f64::consts::PI * (random.open() - 0.5)).tan()
        }
        ("RV.LAPLACE", &[mean, scale]) if positive(scale) => {
            let y = scale * random.exponential();
            if random.uniform() >= 0.5 {
                mean + y
            } else {
                mean - y
            }
        }
        ("RV.LOGISTIC", &[mean, scale]) if positive(scale) => {
            let u = random.open();
            mean + scale * (u / (1.0 - u)).ln()
        }
        ("RV.PARETO", &[threshold, shape]) if positive(threshold) && positive(shape) => {
            threshold / random.open().powf(1.0 / shape)
        }
        ("RV.WEIBULL", &[scale, shape]) if positive(scale) && positive(shape) => {
            scale * random.exponential().powf(1.0 / shape)
        }
        ("RV.IGAUSS", &[mean, shape]) if positive(mean) && positive(shape) => {
            // Michael, Schucany and Haas: a root of the chi-square(1) equation, one of two
            let v = random.normal().powi(2);
            let x = mean + mean * mean * v / (2.0 * shape)
                - mean / (2.0 * shape) * (4.0 * mean * shape * v + (mean * v).powi(2)).sqrt();
            if random.uniform() <= mean / (mean + x) {
                x
            } else {
                mean * mean / x
            }
        }
        ("RV.BERNOULLI", &[p]) if probability(p) => f64::from(u8::from(random.bernoulli(p))),
        ("RV.BINOM", &[n, p]) if whole(n) && n <= MAX_COUNT && probability(p) => {
            (0..n as u64).filter(|_| random.bernoulli(p)).count() as f64
        }
        ("RV.GEOM", &[p]) if p > 0.0 && p <= 1.0 => random.geometric(p),
        ("RV.NEGBIN", &[r, p])
            if whole(r) && (1.0..=MAX_COUNT).contains(&r) && p > 0.0 && p <= 1.0 =>
        {
            (0..r as u64).map(|_| random.geometric(p)).sum()
        }
        ("RV.HYPER", &[total, sample, hits])
            if whole(total)
                && whole(sample)
                && whole(hits)
                && total <= MAX_COUNT
                && sample <= total
                && hits <= total =>
        {
            // draw `sample` objects one by one without replacement
            let (mut left, mut marked, mut found) = (total, hits, 0.0);
            for _ in 0..sample as u64 {
                if random.uniform() * left < marked {
                    marked -= 1.0;
                    found += 1.0;
                }
                left -= 1.0;
            }
            found
        }
        ("RV.POISSON", &[mean]) if (0.0..=MAX_COUNT).contains(&mean) => {
            // Knuth's product of uniforms, in parts of at most 500 so exp(-part) stays above 0
            let (mut rest, mut count) = (mean, 0.0);
            while rest > 0.0 {
                let part = rest.min(500.0);
                rest -= part;
                let limit = (-part).exp();
                let mut product = random.uniform();
                while product > limit {
                    count += 1.0;
                    product *= random.uniform();
                }
            }
            count
        }
        _ => return None,
    })
}

/// MT19937, the 32-bit Mersenne Twister of Matsumoto and Nishimura.
#[derive(Clone)]
struct Twister {
    state: [u32; 624],
    index: usize,
}

impl Twister {
    fn new(seed: u32) -> Self {
        let mut state = [0u32; 624];
        state[0] = seed;
        for i in 1..624 {
            let previous = state[i - 1];
            state[i] = 1_812_433_253u32
                .wrapping_mul(previous ^ (previous >> 30))
                .wrapping_add(i as u32);
        }
        Self { state, index: 624 }
    }

    fn by_array(key: &[u32]) -> Self {
        let mut t = Self::new(19_650_218);
        let x = &mut t.state;
        let (mut i, mut j) = (1usize, 0usize);
        for _ in 0..624.max(key.len()) {
            let previous = x[i - 1];
            x[i] = (x[i] ^ (previous ^ (previous >> 30)).wrapping_mul(1_664_525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= 624 {
                x[0] = x[623];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..623 {
            let previous = x[i - 1];
            x[i] = (x[i] ^ (previous ^ (previous >> 30)).wrapping_mul(1_566_083_941))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                x[0] = x[623];
                i = 1;
            }
        }
        x[0] = 0x8000_0000;
        t
    }

    /// The key that MTINDEX `x` stands for: flags and exponent, then the top 26 and the next
    /// 27 bits of the mantissa.
    fn from_index(x: f64) -> Self {
        let size = x.abs();
        // size = mantissa * 2^exponent with the mantissa from 0.5 to below 1
        let (mantissa, exponent) = if size == 0.0 || !size.is_finite() {
            (0.0, 0)
        } else {
            let exponent = size.log2().floor() as i32 + 1;
            let mut mantissa = size / 2f64.powi(exponent);
            let mut exponent = exponent;
            if mantissa >= 1.0 {
                mantissa /= 2.0;
                exponent += 1;
            } else if mantissa < 0.5 {
                mantissa *= 2.0;
                exponent -= 1;
            }
            (mantissa, exponent)
        };
        // d is the power of two that brings the size into [0.5, 1): divided by it above 1,
        // multiplied by it from 0 to 1
        let d = if size > 1.0 {
            i64::from(exponent)
        } else {
            -i64::from(exponent)
        };
        let flags = 4 * i64::from(size >= 1.0) + 2 * i64::from(x < 0.0) + i64::from(x == 0.0);
        let high = (mantissa * 67_108_864.0).trunc();
        let low = mantissa * 9_007_199_254_740_992.0 - high * 134_217_728.0;
        let key = [(8 * d + flags) as u32, high as u32, low as u32];
        Self::by_array(&key)
    }

    fn next(&mut self) -> u32 {
        if self.index >= 624 {
            for k in 0..624 {
                let y = (self.state[k] & 0x8000_0000) | (self.state[(k + 1) % 624] & 0x7fff_ffff);
                let mut next = self.state[(k + 397) % 624] ^ (y >> 1);
                if y & 1 == 1 {
                    next ^= 0x9908_b0df;
                }
                self.state[k] = next;
            }
            self.index = 0;
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_twister_gives_the_published_reference_outputs() {
        // the first outputs of the reference implementation for init_by_array {0x123, 0x234,
        // 0x345, 0x456}
        let mut t = Twister::by_array(&[0x123, 0x234, 0x345, 0x456]);
        let first: Vec<u32> = (0..5).map(|_| t.next()).collect();
        assert_eq!(
            first,
            [
                1_067_595_299,
                955_945_823,
                477_289_528,
                4_107_218_783,
                4_228_976_476
            ]
        );
        // and for init_genrand(5489), the default seed of the reference
        let mut t = Twister::new(5489);
        assert_eq!(t.next(), 3_499_211_612);
    }

    #[test]
    fn the_congruential_generator_follows_its_recurrence() {
        let mut random = Random::default();
        let u = random.uniform();
        let seed = 397_204_094u64 * 2_000_000 % 2_147_483_647;
        assert_eq!(random.seed, seed);
        assert_eq!(u, seed as f64 / 2_147_483_648.0);
    }

    #[test]
    fn distributions_have_their_documented_means() {
        let mut random = Random {
            mt_active: true,
            ..Random::default()
        };
        let mean = |random: &mut Random, f: &str, p: &[f64]| {
            let n = 20_000;
            (0..n).map(|_| draw(random, f, p).unwrap()).sum::<f64>() / n as f64
        };
        let cases: [(&str, &[f64], f64, f64); 16] = [
            ("UNIFORM", &[10.0], 5.0, 0.1),
            ("RV.NORMAL", &[3.0, 2.0], 3.0, 0.05),
            ("RV.EXP", &[2.0], 0.5, 0.02),
            ("RV.GAMMA", &[0.5, 2.0], 0.25, 0.02),
            ("RV.GAMMA", &[3.0, 2.0], 1.5, 0.05),
            ("RV.CHISQ", &[4.0], 4.0, 0.1),
            ("RV.BETA", &[2.0, 6.0], 0.25, 0.01),
            ("RV.WEIBULL", &[2.0, 1.0], 2.0, 0.06),
            ("RV.LNORMAL", &[1.0, 0.5], (0.125f64).exp(), 0.02),
            ("RV.PARETO", &[1.0, 3.0], 1.5, 0.05),
            ("RV.LAPLACE", &[1.0, 2.0], 1.0, 0.08),
            ("RV.BINOM", &[10.0, 0.3], 3.0, 0.05),
            ("RV.GEOM", &[0.25], 4.0, 0.1),
            ("RV.POISSON", &[700.0], 700.0, 1.0),
            ("RV.HYPER", &[20.0, 5.0, 8.0], 2.0, 0.05),
            ("RV.IGAUSS", &[2.0, 3.0], 2.0, 0.06),
        ];
        for (f, p, expected, tolerance) in cases {
            let m = mean(&mut random, f, p);
            assert!((m - expected).abs() < tolerance, "{f}{p:?}: {m}");
        }
        assert_eq!(draw(&mut random, "RV.NORMAL", &[0.0, -1.0]), None);
        assert_eq!(draw(&mut random, "RV.BINOM", &[2.5, 0.5]), None);
        assert_eq!(draw(&mut random, "RV.BERNOULLI", &[1.0]), Some(1.0));
    }
}
