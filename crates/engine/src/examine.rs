//! EXAMINE: descriptives, percentiles, M-estimators, extreme values and tests of normality of
//! dependent variables, over all cases and within the cells of factors.
//!
//! Rules from the Command Syntax Reference (EXAMINE) and the Algorithms manual (EXAMINE):
//! - Every statistic works on the distinct values y_1 < ... < y_m of a cell with their summed
//!   weights c_i, the cumulative weights cc_i and the total W. Cases with a missing, zero or
//!   negative weight are left out.
//! - Each factor term (one factor, or two crossed) is a model of its own, after the model of all
//!   cases unless NOTOTAL; its cells are the combinations of factor values that occur.
//! - LISTWISE (default) uses a case only when every dependent and every factor of the command is
//!   valid; PAIRWISE when the dependent and the factors of the model are. INCLUDE makes user-
//!   missing values of the dependents valid, REPORT makes missing factor values categories.
//! - The median and the interquartile range use the percentile method of /PERCENTILES.
//! - Percentiles: tc1 = W p and tc2 = (W + 1) p with cc_k <= tc < cc_k+1. Where the manual
//!   is silent: y_0 is y_1 and y_m+1 is y_m, so a percentile never leaves the data's range.
//! - M-estimators start at the median (the manual does not say) and stop when a step is at most
//!   0.005 of the mean of the two estimates, or after 30 steps. The scale is the median absolute
//!   deviation; when it is 0 the estimate is empty.
//! - Kolmogorov-Smirnov with the Lilliefors significance of Dallal and Wilkinson above the
//!   critical value of 0.1, interpolated down to the critical value of 0.2, which the manual does
//!   not give; it is taken from the published table of Molin and Abdi (100,000 samples per size).
//! - Shapiro-Wilk on the series expanded by weights rounded to whole numbers, for 3 to 5000
//!   cases (3 to 50 with non-integer weights), as the manual says. W and its significance follow
//!   Royston (1992, AS R94 1995), not the manual's coefficient formula and table: that table ends
//!   at 50 cases, below the 5000 the manual itself allows, and its coefficients drift from the
//!   exact ones as n grows (W of 0.92 where the standard algorithm gives 0.95 at 181 cases).

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number, summarize};
use crate::dist::{erfc, normal_cdf, normal_quantile, t_quantile};
use crate::format::{Stat, formats_of, procedure_format};
use crate::means::is_valid;
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Values, Variable};
use statscraft_syntax::{Examine, MEstimator, PercentileMethod as Method};

mod plots;

/// The distinct values of a cell in ascending order with their summed weights.
pub(crate) struct Distribution {
    values: Vec<f64>,
    weights: Vec<f64>,
    /// cc_i, the weight up to and including value i.
    cumulative: Vec<f64>,
    total: f64,
}

impl Distribution {
    pub(crate) fn new(pairs: &[(f64, f64)]) -> Self {
        let mut pairs: Vec<(f64, f64)> = pairs.iter().copied().filter(|p| p.1 > 0.0).collect();
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (mut values, mut weights): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
        for (x, w) in pairs {
            if values.last() == Some(&x) {
                *weights.last_mut().expect("a value was pushed") += w;
            } else {
                values.push(x);
                weights.push(w);
            }
        }
        let cumulative: Vec<f64> = weights
            .iter()
            .scan(0.0, |sum, w| {
                *sum += w;
                Some(*sum)
            })
            .collect();
        let total = cumulative.last().copied().unwrap_or(0.0);
        Self {
            values,
            weights,
            cumulative,
            total,
        }
    }

    /// Room for rounding in sums of weights when cumulative weights are compared.
    fn fuzz(&self) -> f64 {
        1e-10 * self.total.max(1.0)
    }

    /// y_i, 1-based, with y_0 = y_1 and y_m+1 = y_m.
    fn y(&self, i: usize) -> f64 {
        self.values[i.clamp(1, self.values.len()) - 1]
    }

    /// c_i, 1-based; 1 outside the values, where y_i equals its neighbour anyway.
    fn c(&self, i: usize) -> f64 {
        match i {
            0 => 1.0,
            i => self.weights.get(i - 1).copied().unwrap_or(1.0),
        }
    }

    fn cc(&self, k: usize) -> f64 {
        match k {
            0 => 0.0,
            k => self.cumulative[k - 1],
        }
    }

    /// The k with cc_k <= t < cc_k+1, and g* = t - cc_k (0 within rounding).
    fn locate(&self, t: f64) -> (usize, f64) {
        let fuzz = self.fuzz();
        let k = self.cumulative.partition_point(|&cc| cc <= t + fuzz);
        let g = t - self.cc(k);
        (k, if g.abs() <= fuzz { 0.0 } else { g })
    }

    /// The weighted average of y_k and y_k+1 at g* = `g` (Waverage, Haverage, Tukey's hinges).
    fn average(&self, k: usize, g: f64) -> f64 {
        let c = self.c(k + 1);
        if g >= 1.0 {
            self.y(k + 1)
        } else {
            let share = if c >= 1.0 { g } else { g / c };
            (1.0 - share) * self.y(k) + share * self.y(k + 1)
        }
    }

    /// The `percent` percentile by `method`; None without cases.
    pub(crate) fn percentile(&self, percent: f64, method: Method) -> Option<f64> {
        if self.values.is_empty() {
            return None;
        }
        let w = self.total;
        // the product first keeps W p exact for round percentages
        let (k, g) = self.locate(w * percent / 100.0);
        Some(match method {
            Method::Haverage => {
                let (k, g) = self.locate((w + 1.0) * percent / 100.0);
                self.average(k, g)
            }
            Method::Waverage => self.average(k, g),
            Method::Round => {
                let c = self.c(k + 1);
                let share = if c >= 1.0 { g } else { g / c };
                if share < 0.5 {
                    self.y(k)
                } else {
                    self.y(k + 1)
                }
            }
            Method::Empirical if g == 0.0 => self.y(k),
            Method::Aempirical if g == 0.0 => (self.y(k) + self.y(k + 1)) / 2.0,
            Method::Empirical | Method::Aempirical => self.y(k + 1),
        })
    }

