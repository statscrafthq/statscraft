//! The gain tables and charts of a categorical dependent: for each target category the terminal
//! nodes in order of their response, node by node and cumulatively, and over percentile groups.
//!
//! Rules from the Algorithms manual (Gain Summary), in our words. A terminal node t has a count
//! n(t), a target count g(t), a weight w(t) and a numerator a(t): with priors from the data (and
//! always for CHAID) w is n and a is g; with other priors w is the node's probability
//! p(t) = sum over the classes i of pi(i) N_i(t) / N_i and a is p(target, t) = pi(target)
//! g(t) / N_target, the counts being the sample's own (a test sample has its own totals, the
//! priors are the training sample's). Then
//! - the score of a node is a / w (its response), the node's size share w / sum of w, its gain
//!   share g / N_target, its index the score over the score of the whole sample, N_target / N
//!   with priors from the data and pi(target) otherwise;
//! - the cumulative figures add n, w, a and g over the nodes in order, the score being sum a
//!   over sum w;
//! - the percentile p of increment q is the first N q p / 100 cases of the nodes in order, the
//!   node that straddles the cut counting in proportion to the part of its cases below it for
//!   n, g, w and a alike; the table gives the rounded size, the gain count and share, the score
//!   and its index, and lists the nodes from the one that holds the previous cut (the next one
//!   when the cut falls on a boundary) to the one that holds this cut.
//! - Nodes are sorted by score, highest first unless ascending, in the training sample's order
//!   for every sample.

use super::{Key, Node, Tree, empirical_priors, key_of, percent, text};
use crate::{Cell, Table};

/// The terminal nodes of a sample as the gain tables of one target category see them.
pub(super) struct Gain<'a> {
    pub nodes: &'a [Node],
    pub target: usize,
    pub empirical: bool,
}

/// The figures of one terminal node: count, target count, weight and numerator of the score, and
/// the numerator of the expense score (profits only).
#[derive(Clone, Copy)]
pub(super) struct Figures {
    pub n: f64,
    pub g: f64,
    pub w: f64,
    pub a: f64,
    pub b: f64,
}

impl Gain<'_> {
    /// The prior of a class: the nodes of a test sample keep the training sample's `assign`, so
    /// its root holds the training sample's priors (the tests check the figures against the
    /// counts and priors worked out apart from this code).
    fn prior(&self, class: usize) -> f64 {
        let root = &self.nodes[0];
        root.assign[class] / root.assign.iter().sum::<f64>()
    }

    pub(super) fn figures(&self, t: usize) -> Figures {
        let (node, root) = (&self.nodes[t], &self.nodes[0]);
        let (n, g) = (node.n, node.counts[self.target]);
        if self.empirical {
            return Figures {
                n,
                g,
                w: n,
                a: g,
                b: 0.0,
            };
        }
        let share = |class: usize| {
            if root.counts[class] > 0.0 {
                self.prior(class) * node.counts[class] / root.counts[class]
            } else {
                0.0
            }
        };
        let w: f64 = (0..node.counts.len()).map(share).sum();
        Figures {
            n,
            g,
            w,
            a: share(self.target),
            b: 0.0,
        }
    }

    /// The score of the sample as a whole: what the index divides by.
    pub(super) fn base(&self) -> f64 {
        if self.empirical {
            self.nodes[0].counts[self.target] / self.nodes[0].n
        } else {
            self.prior(self.target)
        }
    }

    /// The terminal nodes by score, the highest first unless `ascending`, scores within 1e-12 of
    /// each other tied (the nodes keep their order); a node without cases has no score and goes
    /// last.
    pub(super) fn order(&self, terminals: &[usize], ascending: bool) -> Vec<usize> {
        let key = |t: usize| {
            let f = self.figures(t);
            // the score to 12 places as a whole number, so that rounding noise does not order
            (f.w > 0.0).then(|| (f.a / f.w * 1e12).round() as i64)
        };
        let mut order = terminals.to_vec();
        order.sort_by(|&a, &b| match (key(a), key(b)) {
            (Some(x), Some(y)) if ascending => x.cmp(&y),
            (Some(x), Some(y)) => y.cmp(&x),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        order
    }
}

/// One row of a percentile gain table.
pub(super) struct Percentile {
    /// The percentile, p q.
    pub percent: usize,
    /// The number of cases in it, rounded (halves away from zero).
    pub size: f64,
    pub gain: f64,
    pub gain_share: f64,
    pub score: f64,
    /// The expense per case (profits only).
    pub expense: f64,
    /// The terminal nodes that belong to it.
    pub nodes: Vec<usize>,
}

impl Gain<'_> {
    /// The percentiles of increment `q` over the nodes in `order`.
    pub(super) fn percentiles(&self, order: &[usize], q: usize) -> Vec<Percentile> {
        let figures: Vec<Figures> = order.iter().map(|&t| self.figures(t)).collect();
        percentile_rows(
            &figures,
            order,
            self.nodes[0].n,
            self.nodes[0].counts[self.target],
            q,
        )
    }
}

