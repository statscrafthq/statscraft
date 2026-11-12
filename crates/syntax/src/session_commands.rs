//! Parsers of the commands about the session rather than the data: macros, inserted files,
//! settings, datasets and listings.

use crate::ast::{Command, DatasetCommand, Display, List, UseRange};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::table::matches_keyword;

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
    let file = p.file("a quoted file path or a file handle")?;
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

/// What a SET subcommand does in Statscraft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingEffect {
    /// Kept and followed.
    Used,
    /// About speed, memory or where messages go: accepted, nothing to do.
    Silent,
    /// Only changes how output looks: accepted with a warning.
    Appearance,
}

use SettingEffect::{Appearance, Silent, Used};

/// Every SET subcommand with its default as SHOW prints it.
pub const SETTINGS: &[(&str, &str, SettingEffect)] = &[
    ("BASETEXTDIRECTION", "AUTOMATIC", Appearance),
    ("BLANKS", "SYSMIS", Used),
    ("BLOCK", "X'2A'", Appearance),
    ("BOX", "X'2D7C2B2B2B2B2B2B2B2B2B'", Appearance),
    ("CACHE", "20", Silent),
    ("CCA", "-,,,", Used),
    ("CCB", "-,,,", Used),
    ("CCC", "-,,,", Used),
    ("CCD", "-,,,", Used),
    ("CCE", "-,,,", Used),
    ("CELLSBREAK", "10000", Appearance),
    ("CMPTRANS", "NO", Silent),
    ("CTEMPLATE", "NONE", Appearance),
    ("DECIMAL", "DOT", Used),
    ("DEFOLANG", "ENGLISH", Appearance),
    ("DIGITGROUPING", "NO", Appearance),
    ("EPOCH", "AUTOMATIC", Used),
    ("ERRORS", "LISTING", Silent),
    ("FOOTNOTE", "YES", Appearance),
    ("FORMAT", "F8.2", Used),
    ("FUZZBITS", "6", Used),
    ("HEADER", "NO", Appearance),
    ("JOURNAL", "", Silent),
    ("LEADZERO", "NO", Appearance),
    ("LENGTH", "59", Appearance),
    ("LOCALE", "OSLOCALE", Used),
    ("MCACHE", "", Silent),
    ("MDISPLAY", "TEXT", Appearance),
    ("MESSAGES", "NONE", Silent),
    ("MEXPAND", "ON", Used),
    (
        "MIOUTPUT",
        "[OBSERVED IMPUTED POOLED DIAGNOSTICS]",
        Appearance,
    ),
    ("MITERATE", "1000", Used),
    ("MNEST", "50", Used),
    ("MPRINT", "OFF", Used),
    ("MTINDEX", "2000000", Used),
    ("MXCELLS", "AUTOMATIC", Silent),
    ("MXERRS", "100", Silent),
    ("MXLOOPS", "40", Used),
    ("MXWARNS", "10", Silent),
    ("OATTRS", "OLANG", Appearance),
    ("ODISPLAY", "MODELVIEWER", Appearance),
    ("OLANG", "ENGLISH", Appearance),
    ("ONUMBERS", "LABELS", Used),
    ("OVARS", "LABELS", Used),
    ("PRINTBACK", "NONE", Used),
    ("REPDEFER", "NO", Silent),
    ("RESULTS", "LISTING", Silent),
    ("RNG", "MC", Used),
    ("ROWSBREAK", "100", Appearance),
    ("SCALEMIN", "24", Used),
    ("SEED", "2000000", Used),
    ("SIGLESS", "ON", Appearance),
    ("SMALL", "0", Appearance),
    ("SORT", "EXTERNAL", Silent),
    ("SUMMARY", "NONE", Appearance),
    ("TABLERENDER", "FAST", Silent),
    ("TFIT", "BOTH", Appearance),
    ("THREADS", "AUTO", Silent),
    ("TLOOK", "NONE", Appearance),
    ("TNUMBERS", "LABELS", Used),
    ("TOLERANCE", "1", Appearance),
    ("TVARS", "LABELS", Used),
    ("UNDEFINED", "WARN", Silent),
    ("UNICODE", "ON", Used),
    ("WIDTH", "80", Appearance),
    ("WORKSPACE", "24576", Silent),
    ("XVERSION", "DEFAULT", Silent),
    ("ZCOMPRESSION", "YES", Silent),
];

