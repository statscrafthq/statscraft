//! CSGLM, CSLOGISTIC and CSORDINAL: linear, logistic (binary and multinomial) and cumulative
//! link models on a complex sample, with design-based (Taylor linearization) variances.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - Cases: the design cases (see the parent module) whose dependent, factors and covariates are
//!   valid; CLASSMISSING=INCLUDE keeps user-missing factor and categorical dependent values.
//!   DOMAIN keeps the cases with the value; the cases left out, by the domain or by a missing
//!   model value, stay in the design with zero contributions, so df and the variance use the
//!   whole design.
//! - The design matrix is the over-parameterized one of UNIANOVA: the intercept, one column per
//!   level combination of each effect's factors (those that occur) times its covariates.
//!   Without MODEL: the intercept, the factors and the covariates. Sweeping X'WX in column order
//!   finds the redundant columns (SINGULAR); their parameters are 0.
//! - CSGLM: b = (X'WX)^- X'Wy. The score of case i is d_i = x_i (y_i - x_i'b); the covariance
//!   of b is G V(sum w d) G, G the generalized inverse. R square is 1 - r'Wr over the weighted
//!   sum of squares about the weighted mean (about 0 without intercept).
//! - CSLOGISTIC: each category but the reference has a logit against it with its own
//!   parameters. The estimates maximize the weighted log likelihood l = sum w log p(y) by
//!   Newton-Raphson with step-halving (MXSTEP step sizes 1, 1/2, 1/4, ...); they start at 0 with each
//!   intercept at log(N_k / N_ref), N_k the weighted count of category k. Stop when the
//!   parameters change by less than PCONVERGE (relative: over |b| + 1e-6) or the log likelihood
//!   by less than LCONVERGE (relative: over |l| + 1e-6), only criteria above 0 counting, or
//!   after MXITER steps. The score is d_i = (y_i - p_i) (x) x_i over the logits, the covariance
//!   J^- V(sum w d) J^-, J = sum w (diag p - p p') (x) x x'.
//! - The SRS covariance of the score total is fpc N/(n - 1) sum w d d' over the design cases
//!   (d about its weighted mean, which is 0 at a converged fit),
//!   fpc = 1 - n/N under SRSESTIMATOR WOR (when n/N < 1) and 1 under WR; the design effect of a
//!   parameter is its variance over its SRS variance.
//! - df is first-stage units minus first-stage strata, or CRITERIA DF. Intervals b +- t SE.
//! - Tests of L b = K: Wald X2 = (Lb - K)' (L V L')^- (Lb - K) with r = rank(L V L'); Wald F =
//!   (df - r + 1) / (r df) X2 on r and df - r + 1 (undefined when df < r); the adjusted tests
//!   use X2s from the SRS covariance and D = (L Vs L')^- (L V L'): adjusted F = X2s / tr D on d
//!   and d df with d = tr(D)^2 / tr(D^2), adjusted chi-square d times that on d df.
//! - Model effects use Type III L matrices (block diagonal over the logits); the corrected
//!   model tests every parameter but the intercept. A single row gives the same value under
//!   every test type, on 1 and df (or 1 df). PADJUST adjusts the p values of the single rows
//!   of CUSTOM and EMMEANS, R being the tested rows that are not multiples of an earlier one:
//!   Bonferroni R p, Sidak 1 - (1 - p)^R, their sequential forms over the sorted p values with
//!   R - i + 1 and a running maximum; every result at most 1.
//! - Pseudo R squares from l of the model and l0 of the start (the intercept-only fit, or every
//!   category equally likely without intercept), N the summed weights of the analysis cases:
//!   Cox and Snell 1 - exp(-2 (l - l0) / N), Nagelkerke that over 1 - exp(2 l0 / N), McFadden
//!   1 - l / l0. The classification table sums the weights of each observed and most probable
//!   category (the first on a tie).
//! - Odds ratios exp((x1 - x2)' b_k) with the interval from (x1 - x2)' b_k +- t SE; x1 and x2
//!   differ in the factor level or the covariate change, the other variables at CONTROL
//!   (factors at their highest level, covariates at their weighted means by default).
//! - CSORDINAL (the fit in `ordinal`): the design has its intercept only to find the redundant
//!   columns; the K - 1 thresholds take its place and sit after the regression parameters, shown
//!   first. Effects are tested on their regression parameters; the corrected model tests all.
//! - PRINT GEF: L b is estimable when L = L H, H = (X'WX)^- X'WX, so the rows of H at the
//!   columns that are not redundant span every estimable L; the table shows them transposed, a
//!   column (L1, L2, ...) per such row. Logits repeat the block; CSORDINAL puts one per threshold
//!   first, carrying minus the intercept's row on the redundant columns.
//! - OUTFILE COVB / CORB writes RowType_, VarName_ and P1, P2, ... (one per parameter, in the
//!   order of the Parameter Estimates, labelled with the parameter): rows EST, SE, SIG (of the
//!   t test) and DF, then one COV or CORR row per parameter. MODEL / PARAMETER write our own
//!   JSON with the estimates (and for MODEL the covariances).

use super::{Design, keys_of, numbers};
use crate::contrast::contrast_rows;
use crate::crosstabs::Key;
use crate::descriptives::number;
use crate::dist::{chi_square_p, f_p, t_p, t_quantile};
use crate::genlin::value_matches;
use crate::glm::{
    Data, Fit, Part, combinations, covariate_note, covariate_values, effects_of, emmeans_l,
    gef_basis, ginverse, ginverse_scaled, intercept_term, key_value_text, l_row,
};
use crate::matrix::{Matrix, inverse_scaled, multiply, transpose};
use crate::npar_tests::text;
use crate::plum::{cdf, cell_probabilities, link_name};
use crate::regression::{plain, table};
use crate::shown::{Show, Shown};
use crate::{Cell, Engine, Error, Table};
use serde_json::json;
use statscraft_data::{Dataset, Measure, Value as DataValue, Variable};
use statscraft_syntax::{
    Contrast, CsContrast, CsControl, CsIntercept, CsModel, CsModelSave, CsModelStatistics,
    CsPadjust, CsReference, CsTestType, LRow, PlumLink, Term, Value,
};

mod cox;
mod ordinal;

/// The parameters of the model with their covariances; with several logits, logit by logit.
struct Estimates {
    b: Vec<f64>,
    v: Matrix,
    /// The covariance under simple random sampling, when defined.
    vs: Option<Matrix>,
    redundant: Vec<bool>,
    /// Parameters per logit (the columns of the design).
    p: usize,
    logits: usize,
}

impl Estimates {
    fn len(&self) -> usize {
        self.b.len()
    }

    /// A row over the parameters of one logit spread to every parameter.
    fn spread(&self, row: &[f64], logit: usize) -> Vec<f64> {
        let mut out = vec![0.0; self.len()];
        out[logit * self.p..(logit + 1) * self.p].copy_from_slice(row);
        out
    }

    fn quad(&self, m: &Matrix, l: &[f64]) -> f64 {
        (0..l.len())
            .filter(|&i| l[i] != 0.0)
            .map(|i| l[i] * (0..l.len()).map(|j| m[i][j] * l[j]).sum::<f64>())
            .sum()
    }

    fn estimate(&self, l: &[f64]) -> f64 {
        l.iter().zip(&self.b).map(|(a, b)| a * b).sum()
    }
}

/// A Wald test of L b = K.
struct Wald {
    x2: f64,
    r: usize,
    /// The adjusted F and its d, when the SRS covariance is defined.
    adjusted: Option<(f64, f64)>,
}

/// The General Estimable Function table: a row per parameter (in `order`), a column per free
/// coefficient; a cell is the coefficient of the parameter as a combination of L1, L2, ...
fn gef_table(
    basis: &Matrix,
    order: &[usize],
    label: &dyn Fn(usize) -> String,
    caption: String,
) -> Table {
    let mut cols = plain(&["Parameter"]);
    cols.extend((1..=basis.len()).map(|k| (String::new(), format!("L{k}"))));
    let out = order
        .iter()
        .map(|&a| {
            let mut row = vec![text(label(a))];
            row.extend(basis.iter().map(|l| Cell::Number(l[a])));
            row
        })
        .collect();
    table(
        "General Estimable Function",
        1,
        cols,
        out,
        Some(format!(
            "{caption} Each column is a free coefficient; a parameter's coefficient in any estimable function is the combination of them in its row."
        )),
    )
}

fn trace(m: &Matrix) -> f64 {
    (0..m.len()).map(|i| m[i][i]).sum()
}

fn wald(est: &Estimates, l: &Matrix, k: &[f64]) -> Option<Wald> {
    if l.is_empty() {
        return None;
    }
    let diff: Vec<f64> = l
        .iter()
        .zip(k)
        .map(|(row, k)| est.estimate(row) - k)
        .collect();
    let lt = transpose(l);
    let a = multiply(&multiply(l, &est.v), &lt);
    let (ainv, r) = ginverse_scaled(&a, 1e-10);
    if r == 0 {
        return None;
    }
    let column: Matrix = diff.iter().map(|v| vec![*v]).collect();
    let form = |m: &Matrix| multiply(&multiply(&transpose(&column), m), &column)[0][0];
    let x2 = form(&ainv);
    let adjusted = est.vs.as_ref().and_then(|vs| {
        let s = multiply(&multiply(l, vs), &lt);
        let (sinv, _) = ginverse_scaled(&s, 1e-10);
        let x2s = form(&sinv);
        let delta = multiply(&sinv, &a);
        let tr = trace(&delta);
        let tr2 = trace(&multiply(&delta, &delta));
        (tr > 0.0 && tr2 > 0.0).then(|| (x2s / tr, tr * tr / tr2))
    });
    Some(Wald { x2, r, adjusted })
}

fn test_columns(t: CsTestType) -> Vec<(String, String)> {
    match t {
        CsTestType::F => plain(&["df1", "df2", "Wald F", "Sig."]),
        CsTestType::AdjF => plain(&["df1", "df2", "Adjusted Wald F", "Sig."]),
        CsTestType::ChiSquare => plain(&["Wald Chi-Square", "df", "Sig."]),
        CsTestType::AdjChiSquare => plain(&["Adjusted Wald Chi-Square", "df", "Sig."]),
    }
}

/// The statistics of a test in the order of `test_columns` without the p value, and the p value.
fn test_values(t: CsTestType, w: &Wald, df: f64) -> Option<(Vec<f64>, f64)> {
    let r = w.r as f64;
    match t {
        CsTestType::F => (df >= r).then(|| {
            let f = (df - r + 1.0) / (r * df) * w.x2;
            (vec![r, df - r + 1.0, f], f_p(f, r, df - r + 1.0))
        }),
        CsTestType::ChiSquare => Some((vec![w.x2, r], chi_square_p(w.x2, r))),
        // a single row: the adjusted test is the Wald test, defined without the SRS covariance
        CsTestType::AdjF => w
            .adjusted
            .or((w.r == 1).then_some((w.x2, 1.0)))
            .map(|(f, d)| (vec![d, d * df, f], f_p(f, d, d * df))),
        CsTestType::AdjChiSquare => w
            .adjusted
            .or((w.r == 1).then_some((w.x2, 1.0)))
            .map(|(f, d)| (vec![f * d, d], chi_square_p(f * d, d))),
    }
}

/// The test cells, with `p` replacing the p value when given (an adjusted one).
fn test_cells(t: CsTestType, w: Option<&Wald>, df: f64, p: Option<f64>) -> Vec<Cell> {
    let width = test_columns(t).len();
    match w.and_then(|w| test_values(t, w, df)) {
        Some((values, sig)) => values
            .into_iter()
            .map(|v| number(Some(v)))
            .chain([number(Some(p.unwrap_or(sig)))])
            .collect(),
        None => vec![Cell::Empty; width],
    }
}

