//! RECODE and COUNT: both test the values of variables against lists of values.

use crate::variables::{Column, new_names};
use crate::{Engine, Error};
use statscraft_data::{Values, Variable};
use statscraft_syntax::{
    Bound, Count, Recode, RecodeInput, RecodeMapping, RecodeOutput, Value, VarItem,
};

/// The value a transformation stores for one case: `new`, or what the target held before
/// (`old`) when a mask of active cases leaves the case out.
fn assign<T>(mask: Option<&[bool]>, case: usize, new: T, old: impl FnOnce() -> T) -> T {
    if mask.is_none_or(|mask| mask[case]) {
        new
    } else {
        old()
    }
}

fn kind(is_string: bool) -> &'static str {
    if is_string { "string" } else { "numeric" }
}

/// An error unless every input can be compared with the values of the variable.
fn check_inputs<'a>(
    variable: &Variable,
    inputs: impl IntoIterator<Item = &'a RecodeInput>,
) -> Result<(), Error> {
    for input in inputs {
        let for_strings = match input {
            RecodeInput::Else | RecodeInput::Missing => continue,
            RecodeInput::Value(Value::String(_)) => true,
            RecodeInput::Value(Value::Number(_)) | RecodeInput::Range(..) | RecodeInput::Sysmis => {
                false
            }
        };
        if for_strings != variable.is_string() {
            let name = variable.name();
            return Err(Error(if variable.is_string() {
                format!(
                    "{name} is a string variable, so its values must be quoted strings (numbers, ranges and SYSMIS do not apply)"
                )
            } else {
                format!(
                    "{name} is a numeric variable, so its values must be numbers, not quoted strings"
                )
            }));
        }
    }
    Ok(())
}

/// A value or a range also matches a user-missing value; only system-missing needs MISSING,
/// SYSMIS or ELSE. A range written high THRU low is read the other way round.
fn matches(input: &RecodeInput, variable: &Variable, case: usize) -> bool {
    let within = |low: &Bound, high: &Bound, v: f64| match (low, high) {
        (Bound::Number(a), Bound::Number(b)) => a.min(*b) <= v && v <= a.max(*b),
        (Bound::Number(low), _) => *low <= v,
        (_, Bound::Number(high)) => v <= *high,
        _ => true,
    };
    match variable.values() {
        Values::Numeric(values) => match (input, values.get(case)) {
            (RecodeInput::Else, _) => true,
            (RecodeInput::Sysmis, v) => v.is_none(),
            (RecodeInput::Missing, v) => v.is_none_or(|v| variable.is_user_missing(v)),
            (RecodeInput::Value(Value::Number(n)), Some(v)) => v == *n,
            (RecodeInput::Range(low, high), Some(v)) => within(low, high, v),
            _ => false,
        },
        Values::String(values) => match input {
            RecodeInput::Else => true,
            RecodeInput::Missing => variable.is_user_missing_str(values.get(case)),
            RecodeInput::Value(Value::String(s)) => s.trim_end_matches(' ') == values.get(case),
            _ => false,
        },
    }
}

