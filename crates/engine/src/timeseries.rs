//! The time series commands: TSET and TSHOW, CREATE, ACF, PACF, CCF and SEASON.
//!
//! Rules noted from the Command Syntax Reference and the Algorithms manual:
//! - TSET settings last for the session; a procedure's own subcommand wins over them. The
//!   period of seasonal work comes from the procedure, else TSET PERIOD, else the DATE
//!   variables.
//! - Autocorrelation r_k: the sum of products of deviations from the mean k cases apart over
//!   the sum of squares. With missing values only products of two valid values count, and m_k
//!   is their number (m_0 the valid values): standard error sqrt(m_k / ((m_0 + 2) m_0)) under
//!   white noise, sqrt((1 + 2 sum r_l^2, l < k) / m_0) under the MA (Bartlett) model; Box-Ljung
//!   m_0 (m_0 + 2) sum r_l^2 / m_l with k df.
//! - Partial autocorrelations by the Durbin-Levinson recursion, standard error 1 / sqrt(m_0).
//! - Cross-correlation at lag k: the products of x at t and y at t + k over the root of both
//!   sums of squares; standard error 1 / sqrt(n - |k|).
//! - CREATE: CSUM, DIFF, SDIFF, LAG, LEAD, MA, RMED, PMA, T4253H, FFT and IFFT as the
//!   Algorithms manual defines them; an even span of MA and RMED averages two neighbouring
//!   spans; MA, RMED and PMA treat a missing value as the end of one series and the start of
//!   another.
//! - SEASON: the ratio-to-moving-average method; seasonal factors from the mean of the
//!   seasonal-irregular values of a position without the largest and the smallest one.
//!
//! Choices where the manual is silent:
//! - The highest lag is at most the series length less one. SEASONAL shows the lags that are
//!   multiples of the period up to the highest lag.
//! - The missing-value form of the Bartlett standard error keeps the factor 2 of the complete
//!   form. CCF with missing values uses the cases where both series are valid; the standard
//!   error is 1 / sqrt(m), m the valid pairs at the lag.
//! - Charts show the coefficients as a line with limits at two standard errors.
//! - CREATE: LAG and LEAD with two sources and a range run over the orders within each source.
//!   An odd series gets a 0 at its end before FFT, with a warning. FFT gives n/2 cases from where the series starts, the highest frequency's cosine term in
//!   the first imaginary value; IFFT reads such a pair. MA and RMED shrink the span near the
//!   ends to the largest symmetric one, down to the minimum.
//! - SEASON's saved multiplicative factors are ratios (the tables show percentages).

use crate::chart::{Axis, Category, Series, Statistic};
use crate::descriptives::number;
use crate::dist::chi_square_p;
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_data::{Dataset, Measure};
use statscraft_syntax::{
    Command, Correlogram, CorrelogramKind, CreateEquation, NewVar, Season, SeriesFunction,
    TsetItem, TsetPrint,
};

/// The specifications of the last ACF, PACF, CCF and SEASON, for APPLY.
#[derive(Default, Clone)]
pub(crate) struct Models {
    correlograms: [Option<Box<Correlogram>>; 3],
    season: Option<Box<Season>>,
    pub(crate) exsmooth: Option<Box<statscraft_syntax::Exsmooth>>,
    pub(crate) arima: Option<crate::arima::Previous>,
}

/// A series in case order; None where missing.
pub(crate) type Values = Vec<Option<f64>>;

fn kind_index(kind: CorrelogramKind) -> usize {
    match kind {
        CorrelogramKind::Acf => 0,
        CorrelogramKind::Pacf => 1,
        CorrelogramKind::Ccf => 2,
    }
}

/// `x` differenced at `lag`.
fn difference(x: &[Option<f64>], lag: usize) -> Values {
    (0..x.len())
        .map(|i| Some(x[i]? - x[i.checked_sub(lag)?]?))
        .collect()
}

/// The values of a numeric variable; None where system-missing, or user-missing unless
/// `include`. Also the counts of user- and system-missing values.
pub(crate) fn read_series(
    data: &Dataset,
    name: &str,
    include: bool,
    command: &str,
) -> Result<(Values, usize, usize), Error> {
    let variable = data.variable(name)?;
    let values = variable.numeric().map_err(|_| {
        Error(format!(
            "{command} needs numeric series, but {name} is a string variable"
        ))
    })?;
    let (mut user, mut system) = (0, 0);
    let out = (0..data.case_count())
        .map(|k| match values.get(k) {
            None => {
                system += 1;
                None
            }
            Some(v) if !include && variable.is_user_missing(v) => {
                user += 1;
                None
            }
            Some(v) => Some(v),
        })
        .collect();
    Ok((out, user, system))
}

/// The valid stretch of a series: from its first valid value to its last.
fn trimmed(x: &[Option<f64>]) -> &[Option<f64>] {
    let first = x.iter().position(Option::is_some).unwrap_or(x.len());
    let last = x.iter().rposition(Option::is_some).map_or(first, |l| l + 1);
    &x[first..last]
}

/// Autocorrelations r_1..r_max and the counts m_0..m_max of valid products.
pub(crate) fn autocorrelations(x: &[Option<f64>], max: usize) -> (Vec<f64>, Vec<usize>) {
    let valid: Vec<f64> = x.iter().flatten().copied().collect();
    let mean = valid.iter().sum::<f64>() / valid.len() as f64;
    let a: Vec<Option<f64>> = x.iter().map(|v| v.map(|v| v - mean)).collect();
    let sum_sq: f64 = a.iter().flatten().map(|v| v * v).sum();
    let mut r = Vec::with_capacity(max);
    let mut m = vec![valid.len()];
    for k in 1..=max {
        let (mut s, mut count) = (0.0, 0);
        for i in 0..x.len().saturating_sub(k) {
            if let (Some(u), Some(v)) = (a[i], a[i + k]) {
                s += u * v;
                count += 1;
            }
        }
        r.push(s / sum_sq);
        m.push(count);
    }
    (r, m)
}

/// Partial autocorrelations from r_1.. by the Durbin-Levinson recursion.
pub(crate) fn partial_autocorrelations(r: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(r.len());
    let mut phi: Vec<f64> = Vec::new();
    for k in 0..r.len() {
        let num = r[k] - (0..k).map(|j| phi[j] * r[k - 1 - j]).sum::<f64>();
        let den = 1.0 - (0..k).map(|j| phi[j] * r[j]).sum::<f64>();
        let pkk = num / den;
        let next: Vec<f64> = (0..k).map(|j| phi[j] - pkk * phi[k - 1 - j]).collect();
        phi = next;
        phi.push(pkk);
        out.push(pkk);
    }
    out
}

/// Cross-correlations of x at t with y at t + k for k = -max..max, and the valid pairs at each.
pub(crate) fn cross_correlations(
    x: &[Option<f64>],
    y: &[Option<f64>],
    max: usize,
) -> (Vec<f64>, Vec<usize>) {
    // only the cases where both are valid
    let both: Vec<(Option<f64>, Option<f64>)> = x
        .iter()
        .zip(y)
        .map(|(a, b)| match (a, b) {
            (Some(a), Some(b)) => (Some(*a), Some(*b)),
            _ => (None, None),
        })
        .collect();
    let pairs: Vec<(f64, f64)> = both.iter().filter_map(|p| Some((p.0?, p.1?))).collect();
    let count = pairs.len() as f64;
    let mx = pairs.iter().map(|p| p.0).sum::<f64>() / count;
    let my = pairs.iter().map(|p| p.1).sum::<f64>() / count;
    let a: Vec<Option<f64>> = both.iter().map(|p| p.0.map(|v| v - mx)).collect();
    let b: Vec<Option<f64>> = both.iter().map(|p| p.1.map(|v| v - my)).collect();
    let ss = |v: &[Option<f64>]| v.iter().flatten().map(|d| d * d).sum::<f64>();
    let scale = (ss(&a) * ss(&b)).sqrt();
    let n = x.len() as isize;
    let (mut r, mut m) = (Vec::new(), Vec::new());
    for k in -(max as isize)..=(max as isize) {
        let (mut s, mut count) = (0.0, 0);
        for t in 0..n {
            let u = t + k;
            if u < 0 || u >= n {
                continue;
            }
            if let (Some(p), Some(q)) = (a[t as usize], b[u as usize]) {
                s += p * q;
                count += 1;
            }
        }
        r.push(s / scale);
        m.push(count);
    }
    (r, m)
}

