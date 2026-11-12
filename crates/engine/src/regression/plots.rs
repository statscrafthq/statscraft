//! The charts of REGRESSION as data, for the cases of the analysis of one dependent.
//!
//! Rules from the Command Syntax Reference (REGRESSION /RESIDUALS, /SCATTERPLOT, /PARTIALPLOT)
//! and the Algorithms manual (partial residual plots):
//! - HISTOGRAM: a histogram of each temporary variable, with the normal curve.
//! - NORMPROB: a normal P-P plot, always of a standardized variable: PRED, RESID and DRESID are
//!   shown as ZPRED, ZRESID and SDRESID.
//! - SCATTERPLOT (y, x): the first variable along the vertical axis.
//! - PARTIALPLOT: for each independent, the residuals of the dependent against those of the
//!   independent, both regressed on the other independents; it needs two independents in the
//!   equation. Without a list: every independent, in the order of the equation.
//!
//! Where the manuals are silent: the P-P plot puts the cumulative share of the weight up to
//! each distinct value against the normal probability of that value; the regressions of the
//! partial plots use the weights of the analysis and keep its constant (none under ORIGIN).

use super::{CaseValues, Equation, Fit, label_of};
use crate::Chart;
use crate::chart::{Axis, ChartKind};
use crate::dist::normal_cdf;
use crate::frequencies::Histogram;
use crate::matrix::inverse;
use crate::plot::{Plot, Point, PointGroup, ReferenceLine};
use statscraft_syntax::{PlotVariable, Regression, TempVar};

/// A variable of the data by name: its title and its values.
pub(super) type Lookup<'a> = dyn Fn(&str) -> Option<(String, Vec<f64>)> + 'a;

/// What the charts of one equation are made from.
pub(super) struct Plots<'a> {
    pub command: &'a Regression,
    pub eq: &'a Equation,
    pub fit: &'a Fit,
    /// The value of variable j of the equation in case k.
    pub x: &'a dyn Fn(usize, usize) -> f64,
    pub rows: &'a [Option<CaseValues>],
    /// Case weights and REGWGT.
    pub c: &'a [f64],
    pub g: &'a [f64],
    pub in_fit: &'a [bool],
    /// A variable of the data by name: its title and its values.
    pub variable: &'a Lookup<'a>,
    /// The independents PARTIALPLOT names, expanded; empty for all.
    pub partial: &'a [String],
}

