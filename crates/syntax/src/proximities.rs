//! Parser of PROXIMITIES (similarities, dissimilarities and distances between cases or
//! variables).
//!
//! Rules from the Command Syntax Reference:
//! - The variable list comes first; with MATRIX=IN it may be left out, and then the command
//!   starts with a slash. The other subcommands come in any order.
//! - VIEW=CASE (default) or VARIABLE: the items are the cases or the variables.
//! - STANDARDIZE=[VARIABLE | CASE] [NONE | Z | SD | RANGE | MAX | MEAN | RESCALE]: by variable
//!   unless CASE; Z when the subcommand has no method, no standardizing without it.
//! - MEASURE takes at most one measure, EUCLID by default, and any of the transformations
//!   ABSOLUTE, REVERSE and RESCALE, which always apply in that order. NONE computes no measure:
//!   it only transforms a matrix that MATRIX=IN reads.
//! - PRINT=PROXIMITIES (default) or NONE. ID names a variable that labels the cases.
//!   MISSING=EXCLUDE (default) or INCLUDE.
//! - MATRIX=IN(file | dataset | *) and OUT(file | dataset | *), on one subcommand or two.

use crate::ast::{Command, VarItem};
use crate::cluster::{Proximity, matrix_source, measure_names, one_measure};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// How STANDARDIZE rescales the values of each variable or case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standardize {
    /// mean 0, standard deviation 1
    Z,
    /// standard deviation 1
    Sd,
    /// range 1
    Range,
    /// largest value 1
    Max,
    /// mean 1
    Mean,
    /// from 0 to 1
    Rescale,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Proximities {
    /// Empty with matrix input and no list.
    pub variables: Vec<VarItem>,
    /// `VIEW=VARIABLE`: proximities between variables instead of cases.
    pub view_variables: bool,
    /// The method and whether it works within each case (`CASE`) rather than each variable.
    pub standardize: Option<(Standardize, bool)>,
    /// The measure named on MEASURE; None when left out (EUCLID) or NONE.
    pub measure: Option<Proximity>,
    /// `MEASURE=NONE`
    pub no_measure: bool,
    pub absolute: bool,
    pub reverse: bool,
    pub rescale: bool,
    /// `PRINT=PROXIMITIES`
    pub print: bool,
    pub id: Option<String>,
    pub include: bool,
    /// `MATRIX=IN(...)` and `OUT(...)`: the file or dataset, None for `*`.
    pub matrix_in: Option<Option<String>>,
    pub matrix_out: Option<Option<String>>,
}

impl Proximities {
    /// The measure computed from the data.
    pub fn measure_or_default(&self) -> Proximity {
        self.measure.unwrap_or(Proximity::Euclid)
    }
}

const METHODS: [(&str, Standardize); 6] = [
    ("Z", Standardize::Z),
    ("SD", Standardize::Sd),
    ("RANGE", Standardize::Range),
    ("MAX", Standardize::Max),
    ("MEAN", Standardize::Mean),
    ("RESCALE", Standardize::Rescale),
];

fn standardize(p: &mut Parser) -> PResult<Option<(Standardize, bool)>> {
    p.eat(Punct::Eq);
    let (mut method, mut by_case, mut none) = (None, false, false);
    while !at_end(p) {
        if p.eat_keyword("VARIABLE") {
            by_case = false;
        } else if p.eat_keyword("CASE") {
            by_case = true;
        } else if p.eat_keyword("NONE") {
            (method, none) = (None, true);
        } else if let Some((_, m)) = METHODS.iter().find(|(k, _)| match k.len() {
            1..=2 => matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(k)),
            _ => p.at_keyword(k),
        }) {
            if method.is_some() {
                return Err(SyntaxError::new(p.span(), "STANDARDIZE takes one method"));
            }
            p.pos += 1;
            (method, none) = (Some(*m), false);
        } else {
            let mut expected = vec!["VARIABLE", "CASE", "NONE"];
            expected.extend(METHODS.iter().map(|(k, _)| *k));
            return Err(p.unexpected(&expected));
        }
        p.eat(Punct::Comma);
    }
    Ok(match none {
        true => None,
        false => Some((method.unwrap_or(Standardize::Z), by_case)),
    })
}

