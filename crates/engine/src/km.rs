//! KM: product-limit (Kaplan-Meier) estimates of the survival function for each factor level
//! within each stratum, with the log rank, Breslow and Tarone-Ware tests that compare levels.
//!
//! Rules from the Command Syntax Reference (KM) and the Algorithms manual (KM Algorithms), in
//! our words:
//! - A case takes part when its time, status, factor and stratum are valid and its time is 0
//!   or more. A status among the EVENT values is an event at that time; every other status is
//!   censored, and LOST values are censored too but counted apart. Weights count cases.
//! - At each distinct event time t_i with n_i cases still at risk (time at or after t_i) and
//!   d_i events, the estimate drops by the factor 1 - d_i/n_i. Its variance is Greenwood's:
//!   S^2 times the sum of d_i / (n_i (n_i - d_i)); once S reaches 0 the error is 0.
//! - The mean is the area under the step curve from 0 to the last event time, plus the last
//!   level times the stretch up to the longest time when that one is censored. Its variance
//!   sums a_i^2 d_i / (n_i (n_i - d_i)), a_i being the area to the right of t_i; when the
//!   longest time holds both an event and a censored case, the last term is dropped and the
//!   sum is multiplied by d / (d - 1), d the number of events.
//! - The p percentile (p from PERCENTILES over 100) is the first event time at which the
//!   estimate is p or lower. Its variance is var(S) there over the square of the slope of the
//!   curve between the last time at which S is still p + 0.05 or more (time 0 counts, with S
//!   equal to 1) and the first time at which S is p - 0.05 or less.
//! - Tests: at each event time of a stratum, each level's events minus the expected share
//!   d n_l / n are weighted (1 for log rank, n for Breslow, the square root of n for
//!   Tarone-Ware) and summed over the times and the strata into U; V sums w^2 d (n - d) / (n - 1)
//!   (n_j / n)(delta_jl - n_l / n). The statistic is U' V^- U with as many degrees of freedom as
//!   the rank of V over all levels but one. TREND turns it into (t'U)^2 / (t'Vt) with one
//!   degree of freedom, t the weights of the levels in ascending order.

use crate::chart::{Axis, Chart, ChartKind};
use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, normal_quantile};
use crate::glm::ginverse;
use crate::means::is_valid;
use crate::plot::{Plot, Point, PointGroup};
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Measure, Values, Variable};
use statscraft_syntax::{Bound, Km, KmCompare, KmPlot, KmSave, KmTest, MissingValue, Value};

/// One case: its time, whether the event happened and its weight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Obs {
    pub time: f64,
    pub event: bool,
    pub lost: bool,
    pub weight: f64,
    pub case: usize,
}

/// Times ascending, an event before a censored case at the same time.
pub(crate) fn sort_obs(obs: &mut [Obs]) {
    obs.sort_by(|a, b| a.time.total_cmp(&b.time).then(b.event.cmp(&a.event)));
}

/// The product-limit curve at its distinct event times.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Curve {
    pub times: Vec<f64>,
    pub at_risk: Vec<f64>,
    pub events: Vec<f64>,
    pub surv: Vec<f64>,
    pub se: Vec<f64>,
    /// The events at or before each event time.
    pub cumulative: Vec<f64>,
    /// The longest time, event or censored.
    pub longest: f64,
    /// The longest time holds both an event and a censored case.
    pub tied_at_end: bool,
}

pub(crate) fn curve(obs: &[Obs]) -> Curve {
    let mut sorted = obs.to_vec();
    sort_obs(&mut sorted);
    let mut c = Curve {
        longest: sorted.last().map_or(0.0, |o| o.time),
        ..Curve::default()
    };
    // those still at risk from each case on, summed from the end so the last ones are exact
    let mut from = vec![0.0; sorted.len() + 1];
    for i in (0..sorted.len()).rev() {
        from[i] = from[i + 1] + sorted[i].weight;
    }
    let (mut s, mut greenwood) = (1.0, 0.0);
    let mut i = 0;
    while i < sorted.len() {
        let t = sorted[i].time;
        let same = sorted[i..].iter().take_while(|o| o.time == t).count();
        let d: f64 = sorted[i..i + same]
            .iter()
            .filter(|o| o.event)
            .map(|o| o.weight)
            .sum();
        if d > 0.0 {
            let n = from[i];
            // fractional weights must not leave a sliver where everyone at risk had the event
            let all = n - d <= 1e-12 * n;
            s = if all { 0.0 } else { s * (1.0 - d / n) };
            if !all {
                greenwood += d / (n * (n - d));
            }
            c.times.push(t);
            c.at_risk.push(n);
            c.events.push(d);
            c.surv.push(s);
            c.se.push(if s > 0.0 { s * greenwood.sqrt() } else { 0.0 });
            c.cumulative.push(c.cumulative.last().unwrap_or(&0.0) + d);
        }
        if i + same == sorted.len() {
            c.tied_at_end = d > 0.0 && sorted[i..].iter().any(|o| !o.event);
        }
        i += same;
    }
    c
}

