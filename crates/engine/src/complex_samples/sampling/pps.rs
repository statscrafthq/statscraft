//! The draws of CSSELECT with probability proportional to size (PPS), their inclusion
//! probabilities and the joint inclusion probabilities of the methods analyzed as UNEQUAL_WOR.
//!
//! Rules from the Algorithms manual, in our words; p_i is the size of unit i over the total size
//! M, and n p_i is its inclusion probability unless said otherwise:
//! - PPS_WR: n uniform points on (0, M); a unit is hit once for every point that falls in its
//!   share of the running total. Inclusion probability 1 - (1 - p_i)^n, weight 1 / (n p_i) per hit.
//! - PPS_SYSTEMATIC: points S, S + I, ..., S + (n - 1) I with I = M / n and S uniform on (0, I),
//!   hits counted the same way.
//! - PPS_CHROMY: a first unit chosen in proportion to size starts the order, which wraps around;
//!   then Chromy's sequential method follows the running expected hits n (p_1 + ... + p_i).
//! - PPS_WOR (Hanurav and Vijayan): sort the units by size. When the n largest are not all of
//!   the same size, choose r from 1 to n with probability
//!   theta_r = n (q_(r+1) - q_r) (S + r a) / S, where q_r is the relative size of the r-th of the
//!   n largest, q_(n+1) = 1 / n, S the relative size of the other N - n units and a = q_1. The
//!   n - r largest units are taken; the first N - n + r units then share r draws with their
//!   relative sizes, those above a lowered to a, all divided by S + r a. Those r draws (and the
//!   whole draw when the n largest are equal) go in order: with P_k = p_k / (p_(k+1) + ...),
//!   the m-th unit is chosen among the units after the one chosen before and up to the
//!   (N - r + m)-th, in proportion to c p_j times the product of (1 - (c - 1) P_k) over the units
//!   k skipped since the last choice, c = r - m + 1. The joint probability of units i < j of that
//!   ordered part is r (r - 1) (1 - P_1) ... (1 - P_(i-1)) P_i p_j, and of the whole draw the
//!   sum over r of theta_r times 1 (both among the n - r taken), r times the lowered relative
//!   size of i (only j taken) or the ordered part's joint probability.
//! - PPS_SAMPFORD: one unit in proportion to p, then n - 1 with replacement in proportion to
//!   lambda_i = p_i / (1 - n p_i); a sample with a repeated unit is drawn again. The joint
//!   probability is K lambda_i lambda_j sum over k = 2 .. n of (k - n (p_i + p_j)) L_(n-k, ij) /
//!   n^(k-2), with L_(m, ij) the m-th elementary symmetric sum of the lambdas without i and j,
//!   L_m the one of all lambdas and 1 / K = sum over k = 1 .. n of k L_(n-k) / n^k.
//!   PPS_BREWER is Sampford's method with n = 2.
//! - PPS_MURTHY (n = 2): the first unit in proportion to p, the second in proportion to p among
//!   the others. Inclusion probability p_i (1 - p_i / (1 - p_i) + sum of p_k / (1 - p_k)), joint
//!   probability p_i p_j (2 - p_i - p_j) / ((1 - p_i) (1 - p_j)); the weight is one over the
//!   inclusion probability.
//!
//! Choice where the manual is silent: the methods without replacement other than Murthy's need
//! n p_i at most 1 (below 1 for Sampford's). A unit above that is a certainty unit: it is taken
//! with probability 1, and the rest of the sample is drawn among the other units with the
//! sizes and the sample size that remain, repeated until no unit is above. A certainty unit's
//! joint probability with a unit j is the inclusion probability of j.

use super::chromy;
use crate::Error;
use crate::random::Random;
use statscraft_syntax::CsMethod;

/// A draw: the hits, inclusion probability and weight per hit of every unit.
pub(super) struct Drawn {
    pub hits: Vec<u32>,
    pub prob: Vec<f64>,
    pub weight: Vec<f64>,
}

/// The position chosen in proportion to `weights`.
fn pick(weights: &[f64], random: &mut Random) -> usize {
    let mut u = random.uniform() * weights.iter().sum::<f64>();
    for (k, w) in weights.iter().enumerate() {
        if u < *w {
            return k;
        }
        u -= w;
    }
    // rounding left u at the very end: the last unit with a weight
    weights.iter().rposition(|w| *w > 0.0).expect("a weight")
}

