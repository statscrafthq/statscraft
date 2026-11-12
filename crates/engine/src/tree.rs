//! TREE: classification trees grown by CHAID and exhaustive CHAID.
//!
//! Rules from the Command Syntax Reference (TREE) and the Algorithms manual (TREE Algorithms:
//! CHAID and Exhaustive CHAID, Assignment and Risk Estimation, Gain Summary):
//! - The dependent is nominal or ordinal; its classes are its valid values, or USEVALUES (which
//!   may hold user-missing values of a nominal dependent). A case is left out when its dependent
//!   is not a class, its weight rounds to 0 or less, or every predictor is missing. A weight
//!   counts the case as often as its rounded value.
//! - A scale predictor is cut into K groups: with r the average rank (weighted) of a value and
//!   N the weighted count, the values with floor(r K / (N + 1)) = k form group k; the largest value
//!   of each group but the last is a break point, and x falls into the first group whose break
//!   point is at least x. It is then ordinal.
//! - A missing predictor value (system- or user-missing, the latter valid for nominal predictors
//!   under NOMINALMISSING=VALID) is a category of its own. For a nominal predictor it is like any
//!   other; for an ordinal one it floats: the other categories are merged first, then it either
//!   joins its most similar group or stays apart, whichever gives the smaller significance.
//! - CHAID merging: while more than two groups remain, the allowed pair (any pair for nominal,
//!   neighbours for ordinal) whose 2 x J table is least significant is merged when that
//!   significance exceeds ALPHAMERGE. Exhaustive CHAID merges the least significant pair down to
//!   two groups and keeps the set whose whole table is most significant.
//! - Significance: Pearson or likelihood-ratio chi-square for a nominal dependent; for an
//!   ordinal one the likelihood ratio of Goodman's row effects model against independence, on
//!   I - 1 df, fitted by the manual's iteration (CONVERGE, MAXITERATIONS). Classes absent from the
//!   cases tested do not count.
//! - Bonferroni: CHAID multiplies by the number of ways I categories form r groups: C(I-1, r-1)
//!   for ordinal, the Stirling number S(I, r) for nominal, C(I-2, r-2) + r C(I-2, r-1) for ordinal
//!   with a floating category. Exhaustive CHAID by I(I-1)/2 for ordinal and I(I^2-1)/6 for nominal:
//!   the number of pairs it could merge at each step, summed, as the manual's text defines it
//!   (its printed nominal formula has /2, which gives 3 for two categories that allow no choice).
//! - A node splits on the predictor with the smallest adjusted significance when that is at most
//!   ALPHASPLIT. It does not split when pure, smaller than MINPARENTSIZE or at MAXDEPTH. FORCE
//!   splits the root by its variable whatever the significance.
//! - Assignment: the class with the largest count (with equal costs and priors from the data),
//!   the smallest class on ties. Risk is the share misclassified, its standard error
//!   sqrt(R (1 - R) / N).
//! - Gains per target class: node size, target count and percent of all target cases, response
//!   (target share in the node) and index (response over the overall share).

use crate::crosstabs::Key;
use crate::descriptives::case_weights;
use crate::dist::chi_square_p;
use crate::format::category_text;
use crate::pass::default_measure;
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Value, Values, Variable};
use statscraft_syntax::{Level, Tree, TreeCategory, TreeSave, VarItem};
use std::collections::VecDeque;

/// One predictor, its categories coded per analysis case; `missing` (= the number of
/// categories) is the code of a missing value.
struct Predictor {
    name: String,
    nominal: bool,
    labels: Vec<String>,
    /// The break points of a scale predictor, in its format.
    breaks: Option<Vec<String>>,
    codes: Vec<usize>,
}

impl Predictor {
    fn missing(&self) -> usize {
        self.labels.len()
    }

    /// The categories of a group, as the node definition shows them.
    fn describe(&self, group: &[usize]) -> String {
        let missing = group.contains(&self.missing());
        let values: Vec<usize> = group
            .iter()
            .copied()
            .filter(|&c| c != self.missing())
            .collect();
        let mut text = match (&self.breaks, values.first(), values.last()) {
            (Some(breaks), Some(&low), Some(&high)) => {
                let lower = low.checked_sub(1).and_then(|i| breaks.get(i));
                match (lower, breaks.get(high)) {
                    (Some(a), Some(b)) => format!("({a}, {b}]"),
                    (None, Some(b)) => format!("<= {b}"),
                    (Some(a), None) => format!("> {a}"),
                    (None, None) => "all values".to_string(),
                }
            }
            _ => values
                .iter()
                .map(|&c| self.labels[c].clone())
                .collect::<Vec<_>>()
                .join("; "),
        };
        if missing {
            if !text.is_empty() {
                text.push_str("; ");
            }
            text.push_str("<missing>");
        }
        text
    }
}

struct Split {
    predictor: usize,
    groups: Vec<Vec<usize>>,
    statistic: f64,
    df: f64,
    sig: f64,
}

struct Node {
    parent: Option<usize>,
    depth: usize,
    cases: Vec<usize>,
    counts: Vec<f64>,
    n: f64,
    /// The predictor and its categories that define the node.
    definition: Option<(usize, Vec<usize>)>,
    split: Option<Split>,
    children: Vec<usize>,
}

impl Node {
    /// The assigned class: the largest count, the first on ties.
    fn predicted(&self) -> usize {
        let mut best = 0;
        for (j, &count) in self.counts.iter().enumerate() {
            if count > self.counts[best] {
                best = j;
            }
        }
        best
    }
}

/// What the tests of independence need.
struct Tester<'a> {
    /// Scores of the classes of an ordinal dependent; None for a nominal one.
    scores: Option<&'a [f64]>,
    likelihood_ratio: bool,
    converge: f64,
    max_iterations: usize,
}