impl Curve {
    /// The estimate and its error just after time `t`.
    pub(crate) fn at(&self, t: f64) -> (f64, f64) {
        match self.times.partition_point(|&x| x <= t) {
            0 => (1.0, 0.0),
            i => (self.surv[i - 1], self.se[i - 1]),
        }
    }

    /// Events at or before `t`.
    fn events_by(&self, t: f64) -> f64 {
        match self.times.partition_point(|&x| x <= t) {
            0 => 0.0,
            i => self.cumulative[i - 1],
        }
    }

    /// The mean survival time and its standard error.
    pub(crate) fn mean(&self) -> (f64, Option<f64>) {
        let k = self.times.len();
        if k == 0 {
            return (self.longest, None);
        }
        let level = |i: usize| if i == 0 { 1.0 } else { self.surv[i - 1] };
        let start = |i: usize| if i == 0 { 0.0 } else { self.times[i - 1] };
        let tail = self.surv[k - 1] * (self.longest - self.times[k - 1]);
        let mean = (0..k)
            .map(|i| level(i) * (start(i + 1) - start(i)))
            .sum::<f64>()
            + tail;
        // a_i: the area right of the i-th event time
        let mut a = vec![0.0; k];
        let mut right = tail;
        for i in (0..k).rev() {
            a[i] = right;
            if i > 0 {
                right += self.surv[i - 1] * (self.times[i] - self.times[i - 1]);
            }
        }
        let term = |i: usize| {
            let (n, d) = (self.at_risk[i], self.events[i]);
            if n > d {
                a[i] * a[i] * d / (n * (n - d))
            } else {
                0.0
            }
        };
        let variance = if self.tied_at_end {
            let d: f64 = self.events.iter().sum();
            (d > 1.0).then(|| d / (d - 1.0) * (0..k - 1).map(term).sum::<f64>())
        } else {
            Some((0..k).map(term).sum())
        };
        (mean, variance.map(f64::sqrt))
    }

    /// The `p` percentile (0 < p < 1) and its standard error.
    pub(crate) fn percentile(&self, p: f64) -> (Option<f64>, Option<f64>) {
        const TOL: f64 = 1e-12;
        let Some(i) = self.surv.iter().position(|&s| s <= p + TOL) else {
            return (None, None);
        };
        let tp = self.times[i];
        let hi = p + 0.05;
        // the last time at which S is still hi or more, time 0 included
        let upper = std::iter::once((0.0, 1.0))
            .chain(self.times.iter().copied().zip(self.surv.iter().copied()))
            .rfind(|&(_, s)| s >= hi - TOL);
        let lower = self
            .times
            .iter()
            .zip(&self.surv)
            .find(|&(_, &s)| s <= p - 0.05 + TOL);
        let se = match (upper, lower) {
            (Some((u, su)), Some((&l, &sl))) if l > u => {
                let slope = (su - sl) / (l - u);
                (slope > 0.0).then(|| self.se[i] / slope)
            }
            _ => None,
        };
        (Some(tp), se)
    }

    /// The step curve through `f` of the estimate, corners doubled, censored times marked.
    fn steps(&self, censored: &[f64], f: impl Fn(f64) -> f64) -> Vec<Point> {
        let point = |x: f64, s: f64, mark: bool| Point {
            x,
            y: f(s),
            z: None,
            label: mark.then(|| "Censored".to_string()),
        };
        let mut out = vec![point(0.0, 1.0, false)];
        let (mut level, mut e, mut c) = (1.0, 0, 0);
        while e < self.times.len() || c < censored.len() {
            if e < self.times.len() && (c == censored.len() || self.times[e] <= censored[c]) {
                out.push(point(self.times[e], level, false));
                level = self.surv[e];
                out.push(point(self.times[e], level, false));
                e += 1;
            } else {
                out.push(point(censored[c], level, true));
                c += 1;
            }
        }
        if out.last().is_some_and(|p| p.x < self.longest) {
            out.push(point(self.longest, level, false));
        }
        // the log of 0 has no place on the axis: the curve ends there
        out.retain(|p| p.y.is_finite());
        out
    }
}

/// The weights of the trend test when none are given: evenly spaced, centered on 0.
pub(crate) fn default_trend(g: usize) -> Vec<f64> {
    (1..=g)
        .map(|l| {
            let w = 2.0 * l as f64 - g as f64 - 1.0;
            if g.is_multiple_of(2) { w } else { w / 2.0 }
        })
        .collect()
}

