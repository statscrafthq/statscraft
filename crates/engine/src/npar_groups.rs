//! NPAR TESTS of independent samples: Mann-Whitney, two-sample Kolmogorov-Smirnov,
//! Wald-Wolfowitz, Moses, Kruskal-Wallis, the median test and Jonckheere-Terpstra.
//!
//! Rules from the Command Syntax Reference (NPAR TESTS) and the Algorithms manual (NPAR TESTS
//! Algorithms; Jonckheere-Terpstra from the Nonparametric Tests chapter):
//! - M-W, K-S, W-W and MOSES compare the cases with the first value of the grouping variable
//!   with those with the second; K-W and J-T take every value from the first to the second as a
//!   group (J-T in that order); MEDIAN does so when the first is below the second, and compares
//!   the two values otherwise. Other cases are left out.
//! - Mann-Whitney: U = n1 n2 + n1 (n1 + 1) / 2 - S1, replaced by n1 n2 - U above n1 n2 / 2;
//!   W = S1 above, else S2. Z with the tie correction. Exact significance, twice the lower tail
//!   of U without ties, when n1 n2 <= 400 and n1 n2 / 2 + min(n1, n2) <= 220.
//! - Two-sample K-S: D = F1 - F2 with F1 the cdf of the larger group (the first when the sizes
//!   are equal), Z = max |D| sqrt(n1 n2 / (n1 + n2)), Smirnov's significance.
//! - Wald-Wolfowitz: runs of group membership in the sorted pooled values; with ties across the
//!   groups the fewest and the most runs the ties allow. One-tailed: exact up to 30 cases, else
//!   the normal approximation of the runs test.
//! - Moses: span of the control group in the joint average ranks, rounded, and again without the
//!   h lowest and h highest control ranks (h the integer part of 0.05 nc, at least 1, unless
//!   given); exact one-tailed probabilities.
//! - Kruskal-Wallis H corrected for ties, df = nonempty groups - 1.
//! - Median test: the median of all the test's cases unless given. Two-sample: Fisher's exact
//!   test up to 30 cases, else chi-square with the continuity correction; k-sample: chi-square
//!   over the nonempty groups.

use crate::crosstabs::fisher;
use crate::descriptives::number;
use crate::dist::{chi_square_p, normal_cdf, normal_p};
use crate::npar_tests::{
    Run, exact_tails, ln_choose, middle, rank_sum_distribution, runs_exact, runs_z, smirnov_p,
    statistics_table, table, tally, text, tied_ranks, used,
};
use crate::{Cell, Error, Table};
use statscraft_data::Variable;
use statscraft_syntax::GroupTest;

/// The cases of one tested variable split into groups, in group order.
pub(crate) struct Grouped {
    /// The value of the grouping variable of each group.
    pub keys: Vec<f64>,
    /// (value, weight) of the cases of each group.
    pub groups: Vec<Vec<(f64, f64)>>,
}

impl Grouped {
    pub fn sizes(&self) -> Vec<f64> {
        self.groups
            .iter()
            .map(|g| g.iter().map(|p| p.1).sum())
            .collect()
    }

    /// Every case with its group index.
    pub fn pooled(&self) -> Vec<(f64, f64, usize)> {
        self.groups
            .iter()
            .enumerate()
            .flat_map(|(i, g)| g.iter().map(move |&(x, w)| (x, w, i)))
            .collect()
    }

    /// Joint average ranks of every case of the pooled list, and the sum of t^3 - t.
    pub fn ranked(&self) -> (Vec<(f64, f64, usize, f64)>, f64) {
        let pooled = self.pooled();
        let pairs: Vec<(f64, f64)> = pooled.iter().map(|p| (p.0, p.1)).collect();
        let (ranks, ties) = tied_ranks(&pairs);
        let ranked = pooled
            .iter()
            .zip(ranks)
            .map(|(&(x, w, g), r)| (x, w, g, r))
            .collect();
        (ranked, ties)
    }

    /// Two-sample Kolmogorov-Smirnov: the most extreme differences (absolute, positive,
    /// negative) of the larger group's cdf (the first on equal sizes) less the other's, Z and
    /// Smirnov's significance.
    pub fn smirnov(&self) -> [f64; 5] {
        let n = self.sizes();
        let larger = usize::from(n[1] > n[0]);
        let pooled = tally(&self.pooled().iter().map(|p| (p.0, 1.0)).collect::<Vec<_>>());
        let (mut positive, mut negative) = (0.0f64, 0.0f64);
        for &(x, _) in &pooled {
            let cdf = |g: usize| {
                self.groups[g]
                    .iter()
                    .filter(|p| p.0 <= x)
                    .map(|p| p.1)
                    .sum::<f64>()
                    / n[g]
            };
            let diff = cdf(larger) - cdf(1 - larger);
            positive = positive.max(diff);
            negative = negative.min(diff);
        }
        let absolute = positive.max(-negative);
        let z = absolute * (n[0] * n[1] / (n[0] + n[1])).sqrt();
        [absolute, positive, negative, z, smirnov_p(z)]
    }

