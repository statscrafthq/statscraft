//! The plots of EXAMINE as data.
//!
//! Rules from the Command Syntax Reference (EXAMINE /PLOT, /COMPARE, /ID) and the Algorithms
//! manual (EXAMINE plots, spread versus level, Levene):
//! - Boxplot: the box runs between Tukey's hinges, with the median inside. With STEP 1.5 times
//!   the box length, a case at least STEP and less than 2 STEP beyond a hinge is an outlier,
//!   one 2 STEP or more beyond is an extreme. Cases are labeled with their case number, or
//!   with the /ID variable (its value label when it has one).
//! - Normal Q-Q: every distinct value y_i against Psi^-1(R_i / (W + 1)), R_i its rank (the
//!   mean rank of ties, by weight). Detrended: y_i against Z_i minus that normal score.
//! - Spread versus level: ln of the interquartile range against ln of the median of every
//!   cell; with a power a, the interquartile range and median of y^a (ln y for 0). The slope of
//!   spread on level is shown; no plot when a value cannot be transformed or a logarithm is
//!   taken of a value at or below 0. Levene's tests use the same (transformed) data.
//!
//! Where the manuals are silent:
//! - Whiskers reach the furthest values that are neither outliers nor extremes, never inside
//!   the box. A case equal to a hinge is never marked, which matters when the box has no length.
//! - Stem-and-leaf: the cases beyond the boxplot's outlier bounds are counted as extremes.
//!   About 10 log10(n) lines; the width of a line is 1, 2 or 5 times a power of ten, giving
//!   one, five or two lines per stem. A leaf is the digit after the stem, cut (not rounded);
//!   each leaf stands for one case, or for 2, 5, 10, ... cases when a line would hold more
//!   than 50 leaves. Weighted cases give leaves for their weight, rounded.
//! - The power estimate under spread versus level without a power is 1 minus the slope.
//! - Normal plots need at least two cases with different values.

use super::{CellResult, Distribution, Tables, short};
use crate::chart::{Axis, Chart, ChartKind, number_text};
use crate::descriptives::summarize;
use crate::dist::{f_p, normal_quantile};
use crate::format::{Stat, procedure_format};
use crate::frequencies::Histogram;
use crate::glm::levene_of;
use crate::plot::{BoxSummary, Extremes, Mark, Plot, Point, PointGroup, ReferenceLine};
use crate::plot::{StemLeaf, StemLine};
use crate::{Cell, Table};

/// Where a value lies against the boxplot's bounds.
#[derive(Clone, Copy, PartialEq)]
enum Place {
    Inside,
    LowOutlier,
    LowExtreme,
    HighOutlier,
    HighExtreme,
}

/// Tukey's hinges and the rule that marks cases.
struct Fences {
    hinges: [f64; 3],
    step: f64,
}

impl Fences {
    fn of(r: &CellResult) -> Option<Fences> {
        let hinges = Distribution::new(&r.pairs).hinges()?;
        Some(Fences {
            hinges,
            step: 1.5 * (hinges[2] - hinges[0]),
        })
    }

    fn place(&self, y: f64) -> Place {
        let ([q1, _, q3], step) = (self.hinges, self.step);
        if y > q3 && y >= q3 + 2.0 * step {
            Place::HighExtreme
        } else if y > q3 && y >= q3 + step {
            Place::HighOutlier
        } else if y < q1 && y <= q1 - 2.0 * step {
            Place::LowExtreme
        } else if y < q1 && y <= q1 - step {
            Place::LowOutlier
        } else {
            Place::Inside
        }
    }
}

