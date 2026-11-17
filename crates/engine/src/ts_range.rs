//! USE and PREDICT: the historical and the forecast period of the time series procedures.
//!
//! Rules noted from the descriptions of USE and PREDICT:
//! - A range end is a case number or a date of the DATE variables (the highest one among
//!   them); FIRST, LAST and END are the ends of the file. USE ALL restores every case.
//! - USE filters the cases of the procedures that follow, except CREATE (and RMV). USE and
//!   FILTER exclude each other; SORT CASES and DATE end USE, and DATE ends PREDICT.
//! - PREDICT starts after the USE range (or the file) unless told otherwise; a start past
//!   that is moved back to it. It ends at the last case of the file by default. When it runs
//!   past the file, the forecasting procedure adds cases (at most TSET MXPREDICT), missing on
//!   all variables but the new ones and the DATE variables.
//!
//! Choices where the manual is silent:
//! - A date start is the first case of that date, a date end the last; a date end past the
//!   file is counted on from the DATE specification.
//! - PREDICT without an end, starting past the file, forecasts that one case.
//! - Forecasting procedures run on the whole split group or file and apply USE themselves;
//!   only an unsplit, unfiltered file is extended, and only when the procedure saves
//!   variables.

use crate::{Engine, Error};
use statscraft_data::Dataset;
use statscraft_syntax::{CasePoint, Command, DateUnit, NewVar, UseRange};

/// Where a forecasting procedure estimates and forecasts, as cases of the dataset it runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Periods {
    /// The USE range, both ends included.
    pub first: usize,
    pub last: usize,
    /// The first forecast; one-step fits come before it.
    pub origin: usize,
    /// The last case with a fitted value or forecast.
    pub end: usize,
}

/// How a range end reads in TSHOW.
fn point_text(point: &Option<CasePoint>, end: bool) -> String {
    match point {
        None if end => "LAST".into(),
        None => "FIRST".into(),
        Some(CasePoint::Case(n)) => n.to_string(),
        Some(CasePoint::Date(parts)) => parts
            .iter()
            .map(|(unit, v)| format!("{} {v}", unit.name()))
            .collect::<Vec<_>>()
            .join(" "),
    }
}

pub(crate) fn range_text(range: &UseRange) -> String {
    format!(
        "{} THRU {}",
        point_text(&range.first, false),
        point_text(&range.last, true)
    )
}

impl Engine {
    /// The case of `data` a range end names: the first match for a start, the last for an end.
    /// It may lie past the file. `numbers` are the file's case numbers (from 0) of the cases of
    /// `data` when it is a split group.
    fn point_case(
        &self,
        data: &Dataset,
        point: &CasePoint,
        end: bool,
        numbers: Option<&[usize]>,
        command: &str,
    ) -> Result<Option<usize>, Error> {
        let n = data.case_count();
        match point {
            CasePoint::Case(c) => {
                let c = (*c as usize).saturating_sub(1);
                Ok(Some(match numbers {
                    None => c,
                    Some(numbers) if end => match numbers.iter().rposition(|&k| k <= c) {
                        Some(k) => k,
                        None => return Ok(None),
                    },
                    Some(numbers) => numbers.iter().position(|&k| k >= c).unwrap_or(n),
                }))
            }
            CasePoint::Date(parts) => {
                let mut columns = Vec::new();
                for (unit, value) in parts {
                    let name = format!("{}_", unit.name());
                    let Ok(variable) = data.variable(&name) else {
                        return Err(Error(format!(
                            "{command}: there is no date variable {name}; define the dates with DATE"
                        )));
                    };
                    columns.push((variable.numeric()?, *value));
                }
                let mut units = DateUnit::ALL;
                units.sort();
                let highest = units
                    .iter()
                    .find(|u| data.variable(&format!("{}_", u.name())).is_ok());
                if let Some(highest) = highest
                    && !parts.iter().any(|(u, _)| u == highest)
                {
                    return Err(Error(format!(
                        "{command}: a date names the highest date variable, {}",
                        highest.name()
                    )));
                }
                let matches = |k: usize| {
                    columns
                        .iter()
                        .all(|(c, v)| c.get(k).is_some_and(|x| x == *v))
                };
                let found = match end {
                    false => (0..n).find(|&k| matches(k)),
                    true => (0..n).rfind(|&k| matches(k)),
                };
                // a date past the file, or the end of a date the file stops inside of
                let counted = |found: Option<usize>| -> Option<usize> {
                    let spec = self.date_spec.as_ref()?;
                    let limit = n + 100_000;
                    crate::date::date_case(spec, parts, end, limit).filter(|&k| match found {
                        Some(f) => k >= f,
                        None => k >= n,
                    })
                };
                Ok(match found {
                    Some(k) if end && k + 1 == n && numbers.is_none() => counted(found).or(found),
                    Some(k) => Some(k),
                    None if numbers.is_none() => counted(None),
                    None => None,
                })
            }
        }
    }

