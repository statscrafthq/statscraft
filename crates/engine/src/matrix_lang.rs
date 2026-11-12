//! MATRIX ... END MATRIX: runs a program of the matrix language.
//!
//! Rules, from the description of the matrix language:
//! - A matrix variable holds a matrix of any size; COMPUTE, READ, GET, MGET, CALL and the LOOP
//!   index create them. Names are compared without case. The variables are gone after END
//!   MATRIX. A string element holds up to 8 bytes.
//! - `+`, `-`, the elementwise operators and the relational and logical ones need matrices of
//!   the same size, or a scalar, which goes with every element. `*` is the matrix product (or
//!   a scalar times a matrix), `/` divides element by element, `**` raises a square matrix to a
//!   whole power (a negative one inverts it). Positive values are true; comparisons give 1 or 0.
//! - Ranges `a:b:c` take whole numbers; fractions are cut off. Matrices pasted in braces must
//!   fit: the parts of a row have the same rows, the rows the same columns.
//! - `M(i, j)` takes the rows i and columns j, each a vector of positions or all with `:`; a
//!   vector takes one index. COMPUTE on `M(i, j)` changes that part of an existing M, with a
//!   matrix of the same size or a scalar.
//! - DO IF and the IF of LOOP and END LOOP need a scalar, true when positive. A LOOP index
//!   runs from first to last by step (1); the three are truncated to whole numbers. Every
//!   loop, with an index or not, stops after SET MXLOOPS iterations (default 40).
//! - READ reads text files, each row from a new record and on over the next records as needed,
//!   from the columns of FIELD: values separated by blanks or commas, or in fields of a fixed
//!   width (BY, the width of FORMAT, or the columns divided by its repetition factor). MODE
//!   SYMMETRIC reads the lower triangle and mirrors it. REREAD starts at the last record read,
//!   unless EOF was called for the file since. EOF(file) is 1 when the last READ of the file
//!   took its last record.
//! - WRITE writes each row from a new record, in fields of a fixed width or separated by a
//!   blank (each number as precise as needed), going on over the next records when the field
//!   is full. TRIANGULAR writes the lower triangle. HOLD keeps the last line open for the next
//!   WRITE to the file.
//! - GET reads the cases of the active dataset or a data file as rows; string variables are
//!   skipped with a warning; weights are ignored. A missing value needs MISSING: ACCEPT takes
//!   user-missing values as they are (system-missing ones then need SYSMIS), OMIT leaves the
//!   case out, a value replaces them. NAMES gets the names of the columns.
//! - SAVE writes the rows as cases. The first SAVE to a file sets its variables: VARIABLES,
//!   else the strings of NAMES (VARIABLES wins, with a warning), else COL1, COL2, ...; STRINGS
//!   names the string variables. Later SAVEs to the file add cases and need as many columns.
//! - MGET reads a matrix data file into variables named by the record type (CV, CR, MN, SD,
//!   NC, CN), then F and the cell number when there are factors, then S and the split number;
//!   it reports what it creates and does not replace a variable that exists.
//! - MSAVE writes one matrix data file: the split variables, ROWTYPE_, the factors, VARNAME_
//!   and the variables, one record per row of each saved matrix.
//!
//! Choices where the manual is silent:
//! - PRINT gives a table: row labels (row1, row2, ... by default) and column labels (col1,
//!   col2, ...), as the manual describes for tables. Without FORMAT, whole numbers below 10^9
//!   show without decimals, others with 10 significant digits in a common format; values of
//!   10^9 or more, or below 10^-3 throughout, are divided by a power of 10 that the caption
//!   names. Infinities and undefined values show as +Inf, -Inf and NaN. SPACE has no effect on
//!   a table.
//! - An error stops the program at the statement that caused it; nothing is saved or written.
//! - The index of a loop keeps the value of its last iteration.
//! - A loop stopped by MXLOOPS gives a warning.
//! - SAVE, MSAVE and WRITE write their files at END MATRIX; a GET of such a file during the
//!   program reads it as it was. GET of the active dataset reads all its cases, also those
//!   that FILTER leaves out.
//! - Division by zero gives an infinity or NaN, as in floating point; LN, SQRT and the like
//!   check their arguments.
//! - Blank fixed fields that READ reads are system-missing (NaN); running out of records fills
//!   the rest with NaN and warns.

use crate::matrix_functions as functions;
use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, Values};
use statscraft_syntax::{
    CallStatement, DataFormat, ExprKind, Index, Labels, MExpr, MatrixGet, MatrixMget, MatrixMsave,
    MatrixOp, MatrixPrint, MatrixProgram, MatrixRead, MatrixSave, MatrixStatement, MatrixWrite,
    MissingAction, Span, StatementKind, Target,
};
use std::collections::BTreeMap;

/// A matrix, its elements by rows.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Mat {
    pub rows: usize,
    pub cols: usize,
    pub data: Vec<f64>,
}

impl Mat {
    pub fn new(rows: usize, cols: usize, data: Vec<f64>) -> Self {
        debug_assert_eq!(rows * cols, data.len());
        Mat { rows, cols, data }
    }

    pub fn filled(rows: usize, cols: usize, value: f64) -> Self {
        Mat::new(rows, cols, vec![value; rows * cols])
    }

    pub fn scalar(value: f64) -> Self {
        Mat::new(1, 1, vec![value])
    }

    pub fn identity(n: usize) -> Self {
        let mut m = Mat::filled(n, n, 0.0);
        for i in 0..n {
            m.set(i, i, 1.0);
        }
        m
    }

    pub fn at(&self, i: usize, j: usize) -> f64 {
        self.data[i * self.cols + j]
    }

    pub fn set(&mut self, i: usize, j: usize, value: f64) {
        self.data[i * self.cols + j] = value;
    }

    pub fn is_scalar(&self) -> bool {
        self.rows == 1 && self.cols == 1
    }

    pub fn is_vector(&self) -> bool {
        self.rows == 1 || self.cols == 1
    }

    pub fn dims(&self) -> String {
        format!("{}x{}", self.rows, self.cols)
    }

    pub fn map(&self, f: impl Fn(f64) -> f64) -> Mat {
        Mat::new(
            self.rows,
            self.cols,
            self.data.iter().map(|v| f(*v)).collect(),
        )
    }

    pub fn transpose(&self) -> Mat {
        let mut out = Mat::filled(self.cols, self.rows, 0.0);
        for i in 0..self.rows {
            for j in 0..self.cols {
                out.set(j, i, self.at(i, j));
            }
        }
        out
    }

    pub fn product(&self, other: &Mat) -> Mat {
        let mut out = Mat::filled(self.rows, other.cols, 0.0);
        for i in 0..self.rows {
            for k in 0..self.cols {
                let a = self.at(i, k);
                if a == 0.0 {
                    continue;
                }
                for j in 0..other.cols {
                    out.data[i * other.cols + j] += a * other.at(k, j);
                }
            }
        }
        out
    }

    pub fn to_rows(&self) -> Vec<Vec<f64>> {
        self.data
            .chunks(self.cols.max(1))
            .take(self.rows)
            .map(<[f64]>::to_vec)
            .collect()
    }

    pub fn from_rows(rows: &[Vec<f64>], cols: usize) -> Mat {
        Mat::new(rows.len(), cols, rows.concat())
    }
}

/// A string of up to 8 bytes as the number that holds it.
pub(crate) fn pack(text: &str) -> f64 {
    let mut bytes = [b' '; 8];
    for (b, t) in bytes.iter_mut().zip(text.bytes()) {
        *b = t;
    }
    f64::from_bits(u64::from_le_bytes(bytes))
}

