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

/// All compositions of `t` into the groups with room left, with their multinomial weights
/// 1 / prod x_j! (the common factor t! is left out).
fn compositions(
    t: u64,
    room: &[u64],
    group: usize,
    current: &mut Vec<u64>,
    weight: f64,
    out: &mut Vec<(Vec<u64>, f64)>,
) {
    if group + 1 == room.len() {
        if t <= room[group] {
            current.push(t);
            out.push((current.clone(), weight / factorial(t)));
            current.pop();
        }
        return;
    }
    for x in 0..=t.min(room[group]) {
        current.push(x);
        compositions(t - x, room, group + 1, current, weight / factorial(x), out);
        current.pop();
    }
}

fn factorial(n: u64) -> f64 {
    (1..=n).fold(1.0, |f, i| f * i as f64)
}

/// The exact tail of the Kruskal-Wallis test. The pooled cases come as tie blocks (how many
/// cases, their joint rank times two); the groups of sizes `sizes` take them in every way the
/// cases can be labelled (a block putting x_j of its cases in group j in t! / prod x_j! ways).
/// H is monotone in the sum over groups of (rank sum)^2 / n, so the tail is the share of
/// labellings reaching the observed sum (within 1e-9 relative counts as equal), and the share
/// equal to it. The walk keeps, for all groups but the last, the counts and rank sums so far.
/// None when it passes `budget` transitions.
pub(crate) fn kruskal_tail(
    blocks: &[(u64, i64)],
    sizes: &[u64],
    observed: &[i64],
    budget: f64,
) -> Option<(f64, f64)> {
    let k = sizes.len();
    let total_rank: i64 = blocks.iter().map(|&(t, r)| t as i64 * r).sum();
    let statistic = |sums: &[i64]| -> f64 {
        sums.iter()
            .zip(sizes)
            .map(|(&s, &n)| (s as f64) * (s as f64) / n as f64)
            .sum()
    };
    let target = statistic(observed);
    // state: counts then rank sums of groups 0..k-1
    let mut states: HashMap<Vec<i64>, f64> = HashMap::new();
    states.insert(vec![0; 2 * (k - 1)], 1.0);
    let mut seen = 0_u64;
    let mut work = 0.0;
    for &(t, doubled) in blocks {
        let mut next: HashMap<Vec<i64>, f64> = HashMap::with_capacity(states.len() * 2);
        for (state, weight) in &states {
            let used: i64 = state[..k - 1].iter().sum();
            let last_count = seen as i64 - used;
            let mut room: Vec<u64> = (0..k - 1).map(|g| sizes[g] - state[g] as u64).collect();
            room.push(sizes[k - 1] - last_count as u64);
            let mut options = Vec::new();
            compositions(t, &room, 0, &mut Vec::new(), 1.0, &mut options);
            work += options.len() as f64;
            if work > budget {
                return None;
            }
            for (x, ways) in options {
                let mut moved = state.clone();
                for g in 0..k - 1 {
                    moved[g] += x[g] as i64;
                    moved[k - 1 + g] += x[g] as i64 * doubled;
                }
                *next.entry(moved).or_insert(0.0) += weight * ways;
            }
        }
        states = next;
        seen += t;
    }
    let mut all = 0.0;
    let (mut significance, mut point) = (0.0, 0.0);
    let slack = 1e-9 * target.abs().max(1e-300);
    for (state, weight) in &states {
        let mut sums: Vec<i64> = state[k - 1..].to_vec();
        sums.push(total_rank - sums.iter().sum::<i64>());
        let value = statistic(&sums);
        all += weight;
        if value >= target - slack {
            significance += weight;
        }
        if (value - target).abs() <= slack {
            point += weight;
        }
    }
    Some(((significance / all).min(1.0), point / all))
}

