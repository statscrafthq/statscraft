//! Parser of MATRIX DATA: raw matrix materials read into a matrix data file.
//!
//! Rules from the command's description:
//! - VARIABLES is required and names the fields of the raw data in their order; `v1 TO v3`
//!   makes new names. ROWTYPE_ may be one of them (the record types are then in the data);
//!   VARNAME_ never is. The continuous variables come last.
//! - SPLIT and FACTORS follow VARIABLES and name variables of it (TO in the order of
//!   VARIABLES). Without ROWTYPE_ and split variables in the data, SPLIT may name one new
//!   variable, numbered 1, 2, ... for each set of materials. At most eight split variables.
//! - FILE=INLINE (the default) or a file. FORMAT takes LIST or FREE, LOWER, UPPER or FULL,
//!   DIAGONAL or NODIAGONAL; FULL cannot go with NODIAGONAL. CELLS=n and N=n are whole numbers.
//! - CONTENTS lists the record types in the order of the data; types in parentheses are
//!   within-cells records, read for each cell. Without ROWTYPE_, factors need CELLS and a
//!   within-cells group.

use crate::ast::{Command, VarItem};
use crate::data_list::numbered_names;
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// A record type of matrix materials.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowType {
    Corr,
    Cov,
    Mat,
    Mse,
    Dfe,
    Mean,
    StdDev,
    NVector,
    NScalar,
    NMatrix,
    Count,
    Prox,
}

/// The names of the record types, with their synonyms.
const ROW_TYPES: [(&str, RowType); 14] = [
    ("CORR", RowType::Corr),
    ("COV", RowType::Cov),
    ("MAT", RowType::Mat),
    ("MSE", RowType::Mse),
    ("DFE", RowType::Dfe),
    ("MEAN", RowType::Mean),
    ("STDDEV", RowType::StdDev),
    ("SD", RowType::StdDev),
    ("N_VECTOR", RowType::NVector),
    ("N", RowType::NVector),
    ("N_SCALAR", RowType::NScalar),
    ("N_MATRIX", RowType::NMatrix),
    ("COUNT", RowType::Count),
    ("PROX", RowType::Prox),
];