impl Tester<'_> {
    /// Statistic, degrees of freedom and significance of a table with groups as rows and
    /// classes as columns. Empty rows and columns are left out.
    fn test(&self, table: &[Vec<f64>]) -> (f64, f64, f64) {
        let rows: Vec<&Vec<f64>> = table
            .iter()
            .filter(|r| r.iter().sum::<f64>() > 0.0)
            .collect();
        let columns: Vec<usize> = (0..table.first().map_or(0, Vec::len))
            .filter(|&j| rows.iter().map(|r| r[j]).sum::<f64>() > 0.0)
            .collect();
        if rows.len() < 2 || columns.len() < 2 {
            return (0.0, 0.0, 1.0);
        }
        let n: Vec<Vec<f64>> = rows
            .iter()
            .map(|r| columns.iter().map(|&j| r[j]).collect())
            .collect();
        let row_totals: Vec<f64> = n.iter().map(|r| r.iter().sum()).collect();
        let column_totals: Vec<f64> = (0..columns.len())
            .map(|j| n.iter().map(|r| r[j]).sum())
            .collect();
        let total: f64 = row_totals.iter().sum();
        let expected = |i: usize, j: usize| row_totals[i] * column_totals[j] / total;
        let (statistic, df) = match self.scores {
            None => {
                let mut statistic = 0.0;
                for (i, row) in n.iter().enumerate() {
                    for (j, &observed) in row.iter().enumerate() {
                        let e = expected(i, j);
                        statistic += if self.likelihood_ratio {
                            if observed > 0.0 {
                                2.0 * observed * (observed / e).ln()
                            } else {
                                0.0
                            }
                        } else {
                            (observed - e).powi(2) / e
                        };
                    }
                }
                let df = (n.len() - 1) * (columns.len() - 1);
                (statistic, df as f64)
            }
            Some(scores) => {
                let scores: Vec<f64> = columns.iter().map(|&j| scores[j]).collect();
                let fitted = self.row_effects(&n, &scores);
                let mut statistic = 0.0;
                for (i, row) in fitted.iter().enumerate() {
                    for (j, &m) in row.iter().enumerate() {
                        if m > 0.0 {
                            statistic += 2.0 * m * (m / expected(i, j)).ln();
                        }
                    }
                }
                (statistic.max(0.0), (n.len() - 1) as f64)
            }
        };
        (statistic, df, chi_square_p(statistic, df))
    }

    /// The expected counts under Goodman's row effects model, by the manual's iteration (all
    /// case weights 1).
    fn row_effects(&self, n: &[Vec<f64>], scores: &[f64]) -> Vec<Vec<f64>> {
        let rows = n.len();
        let row_totals: Vec<f64> = n.iter().map(|r| r.iter().sum()).collect();
        let column_totals: Vec<f64> = (0..scores.len())
            .map(|j| n.iter().map(|r| r[j]).sum())
            .collect();
        let total: f64 = row_totals.iter().sum();
        let mean: f64 = scores
            .iter()
            .zip(&column_totals)
            .map(|(s, c)| s * c)
            .sum::<f64>()
            / total;
        let d: Vec<f64> = scores.iter().map(|s| s - mean).collect();
        let (mut alpha, mut beta, mut gamma) =
            (vec![1.0; rows], vec![1.0; scores.len()], vec![1.0; rows]);
        let mut m = vec![vec![1.0; scores.len()]; rows];
        let power = |g: f64, dj: f64| g.powf(dj);
        for _ in 0..self.max_iterations {
            for i in 0..rows {
                let sum: f64 = (0..scores.len())
                    .map(|j| beta[j] * power(gamma[i], d[j]))
                    .sum();
                alpha[i] = row_totals[i] / sum;
            }
            for j in 0..scores.len() {
                let sum: f64 = (0..rows).map(|i| alpha[i] * power(gamma[i], d[j])).sum();
                beta[j] = column_totals[j] / sum;
            }
            for i in 0..rows {
                let star: Vec<f64> = (0..scores.len())
                    .map(|j| alpha[i] * beta[j] * power(gamma[i], d[j]))
                    .collect();
                let up: f64 = (0..scores.len()).map(|j| d[j] * (n[i][j] - star[j])).sum();
                let down: f64 = (0..scores.len()).map(|j| d[j] * d[j] * star[j]).sum();
                let g = 1.0 + up / down;
                if g > 0.0 {
                    gamma[i] *= g;
                }
            }
            let mut change: f64 = 0.0;
            for (i, row) in m.iter_mut().enumerate() {
                for (j, cell) in row.iter_mut().enumerate() {
                    let next = alpha[i] * beta[j] * power(gamma[i], d[j]);
                    change = change.max((next - *cell).abs());
                    *cell = next;
                }
            }
            if change < self.converge {
                break;
            }
        }
        m
    }
}

/// The settings of the merging step.
struct Merging<'a> {
    tester: Tester<'a>,
    exhaustive: bool,
    alpha_merge: f64,
    min_child: f64,
    bonferroni: bool,
}

fn row_sum(table: &[Vec<f64>], group: &[usize]) -> Vec<f64> {
    let mut sum = vec![0.0; table[0].len()];
    for &c in group {
        for (s, v) in sum.iter_mut().zip(&table[c]) {
            *s += v;
        }
    }
    sum
}

fn binomial(n: f64, k: f64) -> f64 {
    if k < 0.0 || k > n {
        return 0.0;
    }
    (0..k as usize).fold(1.0, |b, i| b * (n - i as f64) / (i as f64 + 1.0))
}

/// The ways `n` categories form `k` nonempty groups (Stirling number of the second kind).
fn stirling(n: usize, k: usize) -> f64 {
    let mut row = vec![0.0; k + 1];
    row[0] = 1.0;
    for _ in 0..n {
        for j in (1..=k).rev() {
            row[j] = j as f64 * row[j] + row[j - 1];
        }
        row[0] = 0.0;
    }
    row[k]
}

