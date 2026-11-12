//! DISCRIMINANT: linear discriminant analysis.
//!
//! Rules from the Command Syntax Reference (DISCRIMINANT) and the Algorithms manual:
//! - The analysis uses the cases whose group value is a whole number in the range, that have
//!   no missing value on any variable of VARIABLES, a positive weight and, with SELECT, the
//!   selected value. Groups without cases are ignored. n is the sum of the weights.
//! - W and T are the within-groups and total sums of squares and cross-products. Tolerance,
//!   F-to-enter, F-to-remove, Wilks' lambda with Rao's F, Rao's V, Mahalanobis D squared and
//!   the F between groups and the sum of unexplained variation follow the formulas of the
//!   manual, computed from the inverse of W over the variables of a set.
//! - DIRECT enters the variables in the order of ANALYSIS while no tolerance falls to the
//!   limit. The stepwise methods enter, at each step, the eligible variable of the highest
//!   inclusion level with the best criterion (even levels all at once); once only level 1 is
//!   left, a variable of level 1 whose F-to-remove falls below FOUT (or whose significance
//!   rises above POUT) is removed first, the one that leaves the best criterion.
//! - Canonical functions solve (T - W) v = lambda W v through the Cholesky factor of W; V'WV
//!   = I. Standardized coefficients S V, structure R = S^-1 W V (zeros extend V over the
//!   variables not entered), unstandardized B = sqrt(n - g) V with constants -B'xbar.
//! - Classification: posterior P(G_j | X) from the chi-square distances of the function
//!   values to the group centroids and the priors (SEPARATE: the group covariance matrices of
//!   the functions); Fisher's classification functions; cross-validation leaves each case out
//!   of the means and the pooled covariance matrix.

use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, f_p};
use crate::matrix::{Matrix, inverse, symmetric_eigen};
use crate::means::is_valid;
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Measure, Values, Variable};
use statscraft_syntax::{
    ClassifyCases, Discriminant, DiscriminantAnalysis, DiscriminantMethod as M, Priors, VarItem,
};

/// The cases of the analysis phase.
struct Sample {
    /// Values of the variables of the block, by case.
    x: Vec<Vec<f64>>,
    w: Vec<f64>,
    /// Index of the group of each case.
    group: Vec<usize>,
}

/// Sums of the analysis: per group and total, over the variables of the block.
struct Sums {
    g: usize,
    n: f64,
    nj: Vec<f64>,
    count: Vec<usize>,
    mean: Vec<Vec<f64>>,
    total_mean: Vec<f64>,
    w: Matrix,
    t: Matrix,
    /// The SSCP of every group about its mean.
    group_sscp: Vec<Matrix>,
}

fn sums(s: &Sample, g: usize, p: usize) -> Sums {
    let mut nj = vec![0.0; g];
    let mut count = vec![0; g];
    let mut sum = vec![vec![0.0; p]; g];
    for ((x, w), &j) in s.x.iter().zip(&s.w).zip(&s.group) {
        nj[j] += w;
        count[j] += 1;
        for i in 0..p {
            sum[j][i] += w * x[i];
        }
    }
    let n: f64 = nj.iter().sum();
    let mean: Vec<Vec<f64>> = (0..g)
        .map(|j| sum[j].iter().map(|v| v / nj[j]).collect())
        .collect();
    let total_mean: Vec<f64> = (0..p)
        .map(|i| (0..g).map(|j| sum[j][i]).sum::<f64>() / n)
        .collect();
    let mut group_sscp = vec![vec![vec![0.0; p]; p]; g];
    let mut t = vec![vec![0.0; p]; p];
    for ((x, w), &j) in s.x.iter().zip(&s.w).zip(&s.group) {
        for a in 0..p {
            for b in 0..p {
                group_sscp[j][a][b] += w * (x[a] - mean[j][a]) * (x[b] - mean[j][b]);
                t[a][b] += w * (x[a] - total_mean[a]) * (x[b] - total_mean[b]);
            }
        }
    }
    let w = (0..p)
        .map(|a| {
            (0..p)
                .map(|b| (0..g).map(|j| group_sscp[j][a][b]).sum())
                .collect()
        })
        .collect();
    Sums {
        g,
        n,
        nj,
        count,
        mean,
        total_mean,
        w,
        t,
        group_sscp,
    }
}

fn sub(m: &Matrix, set: &[usize]) -> Matrix {
    set.iter()
        .map(|&a| set.iter().map(|&b| m[a][b]).collect())
        .collect()
}

/// The lower Cholesky factor; None when not positive definite.
fn cholesky(a: &Matrix) -> Option<Matrix> {
    let n = a.len();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let s: f64 = a[i][j] - (0..j).map(|k| l[i][k] * l[j][k]).sum::<f64>();
            if i == j {
                if s <= 1e-12 * a[i][i].abs().max(f64::MIN_POSITIVE) {
                    return None;
                }
                l[i][i] = s.sqrt();
            } else {
                l[i][j] = s / l[j][j];
            }
        }
    }
    Some(l)
}

/// The log determinant and rank of a symmetric matrix by Cholesky, None when singular.
fn log_det(a: &Matrix) -> Option<f64> {
    let l = cholesky(a)?;
    Some(2.0 * (0..a.len()).map(|i| l[i][i].ln()).sum::<f64>())
}

/// The statistics of a set of variables that the selection criteria use.
#[derive(Clone, Copy, Debug)]
struct Criteria {
    lambda: f64,
    min_d2: f64,
    min_f: f64,
    resid: f64,
    rao_v: f64,
}

/// Rao's approximate F of Wilks' lambda with its degrees of freedom.
fn rao_f(lambda: f64, q: f64, g: f64, n: f64) -> (f64, f64, f64) {
    let h = g - 1.0;
    let s = match q * q + h * h == 5.0 {
        true => 1.0,
        false => ((q * q + h * h - 5.0) / (q * q * h * h - 4.0)).sqrt(),
    };
    let r = n - 1.0 - (q + g) / 2.0;
    let ls = lambda.powf(s);
    let df2 = r / s + 1.0 - q * h / 2.0;
    ((1.0 - ls) * df2 / (ls * q * h), q * h, df2)
}

struct Analysis<'a> {
    sums: &'a Sums,
    method: M,
}

impl Analysis<'_> {
    fn n_g(&self) -> f64 {
        self.sums.n - self.sums.g as f64
    }

    /// The tolerances of the variables of the set, None when W of the set is singular.
    fn tolerances(&self, set: &[usize]) -> Option<Vec<f64>> {
        if set.iter().any(|&i| self.sums.w[i][i] <= 0.0) {
            return None;
        }
        let (inv, _) = inverse(&sub(&self.sums.w, set))?;
        Some(
            set.iter()
                .enumerate()
                .map(|(k, &i)| 1.0 / (self.sums.w[i][i] * inv[k][k]))
                .collect(),
        )
    }

    /// The pairwise D squared and F of the groups over the set.
    fn pairs(&self, set: &[usize]) -> Vec<(usize, usize, f64, f64)> {
        let s = self.sums;
        let Some((inv, _)) = inverse(&sub(&s.w, set)) else {
            return Vec::new();
        };
        let q = set.len() as f64;
        let mut out = Vec::new();
        for a in 0..s.g {
            for b in a + 1..s.g {
                let d: Vec<f64> = set.iter().map(|&i| s.mean[a][i] - s.mean[b][i]).collect();
                let quad: f64 = (0..d.len())
                    .map(|i| (0..d.len()).map(|l| inv[i][l] * d[i] * d[l]).sum::<f64>())
                    .sum();
                let d2 = self.n_g() * quad;
                let (na, nb) = (s.nj[a], s.nj[b]);
                let f = (s.n - q - s.g as f64 + 1.0) * na * nb / (q * self.n_g() * (na + nb)) * d2;
                out.push((a, b, d2, f));
            }
        }
        out
    }

    fn criteria(&self, set: &[usize]) -> Option<Criteria> {
        let s = self.sums;
        let (wd, td) = (log_det(&sub(&s.w, set))?, log_det(&sub(&s.t, set))?);
        let (inv, _) = inverse(&sub(&s.w, set))?;
        let mut v = 0.0;
        for (a, &i) in set.iter().enumerate() {
            for (b, &l) in set.iter().enumerate() {
                v += inv[a][b] * (s.t[i][l] - s.w[i][l]);
            }
        }
        let pairs = self.pairs(set);
        Some(Criteria {
            lambda: (wd - td).exp(),
            min_d2: pairs.iter().map(|p| p.2).fold(f64::INFINITY, f64::min),
            min_f: pairs.iter().map(|p| p.3).fold(f64::INFINITY, f64::min),
            resid: pairs.iter().map(|p| 4.0 / (4.0 + p.2)).sum(),
            rao_v: self.n_g() * v,
        })
    }

    fn value(&self, c: &Criteria) -> f64 {
        match self.method {
            M::Direct | M::Wilks => c.lambda,
            M::Mahal => c.min_d2,
            M::MaxMinF => c.min_f,
            M::MinResid => c.resid,
            M::Rao => c.rao_v,
        }
    }

    /// True when a is a better value of the criterion than b.
    fn better(&self, a: f64, b: f64) -> bool {
        match self.method {
            M::Direct | M::Wilks | M::MinResid => a < b,
            _ => a > b,
        }
    }

    /// The partial F of variable i given the `others`.
    fn partial_f(&self, i: usize, others: &[usize]) -> f64 {
        let s = self.sums;
        let conditional = |m: &Matrix| -> f64 {
            if others.is_empty() {
                return m[i][i];
            }
            let Some((inv, _)) = inverse(&sub(m, others)) else {
                return f64::NAN;
            };
            let c: Vec<f64> = others.iter().map(|&o| m[i][o]).collect();
            m[i][i]
                - (0..c.len())
                    .map(|a| (0..c.len()).map(|b| c[a] * inv[a][b] * c[b]).sum::<f64>())
                    .sum::<f64>()
        };
        let (w, t) = (conditional(&s.w), conditional(&s.t));
        let q = others.len() as f64;
        (t - w) * (s.n - q - s.g as f64) / (w * (s.g as f64 - 1.0))
    }

    fn f_df(&self, others: usize) -> (f64, f64) {
        (
            self.sums.g as f64 - 1.0,
            self.sums.n - others as f64 - self.sums.g as f64,
        )
    }
}

