//! GENLINMIXED: the generalized linear mixed model g(E(y | u)) = X b + Z u + offset, fitted on
//! the working linear mixed model of MIXED.
//!
//! Rules (our notes from the Command Syntax Reference and the Algorithms manual, GENLINMIXED):
//! - Estimation is restricted pseudo likelihood with the predicted random effects in the
//!   expansion. From means mu (start: (y m + 0.5) / (m + 1) for BINOMIAL with m trials, y for
//!   the others) the working target is v = g(mu) - offset + g'(mu) (y - mu) and the working
//!   weight w = prior / (V(mu) g'(mu)^2), the prior weight being the analysis weight times the
//!   trials. MIXED fits v with regression weights w by REML; then mu = g^-1(X b + Z u + offset)
//!   with u the predicted random effects, and again, until the largest relative change of a
//!   covariance parameter, 2 |new - old| / (|new| + |old|), falls below the tolerance.
//! - The residual covariance is phi W^-1: phi is 1 for BINOMIAL and POISSON and a covariance
//!   parameter (the residual variance) for NORMAL, GAMMA and INVERSE_GAUSSIAN.
//! - Every table comes from the final working model: tests, coefficients and their standard
//!   errors from (X'V^-1 X)^-1, covariance parameters with 2 H^-1, information criteria from
//!   its restricted log pseudo likelihood, degrees of freedom the cases less the rank of X
//!   unless DF_METHOD says otherwise.
//! - Effects: a numeric field of scale level is a covariate, any other a factor. A two-valued
//!   target is BINOMIAL: the event is the value that is not the reference (the last value in
//!   TARGET_CATEGORY_ORDER unless REFERENCE names one).
//! - REPEATED_MEASURES gives every subject of DATA_STRUCTURE a residual block over its repeated
//!   levels, R = A^1/2 R(θ) A^1/2: its parameters are estimated for every distribution, so the
//!   scale is no longer fixed at 1. NEGATIVE_BINOMIAL has V(mu) = mu + k mu^2, k estimated by
//!   maximum likelihood of the model without random or residual effects and then held fixed.
//! - COVB=ROBUST: C (sum over the independent subjects of X'V^-1 r r' V^-1 X) C with r = v - X b
//!   of the final working model; Kenward-Roger keeps the model-based C.
//! - EMMEANS: the means u = L b of the linear predictor, L as in MIXED (covariates at their mean
//!   or their CONTROL value), with variance L C L'; on the original scale M = g^-1(u) with
//!   variance D L C L' D, D = diag(1 / g'(M)), and the interval g^-1(u -+ t se(u)). A contrast c
//!   of the means of the compared factor (at each level of the other factors) has the estimate
//!   c'u or c'M, the variance of the same form, a t test and an F test of the whole set;
//!   PADJUST adjusts the t tests of one set and their intervals, step down from the smallest
//!   significance.
//! - SAVE from eta = x b + z u + offset with the variance of its prediction error: the mean
//!   g^-1(eta) (times the trials for events and trials; for a two-valued target the category
//!   whose probability is at least 0.5), the interval g^-1(eta -+ t se), and the Pearson
//!   residual (y - mu) / sqrt(V(mu) R_ii / prior weight).

use std::cell::Cell;

use super::{Eval, Fitted};
use super::{
    FixedPart, Model, Predict, RandomRow, What, ci_title, dot, fit_model, mat_vec, product,
};
use crate::contrast::contrast_rows;
use crate::crosstabs::{Key, categories};
use crate::descriptives::number;
use crate::genlin::{
    Family, distribution_name, link_d1, link_inverse, link_name, link_value, mu_valid, negbin_k,
    value_matches, variance,
};
use crate::glm::{combinations, covariate_note, emmeans_l};
use crate::matrix::Matrix;
use crate::means::is_valid;
use crate::npar_tests::text;
use crate::pass::default_measure;
use crate::regression::{plain, table};
use crate::{Cell as Out, Engine, Error, Table};
use serde_json::json;
use statscraft_data::{Measure, Values};
use statscraft_dist::{f_p, t_p, t_quantile};
use statscraft_syntax::{
    Contrast, CovType, DfMethod, GenLinMixed, GenlinAdjust, GenlinDistribution, GenlinLink,
    GenlinOffset, GenlinOrder, GenlinTrials, GlmmContrast, GlmmEmmeans, Mixed, MixedPrint,
    RandomEffects, Repeated, VarItem, default_link, link_allowed,
};

/// What GENLINMIXED adds to the linear mixed model.
pub(super) struct Glmm<'a> {
    pub(super) command: &'a GenLinMixed,
    family: Family,
    link: GenlinLink,
    /// The target of every case of the file (a proportion for events and trials); None leaves
    /// the case out.
    pub(super) y: Vec<Option<f64>>,
    trials: Vec<f64>,
    offset: Vec<f64>,
    /// The analysis weight of every case (1 without one).
    weight: Vec<f64>,
    /// The scale is 1 rather than a parameter.
    pub(super) unit_scale: bool,
    /// k of NEGATIVE_BINOMIAL, set by the fit.
    k: Cell<f64>,
    /// Whether the tests use the robust covariance, set by the fit.
    pub(super) robust_used: Cell<bool>,
    /// A target of two categories: its values in TARGET_CATEGORY_ORDER and which one is the
    /// event.
    categories: Option<(Vec<Key>, usize)>,
    target: String,
    target_text: String,
    /// Whether the target is a string, with its width.
    string_width: Option<usize>,
    target_format: String,
}

/// A field that SAVE adds.
pub(super) struct Saved {
    name: String,
    label: String,
    numbers: Vec<Option<f64>>,
    /// The values of a string field instead.
    texts: Option<Vec<String>>,
    /// The print format of a category of the target (else F11.5 and scale).
    format: Option<String>,
}

/// What GENLINMIXED adds after the fit: EMMEANS tables, SAVE fields and the model file.
pub(super) struct After {
    pub(super) tables: Vec<Table>,
    pub(super) saved: Vec<Saved>,
    pub(super) model: Option<serde_json::Value>,
}

/// One estimated marginal mean: its row over the kept columns, u, se(u) and df.
type Mean = (Vec<f64>, f64, f64, f64);