/// The test statistic and its degrees of freedom. `strata` holds, per stratum, each case
/// with the index of its level among `g`.
pub(crate) fn test_statistic(
    strata: &[Vec<(usize, Obs)>],
    g: usize,
    test: KmTest,
    trend: Option<&[f64]>,
) -> Option<(f64, usize)> {
    let mut u = vec![0.0; g];
    let mut v = vec![vec![0.0; g]; g];
    for stratum in strata {
        let mut cases = stratum.clone();
        cases.sort_by(|a, b| a.1.time.total_cmp(&b.1.time));
        let mut at_risk = vec![0.0; g];
        for (l, o) in &cases {
            at_risk[*l] += o.weight;
        }
        let mut i = 0;
        while i < cases.len() {
            let t = cases[i].1.time;
            let same = cases[i..].iter().take_while(|c| c.1.time == t).count();
            let mut d = vec![0.0; g];
            for (l, o) in cases[i..i + same].iter().filter(|c| c.1.event) {
                d[*l] += o.weight;
            }
            let n: f64 = at_risk.iter().sum();
            let dt: f64 = d.iter().sum();
            // one case at risk adds nothing to U or V
            if dt > 0.0 && n > 1.0 {
                let w = match test {
                    KmTest::Logrank => 1.0,
                    KmTest::Breslow => n,
                    KmTest::Tarone => n.sqrt(),
                };
                for l in 0..g {
                    u[l] += w * (d[l] - dt * at_risk[l] / n);
                }
                let c = w * w * dt * (n - dt) / (n - 1.0);
                for j in 0..g {
                    for l in 0..g {
                        let delta = if j == l { 1.0 } else { 0.0 };
                        v[j][l] += c * at_risk[j] / n * (delta - at_risk[l] / n);
                    }
                }
            }
            for (l, o) in &cases[i..i + same] {
                at_risk[*l] -= o.weight;
            }
            i += same;
        }
    }
    if let Some(t) = trend {
        let tu: f64 = t.iter().zip(&u).map(|(a, b)| a * b).sum();
        let tvt: f64 = (0..g)
            .map(|j| (0..g).map(|l| t[j] * v[j][l] * t[l]).sum::<f64>())
            .sum();
        return (tvt > 0.0).then(|| (tu * tu / tvt, 1));
    }
    let m = g.checked_sub(1)?;
    let sub: Vec<Vec<f64>> = v[..m].iter().map(|r| r[..m].to_vec()).collect();
    let (inverse, rank) = ginverse(&sub, 1e-10);
    let chi: f64 = (0..m)
        .map(|j| (0..m).map(|l| u[j] * inverse[j][l] * u[l]).sum::<f64>())
        .sum();
    (rank > 0).then_some((chi.max(0.0), rank))
}

/// The value matches one of the values or ranges of STATUS.
fn matches(list: &[MissingValue], value: &Key) -> bool {
    list.iter().any(|item| match (item, value) {
        (MissingValue::Value(Value::Number(n)), Key::Number(x)) => n == x,
        (MissingValue::Value(Value::String(s)), Key::Text(x)) => s.trim_end() == x.trim_end(),
        (MissingValue::Range(lo, hi), Key::Number(x)) => {
            let above = match lo {
                Bound::Lowest => true,
                Bound::Highest => false,
                Bound::Number(n) => x >= n,
            };
            let below = match hi {
                Bound::Highest => true,
                Bound::Lowest => false,
                Bound::Number(n) => x <= n,
            };
            above && below
        }
        _ => false,
    })
}

fn test_name(test: KmTest) -> &'static str {
    match test {
        KmTest::Logrank => "Log Rank (Mantel-Cox)",
        KmTest::Breslow => "Breslow (Generalized Wilcoxon)",
        KmTest::Tarone => "Tarone-Ware",
    }
}

/// A percentage as a header: 25.0%, 2.5%.
fn percent_label(p: f64) -> String {
    let text = format!("{p:.4}");
    let text = text.trim_end_matches('0');
    match text.strip_suffix('.') {
        Some(whole) => format!("{whole}.0%"),
        None => format!("{text}%"),
    }
}

/// A case with its stratum and factor level.
type KeyedObs = (Option<Key>, Option<Key>, Obs);

/// The curves of one stratum: its key and, per factor level, the cases.
struct Stratum {
    key: Option<Key>,
    levels: Vec<Vec<Obs>>,
}

impl Engine {
    pub(crate) fn km(&mut self, command: &Km) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let time_var = data.variable(&command.time)?;
        if time_var.is_string() {
            return Err(Error(format!(
                "KM needs a numeric time variable, but {} is a string variable",
                command.time
            )));
        }
        let status_var = data.variable(&command.status)?;
        // a string status takes quoted values only, a numeric one numbers and ranges
        let fits = |v: &MissingValue| {
            matches!(v, MissingValue::Value(Value::String(_))) == status_var.is_string()
        };
        if !command.event.iter().chain(&command.lost).all(fits) {
            return Err(Error(format!(
                "the EVENT and LOST values must have the type of {}: quoted values for a string \
                 status, numbers or THRU ranges for a numeric one",
                command.status
            )));
        }
        let factor_var = command
            .factor
            .as_ref()
            .map(|f| data.variable(f))
            .transpose()?;
        let strata_var = command
            .strata
            .as_ref()
            .map(|s| data.variable(s))
            .transpose()?;
        let id_var = command.id.as_ref().map(|s| data.variable(s)).transpose()?;
        let weights = case_weights(data)?;
        let weight = |c: usize| weights.as_ref().map_or(1.0, |w| w[c]);
        let times = match time_var.values() {
            Values::Numeric(v) => v,
            Values::String(_) => unreachable!("checked above"),
        };
        let status = categories(&status_var, false);
        // -0 and 0 are one level
        let keys = |v: &Variable| -> Vec<Option<Key>> {
            let mut out = categories(v, false);
            for k in out.iter_mut().flatten() {
                if let Key::Number(x) = k {
                    *x += 0.0;
                }
            }
            out
        };
        let factor = factor_var.as_ref().map(keys);
        let strata = strata_var.as_ref().map(keys);

