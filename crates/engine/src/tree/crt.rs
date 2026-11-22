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
//! - A case with no value of the split predictor goes to the child with more cases.

use super::{Node, Predictor, Split, make_node};
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
            if weight_left >= weight_right {
                to_left.extend(unknown);
            } else {
                to_right.extend(unknown);
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
            });
        }
        Ok(nodes)
    }
}
