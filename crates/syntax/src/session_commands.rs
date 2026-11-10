//! Parsers of the commands about the session rather than the data: macros, inserted files,
//! settings, datasets and listings.

use crate::ast::Command;
use crate::lexer::Punct;
use crate::parser::{PResult, Parser};

/// The placeholder the macro processor leaves for a definition: `DEFINE 'NAME'`.
pub(crate) fn define(p: &mut Parser) -> PResult<Command> {
    let name = p.string("a macro name")?;
    p.finish(&[])?;
    Ok(Command::Define(name))
}

pub(crate) fn begin_program(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::BeginProgram)
}

/// `KEYWORD [=] value` where value is a word; the word in upper case.
fn keyword_value(p: &mut Parser, allowed: &[&str]) -> PResult<String> {
    p.eat(Punct::Eq);
    for word in allowed {
        if p.eat_keyword(word) {
            return Ok(word.to_string());
        }
    }
    Err(p.unexpected(allowed))
}

fn insert_or_include(p: &mut Parser, include: bool) -> PResult<Command> {
    // FILE may be left out before the path, as many scripts do with INCLUDE
    if p.eat_keyword("FILE") {
        p.eat(Punct::Eq);
    }
    let file = p.string("a quoted file path")?;
    loop {
        p.eat(Punct::Slash);
        if p.eat_keyword("PASSWORD") || p.eat_keyword("ENCODING") {
            p.eat(Punct::Eq);
            p.string("a quoted value")?;
        } else if !include && p.eat_keyword("SYNTAX") {
            keyword_value(p, &["INTERACTIVE", "BATCH"])?;
        } else if !include && p.eat_keyword("ERROR") {
            keyword_value(p, &["CONTINUE", "STOP"])?;
        } else if !include && p.eat_keyword("CD") {
            keyword_value(p, &["NO", "YES"])?;
        } else {
            break;
        }
    }
    let also: &[&str] = if include {
        &["PASSWORD", "ENCODING"]
    } else {
        &["PASSWORD", "SYNTAX", "ERROR", "CD", "ENCODING"]
    };
    p.finish(also)?;
    Ok(Command::Insert { file, include })
}

pub(crate) fn insert(p: &mut Parser) -> PResult<Command> {
    insert_or_include(p, false)
}

pub(crate) fn include(p: &mut Parser) -> PResult<Command> {
    insert_or_include(p, true)
}
