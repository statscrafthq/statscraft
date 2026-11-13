//! PLUM: ordinal regression with a cumulative link model.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (PLUM), in our words:
//! - The dependent's values, sorted ascending, are the ordered categories 1 to J. Factors may be
//!   numbers or strings, levels sorted ascending; covariates are numeric. A case counts when
//!   every variable is valid (listwise; MISSING=INCLUDE takes user-missing values as valid) and
//!   its weight, rounded to a whole number, is positive. The rounded weight is a frequency.
//! - The model: link(P(Y <= j)) = (theta_j - x'beta) / exp(z'tau) for j below J. x holds the
//!   location effects, z the scale effects. A factor effect has one column per level combination
//!   that occurs; a column that adds nothing to the earlier ones and an intercept is redundant
//!   and fixed at 0 (the last level of a main effect).
//! - The cases are pooled into subpopulations, one per combination of factor levels and
//!   covariate values. DELTA is added to the empty cells of a subpopulation, BIAS to every cell.
//! - Fitting starts at the thresholds of the marginal cumulative proportions and zero for the
//!   rest; with SCALE the location-only model is fitted first. Newton-Raphson steps are halved
//!   up to MXSTEP times until the log-likelihood does not fall; out-of-order thresholds are set
//!   to their mean. The fit stops when every parameter moves less than PCONVERGE or the
//!   log-likelihood changes (absolutely or relatively) less than LCONVERGE, or after MXITER.
//! - The log-likelihood is the sum of n_ij log(pi_ij), the kernel. The -2 log-likelihood shown
//!   adds the multinomial constant of the subpopulations, unless PRINT=KERNEL. The pseudo
//!   R-squares use the kernel. Pearson and deviance compare the subpopulation cells with their
//!   expected counts on m(J-1) minus the number of estimated parameters.
//! - The parallel lines test refits the location-only model with separate location parameters
//!   for each threshold and compares the two by their likelihoods, on (J-2) times the number of
//!   location parameters.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, ln_gamma, normal_cdf, normal_quantile};
use crate::glm::{
    Data, Effect, design, effects_of, find_effect, ginverse, key_value_text, parameter_name,
};
use crate::matrix::{Matrix, inverse};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::shown::Shown;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Measure, Values, Variable};
use statscraft_syntax::{CustomTest, Plum, PlumLink, PlumSave, Term};
use std::f64::consts::PI;

/// The inverse link F, its density f and the derivative of the density.
fn cdf(link: PlumLink, x: f64) -> f64 {
    match link {
        PlumLink::Logit => 1.0 / (1.0 + (-x).exp()),
        PlumLink::Probit => normal_cdf(x),
        PlumLink::Cloglog => -(-x.exp()).exp_m1(),
        PlumLink::Nloglog => (-(-x).exp()).exp(),
        PlumLink::Cauchit => 0.5 + x.atan() / PI,
    }
}

fn pdf(link: PlumLink, x: f64) -> f64 {
    match link {
        PlumLink::Logit => {
            let e = (-x.abs()).exp();
            e / ((1.0 + e) * (1.0 + e))
        }
        PlumLink::Probit => (-0.5 * x * x).exp() / (2.0 * PI).sqrt(),
        PlumLink::Cloglog => (x - x.exp()).exp(),
        PlumLink::Nloglog => (-x - (-x).exp()).exp(),
        PlumLink::Cauchit => 1.0 / (PI * (1.0 + x * x)),
    }
}

fn pdf_slope(link: PlumLink, x: f64) -> f64 {
    let f = pdf(link, x);
    match link {
        PlumLink::Logit => f * (1.0 - 2.0 * cdf(link, x)),
        PlumLink::Probit => -x * f,
        PlumLink::Cloglog => f * (1.0 - x.exp()),
        PlumLink::Nloglog => f * ((-x).exp() - 1.0),
        PlumLink::Cauchit => -2.0 * x * f / (1.0 + x * x),
    }
}

/// link(p), the inverse of `cdf`.
fn quantile(link: PlumLink, p: f64) -> f64 {
    match link {
        PlumLink::Logit => (p / (1.0 - p)).ln(),
        PlumLink::Probit => normal_quantile(p),
        PlumLink::Cloglog => (-(-p).ln_1p()).ln(),
        PlumLink::Nloglog => -(-p.ln()).ln(),
        PlumLink::Cauchit => (PI * (p - 0.5)).tan(),
    }
}

fn link_name(link: PlumLink) -> &'static str {
    match link {
        PlumLink::Logit => "Logit",
        PlumLink::Probit => "Probit",
        PlumLink::Cloglog => "Complementary Log-log",
        PlumLink::Nloglog => "Negative Log-log",
        PlumLink::Cauchit => "Cauchit",
    }
}

/// A cumulative link model on pooled counts. The parameters are the J-1 thresholds, the
/// location parameters (one set, or one per threshold when not parallel) and the scale ones.
struct Model<'a> {
    link: PlumLink,
    /// The count of each category in each subpopulation.
    counts: &'a [Vec<f64>],
    x: &'a [Vec<f64>],
    z: &'a [Vec<f64>],
    parallel: bool,
    scale: bool,
}

struct Eval {
    l: f64,
    gradient: Vec<f64>,
    /// The second derivative of l.
    hessian: Matrix,
    /// The expected information, minus the expected second derivative.
    information: Matrix,
}