    /// The sorted distinct values of two groups as blocks of (first group's, second group's)
    /// weight, for counting runs.
    pub fn blocks(&self) -> Vec<(f64, f64)> {
        let mut pooled = self.pooled();
        pooled.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut blocks: Vec<(f64, f64, f64)> = Vec::new();
        for (x, w, g) in pooled {
            match blocks.last_mut() {
                Some(last) if last.0 == x => {
                    if g == 0 {
                        last.1 += w
                    } else {
                        last.2 += w
                    }
                }
                _ => blocks.push(if g == 0 { (x, w, 0.0) } else { (x, 0.0, w) }),
            }
        }
        blocks.iter().map(|b| (b.1, b.2)).collect()
    }

    /// The joint average ranks of the first (control) group's cases, one per unit of weight,
    /// sorted.
    pub fn control_ranks(&self) -> Vec<f64> {
        let (ranked, _) = self.ranked();
        let mut control: Vec<f64> = ranked
            .iter()
            .filter(|r| r.2 == 0)
            .flat_map(|r| std::iter::repeat_n(r.3, r.1 as usize))
            .collect();
        control.sort_by(f64::total_cmp);
        control
    }

    /// The exact Kruskal-Wallis tail (significance, point probability) of the groups that have
    /// cases: the pooled tie blocks (cases, joint rank times two), the group sizes and the
    /// observed rank sums times two go to the walk.
    pub fn kruskal_exact(&self, budget: f64) -> Option<(f64, f64)> {
        let (ranked, _) = self.ranked();
        let n = self.sizes();
        let filled: Vec<usize> = (0..n.len()).filter(|&g| n[g] > 0.0).collect();
        let at = |g: usize| filled.iter().position(|&f| f == g);
        let mut observed = vec![0_i64; filled.len()];
        let mut by_value: Vec<(f64, u64, i64)> = Vec::new();
        for &(x, w, g, rank) in &ranked {
            let doubled = (2.0 * rank).round() as i64;
            if let Some(i) = at(g) {
                observed[i] += w as i64 * doubled;
            }
            match by_value.iter_mut().find(|b| b.0 == x) {
                Some(b) => b.1 += w as u64,
                None => by_value.push((x, w as u64, doubled)),
            }
        }
        let blocks: Vec<(u64, i64)> = by_value.iter().map(|b| (b.1, b.2)).collect();
        let sizes: Vec<u64> = filled.iter().map(|&g| n[g] as u64).collect();
        crate::npar_exact::kruskal_tail(&blocks, &sizes, &observed, budget)
    }

    /// The exact Jonckheere-Terpstra tails (two-sided, one-sided, point) of the groups that
    /// have cases, in the order of the keys.
    pub fn jonckheere_exact(&self, observed: f64, budget: f64) -> Option<(f64, f64, f64)> {
        let n = self.sizes();
        let filled: Vec<usize> = (0..n.len()).filter(|&g| n[g] > 0.0).collect();
        let sizes: Vec<u64> = filled.iter().map(|&g| n[g] as u64).collect();
        let counts = tally(&self.pooled().iter().map(|p| (p.0, p.1)).collect::<Vec<_>>());
        let blocks: Vec<u64> = counts.iter().map(|c| c.1 as u64).collect();
        crate::npar_exact::jonckheere_tail(&blocks, &sizes, (2.0 * observed).round() as i64, budget)
    }

    /// Kruskal-Wallis: the weighted rank sum of every group and H corrected for ties, None
    /// with fewer than two groups or no spread.
    pub fn kruskal_wallis(&self) -> (Vec<f64>, Option<f64>) {
        let (ranked, ties) = self.ranked();
        let n = self.sizes();
        let total: f64 = n.iter().sum();
        let mut sum = vec![0.0; self.keys.len()];
        for r in &ranked {
            sum[r.2] += r.1 * r.3;
        }
        let h: f64 = (0..sum.len()).map(|i| sum[i] * sum[i] / n[i]).sum();
        let h = 12.0 / (total * (total + 1.0)) * h - 3.0 * (total + 1.0);
        let correction = 1.0 - ties / (total.powi(3) - total);
        let usable = self.keys.len() >= 2 && correction > 1e-12;
        (sum, usable.then(|| h / correction))
    }

    /// Jonckheere-Terpstra: the statistic (the Mann-Whitney counts of every earlier group
    /// below every later one), its mean and its standard deviation corrected for ties.
    pub fn jonckheere(&self) -> [f64; 3] {
        let n = self.sizes();
        let total: f64 = n.iter().sum();
        let mut t = 0.0;
        for (a, first) in self.groups.iter().enumerate() {
            for (later, size) in self.groups[a + 1..].iter().zip(&n[a + 1..]) {
                // U of the pair: the rank sum of the later group in the joint ranking, less
                // its smallest possible value
                let pair = Grouped {
                    keys: vec![0.0, 1.0],
                    groups: vec![first.clone(), later.clone()],
                };
                let (ranked, _) = pair.ranked();
                let sum: f64 = ranked.iter().filter(|r| r.2 == 1).map(|r| r.1 * r.3).sum();
                t += sum - size * (size + 1.0) / 2.0;
            }
        }
        let ties = tally(&self.pooled().iter().map(|p| (p.0, p.1)).collect::<Vec<_>>());
        let a1: f64 = ties.iter().map(|(_, t)| t * (t - 1.0)).sum();
        let a2: f64 = ties.iter().map(|(_, t)| t * t * (t - 1.0)).sum();
        let s1: f64 = n.iter().map(|k| k * (k - 1.0)).sum();
        let s2: f64 = n.iter().map(|k| k * (k - 1.0) * (k - 2.0)).sum();
        let s3: f64 = n.iter().map(|k| k * (k - 1.0) * (2.0 * k + 5.0)).sum();
        let mean = (total * total - n.iter().map(|k| k * k).sum::<f64>()) / 4.0;
        let variance = (total * (total - 1.0) * (2.0 * total + 5.0) - s3 - 2.0 * a2 - 5.0 * a1)
            / 72.0
            + s2 * (a2 - 2.0 * a1) / (36.0 * total * (total - 1.0) * (total - 2.0))
            + s1 * a1 / (8.0 * total * (total - 1.0));
        [t, mean, variance.sqrt()]
    }

