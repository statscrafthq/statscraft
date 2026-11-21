//! Matrix data files for the procedures that read or write them through MATRIX=IN and
//! MATRIX=OUT (REGRESSION, PARTIAL CORR, RELIABILITY, ONEWAY).
//!
//! The file: the split variables, ROWTYPE_ (A8), for ONEWAY its independent variable,
//! VARNAME_ (as wide as the longest variable name, at least A8) and the variables (F10.4).
//! The layout is that of MATRIX DATA, so a file made there reads the same as one written here.
//! N is one record without a VARNAME_ when every coefficient rests on the same number of cases
//! and a matrix of records (with the variable names) when pairwise missing values make them
//! differ.
//!
//! Choices where the manual is silent:
//! - A reader takes the numeric variables after VARNAME_ as the variables of the file and the
//!   variables before ROWTYPE_ as split variables: a file of several split groups gives the
//!   rows of the group that runs. A triangle fills the other half.
//! - Records of types the procedure does not know are skipped.

use crate::cluster::value_text;
use crate::crosstabs::Key;
use crate::matrix::Matrix;
use crate::{Engine, Error, Table};
use statscraft_data::{Dataset, Values};
use statscraft_syntax::Command;

/// Matrix records to write: a type, the VARNAME_ and the values (NaN is missing).
pub(crate) struct MatrixRows {
    pub names: Vec<String>,
    pub labels: Vec<Option<String>>,
    /// A variable between ROWTYPE_ and VARNAME_ (ONEWAY's independent variable): its name and
    /// its value in each record.
    pub factor: Option<(String, Vec<Option<f64>>)>,
    pub rows: Vec<(String, String, Vec<f64>)>,
}

impl MatrixRows {
    pub(crate) fn new(names: Vec<String>, labels: Vec<Option<String>>) -> Self {
        Self {
            names,
            labels,
            factor: None,
            rows: Vec::new(),
        }
    }

    pub(crate) fn push(&mut self, kind: &str, varname: &str, values: Vec<f64>) {
        self.rows
            .push((kind.to_string(), varname.to_string(), values));
    }

    /// The records of a square matrix, one per variable.
    pub(crate) fn matrix(&mut self, kind: &str, m: &Matrix) {
        for (i, name) in self.names.clone().iter().enumerate() {
            self.push(kind, name, m[i].clone());
        }
    }

    /// N: one record when the counts of all pairs agree, else a matrix.
    pub(crate) fn counts(&mut self, n: &Matrix) {
        let first = n.first().and_then(|r| r.first()).copied();
        if n.iter().flatten().all(|v| Some(*v) == first) {
            let row = vec![first.unwrap_or(f64::NAN); self.names.len()];
            self.push("N", "", row);
        } else {
            self.matrix("N", n);
        }
    }
}

/// The records of a matrix data file read for one split group.
pub(crate) struct Materials {
    pub data: Dataset,
    rows: Vec<usize>,
    rowtype: String,
    varname: String,
    /// The variables of the file, in its order.
    pub all: Vec<String>,
}

