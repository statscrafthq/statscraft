//! Parsers of the input program commands: INPUT PROGRAM, END INPUT PROGRAM, END CASE,
//! END FILE, LEAVE, REREAD, REPEATING DATA, FILE TYPE, RECORD TYPE and END FILE TYPE.

use crate::ast::{
    Command, Duplicate, FileKind, FileType, IdField, Place, RecordType, RepeatingData,
};
use crate::data_list::{column_format, count, display_format, fixed_fields};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

pub(crate) fn input_program(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::InputProgram)
}

pub(crate) fn end_input_program(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::EndInputProgram)
}

pub(crate) fn end_case(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::EndCase)
}

pub(crate) fn end_file(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::EndFile)
}

pub(crate) fn end_file_type(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::EndFileType)
}

pub(crate) fn leave(p: &mut Parser) -> PResult<Command> {
    let variables = p.var_list()?;
    p.finish(&[])?;
    Ok(Command::Leave(variables))
}

pub(crate) fn reread(p: &mut Parser) -> PResult<Command> {
    let (mut file, mut column) = (None, None);
    while p.peek().is_some() {
        if p.eat(Punct::Slash) {
            continue;
        }
        if file.is_none() && p.eat_keyword("FILE") {
            p.eat(Punct::Eq);
            file = Some(p.file("a quoted file path or a file handle")?);
        } else if column.is_none() && p.eat_keyword("COLUMN") {
            p.eat(Punct::Eq);
            column = Some(p.expr()?);
        } else {
            return Err(p.unexpected(&["FILE", "COLUMN"]));
        }
    }
    Ok(Command::Reread { file, column })
}

/// ENCODING: only UTF-8 text is read yet.
fn encoding(p: &mut Parser) -> PResult<()> {
    p.eat(Punct::Eq);
    let start = p.pos;
    let encoding = p.string("a quoted encoding")?;
    if ["UTF8", "UTF-8"]
        .iter()
        .any(|u| u.eq_ignore_ascii_case(&encoding))
    {
        return Ok(());
    }
    Err(SyntaxError::new(
        p.span_from(start),
        "only UTF-8 text can be read yet",
    ))
}

/// A column number or a variable that holds it.
fn place(p: &mut Parser, what: &str) -> PResult<Place> {
    match p.peek() {
        Some(TokenKind::Word(_)) => Ok(Place::Variable(p.name("a variable name")?)),
        _ => Ok(Place::Column(count(p, what)?)),
    }
}

/// `first[-last]` as numbers.
fn columns(p: &mut Parser) -> PResult<(u32, u32)> {
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
    Ok((first, last))
}

