//! NOMREG: multinomial logistic regression of a nominal dependent variable on factors and
//! covariates, by maximum likelihood on the subpopulations.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (NOMREG):
//! - A case is used when the dependent, every factor and every covariate is valid and its
//!   weight is above 0; weights are not rounded. The response categories are those of the
//!   cases used, sorted by ORDER; the base category (last by default) has no parameters.
//! - Subpopulations are the distinct combinations of the factors and covariates (or of the
//!   SUBPOP variables); the counts of each response category are summed in each. DELTA is
//!   added to the empty cells of a subpopulation, BIAS to every cell.
//! - Design: as in GLM, one column for the intercept and one per level combination of each
//!   effect's factors, times its covariates. Sweeping the weighted cross-products in column
//!   order finds the redundant columns (SINGULAR); their parameters are 0 in every logit.
//! - Newton-Raphson (the same as Fisher scoring here) with step-halving up to MXSTEP times when
//!   the log likelihood falls. Start: the intercept of logit j is log(N_j / N_base), the rest
//!   0. Stop when the log likelihood changes by less than LCONVERGE, every parameter by less
//!   than PCONVERGE, or the largest gradient element is below the smaller of the two (only
//!   criteria above 0 count); at most MXITER iterations. From iteration CHKSEP on, a fit where
//!   every subpopulation is predicted perfectly stops with a warning (complete separation).
//! - Log likelihood l = sum n_ij log p_ij; the -2 log likelihoods shown add the multinomial
//!   constant sum log(n_i! / prod n_ij!) unless KERNEL. The initial model has p_j = N_j / N
//!   with an intercept, 1 / J without.
//! - Covariance: the inverse information times the SCALE value (Pearson or deviance chi-square
//!   over its df). Wald = (B / SE)^2 with 1 df; the interval of Exp(B) from B +- z SE.
//! - Pseudo R square from the kernel likelihoods: Cox and Snell 1 - (L0 / L)^(2/N), Nagelkerke
//!   that over 1 - L0^(2/N), McFadden 1 - l / l0.
//! - Goodness of fit on the subpopulations with m (J - 1) - p df; likelihood ratio test of an
//!   effect: the model refitted without it, df the difference in non-redundant parameters.
//! - Classification: each subpopulation goes to its most probable category (the first on a
//!   tie); counts use the case weights.
//! - Stepwise (MODEL with `|`): the effects left of `|` and the intercept stay in the model;
//!   the listed effects enter by the smallest p below PIN (likelihood ratio, or the score
//!   statistic U' I^- U at the current estimates with the new parameters at 0) and leave by the
//!   largest p above POUT (likelihood ratio, or Wald b' C^- b of the effect's parameters), each
//!   on the number of parameters that are not redundant; a tie goes to the effect listed first.
//!   FORWARD only enters, BACKWARD starts from all and only removes, FSTEP enters one and then
//!   removes while it can, BSTEP starts from all, removes while it can and then enters one. The
//!   stepping stops when nothing qualifies or a model would come back. RULE: SINGLE lets an
//!   effect in only with every effect of the list made of a part of its variables, SFACTOR
//!   applies that to effects of factors alone, CONTAINMENT only with every effect it contains
//!   (the same covariates, fewer factors), NONE without conditions; an effect that another
//!   needs cannot leave. MAXEFFECT caps entry (FORWARD, FSTEP) and MINEFFECT removal (BACKWARD,
//!   BSTEP), counting effects without the intercept. Entry p values that underflow to 0 are
//!   ranked by chi-square per df. SCALE does not change the stepwise tests.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, ln_gamma, normal_quantile};
use crate::format::category_text;
use crate::glm::{
    Effect, Part, default_terms, effects_of, ginverse, intercept_term, sweep_in_order,
};
use crate::matrix::{Matrix, inverse};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Measure, Values, Variable};
use statscraft_syntax::{
    Nomreg, NomregBase, NomregCriteria, NomregOrder, NomregSave, NomregScale, NomregStepwise,
    StepMethod, StepRule, Term, Value,
};
use std::collections::HashMap;

/// The subpopulations: their design rows (every column, redundant or not) and the counts of
/// each response category.
struct Model {
    x: Vec<Vec<f64>>,
    n: Vec<Vec<f64>>,
    base: usize,
    /// The categories with parameters, in category order.
    others: Vec<usize>,
}

struct Fit {
    /// The non-redundant columns.
    cols: Vec<usize>,
    /// The parameters, logit by logit: `b[l * cols.len() + s]`.
    b: Vec<f64>,
    info: Matrix,
    /// The kernel of the log likelihood.
    ll: f64,
    converged: bool,
    iterations: usize,
}

impl Fit {
    fn parameters(&self) -> usize {
        self.b.len()
    }
}

/// log p_j of every category for one design row.
fn log_probs(row: &[f64], cols: &[usize], b: &[f64], base: usize, others: &[usize]) -> Vec<f64> {
    let q = cols.len();
    let eta: Vec<f64> = (0..others.len())
        .map(|l| {
            cols.iter()
                .enumerate()
                .map(|(s, &c)| row[c] * b[l * q + s])
                .sum()
        })
        .collect();
    let top = eta.iter().copied().fold(0.0_f64, f64::max);
    let log_denominator = ((-top).exp() + eta.iter().map(|e| (e - top).exp()).sum::<f64>()).ln();
    let mut out = vec![0.0; others.len() + 1];
    out[base] = -top - log_denominator;
    for (l, &j) in others.iter().enumerate() {
        out[j] = eta[l] - top - log_denominator;
    }
    out
}

impl Model {
    fn categories(&self) -> usize {
        self.others.len() + 1
    }

    fn probs(&self, row: &[f64], cols: &[usize], b: &[f64]) -> Vec<f64> {
        log_probs(row, cols, b, self.base, &self.others)
            .into_iter()
            .map(f64::exp)
            .collect()
    }

    fn log_likelihood(&self, cols: &[usize], b: &[f64]) -> f64 {
        let mut ll = 0.0;
        for (row, counts) in self.x.iter().zip(&self.n) {
            let lp = log_probs(row, cols, b, self.base, &self.others);
            for (n, l) in counts.iter().zip(lp) {
                if *n > 0.0 {
                    ll += n * l;
                }
            }
        }
        ll
    }

    /// The information matrix and the gradient of the log likelihood.
    fn information(&self, cols: &[usize], b: &[f64]) -> (Matrix, Vec<f64>) {
        let q = cols.len();
        let size = self.others.len() * q;
        let mut info = vec![vec![0.0; size]; size];
        let mut g = vec![0.0; size];
        for (row, counts) in self.x.iter().zip(&self.n) {
            let total: f64 = counts.iter().sum();
            if total <= 0.0 {
                continue;
            }
            let p = self.probs(row, cols, b);
            let xs: Vec<f64> = cols.iter().map(|&c| row[c]).collect();
            for (l, &j) in self.others.iter().enumerate() {
                let r = counts[j] - total * p[j];
                for s in 0..q {
                    g[l * q + s] += xs[s] * r;
                }
                for (k, &h) in self.others.iter().enumerate() {
                    let w = total * p[j] * (f64::from(u8::from(l == k)) - p[h]);
                    for s in 0..q {
                        let ws = w * xs[s];
                        if ws == 0.0 {
                            continue;
                        }
                        for t in 0..q {
                            info[l * q + s][k * q + t] += ws * xs[t];
                        }
                    }
                }
            }
        }
        (info, g)
    }