/// The string a number holds, without trailing blanks.
pub(crate) fn unpack(value: f64) -> String {
    let bytes = value.to_bits().to_le_bytes();
    String::from_utf8_lossy(&bytes).trim_end().to_string()
}

/// An error with the place it comes from.
struct Fault {
    message: String,
    span: Span,
}

type Run<T> = Result<T, Fault>;

fn fault(span: Span, message: impl Into<String>) -> Fault {
    Fault {
        message: message.into(),
        span,
    }
}

enum Flow {
    Next,
    Break,
}

/// A text file that READ reads.
struct ReadFile {
    records: Vec<String>,
    /// The record the next READ starts with.
    next: usize,
    /// The last record read.
    last: Option<usize>,
    /// EOF was called since the last READ: REREAD is ignored once.
    eof_called: bool,
}

/// A text file that WRITE writes.
#[derive(Default)]
struct WriteFile {
    lines: Vec<String>,
    /// HOLD: the last line, still open.
    held: Option<String>,
}

/// The cases of SAVE for one file.
struct SaveFile {
    names: Vec<String>,
    strings: Vec<bool>,
    rows: Vec<Vec<f64>>,
}

/// The records of MSAVE.
struct MatrixFile {
    outfile: String,
    snames: Vec<String>,
    fnames: Vec<String>,
    variables: Vec<String>,
    records: Vec<MatrixRecord>,
}

/// One record of MSAVE: split values, row type, factor values, row name, values.
type MatrixRecord = (Vec<f64>, String, Vec<f64>, String, Vec<f64>);

struct Machine<'a> {
    engine: &'a mut Engine,
    /// By upper-case name: the name as first written and the matrix.
    vars: BTreeMap<String, (String, Mat)>,
    tables: Vec<Table>,
    reads: BTreeMap<String, ReadFile>,
    writes: Vec<(String, WriteFile)>,
    saves: Vec<(Option<String>, SaveFile)>,
    msave: Option<MatrixFile>,
    mxloops: usize,
}

impl Engine {
    /// Runs a matrix program. An error stops it and points at its statement.
    pub(crate) fn matrix(&mut self, program: &MatrixProgram) -> Result<Vec<Table>, Error> {
        if self.input.is_some() {
            return Err(Error("MATRIX cannot stand inside an input program".into()));
        }
        self.begin_pass()?;
        let before = self.dataset.clone();
        let mxloops = self.mxloops();
        let mut machine = Machine {
            engine: self,
            vars: BTreeMap::new(),
            tables: Vec::new(),
            reads: BTreeMap::new(),
            writes: Vec::new(),
            saves: Vec::new(),
            msave: None,
            mxloops,
        };
        let result = machine.run(&program.statements).map(|_| ());
        let Machine {
            tables,
            writes,
            saves,
            msave,
            ..
        } = machine;
        if let Err(fault) = result {
            self.error_at = Some((fault.span.line, fault.span.column));
            self.end_pass(before.as_ref());
            return Err(Error(fault.message));
        }
        self.finish_matrix(writes, saves, msave)?;
        self.end_pass(before.as_ref());
        Ok(tables)
    }

    fn finish_matrix(
        &mut self,
        writes: Vec<(String, WriteFile)>,
        saves: Vec<(Option<String>, SaveFile)>,
        msave: Option<MatrixFile>,
    ) -> Result<(), Error> {
        for (file, mut written) in writes {
            written.lines.extend(written.held.take());
            if !self.dry_run {
                let mut text = written.lines.join("\n");
                text.push('\n');
                std::fs::write(self.path(&file), text)
                    .map_err(|e| Error(format!("cannot write {file}: {e}")))?;
            }
        }
        for (target, save) in saves {
            let data = save_dataset(&save)?;
            match target {
                None => {
                    self.park_active();
                    self.dataset = Some(data);
                    self.scratch_assigned.clear();
                    self.temporary = None;
                    self.case_limit = None;
                }
                Some(file) => self.write_data_file(&file, data)?,
            }
        }
        if let Some(file) = msave {
            let data = matrix_dataset(&file)?;
            self.write_data_file(&file.outfile, data)?;
        }
        Ok(())
    }
}

fn save_dataset(save: &SaveFile) -> Result<Dataset, Error> {
    let mut data = Dataset::new(save.rows.len());
    for (j, name) in save.names.iter().enumerate() {
        let column = save.rows.iter().map(|row| row[j]);
        if save.strings[j] {
            data.set_string_values(name, 8, column.map(unpack).collect())?;
        } else {
            let values = column.map(|v| v.is_finite().then_some(v)).collect();
            data.set_numeric_values(name, values)?;
        }
    }
    Ok(data)
}

fn matrix_dataset(file: &MatrixFile) -> Result<Dataset, Error> {
    let n = file.records.len();
    let mut data = Dataset::new(n);
    let number = |v: f64| v.is_finite().then_some(v);
    for (s, name) in file.snames.iter().enumerate() {
        data.set_numeric_values(
            name,
            file.records
                .iter()
                .map(|r| r.0.get(s).copied().and_then(number))
                .collect(),
        )?;
        data.set_format(name, "F4.0")?;
    }
    data.set_string_values(
        "ROWTYPE_",
        8,
        file.records.iter().map(|r| r.1.clone()).collect(),
    )?;
    for (f, name) in file.fnames.iter().enumerate() {
        data.set_numeric_values(
            name,
            file.records
                .iter()
                .map(|r| r.2.get(f).copied().and_then(number))
                .collect(),
        )?;
        data.set_format(name, "F4.0")?;
    }
    let width = file
        .variables
        .iter()
        .map(String::len)
        .max()
        .unwrap_or(0)
        .max(8);
    data.set_string_values(
        "VARNAME_",
        width,
        file.records.iter().map(|r| r.3.clone()).collect(),
    )?;
    for (j, name) in file.variables.iter().enumerate() {
        data.set_numeric_values(name, file.records.iter().map(|r| number(r.4[j])).collect())?;
        data.set_format(name, "F10.4")?;
    }
    Ok(data)
}

/// Whole numbers from a scalar or vector, as positions or a size.
fn whole(value: f64) -> i64 {
    value.trunc() as i64
}

fn truth(m: &Mat, what: &str, span: Span) -> Run<bool> {
    if !m.is_scalar() {
        return Err(fault(
            span,
            format!("{what} must give a scalar, but it gives {}", m.dims()),
        ));
    }
    Ok(m.data[0] > 0.0)
}

