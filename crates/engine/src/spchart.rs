//! SPCHART: Shewhart control charts (X-bar with R or s, individuals with moving ranges, p,
//! np, c and u) and the process capability chart, with the capability and performance
//! statistics and the rules for points out of control.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our own words:
//! - A is the SIGMAS value. Each control chart is a line chart of the process with its center
//!   line and control limits, center plus and minus A sigmas of the plotted statistic. Lower
//!   limits of ranges, deviations and counts stop at 0, a p chart's upper limit at 1 and an np
//!   chart's at the subgroup size.
//! - X-bar charts: the center is the mean of all values; sigma is the mean over subgroups of
//!   R/d2(n) (XR) or S/c4(n) (XS), divided by the square root of the subgroup size. The R
//!   chart's center is sigma d2(n) with sigma d3(n); the s chart's sigma c4(n) with sigma
//!   sqrt(1 - c4(n)^2). d2 and d3 are the mean and standard deviation of the range of n
//!   standard normal values, c4 = sqrt(2 / (n - 1)) G(n / 2) / G((n - 1) / 2).
//! - Individuals: moving ranges over SPAN values (missing for the first SPAN - 1 points); the
//!   individuals chart has the mean of the values as center and the mean moving range over
//!   d2(SPAN) as sigma; the moving range chart has the mean moving range as center.
//! - p: p-bar = all nonconforming / all units, sigma sqrt(p-bar (1 - p-bar) / n); np: center
//!   n p-bar, sigma sqrt(n p-bar (1 - p-bar)); u: u-bar = all defects / all units, sigma
//!   sqrt(u-bar / n); c: center n u-bar, sigma sqrt(n u-bar). With unequal subgroups the limits
//!   follow each subgroup's size; np and c charts then get a warning.
//! - Subgroups smaller than MINSAMPLE are left out of X-bar, R, s and capability charts. A
//!   count above its subgroup size drops the subgroup of a p or np chart.
//! - Weights count only when cases are units, and only whole positive ones; other cases are
//!   dropped. Individuals are repeated as often as their weight says.
//! - Capability sigma: RBAR mean of R/d2(n), SBAR mean of S/c4(n), WITHIN the root of the
//!   pooled within-subgroup variance, MRBAR mean moving range over d2(SPAN). Performance uses
//!   the standard deviation of all values. The indices follow the manual's formulas.
//!
//! Where the manual is silent:
//! - Rules are checked on the first chart only. The UCL and LCL rules flag points beyond the
//!   drawn control limits; the zone rules use the chart's sigma whatever SIGMAS says. A
//!   pattern needs its full length of points, and its last point must itself qualify.
//! - The chart's statistics go to a table: CPCHART without STATISTICS lists the indices it
//!   would show on the chart.

use crate::chart::{Axis, Category, Chart, NormalCurve, Series, Statistic};
use crate::crosstabs::Key;
use crate::descriptives::case_weights;
use crate::frequencies::Histogram;
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table, dist};
use statscraft_data::{Dataset, Values, Variable};
use statscraft_syntax::{
    CapSigma, SPCHART_STATISTICS, Spchart, SpchartData, SpchartKind as K, SpchartNormal,
    SpchartSize, Value,
};
use std::collections::HashMap;

/// The most points a control chart shows.
const MOST_POINTS: usize = 3000;

/// What a plotted point stands for.
#[derive(Clone)]
struct Point {
    label: String,
    value: Cell,
    missing: bool,
    /// How the rules table names it.
    id: String,
}

/// The data of the chart, a point each.
enum Collected {
    /// Variable charts with subgroups: the (value, weight) pairs of each subgroup.
    Groups(Vec<(Point, Vec<(f64, f64)>)>),
    Individuals(Vec<(Point, f64)>),
    /// Attribute charts: the count and the size of each subgroup.
    Counts(Vec<(Point, f64, f64)>),
}

/// One control chart: the process, its center and sigma at each point, and the bounds of its
/// limits.
struct Panel {
    statistic: String,
    values: Vec<f64>,
    center: Vec<f64>,
    sigma: Vec<f64>,
    floor: f64,
    ceiling: Vec<f64>,
}

impl Panel {
    /// The upper and lower control limits at A sigmas.
    fn limits(&self, a: f64) -> (Vec<f64>, Vec<f64>) {
        let upper = (0..self.values.len())
            .map(|i| (self.center[i] + a * self.sigma[i]).min(self.ceiling[i]))
            .collect();
        let lower = (0..self.values.len())
            .map(|i| (self.center[i] - a * self.sigma[i]).max(self.floor))
            .collect();
        (upper, lower)
    }
}

