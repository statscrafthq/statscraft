//! TWOSTEP CLUSTER, and the two-step clustering that DETECTANOMALY builds its peer groups with.
//!
//! Rules, in our words, from the Command Syntax Reference and the Algorithms manual:
//! - A group of records is kept as its summary: the count, for each continuous variable the
//!   mean and the sum of squared deviations, for each categorical variable the count of each
//!   category. Two summaries add up without the records.
//! - Likelihood distance of groups a and b: for a continuous variable
//!   (-na ln(s + va) - nb ln(s + vb) + n ln(s + vab)) / 2, for a categorical one
//!   -na Ea - nb Eb + n Eab, summed over the variables. v is a group's variance (divided by its
//!   count), E = -sum p ln p over its category shares, n = na + nb, and s is the variance of the
//!   variable over all records (DETECTANOMALY divides it by MLWEIGHT). Euclidean distance is
//!   the distance between the group means.
//! - Pre-clustering reads the records in file order into a tree of nodes of at most MXBRANCH
//!   entries. A record walks down by the closest entry; in the leaf it joins the closest entry
//!   when within the threshold (INITHRESHOLD, 0 at first), or starts a new entry. A node with
//!   too many entries splits around its farthest pair, each other entry going to the closer
//!   one. A tree deeper than MXLEVEL is full: it is rebuilt from its leaf entries with a larger
//!   threshold. With HANDLENOISE p, a full tree first sets aside the leaf entries smaller than
//!   p percent of the largest; after each rebuild and at the end those that fit an entry within
//!   the threshold join it, the rest are noise.
//! - The leaf entries are then joined two at a time, the closest pair first, down to one
//!   cluster.
//! - Automatic count from 1 to the maximum: BIC(J) = -2 sum xi_j + m J ln N with
//!   xi = -n (sum 1/2 ln(s + v) + sum E) and m = 2 KA + sum (L - 1); AIC puts 2 m J in place of
//!   m J ln N. dIC(J) = IC(J) - IC(J + 1). One cluster when dIC(1) < 0; else the first estimate
//!   is the smallest J with dIC(J) / dIC(1) < 0.04. Then for J from that estimate down to 2,
//!   R(J) is the distance of the join from J to J - 1 clusters over that from J + 1 to J. The
//!   largest R wins when it exceeds 1.15 times the second largest; else the larger J of the two.
//! - Every record then goes to its closest cluster. With noise handling (on a tree that
//!   filled), a record whose distance is not below C is noise, cluster -1: C = ln of the product
//!   of the continuous ranges and category counts, or for Euclidean distance twice the root of
//!   the mean of the within-cluster variances.
//! - INFILE updates a saved tree: the leaf entries of the STATE file are put into the tree first
//!   and the cases of the active file follow, at the saved threshold, branch and level limits;
//!   the distance and the scales of the standardized variables and the categories come from the
//!   file too, and CRITERIA, DISTANCE, HANDLENOISE and MEMALLOCATE are ignored. A category the
//!   file does not hold is an error. The variance term of the likelihood distance is that of
//!   all records, the saved ones and the new, taken from their summaries. The tables describe
//!   the cases of the active file, clustered by the updated model; the saved records do not exist.
//! - Continuous variables are standardized (mean 0, standard deviation 1 with n - 1) unless
//!   NOSTANDARDIZE names them. Records missing on any variable are left out; the weight is not
//!   used.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::npar_tests::text;
use crate::{Cell, Engine, Error, Table};
use serde_json::json;
use statscraft_data::{Measure, Values, Variable};
use statscraft_syntax::{ClusterCount, Twostep};

/// The summary of a group of records.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cf {
    pub(crate) n: f64,
    pub(crate) mean: Vec<f64>,
    /// Sums of squared deviations from the mean.
    pub(crate) ss: Vec<f64>,
    pub(crate) counts: Vec<Vec<f64>>,
}

impl Cf {
    pub(crate) fn record(x: &[f64], categories: &[usize], levels: &[usize]) -> Cf {
        let counts = categories.iter().zip(levels).map(|(&c, &l)| {
            let mut v = vec![0.0; l];
            v[c] = 1.0;
            v
        });
        Cf {
            n: 1.0,
            mean: x.to_vec(),
            ss: vec![0.0; x.len()],
            counts: counts.collect(),
        }
    }

    pub(crate) fn add(&mut self, other: &Cf) {
        let n = self.n + other.n;
        for k in 0..self.mean.len() {
            let d = other.mean[k] - self.mean[k];
            self.ss[k] += other.ss[k] + d * d * (self.n * other.n) / n;
            self.mean[k] += d * other.n / n;
        }
        for (a, b) in self.counts.iter_mut().zip(&other.counts) {
            for (x, y) in a.iter_mut().zip(b) {
                *x += y;
            }
        }
        self.n = n;
    }
}

/// n times the entropy of the category counts: -sum c ln(c / n).
fn n_entropy(counts: &[f64], n: f64) -> f64 {
    -counts
        .iter()
        .filter(|&&c| c > 0.0)
        .map(|&c| c * (c / n).ln())
        .sum::<f64>()
}

/// How distances are measured.
pub(crate) struct Model {
    /// For each continuous variable the term added to the variances.
    pub(crate) adjust: Vec<f64>,
    pub(crate) euclidean: bool,
}

impl Model {
    /// The share of variable `k` (continuous first) in the likelihood distance of a and b.
    pub(crate) fn term(&self, k: usize, a: &Cf, b: &Cf) -> f64 {
        let n = a.n + b.n;
        let continuous = self.adjust.len();
        if k < continuous {
            let s = self.adjust[k];
            let d = b.mean[k] - a.mean[k];
            let ss = a.ss[k] + b.ss[k] + d * d * (a.n * b.n) / n;
            let la = a.n * (s + a.ss[k] / a.n).ln();
            let lb = b.n * (s + b.ss[k] / b.n).ln();
            (-la - lb + n * (s + ss / n).ln()) / 2.0
        } else {
            let (ca, cb) = (&a.counts[k - continuous], &b.counts[k - continuous]);
            let both = -ca
                .iter()
                .zip(cb)
                .map(|(x, y)| x + y)
                .filter(|&c| c > 0.0)
                .map(|c| c * (c / n).ln())
                .sum::<f64>();
            -n_entropy(ca, a.n) - n_entropy(cb, b.n) + both
        }
    }