    /// The `percent` percentile by the rule of FREQUENCIES: tc = (W + 1) p and, with
    /// cc_k <= tc < cc_k+1, y_k+1 when tc - cc_k >= 1, else (1 - g) y_k + g y_k+1 with
    /// g = tc - cc_k. Unlike HAVERAGE, g is not divided by a weight c_k+1 below 1.
    pub(crate) fn frequencies_percentile(&self, percent: f64) -> Option<f64> {
        if self.values.is_empty() {
            return None;
        }
        let (k, g) = self.locate((self.total + 1.0) * percent / 100.0);
        Some(if g >= 1.0 {
            self.y(k + 1)
        } else {
            (1.0 - g) * self.y(k) + g * self.y(k + 1)
        })
    }

    /// Tukey's hinges: the 25th, 50th and 75th percentiles by the depths of the hinges.
    fn hinges(&self) -> Option<[f64; 3]> {
        if self.values.is_empty() {
            return None;
        }
        let w = self.total;
        let least = self.weights.iter().copied().fold(f64::INFINITY, f64::min);
        let depths = if least >= 1.0 {
            let d = ((w + 3.0) / 2.0).floor() / 2.0;
            [d, w / 2.0 + 0.5, w + 1.0 - d]
        } else {
            let d = ((w / least + 3.0) / 2.0).floor() / 2.0;
            [d * least, w / 2.0 + least / 2.0, w + least - d * least]
        };
        Some(depths.map(|depth| {
            let (h, a) = self.locate(depth);
            self.average(h, a)
        }))
    }

    /// The 5% trimmed mean.
    pub(crate) fn trimmed_mean(&self) -> Option<f64> {
        if self.values.is_empty() {
            return None;
        }
        let (w, fuzz) = (self.total, self.fuzz());
        let tc = 0.05 * w;
        // cc_k1 < tc <= cc_k1+1 and cc_k2-1 <= W - tc < cc_k2
        let k1 = self.cumulative.partition_point(|&cc| cc < tc - fuzz);
        let k2 = self.cumulative.partition_point(|&cc| cc <= w - tc + fuzz) + 1;
        if k1 + 1 >= k2 {
            return Some(self.y(k2));
        }
        let inner: f64 = (k1 + 2..k2).map(|i| self.c(i) * self.y(i)).sum();
        let low = (self.cc(k1 + 1) - tc) * self.y(k1 + 1);
        let high = (w - self.cc(k2 - 1) - tc) * self.y(k2);
        Some((low + high + inner) / (0.9 * w))
    }
}

/// The weight function ω(u) = Ψ(u) / u of an M-estimator.
fn omega(estimator: MEstimator, u: f64) -> f64 {
    let a = u.abs();
    match estimator {
        MEstimator::Huber(k) if a <= k => 1.0,
        MEstimator::Huber(k) => k / a,
        MEstimator::Tukey(c) if a <= c => (1.0 - (u / c).powi(2)).powi(2),
        MEstimator::Tukey(_) => 0.0,
        MEstimator::Hampel(lo, _, _) if a <= lo => 1.0,
        MEstimator::Hampel(lo, mid, _) if a <= mid => lo / a,
        MEstimator::Hampel(lo, mid, hi) if a <= hi => lo / a * (hi - a) / (hi - mid),
        MEstimator::Hampel(..) => 0.0,
        MEstimator::Andrew(c) => {
            // the constant is given in units of pi
            let c = c * std::f64::consts::PI;
            if a == 0.0 {
                1.0
            } else if a <= c {
                c / (std::f64::consts::PI * u) * (std::f64::consts::PI * u / c).sin()
            } else {
                0.0
            }
        }
    }
}

/// The M-estimate of location: T_k+1 = Σ c y ω(u) / Σ c ω(u), u = (y - T_k) / s, from the
/// median, with s the median absolute deviation.
pub(crate) fn m_estimate(
    dist: &Distribution,
    estimator: MEstimator,
    method: Method,
) -> Option<f64> {
    let median = dist.percentile(50.0, method)?;
    let deviations: Vec<(f64, f64)> = dist
        .values
        .iter()
        .zip(&dist.weights)
        .map(|(y, c)| ((y - median).abs(), *c))
        .collect();
    let s = Distribution::new(&deviations).percentile(50.0, method)?;
    if s <= 0.0 {
        return None;
    }
    let mut t = median;
    for _ in 0..30 {
        let (mut top, mut bottom) = (0.0, 0.0);
        for (y, c) in dist.values.iter().zip(&dist.weights) {
            let weight = c * omega(estimator, (y - t) / s);
            top += weight * y;
            bottom += weight;
        }
        if bottom <= 0.0 {
            return None;
        }
        let next = top / bottom;
        let done = (next - t).abs() <= 0.005 * ((next + t) / 2.0).abs();
        t = next;
        if done {
            break;
        }
    }
    Some(t)
}

/// The Kolmogorov-Smirnov statistic against the normal distribution with the cell's mean and
/// standard deviation.
fn kolmogorov_smirnov(dist: &Distribution, mean: f64, sd: f64) -> f64 {
    let w = dist.total;
    let mut d: f64 = 0.0;
    for (i, y) in dist.values.iter().enumerate() {
        let f = normal_cdf((y - mean) / sd);
        d = d.max(dist.cumulative[i] / w - f).max(f - dist.cc(i) / w);
    }
    d
}

