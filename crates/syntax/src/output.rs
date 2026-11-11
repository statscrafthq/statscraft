//! Parsers of the output and session commands TITLE, SUBTITLE, ECHO, NEW FILE, CLEAR
//! TRANSFORMATIONS, ERASE, SYSFILE INFO, PERMISSIONS, HOST and DATE, and of the case output
//! commands PRINT, PRINT EJECT, PRINT SPACE, WRITE, PRINT FORMATS, WRITE FORMATS and XSAVE.

use crate::ast::{Command, Compression, Expr, FileOption, Format, FormatType, VarItem};
use crate::commands::{file_option, format};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

/// PRINT, PRINT EJECT or WRITE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrintKind {
    Print,
    PrintEject,
    Write,
}

/// PRINT, PRINT EJECT or WRITE: the lines to show or write for every case.
#[derive(Debug, Clone, PartialEq)]
pub struct Print {
    pub kind: PrintKind,
    /// None: the lines go to the output.
    pub outfile: Option<String>,
    pub encoding: Option<String>,
    /// WRITE BOM=NO
    pub no_bom: bool,
    /// RECORDS=n as given; it only says how many lines are expected.
    pub records: Option<u32>,
    /// TABLE: a table of where each variable goes.
    pub table: bool,
    /// One list of items per line.
    pub lines: Vec<Vec<PrintItem>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrintItem {
    /// Variables, placed as given; None: each in its own format, with a blank after it on
    /// PRINT.
    Variables(Vec<VarItem>, Option<Placement>),
    /// A quoted text, from a column when given (and to a last column).
    Text {
        text: String,
        columns: Option<(u32, Option<u32>)>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Placement {
    /// `first-last [(type [, decimals])]`: the columns, shared equally by the variables.
    Columns {
        first: u32,
        last: u32,
        kind: Option<FormatType>,
        decimals: u8,
    },
    /// `(format list)`, repeats spelled out.
    Formats(Vec<FormatItem>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FormatItem {
    Format(Format),
    /// `Tn`: go to column n.
    Tab(u32),
    /// `nX`: skip n columns.
    Skip(u32),
}

/// PRINT SPACE: blank lines.
#[derive(Debug, Clone, PartialEq)]
pub struct PrintSpace {
    pub outfile: Option<String>,
    pub encoding: Option<String>,
    /// How many lines; None is one.
    pub lines: Option<Expr>,
}

/// XSAVE: write the cases to a data file as the data pass reads them.
#[derive(Debug, Clone, PartialEq)]
pub struct Xsave {
    pub outfile: String,
    pub compression: Option<Compression>,
    /// `/KEEP`, `/DROP` and `/RENAME` in the order written.
    pub options: Vec<FileOption>,
    pub map: bool,
    /// PERMISSIONS=READONLY is Some(true), WRITEABLE Some(false).
    pub read_only: Option<bool>,
}

/// One keyword of DATE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DateUnit {
    Cycle,
    Year,
    Quarter,
    Month,
    Week,
    Day,
    Hour,
    Minute,
    Second,
    Obs,
}

impl DateUnit {
    pub const ALL: [DateUnit; 10] = [
        DateUnit::Year,
        DateUnit::Quarter,
        DateUnit::Month,
        DateUnit::Week,
        DateUnit::Day,
        DateUnit::Hour,
        DateUnit::Minute,
        DateUnit::Second,
        DateUnit::Cycle,
        DateUnit::Obs,
    ];

    pub fn name(self) -> &'static str {
        match self {
            DateUnit::Cycle => "CYCLE",
            DateUnit::Year => "YEAR",
            DateUnit::Quarter => "QUARTER",
            DateUnit::Month => "MONTH",
            DateUnit::Week => "WEEK",
            DateUnit::Day => "DAY",
            DateUnit::Hour => "HOUR",
            DateUnit::Minute => "MINUTE",
            DateUnit::Second => "SECOND",
            DateUnit::Obs => "OBS",
        }
    }

    /// YEAR, QUARTER and MONTH; they do not go with WEEK to SECOND.
    pub fn is_long(self) -> bool {
        matches!(self, DateUnit::Year | DateUnit::Quarter | DateUnit::Month)
    }

    pub fn is_short(self) -> bool {
        matches!(
            self,
            DateUnit::Week | DateUnit::Day | DateUnit::Hour | DateUnit::Minute | DateUnit::Second
        )
    }
}

/// DATE: the keywords with their starting value and periodicity as given.
#[derive(Debug, Clone, PartialEq)]
pub struct DateSpec {
    /// In the order written, a repeated keyword only at its last place.
    pub keywords: Vec<(DateUnit, Option<f64>, Option<f64>)>,
    /// BY n
    pub by: Option<f64>,
}

/// TITLE and SUBTITLE arrive as one string: the free text, or the quoted title.
pub(crate) fn title(p: &mut Parser) -> PResult<Command> {
    let text = p.string("the title")?;
    p.finish(&[])?;
    Ok(Command::Title {
        text,
        subtitle: false,
    })
}

pub(crate) fn subtitle(p: &mut Parser) -> PResult<Command> {
    let text = p.string("the subtitle")?;
    p.finish(&[])?;
    Ok(Command::Title {
        text,
        subtitle: true,
    })
}

pub(crate) fn echo(p: &mut Parser) -> PResult<Command> {
    let text = p.string("a quoted text")?;
    p.finish(&[])?;
    Ok(Command::Echo(text))
}

pub(crate) fn new_file(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::NewFile)
}

pub(crate) fn clear_transformations(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::ClearTransformations)
}

/// `ERASE FILE [=] 'file'`
pub(crate) fn erase(p: &mut Parser) -> PResult<Command> {
    if !p.eat_keyword("FILE") {
        return Err(p.unexpected(&["FILE"]));
    }
    p.eat(Punct::Eq);
    let file = p.file("a quoted file path")?;
    p.finish(&[])?;
    Ok(Command::Erase(file))
}

/// `SYSFILE INFO [FILE=] 'file' [PASSWORD='password']`
pub(crate) fn sysfile_info(p: &mut Parser) -> PResult<Command> {
    if p.eat_keyword("FILE") {
        p.eat(Punct::Eq);
    }
    let file = p.file("a quoted file path")?;
    if p.eat_keyword("PASSWORD") {
        return Err(SyntaxError::new(
            p.span_from(p.pos - 1),
            "encrypted data files are not supported yet",
        ));
    }
    p.finish(&[])?;
    Ok(Command::SysfileInfo(file))
}

/// `PERMISSIONS FILE='file' /PERMISSIONS READONLY | WRITEABLE`
pub(crate) fn permissions(p: &mut Parser) -> PResult<Command> {
    p.eat(Punct::Slash);
    if !p.eat_keyword("FILE") {
        return Err(p.unexpected(&["FILE"]));
    }
    p.eat(Punct::Eq);
    let file = p.file("a quoted file path")?;
    p.expect(Punct::Slash)?;
    if !p.eat_keyword("PERMISSIONS") {
        return Err(p.unexpected(&["PERMISSIONS"]));
    }
    p.eat(Punct::Eq);
    let read_only = read_only(p)?;
    p.finish(&[])?;
    Ok(Command::Permissions { file, read_only })
}

fn read_only(p: &mut Parser) -> PResult<bool> {
    if p.eat_keyword("READONLY") {
        Ok(true)
    } else if p.eat_keyword("WRITEABLE") {
        Ok(false)
    } else {
        Err(p.unexpected(&["READONLY", "WRITEABLE"]))
    }
}

/// `HOST COMMAND=['command' ...] [TIMELIMIT=n]`: read so that errors point at the right
/// place; the engine never runs it.
pub(crate) fn host(p: &mut Parser) -> PResult<Command> {
    if !p.eat_keyword("COMMAND") {
        return Err(p.unexpected(&["COMMAND"]));
    }
    p.expect(Punct::Eq)?;
    p.expect(Punct::LBracket)?;
    let mut commands = Vec::new();
    while let Some(TokenKind::Str(_)) = p.peek() {
        commands.push(p.string("a quoted command")?);
        p.eat(Punct::Comma);
    }
    if commands.is_empty() {
        return Err(p.unexpected(&["a quoted command"]));
    }
    p.expect(Punct::RBracket)?;
    if p.eat_keyword("TIMELIMIT") {
        p.expect(Punct::Eq)?;
        match p.number() {
            Some(n) if n >= 0.0 => {}
            _ => return Err(p.unexpected(&["a number of seconds"])),
        }
    }
    p.finish(&["TIMELIMIT"])?;
    Ok(Command::Host(commands))
}

/// The DATE keyword `word` stands for: any start of the keyword, MINUTE from two letters.
fn date_unit(word: &str) -> Option<DateUnit> {
    let upper = word.to_uppercase();
    DateUnit::ALL.into_iter().find(|unit| {
        let name = unit.name();
        name.starts_with(&upper) && (*unit != DateUnit::Minute || upper.len() >= 2)
    })
}

pub(crate) fn date(p: &mut Parser) -> PResult<Command> {
    let mut keywords: Vec<(DateUnit, Option<f64>, Option<f64>)> = Vec::new();
    let mut by = None;
    let whole = |p: &Parser, n: f64, what: &str| -> PResult<f64> {
        if n.fract() != 0.0 || n < 0.0 {
            return Err(SyntaxError::new(
                p.span_from(p.pos - 1),
                format!("{what} must be a whole number of 0 or more"),
            ));
        }
        Ok(n)
    };
    loop {
        p.eat(Punct::Comma);
        if by.is_none() && p.eat_word("BY") {
            let n = match p.number() {
                Some(n) => whole(p, n, "the increment after BY")?,
                None => return Err(p.unexpected(&["a whole number"])),
            };
            if n < 1.0 {
                return Err(SyntaxError::new(
                    p.span_from(p.pos - 1),
                    "the increment after BY must be 1 or more",
                ));
            }
            by = Some(n);
            continue;
        }
        let Some(TokenKind::Word(word)) = p.peek() else {
            break;
        };
        let Some(unit) = date_unit(word) else {
            return Err(p.unexpected(&["a DATE keyword such as YEAR or MONTH"]));
        };
        if by.is_some() {
            return Err(SyntaxError::new(
                p.span(),
                "BY goes after the lowest-order keyword, at the end",
            ));
        }
        p.pos += 1;
        p.eat(Punct::Comma);
        let start = match p.number() {
            Some(n) => Some(whole(p, n, "a starting value")?),
            None => None,
        };
        p.eat(Punct::Comma);
        let period = match start.is_some().then(|| p.number()).flatten() {
            Some(n) => Some(whole(p, n, "a periodicity")?),
            None => None,
        };
        if unit == DateUnit::Cycle && period.is_some() {
            return Err(SyntaxError::new(
                p.span_from(p.pos - 1),
                "CYCLE takes a starting value only, no periodicity",
            ));
        }
        if period == Some(0.0) {
            return Err(SyntaxError::new(
                p.span_from(p.pos - 1),
                "a periodicity must be 1 or more",
            ));
        }
        if unit == DateUnit::Obs && period.is_none() {
            return Err(SyntaxError::new(
                p.span_from(p.pos - 1),
                "OBS needs a starting value and a periodicity, as in OBS 1 12",
            ));
        }
        keywords.retain(|(u, _, _)| *u != unit);
        keywords.push((unit, start, period));
    }
    if keywords.is_empty() {
        return Err(p.unexpected(&["a DATE keyword such as YEAR or MONTH"]));
    }
    p.finish(&["a DATE keyword", "BY"])?;
    if keywords.iter().any(|k| k.0.is_long()) && keywords.iter().any(|k| k.0.is_short()) {
        return Err(SyntaxError::new(
            p.span_from(1),
            "YEAR, QUARTER and MONTH cannot go with WEEK, DAY, HOUR, MINUTE or SECOND",
        ));
    }
    Ok(Command::Date(DateSpec { keywords, by }))
}

/// The output formats PRINT and WRITE can show.
fn output_format(p: &Parser, text: &str) -> PResult<Format> {
    let parsed = format(text).map_err(|message| SyntaxError::new(p.span(), message))?;
    let binary = [
        FormatType::Z,
        FormatType::P,
        FormatType::Pk,
        FormatType::Ib,
        FormatType::Pib,
        FormatType::Pibhex,
        FormatType::Rb,
        FormatType::Rbhex,
    ];
    if binary.contains(&parsed.kind) {
        return Err(SyntaxError::new(
            p.span(),
            format!(
                "format {} is not supported yet on PRINT and WRITE",
                parsed.kind.name()
            ),
        ));
    }
    if parsed.width == 0 || parsed.width > 32767 {
        return Err(SyntaxError::new(
            p.span(),
            "a format needs a width of 1 or more",
        ));
    }
    if parsed.kind == FormatType::A && parsed.decimals > 0 {
        return Err(SyntaxError::new(p.span(), "an A format has no decimals"));
    }
    if parsed.decimals > 0 && u16::from(parsed.decimals) >= parsed.width {
        return Err(SyntaxError::new(
            p.span(),
            format!("format {text} has more decimals than its width allows"),
        ));
    }
    Ok(parsed)
}

/// A positive whole number.
fn count(p: &mut Parser, what: &str) -> PResult<u32> {
    match p.peek() {
        Some(TokenKind::Number(n)) if n.fract() == 0.0 && *n >= 1.0 && *n <= 1e9 => {
            let n = *n as u32;
            p.pos += 1;
            Ok(n)
        }
        _ => Err(p.unexpected(&[what])),
    }
}

/// `item, item ...` of a format list, after its opening parenthesis, up to and including the
/// closing one. `n(...)` repeats a group.
fn format_items(p: &mut Parser) -> PResult<Vec<FormatItem>> {
    let mut items = Vec::new();
    while !p.eat(Punct::RParen) {
        if p.eat(Punct::Comma) {
            continue;
        }
        let repeat = match p.peek() {
            Some(TokenKind::Number(_)) => Some(count(p, "a number")?),
            _ => None,
        };
        if p.eat(Punct::LParen) {
            let group = format_items(p)?;
            for _ in 0..repeat.unwrap_or(1) {
                items.extend(group.iter().copied());
            }
            continue;
        }
        let Some(TokenKind::Word(word)) = p.peek() else {
            return Err(p.unexpected(&["a format such as F8.2", "')'"]));
        };
        let upper = word.to_uppercase();
        let digits = upper.len() - upper.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        let (glued, rest) = upper.split_at(digits);
        let number = |text: &str| text.parse::<u32>().ok().filter(|n| *n >= 1);
        let repeat = number(glued).or(repeat);
        if let Some(n) = repeat.filter(|_| rest == "X") {
            items.push(FormatItem::Skip(n));
        } else if let Some(column) = rest.strip_prefix('T').and_then(number)
            && repeat.is_none()
        {
            items.push(FormatItem::Tab(column));
        } else {
            let format = output_format(p, rest)?;
            for _ in 0..repeat.unwrap_or(1) {
                items.push(FormatItem::Format(format));
            }
        }
        p.pos += 1;
    }
    Ok(items)
}

/// `first[-last] [(type [, decimals])]` after a variable list.
fn columns(p: &mut Parser) -> PResult<Placement> {
    let start = p.pos;
    let first = count(p, "a column")?;
    let last = if p.eat(Punct::Minus) {
        count(p, "a column")?
    } else {
        first
    };
    if last < first {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("column {last} comes before column {first}"),
        ));
    }
    let mut kind = None;
    let mut decimals = 0;
    if p.eat(Punct::LParen) {
        if let Some(TokenKind::Word(word)) = p.peek() {
            kind = Some(output_format(p, &format!("{word}40"))?.kind);
            p.pos += 1;
            p.eat(Punct::Comma);
        }
        if let Some(TokenKind::Number(n)) = p.peek() {
            if n.fract() != 0.0 || *n > 16.0 || *n < 0.0 {
                return Err(p.unexpected(&["a number of decimals", "')'"]));
            }
            decimals = *n as u8;
            p.pos += 1;
        }
        p.expect(Punct::RParen)?;
    }
    Ok(Placement::Columns {
        first,
        last,
        kind,
        decimals,
    })
}