/// The adjusted p values of the single rows. `rows` are the L rows with their K values, to find
/// the rows that are multiples of earlier ones.
fn padjust(method: CsPadjust, ps: &[Option<f64>], rows: &[(Vec<f64>, f64)]) -> Vec<Option<f64>> {
    let multiple = |a: &(Vec<f64>, f64), b: &(Vec<f64>, f64)| {
        let ratio =
            a.0.iter()
                .zip(&b.0)
                .find(|(_, y)| **y != 0.0)
                .map(|(x, y)| x / y);
        ratio.is_some_and(|c| {
            c != 0.0
                && a.0
                    .iter()
                    .chain([&a.1])
                    .zip(b.0.iter().chain([&b.1]))
                    .all(|(x, y)| (x - c * y).abs() <= 1e-10 * x.abs().max(1.0))
        })
    };
    // rows without a test do not count
    let tested: Vec<usize> = (0..rows.len()).filter(|&i| ps[i].is_some()).collect();
    let r = (0..tested.len())
        .filter(|&a| !(0..a).any(|b| multiple(&rows[tested[a]], &rows[tested[b]])))
        .count() as f64;
    let mut out: Vec<Option<f64>> = ps.to_vec();
    match method {
        CsPadjust::Lsd => {}
        CsPadjust::Bonferroni => {
            for p in out.iter_mut().flatten() {
                *p = (*p * r).min(1.0);
            }
        }
        CsPadjust::Sidak => {
            for p in out.iter_mut().flatten() {
                *p = (1.0 - (1.0 - *p).powf(r)).min(1.0);
            }
        }
        CsPadjust::SeqBonferroni | CsPadjust::SeqSidak => {
            let mut order: Vec<usize> = (0..ps.len()).filter(|&i| ps[i].is_some()).collect();
            order.sort_by(|&a, &b| ps[a].unwrap().total_cmp(&ps[b].unwrap()));
            let mut running: f64 = 0.0;
            for (i, &at) in order.iter().enumerate() {
                let p = ps[at].expect("kept");
                let m = (r - i as f64).max(1.0);
                let adjusted = if method == CsPadjust::SeqBonferroni {
                    m * p
                } else {
                    1.0 - (1.0 - p).powf(m)
                };
                running = running.max(adjusted).min(1.0);
                out[at] = Some(running);
            }
        }
    }
    out
}

fn padjust_note(method: CsPadjust) -> Option<&'static str> {
    match method {
        CsPadjust::Lsd => None,
        CsPadjust::Bonferroni => Some("Bonferroni"),
        CsPadjust::SeqBonferroni => Some("sequential Bonferroni"),
        CsPadjust::Sidak => Some("Sidak"),
        CsPadjust::SeqSidak => Some("sequential Sidak"),
    }
}

/// The level of `variable` that a value given in the syntax names; a quoted value may also be
/// the formatted text of a number.
fn level_of(variable: &Variable, levels: &[Key], value: &Value) -> Option<usize> {
    levels
        .iter()
        .position(|k| value_matches(variable, k, value))
}

/// The value as written, for messages.
fn value_text(value: &Value) -> String {
    match value {
        Value::Number(n) => n.to_string(),
        Value::String(s) => format!("'{s}'"),
    }
}

/// What the logistic fit gives besides the estimates.
struct Logistic {
    /// The category of every analysis row and its probabilities.
    probs: Vec<Vec<f64>>,
    ll: f64,
    ll0: f64,
    /// (iteration, -2 l, step-halvings, parameters) of every iteration.
    history: Vec<(usize, f64, usize, Vec<f64>)>,
}

/// Probabilities of every category for linear predictors `eta` of the non-reference ones.
fn softmax(eta: &[f64], base: usize, others: &[usize]) -> Vec<f64> {
    let top = eta.iter().copied().fold(0.0_f64, f64::max);
    let denominator = (-top).exp() + eta.iter().map(|e| (e - top).exp()).sum::<f64>();
    let mut out = vec![0.0; others.len() + 1];
    out[base] = (-top).exp() / denominator;
    for (l, &k) in others.iter().enumerate() {
        out[k] = (eta[l] - top).exp() / denominator;
    }
    out
}

/// The pieces of a logistic fit on the analysis rows.
struct LogitModel<'a> {
    x: &'a [Vec<f64>],
    w: &'a [f64],
    y: &'a [usize],
    cols: Vec<usize>,
    base: usize,
    others: Vec<usize>,
}

impl LogitModel<'_> {
    fn q(&self) -> usize {
        self.cols.len()
    }

    fn probs(&self, i: usize, b: &[f64]) -> Vec<f64> {
        let q = self.q();
        let eta: Vec<f64> = (0..self.others.len())
            .map(|l| {
                self.cols
                    .iter()
                    .enumerate()
                    .map(|(s, &c)| self.x[i][c] * b[l * q + s])
                    .sum()
            })
            .collect();
        softmax(&eta, self.base, &self.others)
    }

    fn log_likelihood(&self, b: &[f64]) -> f64 {
        (0..self.x.len())
            .map(|i| self.w[i] * self.probs(i, b)[self.y[i]].max(1e-300).ln())
            .sum()
    }

    /// The score (gradient) and the information J of the weighted log likelihood.
    fn derivatives(&self, b: &[f64]) -> (Vec<f64>, Matrix) {
        let q = self.q();
        let size = self.others.len() * q;
        let mut g = vec![0.0; size];
        let mut info = vec![vec![0.0; size]; size];
        for i in 0..self.x.len() {
            let p = self.probs(i, b);
            let xs: Vec<f64> = self.cols.iter().map(|&c| self.x[i][c]).collect();
            for (l, &k) in self.others.iter().enumerate() {
                let r = f64::from(u8::from(self.y[i] == k)) - p[k];
                for s in 0..q {
                    g[l * q + s] += self.w[i] * xs[s] * r;
                }
                for (m, &h) in self.others.iter().enumerate() {
                    let a = self.w[i] * p[k] * (f64::from(u8::from(l == m)) - p[h]);
                    for s in 0..q {
                        let as_ = a * xs[s];
                        if as_ == 0.0 {
                            continue;
                        }
                        for t in 0..q {
                            info[l * q + s][m * q + t] += as_ * xs[t];
                        }
                    }
                }
            }
        }
        (g, info)
    }
}

/// The inverse of the information matrix, or a generalized inverse when it is singular (with
/// one warning however often that happens).
fn information_inverse(name: &str, info: &Matrix, warnings: &mut Vec<String>) -> Matrix {
    match inverse_scaled(info) {
        Some((m, _)) => m,
        None => {
            let note = format!(
                "{name}: the information matrix is singular (perhaps separation); a generalized inverse is used and the estimates and standard errors may not be reliable"
            );
            if !warnings.contains(&note) {
                warnings.push(note);
            }
            ginverse(info, 1e-12).0
        }
    }
}

/// Fits the logistic model; returns the parameters over `cols`, logit by logit.
fn fit_logistic(
    m: &LogitModel,
    intercept: bool,
    c: &statscraft_syntax::CsCriteria,
    warnings: &mut Vec<String>,
) -> (Vec<f64>, Logistic) {
    let q = m.q();
    let mut b = vec![0.0; m.others.len() * q];
    if intercept && m.cols.first() == Some(&0) {
        let mut totals = vec![0.0; m.others.len() + 1];
        for (w, &y) in m.w.iter().zip(m.y) {
            totals[y] += w;
        }
        for (l, &k) in m.others.iter().enumerate() {
            if totals[k] > 0.0 && totals[m.base] > 0.0 {
                b[l * q] = (totals[k] / totals[m.base]).ln();
            }
        }
    }
    let ll0 = m.log_likelihood(&b);
    let mut ll = ll0;
    let mut history = vec![(0, -2.0 * ll, 0, b.clone())];
    let mut iteration = 0;
    while iteration < c.mxiter as usize && !b.is_empty() {
        let (g, info) = m.derivatives(&b);
        let inv = information_inverse("CSLOGISTIC", &info, warnings);
        let step: Vec<f64> = inv
            .iter()
            .map(|row| row.iter().zip(&g).map(|(a, v)| a * v).sum())
            .collect();
        let mut accepted = None;
        let mut factor = 1.0;
        for halvings in 0..c.mxstep.max(1) as usize {
            let candidate: Vec<f64> = b.iter().zip(&step).map(|(v, s)| v + factor * s).collect();
            let lc = m.log_likelihood(&candidate);
            if lc >= ll - 1e-12 * (1.0 + ll.abs()) {
                accepted = Some((candidate, lc, halvings));
                break;
            }
            factor /= 2.0;
        }
        iteration += 1;
        let Some((candidate, lc, halvings)) = accepted else {
            warnings.push(format!(
                "CSLOGISTIC: the log pseudo-likelihood did not increase with any of the {} step sizes (MXSTEP) at iteration {iteration}; the estimates before it are shown",
                c.mxstep
            ));
            break;
        };
        let (pc, relative_p) = c.pconverge;
        let change = b
            .iter()
            .zip(&candidate)
            .map(|(old, new)| {
                let d = (new - old).abs();
                if relative_p {
                    d / (old.abs() + 1e-6)
                } else {
                    d
                }
            })
            .fold(0.0_f64, f64::max);
        let (lcv, relative_l) = c.lconverge;
        let ll_change = if relative_l {
            (lc - ll).abs() / (ll.abs() + 1e-6)
        } else {
            (lc - ll).abs()
        };
        b = candidate;
        ll = lc;
        history.push((iteration, -2.0 * ll, halvings, b.clone()));
        if (pc > 0.0 && change < pc) || (lcv > 0.0 && ll_change < lcv) {
            break;
        }
        if c.chksep > 0 && iteration >= c.chksep as usize {
            let perfect = (0..m.x.len()).all(|i| m.probs(i, &b)[m.y[i]] > 1.0 - 1e-8);
            if perfect {
                warnings.push(format!(
                    "CSLOGISTIC: complete separation at iteration {iteration}: every case is predicted with certainty, so the estimates are not reliable"
                ));
                break;
            }
        }
        if iteration == c.mxiter as usize {
            warnings.push(format!(
                "CSLOGISTIC: the estimates did not converge within {} iterations (MXITER)",
                c.mxiter
            ));
        }
    }
    let probs = (0..m.x.len()).map(|i| m.probs(i, &b)).collect();
    (
        b,
        Logistic {
            probs,
            ll,
            ll0,
            history,
        },
    )
}

/// A column to save: its name (None: the default root with a suffix), label and values.
struct Saved {
    name: Option<String>,
    root: String,
    label: String,
    numbers: Option<Vec<Option<f64>>>,
    /// A string column with its width.
    texts: Option<(usize, Vec<String>)>,
    /// For a saved category: the dependent's format and value labels.
    category: Option<(String, Vec<(DataValue, String)>)>,
}

/// What OUTFILE writes: the matrix data and the model file.
type Files = (Option<Dataset>, Option<serde_json::Value>);

/// The OUTFILE COVB or CORB data of the parameters in `order`: rows EST, SE, SIG and DF, then one
/// COV or CORR row per parameter; a redundant parameter has its estimate 0 and nothing else.
fn matrix_file(
    est: &Estimates,
    order: &[usize],
    df: f64,
    corr: bool,
    label: &dyn Fn(usize) -> String,
) -> Result<Dataset, Error> {
    let ok = |a: usize| !est.redundant[a];
    let se = |a: usize| ok(a).then(|| est.v[a][a].max(0.0).sqrt());
    let mut rows: Vec<(&str, String, Vec<Option<f64>>)> = vec![
        (
            "EST",
            String::new(),
            order.iter().map(|&a| Some(est.b[a])).collect(),
        ),
        ("SE", String::new(), order.iter().map(|&a| se(a)).collect()),
        (
            "SIG",
            String::new(),
            order
                .iter()
                .map(|&a| {
                    se(a)
                        .filter(|&s| s > 0.0 && df > 0.0)
                        .map(|s| t_p(est.b[a] / s, df))
                })
                .collect(),
        ),
        (
            "DF",
            String::new(),
            order.iter().map(|&a| ok(a).then_some(df)).collect(),
        ),
    ];
    for (i, &a) in order.iter().enumerate() {
        let values = order
            .iter()
            .map(|&b| {
                let v = est.v[a][b];
                let v = if corr {
                    v / (est.v[a][a] * est.v[b][b]).sqrt()
                } else {
                    v
                };
                (ok(a) && ok(b) && v.is_finite()).then_some(v)
            })
            .collect();
        rows.push((
            if corr { "CORR" } else { "COV" },
            format!("P{}", i + 1),
            values,
        ));
    }
    let mut out = Dataset::new(rows.len());
    out.set_string_values(
        "RowType_",
        8,
        rows.iter().map(|r| r.0.to_string()).collect(),
    )?;
    out.set_string_values("VarName_", 8, rows.iter().map(|r| r.1.clone()).collect())?;
    for (i, &a) in order.iter().enumerate() {
        let var = format!("P{}", i + 1);
        out.set_numeric_values(&var, rows.iter().map(|r| r.2[i]).collect())?;
        out.set_variable_label(&var, Some(&label(a)))?;
    }
    Ok(out)
}

/// The parameter names of every parameter, with the category of its logit.
fn parameter_names(d: &Data, fit: &Fit) -> Vec<String> {
    let mut out = vec![String::new(); fit.p];
    for e in &fit.effects {
        for (k, cell) in e.cells.iter().enumerate() {
            out[e.start + k] = crate::glm::parameter_name(d, e, cell);
        }
    }
    out
}

