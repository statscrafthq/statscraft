//! Parsers of the output document commands: OUTPUT NEW, OUTPUT NAME, OUTPUT ACTIVATE,
//! OUTPUT CLOSE, OUTPUT DISPLAY, OUTPUT EXPORT, OUTPUT SAVE and OUTPUT OPEN.
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
//!   are refused.
//! - The image subcommands (/BMP, /EMF, /EPS, /JPG, /PNG, /TIF) set how the pictures of charts
//!   are written in their format. IMAGEROOT is needed only when no subcommand writes a
//!   document; PERCENTSIZE is above 0 and at most 200. GRAYSCALE (JPG) and DEPTH (PNG) choose
//!   color, shades of gray or black and white; COMPRESS, WIDTH, PREVIEW and FONTHANDLING are
//!   read and change nothing.
//! - OUTPUT SAVE and OUTPUT OPEN: keywords spelled in full, each at most once, equals signs
//!   required; OUTFILE and FILE are required. TYPE, LOCK and STOREMODEL are read and change
//!   nothing. A password on /PASSPROTECT is refused here, so nothing is written unprotected
//!   that the analyst wanted protected; PASSWORD=NONE is fine. The PASSWORD of OUTPUT OPEN is
//!   read and ignored, as a Statscraft output file is never encrypted.

use crate::ast::Command;
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// BEGIN CHART, a Statscraft extension: the splitter hands over the JSON lines as one string
/// token each; the engine reads them.
pub(crate) fn begin_chart(p: &mut Parser) -> PResult<Command> {
    let mut lines = Vec::new();
    while let Some(TokenKind::Str(text)) = p.peek() {
        lines.push((p.span().line, text.clone()));
        p.pos += 1;
    }
    p.finish(&[])?;
    Ok(Command::BeginChart(lines))
}

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
    /// Write a document to a Statscraft output file.
    Save(OutputSave),
    /// Read a Statscraft output file into a new document.
    Open(OutputOpen),
    /// Change, hide or delete items of a document.
    Modify(Box<crate::output_modify::OutputModify>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputSave {
    /// None is `*`, the designated document.
    pub name: Option<String>,
    pub file: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputOpen {
    pub file: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputExport {
    /// None is `*`, the designated document.
    pub name: Option<String>,
    /// The files to write, in the order of the subcommands.
    pub targets: Vec<ExportTarget>,
    /// The image subcommands, in their order.
    pub images: Vec<ImageExport>,
    /// CONTENTS EXPORT=VISIBLE: hidden items are left out.
    pub visible_only: bool,
}

/// The picture formats of the image subcommands and of IMAGEFORMAT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Bmp,
    Emf,
    Eps,
    Jpg,
    Png,
    Tif,
}

/// The colors of a picture: GRAYSCALE=YES and DEPTH=GRAYS256 give gray, DEPTH=BLACKWHITE black
/// and white.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageTone {
    Color,
    Gray,
    BlackWhite,
}

/// One image subcommand: the settings of the pictures in its format.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageExport {
    pub format: ImageFormat,
    /// IMAGEROOT: the pictures are root0, root1, ... when no subcommand writes a document.
    pub root: Option<String>,
    /// PERCENTSIZE: the size against the picture of a chart as a file.
    pub percent: f64,
    pub tone: ImageTone,
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
        /// IMAGEFORMAT: the format of the pictures in the page.
        image_format: ImageFormat,
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
        /// IMAGEFORMAT: the format of the picture files beside the text.
        image_format: ImageFormat,
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
    read: impl FnMut(&mut Parser, &str) -> PResult<()>,
) -> PResult<()> {
    keywords_on(p, &format!("/{subcommand}"), known, read)
}