/// The items of one line, up to the next slash or the end.
fn print_line(p: &mut Parser) -> PResult<Vec<PrintItem>> {
    let mut items = Vec::new();
    loop {
        match p.peek() {
            None | Some(TokenKind::Punct(Punct::Slash)) => return Ok(items),
            Some(TokenKind::Punct(Punct::Star)) => p.pos += 1,
            Some(TokenKind::Punct(Punct::Comma)) => p.pos += 1,
            Some(TokenKind::Str(_)) => {
                let text = p.string("a quoted text")?;
                let columns = match p.peek() {
                    Some(TokenKind::Number(_)) => {
                        let first = count(p, "a column")?;
                        let last = if p.eat(Punct::Minus) {
                            Some(count(p, "a column")?)
                        } else {
                            None
                        };
                        Some((first, last))
                    }
                    _ => None,
                };
                items.push(PrintItem::Text { text, columns });
            }
            Some(TokenKind::Word(_)) => {
                let vars = p.var_list()?;
                let placement = match p.peek() {
                    Some(TokenKind::Number(_)) => Some(columns(p)?),
                    Some(TokenKind::Punct(Punct::LParen)) => {
                        p.pos += 1;
                        Some(Placement::Formats(format_items(p)?))
                    }
                    _ => None,
                };
                items.push(PrintItem::Variables(vars, placement));
            }
            _ => {
                return Err(p.unexpected(&["a variable name", "a quoted text", "'*'", "'/'"]));
            }
        }
    }
}

