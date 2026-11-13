//! UNIANOVA and GLM: the general linear model with one dependent variable, and GLM's
//! multivariate and repeated measures models.
//!
//! Rules from the Command Syntax Reference (UNIANOVA, GLM) and the Algorithms manual (GLM, and
//! the appendix Sums of Squares):
//! - A case is used when every dependent, factor and covariate is valid and its weight, rounded
//!   to a whole number, is positive (and REGWGT, when given, is positive). N is the sum of the
//!   rounded weights.
//! - The design matrix has one column for the intercept, one per level combination of the
//!   factors of each effect (the combinations that occur), times the product of the effect's
//!   covariates. Sweeping X'WX in column order gives a g2 generalized inverse; a column whose
//!   pivot falls to EPS times its original diagonal is redundant and its parameter is 0.
//! - Type I sums of squares fit the effects in order; Type II adjusts an effect for the effects
//!   that do not contain it; Type III and IV build their hypothesis matrix from the general form
//!   of the estimable functions as the appendix describes (IV distributes the coefficients of
//!   the effect over the cells of the effects that contain it). F contains E when both have the
//!   same covariates and F has every factor of E and more; the intercept is contained in every
//!   pure factor effect.
//! - Estimated marginal means average the cells of each effect with equal weights over the
//!   levels not fixed by TABLES, with covariates at their means (the mean of a product for a
//!   product) or the WITH values; a mean that is not estimable has no value.
//! - Observed power: noncentrality F times df (|t| for a parameter), power from the noncentral F
//!   (t) at ALPHA.
//! - Multivariate tests of L B M = K: S_H = (LBM - K)' (L G L')^- (LBM - K) against
//!   S_E = M' S M, from the eigenvalues of S_E^-1 S_H; s is the smaller of the hypothesis df and
//!   the rank of S_E. Each statistic's noncentrality is its error df times eta^2 / (1 - eta^2).
//! - Box's M: observed cell means and covariances in the cells of all factors, cells with a
//!   singular matrix left out, F approximation of Box. Bartlett's sphericity test of the
//!   residual SSCP uses the same chi-square expansion as Mauchly's test.

use crate::contrast::contrast_rows;
use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::dist::{f_p, f_quantile, noncentral_f_cdf, noncentral_t_cdf, t_p, t_quantile};
use crate::examine::Distribution;
use crate::format::category_text;
use crate::matrix::{Matrix, multiply, sweep, transpose};
use crate::npar_tests::text;
use crate::oneway::{Group, group, posthoc_tables};
use crate::regression::{plain, table};
use crate::shown::{Show, Shown};
use crate::{Cell, Chart, Engine, Error, Table};
use statscraft_data::{Measure, Values, Variable};
use statscraft_syntax::{
    Adjustment, Contrast, Emmeans, Glm, GlmPlot, GlmTemp, LRow, PercentileMethod, Posthoc, Profile,
    Term,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Part {
    Factor(usize),
    Covariate(usize),
}

#[derive(Clone)]
pub(crate) struct Effect {
    pub(crate) label: String,
    pub(crate) parts: Vec<Part>,
    /// The factors in the order written.
    pub(crate) factors: Vec<usize>,
    /// The covariates, sorted; one may repeat.
    pub(crate) covariates: Vec<usize>,
    /// The level of each factor of every column.
    pub(crate) cells: Vec<Vec<usize>>,
    pub(crate) start: usize,
}

impl Effect {
    pub(crate) fn range(&self) -> std::ops::Range<usize> {
        self.start..self.start + self.cells.len()
    }

    pub(crate) fn is_intercept(&self) -> bool {
        self.parts.is_empty()
    }

    /// True when `self` contains `inner`.
    fn contains(&self, inner: &Effect) -> bool {
        self.covariates == inner.covariates
            && self.factors.len() > inner.factors.len()
            && inner.factors.iter().all(|f| self.factors.contains(f))
    }
}

/// The cases of the analysis.
#[derive(Clone)]
pub(crate) struct Data {
    /// The names of the factors, the value of each level as text and its label.
    pub(crate) factors: Vec<String>,
    /// The factors as tables show them (SET TVARS).
    pub(crate) factor_texts: Vec<String>,
    pub(crate) values: Vec<Vec<String>>,
    pub(crate) labels: Vec<Vec<String>>,
    pub(crate) labelled: bool,
    pub(crate) covariates: Vec<String>,
    pub(crate) case: Vec<usize>,
    /// The level of every factor of each row.
    pub(crate) level: Vec<Vec<usize>>,
    pub(crate) cov: Vec<Vec<f64>>,
    /// The dependent values of each row (several in the repeated measures model).
    pub(crate) y: Vec<Vec<f64>>,
    /// Frequency (rounded case weight) and regression weight.
    pub(crate) f: Vec<f64>,
    pub(crate) g: Vec<f64>,
}

impl Data {
    pub(crate) fn rows(&self) -> usize {
        self.case.len()
    }

    /// The same cases with dependent variable `j` alone.
    fn only(&self, j: usize) -> Data {
        Data {
            y: self.y.iter().map(|row| vec![row[j]]).collect(),
            ..self.clone()
        }
    }

    fn w(&self, i: usize) -> f64 {
        self.f[i] * self.g[i]
    }

    fn n(&self) -> f64 {
        self.f.iter().sum()
    }

    /// The weighted mean of the product of covariates.
    pub(crate) fn covariate_mean(&self, covariates: &[usize]) -> f64 {
        let w: f64 = (0..self.rows()).map(|i| self.w(i)).sum();
        (0..self.rows())
            .map(|i| self.w(i) * covariates.iter().map(|&c| self.cov[i][c]).product::<f64>())
            .sum::<f64>()
            / w
    }
}

/// A generalized inverse of a symmetric matrix by sweeping the pivots that stay above `eps`
/// times their original value, with its rank.
pub(crate) fn ginverse(a: &Matrix, eps: f64) -> (Matrix, usize) {
    let n = a.len();
    let mut m = a.clone();
    let scale = a
        .iter()
        .enumerate()
        .map(|(i, r)| r[i].abs())
        .fold(0.0_f64, f64::max);
    let mut swept = vec![false; n];
    for k in 0..n {
        if m[k][k] > eps * a[k][k].abs().max(1e-300) && m[k][k] > 1e-13 * scale {
            sweep(&mut m, k);
            swept[k] = true;
        }
    }
    let out = (0..n)
        .map(|i| {
            (0..n)
                .map(|j| if swept[i] && swept[j] { m[i][j] } else { 0.0 })
                .collect()
        })
        .collect();
    (out, swept.iter().filter(|&&s| s).count())
}

/// Sets the columns of each effect (the level combinations that occur) and returns the design
/// row of every case.
pub(crate) fn design(data: &Data, effects: &mut [Effect]) -> Vec<Vec<f64>> {
    let mut start = 0;
    for e in effects.iter_mut() {
        let mut cells: Vec<Vec<usize>> = (0..data.rows())
            .map(|i| e.factors.iter().map(|&f| data.level[i][f]).collect())
            .collect();
        cells.sort();
        cells.dedup();
        e.cells = cells;
        e.start = start;
        start += e.cells.len();
    }
    (0..data.rows())
        .map(|i| {
            let mut row = vec![0.0; start];
            for e in effects.iter() {
                let mine: Vec<usize> = e.factors.iter().map(|&f| data.level[i][f]).collect();
                let at = e.cells.iter().position(|c| *c == mine).expect("a cell");
                let value: f64 = e.covariates.iter().map(|&c| data.cov[i][c]).product();
                row[e.start + at] = value;
            }
            row
        })
        .collect()
}

pub(crate) struct Fit {
    pub(crate) effects: Vec<Effect>,
    pub(crate) p: usize,
    pub(crate) r: usize,
    pub(crate) x: Vec<Vec<f64>>,
    /// Z'WZ before sweeping.
    pub(crate) zwz: Matrix,
    pub(crate) ginv: Matrix,
    pub(crate) b: Matrix,
    pub(crate) s: Matrix,
    pub(crate) redundant: Vec<bool>,
    pub(crate) rank: usize,
    pub(crate) n: f64,
    pub(crate) eps: f64,
}

impl Fit {
    pub(crate) fn new(data: &Data, mut effects: Vec<Effect>, y: &[Vec<f64>], eps: f64) -> Fit {
        let x = design(data, &mut effects);
        let p = effects.last().map_or(0, |e| e.start + e.cells.len());
        let r = y.first().map_or(0, Vec::len);
        let mut zwz = vec![vec![0.0; p + r]; p + r];
        for i in 0..data.rows() {
            let z: Vec<f64> = x[i].iter().chain(&y[i]).copied().collect();
            let w = data.w(i);
            for a in 0..p + r {
                if z[a] == 0.0 {
                    continue;
                }
                for b in 0..p + r {
                    zwz[a][b] += w * z[a] * z[b];
                }
            }
        }
        let (m, swept) = sweep_in_order(&zwz, &(0..p).collect::<Vec<_>>(), eps);
        let ginv = (0..p)
            .map(|i| {
                (0..p)
                    .map(|j| if swept[i] && swept[j] { m[i][j] } else { 0.0 })
                    .collect()
            })
            .collect();
        let b = (0..p)
            .map(|i| {
                (0..r)
                    .map(|j| if swept[i] { m[p + j][i] } else { 0.0 })
                    .collect()
            })
            .collect();
        let s = (0..r)
            .map(|i| (0..r).map(|j| m[p + i][p + j]).collect())
            .collect();
        let rank = swept.iter().take(p).filter(|&&s| s).count();
        Fit {
            effects,
            p,
            r,
            x,
            zwz,
            ginv,
            b,
            s,
            redundant: swept.iter().take(p).map(|s| !s).collect(),
            rank,
            n: data.n(),
            eps,
        }
    }

    fn df_error(&self) -> f64 {
        self.n - self.rank as f64
    }

    /// The error SSCP after sweeping the columns `order`, with the number of columns swept.
    pub(crate) fn error_after(&self, order: &[usize]) -> (Matrix, usize) {
        let (m, swept) = sweep_in_order(&self.zwz, order, self.eps);
        let p = self.p;
        let s = (0..self.r)
            .map(|i| (0..self.r).map(|j| m[p + i][p + j]).collect())
            .collect();
        (s, swept.iter().filter(|&&s| s).count())
    }

    pub(crate) fn h(&self) -> Matrix {
        let xwx: Matrix = (0..self.p)
            .map(|i| self.zwz[i][..self.p].to_vec())
            .collect();
        multiply(&self.ginv, &xwx)
    }

    /// The hypothesis SSCP of L B M = K (M the identity when None) and its degrees of freedom
    /// (the rank of L G L').
    pub(crate) fn hypothesis(
        &self,
        l: &Matrix,
        m: Option<&Matrix>,
        k: Option<&Matrix>,
    ) -> (Matrix, usize) {
        let b = m.map_or_else(|| self.b.clone(), |m| multiply(&self.b, m));
        let columns = m.map_or(self.r, |m| m[0].len());
        if l.is_empty() {
            return (vec![vec![0.0; columns]; columns], 0);
        }
        let mut lb = multiply(l, &b);
        if let Some(k) = k {
            for (row, krow) in lb.iter_mut().zip(k) {
                for (v, kv) in row.iter_mut().zip(krow) {
                    *v -= kv;
                }
            }
        }
        let lgl = multiply(&multiply(l, &self.ginv), &transpose(l));
        let (inv, rank) = ginverse(&lgl, 1e-10);
        let ssh = multiply(&multiply(&transpose(&lb), &inv), &lb);
        (ssh, rank)
    }

    /// Whether l is estimable: l' H = l'.
    pub(crate) fn estimable(&self, l: &[f64], h: &Matrix) -> bool {
        let scale = l.iter().fold(1.0_f64, |m, v| m.max(v.abs()));
        (0..self.p).all(|j| {
            let lh: f64 = (0..self.p).map(|i| l[i] * h[i][j]).sum();
            (lh - l[j]).abs() <= 1e-6 * scale
        })
    }

    /// The hypothesis matrix of Type I for effect `e`: its rows of X'WX after sweeping the
    /// columns of the effects before it, with those columns set to 0. The row of a redundant
    /// column is a combination of the rows before it and is left out.
    pub(crate) fn type1(&self, e: usize) -> Matrix {
        let before: Vec<usize> = self.effects[..e].iter().flat_map(Effect::range).collect();
        let (m, _) = sweep_in_order(&self.zwz, &before, self.eps);
        self.effects[e]
            .range()
            .filter(|&r| !self.redundant[r])
            .map(|r| {
                (0..self.p)
                    .map(|c| if before.contains(&c) { 0.0 } else { m[r][c] })
                    .collect()
            })
            .collect()
    }

    /// The hypothesis matrix of Type III (or IV) for effect `e`.
    pub(crate) fn type34(&self, e: usize, type4: bool) -> Matrix {
        let p = self.p;
        let h = self.h();
        let tol: Vec<f64> = (0..p)
            .map(|c| 1e-8 * (0..p).fold(1.0_f64, |m, r| m.max(h[r][c].abs())))
            .collect();
        let mut rows = h;
        let mut alive = vec![true; p];
        let eliminate = |rows: &mut Matrix, r: usize, c: usize| {
            let pivot = rows[r][c];
            for v in rows[r].iter_mut() {
                *v /= pivot;
            }
            let pivot_row = rows[r].clone();
            for (r2, row) in rows.iter_mut().enumerate() {
                if r2 != r && row[c] != 0.0 {
                    let factor = row[c];
                    for (v, pv) in row.iter_mut().zip(&pivot_row) {
                        *v -= factor * pv;
                    }
                }
            }
        };
        let effect = &self.effects[e];
        for (j, other) in self.effects.iter().enumerate() {
            if j == e || other.contains(effect) {
                continue;
            }
            for c in other.range() {
                if let Some(r) = (0..p).find(|&r| alive[r] && rows[r][c].abs() > tol[c]) {
                    eliminate(&mut rows, r, c);
                    rows[r] = vec![0.0; p];
                    alive[r] = false;
                }
            }
        }
        let mut used = vec![false; p];
        for c in effect.range() {
            if let Some(r) = (0..p).find(|&r| alive[r] && !used[r] && rows[r][c].abs() > tol[c]) {
                eliminate(&mut rows, r, c);
                used[r] = true;
            }
        }
        let small = |v: f64| v.abs() <= 1e-9;
        let rows: Matrix = rows
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|v| if small(v) { 0.0 } else { v })
                    .collect::<Vec<f64>>()
            })
            .filter(|row| row.iter().any(|v| *v != 0.0))
            .collect();
        let containing: Vec<usize> = (0..self.effects.len())
            .filter(|&j| self.effects[j].contains(effect))
            .collect();
        if containing.is_empty() {
            return rows;
        }
        let (g1, g0): (Matrix, Matrix) = rows
            .into_iter()
            .partition(|row| effect.range().any(|c| row[c] != 0.0));
        if type4 {
            return g1
                .into_iter()
                .map(|row| self.distribute(e, &containing, row))
                .collect();
        }
        if g0.is_empty() {
            return g1;
        }
        // G1 orthogonal to G0: G1 - G1 G0' (G0 G0')^- G0
        let g0t = transpose(&g0);
        let (inv, _) = ginverse(&multiply(&g0, &g0t), 1e-10);
        let projection = multiply(&multiply(&g0t, &inv), &g0);
        let correction = multiply(&g1, &projection);
        g1.iter()
            .zip(&correction)
            .map(|(a, b)| a.iter().zip(b).map(|(x, y)| x - y).collect())
            .collect()
    }

    /// Type IV: the coefficients of effect `e` in `row`, distributed equally over the cells of
    /// each containing effect at the levels of its other factors where every level of `e` with
    /// a coefficient has a cell.
    fn distribute(&self, e: usize, containing: &[usize], mut row: Vec<f64>) -> Vec<f64> {
        let effect = &self.effects[e];
        for &j in containing {
            let outer = &self.effects[j];
            // for every column of the outer effect: the column of `e` and the other levels
            let split: Vec<(Option<usize>, Vec<usize>)> = outer
                .cells
                .iter()
                .map(|cell| {
                    let inner: Vec<usize> = effect
                        .factors
                        .iter()
                        .map(|f| {
                            cell[outer
                                .factors
                                .iter()
                                .position(|g| g == f)
                                .expect("contained")]
                        })
                        .collect();
                    let rest: Vec<usize> = outer
                        .factors
                        .iter()
                        .enumerate()
                        .filter(|(_, f)| !effect.factors.contains(f))
                        .map(|(k, _)| cell[k])
                        .collect();
                    (effect.cells.iter().position(|c| *c == inner), rest)
                })
                .collect();
            let nonzero: Vec<usize> = effect
                .range()
                .filter(|&c| row[c] != 0.0)
                .map(|c| c - effect.start)
                .collect();
            let mut others: Vec<Vec<usize>> = split.iter().map(|(_, o)| o.clone()).collect();
            others.sort();
            others.dedup();
            let common: Vec<&Vec<usize>> = others
                .iter()
                .filter(|o| {
                    nonzero
                        .iter()
                        .all(|&l| split.iter().any(|(c, r)| *c == Some(l) && r == *o))
                })
                .collect();
            for (k, (c, rest)) in split.iter().enumerate() {
                let column = outer.start + k;
                row[column] = match c {
                    Some(l) if row[effect.start + l] != 0.0 && common.contains(&rest) => {
                        row[effect.start + l] / common.len() as f64
                    }
                    _ => 0.0,
                };
            }
        }
        row
    }
}

