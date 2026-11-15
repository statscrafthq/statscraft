//! Parser of SUMMARIZE: the MEANS cell statistics of dependents within groups of control
//! variables, laid out with the dependents side by side, optionally with the cases listed.
//!
//! Rules from the command's description:
//! - `TABLES=` may be left out for the first table list; more lists follow after a slash, with
//!   or without TABLES. A list is `dependents [BY controls] [BY ...]`, at most 10 BY.
//! - TITLE and FOOTNOTE take quoted strings (several in a row are joined), at most 255
//!   bytes; `\n` in them breaks the line.
//! - CELLS takes the MEANS cell keywords plus NONE; without keywords it is DEFAULT.
//! - MISSING: EXCLUDE (default), INCLUDE or DEPENDENT for user-missing values, and VARIABLE
//!   (default) or TABLE for how a missing dependent leaves a case out.
//! - FORMAT: NOLIST (default), LIST [LIMIT=n] or VALIDLIST; CASENUM (the default) or
//!   NOCASENUM; TOTAL (default) or NOTOTAL; MISSING='text' for system-missing values in the
//!   listing. LIST and VALIDLIST allow one table list only.
//! - STATISTICS: ANOVA, LINEARITY (alias ALL), NONE; without keywords ANOVA.
//! - A subcommand other than TABLES given twice: the last one counts, with a warning.

use crate::ast::{Command, MeansCell as M, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::{at_end, keywords, names};
use crate::stats::{MEANS_DEFAULT, means_cells, variables};
use crate::table::matches_keyword;

/// How user-missing values count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummarizeMissing {
    /// User-missing values are missing everywhere.
    Exclude,
    /// User-missing values are valid everywhere.
    Include,
    /// User-missing values are missing in dependents and valid in controls.
    Dependent,
}

/// `/FORMAT=LIST` or `VALIDLIST`: the cases are listed in the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummarizeList {
    /// VALIDLIST: only cases valid on every dependent are listed.
    pub valid_only: bool,
    /// `LIMIT=n`: only the first n cases are used.
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Summarize {
    /// One entry per table list: the dependents, then one list per BY dimension.
    pub tables: Vec<Vec<Vec<VarItem>>>,
    pub title: Option<String>,
    pub footnote: Option<String>,
    /// In display order, without repeats; empty for CELLS=NONE.
    pub cells: Vec<M>,
    /// The control variables named by `NPCT(var)` and `SPCT(var)`, as written.
    pub within: Vec<String>,
    pub missing: SummarizeMissing,
    /// `MISSING=TABLE`: a case missing on any dependent of the list is left out of all.
    pub tablewise: bool,
    /// None for NOLIST.
    pub list: Option<SummarizeList>,
    /// CASENUM: the listing shows case numbers.
    pub case_numbers: bool,
    /// TOTAL: total rows say "Total"; NOTOTAL leaves the label empty.
    pub total_label: bool,
    /// `FORMAT MISSING='text'`: how system-missing values show in the listing.
    pub missing_text: Option<String>,
    pub anova: bool,
    pub linearity: bool,
    /// Subcommands given more than once, upper case: the last one was used.
    pub repeated: Vec<String>,
}

const SUBCOMMANDS: [&str; 7] = [
    "TABLES",
    "TITLE",
    "FOOTNOTE",
    "CELLS",
    "MISSING",
    "FORMAT",
    "STATISTICS",
];

/// `dependents [BY controls [BY ...]]`, at most 10 BY.
fn table_list(p: &mut Parser) -> PResult<Vec<Vec<VarItem>>> {
    let mut dimensions = vec![variables(p)?];
    let mut start = p.pos;
    while p.eat_word("BY") {
        if dimensions.len() == 11 {
            return Err(SyntaxError::new(
                p.span_from(start),
                "SUMMARIZE allows at most 10 BY in a table list",
            ));
        }
        dimensions.push(variables(p)?);
        start = p.pos;
    }
    Ok(dimensions)
}

