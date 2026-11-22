//! GET DATA /TYPE=TXT, XLSX, XLSM, XLS and OPENSTATSPEC.

use crate::ast::{
    Command, DatabaseKind, DatabaseSource, Format, FormatType, GetData, GetDataVariable, Sheet,
    Spreadsheet,
};
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

/// `IGNORE=NO | YES` after HIDDEN, LEADINGSPACES, MULTIPLESPACES or TRAILINGSPACES.
fn ignore(p: &mut Parser) -> PResult<bool> {
    if !p.eat_keyword("IGNORE") {
        return Err(p.unexpected(&["IGNORE"]));
    }
    p.eat(Punct::Eq);
    // the manual once writes HIDDEN IGNORE=(NO): parentheses are allowed
    let paren = p.eat(Punct::LParen);
    let yes = if p.eat_keyword("YES") {
        true
    } else if p.eat_keyword("NO") {
        false
    } else {
        return Err(p.unexpected(&["YES", "NO"]));
    };
    if paren {
        p.expect(Punct::RParen)?;
    }
    Ok(yes)
}

/// A cell such as B12 as (row, column), counted from 0.
fn cell(text: &str) -> Option<(u32, u32)> {
    let digits = text.find(|c: char| c.is_ascii_digit())?;
    let (letters, number) = text.split_at(digits);
    let letters_ok =
        (1..=3).contains(&letters.len()) && letters.chars().all(|c| c.is_ascii_alphabetic());
    if !letters_ok {
        return None;
    }
    let column = letters.bytes().fold(0u32, |n, b| {
        n * 26 + u32::from(b.to_ascii_uppercase() - b'A' + 1)
    });
    let row: u32 = number.parse().ok()?;
    ((1..=1_048_576).contains(&row) && column <= 16_384).then_some((row - 1, column - 1))
}

/// 'A1:K14' as (first row, first column, last row, last column).
pub(crate) fn cell_range(text: &str) -> Option<(u32, u32, u32, u32)> {
    let (from, to) = text.trim().split_once(':')?;
    let (r0, c0) = cell(from.trim())?;
    let (r1, c1) = cell(to.trim())?;
    (r0 <= r1 && c0 <= c1).then_some((r0, c0, r1, c1))
}

/// Subcommands only a text file takes, and only a workbook.
const TXT_ONLY: [&str; 10] = [
    "ENCODING",
    "ARRANGEMENT",
    "FIRSTCASE",
    "DELCASE",
    "FIXCASE",
    "DELIMITERS",
    "QUALIFIER",
    "IMPORTCASE",
    "MULTIPLESPACES",
    "VARIABLES",
];
const SHEET_ONLY: [&str; 6] = [
    "SHEET",
    "CELLRANGE",
    "READNAMES",
    "ASSUMEDSTRWIDTH",
    "HIDDEN",
    "TRAILINGSPACES",
];

/// Subcommands only a database takes.
const DATABASE_ONLY: [&str; 3] = ["DATABASE", "DATASET", "ALLOWLOSS"];