        // the valid cases with their stratum and level keys
        let mut cases: Vec<KeyedObs> = Vec::new();
        let mut excluded = 0.0;
        for c in (0..data.case_count()).filter(|&c| weight(c) > 0.0) {
            let time = times.get(c).filter(|_| is_valid(&time_var, c, false));
            let level = factor.as_ref().map(|f| f[c].clone());
            let stratum = strata.as_ref().map(|s| s[c].clone());
            let (Some(time), Some(code)) = (time, status[c].clone()) else {
                excluded += weight(c);
                continue;
            };
            if time < 0.0 || level == Some(None) || stratum == Some(None) {
                excluded += weight(c);
                continue;
            }
            let event = matches(&command.event, &code);
            cases.push((
                stratum.flatten(),
                level.flatten(),
                Obs {
                    time,
                    event,
                    lost: !event && matches(&command.lost, &code),
                    weight: weight(c),
                    case: c,
                },
            ));
        }
        if cases.is_empty() {
            return Err(Error("KM has no valid cases".into()));
        }
        let order = |a: &Option<Key>, b: &Option<Key>| match (a, b) {
            (Some(a), Some(b)) => a.compare(b),
            _ => std::cmp::Ordering::Equal,
        };
        let sorted_keys = |pick: &dyn Fn(&KeyedObs) -> Option<Key>| {
            let mut keys: Vec<Option<Key>> = cases.iter().map(pick).collect();
            keys.sort_by(order);
            keys.dedup_by(|a, b| order(a, b).is_eq());
            keys
        };
        let stratum_keys = sorted_keys(&|c| c.0.clone());
        let level_keys = sorted_keys(&|c| c.1.clone());
        let g = level_keys.len();
        let mut strata_list: Vec<Stratum> = stratum_keys
            .iter()
            .map(|sk| Stratum {
                key: sk.clone(),
                levels: vec![Vec::new(); g],
            })
            .collect();
        for (sk, lk, o) in &cases {
            let s = stratum_keys.binary_search_by(|k| order(k, sk));
            let l = level_keys.binary_search_by(|k| order(k, lk));
            strata_list[s.expect("listed")].levels[l.expect("listed")].push(*o);
        }
        for level in strata_list.iter_mut().flat_map(|s| &mut s.levels) {
            sort_obs(level);
        }

        let stratum_text = |k: &Option<Key>| match (k, &strata_var) {
            (Some(k), Some(v)) => shown.value(v, k),
            _ => String::new(),
        };
        let level_text = |k: &Option<Key>| match (k, &factor_var) {
            (Some(k), Some(v)) => shown.value(v, k),
            _ => String::new(),
        };
        // the curves in output order: per stratum, per level, those with cases
        let mut curves: Vec<(usize, usize, Curve)> = Vec::new();
        let mut curve_at = vec![vec![None; g]; strata_list.len()];
        for (s, stratum) in strata_list.iter().enumerate() {
            for (l, obs) in stratum.levels.iter().enumerate() {
                if !obs.is_empty() {
                    curve_at[s][l] = Some(curves.len());
                    curves.push((s, l, curve(obs)));
                }
            }
        }
        let mut headers: Vec<String> = Vec::new();
        if let Some(v) = &strata_var {
            headers.push(shown.variable(v));
        }
        if let Some(v) = &factor_var {
            headers.push(shown.variable(v));
        }
        let several = curves.len() > 1;
        let row_labels = |s: usize, l: usize, first: bool| -> Vec<Cell> {
            let mut out = Vec::new();
            if strata_var.is_some() {
                let text = match first {
                    true => stratum_text(&strata_list[s].key),
                    false => String::new(),
                };
                out.push(Cell::Text(text));
            }
            if factor_var.is_some() {
                out.push(Cell::Text(level_text(&level_keys[l])));
            }
            out
        };
        let overall_labels = |text: &str| -> Vec<Cell> {
            let mut out = vec![Cell::Text(text.into())];
            out.resize(headers.len().max(1), Cell::Text(String::new()));
            out
        };
        let label_columns = || -> Vec<String> {
            match headers.is_empty() {
                true => vec![String::new()],
                false => headers.clone(),
            }
        };
        let solo_label = |s: usize, l: usize, first: bool| -> Vec<Cell> {
            match headers.is_empty() {
                true => vec![Cell::Text("Overall".into())],
                false => row_labels(s, l, first),
            }
        };
        let first_in_stratum = |i: usize| i == 0 || curves[i - 1].0 != curves[i].0;
        let lost_given = !command.lost.is_empty();

        let (mut warnings, mut charts): (Vec<String>, Vec<Chart>) = (Vec::new(), Vec::new());
        if command.status_repeated {
            warnings.push("STATUS was given more than once; the last one was used".into());
        }
        let case_count = data.case_count();

