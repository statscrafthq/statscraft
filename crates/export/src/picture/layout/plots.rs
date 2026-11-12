//! The statistical plots that carry their data in `Chart::plot`: boxes, points with reference
//! lines, merges (dendrogram and icicle) and node trees. A stem-and-leaf display stays text.

use super::{Along, Anchor, Axis, Draw, Frame, LINE, PAD, SMALL, Unit, fit};
use crate::picture::{Shape, text_width};
use statscraft_engine::Cell;
use statscraft_engine::chart::{Chart, ChartKind, Orientation};
use statscraft_engine::plot::{BoxSummary, Merge, Plot, PointGroup, ReferenceLine, TreeNode};

/// The plot below `top`; returns the y below it, or None when it shows as its data table.
pub(super) fn plot(d: &mut Draw, chart: &Chart, plot: &Plot, top: f32) -> Option<f32> {
    match plot {
        Plot::Boxes { boxes } => box_plot(d, chart, boxes, top),
        Plot::Points {
            groups,
            lines,
            z_axis: None,
        } => points(d, chart, groups, lines, top),
        Plot::Merges {
            items,
            order,
            merges,
            solutions,
        } => {
            let ok = order.len() == items.len()
                && order.iter().all(|&i| i < items.len())
                && merges
                    .iter()
                    .all(|m| m.left < items.len() && m.right < items.len());
            match (ok, chart.kind) {
                (false, _) => None,
                (true, ChartKind::Icicle) => icicle(d, chart, items, order, merges, solutions, top),
                (true, _) => dendrogram(d, chart, items, order, merges, top),
            }
        }
        Plot::Tree { nodes } => tree(d, nodes, top),
        // shortcut: a stem-and-leaf display is text, and points with a third axis have no flat
        // drawing; both show as their data table until a drawing is asked for
        Plot::StemLeaf(_) | Plot::Points { .. } => None,
    }
}

/// The lowest and highest finite value.
fn range(values: impl Iterator<Item = f64>) -> Option<(f64, f64)> {
    let (lo, hi) = values
        .filter(|v| v.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v), hi.max(v))
        });
    (lo <= hi).then_some((lo, hi))
}

/// A box per label, the boxes of a label side by side when they belong to groups: the box
/// from hinge to hinge with the median across it, whiskers with caps, outliers as dots and
/// extremes as amber dots, each with its case.
fn box_plot(d: &mut Draw, chart: &Chart, boxes: &[BoxSummary], top: f32) -> Option<f32> {
    let mut labels: Vec<String> = Vec::new();
    let mut groups: Vec<&str> = Vec::new();
    for b in boxes {
        if !labels.contains(&b.label) {
            labels.push(b.label.clone());
        }
        if let Some(group) = &b.group
            && !groups.contains(&group.as_str())
        {
            groups.push(group);
        }
    }
    let values = boxes.iter().flat_map(|b| {
        let marks = b.outliers.iter().chain(&b.extremes).map(|m| m.value);
        [b.lower_whisker, b.upper_whisker].into_iter().chain(marks)
    });
    let (lo, hi) = range(values)?;
    let colors = d.pal.series(groups.len().max(1));
    let top = d.swatches(chart.legend.as_deref(), &groups, &colors, top);
    let axis = Axis::new(lo, hi, chart.value_axis.maximum, Unit::Plain);
    let n = labels.len();
    let (frame, bottom) = Frame::new(d, chart, top, Along::Bands(labels.clone()), axis, false);
    let slots = groups.len().max(1);
    let (muted, amber) = (d.pal.muted, d.pal.amber);
    for b in boxes {
        let i = labels.iter().position(|l| *l == b.label).unwrap_or(0);
        let j = b
            .group
            .as_deref()
            .and_then(|g| groups.iter().position(|x| *x == g))
            .unwrap_or(0);
        let (b0, b1) = frame.band(i.min(n - 1));
        let room = (b1 - b0) * 0.8;
        let slot = room / slots as f32;
        let start = b0 + (b1 - b0 - room) / 2.0 + slot * j as f32;
        let (a, z) = (start + slot * 0.15, start + slot * 0.85);
        let mid = (a + z) / 2.0;
        let cap = (z - a) / 4.0;
        let y = |v: f64| frame.value(v);
        let (fill, median) = match groups.is_empty() {
            true => (d.pal.ramp[1], d.pal.ramp[0]),
            false => (colors[j], d.pal.ink),
        };
        for (from, to) in [
            (b.lower_whisker, b.lower_hinge),
            (b.upper_hinge, b.upper_whisker),
        ] {
            d.line(vec![[mid, y(from)], [mid, y(to)]], 1.0, muted);
        }
        for end in [b.lower_whisker, b.upper_whisker] {
            d.line(vec![[mid - cap, y(end)], [mid + cap, y(end)]], 1.0, muted);
        }
        d.rect([a, y(b.lower_hinge)], [z, y(b.upper_hinge)], fill);
        d.line(vec![[a, y(b.median)], [z, y(b.median)]], 2.0, median);
        for (marks, color, radius) in [(&b.outliers, d.pal.ramp[0], 3.0), (&b.extremes, amber, 3.5)]
        {
            for m in marks.iter().filter(|m| m.value.is_finite()) {
                d.shapes.push(Shape::Circle {
                    center: [mid, y(m.value)],
                    radius,
                    fill: color,
                });
                let case = m.case.clone();
                d.text(mid + 6.0, y(m.value), case, SMALL, muted, Anchor::Start);
            }
        }
    }
    Some(bottom)
}

