//! MCONVERT: covariance matrix materials to correlation matrix materials and back.

use crate::ast::Command;
use crate::error::SyntaxError;
use crate::lexer::Punct;
use crate::matrix_io::{MatrixIo, matrix_io};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mconvert {
    pub matrix: MatrixIo,
    /// APPEND: the original matrix stays in the result, followed by the converted one.
    pub append: bool,
}

pub(crate) fn mconvert(p: &mut Parser) -> PResult<Command> {
    let mut command = Mconvert::default();
    let (mut replace, mut append) = (None, None);
    loop {
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        let start = p.pos;
        if p.eat_keyword("REPLACE") {
            replace = Some(start);
        } else if p.eat_keyword("APPEND") {
            append = Some(start);
        } else if p.at_keyword("MATRIX") || p.at_keyword("IN") || p.at_keyword("OUT") {
            // the word MATRIX is optional
            p.eat_keyword("MATRIX");
            matrix_io(p, &mut command.matrix, "MCONVERT", false, false)?;
        } else {
            return Err(p.unexpected(&["MATRIX", "IN", "OUT", "REPLACE", "APPEND"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    if let (Some(_), Some(at)) = (replace, append) {
        return Err(SyntaxError::new(
            p.span_from(at),
            "APPEND and REPLACE cannot be given on the same MCONVERT command",
        ));
    }
    command.append = append.is_some();
    if let (Some(Some(a)), Some(Some(b))) = (&command.matrix.input, &command.matrix.output)
        && a.eq_ignore_ascii_case(b)
    {
        return Err(SyntaxError::new(
            p.span_from(0),
            "IN and OUT of MCONVERT cannot name the same file",
        ));
    }
    Ok(Command::Mconvert(Box::new(command)))
}
