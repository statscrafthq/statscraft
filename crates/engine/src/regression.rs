//! REGRESSION: linear regression by sweeping the correlation matrix, with forced entry and
//! removal, stepwise selection, residuals and influence statistics.
//!
//! Rules from the Command Syntax Reference (REGRESSION) and the Algorithms manual (REGRESSION):
//! - One correlation matrix of all variables (VARIABLES, else those named on DEPENDENT and
//!   METHOD) serves every equation. LISTWISE uses the cases valid on all of them, PAIRWISE each
//!   pair's cases, MEANSUBSTITUTION every case with the means in place of missing values.
//!   Weights: w = case weight times REGWGT; means and cross-products use w; covariances divide
//!   by C - 1, C being the sum of the case weights; degrees of freedom use C.
//! - With ORIGIN the cross-products are about 0 and there is no constant.
//! - Sweeping a variable enters it; sweeping again removes it. A variable can enter only when
//!   its tolerance and the tolerances of the variables in the equation stay at or above
//!   TOLERANCE. ENTER takes the variable with the largest tolerance first.
//! - F-to-enter = (C - p* - 1) V / (r_yy - V), F-to-remove = (C - p*) |V| / r_yy with
//!   V = r_yk r_ky / r_kk, p* the number of coefficients in the equation.
//! - STEPWISE: remove the variable with the smallest F-to-remove while it fails POUT (any
//!   variable in the equation), else enter the block variable with the largest F-to-enter that
//!   passes PIN; at most twice the block's size of steps unless MAXSTEPS says. FORWARD enters
//!   only, BACKWARD removes only (block variables; an empty equation gets the block first).
//!   PIN above POUT (or FIN below FOUT) moves POUT (FOUT) to it, with a warning.
//! - Leverage, residuals and influence statistics follow the formulas of the Algorithms
//!   manual; ZPRED and ZRESID are system-missing with REGWGT.

use crate::descriptives::{case_weights, number};
use crate::dist::{f_p, t_p, t_quantile};
use crate::matrix::{Matrix, inverse, sweep, symmetric_eigen};
use crate::means::is_valid;
use crate::npar_tests::text;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Measure, Values, Variable};
use statscraft_syntax::{
    Criterion, MethodKind, Regression, RegressionCriteria, RegressionDescriptives,
    RegressionMissing, TempVar,
};

/// Weighted sums of one pair of variables over their common cases.
struct PairSums {
    /// The sum of case weights.
    c: f64,
    /// The sum of w = case weight times REGWGT.
    w: f64,
    mx: f64,
    cxx: f64,
    cyy: f64,
    cxy: f64,
}

/// Groups columns by the cases where they are valid (not NaN): the class of every column, and
/// the valid cases of every class in case order. Columns of one class share every sum over
/// their common cases, so pairwise statistics can take those sums once per column.
pub(crate) fn valid_classes<C: AsRef<[f64]>>(columns: &[C]) -> (Vec<usize>, Vec<Vec<usize>>) {
    let mut first: Vec<usize> = Vec::new();
    let mut class = Vec::with_capacity(columns.len());
    for (i, column) in columns.iter().enumerate() {
        let same = |&f: &usize| {
            let (a, b) = (columns[f].as_ref(), column.as_ref());
            a.iter().zip(b).all(|(x, y)| x.is_nan() == y.is_nan())
        };
        match first.iter().position(same) {
            Some(c) => class.push(c),
            None => {
                class.push(first.len());
                first.push(i);
            }
        }
    }
    let valid = (first.iter())
        .map(|&f| {
            let column = columns[f].as_ref();
            (0..column.len()).filter(|&k| !column[k].is_nan()).collect()
        })
        .collect();
    (class, valid)
}

/// The sums of one pair over `cases`, the cases where both values are valid, in case order.
/// `known` holds c, w and the two weighted means when the caller has them for these cases.
fn pair_sums(
    (x, y): (&[f64], &[f64]),
    (cw, g): (&[f64], &[f64]),
    cases: &[usize],
    known: Option<(f64, f64, f64, f64)>,
    origin: bool,
) -> PairSums {
    let (c, w, mx, my) = known.unwrap_or_else(|| {
        let c: f64 = cases.iter().map(|&k| cw[k]).sum();
        let w: f64 = cases.iter().map(|&k| cw[k] * g[k]).sum();
        let mx = cases.iter().map(|&k| x[k] * cw[k] * g[k]).sum::<f64>() / w;
        let my = cases.iter().map(|&k| y[k] * cw[k] * g[k]).sum::<f64>() / w;
        (c, w, mx, my)
    });
    let (ox, oy) = if origin { (0.0, 0.0) } else { (mx, my) };
    let (mut cxx, mut cyy, mut cxy) = (0.0, 0.0, 0.0);
    for &k in cases {
        let (x, y, w) = (x[k], y[k], cw[k] * g[k]);
        cxx += w * (x - ox) * (x - ox);
        cyy += w * (y - oy) * (y - oy);
        cxy += w * (x - ox) * (y - oy);
    }
    PairSums {
        c,
        w,
        mx,
        cxx,
        cyy,
        cxy,
    }
}

/// The moments of the variables of the correlation matrix.
struct Moments {
    mean: Vec<f64>,
    /// Covariances (about 0 with ORIGIN), divided by C - 1.
    cov: Matrix,
    corr: Matrix,
    /// The sum of case weights behind each entry.
    c: Matrix,
    /// The sum of w behind each variable.
    w: Vec<f64>,
}

fn moments(columns: &[Vec<f64>], c: &[f64], g: &[f64], origin: bool) -> Moments {
    let m = columns.len();
    let mut out = Moments {
        mean: vec![f64::NAN; m],
        cov: vec![vec![f64::NAN; m]; m],
        corr: vec![vec![f64::NAN; m]; m],
        c: vec![vec![0.0; m]; m],
        w: vec![0.0; m],
    };
    // a pair of variables valid in the same cases has the sums of its class, taken once per
    // variable in the same order, so the results stay the same
    let (class, valid) = valid_classes(columns);
    let own: Vec<(f64, f64, f64)> = (0..m)
        .map(|i| {
            let (x, cases) = (&columns[i], &valid[class[i]]);
            let sum_c: f64 = cases.iter().map(|&k| c[k]).sum();
            let w: f64 = cases.iter().map(|&k| c[k] * g[k]).sum();
            let mean = cases.iter().map(|&k| x[k] * c[k] * g[k]).sum::<f64>() / w;
            (sum_c, w, mean)
        })
        .collect();
    for i in 0..m {
        for j in i..m {
            let (x, y) = (&columns[i][..], &columns[j][..]);
            let s = if class[i] == class[j] {
                let known = (own[i].0, own[i].1, own[i].2, own[j].2);
                pair_sums((x, y), (c, g), &valid[class[i]], Some(known), origin)
            } else {
                let both: Vec<usize> = (0..c.len())
                    .filter(|&k| !x[k].is_nan() && !y[k].is_nan())
                    .collect();
                pair_sums((x, y), (c, g), &both, None, origin)
            };
            if i == j {
                out.mean[i] = s.mx;
                out.w[i] = s.w;
            }
            out.cov[i][j] = s.cxy / (s.c - 1.0);
            out.cov[j][i] = out.cov[i][j];
            out.corr[i][j] = if i == j && s.cxx > 0.0 {
                1.0
            } else {
                s.cxy / (s.cxx * s.cyy).sqrt()
            };
            out.corr[j][i] = out.corr[i][j];
            out.c[i][j] = s.c;
            out.c[j][i] = s.c;
        }
    }
    out
}

/// One equation: the candidate independents and the dependent last, as indices into the
/// matrix variables, with what the statistics need.
struct Equation {
    names: Vec<String>,
    /// The names as tables show them (SET TVARS); `names` go into saved variable labels.
    texts: Vec<String>,
    /// Correlations of the candidates and the dependent (last).
    r: Matrix,
    /// Variances (S_kk) and means.
    var: Vec<f64>,
    mean: Vec<f64>,
    /// Covariances, for the cross-product matrix of the collinearity diagnostics.
    cov: Matrix,
    c: f64,
    w: f64,
    origin: bool,
}

impl Equation {
    fn y(&self) -> usize {
        self.names.len() - 1
    }

    fn swept(&self, set: &[bool]) -> Matrix {
        let mut m = self.r.clone();
        for (k, _) in set.iter().enumerate().filter(|(_, in_)| **in_) {
            sweep(&mut m, k);
        }
        m
    }

    /// The number of coefficients of an equation with `p` variables.
    fn coefficients(&self, p: usize) -> f64 {
        (p + usize::from(!self.origin)) as f64
    }

    /// The weighted uncentered cross-product of two terms, None being the constant 1.
    fn cross(&self, a: Option<usize>, b: Option<usize>) -> f64 {
        let mean = |k: Option<usize>| k.map_or(1.0, |k| self.mean[k]);
        let centered = match (a, b) {
            (Some(a), Some(b)) => (self.c - 1.0) * self.cov[a][b],
            _ => 0.0,
        };
        if self.origin {
            centered
        } else {
            centered + self.w * mean(a) * mean(b)
        }
    }