/// The percentiles of increment `q` over nodes with the `figures` they have in `order`, for a
/// sample of `total` cases and `target` target cases (0 when there is no target category).
fn percentile_rows(
    figures: &[Figures],
    order: &[usize],
    total: f64,
    target: f64,
    q: usize,
) -> Vec<Percentile> {
    // the cases up to and including each node
    let mut cumulative = Vec::new();
    let mut running = 0.0;
    for f in figures {
        running += f.n;
        cumulative.push(running);
    }
    let last = order.len() - 1;
    // the first node whose cumulative count reaches the cut, and the first that passes it
    let reaching = |cut: f64| {
        (0..=last)
            .find(|&k| cumulative[k] >= cut * (1.0 - 1e-12))
            .unwrap_or(last)
    };
    let passing = |cut: f64| (0..=last).find(|&k| cumulative[k] > cut * (1.0 + 1e-12));
    let mut rows = Vec::new();
    let mut previous_cut = 0.0;
    for percent in (q..=100).step_by(q) {
        let cut = total * percent as f64 / 100.0;
        let k = reaching(cut);
        let before = |field: fn(&Figures) -> f64| (0..k).map(|i| field(&figures[i])).sum::<f64>();
        let (before_n, f) = (before(|f| f.n), figures[k]);
        let part = if f.n > 0.0 {
            ((cut - before_n) / f.n).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (gain, w, a, b) = (
            before(|f| f.g) + part * f.g,
            before(|f| f.w) + part * f.w,
            before(|f| f.a) + part * f.a,
            before(|f| f.b) + part * f.b,
        );
        // the nodes from the one that holds the previous cut (the next one when that cut falls
        // on a boundary) to the one that holds this cut
        let from = if percent == q {
            0
        } else {
            passing(previous_cut).unwrap_or(last)
        };
        rows.push(Percentile {
            percent,
            size: cut.round(),
            gain,
            gain_share: if target > 0.0 {
                gain / target
            } else {
                f64::NAN
            },
            score: if w > 0.0 { a / w } else { f64::NAN },
            expense: if w > 0.0 { b / w } else { f64::NAN },
            nodes: order[from.min(k)..=k].to_vec(),
        });
        previous_cut = cut;
    }
    rows
}

/// What the tables need to know besides the samples.
pub(super) struct Context<'a> {
    pub command: &'a Tree,
    /// The samples described, the training sample first when there is a test sample too.
    pub samples: &'a [(&'a str, &'a [Node])],
    /// The tree's own nodes: their order of the terminal nodes holds for every sample.
    pub training: &'a [Node],
    pub terminals: &'a [usize],
    pub classes: &'a [Key],
    pub class_labels: &'a [String],
    /// The dependent as shown, and its name.
    pub dependent: &'a str,
    pub dependent_name: &'a str,
    pub validated: bool,
}

