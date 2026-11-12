//! Parsers of the output document commands: OUTPUT NEW, OUTPUT NAME, OUTPUT ACTIVATE,
//! OUTPUT CLOSE, OUTPUT DISPLAY and OUTPUT EXPORT.
//!
//! Rules from the descriptions of these commands:
//! - A document name follows the rules for variable names. NAME= is optional on OUTPUT NAME,
//!   ACTIVATE and CLOSE; OUTPUT CLOSE also takes * (the designated document) or ALL.
//! - OUTPUT NEW takes NAME=name; TYPE is obsolete and ignored.
//! - OUTPUT EXPORT: subcommand names and keywords are spelled in full, each subcommand and each
//!   keyword at most once, and the equals signs are required. NAME=* or name, then /CONTENTS
//!   and the document and image subcommands in any order.
//! - Choices: Statscraft writes /XLSX, /DOC, /PPT, /HTML, /TEXT and /CSV (CSV is its own
//!   addition, the tables as comma-separated values). /DOC and /PPT write the current Word and
//!   PowerPoint formats (.docx, .pptx) whatever the file is called; their page and slide
//!   sizes, margins and wide-table settings are accepted and not applied. /PDF, /XLS and /XLSM
//!   are refused; the image subcommands write no files, as Statscraft draws no pictures yet.

use crate::ast::Command;
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, PartialEq)]
pub enum OutputCommand {
    New(Option<String>),
    Name(String),
    Activate(String),
    /// None is `*`, the designated document.
    Close(Option<String>),
    CloseAll,
    Display,
    Export(Box<OutputExport>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputExport {
    /// None is `*`, the designated document.
    pub name: Option<String>,
    /// The files to write, in the order of the subcommands.
    pub targets: Vec<ExportTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetOperation {
    CreateFile,
    CreateSheet,
    ModifySheet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SheetLocation {
    LastColumn,
    LastRow,
    /// STARTCELL('B2'): the row and column, from 0.
    StartCell(u32, u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextEncoding {
    Locale,
    Utf8,
    Utf16,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportTarget {
    Xlsx {
        file: String,
        operation: SheetOperation,
        sheet: Option<String>,
        location: SheetLocation,
        notes: bool,
    },
    Html {
        file: String,
        notes: bool,
        styling: bool,
    },
    Text {
        file: String,
        notes: bool,
        encoding: TextEncoding,
        /// COLUMNWIDTH=n; None is AUTOFIT.
        column_width: Option<u32>,
        tab: bool,
        row_border: char,
        column_border: char,
    },
    Csv {
        file: String,
        notes: bool,
    },
    /// A Word document: tables, and charts as editable charts.
    Doc {
        file: String,
        notes: bool,
    },
    /// A presentation: a slide per table and per chart; `titles` is SLIDETITLES.
    Ppt {
        file: String,
        notes: bool,
        titles: bool,
    },
}

/// A document name, after an optional `NAME=` when `keyword`.
fn document_name(p: &mut Parser, keyword: bool) -> PResult<String> {
    if keyword
        && matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("NAME"))
        && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq))
    {
        p.pos += 2;
    }
    p.name("a document name")
}

pub(crate) fn output_new(p: &mut Parser) -> PResult<Command> {
    let mut name = None;
    let mut seen_type = false;
    while p.peek().is_some() {
        let at = p.pos;
        if p.eat_word("NAME") && name.is_none() {
            p.expect(Punct::Eq)?;
            name = Some(p.name("a document name")?);
        } else if p.eat_word("TYPE") && !seen_type {
            // obsolete, ignored
            p.expect(Punct::Eq)?;
            p.name("VIEWER")?;
            seen_type = true;
        } else if p.pos > at {
            return Err(SyntaxError::new(
                p.span_from(at),
                "a keyword can be given only once",
            ));
        } else {
            return Err(p.unexpected(&["NAME", "end of command"]));
        }
    }
    Ok(Command::Output(OutputCommand::New(name)))
}

pub(crate) fn output_name(p: &mut Parser) -> PResult<Command> {
    let name = document_name(p, true)?;
    p.finish(&[])?;
    Ok(Command::Output(OutputCommand::Name(name)))
}

pub(crate) fn output_activate(p: &mut Parser) -> PResult<Command> {
    let name = document_name(p, true)?;
    p.finish(&[])?;
    Ok(Command::Output(OutputCommand::Activate(name)))
}

pub(crate) fn output_close(p: &mut Parser) -> PResult<Command> {
    if matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("NAME"))
        && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq))
    {
        p.pos += 2;
    }
    let command = if p.eat(Punct::Star) {
        OutputCommand::Close(None)
    } else if p.eat_word("ALL") {
        OutputCommand::CloseAll
    } else {
        OutputCommand::Close(Some(p.name("a document name, * or ALL")?))
    };
    p.finish(&[])?;
    Ok(Command::Output(command))
}

pub(crate) fn output_display(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::Output(OutputCommand::Display))
}

