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

mod crt;
mod gain;
mod quest;
mod scale;

use crate::crosstabs::Key;
use crate::descriptives::case_weights;
use crate::dist::{chi_square_p, f_p};
use crate::format::category_text;
use crate::pass::default_measure;
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Value, Values, Variable};
use statscraft_syntax::{
    Level, Tree, TreeCategory, TreeImpurity, TreePriors, TreeSave, TreeValidation, VarItem,
};
use std::collections::VecDeque;
use std::rc::Rc;

/// One predictor, its categories coded per analysis case; `missing` (= the number of
/// categories) is the code of a missing value.
struct Predictor {
    name: String,
    nominal: bool,
    labels: Vec<String>,
    /// The break points of a scale predictor, in its format.
    breaks: Option<Vec<String>>,
    codes: Vec<usize>,
    /// The value of each category of a scale or ordinal predictor, for QUEST; the rank for a
    /// category that is no number. Empty for the others.
    numbers: Vec<f64>,
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
    /// The degrees of freedom within the groups, for the F test of a scale dependent.
    df2: Option<f64>,
    sig: f64,
    /// The decrease in impurity of a CRT split; CHAID has the three above instead.
    improvement: Option<f64>,
    /// The splits of other predictors that best imitate a CRT split, most similar first.
    surrogates: Vec<Surrogate>,
    /// What QUEST chose the split by.
    quest: Option<QuestSplit>,
}

/// The test that picked a QUEST split, and the cut of a scale or ordinal predictor.
struct QuestSplit {
    /// F, Levene F or Chi-Square.
    test: &'static str,
    statistic: f64,
    df: f64,
    /// The degrees of freedom within the classes, for an F.
    df2: Option<f64>,
    sig: f64,
    /// The split point d of x <= d; not a number for a nominal predictor.
    cut: f64,
}

/// A split of `predictor` that sends the categories `left` to the left child.
struct Surrogate {
    predictor: usize,
    left: Vec<usize>,
    /// The predictive measure of association with the primary split.
    association: f64,
    /// The improvement the split makes on its own.
    improvement: f64,
}

/// The cost of assigning class i (row) to a case of class j (column); zero on the diagonal.
type Costs = Rc<Vec<Vec<f64>>>;

struct Node {
    parent: Option<usize>,
    depth: usize,
    cases: Vec<usize>,
    counts: Vec<f64>,
    /// The probability of each class in the node, up to a factor all classes share: the count
    /// times prior over class total. For CHAID it is the count.
    assign: Vec<f64>,
    /// A scale dependent's node: `counts` are then the weight, the sum of the values and the sum
    /// of their squares, and `assign` is the node's risk.
    scale: bool,
    /// A scale dependent's values were shifted by this much before adding them up.
    shift: f64,
    /// The weighted sum of the fourth powers of the deviations from the mean, for the risk's error.
    fourth: f64,
    /// A test sample's node of a scale dependent: its sum of squares about the training node's
    /// mean (the node's own mean otherwise).
    squares_about: Option<f64>,
    n: f64,
    /// The class the node is assigned when that is not the most probable one: the cheapest one
    /// under custom costs, or a test sample's node's, the training node's.
    predicted_as: Option<usize>,
    /// Custom misclassification costs, None when every one is 1.
    costs: Option<Costs>,
    /// The predictor and its categories that define the node.
    definition: Option<(usize, Vec<usize>)>,
    split: Option<Split>,
    children: Vec<usize>,
}

impl Node {
    /// The assigned class: the most probable one, the first among those present on ties.
    fn predicted(&self) -> usize {
        if let Some(class) = self.predicted_as {
            return class;
        }
        let mut best = self.counts.iter().position(|&c| c > 0.0).unwrap_or(0);
        for (j, &score) in self.assign.iter().enumerate() {
            if self.counts[j] > 0.0 && score > self.assign[best] {
                best = j;
            }
        }
        best
    }

    /// The probability of class `j` in the node.
    fn probability(&self, j: usize) -> f64 {
        self.assign[j] / self.assign.iter().sum::<f64>()
    }
}

/// The class with the smallest expected cost, the sum over the classes j of the cost of assigning
/// it to a case of class j times the probability of j in the node. Among classes within rounding of
/// the smallest, the lowest one with cases in the node, else the lowest.
fn cheapest(assign: &[f64], counts: &[f64], costs: &[Vec<f64>]) -> usize {
    let expected: Vec<f64> = costs
        .iter()
        .map(|row| row.iter().zip(assign).map(|(c, p)| c * p).sum())
        .collect();
    let least = expected.iter().copied().fold(f64::INFINITY, f64::min);
    let tied = |j: usize| expected[j] <= least + 1e-12 * least.abs().max(f64::MIN_POSITIVE);
    (0..expected.len())
        .find(|&j| tied(j) && counts[j] > 0.0)
        .or_else(|| (0..expected.len()).find(|&j| tied(j)))
        .unwrap_or(0)
}

/// A node holding `cases`; `factor` turns a class count into the probability's numerator. The
/// sample is the class and weight of each case, and the values of a scale dependent, whose node
/// holds the weight, the sum of the values and of their squares in place of class counts.
fn make_node(
    cases: Vec<usize>,
    parent: Option<usize>,
    depth: usize,
    definition: Option<(usize, Vec<usize>)>,
    sample: (&[usize], &[f64], Option<&[f64]>),
    factor: &[f64],
    costs: Option<&Costs>,
) -> Node {
    let (class, weight, values) = sample;
    let mut counts = vec![0.0; if values.is_some() { 3 } else { factor.len() }];
    for &i in &cases {
        match values {
            Some(y) => {
                counts[0] += weight[i];
                counts[1] += weight[i] * y[i];
                counts[2] += weight[i] * y[i] * y[i];
            }
            None => counts[class[i]] += weight[i],
        }
    }
    let (n, assign, fourth) = match values {
        Some(y) => {
            let mean = counts[1] / counts[0];
            let fourth = cases
                .iter()
                .map(|&i| weight[i] * (y[i] - mean).powi(4))
                .sum();
            // the node's risk: its sum of squares over the weight of all cases
            let squares = counts[2] - counts[1] * counts[1] / counts[0];
            (counts[0], vec![squares.max(0.0) * factor[0]], fourth)
        }
        None => (
            counts.iter().sum(),
            counts.iter().zip(factor).map(|(c, f)| c * f).collect(),
            0.0,
        ),
    };
    let predicted_as = costs
        .filter(|_| values.is_none())
        .map(|c| cheapest(&assign, &counts, c));
    Node {
        parent,
        depth,
        n,
        assign,
        predicted_as,
        costs: costs.cloned(),
        scale: values.is_some(),
        shift: 0.0,
        fourth,
        squares_about: None,
        cases,
        counts,
        definition,
        split: None,
        children: Vec::new(),
    }
}

