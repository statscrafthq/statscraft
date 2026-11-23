//! The plots of MANOVA, as chart data.
//!
//! PLOT (once for the procedure, from the cases): BOXPLOTS, a box of each non-empty cell for every
//! continuous variable; CELLPLOTS, the cell means against the cell variances and against the
//! cell standard deviations and a histogram of the cell means; NORMAL, the normal and the
//! detrended normal plots of every variable's scores; ZCORR, the half-normal plot of Fisher's z of
//! the within-cells correlations of the dependents. Per design, PMEANS PLOT draws the predicted
//! means of the cells and RESIDUALS PLOT the standardized residuals against the observed and the
//! predicted values and the case numbers, with their normal and detrended normal plots.
//!
//! Choices where the manual is silent: the expected normal deviate of the i-th of n scores is the
//! normal quantile of i / (n + 1), as in the residual plots of the other procedures; the expected
//! half-normal deviate of the i-th of m values of |z| is the normal quantile of
//! 0.5 + 0.5 i / (m + 1).

use super::*;
use crate::chart::{Axis, Chart, ChartKind};
use crate::dist::normal_quantile;
use crate::frequencies::Histogram;
use crate::plot::{BoxSummary, Plot, Point, PointGroup, ReferenceLine};

fn point(x: f64, y: f64, label: Option<String>) -> Point {
    Point {
        x,
        y,
        z: None,
        label,
    }
}

fn scatter(
    title: String,
    x: &str,
    y: &str,
    name: &str,
    points: Vec<Point>,
    line: Option<ReferenceLine>,
) -> Chart {
    Chart::plot(
        ChartKind::Scatter,
        title,
        Axis::titled(x),
        Axis::titled(y),
        Plot::Points {
            groups: vec![PointGroup {
                name: name.to_string(),
                points,
            }],
            lines: line.into_iter().collect(),
            z_axis: None,
        },
    )
}

