//! GET DATA /TYPE=TXT.

use crate::ast::{Command, Format, FormatType, GetData, GetDataVariable};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

/// The characters of a DELIMITERS string: `\t` is a tab and `\\` a backslash.
fn delimiters(text: &str) -> Vec<char> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('\\', Some('t')) => {
                chars.next();
                out.push('\t');
            }
            ('\\', Some('\\')) => {
                chars.next();
                out.push('\\');
            }
            _ => out.push(c),
        }
    }
    out
}

/// A format of GET DATA: the width may be left out (width 0), AUTO is None, X skips.
fn data_format(word: &str) -> Result<(Option<Format>, bool), String> {
    if word.eq_ignore_ascii_case("AUTO") {
        return Ok((None, false));
    }
    if word.eq_ignore_ascii_case("X") {
        return Ok((None, true));
    }
    let digits = word.find(|c: char| !c.is_ascii_alphabetic());
    let (name, size) = word.split_at(digits.unwrap_or(word.len()));
    let kind = FormatType::from_name(name)
        .ok_or_else(|| format!("unknown format type in {}", word.to_uppercase()))?;
    if size.is_empty() {
        return Ok((
            Some(Format {
                kind,
                width: 0,
                decimals: 0,
            }),
            false,
        ));
    }
    crate::commands::format(word).map(|f| (Some(f), false))
}

fn whole(p: &mut Parser, what: &str) -> PResult<u32> {
    match p.number() {
        Some(n) if n >= 0.0 && n.fract() == 0.0 && n < 1e9 => Ok(n as u32),
        _ => Err(p.unexpected(&[what])),
    }
}

fn single_char(p: &mut Parser, what: &str) -> PResult<char> {
    let start = p.pos;
    let text = p.string(what)?;
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Ok(c),
        _ => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be one character"),
        )),
    }
}