// ---------------------------------------------------------------- CREATE functions

fn csum(x: &[Option<f64>]) -> Values {
    let mut total = 0.0;
    x.iter()
        .map(|v| {
            let v = (*v)?;
            total += v;
            Some(total)
        })
        .collect()
}

fn shift(x: &[Option<f64>], by: isize) -> Values {
    (0..x.len() as isize)
        .map(|i| {
            let j = i - by;
            (j >= 0 && j < x.len() as isize)
                .then(|| x[j as usize])
                .flatten()
        })
        .collect()
}

/// The runs of valid values: (start, values).
fn runs(x: &[Option<f64>]) -> Vec<(usize, Vec<f64>)> {
    let mut out: Vec<(usize, Vec<f64>)> = Vec::new();
    let mut open = false;
    for (i, v) in x.iter().enumerate() {
        match v {
            Some(v) if open => out.last_mut().expect("open").1.push(*v),
            Some(v) => {
                out.push((i, vec![*v]));
                open = true;
            }
            None => open = false,
        }
    }
    out
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    if n % 2 == 1 {
        s[n / 2]
    } else {
        (s[n / 2 - 1] + s[n / 2]) / 2.0
    }
}

/// A centered MA or RMED: `stat` over a span, an even span centered by averaging the spans
/// that end half a span either side; near the ends the largest symmetric span down to
/// `minimum`. Each run of valid values is a series of its own.
fn centered(x: &[Option<f64>], span: usize, minimum: usize, stat: fn(&[f64]) -> f64) -> Values {
    let even = span.is_multiple_of(2);
    // the minimum takes the parity of the span (1 always counts)
    let minimum = match (even, minimum.is_multiple_of(2)) {
        _ if minimum == 1 => 1,
        (true, false) | (false, true) => minimum + 1,
        _ => minimum,
    };
    let mut out = vec![None; x.len()];
    for (start, run) in runs(x) {
        let len = run.len();
        for j in 0..len {
            let h = j.min(len - 1 - j);
            let value = if even {
                let s = (2 * h).min(span);
                if s >= 2 && s >= minimum {
                    let half = s / 2;
                    let first = stat(&run[j - half..j + half]);
                    let second = stat(&run[j + 1 - half..=j + half]);
                    Some((first + second) / 2.0)
                } else {
                    (minimum == 1).then_some(run[j])
                }
            } else {
                let s = (2 * h + 1).min(span);
                let half = (s - 1) / 2;
                (s >= minimum).then(|| stat(&run[j - half..=j + half]))
            };
            out[start + j] = value;
        }
    }
    out
}

fn prior_mean(x: &[Option<f64>], span: usize) -> Values {
    let mut out = vec![None; x.len()];
    for (start, run) in runs(x) {
        for j in span..run.len() {
            out[start + j] = Some(mean(&run[j - span..j]));
        }
    }
    out
}

fn median3(a: f64, b: f64, c: f64) -> f64 {
    a.max(b).min(a.min(b).max(c))
}

/// One pass of the 4253H smoother.
fn smooth_4253h(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    // running median of 4 at the half points, then of 2
    let mut half = vec![0.0; n + 1];
    half[0] = x[0];
    half[1] = (x[0] + x[1]) / 2.0;
    for j in 2..=n - 2 {
        half[j] = median(&x[j - 2..j + 2]);
    }
    half[n - 1] = (x[n - 2] + x[n - 1]) / 2.0;
    half[n] = x[n - 1];
    let mut z1: Vec<f64> = (0..n).map(|j| (half[j] + half[j + 1]) / 2.0).collect();
    z1[0] = half[0];
    z1[n - 1] = half[n];
    // running median of 5
    let mut z2 = z1.clone();
    z2[1] = median(&z1[0..3]);
    z2[n - 2] = median(&z1[n - 3..n]);
    for j in 2..n - 2 {
        z2[j] = median(&z1[j - 2..=j + 2]);
    }
    // running median of 3, the ends by extrapolation
    let mut z3 = z2.clone();
    for j in 1..n - 1 {
        z3[j] = median3(z2[j - 1], z2[j], z2[j + 1]);
    }
    z3[0] = median3(3.0 * z3[1] - 2.0 * z3[2], z2[0], z3[1]);
    z3[n - 1] = median3(3.0 * z3[n - 2] - 2.0 * z3[n - 3], z2[n - 1], z3[n - 2]);
    // hanning
    let mut z4 = z3.clone();
    for j in 1..n - 1 {
        z4[j] = z3[j - 1] / 4.0 + z3[j] / 2.0 + z3[j + 1] / 4.0;
    }
    z4
}

/// T4253H: the smoother, then the smoothed residuals added back.
pub(crate) fn t4253h(x: &[f64]) -> Vec<f64> {
    let first = smooth_4253h(x);
    let residuals: Vec<f64> = x.iter().zip(&first).map(|(a, b)| a - b).collect();
    let second = smooth_4253h(&residuals);
    first.iter().zip(&second).map(|(a, b)| a + b).collect()
}

/// The discrete Fourier transform of an even-length series: n/2 values of the real and the
/// imaginary part, the highest frequency's cosine term (negated) in the first imaginary value.
// shortcut: a direct O(n^2) transform, fine up to some tens of thousands of cases; a fast
// transform of any length if longer series are needed
pub(crate) fn fft(x: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = x.len();
    let angle = |k: usize, t: usize| 2.0 * std::f64::consts::PI * ((k * t) % n) as f64 / n as f64;
    let nf = n as f64;
    let mut re = vec![0.0; n / 2];
    let mut im = vec![0.0; n / 2];
    for k in 0..n / 2 {
        re[k] = x
            .iter()
            .enumerate()
            .map(|(t, v)| v * angle(k, t).cos())
            .sum::<f64>()
            / nf;
        im[k] = -x
            .iter()
            .enumerate()
            .map(|(t, v)| v * angle(k, t).sin())
            .sum::<f64>()
            / nf;
    }
    let alternating: f64 = x
        .iter()
        .enumerate()
        .map(|(t, v)| if t.is_multiple_of(2) { *v } else { -v })
        .sum();
    im[0] = -alternating / nf;
    (re, im)
}

/// The inverse of `fft`: 2m values from m pairs.
pub(crate) fn ifft(re: &[f64], im: &[f64]) -> Vec<f64> {
    let m = re.len();
    let n = 2 * m;
    let angle = |k: usize, t: usize| 2.0 * std::f64::consts::PI * ((k * t) % n) as f64 / n as f64;
    (0..n)
        .map(|t| {
            let sign = if t.is_multiple_of(2) { 1.0 } else { -1.0 };
            let mut v = re[0] - im[0] * sign;
            for k in 1..m {
                v += 2.0 * (re[k] * angle(k, t).cos() - im[k] * angle(k, t).sin());
            }
            v
        })
        .collect()
}