/// d2(n) and d3(n): the mean and standard deviation of the range of n standard normal values,
/// by Simpson's rule over the distribution of the range,
/// P(range <= r) = n times the integral of phi(y) (Phi(y + r) - Phi(y))^(n - 1) dy.
pub(crate) fn range_constants(n: usize) -> (f64, f64) {
    const H: f64 = 0.01;
    const LOW: f64 = -9.0;
    const YS: usize = 1800;
    const RS: usize = 1400;
    let phi: Vec<f64> = (0..=YS + RS)
        .map(|k| dist::normal_cdf(LOW + k as f64 * H))
        .collect();
    let density: Vec<f64> = (0..=YS)
        .map(|k| {
            let y = LOW + k as f64 * H;
            (-y * y / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt()
        })
        .collect();
    let simpson = |f: &dyn Fn(usize) -> f64, steps: usize| -> f64 {
        let weight = |k: usize| match k {
            0 => 1.0,
            k if k == steps => 1.0,
            k if k % 2 == 1 => 4.0,
            _ => 2.0,
        };
        (0..=steps).map(|k| weight(k) * f(k)).sum::<f64>() * H / 3.0
    };
    let power = n as i32 - 1;
    let tail: Vec<f64> = (0..=RS)
        .map(|j| {
            let below = simpson(&|k| density[k] * (phi[k + j] - phi[k]).powi(power), YS);
            1.0 - n as f64 * below
        })
        .collect();
    let mean = simpson(&|j| tail[j], RS);
    let second = simpson(&|j| 2.0 * j as f64 * H * tail[j], RS);
    (mean, (second - mean * mean).max(0.0).sqrt())
}

/// c4(n): the mean of the standard deviation of n standard normal values.
pub(crate) fn c4(n: f64) -> f64 {
    (2.0 / (n - 1.0)).sqrt() * (dist::ln_gamma(n / 2.0) - dist::ln_gamma((n - 1.0) / 2.0)).exp()
}

/// The constants of every subgroup size, computed once a run.
#[derive(Default)]
struct Constants(HashMap<usize, (f64, f64)>);

impl Constants {
    fn range(&mut self, n: f64) -> (f64, f64) {
        *self
            .0
            .entry(n as usize)
            .or_insert_with(|| range_constants(n as usize))
    }
}

/// For each point, the rules it breaks as the last point of a pattern.
fn violations(
    x: &[f64],
    center: &[f64],
    sigma: &[f64],
    limits: (&[f64], &[f64]),
    rules: &[&'static str],
) -> Vec<Vec<&'static str>> {
    let z: Vec<f64> = (0..x.len())
        .map(|i| (x[i] - center[i]) / sigma[i])
        .collect();
    let rises = |j: usize| x[j] > x[j - 1];
    let falls = |j: usize| x[j] < x[j - 1];
    (0..x.len())
        .map(|i| {
            // the last `len` points, when there are that many
            let last = |len: usize| (i + 1 >= len).then(|| &z[i + 1 - len..=i]);
            let count = |len: usize, keep: &dyn Fn(f64) -> bool| {
                last(len).map_or(0, |w| w.iter().filter(|v| keep(**v)).count())
            };
            let all = |len: usize, keep: &dyn Fn(f64) -> bool| {
                last(len).is_some_and(|w| w.iter().all(|v| keep(*v)))
            };
            rules
                .iter()
                .copied()
                .filter(|rule| match *rule {
                    "UCL" => x[i] > limits.0[i],
                    "LCL" => x[i] < limits.1[i],
                    "R2UPPER" => z[i] > 2.0 && count(3, &|v| v > 2.0) >= 2,
                    "R2LOWER" => z[i] < -2.0 && count(3, &|v| v < -2.0) >= 2,
                    "R4UPPER" => z[i] > 1.0 && count(5, &|v| v > 1.0) >= 4,
                    "R4LOWER" => z[i] < -1.0 && count(5, &|v| v < -1.0) >= 4,
                    "R8UPPER" => all(8, &|v| v > 0.0),
                    "R8LOWER" => all(8, &|v| v < 0.0),
                    "TRUP" => i >= 5 && (i - 4..=i).all(&rises),
                    "TRDOWN" => i >= 5 && (i - 4..=i).all(&falls),
                    // 14 points: each of the 13 steps goes the other way from the one before
                    _ => {
                        i >= 13
                            && (i - 11..=i).all(|j| (x[j] - x[j - 1]) * (x[j - 1] - x[j - 2]) < 0.0)
                    }
                })
                .collect()
        })
        .collect()
}

fn rule_text(rule: &str) -> &'static str {
    match rule {
        "UCL" => "Above the upper control limit",
        "R2UPPER" => "2 of the last 3 above +2 sigma",
        "R4UPPER" => "4 of the last 5 above +1 sigma",
        "R8UPPER" => "8 in a row above the center line",
        "R8LOWER" => "8 in a row below the center line",
        "R4LOWER" => "4 of the last 5 below -1 sigma",
        "R2LOWER" => "2 of the last 3 below -2 sigma",
        "LCL" => "Below the lower control limit",
        "TRUP" => "6 in a row going up",
        "TRDOWN" => "6 in a row going down",
        _ => "14 in a row going up and down in turn",
    }
}

