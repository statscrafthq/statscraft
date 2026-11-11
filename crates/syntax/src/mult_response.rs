//! Parser of MULT RESPONSE.
//!
//! Rules from the command's description:
//! - GROUPS, VARIABLES, FREQUENCIES and TABLES come in that order, each at most once, before
//!   the other subcommands; at least one of GROUPS and VARIABLES and one of FREQUENCIES and
//!   TABLES.
//! - A group is `name ['label'] (varlist (value))` for a dichotomy group or
//!   `(varlist (min, max))` for a category group. The label needs no quotes: the words up to
//!   the parenthesis are the label.
//! - TABLES lists are separated by slashes: `items BY items [BY items ...] [(PAIRED)]`.
//! - CELLS without keywords is ALL; MISSING, FORMAT and BASE take their keywords.

use crate::ast::{Command, CrosstabsCell, MrGroup, MrTable, MultResponse, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::table::matches_keyword;

const BASIC: [&str; 4] = ["GROUPS", "VARIABLES", "FREQUENCIES", "TABLES"];
const OPTIONAL: [&str; 4] = ["MISSING", "FORMAT", "BASE", "CELLS"];

/// The subcommand a word after a slash names, if it is one; a word followed by BY or TO
/// starts a table list instead.
fn subcommand(p: &Parser) -> Option<&'static str> {
    let Some(TokenKind::Word(word)) = p.peek() else {
        return None;
    };
    let list = matches!(p.peek_at(1), Some(TokenKind::Word(w))
        if w.eq_ignore_ascii_case("BY") || w.eq_ignore_ascii_case("TO"));
    if list {
        return None;
    }
    BASIC
        .iter()
        .chain(&OPTIONAL)
        .find(|k| matches_keyword(word, k))
        .copied()
}

fn number(p: &mut Parser) -> PResult<f64> {
    p.number().ok_or_else(|| p.unexpected(&["a number"]))
}

fn group(p: &mut Parser) -> PResult<MrGroup> {
    let name = p.name("a group name")?;
    let label = if let Some(TokenKind::Str(_)) = p.peek() {
        Some(p.string("a label")?)
    } else {
        // a label without quotes: the words up to the parenthesis
        let mut words = Vec::new();
        while let Some(TokenKind::Word(w)) = p.peek() {
            words.push(w.clone());
            p.pos += 1;
        }
        (!words.is_empty()).then(|| words.join(" "))
    };
    p.expect(Punct::LParen)?;
    let variables = p.var_list()?;
    p.expect(Punct::LParen)?;
    let value = number(p)?;
    p.eat(Punct::Comma);
    let max = match p.at(Punct::RParen) {
        true => None,
        false => Some(number(p)?),
    };
    p.expect(Punct::RParen)?;
    p.expect(Punct::RParen)?;
    Ok(MrGroup {
        name,
        label,
        variables,
        value,
        max,
    })
}

/// `varlist (min, max) [varlist (min, max) ...]`
fn variables(p: &mut Parser) -> PResult<Vec<(Vec<VarItem>, f64, f64)>> {
    let mut lists = Vec::new();
    while let Some(TokenKind::Word(_)) = p.peek() {
        let items = p.var_list()?;
        p.expect(Punct::LParen)?;
        let min = number(p)?;
        p.eat(Punct::Comma);
        let max = number(p)?;
        p.expect(Punct::RParen)?;
        lists.push((items, min, max));
    }
    if lists.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(lists)
}

/// Group and variable names with TO, up to BY, a parenthesis or a slash.
fn items(p: &mut Parser) -> PResult<Vec<VarItem>> {
    let mut items = Vec::new();
    while let Some(TokenKind::Word(w)) = p.peek() {
        if w.eq_ignore_ascii_case("BY") {
            break;
        }
        let first = p.name("a group or variable name")?;
        if p.eat_word("TO") {
            items.push(VarItem::Range(first, p.name("a group or variable name")?));
        } else {
            items.push(VarItem::Name(first));
        }
        p.eat(Punct::Comma);
    }
    if items.is_empty() {
        return Err(p.unexpected(&["a group or variable name"]));
    }
    Ok(items)
}

fn table(p: &mut Parser) -> PResult<MrTable> {
    let mut dimensions = vec![items(p)?];
    while p.eat_word("BY") {
        dimensions.push(items(p)?);
    }
    if dimensions.len() < 2 {
        return Err(p.unexpected(&["BY"]));
    }
    let paired = p.eat(Punct::LParen);
    if paired {
        if !p.eat_keyword("PAIRED") {
            return Err(p.unexpected(&["PAIRED"]));
        }
        p.expect(Punct::RParen)?;
    }
    Ok(MrTable { dimensions, paired })
}