    pub(crate) fn distance(&self, a: &Cf, b: &Cf) -> f64 {
        if self.euclidean {
            let squares = a.mean.iter().zip(&b.mean).map(|(x, y)| (x - y) * (x - y));
            return squares.sum::<f64>().sqrt();
        }
        (0..self.adjust.len() + a.counts.len())
            .map(|k| self.term(k, a, b))
            .sum()
    }

    /// The log-likelihood of a group, xi.
    pub(crate) fn xi(&self, a: &Cf) -> f64 {
        let spread: f64 = (0..self.adjust.len())
            .map(|k| 0.5 * (self.adjust[k] + a.ss[k] / a.n).ln())
            .sum();
        let entropy: f64 = a.counts.iter().map(|c| n_entropy(c, a.n) / a.n).sum();
        -a.n * (spread + entropy)
    }
}

/// The settings of the pre-clustering tree.
pub(crate) struct Settings {
    pub(crate) threshold: f64,
    pub(crate) branch: usize,
    pub(crate) levels: usize,
    /// A leaf entry smaller than this share of the largest is set aside when the tree fills;
    /// 0 for no noise handling.
    pub(crate) noise: f64,
}

struct Entry {
    cf: Cf,
    child: usize,
}

struct Node {
    leaf: bool,
    entries: Vec<Entry>,
}

struct Tree<'m> {
    model: &'m Model,
    nodes: Vec<Node>,
    root: usize,
    /// Joins from the root to the leaves.
    depth: usize,
    threshold: f64,
    branch: usize,
}