/// The unit whose share of the running total `cum` holds the point `at`.
fn hit_at(cum: &[f64], at: f64) -> usize {
    cum.partition_point(|c| *c < at).min(cum.len() - 1)
}

fn running(sizes: &[f64]) -> Vec<f64> {
    sizes
        .iter()
        .scan(0.0, |s, m| {
            *s += m;
            Some(*s)
        })
        .collect()
}

/// The certainty units (those whose share of the sample would be 1 or more) and the sample
/// size left for the others.
fn certainty(sizes: &[f64], n: usize) -> (Vec<bool>, usize) {
    let mut certain = vec![false; sizes.len()];
    let mut left = n;
    loop {
        let rest: f64 = (0..sizes.len())
            .filter(|&i| !certain[i])
            .map(|i| sizes[i])
            .sum();
        let mut found = false;
        for i in 0..sizes.len() {
            if !certain[i] && left > 0 && left as f64 * sizes[i] / rest >= 1.0 - 1e-12 {
                certain[i] = true;
                left -= 1;
                found = true;
            }
        }
        if !found || left == 0 {
            return (certain, left);
        }
    }
}

/// The units after removing the certainty ones: their positions and sizes, the sample size for
/// them and the inclusion probability of every unit.
struct Rest {
    certain: Vec<bool>,
    units: Vec<usize>,
    sizes: Vec<f64>,
    n: usize,
    prob: Vec<f64>,
}

fn rest(sizes: &[f64], n: usize) -> Rest {
    let (certain, left) = certainty(sizes, n);
    let units: Vec<usize> = (0..sizes.len()).filter(|&i| !certain[i]).collect();
    let rest_sizes: Vec<f64> = units.iter().map(|&i| sizes[i]).collect();
    let total: f64 = rest_sizes.iter().sum();
    let prob = (0..sizes.len())
        .map(|i| {
            if certain[i] {
                1.0
            } else {
                left as f64 * sizes[i] / total
            }
        })
        .collect();
    Rest {
        certain,
        units,
        sizes: rest_sizes,
        n: left,
        prob,
    }
}

fn murthy_prob(p: &[f64]) -> Vec<f64> {
    let s: f64 = p.iter().map(|q| q / (1.0 - q)).sum();
    p.iter().map(|q| q * (1.0 - q / (1.0 - q) + s)).collect()
}

/// Draws `n` of the units with `sizes` (all above 0) by a PPS method; without replacement `n`
/// is below the number of units.
pub(super) fn draw(
    method: CsMethod,
    sizes: &[f64],
    n: usize,
    random: &mut Random,
) -> Result<Drawn, Error> {
    let big_n = sizes.len();
    let total: f64 = sizes.iter().sum();
    let p: Vec<f64> = sizes.iter().map(|m| m / total).collect();
    let mut hits = vec![0u32; big_n];
    match method {
        CsMethod::PpsWr => {
            let cum = running(sizes);
            for _ in 0..n {
                hits[hit_at(&cum, random.uniform() * total)] += 1;
            }
            let nf = n as f64;
            return Ok(Drawn {
                hits,
                prob: p.iter().map(|q| 1.0 - (1.0 - q).powi(n as i32)).collect(),
                weight: p.iter().map(|q| 1.0 / (nf * q)).collect(),
            });
        }
        CsMethod::PpsMurthy => {
            let first = pick(&p, random);
            let mut others = p.clone();
            others[first] = 0.0;
            hits[first] = 1;
            hits[pick(&others, random)] = 1;
            let prob = murthy_prob(&p);
            let weight = prob.iter().map(|q| 1.0 / q).collect();
            return Ok(Drawn { hits, prob, weight });
        }
        _ => {}
    }
    let r = rest(sizes, n);
    for (i, c) in r.certain.iter().enumerate() {
        hits[i] = u32::from(*c);
    }
    let picked = if r.n == 0 {
        Vec::new()
    } else {
        match method {
            CsMethod::PpsWor => Hv::new(&r.sizes, r.n).draw(random),
            CsMethod::PpsSystematic => {
                let cum = running(&r.sizes);
                let rest_total = cum[cum.len() - 1];
                let interval = rest_total / r.n as f64;
                let start = random.uniform() * interval;
                (0..r.n)
                    .map(|k| hit_at(&cum, start + k as f64 * interval))
                    .collect()
            }
            CsMethod::PpsChromy => {
                let order_first = pick(&r.sizes, random);
                let rest_total: f64 = r.sizes.iter().sum();
                let k = r.sizes.len();
                // running expected hits in the order that starts at the first unit
                let expected = running(
                    &(0..k)
                        .map(|i| r.n as f64 * r.sizes[(order_first + i) % k] / rest_total)
                        .collect::<Vec<_>>(),
                );
                let rest_hits = chromy(order_first, k, random, |i| {
                    // a whole number reached up to rounding counts as reached
                    let e = expected[i - 1];
                    let e = if (e - e.round()).abs() < 1e-9 {
                        e.round()
                    } else {
                        e
                    };
                    let whole = e.floor();
                    (whole as usize, e - whole)
                });
                (0..k).filter(|&i| rest_hits[i] > 0).collect()
            }
            _ => sampford_draw(&r.sizes, r.n, random)?,
        }
    };
    for k in picked {
        hits[r.units[k]] += 1;
    }
    let weight = r.prob.iter().map(|q| 1.0 / q).collect();
    Ok(Drawn {
        hits,
        prob: r.prob,
        weight,
    })
}