fn short(v: f64) -> String {
    let text = format!("{v:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A series' name, with its value when it is the same at every point.
fn legend(name: &str, values: &[f64]) -> String {
    match values.first() {
        Some(&v) if v.is_finite() && values.iter().all(|w| (w - v).abs() <= 1e-12 * v.abs()) => {
            format!("{name} = {}", short(v))
        }
        _ => name.to_string(),
    }
}

fn mean_sd(pairs: &[(f64, f64)]) -> (f64, f64, f64) {
    let n: f64 = pairs.iter().map(|p| p.1).sum();
    let mean = pairs.iter().map(|(x, w)| x * w).sum::<f64>() / n;
    let ss: f64 = pairs.iter().map(|(x, w)| w * (x - mean).powi(2)).sum();
    (n, mean, (ss / (n - 1.0)).sqrt())
}

/// The moving ranges over `span` values; NaN for the first span - 1.
fn moving_ranges(x: &[f64], span: usize) -> Vec<f64> {
    (0..x.len())
        .map(|i| match i + 1 >= span {
            true => {
                let w = &x[i + 1 - span..=i];
                let max = w.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                max - w.iter().copied().fold(f64::INFINITY, f64::min)
            }
            false => f64::NAN,
        })
        .collect()
}

/// The parts of a run that do not change while it is read.
struct Reader<'a> {
    data: &'a Dataset,
    shown: Shown,
    c: &'a Spchart,
    /// Whole positive weights of units; None without a weight variable.
    weights: Option<Vec<f64>>,
    /// The case number of each case, counted from 1.
    numbers: Vec<usize>,
}

impl Reader<'_> {
    fn numeric(&self, name: &str) -> Result<Variable<'_>, Error> {
        let v = self.data.variable(name)?;
        if v.is_string() {
            return Err(Error(format!("SPCHART: {name} must be numeric")));
        }
        Ok(v)
    }

    fn valid(&self, v: &Variable, case: usize) -> Option<f64> {
        let x = v.numeric().ok()?.get(case)?;
        (self.c.include || !v.is_user_missing(x)).then_some(x)
    }

    /// The weight of a unit: 0 for one to drop.
    fn weight(&self, case: usize) -> f64 {
        match &self.weights {
            Some(w) if w[case] > 0.0 && w[case].fract() == 0.0 => w[case],
            Some(_) => 0.0,
            None => 1.0,
        }
    }

    /// The point of a case: its BY value, else its case number; None for a missing BY value
    /// under NOREPORT.
    fn point(&self, by: Option<&Variable>, case: usize) -> Option<(Point, (bool, Option<Key>))> {
        let number = self.numbers[case];
        let Some(v) = by else {
            let point = Point {
                label: number.to_string(),
                value: Cell::Number(number as f64),
                missing: false,
                id: self.id(case).unwrap_or_else(|| number.to_string()),
            };
            return Some((point, (false, Some(Key::Number(number as f64)))));
        };
        let (key, missing) = match v.values() {
            Values::Numeric(values) => match values.get(case) {
                Some(x) => (Some(Key::Number(x)), v.is_user_missing(x)),
                None => (None, true),
            },
            Values::String(values) => {
                let text = values.get(case);
                (Some(Key::Text(text.into())), v.is_user_missing_str(text))
            }
        };
        if missing && !self.c.report {
            return None;
        }
        let (label, value) = match &key {
            Some(k) => (
                self.shown.value(v, k),
                match k {
                    Key::Number(n) => Cell::Number(*n),
                    Key::Text(t) => Cell::Text(t.clone()),
                },
            ),
            None => ("System missing".to_string(), Cell::Empty),
        };
        let id = self.id(case).unwrap_or_else(|| label.clone());
        let point = Point {
            label,
            value,
            missing,
            id,
        };
        Some((point, (missing, key)))
    }

    /// The ID variable's value at a case.
    fn id(&self, case: usize) -> Option<String> {
        let v = self.data.variable(self.c.id.as_deref()?).ok()?;
        let key = match v.values() {
            Values::Numeric(values) => Key::Number(values.get(case)?),
            Values::String(values) => Key::Text(values.get(case).into()),
        };
        Some(self.shown.value(&v, &key))
    }

    fn by(&self, name: &Option<String>) -> Result<Option<Variable<'_>>, Error> {
        name.as_deref()
            .map(|n| self.data.variable(n).map_err(Error::from))
            .transpose()
    }

    /// Units grouped by their BY value, valid values first, each kind ascending.
    fn grouped<T>(
        &self,
        by: &Variable,
        mut item: impl FnMut(usize) -> Option<T>,
    ) -> Vec<(Point, Vec<T>)> {
        let mut cases: Vec<(Point, (bool, Option<Key>), T)> = (0..self.data.case_count())
            .filter_map(|case| {
                let (point, class) = self.point(Some(by), case)?;
                Some((point, class, item(case)?))
            })
            .collect();
        let order = |a: &(bool, Option<Key>), b: &(bool, Option<Key>)| {
            a.0.cmp(&b.0).then_with(|| match (&a.1, &b.1) {
                (Some(x), Some(y)) => x.compare(y),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
        };
        cases.sort_by(|a, b| order(&a.1, &b.1));
        let mut out: Vec<(Point, Vec<T>)> = Vec::new();
        let mut previous: Option<(bool, Option<Key>)> = None;
        for (point, class, t) in cases {
            if previous.as_ref().is_some_and(|p| order(p, &class).is_eq()) {
                out.last_mut().expect("a group is open").1.push(t);
            } else {
                out.push((point, vec![t]));
                previous = Some(class);
            }
        }
        out
    }

    fn collect(&self) -> Result<Collected, Error> {
        let c = self.c;
        let cases = 0..self.data.case_count();
        Ok(match (&c.data, c.kind) {
            (SpchartData::Units { process, by }, K::Xr | K::Xs | K::Capability) => {
                let x = self.numeric(process)?;
                let by = self.data.variable(by)?;
                let groups = self.grouped(&by, |case| {
                    let w = self.weight(case);
                    (w > 0.0).then_some((self.valid(&x, case)?, w))
                });
                Collected::Groups(groups)
            }
            (SpchartData::Subgroups { variables, by }, _) => {
                let names = crate::expand_in(self.data, variables)?;
                if names.len() < 2 {
                    return Err(Error(
                        "SPCHART: subgroups need two variables or more".into(),
                    ));
                }
                if names.len() < c.minsample as usize {
                    return Err(Error(format!(
                        "SPCHART: MINSAMPLE={} needs that many variables",
                        c.minsample
                    )));
                }
                let vars = names
                    .iter()
                    .map(|n| self.numeric(n))
                    .collect::<Result<Vec<_>, _>>()?;
                let by = self.by(by)?;
                let groups = cases
                    .filter_map(|case| {
                        let (point, _) = self.point(by.as_ref(), case)?;
                        let values = vars
                            .iter()
                            .filter_map(|v| Some((self.valid(v, case)?, 1.0)))
                            .collect();
                        Some((point, values))
                    })
                    .collect();
                Collected::Groups(groups)
            }
            (SpchartData::Individuals { process, by }, _) => {
                let x = self.numeric(process)?;
                let by = self.by(by)?;
                let mut out = Vec::new();
                for case in cases {
                    let Some((point, _)) = self.point(by.as_ref(), case) else {
                        continue;
                    };
                    let Some(value) = self.valid(&x, case) else {
                        continue;
                    };
                    for _ in 0..self.weight(case) as usize {
                        out.push((point.clone(), value));
                    }
                }
                Collected::Individuals(out)
            }
            (SpchartData::Units { process, by }, kind) => {
                let status = self.data.variable(process)?;
                let by = self.data.variable(by)?;
                let attribute = matches!(kind, K::P | K::Np);
                if !attribute && status.is_string() {
                    return Err(Error(format!("SPCHART: {process} must be numeric")));
                }
                let groups = self.grouped(&by, |case| {
                    let w = self.weight(case);
                    if w <= 0.0 {
                        return None;
                    }
                    let defects = match attribute {
                        true => self.nonconforming(&status, case)?,
                        false => self.valid(&status, case)?,
                    };
                    Some((defects * w, w))
                });
                let counts = groups
                    .into_iter()
                    .map(|(point, items)| {
                        let count = items.iter().map(|i| i.0).sum();
                        (point, count, items.iter().map(|i| i.1).sum())
                    })
                    .collect();
                Collected::Counts(counts)
            }
            (SpchartData::Counts { count, size, by }, kind) => {
                let count = self.numeric(count)?;
                let size_var = match size {
                    SpchartSize::Variable(name) => Some(self.numeric(name)?),
                    SpchartSize::Constant(_) => None,
                };
                let by = self.by(by)?;
                let mut out = Vec::new();
                for case in cases {
                    let Some((point, _)) = self.point(by.as_ref(), case) else {
                        continue;
                    };
                    let n = match (&size_var, size) {
                        (Some(v), _) => self.valid(v, case),
                        (None, SpchartSize::Constant(n)) => Some(*n),
                        _ => None,
                    };
                    let (Some(x), Some(n)) = (self.valid(&count, case), n) else {
                        continue;
                    };
                    // a count above the subgroup size drops the subgroup of a p or np chart
                    if n <= 0.0 || x < 0.0 || (matches!(kind, K::P | K::Np) && x > n) {
                        continue;
                    }
                    out.push((point, x, n));
                }
                Collected::Counts(out)
            }
        })
    }

    /// 1 when the status at a case counts as nonconforming, 0 when not; None when missing.
    fn nonconforming(&self, v: &Variable, case: usize) -> Option<f64> {
        let (flag, given) = self.c.conform.as_ref()?;
        let equal = match (v.values(), given) {
            (Values::Numeric(_), Value::Number(n)) => self.valid(v, case)? == *n,
            (Values::String(values), Value::String(s)) => {
                let text = values.get(case);
                if !self.c.include && v.is_user_missing_str(text) {
                    return None;
                }
                text.trim_end() == s.trim_end()
            }
            _ => false,
        };
        Some(f64::from(u8::from(equal == *flag)))
    }
}

impl Engine {
    pub(crate) fn spchart(&mut self, c: &Spchart) -> Result<Vec<Table>, Error> {
        let mut warnings = Vec::new();
        // datasets share their columns, so the clone is cheap and leaves self free for charts
        let dataset = self.active()?.clone();
        let data = &dataset;
        let units = matches!(
            (&c.data, c.kind),
            (SpchartData::Units { .. }, _) | (SpchartData::Individuals { .. }, _)
        );
        let weights = match units {
            true => case_weights(data)?,
            false => {
                if data.weight_variable().is_some() {
                    warnings.push(
                        "SPCHART does not use case weights when each case is a subgroup".into(),
                    );
                }
                None
            }
        };
        let numbers = match &self.case_numbers {
            Some(numbers) => numbers.iter().map(|n| n + 1).collect(),
            None => (1..=data.case_count()).collect(),
        };
        let reader = Reader {
            data,
            shown: self.shown(),
            c,
            weights,
            numbers,
        };
        let outline = self.outline();
        let process_label = match &c.data {
            SpchartData::Units { process, .. } | SpchartData::Individuals { process, .. } => {
                outline.named(data, process)
            }
            SpchartData::Subgroups { variables, .. } => crate::expand_in(data, variables)?
                .iter()
                .map(|n| outline.named(data, n))
                .collect::<Vec<_>>()
                .join(", "),
            SpchartData::Counts { count, .. } => outline.named(data, count),
        };
        let by_title = match &c.data {
            SpchartData::Units { by, .. } => Some(by.as_str()),
            SpchartData::Subgroups { by, .. }
            | SpchartData::Individuals { by, .. }
            | SpchartData::Counts { by, .. } => by.as_deref(),
        }
        .map_or(String::new(), |by| reader.shown.named(data, by));
        let collected = reader.collect()?;
        let a = c.sigmas;
        let mut constants = Constants::default();
        let mut panels: Vec<Panel> = Vec::new();
        let points: Vec<Point>;
        // the values of the process for the statistics, with their weights
        let mut pairs: Vec<(f64, f64)> = Vec::new();
        let mut groups: Vec<(f64, f64, f64, f64)> = Vec::new();
        let mut individuals: Vec<f64> = Vec::new();
        let span = c.span.unwrap_or(2) as usize;
        match collected {
            Collected::Groups(all) => {
                let kept: Vec<(Point, Vec<(f64, f64)>)> = all
                    .into_iter()
                    .filter(|(_, v)| v.iter().map(|p| p.1).sum::<f64>() >= f64::from(c.minsample))
                    .collect();
                for (_, v) in &kept {
                    let (n, mean, sd) = mean_sd(v);
                    let max = v.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
                    let min = v.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
                    if n > 100.0 && (c.kind == K::Xr || c.capsigma == Some(CapSigma::Rbar)) {
                        return Err(Error(
                            "SPCHART: a subgroup has more than 100 units, the most for ranges"
                                .into(),
                        ));
                    }
                    groups.push((n, mean, max - min, sd));
                    pairs.extend(v);
                }
                points = kept.into_iter().map(|(p, _)| p).collect();
                if matches!(c.kind, K::Xr | K::Xs) {
                    let grand = mean_sd(&pairs).1;
                    let ranges = c.kind == K::Xr;
                    let sigma = groups
                        .iter()
                        .map(|g| match ranges {
                            true => g.2 / constants.range(g.0).0,
                            false => g.3 / c4(g.0),
                        })
                        .sum::<f64>()
                        / groups.len() as f64;
                    panels.push(Panel {
                        statistic: "Mean".into(),
                        values: groups.iter().map(|g| g.1).collect(),
                        center: vec![grand; groups.len()],
                        sigma: groups.iter().map(|g| sigma / g.0.sqrt()).collect(),
                        floor: f64::NEG_INFINITY,
                        ceiling: vec![f64::INFINITY; groups.len()],
                    });
                    if !c.xbar_only {
                        let (center, spread): (Vec<f64>, Vec<f64>) = groups
                            .iter()
                            .map(|g| match ranges {
                                true => {
                                    let (d2, d3) = constants.range(g.0);
                                    (sigma * d2, sigma * d3)
                                }
                                false => {
                                    let c4 = c4(g.0);
                                    (sigma * c4, sigma * (1.0 - c4 * c4).sqrt())
                                }
                            })
                            .unzip();
                        panels.push(Panel {
                            statistic: if ranges {
                                "Range"
                            } else {
                                "Standard deviation"
                            }
                            .into(),
                            values: groups
                                .iter()
                                .map(|g| if ranges { g.2 } else { g.3 })
                                .collect(),
                            center,
                            sigma: spread,
                            floor: 0.0,
                            ceiling: vec![f64::INFINITY; groups.len()],
                        });
                    }
                }
            }
            Collected::Individuals(all) => {
                if all.len() < 2 {
                    return Err(too_few(all.len()));
                }
                individuals = all.iter().map(|p| p.1).collect();
                pairs = individuals.iter().map(|x| (*x, 1.0)).collect();
                points = all.into_iter().map(|p| p.0).collect();
                if span > individuals.len() {
                    return Err(Error(format!(
                        "SPCHART: the span {span} is larger than the {} values",
                        individuals.len()
                    )));
                }
                if c.kind != K::Capability {
                    let n = individuals.len();
                    let ranges = moving_ranges(&individuals, span);
                    let mr = ranges[span - 1..].iter().sum::<f64>() / (n - span + 1) as f64;
                    let (d2, d3) = constants.range(span as f64);
                    let mean = individuals.iter().sum::<f64>() / n as f64;
                    panels.push(Panel {
                        statistic: process_label.clone(),
                        values: individuals.clone(),
                        center: vec![mean; n],
                        sigma: vec![mr / d2; n],
                        floor: f64::NEG_INFINITY,
                        ceiling: vec![f64::INFINITY; n],
                    });
                    if c.kind == K::Ir {
                        panels.push(Panel {
                            statistic: "Moving range".into(),
                            values: ranges,
                            center: vec![mr; n],
                            sigma: vec![mr * d3 / d2; n],
                            floor: 0.0,
                            ceiling: vec![f64::INFINITY; n],
                        });
                    }
                }
            }
            Collected::Counts(all) => {
                let total_x: f64 = all.iter().map(|g| g.1).sum();
                let total_n: f64 = all.iter().map(|g| g.2).sum();
                let bar = total_x / total_n;
                let unequal = all.iter().any(|g| g.2 != all[0].2);
                let (statistic, values, center, sigma, ceiling): (&str, _, Vec<f64>, Vec<f64>, _) =
                    match c.kind {
                        K::P => (
                            "Proportion nonconforming",
                            all.iter().map(|g| g.1 / g.2).collect(),
                            vec![bar; all.len()],
                            all.iter()
                                .map(|g| (bar * (1.0 - bar) / g.2).sqrt())
                                .collect(),
                            vec![1.0; all.len()],
                        ),
                        K::Np => (
                            "Number nonconforming",
                            all.iter().map(|g| g.1).collect(),
                            all.iter().map(|g| g.2 * bar).collect(),
                            all.iter()
                                .map(|g| (g.2 * bar * (1.0 - bar)).sqrt())
                                .collect(),
                            all.iter().map(|g| g.2).collect(),
                        ),
                        K::U => (
                            "Defects per unit",
                            all.iter().map(|g| g.1 / g.2).collect(),
                            vec![bar; all.len()],
                            all.iter().map(|g| (bar / g.2).sqrt()).collect(),
                            vec![f64::INFINITY; all.len()],
                        ),
                        _ => (
                            "Number of defects",
                            all.iter().map(|g| g.1).collect(),
                            all.iter().map(|g| g.2 * bar).collect(),
                            all.iter().map(|g| (g.2 * bar).sqrt()).collect(),
                            vec![f64::INFINITY; all.len()],
                        ),
                    };
                if unequal && matches!(c.kind, K::Np | K::C) {
                    warnings.push(format!(
                        "SPCHART: the subgroups differ in size; a {} chart suits them better",
                        if c.kind == K::Np { "p" } else { "u" }
                    ));
                }
                panels.push(Panel {
                    statistic: statistic.into(),
                    values,
                    center,
                    sigma,
                    floor: 0.0,
                    ceiling,
                });
                points = all.into_iter().map(|g| g.0).collect();
            }
        }
        if points.len() < 2 || points.len() > MOST_POINTS {
            return Err(too_few(points.len()));
        }

        // capability sigma, for the statistics
        let default_sigma = match (c.kind, &c.data) {
            (K::Xs, _) => CapSigma::Sbar,
            (K::I | K::Ir, _) | (K::Capability, SpchartData::Individuals { .. }) => CapSigma::Mrbar,
            _ => CapSigma::Rbar,
        };
        // MRBAR needs individual values, the others subgroups; the capability chart takes either
        let individual_data = matches!(c.data, SpchartData::Individuals { .. });
        let fits = |s: CapSigma| match c.kind {
            K::Xr | K::Xs => s != CapSigma::Mrbar,
            K::I | K::Ir => s == CapSigma::Mrbar,
            K::Capability => (s == CapSigma::Mrbar) == individual_data,
            _ => true,
        };
        let method = match c.capsigma {
            Some(s) if fits(s) => s,
            Some(_) => {
                if !matches!(c.kind, K::Np | K::P | K::C | K::U) {
                    warnings.push(
                        "SPCHART: this CAPSIGMA does not fit the chart and is not used".into(),
                    );
                }
                default_sigma
            }
            None => default_sigma,
        };
        let attribute = matches!(c.kind, K::Np | K::P | K::C | K::U);
        let capsigma = match method {
            _ if attribute => f64::NAN,
            CapSigma::Rbar => {
                groups
                    .iter()
                    .map(|g| g.2 / constants.range(g.0).0)
                    .sum::<f64>()
                    / groups.len() as f64
            }
            CapSigma::Sbar => {
                groups.iter().map(|g| g.3 / c4(g.0)).sum::<f64>() / groups.len() as f64
            }
            CapSigma::Within => {
                let ss: f64 = groups.iter().map(|g| g.3 * g.3 * (g.0 - 1.0)).sum();
                (ss / groups.iter().map(|g| g.0 - 1.0).sum::<f64>()).sqrt()
            }
            CapSigma::Mrbar => {
                let ranges = moving_ranges(&individuals, span);
                let n = individuals.len();
                let mr = ranges[span - 1..].iter().sum::<f64>() / (n - span + 1) as f64;
                mr / constants.range(span as f64).0
            }
        };
        let (total, mean, sd) = mean_sd(&pairs);

        let mut tables = Vec::new();
        let title = |made: String| match c.title.is_empty() {
            true => made,
            false => c.title.join(" "),
        };
        if c.kind == K::Capability {
            let mut sorted = pairs.clone();
            sorted.sort_by(|x, y| x.0.total_cmp(&y.0));
            let heading = process_label.clone();
            let made = title("Process Capability".into());
            let format = match &c.data {
                SpchartData::Units { process, .. } | SpchartData::Individuals { process, .. } => {
                    data.variable(process)?.format().to_string()
                }
                _ => "F8.2".into(),
            };
            let histogram = Histogram {
                title: &made,
                heading: &heading,
                format: &format,
                name: &process_label,
                normal: false,
                scale: None,
            };
            let mut chart = histogram
                .chart(&sorted, &sorted, total)
                .map_err(|e| Error(format!("SPCHART: {e}")))?;
            chart.category_axis.variable = None;
            let curve = |std_dev: f64| {
                (std_dev.is_finite() && std_dev > 0.0).then_some(NormalCurve {
                    mean,
                    std_dev,
                    total,
                })
            };
            chart.normal = match c.normal {
                SpchartNormal::Capability | SpchartNormal::Both => curve(capsigma),
                SpchartNormal::Performance => curve(sd),
                SpchartNormal::None => None,
            };
            let mut notes: Vec<String> = c.footnote.clone();
            let (lsl, usl) = (c.lsl.expect("checked"), c.usl.expect("checked"));
            if c.reference_limits {
                let target = c
                    .target
                    .map_or(String::new(), |t| format!(", target {}", short(t)));
                notes.push(format!(
                    "Specification limits: LSL {}, USL {}{target}.",
                    short(lsl),
                    short(usl)
                ));
            }
            if c.reference_sigmas {
                notes.push(format!(
                    "Mean {} and {} capability sigmas around it: {} and {}.",
                    short(mean),
                    short(a),
                    short(mean - a * capsigma),
                    short(mean + a * capsigma)
                ));
            }
            chart.footnote = (!notes.is_empty()).then(|| notes.join(" "));
            chart.subtitle = c.subtitle.clone();
            self.charts.push(chart);
        } else {
            let made = title(format!("Control Chart: {process_label}"));
            let categories: Vec<Category> = points
                .iter()
                .map(|p| Category {
                    label: p.label.clone(),
                    value: p.value.clone(),
                    missing: p.missing,
                    bin: None,
                })
                .collect();
            let footnote = match c.footnote.is_empty() {
                true => format!("Sigma level: {}", short(a)),
                false => c.footnote.join(" "),
            };
            for (k, panel) in panels.iter().enumerate() {
                let (upper, lower) = panel.limits(a);
                let one = |name: String, values: Vec<f64>| Series {
                    name,
                    statistic: Statistic::Value,
                    variable: None,
                    value: Cell::Empty,
                    values,
                };
                let mut series = vec![
                    one(panel.statistic.clone(), panel.values.clone()),
                    one(legend("Center", &panel.center), panel.center.clone()),
                    one(legend("UCL", &upper), upper.clone()),
                    one(legend("LCL", &lower), lower.clone()),
                ];
                if k == 0 {
                    let n = points.len();
                    if let Some(usl) = c.usl {
                        series.push(one(legend("USL", &[usl]), vec![usl; n]));
                    }
                    if let Some(lsl) = c.lsl {
                        series.push(one(legend("LSL", &[lsl]), vec![lsl; n]));
                    }
                }
                let mut chart = Chart::lines(
                    made.clone(),
                    Axis::titled(by_title.clone()),
                    Axis::titled(panel.statistic.clone()),
                    categories.clone(),
                    series,
                );
                chart.legend = Some("Series".into());
                chart.subtitle = c.subtitle.clone();
                chart.footnote = Some(footnote.clone());
                self.charts.push(chart);
                if k == 0 && !c.rules.is_empty() {
                    let marks = violations(
                        &panel.values,
                        &panel.center,
                        &panel.sigma,
                        (&upper, &lower),
                        &c.rules,
                    );
                    let rows: Vec<Vec<Cell>> = marks
                        .iter()
                        .zip(&points)
                        .filter(|(m, _)| !m.is_empty())
                        .map(|(m, p)| {
                            let texts: Vec<&str> = m.iter().map(|r| rule_text(r)).collect();
                            vec![Cell::Text(p.id.clone()), Cell::Text(texts.join("; "))]
                        })
                        .collect();
                    let first = match c.id.as_deref() {
                        Some(id) => reader.shown.named(data, id),
                        None if by_title.is_empty() => "Point".into(),
                        None => by_title.clone(),
                    };
                    tables.push(Table {
                        title: "Rule Violations".into(),
                        row_headers: 1,
                        columns: vec![first, "Violations".into()],
                        caption: rows
                            .is_empty()
                            .then(|| "No point breaks the rules.".to_string()),
                        rows,
                        ..Table::default()
                    });
                }
            }
        }

        // the statistics table
        let requested: Vec<&str> = match (c.kind, c.statistics.is_empty()) {
            (K::Np | K::P | K::C | K::U, _) => Vec::new(),
            (K::Capability, true) => SPCHART_STATISTICS
                .iter()
                .copied()
                .filter(|k| {
                    !k.starts_with('A')
                        && !k.contains('Z')
                        && (c.target.is_some() || !matches!(*k, "CPM" | "PPM" | "TARGET"))
                })
                .collect(),
            _ => c.statistics.clone(),
        };
        if !requested.is_empty() {
            let stats = Indices {
                mean,
                sd,
                total,
                capsigma,
                lsl: c.lsl,
                usl: c.usl,
                target: c.target,
                values: &pairs,
            };
            let mut rows = Vec::new();
            for &k in SPCHART_STATISTICS.iter().filter(|k| requested.contains(k)) {
                let (value, problem) = stats.value(k);
                if let Some(problem) = problem.filter(|_| !c.statistics.is_empty()) {
                    warnings.push(format!("SPCHART: {k} is left out: {problem}"));
                    continue;
                }
                if problem.is_some() {
                    continue;
                }
                let group = match SPCHART_STATISTICS.iter().position(|s| *s == k) {
                    Some(0..14) => "Process Capability Indices",
                    Some(14..27) => "Process Performance Indices",
                    Some(27..34) => "Process Data",
                    _ => "Assessing Normality",
                };
                rows.push(vec![
                    Cell::Text(group.into()),
                    Cell::Text(statistic_label(k).into()),
                    value.map_or(Cell::Empty, Cell::Number),
                ]);
            }
            if !rows.is_empty() {
                tables.push(Table {
                    title: "Process Statistics".into(),
                    row_headers: 2,
                    columns: vec![String::new(), String::new(), "Value".into()],
                    rows,
                    caption: Some(format!(
                        "Capability sigma from {}.",
                        match method {
                            CapSigma::Rbar => "the mean range",
                            CapSigma::Sbar => "the mean standard deviation",
                            CapSigma::Mrbar => "the mean moving range",
                            CapSigma::Within => "the within-subgroup variance",
                        }
                    )),
                    ..Table::default()
                });
            }
        }
        for w in warnings {
            self.warn(w);
        }
        Ok(tables)
    }
}

fn too_few(points: usize) -> Error {
    Error(format!(
        "SPCHART: a chart needs 2 to {MOST_POINTS} points, the data give {points}"
    ))
}

fn statistic_label(k: &str) -> &'static str {
    match k {
        "CP" => "Cp",
        "CPL" => "CpL",
        "CPU" => "CpU",
        "K" => "K",
        "CPK" => "CpK",
        "CR" => "CR",
        "CPM" => "CpM",
        "CZL" => "Z lower",
        "CZU" => "Z upper",
        "CZMIN" => "Z minimum",
        "CZMAX" => "Z maximum",
        "CZOUT" => "Estimated % outside the limits",
        "CZLOUT" => "Estimated % below LSL",
        "CZUOUT" => "Estimated % above USL",
        "PP" => "Pp",
        "PPL" => "PpL",
        "PPU" => "PpU",
        "PPK" => "PpK",
        "PR" => "PR",
        "PPM" => "PpM",
        "PZL" => "Z lower",
        "PZU" => "Z upper",
        "PZMIN" => "Z minimum",
        "PZMAX" => "Z maximum",
        "PZOUT" => "Estimated % outside the limits",
        "PZLOUT" => "Estimated % below LSL",
        "PZUOUT" => "Estimated % above USL",
        "N" => "N",
        "MEAN" => "Mean",
        "STDDEV" => "Standard deviation",
        "CAPSIGMA" => "Capability sigma",
        "LSL" => "Lower specification limit",
        "USL" => "Upper specification limit",
        "TARGET" => "Target",
        "AZOUT" => "Observed % outside the limits",
        "AZLOUT" => "Observed % at or below LSL",
        _ => "Observed % at or above USL",
    }
}