/// `keywords`, for those of `place`: a subcommand such as /XLSX or a command.
fn keywords_on(
    p: &mut Parser,
    place: &str,
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
                format!("{keyword} is given twice on {place}"),
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
const IMAGES: [(&str, ImageFormat, &[&str]); 6] = [
    (
        "BMP",
        ImageFormat::Bmp,
        &["IMAGEROOT", "PERCENTSIZE", "COMPRESS"],
    ),
    ("EMF", ImageFormat::Emf, &["IMAGEROOT", "PERCENTSIZE"]),
    (
        "EPS",
        ImageFormat::Eps,
        &["IMAGEROOT", "WIDTH", "PREVIEW", "FONTHANDLING"],
    ),
    (
        "JPG",
        ImageFormat::Jpg,
        &["IMAGEROOT", "PERCENTSIZE", "GRAYSCALE"],
    ),
    (
        "PNG",
        ImageFormat::Png,
        &["IMAGEROOT", "PERCENTSIZE", "DEPTH"],
    ),
    ("TIF", ImageFormat::Tif, &["IMAGEROOT", "PERCENTSIZE"]),
];

/// IMAGEFORMAT = one of `formats`, which are names of `IMAGES`.
fn image_format(p: &mut Parser, formats: &[&str]) -> PResult<ImageFormat> {
    let name = formats[choice(p, formats)?];
    Ok(IMAGES
        .iter()
        .find(|(n, ..)| *n == name)
        .expect("a known format")
        .1)
}

/// The keywords of an image subcommand.
fn image_export(
    p: &mut Parser,
    word: &str,
    format: ImageFormat,
    known: &[&str],
) -> PResult<ImageExport> {
    let mut image = ImageExport {
        format,
        root: None,
        percent: 100.0,
        tone: ImageTone::Color,
    };
    keywords(p, word, known, |p, k| {
        match k {
            "IMAGEROOT" => {
                p.expect(Punct::Eq)?;
                image.root = Some(p.string("a root name in quotes")?);
            }
            "PERCENTSIZE" => {
                p.expect(Punct::Eq)?;
                let at = p.pos;
                image.percent = match p.number() {
                    Some(n) if n > 0.0 && n <= 200.0 => n,
                    _ => {
                        return Err(SyntaxError::new(
                            p.span_from(at),
                            "PERCENTSIZE is a number above 0 and at most 200",
                        ));
                    }
                };
            }
            "GRAYSCALE" => {
                if yes_no(p)? {
                    image.tone = ImageTone::Gray;
                }
            }
            "DEPTH" => {
                let depths = [
                    "SCREEN",
                    "BLACKWHITE",
                    "GRAYS256",
                    "COLORS16",
                    "COLORS256",
                    "TRUE24BIT",
                    "TRUE32BIT",
                ];
                image.tone = match choice(p, &depths)? {
                    1 => ImageTone::BlackWhite,
                    2 => ImageTone::Gray,
                    _ => ImageTone::Color,
                };
            }
            "COMPRESS" | "PREVIEW" => {
                yes_no(p)?;
            }
            "FONTHANDLING" => {
                choice(p, &["REFERENCES", "CURVES"])?;
            }
            // WIDTH=PERCENT(n), INCHES(n), CM(n) or POINTS(n)
            _ => skip_value(p)?,
        }
        Ok(())
    })?;
    Ok(image)
}

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
        images: Vec::new(),
        visible_only: false,
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
                // every table has one layer, so the LAYERS and MODELVIEWS choices give the same
                keywords(p, &word, &["EXPORT", "LAYERS", "MODELVIEWS"], |p, k| {
                    if k == "EXPORT" {
                        export.visible_only = choice(p, &["ALL", "VISIBLE"])? == 1;
                        return Ok(());
                    }
                    choice(p, &["PRINTSETTING", "ALL", "VISIBLE"]).map(|_| ())
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
                let mut image = ImageFormat::Jpg;
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
                            "IMAGEFORMAT" => image = image_format(p, &["JPG", "BMP", "PNG"])?,
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
                        image_format: image,
                    });
                }
            }
            "TEXT" => {
                let mut encoding = TextEncoding::Locale;
                let mut column_width = None;
                let mut tab = false;
                let (mut row_border, mut column_border) = ('-', '|');
                let mut image = ImageFormat::Jpg;
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
                                let all = ["JPG", "BMP", "EMF", "EPS", "PNG", "TIF"];
                                image = image_format(p, &all)?;
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
                        image_format: image,
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
            other => match IMAGES.iter().find(|(name, ..)| *name == other) {
                Some((_, format, known)) => {
                    export.images.push(image_export(p, other, *format, known)?);
                }
                None => {
                    p.pos = at;
                    return Err(p.subcommand_error("OUTPUT EXPORT", &["REPORT"]));
                }
            },
        }
    }
    p.finish(&["'/'"])?;
    if export.targets.is_empty() {
        let message = match export.images.is_empty() {
            true => format!(
                "OUTPUT EXPORT needs {SUPPORTED} with DOCUMENTFILE, or an image subcommand \
                 with IMAGEROOT"
            ),
            false => "without a document subcommand, the image subcommands need \
                      IMAGEROOT='root' for the names of their files"
                .to_string(),
        };
        if export.images.is_empty() || export.images.iter().any(|i| i.root.is_none()) {
            return Err(SyntaxError::new(p.span_from(start), message));
        }
    }
    Ok(Command::Output(OutputCommand::Export(Box::new(export))))
}

