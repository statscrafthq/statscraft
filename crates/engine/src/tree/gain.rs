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

/// The figures of one terminal node: count, target count, weight and numerator of the score.
#[derive(Clone, Copy)]
pub(super) struct Figures {
    pub n: f64,
    pub g: f64,
    pub w: f64,
    pub a: f64,
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
            return Figures { n, g, w: n, a: g };
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
        let (gain, w, a) = (
            before(|f| f.g) + part * f.g,
            before(|f| f.w) + part * f.w,
            before(|f| f.a) + part * f.a,
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
        };
        let mut rows = Vec::new();
        for &t in &order {
            let f = gain.figures(t);
            sum = Figures {
                n: sum.n + f.n,
                g: sum.g + f.g,
                w: sum.w + f.w,
                a: sum.a + f.a,
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
