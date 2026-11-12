//! From a chart to its shapes. Each kind has one layout function; `scene` picks it, and a kind
//! without one returns None, so it shows as its data table. The statistical plots that carry
//! their data in `Chart::plot` are laid out in `plots`.

mod plots;

use super::{Anchor, Color, Palette, Paper, Scene, Shape, text_width};
use crate::Cell;
use crate::chart::{Chart, ChartKind, Grouping, Orientation, Statistic};

const PAD: f32 = 12.0;
const TITLE: f32 = 15.0;
const SMALL: f32 = 11.0;
/// The height of a line of labels.
const LINE: f32 = 18.0;
const MIN_WIDTH: f32 = 280.0;

/// The chart laid out `width` pixels wide, as tall as its kind needs; None for a kind that has
/// no layout yet or numbers that do not fit its kind.
pub fn scene(chart: &Chart, width: f32, paper: Paper) -> Option<Scene> {
    let n = chart.categories.len();
    let fits =
        n > 0 && !chart.series.is_empty() && chart.series.iter().all(|s| s.values.len() == n);
    if chart.plot.is_none() && !fits {
        return None;
    }
    let width = width.max(MIN_WIDTH).round();
    let mut d = Draw {
        width,
        pal: Palette::of(paper),
        shapes: Vec::new(),
    };
    let mut top = d.title(&chart.title);
    if let Some(subtitle) = &chart.subtitle {
        top = d.footnote(Some(subtitle), top - 4.0) + 4.0;
    }
    let bottom = match (&chart.plot, chart.kind) {
        (Some(plot), _) => plots::plot(&mut d, chart, plot, top)?,
        (None, ChartKind::Bar) => bars(&mut d, chart, top),
        (None, ChartKind::Line | ChartKind::Area) => lines(&mut d, chart, top),
        (None, ChartKind::Scatter) => scatter(&mut d, chart, top)?,
        (None, ChartKind::Histogram) => histogram(&mut d, chart, top)?,
        (None, ChartKind::Pie) => pie(&mut d, chart, top)?,
        // a kind without a layout here shows as its data table
        (None, _) => return None,
    };
    let bottom = d.footnote(chart.footnote.as_deref(), bottom);
    Some(Scene {
        width,
        height: (bottom + PAD).ceil(),
        background: d.pal.paper,
        shapes: d.shapes,
    })
}

struct Draw {
    width: f32,
    pal: Palette,
    shapes: Vec<Shape>,
}

impl Draw {
    fn text(&mut self, x: f32, y: f32, text: String, size: f32, color: Color, anchor: Anchor) {
        if !text.is_empty() {
            self.shapes.push(Shape::Text {
                x,
                y,
                text,
                size,
                color,
                anchor,
            });
        }
    }

    /// A rectangle between two corners in any order.
    fn rect(&mut self, [x0, y0]: [f32; 2], [x1, y1]: [f32; 2], fill: Color) {
        let (w, h) = ((x1 - x0).abs(), (y1 - y0).abs());
        if w > 0.0 && h > 0.0 {
            self.shapes.push(Shape::Rect {
                x: x0.min(x1),
                y: y0.min(y1),
                w,
                h,
                fill,
            });
        }
    }

    fn line(&mut self, points: Vec<[f32; 2]>, width: f32, color: Color) {
        if points.len() > 1 {
            self.shapes.push(Shape::Line {
                points,
                width,
                color,
            });
        }
    }

    /// The title, wrapped; returns the y below it.
    fn title(&mut self, title: &str) -> f32 {
        let mut y = PAD;
        for line in wrap(title, TITLE, self.width - 2.0 * PAD) {
            self.text(PAD, y + 10.0, line, TITLE, self.pal.ink, Anchor::Start);
            y += 20.0;
        }
        y + 4.0
    }

    /// The legend of several series: its title and a swatch with each name, in rows.
    fn legend(&mut self, chart: &Chart, colors: &[Color], top: f32) -> f32 {
        let names: Vec<&str> = chart.series.iter().map(|s| s.name.as_str()).collect();
        self.swatches(chart.legend.as_deref(), &names, colors, top)
    }

    /// A title and a swatch with each name, in rows; nothing for fewer than two names.
    fn swatches(&mut self, title: Option<&str>, names: &[&str], colors: &[Color], top: f32) -> f32 {
        if names.len() < 2 {
            return top;
        }
        let right = self.width - PAD;
        let (mut x, mut y) = (PAD, top);
        if let Some(title) = title {
            let title = fit(title, SMALL, right - PAD);
            let w = text_width(&title, SMALL);
            self.text(
                x,
                y + LINE / 2.0,
                title,
                SMALL,
                self.pal.muted,
                Anchor::Start,
            );
            x += w + 12.0;
        }
        for (name, &color) in names.iter().zip(colors) {
            let name = fit(name, SMALL, right - PAD - 15.0);
            let w = 15.0 + text_width(&name, SMALL);
            if x + w > right && x > PAD {
                (x, y) = (PAD, y + LINE);
            }
            let mid = y + LINE / 2.0;
            self.rect([x, mid - 5.0], [x + 10.0, mid + 5.0], color);
            self.text(x + 15.0, mid, name, SMALL, self.pal.muted, Anchor::Start);
            x += w + 14.0;
        }
        y + LINE + 4.0
    }

    fn footnote(&mut self, footnote: Option<&str>, top: f32) -> f32 {
        let mut y = top;
        if let Some(footnote) = footnote {
            y += 4.0;
            for line in wrap(footnote, SMALL, self.width - 2.0 * PAD) {
                self.text(PAD, y + 7.5, line, SMALL, self.pal.muted, Anchor::Start);
                y += 15.0;
            }
        }
        y
    }
}

/// The text cut short with an ellipsis to fit `max` pixels.
fn fit(text: &str, size: f32, max: f32) -> String {
    if text_width(text, size) <= max {
        return text.to_string();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let cut: String = chars.iter().collect::<String>().trim_end().to_string() + "\u{2026}";
        if text_width(&cut, size) <= max {
            return cut;
        }
    }
    String::new()
}

