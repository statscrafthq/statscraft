//! CRT: binary trees grown by the best split of each node under a Gini, twoing or ordered twoing
//! measure, for a categorical dependent.
//!
//! Rules from the Command Syntax Reference (TREE, CRT, PRIORS) and the Algorithms manual (TREE
//! Algorithms, CART):
//! - A scale or ordinal predictor offers one split after each of its values present in the node
//!   (x <= v goes left, v a value of the data); a nominal one offers every subset of its
//!   categories. For two classes the best subset is found by ordering the categories by their
//!   share of the first class; for more classes the subsets are listed, up to
//!   `MAX_SUBSET_CATEGORIES` categories present in a node.
//! - The class probability in a node t is p(j,t) = prior(j) N_j(t) / N_j, with the counts of the
//!   cases that have a value of the predictor being split (the root counts too). p(t) is their
//!   sum. Gini: i(t) = 1 - sum_j (p(j,t) / p(t))^2. Twoing: pL pR (sum_j |p(j|tL) - p(j|tR)|)^2.
//!   Ordered twoing: the same with the signed sum over the classes up to a cut, the best cut
//!   taken. The improvement is p(t) times the decrease in impurity (the criterion itself for the
//!   twoings), and the split with the largest improvement over all predictors wins, the first
//!   predictor on ties.
//! - A node is not split when it is pure, at the depth limit, smaller than MINPARENTSIZE, or when
//!   the best improvement is below MINIMPROVEMENT. A candidate that would leave a child below
//!   MINCHILDSIZE (cases with a value of the predictor) is not offered, so the best remaining
//!   split is taken.
//! - A case with no value of the split predictor follows the first surrogate whose predictor it
//!   has, and the child with more cases when there is none. A surrogate for a split s* of X* is
//!   the split of another predictor X with the largest probability of sending a case that has
//!   both values to the side s* does (a cut in either direction, or any subset of the categories
//!   of a nominal predictor, each category going where more of its weight goes), with the
//!   probabilities of the primary split's class weights taken over the cases with both values
//!   and conditional on the node. Its association is (min(pL, pR) - (1 - agreement)) / min(pL, pR),
//!   with pL and pR the shares of the cases with a value of X* that s* sends left and right; only
//!   a positive association keeps the surrogate, the best ones up to MAXSURROGATES.

use super::{Node, Predictor, Split, Surrogate, make_node};
use crate::Error;
use statscraft_syntax::TreeImpurity;
use std::collections::VecDeque;

/// Most categories of a nominal predictor present in a node whose subsets are listed when the
/// dependent has more than two classes.
pub(super) const MAX_SUBSET_CATEGORIES: usize = 20;

/// The categories that go left and right, and the improvement.
type Cut = (Vec<usize>, Vec<usize>, f64);

pub(super) struct Growth<'a> {
    pub predictors: &'a [Predictor],
    pub case_class: &'a [usize],
    pub case_weight: &'a [f64],
    pub classes: usize,
    pub priors: &'a [f64],
    pub impurity: TreeImpurity,
    pub min_improvement: f64,
    pub min_parent: f64,
    pub min_child: f64,
    pub max_depth: usize,
    /// None is AUTO: one fewer than the predictors.
    pub max_surrogates: Option<usize>,
}

/// The classes' share of a node, as the sums of weights.
fn weighted(counts: &[f64], factor: &[f64]) -> Vec<f64> {
    counts.iter().zip(factor).map(|(c, f)| c * f).collect()
}