    /// The columns of `wanted` that are not redundant, in order.
    fn independent(&self, wanted: &[usize], singular: f64) -> Vec<usize> {
        let k = wanted.len();
        let mut xwx = vec![vec![0.0; k]; k];
        for (row, counts) in self.x.iter().zip(&self.n) {
            let w: f64 = counts.iter().sum();
            for a in 0..k {
                let va = row[wanted[a]];
                if va == 0.0 {
                    continue;
                }
                for c in 0..k {
                    xwx[a][c] += w * va * row[wanted[c]];
                }
            }
        }
        let (_, swept) = sweep_in_order(&xwx, &(0..k).collect::<Vec<_>>(), singular);
        wanted
            .iter()
            .zip(swept)
            .filter_map(|(&c, s)| s.then_some(c))
            .collect()
    }

    /// True when every subpopulation's counts all fall in its most probable category.
    fn perfect(&self, cols: &[usize], b: &[f64]) -> (usize, usize) {
        let mut perfect = 0;
        let mut used = 0;
        for (row, counts) in self.x.iter().zip(&self.n) {
            let total: f64 = counts.iter().sum();
            if total <= 0.0 {
                continue;
            }
            used += 1;
            let p = self.probs(row, cols, b);
            let best = most_probable(&p);
            if counts[best] >= total * (1.0 - 1e-12) {
                perfect += 1;
            }
        }
        (perfect, used)
    }

    fn fit(
        &self,
        cols: Vec<usize>,
        intercept: bool,
        c: &NomregCriteria,
        warnings: &mut Vec<String>,
    ) -> Fit {
        if cols.is_empty() {
            // the null model: every category equally likely, nothing to estimate
            return Fit {
                ll: self.log_likelihood(&cols, &[]),
                cols,
                b: Vec::new(),
                info: Vec::new(),
                converged: true,
                iterations: 0,
            };
        }
        let q = cols.len();
        let l_count = self.others.len();
        let mut b = vec![0.0; l_count * q];
        if intercept && cols.first() == Some(&0) {
            let totals: Vec<f64> = (0..self.categories())
                .map(|j| self.n.iter().map(|r| r[j]).sum())
                .collect();
            for (l, &j) in self.others.iter().enumerate() {
                if totals[j] > 0.0 && totals[self.base] > 0.0 {
                    b[l * q] = (totals[j] / totals[self.base]).ln();
                }
            }
        }
        let mut ll = self.log_likelihood(&cols, &b);
        let gradient_tolerance = [c.lconverge, c.pconverge]
            .into_iter()
            .filter(|&v| v > 0.0)
            .reduce(f64::min);
        let mut converged = false;
        let mut iterations = 0;
        loop {
            let (info, g) = self.information(&cols, &b);
            if iterations > 0
                && let Some(t) = gradient_tolerance
                && g.iter().all(|v| v.abs() < t)
            {
                converged = true;
                break;
            }
            if iterations >= c.mxiter as usize {
                break;
            }
            let Some((inv, _)) = inverse(&info) else {
                let (perfect, _) = self.perfect(&cols, &b);
                warnings.push(if perfect > 0 {
                    "NOMREG: quasi-complete separation: some subpopulations are predicted perfectly and the information matrix is singular; the estimates of the last iteration are shown".to_string()
                } else {
                    "NOMREG: the information matrix became singular; the estimates of the last iteration are shown".to_string()
                });
                break;
            };
            let step: Vec<f64> = inv
                .iter()
                .map(|r| r.iter().zip(&g).map(|(a, v)| a * v).sum())
                .collect();
            let mut accepted = None;
            let mut xi = 1.0;
            for _ in 0..c.mxstep.max(1) {
                let candidate: Vec<f64> = b.iter().zip(&step).map(|(v, s)| v + xi * s).collect();
                let lc = self.log_likelihood(&cols, &candidate);
                if lc >= ll - 1e-10 * (1.0 + ll.abs()) {
                    accepted = Some((candidate, lc));
                    break;
                }
                xi /= 2.0;
            }
            iterations += 1;
            let Some((candidate, lc)) = accepted else {
                warnings.push(format!(
                    "NOMREG: the log likelihood did not increase within {} step-halvings at iteration {iterations}; the estimates before it are shown",
                    c.mxstep
                ));
                break;
            };
            let change = b
                .iter()
                .zip(&candidate)
                .map(|(a, v)| (a - v).abs())
                .fold(0.0_f64, f64::max);
            let ll_change = (lc - ll).abs();
            b = candidate;
            ll = lc;
            if (c.lconverge > 0.0 && ll_change < c.lconverge)
                || (c.pconverge > 0.0 && change < c.pconverge)
            {
                converged = true;
                break;
            }
            if iterations >= c.chksep as usize {
                let (perfect, used) = self.perfect(&cols, &b);
                if used > 0 && perfect == used {
                    warnings.push(format!(
                        "NOMREG: complete separation at iteration {iterations}: every subpopulation is predicted perfectly, so the estimates are not reliable"
                    ));
                    break;
                }
            }
        }
        let (info, _) = self.information(&cols, &b);
        Fit {
            cols,
            b,
            info,
            ll,
            converged,
            iterations,
        }
    }
}

/// One step of a stepwise method: the effect entered or removed, the kernel log likelihood
/// of the model after it, and the test that decided.
struct Step {
    entered: bool,
    effect: usize,
    ll: f64,
    chi: f64,
    df: f64,
    p: f64,
}

/// The outcome of a stepwise method: the effects of the starting model and its kernel log
/// likelihood, the steps, and which effects end in the model.
struct Selection {
    start: Vec<usize>,
    start_ll: f64,
    steps: Vec<Step>,
    chosen: Vec<bool>,
}

/// The effects that must be in the model before effect `e` may be, by RULE.
fn required(effects: &[Effect], e: usize, rule: StepRule) -> Vec<usize> {
    let me = &effects[e];
    let sub_multiset = |small: &[usize], big: &[usize]| {
        let mut rest = big.to_vec();
        small
            .iter()
            .all(|x| match rest.iter().position(|y| y == x) {
                Some(at) => {
                    rest.remove(at);
                    true
                }
                None => false,
            })
    };
    (0..effects.len())
        .filter(|&f| f != e && !effects[f].is_intercept())
        .filter(|&f| {
            let other = &effects[f];
            let factors_inside = other.factors.iter().all(|x| me.factors.contains(x));
            let smaller = other.factors.len() + other.covariates.len()
                < me.factors.len() + me.covariates.len();
            match rule {
                StepRule::Single => {
                    factors_inside && sub_multiset(&other.covariates, &me.covariates) && smaller
                }
                StepRule::Sfactor => {
                    me.covariates.is_empty()
                        && other.covariates.is_empty()
                        && factors_inside
                        && smaller
                }
                StepRule::Containment => {
                    other.covariates == me.covariates
                        && me.factors.len() > other.factors.len()
                        && factors_inside
                }
                StepRule::None => false,
            }
        })
        .collect()
}