fn statistic_name(method: M) -> &'static str {
    match method {
        M::Direct | M::Wilks => "Wilks' Lambda",
        M::Mahal => "Min. D Squared",
        M::MaxMinF => "Min. F",
        M::MinResid => "Sum of Unexplained Variation",
        M::Rao => "Rao's V",
    }
}

fn method_text(method: M) -> &'static str {
    match method {
        M::Direct => "",
        M::Wilks => {
            "Each step adds the variable that gives the lowest Wilks' Lambda for the model."
        }
        M::Mahal => {
            "Each step adds the variable that pushes the nearest pair of groups furthest apart in Mahalanobis distance."
        }
        M::MaxMinF => {
            "Each step adds the variable that gives the highest value to the lowest pairwise group F."
        }
        M::MinResid => {
            "Each step adds the variable that leaves the least unexplained variation, summed over every pair of groups."
        }
        M::Rao => {
            "Each step adds the variable that raises Rao's V the most."
        }
    }
}

/// One step of a stepwise selection.
struct Step {
    entered: Vec<usize>,
    removed: Option<usize>,
    set: Vec<usize>,
}

/// The variables a step may enter: eligible by tolerance and, for odd levels, by the entry
/// criteria. Each with its level.
fn eligible(
    a: &Analysis,
    options: &DiscriminantAnalysis,
    levels: &[u32],
    set: &[usize],
) -> Vec<usize> {
    let current_v = match set.is_empty() {
        true => 0.0,
        false => a.criteria(set).map_or(0.0, |c| c.rao_v),
    };
    (0..levels.len())
        .filter(|i| !set.contains(i) && levels[*i] > 0)
        .filter(|&i| {
            let mut with = set.to_vec();
            with.push(i);
            let Some(tol) = a.tolerances(&with) else {
                return false;
            };
            if tol.iter().any(|t| *t <= options.tolerance) {
                return false;
            }
            if levels[i].is_multiple_of(2) {
                return true;
            }
            let f = a.partial_f(i, set);
            let (d1, d2) = a.f_df(set.len());
            let enters = match options.pin {
                Some(pin) => f_p(f, d1, d2) <= pin,
                None => f >= options.fin,
            };
            let rao = match a.method {
                M::Rao => a
                    .criteria(&with)
                    .is_some_and(|c| c.rao_v - current_v >= options.vin),
                _ => true,
            };
            enters && rao
        })
        .collect()
}

fn stepwise(a: &Analysis, options: &DiscriminantAnalysis, levels: &[u32]) -> Vec<Step> {
    let max_steps = options.max_steps.unwrap_or_else(|| {
        levels.iter().filter(|l| **l > 1).count() + 2 * levels.iter().filter(|l| **l == 1).count()
    });
    let mut set: Vec<usize> = Vec::new();
    let mut steps = Vec::new();
    let best_of = |candidates: &[usize], set_of: &dyn Fn(usize) -> Vec<usize>| -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        for &i in candidates {
            let Some(c) = a.criteria(&set_of(i)) else {
                continue;
            };
            let v = a.value(&c);
            if best.is_none_or(|(_, b)| a.better(v, b)) {
                best = Some((i, v));
            }
        }
        best.map(|(i, _)| i)
    };
    while steps.len() < max_steps {
        let candidates = eligible(a, options, levels, &set);
        let top = candidates.iter().map(|&i| levels[i]).max();
        // removal once only level 1 is left to enter
        if top.is_none_or(|l| l <= 1) && set.len() > 1 {
            let removable: Vec<usize> = set
                .iter()
                .copied()
                .filter(|&i| levels[i] == 1)
                .filter(|&i| {
                    let others: Vec<usize> = set.iter().copied().filter(|&o| o != i).collect();
                    let f = a.partial_f(i, &others);
                    let (d1, d2) = a.f_df(others.len());
                    match options.pout {
                        Some(pout) => f_p(f, d1, d2) > pout,
                        None => f < options.fout,
                    }
                })
                .collect();
            let without = |i: usize| set.iter().copied().filter(|&o| o != i).collect::<Vec<_>>();
            if let Some(i) = best_of(&removable, &without) {
                set.retain(|&o| o != i);
                steps.push(Step {
                    entered: Vec::new(),
                    removed: Some(i),
                    set: set.clone(),
                });
                continue;
            }
        } else if top.is_none_or(|l| l <= 1) && set.len() == 1 && levels[set[0]] == 1 {
            // a lone variable is removed when its F falls below FOUT
            let i = set[0];
            let f = a.partial_f(i, &[]);
            let (d1, d2) = a.f_df(0);
            let out = match options.pout {
                Some(pout) => f_p(f, d1, d2) > pout,
                None => f < options.fout,
            };
            if out {
                set.clear();
                steps.push(Step {
                    entered: Vec::new(),
                    removed: Some(i),
                    set: Vec::new(),
                });
                continue;
            }
        }
        let Some(level) = top else { break };
        let at_level: Vec<usize> = candidates
            .iter()
            .copied()
            .filter(|&i| levels[i] == level)
            .collect();
        let entered = if level % 2 == 0 {
            // the whole level, in order, each still passing the tolerance
            let mut entered = Vec::new();
            for i in at_level {
                let mut with = set.clone();
                with.push(i);
                if a.tolerances(&with)
                    .is_some_and(|t| t.iter().all(|t| *t > options.tolerance))
                {
                    set.push(i);
                    entered.push(i);
                }
            }
            entered
        } else {
            let with = |i: usize| {
                let mut s = set.clone();
                s.push(i);
                s
            };
            let Some(i) = best_of(&at_level, &with) else {
                break;
            };
            set.push(i);
            vec![i]
        };
        if entered.is_empty() {
            break;
        }
        steps.push(Step {
            entered,
            removed: None,
            set: set.clone(),
        });
    }
    steps
}

/// The order of the structure matrix: by the function of the largest absolute correlation,
/// within it by the size of that correlation.
fn structure_order(r: &[Vec<f64>]) -> Vec<usize> {
    let top = |row: &Vec<f64>| {
        (0..row.len())
            .max_by(|&a, &b| row[a].abs().total_cmp(&row[b].abs()).then(b.cmp(&a)))
            .unwrap_or(0)
    };
    let mut order: Vec<usize> = (0..r.len()).collect();
    order.sort_by(|&a, &b| {
        let (fa, fb) = (top(&r[a]), top(&r[b]));
        fa.cmp(&fb).then(r[b][fb].abs().total_cmp(&r[a][fa].abs()))
    });
    order
}