    /// The total sum of squares of the dependent.
    fn ss_total(&self) -> f64 {
        (self.c - 1.0) * self.var[self.y()]
    }

    /// True when `k` may enter the equation `m` (swept on `set`) without breaking the
    /// tolerance.
    fn may_enter(&self, m: &Matrix, set: &[bool], k: usize, tolerance: f64) -> bool {
        let r_kk = m[k][k];
        r_kk.is_finite()
            && r_kk >= tolerance
            && set
                .iter()
                .enumerate()
                .filter(|(_, in_)| **in_)
                .all(|(j, _)| (m[j][j] - m[j][k] * m[k][j] / r_kk) * tolerance <= 1.0)
    }

    fn f_to_enter(&self, m: &Matrix, p: usize, k: usize) -> (f64, f64) {
        let y = self.y();
        let v = m[y][k] * m[k][y] / m[k][k];
        let df = self.c - self.coefficients(p) - 1.0;
        let f = df * v / (m[y][y] - v);
        (f, f_p(f, 1.0, df))
    }

    fn f_to_remove(&self, m: &Matrix, p: usize, k: usize) -> (f64, f64) {
        let y = self.y();
        let v = m[y][k] * m[k][y] / m[k][k];
        let df = self.c - self.coefficients(p);
        let f = df * v.abs() / m[y][y];
        (f, f_p(f, 1.0, df))
    }
}

/// The state after one step: the variables in the equation, with what changed.
struct Model {
    set: Vec<bool>,
    entered: Vec<usize>,
    removed: Vec<usize>,
    method: String,
}

fn passes(criterion: Criterion, (f, p): (f64, f64), entry: bool) -> bool {
    match (criterion, entry) {
        (Criterion::Probability(limit), true) => p < limit,
        (Criterion::Probability(limit), false) => p > limit,
        (Criterion::F(limit), true) => f > limit,
        (Criterion::F(limit), false) => f < limit,
    }
}

fn criterion_text(criterion: Criterion, entry: bool) -> String {
    match (criterion, entry) {
        (Criterion::Probability(v), true) => format!("Probability-of-F-to-enter <= {v:.3}"),
        (Criterion::Probability(v), false) => format!("Probability-of-F-to-remove >= {v:.3}"),
        (Criterion::F(v), true) => format!("F-to-enter >= {v:.3}"),
        (Criterion::F(v), false) => format!("F-to-remove <= {v:.3}"),
    }
}

/// Runs the methods on one equation and returns its models (one per step that changed it).
fn select(
    eq: &Equation,
    blocks: &[(MethodKind, Vec<usize>)],
    criteria: &RegressionCriteria,
    warnings: &mut Vec<String>,
) -> Vec<Model> {
    let n = eq.y();
    let mut set = vec![false; n];
    let mut models: Vec<Model> = Vec::new();
    let count = |set: &[bool]| set.iter().filter(|s| **s).count();
    for (kind, block) in blocks {
        let block: Vec<usize> = block.iter().copied().filter(|&k| k < n).collect();
        match kind {
            MethodKind::Enter => {
                let mut entered = Vec::new();
                loop {
                    let m = eq.swept(&set);
                    let best = block
                        .iter()
                        .copied()
                        .filter(|&k| !set[k] && eq.may_enter(&m, &set, k, criteria.tolerance))
                        .max_by(|&a, &b| m[a][a].total_cmp(&m[b][b]).then(b.cmp(&a)));
                    let Some(k) = best else { break };
                    set[k] = true;
                    entered.push(k);
                }
                let refused: Vec<&str> = block
                    .iter()
                    .filter(|&&k| !set[k])
                    .map(|&k| eq.names[k].as_str())
                    .collect();
                if !refused.is_empty() {
                    warnings.push(format!(
                        "{} not entered into the equation of {}: the tolerance limit {} was reached",
                        refused.join(", "),
                        eq.names[n],
                        criteria.tolerance
                    ));
                }
                if !entered.is_empty() {
                    entered.sort_unstable();
                    models.push(Model {
                        set: set.clone(),
                        entered,
                        removed: Vec::new(),
                        method: "Enter".into(),
                    });
                }
            }
            MethodKind::Remove => {
                let mut removed: Vec<usize> = block.iter().copied().filter(|&k| set[k]).collect();
                removed.sort_unstable();
                for &k in &removed {
                    set[k] = false;
                }
                if !removed.is_empty() {
                    models.push(Model {
                        set: set.clone(),
                        entered: Vec::new(),
                        removed,
                        method: "Remove".into(),
                    });
                }
            }
            MethodKind::Stepwise | MethodKind::Forward | MethodKind::Backward => {
                let (enter, remove) = match kind {
                    MethodKind::Stepwise => (true, true),
                    MethodKind::Forward => (true, false),
                    _ => (false, true),
                };
                let method = match kind {
                    MethodKind::Stepwise => format!(
                        "Stepwise (Criteria: {}, {})",
                        criterion_text(criteria.entry, true),
                        criterion_text(criteria.removal, false)
                    ),
                    MethodKind::Forward => {
                        format!(
                            "Forward (Criterion: {})",
                            criterion_text(criteria.entry, true)
                        )
                    }
                    _ => format!(
                        "Backward (Criterion: {})",
                        criterion_text(criteria.removal, false)
                    ),
                };
                if *kind == MethodKind::Backward && count(&set) == 0 {
                    let mut entered = Vec::new();
                    for &k in &block {
                        let m = eq.swept(&set);
                        if eq.may_enter(&m, &set, k, criteria.tolerance) {
                            set[k] = true;
                            entered.push(k);
                        }
                    }
                    if !entered.is_empty() {
                        models.push(Model {
                            set: set.clone(),
                            entered,
                            removed: Vec::new(),
                            method: "Enter".into(),
                        });
                    }
                }
                let limit = criteria.max_steps.map_or(
                    if *kind == MethodKind::Stepwise {
                        2 * block.len()
                    } else {
                        usize::MAX
                    },
                    |n| n as usize,
                );
                let mut steps = 0;
                while steps < limit {
                    let m = eq.swept(&set);
                    let p = count(&set);
                    // stepwise may remove any variable in the equation; backward its block's
                    let removable: Vec<usize> = (0..n)
                        .filter(|&k| {
                            set[k] && (*kind == MethodKind::Stepwise || block.contains(&k))
                        })
                        .collect();
                    let weakest = removable
                        .iter()
                        .map(|&k| (k, eq.f_to_remove(&m, p, k)))
                        .min_by(|a, b| a.1.0.total_cmp(&b.1.0));
                    if remove
                        && let Some((k, test)) = weakest
                        && passes(criteria.removal, test, false)
                    {
                        set[k] = false;
                        models.push(Model {
                            set: set.clone(),
                            entered: Vec::new(),
                            removed: vec![k],
                            method: method.clone(),
                        });
                        steps += 1;
                        continue;
                    }
                    if !enter {
                        break;
                    }
                    let strongest = block
                        .iter()
                        .copied()
                        .filter(|&k| !set[k] && eq.may_enter(&m, &set, k, criteria.tolerance))
                        .map(|k| (k, eq.f_to_enter(&m, p, k)))
                        .max_by(|a, b| a.1.0.total_cmp(&b.1.0).then(b.0.cmp(&a.0)));
                    match strongest {
                        Some((k, test)) if passes(criteria.entry, test, true) => {
                            set[k] = true;
                            models.push(Model {
                                set: set.clone(),
                                entered: vec![k],
                                removed: Vec::new(),
                                method: method.clone(),
                            });
                            steps += 1;
                        }
                        _ => break,
                    }
                }
            }
        }
    }
    models
}

/// The statistics of one model.
struct Fit {
    p: usize,
    /// Coefficients p*.
    k: f64,
    r2: f64,
    ss_res: f64,
    ss_reg: f64,
    df_res: f64,
    mse: f64,
    swept: Matrix,
    /// b, its standard error, for the variables in the equation (in order) and the constant.
    b: Vec<(usize, f64, f64)>,
    constant: Option<(f64, f64)>,
}