impl Merging<'_> {
    fn sig(&self, table: &[Vec<f64>], groups: &[Vec<usize>]) -> f64 {
        let rows: Vec<Vec<f64>> = groups.iter().map(|g| row_sum(table, g)).collect();
        self.tester.test(&rows).2
    }

    fn pair_sig(&self, table: &[Vec<f64>], a: &[usize], b: &[usize]) -> f64 {
        self.sig(table, &[a.to_vec(), b.to_vec()])
    }

    /// The allowed pair whose table is least significant, with that significance. For an
    /// ordinal predictor only neighbours are allowed, and any group with `floating`, the index
    /// of the group that holds the missing category alone.
    fn most_similar(
        &self,
        table: &[Vec<f64>],
        groups: &[Vec<usize>],
        nominal: bool,
        floating: Option<usize>,
        only: Option<usize>,
    ) -> Option<(usize, usize, f64)> {
        let mut best: Option<(usize, usize, f64)> = None;
        for i in 0..groups.len() {
            for j in i + 1..groups.len() {
                let allowed = nominal || j == i + 1 || Some(j) == floating;
                if !allowed || only.is_some_and(|o| o != i && o != j) {
                    continue;
                }
                let sig = self.pair_sig(table, &groups[i], &groups[j]);
                if best.is_none_or(|(_, _, b)| sig > b) {
                    best = Some((i, j, sig));
                }
            }
        }
        best
    }

    fn join(groups: &mut Vec<Vec<usize>>, i: usize, j: usize) {
        let moved = groups.remove(j);
        groups[i].extend(moved);
        groups[i].sort_unstable();
    }

    /// The groups of a predictor in a node from its table (categories by classes, the missing
    /// category last), the statistic, df and adjusted significance of the split they make. None
    /// when fewer than two groups remain.
    fn merge(
        &self,
        predictor: &Predictor,
        table: &[Vec<f64>],
    ) -> Option<(Vec<Vec<usize>>, f64, f64, f64)> {
        let size = |c: &usize| table[*c].iter().sum::<f64>();
        let present: Vec<usize> = (0..table.len()).filter(|c| size(c) > 0.0).collect();
        let missing = predictor.missing();
        let nominal = predictor.nominal;
        let floats = !nominal && present.contains(&missing);
        let mut groups: Vec<Vec<usize>> = present
            .iter()
            .filter(|&&c| !floats || c != missing)
            .map(|&c| vec![c])
            .collect();
        if self.exhaustive {
            let mut best = (self.sig(table, &groups), groups.clone());
            while groups.len() > 2 {
                let (i, j, _) = self.most_similar(table, &groups, nominal, None, None)?;
                Self::join(&mut groups, i, j);
                let sig = self.sig(table, &groups);
                if sig < best.0 {
                    best = (sig, groups.clone());
                }
            }
            if groups.len() > 1 {
                groups = best.1;
            }
        } else {
            while groups.len() > 2 {
                let (i, j, sig) = self.most_similar(table, &groups, nominal, None, None)?;
                if sig <= self.alpha_merge {
                    break;
                }
                Self::join(&mut groups, i, j);
            }
        }
        if floats {
            // the missing category joins its most similar group or stays apart
            let alone = vec![missing];
            let mut apart = groups.clone();
            apart.push(alone.clone());
            groups = match groups
                .iter()
                .enumerate()
                .map(|(i, g)| (i, self.pair_sig(table, g, &alone)))
                .reduce(|a, b| if b.1 > a.1 { b } else { a })
            {
                Some((i, _)) => {
                    let mut joined = groups.clone();
                    joined[i].push(missing);
                    let joined_sig = if joined.len() > 1 {
                        self.sig(table, &joined)
                    } else {
                        1.0
                    };
                    if joined_sig < self.sig(table, &apart) {
                        joined
                    } else {
                        apart
                    }
                }
                None => apart,
            };
        }
        // groups smaller than MINCHILDSIZE join their most similar allowed group, smallest first
        loop {
            let floating = groups.iter().position(|g| g == &[missing] && floats);
            let small = groups
                .iter()
                .enumerate()
                .map(|(i, g)| (i, row_sum(table, g).iter().sum::<f64>()))
                .filter(|&(_, n)| n < self.min_child)
                .reduce(|a, b| if b.1 < a.1 { b } else { a });
            let Some((s, _)) = small.filter(|_| groups.len() > 1) else {
                break;
            };
            let (i, j, _) = self.most_similar(table, &groups, nominal, floating, Some(s))?;
            Self::join(&mut groups, i, j);
        }
        if groups.len() < 2 {
            return None;
        }
        let rows: Vec<Vec<f64>> = groups.iter().map(|g| row_sum(table, g)).collect();
        let (statistic, df, sig) = self.tester.test(&rows);
        let (i, r) = (present.len() as f64, groups.len() as f64);
        let multiplier = match (self.bonferroni, self.exhaustive, nominal) {
            (false, ..) => 1.0,
            (true, true, true) => i * (i * i - 1.0) / 6.0,
            (true, true, false) => i * (i - 1.0) / 2.0,
            (true, false, true) => stirling(present.len(), groups.len()),
            (true, false, false) if floats => {
                binomial(i - 2.0, r - 2.0) + r * binomial(i - 2.0, r - 1.0)
            }
            (true, false, false) => binomial(i - 1.0, r - 1.0),
        };
        Some((groups, statistic, df, (sig * multiplier).min(1.0)))
    }
}

/// The break points of a scale predictor from its valid (value, weight) pairs, for `k` groups.
fn break_points(mut values: Vec<(f64, f64)>, k: usize) -> Vec<f64> {
    values.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = values.iter().map(|v| v.1).sum();
    let mut breaks: Vec<f64> = Vec::new();
    let mut last_group = None;
    let (mut below, mut start) = (0.0, 0);
    while start < values.len() {
        let end = start
            + values[start..]
                .iter()
                .take_while(|v| v.0 == values[start].0)
                .count();
        let t: f64 = values[start..end].iter().map(|v| v.1).sum();
        let rank = below + (t + 1.0) / 2.0;
        let group = (rank * k as f64 / (total + 1.0)).floor() as usize;
        // the largest value of each group; a later value of the same group replaces it
        if last_group == Some(group) {
            breaks.pop();
        }
        breaks.push(values[start].0);
        last_group = Some(group);
        below += t;
        start = end;
    }
    breaks.pop();
    breaks
}

enum Raw {
    Valid(Key),
    UserMissing(Key),
    SysMissing,
}

fn raw(variable: &Variable, case: usize) -> Raw {
    match variable.values() {
        Values::Numeric(values) => match values.get(case) {
            None => Raw::SysMissing,
            Some(v) if variable.is_user_missing(v) => Raw::UserMissing(Key::Number(v)),
            Some(v) => Raw::Valid(Key::Number(v)),
        },
        Values::String(values) => {
            let v = values.get(case);
            let key = Key::Text(v.to_string());
            if variable.is_user_missing_str(v) {
                Raw::UserMissing(key)
            } else {
                Raw::Valid(key)
            }
        }
    }
}

fn key_of(value: &statscraft_syntax::Value) -> Key {
    match value {
        statscraft_syntax::Value::Number(n) => Key::Number(*n),
        statscraft_syntax::Value::String(s) => Key::Text(s.trim_end().to_string()),
    }
}

fn sorted_unique(mut keys: Vec<Key>) -> Vec<Key> {
    keys.sort_by(|a, b| a.compare(b));
    keys.dedup();
    keys
}

/// The measurement level of a variable: the one written, else the dictionary's, else the one
/// its data give it (the rule of a data pass, as the manual says for an unknown level, with SET
/// SCALEMIN as `scale_min`).
fn level_of(
    data: &Dataset,
    variable: &Variable,
    written: Option<Level>,
    scale_min: usize,
) -> Level {
    let measure = match (variable.measure(), data.variable_index(variable.name())) {
        (Measure::Unknown, Some(index)) => default_measure(data, index, scale_min),
        (known, _) => known,
    };
    written.unwrap_or(match measure {
        Measure::Ordinal => Level::Ordinal,
        Measure::Scale => Level::Scale,
        _ => Level::Nominal,
    })
}

fn percent(part: f64, whole: f64) -> Cell {
    if whole > 0.0 {
        Cell::Number(100.0 * part / whole)
    } else {
        Cell::Empty
    }
}

fn text(s: impl Into<String>) -> Cell {
    Cell::Text(s.into())
}