fn function_columns(k: usize) -> Vec<(String, String)> {
    let mut columns = plain(&[""]);
    columns.extend((1..=k).map(|f| ("Function".to_string(), f.to_string())));
    columns
}

/// A classified case: its group (None when ungrouped), the predicted group and its weight.
type Tally = (Option<usize>, usize, f64);

/// Classification of one case: the function values, the posterior of each group and the
/// predicted group.
struct Classified {
    scores: Vec<f64>,
    posterior: Vec<f64>,
    predicted: usize,
}

impl Engine {
    pub(crate) fn discriminant(&mut self, command: &Discriminant) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let names = self.expand(&command.variables)?;
        let variables: Vec<Variable> = names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = variables.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "DISCRIMINANT needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let group_var = data.variable(&command.group)?;
        if group_var.is_string() {
            return Err(Error(format!(
                "DISCRIMINANT: the grouping variable {} must be numeric; AUTORECODE turns strings into numbers",
                command.group
            )));
        }
        let select_var = match &command.select {
            Some((name, _)) => Some(data.variable(name)?),
            None => None,
        };
        let cases = data.case_count();
        let weights = case_weights(data)?;
        let weight = |c: usize| weights.as_ref().map_or(1.0, |w| w[c]);
        let numeric = |var: &Variable, case: usize| match var.values() {
            Values::Numeric(v) => v.get(case),
            Values::String(_) => None,
        };
        // the group value of each case when it is valid and in range
        let group_of: Vec<Option<f64>> = (0..cases)
            .map(|c| {
                numeric(&group_var, c).filter(|v| {
                    is_valid(&group_var, c, command.include)
                        && v.fract() == 0.0
                        && *v >= command.min
                        && *v <= command.max
                })
            })
            .collect();
        let values: Vec<Vec<Option<f64>>> = (0..cases)
            .map(|c| {
                variables
                    .iter()
                    .map(|v| numeric(v, c).filter(|_| is_valid(v, c, command.include)))
                    .collect()
            })
            .collect();
        let selected: Vec<bool> = (0..cases)
            .map(|c| match (&select_var, &command.select) {
                (Some(var), Some((_, value))) => numeric(var, c) == Some(*value),
                _ => true,
            })
            .collect();
        let complete: Vec<bool> = values
            .iter()
            .map(|r| r.iter().all(Option::is_some))
            .collect();
        let used: Vec<usize> = (0..cases)
            .filter(|&c| selected[c] && group_of[c].is_some() && complete[c] && weight(c) > 0.0)
            .collect();
        let mut group_values: Vec<f64> = used.iter().filter_map(|&c| group_of[c]).collect();
        group_values.sort_by(f64::total_cmp);
        group_values.dedup();
        let g = group_values.len();
        let shown = self.shown();
        let group_labels: Vec<String> = group_values
            .iter()
            .map(|v| shown.number(&group_var, *v))
            .collect();
        let group_name = shown.variable(&group_var);
        let mut tables = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        let class_format = group_var.format().to_string();
        let class_labels = group_var.value_labels().to_vec();

        // Analysis Case Processing Summary
        {
            let in_select: Vec<usize> = (0..cases).filter(|&c| selected[c]).collect();
            let count = |f: &dyn Fn(usize) -> bool| in_select.iter().filter(|&&c| f(c)).count();
            let valid = used.len();
            let bad_group = count(&|c| group_of[c].is_none() && complete[c]);
            let bad_var = count(&|c| group_of[c].is_some() && !complete[c]);
            let both = count(&|c| group_of[c].is_none() && !complete[c]);
            let zero = count(&|c| group_of[c].is_some() && complete[c] && weight(c) <= 0.0);
            let unselected = cases - in_select.len();
            let total = cases as f64;
            let pct = |k: usize| Cell::Number(100.0 * k as f64 / total);
            let mut rows = vec![vec![
                text("Valid"),
                Cell::Empty,
                Cell::Number(valid as f64),
                pct(valid),
            ]];
            let mut excluded: Vec<(&str, usize)> = vec![
                ("Missing or out-of-range group codes", bad_group),
                ("At least one missing discriminating variable", bad_var),
                (
                    "Both missing or out-of-range group codes and at least one missing discriminating variable",
                    both,
                ),
            ];
            if zero > 0 {
                excluded.push(("Zero or missing weight", zero));
            }
            if command.select.is_some() {
                excluded.push(("Unselected", unselected));
            }
            let total_excluded: usize = excluded.iter().map(|e| e.1).sum();
            excluded.push(("Total", total_excluded));
            for (k, (what, n)) in excluded.into_iter().enumerate() {
                rows.push(vec![
                    if k == 0 {
                        text("Excluded")
                    } else {
                        Cell::Empty
                    },
                    text(what),
                    Cell::Number(n as f64),
                    pct(n),
                ]);
            }
            rows.push(vec![
                text("Total"),
                Cell::Empty,
                Cell::Number(total),
                pct(cases),
            ]);
            tables.push(table(
                "Analysis Case Processing Summary",
                2,
                plain(&["Unweighted Cases", "", "N", "Percent"]),
                rows,
                None,
            ));
        }
        if g < 2 {
            return Err(Error(format!(
                "DISCRIMINANT needs at least two groups with cases, but there are {g}"
            )));
        }
        let index_of = |v: f64| group_values.iter().position(|x| *x == v).expect("a group");

