//! LOGISTIC REGRESSION: binary logistic regression by maximum likelihood, with categorical
//! covariates, forward and backward stepwise selection and case diagnostics.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (LOGISTIC REGRESSION):
//! - The dependent variable has exactly two values among the cases used; the lower (strings:
//!   the first in alphabetical order of their first eight characters) is 0, the other 1.
//! - A case missing on the dependent or any independent variable is left out of the fit; one
//!   valid on the independent variables still gets a predicted value.
//! - Categorical variables (named on CATEGORICAL, and every string variable) with k categories
//!   become k - 1 design variables coded by their contrast; an interaction with a categorical
//!   variable is the product of the codings. A set enters and leaves as one term.
//! - The likelihood uses the case weights. Newton-Raphson starts from all coefficients 0 and
//!   stops when every coefficient changes by less than BCON, the log likelihood by less than
//!   LCON percent, or after ITERATE iterations; when every case has pi (1 - pi) below 1e-8 it
//!   stops with all predictions 0 or 1.
//! - Entry uses the score statistic; removal uses one of three: conditional, Wald or
//!   likelihood ratio. FSTEP enters the term with the smallest score significance below PIN,
//!   then removes while the largest removal significance is above POUT; BSTEP starts with every
//!   term of its list and removes, then enters. A step that would return to a model already
//!   fitted in the block ends the method.
//! - Model chi-square: against the initial model (the constant, or nothing with ORIGIN); block:
//!   against the end of the previous block; step: against the previous step.
//! - Cox and Snell R square 1 - (l(0)/l(B))^(2/W); Nagelkerke: divided by 1 - l(0)^(2/W).
//! - Hosmer-Lemeshow: the cases sorted by predicted probability, cases with equal predictions
//!   kept together, about ten groups; chi-square with g - 2 df.
//! - Leverage h = v x'(X'CVX)^-1 x with v = pi (1 - pi); deviance, studentized, logit and
//!   normalized residuals, Cook's analog z^2 h / (1 - h) and DFBETA as the manual gives them.

use crate::contrast::coding;
use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::dist::{chi_square_p, normal_quantile};
use crate::matrix::{Matrix, inverse};
use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Measure, Values, Variable};
use statscraft_syntax::{
    Contrast, Logistic, LogisticCriteria, LogisticMethodKind, LogisticTemp, Removal,
};

/// One term of the model: a variable or an interaction, with its design columns.
struct TermInfo {
    label: String,
    columns: Vec<usize>,
    categorical: bool,
}

/// The cases with valid independent variables: their design rows, dependent (NaN where missing)
/// and case weight.
struct Design {
    case: Vec<usize>,
    x: Vec<Vec<f64>>,
    y: Vec<f64>,
    c: Vec<f64>,
    names: Vec<String>,
}

impl Design {
    fn fitted(&self, i: usize) -> bool {
        !self.y[i].is_nan() && self.c[i] > 0.0
    }

    fn row(&self, i: usize, cols: &[usize], constant: bool) -> Vec<f64> {
        let mut r = Vec::with_capacity(cols.len() + 1);
        if constant {
            r.push(1.0);
        }
        r.extend(cols.iter().map(|&j| self.x[i][j]));
        r
    }

    fn eta(&self, i: usize, cols: &[usize], constant: bool, b: &[f64]) -> f64 {
        self.row(i, cols, constant)
            .iter()
            .zip(b)
            .map(|(x, b)| x * b)
            .sum()
    }

    fn log_likelihood(&self, cols: &[usize], constant: bool, b: &[f64]) -> f64 {
        (0..self.x.len())
            .filter(|&i| self.fitted(i))
            .map(|i| {
                let eta = self.eta(i, cols, constant, b);
                // ln pi = -ln(1 + e^-eta), written to keep its precision far out
                let ln_p = -softplus(-eta);
                let ln_q = -softplus(eta);
                self.c[i] * (self.y[i] * ln_p + (1.0 - self.y[i]) * ln_q)
            })
            .sum()
    }

    /// The sum of the case weights of the cases in the fit.
    fn total(&self) -> f64 {
        (0..self.x.len())
            .filter(|&i| self.fitted(i))
            .map(|i| self.c[i])
            .sum()
    }
}

fn softplus(x: f64) -> f64 {
    if x > 0.0 {
        x + (-x).exp().ln_1p()
    } else {
        x.exp().ln_1p()
    }
}

fn logistic(eta: f64) -> f64 {
    1.0 / (1.0 + (-eta).exp())
}

struct Fit {
    cols: Vec<usize>,
    constant: bool,
    b: Vec<f64>,
    cov: Matrix,
    ll: f64,
    /// -2 log likelihood and the coefficients after each iteration.
    history: Vec<(f64, Vec<f64>)>,
    note: String,
}

impl Fit {
    fn pi(&self, design: &Design, i: usize) -> f64 {
        logistic(design.eta(i, &self.cols, self.constant, &self.b))
    }

    fn parameters(&self) -> usize {
        self.b.len()
    }

    /// The position of design column `col` in the coefficients.
    fn at(&self, col: usize) -> Option<usize> {
        self.cols
            .iter()
            .position(|&c| c == col)
            .map(|p| p + usize::from(self.constant))
    }
}

/// The weighted information matrix X'CVX and score X'C(y - pi) at `b`.
fn information(design: &Design, cols: &[usize], constant: bool, b: &[f64]) -> (Matrix, Vec<f64>) {
    let p = cols.len() + usize::from(constant);
    let mut info = vec![vec![0.0; p]; p];
    let mut score = vec![0.0; p];
    for i in (0..design.x.len()).filter(|&i| design.fitted(i)) {
        let x = design.row(i, cols, constant);
        let pi = logistic(x.iter().zip(b).map(|(x, b)| x * b).sum());
        let v = design.c[i] * pi * (1.0 - pi);
        for r in 0..p {
            score[r] += design.c[i] * x[r] * (design.y[i] - pi);
            for s in 0..p {
                info[r][s] += v * x[r] * x[s];
            }
        }
    }
    (info, score)
}

fn fit(design: &Design, cols: &[usize], constant: bool, criteria: &LogisticCriteria) -> Fit {
    let p = cols.len() + usize::from(constant);
    let mut b = vec![0.0; p];
    let mut ll = design.log_likelihood(cols, constant, &b);
    let mut history = Vec::new();
    let mut note = String::new();
    for iteration in 1..=criteria.iterate {
        let (info, score) = information(design, cols, constant, &b);
        let Some((inv, _)) = inverse(&info) else {
            note = format!(
                "Estimation terminated at iteration number {iteration} because the information matrix is singular."
            );
            break;
        };
        let delta: Vec<f64> = (0..p)
            .map(|r| (0..p).map(|s| inv[r][s] * score[s]).sum())
            .collect();
        for (b, d) in b.iter_mut().zip(&delta) {
            *b += d;
        }
        let previous = ll;
        ll = design.log_likelihood(cols, constant, &b);
        history.push((-2.0 * ll, b.clone()));
        let all_certain = (0..design.x.len()).filter(|&i| design.fitted(i)).all(|i| {
            let pi = logistic(design.eta(i, cols, constant, &b));
            pi * (1.0 - pi) < 1e-8
        });
        if all_certain {
            note = format!(
                "Estimation terminated at iteration number {iteration} because all predicted values are either 1 or 0."
            );
            break;
        }
        if delta.iter().all(|d| d.abs() < criteria.bcon) {
            note = format!(
                "Estimation terminated at iteration number {iteration} because parameter estimates changed by less than {}.",
                criteria.bcon
            );
            break;
        }
        if criteria.lcon > 0.0 && ((ll - previous) / previous).abs() * 100.0 < criteria.lcon {
            note = format!(
                "Estimation terminated at iteration number {iteration} because the log likelihood decreased by less than {} percent.",
                criteria.lcon
            );
            break;
        }
        if iteration == criteria.iterate {
            note = format!(
                "Estimation terminated at iteration number {iteration} because maximum iterations has been reached. Final solution cannot be found."
            );
        }
    }
    let (info, _) = information(design, cols, constant, &b);
    let cov = inverse(&info).map_or_else(|| vec![vec![f64::NAN; p]; p], |(inv, _)| inv);
    Fit {
        cols: cols.to_vec(),
        constant,
        b,
        cov,
        ll,
        history,
        note,
    }
}

