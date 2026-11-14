//! PROXIMITIES: similarities, dissimilarities and distances between cases or variables.
//!
//! Rules from the Command Syntax Reference and the Algorithms manual (PROXIMITIES):
//! - A case missing on any variable is left out, for cases and for variables as items
//!   (INCLUDE keeps user-missing values). The weight variable is not used.
//! - STANDARDIZE works on each variable (default) or each case, over the cases kept:
//!   Z subtracts the mean and divides by the standard deviation (all 0 when that is 0); SD only
//!   divides by it (unchanged when 0); RANGE divides by the range (unchanged when 0); RESCALE
//!   maps the smallest value to 0 and the largest to 1 (all 0.5 when the range is 0); MAX divides
//!   by the largest value, and when that is 0 divides by the size of the smallest and adds 1;
//!   MEAN divides by the mean, and adds 1 when the mean is 0. The standard deviation has n - 1
//!   in the denominator.
//! - The measures are those of CLUSTER (`cluster::proximity`), EUCLID by default.
//! - ABSOLUTE, REVERSE (change of sign, a similarity becomes a dissimilarity) and RESCALE
//!   (smallest proximity to 0, largest to 1) apply in that order to the whole matrix, the
//!   diagonal included.
//! - MATRIX=OUT writes ROWTYPE_ (PROX, its value label the measure and SIMILARITY or
//!   DISSIMILARITY), the ID variable (cases with ID), CASENO_ (cases), VARNAME_ and one variable
//!   per item: VAR1, VAR2, ... labeled CASE m for cases, the variables themselves for
//!   VIEW=VARIABLE. With a split each group adds its rows after the split variables.
//! - MATRIX=IN reads such a matrix; a measure is not computed from it, only the
//!   transformations apply.

use crate::cluster::{
    Items, is_similarity, measure_name, proximity, proximity_table, summary_table, value_text,
};
use crate::means::is_valid;
use crate::{Engine, Error, Table};
use statscraft_data::{Dataset, Value, Values, Variable};
use statscraft_syntax::{Proximities, Standardize, VarItem};

