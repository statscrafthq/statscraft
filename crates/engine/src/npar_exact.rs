//! Exact significance of the NPAR TESTS that permute within cases (Friedman, Kendall's W and
//! Cochran's Q): under the null hypothesis the values of each case fall in the columns in any
//! order, independently across cases. Each statistic is a monotone function of the sum of the
//! squared column sums (the rest is fixed by the cases), so the exact significance is the
//! probability that sum reaches the observed one.
//!
//! The columns are exchangeable, so the walk over cases keeps the multiset of column sums (a
//! sorted vector) with its probability; a case adds each distinct arrangement of its values with
//! equal probability.

use std::collections::HashMap;

/// The probability that the sum of squared column sums is at least the observed one, and
/// that it equals it.
pub(crate) struct Tail {
    pub significance: f64,
    pub point: f64,
}

/// The distinct arrangements of `values` over the columns.
fn arrangements(values: &[i64]) -> Vec<Vec<i64>> {
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let mut out = Vec::new();
    let mut used = vec![false; sorted.len()];
    let mut current = Vec::with_capacity(sorted.len());
    fn walk(sorted: &[i64], used: &mut [bool], current: &mut Vec<i64>, out: &mut Vec<Vec<i64>>) {
        if current.len() == sorted.len() {
            out.push(current.clone());
            return;
        }
        for i in 0..sorted.len() {
            // equal values at one position give the same arrangement
            if used[i] || (i > 0 && sorted[i] == sorted[i - 1] && !used[i - 1]) {
                continue;
            }
            used[i] = true;
            current.push(sorted[i]);
            walk(sorted, used, current, out);
            current.pop();
            used[i] = false;
        }
    }
    walk(&sorted, &mut used, &mut current, &mut out);
    out
}

/// The exact tail for `blocks` (a case's integer values and how many cases it stands for) and
/// the observed column sums. None when the work passes `budget` additions.
pub(crate) fn within_case_tail(
    blocks: &[(Vec<i64>, u64)],
    observed: &[i64],
    budget: f64,
) -> Option<Tail> {
    let k = observed.len();
    let square =
        |sums: &[i64]| -> i128 { sums.iter().map(|&s| i128::from(s) * i128::from(s)).sum() };
    let target = square(observed);
    let mut states: HashMap<Vec<i64>, f64> = HashMap::new();
    states.insert(vec![0; k], 1.0);
    let mut work = 0.0;
    for (values, count) in blocks {
        let options = arrangements(values);
        let share = 1.0 / options.len() as f64;
        for _ in 0..*count {
            work += states.len() as f64 * options.len() as f64;
            if work > budget {
                return None;
            }
            let mut next: HashMap<Vec<i64>, f64> = HashMap::with_capacity(states.len() * 2);
            for (sums, probability) in &states {
                for option in &options {
                    let mut moved: Vec<i64> = sums.iter().zip(option).map(|(a, b)| a + b).collect();
                    moved.sort_unstable();
                    *next.entry(moved).or_insert(0.0) += probability * share;
                }
            }
            states = next;
        }
    }
    let (mut significance, mut point) = (0.0, 0.0);
    for (sums, probability) in &states {
        let s = square(sums);
        if s >= target {
            significance += probability;
        }
        if s == target {
            point += probability;
        }
    }
    Some(Tail {
        significance: significance.min(1.0),
        point,
    })
}

/// The exact two-sample Kolmogorov-Smirnov tail: with the distinct values as blocks of (first
/// group's, second group's) counts, the share of the C(n1 + n2, n1) assignments of the pooled
/// cases to the groups (the cases of a tie are interchangeable) whose largest absolute gap
/// between the two distribution functions, taken at the distinct values, is at least the
/// observed one, and the share where it is equal to it. `gap` is the observed gap times n1 n2,
/// a whole number.
pub(crate) fn smirnov_exact(blocks: &[(u64, u64)], gap: i64) -> (f64, f64) {
    let n1: u64 = blocks.iter().map(|b| b.0).sum();
    let n2: u64 = blocks.iter().map(|b| b.1).sum();
    let total_cases = n1 + n2;
    // ways[t][x] = C(t, x) for the blocks' sizes, as f64
    let choose = |t: u64, x: u64| -> f64 {
        let mut c = 1.0;
        for i in 0..x {
            c = c * (t - i) as f64 / (i + 1) as f64;
        }
        c
    };
    // the number of assignments whose gaps all stay within `limit` (inclusive)
    let within = |limit: i64| -> f64 {
        let mut ways = vec![0.0; n1 as usize + 1];
        ways[0] = 1.0;
        let mut seen = 0_u64;
        for &(c1, c2) in blocks {
            let t = c1 + c2;
            let mut next = vec![0.0; n1 as usize + 1];
            for (a, &w) in ways.iter().enumerate() {
                if w == 0.0 {
                    continue;
                }
                for x in 0..=t.min(n1 - a as u64) {
                    let a2 = a as u64 + x;
                    let b2 = seen + t - a2;
                    if b2 > n2 {
                        continue;
                    }
                    let diff = (a2 as i64 * n2 as i64 - b2 as i64 * n1 as i64).abs();
                    if diff <= limit {
                        next[a2 as usize] += w * choose(t, x);
                    }
                }
            }
            ways = next;
            seen += t;
        }
        ways[n1 as usize]
    };
    let all = choose(total_cases, n1);
    let below = within(gap - 1);
    let up_to = within(gap);
    (
        ((all - below) / all).clamp(0.0, 1.0),
        ((up_to - below) / all).max(0.0),
    )
}