impl<'m> Tree<'m> {
    fn new(model: &'m Model, threshold: f64, branch: usize) -> Tree<'m> {
        let root = Node {
            leaf: true,
            entries: Vec::new(),
        };
        Tree {
            model,
            nodes: vec![root],
            root: 0,
            depth: 0,
            threshold,
            branch,
        }
    }

    /// The closest entry of a node, the first of equally close ones.
    fn closest(&self, node: usize, cf: &Cf) -> Option<(usize, f64)> {
        let mut best: Option<(usize, f64)> = None;
        for (i, entry) in self.nodes[node].entries.iter().enumerate() {
            let d = self.model.distance(&entry.cf, cf);
            if best.is_none_or(|(_, b)| d < b) {
                best = Some((i, d));
            }
        }
        best
    }

    /// The way down by the closest entries: (node, entry) pairs, then the leaf.
    fn descend(&self, cf: &Cf) -> (Vec<(usize, usize)>, usize) {
        let mut path = Vec::new();
        let mut node = self.root;
        while !self.nodes[node].leaf {
            let (i, _) = self.closest(node, cf).expect("inner nodes have entries");
            path.push((node, i));
            node = self.nodes[node].entries[i].child;
        }
        (path, node)
    }

    /// Joins `cf` to its closest leaf entry when that lies within the threshold.
    fn absorb(&mut self, cf: &Cf) -> bool {
        let (path, leaf) = self.descend(cf);
        match self.closest(leaf, cf) {
            Some((i, d)) if d <= self.threshold => {
                self.nodes[leaf].entries[i].cf.add(cf);
                for (node, j) in path {
                    self.nodes[node].entries[j].cf.add(cf);
                }
                true
            }
            _ => false,
        }
    }

    fn insert(&mut self, cf: Cf) {
        let (path, leaf) = self.descend(&cf);
        match self.closest(leaf, &cf) {
            Some((i, d)) if d <= self.threshold => self.nodes[leaf].entries[i].cf.add(&cf),
            _ => {
                for &(node, j) in &path {
                    self.nodes[node].entries[j].cf.add(&cf);
                }
                self.nodes[leaf].entries.push(Entry {
                    cf,
                    child: usize::MAX,
                });
                self.split_up(path, leaf);
                return;
            }
        }
        for (node, j) in path {
            self.nodes[node].entries[j].cf.add(&cf);
        }
    }

    fn total(&self, node: usize) -> Cf {
        let entries = &self.nodes[node].entries;
        let mut cf = entries[0].cf.clone();
        for entry in &entries[1..] {
            cf.add(&entry.cf);
        }
        cf
    }

    /// Splits the overfull nodes from `node` up the path.
    fn split_up(&mut self, path: Vec<(usize, usize)>, mut node: usize) {
        let mut level = path.len();
        while self.nodes[node].entries.len() > self.branch {
            let new = self.split(node);
            let (left, right) = (self.total(node), self.total(new));
            if level == 0 {
                self.nodes.push(Node {
                    leaf: false,
                    entries: vec![
                        Entry {
                            cf: left,
                            child: node,
                        },
                        Entry {
                            cf: right,
                            child: new,
                        },
                    ],
                });
                self.root = self.nodes.len() - 1;
                self.depth += 1;
                return;
            }
            level -= 1;
            let (parent, i) = path[level];
            self.nodes[parent].entries[i].cf = left;
            let entry = Entry {
                cf: right,
                child: new,
            };
            self.nodes[parent].entries.insert(i + 1, entry);
            node = parent;
        }
    }

    /// Moves the entries of `node` closer to the second of its farthest pair into a new node.
    fn split(&mut self, node: usize) -> usize {
        let entries = std::mem::take(&mut self.nodes[node].entries);
        let d = |i: usize, j: usize| self.model.distance(&entries[i].cf, &entries[j].cf);
        let (mut first, mut second, mut far) = (0, 1, f64::NEG_INFINITY);
        for i in 0..entries.len() {
            for j in i + 1..entries.len() {
                let dij = d(i, j);
                if dij > far {
                    (first, second, far) = (i, j, dij);
                }
            }
        }
        let right: Vec<bool> = (0..entries.len())
            .map(|k| k == second || (k != first && d(k, second) < d(k, first)))
            .collect();
        let leaf = self.nodes[node].leaf;
        let (mut keep, mut moved) = (Vec::new(), Vec::new());
        for (entry, right) in entries.into_iter().zip(right) {
            match right {
                true => moved.push(entry),
                false => keep.push(entry),
            }
        }
        self.nodes[node].entries = keep;
        self.nodes.push(Node {
            leaf,
            entries: moved,
        });
        self.nodes.len() - 1
    }

    fn leaves(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack = vec![self.root];
        while let Some(node) = stack.pop() {
            if self.nodes[node].leaf {
                out.push(node);
            } else {
                stack.extend(self.nodes[node].entries.iter().rev().map(|e| e.child));
            }
        }
        out
    }

    fn leaf_entries(&self) -> Vec<Cf> {
        let leaves = self.leaves();
        let entries = leaves.iter().flat_map(|&n| &self.nodes[n].entries);
        entries.map(|e| e.cf.clone()).collect()
    }

    /// The next threshold: the median over the leaf nodes of the distance of their closest
    /// pair of entries, at least twice the present threshold.
    fn next_threshold(&self) -> f64 {
        let mut closest: Vec<f64> = Vec::new();
        for leaf in self.leaves() {
            let entries = &self.nodes[leaf].entries;
            let mut best = f64::INFINITY;
            for i in 0..entries.len() {
                for j in i + 1..entries.len() {
                    best = best.min(self.model.distance(&entries[i].cf, &entries[j].cf));
                }
            }
            if best.is_finite() {
                closest.push(best);
            }
        }
        closest.sort_by(f64::total_cmp);
        let median = closest.get(closest.len().saturating_sub(1) / 2).copied();
        let next = median.unwrap_or(0.0).max(2.0 * self.threshold);
        match next > self.threshold {
            true => next,
            false => self.threshold + f64::EPSILON.max(self.threshold),
        }
    }
}

/// The result of pre-clustering.
pub(crate) struct Preclusters {
    pub(crate) subclusters: Vec<Cf>,
    /// Whether the tree ever filled.
    pub(crate) filled: bool,
    /// The threshold of the final tree.
    pub(crate) threshold: f64,
}

/// Reads the records in order into the tree.
pub(crate) fn precluster(model: &Model, records: Vec<Cf>, settings: &Settings) -> Preclusters {
    let mut tree = Tree::new(model, settings.threshold, settings.branch);
    let mut outliers: Vec<Cf> = Vec::new();
    let mut filled = false;
    for record in records {
        tree.insert(record);
        while tree.depth > settings.levels {
            filled = true;
            let threshold = tree.next_threshold();
            // distances that are not numbers never fall under a threshold: stop growing it
            if !threshold.is_finite() {
                break;
            }
            let mut entries = tree.leaf_entries();
            if settings.noise > 0.0 {
                let largest = entries.iter().map(|e| e.n).fold(0.0, f64::max);
                let (small, kept): (Vec<Cf>, Vec<Cf>) = entries
                    .into_iter()
                    .partition(|e| e.n < settings.noise * largest);
                outliers.extend(small);
                entries = kept;
            }
            tree = Tree::new(model, threshold, settings.branch);
            for entry in entries {
                tree.insert(entry);
            }
            if tree.depth <= settings.levels && settings.noise > 0.0 {
                outliers.retain(|o| !tree.absorb(o));
            }
        }
    }
    if settings.noise > 0.0 {
        outliers.retain(|o| !tree.absorb(o));
    }
    Preclusters {
        subclusters: tree.leaf_entries(),
        filled,
        threshold: tree.threshold,
    }
}

/// Joins the subclusters two at a time, the closest pair first (on equal distances the pair
/// with the lowest numbers). Each join is (kept, joined, distance); the kept one is the lower.
pub(crate) fn agglomerate(model: &Model, subclusters: &[Cf]) -> Vec<(usize, usize, f64)> {
    let n = subclusters.len();
    let mut clusters: Vec<Option<Cf>> = subclusters.iter().cloned().map(Some).collect();
    let scan = |clusters: &[Option<Cf>], i: usize| -> (usize, f64) {
        let mut best = (usize::MAX, f64::INFINITY);
        let a = clusters[i].as_ref().expect("active");
        for (j, b) in clusters.iter().enumerate() {
            if let Some(b) = b.as_ref().filter(|_| j != i) {
                let d = model.distance(a, b);
                if d < best.1 || best.0 == usize::MAX {
                    best = (j, d);
                }
            }
        }
        best
    };
    let mut nearest: Vec<(usize, f64)> = (0..n).map(|i| scan(&clusters, i)).collect();
    let mut joins = Vec::new();
    for _ in 1..n {
        let mut i = usize::MAX;
        for k in (0..n).filter(|&k| clusters[k].is_some()) {
            if i == usize::MAX || nearest[k].1 < nearest[i].1 {
                i = k;
            }
        }
        let (j, d) = nearest[i];
        let (a, b) = (i.min(j), i.max(j));
        let joined = clusters[b].take().expect("active");
        clusters[a].as_mut().expect("active").add(&joined);
        joins.push((a, b, d));
        nearest[a] = scan(&clusters, a);
        for k in (0..n).filter(|&k| k != a && clusters[k].is_some()) {
            if nearest[k].0 == a || nearest[k].0 == b {
                nearest[k] = scan(&clusters, k);
            } else {
                let dk = model.distance(
                    clusters[k].as_ref().expect("active"),
                    clusters[a].as_ref().expect("active"),
                );
                if dk < nearest[k].1 || (dk == nearest[k].1 && a < nearest[k].0) {
                    nearest[k] = (a, dk);
                }
            }
        }
    }
    joins
}

/// The clusters after the joins that leave `count` of them, in the order of their first
/// subcluster.
pub(crate) fn clusters_at(
    subclusters: &[Cf],
    joins: &[(usize, usize, f64)],
    count: usize,
) -> Vec<Cf> {
    let mut clusters: Vec<Option<Cf>> = subclusters.iter().cloned().map(Some).collect();
    for &(a, b, _) in &joins[..subclusters.len() - count] {
        let joined = clusters[b].take().expect("active");
        clusters[a].as_mut().expect("active").add(&joined);
    }
    clusters.into_iter().flatten().collect()
}

/// The information criterion for 1 to `max` clusters, and the count it chooses.
pub(crate) fn auto_count(
    model: &Model,
    subclusters: &[Cf],
    joins: &[(usize, usize, f64)],
    max: usize,
    aic: bool,
) -> (Vec<f64>, usize) {
    let max = max.min(subclusters.len());
    let records: f64 = subclusters.iter().map(|c| c.n).sum();
    let first = &subclusters[0];
    let per_cluster = 2.0 * model.adjust.len() as f64
        + first
            .counts
            .iter()
            .map(|c| c.len() as f64 - 1.0)
            .sum::<f64>();
    let penalty = match aic {
        true => 2.0,
        false => records.ln(),
    };
    let ic: Vec<f64> = (1..=max)
        .map(|j| {
            let xi: f64 = clusters_at(subclusters, joins, j)
                .iter()
                .map(|c| model.xi(c))
                .sum();
            -2.0 * xi + per_cluster * j as f64 * penalty
        })
        .collect();
    let chosen = choose(&ic, subclusters.len(), joins);
    (ic, chosen)
}

/// The distance of the join from `j` clusters to `j - 1`.
pub(crate) fn join_distance(subclusters: usize, joins: &[(usize, usize, f64)], j: usize) -> f64 {
    joins[subclusters - j].2
}

/// The distance ratio of `j` clusters, when the hierarchy has `j + 1`.
pub(crate) fn distance_ratio(
    subclusters: usize,
    joins: &[(usize, usize, f64)],
    j: usize,
) -> Option<f64> {
    (j >= 2 && j < subclusters)
        .then(|| join_distance(subclusters, joins, j) / join_distance(subclusters, joins, j + 1))
}

fn choose(ic: &[f64], subclusters: usize, joins: &[(usize, usize, f64)]) -> usize {
    let max = ic.len();
    if max < 2 {
        return 1;
    }
    let change = |j: usize| ic[j - 1] - ic[j];
    if change(1) <= 0.0 {
        return 1;
    }
    let estimate = (1..max)
        .find(|&j| change(j) / change(1) < 0.04)
        .unwrap_or(max);
    let mut ratios: Vec<(usize, f64)> = (2..=estimate)
        .filter_map(|j| distance_ratio(subclusters, joins, j).map(|r| (j, r)))
        .collect();
    // largest ratio first; on equal ratios the larger count
    ratios.sort_by(|a, b| b.1.total_cmp(&a.1).then(b.0.cmp(&a.0)));
    match ratios[..] {
        [] => estimate,
        [(j, _)] => j,
        [(j1, r1), (_, r2), ..] if r1 > 1.15 * r2 => j1,
        [(j1, _), (j2, _), ..] => j1.max(j2),
    }
}

/// The variance (with n - 1) of the values.
pub(crate) fn variance(values: &[f64]) -> f64 {
    let n = values.len() as f64;
    let mean = values.iter().sum::<f64>() / n;
    values.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1.0)
}