fn copy(key: &Key) -> Key {
    match key {
        Key::Number(n) => Key::Number(*n),
        Key::Text(t) => Key::Text(t.clone()),
    }
}

/// The messages of MIXED in the words of GENLINMIXED.
pub(super) fn renamed(message: &str) -> String {
    // messages of GENLINMIXED's own name go through here too; leave them as they are
    if message.contains("GENLINMIXED") {
        return message.to_string();
    }
    message
        .replace("MIXED", "GENLINMIXED")
        .replace("MXITER", "MAX_ITERATIONS")
}

/// Start values of the covariance parameters: the weighted residual variance of the fixed part
/// alone, shared out as MIXED does.
fn start_values(model: &Model) -> Vec<f64> {
    let blocks = || model.blocks.iter().enumerate();
    let none: Vec<f64> = blocks()
        .flat_map(|(k, b)| b.start_values(if k == 0 { 1.0 } else { 0.0 }))
        .collect();
    let n = model.y.len();
    let s2 = model.evaluate(&none, super::Depth::Objective).map(|e| {
        (0..n)
            .map(|i| model.g[i] * (model.y[i] - dot(&model.x[i], &e.beta)).powi(2))
            .sum::<f64>()
            / n.saturating_sub(model.p).max(1) as f64
    });
    let share = s2.filter(|v| v.is_finite() && *v > 0.0).unwrap_or(1.0) / model.blocks.len() as f64;
    model
        .blocks
        .iter()
        .flat_map(|b| b.start_values(share))
        .collect()
}

impl Glmm<'_> {
    /// The doubly iterative fit: `model` ends as the final working model.
    pub(super) fn fit(
        &self,
        model: &mut Model,
        case: &[usize],
        command: &Mixed,
    ) -> Result<(Fitted, Vec<String>), Error> {
        let n = model.y.len();
        let y: Vec<f64> = case
            .iter()
            .map(|&k| self.y[k].expect("a used case"))
            .collect();
        let off: Vec<f64> = case.iter().map(|&k| self.offset[k]).collect();
        let prior: Vec<f64> = case
            .iter()
            .zip(&model.g)
            .map(|(&k, g)| g * self.trials[k])
            .collect();
        let binomial = self.family == Family::Binomial;
        // a count of 0 has no log: such a case starts half a count up (docs/differences.md)
        let mut mu: Vec<f64> = case
            .iter()
            .zip(&y)
            .map(|(&k, &y)| match binomial {
                true => (y * self.trials[k] + 0.5) / (self.trials[k] + 1.0),
                false if mu_valid(self.family, y) => y,
                false => y + 0.5,
            })
            .collect();
        let k = match self.family {
            Family::NegBin => {
                negbin_k(&y, &prior, &off, &model.x, self.link, command.mxiter).ok_or_else(|| {
                    Error("GENLINMIXED: the negative binomial k cannot be estimated from the model without random effects".into())
                })?
            }
            _ => 0.0,
        };
        self.k.set(k);
        let tolerance = if command.pconverge.value > 0.0 {
            command.pconverge.value
        } else {
            1e-6
        };
        let mut previous: Option<Vec<f64>> = None;
        let mut iteration = 0;
        loop {
            iteration += 1;
            for i in 0..n {
                let d = link_d1(self.link, mu[i], 0.0);
                model.y[i] = link_value(self.link, mu[i], 0.0) - off[i] + d * (y[i] - mu[i]);
                model.g[i] = prior[i] / (variance(self.family, mu[i], k) * d * d);
                if !(model.y[i].is_finite() && model.g[i].is_finite() && model.g[i] > 0.0) {
                    return Err(Error(
                        "GENLINMIXED: the working target or weight of a case is not finite; the link may not suit the data"
                            .into(),
                    ));
                }
            }
            let start = previous.clone().unwrap_or_else(|| start_values(model));
            let (fitted, mut warnings) = fit_model(model, start, command)?;
            let change = previous.as_ref().map(|old| {
                old.iter()
                    .zip(&fitted.th)
                    .map(|(a, b)| {
                        let size = a.abs() + b.abs();
                        if size == 0.0 {
                            0.0
                        } else {
                            2.0 * (b - a).abs() / size
                        }
                    })
                    .fold(0.0_f64, f64::max)
            });
            if change.is_some_and(|c| c < tolerance) {
                return Ok((fitted, warnings));
            }
            if iteration >= command.mxiter.max(1) {
                warnings.push(format!(
                    "GENLINMIXED reached MAX_ITERATIONS ({}) outer iterations without converging. The estimates may not be final.",
                    command.mxiter
                ));
                return Ok((fitted, warnings));
            }
            // the new means from X b + Z u, where Z u = v - X b - R V^-1 (v - X b)
            let e = &fitted.eval;
            for (gi, cases) in model.groups.iter().enumerate() {
                let r: Vec<f64> = cases
                    .iter()
                    .map(|&i| model.y[i] - dot(&model.x[i], &e.beta))
                    .collect();
                let q = mat_vec(&e.vi[gi], &r);
                for &i in cases {
                    let eta = model.y[i] - residual_part(model, &fitted.th, cases, &q, i);
                    mu[i] = link_inverse(self.link, eta + off[i], 0.0);
                    if !mu_valid(self.family, mu[i]) {
                        return Err(Error(
                            "GENLINMIXED: a fitted mean left the range of the distribution; the link may not suit the data"
                                .into(),
                        ));
                    }
                }
            }
            previous = Some(fitted.th);
        }
    }
}

/// (R V^-1 r)_i over the cases of one group, `q` = V^-1 r: the working target less it is
/// x b + z u, since Z G Z' = V - R.
fn residual_part(model: &Model, th: &[f64], cases: &[usize], q: &[f64], i: usize) -> f64 {
    cases
        .iter()
        .zip(q)
        .map(|(&j, qj)| model.residual_entry(th, i, j, What::Value) * qj)
        .sum()
}

