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
//! - Pruning (PRUNE=SE(b)): the subtrees T0 (the grown tree), T1, ... down to the root are the
//!   repeated weakest-link cuts: each step turns every internal node with the smallest
//!   (R(t) - R(subtree of t)) / (terminal nodes of the subtree - 1) into a terminal node, R the
//!   resubstitution risk of a node, the weight of the classes it does not assign (priors over class
//!   totals times counts). The risk of each subtree and its standard error come from the same
//!   estimators as the Risk table. With k* the smallest subtree whose risk is the smallest, the
//!   one kept is the smallest subtree with risk at most R(T_k*) + b se(R(T_k*)).
//! - A case with no value of the split predictor follows the first surrogate whose predictor it
//!   has, and the child with more cases when there is none. A surrogate for a split s* of X* is
//!   the split of another predictor X with the largest probability of sending a case that has
//!   both values to the side s* does (a cut in either direction, or any subset of the categories
//!   of a nominal predictor, each category going where more of its weight goes), with the
//!   probabilities of the primary split's class weights taken over the cases with both values
//!   and conditional on the node. Its association is (min(pL, pR) - (1 - agreement)) / min(pL, pR),
//!   with pL and pR the shares of the cases with a value of X* that s* sends left and right; only
//!   a positive association keeps the surrogate, the best ones up to MAXSURROGATES.

