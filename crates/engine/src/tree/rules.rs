//! RULES: the tree written as rules that select or score cases, in the command language (INTERNAL),
//! SQL, or plain "if ... then" sentences (GENERIC).
//!
//! Rules from the Command Syntax Reference (TREE, RULES), in our words; the forms below are our
//! own.
//! - A rule holds for the cases of a node: the conditions of the splits from the root down, each
//!   the condition that sends a case to that child as the tree itself does. For CRT and QUEST a
//!   case whose split value is missing, or a category the tree has not seen, follows the first
//!   surrogate whose predictor it has a value for and else the side that held more cases when the
//!   node was grown; with SURROGATES=EXCLUDE such a case belongs to neither child. For CHAID a
//!   missing value is a category of its own, and a category no group of the split holds goes to
//!   the group the tree sent such cases to.
//! - Scale predictors are cut at exact numbers (x <= v, x > v); the others are listed by their
//!   values. An unknown value is a missing one, or one the tree has no category for.
//! - SCORING gives each node of the scope a rule that sets NodeID, PredictedValue and, for a
//!   categorical dependent, PredictedProbability; parent nodes come before their children, so a
//!   case ends with its terminal node's values. SELECTION gives one rule that selects the cases of
//!   the nodes of the scope. GENERIC rules say the same in sentences, with variable and value
//!   labels when LABELS=YES, and leave out surrogates.

use super::{Key, Node, Predictor, Tree, TreeRuleSyntax, TreeRules};
use crate::{Cell, Table};

/// What the rules need to know of a predictor besides the codes of its cases.
#[derive(Clone, Default)]
pub(super) struct RuleInfo {
    /// The name of the variable, as the rules write it.
    pub name: String,
    pub label: Option<String>,
    /// The value of each category (a scale predictor's distinct values under CRT and QUEST).
    pub keys: Vec<Key>,
    pub value_labels: Vec<Option<String>>,
    /// The break points of a scale predictor of CHAID, in order.
    pub cuts: Vec<f64>,
}

/// A condition on the values of the predictors.
#[derive(Clone)]
enum Cond {
    True,
    False,
    /// The predictor has a value the tree knows: not missing, and one of its categories.
    Known(usize),
    /// The predictor is missing or has no category.
    Unknown(usize),
    /// The value is one of these categories.
    In(usize, Vec<Key>),
    Le(usize, f64),
    Gt(usize, f64),
    And(Vec<Cond>),
    Or(Vec<Cond>),
}

fn and(parts: Vec<Cond>) -> Cond {
    let mut kept = Vec::new();
    for part in parts {
        match part {
            Cond::True => {}
            Cond::False => return Cond::False,
            other => kept.push(other),
        }
    }
    match kept.len() {
        0 => Cond::True,
        1 => kept.remove(0),
        _ => Cond::And(kept),
    }
}

fn or(parts: Vec<Cond>) -> Cond {
    let mut kept = Vec::new();
    for part in parts {
        match part {
            Cond::False => {}
            Cond::True => return Cond::True,
            other => kept.push(other),
        }
    }
    match kept.len() {
        0 => Cond::False,
        1 => kept.remove(0),
        _ => Cond::Or(kept),
    }
}

/// The predictors, the weights of the cases and the tree: what a condition is built from.
pub(super) struct Rules<'a> {
    pub command: &'a Tree,
    pub rules: &'a TreeRules,
    pub nodes: &'a [Node],
    pub predictors: &'a [Predictor],
    /// The training weight of each case, for the side that held more cases.
    pub weights: &'a [f64],
    pub classes: &'a [Key],
    pub class_labels: &'a [String],
    pub dependent: &'a str,
    /// The dependent's variable label, and its value labels per class.
    pub dependent_label: Option<&'a str>,
}

/// A step of the path to a node, as the generic rules say it.
enum Step {
    Cut(usize, bool, f64),
    Range(usize, f64, f64),
    Values(usize, Vec<usize>),
    Missing(usize),
    /// The step, or a missing value.
    OrMissing(Box<Step>),
}