impl Materials {
    fn of_kind<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = usize> + 'a {
        self.rows.iter().copied().filter(move |&r| {
            let var = self.data.variable(&self.rowtype).expect("ROWTYPE_");
            value_text(&var, r).eq_ignore_ascii_case(kind)
        })
    }

    fn number(&self, name: &str, row: usize) -> f64 {
        match self.data.variable(name).map(|v| v.values()) {
            Ok(Values::Numeric(v)) => v.get(row).unwrap_or(f64::NAN),
            _ => f64::NAN,
        }
    }

    /// The first record of `kind` as a vector over `names`; None when there is none.
    /// An error when the file has variables between ROWTYPE_ and VARNAME_ (grouping
    /// variables), which this command does not read as split variables yet.
    pub(crate) fn no_grouping(&self, who: &str) -> Result<(), Error> {
        let names: Vec<&str> = self.data.variable_names().collect();
        let at = |n: &str| names.iter().position(|x| *x == n);
        let (Some(r), Some(v)) = (at(&self.rowtype), at(&self.varname)) else {
            return Ok(());
        };
        match names.get(r + 1..v) {
            Some([first, ..]) => Err(Error(format!(
                "{who} /MATRIX=IN: the matrix file has the variable {first} between ROWTYPE_ and VARNAME_ (grouping variables), which is not supported yet"
            ))),
            _ => Ok(()),
        }
    }

    pub(crate) fn vector(&self, kind: &str, names: &[String]) -> Option<Vec<f64>> {
        let row = self.of_kind(kind).next()?;
        Some(names.iter().map(|n| self.number(n, row)).collect())
    }

    /// The records of `kind` as a square matrix over `names`, rows picked by VARNAME_ and a
    /// triangle mirrored; None when there are no such records.
    pub(crate) fn matrix(&self, kind: &str, names: &[String]) -> Result<Option<Matrix>, String> {
        let varname = self.data.variable(&self.varname).expect("VARNAME_");
        let rows: Vec<(String, usize)> = self
            .of_kind(kind)
            .map(|r| (value_text(&varname, r).to_uppercase(), r))
            .collect();
        if rows.is_empty() {
            return Ok(None);
        }
        let p = names.len();
        let mut m = vec![vec![f64::NAN; p]; p];
        for (i, name) in names.iter().enumerate() {
            if let Some((_, row)) = rows.iter().find(|(v, _)| *v == name.to_uppercase()) {
                for (j, other) in names.iter().enumerate() {
                    m[i][j] = self.number(other, *row);
                }
            }
        }
        let seen = m.clone();
        for (i, row) in m.iter_mut().enumerate() {
            for (j, v) in row.iter_mut().enumerate() {
                if v.is_nan() {
                    *v = seen[j][i];
                }
            }
        }
        if let Some((i, j)) = (0..p)
            .flat_map(|i| (0..p).map(move |j| (i, j)))
            .find(|&(i, j)| m[i][j].is_nan())
        {
            return Err(format!(
                "the matrix has no {kind} value for {} with {}",
                names[i], names[j]
            ));
        }
        Ok(Some(m))
    }

    /// N as the count behind each pair: a matrix of records as it is, a single record as the
    /// smaller count of the two variables.
    pub(crate) fn counts(&self, names: &[String]) -> Result<Option<Matrix>, String> {
        let varname = self.data.variable(&self.varname).expect("VARNAME_");
        let by_variable = self
            .of_kind("N")
            .any(|r| !value_text(&varname, r).trim().is_empty());
        if by_variable {
            return self.matrix("N", names);
        }
        Ok(self.vector("N", names).map(|n| {
            names
                .iter()
                .enumerate()
                .map(|(i, _)| (0..names.len()).map(|j| n[i].min(n[j])).collect())
                .collect()
        }))
    }
}

impl Engine {
    /// Runs a procedure whose runs (one per split group) each leave a matrix for MATRIX=OUT,
    /// and writes them to `target` when it is done. Nothing stays behind when it fails.
    pub(crate) fn procedure_with_matrix_out(
        &mut self,
        command: &Command,
        target: &Option<String>,
    ) -> Result<Vec<Table>, Error> {
        let split = self.active()?.split_file().cloned();
        self.matrix_parts = (split, Vec::new());
        match self.procedure(command) {
            Ok(tables) => {
                self.finish_matrix_out(target)?;
                Ok(tables)
            }
            Err(error) => {
                self.matrix_parts = (None, Vec::new());
                Err(error)
            }
        }
    }

    /// The matrix materials MATRIX=IN names: the active dataset for `*`, else the file or
    /// dataset. `who` names the command in messages.
    pub(crate) fn matrix_materials(
        &self,
        who: &str,
        source: Option<&String>,
    ) -> Result<Materials, Error> {
        let data = match source {
            None => self.active()?.clone(),
            Some(name) => self.data_file(name)?,
        };
        let string = |name: &str| {
            data.variables()
                .find(|v| v.name().eq_ignore_ascii_case(name) && v.is_string())
                .map(|v| v.name().to_string())
                .ok_or_else(|| {
                    Error(format!(
                        "{who} /MATRIX=IN needs the string variable {name} in the matrix file"
                    ))
                })
        };
        let (rowtype, varname) = (string("ROWTYPE_")?, string("VARNAME_")?);
        let at = |name: &str| data.variable_index(name).expect("found");
        let all: Vec<String> = data
            .variables()
            .skip(at(&varname) + 1)
            .filter(|v| !v.is_string())
            .map(|v| v.name().to_string())
            .collect();
        // a file of several split groups: the rows of the group that runs; only the variables
        // before ROWTYPE_ are split variables of the file
        let group: Vec<(String, Option<Key>)> = match (source, &self.split_group) {
            (Some(_), Some(split)) => split
                .iter()
                .filter(|(name, _)| data.variable_index(name).is_some_and(|i| i < at(&rowtype)))
                .cloned()
                .collect(),
            _ => Vec::new(),
        };
        let in_group = |case: usize| {
            group.iter().all(|(name, key)| {
                let Ok(v) = data.variable(name) else {
                    return false;
                };
                match (v.values(), key) {
                    (Values::Numeric(values), Some(Key::Number(x))) => values.get(case) == Some(*x),
                    (Values::Numeric(values), None) => values.get(case).is_none(),
                    (Values::String(values), Some(Key::Text(x))) => {
                        values.get(case).trim_end() == x.trim_end()
                    }
                    _ => false,
                }
            })
        };
        let rows: Vec<usize> = (0..data.case_count()).filter(|&c| in_group(c)).collect();
        Ok(Materials {
            data,
            rows,
            rowtype,
            varname,
            all,
        })
    }