/// SEASON's ratio-to-moving-average method on a complete series: the moving average, the
/// seasonal-irregular values and the factor of each position of the period (multiplicative in
/// percent).
pub(crate) fn seasonal_factors(
    x: &[f64],
    p: usize,
    additive: bool,
    centered: bool,
) -> (Values, Values, Vec<f64>) {
    let n = x.len();
    let pf = p as f64;
    // moving average (0-based t)
    let half = p / 2;
    let ma: Vec<Option<f64>> = (0..n)
        .map(|t| {
            if p % 2 == 1 {
                (t >= half && t + half < n).then(|| mean(&x[t - half..=t + half]))
            } else if centered {
                (t >= half && t + half < n).then(|| {
                    (x[t - half] + x[t + half]) / (2.0 * pf)
                        + x[t + 1 - half..t + half].iter().sum::<f64>() / pf
                })
            } else {
                (t >= half && t + half <= n).then(|| mean(&x[t - half..t + half]))
            }
        })
        .collect();
    let si: Vec<Option<f64>> = (0..n)
        .map(|t| {
            let z = ma[t]?;
            Some(if additive { x[t] - z } else { x[t] / z * 100.0 })
        })
        .collect();
    let raw: Vec<f64> = (0..p)
        .map(|s| {
            let mut v: Vec<f64> = (s..n).step_by(p).filter_map(|t| si[t]).collect();
            v.sort_by(f64::total_cmp);
            if v.len() > 2 {
                mean(&v[1..v.len() - 1])
            } else {
                mean(&v)
            }
        })
        .collect();
    let average = mean(&raw);
    let factors: Vec<f64> = raw
        .iter()
        .map(|f| {
            if additive {
                f - average
            } else {
                f * 100.0 / average
            }
        })
        .collect();
    (ma, si, factors)
}

/// The valid stretch of a series as numbers, and where it starts; an error for a gap inside.
fn without_gaps(x: &[Option<f64>], what: &str) -> Result<(usize, Vec<f64>), Error> {
    let start = x.iter().position(Option::is_some).unwrap_or(0);
    let values: Option<Vec<f64>> = trimmed(x).iter().copied().collect();
    match values {
        Some(v) => Ok((start, v)),
        None => Err(Error(format!(
            "{what} cannot have missing values inside the series"
        ))),
    }
}

/// The decimals of a format such as "F8.2".
fn decimals(format: &str) -> usize {
    format
        .split_once('.')
        .and_then(|(_, d)| d.parse().ok())
        .unwrap_or(0)
}

impl Engine {
    /// The period of seasonal work: the procedure's own, else TSET PERIOD, else DATE's.
    pub(crate) fn ts_period(&self, given: Option<u32>) -> Option<usize> {
        given
            .or(self.tset.period)
            .map(|p| p as usize)
            .or_else(|| self.dataset.as_ref().and_then(crate::date::date_period))
    }

    pub(crate) fn tset_command(&mut self, items: &[TsetItem]) {
        for item in items {
            self.tset.apply(item);
        }
    }

    pub(crate) fn tshow(&self) -> Vec<Table> {
        let t = &self.tset;
        let date = self.dataset.as_ref().and_then(crate::date::date_period);
        let print = match t.print {
            TsetPrint::Brief => "BRIEF",
            TsetPrint::Default => "DEFAULT",
            TsetPrint::Detailed => "DETAILED",
        };
        let newvar = match t.newvar {
            NewVar::Current => "CURRENT",
            NewVar::None => "NONE",
            NewVar::All => "ALL",
        };
        let n = |v: u32| Cell::Number(f64::from(v));
        let none = || text("None");
        let rows = vec![
            vec![text("PRINT"), text(print)],
            vec![text("NEWVAR"), text(newvar)],
            vec![text("MXAUTO"), n(t.mxauto)],
            vec![text("MXCROSS"), n(t.mxcross)],
            vec![text("MXNEWVARS"), n(t.mxnewvars)],
            vec![text("MXPREDICT"), n(t.mxpredict)],
            vec![
                text("MISSING"),
                text(if t.include { "INCLUDE" } else { "EXCLUDE" }),
            ],
            vec![text("CIN"), Cell::Number(t.cin)],
            vec![text("TOLER"), Cell::Number(t.toler)],
            vec![text("CNVERGE"), Cell::Number(t.cnverge)],
            vec![text("ACFSE"), text(if t.acfse_ma { "MA" } else { "IND" })],
            vec![text("PERIOD"), t.period.map_or_else(none, n)],
            vec![
                text("ID"),
                t.id.as_ref().map_or_else(none, |id| text(id.clone())),
            ],
            vec![
                text("CONSTANT"),
                text(if t.constant { "CONSTANT" } else { "NOCONSTANT" }),
            ],
            vec![
                text("Period of the DATE variables"),
                date.map_or_else(none, |p| Cell::Number(p as f64)),
            ],
            vec![
                text("USE"),
                self.ts_use.as_ref().map_or_else(
                    || text("All cases"),
                    |r| text(crate::ts_range::range_text(r)),
                ),
            ],
            vec![
                text("PREDICT"),
                self.ts_predict
                    .as_ref()
                    .map_or_else(none, |r| text(crate::ts_range::range_text(r))),
            ],
            vec![
                text("Period in use"),
                self.ts_period(None)
                    .map_or_else(none, |p| Cell::Number(p as f64)),
            ],
        ];
        vec![table(
            "Time Series Settings",
            1,
            plain(&["Setting", "Value"]),
            rows,
            None,
        )]
    }

    // ------------------------------------------------------------ CREATE

    pub(crate) fn create(&mut self, equations: &[CreateEquation]) -> Result<Vec<Table>, Error> {
        self.begin_pass()?;
        let before = self.dataset.clone();
        let result = self.create_series(equations);
        if result.is_err() {
            self.dataset = before.clone();
        }
        self.end_pass(before.as_ref());
        result
    }