/// `= word`, one of `choices`, spelled in full; its index.
fn choice(p: &mut Parser, choices: &[&str]) -> PResult<usize> {
    p.expect(Punct::Eq)?;
    let found = choices
        .iter()
        .position(|c| matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(c)));
    match found {
        Some(i) => {
            p.pos += 1;
            Ok(i)
        }
        None => Err(p.unexpected(choices)),
    }
}

fn yes_no(p: &mut Parser) -> PResult<bool> {
    Ok(choice(p, &["YES", "NO"])? == 0)
}

/// One character in quotes, for ROWBORDER and COLUMNBORDER.
fn border(p: &mut Parser) -> PResult<char> {
    p.expect(Punct::Eq)?;
    let at = p.pos;
    let text = p.string("a character in quotes")?;
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Ok(c),
        _ => Err(SyntaxError::new(
            p.span_from(at),
            "a border is one character in quotes",
        )),
    }
}

/// A cell reference such as B2 as (row, column) from 0.
fn cell_reference(text: &str) -> Option<(u32, u16)> {
    let letters: String = text.chars().take_while(char::is_ascii_alphabetic).collect();
    let digits = &text[letters.len()..];
    if letters.is_empty() || letters.len() > 3 || digits.is_empty() {
        return None;
    }
    let column = letters.chars().fold(0u32, |n, c| {
        n * 26 + (c.to_ascii_uppercase() as u32 - 'A' as u32 + 1)
    });
    let row: u32 = digits.parse().ok()?;
    if column == 0 || column > 16_384 || row == 0 || row > 1_048_576 {
        return None;
    }
    Some((row - 1, (column - 1) as u16))
}

/// The keywords of one subcommand: `keyword = value`, each at most once. `read` takes the
/// value of a keyword it knows (the parser just after the keyword) and returns false for
/// one it does not know.
fn keywords(
    p: &mut Parser,
    subcommand: &str,
    known: &[&str],
    mut read: impl FnMut(&mut Parser, &str) -> PResult<()>,
) -> PResult<()> {
    let mut seen: Vec<&str> = Vec::new();
    if at_end(p) {
        return Err(p.unexpected(known));
    }
    while !at_end(p) {
        let at = p.pos;
        let Some(keyword) = known
            .iter()
            .find(|k| matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(k)))
        else {
            return Err(p.unexpected(known));
        };
        if seen.contains(keyword) {
            return Err(SyntaxError::new(
                p.span(),
                format!("{keyword} is given twice on /{subcommand}"),
            ));
        }
        seen.push(keyword);
        p.pos += 1;
        read(p, keyword)?;
        if p.pos == at + 1 {
            return Err(p.unexpected(&["'='"]));
        }
    }
    Ok(())
}

/// The refused document formats, with what they are.
const REFUSED: [(&str, &str); 3] = [
    ("PDF", "PDF"),
    ("XLS", "Excel 97-2004"),
    ("XLSM", "macro-enabled Excel"),
];

/// The image subcommands with their keywords.
const IMAGES: [(&str, &[&str]); 6] = [
    ("BMP", &["IMAGEROOT", "PERCENTSIZE", "COMPRESS"]),
    ("EMF", &["IMAGEROOT", "PERCENTSIZE"]),
    ("EPS", &["IMAGEROOT", "WIDTH", "PREVIEW", "FONTHANDLING"]),
    ("JPG", &["IMAGEROOT", "PERCENTSIZE", "GRAYSCALE"]),
    ("PNG", &["IMAGEROOT", "PERCENTSIZE", "DEPTH"]),
    ("TIF", &["IMAGEROOT", "PERCENTSIZE"]),
];