/// A stepwise method over the effects from `first` on; the earlier ones (and the intercept)
/// stay in the model.
#[allow(clippy::too_many_arguments)]
fn select(
    model: &Model,
    effects: &[Effect],
    first: usize,
    method: StepMethod,
    rules: &NomregStepwise,
    intercept: bool,
    c: &NomregCriteria,
    warnings: &mut Vec<String>,
) -> Selection {
    let count = |set: &[bool]| {
        set.iter()
            .zip(effects)
            .filter(|(on, e)| **on && !e.is_intercept())
            .count()
    };
    let needs: Vec<Vec<usize>> = (0..effects.len())
        .map(|e| required(effects, e, rules.rule))
        .collect();
    let max_effects = rules
        .maxeffect
        .unwrap_or_else(|| effects.iter().filter(|e| !e.is_intercept()).count());
    let mut troubled = false;
    let mut refit = |set: &[bool]| -> Fit {
        let wanted: Vec<usize> = effects
            .iter()
            .zip(set)
            .filter(|(_, on)| **on)
            .flat_map(|(e, _)| e.range())
            .collect();
        let cols = model.independent(&wanted, c.singular);
        let mut quiet = Vec::new();
        let fit = model.fit(cols, intercept, c, &mut quiet);
        troubled |= !quiet.is_empty() || !fit.converged;
        fit
    };
    let all_in = matches!(method, StepMethod::Backward | StepMethod::Bstep);
    let mut set: Vec<bool> = (0..effects.len()).map(|e| e < first || all_in).collect();
    let mut fit = refit(&set);
    let start: Vec<usize> = (0..effects.len()).filter(|&e| set[e]).collect();
    let start_ll = fit.ll;
    let mut visited = vec![set.clone()];
    let mut steps = Vec::new();
    let l_count = model.others.len();
    let p_of = |chi: f64, df: f64| {
        if df > 0.0 {
            chi_square_p(chi.max(0.0), df)
        } else {
            1.0
        }
    };
    // the best entry: (effect, chi, df, p) with the smallest p, the first on a tie
    let capped = matches!(method, StepMethod::Forward | StepMethod::Fstep);
    let entry = |set: &[bool], fit: &Fit, fit_of: &mut dyn FnMut(&[bool]) -> Fit| {
        if capped && count(set) >= max_effects {
            return None;
        }
        let mut best: Option<(usize, f64, f64, f64)> = None;
        for e in first..effects.len() {
            if set[e] || !needs[e].iter().all(|&f| set[f]) {
                continue;
            }
            let (chi, df) = if rules.score_entry {
                let mut wanted = fit.cols.clone();
                wanted.extend(effects[e].range());
                let cols = model.independent(&wanted, c.singular);
                let (q0, q1) = (fit.cols.len(), cols.len());
                let mut b = vec![0.0; l_count * q1];
                for l in 0..l_count {
                    b[l * q1..l * q1 + q0].copy_from_slice(&fit.b[l * q0..(l + 1) * q0]);
                }
                let (info, g) = model.information(&cols, &b);
                let (inv, _) = ginverse(&info, 1e-10);
                let score: f64 = (0..g.len())
                    .map(|i| (0..g.len()).map(|j| g[i] * inv[i][j] * g[j]).sum::<f64>())
                    .sum();
                (score, ((q1 - q0) * l_count) as f64)
            } else {
                let mut more = set.to_vec();
                more[e] = true;
                let bigger = fit_of(&more);
                (
                    2.0 * (bigger.ll - fit.ll),
                    (bigger.parameters() - fit.parameters()) as f64,
                )
            };
            let p = p_of(chi, df);
            // p can underflow to 0 for several effects: then the larger chi-square per df wins
            let better = best.is_none_or(|b| {
                p < b.3 || (p == 0.0 && b.3 == 0.0 && df > 0.0 && chi / df > b.1 / b.2)
            });
            if better {
                best = Some((e, chi, df, p));
            }
        }
        best
    };
    // the best removal: (effect, chi, df, p) with the largest p, the first on a tie
    let removal = |set: &[bool], fit: &Fit, fit_of: &mut dyn FnMut(&[bool]) -> Fit| {
        let mut best: Option<(usize, f64, f64, f64)> = None;
        let cov = if rules.wald_removal {
            ginverse(&fit.info, 1e-10).0
        } else {
            Vec::new()
        };
        for e in first..effects.len() {
            let needed = (0..effects.len()).any(|f| set[f] && needs[f].contains(&e));
            if !set[e] || needed {
                continue;
            }
            let (chi, df) = if rules.wald_removal {
                let q = fit.cols.len();
                let at: Vec<usize> = (0..l_count)
                    .flat_map(|l| {
                        fit.cols
                            .iter()
                            .enumerate()
                            .filter(|(_, col)| effects[e].range().contains(col))
                            .map(move |(s, _)| l * q + s)
                    })
                    .collect();
                let sub: Matrix = at
                    .iter()
                    .map(|&i| at.iter().map(|&j| cov[i][j]).collect())
                    .collect();
                let (inv, rank) = ginverse(&sub, 1e-10);
                let b: Vec<f64> = at.iter().map(|&i| fit.b[i]).collect();
                let wald: f64 = (0..b.len())
                    .map(|i| (0..b.len()).map(|j| b[i] * inv[i][j] * b[j]).sum::<f64>())
                    .sum();
                (wald, rank as f64)
            } else {
                let mut fewer = set.to_vec();
                fewer[e] = false;
                let smaller = fit_of(&fewer);
                (
                    2.0 * (fit.ll - smaller.ll),
                    (fit.parameters() - smaller.parameters()) as f64,
                )
            };
            let p = p_of(chi, df);
            if best.is_none_or(|b| p > b.3) {
                best = Some((e, chi, df, p));
            }
        }
        best
    };
    // removals while they qualify; false when a model would come back
    let remove_all = |set: &mut Vec<bool>,
                      fit: &mut Fit,
                      steps: &mut Vec<Step>,
                      visited: &mut Vec<Vec<bool>>,
                      refit: &mut dyn FnMut(&[bool]) -> Fit,
                      minimum: bool|
     -> bool {
        loop {
            if minimum && count(set) <= rules.mineffect {
                return true;
            }
            let Some((e, chi, df, p)) = removal(set, fit, refit) else {
                return true;
            };
            if p <= rules.pout {
                return true;
            }
            set[e] = false;
            if visited.contains(set) {
                set[e] = true;
                return false;
            }
            visited.push(set.clone());
            *fit = refit(set);
            steps.push(Step {
                entered: false,
                effect: e,
                ll: fit.ll,
                chi,
                df,
                p,
            });
        }
    };
    loop {
        if all_in {
            let minimum = true;
            if !remove_all(
                &mut set,
                &mut fit,
                &mut steps,
                &mut visited,
                &mut refit,
                minimum,
            ) || method == StepMethod::Backward
            {
                break;
            }
        }
        let Some((e, chi, df, p)) = entry(&set, &fit, &mut refit) else {
            break;
        };
        if p >= rules.pin {
            break;
        }
        set[e] = true;
        if visited.contains(&set) {
            set[e] = false;
            break;
        }
        visited.push(set.clone());
        fit = refit(&set);
        steps.push(Step {
            entered: true,
            effect: e,
            ll: fit.ll,
            chi,
            df,
            p,
        });
        if method == StepMethod::Fstep
            && !remove_all(
                &mut set,
                &mut fit,
                &mut steps,
                &mut visited,
                &mut refit,
                false,
            )
        {
            break;
        }
    }
    if troubled {
        warnings.push(
            "NOMREG: a model fitted during the stepwise method did not converge cleanly; its tests are approximate"
                .into(),
        );
    }
    Selection {
        start,
        start_ll,
        steps,
        chosen: set,
    }
}

