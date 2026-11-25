//! MCONVERT: covariance matrix materials to correlation materials plus standard deviations, and
//! back.
//!
//! Rules noted from the Command Syntax Reference:
//! - The file has the layout of MATRIX DATA: the split variables, ROWTYPE_, the grouping
//!   variables, VARNAME_, then the variables of the matrix.
//! - COV records become CORR records and a STDDEV record; CORR records become COV records
//!   and need a STDDEV record. Every record of another type is copied.
//! - Each run of records with the same split and grouping values is one set, converted alone.
//! - REPLACE (the default) writes the converted matrix where the original was; APPEND writes the
//!   original set and then its converted matrix. The source is not changed unless it is also the
//!   output.
//!
//! Choices where the manual is silent: a STDDEV record the set has already is kept and none is
//! added; the generated STDDEV record goes before the converted matrix (the order the files
//! other commands write have); it has a blank VARNAME_; a set with both CORR and COV records, a
//! matrix with other than one record per variable, or CORR records without STDDEV are errors; a
//! variance of 0 or below gives missing correlations.

use crate::cluster::value_text;
use crate::{Engine, Error, Table};
use statscraft_data::{Dataset, Values};
use statscraft_syntax::Mconvert;

/// One record of the result: the case of the input whose split and grouping values it takes, its
/// type and VARNAME_, and its values (None: those of that case).
struct Record {
    key: usize,
    rowtype: String,
    varname: String,
    values: Option<Vec<Option<f64>>>,
}

impl Engine {
    pub(crate) fn mconvert(&mut self, command: &Mconvert) -> Result<Vec<Table>, Error> {
        let data = match &command.matrix.input {
            Some(Some(file)) => self.data_file(file)?,
            _ => self.active()?.clone(),
        };
        let out = convert(&data, command.append)?;
        match &command.matrix.output {
            Some(Some(file)) => self.write_data_file(file, out)?,
            _ => self.dataset = Some(out),
        }
        Ok(Vec::new())
    }
}

