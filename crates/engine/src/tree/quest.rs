//! QUEST: the choice of the predictor and of the split point of a node, for a nominal dependent.
//! The growing, surrogates, pruning and tables are those of CRT (`crt.rs`).
//!
//! Rules from the Algorithms manual (TREE Algorithms, QUEST) and the Command Syntax Reference
//! (TREE, QUEST), in our words. Tests use the cases of the node that have a value of the
//! predictor; the classes and categories counted are those present among them.
//! - Selection: a scale or ordinal predictor is tested by the ANOVA F of its values across the
//!   classes, a nominal one by the Pearson chi-square of its categories against the classes. The
//!   predictor with the smallest p-value is taken when that is below alpha / M, M the number of
//!   predictors. Otherwise the scale and ordinal predictors are tested again by the same F on the
//!   absolute deviations of their values from the class means (Levene), and the one with the
//!   smallest p-value is taken when that is below alpha / (M + M1), M1 the scale and ordinal
//!   predictors. Otherwise the node is not split. A test with nothing to test has p = 1.
//! - Split point of a scale or ordinal predictor X: the classes are put in two superclasses. With
//!   two classes they are the superclasses; when every class mean of X is alike, the class with
//!   the most cases (the lowest index on ties) is one and the rest the other; otherwise the class
//!   means are split by 2-means from the lowest and the highest, assigning each mean to the nearer
//!   centre (the lower on ties) and each centre the plain mean of its class means. Superclass A is
//!   the one with the lower mean. With a variance of zero in one superclass, the split point sits
//!   a relative 1e-12 beyond its mean towards the other mean. Otherwise it is a root of the
//!   quadratic discriminant equation of the two superclasses, normal with their means and sample
//!   variances (divisor weight - 1, zero for a superclass of one case), with the superclass
//!   probabilities p(A|t) from the priors and node class weights: the root nearer the mean of A,
//!   or the single root, when it leaves cases on both sides; the midpoint of the means when there
//!   is no such root. The cut is x <= d.
//! - A nominal predictor with two categories sends the first present category left and the other
//!   right. One with more (up to 25) is scored once on the whole sample by its largest
//!   discriminant coordinate (see `coordinates`); the scores stand in for values in the choice of
//!   superclasses and split point, the categories at or below the cut go to one child, and the
//!   child with the first category present in the node is the left one.
//! - A split that leaves a child under MINCHILDSIZE (weight of the cases with a value) is not made;
//!   the node stays a leaf. IMPROVEMENT plays no part; the split's test is what the tables report.

use super::QuestSplit;
use super::crt::Growth;
use crate::dist;

/// A test of a predictor: its name, statistic, degrees of freedom and p-value.
struct Test {
    name: &'static str,
    statistic: f64,
    df: f64,
    df2: Option<f64>,
    sig: f64,
}

/// A case of the node: the value of the predictor, its class and weight.
type Obs = (f64, usize, f64);

/// The ANOVA F test of the values across the classes present, None when there is nothing to
/// test: under two classes or no degree of freedom within them, or no spread at all.
fn anova(obs: &[Obs], classes: usize, name: &'static str) -> Option<Test> {
    let total: f64 = obs.iter().map(|o| o.2).sum();
    // centred, so sums of squares do not cancel for values far from zero
    let centre = obs.iter().map(|o| o.0 * o.2).sum::<f64>() / total;
    let (mut weight, mut sum) = (vec![0.0; classes], vec![0.0; classes]);
    for &(x, j, w) in obs {
        weight[j] += w;
        sum[j] += w * (x - centre);
    }
    let present = weight.iter().filter(|&&w| w > 0.0).count();
    let (dfn, dfd) = ((present as f64) - 1.0, total - present as f64);
    if present < 2 || dfd <= 0.0 {
        return None;
    }
    let mean: Vec<f64> = (0..classes)
        .map(|j| {
            if weight[j] > 0.0 {
                sum[j] / weight[j]
            } else {
                0.0
            }
        })
        .collect();
    let between: f64 = (0..classes).map(|j| weight[j] * mean[j] * mean[j]).sum();
    let within: f64 = obs
        .iter()
        .map(|&(x, j, w)| w * (x - centre - mean[j]).powi(2))
        .sum();
    if between <= 0.0 && within <= 0.0 {
        return None;
    }
    let f = if between <= 0.0 {
        0.0
    } else if within <= 1e-12 * between {
        f64::INFINITY
    } else {
        (between / dfn) / (within / dfd)
    };
    let sig = if f.is_infinite() {
        0.0
    } else {
        dist::f_p(f, dfn, dfd)
    };
    Some(Test {
        name,
        statistic: f,
        df: dfn,
        df2: Some(dfd),
        sig,
    })
}

