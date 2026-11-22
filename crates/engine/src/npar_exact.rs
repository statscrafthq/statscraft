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
    fn two_cases_of_three_ranks_by_enumeration() {
        // ranks 1 2 3 twice: column sums (2,4,6) in the observed order; the 36 pairs of orders
        // give sum of squares 56 in 6 of them (the same order twice), the maximum
        let tail = within_case_tail(&[(vec![1, 2, 3], 2)], &[2, 4, 6], 1e9).unwrap();
        assert!((tail.significance - 6.0 / 36.0).abs() < 1e-12);
        assert!((tail.point - 6.0 / 36.0).abs() < 1e-12);
    }
}