impl Context<'_> {
    /// The target categories that are classes of the analysis.
    fn targets(&self) -> Vec<usize> {
        self.command
            .targets
            .iter()
            .filter_map(|t| self.classes.iter().position(|k| *k == key_of(t)))
            .collect()
    }

    fn empirical(&self) -> bool {
        empirical_priors(self.command)
    }

    /// The gain tables, node by node and by percentile as asked, a table for each sample and target
    /// category.
    pub(super) fn node_tables(&self, out: &mut Vec<Table>) {
        if !self.command.gain_table {
            return;
        }
        for (label, ns) in self.samples {
            for j in self.targets() {
                let gain = Gain {
                    nodes: ns,
                    target: j,
                    empirical: self.empirical(),
                };
                let training = Gain {
                    nodes: self.training,
                    target: j,
                    empirical: self.empirical(),
                };
                if self.command.gain_node {
                    out.push(self.node_table(label, &gain, &training));
                }
                if self.command.gain_ptile {
                    out.push(self.percentile_table(label, &gain, &training));
                }
            }
        }
    }

    fn node_table(&self, label: &str, gain: &Gain, training: &Gain) -> Table {
        let command = self.command;
        let j = gain.target;
        let target_total = gain.nodes[0].counts[j];
        let base = gain.base();
        let order = training.order(self.terminals, command.gain_ascending);
        let total_w: f64 = self.terminals.iter().map(|&t| gain.figures(t).w).sum();
        let mut columns = vec!["Node".to_string()];
        let mut groups: Vec<Vec<String>> = vec![Vec::new()];
        let sets: &[&str] = if command.gain_cumulative {
            &["", "Cumulative"]
        } else {
            &[""]
        };
        for set in sets {
            for (group, name) in [
                ("Node", "N"),
                ("Node", "Percent"),
                ("Gain", "N"),
                ("Gain", "Percent"),
                ("", "Response"),
                ("", "Index"),
            ] {
                columns.push(name.to_string());
                let mut path = Vec::new();
                if !set.is_empty() {
                    path.push(set.to_string());
                }
                if !group.is_empty() {
                    path.push(group.to_string());
                } else if !path.is_empty() {
                    path.push(String::new());
                }
                groups.push(path);
            }
        }
        let mut sum = Figures {
            n: 0.0,
            g: 0.0,
            w: 0.0,
            a: 0.0,
            b: 0.0,
        };
        let mut rows = Vec::new();
        for &t in &order {
            let f = gain.figures(t);
            sum = Figures {
                n: sum.n + f.n,
                g: sum.g + f.g,
                w: sum.w + f.w,
                a: sum.a + f.a,
                b: sum.b + f.b,
            };
            let mut row = vec![text(t.to_string())];
            for f in if command.gain_cumulative {
                vec![f, sum]
            } else {
                vec![f]
            } {
                let score = (f.w > 0.0).then(|| f.a / f.w);
                row.extend([
                    Cell::Number(f.n),
                    percent(f.w, total_w),
                    Cell::Number(f.g),
                    percent(f.g, target_total),
                    score.map_or(Cell::Empty, |s| Cell::Number(100.0 * s)),
                    match score {
                        Some(s) if base > 0.0 => Cell::Number(100.0 * s / base),
                        _ => Cell::Empty,
                    },
                ]);
            }
            rows.push(row);
        }
        let formats = rows
            .iter()
            .map(|row| {
                columns
                    .iter()
                    .zip(row)
                    .map(|(c, _)| match c.as_str() {
                        "Percent" | "Response" | "Index" => "PCT6.1".to_string(),
                        "N" => "F8.0".to_string(),
                        _ => String::new(),
                    })
                    .collect()
            })
            .collect();
        Table {
            title: if self.validated {
                format!("Gain Summary for Nodes ({label} Sample)")
            } else {
                "Gain Summary for Nodes".into()
            },
            row_headers: 1,
            columns,
            column_groups: groups,
            rows,
            formats,
            caption: Some(format!(
                "Target category: {}. Dependent variable: {}.",
                self.class_labels[j], self.dependent
            )),
            outline: None,
            hidden: false,
            cell_stats: None,
            decorations: Vec::new(),
            style: Default::default(),
        }
    }

    /// The percentile gain table of a sample and target category: for every percentile the number
    /// of cases, the gain count and share, the response and the index, and the terminal nodes that
    /// belong to it.
    fn percentile_table(&self, label: &str, gain: &Gain, training: &Gain) -> Table {
        let j = gain.target;
        let base = gain.base();
        let order = training.order(self.terminals, self.command.gain_ascending);
        let rows: Vec<Vec<Cell>> = gain
            .percentiles(&order, self.command.gain_increment)
            .iter()
            .map(|r| {
                let valid = |x: f64| (!x.is_nan()).then_some(x);
                vec![
                    Cell::Number(r.percent as f64),
                    Cell::Number(r.size),
                    Cell::Number(r.gain),
                    valid(r.gain_share).map_or(Cell::Empty, |g| Cell::Number(100.0 * g)),
                    valid(r.score).map_or(Cell::Empty, |s| Cell::Number(100.0 * s)),
                    match valid(r.score) {
                        Some(s) if base > 0.0 => Cell::Number(100.0 * s / base),
                        _ => Cell::Empty,
                    },
                    text(
                        r.nodes
                            .iter()
                            .map(|t| t.to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                ]
            })
            .collect();
        let columns: Vec<String> = [
            "Percentile",
            "N",
            "Gain N",
            "Gain Percent",
            "Response",
            "Index",
            "Nodes",
        ]
        .map(String::from)
        .to_vec();
        let formats = vec![
            vec![
                "F8.0".to_string(),
                "F8.0".into(),
                "F8.2".into(),
                "PCT6.1".into(),
                "PCT6.1".into(),
                "PCT6.1".into(),
                String::new(),
            ];
            rows.len()
        ];
        Table {
            title: if self.validated {
                format!("Gain Summary for Percentiles ({label} Sample)")
            } else {
                "Gain Summary for Percentiles".into()
            },
            row_headers: 1,
            column_groups: vec![Vec::new(); columns.len()],
            columns,
            rows,
            formats,
            caption: Some(format!(
                "Target category: {}. Dependent variable: {}.",
                self.class_labels[j], self.dependent
            )),
            outline: None,
            hidden: false,
            cell_stats: None,
            decorations: Vec::new(),
            style: Default::default(),
        }
    }

    /// The gain, response and index charts over percentile groups, a line per target category.
    pub(super) fn charts(&self, charts: &mut Vec<crate::Chart>) {
        use crate::chart::{Axis, Category, PERCENT_FORMAT, Series, Statistic};
        let command = self.command;
        let wanted = [
            (command.plot_gain, "Gain", 0),
            (command.plot_response, "Response", 1),
            (command.plot_index, "Index", 2),
        ];
        let targets = self.targets();
        if !wanted.iter().any(|w| w.0) || targets.is_empty() {
            return;
        }
        let q = command.plot_increment;
        let categories: Vec<Category> = (q..=100)
            .step_by(q)
            .map(|p| Category {
                label: format!("{p}%"),
                value: Cell::Number(p as f64),
                missing: false,
                bin: None,
            })
            .collect();
        for (label, ns) in self.samples {
            // a line per target category: gain %, response % and index % at each percentile
            let lines: Vec<(usize, [Vec<f64>; 3])> = targets
                .iter()
                .filter(|&&j| ns[0].counts[j] > 0.0)
                .map(|&j| {
                    let gain = Gain {
                        nodes: ns,
                        target: j,
                        empirical: self.empirical(),
                    };
                    let training = Gain {
                        nodes: self.training,
                        target: j,
                        empirical: self.empirical(),
                    };
                    let order = training.order(self.terminals, command.gain_ascending);
                    let base = gain.base();
                    let rows = gain.percentiles(&order, q);
                    let column = |f: &dyn Fn(&Percentile) -> f64| rows.iter().map(f).collect();
                    (
                        j,
                        [
                            column(&|r| 100.0 * r.gain_share),
                            column(&|r| 100.0 * r.score),
                            column(&|r| {
                                if base > 0.0 {
                                    100.0 * r.score / base
                                } else {
                                    f64::NAN
                                }
                            }),
                        ],
                    )
                })
                .collect();
            for (asked, name, k) in wanted {
                if !asked {
                    continue;
                }
                let series = lines
                    .iter()
                    .map(|(j, values)| Series {
                        name: self.class_labels[*j].clone(),
                        statistic: Statistic::Percent,
                        variable: Some(self.dependent_name.to_string()),
                        value: Cell::Empty,
                        values: values[k].clone(),
                    })
                    .collect();
                let mut chart = crate::Chart::lines(
                    if self.validated {
                        format!("{name} chart ({label} Sample)")
                    } else {
                        format!("{name} chart")
                    },
                    Axis::titled("Percentile"),
                    Axis {
                        format: PERCENT_FORMAT.into(),
                        ..Axis::titled(format!("{name} (%)"))
                    },
                    categories.clone(),
                    series,
                );
                chart.legend = Some(self.dependent.to_string());
                chart.footnote = Some(format!(
                    "Dependent variable: {}. Cumulative over percentile groups of {q}%",
                    self.dependent
                ));
                charts.push(chart);
            }
        }
    }
}

/// The terminal nodes of a sample as the gain tables of a scale dependent see them: the score of a
/// node is the mean of the dependent in it, its weight the cases, the numerator their sum.
pub(super) struct MeanGain<'a> {
    pub nodes: &'a [Node],
}