/// The name of a cell: its factors' values, or All without factors.
pub(super) fn cell_label(factors: &[Factor], levels: &[usize]) -> String {
    match factors.is_empty() {
        true => "All".to_string(),
        false => factors
            .iter()
            .zip(levels)
            .map(|(f, l)| format!("{}={}", f.name, f.min + *l as f64))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// The normal and the detrended normal plot of `values`, named by `what`: the i-th smallest of n
/// against the normal quantile of i / (n + 1).
fn normal_charts(what: &str, axis: &str, values: &[f64]) -> Vec<Chart> {
    let mut sorted: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if sorted.is_empty() {
        return Vec::new();
    }
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len() as f64;
    let scores: Vec<f64> = (1..=sorted.len())
        .map(|i| normal_quantile(i as f64 / (n + 1.0)))
        .collect();
    let draw = |kind: ChartKind, title: String, y: &str, deviation: bool, slope: f64| {
        let points: Vec<Point> = sorted
            .iter()
            .zip(&scores)
            .map(|(&v, &s)| point(v, if deviation { v - s } else { s }, None))
            .collect();
        let mut chart = scatter(
            title,
            axis,
            y,
            axis,
            points,
            Some(ReferenceLine {
                intercept: 0.0,
                slope,
            }),
        );
        chart.kind = kind;
        chart
    };
    vec![
        draw(
            ChartKind::NormalQq,
            format!("MANOVA: normal Q-Q plot of {what}"),
            "Expected normal",
            false,
            1.0,
        ),
        draw(
            ChartKind::DetrendedNormalQq,
            format!("MANOVA: detrended normal Q-Q plot of {what}"),
            "Deviation from normal",
            true,
            0.0,
        ),
    ]
}

/// The charts PLOT asks for, with the warnings about what could not be drawn.
pub(super) fn cell_charts(s: &Setup, c: &Manova, ndeps: usize) -> (Vec<Chart>, Vec<String>) {
    {
        let plots = c.plots;
        let mut warnings = Vec::new();
        let mut charts = Vec::new();
        let groups = s.groups();
        for (v, name) in s.cont.iter().enumerate() {
            if plots.boxplots {
                let boxes: Vec<BoxSummary> = groups
                    .iter()
                    .filter_map(|g| {
                        let values: Vec<(f64, usize)> =
                            g.iter().map(|c| (c.v[v], c.index)).collect();
                        crate::neural::box_of(
                            cell_label(s.factors, &g[0].levels),
                            String::new(),
                            &values,
                        )
                        .map(|b| BoxSummary { group: None, ..b })
                    })
                    .collect();
                if !boxes.is_empty() {
                    charts.push(Chart::plot(
                        ChartKind::Boxplot,
                        format!("MANOVA: boxplots of {name} by cell"),
                        Axis::titled("Cell"),
                        Axis::titled(name.clone()),
                        Plot::Boxes { boxes },
                    ));
                }
            }
            if plots.cellplots {
                let stats: Vec<(String, f64, f64)> = groups
                    .iter()
                    .map(|g| {
                        let (mean, sd, _) = moments(g, v);
                        (cell_label(s.factors, &g[0].levels), mean, sd)
                    })
                    .collect();
                for (what, y, square) in [
                    ("variances", "Variance", true),
                    ("standard deviations", "Standard deviation", false),
                ] {
                    let points = stats
                        .iter()
                        .filter(|(_, _, sd)| sd.is_finite())
                        .map(|(label, mean, sd)| {
                            point(
                                *mean,
                                if square { sd * sd } else { *sd },
                                Some(label.clone()),
                            )
                        })
                        .collect();
                    charts.push(scatter(
                        format!("MANOVA: cell means of {name} against cell {what}"),
                        &format!("Mean of {name}"),
                        y,
                        "Cells",
                        points,
                        None,
                    ));
                }
                let mut means: Vec<(f64, f64)> = stats.iter().map(|(_, m, _)| (*m, 1.0)).collect();
                means.sort_by(|a, b| a.0.total_cmp(&b.0));
                let histogram = Histogram {
                    title: &format!("MANOVA: histogram of the cell means of {name}"),
                    heading: &format!("Cell mean of {name}"),
                    format: "F8.2",
                    name,
                    normal: false,
                    scale: None,
                };
                if let Ok(chart) = histogram.chart(&means, &means, means.len() as f64) {
                    charts.push(chart);
                }
            }
            if plots.normal {
                let scores: Vec<f64> = s.cases.iter().map(|c| c.v[v]).collect();
                charts.extend(normal_charts(
                    &format!("the scores of {name}"),
                    name,
                    &scores,
                ));
            }
        }
        if plots.zcorr {
            match zcorr_chart(s, ndeps) {
                Ok(chart) => charts.push(chart),
                Err(why) => warnings.push(format!("MANOVA: PLOT ZCORR {why}")),
            }
        }
        (charts, warnings)
    }
}

/// The half-normal plot of Fisher's z of the within-cells correlations of the dependents.
fn zcorr_chart(s: &Setup, ndeps: usize) -> Result<Chart, String> {
    {
        if ndeps < 2 {
            return Err("needs two or more dependent variables".into());
        }
        let groups = s.groups();
        let k = ndeps;
        let mut w = vec![vec![0.0; k]; k];
        let cells = groups.len() as f64;
        let n: f64 = s.cases.iter().map(|c| c.w).sum();
        for g in &groups {
            let means: Vec<f64> = (0..k).map(|v| moments(g, v).0).collect();
            for case in g {
                for i in 0..k {
                    for j in 0..k {
                        w[i][j] += case.w * (case.v[i] - means[i]) * (case.v[j] - means[j]);
                    }
                }
            }
        }
        if n - cells <= 3.0 {
            return Err("needs more than 3 degrees of freedom within the cells".into());
        }
        let mut z: Vec<(f64, String)> = Vec::new();
        for i in 0..k {
            for j in i + 1..k {
                let r = w[i][j] / (w[i][i] * w[j][j]).sqrt();
                if r.is_finite() && r.abs() < 1.0 {
                    z.push((r.atanh().abs(), format!("{} with {}", s.cont[i], s.cont[j])));
                }
            }
        }
        z.sort_by(|a, b| a.0.total_cmp(&b.0));
        let m = z.len() as f64;
        let points = z
            .iter()
            .enumerate()
            .map(|(i, (v, label))| {
                point(
                    normal_quantile(0.5 + 0.5 * (i + 1) as f64 / (m + 1.0)),
                    *v,
                    Some(label.clone()),
                )
            })
            .collect();
        Ok(scatter(
            "MANOVA: half-normal plot of the within-cells correlations (Fisher's z)".into(),
            "Expected half-normal deviate",
            "Absolute value of Fisher's z",
            "Correlations",
            points,
            None,
        ))
    }
}

/// The plot of the predicted cell means: the adjusted predicted mean of each dependent in each
/// non-empty cell, the cells numbered in order.
pub(super) fn pmeans_chart(names: &[String], means: &[(String, Vec<f64>)]) -> Chart {
    let groups = names
        .iter()
        .enumerate()
        .map(|(j, name)| PointGroup {
            name: name.clone(),
            points: means
                .iter()
                .enumerate()
                .map(|(i, (label, v))| point((i + 1) as f64, v[j], Some(label.clone())))
                .collect(),
        })
        .collect();
    Chart::plot(
        ChartKind::Scatter,
        "MANOVA: predicted means of the cells".into(),
        Axis::titled("Cell number"),
        Axis::titled("Predicted mean"),
        Plot::Points {
            groups,
            lines: Vec::new(),
            z_axis: None,
        },
    )
}

/// One case's fit of a dependent.
pub(super) struct Fitted {
    pub case: usize,
    /// The index of the dependent.
    pub variable: usize,
    pub observed: f64,
    pub predicted: f64,
    pub standardized: f64,
}

/// The five plots of the standardized residuals of a dependent.
pub(super) fn residual_charts(name: &str, rows: &[Fitted]) -> Vec<Chart> {
    let line = Some(ReferenceLine {
        intercept: 0.0,
        slope: 0.0,
    });
    let against = |what: &str, x: &dyn Fn(&Fitted) -> f64| {
        scatter(
            format!("MANOVA: standardized residuals of {name} against {what}"),
            what,
            "Standardized residual",
            name,
            rows.iter()
                .map(|r| point(x(r), r.standardized, None))
                .collect(),
            line.clone(),
        )
    };
    let mut out = vec![
        against("observed values", &|r| r.observed),
        against("predicted values", &|r| r.predicted),
        against("case numbers", &|r| (r.case + 1) as f64),
    ];
    let residuals: Vec<f64> = rows.iter().map(|r| r.standardized).collect();
    out.extend(normal_charts(
        &format!("the standardized residuals of {name}"),
        "Standardized residual",
        &residuals,
    ));
    out
}