/// The Levene test: the F test of the absolute deviations from the class means.
fn levene(obs: &[Obs], classes: usize) -> Option<Test> {
    let (mut weight, mut sum) = (vec![0.0; classes], vec![0.0; classes]);
    for &(x, j, w) in obs {
        weight[j] += w;
        sum[j] += w * x;
    }
    let deviations: Vec<Obs> = obs
        .iter()
        .map(|&(x, j, w)| ((x - sum[j] / weight[j]).abs(), j, w))
        .collect();
    anova(&deviations, classes, "Levene F")
}

/// The Pearson chi-square test of categories (rows) against classes, None under two of either.
fn pearson(cells: &[(usize, usize, f64)], categories: usize, classes: usize) -> Option<Test> {
    let mut table = vec![vec![0.0; classes]; categories];
    for &(i, j, w) in cells {
        table[i][j] += w;
    }
    let rows: Vec<&Vec<f64>> = table
        .iter()
        .filter(|r| r.iter().sum::<f64>() > 0.0)
        .collect();
    let columns: Vec<usize> = (0..classes)
        .filter(|&j| rows.iter().map(|r| r[j]).sum::<f64>() > 0.0)
        .collect();
    if rows.len() < 2 || columns.len() < 2 {
        return None;
    }
    let total: f64 = rows.iter().map(|r| r.iter().sum::<f64>()).sum();
    let column_total = |j: usize| rows.iter().map(|r| r[j]).sum::<f64>();
    let mut statistic = 0.0;
    for r in &rows {
        let row_total: f64 = r.iter().sum();
        for &j in &columns {
            let expected = row_total * column_total(j) / total;
            statistic += (r[j] - expected).powi(2) / expected;
        }
    }
    let df = ((rows.len() - 1) * (columns.len() - 1)) as f64;
    Some(Test {
        name: "Chi-Square",
        statistic,
        df,
        df2: None,
        sig: dist::chi_square_p(statistic, df),
    })
}

/// The weighted mean and sample variance of the values of the classes in `group`.
fn spread(obs: &[Obs], group: &[bool]) -> (f64, f64) {
    let (mut w, mut s) = (0.0, 0.0);
    for &(x, _, wt) in obs.iter().filter(|o| group[o.1]) {
        w += wt;
        s += wt * x;
    }
    let mean = s / w;
    let squares: f64 = obs
        .iter()
        .filter(|o| group[o.1])
        .map(|&(x, _, wt)| wt * (x - mean).powi(2))
        .sum();
    (mean, if w > 1.0 { squares / (w - 1.0) } else { 0.0 })
}