impl MeanGain<'_> {
    fn mean(node: &Node) -> f64 {
        node.counts[1] / node.counts[0] + node.shift
    }

    pub(super) fn figures(&self, t: usize) -> Figures {
        let node = &self.nodes[t];
        let a = if node.n > 0.0 {
            Self::mean(node) * node.n
        } else {
            0.0
        };
        Figures {
            n: node.n,
            g: 0.0,
            w: node.n,
            a,
            b: 0.0,
        }
    }

    /// The mean of the whole sample: what the index divides by.
    pub(super) fn base(&self) -> f64 {
        Self::mean(&self.nodes[0])
    }

    /// The terminal nodes by mean, the largest first unless `ascending`; a node without cases goes
    /// last.
    pub(super) fn order(&self, terminals: &[usize], ascending: bool) -> Vec<usize> {
        let key = |t: usize| (self.nodes[t].n > 0.0).then(|| Self::mean(&self.nodes[t]));
        let mut order = terminals.to_vec();
        order.sort_by(|&a, &b| match (key(a), key(b)) {
            (Some(x), Some(y)) if ascending => x.total_cmp(&y),
            (Some(x), Some(y)) => y.total_cmp(&x),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        order
    }

    pub(super) fn percentiles(&self, order: &[usize], q: usize) -> Vec<Percentile> {
        let figures: Vec<Figures> = order.iter().map(|&t| self.figures(t)).collect();
        percentile_rows(&figures, order, self.nodes[0].n, 0.0, q)
    }
}

/// What the gain tables of a scale dependent need.
pub(super) struct MeanContext<'a> {
    pub command: &'a Tree,
    pub samples: &'a [(&'a str, &'a [Node])],
    pub training: &'a [Node],
    pub terminals: &'a [usize],
    pub validated: bool,
    /// The dependent as shown.
    pub dependent: &'a str,
}