pub(crate) fn repeating_data(p: &mut Parser) -> PResult<Command> {
    let mut file = None;
    let (mut starts, mut ends, mut occurs, mut length) = (None, None, None, None);
    let (mut continued, mut id) = (None, None);
    let mut data = None;
    loop {
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        if p.eat_keyword("FILE") {
            p.eat(Punct::Eq);
            file = Some(p.file("a quoted file path or a file handle")?);
        } else if p.eat_keyword("ENCODING") {
            encoding(p)?;
        } else if p.eat_keyword("STARTS") {
            p.eat(Punct::Eq);
            starts = Some(place(p, "a column")?);
            if p.eat(Punct::Minus) {
                ends = Some(place(p, "a column")?);
            }
        } else if p.eat_keyword("OCCURS") {
            p.eat(Punct::Eq);
            occurs = Some(place(p, "a number of groups")?);
        } else if p.eat_keyword("LENGTH") {
            p.eat(Punct::Eq);
            length = Some(place(p, "a number of columns")?);
        } else if p.eat_keyword("CONTINUED") {
            continued = Some(if p.eat(Punct::Eq) {
                let first = count(p, "a column")?;
                let last = if p.eat(Punct::Minus) {
                    Some(count(p, "a column")?)
                } else {
                    None
                };
                (first, last)
            } else {
                (1, None)
            });
        } else if p.eat_keyword("ID") {
            p.eat(Punct::Eq);
            let (first, last) = columns(p)?;
            p.expect(Punct::Eq)?;
            id = Some((first, last, p.name("a variable name")?));
        } else if p.eat_keyword("TABLE") || p.eat_keyword("NOTABLE") {
        } else if p.eat_keyword("DATA") {
            p.eat(Punct::Eq);
            // DATA is the last subcommand: its variables run to the end of the command
            let (fields, reach) = fixed_fields(p, 1, 1)?;
            if fields.is_empty() {
                return Err(p.unexpected(&["a variable name"]));
            }
            data = Some((fields, reach));
            break;
        } else {
            return Err(p.unexpected(&[
                "FILE",
                "ENCODING",
                "STARTS",
                "OCCURS",
                "LENGTH",
                "CONTINUED",
                "ID",
                "TABLE",
                "NOTABLE",
                "DATA",
            ]));
        }
    }
    let missing = |what: &str| SyntaxError::new(p.span(), format!("REPEATING DATA needs {what}"));
    let starts = starts.ok_or_else(|| missing("STARTS"))?;
    let occurs = occurs.ok_or_else(|| missing("OCCURS"))?;
    let (fields, group_length) = data.ok_or_else(|| missing("DATA, as its last subcommand"))?;
    if id.is_some() && continued.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "ID is for continuation records: give CONTINUED too",
        ));
    }
    Ok(Command::RepeatingData(Box::new(RepeatingData {
        file,
        starts,
        ends,
        occurs,
        length,
        continued,
        id,
        fields,
        group_length,
    })))
}

/// `[name] first[-last] [(format)]` of RECORD and CASE on FILE TYPE.
fn id_field(p: &mut Parser) -> PResult<IdField> {
    let name = match p.peek() {
        Some(TokenKind::Word(_)) => Some(p.name("a variable name")?),
        _ => None,
    };
    let (first, last) = columns(p)?;
    let width = (last - first + 1).min(32767) as u16;
    let format = column_format(p, width)?;
    Ok(IdField {
        name,
        column: first,
        format,
        display: display_format(format),
    })
}

/// WARN or NOWARN: true for WARN.
fn warn(p: &mut Parser) -> PResult<bool> {
    p.eat(Punct::Eq);
    if p.eat_keyword("WARN") {
        Ok(true)
    } else if p.eat_keyword("NOWARN") {
        Ok(false)
    } else {
        Err(p.unexpected(&["WARN", "NOWARN"]))
    }
}

fn duplicate(p: &mut Parser) -> PResult<Duplicate> {
    p.eat(Punct::Eq);
    if p.eat_keyword("WARN") {
        Ok(Duplicate::Warn)
    } else if p.eat_keyword("NOWARN") {
        Ok(Duplicate::NoWarn)
    } else if p.eat_keyword("CASE") {
        Ok(Duplicate::Case)
    } else {
        Err(p.unexpected(&["WARN", "NOWARN", "CASE"]))
    }
}

fn yes_no(p: &mut Parser) -> PResult<bool> {
    p.eat(Punct::Eq);
    if p.eat_keyword("YES") {
        Ok(true)
    } else if p.eat_keyword("NO") {
        Ok(false)
    } else {
        Err(p.unexpected(&["YES", "NO"]))
    }
}

