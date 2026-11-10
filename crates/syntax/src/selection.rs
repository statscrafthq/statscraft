//! Parsers of the commands that choose which cases procedures see and how: SPLIT FILE,
//! FILTER, TEMPORARY, N OF CASES and SAMPLE.

use crate::ast::{Command, Sample, SplitFile};
use crate::error::SyntaxError;
use crate::lexer::TokenKind;
use crate::parser::{PResult, Parser};

pub(crate) fn split_file(p: &mut Parser) -> PResult<Command> {
    if p.eat_word("OFF") {
        p.finish(&[])?;
        return Ok(Command::SplitFile(None));
    }
    let separate = if p.eat_keyword("SEPARATE") {
        true
    } else {
        p.eat_keyword("LAYERED");
        false
    };
    if !p.eat_word("BY") {
        return Err(p.unexpected(&["BY", "LAYERED", "SEPARATE", "OFF"]));
    }
    let variables = p.var_list()?;
    p.finish(&["a variable name"])?;
    Ok(Command::SplitFile(Some(SplitFile {
        variables,
        separate,
    })))
}

pub(crate) fn filter(p: &mut Parser) -> PResult<Command> {
    let variable = if p.eat_word("BY") {
        Some(p.name("a variable name")?)
    } else if p.eat_word("OFF") || p.peek().is_none() {
        None
    } else {
        return Err(p.unexpected(&["BY", "OFF"]));
    };
    let bare = variable.is_none() && p.pos == 1;
    p.finish(&[])?;
    Ok(Command::Filter { variable, bare })
}

pub(crate) fn temporary(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::Temporary)
}

/// A whole number of at least 1.
fn whole(p: &mut Parser, what: &str) -> PResult<u64> {
    match p.peek() {
        Some(TokenKind::Number(n)) if n.fract() == 0.0 && *n >= 1.0 && *n < 1e15 => {
            let n = *n as u64;
            p.pos += 1;
            Ok(n)
        }
        _ => Err(p.unexpected(&[what])),
    }
}

pub(crate) fn n_of_cases(p: &mut Parser) -> PResult<Command> {
    let n = whole(p, "the number of cases")?;
    // ESTIMATED is a hint about the file size from older versions and changes nothing
    p.eat_keyword("ESTIMATED");
    p.finish(&[])?;
    Ok(Command::NOfCases(n))
}

pub(crate) fn sample(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let Some(TokenKind::Number(value)) = p.peek().cloned() else {
        return Err(p.unexpected(&["a proportion such as .25", "n FROM m"]));
    };
    let sample = if p
        .peek_at(1)
        .is_some_and(|t| matches!(t, TokenKind::Word(w) if w.eq_ignore_ascii_case("FROM")))
    {
        let n = whole(p, "the size of the sample")?;
        p.pos += 1;
        let from = whole(p, "the number of cases to sample from")?;
        Sample::Exactly { n, from }
    } else {
        p.pos += 1;
        if !(value > 0.0 && value < 1.0) {
            return Err(SyntaxError::new(
                p.span_from(start),
                "the proportion of SAMPLE must lie between 0 and 1",
            )
            .hint("for an exact number of cases write SAMPLE n FROM m"));
        }
        Sample::Fraction(value)
    };
    p.finish(&["FROM"])?;
    Ok(Command::Sample(sample))
}