/// The joint inclusion probabilities of the units `chosen` (a draw by `method`, an UNEQUAL_WOR
/// one, with the same sizes and n), the inclusion probabilities on the diagonal.
pub(super) fn joint(method: CsMethod, sizes: &[f64], n: usize, chosen: &[usize]) -> Vec<Vec<f64>> {
    let k = chosen.len();
    let mut out = vec![vec![0.0; k]; k];
    if method == CsMethod::PpsMurthy {
        let total: f64 = sizes.iter().sum();
        let p: Vec<f64> = sizes.iter().map(|m| m / total).collect();
        let prob = murthy_prob(&p);
        for (a, &i) in chosen.iter().enumerate() {
            for (b, &j) in chosen.iter().enumerate() {
                out[a][b] = if i == j {
                    prob[i]
                } else {
                    p[i] * p[j] * (2.0 - p[i] - p[j]) / ((1.0 - p[i]) * (1.0 - p[j]))
                };
            }
        }
        return out;
    }
    let r = rest(sizes, n);
    let position = |i: usize| r.units.binary_search(&i).expect("not a certainty unit");
    let hv = (method == CsMethod::PpsWor && r.n > 0).then(|| Hv::new(&r.sizes, r.n));
    for (a, &i) in chosen.iter().enumerate() {
        for (b, &j) in chosen.iter().enumerate() {
            out[a][b] = if i == j {
                r.prob[i]
            } else if r.certain[i] {
                r.prob[j]
            } else if r.certain[j] {
                r.prob[i]
            } else {
                let (x, y) = (position(i), position(j));
                match &hv {
                    Some(hv) => hv.joint(x, y),
                    None => sampford_joint(&r.sizes, r.n, x, y),
                }
            };
        }
    }
    out
}

/// Sampford's draw of `n` units; a sample with a repeated unit is drawn again.
fn sampford_draw(sizes: &[f64], n: usize, random: &mut Random) -> Result<Vec<usize>, Error> {
    let total: f64 = sizes.iter().sum();
    let p: Vec<f64> = sizes.iter().map(|m| m / total).collect();
    let lambda: Vec<f64> = p.iter().map(|q| q / (1.0 - n as f64 * q)).collect();
    // shortcut: a fixed number of tries; units with n p_i close to 1 can make repeats almost sure
    for _ in 0..1_000_000 {
        let mut taken = vec![pick(&p, random)];
        while taken.len() < n {
            let k = pick(&lambda, random);
            if taken.contains(&k) {
                break;
            }
            taken.push(k);
        }
        if taken.len() == n {
            taken.sort_unstable();
            return Ok(taken);
        }
    }
    Err(Error(
        "PPS_SAMPFORD found no sample without a repeated unit in a million tries; a unit's share of the sample is close to 1 (PPS_WOR has no such limit)"
            .into(),
    ))
}