impl MeanContext<'_> {
    fn title(&self, kind: &str, label: &str) -> String {
        if self.validated {
            format!("Gain Summary for {kind} ({label} Sample)")
        } else {
            format!("Gain Summary for {kind}")
        }
    }

    /// The summary tables of every sample: terminal nodes by mean, with cumulative figures, and
    /// percentile groups, as asked.
    pub(super) fn tables(&self, out: &mut Vec<Table>) {
        if !self.command.gain_summary {
            return;
        }
        for (label, ns) in self.samples {
            let gain = MeanGain { nodes: ns };
            let training = MeanGain {
                nodes: self.training,
            };
            let order = training.order(self.terminals, self.command.gain_ascending);
            if self.command.gain_node {
                out.push(self.node_table(label, &gain, &order));
            }
            if self.command.gain_ptile {
                out.push(self.percentile_table(label, &gain, &order));
            }
        }
    }

    /// The mean chart: the mean of the first p percent of the cases at each percentile, the nodes
    /// in the training sample's order of means.
    pub(super) fn charts(&self, charts: &mut Vec<crate::Chart>) {
        use crate::chart::{Axis, Category, Series, Statistic};
        if !self.command.plot_mean {
            return;
        }
        let q = self.command.plot_increment;
        let categories: Vec<Category> = (q..=100)
            .step_by(q)
            .map(|p| Category {
                label: format!("{p}%"),
                value: Cell::Number(p as f64),
                missing: false,
                bin: None,
            })
            .collect();
        let order = MeanGain {
            nodes: self.training,
        }
        .order(self.terminals, self.command.gain_ascending);
        for (label, ns) in self.samples {
            let values = MeanGain { nodes: ns }
                .percentiles(&order, q)
                .iter()
                .map(|r| r.score)
                .collect();
            let mut chart = crate::Chart::lines(
                if self.validated {
                    format!("Mean chart ({label} Sample)")
                } else {
                    "Mean chart".to_string()
                },
                Axis::titled("Percentile"),
                Axis::titled("Mean"),
                categories.clone(),
                vec![Series {
                    name: "Mean".into(),
                    statistic: Statistic::Mean,
                    variable: Some(self.dependent.to_string()),
                    value: Cell::Empty,
                    values,
                }],
            );
            chart.footnote = Some(format!(
                "Dependent variable: {}. Cumulative over percentile groups of {q}%",
                self.dependent
            ));
            charts.push(chart);
        }
    }

    fn node_table(&self, label: &str, gain: &MeanGain, order: &[usize]) -> Table {
        let total_n = gain.nodes[0].n;
        let base = gain.base();
        let cumulative = self.command.gain_cumulative;
        let mut columns = vec!["Node".to_string()];
        let mut groups: Vec<Vec<String>> = vec![Vec::new()];
        for set in if cumulative {
            vec!["", "Cumulative"]
        } else {
            vec![""]
        } {
            for name in ["N", "Percent", "Mean", "Index"] {
                columns.push(name.to_string());
                groups.push(if set.is_empty() {
                    Vec::new()
                } else {
                    vec![set.to_string()]
                });
            }
        }
        let (mut sum_n, mut sum_a) = (0.0, 0.0);
        let rows: Vec<Vec<Cell>> = order
            .iter()
            .map(|&t| {
                let f = gain.figures(t);
                sum_n += f.n;
                sum_a += f.a;
                let mut row = vec![text(t.to_string())];
                for (n, a) in if cumulative {
                    vec![(f.n, f.a), (sum_n, sum_a)]
                } else {
                    vec![(f.n, f.a)]
                } {
                    let mean = (n > 0.0).then(|| a / n);
                    row.extend([
                        Cell::Number(n),
                        percent(n, total_n),
                        mean.map_or(Cell::Empty, Cell::Number),
                        match mean {
                            Some(m) if base != 0.0 => Cell::Number(100.0 * m / base),
                            _ => Cell::Empty,
                        },
                    ]);
                }
                row
            })
            .collect();
        let formats = rows
            .iter()
            .map(|row| {
                columns
                    .iter()
                    .zip(row)
                    .map(|(c, _)| match c.as_str() {
                        "N" => "F8.0".to_string(),
                        "Percent" => "F5.1".to_string(),
                        "Mean" => "F8.3".to_string(),
                        "Index" => "F6.1".to_string(),
                        _ => String::new(),
                    })
                    .collect()
            })
            .collect();
        Table {
            title: self.title("Nodes", label),
            row_headers: 1,
            column_groups: groups,
            columns,
            rows,
            formats,
            caption: Some(format!(
                "Growing Method: {}.",
                super::method_name(self.command)
            )),
            ..Table::default()
        }
    }

    fn percentile_table(&self, label: &str, gain: &MeanGain, order: &[usize]) -> Table {
        let base = gain.base();
        let rows: Vec<Vec<Cell>> = gain
            .percentiles(order, self.command.gain_increment)
            .iter()
            .map(|r| {
                let mean = (!r.score.is_nan()).then_some(r.score);
                vec![
                    Cell::Number(r.percent as f64),
                    Cell::Number(r.size),
                    mean.map_or(Cell::Empty, Cell::Number),
                    match mean {
                        Some(m) if base != 0.0 => Cell::Number(100.0 * m / base),
                        _ => Cell::Empty,
                    },
                    text(
                        r.nodes
                            .iter()
                            .map(|t| t.to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                ]
            })
            .collect();
        let columns: Vec<String> = ["Percentile", "N", "Mean", "Index", "Nodes"]
            .map(String::from)
            .to_vec();
        let formats = vec![
            vec![
                "F8.0".to_string(),
                "F8.0".into(),
                "F8.3".into(),
                "F6.1".into(),
                String::new(),
            ];
            rows.len()
        ];
        Table {
            title: self.title("Percentiles", label),
            row_headers: 1,
            column_groups: vec![Vec::new(); columns.len()],
            columns,
            rows,
            formats,
            caption: Some(format!(
                "Growing Method: {}.",
                super::method_name(self.command)
            )),
            ..Table::default()
        }
    }
}

/// The revenue and the expense of each class (PROFITS CUSTOM).
pub(super) struct Profits {
    pub revenue: Vec<f64>,
    pub expense: Vec<f64>,
}

/// The terminal nodes of a sample as the profit summaries see them: the score of a node is its
/// average profit (revenue less expense of the classes of its cases), its expense score the
/// average expense, and the return on investment the first over the second. With priors from the
/// data the average runs over cases; with others over the node's probability, as in `Gain`.
pub(super) struct ProfitGain<'a> {
    pub nodes: &'a [Node],
    pub profits: &'a Profits,
    pub empirical: bool,
}