/// The score statistic of the columns `extra` at the estimates of `fit`.
fn score_test(design: &Design, fit: &Fit, extra: &[usize]) -> f64 {
    let m = fit.parameters();
    let q = extra.len();
    let (mut a11, mut a12, mut a22) = (
        vec![vec![0.0; m]; m],
        vec![vec![0.0; q]; m],
        vec![vec![0.0; q]; q],
    );
    let mut l = vec![0.0; q];
    for i in (0..design.x.len()).filter(|&i| design.fitted(i)) {
        let x = design.row(i, &fit.cols, fit.constant);
        let g: Vec<f64> = extra.iter().map(|&j| design.x[i][j]).collect();
        let pi = fit.pi(design, i);
        let v = design.c[i] * pi * (1.0 - pi);
        for r in 0..q {
            l[r] += design.c[i] * g[r] * (design.y[i] - pi);
            for s in 0..q {
                a22[r][s] += v * g[r] * g[s];
            }
            for s in 0..m {
                a12[s][r] += v * x[s] * g[r];
            }
        }
        for r in 0..m {
            for s in 0..m {
                a11[r][s] += v * x[r] * x[s];
            }
        }
    }
    let mut b = a22;
    if m > 0 {
        let Some((inv11, _)) = inverse(&a11) else {
            return 0.0;
        };
        for r in 0..q {
            for s in 0..q {
                let mut sum = 0.0;
                for i in 0..m {
                    for j in 0..m {
                        sum += a12[i][r] * inv11[i][j] * a12[j][s];
                    }
                }
                b[r][s] -= sum;
            }
        }
    }
    let Some((b22, det)) = inverse(&b) else {
        return 0.0;
    };
    if det <= 0.0 {
        return 0.0;
    }
    let s: f64 = (0..q)
        .map(|r| (0..q).map(|t| l[r] * b22[r][t] * l[t]).sum::<f64>())
        .sum();
    s.max(0.0)
}

/// The Wald statistic of the coefficients at `positions`.
fn wald(fit: &Fit, positions: &[usize]) -> f64 {
    let sub: Matrix = positions
        .iter()
        .map(|&r| positions.iter().map(|&s| fit.cov[r][s]).collect())
        .collect();
    let beta: Vec<f64> = positions.iter().map(|&r| fit.b[r]).collect();
    match inverse(&sub) {
        Some((inv, _)) => (0..beta.len())
            .map(|r| {
                (0..beta.len())
                    .map(|s| beta[r] * inv[r][s] * beta[s])
                    .sum::<f64>()
            })
            .sum(),
        None => f64::NAN,
    }
}

/// The columns of `wanted` that are not linear combinations of the ones before them (and of
/// the constant): a column whose share not explained by them, relative to its own sum of
/// squares, is below `eps` is redundant.
fn independent(design: &Design, wanted: &[usize], constant: bool, eps: f64) -> Vec<usize> {
    let p = wanted.len();
    let rows: Vec<usize> = (0..design.x.len()).filter(|&i| design.fitted(i)).collect();
    let w: f64 = rows.iter().map(|&i| design.c[i]).sum();
    let mean: Vec<f64> = wanted
        .iter()
        .map(|&j| {
            if constant {
                rows.iter()
                    .map(|&i| design.c[i] * design.x[i][j])
                    .sum::<f64>()
                    / w
            } else {
                0.0
            }
        })
        .collect();
    let mut m = vec![vec![0.0; p]; p];
    for &i in &rows {
        for r in 0..p {
            for s in 0..p {
                m[r][s] += design.c[i]
                    * (design.x[i][wanted[r]] - mean[r])
                    * (design.x[i][wanted[s]] - mean[s]);
            }
        }
    }
    let diag: Vec<f64> = (0..p).map(|r| m[r][r]).collect();
    let mut kept = Vec::new();
    for k in 0..p {
        if m[k][k] <= eps * diag[k] || m[k][k] <= 0.0 {
            continue;
        }
        crate::matrix::sweep(&mut m, k);
        kept.push(wanted[k]);
    }
    kept
}

/// One step of a block: the model after it and what it changed.
struct Step {
    model: Vec<usize>,
    fit: Fit,
    entered: Vec<usize>,
    removed: Vec<usize>,
}

/// The removal statistic of term `t` in `fit`: the statistic, its df and the log likelihood of
/// the model without it (for LR and COND).
fn removal(
    design: &Design,
    fit: &Fit,
    terms: &[TermInfo],
    t: usize,
    kind: Removal,
    criteria: &LogisticCriteria,
) -> (f64, usize, f64) {
    let positions: Vec<usize> = terms[t].columns.iter().filter_map(|&c| fit.at(c)).collect();
    let rest: Vec<usize> = fit
        .cols
        .iter()
        .copied()
        .filter(|c| !terms[t].columns.contains(c))
        .collect();
    let df = positions.len();
    match kind {
        Removal::Wald => (wald(fit, &positions), df, f64::NAN),
        Removal::Lr => {
            let reduced = self::fit(design, &rest, fit.constant, criteria);
            (2.0 * (fit.ll - reduced.ll), df, reduced.ll)
        }
        Removal::Cond => {
            let others: Vec<usize> = (0..fit.parameters())
                .filter(|p| !positions.contains(p))
                .collect();
            let c22: Matrix = positions
                .iter()
                .map(|&r| positions.iter().map(|&s| fit.cov[r][s]).collect())
                .collect();
            let Some((inv, _)) = inverse(&c22) else {
                return (f64::NAN, df, f64::NAN);
            };
            let beta: Vec<f64> = positions.iter().map(|&r| fit.b[r]).collect();
            let solved: Vec<f64> = (0..df)
                .map(|r| (0..df).map(|s| inv[r][s] * beta[s]).sum())
                .collect();
            let b: Vec<f64> = others
                .iter()
                .map(|&o| {
                    fit.b[o]
                        - positions
                            .iter()
                            .zip(&solved)
                            .map(|(&p, v)| fit.cov[o][p] * v)
                            .sum::<f64>()
                })
                .collect();
            let ll = design.log_likelihood(&rest, fit.constant, &b);
            (2.0 * (fit.ll - ll), df, ll)
        }
    }
}

fn method_name(kind: LogisticMethodKind) -> String {
    let removal = |r: Removal| match r {
        Removal::Cond => "Conditional",
        Removal::Lr => "Likelihood Ratio",
        Removal::Wald => "Wald",
    };
    match kind {
        LogisticMethodKind::Enter => "Enter".into(),
        LogisticMethodKind::Fstep(r) => format!("Forward Stepwise ({})", removal(r)),
        LogisticMethodKind::Bstep(r) => format!("Backward Stepwise ({})", removal(r)),
    }
}

fn same_term(a: &[String], b: &[String]) -> bool {
    let norm = |t: &[String]| {
        let mut v: Vec<String> = t.iter().map(|s| s.to_lowercase()).collect();
        v.sort();
        v
    };
    norm(a) == norm(b)
}

/// The value of a numeric or string variable at a case, as a category key.
fn key_text(key: &Key) -> String {
    match key {
        Key::Number(n) => n.to_string(),
        Key::Text(t) => t.clone(),
    }
}

