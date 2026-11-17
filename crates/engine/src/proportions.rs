//! PROPORTIONS: tests and confidence intervals for one binomial proportion, for the difference
//! of two related proportions and for the difference of two independent proportions.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - A case is a success when its value is the success value (LAST, FIRST, LEVEL) or at or above
//!   the middle of the observed range (MIDPOINT) or the cut point; every other valid value is a
//!   failure. LAST, FIRST and MIDPOINT look at the values of the cases the analysis uses.
//! - Weights are frequencies: counts are sums of weights. The exact and mid-p probabilities use
//!   the counts rounded to whole numbers.
//! - Z tests: the difference (shrunk towards 0 by the continuity correction, to 0 when the
//!   correction is larger) over a standard error; the two-sided p value is twice the smaller
//!   normal tail. Exact tests double the smaller binomial tail; mid-p takes half the point
//!   probability off that tail first. Two-sided p values are capped at 1.
//! - One sample: score tests use the standard error under the test value, Wald tests the one
//!   at the observed proportion. Paired: McNemar uses sqrt(n10 + n01), Wald also subtracts
//!   (n10 - n01)^2 / n. Independent: WALDH0 uses the pooled proportion, WALD the separate ones,
//!   Hauck-Anderson divides by n - 1 and corrects by 1 / (2 min(n1, n2)).
//! - Interval bounds outside 0 to 1 (differences: -1 to 1) are cut to that range.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::dist::{beta_i, normal_cdf, normal_quantile, solve};
use crate::npar_tests::{binomial_cdf, ln_choose, table, text};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Variable;
use statscraft_syntax::{
    Proportions, ProportionsAnalysis, ProportionsGroups, ProportionsSuccess, TTestPairs, Value,
};

/// The two-sided p value of a z statistic.
fn z_p(z: f64) -> f64 {
    (2.0 * normal_cdf(-z.abs())).min(1.0)
}

/// `d` moved towards 0 by the continuity correction `c`; 0 when `c` is larger than |d|.
fn shrink(d: f64, c: f64) -> f64 {
    d.signum() * (d.abs() - c).max(0.0)
}

fn beta_quantile(p: f64, a: f64, b: f64) -> f64 {
    solve(|x| beta_i(a, b, x) - p, 0.0, 1.0)
}

/// P(X >= k) for X binomial (n, p).
fn binomial_upper(k: f64, n: f64, p: f64) -> f64 {
    if k <= 0.0 {
        1.0
    } else {
        beta_i(k, n - k + 1.0, p)
    }
}

fn binomial_pmf(k: f64, n: f64, p: f64) -> f64 {
    if p <= 0.0 || p >= 1.0 {
        return f64::from(k == if p <= 0.0 { 0.0 } else { n });
    }
    (ln_choose(n, k) + k * p.ln() + (n - k) * (1.0 - p).ln()).exp()
}

/// The exact and mid-p two-sided p values of r successes in n trials under p0.
fn exact_p(r: f64, n: f64, p0: f64) -> (f64, f64) {
    let (r, n) = (r.round(), n.round());
    let tail = binomial_cdf(r, n, p0).min(binomial_upper(r, n, p0));
    let mid = tail - 0.5 * binomial_pmf(r, n, p0);
    ((2.0 * tail).min(1.0), (2.0 * mid).min(1.0))
}