fn fit(eq: &Equation, set: &[bool]) -> Fit {
    let m = eq.swept(set);
    let y = eq.y();
    let p = set.iter().filter(|s| **s).count();
    let k = eq.coefficients(p);
    let r2 = 1.0 - m[y][y];
    let ss_total = eq.ss_total();
    let df_res = eq.c - k;
    let ss_res = m[y][y] * ss_total;
    let mse = ss_res / df_res;
    let vars: Vec<usize> = (0..y).filter(|&j| set[j]).collect();
    let syy = eq.var[y];
    let b: Vec<(usize, f64, f64)> = vars
        .iter()
        .map(|&j| {
            let b = m[y][j] * (syy / eq.var[j]).sqrt();
            let se = (m[j][j] * m[y][y] * syy / (eq.var[j] * df_res)).sqrt();
            (j, b, se)
        })
        .collect();
    let cov =
        |a: usize, c: usize| m[a][c] * m[y][y] * syy / ((eq.var[a] * eq.var[c]).sqrt() * df_res);
    let constant = (!eq.origin).then(|| {
        let b0 = eq.mean[y] - b.iter().map(|&(j, b, _)| b * eq.mean[j]).sum::<f64>();
        let mut v = mse / eq.w;
        for (i, &a) in vars.iter().enumerate() {
            v += eq.mean[a] * eq.mean[a] * cov(a, a);
            for &c in &vars[..i] {
                v += 2.0 * eq.mean[a] * eq.mean[c] * cov(a, c);
            }
        }
        (b0, v.sqrt())
    });
    Fit {
        p,
        k,
        r2,
        ss_res,
        ss_reg: r2 * ss_total,
        df_res,
        mse,
        swept: m,
        b,
        constant,
    }
}

impl Fit {
    /// The covariance of the coefficients of `a` and `c`.
    fn cov(&self, eq: &Equation, a: usize, c: usize) -> f64 {
        let (m, y) = (&self.swept, eq.y());
        m[a][c] * m[y][y] * eq.var[y] / ((eq.var[a] * eq.var[c]).sqrt() * self.df_res)
    }
}

const LABELS: [(TempVar, &str, &str); 19] = [
    (TempVar::Pred, "PRE", "Unstandardized Predicted Value"),
    (TempVar::AdjPred, "ADJ", "Adjusted Predicted Value"),
    (TempVar::Sresid, "SRE", "Studentized Residual"),
    (TempVar::Mahal, "MAH", "Mahalanobis Distance"),
    (TempVar::Resid, "RES", "Unstandardized Residual"),
    (TempVar::Zpred, "ZPR", "Standardized Predicted Value"),
    (TempVar::Sdresid, "SDR", "Studentized Deleted Residual"),
    (TempVar::Cook, "COO", "Cook's Distance"),
    (TempVar::Dresid, "DRE", "Deleted Residual"),
    (TempVar::Zresid, "ZRE", "Standardized Residual"),
    (TempVar::Sepred, "SEP", "Standard Error of Predicted Value"),
    (TempVar::Lever, "LEV", "Centered Leverage Value"),
    (TempVar::Dfbeta, "DFB", "DFBETA"),
    (TempVar::Sdbeta, "SDB", "Standardized DFBETA"),
    (TempVar::Dffit, "DFF", "DFFIT"),
    (TempVar::Sdfit, "SDF", "Standardized DFFIT"),
    (TempVar::Covratio, "COV", "COVRATIO"),
    (TempVar::Mcin, "MCI", "Mean Confidence Interval"),
    (TempVar::Icin, "ICI", "Individual Confidence Interval"),
];

fn label_of(var: TempVar) -> &'static (TempVar, &'static str, &'static str) {
    LABELS
        .iter()
        .find(|l| l.0 == var)
        .expect("every temporary variable")
}

/// The name of a temporary variable in the Residuals Statistics and casewise tables.
fn short_label(var: TempVar) -> &'static str {
    match var {
        TempVar::Pred => "Predicted Value",
        TempVar::Zpred => "Std. Predicted Value",
        TempVar::Sepred => "Standard Error of Predicted Value",
        TempVar::AdjPred => "Adjusted Predicted Value",
        TempVar::Resid => "Residual",
        TempVar::Zresid => "Std. Residual",
        TempVar::Sresid => "Stud. Residual",
        TempVar::Dresid => "Deleted Residual",
        TempVar::Sdresid => "Stud. Deleted Residual",
        TempVar::Mahal => "Mahal. Distance",
        TempVar::Cook => "Cook's Distance",
        TempVar::Lever => "Centered Leverage Value",
        TempVar::Dffit => "DFFIT",
        TempVar::Sdfit => "Standardized DFFIT",
        TempVar::Covratio => "COVRATIO",
        TempVar::Dfbeta => "DFBETA",
        TempVar::Sdbeta => "Standardized DFBETA",
        TempVar::Mcin => "Mean Confidence Interval",
        TempVar::Icin => "Individual Confidence Interval",
    }
}

/// The temporary variables of one case; NaN where not defined. DFBETA and SDBETA hold one
/// value per coefficient, MCIN and ICIN the lower and upper bound.
#[derive(Clone)]
struct CaseValues {
    pred: f64,
    resid: f64,
    zpred: f64,
    zresid: f64,
    sresid: f64,
    dresid: f64,
    sdresid: f64,
    adjpred: f64,
    sepred: f64,
    mahal: f64,
    cook: f64,
    lever: f64,
    dffit: f64,
    sdfit: f64,
    covratio: f64,
    dfbeta: Vec<f64>,
    sdbeta: Vec<f64>,
    mcin: (f64, f64),
    icin: (f64, f64),
}

impl CaseValues {
    fn empty(k: usize) -> Self {
        let nan = f64::NAN;
        Self {
            pred: nan,
            resid: nan,
            zpred: nan,
            zresid: nan,
            sresid: nan,
            dresid: nan,
            sdresid: nan,
            adjpred: nan,
            sepred: nan,
            mahal: nan,
            cook: nan,
            lever: nan,
            dffit: nan,
            sdfit: nan,
            covratio: nan,
            dfbeta: vec![nan; k],
            sdbeta: vec![nan; k],
            mcin: (nan, nan),
            icin: (nan, nan),
        }
    }

    fn get(&self, var: TempVar) -> f64 {
        match var {
            TempVar::Pred => self.pred,
            TempVar::Resid => self.resid,
            TempVar::Zpred => self.zpred,
            TempVar::Zresid => self.zresid,
            TempVar::Sresid => self.sresid,
            TempVar::Dresid => self.dresid,
            TempVar::Sdresid => self.sdresid,
            TempVar::AdjPred => self.adjpred,
            TempVar::Sepred => self.sepred,
            TempVar::Mahal => self.mahal,
            TempVar::Cook => self.cook,
            TempVar::Lever => self.lever,
            TempVar::Dffit => self.dffit,
            TempVar::Sdfit => self.sdfit,
            TempVar::Covratio => self.covratio,
            TempVar::Dfbeta => self.dfbeta.first().copied().unwrap_or(f64::NAN),
            TempVar::Sdbeta => self.sdbeta.first().copied().unwrap_or(f64::NAN),
            TempVar::Mcin => self.mcin.0,
            TempVar::Icin => self.icin.0,
        }
    }
}

/// Weighted minimum, maximum, mean, standard deviation and N of (value, case weight) pairs.
fn summary(values: &[(f64, f64)]) -> [Option<f64>; 5] {
    let values: Vec<(f64, f64)> = values.iter().copied().filter(|v| v.0.is_finite()).collect();
    let n: f64 = values.iter().map(|v| v.1).sum();
    if values.is_empty() {
        return [None, None, None, None, Some(0.0)];
    }
    let mean = values.iter().map(|v| v.0 * v.1).sum::<f64>() / n;
    let ss: f64 = values.iter().map(|v| v.1 * (v.0 - mean).powi(2)).sum();
    [
        values.iter().map(|v| v.0).reduce(f64::min),
        values.iter().map(|v| v.0).reduce(f64::max),
        Some(mean),
        (n > 1.0).then(|| (ss / (n - 1.0)).sqrt()),
        Some(n),
    ]
}

pub(crate) fn table(
    title: &str,
    row_headers: usize,
    columns: Vec<(String, String)>,
    rows: Vec<Vec<Cell>>,
    caption: Option<String>,
) -> Table {
    let groups = columns
        .iter()
        .map(|(g, _)| {
            if g.is_empty() {
                Vec::new()
            } else {
                vec![g.clone()]
            }
        })
        .collect();
    Table {
        title: title.into(),
        row_headers,
        columns: columns.into_iter().map(|(_, c)| c).collect(),
        column_groups: groups,
        rows,
        caption,
        ..Table::default()
    }
}

pub(crate) fn plain(names: &[&str]) -> Vec<(String, String)> {
    names
        .iter()
        .map(|n| (String::new(), n.to_string()))
        .collect()
}