/// The text in lines no wider than `max`, broken between words.
fn wrap(text: &str, size: f32, max: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let longer = match line.is_empty() {
            true => word.to_string(),
            false => format!("{line} {word}"),
        };
        if line.is_empty() || text_width(&longer, size) <= max {
            line = longer;
        } else {
            lines.push(fit(&line, size, max));
            line = word.to_string();
        }
    }
    if !line.is_empty() {
        lines.push(fit(&line, size, max));
    }
    lines
}

/// Tick values in steps of 1, 2 or 5 times a power of ten, about `count` of them, the first at
/// or below `lo` and the last at or above `hi`; steps of at least 1 when `whole`.
pub(crate) fn ticks(lo: f64, hi: f64, count: usize, whole: bool) -> Vec<f64> {
    let (lo, hi) = match hi > lo {
        true => (lo, hi),
        false if lo == 0.0 => (0.0, 1.0),
        false => (lo - lo.abs() / 2.0, hi + hi.abs() / 2.0),
    };
    let raw = (hi - lo) / count as f64;
    let power = 10f64.powf(raw.log10().floor());
    let step = [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * power)
        .find(|s| *s >= raw * (1.0 - 1e-9))
        .unwrap_or(10.0 * power);
    let step = if whole { step.max(1.0) } else { step };
    let first = (lo / step + 1e-9).floor() as i64;
    let last = (hi / step - 1e-9).ceil() as i64;
    // a decimal more than the step has, so 0.6 is not 0.6000000000000001
    let decimals = (-step.log10().floor()).max(0.0) as usize + 1;
    (first..=last)
        .map(|k| {
            format!("{:.decimals$}", k as f64 * step)
                .parse()
                .unwrap_or(0.0)
        })
        .collect()
}

/// What the numbers on an axis are.
#[derive(Clone, Copy, PartialEq)]
enum Unit {
    Plain,
    /// Counts without fractions: no tick between two whole numbers.
    Whole,
    Percent,
}

/// A numeric axis: the range it shows and its ticks.
struct Axis {
    lo: f64,
    hi: f64,
    ticks: Vec<f64>,
    percent: bool,
}

impl Axis {
    /// Ticks around `lo` to `hi`, the range widened to the outer ticks; a `maximum` at or above
    /// `hi` is the top instead.
    fn new(lo: f64, hi: f64, maximum: Option<f64>, unit: Unit) -> Axis {
        let top = maximum.filter(|m| *m >= hi);
        let mut ticks = ticks(lo, top.unwrap_or(hi), 5, unit == Unit::Whole);
        let (lo, hi) = match top {
            Some(m) => {
                ticks.retain(|t| *t <= m * (1.0 + 1e-9));
                (ticks[0].min(lo), m)
            }
            None => (ticks[0], ticks[ticks.len() - 1]),
        };
        Axis {
            lo,
            hi,
            ticks,
            percent: unit == Unit::Percent,
        }
    }

    /// Exactly `lo` to `hi`, with the ticks that fall inside.
    fn exact(lo: f64, hi: f64) -> Axis {
        let mut ticks = ticks(lo, hi, 6, false);
        ticks.retain(|t| *t >= lo - 1e-9 * (hi - lo) && *t <= hi + 1e-9 * (hi - lo));
        Axis {
            lo,
            hi: if hi > lo { hi } else { lo + 1.0 },
            ticks,
            percent: false,
        }
    }

    /// Where `v` falls between `a` (at the bottom of the range) and `b` (at its top).
    fn at(&self, v: f64, a: f32, b: f32) -> f32 {
        a + (b - a) * ((v - self.lo) / (self.hi - self.lo)) as f32
    }

    fn labels(&self) -> Vec<String> {
        let step = match self.ticks.len() {
            0 | 1 => 1.0,
            _ => self.ticks[1] - self.ticks[0],
        };
        let decimals = (0..6)
            .find(|d| {
                let scaled = step * 10f64.powi(*d);
                (scaled - scaled.round()).abs() < 1e-6 * scaled.abs().max(1.0)
            })
            .unwrap_or(6) as usize;
        let suffix = if self.percent { "%" } else { "" };
        self.ticks
            .iter()
            // no "-0"
            .map(|t| if t.abs() < step * 1e-6 { 0.0 } else { *t })
            .map(|t| format!("{t:.decimals$}{suffix}"))
            .collect()
    }
}

/// What runs along the category axis: one band per category, or numbers.
enum Along {
    Bands(Vec<String>),
    Numbers(Axis),
}

/// The plot area of a chart with categories or numbers along one axis and values along the
/// other, with its gridlines, tick labels and axis titles drawn.
struct Frame {
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    horizontal: bool,
    bands: usize,
    along: Option<Axis>,
    value: Axis,
}