impl Tables<'_> {
    /// "region = North, size = Large", empty for the model of all cases.
    fn cell_name(&self, r: &CellResult) -> String {
        let parts: Vec<String> = (self.names.iter().zip(&r.labels[1..]))
            .map(|(name, level)| format!("{name} = {level}"))
            .collect();
        parts.join(", ")
    }

    /// The dependent, with the cell when there is one.
    fn subject(&self, r: &CellResult) -> String {
        match self.cell_name(r) {
            cell if cell.is_empty() => r.labels[0].clone(),
            cell => format!("{} ({cell})", r.labels[0]),
        }
    }

    fn title(&self, what: &str, r: &CellResult) -> String {
        format!("{what} of {}", self.subject(r))
    }

    fn case_text(&self, case: usize) -> String {
        match self.case_label(case) {
            Cell::Number(n) => short(n),
            Cell::Text(t) => t,
            Cell::Empty => String::new(),
        }
    }

    fn value_axis(&self, r: &CellResult, title: String) -> Axis {
        Axis {
            title,
            format: procedure_format(&r.format, Stat::Value),
            maximum: None,
            variable: None,
        }
    }

    /// The charts and tables of /PLOT for the cells of one model: per dependent the
    /// histogram, stem-and-leaf and normal plots of every cell, then the boxplots and the
    /// spread-versus-level plot.
    pub(super) fn plots(
        &self,
        results: &[&CellResult],
        charts: &mut Vec<Chart>,
        tables: &mut Vec<Table>,
        warnings: &mut Vec<String>,
    ) {
        let asked = |name: &str| self.command.plots.iter().any(|p| p == name);
        let dependents = results
            .iter()
            .map(|r| r.dependent)
            .max()
            .map_or(0, |d| d + 1);
        for d in 0..dependents {
            let cells: Vec<&CellResult> = results
                .iter()
                .copied()
                .filter(|r| r.dependent == d)
                .collect();
            for r in &cells {
                if asked("HISTOGRAM") {
                    let mut pairs = r.pairs.clone();
                    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
                    let histogram = Histogram {
                        title: &self.title("Histogram", r),
                        heading: &r.labels[0],
                        format: &r.format,
                        name: &r.labels[0],
                        normal: false,
                        scale: None,
                    };
                    if let Ok(chart) = histogram.chart(&pairs, &pairs, r.valid) {
                        charts.push(chart);
                    }
                }
                if asked("STEMLEAF") {
                    charts.extend(self.stem_leaf(r));
                }
                if self.command.normality {
                    match self.normal_plots(r) {
                        Some(two) => charts.extend(two),
                        None => warnings.push(format!(
                            "no normal Q-Q plots of {}: they need values that vary",
                            self.subject(r)
                        )),
                    }
                }
            }
            if asked("BOXPLOT") && !self.command.compare_variables {
                let boxes = cells
                    .iter()
                    .filter_map(|r| self.box_summary(r, None))
                    .collect();
                let title = match self.names.is_empty() {
                    true => format!("Boxplot of {}", cells[0].labels[0]),
                    false => format!(
                        "Boxplot of {} by {}",
                        cells[0].labels[0],
                        self.names.join(" by ")
                    ),
                };
                let x = Axis::titled(self.names.join(" * "));
                let y = self.value_axis(cells[0], cells[0].labels[0].clone());
                charts.push(Chart::plot(
                    ChartKind::Boxplot,
                    title,
                    x,
                    y,
                    Plot::Boxes { boxes },
                ));
            }
        }
        if asked("BOXPLOT") && self.command.compare_variables {
            // one plot of every dependent, clustered by cell when there are factors
            let factors = !self.names.is_empty();
            let mut ordered: Vec<&CellResult> = results.to_vec();
            ordered.sort_by_key(|r| self.cell_name(r));
            if factors {
                // cells in their first order, dependents within
                let first: Vec<String> = results.iter().map(|r| self.cell_name(r)).collect();
                ordered.sort_by_key(|r| first.iter().position(|c| *c == self.cell_name(r)));
            }
            let boxes = ordered
                .iter()
                .filter_map(|r| match factors {
                    true => self.box_summary(r, Some(r.labels[0].clone())),
                    false => self.box_summary(r, None).map(|mut b| {
                        b.label = r.labels[0].clone();
                        b
                    }),
                })
                .collect();
            let title = match factors {
                true => format!("Boxplot by {}", self.names.join(" by ")),
                false => "Boxplot".into(),
            };
            let x = Axis::titled(self.names.join(" * "));
            let y = self.value_axis(results[0], String::new());
            let mut chart = Chart::plot(ChartKind::Boxplot, title, x, y, Plot::Boxes { boxes });
            if factors {
                chart.legend = Some("Variable".into());
            }
            charts.push(chart);
        }
        if let Some(power) = self.command.spread_level
            && !self.names.is_empty()
        {
            for d in 0..dependents {
                let cells: Vec<&CellResult> = results
                    .iter()
                    .copied()
                    .filter(|r| r.dependent == d)
                    .collect();
                match self.spread_level(&cells, power) {
                    Ok((chart, rows)) => {
                        charts.push(chart);
                        tables.push(levene_table(rows, cells[0], power));
                    }
                    Err(why) => warnings.push(format!(
                        "no spread-versus-level plot of {} by {}: {why}",
                        cells[0].labels[0],
                        self.names.join(" by ")
                    )),
                }
            }
        }
    }

    /// The box of one cell; None without cases.
    fn box_summary(&self, r: &CellResult, group: Option<String>) -> Option<BoxSummary> {
        let fences = Fences::of(r)?;
        let [q1, median, q3] = fences.hinges;
        let (mut outliers, mut extremes) = (Vec::new(), Vec::new());
        let (mut low, mut high) = (q1, q3);
        let mut cases = r.cases.clone();
        cases.sort_by(|a, b| a.0.total_cmp(&b.0));
        for &(y, case) in &cases {
            let mark = || Mark {
                value: y,
                case: self.case_text(case),
            };
            match fences.place(y) {
                Place::Inside => (low, high) = (low.min(y), high.max(y)),
                Place::LowOutlier | Place::HighOutlier => outliers.push(mark()),
                Place::LowExtreme | Place::HighExtreme => extremes.push(mark()),
            }
        }
        let label = match self.cell_name(r) {
            cell if cell.is_empty() => r.labels[0].clone(),
            _ => r.labels[1..].join(", "),
        };
        Some(BoxSummary {
            label,
            group,
            n: r.valid,
            lower_whisker: low,
            lower_hinge: q1,
            median,
            upper_hinge: q3,
            upper_whisker: high,
            outliers,
            extremes,
        })
    }

    fn stem_leaf(&self, r: &CellResult) -> Option<Chart> {
        let fences = Fences::of(r)?;
        let mut inner = Vec::new();
        let (mut low, mut high): (Option<Extremes>, Option<Extremes>) = (None, None);
        for (&(y, _), &(_, w)) in r.cases.iter().zip(&r.pairs) {
            let add = |e: &mut Option<Extremes>, bound: fn(f64, f64) -> f64| {
                let e = e.get_or_insert(Extremes {
                    frequency: 0.0,
                    bound: y,
                });
                e.frequency += w;
                e.bound = bound(e.bound, y);
            };
            match fences.place(y) {
                Place::Inside => inner.push((y, w)),
                Place::LowOutlier | Place::LowExtreme => add(&mut low, f64::max),
                Place::HighOutlier | Place::HighExtreme => add(&mut high, f64::min),
            }
        }
        let mut display = stem_leaf(&inner);
        display.low_extremes = low;
        display.high_extremes = high;
        let title = self.title("Stem-and-leaf plot", r);
        let mut chart = Chart::plot(
            ChartKind::StemLeaf,
            title,
            self.value_axis(r, r.labels[0].clone()),
            Axis::titled("Frequency"),
            Plot::StemLeaf(display.clone()),
        );
        chart.footnote = Some(format!(
            "Stem width: {}; each leaf: {} case(s)",
            short(display.stem_width),
            short(display.each_leaf)
        ));
        Some(chart)
    }

    /// The normal Q-Q plot and the detrended one; None when the values do not vary.
    fn normal_plots(&self, r: &CellResult) -> Option<[Chart; 2]> {
        let s = summarize(&r.pairs);
        let (mean, sd) = s.mean.zip(s.sd.filter(|sd| *sd > 0.0 && s.n > 1.0))?;
        let dist = Distribution::new(&r.pairs);
        let w = dist.total;
        let (mut qq, mut detrended) = (Vec::new(), Vec::new());
        for (i, &y) in dist.values.iter().enumerate() {
            let rank = dist.cc(i) + (dist.weights[i] + 1.0) / 2.0;
            let score = normal_quantile(rank / (w + 1.0));
            qq.push(Point {
                x: y,
                y: score,
                z: None,
                label: None,
            });
            detrended.push(Point {
                x: y,
                y: (y - mean) / sd - score,
                z: None,
                label: None,
            });
        }
        let group = |points| {
            vec![PointGroup {
                name: r.labels[0].clone(),
                points,
            }]
        };
        let x = self.value_axis(r, "Observed value".into());
        let mut normal = Chart::plot(
            ChartKind::NormalQq,
            self.title("Normal Q-Q plot", r),
            x.clone(),
            Axis::titled("Expected normal"),
            Plot::Points {
                groups: group(qq),
                lines: vec![ReferenceLine {
                    intercept: -mean / sd,
                    slope: 1.0 / sd,
                }],
                z_axis: None,
            },
        );
        normal.footnote = Some(format!(
            "The line is the normal distribution with mean {} and standard deviation {}",
            number_text(mean, &procedure_format(&r.format, Stat::Value)),
            number_text(sd, &procedure_format(&r.format, Stat::Spread))
        ));
        let detrended = Chart::plot(
            ChartKind::DetrendedNormalQq,
            self.title("Detrended normal Q-Q plot", r),
            x,
            Axis::titled("Deviation from normal"),
            Plot::Points {
                groups: group(detrended),
                lines: vec![ReferenceLine {
                    intercept: 0.0,
                    slope: 0.0,
                }],
                z_axis: None,
            },
        );
        Some([normal, detrended])
    }

    /// The plot of one dependent over the cells, and the cells' (transformed) values for the
    /// Levene tests.
    #[allow(clippy::type_complexity)]
    fn spread_level(
        &self,
        cells: &[&CellResult],
        power: Option<f64>,
    ) -> Result<(Chart, Vec<Vec<(f64, f64)>>), String> {
        let values: Vec<f64> = cells
            .iter()
            .flat_map(|r| r.pairs.iter().map(|p| p.0))
            .collect();
        let any = |bad: fn(f64) -> bool| values.iter().any(|&y| bad(y));
        let whole = power.is_some_and(|a| a.fract() == 0.0);
        let refused = match power {
            Some(0.0) => any(|y| y <= 0.0),
            Some(a) if a < 0.0 && whole => any(|y| y == 0.0),
            Some(a) if a < 0.0 => any(|y| y <= 0.0),
            Some(_) if !whole => any(|y| y < 0.0),
            _ => false,
        };
        if refused {
            return Err("some values cannot take the power transformation".into());
        }
        let transform = |y: f64| match power {
            Some(0.0) => y.ln(),
            Some(a) => y.powf(a),
            None => y,
        };
        let groups: Vec<Vec<(f64, f64)>> = cells
            .iter()
            .map(|r| r.pairs.iter().map(|&(y, w)| (transform(y), w)).collect())
            .collect();
        let method = self.command.method;
        let mut points = Vec::new();
        for (r, pairs) in cells.iter().zip(&groups) {
            let dist = Distribution::new(pairs);
            let q = |p| dist.percentile(p, method);
            let (Some(median), Some(q1), Some(q3)) = (q(50.0), q(25.0), q(75.0)) else {
                continue;
            };
            let (level, spread) = match power {
                None if median <= 0.0 || q3 - q1 <= 0.0 => {
                    return Err("a median or an interquartile range is not above 0".into());
                }
                None => (median.ln(), (q3 - q1).ln()),
                Some(_) => (median, q3 - q1),
            };
            points.push(Point {
                x: level,
                y: spread,
                z: None,
                label: Some(r.labels[1..].join(", ")),
            });
        }
        let k = points.len() as f64;
        let (lm, sm) = points
            .iter()
            .fold((0.0, 0.0), |(a, b), p| (a + p.x / k, b + p.y / k));
        let sxx: f64 = points.iter().map(|p| (p.x - lm).powi(2)).sum();
        let sxy: f64 = points.iter().map(|p| (p.x - lm) * (p.y - sm)).sum();
        let slope = (points.len() > 1 && sxx > 0.0).then(|| sxy / sxx);
        let what = match power {
            None => "natural log of ".to_string(),
            Some(0.0) => "data transformed by the natural log: ".into(),
            Some(a) => format!("data transformed by the power {}: ", short(a)),
        };
        let r = cells[0];
        let mut footnote = match slope {
            Some(b) => format!("Slope = {}", short_number(b)),
            None => "Slope: none, the levels do not vary".into(),
        };
        if let (None, Some(b)) = (power, slope) {
            footnote += &format!("; power for transformation = {}", short_number(1.0 - b));
        }
        let mut chart = Chart::plot(
            ChartKind::SpreadLevel,
            format!(
                "Spread-versus-level plot of {} by {}",
                r.labels[0],
                self.names.join(" by ")
            ),
            Axis::titled(format!("Level ({what}median)")),
            Axis::titled(format!("Spread ({what}interquartile range)")),
            Plot::Points {
                groups: vec![PointGroup {
                    name: r.labels[0].clone(),
                    points,
                }],
                lines: slope
                    .map(|b| ReferenceLine {
                        intercept: sm - b * lm,
                        slope: b,
                    })
                    .into_iter()
                    .collect(),
                z_axis: None,
            },
        );
        chart.footnote = Some(footnote);
        let deviations = |centre: &dyn Fn(&Distribution, &[(f64, f64)]) -> Option<f64>| {
            groups
                .iter()
                .map(|g| {
                    let c = centre(&Distribution::new(g), g).unwrap_or(f64::NAN);
                    g.iter().map(|(y, w)| ((y - c).abs(), *w)).collect()
                })
                .collect::<Vec<Vec<(f64, f64)>>>()
        };
        let rows = [
            deviations(&|_, g| summarize(g).mean),
            deviations(&|d, _| d.percentile(50.0, method)),
            deviations(&|d, _| d.trimmed_mean()),
        ]
        .concat();
        Ok((chart, rows))
    }
}