impl Engine {
    pub(crate) fn tree(&mut self, command: &Tree) -> Result<Vec<Table>, Error> {
        for message in &command.ignored {
            self.warn(message.clone());
        }
        let (shown, labels) = (self.shown(), Shown::labels(self.currencies()));
        let scale_min = self.scale_min();
        let data = self.active()?;
        let mut warnings: Vec<String> = Vec::new();
        let dependent = data.variable(&command.dependent)?;
        let dependent_level = level_of(data, &dependent, command.dependent_level, scale_min);
        if dependent_level == Level::Scale {
            if dependent.is_string() {
                warnings.push(format!(
                    "the string variable {} cannot be scale; it is nominal",
                    dependent.name()
                ));
            } else {
                return Err(Error(format!(
                    "TREE with a scale dependent variable ({}) is not supported yet: give it a \
                     level, as in {} [n] or {} [o]",
                    dependent.name(),
                    dependent.name(),
                    dependent.name()
                )));
            }
        }
        let ordinal = dependent_level == Level::Ordinal;
        let weight_name = data.weight_variable().map(str::to_string);

        // the predictors: the last level written for a variable holds, FORCE's first
        let mut predictors: Vec<(String, Option<Level>)> = Vec::new();
        let add = |name: String, level: Option<Level>, list: &mut Vec<(String, Option<Level>)>| {
            match list.iter_mut().find(|(n, _)| n.eq_ignore_ascii_case(&name)) {
                Some(entry) => {
                    if level.is_some() {
                        entry.1 = level;
                    }
                }
                None => list.push((name, level)),
            }
        };
        if let Some((name, level)) = &command.force {
            let name = self.expand(&[VarItem::Name(name.clone())])?.remove(0);
            if name.eq_ignore_ascii_case(dependent.name())
                || weight_name
                    .as_ref()
                    .is_some_and(|w| w.eq_ignore_ascii_case(&name))
            {
                return Err(Error(format!(
                    "FORCE cannot name the dependent or the weight variable ({name})"
                )));
            }
            add(name, *level, &mut predictors);
        }
        let forced_level = predictors.first().and_then(|p| p.1);
        for (item, level) in &command.predictors {
            let names = self.expand(std::slice::from_ref(item))?;
            let count = names.len();
            for (k, name) in names.into_iter().enumerate() {
                // a level after a TO range applies to its last variable only
                let applies = matches!(item, VarItem::All) || k + 1 == count;
                add(name, level.filter(|_| applies), &mut predictors);
            }
        }
        if let (Some(level), true) = (forced_level, command.force.is_some()) {
            predictors[0].1 = Some(level);
        }
        predictors.retain(|(name, _)| {
            !name.eq_ignore_ascii_case(dependent.name())
                && !weight_name
                    .as_ref()
                    .is_some_and(|w| w.eq_ignore_ascii_case(name))
        });
        if predictors.is_empty() {
            return Err(Error("TREE needs at least one predictor".into()));
        }
        let mut intervals: Vec<(String, usize)> = Vec::new();
        for (list, n) in &command.variable_intervals {
            for name in self.expand(list)? {
                intervals.retain(|(v, _)| !v.eq_ignore_ascii_case(&name));
                intervals.push((name, *n));
            }
        }

        let weights = case_weights(data)?;
        let weight = |c: usize| weights.as_ref().map_or(1.0, |w| w[c].round());

        // the classes of the dependent
        let in_use: Vec<usize> = (0..data.case_count())
            .filter(|&c| weight(c) > 0.0)
            .collect();
        let mut valid = Vec::new();
        let mut user_missing = Vec::new();
        for &c in &in_use {
            match raw(&dependent, c) {
                Raw::Valid(key) => valid.push(key),
                Raw::UserMissing(key) => user_missing.push(key),
                Raw::SysMissing => {}
            }
        }
        let (valid, user_missing) = (sorted_unique(valid), sorted_unique(user_missing));
        let classes: Vec<Key> = match &command.use_values {
            None => valid.clone(),
            Some(list) => {
                let mut chosen = Vec::new();
                for item in list {
                    match item {
                        TreeCategory::Valid => chosen.extend(valid.iter().cloned()),
                        TreeCategory::Missing if ordinal => warnings.push(
                            "user-missing values of an ordinal dependent are left out".into(),
                        ),
                        TreeCategory::Missing => chosen.extend(user_missing.iter().cloned()),
                        TreeCategory::Value(value) => {
                            let key = key_of(value);
                            if valid.contains(&key) {
                                chosen.push(key);
                            } else if user_missing.contains(&key) {
                                if ordinal {
                                    warnings.push(format!(
                                        "{} is user-missing and the dependent is ordinal; \
                                         it is left out",
                                        labels.value(&dependent, &key)
                                    ));
                                } else {
                                    chosen.push(key);
                                }
                            } else {
                                warnings.push(format!(
                                    "category {} of {} does not occur in the data",
                                    labels.value(&dependent, &key),
                                    dependent.name()
                                ));
                            }
                        }
                    }
                }
                sorted_unique(chosen)
            }
        };
        if classes.len() < 2 {
            return Err(Error(format!(
                "the dependent variable {} has fewer than two categories in the analysis",
                dependent.name()
            )));
        }
        if classes.len() > 126 {
            return Err(Error(format!(
                "the dependent variable {} has more than 126 categories",
                dependent.name()
            )));
        }
        for target in &command.targets {
            let key = key_of(target);
            if !classes.contains(&key) {
                warnings.push(format!(
                    "target category {} is not a category of the analysis; it has no gain table",
                    labels.value(&dependent, &key)
                ));
            }
        }
        let charted = command.plot_gain || command.plot_response || command.plot_index;
        if charted && !command.targets.iter().any(|t| classes.contains(&key_of(t))) {
            warnings.push(
                "PLOT GAIN, RESPONSE and INDEX need target categories (DEPCATEGORIES TARGET); no chart is made"
                    .into(),
            );
        }
        let class_of = |c: usize| match raw(&dependent, c) {
            Raw::Valid(key) | Raw::UserMissing(key) => classes.iter().position(|k| *k == key),
            Raw::SysMissing => None,
        };
        let scores: Vec<f64> = match &command.scores {
            Some(custom) if ordinal => {
                let mut scores = Vec::new();
                for class in &classes {
                    let score = custom.iter().rev().find(|(v, _)| key_of(v) == *class);
                    match score {
                        Some((_, s)) => scores.push(*s),
                        None => {
                            return Err(Error(format!(
                                "SCORES gives no score for category {}",
                                labels.value(&dependent, class)
                            )));
                        }
                    }
                }
                let mut sorted = scores.clone();
                sorted.sort_by(f64::total_cmp);
                if sorted.windows(2).any(|w| w[0] == w[1]) {
                    return Err(Error("SCORES must differ between categories".into()));
                }
                scores
            }
            Some(_) => {
                warnings.push("SCORES applies to an ordinal dependent only; ignored".into());
                Vec::new()
            }
            None => (1..=classes.len()).map(|s| s as f64).collect(),
        };
        // the predictors' values for the cases with a class
        let mut variables = Vec::new();
        for (name, written) in &predictors {
            let variable = data.variable(name)?;
            let mut level = level_of(data, &variable, *written, scale_min);
            if level == Level::Scale && variable.is_string() {
                warnings.push(format!(
                    "the string variable {name} cannot be scale; it is nominal"
                ));
                level = Level::Nominal;
            }
            variables.push((variable, level));
        }
        for (name, _) in &intervals {
            let used = variables
                .iter()
                .any(|(v, l)| v.name().eq_ignore_ascii_case(name) && *l == Level::Scale);
            if !used {
                warnings.push(format!(
                    "INTERVALS for {name} is ignored: it is not a scale predictor"
                ));
            }
        }
        let missing_of = |level: Level, r: &Raw| match r {
            Raw::Valid(_) => false,
            Raw::UserMissing(_) => !(level == Level::Nominal && command.nominal_missing_valid),
            Raw::SysMissing => true,
        };
        let classed: Vec<(usize, usize)> = in_use
            .iter()
            .filter_map(|&c| class_of(c).map(|k| (c, k)))
            .collect();
        let cases: Vec<(usize, usize)> = classed
            .iter()
            .copied()
            .filter(|&(c, _)| variables.iter().any(|(v, l)| !missing_of(*l, &raw(v, c))))
            .collect();
        if cases.is_empty() {
            return Err(Error("no cases are left for the analysis".into()));
        }
        let mut coded = Vec::new();
        for (variable, level) in &variables {
            let values: Vec<Option<Key>> = cases
                .iter()
                .map(|&(c, _)| {
                    let r = raw(variable, c);
                    if missing_of(*level, &r) {
                        return None;
                    }
                    match r {
                        Raw::Valid(k) | Raw::UserMissing(k) => Some(k),
                        Raw::SysMissing => None,
                    }
                })
                .collect();
            let predictor = if *level == Level::Scale {
                let k = intervals
                    .iter()
                    .find(|(n, _)| n.eq_ignore_ascii_case(variable.name()))
                    .map_or(command.intervals, |(_, k)| *k);
                let pairs: Vec<(f64, f64)> = values
                    .iter()
                    .zip(&cases)
                    .filter_map(|(v, &(c, _))| match v {
                        Some(Key::Number(x)) => Some((*x, weight(c))),
                        _ => None,
                    })
                    .collect();
                let breaks = break_points(pairs, k);
                let missing = breaks.len() + 1;
                let codes = values
                    .iter()
                    .map(|v| match v {
                        Some(Key::Number(x)) => breaks.partition_point(|b| b < x),
                        _ => missing,
                    })
                    .collect();
                Predictor {
                    name: shown.variable(variable),
                    nominal: false,
                    labels: vec![String::new(); missing],
                    breaks: Some(
                        breaks
                            .iter()
                            .map(|b| category_text(*b, variable.format()))
                            .collect(),
                    ),
                    codes,
                }
            } else {
                let keys = sorted_unique(values.iter().flatten().cloned().collect());
                if keys.len() > 126 {
                    return Err(Error(format!(
                        "the predictor {} has more than 126 categories",
                        variable.name()
                    )));
                }
                let codes = values
                    .iter()
                    .map(|v| match v {
                        Some(k) => keys.iter().position(|x| x == k).expect("a key of the data"),
                        None => keys.len(),
                    })
                    .collect();
                Predictor {
                    name: shown.variable(variable),
                    nominal: *level == Level::Nominal,
                    labels: keys.iter().map(|k| shown.value(variable, k)).collect(),
                    breaks: None,
                    codes,
                }
            };
            coded.push(predictor);
        }
        let case_weight: Vec<f64> = cases.iter().map(|&(c, _)| weight(c)).collect();
        let case_class: Vec<usize> = cases.iter().map(|&(_, k)| k).collect();

        // growing
        let merging = Merging {
            tester: Tester {
                scores: ordinal.then_some(&scores[..]),
                likelihood_ratio: command.likelihood_ratio,
                converge: command.converge,
                max_iterations: command.max_iterations,
            },
            exhaustive: command.exhaustive,
            alpha_merge: command.alpha_merge,
            min_child: command.min_child,
            bonferroni: command.bonferroni,
        };
        let max_depth = command.max_depth.unwrap_or(3);
        let node_of = |cases: Vec<usize>, parent, depth, definition| {
            let mut counts = vec![0.0; classes.len()];
            for &i in &cases {
                counts[case_class[i]] += case_weight[i];
            }
            Node {
                parent,
                depth,
                n: counts.iter().sum(),
                cases,
                counts,
                definition,
                split: None,
                children: Vec::new(),
            }
        };
        let mut nodes = vec![node_of((0..cases.len()).collect(), None, 0, None)];
        let mut queue = VecDeque::from([0]);
        while let Some(id) = queue.pop_front() {
            let node = &nodes[id];
            let pure = node.counts.iter().filter(|&&n| n > 0.0).count() < 2;
            if node.depth >= max_depth || node.n < command.min_parent || pure {
                continue;
            }
            let mut best: Option<Split> = None;
            for (p, predictor) in coded.iter().enumerate() {
                if id == 0 && command.force.is_some() && p != 0 {
                    continue;
                }
                let mut table = vec![vec![0.0; classes.len()]; predictor.labels.len() + 1];
                for &i in &node.cases {
                    table[predictor.codes[i]][case_class[i]] += case_weight[i];
                }
                let Some((groups, statistic, df, sig)) = merging.merge(predictor, &table) else {
                    continue;
                };
                if best.as_ref().is_none_or(|b| sig < b.sig) {
                    best = Some(Split {
                        predictor: p,
                        groups,
                        statistic,
                        df,
                        sig,
                    });
                }
            }
            let forced = id == 0 && command.force.is_some();
            let Some(split) = best.filter(|s| forced || s.sig <= command.alpha_split) else {
                continue;
            };
            let predictor = &coded[split.predictor];
            let mut children = Vec::new();
            for group in &split.groups {
                let members: Vec<usize> = nodes[id]
                    .cases
                    .iter()
                    .copied()
                    .filter(|&i| group.contains(&predictor.codes[i]))
                    .collect();
                let depth = nodes[id].depth + 1;
                children.push(node_of(
                    members,
                    Some(id),
                    depth,
                    Some((split.predictor, group.clone())),
                ));
            }
            for child in children {
                let next = nodes.len();
                nodes[id].children.push(next);
                queue.push_back(nodes.len());
                nodes.push(child);
            }
            nodes[id].split = Some(split);
        }

        let mut charts = Vec::new();
        let mut tables =
            self.tree_output(command, &nodes, &coded, &classes, &dependent, &mut charts)?;
        let total_n: f64 = in_use.iter().map(|&c| weight(c)).sum();
        if command.print.case_summary {
            let included = nodes[0].n;
            tables.insert(
                0,
                Table {
                    title: "Case Processing Summary".into(),
                    row_headers: 1,
                    columns: vec![String::new(), "N".into(), "Percent".into()],
                    column_groups: vec![Vec::new(); 3],
                    rows: vec![
                        vec![
                            text("Included"),
                            Cell::Number(included),
                            percent(included, total_n),
                        ],
                        vec![
                            text("Excluded"),
                            Cell::Number(total_n - included),
                            percent(total_n - included, total_n),
                        ],
                        vec![
                            text("Total"),
                            Cell::Number(total_n),
                            percent(total_n, total_n),
                        ],
                    ],
                    formats: vec![vec![String::new(), "F8.0".into(), "PCT6.1".into()]; 3],
                    ..Table::default()
                },
            );
        }
        let save: Vec<(TreeSave, Option<String>)> = command.save.clone();
        let terminal_of: Vec<usize> = {
            let mut of = vec![0; cases.len()];
            for (id, node) in nodes.iter().enumerate() {
                if node.children.is_empty() {
                    for &i in &node.cases {
                        of[i] = id;
                    }
                }
            }
            of
        };
        let case_numbers: Vec<usize> = cases.iter().map(|&(c, _)| c).collect();
        let dependent_name = dependent.name().to_string();
        let dependent_format = dependent.format().to_string();
        let dependent_width = dependent.string_width();
        let dependent_labels: Vec<(Value, String)> = dependent.value_labels().to_vec();
        let dependent_measure = dependent.measure();
        let class_labels: Vec<String> = classes
            .iter()
            .map(|k| labels.value(&dependent, k))
            .collect();
        self.charts.extend(charts);
        for warning in warnings {
            self.warn(warning);
        }
        if save.is_empty() {
            return Ok(tables);
        }
        let case_count = self.active()?.case_count();
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            let fresh = |data: &statscraft_data::Dataset, given: &Option<String>, default: &str| {
                if let Some(name) = given {
                    return Ok(name.clone());
                }
                let mut name = default.to_string();
                let mut k = 0;
                while data.variable_index(&name).is_some() {
                    k += 1;
                    name = format!("{default}_{k}");
                }
                Ok::<String, Error>(name)
            };
            for (what, given) in &save {
                if let Some(name) = given
                    && data.variable_index(name).is_some()
                {
                    return Err(Error(format!("TREE /SAVE: variable {name} already exists")));
                }
                match what {
                    TreeSave::NodeId => {
                        let name = fresh(data, given, "NodeID")?;
                        let mut values = vec![None; case_count];
                        for (i, &c) in case_numbers.iter().enumerate() {
                            values[c] = Some(terminal_of[i] as f64);
                        }
                        data.set_numeric_values(&name, values)?;
                        data.set_variable_label(&name, Some("Terminal node"))?;
                        data.set_format(&name, "F8.0")?;
                        data.set_measure(&name, Measure::Nominal)?;
                    }
                    TreeSave::PredictedValue => {
                        let name = fresh(data, given, "PredictedValue")?;
                        let predicted: Vec<&Key> = terminal_of
                            .iter()
                            .map(|&t| &classes[nodes[t].predicted()])
                            .collect();
                        match dependent_width {
                            Some(width) => {
                                let mut values = vec![String::new(); case_count];
                                for (i, &c) in case_numbers.iter().enumerate() {
                                    if let Key::Text(t) = predicted[i] {
                                        values[c] = t.clone();
                                    }
                                }
                                data.set_string_values(&name, width, values)?;
                            }
                            None => {
                                let mut values = vec![None; case_count];
                                for (i, &c) in case_numbers.iter().enumerate() {
                                    if let Key::Number(n) = predicted[i] {
                                        values[c] = Some(*n);
                                    }
                                }
                                data.set_numeric_values(&name, values)?;
                                data.set_format(&name, &dependent_format)?;
                            }
                        }
                        let label = format!("Predicted value of {dependent_name}");
                        data.set_variable_label(&name, Some(&label))?;
                        data.set_value_labels(&name, dependent_labels.clone())?;
                        data.set_measure(&name, dependent_measure)?;
                    }
                    TreeSave::PredictedProbability => {
                        let root = given.as_deref().unwrap_or("PredictedProbability");
                        for (j, class_label) in class_labels.iter().enumerate() {
                            let name = fresh(data, &None, &format!("{root}_{}", j + 1))?;
                            let mut values = vec![None; case_count];
                            for (i, &c) in case_numbers.iter().enumerate() {
                                let node = &nodes[terminal_of[i]];
                                values[c] = Some(node.counts[j] / node.n);
                            }
                            data.set_numeric_values(&name, values)?;
                            let label = format!(
                                "Predicted probability of {dependent_name} = {class_label}"
                            );
                            data.set_variable_label(&name, Some(&label))?;
                            data.set_format(&name, "F8.3")?;
                            data.set_measure(&name, Measure::Scale)?;
                        }
                    }
                }
            }
            Ok(())
        })?;
        Ok(tables)
    }

    /// The tables of a grown tree.
    fn tree_output(
        &self,
        command: &Tree,
        nodes: &[Node],
        predictors: &[Predictor],
        classes: &[Key],
        dependent: &Variable,
        charts: &mut Vec<crate::Chart>,
    ) -> Result<Vec<Table>, Error> {
        let shown = self.shown();
        let class_labels: Vec<String> = classes.iter().map(|k| shown.value(dependent, k)).collect();
        let root_n = nodes[0].n;
        let terminals: Vec<usize> = (0..nodes.len())
            .filter(|&t| nodes[t].children.is_empty())
            .collect();
        let mut out = Vec::new();

        if command.print.model_summary {
            let mut used: Vec<usize> = nodes
                .iter()
                .filter_map(|n| n.split.as_ref().map(|s| s.predictor))
                .collect();
            used.sort_unstable();
            used.dedup();
            let names = |list: &mut dyn Iterator<Item = &Predictor>| {
                list.map(|p| p.name.clone()).collect::<Vec<_>>().join(", ")
            };
            let depth = nodes.iter().map(|n| n.depth).max().unwrap_or(0);
            let method = if command.exhaustive {
                "Exhaustive CHAID"
            } else {
                "CHAID"
            };
            let row = |group: &str, name: &str, value: Cell| vec![text(group), text(name), value];
            let mut rows = vec![
                row("Specifications", "Growing Method", text(method)),
                row("", "Dependent Variable", text(shown.variable(dependent))),
                row(
                    "",
                    "Independent Variables",
                    text(names(&mut predictors.iter())),
                ),
            ];
            if command.force.is_some() {
                rows.push(row("", "Forced Variable", text(&predictors[0].name)));
            }
            rows.extend([
                row("", "Validation", text("None")),
                row(
                    "",
                    "Maximum Tree Depth",
                    Cell::Number(command.max_depth.unwrap_or(3) as f64),
                ),
                row(
                    "",
                    "Minimum Cases in Parent Node",
                    Cell::Number(command.min_parent),
                ),
                row(
                    "",
                    "Minimum Cases in Child Node",
                    Cell::Number(command.min_child),
                ),
                row(
                    "Results",
                    "Independent Variables Included",
                    text(names(&mut used.iter().map(|&p| &predictors[p]))),
                ),
                row("", "Number of Nodes", Cell::Number(nodes.len() as f64)),
                row(
                    "",
                    "Number of Terminal Nodes",
                    Cell::Number(terminals.len() as f64),
                ),
                row("", "Depth", Cell::Number(depth as f64)),
            ]);
            out.push(Table {
                title: "Model Summary".into(),
                row_headers: 2,
                columns: vec![String::new(), String::new(), String::new()],
                column_groups: vec![Vec::new(); 3],
                rows,
                ..Table::default()
            });
        }

        let node_table = |title: &str, indent: bool| {
            let mut columns = vec!["Node".to_string()];
            let mut groups: Vec<Vec<String>> = vec![Vec::new()];
            let mut push = |group: &str, name: &str| {
                columns.push(name.to_string());
                groups.push(if group.is_empty() {
                    Vec::new()
                } else {
                    vec![group.to_string()]
                });
            };
            if !indent {
                push("", "Parent Node");
            }
            if command.node_definitions {
                push("", "Definition");
            }
            for class in &class_labels {
                push(class, "N");
                push(class, "Percent");
            }
            push("Total", "N");
            push("Total", "Percent");
            push("", "Predicted Category");
            if command.branch_statistics {
                push("Split", "Variable");
                push("Split", "Adj. Sig.");
                push("Split", "Chi-Square");
                push("Split", "df");
            }
            let mut order = Vec::new();
            if indent {
                // depth first, so each node's children follow it
                let mut stack = vec![0];
                while let Some(id) = stack.pop() {
                    order.push(id);
                    stack.extend(nodes[id].children.iter().rev());
                }
            } else {
                order.extend(0..nodes.len());
            }
            let mut rows = Vec::new();
            for id in order {
                let node = &nodes[id];
                let name = if indent {
                    format!("{}Node {id}", "    ".repeat(node.depth))
                } else {
                    format!("{id}")
                };
                let mut row = vec![text(name)];
                if !indent {
                    row.push(node.parent.map_or(Cell::Empty, |p| Cell::Number(p as f64)));
                }
                if command.node_definitions {
                    row.push(match &node.definition {
                        Some((p, group)) => text(format!(
                            "{} = {}",
                            predictors[*p].name,
                            predictors[*p].describe(group)
                        )),
                        None => Cell::Empty,
                    });
                }
                for &count in &node.counts {
                    row.push(Cell::Number(count));
                    row.push(percent(count, node.n));
                }
                row.push(Cell::Number(node.n));
                row.push(percent(node.n, root_n));
                row.push(text(&class_labels[node.predicted()]));
                if command.branch_statistics {
                    match &node.split {
                        Some(split) => {
                            row.push(text(&predictors[split.predictor].name));
                            row.push(Cell::Number(split.sig));
                            row.push(Cell::Number(split.statistic));
                            row.push(Cell::Number(split.df));
                        }
                        None => row.extend([Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]),
                    }
                }
                rows.push(row);
            }
            let format_row: Vec<String> = columns
                .iter()
                .map(|c| match c.as_str() {
                    "Percent" => "F5.1".to_string(),
                    "Adj. Sig." => "F5.3".to_string(),
                    "Chi-Square" => "F8.3".to_string(),
                    "N" | "df" => "F8.0".to_string(),
                    _ => String::new(),
                })
                .collect();
            let formats = vec![format_row; rows.len()];
            Table {
                title: title.to_string(),
                row_headers: 1,
                columns,
                column_groups: groups,
                rows,
                formats,
                caption: Some(format!(
                    "Dependent variable: {}.",
                    shown.variable(dependent)
                )),
            }
        };
        if command.display {
            // the diagram: every node with what its box shows, the branch condition above it
            // and the split of its children
            let mut diagram = Vec::new();
            for (id, node) in nodes.iter().enumerate() {
                let mut statistics: Vec<(String, Cell)> = Vec::new();
                for (class, &count) in class_labels.iter().zip(&node.counts) {
                    statistics.push((format!("{class} %"), percent(count, node.n)));
                    statistics.push((format!("{class} n"), Cell::Number(count)));
                }
                statistics.push(("Total %".into(), percent(node.n, root_n)));
                statistics.push(("Total n".into(), Cell::Number(node.n)));
                statistics.push(("Predicted".into(), text(&class_labels[node.predicted()])));
                if let Some(split) = &node.split {
                    statistics.push(("Split by".into(), text(&predictors[split.predictor].name)));
                    statistics.push(("Adj. Sig.".into(), Cell::Number(split.sig)));
                    statistics.push(("Chi-Square".into(), Cell::Number(split.statistic)));
                    statistics.push(("df".into(), Cell::Number(split.df)));
                }
                diagram.push(crate::plot::TreeNode {
                    id,
                    parent: node.parent,
                    split: match &node.definition {
                        Some((p, group)) => format!(
                            "{} = {}",
                            predictors[*p].name,
                            predictors[*p].describe(group)
                        ),
                        None => String::new(),
                    },
                    statistics,
                });
            }
            let mut chart = crate::Chart::plot(
                crate::chart::ChartKind::TreeDiagram,
                format!("Tree of {}", shown.variable(dependent)),
                crate::chart::Axis::titled(""),
                crate::chart::Axis::titled(""),
                crate::plot::Plot::Tree { nodes: diagram },
            );
            chart.footnote = Some(format!("Dependent variable: {}", shown.variable(dependent)));
            charts.push(chart);
        }
        if command.print.tree_table {
            out.push(node_table("Tree Table", false));
        }

        // misclassification
        let mut confusion = vec![vec![0.0; classes.len()]; classes.len()];
        for &t in &terminals {
            let predicted = nodes[t].predicted();
            for (j, &count) in nodes[t].counts.iter().enumerate() {
                confusion[j][predicted] += count;
            }
        }
        let correct: f64 = (0..classes.len()).map(|j| confusion[j][j]).sum();
        let risk = 1.0 - correct / root_n;
        if command.print.risk {
            out.push(Table {
                title: "Risk".into(),
                row_headers: 0,
                columns: vec!["Estimate".into(), "Std. Error".into()],
                column_groups: vec![Vec::new(); 2],
                rows: vec![vec![
                    Cell::Number(risk),
                    Cell::Number((risk * (1.0 - risk) / root_n).sqrt()),
                ]],
                formats: vec![vec!["F8.3".into(), "F8.3".into()]],
                caption: Some(format!(
                    "Growing Method: {}. Dependent Variable: {}.",
                    if command.exhaustive {
                        "Exhaustive CHAID"
                    } else {
                        "CHAID"
                    },
                    shown.variable(dependent)
                )),
            });
        }
        if command.print.classification {
            let mut columns = vec!["Observed".to_string()];
            columns.extend(class_labels.iter().cloned());
            columns.push("Percent Correct".into());
            let mut groups = vec![Vec::new()];
            groups.extend(vec![vec!["Predicted".to_string()]; classes.len() + 1]);
            let mut rows = Vec::new();
            for (j, row) in confusion.iter().enumerate() {
                let total: f64 = row.iter().sum();
                let mut cells = vec![text(&class_labels[j])];
                cells.extend(row.iter().map(|&n| Cell::Number(n)));
                cells.push(percent(row[j], total));
                rows.push(cells);
            }
            let mut overall = vec![text("Overall Percentage")];
            for k in 0..classes.len() {
                let predicted: f64 = confusion.iter().map(|r| r[k]).sum();
                overall.push(percent(predicted, root_n));
            }
            overall.push(percent(correct, root_n));
            let width = columns.len();
            let mut formats = vec![vec![String::new(); width]; rows.len()];
            for row in formats.iter_mut() {
                row[width - 1] = "PCT6.1".into();
            }
            formats.push(vec!["PCT6.1".to_string(); width]);
            rows.push(overall);
            out.push(Table {
                title: "Classification".into(),
                row_headers: 1,
                columns,
                column_groups: groups,
                rows,
                formats,
                caption: None,
            });
        }

        // gains for each target category
        if command.gain_table && !command.targets.is_empty() {
            for target in &command.targets {
                let key = key_of(target);
                let Some(j) = classes.iter().position(|k| *k == key) else {
                    continue;
                };
                let target_total = nodes[0].counts[j];
                let share = target_total / root_n;
                let index = |gain: f64, n: f64| {
                    if n > 0.0 && share > 0.0 {
                        Some(100.0 * gain / n / share)
                    } else {
                        None
                    }
                };
                let order = gain_order(nodes, &terminals, j, command.gain_ascending);
                let mut columns = vec!["Node".to_string()];
                let mut groups: Vec<Vec<String>> = vec![Vec::new()];
                let sets: &[&str] = if command.gain_cumulative {
                    &["", "Cumulative"]
                } else {
                    &[""]
                };
                for set in sets {
                    for (group, name) in [
                        ("Node", "N"),
                        ("Node", "Percent"),
                        ("Gain", "N"),
                        ("Gain", "Percent"),
                        ("", "Response"),
                        ("", "Index"),
                    ] {
                        columns.push(name.to_string());
                        let mut path = Vec::new();
                        if !set.is_empty() {
                            path.push(set.to_string());
                        }
                        if !group.is_empty() {
                            path.push(group.to_string());
                        } else if !path.is_empty() {
                            path.push(String::new());
                        }
                        groups.push(path);
                    }
                }
                let (mut cum_n, mut cum_gain) = (0.0, 0.0);
                let mut rows = Vec::new();
                for &t in &order {
                    let (n, gain) = (nodes[t].n, nodes[t].counts[j]);
                    cum_n += n;
                    cum_gain += gain;
                    let mut row = vec![text(t.to_string())];
                    let mut cells = |n: f64, gain: f64| {
                        row.extend([
                            Cell::Number(n),
                            percent(n, root_n),
                            Cell::Number(gain),
                            percent(gain, target_total),
                            percent(gain, n),
                            index(gain, n).map_or(Cell::Empty, Cell::Number),
                        ]);
                    };
                    cells(n, gain);
                    if command.gain_cumulative {
                        cells(cum_n, cum_gain);
                    }
                    rows.push(row);
                }
                let formats = rows
                    .iter()
                    .map(|row| {
                        columns
                            .iter()
                            .zip(row)
                            .map(|(c, _)| match c.as_str() {
                                "Percent" | "Response" | "Index" => "PCT6.1".to_string(),
                                "N" => "F8.0".to_string(),
                                _ => String::new(),
                            })
                            .collect()
                    })
                    .collect();
                out.push(Table {
                    title: "Gain Summary for Nodes".into(),
                    row_headers: 1,
                    columns,
                    column_groups: groups,
                    rows,
                    formats,
                    caption: Some(format!(
                        "Target category: {}. Dependent variable: {}.",
                        class_labels[j],
                        shown.variable(dependent)
                    )),
                });
            }
        }
        // the gain, response and index charts over percentile groups, a line per target
        let wanted = [
            (command.plot_gain, "Gain", 0),
            (command.plot_response, "Response", 1),
            (command.plot_index, "Index", 2),
        ];
        let targets: Vec<usize> = command
            .targets
            .iter()
            .filter_map(|t| classes.iter().position(|k| *k == key_of(t)))
            .collect();
        if wanted.iter().any(|w| w.0) && !targets.is_empty() {
            use crate::chart::{Axis, Category, PERCENT_FORMAT, Series, Statistic};
            let q = command.plot_increment;
            let categories: Vec<Category> = (q..=100)
                .step_by(q)
                .map(|p| Category {
                    label: format!("{p}%"),
                    value: Cell::Number(p as f64),
                    missing: false,
                    bin: None,
                })
                .collect();
            let lines: Vec<(usize, [Vec<f64>; 3])> = targets
                .iter()
                .map(|&j| {
                    let order = gain_order(nodes, &terminals, j, command.gain_ascending);
                    (j, percentile_gains(nodes, &order, j, q))
                })
                .collect();
            for (asked, name, k) in wanted {
                if !asked {
                    continue;
                }
                let series = lines
                    .iter()
                    .map(|(j, values)| Series {
                        name: class_labels[*j].clone(),
                        statistic: Statistic::Percent,
                        variable: Some(dependent.name().to_string()),
                        value: Cell::Empty,
                        values: values[k].clone(),
                    })
                    .collect();
                let mut chart = crate::Chart::lines(
                    format!("{name} chart"),
                    Axis::titled("Percentile"),
                    Axis {
                        format: PERCENT_FORMAT.into(),
                        ..Axis::titled(format!("{name} (%)"))
                    },
                    categories.clone(),
                    series,
                );
                chart.legend = Some(shown.variable(dependent));
                chart.footnote = Some(format!(
                    "Dependent variable: {}. Cumulative over percentile groups of {q}%",
                    shown.variable(dependent)
                ));
                charts.push(chart);
            }
        }
        Ok(out)
    }
}