/// Sweeps the pivots of `order` in turn, skipping those that fall to `eps` of their original
/// diagonal; returns the matrix and which pivots were swept.
pub(crate) fn sweep_in_order(zwz: &Matrix, order: &[usize], eps: f64) -> (Matrix, Vec<bool>) {
    let mut m = zwz.clone();
    let mut swept = vec![false; zwz.len()];
    for &k in order {
        let original = zwz[k][k];
        if original > 0.0 && m[k][k] > eps * original {
            sweep(&mut m, k);
            swept[k] = true;
        }
    }
    (m, swept)
}

/// One source of a univariate test.
struct Source {
    label: String,
    ss: f64,
    df: f64,
    tested: bool,
}

struct Flags {
    etasq: bool,
    opower: bool,
    alpha: f64,
}

fn power_f(f: f64, df1: f64, df2: f64, alpha: f64) -> (f64, f64) {
    let lambda = f * df1;
    let critical = f_quantile(alpha, df1, df2);
    (lambda, 1.0 - noncentral_f_cdf(critical, df1, df2, lambda))
}

/// The columns and rows of an F test table: SS, df, MS, F, Sig. and the optional statistics.
fn f_table(
    title: &str,
    sources: &[Source],
    sse: f64,
    dfe: f64,
    flags: &Flags,
    first_header: &str,
    caption: String,
) -> Table {
    let mut columns = vec![
        first_header.to_string(),
        "Sum of Squares".into(),
        "df".into(),
        "Mean Square".into(),
        "F".into(),
        "Sig.".into(),
    ];
    if flags.etasq {
        columns.push("Partial Eta Squared".into());
    }
    if flags.opower {
        columns.push("Noncent. Parameter".into());
        columns.push("Observed Power".into());
    }
    let mse = sse / dfe;
    let rows = sources
        .iter()
        .map(|s| {
            let ms = s.ss / s.df;
            let f = ms / mse;
            let mut row = vec![
                text(s.label.clone()),
                number(Some(s.ss)),
                number(Some(s.df)),
                // the totals have no mean square
                number((s.df > 0.0 && !s.label.ends_with("Total")).then_some(ms)),
            ];
            let tested = s.tested && s.df > 0.0 && dfe > 0.0;
            row.push(number(tested.then_some(f)));
            row.push(number(tested.then(|| f_p(f, s.df, dfe))));
            if flags.etasq {
                row.push(number(tested.then(|| s.ss / (s.ss + sse))));
            }
            if flags.opower {
                let power = tested.then(|| power_f(f, s.df, dfe, flags.alpha));
                row.push(number(power.map(|p| p.0)));
                row.push(number(power.map(|p| p.1)));
            }
            row
        })
        .collect();
    Table {
        title: title.into(),
        row_headers: 1,
        column_groups: vec![Vec::new(); columns.len()],
        columns,
        rows,
        caption: Some(caption),
        ..Table::default()
    }
}

pub(crate) fn adjusted(p: f64, m: f64, adjustment: Adjustment) -> f64 {
    match adjustment {
        Adjustment::Lsd => p,
        Adjustment::Bonferroni => (p * m).min(1.0),
        Adjustment::Sidak => 1.0 - (1.0 - p).powf(m),
    }
}

/// The t value of a two-sided interval at `alpha`, adjusted for m comparisons.
pub(crate) fn adjusted_t(alpha: f64, m: f64, df: f64, adjustment: Adjustment) -> f64 {
    let level = match adjustment {
        Adjustment::Lsd => alpha,
        Adjustment::Bonferroni => alpha / m,
        Adjustment::Sidak => 1.0 - (1.0 - alpha).powf(1.0 / m),
    };
    t_quantile(level / 2.0, df)
}

pub(crate) fn adjustment_name(a: Adjustment) -> &'static str {
    match a {
        Adjustment::Lsd => "Least Significant Difference (equivalent to no adjustments)",
        Adjustment::Bonferroni => "Bonferroni",
        Adjustment::Sidak => "Sidak",
    }
}

pub(crate) fn percent_title(alpha: f64) -> String {
    let p = 100.0 * (1.0 - alpha);
    if (p - p.round()).abs() < 1e-9 {
        format!("{p:.0}%")
    } else {
        format!("{}%", (p * 1e6).round() / 1e6)
    }
}

/// Every combination of one level from each list, the first outermost.
pub(crate) fn combinations(sizes: &[usize]) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = vec![Vec::new()];
    for &n in sizes {
        out = out
            .into_iter()
            .flat_map(|prefix| {
                (0..n).map(move |l| {
                    let mut next = prefix.clone();
                    next.push(l);
                    next
                })
            })
            .collect();
    }
    out
}

/// The design of a model from its terms: each factor and covariate named, factors not repeated
/// within a term, covariates not nested.
pub(crate) fn effects_of(
    terms: &[Term],
    factor_names: &[String],
    covariate_names: &[String],
) -> Result<Vec<Effect>, Error> {
    let mut out: Vec<Effect> = Vec::new();
    for term in terms {
        let mut parts = Vec::new();
        for name in &term.variables {
            if let Some(f) = factor_names
                .iter()
                .position(|n| n.eq_ignore_ascii_case(name))
            {
                if parts.contains(&Part::Factor(f)) {
                    return Err(Error(format!(
                        "{} names the factor {name} twice",
                        term.label
                    )));
                }
                parts.push(Part::Factor(f));
            } else if let Some(c) = covariate_names
                .iter()
                .position(|n| n.eq_ignore_ascii_case(name))
            {
                if term.label.contains('(') && term.label.contains(name.as_str()) {
                    let nested_cov = term
                        .label
                        .find('(')
                        .is_some_and(|at| term.label[at..].contains(name.as_str()));
                    if nested_cov {
                        return Err(Error(format!(
                            "{}: a covariate cannot be nested",
                            term.label
                        )));
                    }
                }
                parts.push(Part::Covariate(c));
            } else {
                return Err(Error(format!(
                    "{name} in the design is neither a factor nor a covariate of the model"
                )));
            }
        }
        let factors: Vec<usize> = parts
            .iter()
            .filter_map(|p| match p {
                Part::Factor(f) => Some(*f),
                Part::Covariate(_) => None,
            })
            .collect();
        let mut covariates: Vec<usize> = parts
            .iter()
            .filter_map(|p| match p {
                Part::Covariate(c) => Some(*c),
                Part::Factor(_) => None,
            })
            .collect();
        covariates.sort_unstable();
        let mut set = factors.clone();
        set.sort_unstable();
        let duplicate = out.iter().any(|e| {
            let mut other = e.factors.clone();
            other.sort_unstable();
            other == set && e.covariates == covariates
        });
        if duplicate {
            continue;
        }
        out.push(Effect {
            label: term.label.clone(),
            parts,
            factors,
            covariates,
            cells: Vec::new(),
            start: 0,
        });
    }
    // the intercept first
    out.sort_by_key(|e| !e.is_intercept());
    Ok(out)
}

pub(crate) fn intercept_term() -> Term {
    Term {
        variables: Vec::new(),
        label: "Intercept".into(),
    }
}

/// The default design: the intercept, the covariates, the full factorial of the factors.
pub(crate) fn default_terms(
    factors: &[String],
    covariates: &[String],
    intercept: bool,
) -> Vec<Term> {
    let mut terms = Vec::new();
    if intercept {
        terms.push(intercept_term());
    }
    for c in covariates {
        terms.push(Term {
            variables: vec![c.clone()],
            label: c.clone(),
        });
    }
    let k = factors.len();
    for size in 1..=k {
        let mut subsets: Vec<Vec<usize>> = Vec::new();
        let mut pick = |set: Vec<usize>| subsets.push(set);
        fn choose(
            start: usize,
            k: usize,
            size: usize,
            current: &mut Vec<usize>,
            out: &mut dyn FnMut(Vec<usize>),
        ) {
            if current.len() == size {
                out(current.clone());
                return;
            }
            for i in start..k {
                current.push(i);
                choose(i + 1, k, size, current, out);
                current.pop();
            }
        }
        choose(0, k, size, &mut Vec::new(), &mut pick);
        for set in subsets {
            let names: Vec<String> = set.iter().map(|&i| factors[i].clone()).collect();
            terms.push(Term {
                label: names.join(" * "),
                variables: names,
            });
        }
    }
    terms
}

pub(crate) fn key_value_text(variable: &Variable, key: &Key) -> String {
    match key {
        Key::Number(n) => category_text(*n, variable.format()),
        Key::Text(t) => t.trim_end().to_string(),
    }
}

impl Engine {
    pub(crate) fn glm(&mut self, command: &Glm) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let dependents = self.expand(&command.dependents)?;
        let factor_names = self.expand(&command.factors)?;
        let covariate_names = self.expand(&command.covariates)?;
        let name = command.command;
        if name == "UNIANOVA" && dependents.len() > 1 {
            return Err(Error(
                "UNIANOVA takes one dependent variable; GLM analyzes several".into(),
            ));
        }
        let mut seen: Vec<String> = Vec::new();
        for n in dependents
            .iter()
            .chain(&factor_names)
            .chain(&covariate_names)
        {
            if seen.iter().any(|s| s.eq_ignore_ascii_case(n)) {
                return Err(Error(format!(
                    "{n} appears more than once among the dependent variables, factors and covariates"
                )));
            }
            seen.push(n.clone());
        }
        let dep_vars: Vec<Variable> = dependents
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let factor_vars: Vec<Variable> = factor_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let cov_vars: Vec<Variable> = covariate_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        for v in dep_vars.iter().chain(&cov_vars) {
            if v.is_string() {
                return Err(Error(format!(
                    "{name} needs numeric dependent variables and covariates, but {} is a string variable",
                    v.name()
                )));
            }
        }
        let cases = data.case_count();
        let weights = case_weights(data)?;
        let regwgt = match &command.regwgt {
            Some(n) => {
                let v = data.variable(n)?;
                if v.is_string() {
                    return Err(Error(format!(
                        "REGWGT needs a numeric variable, but {n} is a string variable"
                    )));
                }
                Some(v)
            }
            None => None,
        };
        let numeric = |v: &Variable, k: usize| -> Option<f64> {
            if !crate::means::is_valid(v, k, command.include) {
                return None;
            }
            match v.values() {
                Values::Numeric(values) => values.get(k),
                Values::String(_) => None,
            }
        };
        let factor_keys: Vec<Vec<Option<Key>>> = factor_vars
            .iter()
            .map(|v| categories(v, command.include))
            .collect();
        let mut rows: Vec<usize> = Vec::new();
        for k in 0..cases {
            let f = weights.as_ref().map_or(1.0, |w| w[k].round());
            let g = match &regwgt {
                Some(v) => numeric(v, k).unwrap_or(0.0),
                None => 1.0,
            };
            let valid = f > 0.0
                && g > 0.0
                && dep_vars.iter().all(|v| numeric(v, k).is_some())
                && cov_vars.iter().all(|v| numeric(v, k).is_some())
                && factor_keys.iter().all(|col| col[k].is_some());
            if valid {
                rows.push(k);
            }
        }
        if rows.is_empty() {
            return Err(Error(format!(
                "{name}: no case has valid values on every variable"
            )));
        }
        let mut levels: Vec<Vec<Key>> = Vec::new();
        for col in &factor_keys {
            let mut found: Vec<Key> = Vec::new();
            for &k in &rows {
                let key = col[k].as_ref().expect("valid");
                if !found.iter().any(|l| l.compare(key).is_eq()) {
                    found.push(match key {
                        Key::Number(n) => Key::Number(*n),
                        Key::Text(t) => Key::Text(t.clone()),
                    });
                }
            }
            found.sort_by(|a, b| a.compare(b));
            levels.push(found);
        }
        // the levels as tables show them (SET TNUMBERS)
        let level_texts: Vec<Vec<String>> = factor_vars
            .iter()
            .zip(&levels)
            .map(|(v, ls)| ls.iter().map(|k| shown.value(v, k)).collect())
            .collect();
        let labels = Shown::labels(shown.currencies.clone());
        let d = Data {
            factors: factor_names.clone(),
            factor_texts: factor_vars.iter().map(|v| shown.variable(v)).collect(),
            values: factor_vars
                .iter()
                .zip(&levels)
                .map(|(v, ls)| ls.iter().map(|k| key_value_text(v, k)).collect())
                .collect(),
            labels: factor_vars
                .iter()
                .zip(&levels)
                .map(|(v, ls)| ls.iter().map(|k| labels.value(v, k)).collect())
                .collect(),
            // the Value Label column, which SET TNUMBERS=VALUES leaves out
            labelled: factor_vars.iter().any(|v| !v.value_labels().is_empty())
                && shown.values != Show::Plain,
            covariates: covariate_names.clone(),
            level: rows
                .iter()
                .map(|&k| {
                    factor_keys
                        .iter()
                        .zip(&levels)
                        .map(|(col, ls)| {
                            let key = col[k].as_ref().expect("valid");
                            ls.iter()
                                .position(|l| l.compare(key).is_eq())
                                .expect("a level")
                        })
                        .collect()
                })
                .collect(),
            cov: rows
                .iter()
                .map(|&k| {
                    cov_vars
                        .iter()
                        .map(|v| numeric(v, k).expect("valid"))
                        .collect()
                })
                .collect(),
            y: rows
                .iter()
                .map(|&k| {
                    dep_vars
                        .iter()
                        .map(|v| numeric(v, k).expect("valid"))
                        .collect()
                })
                .collect(),
            f: rows
                .iter()
                .map(|&k| weights.as_ref().map_or(1.0, |w| w[k].round()))
                .collect(),
            g: rows
                .iter()
                .map(|&k| {
                    regwgt
                        .as_ref()
                        .map_or(1.0, |v| numeric(v, k).expect("valid"))
                })
                .collect(),
            case: rows,
        };
        let terms = match &command.design {
            Some(terms) => {
                let mut terms = terms.clone();
                if command.intercept && !terms.iter().any(|t| t.variables.is_empty()) {
                    terms.insert(0, intercept_term());
                }
                terms
            }
            None => default_terms(&factor_names, &covariate_names, command.intercept),
        };
        let mut effects = effects_of(&terms, &factor_names, &covariate_names)?;
        // an effect that is not nested shows its variables as tables do; a nested one as written
        let covariate_texts: Vec<String> = cov_vars.iter().map(|v| shown.variable(v)).collect();
        for e in effects
            .iter_mut()
            .filter(|e| !e.is_intercept() && !e.label.contains('('))
        {
            let parts = e.parts.iter().map(|p| match p {
                Part::Factor(f) => d.factor_texts[*f].clone(),
                Part::Covariate(c) => covariate_texts[*c].clone(),
            });
            e.label = parts.collect::<Vec<_>>().join(" * ");
        }
        if effects.is_empty() {
            return Err(Error(format!("{name}: the design has no effects")));
        }
        let mut warnings = Vec::new();
        let label_of = |f: usize, l: usize| level_texts[f][l].clone();
        let dependent_texts: Vec<String> = dep_vars.iter().map(|v| shown.variable(v)).collect();
        let mut tables = Vec::new();
        if !command.wsfactors.is_empty() {
            if !command.plots.is_empty() {
                warnings.push(format!(
                    "{name}: PLOT with WSFACTOR is not supported yet; no chart is made"
                ));
            }
            tables.extend(self.repeated_measures(
                command,
                &d,
                effects,
                &dependent_texts,
                &mut warnings,
                &label_of,
            )?);
        } else if dependents.len() > 1 {
            tables.extend(self.multivariate_model(
                command,
                &d,
                effects,
                &dependents,
                &dependent_texts,
                cases,
                &label_of,
                &mut warnings,
            )?);
        } else {
            let y: Vec<Vec<f64>> = d.y.clone();
            let fit = Fit::new(&d, effects, &y, command.eps);
            tables.extend(univariate(
                command,
                &d,
                &fit,
                &dependent_texts[0],
                &label_of,
                &mut warnings,
            )?);
            let charts = plots(command, &d, &fit, &dependent_texts[0], &label_of)?;
            self.charts.extend(charts);
            if !command.save.is_empty() {
                self.save_glm(command, &d, &fit, &dependents[0], cases)?;
            }
        }
        // the dependents of the multivariate model repeat the same warnings
        let mut seen = std::collections::HashSet::new();
        warnings.retain(|w| seen.insert(w.clone()));
        for w in warnings {
            self.warn(w);
        }
        Ok(tables)
    }

    fn save_glm(
        &mut self,
        command: &Glm,
        d: &Data,
        fit: &Fit,
        dependent: &str,
        cases: usize,
    ) -> Result<(), Error> {
        if command.regwgt.is_none()
            && command
                .save
                .iter()
                .any(|(t, _)| matches!(t, GlmTemp::Wpred | GlmTemp::Wresid))
        {
            return Err(Error("SAVE WPRED and WRESID need a REGWGT variable".into()));
        }
        let mse = fit.s[0][0] / fit.df_error();
        let values: Vec<[f64; 10]> = (0..d.rows())
            .map(|i| {
                let x = &fit.x[i];
                let pred: f64 = x.iter().zip(&fit.b).map(|(x, b)| x * b[0]).sum();
                let pi: f64 = (0..fit.p)
                    .map(|a| {
                        (0..fit.p)
                            .map(|b| x[a] * fit.ginv[a][b] * x[b])
                            .sum::<f64>()
                    })
                    .sum();
                let w = d.g[i];
                let e = d.y[i][0] - pred;
                let se_e = (mse * (1.0 / w - pi)).sqrt();
                let sresid = e / se_e;
                let fine = w * pi < 1.0;
                [
                    pred,
                    w.sqrt() * pred,
                    e,
                    w.sqrt() * e,
                    if fine { e / (1.0 / w - pi) } else { f64::NAN },
                    e / (mse / w).sqrt(),
                    if fine { sresid } else { f64::NAN },
                    (mse * pi).sqrt(),
                    if fine {
                        sresid * sresid * (pi / (1.0 / w - pi)) / fit.rank as f64
                    } else {
                        f64::NAN
                    },
                    w * pi,
                ]
            })
            .collect();
        let temps = [
            (GlmTemp::Pred, "PRE", "Predicted Value"),
            (GlmTemp::Wpred, "WPR", "Weighted Predicted Value"),
            (GlmTemp::Resid, "RES", "Residual"),
            (GlmTemp::Wresid, "WRE", "Weighted Residual"),
            (GlmTemp::Dresid, "DRE", "Deleted Residual"),
            (GlmTemp::Zresid, "ZRE", "Standardized Residual"),
            (GlmTemp::Sresid, "SRE", "Studentized Residual"),
            (GlmTemp::Sepred, "SEP", "Standard Error of Predicted Value"),
            (GlmTemp::Cook, "COO", "Cook's Distance"),
            (GlmTemp::Lever, "LEV", "Uncentered Leverage Value"),
        ];
        let name = command.command;
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            for (temp, given) in &command.save {
                let at = temps.iter().position(|t| t.0 == *temp).expect("listed");
                let (_, root, what) = temps[at];
                let var = match given {
                    Some(n) => n.clone(),
                    None => {
                        let mut n = 1;
                        while data.variable_index(&format!("{root}_{n}")).is_some() {
                            n += 1;
                        }
                        format!("{root}_{n}")
                    }
                };
                if data.variable_index(&var).is_some() {
                    return Err(Error(format!(
                        "{name} /SAVE: variable {var} already exists"
                    )));
                }
                let mut column = vec![None; cases];
                for (i, &k) in d.case.iter().enumerate() {
                    let v = values[i][at];
                    column[k] = v.is_finite().then_some(v);
                }
                data.set_numeric_values(&var, column)?;
                data.set_variable_label(&var, Some(&format!("{what} for {dependent}")))?;
                data.set_format(&var, "F11.5")?;
                data.set_measure(&var, Measure::Scale)?;
            }
            Ok(())
        })
    }
}

