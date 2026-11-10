//! CTABLES: custom tables of counts, percentages and statistics.
//!
//! An axis expression is first multiplied out into terms: `(a + b) > c` becomes the stacked
//! terms `a > c` and `b > c`. Every pair of a row term and a column term is a section of the
//! table; its cells are the combinations of the categories of its categorical variables. A
//! case belongs to a section when every categorical variable of the section has one of its
//! categories, and it adds to every cell it falls in, total categories included. A multiple
//! response set can give a case several categories or none (see `mrsets`).
//!
//! A case whose value of a categorical variable is missing (system-missing, or user-missing and
//! not shown) still counts in TOTALN and the .TOTALN percentages: in the totals of that
//! variable and in every area the variable does not decide. A case whose valid value is left
//! out by an explicit category list is not in the table at all.
//!
//! Layers, the third axis after a second BY, are a dimension of the sections like the rows and
//! the columns. Our result table has two dimensions, so every layer entry (a combination of the
//! categories of a layer term, and a summary with SLABELS POSITION=LAYER) becomes a table of its
//! own whose title names the layer, followed by its tests; the manual's pivot table shows one
//! layer at a time in the same way. A layer variable decides every percentage area except the
//! table's, so TABLEPCT spans all layers and LAYERPCT is the table percentage of one layer.

mod format;
mod mrsets;
mod pcompute;
mod sigtest;

pub use format::format_value;

use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Measure, Value as DataValue, Values, Variable};
use statscraft_syntax::{
    AxisExpr, AxisVariable, Bound, Categories, CategoryItem, Ctables, CtablesTable, EmptyCells,
    Format, LabelsMoved, PercentBase, PercentOf, Statistic, Summary, SummaryFunction,
    Value as SyntaxValue, VlabelsDisplay,
};
use std::collections::HashMap;

/// A category value: a number or a right-trimmed string.
#[derive(Clone, Debug, PartialEq)]
enum Key {
    Number(f64),
    Text(String),
}

impl Key {
    fn compare(&self, other: &Key) -> std::cmp::Ordering {
        match (self, other) {
            (Key::Number(a), Key::Number(b)) => a.total_cmp(b),
            (Key::Text(a), Key::Text(b)) => a.cmp(b),
            (Key::Number(_), Key::Text(_)) => std::cmp::Ordering::Less,
            (Key::Text(_), Key::Number(_)) => std::cmp::Ordering::Greater,
        }
    }

    fn of(value: &DataValue) -> Key {
        match value {
            DataValue::Number(n) => Key::Number(*n),
            DataValue::String(s) => Key::Text(s.trim_end().to_string()),
        }
    }
}

#[derive(Default)]
struct Category {
    /// None for the total, a subtotal and a computed category.
    key: Option<Key>,
    label: String,
    /// A user-missing value, listed because of MISSING=INCLUDE or an explicit list.
    missing: bool,
    /// A category computed by /PCOMPUTE.
    computed: Option<pcompute::Computed>,
    /// Not shown: a source of a computed category with HIDESOURCECATS=YES, or a category that
    /// an HSUBTOTAL sums.
    hidden: bool,
    /// A subtotal of the categories before it (after it with POSITION=BEFORE).
    subtotal: Option<Subtotal>,
    /// The position of the subtotal that sums this category.
    within: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
struct Subtotal {
    /// HSUBTOTAL: the categories it sums are hidden.
    hides: bool,
}

/// The categories of one categorical variable, in display order.
struct CategoryList {
    categories: Vec<Category>,
    numbers: HashMap<u64, usize>,
    texts: HashMap<String, usize>,
}

impl CategoryList {
    fn find(&self, variable: &Variable, case: usize) -> Option<usize> {
        match variable.values() {
            Values::Numeric(values) => {
                let v = values.get(case)?;
                self.numbers.get(&(v + 0.0).to_bits()).copied()
            }
            Values::String(values) => self.texts.get(values.get(case)).copied(),
        }
    }

    fn total(&self) -> Option<usize> {
        self.categories
            .iter()
            .position(|c| c.key.is_none() && c.computed.is_none() && c.subtotal.is_none())
    }

    /// The categories a case in category `c` adds to: `c` itself and the subtotal that sums it.
    fn with_subtotal(&self, c: usize) -> impl Iterator<Item = usize> {
        std::iter::once(c).chain(self.categories[c].within)
    }

    /// Whether a case in category `c` counts in the cell of category `target`.
    fn covers(&self, target: usize, c: usize) -> bool {
        target == c || self.categories[c].within == Some(target)
    }
}

/// One variable of a term, resolved against the dictionary.
struct TermVar<'a> {
    /// For a multiple response set its first variable.
    variable: Variable<'a>,
    syntax: &'a AxisVariable,
    scale: bool,
    /// The header label, None for VLABELS DISPLAY=NONE.
    label: Option<String>,
    set: Option<mrsets::Set<'a>>,
}

impl TermVar<'_> {
    /// The variable or set name.
    fn name(&self) -> &str {
        self.set
            .as_ref()
            .map_or(self.variable.name(), |set| set.name)
    }

    /// The name that identifies the variable or set in a table.
    fn key(&self) -> String {
        self.name().to_lowercase()
    }
}

type Term<'a> = Vec<TermVar<'a>>;

/// A summary as it is listed in the table: function, percentile and label. Summaries of
/// stacked terms that agree on these share a row or column.
#[derive(Clone, PartialEq)]
struct Slot {
    function: SummaryFunction,
    percentile: Option<f64>,
    label: String,
}

fn expand(expr: &AxisExpr) -> Vec<Vec<&AxisVariable>> {
    match expr {
        AxisExpr::Variable(v) => vec![vec![v]],
        AxisExpr::Stack(a, b) => {
            let mut terms = expand(a);
            terms.extend(expand(b));
            terms
        }
        AxisExpr::Nest(a, b) => {
            let inner = expand(b);
            let mut terms = Vec::new();
            for outer in expand(a) {
                for tail in &inner {
                    terms.push(outer.iter().chain(tail).copied().collect());
                }
            }
            terms
        }
    }
}

fn percent_label(base: PercentBase, of: PercentOf) -> String {
    let base = match base {
        PercentBase::Row => "Row",
        PercentBase::Column => "Column",
        PercentBase::Table => "Table",
        PercentBase::Subtable => "Subtable",
        PercentBase::Layer => "Layer",
        PercentBase::LayerRow => "Layer Row",
        PercentBase::LayerColumn => "Layer Column",
    };
    match of {
        PercentOf::Count => format!("{base} %"),
        PercentOf::Validn => format!("{base} Valid N %"),
        PercentOf::Totaln => format!("{base} Total N %"),
        PercentOf::Sum => format!("{base} Sum %"),
        PercentOf::Responses => format!("{base} Responses %"),
        PercentOf::ResponsesCount => format!("{base} Responses % (Base: Count)"),
        PercentOf::CountResponses => format!("{base} Count % (Base: Responses)"),
    }
}

fn default_label(function: SummaryFunction, percentile: Option<f64>) -> String {
    let label = match function.statistic {
        Statistic::Count => "Count".into(),
        Statistic::Percent(base, of) => percent_label(base, of),
        Statistic::Mean => "Mean".into(),
        Statistic::Median => "Median".into(),
        Statistic::Mode => "Mode".into(),
        Statistic::Minimum => "Minimum".into(),
        Statistic::Maximum => "Maximum".into(),
        Statistic::Range => "Range".into(),
        Statistic::Sum => "Sum".into(),
        Statistic::Stddev => "Std Deviation".into(),
        Statistic::Variance => "Variance".into(),
        Statistic::Semean => "Std Error of Mean".into(),
        Statistic::Ptile => format!("Percentile {}", percentile.unwrap_or(50.0)),
        Statistic::Validn => "Valid N".into(),
        Statistic::Totaln => "Total N".into(),
        Statistic::Missing => "Missing".into(),
        Statistic::Responses => "Responses".into(),
    };
    if function.unweighted {
        format!("Unweighted {label}")
    } else {
        label
    }
}