/// What the capability and performance statistics are computed from.
struct Indices<'a> {
    mean: f64,
    sd: f64,
    total: f64,
    capsigma: f64,
    lsl: Option<f64>,
    usl: Option<f64>,
    target: Option<f64>,
    values: &'a [(f64, f64)],
}

impl Indices<'_> {
    /// A statistic's value (None where it is not defined), or why it cannot be given.
    fn value(&self, k: &str) -> (Option<f64>, Option<&'static str>) {
        let positive = |s: f64| (s.is_finite() && s > 0.0).then_some(s);
        let both = self.lsl.zip(self.usl);
        let m = self.mean;
        let phi = dist::normal_cdf;
        // the index scaled by sigma `s`: z values, then the named statistic
        let scaled = |s: Option<f64>, k: &str| -> Option<f64> {
            let s = s?;
            let zl = self.lsl.map(|l| (m - l) / s);
            let zu = self.usl.map(|u| (u - m) / s);
            Some(match k {
                "P" => (both?.1 - both?.0) / (6.0 * s),
                "L" => zl? / 3.0,
                "U" => zu? / 3.0,
                "K" => match (zl, zu) {
                    (Some(l), Some(u)) => l.min(u) / 3.0,
                    (Some(l), None) => l / 3.0,
                    (None, Some(u)) => u / 3.0,
                    _ => return None,
                },
                "R" => 6.0 * s / (both?.1 - both?.0),
                "M" => {
                    let t = self.target?;
                    (both?.1 - both?.0) / (6.0 * (s * s + (m - t).powi(2)).sqrt())
                }
                "ZL" => zl?,
                "ZU" => zu?,
                "ZMIN" => zl?.min(zu?),
                "ZMAX" => zl?.max(zu?),
                "ZOUT" => 100.0 * (1.0 - phi(zu?) + phi(-zl?)),
                "ZLOUT" => 100.0 * phi(-zl?),
                _ => 100.0 * (1.0 - phi(zu?)),
            })
        };
        let need_both = !matches!(
            k,
            "CPK" | "PPK" | "N" | "MEAN" | "STDDEV" | "CAPSIGMA" | "LSL" | "USL" | "TARGET"
        );
        if need_both && both.is_none() {
            return (None, Some("it needs both LSL and USL"));
        }
        if matches!(k, "CPK" | "PPK") && self.lsl.is_none() && self.usl.is_none() {
            return (None, Some("it needs LSL or USL"));
        }
        if matches!(k, "CPM" | "PPM") {
            match (self.target, both) {
                (None, _) => return (None, Some("it needs TARGET")),
                (Some(t), Some((l, u))) if t < l || t > u => {
                    return (None, Some("TARGET lies outside the specification limits"));
                }
                _ => {}
            }
        }
        let outside = |low: bool, high: bool| {
            let (l, u) = both?;
            let out: f64 = self
                .values
                .iter()
                .filter(|(x, _)| (low && *x <= l) || (high && *x >= u))
                .map(|p| p.1)
                .sum();
            Some(100.0 * out / self.total)
        };
        let value = match k {
            "K" => both.map(|(l, u)| 2.0 * ((u + l) / 2.0 - m).abs() / (u - l)),
            "N" => Some(self.total),
            "MEAN" => Some(m),
            "STDDEV" => Some(self.sd),
            "CAPSIGMA" => Some(self.capsigma),
            "LSL" => self.lsl,
            "USL" => self.usl,
            "TARGET" => self.target,
            "AZOUT" => outside(true, true),
            "AZLOUT" => outside(true, false),
            "AZUOUT" => outside(false, true),
            _ => {
                let sigma = match k.as_bytes()[0] {
                    b'C' => positive(self.capsigma),
                    _ => positive(self.sd),
                };
                // CP and PP -> "P", CPL and PPL -> "L", CR and PR -> "R", CZL and PZL -> "ZL"
                let rest = &k[1..];
                let name = match rest.len() == 2 && rest.starts_with('P') {
                    true => &rest[1..],
                    false => rest,
                };
                scaled(sigma, name)
            }
        };
        (value.filter(|v| v.is_finite()), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_constants_match_numerical_integration() {
        // d2, d3 and c4 from R's integrate() and lgamma() over the same definitions
        for (n, d2, d3) in [
            (2, std::f64::consts::FRAC_2_SQRT_PI, 0.85250247),
            (5, 2.32592895, 0.86408194),
            (10, 3.07750546, 0.79705067),
            (25, 3.93062922, 0.70844077),
            (100, 5.01518727, 0.60517911),
        ] {
            let (got2, got3) = range_constants(n);
            assert!((got2 - d2).abs() < 1e-7, "d2({n}) = {got2}");
            assert!((got3 - d3).abs() < 1e-7, "d3({n}) = {got3}");
        }
        assert!((c4(5.0) - 0.93998560).abs() < 1e-8);
        assert!((c4(2.0) - 0.79788456).abs() < 1e-8);
    }

    #[test]
    fn rules_mark_the_last_point_of_each_pattern() {
        let n = 16;
        let center = vec![0.0; n];
        let sigma = vec![1.0; n];
        let upper = vec![3.0; n];
        let lower = vec![-3.0; n];
        let rules = statscraft_syntax::SPCHART_RULES.to_vec();
        let check = |x: &[f64]| {
            violations(
                x,
                &center[..x.len()],
                &sigma[..x.len()],
                (&upper, &lower),
                &rules,
            )
        };
        // 3.5 breaks UCL; 2.5, 0, 2.5: the third point ends a 2 of 3 pattern
        let m = check(&[3.5, 2.5, 0.0, 2.5, -3.2]);
        assert_eq!(m[0], ["UCL"]);
        assert!(m[1].is_empty() && m[2].is_empty());
        assert_eq!(m[3], ["R2UPPER"]);
        assert_eq!(m[4], ["LCL"]);
        // eight points above the center: the eighth and ninth are marked
        let m = check(&[0.1, 0.2, 0.1, 0.3, 0.1, 0.2, 0.1, 0.4, 0.2]);
        assert_eq!(m[7], ["R8UPPER"]);
        assert_eq!(m[8], ["R8UPPER"]);
        // six rising points
        let m = check(&[-0.5, -0.4, -0.3, -0.2, -0.1, 0.0]);
        assert_eq!(m[5], ["TRUP"]);
        assert!(m[4].is_empty());
        // fourteen points up and down in turn, around the center so no other rule fires
        let x: Vec<f64> = (0..14)
            .map(|i| if i % 2 == 0 { -0.5 } else { 0.5 })
            .collect();
        let m = check(&x);
        assert_eq!(m[13], ["ALTERNATING"]);
        assert!(m[12].is_empty());
        // four of five above one sigma, the last one too
        let m = check(&[1.5, 1.2, -0.5, 1.1, 1.3]);
        assert_eq!(m[4], ["R4UPPER"]);
    }
}