/// The value of a numeric variable for a case, None when missing.
pub(crate) fn numeric_value(variable: &Variable, case: usize, include: bool) -> Option<f64> {
    match variable.values() {
        Values::Numeric(values) => values
            .get(case)
            .filter(|v| include || !variable.is_user_missing(*v)),
        Values::String(_) => None,
    }
}

/// The distinct categories in order, and the index of each case's category.
pub(crate) fn category_codes(keys: &[Option<Key>]) -> (Vec<Key>, Vec<Option<usize>>) {
    let mut levels: Vec<Key> = Vec::new();
    for key in keys.iter().flatten() {
        if !levels.iter().any(|l| l.compare(key).is_eq()) {
            levels.push(key.clone());
        }
    }
    levels.sort_by(|a, b| a.compare(b));
    let codes = keys
        .iter()
        .map(|k| {
            k.as_ref().map(|k| {
                levels
                    .iter()
                    .position(|l| l.compare(k).is_eq())
                    .expect("listed")
            })
        })
        .collect();
    (levels, codes)
}

/// The first `root`, `root_1`, `root_2` ... that `taken` does not hold.
pub(crate) fn free_name(root: &str, taken: impl Fn(&str) -> bool) -> String {
    std::iter::once(root.to_string())
        .chain((1..).map(|i| format!("{root}_{i}")))
        .find(|n| !taken(n))
        .expect("a free name")
}

/// Numbers the clusters of the records by their first record: each record's number from 1, or
/// None for noise, and the cluster index of each number.
pub(crate) fn number_by_first(assigned: &[Option<usize>]) -> (Vec<Option<usize>>, Vec<usize>) {
    let mut order: Vec<usize> = Vec::new();
    let numbers = assigned
        .iter()
        .map(|a| {
            a.map(|j| match order.iter().position(|&o| o == j) {
                Some(p) => p + 1,
                None => {
                    order.push(j);
                    order.len()
                }
            })
        })
        .collect();
    (numbers, order)
}

/// A category value as JSON: a number or the text without trailing blanks.
pub(crate) fn key_json(key: &Key) -> serde_json::Value {
    match key {
        Key::Number(x) => json!(x),
        Key::Text(t) => json!(t.trim_end()),
    }
}

/// A summary in the units the clustering used (standardized where it standardizes).
pub(crate) fn cf_json(cf: &Cf) -> serde_json::Value {
    json!({"n": cf.n, "mean": cf.mean, "ss": cf.ss, "counts": cf.counts})
}

/// What a TWOSTEP STATE file holds.
struct SavedState {
    euclidean: bool,
    /// Mean and standard deviation of every continuous variable that was standardized.
    scales: Vec<Option<(f64, f64)>>,
    /// Name and categories of every categorical variable.
    categories: Vec<(String, Vec<Key>)>,
    threshold: f64,
    branch: usize,
    levels: usize,
    subclusters: Vec<Cf>,
}