/// What every case gets from the final model.
struct CaseStats {
    pred: f64,
    pgroup: f64,
    resid: f64,
    dev: f64,
    lresid: f64,
    sresid: f64,
    zresid: f64,
    lever: f64,
    cook: f64,
    dfbeta: Vec<f64>,
}

fn case_stats(design: &Design, fit: &Fit, i: usize, cut: f64) -> CaseStats {
    let pi = fit.pi(design, i);
    let pgroup = f64::from(u8::from(pi >= cut));
    let nan = f64::NAN;
    if !design.fitted(i) {
        return CaseStats {
            pred: pi,
            pgroup,
            resid: nan,
            dev: nan,
            lresid: nan,
            sresid: nan,
            zresid: nan,
            lever: nan,
            cook: nan,
            dfbeta: vec![nan; fit.parameters()],
        };
    }
    let y = design.y[i];
    let x = design.row(i, &fit.cols, fit.constant);
    let p = x.len();
    let v = pi * (1.0 - pi);
    let gx: Vec<f64> = (0..p)
        .map(|r| (0..p).map(|s| fit.cov[r][s] * x[s]).sum())
        .collect();
    let h = v * x.iter().zip(&gx).map(|(a, b)| a * b).sum::<f64>();
    let e = y - pi;
    let ll = if y > 0.5 { pi.ln() } else { (1.0 - pi).ln() };
    let dev = (-2.0 * ll).sqrt() * if y > pi { 1.0 } else { -1.0 };
    let z = e / v.sqrt();
    CaseStats {
        pred: pi,
        pgroup,
        resid: e,
        dev,
        lresid: e / v,
        sresid: dev / (1.0 - h).sqrt(),
        zresid: z,
        lever: h,
        // the Algorithms manual: z^2 h / (1 - h)
        cook: z * z * h / (1.0 - h),
        dfbeta: gx.iter().map(|g| g * e / (1.0 - h)).collect(),
    }
}

/// Hosmer-Lemeshow groups: (total, observed events, expected events) of each.
fn hosmer_lemeshow(design: &Design, fit: &Fit) -> Vec<(f64, f64, f64)> {
    let mut cases: Vec<(f64, usize)> = (0..design.x.len())
        .filter(|&i| design.fitted(i))
        .map(|i| (fit.pi(design, i), i))
        .collect();
    cases.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: f64 = cases.iter().map(|&(_, i)| design.c[i]).sum();
    let mut groups: Vec<(f64, f64, f64)> = Vec::new();
    let mut current = (0.0, 0.0, 0.0);
    let mut cumulative = 0.0;
    let mut k = 0;
    while k < cases.len() {
        // a block: the cases with the same predicted probability
        let start = k;
        while k < cases.len() && (cases[k].0 - cases[start].0).abs() <= 1e-10 {
            let (pi, i) = cases[k];
            current.0 += design.c[i];
            current.1 += design.c[i] * design.y[i];
            current.2 += design.c[i] * pi;
            cumulative += design.c[i];
            k += 1;
        }
        // shortcut: a group closes once the cumulative count reaches its tenth of the total;
        // refine if a reference grouping is found to differ
        let boundary = total * (groups.len() + 1) as f64 / 10.0;
        if cumulative >= boundary - 1e-9 * total || k == cases.len() {
            groups.push(current);
            current = (0.0, 0.0, 0.0);
        }
    }
    groups
}