/// Functions that need the values of a scale variable.
fn scale_only(function: SummaryFunction) -> bool {
    !matches!(
        function.statistic,
        Statistic::Count
            | Statistic::Percent(_, PercentOf::Count | PercentOf::Validn | PercentOf::Totaln)
    )
}

/// A format as the cells name it, such as "PCT8.1".
fn format_text(f: &Format) -> String {
    let mut text = format!("{}{}", f.kind.name(), f.width);
    if f.decimals > 0 {
        text.push_str(&format!(".{}", f.decimals));
    }
    text
}

fn default_format(summary: &Summary, variable: &Variable) -> String {
    if let Some(f) = &summary.format {
        return format_text(f);
    }
    match summary.function.statistic {
        Statistic::Count | Statistic::Validn | Statistic::Totaln | Statistic::Responses => {
            "F40.0".into()
        }
        Statistic::Percent(..) => "PCT40.1".into(),
        _ => variable.format().to_string(),
    }
}

fn slot(summary: &Summary) -> Slot {
    Slot {
        function: summary.function,
        percentile: summary.percentile,
        label: summary
            .label
            .clone()
            .unwrap_or_else(|| default_label(summary.function, summary.percentile)),
    }
}

/// What a cell or a percentage area has gathered. `values` holds the valid (value, weight)
/// pairs of the summary variable when it is a scale variable.
#[derive(Default)]
struct Tally {
    cases: f64,
    count: f64,
    valid_cases: f64,
    valid: f64,
    sum: f64,
    unweighted_sum: f64,
    values: Vec<(f64, f64)>,
    responses: f64,
    unweighted_responses: f64,
    /// Cases without a response in a multiple response set: they count in TOTALN only.
    total_only: f64,
    total_only_cases: f64,
}

impl Tally {
    fn add(&mut self, weight: f64, valid: bool, value: Option<f64>, keep_values: bool) {
        self.cases += 1.0;
        self.count += weight;
        if valid {
            self.valid_cases += 1.0;
            self.valid += weight;
        }
        if let Some(x) = value {
            self.sum += weight * x;
            self.unweighted_sum += x;
            if keep_values {
                self.values.push((x, weight));
            }
        }
    }

    /// The numerator or denominator of a percentage.
    fn base(&self, of: PercentOf, unweighted: bool) -> f64 {
        match (of, unweighted) {
            (PercentOf::Count, false) => self.count,
            (PercentOf::Count, true) => self.cases,
            (PercentOf::Totaln, false) => self.count + self.total_only,
            (PercentOf::Totaln, true) => self.cases + self.total_only_cases,
            (PercentOf::Validn, false) => self.valid,
            (PercentOf::Validn, true) => self.valid_cases,
            (PercentOf::Sum, false) => self.sum,
            (PercentOf::Sum, true) => self.unweighted_sum,
            (PercentOf::Responses, false) => self.responses,
            (PercentOf::Responses, true) => self.unweighted_responses,
            (PercentOf::ResponsesCount | PercentOf::CountResponses, _) => {
                unreachable!("mrsets::parts splits these")
            }
        }
    }
}

/// Sorted distinct values with their summed weights, leaving out values that weigh nothing.
fn distinct(values: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut cells: Vec<(f64, f64)> = values.iter().copied().filter(|(_, w)| *w > 0.0).collect();
    cells.sort_by(|a, b| a.0.total_cmp(&b.0));
    cells.dedup_by(|next, kept| {
        let same = next.0 == kept.0;
        if same {
            kept.1 += next.1;
        }
        same
    });
    cells
}

/// The percentile by the averaged empirical distribution: the first value whose cumulated
/// weight passes W * p / 100, or the mean of two neighbouring values when the cumulated weight
/// below the second one equals W * p / 100 exactly. With weight 1 for every case the median is
/// the middle value or the mean of the two middle values.
fn percentile(values: &[(f64, f64)], p: f64) -> Option<f64> {
    let cells = distinct(values);
    let total: f64 = cells.iter().map(|(_, w)| w).sum();
    let target = total * p / 100.0;
    // cumulated weights carry rounding error, so "equal" allows for it
    let slack = total * 1e-12;
    let mut below = 0.0;
    let mut previous: Option<f64> = None;
    for &(value, weight) in &cells {
        if target + slack < below + weight {
            return Some(match previous {
                Some(previous) if (target - below).abs() <= slack => (previous + value) / 2.0,
                _ => value,
            });
        }
        below += weight;
        previous = Some(value);
    }
    previous
}

fn statistic(summary: &Summary, tally: &Tally) -> Option<f64> {
    let unweighted = summary.function.unweighted;
    let values: Vec<(f64, f64)> = tally
        .values
        .iter()
        .map(|&(x, w)| (x, if unweighted { 1.0 } else { w }))
        .collect();
    // The mean is updated case by case, as the reference program does: the last digit of a
    // mean that sits on a rounding half depends on it.
    let (mut total, mut running, mut squares) = (0.0, 0.0, 0.0);
    for &(x, w) in values.iter().filter(|(_, w)| *w > 0.0) {
        total += w;
        let delta = x - running;
        running += delta * (w / total);
        squares += w * delta * (x - running);
    }
    let mean = (total > 0.0).then_some(running);
    let variance = || (total > 1.0).then(|| squares / (total - 1.0));
    let counted = || values.iter().filter(|(_, w)| *w > 0.0).map(|(x, _)| *x);
    let count = tally.base(PercentOf::Count, unweighted);
    let valid = tally.base(PercentOf::Validn, unweighted);
    let result = match summary.function.statistic {
        Statistic::Count => Some(count),
        Statistic::Totaln => Some(tally.base(PercentOf::Totaln, unweighted)),
        Statistic::Responses => Some(tally.base(PercentOf::Responses, unweighted)),
        Statistic::Validn => Some(valid),
        Statistic::Missing => Some(count - valid),
        Statistic::Mean => mean,
        Statistic::Sum => (!values.is_empty()).then(|| values.iter().map(|(x, w)| x * w).sum()),
        Statistic::Minimum => counted().reduce(f64::min),
        Statistic::Maximum => counted().reduce(f64::max),
        Statistic::Range => Some(counted().reduce(f64::max)? - counted().reduce(f64::min)?),
        Statistic::Variance => variance(),
        Statistic::Stddev => variance().map(f64::sqrt),
        Statistic::Semean => variance().map(|v| (v / total).sqrt()),
        Statistic::Median => percentile(&values, 50.0),
        Statistic::Ptile => percentile(&values, summary.percentile.unwrap_or(50.0)),
        Statistic::Mode => {
            let cells = distinct(&values);
            let most = cells.iter().map(|(_, w)| *w).fold(0.0, f64::max);
            cells.iter().find(|(_, w)| *w == most).map(|(x, _)| *x)
        }
        Statistic::Percent(..) => unreachable!("percentages need their area"),
    };
    result.filter(|v| v.is_finite())
}

/// The case weights; a missing or negative weight counts as zero. Fractional survey weights are
/// used as they are, as in FREQUENCIES and CROSSTABS. The reference program rounds every case
/// weight to a whole number in this procedure only, which would drop cases weighted below 0.5.
fn weights(data: &Dataset) -> Result<Option<Vec<f64>>, Error> {
    let Some(name) = data.weight_variable() else {
        return Ok(None);
    };
    let variable = data.variable(name)?;
    let values = variable.numeric()?;
    let usable = |v: Option<f64>| v.filter(|v| !variable.is_user_missing(*v));
    Ok(Some(
        values
            .iter()
            .map(|v| usable(v).unwrap_or(0.0).max(0.0))
            .collect(),
    ))
}

