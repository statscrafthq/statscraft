//! SORT CASES, and the ordering of cases that AGGREGATE /BREAK shares with it.

use crate::{Engine, Error};
use statscraft_data::Values;
use statscraft_syntax::SortKey;
use std::cmp::Ordering;

/// The key variables of a sort, each with its direction (true is descending).
pub(crate) struct Keys<'a> {
    pub names: Vec<String>,
    columns: Vec<(Values<'a>, bool)>,
}

impl Keys<'_> {
    /// System-missing is lower than every number.
    pub(crate) fn compare(&self, a: usize, b: usize) -> Ordering {
        for (values, descending) in &self.columns {
            let ordering = match values {
                Values::Numeric(v) => v.get(a).partial_cmp(&v.get(b)).unwrap_or(Ordering::Equal),
                // shortcut: strings compare by their bytes without the padding, so a control
                // character sorts above the end of a shorter string where a space-padded
                // comparison puts it below; pad when such data turns up
                Values::String(v) => v.get(a).cmp(v.get(b)),
            };
            if ordering != Ordering::Equal {
                return if *descending {
                    ordering.reverse()
                } else {
                    ordering
                };
            }
        }
        Ordering::Equal
    }

    /// The case numbers in sorted order. Cases with equal keys keep their order.
    pub(crate) fn order(&self, case_count: usize) -> Vec<usize> {
        let mut order: Vec<usize> = (0..case_count).collect();
        order.sort_by(|&a, &b| self.compare(a, b));
        order
    }
}

impl Engine {
    pub(crate) fn sort_keys(&self, keys: &[SortKey]) -> Result<Keys<'_>, Error> {
        let data = self.active()?;
        let (mut names, mut columns) = (Vec::new(), Vec::new());
        for key in keys {
            for name in self.expand(&key.vars)? {
                // a key given twice cannot change the order any more
                if names.contains(&name) {
                    continue;
                }
                columns.push((data.variable(&name)?.values(), key.descending));
                names.push(name);
            }
        }
        Ok(Keys { names, columns })
    }

    pub(crate) fn sort_cases(&mut self, keys: &[SortKey]) -> Result<(), Error> {
        let order = self.sort_keys(keys)?.order(self.active()?.case_count());
        Ok(self.active_mut()?.reorder_cases(&order)?)
    }
}