/// The sums of squares of every effect but the intercept (and of the intercept) by the type of
/// METHOD, each as an r x r matrix with its df.
fn effect_sscp(fit: &Fit, sstype: u8) -> Vec<(Matrix, usize)> {
    let n_eff = fit.effects.len();
    match sstype {
        1 => {
            let mut order = Vec::new();
            let (mut before, mut swept_before) = fit.error_after(&[]);
            let mut out = Vec::new();
            for e in &fit.effects {
                order.extend(e.range());
                let (after, swept) = fit.error_after(&order);
                let diff = before
                    .iter()
                    .zip(&after)
                    .map(|(a, b)| a.iter().zip(b).map(|(x, y)| x - y).collect())
                    .collect();
                out.push((diff, swept - swept_before));
                before = after;
                swept_before = swept;
            }
            out
        }
        2 => (0..n_eff)
            .map(|e| {
                let effect = &fit.effects[e];
                let x1: Vec<usize> = fit
                    .effects
                    .iter()
                    .enumerate()
                    .filter(|(j, other)| *j != e && !other.contains(effect))
                    .flat_map(|(_, other)| other.range())
                    .collect();
                let (before, swept_before) = fit.error_after(&x1);
                let mut order = x1.clone();
                order.extend(effect.range());
                let (after, swept) = fit.error_after(&order);
                let diff = before
                    .iter()
                    .zip(&after)
                    .map(|(a, b)| a.iter().zip(b).map(|(x, y)| x - y).collect())
                    .collect();
                (diff, swept - swept_before)
            })
            .collect(),
        _ => (0..n_eff)
            .map(|e| {
                let l = fit.type34(e, sstype == 4);
                fit.hypothesis(&l, None, None)
            })
            .collect(),
    }
}

fn flags_of(command: &Glm) -> Flags {
    Flags {
        etasq: command.print.etasq,
        opower: command.print.opower,
        alpha: command.alpha,
    }
}

fn power_note(flags: &Flags) -> String {
    if flags.opower {
        format!(" Computed using alpha = {}.", flags.alpha)
    } else {
        String::new()
    }
}

/// The tables of the univariate model.
fn univariate(
    command: &Glm,
    d: &Data,
    fit: &Fit,
    dependent: &str,
    label_of: &dyn Fn(usize, usize) -> String,
    warnings: &mut Vec<String>,
) -> Result<Vec<Table>, Error> {
    let mut out = Vec::new();
    let flags = flags_of(command);
    out.push(between_factors(d));
    if command.print.descriptive {
        out.push(descriptive_table(d, 0, dependent, label_of));
    }
    if command.print.homogeneity {
        match levene(d, fit) {
            Some(t) => out.push(t),
            None => warnings.push("Levene's test needs at least one factor".into()),
        }
    }
    let (mut table, r2) = between_subjects(command, d, fit, &flags);
    table.caption = Some(format!(
        "Dependent Variable: {dependent}. Type {} sums of squares. {r2}.{}",
        roman(command.sstype),
        power_note(&flags)
    ));
    out.push(table);
    if command.print.parameter {
        let dfe = fit.df_error();
        out.push(parameter_table(
            command,
            d,
            fit,
            dependent,
            &flags,
            fit.s[0][0] / dfe,
            dfe,
        ));
    }
    if command.print.lof {
        out.push(lack_of_fit(d, fit, dependent, &flags));
    }
    for (what, given) in [
        ("MMATRIX", command.mmatrix.is_some()),
        ("PRINT RSSCP", command.print.rsscp),
        ("PRINT TEST(SSCP)", command.print.sscp),
        ("PRINT TEST(MMATRIX)", command.print.mmatrix),
    ] {
        if given {
            warnings.push(format!(
                "{what} applies only to GLM with several dependent variables and is ignored"
            ));
        }
    }
    out.extend(custom_hypotheses(
        command,
        d,
        fit,
        None,
        &[dependent.to_string()],
        &flags,
        warnings,
    )?);
    out.extend(means_and_posthoc(
        command, d, fit, dependent, &flags, label_of, warnings,
    )?);
    Ok(out)
}

/// Tests of Between-Subjects Effects of one dependent variable, without a caption, and its R
/// squared as text.
fn between_subjects(command: &Glm, d: &Data, fit: &Fit, flags: &Flags) -> (Table, String) {
    let sse = fit.s[0][0];
    let dfe = fit.df_error();
    let has_intercept = fit.effects.iter().any(Effect::is_intercept);
    let w_total: f64 = (0..d.rows()).map(|i| d.w(i)).sum();
    let mean = (0..d.rows()).map(|i| d.w(i) * d.y[i][0]).sum::<f64>() / w_total;
    let total_ss: f64 = (0..d.rows()).map(|i| d.w(i) * d.y[i][0] * d.y[i][0]).sum();
    let corrected_ss: f64 = (0..d.rows())
        .map(|i| d.w(i) * (d.y[i][0] - mean).powi(2))
        .sum();
    let sscp = effect_sscp(fit, command.sstype);
    let mut sources = Vec::new();
    let model_ss = if has_intercept {
        corrected_ss - sse
    } else {
        total_ss - sse
    };
    let model_df = if has_intercept {
        fit.rank as f64 - 1.0
    } else {
        fit.rank as f64
    };
    sources.push(Source {
        label: if has_intercept {
            "Corrected Model".into()
        } else {
            "Model".into()
        },
        ss: model_ss,
        df: model_df,
        tested: true,
    });
    for (e, (ss, df)) in fit.effects.iter().zip(&sscp) {
        sources.push(Source {
            label: e.label.clone(),
            ss: ss[0][0],
            df: *df as f64,
            tested: true,
        });
    }
    sources.push(Source {
        label: "Error".into(),
        ss: sse,
        df: dfe,
        tested: false,
    });
    sources.push(Source {
        label: "Total".into(),
        ss: total_ss,
        df: fit.n,
        tested: false,
    });
    if has_intercept {
        sources.push(Source {
            label: "Corrected Total".into(),
            ss: corrected_ss,
            df: fit.n - 1.0,
            tested: false,
        });
    }
    let (r2, adj) = if has_intercept {
        let r2 = model_ss / corrected_ss;
        (r2, 1.0 - (1.0 - r2) * (fit.n - 1.0) / dfe)
    } else {
        let r2 = model_ss / total_ss;
        (r2, 1.0 - (1.0 - r2) * fit.n / dfe)
    };
    let mut table = f_table(
        "Tests of Between-Subjects Effects",
        &sources,
        sse,
        dfe,
        flags,
        "Source",
        String::new(),
    );
    table.columns[1] = format!("Type {} Sum of Squares", roman(command.sstype));
    let r2 = format!(
        "R Squared = {} (Adjusted R Squared = {})",
        round3(r2),
        round3(adj)
    );
    (table, r2)
}

type NamedRow = (String, Vec<f64>);

/// The custom hypothesis tests of CONTRAST and LMATRIX (with KMATRIX), and of the intercept
/// when only KMATRIX or MMATRIX is given. `given_m` is the M matrix of MMATRIX with the names
/// of its columns; CONTRAST always uses the identity.
fn custom_hypotheses(
    command: &Glm,
    d: &Data,
    fit: &Fit,
    given_m: Option<&(Matrix, Vec<String>)>,
    dependents: &[String],
    flags: &Flags,
    warnings: &mut Vec<String>,
) -> Result<Vec<Table>, Error> {
    let mut out = Vec::new();
    let h = fit.h();
    let identity: Matrix = (0..fit.r)
        .map(|i| (0..fit.r).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect();
    for (factor, kind) in &command.contrasts {
        let Some(f) = d
            .factors
            .iter()
            .position(|v| v.eq_ignore_ascii_case(factor))
        else {
            return Err(Error(format!(
                "CONTRAST names {factor}, which is not a factor"
            )));
        };
        let k = d.values[f].len();
        let rows = contrast_rows(kind, k).map_err(|e| Error(format!("CONTRAST({factor}): {e}")))?;
        let level_l: Vec<Vec<f64>> = (0..k).map(|l| emmeans_l(d, fit, &[(f, l)], &[])).collect();
        let l: Vec<(String, Vec<f64>)> = rows
            .into_iter()
            .map(|(name, c)| {
                let row = (0..fit.p)
                    .map(|j| (0..k).map(|lv| c[lv] * level_l[lv][j]).sum())
                    .collect();
                (name, row)
            })
            .collect();
        let kind_name = contrast_name(kind);
        out.extend(custom_tests(
            fit,
            &h,
            &l,
            (&identity, dependents, false),
            None,
            &format!("{factor} {kind_name} Contrast"),
            flags,
            warnings,
        ));
    }
    let (m, names) = given_m.map_or((&identity, dependents), |(m, n)| (m, n.as_slice()));
    // each L matrix: its title and its named rows
    let mut matrices: Vec<(String, Vec<NamedRow>)> = Vec::new();
    for (n, lm) in command.lmatrix.iter().enumerate() {
        let mut l = Vec::new();
        for (r, row) in lm.rows.iter().enumerate() {
            l.push((format!("L{}", r + 1), l_row(d, fit, row, "LMATRIX")?));
        }
        let title = lm
            .label
            .clone()
            .unwrap_or_else(|| format!("LMATRIX {}", n + 1));
        matrices.push((title, l));
    }
    if command.lmatrix.is_empty() && (command.kmatrix.is_some() || given_m.is_some()) {
        // the L matrix of the intercept's Type III test
        match fit.effects.iter().position(Effect::is_intercept) {
            Some(e) => {
                let l = fit
                    .type34(e, false)
                    .into_iter()
                    .enumerate()
                    .map(|(i, row)| (format!("L{}", i + 1), row))
                    .collect();
                matrices.push(("Intercept".into(), l));
            }
            None => {
                return Err(Error(
                    "KMATRIX or MMATRIX without LMATRIX tests the intercept, which the model leaves out"
                        .into(),
                ));
            }
        }
    }
    let columns = m[0].len();
    for (title, l) in matrices {
        let k: Option<Matrix> = match &command.kmatrix {
            None => None,
            Some(rows) if columns == 1 => {
                let values = rows.concat();
                if values.len() != l.len() {
                    return Err(Error(format!(
                        "KMATRIX has {} rows and the L matrix {}",
                        values.len(),
                        l.len()
                    )));
                }
                Some(values.into_iter().map(|v| vec![v]).collect())
            }
            Some(rows) => {
                if rows.len() != l.len() || rows.iter().any(|r| r.len() != columns) {
                    return Err(Error(format!(
                        "KMATRIX needs a row for each of the {} rows of the L matrix, each with a value for each of the {columns} columns of the M matrix",
                        l.len()
                    )));
                }
                Some(rows.clone())
            }
        };
        out.extend(custom_tests(
            fit,
            &h,
            &l,
            (m, names, given_m.is_some()),
            k.as_ref(),
            &title,
            flags,
            warnings,
        ));
    }
    Ok(out)
}

/// The tables of EMMEANS and POSTHOC for one dependent variable.
fn means_and_posthoc(
    command: &Glm,
    d: &Data,
    fit: &Fit,
    dependent: &str,
    flags: &Flags,
    label_of: &dyn Fn(usize, usize) -> String,
    warnings: &mut Vec<String>,
) -> Result<Vec<Table>, Error> {
    let mut out = Vec::new();
    let h = fit.h();
    let dfe = fit.df_error();
    let mse = fit.s[0][0] / dfe;
    for e in &command.emmeans {
        out.extend(emmeans_tables(
            command, d, fit, &h, e, dependent, flags, label_of,
        )?);
    }
    for (factors, tests) in &command.posthoc {
        for factor in factors {
            let Some(f) = d
                .factors
                .iter()
                .position(|v| v.eq_ignore_ascii_case(factor))
            else {
                return Err(Error(format!(
                    "POSTHOC names {factor}, which is not a factor"
                )));
            };
            let in_design = fit
                .effects
                .iter()
                .any(|e| e.factors == [f] && e.covariates.is_empty());
            if !in_design {
                warnings.push(format!(
                    "POSTHOC: {factor} is not a main effect of the design; it is not tested"
                ));
                continue;
            }
            let mut kept: Vec<Posthoc> = Vec::new();
            for &t in tests {
                let separate = matches!(
                    t,
                    Posthoc::GamesHowell | Posthoc::T2 | Posthoc::T3 | Posthoc::C
                );
                if separate && d.factors.len() > 1 {
                    warnings.push(format!(
                        "POSTHOC: {} needs a model with one factor and is not run",
                        test_keyword(t)
                    ));
                } else {
                    kept.push(t);
                }
            }
            let groups: Vec<Group> = (0..d.values[f].len())
                .map(|l| {
                    let pairs: Vec<(f64, f64)> = (0..d.rows())
                        .filter(|&i| d.level[i][f] == l)
                        .map(|i| (d.y[i][0], d.f[i]))
                        .collect();
                    group(&pairs)
                })
                .collect();
            let labels: Vec<String> = (0..d.values[f].len()).map(|l| label_of(f, l)).collect();
            let (comparison, subsets) = posthoc_tables(
                (dependent, dependent),
                d.factor_texts[f].as_str(),
                &labels,
                &groups,
                &kept,
                mse,
                dfe,
                command.alpha,
                warnings,
            );
            out.extend(comparison);
            out.extend(subsets);
        }
    }
    Ok(out)
}

fn test_keyword(t: Posthoc) -> &'static str {
    match t {
        Posthoc::GamesHowell => "GH",
        Posthoc::T2 => "T2",
        Posthoc::T3 => "T3",
        _ => "C",
    }
}