impl Engine {
    pub(crate) fn logistic(&mut self, command: &Logistic) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let shown = self.shown();
        let criteria = command.criteria;
        let mut warnings = Vec::new();
        let dependent = data.variable(&command.dependent)?;
        // the terms: WITH first, then those named only on a METHOD
        let mut term_vars: Vec<Vec<String>> = Vec::new();
        for term in command
            .terms
            .iter()
            .chain(command.methods.iter().flat_map(|m| m.terms.iter()))
        {
            if !term_vars.iter().any(|t| same_term(t, term)) {
                term_vars.push(term.clone());
            }
        }
        let mut variables: Vec<Variable> = Vec::new();
        for name in term_vars.iter().flatten() {
            let v = data.variable(name)?;
            if v.name().eq_ignore_ascii_case(dependent.name()) {
                return Err(Error(format!(
                    "{} is the dependent variable and cannot be an independent one",
                    v.name()
                )));
            }
            if !variables.iter().any(|w| w.name() == v.name()) {
                variables.push(v);
            }
        }
        let is_categorical = |v: &Variable| {
            v.is_string()
                || command
                    .categorical
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case(v.name()))
        };
        for name in &command.categorical {
            if !variables
                .iter()
                .any(|v| v.name().eq_ignore_ascii_case(name))
            {
                warnings.push(format!(
                    "{name} is on CATEGORICAL but in no term of the model; it is ignored"
                ));
            }
        }
        for (name, _) in &command.contrasts {
            if !variables
                .iter()
                .any(|v| v.name().eq_ignore_ascii_case(name) && is_categorical(v))
            {
                return Err(Error(format!(
                    "CONTRAST names {name}, which is not a categorical variable of the model"
                )));
            }
        }
        // the cases: valid on every independent variable, with a positive weight
        let cases = data.case_count();
        let weights = case_weights(data)?;
        let weight = |k: usize| weights.as_ref().map_or(1.0, |w| w[k]);
        let keys: Vec<Vec<Option<Key>>> = variables
            .iter()
            .map(|v| categories(v, command.include))
            .collect();
        let dep_keys = categories(&dependent, command.include);
        let dep_key = |k: usize| -> Option<Key> {
            dep_keys[k].as_ref().map(|key| match key {
                Key::Text(t) => Key::Text(t.trim_end().chars().take(8).collect()),
                other => clone_key(other),
            })
        };
        let predictors_valid: Vec<bool> = (0..cases)
            .map(|k| keys.iter().all(|col| col[k].is_some()))
            .collect();
        let in_fit: Vec<bool> = (0..cases)
            .map(|k| predictors_valid[k] && dep_keys[k].is_some() && weight(k) > 0.0)
            .collect();
        // the two values of the dependent
        let mut values: Vec<Key> = Vec::new();
        for k in (0..cases).filter(|&k| in_fit[k]) {
            let key = dep_key(k).expect("valid");
            if !values.iter().any(|v| v.compare(&key).is_eq()) {
                values.push(key);
            }
        }
        values.sort_by(|a, b| a.compare(b));
        if values.len() != 2 {
            return Err(Error(format!(
                "LOGISTIC REGRESSION needs a dependent variable with exactly two values among the cases used; {} has {}",
                dependent.name(),
                values.len()
            )));
        }
        // the categories of each categorical variable and their codings
        let mut codings: Vec<Option<(Vec<Key>, Matrix)>> = Vec::new();
        for (v, col) in variables.iter().zip(&keys) {
            if !is_categorical(v) {
                codings.push(None);
                continue;
            }
            let mut levels: Vec<Key> = Vec::new();
            for k in (0..cases).filter(|&k| in_fit[k]) {
                let key = col[k].as_ref().expect("valid");
                if !levels.iter().any(|l| l.compare(key).is_eq()) {
                    levels.push(clone_key(key));
                }
            }
            levels.sort_by(|a, b| a.compare(b));
            if levels.len() < 2 {
                return Err(Error(format!(
                    "the categorical variable {} has fewer than two categories among the cases used",
                    v.name()
                )));
            }
            let kind = command
                .contrasts
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case(v.name()))
                .map_or(Contrast::Indicator(None), |(_, c)| c.clone());
            let matrix = coding(&kind, levels.len())
                .map_err(|e| Error(format!("CONTRAST({}): {e}", v.name())))?;
            codings.push(Some((levels, matrix)));
        }
        let var_index = |name: &str| {
            variables
                .iter()
                .position(|v| v.name().eq_ignore_ascii_case(name))
                .expect("collected")
        };
        // the design columns of every term
        let mut names: Vec<String> = Vec::new();
        let mut terms: Vec<TermInfo> = Vec::new();
        // per column: for each variable of its term, which coding column (None: the value)
        let mut recipes: Vec<Vec<(usize, Option<usize>)>> = Vec::new();
        for term in &term_vars {
            let parts: Vec<usize> = term.iter().map(|n| var_index(n)).collect();
            let mut combos: Vec<Vec<(usize, Option<usize>)>> = vec![Vec::new()];
            for &v in &parts {
                let choices: Vec<Option<usize>> = match &codings[v] {
                    Some((_, m)) => (0..m[0].len()).map(Some).collect(),
                    None => vec![None],
                };
                combos = combos
                    .into_iter()
                    .flat_map(|prefix| {
                        choices.iter().map(move |&c| {
                            let mut next = prefix.clone();
                            next.push((v, c));
                            next
                        })
                    })
                    .collect();
            }
            let mut columns = Vec::new();
            for combo in combos {
                let name = combo
                    .iter()
                    .map(|&(v, c)| match c {
                        Some(c) => format!("{}({})", shown.variable(&variables[v]), c + 1),
                        None => shown.variable(&variables[v]),
                    })
                    .collect::<Vec<_>>()
                    .join(" by ");
                columns.push(names.len());
                names.push(name);
                recipes.push(combo);
            }
            let categorical = parts.iter().any(|&v| codings[v].is_some());
            terms.push(TermInfo {
                label: parts
                    .iter()
                    .map(|&v| shown.variable(&variables[v]))
                    .collect::<Vec<_>>()
                    .join(" by "),
                columns,
                categorical,
            });
        }
        let numeric = |v: &Variable, k: usize| match v.values() {
            Values::Numeric(values) => values.get(k).unwrap_or(f64::NAN),
            Values::String(_) => f64::NAN,
        };
        let mut design = Design {
            case: Vec::new(),
            x: Vec::new(),
            y: Vec::new(),
            c: Vec::new(),
            names: names.clone(),
        };
        for k in (0..cases).filter(|&k| predictors_valid[k] && weight(k) > 0.0) {
            let row: Vec<f64> = recipes
                .iter()
                .map(|recipe| {
                    recipe
                        .iter()
                        .map(|&(v, c)| match (c, &codings[v]) {
                            (Some(c), Some((levels, m))) => {
                                let key = keys[v][k].as_ref().expect("valid");
                                let at = levels
                                    .iter()
                                    .position(|l| l.compare(key).is_eq())
                                    .expect("a category of the analysis");
                                m[at][c]
                            }
                            _ => numeric(&variables[v], k),
                        })
                        .product()
                })
                .collect();
            design.case.push(k);
            design.x.push(row);
            design.y.push(if in_fit[k] {
                let key = dep_key(k).expect("valid");
                f64::from(u8::from(values[1].compare(&key).is_eq()))
            } else {
                f64::NAN
            });
            design.c.push(weight(k));
        }
        let w_total = design.total();
        let constant = !command.origin;
        let dep_label = |at: usize| {
            let original = match &values[at] {
                Key::Text(t) => Key::Text(t.clone()),
                Key::Number(n) => Key::Number(*n),
            };
            shown.value(&dependent, &original)
        };
        let mut tables = Vec::new();

        // Case Processing Summary, unweighted
        let included = in_fit.iter().filter(|&&b| b).count() as f64;
        let total = cases as f64;
        let percent = |n: f64| number(Some(if total > 0.0 { 100.0 * n / total } else { 0.0 }));
        tables.push(table(
            "Case Processing Summary",
            2,
            plain(&["Unweighted Cases", "", "N", "Percent"]),
            vec![
                vec![
                    text("Selected Cases"),
                    text("Included in Analysis"),
                    Cell::Number(included),
                    percent(included),
                ],
                vec![
                    Cell::Empty,
                    text("Missing Cases"),
                    Cell::Number(total - included),
                    percent(total - included),
                ],
                vec![
                    Cell::Empty,
                    text("Total"),
                    Cell::Number(total),
                    percent(total),
                ],
                vec![
                    text("Unselected Cases"),
                    Cell::Empty,
                    Cell::Number(0.0),
                    percent(0.0),
                ],
                vec![text("Total"), Cell::Empty, Cell::Number(total), percent(total)],
            ],
            weights.as_ref().map(|_| {
                "If weight is in effect, see the classification table for the total number of cases."
                    .to_string()
            }),
        ));
        tables.push(table(
            "Dependent Variable Encoding",
            1,
            plain(&["Original Value", "Internal Value"]),
            (0..2)
                .map(|i| vec![text(dep_label(i)), Cell::Number(i as f64)])
                .collect(),
            None,
        ));
        if codings.iter().any(Option::is_some) {
            let width = codings
                .iter()
                .flatten()
                .map(|(_, m)| m[0].len())
                .max()
                .unwrap_or(0);
            let mut columns = plain(&["", "", "Frequency"]);
            columns.extend((1..=width).map(|j| ("Parameter coding".to_string(), format!("({j})"))));
            let mut rows = Vec::new();
            for (v, coded) in variables.iter().zip(&codings) {
                let Some((levels, m)) = coded else { continue };
                for (at, level) in levels.iter().enumerate() {
                    let frequency: f64 = (0..design.x.len())
                        .filter(|&i| design.fitted(i))
                        .filter(|&i| {
                            keys[var_index(v.name())][design.case[i]]
                                .as_ref()
                                .is_some_and(|k| k.compare(level).is_eq())
                        })
                        .map(|i| design.c[i])
                        .sum();
                    let mut row = vec![
                        if at == 0 {
                            text(shown.variable(v))
                        } else {
                            Cell::Empty
                        },
                        text(shown.value(v, level)),
                        Cell::Number(frequency),
                    ];
                    row.extend((0..width).map(|j| number(m[at].get(j).copied())));
                    rows.push(row);
                }
            }
            tables.push(table(
                "Categorical Variables Codings",
                2,
                columns,
                rows,
                None,
            ));
        }

        // Block 0
        let initial = fit(&design, &[], constant, &criteria);
        let first_terms: Vec<usize> = method_terms(command, 0, &term_vars);
        let caption0 = "Block 0: Beginning Block.".to_string();
        let step0 = Step {
            model: Vec::new(),
            fit: initial,
            entered: Vec::new(),
            removed: Vec::new(),
        };
        let context = Context {
            design: &design,
            terms: &terms,
            command,
            dep_label: &dep_label,
            initial_ll: step0.fit.ll,
        };
        if constant && let Some(n) = command.print.iter {
            tables.push(context.iteration_history(&[&step0], n, true, &caption0));
        }
        tables.push(context.classification(&[(0, &step0)], &caption0));
        if constant {
            tables.push(context.in_equation(&[(0, &step0)], &caption0));
        }
        if let Some(t) = context.not_in_equation(&[(0, &step0)], &first_terms, &caption0) {
            tables.push(t);
        }

        // the blocks
        let mut model: Vec<usize> = Vec::new();
        let mut last_fit_ll = step0.fit.ll;
        let mut last: Step = step0;
        let mut seen_columns: Vec<usize> = Vec::new();
        for (b, method) in command.methods.iter().enumerate() {
            let eligible = method_terms(command, b, &term_vars);
            let caption = format!("Block {}: Method = {}.", b + 1, method_name(method.kind));
            let fit_model = |model: &[usize], warnings: &mut Vec<String>, seen: &mut Vec<usize>| {
                let mut wanted: Vec<usize> = model
                    .iter()
                    .flat_map(|&t| terms[t].columns.iter().copied())
                    .collect();
                wanted.sort_unstable();
                let kept = independent(&design, &wanted, constant, criteria.eps);
                for c in wanted.iter().filter(|c| !kept.contains(c)) {
                    if !seen.contains(c) {
                        seen.push(*c);
                        warnings.push(format!(
                            "{} is a linear combination of the variables before it in the model and is left out",
                            names[*c]
                        ));
                    }
                }
                fit(&design, &kept, constant, &criteria)
            };
            let mut steps: Vec<Step> = Vec::new();
            let mut tried: Vec<Vec<usize>> = vec![sorted(&model)];
            let stat = match method.kind {
                LogisticMethodKind::Enter => None,
                LogisticMethodKind::Fstep(r) | LogisticMethodKind::Bstep(r) => Some(r),
            };
            match method.kind {
                LogisticMethodKind::Enter => {
                    let added: Vec<usize> = eligible
                        .iter()
                        .copied()
                        .filter(|t| !model.contains(t))
                        .collect();
                    model.extend(&added);
                    model.sort_unstable();
                    let fitted = fit_model(&model, &mut warnings, &mut seen_columns);
                    steps.push(Step {
                        model: model.clone(),
                        fit: fitted,
                        entered: added,
                        removed: Vec::new(),
                    });
                }
                LogisticMethodKind::Fstep(_) | LogisticMethodKind::Bstep(_) => {
                    let kind = stat.expect("stepwise");
                    let mut forward = matches!(method.kind, LogisticMethodKind::Fstep(_));
                    if !forward {
                        let added: Vec<usize> = eligible
                            .iter()
                            .copied()
                            .filter(|t| !model.contains(t))
                            .collect();
                        model.extend(&added);
                        model.sort_unstable();
                        tried.push(sorted(&model));
                        let fitted = fit_model(&model, &mut warnings, &mut seen_columns);
                        steps.push(Step {
                            model: model.clone(),
                            fit: fitted,
                            entered: added,
                            removed: Vec::new(),
                        });
                    }
                    for _ in 0..200 {
                        let current = steps.last().map_or(&last.fit, |s| &s.fit);
                        if !forward {
                            // removal
                            let removable: Vec<usize> = eligible
                                .iter()
                                .copied()
                                .filter(|t| model.contains(t))
                                .collect();
                            let worst = removable
                                .iter()
                                .map(|&t| {
                                    let (s, df, _) =
                                        removal(&design, current, &terms, t, kind, &criteria);
                                    (t, chi_square_p(s, df as f64))
                                })
                                .filter(|(_, p)| p.is_finite())
                                .max_by(|a, b| a.1.total_cmp(&b.1));
                            if let Some((t, p)) = worst
                                && p > criteria.pout
                            {
                                let next: Vec<usize> =
                                    model.iter().copied().filter(|&m| m != t).collect();
                                if tried.contains(&sorted(&next)) {
                                    break;
                                }
                                tried.push(sorted(&next));
                                model = next;
                                let fitted = fit_model(&model, &mut warnings, &mut seen_columns);
                                steps.push(Step {
                                    model: model.clone(),
                                    fit: fitted,
                                    entered: Vec::new(),
                                    removed: vec![t],
                                });
                                continue;
                            }
                        }
                        // entry
                        let current = steps.last().map_or(&last.fit, |s| &s.fit);
                        let best = eligible
                            .iter()
                            .copied()
                            .filter(|t| !model.contains(t))
                            .map(|t| {
                                let cols =
                                    new_columns(&design, current, &terms[t].columns, &criteria);
                                let s = score_test(&design, current, &cols);
                                (t, chi_square_p(s, cols.len() as f64))
                            })
                            .filter(|(_, p)| p.is_finite())
                            .min_by(|a, b| a.1.total_cmp(&b.1));
                        let Some((t, p)) = best else { break };
                        if p >= criteria.pin {
                            break;
                        }
                        let mut next = model.clone();
                        next.push(t);
                        next.sort_unstable();
                        if tried.contains(&next) {
                            break;
                        }
                        tried.push(next.clone());
                        model = next;
                        let fitted = fit_model(&model, &mut warnings, &mut seen_columns);
                        steps.push(Step {
                            model: model.clone(),
                            fit: fitted,
                            entered: vec![t],
                            removed: Vec::new(),
                        });
                        forward = false;
                    }
                }
            }
            if steps.is_empty() {
                warnings.push(format!(
                    "block {}: no variable entered or left the model",
                    b + 1
                ));
                continue;
            }
            let shown: Vec<(usize, &Step)> = if command.print.summary {
                vec![(steps.len(), steps.last().expect("nonempty"))]
            } else {
                steps.iter().enumerate().map(|(i, s)| (i + 1, s)).collect()
            };
            let previous_ll: Vec<f64> = std::iter::once(last.fit.ll)
                .chain(steps.iter().map(|s| s.fit.ll))
                .collect();
            let previous_p: Vec<usize> = std::iter::once(last.fit.parameters())
                .chain(steps.iter().map(|s| s.fit.parameters()))
                .collect();
            if let Some(n) = command.print.iter {
                let refs: Vec<&Step> = shown.iter().map(|(_, s)| *s).collect();
                tables.push(context.iteration_history(&refs, n, false, &caption));
            }
            // Omnibus Tests of Model Coefficients
            let p0 = usize::from(constant);
            let mut rows = Vec::new();
            for &(n, s) in &shown {
                let ll = s.fit.ll;
                let p = s.fit.parameters();
                for (i, (what, base_ll, base_p)) in [
                    ("Step", previous_ll[n - 1], previous_p[n - 1]),
                    ("Block", last_fit_ll, last.fit.parameters()),
                    ("Model", context.initial_ll, p0),
                ]
                .into_iter()
                .enumerate()
                {
                    let chi = 2.0 * (ll - base_ll);
                    let df = p as f64 - base_p as f64;
                    let df_abs = df.abs();
                    rows.push(vec![
                        if i == 0 {
                            text(format!("Step {n}"))
                        } else {
                            Cell::Empty
                        },
                        text(what),
                        number(Some(chi)),
                        number(Some(df)),
                        number((df_abs > 0.0).then(|| chi_square_p(chi.abs(), df_abs))),
                    ]);
                }
            }
            tables.push(table(
                "Omnibus Tests of Model Coefficients",
                2,
                plain(&["", "", "Chi-square", "df", "Sig."]),
                rows,
                Some(caption.clone()),
            ));
            // Model Summary
            let rows = shown
                .iter()
                .map(|&(n, s)| {
                    let ll = s.fit.ll;
                    let cs = 1.0 - (2.0 * (context.initial_ll - ll) / w_total).exp();
                    let max = 1.0 - (2.0 * context.initial_ll / w_total).exp();
                    vec![
                        text(format!("Step {n}")),
                        number(Some(-2.0 * ll)),
                        number(Some(cs)),
                        number(Some(cs / max)),
                    ]
                })
                .collect();
            let notes: Vec<String> = shown
                .iter()
                .map(|(n, s)| format!("Step {n}: {}", s.fit.note))
                .collect();
            tables.push(table(
                "Model Summary",
                1,
                plain(&[
                    "Step",
                    "-2 Log likelihood",
                    "Cox & Snell R Square",
                    "Nagelkerke R Square",
                ]),
                rows,
                Some(format!("{caption} {}", notes.join(" "))),
            ));
            if command.print.goodfit {
                tables.extend(context.hosmer_lemeshow(&shown, &caption));
            }
            tables.push(context.classification(&shown, &caption));
            tables.push(context.in_equation(&shown, &caption));
            if command.print.corr {
                tables.push(context.correlations(&shown, &caption));
            }
            if matches!(stat, Some(Removal::Lr | Removal::Cond)) {
                let kind = stat.expect("stepwise");
                let mut rows = Vec::new();
                for &(n, s) in &shown {
                    let mut first = true;
                    for &t in eligible.iter().filter(|t| s.model.contains(t)) {
                        let (chi, df, ll) = removal(&design, &s.fit, &terms, t, kind, &criteria);
                        rows.push(vec![
                            if first {
                                text(format!("Step {n}"))
                            } else {
                                Cell::Empty
                            },
                            text(terms[t].label.clone()),
                            number(Some(ll)),
                            number(Some(chi)),
                            number(Some(df as f64)),
                            number(Some(chi_square_p(chi, df as f64))),
                        ]);
                        first = false;
                    }
                }
                if !rows.is_empty() {
                    tables.push(table(
                        "Model if Term Removed",
                        2,
                        plain(&[
                            "",
                            "Variable",
                            "Model Log Likelihood",
                            "Change in -2 Log Likelihood",
                            "df",
                            "Sig. of the Change",
                        ]),
                        rows,
                        Some(caption.clone()),
                    ));
                }
            }
            if let Some(t) = context.not_in_equation(&shown, &eligible, &caption) {
                tables.push(t);
            }
            last = steps.pop().expect("nonempty");
            last_fit_ll = last.fit.ll;
            model = last.model.clone();
        }
        let final_fit = &last.fit;
        let stats: Vec<CaseStats> = (0..design.x.len())
            .map(|i| case_stats(&design, final_fit, i, criteria.cut))
            .collect();
        if let Some(cw) = &command.casewise {
            tables
                .push(self.logistic_casewise(command, &design, final_fit, &stats, cw, &dep_label)?);
        }
        for warning in warnings {
            self.warn(warning);
        }
        if command.classplot {
            self.warn("LOGISTIC REGRESSION: CLASSPLOT accepted; charts are not drawn yet");
        }
        if !command.save.is_empty() {
            let original: Vec<Key> = values.iter().map(clone_key).collect();
            self.save_logistic(command, &design, final_fit, &stats, &original, cases)?;
        }
        Ok(tables)
    }

    fn logistic_casewise(
        &self,
        command: &Logistic,
        design: &Design,
        fit: &Fit,
        stats: &[CaseStats],
        cw: &statscraft_syntax::LogisticCasewise,
        dep_label: &dyn Fn(usize) -> String,
    ) -> Result<Table, Error> {
        let data = self.active()?;
        let number_of = |k: usize| self.case_numbers.as_ref().map_or(k, |n| n[k]) + 1;
        let id = command.id.as_ref().map(|n| data.variable(n)).transpose()?;
        let mut columns = plain(&["Case"]);
        if let Some(v) = &id {
            columns.extend(plain(&[&self.shown().variable(v)]));
        }
        columns.extend(plain(&["Selected Status", "Observed"]));
        let coefficient_names = coefficient_names(design, fit);
        for t in &cw.temps {
            match t {
                LogisticTemp::Dfbeta => columns.extend(
                    coefficient_names
                        .iter()
                        .map(|n| ("DFBETA".to_string(), n.clone())),
                ),
                other => columns.push((String::new(), temp_short(*other).to_string())),
            }
        }
        let mut rows = Vec::new();
        for (i, s) in stats.iter().enumerate() {
            if !design.fitted(i) {
                continue;
            }
            if let Some(limit) = cw.outlier
                && s.sresid.abs() < limit
            {
                continue;
            }
            let k = design.case[i];
            let mut row = vec![Cell::Number(number_of(k) as f64)];
            if let Some(v) = &id {
                row.push(match v.values() {
                    Values::Numeric(values) => number(values.get(k)),
                    Values::String(values) => text(values.get(k).trim_end().to_string()),
                });
            }
            row.push(text("S"));
            let observed = usize::from(design.y[i] > 0.5);
            let mut shown = dep_label(observed);
            if s.pgroup as usize != observed {
                shown.push_str("**");
            }
            row.push(text(shown));
            for t in &cw.temps {
                match t {
                    LogisticTemp::Dfbeta => row.extend(s.dfbeta.iter().map(|v| number(Some(*v)))),
                    LogisticTemp::Pgroup => row.push(text(dep_label(s.pgroup as usize))),
                    other => row.push(number(Some(temp_value(s, *other)))),
                }
            }
            rows.push(row);
        }
        let caption = match cw.outlier {
            Some(limit) => format!(
                "Cases whose studentized residual is at least {limit} in absolute value. S = selected, ** = misclassified case."
            ),
            None => "S = selected, ** = misclassified case.".into(),
        };
        Ok(table("Casewise List", 1, columns, rows, Some(caption)))
    }

    fn save_logistic(
        &mut self,
        command: &Logistic,
        design: &Design,
        fit: &Fit,
        stats: &[CaseStats],
        values: &[Key],
        cases: usize,
    ) -> Result<(), Error> {
        let coefficient_names = coefficient_names(design, fit);
        let first = usize::from(!fit.constant);
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            let row_of: Vec<Option<usize>> = {
                let mut v = vec![None; cases];
                for (i, &k) in design.case.iter().enumerate() {
                    v[k] = Some(i);
                }
                v
            };
            for (temp, given) in &command.save {
                let (root, label) = temp_root(*temp);
                let parts: Vec<(String, String, usize)> = match temp {
                    LogisticTemp::Dfbeta => coefficient_names
                        .iter()
                        .enumerate()
                        .map(|(i, n)| (format!("{}", i + first), format!("DFBETA for {n}"), i))
                        .collect(),
                    _ => vec![(String::new(), label.to_string(), 0)],
                };
                let names: Vec<String> = match given {
                    Some(name) if *temp == LogisticTemp::Dfbeta => parts
                        .iter()
                        .map(|(suffix, _, _)| format!("{name}{suffix}"))
                        .collect(),
                    Some(name) => vec![name.clone()],
                    None => {
                        let make = |n: usize| -> Vec<String> {
                            parts
                                .iter()
                                .map(|(suffix, _, _)| format!("{root}{suffix}_{n}"))
                                .collect()
                        };
                        let mut n = 1;
                        while make(n)
                            .iter()
                            .any(|name| data.variable_index(name).is_some())
                        {
                            n += 1;
                        }
                        make(n)
                    }
                };
                for (name, (_, label, at)) in names.iter().zip(&parts) {
                    if data.variable_index(name).is_some() {
                        return Err(Error(format!(
                            "LOGISTIC REGRESSION /SAVE: variable {name} already exists"
                        )));
                    }
                    if *temp == LogisticTemp::Pgroup
                        && let Key::Text(_) = values[0]
                    {
                        let texts: Vec<String> = (0..cases)
                            .map(|k| match row_of[k] {
                                Some(i) => key_text(&values[stats[i].pgroup as usize]),
                                None => String::new(),
                            })
                            .collect();
                        let width = texts.iter().map(String::len).max().unwrap_or(1).max(1);
                        data.set_string_values(name, width, texts)?;
                        data.set_variable_label(name, Some(label))?;
                        continue;
                    }
                    let column: Vec<Option<f64>> = (0..cases)
                        .map(|k| {
                            let i = row_of[k]?;
                            let s = &stats[i];
                            let v = match temp {
                                LogisticTemp::Dfbeta => s.dfbeta[*at],
                                LogisticTemp::Pgroup => match &values[s.pgroup as usize] {
                                    Key::Number(n) => *n,
                                    Key::Text(_) => s.pgroup,
                                },
                                other => temp_value(s, *other),
                            };
                            v.is_finite().then_some(v)
                        })
                        .collect();
                    data.set_numeric_values(name, column)?;
                    data.set_variable_label(name, Some(label))?;
                    data.set_format(
                        name,
                        if *temp == LogisticTemp::Pgroup {
                            "F8.0"
                        } else {
                            "F11.5"
                        },
                    )?;
                    data.set_measure(
                        name,
                        if *temp == LogisticTemp::Pgroup {
                            Measure::Nominal
                        } else {
                            Measure::Scale
                        },
                    )?;
                }
            }
            Ok(())
        })
    }
}

