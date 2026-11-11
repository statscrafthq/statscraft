//! MATRIX DATA: raw matrix materials read into a matrix data file.
//!
//! Rules, from the command's description:
//! - The new dataset holds the split variables (F4.0), ROWTYPE_ (A8), the factors (F4.0),
//!   VARNAME_ (A8) and the continuous variables (F10.4), in that order; split variables in the
//!   order of SPLIT, factors in the order of FACTORS.
//! - Values are separated by blanks and commas, and by + and - that do not follow D or E;
//!   ROWTYPE_ values may be quoted and abbreviated to three characters. A period is missing.
//! - LIST: each scalar, vector or matrix row starts on a new record and may go on over the
//!   records after it; values left on a record after its row (an upper triangle in lower
//!   format) are ignored with a warning. FREE: rows follow one another anywhere.
//! - With ROWTYPE_ in the data, each record gives its type; pooled records have missing
//!   factor values. Without it, CONTENTS gives the records in order (CORR alone when left
//!   out, with a warning); a group in parentheses holds within-cells records, read once for
//!   each of the CELLS cells, each record starting with the factor values.
//! - A matrix has one row per continuous variable: LOWER and UPPER give a triangle, FULL the
//!   square; NODIAGONAL leaves out the diagonal, which is 1 for CORR and missing otherwise.
//!   N_SCALAR is one value, copied to every variable; N_VECTOR, N_SCALAR and N_MATRIX records
//!   are written with the type N, SD as STDDEV.
//! - N=n writes one N record for each split. A split variable that is not on VARIABLES numbers
//!   the sets of materials 1, 2, ...
//!
//! Choices where the manual is silent:
//! - The rows of a triangle are mirrored, so every matrix in the file is square.
//! - The N record of N=n comes first in its split. Records keep the order of the data; a
//!   matrix stands where its first row was.
//! - VARNAME_ is wider than A8 when a continuous variable's name is longer, so the procedures
//!   can find every variable.
//! - The result is split by the split variables (SPLIT FILE LAYERED), so a procedure reads
//!   each set of materials on its own.
//! - Without SPLIT, data left after the CONTENTS records are an error; a matrix with too few
//!   rows, or a value that is not a number, is an error naming its line.

use crate::data_list::{Record, SHOWN_WARNINGS};
use crate::{Engine, Error};
use statscraft_data::{Dataset, SplitFile};
use statscraft_syntax::{MatrixData, MatrixShape, RowType};

/// One value of the data, with the record it is on.
struct Token {
    text: String,
    quoted: bool,
    record: usize,
}

/// The values of a record: blanks and commas separate them, and so do + and - unless they
/// follow D or E (an exponent); quotes hold a value with blanks.
fn split_record(text: &str, record: usize, out: &mut Vec<Token>) {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() || c == ',' {
            i += 1;
            continue;
        }
        if c == '\'' || c == '"' {
            let end = (i + 1..chars.len())
                .find(|&k| chars[k] == c)
                .unwrap_or(chars.len());
            out.push(Token {
                text: chars[i + 1..end].iter().collect(),
                quoted: true,
                record,
            });
            i = end + 1;
            continue;
        }
        let start = i;
        i += 1;
        while i < chars.len() {
            let c = chars[i];
            let exponent = matches!(chars[i - 1], 'D' | 'd' | 'E' | 'e');
            if c.is_whitespace() || c == ',' || c == '\'' || c == '"' {
                break;
            }
            if (c == '+' || c == '-') && !exponent {
                break;
            }
            i += 1;
        }
        out.push(Token {
            text: chars[start..i].iter().collect(),
            quoted: false,
            record,
        });
    }
}

struct Reader<'a> {
    tokens: Vec<Token>,
    records: &'a [Record],
    pos: usize,
    free: bool,
    warnings: Vec<String>,
}