fn roman(t: u8) -> &'static str {
    ["I", "II", "III", "IV"][usize::from(t.clamp(1, 4)) - 1]
}

fn round3(v: f64) -> String {
    format!("{:.3}", v)
}

fn contrast_name(kind: &Contrast) -> &'static str {
    match kind {
        Contrast::Indicator(_) => "Indicator",
        Contrast::Deviation(_) => "Deviation",
        Contrast::Simple(_) => "Simple",
        Contrast::Difference => "Difference",
        Contrast::Helmert => "Helmert",
        Contrast::Repeated => "Repeated",
        Contrast::Polynomial(_) => "Polynomial",
        Contrast::Special(_) => "Special",
    }
}

fn between_factors(d: &Data) -> Table {
    let mut columns = plain(&["", ""]);
    if d.labelled {
        columns.extend(plain(&["Value Label"]));
    }
    columns.extend(plain(&["N"]));
    let mut rows = Vec::new();
    for (f, name) in d.factor_texts.iter().enumerate() {
        for l in 0..d.values[f].len() {
            let n: f64 = (0..d.rows())
                .filter(|&i| d.level[i][f] == l)
                .map(|i| d.f[i])
                .sum();
            let mut row = vec![
                if l == 0 {
                    text(name.clone())
                } else {
                    Cell::Empty
                },
                text(d.values[f][l].clone()),
            ];
            if d.labelled {
                row.push(if d.labels[f][l] == d.values[f][l] {
                    Cell::Empty
                } else {
                    text(d.labels[f][l].clone())
                });
            }
            row.push(Cell::Number(n));
            rows.push(row);
        }
    }
    table("Between-Subjects Factors", 2, columns, rows, None)
}

fn descriptive_table(
    d: &Data,
    col: usize,
    dependent: &str,
    label_of: &dyn Fn(usize, usize) -> String,
) -> Table {
    let k = d.factors.len();
    let mut columns: Vec<(String, String)> = d
        .factor_texts
        .iter()
        .map(|v| (String::new(), v.clone()))
        .collect();
    columns.extend(plain(&["Mean", "Std. Deviation", "N"]));
    // each factor: its levels, then Total (None)
    let sizes: Vec<usize> = d.values.iter().map(|l| l.len() + 1).collect();
    let mut rows = Vec::new();
    let mut previous: Vec<Option<usize>> = Vec::new();
    for combo in combinations(&sizes) {
        let choice: Vec<Option<usize>> = combo
            .iter()
            .zip(&d.values)
            .map(|(&c, l)| (c < l.len()).then_some(c))
            .collect();
        let members: Vec<usize> = (0..d.rows())
            .filter(|&i| {
                choice
                    .iter()
                    .enumerate()
                    .all(|(f, c)| c.is_none_or(|l| d.level[i][f] == l))
            })
            .collect();
        let n: f64 = members.iter().map(|&i| d.f[i]).sum();
        if n == 0.0 {
            continue;
        }
        let mean = members.iter().map(|&i| d.f[i] * d.y[i][col]).sum::<f64>() / n;
        let ss: f64 = members
            .iter()
            .map(|&i| d.f[i] * (d.y[i][col] - mean).powi(2))
            .sum();
        let mut row: Vec<Cell> = Vec::new();
        for f in 0..k {
            let same_prefix = previous.len() == k && previous[..=f] == choice[..=f];
            row.push(if same_prefix {
                Cell::Empty
            } else {
                text(match choice[f] {
                    Some(l) => label_of(f, l),
                    None => "Total".into(),
                })
            });
        }
        row.push(number(Some(mean)));
        row.push(number((n > 1.0).then(|| (ss / (n - 1.0)).sqrt())));
        row.push(Cell::Number(n));
        rows.push(row);
        previous = choice;
    }
    if k == 0 {
        columns.insert(0, (String::new(), String::new()));
        for row in &mut rows {
            row.insert(0, text("Total"));
        }
    }
    table(
        "Descriptive Statistics",
        k.max(1),
        columns,
        rows,
        Some(format!("Dependent Variable: {dependent}.")),
    )
}

/// The Levene statistic of absolute deviations `z` in groups, with the adjusted df of the
/// median version.
pub(crate) fn levene_of(groups: &[Vec<(f64, f64)>]) -> (f64, f64, f64, f64) {
    let k = groups.len() as f64;
    let stats: Vec<(f64, f64)> = groups
        .iter()
        .map(|g| {
            let w: f64 = g.iter().map(|p| p.1).sum();
            (w, g.iter().map(|p| p.0 * p.1).sum::<f64>() / w)
        })
        .collect();
    let w: f64 = stats.iter().map(|s| s.0).sum();
    let grand = stats.iter().map(|s| s.0 * s.1).sum::<f64>() / w;
    let between: f64 = stats.iter().map(|s| s.0 * (s.1 - grand).powi(2)).sum();
    let u: Vec<f64> = groups
        .iter()
        .zip(&stats)
        .map(|(g, s)| g.iter().map(|p| p.1 * (p.0 - s.1).powi(2)).sum())
        .collect();
    let within: f64 = u.iter().sum();
    let statistic = (w - k) / (k - 1.0) * between / within;
    let v = within * within
        / u.iter()
            .zip(&stats)
            .map(|(u, s)| u * u / (s.0 - 1.0))
            .sum::<f64>();
    (statistic, k - 1.0, w - k, v)
}

fn levene(d: &Data, fit: &Fit) -> Option<Table> {
    if d.factors.is_empty() {
        return None;
    }
    // the cells of all factors
    let mut cells: Vec<Vec<usize>> = (0..d.rows()).map(|i| d.level[i].clone()).collect();
    cells.sort();
    cells.dedup();
    let cell_of = |i: usize| cells.iter().position(|c| *c == d.level[i]).expect("a cell");
    let residual = |i: usize| {
        let pred: f64 = fit.x[i].iter().zip(&fit.b).map(|(x, b)| x * b[0]).sum();
        d.y[i][0] - pred
    };
    let by_cell = |value: &dyn Fn(usize) -> f64| -> Vec<Vec<(f64, f64)>> {
        let mut groups = vec![Vec::new(); cells.len()];
        for i in 0..d.rows() {
            groups[cell_of(i)].push((value(i), d.f[i]));
        }
        groups
    };
    let mean_based = by_cell(&|i| residual(i).abs());
    let full_factorial = d.covariates.is_empty()
        && fit.effects.iter().filter(|e| !e.is_intercept()).count() == (1 << d.factors.len()) - 1;
    let mut rows = Vec::new();
    let mut push = |name: &str, (statistic, df1, _, _): (f64, f64, f64, f64), df2_shown: f64| {
        rows.push(vec![
            text(name),
            number(Some(statistic)),
            number(Some(df1)),
            number(Some(df2_shown)),
            number(Some(f_p(statistic, df1, df2_shown))),
        ]);
    };
    let l = levene_of(&mean_based);
    push("Based on Mean", l, l.2);
    if full_factorial {
        let ys = by_cell(&|i| d.y[i][0]);
        let centre = |stat: &dyn Fn(&Distribution) -> Option<f64>| -> Vec<Vec<(f64, f64)>> {
            ys.iter()
                .map(|g| {
                    let c = stat(&Distribution::new(g)).unwrap_or(f64::NAN);
                    g.iter().map(|(y, w)| ((y - c).abs(), *w)).collect()
                })
                .collect()
        };
        let median = centre(&|dist| dist.percentile(50.0, PercentileMethod::Haverage));
        let l = levene_of(&median);
        push("Based on Median", l, l.2);
        push("Based on Median and with adjusted df", l, l.3);
        let trimmed = centre(&|dist| dist.trimmed_mean());
        let l = levene_of(&trimmed);
        push("Based on trimmed mean", l, l.2);
    }
    let design = fit
        .effects
        .iter()
        .map(|e| e.label.clone())
        .collect::<Vec<_>>()
        .join(" + ");
    Some(table(
        "Levene's Test of Equality of Error Variances",
        1,
        plain(&["", "Levene Statistic", "df1", "df2", "Sig."]),
        rows,
        Some(format!(
            "Tests the null hypothesis that the error variance of the dependent variable is equal across groups. Design: {design}."
        )),
    ))
}

pub(crate) fn parameter_name(d: &Data, e: &Effect, cell: &[usize]) -> String {
    if e.is_intercept() {
        return "Intercept".into();
    }
    let mut k = 0;
    e.parts
        .iter()
        .map(|p| match p {
            Part::Factor(f) => {
                let level = cell[k];
                k += 1;
                format!("[{}={}]", d.factors[*f], d.values[*f][level])
            }
            Part::Covariate(c) => d.covariates[*c].clone(),
        })
        .collect::<Vec<_>>()
        .join(" * ")
}

fn parameter_table(
    command: &Glm,
    d: &Data,
    fit: &Fit,
    dependent: &str,
    flags: &Flags,
    mse: f64,
    dfe: f64,
) -> Table {
    let level = percent_title(command.alpha);
    let mut columns = plain(&["Parameter", "B", "Std. Error", "t", "Sig."]);
    let group = format!("{level} Confidence Interval");
    columns.push((group.clone(), "Lower Bound".into()));
    columns.push((group, "Upper Bound".into()));
    if flags.etasq {
        columns.extend(plain(&["Partial Eta Squared"]));
    }
    if flags.opower {
        columns.extend(plain(&["Noncent. Parameter", "Observed Power"]));
    }
    let tc = t_quantile(command.alpha / 2.0, dfe);
    let mut rows = Vec::new();
    let mut redundant = false;
    for e in &fit.effects {
        for (k, cell) in e.cells.iter().enumerate() {
            let j = e.start + k;
            let name = parameter_name(d, e, cell);
            if fit.redundant[j] {
                redundant = true;
                let mut row = vec![text(format!("{name} (redundant)")), Cell::Number(0.0)];
                row.extend(std::iter::repeat_n(Cell::Empty, columns.len() - 2));
                rows.push(row);
                continue;
            }
            let b = fit.b[j][0];
            let se = (mse * fit.ginv[j][j]).sqrt();
            let t = b / se;
            let mut row = vec![
                text(name),
                number(Some(b)),
                number(Some(se)),
                number(Some(t)),
                number(Some(t_p(t, dfe))),
                number(Some(b - tc * se)),
                number(Some(b + tc * se)),
            ];
            if flags.etasq {
                row.push(number(Some(t * t / (t * t + dfe))));
            }
            if flags.opower {
                let c = t.abs();
                let tcrit = t_quantile(flags.alpha / 2.0, dfe);
                row.push(number(Some(c)));
                row.push(number(Some(
                    1.0 - noncentral_t_cdf(tcrit, dfe, c) + noncentral_t_cdf(-tcrit, dfe, c),
                )));
            }
            rows.push(row);
        }
    }
    let mut caption = format!("Dependent Variable: {dependent}.");
    if redundant {
        caption.push_str(" A parameter marked redundant is set to zero because it is redundant.");
    }
    if flags.opower {
        caption.push_str(&format!(" Computed using alpha = {}.", flags.alpha));
    }
    table("Parameter Estimates", 1, columns, rows, Some(caption))
}

fn lack_of_fit(d: &Data, fit: &Fit, dependent: &str, flags: &Flags) -> Table {
    // the unique combinations of the predictors
    let mut keys: Vec<(Vec<usize>, Vec<u64>, usize)> = (0..d.rows())
        .map(|i| {
            (
                d.level[i].clone(),
                d.cov[i].iter().map(|v| v.to_bits()).collect(),
                i,
            )
        })
        .collect();
    keys.sort();
    let mut pure = 0.0;
    let mut unique = 0usize;
    let mut k = 0;
    while k < keys.len() {
        let start = k;
        while k < keys.len() && keys[k].0 == keys[start].0 && keys[k].1 == keys[start].1 {
            k += 1;
        }
        unique += 1;
        let members: Vec<usize> = keys[start..k].iter().map(|x| x.2).collect();
        let w: f64 = members.iter().map(|&i| d.w(i)).sum();
        let mean = members.iter().map(|&i| d.w(i) * d.y[i][0]).sum::<f64>() / w;
        pure += members
            .iter()
            .map(|&i| d.w(i) * (d.y[i][0] - mean).powi(2))
            .sum::<f64>();
    }
    let sse = fit.s[0][0];
    let df_pure = fit.n - unique as f64;
    let df_lack = unique as f64 - fit.rank as f64;
    let sources = [
        Source {
            label: "Lack of Fit".into(),
            ss: sse - pure,
            df: df_lack,
            tested: true,
        },
        Source {
            label: "Pure Error".into(),
            ss: pure,
            df: df_pure,
            tested: false,
        },
    ];
    f_table(
        "Lack of Fit Tests",
        &sources,
        pure,
        df_pure,
        flags,
        "Source",
        format!("Dependent Variable: {dependent}."),
    )
}

/// The l vector of the estimated marginal mean at the fixed levels, covariates at their means
/// or at the values of `with` (by covariate index).
pub(crate) fn emmeans_l(
    d: &Data,
    fit: &Fit,
    fixed: &[(usize, usize)],
    with: &[(usize, f64)],
) -> Vec<f64> {
    let mut l = vec![0.0; fit.p];
    for e in &fit.effects {
        let agree: Vec<usize> = e
            .cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| {
                fixed.iter().all(|&(f, level)| {
                    e.factors
                        .iter()
                        .position(|g| *g == f)
                        .is_none_or(|at| cell[at] == level)
                })
            })
            .map(|(k, _)| k)
            .collect();
        if agree.is_empty() {
            continue;
        }
        let given = e
            .covariates
            .iter()
            .any(|c| with.iter().any(|(w, _)| w == c));
        let value = if given {
            e.covariates
                .iter()
                .map(|c| {
                    with.iter()
                        .find(|(w, _)| w == c)
                        .map_or_else(|| d.covariate_mean(&[*c]), |(_, v)| *v)
                })
                .product::<f64>()
        } else if e.covariates.is_empty() {
            1.0
        } else {
            d.covariate_mean(&e.covariates)
        };
        for k in &agree {
            l[e.start + k] = value / agree.len() as f64;
        }
    }
    l
}

/// One row of an L matrix over every parameter of `fit`, from coefficients given by effect
/// (or for ALL of them); `what` names the subcommand in errors.
pub(crate) fn l_row(d: &Data, fit: &Fit, row: &LRow, what: &str) -> Result<Vec<f64>, Error> {
    let mut coefficients = vec![0.0; fit.p];
    for (effect, values) in row {
        let range = match effect {
            None => 0..fit.p,
            Some(term) => match find_effect(d, &fit.effects, term)? {
                Some(e) => e.range(),
                None => {
                    return Err(Error(format!(
                        "{what} names {}, which is not an effect of the design",
                        term.label
                    )));
                }
            },
        };
        if values.len() != range.len() {
            return Err(Error(format!(
                "{what} gives {} coefficients where the effect has {} parameters",
                values.len(),
                range.len()
            )));
        }
        for (j, v) in range.zip(values) {
            coefficients[j] += v;
        }
    }
    Ok(coefficients)
}