impl Frame {
    /// Draws the frame below `top`; returns it and the y below its labels.
    fn new(
        d: &mut Draw,
        chart: &Chart,
        top: f32,
        along: Along,
        value: Axis,
        horizontal: bool,
    ) -> (Frame, f32) {
        let (muted, rule) = (d.pal.muted, d.pal.rule);
        let value_labels = value.labels();
        let widest = |labels: &[String]| {
            labels
                .iter()
                .map(|l| text_width(l, SMALL))
                .fold(0.0f32, f32::max)
        };
        let right = d.width - PAD;
        let (bands, labels, along) = match along {
            Along::Bands(labels) => (labels.len(), labels, None),
            Along::Numbers(axis) => (0, axis.labels(), Some(axis)),
        };
        let horizontal = horizontal && along.is_none();
        if horizontal {
            let category_width = widest(&labels).min(d.width * 0.35);
            let x0 = PAD + category_width + 8.0;
            let x1 = right - widest(&value_labels[value_labels.len() - 1..]) / 2.0;
            let title = fit(&chart.category_axis.title, SMALL, right - PAD);
            d.text(PAD, top + LINE / 2.0, title, SMALL, muted, Anchor::Start);
            let y0 = top + LINE + 4.0;
            let band = match chart.grouping {
                Some(Grouping::Stacked | Grouping::Percent) => 24.0,
                _ => (chart.series.len() as f32 * 12.0 + 10.0).clamp(20.0, 56.0),
            };
            let y1 = y0 + band * bands as f32;
            let frame = Frame {
                x0,
                y0,
                x1,
                y1,
                horizontal,
                bands,
                along: None,
                value,
            };
            for (t, label) in frame.value.ticks.iter().zip(value_labels) {
                let x = frame.value.at(*t, x0, x1);
                d.line(vec![[x, y0], [x, y1]], 1.0, rule);
                d.text(x, y1 + 10.0, label, SMALL, muted, Anchor::Middle);
            }
            for (i, label) in labels.iter().enumerate() {
                let (b0, b1) = frame.band(i);
                let label = fit(label, SMALL, category_width);
                d.text(x0 - 8.0, (b0 + b1) / 2.0, label, SMALL, muted, Anchor::End);
            }
            let title = fit(&chart.value_axis.title, SMALL, x1 - x0);
            let below = y1 + 10.0 + LINE;
            d.text((x0 + x1) / 2.0, below, title, SMALL, muted, Anchor::Middle);
            return (frame, below + LINE / 2.0);
        }
        let x0 = PAD + widest(&value_labels) + 8.0;
        let x1 = match &along {
            Some(_) => right - widest(&labels[labels.len().saturating_sub(1)..]) / 2.0,
            None => right,
        };
        let title = fit(&chart.value_axis.title, SMALL, right - PAD);
        d.text(PAD, top + LINE / 2.0, title, SMALL, muted, Anchor::Start);
        let y0 = top + LINE + 10.0;
        let y1 = y0 + (d.width * 0.42).clamp(140.0, 340.0);
        let frame = Frame {
            x0,
            y0,
            x1,
            y1,
            horizontal,
            bands,
            along,
            value,
        };
        for (t, label) in frame.value.ticks.iter().zip(value_labels) {
            let y = frame.value.at(*t, y1, y0);
            d.line(vec![[x0, y], [x1, y]], 1.0, rule);
            d.text(x0 - 6.0, y, label, SMALL, muted, Anchor::End);
        }
        match &frame.along {
            Some(axis) => {
                for (t, label) in axis.ticks.iter().zip(labels) {
                    let x = axis.at(*t, x0, x1);
                    d.line(vec![[x, y1], [x, y1 + 4.0]], 1.0, muted);
                    d.text(x, y1 + 12.0, label, SMALL, muted, Anchor::Middle);
                }
            }
            None => {
                let slot = (x1 - x0) / bands as f32;
                // every k-th label when the bands are too narrow for a few characters each
                let k = (28.0 / slot).ceil().max(1.0) as usize;
                for (i, label) in labels.iter().enumerate().step_by(k) {
                    let (b0, b1) = frame.band(i);
                    let label = fit(label, SMALL, slot * k as f32 - 4.0);
                    d.text(
                        (b0 + b1) / 2.0,
                        y1 + 10.0,
                        label,
                        SMALL,
                        muted,
                        Anchor::Middle,
                    );
                }
            }
        }
        let title = fit(&chart.category_axis.title, SMALL, x1 - x0);
        let below = y1 + 12.0 + LINE;
        d.text((x0 + x1) / 2.0, below, title, SMALL, muted, Anchor::Middle);
        (frame, below + LINE / 2.0)
    }

    /// Band `i` along the category axis, from its start to its end in pixels.
    fn band(&self, i: usize) -> (f32, f32) {
        let (a, b) = match self.horizontal {
            true => (self.y0, self.y1),
            false => (self.x0, self.x1),
        };
        let slot = (b - a) / self.bands as f32;
        (a + slot * i as f32, a + slot * (i + 1) as f32)
    }

    /// Where a value falls along the value axis, in pixels.
    fn value(&self, v: f64) -> f32 {
        match self.horizontal {
            true => self.value.at(v, self.x0, self.x1),
            false => self.value.at(v, self.y1, self.y0),
        }
    }

    /// A point from its place along the category axis and its value in pixels.
    fn point(&self, along: f32, value: f32) -> [f32; 2] {
        match self.horizontal {
            true => [value, along],
            false => [along, value],
        }
    }

    /// The zero line over the marks, or the bottom of the plot when zero is not in it.
    fn baseline(&self, d: &mut Draw) {
        let zero = match self.value.lo <= 0.0 && self.value.hi >= 0.0 {
            true => self.value(0.0),
            false => self.value(self.value.lo),
        };
        let line = match self.horizontal {
            true => vec![[zero, self.y0], [zero, self.y1]],
            false => vec![[self.x0, zero], [self.x1, zero]],
        };
        d.line(line, 1.0, d.pal.muted);
    }
}

fn finite(v: f64) -> f64 {
    if v.is_finite() { v } else { 0.0 }
}

/// Each series' values, as shares of their category's total in percent for a 100% chart.
fn shown_values(chart: &Chart) -> Vec<Vec<f64>> {
    let values: Vec<Vec<f64>> = chart
        .series
        .iter()
        .map(|s| s.values.iter().copied().map(finite).collect())
        .collect();
    if chart.grouping != Some(Grouping::Percent) {
        return values;
    }
    let totals: Vec<f64> = (0..chart.categories.len())
        .map(|i| values.iter().map(|s| s[i].max(0.0)).sum())
        .collect();
    values
        .iter()
        .map(|s| {
            s.iter()
                .zip(&totals)
                .map(|(v, t)| {
                    if *t > 0.0 {
                        100.0 * v.max(0.0) / t
                    } else {
                        0.0
                    }
                })
                .collect()
        })
        .collect()
}