impl Reader<'_> {
    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn place(&self) -> &str {
        let at = self.pos.min(self.tokens.len().saturating_sub(1));
        match self.tokens.get(at) {
            Some(t) => &self.records[t.record].place,
            None => "the data",
        }
    }

    /// With LIST, a row starts on a new record: values left on the last one are ignored.
    fn start_row(&mut self) {
        if self.free || self.pos == 0 {
            return;
        }
        let record = self.tokens[self.pos - 1].record;
        let skipped = self.tokens[self.pos..]
            .iter()
            .take_while(|t| t.record == record)
            .count();
        if skipped > 0 {
            self.warnings.push(format!(
                "{}: {skipped} values after the end of the row are ignored",
                self.records[record].place
            ));
            self.pos += skipped;
        }
    }

    /// The next value: its text, whether it was quoted, and where it is.
    fn next(&mut self, what: &str) -> Result<(String, bool, String), Error> {
        if self.at_end() {
            return Err(Error(format!(
                "MATRIX DATA: the data end where {what} should follow"
            )));
        }
        self.pos += 1;
        let token = &self.tokens[self.pos - 1];
        let place = self.records[token.record].place.clone();
        Ok((token.text.clone(), token.quoted, place))
    }

    /// A number; None for a period.
    fn number(&mut self, what: &str) -> Result<Option<f64>, Error> {
        let (text, quoted, place) = self.next(what)?;
        let text = text.trim();
        if text == "." {
            return Ok(None);
        }
        let plain = text.replace(['D', 'd'], "E");
        match plain.parse::<f64>() {
            Ok(n) if !quoted && n.is_finite() => Ok(Some(n)),
            _ => Err(Error(format!(
                "MATRIX DATA: {place}: {text} is not a number ({what})"
            ))),
        }
    }

    fn row_type(&mut self) -> Result<RowType, Error> {
        let (text, _, place) = self.next("a record type")?;
        RowType::from_name(&text).ok_or_else(|| {
            Error(format!(
                "MATRIX DATA: {place}: {} is no record type (CORR, COV, MAT, MSE, DFE, MEAN, \
                 STDDEV, N, N_SCALAR, N_MATRIX, COUNT or PROX)",
                text.trim()
            ))
        })
    }
}

/// One record of the matrix file.
struct Row {
    split: Vec<Option<f64>>,
    kind: &'static str,
    factors: Vec<Option<f64>>,
    varname: String,
    values: Vec<Option<f64>>,
}

/// A matrix being read: its rows are filled as their records come.
struct Matrix {
    kind: RowType,
    split: Vec<Option<f64>>,
    factors: Vec<Option<f64>>,
    cells: Vec<Vec<Option<Option<f64>>>>,
    rows_read: usize,
    place: String,
}

enum Item {
    Vector(Row),
    Matrix(usize),
}

/// The variable a matrix record stands for and the columns it gives values for.
fn row_cells(
    shape: MatrixShape,
    diagonal: bool,
    n: usize,
    k: usize,
) -> (usize, std::ops::Range<usize>) {
    match (shape, diagonal) {
        (MatrixShape::Lower, true) => (k, 0..k + 1),
        (MatrixShape::Lower, false) => (k + 1, 0..k + 1),
        (MatrixShape::Upper, true) => (k, k..n),
        (MatrixShape::Upper, false) => (k, k + 1..n),
        (MatrixShape::Full, _) => (k, 0..n),
    }
}

struct Build<'a> {
    md: &'a MatrixData,
    names: Vec<&'a str>,
    items: Vec<Item>,
    matrices: Vec<Matrix>,
    /// The matrix whose rows are being read.
    open: Option<usize>,
}

impl Build<'_> {
    fn rows_needed(&self) -> usize {
        let n = self.names.len();
        if self.md.diagonal { n } else { n - 1 }
    }

    /// Reads the values of one record of type `kind` after its leading fields.
    fn record(
        &mut self,
        reader: &mut Reader,
        kind: RowType,
        split: &[Option<f64>],
        factors: &[Option<f64>],
    ) -> Result<(), Error> {
        let n = self.names.len();
        if kind.is_matrix() {
            let needed = self.rows_needed();
            let goes_on = self.open.is_some_and(|i| {
                let m = &self.matrices[i];
                m.kind == kind && m.split == split && m.factors == factors && m.rows_read < needed
            });
            if !goes_on {
                self.matrices.push(Matrix {
                    kind,
                    split: split.to_vec(),
                    factors: factors.to_vec(),
                    cells: vec![vec![None; n]; n],
                    rows_read: 0,
                    place: reader.place().to_string(),
                });
                self.open = Some(self.matrices.len() - 1);
                self.items.push(Item::Matrix(self.matrices.len() - 1));
            }
            let at = self.open.expect("just set");
            let k = self.matrices[at].rows_read;
            let (i, columns) = row_cells(self.md.shape, self.md.diagonal, n, k);
            for c in columns {
                let what = format!(
                    "{} of {} with {}",
                    kind.value(),
                    self.names[i],
                    self.names[c]
                );
                let value = reader.number(&what)?;
                self.matrices[at].cells[i][c] = Some(value);
            }
            self.matrices[at].rows_read += 1;
            return Ok(());
        }
        self.open = None;
        let values = if kind == RowType::NScalar {
            vec![reader.number("the N_SCALAR value")?; n]
        } else {
            let mut values = Vec::with_capacity(n);
            for name in &self.names {
                values.push(reader.number(&format!("{} of {name}", kind.value()))?);
            }
            values
        };
        self.items.push(Item::Vector(Row {
            split: split.to_vec(),
            kind: kind.value(),
            factors: factors.to_vec(),
            varname: String::new(),
            values,
        }));
        Ok(())
    }

    fn n_record(&mut self, split: &[Option<f64>]) {
        if let Some(n) = self.md.n {
            self.items.push(Item::Vector(Row {
                split: split.to_vec(),
                kind: "N",
                factors: vec![None; self.md.factors.len()],
                varname: String::new(),
                values: vec![Some(f64::from(n)); self.names.len()],
            }));
        }
    }

    /// The records of the file, every matrix made square.
    fn rows(self) -> Result<Vec<Row>, Error> {
        let needed = self.rows_needed();
        let mut matrices: Vec<Option<Matrix>> = self.matrices.into_iter().map(Some).collect();
        let mut out = Vec::new();
        for item in self.items {
            let at = match item {
                Item::Vector(row) => {
                    out.push(row);
                    continue;
                }
                Item::Matrix(at) => at,
            };
            let mut m = matrices[at].take().expect("each matrix once");
            if m.rows_read < needed {
                return Err(Error(format!(
                    "MATRIX DATA: {}: the {} matrix has {} of its {needed} records",
                    m.place,
                    m.kind.value(),
                    m.rows_read
                )));
            }
            let n = self.names.len();
            for i in 0..n {
                for j in 0..n {
                    if m.cells[i][j].is_none() {
                        m.cells[i][j] = m.cells[j][i];
                    }
                }
                if m.cells[i][i].is_none() {
                    let one = (m.kind == RowType::Corr).then_some(1.0);
                    m.cells[i][i] = Some(one);
                }
            }
            for (i, name) in self.names.iter().enumerate() {
                out.push(Row {
                    split: m.split.clone(),
                    kind: m.kind.value(),
                    factors: m.factors.clone(),
                    varname: name.to_string(),
                    values: m.cells[i].iter().map(|v| v.flatten()).collect(),
                });
            }
        }
        Ok(out)
    }
}

