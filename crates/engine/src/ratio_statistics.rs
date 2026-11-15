//! RATIO STATISTICS: descriptive statistics of the ratio of two variables.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual, in our words:
//! - A case is used when the numerator and the denominator are valid (MISSING=INCLUDE makes
//!   user-missing values valid) and above 0, and its weight, rounded to a whole number with
//!   halves going up, is above 0. The weights act as frequencies. The group variable's missing
//!   values form groups of their own.
//! - With F the sum of weights: the mean, the standard deviation over F - 1, the median of the
//!   F ordered ratios, AAD the mean absolute distance from the median, COD = 100 AAD / median,
//!   MNCOV = 100 sd / mean, the weighted mean = sum of numerators / sum of denominators and
//!   PRD = mean / weighted mean. A coefficient of concentration is the percent of ratios in a
//!   closed interval: BCOC's own ends, or the median times 1 - g and 1 + g for WCOC.
//! - Intervals: the mean's uses t with F - 1 df and sd / sqrt(F); the median's is distribution
//!   free, between the r-th smallest and r-th largest ratios with r the largest number for
//!   which the binomial (F, 1/2) probability below r is at most alpha / 2; the weighted mean's
//!   uses the delta method variance of the ratio of the two means.

use crate::crosstabs::{Key, categories};
use crate::descriptives::{case_weights, number};
use crate::dist::t_quantile;
use crate::npar_tests::{binomial_cdf, table, text};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Dataset;
use statscraft_syntax::{RatioOrder, RatioRequest, RatioStatistics};

/// One used case: ratio, numerator, denominator and whole weight.
#[derive(Clone, Copy)]
struct Case {
    r: f64,
    a: f64,
    s: f64,
    f: f64,
}

/// One requested statistic: its interval header (if any), column title, variable name in the
/// OUTFILE data and label there.
struct Item {
    group: Option<String>,
    title: String,
    name: String,
    label: String,
}

/// The value of the k-th smallest (from 1) of the ratios, each repeated as often as its weight;
/// `cases` is sorted by ratio.
fn order_statistic(cases: &[Case], k: f64) -> f64 {
    let mut below = 0.0;
    for c in cases {
        below += c.f;
        if below >= k {
            return c.r;
        }
    }
    cases.last().map_or(f64::NAN, |c| c.r)
}

/// The largest r from 1 to n with P(X <= r - 1) <= alpha / 2 for X binomial (n, 1/2), if any.
fn median_rank(n: f64, alpha: f64) -> Option<f64> {
    let fits = |r: f64| binomial_cdf(r - 1.0, n, 0.5) <= alpha / 2.0 + 1e-12;
    if !fits(1.0) {
        return None;
    }
    let (mut low, mut high) = (1.0, n.ceil());
    while high - low > 0.5 {
        let middle = ((low + high) / 2.0).ceil();
        if fits(middle) {
            low = middle;
        } else {
            high = middle - 1.0;
        }
    }
    Some(low)
}

type Results = Vec<(Item, Option<f64>)>;

fn push(out: &mut Results, group: Option<&str>, title: &str, name: &str, label: &str, value: f64) {
    let item = Item {
        group: group.map(str::to_string),
        title: title.to_string(),
        name: name.to_string(),
        label: label.to_string(),
    };
    out.push((item, Some(value).filter(|v| v.is_finite())));
}

/// A statistic whose label is its title.
fn plain(out: &mut Results, on: bool, title: &str, name: &str, value: f64) {
    if on {
        push(out, None, title, name, title, value);
    }
}

/// The lower and upper bounds of the interval for `what` at `level` percent.
fn bounds(out: &mut Results, level: f64, what: &str, name: &str, (low, high): (f64, f64)) {
    let group = format!("{level}% Confidence Interval for {what}");
    for (side, value) in [("Lower", low), ("Upper", high)] {
        let label = format!("{group}, {} bound", side.to_lowercase());
        let title = format!("{side} Bound");
        push(
            out,
            Some(&group),
            &title,
            &format!("{name}_{side}"),
            &label,
            value,
        );
    }
}