/// The exact tail of the Jonckheere-Terpstra test: the J statistic (pairs of cases with the
/// earlier group's value below the later group's, a tie counting half) of every labelling of the
/// pooled cases, `blocks` being how many cases share each distinct value in increasing order.
/// Returns the two-sided share (a distance from the mean at least the observed one), the share
/// on the observed side of the mean (at least, or at most, the observed value) and the share
/// equal to it. Statistics are doubled to whole numbers. None when it passes `budget`.
pub(crate) fn jonckheere_tail(
    blocks: &[u64],
    sizes: &[u64],
    observed_doubled: i64,
    budget: f64,
) -> Option<(f64, f64, f64)> {
    let k = sizes.len();
    let total: u64 = sizes.iter().sum();
    let mean_doubled = ((total * total - sizes.iter().map(|n| n * n).sum::<u64>()) / 2) as i64;
    // state: counts of groups 0..k-1, then the doubled statistic
    let mut states: HashMap<Vec<i64>, f64> = HashMap::new();
    states.insert(vec![0; k], 1.0);
    let mut seen = 0_u64;
    let mut work = 0.0;
    for &t in blocks {
        let mut next: HashMap<Vec<i64>, f64> = HashMap::with_capacity(states.len() * 2);
        for (state, weight) in &states {
            let used: i64 = state[..k - 1].iter().sum();
            let mut earlier: Vec<i64> = state[..k - 1].to_vec();
            earlier.push(seen as i64 - used);
            let room: Vec<u64> = (0..k).map(|g| sizes[g] - earlier[g] as u64).collect();
            let mut options = Vec::new();
            compositions(t, &room, 0, &mut Vec::new(), 1.0, &mut options);
            work += options.len() as f64;
            if work > budget {
                return None;
            }
            for (x, ways) in options {
                // pairs with the cases before the block, and the ties inside it
                let mut gain = 0_i64;
                for j in 1..k {
                    let before: i64 = earlier[..j].iter().sum();
                    gain += 2 * x[j] as i64 * before;
                    for i in 0..j {
                        gain += x[i] as i64 * x[j] as i64;
                    }
                }
                let mut moved = state.clone();
                for g in 0..k - 1 {
                    moved[g] += x[g] as i64;
                }
                moved[k - 1] += gain;
                *next.entry(moved).or_insert(0.0) += weight * ways;
            }
        }
        states = next;
        seen += t;
    }
    let mut by_value: HashMap<i64, f64> = HashMap::new();
    let mut all = 0.0;
    for (state, weight) in &states {
        *by_value.entry(state[k - 1]).or_insert(0.0) += weight;
        all += weight;
    }
    let distance = (observed_doubled - mean_doubled).abs();
    let (mut two, mut one, mut point) = (0.0, 0.0, 0.0);
    for (&value, &weight) in &by_value {
        if (value - mean_doubled).abs() >= distance {
            two += weight;
        }
        let on_side = if observed_doubled >= mean_doubled {
            value >= observed_doubled
        } else {
            value <= observed_doubled
        };
        if on_side {
            one += weight;
        }
        if value == observed_doubled {
            point += weight;
        }
    }
    Some(((two / all).min(1.0), (one / all).min(1.0), point / all))
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
    fn kruskal_tail_of_three_groups_of_two_by_enumeration() {
        // six distinct cases, ranks 1 to 6 (doubled), groups of 2: 90 labellings; the observed
        // ranks (1,2 | 3,4 | 5,6) are the most extreme of the 15 up to the order of the
        // groups' sums, 6 of 90 labellings reach them
        let blocks: Vec<(u64, i64)> = (1..=6).map(|r| (1, 2 * r)).collect();
        let (p, point) = kruskal_tail(&blocks, &[2, 2, 2], &[6, 14, 22], 1e9).unwrap();
        assert!((p - 6.0 / 90.0).abs() < 1e-12, "{p}");
        assert!((point - 6.0 / 90.0).abs() < 1e-12, "{point}");
    }

    #[test]
    fn jonckheere_tail_of_three_ordered_groups_by_enumeration() {
        // six distinct cases in groups of 2: J is 12 when the groups are in order (1 2 | 3 4 |
        // 5 6) in 1 of the 90 labellings, whose mean is 6 (doubled 12, observed doubled 24)
        let (two, one, point) = jonckheere_tail(&[1; 6], &[2, 2, 2], 24, 1e9).unwrap();
        assert!((one - 1.0 / 90.0).abs() < 1e-12 && (point - 1.0 / 90.0).abs() < 1e-12);
        // the reversed order is as far from the mean on the other side
        assert!((two - 2.0 / 90.0).abs() < 1e-12, "{two}");
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