/// Percentages, whole counts or other numbers.
fn unit(chart: &Chart) -> Unit {
    let percent = chart.grouping == Some(Grouping::Percent)
        || chart.value_axis.format.starts_with("PCT")
        || chart
            .series
            .iter()
            .all(|s| s.statistic == Statistic::Percent);
    let whole = chart
        .series
        .iter()
        .all(|s| s.statistic == Statistic::Count && s.values.iter().all(|v| v.fract() == 0.0));
    match (percent, whole) {
        (true, _) => Unit::Percent,
        (false, true) => Unit::Whole,
        (false, false) => Unit::Plain,
    }
}

fn stacked(chart: &Chart) -> bool {
    matches!(chart.grouping, Some(Grouping::Stacked | Grouping::Percent))
}

/// The lowest and highest value the marks reach, with zero: stacked marks reach the sums of
/// their positive and of their negative values.
fn value_range(chart: &Chart, values: &[Vec<f64>]) -> (f64, f64) {
    let (mut lo, mut hi) = (0.0f64, 0.0f64);
    for i in 0..chart.categories.len() {
        let column = values.iter().map(|s| s[i]);
        match stacked(chart) {
            true => {
                hi = hi.max(column.clone().filter(|v| *v > 0.0).sum());
                lo = lo.min(column.filter(|v| *v < 0.0).sum());
            }
            false => column.for_each(|v| (lo, hi) = (lo.min(v), hi.max(v))),
        }
        for [a, b] in error_bars(chart, i).map(|(_, bar)| bar) {
            (lo, hi) = (lo.min(a), hi.max(b));
        }
    }
    (lo, hi)
}

/// The error bars at category `i` with their series; none on stacked marks, where a bar
/// around a share of the stack would mislead.
fn error_bars(chart: &Chart, i: usize) -> impl Iterator<Item = (usize, [f64; 2])> + '_ {
    let n = if stacked(chart) {
        0
    } else {
        chart.series.len()
    };
    (0..n).filter_map(move |j| chart.error_bar(j, i).map(|bar| (j, bar)))
}

/// An error bar along the value axis at `along`, with a cap `cap` pixels wide at each end.
fn whisker(d: &mut Draw, frame: &Frame, along: f32, [lo, hi]: [f64; 2], cap: f32, color: Color) {
    let (a, b) = (frame.value(lo), frame.value(hi));
    d.line(
        vec![frame.point(along, a), frame.point(along, b)],
        1.0,
        color,
    );
    for end in [a, b] {
        let half = cap / 2.0;
        d.line(
            vec![
                frame.point(along - half, end),
                frame.point(along + half, end),
            ],
            1.0,
            color,
        );
    }
}

fn labels(chart: &Chart) -> Vec<String> {
    chart.categories.iter().map(|c| c.label.clone()).collect()
}

fn value_axis(chart: &Chart, values: &[Vec<f64>]) -> Axis {
    let (lo, hi) = value_range(chart, values);
    let maximum = match chart.grouping {
        Some(Grouping::Percent) => None,
        _ => chart.value_axis.maximum,
    };
    let mut axis = Axis::new(lo, hi, maximum, unit(chart));
    if chart.grouping == Some(Grouping::Percent) {
        (axis.lo, axis.hi) = (0.0, 100.0);
        axis.ticks.retain(|t| (0.0..=100.0).contains(t));
    }
    axis
}

/// Bars, vertical or horizontal, side by side or stacked.
fn bars(d: &mut Draw, chart: &Chart, top: f32) -> f32 {
    let colors = d.pal.series(chart.series.len());
    let top = d.legend(chart, &colors, top);
    let values = shown_values(chart);
    let axis = value_axis(chart, &values);
    let horizontal = chart.orientation == Some(Orientation::Horizontal);
    let (frame, bottom) = Frame::new(d, chart, top, Along::Bands(labels(chart)), axis, horizontal);
    let n = chart.series.len();
    let zero = frame.value(0.0);
    for i in 0..chart.categories.len() {
        let (b0, b1) = frame.band(i);
        let group = (b1 - b0) * if n > 1 && !stacked(chart) { 0.8 } else { 0.7 };
        let g0 = b0 + (b1 - b0 - group) / 2.0;
        let (mut positive, mut negative) = (0.0, 0.0);
        for (j, series) in values.iter().enumerate() {
            let v = series[i];
            if stacked(chart) {
                let from = if v >= 0.0 { positive } else { negative };
                if v >= 0.0 {
                    positive += v
                } else {
                    negative += v
                }
                let (v0, v1) = (frame.value(from), frame.value(from + v));
                d.rect(frame.point(g0, v0), frame.point(g0 + group, v1), colors[j]);
            } else {
                let width = group / n as f32;
                // a hairline between bars side by side
                let gap = if n > 1 && width > 4.0 { 0.5 } else { 0.0 };
                let a = g0 + width * j as f32 + gap;
                let b = a + width - 2.0 * gap;
                d.rect(
                    frame.point(a, zero),
                    frame.point(b, frame.value(v)),
                    colors[j],
                );
                if let Some(bar) = chart.error_bar(j, i) {
                    let ink = d.pal.ink;
                    whisker(
                        d,
                        &frame,
                        (a + b) / 2.0,
                        bar,
                        ((b - a) * 0.4).min(12.0),
                        ink,
                    );
                }
            }
        }
    }
    frame.baseline(d);
    bottom
}

