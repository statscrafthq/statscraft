//! AGGREGATE: one value per break group for every target variable.

use crate::variables::{Column, duplicate, new_names};
use crate::{Engine, Error};
use statscraft_data::{Dataset, Values, Variable};
use statscraft_syntax::{Aggregate, AggregateFunction as F, Aggregation};
use std::cmp::Ordering;

/// A target variable with one value per group.
struct Target {
    name: String,
    label: Option<String>,
    /// The source variable whose dictionary entry the target takes over (MIN, MAX, FIRST,
    /// LAST), or the format of a computed result.
    like: Result<String, &'static str>,
    column: Column,
}

fn name(function: F) -> &'static str {
    match function {
        F::Sum => "SUM",
        F::Mean => "MEAN",
        F::Sd => "SD",
        F::Median => "MEDIAN",
        F::Min => "MIN",
        F::Max => "MAX",
        F::First => "FIRST",
        F::Last => "LAST",
        F::N => "N",
        F::Nu => "NU",
        F::Nmiss => "NMISS",
        F::Numiss => "NUMISS",
    }
}

/// N, NU, NMISS and NUMISS of one group. `valid` tells whether the source value of a case
/// counts; NU and NUMISS count cases, the other two add up weights.
fn count(
    function: F,
    group: &[usize],
    valid: impl Fn(usize) -> bool,
    weight: impl Fn(usize) -> f64,
) -> f64 {
    let want = matches!(function, F::N | F::Nu);
    let unweighted = matches!(function, F::Nu | F::Numiss);
    let cases = group.iter().filter(|&&case| valid(case) == want);
    // not sum(): that gives -0 for no cases
    cases
        .map(|&case| if unweighted { 1.0 } else { weight(case) })
        .fold(0.0, |total, weight| total + weight)
}

/// MIN, MAX, FIRST and LAST of the valid values of one group. Weights play no part.
fn pick<T: PartialOrd>(function: F, mut values: impl Iterator<Item = T>) -> Option<T> {
    match function {
        F::First => values.next(),
        F::Last => values.last(),
        F::Min => values.reduce(|a, b| if b < a { b } else { a }),
        _ => values.reduce(|a, b| if b > a { b } else { a }),
    }
}

/// SUM, MEAN, SD and MEDIAN of the valid (value, weight) pairs of one group. A result too
/// large for a number is system-missing.
fn statistic(function: F, cells: &[(f64, f64)]) -> Option<f64> {
    let total: f64 = cells.iter().map(|(_, w)| w).sum();
    let sum: f64 = cells.iter().map(|(x, w)| x * w).sum();
    let result = match function {
        F::Sum => (!cells.is_empty()).then_some(sum),
        F::Mean => (total > 0.0).then(|| sum / total),
        F::Sd => (total > 1.0).then(|| {
            // the mean is updated case by case: equal values then give exactly 0, where
            // subtracting a mean computed from the sums leaves rounding noise
            let (mut seen, mut mean, mut squares) = (0.0, 0.0, 0.0);
            for &(x, w) in cells.iter().filter(|(_, w)| *w > 0.0) {
                seen += w;
                let delta = x - mean;
                mean += delta * (w / seen);
                squares += w * delta * (x - mean);
            }
            (squares / (total - 1.0)).sqrt()
        }),
        _ => median(cells),
    };
    result.filter(|value| value.is_finite())
}

/// The weighted median: the value at position (W + 1) / 2 of the cumulated weights,
/// interpolated between the two neighbouring values.
fn median(cells: &[(f64, f64)]) -> Option<f64> {
    let mut cells: Vec<(f64, f64)> = cells.iter().copied().filter(|(_, w)| *w > 0.0).collect();
    cells.sort_by(|a, b| a.0.total_cmp(&b.0));
    cells.dedup_by(|next, kept| {
        let same = next.0 == kept.0;
        if same {
            kept.1 += next.1;
        }
        same
    });
    let total: f64 = cells.iter().map(|(_, w)| w).sum();
    let position = (total + 1.0) / 2.0;
    let mut below = 0.0;
    let mut previous = None;
    for (value, weight) in cells {
        if position < below + weight {
            let Some(previous) = previous else {
                return Some(value);
            };
            let step = position - below;
            let share = if step >= 1.0 {
                1.0
            } else if weight >= 1.0 {
                step
            } else {
                step / weight
            };
            return Some((1.0 - share) * previous + share * value);
        }
        below += weight;
        previous = Some(value);
    }
    previous
}