        if command.plot {
            warnings.push("DISCRIMINANT: PLOT is not produced yet (no charts, territorial map or casewise listing)".into());
        }
        let mut save_count = 0;
        let mut saves: Vec<(String, String, Vec<Option<f64>>, bool)> = Vec::new();
        for (number_of, options) in command.analyses.iter().enumerate() {
            let analysis_number = number_of + 1;
            // the variables of the block with their levels, in the order of ANALYSIS
            let mut block: Vec<(usize, u32)> = Vec::new();
            match &options.variables {
                None => block.extend((0..names.len()).map(|i| (i, 1))),
                Some(lists) => {
                    for (items, level) in lists {
                        for i in positions(&names, items)? {
                            match block.iter_mut().find(|(b, _)| *b == i) {
                                // a variable named twice: the later level counts
                                Some(entry) => entry.1 = *level,
                                None => block.push((i, *level)),
                            }
                        }
                    }
                }
            }
            let p = block.len();
            let sample = Sample {
                x: used
                    .iter()
                    .map(|&c| {
                        block
                            .iter()
                            .map(|&(i, _)| values[c][i].expect("complete"))
                            .collect()
                    })
                    .collect(),
                w: used.iter().map(|&c| weight(c)).collect(),
                group: used
                    .iter()
                    .map(|&c| index_of(group_of[c].expect("in range")))
                    .collect(),
            };
            let s = sums(&sample, g, p);
            let block_names: Vec<String> = block
                .iter()
                .map(|&(i, _)| shown.variable(&variables[i]))
                .collect();
            if s.n <= g as f64 {
                return Err(Error(format!(
                    "DISCRIMINANT: the {} weighted cases do not exceed the {g} groups",
                    s.n
                )));
            }
            let n_g = s.n - g as f64;
            let a = Analysis {
                sums: &s,
                method: options.method,
            };
            self.discriminant_descriptives(command, &s, &block_names, &group_labels, &mut tables);

            // selection
            let levels: Vec<u32> = block.iter().map(|b| b.1).collect();
            let set: Vec<usize> = if options.method == M::Direct {
                let mut set = Vec::new();
                let mut failing = Vec::new();
                for i in 0..p {
                    let mut with = set.clone();
                    with.push(i);
                    match a.tolerances(&with) {
                        Some(t) if t.iter().all(|t| *t > options.tolerance) => set.push(i),
                        t => failing.push((i, t)),
                    }
                }
                if !failing.is_empty() {
                    let rows = failing
                        .iter()
                        .map(|(i, t)| {
                            let (own, least) = match t {
                                Some(t) => (
                                    t.last().copied(),
                                    Some(t.iter().copied().fold(f64::INFINITY, f64::min)),
                                ),
                                None => (Some(0.0), Some(0.0)),
                            };
                            vec![
                                text(block_names[*i].clone()),
                                number(Some(s.w[*i][*i] / n_g)),
                                number(own),
                                number(least),
                            ]
                        })
                        .collect();
                    tables.push(table(
                        "Variables Failing Tolerance Test",
                        1,
                        plain(&["", "Within-Groups Variance", "Tolerance", "Minimum Tolerance"]),
                        rows,
                        Some(format!(
                            "Every variable that meets the tolerance limits goes into the model in one step. Minimum tolerance level is {}.",
                            options.tolerance
                        )),
                    ));
                }
                set
            } else {
                let steps = stepwise(&a, options, &levels);
                if command.history {
                    self.stepwise_tables(&a, options, &levels, &steps, &block_names, &mut tables);
                }
                let set = steps.last().map_or(Vec::new(), |s| s.set.clone());
                if command.statistics.fpair && !set.is_empty() {
                    let pairs = a.pairs(&set);
                    let q = set.len() as f64;
                    let (d1, d2) = (q, s.n - q - g as f64 + 1.0);
                    let mut rows = Vec::new();
                    for (x, name) in group_labels.iter().enumerate() {
                        for (k, what) in ["F", "Sig."].iter().enumerate() {
                            let mut row = vec![
                                if k == 0 {
                                    text(name.clone())
                                } else {
                                    Cell::Empty
                                },
                                text(*what),
                            ];
                            for y in 0..g {
                                let pair = pairs
                                    .iter()
                                    .find(|p| (p.0, p.1) == (x.min(y), x.max(y)) && x != y);
                                row.push(match pair {
                                    Some(p) if k == 0 => number(Some(p.3)),
                                    Some(p) => number(Some(f_p(p.3, d1, d2))),
                                    None => Cell::Empty,
                                });
                            }
                            rows.push(row);
                        }
                    }
                    let mut columns = plain(&["", ""]);
                    columns.extend(group_labels.iter().map(|l| (group_name.clone(), l.clone())));
                    tables.push(table(
                        "Pairwise Group Comparisons",
                        2,
                        columns,
                        rows,
                        Some(format!(
                            "F with {d1} and {d2} degrees of freedom, at the last step."
                        )),
                    ));
                }
                set
            };
            if set.is_empty() {
                warnings.push(format!(
                    "DISCRIMINANT: analysis {analysis_number} entered no variable, so no functions are computed"
                ));
                continue;
            }
            let q = set.len();
            // canonical functions
            let w11 = sub(&s.w, &set);
            let t11 = sub(&s.t, &set);
            let Some(l) = cholesky(&w11) else {
                return Err(Error(
                    "DISCRIMINANT: the within-groups matrix is singular".into(),
                ));
            };
            let (linv, _) = inverse(&l).expect("a Cholesky factor is regular");
            let between: Matrix = (0..q)
                .map(|i| (0..q).map(|j| t11[i][j] - w11[i][j]).collect())
                .collect();
            let m1 = crate::matrix::multiply(&linv, &between);
            let sym = crate::matrix::multiply(&m1, &crate::matrix::transpose(&linv));
            let m = q.min(g - 1);
            let eigen: Vec<(f64, Vec<f64>)> = symmetric_eigen(&sym)
                .into_iter()
                .take(m)
                .filter(|(v, _)| *v > 1e-12)
                .collect();
            let m = eigen.len();
            if m == 0 {
                return Err(Error(
                    "DISCRIMINANT: the group means do not differ, so there is no function".into(),
                ));
            }
            // V = L'^-1 U, reflected so that the standardized coefficients sum to a positive
            let linv_t = crate::matrix::transpose(&linv);
            let mut v: Vec<Vec<f64>> = eigen
                .iter()
                .map(|(_, u)| {
                    (0..q)
                        .map(|i| (0..q).map(|k| linv_t[i][k] * u[k]).sum())
                        .collect()
                })
                .collect();
            for col in v.iter_mut() {
                let sum: f64 = col
                    .iter()
                    .zip(&set)
                    .map(|(c, &i)| c * s.w[i][i].sqrt())
                    .sum();
                if sum < 0.0 {
                    col.iter_mut().for_each(|c| *c = -*c);
                }
            }
            let lambdas: Vec<f64> = eigen.iter().map(|e| e.0).collect();
            let total: f64 = lambdas.iter().sum();
            let wilks_k = |k: usize| {
                lambdas[k..]
                    .iter()
                    .map(|l| 1.0 / (1.0 + l))
                    .product::<f64>()
            };
            let chi_factor = s.n - (q + g) as f64 / 2.0 - 1.0;
            let wilks_sig = |k: usize| {
                let df = ((q - k) * (g - k - 1)) as f64;
                chi_square_p(-chi_factor * wilks_k(k).ln(), df)
            };
            // FUNCTIONS: the largest number, the cumulative percentage, the significance
            let (n1, n2, n3) = options.functions;
            let mut used_functions = 0;
            let mut cumulative = 0.0;
            for (k, lambda) in lambdas.iter().enumerate() {
                if n1.is_some_and(|n| k >= n) || cumulative >= n2 || wilks_sig(k) > n3 {
                    break;
                }
                cumulative += 100.0 * lambda / total;
                used_functions += 1;
            }
            let kf = used_functions.max(1);
            {
                let mut cum = 0.0;
                let rows = (0..m)
                    .map(|k| {
                        let pct = 100.0 * lambdas[k] / total;
                        cum += pct;
                        vec![
                            Cell::Number((k + 1) as f64),
                            number(Some(lambdas[k])),
                            number(Some(pct)),
                            number(Some(cum)),
                            number(Some((lambdas[k] / (1.0 + lambdas[k])).sqrt())),
                        ]
                    })
                    .collect();
                tables.push(table(
                    "Eigenvalues",
                    1,
                    plain(&[
                        "Function",
                        "Eigenvalue",
                        "% of Variance",
                        "Cumulative %",
                        "Canonical Correlation",
                    ]),
                    rows,
                    Some(format!(
                        "First {kf} canonical discriminant functions were used in the analysis."
                    )),
                ));
                let rows = (0..m)
                    .map(|k| {
                        let name = match k + 1 == m {
                            true => m.to_string(),
                            false => format!("{} through {m}", k + 1),
                        };
                        let lambda = wilks_k(k);
                        vec![
                            text(name),
                            number(Some(lambda)),
                            number(Some(-chi_factor * lambda.ln())),
                            Cell::Number(((q - k) * (g - k - 1)) as f64),
                            number(Some(wilks_sig(k))),
                        ]
                    })
                    .collect();
                tables.push(table(
                    "Wilks' Lambda",
                    1,
                    plain(&[
                        "Test of Function(s)",
                        "Wilks' Lambda",
                        "Chi-square",
                        "df",
                        "Sig.",
                    ]),
                    rows,
                    None,
                ));
            }
            let v = &v[..kf];
            let set_names: Vec<String> = set.iter().map(|&i| block_names[i].clone()).collect();
            // standardized coefficients
            let rows = set
                .iter()
                .enumerate()
                .map(|(a_, &i)| {
                    let mut row = vec![text(block_names[i].clone())];
                    row.extend(v.iter().map(|col| number(Some(col[a_] * s.w[i][i].sqrt()))));
                    row
                })
                .collect();
            tables.push(table(
                "Standardized Canonical Discriminant Function Coefficients",
                1,
                function_columns(kf),
                rows,
                None,
            ));
            // structure matrix over all variables of the block with variance
            let shown: Vec<usize> = (0..p).filter(|&i| s.w[i][i] > 0.0).collect();
            let r: Vec<Vec<f64>> = shown
                .iter()
                .map(|&i| {
                    v.iter()
                        .map(|col| {
                            set.iter()
                                .enumerate()
                                .map(|(a_, &l)| s.w[i][l] * col[a_])
                                .sum::<f64>()
                                / s.w[i][i].sqrt()
                        })
                        .collect()
                })
                .collect();
            let order = structure_order(&r);
            let not_used: Vec<String> = shown
                .iter()
                .filter(|i| !set.contains(i))
                .map(|&i| block_names[i].clone())
                .collect();
            let rows = order
                .iter()
                .map(|&k| {
                    let mut row = vec![text(block_names[shown[k]].clone())];
                    row.extend(r[k].iter().map(|x| number(Some(*x))));
                    row
                })
                .collect();
            let mut caption = "Pooled within-groups correlations between discriminating variables and standardized canonical discriminant functions. Variables ordered by absolute size of correlation within function.".to_string();
            if !not_used.is_empty() {
                caption.push_str(&format!(
                    " Not used in the analysis: {}.",
                    not_used.join(", ")
                ));
            }
            tables.push(table(
                "Structure Matrix",
                1,
                function_columns(kf),
                rows,
                Some(caption),
            ));
            // unstandardized coefficients and the centroids
            let scale = n_g.sqrt();
            let b: Vec<Vec<f64>> = v
                .iter()
                .map(|col| col.iter().map(|c| c * scale).collect())
                .collect();
            let constant: Vec<f64> = b
                .iter()
                .map(|col| {
                    -set.iter()
                        .enumerate()
                        .map(|(a_, &i)| col[a_] * s.total_mean[i])
                        .sum::<f64>()
                })
                .collect();
            if command.statistics.raw {
                let mut rows: Vec<Vec<Cell>> = set
                    .iter()
                    .enumerate()
                    .map(|(a_, &i)| {
                        let mut row = vec![text(block_names[i].clone())];
                        row.extend(b.iter().map(|col| number(Some(col[a_]))));
                        row
                    })
                    .collect();
                let mut last = vec![text("(Constant)")];
                last.extend(constant.iter().map(|c| number(Some(*c))));
                rows.push(last);
                tables.push(table(
                    "Canonical Discriminant Function Coefficients",
                    1,
                    function_columns(kf),
                    rows,
                    Some("Unstandardized coefficients".into()),
                ));
            }
            let score = |x: &[f64]| -> Vec<f64> {
                (0..kf)
                    .map(|k| {
                        constant[k]
                            + set
                                .iter()
                                .enumerate()
                                .map(|(a_, &i)| b[k][a_] * x[i])
                                .sum::<f64>()
                    })
                    .collect()
            };
            let centroids: Vec<Vec<f64>> = (0..g).map(|j| score(&s.mean[j])).collect();
            let rows = (0..g)
                .map(|j| {
                    let mut row = vec![text(group_labels[j].clone())];
                    row.extend(centroids[j].iter().map(|c| number(Some(*c))));
                    row
                })
                .collect();
            let mut columns = plain(&[&group_name]);
            columns.extend((1..=kf).map(|f| ("Function".to_string(), f.to_string())));
            tables.push(table(
                "Functions at Group Centroids",
                1,
                columns,
                rows,
                Some(
                    "Unstandardized canonical discriminant functions evaluated at group means"
                        .into(),
                ),
            ));

            // priors
            let priors: Vec<f64> = match &options.priors {
                Priors::Equal => vec![1.0 / g as f64; g],
                Priors::Size => s.nj.iter().map(|n| n / s.n).collect(),
                Priors::Values(given) => {
                    let range = (command.max - command.min) as usize + 1;
                    let mut values = if given.len() == g {
                        given.clone()
                    } else if given.len() == range {
                        group_values
                            .iter()
                            .map(|v| given[(*v - command.min) as usize])
                            .collect()
                    } else {
                        return Err(Error(format!(
                            "DISCRIMINANT /PRIORS needs {g} probabilities, one per group with cases, got {}",
                            given.len()
                        )));
                    };
                    let sum: f64 = values.iter().sum();
                    if (sum - 1.0).abs() > 1e-9 {
                        warnings.push(format!(
                            "DISCRIMINANT: the prior probabilities sum to {sum}, so they are rescaled to sum to 1"
                        ));
                        values.iter_mut().for_each(|v| *v /= sum);
                    }
                    values
                }
            };
            // group covariance matrices of the functions, for SEPARATE
            let separate: Option<Vec<(Matrix, f64)>> = match command.separate {
                false => None,
                true => {
                    let mut out = Vec::new();
                    for (j, sscp) in s.group_sscp.iter().enumerate() {
                        let c = sub(sscp, &set);
                        let dj: Matrix = (0..kf)
                            .map(|x| {
                                (0..kf)
                                    .map(|y| {
                                        (0..q)
                                            .map(|i| {
                                                (0..q)
                                                    .map(|l| b[x][i] * c[i][l] * b[y][l])
                                                    .sum::<f64>()
                                            })
                                            .sum::<f64>()
                                            / (s.nj[j] - 1.0)
                                    })
                                    .collect()
                            })
                            .collect();
                        let Some((inv, det)) = inverse(&dj).filter(|(_, d)| *d > 0.0) else {
                            return Err(Error(format!(
                                "DISCRIMINANT /CLASSIFY=SEPARATE: the functions have a singular covariance matrix in group {}",
                                group_labels[j]
                            )));
                        };
                        out.push((inv, det));
                    }
                    Some(out)
                }
            };
            let classify = |x: &[f64]| -> Classified {
                let f = score(x);
                let logs: Vec<f64> = (0..g)
                    .map(|j| {
                        let d: Vec<f64> = (0..kf).map(|k| f[k] - centroids[j][k]).collect();
                        let (chi, logdet) = match &separate {
                            None => (d.iter().map(|v| v * v).sum::<f64>(), 0.0),
                            Some(groups) => {
                                let (inv, det) = &groups[j];
                                let chi = (0..kf)
                                    .map(|a_| {
                                        (0..kf).map(|b_| d[a_] * inv[a_][b_] * d[b_]).sum::<f64>()
                                    })
                                    .sum::<f64>();
                                (chi, det.ln())
                            }
                        };
                        priors[j].ln() - 0.5 * (logdet + chi)
                    })
                    .collect();
                let top = logs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let e: Vec<f64> = logs
                    .iter()
                    .map(|l| {
                        if l - top > -46.0 {
                            (l - top).exp()
                        } else {
                            0.0
                        }
                    })
                    .collect();
                let sum: f64 = e.iter().sum();
                let posterior: Vec<f64> = e.iter().map(|v| v / sum).collect();
                let predicted = (0..g)
                    .max_by(|&x, &y| posterior[x].total_cmp(&posterior[y]).then(y.cmp(&x)))
                    .unwrap_or(0);
                Classified {
                    scores: f,
                    posterior,
                    predicted,
                }
            };
            // the cases to classify, with their values (means in place of missing ones with
            // MEANSUB)
            let classified: Vec<(usize, Classified)> = (0..cases)
                .filter(|&c| match command.classify {
                    ClassifyCases::NonMissing => true,
                    ClassifyCases::Unselected => !selected[c],
                    ClassifyCases::Unclassified => group_of[c].is_none(),
                })
                .filter_map(|c| {
                    let x: Option<Vec<f64>> = block
                        .iter()
                        .enumerate()
                        .map(|(k, &(i, _))| match values[c][i] {
                            Some(v) => Some(v),
                            None if command.meansub || !set.contains(&k) => Some(s.total_mean[k]),
                            None => None,
                        })
                        .collect();
                    x.map(|x| (c, classify(&x)))
                })
                .collect();
            let classification_output = command.statistics.table
                || command.statistics.crossvalid
                || options.save.is_some()
                || command.plot;
            if classification_output {
                let processed = cases;
                let missing_group = (0..cases).filter(|&c| group_of[c].is_none()).count();
                let missing_var = cases - classified.len();
                tables.push(table(
                    "Classification Processing Summary",
                    2,
                    plain(&["", "", "N"]),
                    vec![
                        vec![text("Processed"), Cell::Empty, Cell::Number(processed as f64)],
                        vec![
                            text("Excluded"),
                            text("Missing or out-of-range group codes"),
                            Cell::Number(0.0),
                        ],
                        vec![
                            Cell::Empty,
                            text("At least one missing discriminating variable"),
                            Cell::Number(missing_var as f64),
                        ],
                        vec![
                            text("Used in Output"),
                            Cell::Empty,
                            Cell::Number(classified.len() as f64),
                        ],
                    ],
                    Some(format!(
                        "Cases with missing or out-of-range group codes are classified: {missing_group} in the file."
                    )),
                ));
                let rows = (0..g)
                    .map(|j| {
                        vec![
                            text(group_labels[j].clone()),
                            number(Some(priors[j])),
                            Cell::Number(s.count[j] as f64),
                            number(Some(s.nj[j])),
                        ]
                    })
                    .chain(std::iter::once(vec![
                        text("Total"),
                        number(Some(priors.iter().sum())),
                        Cell::Number(s.count.iter().sum::<usize>() as f64),
                        number(Some(s.n)),
                    ]))
                    .collect();
                tables.push(table(
                    "Prior Probabilities for Groups",
                    1,
                    vec![
                        (String::new(), command.group.clone()),
                        (String::new(), "Prior".into()),
                        ("Cases Used in Analysis".into(), "Unweighted".into()),
                        ("Cases Used in Analysis".into(), "Weighted".into()),
                    ],
                    rows,
                    None,
                ));
            }
            if command.statistics.coeff {
                let inv = inverse(&w11).expect("regular").0;
                let coef: Vec<Vec<f64>> = (0..g)
                    .map(|j| {
                        (0..q)
                            .map(|i| {
                                n_g * (0..q).map(|l| inv[i][l] * s.mean[j][set[l]]).sum::<f64>()
                            })
                            .collect()
                    })
                    .collect();
                let mut rows: Vec<Vec<Cell>> = (0..q)
                    .map(|i| {
                        let mut row = vec![text(set_names[i].clone())];
                        row.extend((0..g).map(|j| number(Some(coef[j][i]))));
                        row
                    })
                    .collect();
                let mut last = vec![text("(Constant)")];
                last.extend((0..g).map(|j| {
                    let quad: f64 = (0..q).map(|i| coef[j][i] * s.mean[j][set[i]]).sum();
                    number(Some(priors[j].ln() - 0.5 * quad))
                }));
                rows.push(last);
                let mut columns = plain(&[""]);
                columns.extend(group_labels.iter().map(|l| (group_name.clone(), l.clone())));
                tables.push(table(
                    "Classification Function Coefficients",
                    1,
                    columns,
                    rows,
                    Some("Fisher's linear discriminant functions".into()),
                ));
            }
            if command.statistics.table || command.statistics.crossvalid {
                let cross = command
                    .statistics
                    .crossvalid
                    .then(|| cross_validate(&sample, &s, &set, &priors));
                let parts: Vec<(Option<bool>, String)> = match command.select {
                    Some(_) => vec![
                        (Some(true), "Cases Selected".to_string()),
                        (Some(false), "Cases Not Selected".to_string()),
                    ],
                    None => vec![(None, String::new())],
                };
                for (part, name) in parts {
                    let in_part = |c: usize| part.is_none_or(|p| selected[c] == p);
                    let mut blocks: Vec<(&str, Vec<Tally>)> = Vec::new();
                    let original: Vec<(Option<usize>, usize, f64)> = classified
                        .iter()
                        .filter(|(c, _)| in_part(*c))
                        .map(|(c, k)| (group_of[*c].map(index_of), k.predicted, weight(*c)))
                        .collect();
                    if command.statistics.table || cross.is_none() {
                        blocks.push(("Original", original));
                    }
                    if let Some(cross) = &cross
                        && part != Some(false)
                    {
                        let rows = used
                            .iter()
                            .zip(cross)
                            .map(|(&c, &k)| (group_of[c].map(index_of), k, weight(c)))
                            .collect();
                        blocks.push(("Cross-validated", rows));
                    }
                    let mut rows = Vec::new();
                    let mut notes = Vec::new();
                    for (what, cells) in &blocks {
                        let ungrouped = cells.iter().any(|c| c.0.is_none());
                        let actual: Vec<Option<usize>> =
                            (0..g).map(Some).chain(ungrouped.then_some(None)).collect();
                        for (pass, kind) in ["Count", "%"].iter().enumerate() {
                            for (r, act) in actual.iter().enumerate() {
                                let of: Vec<&(Option<usize>, usize, f64)> =
                                    cells.iter().filter(|c| c.0 == *act).collect();
                                let total: f64 = of.iter().map(|c| c.2).sum();
                                let mut row = vec![
                                    if pass == 0 && r == 0 {
                                        text(*what)
                                    } else {
                                        Cell::Empty
                                    },
                                    if r == 0 { text(*kind) } else { Cell::Empty },
                                    text(match act {
                                        Some(j) => group_labels[*j].clone(),
                                        None => "Ungrouped cases".into(),
                                    }),
                                ];
                                for j in 0..g {
                                    let n: f64 = of.iter().filter(|c| c.1 == j).map(|c| c.2).sum();
                                    row.push(match pass {
                                        0 => number(Some(n)),
                                        _ => number(Some(100.0 * n / total)),
                                    });
                                }
                                row.push(match pass {
                                    0 => number(Some(total)),
                                    _ => number(Some(100.0)),
                                });
                                rows.push(row);
                            }
                        }
                        let grouped: Vec<_> = cells.iter().filter(|c| c.0.is_some()).collect();
                        let right: f64 = grouped
                            .iter()
                            .filter(|c| c.0 == Some(c.1))
                            .map(|c| c.2)
                            .sum();
                        let all: f64 = grouped.iter().map(|c| c.2).sum();
                        notes.push(format!(
                            "{:.1}% of {} grouped cases correctly classified.",
                            100.0 * right / all,
                            what.to_lowercase()
                        ));
                    }
                    if cross.is_some() {
                        notes.push("Cross validation: every case is classified by functions estimated without that case.".into());
                    }
                    let mut columns = plain(&["", "", ""]);
                    columns.extend(
                        group_labels
                            .iter()
                            .map(|l| ("Predicted Group Membership".to_string(), l.clone())),
                    );
                    columns.push((String::new(), "Total".into()));
                    let mut caption = notes.join(" ");
                    if !name.is_empty() {
                        caption = format!("{name}. {caption}");
                    }
                    tables.push(table(
                        "Classification Results",
                        3,
                        columns,
                        rows,
                        Some(caption),
                    ));
                }
            }
            if let Some(save) = &options.save {
                let data = self.active()?;
                let taken = |name: &str, extra: &[(String, String, Vec<Option<f64>>, bool)]| {
                    data.variable_index(name).is_some()
                        || extra.iter().any(|(n, ..)| n.eq_ignore_ascii_case(name))
                };
                let mut fresh =
                    |count: usize, extra: &[(String, String, Vec<Option<f64>>, bool)]| {
                        loop {
                            save_count += 1;
                            let m = save_count;
                            let names: Vec<String> =
                                (1..=count).map(|k| format!("DSC{k}_{m}")).collect();
                            if names.iter().all(|n| !taken(n, extra)) {
                                return names;
                            }
                        }
                    };
                let column = |f: &dyn Fn(&Classified) -> f64| -> Vec<Option<f64>> {
                    let mut out = vec![None; cases];
                    for (c, k) in &classified {
                        out[*c] = Some(f(k));
                    }
                    out
                };
                let mut added = Vec::new();
                if let Some(name) = &save.class {
                    let name = match name {
                        Some(n) => n.clone(),
                        None => (1..)
                            .map(|m| format!("DSC_{m}"))
                            .find(|n| !taken(n, &saves))
                            .expect("a free name"),
                    };
                    let values = column(&|k| group_values[k.predicted]);
                    added.push((
                        name,
                        format!("Predicted group for analysis {analysis_number}"),
                        values,
                        true,
                    ));
                }
                if let Some(root) = &save.scores {
                    let names = match root {
                        Some(r) => (1..=kf).map(|k| format!("{r}{k}")).collect(),
                        None => fresh(kf, &saves),
                    };
                    for (k, name) in names.into_iter().enumerate() {
                        let values = column(&|c| c.scores[k]);
                        added.push((
                            name,
                            format!("Discriminant Scores from Function {} for Analysis {analysis_number}", k + 1),
                            values,
                            false,
                        ));
                    }
                }
                if let Some(root) = &save.probs {
                    let names = match root {
                        Some(r) => (1..=g).map(|k| format!("{r}{k}")).collect(),
                        None => fresh(g, &saves),
                    };
                    for (j, name) in names.into_iter().enumerate() {
                        let values = column(&|c| c.posterior[j]);
                        added.push((
                            name,
                            format!(
                                "Probabilities of Membership in {} for Analysis {analysis_number}",
                                group_labels[j]
                            ),
                            values,
                            false,
                        ));
                    }
                }
                for (name, ..) in &added {
                    if taken(name, &saves) {
                        return Err(Error(format!(
                            "DISCRIMINANT /SAVE: variable {name} already exists"
                        )));
                    }
                }
                saves.extend(added);
            }
        }
        for warning in warnings {
            self.warn(warning);
        }
        if !saves.is_empty() {
            let (format, value_labels) = (class_format, class_labels);
            self.all_or_nothing(|engine| {
                let data = engine.active_mut()?;
                for (name, label, values, class) in saves {
                    data.set_numeric_values(&name, values)?;
                    data.set_variable_label(&name, Some(&label))?;
                    if class {
                        data.set_format(&name, &format)?;
                        data.set_value_labels(&name, value_labels.clone())?;
                        data.set_measure(&name, Measure::Nominal)?;
                    } else {
                        data.set_format(&name, "F11.5")?;
                        data.set_measure(&name, Measure::Scale)?;
                    }
                }
                Ok(())
            })?;
        }
        Ok(tables)
    }

    fn discriminant_descriptives(
        &self,
        command: &Discriminant,
        s: &Sums,
        names: &[String],
        groups: &[String],
        tables: &mut Vec<Table>,
    ) {
        let st = &command.statistics;
        let (g, p) = (s.g, names.len());
        let n_g = s.n - g as f64;
        // Group Statistics: valid N always, means and deviations when asked
        let mut columns = plain(&[&command.group, ""]);
        if st.mean {
            columns.push((String::new(), "Mean".into()));
        }
        if st.stddev {
            columns.push((String::new(), "Std. Deviation".into()));
        }
        columns.push(("Valid N (listwise)".into(), "Unweighted".into()));
        columns.push(("Valid N (listwise)".into(), "Weighted".into()));
        let mut rows = Vec::new();
        let titles = groups.iter().map(String::as_str).chain(["Total"]);
        for (j, title) in titles.enumerate() {
            for (i, name) in names.iter().enumerate() {
                let mut row = vec![
                    match i {
                        0 => text(title),
                        _ => Cell::Empty,
                    },
                    text(name.clone()),
                ];
                let (mean, var, count, weighted) = match j == g {
                    true => (
                        s.total_mean[i],
                        s.t[i][i] / (s.n - 1.0),
                        s.count.iter().sum::<usize>(),
                        s.n,
                    ),
                    false => (
                        s.mean[j][i],
                        s.group_sscp[j][i][i] / (s.nj[j] - 1.0),
                        s.count[j],
                        s.nj[j],
                    ),
                };
                if st.mean {
                    row.push(number(Some(mean)));
                }
                if st.stddev {
                    row.push(number(Some(var.sqrt())));
                }
                row.push(Cell::Number(count as f64));
                row.push(number(Some(weighted)));
                rows.push(row);
            }
        }
        tables.push(table("Group Statistics", 2, columns, rows, None));
        if st.univf {
            let rows = (0..p)
                .map(|i| {
                    let (w, t) = (s.w[i][i], s.t[i][i]);
                    let f = (t - w) * n_g / (w * (g as f64 - 1.0));
                    vec![
                        text(names[i].clone()),
                        number(Some(w / t)),
                        number(Some(f)),
                        Cell::Number(g as f64 - 1.0),
                        number(Some(n_g)),
                        number(Some(f_p(f, g as f64 - 1.0, n_g))),
                    ]
                })
                .collect();
            tables.push(table(
                "Tests of Equality of Group Means",
                1,
                plain(&["", "Wilks' Lambda", "F", "df1", "df2", "Sig."]),
                rows,
                None,
            ));
        }
        let matrix_rows =
            |title: &str, m: &dyn Fn(usize, usize) -> f64, rows: &mut Vec<Vec<Cell>>| {
                for (i, name) in names.iter().enumerate() {
                    let mut row = vec![
                        if i == 0 { text(title) } else { Cell::Empty },
                        text(name.clone()),
                    ];
                    row.extend((0..p).map(|l| number(Some(m(i, l)))));
                    rows.push(row);
                }
            };
        let mut matrix_columns = plain(&["", ""]);
        matrix_columns.extend(names.iter().map(|n| (String::new(), n.clone())));
        if st.cov || st.corr {
            let mut rows = Vec::new();
            if st.cov {
                matrix_rows("Covariance", &|i, l| s.w[i][l] / n_g, &mut rows);
            }
            if st.corr {
                matrix_rows(
                    "Correlation",
                    &|i, l| s.w[i][l] / (s.w[i][i] * s.w[l][l]).sqrt(),
                    &mut rows,
                );
            }
            tables.push(table(
                "Pooled Within-Groups Matrices",
                2,
                matrix_columns.clone(),
                rows,
                None,
            ));
        }
        if st.gcov || st.tcov {
            let mut rows = Vec::new();
            if st.gcov {
                for (j, group) in groups.iter().enumerate() {
                    matrix_rows(
                        group,
                        &|i, l| s.group_sscp[j][i][l] / (s.nj[j] - 1.0),
                        &mut rows,
                    );
                }
            }
            if st.tcov {
                matrix_rows("Total", &|i, l| s.t[i][l] / (s.n - 1.0), &mut rows);
            }
            let mut columns = plain(&[&command.group, ""]);
            columns.extend(names.iter().map(|n| (String::new(), n.clone())));
            tables.push(table("Covariance Matrices", 2, columns, rows, None));
        }
        if st.boxm {
            tables.extend(box_m(s, names, groups));
        }
    }

    fn stepwise_tables(
        &self,
        a: &Analysis,
        options: &DiscriminantAnalysis,
        levels: &[u32],
        steps: &[Step],
        names: &[String],
        tables: &mut Vec<Table>,
    ) {
        let s = a.sums;
        let g = s.g as f64;
        let stat = statistic_name(a.method);
        let value_of = |set: &[usize]| a.criteria(set).map(|c| a.value(&c));
        let rows = steps
            .iter()
            .enumerate()
            .map(|(k, step)| {
                vec![
                    Cell::Number((k + 1) as f64),
                    text(
                        step.entered
                            .iter()
                            .map(|&i| names[i].clone())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                    text(step.removed.map_or(String::new(), |i| names[i].clone())),
                    number(value_of(&step.set)),
                ]
            })
            .collect();
        let entry = match options.pin {
            Some(pin) => format!("Maximum significance of F to enter is {pin}."),
            None => format!("Minimum partial F to enter is {}.", options.fin),
        };
        let removal = match options.pout {
            Some(pout) => format!("Minimum significance of F to remove is {pout}."),
            None => format!("Maximum partial F to remove is {}.", options.fout),
        };
        tables.push(table(
            "Variables Entered/Removed",
            1,
            plain(&["Step", "Entered", "Removed", stat]),
            rows,
            Some(format!("{} {entry} {removal}", method_text(a.method))),
        ));
        // Wilks' lambda of every step
        let rows = steps
            .iter()
            .enumerate()
            .filter(|(_, step)| !step.set.is_empty())
            .filter_map(|(k, step)| {
                let c = a.criteria(&step.set)?;
                let q = step.set.len() as f64;
                let (f, d1, d2) = rao_f(c.lambda, q, g, s.n);
                Some(vec![
                    Cell::Number((k + 1) as f64),
                    Cell::Number(q),
                    number(Some(c.lambda)),
                    Cell::Number(q),
                    Cell::Number(g - 1.0),
                    number(Some(s.n - g)),
                    number(Some(f)),
                    number(Some(d1)),
                    number(Some(d2)),
                    number(Some(f_p(f, d1, d2))),
                ])
            })
            .collect();
        tables.push(table(
            "Wilks' Lambda",
            1,
            vec![
                (String::new(), "Step".into()),
                (String::new(), "Number of Variables".into()),
                (String::new(), "Lambda".into()),
                (String::new(), "df1".into()),
                (String::new(), "df2".into()),
                (String::new(), "df3".into()),
                ("F".into(), "Statistic".into()),
                ("F".into(), "df1".into()),
                ("F".into(), "df2".into()),
                ("F".into(), "Sig.".into()),
            ],
            rows,
            Some("Exact F when there are at most two variables or three groups, else Rao's approximation.".into()),
        ));
        // variables in and not in the analysis at each step
        let mut inside = Vec::new();
        let mut outside = Vec::new();
        let sets: Vec<Vec<usize>> = std::iter::once(Vec::new())
            .chain(steps.iter().map(|s| s.set.clone()))
            .collect();
        for (k, set) in sets.iter().enumerate() {
            let mut first = true;
            for &i in set {
                let others: Vec<usize> = set.iter().copied().filter(|&o| o != i).collect();
                let tol = a
                    .tolerances(set)
                    .and_then(|t| t.get(set.iter().position(|&o| o == i)?).copied());
                inside.push(vec![
                    if first {
                        Cell::Number(k as f64)
                    } else {
                        Cell::Empty
                    },
                    text(names[i].clone()),
                    number(tol),
                    number(Some(a.partial_f(i, &others))),
                    match others.is_empty() {
                        true => Cell::Empty,
                        false => number(value_of(&others)),
                    },
                ]);
                first = false;
            }
            let mut first = true;
            for i in (0..names.len()).filter(|i| !set.contains(i)) {
                let mut with = set.clone();
                with.push(i);
                let tol = a.tolerances(&with);
                outside.push(vec![
                    if first {
                        Cell::Number(k as f64)
                    } else {
                        Cell::Empty
                    },
                    text(names[i].clone()),
                    number(tol.as_ref().and_then(|t| t.last().copied())),
                    number(tol.map(|t| t.iter().copied().fold(f64::INFINITY, f64::min))),
                    number(Some(a.partial_f(i, set))),
                    number(value_of(&with)),
                ]);
                first = false;
            }
        }
        let _ = levels;
        tables.push(table(
            "Variables in the Analysis",
            2,
            plain(&["Step", "", "Tolerance", "F to Remove", stat]),
            inside,
            None,
        ));
        tables.push(table(
            "Variables Not in the Analysis",
            2,
            plain(&[
                "Step",
                "",
                "Tolerance",
                "Min. Tolerance",
                "F to Enter",
                stat,
            ]),
            outside,
            None,
        ));
    }
}

