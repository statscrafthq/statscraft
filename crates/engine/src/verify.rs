//! VERIFY: a report on the DATE variables and the USE and PREDICT periods of the active dataset.
//!
//! Rules noted from the description of VERIFY:
//! - The first and last case of the file are listed by case number and by DATE_; so are the
//!   first and last case of the USE period (the whole file without USE) and of the PREDICT
//!   period (nothing without PREDICT).
//! - Each date variable gets a count of missing or invalid values, and a flag where it is not
//!   nested in the next higher date variable, starts or ends a cycle at an unexpected value,
//!   or steps by more than it should.
//! - With one numeric series, its values at those cases and the number of cases where it is
//!   missing are listed.
//!
//! Choices where the manual is silent:
//! - A value is invalid when it is not a whole number or lies outside the period of its
//!   variable (a year has no period, so only missing or fractional values count).
//! - Nested: when the higher variable changes the lower one starts a new cycle, and when the
//!   lower one starts a new cycle the higher one changes. MONTH is nested in YEAR, not in
//!   QUARTER, which follows it.
//! - Begin and end are the first and last case of the file: a cycle starts at the lowest value
//!   of its variable and ends at the highest it reaches with the step of the lowest variable.
//! - Steps: the lowest variable moves by BY (1 without it) or wraps; a higher one stays or
//!   moves up by 1, or wraps. Only the first case of each problem is named, with the count.
//! - User-missing values of the series are missing unless TSET MISSING=INCLUDE.
//! - A forecast case past the file shows the DATE_ text that DATE would give it.

use crate::npar_tests::text;
use crate::regression::{plain, table};
use crate::timeseries::read_series;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::Dataset;
use statscraft_syntax::DateUnit;

/// One date variable of the file, highest first.
struct DateVar {
    unit: DateUnit,
    name: String,
    values: Vec<Option<f64>>,
    base: i64,
    period: Option<i64>,
}

/// The period and the BY step a date variable's label states ("MONTH, period 12 by 3").
fn label_period(label: Option<&str>) -> (Option<i64>, i64) {
    let Some(rest) = label.and_then(|l| l.split(", period ").nth(1)) else {
        return (None, 1);
    };
    let mut words = rest.split(" by ");
    let period = words.next().and_then(|w| w.trim().parse().ok());
    let step = words.next().and_then(|w| w.trim().parse().ok());
    (period, step.unwrap_or(1))
}

fn date_vars(data: &Dataset) -> (Vec<DateVar>, i64) {
    let mut units = DateUnit::ALL;
    units.sort();
    let mut step = 1;
    let mut vars = Vec::new();
    for unit in units {
        let name = format!("{}_", unit.name());
        let Ok(variable) = data.variable(&name) else {
            continue;
        };
        let Ok(values) = variable.numeric() else {
            continue;
        };
        let (period, by) = label_period(variable.label());
        step = by;
        let base = match unit {
            DateUnit::Hour | DateUnit::Minute | DateUnit::Second => 0,
            _ => 1,
        };
        vars.push(DateVar {
            unit,
            name,
            values: (0..data.case_count()).map(|k| values.get(k)).collect(),
            base,
            period,
        });
    }
    (vars, step)
}

/// What is wrong with one date variable: its count of invalid values and the flags.
fn check(vars: &[DateVar], i: usize, step: i64) -> (usize, Vec<String>) {
    let var = &vars[i];
    let n = var.values.len();
    let invalid = |v: Option<f64>| match v {
        None => true,
        Some(v) => {
            v.fract() != 0.0
                || var
                    .period
                    .is_some_and(|p| v < var.base as f64 || v > (var.base + p - 1) as f64)
        }
    };
    let bad = var.values.iter().filter(|&&v| invalid(v)).count();
    let lowest = i + 1 == vars.len();
    let by = if lowest { step } else { 1 };
    let mut flags = Vec::new();

    // the step from one case to the next
    let mut jumps = Vec::new();
    for k in 1..n {
        let (Some(a), Some(b)) = (var.values[k - 1], var.values[k]) else {
            continue;
        };
        let d = (b - a) as i64;
        let wrap = var.period.map(|p| by - p);
        let fine = if lowest {
            d == by || wrap == Some(d)
        } else {
            d == 0 || d == 1 || var.period.is_some_and(|p| d == 1 - p)
        };
        if !fine {
            jumps.push((k, a, b));
        }
    }
    if let Some(&(k, a, b)) = jumps.first() {
        flags.push(format!(
            "steps from {a} to {b} at case {}{}",
            k + 1,
            more(jumps.len())
        ));
    }

    // nesting in the next higher variable (MONTH skips QUARTER, which follows it)
    let higher = (0..i)
        .rev()
        .find(|&j| !(var.unit == DateUnit::Month && vars[j].unit == DateUnit::Quarter));
    if let (Some(j), Some(_)) = (higher, var.period) {
        let up = &vars[j].values;
        let mut broken = Vec::new();
        for k in 1..n {
            let (Some(a), Some(b), Some(ha), Some(hb)) =
                (var.values[k - 1], var.values[k], up[k - 1], up[k])
            else {
                continue;
            };
            let wrapped = b < a;
            if wrapped != (hb != ha) {
                broken.push(k);
            }
        }
        if let Some(&k) = broken.first() {
            flags.push(format!(
                "not nested in {} at case {}{}",
                vars[j].name,
                k + 1,
                more(broken.len())
            ));
        }
    }

    // where the file begins and ends in the cycle
    if let Some(p) = var.period {
        let end = (var.base + p - by.min(p)) as f64;
        if let Some(first) = var.values.first().copied().flatten()
            && first != var.base as f64
        {
            flags.push(format!("begins at {first}, not {}", var.base));
        }
        if let Some(last) = var.values.last().copied().flatten()
            && last != end
        {
            flags.push(format!("ends at {last}, not {end}"));
        }
    }
    (bad, flags)
}