/// Whether a variable whose level is unknown counts as scale, by the default rules for the
/// measurement level, taken in order: a currency or a date or time format (but not MONTH or
/// WKDAY) is scale; no valid value is categorical; a fraction or a negative value is scale;
/// 24 or more distinct valid values, or none below 10, are scale; anything else is categorical.
/// (The rule "none below 10000" is part of "none below 10".)
fn guess_scale(variable: &Variable) -> bool {
    let Values::Numeric(values) = variable.values() else {
        return false;
    };
    let format = variable.format().to_ascii_uppercase();
    let kind = format.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    const SCALE_FORMATS: &[&str] = &[
        "DOLLAR", "CCA", "CCB", "CCC", "CCD", "CCE", "DATE", "ADATE", "EDATE", "JDATE", "SDATE",
        "QYR", "MOYR", "WKYR", "DATETIME", "YMDHMS", "TIME", "DTIME", "MTIME",
    ];
    if SCALE_FORMATS.contains(&kind) {
        return true;
    }
    let valid: Vec<f64> = values
        .iter()
        .flatten()
        .filter(|v| !variable.is_user_missing(*v))
        .collect();
    if valid.is_empty() {
        return false;
    }
    if valid.iter().any(|v| *v < 0.0 || v.fract() != 0.0) {
        return true;
    }
    let mut distinct = valid;
    distinct.sort_by(f64::total_cmp);
    distinct.dedup();
    distinct.len() >= 24 || distinct[0] >= 10.0
}

/// The value of `key` as the dictionary shows it: the value label of the first variable that
/// has one, or the value in the format of the first variable.
fn category_label(variables: &[Variable], key: &Key) -> String {
    let label = variables
        .iter()
        .flat_map(|variable| variable.value_labels())
        .find(|(v, _)| Key::of(v) == *key)
        .map(|(_, l)| l.clone());
    label.unwrap_or_else(|| match key {
        Key::Number(n) => format_value(*n, variables[0].format(), false),
        Key::Text(s) => s.clone(),
    })
}

fn in_range(key: &Key, low: &Bound, high: &Bound) -> bool {
    let Key::Number(v) = key else {
        return false;
    };
    let above = match low {
        Bound::Lowest => true,
        Bound::Highest => false,
        Bound::Number(n) => v >= n,
    };
    let below = match high {
        Bound::Highest => true,
        Bound::Lowest => false,
        Bound::Number(n) => v <= n,
    };
    above && below
}

/// An item of a category list: a value, a computed category by name, or a subtotal.
#[derive(PartialEq)]
enum Wanted {
    Value(Key),
    Computed(String),
    Subtotal(String, bool),
}

/// The categories of a variable, or of the values of the variables of a category set together.
/// `command` holds the computed categories; None for a set, which has none.
fn categories(
    name: &str,
    variables: &[Variable],
    spec: Option<&Categories>,
    command: Option<&Ctables>,
) -> Result<CategoryList, Error> {
    let variable = &variables[0];
    let mut seen: Vec<Key> = Vec::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    let id = |key: &Key| format!("{key:?}");
    let mut note = |key: Key, seen: &mut Vec<Key>| {
        let entry = counts.entry(id(&key)).or_insert(0);
        if *entry == 0 {
            seen.push(key);
        }
        *entry += 1;
    };
    for variable in variables {
        match variable.values() {
            Values::Numeric(values) => {
                for v in values.iter().flatten() {
                    note(Key::Number(v + 0.0), &mut seen);
                }
            }
            Values::String(values) => {
                for v in values.iter() {
                    note(Key::Text(v.to_string()), &mut seen);
                }
            }
        }
    }
    let in_data = seen.len();
    for (value, _) in variables.iter().flat_map(|v| v.value_labels()) {
        let key = Key::of(value);
        if !seen.contains(&key) {
            seen.push(key);
        }
    }
    // a value that is user-missing for any variable of a set is a missing category
    let is_missing = |key: &Key| {
        variables.iter().any(|variable| match key {
            Key::Number(n) => variable.is_user_missing(*n),
            Key::Text(s) => variable.is_user_missing_str(s),
        })
    };
    let exclude_empty = spec.is_some_and(|s| s.exclude_empty);

    let keys: Vec<Wanted> = match spec.and_then(|s| s.values.as_ref()) {
        Some(items) => {
            let mut wanted: Vec<Wanted> = Vec::new();
            for item in items {
                let key = |v: &SyntaxValue| match (v, variable.is_string()) {
                    (SyntaxValue::Number(n), false) => Ok(Key::Number(*n + 0.0)),
                    (SyntaxValue::String(s), true) => Ok(Key::Text(s.trim_end().to_string())),
                    (_, true) => Err(Error(format!(
                        "/CATEGORIES for {name}: {name} is a string variable, so its values are written in quotes"
                    ))),
                    (_, false) => Err(Error(format!(
                        "/CATEGORIES for {name}: {name} is numeric, so its values are numbers, not quoted strings"
                    ))),
                };
                match item {
                    CategoryItem::Value(v) => wanted.push(Wanted::Value(key(v)?)),
                    CategoryItem::Computed(n) if command.is_none() => {
                        return Err(Error(format!(
                            "/CATEGORIES for {name}: &{n} is a computed category, and those work for categorical variables, not multiple response sets"
                        )));
                    }
                    CategoryItem::Computed(n) => wanted.push(Wanted::Computed(n.clone())),
                    CategoryItem::Subtotal { label, hide } => wanted.push(Wanted::Subtotal(
                        label.clone().unwrap_or_else(|| "Subtotal".into()),
                        *hide,
                    )),
                    CategoryItem::Name(n) => {
                        return Err(Error(format!(
                            "/CATEGORIES for {name}: {n} is a name, but only a multiple dichotomy set lists its categories by name"
                        )));
                    }
                    CategoryItem::Range(low, high) => {
                        if variable.is_string() {
                            return Err(Error(format!(
                                "/CATEGORIES for {name}: ranges (THRU) work only for numeric variables"
                            )));
                        }
                        let mut found: Vec<Key> = seen
                            .iter()
                            .filter(|k| in_range(k, low, high))
                            .cloned()
                            .collect();
                        found.sort_by(Key::compare);
                        wanted.extend(found.into_iter().map(Wanted::Value));
                    }
                }
            }
            // a value named twice, alone or in a range, takes the place of its last mention
            let mut last: Vec<Wanted> = Vec::new();
            for key in wanted.into_iter().rev() {
                if matches!(key, Wanted::Subtotal(..)) || !last.contains(&key) {
                    last.push(key);
                }
            }
            last.reverse();
            last
        }
        None => {
            let include_missing = spec.is_some_and(|s| s.include_missing);
            let mut keys: Vec<Key> = seen
                .iter()
                .enumerate()
                .filter(|(i, k)| {
                    (*i < in_data || !exclude_empty) && (include_missing || !is_missing(k))
                })
                .map(|(_, k)| k.clone())
                .collect();
            if spec.is_some_and(|s| s.by_label) {
                keys.sort_by_cached_key(|k| category_label(variables, k));
            } else {
                keys.sort_by(Key::compare);
            }
            if spec.is_some_and(|s| s.descending) {
                keys.reverse();
            }
            keys.into_iter().map(Wanted::Value).collect()
        }
    };

    let mut list = CategoryList {
        categories: Vec::new(),
        numbers: HashMap::new(),
        texts: HashMap::new(),
    };
    let mut computed = Vec::new();
    for wanted in keys {
        let key = match wanted {
            Wanted::Value(key) => key,
            Wanted::Computed(n) => {
                list.categories.push(pcompute::placeholder());
                computed.push(n);
                continue;
            }
            Wanted::Subtotal(label, hides) => {
                list.categories.push(Category {
                    label,
                    subtotal: Some(Subtotal { hides }),
                    ..Default::default()
                });
                continue;
            }
        };
        if exclude_empty && !counts.contains_key(&id(&key)) {
            continue;
        }
        let index = list.categories.len();
        let duplicate = match &key {
            Key::Number(n) => list.numbers.insert(n.to_bits(), index).is_some(),
            Key::Text(s) => list.texts.insert(s.clone(), index).is_some(),
        };
        if duplicate {
            return Err(Error(format!(
                "/CATEGORIES for {name}: the value {} is listed twice",
                category_label(variables, &key)
            )));
        }
        list.categories.push(Category {
            label: category_label(variables, &key),
            missing: is_missing(&key),
            key: Some(key),
            ..Default::default()
        });
    }
    add_total(&mut list, spec);
    add_subtotals(&mut list, spec);
    if let Some(command) = command.filter(|_| !computed.is_empty()) {
        pcompute::resolve(name, !variable.is_string(), &mut list, &computed, command)?;
    }
    Ok(list)
}