/// Lines with a dot at each value, or areas, stacked when the chart says so.
fn lines(d: &mut Draw, chart: &Chart, top: f32) -> f32 {
    let colors = d.pal.series(chart.series.len());
    let top = d.legend(chart, &colors, top);
    let values = shown_values(chart);
    let axis = value_axis(chart, &values);
    let horizontal = chart.orientation == Some(Orientation::Horizontal);
    let (frame, bottom) = Frame::new(d, chart, top, Along::Bands(labels(chart)), axis, horizontal);
    let n = chart.categories.len();
    let centers: Vec<f32> = (0..n)
        .map(|i| {
            let (b0, b1) = frame.band(i);
            (b0 + b1) / 2.0
        })
        .collect();
    let area = chart.kind == ChartKind::Area;
    let mut below = vec![0.0; n];
    for (j, series) in values.iter().enumerate() {
        let base: Vec<f64> = match area && stacked(chart) {
            true => below.clone(),
            false => vec![0.0; n],
        };
        let upper: Vec<[f32; 2]> = (0..n)
            .map(|i| frame.point(centers[i], frame.value(base[i] + series[i])))
            .collect();
        if area {
            let lower = (0..n)
                .map(|i| frame.point(centers[i], frame.value(base[i])))
                .collect();
            let fill = match stacked(chart) {
                true => colors[j],
                // areas over each other let the ones behind show through
                false => colors[j].alpha(90),
            };
            d.shapes.push(Shape::Band {
                upper: upper.clone(),
                lower,
                fill,
            });
            d.line(upper, 1.5, colors[j]);
        } else {
            d.line(upper.clone(), 2.0, colors[j]);
            if n <= 40 {
                for point in upper {
                    d.shapes.push(Shape::Circle {
                        center: point,
                        radius: 3.0,
                        fill: colors[j],
                    });
                }
            }
        }
        below.iter_mut().zip(series).for_each(|(b, v)| *b += v);
    }
    for (i, &center) in centers.iter().enumerate() {
        for (j, bar) in error_bars(chart, i) {
            whisker(d, &frame, center, bar, 8.0, colors[j]);
        }
    }
    frame.baseline(d);
    bottom
}

/// The x value of each category: its number, or its label read as one.
fn x_values(chart: &Chart) -> Option<Vec<f64>> {
    chart
        .categories
        .iter()
        .map(|c| match &c.value {
            Cell::Number(x) if x.is_finite() => Some(*x),
            _ => c.label.trim().parse().ok(),
        })
        .collect()
}

/// A dot per category and series, at the category's x value.
fn scatter(d: &mut Draw, chart: &Chart, top: f32) -> Option<f32> {
    let xs = x_values(chart)?;
    let colors = d.pal.series(chart.series.len());
    let top = d.legend(chart, &colors, top);
    let ys: Vec<f64> = chart
        .series
        .iter()
        .flat_map(|s| s.values.iter().copied().filter(|v| v.is_finite()))
        .collect();
    let range = |v: &[f64]| {
        let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        (lo, hi)
    };
    let (x_lo, x_hi) = range(&xs);
    let (y_lo, y_hi) = range(&ys);
    if !y_lo.is_finite() {
        return None;
    }
    let x_axis = Axis::new(x_lo, x_hi, None, Unit::Plain);
    let y_axis = Axis::new(y_lo, y_hi, chart.value_axis.maximum, unit(chart));
    let (frame, bottom) = Frame::new(d, chart, top, Along::Numbers(x_axis), y_axis, false);
    let along = frame.along.as_ref()?;
    for t in &along.ticks {
        let x = along.at(*t, frame.x0, frame.x1);
        d.line(vec![[x, frame.y0], [x, frame.y1]], 1.0, d.pal.rule);
    }
    for (series, &color) in chart.series.iter().zip(&colors) {
        for (x, y) in xs.iter().zip(&series.values) {
            if y.is_finite() {
                d.shapes.push(Shape::Circle {
                    center: [
                        along.at(*x, frame.x0, frame.x1),
                        frame.value.at(*y, frame.y1, frame.y0),
                    ],
                    radius: 3.5,
                    fill: color.alpha(220),
                });
            }
        }
    }
    Some(bottom)
}

/// Bars over the bins, touching, with the normal curve over them when the chart has one.
fn histogram(d: &mut Draw, chart: &Chart, top: f32) -> Option<f32> {
    let bins: Vec<[f64; 2]> = chart
        .categories
        .iter()
        .map(|c| c.bin)
        .collect::<Option<_>>()?;
    let counts: Vec<f64> = chart.series[0].values.iter().copied().map(finite).collect();
    let (lo, hi) = (bins[0][0], bins[bins.len() - 1][1]);
    let width = bins[0][1] - bins[0][0];
    let density = |x: f64, n: &crate::chart::NormalCurve| {
        let z = (x - n.mean) / n.std_dev;
        n.total * width * (-z * z / 2.0).exp() / (n.std_dev * (2.0 * std::f64::consts::PI).sqrt())
    };
    let peak = chart.normal.as_ref().map_or(0.0, |n| density(n.mean, n));
    let top_count = counts.iter().copied().fold(peak, f64::max);
    let value = Axis::new(0.0, top_count, chart.value_axis.maximum, unit(chart));
    let along = Along::Numbers(Axis::exact(lo, hi));
    let (frame, bottom) = Frame::new(d, chart, top, along, value, false);
    let x_axis = frame.along.as_ref()?;
    let x = |v: f64| x_axis.at(v, frame.x0, frame.x1);
    let fill = d.pal.ramp[0];
    for (bin, count) in bins.iter().zip(&counts) {
        // a hairline of paper between bars
        let (a, b) = (x(bin[0]) + 0.5, x(bin[1]) - 0.5);
        d.rect([a, frame.value(0.0)], [b, frame.value(*count)], fill);
    }
    frame.baseline(d);
    if let Some(normal) = &chart.normal {
        let steps = 120;
        let curve = (0..=steps)
            .map(|k| {
                let v = lo + (hi - lo) * k as f64 / steps as f64;
                [x(v), frame.value(density(v, normal).min(frame.value.hi))]
            })
            .collect();
        d.line(curve, 2.0, d.pal.amber);
    }
    Some(bottom)
}