fn more(count: usize) -> String {
    match count {
        1 => String::new(),
        n => format!(" ({n} times in all)"),
    }
}

impl Engine {
    pub(crate) fn verify(&mut self, series: Option<&str>) -> Result<Vec<Table>, Error> {
        let data = self.active()?;
        let n = data.case_count();
        if n == 0 {
            return Err(Error("VERIFY: there are no cases".into()));
        }
        let (vars, step) = date_vars(data);
        let texts = data.variable("DATE_").ok().and_then(|v| v.string().ok());
        let spec = self.date_spec.clone();
        let date_text = |k: usize| -> String {
            if k < n
                && let Some(t) = &texts
            {
                return t.get(k).to_string();
            }
            spec.as_ref()
                .and_then(|s| crate::date::date_columns(s, k + 1).ok())
                .and_then(|(_, t, _)| t.get(k).cloned())
                .unwrap_or_default()
        };

        // the periods: the whole file without USE, nothing without PREDICT
        let (use_first, use_last, use_note) = match self.use_bounds(data, None) {
            Ok(Some((a, b))) => (a, b, None),
            Ok(None) => (0, n - 1, None),
            Err(e) => (0, n - 1, Some(e.0)),
        };
        let predict = self.predict_bounds(data, use_last, None);
        let case = |k: usize| Cell::Number((k + 1) as f64);
        let mut cases: Vec<(&str, Option<usize>)> = vec![
            ("File, first case", Some(0)),
            ("File, last case", Some(n - 1)),
        ];
        let mut rows = Vec::new();
        let put = |label: &str, k: Option<usize>, rows: &mut Vec<Vec<Cell>>| {
            rows.push(match k {
                Some(k) => vec![text(label), case(k), text(date_text(k))],
                None => vec![text(label), Cell::Empty, Cell::Empty],
            });
        };
        cases.push(("USE, first case", Some(use_first)));
        cases.push(("USE, last case", Some(use_last)));
        let mut predict_points = Vec::new();
        match &predict {
            Ok(Some((a, b))) => {
                cases.push(("PREDICT, first case", Some(*a)));
                cases.push(("PREDICT, last case", Some(*b)));
                predict_points = vec![*a, *b];
            }
            Ok(None) => {}
            Err(_) => cases.push(("PREDICT, not valid for this file", None)),
        }
        for (label, k) in &cases {
            put(label, *k, &mut rows);
        }
        let mut tables = vec![table(
            "Cases and periods",
            1,
            plain(&["", "Case", "DATE_"]),
            rows,
            None,
        )];
        let mut notes = Vec::new();
        notes.extend(use_note.map(|m| format!("VERIFY: {m}")));
        if let Err(e) = &predict {
            notes.push(format!("VERIFY: {}", e.0));
        }

        if vars.is_empty() {
            notes.push("VERIFY: the file has no DATE variables; define them with DATE".into());
        } else {
            let mut rows = Vec::new();
            for i in 0..vars.len() {
                let (bad, flags) = check(&vars, i, step);
                let verdict = if flags.is_empty() {
                    "no problem".to_string()
                } else {
                    format!("* {}", flags.join("; "))
                };
                rows.push(vec![
                    text(vars[i].name.clone()),
                    Cell::Number(bad as f64),
                    text(verdict),
                ]);
            }
            tables.push(table(
                "Date variables",
                1,
                plain(&["Variable", "Missing or invalid", "Check"]),
                rows,
                None,
            ));
        }

        if let Some(name) = series {
            let data = self.active()?;
            let (values, user, system) = read_series(data, name, self.tset.include, "VERIFY")?;
            let name = data.variable(name)?.name().to_string();
            let value = |k: usize| match values.get(k).copied().flatten() {
                Some(v) => Cell::Number(v),
                None if k < n => text("missing"),
                None => text("not in the file"),
            };
            let mut points = vec![
                ("File, first case", 0),
                ("File, last case", n - 1),
                ("USE, first case", use_first),
                ("USE, last case", use_last),
            ];
            if let [a, b] = predict_points[..] {
                points.push(("PREDICT, first case", a));
                points.push(("PREDICT, last case", b));
            }
            let mut rows: Vec<Vec<Cell>> = points
                .into_iter()
                .map(|(label, k)| vec![text(label), case(k), value(k)])
                .collect();
            rows.push(vec![
                text("Cases with a missing value"),
                Cell::Empty,
                Cell::Number((user + system) as f64),
            ]);
            tables.push(table(
                &format!("Series {name}"),
                1,
                plain(&["", "Case", "Value"]),
                rows,
                None,
            ));
        }
        for note in notes {
            self.warn(note);
        }
        Ok(tables)
    }
}