/// The total category that /CATEGORIES asks for.
fn add_total(list: &mut CategoryList, spec: Option<&Categories>) {
    if let Some(spec) = spec.filter(|s| s.total) {
        let total = Category {
            label: spec.total_label.clone().unwrap_or_else(|| "Total".into()),
            ..Default::default()
        };
        if spec.total_before {
            list.categories.insert(0, total);
            list.numbers.values_mut().for_each(|i| *i += 1);
            list.texts.values_mut().for_each(|i| *i += 1);
        } else {
            list.categories.push(total);
        }
    }
}

/// Gives every subtotal its categories: the categories of the list before it, back to the
/// previous subtotal, or with POSITION=BEFORE the ones after it, up to the next subtotal.
/// Computed categories and the total take no part, and an HSUBTOTAL hides its categories.
fn add_subtotals(list: &mut CategoryList, spec: Option<&Categories>) {
    let before = spec.is_some_and(|s| s.total_before);
    let mut order: Vec<usize> = (0..list.categories.len()).collect();
    if before {
        order.reverse();
    }
    let mut pending: Vec<usize> = Vec::new();
    for i in order {
        let category = &list.categories[i];
        if let Some(subtotal) = category.subtotal {
            for &k in &pending {
                list.categories[k].within = Some(i);
                list.categories[k].hidden |= subtotal.hides;
            }
            pending.clear();
        } else if category.key.is_some() {
            pending.push(i);
        }
    }
}

fn list_of<'l>(lists: &'l HashMap<String, CategoryList>, var: &TermVar) -> &'l CategoryList {
    &lists[&var.key()]
}

/// Every combination of the categories of the lists, the first list varying slowest.
fn combinations(lists: &[&CategoryList]) -> Vec<Vec<usize>> {
    let mut combos = vec![Vec::new()];
    for list in lists {
        combos = combos
            .into_iter()
            .flat_map(|combo| {
                (0..list.categories.len()).map(move |i| {
                    let mut next = combo.clone();
                    next.push(i);
                    next
                })
            })
            .collect();
    }
    combos
}

/// One row or one column of the table.
struct Entry {
    term: usize,
    /// A category index for every categorical variable of the term.
    combo: Vec<usize>,
    slot: Option<usize>,
    path: Vec<String>,
    /// Where the label of the innermost category is in `path`.
    inner_at: Option<usize>,
    /// `path` ends with the label of the summary.
    labelled: bool,
}

/// A row or a column as the table shows it. When /CLABELS moves the categories of one axis to
/// the other, a line of the axis they leave stands for one entry per moved category, and a line
/// of the axis they join shows one moved category of an entry.
struct Line {
    path: Vec<String>,
    /// Indices into the entries of the axis: one, or one per moved category (None where the
    /// term lacks that category).
    entries: Vec<Option<usize>>,
    /// The moved category a line of the joined axis shows.
    moved: Option<usize>,
}

impl Line {
    /// The lines of entries whose labels stay where they are.
    fn plain(entries: &[Entry]) -> Vec<Line> {
        (0..entries.len())
            .map(|i| Line {
                path: entries[i].path.clone(),
                entries: vec![Some(i)],
                moved: None,
            })
            .collect()
    }

    /// The entry this line shows beside a line of the other axis.
    fn entry(&self, other: &Line) -> Option<usize> {
        match other.moved {
            Some(k) => self.entries[k],
            None => self.entries[0],
        }
    }
}

/// Moves the categories of the innermost categorical variable of every term of `from` to `to`
/// (/CLABELS ROWLABELS=OPPOSITE or COLLABELS=OPPOSITE). Each moved category goes inside the
/// categories of a `to` line, just outside its summary label. Every term of `from` must have a
/// categorical variable, and their innermost ones the same categories with the same labels.
fn move_labels(
    from: &[Entry],
    to: &[Entry],
    terms: &[Term],
    lists: &HashMap<String, CategoryList>,
) -> Result<(Vec<Line>, Vec<Line>), Error> {
    let mut labels: Option<(String, Vec<String>)> = None;
    for term in terms {
        let Some(inner) = term.iter().rfind(|v| !v.scale) else {
            return Err(Error(
                "/CLABELS: every variable whose category labels move must be categorical".into(),
            ));
        };
        let these: Vec<String> = list_of(lists, inner)
            .categories
            .iter()
            .filter(|c| !c.hidden)
            .map(|c| c.label.clone())
            .collect();
        match &labels {
            None => labels = Some((inner.name().to_string(), these)),
            Some((first, known)) if *known != these => {
                return Err(Error(format!(
                    "/CLABELS: {first} and {} have different categories; moving category labels needs the same categories with the same labels in every variable",
                    inner.name()
                )));
            }
            Some(_) => {}
        }
    }
    let labels = labels.map(|(_, l)| l).unwrap_or_default();

    let mut lines: Vec<Line> = Vec::new();
    let mut keys: Vec<(usize, Vec<usize>, Option<usize>)> = Vec::new();
    for (i, entry) in from.iter().enumerate() {
        let at = entry.inner_at.expect("a categorical variable");
        let inner = entry.combo.len() - 1;
        let key = (entry.term, entry.combo[..inner].to_vec(), entry.slot);
        let line = match keys.iter().position(|k| *k == key) {
            Some(line) => line,
            None => {
                let mut path = entry.path.clone();
                path.remove(at);
                keys.push(key);
                lines.push(Line {
                    path,
                    entries: vec![None; labels.len()],
                    moved: None,
                });
                lines.len() - 1
            }
        };
        // the position among the shown categories
        let list = list_of(
            lists,
            terms[entry.term]
                .iter()
                .rfind(|v| !v.scale)
                .expect("checked"),
        );
        let shown = list.categories[..entry.combo[inner]]
            .iter()
            .filter(|c| !c.hidden)
            .count();
        lines[line].entries[shown] = Some(i);
    }

    let mut joined = Vec::new();
    let mut start = 0;
    while start < to.len() {
        let same = |e: &Entry| e.term == to[start].term && e.combo == to[start].combo;
        let end = start + to[start..].iter().take_while(|e| same(e)).count();
        for (k, label) in labels.iter().enumerate() {
            for (i, entry) in to.iter().enumerate().take(end).skip(start) {
                let mut path = entry.path.clone();
                let at = path.len() - usize::from(entry.labelled);
                path.insert(at, label.clone());
                joined.push(Line {
                    path,
                    entries: vec![Some(i)],
                    moved: Some(k),
                });
            }
        }
        start = end;
    }
    Ok((lines, joined))
}

#[derive(Clone, Copy, PartialEq)]
enum Axis {
    Rows,
    Columns,
    Layers,
}

