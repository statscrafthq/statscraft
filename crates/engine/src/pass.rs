//! The data pass: what happens when a command reads the data.

use crate::Engine;
use statscraft_data::{Dataset, Measure, Values};

/// The measurement level a variable of unknown level gets from its data, by the first rule
/// that applies: dollar formats and dates are scale; a variable without valid values is
/// nominal; a fraction, a negative value, no value below 10000, at least `scale_min` (SET
/// SCALEMIN) different values or no value below 10 make it scale; anything else is nominal.
/// Strings are nominal.
pub(crate) fn default_measure(data: &Dataset, index: usize, scale_min: usize) -> Measure {
    let variable = data.variable_at(index);
    let Values::Numeric(values) = variable.values() else {
        return Measure::Nominal;
    };
    let format = variable.format().to_uppercase();
    let kind = format.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.');
    let dates = [
        "DATE", "ADATE", "EDATE", "JDATE", "SDATE", "QYR", "MOYR", "WKYR", "DATETIME", "YMDHMS",
        "TIME", "MTIME", "DTIME",
    ];
    if kind == "DOLLAR" || kind.starts_with("CC") || dates.contains(&kind) {
        return Measure::Scale;
    }
    let mut valid: Vec<f64> = values
        .iter()
        .flatten()
        .filter(|v| !variable.is_user_missing(*v))
        .collect();
    if valid.is_empty() {
        return Measure::Nominal;
    }
    if valid.iter().any(|v| v.fract() != 0.0 || *v < 0.0) || valid.iter().all(|v| *v >= 10_000.0) {
        return Measure::Scale;
    }
    valid.sort_by(f64::total_cmp);
    valid.dedup();
    if valid.len() >= scale_min || valid.iter().all(|v| *v >= 10.0) {
        Measure::Scale
    } else {
        Measure::Nominal
    }
}

impl Engine {
    /// Variables of unknown measurement level get one from their values, as data read from
    /// text do.
    pub(crate) fn default_measures(&mut self) {
        let scale_min = self.scale_min();
        let Some(data) = self.dataset.as_mut() else {
            return;
        };
        let unknown: Vec<usize> = (0..data.variable_count())
            .filter(|&i| data.variable_at(i).measure() == Measure::Unknown)
            .collect();
        for index in unknown {
            let measure = default_measure(data, index, scale_min);
            let name = data.variable_at(index).name().to_string();
            data.set_measure(&name, measure)
                .expect("the variable was just found");
        }
    }
}