/// One or more quoted strings joined, `\n` made a line break, at most 255 bytes.
fn text(p: &mut Parser, what: &str) -> PResult<String> {
    p.eat(Punct::Eq);
    let start = p.pos;
    let mut joined = p.string(what)?;
    while matches!(p.peek(), Some(TokenKind::Str(_))) {
        joined.push_str(&p.string(what)?);
    }
    if joined.len() > 255 {
        let message = format!("{what} is longer than 255 bytes");
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(joined.replace("\\n", "\n"))
}

pub(crate) fn summarize(p: &mut Parser) -> PResult<Command> {
    let mut command = Summarize {
        tables: Vec::new(),
        title: None,
        footnote: None,
        cells: MEANS_DEFAULT.to_vec(),
        within: Vec::new(),
        missing: SummarizeMissing::Exclude,
        tablewise: false,
        list: None,
        case_numbers: true,
        total_label: true,
        missing_text: None,
        anova: false,
        linearity: false,
        repeated: Vec::new(),
    };
    let mut seen: Vec<&str> = Vec::new();
    let mut list_at = None;
    p.eat(Punct::Slash);
    let mut more = true;
    if !p.at_keyword("TABLES") {
        command.tables.push(table_list(p)?);
        more = p.eat(Punct::Slash);
    }
    while more {
        let found = SUBCOMMANDS.iter().find(|s| p.at_keyword(s));
        match found {
            Some(&name) => {
                p.pos += 1;
                if name != "TABLES" {
                    if seen.contains(&name) && !command.repeated.iter().any(|r| r == name) {
                        command.repeated.push(name.to_string());
                    }
                    seen.push(name);
                }
                match name {
                    "TABLES" => {
                        p.eat(Punct::Eq);
                        command.tables.push(table_list(p)?);
                    }
                    "TITLE" => command.title = Some(text(p, "a quoted title")?),
                    "FOOTNOTE" => command.footnote = Some(text(p, "a quoted footnote")?),
                    "CELLS" => {
                        let given = p.eat(Punct::Eq);
                        command.within.clear();
                        command.cells = if !given && at_end(p) {
                            MEANS_DEFAULT.to_vec()
                        } else if p.eat_keyword("NONE") {
                            Vec::new()
                        } else {
                            means_cells(p, &mut command.within)?
                        };
                    }
                    "MISSING" => {
                        p.eat(Punct::Eq);
                        const CHOICES: [(&str, &[u8]); 5] = [
                            ("EXCLUDE", &[0]),
                            ("INCLUDE", &[1]),
                            ("DEPENDENT", &[2]),
                            ("VARIABLE", &[3]),
                            ("TABLE", &[4]),
                        ];
                        command.missing = SummarizeMissing::Exclude;
                        command.tablewise = false;
                        for choice in keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))? {
                            match choice {
                                0 => command.missing = SummarizeMissing::Exclude,
                                1 => command.missing = SummarizeMissing::Include,
                                2 => command.missing = SummarizeMissing::Dependent,
                                3 => command.tablewise = false,
                                _ => command.tablewise = true,
                            }
                        }
                    }
                    "FORMAT" => {
                        p.eat(Punct::Eq);
                        list_at = Some(p.span());
                        format(p, &mut command)?;
                    }
                    _ => {
                        const CHOICES: [(&str, &[u8]); 4] = [
                            ("ANOVA", &[1]),
                            ("LINEARITY", &[2]),
                            ("ALL", &[2]),
                            ("NONE", &[0]),
                        ];
                        let chosen = if !p.eat(Punct::Eq) && at_end(p) {
                            vec![1]
                        } else {
                            keywords(p, &CHOICES, |p| p.unexpected(&names(&CHOICES)))?
                        };
                        for choice in chosen {
                            command.anova = choice > 0;
                            command.linearity = choice == 2;
                        }
                    }
                }
            }
            // another table list after a slash, without the TABLES keyword
            None if matches!(p.peek(), Some(TokenKind::Word(_))) => {
                command.tables.push(table_list(p)?);
            }
            None => return Err(p.subcommand_error("SUMMARIZE", &[])),
        }
        more = p.eat(Punct::Slash);
    }
    p.finish(&["'/'", "BY"])?;
    if command.tables.is_empty() {
        return Err(p.unexpected(&["TABLES=dependents BY controls"]));
    }
    if let (Some(_), Some(span)) = (command.list, list_at)
        && command.tables.len() > 1
    {
        return Err(SyntaxError::new(
            span,
            "FORMAT=LIST and VALIDLIST allow only one table list",
        ));
    }
    let mut seen = Vec::new();
    command.cells.retain(|c| {
        let new = !seen.contains(c);
        seen.push(*c);
        new
    });
    Ok(Command::Summarize(Box::new(command)))
}

/// The keywords of /FORMAT; a later keyword of a group replaces an earlier one.
fn format(p: &mut Parser, command: &mut Summarize) -> PResult<()> {
    const KEYWORDS: [&str; 8] = [
        "NOLIST",
        "LIST",
        "VALIDLIST",
        "CASENUM",
        "NOCASENUM",
        "TOTAL",
        "NOTOTAL",
        "MISSING",
    ];
    command.list = None;
    command.case_numbers = true;
    command.total_label = true;
    command.missing_text = None;
    let mut any = false;
    while !at_end(p) {
        let Some(TokenKind::Word(word)) = p.peek() else {
            return Err(p.unexpected(&KEYWORDS));
        };
        let Some(&keyword) = KEYWORDS.iter().find(|k| matches_keyword(word, k)) else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        any = true;
        match keyword {
            "NOLIST" => command.list = None,
            "LIST" | "VALIDLIST" => {
                let mut list = SummarizeList {
                    valid_only: keyword == "VALIDLIST",
                    limit: None,
                };
                if p.eat_keyword("LIMIT") {
                    p.eat(Punct::Eq);
                    let start = p.pos;
                    list.limit = match p.number() {
                        Some(n) if n >= 1.0 && n.fract() == 0.0 => Some(n as usize),
                        Some(_) => {
                            return Err(SyntaxError::new(
                                p.span_from(start),
                                "LIMIT must be a whole number from 1",
                            ));
                        }
                        None => return Err(p.unexpected(&["a whole number from 1"])),
                    };
                }
                command.list = Some(list);
            }
            "CASENUM" => command.case_numbers = true,
            "NOCASENUM" => command.case_numbers = false,
            "TOTAL" => command.total_label = true,
            "NOTOTAL" => command.total_label = false,
            _ => {
                p.expect(Punct::Eq)?;
                command.missing_text = Some(p.string("a quoted text for system-missing values")?);
            }
        }
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&KEYWORDS));
    }
    Ok(())
}