    fn create_series(&mut self, equations: &[CreateEquation]) -> Result<Vec<Table>, Error> {
        let groups = self.case_groups()?;
        let include = self.tset.include;
        let mut rows = Vec::new();
        let mut warnings = Vec::new();
        for equation in equations {
            let f = equation.function;
            let sources = self.expand(&equation.sources)?;
            let data = self.active()?;
            let orders: Vec<u32> = match (f, equation.numbers.as_slice()) {
                (SeriesFunction::Lag | SeriesFunction::Lead, [a]) => vec![*a],
                (SeriesFunction::Lag | SeriesFunction::Lead, [a, b]) => (*a..=*b).collect(),
                _ => vec![0],
            };
            if f == SeriesFunction::Ifft && !sources.len().is_multiple_of(2) {
                return Err(Error(
                    "IFFT reads its series in pairs: a real part, then an imaginary part".into(),
                ));
            }
            let made = match f {
                SeriesFunction::Fft => sources.len() * 2,
                SeriesFunction::Ifft => sources.len() / 2,
                _ => sources.len() * orders.len(),
            };
            if made != equation.targets.len() {
                return Err(Error(format!(
                    "{} makes {made} series here, but {} new names are given",
                    f.name(),
                    equation.targets.len()
                )));
            }
            if let Some(name) = crate::variables::duplicate(&equation.targets) {
                return Err(Error(format!("{name} is named twice as a new series")));
            }
            for target in &equation.targets {
                if crate::rows::is_scratch(target) || target.starts_with('$') {
                    return Err(Error(format!(
                        "{target} cannot be a series: scratch and system names are not allowed"
                    )));
                }
                if data.variable(target).is_ok_and(|v| v.is_string()) {
                    return Err(Error(format!(
                        "{target} is a string variable and cannot hold a series"
                    )));
                }
            }
            let period = match (f, equation.numbers.get(1)) {
                (SeriesFunction::Sdiff, Some(p)) => Some(*p as usize),
                (SeriesFunction::Sdiff, None) => Some(self.ts_period(None).ok_or_else(|| {
                    Error(
                        "SDIFF needs a period: give it as the third number, with TSET PERIOD or \
                         with DATE"
                            .into(),
                    )
                })?),
                _ => None,
            };
            let columns: Vec<(Values, String)> = sources
                .iter()
                .map(|s| {
                    let (values, _, _) = read_series(data, s, include, "CREATE")?;
                    Ok((values, data.variable(s)?.format().to_string()))
                })
                .collect::<Result<_, Error>>()?;
            // per new series: label, format and values over the whole dataset
            let mut outputs: Vec<(String, String, Values)> = Vec::new();
            let cases = data.case_count();
            let number = |i: usize| equation.numbers.get(i).copied().unwrap_or(0) as usize;
            let d = |format: &str| decimals(format);
            match f {
                SeriesFunction::Fft => {
                    let mut padded: Vec<String> = Vec::new();
                    for (i, (x, _)) in columns.iter().enumerate() {
                        let (mut re_all, mut im_all) = (vec![None; cases], vec![None; cases]);
                        for group in &groups {
                            let part: Values = group.iter().map(|&c| x[c]).collect();
                            if part.iter().all(Option::is_none) {
                                continue;
                            }
                            let (start, mut v) = without_gaps(&part, "FFT")?;
                            // an odd length gets a 0 at the end, as documented
                            if v.len() % 2 == 1 {
                                v.push(0.0);
                                padded.push(sources[i].clone());
                            }
                            let (re, im) = fft(&v);
                            for (k, (a, b)) in re.iter().zip(&im).enumerate() {
                                let c = group[start + k];
                                re_all[c] = Some(*a);
                                im_all[c] = Some(*b);
                            }
                        }
                        let s = &sources[i];
                        outputs.push((format!("FFT({s}), real part"), "F10.4".into(), re_all));
                        outputs.push((format!("FFT({s}), imaginary part"), "F10.4".into(), im_all));
                    }
                    padded.dedup();
                    for name in padded {
                        warnings.push(format!(
                            "FFT: {name} has an odd number of values, so a 0 was added at its end"
                        ));
                    }
                }
                SeriesFunction::Ifft => {
                    for pair in 0..sources.len() / 2 {
                        let (re, im) = (&columns[2 * pair].0, &columns[2 * pair + 1].0);
                        let mut out = vec![None; cases];
                        for group in &groups {
                            if !group.iter().any(|&c| re[c].is_some() || im[c].is_some()) {
                                continue;
                            }
                            if group.iter().any(|&c| re[c].is_some() != im[c].is_some()) {
                                return Err(Error(
                                    "IFFT needs the real and the imaginary part valid at the same cases"
                                        .into(),
                                ));
                            }
                            let both: Values = group.iter().map(|&c| re[c]).collect();
                            let (start, _) = without_gaps(&both, "IFFT")?;
                            let len = trimmed(&both).len();
                            let a: Vec<f64> = (0..len)
                                .map(|k| re[group[start + k]].expect("valid"))
                                .collect();
                            let b: Vec<f64> = (0..len)
                                .map(|k| im[group[start + k]].expect("valid"))
                                .collect();
                            let values = ifft(&a, &b);
                            if start + values.len() > group.len() {
                                return Err(Error(format!(
                                    "IFFT of {} values needs {} cases from case {}, but the series \
                                     has only {} there; leave the second half of the pair missing",
                                    len,
                                    values.len(),
                                    group[start] + 1,
                                    group.len() - start
                                )));
                            }
                            for (k, v) in values.into_iter().enumerate() {
                                out[group[start + k]] = Some(v);
                            }
                        }
                        let label =
                            format!("IFFT({}, {})", sources[2 * pair], sources[2 * pair + 1]);
                        outputs.push((label, "F10.4".into(), out));
                    }
                }
                _ => {
                    for (i, (x, format)) in columns.iter().enumerate() {
                        for &order in &orders {
                            let mut out = vec![None; cases];
                            for group in &groups {
                                let part: Values = group.iter().map(|&c| x[c]).collect();
                                let values = match f {
                                    SeriesFunction::Csum => csum(&part),
                                    SeriesFunction::Diff => {
                                        (0..number(0)).fold(part, |s, _| difference(&s, 1))
                                    }
                                    SeriesFunction::Sdiff => {
                                        let p = period.expect("set for SDIFF");
                                        (0..number(0)).fold(part, |s, _| difference(&s, p))
                                    }
                                    SeriesFunction::Lag => shift(&part, order as isize),
                                    SeriesFunction::Lead => shift(&part, -(order as isize)),
                                    SeriesFunction::Ma => centered(
                                        &part,
                                        number(0),
                                        equation.numbers.get(1).map_or(number(0), |&m| m as usize),
                                        mean,
                                    ),
                                    SeriesFunction::Rmed => centered(
                                        &part,
                                        number(0),
                                        equation.numbers.get(1).map_or(number(0), |&m| m as usize),
                                        median,
                                    ),
                                    SeriesFunction::Pma => prior_mean(&part, number(0)),
                                    SeriesFunction::T4253h => {
                                        let mut values = vec![None; part.len()];
                                        if part.iter().any(Option::is_some) {
                                            let (start, v) = without_gaps(&part, "T4253H")?;
                                            if v.len() < 4 {
                                                return Err(Error(format!(
                                                    "T4253H needs at least 4 values of {}",
                                                    sources[i]
                                                )));
                                            }
                                            for (k, s) in t4253h(&v).into_iter().enumerate() {
                                                values[start + k] = Some(s);
                                            }
                                        }
                                        values
                                    }
                                    SeriesFunction::Fft | SeriesFunction::Ifft => {
                                        unreachable!("handled above")
                                    }
                                };
                                for (k, &c) in group.iter().enumerate() {
                                    out[c] = values[k];
                                }
                            }
                            let s = &sources[i];
                            let (label, format) = match f {
                                SeriesFunction::Lag | SeriesFunction::Lead => {
                                    (format!("{}({s},{order})", f.name()), format.clone())
                                }
                                SeriesFunction::Csum
                                | SeriesFunction::Diff
                                | SeriesFunction::Sdiff => {
                                    let numbers = match f {
                                        SeriesFunction::Csum => String::new(),
                                        SeriesFunction::Diff => format!(",{}", number(0)),
                                        _ => format!(
                                            ",{},{}",
                                            number(0),
                                            period.expect("set for SDIFF")
                                        ),
                                    };
                                    (
                                        format!("{}({s}{numbers})", f.name()),
                                        format!("F10.{}", d(format).min(16)),
                                    )
                                }
                                _ => {
                                    let numbers: String =
                                        equation.numbers.iter().map(|n| format!(",{n}")).collect();
                                    (
                                        format!("{}({s}{numbers})", f.name()),
                                        format!("F10.{}", (d(format) + 2).min(16)),
                                    )
                                }
                            };
                            outputs.push((label, format, out));
                        }
                    }
                }
            }
            // filtered-out cases keep the value they had (none for a new series)
            let data = self.active_mut()?;
            let kept: Vec<bool> = {
                let mut k = vec![false; cases];
                for c in groups.iter().flatten() {
                    k[*c] = true;
                }
                k
            };
            for (target, (label, format, values)) in equation.targets.iter().zip(outputs) {
                let old = data
                    .variable(target)
                    .ok()
                    .map(|v| v.numeric().map(|n| (0..cases).map(|c| n.get(c)).collect()));
                let old: Values = match old {
                    Some(o) => o?,
                    None => vec![None; cases],
                };
                let merged: Values = (0..cases)
                    .map(|c| if kept[c] { values[c] } else { old[c] })
                    .collect();
                let valid: Vec<usize> = (0..cases).filter(|&c| merged[c].is_some()).collect();
                data.set_numeric_values(target, merged)?;
                data.set_variable_label(target, Some(&label))?;
                data.set_format(target, &format)?;
                data.set_measure(target, Measure::Scale)?;
                let case =
                    |c: Option<&usize>| c.map_or(Cell::Empty, |c| Cell::Number(*c as f64 + 1.0));
                rows.push(vec![
                    text(target.clone()),
                    case(valid.first()),
                    case(valid.last()),
                    Cell::Number(valid.len() as f64),
                    text(label),
                ]);
            }
        }
        for w in warnings {
            self.warn(w);
        }
        Ok(vec![table(
            "Created Series",
            1,
            vec![
                (String::new(), "Series Name".into()),
                ("Case Number of Valid Values".into(), "First".into()),
                ("Case Number of Valid Values".into(), "Last".into()),
                (String::new(), "N of Valid Cases".into()),
                (String::new(), "Creating Function".into()),
            ],
            rows,
            None,
        )])
    }