/// A server URL with a password in it, as `user:secret@host` or a password parameter.
fn holds_password(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let rest = lower.split_once("://").map_or(lower.as_str(), |(_, r)| r);
    let authority = rest.split(['/', '?']).next().unwrap_or("");
    let user = authority.rsplit_once('@').map(|(user, _)| user);
    user.is_some_and(|u| u.contains(':')) || lower.contains("password=")
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
        spreadsheet: None,
        database: None,
    };
    let mut database = None;
    let mut dataset = None;
    let mut allow_loss = false;
    let mut sheet = Spreadsheet {
        binary: false,
        sheet: Sheet::Index(1),
        range: None,
        read_names: true,
        assumed_width: None,
        ignore_hidden: false,
        ignore_trailing_spaces: false,
        type_min: None,
    };
    let mut kind = None;
    // each subcommand with where it starts, to check it against the type
    let mut seen: Vec<(&str, crate::Span)> = Vec::new();
    while p.eat(Punct::Slash) {
        let equals = |p: &mut Parser| p.eat(Punct::Eq);
        let at = p.span();
        if p.eat_keyword("TYPE") {
            equals(p);
            let Some(TokenKind::Word(w)) = p.peek() else {
                return Err(p.unexpected(&["TXT", "XLSX", "XLS", "OPENSTATSPEC"]));
            };
            kind = Some(w.to_uppercase());
            if let Some(other @ ("ODBC" | "OLEDB")) = kind.as_deref() {
                return Err(SyntaxError::new(
                    p.span(),
                    format!(
                        "GET DATA /TYPE={other} is not supported yet; save the table as text or as a workbook and read that"
                    ),
                ));
            }
            p.pos += 1;
            continue;
        }
        let name = if p.eat_keyword("FILE") {
            equals(p);
            command.file = p.file("a quoted file name or a file handle")?;
            "FILE"
        } else if p.eat_keyword("ENCODING") {
            equals(p);
            command.encoding = Some(p.string("a quoted encoding")?);
            "ENCODING"
        } else if p.eat_keyword("ARRANGEMENT") {
            equals(p);
            command.fixed = if p.eat_keyword("FIXED") {
                true
            } else if p.eat_keyword("DELIMITED") {
                false
            } else {
                return Err(p.unexpected(&["DELIMITED", "FIXED"]));
            };
            "ARRANGEMENT"
        } else if p.eat_keyword("FIRSTCASE") {
            equals(p);
            command.first_case = whole(p, "a line number from 1")?.max(1);
            "FIRSTCASE"
        } else if p.eat_keyword("DELCASE") {
            equals(p);
            if p.eat_keyword("LINE") {
                command.values_per_case = None;
            } else if p.eat_keyword("VARIABLES") {
                command.values_per_case = Some(whole(p, "a number of variables")?.max(1));
            } else {
                return Err(p.unexpected(&["LINE", "VARIABLES"]));
            }
            "DELCASE"
        } else if p.eat_keyword("FIXCASE") {
            equals(p);
            command.lines_per_case = whole(p, "a number of lines")?.max(1);
            "FIXCASE"
        } else if p.eat_keyword("DELIMITERS") {
            equals(p);
            command.delimiters = Some(delimiters(&p.string("quoted delimiters")?));
            "DELIMITERS"
        } else if p.eat_keyword("QUALIFIER") {
            equals(p);
            command.qualifier = Some(single_char(p, "the qualifier")?);
            "QUALIFIER"
        } else if p.eat_keyword("IMPORTCASE") {
            equals(p);
            if p.eat_keyword("FIRST") {
                command.first_cases = Some(u64::from(whole(p, "a number of cases")?));
            } else if !p.eat_word("ALL") {
                return Err(p.unexpected(&["ALL", "FIRST"]));
            }
            "IMPORTCASE"
        } else if p.eat_keyword("DATATYPEMIN") {
            if !p.eat_keyword("PERCENTAGE") {
                return Err(p.unexpected(&["PERCENTAGE"]));
            }
            equals(p);
            let start = p.pos;
            match p.number() {
                Some(n) if n > 50.0 && n <= 100.0 => sheet.type_min = Some(n),
                _ => {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "DATATYPEMIN PERCENTAGE must be more than 50 and at most 100",
                    ));
                }
            }
            "DATATYPEMIN"
        } else if p.eat_keyword("MULTIPLESPACES") {
            command.ignore_multiple_spaces = ignore(p)?;
            "MULTIPLESPACES"
        } else if p.eat_keyword("LEADINGSPACES") {
            command.ignore_leading_spaces = ignore(p)?;
            "LEADINGSPACES"
        } else if p.eat_keyword("TRAILINGSPACES") {
            sheet.ignore_trailing_spaces = ignore(p)?;
            "TRAILINGSPACES"
        } else if p.eat_keyword("HIDDEN") {
            sheet.ignore_hidden = ignore(p)?;
            "HIDDEN"
        } else if p.eat_keyword("SHEET") {
            equals(p);
            sheet.sheet = if p.eat_keyword("INDEX") {
                let start = p.pos;
                match whole(p, "a sheet number from 1")? {
                    0 => {
                        let span = p.span_from(start);
                        return Err(SyntaxError::new(span, "sheets are counted from 1"));
                    }
                    n => Sheet::Index(n),
                }
            } else if p.eat_keyword("NAME") {
                match p.peek() {
                    Some(TokenKind::Word(w)) => {
                        let name = w.clone();
                        p.pos += 1;
                        Sheet::Name(name)
                    }
                    _ => Sheet::Name(p.string("a quoted sheet name")?),
                }
            } else {
                return Err(p.unexpected(&["INDEX", "NAME"]));
            };
            "SHEET"
        } else if p.eat_keyword("CELLRANGE") {
            equals(p);
            sheet.range = if p.eat_keyword("FULL") {
                None
            } else if p.eat_keyword("RANGE") {
                let start = p.pos;
                let text = p.string("a quoted range such as 'A1:K14'")?;
                let range = cell_range(&text);
                Some(range.ok_or_else(|| {
                    let message = format!("'{text}' is not a range such as 'A1:K14'");
                    SyntaxError::new(p.span_from(start), message)
                })?)
            } else {
                return Err(p.unexpected(&["FULL", "RANGE"]));
            };
            "CELLRANGE"
        } else if p.eat_keyword("READNAMES") {
            equals(p);
            sheet.read_names = if p.eat_keyword("ON") {
                true
            } else if p.eat_keyword("OFF") {
                false
            } else {
                return Err(p.unexpected(&["ON", "OFF"]));
            };
            "READNAMES"
        } else if p.eat_keyword("ASSUMEDSTRWIDTH") {
            equals(p);
            let start = p.pos;
            match whole(p, "a width in bytes")? {
                n @ 1..=32_767 => sheet.assumed_width = Some(n),
                _ => {
                    let span = p.span_from(start);
                    return Err(SyntaxError::new(
                        span,
                        "ASSUMEDSTRWIDTH must be from 1 to 32767",
                    ));
                }
            }
            "ASSUMEDSTRWIDTH"
        } else if p.eat_keyword("VARIABLES") {
            equals(p);
            variables(p, &mut command)?;
            seen.push(("VARIABLES", at));
            // VARIABLES comes last; only MAP may follow it, as the text import wizard writes it
            while p.eat(Punct::Slash) {
                if !p.eat_keyword("MAP") {
                    return Err(p.unexpected(&["MAP"]));
                }
            }
            break;
        } else if p.eat_word("DATABASE") {
            equals(p);
            let kind = if p.eat_keyword("SQLITE") {
                DatabaseKind::Sqlite
            } else if p.eat_keyword("POSTGRES") {
                DatabaseKind::Postgres
            } else if p.eat_keyword("MYSQL") {
                DatabaseKind::Mysql
            } else if p.eat_keyword("DOLT") {
                DatabaseKind::Dolt
            } else {
                return Err(p.unexpected(&["SQLITE", "POSTGRES", "MYSQL", "DOLT"]));
            };
            let start = p.pos;
            let location = p.string("a quoted file name or URL")?;
            if kind != DatabaseKind::Sqlite && holds_password(&location) {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "syntax never holds a password: remove it from the URL and set the \
                     STATSCRAFT_DB_PASSWORD environment variable",
                ));
            }
            database = Some((kind, location));
            "DATABASE"
        } else if p.eat_word("DATASET") {
            equals(p);
            dataset = Some(p.file("a dataset name")?);
            "DATASET"
        } else if p.eat_keyword("ALLOWLOSS") {
            equals(p);
            allow_loss = if p.eat_keyword("YES") {
                true
            } else if p.eat_keyword("NO") {
                false
            } else {
                return Err(p.unexpected(&["YES", "NO"]));
            };
            "ALLOWLOSS"
        } else if p.eat_keyword("MAP") {
            // MAP only lists the variables read: accepted, nothing to do
            "MAP"
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
                "SHEET",
                "CELLRANGE",
                "READNAMES",
                "DATABASE",
                "DATASET",
            ]));
        };
        seen.push((name, at));
    }
    // a subcommand of the other kind of file, reported where it stands
    let misplaced = |wrong: &dyn Fn(&str) -> bool, kind: &str| {
        seen.iter().find(|(n, _)| wrong(n)).map(|(n, span)| {
            let message = format!("{n} does not apply to GET DATA /TYPE={kind}");
            SyntaxError::new(*span, message)
        })
    };
    match kind.as_deref() {
        Some("TXT") => {
            let wrong = |n: &str| SHEET_ONLY.contains(&n) || DATABASE_ONLY.contains(&n);
            if let Some(error) = misplaced(&wrong, "TXT") {
                return Err(error);
            }
        }
        Some(name @ ("XLS" | "XLSX" | "XLSM")) => {
            sheet.binary = name == "XLS";
            let mut wrong = TXT_ONLY.to_vec();
            wrong.extend(DATABASE_ONLY);
            if sheet.binary {
                // the manual: HIDDEN is not valid for TYPE=XLS
                wrong.push("HIDDEN");
            }
            if let Some(error) = misplaced(&|n| wrong.contains(&n), name) {
                return Err(error);
            }
            command.spreadsheet = Some(sheet);
        }
        Some("OPENSTATSPEC") => {
            if let Some(error) = misplaced(&|n| !DATABASE_ONLY.contains(&n), "OPENSTATSPEC") {
                return Err(error);
            }
            p.finish(&["'/'"])?;
            let Some((kind, location)) = database else {
                let message = "GET DATA /TYPE=OPENSTATSPEC needs /DATABASE=kind 'file or URL'";
                return Err(SyntaxError::new(p.span(), message));
            };
            let Some(dataset) = dataset else {
                let message = "GET DATA /TYPE=OPENSTATSPEC needs /DATASET='name'";
                return Err(SyntaxError::new(p.span(), message));
            };
            command.database = Some(DatabaseSource {
                kind,
                location,
                dataset,
                allow_loss,
            });
            return Ok(Command::GetData(command));
        }
        Some(other) => {
            return Err(SyntaxError::new(
                p.span(),
                format!("unknown GET DATA type {other}"),
            ));
        }
        None => {
            return Err(SyntaxError::new(
                p.span(),
                "GET DATA needs /TYPE=TXT, XLSX, XLS or OPENSTATSPEC",
            ));
        }
    }
    p.finish(&["'/'"])?;
    if command.file.is_empty() {
        return Err(SyntaxError::new(p.span(), "GET DATA needs /FILE='name'"));
    }
    if command.spreadsheet.is_none() && command.variables.is_empty() {
        return Err(SyntaxError::new(p.span(), "GET DATA needs /VARIABLES"));
    }
    Ok(Command::GetData(command))
}

/// `name format ...`, or for fixed data `[/n] name first-last format ...`.
fn variables(p: &mut Parser, command: &mut GetData) -> PResult<()> {
    let mut record = 1;
    loop {
        // a record number (`/2`), not the MAP subcommand after the list
        if command.fixed
            && p.peek() == Some(&TokenKind::Punct(Punct::Slash))
            && !matches!(p.peek_at(1), Some(TokenKind::Word(_)))
        {
            p.pos += 1;
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