/// The robust covariance of the fixed effects, None (with a warning) where it is not used.
pub(super) fn robust(
    model: &Model,
    e: &Eval,
    df_method: DfMethod,
    warnings: &mut Vec<String>,
) -> Option<Matrix> {
    if df_method == DfMethod::KenwardRoger {
        warnings.push("GENLINMIXED: the Kenward-Roger method works with the model-based covariance of the fixed effects, so COVB=ROBUST is not used".into());
        return None;
    }
    if model.groups.len() < 2 {
        warnings.push("GENLINMIXED: COVB=ROBUST needs independent subjects, and every case is linked to every other, so the model-based covariance is used".into());
        return None;
    }
    let p = model.p;
    let mut meat = vec![vec![0.0; p]; p];
    for (gi, cases) in model.groups.iter().enumerate() {
        let r: Vec<f64> = cases
            .iter()
            .map(|&i| model.y[i] - dot(&model.x[i], &e.beta))
            .collect();
        let q = mat_vec(&e.vi[gi], &r);
        let s: Vec<f64> = (0..p)
            .map(|j| {
                cases
                    .iter()
                    .zip(&q)
                    .map(|(&i, qi)| model.x[i][j] * qi)
                    .sum()
            })
            .collect();
        for (row, sa) in meat.iter_mut().zip(&s) {
            for (m, sb) in row.iter_mut().zip(&s) {
                *m += sa * sb;
            }
        }
    }
    Some(product(&product(&e.a_inv, &meat), &e.a_inv))
}

/// The critical t of a two-sided interval at level `alpha`; infinite at level 0.
fn t_at(alpha: f64, df: f64) -> f64 {
    if !df.is_finite() {
        f64::NAN
    } else if alpha <= 0.0 {
        f64::INFINITY
    } else {
        t_quantile(alpha / 2.0, df)
    }
}

/// Adjusted significances of one set of contrasts and the critical t of each interval. The
/// sequential methods step down from the smallest significance; where an earlier adjusted
/// value carries over, the interval uses the level at which this test reaches it.
fn adjust(p: &[f64], df: &[f64], alpha: f64, method: GenlinAdjust) -> (Vec<f64>, Vec<f64>) {
    if method == GenlinAdjust::Lsd {
        return (p.to_vec(), df.iter().map(|&v| t_at(alpha, v)).collect());
    }
    let sidak = method == GenlinAdjust::SeqSidak;
    let r = p.len() as f64;
    let mut order: Vec<usize> = (0..p.len()).collect();
    order.sort_by(|a, b| p[*a].total_cmp(&p[*b]));
    let mut adjusted = vec![f64::NAN; p.len()];
    let mut critical = vec![f64::NAN; p.len()];
    let mut previous = 0.0_f64;
    for (rank, &i) in order.iter().enumerate() {
        let left = r - rank as f64;
        let own = if sidak {
            1.0 - (1.0 - p[i]).powf(left)
        } else {
            left * p[i]
        };
        let level = if rank == 0 || own >= previous {
            previous = own;
            if sidak {
                1.0 - (1.0 - alpha).powf(1.0 / left)
            } else {
                alpha / left
            }
        } else if sidak {
            1.0 - (1.0 - alpha).powf((1.0 - p[i]).ln() / (1.0 - previous).ln())
        } else {
            alpha * p[i] / previous
        };
        adjusted[i] = previous.min(1.0);
        critical[i] = t_at(level, df[i]);
    }
    (adjusted, critical)
}

fn adjust_name(a: GenlinAdjust) -> &'static str {
    match a {
        GenlinAdjust::SeqBonferroni => "Sequential Bonferroni",
        GenlinAdjust::SeqSidak => "Sequential Sidak",
        _ => "Least Significant Difference",
    }
}

