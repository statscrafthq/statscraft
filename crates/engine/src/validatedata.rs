//! VALIDATEDATA: suspicious variables, incomplete and duplicate case identifiers, empty cases,
//! and the cases that break validation rules kept in the dictionary.
//!
//! From the command's description: missing means user- or system-missing; an ID part or an
//! empty-case value may also be blank. Split variables and rule outcome variables (variable
//! attribute $VD.RuleOutcomeVar = Yes) leave every list; the weight is ignored with a warning.
//! A rule is a data file attribute ($VD.SRule[n] for one variable, linked from the variable by
//! $VD.SRuleRef[n]; $VD.CRule[n] across variables) whose text is a list of Name='value'
//! properties, the last of a name counting and unknown ones ignored. The procedure does not
//! evaluate a rule: its outcome variable, made beforehand, is 1 for a case that breaks it.
//! Broken rules and links are skipped with a warning; two rules with one outcome variable are
//! an error.
//!
//! Choices where the manual is silent:
//! - A blank string counts as a valid value (its own category) in the variable checks, as
//!   missing in the ID and empty-case checks, where the manual says "blank or missing".
//! - PCTMISSING is of all cases; PCTEQUAL and PCTUNEQUAL of the valid values. The standard
//!   deviation divides by n - 1; with fewer than two valid values the scale checks are
//!   skipped.
//! - A rule is valid with a nonempty Label that no earlier rule has (ignoring case and outer
//!   blanks); a single-variable rule also needs Type Numeric, String or Date, and a link needs
//!   a numeric outcome variable and a String rule for a string variable (Numeric or Date for a
//!   number). The outcome variable need not carry $VD.RuleOutcomeVar.
//! - Exactly 1 is a violation; 0, other values and missing are not.
//! - Duplicate ID groups are numbered in the order their first case comes; under SPLIT FILE
//!   the numbers start again in every group. The listings show every case found.
//! - SAVE INCOMPLETEID and DUPLICATEID without ID variables, and RULEVIOLATIONS without
//!   rules, are skipped with a warning.

use crate::pass::default_measure;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Values, Variable};
use statscraft_syntax::{EmptyScope, RuleItem, ValidateSave, Validatedata};
use std::collections::BTreeMap;

/// `Name='value', Name='value' ...` as (lower-case name, value); a value of several quoted
/// pieces (a list) is joined with spaces. None when the text does not have that form.
fn properties(text: &str) -> Option<Vec<(String, String)>> {
    let mut out = Vec::new();
    let mut rest = text;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == ',');
        if rest.is_empty() {
            return Some(out);
        }
        let (name, after) = rest.split_once('=')?;
        rest = after.trim_start();
        let mut pieces = Vec::new();
        while let Some(body) = rest.strip_prefix('\'') {
            let mut value = String::new();
            let mut chars = body.char_indices().peekable();
            let end = loop {
                let (i, c) = chars.next()?;
                if c != '\'' {
                    value.push(c);
                } else if chars.peek().is_some_and(|(_, c)| *c == '\'') {
                    chars.next();
                    value.push('\'');
                } else {
                    break i + 1;
                }
            };
            pieces.push(value);
            rest = body[end..].trim_start();
        }
        if pieces.is_empty() {
            return None;
        }
        out.push((name.trim().to_lowercase(), pieces.join(" ")));
    }
}

/// The last value of a property.
fn get<'a>(properties: &'a [(String, String)], name: &str) -> Option<&'a str> {
    let found = properties.iter().rev().find(|(n, _)| n == name);
    found.map(|(_, v)| v.as_str())
}

/// The values of an array attribute, by its 1-based index; empty slots are left out.
fn array<'a>(attributes: &'a [(String, Vec<String>)], name: &str) -> Vec<(usize, &'a str)> {
    let found = attributes
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name));
    found.map_or(Vec::new(), |(_, values)| {
        let numbered = values.iter().enumerate().map(|(i, v)| (i + 1, v.as_str()));
        numbered.filter(|(_, v)| !v.trim().is_empty()).collect()
    })
}

