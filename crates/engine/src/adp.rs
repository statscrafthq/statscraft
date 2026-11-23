//! ADP: automated preparation of a target and its inputs for modelling. The command reads the
//! data, decides each step and writes syntax (RECODE, COMPUTE, IF, VARIABLE LEVEL) that makes
//! the prepared fields; the active dataset is left as it was.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in the order they run:
//! - Cases count with their frequency weight rounded (WEIGHT) and need a positive, valid
//!   analysis weight (ANALYSISWEIGHT); means and sums of squares weigh by both, counts by the
//!   frequency weight alone. Standard deviations divide by the count minus one.
//! - Date inputs give the time since a reference date in days, months (30.4375 days) or years
//!   (365.25 days), AUTO taking days when the least elapsed time is under 31 days, months when
//!   under 366, else years; time inputs give seconds, minutes (from 60 s) or hours (from 3600 s).
//!   Year, month, day, hour, minute and second parts are ordinal. The date or time field itself
//!   is not kept.
//! - Screening: a field with more than MAXPCT missing values, with no valid value or a single
//!   value; a categorical field whose largest category holds more than SINGLECAT's MAXPCT of the
//!   valid values; a nominal field with more than MAXCAT categories. A screened target, or no
//!   input left, stops the command.
//! - Levels: a continuous field with fewer than MINVALCONTINUOUS values becomes ordinal, a numeric
//!   ordinal field with more than MAXVALORDINAL categories continuous.
//! - Outliers of a continuous field: from the mean and SD, eight intervals one SD wide (open at
//!   both ends beyond 3 SD) are dropped from the thinner tail while the dropped share stays under
//!   5%; the mean and SD of the rest are robust. A value with sqrt(w) |x - m| above CUTOFF times
//!   the robust SD is an outlier, moved to the cutoff or made missing.
//! - Missing values: continuous fields take the mean, ordinal the median (the least value whose
//!   cumulative share reaches one half), nominal the mode (the least of tied values).
//! - Target: a nominal target can be renumbered from 0 by ascending count (ties by value); a
//!   continuous target goes through Box-Cox with lambda from -3 to 3 by 0.5 (the largest
//!   log-likelihood, ties to the smaller lambda, c = minimum - 1), then to the chosen mean and SD.
//! - Continuous inputs: z scores or min-max from the prepared mean, SD, minimum and maximum.
//! - Categorical inputs with a target: categories without a case of valid target set aside; the
//!   pair (adjacent for ordinal inputs) with the largest p is merged while that p exceeds PVALUE.
//!   The tests: F for a continuous target, Pearson chi-square on J - 1 df for a nominal one, the
//!   likelihood ratio of the row effects model against independence on 1 df for an ordinal one.
//!   Ordinal merges cut at the largest value of each merged category; set-aside nominal categories
//!   join the smallest merged one.
//! - Categorical inputs without a target (MERGEUNSUPERVISED): equal frequency merging of
//!   neighbouring categories (by value, or nominal by ascending count) until each holds at least
//!   the rounded MINPCT share of the cases, or two are left.
//! - Continuous inputs with a categorical target: Scheffe homogeneous subsets of the target
//!   classes' means give bins, cut between neighbouring subsets in proportion to their SDs.
//! - Continuous inputs with a continuous target: dropped when the t test of their correlation
//!   with the prepared target has p above PVALUE.

use crate::crosstabs::{Key, categories};
use crate::descriptives::case_weights;
use crate::dist::{chi_square_p, f_p, f_quantile, t_p};
use crate::expr_functions::{date, xdate};
use crate::npar_tests::text;
use crate::twostep::{free_name, plain_table};
use crate::variables::duplicate;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Measure, is_valid_name};
use statscraft_syntax::{Adp, AdpLevels, AdpRescale};

const DAY: f64 = 86400.0;
const DATES: [&str; 10] = [
    "DATE", "ADATE", "EDATE", "JDATE", "SDATE", "QYR", "MOYR", "WKYR", "DATETIME", "YMDHMS",
];
const TIMES: [&str; 3] = ["TIME", "DTIME", "MTIME"];

/// Weighted moments of (value, frequency, analysis weight) triples.
#[derive(Clone, Copy, Default)]
struct Moments {
    /// The sum of the frequency weights.
    n: f64,
    /// The sum of frequency times analysis weights.
    w: f64,
    mean: f64,
    m2: f64,
}

impl Moments {
    fn of(items: &[(f64, f64, f64)]) -> Moments {
        let n = items.iter().map(|t| t.1).sum();
        let w: f64 = items.iter().map(|t| t.1 * t.2).sum();
        let mean = items.iter().map(|t| t.0 * t.1 * t.2).sum::<f64>() / w;
        let m2 = items.iter().map(|t| t.1 * t.2 * (t.0 - mean).powi(2)).sum();
        Moments { n, w, mean, m2 }
    }

    fn sd(&self) -> f64 {
        (self.m2 / (self.n - 1.0)).sqrt()
    }

    fn join(&self, other: &Moments) -> Moments {
        let w = self.w + other.w;
        let mean = (self.w * self.mean + other.w * other.mean) / w;
        Moments {
            n: self.n + other.n,
            w,
            mean,
            m2: self.m2
                + other.m2
                + self.w * (mean - self.mean).powi(2)
                + other.w * (mean - other.mean).powi(2),
        }
    }
}

/// How a categorical field's values become the prepared ones.
#[derive(Clone)]
enum Recode {
    /// Each value seen to its new value; other values become system-missing.
    Map(Vec<(Key, Key)>),
    /// (LOWEST THRU upper_k = k) for k from 1, and values above the last upper the next number.
    Ranges(Vec<f64>),
}

impl Recode {
    fn apply(&self, key: &Key) -> Option<Key> {
        match self {
            Recode::Map(map) => map
                .iter()
                .find(|(from, _)| from.compare(key).is_eq())
                .map(|(_, to)| to.clone()),
            Recode::Ranges(uppers) => match key {
                Key::Number(x) => Some(Key::Number(
                    (uppers.iter().filter(|&&u| u < *x).count() + 1) as f64,
                )),
                Key::Text(_) => None,
            },
        }
    }
}

struct Field {
    /// The variable read, or the name of a derived field.
    name: String,
    /// The expression of a derived field and the field it comes from.
    derive: Option<(String, String)>,
    target: bool,
    /// The width of a string field.
    width: Option<usize>,
    level0: Measure,
    level: Measure,
    values: Vec<Option<Key>>,
    excluded: Option<String>,
    actions: Vec<String>,
    /// Prepared as a continuous field (by COMPUTE and IF), not by a single RECODE.
    continuous: bool,
    /// Statements on the prepared field of a continuous one, `{o}` standing for its name.
    statements: Vec<String>,
    /// The prepared value of missing values of a categorical field.
    missing_to: Option<Key>,
    recode: Option<Recode>,
    changed: bool,
    out: String,
    /// The constructed predictor that replaces this input.
    replaced_by: Option<String>,
}

impl Field {
    fn x(&self, case: usize) -> Option<f64> {
        match &self.values[case] {
            Some(Key::Number(x)) => Some(*x),
            _ => None,
        }
    }

    fn live(&self) -> bool {
        self.excluded.is_none()
    }

    fn exclude(&mut self, reason: String) {
        self.excluded = Some(reason);
    }

    /// The categories of the counted cases with their counts, by value.
    fn counts(&self, used: &[(usize, f64, f64)]) -> Vec<(Key, f64)> {
        let mut counts: Vec<(Key, f64)> = Vec::new();
        for &(c, f, _) in used {
            if let Some(key) = &self.values[c] {
                match counts.iter_mut().find(|(k, _)| k.compare(key).is_eq()) {
                    Some(entry) => entry.1 += f,
                    None => counts.push((key.clone(), f)),
                }
            }
        }
        counts.sort_by(|a, b| a.0.compare(&b.0));
        counts
    }

    /// (value, f, w) of the counted cases with a numeric value.
    fn items(&self, used: &[(usize, f64, f64)]) -> Vec<(f64, f64, f64)> {
        used.iter()
            .filter_map(|&(c, f, w)| self.x(c).map(|x| (x, f, w)))
            .collect()
    }