    /// The median test with `md` as the median; `two` treats the two groups as a fourfold
    /// table (Fisher up to 30 cases, else the corrected chi-square).
    pub fn median_test(&self, md: Option<f64>, two: bool) -> MedianTest {
        let n = self.sizes();
        let total: f64 = n.iter().sum();
        let above: Vec<f64> = self
            .groups
            .iter()
            .map(|g| {
                g.iter()
                    .filter(|p| md.is_some_and(|m| p.0 > m))
                    .map(|p| p.1)
                    .sum()
            })
            .collect();
        // nonempty groups only
        let filled: Vec<usize> = (0..n.len()).filter(|&g| n[g] > 0.0).collect();
        let k = filled.len() as f64;
        let r1: f64 = filled.iter().map(|&g| above[g]).sum();
        let r2 = total - r1;
        let (mut chi, mut exact, mut sparse) = (None, None, None);
        if k >= 2.0 && r1 > 0.0 && r2 > 0.0 {
            if two {
                let (g1, g2, n1, n2) = (above[0], above[1], n[0], n[1]);
                if total <= 30.0 {
                    let (p, _) = fisher(g1 as u64, (n1 - g1) as u64, g2 as u64, (n2 - g2) as u64);
                    exact = Some(p);
                } else {
                    chi = Some(
                        ((g1 * (n2 - g2) - g2 * (n1 - g1)).abs() - total / 2.0).powi(2) * total
                            / ((g1 + g2) * (n1 + n2 - g1 - g2) * n1 * n2),
                    );
                }
            } else {
                let mut sum = 0.0;
                let (mut low, mut below_one) = (0, false);
                for &g in &filled {
                    for (observed, row) in [(above[g], r1), (n[g] - above[g], r2)] {
                        let e = row * n[g] / total;
                        sum += (observed - e).powi(2) / e;
                        low += usize::from(e < 5.0);
                        below_one |= e < 1.0;
                    }
                }
                chi = Some(sum);
                if below_one || low as f64 > 0.2 * 2.0 * k {
                    sparse = Some((low, 2 * filled.len()));
                }
            }
        }
        MedianTest {
            above,
            groups: k,
            chi,
            exact,
            sparse,
        }
    }

    /// Mann-Whitney of two groups: the rank sums, U of the first group (pairs with the first
    /// group's case below, ties half), the variance of U corrected for ties, and the exact
    /// two-sided significance without ties when the groups are small enough.
    pub fn mann_whitney(&self) -> MannWhitney {
        let n = self.sizes();
        let (ranked, ties) = self.ranked();
        let mut s = [0.0; 2];
        for r in &ranked {
            s[r.2] += r.1 * r.3;
        }
        let total = n[0] + n[1];
        let product = n[0] * n[1];
        let u1 = product + n[0] * (n[0] + 1.0) / 2.0 - s[0];
        let variance =
            product / (total * (total - 1.0)) * ((total.powi(3) - total) / 12.0 - ties / 12.0);
        let usable = n[0] > 0.0 && n[1] > 0.0;
        let small = usable && product <= 400.0 && product / 2.0 + n[0].min(n[1]) <= 220.0;
        let u = u1.min(product - u1);
        let exact = small
            .then(|| {
                let ranks: Vec<u64> = (1..=total as u64).collect();
                rank_sum_distribution(&ranks, Some(n[0] as usize)).map(|p| {
                    // U of n1 drawn ranks is their sum less n1 (n1 + 1) / 2
                    let offset = (n[0] * (n[0] + 1.0) / 2.0) as usize;
                    let lower: f64 = p.iter().skip(offset).take(u.floor() as usize + 1).sum();
                    (2.0 * lower).min(1.0)
                })
            })
            .flatten();
        MannWhitney {
            n: [n[0], n[1]],
            s,
            u1,
            variance,
            exact,
        }
    }
}

/// The median test of some groups.
pub(crate) struct MedianTest {
    /// Weight above the median in each group.
    pub above: Vec<f64>,
    /// The number of nonempty groups.
    pub groups: f64,
    /// Chi-square over the nonempty groups (with the continuity correction for two groups
    /// above 30 cases).
    pub chi: Option<f64>,
    /// Fisher's exact two-sided significance (two groups up to 30 cases).
    pub exact: Option<f64>,
    /// (cells below an expected 5, cells) when that makes the chi-square questionable.
    pub sparse: Option<(usize, usize)>,
}

pub(crate) struct MannWhitney {
    pub n: [f64; 2],
    pub s: [f64; 2],
    pub u1: f64,
    pub variance: f64,
    pub exact: Option<f64>,
}

/// n choose k, 0 outside 0 <= k <= n.
fn choose(n: f64, k: f64) -> f64 {
    if k < 0.0 || n < 0.0 || k > n {
        0.0
    } else {
        ln_choose(n, k).exp()
    }
}