    // ------------------------------------------------------------ ACF, PACF, CCF

    /// The command over the previous one of its kind for APPLY; keeps the result.
    pub(crate) fn correlogram_spec(&mut self, command: &Correlogram) -> Result<Correlogram, Error> {
        let slot = kind_index(command.kind);
        let name = command.kind.name();
        let mut given = match &command.apply {
            None => command.clone(),
            Some(Some(model)) => {
                return Err(Error(format!(
                    "{name} APPLY of the named model '{model}' is not supported yet; APPLY \
                     without a name uses the previous {name}"
                )));
            }
            Some(None) => {
                let Some(previous) = &self.ts_models.correlograms[slot] else {
                    return Err(Error(format!(
                        "{name} APPLY: there is no earlier {name} to apply"
                    )));
                };
                let mut given = (**previous).clone();
                if !command.variables.is_empty() {
                    given.variables.clone_from(&command.variables);
                    given.with.clone_from(&command.with);
                }
                macro_rules! over {
                    ($($field:ident),*) => {
                        $(if command.$field.is_some() {
                            given.$field = command.$field;
                        })*
                    };
                }
                over!(diff, sdiff, period, ln, seasonal, max_lag, ma_se, pacf);
                given
            }
        };
        given.apply = None;
        self.ts_models.correlograms[slot] = Some(Box::new(given.clone()));
        Ok(given)
    }