impl Engine {
    pub(crate) fn regression(&mut self, command: &Regression) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let dependents = self.expand(&command.dependent)?;
        let mut method_lists = Vec::new();
        for method in &command.methods {
            method_lists.push(self.expand(&method.variables)?);
        }
        let mut names: Vec<String> = match &command.variables {
            Some(items) => self.expand(items)?,
            None => Vec::new(),
        };
        let given = command.variables.as_ref().map(|_| names.clone());
        for name in dependents.iter().chain(method_lists.iter().flatten()) {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
        let variables: Vec<Variable> = names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = variables.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "REGRESSION needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let index = |name: &str| names.iter().position(|n| n == name).expect("collected");
        let shown = self.shown();
        let mut warnings = Vec::new();
        // the blocks of each method as matrix indices; an empty list is VARIABLES minus the
        // dependents
        let blocks: Vec<(MethodKind, Vec<usize>)> = command
            .methods
            .iter()
            .zip(&method_lists)
            .map(|(method, list)| {
                let list: Vec<String> = if list.is_empty() {
                    given
                        .iter()
                        .flatten()
                        .filter(|n| !dependents.contains(n))
                        .cloned()
                        .collect()
                } else {
                    list.clone()
                };
                (method.kind, list.iter().map(|n| index(n)).collect())
            })
            .collect();
        let mut criteria = command.criteria;
        match (criteria.entry, criteria.removal) {
            (Criterion::Probability(pin), Criterion::Probability(pout)) if pin > pout => {
                criteria.removal = Criterion::Probability(pin);
                warnings.push(format!(
                    "PIN {pin} is above POUT {pout}; POUT is set to {pin} so that no variable can enter and leave in turn"
                ));
            }
            (Criterion::F(fin), Criterion::F(fout)) if fin < fout => {
                criteria.removal = Criterion::F(fin);
                warnings.push(format!(
                    "FIN {fin} is below FOUT {fout}; FOUT is set to {fin} so that no variable can enter and leave in turn"
                ));
            }
            _ => {}
        }

        // the values, NaN where missing; case weights and REGWGT, 0 where the case is unusable
        let cases = data.case_count();
        let weights = case_weights(data)?;
        let regwgt = match &command.regwgt {
            Some(name) => {
                let v = data.variable(name)?;
                if v.is_string() {
                    return Err(Error(format!(
                        "REGWGT needs a numeric variable, but {name} is a string variable"
                    )));
                }
                Some(v)
            }
            None => None,
        };
        let mut c: Vec<f64> = (0..cases)
            .map(|k| weights.as_ref().map_or(1.0, |w| w[k]))
            .collect();
        let g: Vec<f64> = (0..cases)
            .map(|k| match &regwgt {
                Some(v) if is_valid(v, k, false) => match v.values() {
                    Values::Numeric(values) => values.get(k).filter(|g| *g > 0.0).unwrap_or(0.0),
                    Values::String(_) => 0.0,
                },
                Some(_) => 0.0,
                None => 1.0,
            })
            .collect();
        for k in 0..cases {
            if g[k] <= 0.0 {
                c[k] = 0.0;
            }
        }
        let value = |v: &Variable, k: usize| -> f64 {
            if !is_valid(v, k, command.include) {
                return f64::NAN;
            }
            match v.values() {
                Values::Numeric(values) => values.get(k).unwrap_or(f64::NAN),
                Values::String(_) => f64::NAN,
            }
        };
        let raw: Vec<Vec<f64>> = variables
            .iter()
            .map(|v| {
                (0..cases)
                    .map(|k| if c[k] > 0.0 { value(v, k) } else { f64::NAN })
                    .collect()
            })
            .collect();
        let complete: Vec<bool> = (0..cases)
            .map(|k| c[k] > 0.0 && raw.iter().all(|col| !col[k].is_nan()))
            .collect();
        // the columns the matrix is built from
        let columns: Vec<Vec<f64>> = match command.missing {
            RegressionMissing::Listwise => raw
                .iter()
                .map(|col| {
                    col.iter()
                        .zip(&complete)
                        .map(|(v, ok)| if *ok { *v } else { f64::NAN })
                        .collect()
                })
                .collect(),
            RegressionMissing::Pairwise => raw.clone(),
            RegressionMissing::MeanSubstitution => raw
                .iter()
                .map(|col| {
                    let (mut sum, mut w) = (0.0, 0.0);
                    for k in 0..cases {
                        if !col[k].is_nan() {
                            sum += c[k] * g[k] * col[k];
                            w += c[k] * g[k];
                        }
                    }
                    let mean = sum / w;
                    (0..cases)
                        .map(|k| {
                            if c[k] > 0.0 && col[k].is_nan() {
                                mean
                            } else {
                                col[k]
                            }
                        })
                        .collect()
                })
                .collect(),
        };
        let mo = moments(&columns, &c, &g, command.origin);
        let mut tables = Vec::new();
        if let Some(d) = command.descriptives {
            // the descriptive statistics use the cases valid on each variable unless LISTWISE
            let own = (command.missing == RegressionMissing::MeanSubstitution)
                .then(|| moments(&raw, &c, &g, command.origin));
            let texts: Vec<String> = variables.iter().map(|v| shown.variable(v)).collect();
            tables.extend(descriptive_tables(
                &texts,
                own.as_ref().unwrap_or(&mo),
                &mo,
                d,
            ));
        }
        let source = if command.missing == RegressionMissing::MeanSubstitution {
            &columns
        } else {
            &raw
        };
        let mut per_case: Vec<PerCase> = Vec::new();
        for dependent in &dependents {
            let yi = index(dependent);
            // candidates: every variable named on a method, in matrix order, but the dependent
            let mut candidates: Vec<usize> = blocks
                .iter()
                .flat_map(|(_, list)| list.iter().copied())
                .filter(|&k| k != yi)
                .collect();
            candidates.sort_unstable();
            candidates.dedup();
            let order: Vec<usize> = candidates.iter().copied().chain([yi]).collect();
            let sub = |m: &Matrix| -> Matrix {
                order
                    .iter()
                    .map(|&i| order.iter().map(|&j| m[i][j]).collect())
                    .collect()
            };
            // with PAIRWISE the smallest count of a pair stands for C
            let pairs = order
                .iter()
                .flat_map(|&i| order.iter().map(move |&j| (i, j)));
            let eq = Equation {
                names: order.iter().map(|&i| names[i].clone()).collect(),
                texts: order
                    .iter()
                    .map(|&i| shown.variable(&variables[i]))
                    .collect(),
                r: sub(&mo.corr),
                var: order.iter().map(|&i| mo.cov[i][i]).collect(),
                mean: order.iter().map(|&i| mo.mean[i]).collect(),
                cov: sub(&mo.cov),
                c: pairs.map(|(i, j)| mo.c[i][j]).fold(f64::INFINITY, f64::min),
                w: order.iter().map(|&i| mo.w[i]).fold(f64::INFINITY, f64::min),
                origin: command.origin,
            };
            if eq.var[eq.y()].is_nan() || eq.var[eq.y()] <= 0.0 {
                warnings.push(format!(
                    "{dependent} has no variance in the cases of the analysis; it gets no equation"
                ));
                continue;
            }
            // the methods as indices into the equation
            let eq_blocks: Vec<(MethodKind, Vec<usize>)> = blocks
                .iter()
                .map(|(kind, list)| {
                    let list = list
                        .iter()
                        .filter_map(|k| candidates.iter().position(|c| c == k))
                        .collect();
                    (*kind, list)
                })
                .collect();
            let models = select(&eq, &eq_blocks, &criteria, &mut warnings);
            if models.is_empty() {
                warnings.push(format!("no variable entered the equation of {dependent}"));
                continue;
            }
            let fits: Vec<Fit> = models.iter().map(|m| fit(&eq, &m.set)).collect();
            let last = fits.last().expect("at least one model");
            let wants_cases = !command.save.is_empty()
                || command.residuals.is_some()
                || command.casewise.is_some()
                || !command.charts.is_empty();
            let in_fit: Vec<bool> = (0..cases)
                .map(|k| match command.missing {
                    RegressionMissing::Listwise => complete[k],
                    RegressionMissing::MeanSubstitution => c[k] > 0.0,
                    RegressionMissing::Pairwise => {
                        c[k] > 0.0
                            && !raw[yi][k].is_nan()
                            && last.b.iter().all(|b| !raw[order[b.0]][k].is_nan())
                    }
                })
                .collect();
            let rows = wants_cases.then(|| {
                let x = |j: usize, k: usize| source[order[j]][k];
                case_values(&eq, last, &x, &c, &g, command, &in_fit)
            });
            let dw = rows
                .as_ref()
                .and_then(|rows| durbin_watson(rows, &c, &g, &in_fit));
            tables.extend(equation_tables(command, &eq, &models, &fits, &criteria, dw));
            if let Some(rows) = rows {
                let coefficients = coefficient_names(&eq, last);
                tables.extend(self.residual_tables(
                    command,
                    &eq,
                    &rows,
                    &c,
                    &in_fit,
                    &coefficients,
                )?);
                per_case.push(PerCase {
                    dependent: dependent.clone(),
                    coefficients,
                    rows,
                });
            }
        }
        for warning in warnings {
            self.warn(warning);
        }
        if !command.charts.is_empty() {
            self.warn(format!(
                "REGRESSION: {} accepted; charts are not drawn yet",
                command.charts.join(", ")
            ));
        }
        if !command.save.is_empty() {
            self.save_regression(command, &per_case)?;
        }
        Ok(tables)
    }
}