/// The classes of the two superclasses, A with the lower mean.
fn superclasses(obs: &[Obs], classes: usize) -> Option<Vec<bool>> {
    let (mut weight, mut sum) = (vec![0.0; classes], vec![0.0; classes]);
    for &(x, j, w) in obs {
        weight[j] += w;
        sum[j] += w * x;
    }
    let present: Vec<usize> = (0..classes).filter(|&j| weight[j] > 0.0).collect();
    if present.len() < 2 {
        return None;
    }
    let mean: Vec<f64> = (0..classes).map(|j| sum[j] / weight[j]).collect();
    let (low, high) = present
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &j| {
            (l.min(mean[j]), h.max(mean[j]))
        });
    let mut in_a = vec![false; classes];
    if present.len() == 2 {
        let lower = if mean[present[0]] <= mean[present[1]] {
            present[0]
        } else {
            present[1]
        };
        in_a[lower] = true;
    } else if high - low <= 1e-12 * low.abs().max(high.abs()) {
        // every class mean alike: the largest class against the rest
        let mut largest = present[0];
        for &j in &present {
            if weight[j] > weight[largest] {
                largest = j;
            }
        }
        in_a[largest] = true;
    } else {
        // 2-means on the class means, from the lowest and the highest
        let (mut centre_a, mut centre_b) = (low, high);
        for _ in 0..100 {
            for &j in &present {
                in_a[j] = (mean[j] - centre_a).abs() <= (mean[j] - centre_b).abs();
            }
            let centre = |side: bool| {
                let members: Vec<f64> = present
                    .iter()
                    .filter(|&&j| in_a[j] == side)
                    .map(|&j| mean[j])
                    .collect();
                members.iter().sum::<f64>() / members.len() as f64
            };
            let next = (centre(true), centre(false));
            if next == (centre_a, centre_b) {
                break;
            }
            (centre_a, centre_b) = next;
        }
    }
    // the classes of A are `true`; only the present ones matter
    Some(in_a)
}

/// The split point d of the cut x <= d, None when no cut leaves cases on both sides.
fn split_point(obs: &[Obs], in_a: &[bool], factor: &[f64], classes: usize) -> Option<f64> {
    let in_b: Vec<bool> = {
        let mut seen = vec![false; classes];
        for o in obs {
            seen[o.1] = true;
        }
        (0..classes).map(|j| seen[j] && !in_a[j]).collect()
    };
    let (mean_a, var_a) = spread(obs, in_a);
    let (mean_b, var_b) = spread(obs, &in_b);
    let ok = |d: f64| {
        let below: f64 = obs.iter().filter(|o| o.0 <= d).map(|o| o.2).sum();
        let above: f64 = obs.iter().filter(|o| o.0 > d).map(|o| o.2).sum();
        below > 0.0 && above > 0.0
    };
    let midpoint = (mean_a + mean_b) / 2.0;
    // a variance of zero, judged against the spread of all the values
    let (_, all) = spread(obs, &vec![true; classes]);
    let zero = |v: f64| v <= 1e-24 * all.max(f64::MIN_POSITIVE);
    let candidate = if zero(var_a) || zero(var_b) {
        // the superclass of zero variance sits at its mean; the cut goes just beyond it, towards
        // the other mean
        let (mean1, mean2) = if zero(var_a) {
            (mean_a, mean_b)
        } else {
            (mean_b, mean_a)
        };
        let step = 1e-12 * if mean1 == 0.0 { 1.0 } else { mean1.abs() };
        Some(if mean1 < mean2 {
            mean1 + step
        } else {
            mean1 - step
        })
    } else {
        let weight = |side: &[bool]| {
            obs.iter()
                .filter(|o| side[o.1])
                .map(|o| o.2 * factor[o.1])
                .sum::<f64>()
        };
        let (pa, pb) = (weight(in_a), weight(&in_b));
        if pa <= 0.0 || pb <= 0.0 {
            None
        } else {
            let a = var_a - var_b;
            let b = 2.0 * (mean_a * var_b - mean_b * var_a);
            let c = mean_b * mean_b * var_a - mean_a * mean_a * var_b
                + 2.0 * var_a * var_b * ((pa * var_b.sqrt()) / (pb * var_a.sqrt())).ln();
            if a.abs() <= 1e-12 * var_a.max(var_b) {
                (b != 0.0).then(|| -c / b)
            } else {
                let disc = b * b - 4.0 * a * c;
                if disc < 0.0 {
                    None
                } else if disc == 0.0 {
                    Some(-b / (2.0 * a))
                } else {
                    let root = disc.sqrt();
                    let (r1, r2) = ((-b - root) / (2.0 * a), (-b + root) / (2.0 * a));
                    Some(if (r1 - mean_a).abs() <= (r2 - mean_a).abs() {
                        r1
                    } else {
                        r2
                    })
                }
            }
        }
    };
    candidate
        .filter(|&d| d.is_finite() && ok(d))
        .or(Some(midpoint).filter(|&d| ok(d)))
}

/// The most categories a nominal predictor may have under QUEST.
pub(super) const MAX_CATEGORIES: usize = 25;