/// The values standardized in place by `method`.
pub(crate) fn standardize(values: &mut [f64], method: Standardize) {
    let n = values.len() as f64;
    if values.is_empty() {
        return;
    }
    let mean = values.iter().sum::<f64>() / n;
    let sd = match values.len() {
        1 => 0.0,
        _ => (values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt(),
    };
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = max - min;
    let map =
        |f: &dyn Fn(f64) -> f64, values: &mut [f64]| values.iter_mut().for_each(|v| *v = f(*v));
    match method {
        Standardize::Z if sd == 0.0 => map(&|_| 0.0, values),
        Standardize::Z => map(&|v| (v - mean) / sd, values),
        Standardize::Sd if sd == 0.0 => {}
        Standardize::Sd => map(&|v| v / sd, values),
        Standardize::Range if range == 0.0 => {}
        Standardize::Range => map(&|v| v / range, values),
        Standardize::Rescale if range == 0.0 => map(&|_| 0.5, values),
        Standardize::Rescale => map(&|v| (v - min) / range, values),
        Standardize::Max if max != 0.0 => map(&|v| v / max, values),
        Standardize::Max if min != 0.0 => map(&|v| v / min.abs() + 1.0, values),
        Standardize::Max => {}
        Standardize::Mean if mean == 0.0 => map(&|v| v + 1.0, values),
        Standardize::Mean => map(&|v| v / mean, values),
    }
}

/// ABSOLUTE, REVERSE and RESCALE in that order; returns whether the result is a similarity.
fn transform(command: &Proximities, prox: &mut [Vec<f64>], mut similarity: bool) -> bool {
    let apply = |prox: &mut [Vec<f64>], f: &dyn Fn(f64) -> f64| {
        prox.iter_mut().flatten().for_each(|v| *v = f(*v));
    };
    if command.absolute {
        apply(prox, &f64::abs);
    }
    if command.reverse {
        apply(prox, &|v| -v);
        similarity = !similarity;
    }
    if command.rescale {
        let finite = || prox.iter().flatten().copied().filter(|v| v.is_finite());
        let min = finite().fold(f64::INFINITY, f64::min);
        let max = finite().fold(f64::NEG_INFINITY, f64::max);
        if max > min {
            apply(prox, &|v| (v - min) / (max - min));
        }
    }
    similarity
}

impl Engine {
    /// The items and proximities computed from the active dataset; also the variables when
    /// they are the items.
    fn proximity_items(&self, command: &Proximities) -> Result<(Items, Vec<Variable<'_>>), Error> {
        let data = self.active()?;
        let mut names = self.expand(&command.variables)?;
        if command.variables.contains(&VarItem::All) {
            names.retain(|n| data.variable(n).is_ok_and(|v| !v.is_string()));
        }
        let variables: Vec<Variable> = names
            .iter()
            .map(|n| data.variable(n))
            .collect::<Result<_, _>>()?;
        if let Some(v) = variables.iter().find(|v| v.is_string()) {
            return Err(Error(format!(
                "PROXIMITIES needs numeric variables, but {} is a string variable",
                v.name()
            )));
        }
        let id = match &command.id {
            Some(name) => Some(data.variable(name)?),
            None => None,
        };
        let mut cases = Vec::new();
        let mut rows: Vec<Vec<f64>> = Vec::new();
        for case in 0..data.case_count() {
            let row: Option<Vec<f64>> = variables
                .iter()
                .map(|v| match (is_valid(v, case, command.include), v.values()) {
                    (true, Values::Numeric(values)) => values.get(case),
                    _ => None,
                })
                .collect();
            if let Some(row) = row {
                cases.push(case);
                rows.push(row);
            }
        }
        let missing = data.case_count() - cases.len();
        let k = variables.len();
        if let Some((method, by_case)) = command.standardize {
            if by_case {
                rows.iter_mut().for_each(|row| standardize(row, method));
            } else {
                for j in 0..k {
                    let mut column: Vec<f64> = rows.iter().map(|r| r[j]).collect();
                    standardize(&mut column, method);
                    rows.iter_mut().zip(column).for_each(|(r, v)| r[j] = v);
                }
            }
        }
        let vectors: Vec<Vec<f64>> = match command.view_variables {
            false => rows,
            true => (0..k)
                .map(|j| rows.iter().map(|r| r[j]).collect())
                .collect(),
        };
        let measure = command.measure_or_default();
        let n = vectors.len();
        let mut prox = vec![vec![0.0; n]; n];
        for i in 0..n {
            for j in i..n {
                let v = proximity(&measure, &vectors[i], &vectors[j]);
                prox[i][j] = v;
                prox[j][i] = v;
            }
        }
        let labels = match command.view_variables {
            true => names.clone(),
            false => cases
                .iter()
                .map(|&case| {
                    let number = self.case_number(case);
                    match &id {
                        Some(var) => format!("{number}:{}", value_text(var, case)),
                        None => number.to_string(),
                    }
                })
                .collect(),
        };
        let items = Items {
            labels,
            cases,
            prox,
            similarity: is_similarity(&measure),
            missing,
            names,
        };
        Ok((items, variables))
    }

    pub(crate) fn proximities(&mut self, command: &Proximities) -> Result<Vec<Table>, Error> {
        let mut tables = Vec::new();
        let (mut items, measure, variables) = match &command.matrix_in {
            Some(source) => {
                if command.measure.is_some() || command.standardize.is_some() {
                    self.warn(
                        "PROXIMITIES computes no measure from a matrix it reads: MEASURE and \
                         STANDARDIZE are ignored, ABSOLUTE, REVERSE and RESCALE apply",
                    );
                }
                let items = self.proximity_matrix(
                    "PROXIMITIES",
                    &command.variables,
                    command.id.as_ref(),
                    source.as_ref(),
                )?;
                let measure = match items.similarity {
                    true => "a similarity matrix".to_string(),
                    false => "a dissimilarity matrix".to_string(),
                };
                (items, measure, Vec::new())
            }
            None => {
                if self.active()?.weight_variable().is_some() {
                    self.warn("PROXIMITIES does not use the weight variable");
                }
                let (items, variables) = self.proximity_items(command)?;
                let variables: Vec<(String, Option<String>)> = variables
                    .iter()
                    .map(|v| (v.name().to_string(), v.label().map(str::to_string)))
                    .collect();
                let measure = measure_name(&command.measure_or_default());
                tables.push(summary_table(
                    items.cases.len(),
                    items.missing,
                    &format!("{measure} used"),
                ));
                (items, measure, variables)
            }
        };
        let n = items.labels.len();
        if n == 0 {
            return Err(Error(
                "PROXIMITIES has no items: every case misses a value".into(),
            ));
        }
        if (0..n).any(|i| (0..n).any(|j| i != j && !items.prox[i][j].is_finite())) {
            self.warn(format!(
                "PROXIMITIES: the {measure} is undefined for some pairs; they are system-missing"
            ));
        }
        items.similarity = transform(command, &mut items.prox, items.similarity);
        if command.print {
            let head = match command.view_variables && command.matrix_in.is_none() {
                true => "Variable",
                false => "Case",
            };
            tables.push(proximity_table(
                head,
                &items.labels,
                &items.prox,
                items.similarity,
                &measure,
                true,
            ));
        }
        if command.matrix_out.is_some() {
            let out = self.proximity_dataset(command, &items, &measure, &variables)?;
            let out = self.after_split(command, out)?;
            self.proximity_out.1.push(out);
        }
        Ok(tables)
    }

    /// The matrix file of one split group (or of all the data).
    fn proximity_dataset(
        &self,
        command: &Proximities,
        items: &Items,
        measure: &str,
        variables: &[(String, Option<String>)],
    ) -> Result<Dataset, Error> {
        let n = items.labels.len();
        let mut out = Dataset::new(n);
        let width = |values: &[String]| {
            values
                .iter()
                .map(String::len)
                .max()
                .unwrap_or(1)
                .clamp(8, 32767)
        };
        out.set_string_values("ROWTYPE_", 8, vec!["PROX".to_string(); n])?;
        let kind = match items.similarity {
            true => "SIMILARITY",
            false => "DISSIMILARITY",
        };
        out.set_value_labels(
            "ROWTYPE_",
            vec![(
                Value::String("PROX".into()),
                // a matrix read has no measure name, and its description says the old kind
                match command.matrix_in {
                    Some(_) => kind.to_string(),
                    None => format!("{measure} {kind}"),
                },
            )],
        )?;
        let by_case = !command.view_variables && command.matrix_in.is_none();
        let names: Vec<String> = match (command.matrix_in.is_some(), by_case) {
            (true, _) => items.names.clone(),
            (false, true) => (1..=n).map(|i| format!("VAR{i}")).collect(),
            (false, false) => variables.iter().map(|(name, _)| name.clone()).collect(),
        };
        if by_case {
            let data = self.active()?;
            if let Some(id) = &command.id {
                let var = data.variable(id)?;
                let values: Vec<String> =
                    items.cases.iter().map(|&c| value_text(&var, c)).collect();
                out.set_string_values(var.name(), width(&values), values)?;
            }
            let numbers = items
                .cases
                .iter()
                .map(|&c| Some(self.case_number(c) as f64));
            out.set_numeric_values("CASENO_", numbers.collect())?;
            out.set_format("CASENO_", "F8.0")?;
        }
        out.set_string_values("VARNAME_", width(&names), names.clone())?;
        for (j, name) in names.iter().enumerate() {
            let values = (0..n).map(|i| Some(items.prox[i][j]).filter(|v| v.is_finite()));
            out.set_numeric_values(name, values.collect())?;
            let label = match by_case {
                true => Some(format!("CASE {}", self.case_number(items.cases[j]))),
                false => variables.get(j).and_then(|(_, label)| label.clone()),
            };
            out.set_variable_label(name, label.as_deref())?;
        }
        Ok(out)
    }

    /// The matrix of one group after the values of the split variables, taken from the
    /// group's first case; the items then carry no case labels.
    fn after_split(&self, command: &Proximities, part: Dataset) -> Result<Dataset, Error> {
        let names = &self.proximity_out.0;
        if names.is_empty() {
            return Ok(part);
        }
        let group = self.active()?;
        let n = part.case_count();
        let mut joined = Dataset::new(n);
        for name in names {
            joined.copy_variable(group, name, &vec![Some(0); n])?;
        }
        let cases: Vec<Option<usize>> = (0..n).map(Some).collect();
        let mut items = false;
        for name in part.variable_names() {
            joined.copy_variable(&part, name, &cases)?;
            if items && !command.view_variables {
                joined.set_variable_label(name, None)?;
            }
            items |= name == "VARNAME_";
        }
        Ok(joined)
    }

    /// Writes the matrices of MATRIX=OUT that the runs of the split groups left, one after
    /// the other. A group with fewer items has the others blank; strings take the widest.
    pub(crate) fn proximities_out(&mut self, command: &Proximities) -> Result<(), Error> {
        let (_, parts) = std::mem::take(&mut self.proximity_out);
        let Some(target) = &command.matrix_out else {
            return Ok(());
        };
        let Some(widest) = parts.iter().max_by_key(|d| d.variable_count()) else {
            return Ok(());
        };
        let mut template = widest.slice(0, 0);
        let strings: Vec<String> = template
            .variables()
            .filter(|v| v.is_string())
            .map(|v| v.name().to_string())
            .collect();
        for name in strings {
            let width = parts
                .iter()
                .filter_map(|p| p.variable(&name).ok()?.string_width())
                .max()
                .unwrap_or(1);
            template.set_string_width(&name, width)?;
        }
        let out = Dataset::from_parts(&template, &parts)?;
        match target {
            None => self.dataset = Some(out),
            Some(file) => self.write_data_file(file, out)?,
        }
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn standardizing_a_constant_or_a_zero_largest_value() {
    let run = |values: &[f64], method| {
        let mut v = values.to_vec();
        standardize(&mut v, method);
        v
    };
    assert_eq!(run(&[2.0, 2.0], Standardize::Z), [0.0, 0.0]);
    assert_eq!(run(&[2.0, 2.0], Standardize::Sd), [2.0, 2.0]);
    assert_eq!(run(&[2.0, 2.0], Standardize::Range), [2.0, 2.0]);
    assert_eq!(run(&[2.0, 2.0], Standardize::Rescale), [0.5, 0.5]);
    assert_eq!(run(&[-4.0, -2.0, 0.0], Standardize::Max), [0.0, 0.5, 1.0]);
    assert_eq!(run(&[-1.0, 1.0], Standardize::Mean), [0.0, 2.0]);
    assert_eq!(run(&[1.0, 3.0], Standardize::Mean), [0.5, 1.5]);
    assert_eq!(run(&[1.0, 3.0], Standardize::Rescale), [0.0, 1.0]);
    let z = run(&[1.0, 2.0, 3.0], Standardize::Z);
    assert_eq!(z, [-1.0, 0.0, 1.0]);
}