fn equation_tables(
    command: &Regression,
    eq: &Equation,
    models: &[Model],
    fits: &[Fit],
    criteria: &RegressionCriteria,
    dw: Option<f64>,
) -> Vec<Table> {
    let s = &command.statistics;
    let y = eq.y();
    let dependent = &eq.texts[y];
    let caption = Some(format!("Dependent variable: {dependent}."));
    let model_cell = |i: usize| Cell::Number((i + 1) as f64);
    let mut out = Vec::new();
    let names = |list: &[usize]| -> Cell {
        if list.is_empty() {
            Cell::Empty
        } else {
            Cell::Text(
                list.iter()
                    .map(|&k| eq.texts[k].as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        }
    };
    out.push(table(
        "Variables Entered/Removed",
        1,
        plain(&["Model", "Variables Entered", "Variables Removed", "Method"]),
        models
            .iter()
            .enumerate()
            .map(|(i, m)| {
                vec![
                    model_cell(i),
                    names(&m.entered),
                    names(&m.removed),
                    text(&m.method),
                ]
            })
            .collect(),
        caption.clone(),
    ));
    let predictors: Vec<String> = models
        .iter()
        .enumerate()
        .map(|(i, m)| {
            let mut list: Vec<&str> = Vec::new();
            if !eq.origin {
                list.push("(Constant)");
            }
            list.extend((0..y).filter(|&j| m.set[j]).map(|j| eq.texts[j].as_str()));
            format!("model {}: {}", i + 1, list.join(", "))
        })
        .collect();
    let predictors_caption = Some(format!(
        "Predictors: {}. Dependent variable: {dependent}.",
        predictors.join("; ")
    ));

    // Model Summary
    let durbin = command.residuals.as_ref().is_some_and(|r| r.durbin);
    if s.r || s.change || s.selection || durbin {
        let mut columns = plain(&["Model"]);
        if s.r {
            columns.extend(plain(&[
                "R",
                "R Square",
                "Adjusted R Square",
                "Std. Error of the Estimate",
            ]));
        }
        if s.change {
            for c in ["R Square Change", "F Change", "df1", "df2", "Sig. F Change"] {
                columns.push(("Change Statistics".into(), c.into()));
            }
        }
        if durbin {
            columns.extend(plain(&["Durbin-Watson"]));
        }
        if s.selection {
            for c in [
                "Akaike Information Criterion",
                "Amemiya Prediction Criterion",
                "Mallows' Prediction Criterion",
                "Schwarz Bayesian Criterion",
            ] {
                columns.push(("Selection Criteria".into(), c.into()));
            }
        }
        // Mallows' criterion compares with the equation of every candidate
        let full_mse = {
            let mut set = vec![false; y];
            for k in 0..y {
                let m = eq.swept(&set);
                if eq.may_enter(&m, &set, k, criteria.tolerance) {
                    set[k] = true;
                }
            }
            fit(eq, &set).mse
        };
        let rows = fits
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let mut row = vec![model_cell(i)];
                if s.r {
                    let adj = f.r2 - (1.0 - f.r2) * f.p as f64 / f.df_res;
                    row.extend([
                        number(Some(f.r2.max(0.0).sqrt())),
                        number(Some(f.r2)),
                        number(Some(adj)),
                        number(Some(f.mse.sqrt())),
                    ]);
                }
                if s.change {
                    let previous = if i == 0 { 0.0 } else { fits[i - 1].r2 };
                    let change = f.r2 - previous;
                    let (q_in, q_out) = (models[i].entered.len(), models[i].removed.len());
                    let (fc, df1, df2) = if q_out > 0 && q_in == 0 {
                        let q = q_out as f64;
                        (
                            change * (eq.c - f.k - q) / (q * (previous - 1.0)),
                            q,
                            eq.c - f.k - q,
                        )
                    } else {
                        let q = if i == 0 { f.p } else { q_in }.max(1) as f64;
                        (change * f.df_res / (q * (1.0 - f.r2)), q, f.df_res)
                    };
                    row.extend([
                        number(Some(change)),
                        number(Some(fc)),
                        number(Some(df1)),
                        number(Some(df2)),
                        number(Some(f_p(fc, df1, df2))),
                    ]);
                }
                if durbin {
                    row.push(if i + 1 == fits.len() {
                        number(dw)
                    } else {
                        Cell::Empty
                    });
                }
                if s.selection {
                    let ln = (f.ss_res / eq.c).ln();
                    row.extend([
                        number(Some(eq.c * ln + 2.0 * f.k)),
                        number(Some((1.0 - f.r2) * (eq.c + f.k) / (eq.c - f.k))),
                        number(Some(f.ss_res / full_mse + 2.0 * f.k - eq.c)),
                        number(Some(eq.c * ln + f.k * eq.c.ln())),
                    ]);
                }
                row
            })
            .collect();
        out.push(table(
            "Model Summary",
            1,
            columns,
            rows,
            predictors_caption.clone(),
        ));
    }

    if s.anova {
        let mut rows = Vec::new();
        for (i, f) in fits.iter().enumerate() {
            let df_reg = f.p as f64;
            let ms_reg = f.ss_reg / df_reg;
            let fv = ms_reg / f.mse;
            let df_total = eq.c - f64::from(u8::from(!eq.origin));
            rows.push(vec![
                model_cell(i),
                text("Regression"),
                number(Some(f.ss_reg)),
                number(Some(df_reg)),
                number(Some(ms_reg)),
                number(Some(fv)),
                number(Some(f_p(fv, df_reg, f.df_res))),
            ]);
            rows.push(vec![
                Cell::Empty,
                text("Residual"),
                number(Some(f.ss_res)),
                number(Some(f.df_res)),
                number(Some(f.mse)),
                Cell::Empty,
                Cell::Empty,
            ]);
            rows.push(vec![
                Cell::Empty,
                text("Total"),
                number(Some(f.ss_reg + f.ss_res)),
                number(Some(df_total)),
                Cell::Empty,
                Cell::Empty,
                Cell::Empty,
            ]);
        }
        out.push(table(
            "ANOVA",
            2,
            plain(&[
                "Model",
                "",
                "Sum of Squares",
                "df",
                "Mean Square",
                "F",
                "Sig.",
            ]),
            rows,
            predictors_caption.clone(),
        ));
    }

    let test_name = if s.f { "F" } else { "t" };
    if s.coeff || s.ci.is_some() || s.zpp || s.tol || s.collin || s.ses {
        let mut columns = plain(&["Model", ""]);
        if s.coeff {
            columns.push(("Unstandardized Coefficients".into(), "B".into()));
            columns.push(("Unstandardized Coefficients".into(), "Std. Error".into()));
            columns.push(("Standardized Coefficients".into(), "Beta".into()));
        }
        if s.ses {
            columns.push(("Standardized Coefficients".into(), "Std. Error".into()));
        }
        if s.coeff {
            columns.extend(plain(&[test_name, "Sig."]));
        }
        let ci_group =
            s.ci.map(|level| format!("{level:.1}% Confidence Interval for B"));
        if let Some(group) = &ci_group {
            columns.push((group.clone(), "Lower Bound".into()));
            columns.push((group.clone(), "Upper Bound".into()));
        }
        if s.zpp {
            for c in ["Zero-order", "Partial", "Part"] {
                columns.push(("Correlations".into(), c.into()));
            }
        }
        if s.tol {
            columns.push(("Collinearity Statistics".into(), "Tolerance".into()));
        }
        if s.collin {
            columns.push(("Collinearity Statistics".into(), "VIF".into()));
        }
        let mut rows = Vec::new();
        for (i, f) in fits.iter().enumerate() {
            let m = &f.swept;
            let q =
                s.ci.map(|level| t_quantile((1.0 - level / 100.0) / 2.0, f.df_res));
            let test = |b: f64, se: f64| {
                let t = b / se;
                let sig = t_p(t, f.df_res);
                (if s.f { t * t } else { t }, sig)
            };
            let mut first = true;
            let mut push = |name: String, b: f64, se: f64, beta: Option<(f64, usize)>| {
                let mut row = vec![
                    if first { model_cell(i) } else { Cell::Empty },
                    Cell::Text(name),
                ];
                first = false;
                let (tv, sig) = test(b, se);
                if s.coeff {
                    row.extend([number(Some(b)), number(Some(se)), number(beta.map(|b| b.0))]);
                }
                if s.ses {
                    row.push(number(
                        beta.map(|(_, j)| (m[y][y] * m[j][j] / f.df_res).sqrt()),
                    ));
                }
                if s.coeff {
                    row.extend([number(Some(tv)), number(Some(sig))]);
                }
                if let Some(q) = q {
                    row.extend([number(Some(b - q * se)), number(Some(b + q * se))]);
                }
                if s.zpp {
                    match beta {
                        Some((beta, j)) => row.extend([
                            number(Some(eq.r[y][j])),
                            number(Some(beta / (m[j][j] * m[y][y] + beta * beta).sqrt())),
                            number(Some(beta / m[j][j].sqrt())),
                        ]),
                        None => row.extend([Cell::Empty, Cell::Empty, Cell::Empty]),
                    }
                }
                if s.tol {
                    row.push(number(beta.map(|(_, j)| 1.0 / m[j][j])));
                }
                if s.collin {
                    row.push(number(beta.map(|(_, j)| m[j][j])));
                }
                rows.push(row);
            };
            if let Some((b0, se0)) = f.constant {
                push("(Constant)".into(), b0, se0, None);
            }
            for &(j, b, se) in &f.b {
                push(eq.texts[j].clone(), b, se, Some((m[y][j], j)));
            }
        }
        out.push(table("Coefficients", 2, columns, rows, caption.clone()));
    }

    if s.outs {
        let mut columns = plain(&[
            "Model",
            "",
            "Beta In",
            test_name,
            "Sig.",
            "Partial Correlation",
        ]);
        if s.tol {
            columns.push(("Collinearity Statistics".into(), "Tolerance".into()));
        }
        if s.collin {
            columns.push(("Collinearity Statistics".into(), "VIF".into()));
        }
        columns.push(("Collinearity Statistics".into(), "Minimum Tolerance".into()));
        let mut rows = Vec::new();
        for (i, (model, f)) in models.iter().zip(fits).enumerate() {
            let m = &f.swept;
            let mut first = true;
            for k in (0..y).filter(|&k| !model.set[k]) {
                let r_kk = m[k][k];
                let usable = r_kk > 1e-12;
                let beta = usable.then(|| m[y][k] / r_kk);
                let df = eq.c - f.k - 1.0;
                let fv = df * m[y][k] * m[y][k] / (r_kk * m[y][y] - m[y][k] * m[y][k]);
                let t = fv.sqrt() * m[y][k].signum();
                let min_tol = (0..y)
                    .filter(|&j| model.set[j])
                    .map(|j| 1.0 / (m[j][j] - m[j][k] * m[k][j] / r_kk))
                    .fold(r_kk, f64::min);
                let mut row = vec![
                    if first { model_cell(i) } else { Cell::Empty },
                    Cell::Text(eq.texts[k].clone()),
                    number(beta),
                    number(usable.then_some(if s.f { fv } else { t })),
                    number(usable.then(|| t_p(t, df))),
                    number(usable.then(|| m[y][k] / (m[y][y] * r_kk).sqrt())),
                ];
                first = false;
                if s.tol {
                    row.push(number(Some(r_kk)));
                }
                if s.collin {
                    row.push(number(usable.then(|| 1.0 / r_kk)));
                }
                row.push(number(usable.then_some(min_tol)));
                rows.push(row);
            }
        }
        if !rows.is_empty() {
            out.push(table(
                "Excluded Variables",
                2,
                columns,
                rows,
                predictors_caption.clone(),
            ));
        }
    }

    let used: Vec<usize> = (0..y)
        .filter(|&j| models.iter().any(|m| m.set[j]))
        .collect();
    if s.bcov {
        let mut columns = plain(&["Model", "", ""]);
        columns.extend(used.iter().map(|&j| (String::new(), eq.texts[j].clone())));
        let mut rows = Vec::new();
        for (i, (model, f)) in models.iter().zip(fits).enumerate() {
            let in_: Vec<usize> = (0..y).filter(|&j| model.set[j]).collect();
            let mut first = true;
            for (block, correlation) in [("Correlations", true), ("Covariances", false)] {
                for (n, &a) in in_.iter().enumerate() {
                    let mut row = vec![
                        if first { model_cell(i) } else { Cell::Empty },
                        if n == 0 { text(block) } else { Cell::Empty },
                        Cell::Text(eq.texts[a].clone()),
                    ];
                    first = false;
                    for &c in &used {
                        row.push(if !model.set[c] {
                            Cell::Empty
                        } else if correlation {
                            let m = &f.swept;
                            number(Some(m[a][c] / (m[a][a] * m[c][c]).sqrt()))
                        } else {
                            number(Some(f.cov(eq, a, c)))
                        });
                    }
                    rows.push(row);
                }
            }
        }
        out.push(table(
            "Coefficient Correlations",
            3,
            columns,
            rows,
            caption.clone(),
        ));
    }

    if s.collin {
        let mut columns = plain(&["Model", "Dimension", "Eigenvalue", "Condition Index"]);
        if !eq.origin {
            columns.push(("Variance Proportions".into(), "(Constant)".into()));
        }
        columns.extend(
            used.iter()
                .map(|&j| ("Variance Proportions".into(), eq.texts[j].clone())),
        );
        let mut rows = Vec::new();
        for (i, model) in models.iter().enumerate() {
            let in_: Vec<usize> = (0..y).filter(|&j| model.set[j]).collect();
            // the uncentered cross-products of the constant and the variables, scaled to
            // unit diagonal
            let terms: Vec<Option<usize>> = (!eq.origin)
                .then_some(None)
                .into_iter()
                .chain(in_.iter().map(|&j| Some(j)))
                .collect();
            let raw: Matrix = terms
                .iter()
                .map(|&a| terms.iter().map(|&b| eq.cross(a, b)).collect())
                .collect();
            let scaled: Matrix = (0..terms.len())
                .map(|a| {
                    (0..terms.len())
                        .map(|b| raw[a][b] / (raw[a][a] * raw[b][b]).sqrt())
                        .collect()
                })
                .collect();
            let pairs = symmetric_eigen(&scaled);
            let largest = pairs.first().map_or(f64::NAN, |p| p.0);
            let phi: Vec<Vec<f64>> = pairs
                .iter()
                .map(|(lambda, v)| v.iter().map(|x| x * x / lambda).collect())
                .collect();
            let totals: Vec<f64> = (0..terms.len())
                .map(|t| phi.iter().map(|row| row[t]).sum())
                .collect();
            for (d, (lambda, _)) in pairs.iter().enumerate() {
                let mut row = vec![
                    if d == 0 { model_cell(i) } else { Cell::Empty },
                    Cell::Number((d + 1) as f64),
                    number(Some(*lambda)),
                    // Belsley's condition index, the root of the ratio
                    number(Some((largest / lambda).sqrt())),
                ];
                let proportion = |t: usize| number(Some(phi[d][t] / totals[t]));
                if !eq.origin {
                    row.push(proportion(0));
                }
                for &j in &used {
                    row.push(match in_.iter().position(|&k| k == j) {
                        Some(at) => proportion(at + usize::from(!eq.origin)),
                        None => Cell::Empty,
                    });
                }
                rows.push(row);
            }
        }
        out.push(table(
            "Collinearity Diagnostics",
            2,
            columns,
            rows,
            caption.clone(),
        ));
    }

    if s.xtx {
        let last = fits.last().expect("a model");
        let mut columns = plain(&[""]);
        columns.extend(eq.texts.iter().map(|n| (String::new(), n.clone())));
        let rows = eq
            .texts
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let mut row = vec![Cell::Text(name.clone())];
                row.extend(last.swept[i].iter().map(|v| number(Some(*v))));
                row
            })
            .collect();
        out.push(table(
                "Swept Correlation Matrix",
                1,
                columns,
                rows,
                Some(format!(
                    "The correlation matrix swept on the variables of model {}. Dependent variable: {dependent}.",
                    fits.len()
                )),
            ));
    }
    out
}