    /// Maps every value, the missing value's replacement and the recode through `map`.
    fn remap(&mut self, map: Vec<(Key, Key)>) {
        let step = Recode::Map(map.clone());
        for v in &mut self.values {
            *v = v.as_ref().and_then(|k| step.apply(k));
        }
        self.missing_to = self.missing_to.as_ref().and_then(|k| step.apply(k));
        self.recode = Some(match self.recode.take() {
            Some(Recode::Map(before)) => Recode::Map(
                before
                    .into_iter()
                    .filter_map(|(from, to)| step.apply(&to).map(|to| (from, to)))
                    .collect(),
            ),
            _ => step,
        });
        self.changed = true;
    }

    /// Applies ranges to a numeric field.
    fn ranges(&mut self, uppers: Vec<f64>) {
        let step = Recode::Ranges(uppers);
        for v in &mut self.values {
            *v = v.as_ref().and_then(|k| step.apply(k));
        }
        self.missing_to = self.missing_to.as_ref().and_then(|k| step.apply(k));
        self.recode = Some(step);
        self.changed = true;
    }

    /// The syntax that makes the prepared field.
    fn syntax(&self) -> String {
        let mut out = String::new();
        if let Some((expr, _)) = &self.derive {
            out += &format!("COMPUTE {} = {expr}.\n", self.name);
            if !self.changed {
                out += &format!(
                    "VARIABLE LEVEL {} ({}).\n",
                    self.name,
                    level_word(self.level)
                );
            }
        }
        if !self.changed {
            return out;
        }
        let (name, o) = (&self.name, &self.out);
        let specs = |recode: &Option<Recode>| match recode {
            None => " (ELSE=COPY)".to_string(),
            Some(Recode::Map(map)) => {
                let mut s: String = map
                    .iter()
                    .map(|(from, to)| format!(" ({}={})", key_literal(from), key_literal(to)))
                    .collect();
                s += " (ELSE=SYSMIS)";
                s
            }
            Some(Recode::Ranges(uppers)) => {
                let mut s: String = uppers
                    .iter()
                    .enumerate()
                    .map(|(k, u)| format!(" (LOWEST THRU {}={})", literal(*u), k + 1))
                    .collect();
                s += &format!(" (ELSE={})", uppers.len() + 1);
                s
            }
        };
        if self.continuous {
            out += &format!("RECODE {name} (MISSING=SYSMIS) (ELSE=COPY) INTO {o}.\n");
            for s in &self.statements {
                out += &s.replace("{o}", o);
                out += "\n";
            }
            if self.recode.is_some() {
                out += &format!("RECODE {o} (MISSING=SYSMIS){}.\n", specs(&self.recode));
            }
        } else {
            if let (Some(width), None) = (self.width, &self.recode) {
                out += &format!("STRING {o} (A{width}).\n");
            }
            let missing = match &self.missing_to {
                Some(k) => key_literal(k),
                None => "SYSMIS".into(),
            };
            out += &format!(
                "RECODE {name} (MISSING={missing}){} INTO {o}.\n",
                specs(&self.recode)
            );
        }
        out += &format!("VARIABLE LEVEL {o} ({}).\n", level_word(self.level));
        out
    }
}

fn level_word(level: Measure) -> &'static str {
    match level {
        Measure::Scale => "SCALE",
        Measure::Ordinal => "ORDINAL",
        _ => "NOMINAL",
    }
}

fn level_name(level: Measure) -> &'static str {
    match level {
        Measure::Scale => "Continuous",
        Measure::Ordinal => "Ordinal",
        _ => "Nominal",
    }
}

/// A number as syntax: the shortest text that reads back as the same number.
fn literal(x: f64) -> String {
    format!("{x}")
}

fn key_literal(key: &Key) -> String {
    match key {
        Key::Number(x) => literal(*x),
        Key::Text(s) => format!("'{}'", s.trim_end().replace('\'', "''")),
    }
}

/// A number for a table note, to four decimals.
fn short(x: f64) -> String {
    let s = format!("{x:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    match s {
        "-0" => "0".into(),
        s => s.into(),
    }
}

fn key_text(key: &Key) -> String {
    match key {
        Key::Number(x) => short(*x),
        Key::Text(s) => s.trim_end().to_string(),
    }
}

/// The kind of target a test is for.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Scale,
    Nominal,
    Ordinal,
}

/// The statistics of one category of an input against the target.
#[derive(Clone)]
enum Stats {
    Scale(Moments),
    /// Frequency and weight sums per target class.
    Table(Vec<f64>, Vec<f64>),
}

impl Stats {
    fn join(&self, other: &Stats) -> Stats {
        match (self, other) {
            (Stats::Scale(a), Stats::Scale(b)) => Stats::Scale(a.join(b)),
            (Stats::Table(n1, w1), Stats::Table(n2, w2)) => Stats::Table(
                n1.iter().zip(n2).map(|(a, b)| a + b).collect(),
                w1.iter().zip(w2).map(|(a, b)| a + b).collect(),
            ),
            _ => unreachable!("one target"),
        }
    }

    fn count(&self) -> f64 {
        match self {
            Stats::Scale(m) => m.n,
            Stats::Table(n, _) => n.iter().sum(),
        }
    }
}

/// The mean analysis weight of each cell, 1 for an empty cell.
fn mean_weights(rows: &[&Stats]) -> (Vec<Vec<f64>>, Vec<Vec<f64>>) {
    let mut counts = Vec::new();
    let mut wbar = Vec::new();
    for row in rows {
        let Stats::Table(n, w) = row else {
            unreachable!("a categorical target")
        };
        counts.push(n.clone());
        wbar.push(
            n.iter()
                .zip(w)
                .map(|(n, w)| if *n > 0.0 { w / n } else { 1.0 })
                .collect(),
        );
    }
    (counts, wbar)
}

/// Expected counts with row effects (`scores` Some) or under independence (None), by the
/// iterative fit of the Algorithms manual: m = alpha_i beta_j gamma_i^(s_j - s_bar) / wbar_ij.
fn expected(counts: &[Vec<f64>], wbar: &[Vec<f64>], scores: Option<&[f64]>) -> Vec<Vec<f64>> {
    let (rows, cols) = (counts.len(), counts[0].len());
    let row_n: Vec<f64> = counts.iter().map(|r| r.iter().sum()).collect();
    let col_n: Vec<f64> = (0..cols)
        .map(|j| counts.iter().map(|r| r[j]).sum())
        .collect();
    // the scores centred at their weighted mean
    let centred: Vec<f64> = match scores {
        Some(s) => {
            let col_w: Vec<f64> = (0..cols)
                .map(|j| (0..rows).map(|i| counts[i][j] * wbar[i][j]).sum())
                .collect();
            let bar =
                s.iter().zip(&col_w).map(|(s, w)| s * w).sum::<f64>() / col_w.iter().sum::<f64>();
            s.iter().map(|s| s - bar).collect()
        }
        None => vec![0.0; cols],
    };
    let (mut alpha, mut beta, mut gamma) = (vec![1.0; rows], vec![1.0; cols], vec![1.0; rows]);
    let fit = |alpha: &[f64], beta: &[f64], gamma: &[f64], i: usize, j: usize| {
        alpha[i] * beta[j] * f64::powf(gamma[i], centred[j]) / wbar[i][j]
    };
    let mut m: Vec<Vec<f64>> = wbar
        .iter()
        .map(|r| r.iter().map(|w| 1.0 / w).collect())
        .collect();
    for _ in 0..100 {
        for i in 0..rows {
            alpha[i] *= row_n[i] / m[i].iter().sum::<f64>();
        }
        for j in 0..cols {
            let sum: f64 = (0..rows)
                .map(|i| alpha[i] * f64::powf(gamma[i], centred[j]) / wbar[i][j])
                .sum();
            beta[j] = col_n[j] / sum;
        }
        if scores.is_some() {
            for i in 0..rows {
                let star: Vec<f64> = (0..cols)
                    .map(|j| fit(&alpha, &beta, &gamma, i, j))
                    .collect();
                let num: f64 = (0..cols)
                    .map(|j| centred[j] * (counts[i][j] - star[j]))
                    .sum();
                let den: f64 = (0..cols).map(|j| centred[j].powi(2) * star[j]).sum();
                let g = if den > 0.0 { 1.0 + num / den } else { 1.0 };
                if g > 0.0 {
                    gamma[i] *= g;
                }
            }
        }
        let next: Vec<Vec<f64>> = (0..rows)
            .map(|i| {
                (0..cols)
                    .map(|j| fit(&alpha, &beta, &gamma, i, j))
                    .collect()
            })
            .collect();
        let change = next
            .iter()
            .flatten()
            .zip(m.iter().flatten())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        m = next;
        if change < 0.001 {
            break;
        }
    }
    m
}