/// The part of a reference line inside the axes' ranges, as two points.
fn clip(line: &ReferenceLine, x: &Axis, y: &Axis) -> Option<[[f64; 2]; 2]> {
    let (b, m) = (line.intercept, line.slope);
    let (mut from, mut to) = (x.lo, x.hi);
    if m != 0.0 {
        let (p, q) = ((y.lo - b) / m, (y.hi - b) / m);
        from = from.max(p.min(q));
        to = to.min(p.max(q));
    } else if b < y.lo || b > y.hi {
        return None;
    }
    (from < to && b.is_finite() && m.is_finite()).then(|| [[from, b + m * from], [to, b + m * to]])
}

/// Points in groups with their reference lines, such as a normal Q-Q plot with the line of
/// the normal distribution; a point's label beside it when there are few.
fn points(
    d: &mut Draw,
    chart: &Chart,
    groups: &[PointGroup],
    lines: &[ReferenceLine],
    top: f32,
) -> Option<f32> {
    let all = || groups.iter().flat_map(|g| &g.points);
    let (x_lo, x_hi) = range(all().map(|p| p.x))?;
    let (y_lo, y_hi) = range(all().map(|p| p.y))?;
    let colors = d.pal.series(groups.len());
    let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    let top = d.swatches(chart.legend.as_deref(), &names, &colors, top);
    let x_axis = Axis::new(x_lo, x_hi, None, Unit::Plain);
    let y_axis = Axis::new(y_lo, y_hi, chart.value_axis.maximum, Unit::Plain);
    let (frame, bottom) = Frame::new(d, chart, top, Along::Numbers(x_axis), y_axis, false);
    let along = frame.along.as_ref()?;
    let at = |[x, y]: [f64; 2]| [along.at(x, frame.x0, frame.x1), frame.value(y)];
    for t in &along.ticks {
        let x = along.at(*t, frame.x0, frame.x1);
        d.line(vec![[x, frame.y0], [x, frame.y1]], 1.0, d.pal.rule);
    }
    for line in lines {
        if let Some([a, b]) = clip(line, along, &frame.value) {
            d.line(vec![at(a), at(b)], 1.5, d.pal.amber);
        }
    }
    let labelled = all().filter(|p| p.label.is_some()).count() <= 40;
    for (group, &color) in groups.iter().zip(&colors) {
        for p in group
            .points
            .iter()
            .filter(|p| p.x.is_finite() && p.y.is_finite())
        {
            let center = at([p.x, p.y]);
            d.shapes.push(Shape::Circle {
                center,
                radius: 3.0,
                fill: color.alpha(220),
            });
            if let Some(label) = p.label.clone().filter(|_| labelled) {
                let muted = d.pal.muted;
                d.text(
                    center[0] + 6.0,
                    center[1],
                    label,
                    SMALL,
                    muted,
                    Anchor::Start,
                );
            }
        }
    }
    Some(bottom)
}