        // Case Processing Summary
        let mut tables = Vec::new();
        let mut columns = label_columns();
        columns.extend(["Total N", "N of Events", "N", "Percent"].map(String::from));
        let mut column_groups = vec![Vec::new(); headers.len().max(1) + 2];
        column_groups.extend([vec!["Censored".to_string()], vec!["Censored".to_string()]]);
        if lost_given {
            columns.extend(["N", "Percent"].map(String::from));
            column_groups.extend([vec!["Lost".to_string()], vec!["Lost".to_string()]]);
        }
        let summary_row = |obs: &mut dyn Iterator<Item = &Obs>| -> Vec<Cell> {
            let (mut n, mut e, mut cen, mut lost) = (0.0, 0.0, 0.0, 0.0);
            for o in obs {
                n += o.weight;
                if o.event {
                    e += o.weight;
                } else if o.lost {
                    lost += o.weight;
                } else {
                    cen += o.weight;
                }
            }
            if !lost_given {
                cen += lost;
            }
            let mut row = vec![
                Cell::Number(n),
                Cell::Number(e),
                Cell::Number(cen),
                Cell::Number(100.0 * cen / n),
            ];
            if lost_given {
                row.extend([Cell::Number(lost), Cell::Number(100.0 * lost / n)]);
            }
            row
        };
        let mut rows = Vec::new();
        for (i, (s, l, _)) in curves.iter().enumerate() {
            let mut row = solo_label(*s, *l, first_in_stratum(i));
            row.extend(summary_row(&mut strata_list[*s].levels[*l].iter()));
            rows.push(row);
        }
        if several {
            let mut row = overall_labels("Overall");
            row.extend(summary_row(&mut cases.iter().map(|c| &c.2)));
            rows.push(row);
        }
        let caption = (excluded > 0.0).then(|| {
            format!(
                "{} cases with a missing value or a negative time were left out.",
                crate::format::format_value(excluded, "F8.0", false).trim()
            )
        });
        tables.push(Table {
            title: "Case Processing Summary".into(),
            row_headers: headers.len().max(1),
            columns,
            column_groups,
            rows,
            caption,
            ..Table::default()
        });

        // Survival Table
        if command.table {
            let mut columns = headers.clone();
            if let Some(v) = &id_var {
                columns.push(shown.variable(v));
            }
            let lead = columns.len();
            columns.extend(
                [
                    "Time",
                    "Status",
                    "Estimate",
                    "Std. Error",
                    "N of Cumulative Events",
                    "N of Remaining Cases",
                ]
                .map(String::from),
            );
            let mut column_groups = vec![Vec::new(); lead + 2];
            let at_time = "Cumulative Proportion Surviving at the Time".to_string();
            column_groups.extend([vec![at_time.clone()], vec![at_time], vec![], vec![]]);
            let mut rows = Vec::new();
            for (i, (s, l, c)) in curves.iter().enumerate() {
                let obs = &strata_list[*s].levels[*l];
                // the cases after each row, summed from the end so the last row is exactly 0
                let mut after = vec![0.0; obs.len() + 1];
                for k in (0..obs.len()).rev() {
                    after[k] = after[k + 1] + obs[k].weight;
                }
                let mut cumulative = 0.0;
                for (k, o) in obs.iter().enumerate() {
                    let mut row = match headers.is_empty() {
                        true => Vec::new(),
                        false if k == 0 => row_labels(*s, *l, first_in_stratum(i)),
                        false => vec![Cell::Text(String::new()); headers.len()],
                    };
                    if let Some(v) = &id_var {
                        row.push(Cell::Text(id_text(&shown, v, o.case)));
                    }
                    if o.event {
                        cumulative += o.weight;
                    }
                    let last_event_at_time =
                        o.event && obs.get(k + 1).is_none_or(|n| n.time != o.time || !n.event);
                    let (estimate, se) = match last_event_at_time {
                        true => {
                            let (s, e) = c.at(o.time);
                            (Cell::Number(s), Cell::Number(e))
                        }
                        false => (Cell::Empty, Cell::Empty),
                    };
                    let status = match (o.event, o.lost) {
                        (true, _) => "Event",
                        (false, true) => "Lost",
                        (false, false) => "Censored",
                    };
                    row.extend([
                        Cell::Number(o.time),
                        Cell::Text(status.into()),
                        estimate,
                        se,
                        Cell::Number(cumulative),
                        Cell::Number(after[k + 1]),
                    ]);
                    rows.push(row);
                }
            }
            tables.push(Table {
                title: "Survival Table".into(),
                row_headers: lead,
                columns,
                column_groups,
                rows,
                ..Table::default()
            });
        }