/// The index of the largest probability, the first on a tie.
fn most_probable(p: &[f64]) -> usize {
    let mut best = 0;
    for (j, v) in p.iter().enumerate() {
        if *v > p[best] {
            best = j;
        }
    }
    best
}

fn key_text(variable: &Variable, key: &Key) -> String {
    match key {
        Key::Number(n) => category_text(*n, variable.format()),
        Key::Text(t) => t.trim_end().to_string(),
    }
}

fn trimmed(key: Key) -> Key {
    match key {
        Key::Text(t) => Key::Text(t.trim_end().to_string()),
        other => other,
    }
}

fn clone_key(key: &Key) -> Key {
    match key {
        Key::Number(n) => Key::Number(*n),
        Key::Text(t) => Key::Text(t.clone()),
    }
}

/// The design columns of the effects: each effect's level combinations among `levels`.
fn place_effects(effects: &mut [Effect], levels: &[Vec<usize>]) -> usize {
    let mut start = 0;
    for e in effects.iter_mut() {
        let mut cells: Vec<Vec<usize>> = levels
            .iter()
            .map(|l| e.factors.iter().map(|&f| l[f]).collect())
            .collect();
        cells.sort();
        cells.dedup();
        e.cells = cells;
        e.start = start;
        start += e.cells.len();
    }
    start
}

/// The design row of a case or subpopulation; None when an effect's level combination has no
/// column.
fn design_row(effects: &[Effect], p: usize, levels: &[usize], cov: &[f64]) -> Option<Vec<f64>> {
    let mut row = vec![0.0; p];
    for e in effects {
        let mine: Vec<usize> = e.factors.iter().map(|&f| levels[f]).collect();
        let at = e.cells.iter().position(|c| *c == mine)?;
        row[e.start + at] = e.covariates.iter().map(|&c| cov[c]).product();
    }
    Some(row)
}