/// The exact one-tailed probability of a Moses span at most `span` with `h` trimmed from each
/// end of `nc` control and `ne` experimental cases.
pub(crate) fn moses_p(span: f64, nc: f64, ne: f64, h: f64) -> f64 {
    let g = span - (nc - 2.0 * h);
    if g < 0.0 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..=(g as i64) {
        let i = i as f64;
        sum += choose(i + nc - 2.0 * h - 2.0, i) * choose(ne + 2.0 * h + 1.0 - i, ne - i);
    }
    (sum / choose(nc + ne, nc)).min(1.0)
}

/// The fewest and the most runs of two groups over sorted blocks of tied values, each block
/// with the weight of the first and of the second group.
pub(crate) fn run_bounds(blocks: &[(f64, f64)]) -> (f64, f64) {
    // for a block: the possible (first label, last label, runs) arrangements
    let options = |a: f64, b: f64, most: bool| -> Vec<(usize, usize, f64)> {
        if b == 0.0 {
            return vec![(0, 0, 1.0)];
        }
        if a == 0.0 {
            return vec![(1, 1, 1.0)];
        }
        let count = [a, b];
        let mut out = Vec::new();
        for s in 0..2 {
            for e in 0..2 {
                let runs = if s == e {
                    if count[s] < 2.0 {
                        continue;
                    }
                    if most {
                        2.0 * (count[s] - 1.0).min(count[1 - s]) + 1.0
                    } else {
                        3.0
                    }
                } else if most {
                    2.0 * count[s].min(count[e])
                } else {
                    2.0
                };
                out.push((s, e, runs));
            }
        }
        out
    };
    let bound = |most: bool| {
        // best total runs so far ending with each label
        let mut best: [Option<f64>; 2] = [None, None];
        let mut first = true;
        for &(a, b) in blocks {
            let mut next: [Option<f64>; 2] = [None, None];
            for (s, e, runs) in options(a, b, most) {
                let candidates: Vec<f64> = if first {
                    vec![runs]
                } else {
                    (0..2)
                        .filter_map(|last| best[last].map(|t| t + runs - f64::from(last == s)))
                        .collect()
                };
                for total in candidates {
                    let slot = &mut next[e];
                    *slot = Some(match *slot {
                        None => total,
                        Some(old) if most => old.max(total),
                        Some(old) => old.min(total),
                    });
                }
            }
            best = next;
            first = false;
        }
        let both = best.iter().flatten().copied();
        if most {
            both.fold(f64::NEG_INFINITY, f64::max)
        } else {
            both.fold(f64::INFINITY, f64::min)
        }
    };
    (bound(false), bound(true))
}

/// The table of group sizes shared by K-S, W-W and MOSES.
fn frequencies(names: &[String], labels: &[[String; 2]], sizes: &[[f64; 2]]) -> Table {
    let mut rows = Vec::new();
    for ((name, label), size) in names.iter().zip(labels).zip(sizes) {
        rows.push(vec![
            text(name.clone()),
            text(label[0].clone()),
            Cell::Number(size[0]),
        ]);
        rows.push(vec![
            Cell::Empty,
            text(label[1].clone()),
            Cell::Number(size[1]),
        ]);
        rows.push(vec![
            Cell::Empty,
            text("Total"),
            Cell::Number(size[0] + size[1]),
        ]);
    }
    table(
        "Frequencies",
        2,
        vec![String::new(), String::new(), "N".into()],
        rows,
    )
}