impl Glmm<'_> {
    fn inverse(&self, eta: f64) -> f64 {
        link_inverse(self.link, eta, 0.0)
    }

    /// The interval g^-1(eta -+ t se), lower bound first.
    fn interval(&self, eta: f64, half: f64) -> (f64, f64) {
        let (a, b) = (self.inverse(eta - half), self.inverse(eta + half));
        (a.min(b), a.max(b))
    }

    /// EMMEANS tables, the SAVE fields and the model file, from the final working model.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn after(
        &self,
        model: &Model,
        th: &[f64],
        part: &FixedPart,
        predict: Option<&Predict>,
        names: &[String],
        label_of: &dyn Fn(usize, usize) -> String,
        warnings: &mut Vec<String>,
    ) -> Result<After, Error> {
        let mut tables = Vec::new();
        for em in &self.command.emmeans {
            tables.extend(self.emmeans(part, em, label_of)?);
        }
        let saved = match &self.command.save {
            Some(_) => self.save(model, th, part, predict, warnings),
            None => Vec::new(),
        };
        let model_file = self.command.outfile.as_ref().map(|_| {
            let e = part.inf.e;
            let d = part.d;
            let parameters: Vec<_> = names
                .iter()
                .enumerate()
                .map(|(j, name)| {
                    let at = part.kept.iter().position(|&c| c == j);
                    json!({
                        "name": name,
                        "estimate": at.map_or(0.0, |a| e.beta[a]),
                        "redundant": at.is_none(),
                    })
                })
                .collect();
            let mut file = json!({
                "format": "statscraft-glmm-model",
                "version": 1,
                "target": self.target,
                "distribution": distribution_name(self.distribution(), None),
                "link": link_name(self.link),
                "factors": d.factors.iter().zip(&d.values).map(|(f, levels)| json!({
                    "name": f,
                    "levels": levels,
                })).collect::<Vec<_>>(),
                "covariates": d.covariates,
                "parameters": parameters,
                "covariance": part.inf.cov(),
                "robust": self.robust_used.get(),
            });
            if self.family == Family::NegBin {
                file["negative_binomial_k"] = json!(self.k.get());
            }
            if let Some((keys, event)) = &self.categories {
                file["event"] = json!(match &keys[*event] {
                    Key::Number(n) => json!(n),
                    Key::Text(t) => json!(t.trim_end()),
                });
            }
            file
        });
        Ok(After {
            tables,
            saved,
            model: model_file,
        })
    }

    fn distribution(&self) -> GenlinDistribution {
        match self.family {
            Family::Normal => GenlinDistribution::Normal,
            Family::Binomial => GenlinDistribution::Binomial,
            Family::Gamma => GenlinDistribution::Gamma,
            Family::InverseGaussian => GenlinDistribution::InverseGaussian,
            Family::Poisson => GenlinDistribution::Poisson,
            Family::NegBin => GenlinDistribution::NegativeBinomial(Some(self.k.get())),
        }
    }

    /// One EMMEANS: the means and, with COMPARE, the contrasts and their overall tests.
    fn emmeans(
        &self,
        part: &FixedPart,
        em: &GlmmEmmeans,
        label_of: &dyn Fn(usize, usize) -> String,
    ) -> Result<Vec<Table>, Error> {
        let d = part.d;
        let find = |name: &str| {
            d.factors
                .iter()
                .position(|v| v.eq_ignore_ascii_case(name))
                .ok_or_else(|| {
                    Error(format!(
                        "GENLINMIXED: EMMEANS names {name}, which is not a factor of an effect on FIXED"
                    ))
                })
        };
        let factors: Vec<usize> = em
            .tables
            .iter()
            .map(|n| find(n))
            .collect::<Result<_, _>>()?;
        let mut sorted = factors.clone();
        sorted.sort_unstable();
        let is_effect = part.fit.effects.iter().any(|ef| {
            let mut f = ef.factors.clone();
            f.sort_unstable();
            ef.covariates.is_empty() && f == sorted
        });
        if !factors.is_empty() && !is_effect {
            return Err(Error(format!(
                "GENLINMIXED: EMMEANS TABLES={} is not an effect on FIXED",
                em.tables.join("*")
            )));
        }
        let mut with = Vec::new();
        for (name, v) in &self.command.control {
            let c = d
                .covariates
                .iter()
                .position(|c| c.eq_ignore_ascii_case(name))
                .ok_or_else(|| {
                    Error(format!(
                        "GENLINMIXED: CONTROL names {name}, which is not a covariate"
                    ))
                })?;
            with.push((c, *v));
        }
        let original = !self.command.transformed;
        let alpha = part.alpha();
        let combos = combinations(
            &factors
                .iter()
                .map(|&f| d.values[f].len())
                .collect::<Vec<_>>(),
        );
        // each mean: its row over the kept columns, u, se(u), df
        let means: Vec<Option<Mean>> = combos
            .iter()
            .map(|combo| {
                let fixed: Vec<(usize, usize)> =
                    factors.iter().copied().zip(combo.iter().copied()).collect();
                part.reduce(&emmeans_l(d, part.fit, &fixed, &with))
                    .map(|l| {
                        let (u, se, df) = part.inf.estimate(&l);
                        (l, u, se, df)
                    })
            })
            .collect();
        // dM/du at each mean (1 on the scale of the linear predictor)
        let slope = |u: f64| {
            if original {
                1.0 / link_d1(self.link, self.inverse(u), 0.0)
            } else {
                1.0
            }
        };
        let level_cells = |combo: &[usize], row: &mut Vec<Out>, first: bool| {
            for (k, (&f, &lv)) in factors.iter().zip(combo).enumerate() {
                row.push(if first || combo[k + 1..].iter().all(|&x| x == 0) {
                    text(label_of(f, lv))
                } else {
                    Out::Empty
                });
            }
        };
        let ci = |columns: &mut Vec<(String, String)>| {
            let group = ci_title(part.cin);
            columns.push((group.clone(), "Lower Bound".into()));
            columns.push((group, "Upper Bound".into()));
        };
        let mut columns: Vec<(String, String)> = factors
            .iter()
            .map(|&f| (String::new(), d.factor_texts[f].clone()))
            .collect();
        columns.extend(plain(&["Mean", "Std. Error", "df"]));
        ci(&mut columns);
        let mut rows = Vec::new();
        let mut missing = false;
        for (n, (combo, mean)) in combos.iter().zip(&means).enumerate() {
            let mut row = Vec::new();
            level_cells(combo, &mut row, n == 0);
            match mean {
                Some((_, u, se, df)) => {
                    let half = t_at(alpha, *df) * se;
                    let (m, lo, hi) = if original {
                        let (lo, hi) = self.interval(*u, half);
                        (self.inverse(*u), lo, hi)
                    } else {
                        (*u, u - half, u + half)
                    };
                    let s = se * slope(*u).abs();
                    row.extend([m, s, *df, lo, hi].map(|v| number(Some(v))));
                }
                None => {
                    missing = true;
                    row.extend(std::iter::repeat_n(Out::Empty, 5));
                }
            }
            rows.push(row);
        }
        let scale = if original {
            "the original scale of the target"
        } else {
            "the scale of the linear predictor"
        };
        let mut caption = format!(
            "Target: {}. Means on {scale}.{}",
            self.target_text,
            covariate_note(d, &with)
        );
        if missing {
            caption.push_str(" A mean without a value is not estimable.");
        }
        let title = if factors.is_empty() {
            "Estimated Marginal Means: Overall".to_string()
        } else {
            format!("Estimated Marginal Means: {}", em.tables.join(" * "))
        };
        let mut out = vec![table(&title, factors.len(), columns, rows, Some(caption))];
        let Some(compare) = em
            .compare
            .as_ref()
            .filter(|_| em.contrast != GlmmContrast::None)
        else {
            return Ok(out);
        };
        let compared = find(compare)?;
        let at = factors
            .iter()
            .position(|&f| f == compared)
            .expect("on TABLES");
        let k = d.values[compared].len();
        let mut contrasts: Vec<(String, Vec<f64>)> = match em.contrast {
            GlmmContrast::Pairwise => (0..k)
                .flat_map(|i| (i + 1..k).map(move |j| (i, j)))
                .map(|(i, j)| {
                    let mut c = vec![0.0; k];
                    c[i] = 1.0;
                    c[j] = -1.0;
                    (format!("Level {} - Level {}", i + 1, j + 1), c)
                })
                .collect(),
            GlmmContrast::Deviation => contrast_rows(&Contrast::Deviation(None), k)
                .map_err(|m| Error(format!("GENLINMIXED EMMEANS: {m}")))?,
            _ => contrast_rows(&Contrast::Simple(None), k)
                .map_err(|m| Error(format!("GENLINMIXED EMMEANS: {m}")))?,
        };
        for (label, _) in &mut contrasts {
            for l in (0..k).rev() {
                *label = label.replace(&format!("Level {}", l + 1), &label_of(compared, l));
            }
        }
        let others: Vec<usize> = (0..factors.len()).filter(|&i| i != at).collect();
        let other_combos = combinations(
            &others
                .iter()
                .map(|&i| d.values[factors[i]].len())
                .collect::<Vec<_>>(),
        );
        let mut columns: Vec<(String, String)> = others
            .iter()
            .map(|&i| (String::new(), d.factor_texts[factors[i]].clone()))
            .collect();
        columns.push((String::new(), d.factor_texts[compared].clone()));
        columns.extend(plain(&[
            "Contrast Estimate",
            "Std. Error",
            "t",
            "df",
            "Adj. Sig.",
        ]));
        ci(&mut columns);
        let mut test_columns: Vec<(String, String)> = others
            .iter()
            .map(|&i| (String::new(), d.factor_texts[factors[i]].clone()))
            .collect();
        test_columns.extend(plain(&["F", "df1", "df2", "Sig."]));
        let mut detail = Vec::new();
        let mut overall = Vec::new();
        let mut missing = false;
        for other in &other_combos {
            let mean_of = |level: usize| {
                let mut combo = vec![0; factors.len()];
                for (o, &i) in other.iter().zip(&others) {
                    combo[i] = *o;
                }
                combo[at] = level;
                &means[combos.iter().position(|c| *c == combo).expect("a cell")]
            };
            // each contrast as a row in b (the delta method on the original scale) and its value
            let mut set: Vec<(usize, Vec<f64>, f64, f64, f64)> = Vec::new();
            for (n, (_, c)) in contrasts.iter().enumerate() {
                let mut row = vec![0.0; part.kept.len()];
                let mut value = 0.0;
                let mut ok = true;
                for (level, &w) in c.iter().enumerate().filter(|(_, w)| **w != 0.0) {
                    match mean_of(level) {
                        Some((l, u, _, _)) => {
                            let s = slope(*u);
                            row.iter_mut().zip(l).for_each(|(r, x)| *r += w * s * x);
                            value += w * if original { self.inverse(*u) } else { *u };
                        }
                        None => ok = false,
                    }
                }
                if ok {
                    let (_, se, df) = part.inf.estimate(&row);
                    set.push((n, row, value, se, df));
                } else {
                    missing = true;
                }
            }
            let p: Vec<f64> = set
                .iter()
                .map(|(_, _, v, se, df)| t_p(v / se, *df))
                .collect();
            let dfs: Vec<f64> = set.iter().map(|s| s.4).collect();
            let (adjusted, critical) = adjust(&p, &dfs, alpha, self.command.padjust);
            for (m, (n, _, value, se, df)) in set.iter().enumerate() {
                let mut row: Vec<Out> = others
                    .iter()
                    .zip(other)
                    .map(|(&o, &lv)| {
                        if m == 0 {
                            text(label_of(factors[o], lv))
                        } else {
                            Out::Empty
                        }
                    })
                    .collect();
                row.push(text(contrasts[*n].0.clone()));
                let half = critical[m] * se;
                row.extend(
                    [
                        *value,
                        *se,
                        value / se,
                        *df,
                        adjusted[m],
                        value - half,
                        value + half,
                    ]
                    .map(|v| number(Some(v))),
                );
                detail.push(row);
            }
            let mut row: Vec<Out> = others
                .iter()
                .zip(other)
                .map(|(&o, &lv)| text(label_of(factors[o], lv)))
                .collect();
            // F of L b = a with a chosen so that L b - a is the set of contrast values
            let l: Matrix = set.iter().map(|s| s.1.clone()).collect();
            let a: Vec<f64> = set
                .iter()
                .map(|(_, r, v, _, _)| dot(r, &part.inf.e.beta) - v)
                .collect();
            match part.inf.f_test(&l, &a) {
                Some((q, df, f)) => row.extend([
                    number(Some(f)),
                    Out::Number(q as f64),
                    number(Some(df)),
                    number(Some(f_p(f, q as f64, df))),
                ]),
                None => row.extend(std::iter::repeat_n(Out::Empty, 4)),
            }
            overall.push(row);
        }
        let name = &d.factor_texts[compared];
        let note = if missing {
            " A contrast of a mean that is not estimable is left out."
        } else {
            ""
        };
        out.push(table(
            &format!("Contrast Estimates: {name}"),
            others.len() + 1,
            columns,
            detail,
            Some(format!(
                "Target: {}. Contrasts of the means on {scale}. Adjustment for multiple comparisons: {}.{note}",
                self.target_text,
                adjust_name(self.command.padjust)
            )),
        ));
        out.push(table(
            &format!("Overall Test: {name}"),
            others.len(),
            test_columns,
            overall,
            Some(format!(
                "Target: {}. The F tests the contrasts of {name} together.{note}",
                self.target_text
            )),
        ));
        Ok(out)
    }

    /// The SAVE fields over the cases of the file; cases not used stay missing.
    fn save(
        &self,
        model: &Model,
        th: &[f64],
        part: &FixedPart,
        predict: Option<&Predict>,
        warnings: &mut Vec<String>,
    ) -> Vec<Saved> {
        let s = self.command.save.as_ref().expect("SAVE");
        let cases = self.y.len();
        let k = self.k.get();
        let e = part.inf.e;
        let case = &part.d.case;
        // per case: mu, eta, se(eta), df, Pearson residual
        let mut at: Vec<Option<[f64; 5]>> = vec![None; cases];
        for (gi, group) in model.groups.iter().enumerate() {
            let r: Vec<f64> = group
                .iter()
                .map(|&i| model.y[i] - dot(&model.x[i], &e.beta))
                .collect();
            let q = mat_vec(&e.vi[gi], &r);
            for &i in group {
                let c = case[i];
                if at[c].is_some() {
                    continue;
                }
                let eta = model.y[i] - residual_part(model, th, group, &q, i) + self.offset[c];
                let (se, df) = match predict {
                    Some(pr) if s.intervals.is_some() => {
                        let lr: RandomRow = model
                            .nz
                            .iter()
                            .enumerate()
                            .flat_map(|(b, nz)| {
                                let sub = model.subject[b][i];
                                nz[i].iter().map(move |&(col, z)| (b, sub, col, z))
                            })
                            .collect();
                        let (_, se, df) = pr.estimate_with(&model.x[i], &lr, true);
                        (se, df)
                    }
                    _ => (f64::NAN, f64::NAN),
                };
                let mu = self.inverse(eta);
                // R_ii of the residual structure, without the working weight
                let r_ii = model.residual_entry(th, i, i, What::Value) * model.g[i];
                let prior = self.weight[c] * self.trials[c];
                let y = self.y[c].expect("a used case");
                let pearson = (y - mu) / (variance(self.family, mu, k) * r_ii / prior).sqrt();
                at[c] = Some([mu, eta, se, df, pearson]);
            }
        }
        let alpha = part.alpha();
        let column = |f: &dyn Fn(usize, &[f64; 5]) -> f64| -> Vec<Option<f64>> {
            (0..cases)
                .map(|c| at[c].as_ref().map(|v| f(c, v)).filter(|v| v.is_finite()))
                .collect()
        };
        // the predicted events for events and trials, else the mean
        let times = |c: usize| match (&self.categories, self.family) {
            (None, Family::Binomial) => self.trials[c],
            _ => 1.0,
        };
        let target = &self.target_text;
        let mut out = Vec::new();
        let field = |name: &str, label: String, numbers: Vec<Option<f64>>| Saved {
            name: name.to_string(),
            label,
            numbers,
            texts: None,
            format: None,
        };
        if let Some(name) = &s.predicted {
            match &self.categories {
                Some((keys, event)) => {
                    let pick = |c: usize| {
                        at[c].map(|v| &keys[if v[0] >= 0.5 { *event } else { 1 - *event }])
                    };
                    let label = format!("Predicted Value of {target}");
                    out.push(match self.string_width {
                        Some(w) => Saved {
                            name: name.clone(),
                            label,
                            numbers: Vec::new(),
                            format: None,
                            texts: Some(
                                (0..cases)
                                    .map(|c| match pick(c) {
                                        Some(Key::Text(t)) => t.clone(),
                                        _ => " ".repeat(w),
                                    })
                                    .collect(),
                            ),
                        },
                        None => Saved {
                            format: Some(self.target_format.clone()),
                            ..field(
                                name,
                                label,
                                (0..cases)
                                    .map(|c| match pick(c) {
                                        Some(Key::Number(n)) => Some(*n),
                                        _ => None,
                                    })
                                    .collect(),
                            )
                        },
                    });
                }
                None => out.push(field(
                    name,
                    format!("Predicted Value of Mean of {target}"),
                    column(&|c, v| v[0] * times(c)),
                )),
            }
        }
        let categorical = |what: &str, warnings: &mut Vec<String>| {
            if self.categories.is_none() {
                warnings.push(format!(
                    "GENLINMIXED: SAVE {what} needs a categorical target and is ignored"
                ));
            }
            self.categories.as_ref()
        };
        if let Some(root) = &s.probability
            && let Some((keys, event)) = categorical("PREDICTED_PROBABILITY", warnings)
        {
            for (j, key) in keys.iter().enumerate().take(s.max_categories) {
                let value = match key {
                    Key::Number(n) => n.to_string(),
                    Key::Text(t) => t.trim_end().to_string(),
                };
                let event = j == *event;
                out.push(field(
                    &format!("{root}_{}", j + 1),
                    format!("Predicted Probability of {target} = {value}"),
                    column(&|_, v| if event { v[0] } else { 1.0 - v[0] }),
                ));
            }
        }
        if let Some(name) = &s.confidence
            && categorical("CONFIDENCE", warnings).is_some()
        {
            out.push(field(
                name,
                format!("Confidence of the Predicted Value of {target}"),
                column(&|_, v| {
                    if s.improvement {
                        (2.0 * v[0] - 1.0).abs()
                    } else {
                        v[0].max(1.0 - v[0])
                    }
                }),
            ));
        }
        if let Some(root) = &s.intervals {
            for (lower, side) in [(true, "Lower"), (false, "Upper")] {
                out.push(field(
                    &format!("{root}_{side}"),
                    format!("{side} Confidence Limit of the Mean of {target}"),
                    column(&|c, v| {
                        let (lo, hi) = self.interval(v[1], t_at(alpha, v[3]) * v[2]);
                        times(c) * if lower { lo } else { hi }
                    }),
                ));
            }
        }
        if let Some(name) = &s.pearson {
            out.push(field(
                name,
                format!("Pearson Residual of {target}"),
                column(&|_, v| v[4]),
            ));
        }
        out
    }
}

