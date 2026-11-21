//! The MATRIX subcommand that reads and writes matrix data files, as REGRESSION, PARTIAL
//! CORR, RELIABILITY and ONEWAY have it: `IN(file | dataset | *)` and `OUT(file | dataset |
//! *)`, on one subcommand or on two, with `=` or without; RELIABILITY adds NOPRINT inside it
//! and ONEWAY accepts NONE.

use crate::cluster::matrix_source;
use crate::error::SyntaxError;
use crate::lexer::Punct;
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// What the MATRIX subcommands of one command asked for. A source is `None` for the active
/// dataset (`*`), else a file or dataset name.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MatrixIo {
    pub input: Option<Option<String>>,
    pub output: Option<Option<String>>,
    /// NOPRINT (RELIABILITY): the analysis is not displayed while a matrix is written.
    pub noprint: bool,
}

/// Reads the words of one MATRIX subcommand into `io`. `who` names the command in messages.
pub(crate) fn matrix_io(
    p: &mut Parser,
    io: &mut MatrixIo,
    who: &str,
    none: bool,
    noprint: bool,
) -> PResult<()> {
    p.eat(Punct::Eq);
    let mut any = false;
    while !at_end(p) {
        let start = p.pos;
        if p.eat_keyword("IN") {
            if io.input.is_some() {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!("{who} /MATRIX takes IN only once"),
                ));
            }
            io.input = Some(matrix_source(p)?);
        } else if p.eat_keyword("OUT") {
            if io.output.is_some() {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!("{who} /MATRIX takes OUT only once"),
                ));
            }
            io.output = Some(matrix_source(p)?);
        } else if none && p.eat_keyword("NONE") {
            // a matrix file is neither read nor written
            io.input = None;
            io.output = None;
        } else if noprint && p.eat_keyword("NOPRINT") {
            io.noprint = true;
        } else {
            let mut expected = vec!["IN", "OUT"];
            if none {
                expected.push("NONE");
            }
            if noprint {
                expected.push("NOPRINT");
            }
            return Err(p.unexpected(&expected));
        }
        any = true;
    }
    if !any {
        return Err(p.unexpected(&["IN", "OUT"]));
    }
    Ok(())
}