/// A slope or power as the plot shows it: 3 decimals.
fn short_number(x: f64) -> String {
    format!("{x:.3}")
}

/// Test of Homogeneity of Variance of one dependent: the deviations from the mean, the median
/// and the trimmed mean of every cell, one after another.
fn levene_table(rows: Vec<Vec<(f64, f64)>>, r: &CellResult, power: Option<f64>) -> Table {
    let k = rows.len() / 3;
    let (mean, rest) = rows.split_at(k);
    let (median, trimmed) = rest.split_at(k);
    let mut out = Vec::new();
    let mut line = |name: &str, (stat, df1, df2, v): (f64, f64, f64, f64), adjusted: bool| {
        let df2 = if adjusted { v } else { df2 };
        out.push(vec![
            Cell::Text(r.labels[0].clone()),
            Cell::Text(name.into()),
            Cell::Number(stat),
            Cell::Number(df1),
            Cell::Number(df2),
            Cell::Number(f_p(stat, df1, df2)),
        ]);
    };
    line("Based on Mean", levene_of(mean), false);
    let m = levene_of(median);
    line("Based on Median", m, false);
    line("Based on Median and with adjusted df", m, true);
    line("Based on trimmed mean", levene_of(trimmed), false);
    let formats = out
        .iter()
        .map(|_| {
            ["", "", "F8.3", "F8.0", "F8.3", "F8.3"]
                .map(String::from)
                .to_vec()
        })
        .collect();
    Table {
        title: "Test of Homogeneity of Variance".into(),
        row_headers: 2,
        columns: ["", "", "Levene Statistic", "df1", "df2", "Sig."]
            .map(String::from)
            .to_vec(),
        column_groups: vec![Vec::new(); 6],
        rows: out,
        formats,
        caption: power.map(|a| match a {
            0.0 => "On the natural logs of the values.".to_string(),
            a => format!("On the values raised to the power {}.", short(a)),
        }),
    }
}