fn logistic(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// The Wilson score interval of r successes in n trials.
fn wilson(r: f64, n: f64, z: f64) -> (f64, f64) {
    let p = r / n;
    let z2 = z * z;
    let centre = p + z2 / (2.0 * n);
    let half = z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt();
    let scale = 1.0 + z2 / n;
    ((centre - half) / scale, (centre + half) / scale)
}

/// The continuity-corrected Wilson interval (Newcombe's form, each bound with its own sign
/// terms; 0 and 1 at the ends of the range).
fn wilson_cc(r: f64, n: f64, z: f64) -> (f64, f64) {
    let (p, z2) = (r / n, z * z);
    let q = 1.0 - p;
    let bottom = 2.0 * (n + z2);
    let low = if r <= 0.0 {
        0.0
    } else {
        let root = (z2 - 2.0 - 1.0 / n + 4.0 * p * (n * q + 1.0))
            .max(0.0)
            .sqrt();
        (2.0 * n * p + z2 - 1.0 - z * root) / bottom
    };
    let high = if r >= n {
        1.0
    } else {
        let root = (z2 + 2.0 - 1.0 / n + 4.0 * p * (n * q - 1.0))
            .max(0.0)
            .sqrt();
        (2.0 * n * p + z2 + 1.0 + z * root) / bottom
    };
    (low, high)
}

/// A one-sample interval by its CITYPES keyword; None where the method is not defined.
fn one_interval(method: &str, r: f64, n: f64, z: f64, alpha: f64) -> Option<(f64, f64)> {
    let p = r / n;
    let se = (p * (1.0 - p) / n).sqrt();
    let around = |centre: f64, half: f64| (centre - half, centre + half);
    let bounds = match method {
        "AGRESTI_COULL" => {
            let n2 = n + z * z;
            let p2 = (r + z * z / 2.0) / n2;
            around(p2, z * (p2 * (1.0 - p2) / n2).sqrt())
        }
        "ANSCOMBE" => {
            let centre = ((r + 0.5) / (n - r + 0.5)).ln();
            let half = z * ((n + 1.0) * (n + 2.0) / (n * (r + 1.0) * (n - r + 1.0))).sqrt();
            (logistic(centre - half), logistic(centre + half))
        }
        "CLOPPER_PEARSON" => (
            if r <= 0.0 {
                0.0
            } else {
                beta_quantile(alpha / 2.0, r, n - r + 1.0)
            },
            if r >= n {
                1.0
            } else {
                beta_quantile(1.0 - alpha / 2.0, r + 1.0, n - r)
            },
        ),
        "JEFFREYS" => (
            beta_quantile(alpha / 2.0, r + 0.5, n - r + 0.5),
            beta_quantile(1.0 - alpha / 2.0, r + 0.5, n - r + 0.5),
        ),
        "LOGIT" => {
            if r <= 0.0 || r >= n {
                return None;
            }
            let centre = (r / (n - r)).ln();
            let half = z * (n / (r * (n - r))).sqrt();
            (logistic(centre - half), logistic(centre + half))
        }
        "WALD" => around(p, z * se),
        "WALDCC" => around(p, z * se + 0.5 / n),
        "WILSON_SCORE" => wilson(r, n, z),
        _ => wilson_cc(r, n, z),
    };
    Some(bounds)
}

/// One test row: the method's label, the standard error it uses, z and the two-sided p.
type TestRow = (&'static str, Option<f64>, Option<f64>, f64);

fn label(table: &[(&'static str, &'static str)], keyword: &str) -> &'static str {
    table
        .iter()
        .find(|(k, _)| *k == keyword)
        .map_or("", |(_, l)| *l)
}

const ONE_TEST_LABELS: [(&str, &str); 6] = [
    ("EXACT", "Exact Binomial"),
    ("MIDP", "Mid-p Adjusted Binomial"),
    ("SCORE", "Score"),
    ("SCORECC", "Continuity-Corrected Score"),
    ("WALD", "Wald"),
    ("WALDCC", "Continuity-Corrected Wald"),
];
const ONE_INTERVAL_LABELS: [(&str, &str); 9] = [
    ("AGRESTI_COULL", "Agresti-Coull"),
    ("ANSCOMBE", "Anscombe"),
    ("CLOPPER_PEARSON", "Clopper-Pearson (Exact)"),
    ("JEFFREYS", "Jeffreys"),
    ("LOGIT", "Logit"),
    ("WALD", "Wald"),
    ("WALDCC", "Wald (Continuity Corrected)"),
    ("WILSON_SCORE", "Wilson Score"),
    ("WILSON_SCORECC", "Wilson Score (Continuity Corrected)"),
];
const PAIRED_TEST_LABELS: [(&str, &str); 6] = [
    ("EXACT", "Exact Binomial"),
    ("MIDP", "Mid-p Adjusted Binomial"),
    ("MCNEMAR", "McNemar"),
    ("MCNEMARCC", "Continuity-Corrected McNemar"),
    ("WALD", "Wald"),
    ("WALDCC", "Continuity-Corrected Wald"),
];
const PAIRED_INTERVAL_LABELS: [(&str, &str); 5] = [
    ("AGRESTI_MIN", "Agresti-Min"),
    ("BONETT_PRICE", "Bonett-Price"),
    ("NEWCOMBE", "Newcombe"),
    ("WALD", "Wald"),
    ("WALDCC", "Wald (Continuity Corrected)"),
];
const INDEPENDENT_TEST_LABELS: [(&str, &str); 5] = [
    ("HAUCK_ANDERSON", "Hauck-Anderson"),
    ("WALD", "Wald"),
    ("WALDCC", "Continuity-Corrected Wald"),
    ("WALDH0", "Wald H0"),
    ("WALDH0CC", "Continuity-Corrected Wald H0"),
];
const INDEPENDENT_INTERVAL_LABELS: [(&str, &str); 7] = [
    ("AGRESTI_CAFFO", "Agresti-Caffo"),
    ("BROWN_LI_JEFFREYS", "Brown-Li-Jeffreys"),
    ("HAUCK_ANDERSON", "Hauck-Anderson"),
    ("NEWCOMBE", "Newcombe"),
    ("NEWCOMBECC", "Newcombe (Continuity Corrected)"),
    ("WALD", "Wald"),
    ("WALDCC", "Wald (Continuity Corrected)"),
];

/// A z row; z is None where its standard error is 0.
fn z_row(label: &'static str, d: f64, se: f64) -> TestRow {
    let z = Some(d / se).filter(|z| z.is_finite());
    (label, Some(se), z, z.map_or(f64::NAN, z_p))
}

fn one_test(method: &str, r: f64, n: f64, p0: f64) -> TestRow {
    let p = r / n;
    let d = p - p0;
    let (se0, se1) = ((p0 * (1.0 - p0) / n).sqrt(), (p * (1.0 - p) / n).sqrt());
    let l = label(&ONE_TEST_LABELS, method);
    match method {
        "EXACT" => (l, None, None, exact_p(r, n, p0).0),
        "MIDP" => (l, None, None, exact_p(r, n, p0).1),
        "SCORE" => z_row(l, d, se0),
        "SCORECC" => z_row(l, shrink(d, 0.5 / n), se0),
        "WALD" => z_row(l, d, se1),
        _ => z_row(l, shrink(d, 0.5 / n), se1),
    }
}

/// The four cells of a pair: both successes, first only, second only, neither.
#[derive(Clone, Copy, Default, Debug)]
struct Pair {
    n11: f64,
    n10: f64,
    n01: f64,
    n00: f64,
}

impl Pair {
    fn n(&self) -> f64 {
        self.n11 + self.n10 + self.n01 + self.n00
    }

    /// The standard errors of the difference under H0 and at the observed cells.
    fn ses(&self) -> (f64, f64) {
        let (n, s, d) = (self.n(), self.n10 + self.n01, self.n10 - self.n01);
        (s.sqrt() / n, (s - d * d / n).max(0.0).sqrt() / n)
    }

    fn test(&self, method: &str) -> TestRow {
        let n = self.n();
        let (se0, se1) = self.ses();
        let d = (self.n10 - self.n01) / n;
        let l = label(&PAIRED_TEST_LABELS, method);
        let exact = || {
            let (a, b) = (self.n10.round(), self.n01.round());
            exact_p(a.min(b), a + b, 0.5)
        };
        match method {
            "EXACT" => (l, None, None, exact().0),
            "MIDP" => (l, None, None, exact().1),
            "MCNEMAR" => z_row(l, d, se0),
            "MCNEMARCC" => z_row(l, shrink(d, 1.0 / n), se0),
            "WALD" => z_row(l, d, se1),
            _ => z_row(l, shrink(d, 1.0 / n), se1),
        }
    }

    fn interval(&self, method: &str, z: f64) -> (f64, f64) {
        let n = self.n();
        let d = (self.n10 - self.n01) / n;
        // Agresti-Min adds 0.5 to every cell, Bonett-Price 1 to each discordant cell
        let adjusted = |add: f64| {
            let m = n + 2.0;
            let centre = (self.n10 - self.n01) / m;
            let inner = (self.n10 + add) / m + (self.n01 + add) / m - centre * centre;
            let half = z * (inner / m).max(0.0).sqrt();
            (centre - half, centre + half)
        };
        let se1 = self.ses().1;
        match method {
            "AGRESTI_MIN" => adjusted(0.5),
            "BONETT_PRICE" => adjusted(1.0),
            "WALD" => (d - z * se1, d + z * se1),
            "WALDCC" => (d - z * se1 - 1.0 / n, d + z * se1 + 1.0 / n),
            _ => {
                let (first, second) = (self.n11 + self.n10, self.n11 + self.n01);
                let (p1, p2) = (first / n, second / n);
                let (l1, u1) = wilson(first, n, z);
                let (l2, u2) = wilson(second, n, z);
                let margins = first * (n - first) * second * (n - second);
                let a = self.n11 * self.n00 - self.n10 * self.n01;
                let psi = if margins <= 0.0 || (0.0..=n / 2.0).contains(&a) {
                    0.0
                } else if a > n / 2.0 {
                    (a - n / 2.0) / margins.sqrt()
                } else {
                    a / margins.sqrt()
                };
                let mover = |x: f64, y: f64| (x * x + y * y - 2.0 * psi * x * y).max(0.0).sqrt();
                (d - mover(p1 - l1, u2 - p2), d + mover(u1 - p1, p2 - l2))
            }
        }
    }
}

/// Successes and trials of two independent groups.
#[derive(Clone, Copy, Default, Debug)]
struct Groups {
    r: [f64; 2],
    n: [f64; 2],
}

impl Groups {
    fn p(&self, g: usize) -> f64 {
        self.r[g] / self.n[g]
    }

    /// The pooled and the separate standard errors of the difference.
    fn ses(&self) -> (f64, f64) {
        let pooled = (self.r[0] + self.r[1]) / (self.n[0] + self.n[1]);
        let inverse = 1.0 / self.n[0] + 1.0 / self.n[1];
        let separate = (0..2).map(|g| self.p(g) * (1.0 - self.p(g)) / self.n[g]);
        (
            (pooled * (1.0 - pooled) * inverse).sqrt(),
            separate.sum::<f64>().sqrt(),
        )
    }

    /// Hauck-Anderson: its standard error and continuity correction.
    fn hauck_anderson(&self) -> (f64, f64) {
        let v: f64 = (0..2)
            .map(|g| self.p(g) * (1.0 - self.p(g)) / (self.n[g] - 1.0))
            .sum();
        (v.sqrt(), 0.5 / self.n[0].min(self.n[1]))
    }

    fn test(&self, method: &str) -> TestRow {
        let d = self.p(0) - self.p(1);
        let (se0, se1) = self.ses();
        let cc = (1.0 / self.n[0] + 1.0 / self.n[1]) / 2.0;
        let l = label(&INDEPENDENT_TEST_LABELS, method);
        match method {
            "HAUCK_ANDERSON" => {
                let (se, c) = self.hauck_anderson();
                z_row(l, shrink(d, c), se)
            }
            "WALD" => z_row(l, d, se1),
            "WALDCC" => z_row(l, shrink(d, cc), se1),
            "WALDH0" => z_row(l, d, se0),
            _ => z_row(l, shrink(d, cc), se0),
        }
    }

    fn interval(&self, method: &str, z: f64) -> (f64, f64) {
        let d = self.p(0) - self.p(1);
        let se1 = self.ses().1;
        let cc = (1.0 / self.n[0] + 1.0 / self.n[1]) / 2.0;
        // a Wald interval of proportions (r + add) / (n + 2 add), the variance over n + extra
        let shifted = |add: f64, extra: f64| {
            let q = |g: usize| (self.r[g] + add) / (self.n[g] + 2.0 * add);
            let v: f64 = (0..2)
                .map(|g| q(g) * (1.0 - q(g)) / (self.n[g] + extra))
                .sum();
            let centre = q(0) - q(1);
            (centre - z * v.sqrt(), centre + z * v.sqrt())
        };
        let mover = |corrected: bool| {
            let bounds = |g: usize| match corrected {
                true => wilson_cc(self.r[g], self.n[g], z),
                false => wilson(self.r[g], self.n[g], z),
            };
            let ((l1, u1), (l2, u2)) = (bounds(0), bounds(1));
            let (p1, p2) = (self.p(0), self.p(1));
            (
                d - ((p1 - l1).powi(2) + (u2 - p2).powi(2)).sqrt(),
                d + ((u1 - p1).powi(2) + (p2 - l2).powi(2)).sqrt(),
            )
        };
        match method {
            "AGRESTI_CAFFO" => shifted(1.0, 2.0),
            "BROWN_LI_JEFFREYS" => shifted(0.5, 0.0),
            "HAUCK_ANDERSON" => {
                let (se, c) = self.hauck_anderson();
                (d - c - z * se, d + c + z * se)
            }
            "NEWCOMBE" => mover(false),
            "NEWCOMBECC" => mover(true),
            "WALD" => (d - z * se1, d + z * se1),
            _ => (d - cc - z * se1, d + cc + z * se1),
        }
    }
}

/// How the success of a valid value is decided for one variable.
enum Rule {
    Values(Vec<Key>),
    AtLeast(f64),
}

fn same(a: &Key, b: &Key) -> bool {
    match (a, b) {
        (Key::Number(x), Key::Number(y)) => x == y,
        (Key::Text(x), Key::Text(y)) => x.trim_end() == y.trim_end(),
        _ => false,
    }
}

impl Rule {
    fn new<'k>(success: &ProportionsSuccess, seen: impl Iterator<Item = &'k Key> + Clone) -> Rule {
        let numbers = seen.clone().filter_map(|k| match k {
            Key::Number(x) => Some(*x),
            Key::Text(_) => None,
        });
        let extreme = |pick_last: bool| {
            let best = seen.clone().reduce(|a, b| {
                let later = b.compare(a).is_gt();
                if later == pick_last { b } else { a }
            });
            Rule::Values(best.into_iter().cloned().collect())
        };
        match success {
            ProportionsSuccess::Last => extreme(true),
            ProportionsSuccess::First => extreme(false),
            ProportionsSuccess::Level(values) => Rule::Values(
                values
                    .iter()
                    .map(|v| match v {
                        Value::Number(x) => Key::Number(*x),
                        Value::String(s) => Key::Text(s.clone()),
                    })
                    .collect(),
            ),
            ProportionsSuccess::Midpoint => {
                let low = numbers.clone().fold(f64::INFINITY, f64::min);
                let high = numbers.fold(f64::NEG_INFINITY, f64::max);
                Rule::AtLeast((low + high) / 2.0)
            }
            ProportionsSuccess::Cutpoint(c) => Rule::AtLeast(*c),
        }
    }

    fn success(&self, key: &Key) -> bool {
        match (self, key) {
            (Rule::Values(values), key) => values.iter().any(|v| same(v, key)),
            (Rule::AtLeast(c), Key::Number(x)) => x >= c,
            _ => false,
        }
    }
}