pub(crate) fn get_data(p: &mut Parser) -> PResult<Command> {
    let mut command = GetData {
        file: String::new(),
        encoding: None,
        fixed: false,
        first_case: 1,
        values_per_case: None,
        lines_per_case: 1,
        delimiters: None,
        qualifier: None,
        first_cases: None,
        ignore_multiple_spaces: false,
        ignore_leading_spaces: false,
        variables: Vec::new(),
    };
    let mut kind = None;
    while p.eat(Punct::Slash) {
        let equals = |p: &mut Parser| p.eat(Punct::Eq);
        if p.eat_keyword("TYPE") {
            equals(p);
            let Some(TokenKind::Word(w)) = p.peek() else {
                return Err(p.unexpected(&["TXT"]));
            };
            kind = Some(w.to_uppercase());
            p.pos += 1;
            // the other types have subcommands of their own: stop at the type
            if kind.as_deref() != Some("TXT") {
                break;
            }
        } else if p.eat_keyword("FILE") {
            equals(p);
            command.file = p.file("a quoted file name or a file handle")?;
        } else if p.eat_keyword("ENCODING") {
            equals(p);
            command.encoding = Some(p.string("a quoted encoding")?);
        } else if p.eat_keyword("ARRANGEMENT") {
            equals(p);
            command.fixed = if p.eat_keyword("FIXED") {
                true
            } else if p.eat_keyword("DELIMITED") {
                false
            } else {
                return Err(p.unexpected(&["DELIMITED", "FIXED"]));
            };
        } else if p.eat_keyword("FIRSTCASE") {
            equals(p);
            command.first_case = whole(p, "a line number from 1")?.max(1);
        } else if p.eat_keyword("DELCASE") {
            equals(p);
            if p.eat_keyword("LINE") {
                command.values_per_case = None;
            } else if p.eat_keyword("VARIABLES") {
                command.values_per_case = Some(whole(p, "a number of variables")?.max(1));
            } else {
                return Err(p.unexpected(&["LINE", "VARIABLES"]));
            }
        } else if p.eat_keyword("FIXCASE") {
            equals(p);
            command.lines_per_case = whole(p, "a number of lines")?.max(1);
        } else if p.eat_keyword("DELIMITERS") {
            equals(p);
            command.delimiters = Some(delimiters(&p.string("quoted delimiters")?));
        } else if p.eat_keyword("QUALIFIER") {
            equals(p);
            command.qualifier = Some(single_char(p, "the qualifier")?);
        } else if p.eat_keyword("IMPORTCASE") {
            equals(p);
            if p.eat_keyword("FIRST") {
                command.first_cases = Some(u64::from(whole(p, "a number of cases")?));
            } else if !p.eat_word("ALL") {
                return Err(p.unexpected(&["ALL", "FIRST"]));
            }
        } else if p.eat_keyword("DATATYPEMIN") {
            if !p.eat_keyword("PERCENTAGE") {
                return Err(p.unexpected(&["PERCENTAGE"]));
            }
            equals(p);
            whole(p, "a percentage")?;
        } else if p.at_keyword("MULTIPLESPACES") || p.at_keyword("LEADINGSPACES") {
            let multiple = p.at_keyword("MULTIPLESPACES");
            p.pos += 1;
            if !p.eat_keyword("IGNORE") {
                return Err(p.unexpected(&["IGNORE"]));
            }
            equals(p);
            let yes = if p.eat_keyword("YES") {
                true
            } else if p.eat_keyword("NO") {
                false
            } else {
                return Err(p.unexpected(&["YES", "NO"]));
            };
            if multiple {
                command.ignore_multiple_spaces = yes;
            } else {
                command.ignore_leading_spaces = yes;
            }
        } else if p.eat_keyword("VARIABLES") {
            equals(p);
            variables(p, &mut command)?;
            break;
        } else if p.eat_keyword("MAP") {
            // MAP only lists the variables read: accepted, nothing to do
        } else {
            return Err(p.unexpected(&[
                "TYPE",
                "FILE",
                "ENCODING",
                "ARRANGEMENT",
                "FIRSTCASE",
                "DELCASE",
                "FIXCASE",
                "DELIMITERS",
                "QUALIFIER",
                "IMPORTCASE",
                "VARIABLES",
            ]));
        }
    }
    match kind.as_deref() {
        Some("TXT") => {}
        Some(other @ ("XLS" | "XLSX" | "XLSM" | "ODBC" | "OLEDB")) => {
            let span = p.span();
            return Err(SyntaxError::new(
                span,
                format!(
                    "GET DATA /TYPE={other} is not supported yet; save the data as text and read it with /TYPE=TXT"
                ),
            ));
        }
        Some(other) => {
            return Err(SyntaxError::new(
                p.span(),
                format!("unknown GET DATA type {other}"),
            ));
        }
        None => return Err(SyntaxError::new(p.span(), "GET DATA needs /TYPE=TXT")),
    }
    p.finish(&["'/'"])?;
    if command.file.is_empty() {
        return Err(SyntaxError::new(p.span(), "GET DATA needs /FILE='name'"));
    }
    if command.variables.is_empty() {
        return Err(SyntaxError::new(p.span(), "GET DATA needs /VARIABLES"));
    }
    Ok(Command::GetData(command))
}

/// `name format ...`, or for fixed data `[/n] name first-last format ...`.
fn variables(p: &mut Parser, command: &mut GetData) -> PResult<()> {
    let mut record = 1;
    loop {
        if command.fixed && p.eat(Punct::Slash) {
            record = whole(p, "a record number")?.max(1);
            continue;
        }
        let Some(TokenKind::Word(_)) = p.peek() else {
            break;
        };
        let name = p.name("a variable name")?;
        let columns = if command.fixed {
            let first = whole(p, "a first column")?;
            p.expect(Punct::Minus)?;
            let last = whole(p, "a last column")?;
            if last < first {
                return Err(p.unexpected(&["a last column after the first"]));
            }
            Some((first, last))
        } else {
            None
        };
        let Some(TokenKind::Word(word)) = p.peek() else {
            return Err(p.unexpected(&["a format such as F8.2, A20, AUTO or X"]));
        };
        let (format, skip) = data_format(word).map_err(|m| SyntaxError::new(p.span(), m))?;
        p.pos += 1;
        command.variables.push(GetDataVariable {
            name,
            record,
            columns,
            format,
            skip,
        });
    }
    Ok(())
}
