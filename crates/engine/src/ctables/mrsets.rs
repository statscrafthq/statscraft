//! Multiple response sets in CTABLES.
//!
//! A set (`$name`) stands where a categorical variable stands. Its categories are the
//! variables of a dichotomy set, or the values of the variables of a category set. Unlike a
//! categorical variable, a case can fall in several categories of a set, or in none:
//!
//! - A case counts once in every category it has a response in. A dichotomy set has a
//!   response in a variable that holds the counted value; a category set has one in every
//!   listed value of its variables.
//! - The total of a set counts every case with at least one response once, so it is less than
//!   the sum of its categories.
//! - RESPONSES counts the responses instead: in a dichotomy set it equals the count of a
//!   category, and its total is the sum of the categories. In a category set a value given in
//!   two variables of one case is one response unless COUNTDUPLICATES=YES; counts never count it
//!   twice.
//! - A percentage area of a case counts the case once, so the percentages of a set over an area
//!   that its categories do not decide (column percentages of a set in the rows) add up to more
//!   than 100. The .RESPONSES functions use responses in the numerator and the denominator,
//!   .RESPONSES.COUNT responses over the count, .COUNT.RESPONSES the count over the responses.
//! - A case without a response in a set (a dichotomy set without the counted value, a category
//!   set whose values are all missing) still counts in TOTALN and the .TOTALN percentages, but
//!   not in COUNT or VALIDN. A case whose responses all lie in categories left out by
//!   /CATEGORIES is not in the table at all, like a hidden category of a variable.
//! - A category set whose values are user-missing for any of its variables treats them as
//!   missing categories: MISSING=INCLUDE shows them, and they count in COUNT but not VALIDN.
//! - The case weight weighs every response of the case.

use super::{Category, CategoryList, Key, Tally, TermVar, VlabelsDisplay, add_total, categories};
use crate::{Engine, Error};
use statscraft_data::{Dataset, Values, Variable};
use statscraft_syntax::{
    AxisVariable, Categories, CategoryItem, Ctables, PercentOf, Statistic, Summary,
    SummaryFunction, VarItem,
};
use std::collections::HashMap;

/// A set as a table variable, resolved against the dictionary.
pub(super) struct Set<'a> {
    /// As the dictionary spells it, with the "$".
    pub name: &'a str,
    pub variables: Vec<Variable<'a>>,
    /// The counted value of a dichotomy set; None for a category set.
    pub counted: Option<Key>,
    /// /MRSETS COUNTDUPLICATES=YES
    pub count_duplicates: bool,
}

/// The categories a case falls in for one categorical variable, each once and with its number
/// of responses. Empty for a set without a response: the case then counts in TOTALN only.
pub(super) type Choice = Vec<(usize, f64)>;

impl Engine {
    /// The table variable for `$name`.
    pub(super) fn set_term<'a>(
        &self,
        data: &'a Dataset,
        command: &Ctables,
        syntax: &'a AxisVariable,
    ) -> Result<TermVar<'a>, Error> {
        let set = data
            .multiple_response_sets()
            .iter()
            .find(|set| set.name.eq_ignore_ascii_case(&syntax.name))
            .ok_or_else(|| {
                Error(format!(
                    "/TABLE: unknown multiple response set {} (define it with MRSETS)",
                    syntax.name
                ))
            })?;
        if syntax.scale == Some(true) {
            return Err(Error(format!(
                "/TABLE: {} is a multiple response set, so it cannot be a scale variable; remove [S]",
                set.name
            )));
        }
        let variables = set
            .variables
            .iter()
            .map(|name| data.variable(name))
            .collect::<Result<Vec<_>, _>>()?;
        let counted = match &set.counted_value {
            None => None,
            Some(text) if variables[0].is_string() => Some(Key::Text(text.trim_end().into())),
            Some(text) => Some(Key::Number(text.trim().parse::<f64>().map_err(|_| {
                Error(format!(
                    "/TABLE: {} counts the value {text}, which is not a number",
                    set.name
                ))
            })?)),
        };
        let name = set.name.as_str();
        let label = match self.display(command, name)? {
            VlabelsDisplay::None => None,
            VlabelsDisplay::Name => Some(name.to_string()),
            VlabelsDisplay::Both if !set.label.is_empty() => Some(format!("{name} {}", set.label)),
            _ if set.label.is_empty() => Some(name.to_string()),
            _ => Some(set.label.clone()),
        };
        Ok(TermVar {
            variable: variables[0],
            syntax,
            scale: false,
            label,
            set: Some(Set {
                name,
                variables,
                counted,
                count_duplicates: command.count_duplicates,
            }),
        })
    }

    /// The names a variable list of /VLABELS or /CATEGORIES refers to: variables as the
    /// dictionary spells them, and sets. ALL includes every set.
    pub(super) fn names_with_sets(&self, items: &[VarItem]) -> Result<Vec<String>, Error> {
        let sets = self.active()?.multiple_response_sets();
        let mut plain = Vec::new();
        let mut names = Vec::new();
        for item in items {
            match item {
                VarItem::Name(name) if name.starts_with('$') => {
                    let set = sets
                        .iter()
                        .find(|set| set.name.eq_ignore_ascii_case(name))
                        .ok_or_else(|| Error(format!("unknown multiple response set {name}")))?;
                    names.push(set.name.clone());
                }
                VarItem::All => {
                    names.extend(sets.iter().map(|set| set.name.clone()));
                    plain.push(item.clone());
                }
                _ => plain.push(item.clone()),
            }
        }
        names.extend(self.expand(&plain)?);
        Ok(names)
    }
}