fn clone_key(key: &Key) -> Key {
    match key {
        Key::Number(n) => Key::Number(*n),
        Key::Text(t) => Key::Text(t.clone()),
    }
}

fn sorted(model: &[usize]) -> Vec<usize> {
    let mut v = model.to_vec();
    v.sort_unstable();
    v
}

/// The terms of METHOD number `b`: its list, or every term after WITH.
fn method_terms(command: &Logistic, b: usize, term_vars: &[Vec<String>]) -> Vec<usize> {
    let Some(method) = command.methods.get(b) else {
        return Vec::new();
    };
    let list = if method.terms.is_empty() {
        &command.terms
    } else {
        &method.terms
    };
    let mut out: Vec<usize> = list
        .iter()
        .filter_map(|t| term_vars.iter().position(|u| same_term(u, t)))
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The columns of a term that would not be redundant next to the model of `fit`.
fn new_columns(
    design: &Design,
    fit: &Fit,
    columns: &[usize],
    criteria: &LogisticCriteria,
) -> Vec<usize> {
    let mut wanted = fit.cols.clone();
    wanted.extend(columns);
    independent(design, &wanted, fit.constant, criteria.eps)
        .into_iter()
        .filter(|c| columns.contains(c))
        .collect()
}

fn coefficient_names(design: &Design, fit: &Fit) -> Vec<String> {
    let mut out = Vec::new();
    if fit.constant {
        out.push("Constant".to_string());
    }
    out.extend(fit.cols.iter().map(|&c| design.names[c].clone()));
    out
}

fn temp_short(t: LogisticTemp) -> &'static str {
    match t {
        LogisticTemp::Pred => "Predicted",
        LogisticTemp::Pgroup => "Predicted Group",
        LogisticTemp::Resid => "Resid",
        LogisticTemp::Dev => "Dev",
        LogisticTemp::Lresid => "LResid",
        LogisticTemp::Sresid => "SResid",
        LogisticTemp::Zresid => "ZResid",
        LogisticTemp::Lever => "Lever",
        LogisticTemp::Cook => "Cook",
        LogisticTemp::Dfbeta => "DFBETA",
    }
}

