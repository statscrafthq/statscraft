//! Parser of CODEBOOK: the dictionary entry and summary statistics of variables and multiple
//! response sets.
//!
//! Rules from the command's description:
//! - An optional list of variables and sets comes first: names, `a TO b`, ALL (every
//!   variable), `$name` and $ALL (every set). TO never joins sets. A name or TO list may be
//!   followed by `[N]`, `[O]` or `[S]`, the level its statistics use (for a TO list: its
//!   last variable); ALL and $ALL take none. A variable or set may be named once only.
//! - Subcommands in any order, each at most once: VARINFO, FILEINFO, STATISTICS, OPTIONS.
//! - VARINFO: POSITION LABEL TYPE FORMAT MEASURE ROLE ATTRIBUTES VALUELABELS MISSING
//!   RESERVEDATTRIBUTES; left out, everything but RESERVEDATTRIBUTES.
//! - FILEINFO: NAME LOCATION CASECOUNT LABEL ATTRIBUTES DOCUMENTS WEIGHT RESERVEDATTRIBUTES;
//!   left out, no file table.
//! - STATISTICS: COUNT PERCENT MEAN STDDEV QUARTILES, or NONE alone; left out, all.
//! - OPTIONS: MAXCATS=n (200), VARORDER=FILE|ALPHA|VARLIST|MEASURE|ATTRIBUTE(name),
//!   SORT=ASCENDING|DESCENDING. VARORDER defaults to VARLIST with a list, else FILE.