impl Engine {
    pub(crate) fn cs_model(&mut self, c: &CsModel) -> Result<Vec<Table>, Error> {
        // a SAVE name that is taken stops the command before the fit
        let data = self.active()?;
        for s in &c.save {
            if let CsModelSave::Pred(Some(n))
            | CsModelSave::Resid(Some(n))
            | CsModelSave::PredVal(Some(n))
            | CsModelSave::PredValProb(Some(n))
            | CsModelSave::ObsValProb(Some(n)) = s
                && data.variable_index(n).is_some()
            {
                return Err(Error(format!(
                    "{} /SAVE: variable {n} already exists",
                    c.name()
                )));
            }
        }
        let mut warnings = Vec::new();
        let (tables, saved, cases, (matrix, model)) = self.cs_model_tables(c, &mut warnings)?;
        self.warn_weight_and_split(c.name());
        for w in warnings {
            self.warn(w);
        }
        if let (Some(data), Some((file, _))) = (matrix, &c.outfile.matrix) {
            self.write_data_file(file, data)?;
        }
        if let (Some(model), Some((file, _))) = (model, &c.outfile.model)
            && !self.dry_run
        {
            self.write_json(c.name(), file, "model", &model)?;
        }
        if !saved.is_empty() {
            self.cs_save(c.name(), saved, cases)?;
        }
        Ok(tables)
    }