/// The labels of the items down the left, in their order, and where that column ends.
fn item_labels(d: &mut Draw, labels: &[&str], y: &dyn Fn(usize) -> f32) -> f32 {
    let widest = labels
        .iter()
        .map(|l| text_width(l, SMALL))
        .fold(0.0f32, f32::max)
        .min(d.width * 0.35);
    let right = PAD + widest;
    for (p, label) in labels.iter().enumerate() {
        let label = fit(label, SMALL, widest);
        d.text(right, y(p), label, SMALL, d.pal.muted, Anchor::End);
    }
    right
}

/// The height of a row per item: less when there are many.
fn item_row(n: usize) -> f32 {
    if n > 40 { 12.0 } else { 16.0 }
}

/// The clustering as a tree lying on its side: the items down the left in the plot's order,
/// each join a vertical line at the rescaled distance (0 to 25) where its clusters combine.
fn dendrogram(
    d: &mut Draw,
    chart: &Chart,
    items: &[String],
    order: &[usize],
    merges: &[Merge],
    top: f32,
) -> Option<f32> {
    let row = item_row(items.len());
    let y0 = top + 4.0;
    let y = |p: usize| y0 + row * (p as f32 + 0.5);
    let labels: Vec<&str> = order.iter().map(|&i| items[i].as_str()).collect();
    let x0 = item_labels(d, &labels, &y) + 8.0;
    let x1 = d.width - PAD - 8.0;
    let y1 = y0 + row * items.len() as f32;
    let axis = Axis::exact(0.0, 25.0);
    let x = |h: f64| axis.at(h, x0, x1);
    let (rule, muted) = (d.pal.rule, d.pal.muted);
    for t in axis.ticks.clone() {
        d.line(vec![[x(t), y0], [x(t), y1]], 1.0, rule);
        d.text(
            x(t),
            y1 + 10.0,
            format!("{t}"),
            SMALL,
            muted,
            Anchor::Middle,
        );
    }
    let position = |i: usize| order.iter().position(|&o| o == i).unwrap_or(0);
    // every cluster: the y of its line and the height it was formed at
    let mut place: Vec<(f32, f64)> = (0..items.len()).map(|i| (y(position(i)), 0.0)).collect();
    let petrol = d.pal.ramp[0];
    for m in merges {
        let (a, b) = (place[m.left], place[m.right]);
        let h = m.rescaled.clamp(0.0, 25.0);
        d.line(
            vec![[x(a.1), a.0], [x(h), a.0], [x(h), b.0], [x(b.1), b.0]],
            1.5,
            petrol,
        );
        place[m.left] = ((a.0 + b.0) / 2.0, h);
    }
    let title = fit(&chart.category_axis.title, SMALL, x1 - x0);
    let below = y1 + 10.0 + LINE;
    d.text((x0 + x1) / 2.0, below, title, SMALL, muted, Anchor::Middle);
    Some(below + LINE / 2.0)
}

/// For every number of clusters the plot shows, from one up, the cluster of each item in the
/// plot's order, as an index that only says which items share a cluster.
fn cluster_levels(
    n: usize,
    order: &[usize],
    merges: &[Merge],
    solutions: &[usize],
) -> Vec<(usize, Vec<usize>)> {
    let mut cluster: Vec<usize> = (0..n).collect();
    let mut levels = Vec::new();
    for (step, m) in merges.iter().enumerate() {
        let (keep, gone) = (cluster[m.left], cluster[m.right]);
        cluster
            .iter_mut()
            .filter(|c| **c == gone)
            .for_each(|c| *c = keep);
        let k = n - step - 1;
        if k >= 1 && (solutions.is_empty() || solutions.contains(&k)) {
            levels.push((k, order.iter().map(|&i| cluster[i]).collect()));
        }
    }
    levels.reverse();
    levels
}