/// The requested statistics in the order they show, with their values for `cases` (sorted by
/// ratio).
fn statistics(cases: &[Case], request: &RatioRequest) -> Results {
    let sum = |g: &dyn Fn(&Case) -> f64| cases.iter().map(|c| c.f * g(c)).sum::<f64>();
    let f = sum(&|_| 1.0);
    let mean = sum(&|c| c.r) / f;
    let median = if f % 2.0 == 1.0 {
        order_statistic(cases, (f + 1.0) / 2.0)
    } else {
        (order_statistic(cases, f / 2.0) + order_statistic(cases, f / 2.0 + 1.0)) / 2.0
    };
    let sd = (sum(&|c| (c.r - mean).powi(2)) / (f - 1.0)).sqrt();
    let (a_mean, s_mean) = (sum(&|c| c.a) / f, sum(&|c| c.s) / f);
    let wgtmean = a_mean / s_mean;
    let aad = sum(&|c| (c.r - median).abs()) / f;
    let coc = |low: f64, high: f64| 100.0 * sum(&|c| f64::from(c.r >= low && c.r <= high)) / f;
    // the level, alpha and the t quantile of the intervals
    let cin = request.cin.map(|level| {
        let alpha = 1.0 - level / 100.0;
        (level, alpha, t_quantile(alpha / 2.0, f - 1.0))
    });
    let mut out = Vec::new();
    plain(&mut out, request.mean, "Mean", "Mean", mean);
    if let (true, Some((level, _, t))) = (request.mean, cin) {
        let half = t * sd / f.sqrt();
        bounds(&mut out, level, "Mean", "Mean", (mean - half, mean + half));
    }
    plain(&mut out, request.median, "Median", "Median", median);
    if let (true, Some((level, alpha, _))) = (request.median, cin) {
        let (low, high, coverage) = match median_rank(f, alpha) {
            Some(r) => (
                order_statistic(cases, r),
                order_statistic(cases, f - r + 1.0),
                1.0 - 2.0 * binomial_cdf(r - 1.0, f, 0.5),
            ),
            None => (f64::NAN, f64::NAN, f64::NAN),
        };
        bounds(&mut out, level, "Median", "Median", (low, high));
        let group = format!("{level}% Confidence Interval for Median");
        let label = format!("{group}, actual coverage");
        let title = "Actual Coverage";
        push(
            &mut out,
            Some(&group),
            title,
            "Median_Coverage",
            &label,
            100.0 * coverage,
        );
    }
    plain(
        &mut out,
        request.wgtmean,
        "Weighted Mean",
        "WgtMean",
        wgtmean,
    );
    if let (true, Some((level, _, t))) = (request.wgtmean, cin) {
        // the variances of the two means, frequency weights counting as repeated cases
        let moment = |g: &dyn Fn(&Case) -> f64| sum(g) / (f - 1.0) / f;
        let var_a = moment(&|c| (c.a - a_mean).powi(2));
        let var_s = moment(&|c| (c.s - s_mean).powi(2));
        let cov = moment(&|c| (c.a - a_mean) * (c.s - s_mean));
        let variance = var_a / s_mean.powi(2) - 2.0 * a_mean * cov / s_mean.powi(3)
            + a_mean.powi(2) * var_s / s_mean.powi(4);
        let half = t * variance.max(0.0).sqrt();
        bounds(
            &mut out,
            level,
            "Weighted Mean",
            "WgtMean",
            (wgtmean - half, wgtmean + half),
        );
    }
    let min = cases.first().map_or(f64::NAN, |c| c.r);
    let max = cases.last().map_or(f64::NAN, |c| c.r);
    plain(&mut out, request.min, "Minimum", "Minimum", min);
    plain(&mut out, request.max, "Maximum", "Maximum", max);
    plain(&mut out, request.range, "Range", "Range", max - min);
    plain(&mut out, request.stddev, "Std. Deviation", "StdDev", sd);
    plain(
        &mut out,
        request.aad,
        "Average Absolute Deviation",
        "AAD",
        aad,
    );
    plain(
        &mut out,
        request.prd,
        "Price Related Differential",
        "PRD",
        mean / wgtmean,
    );
    plain(
        &mut out,
        request.cod,
        "Coefficient of Dispersion",
        "COD",
        100.0 * aad / median,
    );
    let title = "Coefficient of Variation, Mean Centered";
    plain(&mut out, request.mncov, title, "MnCOV", 100.0 * sd / mean);
    let mdcov = 100.0 * (sum(&|c| (c.r - median).powi(2)) / (f - 1.0)).sqrt() / median;
    let title = "Coefficient of Variation, Median Centered";
    plain(&mut out, request.mdcov, title, "MdCOV", mdcov);
    for (i, (low, high)) in request.bcoc.iter().enumerate() {
        let title = format!("Coefficient of Concentration, {low} to {high}");
        let name = format!("BCOC_{}", i + 1);
        push(&mut out, None, &title, &name, &title, coc(*low, *high));
    }
    for (i, g) in request.wcoc.iter().enumerate() {
        let title = format!("Coefficient of Concentration, Within {g}% of Median");
        let share = g / 100.0;
        let value = coc((1.0 - share) * median, (1.0 + share) * median);
        push(
            &mut out,
            None,
            &title,
            &format!("WCOC_{}", i + 1),
            &title,
            value,
        );
    }
    out
}