/// The lines of a stem-and-leaf display of (value, weight) pairs.
pub(crate) fn stem_leaf(pairs: &[(f64, f64)]) -> StemLeaf {
    let mut display = StemLeaf {
        stem_width: 1.0,
        each_leaf: 1.0,
        low_extremes: None,
        lines: Vec::new(),
        high_extremes: None,
    };
    if pairs.is_empty() {
        return display;
    }
    let n: f64 = pairs.iter().map(|p| p.1).sum();
    let lo = pairs.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let hi = pairs.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    let wanted = (10.0 * n.max(1.0).log10()).floor().max(1.0);
    let raw = match (hi > lo, lo != 0.0) {
        (true, _) => (hi - lo) / wanted,
        (false, true) => lo.abs() / 10.0,
        (false, false) => 1.0,
    };
    let power = 10f64.powf(raw.log10().floor());
    let mantissa = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .find(|m| m * power >= raw * (1.0 - 1e-9))
        .unwrap_or(10.0);
    // a line of width 1 is a stem of ten leaf units; 2 and 5 split a stem in five and two
    let unit = tidy(if mantissa == 1.0 { power / 10.0 } else { power });
    let per_stem = match mantissa {
        2.0 => 5,
        5.0 => 2,
        _ => 1,
    };
    display.stem_width = tidy(unit * 10.0);
    // the line of a value, below 0 for negative values, and its leaf digit
    let place = |y: f64| {
        let t = (y.abs() / unit + 1e-9).floor() as i64;
        let (stem, leaf) = (t / 10, t % 10);
        let m = stem * per_stem + leaf * per_stem / 10;
        (if y < 0.0 { -m - 1 } else { m }, leaf as u8)
    };
    let first = place(lo).0;
    let last = place(hi).0;
    let mut digits = vec![[0.0f64; 10]; (last - first + 1) as usize];
    for &(y, w) in pairs {
        let (line, leaf) = place(y);
        digits[(line - first) as usize][leaf as usize] += w;
    }
    let most = digits
        .iter()
        .map(|d| d.iter().sum::<f64>())
        .fold(0.0, f64::max);
    let mut each = 1.0;
    for step in [2.0, 2.5, 2.0].iter().cycle() {
        if most / each <= 50.0 {
            break;
        }
        each *= step;
    }
    display.each_leaf = each;
    for (i, line) in digits.iter().enumerate() {
        let position = first + i as i64;
        let magnitude = if position < 0 {
            -position - 1
        } else {
            position
        };
        let stem = magnitude / per_stem;
        let leaves: String = (0..10)
            .flat_map(|d| {
                let count = (line[d] / each).round() as usize;
                std::iter::repeat_n(char::from(b'0' + d as u8), count)
            })
            .collect();
        display.lines.push(StemLine {
            frequency: line.iter().sum(),
            stem: match position < 0 {
                true => format!("-{stem}"),
                false => stem.to_string(),
            },
            leaves,
        });
    }
    display
}

