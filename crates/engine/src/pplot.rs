//! PPLOT: probability plots of variables against a test distribution.
//!
//! Rules from the Command Syntax Reference (PPLOT) and the Algorithms manual (PPLOT
//! Algorithms):
//! - Each variable is a series in case order. User- and system-missing values are left out of
//!   the plot. LN takes the natural log first (a value of 0 or less becomes missing), then DIFF
//!   differences the series and SDIFF differences it over PERIOD cases; a difference with a
//!   missing side is missing. STANDARDIZE then turns the values into z-scores.
//! - Ranks: the mean rank of tied values, or the lowest, the highest, or BREAK's consecutive
//!   ranks of the distinct values. The fraction p of rank r out of n values is BLOM
//!   (r - 3/8) / (n + 1/4), RANKIT (r - 1/2) / n, TUKEY (r - 1/3) / (n + 1/3) or VW r / (n + 1).
//! - Q-Q: each value against the quantile of the distribution at its p; P-P: p against the
//!   distribution function at the value. A detrended plot shows the observed minus the
//!   expected. The plain plots have the line y = x, the detrended ones y = 0.
//! - Parameters not given are estimated, with S the standard deviation over n - 1: normal mean
//!   and S; exponential rate 1 / mean; gamma shape mean^2 / S^2 and rate mean / S^2; logistic
//!   location mean and scale S sqrt(3) / pi; Laplace location mean and scale S / sqrt(2);
//!   lognormal scale exp(mean of ln x) and shape the S of ln x; beta shapes m c and (1 - m) c
//!   with c = m (1 - m) / S^2 - 1; Pareto threshold min and shape 1 / (mean of ln x - ln min);
//!   uniform min and max; half-normal scale sqrt(sum of x^2 / n), the maximum likelihood
//!   estimate; Weibull by least squares of ln(-ln(1 - p)) on ln x: the slope is the shape and
//!   exp(-intercept / slope) the scale. CHI and STUDENT take their df as given.
//! - Values outside the distribution's range (negative where it has none, outside 0 to 1 for
//!   beta) leave the variable out with a warning, as does a series that does not vary.
//! - PPLOT does not use case weights. APPLY takes the previous PPLOT's specifications, the
//!   distribution without its parameters, and the subcommands given now override them.

use crate::chart::{Axis, Chart, ChartKind};
use crate::distributions::evaluate;
use crate::plot::{Plot, Point, PointGroup, ReferenceLine};
use crate::{Cell, Engine, Error, Table};
use statscraft_syntax::{Pplot, PplotPlot, RankFraction, RankTies, TestDistribution as D};

/// The specifications of a run: the command over the previous one for APPLY, with defaults.
struct Spec {
    variables: Vec<String>,
    distribution: D,
    parameters: Vec<f64>,
    fraction: RankFraction,
    ties: RankTies,
    standardize: bool,
    pp: bool,
    plot: PplotPlot,
    diff: u32,
    sdiff: u32,
    period: Option<u32>,
    ln: bool,
}

/// The name of a distribution in titles, its function name for `evaluate`, and the names of
/// its parameters.
fn about(d: D) -> (&'static str, &'static str, &'static [&'static str]) {
    match d {
        D::Normal => ("Normal", "NORMAL", &["Location", "Scale"]),
        D::Exponential => ("Exponential", "EXP", &["Rate"]),
        D::Weibull => ("Weibull", "WEIBULL", &["Scale", "Shape"]),
        D::Pareto => ("Pareto", "PARETO", &["Threshold", "Shape"]),
        D::Lognormal => ("Lognormal", "LNORMAL", &["Scale", "Shape"]),
        D::Beta => ("Beta", "BETA", &["Shape 1", "Shape 2"]),
        D::Gamma => ("Gamma", "GAMMA", &["Shape", "Rate"]),
        D::Logistic => ("Logistic", "LOGISTIC", &["Location", "Scale"]),
        D::Laplace => ("Laplace", "LAPLACE", &["Location", "Scale"]),
        D::Uniform => ("Uniform", "UNIFORM", &["Minimum", "Maximum"]),
        D::HalfNormal => ("Half-normal", "HALFNRM", &["Scale"]),
        D::ChiSquare => ("Chi-square", "CHISQ", &["df"]),
        D::StudentT => ("Student t", "T", &["df"]),
    }
}

/// The distribution's function `family` ("CDF" or "IDF") at x.
fn at(d: D, family: &str, x: f64, parameters: &[f64]) -> Option<f64> {
    let mut args = vec![x];
    if d == D::HalfNormal {
        args.push(0.0);
    }
    args.extend(parameters);
    evaluate(&format!("{family}.{}", about(d).1), &args).filter(|v| v.is_finite())
}