pub(crate) fn groups(
    run: &mut Run,
    variables: &[Variable],
    group: &Variable,
    test: GroupTest,
    (v1, v2): (f64, f64),
) -> Result<Vec<Table>, Error> {
    let ranged = match test {
        GroupTest::KruskalWallis | GroupTest::JonckheereTerpstra => true,
        GroupTest::Median(_) => v1 < v2,
        _ => false,
    };
    if v1 == v2
        && !matches!(
            test,
            GroupTest::KruskalWallis | GroupTest::JonckheereTerpstra
        )
    {
        return Err(Error(format!(
            "NPAR TESTS: the two values of {} must differ",
            group.name()
        )));
    }
    let (low, high) = (v1.min(v2), v1.max(v2));
    let mut data = Vec::new();
    for v in variables {
        let cases = run.cases(&[v, group]);
        let mut keys: Vec<f64> = Vec::new();
        let member: Vec<(f64, f64, f64)> = cases
            .iter()
            .map(|&c| (Run::value(group, c), Run::value(v, c), run.weights[c]))
            .filter(|(g, _, _)| {
                if ranged {
                    *g >= low && *g <= high
                } else {
                    *g == v1 || *g == v2
                }
            })
            .collect();
        if ranged {
            keys = tally(&member.iter().map(|m| (m.0, 1.0)).collect::<Vec<_>>())
                .into_iter()
                .map(|t| t.0)
                .collect();
            if v1 > v2 {
                keys.reverse();
            }
        } else {
            keys.extend([v1, v2]);
        }
        let groups = keys
            .iter()
            .map(|k| {
                member
                    .iter()
                    .filter(|m| m.0 == *k)
                    .map(|m| (m.1, m.2))
                    .collect()
            })
            .collect();
        data.push(Grouped { keys, groups });
    }
    let names: Vec<String> = variables.iter().map(|v| run.shown.variable(v)).collect();
    let label = |run: &Run, key: f64| run.label(group, key);
    Ok(match test {
        GroupTest::MannWhitney => mann_whitney(run, &names, &data, group),
        GroupTest::KolmogorovSmirnov => {
            let mut stats: Vec<Vec<Option<f64>>> = vec![Vec::new(); 5];
            let mut exact_stats: Vec<Vec<Option<f64>>> = vec![Vec::new(); 2];
            let mut labels = Vec::new();
            let mut sizes = Vec::new();
            for d in &data {
                let n = d.sizes();
                labels.push([label(run, d.keys[0]), label(run, d.keys[1])]);
                sizes.push([n[0], n[1]]);
                let usable = n[0] > 0.0 && n[1] > 0.0;
                let values = d.smirnov();
                for (s, value) in stats.iter_mut().zip(values) {
                    s.push(usable.then_some(value));
                }
                // the exact tail of the absolute gap, from the counts of each distinct value
                let tail = (run.exact && usable).then(|| {
                    let blocks: Vec<(u64, u64)> = d
                        .blocks()
                        .iter()
                        .map(|b| (b.0.round() as u64, b.1.round() as u64))
                        .collect();
                    let gap = (values[0] * n[0] * n[1]).round() as i64;
                    crate::npar_exact::smirnov_exact(&blocks, gap)
                });
                exact_stats[0].push(tail.map(|t| t.0));
                exact_stats[1].push(tail.map(|t| t.1));
            }
            let mut labels_rows = vec![
                ("Most Extreme Differences", "Absolute"),
                ("", "Positive"),
                ("", "Negative"),
                ("Kolmogorov-Smirnov Z", ""),
                ("Asymp. Sig. (2-tailed)", ""),
            ];
            if run.exact {
                labels_rows.extend([("Exact Sig. (2-tailed)", ""), ("Point Probability", "")]);
                stats.extend(exact_stats);
            }
            let mut columns = vec![String::new(), String::new()];
            columns.extend(names.iter().cloned());
            let rows = labels_rows
                .iter()
                .zip(stats)
                .map(|((a, b), values)| {
                    let mut row = vec![text(*a), text(*b)];
                    row.extend(values.into_iter().map(number));
                    row
                })
                .collect();
            vec![
                frequencies(&names, &labels, &sizes),
                table("Two-Sample Kolmogorov-Smirnov Test", 2, columns, rows),
            ]
        }
        GroupTest::WaldWolfowitz => {
            let mut labels = Vec::new();
            let mut sizes = Vec::new();
            let mut rows = Vec::new();
            for (d, name) in data.iter().zip(&names) {
                let n = d.sizes();
                labels.push([label(run, d.keys[0]), label(run, d.keys[1])]);
                sizes.push([n[0], n[1]]);
                let blocks = d.blocks();
                let total = n[0] + n[1];
                let usable = n[0] > 0.0 && n[1] > 0.0;
                let (fewest, most) = if usable {
                    run_bounds(&blocks)
                } else {
                    (f64::NAN, f64::NAN)
                };
                let kinds: Vec<(&str, f64)> = if fewest == most || !usable {
                    vec![("Exact Number of Runs", fewest)]
                } else {
                    vec![("Minimum Possible", fewest), ("Maximum Possible", most)]
                };
                for (i, (kind, runs)) in kinds.into_iter().enumerate() {
                    let z = usable.then(|| runs_z(runs, n[0], n[1])).flatten();
                    let exact = (usable && (total <= 30.0 || run.exact))
                        .then(|| runs_exact(runs, n[0], n[1])[0]);
                    let asymp = (total > 30.0).then(|| z.map(normal_cdf)).flatten();
                    rows.push(vec![
                        if i == 0 {
                            text(name.clone())
                        } else {
                            Cell::Empty
                        },
                        text(kind),
                        number(usable.then_some(runs)),
                        number(z),
                        number(exact),
                        number(asymp),
                    ]);
                }
            }
            let columns = [
                "",
                "",
                "Number of Runs",
                "Z",
                "Exact Sig. (1-tailed)",
                "Asymp. Sig. (1-tailed)",
            ]
            .map(String::from)
            .to_vec();
            vec![
                frequencies(&names, &labels, &sizes),
                table("Wald-Wolfowitz Test", 2, columns, rows),
            ]
        }
        GroupTest::Moses(trim) => {
            let mut labels = Vec::new();
            let mut sizes = Vec::new();
            let mut stats: Vec<Vec<Option<f64>>> = vec![Vec::new(); 5];
            for d in &data {
                let n = d.sizes();
                labels.push([
                    format!("{} (Control)", label(run, d.keys[0])),
                    format!("{} (Experimental)", label(run, d.keys[1])),
                ]);
                sizes.push([n[0], n[1]]);
                let control = d.control_ranks();
                let (nc, ne) = (n[0], n[1]);
                let h = match trim {
                    Some(h) if h.trunc() >= 1.0 => h.trunc(),
                    _ => (0.05 * nc).floor().max(1.0),
                };
                let span_of = |ranks: &[f64]| -> Option<f64> {
                    Some((ranks.last()? - ranks.first()? + 1.0).round())
                };
                let span = span_of(&control);
                let kept = h as usize;
                let trimmed = if control.len() > 2 * kept {
                    span_of(&control[kept..control.len() - kept])
                } else {
                    None
                };
                let values = [
                    span,
                    span.map(|s| moses_p(s, nc, ne, 0.0)),
                    trimmed,
                    trimmed.map(|s| moses_p(s, nc, ne, h)),
                    Some(h),
                ];
                for (s, value) in stats.iter_mut().zip(values) {
                    s.push(value);
                }
            }
            let labels_rows = [
                ("Observed Control Group Span", "Span"),
                ("", "Sig. (1-tailed)"),
                ("Trimmed Control Group Span", "Span"),
                ("", "Sig. (1-tailed)"),
                ("Outliers Trimmed from each End", ""),
            ];
            let mut columns = vec![String::new(), String::new()];
            columns.extend(names.iter().cloned());
            let rows = labels_rows
                .iter()
                .zip(stats)
                .map(|((a, b), values)| {
                    let mut row = vec![text(*a), text(*b)];
                    row.extend(values.into_iter().map(number));
                    row
                })
                .collect();
            vec![
                frequencies(&names, &labels, &sizes),
                table("Moses Test", 2, columns, rows),
            ]
        }
        GroupTest::KruskalWallis => {
            let mut rank_rows = Vec::new();
            let mut stats: Vec<Vec<Option<f64>>> = vec![Vec::new(); 3];
            let mut exact_stats: Vec<Vec<Option<f64>>> = vec![Vec::new(); 2];
            for (d, name) in data.iter().zip(&names) {
                let n = d.sizes();
                let total: f64 = n.iter().sum();
                let (sum, h) = d.kruskal_wallis();
                for (i, key) in d.keys.iter().enumerate() {
                    rank_rows.push(vec![
                        if i == 0 {
                            text(name.clone())
                        } else {
                            Cell::Empty
                        },
                        text(label(run, *key)),
                        Cell::Number(n[i]),
                        Cell::Number(sum[i] / n[i]),
                    ]);
                }
                rank_rows.push(vec![
                    if d.keys.is_empty() {
                        text(name.clone())
                    } else {
                        Cell::Empty
                    },
                    text("Total"),
                    Cell::Number(total),
                    Cell::Empty,
                ]);
                let k = d.keys.len() as f64;
                if h.is_none() {
                    run.warnings.push(format!(
                        "K-W: {name} has fewer than two groups or no spread; no test"
                    ));
                }
                stats[0].push(h);
                stats[1].push(h.map(|_| k - 1.0));
                stats[2].push(h.map(|h| chi_square_p(h, k - 1.0)));
                let tail = if run.exact && h.is_some() {
                    let tail = d.kruskal_exact(3e7);
                    if tail.is_none() {
                        run.warnings.push(format!(
                            "/METHOD: the exact significance of the Kruskal-Wallis test of {name} needs too many steps; the asymptotic one is shown"
                        ));
                    }
                    tail
                } else {
                    None
                };
                exact_stats[0].push(tail.map(|t| t.0));
                exact_stats[1].push(tail.map(|t| t.1));
            }
            let mut names_of_rows = vec!["Chi-Square", "df", "Asymp. Sig."];
            if run.exact {
                names_of_rows.extend(["Exact Sig.", "Point Probability"]);
                stats.extend(exact_stats);
            }
            let rows = names_of_rows
                .iter()
                .map(|s| s.to_string())
                .zip(stats)
                .collect();
            vec![
                table(
                    "Ranks",
                    2,
                    ["", "", "N", "Mean Rank"].map(String::from).to_vec(),
                    rank_rows,
                ),
                statistics_table("Kruskal-Wallis Test", &names, rows),
            ]
        }
        GroupTest::Median(given) => median(run, &names, &data, group, given, ranged),
        GroupTest::JonckheereTerpstra => {
            let mut rows = Vec::new();
            for (d, name) in data.iter().zip(&names) {
                let n = d.sizes();
                let total: f64 = n.iter().sum();
                let [t, mean, sd] = d.jonckheere();
                let usable = d.keys.len() >= 2 && sd > 0.0;
                let z = usable.then(|| (t - mean) / sd);
                let mut row = vec![text(name.clone())];
                row.extend(
                    [
                        Some(d.keys.len() as f64),
                        Some(total),
                        usable.then_some(t),
                        usable.then_some(mean),
                        usable.then_some(sd),
                        z,
                        z.map(normal_p),
                    ]
                    .map(number),
                );
                if run.exact {
                    let tail = if usable {
                        let tail = d.jonckheere_exact(t, 3e7);
                        if tail.is_none() {
                            run.warnings.push(format!(
                                "/METHOD: the exact significance of the Jonckheere-Terpstra test of {name} needs too many steps; the asymptotic one is shown"
                            ));
                        }
                        tail
                    } else {
                        None
                    };
                    row.extend([
                        number(tail.map(|t| t.0)),
                        number(tail.map(|t| t.1)),
                        number(tail.map(|t| t.2)),
                    ]);
                }
                rows.push(row);
            }
            let columns = vec![
                String::new(),
                format!("Number of Levels in {}", run.shown.variable(group)),
                "N".into(),
                "Observed J-T Statistic".into(),
                "Mean J-T Statistic".into(),
                "Std. Deviation of J-T Statistic".into(),
                "Std. J-T Statistic".into(),
                "Asymp. Sig. (2-tailed)".into(),
            ];
            let mut columns = columns;
            if run.exact {
                columns.extend(
                    [
                        "Exact Sig. (2-tailed)",
                        "Exact Sig. (1-tailed)",
                        "Point Probability",
                    ]
                    .map(String::from),
                );
            }
            vec![table("Jonckheere-Terpstra Test", 1, columns, rows)]
        }
    })
}