impl ProfitGain<'_> {
    fn prior(&self, class: usize) -> f64 {
        let root = &self.nodes[0];
        root.assign[class] / root.assign.iter().sum::<f64>()
    }

    fn profit(&self, class: usize) -> f64 {
        self.profits.revenue[class] - self.profits.expense[class]
    }

    pub(super) fn figures(&self, t: usize) -> Figures {
        let (node, root) = (&self.nodes[t], &self.nodes[0]);
        // the weight of each class's cases in the node: their count, or their probability
        let share = |class: usize| {
            if self.empirical {
                node.counts[class]
            } else if root.counts[class] > 0.0 {
                self.prior(class) * node.counts[class] / root.counts[class]
            } else {
                0.0
            }
        };
        let classes = node.counts.len();
        Figures {
            n: node.n,
            g: 0.0,
            w: (0..classes).map(share).sum(),
            a: (0..classes).map(|j| share(j) * self.profit(j)).sum(),
            b: (0..classes)
                .map(|j| share(j) * self.profits.expense[j])
                .sum(),
        }
    }

    /// The average profit of the sample as a whole: what the index divides by.
    pub(super) fn base(&self) -> f64 {
        let root = &self.nodes[0];
        let classes = root.counts.len();
        if self.empirical {
            (0..classes)
                .map(|j| root.counts[j] * self.profit(j))
                .sum::<f64>()
                / root.n
        } else {
            (0..classes).map(|j| self.prior(j) * self.profit(j)).sum()
        }
    }

    /// The terminal nodes by average profit, the largest first unless `ascending`; a node without
    /// cases goes last.
    pub(super) fn order(&self, terminals: &[usize], ascending: bool) -> Vec<usize> {
        let key = |t: usize| {
            let f = self.figures(t);
            (f.w > 0.0).then(|| (f.a / f.w * 1e9).round() as i64)
        };
        let mut order = terminals.to_vec();
        order.sort_by(|&a, &b| match (key(a), key(b)) {
            (Some(x), Some(y)) if ascending => x.cmp(&y),
            (Some(x), Some(y)) => y.cmp(&x),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        order
    }

    pub(super) fn percentiles(&self, order: &[usize], q: usize) -> Vec<Percentile> {
        let figures: Vec<Figures> = order.iter().map(|&t| self.figures(t)).collect();
        percentile_rows(&figures, order, self.nodes[0].n, 0.0, q)
    }
}

/// What the profit summaries need.
pub(super) struct ProfitContext<'a> {
    pub command: &'a Tree,
    pub samples: &'a [(&'a str, &'a [Node])],
    pub training: &'a [Node],
    pub terminals: &'a [usize],
    pub profits: &'a Profits,
    pub validated: bool,
    pub dependent: &'a str,
}

