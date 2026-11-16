//! DATE: variables that count the cases of a time series.
//!
//! Rules, from the description of DATE:
//! - Every keyword makes a numeric variable named after it with an underscore (YEAR_), and
//!   DATE_, a string, shows them together. Variables of an earlier DATE are replaced.
//! - Starting values default to 1, for HOUR, MINUTE and SECOND to 0; a periodic value runs from
//!   there to the period (HOUR, MINUTE, SECOND: to the period minus 1) and then starts over,
//!   adding 1 to the keyword above it.
//! - Default periods: QUARTER 4, MONTH 12, DAY 7, HOUR 24, MINUTE and SECOND 60. The highest
//!   keyword gets no default period, except QUARTER. When the highest keyword has a period,
//!   CYCLE_ is added above it. OBS needs a start and a period; CYCLE takes no period.
//! - QUARTER and MONTH infer each other's start; conflicting starts are an error. With MONTH the
//!   quarter is the quarter of the month.
//! - BY steps the lowest keyword and must divide its period.
//! - DATE_ lists the parts in fixed widths, as "MMM YYYY" or "WWWW DD HH"; each label states the
//!   period ("MONTH, period 12", "YEAR, not periodic") and DATE_'s label its form.
//!
//! Choices where the manual is silent:
//! - In DATE_ a long-period date reads cycle, month (as JAN to DEC) or quarter, then year; a
//!   short one from the highest part to the lowest. A part is as wide as the largest value its
//!   period allows (four digits without a period, a year as given in two or four digits), wider
//!   when the data need it; MINUTE is shown with the letter M.
//! - The numeric variables get an F format as wide as their largest value.

use crate::{Cell, Engine, Error, Table};
use statscraft_syntax::{DateSpec, DateUnit};

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// One date variable, as it will be made.
struct Part {
    unit: DateUnit,
    start: i64,
    period: Option<i64>,
    values: Vec<i64>,
}

fn base(unit: DateUnit) -> i64 {
    match unit {
        DateUnit::Hour | DateUnit::Minute | DateUnit::Second => 0,
        _ => 1,
    }
}

fn default_period(unit: DateUnit) -> Option<i64> {
    match unit {
        DateUnit::Quarter => Some(4),
        DateUnit::Month => Some(12),
        DateUnit::Day => Some(7),
        DateUnit::Hour => Some(24),
        DateUnit::Minute | DateUnit::Second => Some(60),
        _ => None,
    }
}

fn letter(unit: DateUnit) -> char {
    match unit {
        DateUnit::Cycle => 'C',
        DateUnit::Year => 'Y',
        DateUnit::Quarter => 'Q',
        DateUnit::Month | DateUnit::Minute => 'M',
        DateUnit::Week => 'W',
        DateUnit::Day => 'D',
        DateUnit::Hour => 'H',
        DateUnit::Second => 'S',
        DateUnit::Obs => 'O',
    }
}

fn digits(n: i64) -> usize {
    n.unsigned_abs().to_string().len() + usize::from(n < 0)
}

impl Engine {
    pub(crate) fn date(&mut self, spec: &DateSpec) -> Result<Vec<Table>, Error> {
        self.begin_pass()?;
        let before = self.dataset.clone();
        let result = self.add_dates(spec);
        self.end_pass(before.as_ref());
        result
    }

    fn add_dates(&mut self, spec: &DateSpec) -> Result<Vec<Table>, Error> {
        let cases = self.active()?.case_count();
        let parts = date_parts(spec, cases)?;
        let step = spec.by.map_or(1, |b| b as i64);
        let lowest = parts.len() - 1;
        let (texts, text_width, form) = date_texts(&parts, cases);
        let mut data = self.active()?.clone();
        let old: Vec<String> = data
            .variable_names()
            .filter(|name| {
                let upper = name.to_uppercase();
                upper == "DATE_"
                    || DateUnit::ALL
                        .iter()
                        .any(|u| upper == format!("{}_", u.name()))
            })
            .map(str::to_string)
            .collect();
        if old.len() == data.variable_count() {
            data = statscraft_data::Dataset::new(cases);
        } else if !old.is_empty() {
            let names: Vec<&str> = old.iter().map(String::as_str).collect();
            data.delete_variables(&names)?;
        }
        let mut rows = Vec::new();
        for (i, part) in parts.iter().enumerate() {
            let name = format!("{}_", part.unit.name());
            let values = part.values.iter().map(|&v| Some(v as f64)).collect();
            data.set_numeric_values(&name, values)?;
            let width = part.values.iter().map(|&v| digits(v)).max().unwrap_or(1);
            data.set_format(&name, &format!("F{}.0", width.max(1)))?;
            let label = match part.period {
                Some(p) if i == lowest && step > 1 => {
                    format!("{}, period {p} by {step}", part.unit.name())
                }
                Some(p) => format!("{}, period {p}", part.unit.name()),
                None => format!("{}, not periodic", part.unit.name()),
            };
            data.set_variable_label(&name, Some(&label))?;
            rows.push(vec![Cell::Text(name), Cell::Text(label)]);
        }
        data.set_string_values("DATE_", text_width.max(1), texts)?;
        let label = format!("DATE. FORMAT: \"{}\"", form.join(" "));
        data.set_variable_label("DATE_", Some(&label))?;
        rows.push(vec![Cell::Text("DATE_".into()), Cell::Text(label)]);
        self.dataset = Some(data);
        Ok(vec![Table {
            title: "New variables".into(),
            columns: vec!["Name".into(), "Label".into()],
            rows,
            ..Table::default()
        }])
    }
}