use super::{Node, Predictor, Split, Surrogate, make_node, risk_estimate};
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
    /// A scale dependent's values; the classes are then the one class of all cases.
    pub values: Option<&'a [f64]>,
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
    /// The length of the statistics a node adds up: the class weights, or for a scale dependent
    /// the weight, the sum of the values and of their squares.
    fn dim(&self) -> usize {
        if self.values.is_some() {
            3
        } else {
            self.classes
        }
    }

    /// Adds case `i` to a node's statistics.
    fn add(&self, into: &mut [f64], i: usize) {
        let w = self.case_weight[i];
        match self.values {
            Some(y) => {
                into[0] += w;
                into[1] += w * y[i];
                into[2] += w * y[i] * y[i];
            }
            None => into[self.case_class[i]] += w,
        }
    }

    /// The weight of the cases in statistics.
    fn size(&self, stats: &[f64]) -> f64 {
        if self.values.is_some() {
            stats[0]
        } else {
            stats.iter().sum()
        }
    }

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
        if self.values.is_some() {
            // least squares deviation: the drop in the sum of squares over the weight of the
            // root cases that have a value of the predictor
            let squares = |a: &[f64]| a[2] - a[1] * a[1] / a[0];
            if left[0] <= 0.0 || right[0] <= 0.0 {
                return 0.0;
            }
            let parent = [left[0] + right[0], left[1] + right[1], left[2] + right[2]];
            return factor[0] * (squares(&parent) - squares(left) - squares(right));
        }
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
        let mut table = vec![vec![0.0; self.dim()]; missing];
        for &i in &node.cases {
            let code = predictor.codes[i];
            if code != missing {
                self.add(&mut table[code], i);
            }
        }
        let present: Vec<usize> = (0..missing)
            .filter(|&c| self.size(&table[c]) > 0.0)
            .collect();
        if present.len() < 2 {
            return Ok(None);
        }
        let factor = self.factor(root_valid);
        let mut total = vec![0.0; self.dim()];
        for &c in &present {
            for (t, n) in total.iter_mut().zip(&table[c]) {
                *t += n;
            }
        }
        let sum = |codes: &[usize]| {
            let mut counts = vec![0.0; self.dim()];
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
            let (nl, nr) = (self.size(&l), self.size(&r));
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
        } else if self.values.is_some() {
            // the categories ordered by their mean give the best subset
            let mean = |c: usize| table[c][1] / table[c][0];
            let mut order = present.clone();
            order.sort_by(|&a, &b| mean(a).total_cmp(&mean(b)));
            for k in 1..order.len() {
                offer(order[..k].to_vec());
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
        valid: &[Vec<f64>],
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
        let factor = self.factor(&valid[p]);
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
                // the improvement of the surrogate as a split of the node's cases with a value
                let (mut l, mut r) = (vec![0.0; self.dim()], vec![0.0; self.dim()]);
                for &i in node
                    .cases
                    .iter()
                    .filter(|&&i| other.codes[i] != other.missing())
                {
                    let side = if codes.contains(&other.codes[i]) {
                        &mut l
                    } else {
                        &mut r
                    };
                    self.add(side, i);
                }
                let improvement = self.improvement(&l, &r, &self.factor(&valid[q]));
                found.push(Surrogate {
                    predictor: q,
                    left: codes,
                    association,
                    improvement,
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
        let sample = (self.case_class, self.case_weight, self.values);
        let mut nodes = vec![make_node((0..n).collect(), None, 0, None, sample, &factor)];
        let mut queue = VecDeque::from([0]);
        while let Some(id) = queue.pop_front() {
            let node = &nodes[id];
            let pure = if self.values.is_some() {
                // every value alike, up to rounding
                let [w, s, q] = [node.counts[0], node.counts[1], node.counts[2]];
                q * w - s * s <= 1e-12 * q * w
            } else {
                node.counts.iter().filter(|&&c| c > 0.0).count() < 2
            };
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
            let surrogates = self.surrogates(node, p, &left, &valid);
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

/// The node's own risk: the weight of the classes it does not assign.
fn node_risk(node: &Node) -> f64 {
    if node.scale {
        return node.assign[0];
    }
    node.assign.iter().sum::<f64>() - node.assign[node.predicted()]
}

/// Prunes by the standard error rule: the nodes of the kept subtree, renumbered in order.
pub(super) fn prune(nodes: Vec<Node>, b: f64) -> Vec<Node> {
    let n = nodes.len();
    // cut[t]: the subtree below t is gone and t is terminal
    let mut cut = vec![false; n];
    let mut sequence = vec![cut.clone()];
    let terminals = |cut: &[bool]| -> Vec<usize> {
        let mut found = Vec::new();
        let mut stack = vec![0];
        while let Some(t) = stack.pop() {
            if cut[t] || nodes[t].children.is_empty() {
                found.push(t);
            } else {
                stack.extend(&nodes[t].children);
            }
        }
        found
    };
    loop {
        // children come after their parents, so a backward pass sees them first
        let (mut leaves, mut subtree_risk) = (vec![1.0; n], vec![0.0; n]);
        let mut gap = vec![f64::INFINITY; n];
        for t in (0..n).rev() {
            if cut[t] || nodes[t].children.is_empty() {
                subtree_risk[t] = node_risk(&nodes[t]);
            } else {
                let [l, r] = [nodes[t].children[0], nodes[t].children[1]];
                leaves[t] = leaves[l] + leaves[r];
                subtree_risk[t] = subtree_risk[l] + subtree_risk[r];
                gap[t] = (node_risk(&nodes[t]) - subtree_risk[t]) / (leaves[t] - 1.0);
            }
        }
        // only nodes still part of the tree count
        let mut live = vec![false; n];
        for t in terminals(&cut) {
            let mut up = Some(t);
            while let Some(u) = up {
                live[u] = true;
                up = nodes[u].parent;
            }
        }
        let weakest = (0..n)
            .filter(|&t| live[t] && gap[t].is_finite())
            .map(|t| gap[t])
            .fold(f64::INFINITY, f64::min);
        if !weakest.is_finite() {
            break;
        }
        let limit = weakest + 1e-9 * weakest.abs() + 1e-15;
        for t in 0..n {
            let below_cut = {
                let mut up = nodes[t].parent;
                let mut found = false;
                while let Some(u) = up {
                    found |= cut[u];
                    up = nodes[u].parent;
                }
                found
            };
            if live[t] && gap[t] <= limit && !cut[t] && !below_cut {
                cut[t] = true;
            }
        }
        sequence.push(cut.clone());
    }
    let risks: Vec<(f64, f64)> = sequence
        .iter()
        .map(|c| {
            let (risk, variance) =
                risk_estimate(terminals(c).into_iter().map(|t| &nodes[t]), &nodes[0]);
            (risk, variance.sqrt())
        })
        .collect();
    let lowest = risks.iter().map(|r| r.0).fold(f64::INFINITY, f64::min);
    let smallest = (0..risks.len())
        .rev()
        .find(|&k| risks[k].0 <= lowest + 1e-12)
        .expect("a subtree has the lowest risk");
    let allowed = risks[smallest].0 + b * risks[smallest].1 + 1e-12;
    let kept = (0..risks.len())
        .rev()
        .find(|&k| risks[k].0 <= allowed)
        .expect("the subtree with the lowest risk is allowed");
    renumber(nodes, &sequence[kept])
}

/// The nodes that stay when the subtrees below the `cut` nodes go, in their order.
fn renumber(nodes: Vec<Node>, cut: &[bool]) -> Vec<Node> {
    let mut keep = vec![false; nodes.len()];
    keep[0] = true;
    for t in 0..nodes.len() {
        if keep[t] && !cut[t] {
            for &c in &nodes[t].children {
                keep[c] = true;
            }
        }
    }
    let mut new_id = vec![0; nodes.len()];
    let mut next = 0;
    for t in 0..nodes.len() {
        if keep[t] {
            new_id[t] = next;
            next += 1;
        }
    }
    let mut kept = Vec::new();
    for (t, mut node) in nodes.into_iter().enumerate() {
        if !keep[t] {
            continue;
        }
        node.parent = node.parent.map(|p| new_id[p]);
        if cut[t] {
            node.children.clear();
            node.split = None;
        } else {
            node.children = node.children.iter().map(|&c| new_id[c]).collect();
        }
        kept.push(node);
    }
    kept
}