impl Engine {
    pub(crate) fn nomreg(&mut self, command: &Nomreg) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let c = command.criteria;
        let mut warnings = Vec::new();
        let dependent = data.variable(&command.dependent)?;
        let factor_names = self.expand(&command.factors)?;
        let covariate_names = self.expand(&command.covariates)?;
        let mut seen: Vec<&str> = vec![dependent.name()];
        for n in factor_names.iter().chain(&covariate_names) {
            if seen.iter().any(|s| s.eq_ignore_ascii_case(n)) {
                return Err(Error(format!(
                    "{n} appears more than once among the dependent variable, factors and covariates"
                )));
            }
            seen.push(n);
        }
        let factor_vars: Vec<Variable> = factor_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let cov_vars: Vec<Variable> = covariate_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = cov_vars.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "NOMREG covariates are numeric, but {} is a string variable",
                v.name()
            )));
        }
        // the subpopulation variables: by default every factor and covariate
        let (sub_factors, sub_covariates): (Vec<usize>, Vec<usize>) = match &command.subpop {
            None => (
                (0..factor_vars.len()).collect(),
                (0..cov_vars.len()).collect(),
            ),
            Some(items) => {
                let mut f = Vec::new();
                let mut cv = Vec::new();
                for name in self.expand(items)? {
                    if let Some(i) = factor_names
                        .iter()
                        .position(|n| n.eq_ignore_ascii_case(&name))
                    {
                        f.push(i);
                    } else if let Some(i) = covariate_names
                        .iter()
                        .position(|n| n.eq_ignore_ascii_case(&name))
                    {
                        cv.push(i);
                    } else {
                        return Err(Error(format!(
                            "SUBPOP names {name}, which is neither a factor nor a covariate of NOMREG"
                        )));
                    }
                }
                (f, cv)
            }
        };
        // the model
        let terms: Vec<Term> = match &command.model {
            // with a stepwise method the left side is the starting model, even when empty
            Some(terms) if !terms.is_empty() || command.stepwise.is_some() => {
                let mut terms = terms.clone();
                if command.intercept {
                    terms.insert(0, intercept_term());
                }
                terms
            }
            _ if command.fullfactorial => {
                default_terms(&factor_names, &covariate_names, command.intercept)
            }
            _ => {
                let mut terms = Vec::new();
                if command.intercept {
                    terms.push(intercept_term());
                }
                for name in covariate_names.iter().chain(&factor_names) {
                    terms.push(Term {
                        variables: vec![name.clone()],
                        label: name.clone(),
                    });
                }
                terms
            }
        };
        let first = effects_of(&terms, &factor_names, &covariate_names)?.len();
        let mut terms = terms;
        if let Some((_, stepped)) = &command.stepwise {
            terms.extend(stepped.iter().cloned());
        }
        let mut effects = effects_of(&terms, &factor_names, &covariate_names)?;
        if let Some((_, stepped)) = &command.stepwise
            && effects.len() - first < stepped.len()
        {
            return Err(Error(
                "NOMREG: an effect of the stepwise method repeats an effect of the model or of its list"
                    .into(),
            ));
        }
        if effects.is_empty() {
            return Err(Error(
                "NOMREG: the model has no effects and no intercept".into(),
            ));
        }
        for e in &effects {
            for &part in &e.parts {
                let inside = match part {
                    Part::Factor(f) => sub_factors.contains(&f),
                    Part::Covariate(v) => sub_covariates.contains(&v),
                };
                if !inside {
                    return Err(Error(format!(
                        "the NOMREG effect {} uses a variable that is not on SUBPOP",
                        e.label
                    )));
                }
            }
        }
        // the cases
        let cases = data.case_count();
        let weights = case_weights(data)?;
        let weight = |k: usize| weights.as_ref().map_or(1.0, |w| w[k]);
        let dep_keys: Vec<Option<Key>> = categories(&dependent, command.include)
            .into_iter()
            .map(|k| k.map(trimmed))
            .collect();
        let factor_keys: Vec<Vec<Option<Key>>> = factor_vars
            .iter()
            .map(|v| {
                categories(v, command.include)
                    .into_iter()
                    .map(|k| k.map(trimmed))
                    .collect()
            })
            .collect();
        let cov_values: Vec<Vec<Option<f64>>> = cov_vars
            .iter()
            .map(|v| {
                let values = match v.values() {
                    Values::Numeric(values) => values,
                    Values::String(_) => unreachable!("checked above"),
                };
                (0..cases)
                    .map(|k| {
                        values
                            .get(k)
                            .filter(|x| command.include || !v.is_user_missing(*x))
                    })
                    .collect()
            })
            .collect();
        let predictors_valid = |k: usize| {
            factor_keys.iter().all(|col| col[k].is_some())
                && cov_values.iter().all(|col| col[k].is_some())
        };
        let mut rows: Vec<usize> = Vec::new();
        let mut missing = 0.0;
        for (k, dep) in dep_keys.iter().enumerate() {
            if weight(k) <= 0.0 {
                continue;
            }
            if dep.is_some() && predictors_valid(k) {
                rows.push(k);
            } else {
                missing += weight(k);
            }
        }
        if rows.is_empty() {
            return Err(Error(
                "NOMREG: no case has valid values on every variable".into(),
            ));
        }
        // the response categories in ORDER
        let mut cats: Vec<Key> = Vec::new();
        for &k in &rows {
            let key = dep_keys[k].as_ref().expect("valid");
            if !cats.iter().any(|c| c.compare(key).is_eq()) {
                cats.push(clone_key(key));
            }
        }
        match command.order {
            NomregOrder::Ascending => cats.sort_by(|a, b| a.compare(b)),
            NomregOrder::Descending => cats.sort_by(|a, b| b.compare(a)),
            NomregOrder::Data => {}
        }
        let j_count = cats.len();
        if j_count < 2 {
            return Err(Error(format!(
                "NOMREG needs at least two categories of {} among the cases used",
                dependent.name()
            )));
        }
        let base = match &command.base {
            NomregBase::First => 0,
            NomregBase::Last => j_count - 1,
            NomregBase::Value(v) => cats
                .iter()
                .position(|key| match (v, key) {
                    (Value::Number(a), Key::Number(b)) => a == b,
                    (Value::String(a), Key::Text(b)) => a.trim_end() == b,
                    (Value::String(a), Key::Number(b)) => {
                        a.trim().parse::<f64>().ok() == Some(*b)
                            || category_text(*b, dependent.format()).trim() == a.trim()
                    }
                    _ => false,
                })
                .ok_or_else(|| {
                    let shown_value = match v {
                        Value::Number(n) => n.to_string(),
                        Value::String(s) => format!("'{s}'"),
                    };
                    Error(format!(
                        "the BASE value {shown_value} is not a category of {} among the cases used",
                        dependent.name()
                    ))
                })?,
        };
        let others: Vec<usize> = (0..j_count).filter(|&j| j != base).collect();
        let category_of = |k: usize| -> Option<usize> {
            let key = dep_keys[k].as_ref()?;
            cats.iter().position(|c| c.compare(key).is_eq())
        };
        // the levels of each factor, ascending
        let mut levels: Vec<Vec<Key>> = Vec::new();
        for col in &factor_keys {
            let mut found: Vec<Key> = Vec::new();
            for &k in &rows {
                let key = col[k].as_ref().expect("valid");
                if !found.iter().any(|l| l.compare(key).is_eq()) {
                    found.push(clone_key(key));
                }
            }
            found.sort_by(|a, b| a.compare(b));
            levels.push(found);
        }
        let level_of = |f: usize, k: usize| -> Option<usize> {
            let key = factor_keys[f][k].as_ref()?;
            levels[f].iter().position(|l| l.compare(key).is_eq())
        };
        // the subpopulations
        let mut index: HashMap<Vec<u64>, usize> = HashMap::new();
        let mut members: Vec<usize> = Vec::new();
        let mut subpop_of: Vec<usize> = Vec::with_capacity(rows.len());
        for &k in &rows {
            let key: Vec<u64> = sub_factors
                .iter()
                .map(|&f| level_of(f, k).expect("valid") as u64)
                .chain(
                    sub_covariates
                        .iter()
                        .map(|&v| cov_values[v][k].expect("valid").to_bits()),
                )
                .collect();
            let next = members.len();
            let i = *index.entry(key).or_insert(next);
            if i == next {
                members.push(k);
            }
            subpop_of.push(i);
        }
        let m = members.len();
        let case_levels = |k: usize| -> Option<Vec<usize>> {
            (0..factor_vars.len()).map(|f| level_of(f, k)).collect()
        };
        let case_covs = |k: usize| -> Option<Vec<f64>> {
            (0..cov_vars.len()).map(|v| cov_values[v][k]).collect()
        };
        let subpop_levels: Vec<Vec<usize>> = members
            .iter()
            .map(|&k| case_levels(k).expect("valid"))
            .collect();
        let mut p = place_effects(&mut effects, &subpop_levels);
        let x: Vec<Vec<f64>> = members
            .iter()
            .zip(&subpop_levels)
            .map(|(&k, l)| {
                design_row(&effects, p, l, &case_covs(k).expect("valid")).expect("a column")
            })
            .collect();
        let mut raw = vec![vec![0.0; j_count]; m];
        for (&k, &i) in rows.iter().zip(&subpop_of) {
            raw[i][category_of(k).expect("a category")] += weight(k);
        }
        let n: Vec<Vec<f64>> = raw
            .iter()
            .map(|r| {
                let total: f64 = r.iter().sum();
                r.iter()
                    .map(|&v| {
                        let v = v + c.bias;
                        if v == 0.0 && total > 0.0 { c.delta } else { v }
                    })
                    .collect()
            })
            .collect();
        let mut model = Model {
            x,
            n,
            base,
            others: others.clone(),
        };
        let effect_label = |e: &Effect| -> String {
            if e.label.contains('(') {
                return e.label.clone();
            }
            e.parts
                .iter()
                .map(|part| match part {
                    Part::Factor(f) => shown.variable(&factor_vars[*f]),
                    Part::Covariate(v) => shown.variable(&cov_vars[*v]),
                })
                .collect::<Vec<_>>()
                .join(" * ")
        };
        let mut selection = None;
        if let Some((method, _)) = &command.stepwise {
            let chosen = select(
                &model,
                &effects,
                first,
                *method,
                &command.step_criteria,
                command.intercept,
                &c,
                &mut warnings,
            );
            let labels: Vec<String> = effects
                .iter()
                .map(|e| {
                    if e.is_intercept() {
                        "Intercept".to_string()
                    } else {
                        effect_label(e)
                    }
                })
                .collect();
            let mut at = 0;
            effects.retain(|_| {
                at += 1;
                chosen.chosen[at - 1]
            });
            p = place_effects(&mut effects, &subpop_levels);
            model.x = members
                .iter()
                .zip(&subpop_levels)
                .map(|(&k, l)| {
                    design_row(&effects, p, l, &case_covs(k).expect("valid")).expect("a column")
                })
                .collect();
            selection = Some((*method, chosen, labels));
        }
        let l_count = others.len() as f64;
        // the fit
        let all: Vec<usize> = (0..p).collect();
        let cols = model.independent(&all, c.singular);
        let fit = model.fit(cols, command.intercept, &c, &mut warnings);
        if !fit.converged && fit.iterations >= c.mxiter as usize {
            warnings.push(format!(
                "NOMREG: no convergence after {} iterations; the estimates of the last iteration are shown",
                fit.iterations
            ));
        }
        let totals: Vec<f64> = (0..j_count)
            .map(|j| model.n.iter().map(|r| r[j]).sum())
            .collect();
        let big_n: f64 = totals.iter().sum();
        let constant: f64 = model
            .n
            .iter()
            .map(|r| {
                ln_gamma(r.iter().sum::<f64>() + 1.0)
                    - r.iter().map(|v| ln_gamma(v + 1.0)).sum::<f64>()
            })
            .sum();
        let shown_m2ll = |ll: f64| {
            if command.print.kernel {
                -2.0 * ll
            } else {
                -2.0 * (ll + constant)
            }
        };
        let ll0 = if command.intercept {
            totals
                .iter()
                .filter(|&&t| t > 0.0)
                .map(|t| t * (t / big_n).ln())
                .sum::<f64>()
        } else {
            big_n * (1.0 / j_count as f64).ln()
        };
        let parameters = fit.parameters() as f64;
        // goodness of fit
        let mut pearson = 0.0;
        let mut deviance = 0.0;
        for (row, counts) in model.x.iter().zip(&model.n) {
            let total: f64 = counts.iter().sum();
            let prob = model.probs(row, &fit.cols, &fit.b);
            for (o, pr) in counts.iter().zip(&prob) {
                let e = total * pr;
                if e > 0.0 {
                    pearson += (o - e).powi(2) / e;
                }
                if *o > 0.0 {
                    deviance += 2.0 * o * (o / e).ln();
                }
            }
        }
        let fit_df = m as f64 * l_count - parameters;
        let kappa = match command.scale {
            NomregScale::Value(v) => v,
            NomregScale::Pearson if fit_df > 0.0 => pearson / fit_df,
            NomregScale::Deviance if fit_df > 0.0 => deviance / fit_df,
            _ => {
                warnings
                    .push("NOMREG: the goodness-of-fit df are not above 0, so SCALE uses 1".into());
                1.0
            }
        };
        let cov: Option<Matrix> = inverse(&fit.info).map(|(inv, _)| {
            inv.into_iter()
                .map(|r| r.into_iter().map(|v| v * kappa).collect())
                .collect()
        });
        if cov.is_none() {
            warnings.push(
                "NOMREG: the information matrix is singular, so the standard errors are not shown"
                    .into(),
            );
        }
        // texts
        let dep_text = shown.variable(&dependent);
        let cat_label = |j: usize| shown.value(&dependent, &cats[j]);
        let col_name = |col: usize| -> String {
            let e = effects
                .iter()
                .find(|e| e.range().contains(&col))
                .expect("an effect");
            if e.is_intercept() {
                return "Intercept".into();
            }
            let cell = &e.cells[col - e.start];
            let mut f_at = 0;
            e.parts
                .iter()
                .map(|part| match part {
                    Part::Factor(f) => {
                        let level = cell[f_at];
                        f_at += 1;
                        format!(
                            "[{}={}]",
                            factor_vars[*f].name(),
                            key_text(&factor_vars[*f], &levels[*f][level])
                        )
                    }
                    Part::Covariate(v) => cov_vars[*v].name().to_string(),
                })
                .collect::<Vec<_>>()
                .join(" * ")
        };
        let print = command.print;
        let mut tables = Vec::new();
        if print.cps {
            tables.push(case_summary(
                &dep_text,
                &dependent,
                &cats,
                &factor_vars,
                &levels,
                &rows,
                &weight,
                &category_of,
                &level_of,
                missing,
                &raw,
                &shown,
            ));
        }
        let kernel_note = if print.kernel {
            " The -2 log likelihoods are the kernel, without the multinomial constant."
        } else {
            ""
        };
        if let Some((method, chosen, labels)) = &selection
            && print.step
        {
            let mut columns = plain(&["Model", "Action", "Effect(s)"]);
            columns.push(("Model Fitting Criteria".into(), "-2 Log Likelihood".into()));
            for name in ["Chi-Square", "df", "Sig."] {
                columns.push(("Effect Selection Tests".into(), name.into()));
            }
            let start: Vec<&str> = chosen.start.iter().map(|&e| labels[e].as_str()).collect();
            let mut rows = vec![vec![
                Cell::Number(0.0),
                text("Entered"),
                text(if start.is_empty() {
                    "(none)".to_string()
                } else {
                    start.join(", ")
                }),
                number(Some(shown_m2ll(chosen.start_ll))),
                Cell::Empty,
                Cell::Empty,
                Cell::Empty,
            ]];
            for (k, step) in chosen.steps.iter().enumerate() {
                rows.push(vec![
                    Cell::Number((k + 1) as f64),
                    text(if step.entered { "Entered" } else { "Removed" }),
                    text(labels[step.effect].clone()),
                    number(Some(shown_m2ll(step.ll))),
                    number(Some(step.chi)),
                    Cell::Number(step.df),
                    number(Some(step.p)),
                ]);
            }
            let rules = &command.step_criteria;
            let name = match method {
                StepMethod::Backward => "Backward Entry",
                StepMethod::Forward => "Forward Entry",
                StepMethod::Bstep => "Backward Stepwise",
                StepMethod::Fstep => "Forward Stepwise",
            };
            let entry = if rules.score_entry {
                "the score statistic"
            } else {
                "the likelihood ratio"
            };
            let removal = if rules.wald_removal {
                "the Wald statistic"
            } else {
                "the likelihood ratio"
            };
            tables.push(table(
                "Step Summary",
                1,
                columns,
                rows,
                Some(format!(
                    "Stepwise method: {name}. An effect enters by {entry} (p below {}) and leaves by {removal} (p above {}).{kernel_note}",
                    rules.pin, rules.pout
                )),
            ));
        }
        let ic_columns = |rows_aic: bool| -> Vec<(String, String)> {
            let group = "Model Fitting Criteria".to_string();
            let mut cols = Vec::new();
            if print.ic {
                let suffix = if rows_aic { " of Reduced Model" } else { "" };
                cols.push((group.clone(), format!("AIC{suffix}")));
                cols.push((group.clone(), format!("BIC{suffix}")));
            }
            cols.push((
                group,
                format!(
                    "-2 Log Likelihood{}",
                    if rows_aic { " of Reduced Model" } else { "" }
                ),
            ));
            for name in ["Chi-Square", "df", "Sig."] {
                cols.push(("Likelihood Ratio Tests".into(), name.into()));
            }
            cols
        };
        let criteria_cells = |m2ll: f64, k: f64| -> Vec<Cell> {
            let mut out = Vec::new();
            if print.ic {
                out.push(number(Some(m2ll + 2.0 * k)));
                out.push(number(Some(m2ll + k * big_n.ln())));
            }
            out.push(number(Some(m2ll)));
            out
        };
        if print.mfi {
            let mut columns = plain(&["Model"]);
            columns.extend(ic_columns(false));
            let initial_k = if command.intercept { l_count } else { 0.0 };
            let mut first = vec![text(if command.intercept {
                "Intercept Only"
            } else {
                "Null"
            })];
            first.extend(criteria_cells(shown_m2ll(ll0), initial_k));
            first.extend([Cell::Empty, Cell::Empty, Cell::Empty]);
            let chi = 2.0 * (fit.ll - ll0);
            let df = parameters - initial_k;
            let mut last = vec![text("Final")];
            last.extend(criteria_cells(shown_m2ll(fit.ll), parameters));
            last.push(number(Some(chi)));
            last.push(Cell::Number(df));
            last.push(number((df > 0.0).then(|| chi_square_p(chi.max(0.0), df))));
            tables.push(table(
                "Model Fitting Information",
                1,
                columns,
                vec![first, last],
                Some(kernel_note.trim().to_string()).filter(|s| !s.is_empty()),
            ));
        }
        if print.fit {
            let row = |name: &str, v: f64| {
                vec![
                    text(name),
                    number(Some(v)),
                    Cell::Number(fit_df),
                    number((fit_df > 0.0).then(|| chi_square_p(v, fit_df))),
                ]
            };
            tables.push(table(
                "Goodness-of-Fit",
                1,
                plain(&["", "Chi-Square", "df", "Sig."]),
                vec![row("Pearson", pearson), row("Deviance", deviance)],
                None,
            ));
        }
        if print.summary {
            let cox_snell = 1.0 - (2.0 * (ll0 - fit.ll) / big_n).exp();
            let nagelkerke = cox_snell / (1.0 - (2.0 * ll0 / big_n).exp());
            let mcfadden = 1.0 - fit.ll / ll0;
            tables.push(table(
                "Pseudo R-Square",
                1,
                plain(&["", "Value"]),
                vec![
                    vec![text("Cox and Snell"), number(Some(cox_snell))],
                    vec![text("Nagelkerke"), number(Some(nagelkerke))],
                    vec![text("McFadden"), number(Some(mcfadden))],
                ],
                None,
            ));
        }
        if print.lrt {
            let mut columns = plain(&["Effect"]);
            columns.extend(ic_columns(true));
            let mut out = Vec::new();
            for e in effects.iter().filter(|e| !e.is_intercept()) {
                let wanted: Vec<usize> = all
                    .iter()
                    .copied()
                    .filter(|c| !e.range().contains(c))
                    .collect();
                let cols = model.independent(&wanted, c.singular);
                let mut sub_warnings = Vec::new();
                let reduced = model.fit(cols, command.intercept, &c, &mut sub_warnings);
                let chi = 2.0 * (fit.ll - reduced.ll);
                let df = parameters - reduced.parameters() as f64;
                let mut row = vec![text(effect_label(e))];
                row.extend(criteria_cells(
                    shown_m2ll(reduced.ll),
                    reduced.parameters() as f64,
                ));
                row.push(number(Some(chi)));
                row.push(Cell::Number(df));
                row.push(number((df > 0.0).then(|| chi_square_p(chi.max(0.0), df))));
                out.push(row);
                if !sub_warnings.is_empty() || !reduced.converged {
                    warnings.push(format!(
                        "NOMREG: the model without {} did not converge cleanly; its likelihood ratio test is approximate",
                        effect_label(e)
                    ));
                }
            }
            tables.push(table(
                "Likelihood Ratio Tests",
                1,
                columns,
                out,
                Some(format!(
                    "The chi-square is the -2 log likelihood of the reduced model minus that of the final model. The reduced model leaves the effect out; the test asks whether all its parameters are 0.{kernel_note}"
                )),
            ));
        }
        if print.parameter {
            let z = normal_quantile(0.5 + c.cin / 200.0);
            let interval = format!("{}% Confidence Interval for Exp(B)", trim_number(c.cin));
            let mut columns = plain(&[&dep_text, ""]);
            columns.extend(plain(&["B", "Std. Error", "Wald", "df", "Sig.", "Exp(B)"]));
            columns.push((interval.clone(), "Lower Bound".into()));
            columns.push((interval, "Upper Bound".into()));
            let q = fit.cols.len();
            let mut out = Vec::new();
            let mut any_redundant = false;
            for (l, &j) in others.iter().enumerate() {
                for col in 0..p {
                    let header = if col == 0 {
                        text(cat_label(j))
                    } else {
                        Cell::Empty
                    };
                    let name = col_name(col);
                    let Some(s) = fit.cols.iter().position(|&cc| cc == col) else {
                        any_redundant = true;
                        out.push(vec![
                            header,
                            text(name),
                            Cell::Number(0.0),
                            Cell::Empty,
                            Cell::Empty,
                            Cell::Number(0.0),
                            Cell::Empty,
                            Cell::Empty,
                            Cell::Empty,
                            Cell::Empty,
                        ]);
                        continue;
                    };
                    let at = l * q + s;
                    let b = fit.b[at];
                    let se = cov.as_ref().map(|v| v[at][at].sqrt());
                    let wald = se.map(|se| (b / se).powi(2));
                    let intercept = effects[0].is_intercept() && col == 0;
                    out.push(vec![
                        header,
                        text(name),
                        number(Some(b)),
                        number(se),
                        number(wald),
                        Cell::Number(1.0),
                        number(wald.map(|w| chi_square_p(w, 1.0))),
                        number((!intercept).then(|| b.exp())),
                        number(se.filter(|_| !intercept).map(|se| (b - z * se).exp())),
                        number(se.filter(|_| !intercept).map(|se| (b + z * se).exp())),
                    ]);
                }
            }
            let mut caption = format!("The reference category is: {}.", cat_label(base));
            if any_redundant {
                caption.push_str(" A parameter of 0 with df 0 is redundant and set to 0.");
            }
            tables.push(table("Parameter Estimates", 2, columns, out, Some(caption)));
        }
        let names: Vec<String> = others
            .iter()
            .flat_map(|&j| fit.cols.iter().map(move |&col| (j, col)))
            .map(|(j, col)| format!("{}: {}", cat_label(j), col_name(col)))
            .collect();
        for (wanted, title, correlation) in [
            (print.covb, "Asymptotic Covariance Matrix", false),
            (print.corb, "Asymptotic Correlation Matrix", true),
        ] {
            if !wanted {
                continue;
            }
            let Some(v) = &cov else { continue };
            let mut columns = plain(&[""]);
            columns.extend(names.iter().map(|n| (String::new(), n.clone())));
            let out = (0..names.len())
                .map(|a| {
                    let mut row = vec![text(names[a].clone())];
                    row.extend((0..names.len()).map(|b| {
                        let value = if correlation {
                            v[a][b] / (v[a][a] * v[b][b]).sqrt()
                        } else {
                            v[a][b]
                        };
                        number(Some(value))
                    }));
                    row
                })
                .collect();
            tables.push(table(title, 1, columns, out, None));
        }
        let predicted: Vec<usize> = model
            .x
            .iter()
            .map(|row| most_probable(&model.probs(row, &fit.cols, &fit.b)))
            .collect();
        if print.classtable {
            let mut counts = vec![vec![0.0; j_count]; j_count];
            for (i, r) in raw.iter().enumerate() {
                for (j, v) in r.iter().enumerate() {
                    counts[j][predicted[i]] += v;
                }
            }
            let total: f64 = counts.iter().flatten().sum();
            let mut columns = plain(&["Observed"]);
            for j in 0..j_count {
                columns.push(("Predicted".into(), cat_label(j)));
            }
            columns.push(("Predicted".into(), "Percent Correct".into()));
            let mut out = Vec::new();
            for (j, row_counts) in counts.iter().enumerate() {
                let row_total: f64 = row_counts.iter().sum();
                let mut row = vec![text(cat_label(j))];
                row.extend(row_counts.iter().map(|&v| Cell::Number(v)));
                row.push(number(
                    (row_total > 0.0).then(|| 100.0 * row_counts[j] / row_total),
                ));
                out.push(row);
            }
            let correct: f64 = (0..j_count).map(|j| counts[j][j]).sum();
            let mut row = vec![text("Overall Percentage")];
            row.extend((0..j_count).map(|j| {
                let column: f64 = counts.iter().map(|r| r[j]).sum();
                number((total > 0.0).then(|| 100.0 * column / total))
            }));
            row.push(number((total > 0.0).then(|| 100.0 * correct / total)));
            out.push(row);
            tables.push(table("Classification", 1, columns, out, None));
        }
        let save_plan = SavePlan {
            command,
            effects: &effects,
            p,
            fit: &fit,
            model: &model,
            cats: &cats,
            dependent: &dependent,
            case_levels: &case_levels,
            case_covs: &case_covs,
            category_of: &category_of,
            cases,
            dep_text: &dep_text,
        };
        let save = if command.save.is_empty() {
            None
        } else {
            Some(save_plan.columns())
        };
        for w in warnings {
            self.warn(w);
        }
        if let Some(columns) = save {
            self.save_nomreg(columns)?;
        }
        Ok(tables)
    }

    fn save_nomreg(&mut self, columns: Vec<SaveColumn>) -> Result<(), Error> {
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            for column in columns {
                let name = match column.name {
                    Some(name) => name,
                    None => {
                        let mut n = 1;
                        while data
                            .variable_index(&format!("{}_{n}", column.root))
                            .is_some()
                        {
                            n += 1;
                        }
                        format!("{}_{n}", column.root)
                    }
                };
                if data.variable_index(&name).is_some() {
                    return Err(Error(format!(
                        "NOMREG /SAVE: variable {name} already exists"
                    )));
                }
                match column.values {
                    SaveValues::Numbers(values, format, measure) => {
                        data.set_numeric_values(&name, values)?;
                        data.set_format(&name, format)?;
                        data.set_measure(&name, measure)?;
                    }
                    SaveValues::Texts(values) => {
                        let width = values.iter().map(String::len).max().unwrap_or(1).max(1);
                        data.set_string_values(&name, width, values)?;
                    }
                }
                data.set_variable_label(&name, Some(&column.label))?;
            }
            Ok(())
        })
    }
}