/// The exact tail of the k-sample median test: the groups of sizes `sizes`, `above` of the
/// cases above the median in total split between them in every way (hypergeometric, the
/// totals fixed), ordered by Pearson's chi-square of the 2 x k table. The share of splits whose
/// chi-square is at least the observed one (equal within 1e-7 relative counts as equal) and the
/// share equal to it; None when there are more than `budget` splits.
pub(crate) fn median_tail(sizes: &[u64], observed: &[u64], budget: f64) -> Option<(f64, f64)> {
    let total: u64 = sizes.iter().sum();
    let above: u64 = observed.iter().sum();
    let below = total - above;
    let chi = |counts: &[u64]| -> f64 {
        sizes
            .iter()
            .zip(counts)
            .map(|(&n, &a)| {
                let e1 = n as f64 * above as f64 / total as f64;
                let e2 = n as f64 * below as f64 / total as f64;
                (a as f64 - e1).powi(2) / e1 + ((n - a) as f64 - e2).powi(2) / e2
            })
            .sum()
    };
    let choose = |n: u64, k: u64| -> f64 {
        let k = k.min(n - k);
        (0..k).fold(1.0, |c, i| c * (n - i) as f64 / (i + 1) as f64)
    };
    let splits: f64 = sizes.iter().map(|&n| (n + 1) as f64).product();
    if splits > budget {
        return None;
    }
    let target = chi(observed);
    let all = choose(total, above);
    let (mut significance, mut point) = (0.0, 0.0);
    fn walk(
        sizes: &[u64],
        left: u64,
        counts: &mut Vec<u64>,
        weight: f64,
        visit: &mut dyn FnMut(&[u64], f64),
        choose: &dyn Fn(u64, u64) -> f64,
    ) {
        let g = counts.len();
        if g + 1 == sizes.len() {
            if left <= sizes[g] {
                counts.push(left);
                visit(counts, weight * choose(sizes[g], left));
                counts.pop();
            }
            return;
        }
        for a in 0..=left.min(sizes[g]) {
            counts.push(a);
            walk(
                sizes,
                left - a,
                counts,
                weight * choose(sizes[g], a),
                visit,
                choose,
            );
            counts.pop();
        }
    }
    let mut visit = |counts: &[u64], weight: f64| {
        let value = chi(counts);
        let slack = 1e-7 * target.abs().max(1e-300);
        if value >= target - slack {
            significance += weight / all;
        }
        if (value - target).abs() <= slack {
            point += weight / all;
        }
    };
    walk(sizes, above, &mut Vec::new(), 1.0, &mut visit, &choose);
    Some((significance.min(1.0), point))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrangements_are_the_distinct_orders() {
        assert_eq!(arrangements(&[1, 2, 3]).len(), 6);
        assert_eq!(arrangements(&[0, 0, 1]).len(), 3);
        assert_eq!(arrangements(&[2, 2, 2]).len(), 1);
    }

    #[test]
    fn median_tail_of_two_groups_is_fishers_two_sided() {
        // groups of 3 and 3, 3 above the median: splits (3,0) (2,1) (1,2) (0,3) with
        // probabilities 1 20 ... of C(6,3) = 20: 1, 9, 9, 1; the extreme two reach the observed
        let (p, point) = median_tail(&[3, 3], &[3, 0], 1e9).unwrap();
        assert!((p - 2.0 / 20.0).abs() < 1e-12 && (point - 2.0 / 20.0).abs() < 1e-12);
    }

    #[test]
    fn smirnov_tail_by_enumeration() {
        // 2 + 2 distinct cases: of the 6 orders, AABB and BBAA reach the gap 4 (in units of
        // 1 / (n1 n2)) and the other four stop at 2
        let blocks = [(1, 0), (1, 0), (0, 1), (0, 1)];
        let (p, point) = smirnov_exact(&blocks, 4);
        assert!((p - 2.0 / 6.0).abs() < 1e-12 && (point - 2.0 / 6.0).abs() < 1e-12);
        let (p, _) = smirnov_exact(&blocks, 2);
        assert!((p - 1.0).abs() < 1e-12);
    }

    #[test]
    fn two_cases_of_three_ranks_by_enumeration() {
        // ranks 1 2 3 twice: column sums (2,4,6) in the observed order; the 36 pairs of orders
        // give sum of squares 56 in 6 of them (the same order twice), the maximum
        let tail = within_case_tail(&[(vec![1, 2, 3], 2)], &[2, 4, 6], 1e9).unwrap();
        assert!((tail.significance - 6.0 / 36.0).abs() < 1e-12);
        assert!((tail.point - 6.0 / 36.0).abs() < 1e-12);
    }
}
