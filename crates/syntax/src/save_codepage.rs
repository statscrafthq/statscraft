//! SAVE CODEPAGE: the active dataset as a data file whose text is in a code page of the
//! caller's choice.

use crate::ast::{Command, FileOption, SaveCodepage};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

pub(crate) fn save_codepage(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let mut command = SaveCodepage {
        outfile: String::new(),
        encoding: None,
        delete_unselected: false,
        options: Vec::new(),
        password: false,
    };
    let mut given = [false; 3];
    loop {
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        let at = p.span();
        let mut once = |p: &mut Parser, kind: usize, name: &str| -> PResult<()> {
            match std::mem::replace(&mut given[kind], true) {
                true => Err(SyntaxError::new(
                    at,
                    format!("{name} of SAVE CODEPAGE is given only once"),
                )),
                false => {
                    let _ = p;
                    Ok(())
                }
            }
        };
        if p.at_keyword("KEEP") || p.at_keyword("DROP") {
            let Some(option) = crate::commands::file_option(p)? else {
                return Err(p.subcommand_error("SAVE CODEPAGE", &[]));
            };
            match option {
                FileOption::Keep(_) | FileOption::Drop(_) => command.options.push(option),
                _ => return Err(p.subcommand_error("SAVE CODEPAGE", &[])),
            }
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            command.outfile = p.file("a quoted file name or a file handle")?;
        } else if p.eat_keyword("ENCODING") {
            once(p, 0, "ENCODING")?;
            p.eat(Punct::Eq);
            command.encoding = if p.eat_keyword("LOCALE") {
                None
            } else {
                Some(p.string("LOCALE or a quoted encoding")?)
            };
        } else if p.eat_keyword("UNSELECTED") {
            once(p, 1, "UNSELECTED")?;
            p.eat(Punct::Eq);
            if p.eat_keyword("DELETE") {
                command.delete_unselected = true;
            } else {
                p.eat_keyword("RETAIN");
            }
        } else if p.eat_keyword("PASSPROTECT") {
            once(p, 2, "PASSPROTECT")?;
            if !p.eat_keyword("PASSWORD") {
                return Err(p.unexpected(&["PASSWORD"]));
            }
            p.eat(Punct::Eq);
            if matches!(p.peek(), Some(TokenKind::Str(_))) {
                p.string("a quoted password")?;
                command.password = true;
            } else if !p.eat_keyword("NONE") {
                return Err(p.unexpected(&["a quoted password", "NONE"]));
            }
            if p.eat_keyword("ENCRYPTEDPW") {
                p.eat(Punct::Eq);
                if !p.eat_keyword("NO") && !p.eat_keyword("YES") {
                    return Err(p.unexpected(&["NO", "YES"]));
                }
            }
        } else {
            return Err(p.subcommand_error("SAVE CODEPAGE", &[]));
        }
    }
    if command.outfile.is_empty() {
        return Err(SyntaxError::new(
            p.span_from(start),
            "SAVE CODEPAGE needs OUTFILE='name'",
        ));
    }
    Ok(Command::SaveCodepage(command))
}