/// A number such as 95 or 97.5 without trailing zeros.
fn trim_number(v: f64) -> String {
    let s = format!("{v}");
    s.trim_end_matches(".0").to_string()
}

enum SaveValues {
    Numbers(Vec<Option<f64>>, &'static str, Measure),
    Texts(Vec<String>),
}

struct SaveColumn {
    /// The name given, or None for `root_n` with the first free n.
    name: Option<String>,
    root: String,
    label: String,
    values: SaveValues,
}

struct SavePlan<'a> {
    command: &'a Nomreg,
    effects: &'a [Effect],
    p: usize,
    fit: &'a Fit,
    model: &'a Model,
    cats: &'a [Key],
    dependent: &'a Variable<'a>,
    case_levels: &'a dyn Fn(usize) -> Option<Vec<usize>>,
    case_covs: &'a dyn Fn(usize) -> Option<Vec<f64>>,
    category_of: &'a dyn Fn(usize) -> Option<usize>,
    cases: usize,
    dep_text: &'a str,
}

impl SavePlan<'_> {
    fn columns(&self) -> Vec<SaveColumn> {
        // the probabilities of every case whose factors and covariates are valid and known
        let probs: Vec<Option<Vec<f64>>> = (0..self.cases)
            .map(|k| {
                let levels = (self.case_levels)(k)?;
                let covs = (self.case_covs)(k)?;
                let row = design_row(self.effects, self.p, &levels, &covs)?;
                Some(self.model.probs(&row, &self.fit.cols, &self.fit.b))
            })
            .collect();
        let cat_text = |j: usize| key_text(self.dependent, &self.cats[j]);
        let probability =
            |values: Vec<Option<f64>>| SaveValues::Numbers(values, "F8.2", Measure::Scale);
        let mut out = Vec::new();
        for item in &self.command.save {
            match item {
                NomregSave::Estprob { root, count } => {
                    for j in 0..self.cats.len().min(*count as usize) {
                        let (name, default_root) = match root {
                            Some(r) => (Some(format!("{r}{}", j + 1)), String::new()),
                            None => (None, format!("EST{}", j + 1)),
                        };
                        out.push(SaveColumn {
                            name,
                            root: default_root,
                            label: format!(
                                "Estimated probability of {} = {}",
                                self.dep_text,
                                cat_text(j)
                            ),
                            values: probability(
                                probs.iter().map(|p| p.as_ref().map(|p| p[j])).collect(),
                            ),
                        });
                    }
                }
                NomregSave::Predcat(name) => {
                    let best: Vec<Option<usize>> = probs
                        .iter()
                        .map(|p| p.as_ref().map(|p| most_probable(p)))
                        .collect();
                    let values = match self.cats[0] {
                        Key::Number(_) => SaveValues::Numbers(
                            best.iter()
                                .map(|b| {
                                    b.and_then(|j| match self.cats[j] {
                                        Key::Number(n) => Some(n),
                                        Key::Text(_) => None,
                                    })
                                })
                                .collect(),
                            "F8.0",
                            Measure::Nominal,
                        ),
                        Key::Text(_) => SaveValues::Texts(
                            best.iter()
                                .map(|b| b.map(cat_text).unwrap_or_default())
                                .collect(),
                        ),
                    };
                    out.push(SaveColumn {
                        name: name.clone(),
                        root: "PRE".into(),
                        label: "Predicted response category".into(),
                        values,
                    });
                }
                NomregSave::Pcprob(name) => out.push(SaveColumn {
                    name: name.clone(),
                    root: "PCP".into(),
                    label: "Probability of the predicted category".into(),
                    values: probability(
                        probs
                            .iter()
                            .map(|p| p.as_ref().map(|p| p[most_probable(p)]))
                            .collect(),
                    ),
                }),
                NomregSave::Acprob(name) => out.push(SaveColumn {
                    name: name.clone(),
                    root: "ACP".into(),
                    label: "Probability of the observed category".into(),
                    values: probability(
                        (0..self.cases)
                            .map(|k| {
                                let p = probs[k].as_ref()?;
                                Some(p[(self.category_of)(k)?])
                            })
                            .collect(),
                    ),
                }),
            }
        }
        out
    }
}