impl Engine {
    /// Adds the SAVE fields, all or none.
    pub(super) fn save_glmm(&mut self, saved: Vec<Saved>) -> Result<(), Error> {
        self.all_or_nothing(|engine| {
            let data = engine.active_mut()?;
            for s in &saved {
                if data.variable_index(&s.name).is_some() {
                    return Err(Error(format!(
                        "GENLINMIXED /SAVE: field {} already exists",
                        s.name
                    )));
                }
                match &s.texts {
                    Some(t) => {
                        let width = t.iter().map(String::len).max().unwrap_or(1).max(1);
                        data.set_string_values(&s.name, width, t.clone())?;
                    }
                    None => {
                        data.set_numeric_values(&s.name, s.numbers.clone())?;
                        let (format, measure) = match &s.format {
                            Some(f) => (f.as_str(), Measure::Nominal),
                            None => ("F11.5", Measure::Scale),
                        };
                        data.set_format(&s.name, format)?;
                        data.set_measure(&s.name, measure)?;
                    }
                }
                data.set_variable_label(&s.name, Some(&s.label))?;
            }
            Ok(())
        })
    }
}

fn family_of(d: GenlinDistribution) -> Family {
    match d {
        GenlinDistribution::Normal => Family::Normal,
        GenlinDistribution::Binomial => Family::Binomial,
        GenlinDistribution::Gamma => Family::Gamma,
        GenlinDistribution::InverseGaussian => Family::InverseGaussian,
        GenlinDistribution::Poisson => Family::Poisson,
        // the parser stops the rest
        _ => Family::NegBin,
    }
}