struct Layout<'a> {
    rows: Vec<Term<'a>>,
    columns: Vec<Term<'a>>,
    /// One empty term when the table has no layers.
    layers: Vec<Term<'a>>,
}

impl<'a> Layout<'a> {
    fn terms(&self, axis: Axis) -> &[Term<'a>] {
        match axis {
            Axis::Rows => &self.rows,
            Axis::Columns => &self.columns,
            Axis::Layers => &self.layers,
        }
    }

    /// The summaries of a term on the summary axis and the variable they summarize.
    fn summaries<'t>(&self, term: &'t Term<'a>) -> (Vec<Summary>, &'t TermVar<'a>) {
        let default = |statistic| {
            vec![Summary {
                function: SummaryFunction {
                    statistic,
                    unweighted: false,
                },
                percentile: None,
                label: None,
                format: None,
            }]
        };
        if let Some(scale) = term.iter().find(|v| v.scale) {
            let list = scale.syntax.summaries.clone();
            return (list.unwrap_or_else(|| default(Statistic::Mean)), scale);
        }
        let innermost = term.last().expect("a term has a variable");
        let source = mrsets::summary_set(term).unwrap_or(innermost);
        let list = source.syntax.summaries.clone();
        (list.unwrap_or_else(|| default(Statistic::Count)), source)
    }
}

impl Engine {
    pub(crate) fn ctables(&self, command: &Ctables) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let weights = weights(data)?;
        let many = command.tables.len() > 1;
        command
            .tables
            .iter()
            .enumerate()
            .map(|(i, table)| {
                self.ctable(data, command, table, weights.as_deref())
                    .map_err(|Error(message)| {
                        Error(if many {
                            format!("table {}: {message}", i + 1)
                        } else {
                            message
                        })
                    })
            })
            .collect::<Result<Vec<Vec<Table>>, Error>>()
            .map(|tables| tables.into_iter().flatten().collect())
    }

    fn display(&self, command: &Ctables, name: &str) -> Result<VlabelsDisplay, Error> {
        let mut display = VlabelsDisplay::Default;
        for (vars, choice) in &command.vlabels {
            let names = self
                .names_with_sets(vars)
                .map_err(|Error(m)| Error(format!("/VLABELS: {m}")))?;
            if names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
                display = *choice;
            }
        }
        Ok(display)
    }

    fn resolve<'a>(
        &self,
        data: &'a Dataset,
        command: &Ctables,
        expr: &'a AxisExpr,
    ) -> Result<Vec<Term<'a>>, Error> {
        expand(expr)
            .into_iter()
            .map(|term| {
                let resolved = term
                    .into_iter()
                    .map(|syntax| {
                        if syntax.name.starts_with('$') {
                            return self.set_term(data, command, syntax);
                        }
                        let variable = data.variable(&syntax.name).map_err(|_| {
                            Error(format!("/TABLE: unknown variable {}", syntax.name))
                        })?;
                        let scale = syntax.scale.unwrap_or_else(|| match variable.measure() {
                            Measure::Scale => true,
                            Measure::Unknown => guess_scale(&variable),
                            _ => false,
                        });
                        if scale && variable.is_string() {
                            return Err(Error(format!(
                                "/TABLE: {} is a string variable, so it cannot be a scale variable; remove [S]",
                                variable.name()
                            )));
                        }
                        let name = variable.name().to_string();
                        let label = match self.display(command, variable.name())? {
                            VlabelsDisplay::None => None,
                            VlabelsDisplay::Name => Some(name),
                            VlabelsDisplay::Both => Some(match variable.label() {
                                Some(label) => format!("{name} {label}"),
                                None => name,
                            }),
                            _ => Some(variable.label().map_or(name, str::to_string)),
                        };
                        Ok(TermVar {
                            variable,
                            syntax,
                            scale,
                            label,
                            set: None,
                        })
                    })
                    .collect::<Result<Term, Error>>()?;
                Ok(resolved)
            })
            .collect()
    }

    fn ctable(
        &self,
        data: &Dataset,
        command: &Ctables,
        table: &CtablesTable,
        weights: Option<&[f64]>,
    ) -> Result<Vec<Table>, Error> {
        let rows = self.resolve(data, command, &table.rows)?;
        let columns = match &table.columns {
            Some(expr) => self.resolve(data, command, expr)?,
            None => vec![Vec::new()],
        };
        let layers = match &table.layers {
            Some(expr) => self.resolve(data, command, expr)?,
            None => vec![Vec::new()],
        };
        let summary_axis = check_summaries([&rows, &columns, &layers])?;
        let layout = Layout {
            rows,
            columns,
            layers,
        };

        // categories of every categorical variable, by name
        let mut lists: HashMap<String, CategoryList> = HashMap::new();
        let all_terms = layout
            .rows
            .iter()
            .chain(&layout.columns)
            .chain(&layout.layers);
        for var in all_terms.flatten() {
            let name = var.key();
            if var.scale || lists.contains_key(&name) {
                continue;
            }
            let mut spec = None;
            for categories in &table.categories {
                let names = self
                    .names_with_sets(&categories.vars)
                    .map_err(|Error(m)| Error(format!("/CATEGORIES: {m}")))?;
                if names.iter().any(|n| n.to_lowercase() == name) {
                    spec = Some(categories);
                }
            }
            let list = match &var.set {
                Some(set) => mrsets::set_categories(set, spec)?,
                None => categories(var.variable.name(), &[var.variable], spec, Some(command))?,
            };
            lists.insert(name, list);
        }
        let list_of = |var: &TermVar| list_of(&lists, var);

        // the summaries of each term on the summary axis, and the slots they fill
        let summary_terms = layout.terms(summary_axis);
        let mut slots: Vec<Slot> = Vec::new();
        let mut term_summaries = Vec::new();
        for term in summary_terms {
            let (list, variable) = layout.summaries(term);
            let mut indexed = Vec::new();
            for summary in list {
                let s = slot(&summary);
                let index = match slots.iter().position(|known| *known == s) {
                    Some(index) => index,
                    None => {
                        slots.push(s);
                        slots.len() - 1
                    }
                };
                let format = default_format(&summary, &variable.variable);
                indexed.push((index, summary, format));
            }
            term_summaries.push((indexed, variable));
        }

        let label_axis = if table.summaries_in_layers {
            Axis::Layers
        } else if table.summaries_in_rows {
            Axis::Rows
        } else {
            Axis::Columns
        };
        let entries = |axis: Axis| -> Vec<Entry> {
            let mut out = Vec::new();
            for (t, term) in layout.terms(axis).iter().enumerate() {
                let cats: Vec<&CategoryList> =
                    term.iter().filter(|v| !v.scale).map(list_of).collect();
                let term_slots: Vec<Option<usize>> = if axis != label_axis {
                    vec![None]
                } else if axis == summary_axis {
                    term_summaries[t].0.iter().map(|(i, ..)| Some(*i)).collect()
                } else {
                    (0..slots.len()).map(Some).collect()
                };
                for combo in combinations(&cats) {
                    if combo
                        .iter()
                        .zip(&cats)
                        .any(|(&k, l)| l.categories[k].hidden)
                    {
                        continue;
                    }
                    let mut path = Vec::new();
                    let mut c = 0;
                    let mut inner_at = None;
                    for var in term {
                        path.extend(var.label.clone());
                        if !var.scale {
                            inner_at = Some(path.len());
                            path.push(cats[c].categories[combo[c]].label.clone());
                            c += 1;
                        }
                    }
                    for slot in &term_slots {
                        let mut path = path.clone();
                        let labelled = slot.is_some() && table.summary_labels;
                        if let (Some(s), true) = (slot, labelled) {
                            path.push(slots[*s].label.clone());
                        }
                        out.push(Entry {
                            term: t,
                            combo: combo.clone(),
                            slot: *slot,
                            path,
                            inner_at,
                            labelled,
                        });
                    }
                }
            }
            out
        };
        let row_entries = entries(Axis::Rows);
        let column_entries = entries(Axis::Columns);
        let layer_entries = entries(Axis::Layers);

        // gather the cases of every section
        let mut sections: HashMap<(usize, usize, usize), Section> = HashMap::new();
        let tested = table.sigtest.is_some() || !table.comparetests.is_empty();
        for (r, row_term) in layout.rows.iter().enumerate() {
            for (c, column_term) in layout.columns.iter().enumerate() {
                for (l, layer_term) in layout.layers.iter().enumerate() {
                    let summary_term = match summary_axis {
                        Axis::Rows => r,
                        Axis::Columns => c,
                        Axis::Layers => l,
                    };
                    let (summaries, variable) = &term_summaries[summary_term];
                    let section = Section::gather(
                        data,
                        weights,
                        [row_term, column_term, layer_term],
                        &lists,
                        summaries,
                        variable,
                        (tested, table.labels_moved),
                    )?;
                    sections.insert((r, c, l), section);
                }
            }
        }

        // the rows and columns as shown, with category labels moved by /CLABELS
        let (row_lines, column_lines) = match table.labels_moved {
            LabelsMoved::No => (Line::plain(&row_entries), Line::plain(&column_entries)),
            LabelsMoved::RowsToColumns => {
                move_labels(&row_entries, &column_entries, &layout.rows, &lists)?
            }
            LabelsMoved::ColumnsToRows => {
                let (columns, rows) =
                    move_labels(&column_entries, &row_entries, &layout.columns, &lists)?;
                (rows, columns)
            }
        };
        if table.labels_moved != LabelsMoved::No && tested {
            return Err(Error(
                "/CLABELS: significance tests are not available when category labels are moved; leave out CLABELS or the tests".into(),
            ));
        }
        // categories an HSUBTOTAL hides are tested through their subtotal
        let hides = lists.values().any(|l| {
            l.categories.iter().any(|c| {
                let collapsed = c
                    .within
                    .is_some_and(|s| l.categories[s].subtotal.is_some_and(|t| t.hides));
                c.hidden && !collapsed
            })
        });
        if hides && tested {
            return Err(Error(
                "/PPROPERTIES: HIDESOURCECATS=YES cannot go with significance tests; leave out one of them".into(),
            ));
        }

        let row_headers = row_lines
            .iter()
            .map(|e| e.path.len())
            .max()
            .unwrap_or(0)
            .max(1);
        let levels = column_lines
            .iter()
            .map(|e| e.path.len())
            .max()
            .unwrap_or(0)
            .max(1);
        let level = |path: &[String], i: usize| path.get(i).cloned().unwrap_or_default();
        let mut columns = vec![String::new(); row_headers];
        let mut column_groups = vec![vec![String::new(); levels - 1]; row_headers];
        for line in &column_lines {
            columns.push(level(&line.path, levels - 1));
            column_groups.push((0..levels - 1).map(|i| level(&line.path, i)).collect());
        }
        if let Some(corner) = &table.corner {
            match column_groups[0].first_mut() {
                Some(top) => *top = corner.clone(),
                None => columns[0] = corner.clone(),
            }
        }

        // one table for every layer, each followed by its tests
        let title = table.title.as_deref().unwrap_or("Custom Tables");
        let mut out = Vec::new();
        let mut tested_layers: Vec<(usize, &[usize])> = Vec::new();
        for layer in &layer_entries {
            let mut out_rows = Vec::new();
            let mut formats = Vec::new();
            for row_line in &row_lines {
                let mut cells: Vec<Cell> = (0..row_headers)
                    .map(|i| match row_line.path.get(i) {
                        Some(text) => Cell::Text(text.clone()),
                        None => Cell::Empty,
                    })
                    .collect();
                let mut cell_formats = vec![String::new(); row_headers];
                for column_line in &column_lines {
                    let picked = row_line.entry(column_line).zip(column_line.entry(row_line));
                    let Some((r, c)) = picked else {
                        cells.push(Cell::Empty);
                        cell_formats.push(String::new());
                        continue;
                    };
                    let (row, column) = (&row_entries[r], &column_entries[c]);
                    let section = &sections[&(row.term, column.term, layer.term)];
                    let slot = row.slot.or(column.slot).or(layer.slot).unwrap_or(0);
                    let key = [&row.combo[..], &column.combo, &layer.combo].concat();
                    let (cell, format) = section.cell(&key, slot, command);
                    cells.push(cell);
                    cell_formats.push(format);
                }
                out_rows.push(cells);
                formats.push(cell_formats);
            }
            let layer_title = |title: &str| match layer_name(
                &layout.layers[layer.term],
                &lists,
                layer,
                &slots,
                table.summary_labels,
            ) {
                Some(name) => format!("{title} ({name})"),
                None => title.to_string(),
            };
            let mut main = Table {
                title: layer_title(title),
                row_headers,
                columns: columns.clone(),
                column_groups: column_groups.clone(),
                rows: out_rows,
                formats,
                caption: table.caption.clone(),
            };
            // the tests do not depend on the summary of a layer, so they run once per category
            let tests = if tested_layers.contains(&(layer.term, &layer.combo[..])) {
                Vec::new()
            } else {
                tested_layers.push((layer.term, &layer.combo));
                let in_layer: HashMap<(usize, usize), &Section> = sections
                    .iter()
                    .filter(|((_, _, l), _)| *l == layer.term)
                    .map(|((r, c, _), section)| ((*r, *c), section))
                    .collect();
                let (tests, merged) =
                    sigtest::tables(table, &layout, &lists, &in_layer, &layer.combo)?;
                for (spec, found) in &merged {
                    sigtest::merge(
                        &mut main,
                        spec,
                        found,
                        &row_entries,
                        &column_entries,
                        &slots,
                    );
                }
                tests
            };
            out.push(main);
            out.extend(tests.into_iter().map(|mut test| {
                test.title = layer_title(&test.title);
                test
            }));
        }
        Ok(out)
    }
}