#[allow(clippy::too_many_arguments)]
fn case_summary(
    dep_text: &str,
    dependent: &Variable,
    cats: &[Key],
    factor_vars: &[Variable],
    levels: &[Vec<Key>],
    rows: &[usize],
    weight: &dyn Fn(usize) -> f64,
    category_of: &dyn Fn(usize) -> Option<usize>,
    level_of: &dyn Fn(usize, usize) -> Option<usize>,
    missing: f64,
    raw: &[Vec<f64>],
    shown: &crate::shown::Shown,
) -> Table {
    let valid: f64 = rows.iter().map(|&k| weight(k)).sum();
    let percent = |v: f64| number((valid > 0.0).then(|| 100.0 * v / valid));
    let mut out = Vec::new();
    let mut dep_counts = vec![0.0; cats.len()];
    for &k in rows {
        dep_counts[category_of(k).expect("a category")] += weight(k);
    }
    for (j, v) in dep_counts.iter().enumerate() {
        out.push(vec![
            if j == 0 { text(dep_text) } else { Cell::Empty },
            text(shown.value(dependent, &cats[j])),
            Cell::Number(*v),
            percent(*v),
        ]);
    }
    for (f, var) in factor_vars.iter().enumerate() {
        let mut counts = vec![0.0; levels[f].len()];
        for &k in rows {
            counts[level_of(f, k).expect("a level")] += weight(k);
        }
        for (l, v) in counts.iter().enumerate() {
            out.push(vec![
                if l == 0 {
                    text(shown.variable(var))
                } else {
                    Cell::Empty
                },
                text(shown.value(var, &levels[f][l])),
                Cell::Number(*v),
                percent(*v),
            ]);
        }
    }
    out.push(vec![
        text("Valid"),
        Cell::Empty,
        Cell::Number(valid),
        Cell::Empty,
    ]);
    out.push(vec![
        text("Missing"),
        Cell::Empty,
        Cell::Number(missing),
        Cell::Empty,
    ]);
    out.push(vec![
        text("Total"),
        Cell::Empty,
        Cell::Number(valid + missing),
        Cell::Empty,
    ]);
    out.push(vec![
        text("Subpopulation"),
        Cell::Empty,
        Cell::Number(raw.len() as f64),
        Cell::Empty,
    ]);
    let single = raw
        .iter()
        .filter(|r| r.iter().filter(|&&v| v > 0.0).count() == 1)
        .count();
    let caption = (single > 0).then(|| {
        format!(
            "{} has only one observed value in {single} ({:.1}%) of the subpopulations.",
            dep_text,
            100.0 * single as f64 / raw.len() as f64
        )
    });
    table(
        "Case Processing Summary",
        2,
        plain(&["", "", "N", "Marginal Percentage"]),
        out,
        caption,
    )
}