/// The elementary symmetric sums e_0 .. e_upto of `values`.
fn elementary(values: impl Iterator<Item = f64>, upto: usize) -> Vec<f64> {
    let mut e = vec![0.0; upto + 1];
    e[0] = 1.0;
    for v in values {
        for k in (1..=upto).rev() {
            e[k] += v * e[k - 1];
        }
    }
    e
}

fn sampford_joint(sizes: &[f64], n: usize, i: usize, j: usize) -> f64 {
    let total: f64 = sizes.iter().sum();
    let nf = n as f64;
    let p: Vec<f64> = sizes.iter().map(|m| m / total).collect();
    let lambda: Vec<f64> = p.iter().map(|q| q / (1.0 - nf * q)).collect();
    let all = elementary(lambda.iter().copied(), n);
    let without = elementary(
        lambda
            .iter()
            .enumerate()
            .filter(|(k, _)| *k != i && *k != j)
            .map(|(_, l)| *l),
        n,
    );
    let k_n = 1.0
        / (1..=n)
            .map(|k| k as f64 * all[n - k] / nf.powi(k as i32))
            .sum::<f64>();
    let sum: f64 = (2..=n)
        .map(|k| (k as f64 - nf * (p[i] + p[j])) * without[n - k] / nf.powi(k as i32 - 2))
        .sum();
    k_n * lambda[i] * lambda[j] * sum
}

/// The ordered part of Hanurav and Vijayan's scheme: r draws among relative sizes `ps` (sorted
/// up, the last r equal).
struct Ordered {
    ps: Vec<f64>,
    r: usize,
    /// P_k = ps_k / (ps_(k+1) + ...), and the products of (1 - P) before each unit.
    big_p: Vec<f64>,
    before: Vec<f64>,
}

impl Ordered {
    fn new(ps: Vec<f64>, r: usize) -> Ordered {
        let k = ps.len();
        let mut tail = vec![0.0; k + 1];
        for i in (0..k).rev() {
            tail[i] = tail[i + 1] + ps[i];
        }
        let big_p: Vec<f64> = (0..k.saturating_sub(1))
            .map(|i| ps[i] / tail[i + 1])
            .collect();
        let mut before = vec![1.0; k];
        for i in 1..k {
            before[i] = before[i - 1] * (1.0 - big_p[i - 1]);
        }
        Ordered {
            ps,
            r,
            big_p,
            before,
        }
    }

    fn draw(&self, random: &mut Random) -> Vec<usize> {
        let (k, r) = (self.ps.len(), self.r);
        let mut out = Vec::with_capacity(r);
        let mut low = 0;
        for m in 1..=r {
            let c = (r - m + 1) as f64;
            let high = k - r + m - 1;
            let mut product = 1.0;
            let mut weights = Vec::with_capacity(high + 1 - low);
            for j in low..=high {
                weights.push(c * self.ps[j] * product);
                if j < high {
                    product *= 1.0 - (c - 1.0) * self.big_p[j];
                }
            }
            let j = low + pick(&weights, random);
            out.push(j);
            low = j + 1;
        }
        out
    }

    /// Units i < j.
    fn joint(&self, i: usize, j: usize) -> f64 {
        let r = self.r as f64;
        r * (r - 1.0) * self.before[i] * self.big_p[i] * self.ps[j]
    }
}

/// Hanurav and Vijayan's scheme for `n` of the units with `sizes`, each n p at most 1.
struct Hv {
    /// The units by size, smallest first (ties in the order given).
    order: Vec<usize>,
    n: usize,
    /// When the n largest are of the same size, the whole draw is ordered.
    whole: Option<Ordered>,
    /// Otherwise theta_r and the ordered part for each r = 1 .. n.
    theta: Vec<f64>,
    parts: Vec<Ordered>,
}