/// What SHOW can show beyond the SET subcommands.
const SHOW_ONLY: [&str; 18] = [
    "ALL",
    "CC",
    "DIRECTORY",
    "ENVIRONMENT",
    "EXTENSIONS",
    "EXTPATHS",
    "FILTER",
    "HANDLES",
    "LICENSE",
    "MXMEMORY",
    "N",
    "PLUGINS",
    "SCOMPRESSION",
    "SPLIT",
    "SYSMIS",
    "TMSRECORDING",
    "VERSION",
    "WEIGHT",
];

/// The SET subcommand a word names, written in full or shortened to three or more letters.
fn setting(word: &str) -> Option<&'static str> {
    let exact = SETTINGS.iter().find(|s| s.0.eq_ignore_ascii_case(word));
    exact
        .or_else(|| SETTINGS.iter().find(|s| matches_keyword(word, s.0)))
        .map(|s| s.0)
}

pub(crate) fn set(p: &mut Parser) -> PResult<Command> {
    let mut settings = Vec::new();
    loop {
        p.eat(Punct::Slash);
        let Some(TokenKind::Word(word)) = p.peek() else {
            break;
        };
        let Some(name) = setting(word) else {
            let error = SyntaxError::new(
                p.span(),
                format!("unknown SET subcommand {}", word.to_uppercase()),
            );
            return Err(error);
        };
        p.pos += 1;
        p.eat(Punct::Eq);
        let value = if p.eat(Punct::LBracket) {
            let mut words = Vec::new();
            while let Some(TokenKind::Word(w)) = p.peek() {
                words.push(w.to_uppercase());
                p.pos += 1;
            }
            p.expect(Punct::RBracket)?;
            format!("[{}]", words.join(" "))
        } else {
            match p.peek() {
                Some(TokenKind::Word(w)) => {
                    let w = w.clone();
                    p.pos += 1;
                    // a Windows code page locale such as 'Estonian_Estonia.1257' is one word
                    w
                }
                Some(TokenKind::Str(_)) => p.string("a quoted value")?,
                _ => match p.number() {
                    Some(n) => format!("{n}"),
                    None => return Err(p.unexpected(&[&format!("a value for {name}")])),
                },
            }
        };
        settings.push((name.to_string(), value));
    }
    if settings.is_empty() {
        return Err(p.unexpected(&["a SET subcommand such as FORMAT or MPRINT"]));
    }
    p.finish(&["a SET subcommand"])?;
    Ok(Command::Set(settings))
}

pub(crate) fn show(p: &mut Parser) -> PResult<Command> {
    let mut names = Vec::new();
    loop {
        p.eat(Punct::Slash);
        let Some(TokenKind::Word(word)) = p.peek() else {
            break;
        };
        let name = setting(word)
            .or_else(|| {
                SHOW_ONLY
                    .iter()
                    .copied()
                    .find(|s| s.eq_ignore_ascii_case(word))
            })
            .or_else(|| word.eq_ignore_ascii_case("$VARS").then_some("$VARS"));
        let Some(name) = name else {
            return Err(SyntaxError::new(
                p.span(),
                format!("unknown SHOW keyword {}", word.to_uppercase()),
            ));
        };
        p.pos += 1;
        names.push(name.to_string());
    }
    p.finish(&["a setting name"])?;
    Ok(Command::Show(names))
}

/// The words after PRESERVE or RESTORE, which the manual does not provide for.
fn words(p: &mut Parser) -> PResult<Vec<String>> {
    let mut words = Vec::new();
    while let Some(TokenKind::Word(w)) = p.peek() {
        words.push(w.to_uppercase());
        p.pos += 1;
    }
    p.finish(&[])?;
    Ok(words)
}

pub(crate) fn preserve(p: &mut Parser) -> PResult<Command> {
    Ok(Command::Preserve(words(p)?))
}

pub(crate) fn restore(p: &mut Parser) -> PResult<Command> {
    Ok(Command::Restore(words(p)?))
}

pub(crate) fn cache(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::Cache)
}

pub(crate) fn cd(p: &mut Parser) -> PResult<Command> {
    let folder = p.file("a quoted folder or a file handle")?;
    p.finish(&[])?;
    Ok(Command::Cd(folder))
}