/// The nodes of a tree with the cases of another sample counted in them: the structure, the
/// assignments (the training sample's) and the prior weights are kept, the class counts are those
/// of `weight`. For a scale dependent (`values`) the node holds the weight, sum and sum of squares
/// of the values, and the sum of squares and of fourth powers about the training node's mean.
fn rescore(nodes: &[Node], class: &[usize], weight: &[f64], values: Option<&[f64]>) -> Vec<Node> {
    nodes
        .iter()
        .map(|node| {
            let mut counts = vec![0.0; node.counts.len()];
            let (mut squares, mut fourth) = (None, 0.0);
            if let Some(y) = values {
                let centre = node.counts[1] / node.counts[0];
                let (mut about, mut quartic) = (0.0, 0.0);
                for &i in &node.cases {
                    let (w, d) = (weight[i], y[i] - centre);
                    counts[0] += w;
                    counts[1] += w * y[i];
                    counts[2] += w * y[i] * y[i];
                    about += w * d * d;
                    quartic += w * d.powi(4);
                }
                (squares, fourth) = (Some(about), quartic);
            } else {
                for &i in &node.cases {
                    counts[class[i]] += weight[i];
                }
            }
            Node {
                parent: node.parent,
                depth: node.depth,
                cases: node.cases.clone(),
                n: if values.is_some() {
                    counts[0]
                } else {
                    counts.iter().sum()
                },
                counts,
                assign: node.assign.clone(),
                scale: node.scale,
                shift: node.shift,
                fourth,
                squares_about: squares,
                predicted_as: (values.is_none()).then(|| node.predicted()),
                costs: node.costs.clone(),
                definition: node.definition.clone(),
                split: None,
                children: node.children.clone(),
            }
        })
        .collect()
}

/// What the tree was checked against besides the cases it grew on.
#[derive(Clone, Copy, Default)]
struct Checks<'a> {
    /// The tree's nodes counting the test sample's cases.
    test: Option<&'a [Node]>,
    /// The cross-validated risk and its variance.
    cross: Option<(f64, f64)>,
}

/// What the tests of independence need.
struct Tester<'a> {
    /// A scale dependent: the table rows hold the weight, the sum of the values and of their
    /// squares, and the test is the ANOVA F test across the rows.
    scale: bool,
    /// Scores of the classes of an ordinal dependent; None for a nominal one.
    scores: Option<&'a [f64]>,
    likelihood_ratio: bool,
    converge: f64,
    max_iterations: usize,
}

