//! Exact significance of the NPAR TESTS that permute within cases (Friedman, Kendall's W and
//! Cochran's Q): under the null hypothesis the values of each case fall in the columns in any
//! order, independently across cases. Each statistic is a monotone function of the sum of the
//! squared column sums (the rest is fixed by the cases), so the exact significance is the
//! probability that sum reaches the observed one.
//!
//! The columns are exchangeable, so the walk over cases keeps the multiset of column sums (a
//! sorted vector) with its probability; a case adds each distinct arrangement of its values with
//! equal probability.

use crate::ctables::FastMap;

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
    if within_case_floor(blocks, budget) > budget {
        return None;
    }
    within_case_walk(blocks, observed, budget)
}

/// The within-case walk itself.
fn within_case_walk(blocks: &[(Vec<i64>, u64)], observed: &[i64], budget: f64) -> Option<Tail> {
    let k = observed.len();
    let square =
        |sums: &[i64]| -> i128 { sums.iter().map(|&s| i128::from(s) * i128::from(s)).sum() };
    let target = square(observed);
    let options: Vec<Vec<Vec<i64>>> = blocks.iter().map(|b| arrangements(&b.0)).collect();
    // A sorted state plus the case's values in increasing order is a sorted state, and two
    // states stay apart, so the states never become fewer: each case still to come costs at
    // least the states now times its arrangements. The walk stops as soon as that passes the
    // budget instead of when it gets there.
    let mut left: f64 = blocks
        .iter()
        .zip(&options)
        .map(|(b, o)| b.1 as f64 * o.len() as f64)
        .sum();
    let mut states: FastMap<Vec<i64>, f64> = FastMap::default();
    states.insert(vec![0; k], 1.0);
    let mut work = 0.0;
    let mut moved = vec![0_i64; k];
    for ((_, count), options) in blocks.iter().zip(&options) {
        let share = 1.0 / options.len() as f64;
        for _ in 0..*count {
            if work + states.len() as f64 * left > budget {
                return None;
            }
            work += states.len() as f64 * options.len() as f64;
            left -= options.len() as f64;
            let mut next: FastMap<Vec<i64>, f64> =
                FastMap::with_capacity_and_hasher(states.len() * 2, Default::default());
            for (sums, probability) in &states {
                for option in options {
                    // a plain loop and an insertion sort of the few columns, quicker than
                    // iterators and the general sort in a debug build
                    for c in 0..k {
                        let value = sums[c] + option[c];
                        let mut at = c;
                        while at > 0 && moved[at - 1] > value {
                            moved[at] = moved[at - 1];
                            at -= 1;
                        }
                        moved[at] = value;
                    }
                    // only a new state allocates its key
                    match next.get_mut(moved.as_slice()) {
                        Some(p) => *p += probability * share,
                        None => {
                            next.insert(moved.clone(), probability * share);
                        }
                    }
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

/// A lower bound of the steps of the within-case walk, found before it starts from the cases
/// whose values take two values the same distance apart (all of Cochran's do). Such a
/// case puts its r higher values in any r of the columns, so the columns' counts of higher
/// values reach every vector a 0-1 table with these row sums can have: by the Gale-Ryser
/// condition, every vector whose l largest entries add up to at most the first l column sums
/// of the conjugate table. All vectors of k counts of at most U meet it, U being the least
/// such sum over l wherever it is below the total. The states keep sorted sums, so at least
/// that many vectors over k! are different states, and the other cases only add states. The
/// sum ends early once it passes `budget` or after `FLOOR_EFFORT` work.
fn within_case_floor(blocks: &[(Vec<i64>, u64)], budget: f64) -> f64 {
    let k = blocks.first().map_or(0, |b| b.0.len());
    // beyond this k! leaves no bound and the alternating sum below loses its precision
    if !(2..=10).contains(&k) {
        return 0.0;
    }
    // the distance between the two values and how many take the higher one
    let two = |v: &[i64]| -> Option<(i64, usize)> {
        let (low, high) = (*v.iter().min()?, *v.iter().max()?);
        (low < high && v.iter().all(|&x| x == low || x == high))
            .then(|| (high - low, v.iter().filter(|&&x| x == high).count()))
    };
    let mut steps: Vec<(i64, u64)> = Vec::new();
    for (values, count) in blocks {
        if let Some((d, _)) = two(values) {
            match steps.iter_mut().find(|s| s.0 == d) {
                Some(s) => s.1 += count,
                None => steps.push((d, *count)),
            }
        }
    }
    let Some(&(step, _)) = steps.iter().max_by_key(|s| s.1) else {
        return 0.0;
    };
    let orders: f64 = (1..=k).map(|i| i as f64).product();
    // rows[r]: the two-valued cases so far with r higher values
    let mut rows = vec![0_u64; k + 1];
    let states = |rows: &[u64]| -> f64 {
        let total: u64 = (1..=k).map(|r| r as u64 * rows[r]).sum();
        let (mut most, mut first) = (total, 0_u64);
        for l in 1..k {
            first += rows[l..].iter().sum::<u64>();
            if first < total {
                most = most.min(first / l as u64);
            }
        }
        let choose = |n: u64, m: u64| (0..m).fold(1.0, |c, i| c * (n - i) as f64 / (i + 1) as f64);
        let k = k as u64;
        let mut vectors = 0.0;
        for s in 0..=k {
            let Some(rest) = total.checked_sub(s * (most + 1)) else {
                break;
            };
            let term = choose(k, s) * choose(rest + k - 1, k - 1);
            vectors += if s % 2 == 0 { term } else { -term };
        }
        (vectors / orders).floor().max(1.0)
    };
    let (mut floor, mut effort) = (0.0, 0_u64);
    for (values, count) in blocks {
        let r = two(values).filter(|t| t.0 == step).map(|t| t.1);
        let options = arrangements(values).len() as f64;
        let mut left = *count;
        while left > 0 {
            effort += (k * k) as u64;
            if floor > budget || effort > FLOOR_EFFORT {
                return floor;
            }
            // the other cases leave the bound where it is, all their copies at once
            let units = if r.is_some() { 1 } else { left };
            floor += states(&rows) * options * units as f64;
            if let Some(r) = r {
                rows[r] += 1;
            }
            left -= units;
        }
    }
    floor
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

/// How much work a lower bound of a walk's steps does before it stops short and lets the walk
/// decide (in cases, or in count vectors times the work of counting their compositions); a small
/// share of the time of the walks it lets through.
const FLOOR_EFFORT: u64 = 2_000_000;

/// A lower bound of the steps of a walk over the groups' counts (Kruskal-Wallis,
/// Jonckheere-Terpstra), found before it starts: before block b, every way to share the cases
/// seen so far between the groups (each within its size) is the count part of at least
/// `states(b, counts)` states of the walk, and each of those states takes at least
/// `options_floor` compositions of the block. A bound past the budget means the walk would stop
/// too. The sum ends early once it passes `budget` or after `FLOOR_EFFORT` work.
fn walk_floor(
    blocks: &[u64],
    sizes: &[u64],
    budget: f64,
    states: &mut dyn FnMut(usize, &[u64]) -> f64,
) -> f64 {
    fn each(
        sizes: &[u64],
        left: u64,
        counts: &mut Vec<u64>,
        visit: &mut dyn FnMut(&[u64]) -> bool,
    ) -> bool {
        let g = counts.len();
        if g + 1 == sizes.len() {
            if left > sizes[g] {
                return true;
            }
            counts.push(left);
            let go = visit(counts);
            counts.pop();
            return go;
        }
        let rest: u64 = sizes[g + 1..].iter().sum();
        for c in left.saturating_sub(rest)..=left.min(sizes[g]) {
            counts.push(c);
            let go = each(sizes, left - c, counts, visit);
            counts.pop();
            if !go {
                return false;
            }
        }
        true
    }
    let (mut floor, mut effort, mut seen) = (0.0, 0_u64, 0_u64);
    let mut room = vec![0_u64; sizes.len()];
    for (b, &t) in blocks.iter().enumerate() {
        let mut visit = |counts: &[u64]| {
            effort += (t.min(1000) + 1) * sizes.len() as u64;
            for ((r, n), c) in room.iter_mut().zip(sizes).zip(counts) {
                *r = n - c;
            }
            floor += states(b, counts) * options_floor(t, &room);
            floor <= budget && effort <= FLOOR_EFFORT
        };
        if !each(sizes, seen, &mut Vec::new(), &mut visit) {
            break;
        }
        seen += t;
    }
    floor
}

/// A lower bound of the compositions of `t` cases into groups with `room` left (at least one,
/// since the walk only keeps states it can finish): their exact number for blocks of up to a
/// thousand cases, else the C(t + q - 1, q - 1) ways the q groups with room for all of them
/// can take them alone.
fn options_floor(t: u64, room: &[u64]) -> f64 {
    if t > 1000 {
        let q = room.iter().filter(|&&r| r >= t).count() as u64;
        return (1..q).fold(1.0_f64, |c, i| c * (t + i) as f64 / i as f64);
    }
    // ways[s]: the compositions of s into the groups so far
    let t = t as usize;
    let mut ways = vec![0.0_f64; t + 1];
    ways[0] = 1.0;
    for &r in room {
        for s in 1..=t {
            ways[s] += ways[s - 1];
        }
        for s in (r as usize + 1..=t).rev() {
            ways[s] -= ways[s - r as usize - 1];
        }
    }
    ways[t].max(1.0)
}

/// How many different values a sum can take, at least, when a step of a path between its
/// smallest and largest values moves it by at most `step`.
fn values_floor(spread: i128, step: i128) -> f64 {
    if step <= 0 {
        1.0
    } else {
        (spread / step + 1) as f64
    }
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
    if kruskal_floor(blocks, sizes, budget) > budget {
        return None;
    }
    kruskal_walk(blocks, sizes, observed, budget)
}

/// A lower bound of the steps of the Kruskal-Wallis walk (see `walk_floor`).
fn kruskal_floor(blocks: &[(u64, i64)], sizes: &[u64], budget: f64) -> f64 {
    // A group of c of the cases seen so far sums anything from their c lowest ranks to their c
    // highest, and moving one case to the next higher rank moves the sum by one gap at most,
    // so the states with given counts are at least the spread over the largest gap, plus one.
    let mut seen_ranks: Vec<(i64, u64)> = Vec::new();
    let mut below: Vec<(u64, i128)> = Vec::new();
    let mut gap = 0_i128;
    let mut built = usize::MAX;
    let mut floor = |b: usize, counts: &[u64]| -> f64 {
        if built != b {
            seen_ranks = blocks[..b].iter().map(|&(t, r)| (r, t)).collect();
            seen_ranks.sort_unstable();
            below = vec![(0, 0)];
            for &(r, t) in &seen_ranks {
                let (cases, sum) = below[below.len() - 1];
                below.push((cases + t, sum + i128::from(r) * i128::from(t)));
            }
            gap = seen_ranks
                .windows(2)
                .map(|w| i128::from(w[1].0 - w[0].0))
                .max()
                .unwrap_or(0);
            built = b;
        }
        let lowest = |m: u64| -> i128 {
            let i = below.partition_point(|p| p.0 <= m) - 1;
            let (cases, sum) = below[i];
            if m == cases {
                sum
            } else {
                sum + i128::from(m - cases) * i128::from(seen_ranks[i].0)
            }
        };
        let (seen, all) = below[below.len() - 1];
        counts
            .iter()
            .map(|&c| values_floor(all - lowest(seen - c) - lowest(c), gap))
            .fold(1.0, f64::max)
    };
    let counts: Vec<u64> = blocks.iter().map(|b| b.0).collect();
    walk_floor(&counts, sizes, budget, &mut floor)
}

/// The Kruskal-Wallis walk itself.
fn kruskal_walk(
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
    let mut states: FastMap<Vec<i64>, f64> = FastMap::default();
    states.insert(vec![0; 2 * (k - 1)], 1.0);
    let mut seen = 0_u64;
    let mut work = 0.0;
    for &(t, doubled) in blocks {
        let mut next: FastMap<Vec<i64>, f64> =
            FastMap::with_capacity_and_hasher(states.len() * 2, Default::default());
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
    if jonckheere_floor(blocks, sizes, budget) > budget {
        return None;
    }
    jonckheere_walk(blocks, sizes, observed_doubled, budget)
}

/// A lower bound of the steps of the Jonckheere-Terpstra walk (see `walk_floor`).
fn jonckheere_floor(blocks: &[u64], sizes: &[u64], budget: f64) -> f64 {
    // Two neighbouring groups g and g + 1 of a and b cases can trade labels without changing
    // the pairs with the other groups. Taking the lowest a + b seen cases, the doubled J of
    // their pairs runs from about 0 (the later group lowest) to 2ab (the earlier one lowest),
    // short of ties of one block at each end. Swapping two neighbouring cases of the two groups
    // moves it by at most the sizes of their two blocks, so the states with given counts are at
    // least that spread over the largest pair of neighbouring blocks, plus one.
    let (mut gap, mut tied, mut built) = (0_i128, 0_i128, usize::MAX);
    let mut floor = |b: usize, counts: &[u64]| -> f64 {
        if built != b {
            let seen = &blocks[..b];
            gap = seen
                .windows(2)
                .map(|w| i128::from(w[0] + w[1]))
                .max()
                .unwrap_or(0);
            tied = seen
                .iter()
                .map(|&t| i128::from(t))
                .max()
                .unwrap_or(0)
                .pow(2)
                / 4;
            built = b;
        }
        counts
            .windows(2)
            .map(|w| {
                let pairs = i128::from(w[0]) * i128::from(w[1]);
                values_floor(2 * pairs - 2 * pairs.min(tied), gap)
            })
            .fold(1.0, f64::max)
    };
    walk_floor(blocks, sizes, budget, &mut floor)
}

/// The Jonckheere-Terpstra walk itself.
fn jonckheere_walk(
    blocks: &[u64],
    sizes: &[u64],
    observed_doubled: i64,
    budget: f64,
) -> Option<(f64, f64, f64)> {
    let k = sizes.len();
    let total: u64 = sizes.iter().sum();
    let mean_doubled = ((total * total - sizes.iter().map(|n| n * n).sum::<u64>()) / 2) as i64;
    // state: counts of groups 0..k-1, then the doubled statistic
    let mut states: FastMap<Vec<i64>, f64> = FastMap::default();
    states.insert(vec![0; k], 1.0);
    let mut seen = 0_u64;
    let mut work = 0.0;
    for &t in blocks {
        let mut next: FastMap<Vec<i64>, f64> =
            FastMap::with_capacity_and_hasher(states.len() * 2, Default::default());
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
    let mut by_value: FastMap<i64, f64> = FastMap::default();
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

/// The exact tail of the marginal homogeneity test. Each case off the diagonal is a pair of
/// values (first, second); the statistic is the sum of the first values (the scores are the
/// values) and, given the unordered pairs, each case puts either value of its pair first with
/// probability 1/2. `groups` holds each unordered pair (a, b) with how many cases have it.
/// Returns the two-sided share (a distance from the mean at least the observed one), the share
/// on the observed side and the share equal to the observed sum. None when the sums of
/// possible values pass `budget` steps.
pub(crate) fn marginal_tail(
    groups: &[(f64, f64, u64)],
    observed: f64,
    budget: f64,
) -> Option<(f64, f64, f64)> {
    // sums are kept as whole millionths so equal sums meet in one entry
    let key = |v: f64| (v * 1e6).round() as i64;
    let mut sums: FastMap<i64, f64> = FastMap::default();
    sums.insert(0, 1.0);
    let mut work = 0.0;
    let mut mean = 0.0;
    // every sum plus the second value of each case is a sum of the next step, so the sums never
    // become fewer: the steps still to come are at least the sums now times their choices
    let mut left: f64 = groups.iter().map(|g| g.2 as f64 + 1.0).sum();
    for &(a, b, m) in groups {
        // 0.5^m and C(m, x) leave the range of a float far beyond what the walk can do anyway
        if m > 1000 || work + sums.len() as f64 * left > budget {
            return None;
        }
        left -= m as f64 + 1.0;
        mean += m as f64 * (a + b) / 2.0;
        let mut choose = vec![1.0_f64; m as usize + 1];
        for x in 1..=m as usize {
            choose[x] = choose[x - 1] * (m as usize - x + 1) as f64 / x as f64;
        }
        let half = 0.5_f64.powi(m as i32);
        work += sums.len() as f64 * (m as f64 + 1.0);
        if work > budget {
            return None;
        }
        let mut next: FastMap<i64, f64> =
            FastMap::with_capacity_and_hasher(sums.len() * 2, Default::default());
        for (&t, &p) in &sums {
            for (x, c) in choose.iter().enumerate() {
                let moved = t + key(a) * x as i64 + key(b) * (m as i64 - x as i64);
                *next.entry(moved).or_insert(0.0) += p * c * half;
            }
        }
        sums = next;
    }
    let target = key(observed);
    let centre = key(mean);
    let distance = (target - centre).abs();
    let (mut two, mut one, mut point) = (0.0, 0.0, 0.0);
    for (&t, &p) in &sums {
        if (t - centre).abs() >= distance {
            two += p;
        }
        if (target >= centre && t >= target) || (target < centre && t <= target) {
            one += p;
        }
        if t == target {
            point += p;
        }
    }
    Some((two.min(1.0), one.min(1.0), point))
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
    fn marginal_tail_of_one_pair_is_a_binomial() {
        // 4 cases of the pair (1, 3): the sum of first scores is 4 + 2 x, x ~ Bin(4, 1/2) of
        // the cases that put 1 first; the observed 6 + 6 = 12 (all first scores 3) is the
        // maximum, probability 1/16, and mirrors the minimum 4
        let (two, one, point) = marginal_tail(&[(1.0, 3.0, 4)], 12.0, 1e9).unwrap();
        assert!((one - 1.0 / 16.0).abs() < 1e-12 && (point - 1.0 / 16.0).abs() < 1e-12);
        assert!((two - 2.0 / 16.0).abs() < 1e-12, "{two}");
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

    /// Small synthetic cases from a fixed linear congruential sequence.
    fn sequence(seed: u64) -> impl FnMut(u64) -> u64 {
        let mut state = seed;
        move |below| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 33) % below
        }
    }

    // A walk stops exactly when its steps pass the budget, so with the budget just under the
    // bound it must stop whenever the bound is right.
    #[test]
    fn the_bounds_never_pass_the_steps_of_the_walks() {
        let mut biggest = [0.0_f64; 3];
        for seed in 0..40 {
            let mut next = sequence(seed);
            let k = 2 + next(3) as usize;
            let sizes: Vec<u64> = (0..k).map(|_| 2 + next(4)).collect();
            let total: u64 = sizes.iter().sum();
            // tie blocks of one to three cases, in a scrambled order of ranks for K-W
            let mut blocks = Vec::new();
            let mut left = total;
            while left > 0 {
                let t = (1 + next(3)).min(left);
                blocks.push(t);
                left -= t;
            }
            let floor = jonckheere_floor(&blocks, &sizes, f64::INFINITY);
            biggest[0] = biggest[0].max(floor);
            assert!(
                jonckheere_walk(&blocks, &sizes, 0, floor - 0.5).is_none(),
                "{seed}"
            );
            let mut start = 0;
            let mut ranked: Vec<(u64, i64)> = blocks
                .iter()
                .map(|&t| {
                    let doubled = 2 * start as i64 + t as i64 + 1;
                    start += t;
                    (t, doubled)
                })
                .collect();
            let turn = next(ranked.len() as u64) as usize;
            ranked.rotate_left(turn);
            let floor = kruskal_floor(&ranked, &sizes, f64::INFINITY);
            biggest[1] = biggest[1].max(floor);
            let observed = vec![0; k];
            assert!(
                kruskal_walk(&ranked, &sizes, &observed, floor - 0.5).is_none(),
                "{seed}"
            );
            // cases of 0-1 values (some with a third value), each standing for one or two
            let columns = 3 + next(2) as usize;
            let cases: Vec<(Vec<i64>, u64)> = (0..4 + next(6))
                .map(|_| {
                    let values = (0..columns).map(|_| next(2) as i64 * (1 + next(8) / 7) as i64);
                    (values.collect(), 1 + next(2))
                })
                .collect();
            let floor = within_case_floor(&cases, f64::INFINITY);
            biggest[2] = biggest[2].max(floor);
            let observed = vec![0; columns];
            assert!(
                within_case_walk(&cases, &observed, floor - 0.5).is_none(),
                "{seed}"
            );
        }
        // the bounds are more than one step per case somewhere
        assert!(biggest.iter().all(|&b| b > 100.0), "{biggest:?}");
    }

    // 200 cases with distinct values in four groups of 50: far more steps than the budget,
    // known before the walk
    #[test]
    fn large_cases_are_rejected_before_the_walk() {
        let sizes = [50; 4];
        let ranked: Vec<(u64, i64)> = (1..=200).map(|r| (1, 2 * r)).collect();
        assert!(kruskal_floor(&ranked, &sizes, 3e7) > 3e7);
        assert!(kruskal_tail(&ranked, &sizes, &[0; 4], 3e7).is_none());
        assert!(jonckheere_floor(&[1; 200], &sizes, 3e7) > 3e7);
        assert!(jonckheere_tail(&[1; 200], &sizes, 0, 3e7).is_none());
        // five 0-1 columns of 200 cases with two hits each
        let cases: Vec<(Vec<i64>, u64)> = (0..200)
            .map(|i| {
                (
                    (0..5)
                        .map(|j| i64::from(j == i % 5 || j == (i + 2) % 5))
                        .collect(),
                    1,
                )
            })
            .collect();
        assert!(within_case_floor(&cases, 4e8) > 4e8);
        assert!(within_case_tail(&cases, &[80; 5], 4e8).is_none());
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
