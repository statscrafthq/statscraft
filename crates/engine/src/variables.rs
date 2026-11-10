//! RENAME VARIABLES, and what the commands that create or rename variables share.

use crate::{Engine, Error};
use statscraft_data::Dataset;
use statscraft_syntax::VarItem;

/// The values a command computed for one variable; a string variable comes with its width.
pub(crate) enum Column {
    Numeric(Vec<Option<f64>>),
    String(usize, Vec<String>),
}

/// Names for new variables. `v1 TO v3` counts the number at the end of the name up.
pub(crate) fn new_names(items: &[VarItem]) -> Result<Vec<String>, Error> {
    let mut names = Vec::new();
    for item in items {
        match item {
            VarItem::Name(name) => names.push(name.clone()),
            VarItem::All => {
                return Err(Error(
                    "ALL cannot name new variables: list the new names".into(),
                ));
            }
            VarItem::Range(first, last) => {
                names.extend(statscraft_syntax::numbered_names(first, last).map_err(Error)?)
            }
        }
    }
    Ok(names)
}

/// The first name that occurs twice, ignoring case.
pub(crate) fn duplicate(names: &[String]) -> Option<&String> {
    let lower: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
    (1..names.len())
        .find(|&i| lower[..i].contains(&lower[i]))
        .map(|i| &names[i])
}

/// `count` temporary names `@sc.<purpose>.<n>` that no variable of `data` and none of `reserved`
/// has, ignoring case: users may name their own variables like that.
pub(crate) fn unused_names(
    data: &Dataset,
    reserved: &[String],
    purpose: &str,
    count: usize,
) -> Vec<String> {
    (0..)
        .map(|i| format!("@sc.{purpose}.{i}"))
        .filter(|name| {
            data.variable_index(name).is_none()
                && !reserved.iter().any(|r| r.eq_ignore_ascii_case(name))
        })
        .take(count)
        .collect()
}

impl Engine {
    /// Runs a command that changes the dataset in several steps. An error undoes all of them.
    pub(crate) fn all_or_nothing(
        &mut self,
        run: impl FnOnce(&mut Self) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let before = self.dataset.clone();
        let result = run(self);
        if result.is_err() {
            self.dataset = before;
        }
        result
    }

    pub(crate) fn rename_variables(
        &mut self,
        groups: &[(Vec<VarItem>, Vec<VarItem>)],
    ) -> Result<(), Error> {
        let (mut old, mut new) = (Vec::new(), Vec::new());
        for (old_items, new_items) in groups {
            let (group_old, group_new) = (self.expand(old_items)?, new_names(new_items)?);
            if group_old.len() != group_new.len() {
                return Err(Error(format!(
                    "{} old names ({}) but {} new names ({}); give one new name per variable",
                    group_old.len(),
                    group_old.join(" "),
                    group_new.len(),
                    group_new.join(" ")
                )));
            }
            old.extend(group_old);
            new.extend(group_new);
        }
        if let Some(name) = duplicate(&old) {
            return Err(Error(format!("{name} is renamed twice")));
        }
        if let Some(name) = duplicate(&new) {
            return Err(Error(format!(
                "the new name {name} is given to two variables"
            )));
        }
        // The dataset renames one variable at a time and refuses a name that is in use, so
        // names that swap have to pass through names nothing else uses.
        let parked = unused_names(self.active()?, &new, "renaming", old.len());
        let data = self.active_mut()?;
        for (name, parked) in old.iter().zip(&parked) {
            data.rename_variable(name, parked)?;
        }
        for (parked, name) in parked.iter().zip(&new) {
            data.rename_variable(parked, name)?;
        }
        Ok(())
    }
}