/// The clusters at every number of clusters: a bar over each run of items that share a
/// cluster. Vertical: the items across, one cluster at the top and more below; horizontal: the
/// items down the left, one cluster at the left and more to the right.
fn icicle(
    d: &mut Draw,
    chart: &Chart,
    items: &[String],
    order: &[usize],
    merges: &[Merge],
    solutions: &[usize],
    top: f32,
) -> Option<f32> {
    let n = items.len();
    let levels = cluster_levels(n, order, merges, solutions);
    if levels.is_empty() {
        return None;
    }
    let labels: Vec<&str> = order.iter().map(|&i| items[i].as_str()).collect();
    let (muted, petrol) = (d.pal.muted, d.pal.ramp[0]);
    let vertical = chart.orientation != Some(Orientation::Horizontal);
    // runs of one cluster within a level: (first item, last item)
    let runs = |state: &[usize]| {
        let mut runs = Vec::new();
        let mut start = 0;
        for p in 1..=state.len() {
            if p == state.len() || state[p] != state[start] {
                runs.push((start, p - 1));
                start = p;
            }
        }
        runs
    };
    // the title of the numbers of clusters: above them when they run down the left, below
    // them when they run along the bottom
    let title = fit(&chart.category_axis.title, SMALL, d.width - 2.0 * PAD);
    let y0 = match vertical {
        true => {
            d.text(
                PAD,
                top + LINE / 2.0,
                title.clone(),
                SMALL,
                muted,
                Anchor::Start,
            );
            top + LINE + 4.0
        }
        false => top + 4.0,
    };
    if vertical {
        let widest = text_width(&levels[levels.len() - 1].0.to_string(), SMALL);
        let x0 = PAD + widest + 8.0;
        let x1 = d.width - PAD;
        let slot = (x1 - x0) / n as f32;
        let h = if levels.len() > 30 { 10.0 } else { 16.0 };
        for (l, (k, state)) in levels.iter().enumerate() {
            let y = y0 + h * l as f32;
            d.text(
                x0 - 8.0,
                y + h / 2.0,
                k.to_string(),
                SMALL,
                muted,
                Anchor::End,
            );
            for (a, b) in runs(state) {
                let from = [x0 + slot * a as f32 + 1.0, y + 1.0];
                d.rect(
                    from,
                    [x0 + slot * (b + 1) as f32 - 1.0, y + h - 1.0],
                    petrol,
                );
            }
        }
        let y1 = y0 + h * levels.len() as f32;
        let k = (28.0 / slot).ceil().max(1.0) as usize;
        for (p, label) in labels.iter().enumerate().step_by(k) {
            let label = fit(label, SMALL, slot * k as f32 - 4.0);
            let x = x0 + slot * (p as f32 + 0.5);
            d.text(x, y1 + 10.0, label, SMALL, muted, Anchor::Middle);
        }
        return Some(y1 + LINE + 4.0);
    }
    let row = item_row(n);
    let y = |p: usize| y0 + row * (p as f32 + 0.5);
    let x0 = item_labels(d, &labels, &y) + 8.0;
    let x1 = d.width - PAD;
    let w = (x1 - x0) / levels.len() as f32;
    let y1 = y0 + row * n as f32;
    let k = (24.0 / w).ceil().max(1.0) as usize;
    for (l, (count, state)) in levels.iter().enumerate() {
        let x = x0 + w * l as f32;
        for (a, b) in runs(state) {
            let from = [x + 1.0, y0 + row * a as f32 + 1.0];
            d.rect(from, [x + w - 1.0, y0 + row * (b + 1) as f32 - 1.0], petrol);
        }
        if l % k == 0 {
            let text = count.to_string();
            d.text(x + w / 2.0, y1 + 10.0, text, SMALL, muted, Anchor::Middle);
        }
    }
    let below = y1 + 10.0 + LINE;
    d.text((x0 + x1) / 2.0, below, title, SMALL, muted, Anchor::Middle);
    Some(below + LINE / 2.0)
}