/// The return on investment in percent: profit over expense; none without an expense.
fn roi(profit: f64, expense: f64) -> Option<f64> {
    (expense > 0.0).then(|| 100.0 * profit / expense)
}

impl ProfitContext<'_> {
    fn gain<'a>(&self, nodes: &'a [Node]) -> ProfitGain<'a>
    where
        Self: 'a,
    {
        ProfitGain {
            nodes,
            profits: self.profits,
            empirical: empirical_priors(self.command),
        }
    }

    fn title(&self, kind: &str, label: &str) -> String {
        if self.validated {
            format!("Profit Summary for {kind} ({label} Sample)")
        } else {
            format!("Profit Summary for {kind}")
        }
    }

    /// The profit summaries as asked: terminal nodes with cumulative figures, percentile groups.
    pub(super) fn tables(&self, out: &mut Vec<Table>) {
        if !self.command.gain_summary {
            return;
        }
        for (label, ns) in self.samples {
            let gain = self.gain(ns);
            let order = self
                .gain(self.training)
                .order(self.terminals, self.command.gain_ascending);
            if self.command.gain_node {
                out.push(self.node_table(label, &gain, &order));
            }
            if self.command.gain_ptile {
                out.push(self.percentile_table(label, &gain, &order));
            }
        }
    }

    fn node_table(&self, label: &str, gain: &ProfitGain, order: &[usize]) -> Table {
        let total_w: f64 = self.terminals.iter().map(|&t| gain.figures(t).w).sum();
        let base = gain.base();
        let cumulative = self.command.gain_cumulative;
        let mut columns = vec!["Node".to_string()];
        let mut groups: Vec<Vec<String>> = vec![Vec::new()];
        for set in if cumulative {
            vec!["", "Cumulative"]
        } else {
            vec![""]
        } {
            for name in ["N", "Percent", "Average Profit", "ROI", "Index"] {
                columns.push(name.to_string());
                groups.push(if set.is_empty() {
                    Vec::new()
                } else {
                    vec![set.to_string()]
                });
            }
        }
        let mut sum = Figures {
            n: 0.0,
            g: 0.0,
            w: 0.0,
            a: 0.0,
            b: 0.0,
        };
        let rows: Vec<Vec<Cell>> = order
            .iter()
            .map(|&t| {
                let f = gain.figures(t);
                sum = Figures {
                    n: sum.n + f.n,
                    g: 0.0,
                    w: sum.w + f.w,
                    a: sum.a + f.a,
                    b: sum.b + f.b,
                };
                let mut row = vec![text(t.to_string())];
                for f in if cumulative { vec![f, sum] } else { vec![f] } {
                    let score = (f.w > 0.0).then(|| f.a / f.w);
                    row.extend([
                        Cell::Number(f.n),
                        percent(f.w, total_w),
                        score.map_or(Cell::Empty, Cell::Number),
                        roi(f.a, f.b).map_or(Cell::Empty, Cell::Number),
                        match score {
                            Some(s) if base != 0.0 => Cell::Number(100.0 * s / base),
                            _ => Cell::Empty,
                        },
                    ]);
                }
                row
            })
            .collect();
        let formats = rows
            .iter()
            .map(|row| {
                columns
                    .iter()
                    .zip(row)
                    .map(|(c, _)| match c.as_str() {
                        "N" => "F8.0".to_string(),
                        "Percent" | "ROI" | "Index" => "PCT6.1".to_string(),
                        "Average Profit" => "F10.3".to_string(),
                        _ => String::new(),
                    })
                    .collect()
            })
            .collect();
        Table {
            title: self.title("Nodes", label),
            row_headers: 1,
            column_groups: groups,
            columns,
            rows,
            formats,
            caption: Some(format!("Dependent variable: {}.", self.dependent)),
            ..Table::default()
        }
    }

    fn percentile_table(&self, label: &str, gain: &ProfitGain, order: &[usize]) -> Table {
        let base = gain.base();
        let rows: Vec<Vec<Cell>> = gain
            .percentiles(order, self.command.gain_increment)
            .iter()
            .map(|r| {
                let score = (!r.score.is_nan()).then_some(r.score);
                vec![
                    Cell::Number(r.percent as f64),
                    Cell::Number(r.size),
                    score.map_or(Cell::Empty, Cell::Number),
                    roi(r.score, r.expense).map_or(Cell::Empty, Cell::Number),
                    match score {
                        Some(s) if base != 0.0 => Cell::Number(100.0 * s / base),
                        _ => Cell::Empty,
                    },
                    text(
                        r.nodes
                            .iter()
                            .map(|t| t.to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                ]
            })
            .collect();
        let columns: Vec<String> = ["Percentile", "N", "Average Profit", "ROI", "Index", "Nodes"]
            .map(String::from)
            .to_vec();
        let formats = vec![
            vec![
                "F8.0".to_string(),
                "F8.0".into(),
                "F10.3".into(),
                "PCT6.1".into(),
                "PCT6.1".into(),
                String::new(),
            ];
            rows.len()
        ];
        Table {
            title: self.title("Percentiles", label),
            row_headers: 1,
            column_groups: vec![Vec::new(); columns.len()],
            columns,
            rows,
            formats,
            caption: Some(format!("Dependent variable: {}.", self.dependent)),
            ..Table::default()
        }
    }

    /// The profit and ROI charts over percentile groups: the average profit, and the return on
    /// investment in percent, of the first p percent of the cases at each percentile.
    pub(super) fn charts(&self, charts: &mut Vec<crate::Chart>) {
        use crate::chart::{Axis, Category, PERCENT_FORMAT, Series, Statistic};
        let command = self.command;
        let wanted = [(command.plot_profit, "Profit"), (command.plot_roi, "ROI")];
        if !wanted.iter().any(|w| w.0) {
            return;
        }
        let q = command.plot_increment;
        let categories: Vec<Category> = (q..=100)
            .step_by(q)
            .map(|p| Category {
                label: format!("{p}%"),
                value: Cell::Number(p as f64),
                missing: false,
                bin: None,
            })
            .collect();
        for (label, ns) in self.samples {
            let gain = self.gain(ns);
            let order = self
                .gain(self.training)
                .order(self.terminals, command.gain_ascending);
            let rows = gain.percentiles(&order, q);
            for (asked, name) in wanted {
                if !asked {
                    continue;
                }
                let (values, statistic) = if name == "Profit" {
                    (rows.iter().map(|r| r.score).collect(), Statistic::Value)
                } else {
                    (
                        rows.iter()
                            .map(|r| roi(r.score, r.expense).unwrap_or(f64::NAN))
                            .collect(),
                        Statistic::Percent,
                    )
                };
                let series = vec![Series {
                    name: name.to_string(),
                    statistic,
                    variable: Some(self.dependent.to_string()),
                    value: Cell::Empty,
                    values,
                }];
                let axis = if name == "ROI" {
                    Axis {
                        format: PERCENT_FORMAT.into(),
                        ..Axis::titled("ROI (%)")
                    }
                } else {
                    Axis::titled("Average profit")
                };
                let mut chart = crate::Chart::lines(
                    if self.validated {
                        format!("{name} chart ({label} Sample)")
                    } else {
                        format!("{name} chart")
                    },
                    Axis::titled("Percentile"),
                    axis,
                    categories.clone(),
                    series,
                );
                chart.footnote = Some(format!(
                    "Dependent variable: {}. Cumulative over percentile groups of {q}%",
                    self.dependent
                ));
                charts.push(chart);
            }
        }
    }
}