/// `= name`, or `= *` (None).
fn document_or_star(p: &mut Parser) -> PResult<Option<String>> {
    p.expect(Punct::Eq)?;
    if p.eat(Punct::Star) {
        return Ok(None);
    }
    p.name("a document name or *").map(Some)
}

fn file_value(p: &mut Parser) -> PResult<String> {
    p.expect(Punct::Eq)?;
    p.file("a file name in quotes")
}

pub(crate) fn output_save(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let (mut name, mut file) = (None, None);
    keywords_on(
        p,
        "OUTPUT SAVE",
        &["NAME", "OUTFILE", "TYPE", "LOCK", "STOREMODEL"],
        |p, k| {
            match k {
                "NAME" => name = document_or_star(p)?,
                "OUTFILE" => file = Some(file_value(p)?),
                "TYPE" => {
                    choice(p, &["SPV", "SPW"])?;
                }
                _ => {
                    yes_no(p)?;
                }
            }
            Ok(())
        },
    )?;
    let file = file
        .ok_or_else(|| SyntaxError::new(p.span_from(start), "OUTPUT SAVE needs OUTFILE='file'"))?;
    if p.eat(Punct::Slash) {
        if !p.eat_word("PASSPROTECT") {
            return Err(p.unexpected(&["PASSPROTECT"]));
        }
        keywords(p, "PASSPROTECT", &["PASSWORD", "ENCRYPTEDPW"], |p, k| {
            if k == "ENCRYPTEDPW" {
                return yes_no(p).map(|_| ());
            }
            p.expect(Punct::Eq)?;
            if p.eat_word("NONE") {
                return Ok(());
            }
            let at = p.pos;
            p.string("NONE or a password in quotes")?;
            Err(SyntaxError::new(
                p.span_from(at),
                "Statscraft output files cannot be protected by a password, so nothing is saved",
            )
            .hint("leave out /PASSPROTECT, or write PASSWORD=NONE"))
        })?;
    }
    p.finish(&[])?;
    Ok(Command::Output(OutputCommand::Save(OutputSave {
        name,
        file,
    })))
}

pub(crate) fn output_open(p: &mut Parser) -> PResult<Command> {
    let start = p.pos;
    let (mut name, mut file) = (None, None);
    keywords_on(
        p,
        "OUTPUT OPEN",
        &["FILE", "NAME", "LOCK", "PASSWORD"],
        |p, k| {
            match k {
                "FILE" => file = Some(file_value(p)?),
                "NAME" => {
                    p.expect(Punct::Eq)?;
                    name = Some(p.name("a document name")?);
                }
                "LOCK" => {
                    yes_no(p)?;
                }
                _ => {
                    p.expect(Punct::Eq)?;
                    p.string("a password in quotes")?;
                }
            }
            Ok(())
        },
    )?;
    p.finish(&[])?;
    let file =
        file.ok_or_else(|| SyntaxError::new(p.span_from(start), "OUTPUT OPEN needs FILE='file'"))?;
    Ok(Command::Output(OutputCommand::Open(OutputOpen {
        file,
        name,
    })))
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