impl Plots<'_> {
    fn cases(&self) -> Vec<usize> {
        (0..self.c.len())
            .filter(|&k| self.in_fit[k] && self.rows[k].is_some())
            .collect()
    }

    fn dependent(&self) -> &str {
        &self.eq.texts[self.eq.y()]
    }

    /// (value, case weight) of a temporary variable over the cases, in ascending order.
    fn temp(&self, var: TempVar) -> Vec<(f64, f64)> {
        let mut pairs: Vec<(f64, f64)> = self
            .cases()
            .into_iter()
            .filter_map(|k| {
                let v = self.rows[k].as_ref()?.get(var);
                (!v.is_nan()).then_some((v, self.c[k]))
            })
            .collect();
        pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
        pairs
    }

    pub(super) fn charts(&self, warnings: &mut Vec<String>) -> Vec<Chart> {
        let mut charts = Vec::new();
        let footnote = Some(format!("Dependent variable: {}", self.dependent()));
        for &var in &self.command.histograms {
            let pairs = self.temp(var);
            let title = label_of(var).2;
            let histogram = Histogram {
                title: &format!("Histogram of the {}", title.to_lowercase()),
                heading: title,
                format: "",
                name: label_of(var).1,
                normal: true,
                scale: None,
            };
            let valid = pairs.iter().map(|p| p.1).sum();
            match histogram.chart(&pairs, &pairs, valid) {
                Ok(mut chart) => {
                    let stats = chart.footnote.take().unwrap_or_default();
                    chart.footnote =
                        Some(format!("{stats}. Dependent variable: {}", self.dependent()));
                    charts.push(chart);
                }
                Err(why) => {
                    warnings.push(format!("no histogram of {}: {why}", title.to_lowercase()))
                }
            }
        }
        for &var in &self.command.normprobs {
            let var = match var {
                TempVar::Pred => TempVar::Zpred,
                TempVar::Resid => TempVar::Zresid,
                TempVar::Dresid => TempVar::Sdresid,
                other => other,
            };
            let pairs = self.temp(var);
            let total: f64 = pairs.iter().map(|p| p.1).sum();
            let mut points: Vec<Point> = Vec::new();
            let mut cumulative = 0.0;
            for (i, &(v, w)) in pairs.iter().enumerate() {
                cumulative += w;
                if pairs.get(i + 1).is_some_and(|next| next.0 == v) {
                    continue;
                }
                points.push(Point {
                    x: cumulative / total,
                    y: normal_cdf(v),
                    z: None,
                    label: None,
                });
            }
            let mut chart = Chart::plot(
                ChartKind::NormalPp,
                format!("Normal P-P plot of the {}", label_of(var).2.to_lowercase()),
                Axis::titled("Observed cumulative probability"),
                Axis::titled("Expected cumulative probability"),
                Plot::Points {
                    groups: vec![PointGroup {
                        name: label_of(var).2.into(),
                        points,
                    }],
                    lines: vec![ReferenceLine {
                        intercept: 0.0,
                        slope: 1.0,
                    }],
                    z_axis: None,
                },
            );
            chart.footnote = footnote.clone();
            charts.push(chart);
        }
        for (y, x) in &self.command.scatterplots {
            let (Some((y_title, ys)), Some((x_title, xs))) = (self.values(y), self.values(x))
            else {
                let missing = [y, x]
                    .into_iter()
                    .find(|v| self.values(v).is_none())
                    .map_or(String::new(), name_of);
                warnings.push(format!(
                    "SCATTERPLOT: {missing} is not a variable of the analysis, so its plot is left out"
                ));
                continue;
            };
            let points = self
                .cases()
                .into_iter()
                .filter(|&k| !xs[k].is_nan() && !ys[k].is_nan())
                .map(|k| Point {
                    x: xs[k],
                    y: ys[k],
                    z: None,
                    label: None,
                })
                .collect();
            let mut chart = Chart::plot(
                ChartKind::Scatter,
                format!("Scatterplot of {y_title} against {x_title}"),
                Axis::titled(x_title.clone()),
                Axis::titled(y_title.clone()),
                Plot::Points {
                    groups: vec![PointGroup {
                        name: y_title,
                        points,
                    }],
                    lines: Vec::new(),
                    z_axis: None,
                },
            );
            chart.footnote = footnote.clone();
            charts.push(chart);
        }
        if self.command.partial.is_some() {
            charts.extend(self.partial_plots(warnings));
        }
        charts
    }

    /// A scatterplot variable's title and its value in every case, NaN where it has none.
    fn values(&self, v: &PlotVariable) -> Option<(String, Vec<f64>)> {
        match v {
            PlotVariable::Temp(var) => Some((
                label_of(*var).2.to_string(),
                self.rows
                    .iter()
                    .map(|r| r.as_ref().map_or(f64::NAN, |r| r.get(*var)))
                    .collect(),
            )),
            PlotVariable::Variable(name) => (self.variable)(name),
        }
    }

    fn partial_plots(&self, warnings: &mut Vec<String>) -> Vec<Chart> {
        let inside: Vec<usize> = self.fit.b.iter().map(|b| b.0).collect();
        if inside.len() < 2 {
            warnings.push(format!(
                "PARTIALPLOT needs two independents in the equation of {}, so no partial plot is made",
                self.dependent()
            ));
            return Vec::new();
        }
        let mut wanted = inside.clone();
        if !self.partial.is_empty() {
            wanted.clear();
            for name in self.partial {
                match inside
                    .iter()
                    .find(|&&j| self.eq.names[j].eq_ignore_ascii_case(name))
                {
                    Some(&j) => wanted.push(j),
                    None => warnings.push(format!(
                        "PARTIALPLOT: {name} is not in the equation of {}, so it gets no plot",
                        self.dependent()
                    )),
                }
            }
        }
        let cases = self.cases();
        let y = self.eq.y();
        let weight = |k: usize| self.c[k] * self.g[k];
        let mut charts = Vec::new();
        for j in wanted {
            let others: Vec<usize> = inside.iter().copied().filter(|&o| o != j).collect();
            let fitted =
                |target: usize| residuals(&cases, &others, target, self.x, &weight, self.eq.origin);
            let (Some(ry), Some(rx)) = (fitted(y), fitted(j)) else {
                continue;
            };
            let points = rx
                .iter()
                .zip(&ry)
                .map(|(&x, &y)| Point {
                    x,
                    y,
                    z: None,
                    label: None,
                })
                .collect();
            let mut chart = Chart::plot(
                ChartKind::Scatter,
                format!(
                    "Partial regression plot of {} and {}",
                    self.dependent(),
                    self.eq.texts[j]
                ),
                Axis::titled(self.eq.texts[j].clone()),
                Axis::titled(self.dependent().to_string()),
                Plot::Points {
                    groups: vec![PointGroup {
                        name: self.eq.texts[j].clone(),
                        points,
                    }],
                    lines: Vec::new(),
                    z_axis: None,
                },
            );
            chart.footnote = Some(format!(
                "Residuals of both after a regression on the other independents. Dependent variable: {}",
                self.dependent()
            ));
            charts.push(chart);
        }
        charts
    }
}

fn name_of(v: &PlotVariable) -> String {
    match v {
        PlotVariable::Temp(var) => format!("*{}", label_of(*var).1),
        PlotVariable::Variable(name) => name.clone(),
    }
}

/// The residuals of variable `target` regressed by weighted least squares on `predictors`
/// (with a constant unless `origin`), in the order of `cases`; None when the predictors are
/// collinear.
fn residuals(
    cases: &[usize],
    predictors: &[usize],
    target: usize,
    x: &dyn Fn(usize, usize) -> f64,
    weight: &dyn Fn(usize) -> f64,
    origin: bool,
) -> Option<Vec<f64>> {
    let row = |k: usize| -> Vec<f64> {
        let constant = (!origin).then_some(1.0);
        constant
            .into_iter()
            .chain(predictors.iter().map(|&j| x(j, k)))
            .collect()
    };
    let p = predictors.len() + usize::from(!origin);
    let mut xtx = vec![vec![0.0; p]; p];
    let mut xty = vec![0.0; p];
    for &k in cases {
        let (r, w, t) = (row(k), weight(k), x(target, k));
        for a in 0..p {
            xty[a] += w * r[a] * t;
            for b in 0..p {
                xtx[a][b] += w * r[a] * r[b];
            }
        }
    }
    let (inv, _) = inverse(&xtx)?;
    let b: Vec<f64> = (0..p)
        .map(|a| (0..p).map(|c| inv[a][c] * xty[c]).sum())
        .collect();
    Some(
        cases
            .iter()
            .map(|&k| {
                let r = row(k);
                x(target, k) - r.iter().zip(&b).map(|(v, b)| v * b).sum::<f64>()
            })
            .collect(),
    )
}
