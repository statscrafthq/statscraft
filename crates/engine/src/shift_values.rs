//! SHIFT VALUES: a new variable with the value of a variable from an earlier or later case.
//!
//! Rules noted from the Command Syntax Reference:
//! - LEAD=n takes the value from the case n after, LAG=n from n before, SHIFT=n from n after
//!   (n below 0: before). With 0 the value is copied in place.
//! - Under SPLIT FILE a value never comes from another group. A filter is ignored.
//! - The first or last n cases of the data, or of each group, have no source: the result is
//!   system-missing (an empty string for a string variable). User-missing values are copied as
//!   they are.
//! - The new variable has the dictionary entry of the old one (format, value labels, missing
//!   values, measurement level) without its custom attributes, and a label saying what was
//!   shifted. The new name must not exist, which includes one an earlier part of the command made.
//!
//! shortcut: the data is cloned once per specification to read from while writing, so a very
//! wide, very long dataset briefly doubles in memory; upgrade if that bites.

use crate::{Engine, Error, Table};
use statscraft_syntax::ShiftSpec;

impl Engine {
    pub(crate) fn shift_values(&mut self, specs: &[ShiftSpec]) -> Result<Vec<Table>, Error> {
        self.begin_pass()?;
        let before = self.dataset.clone();
        let result = self.shift_all(specs);
        if result.is_err() {
            self.dataset = before.clone();
        }
        self.end_pass(before.as_ref());
        result
    }

    fn shift_all(&mut self, specs: &[ShiftSpec]) -> Result<Vec<Table>, Error> {
        for spec in specs {
            let runs = self.split_runs()?;
            let data = self.active()?;
            data.variable(&spec.variable)?;
            if data.variable(&spec.result).is_ok() {
                return Err(Error(format!(
                    "{} already exists: the RESULT of SHIFT VALUES must be a new variable",
                    spec.result
                )));
            }
            let mut source = vec![None; data.case_count()];
            for run in &runs {
                for (k, &case) in run.iter().enumerate() {
                    let from = k as i64 + spec.offset;
                    if (0..run.len() as i64).contains(&from) {
                        source[case] = Some(run[from as usize]);
                    }
                }
            }
            let from = data.clone();
            let label = match spec.offset {
                0 => format!("{} from the same case", spec.variable),
                n if n > 0 => format!("{} from {n} case(s) later", spec.variable),
                n => format!("{} from {} case(s) earlier", spec.variable, -n),
            };
            let data = self.active_mut()?;
            data.copy_variable_as(&from, &spec.variable, &spec.result, &source)?;
            data.set_variable_attributes(&spec.result, Vec::new())?;
            data.set_variable_label(&spec.result, Some(&label))?;
        }
        Ok(Vec::new())
    }
}