/// The date variables of `spec` over `cases` cases, checked.
fn date_parts(spec: &DateSpec, cases: usize) -> Result<Vec<Part>, Error> {
    let mut keywords = spec.keywords.clone();
    keywords.sort_by_key(|k| k.0);
    let highest = keywords[0].0;
    let given = |unit: DateUnit| keywords.iter().find(|k| k.0 == unit);
    let as_int = |n: f64| n as i64;
    // larger starts, periods and steps would overflow the counting below
    let numbers = keywords.iter().flat_map(|k| [k.1, k.2]).chain([spec.by]);
    if let Some(n) = numbers.flatten().find(|n| n.abs() > 1e9) {
        return Err(Error(format!(
            "{n:e} is too large for DATE: starts, periods and BY lie within 1e9"
        )));
    }

    // QUARTER and MONTH give each other their start
    let (quarter, month) = (given(DateUnit::Quarter), given(DateUnit::Month));
    let (mut quarter_start, mut month_start) = (None, None);
    if let (Some(q), Some(m)) = (quarter, month) {
        match (q.1.map(as_int), m.1.map(as_int)) {
            (Some(q), Some(m)) if (m - 1).div_euclid(3) + 1 != q => {
                return Err(Error(format!(
                    "QUARTER {q} and MONTH {m} conflict: month {m} is not in quarter {q}"
                )));
            }
            (Some(q), None) => month_start = Some((q - 1) * 3 + 1),
            (None, Some(m)) => quarter_start = Some((m - 1).div_euclid(3) + 1),
            _ => {}
        }
    }

    let mut parts: Vec<Part> = Vec::new();
    for &(unit, start, period) in &keywords {
        let period = period.map(as_int).or_else(|| {
            (unit != highest || unit == DateUnit::Quarter)
                .then(|| default_period(unit))
                .flatten()
        });
        let start = start.map(as_int).unwrap_or(match unit {
            DateUnit::Quarter => quarter_start.unwrap_or(1),
            DateUnit::Month => month_start.unwrap_or(1),
            _ => base(unit),
        });
        if let Some(p) = period {
            let low = base(unit);
            if start < low || start > low + p - 1 {
                return Err(Error(format!(
                    "the starting value of {} must be from {low} to {} with period {p}",
                    unit.name(),
                    low + p - 1
                )));
            }
        }
        parts.push(Part {
            unit,
            start,
            period,
            values: Vec::with_capacity(cases),
        });
    }
    if parts[0].period.is_some() {
        parts.insert(
            0,
            Part {
                unit: DateUnit::Cycle,
                start: 1,
                period: None,
                values: Vec::with_capacity(cases),
            },
        );
    }
    let step = spec.by.map_or(1, as_int);
    let lowest = parts.len() - 1;
    if let Some(p) = parts[lowest].period
        && p % step != 0
    {
        return Err(Error(format!(
            "BY {step} does not divide the period {p} of {}",
            parts[lowest].unit.name()
        )));
    }

    // with MONTH, the quarter follows the month and does not count on its own
    let has_month = month.is_some();
    let counted: Vec<usize> = (0..parts.len())
        .filter(|&i| !(has_month && parts[i].unit == DateUnit::Quarter))
        .collect();
    for case in 0..cases {
        let mut carry = case as i64 * step;
        for &i in counted.iter().rev() {
            let part = &mut parts[i];
            let value = match part.period {
                Some(p) => {
                    let position = part.start - base(part.unit) + carry;
                    carry = position.div_euclid(p);
                    base(part.unit) + position.rem_euclid(p)
                }
                None => {
                    let value = part.start + carry;
                    carry = 0;
                    value
                }
            };
            part.values.push(value);
        }
        if has_month && let Some(q) = parts.iter().position(|p| p.unit == DateUnit::Quarter) {
            let m = parts.iter().position(|p| p.unit == DateUnit::Month);
            let month = parts[m.expect("has_month")].values[case];
            parts[q].values.push((month - 1).div_euclid(3) + 1);
        }
    }

    Ok(parts)
}