pub(crate) fn file_type(p: &mut Parser) -> PResult<Command> {
    let kind = if p.eat_keyword("MIXED") {
        FileKind::Mixed
    } else if p.eat_keyword("GROUPED") {
        FileKind::Grouped
    } else if p.eat_keyword("NESTED") {
        FileKind::Nested
    } else {
        return Err(p.unexpected(&["MIXED", "GROUPED", "NESTED"]));
    };
    let mut file = None;
    let (mut record, mut case) = (None, None);
    let (mut wild, mut dup, mut missing, mut ordered) = (None, None, None, None);
    let not_for = |p: &Parser, what: &str, kinds: &str| {
        SyntaxError::new(p.span(), format!("{what} is for {kinds} files only"))
    };
    while p.peek().is_some() {
        if p.eat(Punct::Slash) {
            continue;
        }
        if p.eat_keyword("FILE") {
            p.eat(Punct::Eq);
            file = Some(p.file("a quoted file path or a file handle")?);
        } else if p.eat_keyword("ENCODING") {
            encoding(p)?;
        } else if p.eat_keyword("RECORD") {
            p.eat(Punct::Eq);
            record = Some(id_field(p)?);
        } else if p.at_keyword("CASE") {
            if kind == FileKind::Mixed {
                return Err(not_for(p, "CASE", "grouped and nested"));
            }
            p.pos += 1;
            p.eat(Punct::Eq);
            case = Some(id_field(p)?);
        } else if p.eat_keyword("WILD") {
            wild = Some(warn(p)?);
        } else if p.at_keyword("DUPLICATE") {
            if kind == FileKind::Mixed {
                return Err(not_for(p, "DUPLICATE", "grouped and nested"));
            }
            p.pos += 1;
            let start = p.pos;
            let value = duplicate(p)?;
            if value == Duplicate::Case && kind != FileKind::Nested {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "DUPLICATE=CASE is for nested files only",
                ));
            }
            dup = Some(value);
        } else if p.at_keyword("MISSING") {
            if kind == FileKind::Mixed {
                return Err(not_for(p, "MISSING", "grouped and nested"));
            }
            p.pos += 1;
            missing = Some(warn(p)?);
        } else if p.at_keyword("ORDERED") {
            if kind != FileKind::Grouped {
                return Err(not_for(p, "ORDERED", "grouped"));
            }
            p.pos += 1;
            ordered = Some(yes_no(p)?);
        } else {
            return Err(p.unexpected(&[
                "FILE",
                "ENCODING",
                "RECORD",
                "CASE",
                "WILD",
                "DUPLICATE",
                "MISSING",
                "ORDERED",
            ]));
        }
    }
    let Some(record) = record else {
        return Err(SyntaxError::new(p.span(), "FILE TYPE needs RECORD"));
    };
    if kind == FileKind::Grouped && case.is_none() {
        return Err(SyntaxError::new(p.span(), "FILE TYPE GROUPED needs CASE"));
    }
    Ok(Command::FileType(Box::new(FileType {
        kind,
        file,
        record,
        case,
        wild,
        duplicate: dup,
        missing,
        ordered,
    })))
}

/// The checks that need the FILE TYPE come when the program runs.
pub(crate) fn record_type(p: &mut Parser) -> PResult<Command> {
    let mut values = Vec::new();
    if !p.eat_keyword("OTHER") {
        while let Some(value) = p.value() {
            values.push(value);
            p.eat(Punct::Comma);
        }
        if values.is_empty() {
            return Err(p.unexpected(&["a record type value", "OTHER"]));
        }
    }
    let mut record = RecordType {
        values,
        skip: false,
        case: None,
        duplicate: None,
        missing: None,
        spread: true,
    };
    while p.peek().is_some() {
        if p.eat(Punct::Slash) {
            continue;
        }
        if p.eat_keyword("SKIP") {
            record.skip = true;
        } else if p.eat_keyword("CASE") {
            p.eat(Punct::Eq);
            record.case = Some(columns(p)?);
        } else if p.eat_keyword("DUPLICATE") {
            let start = p.pos;
            record.duplicate = Some(duplicate(p)?);
            if record.duplicate == Some(Duplicate::Case) {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "DUPLICATE=CASE belongs on FILE TYPE NESTED, not on RECORD TYPE",
                ));
            }
        } else if p.eat_keyword("MISSING") {
            record.missing = Some(warn(p)?);
        } else if p.eat_keyword("SPREAD") {
            record.spread = yes_no(p)?;
        } else {
            return Err(p.unexpected(&["SKIP", "CASE", "DUPLICATE", "MISSING", "SPREAD"]));
        }
    }
    Ok(Command::RecordType(record))
}