fn convert(data: &Dataset, append: bool) -> Result<Dataset, Error> {
    let names: Vec<String> = data.variable_names().map(str::to_string).collect();
    let find = |wanted: &str| {
        names
            .iter()
            .position(|n| n.eq_ignore_ascii_case(wanted))
            .filter(|&i| data.variable(&names[i]).is_ok_and(|v| v.is_string()))
            .ok_or_else(|| {
                Error(format!(
                    "MCONVERT needs the string variable {wanted} in the matrix data"
                ))
            })
    };
    let (rowtype, varname) = (find("ROWTYPE_")?, find("VARNAME_")?);
    if varname < rowtype {
        return Err(Error(
            "VARNAME_ must come after ROWTYPE_ in the matrix data".into(),
        ));
    }
    let matrix: Vec<usize> = (varname + 1..names.len()).collect();
    if matrix.is_empty()
        || matrix
            .iter()
            .any(|&i| data.variable(&names[i]).is_ok_and(|v| v.is_string()))
    {
        return Err(Error(
            "the variables after VARNAME_ must be numeric, and there must be at least one".into(),
        ));
    }
    let k = matrix.len();
    let keys: Vec<usize> = (0..varname).filter(|&i| i != rowtype).collect();
    let text = |col: usize, row: usize| {
        let variable = data.variable(&names[col]).expect("a column");
        value_text(&variable, row)
    };
    let number = |col: usize, row: usize| match data.variable(&names[col]).map(|v| v.values()) {
        Ok(Values::Numeric(v)) => v.get(row),
        _ => None,
    };
    let kind = |row: usize| text(rowtype, row).trim().to_uppercase();
    let n = data.case_count();
    let same_set = |a: usize, b: usize| keys.iter().all(|&c| text(c, a) == text(c, b));

    let mut records: Vec<Record> = Vec::new();
    let mut start = 0;
    while start < n {
        let mut end = start + 1;
        while end < n && same_set(start, end) {
            end += 1;
        }
        let rows = start..end;
        let of =
            |wanted: &str| -> Vec<usize> { rows.clone().filter(|&r| kind(r) == wanted).collect() };
        let (corr, cov, stddev) = (of("CORR"), of("COV"), of("STDDEV"));
        let original = |r: usize| Record {
            key: r,
            rowtype: kind(r),
            varname: text(varname, r).trim_end().to_string(),
            values: None,
        };
        let keep_all = |records: &mut Vec<Record>| {
            for r in rows.clone() {
                records.push(Record {
                    rowtype: text(rowtype, r).trim_end().to_string(),
                    ..original(r)
                });
            }
        };
        if corr.is_empty() && cov.is_empty() {
            keep_all(&mut records);
            start = end;
            continue;
        }
        if !corr.is_empty() && !cov.is_empty() {
            return Err(Error(format!(
                "records {} to {} have both CORR and COV records: MCONVERT converts one of them",
                start + 1,
                end
            )));
        }
        let (from, to) = if corr.is_empty() {
            (&cov, "CORR")
        } else {
            (&corr, "COV")
        };
        if from.len() != k {
            return Err(Error(format!(
                "the {} matrix at records {} to {} has {} records for {k} variables: it needs one per variable",
                if corr.is_empty() { "COV" } else { "CORR" },
                start + 1,
                end,
                from.len()
            )));
        }
        let m: Vec<Vec<f64>> = from
            .iter()
            .map(|&r| {
                matrix
                    .iter()
                    .map(|&c| number(c, r).unwrap_or(f64::NAN))
                    .collect()
            })
            .collect();
        let (converted, made_stddev): (Vec<Vec<f64>>, Option<Vec<f64>>) = if to == "CORR" {
            let s: Vec<f64> = (0..k)
                .map(|i| {
                    if m[i][i] > 0.0 {
                        m[i][i].sqrt()
                    } else {
                        f64::NAN
                    }
                })
                .collect();
            let corr: Vec<Vec<f64>> = (0..k)
                .map(|i| {
                    (0..k)
                        .map(|j| {
                            if i == j && s[i].is_finite() {
                                1.0
                            } else {
                                m[i][j] / (s[i] * s[j])
                            }
                        })
                        .collect()
                })
                .collect();
            (corr, stddev.is_empty().then_some(s))
        } else {
            let Some(&row) = stddev.first() else {
                return Err(Error(format!(
                    "the CORR matrix at records {} to {} needs a STDDEV record to become a COV matrix",
                    start + 1,
                    end
                )));
            };
            let s: Vec<f64> = matrix
                .iter()
                .map(|&c| number(c, row).unwrap_or(f64::NAN))
                .collect();
            let cov = (0..k)
                .map(|i| (0..k).map(|j| m[i][j] * s[i] * s[j]).collect())
                .collect();
            (cov, None)
        };
        let finite = |v: &[f64]| -> Option<Vec<Option<f64>>> {
            Some(
                v.iter()
                    .map(|x| Some(*x).filter(|x| x.is_finite()))
                    .collect(),
            )
        };
        let converted_rows = |records: &mut Vec<Record>| {
            if let Some(s) = &made_stddev {
                records.push(Record {
                    key: start,
                    rowtype: "STDDEV".into(),
                    varname: String::new(),
                    values: finite(s),
                });
            }
            for (i, row) in converted.iter().enumerate() {
                records.push(Record {
                    key: from[i],
                    rowtype: to.into(),
                    varname: text(varname, from[i]).trim_end().to_string(),
                    values: finite(row),
                });
            }
        };
        if append {
            keep_all(&mut records);
            converted_rows(&mut records);
        } else {
            let first = from[0];
            for r in rows.clone() {
                if r == first {
                    converted_rows(&mut records);
                }
                if !from.contains(&r) {
                    records.push(Record {
                        rowtype: text(rowtype, r).trim_end().to_string(),
                        ..original(r)
                    });
                }
            }
        }
        start = end;
    }

    // the result: the columns in the order of the input, the records in the order built
    let cases: Vec<Option<usize>> = records.iter().map(|r| Some(r.key)).collect();
    let mut out = Dataset::new(records.len());
    let width = |col: usize| {
        data.variable(&names[col])
            .ok()
            .and_then(|v| v.string_width())
            .unwrap_or(8)
    };
    for (col, name) in names.iter().enumerate() {
        if col == rowtype {
            let w = records
                .iter()
                .map(|r| r.rowtype.len())
                .max()
                .unwrap_or(0)
                .max(width(col));
            out.set_string_values(name, w, records.iter().map(|r| r.rowtype.clone()).collect())?;
        } else if col == varname {
            let w = records
                .iter()
                .map(|r| r.varname.len())
                .max()
                .unwrap_or(0)
                .max(width(col));
            out.set_string_values(name, w, records.iter().map(|r| r.varname.clone()).collect())?;
        } else {
            out.copy_variable(data, name, &cases)?;
            if let Some(j) = matrix.iter().position(|&c| c == col) {
                let values: Vec<Option<f64>> = records
                    .iter()
                    .map(|r| match &r.values {
                        Some(v) => v[j],
                        None => number(col, r.key),
                    })
                    .collect();
                out.set_numeric_values(name, values)?;
            }
        }
    }
    if let Some(split) = data.split_file() {
        out.set_split_file(Some(split.clone()))?;
    }
    Ok(out)
}