/// The n of `$VD.SRule[n]`.
fn rule_number(text: &str) -> Option<usize> {
    let text = text.trim();
    let prefix = text.get(..10)?;
    if !prefix.eq_ignore_ascii_case("$VD.SRule[") {
        return None;
    }
    text[10..].strip_suffix(']')?.trim().parse().ok()
}

struct Rule {
    label: String,
    /// Single-variable: the type; cross-variable: the expression.
    detail: String,
    /// Cross-variable only.
    outcome: String,
}

/// The rules of an attribute that are valid, by number; the reasons the others are not.
/// `labels` holds the labels of valid rules so far, which a later rule may not repeat.
fn rules(
    attributes: &[(String, Vec<String>)],
    name: &str,
    single: bool,
    labels: &mut Vec<String>,
) -> BTreeMap<usize, Result<Rule, String>> {
    let mut out = BTreeMap::new();
    for (n, text) in array(attributes, name) {
        let rule = (|| {
            let props = properties(text).ok_or("its text is not a list of Name='value'")?;
            let label = get(&props, "label").unwrap_or_default().trim().to_string();
            if label.is_empty() {
                return Err("it has no Label".to_string());
            }
            if labels.iter().any(|l| l.eq_ignore_ascii_case(&label)) {
                return Err(format!("another rule has the label {label}"));
            }
            let rule = if single {
                let kind = get(&props, "type").unwrap_or_default().trim();
                let known = ["Numeric", "String", "Date"];
                let Some(kind) = known.iter().find(|k| k.eq_ignore_ascii_case(kind)) else {
                    return Err("its Type is not Numeric, String or Date".to_string());
                };
                Rule {
                    label,
                    detail: kind.to_string(),
                    outcome: String::new(),
                }
            } else {
                let expression = get(&props, "expression").unwrap_or_default().trim();
                let outcome = get(&props, "outcomevar").unwrap_or_default().trim();
                if expression.is_empty() || outcome.is_empty() {
                    return Err("it needs an Expression and an OutcomeVar".to_string());
                }
                Rule {
                    label,
                    detail: expression.to_string(),
                    outcome: outcome.to_string(),
                }
            };
            labels.push(rule.label.clone());
            Ok(rule)
        })();
        out.insert(n, rule);
    }
    out
}

/// A single-variable rule applied to an analysis variable.
struct Link {
    variable: String,
    rule: usize,
    outcome: String,
}

fn missing(variable: &Variable, case: usize, blank: bool) -> bool {
    match variable.values() {
        Values::Numeric(v) => v.get(case).is_none_or(|x| variable.is_user_missing(x)),
        Values::String(v) => {
            let text = v.get(case);
            (blank && text.trim().is_empty()) || variable.is_user_missing_str(text)
        }
    }
}

/// The value of an outcome variable is 1.
fn broken(data: &Dataset, outcome: &str, case: usize) -> bool {
    let values = data.variable(outcome).and_then(|v| v.numeric());
    values.is_ok_and(|v| v.get(case) == Some(1.0))
}

fn listed(table: &str, columns: Vec<String>, rows: Vec<Vec<Cell>>, none: &str) -> Table {
    let empty = rows.is_empty();
    Table {
        title: table.into(),
        row_headers: 1,
        columns,
        rows,
        caption: empty.then(|| none.to_string()),
        ..Table::default()
    }
}