/// The effect of `effects` that `term` names, its factors in any order.
pub(crate) fn find_effect<'e>(
    d: &Data,
    effects: &'e [Effect],
    term: &Term,
) -> Result<Option<&'e Effect>, Error> {
    let wanted = effects_of(std::slice::from_ref(term), &d.factors, &d.covariates)?;
    let target = &wanted[0];
    let mut b = target.factors.clone();
    b.sort_unstable();
    Ok(effects.iter().find(|e| {
        let mut a = e.factors.clone();
        a.sort_unstable();
        a == b && e.covariates == target.covariates
    }))
}

/// The caption note on the values the covariates are held at.
pub(crate) fn covariate_note(d: &Data, with: &[(usize, f64)]) -> String {
    if d.covariates.is_empty() {
        return String::new();
    }
    let values: Vec<String> = d
        .covariates
        .iter()
        .enumerate()
        .map(|(c, n)| {
            let v = with
                .iter()
                .find(|(w, _)| *w == c)
                .map_or_else(|| d.covariate_mean(&[c]), |(_, v)| *v);
            format!("{n} = {}", (v * 1e4).round() / 1e4)
        })
        .collect();
    format!(
        " Covariates appearing in the model are evaluated at the following values: {}.",
        values.join(", ")
    )
}

/// The covariates of WITH by index, with the values given; MEAN ones are left out, so that
/// they stay at their means.
pub(crate) fn covariate_values(
    d: &Data,
    with: &[(String, Option<f64>)],
    what: &str,
) -> Result<Vec<(usize, f64)>, Error> {
    let mut out = Vec::new();
    for (name, value) in with {
        let Some(c) = d
            .covariates
            .iter()
            .position(|n| n.eq_ignore_ascii_case(name))
        else {
            return Err(Error(format!(
                "{what} WITH names {name}, which is not a covariate"
            )));
        };
        if let Some(v) = value {
            out.push((c, *v));
        }
    }
    Ok(out)
}

/// PLOT of the univariate model: profile plots of the estimated marginal means, the
/// spread-versus-level plots of the observed cells and the observed by predicted by
/// standardized residual plot.
fn plots(
    command: &Glm,
    d: &Data,
    fit: &Fit,
    dependent: &str,
    label_of: &dyn Fn(usize, usize) -> String,
) -> Result<Vec<Chart>, Error> {
    let mut charts = Vec::new();
    for plot in &command.plots {
        match plot {
            GlmPlot::Profile(profile) => {
                charts.extend(profile_plots(
                    d,
                    fit,
                    profile,
                    dependent,
                    label_of,
                    command.alpha,
                )?);
            }
            GlmPlot::SpreadLevel => charts.extend(spread_level(d, dependent, label_of)),
            GlmPlot::Residuals => charts.push(residual_plot(d, fit, dependent)),
        }
    }
    Ok(charts)
}

/// The estimated marginal mean l'b with its standard error from the error mean square and
/// l'G l; None when l is not estimable.
fn emmean(fit: &Fit, h: &Matrix, l: &[f64]) -> Option<(f64, f64)> {
    let mse = fit.s[0][0] / fit.df_error();
    fit.estimable(l, h).then(|| {
        let m: f64 = l.iter().zip(&fit.b).map(|(a, b)| a * b[0]).sum();
        let v: f64 = (0..fit.p)
            .map(|i| {
                (0..fit.p)
                    .map(|j| l[i] * fit.ginv[i][j] * l[j])
                    .sum::<f64>()
            })
            .sum();
        (m, (mse * v).sqrt())
    })
}

/// PROFILE: the estimated marginal means along the levels of the first factor, a line for
/// each level of the second and a chart for each level of the third; covariates at their
/// means or the WITH values. A mean that is not estimable has no value. ERRORBAR puts a bar
/// around each mean: the confidence interval at 1 - `alpha` (the t quantile on the error
/// degrees of freedom times the standard error) or a number of standard errors on each side;
/// the grand mean line has none.
fn profile_plots(
    d: &Data,
    fit: &Fit,
    profile: &Profile,
    dependent: &str,
    label_of: &dyn Fn(usize, usize) -> String,
    alpha: f64,
) -> Result<Vec<Chart>, Error> {
    use crate::chart::{Axis, Category, ChartKind, ErrorBars, Series, Statistic};
    use statscraft_syntax::ErrorBar;
    let h = fit.h();
    let mean_of = |l: &[f64]| emmean(fit, &h, l).unwrap_or((f64::NAN, f64::NAN));
    let factors: Vec<usize> = profile
        .factors
        .iter()
        .map(|n| {
            d.factors
                .iter()
                .position(|v| v.eq_ignore_ascii_case(n))
                .ok_or_else(|| Error(format!("PLOT PROFILE names {n}, which is not a factor")))
        })
        .collect::<Result<_, _>>()?;
    if factors.len() > 3 {
        return Err(Error("PLOT PROFILE crosses at most three factors".into()));
    }
    // the bars' label and how many standard errors they reach on each side
    let bars = profile.error_bar.map(|e| match e {
        ErrorBar::Ci => (
            format!("{} CI", percent_title(alpha)),
            t_quantile(alpha / 2.0, fit.df_error()),
        ),
        ErrorBar::Se(times) => (format!("{times} SE"), times),
    });
    let with = covariate_values(d, &profile.with, "PLOT PROFILE")?;
    let mut notes = vec![format!("Dependent variable: {dependent}")];
    if !d.covariates.is_empty() {
        let at: Vec<String> = (0..d.covariates.len())
            .map(|c| {
                let v = with
                    .iter()
                    .find(|w| w.0 == c)
                    .map_or_else(|| d.covariate_mean(&[c]), |w| w.1);
                format!("{} = {}", d.covariates[c], round3(v))
            })
            .collect();
        notes.push(format!("Covariates at {}", at.join(", ")));
    }
    if let Some((label, _)) = &bars {
        notes.push(format!("Error bars: {label}"));
    }
    let grand = profile
        .mean_reference
        .then(|| mean_of(&emmeans_l(d, fit, &[], &with)).0);
    let levels = |k: usize| factors.get(k).map_or(1, |&f| d.values[f].len());
    let f0 = factors[0];
    let categories: Vec<Category> = (0..levels(0))
        .map(|l| Category {
            label: label_of(f0, l),
            value: Cell::Text(d.values[f0][l].clone()),
            missing: false,
            bin: None,
        })
        .collect();
    let mut charts = Vec::new();
    for panel in 0..levels(2) {
        let mut margins = Vec::new();
        let mut series: Vec<Series> = (0..levels(1))
            .map(|line| {
                let (values, se): (Vec<f64>, Vec<f64>) = (0..levels(0))
                    .map(|l| {
                        let fixed: Vec<(usize, usize)> =
                            factors.iter().copied().zip([l, line, panel]).collect();
                        mean_of(&emmeans_l(d, fit, &fixed, &with))
                    })
                    .unzip();
                if let Some((_, times)) = &bars {
                    margins.push(se.iter().map(|s| times * s).collect());
                }
                match factors.get(1) {
                    Some(&f1) => Series {
                        name: label_of(f1, line),
                        statistic: Statistic::Mean,
                        variable: Some(d.factors[f1].clone()),
                        value: Cell::Text(d.values[f1][line].clone()),
                        values,
                    },
                    None => Series {
                        name: "Estimated marginal mean".into(),
                        statistic: Statistic::Mean,
                        variable: None,
                        value: Cell::Empty,
                        values,
                    },
                }
            })
            .collect();
        if let Some(g) = grand {
            series.push(Series {
                name: "Grand mean".into(),
                statistic: Statistic::Mean,
                variable: None,
                value: Cell::Empty,
                values: vec![g; levels(0)],
            });
            // a reference line: no bars
            margins.push(Vec::new());
        }
        let mut title = format!("Estimated marginal means of {dependent}");
        if let Some(&f2) = factors.get(2) {
            title = format!(
                "{title} at {} = {}",
                d.factor_texts[f2],
                label_of(f2, panel)
            );
        }
        let mut chart = Chart::lines(
            title,
            Axis {
                variable: Some(d.factors[f0].clone()),
                ..Axis::titled(d.factor_texts[f0].clone())
            },
            Axis::titled("Estimated marginal means"),
            categories.clone(),
            series,
        );
        if profile.bar {
            chart.kind = ChartKind::Bar;
        }
        if let Some(&f1) = factors.get(1) {
            chart.legend = Some(d.factor_texts[f1].clone());
        }
        chart.error_bars = bars.as_ref().map(|(label, _)| ErrorBars {
            label: label.clone(),
            margins,
            lower: Vec::new(),
        });
        chart.footnote = Some(notes.join(". "));
        charts.push(chart);
    }
    Ok(charts)
}

/// SPREADLEVEL: every cell of the factors with more than one case, its mean against its
/// standard deviation and against its variance, with the frequency weights of the
/// descriptive statistics.
fn spread_level(
    d: &Data,
    dependent: &str,
    label_of: &dyn Fn(usize, usize) -> String,
) -> Vec<Chart> {
    use crate::chart::{Axis, ChartKind};
    use crate::plot::{Plot, Point, PointGroup};
    let sizes: Vec<usize> = d.values.iter().map(Vec::len).collect();
    let mut cells: Vec<(String, f64, f64)> = Vec::new();
    for combo in combinations(&sizes) {
        let members: Vec<usize> = (0..d.rows())
            .filter(|&i| combo.iter().enumerate().all(|(f, &l)| d.level[i][f] == l))
            .collect();
        let n: f64 = members.iter().map(|&i| d.f[i]).sum();
        if n <= 1.0 {
            continue;
        }
        let mean = members.iter().map(|&i| d.f[i] * d.y[i][0]).sum::<f64>() / n;
        let ss: f64 = members
            .iter()
            .map(|&i| d.f[i] * (d.y[i][0] - mean).powi(2))
            .sum();
        let label = match combo.is_empty() {
            true => "Total".to_string(),
            false => combo
                .iter()
                .enumerate()
                .map(|(f, &l)| label_of(f, l))
                .collect::<Vec<_>>()
                .join(", "),
        };
        cells.push((label, mean, ss / (n - 1.0)));
    }
    ["standard deviation", "variance"]
        .into_iter()
        .map(|what| {
            let spread = |var: f64| match what {
                "variance" => var,
                _ => var.sqrt(),
            };
            let points = cells
                .iter()
                .map(|(label, mean, var)| Point {
                    x: *mean,
                    y: spread(*var),
                    z: None,
                    label: Some(label.clone()),
                })
                .collect();
            let mut chart = Chart::plot(
                ChartKind::SpreadLevel,
                format!("Spread versus level of {dependent}: {what}"),
                Axis::titled("Level (mean)"),
                Axis::titled(format!("Spread ({what})")),
                Plot::Points {
                    groups: vec![PointGroup {
                        name: what.into(),
                        points,
                    }],
                    lines: Vec::new(),
                    z_axis: None,
                },
            );
            chart.footnote = Some(format!(
                "Dependent variable: {dependent}. Cells of the factors"
            ));
            chart
        })
        .collect()
}

/// RESIDUALS: every case of the analysis with its predicted value (x), observed value (y)
/// and standardized residual (z), from which the matrix of the three is drawn.
fn residual_plot(d: &Data, fit: &Fit, dependent: &str) -> Chart {
    use crate::chart::{Axis, ChartKind};
    use crate::plot::{Plot, Point, PointGroup};
    let mse = fit.s[0][0] / fit.df_error();
    let points = (0..d.rows())
        .map(|i| {
            let pred: f64 = fit.x[i].iter().zip(&fit.b).map(|(x, b)| x * b[0]).sum();
            Point {
                x: pred,
                y: d.y[i][0],
                z: Some((d.y[i][0] - pred) / (mse / d.g[i]).sqrt()),
                label: None,
            }
        })
        .collect();
    let mut chart = Chart::plot(
        ChartKind::Scatter,
        format!("Observed by predicted by standardized residual: {dependent}"),
        Axis::titled("Predicted"),
        Axis::titled("Observed"),
        Plot::Points {
            groups: vec![PointGroup {
                name: dependent.to_string(),
                points,
            }],
            lines: Vec::new(),
            z_axis: Some(Axis::titled("Standardized residual")),
        },
    );
    chart.footnote = Some(format!("Dependent variable: {dependent}"));
    chart
}