use crate::ast::{Command, Level, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::{at_end, keywords, names};

/// A variable, TO list, ALL, set or $ALL, with the level its statistics use.
#[derive(Debug, Clone, PartialEq)]
pub struct CodebookItem {
    /// A set is a `Name` starting with "$"; $ALL is `Name("$ALL")`.
    pub item: VarItem,
    pub level: Option<Level>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodebookInfo {
    Position,
    Label,
    Type,
    Format,
    Measure,
    Role,
    Attributes,
    ValueLabels,
    Missing,
    ReservedAttributes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodebookFile {
    Name,
    Location,
    CaseCount,
    Label,
    Attributes,
    Documents,
    Weight,
    ReservedAttributes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodebookStatistic {
    Count,
    Percent,
    Mean,
    StdDev,
    Quartiles,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CodebookOrder {
    File,
    Alpha,
    Varlist,
    Measure,
    /// ATTRIBUTE(name): by the value of that custom attribute.
    Attribute(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Codebook {
    /// Empty: every variable, then every set.
    pub items: Vec<CodebookItem>,
    pub varinfo: Vec<CodebookInfo>,
    pub fileinfo: Vec<CodebookFile>,
    /// Empty for NONE.
    pub statistics: Vec<CodebookStatistic>,
    pub maxcats: usize,
    /// None: VARLIST with a list, else FILE.
    pub order: Option<CodebookOrder>,
    pub descending: bool,
}

const SUBCOMMANDS: [&str; 4] = ["VARINFO", "FILEINFO", "STATISTICS", "OPTIONS"];

const VARINFO: [(&str, &[CodebookInfo]); 10] = [
    ("POSITION", &[CodebookInfo::Position]),
    ("LABEL", &[CodebookInfo::Label]),
    ("TYPE", &[CodebookInfo::Type]),
    ("FORMAT", &[CodebookInfo::Format]),
    ("MEASURE", &[CodebookInfo::Measure]),
    ("ROLE", &[CodebookInfo::Role]),
    ("ATTRIBUTES", &[CodebookInfo::Attributes]),
    ("VALUELABELS", &[CodebookInfo::ValueLabels]),
    ("MISSING", &[CodebookInfo::Missing]),
    ("RESERVEDATTRIBUTES", &[CodebookInfo::ReservedAttributes]),
];

const FILEINFO: [(&str, &[CodebookFile]); 8] = [
    ("NAME", &[CodebookFile::Name]),
    ("LOCATION", &[CodebookFile::Location]),
    ("CASECOUNT", &[CodebookFile::CaseCount]),
    ("LABEL", &[CodebookFile::Label]),
    ("ATTRIBUTES", &[CodebookFile::Attributes]),
    ("DOCUMENTS", &[CodebookFile::Documents]),
    ("WEIGHT", &[CodebookFile::Weight]),
    ("RESERVEDATTRIBUTES", &[CodebookFile::ReservedAttributes]),
];

const STATISTICS: [(&str, &[CodebookStatistic]); 6] = [
    ("COUNT", &[CodebookStatistic::Count]),
    ("PERCENT", &[CodebookStatistic::Percent]),
    ("MEAN", &[CodebookStatistic::Mean]),
    ("STDDEV", &[CodebookStatistic::StdDev]),
    ("QUARTILES", &[CodebookStatistic::Quartiles]),
    ("NONE", &[]),
];

/// `[N]`, `[O]` or `[S]` after an item, if there.
fn level(p: &mut Parser) -> PResult<Option<Level>> {
    if !p.eat(Punct::LBracket) {
        return Ok(None);
    }
    let level = match p.peek() {
        Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("N") => Level::Nominal,
        Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("O") => Level::Ordinal,
        Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("S") => Level::Scale,
        _ => return Err(p.unexpected(&["N", "O", "S"])),
    };
    p.pos += 1;
    p.expect(Punct::RBracket)?;
    Ok(Some(level))
}

fn items(p: &mut Parser) -> PResult<Vec<CodebookItem>> {
    let mut items = Vec::new();
    while let Some(TokenKind::Word(_)) = p.peek() {
        let start = p.pos;
        let item = if p.eat_word("ALL") {
            VarItem::All
        } else {
            let first = p.name("a variable or multiple response set")?;
            if p.eat_word("TO") {
                let last = p.name("a variable name")?;
                if first.starts_with('$') || last.starts_with('$') {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "TO lists variables only, not multiple response sets",
                    ));
                }
                VarItem::Range(first, last)
            } else {
                VarItem::Name(first)
            }
        };
        let everything = match &item {
            VarItem::All => true,
            VarItem::Name(name) => name.eq_ignore_ascii_case("$ALL"),
            VarItem::Range(..) => false,
        };
        let at = p.pos;
        let level = level(p)?;
        if everything && level.is_some() {
            return Err(SyntaxError::new(
                p.span_from(at),
                "ALL and $ALL take no measurement level",
            ));
        }
        items.push(CodebookItem { item, level });
        p.eat(Punct::Comma);
    }
    Ok(items)
}

pub(crate) fn codebook(p: &mut Parser) -> PResult<Command> {
    let mut command = Codebook {
        items: items(p)?,
        varinfo: VARINFO
            .iter()
            .flat_map(|(_, v)| v.iter().copied())
            .filter(|v| *v != CodebookInfo::ReservedAttributes)
            .collect(),
        fileinfo: Vec::new(),
        statistics: STATISTICS
            .iter()
            .flat_map(|(_, s)| s.iter().copied())
            .collect(),
        maxcats: 200,
        order: None,
        descending: false,
    };
    let mut seen: Vec<&str> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        let Some(&name) = SUBCOMMANDS.iter().find(|s| p.at_keyword(s)) else {
            return Err(p.subcommand_error("CODEBOOK", &[]));
        };
        p.pos += 1;
        if seen.contains(&name) {
            let message = format!("CODEBOOK allows {name} once");
            return Err(SyntaxError::new(p.span_from(start), message));
        }
        seen.push(name);
        p.eat(Punct::Eq);
        match name {
            "VARINFO" => {
                command.varinfo = keywords(p, &VARINFO, |p| p.unexpected(&names(&VARINFO)))?;
            }
            "FILEINFO" => {
                command.fileinfo = keywords(p, &FILEINFO, |p| p.unexpected(&names(&FILEINFO)))?;
            }
            "STATISTICS" => {
                let (mut none, mut words) = (false, 0);
                command.statistics = Vec::new();
                while !at_end(p) {
                    let found = STATISTICS.iter().find(|(name, _)| p.at_keyword(name));
                    let Some((keyword, chosen)) = found else {
                        return Err(p.unexpected(&names(&STATISTICS)));
                    };
                    p.pos += 1;
                    none |= *keyword == "NONE";
                    words += 1;
                    command.statistics.extend_from_slice(chosen);
                    p.eat(Punct::Comma);
                }
                if words == 0 {
                    return Err(p.unexpected(&names(&STATISTICS)));
                }
                if none && words > 1 {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "STATISTICS NONE stands alone",
                    ));
                }
            }
            _ => options(p, &mut command)?,
        }
    }
    p.finish(&["'/'"])?;
    dedup(&mut command.varinfo);
    dedup(&mut command.fileinfo);
    dedup(&mut command.statistics);
    Ok(Command::Codebook(Box::new(command)))
}

fn dedup<T: PartialEq + Copy>(list: &mut Vec<T>) {
    let mut seen = Vec::new();
    list.retain(|x| {
        let new = !seen.contains(x);
        seen.push(*x);
        new
    });
}

fn options(p: &mut Parser, command: &mut Codebook) -> PResult<()> {
    const KEYWORDS: [&str; 3] = ["MAXCATS", "VARORDER", "SORT"];
    let mut any = false;
    while !at_end(p) {
        any = true;
        if p.eat_keyword("MAXCATS") {
            p.expect(Punct::Eq)?;
            let start = p.pos;
            command.maxcats = match p.number() {
                Some(n) if n >= 0.0 && n.fract() == 0.0 => n as usize,
                Some(_) => {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "MAXCATS must be a whole number from 0",
                    ));
                }
                None => return Err(p.unexpected(&["a whole number"])),
            };
        } else if p.eat_keyword("VARORDER") {
            p.expect(Punct::Eq)?;
            command.order = Some(if p.eat_keyword("FILE") {
                CodebookOrder::File
            } else if p.eat_keyword("ALPHA") {
                CodebookOrder::Alpha
            } else if p.eat_keyword("VARLIST") {
                CodebookOrder::Varlist
            } else if p.eat_keyword("MEASURE") {
                CodebookOrder::Measure
            } else if p.eat_keyword("ATTRIBUTE") {
                p.expect(Punct::LParen)?;
                let name = p.name("a custom attribute name")?;
                p.expect(Punct::RParen)?;
                CodebookOrder::Attribute(name)
            } else {
                return Err(p.unexpected(&["FILE", "ALPHA", "VARLIST", "MEASURE", "ATTRIBUTE"]));
            });
        } else if p.eat_keyword("SORT") {
            p.expect(Punct::Eq)?;
            command.descending = if p.eat_keyword("ASCENDING") {
                false
            } else if p.eat_keyword("DESCENDING") {
                true
            } else {
                return Err(p.unexpected(&["ASCENDING", "DESCENDING"]));
            };
        } else {
            return Err(p.unexpected(&KEYWORDS));
        }
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&KEYWORDS));
    }
    Ok(())
}