/// Column titles of the interval bounds and their group.
fn interval_columns(columns: &mut Vec<String>, groups: &mut Vec<Vec<String>>, level: f64) {
    let group = format!("{level}% Confidence Interval");
    for side in ["Lower", "Upper"] {
        columns.push(side.into());
        groups.push(vec![group.clone()]);
    }
}

/// A table whose rows start with an item and a method; the item shows only on its first row.
fn method_table(
    title: &str,
    heads: [&str; 2],
    columns: &[&str],
    level: Option<f64>,
    rows: Vec<Vec<Cell>>,
) -> Table {
    let mut names: Vec<String> = heads.iter().chain(columns).map(|s| s.to_string()).collect();
    let mut groups = vec![Vec::new(); names.len()];
    if let Some(level) = level {
        interval_columns(&mut names, &mut groups, level);
    }
    let mut t = table(title, 2, names, rows);
    t.column_groups = groups;
    t
}

/// The first cell of an item's rows: its name on the first row only.
fn lead(name: &str, first: bool) -> Cell {
    if first { text(name) } else { Cell::Empty }
}

fn clamp(bounds: (f64, f64), low: f64) -> (Cell, Cell) {
    let c = |x: f64| number(x.is_finite().then_some(x.clamp(low, 1.0)));
    (c(bounds.0), c(bounds.1))
}