        // Means and Medians, Percentiles
        let pooled = (several).then(|| curve(&cases.iter().map(|c| c.2).collect::<Vec<_>>()));
        let z = normal_quantile(0.975);
        let all_curves = || {
            curves
                .iter()
                .enumerate()
                .map(|(i, (s, l, c))| (solo_label(*s, *l, first_in_stratum(i)), c))
                .chain(pooled.iter().map(|c| (overall_labels("Overall"), c)))
        };
        if command.mean {
            let mut columns = label_columns();
            let parts = ["Estimate", "Std. Error", "Lower Bound", "Upper Bound"];
            columns.extend(parts.iter().chain(&parts).map(|s| s.to_string()));
            let mut column_groups = vec![Vec::new(); headers.len().max(1)];
            for what in ["Mean", "Median"] {
                let ci = "95% Confidence Interval".to_string();
                column_groups.extend([
                    vec![what.to_string()],
                    vec![what.to_string()],
                    vec![what.to_string(), ci.clone()],
                    vec![what.to_string(), ci],
                ]);
            }
            let mut rows = Vec::new();
            let mut limited = false;
            for (mut row, c) in all_curves() {
                let (mean, se) = c.mean();
                limited |= c.times.last().is_none_or(|&t| t < c.longest);
                let bounds = |e: f64, se: Option<f64>| -> [Cell; 4] {
                    [
                        Cell::Number(e),
                        number(se),
                        number(se.map(|s| e - z * s)),
                        number(se.map(|s| e + z * s)),
                    ]
                };
                row.extend(bounds(mean, se));
                match c.percentile(0.5) {
                    (Some(m), se) => row.extend(bounds(m, se)),
                    (None, _) => row.extend(std::iter::repeat_n(Cell::Empty, 4)),
                }
                rows.push(row);
            }
            let caption = limited
                .then(|| "Where the longest time is censored, the mean stops at it.".to_string());
            tables.push(Table {
                title: "Means and Medians for Survival Time".into(),
                row_headers: headers.len().max(1),
                columns,
                column_groups,
                rows,
                caption,
                ..Table::default()
            });
        }
        if !command.percentiles.is_empty() {
            let mut columns = label_columns();
            let mut column_groups = vec![Vec::new(); headers.len().max(1)];
            for &p in &command.percentiles {
                columns.extend(["Estimate", "Std. Error"].map(String::from));
                column_groups.extend([vec![percent_label(p)], vec![percent_label(p)]]);
            }
            let mut rows = Vec::new();
            for (mut row, c) in all_curves() {
                for &p in &command.percentiles {
                    let (t, se) = c.percentile(p / 100.0);
                    row.extend([number(t), number(se)]);
                }
                rows.push(row);
            }
            tables.push(Table {
                title: "Percentiles".into(),
                row_headers: headers.len().max(1),
                columns,
                column_groups,
                rows,
                caption: Some(
                    "The p% value is the time by which the estimated survival has fallen to p%."
                        .into(),
                ),
                ..Table::default()
            });
        }