/// The largest discriminant coordinate of each category of a nominal predictor, from the whole
/// learning sample: the scores that make the ratio of between-class to within-class sum of
/// squares largest. With the category indicators v, T their total scatter and B the scatter of
/// the class means, T = Q D Q' (its null direction, the sum of the indicators, dropped: a
/// direction whose d is under 1e-10 of the largest), the leading eigenvector a of
/// D^-1/2 Q' B Q D^-1/2 gives the score c = Q D^-1/2 a of each category. The scores sum to
/// zero; the sign is the one that makes the first category's score negative.
pub(super) fn coordinates(
    codes: &[usize],
    categories: usize,
    class: &[usize],
    weight: &[f64],
    classes: usize,
) -> Vec<f64> {
    let mut count = vec![vec![0.0; categories]; classes];
    for (i, &code) in codes.iter().enumerate() {
        if code < categories {
            count[class[i]][code] += weight[i];
        }
    }
    let total: Vec<f64> = (0..categories)
        .map(|c| count.iter().map(|row| row[c]).sum())
        .collect();
    let n: f64 = total.iter().sum();
    let mean: Vec<f64> = total.iter().map(|t| t / n).collect();
    let mut t: Vec<Vec<f64>> = (0..categories)
        .map(|a| {
            (0..categories)
                .map(|b| {
                    let own = if a == b { total[a] } else { 0.0 };
                    own - n * mean[a] * mean[b]
                })
                .collect()
        })
        .collect();
    let mut b = vec![vec![0.0; categories]; categories];
    for row in &count {
        let size: f64 = row.iter().sum();
        if size <= 0.0 {
            continue;
        }
        let diff: Vec<f64> = (0..categories).map(|c| row[c] / size - mean[c]).collect();
        for x in 0..categories {
            for y in 0..categories {
                b[x][y] += size * diff[x] * diff[y];
            }
        }
    }
    // symmetric up to rounding
    for x in 0..categories {
        for y in 0..x {
            let (tm, bm) = ((t[x][y] + t[y][x]) / 2.0, (b[x][y] + b[y][x]) / 2.0);
            (t[x][y], t[y][x], b[x][y], b[y][x]) = (tm, tm, bm, bm);
        }
    }
    let eigen = crate::matrix::symmetric_eigen(&t);
    let largest = eigen.first().map_or(0.0, |e| e.0);
    let kept: Vec<&(f64, Vec<f64>)> = eigen.iter().filter(|e| e.0 > 1e-10 * largest).collect();
    if kept.is_empty() {
        return vec![0.0; categories];
    }
    let apply = |q: &[f64]| -> Vec<f64> {
        (0..categories)
            .map(|x| (0..categories).map(|y| b[x][y] * q[y]).sum())
            .collect()
    };
    let projected: Vec<Vec<f64>> = kept.iter().map(|(_, q)| apply(q)).collect();
    let m: Vec<Vec<f64>> = (0..kept.len())
        .map(|i| {
            (0..kept.len())
                .map(|j| {
                    let dot: f64 = kept[i]
                        .1
                        .iter()
                        .zip(&projected[j])
                        .map(|(a, b)| a * b)
                        .sum();
                    dot / (kept[i].0 * kept[j].0).sqrt()
                })
                .collect()
        })
        .collect();
    let leading = crate::matrix::symmetric_eigen(&m);
    let a = &leading[0].1;
    let mut score = vec![0.0; categories];
    for (k, (d, q)) in kept.iter().enumerate() {
        for c in 0..categories {
            score[c] += q[c] * a[k] / d.sqrt();
        }
    }
    let first = (0..categories).find(|&c| total[c] > 0.0).unwrap_or(0);
    if score[first] > 0.0 {
        score.iter_mut().for_each(|x| *x = -*x);
    }
    score
}

/// What QUEST chose for a node: the predictor, the categories that go left and right, and the
/// test and cut behind it.
pub(super) type Chosen = (usize, Vec<usize>, Vec<usize>, QuestSplit);

