//! RMV: copies of series with their missing values replaced by an estimate.
//!
//! Rules noted from the Command Syntax Reference (the Algorithms manual has no section for it):
//! - LINT interpolates linearly between the last valid value before and the first valid value
//!   after a missing one, by case position; a missing value before the first or after the last
//!   valid value stays missing.
//! - MEAN and MEDIAN take the nearest `span` valid values on each side (2 when no span is
//!   written), skipping missing cases, and replace nothing when a side has fewer; with ALL (and
//!   for SMEAN, which is MEAN with ALL) they take every valid value. An even count of values
//!   has the average of its two middle values as the median.
//! - TREND regresses the valid values on the case position 1 to n and replaces a missing value
//!   by its predicted value; it needs two valid values.
//! - Only the valid values of the source feed an estimate, never an estimate made before.
//! - A user-missing value counts as missing unless TSET MISSING=INCLUDE. USE is not honored.
//!   With SPLIT FILE each group is a series of its own (and the position for TREND restarts);
//!   cases a filter leaves out keep the value of an existing target, else system-missing.
//! - The new variable has the source's decimals plus two, and a label naming the function and
//!   the source. (The label has no date and time of creation, so that a run gives the same
//!   output every time.)
//! - The table lists, per new variable, the number of values replaced, the first and last case
//!   with a valid value in it, its number of valid values and its function.

use crate::npar_tests::text;
use crate::regression::table;
use crate::timeseries::{Values, decimals, mean, median, read_series};
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Measure;
use statscraft_syntax::{RmvEquation, RmvFunction, RmvSpan};

/// `part` with its missing values estimated, and how many were.
fn fill(part: &[Option<f64>], function: RmvFunction, span: RmvSpan) -> (Values, usize) {
    let valid: Vec<usize> = (0..part.len()).filter(|&i| part[i].is_some()).collect();
    let value = |i: usize| part[i].expect("valid");
    let all: Vec<f64> = valid.iter().map(|&i| value(i)).collect();
    // regression on the case position, over the valid values
    let line = (function == RmvFunction::Trend && valid.len() >= 2).then(|| {
        let n = valid.len() as f64;
        let mx = valid.iter().map(|&i| (i + 1) as f64).sum::<f64>() / n;
        let my = mean(&all);
        let sxx: f64 = valid.iter().map(|&i| ((i + 1) as f64 - mx).powi(2)).sum();
        let sxy: f64 = valid
            .iter()
            .map(|&i| ((i + 1) as f64 - mx) * (value(i) - my))
            .sum();
        let slope = sxy / sxx;
        (my - slope * mx, slope)
    });
    let mut out = part.to_vec();
    let mut replaced = 0;
    for i in (0..part.len()).filter(|&i| part[i].is_none()) {
        let before = valid.partition_point(|&j| j < i);
        let estimate = match function {
            RmvFunction::Lint => (before > 0 && before < valid.len()).then(|| {
                let (a, b) = (valid[before - 1], valid[before]);
                value(a) + (value(b) - value(a)) * (i - a) as f64 / (b - a) as f64
            }),
            RmvFunction::Trend => line.map(|(intercept, slope)| intercept + slope * (i + 1) as f64),
            RmvFunction::Mean | RmvFunction::Median | RmvFunction::Smean => {
                let stat = if function == RmvFunction::Median {
                    median
                } else {
                    mean
                };
                match span {
                    RmvSpan::All => (!all.is_empty()).then(|| stat(&all)),
                    RmvSpan::Cases(n) => {
                        let n = n as usize;
                        (before >= n && valid.len() - before >= n)
                            .then(|| stat(&all[before - n..before + n]))
                    }
                }
            }
        };
        if let Some(v) = estimate {
            out[i] = Some(v);
            replaced += 1;
        }
    }
    (out, replaced)
}

impl Engine {
    pub(crate) fn rmv(&mut self, equations: &[RmvEquation]) -> Result<Vec<Table>, Error> {
        self.begin_pass()?;
        let before = self.dataset.clone();
        let result = self.rmv_series(equations);
        if result.is_err() {
            self.dataset = before.clone();
        }
        self.end_pass(before.as_ref());
        result
    }