impl Hv {
    fn new(sizes: &[f64], n: usize) -> Hv {
        let big_n = sizes.len();
        let mut order: Vec<usize> = (0..big_n).collect();
        order.sort_by(|a, b| sizes[*a].total_cmp(&sizes[*b]));
        let total: f64 = sizes.iter().sum();
        let p: Vec<f64> = order.iter().map(|&i| sizes[i] / total).collect();
        let first_large = big_n - n;
        if sizes[order[first_large]] == sizes[order[big_n - 1]] {
            return Hv {
                order,
                n,
                whole: Some(Ordered::new(p, n)),
                theta: Vec::new(),
                parts: Vec::new(),
            };
        }
        let s: f64 = p[..first_large].iter().sum();
        let a = p[first_large];
        let q = |r: usize| {
            if r <= n {
                p[first_large + r - 1]
            } else {
                1.0 / n as f64
            }
        };
        let nf = n as f64;
        let theta = (1..=n)
            .map(|r| nf * (q(r + 1) - q(r)) * (s + r as f64 * a) / s)
            .collect();
        let parts = (1..=n)
            .map(|r| {
                let d = s + r as f64 * a;
                let ps = (0..first_large + r)
                    .map(|k| p[k.min(first_large)] / d)
                    .collect();
                Ordered::new(ps, r)
            })
            .collect();
        Hv {
            order,
            n,
            whole: None,
            theta,
            parts,
        }
    }

    fn draw(&self, random: &mut Random) -> Vec<usize> {
        let sorted = match &self.whole {
            Some(whole) => whole.draw(random),
            None => {
                let r = pick(&self.theta, random) + 1;
                let first_large = self.order.len() - self.n;
                let mut out = self.parts[r - 1].draw(random);
                out.extend(first_large + r..self.order.len());
                out
            }
        };
        let mut units: Vec<usize> = sorted.into_iter().map(|k| self.order[k]).collect();
        units.sort_unstable();
        units
    }

    /// The joint inclusion probability of units x and y (positions in the sizes given).
    fn joint(&self, x: usize, y: usize) -> f64 {
        let rank = |u: usize| self.order.iter().position(|&o| o == u).expect("a unit");
        let (i, j) = {
            let (a, b) = (rank(x), rank(y));
            (a.min(b), a.max(b))
        };
        if let Some(whole) = &self.whole {
            return whole.joint(i, j);
        }
        let first_large = self.order.len() - self.n;
        self.theta
            .iter()
            .zip(&self.parts)
            .enumerate()
            .map(|(r0, (theta, part))| {
                let r = r0 + 1;
                let k = if i >= first_large + r {
                    1.0
                } else if j >= first_large + r {
                    r as f64 * part.ps[i]
                } else {
                    part.joint(i, j)
                };
                theta * k
            })
            .sum()
    }
}

#[cfg(test)]
// the tests index several parallel tables by unit
#[allow(clippy::needless_range_loop)]
mod tests {
    use super::*;

