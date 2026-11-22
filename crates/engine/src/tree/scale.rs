//! The tables of a CRT tree for a scale dependent: each node holds the weight, the mean and the
//! spread of the dependent instead of class counts, and the risk is the mean squared deviation
//! from the node means.

use super::{Node, Predictor, Tree, depth_limit, method_name, percent, risk_estimate, text};
use crate::{Cell, Engine, Table};
use statscraft_data::Variable;

/// The mean and, with more than one case in the node, the sample standard deviation.
fn spread(node: &Node) -> (f64, Option<f64>) {
    let mean = node.counts[1] / node.counts[0] + node.shift;
    let squares = (node.counts[2] - node.counts[1] * node.counts[1] / node.counts[0]).max(0.0);
    let sd = (node.n > 1.0).then(|| (squares / (node.n - 1.0)).sqrt());
    (mean, sd)
}

impl Engine {
    pub(super) fn scale_output(
        &self,
        command: &Tree,
        nodes: &[Node],
        predictors: &[Predictor],
        dependent: &Variable,
        charts: &mut Vec<crate::Chart>,
    ) -> Vec<Table> {
        let shown = self.shown();
        let root_n = nodes[0].n;
        let terminals: Vec<usize> = (0..nodes.len())
            .filter(|&t| nodes[t].children.is_empty())
            .collect();
        let mut out = Vec::new();
        let number_or_empty = |x: Option<f64>| x.map_or(Cell::Empty, Cell::Number);

        if command.print.model_summary {
            let mut used: Vec<usize> = nodes
                .iter()
                .filter_map(|n| n.split.as_ref().map(|s| s.predictor))
                .collect();
            used.sort_unstable();
            used.dedup();
            let names = |list: Vec<&Predictor>| {
                list.iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let row = |group: &str, name: &str, value: Cell| vec![text(group), text(name), value];
            let depth = nodes.iter().map(|n| n.depth).max().unwrap_or(0);
            let rows = vec![
                row(
                    "Specifications",
                    "Growing Method",
                    text(method_name(command)),
                ),
                row("", "Dependent Variable", text(shown.variable(dependent))),
                row(
                    "",
                    "Independent Variables",
                    text(names(predictors.iter().collect())),
                ),
                row("", "Impurity Measure", text("Least squares deviation")),
                row(
                    "",
                    "Minimum Change in Improvement",
                    Cell::Number(command.min_improvement.unwrap_or(0.0001)),
                ),
                row(
                    "",
                    "Pruning",
                    text(match command.prune {
                        None => "None".to_string(),
                        Some(b) => format!("Standard error rule, {b} standard errors"),
                    }),
                ),
                row("", "Validation", text("None")),
                row(
                    "",
                    "Maximum Tree Depth",
                    Cell::Number(depth_limit(command) as f64),
                ),
                row(
                    "",
                    "Minimum Cases in Parent Node",
                    Cell::Number(command.min_parent),
                ),
                row(
                    "",
                    "Minimum Cases in Child Node",
                    Cell::Number(command.min_child),
                ),
                row(
                    "Results",
                    "Independent Variables Included",
                    text(names(used.iter().map(|&p| &predictors[p]).collect())),
                ),
                row("", "Number of Nodes", Cell::Number(nodes.len() as f64)),
                row(
                    "",
                    "Number of Terminal Nodes",
                    Cell::Number(terminals.len() as f64),
                ),
                row("", "Depth", Cell::Number(depth as f64)),
            ];
            out.push(Table {
                title: "Model Summary".into(),
                row_headers: 2,
                columns: vec![String::new(), String::new(), String::new()],
                column_groups: vec![Vec::new(); 3],
                rows,
                ..Table::default()
            });
        }

        let definition = |node: &Node| match &node.definition {
            Some((p, group)) => format!(
                "{} = {}",
                predictors[*p].name,
                predictors[*p].describe(group)
            ),
            None => String::new(),
        };
        if command.display {
            let mut diagram = Vec::new();
            for (id, node) in nodes.iter().enumerate() {
                let (mean, sd) = spread(node);
                let mut statistics: Vec<(String, Cell)> = vec![
                    ("Mean".into(), Cell::Number(mean)),
                    ("Std. Deviation".into(), number_or_empty(sd)),
                    ("n".into(), Cell::Number(node.n)),
                    ("%".into(), percent(node.n, root_n)),
                    ("Predicted".into(), Cell::Number(mean)),
                ];
                if let Some(split) = &node.split {
                    statistics.push(("Split by".into(), text(&predictors[split.predictor].name)));
                    statistics.push(("Improvement".into(), number_or_empty(split.improvement)));
                }
                diagram.push(crate::plot::TreeNode {
                    id,
                    parent: node.parent,
                    split: definition(node),
                    statistics,
                });
            }
            let mut chart = crate::Chart::plot(
                crate::chart::ChartKind::TreeDiagram,
                format!("Tree of {}", shown.variable(dependent)),
                crate::chart::Axis::titled(""),
                crate::chart::Axis::titled(""),
                crate::plot::Plot::Tree { nodes: diagram },
            );
            chart.footnote = Some(format!("Dependent variable: {}", shown.variable(dependent)));
            charts.push(chart);
        }

        if command.print.tree_table {
            let mut columns = vec!["Node".to_string(), "Parent Node".to_string()];
            if command.node_definitions {
                columns.push("Definition".into());
            }
            columns.extend(
                ["N", "Percent", "Mean", "Std. Deviation", "Predicted Mean"].map(String::from),
            );
            if command.branch_statistics {
                columns.extend(["Variable", "Improvement"].map(String::from));
            }
            let rows: Vec<Vec<Cell>> = nodes
                .iter()
                .enumerate()
                .map(|(id, node)| {
                    let (mean, sd) = spread(node);
                    let mut row = vec![
                        text(format!("{id}")),
                        node.parent.map_or(Cell::Empty, |p| Cell::Number(p as f64)),
                    ];
                    if command.node_definitions {
                        row.push(text(definition(node)));
                    }
                    row.extend([
                        Cell::Number(node.n),
                        percent(node.n, root_n),
                        Cell::Number(mean),
                        number_or_empty(sd),
                        Cell::Number(mean),
                    ]);
                    if command.branch_statistics {
                        match &node.split {
                            Some(split) => row.extend([
                                text(&predictors[split.predictor].name),
                                number_or_empty(split.improvement),
                            ]),
                            None => row.extend([Cell::Empty, Cell::Empty]),
                        }
                    }
                    row
                })
                .collect();
            let formats: Vec<Vec<String>> = rows
                .iter()
                .map(|_| {
                    columns
                        .iter()
                        .map(|c| match c.as_str() {
                            "N" => "F8.0".to_string(),
                            "Percent" => "F5.1".to_string(),
                            "Mean" | "Std. Deviation" | "Predicted Mean" => "F8.3".to_string(),
                            "Improvement" => "F8.5".to_string(),
                            _ => String::new(),
                        })
                        .collect()
                })
                .collect();
            out.push(Table {
                title: "Tree Table".into(),
                row_headers: 1,
                column_groups: vec![Vec::new(); columns.len()],
                columns,
                rows,
                formats,
                caption: Some(format!(
                    "Dependent variable: {}.",
                    shown.variable(dependent)
                )),
                ..Table::default()
            });
        }
        self.crt_tables(command, nodes, predictors, dependent, (&mut out, charts));

        let (risk, variance) = risk_estimate(terminals.iter().map(|&t| &nodes[t]), &nodes[0]);
        if command.print.risk {
            out.push(Table {
                title: "Risk".into(),
                row_headers: 0,
                columns: vec!["Estimate".into(), "Std. Error".into()],
                column_groups: vec![Vec::new(); 2],
                rows: vec![vec![Cell::Number(risk), Cell::Number(variance.sqrt())]],
                formats: vec![vec!["F8.3".into(), "F8.3".into()]],
                caption: Some(format!(
                    "Growing Method: {}. Dependent Variable: {}.",
                    method_name(command),
                    shown.variable(dependent)
                )),
                ..Table::default()
            });
        }

        if command.gain_table {
            // the terminal nodes by their mean, with the mean over the overall mean as the index
            let overall = spread(&nodes[0]).0;
            let mut order = terminals.clone();
            order.sort_by(|&a, &b| {
                let (ma, mb) = (spread(&nodes[a]).0, spread(&nodes[b]).0);
                if command.gain_ascending {
                    ma.total_cmp(&mb)
                } else {
                    mb.total_cmp(&ma)
                }
            });
            let rows: Vec<Vec<Cell>> = order
                .iter()
                .map(|&t| {
                    let mean = spread(&nodes[t]).0;
                    vec![
                        text(format!("{t}")),
                        Cell::Number(nodes[t].n),
                        percent(nodes[t].n, root_n),
                        Cell::Number(mean),
                        if overall == 0.0 {
                            Cell::Empty
                        } else {
                            percent(mean, overall)
                        },
                    ]
                })
                .collect();
            let formats = vec![
                vec![
                    String::new(),
                    "F8.0".into(),
                    "F5.1".into(),
                    "F8.3".into(),
                    "F6.1".into()
                ];
                rows.len()
            ];
            out.push(Table {
                title: "Gain Summary for Nodes".into(),
                row_headers: 1,
                columns: ["Node", "N", "Percent", "Mean", "Index"]
                    .map(String::from)
                    .to_vec(),
                column_groups: vec![Vec::new(); 5],
                rows,
                formats,
                caption: Some(format!("Growing Method: {}.", method_name(command))),
                ..Table::default()
            });
        }
        out
    }
}