/// The p-value of the association between the categories `rows` and the target.
fn association(rows: &[&Stats], kind: Kind, classes: usize) -> f64 {
    let i = rows.len() as f64;
    if rows.len() < 2 {
        return 1.0;
    }
    match kind {
        Kind::Scale => {
            let m: Vec<Moments> = rows
                .iter()
                .map(|r| match r {
                    Stats::Scale(m) => *m,
                    _ => unreachable!("a continuous target"),
                })
                .collect();
            let all = m[1..].iter().fold(m[0], |a, b| a.join(b));
            let between: f64 = m.iter().map(|g| g.w * (g.mean - all.mean).powi(2)).sum();
            let within: f64 = m.iter().map(|g| g.m2).sum();
            let df2 = all.n - i;
            if df2 <= 0.0 {
                return 1.0;
            }
            if within <= 0.0 {
                return if between > 0.0 { 0.0 } else { 1.0 };
            }
            f_p((between / (i - 1.0)) / (within / df2), i - 1.0, df2)
        }
        Kind::Nominal => {
            let (counts, wbar) = mean_weights(rows);
            let m = expected(&counts, &wbar, None);
            let x2: f64 = counts
                .iter()
                .flatten()
                .zip(m.iter().flatten())
                .filter(|(_, m)| **m > 0.0)
                .map(|(n, m)| (n - m).powi(2) / m)
                .sum();
            let df = (classes as f64 - 1.0) * (i - 1.0);
            if df <= 0.0 { 1.0 } else { chi_square_p(x2, df) }
        }
        Kind::Ordinal => {
            if classes < 2 {
                return 1.0;
            }
            let (counts, wbar) = mean_weights(rows);
            let scores: Vec<f64> = (0..classes)
                .map(|j| 100.0 * j as f64 / (classes as f64 - 1.0))
                .collect();
            let independent = expected(&counts, &wbar, None);
            let effects = expected(&counts, &wbar, Some(&scores));
            let h2: f64 = 2.0
                * effects
                    .iter()
                    .flatten()
                    .zip(independent.iter().flatten())
                    .filter(|(e, m)| **e > 0.0 && **m > 0.0)
                    .map(|(e, m)| e * (e / m).ln())
                    .sum::<f64>();
            chi_square_p(h2.max(0.0), i - 1.0)
        }
    }
}

/// Supervised merging: the groups of category indices, each sorted, after merging the pair
/// with the largest p while it exceeds `alpha`.
fn merge_supervised(
    stats: &[Stats],
    ordinal: bool,
    kind: Kind,
    classes: usize,
    alpha: f64,
) -> Vec<Vec<usize>> {
    let mut groups: Vec<(Vec<usize>, Stats)> = stats
        .iter()
        .enumerate()
        .map(|(i, s)| (vec![i], s.clone()))
        .collect();
    while groups.len() > 1 {
        let mut best: Option<(f64, usize, usize)> = None;
        for a in 0..groups.len() {
            let partners = match ordinal {
                true => a + 1..(a + 2).min(groups.len()),
                false => a + 1..groups.len(),
            };
            for b in partners {
                let p = association(&[&groups[a].1, &groups[b].1], kind, classes);
                if best.is_none_or(|(q, _, _)| p > q) {
                    best = Some((p, a, b));
                }
            }
        }
        let Some((p, a, b)) = best else { break };
        if p <= alpha {
            break;
        }
        let (members, s) = groups.remove(b);
        groups[a].0.extend(members);
        groups[a].0.sort_unstable();
        groups[a].1 = groups[a].1.join(&s);
    }
    groups.into_iter().map(|g| g.0).collect()
}