    /// The matrix file of one run: the records, with the split variables of the group in
    /// front of them.
    pub(crate) fn matrix_part(&self, who: &str, m: &MatrixRows) -> Result<Dataset, Error> {
        let n = m.rows.len();
        let mut part = Dataset::new(n);
        part.set_string_values("ROWTYPE_", 8, m.rows.iter().map(|r| r.0.clone()).collect())?;
        if let Some((name, values)) = &m.factor {
            part.set_numeric_values(name, values.clone())?;
        }
        let width = m.names.iter().map(String::len).max().unwrap_or(0).max(8);
        part.set_string_values(
            "VARNAME_",
            width,
            m.rows.iter().map(|r| r.1.clone()).collect(),
        )?;
        for (j, name) in m.names.iter().enumerate() {
            let values = m
                .rows
                .iter()
                .map(|r| Some(r.2[j]).filter(|v| v.is_finite()));
            part.set_numeric_values(name, values.collect())?;
            part.set_format(name, "F10.4")?;
            part.set_variable_label(name, m.labels[j].as_deref())?;
        }
        self.after_split_values(who, part)
    }

    /// Writes the matrices of MATRIX=OUT that the runs of the split groups left, one after
    /// the other, into the active dataset (`*`), a dataset or a file. A group with fewer
    /// variables has the others blank; strings take the widest.
    pub(crate) fn finish_matrix_out(&mut self, target: &Option<String>) -> Result<(), Error> {
        let (split, parts) = std::mem::take(&mut self.matrix_parts);
        let Some(widest) = parts.iter().max_by_key(|d| d.variable_count()) else {
            self.warn("MATRIX=OUT: the procedure left no matrix, so none was written".to_string());
            return Ok(());
        };
        let mut template = widest.slice(0, 0);
        // the variables of every group, in case groups have items of different names
        for part in &parts {
            for name in part.variable_names() {
                if template.variable_index(name).is_none() {
                    template.copy_variable(part, name, &[])?;
                }
            }
        }
        let strings: Vec<String> = template
            .variables()
            .filter(|v| v.is_string())
            .map(|v| v.name().to_string())
            .collect();
        for name in strings {
            let width = parts
                .iter()
                .filter_map(|p| p.variable(&name).ok()?.string_width())
                .max()
                .unwrap_or(1);
            template.set_string_width(&name, width)?;
        }
        let mut out = Dataset::from_parts(&template, &parts)?;
        match target {
            None => {
                // the matrix keeps the split, so MATRIX=IN(*) reads it group by group
                // the split the groups ran under, whatever TEMPORARY restored
                if split.is_some() {
                    out.set_split_file(split)?;
                }
                self.dataset = Some(out);
            }
            Some(file) => self.write_data_file(file, out)?,
        }
        Ok(())
    }

    /// `part` with the split variables of the running group (the values of its first case)
    /// in front.
    fn after_split_values(&self, who: &str, part: Dataset) -> Result<Dataset, Error> {
        let Some(split) = &self.matrix_parts.0 else {
            return Ok(part);
        };
        let group = self.active()?;
        if let Some(name) = split
            .variables
            .iter()
            .find(|name| part.variable_index(name).is_some())
        {
            return Err(Error(format!(
                "{who} /MATRIX=OUT: the split variable {name} has the name of a variable of the matrix file"
            )));
        }
        let n = part.case_count();
        let mut joined = Dataset::new(n);
        for name in &split.variables {
            joined.copy_variable(group, name, &vec![Some(0); n])?;
        }
        let cases: Vec<Option<usize>> = (0..n).map(Some).collect();
        for name in part.variable_names() {
            joined.copy_variable(&part, name, &cases)?;
        }
        Ok(joined)
    }
}
