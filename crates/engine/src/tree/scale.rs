//! The tables of a CRT tree for a scale dependent: each node holds the weight, the mean and the
//! spread of the dependent instead of class counts, and the risk is the mean squared deviation
//! from the node means.

use super::{
    Checks, Node, Predictor, Tree, TreeValidation, depth_limit, method_name, percent,
    risk_estimate, text,
};
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
        trees: (&[Node], Checks),
        predictors: &[Predictor],
        dependent: &Variable,
        charts: &mut Vec<crate::Chart>,
    ) -> Vec<Table> {
        let (nodes, checks) = trees;
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
            let binary = command.binary();
            let mut rows = vec![
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
            ];
            if binary {
                rows.extend([
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
                ]);
            }
            rows.extend([
                row(
                    "",
                    "Validation",
                    text(match &command.validation {
                        TreeValidation::None => "None".to_string(),
                        TreeValidation::SplitPercent(p) => {
                            format!("Split sample, {p}% to grow the tree")
                        }
                        TreeValidation::SplitVariable(v) => format!("Split sample by {v}"),
                        TreeValidation::Cross(v) => format!("Cross-validation, {v} folds"),
                    }),
                ),
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
            ]);
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
        let binary = command.binary();
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
                    if binary {
                        statistics.push(("Improvement".into(), number_or_empty(split.improvement)));
                    } else {
                        statistics.push(("Adj. Sig.".into(), Cell::Number(split.sig)));
                        statistics.push(("F".into(), Cell::Number(split.statistic)));
                        statistics.push(("df".into(), Cell::Number(split.df)));
                        statistics.push(("Error df".into(), number_or_empty(split.df2)));
                    }
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
                if binary {
                    columns.extend(["Variable", "Improvement"].map(String::from));
                } else {
                    columns
                        .extend(["Variable", "Adj. Sig.", "F", "df", "Error df"].map(String::from));
                }
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
                            Some(split) if binary => row.extend([
                                text(&predictors[split.predictor].name),
                                number_or_empty(split.improvement),
                            ]),
                            Some(split) => row.extend([
                                text(&predictors[split.predictor].name),
                                Cell::Number(split.sig),
                                Cell::Number(split.statistic),
                                Cell::Number(split.df),
                                number_or_empty(split.df2),
                            ]),
                            None => row.extend(vec![Cell::Empty; if binary { 2 } else { 5 }]),
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
                            "Adj. Sig." => "F5.3".to_string(),
                            "F" => "F8.3".to_string(),
                            "df" | "Error df" => "F8.0".to_string(),
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

        // the samples the risk and gain tables describe
        let test = checks.test;
        let validated = test.is_some();
        let samples: Vec<(&str, &[Node])> = match test {
            None => vec![("", nodes)],
            Some(test) if command.test_sample_only => vec![("Test", test)],
            Some(test) => vec![("Training", nodes), ("Test", test)],
        };
        if command.print.risk {
            let labelled = validated || checks.cross.is_some();
            let mut rows: Vec<Vec<Cell>> = samples
                .iter()
                .map(|(label, ns)| {
                    let (risk, variance) =
                        risk_estimate(terminals.iter().map(|&t| &ns[t]), &ns[0], None);
                    let mut row = Vec::new();
                    if labelled {
                        let name = if checks.cross.is_some() {
                            "Resubstitution"
                        } else {
                            label
                        };
                        row.push(text(name));
                    }
                    row.extend([Cell::Number(risk), Cell::Number(variance.sqrt())]);
                    row
                })
                .collect();
            if let Some((risk, variance)) = checks.cross {
                rows.push(vec![
                    text("Cross-Validation"),
                    Cell::Number(risk),
                    Cell::Number(variance.sqrt()),
                ]);
            }
            let mut columns = vec!["Estimate".to_string(), "Std. Error".to_string()];
            if labelled {
                columns.insert(0, if validated { "Sample" } else { "Method" }.into());
            }
            let width = columns.len();
            out.push(Table {
                title: "Risk".into(),
                row_headers: usize::from(labelled),
                column_groups: vec![Vec::new(); width],
                formats: vec![
                    vec![String::new(); width - 2]
                        .into_iter()
                        .chain(["F8.3".into(), "F8.3".into()])
                        .collect();
                    rows.len()
                ],
                columns,
                rows,
                caption: Some(format!(
                    "Growing Method: {}. Dependent Variable: {}.",
                    method_name(command),
                    shown.variable(dependent)
                )),
                ..Table::default()
            });
        }

        let means = super::gain::MeanContext {
            command,
            samples: &samples,
            training: nodes,
            terminals: &terminals,
            validated,
            dependent: &shown.variable(dependent),
        };
        means.tables(&mut out);
        means.charts(charts);
        out
    }
}