impl Rules<'_> {
    fn predictor(&self, p: usize) -> &Predictor {
        &self.predictors[p]
    }

    /// The categories of predictor `p` that `codes` stand for, without the missing one.
    fn keys_of(&self, p: usize, codes: &[usize]) -> Vec<Key> {
        let pr = self.predictor(p);
        codes
            .iter()
            .filter(|&&c| c != pr.missing())
            .map(|&c| pr.rule.keys[c].clone())
            .collect()
    }

    /// The condition that a known value of predictor `p` has a code in `codes` (a split group).
    fn group(&self, p: usize, codes: &[usize]) -> Cond {
        let pr = self.predictor(p);
        let present: Vec<usize> = codes
            .iter()
            .copied()
            .filter(|&c| c != pr.missing())
            .collect();
        if present.is_empty() {
            return Cond::False;
        }
        if pr.breaks.is_some() && !pr.numbers.is_empty() {
            // a cut of a scale predictor of CRT or QUEST: codes below or above a value
            let (lo, hi) = (present[0], present[present.len() - 1]);
            if present.len() == hi - lo + 1 {
                return if lo == 0 {
                    Cond::Le(p, pr.numbers[hi])
                } else if hi + 1 == pr.missing() {
                    Cond::Gt(p, pr.numbers[lo - 1])
                } else {
                    and(vec![
                        Cond::Gt(p, pr.numbers[lo - 1]),
                        Cond::Le(p, pr.numbers[hi]),
                    ])
                };
            }
        }
        if pr.breaks.is_some() {
            // the bins of a scale predictor of CHAID, runs of neighbouring bins as ranges
            let mut runs: Vec<(usize, usize)> = Vec::new();
            for &c in &present {
                match runs.last_mut() {
                    Some(run) if run.1 + 1 == c => run.1 = c,
                    _ => runs.push((c, c)),
                }
            }
            return or(runs.iter().map(|&(lo, hi)| self.bins(p, lo, hi)).collect());
        }
        Cond::In(p, self.keys_of(p, &present))
    }

    /// The condition that a value falls in bins `lo` to `hi` of a scale predictor of CHAID.
    fn bins(&self, p: usize, lo: usize, hi: usize) -> Cond {
        let cuts = &self.predictor(p).rule.cuts;
        let upper = (hi < cuts.len()).then(|| Cond::Le(p, cuts[hi]));
        let lower = (lo > 0).then(|| Cond::Gt(p, cuts[lo - 1]));
        and(vec![
            lower.unwrap_or(Cond::True),
            upper.unwrap_or(Cond::True),
        ])
    }

    /// The condition that a case belongs to child `at` (0 left, 1 right) of a binary split of the
    /// node by predictor `p` with `left` codes going left.
    fn side(&self, p: usize, left: &[usize], at: usize) -> Cond {
        let pr = self.predictor(p);
        let codes: Vec<usize> = if at == 0 {
            left.to_vec()
        } else {
            (0..pr.missing()).filter(|c| !left.contains(c)).collect()
        };
        self.group(p, &codes)
    }

    /// The condition that a case belongs to child number `at` of node `id`.
    fn child(&self, id: usize, at: usize) -> Cond {
        let node = &self.nodes[id];
        let split = node.split.as_ref().expect("a split");
        let p = split.predictor;
        let child = &self.nodes[node.children[at]];
        let (_, codes) = child.definition.as_ref().expect("a definition");
        let pr = self.predictor(p);
        if split.improvement.is_none() && split.quest.is_none() {
            return self.chaid_child(id, at, p, codes);
        }
        // CRT and QUEST: the primary split, then the surrogates, then the larger side
        let applies = and(vec![Cond::Known(p), self.group(p, codes)]);
        if !self.rules.surrogates {
            return applies;
        }
        let left: Vec<usize> = self.nodes[node.children[0]]
            .definition
            .as_ref()
            .expect("a definition")
            .1
            .clone();
        let mut parts = vec![applies];
        let mut unknown = vec![Cond::Unknown(p)];
        for s in &split.surrogates {
            let mut conditions = unknown.clone();
            conditions.push(Cond::Known(s.predictor));
            conditions.push(self.side(s.predictor, &s.left, at));
            parts.push(and(conditions));
            unknown.push(Cond::Unknown(s.predictor));
        }
        // the side that held more of the cases with a value, the left on a tie
        let (mut weight_left, mut weight_right) = (0.0, 0.0);
        for &i in &node.cases {
            let code = pr.codes[i];
            if code == pr.missing() {
                continue;
            }
            if left.contains(&code) {
                weight_left += self.weights[i];
            } else {
                weight_right += self.weights[i];
            }
        }
        let larger = usize::from(weight_left < weight_right);
        if larger == at {
            parts.push(and(unknown));
        }
        or(parts)
    }

    /// The condition of child `at` of a CHAID split: the categories its group holds, and those
    /// the tree sends there for want of a group.
    fn chaid_child(&self, id: usize, at: usize, p: usize, group: &[usize]) -> Cond {
        let node = &self.nodes[id];
        let pr = self.predictor(p);
        let missing = pr.missing();
        // where each code goes: observed in the node's cases, else as the tree sends such cases
        let mut goes: Vec<Option<usize>> = vec![None; missing + 1];
        for (k, &c) in node.children.iter().enumerate() {
            for &i in &self.nodes[c].cases {
                goes[pr.codes[i]].get_or_insert(k);
            }
        }
        let groups: Vec<&Vec<usize>> = node
            .children
            .iter()
            .map(|&c| &self.nodes[c].definition.as_ref().expect("a definition").1)
            .collect();
        for (code, target) in goes.iter_mut().enumerate() {
            if target.is_some() {
                continue;
            }
            *target = groups.iter().position(|g| g.contains(&code)).or_else(|| {
                if !pr.nominal && code != missing {
                    // the group with the nearest category
                    (0..groups.len()).min_by_key(|&k| {
                        groups[k]
                            .iter()
                            .filter(|&&c| c != missing)
                            .map(|&c| c.abs_diff(code))
                            .min()
                            .unwrap_or(usize::MAX)
                    })
                } else {
                    // the child with the most cases
                    (0..groups.len()).reduce(|a, b| {
                        let size = |k: usize| self.nodes[node.children[k]].n;
                        if size(b) > size(a) { b } else { a }
                    })
                }
            });
        }
        let mine: Vec<usize> = (0..=missing).filter(|&c| goes[c] == Some(at)).collect();
        debug_assert!(group.iter().all(|c| mine.contains(c)));
        let mut parts = vec![self.group(p, &mine)];
        if mine.contains(&missing) {
            parts.push(Cond::Unknown(p));
        }
        or(parts)
    }

    /// The condition of node `id`: the conditions of the splits from the root.
    fn path(&self, id: usize) -> Cond {
        let mut parts = Vec::new();
        let mut at = id;
        while let Some(parent) = self.nodes[at].parent {
            let k = self.nodes[parent]
                .children
                .iter()
                .position(|&c| c == at)
                .expect("a child");
            parts.push(self.child(parent, k));
            at = parent;
        }
        parts.reverse();
        and(parts)
    }

    /// The steps from the root to node `id` as the generic rules say them.
    fn steps(&self, id: usize) -> Vec<Step> {
        let mut steps = Vec::new();
        let mut at = id;
        while let Some(parent) = self.nodes[at].parent {
            let (p, codes) = self.nodes[at].definition.clone().expect("a definition");
            let pr = self.predictor(p);
            let present: Vec<usize> = codes
                .iter()
                .copied()
                .filter(|&c| c != pr.missing())
                .collect();
            let with_missing = codes.contains(&pr.missing());
            // the step for the categories the group holds, None when that is every value
            let base: Option<Step> = if present.is_empty() {
                None
            } else if pr.breaks.is_some() && !pr.numbers.is_empty() {
                let (lo, hi) = (present[0], present[present.len() - 1]);
                Some(if lo == 0 {
                    Step::Cut(p, true, pr.numbers[hi])
                } else {
                    Step::Cut(p, false, pr.numbers[lo - 1])
                })
            } else if pr.breaks.is_some() {
                let (lo, hi) = (present[0], present[present.len() - 1]);
                let cuts = &pr.rule.cuts;
                if lo == 0 && hi < cuts.len() {
                    Some(Step::Cut(p, true, cuts[hi]))
                } else if lo > 0 && hi >= cuts.len() {
                    Some(Step::Cut(p, false, cuts[lo - 1]))
                } else if lo > 0 {
                    Some(Step::Range(p, cuts[lo - 1], cuts[hi]))
                } else {
                    None
                }
            } else {
                Some(Step::Values(p, present))
            };
            match (base, with_missing) {
                (Some(step), false) => steps.push(step),
                (Some(step), true) => steps.push(Step::OrMissing(Box::new(step))),
                (None, true) if codes.len() == 1 => steps.push(Step::Missing(p)),
                // every value, missing ones too: no condition
                (None, _) => {}
            }
            at = parent;
        }
        steps.reverse();
        steps
    }

    // ---- rendering --------------------------------------------------------

    fn name(&self, p: usize) -> &str {
        &self.predictor(p).rule.name
    }

    fn value(&self, key: &Key) -> String {
        match key {
            Key::Number(x) => format!("{x}"),
            Key::Text(t) => format!("'{}'", t.replace('\'', "''")),
        }
    }

    fn render(&self, cond: &Cond, syntax: TreeRuleSyntax) -> String {
        let sql = syntax == TreeRuleSyntax::Sql;
        match cond {
            Cond::True => "(1 = 1)".to_string(),
            Cond::False => "(1 = 0)".to_string(),
            Cond::Known(p) => {
                let pr = self.predictor(*p);
                let name = self.name(*p);
                if pr.breaks.is_some() {
                    return if sql {
                        format!("{name} IS NOT NULL")
                    } else {
                        format!("NOT MISSING({name})")
                    };
                }
                let list = self.list(&pr.rule.keys);
                let numeric = pr.rule.keys.iter().all(|k| matches!(k, Key::Number(_)));
                if sql {
                    format!("{name} IN ({list})")
                } else if numeric {
                    format!("(NOT SYSMIS({name}) AND ANY({name}, {list}))")
                } else {
                    format!("ANY({name}, {list})")
                }
            }
            Cond::Unknown(p) => {
                let pr = self.predictor(*p);
                let name = self.name(*p);
                if pr.breaks.is_some() {
                    return if sql {
                        format!("{name} IS NULL")
                    } else {
                        format!("MISSING({name})")
                    };
                }
                let list = self.list(&pr.rule.keys);
                let numeric = pr.rule.keys.iter().all(|k| matches!(k, Key::Number(_)));
                if sql {
                    format!("({name} IS NULL OR {name} NOT IN ({list}))")
                } else if numeric {
                    format!("(SYSMIS({name}) OR NOT ANY({name}, {list}))")
                } else {
                    format!("NOT ANY({name}, {list})")
                }
            }
            Cond::In(p, keys) => {
                let list = self.list(keys);
                let name = self.name(*p);
                if sql {
                    format!("{name} IN ({list})")
                } else {
                    format!("ANY({name}, {list})")
                }
            }
            Cond::Le(p, v) => format!("{} <= {v}", self.name(*p)),
            Cond::Gt(p, v) => format!("{} > {v}", self.name(*p)),
            Cond::And(parts) => format!(
                "({})",
                parts
                    .iter()
                    .map(|c| self.render(c, syntax))
                    .collect::<Vec<_>>()
                    .join(" AND ")
            ),
            Cond::Or(parts) => format!(
                "({})",
                parts
                    .iter()
                    .map(|c| self.render(c, syntax))
                    .collect::<Vec<_>>()
                    .join(" OR ")
            ),
        }
    }

    fn list(&self, keys: &[Key]) -> String {
        keys.iter()
            .map(|k| self.value(k))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The category a node is assigned and its probability, or for a scale dependent its mean.
    fn prediction(&self, id: usize) -> (String, Option<String>, String) {
        let node = &self.nodes[id];
        if node.scale {
            let mean = node.counts[1] / node.counts[0] + node.shift;
            return (format!("{mean}"), None, format!("{mean}"));
        }
        let j = node.predicted();
        let value = self.value(&self.classes[j]);
        (
            value,
            Some(format!("{}", node.probability(j))),
            self.class_labels[j].clone(),
        )
    }

    /// The nodes the rules are written for: for the top nodes by index, the terminal nodes of the
    /// first target category with the largest index values, in that order.
    fn scope(&self) -> Vec<usize> {
        use statscraft_syntax::TreeRuleNodes as N;
        let terminals: Vec<usize> = (0..self.nodes.len())
            .filter(|&t| self.nodes[t].children.is_empty())
            .collect();
        let target = self
            .command
            .targets
            .iter()
            .find_map(|t| self.classes.iter().position(|k| *k == super::key_of(t)));
        match (self.rules.nodes, target) {
            (N::None, _) => Vec::new(),
            (N::All, _) => (0..self.nodes.len()).collect(),
            (N::TopN(_) | N::TopPercent(_) | N::MinIndex(_), Some(j)) if !self.nodes[0].scale => {
                let gain = super::gain::Gain {
                    nodes: self.nodes,
                    target: j,
                    empirical: super::empirical_priors(self.command),
                };
                let order = gain.order(&terminals, self.command.gain_ascending);
                match self.rules.nodes {
                    N::TopN(n) => order.into_iter().take(n).collect(),
                    N::TopPercent(percent) => {
                        // the nodes that start before the percent of the cases is reached
                        let limit = self.nodes[0].n * percent / 100.0;
                        let mut taken = 0.0;
                        let mut kept = Vec::new();
                        for t in order {
                            if taken >= limit * (1.0 - 1e-12) {
                                break;
                            }
                            taken += self.nodes[t].n;
                            kept.push(t);
                        }
                        kept
                    }
                    N::MinIndex(least) => {
                        let base = gain.base();
                        order
                            .into_iter()
                            .filter(|&t| {
                                let f = gain.figures(t);
                                f.w > 0.0
                                    && base > 0.0
                                    && 100.0 * (f.a / f.w) / base >= least * (1.0 - 1e-12)
                            })
                            .collect()
                    }
                    _ => unreachable!("handled above"),
                }
            }
            _ => terminals,
        }
    }

    // ---- the tables -------------------------------------------------------

    pub(super) fn table(&self) -> Option<Table> {
        let scope = self.scope();
        if scope.is_empty() {
            return None;
        }
        let lines = match self.rules.syntax {
            TreeRuleSyntax::Generic => self.generic(&scope),
            TreeRuleSyntax::Internal => self.internal(&scope),
            TreeRuleSyntax::Sql => self.sql(&scope),
        };
        let rows: Vec<Vec<Cell>> = lines.into_iter().map(|l| vec![Cell::Text(l)]).collect();
        Some(Table {
            title: "Rules".into(),
            row_headers: 0,
            columns: vec!["Rule".into()],
            column_groups: vec![Vec::new()],
            formats: vec![vec![String::new()]; rows.len()],
            rows,
            caption: Some(format!(
                "Growing Method: {}. Dependent Variable: {}.",
                super::method_name(self.command),
                self.dependent
            )),
            ..Table::default()
        })
    }

    fn internal(&self, scope: &[usize]) -> Vec<String> {
        let syntax = TreeRuleSyntax::Internal;
        let mut lines = Vec::new();
        if self.rules.selection {
            let cond = or(scope.iter().map(|&t| self.path(t)).collect());
            lines.push(format!(
                "COMPUTE filter_$ = {}.",
                self.render(&cond, syntax)
            ));
            lines.push("FILTER BY filter_$.".into());
            lines.push("EXECUTE.".into());
            return lines;
        }
        let text_dependent =
            !self.nodes[0].scale && self.classes.iter().any(|k| matches!(k, Key::Text(_)));
        if text_dependent {
            let width = self
                .classes
                .iter()
                .map(|k| match k {
                    Key::Text(t) => t.chars().count(),
                    Key::Number(_) => 0,
                })
                .max()
                .unwrap_or(1)
                .max(1);
            lines.push(format!("STRING PredictedValue (A{width})."));
        }
        for &t in scope {
            let (value, probability, _) = self.prediction(t);
            lines.push(format!("/* Node {t} */."));
            lines.push(format!("DO IF {}.", self.render(&self.path(t), syntax)));
            lines.push(format!("COMPUTE NodeID = {t}."));
            lines.push(format!("COMPUTE PredictedValue = {value}."));
            if let Some(p) = probability {
                lines.push(format!("COMPUTE PredictedProbability = {p}."));
            }
            lines.push("END IF.".into());
        }
        lines.push("EXECUTE.".into());
        lines
    }

    fn sql(&self, scope: &[usize]) -> Vec<String> {
        let syntax = TreeRuleSyntax::Sql;
        if self.rules.selection {
            let cond = or(scope.iter().map(|&t| self.path(t)).collect());
            return vec![format!("WHERE {}", self.render(&cond, syntax))];
        }
        let mut lines = Vec::new();
        for &t in scope {
            let (value, probability, _) = self.prediction(t);
            let cond = self.render(&self.path(t), syntax);
            lines.push(format!("/* Node {t} */"));
            let mut columns = vec![
                format!("CASE WHEN {cond} THEN {t} END AS NodeID"),
                format!("CASE WHEN {cond} THEN {value} END AS PredictedValue"),
            ];
            if let Some(p) = probability {
                columns.push(format!(
                    "CASE WHEN {cond} THEN {p} END AS PredictedProbability"
                ));
            }
            lines.push(format!("SELECT {};", columns.join(", ")));
        }
        lines
    }

    /// The name of a variable as a sentence says it.
    fn variable_text(&self, p: usize, labels: bool) -> String {
        let info = &self.predictor(p).rule;
        match (&info.label, labels) {
            (Some(l), true) => l.clone(),
            _ => info.name.clone(),
        }
    }

    /// The category `c` of predictor `p` as a sentence says it.
    fn category_text(&self, p: usize, c: usize, labels: bool) -> String {
        let info = &self.predictor(p).rule;
        match (&info.value_labels[c], labels) {
            (Some(l), true) => l.clone(),
            _ => match &info.keys[c] {
                Key::Number(x) => format!("{x}"),
                Key::Text(t) => t.clone(),
            },
        }
    }

    /// A step of the path as a phrase of a generic rule.
    fn sentence(&self, step: &Step, labels: bool) -> String {
        match step {
            Step::Cut(p, true, v) => format!("{} <= {v}", self.variable_text(*p, labels)),
            Step::Cut(p, false, v) => format!("{} > {v}", self.variable_text(*p, labels)),
            Step::Range(p, lo, hi) => {
                format!("{} > {lo} and <= {hi}", self.variable_text(*p, labels))
            }
            Step::Values(p, codes) => format!(
                "{} is {}",
                self.variable_text(*p, labels),
                codes
                    .iter()
                    .map(|&c| self.category_text(*p, c, labels))
                    .collect::<Vec<_>>()
                    .join(" or ")
            ),
            Step::Missing(p) => format!("{} is missing", self.variable_text(*p, labels)),
            Step::OrMissing(inner) => {
                let p = match &**inner {
                    Step::Cut(p, ..)
                    | Step::Range(p, ..)
                    | Step::Values(p, _)
                    | Step::Missing(p) => *p,
                    Step::OrMissing(_) => unreachable!("a step is wrapped once"),
                };
                format!(
                    "({} or {} is missing)",
                    self.sentence(inner, labels),
                    self.variable_text(p, labels)
                )
            }
        }
    }

    fn generic(&self, scope: &[usize]) -> Vec<String> {
        let labels = self.rules.labels;
        let mut lines = Vec::new();
        let selection = self.rules.selection;
        let mut selected = Vec::new();
        for &t in scope {
            let steps: Vec<String> = self
                .steps(t)
                .iter()
                .map(|step| self.sentence(step, labels))
                .collect();
            let when = if steps.is_empty() {
                "always".to_string()
            } else {
                steps.join(" and ")
            };
            if selection {
                selected.push(format!("({when})"));
                continue;
            }
            let (_, probability, shown) = self.prediction(t);
            let then = match probability {
                Some(p) => format!(
                    "the predicted {} is {shown} with probability {p}",
                    self.dependent_label.unwrap_or(self.dependent)
                ),
                None => format!(
                    "the predicted mean of {} is {shown}",
                    self.dependent_label.unwrap_or(self.dependent)
                ),
            };
            lines.push(format!("Node {t}: if {when} then {then}"));
        }
        if selection {
            lines.push(format!(
                "Select the cases for which {}",
                selected.join(" or ")
            ));
        }
        lines
    }
}