    pub(crate) fn correlogram(&mut self, spec: &Correlogram) -> Result<Vec<Table>, Error> {
        let kind = spec.kind;
        let name = kind.name();
        let data = self.active()?;
        let shown = self.shown();
        let include = self.tset.include;
        let mut warnings = Vec::new();
        let mut period = self.ts_period(spec.period);
        let mut sdiff = spec.sdiff.unwrap_or(0) as usize;
        let mut seasonal = spec.seasonal.unwrap_or(false);
        if period.is_none() && (sdiff > 0 || seasonal) {
            warnings.push(format!(
                "{name}: no period is set (PERIOD, TSET PERIOD or DATE), so SDIFF and SEASONAL are left out"
            ));
            (sdiff, seasonal) = (0, false);
        }
        if period == Some(1) && (sdiff > 0 || seasonal) {
            warnings.push(format!(
                "{name}: a period of 1 has no seasons, so SDIFF and SEASONAL are left out"
            ));
            (sdiff, seasonal) = (0, false);
            period = None;
        }
        let diff = spec.diff.unwrap_or(0) as usize;
        let ln = spec.ln.unwrap_or(false);
        let max_lag = spec.max_lag.unwrap_or(match kind {
            CorrelogramKind::Ccf => self.tset.mxcross,
            _ => self.tset.mxauto,
        }) as usize;
        let ma_se = kind == CorrelogramKind::Acf && spec.ma_se.unwrap_or(self.tset.acfse_ma);
        let with_pacf = kind == CorrelogramKind::Pacf || spec.pacf.unwrap_or(false);
        let with_acf = kind == CorrelogramKind::Acf;

        let names = self.expand(&spec.variables)?;
        let with = match &spec.with {
            Some(items) => Some(self.expand(items)?),
            None => None,
        };
        let mut all: Vec<String> = names.clone();
        all.extend(with.iter().flatten().cloned());
        let mut unique: Vec<String> = Vec::new();
        for n in &all {
            if !unique.iter().any(|u| u.eq_ignore_ascii_case(n)) {
                unique.push(n.clone());
            }
        }
        // each series: the prepared values, or None when LN cannot apply
        let mut prepared: Vec<(String, Option<Values>, usize, usize)> = Vec::new();
        for n in &unique {
            let (mut x, user, system) = read_series(data, n, include, name)?;
            let mut usable = true;
            if ln {
                if x.iter().flatten().any(|v| *v <= 0.0) {
                    warnings.push(format!(
                        "{name}: {n} has values not above 0 and cannot be logged; it is left out"
                    ));
                    usable = false;
                } else {
                    x = x.iter().map(|v| v.map(f64::ln)).collect();
                }
            }
            for _ in 0..diff {
                x = difference(&x, 1);
            }
            if let Some(p) = period {
                for _ in 0..sdiff {
                    x = difference(&x, p);
                }
            }
            prepared.push((n.clone(), usable.then_some(x), user, system));
        }
        let find = |n: &str| {
            prepared
                .iter()
                .find(|p| p.0.eq_ignore_ascii_case(n))
                .expect("prepared")
        };
        let label = |n: &str| shown.named(data, n);
        let lags_shown = |k: usize| !seasonal || period.is_some_and(|p| k.is_multiple_of(p));
        let mut tables = Vec::new();
        let mut charts = Vec::new();

        // model and cases
        let text_cell = |s: &str| text(s);
        let mut model = vec![
            vec![
                text_cell("Series"),
                text(
                    unique
                        .iter()
                        .map(|n| label(n))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            ],
            vec![
                text_cell("Transformation"),
                text_cell(if ln { "Natural logarithm" } else { "None" }),
            ],
            vec![
                text_cell("Non-seasonal differencing"),
                Cell::Number(diff as f64),
            ],
            vec![
                text_cell("Seasonal differencing"),
                Cell::Number(sdiff as f64),
            ],
            vec![
                text_cell("Length of seasonal period"),
                period.map_or_else(|| text_cell("No periodicity"), |p| Cell::Number(p as f64)),
            ],
            vec![
                text_cell("Maximum number of lags"),
                Cell::Number(max_lag as f64),
            ],
        ];
        if with_acf {
            model.push(vec![
                text_cell("Standard error method"),
                text_cell(if ma_se {
                    "Bartlett's approximation (MA)"
                } else {
                    "Independence (white noise)"
                }),
            ]);
        }
        model.push(vec![
            text_cell("Lags shown"),
            text_cell(if seasonal {
                "Seasonal lags only"
            } else {
                "All lags"
            }),
        ]);
        tables.push(table(
            "Model Description",
            1,
            plain(&["", "Specification"]),
            model,
            None,
        ));
        let mut cases_rows: Vec<Vec<Cell>> = [
            "Series length",
            "User-missing values",
            "System-missing values",
            "Valid values after transformation",
        ]
        .iter()
        .map(|s| vec![text(*s)])
        .collect();
        for (_, x, user, system) in &prepared {
            cases_rows[0].push(Cell::Number(data.case_count() as f64));
            cases_rows[1].push(Cell::Number(*user as f64));
            cases_rows[2].push(Cell::Number(*system as f64));
            cases_rows[3].push(x.as_ref().map_or(Cell::Empty, |x| {
                Cell::Number(x.iter().flatten().count() as f64)
            }));
        }
        let mut columns = vec![(String::new(), String::new())];
        columns.extend(unique.iter().map(|n| (String::new(), label(n))));
        tables.push(table(
            "Case Processing Summary",
            1,
            columns,
            cases_rows,
            data.weight_variable()
                .is_some()
                .then(|| format!("{name} does not use case weights.")),
        ));

        let limits_chart = |title: String, lags: &[isize], values: &[f64], se: &[f64]| {
            let categories = lags
                .iter()
                .map(|k| Category {
                    label: k.to_string(),
                    value: Cell::Number(*k as f64),
                    missing: false,
                    bin: None,
                })
                .collect();
            let series = |name: &str, v: Vec<f64>| Series {
                name: name.into(),
                statistic: Statistic::Value,
                variable: None,
                value: Cell::Empty,
                values: v,
            };
            Chart::lines(
                title,
                Axis::titled("Lag"),
                Axis::titled("Coefficient"),
                categories,
                vec![
                    series("Coefficient", values.to_vec()),
                    series(
                        "Upper Confidence Limit",
                        se.iter().map(|s| 2.0 * s).collect(),
                    ),
                    series(
                        "Lower Confidence Limit",
                        se.iter().map(|s| -2.0 * s).collect(),
                    ),
                ],
            )
        };

        if kind == CorrelogramKind::Ccf {
            let pairs: Vec<(String, String)> = match &with {
                Some(right) => names
                    .iter()
                    .flat_map(|a| right.iter().map(move |b| (a.clone(), b.clone())))
                    .collect(),
                None => (0..names.len())
                    .flat_map(|i| {
                        let names = &names;
                        (i + 1..names.len()).map(move |j| (names[i].clone(), names[j].clone()))
                    })
                    .collect(),
            };
            if pairs.is_empty() {
                return Err(Error(
                    "CCF needs at least two series, or series on both sides of WITH".into(),
                ));
            }
            for (a, b) in pairs {
                let (Some(x), Some(y)) = (&find(&a).1, &find(&b).1) else {
                    continue;
                };
                let both = x.iter().zip(y).filter(|(u, v)| u.is_some() && v.is_some());
                let count = both.count();
                // the cases from the first to the last where both are valid
                let first = (0..x.len()).find(|&i| x[i].is_some() && y[i].is_some());
                let last = (0..x.len()).rfind(|&i| x[i].is_some() && y[i].is_some());
                let (Some(first), Some(last)) = (first, last) else {
                    warnings.push(format!(
                        "{name}: {a} and {b} have no valid values in common"
                    ));
                    continue;
                };
                if count < 2 {
                    warnings.push(format!(
                        "{name}: {a} and {b} have fewer than two valid values in common"
                    ));
                    continue;
                }
                let (xs, ys) = (&x[first..=last], &y[first..=last]);
                let varies = |s: &[Option<f64>]| {
                    let v: Vec<f64> = s.iter().flatten().copied().collect();
                    v.iter().any(|a| *a != v[0])
                };
                if !varies(xs) || !varies(ys) {
                    warnings.push(format!(
                        "{name}: {a} or {b} does not vary, so the pair is left out"
                    ));
                    continue;
                }
                let k = max_lag.min(xs.len() - 1);
                let (r, m) = cross_correlations(xs, ys, k);
                let mut rows = Vec::new();
                let (mut lags, mut values, mut ses) = (Vec::new(), Vec::new(), Vec::new());
                for (i, lag) in (-(k as isize)..=(k as isize)).enumerate() {
                    if !lags_shown(lag.unsigned_abs()) && lag != 0 {
                        continue;
                    }
                    let se = (m[i] > 0).then(|| 1.0 / (m[i] as f64).sqrt());
                    rows.push(vec![
                        Cell::Number(lag as f64),
                        number(Some(r[i])),
                        number(se),
                    ]);
                    lags.push(lag);
                    values.push(r[i]);
                    ses.push(se.unwrap_or(f64::NAN));
                }
                let pair = format!("{} with {}", label(&a), label(&b));
                tables.push(table(
                    "Cross Correlations",
                    1,
                    plain(&["Lag", "Cross Correlation", "Std. Error"]),
                    rows,
                    Some(format!(
                        "Series pair: {pair}. Positive lags: {} leads. The standard error assumes \
                         the series are not cross-correlated and one of them is white noise.",
                        label(&a)
                    )),
                ));
                charts.push(limits_chart(format!("CCF: {pair}"), &lags, &values, &ses));
            }
        } else {
            for n in &names {
                let Some(x) = &find(n).1 else { continue };
                let x = trimmed(x);
                let m0 = x.iter().flatten().count();
                if m0 < 2 {
                    warnings.push(format!(
                        "{name}: {n} has fewer than two valid values after the transformation"
                    ));
                    continue;
                }
                if x.iter().flatten().all(|v| Some(*v) == x[0]) {
                    warnings.push(format!(
                        "{name}: {n} does not vary and has no autocorrelations"
                    ));
                    continue;
                }
                let k_max = max_lag.min(x.len() - 1);
                let (r, m) = autocorrelations(x, k_max);
                let n0 = m0 as f64;
                let shown_lags: Vec<usize> = (1..=k_max).filter(|&k| lags_shown(k)).collect();
                if shown_lags.is_empty() {
                    warnings.push(format!(
                        "{name}: no lag of {n} up to {k_max} is a multiple of the period"
                    ));
                    continue;
                }
                let title_label = label(n);
                if with_acf {
                    let mut rows = Vec::new();
                    let mut q = 0.0;
                    let mut q_at = vec![f64::NAN; k_max + 1];
                    for k in 1..=k_max {
                        q += match m[k] {
                            0 => f64::NAN,
                            mk => r[k - 1] * r[k - 1] / mk as f64,
                        };
                        q_at[k] = n0 * (n0 + 2.0) * q;
                    }
                    let se = |k: usize| -> f64 {
                        if ma_se {
                            let s: f64 = r[..k - 1].iter().map(|v| v * v).sum();
                            ((1.0 + 2.0 * s) / n0).sqrt()
                        } else {
                            (m[k] as f64 / ((n0 + 2.0) * n0)).sqrt()
                        }
                    };
                    let (mut values, mut ses) = (Vec::new(), Vec::new());
                    for &k in &shown_lags {
                        let df = k as f64;
                        rows.push(vec![
                            Cell::Number(k as f64),
                            number(Some(r[k - 1])),
                            number(Some(se(k))),
                            number(Some(q_at[k])),
                            Cell::Number(df),
                            number(Some(chi_square_p(q_at[k], df))),
                        ]);
                        values.push(r[k - 1]);
                        ses.push(se(k));
                    }
                    let method = if ma_se {
                        "Bartlett's approximation (MA)"
                    } else {
                        "white noise"
                    };
                    tables.push(table(
                        "Autocorrelations",
                        1,
                        vec![
                            (String::new(), "Lag".into()),
                            (String::new(), "Autocorrelation".into()),
                            (String::new(), "Std. Error".into()),
                            ("Box-Ljung Statistic".into(), "Value".into()),
                            ("Box-Ljung Statistic".into(), "df".into()),
                            ("Box-Ljung Statistic".into(), "Sig.".into()),
                        ],
                        rows,
                        Some(format!(
                            "Series: {title_label}. Standard errors under {method}; the \
                             significance is from the chi-square distribution."
                        )),
                    ));
                    let lags: Vec<isize> = shown_lags.iter().map(|&k| k as isize).collect();
                    charts.push(limits_chart(
                        format!("ACF: {title_label}"),
                        &lags,
                        &values,
                        &ses,
                    ));
                }
                if with_pacf {
                    let p = partial_autocorrelations(&r);
                    let se = 1.0 / n0.sqrt();
                    let rows = shown_lags
                        .iter()
                        .map(|&k| {
                            vec![
                                Cell::Number(k as f64),
                                number(Some(p[k - 1])),
                                Cell::Number(se),
                            ]
                        })
                        .collect();
                    tables.push(table(
                        "Partial Autocorrelations",
                        1,
                        plain(&["Lag", "Partial Autocorrelation", "Std. Error"]),
                        rows,
                        Some(format!("Series: {title_label}.")),
                    ));
                    let lags: Vec<isize> = shown_lags.iter().map(|&k| k as isize).collect();
                    let values: Vec<f64> = shown_lags.iter().map(|&k| p[k - 1]).collect();
                    charts.push(limits_chart(
                        format!("PACF: {title_label}"),
                        &lags,
                        &values,
                        &vec![se; lags.len()],
                    ));
                }
            }
        }
        for w in warnings {
            self.warn(w);
        }
        self.charts.extend(charts);
        Ok(tables)
    }

    // ------------------------------------------------------------ SEASON

    pub(crate) fn season_spec(&mut self, command: &Season) -> Result<Season, Error> {
        let mut given = match &command.apply {
            None => command.clone(),
            Some(Some(model)) => {
                return Err(Error(format!(
                    "SEASON APPLY of the named model '{model}' is not supported yet; APPLY \
                     without a name uses the previous SEASON"
                )));
            }
            Some(None) => {
                let Some(previous) = &self.ts_models.season else {
                    return Err(Error(
                        "SEASON APPLY: there is no earlier SEASON to apply".into(),
                    ));
                };
                let mut given = (**previous).clone();
                if !command.variables.is_empty() {
                    given.variables.clone_from(&command.variables);
                }
                if command.additive.is_some() {
                    given.additive = command.additive;
                }
                if command.centered.is_some() {
                    given.centered = command.centered;
                }
                if command.period.is_some() {
                    given.period = command.period;
                }
                given
            }
        };
        given.apply = None;
        self.ts_models.season = Some(Box::new(given.clone()));
        Ok(given)
    }

    /// Saves the new series of a time series procedure as TSET NEWVAR says: per item, one
    /// variable per root (FIT, ERR, ...) with its label and values. CURRENT numbers the items
    /// from 1, ALL takes the first number free for every root; NONE saves nothing.
    pub(crate) fn save_series(
        &mut self,
        command: &str,
        roots: &[&str],
        items: Vec<(Vec<String>, Vec<Values>)>,
    ) -> Result<(), Error> {
        let newvar = self.tset.newvar;
        if newvar == NewVar::None || items.is_empty() {
            return Ok(());
        }
        let count = roots.len() * items.len();
        let most = self.tset.mxnewvars as usize;
        if count > most {
            self.warn(format!(
                "{command} would make {count} new variables, more than TSET MXNEWVARS={most}; none are saved"
            ));
            return Ok(());
        }
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            for (i, (labels, columns)) in items.into_iter().enumerate() {
                let names = |n: usize| roots.iter().map(move |r| format!("{r}_{n}"));
                let n = match newvar {
                    NewVar::Current => i + 1,
                    _ => (1..)
                        .find(|&n| names(n).all(|v| data.variable_index(&v).is_none()))
                        .expect("a free number"),
                };
                for ((name, label), values) in names(n).zip(labels).zip(columns) {
                    data.set_numeric_values(&name, values)?;
                    data.set_variable_label(&name, Some(&label))?;
                    data.set_format(&name, "F11.5")?;
                    data.set_measure(&name, Measure::Scale)?;
                }
            }
            Ok(())
        })
    }

    /// Under TSET NEWVAR=CURRENT, drops the variables of these names before a procedure
    /// saves its own under them.
    pub(crate) fn replace_current(&mut self, names: &[String]) -> Result<(), Error> {
        let data = self.active_mut()?;
        let existing: Vec<String> = names
            .iter()
            .filter(|n| data.variable_index(n).is_some())
            .cloned()
            .collect();
        if existing.is_empty() {
            return Ok(());
        }
        if existing.len() == data.variable_count() {
            return Err(Error(
                "the new variables would replace every variable of the dataset".into(),
            ));
        }
        let names: Vec<&str> = existing.iter().map(String::as_str).collect();
        data.delete_variables(&names)?;
        Ok(())
    }

    /// SEASON, CURVEFIT and WLS: the names to replace before the run under NEWVAR=CURRENT.
    pub(crate) fn current_names(&self, command: &Command) -> Result<Vec<String>, Error> {
        if self.tset.newvar != NewVar::Current {
            return Ok(Vec::new());
        }
        let numbered = |roots: &[&str], count: usize| -> Vec<String> {
            (1..=count)
                .flat_map(|n| roots.iter().map(move |r| format!("{r}_{n}")))
                .collect()
        };
        Ok(match command {
            Command::Season(s) => numbered(
                &["ERR", "SAS", "SAF", "STC"],
                self.expand(&s.variables)?.len(),
            ),
            Command::Curvefit(c) if !(c.save_pred || c.save_resid || c.save_cin) => numbered(
                &["FIT", "ERR", "LCL", "UCL"],
                self.expand(&c.dependents)?.len() * c.models.len(),
            ),
            Command::Wls(w) if !w.save_weight && w.weight.is_none() => numbered(&["WGT"], 1),
            Command::Exsmooth(e) => numbered(&["FIT", "ERR"], self.expand(&e.variables)?.len()),
            Command::Arima(_) => numbered(&["FIT", "ERR", "LCL", "UCL", "SEP"], 1),
            _ => Vec::new(),
        })
    }

    pub(crate) fn season(&mut self, spec: &Season) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let names = self.expand(&spec.variables)?;
        let Some(p) = self.ts_period(spec.period) else {
            return Err(Error(
                "SEASON needs a period: give PERIOD, TSET PERIOD or DATE variables".into(),
            ));
        };
        if p < 2 {
            return Err(Error("SEASON needs a period of at least 2".into()));
        }
        let additive = spec.additive.unwrap_or(false);
        let centered = p.is_multiple_of(2) && spec.centered.unwrap_or(true);
        let include = self.tset.include;
        let brief = self.tset.print == TsetPrint::Brief;
        let mut tables = vec![table(
            "Model Description",
            1,
            plain(&["", "Specification"]),
            vec![
                vec![
                    text("Series"),
                    text(
                        names
                            .iter()
                            .map(|n| shown.named(data, n))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                ],
                vec![
                    text("Model"),
                    text(if additive {
                        "Additive"
                    } else {
                        "Multiplicative"
                    }),
                ],
                vec![text("Period"), Cell::Number(p as f64)],
                vec![
                    text("Moving average"),
                    text(if centered {
                        "Span equal to the period plus 1, end points weighted by 0.5"
                    } else {
                        "Span equal to the period, all points weighted equally"
                    }),
                ],
            ],
            None,
        )];
        // per series: the saved components
        let mut saved: Vec<(String, [Vec<f64>; 4])> = Vec::new();
        for name in &names {
            let (x, _, _) = read_series(data, name, include, "SEASON")?;
            let Some(x): Option<Vec<f64>> = x.iter().copied().collect() else {
                return Err(Error(format!(
                    "SEASON cannot use {name}: it has missing values; replace them first"
                )));
            };
            let n = x.len();
            if n < 4 * p {
                return Err(Error(format!(
                    "SEASON needs at least four full periods ({}) of {name}, which has {n} cases",
                    4 * p
                )));
            }
            if !additive && x.iter().any(|v| *v <= 0.0) {
                return Err(Error(format!(
                    "the multiplicative model needs values above 0, but {name} has others; use \
                     MODEL=ADDITIVE"
                )));
            }
            let (ma, si, factors) = seasonal_factors(&x, p, additive, centered);
            let saf: Vec<f64> = (0..n).map(|t| factors[t % p]).collect();
            let sas: Vec<f64> = (0..n)
                .map(|t| {
                    if additive {
                        x[t] - saf[t]
                    } else {
                        x[t] / saf[t] * 100.0
                    }
                })
                .collect();
            let mut stc = vec![0.0; n];
            for t in 2..n - 2 {
                stc[t] =
                    (sas[t - 2] + 2.0 * sas[t - 1] + 3.0 * sas[t] + 2.0 * sas[t + 1] + sas[t + 2])
                        / 9.0;
            }
            stc[1] = (sas[0] + sas[1] + sas[2]) / 3.0;
            stc[n - 2] = (sas[n - 3] + sas[n - 2] + sas[n - 1]) / 3.0;
            stc[0] = stc[1] + (stc[1] - stc[2]) / 2.0;
            stc[n - 1] = stc[n - 2] + (stc[n - 2] - stc[n - 3]) / 2.0;
            let err: Vec<f64> = (0..n)
                .map(|t| {
                    if additive {
                        sas[t] - stc[t]
                    } else {
                        sas[t] / stc[t]
                    }
                })
                .collect();
            let label = shown.named(data, name);
            if !brief {
                let rows = (0..n)
                    .map(|t| {
                        vec![
                            Cell::Number(t as f64 + 1.0),
                            Cell::Number(x[t]),
                            number(ma[t]),
                            number(si[t]),
                            Cell::Number(saf[t]),
                            Cell::Number(sas[t]),
                            Cell::Number(stc[t]),
                            Cell::Number(err[t]),
                        ]
                    })
                    .collect();
                let ratio = if additive {
                    "Difference from Moving Average"
                } else {
                    "Ratio to Moving Average (%)"
                };
                let factor = if additive {
                    "Seasonal Factor"
                } else {
                    "Seasonal Factor (%)"
                };
                tables.push(table(
                    "Seasonal Decomposition",
                    1,
                    plain(&[
                        "Case",
                        "Original Series",
                        "Moving Average",
                        ratio,
                        factor,
                        "Seasonally Adjusted Series",
                        "Smoothed Trend-Cycle",
                        "Irregular Component",
                    ]),
                    rows,
                    Some(format!("Series: {label}.")),
                ));
            }
            tables.push(table(
                "Seasonal Factors",
                1,
                plain(&[
                    "Period",
                    if additive {
                        "Seasonal Factor"
                    } else {
                        "Seasonal Factor (%)"
                    },
                ]),
                (0..p)
                    .map(|s| vec![Cell::Number(s as f64 + 1.0), Cell::Number(factors[s])])
                    .collect(),
                Some(format!("Series: {label}.")),
            ));
            let saf_saved: Vec<f64> = if additive {
                saf.clone()
            } else {
                saf.iter().map(|f| f / 100.0).collect()
            };
            saved.push((name.clone(), [err, sas, saf_saved, stc]));
        }
        // new variables, as TSET NEWVAR says
        let newvar = self.tset.newvar;
        if newvar != NewVar::None && !saved.is_empty() {
            if 4 * saved.len() > self.tset.mxnewvars as usize {
                self.warn(format!(
                    "SEASON would make {} new variables, more than TSET MXNEWVARS={}; none are saved",
                    4 * saved.len(),
                    self.tset.mxnewvars
                ));
                return Ok(tables);
            }
            let model = if additive {
                "additive"
            } else {
                "multiplicative"
            };
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                let roots = ["ERR", "SAS", "SAF", "STC"];
                for (i, (name, components)) in saved.iter().enumerate() {
                    let names = |n: usize| roots.map(|r| format!("{r}_{n}"));
                    let n = match newvar {
                        NewVar::Current => i + 1,
                        _ => (1..)
                            .find(|&n| names(n).iter().all(|v| data.variable_index(v).is_none()))
                            .expect("a free number"),
                    };
                    let labels = [
                        format!("Irregular component of {name}, {model}, period {p}"),
                        format!("Seasonally adjusted {name}, {model}, period {p}"),
                        format!("Seasonal factor of {name}, {model}, period {p}"),
                        format!("Trend-cycle of {name}, {model}, period {p}"),
                    ];
                    for (j, v) in names(n).iter().enumerate() {
                        data.set_numeric_values(
                            v,
                            components[j].iter().map(|x| Some(*x)).collect(),
                        )?;
                        data.set_variable_label(v, Some(&labels[j]))?;
                        data.set_format(v, "F11.5")?;
                        data.set_measure(v, Measure::Scale)?;
                    }
                }
                Ok(())
            })?;
        }
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn some(v: &[f64]) -> Values {
        v.iter().map(|x| Some(*x)).collect()
    }

    #[test]
    fn centered_moving_averages_shrink_at_the_ends() {
        let x = some(&[2.0, 4.0, 9.0, 3.0, 7.0, 5.0]);
        // span 4, minimum 2: span 2 next to the ends, nothing at the ends
        let y = centered(&x, 4, 2, mean);
        assert_eq!(y[0], None);
        assert_eq!(y[1], Some((2.0 + 2.0 * 4.0 + 9.0) / 4.0));
        assert_eq!(y[2], Some((2.0 / 2.0 + 4.0 + 9.0 + 3.0 + 7.0 / 2.0) / 4.0));
        // an odd minimum with an even span counts as the next even one: 4
        let four = centered(&x, 4, 3, mean);
        assert_eq!((four[1], four[2]), (None, y[2]));
        // span 3, minimum 1: the ends keep their value
        let z = centered(&x, 3, 1, median);
        assert_eq!(z[0], Some(2.0));
        assert_eq!(z[1], Some(4.0));
        // a gap starts a new series
        let g = centered(
            &[Some(1.0), Some(2.0), None, Some(5.0), Some(6.0), Some(9.0)],
            3,
            3,
            mean,
        );
        assert_eq!(g, vec![None, None, None, None, Some(20.0 / 3.0), None]);
    }

    #[test]
    fn prior_means_and_shifts() {
        let x = some(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(prior_mean(&x, 2), vec![None, None, Some(1.5), Some(2.5)]);
        assert_eq!(shift(&x, 1), vec![None, Some(1.0), Some(2.0), Some(3.0)]);
        assert_eq!(shift(&x, -2), vec![Some(3.0), Some(4.0), None, None]);
        assert_eq!(
            csum(&[Some(1.0), None, Some(2.0)]),
            vec![Some(1.0), None, Some(3.0)]
        );
    }

    #[test]
    fn fourier_transform_round_trip() {
        let x = [3.0, -1.0, 4.0, 1.0, -5.0, 9.0];
        let (re, im) = fft(&x);
        assert_eq!(re.len(), 3);
        assert!((re[0] - 11.0 / 6.0).abs() < 1e-12);
        // the highest frequency: (3 + 1 + 4 - 1 - 5 - 9) / 6, negated
        assert!((im[0] - 7.0 / 6.0).abs() < 1e-12);
        let back = ifft(&re, &im);
        for (a, b) in back.iter().zip(&x) {
            assert!((a - b).abs() < 1e-9);
        }
    }

    #[test]
    fn partial_autocorrelations_of_an_ar1_pattern() {
        // r_k = 0.5^k gives 0.5 then zeros
        let p = partial_autocorrelations(&[0.5, 0.25, 0.125]);
        assert!((p[0] - 0.5).abs() < 1e-12 && p[1].abs() < 1e-12 && p[2].abs() < 1e-12);
    }
}