fn print_command(p: &mut Parser, kind: PrintKind) -> PResult<Command> {
    let mut print = Print {
        kind,
        outfile: None,
        encoding: None,
        no_bom: false,
        records: None,
        table: false,
        lines: Vec::new(),
    };
    let mut known = vec!["OUTFILE", "ENCODING", "RECORDS", "TABLE", "NOTABLE"];
    if kind == PrintKind::Write {
        known.push("BOM");
    }
    while !p.at(Punct::Slash) {
        if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            print.outfile = Some(p.file("a quoted file path or a file handle")?);
        } else if p.eat_keyword("ENCODING") {
            p.eat(Punct::Eq);
            print.encoding = Some(p.string("a quoted encoding")?);
        } else if p.eat_keyword("RECORDS") {
            p.eat(Punct::Eq);
            p.eat(Punct::LParen);
            print.records = Some(count(p, "a number of lines")?);
            p.eat(Punct::RParen);
        } else if p.eat_keyword("TABLE") {
            print.table = true;
        } else if p.eat_keyword("NOTABLE") {
            print.table = false;
        } else if kind == PrintKind::Write && p.eat_keyword("BOM") {
            p.eat(Punct::Eq);
            if p.eat_keyword("NO") {
                print.no_bom = true;
            } else if p.eat_keyword("YES") {
                print.no_bom = false;
            } else {
                return Err(p.unexpected(&["YES", "NO"]));
            }
        } else {
            let mut expected = known.clone();
            expected.push("'/' and the variables");
            return Err(p.unexpected(&expected));
        }
    }
    while p.eat(Punct::Slash) {
        // the line number after the slash only documents the line
        if let Some(TokenKind::Number(_)) = p.peek() {
            count(p, "a line number")?;
        }
        print.lines.push(print_line(p)?);
    }
    p.finish(&["'/'"])?;
    Ok(Command::Print(Box::new(print)))
}