/// The temporary variables of one dependent's final equation, case by case.
struct PerCase {
    dependent: String,
    /// The names of the coefficients, the constant ("Intercept") first, for DFBETA and SDBETA.
    coefficients: Vec<String>,
    rows: Vec<Option<CaseValues>>,
}

fn coefficient_names(eq: &Equation, fit: &Fit) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    if !eq.origin {
        names.push("Intercept".into());
    }
    names.extend(fit.b.iter().map(|b| eq.names[b.0].clone()));
    names
}

/// The temporary variables of every case; None for a case without valid values of the
/// variables of the equation. `x(j, k)` is the value of equation variable j in case k.
fn case_values(
    eq: &Equation,
    fit: &Fit,
    x: &dyn Fn(usize, usize) -> f64,
    c: &[f64],
    g: &[f64],
    command: &Regression,
    in_fit: &[bool],
) -> Vec<Option<CaseValues>> {
    let y = eq.y();
    let m = &fit.swept;
    let vars: Vec<usize> = fit.b.iter().map(|b| b.0).collect();
    let k_coef = fit.k;
    let s2 = fit.mse;
    let s = s2.sqrt();
    let weighted = command.regwgt.is_some();
    let origin = eq.origin;
    let level = command.criteria.cin;
    let t = t_quantile((1.0 - level / 100.0) / 2.0, fit.df_res);
    // (X'WX)^-1 of the constant and the variables, for DFBETA and SDBETA
    let terms: Vec<Option<usize>> = (!origin)
        .then_some(None)
        .into_iter()
        .chain(vars.iter().map(|&j| Some(j)))
        .collect();
    let xtwx: Matrix = terms
        .iter()
        .map(|&a| terms.iter().map(|&b| eq.cross(a, b)).collect())
        .collect();
    let xtwx_inv = inverse(&xtwx).map(|(inv, _)| inv);
    let cases = c.len();
    let predictors = |k: usize| -> Option<Vec<f64>> {
        let values: Vec<f64> = vars.iter().map(|&j| x(j, k)).collect();
        values.iter().all(|v| !v.is_nan()).then_some(values)
    };
    let predict = |values: &[f64]| -> f64 {
        fit.constant.map_or(0.0, |c| c.0)
            + fit.b.iter().zip(values).map(|(b, v)| b.1 * v).sum::<f64>()
    };
    // the spread of the predicted values over the cases of the analysis, for ZPRED
    let (mut sum, mut n) = (0.0, 0.0);
    for k in (0..cases).filter(|&k| in_fit[k]) {
        if let Some(values) = predictors(k) {
            let d = predict(&values) - eq.mean[y];
            sum += c[k] * d * d;
            n += c[k];
        }
    }
    let sd_pred = (sum / (n - 1.0)).sqrt();
    (0..cases)
        .map(|k| {
            if c[k] <= 0.0 {
                return None;
            }
            let values = predictors(k)?;
            let gk = g[k];
            let mut out = CaseValues::empty(terms.len());
            let pred = predict(&values);
            let d: Vec<f64> = vars
                .iter()
                .zip(&values)
                .map(|(&j, v)| if origin { *v } else { v - eq.mean[j] })
                .collect();
            let mut h = 0.0;
            for (a, &ja) in vars.iter().enumerate() {
                for (b, &jb) in vars.iter().enumerate() {
                    h += d[a] * d[b] * m[ja][jb] / (eq.var[ja] * eq.var[jb]).sqrt();
                }
            }
            h *= gk / (eq.c - 1.0);
            let ht = if origin { h } else { h + gk / eq.w };
            out.pred = pred;
            out.zpred = if weighted {
                f64::NAN
            } else {
                (pred - eq.mean[y]) / sd_pred
            };
            out.lever = h;
            out.mahal = if origin { eq.c * h } else { (eq.c - 1.0) * h };
            out.sepred = s * (ht / gk).sqrt();
            out.mcin = (pred - t * out.sepred, pred + t * out.sepred);
            let half = t * s * ((ht + 1.0) / gk).sqrt();
            out.icin = (pred - half, pred + half);
            let yv = x(y, k);
            if yv.is_nan() {
                return Some(out);
            }
            let e = yv - pred;
            out.resid = e;
            out.zresid = if weighted { f64::NAN } else { e / s };
            if in_fit[k] {
                out.sresid = e / (s * ((1.0 - ht) / gk).sqrt());
                out.dresid = e / (1.0 - ht);
                let star = (((eq.c - k_coef) * s2 / (1.0 - ht) - out.dresid * out.dresid)
                    / (eq.c - k_coef - 1.0))
                    .sqrt();
                out.sdresid = out.dresid / star;
                out.adjpred = yv - out.dresid;
                out.cook = out.dresid * out.dresid * ht * gk / (s2 * k_coef);
                if c[k] >= 1.0 {
                    let s_i = star * (1.0 - ht).sqrt();
                    out.dffit = ht * e / (1.0 - ht);
                    out.sdfit = out.dffit / (s_i * ht.sqrt());
                    out.covratio = (s_i / s).powf(2.0 * k_coef) / (1.0 - ht);
                    if let Some(inv) = &xtwx_inv {
                        let xt: Vec<f64> = std::iter::repeat_n(1.0, usize::from(!origin))
                            .chain(values.iter().copied())
                            .collect();
                        for (a, row) in inv.iter().enumerate() {
                            let dot: f64 = row.iter().zip(&xt).map(|(i, x)| i * x).sum();
                            let dfb = gk * e * dot / (1.0 - ht);
                            out.dfbeta[a] = dfb;
                            out.sdbeta[a] = dfb / (s_i * row[a].sqrt());
                        }
                    }
                }
            } else {
                let scale = s * ((1.0 + ht) / gk).sqrt();
                out.sresid = e / scale;
                out.dresid = e;
                out.sdresid = e / scale;
                out.adjpred = yv - e;
            }
            Some(out)
        })
        .collect()
}