fn temp_root(t: LogisticTemp) -> (&'static str, &'static str) {
    match t {
        LogisticTemp::Pred => ("PRE", "Predicted probability"),
        LogisticTemp::Pgroup => ("PGR", "Predicted group"),
        LogisticTemp::Resid => (
            "RES",
            "Difference between observed and predicted probabilities",
        ),
        LogisticTemp::Dev => ("DEV", "Deviance value"),
        LogisticTemp::Lresid => ("LRE", "Logit residual"),
        LogisticTemp::Sresid => ("SRE", "Studentized residual"),
        LogisticTemp::Zresid => ("ZRE", "Normalized residual"),
        LogisticTemp::Lever => ("LEV", "Leverage value"),
        LogisticTemp::Cook => ("COO", "Analog of Cook's influence statistics"),
        LogisticTemp::Dfbeta => ("DFB", "DFBETA"),
    }
}

fn temp_value(s: &CaseStats, t: LogisticTemp) -> f64 {
    match t {
        LogisticTemp::Pred => s.pred,
        LogisticTemp::Pgroup => s.pgroup,
        LogisticTemp::Resid => s.resid,
        LogisticTemp::Dev => s.dev,
        LogisticTemp::Lresid => s.lresid,
        LogisticTemp::Sresid => s.sresid,
        LogisticTemp::Zresid => s.zresid,
        LogisticTemp::Lever => s.lever,
        LogisticTemp::Cook => s.cook,
        LogisticTemp::Dfbeta => f64::NAN,
    }
}