/// The positions in `names` of the items of ANALYSIS; TO follows the order of VARIABLES.
fn positions(names: &[String], items: &[VarItem]) -> Result<Vec<usize>, Error> {
    let find = |name: &str| {
        names
            .iter()
            .position(|n| n.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                Error(format!(
                    "DISCRIMINANT /ANALYSIS: {name} is not on VARIABLES"
                ))
            })
    };
    let mut out = Vec::new();
    for item in items {
        match item {
            VarItem::All => out.extend(0..names.len()),
            VarItem::Name(n) => out.push(find(n)?),
            VarItem::Range(a, b) => {
                let (a, b) = (find(a)?, find(b)?);
                if a > b {
                    return Err(Error(format!(
                        "DISCRIMINANT /ANALYSIS: {} comes after {} on VARIABLES",
                        names[a], names[b]
                    )));
                }
                out.extend(a..=b);
            }
        }
    }
    Ok(out)
}

/// Box's M test of equal group covariance matrices, over all variables of the block; groups
/// with a singular covariance matrix are left out.
fn box_m(s: &Sums, names: &[String], groups: &[String]) -> Vec<Table> {
    let p = names.len();
    let pf = p as f64;
    let logs: Vec<Option<f64>> = (0..s.g)
        .map(|j| {
            let c: Matrix = s.group_sscp[j]
                .iter()
                .map(|r| r.iter().map(|v| v / (s.nj[j] - 1.0)).collect())
                .collect();
            if s.nj[j] - 1.0 <= 0.0 {
                return None;
            }
            log_det(&c)
        })
        .collect();
    let ok: Vec<usize> = (0..s.g).filter(|&j| logs[j].is_some()).collect();
    let gp = ok.len() as f64;
    let n1: f64 = ok.iter().map(|&j| s.nj[j]).sum();
    let pooled: Matrix = (0..p)
        .map(|a| {
            (0..p)
                .map(|b| ok.iter().map(|&j| s.group_sscp[j][a][b]).sum::<f64>() / (n1 - gp))
                .collect()
        })
        .collect();
    let pooled_log = log_det(&pooled);
    let mut rows: Vec<Vec<Cell>> = (0..s.g)
        .map(|j| {
            vec![
                text(groups[j].clone()),
                match logs[j] {
                    Some(_) => Cell::Number(pf),
                    None => Cell::Empty,
                },
                number(logs[j]),
            ]
        })
        .collect();
    rows.push(vec![
        text("Pooled within-groups"),
        Cell::Number(pf),
        number(pooled_log),
    ]);
    let mut out = vec![table(
        "Log Determinants",
        1,
        plain(&["", "Rank", "Log Determinant"]),
        rows,
        Some("The ranks and natural logarithms of determinants printed are those of the group covariance matrices.".into()),
    )];
    let Some(pooled_log) = pooled_log.filter(|_| ok.len() >= 2) else {
        return out;
    };
    let m = (n1 - gp) * pooled_log
        - ok.iter()
            .map(|&j| (s.nj[j] - 1.0) * logs[j].expect("nonsingular"))
            .sum::<f64>();
    let inv_sum: f64 = ok.iter().map(|&j| 1.0 / (s.nj[j] - 1.0)).sum();
    let inv_sq: f64 = ok.iter().map(|&j| 1.0 / (s.nj[j] - 1.0).powi(2)).sum();
    let e1 = (inv_sum - 1.0 / (n1 - gp)) * (2.0 * pf * pf + 3.0 * pf - 1.0)
        / (6.0 * (gp - 1.0) * (pf + 1.0));
    let e2 = (inv_sq - 1.0 / (n1 - gp).powi(2)) * (pf - 1.0) * (pf + 2.0) / (6.0 * (gp - 1.0));
    let t1 = (gp - 1.0) * pf * (pf + 1.0) / 2.0;
    let (f, d1, d2) = if (e1 * e1 - e2).abs() <= 1e-4 * e2.abs() {
        // the F cannot be computed accurately: Bartlett's chi-square
        (f64::NAN, t1, f64::NAN)
    } else {
        let t2 = (t1 + 2.0) / (e2 - e1 * e1).abs();
        if e2 > e1 * e1 {
            let b = t1 / (1.0 - e1 - t1 / t2);
            (m / b, t1, t2)
        } else {
            // Box (1949): b = t2 / (1 - e1 + 2 / t2); the manual prints a minus there
            let b = t2 / (1.0 - e1 + 2.0 / t2);
            (t2 * m / (t1 * (b - m)), t1, t2)
        }
    };
    let rows = if f.is_nan() {
        let chi = m * (1.0 - e1);
        vec![
            vec![text("Box's M"), Cell::Empty, number(Some(m))],
            vec![text("Chi-square"), Cell::Empty, number(Some(chi))],
            vec![Cell::Empty, text("df"), number(Some(d1))],
            vec![
                Cell::Empty,
                text("Sig."),
                number(Some(chi_square_p(chi, d1))),
            ],
        ]
    } else {
        vec![
            vec![text("Box's M"), Cell::Empty, number(Some(m))],
            vec![text("F"), text("Approx."), number(Some(f))],
            vec![Cell::Empty, text("df1"), number(Some(d1))],
            vec![Cell::Empty, text("df2"), number(Some(d2))],
            vec![Cell::Empty, text("Sig."), number(Some(f_p(f, d1, d2)))],
        ]
    };
    out.push(table(
        "Test Results",
        2,
        plain(&["", "", ""]),
        rows,
        Some("Tests null hypothesis of equal population covariance matrices.".into()),
    ));
    out
}

