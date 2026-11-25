//! SHIFT VALUES: new variables holding the values of other cases.

use crate::ast::Command;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

/// One `VARIABLE= RESULT= LEAD= | LAG= | SHIFT=` specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShiftSpec {
    pub variable: String,
    pub result: String,
    /// The case the value comes from, relative to the current one: LEAD=n is +n, LAG=n is -n and
    /// SHIFT=n is n as written.
    pub offset: i64,
}

fn integer(p: &mut Parser, signed: bool, what: &str) -> PResult<i64> {
    let start = p.pos;
    let negative = signed && p.eat(Punct::Minus);
    if !signed && p.at(Punct::Minus) {
        return Err(p.unexpected(&["a non-negative whole number"]));
    }
    match p.number() {
        Some(n) if n.fract() == 0.0 && n <= i64::MAX as f64 / 2.0 => {
            Ok(if negative { -(n as i64) } else { n as i64 })
        }
        Some(n) => Err(crate::error::SyntaxError::new(
            p.span_from(start),
            format!("{what} takes a whole number, got {n}"),
        )),
        None => Err(p.unexpected(&["a whole number"])),
    }
}

pub(crate) fn shift_values(p: &mut Parser) -> PResult<Command> {
    let mut specs = Vec::new();
    loop {
        p.eat(Punct::Slash);
        let start = p.pos;
        let (mut variable, mut result, mut offset) = (None, None, None);
        while matches!(p.peek(), Some(TokenKind::Word(_))) {
            if p.eat_keyword("VARIABLE") {
                p.expect(Punct::Eq)?;
                variable = Some(p.name("a variable name")?);
            } else if p.eat_keyword("RESULT") {
                p.expect(Punct::Eq)?;
                result = Some(p.name("a new variable name")?);
            } else if p.eat_keyword("LEAD") {
                p.expect(Punct::Eq)?;
                offset = Some(integer(p, false, "LEAD")?);
            } else if p.eat_keyword("LAG") {
                p.expect(Punct::Eq)?;
                offset = Some(-integer(p, false, "LAG")?);
            } else if p.eat_keyword("SHIFT") {
                p.expect(Punct::Eq)?;
                offset = Some(integer(p, true, "SHIFT")?);
            } else {
                return Err(p.unexpected(&["VARIABLE", "RESULT", "LEAD", "LAG", "SHIFT"]));
            }
        }
        let missing = |what: &str| {
            crate::error::SyntaxError::new(
                p.span_from(start),
                format!("each specification of SHIFT VALUES needs {what}"),
            )
        };
        specs.push(ShiftSpec {
            variable: variable.ok_or_else(|| missing("VARIABLE="))?,
            result: result.ok_or_else(|| missing("RESULT="))?,
            offset: offset.ok_or_else(|| missing("LEAD=, LAG= or SHIFT="))?,
        });
        if !p.at(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::ShiftValues(specs))
}