/// Equal frequency merging of categories in the given order with counts `c`: groups of
/// consecutive positions.
fn merge_unsupervised(c: &[f64], minpct: f64) -> Vec<Vec<usize>> {
    let n = c.len();
    let total: f64 = c.iter().sum();
    let least = (minpct / 100.0 * total).round();
    if n < 3 {
        return (0..n).map(|i| vec![i]).collect();
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let (mut j1, mut j2) = (0, 0);
    while j1 < n {
        if c[j1..=j2].iter().sum::<f64>() >= least {
            groups.push((j1..=j2).collect());
            j1 = j2 + 1;
            j2 = j1;
            continue;
        }
        j2 += 1;
        // as the manual has it: once the window reaches the last category, the rest joins
        // the previous category without a further count check
        if j2 >= n - 1 {
            match groups.len() {
                0 => {
                    groups.push((0..n - 1).collect());
                    groups.push(vec![n - 1]);
                }
                1 => groups.push((j1..n).collect()),
                g => groups[g - 1].extend(j1..n),
            }
            break;
        }
    }
    groups
}

/// Box-Cox of y: lambda and the transformed values' weighted mean and SD.
fn box_cox(items: &[(f64, f64, f64)], c: f64) -> (f64, f64, f64) {
    let n: f64 = items.iter().map(|t| t.1).sum();
    let logs: f64 = items.iter().map(|t| t.1 * (t.0 - c).ln()).sum();
    let mut best: Option<(f64, f64, Moments)> = None;
    for j in 0..=12 {
        let lambda = -3.0 + 0.5 * j as f64;
        let g: Vec<(f64, f64, f64)> = items
            .iter()
            .map(|&(y, f, w)| (box_cox_value(y, c, lambda), f, w))
            .collect();
        let m = Moments::of(&g);
        let var = m.m2 / (n - 1.0);
        let l = -n / 2.0 * ((n - 1.0) / n * var).ln() + (lambda - 1.0) * logs;
        if best.as_ref().is_none_or(|(b, _, _)| l > *b) {
            best = Some((l, lambda, m));
        }
    }
    let (_, lambda, m) = best.expect("a lambda");
    (lambda, m.mean, m.sd())
}

fn box_cox_value(y: f64, c: f64, lambda: f64) -> f64 {
    if lambda == 0.0 {
        (y - c).ln()
    } else {
        ((y - c).powf(lambda) - 1.0) / lambda
    }
}

/// Supervised bins of a continuous input against the target classes: the cut points.
fn bins(classes: &[Moments], alpha: f64) -> Vec<f64> {
    let mut means: Vec<Moments> = classes.iter().filter(|m| m.n > 0.0).copied().collect();
    means.sort_by(|a, b| a.mean.total_cmp(&b.mean));
    let j = means.len();
    if j < 2 {
        return Vec::new();
    }
    let n: f64 = means.iter().map(|m| m.n).sum();
    let critical = if n <= j as f64 {
        means[j - 1].mean - means[0].mean
    } else {
        let df = n - j as f64;
        let r = (2.0 * (j as f64 - 1.0) * f_quantile(alpha, j as f64 - 1.0, df)).sqrt();
        let ms = (means.iter().map(|m| m.m2).sum::<f64>() / df).sqrt();
        let c = ms * (means.iter().map(|m| 1.0 / m.w).sum::<f64>() / j as f64).sqrt();
        r * c
    };
    // the homogeneous subsets: their joined moments
    let mut subsets: Vec<Moments> = Vec::new();
    let (mut i, mut q) = (0, j - 1);
    while i < j {
        if q == i || means[q].mean - means[i].mean <= critical {
            subsets.push(means[i + 1..=q].iter().fold(means[i], |a, b| a.join(b)));
            i = q + 1;
            q = j - 1;
        } else {
            q -= 1;
        }
    }
    let sd = |m: &Moments| if m.n > 1.0 { m.sd() } else { 0.0 };
    const EPS: f64 = 1e-10;
    subsets
        .windows(2)
        .map(|w| {
            let (a, b) = (sd(&w[0]), sd(&w[1]));
            w[0].mean + (a + EPS) / (a + b + 2.0 * EPS) * (w[1].mean - w[0].mean)
        })
        .collect()
}

/// The two-sided p of a correlation on `n` cases.
fn correlation_p(items: &[(f64, f64, f64, f64)]) -> f64 {
    let n: f64 = items.iter().map(|t| t.2).sum();
    if n <= 2.0 {
        return 1.0;
    }
    let w: f64 = items.iter().map(|t| t.2 * t.3).sum();
    let mx = items.iter().map(|t| t.0 * t.2 * t.3).sum::<f64>() / w;
    let my = items.iter().map(|t| t.1 * t.2 * t.3).sum::<f64>() / w;
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for &(x, y, f, w) in items {
        sxy += f * w * (x - mx) * (y - my);
        sxx += f * w * (x - mx).powi(2);
        syy += f * w * (y - my).powi(2);
    }
    if sxx <= 0.0 || syy <= 0.0 {
        return 1.0;
    }
    let r = sxy / (sxx * syy).sqrt();
    if r * r >= 1.0 {
        return 0.0;
    }
    t_p(r * ((n - 2.0) / (1.0 - r * r)).sqrt(), n - 2.0)
}

/// The weighted covariance of two columns over the cases where both have a value, with the
/// weight total less one as divisor, and that count of cases: pairwise deletion.
fn pair_covariance(a: &[Option<f64>], b: &[Option<f64>], used: &[(usize, f64, f64)]) -> (f64, f64) {
    let (mut sf, mut sw, mut sx, mut sy) = (0.0, 0.0, 0.0, 0.0);
    for &(c, f, w) in used {
        if let (Some(x), Some(y)) = (a[c], b[c]) {
            sf += f;
            sw += f * w;
            sx += f * w * x;
            sy += f * w * y;
        }
    }
    if sw <= 1.0 {
        return (0.0, sf);
    }
    let (mx, my) = (sx / sw, sy / sw);
    let mut sxy = 0.0;
    for &(c, f, w) in used {
        if let (Some(x), Some(y)) = (a[c], b[c]) {
            sxy += f * w * (x - mx) * (y - my);
        }
    }
    (sxy / (sw - 1.0), sf)
}

/// The two-sided p of a partial correlation r on `n` cases (n - 3 degrees of freedom).
fn partial_p(r: f64, n: f64) -> f64 {
    if n <= 3.0 {
        return 1.0;
    }
    if r * r >= 1.0 {
        return 0.0;
    }
    t_p(r * ((n - 3.0) / (1.0 - r * r)).sqrt(), n - 3.0)
}

/// ADP feature construction by the Algorithms manual: the scale inputs left are grouped by
/// their correlation (the correlation of an input with a group is the smallest absolute
/// correlation with its members; the level starts at 0.9 and falls by 0.1 down to 0.2, a group
/// taking at most five inputs), each group is replaced by its first principal component of the
/// covariance matrix (the eigenvector over the root of its eigenvalue), and with selection in
/// effect the inputs whose partial correlation with the target, given the new predictor, has a p
/// above the level are dropped. Covariances use pairwise deletion. Returns the groups as the
/// positions of the fields with their coefficients.
fn construct(
    fields: &mut [Field],
    target: &[Option<Key>],
    used: &[(usize, f64, f64)],
    selection: Option<f64>,
) -> Vec<Vec<(usize, f64)>> {
    let column = |f: &Field| -> Vec<Option<f64>> { (0..f.values.len()).map(|c| f.x(c)).collect() };
    let y: Vec<Option<f64>> = target
        .iter()
        .map(|k| match k {
            Some(Key::Number(v)) => Some(*v),
            _ => None,
        })
        .collect();
    let columns: Vec<Option<Vec<Option<f64>>>> = fields
        .iter()
        .map(|f| (!f.target && f.live() && f.level == Measure::Scale).then(|| column(f)))
        .collect();
    let mut pool: Vec<usize> = (0..fields.len())
        .filter(|&i| columns[i].is_some())
        .collect();
    let col = |i: usize| columns[i].as_ref().expect("a scale input");
    let cov = |i: usize, j: usize| pair_covariance(col(i), col(j), used).0;
    let corr = |i: usize, j: usize| {
        let d = (cov(i, i) * cov(j, j)).sqrt();
        if d > 0.0 { cov(i, j) / d } else { 0.0 }
    };
    let mut groups: Vec<Vec<(usize, f64)>> = Vec::new();
    let mut level = 9;
    while level > 1 {
        let alpha = f64::from(level) / 10.0;
        // the two most correlated inputs
        let mut best: Option<(usize, usize, f64)> = None;
        for (a, &i) in pool.iter().enumerate() {
            for &j in &pool[a + 1..] {
                let r = corr(i, j).abs();
                if r > alpha && best.is_none_or(|b| r > b.2) {
                    best = Some((i, j, r));
                }
            }
        }
        let Some((i, j, _)) = best else {
            level -= 1;
            continue;
        };
        let mut group = vec![i, j];
        while group.len() < 5 {
            let mut next: Option<(usize, f64)> = None;
            for &x in pool.iter().filter(|x| !group.contains(x)) {
                let r = group
                    .iter()
                    .map(|&g| corr(x, g).abs())
                    .fold(f64::INFINITY, f64::min);
                if r > alpha && next.is_none_or(|n| r > n.1) {
                    next = Some((x, r));
                }
            }
            match next {
                Some((x, _)) => group.push(x),
                None => break,
            }
        }
        // the first principal component of the group's covariance matrix
        let c: crate::matrix::Matrix = group
            .iter()
            .map(|&a| group.iter().map(|&b| cov(a, b)).collect())
            .collect();
        let (lambda, v) = crate::matrix::symmetric_eigen(&c)
            .into_iter()
            .next()
            .expect("an eigenvalue");
        // the sign is not given: the largest coefficient is positive
        let sign = v
            .iter()
            .fold(0.0_f64, |m, x| if x.abs() > m.abs() { *x } else { m })
            .signum();
        if lambda <= 0.0 {
            pool.retain(|x| !group.contains(x));
            continue;
        }
        let coefficients: Vec<f64> = v.iter().map(|x| sign * x / lambda.sqrt()).collect();
        let z: Vec<Option<f64>> = (0..y.len())
            .map(|case| {
                group
                    .iter()
                    .zip(&coefficients)
                    .map(|(&g, a)| col(g)[case].map(|x| a * x))
                    .sum::<Option<f64>>()
            })
            .collect();
        // step 7: the other inputs that no longer relate to the target given the new predictor
        if let Some(limit) = selection {
            let czz = pair_covariance(&z, &z, used).0;
            let cyy = pair_covariance(&y, &y, used).0;
            let czy = pair_covariance(&z, &y, used).0;
            let ryz = if czz > 0.0 && cyy > 0.0 {
                czy / (czz * cyy).sqrt()
            } else {
                0.0
            };
            for &x in pool
                .iter()
                .filter(|x| !group.contains(x))
                .copied()
                .collect::<Vec<_>>()
                .iter()
            {
                let (cxy, n) = pair_covariance(col(x), &y, used);
                let cxx = cov(x, x);
                let (czx, _) = pair_covariance(&z, col(x), used);
                if !(cxx > 0.0 && cyy > 0.0 && czz > 0.0) {
                    continue;
                }
                let rxy = cxy / (cxx * cyy).sqrt();
                let rxz = czx / (czz * cxx).sqrt();
                let (d1, d2) = (1.0 - rxz * rxz, 1.0 - ryz * ryz);
                let r = if d1 < 1e-10 || d2 < 1e-10 {
                    0.0
                } else {
                    ((rxy - rxz * ryz) / (d1.sqrt() * d2.sqrt())).clamp(-1.0, 1.0)
                };
                let p = partial_p(r, n);
                if p > limit {
                    fields[x].exclude(format!(
                        "not correlated with the target given the constructed predictor (p = {})",
                        short(p)
                    ));
                    pool.retain(|&q| q != x);
                }
            }
        }
        pool.retain(|x| !group.contains(x));
        groups.push(group.into_iter().zip(coefficients).collect());
    }
    groups
}

/// Above 0, and not NaN.
fn positive(x: f64) -> bool {
    x > 0.0
}

fn kind_of(field: &Field) -> Kind {
    match field.level {
        Measure::Scale => Kind::Scale,
        Measure::Ordinal => Kind::Ordinal,
        _ => Kind::Nominal,
    }
}

impl Engine {
    pub(crate) fn adp(&mut self, command: &Adp) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        let inputs = self.expand(&command.inputs)?;
        if let Some(twice) = duplicate(&inputs) {
            return Err(Error(format!("ADP: {twice} is given twice on INPUT")));
        }
        let mut names: Vec<(String, bool)> = Vec::new();
        if let Some(t) = &command.target {
            let t = data.variable(t)?.name().to_string();
            if inputs.iter().any(|n| same(n, &t)) {
                return Err(Error(format!(
                    "ADP: the target {t} cannot also be an input"
                )));
            }
            names.push((t, true));
        }
        names.extend(inputs.iter().map(|n| (n.clone(), false)));
        let cases = data.case_count();
        let freq: Vec<f64> = match case_weights(data)? {
            Some(w) => w.iter().map(|w| w.round()).collect(),
            None => vec![1.0; cases],
        };
        if let Some(w) = data.weight_variable()
            && names.iter().any(|(n, _)| same(n, w))
        {
            return Err(Error(format!("ADP cannot prepare the weight variable {w}")));
        }
        let analysis: Vec<Option<f64>> = match &command.analysis_weight {
            None => vec![Some(1.0); cases],
            Some(a) => {
                let v = data.variable(a)?;
                if v.is_string() {
                    return Err(Error(format!(
                        "ADP: the analysis weight {a} must be numeric"
                    )));
                }
                if names.iter().any(|(n, _)| same(n, v.name())) {
                    return Err(Error(format!(
                        "ADP: the analysis weight {a} cannot also be a target or input"
                    )));
                }
                (0..cases)
                    .map(|c| crate::twostep::numeric_value(&v, c, false).filter(|w| *w > 0.0))
                    .collect()
            }
        };
        let used: Vec<(usize, f64, f64)> = (0..cases)
            .filter(|&c| freq[c] > 0.0)
            .filter_map(|c| analysis[c].map(|w| (c, freq[c], w)))
            .collect();
        let taken = |n: &str, fields: &[Field]| {
            data.variable_index(n).is_some()
                || fields.iter().any(|f| same(&f.name, n) || same(&f.out, n))
        };

        // the fields, with the parts of dates and times
        let now = chrono::Local::now().naive_local();
        let mut fields: Vec<Field> = Vec::new();
        for (name, target) in &names {
            let v = data.variable(name)?;
            let mut level = v.measure();
            if v.is_string() && level != Measure::Ordinal {
                level = Measure::Nominal;
            } else if level == Measure::Unknown {
                level = Measure::Scale;
            }
            let values = categories(&v, false);
            let mut field = Field {
                name: v.name().to_string(),
                derive: None,
                target: *target,
                width: v.string_width(),
                level0: level,
                level,
                values,
                excluded: None,
                actions: Vec::new(),
                continuous: false,
                statements: Vec::new(),
                missing_to: None,
                recode: None,
                changed: false,
                out: String::new(),
                replaced_by: None,
            };
            let format = v.format().to_uppercase();
            let kind = format.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
            let is_date = !v.is_string() && DATES.contains(&kind);
            let is_time = !v.is_string() && TIMES.contains(&kind);
            if *target || !(is_date || is_time) {
                fields.push(field);
                continue;
            }
            let x: Vec<Option<f64>> = (0..cases).map(|c| field.x(c)).collect();
            let mut derived: Vec<Field> = Vec::new();
            let add = |suffix: &str,
                       expr: String,
                       level: Measure,
                       values: Vec<Option<f64>>,
                       action: String,
                       fields: &[Field],
                       derived: &[Field]| {
                let all: Vec<&Field> = fields.iter().chain(derived).collect();
                let root = format!("{}{suffix}", field.name);
                let name = free_name(&root, |n| {
                    data.variable_index(n).is_some() || all.iter().any(|f| same(&f.name, n))
                });
                Field {
                    name,
                    derive: Some((expr, field.name.clone())),
                    target: false,
                    width: None,
                    level0: level,
                    level,
                    values: values.into_iter().map(|v| v.map(Key::Number)).collect(),
                    excluded: None,
                    actions: vec![action],
                    continuous: false,
                    statements: Vec::new(),
                    missing_to: None,
                    recode: None,
                    changed: false,
                    out: String::new(),
                    replaced_by: None,
                }
            };
            let duration = if is_date {
                command.date_duration.as_ref()
            } else {
                command.time_duration.as_ref()
            };
            if let Some(d) = duration {
                let (reference, written, shown) = if is_date {
                    let [y, m, dd] = match d.reference {
                        Some(r) => r,
                        None => {
                            use chrono::Datelike;
                            [now.year() as f64, now.month() as f64, now.day() as f64]
                        }
                    };
                    let r = date(dd, m, y).ok_or_else(|| {
                        Error(format!(
                            "ADP: the reference date {y}-{m}-{dd} is not a valid date"
                        ))
                    })?;
                    (
                        r,
                        format!("DATE.DMY({dd},{m},{y})"),
                        format!("{y}-{m:02}-{dd:02}"),
                    )
                } else {
                    let [h, m, s] = match d.reference {
                        Some(r) => r,
                        None => {
                            use chrono::Timelike;
                            [now.hour() as f64, now.minute() as f64, now.second() as f64]
                        }
                    };
                    (
                        h * 3600.0 + m * 60.0 + s,
                        format!("TIME.HMS({h},{m},{s})"),
                        format!("{h:02}:{m:02}:{s:02}"),
                    )
                };
                let base = if is_date { DAY } else { 1.0 };
                let elapsed: Vec<Option<f64>> = x
                    .iter()
                    .map(|x| x.map(|x| (reference - x) / base))
                    .collect();
                let least = used
                    .iter()
                    .filter_map(|&(c, _, _)| elapsed[c])
                    .fold(f64::INFINITY, f64::min);
                let unit = d.unit.unwrap_or(match (is_date, least) {
                    (true, l) if l < 31.0 => 2,
                    (true, l) if l < 366.0 => 1,
                    (true, _) => 0,
                    (false, l) if l < 60.0 => 2,
                    (false, l) if l < 3600.0 => 1,
                    (false, _) => 0,
                });
                let (size, word) = match (is_date, unit) {
                    (true, 0) => (365.25, "years"),
                    (true, 1) => (30.4375, "months"),
                    (true, _) => (1.0, "days"),
                    (false, 0) => (3600.0, "hours"),
                    (false, 1) => (60.0, "minutes"),
                    (false, _) => (1.0, "seconds"),
                };
                let suffix = d.suffix.clone().unwrap_or_else(|| format!("_{word}"));
                let mut expr = format!("({written} - {})", field.name);
                if is_date {
                    expr += " / 86400";
                }
                if size != 1.0 {
                    expr += &format!(" / {}", literal(size));
                }
                let values = x
                    .iter()
                    .map(|x| {
                        x.map(|x| {
                            let mut v = reference - x;
                            if is_date {
                                v /= DAY;
                            }
                            if size != 1.0 {
                                v /= size;
                            }
                            v
                        })
                    })
                    .collect();
                let action = format!("{word} since {shown}");
                let f = add(
                    &suffix,
                    expr,
                    Measure::Scale,
                    values,
                    action,
                    &fields,
                    &derived,
                );
                derived.push(f);
            }
            for (i, suffix) in command.extract.iter().enumerate() {
                let (Some(suffix), true) =
                    (suffix, if i < 3 { is_date } else { is_date || is_time })
                else {
                    continue;
                };
                let function = [
                    "XDATE.YEAR",
                    "XDATE.MONTH",
                    "XDATE.MDAY",
                    "XDATE.HOUR",
                    "XDATE.MINUTE",
                    "XDATE.SECOND",
                ][i];
                let values = x
                    .iter()
                    .map(|x| x.and_then(|x| xdate(function, x)))
                    .collect();
                let action = format!(
                    "{} of {}",
                    ["year", "month", "day", "hour", "minute", "second"][i],
                    field.name
                );
                let f = add(
                    suffix,
                    format!("{function}({})", field.name),
                    Measure::Ordinal,
                    values,
                    action,
                    &fields,
                    &derived,
                );
                derived.push(f);
            }
            field.exclude(match derived.is_empty() {
                true => "a date or time field; no field is derived from it".into(),
                false => "a date or time field, replaced by the fields derived from it".into(),
            });
            fields.push(field);
            fields.extend(derived);
        }

        // screening
        for f in fields.iter_mut().filter(|f| f.live()) {
            let valid: f64 = used
                .iter()
                .filter(|u| f.values[u.0].is_some())
                .map(|u| u.1)
                .sum();
            let total: f64 = used.iter().map(|u| u.1).sum();
            let counts = f.counts(&used);
            let pct_missing = 100.0 * (total - valid) / total;
            let categorical = f.level != Measure::Scale;
            let largest = counts.iter().map(|c| c.1).fold(0.0, f64::max);
            let reason = if valid == 0.0 {
                Some("no valid values".to_string())
            } else if let Some(max) = command.max_missing.filter(|m| pct_missing > *m) {
                Some(format!(
                    "{}% missing values, more than {}",
                    short(pct_missing),
                    short(max)
                ))
            } else if counts.len() < 2 {
                Some("a constant field".into())
            } else if let Some(max) = command
                .max_single
                .filter(|m| categorical && 100.0 * largest / valid > *m)
            {
                Some(format!(
                    "{}% of the values in one category, more than {}",
                    short(100.0 * largest / valid),
                    short(max)
                ))
            } else {
                command
                    .max_categories
                    .filter(|m| f.level == Measure::Nominal && counts.len() > *m)
                    .map(|m| format!("{} categories, more than {m}", counts.len()))
            };
            if let Some(reason) = reason {
                f.exclude(reason);
            }
        }
        if let Some(t) = fields.iter().find(|f| f.target)
            && let Some(reason) = &t.excluded
        {
            return Err(Error(format!(
                "ADP: the target {} is screened out ({reason}), so there is nothing to prepare for",
                t.name
            )));
        }
        if !fields.iter().any(|f| !f.target && f.live()) {
            return Err(Error(
                "ADP: every input is screened out or is a date or time field without derived fields; see the screening settings".into(),
            ));
        }

        // levels
        for f in fields.iter_mut().filter(|f| f.live() && f.width.is_none()) {
            if !(if f.target {
                command.adjust_target
            } else {
                command.adjust_input
            }) {
                continue;
            }
            let distinct = f.counts(&used).len();
            let to = match f.level {
                Measure::Scale if distinct < command.min_continuous => Measure::Ordinal,
                Measure::Ordinal if distinct > command.max_ordinal => Measure::Scale,
                _ => continue,
            };
            f.actions.push(format!(
                "{} to {} ({distinct} values)",
                level_name(f.level).to_lowercase(),
                level_name(to).to_lowercase()
            ));
            f.level = to;
            f.changed = true;
        }
        for f in &mut fields {
            f.continuous = f.level == Measure::Scale && f.width.is_none();
        }

        // outliers
        let weighted = command.analysis_weight.as_deref();
        for f in fields.iter_mut().filter(|f| f.live() && f.continuous) {
            if !(if f.target {
                command.outlier_target
            } else {
                command.outlier_input
            }) {
                continue;
            }
            let items = f.items(&used);
            let all = Moments::of(&items);
            if all.n < 2.0 {
                continue;
            }
            let sd = all.sd() * ((all.n - 1.0) / (all.w - 1.0)).sqrt();
            if !positive(sd) {
                continue;
            }
            // the interval of each value: -3 (to 3 SD below) through 4 (beyond 3 SD above)
            let interval = |x: f64| ((x - all.mean) / sd).ceil().clamp(-3.0, 4.0) as i32;
            let mut size = [0.0; 8];
            for &(x, fr, _) in &items {
                size[(interval(x) + 3) as usize] += fr;
            }
            let (mut l, mut r, mut p) = (-3, 4, 0.0);
            while l < r {
                let (nl, nr) = (size[(l + 3) as usize], size[(r + 3) as usize]);
                let share = nl.min(nr) / all.n;
                if p + share >= 0.05 {
                    break;
                }
                p += share;
                if nl <= nr {
                    l += 1;
                } else {
                    r -= 1;
                }
            }
            let kept: Vec<(f64, f64, f64)> = items
                .iter()
                .filter(|t| (l..=r).contains(&interval(t.0)))
                .copied()
                .collect();
            let robust = Moments::of(&kept);
            let reach = command.cutoff * robust.sd();
            if !positive(reach) {
                continue;
            }
            let m = robust.mean;
            let (lo, hi) = (m - reach, m + reach);
            // -1 below the cutoff, 1 above it, 0 inside; the same comparisons as the syntax
            let side = |x: f64, w: f64| match weighted {
                None => (x > hi) as i32 - (x < lo) as i32,
                Some(_) => {
                    let d = w.sqrt() * (x - m);
                    (d > reach) as i32 - (d < -reach) as i32
                }
            };
            let mut outliers = 0.0;
            for &(c, fr, w) in &used {
                if f.x(c).is_some_and(|x| side(x, w) != 0) {
                    outliers += fr;
                }
            }
            if outliers == 0.0 {
                continue;
            }
            for (c, w) in analysis.iter().enumerate() {
                // a case without a valid analysis weight keeps its value, as in the syntax
                let (Some(x), &Some(w)) = (f.x(c), w) else {
                    continue;
                };
                let s = side(x, w);
                f.values[c] = match (command.outlier_to_missing, s, weighted) {
                    (_, 0, _) => Some(x),
                    (true, _, _) => None,
                    (false, -1, None) => Some(lo),
                    (false, _, None) => Some(hi),
                    (false, -1, Some(_)) => Some(m - reach / w.sqrt()),
                    (false, _, Some(_)) => Some(m + reach / w.sqrt()),
                }
                .map(Key::Number);
            }
            f.statements
                .push(match (weighted, command.outlier_to_missing) {
                    (None, true) => format!(
                        "IF ({{o}} < {} OR {{o}} > {}) {{o}} = $SYSMIS.",
                        literal(lo),
                        literal(hi)
                    ),
                    (None, false) => format!(
                        "IF ({{o}} < {lo}) {{o}} = {lo}.\nIF ({{o}} > {hi}) {{o}} = {hi}.",
                        lo = literal(lo),
                        hi = literal(hi)
                    ),
                    (Some(a), true) => format!(
                        "IF (ABS(SQRT({a}) * ({{o}} - {m})) > {r}) {{o}} = $SYSMIS.",
                        m = literal(m),
                        r = literal(reach)
                    ),
                    (Some(a), false) => format!(
                        "IF (SQRT({a}) * ({{o}} - {m}) < -{r}) {{o}} = {m} - {r} / SQRT({a}).\n\
                     IF (SQRT({a}) * ({{o}} - {m}) > {r}) {{o}} = {m} + {r} / SQRT({a}).",
                        m = literal(m),
                        r = literal(reach)
                    ),
                });
            f.changed = true;
            f.actions.push(match command.outlier_to_missing {
                true => format!(
                    "{} outliers made missing (beyond {} and {})",
                    short(outliers),
                    short(lo),
                    short(hi)
                ),
                false => format!(
                    "{} outliers moved to {} and {}",
                    short(outliers),
                    short(lo),
                    short(hi)
                ),
            });
        }

        // missing values
        for f in fields.iter_mut().filter(|f| f.live()) {
            let levels = if f.target {
                command.replace_target
            } else {
                command.replace_input
            };
            let Some(AdpLevels {
                continuous,
                nominal,
                ordinal,
            }) = levels
            else {
                continue;
            };
            let wanted = match f.level {
                Measure::Scale => continuous,
                Measure::Ordinal => ordinal,
                _ => nominal,
            };
            if !wanted || !used.iter().any(|u| f.values[u.0].is_none()) {
                continue;
            }
            let (value, how) = match f.level {
                Measure::Scale => (Key::Number(Moments::of(&f.items(&used)).mean), "mean"),
                Measure::Ordinal => {
                    let counts = f.counts(&used);
                    let n: f64 = counts.iter().map(|c| c.1).sum();
                    let mut cumulative = 0.0;
                    let median = counts
                        .iter()
                        .find(|(_, c)| {
                            cumulative += c;
                            cumulative / n >= 0.5
                        })
                        .expect("a valid value")
                        .0
                        .clone();
                    (median, "median")
                }
                _ => {
                    let counts = f.counts(&used);
                    // the first of the largest counts: the least value
                    let mode = counts
                        .iter()
                        .fold(None::<&(Key, f64)>, |best, c| match best {
                            Some(b) if b.1 >= c.1 => Some(b),
                            _ => Some(c),
                        })
                        .expect("a valid value")
                        .0
                        .clone();
                    (mode, "mode")
                }
            };
            for v in &mut f.values {
                if v.is_none() {
                    *v = Some(value.clone());
                }
            }
            f.actions.push(format!(
                "missing values replaced by the {how} {}",
                key_text(&value)
            ));
            if f.continuous {
                f.statements.push(format!(
                    "IF (SYSMIS({{o}})) {{o}} = {}.",
                    key_literal(&value)
                ));
            } else {
                f.missing_to = Some(value);
            }
            f.changed = true;
        }

        // target handling
        let target = fields.iter().position(|f| f.target);
        if let Some(t) = target {
            let f = &mut fields[t];
            if f.level == Measure::Nominal && command.reorder_target {
                reorder(f, &used);
            }
            if f.continuous
                && let Some((mean, sd_user)) = command.rescale_target
            {
                let items = f.items(&used);
                let c = items.iter().map(|t| t.0).fold(f64::INFINITY, f64::min) - 1.0;
                let (lambda, gbar, gsd) = box_cox(&items, c);
                if gsd > 0.0 {
                    let a = sd_user / gsd;
                    for v in &mut f.values {
                        if let Some(Key::Number(y)) = v {
                            *y = (box_cox_value(*y, c, lambda) - gbar) * a + mean;
                        }
                    }
                    f.statements.push(match lambda == 0.0 {
                        true => format!("COMPUTE {{o}} = LN({{o}} - {}).", literal(c)),
                        false => format!(
                            "COMPUTE {{o}} = (({{o}} - {}) ** {} - 1) / {}.",
                            literal(c),
                            literal(lambda),
                            literal(lambda)
                        ),
                    });
                    f.statements.push(format!(
                        "COMPUTE {{o}} = ({{o}} - {}) * {} + {}.",
                        literal(gbar),
                        literal(a),
                        literal(mean)
                    ));
                    f.actions.push(format!(
                        "Box-Cox with lambda {}, then mean {} and SD {}",
                        short(lambda),
                        short(mean),
                        short(sd_user)
                    ));
                    f.changed = true;
                }
            }
        }
        let target_kind = target.map(|t| kind_of(&fields[t]));
        let target_values: Vec<Option<Key>> = match target {
            Some(t) => fields[t].values.clone(),
            None => Vec::new(),
        };
        let target_classes: Vec<Key> = match target {
            Some(t) if target_kind != Some(Kind::Scale) => {
                fields[t].counts(&used).into_iter().map(|c| c.0).collect()
            }
            _ => Vec::new(),
        };
        let class_of = |c: usize| {
            target_values[c]
                .as_ref()
                .and_then(|k| target_classes.iter().position(|t| t.compare(k).is_eq()))
        };

        // inputs
        for f in fields.iter_mut().filter(|f| f.live() && !f.target) {
            if !f.continuous {
                let ordinal = f.level == Measure::Ordinal;
                match (target_kind, command.merge_unsupervised) {
                    (Some(kind), _) => {
                        if let Some(alpha) = command.merge_supervised {
                            supervised(
                                f,
                                &used,
                                kind,
                                &target_values,
                                &class_of,
                                target_classes.len(),
                                alpha,
                            );
                        }
                    }
                    (None, Some((levels, minpct)))
                        if (ordinal && levels.ordinal) || (!ordinal && levels.nominal) =>
                    {
                        unsupervised(f, &used, minpct);
                    }
                    _ => {}
                }
                if f.live() && f.level == Measure::Nominal && command.reorder_input {
                    reorder(f, &used);
                }
                continue;
            }
            match target_kind {
                Some(Kind::Nominal | Kind::Ordinal) if command.binning.is_some() => {
                    let mut classes = vec![Moments::default(); target_classes.len()];
                    let mut items: Vec<Vec<(f64, f64, f64)>> =
                        vec![Vec::new(); target_classes.len()];
                    for &(c, fr, w) in &used {
                        if let (Some(x), Some(k)) = (f.x(c), class_of(c)) {
                            items[k].push((x, fr, w));
                        }
                    }
                    for (m, items) in classes.iter_mut().zip(&items) {
                        if !items.is_empty() {
                            *m = Moments::of(items);
                        }
                    }
                    let cuts = bins(&classes, command.binning.expect("checked"));
                    if cuts.is_empty() {
                        f.exclude("binning against the target leaves a single bin".into());
                        continue;
                    }
                    f.actions.push(format!(
                        "{} bins, cut at {}",
                        cuts.len() + 1,
                        cuts.iter()
                            .map(|c| short(*c))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    f.ranges(cuts);
                    f.level = Measure::Ordinal;
                    continue;
                }
                Some(Kind::Scale) => {
                    if let Some(alpha) = command.selection {
                        let pairs: Vec<(f64, f64, f64, f64)> = used
                            .iter()
                            .filter_map(|&(c, fr, w)| match (f.x(c), &target_values[c]) {
                                (Some(x), Some(Key::Number(y))) => Some((x, *y, fr, w)),
                                _ => None,
                            })
                            .collect();
                        let p = correlation_p(&pairs);
                        if p > alpha {
                            f.exclude(format!("not correlated with the target (p = {})", short(p)));
                            continue;
                        }
                    }
                }
                _ => {}
            }
            let m = Moments::of(&f.items(&used));
            let (from, a, to, action) = match command.rescale_input {
                AdpRescale::None => continue,
                AdpRescale::Zscore { mean, sd } => {
                    if !(m.n > 1.0 && m.sd() > 0.0) {
                        continue;
                    }
                    (
                        m.mean,
                        sd / m.sd(),
                        mean,
                        format!("z score with mean {} and SD {}", short(mean), short(sd)),
                    )
                }
                AdpRescale::MinMax { min, max } => {
                    let items = f.items(&used);
                    let lo = items.iter().map(|t| t.0).fold(f64::INFINITY, f64::min);
                    let hi = items.iter().map(|t| t.0).fold(f64::NEG_INFINITY, f64::max);
                    if !positive(hi - lo) {
                        continue;
                    }
                    (
                        lo,
                        (max - min) / (hi - lo),
                        min,
                        format!("min-max from {} to {}", short(min), short(max)),
                    )
                }
            };
            for v in &mut f.values {
                if let Some(Key::Number(x)) = v {
                    *x = (*x - from) * a + to;
                }
            }
            f.statements.push(format!(
                "COMPUTE {{o}} = ({{o}} - {}) * {} + {}.",
                literal(from),
                literal(a),
                literal(to)
            ));
            f.actions.push(action);
            f.changed = true;
        }
        // feature construction: the groups of correlated scale inputs become predictors
        let mut built: Vec<(usize, Vec<(usize, f64)>)> = Vec::new();
        if let (Some(root), Some(Kind::Scale)) = (&command.construction, target_kind) {
            let groups = construct(&mut fields, &target_values, &used, command.selection);
            for (k, group) in groups.into_iter().enumerate() {
                let name = free_name(&format!("{root}{}", k + 1), |n| taken(n, &fields));
                let values: Vec<Option<Key>> = (0..cases)
                    .map(|c| {
                        group
                            .iter()
                            .map(|&(j, a)| fields[j].x(c).map(|x| a * x))
                            .sum::<Option<f64>>()
                            .map(Key::Number)
                    })
                    .collect();
                let sources: Vec<String> =
                    group.iter().map(|&(j, _)| fields[j].name.clone()).collect();
                let weights: Vec<String> = group.iter().map(|&(_, a)| short(a)).collect();
                for &(j, _) in &group {
                    fields[j].replaced_by = Some(name.clone());
                }
                built.push((fields.len(), group));
                fields.push(Field {
                    name: name.clone(),
                    derive: Some((String::new(), sources.join(", "))),
                    target: false,
                    width: None,
                    level0: Measure::Scale,
                    level: Measure::Scale,
                    values,
                    excluded: None,
                    actions: vec![format!(
                        "constructed from {} with coefficients {}",
                        sources.join(", "),
                        weights.join(", ")
                    )],
                    continuous: false,
                    statements: Vec::new(),
                    missing_to: None,
                    recode: None,
                    changed: false,
                    out: String::new(),
                    replaced_by: None,
                });
            }
        }
        if !fields.iter().any(|f| !f.target && f.live()) {
            return Err(Error(
                "ADP: no input is left after preparation: every one was screened out, merged into one category or found unrelated to the target".into(),
            ));
        }

        // names of the prepared fields
        for i in 0..fields.len() {
            if !fields[i].live() {
                continue;
            }
            let out = match fields[i].changed {
                false => fields[i].name.clone(),
                true => {
                    let suffix = match fields[i].target {
                        true => &command.suffix_target,
                        false => &command.suffix_input,
                    };
                    let root = format!("{}{suffix}", fields[i].name);
                    let out = free_name(&root, |n| taken(n, &fields));
                    if !is_valid_name(&out) {
                        return Err(Error(format!(
                            "ADP: the prepared field name {out} is not a valid variable name; choose a shorter suffix on CRITERIA"
                        )));
                    }
                    out
                }
            };
            fields[i].out = out;
        }
        for (at, group) in &built {
            let expr = group
                .iter()
                .map(|&(j, a)| format!("({}) * {}", literal(a), fields[j].out))
                .collect::<Vec<_>>()
                .join(" + ");
            if let Some((e, _)) = &mut fields[*at].derive {
                *e = expr;
            }
        }

        // the syntax file
        let mut syntax = String::from("* Data preparation written by ADP.\n");
        for f in fields.iter().filter(|f| f.live()) {
            syntax += &f.syntax();
        }
        let prepared: Vec<&str> = fields
            .iter()
            .filter(|f| f.live() && f.replaced_by.is_none())
            .map(|f| f.out.as_str())
            .collect();
        syntax += &format!("EXECUTE.\n* Prepared fields: {}.\n", prepared.join(" "));
        if !self.dry_run {
            let path = self.path(&command.outfile);
            std::fs::write(&path, &syntax).map_err(|e| {
                Error(format!(
                    "ADP cannot write the preparation file {}: {e}",
                    path.display()
                ))
            })?;
        }

        let shown = self.shown();
        let data = self.active()?;
        let rows = fields
            .iter()
            .map(|f| {
                let field = match data.variable(&f.name) {
                    Ok(v) => shown.variable(&v),
                    Err(_) => f.name.clone(),
                };
                let role = match (&f.derive, f.target) {
                    (_, true) => "Target".to_string(),
                    (Some((_, from)), _) => format!("Input from {from}"),
                    (None, false) => "Input".to_string(),
                };
                let (prepared, level, actions) = match (&f.excluded, &f.replaced_by) {
                    (Some(reason), _) => (Cell::Empty, Cell::Empty, format!("Excluded: {reason}")),
                    (None, Some(by)) => (
                        Cell::Empty,
                        Cell::Empty,
                        format!("Replaced by the constructed predictor {by}"),
                    ),
                    (None, None) => (
                        text(f.out.clone()),
                        text(level_name(f.level)),
                        match f.actions.is_empty() {
                            true => "Unchanged".into(),
                            false => f.actions.join("; "),
                        },
                    ),
                };
                vec![
                    text(field),
                    text(role),
                    text(level_name(f.level0)),
                    prepared,
                    level,
                    text(actions),
                ]
            })
            .collect();
        let columns = [
            "Field",
            "Role",
            "Level",
            "Prepared Field",
            "Prepared Level",
            "Actions",
        ];
        let mut table = plain_table("Field Processing", 1, &columns, rows);
        table.caption = Some(format!(
            "The preparation is written as syntax to {}; run it to make the prepared fields.",
            command.outfile
        ));
        Ok(vec![table])
    }
}

/// Renumbers a nominal field from 0 by ascending count, ties by value.
fn reorder(f: &mut Field, used: &[(usize, f64, f64)]) {
    let mut counts = f.counts(used);
    counts.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.compare(&b.0)));
    let map = counts
        .iter()
        .enumerate()
        .map(|(i, (k, _))| (k.clone(), Key::Number(i as f64)))
        .collect();
    f.remap(map);
    f.actions
        .push("categories numbered from 0 by ascending count".into());
}