fn mann_whitney(run: &mut Run, names: &[String], data: &[Grouped], group: &Variable) -> Vec<Table> {
    let mut rank_rows = Vec::new();
    let mut stats: Vec<(String, Vec<Option<f64>>)> = [
        "Mann-Whitney U",
        "Wilcoxon W",
        "Z",
        "Asymp. Sig. (2-tailed)",
        "Exact Sig. [2*(1-tailed Sig.)]",
        "Exact Sig. (2-tailed)",
        "Exact Sig. (1-tailed)",
        "Point Probability",
    ]
    .iter()
    .map(|r| (r.to_string(), Vec::new()))
    .collect();
    for (d, name) in data.iter().zip(names) {
        let mw = d.mann_whitney();
        let (n, s) = (mw.n, mw.s);
        for i in 0..2 {
            rank_rows.push(vec![
                if i == 0 {
                    text(name.clone())
                } else {
                    Cell::Empty
                },
                text(run.label(group, d.keys[i])),
                Cell::Number(n[i]),
                number(Some(s[i] / n[i])),
                Cell::Number(s[i]),
            ]);
        }
        let total = n[0] + n[1];
        rank_rows.push(vec![
            Cell::Empty,
            text("Total"),
            Cell::Number(total),
            Cell::Empty,
            Cell::Empty,
        ]);
        let usable = n[0] > 0.0 && n[1] > 0.0;
        let product = n[0] * n[1];
        let u1 = mw.u1;
        let (u, w) = if u1 > product / 2.0 {
            (product - u1, s[0])
        } else {
            (u1, s[1])
        };
        let variance = mw.variance;
        let z = (usable && variance > 0.0).then(|| (u - product / 2.0) / variance.sqrt());
        let exact_default = mw.exact;
        let exact = if run.exact && usable {
            let (ranked, _) = d.ranked();
            let doubled: Vec<(u64, usize)> = ranked
                .iter()
                .flat_map(|r| std::iter::repeat_n(((2.0 * r.3).round() as u64, r.2), r.1 as usize))
                .collect();
            let ranks: Vec<u64> = doubled.iter().map(|d| d.0).collect();
            let found = rank_sum_distribution(&ranks, Some(n[0] as usize));
            if found.is_none() {
                run.warnings
                    .push(format!("M-W: {name} has too many cases for the exact test"));
            }
            found.map(|p| exact_tails(&p, (2.0 * s[0]).round() as usize, n[0] * (total + 1.0)))
        } else {
            None
        };
        let values = [
            usable.then_some(u),
            usable.then_some(w),
            z,
            z.map(normal_p),
            exact_default,
            exact.map(|e| e[0]),
            exact.map(|e| e[1]),
            exact.map(|e| e[2]),
        ];
        for (row, value) in stats.iter_mut().zip(values) {
            row.1.push(value);
        }
    }
    let mut test = statistics_table("Mann-Whitney Test", names, used(stats));
    test.caption = Some(format!(
        "Grouping variable: {}. The exact significance [2*(1-tailed Sig.)] is not corrected for ties.",
        run.shown.variable(group)
    ));
    vec![
        table(
            "Ranks",
            2,
            ["", "", "N", "Mean Rank", "Sum of Ranks"]
                .map(String::from)
                .to_vec(),
            rank_rows,
        ),
        test,
    ]
}