impl Machine<'_> {
    fn get(&self, name: &str, span: Span) -> Run<&Mat> {
        self.vars
            .get(&name.to_uppercase())
            .map(|(_, m)| m)
            .ok_or_else(|| fault(span, format!("the matrix variable {name} does not exist")))
    }

    fn set(&mut self, name: &str, value: Mat) {
        let key = name.to_uppercase();
        match self.vars.get_mut(&key) {
            Some(slot) => slot.1 = value,
            None => {
                self.vars.insert(key, (name.to_string(), value));
            }
        }
    }

    fn run(&mut self, statements: &[MatrixStatement]) -> Run<Flow> {
        for statement in statements {
            if let Flow::Break = self.statement(statement)? {
                return Ok(Flow::Break);
            }
        }
        Ok(Flow::Next)
    }

    fn statement(&mut self, statement: &MatrixStatement) -> Run<Flow> {
        let span = statement.span;
        match &statement.kind {
            StatementKind::Compute { target, expr } => {
                let value = self.eval(expr)?;
                self.assign(target, value)?;
            }
            StatementKind::Call(call) => self.call(call, span)?,
            StatementKind::Print(print) => self.print(print)?,
            StatementKind::DoIf {
                branches,
                otherwise,
            } => {
                for (condition, body) in branches {
                    let value = self.eval(condition)?;
                    if truth(&value, "the condition of DO IF", condition.span)? {
                        return self.run(body);
                    }
                }
                if let Some(body) = otherwise {
                    return self.run(body);
                }
            }
            StatementKind::Loop(spec) => {
                let index = match &spec.index {
                    Some(index) => {
                        let scalar = |m: &mut Self, e: &MExpr, what: &str| -> Run<i64> {
                            let v = m.eval(e)?;
                            if !v.is_scalar() {
                                return Err(fault(
                                    e.span,
                                    format!(
                                        "the {what} of LOOP must be a scalar, but it is {}",
                                        v.dims()
                                    ),
                                ));
                            }
                            Ok(whole(v.data[0]))
                        };
                        let first = scalar(self, &index.first, "first value")?;
                        let last = scalar(self, &index.last, "last value")?;
                        let step = match &index.step {
                            Some(step) => scalar(self, step, "increment")?,
                            None => 1,
                        };
                        if step == 0 {
                            let at = index.step.as_ref().map_or(span, |s| s.span);
                            return Err(fault(at, "the increment of LOOP must not be 0"));
                        }
                        Some((index.name.as_str(), first, last, step))
                    }
                    None => None,
                };
                let mut value = index.map_or(0, |(_, first, _, _)| first);
                let mut iterations = 0;
                loop {
                    if let Some((_, _, last, step)) = index
                        && ((step > 0 && value > last) || (step < 0 && value < last))
                    {
                        break;
                    }
                    if iterations == self.mxloops {
                        self.engine.warn(format!(
                            "the LOOP on line {} stopped after {} iterations, the limit SET MXLOOPS gives",
                            span.line, self.mxloops
                        ));
                        break;
                    }
                    if let Some((name, ..)) = index {
                        self.set(name, Mat::scalar(value as f64));
                    }
                    if let Some(condition) = &spec.condition {
                        let c = self.eval(condition)?;
                        if !truth(&c, "the condition of LOOP", condition.span)? {
                            break;
                        }
                    }
                    iterations += 1;
                    if let Flow::Break = self.run(&spec.body)? {
                        break;
                    }
                    if let Some(until) = &spec.until {
                        let c = self.eval(until)?;
                        if truth(&c, "the condition of END LOOP", until.span)? {
                            break;
                        }
                    }
                    if let Some((_, _, _, step)) = index {
                        value += step;
                    }
                }
            }
            StatementKind::Break => return Ok(Flow::Break),
            StatementKind::Read(read) => self.read(read, span)?,
            StatementKind::Write(write) => self.write(write, span)?,
            StatementKind::Get(get) => self.get_data(get, span)?,
            StatementKind::Save(save) => self.save(save, span)?,
            StatementKind::Mget(mget) => self.mget(mget, span)?,
            StatementKind::Msave(msave) => self.msave(msave, span)?,
            StatementKind::Display { dictionary, status } => self.display(*dictionary, *status),
            StatementKind::Release(names) => {
                for name in names {
                    if self.vars.remove(&name.to_uppercase()).is_none() {
                        self.engine.warn(format!(
                            "RELEASE: the matrix variable {name} does not exist"
                        ));
                    }
                }
            }
        }
        Ok(Flow::Next)
    }

    // ---- expressions ----

    fn eval(&mut self, expr: &MExpr) -> Run<Mat> {
        let span = expr.span;
        Ok(match &expr.kind {
            ExprKind::Number(n) => Mat::scalar(*n),
            ExprKind::String(s) => Mat::scalar(pack(s)),
            ExprKind::Variable(name) => self.get(name, span)?.clone(),
            ExprKind::Matrix(rows) => {
                let mut pieces = Vec::new();
                for row in rows {
                    let mut parts = Vec::new();
                    for element in row {
                        parts.push((self.eval(element)?, element.span));
                    }
                    pieces.push(hconcat(parts)?);
                }
                vconcat(pieces, span)?
            }
            ExprKind::Index { name, indexes } => {
                let m = self.get(name, span)?.clone();
                let (rows, cols) = self.positions(&m, name, indexes, span)?;
                let mut data = Vec::with_capacity(rows.len() * cols.len());
                for &i in &rows {
                    for &j in &cols {
                        data.push(m.at(i, j));
                    }
                }
                // one index into a column vector keeps it a column
                if indexes.len() == 1 && m.cols == 1 && m.rows != 1 {
                    Mat::new(rows.len(), 1, data)
                } else {
                    Mat::new(rows.len(), cols.len(), data)
                }
            }
            ExprKind::Call { name, args } => {
                let mut values = Vec::with_capacity(args.len());
                for arg in args {
                    values.push(self.eval(arg)?);
                }
                let mut warnings = Vec::new();
                let result = functions::call(name, &values, &self.engine.random, &mut warnings);
                for warning in warnings {
                    self.engine.warn(warning);
                }
                result.map_err(|m| fault(span, m))?
            }
            ExprKind::Eof(file) => {
                let key = file.to_uppercase();
                let at_end = match self.reads.get_mut(&key) {
                    Some(state) => {
                        state.eof_called = true;
                        state.next >= state.records.len()
                    }
                    None => false,
                };
                Mat::scalar(f64::from(u8::from(at_end)))
            }
            ExprKind::Neg(operand) => self.eval(operand)?.map(|v| -v),
            ExprKind::Not(operand) => self.eval(operand)?.map(|v| f64::from(u8::from(v <= 0.0))),
            ExprKind::Binary { op, left, right } => {
                let (a, b) = (self.eval(left)?, self.eval(right)?);
                binary(*op, &a, &b).map_err(|m| fault(span, m))?
            }
            ExprKind::Sequence { first, last, step } => {
                let scalar = |m: Mat, what: &str, at: Span| -> Run<i64> {
                    if !m.is_scalar() {
                        return Err(fault(
                            at,
                            format!(
                                "the {what} of a range must be a scalar, but it is {}",
                                m.dims()
                            ),
                        ));
                    }
                    Ok(whole(m.data[0]))
                };
                let a = scalar(self.eval(first)?, "start", first.span)?;
                let b = scalar(self.eval(last)?, "end", last.span)?;
                let c = match step {
                    Some(step) => scalar(self.eval(step)?, "increment", step.span)?,
                    None => 1,
                };
                if c == 0 {
                    return Err(fault(span, "the increment of a range must not be 0"));
                }
                let count = if (c > 0 && b >= a) || (c < 0 && b <= a) {
                    ((b - a) / c + 1) as usize
                } else {
                    0
                };
                if count > 100_000_000 {
                    return Err(fault(span, "the range has too many elements"));
                }
                let data: Vec<f64> = (0..count as i64).map(|k| (a + k * c) as f64).collect();
                Mat::new(1, data.len(), data)
            }
        })
    }

    /// The 0-based rows and columns that indexes select from `m`.
    fn positions(
        &mut self,
        m: &Mat,
        name: &str,
        indexes: &[Index],
        span: Span,
    ) -> Run<(Vec<usize>, Vec<usize>)> {
        let mut pick = |index: &Index, size: usize, what: &str| -> Run<Vec<usize>> {
            match index {
                Index::All => Ok((0..size).collect()),
                Index::Expr(e) => {
                    let v = self.eval(e)?;
                    if !v.is_vector() && !v.data.is_empty() {
                        return Err(fault(
                            e.span,
                            format!(
                                "an index must be a scalar or a vector, but it is {}",
                                v.dims()
                            ),
                        ));
                    }
                    v.data
                        .iter()
                        .map(|&x| {
                            let k = whole(x);
                            if k < 1 || k as usize > size {
                                Err(fault(
                                    e.span,
                                    format!("index {k} is out of range: {name} has {size} {what}"),
                                ))
                            } else {
                                Ok(k as usize - 1)
                            }
                        })
                        .collect()
                }
            }
        };
        match indexes {
            [one] => {
                if !m.is_vector() {
                    return Err(fault(
                        span,
                        format!(
                            "{name} is a {} matrix and needs two indexes, rows and columns",
                            m.dims()
                        ),
                    ));
                }
                if m.rows == 1 {
                    Ok((vec![0], pick(one, m.cols, "elements")?))
                } else {
                    Ok((pick(one, m.rows, "elements")?, vec![0]))
                }
            }
            [rows, cols] => Ok((pick(rows, m.rows, "rows")?, pick(cols, m.cols, "columns")?)),
            _ => unreachable!("the parser allows one or two indexes"),
        }
    }

    fn assign(&mut self, target: &Target, value: Mat) -> Run<()> {
        let Some(indexes) = &target.index else {
            self.set(&target.name, value);
            return Ok(());
        };
        let mut m = self
            .get(&target.name, target.span)
            .map_err(|_| {
                fault(
                    target.span,
                    format!(
                        "the matrix variable {} must exist before a part of it is set",
                        target.name
                    ),
                )
            })?
            .clone();
        let (rows, cols) = self.positions(&m, &target.name, indexes, target.span)?;
        let fits = value.is_scalar()
            || (value.rows == rows.len() && value.cols == cols.len())
            || (indexes.len() == 1
                && value.is_vector()
                && value.data.len() == rows.len() * cols.len());
        if !fits {
            return Err(fault(
                target.span,
                format!(
                    "the value is {} but the part of {} it goes to is {}x{}",
                    value.dims(),
                    target.name,
                    rows.len(),
                    cols.len()
                ),
            ));
        }
        let mut k = 0;
        for &i in &rows {
            for &j in &cols {
                let v = if value.is_scalar() {
                    value.data[0]
                } else {
                    value.data[k]
                };
                m.set(i, j, v);
                k += 1;
            }
        }
        self.set(&target.name, m);
        Ok(())
    }

    fn call(&mut self, call: &CallStatement, span: Span) -> Run<()> {
        match call {
            CallStatement::Eigen {
                matrix,
                vectors,
                values,
            } => {
                let m = self.eval(matrix)?;
                if m.rows != m.cols {
                    return Err(fault(
                        matrix.span,
                        format!(
                            "EIGEN needs a square symmetric matrix, but it is {}",
                            m.dims()
                        ),
                    ));
                }
                let mut warnings = Vec::new();
                functions::call(
                    "EVAL",
                    std::slice::from_ref(&m),
                    &self.engine.random,
                    &mut warnings,
                )
                .map_err(|e| fault(matrix.span, e.replace("EVAL", "EIGEN")))?;
                let (e, v) = functions::eigen(&m);
                self.set(vectors, v);
                self.set(values, e);
            }
            CallStatement::Setdiag { matrix, values } => {
                let v = self.eval(values)?;
                let mut m = self.get(matrix, span)?.clone();
                if !v.is_vector() {
                    return Err(fault(
                        values.span,
                        format!("SETDIAG needs a scalar or a vector, but it is {}", v.dims()),
                    ));
                }
                let n = m.rows.min(m.cols);
                for i in 0..n {
                    if v.is_scalar() {
                        m.set(i, i, v.data[0]);
                    } else if let Some(x) = v.data.get(i) {
                        m.set(i, i, *x);
                    }
                }
                self.set(matrix, m);
            }
            CallStatement::Svd { matrix, u, q, v } => {
                let m = self.eval(matrix)?;
                let (mu, mq, mv) = functions::svd(&m);
                self.set(u, mu);
                self.set(q, mq);
                self.set(v, mv);
            }
        }
        Ok(())
    }

    // ---- PRINT ----

    fn labels(&mut self, labels: &Option<Labels>) -> Run<Option<Vec<String>>> {
        Ok(match labels {
            None => None,
            Some(Labels::List(list)) => Some(list.clone()),
            Some(Labels::Names(expr)) => {
                let v = self.eval(expr)?;
                if !v.is_vector() {
                    return Err(fault(
                        expr.span,
                        format!("RNAMES and CNAMES need a vector, but it is {}", v.dims()),
                    ));
                }
                Some(v.data.iter().map(|x| unpack(*x)).collect())
            }
        })
    }

    fn print(&mut self, print: &MatrixPrint) -> Run<()> {
        let title = print.title.clone().unwrap_or_else(|| print.text.clone());
        let Some(expr) = &print.expr else {
            self.tables.push(Table {
                title,
                ..Table::default()
            });
            return Ok(());
        };
        let m = self.eval(expr)?;
        let rlabels = self.labels(&print.rlabels)?;
        let clabels = self.labels(&print.clabels)?;
        let label = |list: &Option<Vec<String>>, k: usize, default: &str| match list {
            Some(list) => list.get(k).cloned().unwrap_or_default(),
            None => format!("{default}{}", k + 1),
        };
        let mut columns = vec![String::new()];
        columns.extend((0..m.cols).map(|j| label(&clabels, j, "col")));
        let (values, format, caption) = match &print.format {
            Some(format) => (m.data.clone(), format.clone(), None),
            None => auto_format(&m.data),
        };
        let string = format.starts_with('A');
        let width: usize = format
            .trim_start_matches(|c: char| c.is_ascii_alphabetic())
            .split('.')
            .next()
            .and_then(|w| w.parse().ok())
            .unwrap_or(8);
        let mut rows = Vec::new();
        let mut formats = Vec::new();
        for i in 0..m.rows {
            let mut row = vec![Cell::Text(label(&rlabels, i, "row"))];
            let mut row_formats = vec![String::new()];
            for j in 0..m.cols {
                let v = values[i * m.cols + j];
                let cell = if string {
                    Cell::Text(unpack(v).chars().take(width).collect())
                } else if v.is_nan() {
                    Cell::Text("NaN".into())
                } else if v.is_infinite() {
                    Cell::Text(if v > 0.0 { "+Inf" } else { "-Inf" }.into())
                } else {
                    Cell::Number(v)
                };
                row_formats.push(if matches!(cell, Cell::Number(_)) {
                    format.clone()
                } else {
                    String::new()
                });
                row.push(cell);
            }
            rows.push(row);
            formats.push(row_formats);
        }
        self.tables.push(Table {
            title,
            row_headers: 1,
            columns,
            rows,
            formats,
            caption,
            ..Table::default()
        });
        Ok(())
    }

    // ---- DISPLAY ----

    fn display(&mut self, dictionary: bool, status: bool) {
        if dictionary {
            let rows = self
                .vars
                .values()
                .map(|(name, m)| {
                    vec![
                        Cell::Text(name.clone()),
                        Cell::Number(m.rows as f64),
                        Cell::Number(m.cols as f64),
                    ]
                })
                .collect();
            self.tables.push(Table {
                title: "Matrix variables".into(),
                columns: vec!["Name".into(), "Rows".into(), "Columns".into()],
                rows,
                ..Table::default()
            });
        }
        if status {
            let elements: usize = self.vars.values().map(|(_, m)| m.data.len()).sum();
            self.tables.push(Table {
                title: "Matrix status".into(),
                columns: vec!["Item".into(), "Value".into()],
                rows: vec![
                    vec![
                        Cell::Text("Variables".into()),
                        Cell::Number(self.vars.len() as f64),
                    ],
                    vec![Cell::Text("Elements".into()), Cell::Number(elements as f64)],
                    vec![
                        Cell::Text("Bytes".into()),
                        Cell::Number((elements * 8) as f64),
                    ],
                ],
                ..Table::default()
            });
        }
    }

    // ---- READ and WRITE ----

    /// The size of the target of READ or GET: of its part when it has indexes.
    fn target_part(&mut self, target: &Target) -> Run<Option<(Vec<usize>, Vec<usize>)>> {
        let Some(indexes) = &target.index else {
            return Ok(None);
        };
        let m = self
            .get(&target.name, target.span)
            .map_err(|_| {
                fault(
                    target.span,
                    format!(
                        "the matrix variable {} must exist before a part of it is read",
                        target.name
                    ),
                )
            })?
            .clone();
        Ok(Some(self.positions(
            &m,
            &target.name,
            indexes,
            target.span,
        )?))
    }

    /// Stores what READ or GET read: the whole variable, or the part the indexes select.
    fn store(&mut self, target: &Target, value: Mat) -> Run<()> {
        self.assign(target, value)
    }

    fn read(&mut self, read: &MatrixRead, span: Span) -> Run<()> {
        let part = self.target_part(&read.target)?;
        let size = match &read.size {
            Some(expr) => {
                let v = self.eval(expr)?;
                let (r, c) = match v.data.as_slice() {
                    [n] if v.is_scalar() => (whole(*n), 1),
                    [r, c] => (whole(*r), whole(*c)),
                    _ => {
                        return Err(fault(
                            expr.span,
                            format!("SIZE needs a scalar or two numbers, but it is {}", v.dims()),
                        ));
                    }
                };
                if r < 0 || c < 0 {
                    return Err(fault(expr.span, "SIZE needs sizes of 0 or more"));
                }
                Some((r as usize, c as usize))
            }
            None => None,
        };
        let (rows, cols) = match (&part, size) {
            (Some((r, c)), Some(size)) if (r.len(), c.len()) != size => {
                return Err(fault(
                    span,
                    format!(
                        "SIZE gives {}x{} but the part of {} is {}x{}",
                        size.0,
                        size.1,
                        read.target.name,
                        r.len(),
                        c.len()
                    ),
                ));
            }
            (Some((r, c)), _) => (r.len(), c.len()),
            (None, Some(size)) => size,
            (None, None) => unreachable!("the parser asks for SIZE"),
        };
        if read.symmetric && rows != cols {
            return Err(fault(
                span,
                format!("MODE=SYMMETRIC needs a square matrix, but it is {rows}x{cols}"),
            ));
        }
        let key = read.file.to_uppercase();
        if !self.reads.contains_key(&key) {
            let records = self
                .engine
                .file_records(&read.file)
                .map_err(|e| fault(span, e.0))?;
            self.reads.insert(
                key.clone(),
                ReadFile {
                    records: records.into_iter().map(|r| r.text).collect(),
                    next: 0,
                    last: None,
                    eof_called: false,
                },
            );
        }
        let state = self.reads.get_mut(&key).expect("inserted");
        if read.reread && !state.eof_called {
            match state.last {
                Some(last) => state.next = last,
                None => {
                    return Err(fault(
                        span,
                        format!("REREAD needs an earlier READ of {}", read.file),
                    ));
                }
            }
        }
        state.eof_called = false;
        let field = read.field;
        let width = field_width(field.first, field.last, field.width, read.format.as_ref());
        let format = read.format.unwrap_or(DataFormat {
            repeat: None,
            kind: statscraft_syntax::FormatType::F,
            width: None,
            decimals: 0,
        });
        let mut out = Mat::filled(rows, cols, f64::NAN);
        let mut short = false;
        for i in 0..rows {
            let wanted = if read.symmetric { i + 1 } else { cols };
            let mut values: Vec<String> = Vec::new();
            while values.len() < wanted {
                let Some(record) = state.records.get(state.next) else {
                    short = true;
                    break;
                };
                state.last = Some(state.next);
                state.next += 1;
                values.extend(split_field(record, field.first, field.last, width));
            }
            for (j, text) in values.iter().take(wanted).enumerate() {
                let value = if format.kind == statscraft_syntax::FormatType::A {
                    pack(text.trim())
                } else {
                    crate::data_list::number(text, format.kind, format.decimals as u8)
                        .map_err(|m| fault(span, format!("READ: {} {m}", text.trim())))?
                        .unwrap_or(f64::NAN)
                };
                out.set(i, j, value);
                if read.symmetric {
                    out.set(j, i, value);
                }
            }
        }
        if short {
            self.engine.warn(format!(
                "READ reached the end of {} before the matrix was full; the rest is missing",
                read.file
            ));
        }
        self.store(&read.target, out)
    }

    fn write(&mut self, write: &MatrixWrite, span: Span) -> Run<()> {
        let m = self.eval(&write.expr)?;
        if write.triangular && m.rows != m.cols {
            return Err(fault(
                span,
                format!(
                    "MODE=TRIANGULAR needs a square matrix, but it is {}",
                    m.dims()
                ),
            ));
        }
        let field = write.field;
        let width = field_width(field.first, field.last, field.width, write.format.as_ref());
        let format = write.format;
        let text = |v: f64| -> String {
            match format {
                Some(f) if f.kind == statscraft_syntax::FormatType::A => unpack(v),
                Some(f) if width.is_some() => {
                    let w = width.expect("checked");
                    crate::format_value(v, &format!("{}{}.{}", f.kind.name(), w, f.decimals), false)
                }
                Some(f) => {
                    crate::format_value(v, &format!("{}40.{}", f.kind.name(), f.decimals), true)
                }
                None => free_number(v),
            }
        };
        let position = match self
            .writes
            .iter()
            .position(|(f, _)| f.eq_ignore_ascii_case(&write.outfile))
        {
            Some(at) => at,
            None => {
                self.writes
                    .push((write.outfile.clone(), WriteFile::default()));
                self.writes.len() - 1
            }
        };
        let file = &mut self.writes[position].1;
        let area = field.last - field.first + 1;
        let indent = " ".repeat(field.first - 1);
        for i in 0..m.rows {
            let count = if write.triangular { i + 1 } else { m.cols };
            let mut line = match file.held.take() {
                Some(held) => {
                    let mut held: String = held.chars().take(field.first - 1).collect();
                    while held.chars().count() < field.first - 1 {
                        held.push(' ');
                    }
                    held
                }
                None => indent.clone(),
            };
            let mut used = 0;
            for j in 0..count {
                let v = m.at(i, j);
                let piece = match width {
                    Some(w) => {
                        let t = text(v);
                        let t: String = if t.chars().count() > w {
                            "*".repeat(w)
                        } else {
                            t
                        };
                        if format.is_some_and(|f| f.kind == statscraft_syntax::FormatType::A) {
                            format!("{t:<w$}")
                        } else {
                            format!("{t:>w$}")
                        }
                    }
                    None => {
                        let t = text(v);
                        if used > 0 { format!(" {t}") } else { t }
                    }
                };
                let length = piece.chars().count();
                if used > 0 && used + length > area {
                    file.lines
                        .push(std::mem::replace(&mut line, indent.clone()));
                    used = 0;
                    let piece = piece.trim_start().to_string();
                    let piece = match width {
                        Some(w)
                            if format
                                .is_none_or(|f| f.kind != statscraft_syntax::FormatType::A) =>
                        {
                            format!("{piece:>w$}")
                        }
                        _ => piece,
                    };
                    used += piece.chars().count();
                    line.push_str(&piece);
                    continue;
                }
                used += length;
                line.push_str(&piece);
            }
            if write.hold && i + 1 == m.rows {
                file.held = Some(line);
            } else {
                file.lines.push(line);
            }
        }
        Ok(())
    }

    // ---- GET, SAVE, MGET, MSAVE ----

    fn source(&self, file: &Option<String>, span: Span) -> Run<Dataset> {
        match file {
            None => self.engine.active().cloned().map_err(|e| fault(span, e.0)),
            Some(name) => self.engine.data_file(name).map_err(|e| fault(span, e.0)),
        }
    }

    fn get_data(&mut self, get: &MatrixGet, span: Span) -> Run<()> {
        let data = self.source(&get.file, span)?;
        let names = match &get.variables {
            Some(items) => crate::expand_in(&data, items).map_err(|e| fault(span, e.0))?,
            None => data
                .variable_names()
                .filter(|n| !crate::rows::is_scratch(n))
                .map(str::to_string)
                .collect(),
        };
        let mut columns = Vec::new();
        let mut kept = Vec::new();
        for name in &names {
            let variable = data
                .variable(name)
                .map_err(|e| fault(span, e.to_string()))?;
            match variable.values() {
                Values::Numeric(values) => {
                    let user: Vec<bool> = values
                        .iter()
                        .map(|v| v.is_some_and(|v| variable.is_user_missing(v)))
                        .collect();
                    columns.push((values.iter().collect::<Vec<_>>(), user));
                    kept.push(variable.name().to_string());
                }
                Values::String(_) => self
                    .engine
                    .warn(format!("GET skips the string variable {name}")),
            }
        }
        let mut rows: Vec<Vec<f64>> = Vec::new();
        'cases: for case in 0..data.case_count() {
            let mut row = Vec::with_capacity(columns.len());
            for (j, (values, user)) in columns.iter().enumerate() {
                let value = match (values[case], user[case]) {
                    (Some(v), false) => v,
                    (Some(v), true) => match get.missing {
                        None => {
                            return Err(fault(
                                span,
                                format!(
                                    "{} has a user-missing value in case {}; say how to treat it with MISSING=ACCEPT, OMIT or a value",
                                    kept[j],
                                    case + 1
                                ),
                            ));
                        }
                        Some(MissingAction::Accept) => v,
                        Some(MissingAction::Omit) => continue 'cases,
                        Some(MissingAction::Value(x)) => x,
                    },
                    (None, _) => match (get.missing, get.sysmis) {
                        (None, _) => {
                            return Err(fault(
                                span,
                                format!(
                                    "{} is system-missing in case {}; say how to treat it with MISSING=OMIT or a value",
                                    kept[j],
                                    case + 1
                                ),
                            ));
                        }
                        (Some(MissingAction::Omit), _) => continue 'cases,
                        (Some(MissingAction::Value(x)), _) => x,
                        (Some(MissingAction::Accept), Some(MissingAction::Omit)) => continue 'cases,
                        (Some(MissingAction::Accept), Some(MissingAction::Value(x))) => x,
                        (Some(MissingAction::Accept), _) => {
                            return Err(fault(
                                span,
                                format!(
                                    "{} is system-missing in case {}; with MISSING=ACCEPT say how to treat it with SYSMIS=OMIT or a value",
                                    kept[j],
                                    case + 1
                                ),
                            ));
                        }
                    },
                };
                row.push(value);
            }
            rows.push(row);
        }
        let value = Mat::from_rows(&rows, kept.len());
        if let Some(name) = &get.names {
            let names: Vec<f64> = kept.iter().map(|n| pack(n)).collect();
            self.set(name, Mat::new(1, names.len(), names));
        }
        self.store(&get.target, value)
    }

    fn save(&mut self, save: &MatrixSave, span: Span) -> Run<()> {
        let m = self.eval(&save.expr)?;
        let position = self
            .saves
            .iter()
            .position(|(target, _)| match (target, &save.outfile) {
                (None, None) => true,
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                _ => false,
            });
        let at = match position {
            Some(at) => at,
            None => {
                let given = match (&save.variables, &save.names) {
                    (Some(variables), names) => {
                        if names.is_some() {
                            self.engine.warn(
                                "SAVE has both VARIABLES and NAMES; VARIABLES gives the names",
                            );
                        }
                        variables.clone()
                    }
                    (None, Some(names)) => {
                        let v = self.eval(names)?;
                        v.data.iter().map(|x| unpack(*x)).collect()
                    }
                    (None, None) => Vec::new(),
                };
                let names: Vec<String> = (0..m.cols)
                    .map(|j| {
                        given
                            .get(j)
                            .cloned()
                            .unwrap_or_else(|| format!("COL{}", j + 1))
                    })
                    .collect();
                for (j, name) in names.iter().enumerate() {
                    if !statscraft_data::is_valid_name(name) {
                        return Err(fault(
                            span,
                            format!("SAVE: {name} is not a valid variable name"),
                        ));
                    }
                    if names[..j].iter().any(|n| n.eq_ignore_ascii_case(name)) {
                        return Err(fault(
                            span,
                            format!("SAVE: the variable name {name} comes twice"),
                        ));
                    }
                }
                let mut strings = vec![false; names.len()];
                for s in &save.strings {
                    match names.iter().position(|n| n.eq_ignore_ascii_case(s)) {
                        Some(k) => strings[k] = true,
                        None => {
                            return Err(fault(
                                span,
                                format!(
                                    "STRINGS names {s}, which is not one of the variables of SAVE"
                                ),
                            ));
                        }
                    }
                }
                self.saves.push((
                    save.outfile.clone(),
                    SaveFile {
                        names,
                        strings,
                        rows: Vec::new(),
                    },
                ));
                self.saves.len() - 1
            }
        };
        let file = &mut self.saves[at].1;
        if m.cols != file.names.len() {
            return Err(fault(
                span,
                format!(
                    "SAVE adds {} columns to a file of {} variables",
                    m.cols,
                    file.names.len()
                ),
            ));
        }
        file.rows.extend(m.to_rows());
        Ok(())
    }

    fn mget(&mut self, mget: &MatrixMget, span: Span) -> Run<()> {
        let data = self.source(&mget.file, span)?;
        let find = |name: &str| {
            data.variables()
                .position(|v| v.name().eq_ignore_ascii_case(name) && v.is_string())
                .ok_or_else(|| {
                    fault(
                        span,
                        format!("MGET needs the string variable {name} in a matrix data file"),
                    )
                })
        };
        let (rowtype, varname) = (find("ROWTYPE_")?, find("VARNAME_")?);
        let all: Vec<statscraft_data::Variable> = data.variables().collect();
        let splits: Vec<usize> = (0..rowtype).collect();
        let factors: Vec<usize> = (rowtype + 1..varname).collect();
        let continuous: Vec<usize> = (varname + 1..all.len())
            .filter(|&k| !all[k].is_string())
            .collect();
        let number = |k: usize, case: usize| match all[k].values() {
            Values::Numeric(v) => v.get(case),
            Values::String(_) => None,
        };
        let text = |k: usize, case: usize| match all[k].values() {
            Values::String(s) => s.get(case).trim().to_uppercase(),
            Values::Numeric(_) => String::new(),
        };
        let key = |ks: &[usize], case: usize| -> Vec<Option<u64>> {
            ks.iter()
                .map(|&k| number(k, case).map(f64::to_bits))
                .collect()
        };
        let mut split_groups: Vec<Vec<Option<u64>>> = Vec::new();
        let mut cells: Vec<Vec<Option<u64>>> = Vec::new();
        // (name, rows)
        let mut found: Vec<(String, Vec<Vec<f64>>)> = Vec::new();
        for case in 0..data.case_count() {
            let prefix = match text(rowtype, case).as_str() {
                "COV" => ("CV", "COV"),
                "CORR" => ("CR", "CORR"),
                "MEAN" => ("MN", "MEAN"),
                "STDDEV" | "SD" => ("SD", "STDDEV"),
                "N" | "N_VECTOR" => ("NC", "N"),
                "COUNT" => ("CN", "COUNT"),
                _ => continue,
            };
            if !mget.types.is_empty() && !mget.types.iter().any(|t| t == prefix.1) {
                continue;
            }
            let mut name = prefix.0.to_string();
            let cell = key(&factors, case);
            if !factors.is_empty() && cell.iter().any(Option::is_some) {
                let at = cells.iter().position(|c| *c == cell).unwrap_or_else(|| {
                    cells.push(cell.clone());
                    cells.len() - 1
                });
                name.push_str(&format!("F{}", at + 1));
            }
            if !splits.is_empty() {
                let group = key(&splits, case);
                let at = split_groups
                    .iter()
                    .position(|g| *g == group)
                    .unwrap_or_else(|| {
                        split_groups.push(group.clone());
                        split_groups.len() - 1
                    });
                name.push_str(&format!("S{}", at + 1));
            }
            let row: Vec<f64> = continuous
                .iter()
                .map(|&k| number(k, case).unwrap_or(f64::NAN))
                .collect();
            match found.iter_mut().find(|(n, _)| *n == name) {
                Some((_, rows)) => rows.push(row),
                None => found.push((name, vec![row])),
            }
        }
        for wanted in &mget.types {
            let prefix = match wanted.as_str() {
                "COV" => "CV",
                "CORR" => "CR",
                "MEAN" => "MN",
                "STDDEV" => "SD",
                "N" => "NC",
                _ => "CN",
            };
            if !found.iter().any(|(n, _)| n.starts_with(prefix)) {
                return Err(fault(
                    span,
                    format!("the matrix data file has no records of type {wanted}"),
                ));
            }
        }
        let mut rows = Vec::new();
        for (name, records) in found {
            let value = Mat::from_rows(&records, continuous.len());
            if self.vars.contains_key(&name) {
                self.engine.warn(format!(
                    "MGET does not replace the existing matrix variable {name}"
                ));
                continue;
            }
            rows.push(vec![
                Cell::Text(name.clone()),
                Cell::Number(value.rows as f64),
                Cell::Number(value.cols as f64),
            ]);
            self.set(&name, value);
        }
        self.tables.push(Table {
            title: "Matrix variables created by MGET".into(),
            columns: vec!["Name".into(), "Rows".into(), "Columns".into()],
            rows,
            ..Table::default()
        });
        Ok(())
    }

    fn msave(&mut self, msave: &MatrixMsave, span: Span) -> Run<()> {
        let mut m = self.eval(&msave.expr)?;
        let matrix = matches!(msave.kind.as_str(), "COV" | "CORR");
        if !matrix {
            if !m.is_vector() {
                return Err(fault(
                    span,
                    format!(
                        "MSAVE /TYPE={} needs a vector, but it is {}",
                        msave.kind,
                        m.dims()
                    ),
                ));
            }
            if m.cols == 1 && m.rows > 1 {
                m = m.transpose();
            }
        }
        let vector = |s: &mut Self, e: &Option<MExpr>| -> Run<Option<Vec<f64>>> {
            match e {
                Some(e) => {
                    let v = s.eval(e)?;
                    if !v.is_vector() {
                        return Err(fault(
                            e.span,
                            format!("SPLIT and FACTOR need a vector, but it is {}", v.dims()),
                        ));
                    }
                    Ok(Some(v.data))
                }
                None => Ok(None),
            }
        };
        let split = vector(self, &msave.split)?;
        let factor = vector(self, &msave.factor)?;
        let file = self.msave.get_or_insert_with(|| MatrixFile {
            outfile: msave.outfile.clone(),
            snames: Vec::new(),
            fnames: Vec::new(),
            variables: Vec::new(),
            records: Vec::new(),
        });
        let names = |given: &Option<Vec<String>>, count: usize, stem: &str| -> Vec<String> {
            (0..count)
                .map(|k| {
                    given
                        .as_ref()
                        .and_then(|g| g.get(k).cloned())
                        .unwrap_or_else(|| format!("{stem}{}", k + 1))
                })
                .collect()
        };
        if file.records.is_empty() {
            file.variables = names(&msave.variables, m.cols, "COL");
            let splits = msave
                .snames
                .as_ref()
                .map_or(split.as_ref().map_or(0, Vec::len), Vec::len);
            let factors = msave
                .fnames
                .as_ref()
                .map_or(factor.as_ref().map_or(0, Vec::len), Vec::len);
            file.snames = names(&msave.snames, splits, "SPL");
            file.fnames = names(&msave.fnames, factors, "FAC");
        }
        if m.cols != file.variables.len() {
            return Err(fault(
                span,
                format!(
                    "MSAVE writes {} columns to a matrix file of {} variables",
                    m.cols,
                    file.variables.len()
                ),
            ));
        }
        let split = split.unwrap_or_else(|| vec![f64::NAN; file.snames.len()]);
        if split.len() != file.snames.len() {
            return Err(fault(
                span,
                format!(
                    "SPLIT gives {} values for {} split variables",
                    split.len(),
                    file.snames.len()
                ),
            ));
        }
        let factor = factor.unwrap_or_else(|| vec![f64::NAN; file.fnames.len()]);
        if factor.len() != file.fnames.len() {
            return Err(fault(
                span,
                format!(
                    "FACTOR gives {} values for {} factors",
                    factor.len(),
                    file.fnames.len()
                ),
            ));
        }
        for (i, row) in m.to_rows().into_iter().enumerate() {
            let varname = if matrix {
                file.variables.get(i).cloned().unwrap_or_default()
            } else {
                String::new()
            };
            file.records.push((
                split.clone(),
                msave.kind.clone(),
                factor.clone(),
                varname,
                row,
            ));
        }
        Ok(())
    }
}