    fn rmv_series(&mut self, equations: &[RmvEquation]) -> Result<Vec<Table>, Error> {
        let groups = self.case_groups()?;
        let include = self.tset.include;
        let mut rows = Vec::new();
        for equation in equations {
            let f = equation.function;
            let sources = self.expand(&equation.sources)?;
            if sources.len() != equation.targets.len() {
                return Err(Error(format!(
                    "{} makes {} series here, but {} new names are given",
                    f.name(),
                    sources.len(),
                    equation.targets.len()
                )));
            }
            if let Some(name) = crate::variables::duplicate(&equation.targets) {
                return Err(Error(format!("{name} is named twice as a new variable")));
            }
            let data = self.active()?;
            for target in &equation.targets {
                if crate::rows::is_scratch(target) || target.starts_with('$') {
                    return Err(Error(format!(
                        "{target} cannot be a series: scratch and system names are not allowed"
                    )));
                }
                if data.variable(target).is_ok_and(|v| v.is_string()) {
                    return Err(Error(format!(
                        "{target} is a string variable and cannot hold a series"
                    )));
                }
            }
            let cases = data.case_count();
            let mut made = Vec::new();
            for source in &sources {
                let (x, _, _) = read_series(data, source, include, "RMV")?;
                let format = data.variable(source)?.format().to_string();
                let mut out = vec![None; cases];
                let mut replaced = 0;
                for group in &groups {
                    let part: Values = group.iter().map(|&c| x[c]).collect();
                    let (values, n) = fill(&part, f, equation.span);
                    replaced += n;
                    for (k, &c) in group.iter().enumerate() {
                        out[c] = values[k];
                    }
                }
                let span = match (f, equation.span) {
                    (RmvFunction::Mean | RmvFunction::Median, RmvSpan::Cases(n)) => format!(",{n}"),
                    (RmvFunction::Mean | RmvFunction::Median, RmvSpan::All) => ",ALL".to_string(),
                    _ => String::new(),
                };
                let label = format!("{}({source}{span})", f.name());
                made.push((
                    label,
                    format!("F10.{}", (decimals(&format) + 2).min(16)),
                    out,
                    replaced,
                ));
            }
            // filtered-out cases keep the value they had (none for a new variable)
            let data = self.active_mut()?;
            let mut kept = vec![false; cases];
            for c in groups.iter().flatten() {
                kept[*c] = true;
            }
            for (target, (label, format, values, replaced)) in equation.targets.iter().zip(made) {
                let old: Values = match data.variable(target).ok() {
                    Some(v) => {
                        let n = v.numeric()?;
                        (0..cases).map(|c| n.get(c)).collect()
                    }
                    None => vec![None; cases],
                };
                let merged: Values = (0..cases)
                    .map(|c| if kept[c] { values[c] } else { old[c] })
                    .collect();
                let valid: Vec<usize> = (0..cases).filter(|&c| merged[c].is_some()).collect();
                data.set_numeric_values(target, merged)?;
                data.set_variable_label(target, Some(&label))?;
                data.set_format(target, &format)?;
                data.set_measure(target, Measure::Scale)?;
                let case =
                    |c: Option<&usize>| c.map_or(Cell::Empty, |c| Cell::Number(*c as f64 + 1.0));
                rows.push(vec![
                    text(target.clone()),
                    Cell::Number(replaced as f64),
                    case(valid.first()),
                    case(valid.last()),
                    Cell::Number(valid.len() as f64),
                    text(label),
                ]);
            }
        }
        Ok(vec![table(
            "Result Variables",
            1,
            vec![
                (String::new(), "Result Variable".into()),
                (String::new(), "N of Replaced Missing Values".into()),
                ("Case Number of Non-Missing Values".into(), "First".into()),
                ("Case Number of Non-Missing Values".into(), "Last".into()),
                (String::new(), "N of Valid Cases".into()),
                (String::new(), "Creating Function".into()),
            ],
            rows,
            None,
        )])
    }
}