impl Engine {
    pub(crate) fn ratio_statistics(
        &mut self,
        command: &RatioStatistics,
    ) -> Result<Vec<Table>, Error> {
        if command.outfile.is_some() && self.split_group.is_some() {
            return Err(Error(
                "RATIO STATISTICS /OUTFILE is not available with SPLIT FILE; turn SPLIT FILE off"
                    .into(),
            ));
        }
        let data = self.active()?;
        let numerator = data.variable(&command.numerator)?;
        let denominator = data.variable(&command.denominator)?;
        for v in [&numerator, &denominator] {
            if v.is_string() {
                return Err(Error(format!(
                    "RATIO STATISTICS needs a numeric numerator and denominator, but {} is a \
                     string variable",
                    v.name()
                )));
            }
        }
        let group_variable = match &command.group {
            Some((name, _)) => Some(data.variable(name)?),
            None => None,
        };
        let group_keys = group_variable.as_ref().map(|v| categories(v, true));
        let weights = case_weights(data)?;
        let (a_values, s_values) = (numerator.numeric()?, denominator.numeric()?);
        let valid = |v: &statscraft_data::Variable, x: Option<f64>| {
            x.filter(|x| command.include || !v.is_user_missing(*x))
        };
        // the groups in order of appearance, with the first case of each and its cases
        let mut groups: Vec<(Option<Key>, usize, Vec<Case>)> = Vec::new();
        let mut overall: Vec<Case> = Vec::new();
        let (mut total, mut excluded) = (0.0, 0.0);
        for case in 0..data.case_count() {
            let f = weights.as_ref().map_or(1.0, |w| w[case]).round();
            if f <= 0.0 {
                continue;
            }
            total += f;
            let a = valid(&numerator, a_values.get(case)).filter(|a| *a > 0.0);
            let s = valid(&denominator, s_values.get(case)).filter(|s| *s > 0.0);
            let (Some(a), Some(s)) = (a, s) else {
                excluded += f;
                continue;
            };
            let c = Case { r: a / s, a, s, f };
            overall.push(c);
            if let Some(keys) = &group_keys {
                let key = keys[case].clone();
                match groups.iter_mut().find(|g| g.0 == key) {
                    Some(group) => group.2.push(c),
                    None => groups.push((key, case, vec![c])),
                }
            }
        }
        if let Some((_, order)) = &command.group {
            // a missing group value sorts below every other
            let compare = |x: &Option<Key>, y: &Option<Key>| match (x, y) {
                (Some(x), Some(y)) => x.compare(y),
                _ => x.is_some().cmp(&y.is_some()),
            };
            match order {
                RatioOrder::Ascending => groups.sort_by(|x, y| compare(&x.0, &y.0)),
                RatioOrder::Descending => groups.sort_by(|x, y| compare(&y.0, &x.0)),
                RatioOrder::NoSort => {}
            }
        }
        let shown = self.shown();
        let group_title = group_variable
            .as_ref()
            .map_or(String::new(), |v| shown.variable(v));
        let group_text = |key: &Option<Key>| match (key, &group_variable) {
            (Some(key), Some(v)) => shown.value(v, key),
            _ => ".".to_string(),
        };
        let percent = |n: f64| number(Some(100.0 * n / total).filter(|v| v.is_finite()));
        let used: f64 = overall.iter().map(|c| c.f).sum();
        let mut processing_rows: Vec<Vec<Cell>> = groups
            .iter()
            .map(|(key, _, cases)| {
                let n: f64 = cases.iter().map(|c| c.f).sum();
                vec![text(group_text(key)), Cell::Number(n), percent(n)]
            })
            .collect();
        processing_rows.push(vec![text("Overall"), Cell::Number(used), percent(used)]);
        processing_rows.push(vec![
            text("Excluded"),
            Cell::Number(excluded),
            percent(excluded),
        ]);
        processing_rows.push(vec![text("Total"), Cell::Number(total), percent(total)]);
        let columns = vec![group_title.clone(), "Count".into(), "Percent".into()];
        let mut tables = vec![table(
            "Case Processing Summary",
            1,
            columns,
            processing_rows,
        )];
        if overall.is_empty() {
            self.warn("RATIO STATISTICS: no case has a valid positive numerator and denominator");
            return Ok(tables);
        }
        let sort = |cases: &mut Vec<Case>| cases.sort_by(|x, y| x.r.total_cmp(&y.r));
        sort(&mut overall);
        for group in &mut groups {
            sort(&mut group.2);
        }
        if let Some(request) = &command.print {
            let mut rows = Vec::new();
            let mut items = Vec::new();
            for (label, cases) in groups
                .iter()
                .map(|g| (group_text(&g.0), &g.2))
                .chain([("Overall".to_string(), &overall)])
            {
                let stats = statistics(cases, request);
                let mut row = vec![text(label)];
                row.extend(stats.iter().map(|(_, v)| number(*v)));
                rows.push(row);
                items = stats.into_iter().map(|(item, _)| item).collect();
            }
            if !items.is_empty() {
                let mut columns = vec![group_title.clone()];
                columns.extend(items.iter().map(|i| i.title.clone()));
                let title = format!(
                    "Ratio Statistics for {} / {}",
                    shown.variable(&numerator),
                    shown.variable(&denominator)
                );
                let mut t = table(title, 1, columns, rows);
                for (column, item) in items.iter().enumerate() {
                    if let Some(group) = &item.group {
                        t.column_groups[column + 1] = vec![group.clone()];
                    }
                }
                if items.iter().any(|i| i.group.is_some()) {
                    t.caption = Some(
                        "The interval for the median is built from order statistics, without \
                         assuming a distribution; its actual coverage may exceed the level asked. \
                         The other intervals assume normally distributed ratios."
                            .into(),
                    );
                }
                tables.push(t);
            }
        }
        if let Some((file, request)) = &command.outfile {
            let records: Vec<(Option<usize>, &Vec<Case>)> = [(None, &overall)]
                .into_iter()
                .chain(groups.iter().map(|g| (Some(g.1), &g.2)))
                .collect();
            let mut out = Dataset::new(records.len());
            if let Some((name, _)) = &command.group {
                let cases: Vec<Option<usize>> = records.iter().map(|r| r.0).collect();
                out.copy_variable(data, name, &cases)?;
            }
            let results: Vec<Vec<(Item, Option<f64>)>> =
                records.iter().map(|r| statistics(r.1, request)).collect();
            for (i, (item, _)) in results[0].iter().enumerate() {
                let values = results.iter().map(|r| r[i].1).collect();
                out.set_numeric_values(&item.name, values)?;
                out.set_variable_label(&item.name, Some(&item.label))?;
            }
            self.write_data_file(file, out)?;
        }
        Ok(tables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_rank_matches_the_binomial_tail() {
        // n = 10, 95%: P(X <= 1) = 11/1024 <= 0.025 < P(X <= 2) = 56/1024
        assert_eq!(median_rank(10.0, 0.05), Some(2.0));
        // n = 5: even P(X <= 0) = 1/32 is above 0.025
        assert_eq!(median_rank(5.0, 0.05), None);
        // level 0: the interval shrinks to the median
        assert_eq!(median_rank(7.0, 1.0), Some(4.0));
    }
}