/// The longest prefix of `text` that fits in `width` bytes.
fn fitted(text: &str, width: usize) -> &str {
    let mut end = text.len().min(width);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

impl Engine {
    /// The cases a transformation may write to; None is every case.
    // DO IF hooks in here: return its mask of currently active cases.
    fn case_mask(&self) -> Option<&[bool]> {
        None
    }

    /// Existing variables in dictionary spelling, new names as written.
    fn recode_targets(&self, into: &[VarItem]) -> Result<Vec<String>, Error> {
        let data = self.active()?;
        let mut names = Vec::new();
        for item in into {
            let existing = match item {
                VarItem::Range(a, b) => {
                    data.variable_index(a).is_some() && data.variable_index(b).is_some()
                }
                _ => false,
            };
            let item = std::slice::from_ref(item);
            names.extend(if existing {
                self.expand(item)?
            } else {
                new_names(item)?
            });
        }
        Ok(names)
    }

    pub(crate) fn recode(&mut self, groups: &[Recode]) -> Result<(), Error> {
        for group in groups {
            let sources = self.expand(&group.vars)?;
            let targets = if group.into.is_empty() {
                sources.clone()
            } else {
                self.recode_targets(&group.into)?
            };
            if targets.len() != sources.len() {
                return Err(Error(format!(
                    "{} source variables ({}) but {} target variables after INTO ({}); give one target per source",
                    sources.len(),
                    sources.join(" "),
                    targets.len(),
                    targets.join(" ")
                )));
            }
            // one variable after the other, so a later source sees an earlier target
            for (source, target) in sources.iter().zip(&targets) {
                let column = self.recoded(group, source, target)?;
                let data = self.active_mut()?;
                match column {
                    Column::Numeric(values) => data.set_numeric_values(target, values)?,
                    Column::String(width, values) => {
                        data.set_string_values(target, width, values)?
                    }
                }
            }
        }
        Ok(())
    }

    fn recoded(&self, group: &Recode, source: &str, target: &str) -> Result<Column, Error> {
        let data = self.active()?;
        let mask = self.case_mask();
        let in_place = group.into.is_empty();
        let source = data.variable(source)?;
        check_inputs(&source, group.mappings.iter().flat_map(|m| &m.inputs))?;

        let is_string = |output: &RecodeOutput| match output {
            RecodeOutput::Value(Value::String(_)) => true,
            RecodeOutput::Copy => source.is_string(),
            RecodeOutput::Value(Value::Number(_)) | RecodeOutput::Sysmis => false,
        };
        let mut outputs = group.mappings.iter().map(|m| &m.output);
        let to_string = outputs.next().is_some_and(is_string);
        if outputs.any(|output| is_string(output) != to_string) {
            return Err(Error(format!(
                "the output values of {} must be all numbers or all strings (SYSMIS is a number, COPY has the type of the source variable)",
                source.name()
            )));
        }
        let existing = data.variable(target).ok();
        if in_place && to_string != source.is_string() {
            return Err(Error(format!(
                "INTO is required to recode the {} variable {} into {} values",
                kind(source.is_string()),
                source.name(),
                kind(to_string)
            )));
        }
        if let Some(existing) = existing.filter(|t| t.is_string() != to_string) {
            return Err(Error(format!(
                "cannot store {} values in the {} variable {}",
                kind(to_string),
                kind(existing.is_string()),
                existing.name()
            )));
        }

        let output = |case: usize| {
            let hit =
                |m: &&RecodeMapping| m.inputs.iter().any(|input| matches(input, &source, case));
            group.mappings.iter().find(hit).map(|m| &m.output)
        };
        let cases = 0..data.case_count();
        if !to_string {
            let own = source.numeric().ok();
            let old = existing.and_then(|t| t.numeric().ok());
            let copy = |case: usize| own.and_then(|values| values.get(case));
            return Ok(Column::Numeric(
                cases
                    .map(|case| {
                        let new = match output(case) {
                            Some(RecodeOutput::Value(Value::Number(n))) => Some(*n),
                            Some(RecodeOutput::Copy) => copy(case),
                            None if in_place => copy(case),
                            // without a match INTO clears the target even when it existed
                            // before, as the reference program does
                            _ => None,
                        };
                        assign(mask, case, new, || old.and_then(|values| values.get(case)))
                    })
                    .collect(),
            ));
        }

        let Some((existing, width)) = existing.and_then(|t| Some((t, t.string_width()?))) else {
            return Err(Error(format!(
                "the string target {target} does not exist; create it first with the STRING command, for example STRING {target} (A8)"
            )));
        };
        for mapping in &group.mappings {
            if let RecodeOutput::Value(Value::String(text)) = &mapping.output
                && text.trim_end_matches(' ').len() > width
            {
                return Err(Error(format!(
                    "the output value '{text}' needs {} bytes but {} has width {width}",
                    text.trim_end_matches(' ').len(),
                    existing.name()
                )));
            }
        }
        let own = source.string().ok();
        let old = existing.string()?;
        let copy = |case: usize| own.map_or("", |values| fitted(values.get(case), width));
        Ok(Column::String(
            width,
            cases
                .map(|case| {
                    let new = match output(case) {
                        Some(RecodeOutput::Value(Value::String(text))) => text.as_str(),
                        Some(RecodeOutput::Copy) => copy(case),
                        None if in_place => copy(case),
                        _ => "",
                    };
                    assign(mask, case, new, || old.get(case)).to_string()
                })
                .collect(),
        ))
    }

    pub(crate) fn count(&mut self, counts: &[Count]) -> Result<(), Error> {
        for count in counts {
            let values = self.counted(count)?;
            self.active_mut()?
                .set_numeric_values(&count.target, values)?;
        }
        Ok(())
    }

    fn counted(&self, count: &Count) -> Result<Vec<Option<f64>>, Error> {
        let data = self.active()?;
        let mask = self.case_mask();
        let mut tests = Vec::new();
        for (vars, inputs) in &count.criteria {
            for name in self.expand(vars)? {
                let variable = data.variable(&name)?;
                check_inputs(&variable, inputs)?;
                tests.push((variable, inputs));
            }
        }
        let old = match data.variable(&count.target) {
            Ok(target) => Some(target.numeric().map_err(|_| {
                Error(format!(
                    "the target {} is a string variable; a count needs a numeric or a new variable",
                    target.name()
                ))
            })?),
            Err(_) => None,
        };
        Ok((0..data.case_count())
            .map(|case| {
                let hits = tests
                    .iter()
                    .filter(|(variable, inputs)| inputs.iter().any(|i| matches(i, variable, case)))
                    .count();
                assign(mask, case, Some(hits as f64), || {
                    old.and_then(|values| values.get(case))
                })
            })
            .collect())
    }
}