fn median(
    run: &mut Run,
    names: &[String],
    data: &[Grouped],
    group: &Variable,
    given: Option<f64>,
    ranged: bool,
) -> Vec<Table> {
    // the groups of every variable, for the columns of the frequency table
    let mut keys: Vec<f64> = Vec::new();
    for d in data {
        for k in &d.keys {
            if !keys.contains(k) {
                keys.push(*k);
            }
        }
    }
    let mut frequency_rows = Vec::new();
    let mut stats: Vec<(String, Vec<Option<f64>>)> = [
        "N",
        "Median",
        "Chi-Square",
        "df",
        "Asymp. Sig.",
        "Exact Sig.",
    ]
    .iter()
    .map(|r| (r.to_string(), Vec::new()))
    .collect();
    let mut notes = Vec::new();
    for (d, name) in data.iter().zip(names) {
        let pooled: Vec<(f64, f64)> = d.pooled().iter().map(|p| (p.0, p.1)).collect();
        let total: f64 = pooled.iter().map(|p| p.1).sum();
        let md = given.or_else(|| middle(&pooled));
        let n = d.sizes();
        let test = d.median_test(md, !ranged);
        let above = &test.above;
        for (i, label) in ["> Median", "<= Median"].iter().enumerate() {
            let mut row = vec![
                if i == 0 {
                    text(name.clone())
                } else {
                    Cell::Empty
                },
                text(*label),
            ];
            for k in &keys {
                let at = d.keys.iter().position(|x| x == k);
                row.push(Cell::Number(at.map_or(0.0, |g| {
                    if i == 0 { above[g] } else { n[g] - above[g] }
                })));
            }
            frequency_rows.push(row);
        }
        let k = test.groups;
        let chi = test.chi;
        let mut exact = test.exact;
        // k groups, METHOD=EXACT: every split of the cases above the median over the groups
        if run.exact && ranged && chi.is_some() {
            let filled: Vec<usize> = (0..n.len()).filter(|&g| n[g] > 0.0).collect();
            let sizes: Vec<u64> = filled.iter().map(|&g| n[g] as u64).collect();
            let counts: Vec<u64> = filled.iter().map(|&g| above[g] as u64).collect();
            match crate::npar_exact::median_tail(&sizes, &counts, 5e7) {
                Some((p, _)) => exact = Some(p),
                None => run.warnings.push(format!(
                    "/METHOD: the exact significance of the median test of {name} needs too many steps; the asymptotic one is shown"
                )),
            }
        }
        if let Some((low, cells)) = test.sparse {
            notes.push(format!(
                "{name}: {low} of {cells} cells have expected frequencies less than 5."
            ));
        }
        if chi.is_none() && exact.is_none() {
            run.warnings.push(format!(
                "MEDIAN: {name} needs two groups with cases on both sides of the median; no test"
            ));
        }
        let df = (chi.is_some()).then_some(k - 1.0);
        let values = [
            Some(total),
            md,
            chi,
            df,
            chi.map(|c| chi_square_p(c, k - 1.0)),
            exact,
        ];
        for (row, value) in stats.iter_mut().zip(values) {
            row.1.push(value);
        }
    }
    let mut columns = vec![String::new(), String::new()];
    columns.extend(keys.iter().map(|k| run.label(group, *k)));
    let frequencies = Table {
        title: "Median Test Frequencies".into(),
        row_headers: 2,
        column_groups: (0..columns.len())
            .map(|i| {
                if i < 2 {
                    Vec::new()
                } else {
                    vec![run.shown.variable(group)]
                }
            })
            .collect(),
        columns,
        rows: frequency_rows,
        ..Table::default()
    };
    let mut test = statistics_table("Median Test", names, used(stats));
    if !notes.is_empty() {
        test.caption = Some(notes.join(" "));
    }
    vec![frequencies, test]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_bounds_with_mixed_ties() {
        // A A | (A B) | B: fewest 2 (A A A B B), most 4 (A A B A B)
        assert_eq!(
            run_bounds(&[(2.0, 0.0), (1.0, 1.0), (0.0, 1.0)]),
            (2.0, 4.0)
        );
        // no ties across groups: one count
        assert_eq!(
            run_bounds(&[(1.0, 0.0), (0.0, 1.0), (1.0, 0.0)]),
            (3.0, 3.0)
        );
        // a block of 2 A and 2 B alone: 2 to 4 runs
        assert_eq!(run_bounds(&[(2.0, 2.0)]), (2.0, 4.0));
    }

    /// Every arrangement of `a` first-group and `b` second-group records as 0s and 1s.
    fn arrangements(a: usize, b: usize) -> Vec<Vec<u8>> {
        if a == 0 {
            return vec![vec![1; b]];
        }
        if b == 0 {
            return vec![vec![0; a]];
        }
        let mut out = Vec::new();
        for mut rest in arrangements(a - 1, b) {
            rest.insert(0, 0);
            out.push(rest);
        }
        for mut rest in arrangements(a, b - 1) {
            rest.insert(0, 1);
            out.push(rest);
        }
        out
    }

    #[test]
    fn run_bounds_equal_the_enumeration_of_every_arrangement() {
        // random tie patterns of up to five blocks, checked against every way to order the
        // records inside each block
        let mut state: u64 = 12345;
        let mut next = |bound: u64| {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) % bound
        };
        let mut checked = 0;
        while checked < 300 {
            let blocks: Vec<(usize, usize)> = (0..1 + next(5))
                .map(|_| (next(4) as usize, next(4) as usize))
                .filter(|&(a, b)| a + b > 0)
                .collect();
            let arranged: Vec<Vec<Vec<u8>>> =
                blocks.iter().map(|&(a, b)| arrangements(a, b)).collect();
            let total: usize = arranged.iter().map(Vec::len).product();
            if blocks.is_empty() || total > 5000 {
                continue;
            }
            // every sequence: one arrangement per block
            let (mut fewest, mut most) = (usize::MAX, 0);
            for mut index in 0..total {
                let mut sequence: Vec<u8> = Vec::new();
                for options in &arranged {
                    sequence.extend(&options[index % options.len()]);
                    index /= options.len();
                }
                let runs = 1 + sequence.windows(2).filter(|w| w[0] != w[1]).count();
                fewest = fewest.min(runs);
                most = most.max(runs);
            }
            let given: Vec<(f64, f64)> =
                blocks.iter().map(|&(a, b)| (a as f64, b as f64)).collect();
            assert_eq!(
                run_bounds(&given),
                (fewest as f64, most as f64),
                "blocks {blocks:?}"
            );
            checked += 1;
        }
    }

    #[test]
    fn moses_distribution_sums_to_one() {
        // spans of 3 control cases among 7: from 3 to 7
        let all = moses_p(7.0, 3.0, 4.0, 0.0);
        assert!((all - 1.0).abs() < 1e-12);
        // span 3: the controls adjacent, 5 of the C(7, 3) = 35 placements
        assert!((moses_p(3.0, 3.0, 4.0, 0.0) - 5.0 / 35.0).abs() < 1e-12);
    }
}
