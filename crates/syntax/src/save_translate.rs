//! SAVE TRANSLATE: the active dataset as a CSV or tab-delimited text file, or an Excel 2007
//! workbook.

use crate::ast::{Command, SaveTranslate, TranslateType};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

/// Subcommands of the manual that are refused as not supported yet.
const NOT_YET: [&str; 14] = [
    "APPEND",
    "CONNECT",
    "ENCRYPTED",
    "UNENCRYPTED",
    "TABLE",
    "SQL",
    "BULKLOADING",
    "ODBCOPTIONS",
    "EDITION",
    "PLATFORM",
    "VALFILE",
    "COMPRESSED",
    "UNCOMPRESSED",
    "MAP",
];

/// One of `choices` as its index, or an error that lists them.
fn choice(p: &mut Parser, choices: &[&str]) -> PResult<usize> {
    p.eat(Punct::Eq);
    match choices.iter().position(|c| p.at_keyword(c)) {
        Some(i) => {
            p.pos += 1;
            Ok(i)
        }
        None => Err(p.unexpected(choices)),
    }
}

/// A quoted single character of TEXTOPTIONS.
fn character(p: &mut Parser, what: &str) -> PResult<char> {
    p.eat(Punct::Eq);
    let start = p.pos;
    let text = p.string(what)?;
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Ok(c),
        _ => Err(SyntaxError::new(
            p.span_from(start),
            format!("the {what} must be one character"),
        )),
    }
}