/// The Durbin-Watson statistic of the residuals of the cases of the analysis in case order;
/// None with fractional case weights.
fn durbin_watson(
    rows: &[Option<CaseValues>],
    c: &[f64],
    g: &[f64],
    in_fit: &[bool],
) -> Option<f64> {
    let mut previous: Option<f64> = None;
    let (mut num, mut den) = (0.0, 0.0);
    for (k, row) in rows.iter().enumerate() {
        let Some(row) = row.as_ref().filter(|r| in_fit[k] && !r.resid.is_nan()) else {
            continue;
        };
        if c[k].fract() != 0.0 {
            return None;
        }
        let e = row.resid * g[k].sqrt();
        if let Some(p) = previous {
            num += (e - p) * (e - p);
        }
        den += c[k] * e * e;
        previous = Some(e);
    }
    Some(num / den)
}

/// The columns of a temporary variable in a table: one per coefficient for DFBETA and SDBETA,
/// a lower and an upper bound for MCIN and ICIN.
fn temp_columns(var: TempVar, coefficients: &[String]) -> Vec<(String, TempVar, usize)> {
    let label = short_label(var);
    match var {
        TempVar::Dfbeta | TempVar::Sdbeta => coefficients
            .iter()
            .enumerate()
            .map(|(i, name)| (format!("{label} {name}"), var, i))
            .collect(),
        TempVar::Mcin | TempVar::Icin => vec![
            (format!("{label} Lower Bound"), var, 0),
            (format!("{label} Upper Bound"), var, 1),
        ],
        _ => vec![(label.to_string(), var, 0)],
    }
}

fn temp_value(row: &CaseValues, var: TempVar, at: usize) -> f64 {
    match var {
        TempVar::Dfbeta => row.dfbeta.get(at).copied().unwrap_or(f64::NAN),
        TempVar::Sdbeta => row.sdbeta.get(at).copied().unwrap_or(f64::NAN),
        TempVar::Mcin => [row.mcin.0, row.mcin.1][at],
        TempVar::Icin => [row.icin.0, row.icin.1][at],
        _ => row.get(var),
    }
}