impl Model<'_> {
    fn k(&self) -> usize {
        self.counts[0].len() - 1
    }

    fn px(&self) -> usize {
        self.x.first().map_or(0, Vec::len)
    }

    fn pz(&self) -> usize {
        if self.scale {
            self.z.first().map_or(0, Vec::len)
        } else {
            0
        }
    }

    fn location_size(&self) -> usize {
        if self.parallel {
            self.px()
        } else {
            self.k() * self.px()
        }
    }

    fn size(&self) -> usize {
        self.k() + self.location_size() + self.pz()
    }

    /// The linear predictors of subpopulation i and their first and second derivatives (the
    /// second only with a scale part).
    #[allow(clippy::type_complexity)]
    fn predictors(&self, b: &[f64], i: usize) -> (Vec<f64>, Vec<Vec<f64>>, Vec<Option<Matrix>>) {
        let (k, px, pz, n) = (self.k(), self.px(), self.pz(), self.size());
        let x = &self.x[i];
        let start_z = k + self.location_size();
        let s = if pz > 0 {
            let zt: f64 = (0..pz).map(|d| self.z[i][d] * b[start_z + d]).sum();
            (-zt).exp()
        } else {
            1.0
        };
        let mut eta = Vec::with_capacity(k);
        let mut first = Vec::with_capacity(k);
        let mut second = Vec::with_capacity(k);
        for j in 0..k {
            let offset = if self.parallel { k } else { k + j * px };
            let xb: f64 = (0..px).map(|c| x[c] * b[offset + c]).sum();
            let e = (b[j] - xb) * s;
            let mut d = vec![0.0; n];
            d[j] = s;
            let factor = if self.parallel { s } else { 1.0 };
            for c in 0..px {
                d[offset + c] = -x[c] * factor;
            }
            for q in 0..pz {
                d[start_z + q] = -self.z[i][q] * e;
            }
            let dd = (pz > 0).then(|| {
                let mut m = vec![vec![0.0; n]; n];
                for q in 0..pz {
                    let zq = self.z[i][q];
                    m[j][start_z + q] = -zq * s;
                    m[start_z + q][j] = -zq * s;
                    for c in 0..px {
                        m[offset + c][start_z + q] = x[c] * zq * s;
                        m[start_z + q][offset + c] = x[c] * zq * s;
                    }
                    for r in 0..pz {
                        m[start_z + q][start_z + r] = zq * self.z[i][r] * e;
                    }
                }
                m
            });
            eta.push(e);
            first.push(d);
            second.push(dd);
        }
        (eta, first, second)
    }

    /// The cumulative probabilities of subpopulation i, J-1 of them.
    fn cumulative(&self, b: &[f64], i: usize) -> Vec<f64> {
        let (eta, _, _) = self.predictors(b, i);
        eta.iter().map(|&e| cdf(self.link, e)).collect()
    }

    /// The log-likelihood with its derivatives; None where a cell with cases has no probability.
    fn eval(&self, b: &[f64]) -> Option<Eval> {
        let (k, n) = (self.k(), self.size());
        let mut l = 0.0;
        let mut gradient = vec![0.0; n];
        let mut hessian = vec![vec![0.0; n]; n];
        let mut information = vec![vec![0.0; n]; n];
        for (i, counts) in self.counts.iter().enumerate() {
            let total: f64 = counts.iter().sum();
            if total <= 0.0 {
                continue;
            }
            let (eta, first, second) = self.predictors(b, i);
            let gamma: Vec<f64> = eta.iter().map(|&e| cdf(self.link, e)).collect();
            let f: Vec<f64> = eta.iter().map(|&e| pdf(self.link, e)).collect();
            let g: Vec<f64> = eta.iter().map(|&e| pdf_slope(self.link, e)).collect();
            for (c, &count) in counts.iter().enumerate() {
                let upper = if c < k { gamma[c] } else { 1.0 };
                let lower = if c > 0 { gamma[c - 1] } else { 0.0 };
                let pi = upper - lower;
                if pi <= 0.0 || !pi.is_finite() {
                    if count > 0.0 {
                        return None;
                    }
                    continue;
                }
                let mut dpi = vec![0.0; n];
                if c < k {
                    for (t, v) in dpi.iter_mut().zip(&first[c]) {
                        *t += f[c] * v;
                    }
                }
                if c > 0 {
                    for (t, v) in dpi.iter_mut().zip(&first[c - 1]) {
                        *t -= f[c - 1] * v;
                    }
                }
                for a in 0..n {
                    if dpi[a] == 0.0 {
                        continue;
                    }
                    for bb in 0..n {
                        information[a][bb] += total * dpi[a] * dpi[bb] / pi;
                    }
                }
                if count <= 0.0 {
                    continue;
                }
                l += count * pi.ln();
                for a in 0..n {
                    gradient[a] += count * dpi[a] / pi;
                }
                // second derivative of pi: f' d eta d eta' + f d2 eta at both bounds
                let mut add = |j: usize, sign: f64| {
                    let w = sign * count / pi;
                    for a in 0..n {
                        if first[j][a] == 0.0 {
                            continue;
                        }
                        for bb in 0..n {
                            hessian[a][bb] += w * g[j] * first[j][a] * first[j][bb];
                        }
                    }
                    if let Some(m) = &second[j] {
                        for a in 0..n {
                            for bb in 0..n {
                                hessian[a][bb] += w * f[j] * m[a][bb];
                            }
                        }
                    }
                };
                if c < k {
                    add(c, 1.0);
                }
                if c > 0 {
                    add(c - 1, -1.0);
                }
                for a in 0..n {
                    if dpi[a] == 0.0 {
                        continue;
                    }
                    for bb in 0..n {
                        hessian[a][bb] -= count * dpi[a] * dpi[bb] / (pi * pi);
                    }
                }
            }
        }
        Some(Eval {
            l,
            gradient,
            hessian,
            information,
        })
    }
}

/// One row of the iteration history: step-halvings, kernel log-likelihood, parameters.
struct Iteration {
    halvings: u32,
    l: f64,
    b: Vec<f64>,
}

struct Estimate {
    b: Vec<f64>,
    eval: Eval,
    history: Vec<Iteration>,
    converged: bool,
}

/// Puts out-of-order thresholds back in order: a pair out of order both take their mean.
fn order_thresholds(b: &mut [f64], k: usize) {
    for j in 0..k.saturating_sub(1) {
        if b[j] > b[j + 1] {
            let mean = (b[j] + b[j + 1]) / 2.0;
            b[j] = mean;
            b[j + 1] = mean;
        }
    }
}

fn solve(a: &Matrix, v: &[f64]) -> Option<Vec<f64>> {
    let (inv, _) = inverse(a)?;
    Some(
        inv.iter()
            .map(|row| row.iter().zip(v).map(|(x, y)| x * y).sum())
            .collect(),
    )
}

fn negated(m: &Matrix) -> Matrix {
    m.iter().map(|r| r.iter().map(|v| -v).collect()).collect()
}