#[allow(clippy::too_many_arguments)]
fn emmeans_tables(
    command: &Glm,
    d: &Data,
    fit: &Fit,
    h: &Matrix,
    e: &Emmeans,
    dependent: &str,
    flags: &Flags,
    label_of: &dyn Fn(usize, usize) -> String,
) -> Result<Vec<Table>, Error> {
    let find = |name: &str| -> Result<usize, Error> {
        d.factors
            .iter()
            .position(|v| v.eq_ignore_ascii_case(name))
            .ok_or_else(|| Error(format!("EMMEANS names {name}, which is not a factor")))
    };
    let factors: Vec<usize> = e
        .factors
        .iter()
        .map(|n| find(n))
        .collect::<Result<_, _>>()?;
    let with = covariate_values(d, &e.with, "EMMEANS")?;
    let dfe = fit.df_error();
    let tc = t_quantile(command.alpha / 2.0, dfe);
    let level = percent_title(command.alpha);
    let combos = combinations(
        &factors
            .iter()
            .map(|&f| d.values[f].len())
            .collect::<Vec<_>>(),
    );
    let ls: Vec<Vec<f64>> = combos
        .iter()
        .map(|combo| {
            let fixed: Vec<(usize, usize)> =
                factors.iter().copied().zip(combo.iter().copied()).collect();
            emmeans_l(d, fit, &fixed, &with)
        })
        .collect();
    let mean_of = |l: &[f64]| emmean(fit, h, l);
    let mut columns: Vec<(String, String)> = factors
        .iter()
        .map(|&f| (String::new(), d.factor_texts[f].clone()))
        .collect();
    columns.extend(plain(&["Mean", "Std. Error"]));
    let group = format!("{level} Confidence Interval");
    columns.push((group.clone(), "Lower Bound".into()));
    columns.push((group, "Upper Bound".into()));
    let mut rows = Vec::new();
    let mut not_estimable = false;
    for (combo, l) in combos.iter().zip(&ls) {
        let mut row: Vec<Cell> = factors
            .iter()
            .zip(combo)
            .enumerate()
            .map(|(k, (&f, &lv))| {
                let first = rows.is_empty() || combo[k + 1..].iter().all(|&x| x == 0);
                if first {
                    text(label_of(f, lv))
                } else {
                    Cell::Empty
                }
            })
            .collect();
        match mean_of(l) {
            Some((m, se)) => row.extend([
                number(Some(m)),
                number(Some(se)),
                number(Some(m - tc * se)),
                number(Some(m + tc * se)),
            ]),
            None => {
                not_estimable = true;
                row.extend([Cell::Empty, Cell::Empty, Cell::Empty, Cell::Empty]);
            }
        }
        rows.push(row);
    }
    let covariate_note = covariate_note(d, &with);
    let mut caption = format!("Dependent Variable: {dependent}.{covariate_note}");
    if not_estimable {
        caption.push_str(" A mean without a value is not estimable.");
    }
    let title = if factors.is_empty() {
        "Estimated Marginal Means: Grand Mean".to_string()
    } else {
        format!("Estimated Marginal Means: {}", e.factors.join(" * "))
    };
    let mut out = Vec::new();
    if factors.is_empty() {
        let mut cols = plain(&["Mean", "Std. Error"]);
        let group = format!("{level} Confidence Interval");
        cols.push((group.clone(), "Lower Bound".into()));
        cols.push((group, "Upper Bound".into()));
        out.push(table(&title, 0, cols, rows, Some(caption.clone())));
    } else {
        out.push(table(
            &title,
            factors.len(),
            columns,
            rows,
            Some(caption.clone()),
        ));
    }
    let Some(compare) = &e.compare else {
        return Ok(out);
    };
    let compared = match compare {
        Some(n) => find(n)?,
        None => factors[0],
    };
    let Some(at) = factors.iter().position(|&f| f == compared) else {
        return Err(Error(format!(
            "COMPARE names {}, which is not in TABLES",
            d.factors[compared].as_str()
        )));
    };
    let k = d.values[compared].len();
    let m = (k * (k - 1) / 2) as f64;
    let others: Vec<usize> = (0..factors.len()).filter(|&i| i != at).collect();
    let other_combos = combinations(
        &others
            .iter()
            .map(|&i| d.values[factors[i]].len())
            .collect::<Vec<_>>(),
    );
    let index_of = |other: &[usize], lv: usize| -> usize {
        let mut combo = vec![0; factors.len()];
        for (o, &i) in other.iter().zip(&others) {
            combo[i] = *o;
        }
        combo[at] = lv;
        combos
            .iter()
            .position(|c| *c == combo)
            .expect("a combination")
    };
    let tcrit = adjusted_t(command.alpha, m, dfe, e.adjustment);
    let mut columns: Vec<(String, String)> = others
        .iter()
        .map(|&i| (String::new(), d.factor_texts[factors[i]].clone()))
        .collect();
    let name = d.factor_texts[compared].as_str();
    columns.push((String::new(), format!("(I) {name}")));
    columns.push((String::new(), format!("(J) {name}")));
    columns.extend(plain(&["Mean Difference (I-J)", "Std. Error", "Sig."]));
    let group = format!("{level} Confidence Interval for Difference");
    columns.push((group.clone(), "Lower Bound".into()));
    columns.push((group, "Upper Bound".into()));
    let mut rows = Vec::new();
    let mut tests = Vec::new();
    for other in &other_combos {
        for i in 0..k {
            for j in (0..k).filter(|&j| j != i) {
                let li = &ls[index_of(other, i)];
                let lj = &ls[index_of(other, j)];
                let diff: Vec<f64> = li.iter().zip(lj).map(|(a, b)| a - b).collect();
                let mut row: Vec<Cell> = others
                    .iter()
                    .zip(other)
                    .map(|(&o, &lv)| {
                        if i == 0 && j == 1 {
                            text(label_of(factors[o], lv))
                        } else {
                            Cell::Empty
                        }
                    })
                    .collect();
                row.push(if j == usize::from(i == 0) {
                    text(label_of(compared, i))
                } else {
                    Cell::Empty
                });
                row.push(text(label_of(compared, j)));
                match mean_of(&diff) {
                    Some((md, se)) => {
                        let p = adjusted(t_p(md / se, dfe), m, e.adjustment);
                        row.extend([
                            number(Some(md)),
                            number(Some(se)),
                            number(Some(p)),
                            number(Some(md - tcrit * se)),
                            number(Some(md + tcrit * se)),
                        ]);
                    }
                    None => row.extend(std::iter::repeat_n(Cell::Empty, 5)),
                }
                rows.push(row);
            }
        }
        // the univariate test of the compared factor at these levels: every estimable pairwise
        // difference (the F does not depend on which rows span the hypothesis)
        let l: Matrix = (0..k)
            .flat_map(|i| (i + 1..k).map(move |j| (i, j)))
            .map(|(i, j)| {
                let li = &ls[index_of(other, i)];
                let lj = &ls[index_of(other, j)];
                li.iter().zip(lj).map(|(a, b)| a - b).collect()
            })
            .filter(|row: &Vec<f64>| fit.estimable(row, h))
            .collect();
        let (ssh, df) = fit.hypothesis(&l, None, None);
        let label: Vec<String> = others
            .iter()
            .zip(other)
            .map(|(&o, &lv)| {
                format!(
                    "{} = {}",
                    d.factor_texts[factors[o]].as_str(),
                    label_of(factors[o], lv)
                )
            })
            .collect();
        tests.push((label.join(", "), ssh[0][0], df as f64));
    }
    out.push(table(
        "Pairwise Comparisons",
        others.len() + 2,
        columns,
        rows,
        Some(format!(
            "Dependent Variable: {dependent}. Based on estimated marginal means. Adjustment for multiple comparisons: {}.",
            adjustment_name(e.adjustment)
        )),
    ));
    let sse = fit.s[0][0];
    let mut sources = Vec::new();
    for (label, ss, df) in tests {
        sources.push(Source {
            label: if label.is_empty() {
                "Contrast".into()
            } else {
                format!("{label}: Contrast")
            },
            ss,
            df,
            tested: true,
        });
        sources.push(Source {
            label: if label.is_empty() {
                "Error".into()
            } else {
                format!("{label}: Error")
            },
            ss: sse,
            df: dfe,
            tested: false,
        });
    }
    out.push(f_table(
        "Univariate Tests",
        &sources,
        sse,
        dfe,
        flags,
        "",
        format!(
            "Dependent Variable: {dependent}. The F tests the effect of {name}, based on the linearly independent pairwise comparisons among the estimated marginal means."
        ),
    ));
    Ok(out)
}

/// The custom hypothesis test L B M = K of the rows of `l`: the Contrast Results (K Matrix),
/// then for one dependent variable the Test Results, else the Multivariate and the Univariate
/// Test Results of the transformed variables (the columns of M, named `names`).
#[allow(clippy::too_many_arguments)]
fn custom_tests(
    fit: &Fit,
    h: &Matrix,
    l: &[(String, Vec<f64>)],
    (m, names, transformed): (&Matrix, &[String], bool),
    k: Option<&Matrix>,
    title: &str,
    flags: &Flags,
    warnings: &mut Vec<String>,
) -> Vec<Table> {
    let dfe = fit.df_error();
    let bm = multiply(&fit.b, m);
    let se_m = quadratic(m, &fit.s);
    let tc = t_quantile(flags.alpha / 2.0, dfe);
    let level = percent_title(flags.alpha);
    let univariate = fit.r == 1 && names.len() == 1;
    let caption = if univariate {
        format!("{title}. Dependent Variable: {}.", names[0])
    } else {
        format!("{title}.")
    };
    let mut rows = Vec::new();
    let mut kept: Matrix = Vec::new();
    let mut kept_k: Matrix = Vec::new();
    for (i, (name, row)) in l.iter().enumerate() {
        if !fit.estimable(row, h) {
            warnings.push(format!(
                "{title}: {name} is not estimable and is not tested"
            ));
            continue;
        }
        let v: f64 = (0..fit.p)
            .map(|a| {
                (0..fit.p)
                    .map(|b| row[a] * fit.ginv[a][b] * row[b])
                    .sum::<f64>()
            })
            .sum();
        let mut lines: Vec<Vec<Option<f64>>> = vec![Vec::new(); 7];
        let hyps: Vec<f64> = (0..names.len())
            .map(|c| k.map_or(0.0, |k| k[i][c]))
            .collect();
        for (c, hyp) in hyps.iter().enumerate() {
            let estimate: f64 = row.iter().zip(&bm).map(|(a, b)| a * b[c]).sum();
            let se = (se_m[c][c] / dfe * v).sqrt();
            let diff = estimate - hyp;
            for (line, value) in lines.iter_mut().zip([
                estimate,
                *hyp,
                diff,
                se,
                t_p(diff / se, dfe),
                diff - tc * se,
                diff + tc * se,
            ]) {
                line.push(Some(value));
            }
        }
        let whats = [
            "Contrast Estimate",
            "Hypothesized Value",
            "Difference (Estimate - Hypothesized)",
            "Std. Error",
            "Sig.",
            "Lower Bound",
            "Upper Bound",
        ];
        for (j, (what, values)) in whats.into_iter().zip(lines).enumerate() {
            let what = if j >= 5 {
                format!("{level} Confidence Interval for Difference: {what}")
            } else {
                what.to_string()
            };
            let mut cells = vec![
                if j == 0 {
                    text(name.clone())
                } else {
                    Cell::Empty
                },
                text(what),
            ];
            cells.extend(values.into_iter().map(number));
            rows.push(cells);
        }
        kept.push(row.clone());
        kept_k.push(hyps);
    }
    let mut columns = plain(&["Contrast", ""]);
    columns.extend(names.iter().map(|n| (String::new(), n.clone())));
    let mut out = vec![table(
        "Contrast Results (K Matrix)",
        2,
        columns,
        rows,
        Some(caption.clone()),
    )];
    if kept.is_empty() {
        return out;
    }
    let (ssh, df) = fit.hypothesis(&kept, Some(m), k.map(|_| &kept_k));
    let tests = |c: usize| {
        let sources = [
            Source {
                label: "Contrast".into(),
                ss: ssh[c][c],
                df: df as f64,
                tested: true,
            },
            Source {
                label: "Error".into(),
                ss: se_m[c][c],
                df: dfe,
                tested: false,
            },
        ];
        f_table(
            "Test Results",
            &sources,
            se_m[c][c],
            dfe,
            flags,
            "Source",
            caption.clone(),
        )
    };
    if univariate {
        out.push(tests(0));
        return out;
    }
    out.push(table(
        "Multivariate Test Results",
        1,
        multivariate_columns(flags, false),
        multivariate_rows(&ssh, &se_m, df as f64, dfe, flags),
        Some(format!("{caption}{}", multivariate_note(flags))),
    ));
    let header = if transformed {
        "Transformed Variable"
    } else {
        "Dependent Variable"
    };
    let mut univariate = merge((0..names.len()).map(tests).collect(), names, true, header);
    univariate.title = "Univariate Test Results".into();
    univariate.caption = Some(caption);
    out.push(univariate);
    out
}

/// The Kronecker product of the columns of each factor, the first factor's rows changing
/// slowest.
fn kronecker(parts: &[Matrix]) -> Matrix {
    let mut out: Matrix = vec![vec![1.0]];
    for a in parts {
        let rows = out.len() * a.len();
        let cols = out[0].len() * a[0].len();
        let mut next = vec![vec![0.0; cols]; rows];
        for (i, orow) in out.iter().enumerate() {
            for (k, arow) in a.iter().enumerate() {
                for (j, ov) in orow.iter().enumerate() {
                    for (l, av) in arow.iter().enumerate() {
                        next[i * a.len() + k][j * a[0].len() + l] = ov * av;
                    }
                }
            }
        }
        out = next;
    }
    out
}

/// M' A M.
fn quadratic(m: &Matrix, a: &Matrix) -> Matrix {
    multiply(&multiply(&transpose(m), a), m)
}

fn trace(a: &Matrix) -> f64 {
    (0..a.len()).map(|i| a[i][i]).sum()
}

/// The eigenvalues of S_E^-1 S_H, largest first, and the rank of S_E.
fn relative_eigenvalues(sh: &Matrix, se: &Matrix) -> (Vec<f64>, usize) {
    let pairs = crate::matrix::symmetric_eigen(se);
    let n = se.len();
    let mut rank = 0;
    // S_E^-1/2 = V diag(1/sqrt(e)) V'
    let mut root = vec![vec![0.0; n]; n];
    for (value, vector) in &pairs {
        if *value <= 1e-12 * pairs[0].0.abs().max(1e-300) {
            continue;
        }
        rank += 1;
        for i in 0..n {
            for j in 0..n {
                root[i][j] += vector[i] * vector[j] / value.sqrt();
            }
        }
    }
    let middle = multiply(&multiply(&root, sh), &root);
    let mut values: Vec<f64> = crate::matrix::symmetric_eigen(&middle)
        .into_iter()
        .map(|(v, _)| v.max(0.0))
        .collect();
    values.sort_by(|a, b| b.total_cmp(a));
    (values, rank)
}

/// The four multivariate statistics: name, value, F, hypothesis df, error df, eta squared and
/// the denominator df of the noncentrality.
fn multivariate(
    sh: &Matrix,
    se: &Matrix,
    l: f64,
    ne: f64,
) -> Vec<(&'static str, f64, f64, f64, f64, f64)> {
    let (lambdas, re) = relative_eigenvalues(sh, se);
    let re = re as f64;
    let s = l.min(re);
    let m = ((re - l).abs() - 1.0) / 2.0;
    let n = (ne - re - 1.0) / 2.0;
    let pillai: f64 = lambdas.iter().map(|x| x / (1.0 + x)).sum();
    let wilks: f64 = lambdas.iter().map(|x| 1.0 / (1.0 + x)).product();
    let hotelling: f64 = lambdas.iter().sum();
    let roy = lambdas.first().copied().unwrap_or(0.0);
    let mut out = Vec::new();
    let (df1, df2) = (s * (2.0 * m + s + 1.0), s * (2.0 * n + s + 1.0));
    out.push((
        "Pillai's Trace",
        pillai,
        df2 / df1 * pillai / (s - pillai),
        df1,
        df2,
        pillai / s,
    ));
    let varsigma = ne - (re - l + 1.0) / 2.0;
    let upsilon = (l * re - 2.0) / 4.0;
    let tau = if l * l + re * re - 5.0 > 0.0 {
        ((l * l * re * re - 4.0) / (l * l + re * re - 5.0)).sqrt()
    } else {
        1.0
    };
    let root = wilks.powf(1.0 / tau);
    let df2w = varsigma * tau - 2.0 * upsilon;
    out.push((
        "Wilks' Lambda",
        wilks,
        df2w / (l * re) * (1.0 - root) / root,
        l * re,
        df2w,
        1.0 - wilks.powf(1.0 / s),
    ));
    let df2h = 2.0 * (s * n + 1.0);
    out.push((
        "Hotelling's Trace",
        hotelling,
        df2h / (s * df1 / s) * hotelling / s,
        df1,
        df2h,
        (hotelling / s) / (hotelling / s + 1.0),
    ));
    let omega = l.max(re);
    let df2r = ne - omega + l;
    out.push((
        "Roy's Largest Root",
        roy,
        roy * df2r / omega,
        omega,
        df2r,
        roy / (1.0 + roy),
    ));
    out
}

/// The significance of Mauchly's chi-square by the manual's expansion.
fn mauchly(xi: &Matrix, dfe: f64) -> (f64, f64, f64, f64) {
    let m = xi.len() as f64;
    let det = crate::matrix::inverse(xi).map_or(0.0, |(_, d)| d);
    let w = det / (trace(xi) / m).powf(m);
    let rho = 1.0 - (2.0 * m * m + m + 2.0) / (6.0 * m * dfe);
    let f = m * (m + 1.0) / 2.0 - 1.0;
    let c = -rho * dfe * w.ln();
    let omega2 =
        (m + 2.0) * (m - 1.0) * (m - 2.0) * (2.0 * m.powi(3) + 6.0 * m * m + 3.0 * m + 2.0)
            / (288.0 * m * m * dfe * dfe * rho * rho);
    let cdf = |c: f64, f: f64| 1.0 - crate::dist::chi_square_p(c, f);
    let sig = (1.0 - cdf(c, f) - omega2 * (cdf(c, f + 4.0) - cdf(c, f))).max(0.0);
    (w, c, f, sig)
}