impl Tester<'_> {
    /// The weight of the cases in a row of the table.
    fn size(&self, row: &[f64]) -> f64 {
        if self.scale { row[0] } else { row.iter().sum() }
    }

    /// The ANOVA F test of the means of the dependent across the rows (groups): the F statistic,
    /// its degrees of freedom between, and its significance. Empty rows are left out.
    fn anova(&self, table: &[Vec<f64>]) -> (f64, f64, f64) {
        let rows: Vec<&Vec<f64>> = table.iter().filter(|r| r[0] > 0.0).collect();
        let total: f64 = rows.iter().map(|r| r[0]).sum();
        let (dfn, dfd) = (rows.len() as f64 - 1.0, total - rows.len() as f64);
        if rows.len() < 2 || dfd <= 0.0 {
            return (0.0, 0.0, 1.0);
        }
        let sum: f64 = rows.iter().map(|r| r[1]).sum();
        let by_group: f64 = rows.iter().map(|r| r[1] * r[1] / r[0]).sum();
        let between = (by_group - sum * sum / total).max(0.0);
        let within = (rows.iter().map(|r| r[2]).sum::<f64>() - by_group).max(0.0);
        if between <= 0.0 {
            return (0.0, dfn, 1.0);
        }
        if within <= 1e-12 * between {
            return (f64::INFINITY, dfn, 0.0);
        }
        let f = (between / dfn) / (within / dfd);
        (f, dfn, f_p(f, dfn, dfd))
    }

    /// Statistic, degrees of freedom and significance of a table with groups as rows and
    /// classes as columns. Empty rows and columns are left out.
    fn test(&self, table: &[Vec<f64>]) -> (f64, f64, f64) {
        if self.scale {
            return self.anova(table);
        }
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
        let size = |c: &usize| self.tester.size(&table[*c]);
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
                .map(|(i, g)| (i, self.tester.size(&row_sum(table, g))))
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

/// The value of each category of a scale or ordinal predictor: the number it is, else its rank.
fn numbers_of(keys: &[Key]) -> Vec<f64> {
    keys.iter()
        .enumerate()
        .map(|(rank, k)| match k {
            Key::Number(x) => *x,
            _ => rank as f64,
        })
        .collect()
}

fn sorted_unique(mut keys: Vec<Key>) -> Vec<Key> {
    keys.sort_by(|a, b| a.compare(b));
    keys.dedup();
    keys
}

/// The measurement level of a variable: the one written, else the dictionary's, else the one
/// its data give it (the rule of a data pass, as the manual says for an unknown level, with SET
/// SCALEMIN as `scale_min`).
pub(crate) fn level_of(
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
        if dependent_level == Level::Scale && dependent.is_string() {
            warnings.push(format!(
                "the string variable {} cannot be scale; it is nominal",
                dependent.name()
            ));
        }
        let scale_dep = dependent_level == Level::Scale && !dependent.is_string();
        let ordinal = dependent_level == Level::Ordinal;
        if command.quest && (scale_dep || ordinal) {
            return Err(Error(format!(
                "METHOD TYPE=QUEST needs a nominal dependent variable; {} is not (give it the \
                 level as in {} [n])",
                dependent.name(),
                dependent.name()
            )));
        }
        if command.crt && command.impurity == TreeImpurity::OrderedTwoing && !ordinal {
            return Err(Error(format!(
                "IMPURITY=ORDEREDTWOING needs an ordinal dependent variable; {} is not",
                dependent.name()
            )));
        }
        if command.binary() && command.force.is_some() {
            warnings.push("FORCE applies to CHAID trees only; ignored".into());
        }
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
        let classes: Vec<Key> = if scale_dep {
            // one class of all cases; the dependent's values are used as they are
            vec![Key::Number(0.0)]
        } else {
            match &command.use_values {
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
            }
        };
        if classes.len() < 2 && !scale_dep {
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
        if scale_dep {
            let message =
                |what: &str| format!("{what} needs a categorical dependent variable; ignored");
            if !command.targets.is_empty() {
                warnings.push(message("DEPCATEGORIES TARGET"));
            }
            if command.priors != TreePriors::FromData {
                warnings.push(message("PRIORS"));
            }
            if command.costs.is_some() {
                warnings.push(message("COSTS"));
            }
            // the CHAID settings of the chi-square tests have nothing to act on
            if !command.binary() {
                if command.likelihood_ratio {
                    warnings.push("CHAID CHISQUARE applies to a nominal dependent; ignored".into());
                }
                if command.converge != 0.001 || command.max_iterations != 100 {
                    warnings.push(
                        "CHAID CONVERGE and MAXITERATIONS apply to an ordinal dependent; ignored"
                            .into(),
                    );
                }
            }
            if command.plot_gain || command.plot_response || command.plot_index {
                warnings.push(message("PLOT GAIN, RESPONSE and INDEX"));
            }
            if command
                .save
                .iter()
                .any(|(what, _)| *what == TreeSave::PredictedProbability)
            {
                warnings.push(message("SAVE PREDPROB"));
            }
        }
        for target in command.targets.iter().filter(|_| !scale_dep) {
            let key = key_of(target);
            if !classes.contains(&key) {
                warnings.push(format!(
                    "target category {} is not a category of the analysis; it has no gain table",
                    labels.value(&dependent, &key)
                ));
            }
        }
        let charted = command.plot_gain || command.plot_response || command.plot_index;
        if charted && !scale_dep && !command.targets.iter().any(|t| classes.contains(&key_of(t))) {
            warnings.push(
                "PLOT GAIN, RESPONSE and INDEX need target categories (DEPCATEGORIES TARGET); no chart is made"
                    .into(),
            );
        }
        let class_of = |c: usize| match raw(&dependent, c) {
            Raw::Valid(_) if scale_dep => Some(0),
            Raw::UserMissing(_) if scale_dep => None,
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
        // the cases that grow the tree; the others are the test sample, carried along with weight
        // 0 so that they are routed like the rest without counting in any statistic
        let is_train: Vec<bool> = match &command.validation {
            TreeValidation::None | TreeValidation::Cross(_) => vec![true; cases.len()],
            TreeValidation::SplitPercent(percent) => cases
                .iter()
                .map(|_| self.random.borrow_mut().uniform() < percent / 100.0)
                .collect(),
            TreeValidation::SplitVariable(name) => {
                let split = data.variable(name)?;
                if split.is_string() {
                    return Err(Error(format!(
                        "the SPLITSAMPLE variable {} must be numeric",
                        split.name()
                    )));
                }
                let taken = |other: &str| split.name().eq_ignore_ascii_case(other);
                if taken(dependent.name())
                    || weight_name.as_ref().is_some_and(|w| taken(w))
                    || command.force.as_ref().is_some_and(|(f, _)| taken(f))
                {
                    return Err(Error(format!(
                        "the SPLITSAMPLE variable {} cannot be the dependent, weight or forced variable",
                        split.name()
                    )));
                }
                // a value of 1 grows the tree; anything else, a missing value too, tests it
                cases
                    .iter()
                    .map(|&(c, _)| matches!(raw(&split, c), Raw::Valid(Key::Number(x)) if x == 1.0))
                    .collect()
            }
        };
        // a test sample is held out, as opposed to cross-validation, which tests every case once
        let validated = matches!(
            command.validation,
            TreeValidation::SplitPercent(_) | TreeValidation::SplitVariable(_)
        );
        let all_weight: Vec<f64> = cases.iter().map(|&(c, _)| weight(c)).collect();
        let case_weight: Vec<f64> = all_weight
            .iter()
            .zip(&is_train)
            .map(|(&w, &train)| if train { w } else { 0.0 })
            .collect();
        let test_weight: Vec<f64> = all_weight
            .iter()
            .zip(&is_train)
            .map(|(&w, &train)| if train { 0.0 } else { w })
            .collect();
        let case_class: Vec<usize> = cases.iter().map(|&(_, k)| k).collect();
        if validated {
            let mut trained = vec![0.0; classes.len()];
            for (&j, &w) in case_class.iter().zip(&case_weight) {
                trained[j] += w;
            }
            if test_weight.iter().sum::<f64>() <= 0.0 {
                return Err(Error("the test sample has no cases".into()));
            }
            if !scale_dep && trained.iter().filter(|&&w| w > 0.0).count() < 2 {
                return Err(Error(format!(
                    "the training sample has fewer than two categories of {}",
                    dependent.name()
                )));
            }
            for (j, _) in trained
                .iter()
                .enumerate()
                .filter(|(_, w)| **w == 0.0 && !scale_dep)
            {
                warnings.push(format!(
                    "category {} of {} has no cases in the training sample",
                    labels.value(&dependent, &classes[j]),
                    dependent.name()
                ));
            }
        }

        // the predictors coded from the cases `train` marks (the categories and break points are
        // theirs); QUEST scores a nominal one on them with `weights`
        let code = |train: &[bool], weights: &[f64]| -> Result<Vec<Predictor>, Error> {
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
                let predictor = if *level == Level::Scale && command.binary() {
                    // every distinct value is a candidate cut, so each is a category of its own
                    let keys = sorted_unique(values.iter().flatten().cloned().collect());
                    let codes = values
                        .iter()
                        .map(|v| match v {
                            Some(k) => keys.iter().position(|x| x == k).expect("a key of the data"),
                            None => keys.len(),
                        })
                        .collect();
                    let cuts: Vec<String> = keys
                        .iter()
                        .take(keys.len().saturating_sub(1))
                        .map(|k| shown.value(variable, k))
                        .collect();
                    Predictor {
                        name: shown.variable(variable),
                        nominal: false,
                        labels: vec![String::new(); keys.len()],
                        breaks: Some(cuts),
                        codes,
                        numbers: numbers_of(&keys),
                    }
                } else if *level == Level::Scale {
                    let k = intervals
                        .iter()
                        .find(|(n, _)| n.eq_ignore_ascii_case(variable.name()))
                        .map_or(command.intervals, |(_, k)| *k);
                    let pairs: Vec<(f64, f64)> = values
                        .iter()
                        .zip(&cases)
                        .zip(train)
                        .filter_map(|((v, &(c, _)), &train)| match v {
                            Some(Key::Number(x)) if train => Some((*x, weight(c))),
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
                        numbers: Vec::new(),
                    }
                } else {
                    // the categories are those of the training cases; another one is missing
                    let keys = sorted_unique(
                        values
                            .iter()
                            .zip(train)
                            .filter(|(_, train)| **train)
                            .filter_map(|(v, _)| v.clone())
                            .collect(),
                    );
                    if command.quest
                        && *level == Level::Nominal
                        && keys.len() > quest::MAX_CATEGORIES
                    {
                        return Err(Error(format!(
                            "METHOD TYPE=QUEST allows a nominal predictor at most {} categories; {} has {}",
                            quest::MAX_CATEGORIES,
                            variable.name(),
                            keys.len()
                        )));
                    }
                    if keys.len() > 126 {
                        return Err(Error(format!(
                            "the predictor {} has more than 126 categories",
                            variable.name()
                        )));
                    }
                    let codes = values
                        .iter()
                        .map(|v| match v {
                            Some(k) => keys.iter().position(|x| x == k).unwrap_or(keys.len()),
                            None => keys.len(),
                        })
                        .collect();
                    Predictor {
                        name: shown.variable(variable),
                        nominal: *level == Level::Nominal,
                        labels: keys.iter().map(|k| shown.value(variable, k)).collect(),
                        breaks: None,
                        codes,
                        numbers: if *level == Level::Nominal {
                            Vec::new()
                        } else {
                            numbers_of(&keys)
                        },
                    }
                };
                coded.push(predictor);
            }
            if command.quest {
                // a nominal predictor of more than two categories is scored once, on the whole sample
                for p in coded.iter_mut().filter(|p| p.nominal && p.labels.len() > 2) {
                    p.numbers = quest::coordinates(
                        &p.codes,
                        p.labels.len(),
                        &case_class,
                        weights,
                        classes.len(),
                    );
                }
            }
            Ok(coded)
        };
        let coded = code(&is_train, &case_weight)?;
        // a scale dependent is shifted by its mean, so sums of squares do not cancel
        let (dep_values, dep_shift): (Option<Vec<f64>>, f64) = if scale_dep {
            let values: Vec<f64> = cases
                .iter()
                .map(|&(c, _)| match raw(&dependent, c) {
                    Raw::Valid(Key::Number(x)) => x,
                    _ => f64::NAN,
                })
                .collect();
            let total: f64 = case_weight.iter().sum();
            let shift = values
                .iter()
                .zip(&case_weight)
                .map(|(y, w)| y * w)
                .sum::<f64>()
                / total;
            (Some(values.iter().map(|y| y - shift).collect()), shift)
        } else {
            (None, 0.0)
        };
        let costs = (!scale_dep)
            .then(|| custom_costs(command, &classes, &labels, &dependent, &mut warnings))
            .flatten();
        let priors = if !command.binary() {
            Vec::new()
        } else if scale_dep {
            vec![1.0]
        } else {
            crt_priors(
                command,
                &classes,
                &case_class,
                &case_weight,
                &labels,
                &dependent,
                &mut warnings,
            )?
        };
        // altered priors: with ADJUST=YES the costs go into the priors, C(j) times the prior of each
        // category j over the sum of those, and the problem is then taken as one without costs
        let (priors, costs) = match &costs {
            Some(matrix) if command.adjust_priors && command.binary() => {
                let altered: Vec<f64> = priors
                    .iter()
                    .enumerate()
                    .map(|(j, p)| p * matrix.iter().map(|row| row[j]).sum::<f64>())
                    .collect();
                let total: f64 = altered.iter().sum();
                if total <= 0.0 {
                    return Err(Error(
                        "the prior probabilities adjusted by the costs sum to zero".into(),
                    ));
                }
                (altered.iter().map(|p| p / total).collect(), None)
            }
            _ => (priors, costs),
        };
        let grow = |coded: &[Predictor],
                    weights: &[f64],
                    held_out: Option<&[f64]>|
         -> Result<Vec<Node>, Error> {
            Ok(if command.binary() {
                let grown = crt::Growth {
                    predictors: coded,
                    case_class: &case_class,
                    case_weight: weights,
                    values: dep_values.as_deref(),
                    shift: dep_shift,
                    classes: classes.len(),
                    priors: &priors,
                    // QUEST ranks no split by impurity; surrogates still do, by Gini
                    impurity: if command.quest {
                        TreeImpurity::Gini
                    } else {
                        command.impurity
                    },
                    costs: costs.clone(),
                    quest: command.quest.then_some(command.quest_alpha),
                    min_improvement: command.min_improvement.unwrap_or(0.0001),
                    min_parent: command.min_parent,
                    min_child: command.min_child,
                    max_depth: depth_limit(command),
                    max_surrogates: command.max_surrogates,
                }
                .grow()?;
                match command.prune {
                    Some(b) => {
                        // with a test sample, the subtrees' risks are the test sample's
                        let tested = held_out
                            .map(|test| rescore(&grown, &case_class, test, dep_values.as_deref()));
                        crt::prune(grown, b, tested.as_deref(), empirical_priors(command))
                    }
                    None => grown,
                }
            } else {
                let merging = Merging {
                    tester: Tester {
                        scale: dep_values.is_some(),
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
                let max_depth = depth_limit(command);
                // a scale dependent: a node's statistics are its weight, sum of values and of squares
                let scale = dep_values.is_some();
                let ones = if scale {
                    vec![1.0 / weights.iter().sum::<f64>()]
                } else {
                    vec![1.0; classes.len()]
                };
                let node_of = |cases: Vec<usize>, parent, depth, definition| {
                    let mut node = make_node(
                        cases,
                        parent,
                        depth,
                        definition,
                        (&case_class, weights, dep_values.as_deref()),
                        &ones,
                        costs.as_ref(),
                    );
                    node.shift = dep_shift;
                    node
                };
                let mut nodes = vec![node_of((0..cases.len()).collect(), None, 0, None)];
                let mut queue = VecDeque::from([0]);
                while let Some(id) = queue.pop_front() {
                    let node = &nodes[id];
                    let pure = if scale {
                        // every value alike, up to rounding
                        let [w, s, q] = [node.counts[0], node.counts[1], node.counts[2]];
                        q * w - s * s <= 1e-12 * q * w
                    } else {
                        node.counts.iter().filter(|&&n| n > 0.0).count() < 2
                    };
                    if node.depth >= max_depth || node.n < command.min_parent || pure {
                        continue;
                    }
                    let mut best: Option<Split> = None;
                    for (p, predictor) in coded.iter().enumerate() {
                        if id == 0 && command.force.is_some() && p != 0 {
                            continue;
                        }
                        let width = if scale { 3 } else { classes.len() };
                        let mut table = vec![vec![0.0; width]; predictor.labels.len() + 1];
                        for &i in &node.cases {
                            let row = &mut table[predictor.codes[i]];
                            match &dep_values {
                                Some(y) => {
                                    row[0] += weights[i];
                                    row[1] += weights[i] * y[i];
                                    row[2] += weights[i] * y[i] * y[i];
                                }
                                None => row[case_class[i]] += weights[i],
                            }
                        }
                        let Some((groups, statistic, df, sig)) = merging.merge(predictor, &table)
                        else {
                            continue;
                        };
                        // the degrees of freedom within the groups of the F test
                        let df2 = scale.then(|| {
                            let total: f64 = groups.iter().map(|g| row_sum(&table, g)[0]).sum();
                            total - groups.len() as f64
                        });
                        if best.as_ref().is_none_or(|b| sig < b.sig) {
                            best = Some(Split {
                                predictor: p,
                                groups,
                                statistic,
                                df,
                                df2,
                                sig,
                                improvement: None,
                                surrogates: Vec::new(),
                                quest: None,
                            });
                        }
                    }
                    let forced = id == 0 && command.force.is_some();
                    let Some(split) = best.filter(|s| forced || s.sig <= command.alpha_split)
                    else {
                        continue;
                    };
                    let predictor = &coded[split.predictor];
                    let mut members: Vec<Vec<usize>> = split
                        .groups
                        .iter()
                        .map(|group| {
                            nodes[id]
                                .cases
                                .iter()
                                .copied()
                                .filter(|&i| group.contains(&predictor.codes[i]))
                                .collect()
                        })
                        .collect();
                    // a case whose category no training case of the node has (a test case) goes to
                    // the group with the nearest category of an ordered predictor, else to the group
                    // with the most weight
                    let missing = predictor.missing();
                    for &i in &nodes[id].cases {
                        let code = predictor.codes[i];
                        if split.groups.iter().any(|g| g.contains(&code)) {
                            continue;
                        }
                        let target = if !predictor.nominal && code != missing {
                            (0..split.groups.len()).min_by_key(|&g| {
                                let near = split.groups[g].iter().filter(|&&c| c != missing);
                                near.map(|&c| c.abs_diff(code)).min().unwrap_or(usize::MAX)
                            })
                        } else {
                            let weight = |g: &usize| -> f64 {
                                members[*g].iter().map(|&k| weights[k]).sum()
                            };
                            (0..split.groups.len())
                                .reduce(|a, b| if weight(&b) > weight(&a) { b } else { a })
                        };
                        if let Some(g) = target {
                            members[g].push(i);
                        }
                    }
                    let depth = nodes[id].depth + 1;
                    let children: Vec<Node> = members
                        .into_iter()
                        .zip(&split.groups)
                        .map(|(cases, group)| {
                            node_of(
                                cases,
                                Some(id),
                                depth,
                                Some((split.predictor, group.clone())),
                            )
                        })
                        .collect();
                    for child in children {
                        let next = nodes.len();
                        nodes[id].children.push(next);
                        queue.push_back(nodes.len());
                        nodes.push(child);
                    }
                    nodes[id].split = Some(split);
                }

                nodes
            })
        };
        let nodes = grow(&coded, &case_weight, validated.then_some(&test_weight[..]))?;

        let tested =
            validated.then(|| rescore(&nodes, &case_class, &test_weight, dep_values.as_deref()));
        // cross-validation: a tree grown without each fold of the cases, scored on that fold, and
        // the weights of the cases each fold's tree misclassifies added up
        let cross = match command.validation {
            TreeValidation::Cross(folds) => {
                let mut keys: Vec<(f64, usize)> = (0..cases.len())
                    .map(|i| (self.random.borrow_mut().uniform(), i))
                    .collect();
                keys.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut fold = vec![0; cases.len()];
                for (rank, &(_, i)) in keys.iter().enumerate() {
                    fold[i] = rank % folds;
                }
                let (mut loss, mut loss_squared) =
                    (vec![0.0; classes.len()], vec![0.0; classes.len()]);
                let (mut squares, mut fourth) = (0.0, 0.0);
                for f in 0..folds {
                    let weights = |keep: bool| -> Vec<f64> {
                        case_weight
                            .iter()
                            .zip(&fold)
                            .map(|(&w, &k)| if (k == f) == keep { w } else { 0.0 })
                            .collect()
                    };
                    let (learning, held) = (weights(false), weights(true));
                    let learning_cases: Vec<bool> = fold.iter().map(|&k| k != f).collect();
                    let fold_coded = code(&learning_cases, &learning)?;
                    let grown = grow(&fold_coded, &learning, None)?;
                    let scored = rescore(&grown, &case_class, &held, dep_values.as_deref());
                    for node in scored.iter().filter(|n| n.children.is_empty()) {
                        if scale_dep {
                            squares += node.squares_about.unwrap_or(0.0);
                            fourth += node.fourth;
                            continue;
                        }
                        add_losses(node, &mut loss, &mut loss_squared);
                    }
                }
                Some(if scale_dep {
                    // the mean squared deviation of each case from its fold tree's node mean
                    let n = nodes[0].n;
                    let risk = squares / n;
                    (risk, ((fourth - n * risk * risk) / (n * n)).max(0.0))
                } else {
                    misclassification_risk(
                        &loss,
                        &loss_squared,
                        &nodes[0],
                        Some(empirical_priors(command)),
                    )
                })
            }
            _ => None,
        };
        let mut charts = Vec::new();
        let mut tables = self.tree_output(
            command,
            (
                &nodes,
                Checks {
                    test: tested.as_deref(),
                    cross,
                },
            ),
            &coded,
            &classes,
            &dependent,
            &mut charts,
        )?;
        let total_n: f64 = in_use.iter().map(|&c| weight(c)).sum();
        if command.print.case_summary {
            let included = nodes[0].n + tested.as_ref().map_or(0.0, |t| t[0].n);
            let mut rows = vec![vec![
                text("Included"),
                Cell::Number(included),
                percent(included, total_n),
            ]];
            if let Some(test) = &tested {
                for (name, n) in [("Training sample", nodes[0].n), ("Test sample", test[0].n)] {
                    rows.push(vec![text(name), Cell::Number(n), percent(n, total_n)]);
                }
            }
            rows.push(vec![
                text("Excluded"),
                Cell::Number(total_n - included),
                percent(total_n - included, total_n),
            ]);
            rows.push(vec![
                text("Total"),
                Cell::Number(total_n),
                percent(total_n, total_n),
            ]);
            let rows_len = rows.len();
            tables.insert(
                0,
                Table {
                    title: "Case Processing Summary".into(),
                    row_headers: 1,
                    columns: vec![String::new(), "N".into(), "Percent".into()],
                    column_groups: vec![Vec::new(); 3],
                    rows,
                    formats: vec![vec![String::new(), "F8.0".into(), "PCT6.1".into()]; rows_len],
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
                    TreeSave::PredictedValue if scale_dep => {
                        // the mean of the dependent in the case's terminal node
                        let name = fresh(data, given, "PredictedValue")?;
                        let mut values = vec![None; case_count];
                        for (i, &c) in case_numbers.iter().enumerate() {
                            let node = &nodes[terminal_of[i]];
                            values[c] = Some(node.counts[1] / node.counts[0] + node.shift);
                        }
                        data.set_numeric_values(&name, values)?;
                        data.set_format(&name, "F8.3")?;
                        let label = format!("Predicted value of {dependent_name}");
                        data.set_variable_label(&name, Some(&label))?;
                        data.set_measure(&name, Measure::Scale)?;
                    }
                    TreeSave::PredictedProbability if scale_dep => {}
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
                                values[c] = Some(node.probability(j));
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

    /// The importance and surrogate tables of a CRT tree.
    fn crt_tables(
        &self,
        command: &Tree,
        nodes: &[Node],
        predictors: &[Predictor],
        dependent: &Variable,
        (out, charts): (&mut Vec<Table>, &mut Vec<crate::Chart>),
    ) {
        let shown = self.shown();
        if command.crt && (command.print_importance || command.plot_importance) {
            // the sum of the improvements of the splits where a predictor is the main one or a
            // surrogate, relative to the largest
            let mut total = vec![0.0; predictors.len()];
            for split in nodes.iter().filter_map(|n| n.split.as_ref()) {
                total[split.predictor] += split.improvement.unwrap_or(0.0);
                for found in &split.surrogates {
                    total[found.predictor] += found.improvement;
                }
            }
            let mut order: Vec<usize> = (0..predictors.len()).filter(|&p| total[p] > 0.0).collect();
            order.sort_by(|&a, &b| total[b].total_cmp(&total[a]));
            let largest = order.first().map_or(1.0, |&p| total[p]);
            if command.print_importance {
                let rows: Vec<Vec<Cell>> = order
                    .iter()
                    .map(|&p| {
                        vec![
                            text(&predictors[p].name),
                            Cell::Number(total[p]),
                            percent(total[p], largest),
                        ]
                    })
                    .collect();
                let formats = vec![vec![String::new(), "F8.5".into(), "PCT6.1".into()]; rows.len()];
                out.push(Table {
                    title: "Independent Variable Importance".into(),
                    row_headers: 1,
                    columns: vec![
                        "Independent Variable".into(),
                        "Importance".into(),
                        "Normalized Importance".into(),
                    ],
                    column_groups: vec![Vec::new(); 3],
                    rows,
                    formats,
                    ..Table::default()
                });
            }
            if command.plot_importance && !order.is_empty() {
                let categories = order
                    .iter()
                    .map(|&p| crate::chart::Category {
                        label: predictors[p].name.clone(),
                        value: text(&predictors[p].name),
                        missing: false,
                        bin: None,
                    })
                    .collect();
                let series = vec![crate::chart::Series {
                    name: "Normalized importance".into(),
                    statistic: crate::chart::Statistic::Value,
                    variable: None,
                    value: Cell::Empty,
                    values: order.iter().map(|&p| 100.0 * total[p] / largest).collect(),
                }];
                let mut chart = crate::Chart::lines(
                    format!(
                        "Independent variable importance for {}",
                        shown.variable(dependent)
                    ),
                    crate::chart::Axis::titled("Independent variable"),
                    crate::chart::Axis::titled("Normalized importance"),
                    categories,
                    series,
                );
                chart.kind = crate::chart::ChartKind::Bar;
                chart.grouping = Some(crate::chart::Grouping::Clustered);
                chart.orientation = Some(crate::chart::Orientation::Horizontal);
                charts.push(chart);
            }
        }
        if command.binary() && command.print_surrogates {
            let mut rows = Vec::new();
            for (id, node) in nodes.iter().enumerate() {
                let Some(split) = &node.split else { continue };
                for found in &split.surrogates {
                    let other = &predictors[found.predictor];
                    rows.push(vec![
                        Cell::Number(id as f64),
                        text(&predictors[split.predictor].name),
                        text(format!("{} = {}", other.name, other.describe(&found.left))),
                        Cell::Number(found.association),
                    ]);
                }
            }
            let formats =
                vec![vec![String::new(), String::new(), String::new(), "F8.5".into()]; rows.len()];
            out.push(Table {
                title: "Surrogates".into(),
                row_headers: 1,
                columns: vec![
                    "Node".into(),
                    "Split Variable".into(),
                    "Surrogate".into(),
                    "Association".into(),
                ],
                column_groups: vec![Vec::new(); 4],
                rows,
                formats,
                caption: Some(
                    "Each surrogate sends its listed categories to the left child.".into(),
                ),
                ..Table::default()
            });
        }
    }

    /// The tables of a grown tree.
    fn tree_output(
        &self,
        command: &Tree,
        trees: (&[Node], Checks),
        predictors: &[Predictor],
        classes: &[Key],
        dependent: &Variable,
        charts: &mut Vec<crate::Chart>,
    ) -> Result<Vec<Table>, Error> {
        // the tree grown, and the same nodes counting the test sample's cases
        let (nodes, checks) = trees;
        let test = checks.test;
        if nodes[0].scale {
            return Ok(self.scale_output(command, trees, predictors, dependent, charts));
        }
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
            let method = method_name(command);
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
            if command.force.is_some() && !command.binary() {
                rows.push(row("", "Forced Variable", text(&predictors[0].name)));
            }
            if command.quest {
                rows.push(row(
                    "",
                    "Alpha for Split Selection",
                    Cell::Number(command.quest_alpha),
                ));
            }
            if command.crt {
                rows.push(row(
                    "",
                    "Impurity Measure",
                    text(match command.impurity {
                        TreeImpurity::Gini => "Gini",
                        TreeImpurity::Twoing => "Twoing",
                        TreeImpurity::OrderedTwoing => "Ordered Twoing",
                    }),
                ));
            }
            if command.costs.is_some() {
                rows.push(row("", "Misclassification Costs", text("Custom")));
            }
            if command.binary() {
                rows.push(row(
                    "",
                    "Prior Probabilities",
                    text(format!(
                        "{}{}",
                        match command.priors {
                            TreePriors::FromData => "From data",
                            TreePriors::Equal => "Equal",
                            TreePriors::Custom(_) => "Custom",
                        },
                        if command.adjust_priors && command.costs.is_some() {
                            ", adjusted for costs"
                        } else {
                            ""
                        }
                    )),
                ));
            }
            if command.crt {
                rows.push(row(
                    "",
                    "Minimum Change in Improvement",
                    Cell::Number(command.min_improvement.unwrap_or(0.0001)),
                ));
            }
            if command.binary() {
                rows.push(row(
                    "",
                    "Pruning",
                    text(match command.prune {
                        None => "None".to_string(),
                        Some(b) => format!("Standard error rule, {b} standard errors"),
                    }),
                ));
            }
            rows.push(row(
                "",
                "Validation",
                text(match &command.validation {
                    TreeValidation::None => "None".to_string(),
                    TreeValidation::SplitPercent(p) => {
                        format!("Split sample, {p}% to grow the tree")
                    }
                    TreeValidation::SplitVariable(v) => format!("Split sample by {v}"),
                    TreeValidation::Cross(v) => format!("Cross-validation, {v} folds"),
                }),
            ));
            if let Some(test) = test {
                rows.push(row(
                    "",
                    "Cases in Training Sample",
                    Cell::Number(nodes[0].n),
                ));
                rows.push(row("", "Cases in Test Sample", Cell::Number(test[0].n)));
            }
            rows.extend([
                row(
                    "",
                    "Maximum Tree Depth",
                    Cell::Number(depth_limit(command) as f64),
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
                if command.quest {
                    for name in ["Test", "Statistic", "df", "Error df", "Sig.", "Split Point"] {
                        push("Split", name);
                    }
                } else if command.crt {
                    push("Split", "Improvement");
                } else {
                    push("Split", "Adj. Sig.");
                    push("Split", "Chi-Square");
                    push("Split", "df");
                }
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
                            match (&split.quest, split.improvement) {
                                (Some(q), _) => row.extend([
                                    text(q.test),
                                    Cell::Number(q.statistic),
                                    Cell::Number(q.df),
                                    q.df2.map_or(Cell::Empty, Cell::Number),
                                    Cell::Number(q.sig),
                                    if q.cut.is_nan() {
                                        Cell::Empty
                                    } else {
                                        Cell::Number(q.cut)
                                    },
                                ]),
                                (None, Some(gain)) => row.push(Cell::Number(gain)),
                                (None, None) => row.extend([
                                    Cell::Number(split.sig),
                                    Cell::Number(split.statistic),
                                    Cell::Number(split.df),
                                ]),
                            }
                        }
                        None => {
                            let width = if command.quest {
                                7
                            } else if command.crt {
                                2
                            } else {
                                4
                            };
                            row.extend(vec![Cell::Empty; width]);
                        }
                    }
                }
                rows.push(row);
            }
            let format_row: Vec<String> = columns
                .iter()
                .map(|c| match c.as_str() {
                    "Percent" => "F5.1".to_string(),
                    "Adj. Sig." | "Sig." => "F5.3".to_string(),
                    "Improvement" => "F8.5".to_string(),
                    "Chi-Square" | "Statistic" | "Split Point" => "F8.3".to_string(),
                    "N" | "df" | "Error df" => "F8.0".to_string(),
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
                outline: None,
                hidden: false,
                cell_stats: None,
                decorations: Vec::new(),
                style: Default::default(),
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
                    match (&split.quest, split.improvement) {
                        (Some(q), _) => {
                            statistics.push((q.test.into(), Cell::Number(q.statistic)));
                            statistics.push(("df".into(), Cell::Number(q.df)));
                            if let Some(df2) = q.df2 {
                                statistics.push(("Error df".into(), Cell::Number(df2)));
                            }
                            statistics.push(("Sig.".into(), Cell::Number(q.sig)));
                            if !q.cut.is_nan() {
                                statistics.push(("Split Point".into(), Cell::Number(q.cut)));
                            }
                        }
                        (None, Some(gain)) => {
                            statistics.push(("Improvement".into(), Cell::Number(gain)))
                        }
                        (None, None) => {
                            statistics.push(("Adj. Sig.".into(), Cell::Number(split.sig)));
                            statistics.push(("Chi-Square".into(), Cell::Number(split.statistic)));
                            statistics.push(("df".into(), Cell::Number(split.df)));
                        }
                    }
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
        self.crt_tables(command, nodes, predictors, dependent, (&mut out, charts));

        // the samples the risk, classification and gain tables describe
        let samples: Vec<(&str, &[Node])> = match test {
            None => vec![("", nodes)],
            Some(test) if command.test_sample_only => vec![("Test", test)],
            Some(test) => vec![("Training", nodes), ("Test", test)],
        };
        let validated = test.is_some();
        // a test sample's risk takes M1 when the priors come from the data
        let empirical = |label: &str| (label == "Test").then(|| empirical_priors(command));
        let mut confusions = Vec::new();
        for (_, ns) in &samples {
            let mut confusion = vec![vec![0.0; classes.len()]; classes.len()];
            for &t in &terminals {
                let predicted = ns[t].predicted();
                for (j, &count) in ns[t].counts.iter().enumerate() {
                    confusion[j][predicted] += count;
                }
            }
            confusions.push(confusion);
        }
        if command.print.risk {
            let labelled = validated || checks.cross.is_some();
            let mut rows: Vec<Vec<Cell>> = samples
                .iter()
                .map(|(label, ns)| {
                    let (risk, variance) =
                        risk_estimate(terminals.iter().map(|&t| &ns[t]), &ns[0], empirical(label));
                    let mut row = Vec::new();
                    if labelled {
                        let name = if checks.cross.is_some() {
                            "Resubstitution"
                        } else {
                            label
                        };
                        row.push(text(name));
                    }
                    row.extend([Cell::Number(risk), Cell::Number(variance.sqrt())]);
                    row
                })
                .collect();
            if let Some((risk, variance)) = checks.cross {
                rows.push(vec![
                    text("Cross-Validation"),
                    Cell::Number(risk),
                    Cell::Number(variance.sqrt()),
                ]);
            }
            let mut columns = vec!["Estimate".to_string(), "Std. Error".to_string()];
            if labelled {
                columns.insert(0, if validated { "Sample" } else { "Method" }.into());
            }
            let width = columns.len();
            out.push(Table {
                title: "Risk".into(),
                row_headers: usize::from(labelled),
                column_groups: vec![Vec::new(); width],
                columns,
                formats: vec![
                    vec![String::new(); width - 2]
                        .into_iter()
                        .chain(["F8.3".into(), "F8.3".into()])
                        .collect();
                    rows.len()
                ],
                rows,
                caption: Some(format!(
                    "Growing Method: {}. Dependent Variable: {}.",
                    method_name(command),
                    shown.variable(dependent)
                )),
                outline: None,
                hidden: false,
                cell_stats: None,
                decorations: Vec::new(),
                style: Default::default(),
            });
        }
        if command.print.classification {
            let mut columns = vec!["Observed".to_string()];
            columns.extend(class_labels.iter().cloned());
            columns.push("Percent Correct".into());
            let mut groups = vec![Vec::new()];
            groups.extend(vec![vec!["Predicted".to_string()]; classes.len() + 1]);
            if validated {
                columns.insert(0, "Sample".into());
                groups.insert(0, Vec::new());
            }
            let width = columns.len();
            let mut rows = Vec::new();
            let mut formats = Vec::new();
            for (((label, ns), confusion), _) in samples.iter().zip(&confusions).zip(0..) {
                let sample_n = ns[0].n;
                let correct: f64 = (0..classes.len()).map(|j| confusion[j][j]).sum();
                let lead = |name: String| -> Vec<Cell> {
                    if validated {
                        vec![text(*label), text(name)]
                    } else {
                        vec![text(name)]
                    }
                };
                for (j, row) in confusion.iter().enumerate() {
                    let total: f64 = row.iter().sum();
                    let mut cells = lead(class_labels[j].clone());
                    cells.extend(row.iter().map(|&n| Cell::Number(n)));
                    cells.push(percent(row[j], total));
                    rows.push(cells);
                    let mut format = vec![String::new(); width];
                    format[width - 1] = "PCT6.1".into();
                    formats.push(format);
                }
                let mut overall = lead("Overall Percentage".into());
                for k in 0..classes.len() {
                    let predicted: f64 = confusion.iter().map(|r| r[k]).sum();
                    overall.push(percent(predicted, sample_n));
                }
                overall.push(percent(correct, sample_n));
                rows.push(overall);
                formats.push(vec!["PCT6.1".to_string(); width]);
            }
            out.push(Table {
                title: "Classification".into(),
                row_headers: if validated { 2 } else { 1 },
                columns,
                column_groups: groups,
                rows,
                formats,
                caption: None,
                outline: None,
                hidden: false,
                cell_stats: None,
                decorations: Vec::new(),
                style: Default::default(),
            });
        }

        // the gain tables and charts
        let class_names: &[String] = &class_labels;
        let gains = gain::Context {
            command,
            samples: &samples,
            training: nodes,
            terminals: &terminals,
            classes,
            class_labels: class_names,
            dependent: &shown.variable(dependent),
            dependent_name: dependent.name(),
            validated,
        };
        gains.node_tables(&mut out);
        gains.charts(charts);
        Ok(out)
    }
}

/// The terminal nodes in the order of the gain tables: by the index of target class `j`,
/// highest first unless `ascending`.
fn method_name(command: &Tree) -> &'static str {
    match (command.crt, command.quest, command.exhaustive) {
        (true, _, _) => "CRT",
        (_, true, _) => "QUEST",
        (_, _, true) => "Exhaustive CHAID",
        _ => "CHAID",
    }
}

/// The resubstitution risk of a tree and its variance, from its terminal nodes and the root.
/// With the priors the data give it is the share misclassified; otherwise each class's share
/// misclassified, weighted by its prior.
fn risk_estimate<'a>(
    terminals: impl Iterator<Item = &'a Node>,
    root: &Node,
    empirical: Option<bool>,
) -> (f64, f64) {
    if root.scale {
        // the sum of squares about the node means over the weight, and its error from the
        // fourth moments of the cases
        let (mut squares, mut fourth) = (0.0, 0.0);
        for node in terminals {
            squares += node
                .squares_about
                .unwrap_or(node.counts[2] - node.counts[1] * node.counts[1] / node.counts[0]);
            fourth += node.fourth;
        }
        let risk = squares / root.n;
        return (
            risk,
            ((fourth - root.n * risk * risk) / (root.n * root.n)).max(0.0),
        );
    }
    let classes = root.counts.len();
    let (mut loss, mut squared) = (vec![0.0; classes], vec![0.0; classes]);
    for node in terminals {
        add_losses(node, &mut loss, &mut squared);
    }
    misclassification_risk(&loss, &squared, root, empirical)
}

/// Adds the cost of the node's assignment to each class's cases in it, and the square of that cost.
fn add_losses(node: &Node, loss: &mut [f64], squared: &mut [f64]) {
    let predicted = node.predicted();
    for (j, &count) in node.counts.iter().enumerate() {
        let cost = match &node.costs {
            Some(matrix) => matrix[predicted][j],
            None => f64::from(u8::from(j != predicted)),
        };
        loss[j] += count * cost;
        squared[j] += count * cost * cost;
    }
}

/// The risk and its variance from the cost of the assignments of each class's cases (`loss`) and
/// the sum of the squares of those costs, and the root of the sample the classes are counted in
/// (its priors in `assign`, its class totals). With costs of 0 and 1 the loss is the weight
/// assigned wrongly.
fn misclassification_risk(
    loss: &[f64],
    squared: &[f64],
    root: &Node,
    empirical: Option<bool>,
) -> (f64, f64) {
    let classes = root.counts.len();
    let prior_total: f64 = root.assign.iter().sum();
    // priors from the data (M1), judged by the priors' setting for a test sample, else by whether
    // they equal the shares of the cases
    let from_data = empirical.unwrap_or_else(|| {
        (0..classes).all(|j| (root.assign[j] / prior_total - root.counts[j] / root.n).abs() < 1e-12)
    });
    if from_data {
        let risk = loss.iter().sum::<f64>() / root.n;
        let variance = (squared.iter().sum::<f64>() - root.n * risk * risk) / (root.n * root.n);
        return (risk, variance.max(0.0));
    }
    let (mut risk, mut variance) = (0.0, 0.0);
    for (j, &missed) in loss.iter().enumerate() {
        let total = root.counts[j];
        if total > 0.0 {
            let prior = root.assign[j] / prior_total;
            risk += prior * missed / total;
            variance += (prior / total).powi(2) * (squared[j] - missed * missed / total);
        }
    }
    (risk, variance)
}

/// The depth limit: AUTO is three levels for CHAID and five for CRT.
fn depth_limit(command: &Tree) -> usize {
    command
        .max_depth
        .unwrap_or(if command.binary() { 5 } else { 3 })
}

/// Whether the priors are the data's shares (the empirical case M1 of the risk): always for CHAID,
/// for CRT and QUEST when they come from the data and the costs have not altered them.
fn empirical_priors(command: &Tree) -> bool {
    !command.binary()
        || (command.priors == TreePriors::FromData
            && !(command.adjust_priors && command.costs.is_some()))
}

/// The cost matrix of `COSTS CUSTOM`: every cost 1 unless given, 0 on the diagonal (a nonzero one
/// given there is ignored with a warning), a category that is no class ignored with a warning.
fn custom_costs(
    command: &Tree,
    classes: &[Key],
    labels: &Shown,
    dependent: &Variable,
    warnings: &mut Vec<String>,
) -> Option<Costs> {
    let given = command.costs.as_ref()?;
    let n = classes.len();
    let mut matrix: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| if i == j { 0.0 } else { 1.0 }).collect())
        .collect();
    let index = |v: &statscraft_syntax::Value| classes.iter().position(|k| *k == key_of(v));
    for (actual, predicted, cost) in given {
        let (Some(j), Some(i)) = (index(actual), index(predicted)) else {
            warnings.push(format!(
                "COSTS CUSTOM names a category of {} that is not in the analysis; ignored",
                dependent.name()
            ));
            continue;
        };
        if i == j {
            if *cost != 0.0 {
                warnings.push(format!(
                    "COSTS CUSTOM gives a cost for classifying category {} correctly; it is 0",
                    labels.value(dependent, &classes[j])
                ));
            }
            continue;
        }
        matrix[i][j] = *cost;
    }
    Some(Rc::new(matrix))
}

/// The prior probabilities of the classes, summing to 1.
fn crt_priors(
    command: &Tree,
    classes: &[Key],
    case_class: &[usize],
    case_weight: &[f64],
    labels: &Shown,
    dependent: &Variable,
    warnings: &mut Vec<String>,
) -> Result<Vec<f64>, Error> {
    let priors: Vec<f64> = match &command.priors {
        TreePriors::FromData => {
            let mut counts = vec![0.0; classes.len()];
            for (&j, &w) in case_class.iter().zip(case_weight) {
                counts[j] += w;
            }
            counts
        }
        TreePriors::Equal => vec![1.0; classes.len()],
        TreePriors::Custom(given) => {
            let mut priors = Vec::new();
            for class in classes {
                match given.iter().find(|(v, _)| key_of(v) == *class) {
                    Some((_, p)) => priors.push(*p),
                    None => {
                        return Err(Error(format!(
                            "PRIORS CUSTOM gives no probability for category {}",
                            labels.value(dependent, class)
                        )));
                    }
                }
            }
            for (value, _) in given {
                if !classes.contains(&key_of(value)) {
                    warnings.push(format!(
                        "PRIORS CUSTOM names a category of {} that is not in the analysis; ignored",
                        dependent.name()
                    ));
                }
            }
            priors
        }
    };
    let total: f64 = priors.iter().sum();
    if total <= 0.0 {
        return Err(Error("the prior probabilities sum to zero".into()));
    }
    Ok(priors.iter().map(|p| p / total).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cheapest_class_may_be_one_the_node_does_not_hold() {
        // assigning class 3 costs 0.1 whoever the case is, the others cost 5 when wrong
        let costs = vec![
            vec![0.0, 5.0, 5.0],
            vec![5.0, 0.0, 5.0],
            vec![0.1, 0.1, 0.0],
        ];
        // a node of classes 1 and 2 only: expected cost 2.5 for class 1, 2.5 for 2, 0.1 for 3
        assert_eq!(cheapest(&[0.5, 0.5, 0.0], &[10.0, 10.0, 0.0], &costs), 2);
        // equal costs, a tie between the classes: the lowest one that the node holds
        let unit = vec![
            vec![0.0, 1.0, 1.0],
            vec![1.0, 0.0, 1.0],
            vec![1.0, 1.0, 0.0],
        ];
        assert_eq!(cheapest(&[0.0, 0.5, 0.5], &[0.0, 4.0, 4.0], &unit), 1);
        // none of the tied classes is held: the lowest of them
        assert_eq!(cheapest(&[0.0, 0.0, 0.0], &[0.0, 0.0, 0.0], &unit), 0);
    }

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
