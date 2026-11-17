//! The fit of CSORDINAL: the cumulative link model of PLUM, one case per row, weighted.
//!
//! Rules from the Algorithms manual, in our words:
//! - link(P(Y <= k | x)) = theta_k - x'b for k below K. The parameters are the K - 1
//!   thresholds and then b over the non-redundant columns of the design without the intercept.
//! - The start: b = 0 and theta_k = link(sum of the weights of categories 1..k over all
//!   weights); its log likelihood is that of the thresholds-only model, for the pseudo R squares.
//! - A step solves J d = score, J the observed information (Newton-Raphson) or the expected one
//!   (Fisher scoring, the first n steps of FISHER(n)). A step is halved, up to MXSTEP sizes, until
//!   the log likelihood does not fall and every case keeps a positive probability of its own
//!   category. Convergence, MXITER and CHKSEP work as in CSLOGISTIC; a fit that converges while
//!   still scoring goes on with Newton-Raphson.
//! - The score of case i is the derivative of log P(y_i) at the estimates, unweighted; the
//!   covariance uses the observed information at the estimates.
//! - NONPARALLEL fits the general model, link(P(Y <= k | x)) = theta_k - x'b_k with a set of
//!   coefficients per threshold, the same way, starting at the estimates of the model.

use super::{Logistic, information_inverse};
use crate::matrix::Matrix;
use crate::plum::{Model, cell_probabilities, negated, quantile, solve};
use statscraft_syntax::{CsCriteria, PlumLink};

/// What the fit gives: the parameters (thresholds first), the likelihoods and probabilities,
/// the score of every row and the observed information.
pub(super) struct Ordinal {
    pub(super) b: Vec<f64>,
    pub(super) fit: Logistic,
    pub(super) scores: Vec<Vec<f64>>,
    pub(super) information: Matrix,
}

/// Fits the model to rows `x` (the non-redundant regression columns) with weights `w` and
/// categories `y` in 0..k. With `general`, the start of the general model (thresholds, then the
/// coefficients of each threshold in turn), that model is fitted instead.
#[allow(clippy::too_many_arguments)]
pub(super) fn fit(
    link: PlumLink,
    x: &[Vec<f64>],
    w: &[f64],
    y: &[usize],
    k: usize,
    c: &CsCriteria,
    general: Option<&[f64]>,
    warnings: &mut Vec<String>,
) -> Ordinal {
    let who = if general.is_some() {
        "CSORDINAL NONPARALLEL (the general model)"
    } else {
        "CSORDINAL"
    };
    let one_hot = |r: usize, value: f64| -> Vec<f64> {
        let mut v = vec![0.0; k];
        v[y[r]] = value;
        v
    };
    let counts: Vec<Vec<f64>> = (0..x.len()).map(|r| one_hot(r, w[r])).collect();
    let model = Model {
        link,
        counts: &counts,
        x,
        z: &[],
        parallel: general.is_none(),
        scale: false,
    };
    let mut totals = vec![0.0; k];
    for (row, &wr) in y.iter().zip(w) {
        totals[*row] += wr;
    }
    let all: f64 = totals.iter().sum();
    let mut b = vec![0.0; k - 1 + x.first().map_or(0, Vec::len)];
    let mut cumulative = 0.0;
    for j in 0..k - 1 {
        cumulative += totals[j];
        b[j] = quantile(link, cumulative / all);
    }
    if let Some(start) = general {
        b = start.to_vec();
    }
    // every category has weight, so the start gives each case a positive probability (the
    // general model starts at a fit that does)
    let mut eval = model.eval(&b).expect("the start fits every case");
    let ll0 = eval.l;
    let mut history = vec![(0, -2.0 * eval.l, 0, b.clone())];
    let mut fisher = c.fisher;
    let mut iteration = 0;
    while iteration < c.mxiter as usize {
        let scoring = (iteration as u32) < fisher;
        // Newton-Raphson; Fisher scoring when asked or where the observed information gives no
        // ascent
        let newton = (!scoring)
            .then(|| solve(&negated(&eval.hessian), &eval.gradient))
            .flatten()
            .filter(|d| {
                d.iter()
                    .zip(&eval.gradient)
                    .map(|(a, g)| a * g)
                    .sum::<f64>()
                    > 0.0
            });
        let step = newton.unwrap_or_else(|| {
            let inv = information_inverse(who, &eval.information, warnings);
            inv.iter()
                .map(|row| row.iter().zip(&eval.gradient).map(|(a, g)| a * g).sum())
                .collect()
        });
        let mut accepted = None;
        let mut factor = 1.0;
        for halvings in 0..c.mxstep.max(1) as usize {
            let candidate: Vec<f64> = b.iter().zip(&step).map(|(v, s)| v + factor * s).collect();
            if let Some(e) = model.eval(&candidate)
                && e.l >= eval.l - 1e-12 * (1.0 + eval.l.abs())
            {
                accepted = Some((candidate, e, halvings));
                break;
            }
            factor /= 2.0;
        }
        iteration += 1;
        let Some((candidate, next, halvings)) = accepted else {
            warnings.push(format!(
                "{who}: the log pseudo-likelihood did not increase with any of the {} step sizes (MXSTEP) at iteration {iteration}; the estimates before it are shown",
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
        let (lc, relative_l) = c.lconverge;
        let ll_change =
            (next.l - eval.l).abs() / if relative_l { eval.l.abs() + 1e-6 } else { 1.0 };
        b = candidate;
        eval = next;
        history.push((iteration, -2.0 * eval.l, halvings, b.clone()));
        let converged = (pc > 0.0 && change < pc) || (lc > 0.0 && ll_change < lc);
        if converged && !scoring {
            break;
        }
        if converged {
            fisher = 0;
        }
        if c.chksep > 0
            && iteration >= c.chksep as usize
            && (0..x.len()).all(|r| probabilities(&model, &b, r)[y[r]] > 1.0 - 1e-8)
        {
            warnings.push(format!(
                "{who}: complete separation at iteration {iteration}: every case is predicted with certainty, so the estimates are not reliable"
            ));
            break;
        }
        if iteration == c.mxiter as usize && !converged {
            warnings.push(format!(
                "{who}: the estimates did not converge within {} iterations (MXITER)",
                c.mxiter
            ));
        }
    }
    let scores = (0..x.len())
        .map(|r| {
            let unweighted = [one_hot(r, 1.0)];
            let single = Model {
                link,
                counts: &unweighted,
                x: &x[r..=r],
                z: &[],
                parallel: general.is_none(),
                scale: false,
            };
            single
                .eval(&b)
                .map_or_else(|| vec![0.0; b.len()], |e| e.gradient)
        })
        .collect();
    let probs = (0..x.len()).map(|r| probabilities(&model, &b, r)).collect();
    Ordinal {
        b,
        fit: Logistic {
            probs,
            ll: eval.l,
            ll0,
            history,
        },
        scores,
        information: negated(&eval.hessian),
    }
}

/// The probability of every category for row r.
fn probabilities(model: &Model, b: &[f64], r: usize) -> Vec<f64> {
    cell_probabilities(&model.cumulative(b, r))
}