/// The split of a node by QUEST at significance level `alpha`, None when it is not split.
pub(super) fn choose(g: &Growth, cases: &[usize], factor: &[f64], alpha: f64) -> Option<Chosen> {
    let predictors = g.predictors;
    let (m, m1) = (
        predictors.len(),
        predictors.iter().filter(|p| !p.nominal).count(),
    );
    // the cases with a value of each predictor, as (value, class, weight); a nominal one has the
    // category in place of the value
    let observed: Vec<Vec<Obs>> = predictors
        .iter()
        .map(|p| {
            cases
                .iter()
                .filter(|&&i| p.codes[i] != p.missing())
                .map(|&i| {
                    let code = p.codes[i];
                    let x = if p.nominal {
                        code as f64
                    } else {
                        p.numbers[code]
                    };
                    (x, g.case_class[i], g.case_weight[i])
                })
                .collect()
        })
        .collect();
    let first_below = |tests: Vec<(usize, Test)>, limit: f64| {
        let mut best: Option<(usize, Test)> = None;
        for (p, t) in tests {
            if best.as_ref().is_none_or(|(_, b)| t.sig < b.sig) {
                best = Some((p, t));
            }
        }
        best.filter(|(_, t)| t.sig < limit)
    };
    let main: Vec<(usize, Test)> = observed
        .iter()
        .enumerate()
        .filter_map(|(p, obs)| {
            let test = if predictors[p].nominal {
                let cells: Vec<(usize, usize, f64)> =
                    obs.iter().map(|&(x, j, w)| (x as usize, j, w)).collect();
                pearson(&cells, predictors[p].labels.len(), g.classes)
            } else {
                anova(obs, g.classes, "F")
            };
            test.map(|t| (p, t))
        })
        .collect();
    let (p, test) = first_below(main, alpha / m as f64).or_else(|| {
        let spread: Vec<(usize, Test)> = observed
            .iter()
            .enumerate()
            .filter(|(p, _)| !predictors[*p].nominal)
            .filter_map(|(p, obs)| levene(obs, g.classes).map(|t| (p, t)))
            .collect();
        first_below(spread, alpha / (m + m1) as f64)
    })?;
    let predictor = &predictors[p];
    let obs = &observed[p];
    let missing = predictor.missing();
    let (left, right, cut) = if predictor.nominal && predictor.numbers.is_empty() {
        // two categories: the first present goes left
        let first = obs.iter().map(|o| o.0 as usize).min()?;
        let left = vec![first];
        let right: Vec<usize> = (0..missing).filter(|&c| c != first).collect();
        (left, right, f64::NAN)
    } else if predictor.nominal {
        // the discriminant coordinate of each category stands in for the value; the categories at
        // or below the cut go left, and the side holding the first category present goes left
        let scores: Vec<Obs> = obs
            .iter()
            .map(|&(code, j, w)| (predictor.numbers[code as usize], j, w))
            .collect();
        let in_a = superclasses(&scores, g.classes)?;
        let d = split_point(&scores, &in_a, factor, g.classes)?;
        let first = obs.iter().map(|o| o.0 as usize).min()?;
        let below = |c: usize| (predictor.numbers[c] <= d) == (predictor.numbers[first] <= d);
        (
            (0..missing).filter(|&c| below(c)).collect(),
            (0..missing).filter(|&c| !below(c)).collect(),
            f64::NAN,
        )
    } else {
        let in_a = superclasses(obs, g.classes)?;
        let d = split_point(obs, &in_a, factor, g.classes)?;
        // the largest value present that goes left names the cut
        let last = (0..missing)
            .filter(|&c| predictor.numbers[c] <= d)
            .filter(|&c| obs.iter().any(|o| o.0 == predictor.numbers[c]))
            .max()?;
        ((0..=last).collect(), (last + 1..missing).collect(), d)
    };
    let side = |codes: &[usize]| -> f64 {
        let mut weight = 0.0;
        for &i in cases {
            let code = predictor.codes[i];
            if code != missing && codes.contains(&code) {
                weight += g.case_weight[i];
            }
        }
        weight
    };
    if side(&left) < g.min_child || side(&right) < g.min_child {
        return None;
    }
    Some((
        p,
        left,
        right,
        QuestSplit {
            test: test.name,
            statistic: test.statistic,
            df: test.df,
            df2: test.df2,
            sig: test.sig,
            cut,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(values: &[(f64, usize)]) -> Vec<Obs> {
        values.iter().map(|&(x, j)| (x, j, 1.0)).collect()
    }

    #[test]
    fn the_f_test_of_two_groups() {
        // means 2 and 5 about 3.5: between 13.5, within 4 on 4 df
        let data = obs(&[(1.0, 0), (2.0, 0), (3.0, 0), (4.0, 1), (5.0, 1), (6.0, 1)]);
        let t = anova(&data, 2, "F").expect("a test");
        assert!((t.statistic - 13.5).abs() < 1e-12);
        assert_eq!((t.df, t.df2), (1.0, Some(4.0)));
        assert!((t.sig - dist::f_p(13.5, 1.0, 4.0)).abs() < 1e-15);
        // no spread at all, and one class, test nothing
        assert!(anova(&obs(&[(1.0, 0), (1.0, 1)]), 2, "F").is_none());
        assert!(anova(&obs(&[(1.0, 0), (2.0, 0), (3.0, 0)]), 2, "F").is_none());
    }

    #[test]
    fn a_constant_superclass_below_zero_is_cut_just_above_its_mean() {
        // the printed rule would put the cut below -5 and send the constant class right
        let data = obs(&[
            (-5.0, 0),
            (-5.0, 0),
            (-5.0, 0),
            (-1.0, 1),
            (0.0, 1),
            (1.0, 1),
            (2.0, 1),
        ]);
        let d = split_point(&data, &[true, false], &[1.0, 1.0], 2).expect("a cut");
        assert!(d > -5.0 && d < -1.0, "{d}");
        // and one above the other class is cut just below its mean
        let data = obs(&[
            (5.0, 0),
            (5.0, 0),
            (5.0, 0),
            (-1.0, 1),
            (0.0, 1),
            (1.0, 1),
            (2.0, 1),
        ]);
        let d = split_point(&data, &[true, false], &[1.0, 1.0], 2).expect("a cut");
        assert!(d < 5.0 && d > 2.0, "{d}");
    }

    #[test]
    fn the_discriminant_scores_maximize_the_ratio_of_between_to_within() {
        // three categories over two classes, 8 cases
        let codes = [0, 0, 0, 1, 1, 2, 2, 2];
        let class = [0, 0, 1, 0, 1, 1, 1, 0];
        let weight = [1.0; 8];
        let score = coordinates(&codes, 3, &class, &weight, 2);
        assert!(score.iter().sum::<f64>().abs() < 1e-12);
        assert!(score[0] <= 0.0);
        let ratio = |c: &[f64]| {
            let x: Vec<f64> = codes.iter().map(|&k| c[k]).collect();
            let mean = x.iter().sum::<f64>() / 8.0;
            let (mut between, mut within) = (0.0, 0.0);
            for j in 0..2 {
                let members: Vec<f64> = (0..8).filter(|&i| class[i] == j).map(|i| x[i]).collect();
                let m = members.iter().sum::<f64>() / members.len() as f64;
                between += members.len() as f64 * (m - mean).powi(2);
                within += members.iter().map(|v| (v - m).powi(2)).sum::<f64>();
            }
            between / within
        };
        let best = ratio(&score);
        let mut seed = 12345_u64;
        for _ in 0..500 {
            let mut other = [0.0; 3];
            for v in &mut other {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                *v = (seed >> 33) as f64 / (1u64 << 31) as f64;
            }
            assert!(ratio(&other) <= best + 1e-9);
        }
    }

    #[test]
    fn equal_class_means_put_the_largest_class_against_the_rest() {
        let data = obs(&[
            (1.0, 0),
            (3.0, 0),
            (0.0, 1),
            (2.0, 1),
            (4.0, 1),
            (1.0, 1),
            (3.0, 1),
            (2.0, 2),
            (2.0, 2),
        ]);
        // class means 2, 2 and 2
        let in_a = superclasses(&data, 3).expect("superclasses");
        assert_eq!(in_a, [false, true, false]);
    }
}
