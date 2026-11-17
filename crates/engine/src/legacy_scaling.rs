//! HOMALS, PRINCALS and ANACOR: the older optimal scaling commands, run through the engines
//! of MULTIPLE CORRESPONDENCE, CATPCA and CORRESPONDENCE, which fit the same models; and what
//! they share with OVERALS: the data read as categories 1 to a stated largest value and the
//! matrix file.
//!
//! Rules from the Command Syntax Reference, in our words:
//! - Every whole value from 1 to the largest category named on VARIABLES is a category; a
//!   fraction is truncated, user-missing values count as values, and anything outside that
//!   range (system-missing included) is missing. String variables are not allowed.
//! - MATRIX=OUT writes one case per category of each analysis variable (and one per variable
//!   for its loadings): ROWTYPE_, LEVEL, VARNAME_, for PRINCALS and OVERALS VARTYPE_, for
//!   OVERALS SET_, and DIM1 to DIMn. An asterisk replaces the active dataset, after which
//!   nothing is saved to it.

use crate::optimal_scaling::{Coded, plain_number};
use crate::{Engine, Error, Table};
use statscraft_data::{Dataset, Values, Variable};
use statscraft_syntax::{Catpca, Correspondence, MultipleCorrespondence, VarItem};
use std::collections::BTreeMap;

/// The largest category of every variable named in `ranges`, by upper-case name.
pub(crate) fn largest_categories(
    who: &str,
    data: &Dataset,
    ranges: &[(Vec<VarItem>, usize)],
) -> Result<BTreeMap<String, usize>, Error> {
    let mut out = BTreeMap::new();
    for (items, max) in ranges {
        for name in crate::expand_in(data, items)? {
            if out.insert(name.to_uppercase(), *max).is_some() {
                return Err(Error(format!("{who}: VARIABLES names {name} twice")));
            }
        }
    }
    Ok(out)
}

/// A numeric variable read as categories 1 to `max`.
pub(crate) fn code_in_range(
    who: &str,
    var: &Variable,
    max: usize,
    shown: &crate::shown::Shown,
) -> Result<Coded, Error> {
    let Values::Numeric(values) = var.values() else {
        return Err(Error(format!(
            "{who}: {} is a string variable; recode it into whole numbers first, for example with AUTORECODE",
            var.name()
        )));
    };
    let codes: Vec<Option<f64>> = values
        .iter()
        .map(|v| v.map(f64::trunc).filter(|t| *t >= 1.0 && *t <= max as f64))
        .collect();
    let labels = codes
        .iter()
        .flatten()
        .map(|c| (*c as i64, (shown.number(var, *c), plain_number(*c))))
        .collect();
    Ok(Coded {
        name: var.name().to_string(),
        title: shown.variable(var),
        codes,
        labels,
    })
}

/// One case of a matrix file.
pub(crate) struct MatrixRow {
    pub(crate) rowtype: &'static str,
    pub(crate) level: String,
    pub(crate) varname: String,
    pub(crate) vartype: Option<&'static str>,
    pub(crate) set: Option<usize>,
    pub(crate) dims: Vec<f64>,
}

/// The VARTYPE_ value of a level.
pub(crate) fn vartype(multiple: bool, nominal: bool, ordinal: bool) -> &'static str {
    match (multiple, nominal, ordinal) {
        (true, _, _) => "MULTIPLE",
        (_, true, _) => "SINGLE N",
        (_, _, true) => "ORDINAL",
        _ => "NUMERICAL",
    }
}

impl Engine {
    /// Writes MATRIX=OUT: to a file or dataset, or (None) as the dataset that replaces the
    /// active one once the procedure is done.
    pub(crate) fn write_matrix(
        &mut self,
        target: &Option<String>,
        rows: &[MatrixRow],
        p: usize,
    ) -> Result<(), Error> {
        let mut out = Dataset::new(rows.len());
        let mut strings: Vec<(&str, Vec<String>)> = vec![
            (
                "ROWTYPE_",
                rows.iter().map(|r| r.rowtype.to_string()).collect(),
            ),
            ("LEVEL", rows.iter().map(|r| r.level.clone()).collect()),
            ("VARNAME_", rows.iter().map(|r| r.varname.clone()).collect()),
        ];
        if rows.iter().any(|r| r.vartype.is_some()) {
            strings.push((
                "VARTYPE_",
                rows.iter()
                    .map(|r| r.vartype.unwrap_or_default().to_string())
                    .collect(),
            ));
        }
        for (name, values) in strings {
            let width = values.iter().map(String::len).max().unwrap_or(1).max(8);
            out.set_string_values(name, width, values)?;
        }
        if rows.iter().any(|r| r.set.is_some()) {
            let sets = rows.iter().map(|r| r.set.map(|s| s as f64)).collect();
            out.set_numeric_values("SET_", sets)?;
        }
        for s in 0..p {
            let name = format!("DIM{}", s + 1);
            out.set_numeric_values(&name, rows.iter().map(|r| r.dims.get(s).copied()).collect())?;
            out.set_format(&name, "F10.5")?;
        }
        match target {
            Some(file) => self.write_data_file(file, out),
            None => {
                self.replacement = Some(out);
                Ok(())
            }
        }
    }

    /// Runs `run` and puts `who` for `engine` in its error and in the warnings it adds.
    fn named_as(
        &mut self,
        who: &str,
        engine: &str,
        run: impl FnOnce(&mut Self) -> Result<Vec<Table>, Error>,
    ) -> Result<Vec<Table>, Error> {
        let before = self.warnings.len();
        let result = run(self);
        for warning in &mut self.warnings[before..] {
            *warning = warning.replace(engine, who);
        }
        // the labels of the saved variables name the command too
        let tag = format!("({engine} ");
        if let Some(data) = self.dataset.as_mut() {
            let relabel: Vec<(String, String)> = data
                .variables()
                .filter_map(|v| {
                    let label = v.label()?;
                    label.contains(&tag).then(|| {
                        (
                            v.name().to_string(),
                            label.replace(&tag, &format!("({who} ")),
                        )
                    })
                })
                .collect();
            for (name, label) in relabel {
                data.set_variable_label(&name, Some(&label))?;
            }
        }
        result.map_err(|e| Error(e.0.replace(engine, who)))
    }

    pub(crate) fn homals(&mut self, command: &MultipleCorrespondence) -> Result<Vec<Table>, Error> {
        self.named_as("HOMALS", "MULTIPLE CORRESPONDENCE", |e| {
            e.multiple_correspondence(command)
        })
    }

    pub(crate) fn princals(&mut self, command: &Catpca) -> Result<Vec<Table>, Error> {
        self.named_as("PRINCALS", "CATPCA", |e| e.catpca(command))
    }

    pub(crate) fn anacor(&mut self, command: &Correspondence) -> Result<Vec<Table>, Error> {
        self.named_as("ANACOR", "CORRESPONDENCE", |e| e.correspondence(command))
    }

    /// OUTFILE of CORRESPONDENCE and MATRIX OUT of ANACOR: `*` replaces the active dataset
    /// after the procedure.
    pub(crate) fn write_out(&mut self, file: &str, out: Dataset) -> Result<(), Error> {
        match file {
            "*" => {
                self.replacement = Some(out);
                Ok(())
            }
            _ => self.write_data_file(file, out),
        }
    }
}