fn maximize(
    model: &Model,
    start: Vec<f64>,
    c: &statscraft_syntax::PlumCriteria,
) -> Option<Estimate> {
    let k = model.k();
    let mut b = start;
    let mut eval = model.eval(&b)?;
    let mut history = vec![Iteration {
        halvings: 0,
        l: eval.l,
        b: b.clone(),
    }];
    let mut converged = c.mxiter == 0;
    for _ in 0..c.mxiter {
        // Newton-Raphson; the expected information where the observed one gives no ascent
        let direction = solve(&negated(&eval.hessian), &eval.gradient)
            .filter(|d| {
                d.iter()
                    .zip(&eval.gradient)
                    .map(|(a, g)| a * g)
                    .sum::<f64>()
                    > 0.0
            })
            .or_else(|| solve(&eval.information, &eval.gradient))?;
        let mut step = 1.0;
        let mut accepted = None;
        for halvings in 0..c.mxstep {
            let mut next: Vec<f64> = b
                .iter()
                .zip(&direction)
                .map(|(v, d)| v + step * d)
                .collect();
            if model.parallel || model.px() == 0 {
                order_thresholds(&mut next, k);
            }
            if let Some(e) = model.eval(&next)
                && e.l >= eval.l - 1e-12 * (1.0 + eval.l.abs())
            {
                accepted = Some((next, e, halvings));
                break;
            }
            step /= 2.0;
        }
        let Some((next, next_eval, halvings)) = accepted else {
            break;
        };
        let change = b
            .iter()
            .zip(&next)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        let dl = (next_eval.l - eval.l).abs();
        b = next;
        eval = next_eval;
        history.push(Iteration {
            halvings,
            l: eval.l,
            b: b.clone(),
        });
        if (c.pconverge > 0.0 && change < c.pconverge)
            || (c.lconverge > 0.0 && (dl < c.lconverge || dl < c.lconverge * eval.l.abs()))
        {
            converged = true;
            break;
        }
    }
    Some(Estimate {
        b,
        eval,
        history,
        converged,
    })
}

/// TEST: each row of coefficients over every parameter (`at` places a parameter in the
/// estimates, None when it is redundant) is tested alone and all rows jointly by Wald
/// chi-squares, (L b - c)' (L C L')^- (L b - c) on the rank of L C L'.
// shortcut: a coefficient on a redundant parameter multiplies its fixed 0, with no
// estimability check; check estimability if users write tests on those parameters.
#[allow(clippy::too_many_arguments)]
pub(crate) fn wald_tests(
    t: &CustomTest,
    n: usize,
    rows: &[Vec<f64>],
    at: &[Option<usize>],
    b: &[f64],
    cov: &Matrix,
    cin: f64,
    caption: &str,
    warnings: &mut Vec<String>,
) -> Vec<Table> {
    let title = t
        .label
        .clone()
        .unwrap_or_else(|| format!("Custom Hypothesis Test {n}"));
    let z = normal_quantile(0.5 + cin / 200.0);
    let mut out_rows = Vec::new();
    let mut l: Matrix = Vec::new();
    let mut c = Vec::new();
    for (r, (full, constant)) in rows.iter().zip(&t.constants).enumerate() {
        let mut row = vec![0.0; b.len()];
        for (v, a) in full.iter().zip(at) {
            if let Some(a) = a {
                row[*a] += v;
            }
        }
        let name = format!("L{}", r + 1);
        let var: f64 = (0..b.len())
            .map(|i| {
                (0..b.len())
                    .map(|j| row[i] * cov[i][j] * row[j])
                    .sum::<f64>()
            })
            .sum();
        if row.iter().all(|v| *v == 0.0) || var.is_nan() || var <= 0.0 {
            warnings.push(format!(
                "TEST {title}: {name} has no coefficient on an estimated parameter and is not tested"
            ));
            continue;
        }
        let est: f64 = row.iter().zip(b).map(|(x, y)| x * y).sum();
        let se = var.sqrt();
        let wald = ((est - constant) / se).powi(2);
        out_rows.push(vec![
            text(name),
            Cell::Number(est),
            Cell::Number(*constant),
            Cell::Number(se),
            Cell::Number(wald),
            Cell::Number(1.0),
            Cell::Number(chi_square_p(wald, 1.0)),
            Cell::Number(est - z * se),
            Cell::Number(est + z * se),
        ]);
        l.push(row);
        c.push(*constant);
    }
    if l.is_empty() {
        return Vec::new();
    }
    let caption = format!("{title}. {caption}");
    let mut columns = plain(&[
        "Contrast",
        "Estimate",
        "Test Value",
        "Std. Error",
        "Wald",
        "df",
        "Sig.",
    ]);
    let group = format!("{} Confidence Interval", percent(cin));
    columns.push((group.clone(), "Lower Bound".into()));
    columns.push((group, "Upper Bound".into()));
    let lc: Matrix = l
        .iter()
        .map(|x| {
            (0..b.len())
                .map(|j| x.iter().zip(cov).map(|(v, row)| v * row[j]).sum())
                .collect()
        })
        .collect();
    let lcl: Matrix = lc
        .iter()
        .map(|x| {
            l.iter()
                .map(|y| x.iter().zip(y).map(|(p, q)| p * q).sum())
                .collect()
        })
        .collect();
    let (inv, rank) = ginverse(&lcl, 1e-10);
    let d: Vec<f64> = l
        .iter()
        .zip(&c)
        .map(|(x, ci)| x.iter().zip(b).map(|(p, q)| p * q).sum::<f64>() - ci)
        .collect();
    let wald: f64 = (0..d.len())
        .map(|i| (0..d.len()).map(|j| d[i] * inv[i][j] * d[j]).sum::<f64>())
        .sum();
    vec![
        table(
            "Contrast Estimates",
            1,
            columns,
            out_rows,
            Some(caption.clone()),
        ),
        table(
            "Test Results",
            1,
            plain(&["Source", "Wald", "df", "Sig."]),
            vec![vec![
                text(title),
                Cell::Number(wald),
                Cell::Number(rank as f64),
                Cell::Number(chi_square_p(wald, rank as f64)),
            ]],
            Some(caption),
        ),
    ]
}

/// The multinomial constant log(n_i! / prod n_ij!) summed over the subpopulations.
fn multinomial_constant(counts: &[Vec<f64>]) -> f64 {
    counts
        .iter()
        .map(|row| {
            ln_gamma(row.iter().sum::<f64>() + 1.0)
                - row.iter().map(|&n| ln_gamma(n + 1.0)).sum::<f64>()
        })
        .sum()
}

/// A new variable of SAVE: name, label, its value for a case, and whether it is a category.
type SaveColumn = (
    String,
    String,
    Box<dyn Fn(&Prediction) -> Option<f64>>,
    bool,
);

/// What SAVE writes for one case.
struct Prediction {
    probabilities: Vec<f64>,
    predicted: usize,
    actual: Option<usize>,
}

fn columns_with_intercept(x: &[Vec<f64>], w: &[f64], singular: f64) -> Vec<bool> {
    // a column is redundant when it adds nothing to the intercept and the columns before it
    let p = x.first().map_or(0, Vec::len);
    let mut xwx = vec![vec![0.0; p + 1]; p + 1];
    for (row, &wi) in x.iter().zip(w) {
        let r: Vec<f64> = std::iter::once(1.0).chain(row.iter().copied()).collect();
        for a in 0..=p {
            if r[a] == 0.0 {
                continue;
            }
            for b in 0..=p {
                xwx[a][b] += wi * r[a] * r[b];
            }
        }
    }
    let (g, _) = ginverse(&xwx, singular);
    (1..=p).map(|j| g[j][j] == 0.0).collect()
}