impl Engine {
    /// Reads the STATE file `file` that INFILE updates.
    fn read_twostep_state(&self, file: &str) -> Result<SavedState, Error> {
        let path = self.path(file);
        let text = std::fs::read_to_string(&path).map_err(|e| {
            Error(format!(
                "TWOSTEP CLUSTER cannot read the INFILE {}: {e}",
                path.display()
            ))
        })?;
        let bad = |what: &str| {
            Error(format!(
                "TWOSTEP CLUSTER /INFILE {}: {what}; it must be a STATE file this program wrote",
                path.display()
            ))
        };
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|_| bad("it is not readable"))?;
        if value["format"] != "statscraft-twostep-state" {
            return Err(bad("it is not a TWOSTEP state file"));
        }
        let number = |v: &serde_json::Value| v.as_f64().ok_or_else(|| bad("a number is missing"));
        let numbers = |v: &serde_json::Value| -> Result<Vec<f64>, Error> {
            v.as_array()
                .ok_or_else(|| bad("a list is missing"))?
                .iter()
                .map(number)
                .collect()
        };
        let list = |v: &serde_json::Value| {
            v.as_array()
                .cloned()
                .ok_or_else(|| bad("a list is missing"))
        };
        let scales = list(&value["continuous"])?
            .iter()
            .map(|c| match (c["mean"].as_f64(), c["sd"].as_f64()) {
                (Some(m), Some(sd)) if sd > 0.0 => Ok(Some((m, sd))),
                (None, None) => Ok(None),
                _ => Err(bad("a scale is incomplete")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let categories = list(&value["categorical"])?
            .iter()
            .map(|c| {
                let keys = list(&c["categories"])?
                    .iter()
                    .map(|k| match (k.as_f64(), k.as_str()) {
                        (Some(x), _) => Ok(Key::Number(x)),
                        (_, Some(t)) => Ok(Key::Text(t.to_string())),
                        _ => Err(bad("a category is neither a number nor text")),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((c["name"].as_str().unwrap_or("").to_string(), keys))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let subclusters = list(&value["subclusters"])?
            .iter()
            .map(|s| {
                let counts = list(&s["counts"])?
                    .iter()
                    .map(numbers)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(Cf {
                    n: number(&s["n"])?,
                    mean: numbers(&s["mean"])?,
                    ss: numbers(&s["ss"])?,
                    counts,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        if subclusters.is_empty() {
            return Err(bad("it holds no subclusters"));
        }
        for cf in &subclusters {
            let fits = cf.n > 0.0
                && cf.mean.len() == scales.len()
                && cf.ss.len() == scales.len()
                && cf.counts.len() == categories.len()
                && cf
                    .counts
                    .iter()
                    .zip(&categories)
                    .all(|(c, (_, levels))| c.len() == levels.len());
            if !fits {
                return Err(bad(
                    "a subcluster does not fit the variables and categories it names",
                ));
            }
        }
        let whole = |v: &serde_json::Value| {
            v.as_u64()
                .map(|n| n as usize)
                .filter(|&n| n >= 2)
                .ok_or_else(|| bad("a tree limit is missing"))
        };
        Ok(SavedState {
            euclidean: value["distance"] == "euclidean",
            scales,
            categories,
            threshold: number(&value["threshold"])?,
            branch: whole(&value["max_branch"])?,
            levels: whole(&value["max_level"])?,
            subclusters,
        })
    }

    /// Writes a model file of the command `who` as pretty JSON.
    pub(crate) fn write_json(
        &self,
        who: &str,
        file: &str,
        what: &str,
        value: &serde_json::Value,
    ) -> Result<(), Error> {
        let path = self.path(file);
        let text = serde_json::to_string_pretty(value).expect("a model serializes");
        std::fs::write(&path, text + "\n").map_err(|e| {
            Error(format!(
                "{who} cannot write the {what} file {}: {e}",
                path.display()
            ))
        })
    }
}

/// A table without column groups.
pub(crate) fn plain_table(
    title: &str,
    row_headers: usize,
    columns: &[impl AsRef<str>],
    rows: Vec<Vec<Cell>>,
) -> Table {
    Table {
        title: title.into(),
        row_headers,
        columns: columns.iter().map(|c| c.as_ref().to_string()).collect(),
        column_groups: vec![Vec::new(); columns.len()],
        rows,
        ..Table::default()
    }
}

/// Counts and shares of the groups, the noise (when any), all of them, the cases left out and
/// the total.
pub(crate) fn distribution(
    title: &str,
    label: &str,
    sizes: &[f64],
    noise: f64,
    cases: f64,
) -> Table {
    let combined = sizes.iter().sum::<f64>() + noise;
    let row = |head: [Cell; 2], n: f64, share: Option<f64>| {
        let mut row = head.to_vec();
        row.extend([
            Cell::Number(n),
            number(share),
            Cell::Number(100.0 * n / cases),
        ]);
        row
    };
    let mut rows: Vec<Vec<Cell>> = sizes
        .iter()
        .enumerate()
        .map(|(j, &n)| {
            let first = if j == 0 { text(label) } else { Cell::Empty };
            row(
                [first, Cell::Number((j + 1) as f64)],
                n,
                Some(100.0 * n / combined),
            )
        })
        .collect();
    if noise > 0.0 {
        rows.push(row(
            [Cell::Empty, text("Outlier (-1)")],
            noise,
            Some(100.0 * noise / combined),
        ));
    }
    rows.push(row([text("Combined"), Cell::Empty], combined, Some(100.0)));
    rows.push(row(
        [text("Excluded Cases"), Cell::Empty],
        cases - combined,
        None,
    ));
    rows.push(row([text("Total"), Cell::Empty], cases, None));
    let columns = ["", "", "N", "% of Combined", "% of Total"];
    plain_table(title, 2, &columns, rows)
}

/// Mean and standard deviation (with n - 1) of the values.
pub(crate) fn mean_sd(values: &[f64]) -> (Option<f64>, Option<f64>) {
    let n = values.len() as f64;
    if values.is_empty() {
        return (None, None);
    }
    // squares of values near 1e200 or 1e-200 overflow or vanish: work on the values over their
    // largest, then give the scale back
    let largest = values.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    if largest.is_finite() && !(1e-100..=1e100).contains(&largest) {
        let scaled: Vec<f64> = values.iter().map(|v| v / largest).collect();
        let mean = scaled.iter().sum::<f64>() / n * largest;
        let sd = (values.len() > 1).then(|| variance(&scaled).sqrt() * largest);
        return (Some(mean), sd);
    }
    let mean = values.iter().sum::<f64>() / n;
    let sd = (values.len() > 1).then(|| variance(values).sqrt());
    (Some(mean), sd)
}

impl Engine {
    pub(crate) fn twostep(&mut self, command: &Twostep) -> Result<Vec<Table>, Error> {
        let saved = command
            .infile
            .as_ref()
            .map(|file| self.read_twostep_state(file))
            .transpose()?;
        let outfile = command.model_file.is_some() || command.state_file.is_some();
        if outfile && self.split_group.is_some() {
            return Err(Error(
                "TWOSTEP CLUSTER /OUTFILE is not available with SPLIT FILE; turn SPLIT FILE off"
                    .into(),
            ));
        }
        let data = self.active()?;
        let unique = |mut names: Vec<String>| {
            let mut seen: Vec<String> = Vec::new();
            names.retain(|n| {
                let fresh = !seen.iter().any(|s| s.eq_ignore_ascii_case(n));
                seen.push(n.clone());
                fresh
            });
            names
        };
        let continuous_names = unique(self.expand(&command.continuous)?);
        let categorical_names = unique(self.expand(&command.categorical)?);
        if let Some(both) = continuous_names
            .iter()
            .find(|n| categorical_names.iter().any(|c| c.eq_ignore_ascii_case(n)))
        {
            return Err(Error(format!(
                "TWOSTEP CLUSTER: {both} is both continuous and categorical"
            )));
        }
        let unstandardized = self.expand(&command.unstandardized)?;
        if let Some(name) = unstandardized
            .iter()
            .find(|n| !continuous_names.iter().any(|c| c.eq_ignore_ascii_case(n)))
        {
            return Err(Error(format!(
                "TWOSTEP CLUSTER /NOSTANDARDIZE names {name}, which is not a continuous variable"
            )));
        }
        let mut continuous: Vec<Variable> = continuous_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = continuous.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "TWOSTEP CLUSTER: continuous variable {} is a string variable",
                v.name()
            )));
        }
        let categorical: Vec<Variable> = categorical_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let mut warnings = Vec::new();
        if let Some(state) = &saved {
            if continuous.len() != state.scales.len() || categorical.len() != state.categories.len()
            {
                return Err(Error(format!(
                    "TWOSTEP CLUSTER /INFILE: the saved model has {} continuous and {} categorical variables, but the command names {} and {}; name the variables in the order of the file",
                    state.scales.len(),
                    state.categories.len(),
                    continuous.len(),
                    categorical.len()
                )));
            }
            if command.euclidean != state.euclidean {
                warnings.push(
                    "TWOSTEP CLUSTER /INFILE: DISTANCE is ignored; the distance of the saved model is used"
                        .to_string(),
                );
            }
        }
        if case_weights(data)?.is_some() {
            warnings.push("TWOSTEP CLUSTER does not use the weight variable".to_string());
        }
        if !command.evaluation.is_empty() {
            warnings.push(
                "TWOSTEP CLUSTER: evaluation fields belong to the interactive model view and are not used"
                    .to_string(),
            );
        }
        let auto = match command.count {
            ClusterCount::Auto { max, aic } => Some((max, aic)),
            ClusterCount::Fixed(_) => None,
        };
        if command.print_ic && auto.is_none() {
            warnings.push(
                "TWOSTEP CLUSTER /PRINT IC needs NUMCLUSTERS AUTO; it is skipped".to_string(),
            );
        }
        let print_ic = command.print_ic && auto.is_some_and(|(max, _)| max > 1);
        if !command.view
            && !command.print_count
            && !command.print_summary
            && !print_ic
            && command.save.is_none()
            && !outfile
        {
            return Err(Error(
                "TWOSTEP CLUSTER has nothing to show or save: give PRINT, SAVE, OUTFILE or VIEWMODEL DISPLAY=YES"
                    .into(),
            ));
        }

        // the cases valid on every variable
        let cases = data.case_count();
        let keys: Vec<Vec<Option<Key>>> = categorical
            .iter()
            .map(|v| categories(v, command.include))
            .collect();
        let used: Vec<usize> = (0..cases)
            .filter(|&case| {
                continuous
                    .iter()
                    .all(|v| numeric_value(v, case, command.include).is_some())
                    && keys.iter().all(|k| k[case].is_some())
            })
            .collect();
        let needed = if saved.is_some() { 1 } else { 2 };
        if used.len() < needed {
            return Err(Error(format!(
                "TWOSTEP CLUSTER needs at least {needed} case{} without missing values, found {}",
                if needed == 1 { "" } else { "s" },
                used.len()
            )));
        }
        let mut raw: Vec<Vec<f64>> = continuous
            .iter()
            .map(|v| {
                used.iter()
                    .map(|&c| numeric_value(v, c, command.include).expect("valid"))
                    .collect()
            })
            .collect();
        // a constant continuous variable has no spread to measure
        let mut k = 0;
        while saved.is_none() && k < continuous.len() {
            // by the standard deviation: the variance of values near 1e-200 is below the smallest number
            if mean_sd(&raw[k]).1.is_some_and(|sd| sd > 0.0) {
                k += 1;
            } else {
                warnings.push(format!(
                    "TWOSTEP CLUSTER leaves out {}: it has the same value in every case",
                    continuous[k].name()
                ));
                continuous.remove(k);
                raw.remove(k);
            }
        }
        if continuous.is_empty() && categorical.is_empty() {
            return Err(Error(
                "TWOSTEP CLUSTER has no variable left to cluster".into(),
            ));
        }
        // the mean and standard deviation of each standardized variable
        let scales: Vec<Option<(f64, f64)>> = match &saved {
            Some(state) => state.scales.clone(),
            None => continuous
                .iter()
                .zip(&raw)
                .map(|(v, values)| {
                    let keep = unstandardized
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(v.name()));
                    let (mean, sd) = mean_sd(values);
                    (!keep).then(|| (mean.expect("cases"), sd.expect("two cases")))
                })
                .collect(),
        };
        let transformed: Vec<Vec<f64>> = raw
            .iter()
            .zip(&scales)
            .map(|(values, scale)| match scale {
                Some((mean, sd)) => values.iter().map(|x| (x - mean) / sd).collect(),
                None => values.clone(),
            })
            .collect();
        let coded: Vec<(Vec<Key>, Vec<Option<usize>>)> = match &saved {
            Some(state) => {
                let mut out = Vec::new();
                for (v, (k, (name, levels))) in
                    categorical.iter().zip(keys.iter().zip(&state.categories))
                {
                    let mut codes = Vec::with_capacity(used.len());
                    for &c in &used {
                        let key = k[c].as_ref().expect("valid");
                        let at = levels.iter().position(|l| l.compare(key).is_eq());
                        let Some(at) = at else {
                            return Err(Error(format!(
                                "TWOSTEP CLUSTER /INFILE: {} has a category the saved model does not hold (it knows the categories of {name})",
                                v.name()
                            )));
                        };
                        codes.push(Some(at));
                    }
                    out.push((levels.clone(), codes));
                }
                out
            }
            None => keys
                .iter()
                .map(|k| category_codes(&used.iter().map(|&c| k[c].clone()).collect::<Vec<_>>()))
                .collect(),
        };
        let levels: Vec<usize> = coded.iter().map(|(l, _)| l.len()).collect();
        let records: Vec<Cf> = (0..used.len())
            .map(|r| {
                let x: Vec<f64> = transformed.iter().map(|t| t[r]).collect();
                let c: Vec<usize> = coded
                    .iter()
                    .map(|(_, codes)| codes[r].expect("valid"))
                    .collect();
                Cf::record(&x, &c, &levels)
            })
            .collect();
        let euclidean = saved.as_ref().map_or(command.euclidean, |s| s.euclidean);
        // the variance of each variable over all records: from the summaries when updating
        let adjust: Vec<f64> = match &saved {
            Some(state) => {
                let mut all: Option<Cf> = None;
                for cf in state.subclusters.iter().chain(&records) {
                    match all.as_mut() {
                        Some(total) => total.add(cf),
                        None => all = Some(cf.clone()),
                    }
                }
                let total = all.expect("a record");
                (0..transformed.len())
                    .map(|k| total.ss[k] / (total.n - 1.0).max(1.0))
                    .collect()
            }
            None => transformed.iter().map(|t| variance(t)).collect(),
        };
        let model = Model { adjust, euclidean };
        let (settings, branch, tree_levels) = match &saved {
            Some(state) => (
                Settings {
                    threshold: state.threshold,
                    branch: state.branch,
                    levels: state.levels,
                    noise: 0.0,
                },
                state.branch,
                state.levels,
            ),
            None => (
                Settings {
                    threshold: command.threshold,
                    branch: command.branch,
                    levels: command.levels,
                    noise: command.noise / 100.0,
                },
                command.branch,
                command.levels,
            ),
        };
        let start: Vec<Cf> = match &saved {
            Some(state) => state
                .subclusters
                .iter()
                .cloned()
                .chain(records.clone())
                .collect(),
            None => records.clone(),
        };
        let pre = precluster(&model, start, &settings);
        let subs = &pre.subclusters;
        let joins = agglomerate(&model, subs);
        let (ic, count) = match (command.count, auto) {
            (_, Some((max, aic))) => auto_count(&model, subs, &joins, max, aic),
            (ClusterCount::Fixed(n), _) => {
                if n > subs.len() {
                    warnings.push(format!(
                        "TWOSTEP CLUSTER finds only {} distinct groups, fewer than the {n} clusters asked for",
                        subs.len()
                    ));
                }
                (Vec::new(), n.min(subs.len()))
            }
            _ => unreachable!("auto is set for AUTO"),
        };
        let clusters = clusters_at(subs, &joins, count);

        // the critical distance of noise
        let noise_limit = (saved.is_none() && command.noise > 0.0 && pre.filled).then(|| {
            if euclidean {
                let within: f64 = clusters
                    .iter()
                    .flat_map(|c| c.ss.iter().map(move |ss| ss / c.n))
                    .sum();
                2.0 * (within / (clusters.len() * continuous.len()) as f64).sqrt()
            } else {
                let ranges: f64 = transformed
                    .iter()
                    .map(|t| {
                        let (lo, hi) = t
                            .iter()
                            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| {
                                (a.min(x), b.max(x))
                            });
                        (hi - lo).ln()
                    })
                    .sum();
                ranges + levels.iter().map(|&l| (l as f64).ln()).sum::<f64>()
            }
        });
        let assigned: Vec<Option<usize>> = records
            .iter()
            .map(|r| {
                let mut best = (0, f64::INFINITY);
                for (j, c) in clusters.iter().enumerate() {
                    let d = model.distance(c, r);
                    if d < best.1 {
                        best = (j, d);
                    }
                }
                match noise_limit {
                    Some(limit) if best.1 >= limit => None,
                    _ => Some(best.0),
                }
            })
            .collect();
        let (numbers, order) = number_by_first(&assigned);
        if order.len() < clusters.len() {
            warnings.push(format!(
                "TWOSTEP CLUSTER: {} of the {} clusters have no case closest to them and are left out",
                clusters.len() - order.len(),
                clusters.len()
            ));
        }
        let k = order.len();
        let noise_count = numbers.iter().filter(|n| n.is_none()).count();
        let shown = self.shown();
        let mut tables = Vec::new();

        if command.view {
            let rows = vec![
                vec![text("Algorithm"), text("TwoStep")],
                vec![
                    text("Inputs"),
                    Cell::Number((continuous.len() + categorical.len()) as f64),
                ],
                vec![text("Clusters"), Cell::Number(k as f64)],
            ];
            tables.push(plain_table("Model Summary", 1, &["", "Value"], rows));
        }
        if print_ic {
            let (_, aic) = auto.expect("print_ic needs AUTO");
            let name = if aic { "AIC" } else { "BIC" };
            let rows = (1..=ic.len())
                .map(|j| {
                    let change = (j >= 2).then(|| ic[j - 1] - ic[j - 2]);
                    let ratio = (j >= 2).then(|| (ic[j - 1] - ic[j - 2]) / (ic[1] - ic[0]));
                    vec![
                        Cell::Number(j as f64),
                        number(Some(ic[j - 1])),
                        number(change),
                        number(ratio),
                        number(distance_ratio(subs.len(), &joins, j)),
                    ]
                })
                .collect();
            let columns = [
                "Number of Clusters",
                name,
                "Change",
                "Ratio of Changes",
                "Ratio of Distance Measures",
            ];
            let mut table = plain_table("Auto-Clustering", 1, &columns, rows);
            table.caption = Some(format!(
                "Change is the {name} less that of one cluster fewer; Ratio of Changes divides it by the change at 2 clusters. Ratio of Distance Measures is the distance of the join to one cluster fewer over that of the join to this number. Chosen: {count} {}.",
                if count == 1 { "cluster" } else { "clusters" }
            ));
            tables.push(table);
        }

        // rows of the summary tables: the clusters, the noise, all
        let mut groups: Vec<(Cell, Vec<usize>)> = (1..=k)
            .map(|j| {
                let members = (0..used.len()).filter(|&r| numbers[r] == Some(j)).collect();
                (Cell::Number(j as f64), members)
            })
            .collect();
        if noise_count > 0 {
            let members = (0..used.len()).filter(|&r| numbers[r].is_none()).collect();
            groups.push((text("Outlier (-1)"), members));
        }
        groups.push((text("Combined"), (0..used.len()).collect()));
        if command.print_count {
            let sizes: Vec<f64> = groups[..k].iter().map(|g| g.1.len() as f64).collect();
            let table = distribution(
                "Cluster Distribution",
                "Cluster",
                &sizes,
                noise_count as f64,
                cases as f64,
            );
            tables.push(table);
        }
        if command.print_summary && !continuous.is_empty() {
            let rows = groups
                .iter()
                .map(|(label, members)| {
                    let mut row = vec![label.clone()];
                    for values in &raw {
                        let picked: Vec<f64> = members.iter().map(|&r| values[r]).collect();
                        let (mean, sd) = mean_sd(&picked);
                        row.extend([number(mean), number(sd)]);
                    }
                    row
                })
                .collect();
            let mut columns = vec![String::new()];
            let mut column_groups = vec![Vec::new()];
            for v in &continuous {
                for stat in ["Mean", "Std. Deviation"] {
                    columns.push(stat.into());
                    column_groups.push(vec![shown.variable(v)]);
                }
            }
            tables.push(Table {
                title: "Centroids".into(),
                row_headers: 1,
                columns,
                column_groups,
                rows,
                ..Table::default()
            });
        }
        if command.print_summary {
            for (v, (cats, codes)) in categorical.iter().zip(&coded) {
                let rows = groups
                    .iter()
                    .map(|(label, members)| {
                        let mut row = vec![label.clone()];
                        for c in 0..cats.len() {
                            let n = members.iter().filter(|&&r| codes[r] == Some(c)).count() as f64;
                            let share =
                                (!members.is_empty()).then(|| 100.0 * n / members.len() as f64);
                            row.extend([Cell::Number(n), number(share)]);
                        }
                        row
                    })
                    .collect();
                let mut columns = vec![String::new()];
                let mut column_groups = vec![Vec::new()];
                for cat in cats {
                    for stat in ["Frequency", "Percent"] {
                        columns.push(stat.into());
                        column_groups.push(vec![shown.value(v, cat)]);
                    }
                }
                tables.push(Table {
                    title: shown.variable(v),
                    row_headers: 1,
                    columns,
                    column_groups,
                    rows,
                    ..Table::default()
                });
            }
        }
        if outfile {
            let continuous: Vec<_> = continuous
                .iter()
                .zip(&scales)
                .zip(&model.adjust)
                .map(|((v, scale), adjust)| {
                    json!({
                        "name": v.name(),
                        "mean": scale.map(|s| s.0),
                        "sd": scale.map(|s| s.1),
                        "variance": adjust,
                    })
                })
                .collect();
            let categorical: Vec<_> = categorical
                .iter()
                .zip(&coded)
                .map(|(v, (cats, _))| {
                    let cats: Vec<_> = cats.iter().map(key_json).collect();
                    json!({"name": v.name(), "categories": cats})
                })
                .collect();
            let distance = if euclidean { "euclidean" } else { "likelihood" };
            if let Some(file) = &command.model_file {
                let found: Vec<_> = order
                    .iter()
                    .enumerate()
                    .map(|(i, &j)| {
                        let cases = numbers.iter().filter(|n| **n == Some(i + 1)).count();
                        json!({"number": i + 1, "cases": cases, "summary": cf_json(&clusters[j])})
                    })
                    .collect();
                let model = json!({
                    "format": "statscraft-twostep-model",
                    "distance": distance,
                    "continuous": continuous,
                    "categorical": categorical,
                    "noise_cases": noise_count,
                    "noise_limit": noise_limit,
                    "clusters": found,
                });
                self.write_json("TWOSTEP CLUSTER", file, "model", &model)?;
            }
            if let Some(file) = &command.state_file {
                let subclusters: Vec<_> = subs.iter().map(cf_json).collect();
                let state = json!({
                    "format": "statscraft-twostep-state",
                    "distance": distance,
                    "continuous": continuous,
                    "categorical": categorical,
                    "threshold": pre.threshold,
                    "max_branch": branch,
                    "max_level": tree_levels,
                    "subclusters": subclusters,
                });
                self.write_json("TWOSTEP CLUSTER", file, "state", &state)?;
            }
        }
        for warning in warnings {
            self.warn(warning);
        }
        if let Some(name) = &command.save {
            let mut values = vec![None; cases];
            for (r, &case) in used.iter().enumerate() {
                values[case] = Some(numbers[r].map_or(-1.0, |n| n as f64));
            }
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                let name = match name {
                    Some(name) if data.variable_index(name).is_some() => {
                        return Err(Error(format!(
                            "TWOSTEP CLUSTER /SAVE: variable {name} already exists; choose a new name"
                        )));
                    }
                    Some(name) => name.clone(),
                    None => (1..)
                        .map(|n| format!("TSC_{n}"))
                        .find(|n| data.variable_index(n).is_none())
                        .expect("a free name"),
                };
                data.set_numeric_values(&name, values)?;
                data.set_variable_label(&name, Some("TwoStep cluster number"))?;
                data.set_format(&name, "F8.0")?;
                data.set_measure(&name, Measure::Nominal)?;
                Ok(())
            })?;
        }
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(adjust: Vec<f64>) -> Model {
        Model {
            adjust,
            euclidean: false,
        }
    }

    #[test]
    fn distance_is_the_loss_of_log_likelihood_and_symmetric() {
        let m = model(vec![1.0]);
        let levels = [3];
        let a = Cf::record(&[0.5], &[0], &levels);
        let mut b = Cf::record(&[1.5], &[2], &levels);
        b.add(&Cf::record(&[2.5], &[2], &levels));
        let d = m.distance(&a, &b);
        let mut ab = a.clone();
        ab.add(&b);
        assert!((d - (m.xi(&a) + m.xi(&b) - m.xi(&ab))).abs() < 1e-12);
        assert_eq!(d, m.distance(&b, &a));
        // a record and its copy are at distance 0
        assert_eq!(m.distance(&a, &a.clone()), 0.0);
    }

    #[test]
    fn the_tree_depends_only_on_the_order_and_respects_its_limits() {
        let m = model(vec![1.0, 1.0]);
        let records: Vec<Cf> = (0..40)
            .map(|i| {
                let x = f64::from(i);
                Cf::record(&[(x * 0.37).sin() * 3.0, (x * 1.3).cos() * 2.0], &[], &[])
            })
            .collect();
        let settings = Settings {
            threshold: 0.0,
            branch: 3,
            levels: 2,
            noise: 0.0,
        };
        let first = precluster(&m, records.clone(), &settings);
        let again = precluster(&m, records.clone(), &settings);
        assert_eq!(first.subclusters, again.subclusters);
        assert!(first.filled);
        assert!(first.subclusters.len() <= 27);
        let n: f64 = first.subclusters.iter().map(|c| c.n).sum();
        assert_eq!(n, 40.0);
    }
}