fn mean_sd(x: &[f64]) -> (f64, f64) {
    let n = x.len() as f64;
    let mean = x.iter().sum::<f64>() / n;
    let ss: f64 = x.iter().map(|v| (v - mean).powi(2)).sum();
    (mean, (ss / (n - 1.0)).sqrt())
}

/// The rank of every value, ties after `ties`.
pub(crate) fn tied_ranks(x: &[f64], ties: RankTies) -> Vec<f64> {
    let mut order: Vec<usize> = (0..x.len()).collect();
    order.sort_by(|&a, &b| x[a].total_cmp(&x[b]));
    let mut ranks = vec![0.0; x.len()];
    let (mut start, mut distinct) = (0, 0.0);
    while start < order.len() {
        let end = start
            + order[start..]
                .iter()
                .take_while(|&&i| x[i] == x[order[start]])
                .count();
        distinct += 1.0;
        let rank = match ties {
            RankTies::Mean => (start + end + 1) as f64 / 2.0,
            RankTies::Low => start as f64 + 1.0,
            RankTies::High => end as f64,
            RankTies::Condense => distinct,
        };
        for &i in &order[start..end] {
            ranks[i] = rank;
        }
        start = end;
    }
    ranks
}

pub(crate) fn fraction(r: f64, n: f64, f: RankFraction) -> f64 {
    match f {
        RankFraction::Blom => (r - 3.0 / 8.0) / (n + 0.25),
        RankFraction::Rankit => (r - 0.5) / n,
        RankFraction::Tukey => (r - 1.0 / 3.0) / (n + 1.0 / 3.0),
        RankFraction::Vw => r / (n + 1.0),
    }
}

/// The parameters estimated from the values `x` with fractional ranks `p`; None when the
/// values give none, such as values that do not vary.
pub(crate) fn estimate(d: D, x: &[f64], p: &[f64]) -> Option<Vec<f64>> {
    let n = x.len() as f64;
    let (mean, sd) = mean_sd(x);
    let ln: Vec<f64> = x.iter().map(|v| v.ln()).collect();
    let min = x.iter().copied().fold(f64::INFINITY, f64::min);
    let max = x.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let var = sd * sd;
    let out = match d {
        D::Normal => vec![mean, sd],
        D::Exponential => vec![1.0 / mean],
        D::Gamma => vec![mean * mean / var, mean / var],
        D::Logistic => vec![mean, sd * 3f64.sqrt() / std::f64::consts::PI],
        D::Laplace => vec![mean, (var / 2.0).sqrt()],
        D::Lognormal => {
            let (m, s) = mean_sd(&ln);
            vec![m.exp(), s]
        }
        D::Beta => {
            let c = mean * (1.0 - mean) / var - 1.0;
            vec![mean * c, (1.0 - mean) * c]
        }
        D::Pareto => {
            let m = ln.iter().sum::<f64>() / n;
            vec![min, 1.0 / (m - min.ln())]
        }
        D::Uniform => vec![min, max],
        D::HalfNormal => vec![(x.iter().map(|v| v * v).sum::<f64>() / n).sqrt()],
        D::Weibull => {
            let y: Vec<f64> = p.iter().map(|p| (-(1.0 - p).ln()).ln()).collect();
            let (mu, my) = (ln.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
            let sxy: f64 = ln.iter().zip(&y).map(|(u, y)| (u - mu) * (y - my)).sum();
            let sxx: f64 = ln.iter().map(|u| (u - mu).powi(2)).sum();
            let slope = sxy / sxx;
            vec![(-(my - slope * mu) / slope).exp(), slope]
        }
        D::ChiSquare | D::StudentT => return None,
    };
    let valid = match d {
        D::Normal | D::Logistic | D::Laplace => out[1] > 0.0,
        D::Uniform => out[0] < out[1],
        _ => out.iter().all(|v| *v > 0.0),
    };
    (valid && out.iter().all(|v| v.is_finite())).then_some(out)
}

/// Why the values cannot come from the distribution; None when they can.
fn out_of_range(d: D, x: &[f64], estimated: bool) -> Option<&'static str> {
    let negative = x.iter().any(|v| *v < 0.0);
    let not_positive = x.iter().any(|v| *v <= 0.0);
    match d {
        D::Beta if x.iter().any(|v| !(0.0..=1.0).contains(v)) => {
            Some("the beta distribution needs values from 0 to 1")
        }
        D::Lognormal | D::Pareto | D::Weibull if estimated && not_positive => {
            Some("estimating this distribution needs positive values")
        }
        D::Exponential
        | D::Weibull
        | D::Pareto
        | D::Lognormal
        | D::Gamma
        | D::HalfNormal
        | D::ChiSquare
            if negative =>
        {
            Some("this distribution has no negative values")
        }
        _ => None,
    }
}