const SUPPORTED: &str = "/XLSX, /DOC, /PPT, /HTML, /TEXT or /CSV";

/// Passes over the value of an image setting: a word, number or string, or a function such
/// as PERCENT(50).
fn skip_value(p: &mut Parser) -> PResult<()> {
    p.expect(Punct::Eq)?;
    match p.peek() {
        Some(TokenKind::Word(_)) => {
            p.pos += 1;
            if p.eat(Punct::LParen) {
                while !p.eat(Punct::RParen) {
                    if at_end(p) {
                        return Err(p.unexpected(&["')'"]));
                    }
                    p.pos += 1;
                }
            }
        }
        Some(TokenKind::Number(_) | TokenKind::Str(_)) => p.pos += 1,
        _ => return Err(p.unexpected(&["a value"])),
    }
    Ok(())
}

fn needs_file(p: &Parser, at: usize, word: &str, file: Option<String>) -> PResult<String> {
    file.ok_or_else(|| {
        SyntaxError::new(
            p.span_from(at),
            format!("/{word} needs DOCUMENTFILE='file'"),
        )
    })
}

pub(crate) fn output_export(p: &mut Parser) -> PResult<Command> {
    let mut export = OutputExport {
        name: None,
        targets: Vec::new(),
    };
    let start = p.pos;
    if p.eat_word("NAME") {
        p.expect(Punct::Eq)?;
        export.name = if p.eat(Punct::Star) {
            None
        } else {
            Some(p.name("a document name or *")?)
        };
    }
    let mut seen: Vec<String> = Vec::new();
    while p.eat(Punct::Slash) {
        let Some(TokenKind::Word(word)) = p.peek() else {
            return Err(p.unexpected(&["a subcommand name"]));
        };
        let word = word.to_uppercase();
        let at = p.pos;
        if seen.contains(&word) {
            return Err(SyntaxError::new(
                p.span(),
                format!("subcommand {word} is given twice"),
            ));
        }
        if let Some((_, what)) = REFUSED.iter().find(|(name, _)| *name == word) {
            return Err(SyntaxError::new(
                p.span(),
                format!(
                    "OUTPUT EXPORT cannot write {what} files (/{word}); Statscraft exports \
                     output with {SUPPORTED}"
                ),
            ));
        }
        p.pos += 1;
        seen.push(word.clone());
        let mut file = None;
        let mut notes = true;
        match word.as_str() {
            "CONTENTS" => {
                // every table is visible and has one layer, so all choices give the same
                keywords(p, &word, &["EXPORT", "LAYERS", "MODELVIEWS"], |p, k| {
                    let choices: &[&str] = match k {
                        "EXPORT" => &["ALL", "VISIBLE"],
                        _ => &["PRINTSETTING", "ALL", "VISIBLE"],
                    };
                    choice(p, choices).map(|_| ())
                })?;
            }
            "XLSX" => {
                let mut operation = SheetOperation::CreateFile;
                let mut sheet = None;
                let mut location = SheetLocation::LastColumn;
                keywords(
                    p,
                    &word,
                    &[
                        "DOCUMENTFILE",
                        "OPERATION",
                        "SHEET",
                        "LOCATION",
                        "NOTESCAPTIONS",
                    ],
                    |p, k| {
                        match k {
                            "DOCUMENTFILE" => {
                                p.expect(Punct::Eq)?;
                                file = Some(p.file("a file name in quotes")?);
                            }
                            "OPERATION" => {
                                operation = [
                                    SheetOperation::CreateFile,
                                    SheetOperation::CreateSheet,
                                    SheetOperation::ModifySheet,
                                ][choice(p, &["CREATEFILE", "CREATESHEET", "MODIFYSHEET"])?]
                            }
                            "SHEET" => {
                                p.expect(Punct::Eq)?;
                                let at = p.pos;
                                let name = p.string("a sheet name in quotes")?;
                                if name.is_empty()
                                    || name.chars().count() > 31
                                    || name.contains(['/', '\\', '[', ']', '?', '*', ':'])
                                {
                                    return Err(SyntaxError::new(
                                        p.span_from(at),
                                        "a sheet name has 1 to 31 characters and none of / \\ [ ] ? * :",
                                    ));
                                }
                                sheet = Some(name);
                            }
                            "LOCATION" => {
                                location = match choice(p, &["LASTCOLUMN", "LASTROW", "STARTCELL"])?
                                {
                                    0 => SheetLocation::LastColumn,
                                    1 => SheetLocation::LastRow,
                                    _ => {
                                        p.expect(Punct::LParen)?;
                                        let at = p.pos;
                                        let text = p.string("a cell such as 'B2'")?;
                                        let Some((row, column)) = cell_reference(&text) else {
                                            return Err(SyntaxError::new(
                                                p.span_from(at),
                                                format!(
                                                    "{text} is no cell; write one such as 'B2'"
                                                ),
                                            ));
                                        };
                                        p.expect(Punct::RParen)?;
                                        SheetLocation::StartCell(row, column)
                                    }
                                }
                            }
                            _ => notes = yes_no(p)?,
                        }
                        Ok(())
                    },
                )?;
                if operation == SheetOperation::ModifySheet && sheet.is_none() {
                    return Err(SyntaxError::new(
                        p.span_from(at),
                        "OPERATION=MODIFYSHEET needs SHEET='name'",
                    ));
                }
                {
                    let file = needs_file(p, at, &word, file.take())?;
                    export.targets.push(ExportTarget::Xlsx {
                        file,
                        operation,
                        sheet,
                        location,
                        notes,
                    });
                }
            }
            "HTML" => {
                let mut styling = true;
                keywords(
                    p,
                    &word,
                    &[
                        "DOCUMENTFILE",
                        "NOTESCAPTIONS",
                        "IMAGEFORMAT",
                        "STYLING",
                        "INTERACTIVELAYERS",
                    ],
                    |p, k| {
                        match k {
                            "DOCUMENTFILE" => {
                                p.expect(Punct::Eq)?;
                                file = Some(p.file("a file name in quotes")?);
                            }
                            "NOTESCAPTIONS" => notes = yes_no(p)?,
                            "IMAGEFORMAT" => {
                                choice(p, &["JPG", "BMP", "PNG"])?;
                            }
                            "STYLING" => styling = yes_no(p)?,
                            _ => {
                                yes_no(p)?;
                            }
                        }
                        Ok(())
                    },
                )?;
                {
                    let file = needs_file(p, at, &word, file.take())?;
                    export.targets.push(ExportTarget::Html {
                        file,
                        notes,
                        styling,
                    });
                }
            }
            "TEXT" => {
                let mut encoding = TextEncoding::Locale;
                let mut column_width = None;
                let mut tab = false;
                let (mut row_border, mut column_border) = ('-', '|');
                keywords(
                    p,
                    &word,
                    &[
                        "DOCUMENTFILE",
                        "ENCODING",
                        "NOTESCAPTIONS",
                        "COLUMNWIDTH",
                        "SEPARATOR",
                        "ROWBORDER",
                        "COLUMNBORDER",
                        "IMAGEFORMAT",
                    ],
                    |p, k| {
                        match k {
                            "DOCUMENTFILE" => {
                                p.expect(Punct::Eq)?;
                                file = Some(p.file("a file name in quotes")?);
                            }
                            "ENCODING" => {
                                encoding = [
                                    TextEncoding::Locale,
                                    TextEncoding::Utf8,
                                    TextEncoding::Utf16,
                                ][choice(p, &["LOCALE", "UTF8", "UTF16"])?]
                            }
                            "NOTESCAPTIONS" => notes = yes_no(p)?,
                            "COLUMNWIDTH" => {
                                p.expect(Punct::Eq)?;
                                if p.eat_word("AUTOFIT") {
                                    column_width = None;
                                } else {
                                    column_width =
                                        Some(crate::data_list::count(p, "AUTOFIT or a width")?);
                                }
                            }
                            "SEPARATOR" => tab = choice(p, &["SPACE", "TAB"])? == 1,
                            "ROWBORDER" => row_border = border(p)?,
                            "COLUMNBORDER" => column_border = border(p)?,
                            _ => {
                                choice(p, &["JPG", "BMP", "EMF", "EPS", "PNG", "TIF"])?;
                            }
                        }
                        Ok(())
                    },
                )?;
                {
                    let file = needs_file(p, at, &word, file.take())?;
                    export.targets.push(ExportTarget::Text {
                        file,
                        notes,
                        encoding,
                        column_width,
                        tab,
                        row_border,
                        column_border,
                    });
                }
            }
            "DOC" | "PPT" => {
                let mut titles = true;
                let known: &[&str] = match word.as_str() {
                    "DOC" => &[
                        "DOCUMENTFILE",
                        "NOTESCAPTIONS",
                        "WIDETABLES",
                        "PAGESIZE",
                        "TOPMARGIN",
                        "BOTTOMMARGIN",
                        "LEFTMARGIN",
                        "RIGHTMARGIN",
                        "PAGEBREAKS",
                    ],
                    _ => &[
                        "DOCUMENTFILE",
                        "NOTESCAPTIONS",
                        "SLIDETITLES",
                        "WIDETABLES",
                        "SLIDESIZE",
                        "TOPMARGIN",
                        "BOTTOMMARGIN",
                        "LEFTMARGIN",
                        "RIGHTMARGIN",
                    ],
                };
                keywords(p, &word, known, |p, k| {
                    match k {
                        "DOCUMENTFILE" => {
                            p.expect(Punct::Eq)?;
                            file = Some(p.file("a file name in quotes")?);
                        }
                        "NOTESCAPTIONS" => notes = yes_no(p)?,
                        "SLIDETITLES" => titles = yes_no(p)?,
                        "PAGEBREAKS" => {
                            yes_no(p)?;
                        }
                        "WIDETABLES" => {
                            choice(p, &["WRAP", "SHRINK", "ASIS"])?;
                        }
                        // sizes and margins: INCHES(...) or MM(...)
                        _ => skip_value(p)?,
                    }
                    Ok(())
                })?;
                let file = needs_file(p, at, &word, file.take())?;
                export.targets.push(match word.as_str() {
                    "DOC" => ExportTarget::Doc { file, notes },
                    _ => ExportTarget::Ppt {
                        file,
                        notes,
                        titles,
                    },
                });
            }
            "CSV" => {
                keywords(p, &word, &["DOCUMENTFILE", "NOTESCAPTIONS"], |p, k| {
                    if k == "DOCUMENTFILE" {
                        p.expect(Punct::Eq)?;
                        file = Some(p.file("a file name in quotes")?);
                    } else {
                        notes = yes_no(p)?;
                    }
                    Ok(())
                })?;
                let file = needs_file(p, at, &word, file.take())?;
                export.targets.push(ExportTarget::Csv { file, notes });
            }
            other => match IMAGES.iter().find(|(name, _)| *name == other) {
                Some((_, known)) => keywords(p, other, known, |p, _| skip_value(p))?,
                None => {
                    p.pos = at;
                    return Err(p.subcommand_error("OUTPUT EXPORT", &["REPORT"]));
                }
            },
        }
    }
    p.finish(&["'/'"])?;
    if export.targets.is_empty() {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!(
                "OUTPUT EXPORT needs {SUPPORTED} with DOCUMENTFILE; Statscraft draws no \
                 pictures yet, so the image subcommands write no files"
            ),
        ));
    }
    Ok(Command::Output(OutputCommand::Export(Box::new(export))))
}

#[cfg(test)]
mod tests {
    use super::cell_reference;

    #[test]
    fn cell_references() {
        assert_eq!(cell_reference("A1"), Some((0, 0)));
        assert_eq!(cell_reference("b2"), Some((1, 1)));
        assert_eq!(cell_reference("AA10"), Some((9, 26)));
        assert_eq!(cell_reference("XFD1048576"), Some((1_048_575, 16_383)));
        assert_eq!(cell_reference("XFE1"), None);
        assert_eq!(cell_reference("B"), None);
        assert_eq!(cell_reference("2"), None);
        assert_eq!(cell_reference("B0"), None);
    }
}