pub(crate) fn save_translate(p: &mut Parser) -> PResult<Command> {
    let mut command = SaveTranslate {
        outfile: String::new(),
        kind: TranslateType::Csv,
        field_names: None,
        labels: false,
        delimiter: None,
        qualifier: '"',
        decimal_comma: None,
        print_formats: false,
        sheet: None,
        replace: false,
        options: Vec::new(),
        recode_missing: false,
        delete_unselected: false,
        encoding: None,
        bom: true,
    };
    let mut kind = None;
    let mut version = None;
    let start = p.pos;
    loop {
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        if let Some(option) = crate::commands::file_option(p)? {
            command.options.push(option);
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            command.outfile = p.file("a quoted file name or a file handle")?;
        } else if p.eat_keyword("TYPE") {
            p.eat(Punct::Eq);
            let Some(TokenKind::Word(w)) = p.peek() else {
                return Err(p.unexpected(&["CSV", "TAB", "XLS"]));
            };
            kind = Some((w.to_uppercase(), p.span()));
            p.pos += 1;
        } else if p.eat_keyword("VERSION") {
            p.eat(Punct::Eq);
            version = match p.peek() {
                Some(TokenKind::Number(n)) => Some(n.to_string()),
                Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("X") => Some("X".into()),
                _ => return Err(p.unexpected(&["a version number such as 12"])),
            };
            p.pos += 1;
        } else if p.eat_keyword("FIELDNAMES") {
            command.field_names = Some(false);
            if p.eat_keyword("VALUE") {
                command.field_names = Some(choice(p, &["NAMES", "LABELS"])? == 1);
            }
        } else if p.eat_keyword("CELLS") {
            command.labels = choice(p, &["VALUES", "LABELS"])? == 1;
        } else if p.eat_keyword("TEXTOPTIONS") {
            loop {
                if p.eat_keyword("DELIMITER") {
                    command.delimiter = Some(character(p, "delimiter")?);
                } else if p.eat_keyword("QUALIFIER") {
                    command.qualifier = character(p, "qualifier")?;
                } else if p.eat_keyword("DECIMAL") {
                    command.decimal_comma = Some(choice(p, &["DOT", "COMMA"])? == 1);
                } else if p.eat_keyword("FORMAT") {
                    command.print_formats = choice(p, &["PLAIN", "VARIABLE"])? == 1;
                } else {
                    break;
                }
            }
        } else if p.eat_keyword("EXCELOPTIONS") {
            if !p.eat_keyword("SHEET") {
                return Err(p.unexpected(&["SHEET"]));
            }
            p.eat(Punct::Eq);
            let at = p.pos;
            let name = p.string("a quoted sheet name")?;
            let bad = name.chars().count() > 31
                || name.trim().is_empty()
                || name.contains(['/', '\\', '?', '[', ']', '*', ':']);
            if bad {
                return Err(SyntaxError::new(
                    p.span_from(at),
                    "a sheet name has 1 to 31 characters and none of / \\ ? [ ] * :",
                ));
            }
            command.sheet = Some(name);
        } else if p.eat_keyword("REPLACE") {
            command.replace = true;
        } else if p.eat_keyword("UNSELECTED") {
            p.eat(Punct::Eq);
            if p.eat_keyword("DELETE") {
                command.delete_unselected = true;
            } else {
                p.eat_keyword("RETAIN");
            }
        } else if p.eat_keyword("MISSING") {
            command.recode_missing = choice(p, &["IGNORE", "RECODE"])? == 1;
        } else if p.eat_keyword("ENCODING") {
            p.eat(Punct::Eq);
            command.encoding = Some(p.string("a quoted encoding")?);
            if p.eat_keyword("BOM") {
                command.bom = choice(p, &["YES", "NO"])? == 0;
            }
        } else {
            return Err(p.subcommand_error("SAVE TRANSLATE", &NOT_YET));
        }
    }
    let span = p.span_from(start);
    if command.outfile.is_empty() {
        return Err(SyntaxError::new(
            span,
            "SAVE TRANSLATE needs OUTFILE='name'",
        ));
    }
    let extension = std::path::Path::new(&command.outfile)
        .extension()
        .map(|e| e.to_string_lossy().to_uppercase());
    let (name, span) = match kind {
        Some(kind) => kind,
        None => match extension.as_deref() {
            Some(e @ ("CSV" | "XLSX" | "XLS")) => (e.to_string(), span),
            _ => {
                return Err(SyntaxError::new(
                    span,
                    "SAVE TRANSLATE needs /TYPE=CSV, TAB or XLS /VERSION=12",
                ));
            }
        },
    };
    command.kind = match name.as_str() {
        "CSV" => TranslateType::Csv,
        "TAB" => TranslateType::Tab,
        // XLSX is not a type of the manual; it stands for XLS /VERSION=12
        "XLSX" => TranslateType::Xlsx,
        "XLS" if version.as_deref() == Some("12") => TranslateType::Xlsx,
        "XLS" if version.is_none() && extension.as_deref() == Some("XLSX") => TranslateType::Xlsx,
        "XLS" => {
            return Err(SyntaxError::new(
                span,
                "Statscraft writes Excel 2007 and later workbooks: use /VERSION=12 and a .xlsx file",
            ));
        }
        "DB2" | "DB3" | "DB4" | "ODBC" | "PC" | "SAS" | "STATA" | "SYM" | "SLK" | "WKS" | "WK1"
        | "WK3" => {
            return Err(SyntaxError::new(
                span,
                format!(
                    "SAVE TRANSLATE /TYPE={name} is not supported yet; write CSV, TAB or XLS /VERSION=12"
                ),
            ));
        }
        other => {
            return Err(SyntaxError::new(
                span,
                format!("unknown SAVE TRANSLATE type {other}"),
            ));
        }
    };
    if command.kind == TranslateType::Xlsx && command.encoding.is_some() {
        return Err(SyntaxError::new(
            span,
            "ENCODING applies to TYPE=CSV and TAB; a workbook is always Unicode",
        ));
    }
    let decimal = command
        .decimal_comma
        .map(|comma| if comma { ',' } else { '.' });
    let clash = Some(command.qualifier) == command.delimiter
        || (decimal.is_some() && [command.delimiter, Some(command.qualifier)].contains(&decimal));
    if clash {
        return Err(SyntaxError::new(
            span,
            "the delimiter, qualifier and decimal mark must all differ",
        ));
    }
    Ok(Command::SaveTranslate(command))
}