    fn cs_save(&mut self, name: &str, saved: Vec<Saved>, cases: usize) -> Result<(), Error> {
        // names given in SAVE are kept free for their own columns
        let given: Vec<String> = saved
            .iter()
            .filter_map(|c| c.name.as_ref().map(|n| n.to_uppercase()))
            .collect();
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            for column in saved {
                let var = match column.name {
                    Some(n) => {
                        if data.variable_index(&n).is_some() {
                            return Err(Error(format!(
                                "{name} /SAVE: variable {n} already exists"
                            )));
                        }
                        n
                    }
                    None => {
                        let mut candidate = column.root.clone();
                        let mut k = 1;
                        while data.variable_index(&candidate).is_some()
                            || given.contains(&candidate.to_uppercase())
                        {
                            candidate = format!("{}_{k}", column.root);
                            k += 1;
                        }
                        candidate
                    }
                };
                if let Some(values) = column.numbers {
                    debug_assert_eq!(values.len(), cases);
                    data.set_numeric_values(&var, values)?;
                    match column.category {
                        Some((format, labels)) => {
                            data.set_format(&var, &format)?;
                            data.set_measure(&var, Measure::Nominal)?;
                            data.set_value_labels(&var, labels)?;
                        }
                        None => {
                            data.set_format(&var, "F11.5")?;
                            data.set_measure(&var, Measure::Scale)?;
                        }
                    }
                } else if let Some((width, texts)) = column.texts {
                    data.set_string_values(&var, width, texts)?;
                    if let Some((_, labels)) = column.category {
                        data.set_measure(&var, Measure::Nominal)?;
                        data.set_value_labels(&var, labels)?;
                    }
                }
                data.set_variable_label(&var, Some(&column.label))?;
            }
            Ok(())
        })
    }

    fn cs_model_tables(
        &self,
        c: &CsModel,
        warnings: &mut Vec<String>,
    ) -> Result<(Vec<Table>, Vec<Saved>, usize, Files), Error> {
        let name = c.name();
        let plan = self.read_plan(&c.plan)?;
        let joint = self.joint_file(&plan, &c.plan, c.jointprob.as_deref())?;
        let data = self.active()?;
        let shown = self.shown();
        let design = Design::new(data, &plan, c.include, false, joint.as_ref())?;
        let factor_names = self.expand(&c.factors)?;
        let covariate_names = self.expand(&c.covariates)?;
        let dep_var = data.variable(&c.dependent)?;
        let dependent = dep_var.name().to_string();
        let mut seen: Vec<String> = Vec::new();
        for n in std::iter::once(&dependent)
            .chain(&factor_names)
            .chain(&covariate_names)
        {
            let upper = n.to_uppercase();
            if seen.contains(&upper) {
                return Err(Error(format!(
                    "{n} appears more than once among the dependent variable, factors and covariates"
                )));
            }
            if design.plan_vars.contains(&upper) {
                return Err(Error(format!(
                    "{n} is a plan variable and cannot be in the model"
                )));
            }
            seen.push(upper);
        }
        if let Some((v, _)) = &c.domain {
            let upper = v.to_uppercase();
            if seen.contains(&upper) {
                return Err(Error(format!(
                    "{v} is in the model and cannot define the DOMAIN"
                )));
            }
            if design.plan_vars.contains(&upper) && !design.strata_clusters.contains(&upper) {
                return Err(Error(format!(
                    "{v} is a plan variable other than a stratum or cluster and cannot define the DOMAIN"
                )));
            }
        }
        let factor_vars: Vec<Variable> = factor_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        let cov_vars: Vec<Variable> = covariate_names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if !c.logistic && dep_var.is_string() {
            return Err(Error(format!(
                "CSGLM needs a numeric dependent variable, but {dependent} is a string variable"
            )));
        }
        if let Some(v) = cov_vars.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "{name} needs numeric covariates, but {} is a string variable",
                v.name()
            )));
        }
        let n = design.cases();
        let at = |values: Vec<Option<f64>>| -> Vec<Option<f64>> {
            design.rows.iter().map(|&r| values[r]).collect()
        };
        let keys_at = |v: &Variable| -> Vec<Option<Key>> {
            let all = keys_of(v, c.include);
            design.rows.iter().map(|&r| all[r].clone()).collect()
        };
        let dep_numbers = if c.logistic {
            Vec::new()
        } else {
            at(numbers(&dep_var)?)
        };
        let dep_keys = if c.logistic {
            keys_at(&dep_var)
        } else {
            Vec::new()
        };
        let factor_keys: Vec<Vec<Option<Key>>> = factor_vars.iter().map(keys_at).collect();
        let cov_values: Vec<Vec<Option<f64>>> = cov_vars
            .iter()
            .map(|v| Ok(at(numbers(v)?)))
            .collect::<Result<_, Error>>()?;
        let valid: Vec<bool> = (0..n)
            .map(|i| {
                (if c.logistic {
                    dep_keys[i].is_some()
                } else {
                    dep_numbers[i].is_some()
                }) && factor_keys.iter().all(|k| k[i].is_some())
                    && cov_values.iter().all(|v| v[i].is_some())
            })
            .collect();
        let in_domain: Vec<bool> = match &c.domain {
            None => vec![true; n],
            Some((v, value)) => {
                let variable = data.variable(v)?;
                keys_at(&variable)
                    .iter()
                    .map(|k| {
                        k.as_ref()
                            .is_some_and(|k| value_matches(&variable, k, value))
                    })
                    .collect()
            }
        };
        let rows: Vec<usize> = (0..n).filter(|&i| valid[i] && in_domain[i]).collect();
        if rows.is_empty() {
            return Err(Error(format!(
                "{name}: no case in the design{} has valid values on every model variable",
                if c.domain.is_some() {
                    " and the domain"
                } else {
                    ""
                }
            )));
        }
        let sorted_levels = |keys: &[Option<Key>]| -> Vec<Key> {
            let mut found: Vec<Key> = Vec::new();
            for &i in &rows {
                let key = keys[i].as_ref().expect("valid");
                if !found.iter().any(|l| l.compare(key).is_eq()) {
                    found.push(key.clone());
                }
            }
            found.sort_by(|a, b| a.compare(b));
            found
        };
        let levels: Vec<Vec<Key>> = factor_keys.iter().map(|k| sorted_levels(k)).collect();
        let position = |levels: &[Key], key: &Key| -> usize {
            levels
                .iter()
                .position(|l| l.compare(key).is_eq())
                .expect("a level")
        };
        let labels = Shown::labels(shown.currencies.clone());
        let w_rows: Vec<f64> = rows.iter().map(|&i| design.w[i]).collect();
        // the logistic categories, the reference and the category of every row
        let mut categories = if c.logistic {
            sorted_levels(&dep_keys)
        } else {
            Vec::new()
        };
        if c.ordinal.is_some_and(|o| o.descending) {
            categories.reverse();
        }
        if c.logistic && categories.len() < 2 {
            return Err(Error(format!(
                "{name}: the dependent variable {dependent} has fewer than two categories among the cases analyzed"
            )));
        }
        let base = match &c.reference {
            _ if !c.logistic => 0,
            CsReference::High => categories.len() - 1,
            CsReference::Low => 0,
            CsReference::Value(v) => match level_of(&dep_var, &categories, v) {
                Some(k) => k,
                None => {
                    warnings.push(format!(
                        "CSLOGISTIC: the reference category {} is not among the categories of {dependent}; the highest category is the reference",
                        value_text(v)
                    ));
                    categories.len() - 1
                }
            },
        };
        let category_of: Vec<usize> = if c.logistic {
            rows.iter()
                .map(|&i| position(&categories, dep_keys[i].as_ref().expect("valid")))
                .collect()
        } else {
            Vec::new()
        };
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
            labelled: factor_vars.iter().any(|v| !v.value_labels().is_empty())
                && shown.values != Show::Plain,
            covariates: covariate_names.clone(),
            case: rows.iter().map(|&i| design.rows[i]).collect(),
            level: rows
                .iter()
                .map(|&i| {
                    factor_keys
                        .iter()
                        .zip(&levels)
                        .map(|(k, ls)| position(ls, k[i].as_ref().expect("valid")))
                        .collect()
                })
                .collect(),
            cov: rows
                .iter()
                .map(|&i| cov_values.iter().map(|v| v[i].expect("valid")).collect())
                .collect(),
            y: rows
                .iter()
                .map(|&i| {
                    vec![if c.logistic {
                        0.0
                    } else {
                        dep_numbers[i].expect("valid")
                    }]
                })
                .collect(),
            f: w_rows.clone(),
            g: vec![1.0; rows.len()],
        };
        // the model terms
        let intercept = c.intercept != CsIntercept::No;
        let mut terms: Vec<Term> = Vec::new();
        if intercept {
            terms.push(intercept_term());
        }
        match (&c.model, c.intercept) {
            (_, CsIntercept::Only) => {}
            (Some(model), _) => terms.extend(model.iter().cloned()),
            (None, _) => {
                for v in factor_names.iter().chain(&covariate_names) {
                    terms.push(Term {
                        variables: vec![v.clone()],
                        label: v.clone(),
                    });
                }
            }
        }
        let mut effects = effects_of(&terms, &factor_names, &covariate_names)?;
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
            return Err(Error(format!("{name}: the model has no effects")));
        }
        let fit = Fit::new(&d, effects, &d.y, c.criteria.singular);
        let p = fit.p;
        let df = c.criteria.df.unwrap_or(design.df);
        let big_n = design.total_w;
        let srs_factor = if design.srs_wr {
            Some(1.0)
        } else if (n as f64) < big_n {
            Some(1.0 - n as f64 / big_n)
        } else {
            None
        }
        .filter(|_| n > 1)
        .map(|fpc| fpc * big_n / (n as f64 - 1.0));
        let wants_srs = matches!(c.test, CsTestType::AdjF | CsTestType::AdjChiSquare)
            || c.statistics.is_some_and(|s| s.deff || s.deffsqrt);
        if wants_srs && srs_factor.is_none() {
            warnings.push(format!(
                "{name}: the simple random sampling variance is not defined (without replacement it needs a sample smaller than the estimated population), so design effects and adjusted tests of several rows are left empty"
            ));
        }
        let srs_factor = srs_factor.filter(|_| wants_srs);
        // scores: one column per parameter (of the non-redundant columns for logistic) over the
        // design cases
        let srs_of = |us: &[Vec<f64>]| -> Option<Matrix> {
            srs_factor.map(|f| {
                // about the weighted mean score, which is 0 at a converged fit
                let k = us.len();
                let means: Vec<f64> = us
                    .iter()
                    .map(|u| u.iter().zip(&design.w).map(|(u, w)| u * w).sum::<f64>() / big_n)
                    .collect();
                // sum w (d - mean)(d - mean)' = sum w d d' - N mean mean', the sum skipping zeros
                let mut m = vec![vec![0.0; k]; k];
                for (i, w) in design.w.iter().enumerate() {
                    for a in 0..k {
                        if us[a][i] == 0.0 {
                            continue;
                        }
                        for b in 0..k {
                            m[a][b] += w * us[a][i] * us[b][i];
                        }
                    }
                }
                for a in 0..k {
                    for b in 0..k {
                        m[a][b] = f * (m[a][b] - big_n * means[a] * means[b]);
                    }
                }
                m
            })
        };
        let names = parameter_names(&d, &fit);
        let mut logistic: Option<Logistic> = None;
        // CSORDINAL NONPARALLEL: the general model's estimates (thresholds, then the coefficients
        // of each threshold over `cols`) and its regression columns
        let mut general: Option<(Estimates, Vec<usize>)> = None;
        let others: Vec<usize> = (0..categories.len()).filter(|&k| k != base).collect();
        let est = if let Some(o) = c.ordinal {
            // the regression columns: the non-redundant ones but the intercept
            let cols: Vec<usize> = (1..p).filter(|&j| !fit.redundant[j]).collect();
            let x: Vec<Vec<f64>> = fit
                .x
                .iter()
                .map(|row| cols.iter().map(|&j| row[j]).collect())
                .collect();
            let k = categories.len();
            let fit_of = |start: Option<&[f64]>, warnings: &mut Vec<String>| {
                ordinal::fit(
                    o.link,
                    &x,
                    &w_rows,
                    &category_of,
                    k,
                    &c.criteria,
                    start,
                    warnings,
                )
            };
            // the design covariance of a fit's estimates, and the SRS one
            let sandwich_of = |r: &ordinal::Ordinal, warnings: &mut Vec<String>| {
                let mut us = vec![vec![0.0; n]; r.b.len()];
                for (row, &i) in rows.iter().enumerate() {
                    for (u, s) in us.iter_mut().zip(&r.scores[row]) {
                        u[i] = *s;
                    }
                }
                let jinv = information_inverse(name, &r.information, warnings);
                let sandwich = |m: &Matrix| -> Matrix { multiply(&multiply(&jinv, m), &jinv) };
                (
                    sandwich(&design.covariance(&us)),
                    srs_of(&us).map(|m| sandwich(&m)),
                )
            };
            let result = fit_of(None, warnings);
            let size = result.b.len();
            let (v_small, vs_small) = sandwich_of(&result, warnings);
            if o.nonparallel.is_some() {
                if k < 3 || cols.is_empty() {
                    warnings.push(
                        "CSORDINAL NONPARALLEL: the general model needs at least three categories and a coefficient; it is left out".into(),
                    );
                } else {
                    // the general model starts at the model's estimates, its coefficients
                    // repeated for every threshold
                    let mut start = result.b[..k - 1].to_vec();
                    for _ in 0..k - 1 {
                        start.extend_from_slice(&result.b[k - 1..]);
                    }
                    let g = fit_of(Some(&start), warnings);
                    let (v, vs) = sandwich_of(&g, warnings);
                    let len = g.b.len();
                    general = Some((
                        Estimates {
                            b: g.b,
                            v,
                            vs,
                            redundant: vec![false; len],
                            p: len,
                            logits: 1,
                        },
                        cols.clone(),
                    ));
                }
            }
            // the thresholds go after the p columns of the design
            let full = p + k - 1;
            let index = |a: usize| if a < k - 1 { p + a } else { cols[a - (k - 1)] };
            let expand = |m: &Matrix| -> Matrix {
                let mut out = vec![vec![0.0; full]; full];
                for a in 0..size {
                    for bb in 0..size {
                        out[index(a)][index(bb)] = m[a][bb];
                    }
                }
                out
            };
            let mut b = vec![0.0; full];
            for a in 0..size {
                b[index(a)] = result.b[a];
            }
            let v = expand(&v_small);
            let vs = vs_small.as_ref().map(expand);
            logistic = Some(result.fit);
            Estimates {
                b,
                v,
                vs,
                // the intercept column has no parameter of its own
                redundant: (0..full)
                    .map(|a| a == 0 || (a < p && fit.redundant[a]))
                    .collect(),
                p,
                logits: 1,
            }
        } else if !c.logistic {
            let b: Vec<f64> = (0..p).map(|j| fit.b[j][0]).collect();
            let mut us = vec![vec![0.0; n]; p];
            for (r, &i) in rows.iter().enumerate() {
                let x = &fit.x[r];
                let resid = d.y[r][0] - x.iter().zip(&b).map(|(a, b)| a * b).sum::<f64>();
                for j in 0..p {
                    us[j][i] = x[j] * resid;
                }
            }
            let sandwich = |m: &Matrix| -> Matrix { multiply(&multiply(&fit.ginv, m), &fit.ginv) };
            let v = sandwich(&design.covariance(&us));
            let vs = srs_of(&us).map(|m| sandwich(&m));
            Estimates {
                b,
                v,
                vs,
                redundant: fit.redundant.clone(),
                p,
                logits: 1,
            }
        } else {
            let cols: Vec<usize> = (0..p).filter(|&j| !fit.redundant[j]).collect();
            let model = LogitModel {
                x: &fit.x,
                w: &w_rows,
                y: &category_of,
                cols: cols.clone(),
                base,
                others: others.clone(),
            };
            let (small, result) = fit_logistic(&model, intercept, &c.criteria, warnings);
            let q = cols.len();
            let lq = others.len() * q;
            let mut us = vec![vec![0.0; n]; lq];
            for (r, &i) in rows.iter().enumerate() {
                let probs = &result.probs[r];
                for (l, &k) in others.iter().enumerate() {
                    let resid = f64::from(u8::from(category_of[r] == k)) - probs[k];
                    for (s, &col) in cols.iter().enumerate() {
                        us[l * q + s][i] = resid * fit.x[r][col];
                    }
                }
            }
            let (_, info) = model.derivatives(&small);
            let jinv = information_inverse("CSLOGISTIC", &info, warnings);
            let sandwich = |m: &Matrix| -> Matrix { multiply(&multiply(&jinv, m), &jinv) };
            let v_small = sandwich(&design.covariance(&us));
            let vs_small = srs_of(&us).map(|m| sandwich(&m));
            let full = others.len() * p;
            let index = |l: usize, s: usize| l * p + cols[s];
            let mut b = vec![0.0; full];
            let expand = |m: &Matrix| -> Matrix {
                let mut out = vec![vec![0.0; full]; full];
                for a in 0..lq {
                    for bb in 0..lq {
                        out[index(a / q, a % q)][index(bb / q, bb % q)] = m[a][bb];
                    }
                }
                out
            };
            for a in 0..lq {
                b[index(a / q, a % q)] = small[a];
            }
            let v = expand(&v_small);
            let vs = vs_small.as_ref().map(expand);
            logistic = Some(result);
            Estimates {
                b,
                v,
                vs,
                redundant: (0..full).map(|a| fit.redundant[a % p]).collect(),
                p,
                logits: others.len(),
            }
        };
        let category_text = |k: usize| shown.value(&dep_var, &categories[k]);
        // a label for each logit and each parameter
        let logit_labels: Vec<String> = others.iter().map(|&k| category_text(k)).collect();
        // the order of the parameters in the tables: thresholds first
        let order: Vec<usize> = if c.ordinal.is_some() {
            (p..est.len()).chain(1..p).collect()
        } else {
            (0..est.len()).collect()
        };
        let threshold_name = |a: usize| {
            format!(
                "[{}={}]",
                dependent,
                key_value_text(&dep_var, &categories[a - p])
            )
        };
        let param_label = |a: usize| -> String {
            if a >= est.logits * p {
                format!("Threshold {}", threshold_name(a))
            } else if c.logistic && est.logits > 1 {
                format!("{}: {}", logit_labels[a / p], names[a % p])
            } else {
                names[a % p].clone()
            }
        };
        let dep_text = shown.variable(&dep_var);
        let mut caption_base = format!("Dependent Variable: {dep_text}");
        if let Some(o) = c.ordinal {
            caption_base.push_str(&format!(". Link Function: {}", link_name(o.link)));
        } else if c.logistic {
            caption_base.push_str(&format!(" (reference category: {})", category_text(base)));
        }
        let model_text = fit
            .effects
            .iter()
            .map(|e| {
                if e.is_intercept() && c.ordinal.is_some() {
                    "(Threshold)".to_string()
                } else if e.is_intercept() {
                    "(Intercept)".to_string()
                } else {
                    e.label.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        let caption = format!("{caption_base}. Model: {model_text}.");
        let mut tables = Vec::new();
        let show_intercept = c.show_intercept || c.intercept != CsIntercept::Yes;
        let level = c.criteria.cilevel;
        let tq = if df > 0.0 {
            t_quantile((1.0 - level / 100.0) / 2.0, df)
        } else {
            f64::NAN
        };
        let ci_group = format!("{level}% Confidence Interval");

        // Sample Design Information
        if c.print.sampleinfo {
            let valid_count = valid.iter().filter(|v| **v).count();
            let valid_w: f64 = (0..n).filter(|&i| valid[i]).map(|i| design.w[i]).sum();
            let mut out = vec![
                vec![
                    text("N of Unweighted Cases"),
                    text("Valid"),
                    Cell::Number(valid_count as f64),
                ],
                vec![
                    text(""),
                    text("Invalid"),
                    Cell::Number((data.case_count() - valid_count) as f64),
                ],
                vec![
                    text(""),
                    text("Total"),
                    Cell::Number(data.case_count() as f64),
                ],
                vec![text("Population Size"), text(""), Cell::Number(valid_w)],
            ];
            if c.domain.is_some() {
                out.push(vec![
                    text("Subpopulation"),
                    text("Unweighted Count"),
                    Cell::Number(rows.len() as f64),
                ]);
                out.push(vec![
                    text(""),
                    text("Population Size"),
                    Cell::Number(w_rows.iter().sum()),
                ]);
            }
            let first = &design.stages[0];
            out.push(vec![
                text("Stage 1"),
                text("Strata"),
                Cell::Number(first.n_units.len() as f64),
            ]);
            out.push(vec![
                text(""),
                text("Units"),
                Cell::Number(first.unit_stratum.len() as f64),
            ]);
            out.push(vec![
                text("Sampling Design Degrees of Freedom"),
                text(""),
                Cell::Number(design.df),
            ]);
            let mut cols = plain(&["", ""]);
            cols.push((String::new(), "Value".into()));
            tables.push(table("Sample Design Information", 2, cols, out, None));
        }

        // Variable Information
        if c.print.variableinfo {
            let total_w: f64 = w_rows.iter().sum();
            let mut cat_rows = Vec::new();
            let mut categorical =
                |role: &str, var: &Variable, keys: &[Key], index: &dyn Fn(usize) -> usize| {
                    let mut sums = vec![(0.0, 0usize); keys.len()];
                    for (r, w) in w_rows.iter().enumerate() {
                        let k = index(r);
                        sums[k].0 += w;
                        sums[k].1 += 1;
                    }
                    for (k, key) in keys.iter().enumerate() {
                        cat_rows.push(vec![
                            text(if k == 0 { role } else { "" }),
                            text(if k == 0 {
                                shown.variable(var)
                            } else {
                                String::new()
                            }),
                            text(shown.value(var, key)),
                            Cell::Number(sums[k].0),
                            Cell::Number(100.0 * sums[k].0 / total_w),
                            Cell::Number(sums[k].1 as f64),
                        ]);
                    }
                };
            if c.logistic {
                categorical("Dependent Variable", &dep_var, &categories, &|r| {
                    category_of[r]
                });
            }
            for (f, var) in factor_vars.iter().enumerate() {
                categorical("Factor", var, &levels[f], &|r| d.level[r][f]);
            }
            if !cat_rows.is_empty() {
                let mut cols = plain(&["", "", ""]);
                cols.extend(plain(&["Population Size", "Percent", "Unweighted Count"]));
                tables.push(table(
                    "Categorical Variable Information",
                    3,
                    cols,
                    cat_rows,
                    None,
                ));
            }
            let mut cont_rows = Vec::new();
            let mut continuous =
                |role: &str, first: bool, var: &Variable, value: &dyn Fn(usize) -> f64| {
                    let mean = (0..rows.len()).map(|r| w_rows[r] * value(r)).sum::<f64>() / total_w;
                    cont_rows.push(vec![
                        text(if first { role } else { "" }),
                        text(shown.variable(var)),
                        Cell::Number(mean),
                        Cell::Number(total_w),
                        Cell::Number(rows.len() as f64),
                    ]);
                };
            if !c.logistic {
                continuous("Dependent Variable", true, &dep_var, &|r| d.y[r][0]);
            }
            for (k, var) in cov_vars.iter().enumerate() {
                continuous("Covariate", k == 0, var, &|r| d.cov[r][k]);
            }
            if !cont_rows.is_empty() {
                let mut cols = plain(&["", ""]);
                cols.extend(plain(&["Mean", "Population Size", "Unweighted Count"]));
                tables.push(table(
                    "Continuous Variable Information",
                    2,
                    cols,
                    cont_rows,
                    None,
                ));
            }
        }

        // Iteration History
        if let (Some(every), Some(lg)) = (c.print.history, &logistic) {
            let every = every.max(1) as usize;
            let last = lg.history.len() - 1;
            let cols_used: Vec<usize> = order
                .iter()
                .copied()
                .filter(|&a| !est.redundant[a])
                .collect();
            let mut cols = plain(&["Iteration", "-2 Log Pseudo Likelihood", "Step-halving"]);
            cols.extend(
                cols_used
                    .iter()
                    .map(|&a| ("Parameter".to_string(), param_label(a))),
            );
            let out = lg
                .history
                .iter()
                .enumerate()
                .filter(|(k, _)| k % every == 0 || *k == last)
                .map(|(_, (iteration, m2ll, halvings, small))| {
                    let mut row = vec![
                        text(iteration.to_string()),
                        Cell::Number(*m2ll),
                        Cell::Number(*halvings as f64),
                    ];
                    row.extend(small.iter().map(|v| Cell::Number(*v)));
                    row
                })
                .collect();
            tables.push(table(
                "Iteration History",
                1,
                cols,
                out,
                Some(caption.clone()),
            ));
        }

        // Model Summary
        if c.print.summary {
            if let Some(lg) = &logistic {
                let total_w: f64 = w_rows.iter().sum();
                let cs = 1.0 - (-2.0 * (lg.ll - lg.ll0) / total_w).exp();
                let nag = cs / (1.0 - (2.0 * lg.ll0 / total_w).exp());
                let mcf = 1.0 - lg.ll / lg.ll0;
                tables.push(table(
                    "Pseudo R Squares",
                    0,
                    plain(&["Cox and Snell", "Nagelkerke", "McFadden"]),
                    vec![vec![number(Some(cs)), number(Some(nag)), number(Some(mcf))]],
                    Some(caption.clone()),
                ));
            } else {
                let mut sse = 0.0;
                let total_w: f64 = w_rows.iter().sum();
                let mean = (0..rows.len()).map(|r| w_rows[r] * d.y[r][0]).sum::<f64>() / total_w;
                let mut sst = 0.0;
                let centre = if intercept { mean } else { 0.0 };
                for (r, w) in w_rows.iter().enumerate() {
                    let y = d.y[r][0];
                    let pred: f64 = fit.x[r].iter().zip(&est.b).map(|(a, b)| a * b).sum();
                    sse += w * (y - pred).powi(2);
                    sst += w * (y - centre).powi(2);
                }
                tables.push(table(
                    "Model Summary",
                    0,
                    plain(&["R Square"]),
                    vec![vec![number(Some(1.0 - sse / sst))]],
                    Some(caption.clone()),
                ));
            }
        }

        // L matrices of the model effects, logit block diagonal
        let blocks = |rows: &Matrix| -> Matrix {
            (0..est.logits)
                .flat_map(|l| rows.iter().map(move |r| (l, r)))
                .map(|(l, r)| est.spread(r, l))
                .collect()
        };
        let h = fit.h();
        let mut lmatrices: Vec<(String, Matrix)> = Vec::new();
        // Tests of Model Effects
        {
            let mut out = Vec::new();
            let mut sources: Vec<(String, Matrix)> = Vec::new();
            let non_intercept: Matrix = fit
                .effects
                .iter()
                .filter(|e| !e.is_intercept())
                .flat_map(|e| e.range())
                .filter(|&j| !fit.redundant[j])
                .map(|j| {
                    // row j of H is exactly 1 at j and 0 at the other non-redundant columns
                    (0..p)
                        .map(|c| match fit.redundant[c] {
                            false => f64::from(u8::from(c == j)),
                            true => h[j][c],
                        })
                        .collect()
                })
                .collect();
            if !non_intercept.is_empty() {
                let label = if intercept {
                    "(Corrected Model)"
                } else {
                    "(Model)"
                };
                sources.push((label.into(), non_intercept));
            }
            for (e, effect) in fit.effects.iter().enumerate() {
                if effect.is_intercept() && !show_intercept {
                    continue;
                }
                let label = if effect.is_intercept() {
                    "(Intercept)".to_string()
                } else {
                    effect.label.clone()
                };
                sources.push((label, fit.type34(e, false)));
            }
            for (label, l_star) in sources {
                // the coefficients are whole numbers for factor columns (the weight of a column
                // in a redundant one, or the elimination of Type III); rounding residue goes
                let snap = |v: f64| match v.round() {
                    r if (v - r).abs() <= 1e-9 => r + 0.0,
                    _ => v,
                };
                let l_star: Matrix = l_star
                    .iter()
                    .map(|row| row.iter().map(|&v| snap(v)).collect())
                    .collect();
                let l = blocks(&l_star);
                let w = wald(&est, &l, &vec![0.0; l.len()]);
                let mut row = vec![text(label.clone())];
                row.extend(test_cells(c.test, w.as_ref(), df, None));
                out.push(row);
                lmatrices.push((label, l));
            }
            let mut cols = plain(&["Source"]);
            cols.extend(test_columns(c.test));
            tables.push(table(
                "Tests of Model Effects",
                1,
                cols,
                out,
                Some(caption.clone()),
            ));
        }

        // Parameter Estimates
        let stats = match c.statistics {
            Some(s) => Some(s),
            None if !c.logistic => Some(CsModelStatistics {
                parameter: true,
                se: true,
                ttest: true,
                cinterval: true,
                ..CsModelStatistics::default()
            }),
            None => None,
        };
        let stats = stats.map(|mut s| {
            if s.exp && c.ordinal.is_some_and(|o| o.link != PlumLink::Logit) {
                warnings
                    .push("CSORDINAL: STATISTICS EXP needs the logit link and is left out".into());
                s.exp = false;
            }
            s
        });
        if let Some(s) = stats {
            let headers = if c.logistic { 2 } else { 1 };
            let mut cols = if c.logistic {
                plain(&["", "Parameter"])
            } else {
                plain(&["Parameter"])
            };
            if s.parameter {
                cols.extend(plain(&["Estimate"]));
            }
            if s.se {
                cols.extend(plain(&["Std. Error"]));
            }
            if s.cinterval && (s.parameter || !s.exp) {
                cols.push((ci_group.clone(), "Lower".into()));
                cols.push((ci_group.clone(), "Upper".into()));
            }
            if s.ttest {
                for leaf in ["t", "df", "Sig."] {
                    cols.push(("Hypothesis Test".into(), leaf.into()));
                }
            }
            if s.exp {
                cols.extend(plain(&["Exp(B)"]));
                if s.cinterval {
                    let group = format!("{level}% Confidence Interval for Exp(B)");
                    cols.push((group.clone(), "Lower".into()));
                    cols.push((group, "Upper".into()));
                }
            }
            if s.deff {
                cols.extend(plain(&["Design Effect"]));
            }
            if s.deffsqrt {
                cols.extend(plain(&["Square Root of Design Effect"]));
            }
            let mut out = Vec::new();
            let mut redundant_seen = false;
            let mut last_group = None;
            for &a in &order {
                let threshold = a >= est.logits * p;
                let j = a % p;
                let is_intercept = !threshold
                    && fit
                        .effects
                        .iter()
                        .any(|e| e.is_intercept() && e.range().contains(&j));
                if is_intercept && !show_intercept {
                    continue;
                }
                let mut row = Vec::new();
                if c.logistic {
                    // the thresholds, the logit or the location parameters
                    let group = (threshold, a / p);
                    let label = match (threshold, c.ordinal.is_some()) {
                        (true, _) => "Threshold".to_string(),
                        (false, true) => "Location".to_string(),
                        (false, false) => logit_labels[a / p].clone(),
                    };
                    let first = last_group != Some(group);
                    last_group = Some(group);
                    row.push(text(if first { label } else { String::new() }));
                }
                let name_of = if threshold {
                    threshold_name(a)
                } else {
                    names[j].clone()
                };
                let b = est.b[a];
                if est.redundant[a] {
                    redundant_seen = true;
                    row.push(text(format!("{name_of} (redundant)")));
                    let width = cols.len() - headers;
                    if s.parameter {
                        row.push(Cell::Number(0.0));
                        row.extend(std::iter::repeat_n(Cell::Empty, width - 1));
                    } else {
                        row.extend(std::iter::repeat_n(Cell::Empty, width));
                    }
                    out.push(row);
                    continue;
                }
                row.push(text(name_of));
                let se = est.v[a][a].max(0.0).sqrt();
                let lo = b - tq * se;
                let hi = b + tq * se;
                if s.parameter {
                    row.push(number(Some(b)));
                }
                if s.se {
                    row.push(number(Some(se)));
                }
                if s.cinterval && (s.parameter || !s.exp) {
                    row.push(number(Some(lo)));
                    row.push(number(Some(hi)));
                }
                if s.ttest {
                    let t = b / se;
                    let ok = se > 0.0 && df > 0.0;
                    row.push(number(ok.then_some(t)));
                    row.push(number(Some(df)));
                    row.push(number(ok.then(|| t_p(t, df))));
                }
                if s.exp {
                    row.push(number(Some(b.exp())));
                    if s.cinterval {
                        row.push(number(Some(lo.exp())));
                        row.push(number(Some(hi.exp())));
                    }
                }
                let deff = est
                    .vs
                    .as_ref()
                    .map(|vs| vs[a][a])
                    .filter(|v| *v > 0.0)
                    .map(|v| est.v[a][a] / v);
                if s.deff {
                    row.push(number(deff));
                }
                if s.deffsqrt {
                    row.push(number(deff.map(f64::sqrt)));
                }
                out.push(row);
            }
            let mut cap = caption.clone();
            if redundant_seen {
                cap.push_str(
                    " A parameter marked redundant is set to zero because it is redundant.",
                );
            }
            tables.push(table("Parameter Estimates", headers, cols, out, Some(cap)));
        }

        // Covariances and correlations of the parameter estimates
        // SHOW=NO leaves the intercept out of these tables too
        let used: Vec<usize> = order
            .iter()
            .copied()
            .filter(|&a| {
                !est.redundant[a]
                    && (show_intercept || !(intercept && a < est.logits * p && a % p == 0))
            })
            .collect();
        for (wanted, title) in [
            (c.print.covb, "Covariances of Parameter Estimates"),
            (c.print.corb, "Correlations of Parameter Estimates"),
        ] {
            if !wanted {
                continue;
            }
            let corr = title.starts_with("Corr");
            let mut cols = plain(&["Parameter"]);
            cols.extend(used.iter().map(|&a| (String::new(), param_label(a))));
            let out = used
                .iter()
                .map(|&a| {
                    let mut row = vec![text(param_label(a))];
                    row.extend(used.iter().map(|&b| {
                        let v = est.v[a][b];
                        number(Some(if corr {
                            v / (est.v[a][a] * est.v[b][b]).sqrt()
                        } else {
                            v
                        }))
                    }));
                    row
                })
                .collect();
            tables.push(table(title, 1, cols, out, Some(caption.clone())));
        }

        // OUTFILE: every parameter of the Parameter Estimates, the intercept whatever SHOW says; a
        // redundant one has its estimate 0 and nothing else
        let files: Files = {
            let ok = |a: usize| !est.redundant[a];
            let matrix = c
                .outfile
                .matrix
                .as_ref()
                .map(|(_, corr)| matrix_file(&est, &order, df, *corr, &param_label));
            let model = c.outfile.model.as_ref().map(|(_, with_covariance)| {
                let parameters: Vec<_> = order
                    .iter()
                    .map(|&a| json!({"name": param_label(a), "estimate": est.b[a], "redundant": !ok(a)}))
                    .collect();
                let mut model = json!({
                    "format": "statscraft-cs-model",
                    "version": 1,
                    "command": name,
                    "plan": c.plan,
                    "dependent": dependent,
                    "factors": factor_names,
                    "covariates": covariate_names,
                    "model": model_text,
                    "df": df,
                    "parameters": parameters,
                });
                if c.logistic {
                    let values: Vec<_> = categories
                        .iter()
                        .map(|k| key_value_text(&dep_var, k))
                        .collect();
                    model["categories"] = json!(values);
                }
                match c.ordinal {
                    Some(o) => model["link"] = json!(link_name(o.link)),
                    None if c.logistic => {
                        model["reference"] = json!(key_value_text(&dep_var, &categories[base]))
                    }
                    None => {}
                }
                if *with_covariance {
                    let covariance: Vec<Vec<Option<f64>>> = order
                        .iter()
                        .map(|&a| {
                            order
                                .iter()
                                .map(|&b| (ok(a) && ok(b)).then_some(est.v[a][b]))
                                .collect()
                        })
                        .collect();
                    model["covariance"] = json!(covariance);
                }
                model
            });
            (matrix.transpose()?, model)
        };

        // NONPARALLEL: the test of parallel lines and the general model
        if let (Some((g, gcols)), Some(np)) = (&general, c.ordinal.and_then(|o| o.nonparallel)) {
            let k = categories.len();
            let q = gcols.len();
            // the coefficient of design column gcols[s] for threshold j
            let at = |j: usize, s: usize| k - 1 + j * q + s;
            if np.test {
                // each coefficient of the first threshold against the same one of every later
                // threshold
                let mut l: Matrix = Vec::new();
                for s in 0..q {
                    for j in 1..k - 1 {
                        let mut row = vec![0.0; g.len()];
                        row[at(0, s)] = 1.0;
                        row[at(j, s)] = -1.0;
                        l.push(row);
                    }
                }
                let w = wald(g, &l, &vec![0.0; l.len()]);
                let mut row = vec![text("General Model")];
                row.extend(test_cells(c.test, w.as_ref(), df, None));
                let mut cols = plain(&[""]);
                cols.extend(test_columns(c.test));
                tables.push(table(
                    "Test of Parallel Lines",
                    1,
                    cols,
                    vec![row],
                    Some(format!(
                        "{caption} The general model gives every threshold its own location parameters; the test asks whether they are equal."
                    )),
                ));
            }
            let location = |j: usize| format!("Location for {}", threshold_name(p + j));
            if np.parameter {
                let mut out = Vec::new();
                let mut push = |group: String, label: String, a: Option<usize>| {
                    let mut row = vec![text(group)];
                    match a {
                        Some(a) => row.extend([
                            text(label),
                            number(Some(g.b[a])),
                            number(Some(g.v[a][a].max(0.0).sqrt())),
                        ]),
                        None => row.extend([
                            text(format!("{label} (redundant)")),
                            Cell::Number(0.0),
                            Cell::Empty,
                        ]),
                    }
                    out.push(row);
                };
                for j in 0..k - 1 {
                    let group = if j == 0 { "Threshold" } else { "" };
                    push(group.into(), threshold_name(p + j), Some(j));
                }
                for j in 0..k - 1 {
                    for (col, name) in names.iter().enumerate().skip(1) {
                        let group = if col == 1 { location(j) } else { String::new() };
                        let a = gcols.iter().position(|&c| c == col).map(|s| at(j, s));
                        push(group, name.clone(), a);
                    }
                }
                tables.push(table(
                    "Parameter Estimates of the General Model",
                    2,
                    plain(&["", "Parameter", "Estimate", "Std. Error"]),
                    out,
                    Some(caption.clone()),
                ));
            }
            if np.covb {
                let label = |a: usize| {
                    if a < k - 1 {
                        format!("Threshold {}", threshold_name(p + a))
                    } else {
                        let (j, s) = ((a - (k - 1)) / q, (a - (k - 1)) % q);
                        format!("{}: {}", location(j), names[gcols[s]])
                    }
                };
                let mut cols = plain(&["Parameter"]);
                cols.extend((0..g.len()).map(|a| (String::new(), label(a))));
                let out = (0..g.len())
                    .map(|a| {
                        let mut row = vec![text(label(a))];
                        row.extend(g.v[a].iter().map(|v| number(Some(*v))));
                        row
                    })
                    .collect();
                tables.push(table(
                    "Covariances of the General Model Parameters",
                    1,
                    cols,
                    out,
                    Some(caption.clone()),
                ));
            }
        }

        // the predicted category: the most probable one; on a tie the first, for CSORDINAL the
        // one with the larger summed weight, then the larger count, then the first
        let mut tally = vec![(0.0, 0usize); categories.len()];
        for (r, &y) in category_of.iter().enumerate() {
            tally[y].0 += w_rows[r];
            tally[y].1 += 1;
        }
        let predicted = |probs: &[f64]| -> usize {
            (0..probs.len()).fold(0, |m, j| {
                let tie = probs[j] == probs[m] && c.ordinal.is_some() && tally[j] > tally[m];
                if probs[j] > probs[m] || tie { j } else { m }
            })
        };

        // Classification
        if let (true, Some(lg)) = (c.print.classtable, &logistic) {
            let k = categories.len();
            let mut counts = vec![vec![0.0; k]; k];
            for (r, probs) in lg.probs.iter().enumerate() {
                counts[category_of[r]][predicted(probs)] += w_rows[r];
            }
            let total: f64 = counts.iter().flatten().sum();
            let mut cols = plain(&["Observed"]);
            cols.extend((0..k).map(|j| ("Predicted".to_string(), category_text(j))));
            cols.push(("Predicted".to_string(), "Percent Correct".to_string()));
            let mut out: Vec<Vec<Cell>> = (0..k)
                .map(|i| {
                    let row_total: f64 = counts[i].iter().sum();
                    let mut row = vec![text(category_text(i))];
                    row.extend(counts[i].iter().map(|v| Cell::Number(*v)));
                    row.push(number(
                        (row_total > 0.0).then(|| 100.0 * counts[i][i] / row_total),
                    ));
                    row
                })
                .collect();
            let mut overall = vec![text("Overall Percent")];
            overall.extend(
                (0..k).map(|j| {
                    Cell::Number(100.0 * (0..k).map(|i| counts[i][j]).sum::<f64>() / total)
                }),
            );
            overall.push(Cell::Number(
                100.0 * (0..k).map(|i| counts[i][i]).sum::<f64>() / total,
            ));
            out.push(overall);
            tables.push(table(
                "Classification",
                1,
                cols,
                out,
                Some(format!("{caption_base}. Cells are sums of the weights.")),
            ));
        }

        // the design row of a pattern of factor levels and covariate values, and whether every
        // effect has a column for it
        let row_of = |lv: &[usize], cv: &[f64]| -> (Vec<f64>, bool) {
            let mut x = vec![0.0; p];
            let mut complete = true;
            for e in &fit.effects {
                let cell: Vec<usize> = e.factors.iter().map(|&f| lv[f]).collect();
                match e.cells.iter().position(|c| *c == cell) {
                    Some(k) => x[e.start + k] = e.covariates.iter().map(|&k| cv[k]).product(),
                    None => complete = false,
                }
            }
            (x, complete)
        };

        // Odds Ratios; CSORDINAL has them for the logit link only
        let control_default = |f: usize| levels[f].len() - 1;
        let odds = c.ordinal.is_none_or(|o| o.link == PlumLink::Logit);
        if !odds && !c.oddsratios.is_empty() {
            warnings.push("CSORDINAL: ODDSRATIOS needs the logit link and is left out".into());
        }
        for o in c.oddsratios.iter().filter(|_| odds) {
            let factor_index =
                |n: &str| factor_names.iter().position(|f| f.eq_ignore_ascii_case(n));
            let cov_index = |n: &str| {
                covariate_names
                    .iter()
                    .position(|f| f.eq_ignore_ascii_case(n))
            };
            let mut fixed_levels: Vec<usize> =
                (0..factor_names.len()).map(control_default).collect();
            let mut fixed_covs: Vec<f64> = (0..covariate_names.len())
                .map(|k| d.covariate_mean(&[k]))
                .collect();
            for (n, value) in &o.control {
                if let Some(f) = factor_index(n) {
                    fixed_levels[f] = match value {
                        CsControl::Low => 0,
                        CsControl::High => control_default(f),
                        CsControl::Value(v) => level_of(&factor_vars[f], &levels[f], v)
                            .unwrap_or_else(|| {
                                warnings.push(format!(
                                "{name} ODDSRATIOS: {n} has no level {}; its highest level is used",
                                value_text(v)
                            ));
                                control_default(f)
                            }),
                        CsControl::Mean => {
                            return Err(Error(format!(
                                "ODDSRATIOS CONTROL: {n} is a factor; give LOW, HIGH or a value"
                            )));
                        }
                    };
                } else if let Some(k) = cov_index(n) {
                    fixed_covs[k] = match value {
                        CsControl::Mean => d.covariate_mean(&[k]),
                        CsControl::Value(Value::Number(v)) => *v,
                        _ => {
                            return Err(Error(format!(
                                "ODDSRATIOS CONTROL: {n} is a covariate; give MEAN or a number"
                            )));
                        }
                    };
                } else {
                    return Err(Error(format!(
                        "ODDSRATIOS CONTROL names {n}, which is neither a factor nor a covariate of the model"
                    )));
                }
            }
            // (label, x1 - x2)
            let mut pairs: Vec<(String, Vec<f64>)> = Vec::new();
            for (n, reference) in &o.factors {
                let Some(f) = factor_index(n) else {
                    return Err(Error(format!(
                        "ODDSRATIOS FACTOR names {n}, which is not a factor of the model"
                    )));
                };
                let r = match reference {
                    CsReference::Low => 0,
                    CsReference::High => control_default(f),
                    CsReference::Value(v) => level_of(&factor_vars[f], &levels[f], v).unwrap_or_else(|| {
                        warnings.push(format!(
                            "{name} ODDSRATIOS: {n} has no level {}; its highest level is the reference",
                            value_text(v)
                        ));
                        control_default(f)
                    }),
                };
                let text_of = |l: usize| shown.value(&factor_vars[f], &levels[f][l]);
                if levels[f].len() < 2 {
                    warnings.push(format!(
                        "{name} ODDSRATIOS: {n} has one level among the cases analyzed, so it has no odds ratios"
                    ));
                }
                for l in (0..levels[f].len()).filter(|&l| l != r) {
                    let mut a = fixed_levels.clone();
                    a[f] = l;
                    let mut b = fixed_levels.clone();
                    b[f] = r;
                    let x1 = row_of(&a, &fixed_covs).0;
                    let x2 = row_of(&b, &fixed_covs).0;
                    pairs.push((
                        format!(
                            "{} {} vs. {}",
                            shown.variable(&factor_vars[f]),
                            text_of(l),
                            text_of(r)
                        ),
                        x1.iter().zip(&x2).map(|(a, b)| a - b).collect(),
                    ));
                }
            }
            for (n, changes) in &o.covariates {
                let Some(k) = cov_index(n) else {
                    return Err(Error(format!(
                        "ODDSRATIOS COVARIATE names {n}, which is not a covariate of the model"
                    )));
                };
                for change in changes {
                    let mut moved = fixed_covs.clone();
                    moved[k] += change;
                    let x1 = row_of(&fixed_levels, &moved).0;
                    let x2 = row_of(&fixed_levels, &fixed_covs).0;
                    pairs.push((
                        format!("{} up by {change}", covariate_texts[k]),
                        x1.iter().zip(&x2).map(|(a, b)| a - b).collect(),
                    ));
                }
            }
            for (label, diff) in &pairs {
                if diff.iter().all(|v| *v == 0.0) {
                    return Err(Error(format!(
                        "ODDSRATIOS: {label} does not change the model, as the variable is in no effect of it"
                    )));
                }
            }
            let mut out = Vec::new();
            let estimable: Vec<bool> = pairs.iter().map(|(_, d)| fit.estimable(d, &h)).collect();
            // CSLOGISTIC: a row per logit; CSORDINAL: one cumulative odds ratio, exp(-b'(x1 - x2))
            let (groups, sign) = match c.ordinal {
                Some(_) => (vec![None], -1.0),
                None => (logit_labels.iter().map(Some).collect(), 1.0),
            };
            for (l, label_l) in groups.iter().enumerate() {
                for (k, (label, diff)) in pairs.iter().enumerate() {
                    let ok = estimable[k];
                    // a comparison through a cell without cases is not estimable: no value
                    let row_l = est.spread(diff, l);
                    let cc = sign * est.estimate(&row_l);
                    let se = est.quad(&est.v, &row_l).max(0.0).sqrt();
                    let shown_value = |v: f64| number(ok.then(|| v.exp()));
                    let mut row = Vec::new();
                    if let Some(label_l) = label_l {
                        row.push(text(if k == 0 {
                            (*label_l).clone()
                        } else {
                            String::new()
                        }));
                    }
                    row.extend([
                        text(label.clone()),
                        shown_value(cc),
                        shown_value(cc - tq * se),
                        shown_value(cc + tq * se),
                    ]);
                    out.push(row);
                }
            }
            let (title, mut cols) = match c.ordinal {
                Some(_) => (
                    "Cumulative Odds Ratios",
                    plain(&["Comparison", "Cumulative Odds Ratio"]),
                ),
                None => ("Odds Ratios", plain(&["", "Comparison", "Odds Ratio"])),
            };
            cols.push((ci_group.clone(), "Lower".into()));
            cols.push((ci_group.clone(), "Upper".into()));
            let mut held: Vec<String> = factor_vars
                .iter()
                .enumerate()
                .map(|(f, v)| {
                    format!(
                        "{} = {}",
                        shown.variable(v),
                        shown.value(v, &levels[f][fixed_levels[f]])
                    )
                })
                .collect();
            held.extend(
                covariate_texts
                    .iter()
                    .zip(&fixed_covs)
                    .map(|(t, v)| format!("{t} = {}", (v * 1e4).round() / 1e4)),
            );
            let mut cap = caption_base.clone();
            if !held.is_empty() {
                cap.push_str(&format!(
                    ". Each row holds the variables it does not compare at: {}.",
                    held.join(", ")
                ));
            } else {
                cap.push('.');
            }
            if !out.is_empty() {
                let headers = if c.ordinal.is_some() { 1 } else { 2 };
                tables.push(table(title, headers, cols, out, Some(cap)));
            }
        }

        // Custom hypothesis tests
        let row_cells = |l: &[f64], k: f64, w: Option<&Wald>, sig: Option<f64>| -> Vec<Cell> {
            let e = est.estimate(l);
            let var = est.quad(&est.v, l);
            let se = var.max(0.0).sqrt();
            let mut cells = vec![
                number(Some(e)),
                number(Some(k)),
                number(Some(e - k)),
                number(Some(se)),
                number(Some(e - k - tq * se)),
                number(Some(e - k + tq * se)),
            ];
            cells.extend(test_cells(c.test, w, df, sig));
            cells
        };
        let detail_columns = |first: &[&str]| -> Vec<(String, String)> {
            let mut cols = plain(first);
            cols.extend(plain(&[
                "Estimate",
                "Hypothesized Value",
                "Difference (Estimate - Hypothesized)",
                "Std. Error",
            ]));
            cols.push((ci_group.clone(), "Lower".into()));
            cols.push((ci_group.clone(), "Upper".into()));
            cols.extend(test_columns(c.test));
            cols
        };
        let single = |l: &[f64], k: f64| wald(&est, &vec![l.to_vec()], &[k]);
        let p_of = |w: &Option<Wald>| {
            w.as_ref()
                .and_then(|w| test_values(c.test, w, df))
                .map(|(_, p)| p)
        };
        let has_intercept_column = fit.effects.first().is_some_and(|e| e.is_intercept());
        // the thresholds of CSORDINAL (after the p columns of the design)
        let thresholds = est.len() - est.logits * p;
        // whether a row over every parameter is estimable; with thresholds, a shift of all of
        // them offsets one of the intercept, so the row is estimable when its regression part
        // is, with minus the sum of its threshold coefficients on the intercept
        let estimable_full = |full: &[f64]| -> bool {
            if thresholds > 0 {
                let mut block = full[..p].to_vec();
                block[0] = -full[p..].iter().sum::<f64>();
                fit.estimable(&block, &h)
            } else {
                (0..est.logits).all(|l| fit.estimable(&full[l * p..(l + 1) * p], &h))
            }
        };
        for (number_of, custom) in c.custom.iter().enumerate() {
            let what = format!("CUSTOM {}", number_of + 1);
            // (label, full row, K value)
            let mut expanded: Vec<(String, Vec<f64>, f64)> = Vec::new();
            let given_rows: Vec<LRow> = if custom.rows.is_empty() {
                if !has_intercept_column {
                    return Err(Error(format!(
                        "{what}: KMATRIX without LMATRIX tests the intercept, which the model does not have"
                    )));
                }
                // an empty row stands for the intercept's row of the estimable functions
                vec![Vec::new()]
            } else {
                custom.rows.clone()
            };
            let k_values = custom
                .k
                .clone()
                .unwrap_or_else(|| vec![0.0; given_rows.len()]);
            if k_values.len() != given_rows.len() {
                return Err(Error(format!(
                    "{what}: KMATRIX gives {} values for {} rows",
                    k_values.len(),
                    given_rows.len()
                )));
            }
            for (i, row) in given_rows.iter().enumerate() {
                let all = row.iter().any(|(e, _)| e.is_none());
                if all && row.iter().any(|(e, _)| e.is_some()) {
                    return Err(Error(format!(
                        "{what}: a row with ALL cannot also name effects"
                    )));
                }
                let label = format!("Contrast {}", i + 1);
                let not_estimable = || {
                    Error(format!(
                        "{what}: row {} of the L matrix is not estimable",
                        i + 1
                    ))
                };
                if all {
                    // several ALL lists in one row add up; CSORDINAL lists the thresholds first
                    // and has no intercept
                    let wanted = est.len() - usize::from(thresholds > 0);
                    let at = |j: usize| match thresholds {
                        0 => j,
                        t if j < t => p + j,
                        t => j - t + 1,
                    };
                    let mut values = vec![0.0; est.len()];
                    for (_, list) in row {
                        if list.len() != wanted {
                            return Err(Error(format!(
                                "{what} gives {} coefficients after ALL where the model has {wanted} parameters",
                                list.len(),
                            )));
                        }
                        for (j, x) in list.iter().enumerate() {
                            values[at(j)] += x;
                        }
                    }
                    if !estimable_full(&values) {
                        return Err(not_estimable());
                    }
                    expanded.push((label, values, k_values[i]));
                } else if thresholds > 0 {
                    // the leading numbers are the threshold coefficients
                    let (lead, effects): (LRow, LRow) = row
                        .iter()
                        .cloned()
                        .partition(|(e, _)| e.as_ref().is_some_and(|t| t.variables.is_empty()));
                    let mut values = vec![0.0; est.len()];
                    if !effects.is_empty() {
                        values[..p].copy_from_slice(&l_row(&d, &fit, &effects, &what)?);
                    }
                    for (_, list) in &lead {
                        if list.len() != thresholds {
                            return Err(Error(format!(
                                "{what} gives {} threshold coefficients where the model has {thresholds} thresholds",
                                list.len()
                            )));
                        }
                        for (j, x) in list.iter().enumerate() {
                            values[p + j] += x;
                        }
                    }
                    if !estimable_full(&values) {
                        return Err(not_estimable());
                    }
                    expanded.push((label, values, k_values[i]));
                } else {
                    let l_star = if row.is_empty() {
                        h[0].clone()
                    } else {
                        l_row(&d, &fit, row, &what)?
                    };
                    if !fit.estimable(&l_star, &h) {
                        return Err(not_estimable());
                    }
                    // one row per logit (CSLOGISTIC); CSGLM has no logit labels
                    for (l, logit) in logit_labels.iter().enumerate() {
                        let label = format!("{label} ({logit})");
                        expanded.push((label, est.spread(&l_star, l), k_values[i]));
                    }
                    if !c.logistic {
                        expanded.push((label, l_star, k_values[i]));
                    }
                }
            }
            let tests: Vec<Option<Wald>> = expanded.iter().map(|(_, l, k)| single(l, *k)).collect();
            let ps: Vec<Option<f64>> = tests.iter().map(p_of).collect();
            let keyed: Vec<(Vec<f64>, f64)> =
                expanded.iter().map(|(_, l, k)| (l.clone(), *k)).collect();
            let adjusted = padjust(c.padjust, &ps, &keyed);
            let mut out: Vec<Vec<Cell>> = expanded
                .iter()
                .zip(&adjusted)
                .zip(&tests)
                .map(|(((label, l, k), sig), w)| {
                    let mut row = vec![text(label.clone())];
                    row.extend(row_cells(l, *k, w.as_ref(), *sig));
                    row
                })
                .collect();
            let l: Matrix = expanded.iter().map(|(_, l, _)| l.clone()).collect();
            let k: Vec<f64> = expanded.iter().map(|(_, _, k)| *k).collect();
            let mut overall = vec![text("Overall Test")];
            overall.extend(std::iter::repeat_n(Cell::Empty, 6));
            overall.extend(test_cells(c.test, wald(&est, &l, &k).as_ref(), df, None));
            out.push(overall);
            let mut cap = caption.clone();
            if let Some(label) = &custom.label {
                cap = format!("{label}. {cap}");
            }
            if let Some(m) = padjust_note(c.padjust) {
                cap.push_str(&format!(
                    " The significance of each contrast is adjusted by the {m} method."
                ));
            }
            tables.push(table(
                "Custom Hypothesis Tests",
                1,
                detail_columns(&["Contrast"]),
                out,
                Some(cap),
            ));
            lmatrices.push((custom.label.clone().unwrap_or(what), l));
        }

        // Estimated marginal means (CSGLM)
        for em in &c.emmeans {
            let fs: Vec<usize> = em
                .tables
                .iter()
                .map(|t| {
                    factor_names
                        .iter()
                        .position(|f| f.eq_ignore_ascii_case(t))
                        .ok_or_else(|| {
                            Error(format!("EMMEANS TABLES names {t}, which is not a factor"))
                        })
                })
                .collect::<Result<_, _>>()?;
            if let Some((other, _)) = em
                .other
                .iter()
                .find(|(o, _)| !covariate_names.iter().any(|n| n.eq_ignore_ascii_case(o)))
            {
                return Err(Error(format!(
                    "EMMEANS OTHER names {other}, which is not a covariate of the model"
                )));
            }
            let with = covariate_values(&d, &em.other, "EMMEANS OTHER")?;
            let sizes: Vec<usize> = fs.iter().map(|&f| levels[f].len()).collect();
            let combos = combinations(&sizes);
            let mean_of = |combo: &[usize]| -> Option<(Vec<f64>, f64, f64)> {
                let fixed: Vec<(usize, usize)> =
                    fs.iter().zip(combo).map(|(&f, &l)| (f, l)).collect();
                let l = emmeans_l(&d, &fit, &fixed, &with);
                fit.estimable(&l, &h).then(|| {
                    let m = est.estimate(&l);
                    let se = est.quad(&est.v, &l).max(0.0).sqrt();
                    (l, m, se)
                })
            };
            let text_of = |f: usize, l: usize| shown.value(&factor_vars[f], &levels[f][l]);
            let mut out = Vec::new();
            for combo in &combos {
                let mut row: Vec<Cell> = fs
                    .iter()
                    .zip(combo)
                    .map(|(&f, &l)| text(text_of(f, l)))
                    .collect();
                if fs.is_empty() {
                    row.push(text("Overall"));
                }
                match mean_of(combo) {
                    Some((_, m, se)) => row.extend([
                        number(Some(m)),
                        number(Some(se)),
                        number(Some(m - tq * se)),
                        number(Some(m + tq * se)),
                    ]),
                    None => row.extend(vec![Cell::Empty; 4]),
                }
                out.push(row);
            }
            let mut cols: Vec<(String, String)> = if fs.is_empty() {
                plain(&[""])
            } else {
                fs.iter()
                    .map(|&f| (String::new(), d.factor_texts[f].clone()))
                    .collect()
            };
            cols.extend(plain(&["Mean", "Std. Error"]));
            cols.push((ci_group.clone(), "Lower".into()));
            cols.push((ci_group.clone(), "Upper".into()));
            let note = covariate_note(&d, &with);
            tables.push(table(
                "Estimated Marginal Means",
                fs.len().max(1),
                cols,
                out,
                Some(format!("{caption_base}.{note}")),
            ));
            let Some(compare) = &em.compare else {
                continue;
            };
            // a bare COMPARE names the only factor of TABLES
            let compare = if compare.is_empty() {
                match em.tables.as_slice() {
                    [only] => only,
                    _ => {
                        return Err(Error(
                            "EMMEANS COMPARE without a factor needs exactly one factor on TABLES"
                                .into(),
                        ));
                    }
                }
            } else {
                compare
            };
            let Some(at) = fs
                .iter()
                .position(|&f| factor_names[f].eq_ignore_ascii_case(compare))
            else {
                return Err(Error(format!(
                    "EMMEANS COMPARE names {compare}, which is not on TABLES"
                )));
            };
            let f = fs[at];
            let k = levels[f].len();
            let mut pos = |v: &Option<Value>| -> Option<u32> {
                let v = v.as_ref()?;
                match level_of(&factor_vars[f], &levels[f], v) {
                    Some(l) => Some(l as u32 + 1),
                    None => {
                        warnings.push(format!(
                            "CSGLM EMMEANS: {compare} has no level {}; the highest level is the reference",
                            value_text(v)
                        ));
                        None
                    }
                }
            };
            let kind = match &em.contrast {
                CsContrast::Simple(v) => Contrast::Simple(pos(v)),
                CsContrast::Deviation(v) => Contrast::Deviation(pos(v)),
                CsContrast::Difference => Contrast::Difference,
                CsContrast::Helmert => Contrast::Helmert,
                CsContrast::Repeated => Contrast::Repeated,
                CsContrast::Polynomial(m) => Contrast::Polynomial(m.clone()),
            };
            if k < 2 {
                warnings.push(format!(
                    "CSGLM EMMEANS: {compare} has one level, so there is nothing to compare"
                ));
                continue;
            }
            let contrasts =
                contrast_rows(&kind, k).map_err(|e| Error(format!("EMMEANS CONTRAST: {e}")))?;
            let level_names: Vec<String> = (0..k).map(|l| text_of(f, l)).collect();
            let rename = |label: &str| -> String {
                // through placeholders, so that a level name holding "Level n" stays as it is
                let mut s = label.to_string();
                for l in (0..k).rev() {
                    s = s.replace(&format!("Level {}", l + 1), &format!("\u{0}{l}\u{0}"));
                }
                for (l, name) in level_names.iter().enumerate() {
                    s = s.replace(&format!("\u{0}{l}\u{0}"), name);
                }
                s
            };
            let other_fs: Vec<usize> = fs.iter().copied().filter(|&g| g != f).collect();
            let other_sizes: Vec<usize> = other_fs.iter().map(|&g| levels[g].len()).collect();
            let mut out = Vec::new();
            for group in combinations(&other_sizes) {
                // the means of every level of f at this combination of the other factors
                let mut lmu: Vec<Option<Vec<f64>>> = Vec::new();
                for l in 0..k {
                    let combo: Vec<usize> = fs
                        .iter()
                        .map(|&g| {
                            if g == f {
                                l
                            } else {
                                group[other_fs.iter().position(|&x| x == g).expect("other")]
                            }
                        })
                        .collect();
                    lmu.push(mean_of(&combo).map(|(l, _, _)| l));
                }
                let group_cells: Vec<Cell> = other_fs
                    .iter()
                    .zip(&group)
                    .map(|(&g, &l)| text(text_of(g, l)))
                    .collect();
                let estimable = lmu.iter().all(Option::is_some);
                let rows_l: Vec<(String, Option<Vec<f64>>)> = contrasts
                    .iter()
                    .map(|(label, coef)| {
                        let l = estimable.then(|| {
                            let mut l = vec![0.0; p];
                            for (cf, m) in coef.iter().zip(&lmu) {
                                for (a, v) in l.iter_mut().zip(m.as_ref().expect("estimable")) {
                                    *a += cf * v;
                                }
                            }
                            l
                        });
                        (rename(label), l)
                    })
                    .collect();
                let tests: Vec<Option<Wald>> = rows_l
                    .iter()
                    .map(|(_, l)| l.as_ref().and_then(|l| single(l, 0.0)))
                    .collect();
                let ps: Vec<Option<f64>> = tests.iter().map(p_of).collect();
                let keyed: Vec<(Vec<f64>, f64)> = rows_l
                    .iter()
                    .map(|(_, l)| (l.clone().unwrap_or_default(), 0.0))
                    .collect();
                let adjusted = padjust(c.padjust, &ps, &keyed);
                for (idx, (((label, l), sig), w)) in
                    rows_l.iter().zip(&adjusted).zip(&tests).enumerate()
                {
                    let mut row: Vec<Cell> = if idx == 0 {
                        group_cells.clone()
                    } else {
                        vec![text(""); group_cells.len()]
                    };
                    row.push(text(label.clone()));
                    match l {
                        Some(l) => row.extend(row_cells(l, 0.0, w.as_ref(), *sig)),
                        None => row.extend(vec![Cell::Empty; 6 + test_columns(c.test).len()]),
                    }
                    out.push(row);
                }
                let mut overall: Vec<Cell> = vec![text(""); group_cells.len()];
                overall.push(text("Overall Test"));
                overall.extend(std::iter::repeat_n(Cell::Empty, 6));
                let l: Matrix = rows_l.iter().filter_map(|(_, l)| l.clone()).collect();
                let w = if estimable {
                    wald(&est, &l, &vec![0.0; l.len()])
                } else {
                    None
                };
                overall.extend(test_cells(c.test, w.as_ref(), df, None));
                out.push(overall);
            }
            let mut first: Vec<String> = other_fs
                .iter()
                .map(|&g| d.factor_texts[g].clone())
                .collect();
            first.push(format!("{} Contrast", d.factor_texts[f]));
            let first_refs: Vec<&str> = first.iter().map(String::as_str).collect();
            let mut cap = format!("{caption_base}.{note}");
            if let Some(m) = padjust_note(c.padjust) {
                cap.push_str(&format!(
                    " The significance of each contrast is adjusted by the {m} method."
                ));
            }
            tables.push(table(
                "Individual Test Results",
                first.len(),
                detail_columns(&first_refs),
                out,
                Some(cap),
            ));
        }

        // General Estimable Function: logits block diagonal; with thresholds, moving one of them
        // moves the intercept, so its free coefficient takes minus the intercept's row on the
        // redundant columns (the model is threshold - x'b)
        if c.print.gef {
            let base = gef_basis(&h, &fit.redundant);
            let basis: Matrix = if c.ordinal.is_some() {
                let intercept_row = base.first().filter(|_| has_intercept_column);
                let mut out: Matrix = (p..est.len())
                    .map(|t| {
                        let mut row = vec![0.0; est.len()];
                        row[t] = 1.0;
                        if let Some(r) = intercept_row {
                            for j in 1..p {
                                row[j] = -r[j] + 0.0;
                            }
                        }
                        row
                    })
                    .collect();
                let skip = usize::from(has_intercept_column);
                out.extend(base.iter().skip(skip).map(|r| {
                    let mut row = r.clone();
                    row.resize(est.len(), 0.0);
                    row
                }));
                out
            } else {
                blocks(&base)
            };
            tables.push(gef_table(&basis, &order, &param_label, caption.clone()));
        }

        // L matrices
        if c.print.lmatrix {
            for (label, l) in &lmatrices {
                let mut cols = plain(&["Contrast"]);
                cols.extend(order.iter().map(|&a| (String::new(), param_label(a))));
                let out = l
                    .iter()
                    .enumerate()
                    .map(|(i, row)| {
                        let mut cells = vec![text(format!("L{}", i + 1))];
                        cells.extend(order.iter().map(|&a| Cell::Number(row[a])));
                        cells
                    })
                    .collect();
                tables.push(table(
                    "Contrast Coefficients (L Matrix)",
                    1,
                    cols,
                    out,
                    Some(format!("{label}. {caption}")),
                ));
            }
        }

        // SAVE
        let cases = data.case_count();
        let mut saved = Vec::new();
        let put = |values: Vec<(usize, f64)>| -> Vec<Option<f64>> {
            let mut column = vec![None; cases];
            for (case, v) in values {
                column[case] = v.is_finite().then_some(v);
            }
            column
        };
        // (case, probabilities, observed category) of every case with saved probabilities: the
        // analysis cases, and for CSORDINAL also the domain's cases whose dependent is missing
        // but whose predictors all have parameters
        let predictions: Vec<(usize, Vec<f64>, Option<usize>)> = match (c.ordinal, &logistic) {
            _ if c.save.is_empty() => Vec::new(),
            (Some(o), _) => {
                let mut row_at = vec![None; n];
                for (r, &i) in rows.iter().enumerate() {
                    row_at[i] = Some(r);
                }
                (0..n)
                    .filter(|&i| in_domain[i])
                    .filter_map(|i| {
                        let lv: Option<Vec<usize>> = factor_keys
                            .iter()
                            .zip(&levels)
                            .map(|(k, ls)| {
                                let key = k[i].as_ref()?;
                                ls.iter().position(|l| l.compare(key).is_eq())
                            })
                            .collect();
                        let cv: Option<Vec<f64>> = cov_values.iter().map(|v| v[i]).collect();
                        let (x, complete) = row_of(&lv?, &cv?);
                        if !complete {
                            return None;
                        }
                        let xb: f64 = (1..p).map(|j| x[j] * est.b[j]).sum();
                        let gamma: Vec<f64> = (0..thresholds)
                            .map(|t| cdf(o.link, est.b[p + t] - xb))
                            .collect();
                        Some((
                            design.rows[i],
                            cell_probabilities(&gamma),
                            row_at[i].map(|r| category_of[r]),
                        ))
                    })
                    .collect()
            }
            (None, Some(lg)) => lg
                .probs
                .iter()
                .enumerate()
                .map(|(r, probs)| (d.case[r], probs.clone(), Some(category_of[r])))
                .collect(),
            (None, None) => Vec::new(),
        };
        let numeric = |values: Vec<(usize, f64)>, name: Option<String>, root: &str, label| Saved {
            name,
            root: root.into(),
            label,
            numbers: Some(put(values)),
            texts: None,
            category: None,
        };
        for s in &c.save {
            match s {
                CsModelSave::Pred(given) | CsModelSave::Resid(given) => {
                    let is_pred = matches!(s, CsModelSave::Pred(_));
                    let values = (0..rows.len())
                        .map(|r| {
                            let pred: f64 = fit.x[r].iter().zip(&est.b).map(|(a, b)| a * b).sum();
                            (d.case[r], if is_pred { pred } else { d.y[r][0] - pred })
                        })
                        .collect();
                    let (root, what) = if is_pred {
                        ("Predicted", "Predicted Value")
                    } else {
                        ("Residual", "Residual")
                    };
                    saved.push(numeric(
                        values,
                        given.clone(),
                        root,
                        format!("{what} for {dependent}"),
                    ));
                }
                CsModelSave::PredProb(root, count) | CsModelSave::CumProb(root, count) => {
                    let cumulative = matches!(s, CsModelSave::CumProb(..));
                    let root = root.clone().unwrap_or_else(|| {
                        if cumulative {
                            "CumulativeProbability"
                        } else {
                            "PredictedProbability"
                        }
                        .into()
                    });
                    for k in 0..categories.len().min(*count as usize) {
                        let first = if cumulative { 0 } else { k };
                        let values = predictions
                            .iter()
                            .map(|(case, probs, _)| (*case, probs[first..=k].iter().sum()))
                            .collect();
                        let label = if cumulative {
                            format!(
                                "Predicted cumulative probability of {dependent} through {}",
                                category_text(k)
                            )
                        } else {
                            format!(
                                "Predicted probability of {dependent} = {}",
                                category_text(k)
                            )
                        };
                        saved.push(numeric(values, None, &format!("{root}_{}", k + 1), label));
                    }
                }
                CsModelSave::PredValProb(given) | CsModelSave::ObsValProb(given) => {
                    let observed = matches!(s, CsModelSave::ObsValProb(_));
                    let values = predictions
                        .iter()
                        .filter_map(|(case, probs, y)| {
                            let k = if observed { (*y)? } else { predicted(probs) };
                            Some((*case, probs[k]))
                        })
                        .collect();
                    let (root, which) = if observed {
                        ("ObservedValueProbability", "observed")
                    } else {
                        ("PredictedValueProbability", "predicted")
                    };
                    saved.push(numeric(
                        values,
                        given.clone(),
                        root,
                        format!("Probability of the {which} category of {dependent}"),
                    ));
                }
                CsModelSave::PredVal(given) => {
                    let best: Vec<(usize, usize)> = predictions
                        .iter()
                        .map(|(case, probs, _)| (*case, predicted(probs)))
                        .collect();
                    let label = format!("Predicted value of {dependent}");
                    let category = Some((
                        dep_var.format().to_string(),
                        dep_var.value_labels().to_vec(),
                    ));
                    if let Some(width) = dep_var.string_width() {
                        let mut texts = vec![String::new(); cases];
                        for &(case, k) in &best {
                            if let Key::Text(t) = &categories[k] {
                                texts[case] = t.clone();
                            }
                        }
                        saved.push(Saved {
                            name: given.clone(),
                            root: "PredictedValue".into(),
                            label,
                            numbers: None,
                            texts: Some((width, texts)),
                            category,
                        });
                    } else {
                        let values = best
                            .iter()
                            .map(|&(case, k)| match categories[k] {
                                Key::Number(v) => (case, v),
                                Key::Text(_) => (case, f64::NAN),
                            })
                            .collect();
                        saved.push(Saved {
                            name: given.clone(),
                            root: "PredictedValue".into(),
                            label,
                            numbers: Some(put(values)),
                            texts: None,
                            category,
                        });
                    }
                }
            }
        }
        Ok((tables, saved, cases, files))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_estimable_functions_of_a_factor_a_covariate_and_their_interaction() {
        // (level 1..3, covariate, weight): two covariate values per level, uneven weights
        let cases = [
            (1, 2.0, 1.0),
            (1, 5.0, 3.0),
            (2, 1.0, 2.0),
            (2, 4.0, 1.5),
            (2, 6.0, 1.0),
            (3, 3.0, 2.5),
            (3, 7.0, 1.0),
        ];
        // columns: intercept, a1, a2, a3, x, a1*x, a2*x, a3*x
        let x: Matrix = cases
            .iter()
            .map(|&(level, v, _)| {
                let mut row = vec![1.0, 0.0, 0.0, 0.0, v, 0.0, 0.0, 0.0];
                row[level] = 1.0;
                row[4 + level] = v;
                row
            })
            .collect();
        let mut xwx = vec![vec![0.0; 8]; 8];
        for (row, &(_, _, w)) in x.iter().zip(&cases) {
            for i in 0..8 {
                for j in 0..8 {
                    xwx[i][j] += w * row[i] * row[j];
                }
            }
        }
        let (g, rank) = ginverse(&xwx, 1e-12);
        assert_eq!(rank, 6);
        let h = multiply(&g, &xwx);
        let redundant: Vec<bool> = (0..8).map(|j| g[j][j] == 0.0).collect();
        assert_eq!(
            redundant,
            [false, false, false, true, false, false, false, true]
        );
        let basis = gef_basis(&h, &redundant);
        // a3 = intercept - a1 - a2 and a3*x = x - a1*x - a2*x, so a3 takes L1 - L2 - L3 and
        // a3*x takes L4 - L5 - L6
        let expected: Matrix = vec![
            vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -1.0],
            vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, -1.0],
        ];
        assert_eq!(basis, expected);
        // every row is estimable: L = L (X'WX)^- (X'WX)
        for l in &basis {
            for j in 0..8 {
                let lh: f64 = (0..8).map(|i| l[i] * h[i][j]).sum();
                assert!((lh - l[j]).abs() < 1e-9, "{l:?} column {j}: {lh}");
            }
        }
    }
}