impl Engine {
    pub(crate) fn plum(&mut self, command: &Plum) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let criteria = command.criteria;
        let factor_names = self.expand(&command.factors)?;
        let covariate_names = self.expand(&command.covariates)?;
        let dependent = data.variable(&command.dependent)?;
        let mut seen: Vec<String> = vec![dependent.name().to_string()];
        for n in factor_names.iter().chain(&covariate_names) {
            if seen.iter().any(|s| s.eq_ignore_ascii_case(n)) {
                return Err(Error(format!(
                    "{n} appears more than once among the dependent variable, factors and covariates"
                )));
            }
            seen.push(n.clone());
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
                "PLUM needs numeric covariates, but {} is a string variable",
                v.name()
            )));
        }
        let cases = data.case_count();
        let weights = case_weights(data)?;
        let freq = |k: usize| weights.as_ref().map_or(1.0, |w| w[k].round());
        let dep_keys = categories(&dependent, command.include);
        let factor_keys: Vec<Vec<Option<Key>>> = factor_vars
            .iter()
            .map(|v| categories(v, command.include))
            .collect();
        let covariate = |v: &Variable, k: usize| -> Option<f64> {
            if !crate::means::is_valid(v, k, command.include) {
                return None;
            }
            match v.values() {
                Values::Numeric(values) => values.get(k),
                Values::String(_) => None,
            }
        };
        let predictors_valid: Vec<bool> = (0..cases)
            .map(|k| {
                factor_keys.iter().all(|col| col[k].is_some())
                    && cov_vars.iter().all(|v| covariate(v, k).is_some())
            })
            .collect();
        let rows: Vec<usize> = (0..cases)
            .filter(|&k| predictors_valid[k] && dep_keys[k].is_some() && freq(k) > 0.0)
            .collect();
        let missing: f64 = (0..cases)
            .filter(|&k| freq(k) > 0.0 && !(predictors_valid[k] && dep_keys[k].is_some()))
            .map(freq)
            .sum();
        if rows.is_empty() {
            return Err(Error(
                "PLUM: no case has valid values on every variable and a positive weight".into(),
            ));
        }
        let sorted_levels = |col: &[Option<Key>]| -> Vec<Key> {
            let mut found: Vec<Key> = Vec::new();
            for &k in &rows {
                let key = col[k].as_ref().expect("valid");
                if !found.iter().any(|l| l.compare(key).is_eq()) {
                    found.push(clone_key(key));
                }
            }
            found.sort_by(|a, b| a.compare(b));
            found
        };
        let dep_levels = sorted_levels(&dep_keys);
        let levels: Vec<Vec<Key>> = factor_keys.iter().map(|c| sorted_levels(c)).collect();
        let jn = dep_levels.len();
        if jn < 2 {
            return Err(Error(format!(
                "PLUM needs a dependent variable with at least two values among the cases used; {} has {jn}",
                dependent.name()
            )));
        }
        let level_of = |keys: &[Key], key: &Key| keys.iter().position(|l| l.compare(key).is_eq());
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
            labelled: false,
            covariates: covariate_names.clone(),
            level: rows
                .iter()
                .map(|&k| {
                    factor_keys
                        .iter()
                        .zip(&levels)
                        .map(|(col, ls)| {
                            level_of(ls, col[k].as_ref().expect("valid")).expect("a level")
                        })
                        .collect()
                })
                .collect(),
            cov: rows
                .iter()
                .map(|&k| {
                    cov_vars
                        .iter()
                        .map(|v| covariate(v, k).expect("valid"))
                        .collect()
                })
                .collect(),
            y: vec![Vec::new(); rows.len()],
            f: rows.iter().map(|&k| freq(k)).collect(),
            g: vec![1.0; rows.len()],
            case: rows.clone(),
        };
        let category: Vec<usize> = rows
            .iter()
            .map(|&k| {
                level_of(&dep_levels, dep_keys[k].as_ref().expect("valid")).expect("a category")
            })
            .collect();
        // the location effects: by default the covariates, then the factors' main effects
        let location_terms: Vec<Term> = match &command.location {
            Some(terms) => terms.clone(),
            None => covariate_names
                .iter()
                .chain(&factor_names)
                .map(|n| Term {
                    variables: vec![n.clone()],
                    label: n.clone(),
                })
                .collect(),
        };
        let scale_terms: Vec<Term> = command.scale.clone().unwrap_or_default();
        let mut location = effects_of(&location_terms, &factor_names, &covariate_names)?;
        let mut scale = effects_of(&scale_terms, &factor_names, &covariate_names)?;
        let x_full = design(&d, &mut location);
        let z_full = design(&d, &mut scale);
        // the last level is the redundant one, as the manual says (R's polr drops the first)
        let x_redundant = columns_with_intercept(&x_full, &d.f, criteria.singular);
        let z_redundant = columns_with_intercept(&z_full, &d.f, criteria.singular);
        let keep = |row: &[f64], redundant: &[bool]| -> Vec<f64> {
            row.iter()
                .zip(redundant)
                .filter(|(_, r)| !**r)
                .map(|(v, _)| *v)
                .collect()
        };
        // subpopulations: the combinations of factor levels and covariate values
        let mut order: Vec<usize> = (0..rows.len()).collect();
        let pattern_cmp = |a: &usize, b: &usize| {
            d.level[*a].cmp(&d.level[*b]).then_with(|| {
                d.cov[*a]
                    .iter()
                    .zip(&d.cov[*b])
                    .map(|(x, y)| x.total_cmp(y))
                    .find(|o| o.is_ne())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        };
        order.sort_by(pattern_cmp);
        let mut sub_of = vec![0; rows.len()];
        let mut first_row: Vec<usize> = Vec::new();
        for (at, &r) in order.iter().enumerate() {
            if at == 0 || pattern_cmp(&order[at - 1], &r).is_ne() {
                first_row.push(r);
            }
            sub_of[r] = first_row.len() - 1;
        }
        let m = first_row.len();
        let mut observed = vec![vec![0.0; jn]; m];
        for (r, &c) in category.iter().enumerate() {
            observed[sub_of[r]][c] += d.f[r];
        }
        let counts: Vec<Vec<f64>> = observed
            .iter()
            .map(|row| {
                row.iter()
                    .map(|&n| if n == 0.0 { n + criteria.delta } else { n } + criteria.bias)
                    .collect()
            })
            .collect();
        let x: Vec<Vec<f64>> = first_row
            .iter()
            .map(|&r| keep(&x_full[r], &x_redundant))
            .collect();
        let z: Vec<Vec<f64>> = first_row
            .iter()
            .map(|&r| keep(&z_full[r], &z_redundant))
            .collect();
        let k = jn - 1;
        let total: f64 = counts.iter().flatten().sum();
        let mut cumulative = 0.0;
        let start: Vec<f64> = (0..k)
            .map(|j| {
                cumulative += counts.iter().map(|row| row[j]).sum::<f64>();
                quantile(command.link, cumulative / total)
            })
            .collect();
        let has_scale = z.first().is_some_and(|r| !r.is_empty());
        let location_model = Model {
            link: command.link,
            counts: &counts,
            x: &x,
            z: &z,
            parallel: true,
            scale: false,
        };
        let pad = |b: &[f64], n: usize| -> Vec<f64> {
            let mut v = b.to_vec();
            v.resize(n, 0.0);
            v
        };
        let failed =
            || Error("The model could not be fitted; a cell with cases has no probability".into());
        let mut warnings: Vec<String> = Vec::new();
        let b0 = pad(&start, location_model.size());
        let l0 = location_model.eval(&b0).ok_or_else(failed)?.l;
        let located = maximize(&location_model, b0, &criteria).ok_or_else(failed)?;
        let full_model = Model {
            scale: true,
            ..location_model
        };
        let fit = if has_scale {
            let general = maximize(&full_model, pad(&located.b, full_model.size()), &criteria)
                .ok_or_else(failed)?;
            Some(general)
        } else {
            None
        };
        let (model, fit) = match fit {
            Some(f) => (&full_model, f),
            None => (&location_model, located),
        };
        if !fit.converged {
            warnings.push(format!(
                "The estimates did not converge within {} iterations (MXITER) or the log-likelihood could not be increased by step-halving (MXSTEP); the results may not be final",
                criteria.mxiter
            ));
        }
        let size = model.size();
        let (covariance, rank) = {
            let observed_info = negated(&fit.eval.hessian);
            let (g, rank) = ginverse(&observed_info, criteria.singular);
            if (0..size).all(|i| g[i][i] > 0.0) {
                (g, rank)
            } else {
                ginverse(&fit.eval.information, criteria.singular)
            }
        };
        if rank < size {
            warnings.push(
                "The information matrix is singular; the standard errors of some parameters are not shown".into(),
            );
        }
        let constant = if command.print.is_some_and(|p| p.kernel) {
            0.0
        } else {
            multinomial_constant(&counts)
        };
        let empty = observed.iter().flatten().filter(|&&n| n == 0.0).count();
        if empty > 0 {
            let cells = m * jn;
            warnings.push(format!(
                "{empty} of the {cells} cells ({:.1}%) have no cases. A cell is one category of the dependent variable within one observed combination of factor levels and covariate values. With many empty cells the goodness-of-fit statistics are not reliable",
                100.0 * empty as f64 / cells as f64
            ));
        }
        // the tables
        let print = command.print.unwrap_or(statscraft_syntax::PlumPrint {
            fit: true,
            summary: true,
            parameter: true,
            ..Default::default()
        });
        let link_caption = format!("Link function: {}.", link_name(command.link));
        let n_used: f64 = d.f.iter().sum();
        let mut tables = Vec::new();
        // Case Processing Summary
        {
            let mut out: Vec<Vec<Cell>> = Vec::new();
            let mut block = |v: &Variable, ls: &[Key], of: &dyn Fn(usize) -> usize| {
                let mut n = vec![0.0; ls.len()];
                for r in 0..rows.len() {
                    n[of(r)] += d.f[r];
                }
                for (l, key) in ls.iter().enumerate() {
                    out.push(vec![
                        if l == 0 {
                            text(shown.variable(v))
                        } else {
                            Cell::Empty
                        },
                        text(shown.value(v, key)),
                        Cell::Number(n[l]),
                        Cell::Number(100.0 * n[l] / n_used),
                    ]);
                }
            };
            block(&dependent, &dep_levels, &|r| category[r]);
            for (f, v) in factor_vars.iter().enumerate() {
                block(v, &levels[f], &|r| d.level[r][f]);
            }
            for (name, n) in [
                ("Valid", n_used),
                ("Missing", missing),
                ("Total", n_used + missing),
            ] {
                out.push(vec![text(name), Cell::Empty, Cell::Number(n), Cell::Empty]);
            }
            tables.push(table(
                "Case Processing Summary",
                2,
                plain(&["", "", "N", "Marginal Percentage"]),
                out,
                None,
            ));
        }
        if let Some(every) = print.history {
            tables.push(history_table(&fit.history, every, constant, size));
        }
        let ll_final = fit.eval.l;
        let estimated = rank;
        let chi = 2.0 * (ll_final - l0);
        let df_model = estimated.saturating_sub(k) as f64;
        tables.push(table(
            "Model Fitting Information",
            1,
            plain(&["Model", "-2 Log Likelihood", "Chi-Square", "df", "Sig."]),
            vec![
                vec![
                    text("Intercept Only"),
                    Cell::Number(-2.0 * (l0 + constant)),
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                ],
                vec![
                    text("Final"),
                    Cell::Number(-2.0 * (ll_final + constant)),
                    Cell::Number(chi),
                    Cell::Number(df_model),
                    number(Some(chi_square_p(chi.max(0.0), df_model))),
                ],
            ],
            Some(link_caption.clone()),
        ));
        if print.fit {
            let mut pearson = 0.0;
            let mut deviance = 0.0;
            for (i, row) in counts.iter().enumerate() {
                let ni: f64 = row.iter().sum();
                let probs = cell_probabilities(&model.cumulative(&fit.b, i));
                for (c, &n) in row.iter().enumerate() {
                    let e = ni * probs[c];
                    if e > 0.0 {
                        pearson += (n - e) * (n - e) / e;
                    }
                    if n > 0.0 {
                        deviance += 2.0 * n * (n / e).ln();
                    }
                }
            }
            let df = (m * k) as f64 - estimated as f64;
            let row = |name: &str, v: f64| {
                vec![
                    text(name),
                    Cell::Number(v),
                    Cell::Number(df),
                    number((df > 0.0).then(|| chi_square_p(v.max(0.0), df))),
                ]
            };
            tables.push(table(
                "Goodness-of-Fit",
                1,
                plain(&["", "Chi-Square", "df", "Sig."]),
                vec![row("Pearson", pearson), row("Deviance", deviance)],
                Some(link_caption.clone()),
            ));
        }
        if print.summary {
            let cs = 1.0 - (2.0 * (l0 - ll_final) / total).exp();
            let nagelkerke = cs / (1.0 - (2.0 * l0 / total).exp());
            let mcfadden = 1.0 - ll_final / l0;
            tables.push(table(
                "Pseudo R-Square",
                1,
                plain(&["", "Value"]),
                vec![
                    vec![text("Cox and Snell"), number(Some(cs))],
                    vec![text("Nagelkerke"), number(Some(nagelkerke))],
                    vec![text("McFadden"), number(Some(mcfadden))],
                ],
                Some(link_caption.clone()),
            ));
        }
        // the parameter names and where each sits in the estimate vector
        let mut parameters: Vec<(&str, String, Option<usize>)> = Vec::new();
        for (j, key) in dep_levels.iter().take(k).enumerate() {
            parameters.push((
                "Threshold",
                format!("[{}={}]", dependent.name(), key_value_text(&dependent, key)),
                Some(j),
            ));
        }
        let mut at = k;
        let mut add_effects =
            |group: &'static str,
             effects: &[Effect],
             redundant: &[bool],
             parameters: &mut Vec<(&str, String, Option<usize>)>| {
                for e in effects {
                    for (c, cell) in e.cells.iter().enumerate() {
                        let name = parameter_name(&d, e, cell);
                        if redundant[e.start + c] {
                            parameters.push((group, name, None));
                        } else {
                            parameters.push((group, name, Some(at)));
                            at += 1;
                        }
                    }
                }
            };
        add_effects("Location", &location, &x_redundant, &mut parameters);
        if has_scale {
            add_effects("Scale", &scale, &z_redundant, &mut parameters);
        }
        if print.parameter {
            let level = criteria.cin;
            let z = normal_quantile(0.5 + level / 200.0);
            let group = format!("{} Confidence Interval", percent(level));
            let mut columns = plain(&["", "", "Estimate", "Std. Error", "Wald", "df", "Sig."]);
            columns.push((group.clone(), "Lower Bound".into()));
            columns.push((group, "Upper Bound".into()));
            let mut out = Vec::new();
            let mut last_group = "";
            let mut any_redundant = false;
            for (group, name, at) in &parameters {
                let head = if *group == last_group {
                    Cell::Empty
                } else {
                    text(*group)
                };
                last_group = group;
                match at {
                    Some(i) => {
                        let b = fit.b[*i];
                        let se = covariance[*i][*i].sqrt();
                        let ok = covariance[*i][*i] > 0.0;
                        let wald = (b / se).powi(2);
                        out.push(vec![
                            head,
                            text(name.clone()),
                            Cell::Number(b),
                            number(ok.then_some(se)),
                            number(ok.then_some(wald)),
                            Cell::Number(1.0),
                            number(ok.then(|| chi_square_p(wald, 1.0))),
                            number(ok.then_some(b - z * se)),
                            number(ok.then_some(b + z * se)),
                        ]);
                    }
                    None => {
                        any_redundant = true;
                        let mut row = vec![
                            head,
                            text(name.clone()),
                            Cell::Number(0.0),
                            Cell::Empty,
                            Cell::Empty,
                            Cell::Number(0.0),
                        ];
                        row.extend([Cell::Empty, Cell::Empty, Cell::Empty]);
                        out.push(row);
                    }
                }
            }
            let mut caption = link_caption.clone();
            if any_redundant {
                caption.push_str(" A parameter of 0 with 0 df is redundant and fixed at zero.");
            }
            tables.push(table("Parameter Estimates", 2, columns, out, Some(caption)));
        }
        let estimated_names: Vec<String> = parameters
            .iter()
            .filter(|(_, _, at)| at.is_some())
            .map(|(g, n, _)| format!("{g} {n}"))
            .collect();
        if print.covb || print.corb {
            let matrix = |title: &str, value: &dyn Fn(usize, usize) -> Option<f64>| {
                let mut columns = plain(&[""]);
                columns.extend(plain(
                    &estimated_names
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                ));
                let rows = (0..size)
                    .map(|i| {
                        let mut row = vec![text(estimated_names[i].clone())];
                        row.extend((0..size).map(|j| number(value(i, j))));
                        row
                    })
                    .collect();
                table(title, 1, columns, rows, Some(link_caption.clone()))
            };
            if print.covb {
                tables.push(matrix("Parameter Covariance", &|i, j| {
                    Some(covariance[i][j])
                }));
            }
            if print.corb {
                tables.push(matrix("Parameter Correlation", &|i, j| {
                    let v = covariance[i][i] * covariance[j][j];
                    (v > 0.0).then(|| covariance[i][j] / v.sqrt())
                }));
            }
        }
        for (n, t) in command.tests.iter().enumerate() {
            // every parameter in order: thresholds, location, scale, with redundant ones
            let blocks: [(usize, &[Effect]); 2] = [
                (k, &location),
                (k + x_redundant.len(), if has_scale { &scale } else { &[] }),
            ];
            let full = parameters.len();
            let mut rows = Vec::new();
            for row in &t.rows {
                let mut l = vec![0.0; full];
                for (effect, values) in row {
                    let range = match effect {
                        None => 0..full,
                        Some(term) if term.variables.is_empty() => 0..k,
                        Some(term) => {
                            let mut found = Vec::new();
                            for (offset, effects) in blocks {
                                if let Some(e) = find_effect(&d, effects, term)? {
                                    found.push(offset + e.start..offset + e.start + e.cells.len());
                                }
                            }
                            match found.len() {
                                1 => found.remove(0),
                                0 => {
                                    return Err(Error(format!(
                                        "TEST names {}, which is not a LOCATION or SCALE effect",
                                        term.label
                                    )));
                                }
                                _ => {
                                    return Err(Error(format!(
                                        "TEST names {}, which is both a LOCATION and a SCALE effect; give the coefficients of every row with ALL",
                                        term.label
                                    )));
                                }
                            }
                        }
                    };
                    if values.len() != range.len() {
                        return Err(Error(format!(
                            "TEST gives {} coefficients where {} parameters are expected",
                            values.len(),
                            range.len()
                        )));
                    }
                    for (j, v) in range.zip(values) {
                        l[j] += v;
                    }
                }
                rows.push(l);
            }
            let at: Vec<Option<usize>> = parameters.iter().map(|p| p.2).collect();
            tables.extend(wald_tests(
                t,
                n + 1,
                &rows,
                &at,
                &fit.b,
                &covariance,
                criteria.cin,
                &link_caption,
                &mut warnings,
            ));
        }
        if print.cellinfo {
            tables.push(cell_table(
                &d,
                &factor_vars,
                &cov_vars,
                &shown,
                &dependent,
                &dep_levels,
                &first_row,
                &observed,
                &|i| model.cumulative(&fit.b, i),
            ));
        }
        if print.tparallel {
            if has_scale {
                warnings.push("The test of parallel lines is made for a location-only model; it is left out with SCALE".into());
            } else if k < 2 || model.px() == 0 {
                warnings.push("The test of parallel lines needs at least three categories and a location parameter; it is left out".into());
            } else {
                let general_model = Model {
                    parallel: false,
                    ..location_model
                };
                let mut start = fit.b[..k].to_vec();
                for _ in 0..k {
                    start.extend_from_slice(&fit.b[k..]);
                }
                match maximize(&general_model, start, &criteria) {
                    Some(general) => {
                        if !general.converged {
                            warnings.push("The general model of the parallel lines test did not converge; its log-likelihood may not be final".into());
                        }
                        let chi = 2.0 * (general.eval.l - ll_final);
                        let df = ((k - 1) * model.px()) as f64;
                        tables.push(table(
                            "Test of Parallel Lines",
                            1,
                            plain(&["Model", "-2 Log Likelihood", "Chi-Square", "df", "Sig."]),
                            vec![
                                vec![
                                    text("Null Hypothesis"),
                                    Cell::Number(-2.0 * (ll_final + constant)),
                                    Cell::Empty,
                                    Cell::Empty,
                                    Cell::Empty,
                                ],
                                vec![
                                    text("General"),
                                    Cell::Number(-2.0 * (general.eval.l + constant)),
                                    Cell::Number(chi),
                                    Cell::Number(df),
                                    number(Some(chi_square_p(chi.max(0.0), df))),
                                ],
                            ],
                            Some(format!(
                                "The null hypothesis is that the location parameters are the same for every category of the dependent variable. {link_caption}"
                            )),
                        ));
                    }
                    None => warnings.push(
                        "The general model of the parallel lines test could not be fitted; the test is left out".into(),
                    ),
                }
            }
        }
        // predictions for SAVE: every case with valid predictors whose factor levels were fitted
        if !command.save.is_empty() {
            let x_rows: Vec<Option<(Vec<f64>, Vec<f64>)>> = (0..cases)
                .map(|kc| {
                    if !predictors_valid[kc] {
                        return None;
                    }
                    let mut lv = Vec::new();
                    for (col, ls) in factor_keys.iter().zip(&levels) {
                        lv.push(level_of(ls, col[kc].as_ref()?)?);
                    }
                    let cv: Vec<f64> = cov_vars
                        .iter()
                        .map(|v| covariate(v, kc))
                        .collect::<Option<_>>()?;
                    let one = Data {
                        level: vec![lv],
                        cov: vec![cv],
                        y: vec![Vec::new()],
                        f: vec![1.0],
                        g: vec![1.0],
                        case: vec![kc],
                        factors: Vec::new(),
                        factor_texts: Vec::new(),
                        values: Vec::new(),
                        labels: Vec::new(),
                        labelled: false,
                        covariates: Vec::new(),
                    };
                    let row = |effects: &[Effect], redundant: &[bool]| -> Option<Vec<f64>> {
                        let mut out = vec![0.0; redundant.len()];
                        for e in effects {
                            let mine: Vec<usize> =
                                e.factors.iter().map(|&f| one.level[0][f]).collect();
                            let at = e.cells.iter().position(|c| *c == mine)?;
                            out[e.start + at] =
                                e.covariates.iter().map(|&c| one.cov[0][c]).product();
                        }
                        Some(keep(&out, redundant))
                    };
                    Some((row(&location, &x_redundant)?, row(&scale, &z_redundant)?))
                })
                .collect();
            let predictions: Vec<Option<Prediction>> = (0..cases)
                .map(|kc| {
                    let (xr, zr) = x_rows[kc].as_ref()?;
                    let one_counts = vec![vec![0.0; jn]];
                    let single = Model {
                        counts: &one_counts,
                        x: std::slice::from_ref(xr),
                        z: std::slice::from_ref(zr),
                        ..*model
                    };
                    let probabilities = cell_probabilities(&single.cumulative(&fit.b, 0));
                    let predicted = probabilities.iter().enumerate().fold(0, |best, (c, &p)| {
                        if p > probabilities[best] { c } else { best }
                    });
                    let actual = dep_keys[kc]
                        .as_ref()
                        .and_then(|key| level_of(&dep_levels, key));
                    Some(Prediction {
                        probabilities,
                        predicted,
                        actual,
                    })
                })
                .collect();
            let category_texts: Vec<String> = dep_levels
                .iter()
                .map(|k| labels.value(&dependent, k))
                .collect();
            let format = dependent.format().to_string();
            for w in warnings.drain(..) {
                self.warn(w);
            }
            self.save_plum(command, &category_texts, &format, &dep_levels, &predictions)?;
        }
        for w in warnings {
            self.warn(w);
        }
        Ok(tables)
    }

    fn save_plum(
        &mut self,
        command: &Plum,
        category_texts: &[String],
        format: &str,
        dep_levels: &[Key],
        predictions: &[Option<Prediction>],
    ) -> Result<(), Error> {
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            let free = |names: &dyn Fn(usize) -> Vec<String>, data: &statscraft_data::Dataset| {
                let mut n = 1;
                while names(n)
                    .iter()
                    .any(|name| data.variable_index(name).is_some())
                {
                    n += 1;
                }
                names(n)
            };
            for item in &command.save {
                // (name, label, value of a prediction, is the category itself)
                let mut columns: Vec<SaveColumn> = Vec::new();
                match item {
                    PlumSave::Estprob { root, count } => {
                        let n = (*count).min(dep_levels.len());
                        let names = match root {
                            Some(r) => (1..=n).map(|c| format!("{r}{c}")).collect(),
                            None => {
                                free(&|m| (1..=n).map(|c| format!("EST{c}_{m}")).collect(), data)
                            }
                        };
                        for (c, name) in names.into_iter().enumerate() {
                            columns.push((
                                name,
                                format!("Estimated probability of category {}", category_texts[c]),
                                Box::new(move |p: &Prediction| Some(p.probabilities[c])),
                                false,
                            ));
                        }
                    }
                    PlumSave::Predcat(name) => columns.push((
                        name.clone()
                            .unwrap_or_else(|| free(&|m| vec![format!("PRE_{m}")], data).remove(0)),
                        "Predicted category".into(),
                        Box::new(|p: &Prediction| Some(p.predicted as f64)),
                        true,
                    )),
                    PlumSave::Pcprob(name) => columns.push((
                        name.clone()
                            .unwrap_or_else(|| free(&|m| vec![format!("PCP_{m}")], data).remove(0)),
                        "Estimated probability of the predicted category".into(),
                        Box::new(|p: &Prediction| Some(p.probabilities[p.predicted])),
                        false,
                    )),
                    PlumSave::Acprob(name) => columns.push((
                        name.clone()
                            .unwrap_or_else(|| free(&|m| vec![format!("ACP_{m}")], data).remove(0)),
                        "Estimated probability of the actual category".into(),
                        Box::new(|p: &Prediction| p.actual.map(|a| p.probabilities[a])),
                        false,
                    )),
                }
                for (name, label, value, is_category) in columns {
                    if data.variable_index(&name).is_some() {
                        return Err(Error(format!("PLUM /SAVE: variable {name} already exists")));
                    }
                    if is_category && let Some(Key::Text(_)) = dep_levels.first() {
                        let texts: Vec<String> = predictions
                            .iter()
                            .map(|p| match p {
                                Some(p) => key_text(&dep_levels[p.predicted]),
                                None => String::new(),
                            })
                            .collect();
                        let width = texts.iter().map(String::len).max().unwrap_or(1).max(1);
                        data.set_string_values(&name, width, texts)?;
                        data.set_variable_label(&name, Some(&label))?;
                        data.set_measure(&name, Measure::Ordinal)?;
                        continue;
                    }
                    let column: Vec<Option<f64>> = predictions
                        .iter()
                        .map(|p| {
                            let p = p.as_ref()?;
                            let v = value(p)?;
                            if is_category {
                                match &dep_levels[v as usize] {
                                    Key::Number(n) => Some(*n),
                                    Key::Text(_) => None,
                                }
                            } else {
                                Some(v)
                            }
                        })
                        .collect();
                    data.set_numeric_values(&name, column)?;
                    data.set_variable_label(&name, Some(&label))?;
                    if is_category {
                        data.set_format(&name, format)?;
                        data.set_measure(&name, Measure::Ordinal)?;
                    } else {
                        data.set_format(&name, "F8.2")?;
                        data.set_measure(&name, Measure::Scale)?;
                    }
                }
            }
            Ok(())
        })
    }
}