/// The slices of the first series, clockwise from twelve o'clock, with a legend of labels and
/// shares beside the pie, or below it when the picture is narrow.
fn pie(d: &mut Draw, chart: &Chart, top: f32) -> Option<f32> {
    let values: Vec<f64> = chart.series[0]
        .values
        .iter()
        .map(|v| finite(*v).max(0.0))
        .collect();
    let total: f64 = values.iter().sum();
    if total <= 0.0 {
        return None;
    }
    let mut colors = d.pal.series(values.len());
    for (color, category) in colors.iter_mut().zip(&chart.categories) {
        if category.missing {
            *color = d.pal.muted;
        }
    }
    let room = d.width - 2.0 * PAD;
    let narrow = room < 460.0;
    let radius = match narrow {
        true => (room * 0.35).clamp(50.0, 120.0),
        false => (room * 0.22).clamp(60.0, 140.0),
    };
    let center = [PAD + radius, top + 8.0 + radius];
    let mut angle = 0.0f32;
    for (v, &fill) in values.iter().zip(&colors) {
        let sweep = (std::f64::consts::TAU * v / total) as f32;
        if sweep > 0.0 {
            d.shapes.push(Shape::Wedge {
                center,
                radius,
                start: angle,
                end: angle + sweep,
                fill,
                edge: d.pal.paper,
            });
        }
        angle += sweep;
    }
    let (x, mut y) = match narrow {
        true => (PAD, center[1] + radius + 12.0),
        false => (PAD + 2.0 * radius + 28.0, top + 8.0),
    };
    let right = d.width - PAD;
    let title = fit(&chart.category_axis.title, SMALL, right - x);
    if !title.is_empty() {
        d.text(x, y + LINE / 2.0, title, SMALL, d.pal.muted, Anchor::Start);
        y += LINE;
    }
    for ((category, v), &color) in chart.categories.iter().zip(&values).zip(&colors) {
        let mid = y + LINE / 2.0;
        let share = format!("{:.1}%", 100.0 * v / total);
        let room = right - x - 15.0 - text_width("100.0%", SMALL) - 10.0;
        d.rect([x, mid - 5.0], [x + 10.0, mid + 5.0], color);
        let label = fit(&category.label, SMALL, room);
        d.text(x + 15.0, mid, label, SMALL, d.pal.ink, Anchor::Start);
        d.text(right, mid, share, SMALL, d.pal.muted, Anchor::End);
        y += LINE;
    }
    Some(y.max(center[1] + radius) + 8.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::{Axis as ChartAxis, Category, ErrorBars, NormalCurve, Series, Tier};

    pub(crate) fn chart(kind: ChartKind, grouping: Option<Grouping>, series: &[&[f64]]) -> Chart {
        let n = series[0].len();
        Chart {
            kind,
            tier: Tier::Presentation,
            orientation: Some(Orientation::Vertical),
            grouping,
            title: "Fruit eaten per week".into(),
            subtitle: None,
            footnote: None,
            category_axis: ChartAxis {
                title: "Fruit".into(),
                format: String::new(),
                maximum: None,
                variable: None,
            },
            value_axis: ChartAxis {
                title: "Count".into(),
                format: "F8.0".into(),
                maximum: None,
                variable: None,
            },
            legend: (series.len() > 1).then(|| "Household".into()),
            categories: (0..n)
                .map(|i| Category {
                    label: format!("Kind {}", i + 1),
                    value: Cell::Number(i as f64 + 1.0),
                    missing: false,
                    bin: None,
                })
                .collect(),
            series: series
                .iter()
                .enumerate()
                .map(|(j, values)| Series {
                    name: format!("Group {}", j + 1),
                    statistic: Statistic::Count,
                    variable: None,
                    value: Cell::Empty,
                    values: values.to_vec(),
                })
                .collect(),
            normal: None,
            error_bars: None,
            plot: None,
        }
    }

    /// The bars: rectangles in a series color, without the legend's swatches.
    fn bars_of(scene: &Scene, colors: &[Color]) -> Vec<(f32, f32, f32, f32, Color)> {
        scene
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Rect { x, y, w, h, fill } if colors.contains(fill) && *w > 10.5 => {
                    Some((*x, *y, *w, *h, *fill))
                }
                _ => None,
            })
            .collect()
    }

    /// Upright lines of width 1 in `color`: the stems of error bars, top and bottom y.
    fn stems(scene: &Scene, color: Color) -> Vec<(f32, f32)> {
        scene
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Line {
                    points,
                    width,
                    color: c,
                } if *width == 1.0 && *c == color && points[0][0] == points[1][0] => Some((
                    points[0][1].min(points[1][1]),
                    points[0][1].max(points[1][1]),
                )),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn error_bars_reach_their_half_length_on_both_sides_of_the_value() {
        let mut c = chart(ChartKind::Line, None, &[&[10.0, 20.0]]);
        c.error_bars = Some(ErrorBars {
            label: "95% CI".into(),
            margins: vec![vec![2.0, 25.0]],
        });
        let s = scene(&c, 600.0, Paper::Light).unwrap();
        let petrol = Palette::of(Paper::Light).ramp[0];
        let bars = stems(&s, petrol);
        assert_eq!(bars.len(), 2);
        let length = |(top, bottom): (f32, f32)| bottom - top;
        assert!((length(bars[1]) / length(bars[0]) - 12.5).abs() < 1e-3);
        // each dot sits in the middle of its bar
        let dots: Vec<f32> = s
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Circle { center, .. } => Some(center[1]),
                _ => None,
            })
            .collect();
        for (dot, (top, bottom)) in dots.iter().zip(&bars) {
            assert!((dot - (top + bottom) / 2.0).abs() < 0.01);
        }
        // the axis reaches the top of the longer bar, 45, and the bottom of it, -5
        let labels: Vec<&str> = s
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(
            labels.contains(&"50") && labels.contains(&"-10"),
            "{labels:?}"
        );
        // bars carry theirs in ink; stacked marks none
        c.kind = ChartKind::Bar;
        let ink = Palette::of(Paper::Dark).ink;
        assert_eq!(stems(&scene(&c, 600.0, Paper::Dark).unwrap(), ink).len(), 2);
        c.grouping = Some(Grouping::Stacked);
        assert!(stems(&scene(&c, 600.0, Paper::Dark).unwrap(), ink).is_empty());
    }

    #[test]
    fn ticks_are_steps_of_one_two_or_five() {
        assert_eq!(
            ticks(0.0, 37.0, 5, false),
            vec![0.0, 10.0, 20.0, 30.0, 40.0]
        );
        assert_eq!(
            ticks(0.0, 100.0, 5, false),
            vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0]
        );
        assert_eq!(
            ticks(-3.0, 7.0, 5, false),
            vec![-4.0, -2.0, 0.0, 2.0, 4.0, 6.0, 8.0]
        );
        assert_eq!(
            ticks(0.0, 0.9, 5, false),
            vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]
        );
        assert_eq!(ticks(0.0, 0.3, 5, false), vec![0.0, 0.1, 0.2, 0.3]);
        assert_eq!(
            ticks(1200.0, 1350.0, 5, false),
            vec![1200.0, 1250.0, 1300.0, 1350.0]
        );
        // whole counts: no half steps
        assert_eq!(ticks(0.0, 2.0, 5, true), vec![0.0, 1.0, 2.0]);
        assert_eq!(ticks(0.0, 2.0, 5, false), vec![0.0, 0.5, 1.0, 1.5, 2.0]);
        // nothing to span: zero to one, or half the value around it
        assert_eq!(
            ticks(0.0, 0.0, 5, false),
            vec![0.0, 0.2, 0.4, 0.6, 0.8, 1.0]
        );
        assert_eq!(ticks(4.0, 4.0, 5, false), vec![2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    #[test]
    fn tick_labels_carry_the_step_s_decimals_and_a_percent_sign() {
        let axis = Axis::new(0.0, 0.9, None, Unit::Plain);
        assert_eq!(axis.labels(), ["0.0", "0.2", "0.4", "0.6", "0.8", "1.0"]);
        let axis = Axis::new(0.0, 47.0, None, Unit::Percent);
        assert_eq!(axis.labels(), ["0%", "10%", "20%", "30%", "40%", "50%"]);
        // a maximum from the syntax is the top; the ticks stop at it
        let axis = Axis::new(0.0, 37.0, Some(45.0), Unit::Whole);
        assert_eq!((axis.lo, axis.hi), (0.0, 45.0));
        assert_eq!(axis.ticks, vec![0.0, 10.0, 20.0, 30.0, 40.0]);
    }

    #[test]
    fn bars_stand_on_zero_in_proportion_to_their_values() {
        let c = chart(
            ChartKind::Bar,
            Some(Grouping::Clustered),
            &[&[10.0, 20.0, 35.0]],
        );
        let scene = scene(&c, 600.0, Paper::Light).unwrap();
        let petrol = Palette::of(Paper::Light).ramp[0];
        let bars = bars_of(&scene, &[petrol]);
        assert_eq!(bars.len(), 3);
        let bottom = bars[0].1 + bars[0].3;
        for &(x, y, w, h, _) in &bars {
            assert!((y + h - bottom).abs() < 0.01, "all stand on the zero line");
            assert!((w - bars[0].2).abs() < 0.01, "all equally wide");
            let _ = x;
        }
        // the axis runs 0 to 40, so 10 is a quarter of the way up
        let full = bars[2].3 / 35.0 * 40.0;
        assert!((bars[0].3 - full / 4.0).abs() < 0.01);
        assert!((bars[1].3 - full / 2.0).abs() < 0.01);
        assert!(bars[0].0 < bars[1].0 && bars[1].0 < bars[2].0);
    }

    #[test]
    fn clustered_bars_sit_side_by_side_within_their_band() {
        let c = chart(
            ChartKind::Bar,
            Some(Grouping::Clustered),
            &[&[4.0, 6.0], &[8.0, 2.0]],
        );
        let scene = scene(&c, 600.0, Paper::Light).unwrap();
        let colors = Palette::of(Paper::Light).series(2);
        let bars = bars_of(&scene, &colors);
        assert_eq!(bars.len(), 4);
        // category 1: series 1 then series 2, touching but for a hairline
        let (a, b) = (bars[0], bars[1]);
        assert_eq!((a.4, b.4), (colors[0], colors[1]));
        assert!((a.0 + a.2 + 1.0 - b.0).abs() < 0.01);
        assert!((b.3 - 2.0 * a.3).abs() < 0.01, "8 is twice 4");
    }

    #[test]
    fn stacked_bars_reach_the_sum_of_their_values() {
        let c = chart(
            ChartKind::Bar,
            Some(Grouping::Stacked),
            &[&[10.0, 30.0], &[20.0, 10.0]],
        );
        let scene = scene(&c, 600.0, Paper::Light).unwrap();
        let colors = Palette::of(Paper::Light).series(2);
        let bars = bars_of(&scene, &colors);
        assert_eq!(bars.len(), 4);
        let (first, second) = (bars[0], bars[1]);
        // the second segment starts where the first ends, at the same x and width
        assert!((second.1 + second.3 - first.1).abs() < 0.01);
        assert_eq!((first.0, first.2), (second.0, second.2));
        // 10 + 20 = 30 and 30 + 10 = 40: the second column is a third taller
        let column = |i: usize| bars[2 * i].3 + bars[2 * i + 1].3;
        assert!((column(1) / column(0) - 4.0 / 3.0).abs() < 1e-4);
    }

    #[test]
    fn hundred_percent_bars_fill_the_plot() {
        let c = chart(
            ChartKind::Bar,
            Some(Grouping::Percent),
            &[&[1.0, 30.0], &[3.0, 10.0]],
        );
        let scene = scene(&c, 600.0, Paper::Light).unwrap();
        let colors = Palette::of(Paper::Light).series(2);
        let bars = bars_of(&scene, &colors);
        let column = |i: usize| bars[2 * i].3 + bars[2 * i + 1].3;
        assert!((column(0) - column(1)).abs() < 0.01);
        // a quarter and three quarters
        assert!((bars[0].3 / column(0) - 0.25).abs() < 1e-4);
        assert!((bars[2].3 / column(1) - 0.75).abs() < 1e-4);
    }

    #[test]
    fn horizontal_bars_grow_to_the_right() {
        let mut c = chart(ChartKind::Bar, Some(Grouping::Clustered), &[&[10.0, 20.0]]);
        c.orientation = Some(Orientation::Horizontal);
        let scene = scene(&c, 600.0, Paper::Light).unwrap();
        let petrol = Palette::of(Paper::Light).ramp[0];
        let bars: Vec<_> = scene
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Rect { x, y, w, h, fill } if *fill == petrol => Some((*x, *y, *w, *h)),
                _ => None,
            })
            .collect();
        assert_eq!(bars.len(), 2);
        assert_eq!(bars[0].0, bars[1].0, "both start at zero");
        assert!((bars[1].2 - 2.0 * bars[0].2).abs() < 0.01);
        assert!(bars[0].1 < bars[1].1, "the first category on top");
    }

    #[test]
    fn pie_slices_have_angles_in_proportion_and_close_the_circle() {
        let c = chart(ChartKind::Pie, None, &[&[1.0, 2.0, 0.0, 5.0]]);
        let scene = scene(&c, 600.0, Paper::Light).unwrap();
        let wedges: Vec<(f32, f32)> = scene
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Wedge { start, end, .. } => Some((*start, *end)),
                _ => None,
            })
            .collect();
        // the empty slice is left out
        assert_eq!(wedges.len(), 3);
        let tau = std::f32::consts::TAU;
        assert_eq!(wedges[0].0, 0.0);
        assert!((wedges[0].1 - tau / 8.0).abs() < 1e-5);
        assert!((wedges[1].1 - wedges[1].0 - tau / 4.0).abs() < 1e-5);
        assert!((wedges[2].1 - tau).abs() < 1e-5);
        let shares: Vec<&str> = scene
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Text { text, .. } if text.ends_with('%') => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(shares, ["12.5%", "25.0%", "0.0%", "62.5%"]);
    }

    #[test]
    fn the_normal_curve_peaks_at_the_mean_scaled_to_the_bars() {
        let mut c = chart(ChartKind::Histogram, None, &[&[2.0, 6.0, 2.0]]);
        for (i, category) in c.categories.iter_mut().enumerate() {
            category.bin = Some([i as f64 * 10.0, (i + 1) as f64 * 10.0]);
        }
        c.normal = Some(NormalCurve {
            mean: 15.0,
            std_dev: 5.0,
            total: 10.0,
        });
        let scene = scene(&c, 600.0, Paper::Light).unwrap();
        let curve = scene
            .shapes
            .iter()
            .find_map(|s| match s {
                Shape::Line { points, .. } if points.len() == 121 => Some(points.clone()),
                _ => None,
            })
            .expect("a curve");
        let petrol = Palette::of(Paper::Light).ramp[0];
        let bars = bars_of(&scene, &[petrol]);
        assert_eq!(bars.len(), 3);
        // the bars touch but for a hairline and span the curve's range
        assert!((bars[0].0 + bars[0].2 + 1.0 - bars[1].0).abs() < 0.01);
        assert!((curve[0][0] - (bars[0].0 - 0.5)).abs() < 0.01);
        // height at the mean: 10 cases x width 10 x density 1/(5 sqrt(2 pi)) = 7.98
        let middle = curve[60];
        let bottom = bars[1].1 + bars[1].3;
        let per_case = bars[1].3 / 6.0;
        let peak = 100.0 / (5.0 * (2.0 * std::f32::consts::PI).sqrt());
        assert!(((bottom - middle[1]) / per_case - peak).abs() < 0.01);
    }

    #[test]
    fn a_scatter_plot_needs_numbers_for_x() {
        let mut c = chart(ChartKind::Scatter, None, &[&[3.0, 5.0, 4.0]]);
        let scene_of = |c: &Chart| scene(c, 600.0, Paper::Light);
        let dots = |s: Scene| {
            s.shapes
                .iter()
                .filter(|s| matches!(s, Shape::Circle { .. }))
                .count()
        };
        assert_eq!(dots(scene_of(&c).unwrap()), 3);
        c.categories[1].value = Cell::Text("many".into());
        c.categories[1].label = "many".into();
        assert!(scene_of(&c).is_none());
    }

    #[test]
    fn series_colors_run_from_petrol_to_amber() {
        let light = Palette::of(Paper::Light);
        assert_eq!(light.series(1), vec![Color::hex(0x0B4F5C)]);
        assert_eq!(
            light.series(2),
            vec![Color::hex(0x0B4F5C), Color::hex(0xC98318)]
        );
        assert_eq!(light.series(4), light.ramp.to_vec());
        // an odd count has a neutral middle; more than four shade each side
        let three = light.series(3);
        assert_eq!(three, vec![light.ramp[0], light.neutral, light.ramp[3]]);
        let six = light.series(6);
        assert_eq!(six[1], light.ramp[0].mix(light.ramp[1], 0.5));
        assert_eq!(
            (six[2], six[3], six[5]),
            (light.ramp[1], light.ramp[2], light.ramp[3])
        );
    }

    #[test]
    fn long_labels_are_cut_and_titles_wrap() {
        let long = "A label much too long to fit the room it has been given here";
        let cut = fit(long, SMALL, 80.0);
        assert!(cut.ends_with('\u{2026}') && text_width(&cut, SMALL) <= 80.0);
        let lines = wrap(long, SMALL, 120.0);
        assert!(lines.len() > 2);
        assert!(lines.iter().all(|l| text_width(l, SMALL) <= 120.0));
        assert_eq!(lines.join(" "), long);
    }
}