/// The layer of a table in words, such as "Gender: Male, Region: North", for its title; None
/// when the table has no layers.
fn layer_name(
    term: &Term,
    lists: &HashMap<String, CategoryList>,
    layer: &Entry,
    slots: &[Slot],
    summary_labels: bool,
) -> Option<String> {
    let mut parts = Vec::new();
    let mut c = 0;
    for var in term {
        if var.scale {
            parts.extend(var.label.clone());
            continue;
        }
        let category = &list_of(lists, var).categories[layer.combo[c]].label;
        c += 1;
        parts.push(match &var.label {
            Some(label) => format!("{label}: {category}"),
            None => category.clone(),
        });
    }
    if let (Some(s), true) = (layer.slot, summary_labels) {
        parts.push(slots[s].label.clone());
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// Which axis carries the summaries: the one with a scale variable or with summaries written
/// in the syntax. Also checks where summaries may appear.
fn check_summaries(axes: [&[Term]; 3]) -> Result<Axis, Error> {
    let mut found: [Option<String>; 3] = [None, None, None];
    for (axis, terms) in axes.iter().enumerate() {
        for term in terms.iter() {
            let scales: Vec<&TermVar> = term.iter().filter(|v| v.scale).collect();
            if scales.len() > 1 {
                return Err(Error(format!(
                    "/TABLE: {} and {} are both scale variables in one nesting; a cell summarizes one scale variable, so stack them with + instead",
                    scales[0].name(),
                    scales[1].name()
                )));
            }
            let innermost = term.iter().rposition(|v| !v.scale);
            for (i, var) in term.iter().enumerate() {
                let name = var.name();
                if var.scale {
                    found[axis] = Some(name.to_string());
                }
                let Some(summaries) = &var.syntax.summaries else {
                    continue;
                };
                found[axis] = Some(name.to_string());
                mrsets::check_functions(var, summaries)?;
                if var.scale {
                    continue;
                }
                if !scales.is_empty() {
                    return Err(Error(format!(
                        "/TABLE: {name} has summaries, but the scale variable {} in the same nesting is the one summarized; move the summaries to it",
                        scales[0].name()
                    )));
                }
                let set_carries = mrsets::summary_set(term).is_some_and(|s| std::ptr::eq(s, var));
                if Some(i) != innermost && !set_carries {
                    return Err(Error(format!(
                        "/TABLE: summaries may follow only the innermost categorical variable of a nesting, or a multiple response set with no other set or summaries inside it, and {name} is an outer one"
                    )));
                }
                if let Some(s) = summaries
                    .iter()
                    .find(|s| scale_only(s.function) && !mrsets::counts_responses(s.function))
                {
                    return Err(Error(format!(
                        "/TABLE: summary function {} applies only to scale variables, and {name} is categorical; write {name} [S] to treat it as scale",
                        s.function.name()
                    )));
                }
            }
        }
    }
    const NAMES: [&str; 3] = ["rows", "columns", "layers"];
    let with: Vec<(usize, &String)> = (0..3)
        .filter_map(|a| found[a].as_ref().map(|name| (a, name)))
        .collect();
    match with[..] {
        [(a, first), (b, second), ..] => Err(Error(format!(
            "/TABLE: summaries may appear on one axis only, but the {} have {first} and the {} have {second} (a scale variable always has a summary)",
            NAMES[a], NAMES[b]
        ))),
        [(1, _)] => Ok(Axis::Columns),
        [(2, _)] => Ok(Axis::Layers),
        _ => Ok(Axis::Rows),
    }
}

/// The gathered cases of one row term crossed with one column term and one layer term. A key
/// lists the categories of the rows, then the columns, then the layers.
struct Section {
    /// For every categorical variable: true when its category of a cell decides the area of
    /// each kind of percentage. Index by `area`.
    areas: [Vec<bool>; 7],
    /// The total category of each categorical variable.
    totals: Vec<Option<usize>>,
    /// The computed categories of each categorical variable, by category index.
    computed: Vec<Vec<(usize, pcompute::Computed)>>,
    /// The cells by category index, totals included. For the significance tests the innermost
    /// row and column variables also have the index MARGIN: every case of the other
    /// categories once, whatever its categories of that variable.
    cells: HashMap<Vec<usize>, Tally>,
    denominators: [HashMap<Vec<usize>, Tally>; 7],
    /// For the tests, when the innermost column variable is a set: the cases two of its
    /// categories share, by the categories of the other variables (rows, outer columns,
    /// layers) and then the two categories.
    pairs: HashMap<Vec<usize>, Tally>,
    summaries: Vec<(usize, Summary, String)>,
    has_scale: bool,
}

/// The category index of a margin cell, see `Section::cells`.
const MARGIN: usize = usize::MAX;

fn area(base: PercentBase) -> usize {
    base as usize
}

impl Section {
    fn gather<'a>(
        data: &Dataset,
        weights: Option<&[f64]>,
        terms: [&[TermVar<'a>]; 3],
        lists: &HashMap<String, CategoryList>,
        summaries: &[(usize, Summary, String)],
        summary_variable: &TermVar<'a>,
        (margins, moved): (bool, LabelsMoved),
    ) -> Result<Section, Error> {
        let [nr, nc, _] = terms.map(|term| term.iter().filter(|v| !v.scale).count());
        // the categorical variables of the rows, then the columns, then the layers
        let cats: Vec<&TermVar> = terms
            .iter()
            .flat_map(|t| t.iter())
            .filter(|v| !v.scale)
            .collect();
        let lists: Vec<&CategoryList> = cats.iter().map(|v| list_of(lists, v)).collect();
        let scale = terms.iter().flat_map(|t| t.iter()).find(|v| v.scale);

        // the axis of every variable and whether it is the innermost of its axis; a variable
        // whose labels /CLABELS moves counts as the innermost of the axis it joins
        let mut axis: Vec<Axis> = (0..cats.len())
            .map(|i| match i {
                i if i < nr => Axis::Rows,
                i if i < nr + nc => Axis::Columns,
                _ => Axis::Layers,
            })
            .collect();
        let (from, to) = match moved {
            LabelsMoved::RowsToColumns => (nr.checked_sub(1), Axis::Columns),
            LabelsMoved::ColumnsToRows => ((nc > 0).then(|| nr + nc - 1), Axis::Rows),
            LabelsMoved::No => (None, Axis::Rows),
        };
        if let Some(m) = from {
            axis[m] = to;
        }
        let innermost = |i: usize| {
            let last = (0..cats.len()).rfind(|&j| axis[j] == axis[i] && Some(j) != from);
            match (from, axis[i]) {
                (Some(m), a) if a == to => i == m,
                (_, Axis::Layers) => false,
                _ => Some(i) == last,
            }
        };
        // which variables decide each area, in PercentBase order; a layer variable decides
        // every area but the table's, as an outer variable does
        let is_row = |i: usize| axis[i] == Axis::Rows;
        let is_column = |i: usize| axis[i] == Axis::Columns;
        let is_layer = |i: usize| axis[i] == Axis::Layers;
        let outer = |i: usize| !innermost(i);
        let decides = |f: &dyn Fn(usize) -> bool| (0..cats.len()).map(f).collect::<Vec<bool>>();
        let areas = [
            decides(&|i| is_row(i) || outer(i)),
            decides(&|i| is_column(i) || outer(i)),
            decides(&|_| false),
            decides(&outer),
            decides(&is_layer),
            decides(&|i| is_row(i) || is_layer(i)),
            decides(&|i| is_column(i) || is_layer(i)),
        ];

        let summary_list = summary_variable_list(&cats, &lists, summary_variable);
        let mut section = Section {
            areas,
            totals: lists.iter().map(|l| l.total()).collect(),
            computed: lists
                .iter()
                .map(|l| {
                    let found = l.categories.iter().enumerate();
                    found
                        .filter_map(|(i, c)| c.computed.clone().map(|c| (i, c)))
                        .collect()
                })
                .collect(),
            cells: HashMap::new(),
            denominators: Default::default(),
            pairs: HashMap::new(),
            summaries: summaries.to_vec(),
            has_scale: scale.is_some(),
        };
        let keep_values = scale.is_some();
        'cases: for case in 0..data.case_count() {
            // the categories of the case: one per variable, any number for a set
            let mut choices = Vec::with_capacity(cats.len());
            for (var, list) in cats.iter().zip(&lists) {
                match mrsets::choice(var, list, case) {
                    Some(choice) => choices.push(choice),
                    None => continue 'cases,
                }
            }
            let weight = weights.map_or(1.0, |w| w[case]);
            // a case that weighs nothing is left out, also from the unweighted counts
            if weight <= 0.0 {
                continue;
            }
            let value = match scale {
                Some(scale) => scale
                    .variable
                    .numeric()?
                    .get(case)
                    .filter(|v| !scale.variable.is_user_missing(*v)),
                None => None,
            };
            // whether the case is valid, and its responses, in one category of the summary
            // variable, or in all of them for None
            let judge = |of: Option<usize>| match (scale, summary_list) {
                (Some(_), _) => (value.is_some(), 1.0),
                (None, Some((i, list))) => choices[i]
                    .iter()
                    .filter(|(c, _)| of.is_none_or(|of| list.covers(of, *c)))
                    .fold((false, 0.0), |(valid, sum), (c, responses)| {
                        (valid || !list.categories[*c].missing, sum + responses)
                    }),
                (None, None) => (true, 1.0),
            };
            let total_only = choices.iter().any(Vec::is_empty);
            // the cells of every variable the case adds to: its categories, the subtotals that
            // sum them and the total, each once
            let expanded: Vec<Vec<usize>> = choices
                .iter()
                .zip(&lists)
                .zip(&section.totals)
                .map(|((choice, list), total)| {
                    let mut part: Vec<usize> = Vec::new();
                    for k in choice.iter().flat_map(|(c, _)| list.with_subtotal(*c)) {
                        if !part.contains(&k) {
                            part.push(k);
                        }
                    }
                    part.extend(*total);
                    part
                })
                .collect();
            // every area of the case once; the total or a subtotal of a deciding variable is an
            // area too, the one of a total or subtotal row or column
            for (a, decides) in section.areas.iter().enumerate() {
                let parts: Vec<Vec<usize>> = expanded
                    .iter()
                    .zip(decides)
                    .filter(|(_, d)| **d)
                    .map(|(part, _)| part.clone())
                    .collect();
                let at = summary_list
                    .filter(|(i, _)| decides[*i])
                    .map(|(i, _)| (i, decides[..i].iter().filter(|d| **d).count()));
                for area_key in mrsets::product(&parts) {
                    let of = at.and_then(|(i, at)| {
                        (section.totals[i] != Some(area_key[at])).then_some(area_key[at])
                    });
                    let stats = judge(of);
                    section.denominators[a]
                        .entry(area_key)
                        .or_default()
                        .add_case(weight, stats, value, false, total_only);
                }
            }
            // the cells of the categories, and of the totals and subtotals in their place
            let mut parts = expanded;
            if margins {
                for i in [nr.checked_sub(1), (nc > 0).then(|| nr + nc - 1)]
                    .into_iter()
                    .flatten()
                {
                    parts[i].push(MARGIN);
                }
            }
            for target in mrsets::product(&parts) {
                let of = summary_list.and_then(|(i, _)| {
                    (target[i] != MARGIN && section.totals[i] != Some(target[i]))
                        .then_some(target[i])
                });
                section.cells.entry(target).or_default().add_case(
                    weight,
                    judge(of),
                    value,
                    keep_values,
                    total_only,
                );
            }
            // the cases two categories of a set in the innermost column share, by the
            // categories of the other variables
            let inner_set = nc > 0 && cats[nr + nc - 1].set.is_some();
            if margins && inner_set && !total_only {
                let total = section.totals[nr + nc - 1];
                let inner: Vec<usize> = parts[nr + nc - 1]
                    .iter()
                    .copied()
                    .filter(|&k| k != MARGIN && Some(k) != total)
                    .collect();
                let mut others = parts.clone();
                others.remove(nr + nc - 1);
                let shared: Vec<Vec<usize>> = mrsets::product(&others);
                for (n, &a) in inner.iter().enumerate() {
                    for &b in &inner[n + 1..] {
                        let pair = [a.min(b), a.max(b)];
                        for prefix in &shared {
                            let key = [&prefix[..], &pair[..]].concat();
                            section.pairs.entry(key).or_default().add_case(
                                weight,
                                (value.is_some(), 1.0),
                                value,
                                false,
                                false,
                            );
                        }
                    }
                }
            }
        }
        Ok(section)
    }

    /// The first computed category of a key, by position in the key.
    fn computed_at(&self, key: &[usize]) -> Option<(usize, &pcompute::Computed)> {
        key.iter().enumerate().find_map(|(i, k)| {
            self.computed[i]
                .iter()
                .find(|(c, _)| c == k)
                .map(|(_, computed)| (i, computed))
        })
    }

    /// The tally of a cell; for a cell without cases an empty one where a count is 0 (EMPTY
    /// decides how it shows), as is a percentage beside categorical variables only. Other
    /// statistics have no value there.
    fn tally<'t>(
        &'t self,
        key: &[usize],
        summary: &Summary,
        empty: &'t Tally,
    ) -> Option<&'t Tally> {
        let counting = matches!(
            summary.function.statistic,
            Statistic::Count
                | Statistic::Validn
                | Statistic::Totaln
                | Statistic::Missing
                | Statistic::Responses
        );
        match self.cells.get(key) {
            Some(tally) => Some(tally),
            None if counting || !self.has_scale => Some(empty),
            None => None,
        }
    }

    /// The cell at `key` for the summary in `slot`, with its display format.
    fn cell(&self, key: &[usize], slot: usize, command: &Ctables) -> (Cell, String) {
        let Some((_, summary, format)) = self.summaries.iter().find(|(s, ..)| *s == slot) else {
            return (Cell::Empty, String::new());
        };
        if let Some((_, computed)) = self.computed_at(key) {
            let format = computed
                .formats
                .iter()
                .find(|(f, _)| *f == summary.function)
                .map_or_else(|| format.clone(), |(_, f)| f.clone());
            let cell = match (self.value(key, summary), &command.missing) {
                (Some(v), _) => Cell::Number(v),
                (None, Some(text)) => Cell::Text(text.clone()),
                (None, None) => Cell::Empty,
            };
            return (cell, format);
        }
        let empty = Tally::default();
        let Some(tally) = self.tally(key, summary, &empty) else {
            return (Cell::Empty, format.clone());
        };
        if tally.cases == 0.0 {
            match &command.empty {
                EmptyCells::Zero => {}
                EmptyCells::Blank => return (Cell::Empty, format.clone()),
                EmptyCells::Text(text) => return (Cell::Text(text.clone()), format.clone()),
            }
        }
        let value = self.value(key, summary);
        // HIDESMALLCOUNTS hides the summaries shown in the count format, weighted or not
        let count_format = matches!(
            summary.function.statistic,
            Statistic::Count | Statistic::Validn | Statistic::Totaln | Statistic::Responses
        );
        let cell = match (value, &command.missing, command.hide_small_counts) {
            (Some(v), _, Some(n)) if count_format && v < f64::from(n) => {
                Cell::Text(format!("<{n}"))
            }
            (Some(v), ..) => Cell::Number(v),
            (None, Some(text), _) => Cell::Text(text.clone()),
            (None, None, _) => Cell::Empty,
        };
        (cell, format.clone())
    }

    /// The value of a summary in a cell. A computed category takes it from the same summary in
    /// the categories its expression uses.
    fn value(&self, key: &[usize], summary: &Summary) -> Option<f64> {
        if let Some((i, computed)) = self.computed_at(key) {
            return computed.calc.eval(&|k| {
                let mut at = key.to_vec();
                at[i] = k;
                self.value(&at, summary)
            });
        }
        let empty = Tally::default();
        let tally = self.tally(key, summary, &empty)?;
        match summary.function.statistic {
            Statistic::Percent(base, of) => {
                // a total of a deciding variable is the area of all its categories
                let area_key: Vec<usize> = key
                    .iter()
                    .zip(&self.areas[area(base)])
                    .filter(|(_, d)| **d)
                    .map(|(k, _)| *k)
                    .collect();
                let unweighted = summary.function.unweighted;
                let (numerator, denominator) = mrsets::parts(of);
                let whole = self.denominators[area(base)]
                    .get(&area_key)
                    .map_or(0.0, |t| t.base(denominator, unweighted));
                let part = tally.base(numerator, unweighted);
                (whole != 0.0).then(|| 100.0 * part / whole)
            }
            _ => statistic(summary, tally),
        }
    }
}

/// The position and categories of the categorical summary variable among the section's
/// categorical variables, for telling valid cases from cases with a missing category.
fn summary_variable_list<'l>(
    cats: &[&TermVar],
    lists: &[&'l CategoryList],
    summary_variable: &TermVar,
) -> Option<(usize, &'l CategoryList)> {
    cats.iter()
        .position(|v| v.key() == summary_variable.key())
        .map(|i| (i, lists[i]))
}