/// The categories of a set in display order.
pub(super) fn set_categories(set: &Set, spec: Option<&Categories>) -> Result<CategoryList, Error> {
    let names = spec
        .and_then(|s| s.values.as_ref())
        .is_some_and(|items| items.iter().any(|i| matches!(i, CategoryItem::Name(_))));
    let Some(counted) = &set.counted else {
        if names {
            return Err(Error(format!(
                "/CATEGORIES for {}: a category set lists values; only a dichotomy set lists its variables by name",
                set.name
            )));
        }
        return categories(set.name, &set.variables, spec);
    };
    let label = |v: &Variable| v.label().unwrap_or(v.name()).to_string();
    let mut order: Vec<usize> = match spec.and_then(|s| s.values.as_ref()) {
        Some(items) => {
            let mut order = Vec::new();
            for item in items {
                let CategoryItem::Name(name) = item else {
                    return Err(Error(format!(
                        "/CATEGORIES for {}: the categories of a dichotomy set are its variables, so list them by name",
                        set.name
                    )));
                };
                let index = set
                    .variables
                    .iter()
                    .position(|v| v.name().eq_ignore_ascii_case(name))
                    .ok_or_else(|| {
                        Error(format!(
                            "/CATEGORIES for {}: {name} is not a variable of the set",
                            set.name
                        ))
                    })?;
                // a variable named twice takes the place of its last mention
                order.retain(|i| *i != index);
                order.push(index);
            }
            order
        }
        None => {
            let mut order: Vec<usize> = (0..set.variables.len()).collect();
            if spec.is_some_and(|s| s.by_label) {
                order.sort_by_cached_key(|i| label(&set.variables[*i]));
            }
            if spec.is_some_and(|s| s.descending) {
                order.reverse();
            }
            order
        }
    };
    if spec.is_some_and(|s| s.exclude_empty) {
        order.retain(|i| match (set.variables[*i].values(), counted) {
            (Values::Numeric(values), Key::Number(n)) => values.iter().any(|x| x == Some(*n)),
            (Values::String(values), Key::Text(t)) => values.iter().any(|s| s == t),
            _ => false,
        });
    }
    let mut list = CategoryList {
        categories: Vec::new(),
        numbers: HashMap::new(),
        texts: HashMap::new(),
    };
    for i in order {
        let v = &set.variables[i];
        list.texts
            .insert(v.name().to_string(), list.categories.len());
        list.categories.push(Category {
            key: Some(Key::Text(v.name().to_string())),
            label: label(v),
            missing: false,
        });
    }
    add_total(&mut list, spec);
    Ok(list)
}

/// True when the variable holds `key` in `case`.
fn holds(variable: &Variable, case: usize, key: &Key) -> bool {
    match (variable.values(), key) {
        (Values::Numeric(values), Key::Number(n)) => values.get(case) == Some(*n),
        (Values::String(values), Key::Text(t)) => values.get(case) == t,
        _ => false,
    }
}