/// One variable's plotted series: the values in case order after LN and differencing, None
/// where missing, and the counts of user- and system-missing values.
fn series(engine: &Engine, name: &str, spec: &Spec) -> Result<(Vec<Option<f64>>, f64), Error> {
    let data = engine.active()?;
    let variable = data.variable(name)?;
    let values = variable.numeric().map_err(|_| {
        Error(format!(
            "PPLOT needs numeric variables, but {name} is a string variable"
        ))
    })?;
    let mut user = 0.0;
    let mut out: Vec<Option<f64>> = (0..data.case_count())
        .map(|c| {
            let v = values.get(c)?;
            if variable.is_user_missing(v) {
                user += 1.0;
                return None;
            }
            match spec.ln {
                true => (v > 0.0).then(|| v.ln()),
                false => Some(v),
            }
        })
        .collect();
    let difference = |s: &[Option<f64>], lag: usize| -> Vec<Option<f64>> {
        (0..s.len())
            .map(|i| match i.checked_sub(lag) {
                Some(j) => Some(s[i]? - s[j]?),
                None => None,
            })
            .collect()
    };
    for _ in 0..spec.diff {
        out = difference(&out, 1);
    }
    if let Some(period) = spec.period {
        for _ in 0..spec.sdiff {
            out = difference(&out, period as usize);
        }
    }
    Ok((out, user))
}

fn number(v: f64) -> Cell {
    if v.is_finite() {
        Cell::Number(v)
    } else {
        Cell::Empty
    }
}

impl Engine {
    /// The command over the previous PPLOT for APPLY; keeps the result for the next APPLY.
    fn pplot_spec(&mut self, command: &Pplot) -> Result<Spec, Error> {
        let mut given = match &command.apply {
            None => Pplot::default(),
            Some(Some(name)) => {
                return Err(Error(format!(
                    "PPLOT APPLY of the named model '{name}' is not supported yet; APPLY \
                     without a name uses the previous PPLOT"
                )));
            }
            Some(None) => {
                let Some(previous) = &self.pplot_model else {
                    return Err(Error(
                        "PPLOT APPLY: there is no earlier PPLOT to apply".into(),
                    ));
                };
                let mut previous = (**previous).clone();
                // the distribution applies, its parameters do not
                if let Some((_, parameters)) = &mut previous.distribution {
                    parameters.clear();
                }
                previous
            }
        };
        if !command.variables.is_empty() {
            given.variables.clone_from(&command.variables);
        }
        macro_rules! over {
            ($($field:ident),*) => {
                $(if command.$field.is_some() {
                    given.$field.clone_from(&command.$field);
                })*
            };
        }
        over!(
            distribution,
            fraction,
            ties,
            standardize,
            pp,
            plot,
            diff,
            sdiff,
            period,
            ln
        );
        given.apply = None;
        let variables = self.expand(&given.variables)?;
        let (distribution, parameters) = given.distribution.clone().unwrap_or((D::Normal, vec![]));
        let spec = Spec {
            variables,
            distribution,
            parameters,
            fraction: given.fraction.unwrap_or(RankFraction::Blom),
            ties: given.ties.unwrap_or(RankTies::Mean),
            standardize: given.standardize.unwrap_or(false),
            pp: given.pp.unwrap_or(false),
            plot: given.plot.unwrap_or(PplotPlot::Both),
            diff: given.diff.unwrap_or(0),
            sdiff: given.sdiff.unwrap_or(0),
            period: given.period,
            ln: given.ln.unwrap_or(false),
        };
        self.pplot_model = Some(Box::new(given));
        Ok(spec)
    }