impl Engine {
    #[allow(clippy::too_many_arguments)]
    fn repeated_measures(
        &mut self,
        command: &Glm,
        d: &Data,
        effects: Vec<Effect>,
        dependents: &[String],
        warnings: &mut Vec<String>,
        label_of: &dyn Fn(usize, usize) -> String,
    ) -> Result<Vec<Table>, Error> {
        let ws = &command.wsfactors;
        let r: usize = ws.iter().map(|w| w.levels).product();
        if r != dependents.len() {
            return Err(Error(format!(
                "WSFACTOR gives {r} level combinations for {} dependent variables; they must be equal",
                dependents.len()
            )));
        }
        for (what, given) in [
            ("SAVE", !command.save.is_empty()),
            ("POSTHOC", !command.posthoc.is_empty()),
            ("CONTRAST", !command.contrasts.is_empty()),
            ("LMATRIX", !command.lmatrix.is_empty()),
            ("PRINT LOF", command.print.lof),
            ("PRINT PARAMETER", command.print.parameter),
            ("PRINT HOMOGENEITY", command.print.homogeneity),
            ("MMATRIX", command.mmatrix.is_some()),
            ("KMATRIX", command.kmatrix.is_some()),
            ("PRINT RSSCP", command.print.rsscp),
            ("PRINT TEST(SSCP)", command.print.sscp),
            ("PRINT TEST(MMATRIX)", command.print.mmatrix),
        ] {
            if given {
                return Err(Error(format!("{what} with WSFACTOR is not supported yet")));
            }
        }
        for w in ws {
            let clash = d
                .factors
                .iter()
                .chain(&d.covariates)
                .any(|n| n.eq_ignore_ascii_case(&w.name));
            if clash {
                return Err(Error(format!(
                    "the within-subjects factor {} has the name of a variable of the model",
                    w.name
                )));
            }
        }
        // the within-subjects effects
        let within: Vec<Vec<usize>> = match &command.wsdesign {
            Some(terms) => {
                let mut out = Vec::new();
                for t in terms.iter().filter(|t| !t.variables.is_empty()) {
                    let mut set = Vec::new();
                    for name in &t.variables {
                        let Some(k) = ws.iter().position(|w| w.name.eq_ignore_ascii_case(name))
                        else {
                            return Err(Error(format!(
                                "{name} in WSDESIGN is not a within-subjects factor"
                            )));
                        };
                        if t.label.contains('(') {
                            return Err(Error("nesting in WSDESIGN is not supported yet".into()));
                        }
                        set.push(k);
                    }
                    set.sort_unstable();
                    set.dedup();
                    out.push(set);
                }
                out
            }
            None => {
                let t = ws.len();
                let mut out: Vec<Vec<usize>> = (1u32..(1 << t))
                    .map(|mask| (0..t).filter(|k| mask & (1 << k) != 0).collect())
                    .collect();
                out.sort_by_key(|s: &Vec<usize>| (s.len(), s.clone()));
                out
            }
        };
        let within_label = |set: &[usize]| {
            set.iter()
                .map(|&k| ws[k].name.clone())
                .collect::<Vec<_>>()
                .join(" * ")
        };
        let y = d.y.clone();
        let fit = Fit::new(d, effects, &y, command.eps);
        let sscp = effect_sscp(&fit, command.sstype);
        let dfe = fit.df_error();
        let flags = flags_of(command);
        let between_label = |e: &Effect| e.label.clone();
        let mut out = Vec::new();
        // Within-Subjects Factors
        let mut columns: Vec<(String, String)> =
            ws.iter().map(|w| (String::new(), w.name.clone())).collect();
        columns.extend(plain(&["Dependent Variable"]));
        let combos = combinations(&ws.iter().map(|w| w.levels).collect::<Vec<_>>());
        let rows = combos
            .iter()
            .zip(dependents)
            .map(|(combo, dep)| {
                let mut row: Vec<Cell> = combo
                    .iter()
                    .map(|&l| Cell::Number((l + 1) as f64))
                    .collect();
                row.push(text(dep.clone()));
                row
            })
            .collect();
        out.push(table(
            "Within-Subjects Factors",
            ws.len(),
            columns,
            rows,
            None,
        ));
        if !d.factors.is_empty() {
            out.push(between_factors(d));
        }
        if command.print.descriptive {
            for (j, dep) in dependents.iter().enumerate() {
                out.push(descriptive_table(d, j, dep, label_of));
            }
        }
        // the columns of each within-subjects factor
        let contrast_columns = |k: usize, kind: &Contrast| -> Result<Matrix, Error> {
            let rows = contrast_rows(kind, ws[k].levels)
                .map_err(|e| Error(format!("WSFACTOR {}: {e}", ws[k].name)))?;
            Ok(transpose(
                &rows.into_iter().map(|(_, r)| r).collect::<Matrix>(),
            ))
        };
        let contrast_names = |k: usize| -> Vec<String> {
            contrast_rows(&ws[k].contrast, ws[k].levels)
                .map(|rows| rows.into_iter().map(|(n, _)| n).collect())
                .unwrap_or_default()
        };
        let polynomial = Contrast::Polynomial(None);
        let mut mv_rows = Vec::new();
        let mut mauchly_rows = Vec::new();
        let mut averaged = Vec::new();
        let mut contrast_table_rows = Vec::new();
        let mut contrast_sources: Vec<(String, String, f64, f64, f64, f64)> = Vec::new();
        for set in &within {
            let name = within_label(set);
            let mut mv_parts = Vec::new();
            let mut avg_parts = Vec::new();
            for (k, w) in ws.iter().enumerate() {
                if set.contains(&k) {
                    mv_parts.push(contrast_columns(k, &w.contrast)?);
                    avg_parts.push(contrast_columns(k, &polynomial)?);
                } else {
                    let n = w.levels;
                    mv_parts.push(vec![vec![1.0]; n]);
                    avg_parts.push(vec![vec![1.0 / (n as f64).sqrt()]; n]);
                }
            }
            let m_mv = kronecker(&mv_parts);
            let m_avg = kronecker(&avg_parts);
            let dd = m_avg[0].len() as f64;
            let se_mv = quadratic(&m_mv, &fit.s);
            let se_avg = quadratic(&m_avg, &fit.s);
            // Mauchly's test and the epsilons
            let gg = trace(&se_avg).powi(2) / (dd * trace(&multiply(&se_avg, &se_avg)));
            let n = fit.n;
            let hf = ((n * dd * gg - 2.0) / (dd * dfe - dd * dd * gg)).min(1.0);
            let lb = 1.0 / dd;
            let (w, chi, df, sig) = if dd >= 2.0 {
                mauchly(&se_avg, dfe)
            } else {
                (1.0, 0.0, 0.0, f64::NAN)
            };
            mauchly_rows.push(vec![
                text(name.clone()),
                number(Some(w)),
                number(Some(chi)),
                number(Some(df)),
                number(Some(sig)),
                number(Some(gg)),
                number(Some(hf)),
                number(Some(lb)),
            ]);
            // the multivariate, averaged and contrast tests of each between-subjects source
            let mut sources = Vec::new();
            for (e, (sh, rank)) in fit.effects.iter().zip(&sscp) {
                let label = if e.is_intercept() {
                    name.clone()
                } else {
                    format!("{name} * {}", between_label(e))
                };
                let sh_mv = quadratic(&m_mv, sh);
                let l = *rank as f64;
                for (i, mut row) in multivariate_rows(&sh_mv, &se_mv, l, dfe, &flags)
                    .into_iter()
                    .enumerate()
                {
                    let first = if i == 0 {
                        text(label.clone())
                    } else {
                        Cell::Empty
                    };
                    row.insert(0, first);
                    mv_rows.push(row);
                }
                let sh_avg = quadratic(&m_avg, sh);
                sources.push((label.clone(), trace(&sh_avg), l * dd));
                let names: Vec<Vec<String>> = (0..ws.len())
                    .filter(|k| set.contains(k))
                    .map(contrast_names)
                    .collect();
                let names = combinations(&names.iter().map(Vec::len).collect::<Vec<_>>())
                    .into_iter()
                    .map(|c| {
                        c.iter()
                            .enumerate()
                            .map(|(i, &j)| names[i][j].clone())
                            .collect::<Vec<_>>()
                            .join(" * ")
                    })
                    .collect::<Vec<_>>();
                for (j, cname) in names.iter().enumerate() {
                    contrast_sources.push((
                        label.clone(),
                        cname.clone(),
                        sh_mv[j][j],
                        l,
                        se_mv[j][j],
                        dfe,
                    ));
                }
            }
            averaged.push((
                name.clone(),
                sources,
                trace(&se_avg),
                dfe * dd,
                [gg, hf, lb],
            ));
            let names = (0..ws.len())
                .filter(|k| set.contains(k))
                .map(contrast_names)
                .collect::<Vec<_>>();
            let names = combinations(&names.iter().map(Vec::len).collect::<Vec<_>>())
                .into_iter()
                .map(|c| {
                    c.iter()
                        .enumerate()
                        .map(|(i, &j)| names[i][j].clone())
                        .collect::<Vec<_>>()
                        .join(" * ")
                })
                .collect::<Vec<_>>();
            for (j, cname) in names.iter().enumerate() {
                contrast_sources.push((
                    format!("Error({name})"),
                    cname.clone(),
                    se_mv[j][j],
                    dfe,
                    f64::NAN,
                    dfe,
                ));
            }
        }
        out.push(table(
            "Multivariate Tests",
            2,
            multivariate_columns(&flags, true),
            mv_rows,
            Some(format!(
                "Design: {}. Within Subjects Design: {}.",
                fit.effects
                    .iter()
                    .map(|e| e.label.clone())
                    .collect::<Vec<_>>()
                    .join(" + "),
                within
                    .iter()
                    .map(|s| within_label(s))
                    .collect::<Vec<_>>()
                    .join(" + ")
            )),
        ));
        let mut columns = plain(&[
            "Within Subjects Effect",
            "Mauchly's W",
            "Approx. Chi-Square",
            "df",
            "Sig.",
        ]);
        for e in ["Greenhouse-Geisser", "Huynh-Feldt", "Lower-bound"] {
            columns.push(("Epsilon".into(), e.into()));
        }
        out.push(table(
            "Mauchly's Test of Sphericity",
            1,
            columns,
            mauchly_rows,
            Some(
                "Tests the null hypothesis that the error covariance matrix of the orthonormalized transformed dependent variables is proportional to an identity matrix. The epsilons adjust the degrees of freedom of the averaged tests of significance."
                    .into(),
            ),
        ));
        // Tests of Within-Subjects Effects
        let mut columns = plain(&[
            "Source",
            "",
            "Type III Sum of Squares",
            "df",
            "Mean Square",
            "F",
            "Sig.",
        ]);
        columns[2].1 = format!("Type {} Sum of Squares", roman(command.sstype));
        if flags.etasq {
            columns.extend(plain(&["Partial Eta Squared"]));
        }
        if flags.opower {
            columns.extend(plain(&["Noncent. Parameter", "Observed Power"]));
        }
        let mut rows = Vec::new();
        let corrections = [
            "Sphericity Assumed",
            "Greenhouse-Geisser",
            "Huynh-Feldt",
            "Lower-bound",
        ];
        for (_, sources, sse, dfe_avg, eps) in &averaged {
            let mse_sa = sse / dfe_avg;
            for (label, ss, df) in sources {
                for (i, corr) in corrections.iter().enumerate() {
                    let e = if i == 0 { 1.0 } else { eps[i - 1] };
                    let (df1, df2) = (df * e, dfe_avg * e);
                    let f = (ss / df) / mse_sa;
                    let mut row = vec![
                        if i == 0 {
                            text(label.clone())
                        } else {
                            Cell::Empty
                        },
                        text(*corr),
                        number(Some(*ss)),
                        number(Some(df1)),
                        number(Some(ss / df1)),
                        number(Some(f)),
                        number(Some(f_p(f, df1, df2))),
                    ];
                    if flags.etasq {
                        row.push(number(Some(ss / (ss + sse))));
                    }
                    if flags.opower {
                        let (lambda, power) = power_f(f, df1, df2, flags.alpha);
                        row.push(number(Some(lambda)));
                        row.push(number(Some(power)));
                    }
                    rows.push(row);
                }
            }
        }
        for (name, _, sse, dfe_avg, eps) in &averaged {
            for (i, corr) in corrections.iter().enumerate() {
                let e = if i == 0 { 1.0 } else { eps[i - 1] };
                let mut row = vec![
                    if i == 0 {
                        text(format!("Error({name})"))
                    } else {
                        Cell::Empty
                    },
                    text(*corr),
                    number(Some(*sse)),
                    number(Some(dfe_avg * e)),
                    number(Some(sse / (dfe_avg * e))),
                    Cell::Empty,
                    Cell::Empty,
                ];
                if flags.etasq {
                    row.push(Cell::Empty);
                }
                if flags.opower {
                    row.extend([Cell::Empty, Cell::Empty]);
                }
                rows.push(row);
            }
        }
        // the errors after their effects, as in the manual's tables: reorder by effect
        let mut ordered = Vec::new();
        let per_effect: Vec<usize> = averaged.iter().map(|a| a.1.len() * 4).collect();
        let split = per_effect.iter().sum::<usize>();
        let mut error_rows = rows.split_off(split).into_iter();
        let mut effect_rows = rows.into_iter();
        for n in &per_effect {
            ordered.extend(effect_rows.by_ref().take(*n));
            ordered.extend(error_rows.by_ref().take(4));
        }
        out.push(table(
            "Tests of Within-Subjects Effects",
            2,
            columns,
            ordered,
            None,
        ));
        // Tests of Within-Subjects Contrasts
        let mut columns = plain(&[
            "Source",
            "",
            "Sum of Squares",
            "df",
            "Mean Square",
            "F",
            "Sig.",
        ]);
        columns[2].1 = format!("Type {} Sum of Squares", roman(command.sstype));
        if flags.etasq {
            columns.extend(plain(&["Partial Eta Squared"]));
        }
        if flags.opower {
            columns.extend(plain(&["Noncent. Parameter", "Observed Power"]));
        }
        let mut last_label = String::new();
        for (label, cname, ss, df, sse, dfe) in &contrast_sources {
            let first = *label != last_label;
            last_label = label.clone();
            let mut row = vec![
                if first {
                    text(label.clone())
                } else {
                    Cell::Empty
                },
                text(cname.clone()),
                number(Some(*ss)),
                number(Some(*df)),
                number(Some(ss / df)),
            ];
            if sse.is_nan() {
                row.extend([Cell::Empty, Cell::Empty]);
                if flags.etasq {
                    row.push(Cell::Empty);
                }
                if flags.opower {
                    row.extend([Cell::Empty, Cell::Empty]);
                }
            } else {
                let f = (ss / df) / (sse / dfe);
                row.push(number(Some(f)));
                row.push(number(Some(f_p(f, *df, *dfe))));
                if flags.etasq {
                    row.push(number(Some(ss / (ss + sse))));
                }
                if flags.opower {
                    let (lambda, power) = power_f(f, *df, *dfe, flags.alpha);
                    row.push(number(Some(lambda)));
                    row.push(number(Some(power)));
                }
            }
            contrast_table_rows.push(row);
        }
        out.push(table(
            "Tests of Within-Subjects Contrasts",
            2,
            columns,
            contrast_table_rows,
            None,
        ));
        // Tests of Between-Subjects Effects on the average
        let m_b = kronecker(
            &ws.iter()
                .map(|w| vec![vec![1.0 / (w.levels as f64).sqrt()]; w.levels])
                .collect::<Vec<_>>(),
        );
        let sse_b = quadratic(&m_b, &fit.s)[0][0];
        let mut sources = Vec::new();
        for (e, (sh, rank)) in fit.effects.iter().zip(&sscp) {
            sources.push(Source {
                label: e.label.clone(),
                ss: quadratic(&m_b, sh)[0][0],
                df: *rank as f64,
                tested: true,
            });
        }
        sources.push(Source {
            label: "Error".into(),
            ss: sse_b,
            df: dfe,
            tested: false,
        });
        let mut between = f_table(
            "Tests of Between-Subjects Effects",
            &sources,
            sse_b,
            dfe,
            &flags,
            "Source",
            "Transformed Variable: Average.".into(),
        );
        between.columns[1] = format!("Type {} Sum of Squares", roman(command.sstype));
        out.push(between);
        if !command.emmeans.is_empty() {
            warnings.push("EMMEANS with WSFACTOR is not supported yet; it is left out".into());
        }
        Ok(out)
    }
}

/// One table from the like tables of several dependent variables (or transformed variables),
/// with a column `header` naming them: after the source column with the sources outermost
/// (`by_source`), or first with the variables outermost. The caption is the first table's
/// without its "Dependent Variable" sentence.
fn merge(tables: Vec<Table>, names: &[String], by_source: bool, header: &str) -> Table {
    let at = usize::from(by_source);
    let mut out = tables[0].clone();
    out.columns.insert(at, header.into());
    out.column_groups.insert(at, Vec::new());
    out.row_headers += 1;
    out.rows.clear();
    out.formats.clear();
    let with_name = |row: &[Cell], name: Cell| {
        let mut row = row.to_vec();
        row.insert(at, name);
        row
    };
    let name = |j: usize| text(names[j].clone());
    if by_source {
        for k in 0..tables[0].rows.len() {
            for (j, t) in tables.iter().enumerate() {
                let mut row = with_name(&t.rows[k], name(j));
                if j > 0 {
                    row[0] = Cell::Empty;
                }
                out.rows.push(row);
            }
        }
    } else {
        for (j, t) in tables.iter().enumerate() {
            for (k, row) in t.rows.iter().enumerate() {
                out.rows
                    .push(with_name(row, if k == 0 { name(j) } else { Cell::Empty }));
            }
        }
    }
    let sentence = format!("Dependent Variable: {}.", names[0]);
    out.caption = tables[0]
        .caption
        .as_ref()
        .map(|c| c.replace(&sentence, "").trim().to_string())
        .filter(|c| !c.is_empty());
    out
}

fn multivariate_columns(flags: &Flags, effect: bool) -> Vec<(String, String)> {
    let mut columns = if effect {
        plain(&["Effect", ""])
    } else {
        plain(&[""])
    };
    columns.extend(plain(&["Value", "F", "Hypothesis df", "Error df", "Sig."]));
    if flags.etasq {
        columns.extend(plain(&["Partial Eta Squared"]));
    }
    if flags.opower {
        columns.extend(plain(&["Noncent. Parameter", "Observed Power"]));
    }
    columns
}