impl Engine {
    pub(crate) fn proportions(&mut self, command: &Proportions) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let weights = case_weights(data)?;
        let weight = |case: usize| weights.as_ref().map_or(1.0, |w| w[case]);
        let shown = self.shown();
        let alpha = 1.0 - command.level / 100.0;
        let z = normal_quantile(1.0 - alpha / 2.0);
        let level = command.level;
        let mut warnings = Vec::new();
        let variable = |name: &str| data.variable(name);
        // every variable of the analysis, for the type checks and LISTWISE
        let (lists, group): (Vec<String>, Option<Variable>) = match &command.analysis {
            ProportionsAnalysis::OneSample { variables, .. }
            | ProportionsAnalysis::Independent { variables, .. } => {
                let group = match &command.analysis {
                    ProportionsAnalysis::Independent { group, .. } => Some(variable(group)?),
                    _ => None,
                };
                (self.expand(variables)?, group)
            }
            ProportionsAnalysis::Paired(pairs) => {
                let mut all = self.expand(&pairs.left)?;
                all.extend(self.expand(pairs.right.as_deref().unwrap_or_default())?);
                (all, None)
            }
        };
        let numeric_only = matches!(
            command.success,
            ProportionsSuccess::Midpoint | ProportionsSuccess::Cutpoint(_)
        );
        let mut keys: Vec<(String, Vec<Option<Key>>)> = Vec::new();
        for name in &lists {
            let v = variable(name)?;
            if numeric_only && v.is_string() {
                return Err(Error(format!(
                    "SUCCESSVALUE MIDPOINT and CUTPOINT need numeric variables, but {} is a string \
                     variable",
                    v.name()
                )));
            }
            if let ProportionsSuccess::Level(values) = &command.success
                && values
                    .iter()
                    .any(|x| matches!(x, Value::String(_)) != v.is_string())
            {
                return Err(Error(format!(
                    "the SUCCESSVALUE LEVEL values must be {} for {}",
                    if v.is_string() {
                        "quoted strings"
                    } else {
                        "numbers"
                    },
                    v.name()
                )));
            }
            keys.push((v.name().to_string(), categories(&v, command.include)));
        }
        let group_keys = group.as_ref().map(|g| categories(g, command.include));
        let key_of = |name: &str| {
            &keys
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(name))
                .expect("expanded above")
                .1
        };
        let cases = data.case_count();
        let complete: Vec<bool> = (0..cases)
            .map(|c| {
                !command.listwise
                    || (keys.iter().all(|(_, k)| k[c].is_some())
                        && group_keys.as_ref().is_none_or(|g| g[c].is_some()))
            })
            .collect();
        let base: Vec<usize> = (0..cases)
            .filter(|&c| weight(c) > 0.0 && complete[c])
            .collect();
        let mut tables = Vec::new();
        match &command.analysis {
            ProportionsAnalysis::OneSample { test_value, .. } => {
                let (mut tests, mut intervals) = (Vec::new(), Vec::new());
                let mut seen = Vec::new();
                for name in &lists {
                    if seen.iter().any(|s: &String| s.eq_ignore_ascii_case(name)) {
                        continue;
                    }
                    seen.push(name.clone());
                    let k = key_of(name);
                    let used: Vec<usize> =
                        base.iter().copied().filter(|&c| k[c].is_some()).collect();
                    let rule =
                        Rule::new(&command.success, used.iter().filter_map(|&c| k[c].as_ref()));
                    let (mut r, mut n) = (0.0, 0.0);
                    for &c in &used {
                        n += weight(c);
                        if rule.success(k[c].as_ref().expect("used")) {
                            r += weight(c);
                        }
                    }
                    if n <= 0.0 {
                        warnings.push(format!("PROPORTIONS: {name} has no valid cases"));
                        continue;
                    }
                    let v = variable(name)?;
                    let shown_name = shown.variable(&v);
                    let p = r / n;
                    for (i, method) in command.tests.iter().enumerate() {
                        let (l, se, zv, sig) = one_test(method, r, n, *test_value);
                        tests.push(vec![
                            lead(&shown_name, i == 0),
                            text(l),
                            Cell::Number(r),
                            Cell::Number(n),
                            Cell::Number(p),
                            Cell::Number(p - test_value),
                            number(se),
                            number(zv),
                            number(Some(sig)),
                        ]);
                    }
                    for (i, method) in command.intervals.iter().enumerate() {
                        let bounds = one_interval(method, r, n, z, alpha);
                        let (low, high) = clamp(bounds.unwrap_or((f64::NAN, f64::NAN)), 0.0);
                        intervals.push(vec![
                            lead(&shown_name, i == 0),
                            text(label(&ONE_INTERVAL_LABELS, method)),
                            Cell::Number(r),
                            Cell::Number(n),
                            Cell::Number(p),
                            number(Some((p * (1.0 - p) / n).sqrt())),
                            low,
                            high,
                        ]);
                    }
                }
                if !tests.is_empty() {
                    let mut t = method_table(
                        "One-Sample Proportions Tests",
                        ["Variable", "Test Type"],
                        &[
                            "Successes",
                            "Trials",
                            "Observed Proportion",
                            "Observed - Test Value",
                            "Asymptotic Standard Error",
                            "Z",
                            "Significance (Two-Sided p)",
                        ],
                        None,
                        tests,
                    );
                    t.caption = Some(format!("Test value = {test_value}."));
                    tables.push(t);
                }
                if !intervals.is_empty() {
                    tables.push(method_table(
                        "One-Sample Proportions Confidence Intervals",
                        ["Variable", "Interval Type"],
                        &[
                            "Successes",
                            "Trials",
                            "Observed Proportion",
                            "Asymptotic Standard Error",
                        ],
                        Some(level),
                        intervals,
                    ));
                }
            }
            ProportionsAnalysis::Paired(pairs) => {
                let pairs = self.pairs(pairs, &mut warnings)?;
                let (mut statistics, mut tests, mut intervals) =
                    (Vec::new(), Vec::new(), Vec::new());
                for (a, b) in &pairs {
                    let (ka, kb) = (key_of(a), key_of(b));
                    let used: Vec<usize> = base
                        .iter()
                        .copied()
                        .filter(|&c| ka[c].is_some() && kb[c].is_some())
                        .collect();
                    let rule = |k: &Vec<Option<Key>>| {
                        Rule::new(&command.success, used.iter().filter_map(|&c| k[c].as_ref()))
                    };
                    let (ra, rb) = (rule(ka), rule(kb));
                    let mut cells = Pair::default();
                    for &c in &used {
                        let w = weight(c);
                        let first = ra.success(ka[c].as_ref().expect("used"));
                        let second = rb.success(kb[c].as_ref().expect("used"));
                        *match (first, second) {
                            (true, true) => &mut cells.n11,
                            (true, false) => &mut cells.n10,
                            (false, true) => &mut cells.n01,
                            (false, false) => &mut cells.n00,
                        } += w;
                    }
                    let n = cells.n();
                    let (va, vb) = (variable(a)?, variable(b)?);
                    let name = format!("{} - {}", shown.variable(&va), shown.variable(&vb));
                    if n <= 0.0 {
                        warnings.push(format!(
                            "PROPORTIONS: the pair {a} - {b} has no valid cases"
                        ));
                        continue;
                    }
                    for (i, (v, r)) in [(&va, cells.n11 + cells.n10), (&vb, cells.n11 + cells.n01)]
                        .into_iter()
                        .enumerate()
                    {
                        let p = r / n;
                        statistics.push(vec![
                            lead(&name, i == 0),
                            text(shown.variable(v)),
                            Cell::Number(r),
                            Cell::Number(n),
                            Cell::Number(p),
                            number(Some((p * (1.0 - p) / n).sqrt())),
                        ]);
                    }
                    let d = (cells.n10 - cells.n01) / n;
                    for (i, method) in command.tests.iter().enumerate() {
                        let (l, se, zv, sig) = cells.test(method);
                        tests.push(vec![
                            lead(&name, i == 0),
                            text(l),
                            Cell::Number(d),
                            number(se),
                            number(zv),
                            number(Some(sig)),
                        ]);
                    }
                    for (i, method) in command.intervals.iter().enumerate() {
                        let (low, high) = clamp(cells.interval(method, z), -1.0);
                        intervals.push(vec![
                            lead(&name, i == 0),
                            text(label(&PAIRED_INTERVAL_LABELS, method)),
                            Cell::Number(d),
                            number(Some(cells.ses().1)),
                            low,
                            high,
                        ]);
                    }
                }
                push_difference_tables(
                    &mut tables,
                    "Paired-Samples",
                    ["Pair", "Variable"],
                    level,
                    statistics,
                    tests,
                    intervals,
                );
            }
            ProportionsAnalysis::Independent { select, .. } => {
                let g = group.as_ref().expect("independent");
                let gk = group_keys.as_ref().expect("independent");
                let numeric = !g.is_string();
                let (membership, labels): (Vec<Option<usize>>, [String; 2]) = match select {
                    ProportionsGroups::Levels(a, b) => {
                        let key = |v: &Value| match v {
                            Value::Number(x) => Key::Number(*x),
                            Value::String(s) => Key::Text(s.clone()),
                        };
                        if [a, b]
                            .iter()
                            .any(|v| matches!(v, Value::Number(_)) != numeric)
                        {
                            // without SELECT the default levels 1 and 2 meet a string variable
                            return Err(Error(format!(
                                "the SELECT values of {} must be {}",
                                g.name(),
                                if numeric {
                                    "numbers"
                                } else {
                                    "quoted strings, as in SELECT=LEVEL('a', 'b')"
                                }
                            )));
                        }
                        let (ka, kb) = (key(a), key(b));
                        let membership = gk
                            .iter()
                            .map(|k| {
                                let k = k.as_ref()?;
                                if same(k, &ka) {
                                    Some(0)
                                } else if same(k, &kb) {
                                    Some(1)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        (membership, [shown.value(g, &ka), shown.value(g, &kb)])
                    }
                    ProportionsGroups::Midpoint | ProportionsGroups::Cutpoint(_) => {
                        if !numeric {
                            return Err(Error(format!(
                                "SELECT=MIDPOINT and CUTPOINT need a numeric grouping variable, but \
                                 {} is a string variable",
                                g.name()
                            )));
                        }
                        let at = |c: usize| match gk[c] {
                            Some(Key::Number(x)) => Some(x),
                            _ => None,
                        };
                        let cut = match select {
                            ProportionsGroups::Cutpoint(c) => *c,
                            _ => {
                                let values = base.iter().filter_map(|&c| at(c));
                                let low = values.clone().fold(f64::INFINITY, f64::min);
                                (low + values.fold(f64::NEG_INFINITY, f64::max)) / 2.0
                            }
                        };
                        let membership = (0..cases)
                            .map(|c| at(c).map(|x| usize::from(x >= cut)))
                            .collect();
                        // the cut as written: the variable's format may round it away
                        (membership, [format!("< {cut}"), format!(">= {cut}")])
                    }
                };
                let group_name = shown.variable(g);
                let (mut statistics, mut tests, mut intervals) =
                    (Vec::new(), Vec::new(), Vec::new());
                let mut seen = Vec::new();
                for name in &lists {
                    if seen.iter().any(|s: &String| s.eq_ignore_ascii_case(name)) {
                        continue;
                    }
                    seen.push(name.clone());
                    let k = key_of(name);
                    let used: Vec<usize> = base
                        .iter()
                        .copied()
                        .filter(|&c| k[c].is_some() && membership[c].is_some())
                        .collect();
                    let rule =
                        Rule::new(&command.success, used.iter().filter_map(|&c| k[c].as_ref()));
                    let mut counts = Groups::default();
                    for &c in &used {
                        let i = membership[c].expect("used");
                        counts.n[i] += weight(c);
                        if rule.success(k[c].as_ref().expect("used")) {
                            counts.r[i] += weight(c);
                        }
                    }
                    if counts.n.iter().any(|n| *n <= 0.0) {
                        warnings.push(format!(
                            "PROPORTIONS: {name} is not compared, since a group of {} has no valid cases",
                            g.name()
                        ));
                        continue;
                    }
                    let v = variable(name)?;
                    let shown_name = shown.variable(&v);
                    for (i, group_label) in labels.iter().enumerate() {
                        let p = counts.p(i);
                        statistics.push(vec![
                            lead(&shown_name, i == 0),
                            text(format!("{group_name} = {group_label}")),
                            Cell::Number(counts.r[i]),
                            Cell::Number(counts.n[i]),
                            Cell::Number(p),
                            number(Some((p * (1.0 - p) / counts.n[i]).sqrt())),
                        ]);
                    }
                    let d = counts.p(0) - counts.p(1);
                    for (i, method) in command.tests.iter().enumerate() {
                        let (l, se, zv, sig) = counts.test(method);
                        tests.push(vec![
                            lead(&shown_name, i == 0),
                            text(l),
                            Cell::Number(d),
                            number(se),
                            number(zv),
                            number(Some(sig)),
                        ]);
                    }
                    for (i, method) in command.intervals.iter().enumerate() {
                        let (low, high) = clamp(counts.interval(method, z), -1.0);
                        intervals.push(vec![
                            lead(&shown_name, i == 0),
                            text(label(&INDEPENDENT_INTERVAL_LABELS, method)),
                            Cell::Number(d),
                            number(Some(counts.ses().1)),
                            low,
                            high,
                        ]);
                    }
                }
                push_difference_tables(
                    &mut tables,
                    "Independent-Samples",
                    ["Variable", "Group"],
                    level,
                    statistics,
                    tests,
                    intervals,
                );
            }
        }
        for w in warnings {
            self.warn(w);
        }
        Ok(tables)
    }

    /// The pairs of PAIREDSAMPLES; (PAIRED) with lists of different lengths warns and leaves
    /// the unmatched variables out.
    fn pairs(
        &self,
        pairs: &TTestPairs,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<(String, String)>, Error> {
        let left = self.expand(&pairs.left)?;
        let mut out = Vec::new();
        match &pairs.right {
            None => {
                for (i, a) in left.iter().enumerate() {
                    out.extend(left[i + 1..].iter().map(|b| (a.clone(), b.clone())));
                }
            }
            Some(right) => {
                let right = self.expand(right)?;
                if pairs.paired {
                    if left.len() != right.len() {
                        warnings.push(format!(
                            "(PAIRED) has {} variables before WITH and {} after; the unmatched ones are left out",
                            left.len(),
                            right.len()
                        ));
                    }
                    out.extend(left.into_iter().zip(right));
                } else {
                    for a in &left {
                        out.extend(right.iter().map(|b| (a.clone(), b.clone())));
                    }
                }
            }
        }
        out.retain(|(a, b)| !a.eq_ignore_ascii_case(b));
        Ok(out)
    }
}

/// The statistics, tests and intervals tables of a paired or independent analysis.
fn push_difference_tables(
    tables: &mut Vec<Table>,
    kind: &str,
    heads: [&str; 2],
    level: f64,
    statistics: Vec<Vec<Cell>>,
    tests: Vec<Vec<Cell>>,
    intervals: Vec<Vec<Cell>>,
) {
    if statistics.is_empty() {
        return;
    }
    tables.push(method_table(
        &format!("{kind} Proportions Statistics"),
        heads,
        &[
            "Successes",
            "Trials",
            "Proportion",
            "Asymptotic Standard Error",
        ],
        None,
        statistics,
    ));
    let item = [heads[0], "Test Type"];
    let columns = ["Difference in Proportions", "Asymptotic Standard Error"];
    if !tests.is_empty() {
        tables.push(method_table(
            &format!("{kind} Proportions Tests"),
            item,
            &[columns[0], columns[1], "Z", "Significance (Two-Sided p)"],
            None,
            tests,
        ));
    }
    if !intervals.is_empty() {
        tables.push(method_table(
            &format!("{kind} Proportions Confidence Intervals"),
            [heads[0], "Interval Type"],
            &columns,
            Some(level),
            intervals,
        ));
    }
}