impl Engine {
    fn residual_tables(
        &self,
        command: &Regression,
        eq: &Equation,
        rows: &[Option<CaseValues>],
        c: &[f64],
        in_fit: &[bool],
        coefficients: &[String],
    ) -> Result<Vec<Table>, Error> {
        use TempVar as T;
        let data = self.active()?;
        let dependent = &eq.texts[eq.y()];
        let caption = Some(format!("Dependent variable: {dependent}."));
        let number_of = |k: usize| self.case_numbers.as_ref().map_or(k, |n| n[k]) + 1;
        let id = command.residuals.as_ref().and_then(|r| r.id.as_ref());
        let id_variable = id.map(|name| data.variable(name)).transpose()?;
        let id_cell = |k: usize| match id_variable.as_ref().map(|v| v.values()) {
            Some(Values::Numeric(values)) => number(values.get(k)),
            Some(Values::String(values)) => Cell::Text(values.get(k).trim_end().to_string()),
            None => Cell::Empty,
        };
        let mut referred: Vec<TempVar> = command.save.iter().map(|s| s.0).collect();
        if let Some(cw) = &command.casewise {
            referred.push(cw.plot);
            referred.extend(&cw.listed);
        }
        if let Some(r) = &command.residuals {
            referred.extend(&r.outliers);
        }
        let mut out = Vec::new();
        let fitted: Vec<(usize, &CaseValues)> = rows
            .iter()
            .enumerate()
            .filter(|(k, _)| in_fit[*k])
            .filter_map(|(k, r)| r.as_ref().map(|r| (k, r)))
            .collect();
        let shown = [
            (T::Pred, true),
            (T::Zpred, true),
            (T::Sepred, referred.contains(&T::Sepred)),
            (T::AdjPred, referred.contains(&T::AdjPred)),
            (T::Resid, true),
            (T::Zresid, true),
            (T::Sresid, referred.contains(&T::Sresid)),
            (T::Dresid, referred.contains(&T::Dresid)),
            (T::Sdresid, referred.contains(&T::Sdresid)),
            (T::Mahal, referred.contains(&T::Mahal)),
            (T::Cook, referred.contains(&T::Cook)),
            (T::Lever, referred.contains(&T::Lever)),
        ];
        let mut stats_rows = Vec::new();
        for (var, _) in shown.iter().filter(|s| s.1) {
            let values: Vec<(f64, f64)> =
                fitted.iter().map(|(k, r)| (r.get(*var), c[*k])).collect();
            let s = summary(&values);
            if s[4] == Some(0.0) {
                continue;
            }
            let mut row = vec![text(short_label(*var))];
            row.extend(s.iter().map(|v| number(*v)));
            stats_rows.push(row);
        }
        out.push(table(
            "Residuals Statistics",
            1,
            plain(&["", "Minimum", "Maximum", "Mean", "Std. Deviation", "N"]),
            stats_rows,
            caption.clone(),
        ));
        if let Some(cw) = &command.casewise {
            // RESID is shown standardized and DRESID studentized
            let plot = match cw.plot {
                T::Resid => T::Zresid,
                T::Dresid => T::Sdresid,
                other => other,
            };
            let mut columns = plain(&["Case Number"]);
            if let Some(id) = id {
                columns.extend(plain(&[id.as_str()]));
            }
            columns.extend(plain(&[short_label(plot)]));
            if cw.dependent {
                columns.extend(plain(&[dependent.as_str()]));
            }
            let listed: Vec<(String, TempVar, usize)> = cw
                .listed
                .iter()
                .flat_map(|&v| temp_columns(v, coefficients))
                .collect();
            columns.extend(listed.iter().map(|(l, _, _)| (String::new(), l.clone())));
            let mut table_rows = Vec::new();
            for &(k, row) in &fitted {
                let value = row.get(plot);
                if value.is_nan() || !(cw.all || value.abs() >= cw.limit) {
                    continue;
                }
                let mut cells = vec![Cell::Number(number_of(k) as f64)];
                if id.is_some() {
                    cells.push(id_cell(k));
                }
                cells.push(number(Some(value)));
                if cw.dependent {
                    cells.push(number(Some(row.pred + row.resid)));
                }
                cells.extend(
                    listed
                        .iter()
                        .map(|(_, v, at)| number(Some(temp_value(row, *v, *at)))),
                );
                table_rows.push(cells);
            }
            let caption = if cw.all {
                caption.clone()
            } else {
                Some(format!(
                    "Cases whose {} is at least {} in absolute value. Dependent variable: {dependent}.",
                    short_label(plot),
                    cw.limit
                ))
            };
            out.push(table(
                "Casewise Diagnostics",
                1,
                columns,
                table_rows,
                caption,
            ));
        }
        if let Some(r) = command
            .residuals
            .as_ref()
            .filter(|r| !r.outliers.is_empty())
        {
            let mut columns = plain(&["", "", "Case Number"]);
            if let Some(id) = id {
                columns.extend(plain(&[id.as_str()]));
            }
            columns.extend(plain(&["Statistic"]));
            let mut table_rows = Vec::new();
            for &var in &r.outliers {
                let mut ranked: Vec<(usize, f64)> = fitted
                    .iter()
                    .map(|(k, row)| (*k, row.get(var)))
                    .filter(|(_, v)| !v.is_nan())
                    .collect();
                ranked.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()).then(a.0.cmp(&b.0)));
                for (rank, (k, value)) in ranked.into_iter().take(10).enumerate() {
                    let mut cells = vec![
                        if rank == 0 {
                            text(short_label(var))
                        } else {
                            Cell::Empty
                        },
                        Cell::Number((rank + 1) as f64),
                        Cell::Number(number_of(k) as f64),
                    ];
                    if id.is_some() {
                        cells.push(id_cell(k));
                    }
                    cells.push(number(Some(value)));
                    table_rows.push(cells);
                }
            }
            out.push(table(
                "Outlier Statistics",
                2,
                columns,
                table_rows,
                caption.clone(),
            ));
        }
        Ok(out)
    }

    /// SAVE: adds the temporary variables of each dependent's final equation.
    fn save_regression(&mut self, command: &Regression, per_case: &[PerCase]) -> Result<(), Error> {
        let level = command.criteria.cin;
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            let cases = data.case_count();
            for equation in per_case {
                let coefficients = &equation.coefficients;
                // DFBETA numbers the coefficients from 0, the constant; without it from 1
                let first = usize::from(coefficients.first().is_none_or(|c| c != "Intercept"));
                for (var, given) in &command.save {
                    let (_, root, label) = *label_of(*var);
                    let parts: Vec<(String, String, usize)> = match var {
                        TempVar::Dfbeta | TempVar::Sdbeta => coefficients
                            .iter()
                            .enumerate()
                            .map(|(i, name)| (format!("{}", i + first), format!("{label} {name}"), i))
                            .collect(),
                        TempVar::Mcin | TempVar::Icin => {
                            let (kind, letters) = if *var == TempVar::Mcin {
                                ("Mean", "MCI")
                            } else {
                                ("Individual", "ICI")
                            };
                            let dependent = &equation.dependent;
                            ["Lower", "Upper"]
                                .iter()
                                .enumerate()
                                .map(|(at, bound)| {
                                    (
                                        format!("{}{letters}", &bound[..1]),
                                        format!(
                                            "{level}% {kind} Confidence Interval {bound} Bound for {dependent}"
                                        ),
                                        at,
                                    )
                                })
                                .collect()
                        }
                        _ => vec![(String::new(), label.to_string(), 0)],
                    };
                    let names: Vec<String> = match (given, var) {
                        (Some(name), TempVar::Dfbeta | TempVar::Sdbeta) => parts
                            .iter()
                            .map(|(suffix, _, _)| format!("{name}{suffix}"))
                            .collect(),
                        (Some(name), _) => vec![name.clone()],
                        (None, _) => {
                            let make = |n: usize| -> Vec<String> {
                                parts
                                    .iter()
                                    .map(|(part, _, _)| match var {
                                        TempVar::Mcin | TempVar::Icin => format!("{part}_{n}"),
                                        _ => format!("{root}{part}_{n}"),
                                    })
                                    .collect()
                            };
                            let mut n = 1;
                            while make(n).iter().any(|name| data.variable_index(name).is_some()) {
                                n += 1;
                            }
                            make(n)
                        }
                    };
                    for (name, (_, label, at)) in names.iter().zip(&parts) {
                        if data.variable_index(name).is_some() {
                            return Err(Error(format!(
                                "REGRESSION /SAVE: variable {name} already exists"
                            )));
                        }
                        let values: Vec<Option<f64>> = (0..cases)
                            .map(|k| {
                                equation.rows[k]
                                    .as_ref()
                                    .map(|row| temp_value(row, *var, *at))
                                    .filter(|v| v.is_finite())
                            })
                            .collect();
                        data.set_numeric_values(name, values)?;
                        data.set_variable_label(name, Some(label))?;
                        data.set_format(name, "F11.5")?;
                        data.set_measure(name, Measure::Scale)?;
                    }
                }
            }
            Ok(())
        })
    }
}

/// The Descriptive Statistics and Correlations tables of DESCRIPTIVES: `own` holds the moments
/// of each variable on its own cases, `used` the matrix of the analysis.
fn descriptive_tables(
    names: &[String],
    own: &Moments,
    used: &Moments,
    d: RegressionDescriptives,
) -> Vec<Table> {
    let mut out = Vec::new();
    if d.mean || d.stddev || d.variance {
        let mut columns = plain(&[""]);
        let picked: Vec<&str> = [
            ("Mean", d.mean),
            ("Std. Deviation", d.stddev),
            ("Variance", d.variance),
        ]
        .iter()
        .filter(|c| c.1)
        .map(|c| c.0)
        .collect();
        columns.extend(plain(&picked));
        columns.extend(plain(&["N"]));
        let rows = names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let mut row = vec![Cell::Text(name.clone())];
                if d.mean {
                    row.push(number(Some(own.mean[i])));
                }
                if d.stddev {
                    row.push(number(Some(own.cov[i][i].sqrt())));
                }
                if d.variance {
                    row.push(number(Some(own.cov[i][i])));
                }
                row.push(number(Some(own.c[i][i])));
                row
            })
            .collect();
        out.push(table("Descriptive Statistics", 1, columns, rows, None));
    }
    let bad = used.corr.iter().flatten().any(|r| !r.is_finite());
    let lines: Vec<&str> = [
        ("Pearson Correlation", d.corr || (d.badcorr && bad)),
        ("Sig. (1-tailed)", d.sig),
        ("Sum of Squares and Cross-products", d.xprod),
        ("Covariance", d.cov),
        ("N", d.n),
    ]
    .iter()
    .filter(|l| l.1)
    .map(|l| l.0)
    .collect();
    if !lines.is_empty() {
        let mut columns = plain(&["", ""]);
        columns.extend(names.iter().map(|n| (String::new(), n.clone())));
        let mut rows = Vec::new();
        for (i, name) in names.iter().enumerate() {
            for (l, line) in lines.iter().enumerate() {
                let mut row = vec![
                    if l == 0 {
                        Cell::Text(name.clone())
                    } else {
                        Cell::Empty
                    },
                    text(*line),
                ];
                for j in 0..names.len() {
                    let (r, n) = (used.corr[i][j], used.c[i][j]);
                    row.push(match *line {
                        "Pearson Correlation" => number(Some(r)),
                        "Sig. (1-tailed)" if i != j => {
                            let t = r * ((n - 2.0) / (1.0 - r * r)).sqrt();
                            number(Some(t_p(t, n - 2.0) / 2.0))
                        }
                        "Sig. (1-tailed)" => Cell::Empty,
                        "Sum of Squares and Cross-products" => {
                            number(Some(used.cov[i][j] * (n - 1.0)))
                        }
                        "Covariance" => number(Some(used.cov[i][j])),
                        _ => number(Some(n)),
                    });
                }
                rows.push(row);
            }
        }
        out.push(table("Correlations", 2, columns, rows, None));
    }
    out
}