impl RowType {
    /// A record type by name, synonym or an abbreviation of at least three characters, as
    /// ROWTYPE_ values and CONTENTS may write it.
    pub fn from_name(text: &str) -> Option<RowType> {
        let text = text.trim();
        if let Some((_, kind)) = ROW_TYPES
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(text))
        {
            return Some(*kind);
        }
        let mut found = ROW_TYPES.iter().filter(|(name, _)| {
            text.len() >= 3
                && name.len() > text.len()
                && name[..text.len()].eq_ignore_ascii_case(text)
        });
        match (found.next(), found.next()) {
            (Some((_, kind)), None) => Some(*kind),
            _ => None,
        }
    }

    /// The ROWTYPE_ value the matrix file holds.
    pub fn value(self) -> &'static str {
        match self {
            RowType::Corr => "CORR",
            RowType::Cov => "COV",
            RowType::Mat => "MAT",
            RowType::Mse => "MSE",
            RowType::Dfe => "DFE",
            RowType::Mean => "MEAN",
            RowType::StdDev => "STDDEV",
            RowType::NVector | RowType::NScalar | RowType::NMatrix => "N",
            RowType::Count => "COUNT",
            RowType::Prox => "PROX",
        }
    }

    /// True for the square matrices, which have one record per continuous variable.
    pub fn is_matrix(self) -> bool {
        matches!(
            self,
            RowType::Corr | RowType::Cov | RowType::Mat | RowType::NMatrix | RowType::Prox
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixShape {
    Lower,
    Upper,
    Full,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatrixData {
    /// The fields of the raw data in order, ROWTYPE_ included when the data hold it.
    pub variables: Vec<String>,
    /// None: inline data between BEGIN DATA and END DATA.
    pub file: Option<String>,
    /// FORMAT=FREE: a row need not start a new record.
    pub free: bool,
    pub shape: MatrixShape,
    pub diagonal: bool,
    /// In the order of SPLIT. A name not on VARIABLES is a new variable numbered 1, 2, ...
    pub split: Vec<String>,
    /// In the order of FACTORS.
    pub factors: Vec<String>,
    pub cells: Option<u32>,
    /// The record types in data order; true for a within-cells group (in parentheses).
    pub contents: Vec<(bool, Vec<RowType>)>,
    pub n: Option<u32>,
}

impl MatrixData {
    /// True when ROWTYPE_ is one of the fields of the data.
    pub fn explicit(&self) -> bool {
        self.variables
            .iter()
            .any(|v| v.eq_ignore_ascii_case("ROWTYPE_"))
    }

    /// The variables the statistics are about: neither ROWTYPE_ nor a split or factor.
    pub fn continuous(&self) -> Vec<&str> {
        let named = |list: &[String], v: &str| list.iter().any(|n| n.eq_ignore_ascii_case(v));
        self.variables
            .iter()
            .filter(|v| {
                !v.eq_ignore_ascii_case("ROWTYPE_")
                    && !named(&self.split, v)
                    && !named(&self.factors, v)
            })
            .map(String::as_str)
            .collect()
    }
}

/// A variable list of new names; `a TO b` makes the numbered names between.
fn new_names(p: &mut Parser) -> PResult<Vec<String>> {
    let mut names = Vec::new();
    while matches!(p.peek(), Some(TokenKind::Word(_))) {
        let start = p.pos;
        let first = p.name("a variable name")?;
        if p.eat_word("TO") {
            let last = p.name("a variable name")?;
            names.extend(
                numbered_names(&first, &last)
                    .map_err(|e| SyntaxError::new(p.span_from(start), e))?,
            );
        } else {
            names.push(first);
        }
        p.eat(Punct::Comma);
    }
    if names.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(names)
}

/// A list that names variables of VARIABLES; TO runs in the order of VARIABLES. With `new`,
/// one name that is not on VARIABLES is allowed (the split variable MATRIX DATA numbers).
fn listed(p: &mut Parser, variables: &[String], what: &str, new: bool) -> PResult<Vec<String>> {
    let start = p.pos;
    let items = p.var_list()?;
    let index = |name: &str| variables.iter().position(|v| v.eq_ignore_ascii_case(name));
    let unknown = |name: &str| {
        SyntaxError::new(
            p.span_from(start),
            format!("{what} names {name}, which is not on VARIABLES"),
        )
    };
    let mut out = Vec::new();
    for item in &items {
        match item {
            VarItem::Name(name) => match index(name) {
                Some(i) => out.push(variables[i].clone()),
                None if new && items.len() == 1 => out.push(name.clone()),
                None => return Err(unknown(name)),
            },
            VarItem::Range(a, b) => {
                let (Some(i), Some(j)) = (index(a), index(b)) else {
                    return Err(unknown(if index(a).is_none() { a } else { b }));
                };
                if i > j {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        format!("{a} comes after {b} on VARIABLES"),
                    ));
                }
                out.extend(variables[i..=j].iter().cloned());
            }
            VarItem::All => out.extend(variables.iter().cloned()),
        }
    }
    if out.iter().any(|v| v.eq_ignore_ascii_case("ROWTYPE_")) {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} cannot name ROWTYPE_"),
        ));
    }
    Ok(out)
}

fn whole(p: &mut Parser, what: &str) -> PResult<u32> {
    p.eat(Punct::Eq);
    crate::data_list::count(p, what)
}

fn row_type(p: &mut Parser) -> PResult<RowType> {
    let names: Vec<&str> = ROW_TYPES.iter().map(|(n, _)| *n).collect();
    match p.peek() {
        Some(TokenKind::Word(w)) => match RowType::from_name(w) {
            Some(kind) => {
                p.pos += 1;
                Ok(kind)
            }
            None => Err(p.unexpected(&names)),
        },
        _ => Err(p.unexpected(&names)),
    }
}