/// The width of a fixed field, when READ or WRITE has one.
fn field_width(
    first: usize,
    last: usize,
    by: Option<usize>,
    format: Option<&DataFormat>,
) -> Option<usize> {
    by.or(format.and_then(|f| f.width)).or(format
        .and_then(|f| f.repeat)
        .map(|r| (last - first + 1) / r))
}

/// The values of a record within the columns: fixed fields of `width`, or separated by blanks
/// and commas.
fn split_field(record: &str, first: usize, last: usize, width: Option<usize>) -> Vec<String> {
    let chars: Vec<char> = record.chars().collect();
    let area: String = chars
        .iter()
        .skip(first - 1)
        .take(last - first + 1)
        .collect();
    match width {
        Some(w) => {
            let area: Vec<char> = area.chars().collect();
            let used = area.len();
            // a field the record does not reach is no value
            (0..(last - first + 1) / w)
                .filter(|k| k * w < used)
                .map(|k| area[k * w..((k + 1) * w).min(used)].iter().collect())
                .collect()
        }
        None => area
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect(),
    }
}

/// A number as short as it can be written without losing precision.
fn free_number(v: f64) -> String {
    if v.is_nan() {
        ".".into()
    } else {
        format!("{v}")
    }
}

/// The values, format and caption of PRINT without FORMAT.
fn auto_format(values: &[f64]) -> (Vec<f64>, String, Option<String>) {
    let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    let largest = finite.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let negative = finite.iter().any(|v| *v < 0.0);
    let digits = |x: f64| {
        if x < 1.0 {
            0
        } else {
            x.log10().floor() as usize + 1
        }
    };
    if finite.iter().all(|v| v.fract() == 0.0) && largest < 1e9 {
        let width = digits(largest).max(1) + usize::from(negative);
        return (values.to_vec(), format!("F{}.0", width.max(1)), None);
    }
    let power = if largest >= 1e9 || (largest > 0.0 && largest < 1e-3) {
        largest.log10().floor() as i32
    } else {
        0
    };
    let scaled: Vec<f64> = values.iter().map(|v| v / 10f64.powi(power)).collect();
    let top = largest / 10f64.powi(power);
    let decimals = (10 - digits(top) as i32).clamp(1, 9) as usize;
    let width = digits(top).max(1) + decimals + 1 + usize::from(negative);
    let caption = (power != 0).then(|| format!("The values are shown divided by 10**{power}"));
    (scaled, format!("F{width}.{decimals}"), caption)
}