pub(crate) fn print(p: &mut Parser) -> PResult<Command> {
    print_command(p, PrintKind::Print)
}

pub(crate) fn print_eject(p: &mut Parser) -> PResult<Command> {
    print_command(p, PrintKind::PrintEject)
}

pub(crate) fn write(p: &mut Parser) -> PResult<Command> {
    print_command(p, PrintKind::Write)
}

/// `PRINT SPACE [OUTFILE=file] [ENCODING='name'] [expression]`
pub(crate) fn print_space(p: &mut Parser) -> PResult<Command> {
    let mut space = PrintSpace {
        outfile: None,
        encoding: None,
        lines: None,
    };
    loop {
        let keyword = |p: &Parser, k: &str| {
            p.at_keyword(k) && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq))
        };
        if keyword(p, "OUTFILE") {
            p.pos += 2;
            space.outfile = Some(p.file("a quoted file path or a file handle")?);
        } else if keyword(p, "ENCODING") {
            p.pos += 2;
            space.encoding = Some(p.string("a quoted encoding")?);
        } else {
            break;
        }
    }
    if p.peek().is_some() {
        space.lines = Some(p.expr()?);
    }
    p.finish(&[])?;
    Ok(Command::PrintSpace(space))
}

/// PRINT FORMATS and WRITE FORMATS, as FORMATS.
pub(crate) fn print_formats(p: &mut Parser) -> PResult<Command> {
    match crate::commands::formats(p)? {
        Command::Formats(groups) => Ok(Command::PrintFormats(groups)),
        _ => unreachable!("formats gives FORMATS"),
    }
}