/// What the tables of every block share.
struct Context<'a> {
    design: &'a Design,
    terms: &'a [TermInfo],
    command: &'a Logistic,
    dep_label: &'a dyn Fn(usize) -> String,
    initial_ll: f64,
}

impl Context<'_> {
    fn step_cell(first: bool, n: usize) -> Cell {
        if first {
            text(format!("Step {n}"))
        } else {
            Cell::Empty
        }
    }

    fn iteration_history(&self, steps: &[&Step], every: u32, block0: bool, caption: &str) -> Table {
        let width = steps.iter().map(|s| s.fit.parameters()).max().unwrap_or(0);
        let mut columns = plain(&["", "Iteration", "-2 Log likelihood"]);
        // the coefficients of the last step name the columns; earlier steps fill from the left
        let names = steps
            .last()
            .map(|s| coefficient_names(self.design, &s.fit))
            .unwrap_or_default();
        columns.extend((0..width).map(|j| {
            (
                "Coefficients".to_string(),
                names.get(j).cloned().unwrap_or_default(),
            )
        }));
        let mut rows = Vec::new();
        for (n, s) in steps.iter().enumerate() {
            let history = &s.fit.history;
            for (i, (m2ll, b)) in history.iter().enumerate() {
                let iteration = i + 1;
                if (iteration - 1) % every as usize != 0 && iteration != history.len() {
                    continue;
                }
                let mut row = vec![
                    if rows.is_empty() || (i == 0) {
                        text(if block0 {
                            "Step 0".to_string()
                        } else {
                            format!("Step {}", n + 1)
                        })
                    } else {
                        Cell::Empty
                    },
                    Cell::Number(iteration as f64),
                    number(Some(*m2ll)),
                ];
                row.extend((0..width).map(|j| number(b.get(j).copied())));
                rows.push(row);
            }
        }
        let mut notes = vec![caption.to_string()];
        notes.extend(steps.iter().map(|s| s.fit.note.clone()));
        table("Iteration History", 2, columns, rows, Some(notes.join(" ")))
    }

    fn classification(&self, steps: &[(usize, &Step)], caption: &str) -> Table {
        let d = self.design;
        let cut = self.command.criteria.cut;
        let mut columns = plain(&["", "Observed"]);
        columns.push(("Predicted".into(), (self.dep_label)(0)));
        columns.push(("Predicted".into(), (self.dep_label)(1)));
        columns.push(("Predicted".into(), "Percentage Correct".into()));
        let mut rows = Vec::new();
        for &(n, s) in steps {
            let mut counts = [[0.0; 2]; 2];
            for i in (0..d.x.len()).filter(|&i| d.fitted(i)) {
                let observed = usize::from(d.y[i] > 0.5);
                let predicted = usize::from(s.fit.pi(d, i) >= cut);
                counts[observed][predicted] += d.c[i];
            }
            for (observed, count) in counts.iter().enumerate() {
                let row_total = count[0] + count[1];
                rows.push(vec![
                    Self::step_cell(observed == 0, n),
                    text((self.dep_label)(observed)),
                    Cell::Number(count[0]),
                    Cell::Number(count[1]),
                    number((row_total > 0.0).then(|| 100.0 * count[observed] / row_total)),
                ]);
            }
            let total = counts[0][0] + counts[0][1] + counts[1][0] + counts[1][1];
            rows.push(vec![
                Cell::Empty,
                text("Overall Percentage"),
                Cell::Empty,
                Cell::Empty,
                number((total > 0.0).then(|| 100.0 * (counts[0][0] + counts[1][1]) / total)),
            ]);
        }
        table(
            "Classification Table",
            2,
            columns,
            rows,
            Some(format!("{caption} The cut value is {cut}.")),
        )
    }

    fn in_equation(&self, steps: &[(usize, &Step)], caption: &str) -> Table {
        let ci = self.command.print.ci;
        let mut columns = plain(&["", "", "B", "S.E.", "Wald", "df", "Sig.", "Exp(B)"]);
        if let Some(level) = ci {
            let group = format!("{level}% C.I.for EXP(B)");
            columns.push((group.clone(), "Lower".into()));
            columns.push((group, "Upper".into()));
        }
        let z = ci.map(|level| normal_quantile(0.5 + level / 200.0));
        let mut rows = Vec::new();
        let mut entered_notes = Vec::new();
        for &(n, s) in steps {
            let fit = &s.fit;
            let mut first = true;
            let push_coefficient =
                |rows: &mut Vec<Vec<Cell>>, first: &mut bool, name: String, at: usize| {
                    let constant = fit.constant && at == 0;
                    let b = fit.b[at];
                    let se = fit.cov[at][at].sqrt();
                    let w = (b / se).powi(2);
                    let mut row = vec![
                        Self::step_cell(*first, n),
                        text(name),
                        number(Some(b)),
                        number(Some(se)),
                        number(Some(w)),
                        Cell::Number(1.0),
                        number(Some(chi_square_p(w, 1.0))),
                        number(Some(b.exp())),
                    ];
                    if let Some(z) = z {
                        // the constant has no interval
                        row.push(number((!constant).then(|| (b - z * se).exp())));
                        row.push(number((!constant).then(|| (b + z * se).exp())));
                    }
                    rows.push(row);
                    *first = false;
                };
            for &t in &s.model {
                let term = &self.terms[t];
                let positions: Vec<(usize, usize)> = term
                    .columns
                    .iter()
                    .filter_map(|&c| fit.at(c).map(|p| (c, p)))
                    .collect();
                if term.categorical && positions.len() > 1 {
                    let at: Vec<usize> = positions.iter().map(|p| p.1).collect();
                    let w = wald(fit, &at);
                    let df = at.len() as f64;
                    let mut row = vec![
                        Self::step_cell(first, n),
                        text(term.label.clone()),
                        Cell::Empty,
                        Cell::Empty,
                        number(Some(w)),
                        Cell::Number(df),
                        number(Some(chi_square_p(w, df))),
                        Cell::Empty,
                    ];
                    if z.is_some() {
                        row.extend([Cell::Empty, Cell::Empty]);
                    }
                    rows.push(row);
                    first = false;
                }
                for (c, p) in positions {
                    push_coefficient(&mut rows, &mut first, self.design.names[c].clone(), p);
                }
            }
            if fit.constant {
                push_coefficient(&mut rows, &mut first, "Constant".into(), 0);
            }
            let list = |ts: &[usize]| {
                ts.iter()
                    .map(|&t| self.terms[t].label.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            if !s.entered.is_empty() {
                entered_notes.push(format!(
                    "Variable(s) entered on step {n}: {}.",
                    list(&s.entered)
                ));
            }
            if !s.removed.is_empty() {
                entered_notes.push(format!(
                    "Variable(s) removed on step {n}: {}.",
                    list(&s.removed)
                ));
            }
        }
        let mut note = caption.to_string();
        for e in entered_notes {
            note.push(' ');
            note.push_str(&e);
        }
        table("Variables in the Equation", 2, columns, rows, Some(note))
    }

    fn correlations(&self, steps: &[(usize, &Step)], caption: &str) -> Table {
        let width = steps
            .iter()
            .map(|(_, s)| s.fit.parameters())
            .max()
            .unwrap_or(0);
        let last_names = steps
            .last()
            .map(|(_, s)| coefficient_names(self.design, &s.fit))
            .unwrap_or_default();
        let mut columns = plain(&["", ""]);
        columns.extend((0..width).map(|j| {
            (
                String::new(),
                last_names.get(j).cloned().unwrap_or_default(),
            )
        }));
        let mut rows = Vec::new();
        for &(n, s) in steps {
            let names = coefficient_names(self.design, &s.fit);
            let cov = &s.fit.cov;
            for (r, name) in names.iter().enumerate() {
                let mut row = vec![Self::step_cell(r == 0, n), text(name.clone())];
                row.extend((0..width).map(|c| {
                    number((c < names.len()).then(|| cov[r][c] / (cov[r][r] * cov[c][c]).sqrt()))
                }));
                rows.push(row);
            }
        }
        table(
            "Correlation Matrix",
            2,
            columns,
            rows,
            Some(caption.to_string()),
        )
    }

    fn not_in_equation(
        &self,
        steps: &[(usize, &Step)],
        eligible: &[usize],
        caption: &str,
    ) -> Option<Table> {
        let criteria = &self.command.criteria;
        let mut rows = Vec::new();
        for &(n, s) in steps {
            let out: Vec<usize> = eligible
                .iter()
                .copied()
                .filter(|t| !s.model.contains(t))
                .collect();
            if out.is_empty() {
                continue;
            }
            let mut first = true;
            let mut all = Vec::new();
            for &t in &out {
                let cols = new_columns(self.design, &s.fit, &self.terms[t].columns, criteria);
                all.extend(cols.iter().copied());
                let mut push = |name: String, cols: &[usize], rows: &mut Vec<Vec<Cell>>| {
                    let score = score_test(self.design, &s.fit, cols);
                    let df = cols.len() as f64;
                    rows.push(vec![
                        Self::step_cell(first, n),
                        text(name),
                        number(Some(score)),
                        Cell::Number(df),
                        number(Some(chi_square_p(score, df))),
                    ]);
                    first = false;
                };
                if self.terms[t].categorical && cols.len() > 1 {
                    push(self.terms[t].label.clone(), &cols, &mut rows);
                }
                for &c in &cols {
                    push(self.design.names[c].clone(), &[c], &mut rows);
                }
            }
            let overall: Vec<usize> = {
                let mut wanted = s.fit.cols.clone();
                wanted.extend(&all);
                independent(self.design, &wanted, s.fit.constant, criteria.eps)
                    .into_iter()
                    .filter(|c| all.contains(c))
                    .collect()
            };
            let score = score_test(self.design, &s.fit, &overall);
            let df = overall.len() as f64;
            rows.push(vec![
                Cell::Empty,
                text("Overall Statistics"),
                number(Some(score)),
                Cell::Number(df),
                number(Some(chi_square_p(score, df))),
            ]);
        }
        (!rows.is_empty()).then(|| {
            table(
                "Variables not in the Equation",
                2,
                plain(&["", "", "Score", "df", "Sig."]),
                rows,
                Some(caption.to_string()),
            )
        })
    }

    fn hosmer_lemeshow(&self, steps: &[(usize, &Step)], caption: &str) -> Vec<Table> {
        let mut test = Vec::new();
        let mut contingency = Vec::new();
        for &(n, s) in steps {
            let groups = hosmer_lemeshow(self.design, &s.fit);
            let chi: f64 = groups
                .iter()
                .map(|&(total, observed, expected)| {
                    let xi = expected / total;
                    (observed - expected).powi(2) / (expected * (1.0 - xi))
                })
                .sum();
            let df = groups.len() as f64 - 2.0;
            test.push(vec![
                text(format!("Step {n}")),
                number(Some(chi)),
                number(Some(df)),
                number((df > 0.0).then(|| chi_square_p(chi, df))),
            ]);
            for (g, &(total, observed, expected)) in groups.iter().enumerate() {
                contingency.push(vec![
                    Self::step_cell(g == 0, n),
                    Cell::Number((g + 1) as f64),
                    number(Some(total - observed)),
                    number(Some(total - expected)),
                    number(Some(observed)),
                    number(Some(expected)),
                    number(Some(total)),
                ]);
            }
        }
        let dep = &self.command.dependent;
        let mut columns = plain(&["", ""]);
        for v in 0..2 {
            let group = format!("{dep} = {}", (self.dep_label)(v));
            columns.push((group.clone(), "Observed".into()));
            columns.push((group, "Expected".into()));
        }
        columns.push((String::new(), "Total".into()));
        vec![
            table(
                "Hosmer and Lemeshow Test",
                1,
                plain(&["Step", "Chi-square", "df", "Sig."]),
                test,
                Some(caption.to_string()),
            ),
            table(
                "Contingency Table for Hosmer and Lemeshow Test",
                2,
                columns,
                contingency,
                Some(caption.to_string()),
            ),
        ]
    }
}
