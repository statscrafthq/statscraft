//! EXPORT and IMPORT (portable files) and GET TRANSLATE (tab-delimited text, dBASE files and
//! old Excel workbooks).

use crate::ast::{Command, Export, FileOption, GetTranslate, Import, TranslateSource};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

/// `TYPE=COMM | TAPE`: how the file travels, which changes nothing in a file on disk.
fn transport(p: &mut Parser) -> PResult<()> {
    p.eat(Punct::Eq);
    if p.eat_keyword("COMM") || p.eat_keyword("TAPE") {
        Ok(())
    } else {
        Err(p.unexpected(&["COMM", "TAPE"]))
    }
}

/// KEEP, DROP, RENAME or MAP, with the parser after the slash.
fn variable_option(p: &mut Parser) -> PResult<Option<FileOption>> {
    if p.eat_keyword("MAP") {
        return Ok(Some(FileOption::Map));
    }
    crate::commands::file_option(p)
}

fn required(p: &Parser, start: usize, value: &str, message: &str) -> PResult<()> {
    if value.is_empty() {
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(())
}

pub(crate) fn export(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let mut command = Export {
        outfile: String::new(),
        options: Vec::new(),
        digits: None,
        delete_unselected: false,
    };
    loop {
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        if let Some(option) = variable_option(p)? {
            command.options.push(option);
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            command.outfile = p.file("a quoted file name or a file handle")?;
        } else if p.eat_keyword("TYPE") {
            transport(p)?;
        } else if p.eat_keyword("UNSELECTED") {
            p.eat(Punct::Eq);
            if p.eat_keyword("DELETE") {
                command.delete_unselected = true;
            } else {
                p.eat_keyword("RETAIN");
            }
        } else if p.eat_keyword("DIGITS") {
            p.eat(Punct::Eq);
            command.digits = match p.number() {
                Some(n) if n.fract() == 0.0 && (1.0..=16.0).contains(&n) => Some(n as u32),
                _ => return Err(p.unexpected(&["a number of digits from 1 to 16"])),
            };
        } else {
            return Err(p.subcommand_error("EXPORT", &[]));
        }
    }
    required(p, start, &command.outfile, "EXPORT needs OUTFILE='name'")?;
    Ok(Command::Export(command))
}

pub(crate) fn import(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let mut command = Import {
        file: String::new(),
        options: Vec::new(),
    };
    loop {
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        if let Some(option) = variable_option(p)? {
            command.options.push(option);
        } else if p.eat_keyword("FILE") {
            p.eat(Punct::Eq);
            command.file = p.file("a quoted file name or a file handle")?;
        } else if p.eat_keyword("TYPE") {
            transport(p)?;
        } else {
            return Err(p.subcommand_error("IMPORT", &[]));
        }
    }
    required(p, start, &command.file, "IMPORT needs FILE='name'")?;
    Ok(Command::Import(command))
}

/// The file types of the manual that Statscraft does not read.
const OTHER_TYPES: [&str; 7] = ["WK", "WK1", "WKS", "WR1", "WRK", "SYM", "SLK"];

fn source(name: &str) -> Option<TranslateSource> {
    match name {
        "TAB" => Some(TranslateSource::Tab),
        "DBF" => Some(TranslateSource::Dbf),
        "XLS" => Some(TranslateSource::Xls),
        _ => None,
    }
}

pub(crate) fn get_translate(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let mut command = GetTranslate {
        file: String::new(),
        source: TranslateSource::Tab,
        field_names: false,
        range: None,
        options: Vec::new(),
    };
    let mut kind = None;
    loop {
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        if let Some(option) = variable_option(p)? {
            command.options.push(option);
        } else if p.eat_keyword("FILE") {
            p.eat(Punct::Eq);
            command.file = p.file("a quoted file name or a file handle")?;
        } else if p.eat_keyword("TYPE") {
            p.eat(Punct::Eq);
            let Some(TokenKind::Word(word)) = p.peek() else {
                return Err(p.unexpected(&["TAB", "DBF", "XLS"]));
            };
            kind = Some((word.to_uppercase(), p.span()));
            p.pos += 1;
        } else if p.eat_keyword("FIELDNAMES") {
            command.field_names = true;
        } else if p.eat_keyword("RANGE") {
            p.eat(Punct::Eq);
            let at = p.pos;
            // the 1-2-3 form D3..J279 is one word; the Excel form needs quotes for its colon
            let text = match p.peek() {
                Some(TokenKind::Word(word)) => {
                    let word = word.clone();
                    p.pos += 1;
                    word
                }
                _ => p.string("a range such as A1..K14 or 'A1:K14'")?,
            };
            let range = crate::get_data::cell_range(&text.replace("..", ":"));
            command.range = Some(range.ok_or_else(|| {
                let message = format!(
                    "'{text}' is not a range of cells such as A1..K14 or 'A1:K14'; \
                     range names are not supported"
                );
                SyntaxError::new(p.span_from(at), message)
            })?);
        } else {
            return Err(p.subcommand_error("GET TRANSLATE", &[]));
        }
    }
    let span = p.span_from(start);
    required(p, start, &command.file, "GET TRANSLATE needs FILE='name'")?;
    let extension = std::path::Path::new(&command.file)
        .extension()
        .map(|e| e.to_string_lossy().to_uppercase());
    let (name, span) = match kind {
        Some(kind) => kind,
        None => match extension {
            Some(e) if source(&e).is_some() || OTHER_TYPES.contains(&e.as_str()) => (e, span),
            _ => {
                return Err(SyntaxError::new(
                    span,
                    "GET TRANSLATE needs /TYPE=TAB, DBF or XLS for a file with this extension",
                ));
            }
        },
    };
    command.source = match source(&name) {
        Some(source) => source,
        None if OTHER_TYPES.contains(&name.as_str()) => {
            return Err(SyntaxError::new(
                span,
                format!(
                    "GET TRANSLATE /TYPE={name} is not supported; Statscraft reads TYPE=TAB, \
                     DBF and XLS (GET DATA reads newer workbooks and other text files)"
                ),
            ));
        }
        None => {
            return Err(SyntaxError::new(
                span,
                format!("unknown GET TRANSLATE type {name}"),
            ));
        }
    };
    if command.source == TranslateSource::Dbf && command.range.is_some() {
        return Err(SyntaxError::new(
            span,
            "RANGE applies to spreadsheets and tab-delimited files, not to dBASE files",
        ));
    }
    Ok(Command::GetTranslate(command))
}