        // comparisons of the factor levels
        let asked = !command.tests.is_empty() || !command.compare.is_empty();
        if factor_var.is_some() && g < 2 && (asked || command.trend.is_some()) {
            warnings.push(
                "the factor has only one level with valid cases, so nothing was compared".into(),
            );
        } else if factor_var.is_none() && (asked || command.trend.is_some()) {
            warnings.push(
                "KM compares factor levels only when BY names a factor; TEST, COMPARE and \
                 TREND were ignored"
                    .into(),
            );
        } else if let Some(factor_v) = &factor_var
            && g > 1
        {
            let tests = match command.tests.is_empty() {
                true => vec![KmTest::Logrank],
                false => command.tests.clone(),
            };
            let compares = match command.compare.is_empty() {
                true => vec![KmCompare {
                    pairwise: false,
                    per_stratum: false,
                }],
                false => command.compare.clone(),
            };
            let trend = match &command.trend {
                None => None,
                Some(w) if w.is_empty() => Some(default_trend(g)),
                Some(w) if w.len() < g => {
                    return Err(Error(format!(
                        "TREND gives {} weights, but {} has {g} levels",
                        w.len(),
                        shown.variable(factor_v)
                    )));
                }
                Some(w) => {
                    if w.len() > g {
                        warnings.push(format!(
                            "TREND gives {} weights for {g} levels; the extra ones were ignored",
                            w.len()
                        ));
                    }
                    Some(w[..g].to_vec())
                }
            };
            let level_sets: Vec<Vec<bool>> = strata_list
                .iter()
                .map(|s| s.levels.iter().map(|o| !o.is_empty()).collect())
                .collect();
            let differ = level_sets.windows(2).any(|w| w[0] != w[1]);
            let mut done = Vec::new();
            for cmp in compares {
                // STRATA without a STRATA variable is the pooled comparison
                let per_stratum = cmp.per_stratum && strata_var.is_some();
                if done.contains(&(cmp.pairwise, per_stratum)) {
                    continue;
                }
                done.push((cmp.pairwise, per_stratum));
                if !per_stratum && strata_var.is_some() && differ {
                    let w = "the factor has different levels in different strata, so the \
                             comparison pooled over the strata was left out";
                    if !warnings.iter().any(|x| x == w) {
                        warnings.push(w.into());
                    }
                    continue;
                }
                // the groups of strata each statistic pools
                let pools: Vec<(Option<usize>, Vec<usize>)> = match per_stratum {
                    true => (0..strata_list.len()).map(|s| (Some(s), vec![s])).collect(),
                    false => vec![(None, (0..strata_list.len()).collect())],
                };
                let pairs: Vec<Option<(usize, usize)>> = match cmp.pairwise {
                    true => (0..g)
                        .flat_map(|a| (a + 1..g).map(move |b| Some((a, b))))
                        .collect(),
                    false => vec![None],
                };
                let mut columns: Vec<String> = vec!["Test".into()];
                if per_stratum {
                    columns.push(shown.variable(strata_var.as_ref().expect("per stratum")));
                }
                if cmp.pairwise {
                    columns.push(format!("{} (first)", shown.variable(factor_v)));
                    columns.push(format!("{} (second)", shown.variable(factor_v)));
                }
                let lead = columns.len();
                columns.extend(["Chi-Square", "df", "Sig."].map(String::from));
                // the cases of each statistic, by stratum, with the index of their level
                let mut inputs = Vec::new();
                for (s, members) in &pools {
                    for pair in &pairs {
                        let mut groups = Vec::new();
                        for &m in members {
                            let mut cases = Vec::new();
                            for (l, obs) in strata_list[m].levels.iter().enumerate() {
                                let index = match pair {
                                    None => Some(l),
                                    Some((a, _)) if *a == l => Some(0),
                                    Some((_, b)) if *b == l => Some(1),
                                    Some(_) => None,
                                };
                                if let Some(index) = index {
                                    cases.extend(obs.iter().map(|o| (index, *o)));
                                }
                            }
                            groups.push(cases);
                        }
                        inputs.push((s, pair, groups));
                    }
                }
                let mut rows = Vec::new();
                for &test in &tests {
                    let mut first = true;
                    for (s, pair, groups) in &inputs {
                        let (size, weights) = match pair {
                            None => (g, trend.as_deref()),
                            Some(_) => (2, None),
                        };
                        let result = test_statistic(groups, size, test, weights);
                        let mut row = vec![Cell::Text(match first {
                            true => test_name(test).to_string(),
                            false => String::new(),
                        })];
                        first = false;
                        if let Some(s) = s {
                            row.push(Cell::Text(stratum_text(&strata_list[*s].key)));
                        }
                        if let Some((a, b)) = pair {
                            row.push(Cell::Text(level_text(&level_keys[*a])));
                            row.push(Cell::Text(level_text(&level_keys[*b])));
                        }
                        match result {
                            Some((chi, df)) => row.extend([
                                Cell::Number(chi),
                                Cell::Number(df as f64),
                                Cell::Number(chi_square_p(chi, df as f64)),
                            ]),
                            None => row.extend([Cell::Empty, Cell::Empty, Cell::Empty]),
                        }
                        rows.push(row);
                    }
                }
                let mut notes = Vec::new();
                notes.push(match (cmp.pairwise, &trend) {
                    (false, None) => format!(
                        "Tests whether the levels of {} share one survival distribution.",
                        shown.variable(factor_v)
                    ),
                    (false, Some(w)) => format!(
                        "Tests a linear trend over the levels of {} with the weights {}.",
                        shown.variable(factor_v),
                        w.iter()
                            .map(|x| crate::format::format_value(*x, "F8.2", false)
                                .trim()
                                .to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    (true, _) => "Tests each pair of levels.".into(),
                });
                if let (Some(v), false) = (&strata_var, per_stratum) {
                    notes.push(format!("Adjusted for {}.", shown.variable(v)));
                }
                tables.push(Table {
                    title: match cmp.pairwise {
                        true => "Pairwise Comparisons".into(),
                        false => "Overall Comparisons".into(),
                    },
                    row_headers: lead,
                    columns,
                    rows,
                    caption: Some(notes.join(" ")),
                    ..Table::default()
                });
            }
        }

        // plots: per stratum, the levels as lines
        for &kind in &command.plots {
            let (title, axis, f): (&str, &str, fn(f64) -> f64) = match kind {
                KmPlot::Survival => ("Survival Functions", "Cumulative Survival", |s| s),
                KmPlot::LogSurvival => ("Log Survival Functions", "Log Cumulative Survival", |s| {
                    s.ln()
                }),
                KmPlot::Hazard => ("Hazard Functions", "Cumulative Hazard", |s| 0.0 - s.ln()),
                KmPlot::OneMinusSurvival => (
                    "One Minus Survival Functions",
                    "One Minus Cumulative Survival",
                    |s| 1.0 - s,
                ),
            };
            for (s, stratum) in strata_list.iter().enumerate() {
                // every level, even one without cases here, so that it keeps its color
                let groups: Vec<PointGroup> = (0..g)
                    .map(|l| {
                        let Some((_, _, c)) = curve_at[s][l].map(|i| &curves[i]) else {
                            return PointGroup {
                                name: level_text(&level_keys[l]),
                                points: Vec::new(),
                            };
                        };
                        let mut censored: Vec<f64> = stratum.levels[l]
                            .iter()
                            .filter(|o| !o.event)
                            .map(|o| o.time)
                            .collect();
                        censored.dedup();
                        PointGroup {
                            name: match factor_var.is_some() {
                                true => level_text(&level_keys[l]),
                                false => shown.variable(&time_var),
                            },
                            points: c.steps(&censored, f),
                        }
                    })
                    .collect();
                let mut chart = Chart::plot(
                    ChartKind::Line,
                    title.into(),
                    Axis::titled(shown.variable(&time_var)),
                    Axis::titled(axis),
                    Plot::Points {
                        groups,
                        lines: Vec::new(),
                        z_axis: None,
                    },
                );
                if let Some(v) = &strata_var {
                    chart.subtitle = Some(format!(
                        "{} = {}",
                        shown.variable(v),
                        stratum_text(&stratum.key)
                    ));
                }
                if let Some(v) = &factor_var {
                    chart.legend = Some(shown.variable(v));
                }
                charts.push(chart);
            }
        }

        // SAVE
        if !command.save.is_empty() {
            let n = case_count;
            let mut saved: Vec<(KmSave, Vec<Option<f64>>)> = command
                .save
                .iter()
                .map(|(kind, _)| (*kind, vec![None; n]))
                .collect();
            for (s, l, c) in &curves {
                for o in &strata_list[*s].levels[*l] {
                    let (est, se) = c.at(o.time);
                    for (kind, values) in &mut saved {
                        values[o.case] = match kind {
                            KmSave::Survival => Some(est),
                            KmSave::Hazard => (est > 0.0).then(|| 0.0 - est.ln()),
                            KmSave::Se => Some(se),
                            KmSave::CumEvent => Some(c.events_by(o.time)),
                        };
                    }
                }
            }
            self.save_km(command, saved)?;
        }
        for w in warnings {
            self.warn(w);
        }
        self.charts.extend(charts);
        Ok(tables)
    }

    fn save_km(
        &mut self,
        command: &Km,
        saved: Vec<(KmSave, Vec<Option<f64>>)>,
    ) -> Result<(), Error> {
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            for ((kind, values), (_, name)) in saved.into_iter().zip(&command.save) {
                let (root, label, format) = match kind {
                    KmSave::Survival => ("SUR", "Survival function", "F8.4"),
                    KmSave::Hazard => ("HAZ", "Cumulative hazard function", "F8.4"),
                    KmSave::Se => ("SE", "Standard error of the survival function", "F8.4"),
                    KmSave::CumEvent => ("CUM", "Cumulative number of events", "F8.0"),
                };
                let name = match name {
                    Some(name) => name.clone(),
                    None => {
                        let mut k = 1;
                        while data.variable_index(&format!("{root}_{k}")).is_some() {
                            k += 1;
                        }
                        format!("{root}_{k}")
                    }
                };
                if data.variable_index(&name).is_some() {
                    return Err(Error(format!("KM /SAVE: variable {name} already exists")));
                }
                data.set_numeric_values(&name, values)?;
                data.set_format(&name, format)?;
                data.set_measure(&name, Measure::Scale)?;
                data.set_variable_label(&name, Some(label))?;
            }
            Ok(())
        })
    }
}