/// The critical values of the Lilliefors statistic at 0.2 for 4 to 50 cases (Molin and Abdi,
/// 1998, as published by Abdi and Molin, 2007).
const LILLIEFORS_20: [f64; 47] = [
    0.3027, 0.2893, 0.2694, 0.2521, 0.2387, 0.2273, 0.2171, 0.2080, 0.2004, 0.1932, 0.1869, 0.1811,
    0.1758, 0.1711, 0.1666, 0.1624, 0.1589, 0.1553, 0.1517, 0.1484, 0.1458, 0.1429, 0.1406, 0.1381,
    0.1358, 0.1334, 0.1315, 0.1291, 0.1274, 0.1254, 0.1236, 0.1220, 0.1203, 0.1188, 0.1174, 0.1159,
    0.1147, 0.1131, 0.1119, 0.1106, 0.1095, 0.1083, 0.1071, 0.1062, 0.1047, 0.1040, 0.1030,
];

/// The critical value at 0.2 for `n` cases, at least 4: the table, linear between its rows,
/// and the published formula above 50.
fn lilliefors_20(n: f64) -> f64 {
    if n > 50.0 {
        return 0.741 / ((0.83 + n) / n.sqrt() - 0.01);
    }
    let low = (n.floor() as usize).clamp(4, 50);
    let high = (low + 1).min(50);
    let share = n - low as f64;
    (1.0 - share) * LILLIEFORS_20[low - 4] + share * LILLIEFORS_20[high - 4]
}

/// The Lilliefors significance of `d` for `n` cases (Dallal and Wilkinson's approximation).
pub(crate) fn lilliefors(d: f64, n: f64) -> Cell {
    if n < 4.0 {
        return Cell::Empty;
    }
    let (a, b, c) = if n <= 100.0 {
        (
            -7.01256 * (n + 2.78019),
            2.99587 * (n + 2.78019).sqrt(),
            2.1804661 + 0.974598 / n.sqrt() + 1.67997 / n,
        )
    } else {
        (
            -7.90289126054 * n.powf(0.98),
            3.180370175721 * n.powf(0.49),
            2.2947256,
        )
    };
    let critical = (-b - (b * b - 4.0 * a * c).sqrt()) / (2.0 * a);
    if d >= critical {
        // the manual writes ln 10 as 2.3025851, so p is 0.1 at the critical value
        return Cell::Number((a * d * d + b * d + c - std::f64::consts::LN_10).exp());
    }
    let low = lilliefors_20(n);
    if d >= low {
        return Cell::Number(0.2 - 0.1 * (d - low) / (critical - low));
    }
    Cell::Text(">0.200".into())
}

/// The coefficients a_n/2+1 .. a_n of W for `n` cases (the others are their negatives), by
/// Royston's approximation (1992; Applied Statistics algorithm AS R94, 1995).
pub(crate) fn shapiro_wilk_coefficients(n: usize) -> Vec<f64> {
    if n == 3 {
        return vec![0.0, std::f64::consts::FRAC_1_SQRT_2];
    }
    let nf = n as f64;
    let m: Vec<f64> = (1..=n)
        .map(|i| normal_quantile((i as f64 - 0.375) / (nf + 0.25)))
        .collect();
    let sum: f64 = m.iter().map(|v| v * v).sum();
    let u = 1.0 / nf.sqrt();
    let poly = |c: [f64; 5]| c.iter().rev().fold(0.0, |acc, c| (acc + c) * u);
    let last = m[n - 1] / sum.sqrt() + poly([0.221157, -0.147981, -2.071190, 4.434685, -2.706056]);
    let mut a: Vec<f64> = Vec::new();
    if n > 5 {
        let next =
            m[n - 2] / sum.sqrt() + poly([0.042981, -0.293762, -1.752461, 5.682633, -3.582633]);
        let rest = (sum - 2.0 * m[n - 1].powi(2) - 2.0 * m[n - 2].powi(2))
            / (1.0 - 2.0 * last * last - 2.0 * next * next);
        a.extend(m[n / 2..n - 2].iter().map(|v| v / rest.sqrt()));
        a.extend([next, last]);
    } else {
        let rest = (sum - 2.0 * m[n - 1].powi(2)) / (1.0 - 2.0 * last * last);
        a.extend(m[n / 2..n - 1].iter().map(|v| v / rest.sqrt()));
        a.push(last);
    }
    a
}

/// The significance of W for `n` cases (Royston, 1992 and 1995): exact for 3 cases, otherwise
/// a normal approximation of a transformation of 1 - W.
pub(crate) fn shapiro_wilk_p(w: f64, n: usize) -> f64 {
    if n == 3 {
        let p = 6.0 / std::f64::consts::PI * (w.sqrt().asin() - 0.75f64.sqrt().asin());
        return p.clamp(0.0, 1.0);
    }
    let nf = n as f64;
    let poly = |c: &[f64], x: f64| c.iter().rev().fold(0.0, |acc, c| acc * x + c);
    let y = (1.0 - w).ln();
    let (z, mean, sd) = if n <= 11 {
        let gamma = poly(&[-2.273, 0.459], nf);
        if y >= gamma {
            return 0.0;
        }
        (
            -(gamma - y).ln(),
            poly(&[0.5440, -0.39978, 0.025054, -6.714e-4], nf),
            poly(&[1.3822, -0.77857, 0.062767, -0.0020322], nf).exp(),
        )
    } else {
        let x = nf.ln();
        (
            y,
            poly(&[-1.5861, -0.31082, -0.083751, 0.0038915], x),
            poly(&[-0.4803, -0.082676, 0.0030302], x).exp(),
        )
    };
    // the upper tail, by erfc so that small significances keep their precision
    erfc((z - mean) / sd / std::f64::consts::SQRT_2) / 2.0
}