/// The rows of the four multivariate statistics of S_H against S_E, with `l` hypothesis and
/// `ne` error degrees of freedom; empty when the hypothesis has no degrees of freedom.
fn multivariate_rows(sh: &Matrix, se: &Matrix, l: f64, ne: f64, flags: &Flags) -> Vec<Vec<Cell>> {
    multivariate(sh, se, l, ne)
        .into_iter()
        .map(|(stat, value, f, df1, df2, eta)| {
            let ok = l > 0.0 && df1 > 0.0 && df2 > 0.0 && f.is_finite();
            let mut row = vec![
                text(stat),
                number((l > 0.0 && value.is_finite()).then_some(value)),
                number(ok.then_some(f)),
                number(ok.then_some(df1)),
                number(ok.then_some(df2)),
                number(ok.then(|| f_p(f, df1, df2))),
            ];
            if flags.etasq {
                row.push(number(ok.then_some(eta)));
            }
            if flags.opower {
                let lambda = df2 * eta / (1.0 - eta);
                let power = ok.then(|| {
                    let crit = f_quantile(flags.alpha, df1, df2);
                    1.0 - noncentral_f_cdf(crit, df1, df2, lambda)
                });
                row.push(number(ok.then_some(lambda)));
                row.push(number(power));
            }
            row
        })
        .collect()
}

fn multivariate_note(flags: &Flags) -> String {
    format!(
        " F is exact when the smaller of the hypothesis df and the number of variables is 1 (for Wilks' lambda also when it is 2), else approximate; Roy's F is then an upper bound, so its Sig. is a lower bound.{}",
        power_note(flags)
    )
}

/// The log determinant of a symmetric matrix and its rank, from its eigenvalues (the log is
/// meaningful only at full rank).
fn log_determinant_and_rank(a: &Matrix) -> (f64, usize) {
    let values: Vec<f64> = crate::matrix::symmetric_eigen(a)
        .into_iter()
        .map(|(v, _)| v)
        .collect();
    let top = values.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let rank = values.iter().filter(|v| **v > 1e-10 * top).count();
    (values.iter().map(|v| v.ln()).sum(), rank)
}

/// Box's M test that the covariance matrices of the dependent variables are equal in the cells
/// of all factors, from the observed cell means. A cell with a singular covariance matrix is
/// left out; None when fewer than two cells remain or the pooled matrix is singular.
fn box_m(d: &Data, design: &str) -> Option<Table> {
    let r = d.y.first()?.len();
    let mut cells: Vec<Vec<usize>> = d.level.clone();
    cells.sort();
    cells.dedup();
    let mut kept: Vec<(f64, Matrix, f64)> = Vec::new();
    for cell in &cells {
        let members: Vec<usize> = (0..d.rows()).filter(|&i| d.level[i] == *cell).collect();
        let n: f64 = members.iter().map(|&i| d.f[i]).sum();
        if n <= 1.0 {
            continue;
        }
        let mean: Vec<f64> = (0..r)
            .map(|a| members.iter().map(|&i| d.f[i] * d.y[i][a]).sum::<f64>() / n)
            .collect();
        let s: Matrix = (0..r)
            .map(|a| {
                (0..r)
                    .map(|b| {
                        members
                            .iter()
                            .map(|&i| d.w(i) * (d.y[i][a] - mean[a]) * (d.y[i][b] - mean[b]))
                            .sum::<f64>()
                            / (n - 1.0)
                    })
                    .collect()
            })
            .collect();
        let (log_det, rank) = log_determinant_and_rank(&s);
        if rank == r {
            kept.push((n, s, log_det));
        }
    }
    let g = kept.len() as f64;
    let n: f64 = kept.iter().map(|k| k.0).sum();
    if g < 2.0 || n <= g {
        return None;
    }
    let pooled: Matrix = (0..r)
        .map(|a| {
            (0..r)
                .map(|b| kept.iter().map(|k| (k.0 - 1.0) * k.1[a][b]).sum::<f64>() / (n - g))
                .collect()
        })
        .collect();
    let (log_det, rank) = log_determinant_and_rank(&pooled);
    if rank < r {
        return None;
    }
    let m = (n - g) * log_det - kept.iter().map(|k| (k.0 - 1.0) * k.2).sum::<f64>();
    let r = r as f64;
    let f1 = (g - 1.0) * r * (r + 1.0) / 2.0;
    let inverse_sum: f64 = kept.iter().map(|k| 1.0 / (k.0 - 1.0)).sum::<f64>() - 1.0 / (n - g);
    let square_sum: f64 =
        kept.iter().map(|k| 1.0 / (k.0 - 1.0).powi(2)).sum::<f64>() - 1.0 / (n - g).powi(2);
    let rho = 1.0 - (2.0 * r * r + 3.0 * r - 1.0) / (6.0 * (r + 1.0) * (g - 1.0)) * inverse_sum;
    let tau = (r - 1.0) * (r + 2.0) / (6.0 * (g - 1.0)) * square_sum;
    let f2 = (f1 + 2.0) / (tau - (1.0 - rho).powi(2)).abs();
    let gamma = (rho - f1 / f2) / f1;
    let f = gamma * m;
    let rows = vec![
        vec![text("Box's M"), number(Some(m))],
        vec![text("F"), number(f.is_finite().then_some(f))],
        vec![text("df1"), number(Some(f1))],
        vec![text("df2"), number(f2.is_finite().then_some(f2))],
        vec![
            text("Sig."),
            number((f.is_finite() && f2.is_finite()).then(|| f_p(f, f1, f2))),
        ],
    ];
    Some(table(
        "Box's Test of Equality of Covariance Matrices",
        1,
        plain(&["", "Value"]),
        rows,
        Some(format!(
            "Tests whether the covariance matrix of the dependent variables is the same in every cell. Design: {design}."
        )),
    ))
}

/// Residual SSCP Matrix (sums of squares and cross-products, covariance, correlation) and
/// Bartlett's test of sphericity of the residual covariance matrix.
fn residual_tables(fit: &Fit, names: &[String], design: &str) -> Vec<Table> {
    let dfe = fit.df_error();
    let s = &fit.s;
    let mut columns = plain(&["", ""]);
    columns.extend(names.iter().map(|n| (String::new(), n.clone())));
    let mut rows = Vec::new();
    let scaled = |f: &dyn Fn(usize, usize) -> f64| -> Matrix {
        (0..s.len())
            .map(|a| (0..s.len()).map(|b| s[a][b] / f(a, b)).collect())
            .collect()
    };
    let blocks = [
        ("Sum-of-Squares and Cross-Products", s.clone()),
        ("Covariance", scaled(&|_, _| dfe)),
        ("Correlation", scaled(&|a, b| (s[a][a] * s[b][b]).sqrt())),
    ];
    for (block, m) in blocks {
        for (a, (name, values)) in names.iter().zip(&m).enumerate() {
            let mut row = vec![
                if a == 0 { text(block) } else { Cell::Empty },
                text(name.clone()),
            ];
            row.extend(
                values
                    .iter()
                    .map(|v| number(Some(*v).filter(|v| v.is_finite()))),
            );
            rows.push(row);
        }
    }
    let residual = table("Residual SSCP Matrix", 2, columns, rows, None);
    let (w, chi, df, sig) = mauchly(s, dfe);
    let fine = w > 0.0 && dfe > 0.0;
    let rows = vec![
        vec![
            text("Likelihood Ratio"),
            number(fine.then(|| (fit.n / 2.0 * w.ln()).exp())),
        ],
        vec![text("Approx. Chi-Square"), number(fine.then_some(chi))],
        vec![text("df"), number(Some(df))],
        vec![text("Sig."), number(fine.then_some(sig))],
    ];
    let bartlett = table(
        "Bartlett's Test of Sphericity",
        1,
        plain(&["", "Value"]),
        rows,
        Some(format!(
            "Tests whether the residual covariance matrix is a multiple of the identity matrix. Design: {design}."
        )),
    );
    vec![residual, bartlett]
}

/// Between-Subjects SSCP Matrix: the hypothesis SSCP of every effect and the error SSCP.
fn sscp_table(fit: &Fit, sscp: &[(Matrix, usize)], names: &[String], sstype: u8) -> Table {
    let r = names.len();
    let mut columns = plain(&["", "", ""]);
    columns.extend(names.iter().map(|n| (String::new(), n.clone())));
    let mut rows = Vec::new();
    let blocks = fit
        .effects
        .iter()
        .zip(sscp)
        .map(|(e, (m, _))| ("Hypothesis", e.label.as_str(), m))
        .chain(std::iter::once(("Error", "", &fit.s)));
    let mut previous = "";
    for (kind, label, m) in blocks {
        for a in 0..r {
            let mut row = vec![
                if a == 0 && kind != previous {
                    text(kind)
                } else {
                    Cell::Empty
                },
                if a == 0 { text(label) } else { Cell::Empty },
                text(names[a].clone()),
            ];
            row.extend((0..r).map(|b| number(Some(m[a][b]))));
            rows.push(row);
        }
        previous = kind;
    }
    table(
        "Between-Subjects SSCP Matrix",
        3,
        columns,
        rows,
        Some(format!("Based on Type {} Sum of Squares.", roman(sstype))),
    )
}

/// The M matrix of MMATRIX (one row per dependent) with the names of its columns.
fn m_matrix(command: &Glm, dependents: &[String]) -> Result<Option<(Matrix, Vec<String>)>, Error> {
    let Some(columns) = &command.mmatrix else {
        return Ok(None);
    };
    let r = dependents.len();
    let mut m = vec![vec![0.0; columns.len()]; r];
    let mut names = Vec::new();
    for (c, column) in columns.iter().enumerate() {
        if column.entries.len() > 1 && column.entries.iter().any(|(n, _)| n.is_none()) {
            return Err(Error(
                "ALL in MMATRIX gives a whole column; end the column with ';' before naming dependents"
                    .into(),
            ));
        }
        for (name, values) in &column.entries {
            match name {
                Some(name) => {
                    let Some(j) = dependents.iter().position(|d| d.eq_ignore_ascii_case(name))
                    else {
                        return Err(Error(format!(
                            "MMATRIX names {name}, which is not a dependent variable"
                        )));
                    };
                    let twice = column
                        .entries
                        .iter()
                        .filter(|(n, _)| n.as_ref().is_some_and(|n| n.eq_ignore_ascii_case(name)))
                        .count()
                        > 1;
                    if twice {
                        return Err(Error(format!("MMATRIX names {name} twice in one column")));
                    }
                    m[j][c] = values[0];
                }
                None if values.len() == r => {
                    for (row, v) in m.iter_mut().zip(values) {
                        row[c] = *v;
                    }
                }
                None => {
                    return Err(Error(format!(
                        "ALL in MMATRIX needs {r} values, one per dependent variable, but has {}",
                        values.len()
                    )));
                }
            }
        }
        let name = column
            .label
            .clone()
            .unwrap_or_else(|| format!("T{}", c + 1));
        if m.iter().all(|row| row[c] == 0.0) {
            return Err(Error(format!(
                "MMATRIX column {name} has no coefficient other than 0"
            )));
        }
        names.push(name);
    }
    Ok(Some((m, names)))
}

impl Engine {
    /// The multivariate model: several dependent variables without WSFACTOR.
    #[allow(clippy::too_many_arguments)]
    fn multivariate_model(
        &mut self,
        command: &Glm,
        d: &Data,
        effects: Vec<Effect>,
        dependents: &[String],
        texts: &[String],
        cases: usize,
        label_of: &dyn Fn(usize, usize) -> String,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<Table>, Error> {
        let flags = flags_of(command);
        let given_m = m_matrix(command, dependents)?;
        if command.save.iter().any(|(_, name)| name.is_some()) {
            return Err(Error(format!(
                "{}: SAVE with a variable name is not supported yet with several dependent variables; leave the name out",
                command.command
            )));
        }
        let fit = Fit::new(d, effects.clone(), &d.y, command.eps);
        // each dependent alone gives the univariate tables
        let parts: Vec<(Data, Fit)> = (0..texts.len())
            .map(|j| {
                let dj = d.only(j);
                let fj = Fit::new(&dj, effects.clone(), &dj.y, command.eps);
                (dj, fj)
            })
            .collect();
        let dfe = fit.df_error();
        let design = fit
            .effects
            .iter()
            .map(|e| e.label.clone())
            .collect::<Vec<_>>()
            .join(" + ");
        let mut out = Vec::new();
        if !d.factors.is_empty() {
            out.push(between_factors(d));
        }
        if command.print.descriptive {
            for (j, dep) in texts.iter().enumerate() {
                out.push(descriptive_table(d, j, dep, label_of));
            }
        }
        if command.print.homogeneity {
            match box_m(d, &design) {
                Some(t) => out.push(t),
                None => warnings.push(
                    "Box's M test needs two cells or more whose covariance matrix is not singular"
                        .into(),
                ),
            }
        }
        let sscp = effect_sscp(&fit, command.sstype);
        let mut rows = Vec::new();
        for (e, (sh, l)) in fit.effects.iter().zip(&sscp) {
            for (i, mut row) in multivariate_rows(sh, &fit.s, *l as f64, dfe, &flags)
                .into_iter()
                .enumerate()
            {
                row.insert(
                    0,
                    if i == 0 {
                        text(e.label.clone())
                    } else {
                        Cell::Empty
                    },
                );
                rows.push(row);
            }
        }
        out.push(table(
            "Multivariate Tests",
            2,
            multivariate_columns(&flags, true),
            rows,
            Some(format!("Design: {design}.{}", multivariate_note(&flags))),
        ));
        if command.print.homogeneity {
            let tables: Option<Vec<Table>> = parts.iter().map(|(dj, fj)| levene(dj, fj)).collect();
            match tables {
                Some(tables) => out.push(merge(tables, texts, false, "Dependent Variable")),
                None => warnings.push("Levene's test needs at least one factor".into()),
            }
        }
        let (tables, r2): (Vec<Table>, Vec<String>) = parts
            .iter()
            .map(|(dj, fj)| between_subjects(command, dj, fj, &flags))
            .unzip();
        let mut between = merge(tables, texts, true, "Dependent Variable");
        between.caption = Some(format!(
            "Type {} sums of squares. {}.{}",
            roman(command.sstype),
            texts
                .iter()
                .zip(&r2)
                .map(|(n, r)| format!("{n}: {r}"))
                .collect::<Vec<_>>()
                .join("; "),
            power_note(&flags)
        ));
        out.push(between);
        if command.print.parameter {
            let tables = parts
                .iter()
                .zip(texts)
                .map(|((dj, fj), dep)| {
                    parameter_table(command, dj, fj, dep, &flags, fj.s[0][0] / dfe, dfe)
                })
                .collect();
            out.push(merge(tables, texts, false, "Dependent Variable"));
        }
        if command.print.lof {
            let tables = parts
                .iter()
                .zip(texts)
                .map(|((dj, fj), dep)| lack_of_fit(dj, fj, dep, &flags))
                .collect();
            out.push(merge(tables, texts, true, "Dependent Variable"));
        }
        if command.print.sscp {
            out.push(sscp_table(&fit, &sscp, texts, command.sstype));
        }
        if command.print.rsscp {
            out.extend(residual_tables(&fit, texts, &design));
        }
        if let (true, Some((m, names))) = (command.print.mmatrix, &given_m) {
            let mut columns = plain(&["Dependent Variable"]);
            columns.extend(
                names
                    .iter()
                    .map(|n| ("Transformed Variable".into(), n.clone())),
            );
            let rows = texts
                .iter()
                .zip(m)
                .map(|(dep, row)| {
                    let mut cells = vec![text(dep.clone())];
                    cells.extend(row.iter().map(|v| number(Some(*v))));
                    cells
                })
                .collect();
            out.push(table(
                "Transformation Coefficients (M Matrix)",
                1,
                columns,
                rows,
                None,
            ));
        }
        out.extend(custom_hypotheses(
            command,
            d,
            &fit,
            given_m.as_ref(),
            texts,
            &flags,
            warnings,
        )?);
        for ((dj, fj), dep) in parts.iter().zip(texts) {
            out.extend(means_and_posthoc(
                command, dj, fj, dep, &flags, label_of, warnings,
            )?);
        }
        for ((dj, fj), dep) in parts.iter().zip(texts) {
            let charts = plots(command, dj, fj, dep, label_of)?;
            self.charts.extend(charts);
        }
        if !command.save.is_empty() {
            for ((dj, fj), name) in parts.iter().zip(dependents) {
                self.save_glm(command, dj, fj, name, cases)?;
            }
        }
        Ok(out)
    }
}