fn cell_text(name: &str, value: &Cell) -> String {
    match value {
        Cell::Number(v) if v.fract() == 0.0 => format!("{name}: {v:.0}"),
        // a significance below 1 keeps three decimals
        Cell::Number(v) if v.abs() < 1.0 => format!("{name}: {v:.3}"),
        Cell::Number(v) => format!("{name}: {v:.2}"),
        Cell::Text(t) => format!("{name}: {t}"),
        Cell::Empty => name.to_string(),
    }
}

/// A tree of nodes from the root down: each node a box with its number and statistics, its
/// leaves side by side and a parent over the middle of its children, joined by elbow lines
/// with the split that leads to each child above it.
fn tree(d: &mut Draw, nodes: &[TreeNode], top: f32) -> Option<f32> {
    let index = |id: usize| nodes.iter().position(|n| n.id == id);
    let children: Vec<Vec<usize>> = nodes
        .iter()
        .map(|n| {
            (0..nodes.len())
                .filter(|&k| nodes[k].parent == Some(n.id))
                .collect()
        })
        .collect();
    // the root first and every other node after its parent, so one pass finds the depths
    let mut depth = vec![0usize; nodes.len()];
    for (i, node) in nodes.iter().enumerate() {
        if let Some(p) = node.parent {
            let p = index(p).filter(|&p| p < i)?;
            depth[i] = depth[p] + 1;
        }
    }
    let leaves = children.iter().filter(|c| c.is_empty()).count().max(1);
    let text: Vec<Vec<String>> = nodes
        .iter()
        .map(|n| {
            let mut lines = vec![format!("Node {}", n.id)];
            lines.extend(n.statistics.iter().map(|(name, v)| cell_text(name, v)));
            lines
        })
        .collect();
    let widest = text
        .iter()
        .flatten()
        .map(|l| text_width(l, SMALL))
        .fold(0.0f32, f32::max);
    let room = d.width - 2.0 * PAD;
    let slot = (widest + 28.0).min(room / leaves as f32);
    let box_width = slot - 10.0;
    let line = 14.0;
    let box_height = |i: usize| text[i].len() as f32 * line + 8.0;
    let tallest = (0..nodes.len()).map(box_height).fold(0.0f32, f32::max);
    let level = tallest + 40.0;
    // leaves left to right in the order of the tree, parents over their children
    let mut x = vec![0.0f32; nodes.len()];
    let mut next = 0.0;
    fn place(i: usize, x: &mut [f32], next: &mut f32, children: &[Vec<usize>]) {
        if children[i].is_empty() {
            x[i] = *next;
            *next += 1.0;
            return;
        }
        for &k in &children[i] {
            place(k, x, next, children);
        }
        x[i] = children[i].iter().map(|&k| x[k]).sum::<f32>() / children[i].len() as f32;
    }
    if !nodes.is_empty() {
        place(0, &mut x, &mut next, &children);
    }
    let left = PAD + (room - slot * leaves as f32) / 2.0;
    let center = |i: usize| left + slot * (x[i] + 0.5);
    let box_top = |i: usize| top + 8.0 + level * depth[i] as f32;
    let (ink, muted, rule, tint) = (d.pal.ink, d.pal.muted, d.pal.rule, d.pal.tint);
    for (i, node) in nodes.iter().enumerate() {
        let (cx, y) = (center(i), box_top(i));
        if let Some(p) = node.parent.and_then(index) {
            let below = box_top(p) + box_height(p);
            let elbow = below + 10.0;
            let edge = vec![[center(p), below], [center(p), elbow], [cx, elbow], [cx, y]];
            d.line(edge, 1.0, muted);
            let split = fit(&node.split, SMALL, slot - 4.0);
            // on paper, so the edge does not run through the words
            let half = text_width(&split, SMALL) / 2.0 + 3.0;
            if !split.is_empty() {
                d.rect([cx - half, y - 16.0], [cx + half, y - 2.0], d.pal.paper);
            }
            d.text(cx, y - 9.0, split, SMALL, muted, Anchor::Middle);
        }
        let (a, z) = (cx - box_width / 2.0, cx + box_width / 2.0);
        let bottom = y + box_height(i);
        d.rect([a, y], [z, bottom], tint);
        d.line(
            vec![[a, y], [z, y], [z, bottom], [a, bottom], [a, y]],
            1.0,
            rule,
        );
        for (k, line_text) in text[i].iter().enumerate() {
            let t = fit(line_text, SMALL, box_width - 8.0);
            let color = if k == 0 { ink } else { muted };
            d.text(
                cx,
                y + 4.0 + line * (k as f32 + 0.5),
                t,
                SMALL,
                color,
                Anchor::Middle,
            );
        }
    }
    let bottom = (0..nodes.len())
        .map(|i| box_top(i) + box_height(i))
        .fold(top, f32::max);
    Some(bottom + 4.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::picture::{Paper, scene};
    use statscraft_engine::chart::{Axis as ChartAxis, Tier};
    use statscraft_engine::plot::{Mark, Point};

    fn plot_chart(kind: ChartKind, plot: Plot) -> Chart {
        Chart {
            kind,
            tier: Tier::Statistical,
            orientation: None,
            grouping: None,
            title: "Waiting minutes".into(),
            footnote: None,
            category_axis: ChartAxis {
                title: "Depot".into(),
                format: String::new(),
                maximum: None,
                variable: None,
            },
            value_axis: ChartAxis {
                title: "Minutes".into(),
                format: String::new(),
                maximum: None,
                variable: None,
            },
            legend: None,
            categories: Vec::new(),
            series: Vec::new(),
            normal: None,
            plot: Some(plot),
        }
    }

    fn texts(scene: &crate::picture::Scene) -> Vec<&str> {
        scene
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_box_spans_its_hinges_with_the_median_across() {
        let b = BoxSummary {
            label: "North".into(),
            group: None,
            n: 9.0,
            lower_whisker: 2.0,
            lower_hinge: 4.0,
            median: 5.0,
            upper_hinge: 8.0,
            upper_whisker: 10.0,
            outliers: vec![Mark {
                value: 16.0,
                case: "7".into(),
            }],
            extremes: vec![Mark {
                value: 23.0,
                case: "3".into(),
            }],
        };
        let chart = plot_chart(ChartKind::Boxplot, Plot::Boxes { boxes: vec![b] });
        let s = scene(&chart, 500.0, Paper::Light).unwrap();
        let light = crate::picture::Palette::of(Paper::Light);
        let body = s
            .shapes
            .iter()
            .find_map(|s| match s {
                Shape::Rect { y, h, fill, .. } if *fill == light.ramp[1] => Some((*y, *h)),
                _ => None,
            })
            .unwrap();
        let median = s
            .shapes
            .iter()
            .find_map(|s| match s {
                Shape::Line { points, width, .. } if *width == 2.0 => Some(points[0][1]),
                _ => None,
            })
            .unwrap();
        // hinges 4 and 8, median 5: a quarter of the way up the box
        assert!(((body.0 + body.1 - median) / body.1 - 0.25).abs() < 1e-4);
        let dots: Vec<Color> = s
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Circle { fill, .. } => Some(*fill),
                _ => None,
            })
            .collect();
        assert_eq!(dots, [light.ramp[0], light.amber]);
        assert!(texts(&s).contains(&"7") && texts(&s).contains(&"3"));
    }

    use crate::picture::Color;

    #[test]
    fn a_reference_line_is_cut_to_the_plot() {
        let x = Axis::new(0.0, 10.0, None, Unit::Plain);
        let y = Axis::new(0.0, 5.0, None, Unit::Plain);
        let line = ReferenceLine {
            intercept: 0.0,
            slope: 1.0,
        };
        // y = x leaves the plot at the top, where y is 5
        assert_eq!(clip(&line, &x, &y), Some([[0.0, 0.0], [5.0, 5.0]]));
        let flat = ReferenceLine {
            intercept: 9.0,
            slope: 0.0,
        };
        assert_eq!(clip(&flat, &x, &y), None);
        let points = Plot::Points {
            groups: vec![PointGroup {
                name: "wait".into(),
                points: (0..5)
                    .map(|i| Point {
                        x: i as f64,
                        y: i as f64 * 1.1,
                        z: None,
                        label: None,
                    })
                    .collect(),
            }],
            lines: vec![line],
            z_axis: None,
        };
        let s = scene(
            &plot_chart(ChartKind::NormalQq, points),
            500.0,
            Paper::Light,
        )
        .unwrap();
        let circles = s
            .shapes
            .iter()
            .filter(|s| matches!(s, Shape::Circle { .. }));
        assert_eq!(circles.count(), 5);
    }

    fn merges() -> (Vec<String>, Vec<usize>, Vec<Merge>) {
        let items = ["a", "b", "c", "d"].map(String::from).to_vec();
        let merge = |left, right, rescaled| Merge {
            left,
            right,
            height: rescaled,
            rescaled,
        };
        // a and b first, then c and d, then the two pairs
        let merges = vec![merge(0, 1, 1.0), merge(2, 3, 5.0), merge(0, 2, 25.0)];
        (items, vec![0, 1, 2, 3], merges)
    }

    #[test]
    fn icicle_levels_say_which_items_share_a_cluster() {
        let (_, order, merges) = merges();
        let levels = cluster_levels(4, &order, &merges, &[]);
        let shown: Vec<(usize, Vec<usize>)> = levels;
        assert_eq!(shown[0], (1, vec![0, 0, 0, 0]));
        assert_eq!(shown[1], (2, vec![0, 0, 2, 2]));
        assert_eq!(shown[2], (3, vec![0, 0, 2, 3]));
        assert_eq!(cluster_levels(4, &order, &merges, &[2]).len(), 1);
    }

    #[test]
    fn a_dendrogram_joins_at_the_rescaled_distance() {
        let (items, order, merges) = merges();
        let plot = Plot::Merges {
            items,
            order,
            merges,
            solutions: Vec::new(),
        };
        let s = scene(
            &plot_chart(ChartKind::Dendrogram, plot),
            500.0,
            Paper::Light,
        )
        .unwrap();
        let petrol = crate::picture::Palette::of(Paper::Light).ramp[0];
        let joins: Vec<f32> = s
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Line { points, color, .. } if *color == petrol => Some(points[1][0]),
                _ => None,
            })
            .collect();
        assert_eq!(joins.len(), 3);
        // 1, 5 and 25 on a 0 to 25 axis
        let per = (joins[2] - joins[0]) / 24.0;
        assert!((joins[1] - joins[0] - 4.0 * per).abs() < 0.01);
        assert!(texts(&s).contains(&"a"));
    }

    #[test]
    fn a_tree_puts_parents_over_the_middle_of_their_children() {
        let node = |id, parent, split: &str| TreeNode {
            id,
            parent,
            split: split.into(),
            statistics: vec![("n".into(), Cell::Number(10.0))],
        };
        let nodes = vec![
            node(0, None, ""),
            node(1, Some(0), "plan = 1"),
            node(2, Some(0), "plan = 2; 3"),
        ];
        let s = scene(
            &plot_chart(ChartKind::TreeDiagram, Plot::Tree { nodes }),
            500.0,
            Paper::Light,
        )
        .unwrap();
        let tint = crate::picture::Palette::of(Paper::Light).tint;
        let boxes: Vec<(f32, f32)> = s
            .shapes
            .iter()
            .filter_map(|s| match s {
                Shape::Rect { x, w, fill, .. } if *fill == tint => Some((*x + w / 2.0, *w)),
                _ => None,
            })
            .collect();
        assert_eq!(boxes.len(), 3);
        assert!((boxes[0].0 - (boxes[1].0 + boxes[2].0) / 2.0).abs() < 0.01);
        let t = texts(&s);
        assert!(t.contains(&"Node 2") && t.contains(&"plan = 2; 3") && t.contains(&"n: 10"));
    }

    #[test]
    fn a_stem_and_leaf_display_stays_a_table() {
        let plot = Plot::StemLeaf(statscraft_engine::plot::StemLeaf {
            stem_width: 10.0,
            each_leaf: 1.0,
            low_extremes: None,
            lines: Vec::new(),
            high_extremes: None,
        });
        assert!(scene(&plot_chart(ChartKind::StemLeaf, plot), 500.0, Paper::Light).is_none());
    }
}