/// Keywords up to the next slash, each one of `allowed`, in upper case and in full.
fn keywords(p: &mut Parser, allowed: &[&'static str]) -> PResult<Vec<&'static str>> {
    p.eat(Punct::Eq);
    let mut found = Vec::new();
    while let Some(TokenKind::Word(_)) = p.peek() {
        match allowed.iter().find(|k| p.at_keyword(k)) {
            Some(k) => {
                found.push(*k);
                p.pos += 1;
            }
            None => return Err(p.unexpected(allowed)),
        }
        p.eat(Punct::Comma);
    }
    Ok(found)
}

pub(crate) fn mult_response(p: &mut Parser) -> PResult<Command> {
    let mut command = MultResponse {
        groups: Vec::new(),
        variables: Vec::new(),
        frequencies: Vec::new(),
        tables: Vec::new(),
        mdgroup: false,
        mrgroup: false,
        include: false,
        no_labels: false,
        condense: false,
        base_responses: None,
        cells: vec![CrosstabsCell::Count],
        layout: Vec::new(),
    };
    // how far the basic subcommands got, in their order; 4 once an optional one came
    let mut stage = 0;
    let mut last = "";
    p.eat(Punct::Slash);
    loop {
        let start = p.pos;
        let Some(name) = subcommand(p) else {
            // the manual's own example writes a slash between two groups
            if last == "GROUPS" && matches!(p.peek(), Some(TokenKind::Word(_))) {
                command.groups.push(group(p)?);
                if !p.eat(Punct::Slash) {
                    break;
                }
                continue;
            }
            if last == "TABLES" && matches!(p.peek(), Some(TokenKind::Word(_))) {
                command.tables.push(table(p)?);
                if !p.eat(Punct::Slash) {
                    break;
                }
                continue;
            }
            return Err(p.subcommand_error("MULT RESPONSE", &[]));
        };
        p.pos += 1;
        if let Some(rank) = BASIC.iter().position(|b| *b == name) {
            if rank < stage {
                let message = match stage {
                    4 => format!("{name} must come before MISSING, FORMAT, BASE and CELLS"),
                    _ if BASIC[stage - 1] == name => format!("{name} may be given only once"),
                    _ => format!("{name} must come before {}", BASIC[stage - 1]),
                };
                return Err(SyntaxError::new(p.span_from(start), message));
            }
            stage = rank + 1;
        } else {
            stage = 4;
        }
        last = name;
        match name {
            "GROUPS" => {
                p.eat(Punct::Eq);
                while matches!(p.peek(), Some(TokenKind::Word(_))) {
                    command.groups.push(group(p)?);
                }
                if command.groups.is_empty() {
                    return Err(p.unexpected(&["a group name"]));
                }
            }
            "VARIABLES" => {
                p.eat(Punct::Eq);
                command.variables = variables(p)?;
            }
            "FREQUENCIES" => {
                p.eat(Punct::Eq);
                command.frequencies = items(p)?;
            }
            "TABLES" => {
                p.eat(Punct::Eq);
                command.tables.push(table(p)?);
            }
            "MISSING" => {
                for k in keywords(p, &["TABLE", "MDGROUP", "MRGROUP", "INCLUDE"])? {
                    match k {
                        "MDGROUP" => command.mdgroup = true,
                        "MRGROUP" => command.mrgroup = true,
                        "INCLUDE" => command.include = true,
                        _ => (command.mdgroup, command.mrgroup) = (false, false),
                    }
                }
            }
            "FORMAT" => {
                let allowed = [
                    "LABELS", "NOLABELS", "TABLE", "CONDENSE", "ONEPAGE", "DOUBLE",
                ];
                for k in keywords(p, &allowed)? {
                    match k {
                        "LABELS" | "NOLABELS" => command.no_labels = k == "NOLABELS",
                        "TABLE" | "CONDENSE" => command.condense = k == "CONDENSE",
                        _ => command.layout.push(k.to_string()),
                    }
                }
            }
            "BASE" => {
                let chosen = keywords(p, &["CASES", "RESPONSES"])?;
                let Some(last) = chosen.last() else {
                    return Err(p.unexpected(&["CASES", "RESPONSES"]));
                };
                command.base_responses = Some(*last == "RESPONSES");
            }
            _ => {
                let allowed = ["COUNT", "ROW", "COLUMN", "TOTAL", "ALL"];
                let chosen = keywords(p, &allowed)?;
                // without keywords CELLS is ALL
                let all = chosen.is_empty() || chosen.contains(&"ALL");
                let wanted = |k: &str| all || chosen.contains(&k);
                command.cells = vec![CrosstabsCell::Count];
                for (k, cell) in [
                    ("ROW", CrosstabsCell::Row),
                    ("COLUMN", CrosstabsCell::Column),
                    ("TOTAL", CrosstabsCell::Total),
                ] {
                    if wanted(k) {
                        command.cells.push(cell);
                    }
                }
            }
        }
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    if command.groups.is_empty() && command.variables.is_empty() {
        return Err(p.unexpected(&["GROUPS", "VARIABLES"]));
    }
    if command.frequencies.is_empty() && command.tables.is_empty() {
        return Err(p.unexpected(&["FREQUENCIES", "TABLES"]));
    }
    if command.base_responses == Some(false) && command.tables.iter().any(|t| t.paired) {
        return Err(SyntaxError::new(
            p.span(),
            "BASE=CASES cannot be used with PAIRED tables, whose percentages are based on \
             responses",
        ));
    }
    Ok(Command::MultResponse(Box::new(command)))
}