/// W and its significance for the series with weights rounded to whole numbers, None outside
/// its range (3 to 5000 cases, 3 to 50 with non-integer weights) or without spread. Also the
/// number of cases of the series.
pub(crate) fn shapiro_wilk(dist: &Distribution) -> (Option<(f64, f64)>, f64) {
    let rounded: Vec<f64> = dist.weights.iter().map(|c| c.round()).collect();
    let n: f64 = rounded.iter().sum();
    let whole = dist.weights.iter().all(|c| c.fract() == 0.0);
    if n < 3.0 || n > if whole { 5000.0 } else { 50.0 } {
        return (None, n);
    }
    let x: Vec<f64> = dist
        .values
        .iter()
        .zip(&rounded)
        .flat_map(|(y, c)| std::iter::repeat_n(*y, *c as usize))
        .collect();
    let count = x.len();
    let mean = x.iter().sum::<f64>() / n;
    let ss: f64 = x.iter().map(|v| (v - mean).powi(2)).sum();
    if ss <= 1e-20 * mean * mean {
        return (None, n);
    }
    let a = shapiro_wilk_coefficients(count);
    let half = count / 2;
    let upper = &a[a.len() - half..];
    let linear: f64 = (0..half)
        .map(|i| upper[half - 1 - i] * (x[count - 1 - i] - x[i]))
        .sum();
    let w = (linear * linear / ss).min(1.0);
    (Some((w, shapiro_wilk_p(w, count))), n)
}

/// A number with up to six decimals and no trailing zeros, as in "95" or "97.5".
pub(crate) fn short(x: f64) -> String {
    let text = format!("{x:.6}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn method_name(method: Method) -> &'static str {
    match method {
        Method::Haverage => "Weighted Average at (W+1)p",
        Method::Waverage => "Weighted Average at Wp",
        Method::Round => "Closest Observation",
        Method::Empirical => "Empirical Distribution Function",
        Method::Aempirical => "Empirical Distribution Function with Averaging",
    }
}

/// A category of a factor: a value, or the system-missing value under REPORT.
#[derive(Clone, PartialEq)]
enum Level {
    Value(Key),
    System,
}

impl Level {
    fn compare(&self, other: &Level) -> std::cmp::Ordering {
        match (self, other) {
            (Level::Value(a), Level::Value(b)) => a.compare(b),
            (Level::Value(_), Level::System) => std::cmp::Ordering::Less,
            (Level::System, Level::Value(_)) => std::cmp::Ordering::Greater,
            (Level::System, Level::System) => std::cmp::Ordering::Equal,
        }
    }

    fn name(&self, variable: &Variable, shown: &Shown) -> String {
        match self {
            Level::Value(key) => shown.value(variable, key),
            Level::System => "System Missing".into(),
        }
    }
}

/// The level of every case of a factor, None where it is missing and REPORT is off.
fn levels(variable: &Variable, report: bool) -> Vec<Option<Level>> {
    categories(variable, report)
        .into_iter()
        .map(|key| match key {
            Some(key) => Some(Level::Value(key)),
            None if report => Some(Level::System),
            None => None,
        })
        .collect()
}

/// What one dependent gives in one cell.
struct CellResult {
    /// The label of the dependent and of each factor level.
    labels: Vec<String>,
    /// The print format of the dependent.
    format: String,
    /// Which dependent, in the order of the command.
    dependent: usize,
    valid: f64,
    total: f64,
    /// (value, case) of the cases used, in case order.
    cases: Vec<(f64, usize)>,
    pairs: Vec<(f64, f64)>,
}

impl Engine {
    pub(crate) fn examine(&mut self, command: &Examine) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let mut dependents = self.expand(&command.dependents)?;
        dedup(&mut dependents);
        for name in &dependents {
            if data.variable(name)?.is_string() {
                return Err(Error(format!(
                    "EXAMINE needs numeric dependent variables, but {name} is a string variable"
                )));
            }
        }
        let mut terms: Vec<Vec<String>> = Vec::new();
        for term in &command.factors {
            let names = self.expand(term)?;
            if term.len() == 1 {
                terms.extend(names.into_iter().map(|name| vec![name]));
            } else {
                terms.push(names);
            }
        }
        // a repeated factor term is analysed once
        let mut seen: Vec<String> = Vec::new();
        terms.retain(|term| {
            let key = term.join(" BY ").to_ascii_uppercase();
            let new = !seen.contains(&key);
            seen.push(key);
            new
        });
        let id = command
            .id
            .as_ref()
            .map(|name| data.variable(name))
            .transpose()?;
        let weights = case_weights(data)?;
        let weight = |case: usize| weights.as_ref().map_or(1.0, |w| w[case]);
        let mut warnings = Vec::new();
        let level = if (50.0..=99.99).contains(&command.cinterval) {
            command.cinterval
        } else {
            warnings.push(format!(
                "CINTERVAL {} lies outside 50 to 99.99, so the 95% interval is shown",
                short(command.cinterval)
            ));
            95.0
        };
        if command.spread_level.is_some() && terms.is_empty() {
            warnings.push(
                "SPREADLEVEL needs a factor after BY, so no spread-versus-level plot is made"
                    .into(),
            );
        }
        let mut charts = Vec::new();
        let factor_names: Vec<&String> = terms.iter().flatten().collect();
        let factor_levels: Vec<Vec<Option<Level>>> = factor_names
            .iter()
            .map(|name| Ok(levels(&data.variable(name)?, command.report)))
            .collect::<Result<_, Error>>()?;
        let level_of = |name: &str| {
            let i = factor_names
                .iter()
                .position(|n| n.as_str() == name)
                .expect("every factor has its levels");
            &factor_levels[i]
        };
        let cases = data.case_count();
        let mut valid_dependent = Vec::new();
        for name in &dependents {
            let variable = data.variable(name)?;
            valid_dependent.push(
                (0..cases)
                    .map(|c| is_valid(&variable, c, command.include))
                    .collect::<Vec<bool>>(),
            );
        }
        // listwise: valid on every dependent and every factor of the command
        let complete: Vec<bool> = (0..cases)
            .map(|c| {
                valid_dependent.iter().all(|v| v[c]) && factor_levels.iter().all(|l| l[c].is_some())
            })
            .collect();
        let mut models: Vec<Vec<String>> = Vec::new();
        if terms.is_empty() || command.total {
            models.push(Vec::new());
        }
        models.extend(terms.iter().cloned());
        let mut out = Vec::new();
        for model in &models {
            let factors: Vec<Variable> = model
                .iter()
                .map(|name| data.variable(name))
                .collect::<Result<_, _>>()?;
            let model_levels: Vec<&Vec<Option<Level>>> =
                model.iter().map(|name| level_of(name)).collect();
            let cell_of = |case: usize| -> Option<Vec<Level>> {
                model_levels.iter().map(|l| l[case].clone()).collect()
            };
            let order = |a: &Vec<Level>, b: &Vec<Level>| {
                a.iter()
                    .zip(b)
                    .map(|(x, y)| x.compare(y))
                    .find(|o| o.is_ne())
                    .unwrap_or(std::cmp::Ordering::Equal)
            };
            let mut cells: Vec<Vec<Level>> = (0..cases)
                .filter(|&c| weight(c) > 0.0)
                .filter_map(cell_of)
                .collect();
            cells.sort_by(order);
            cells.dedup();
            // the cell of every counted case, found once
            let case_cell: Vec<Option<usize>> = (0..cases)
                .map(|c| {
                    let cell = cell_of(c).filter(|_| weight(c) > 0.0)?;
                    cells.binary_search_by(|probe| order(probe, &cell)).ok()
                })
                .collect();
            let mut results = Vec::new();
            for (d, name) in dependents.iter().enumerate() {
                let dependent = data.variable(name)?;
                let format = dependent.format();
                let Values::Numeric(values) = dependent.values() else {
                    unreachable!("string dependents were refused above")
                };
                let first = results.len();
                results.extend(cells.iter().map(|cell| {
                    CellResult {
                        labels: std::iter::once(shown.variable(&dependent))
                            .chain(cell.iter().zip(&factors).map(|(l, v)| l.name(v, &shown)))
                            .collect(),
                        format: format.to_string(),
                        dependent: d,
                        valid: 0.0,
                        total: 0.0,
                        cases: Vec::new(),
                        pairs: Vec::new(),
                    }
                }));
                for (case, cell) in case_cell.iter().enumerate() {
                    let Some(cell) = cell else { continue };
                    let result = &mut results[first + cell];
                    result.total += weight(case);
                    let used = if command.pairwise {
                        valid_dependent[d][case]
                    } else {
                        complete[case]
                    };
                    if used && let Some(value) = values.get(case) {
                        result.valid += weight(case);
                        result.cases.push((value, case));
                        result.pairs.push((value, weight(case)));
                    }
                }
            }
            let model: Vec<String> = model.iter().map(|n| shown.named(data, n)).collect();
            out.push(summary_table(&model, &results));
            let tables = Tables {
                command,
                names: &model,
                level,
                id: id.as_ref(),
                shown: &shown,
                numbers: self.case_numbers.as_deref(),
            };
            let used: Vec<&CellResult> = results.iter().filter(|r| r.valid > 0.0).collect();
            if used.is_empty() {
                continue;
            }
            if command.descriptives {
                out.push(tables.descriptives(&used));
            }
            if !command.mestimators.is_empty() {
                out.push(tables.mestimators(&used));
            }
            if let Some(percents) = &command.percentiles {
                out.push(tables.percentiles(&used, percents));
            }
            if let Some(n) = command.extreme {
                out.push(tables.extremes(&used, n));
            }
            if command.normality {
                out.push(tables.normality(&used));
            }
            tables.plots(&used, &mut charts, &mut out, &mut warnings);
        }
        self.charts.extend(charts);
        for warning in warnings {
            self.warn(warning);
        }
        Ok(out)
    }
}