/// The ID of a case as the table shows it: the string, or the value label or number.
fn id_text(shown: &Shown, variable: &Variable, case: usize) -> String {
    let key = match variable.values() {
        Values::Numeric(v) => v.get(case).map(Key::Number),
        Values::String(v) => Some(Key::Text(v.get(case).trim_end().to_string())),
    };
    key.map_or_else(|| ".".into(), |k| shown.value(variable, &k))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(time: f64, event: bool) -> Obs {
        Obs {
            time,
            event,
            lost: false,
            weight: 1.0,
            case: 0,
        }
    }

    #[test]
    fn a_weight_counts_as_repeated_cases() {
        let mut heavy = vec![obs(2.0, true), obs(3.0, false), obs(5.0, true)];
        heavy[0].weight = 3.0;
        let repeated = vec![
            obs(2.0, true),
            obs(2.0, true),
            obs(2.0, true),
            obs(3.0, false),
            obs(5.0, true),
        ];
        assert_eq!(curve(&heavy), curve(&repeated));
        // fractional weights that all have the event end the curve at exactly 0
        let tenths: Vec<Obs> = (0..3)
            .map(|k| Obs {
                weight: 0.1,
                ..obs(1.0 + k as f64, true)
            })
            .collect();
        assert_eq!(*curve(&tenths).surv.last().unwrap(), 0.0);
        assert_eq!(default_trend(4), [-3.0, -1.0, 1.0, 3.0]);
        assert_eq!(default_trend(3), [-1.0, 0.0, 1.0]);
        assert_eq!(percent_label(25.0), "25.0%");
        assert_eq!(percent_label(2.5), "2.5%");
    }

    #[test]
    fn the_step_curve_has_only_flat_and_upright_segments() {
        let c = curve(&[
            obs(1.0, true),
            obs(2.0, false),
            obs(3.0, true),
            obs(4.0, false),
        ]);
        let points = c.steps(&[2.0, 4.0], |s| s);
        for w in points.windows(2) {
            assert!(w[0].x == w[1].x || w[0].y == w[1].y, "{w:?}");
        }
        let marks: Vec<f64> = points
            .iter()
            .filter(|p| p.label.is_some())
            .map(|p| p.x)
            .collect();
        assert_eq!(marks, [2.0, 4.0]);
        assert_eq!((points[0].x, points[0].y), (0.0, 1.0));
    }
}
