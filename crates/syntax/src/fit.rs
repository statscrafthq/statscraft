//! FIT: goodness-of-fit statistics of residual series.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// The degrees of freedom of the residual series, one value for each.
#[derive(Debug, Clone, PartialEq)]
pub enum FitFreedom {
    /// DFE: the error degrees of freedom.
    Error(Vec<f64>),
    /// DFH: the hypothesis degrees of freedom, which the number of cases is reduced by.
    Hypothesis(Vec<f64>),
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Fit {
    /// The residual series; empty for the last one a procedure saved.
    pub errors: Vec<VarItem>,
    pub observed: Vec<VarItem>,
    pub freedom: Option<FitFreedom>,
}

fn numbers(p: &mut Parser) -> PResult<Vec<f64>> {
    p.eat(Punct::Eq);
    let mut values = Vec::new();
    while let Some(TokenKind::Number(_) | TokenKind::Punct(Punct::Minus)) = p.peek() {
        let negative = p.eat(Punct::Minus);
        let Some(n) = p.number() else {
            return Err(p.unexpected(&["a number"]));
        };
        values.push(if negative { -n } else { n });
        p.eat(Punct::Comma);
    }
    if values.is_empty() {
        return Err(p.unexpected(&["a number"]));
    }
    Ok(values)
}

pub(crate) fn fit(p: &mut Parser) -> PResult<Command> {
    let mut command = Fit::default();
    let mut first = true;
    loop {
        let slash = p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        let start = p.pos;
        if p.eat_keyword("OBS") {
            p.eat(Punct::Eq);
            command.observed = list_until(p, &[])?;
        } else if p.eat_keyword("DFE") {
            command.freedom = Some(FitFreedom::Error(numbers(p)?));
        } else if p.eat_keyword("DFH") {
            command.freedom = Some(FitFreedom::Hypothesis(numbers(p)?));
        } else if p.at_keyword("ERRORS") || p.at_keyword("VARIABLES") || (first && !slash) {
            if p.eat_keyword("ERRORS") || p.eat_keyword("VARIABLES") {
                p.eat(Punct::Eq);
            }
            command.errors = list_until(p, &[])?;
        } else {
            return Err(p.unexpected(&["ERRORS", "OBS", "DFE", "DFH"]));
        }
        first = false;
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
        let _ = start;
    }
    if !command.observed.is_empty() && command.errors.is_empty() {
        return Err(SyntaxError::new(
            p.span_from(0),
            "OBS needs the residual series named with ERRORS",
        ));
    }
    Ok(Command::Fit(Box::new(command)))
}