fn hconcat(parts: Vec<(Mat, Span)>) -> Run<Mat> {
    let parts: Vec<(Mat, Span)> = parts
        .into_iter()
        .filter(|(m, _)| m.rows * m.cols > 0)
        .collect();
    let Some((first, _)) = parts.first() else {
        return Ok(Mat::new(0, 0, Vec::new()));
    };
    let rows = first.rows;
    if let Some((m, span)) = parts.iter().find(|(m, _)| m.rows != rows) {
        return Err(fault(
            *span,
            format!(
                "the parts of a row of a matrix need the same number of rows, but this one has {} and the first {rows}",
                m.rows
            ),
        ));
    }
    let cols = parts.iter().map(|(m, _)| m.cols).sum();
    let mut out = Mat::filled(rows, cols, 0.0);
    let mut offset = 0;
    for (m, _) in &parts {
        for i in 0..rows {
            for j in 0..m.cols {
                out.set(i, offset + j, m.at(i, j));
            }
        }
        offset += m.cols;
    }
    Ok(out)
}

fn vconcat(rows: Vec<Mat>, span: Span) -> Run<Mat> {
    let rows: Vec<Mat> = rows.into_iter().filter(|m| m.rows * m.cols > 0).collect();
    let Some(first) = rows.first() else {
        return Ok(Mat::new(0, 0, Vec::new()));
    };
    let cols = first.cols;
    if let Some(m) = rows.iter().find(|m| m.cols != cols) {
        return Err(fault(
            span,
            format!(
                "the rows of a matrix need the same number of columns, but they have {cols} and {}",
                m.cols
            ),
        ));
    }
    let data: Vec<f64> = rows.iter().flat_map(|m| m.data.iter().copied()).collect();
    Ok(Mat::new(data.len() / cols, cols, data))
}