pub(crate) fn file_handle(p: &mut Parser) -> PResult<Command> {
    let name = p.name("a handle name")?;
    let (mut path, mut encoding, mut ignored) = (None, None, Vec::new());
    loop {
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        let start = p.pos;
        if p.eat_keyword("NAME") {
            p.eat(Punct::Eq);
            path = Some(p.string("a quoted path")?);
        } else if p.eat_keyword("ENCODING") {
            p.eat(Punct::Eq);
            encoding = Some(p.string("a quoted encoding")?);
        } else if p.eat_keyword("MODE") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("CHARACTER") {
                let mode = match p.peek() {
                    Some(TokenKind::Word(w)) => w.to_uppercase(),
                    Some(TokenKind::Number(n)) => n.to_string(),
                    _ => return Err(p.unexpected(&["CHARACTER"])),
                };
                let known = ["BINARY", "MULTIPUNCH", "IMAGE", "360"];
                if !known.iter().any(|k| matches_keyword(&mode, k)) {
                    return Err(p.unexpected(&[
                        "CHARACTER",
                        "BINARY",
                        "MULTIPUNCH",
                        "IMAGE",
                        "360",
                    ]));
                }
                let message = format!(
                    "FILE HANDLE /MODE={mode} is for binary, column binary and EBCDIC files, which \
                     Statscraft does not read; text files are MODE=CHARACTER"
                );
                return Err(SyntaxError::new(p.span(), message));
            }
        } else if p.eat_keyword("RECFORM") {
            p.eat(Punct::Eq);
            let forms = ["FIXED", "VARIABLE", "SPANNED", "F", "V", "VS"];
            if !forms.iter().any(|f| p.eat_word(f) || p.eat_keyword(f)) {
                return Err(p.unexpected(&["FIXED", "VARIABLE", "SPANNED"]));
            }
            ignored.push("RECFORM".to_string());
        } else if p.eat_keyword("LRECL") {
            p.eat(Punct::Eq);
            if p.number().is_none() {
                return Err(p.unexpected(&["a record length"]));
            }
            ignored.push("LRECL".to_string());
        } else {
            p.pos = start;
            return Err(p.subcommand_error("FILE HANDLE", &[]));
        }
    }
    let Some(path) = path else {
        return Err(p.unexpected(&["/NAME='path'"]));
    };
    Ok(Command::FileHandle {
        name,
        path,
        encoding,
        ignored,
    })
}

/// `[WINDOW=keyword]` after a dataset name.
fn window(p: &mut Parser, allowed: &[&str]) -> PResult<()> {
    if p.eat_keyword("WINDOW") {
        keyword_value(p, allowed)?;
    }
    p.finish(&["WINDOW"])
}

fn dataset_name(p: &mut Parser) -> PResult<String> {
    p.name("a dataset name")
}

pub(crate) fn dataset_name_command(p: &mut Parser) -> PResult<Command> {
    let name = dataset_name(p)?;
    window(p, &["ASIS", "FRONT"])?;
    Ok(Command::Dataset(DatasetCommand::Name(name)))
}

pub(crate) fn dataset_activate(p: &mut Parser) -> PResult<Command> {
    let name = dataset_name(p)?;
    window(p, &["ASIS", "FRONT"])?;
    Ok(Command::Dataset(DatasetCommand::Activate(name)))
}

pub(crate) fn dataset_copy(p: &mut Parser) -> PResult<Command> {
    let name = dataset_name(p)?;
    window(p, &["MINIMIZED", "HIDDEN", "FRONT"])?;
    Ok(Command::Dataset(DatasetCommand::Copy(name)))
}

pub(crate) fn dataset_declare(p: &mut Parser) -> PResult<Command> {
    let name = dataset_name(p)?;
    window(p, &["MINIMIZED", "HIDDEN", "FRONT"])?;
    Ok(Command::Dataset(DatasetCommand::Declare(name)))
}

pub(crate) fn dataset_close(p: &mut Parser) -> PResult<Command> {
    let command = if p.eat(Punct::Star) {
        DatasetCommand::Close(None)
    } else if p.eat_word("ALL") {
        DatasetCommand::CloseAll
    } else {
        DatasetCommand::Close(Some(dataset_name(p)?))
    };
    p.finish(&[])?;
    Ok(Command::Dataset(command))
}

pub(crate) fn dataset_display(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::Dataset(DatasetCommand::Display))
}

/// `USE ALL`, `USE THRU`, `USE [first] [THRU [last]] [/PERMANENT]` with case numbers.
pub(crate) fn use_command(p: &mut Parser) -> PResult<Command> {
    if p.eat_word("ALL") {
        p.finish(&[])?;
        return Ok(Command::Use(None));
    }
    let case = |p: &mut Parser| -> PResult<Option<u64>> {
        if p.eat_keyword("FIRST") || p.eat_keyword("LAST") {
            return Ok(None);
        }
        match p.number() {
            Some(n) if n >= 1.0 && n.fract() == 0.0 => Ok(Some(n as u64)),
            Some(_) => Err(p.unexpected(&["a case number from 1"])),
            None => Ok(None),
        }
    };
    if matches!(p.peek(), Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case("THRU") && !w.eq_ignore_ascii_case("FIRST"))
    {
        return Err(SyntaxError::new(
            p.span(),
            "USE with dates is not supported yet; give case numbers or ALL",
        ));
    }
    let first = case(p)?;
    let last = if p.eat_keyword("THRU") {
        case(p)?
    } else {
        None
    };
    let permanent = p.eat(Punct::Slash) && {
        if !p.eat_keyword("PERMANENT") {
            return Err(p.unexpected(&["PERMANENT"]));
        }
        true
    };
    p.finish(&["THRU", "/PERMANENT"])?;
    if first.is_none() && last.is_none() && !permanent {
        return Ok(Command::Use(None));
    }
    Ok(Command::Use(Some(UseRange {
        first,
        last,
        permanent,
    })))
}