    /// Every sample of `n` of the units with its probability under Hanurav and Vijayan's
    /// scheme, by following every branch of the draw.
    fn hv_samples(sizes: &[f64], n: usize) -> Vec<(Vec<usize>, f64)> {
        fn ordered(
            part: &Ordered,
            low: usize,
            m: usize,
            got: &mut Vec<usize>,
            prob: f64,
            out: &mut Vec<(Vec<usize>, f64)>,
        ) {
            let (k, r) = (part.ps.len(), part.r);
            if m > r {
                out.push((got.clone(), prob));
                return;
            }
            let c = (r - m + 1) as f64;
            let high = k - r + m - 1;
            let mut product = 1.0;
            let mut weights = Vec::new();
            for j in low..=high {
                weights.push(c * part.ps[j] * product);
                if j < high {
                    product *= 1.0 - (c - 1.0) * part.big_p[j];
                }
            }
            let total: f64 = weights.iter().sum();
            for (j, w) in (low..=high).zip(&weights) {
                got.push(j);
                ordered(part, j + 1, m + 1, got, prob * w / total, out);
                got.pop();
            }
        }
        let hv = Hv::new(sizes, n);
        let big_n = sizes.len();
        let mut sorted = Vec::new();
        match &hv.whole {
            Some(whole) => ordered(whole, 0, 1, &mut Vec::new(), 1.0, &mut sorted),
            None => {
                for (r0, (theta, part)) in hv.theta.iter().zip(&hv.parts).enumerate() {
                    let mut some = Vec::new();
                    ordered(part, 0, 1, &mut Vec::new(), *theta, &mut some);
                    for (mut s, prob) in some {
                        s.extend(big_n - n + r0 + 1..big_n);
                        sorted.push((s, prob));
                    }
                }
            }
        }
        sorted
            .into_iter()
            .map(|(s, prob)| (s.into_iter().map(|k| hv.order[k]).collect(), prob))
            .collect()
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn hanurav_vijayan_gives_n_p_and_its_closed_form_joint_probabilities() {
        // exact: the probabilities of all samples, summed, against n p_i and the closed form
        for (sizes, n) in [
            (vec![1.0, 2.0, 3.0, 4.0, 5.0], 2),
            (vec![5.0, 1.0, 4.0, 2.0, 3.0], 3),
            (vec![3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0], 3),
            (vec![2.0, 3.0, 3.0, 3.0], 3),
            (vec![1.0, 1.0, 2.0, 5.0, 5.0], 2),
            (vec![4.0, 4.0, 4.0, 4.0], 2),
            (vec![7.0, 1.0, 6.0, 2.0, 5.0, 3.0, 4.0], 4),
            (vec![5.0, 1.0, 2.0], 1),
        ] {
            let samples = hv_samples(&sizes, n);
            let total: f64 = sizes.iter().sum();
            assert!(close(samples.iter().map(|s| s.1).sum(), 1.0));
            let hv = Hv::new(&sizes, n);
            for i in 0..sizes.len() {
                let pi: f64 = samples
                    .iter()
                    .filter(|s| s.0.contains(&i))
                    .map(|s| s.1)
                    .sum();
                assert!(close(pi, n as f64 * sizes[i] / total), "{sizes:?} {i}");
                for j in i + 1..sizes.len() {
                    let both: f64 = samples
                        .iter()
                        .filter(|s| s.0.contains(&i) && s.0.contains(&j))
                        .map(|s| s.1)
                        .sum();
                    assert!(close(both, hv.joint(i, j)), "{sizes:?} {i} {j}");
                }
            }
        }
    }

    #[test]
    fn sampford_joint_probabilities_match_the_sample_probabilities() {
        // exact: Sampford's draw gives sample s the probability proportional to
        // sum over i in s of p_i times the product of lambda over the rest of s
        for (sizes, n) in [
            (vec![1.0, 2.0, 3.0, 4.0, 5.0], 2),
            (vec![1.0, 2.0, 3.0, 4.0, 6.0], 3),
            (vec![2.0, 3.0, 3.0, 4.0, 5.0, 6.0, 7.0], 4),
        ] {
            let big_n = sizes.len();
            let total: f64 = sizes.iter().sum();
            let p: Vec<f64> = sizes.iter().map(|m| m / total).collect();
            let lambda: Vec<f64> = p.iter().map(|q| q / (1.0 - n as f64 * q)).collect();
            let mut samples = Vec::new();
            for mask in 0u32..1 << big_n {
                if mask.count_ones() as usize != n {
                    continue;
                }
                let s: Vec<usize> = (0..big_n).filter(|k| mask & (1 << k) != 0).collect();
                let w: f64 = s
                    .iter()
                    .map(|&i| {
                        p[i] * s
                            .iter()
                            .filter(|&&k| k != i)
                            .map(|&k| lambda[k])
                            .product::<f64>()
                    })
                    .sum();
                samples.push((s, w));
            }
            let sum: f64 = samples.iter().map(|s| s.1).sum();
            for i in 0..big_n {
                let pi: f64 = samples
                    .iter()
                    .filter(|s| s.0.contains(&i))
                    .map(|s| s.1)
                    .sum::<f64>()
                    / sum;
                assert!(close(pi, n as f64 * p[i]));
                for j in i + 1..big_n {
                    let both: f64 = samples
                        .iter()
                        .filter(|s| s.0.contains(&i) && s.0.contains(&j))
                        .map(|s| s.1)
                        .sum::<f64>()
                        / sum;
                    assert!(close(both, sampford_joint(&sizes, n, i, j)), "{i} {j}");
                }
            }
        }
    }

    #[test]
    fn certainty_units_are_taken_and_the_rest_drawn_among_the_others() {
        // sizes 10, 1, 1, 1, 1 with n = 3: 3 * 10 / 14 > 1, so unit 0 is certain and the other
        // two draws share 4 units of size 1: probability 1/2 each, joint 1/2 * 1/3 = 1/6
        let sizes = [10.0, 1.0, 1.0, 1.0, 1.0];
        for method in [
            CsMethod::PpsWor,
            CsMethod::PpsSampford,
            CsMethod::PpsSystematic,
            CsMethod::PpsChromy,
        ] {
            for seed in 1..50 {
                let d = draw(method, &sizes, 3, &mut Random::seeded(seed)).unwrap();
                assert_eq!(d.hits[0], 1);
                assert_eq!(d.hits.iter().sum::<u32>(), 3, "{method:?}");
                assert!(d.hits.iter().all(|h| *h <= 1));
                assert_eq!(d.prob, vec![1.0, 0.5, 0.5, 0.5, 0.5]);
            }
        }
        let j = joint(CsMethod::PpsSampford, &sizes, 3, &[0, 2, 4]);
        assert!(close(j[0][1], 0.5) && close(j[1][2], 1.0 / 6.0) && close(j[0][0], 1.0));
    }

    #[test]
    fn murthy_inclusion_and_joint_probabilities_are_exact() {
        // sizes 1, 2, 3, 4: p = 0.1 .. 0.4; pi_1 = 0.1 (1 - 1/9 + 1/9 + 2/8 + 3/7 + 4/6)
        let sizes = [1.0, 2.0, 3.0, 4.0];
        let d = draw(CsMethod::PpsMurthy, &sizes, 2, &mut Random::seeded(3)).unwrap();
        let s = 0.1 / 0.9 + 0.2 / 0.8 + 0.3 / 0.7 + 0.4 / 0.6;
        assert!(close(d.prob[0], 0.1 * (1.0 - 0.1 / 0.9 + s)));
        assert!(close(d.prob.iter().sum(), 2.0));
        let j = joint(CsMethod::PpsMurthy, &sizes, 2, &[0, 3]);
        assert!(close(j[0][1], 0.1 * 0.4 * 1.5 / (0.9 * 0.6)));
    }

    #[test]
    fn empirical_inclusion_and_joint_frequencies_match_the_stated_probabilities() {
        // 20000 draws of 3 of 6 units; a probability of up to 1 is estimated with a binomial
        // standard error of at most sqrt(0.25 / 20000) = 0.0035, so 0.015 is over 4 of them
        let sizes = [1.0, 2.0, 2.0, 3.0, 4.0, 5.0];
        let draws = 20000;
        for method in [
            CsMethod::PpsWor,
            CsMethod::PpsSampford,
            CsMethod::PpsSystematic,
            CsMethod::PpsChromy,
            CsMethod::PpsBrewer,
            CsMethod::PpsMurthy,
            CsMethod::PpsWr,
        ] {
            let n = if method.draws_two() { 2 } else { 3 };
            let mut random = Random::seeded(20261117);
            let mut once = [0.0; 6];
            let mut both = [[0.0; 6]; 6];
            let mut prob = Vec::new();
            for _ in 0..draws {
                let d = draw(method, &sizes, n, &mut random).unwrap();
                prob = d.prob;
                for i in 0..6 {
                    if d.hits[i] > 0 {
                        once[i] += 1.0;
                        for j in 0..6 {
                            if d.hits[j] > 0 {
                                both[i][j] += 1.0;
                            }
                        }
                    }
                }
            }
            for i in 0..6 {
                let f = once[i] / draws as f64;
                assert!(
                    (f - prob[i]).abs() < 0.015,
                    "{method:?} {i}: {f} {}",
                    prob[i]
                );
            }
            if method.estimator() == statscraft_syntax::CsEstimator::UnequalWor {
                let all: Vec<usize> = (0..6).collect();
                let j = joint(method, &sizes, n, &all);
                for a in 0..6 {
                    for b in 0..6 {
                        let f = both[a][b] / draws as f64;
                        assert!(
                            (f - j[a][b]).abs() < 0.015,
                            "{method:?} {a} {b}: {f} {}",
                            j[a][b]
                        );
                    }
                }
            }
        }
    }
}
