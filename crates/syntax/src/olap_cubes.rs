//! Parser of OLAP CUBES: the MEANS cell statistics of summary variables within the groups of
//! grouping variables, with differences between categories or between summary variables.
//!
//! Rules from the command's description:
//! - `summary variables BY grouping variables [BY ...]`, at most 10 BY.
//! - CELLS takes the MEANS cell keywords plus NONE; DEFAULT (and CELLS without keywords or
//!   left out) is SUM COUNT MEAN STDDEV SPCT NPCT.
//! - CREATE: optional labels first (quoted, or DEFAULT), then GAC and/or GPC with
//!   `(gvar (v1 v2) ...)`, or VAC and/or VPC with `(svar1 svar2) ...`. Group and variable
//!   differences cannot be mixed. Each pair gives one difference per type, in written order.
//! - TITLE and FOOTNOTE take quoted strings, each one a line, at most 255 bytes; `\n` in them
//!   breaks the line too.
//! - HIDESMALLCOUNTS [COUNT=n]: n a whole number of 2 or more, 5 when left out.
//! - A subcommand given twice: the last one counts, with a warning.

use crate::ast::{Command, MeansCell as M, Value, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::stats::{means_cells_with, variables};

/// DEFAULT, and the cells when /CELLS is left out.
pub(crate) const OLAP_DEFAULT: [M; 6] = [M::Sum, M::Count, M::Mean, M::StdDev, M::Spct, M::Npct];

/// The two sides of one difference: the result is second minus first.
#[derive(Debug, Clone, PartialEq)]
pub enum OlapPair {
    /// GAC or GPC: two categories of `OlapCubes::create_variable`.
    Categories(Value, Value),
    /// VAC or VPC: two summary variables.
    Variables(String, String),
}

/// One difference category of /CREATE.
#[derive(Debug, Clone, PartialEq)]
pub struct OlapDifference {
    /// None for DEFAULT or no label.
    pub label: Option<String>,
    /// GPC or VPC: (second - first) / first, as a percentage.
    pub percent: bool,
    pub pair: OlapPair,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OlapCubes {
    pub summary: Vec<VarItem>,
    /// One list per BY.
    pub groups: Vec<Vec<VarItem>>,
    /// In display order, without repeats; empty for CELLS=NONE.
    pub cells: Vec<M>,
    /// The grouping variables named by `NPCT(var)` and `SPCT(var)`, as written.
    pub within: Vec<String>,
    /// The grouping variable of GAC and GPC.
    pub create_variable: Option<String>,
    /// In display order.
    pub differences: Vec<OlapDifference>,
    pub title: Option<String>,
    pub footnote: Option<String>,
    /// `/HIDESMALLCOUNTS`: counts below this show as "<n".
    pub hide_small_counts: Option<u32>,
    /// Subcommands given more than once, upper case: the last one was used.
    pub repeated: Vec<String>,
}

const SUBCOMMANDS: [&str; 5] = ["CELLS", "CREATE", "TITLE", "FOOTNOTE", "HIDESMALLCOUNTS"];

/// Quoted strings, each a line, `\n` a line break too, at most 255 bytes.
fn text(p: &mut Parser, what: &str) -> PResult<String> {
    p.eat(Punct::Eq);
    let start = p.pos;
    let mut lines = vec![p.string(what)?];
    while matches!(p.peek(), Some(TokenKind::Str(_))) {
        lines.push(p.string(what)?);
    }
    let joined = lines.join("\n");
    if joined.len() > 255 {
        let message = format!("{what} is longer than 255 bytes");
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(joined.replace("\\n", "\n"))
}

pub(crate) fn olap_cubes(p: &mut Parser) -> PResult<Command> {
    let summary = variables(p)?;
    let mut groups = Vec::new();
    let mut start = p.pos;
    while p.eat_word("BY") {
        if groups.len() == 10 {
            return Err(SyntaxError::new(
                p.span_from(start),
                "OLAP CUBES allows at most 10 BY",
            ));
        }
        groups.push(variables(p)?);
        start = p.pos;
    }
    if groups.is_empty() {
        return Err(p.unexpected(&["BY grouping variables"]));
    }
    let mut command = OlapCubes {
        summary,
        groups,
        cells: OLAP_DEFAULT.to_vec(),
        within: Vec::new(),
        create_variable: None,
        differences: Vec::new(),
        title: None,
        footnote: None,
        hide_small_counts: None,
        repeated: Vec::new(),
    };
    let mut seen: Vec<&str> = Vec::new();
    while p.eat(Punct::Slash) {
        let Some(&name) = SUBCOMMANDS.iter().find(|s| p.at_keyword(s)) else {
            return Err(p.subcommand_error("OLAP CUBES", &[]));
        };
        p.pos += 1;
        if seen.contains(&name) && !command.repeated.iter().any(|r| r == name) {
            command.repeated.push(name.to_string());
        }
        seen.push(name);
        match name {
            "CELLS" => {
                let given = p.eat(Punct::Eq);
                command.within.clear();
                command.cells = if !given && at_end(p) {
                    OLAP_DEFAULT.to_vec()
                } else if p.eat_keyword("NONE") {
                    Vec::new()
                } else {
                    means_cells_with(p, &mut command.within, &OLAP_DEFAULT)?
                };
            }
            "CREATE" => create(p, &mut command)?,
            "TITLE" => command.title = Some(text(p, "a quoted title")?),
            "FOOTNOTE" => command.footnote = Some(text(p, "a quoted footnote")?),
            _ => {
                command.hide_small_counts = Some(crate::ctables::hide_small_counts(p)?);
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'", "BY"])?;
    let mut seen = Vec::new();
    command.cells.retain(|c| {
        let new = !seen.contains(c);
        seen.push(*c);
        new
    });
    Ok(Command::OlapCubes(Box::new(command)))
}

/// `/CREATE ['label' | DEFAULT ...] [=] types (specification)`.
fn create(p: &mut Parser, command: &mut OlapCubes) -> PResult<()> {
    const TYPES: [&str; 4] = ["GAC", "GPC", "VAC", "VPC"];
    let mut labels = Vec::new();
    loop {
        if matches!(p.peek(), Some(TokenKind::Str(_))) {
            labels.push(Some(p.string("a label")?));
        } else if p.eat_keyword("DEFAULT") {
            labels.push(None);
        } else {
            break;
        }
    }
    p.eat(Punct::Eq);
    let mut types: Vec<&str> = Vec::new();
    while let Some(&kind) = TYPES.iter().find(|t| p.at_keyword(t)) {
        p.pos += 1;
        if !types.contains(&kind) {
            types.push(kind);
        }
    }
    if types.is_empty() {
        return Err(p.unexpected(&TYPES));
    }
    let groups = types.iter().any(|t| t.starts_with('G'));
    if groups && types.iter().any(|t| t.starts_with('V')) {
        return Err(SyntaxError::new(
            p.span(),
            "CREATE cannot mix differences of categories (GAC, GPC) with differences of variables (VAC, VPC)",
        ));
    }
    let mut pairs = Vec::new();
    if groups {
        p.expect(Punct::LParen)?;
        command.create_variable = Some(p.name("a grouping variable")?);
        while p.eat(Punct::LParen) {
            let first = category(p)?;
            let second = category(p)?;
            p.expect(Punct::RParen)?;
            pairs.push(OlapPair::Categories(first, second));
        }
        if pairs.is_empty() {
            return Err(p.unexpected(&["(value value)"]));
        }
        p.expect(Punct::RParen)?;
    } else {
        command.create_variable = None;
        while p.eat(Punct::LParen) {
            let first = p.name("a summary variable")?;
            p.eat(Punct::Comma);
            let second = p.name("a summary variable")?;
            p.expect(Punct::RParen)?;
            pairs.push(OlapPair::Variables(first, second));
        }
        if pairs.is_empty() {
            return Err(p.unexpected(&["(variable variable)"]));
        }
    }
    let count = pairs.len() * types.len();
    if labels.len() > count {
        return Err(SyntaxError::new(
            p.span(),
            format!(
                "CREATE gives {} labels for {count} differences",
                labels.len()
            ),
        ));
    }
    labels.resize(count, None);
    let mut labels = labels.into_iter();
    command.differences.clear();
    for pair in pairs {
        for kind in &types {
            command.differences.push(OlapDifference {
                label: labels.next().flatten(),
                percent: kind.ends_with("PC"),
                pair: pair.clone(),
            });
        }
    }
    Ok(())
}

/// A category value of CREATE, with an optional comma after it.
fn category(p: &mut Parser) -> PResult<Value> {
    let value = p
        .value()
        .ok_or_else(|| p.unexpected(&["a category value"]))?;
    p.eat(Punct::Comma);
    Ok(value)
}