const DISPLAY_KEYWORDS: [&str; 11] = [
    "NAMES",
    "INDEX",
    "VARIABLES",
    "LABELS",
    "DICTIONARY",
    "ATTRIBUTES",
    "SCRATCH",
    "VECTOR",
    "MACROS",
    "DOCUMENTS",
    "FILE",
];

pub(crate) fn display(p: &mut Parser) -> PResult<Command> {
    let sorted = p.eat_keyword("SORTED");
    let mut what = "NAMES".to_string();
    if p.at(Punct::Slash) || p.peek().is_none() {
        // DISPLAY alone, or with /VARIABLES only
    } else if matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("@ATTRIBUTES"))
    {
        p.pos += 1;
        what = "@ATTRIBUTES".into();
    } else {
        let found = DISPLAY_KEYWORDS.iter().find(|k| p.at_keyword(k));
        let Some(found) = found else {
            return Err(p.unexpected(&DISPLAY_KEYWORDS));
        };
        p.pos += 1;
        what = found.to_string();
        if what == "FILE" {
            // DISPLAY FILE LABEL, a form some scripts use
            if !p.eat_keyword("LABEL") {
                return Err(p.unexpected(&["LABEL"]));
            }
            what = "FILE LABEL".into();
        }
    }
    let mut variables = None;
    let slash = p.eat(Punct::Slash);
    if (slash || what != "VARIABLES") && (p.eat_keyword("VARIABLES") || p.eat_keyword("NAMES")) {
        p.eat(Punct::Eq);
        variables = Some(p.var_list()?);
    } else if slash {
        return Err(p.unexpected(&["VARIABLES"]));
    } else if what == "VARIABLES" && p.eat(Punct::Eq) {
        // DISPLAY VARIABLES=varlist: the VARIABLES subcommand without its slash
        what = "NAMES".into();
        variables = Some(p.var_list()?);
    }
    p.finish(&["/VARIABLES"])?;
    Ok(Command::Display(Display {
        what,
        sorted,
        variables,
    }))
}

pub(crate) fn list(p: &mut Parser) -> PResult<Command> {
    let mut list = List {
        variables: None,
        from: 1,
        to: None,
        by: 1,
        numbered: false,
    };
    let count = |p: &mut Parser| -> PResult<u64> {
        match p.number() {
            Some(n) if n >= 1.0 && n.fract() == 0.0 => Ok(n as u64),
            _ => Err(p.unexpected(&["a whole number from 1"])),
        }
    };
    let mut first = true;
    loop {
        let slash = p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        if p.eat_keyword("VARIABLES") {
            p.eat(Punct::Eq);
            list.variables = Some(p.var_list()?);
        } else if p.eat_keyword("CASES") {
            p.eat(Punct::Eq);
            if let Some(TokenKind::Number(_)) = p.peek() {
                list.to = Some(count(p)?);
            }
            loop {
                if p.eat_keyword("FROM") {
                    list.from = count(p)?;
                } else if p.eat_word("TO") {
                    list.to = Some(count(p)?);
                } else if p.eat_word("BY") {
                    list.by = count(p)?;
                } else {
                    break;
                }
            }
        } else if p.eat_keyword("FORMAT") {
            p.eat(Punct::Eq);
            loop {
                if p.eat_keyword("NUMBERED") {
                    list.numbered = true;
                } else if p.eat_keyword("UNNUMBERED") {
                    list.numbered = false;
                } else if !(p.eat_keyword("WRAP") || p.eat_keyword("SINGLE")) {
                    break;
                }
            }
        } else if first && !slash && matches!(p.peek(), Some(TokenKind::Word(_))) {
            // LIST varlist: the VARIABLES keyword left out
            list.variables = Some(p.var_list()?);
        } else {
            return Err(p.unexpected(&["VARIABLES", "CASES", "FORMAT"]));
        }
        first = false;
    }
    p.finish(&[])?;
    Ok(Command::List(list))
}