/// The categories of `case` for one categorical variable of a section, or None when the case
/// is not in the section.
pub(super) fn choice(var: &TermVar, list: &CategoryList, case: usize) -> Option<Choice> {
    let Some(set) = &var.set else {
        return list.find(&var.variable, case).map(|i| vec![(i, 1.0)]);
    };
    let mut found: Choice = Vec::new();
    // a response in a left-out category, and a missing value
    let (mut hidden, mut missing) = (false, false);
    for v in &set.variables {
        let index = match &set.counted {
            Some(counted) if holds(v, case, counted) => list.texts.get(v.name()).copied(),
            Some(_) => continue,
            None => list.find(v, case),
        };
        match index {
            Some(index) => match found.iter_mut().find(|(c, _)| *c == index) {
                Some((_, responses)) if set.count_duplicates => *responses += 1.0,
                Some(_) => {}
                None => found.push((index, 1.0)),
            },
            None if set.counted.is_some() => hidden = true,
            None => match v.values() {
                Values::Numeric(values) => match values.get(case) {
                    Some(x) if !v.is_user_missing(x) => hidden = true,
                    _ => missing = true,
                },
                Values::String(values) if v.is_user_missing_str(values.get(case)) => missing = true,
                Values::String(_) => hidden = true,
            },
        }
    }
    // without a response the case counts in TOTALN, unless all it gave is left out (a
    // dichotomy set without the counted value has nothing left out)
    let in_totaln = match &set.counted {
        Some(_) => !hidden,
        None => missing,
    };
    (!found.is_empty() || in_totaln).then_some(found)
}

/// Every combination of one entry of each part, the first part varying slowest.
pub(super) fn product(parts: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut combos = vec![Vec::new()];
    for part in parts {
        combos = combos
            .into_iter()
            .flat_map(|combo| {
                part.iter().map(move |&i| {
                    let mut next = combo.clone();
                    next.push(i);
                    next
                })
            })
            .collect();
    }
    combos
}

impl Tally {
    /// Adds a case with its responses; a case without a response in some set adds to TOTALN
    /// only.
    pub(super) fn add_case(
        &mut self,
        weight: f64,
        (valid, responses): (bool, f64),
        value: Option<f64>,
        keep_values: bool,
        total_only: bool,
    ) {
        if total_only {
            self.total_only += weight;
            self.total_only_cases += 1.0;
            return;
        }
        self.add(weight, valid, value, keep_values);
        self.responses += weight * responses;
        self.unweighted_responses += responses;
    }
}

/// The functions that only multiple response sets have.
pub(super) fn counts_responses(function: SummaryFunction) -> bool {
    matches!(
        function.statistic,
        Statistic::Responses
            | Statistic::Percent(
                _,
                PercentOf::Responses | PercentOf::ResponsesCount | PercentOf::CountResponses
            )
    )
}

/// The numerator and the denominator of a percentage.
pub(super) fn parts(of: PercentOf) -> (PercentOf, PercentOf) {
    match of {
        PercentOf::ResponsesCount => (PercentOf::Responses, PercentOf::Count),
        PercentOf::CountResponses => (PercentOf::Count, PercentOf::Responses),
        of => (of, of),
    }
}

/// Response functions need a set, and a set has no functions of scale variables.
pub(super) fn check_functions(var: &TermVar, summaries: &[Summary]) -> Result<(), Error> {
    let name = var.name();
    for s in summaries {
        let responses = counts_responses(s.function);
        if responses && var.set.is_none() {
            return Err(Error(format!(
                "/TABLE: summary function {} applies only to multiple response sets, and {name} is not one",
                s.function.name()
            )));
        }
        if var.set.is_some() && !responses && super::scale_only(s.function) {
            return Err(Error(format!(
                "/TABLE: summary function {} applies only to scale variables, and {name} is a multiple response set",
                s.function.name()
            )));
        }
    }
    Ok(())
}

/// The set of a nesting that carries the summaries. A set may carry them although categorical
/// variables are nested inside it, but not when another set is.
pub(super) fn summary_set<'t, 'a>(term: &'t [TermVar<'a>]) -> Option<&'t TermVar<'a>> {
    let i = term
        .iter()
        .rposition(|v| v.set.is_some() && v.syntax.summaries.is_some())?;
    (term[i + 1..]
        .iter()
        .all(|v| v.set.is_none() && v.syntax.summaries.is_none()))
    .then(|| &term[i])
}