fn tidy(x: f64) -> f64 {
    format!("{x:.12}").parse().unwrap_or(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(values: &[f64]) -> Vec<(f64, f64)> {
        values.iter().map(|v| (*v, 1.0)).collect()
    }

    #[test]
    fn stems_of_whole_numbers_split_in_halves() {
        // 20 cases from 3 to 38: 13 lines wanted, width 35/13 = 2.69 goes to 5, two per stem
        let values = [
            3.0, 5.0, 7.0, 8.0, 11.0, 12.0, 12.0, 14.0, 16.0, 19.0, 21.0, 22.0, 24.0, 25.0, 27.0,
            29.0, 31.0, 33.0, 36.0, 38.0,
        ];
        let s = stem_leaf(&unit(&values));
        assert_eq!((s.stem_width, s.each_leaf), (10.0, 1.0));
        let lines: Vec<(f64, &str, &str)> = s
            .lines
            .iter()
            .map(|l| (l.frequency, l.stem.as_str(), l.leaves.as_str()))
            .collect();
        assert_eq!(
            lines,
            [
                (1.0, "0", "3"),
                (3.0, "0", "578"),
                (4.0, "1", "1224"),
                (2.0, "1", "69"),
                (3.0, "2", "124"),
                (3.0, "2", "579"),
                (2.0, "3", "13"),
                (2.0, "3", "68"),
            ]
        );
    }

    #[test]
    fn stems_of_negative_values_and_decimals() {
        // 5 cases: 6 lines wanted, width 2.3/6 = 0.383 goes to 0.5; leaf unit 0.1
        let s = stem_leaf(&unit(&[-1.2, -0.4, 0.3, 0.8, 1.1]));
        assert_eq!(s.stem_width, 1.0);
        let lines: Vec<(&str, &str)> = s
            .lines
            .iter()
            .map(|l| (l.stem.as_str(), l.leaves.as_str()))
            .collect();
        // lines from -1 (leaves 0 to 4) up to 1 (leaves 0 to 4)
        assert_eq!(
            lines,
            [
                ("-1", "2"),
                ("-0", ""),
                ("-0", "4"),
                ("0", "3"),
                ("0", "8"),
                ("1", "1"),
            ]
        );
    }

    #[test]
    fn many_cases_share_a_leaf() {
        let pairs = [(4.0, 120.0), (5.0, 30.0)];
        let s = stem_leaf(&pairs);
        // 21 lines wanted, width 1/21 goes to 0.05: leaf unit 0.01, two lines per stem; 120
        // cases on one line are over 50 leaves at 1 and at 2 cases a leaf, so 5
        assert_eq!((s.stem_width, s.each_leaf), (0.1, 5.0));
        assert_eq!(s.lines.len(), 21);
        assert_eq!(s.lines[0].stem, "40");
        assert_eq!(s.lines[0].leaves, "0".repeat(24));
        assert_eq!(s.lines[20].leaves, "000000");
    }
}