/// The values of one aggregation for every group, and what the target takes its format from.
fn aggregated(
    function: F,
    source: Option<&Variable>,
    groups: &[Vec<usize>],
    weights: Option<&[f64]>,
) -> Result<(Column, Result<String, &'static str>), Error> {
    let weight = |case: usize| weights.map_or(1.0, |w| w[case]);
    let counting = matches!(function, F::N | F::Nu | F::Nmiss | F::Numiss);
    let picking = matches!(function, F::Min | F::Max | F::First | F::Last);
    let like = match source {
        Some(source) if picking => Ok(source.name().to_string()),
        _ if !counting => Err("F8.2"),
        _ if weights.is_some() && matches!(function, F::N | F::Nmiss) => Err("F8.2"),
        _ => Err("F7.0"),
    };
    let numbers = |value: &dyn Fn(&[usize]) -> Option<f64>| {
        Column::Numeric(groups.iter().map(|group| value(group)).collect())
    };
    let column = match source.map(|source| (source, source.values())) {
        None => numbers(&|group| Some(count(function, group, |_| true, weight))),
        Some((source, Values::Numeric(values))) => {
            let valid = |case: usize| values.get(case).filter(|v| !source.is_user_missing(*v));
            numbers(&|group| {
                if counting {
                    Some(count(function, group, |case| valid(case).is_some(), weight))
                } else if picking {
                    pick(function, group.iter().filter_map(|&case| valid(case)))
                } else {
                    let cells: Vec<(f64, f64)> = group
                        .iter()
                        .filter_map(|&case| Some((valid(case)?, weight(case))))
                        .collect();
                    statistic(function, &cells)
                }
            })
        }
        Some((source, Values::String(values))) => {
            let valid =
                |case: usize| Some(values.get(case)).filter(|v| !source.is_user_missing_str(v));
            if counting {
                numbers(&|group| Some(count(function, group, |case| valid(case).is_some(), weight)))
            } else if picking {
                let picked = |group: &Vec<usize>| {
                    let valid_values = group.iter().filter_map(|&case| valid(case));
                    pick(function, valid_values).unwrap_or("").to_string()
                };
                let width = source.string_width().unwrap_or(1);
                Column::String(width, groups.iter().map(picked).collect())
            } else {
                return Err(Error(format!(
                    "{} needs a numeric variable, {} is a string variable",
                    name(function),
                    source.name()
                )));
            }
        }
    };
    Ok((column, like))
}

/// Adds the target to `out` under `name`: one value per group, or one per case when
/// `group_of` gives the group of every case.
fn add(
    out: &mut Dataset,
    name: &str,
    target: &Target,
    group_of: Option<&[usize]>,
    original: &Dataset,
) -> Result<(), Error> {
    fn spread<T: Clone>(values: &[T], group_of: Option<&[usize]>) -> Vec<T> {
        match group_of {
            Some(group_of) => group_of.iter().map(|&g| values[g].clone()).collect(),
            None => values.to_vec(),
        }
    }
    match &target.column {
        Column::Numeric(values) => out.set_numeric_values(name, spread(values, group_of))?,
        Column::String(width, values) => {
            out.set_string_values(name, *width, spread(values, group_of))?
        }
    }
    match &target.like {
        Ok(source) => {
            let source = original.variable(source)?;
            out.set_variable_label(name, source.label())?;
            if !source.is_string() {
                out.set_format(name, source.format())?;
            }
            out.set_value_labels(name, source.value_labels().to_vec())?;
            out.set_missing_values(name, source.missing_values().clone())?;
            out.set_measure(name, source.measure())?;
        }
        Err(format) => out.set_format(name, format)?,
    }
    if let Some(label) = &target.label {
        out.set_variable_label(name, Some(label))?;
    }
    Ok(())
}

impl Engine {
    pub(crate) fn aggregate(&mut self, command: &Aggregate) -> Result<(), Error> {
        let result = self.aggregate_dataset(command)?;
        match &command.outfile {
            None => self.dataset = Some(result),
            Some(path) if !self.dry_run => result.write_sav(path)?,
            Some(_) => {}
        }
        Ok(())
    }