fn dedup(names: &mut Vec<String>) {
    let mut seen: Vec<String> = Vec::new();
    names.retain(|name| {
        let key = name.to_ascii_uppercase();
        let new = !seen.contains(&key);
        seen.push(key);
        new
    });
}

/// The header of a table: the dependent and factor columns, `extra` empty label columns and
/// then the value columns, each with its column groups.
fn header(
    factors: &[String],
    extra: usize,
    values: &[(&str, Vec<String>)],
) -> (Vec<String>, Vec<Vec<String>>) {
    let mut columns = vec![String::new()];
    columns.extend(factors.iter().cloned());
    columns.extend(std::iter::repeat_n(String::new(), extra));
    let mut groups = vec![Vec::new(); columns.len()];
    for (name, group) in values {
        columns.push(name.to_string());
        groups.push(group.clone());
    }
    (columns, groups)
}

fn summary_table(factors: &[String], results: &[CellResult]) -> Table {
    let values: Vec<(&str, Vec<String>)> = ["Valid", "Missing", "Total"]
        .iter()
        .flat_map(|group| {
            ["N", "Percent"].map(|leaf| (leaf, vec!["Cases".to_string(), group.to_string()]))
        })
        .collect();
    let (columns, column_groups) = header(factors, 0, &values);
    let rows = results
        .iter()
        .map(|r| {
            let mut row: Vec<Cell> = r.labels.iter().cloned().map(Cell::Text).collect();
            for n in [r.valid, r.total - r.valid, r.total] {
                row.push(Cell::Number(n));
                row.push(number((r.total > 0.0).then(|| 100.0 * n / r.total)));
            }
            row
        })
        .collect();
    Table {
        title: "Case Processing Summary".into(),
        row_headers: factors.len() + 1,
        columns,
        column_groups,
        rows,
        ..Table::default()
    }
}

/// What every statistics table of one model needs.
struct Tables<'a> {
    command: &'a Examine,
    /// The factors of the model.
    names: &'a [String],
    level: f64,
    id: Option<&'a Variable<'a>>,
    shown: &'a Shown,
    /// The case numbers in the whole dataset when the procedure sees a part of it.
    numbers: Option<&'a [usize]>,
}