    /// The USE range of `data`, both ends included; None without USE. An error when it holds
    /// no case.
    pub(crate) fn use_bounds(
        &self,
        data: &Dataset,
        numbers: Option<&[usize]>,
    ) -> Result<Option<(usize, usize)>, Error> {
        let Some(range) = &self.ts_use else {
            return Ok(None);
        };
        Ok(Some(self.bounds(data, range, numbers)?))
    }

    fn bounds(
        &self,
        data: &Dataset,
        range: &UseRange,
        numbers: Option<&[usize]>,
    ) -> Result<(usize, usize), Error> {
        let n = data.case_count();
        let first = match &range.first {
            None => 0,
            Some(p) => self
                .point_case(data, p, false, numbers, "USE")?
                .ok_or_else(|| {
                    Error(format!(
                        "USE: no case is at or after {}",
                        point_text(&range.first, false)
                    ))
                })?,
        };
        let last = match &range.last {
            None => n.saturating_sub(1),
            Some(p) => self
                .point_case(data, p, true, numbers, "USE")?
                .ok_or_else(|| {
                    Error(format!(
                        "USE: no case is at or before {}",
                        point_text(&range.last, true)
                    ))
                })?
                .min(n.saturating_sub(1)),
        };
        if n == 0 || first > last {
            return Err(Error(format!(
                "the USE range {} holds no case",
                range_text(range)
            )));
        }
        Ok((first, last))
    }

    /// The forecast period: its first and last case, either may lie past the file; None
    /// without PREDICT.
    pub(crate) fn predict_bounds(
        &self,
        data: &Dataset,
        last_used: usize,
        numbers: Option<&[usize]>,
    ) -> Result<Option<(usize, usize)>, Error> {
        let Some(range) = &self.ts_predict else {
            return Ok(None);
        };
        let n = data.case_count();
        let start = match &range.first {
            None => last_used + 1,
            Some(p) => self
                .point_case(data, p, false, numbers, "PREDICT")?
                .unwrap_or(last_used + 1)
                .min(last_used + 1),
        };
        let end = match &range.last {
            None => n.saturating_sub(1).max(start),
            Some(p) => self
                .point_case(data, p, true, numbers, "PREDICT")?
                .ok_or_else(|| {
                    Error(format!(
                        "PREDICT: the end {} is not a case of the file",
                        point_text(&range.last, true)
                    ))
                })?,
        };
        if end < start {
            return Err(Error(format!(
                "PREDICT {} ends before it starts",
                range_text(range)
            )));
        }
        Ok(Some((start, end)))
    }