impl Growth<'_> {
    /// The factor that turns a class count into p(j, t): prior over the class total.
    fn factor(&self, totals: &[f64]) -> Vec<f64> {
        totals
            .iter()
            .zip(self.priors)
            .map(|(&n, &p)| if n > 0.0 { p / n } else { 0.0 })
            .collect()
    }

    /// The improvement of sending `left` and `right` (class counts) to the two children.
    fn improvement(&self, left: &[f64], right: &[f64], factor: &[f64]) -> f64 {
        let (l, r) = (weighted(left, factor), weighted(right, factor));
        let (pl, pr) = (l.iter().sum::<f64>(), r.iter().sum::<f64>());
        let total = pl + pr;
        if pl <= 0.0 || pr <= 0.0 {
            return 0.0;
        }
        let (share_l, share_r) = (pl / total, pr / total);
        let gini = |p: &[f64], sum: f64| 1.0 - p.iter().map(|x| (x / sum).powi(2)).sum::<f64>();
        match self.impurity {
            TreeImpurity::Gini => {
                let parent: Vec<f64> = l.iter().zip(&r).map(|(a, b)| a + b).collect();
                total * (gini(&parent, total) - share_l * gini(&l, pl) - share_r * gini(&r, pr))
            }
            TreeImpurity::Twoing => {
                let spread: f64 = l.iter().zip(&r).map(|(a, b)| (a / pl - b / pr).abs()).sum();
                total * share_l * share_r * spread * spread
            }
            TreeImpurity::OrderedTwoing => {
                let mut running = 0.0;
                let mut best = 0.0_f64;
                for (a, b) in l.iter().zip(&r).take(self.classes - 1) {
                    running += a / pl - b / pr;
                    best = best.max(running * running);
                }
                total * share_l * share_r * best
            }
        }
    }

    /// The best split of a node by predictor `p`: the codes that go left and right, and its
    /// improvement. None when the predictor offers no allowed split.
    fn best_split(&self, p: usize, node: &Node, root_valid: &[f64]) -> Result<Option<Cut>, Error> {
        let predictor = &self.predictors[p];
        let missing = predictor.missing();
        let mut table = vec![vec![0.0; self.classes]; missing];
        for &i in &node.cases {
            let code = predictor.codes[i];
            if code != missing {
                table[code][self.case_class[i]] += self.case_weight[i];
            }
        }
        let present: Vec<usize> = (0..missing)
            .filter(|&c| table[c].iter().sum::<f64>() > 0.0)
            .collect();
        if present.len() < 2 {
            return Ok(None);
        }
        let factor = self.factor(root_valid);
        let mut total = vec![0.0; self.classes];
        for &c in &present {
            for (t, n) in total.iter_mut().zip(&table[c]) {
                *t += n;
            }
        }
        let sum = |codes: &[usize]| {
            let mut counts = vec![0.0; self.classes];
            for &c in codes {
                for (t, n) in counts.iter_mut().zip(&table[c]) {
                    *t += n;
                }
            }
            counts
        };
        // each candidate is the present codes that go left
        let mut best: Option<(Vec<usize>, f64)> = None;
        let mut offer = |left: Vec<usize>| {
            let l = sum(&left);
            let r: Vec<f64> = total.iter().zip(&l).map(|(t, a)| t - a).collect();
            let (nl, nr) = (l.iter().sum::<f64>(), r.iter().sum::<f64>());
            if nl < self.min_child || nr < self.min_child {
                return;
            }
            let gain = self.improvement(&l, &r, &factor);
            if best
                .as_ref()
                .is_none_or(|(_, b)| gain > *b + 1e-12 * b.abs())
            {
                best = Some((left, gain));
            }
        };
        if !predictor.nominal {
            for k in 1..present.len() {
                offer(present[..k].to_vec());
            }
        } else if self.classes == 2 {
            let share = |c: usize| {
                let w = weighted(&table[c], &factor);
                w[0] / (w[0] + w[1])
            };
            let mut order = present.clone();
            order.sort_by(|&a, &b| share(a).total_cmp(&share(b)));
            for k in 1..order.len() {
                offer(order[..k].to_vec());
            }
        } else {
            if present.len() > MAX_SUBSET_CATEGORIES {
                return Err(Error(format!(
                    "the nominal predictor {} has {} categories in a node, and the dependent has \
                     more than two classes; the subsets are searched up to {} categories",
                    predictor.name,
                    present.len(),
                    MAX_SUBSET_CATEGORIES
                )));
            }
            // the first category always goes left, so no split is listed twice
            let rest = present.len() - 1;
            for mask in 0..(1_usize << rest) - 1 {
                let mut left = vec![present[0]];
                left.extend(
                    (0..rest)
                        .filter(|b| mask >> b & 1 == 1)
                        .map(|b| present[b + 1]),
                );
                offer(left);
            }
        }
        Ok(best.map(|(left, gain)| {
            // the subset with the first category present goes left, however it was found
            let flip = predictor.nominal && !left.contains(&present[0]);
            let to_left = |c: usize| left.contains(&c) != flip;
            let last = *left.iter().max().expect("a split sends a category left");
            let (left, right): (Vec<usize>, Vec<usize>) = if predictor.nominal {
                (
                    (0..missing).filter(|&c| to_left(c)).collect(),
                    (0..missing).filter(|&c| !to_left(c)).collect(),
                )
            } else {
                // the cut is the largest value that goes left
                ((0..=last).collect(), (last + 1..missing).collect())
            };
            (left, right, gain)
        }))
    }

    /// The surrogates of the split of predictor `p` that sends the codes `left` left.
    fn surrogates(
        &self,
        node: &Node,
        p: usize,
        left: &[usize],
        priors_root: &[f64],
    ) -> Vec<Surrogate> {
        let limit = self
            .max_surrogates
            .unwrap_or(usize::MAX)
            .min(self.predictors.len() - 1);
        if limit == 0 {
            return Vec::new();
        }
        let primary = &self.predictors[p];
        let goes_left = |i: usize| left.contains(&primary.codes[i]);
        let with_value: Vec<usize> = node
            .cases
            .iter()
            .copied()
            .filter(|&i| primary.codes[i] != primary.missing())
            .collect();
        let factor = self.factor(priors_root);
        let (mut pl, mut pr) = (0.0, 0.0);
        for &i in &with_value {
            let w = self.case_weight[i] * factor[self.case_class[i]];
            if goes_left(i) {
                pl += w;
            } else {
                pr += w;
            }
        }
        let smaller = pl.min(pr) / (pl + pr);
        if smaller.is_nan() || smaller <= 0.0 {
            return Vec::new();
        }
        let mut found: Vec<Surrogate> = Vec::new();
        for (q, other) in self.predictors.iter().enumerate() {
            if q == p {
                continue;
            }
            let both = |i: usize| {
                primary.codes[i] != primary.missing() && other.codes[i] != other.missing()
            };
            // the class totals of every case with both values give the factors
            let mut totals = vec![0.0; self.classes];
            for i in 0..self.case_class.len() {
                if both(i) {
                    totals[self.case_class[i]] += self.case_weight[i];
                }
            }
            let factor = self.factor(&totals);
            // the weight of the cases of each category that s* sends left and right
            let mut side = vec![[0.0; 2]; other.missing()];
            let mut all = 0.0;
            for &i in node.cases.iter().filter(|&&i| both(i)) {
                let w = self.case_weight[i] * factor[self.case_class[i]];
                side[other.codes[i]][usize::from(!goes_left(i))] += w;
                all += w;
            }
            if all <= 0.0 {
                continue;
            }
            let present: Vec<usize> = (0..other.missing())
                .filter(|&c| side[c][0] + side[c][1] > 0.0)
                .collect();
            // the split and the weight of the cases it sends where s* does
            let mut best: Option<(Vec<usize>, f64)> = None;
            let mut offer = |codes: Vec<usize>, agreement: f64| {
                if best
                    .as_ref()
                    .is_none_or(|(_, b)| agreement > *b + 1e-12 * b.abs())
                {
                    best = Some((codes, agreement));
                }
            };
            if other.nominal {
                let codes: Vec<usize> = present
                    .iter()
                    .copied()
                    .filter(|&c| side[c][0] >= side[c][1])
                    .collect();
                let agreement = present.iter().map(|&c| side[c][0].max(side[c][1])).sum();
                offer(codes, agreement);
            } else {
                for k in 1..present.len() {
                    let (below, above) = present.split_at(k);
                    let last = *below.last().expect("a prefix");
                    let sum =
                        |codes: &[usize], s: usize| codes.iter().map(|&c| side[c][s]).sum::<f64>();
                    // x <= v goes left, or x > v goes left
                    offer((0..=last).collect(), sum(below, 0) + sum(above, 1));
                    offer(
                        (last + 1..other.missing()).collect(),
                        sum(below, 1) + sum(above, 0),
                    );
                }
            }
            let Some((codes, agreement)) = best else {
                continue;
            };
            let association = (smaller - (1.0 - agreement / all)) / smaller;
            if association > 1e-12 {
                found.push(Surrogate {
                    predictor: q,
                    left: codes,
                    association,
                });
            }
        }
        found.sort_by(|a, b| b.association.total_cmp(&a.association));
        found.truncate(limit);
        found
    }

    pub(super) fn grow(&self) -> Result<Vec<Node>, Error> {
        let n = self.case_class.len();
        let mut totals = vec![0.0; self.classes];
        for i in 0..n {
            totals[self.case_class[i]] += self.case_weight[i];
        }
        let factor = self.factor(&totals);
        // the class totals of the cases that have a value of each predictor
        let valid: Vec<Vec<f64>> = self
            .predictors
            .iter()
            .map(|p| {
                let mut counts = vec![0.0; self.classes];
                for i in 0..n {
                    if p.codes[i] != p.missing() {
                        counts[self.case_class[i]] += self.case_weight[i];
                    }
                }
                counts
            })
            .collect();
        let sample = (self.case_class, self.case_weight);
        let mut nodes = vec![make_node((0..n).collect(), None, 0, None, sample, &factor)];
        let mut queue = VecDeque::from([0]);
        while let Some(id) = queue.pop_front() {
            let node = &nodes[id];
            let pure = node.counts.iter().filter(|&&c| c > 0.0).count() < 2;
            if node.depth >= self.max_depth || node.n < self.min_parent || pure {
                continue;
            }
            let mut best: Option<(usize, Vec<usize>, Vec<usize>, f64)> = None;
            for (p, valid_counts) in valid.iter().enumerate() {
                if let Some((left, right, gain)) = self.best_split(p, node, valid_counts)?
                    && best.as_ref().is_none_or(|b| gain > b.3 + 1e-12 * b.3.abs())
                {
                    best = Some((p, left, right, gain));
                }
            }
            let Some((p, left, right, gain)) = best.filter(|b| b.3 >= self.min_improvement) else {
                continue;
            };
            let predictor = &self.predictors[p];
            let surrogates = self.surrogates(node, p, &left, &valid[p]);
            let (mut to_left, mut to_right) = (Vec::new(), Vec::new());
            let (mut weight_left, mut weight_right) = (0.0, 0.0);
            let mut unknown = Vec::new();
            for &i in &node.cases {
                let code = predictor.codes[i];
                if code == predictor.missing() {
                    unknown.push(i);
                } else if left.contains(&code) {
                    to_left.push(i);
                    weight_left += self.case_weight[i];
                } else {
                    to_right.push(i);
                    weight_right += self.case_weight[i];
                }
            }
            // a case with no value follows the first surrogate it has a value for, else the majority
            for i in unknown {
                let by_surrogate = surrogates.iter().find_map(|s| {
                    let other = &self.predictors[s.predictor];
                    (other.codes[i] != other.missing()).then(|| s.left.contains(&other.codes[i]))
                });
                if by_surrogate.unwrap_or(weight_left >= weight_right) {
                    to_left.push(i);
                } else {
                    to_right.push(i);
                }
            }
            let depth = node.depth + 1;
            let children = [
                make_node(to_left, Some(id), depth, Some((p, left)), sample, &factor),
                make_node(to_right, Some(id), depth, Some((p, right)), sample, &factor),
            ];
            for child in children {
                let next = nodes.len();
                nodes[id].children.push(next);
                queue.push_back(next);
                nodes.push(child);
            }
            nodes[id].split = Some(Split {
                predictor: p,
                groups: Vec::new(),
                statistic: 0.0,
                df: 0.0,
                sig: 0.0,
                improvement: Some(gain),
                surrogates,
            });
        }
        Ok(nodes)
    }
}