impl Engine {
    pub(crate) fn validatedata(&mut self, command: &Validatedata) -> Result<Vec<Table>, Error> {
        let data = self.active()?.clone();
        let is = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        let yes = |values: &Vec<String>| values.first().is_some_and(|v| is(v.trim(), "Yes"));
        let outcomes: Vec<String> = data
            .variables()
            .filter(|v| {
                let attributes = v.attributes();
                let mark = attributes.iter().find(|(n, _)| is(n, "$VD.RuleOutcomeVar"));
                mark.is_some_and(|(_, values)| yes(values))
            })
            .map(|v| v.name().to_string())
            .collect();
        let split: Vec<String> = self
            .split_group
            .iter()
            .flatten()
            .map(|(name, _)| name.clone())
            .collect();
        let left_out = |name: &String| outcomes.iter().chain(&split).any(|o| is(o, name));
        let list = |items| -> Result<Vec<String>, Error> {
            let mut names: Vec<String> = Vec::new();
            for name in self.expand(items)? {
                if !left_out(&name) && !names.iter().any(|n| is(n, &name)) {
                    names.push(name);
                }
            }
            Ok(names)
        };
        let analysis = list(&command.variables)?;
        let ids = list(&command.id)?;
        if data.weight_variable().is_some() {
            self.warnings
                .push("the weight is ignored: every case counts once".into());
        }

        // the rules and links that apply
        let mut labels = Vec::new();
        let single = rules(data.attributes(), "$VD.SRule", true, &mut labels);
        let cross_defined = rules(data.attributes(), "$VD.CRule", false, &mut labels);
        let mut links: Vec<Link> = Vec::new();
        for name in &analysis {
            let variable = data.variable(name)?;
            for (n, text) in array(variable.attributes(), "$VD.SRuleRef") {
                let link = (|| {
                    let props = properties(text).ok_or("its text is not a list of Name='value'")?;
                    let rule_text = get(&props, "rule").unwrap_or_default();
                    let rule = rule_number(rule_text)
                        .ok_or_else(|| format!("its Rule {rule_text} is not $VD.SRule[n]"))?;
                    let kind = match single.get(&rule) {
                        None => return Err(format!("there is no rule $VD.SRule[{rule}]")),
                        Some(Err(why)) => {
                            return Err(format!("the rule $VD.SRule[{rule}] is skipped: {why}"));
                        }
                        Some(Ok(found)) => &found.detail,
                    };
                    if variable.is_string() != (kind == "String") {
                        return Err(format!("a {kind} rule does not fit this variable's type"));
                    }
                    let outcome = get(&props, "outcomevar").unwrap_or_default().trim();
                    match data.variable(outcome) {
                        Ok(v) if !v.is_string() => Ok(Link {
                            variable: name.clone(),
                            rule,
                            outcome: v.name().to_string(),
                        }),
                        _ => Err(format!(
                            "its OutcomeVar {outcome} is not a numeric variable"
                        )),
                    }
                })();
                match link {
                    Ok(link) => links.push(link),
                    Err(why) => self
                        .warnings
                        .push(format!("skips the link $VD.SRuleRef[{n}] of {name}: {why}")),
                }
            }
        }
        let mut wanted: Vec<usize> = Vec::new();
        for item in &command.rules {
            let numbers: Vec<usize> = match item {
                RuleItem::All => cross_defined.keys().copied().collect(),
                RuleItem::Number(n) => vec![*n],
            };
            for n in numbers {
                if !wanted.contains(&n) {
                    wanted.push(n);
                }
            }
        }
        let mut cross: Vec<(usize, &Rule)> = Vec::new();
        for n in wanted {
            match cross_defined.get(&n) {
                None => self.warnings.push(format!(
                    "skips $VD.CRule[{n}]: the data file has no such rule"
                )),
                Some(Err(why)) => self.warnings.push(format!("skips $VD.CRule[{n}]: {why}")),
                Some(Ok(rule)) => match data.variable(&rule.outcome) {
                    Ok(v) if !v.is_string() => cross.push((n, rule)),
                    _ => self.warnings.push(format!(
                        "skips $VD.CRule[{n}]: its OutcomeVar {} is not a numeric \
                         variable",
                        rule.outcome
                    )),
                },
            }
        }
        let mut used: Vec<&str> = Vec::new();
        let outcome_names = links.iter().map(|l| l.outcome.as_str());
        for outcome in outcome_names.chain(cross.iter().map(|(_, r)| r.outcome.as_str())) {
            if used.iter().any(|u| is(u, outcome)) {
                return Err(Error(format!(
                    "the outcome variable {outcome} belongs to more than one rule; \
                     give every rule its own"
                )));
            }
            used.push(outcome);
        }
        let any_rules = !links.is_empty() || !cross.is_empty();
        let summaries = !links.is_empty() && (command.by_variable || command.by_rule);
        if command.summaries_given && (command.by_variable || command.by_rule) && links.is_empty() {
            self.warnings
                .push("/RULESUMMARIES: no analysis variable has a single-variable rule".into());
        }

        // what was asked for that can be given
        let var_checks = command.var_checks.filter(|_| !analysis.is_empty());
        let id_checks = !ids.is_empty() && (command.incomplete_ids || command.duplicate_ids);
        let case_report = any_rules && command.case_report;
        let mut saves = Vec::new();
        for (what, name) in &command.save {
            let skip = match what {
                ValidateSave::IncompleteId if ids.is_empty() => {
                    Some("INCOMPLETEID: there are no ID variables")
                }
                ValidateSave::DuplicateId if ids.is_empty() => {
                    Some("DUPLICATEID: there are no ID variables")
                }
                ValidateSave::RuleViolations if !any_rules => {
                    Some("RULEVIOLATIONS: no validation rule applies")
                }
                _ => None,
            };
            match skip {
                Some(why) => self.warnings.push(format!("/SAVE skips {why}")),
                None => saves.push((*what, name.clone())),
            }
        }
        let anything = var_checks.is_some()
            || id_checks
            || command.report_empty
            || summaries
            || !cross.is_empty()
            || case_report
            || !saves.is_empty();
        if !anything {
            return Err(Error(
                "nothing to report or save with these settings".into(),
            ));
        }

        let cases = data.case_count();
        let number = |case: usize| self.case_numbers.as_ref().map_or(case, |n| n[case]) + 1;
        let shown = self.shown();
        let id_vars: Vec<Variable> = ids
            .iter()
            .map(|id| data.variable(id))
            .collect::<Result<_, _>>()?;
        let case_cells = |case: usize| -> Vec<Cell> {
            let mut cells = vec![Cell::Number(number(case) as f64)];
            for v in &id_vars {
                cells.push(Cell::Text(match v.values() {
                    Values::Numeric(x) => {
                        x.get(case).map_or(".".to_string(), |n| shown.number(v, n))
                    }
                    Values::String(x) => x.get(case).to_string(),
                }));
            }
            cells
        };
        let case_columns = |more: &[&str]| -> Vec<String> {
            let mut columns = vec!["Case".to_string()];
            columns.extend(id_vars.iter().map(|v| shown.variable(v)));
            columns.extend(more.iter().map(|m| m.to_string()));
            columns
        };
        let mut tables = Vec::new();

        if let Some(checks) = var_checks {
            let scale_min = self.scale_min();
            let mut rows = Vec::new();
            for name in &analysis {
                let index = data.variable_index(name).expect("expanded");
                let variable = data.variable_at(index);
                let label = shown.variable(&variable);
                let mut flag = |check: String, value: f64| {
                    rows.push(vec![
                        Cell::Text(check),
                        Cell::Text(label.clone()),
                        Cell::Number(value),
                    ])
                };
                let valid: Vec<usize> = (0..cases)
                    .filter(|&c| !missing(&variable, c, false))
                    .collect();
                if cases > 0 {
                    let pct = 100.0 * (cases - valid.len()) as f64 / cases as f64;
                    if pct > checks.pct_missing {
                        flag(format!("Missing values above {}%", checks.pct_missing), pct);
                    }
                }
                if valid.is_empty() {
                    continue;
                }
                let measure = match variable.measure() {
                    Measure::Unknown => default_measure(&data, index, scale_min),
                    m => m,
                };
                match (variable.values(), measure) {
                    (Values::Numeric(values), Measure::Scale) => {
                        let x: Vec<f64> = valid.iter().filter_map(|&c| values.get(c)).collect();
                        if x.len() < 2 {
                            continue;
                        }
                        let n = x.len() as f64;
                        let mean = x.iter().sum::<f64>() / n;
                        let sd =
                            (x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
                        if checks.cv > 0.0 && mean != 0.0 && (sd / mean).abs() < checks.cv {
                            flag(
                                format!("Coefficient of variation below {}", checks.cv),
                                (sd / mean).abs(),
                            );
                        }
                        if sd < checks.std_dev {
                            flag(format!("Standard deviation below {}", checks.std_dev), sd);
                        }
                    }
                    (values, _) => {
                        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
                        for &c in &valid {
                            let key = match &values {
                                Values::Numeric(v) => format!("{}", v.get(c).unwrap_or_default()),
                                Values::String(v) => v.get(c).trim_end().to_string(),
                            };
                            *counts.entry(key).or_default() += 1;
                        }
                        let largest = counts.values().max().copied().unwrap_or_default();
                        let pct = 100.0 * largest as f64 / valid.len() as f64;
                        if pct > checks.pct_equal {
                            flag(
                                format!(
                                    "Largest category above {}% of the valid values",
                                    checks.pct_equal
                                ),
                                pct,
                            );
                        }
                        let singles = counts.values().filter(|n| **n == 1).count();
                        let pct = 100.0 * singles as f64 / counts.len() as f64;
                        if pct > checks.pct_unequal {
                            flag(
                                format!("Categories with one case above {}%", checks.pct_unequal),
                                pct,
                            );
                        }
                    }
                }
            }
            let mut table = listed(
                "Variable Checks",
                vec!["Check".into(), "Variable".into(), "Value".into()],
                rows,
                "No analysis variable failed a check.",
            );
            table.row_headers = 2;
            tables.push(table);
        }

        // incomplete and duplicate IDs
        let incomplete: Vec<bool> = (0..cases)
            .map(|c| id_vars.iter().any(|v| missing(v, c, true)))
            .collect();
        let mut groups = vec![0usize; cases];
        if !ids.is_empty() {
            let key = |case: usize| -> Vec<String> {
                id_vars
                    .iter()
                    .map(|v| match v.values() {
                        Values::Numeric(x) => format!("{}", x.get(case).unwrap_or_default()),
                        Values::String(x) => x.get(case).trim_end().to_string(),
                    })
                    .collect()
            };
            let mut seen: BTreeMap<Vec<String>, Vec<usize>> = BTreeMap::new();
            for case in (0..cases).filter(|&c| !incomplete[c]) {
                seen.entry(key(case)).or_default().push(case);
            }
            let mut duplicated: Vec<&Vec<usize>> = seen.values().filter(|c| c.len() > 1).collect();
            duplicated.sort_by_key(|c| c[0]);
            for (group, members) in duplicated.iter().enumerate() {
                for &case in *members {
                    groups[case] = group + 1;
                }
            }
        }
        if id_checks && command.incomplete_ids {
            let rows = (0..cases)
                .filter(|&c| incomplete[c])
                .map(case_cells)
                .collect();
            tables.push(listed(
                "Incomplete Identifiers",
                case_columns(&[]),
                rows,
                "Every case has a complete identifier.",
            ));
        }
        if id_checks && command.duplicate_ids {
            let mut order: Vec<usize> = (0..cases).filter(|&c| groups[c] > 0).collect();
            order.sort_by_key(|&c| (groups[c], c));
            let rows = order
                .into_iter()
                .map(|c| {
                    let mut row = vec![Cell::Number(groups[c] as f64)];
                    row.extend(case_cells(c));
                    row
                })
                .collect();
            let mut columns = vec!["Group".to_string()];
            columns.extend(case_columns(&[]));
            tables.push(listed(
                "Duplicate Identifiers",
                columns,
                rows,
                "No two cases share an identifier.",
            ));
        }

        // empty cases
        let scope = match command.empty_scope {
            Some(EmptyScope::AnalysisVariables) | None if !analysis.is_empty() => analysis.clone(),
            _ => data
                .variable_names()
                .filter(|name| {
                    let name = name.to_string();
                    !left_out(&name) && !ids.iter().any(|id| is(id, &name))
                })
                .map(str::to_string)
                .collect(),
        };
        let scope_vars: Vec<Variable> = scope
            .iter()
            .map(|name| data.variable(name))
            .collect::<Result<_, _>>()?;
        let empty: Vec<bool> = (0..cases)
            .map(|c| !scope_vars.is_empty() && scope_vars.iter().all(|v| missing(v, c, true)))
            .collect();
        if command.report_empty {
            let rows = (0..cases).filter(|&c| empty[c]).map(case_cells).collect();
            tables.push(listed(
                "Empty Cases",
                case_columns(&[]),
                rows,
                "No case is empty.",
            ));
        }

        // rule violations
        let link_counts: Vec<usize> = links
            .iter()
            .map(|l| (0..cases).filter(|&c| broken(&data, &l.outcome, c)).count())
            .collect();
        let label_of = |n: usize| match single.get(&n) {
            Some(Ok(rule)) => rule.label.clone(),
            _ => format!("$VD.SRule[{n}]"),
        };
        if summaries && command.by_variable {
            let mut rows = Vec::new();
            for name in &analysis {
                let mine: Vec<usize> = (0..links.len())
                    .filter(|&i| is(&links[i].variable, name) && link_counts[i] > 0)
                    .collect();
                if mine.is_empty() {
                    continue;
                }
                let variable = shown.named(&data, name);
                for &i in &mine {
                    rows.push(vec![
                        Cell::Text(variable.clone()),
                        Cell::Text(label_of(links[i].rule)),
                        Cell::Number(link_counts[i] as f64),
                    ]);
                }
                let total: usize = mine.iter().map(|&i| link_counts[i]).sum();
                rows.push(vec![
                    Cell::Text(variable),
                    Cell::Text("Total".into()),
                    Cell::Number(total as f64),
                ]);
            }
            let mut table = listed(
                "Single-Variable Rule Violations by Variable",
                vec!["Variable".into(), "Rule".into(), "Violations".into()],
                rows,
                "No value breaks a single-variable rule.",
            );
            table.row_headers = 2;
            tables.push(table);
        }
        if summaries && command.by_rule {
            let mut rows = Vec::new();
            for (&n, _) in single.iter().filter(|(_, r)| r.is_ok()) {
                let mine: Vec<usize> = (0..links.len())
                    .filter(|&i| links[i].rule == n && link_counts[i] > 0)
                    .collect();
                if mine.is_empty() {
                    continue;
                }
                for &i in &mine {
                    rows.push(vec![
                        Cell::Text(label_of(n)),
                        Cell::Text(shown.named(&data, &links[i].variable)),
                        Cell::Number(link_counts[i] as f64),
                    ]);
                }
                let total: usize = mine.iter().map(|&i| link_counts[i]).sum();
                rows.push(vec![
                    Cell::Text(label_of(n)),
                    Cell::Text("Total".into()),
                    Cell::Number(total as f64),
                ]);
            }
            let mut table = listed(
                "Single-Variable Rule Violations by Rule",
                vec!["Rule".into(), "Variable".into(), "Violations".into()],
                rows,
                "No value breaks a single-variable rule.",
            );
            table.row_headers = 2;
            tables.push(table);
        }
        if !cross.is_empty() {
            let rows = cross
                .iter()
                .map(|(_, rule)| {
                    let count = (0..cases)
                        .filter(|&c| broken(&data, &rule.outcome, c))
                        .count();
                    vec![
                        Cell::Text(rule.label.clone()),
                        Cell::Number(count as f64),
                        Cell::Text(rule.detail.clone()),
                    ]
                })
                .collect();
            tables.push(listed(
                "Cross-Variable Rule Violations",
                vec!["Rule".into(), "Violations".into(), "Expression".into()],
                rows,
                "",
            ));
        }
        let mut violations = vec![0usize; cases];
        let mut which: Vec<Vec<String>> = vec![Vec::new(); cases];
        for link in &links {
            for case in (0..cases).filter(|&c| broken(&data, &link.outcome, c)) {
                violations[case] += 1;
                let variable = shown.named(&data, &link.variable);
                which[case].push(format!("{} ({variable})", label_of(link.rule)));
            }
        }
        for (_, rule) in &cross {
            for case in (0..cases).filter(|&c| broken(&data, &rule.outcome, c)) {
                violations[case] += 1;
                which[case].push(rule.label.clone());
            }
        }
        if case_report {
            let found: Vec<usize> = (0..cases)
                .filter(|&c| violations[c] >= command.min_violations)
                .collect();
            let limit = command.case_limit.unwrap_or(found.len()).min(found.len());
            let rows = found[..limit]
                .iter()
                .map(|&c| {
                    let mut row = case_cells(c);
                    row.push(Cell::Number(violations[c] as f64));
                    row.push(Cell::Text(which[c].join("; ")));
                    row
                })
                .collect();
            let mut table = listed(
                "Case Report",
                case_columns(&["Violations", "Rules broken"]),
                rows,
                "No case breaks enough rules to be listed.",
            );
            if limit < found.len() {
                table.caption = Some(format!(
                    "The first {limit} of {} cases are listed.",
                    found.len()
                ));
            }
            tables.push(table);
        }

        // saved variables: names first, so that a clash changes nothing
        let mut names: Vec<String> = Vec::new();
        for (what, name) in &saves {
            let taken =
                |n: &str| data.variable_index(n).is_some() || names.iter().any(|m| is(m, n));
            let name = match name {
                Some(name) if taken(name) => {
                    return Err(Error(format!(
                        "/SAVE names {name}, a variable that already exists; choose another name"
                    )));
                }
                Some(name) => name.clone(),
                None => {
                    let root = match what {
                        ValidateSave::EmptyCase => "EmptyCase",
                        ValidateSave::IncompleteId => "IncompleteID",
                        ValidateSave::DuplicateId => "DuplicateIDGroup",
                        ValidateSave::RuleViolations => "ValidationRuleViolations",
                    };
                    std::iter::once(root.to_string())
                        .chain((1..).map(|i| format!("{root}_{i}")))
                        .find(|n| !taken(n))
                        .expect("a free name")
                }
            };
            names.push(name);
        }
        let flag = |values: &[bool]| {
            values
                .iter()
                .map(|b| Some(f64::from(u8::from(*b))))
                .collect()
        };
        let out = self.active_mut()?;
        for ((what, _), name) in saves.iter().zip(&names) {
            let (values, label, format, measure): (Vec<Option<f64>>, &str, &str, Measure) =
                match what {
                    ValidateSave::EmptyCase => {
                        (flag(&empty), "Empty case", "F1.0", Measure::Nominal)
                    }
                    ValidateSave::IncompleteId => (
                        flag(&incomplete),
                        "Incomplete identifier",
                        "F1.0",
                        Measure::Nominal,
                    ),
                    ValidateSave::DuplicateId => (
                        groups.iter().map(|g| Some(*g as f64)).collect(),
                        "Duplicate identifier group",
                        "F8.0",
                        Measure::Nominal,
                    ),
                    ValidateSave::RuleViolations => (
                        violations.iter().map(|v| Some(*v as f64)).collect(),
                        "Validation rule violations",
                        "F8.0",
                        Measure::Scale,
                    ),
                };
            out.set_numeric_values(name, values)?;
            out.set_format(name, format)?;
            out.set_measure(name, measure)?;
            out.set_variable_label(name, Some(label))?;
        }
        Ok(tables)
    }
}

#[cfg(test)]
#[test]
fn rule_texts_read_as_properties() {
    let props =
        properties("Label='Age range', Type='Numeric',List='1' '2'  ,Note='it''s'").unwrap();
    assert_eq!(get(&props, "label"), Some("Age range"));
    assert_eq!(get(&props, "list"), Some("1 2"));
    assert_eq!(get(&props, "note"), Some("it's"));
    assert!(properties("Label=Age").is_none());
    assert!(properties("Label='open").is_none());
    assert_eq!(rule_number(" $vd.srule[12] "), Some(12));
    assert_eq!(rule_number("$VD.CRule[1]"), None);
}