/// Applies merged groups of the categories `keys` (sorted by value): ordinal numeric fields by
/// ranges at each group's largest value, others by a map; groups numbered from 1 in order of
/// their least value.
fn apply_groups(f: &mut Field, keys: &[Key], mut groups: Vec<Vec<usize>>, how: &str) {
    if groups.len() == keys.len() {
        return;
    }
    let before = keys.len();
    if groups.len() < 2 {
        f.exclude(format!("{how} merging leaves a single category"));
        return;
    }
    for g in &mut groups {
        g.sort_unstable();
    }
    groups.sort_by_key(|g| g[0]);
    let numeric = keys.iter().all(|k| matches!(k, Key::Number(_)));
    if f.level == Measure::Ordinal && numeric {
        let uppers = groups[..groups.len() - 1]
            .iter()
            .map(|g| match keys[*g.last().expect("a member")] {
                Key::Number(x) => x,
                Key::Text(_) => unreachable!("numeric"),
            })
            .collect();
        f.ranges(uppers);
    } else {
        let mut map = Vec::new();
        for (code, g) in groups.iter().enumerate() {
            for &i in g {
                map.push((keys[i].clone(), Key::Number((code + 1) as f64)));
            }
        }
        map.sort_by(|a, b| a.0.compare(&b.0));
        f.remap(map);
    }
    f.actions.push(format!(
        "{how} merging: {before} categories into {}",
        groups.len()
    ));
}