    /// The periods of the forecasting procedure on the active dataset (the whole file or a
    /// split group).
    pub(crate) fn periods(&self, command: &str) -> Result<Periods, Error> {
        let data = self.active()?;
        let numbers = self.case_numbers.as_deref();
        let n = data.case_count();
        // the series ends before the cases PREDICT added
        let series = n - self.ts_added.min(n);
        if series == 0 {
            return Err(Error(format!("{command}: there are no cases")));
        }
        let (first, last) = self.use_bounds(data, numbers)?.unwrap_or((0, series - 1));
        let last = last.min(series - 1);
        let Some((start, end)) = self.predict_bounds(data, last, numbers)? else {
            return Ok(Periods {
                first,
                last,
                origin: last + 1,
                end: last,
            });
        };
        if start <= first {
            return Err(Error(format!(
                "{command}: the PREDICT period starts at the second case of the USE range or later"
            )));
        }
        Ok(Periods {
            first,
            last,
            origin: start,
            end: end.min(n - 1).max(start - 1),
        })
    }

    /// Before EXSMOOTH or ARIMA: adds the cases the PREDICT period needs past the end of an
    /// unsplit, unfiltered file, with their date variables.
    pub(crate) fn extend_for_predict(&mut self, command: &Command) -> Result<(), Error> {
        let name = match command {
            Command::Exsmooth(_) => "EXSMOOTH",
            Command::Arima(a) if a.regressors.is_empty() => "ARIMA",
            // these two do not follow TSET: they add cases when they save variables
            Command::Tsmodel(t) if !t.output.save.is_empty() => "TSMODEL",
            Command::Tsapply(t) if !t.output.save.is_empty() => "TSAPPLY",
            _ => return Ok(()),
        };
        let saves = matches!(command, Command::Tsmodel(_) | Command::Tsapply(_));
        if self.ts_predict.is_none() || (self.tset.newvar == NewVar::None && !saves) {
            return Ok(());
        }
        let data = self.active()?;
        let n = data.case_count();
        if n == 0 {
            return Ok(());
        }
        let (_, last) = self.use_bounds(data, None)?.unwrap_or((0, n - 1));
        let Some((_, end)) = self.predict_bounds(data, last, None)? else {
            return Ok(());
        };
        if end < n {
            return Ok(());
        }
        if data.split_file().is_some() || data.filter_variable().is_some() {
            self.warn(format!(
                "{name}: under SPLIT FILE or FILTER the file is not extended, so there are no \
                 forecasts past its last case"
            ));
            return Ok(());
        }
        let mut added = end + 1 - n;
        let most = self.tset.mxpredict as usize;
        if added > most {
            self.warn(format!(
                "{name}: the PREDICT period needs {added} new cases, more than TSET MXPREDICT={most}; \
                 {most} are added"
            ));
            added = most;
        }
        let spec = self.date_spec.clone();
        let data = self.active_mut()?;
        let cases: Vec<Option<usize>> = (0..n).map(Some).chain((0..added).map(|_| None)).collect();
        data.take_cases(&cases)?;
        self.ts_added = added;
        let data = self.active_mut()?;
        if let Some(spec) = spec {
            let (columns, texts, width) = crate::date::date_columns(&spec, n + added)?;
            if columns.iter().all(|(c, _)| data.variable(c).is_ok())
                && data.variable("DATE_").is_ok()
            {
                for (column, values) in columns {
                    data.set_numeric_values(&column, values)?;
                }
                data.set_string_values("DATE_", width.max(1), texts)?;
            }
        }
        Ok(())
    }

    /// USE: a range for the procedures that follow, ALL, or /PERMANENT.
    pub(crate) fn use_cases(&mut self, range: Option<&UseRange>) -> Result<(), Error> {
        // USE and FILTER exclude each other
        if let Some(data) = self
            .dataset
            .as_mut()
            .filter(|d| d.filter_variable().is_some())
        {
            data.set_filter_variable(None)?;
        }
        let Some(range) = range else {
            self.ts_use = None;
            return Ok(());
        };
        if !range.permanent {
            self.ts_use = Some(range.clone());
            return Ok(());
        }
        self.begin_pass()?;
        let data = self.active()?;
        let (first, last) = self.bounds(data, range, None)?;
        let keep: Vec<bool> = (0..data.case_count())
            .map(|c| c >= first && c <= last)
            .collect();
        self.active_mut()?.select_cases(&keep)?;
        self.ts_use = None;
        Ok(())
    }
}