fn op_name(op: MatrixOp) -> &'static str {
    match op {
        MatrixOp::Add => "+",
        MatrixOp::Sub => "-",
        MatrixOp::Mul => "*",
        MatrixOp::Div => "/",
        MatrixOp::Pow => "**",
        MatrixOp::ElemMul => "&*",
        MatrixOp::ElemDiv => "&/",
        MatrixOp::ElemPow => "&**",
        MatrixOp::Eq => "=",
        MatrixOp::Ne => "<>",
        MatrixOp::Lt => "<",
        MatrixOp::Le => "<=",
        MatrixOp::Gt => ">",
        MatrixOp::Ge => ">=",
        MatrixOp::And => "AND",
        MatrixOp::Or => "OR",
        MatrixOp::Xor => "XOR",
    }
}

/// Element by element, a scalar going with every element.
fn zip(op: MatrixOp, a: &Mat, b: &Mat, f: impl Fn(f64, f64) -> f64) -> Result<Mat, String> {
    let (rows, cols) = if a.is_scalar() {
        (b.rows, b.cols)
    } else if b.is_scalar() || (a.rows, a.cols) == (b.rows, b.cols) {
        (a.rows, a.cols)
    } else {
        return Err(format!(
            "{} needs matrices of the same size or a scalar, but they are {} and {}",
            op_name(op),
            a.dims(),
            b.dims()
        ));
    };
    let pick = |m: &Mat, k: usize| if m.is_scalar() { m.data[0] } else { m.data[k] };
    let data = (0..rows * cols)
        .map(|k| f(pick(a, k), pick(b, k)))
        .collect();
    Ok(Mat::new(rows, cols, data))
}