fn measure(p: &mut Parser, command: &mut Proximities) -> PResult<()> {
    p.eat(Punct::Eq);
    let mut named = false;
    while !at_end(p) {
        if p.eat_keyword("ABSOLUTE") {
            command.absolute = true;
        } else if p.eat_keyword("REVERSE") {
            command.reverse = true;
        } else if p.eat_keyword("RESCALE") {
            command.rescale = true;
        } else if named {
            return Err(SyntaxError::new(p.span(), "MEASURE takes one measure"));
        } else if p.eat_keyword("NONE") {
            (command.measure, command.no_measure, named) = (None, true, true);
        } else if let Some(m) = one_measure(p)? {
            (command.measure, command.no_measure, named) = (Some(m), false, true);
        } else {
            let mut expected = measure_names();
            expected.retain(|k| *k != "DEFAULT");
            expected.extend(["NONE", "ABSOLUTE", "REVERSE", "RESCALE"]);
            return Err(p.unexpected(&expected));
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

pub(crate) fn proximities(p: &mut Parser) -> PResult<Command> {
    let mut command = Proximities {
        variables: Vec::new(),
        view_variables: false,
        standardize: None,
        measure: None,
        no_measure: false,
        absolute: false,
        reverse: false,
        rescale: false,
        print: true,
        id: None,
        include: false,
        matrix_in: None,
        matrix_out: None,
    };
    if matches!(p.peek(), Some(TokenKind::Word(_))) {
        command.variables = p.var_list()?;
    } else if !p.at(Punct::Slash) {
        return Err(p.unexpected(&["a variable name", "'/'"]));
    }
    while p.eat(Punct::Slash) {
        if p.peek().is_none() {
            break;
        }
        if p.eat_keyword("VIEW") {
            p.eat(Punct::Eq);
            if p.eat_keyword("CASE") {
                command.view_variables = false;
            } else if p.eat_keyword("VARIABLE") {
                command.view_variables = true;
            } else {
                return Err(p.unexpected(&["CASE", "VARIABLE"]));
            }
        } else if p.eat_keyword("STANDARDIZE") {
            command.standardize = standardize(p)?;
        } else if p.eat_keyword("MEASURE") {
            measure(p, &mut command)?;
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            if p.eat_keyword("PROXIMITIES") {
                command.print = true;
            } else if p.eat_keyword("NONE") {
                command.print = false;
            } else if !at_end(p) {
                return Err(p.unexpected(&["PROXIMITIES", "NONE"]));
            }
        } else if p.eat_word("ID") {
            p.eat(Punct::Eq);
            command.id = Some(p.name("a variable name")?);
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("EXCLUDE") {
                    command.include = false;
                } else if p.eat_keyword("INCLUDE") {
                    command.include = true;
                } else {
                    return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("MATRIX") {
            p.eat(Punct::Eq);
            let mut any = false;
            while !at_end(p) {
                if p.eat_keyword("IN") {
                    command.matrix_in = Some(matrix_source(p)?);
                } else if p.eat_keyword("OUT") {
                    command.matrix_out = Some(matrix_source(p)?);
                } else {
                    return Err(p.unexpected(&["IN", "OUT"]));
                }
                any = true;
            }
            if !any {
                return Err(p.unexpected(&["IN", "OUT"]));
            }
        } else {
            return Err(p.subcommand_error("PROXIMITIES", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.variables.is_empty() && command.matrix_in.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "PROXIMITIES needs a variable list unless MATRIX=IN reads a proximity matrix",
        ));
    }
    if command.no_measure && command.matrix_in.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "MEASURE=NONE computes nothing: it only transforms a matrix that MATRIX=IN reads",
        ));
    }
    if command.standardize.is_some() && matches!(command.measure, Some(Proximity::Binary(..))) {
        return Err(SyntaxError::new(
            p.span(),
            "STANDARDIZE cannot be used with a binary measure",
        ));
    }
    Ok(Command::Proximities(Box::new(command)))
}