/// The group each analysis case is classified into when it is left out of the means and the
/// pooled covariance matrix (the linear discriminant rule over the entered variables).
fn cross_validate(sample: &Sample, s: &Sums, set: &[usize], priors: &[f64]) -> Vec<usize> {
    let q = set.len();
    let g = s.g;
    sample
        .x
        .iter()
        .zip(&sample.w)
        .zip(&sample.group)
        .map(|((x, &f), &own)| {
            let nj = s.nj[own];
            let xs: Vec<f64> = set.iter().map(|&i| x[i]).collect();
            let means: Vec<Vec<f64>> = (0..g)
                .map(|j| {
                    set.iter()
                        .enumerate()
                        .map(|(a, &i)| match j == own {
                            true => (nj * s.mean[j][i] - f * xs[a]) / (nj - f),
                            false => s.mean[j][i],
                        })
                        .collect()
                })
                .collect();
            let d: Vec<f64> = set
                .iter()
                .enumerate()
                .map(|(a, &i)| xs[a] - s.mean[own][i])
                .collect();
            let shrink = f * nj / (nj - f);
            let cov: Matrix = (0..q)
                .map(|a| {
                    (0..q)
                        .map(|b| {
                            (s.w[set[a]][set[b]] - shrink * d[a] * d[b]) / (s.n - f - g as f64)
                        })
                        .collect()
                })
                .collect();
            let Some((inv, _)) = inverse(&cov) else {
                return own;
            };
            (0..g)
                .map(|j| {
                    let e: Vec<f64> = (0..q).map(|a| xs[a] - means[j][a]).collect();
                    let d2: f64 = (0..q)
                        .map(|a| (0..q).map(|b| e[a] * inv[a][b] * e[b]).sum::<f64>())
                        .sum();
                    priors[j].ln() - d2 / 2.0
                })
                .enumerate()
                .fold((0, f64::NEG_INFINITY), |best, (j, v)| {
                    if v > best.1 { (j, v) } else { best }
                })
                .0
        })
        .collect()
}