pub(crate) fn write_formats(p: &mut Parser) -> PResult<Command> {
    match crate::commands::formats(p)? {
        Command::Formats(groups) => Ok(Command::WriteFormats(groups)),
        _ => unreachable!("formats gives FORMATS"),
    }
}

/// XSAVE OUTFILE='file' with the subcommands of SAVE, MAP and PERMISSIONS, in any order.
pub(crate) fn xsave(p: &mut Parser) -> PResult<Command> {
    let mut outfile = None;
    let mut xsave = Xsave {
        outfile: String::new(),
        compression: None,
        options: Vec::new(),
        map: false,
        read_only: None,
    };
    let known = [
        "OUTFILE",
        "KEEP",
        "DROP",
        "RENAME",
        "MAP",
        "COMPRESSED",
        "UNCOMPRESSED",
        "ZCOMPRESSED",
        "PERMISSIONS",
    ];
    loop {
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            outfile = Some(p.file("a quoted file path or a file handle")?);
        } else if let Some(option) = file_option(p)? {
            xsave.options.push(option);
        } else if p.eat_keyword("MAP") {
            xsave.map = true;
        } else if p.eat_keyword("COMPRESSED") {
            xsave.compression = Some(Compression::Compressed);
        } else if p.eat_keyword("UNCOMPRESSED") {
            xsave.compression = Some(Compression::Uncompressed);
        } else if p.eat_keyword("ZCOMPRESSED") {
            xsave.compression = Some(Compression::ZCompressed);
        } else if p.eat_keyword("PERMISSIONS") {
            p.eat(Punct::Eq);
            xsave.read_only = Some(read_only(p)?);
        } else {
            return Err(p.unexpected(&known));
        }
    }
    let Some(outfile) = outfile else {
        return Err(SyntaxError::new(p.span(), "XSAVE needs OUTFILE='file'"));
    };
    xsave.outfile = outfile;
    Ok(Command::Xsave(Box::new(xsave)))
}