fn key_text(key: &Key) -> String {
    match key {
        Key::Number(n) => n.to_string(),
        Key::Text(t) => t.trim_end().to_string(),
    }
}

fn clone_key(key: &Key) -> Key {
    match key {
        Key::Number(n) => Key::Number(*n),
        Key::Text(t) => Key::Text(t.clone()),
    }
}

fn percent(level: f64) -> String {
    if (level - level.round()).abs() < 1e-9 {
        format!("{level:.0}%")
    } else {
        format!("{level}%")
    }
}

/// The category probabilities from the J-1 cumulative ones.
fn cell_probabilities(gamma: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(gamma.len() + 1);
    let mut lower = 0.0;
    for &g in gamma.iter().chain(std::iter::once(&1.0)) {
        out.push(g - lower);
        lower = g;
    }
    out
}

fn history_table(history: &[Iteration], every: u32, constant: f64, size: usize) -> Table {
    let mut columns = plain(&["Iteration", "Step-halving", "-2 Log Likelihood"]);
    columns.extend((1..=size).map(|i| ("Parameter".to_string(), format!("{i}"))));
    let last = history.len() - 1;
    let rows = history
        .iter()
        .enumerate()
        .filter(|(t, _)| *t == 0 || *t == last || (*t as u32).is_multiple_of(every))
        .map(|(t, it)| {
            let mut row = vec![
                Cell::Number(t as f64),
                Cell::Number(f64::from(it.halvings)),
                Cell::Number(-2.0 * (it.l + constant)),
            ];
            row.extend(it.b.iter().map(|&v| Cell::Number(v)));
            row
        })
        .collect();
    table(
        "Iteration History",
        1,
        columns,
        rows,
        Some("The parameters are in the order of the Parameter Estimates table, redundant ones left out.".into()),
    )
}