    pub(crate) fn pplot(&mut self, command: &Pplot) -> Result<Vec<Table>, Error> {
        let mut spec = self.pplot_spec(command)?;
        if spec.sdiff > 0 && spec.period.is_none() {
            self.warn("PPLOT: SDIFF needs PERIOD, as no periodicity is set; seasonal differencing is left out");
            spec.sdiff = 0;
        }
        let shown = self.shown();
        let (dist_name, _, parameter_names) = about(spec.distribution);
        let estimated = spec.parameters.is_empty();
        let mut charts = Vec::new();
        let mut cases_rows: Vec<Vec<Cell>> = vec![
            vec![Cell::Text("Series length".into())],
            vec![Cell::Text("User-missing values".into())],
            vec![Cell::Text("System-missing values".into())],
            vec![Cell::Text("Values plotted".into())],
        ];
        let mut estimates: Vec<Vec<Cell>> = parameter_names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let first = if i == 0 { dist_name } else { "" };
                vec![Cell::Text(first.into()), Cell::Text((*name).into())]
            })
            .collect();
        let mut headers = Vec::new();
        for name in &spec.variables {
            let (values, user) = series(self, name, &spec)?;
            let data = self.active()?;
            let variable = data.variable(name)?;
            let label = shown.variable(&variable);
            headers.push(label.clone());
            let mut x: Vec<f64> = values.iter().flatten().copied().collect();
            let n = x.len() as f64;
            let length = values.len() as f64;
            for (row, v) in cases_rows
                .iter_mut()
                .zip([length, user, length - user - n, n])
            {
                row.push(Cell::Number(v));
            }
            let mut plotted = None;
            if x.len() < 2 {
                self.warn(format!("PPLOT: {name} has fewer than two values to plot"));
            } else if spec.standardize && mean_sd(&x).1 == 0.0 {
                self.warn(format!(
                    "PPLOT: {name} does not vary, so it cannot be standardized"
                ));
            } else {
                if spec.standardize {
                    let (m, s) = mean_sd(&x);
                    x.iter_mut().for_each(|v| *v = (*v - m) / s);
                }
                let ranks = tied_ranks(&x, spec.ties);
                let p: Vec<f64> = ranks
                    .iter()
                    .map(|&r| fraction(r, n, spec.fraction))
                    .collect();
                if let Some(why) = out_of_range(spec.distribution, &x, estimated) {
                    self.warn(format!("PPLOT: {name} is left out: {why}"));
                } else {
                    let parameters = match estimated {
                        true => estimate(spec.distribution, &x, &p),
                        false => Some(spec.parameters.clone()),
                    };
                    match parameters {
                        Some(parameters) => plotted = Some((x, p, parameters)),
                        None => self.warn(format!(
                            "PPLOT: the {} parameters cannot be estimated from {name}",
                            dist_name.to_lowercase()
                        )),
                    }
                }
            }
            let Some((x, p, parameters)) = plotted else {
                estimates.iter_mut().for_each(|row| row.push(Cell::Empty));
                continue;
            };
            for (row, v) in estimates.iter_mut().zip(&parameters) {
                row.push(number(*v));
            }
            let d = spec.distribution;
            // (observed, expected) per value, in data units (Q-Q) or proportions (P-P)
            let mut pairs: Vec<(f64, f64)> = x
                .iter()
                .zip(&p)
                .filter_map(|(&x, &p)| match spec.pp {
                    true => Some((p, at(d, "CDF", x, &parameters)?)),
                    false => Some((x, at(d, "IDF", p, &parameters)?)),
                })
                .collect();
            pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
            let footnote = format!(
                "Expected values from the {} distribution with {}",
                dist_name.to_lowercase(),
                parameter_names
                    .iter()
                    .zip(&parameters)
                    .map(|(n, v)| format!("{} {}", n.to_lowercase(), short(*v)))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let (kind, detrended_kind, x_title, y_title) = match spec.pp {
                true => (
                    ChartKind::Pp,
                    ChartKind::DetrendedPp,
                    "Observed cumulative probability".to_string(),
                    "Expected cumulative probability".to_string(),
                ),
                false => (
                    ChartKind::Qq,
                    ChartKind::DetrendedQq,
                    "Observed value".to_string(),
                    format!("Expected {} value", dist_name.to_lowercase()),
                ),
            };
            let what = if spec.pp { "P-P" } else { "Q-Q" };
            let make = |kind, title: String, y: String, points: Vec<Point>, line: ReferenceLine| {
                let mut chart = Chart::plot(
                    kind,
                    title,
                    Axis::titled(x_title.clone()),
                    Axis::titled(y),
                    Plot::Points {
                        groups: vec![PointGroup {
                            name: label.clone(),
                            points,
                        }],
                        lines: vec![line],
                        z_axis: None,
                    },
                );
                chart.footnote = Some(footnote.clone());
                chart
            };
            let point = |x: f64, y: f64| Point {
                x,
                y,
                z: None,
                label: None,
            };
            if spec.plot != PplotPlot::Detrended {
                charts.push(make(
                    kind,
                    format!("{dist_name} {what} plot of {label}"),
                    y_title,
                    pairs.iter().map(|&(o, e)| point(o, e)).collect(),
                    ReferenceLine {
                        intercept: 0.0,
                        slope: 1.0,
                    },
                ));
            }
            if spec.plot != PplotPlot::Normal {
                charts.push(make(
                    detrended_kind,
                    format!(
                        "Detrended {} {what} plot of {label}",
                        dist_name.to_lowercase()
                    ),
                    format!("Deviation from {}", dist_name.to_lowercase()),
                    pairs.iter().map(|&(o, e)| point(o, o - e)).collect(),
                    ReferenceLine {
                        intercept: 0.0,
                        slope: 0.0,
                    },
                ));
            }
        }
        let text = |s: &str| Cell::Text(s.to_string());
        let ties = match spec.ties {
            RankTies::Mean => "Mean rank of tied values",
            RankTies::Low => "Lowest rank of tied values",
            RankTies::High => "Highest rank of tied values",
            RankTies::Condense => "Consecutive ranks of distinct values",
        };
        let fraction = match spec.fraction {
            RankFraction::Blom => "Blom",
            RankFraction::Rankit => "Rankit",
            RankFraction::Tukey => "Tukey",
            RankFraction::Vw => "Van der Waerden",
        };
        let mut model = vec![
            vec![text("Variables"), text(&headers.join(", "))],
            vec![
                text("Transformation"),
                text(if spec.ln { "Natural log" } else { "None" }),
            ],
            vec![text("Differencing"), Cell::Number(f64::from(spec.diff))],
            vec![
                text("Seasonal differencing"),
                Cell::Number(f64::from(spec.sdiff)),
            ],
            vec![
                text("Seasonal period"),
                match spec.period.filter(|_| spec.sdiff > 0) {
                    Some(p) => Cell::Number(f64::from(p)),
                    None => text("None"),
                },
            ],
            vec![
                text("Standardized"),
                text(if spec.standardize { "Yes" } else { "No" }),
            ],
            vec![text("Distribution"), text(dist_name)],
        ];
        for (i, name) in parameter_names.iter().enumerate() {
            model.push(vec![
                text(name),
                spec.parameters
                    .get(i)
                    .map_or_else(|| text("Estimated"), |v| Cell::Number(*v)),
            ]);
        }
        model.push(vec![text("Fractional rank"), text(fraction)]);
        model.push(vec![text("Ties"), text(ties)]);
        let weighted = self.active()?.weight_variable().is_some();
        let mut tables = vec![
            Table {
                title: "Model Description".into(),
                row_headers: 1,
                columns: vec![String::new(), "Specification".into()],
                rows: model,
                ..Table::default()
            },
            Table {
                title: "Case Processing Summary".into(),
                row_headers: 1,
                columns: std::iter::once(String::new())
                    .chain(headers.iter().cloned())
                    .collect(),
                rows: cases_rows,
                caption: weighted.then(|| "PPLOT does not use case weights.".to_string()),
                ..Table::default()
            },
        ];
        if estimated && !matches!(spec.distribution, D::ChiSquare | D::StudentT) {
            tables.push(Table {
                title: "Estimated Distribution Parameters".into(),
                row_headers: 2,
                columns: [String::new(), String::new()]
                    .into_iter()
                    .chain(headers.iter().cloned())
                    .collect(),
                rows: estimates,
                ..Table::default()
            });
        }
        self.charts.extend(charts);
        Ok(tables)
    }
}