/// DATE_ of every case, its width and its form ("MMM YYYY").
fn date_texts(parts: &[Part], cases: usize) -> (Vec<String>, usize, Vec<String>) {
    // DATE_: the parts in fixed widths
    let order: Vec<usize> = if parts.iter().any(|p| p.unit.is_long()) {
        let find = |unit| parts.iter().position(|p: &Part| p.unit == unit);
        let middle = find(DateUnit::Month).or(find(DateUnit::Quarter));
        let rest = (0..parts.len()).filter(|&i| parts[i].unit == DateUnit::Obs);
        [find(DateUnit::Cycle), middle, find(DateUnit::Year)]
            .into_iter()
            .flatten()
            .chain(rest)
            .collect()
    } else {
        (0..parts.len()).collect()
    };
    let widths: Vec<usize> = order
        .iter()
        .map(|&i| {
            let part = &parts[i];
            let nominal = match (part.unit, part.period) {
                (DateUnit::Month, _) => 3,
                (DateUnit::Year, _) if part.start < 100 => 2,
                (_, None) => 4,
                (unit, Some(p)) => digits(base(unit) + p - 1),
            };
            let largest = part.values.iter().map(|&v| digits(v)).max().unwrap_or(0);
            match part.unit {
                DateUnit::Month => 3,
                _ => nominal.max(largest),
            }
        })
        .collect();
    let form: Vec<String> = order
        .iter()
        .zip(&widths)
        .map(|(&i, &w)| letter(parts[i].unit).to_string().repeat(w))
        .collect();
    let texts: Vec<String> = (0..cases)
        .map(|case| {
            let pieces: Vec<String> = order
                .iter()
                .zip(&widths)
                .map(|(&i, &w)| match parts[i].unit {
                    DateUnit::Month => {
                        let m = parts[i].values[case];
                        MONTHS[(m - 1).clamp(0, 11) as usize].to_string()
                    }
                    _ => format!("{:>w$}", parts[i].values[case]),
                })
                .collect();
            pieces.join(" ")
        })
        .collect();
    let text_width = widths.iter().sum::<usize>() + widths.len().saturating_sub(1);

    (texts, text_width, form)
}

/// The case (from 0) of a date given as DATE keywords and values: the first case that
/// matches, or the last when `end`; cases past the file count on as DATE would. None when no
/// case up to `limit` matches.
pub(crate) fn date_case(
    spec: &DateSpec,
    point: &[(DateUnit, f64)],
    end: bool,
    limit: usize,
) -> Option<usize> {
    let parts = date_parts(spec, limit).ok()?;
    let columns: Vec<&Part> = point
        .iter()
        .map(|(unit, _)| parts.iter().find(|p| p.unit == *unit))
        .collect::<Option<_>>()?;
    let matches = |case: usize| {
        columns
            .iter()
            .zip(point)
            .all(|(part, (_, value))| part.values[case] as f64 == *value)
    };
    match end {
        false => (0..limit).find(|&c| matches(c)),
        true => (0..limit).rfind(|&c| matches(c)),
    }
}

/// Each date variable's name and values, DATE_'s texts and its width.
type DateColumns = (Vec<(String, Vec<Option<f64>>)>, Vec<String>, usize);

/// The values of every date variable of `spec` over `cases` cases, and DATE_; for cases added
/// at the end of the file.
pub(crate) fn date_columns(spec: &DateSpec, cases: usize) -> Result<DateColumns, Error> {
    let parts = date_parts(spec, cases)?;
    let (texts, width, _) = date_texts(&parts, cases);
    let numeric = parts
        .iter()
        .map(|p| {
            let values = p.values.iter().map(|&v| Some(v as f64)).collect();
            (format!("{}_", p.unit.name()), values)
        })
        .collect();
    Ok((numeric, texts, width))
}

/// The period, in cases, that the DATE variables of `data` give: the period of the lowest
/// date variable, divided by its BY step, read from its label. None when that variable has no
/// period or there is none.
pub(crate) fn date_period(data: &statscraft_data::Dataset) -> Option<usize> {
    let mut units = DateUnit::ALL;
    units.sort();
    let lowest = units.iter().rev().find_map(|unit| {
        let name = format!("{}_", unit.name());
        data.variable(&name).ok()
    })?;
    let label = lowest.label()?;
    let rest = label.split(", period ").nth(1)?;
    let mut words = rest.split(" by ");
    let period: usize = words.next()?.trim().parse().ok()?;
    let step: usize = words.next().map_or(Some(1), |s| s.trim().parse().ok())?;
    (step > 0 && period.is_multiple_of(step) && period / step > 0).then_some(period / step)
}