impl Tables<'_> {
    fn labels(&self, r: &CellResult) -> Vec<Cell> {
        r.labels.iter().cloned().map(Cell::Text).collect()
    }

    fn descriptives(&self, results: &[&CellResult]) -> Table {
        let interval = format!("{}% Confidence Interval for Mean", short(self.level));
        let mut rows = Vec::new();
        let mut formats = Vec::new();
        for r in results {
            let dist = Distribution::new(&r.pairs);
            let s = summarize(&r.pairs);
            let w = s.n;
            let se = s.sd.map(|sd| sd / w.sqrt());
            let margin = se
                .filter(|_| w > 1.0)
                .map(|se| se * t_quantile((1.0 - self.level / 100.0) / 2.0, w - 1.0));
            let method = self.command.method;
            let quartile = |p| dist.percentile(p, method);
            let iqr = quartile(75.0).zip(quartile(25.0)).map(|(h, l)| h - l);
            // each line with how its statistic and its standard error relate to the values
            let lines: [(&str, &str, Option<f64>, Option<f64>); 13] = [
                ("Mean", "", s.mean, se),
                (
                    &interval,
                    "Lower Bound",
                    s.mean.zip(margin).map(|(m, e)| m - e),
                    None,
                ),
                (
                    &interval,
                    "Upper Bound",
                    s.mean.zip(margin).map(|(m, e)| m + e),
                    None,
                ),
                ("5% Trimmed Mean", "", dist.trimmed_mean(), None),
                ("Median", "", quartile(50.0), None),
                ("Variance", "", s.sd.map(|sd| sd * sd), None),
                ("Std. Deviation", "", s.sd, None),
                ("Minimum", "", s.min, None),
                ("Maximum", "", s.max, None),
                ("Range", "", s.max.zip(s.min).map(|(h, l)| h - l), None),
                ("Interquartile Range", "", iqr, None),
                ("Skewness", "", s.skewness, s.se_skewness),
                ("Kurtosis", "", s.kurtosis, s.se_kurtosis),
            ];
            let kinds = [
                (Some(Stat::Value), Some(Stat::Spread)),
                (Some(Stat::Value), None),
                (Some(Stat::Value), None),
                (Some(Stat::Value), None),
                (Some(Stat::Value), None),
                (Some(Stat::Other), None),
                (Some(Stat::Spread), None),
                (Some(Stat::Value), None),
                (Some(Stat::Value), None),
                (Some(Stat::Spread), None),
                (Some(Stat::Spread), None),
                (None, None),
                (None, None),
            ];
            let shown = |kind: Option<Stat>| {
                kind.map_or_else(String::new, |kind| procedure_format(&r.format, kind))
            };
            for ((name, bound, value, error), (kind, error_kind)) in lines.iter().zip(kinds) {
                let mut row = self.labels(r);
                let mut format = vec![String::new(); row.len() + 2];
                row.push(Cell::Text(name.to_string()));
                row.push(Cell::Text(bound.to_string()));
                row.push(number(*value));
                row.push(number(*error));
                format.extend([shown(kind), shown(error_kind)]);
                rows.push(row);
                formats.push(format);
            }
        }
        let (columns, column_groups) = header(
            self.names,
            2,
            &[("Statistic", Vec::new()), ("Std. Error", Vec::new())],
        );
        Table {
            formats: formats_of(&rows, |r, c| formats[r][c].clone()),
            title: "Descriptives".into(),
            row_headers: self.names.len() + 3,
            columns,
            column_groups,
            rows,
            ..Table::default()
        }
    }

    fn mestimators(&self, results: &[&CellResult]) -> Table {
        let estimators = &self.command.mestimators;
        let rows: Vec<Vec<Cell>> = results
            .iter()
            .map(|r| {
                let dist = Distribution::new(&r.pairs);
                let mut row = self.labels(r);
                row.extend(
                    estimators
                        .iter()
                        .map(|e| number(m_estimate(&dist, *e, self.command.method))),
                );
                row
            })
            .collect();
        let names: Vec<(&str, Vec<String>)> = estimators
            .iter()
            .map(|e| {
                let name = match e {
                    MEstimator::Huber(_) => "Huber's M-Estimator",
                    MEstimator::Tukey(_) => "Tukey's Biweight",
                    MEstimator::Hampel(..) => "Hampel's M-Estimator",
                    MEstimator::Andrew(_) => "Andrews' Wave",
                };
                (name, Vec::new())
            })
            .collect();
        let constants: Vec<String> = estimators
            .iter()
            .map(|e| match e {
                MEstimator::Huber(k) => format!("Huber {}", short(*k)),
                MEstimator::Tukey(c) => format!("Tukey {}", short(*c)),
                MEstimator::Hampel(a, b, c) => {
                    format!("Hampel {}, {} and {}", short(*a), short(*b), short(*c))
                }
                MEstimator::Andrew(c) => format!("Andrews {} * pi", short(*c)),
            })
            .collect();
        let (columns, column_groups) = header(self.names, 0, &names);
        let labels = self.names.len() + 1;
        Table {
            formats: formats_of(&rows, |r, c| match c >= labels {
                true => procedure_format(&results[r].format, Stat::Value),
                false => String::new(),
            }),
            title: "M-Estimators".into(),
            row_headers: self.names.len() + 1,
            columns,
            column_groups,
            rows,
            caption: Some(format!("Weighting constants: {}.", constants.join("; "))),
        }
    }

    fn percentiles(&self, results: &[&CellResult], percents: &[f64]) -> Table {
        let method = self.command.method;
        let hinge_at = |p: f64| [25.0, 50.0, 75.0].iter().position(|&h| h == p);
        let hinges = percents.iter().any(|&p| hinge_at(p).is_some());
        let mut rows = Vec::new();
        let mut owners = Vec::new();
        for r in results {
            let dist = Distribution::new(&r.pairs);
            let mut row = self.labels(r);
            owners.push(r);
            if hinges {
                owners.push(r);
            }
            row.push(Cell::Text(method_name(method).into()));
            row.extend(percents.iter().map(|&p| number(dist.percentile(p, method))));
            rows.push(row);
            if hinges {
                let tukey = dist.hinges();
                let mut row = self.labels(r);
                row.push(Cell::Text("Tukey's Hinges".into()));
                row.extend(
                    percents
                        .iter()
                        .map(|&p| number(hinge_at(p).and_then(|i| tukey.map(|t| t[i])))),
                );
                rows.push(row);
            }
        }
        let labels: Vec<String> = percents.iter().map(|&p| short(p)).collect();
        let values: Vec<(&str, Vec<String>)> = labels
            .iter()
            .map(|l| (l.as_str(), vec!["Percentiles".to_string()]))
            .collect();
        let (columns, column_groups) = header(self.names, 1, &values);
        let labels = self.names.len() + 2;
        Table {
            formats: formats_of(&rows, |r, c| match c >= labels {
                true => procedure_format(&owners[r].format, Stat::Value),
                false => String::new(),
            }),
            title: "Percentiles".into(),
            row_headers: self.names.len() + 2,
            columns,
            column_groups,
            rows,
            ..Table::default()
        }
    }

    /// The label of a case: its number, or its value of the ID variable.
    fn case_label(&self, case: usize) -> Cell {
        let Some(id) = self.id else {
            let number = self.numbers.map_or(case, |numbers| numbers[case]);
            return Cell::Number(number as f64 + 1.0);
        };
        match id.values() {
            Values::Numeric(values) => match values.get(case) {
                None => Cell::Empty,
                Some(v) => {
                    let labelled = self.shown.number(id, v);
                    if labelled == v.to_string() {
                        Cell::Number(v)
                    } else {
                        Cell::Text(labelled)
                    }
                }
            },
            Values::String(values) => Cell::Text(values.get(case).trim_end().to_string()),
        }
    }

    fn extremes(&self, results: &[&CellResult], n: usize) -> Table {
        let mut rows = Vec::new();
        let mut owners = Vec::new();
        for r in results {
            let mut sorted = r.cases.clone();
            // stable: equal values keep the case order
            sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
            let count = n.min(sorted.len());
            let highest = sorted.iter().rev().take(count);
            let lowest = sorted.iter().take(count);
            for (side, list) in [
                ("Highest", highest.collect::<Vec<_>>()),
                ("Lowest", lowest.collect()),
            ] {
                for (rank, (value, case)) in list.into_iter().enumerate() {
                    let mut row = self.labels(r);
                    row.push(Cell::Text(side.into()));
                    row.push(Cell::Number(rank as f64 + 1.0));
                    row.push(self.case_label(*case));
                    row.push(Cell::Number(*value));
                    rows.push(row);
                    owners.push(r);
                }
            }
        }
        let id = self
            .id
            .map_or("Case Number".to_string(), |v| self.shown.variable(v));
        let (columns, column_groups) = header(
            self.names,
            2,
            &[(id.as_str(), Vec::new()), ("Value", Vec::new())],
        );
        Table {
            // the value is the last column
            formats: formats_of(&rows, |r, c| match c + 1 == rows[r].len() {
                true => procedure_format(&owners[r].format, Stat::Value),
                false => String::new(),
            }),
            title: "Extreme Values".into(),
            row_headers: self.names.len() + 3,
            columns,
            column_groups,
            rows,
            ..Table::default()
        }
    }

    fn normality(&self, results: &[&CellResult]) -> Table {
        let mut rows = Vec::new();
        for r in results {
            let dist = Distribution::new(&r.pairs);
            let s = summarize(&r.pairs);
            let mut row = self.labels(r);
            let spread = s.mean.zip(s.sd.filter(|sd| *sd > 0.0));
            match spread {
                Some((mean, sd)) => {
                    let d = kolmogorov_smirnov(&dist, mean, sd);
                    row.extend([Cell::Number(d), Cell::Number(s.n), lilliefors(d, s.n)]);
                }
                None => row.extend([Cell::Empty, Cell::Number(s.n), Cell::Empty]),
            }
            let (test, n) = shapiro_wilk(&dist);
            row.extend([
                number(test.map(|t| t.0)),
                Cell::Number(n),
                number(test.map(|t| t.1)),
            ]);
            rows.push(row);
        }
        let values: Vec<(&str, Vec<String>)> = ["Kolmogorov-Smirnov", "Shapiro-Wilk"]
            .iter()
            .flat_map(|test| ["Statistic", "df", "Sig."].map(|leaf| (leaf, vec![test.to_string()])))
            .collect();
        let (columns, column_groups) = header(self.names, 0, &values);
        Table {
            title: "Tests of Normality".into(),
            row_headers: self.names.len() + 1,
            columns,
            column_groups,
            rows,
            caption: Some(
                "Kolmogorov-Smirnov with the Lilliefors significance (Dallal and Wilkinson); a significance shown as >0.200 has 0.2 as its lower bound. Shapiro-Wilk by Royston's algorithm (AS R94)."
                    .into(),
            ),
            ..Table::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dist(pairs: &[(f64, f64)]) -> Distribution {
        Distribution::new(pairs)
    }

    #[test]
    fn percentile_methods_by_hand() {
        let d = dist(&[(3.0, 1.0), (1.0, 1.0), (4.0, 1.0), (2.0, 1.0)]);
        let at = |p, m| d.percentile(p, m).unwrap();
        // W p = 1 falls on a value, (W + 1) p = 1.25 between the first two
        assert_eq!(at(25.0, Method::Waverage), 1.0);
        assert_eq!(at(25.0, Method::Haverage), 1.25);
        assert_eq!(at(25.0, Method::Round), 1.0);
        assert_eq!(at(25.0, Method::Empirical), 1.0);
        assert_eq!(at(25.0, Method::Aempirical), 1.5);
        assert!((at(30.0, Method::Waverage) - 1.2).abs() < 1e-12);
        assert_eq!(at(30.0, Method::Haverage), 1.5);
        assert_eq!(at(30.0, Method::Round), 1.0);
        assert_eq!(at(30.0, Method::Empirical), 2.0);
        assert_eq!(at(30.0, Method::Aempirical), 2.0);
        // beyond the data the percentile stays at its ends
        assert_eq!(at(5.0, Method::Haverage), 1.0);
        assert_eq!(at(99.0, Method::Haverage), 4.0);
        // weights below 1 share the step by g* / c
        let d = dist(&[(1.0, 0.5), (2.0, 0.5), (3.0, 2.0)]);
        let at = |p, m| d.percentile(p, m).unwrap();
        assert_eq!(at(50.0, Method::Waverage), 2.5);
        assert_eq!(at(50.0, Method::Haverage), 3.0);
        assert!((at(20.0, Method::Waverage) - 1.2).abs() < 1e-12);
        assert_eq!(at(20.0, Method::Round), 1.0);
        assert_eq!(at(30.0, Method::Round), 2.0);
    }

    #[test]
    fn hinges_and_trimmed_mean_by_hand() {
        let d = dist(&[(1.0, 1.0), (2.0, 1.0), (3.0, 1.0), (4.0, 1.0)]);
        assert_eq!(d.hinges(), Some([1.5, 2.5, 3.5]));
        let ten: Vec<(f64, f64)> = (1..=10).map(|i| (f64::from(i), 1.0)).collect();
        assert!((dist(&ten).trimmed_mean().unwrap() - 5.5).abs() < 1e-12);
        let skewed: Vec<(f64, f64)> = (1..=20).map(|i| (f64::from(i * i), 1.0)).collect();
        let inner: f64 = (2..=19).map(|i| f64::from(i * i)).sum::<f64>() / 18.0;
        assert!((dist(&skewed).trimmed_mean().unwrap() - inner).abs() < 1e-9);
        assert_eq!(dist(&[(7.0, 3.0)]).trimmed_mean(), Some(7.0));
    }

    #[test]
    fn shapiro_wilk_coefficients_near_the_published_ones() {
        // exact coefficients of Shapiro and Wilk (1965), largest first
        let published: [(usize, &[f64]); 4] = [
            (4, &[0.6872, 0.1677]),
            (5, &[0.6646, 0.2413, 0.0]),
            (10, &[0.5739, 0.3291, 0.2141, 0.1224, 0.0399]),
            (20, &[0.4734, 0.3211, 0.2565, 0.2085, 0.1686]),
        ];
        for (n, exact) in published {
            let a = shapiro_wilk_coefficients(n);
            let middle = if n % 2 == 1 { a[0] * a[0] } else { 0.0 };
            let sum: f64 = 2.0 * a.iter().map(|v| v * v).sum::<f64>() - middle;
            assert!((sum - 1.0).abs() < 1e-12, "{n}: {sum}");
            for (i, e) in exact.iter().enumerate() {
                let got = a[a.len() - 1 - i];
                assert!((got - e).abs() < 2e-3, "{n} {i}: {got} vs {e}");
            }
        }
    }

    #[test]
    fn shapiro_wilk_significance() {
        // three cases: exact, 0 at the smallest possible W and 1 at W = 1
        assert_eq!(shapiro_wilk_p(0.75, 3), 0.0);
        assert!((shapiro_wilk_p(1.0, 3) - 1.0).abs() < 1e-12);
        let p = shapiro_wilk_p(0.9, 3);
        let exact =
            6.0 / std::f64::consts::PI * (0.9f64.sqrt().asin() - std::f64::consts::PI / 3.0);
        assert!((p - exact).abs() < 1e-15);
        // the significance falls as W falls, in both approximations
        for n in [4, 7, 11, 12, 50, 1000] {
            let ps: Vec<f64> = [0.7, 0.8, 0.9, 0.95, 0.99]
                .iter()
                .map(|&w| shapiro_wilk_p(w, n))
                .collect();
            assert!(ps.windows(2).all(|p| p[0] <= p[1]), "{n}: {ps:?}");
        }
    }

    #[test]
    fn lilliefors_significance_is_continuous() {
        for n in [4.0, 10.0, 37.5, 50.0, 80.0, 100.0, 101.0, 500.0] {
            let low = lilliefors_20(n);
            let (Cell::Number(at_low), Cell::Text(below)) =
                (lilliefors(low, n), lilliefors(low * 0.999, n))
            else {
                panic!("{n}")
            };
            assert!((at_low - 0.2).abs() < 1e-12 && below == ">0.200", "{n}");
            // the critical value of 0.1 is where both formulas meet
            let p = |d| match lilliefors(d, n) {
                Cell::Number(p) => p,
                other => panic!("{other:?}"),
            };
            let critical = crate::dist::solve(|d| 0.1 - p(d), low, 1.0);
            assert!((p(critical) - 0.1).abs() < 1e-6, "{n}");
            assert!(p(critical * 1.01) < 0.1 && p(critical * 0.99) > 0.1, "{n}");
        }
        assert_eq!(lilliefors(0.5, 3.0), Cell::Empty);
    }

    #[test]
    fn m_estimates_of_symmetric_data_are_the_centre() {
        let d = dist(&[
            (1.0, 1.0),
            (2.0, 2.0),
            (3.0, 4.0),
            (4.0, 2.0),
            (5.0, 1.0),
            (40.0, 0.0),
        ]);
        for e in [
            MEstimator::Huber(1.339),
            MEstimator::Tukey(4.685),
            MEstimator::Hampel(1.7, 3.4, 8.5),
            MEstimator::Andrew(1.34),
        ] {
            assert!((m_estimate(&d, e, Method::Haverage).unwrap() - 3.0).abs() < 1e-12);
        }
        // without spread around the median there is no scale
        assert_eq!(
            m_estimate(
                &dist(&[(1.0, 5.0), (9.0, 1.0)]),
                MEstimator::Huber(1.339),
                Method::Haverage
            ),
            None
        );
    }
}