pub(crate) fn matrix_data(p: &mut Parser) -> PResult<Command> {
    let mut command = MatrixData {
        variables: Vec::new(),
        file: None,
        free: false,
        shape: MatrixShape::Lower,
        diagonal: true,
        split: Vec::new(),
        factors: Vec::new(),
        cells: None,
        contents: Vec::new(),
        n: None,
    };
    let start = p.pos;
    let mut first = true;
    let mut no_diagonal = None;
    loop {
        if first {
            p.eat(Punct::Slash);
        } else if !p.eat(Punct::Slash) {
            break;
        }
        first = false;
        if p.peek().is_none() {
            break;
        }
        let at = p.pos;
        if p.eat_keyword("VARIABLES") {
            p.eat(Punct::Eq);
            command.variables = new_names(p)?;
            if let Some(v) = command
                .variables
                .iter()
                .find(|v| v.eq_ignore_ascii_case("VARNAME_"))
            {
                return Err(SyntaxError::new(
                    p.span_from(at),
                    format!("{v} cannot be named: MATRIX DATA makes VARNAME_ itself"),
                ));
            }
        } else if p.eat_keyword("FILE") {
            p.eat(Punct::Eq);
            command.file = if p.eat_keyword("INLINE") {
                None
            } else {
                Some(p.file("a file")?)
            };
        } else if p.eat_keyword("FORMAT") {
            p.eat(Punct::Eq);
            let mut any = false;
            while !at_end(p) {
                if p.eat_keyword("LIST") {
                    command.free = false;
                } else if p.eat_keyword("FREE") {
                    command.free = true;
                } else if p.eat_keyword("LOWER") {
                    command.shape = MatrixShape::Lower;
                } else if p.eat_keyword("UPPER") {
                    command.shape = MatrixShape::Upper;
                } else if p.eat_keyword("FULL") {
                    command.shape = MatrixShape::Full;
                } else if p.eat_keyword("DIAGONAL") {
                    command.diagonal = true;
                } else if p.eat_keyword("NODIAGONAL") {
                    command.diagonal = false;
                    no_diagonal = Some(p.span_from(p.pos - 1));
                } else {
                    return Err(p.unexpected(&[
                        "LIST",
                        "FREE",
                        "LOWER",
                        "UPPER",
                        "FULL",
                        "DIAGONAL",
                        "NODIAGONAL",
                    ]));
                }
                any = true;
                p.eat(Punct::Comma);
            }
            if !any {
                return Err(p.unexpected(&["LIST", "FREE", "LOWER", "UPPER", "FULL"]));
            }
        } else if p.at_keyword("SPLIT") || p.at_keyword("FACTORS") {
            let split = p.eat_keyword("SPLIT");
            p.eat_keyword("FACTORS");
            if command.variables.is_empty() {
                return Err(SyntaxError::new(
                    p.span_from(at),
                    format!(
                        "{} must follow VARIABLES",
                        if split { "SPLIT" } else { "FACTORS" }
                    ),
                ));
            }
            p.eat(Punct::Eq);
            if split {
                command.split = listed(p, &command.variables, "SPLIT", true)?;
                if command.split.len() > 8 {
                    return Err(SyntaxError::new(
                        p.span_from(at),
                        "SPLIT takes at most eight variables",
                    ));
                }
            } else {
                command.factors = listed(p, &command.variables, "FACTORS", false)?;
            }
        } else if p.eat_keyword("CELLS") {
            command.cells = Some(whole(p, "a number of cells")?);
        } else if p.eat_word("N") {
            command.n = Some(whole(p, "a number of cases")?);
        } else if p.eat_keyword("CONTENTS") {
            p.eat(Punct::Eq);
            command.contents.clear();
            while !at_end(p) {
                if p.eat(Punct::LParen) {
                    let mut kinds = vec![row_type(p)?];
                    while !p.eat(Punct::RParen) {
                        p.eat(Punct::Comma);
                        kinds.push(row_type(p)?);
                    }
                    command.contents.push((true, kinds));
                } else {
                    command.contents.push((false, vec![row_type(p)?]));
                }
                p.eat(Punct::Comma);
            }
            if command.contents.is_empty() {
                return Err(p.unexpected(&["a record type such as CORR"]));
            }
        } else {
            return Err(p.subcommand_error("MATRIX DATA", &[]));
        }
    }
    p.finish(&["'/'"])?;
    let whole_span = p.span_from(start);
    let fail = |message: String| Err(SyntaxError::new(whole_span, message));
    if command.variables.is_empty() {
        return fail("MATRIX DATA needs VARIABLES".into());
    }
    if let (MatrixShape::Full, Some(span)) = (command.shape, no_diagonal) {
        return Err(SyntaxError::new(
            span,
            "FORMAT=FULL cannot go with NODIAGONAL",
        ));
    }
    let continuous = command.continuous();
    if continuous.is_empty() {
        return fail("VARIABLES names no continuous variables".into());
    }
    let first_continuous = command
        .variables
        .iter()
        .position(|v| continuous.contains(&v.as_str()))
        .expect("there is one");
    if let Some(late) = command.variables[first_continuous..]
        .iter()
        .find(|v| !continuous.contains(&v.as_str()))
    {
        return fail(format!(
            "{late} must come before the continuous variables on VARIABLES"
        ));
    }
    let new_split = command
        .split
        .iter()
        .any(|s| !command.variables.iter().any(|v| v.eq_ignore_ascii_case(s)));
    if new_split && command.explicit() {
        return fail(format!(
            "SPLIT names {}, which is not on VARIABLES; with ROWTYPE_ in the data the split \
             variables are in the data too",
            command.split[0]
        ));
    }
    if !command.explicit() && !command.factors.is_empty() {
        if command.cells.is_none() {
            return fail("without ROWTYPE_, FACTORS needs CELLS: the number of cells".into());
        }
        if !command.contents.iter().any(|(within, _)| *within) {
            return fail(
                "without ROWTYPE_, FACTORS needs CONTENTS with the within-cells record types in \
                 parentheses, such as CONTENTS=(MEAN SD N) CORR"
                    .into(),
            );
        }
    }
    Ok(Command::MatrixData(Box::new(command)))
}