/// A parameter in a footnote: up to four decimals, without trailing zeros.
fn short(v: f64) -> String {
    let text = format!("{v:.4}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_follow_the_ties_rule() {
        let x = [3.0, 1.0, 3.0, 7.0, 3.0];
        assert_eq!(tied_ranks(&x, RankTies::Mean), [3.0, 1.0, 3.0, 5.0, 3.0]);
        assert_eq!(tied_ranks(&x, RankTies::Low), [2.0, 1.0, 2.0, 5.0, 2.0]);
        assert_eq!(tied_ranks(&x, RankTies::High), [4.0, 1.0, 4.0, 5.0, 4.0]);
        assert_eq!(
            tied_ranks(&x, RankTies::Condense),
            [2.0, 1.0, 2.0, 3.0, 2.0]
        );
    }

    #[test]
    fn fractions_of_rank_two_of_four() {
        let close = |a: f64, b: f64| assert!((a - b).abs() < 1e-12, "{a} {b}");
        close(fraction(2.0, 4.0, RankFraction::Blom), 1.625 / 4.25);
        close(fraction(2.0, 4.0, RankFraction::Rankit), 0.375);
        close(
            fraction(2.0, 4.0, RankFraction::Tukey),
            (5.0 / 3.0) / (13.0 / 3.0),
        );
        close(fraction(2.0, 4.0, RankFraction::Vw), 0.4);
    }
}