fn binary(op: MatrixOp, a: &Mat, b: &Mat) -> Result<Mat, String> {
    let truth = |b: bool| f64::from(u8::from(b));
    match op {
        MatrixOp::Add => zip(op, a, b, |x, y| x + y),
        MatrixOp::Sub => zip(op, a, b, |x, y| x - y),
        MatrixOp::Div | MatrixOp::ElemDiv => zip(op, a, b, |x, y| x / y),
        MatrixOp::ElemMul => zip(op, a, b, |x, y| x * y),
        MatrixOp::ElemPow => zip(op, a, b, f64::powf),
        MatrixOp::Eq => zip(op, a, b, |x, y| truth(x == y)),
        MatrixOp::Ne => zip(op, a, b, |x, y| truth(x != y)),
        MatrixOp::Lt => zip(op, a, b, |x, y| truth(x < y)),
        MatrixOp::Le => zip(op, a, b, |x, y| truth(x <= y)),
        MatrixOp::Gt => zip(op, a, b, |x, y| truth(x > y)),
        MatrixOp::Ge => zip(op, a, b, |x, y| truth(x >= y)),
        MatrixOp::And => zip(op, a, b, |x, y| truth(x > 0.0 && y > 0.0)),
        MatrixOp::Or => zip(op, a, b, |x, y| truth(x > 0.0 || y > 0.0)),
        MatrixOp::Xor => zip(op, a, b, |x, y| truth((x > 0.0) != (y > 0.0))),
        MatrixOp::Mul if a.is_scalar() || b.is_scalar() => zip(op, a, b, |x, y| x * y),
        MatrixOp::Mul => {
            if a.cols != b.rows {
                return Err(format!(
                    "* needs as many columns in the first matrix as rows in the second, but they are {} and {}",
                    a.dims(),
                    b.dims()
                ));
            }
            Ok(a.product(b))
        }
        MatrixOp::Pow => {
            if !b.is_scalar() {
                return Err(format!(
                    "the exponent of ** must be a scalar, but it is {}",
                    b.dims()
                ));
            }
            let e = b.data[0];
            if a.is_scalar() {
                return Ok(Mat::scalar(a.data[0].powf(e)));
            }
            if a.rows != a.cols {
                return Err(format!("** needs a square matrix, but it is {}", a.dims()));
            }
            if e.fract() != 0.0 {
                return Err(format!(
                    "a matrix can only be raised to a whole power, not {e}"
                ));
            }
            let mut result = Mat::identity(a.rows);
            let mut base = a.clone();
            let mut n = e.abs() as u64;
            while n > 0 {
                if n & 1 == 1 {
                    result = result.product(&base);
                }
                base = base.product(&base);
                n >>= 1;
            }
            if e < 0.0 {
                return match crate::matrix::inverse(&result.to_rows()) {
                    Some((inverse, _)) => Ok(Mat::from_rows(&inverse, a.cols)),
                    None => Err("** with a negative power needs a nonsingular matrix".into()),
                };
            }
            Ok(result)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_round_trip_up_to_eight_bytes() {
        assert_eq!(unpack(pack("Observe")), "Observe");
        assert_eq!(unpack(pack("Predicted")), "Predicte");
        assert_eq!(unpack(pack("")), "");
    }

    #[test]
    fn default_formats() {
        assert_eq!(auto_format(&[1.0, -2.0, 30.0]).1, "F3.0");
        assert_eq!(auto_format(&[0.5, 100.0]).1, "F11.7");
        let (values, format, caption) = auto_format(&[1.357e-10, 2.468, 3.69e9]);
        assert_eq!(format, "F11.9");
        assert!((values[2] - 3.69).abs() < 1e-12);
        assert_eq!(
            caption.as_deref(),
            Some("The values are shown divided by 10**9")
        );
    }

    #[test]
    fn fields() {
        assert_eq!(split_field("  1  2, 3", 1, 9, None), ["1", "2", "3"]);
        assert_eq!(split_field("123456", 1, 6, Some(2)), ["12", "34", "56"]);
        assert_eq!(split_field("1234", 1, 6, Some(2)), ["12", "34"]);
    }
}