fn not_yet(what: &str) -> Error {
    Error(format!("GENLINMIXED: {what} is not supported yet"))
}

impl Engine {
    pub(crate) fn genlinmixed(&mut self, command: &GenLinMixed) -> Result<Vec<Table>, Error> {
        if command.random.is_empty() && command.repeated.is_empty() {
            return Err(not_yet(
                "a model without RANDOM or REPEATED_MEASURES (use GENLIN for a model with fixed effects only)",
            ));
        }
        let scale_min = self.scale_min();
        let data = self.active()?;
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        // the fields of FIELDS are used nowhere else, nor the subjects in the effects
        let mut fields: Vec<&str> = vec![&command.target];
        if let Some(GenlinTrials::Variable(t)) = &command.trials {
            fields.push(t);
        }
        if let Some(GenlinOffset::Variable(o)) = &command.offset {
            fields.push(o);
        }
        fields.extend(command.analysis_weight.as_deref());
        for (i, f) in fields.iter().enumerate() {
            if fields[..i].iter().any(|g| same(f, g)) {
                return Err(Error(format!(
                    "GENLINMIXED: {f} appears more than once on FIELDS"
                )));
            }
        }
        let effect_fields: Vec<&String> = command
            .fixed
            .iter()
            .chain(command.random.iter().flat_map(|r| &r.terms))
            .flat_map(|t| &t.variables)
            .collect();
        let subject_fields: Vec<&String> = command
            .subjects
            .iter()
            .chain(command.random.iter().flat_map(|r| &r.subjects))
            .collect();
        for name in effect_fields
            .iter()
            .chain(&subject_fields)
            .chain(&command.repeated.iter().collect::<Vec<_>>())
        {
            if fields.iter().any(|f| same(f, name)) {
                return Err(Error(format!(
                    "GENLINMIXED: {name} is on FIELDS and cannot be used elsewhere"
                )));
            }
        }
        for r in &command.random {
            if let Some(s) = r
                .subjects
                .iter()
                .find(|s| !command.subjects.iter().any(|d| same(d, s)))
            {
                return Err(Error(format!(
                    "GENLINMIXED: the RANDOM subject {s} must also be a SUBJECTS field of DATA_STRUCTURE"
                )));
            }
        }
        if let Some(s) = subject_fields
            .iter()
            .find(|s| effect_fields.iter().any(|e| same(e, s)))
        {
            return Err(Error(format!(
                "GENLINMIXED: the subject field {s} cannot also be an effect"
            )));
        }
        if let Some(r) = command
            .repeated
            .iter()
            .find(|r| subject_fields.iter().any(|s| same(s, r)))
        {
            return Err(Error(format!(
                "GENLINMIXED: the repeated measures field {r} cannot also be a subject field"
            )));
        }
        // factors and covariates by measurement level
        let mut factors: Vec<String> = Vec::new();
        let mut covariates: Vec<String> = Vec::new();
        for name in &effect_fields {
            let v = data.variable(name)?;
            if factors.iter().chain(&covariates).any(|n| same(n, v.name())) {
                continue;
            }
            let index = data.variable_index(v.name()).expect("found");
            let measure = match v.measure() {
                Measure::Unknown => default_measure(data, index, scale_min),
                known => known,
            };
            if !v.is_string() && measure == Measure::Scale {
                covariates.push(v.name().to_string());
            } else {
                factors.push(v.name().to_string());
            }
        }
        // the distribution: a scale target is NORMAL, events and trials BINOMIAL, and a
        // categorical target of two values BINOMIAL (a nominal multinomial model of two
        // categories is the binomial logit model; docs/differences.md)
        let target = data.variable(&command.target)?;
        let target_index = data.variable_index(target.name()).expect("found");
        let target_measure = match target.measure() {
            Measure::Unknown => default_measure(data, target_index, scale_min),
            known => known,
        };
        let categorical = target.is_string() || target_measure != Measure::Scale;
        let cases = data.case_count();
        let keys = categories(&target, false);
        let mut values: Vec<&Key> = Vec::new();
        for key in keys.iter().flatten() {
            if !values.iter().any(|v| v.compare(key).is_eq()) {
                values.push(key);
            }
        }
        let distribution = match command.distribution {
            Some(d) => d,
            None if command.trials.is_some() => GenlinDistribution::Binomial,
            None if !categorical => GenlinDistribution::Normal,
            None if values.len() == 2 => GenlinDistribution::Binomial,
            None => {
                return Err(not_yet(&format!(
                    "the multinomial distribution, the default for the categorical target {} with {} values",
                    target.name(),
                    values.len()
                )));
            }
        };
        let link = command.link.unwrap_or(default_link(distribution));
        if !link_allowed(distribution, link) {
            return Err(Error(format!(
                "GENLINMIXED: the {} link cannot be used with the {} distribution",
                link_name(link),
                distribution_name(distribution, None)
            )));
        }
        if command.trials.is_some() && distribution != GenlinDistribution::Binomial {
            return Err(Error(
                "GENLINMIXED: TRIALS needs the BINOMIAL distribution".into(),
            ));
        }
        if target.is_string()
            && !(distribution == GenlinDistribution::Binomial && command.trials.is_none())
        {
            return Err(Error(format!(
                "GENLINMIXED: the string target {} can only be a two-valued BINOMIAL target",
                target.name()
            )));
        }
        let numeric = |name: &str| -> Result<Vec<Option<f64>>, Error> {
            let v = data.variable(name)?;
            let Values::Numeric(values) = v.values() else {
                return Err(Error(format!(
                    "GENLINMIXED needs a numeric field, but {name} is a string field"
                )));
            };
            Ok((0..cases)
                .map(|k| values.get(k).filter(|_| is_valid(&v, k, false)))
                .collect())
        };
        let mut y: Vec<Option<f64>> = vec![None; cases];
        let mut trials = vec![1.0; cases];
        let mut two_categories = None;
        let whole = |v: f64| v.fract() == 0.0;
        if distribution == GenlinDistribution::Binomial && command.trials.is_none() {
            if values.len() != 2 {
                return Err(Error(format!(
                    "GENLINMIXED: a BINOMIAL target without TRIALS has two values, but {} has {}",
                    target.name(),
                    values.len()
                )));
            }
            values.sort_by(|a, b| a.compare(b));
            let reference = match (&command.reference, command.target_order) {
                (Some(r), _) => values
                    .iter()
                    .position(|k| value_matches(&target, k, r))
                    .ok_or_else(|| {
                        Error(format!(
                            "GENLINMIXED: REFERENCE is not a value of {}",
                            target.name()
                        ))
                    })?,
                (None, GenlinOrder::Ascending) => 1,
                (None, GenlinOrder::Descending) => 0,
                // the value met last in the file is the last category
                (None, GenlinOrder::Data) => {
                    let first = keys.iter().flatten().next().expect("two values");
                    usize::from(values[0].compare(first).is_eq())
                }
            };
            for (k, key) in keys.iter().enumerate() {
                y[k] = key
                    .as_ref()
                    .map(|key| f64::from(u8::from(values[reference].compare(key).is_ne())));
            }
            // the two values in TARGET_CATEGORY_ORDER, and the event among them
            let event = 1 - reference;
            let first_seen = keys.iter().flatten().next().expect("two values");
            let swap = match command.target_order {
                GenlinOrder::Ascending => false,
                GenlinOrder::Descending => true,
                GenlinOrder::Data => values[0].compare(first_seen).is_ne(),
            };
            let ordered: Vec<Key> = if swap {
                vec![copy(values[1]), copy(values[0])]
            } else {
                vec![copy(values[0]), copy(values[1])]
            };
            two_categories = Some((ordered, if swap { 1 - event } else { event }));
        } else {
            let events = numeric(target.name())?;
            let family = family_of(distribution);
            for (k, e) in events.iter().enumerate() {
                y[k] = e.filter(|&e| match family {
                    Family::Binomial => e >= 0.0 && whole(e),
                    Family::Poisson | Family::NegBin => e >= 0.0 && whole(e),
                    Family::Gamma | Family::InverseGaussian => e > 0.0,
                    _ => true,
                });
            }
            match &command.trials {
                Some(GenlinTrials::Number(m)) => trials = vec![*m; cases],
                Some(GenlinTrials::Variable(t)) => {
                    for (k, m) in numeric(t)?.into_iter().enumerate() {
                        trials[k] = m.unwrap_or(f64::NAN);
                    }
                }
                None => {}
            }
            if family == Family::Binomial {
                for k in 0..cases {
                    let m = trials[k];
                    y[k] = y[k]
                        .filter(|&e| m >= 1.0 && whole(m) && e <= m)
                        .map(|e| e / m);
                }
            }
        }
        let offset = match &command.offset {
            None => vec![0.0; cases],
            Some(GenlinOffset::Value(x)) => vec![*x; cases],
            Some(GenlinOffset::Variable(o)) => {
                let values = numeric(o)?;
                for (k, v) in values.iter().enumerate() {
                    if v.is_none() {
                        y[k] = None;
                    }
                }
                values.into_iter().map(|v| v.unwrap_or(0.0)).collect()
            }
        };
        // a case missing on a subject field of DATA_STRUCTURE is left out
        for name in &command.subjects {
            let keys = categories(&data.variable(name)?, false);
            for (k, key) in keys.iter().enumerate() {
                if key.is_none() {
                    y[k] = None;
                }
            }
        }
        let weight = match &command.analysis_weight {
            Some(w) => numeric(w)?
                .into_iter()
                .map(|v| v.unwrap_or(f64::NAN))
                .collect(),
            None => vec![1.0; cases],
        };
        let family = family_of(distribution);
        let target_text = self.shown().variable(&target);
        let glmm = Glmm {
            command,
            family,
            link,
            y,
            trials,
            offset,
            weight,
            // R-side parameters take the place of the scale
            unit_scale: matches!(family, Family::Binomial | Family::Poisson | Family::NegBin)
                && command.repeated.is_empty(),
            k: Cell::new(0.0),
            robust_used: Cell::new(false),
            categories: two_categories,
            target: target.name().to_string(),
            target_text: target_text.clone(),
            string_width: target.string_width(),
            target_format: target.format().to_string(),
        };
        let mixed = Mixed {
            dependent: target.name().to_string(),
            factors: factors.into_iter().map(VarItem::Name).collect(),
            covariates: covariates.into_iter().map(VarItem::Name).collect(),
            fixed: command.fixed.clone(),
            intercept: command.intercept,
            sstype: 3,
            ml: false,
            include: false,
            cin: command.confidence,
            mxiter: command.max_iterations,
            mxstep: 10,
            scoring: command.scoring,
            singular: command.singular,
            pconverge: command.pconverge,
            lconverge: command.lconverge,
            hconverge: command.hconverge,
            df_method: command.df_method,
            print: MixedPrint {
                solution: true,
                testcov: true,
                ..MixedPrint::default()
            },
            random: command
                .random
                .iter()
                .map(|r| RandomEffects {
                    terms: r.terms.clone(),
                    subject: r.subjects.clone(),
                    covtype: r.covtype,
                    solution: r.solution,
                })
                .collect(),
            repeated: (!command.repeated.is_empty()).then(|| Repeated {
                variables: command.repeated.clone(),
                subject: command.subjects.clone(),
                // for repeated measures VARIANCE_COMPONENTS is DIAGONAL
                covtype: match command.residual_covtype {
                    CovType::Vc => CovType::Diag,
                    t => t,
                },
            }),
            regwgt: command.analysis_weight.clone(),
            save: Vec::new(),
            emmeans: Vec::new(),
            tests: Vec::new(),
        };
        let fitted = self
            .mixed_fit(&mixed, Some(&glmm))
            .map_err(|e| Error(renamed(&e.0)))?;
        let k = (family == Family::NegBin).then(|| glmm.k.get());
        let mut caption = "Fitted by restricted pseudo likelihood: the information criteria belong to the final working linear model.".to_string();
        if glmm.robust_used.get() {
            caption.push_str(" The tests use the robust covariance of the fixed effects.");
        }
        let summary = table(
            "Model Summary",
            1,
            plain(&["", "Value"]),
            vec![
                vec![text("Target"), text(target_text)],
                vec![
                    text("Probability Distribution"),
                    text(distribution_name(distribution, k)),
                ],
                vec![text("Link Function"), text(link_name(link))],
            ],
            Some(caption),
        );
        let mut tables = vec![summary];
        tables.extend(fitted);
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn renamed_once() {
        assert_eq!(
            super::renamed("MIXED reached MXITER"),
            "GENLINMIXED reached MAX_ITERATIONS"
        );
        let own = "GENLINMIXED reached MAX_ITERATIONS (50)";
        assert_eq!(super::renamed(own), own);
    }
}