/// The index of `name` in `list`, ignoring case.
fn index(list: &[String], name: &str) -> Option<usize> {
    list.iter().position(|v| v.eq_ignore_ascii_case(name))
}

impl Engine {
    pub(crate) fn read_matrix(
        &mut self,
        md: &MatrixData,
        records: Vec<Record>,
    ) -> Result<(), Error> {
        let mut tokens = Vec::new();
        for (i, record) in records.iter().enumerate() {
            split_record(&record.text, i, &mut tokens);
        }
        let mut reader = Reader {
            tokens,
            records: &records,
            pos: 0,
            free: md.free,
            warnings: Vec::new(),
        };
        let names = md.continuous();
        let leading: Vec<&String> = md
            .variables
            .iter()
            .take_while(|v| !names.contains(&v.as_str()))
            .collect();
        let in_data = |name: &String| index(&md.variables, name).is_some();
        let generated = md.split.len() == 1 && !in_data(&md.split[0]);
        let mut build = Build {
            md,
            names,
            items: Vec::new(),
            matrices: Vec::new(),
            open: None,
        };
        let explicit = md.explicit();
        // the leading fields of a record: (split values, factor values, record type)
        let lead = |reader: &mut Reader,
                    within: bool,
                    split: &mut Vec<Option<f64>>,
                    factors: &mut Vec<Option<f64>>|
         -> Result<Option<RowType>, Error> {
            let mut kind = None;
            for name in &leading {
                if name.eq_ignore_ascii_case("ROWTYPE_") {
                    kind = Some(reader.row_type()?);
                } else if let Some(at) = index(&md.split, name) {
                    split[at] = reader.number(&format!("the value of {name}"))?;
                } else if let Some(at) = index(&md.factors, name)
                    && within
                {
                    factors[at] = reader.number(&format!("the value of {name}"))?;
                }
            }
            Ok(kind)
        };
        if explicit {
            if md.cells.is_some() {
                self.warn("CELLS is ignored: the record types are in the data (ROWTYPE_)");
            }
            let listed: Vec<RowType> = md.contents.iter().flat_map(|(_, k)| k.clone()).collect();
            let mut unlisted: Vec<&str> = Vec::new();
            let mut current: Option<Vec<Option<f64>>> = None;
            while !reader.at_end() {
                reader.start_row();
                if reader.at_end() {
                    break;
                }
                let mut split = vec![None; md.split.len()];
                let mut factors = vec![None; md.factors.len()];
                let kind = lead(&mut reader, true, &mut split, &mut factors)?.expect("ROWTYPE_");
                if !listed.is_empty()
                    && !listed.iter().any(|k| k.value() == kind.value())
                    && !unlisted.contains(&kind.value())
                {
                    unlisted.push(kind.value());
                }
                if current.as_ref() != Some(&split) {
                    build.n_record(&split);
                    current = Some(split.clone());
                }
                build.record(&mut reader, kind, &split, &factors)?;
            }
            for kind in unlisted {
                self.warn(format!(
                    "the data hold {kind} records, which CONTENTS does not list"
                ));
            }
        } else {
            let contents = if md.contents.is_empty() {
                self.warn(
                    "without ROWTYPE_ or CONTENTS every record is read as a row of a correlation \
                     matrix (CORR)",
                );
                vec![(false, vec![RowType::Corr])]
            } else {
                md.contents.clone()
            };
            let cells = md.cells.unwrap_or(1) as usize;
            let mut plan: Vec<(RowType, bool)> = Vec::new();
            for (within, kinds) in &contents {
                let times = if *within { cells } else { 1 };
                for _ in 0..times {
                    plan.extend(kinds.iter().map(|k| (*k, *within)));
                }
            }
            let mut group = 0;
            while !reader.at_end() {
                if group == 1 && md.split.is_empty() {
                    return Err(Error(format!(
                        "MATRIX DATA: {}: the data go on after the records CONTENTS describes; \
                         give SPLIT to read several sets of materials",
                        reader.place()
                    )));
                }
                group += 1;
                let mut split = vec![None; md.split.len()];
                if generated {
                    split[0] = Some(group as f64);
                }
                let mut first = true;
                for &(kind, within) in &plan {
                    let records = if kind.is_matrix() {
                        build.rows_needed()
                    } else {
                        1
                    };
                    for _ in 0..records {
                        reader.start_row();
                        let mut factors = vec![None; md.factors.len()];
                        lead(&mut reader, within, &mut split, &mut factors)?;
                        if std::mem::take(&mut first) {
                            build.n_record(&split);
                        }
                        build.record(&mut reader, kind, &split, &factors)?;
                    }
                    // a new spec starts a new matrix even of the same type
                    build.open = None;
                }
                reader.start_row();
            }
        }
        let rows = build.rows()?;
        let hidden = reader.warnings.len().saturating_sub(SHOWN_WARNINGS);
        reader.warnings.truncate(SHOWN_WARNINGS);
        for warning in std::mem::take(&mut reader.warnings) {
            self.warn(warning);
        }
        if hidden > 0 {
            self.warn(format!("{hidden} more records had values left over"));
        }
        let mut data = Dataset::new(rows.len());
        for (s, name) in md.split.iter().enumerate() {
            data.set_numeric_values(name, rows.iter().map(|r| r.split[s]).collect())?;
            data.set_format(name, "F4.0")?;
        }
        let kinds = rows.iter().map(|r| r.kind.to_string()).collect();
        data.set_string_values("ROWTYPE_", 8, kinds)?;
        for (f, name) in md.factors.iter().enumerate() {
            data.set_numeric_values(name, rows.iter().map(|r| r.factors[f]).collect())?;
            data.set_format(name, "F4.0")?;
        }
        let names = md.continuous();
        let width = names.iter().map(|n| n.len()).max().unwrap_or(0).max(8);
        let varnames = rows.iter().map(|r| r.varname.clone()).collect();
        data.set_string_values("VARNAME_", width, varnames)?;
        for (i, name) in names.iter().enumerate() {
            data.set_numeric_values(name, rows.iter().map(|r| r.values[i]).collect())?;
            data.set_format(name, "F10.4")?;
        }
        if !md.split.is_empty() {
            data.set_split_file(Some(SplitFile {
                variables: md.split.clone(),
                layered: true,
            }))?;
        }
        self.dataset = Some(data);
        self.default_measures();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(line: &str) -> Vec<String> {
        let mut out = Vec::new();
        split_record(line, 0, &mut out);
        out.into_iter().map(|t| t.text).collect()
    }

    #[test]
    fn values_split_at_blanks_commas_and_signs() {
        assert_eq!(texts("CORR 1,.5  -.25"), ["CORR", "1", ".5", "-.25"]);
        assert_eq!(texts("1.0-0.5+0.25"), ["1.0", "-0.5", "+0.25"]);
        assert_eq!(texts("1E-5 2D+3 -1e+2"), ["1E-5", "2D+3", "-1e+2"]);
        assert_eq!(texts("'MEAN    ' 5"), ["MEAN    ", "5"]);
        assert_eq!(texts(". 1"), [".", "1"]);
    }

    #[test]
    fn matrix_rows_by_shape() {
        use MatrixShape::*;
        assert_eq!(row_cells(Lower, true, 3, 2), (2, 0..3));
        assert_eq!(row_cells(Lower, false, 3, 0), (1, 0..1));
        assert_eq!(row_cells(Upper, true, 3, 1), (1, 1..3));
        assert_eq!(row_cells(Upper, false, 3, 1), (1, 2..3));
        assert_eq!(row_cells(Full, false, 3, 1), (1, 0..3));
    }
}