#[allow(clippy::too_many_arguments)]
fn cell_table(
    d: &Data,
    factor_vars: &[Variable],
    cov_vars: &[Variable],
    shown: &Shown,
    dependent: &Variable,
    dep_levels: &[Key],
    first_row: &[usize],
    observed: &[Vec<f64>],
    cumulative: &dyn Fn(usize) -> Vec<f64>,
) -> Table {
    let mut columns = plain(&["", ""]);
    for group in ["Frequency", "Cumulative Frequency"] {
        for leaf in ["Observed", "Expected", "Pearson Residual"] {
            columns.push((group.to_string(), leaf.to_string()));
        }
    }
    for group in ["Probability", "Cumulative Probability"] {
        for leaf in ["Observed", "Expected"] {
            columns.push((group.to_string(), leaf.to_string()));
        }
    }
    let mut rows = Vec::new();
    for (i, &r) in first_row.iter().enumerate() {
        let mut pattern: Vec<String> = factor_vars
            .iter()
            .enumerate()
            .map(|(f, v)| format!("{}={}", shown.variable(v), d.labels[f][d.level[r][f]]))
            .collect();
        pattern.extend(
            cov_vars
                .iter()
                .enumerate()
                .map(|(c, v)| format!("{}={}", shown.variable(v), shown.number(v, d.cov[r][c]))),
        );
        let label = if pattern.is_empty() {
            "All cases".to_string()
        } else {
            pattern.join(", ")
        };
        let n: f64 = observed[i].iter().sum();
        let gamma = cumulative(i);
        let probs = cell_probabilities(&gamma);
        let residual = |o: f64, p: f64| {
            let v = n * p * (1.0 - p);
            number((v > 0.0).then(|| (o - n * p) / v.sqrt()))
        };
        let mut cum_o = 0.0;
        let mut cum_p = 0.0;
        for (c, key) in dep_levels.iter().enumerate() {
            cum_o += observed[i][c];
            cum_p += probs[c];
            let cum_p = if c + 1 == dep_levels.len() {
                1.0
            } else {
                cum_p
            };
            rows.push(vec![
                if c == 0 {
                    text(label.clone())
                } else {
                    Cell::Empty
                },
                text(shown.value(dependent, key)),
                Cell::Number(observed[i][c]),
                Cell::Number(n * probs[c]),
                residual(observed[i][c], probs[c]),
                Cell::Number(cum_o),
                Cell::Number(n * cum_p),
                residual(cum_o, cum_p),
                number((n > 0.0).then(|| observed[i][c] / n)),
                Cell::Number(probs[c]),
                number((n > 0.0).then(|| cum_o / n)),
                Cell::Number(cum_p),
            ]);
        }
    }
    table(
        "Cell Information",
        2,
        columns,
        rows,
        Some(format!(
            "One block per combination of factor levels and covariate values; the rows are the categories of {}.",
            shown.variable(dependent)
        )),
    )
}