    fn targets(
        &self,
        aggregation: &Aggregation,
        groups: &[Vec<usize>],
        weights: Option<&[f64]>,
    ) -> Result<Vec<Target>, Error> {
        let data = self.active()?;
        let function = aggregation.function;
        let mut names = Vec::new();
        for (item, label) in &aggregation.targets {
            let mut expanded = new_names(std::slice::from_ref(item))?;
            let last = expanded.pop();
            names.extend(expanded.into_iter().map(|name| (name, None)));
            names.extend(last.map(|name| (name, label.clone())));
        }
        let sources = self.expand(&aggregation.sources)?;
        if !aggregation.sources.is_empty() && sources.len() != names.len() {
            let targets: Vec<&str> = names.iter().map(|(name, _)| name.as_str()).collect();
            return Err(Error(format!(
                "{} target variables ({}) but {} source variables in {}({}); give one target per source",
                names.len(),
                targets.join(" "),
                sources.len(),
                name(function),
                sources.join(" ")
            )));
        }
        let mut targets = Vec::new();
        for (i, (name, label)) in names.into_iter().enumerate() {
            let source = sources.get(i).map(|s| data.variable(s)).transpose()?;
            let (column, like) = aggregated(function, source.as_ref(), groups, weights)?;
            targets.push(Target {
                name,
                label,
                like,
                column,
            });
        }
        Ok(targets)
    }

    /// The dataset the command produces: the active one with the targets added, or one case
    /// per break group.
    fn aggregate_dataset(&self, command: &Aggregate) -> Result<Dataset, Error> {
        let data = self.active()?;
        if command.add_variables && command.outfile.is_some() {
            return Err(Error(
                "MODE=ADDVARIABLES adds to the active dataset, so it needs OUTFILE=* instead of a file".into(),
            ));
        }
        let keys = self.sort_keys(&command.breaks)?;
        let order = keys.order(data.case_count());
        // groups in break order, the cases of a group in file order
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut starts_group = Vec::with_capacity(order.len());
        for (i, &case) in order.iter().enumerate() {
            let starts = i == 0 || keys.compare(order[i - 1], case) != Ordering::Equal;
            match groups.last_mut() {
                Some(group) if !starts => group.push(case),
                _ => groups.push(vec![case]),
            }
            starts_group.push(starts);
        }

        // a case whose weight is missing, zero or negative weighs nothing
        let weights = match data.weight_variable() {
            Some(name) => {
                let variable = data.variable(name)?;
                let usable = |v: Option<f64>| v.filter(|v| !variable.is_user_missing(*v));
                let values = variable.numeric()?.iter();
                Some(
                    values
                        .map(|v| usable(v).unwrap_or(0.0).max(0.0))
                        .collect::<Vec<f64>>(),
                )
            }
            None => None,
        };

        let mut targets = Vec::new();
        for aggregation in &command.aggregations {
            targets.extend(self.targets(aggregation, &groups, weights.as_deref())?);
        }
        let names: Vec<String> = targets.iter().map(|t| t.name.clone()).collect();
        if let Some(name) = duplicate(&names) {
            return Err(Error(format!("the target name {name} is used twice")));
        }
        let same = |a: &str, b: &str| a.to_lowercase() == b.to_lowercase();
        if let Some(name) = names
            .iter()
            .find(|name| keys.names.iter().any(|key| same(key, name)))
        {
            return Err(Error(format!(
                "the target {name} has the name of a break variable; choose another name"
            )));
        }

        let mut out = data.clone();
        if command.add_variables {
            if let Some(name) = names.iter().find(|n| data.variable_index(n).is_some()) {
                return Err(Error(format!(
                    "the target {name} already exists in the dataset; choose a new name or delete the variable first"
                )));
            }
            let mut group_of = vec![0; data.case_count()];
            for (number, group) in groups.iter().enumerate() {
                for &case in group {
                    group_of[case] = number;
                }
            }
            for target in &targets {
                add(&mut out, &target.name, target, Some(&group_of), data)?;
            }
            return Ok(out);
        }

        out.reorder_cases(&order)?;
        out.select_cases(&starts_group)?;
        // A target may take the name of a variable that is about to go, so the targets are
        // added under names nothing else uses and renamed once the other variables are gone.
        let parked: Vec<String> = (0..targets.len())
            .map(|i| format!("@sc.aggregating.{i}"))
            .collect();
        for (target, parked) in targets.iter().zip(&parked) {
            add(&mut out, parked, target, None, data)?;
        }
        let kept: Vec<&str> = keys
            .names
            .iter()
            .chain(&parked)
            .map(String::as_str)
            .collect();
        out.keep_variables(&kept)?;
        for (parked, name) in parked.iter().zip(&names) {
            out.rename_variable(parked, name)?;
        }
        Ok(out)
    }
}