/// The terminal nodes in the order of the gain tables: by the index of target class `j`,
/// highest first unless `ascending`.
fn gain_order(nodes: &[Node], terminals: &[usize], j: usize, ascending: bool) -> Vec<usize> {
    let share = nodes[0].counts[j] / nodes[0].n;
    let index = |t: usize| match nodes[t].n > 0.0 && share > 0.0 {
        true => nodes[t].counts[j] / nodes[t].n / share,
        false => f64::NAN,
    };
    let mut order = terminals.to_vec();
    order.sort_by(|&a, &b| match ascending {
        true => index(a).total_cmp(&index(b)),
        false => index(b).total_cmp(&index(a)),
    });
    order
}

/// The percentile gain summary of target class `j` (Algorithms manual, TREE): for every
/// percentile p = q, 2q, ..., 100 the cumulative gain %, response % and index %, where the
/// node that straddles the cut counts in proportion to the part of it below the cut.
fn percentile_gains(nodes: &[Node], order: &[usize], j: usize, q: usize) -> [Vec<f64>; 3] {
    let (total, target) = (nodes[0].n, nodes[0].counts[j]);
    let mut out = [Vec::new(), Vec::new(), Vec::new()];
    for p in (q..=100).step_by(q) {
        let cut = total * p as f64 / 100.0;
        let (mut before_n, mut before_gain) = (0.0, 0.0);
        let mut gain = before_gain;
        for (k, &t) in order.iter().enumerate() {
            let (n, g) = (nodes[t].n, nodes[t].counts[j]);
            // the last node takes what rounding leaves at 100%
            if before_n + n >= cut * (1.0 - 1e-12) || k + 1 == order.len() {
                gain = before_gain + (cut - before_n) / n * g;
                break;
            }
            before_n += n;
            before_gain += g;
        }
        let response = 100.0 * gain / cut;
        out[0].push(100.0 * gain / target);
        out[1].push(response);
        out[2].push(100.0 * response / (100.0 * target / total));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bonferroni_counts() {
        // S(4, 2) = 7, S(5, 3) = 25
        assert_eq!(stirling(4, 2), 7.0);
        assert_eq!(stirling(5, 3), 25.0);
        assert_eq!(stirling(3, 3), 1.0);
        assert_eq!(binomial(4.0, 2.0), 6.0);
        assert_eq!(binomial(3.0, -1.0), 0.0);
    }

    #[test]
    fn break_points_follow_the_ranks() {
        // ten values 1..10 in 3 groups: floor(r * 3 / 11) is 0 for r 1..3, 1 for 4..7, 2 for 8..10
        let values: Vec<(f64, f64)> = (1..=10).map(|v| (v as f64, 1.0)).collect();
        assert_eq!(break_points(values, 3), [3.0, 7.0]);
        // ties share their average rank: 1 1 1 1 2 3 with weights, 2 groups
        let values = vec![(1.0, 4.0), (2.0, 1.0), (3.0, 1.0)];
        // ranks 2.5, 5, 6 of N = 6: floor(r * 2 / 7) = 0, 1, 1
        assert_eq!(break_points(values, 2), [1.0]);
    }
}
