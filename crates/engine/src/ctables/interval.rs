//! Confidence limits and standard errors of CTABLES summaries: the .LCL, .UCL and .SE suffixes,
//! at the level of /CRITERIA CILEVEL (95 by default).
//!
//! The rules, in our words, from the command reference (CTABLES, "Confidence intervals and
//! standard error", the CRITERIA subcommand and "Weights and Rounding") and the CTABLES chapter
//! of the algorithms reference ("Means and Sums", "Counts and Percentages", "Percentiles"):
//! - A proportion p = w / W of an area: with the WEIGHT command the weights are rounded to w'
//!   and W', the standard error is sqrt(p (1 - p) / W'), and the limits are the alpha / 2 and
//!   1 - alpha / 2 quantiles of Beta(w' + 0.5, W' - w' + 0.5). With effective base weights
//!   (/WEIGHT) nothing is rounded: the effective base E = W^2 / q takes the place of W', and
//!   E p and E (1 - p) the place of w' and W' - w'. A percentage shows 100 times these. Where
//!   the manual is silent: a proportion of 0 has the lower limit 0 and one of 1 the upper limit
//!   1, as is usual for this (Jeffreys) interval, so that the limits enclose the estimate.
//! - A count is W' times the proportion of its area. The manual does not name the area; we take
//!   the whole table (of one layer), the area of LAYERPCT.
//! - A mean: x +- t(1 - alpha / 2, w' - 1) s / sqrt(w'), with s^2 the weighted variance. With
//!   effective base weights the variance is adjusted to the effective base e (the weighted sum of
//!   squared deviations over w - w / e), and e takes the place of w'. A sum: the same with the
//!   standard error sqrt(w') s, or w times that of the mean with effective base weights.
//! - A percentile (MEDIAN, PTILE): the share P = w / W of the weight at or below it gets the
//!   limits of a proportion above, and the percentiles at those shares are its limits.
//! - Sets have no intervals (the syntax refuses them); a cell without the values needed shows
//!   none.

use super::{Section, Tally, effective_base, percentile};
use crate::dist::{beta_i, solve, t_quantile};
use statscraft_syntax::{Estimate, PercentBase, PercentOf, Statistic, Summary};

/// The p quantile of the Beta(a, b) distribution.
fn beta_quantile(p: f64, a: f64, b: f64) -> f64 {
    solve(|x| beta_i(a, b, x) - p, 0.0, 1.0)
}

impl Section {
    fn alpha(&self) -> f64 {
        1.0 - self.level / 100.0
    }

    /// The lower and upper limits and the standard error of the proportion `part` of `whole`,
    /// whose squared weights sum to `squares`.
    fn proportion(&self, part: f64, whole: f64, squares: f64) -> Option<[f64; 3]> {
        if whole <= 0.0 {
            return None;
        }
        let p = part / whole;
        let ((a, b), size) = if self.effective {
            let e = effective_base(whole, squares);
            ((e * p + 0.5, e * (1.0 - p) + 0.5), e)
        } else {
            let (w, total) = (part.round(), whole.round());
            ((w + 0.5, total - w + 0.5), total)
        };
        if size <= 0.0 || b <= 0.0 {
            return None;
        }
        let alpha = self.alpha();
        // none of the area, or all of it: the limit on that side is the proportion itself
        let low = if a <= 0.5 {
            0.0
        } else {
            beta_quantile(alpha / 2.0, a, b)
        };
        let high = if b <= 0.5 {
            1.0
        } else {
            beta_quantile(1.0 - alpha / 2.0, a, b)
        };
        Some([low, high, (p * (1.0 - p) / size).sqrt()])
    }

    /// The .LCL, .UCL or .SE of a summary in the cell at `key`.
    pub(super) fn interval(&self, key: &[usize], summary: &Summary, tally: &Tally) -> Option<f64> {
        let estimate = summary.function.estimate;
        let pick = |[low, high, error]: [f64; 3]| match estimate {
            Estimate::Lower => low,
            Estimate::Upper => high,
            _ => error,
        };
        match summary.function.statistic {
            Statistic::Percent(base, of) => {
                let area = self.area_tally(key, base)?;
                let squares = match of {
                    PercentOf::Validn => area.valid_squares,
                    PercentOf::Totaln => area.squares + area.total_only_squares,
                    _ => area.squares,
                };
                let found =
                    self.proportion(tally.base(of, false), area.base(of, false), squares)?;
                Some(100.0 * pick(found))
            }
            Statistic::Count => {
                let area = self.area_tally(key, PercentBase::Layer)?;
                let found = self.proportion(tally.count, area.count, area.squares)?;
                let whole = if self.effective {
                    area.count
                } else {
                    area.count.round()
                };
                Some(whole * pick(found))
            }
            Statistic::Mean | Statistic::Sum => {
                let values = &tally.values;
                let w: f64 = values.iter().map(|v| v.1).sum();
                if w <= 0.0 {
                    return None;
                }
                let sum: f64 = values.iter().map(|(x, v)| x * v).sum();
                let mean = sum / w;
                let deviations: f64 = values.iter().map(|(x, v)| v * (x - mean).powi(2)).sum();
                let (n, variance) = if self.effective {
                    let e = effective_base(w, values.iter().map(|v| v.1 * v.1).sum());
                    (e, deviations / (w - w / e))
                } else {
                    (w.round(), deviations / (w - 1.0))
                };
                if !(n > 1.0 && variance.is_finite()) {
                    return None;
                }
                let of_mean = (variance / n).sqrt();
                let (value, error) = match summary.function.statistic {
                    Statistic::Mean => (mean, of_mean),
                    _ if self.effective => (sum, w * of_mean),
                    _ => (sum, (n * variance).sqrt()),
                };
                let t = t_quantile(self.alpha() / 2.0, n - 1.0);
                Some(pick([value - t * error, value + t * error, error]))
            }
            Statistic::Median | Statistic::Ptile => {
                let p = match summary.function.statistic {
                    Statistic::Median => 50.0,
                    _ => summary.percentile.unwrap_or(50.0),
                };
                let values: Vec<(f64, f64)> =
                    tally.values.iter().copied().filter(|v| v.1 > 0.0).collect();
                let at = percentile(&values, p)?;
                let whole: f64 = values.iter().map(|v| v.1).sum();
                let below: f64 = values.iter().filter(|v| v.0 <= at).map(|v| v.1).sum();
                let squares: f64 = values.iter().map(|v| v.1 * v.1).sum();
                let [low, high, _] = self.proportion(below, whole, squares)?;
                let share = if estimate == Estimate::Lower {
                    low
                } else {
                    high
                };
                percentile(&values, 100.0 * share)
            }
            _ => None,
        }
    }
}