fn supervised(
    f: &mut Field,
    used: &[(usize, f64, f64)],
    kind: Kind,
    target: &[Option<Key>],
    class_of: &dyn Fn(usize) -> Option<usize>,
    classes: usize,
    alpha: f64,
) {
    let keys: Vec<Key> = f.counts(used).into_iter().map(|c| c.0).collect();
    let mut stats: Vec<Option<Stats>> = vec![None; keys.len()];
    for &(c, fr, w) in used {
        let Some(k) = &f.values[c] else { continue };
        let i = keys
            .iter()
            .position(|x| x.compare(k).is_eq())
            .expect("counted");
        let s = match kind {
            Kind::Scale => match &target[c] {
                Some(Key::Number(y)) => Stats::Scale(Moments::of(&[(*y, fr, w)])),
                _ => continue,
            },
            _ => {
                let Some(j) = class_of(c) else { continue };
                let (mut n, mut ws) = (vec![0.0; classes], vec![0.0; classes]);
                n[j] = fr;
                ws[j] = fr * w;
                Stats::Table(n, ws)
            }
        };
        stats[i] = Some(match &stats[i] {
            Some(old) => old.join(&s),
            None => s,
        });
    }
    let present: Vec<usize> = (0..keys.len()).filter(|&i| stats[i].is_some()).collect();
    if present.len() < 2 {
        f.exclude("no two categories have cases with a valid target".into());
        return;
    }
    let rows: Vec<Stats> = present
        .iter()
        .map(|&i| stats[i].clone().expect("present"))
        .collect();
    let ordinal = f.level == Measure::Ordinal;
    let merged = merge_supervised(&rows, ordinal, kind, classes, alpha);
    let groups_stats: Vec<Stats> = merged
        .iter()
        .map(|g| {
            g[1..]
                .iter()
                .fold(rows[g[0]].clone(), |a, &i| a.join(&rows[i]))
        })
        .collect();
    let p = association(&groups_stats.iter().collect::<Vec<_>>(), kind, classes);
    let mut groups: Vec<Vec<usize>> = merged
        .iter()
        .map(|g| g.iter().map(|&i| present[i]).collect())
        .collect();
    // the categories without a case of valid target join the smallest group (ordinal ones
    // fall in the ranges)
    let missing: Vec<usize> = (0..keys.len()).filter(|i| stats[*i].is_none()).collect();
    if !missing.is_empty() && !ordinal {
        let smallest = (0..groups.len())
            .min_by(|&a, &b| {
                groups_stats[a]
                    .count()
                    .total_cmp(&groups_stats[b].count())
                    .then(groups[a][0].cmp(&groups[b][0]))
            })
            .expect("groups");
        groups[smallest].extend(missing);
    } else if !missing.is_empty() {
        // an ordinal category joins the group whose range holds it
        for i in missing {
            let g = groups
                .iter()
                .position(|g| g.iter().any(|&m| m > i))
                .unwrap_or(groups.len() - 1);
            groups[g].push(i);
        }
    }
    if groups.len() >= 2 && p > alpha {
        f.exclude(format!("not related to the target (p = {})", short(p)));
        return;
    }
    apply_groups(f, &keys, groups, "supervised");
}

fn unsupervised(f: &mut Field, used: &[(usize, f64, f64)], minpct: f64) {
    let counts = f.counts(used);
    let mut order: Vec<usize> = (0..counts.len()).collect();
    if f.level != Measure::Ordinal {
        order.sort_by(|&a, &b| counts[a].1.total_cmp(&counts[b].1).then(a.cmp(&b)));
    }
    let c: Vec<f64> = order.iter().map(|&i| counts[i].1).collect();
    let groups = merge_unsupervised(&c, minpct)
        .into_iter()
        .map(|g| g.into_iter().map(|p| order[p]).collect())
        .collect();
    let keys: Vec<Key> = counts.into_iter().map(|c| c.0).collect();
    apply_groups(f, &keys, groups, "unsupervised");
}
