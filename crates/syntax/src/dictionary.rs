//! Parsers of the dictionary commands: ADD VALUE LABELS, FILE LABEL, DOCUMENT, ADD DOCUMENT,
//! DROP DOCUMENTS, VARIABLE ROLE, VARIABLE WIDTH, VARIABLE ALIGNMENT, VARIABLE ATTRIBUTE,
//! DATAFILE ATTRIBUTE, ALTER TYPE and APPLY DICTIONARY.

use crate::ast::{
    AlterTarget, AlterType, ApplyDictionary, AttributeChange, Command, FormatType, VarItem,
};
use crate::commands::{format, value_labels};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

pub(crate) fn add_value_labels(p: &mut Parser) -> PResult<Command> {
    match value_labels(p)? {
        Command::ValueLabels(groups) => Ok(Command::AddValueLabels(groups)),
        _ => unreachable!("value_labels gives VALUE LABELS"),
    }
}

/// FILE LABEL arrives as one string: the free text, or the quoted label.
pub(crate) fn file_label(p: &mut Parser) -> PResult<Command> {
    let label = p.string("the label")?;
    p.finish(&[])?;
    Ok(Command::FileLabel(label))
}

/// DOCUMENT arrives as one string of free text; each line is a line of the document.
pub(crate) fn document(p: &mut Parser) -> PResult<Command> {
    let text = p.string("the text")?;
    p.finish(&[])?;
    Ok(Command::AddDocument(
        text.lines().map(|l| l.trim_end().to_string()).collect(),
    ))
}

pub(crate) fn add_document(p: &mut Parser) -> PResult<Command> {
    let mut lines = Vec::new();
    while let Some(TokenKind::Str(_)) = p.peek() {
        lines.push(p.string("a quoted line")?);
    }
    p.finish(&["a quoted line"])?;
    Ok(Command::AddDocument(lines))
}

pub(crate) fn drop_documents(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::DropDocuments)
}

const ROLES: [&str; 6] = ["INPUT", "TARGET", "BOTH", "NONE", "PARTITION", "SPLIT"];

pub(crate) fn variable_role(p: &mut Parser) -> PResult<Command> {
    let mut roles = Vec::new();
    while p.eat(Punct::Slash) {
        let role = ROLES.iter().find(|r| p.eat_word(r));
        let Some(role) = role else {
            return Err(p.unexpected(&ROLES));
        };
        roles.push((role.to_lowercase(), p.var_list()?));
    }
    if roles.is_empty() {
        return Err(p.unexpected(&["'/' and a role such as /INPUT"]));
    }
    p.finish(&["'/'"])?;
    Ok(Command::VariableRole(roles))
}

/// `varlist (value) [/] varlist (value) ...`
fn groups<T>(
    p: &mut Parser,
    mut value: impl FnMut(&mut Parser) -> PResult<T>,
) -> PResult<Vec<(Vec<VarItem>, T)>> {
    let mut out = Vec::new();
    loop {
        p.eat(Punct::Slash);
        if p.peek().is_none() && !out.is_empty() {
            break;
        }
        let vars = p.var_list()?;
        p.expect(Punct::LParen)?;
        let v = value(p)?;
        p.expect(Punct::RParen)?;
        out.push((vars, v));
        if p.peek().is_none() {
            break;
        }
    }
    p.finish(&[])?;
    Ok(out)
}

pub(crate) fn variable_alignment(p: &mut Parser) -> PResult<Command> {
    groups(p, |p| {
        for a in ["LEFT", "RIGHT", "CENTER"] {
            if p.eat_keyword(a) {
                return Ok(a.to_lowercase());
            }
        }
        Err(p.unexpected(&["LEFT", "RIGHT", "CENTER"]))
    })
    .map(Command::VariableAlignment)
}

/// `name[n]` of an attribute, with the 1-based index when given.
fn attribute_name(p: &mut Parser) -> PResult<(String, Option<usize>)> {
    let name = match p.peek() {
        Some(TokenKind::Word(w)) => w.clone(),
        _ => return Err(p.unexpected(&["an attribute name"])),
    };
    p.pos += 1;
    if !p.eat(Punct::LBracket) {
        return Ok((name, None));
    }
    let index = match p.number() {
        Some(n) if n >= 1.0 && n.fract() == 0.0 => n as usize,
        _ => return Err(p.unexpected(&["an index from 1"])),
    };
    p.expect(Punct::RBracket)?;
    Ok((name, Some(index)))
}

/// `ATTRIBUTE=name('value') ...` and `DELETE=name ...` into `change`.
fn attribute_specs(p: &mut Parser, change: &mut AttributeChange) -> PResult<bool> {
    if p.eat_keyword("ATTRIBUTE") {
        p.expect(Punct::Eq)?;
        while let Some(TokenKind::Word(_)) = p.peek() {
            if p.at_keyword("DELETE") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
                break;
            }
            let (name, index) = attribute_name(p)?;
            p.expect(Punct::LParen)?;
            let value = p.string("a quoted value")?;
            p.expect(Punct::RParen)?;
            change.set.push((name, index, value));
        }
        Ok(true)
    } else if p.eat_keyword("DELETE") {
        p.expect(Punct::Eq)?;
        while let Some(TokenKind::Word(_)) = p.peek() {
            if p.at_keyword("ATTRIBUTE") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
                break;
            }
            change.delete.push(attribute_name(p)?);
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

pub(crate) fn datafile_attribute(p: &mut Parser) -> PResult<Command> {
    let mut change = AttributeChange {
        variables: None,
        set: Vec::new(),
        delete: Vec::new(),
    };
    while attribute_specs(p, &mut change)? {}
    if change.set.is_empty() && change.delete.is_empty() {
        return Err(p.unexpected(&["ATTRIBUTE=", "DELETE="]));
    }
    p.finish(&["ATTRIBUTE", "DELETE"])?;
    Ok(Command::Attribute(vec![change]))
}

pub(crate) fn variable_attribute(p: &mut Parser) -> PResult<Command> {
    let mut changes = Vec::new();
    loop {
        p.eat(Punct::Slash);
        if !p.eat_keyword("VARIABLES") {
            return Err(p.unexpected(&["VARIABLES="]));
        }
        p.expect(Punct::Eq)?;
        // the list ends where ATTRIBUTE= or DELETE= begins
        let mut vars = Vec::new();
        while let Some(TokenKind::Word(_)) = p.peek() {
            let assigns = p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq));
            if assigns && (p.at_keyword("ATTRIBUTE") || p.at_keyword("DELETE")) {
                break;
            }
            if p.eat_word("ALL") {
                vars.push(VarItem::All);
                continue;
            }
            let first = p.name("a variable name")?;
            vars.push(if p.eat_word("TO") {
                VarItem::Range(first, p.name("a variable name")?)
            } else {
                VarItem::Name(first)
            });
            p.eat(Punct::Comma);
        }
        if vars.is_empty() {
            return Err(p.unexpected(&["a variable name"]));
        }
        let mut change = AttributeChange {
            variables: Some(vars),
            set: Vec::new(),
            delete: Vec::new(),
        };
        while attribute_specs(p, &mut change)? {}
        if change.set.is_empty() && change.delete.is_empty() {
            return Err(p.unexpected(&["ATTRIBUTE=", "DELETE="]));
        }
        changes.push(change);
        if !p.at(Punct::Slash) {
            break;
        }
    }
    p.finish(&["/VARIABLES"])?;
    Ok(Command::Attribute(changes))
}

fn format_word(p: &mut Parser, example: &str) -> PResult<String> {
    match p.peek() {
        Some(TokenKind::Word(w)) => {
            let w = w.clone();
            p.pos += 1;
            Ok(w)
        }
        _ => Err(p.unexpected(&[example])),
    }
}

pub(crate) fn alter_type(p: &mut Parser) -> PResult<Command> {
    let mut groups = Vec::new();
    while let Some(TokenKind::Word(_)) = p.peek() {
        if p.at_keyword("PRINT") && p.peek_at(1).is_none() {
            break;
        }
        let vars = p.var_list()?;
        p.expect(Punct::LParen)?;
        let start = p.pos;
        let mut first = format_word(p, "a format such as A20 or F8.2, or AMIN")?;
        let mut only = None;
        if p.eat(Punct::Eq) {
            // the input format: a type, with a width when only that width is meant
            let digits = first.find(|c: char| c.is_ascii_digit());
            let (name, width) = first.split_at(digits.unwrap_or(first.len()));
            let Some(kind) = FormatType::from_name(name) else {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!("unknown format type {}", name.to_uppercase()),
                ));
            };
            only = Some((kind, width.parse::<u16>().ok()));
            first = format_word(p, "a format such as A20 or F8.2, or AMIN")?;
        }
        let to = if first.eq_ignore_ascii_case("AMIN") || first.eq_ignore_ascii_case("AHEXMIN") {
            // shortcut: AMIN + n% is not read, since % is no token yet; add it when a file needs it
            let mut plus = 0.0;
            if p.eat(Punct::Plus) {
                plus = p.number().ok_or_else(|| p.unexpected(&["a number"]))?;
            }
            AlterTarget::Amin {
                plus,
                percent: false,
            }
        } else {
            let parsed =
                format(&first).map_err(|message| SyntaxError::new(p.span_from(start), message))?;
            AlterTarget::Format(parsed)
        };
        p.expect(Punct::RParen)?;
        groups.push(AlterType { vars, only, to });
    }
    if groups.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    if p.eat(Punct::Slash) {
        if !p.eat_keyword("PRINT") {
            return Err(p.unexpected(&["PRINT"]));
        }
        p.eat(Punct::Eq);
        while p.eat_keyword("ALTEREDTYPES")
            || p.eat_keyword("ALTEREDVALUES")
            || p.eat_keyword("NONE")
        {}
    }
    p.finish(&["/PRINT"])?;
    Ok(Command::AlterType(groups))
}

const FILEINFO: [&str; 7] = [
    "ATTRIBUTES",
    "DOCUMENTS",
    "FILELABEL",
    "MRSETS",
    "VARSETS",
    "WEIGHT",
    "ALL",
];
const VARINFO: [&str; 10] = [
    "ALIGNMENT",
    "ATTRIBUTES",
    "FORMATS",
    "LEVEL",
    "MISSING",
    "ROLE",
    "VALLABELS",
    "VARLABEL",
    "WIDTH",
    "ALL",
];

/// Keywords of FILEINFO or VARINFO, each with `= REPLACE | MERGE` when given.
fn info_keywords(p: &mut Parser, known: &[&str]) -> PResult<Vec<(String, Option<String>)>> {
    let mut out = Vec::new();
    while let Some(k) = known.iter().find(|k| p.at_keyword(k)) {
        p.pos += 1;
        let mode = if p.eat(Punct::Eq) {
            if p.eat_keyword("REPLACE") {
                Some("REPLACE".to_string())
            } else if p.eat_keyword("MERGE") {
                Some("MERGE".to_string())
            } else {
                return Err(p.unexpected(&["REPLACE", "MERGE"]));
            }
        } else {
            None
        };
        out.push((k.to_string(), mode));
    }
    Ok(out)
}

pub(crate) fn apply_dictionary(p: &mut Parser) -> PResult<Command> {
    p.eat(Punct::Slash);
    if !p.eat_keyword("FROM") {
        return Err(p.unexpected(&["FROM"]));
    }
    p.eat(Punct::Eq);
    let from = if p.eat(Punct::Star) {
        None
    } else if let Some(TokenKind::Word(w)) = p.peek() {
        let w = w.clone();
        p.pos += 1;
        Some(w)
    } else {
        Some(p.string("a quoted file name, a dataset name or *")?)
    };
    let mut command = ApplyDictionary {
        from,
        source: None,
        target: None,
        newvars: false,
        fileinfo: Vec::new(),
        varinfo: None,
    };
    if p.eat_keyword("PASSWORD") {
        p.eat(Punct::Eq);
        p.string("a quoted password")?;
    }
    while p.eat(Punct::Slash) {
        let side = if p.eat_keyword("SOURCE") {
            Some(false)
        } else if p.eat_keyword("TARGET") {
            Some(true)
        } else {
            None
        };
        if let Some(target) = side {
            if !p.eat_keyword("VARIABLES") {
                return Err(p.unexpected(&["VARIABLES"]));
            }
            p.expect(Punct::Eq)?;
            let vars = Some(p.var_list()?);
            if target {
                command.target = vars;
            } else {
                command.source = vars;
            }
        } else if p.eat_keyword("NEWVARS") {
            command.newvars = true;
        } else if p.eat_keyword("FILEINFO") {
            command.fileinfo = info_keywords(p, &FILEINFO)?;
        } else if p.eat_keyword("VARINFO") {
            command.varinfo = Some(info_keywords(p, &VARINFO)?);
        } else {
            return Err(p.unexpected(&["SOURCE", "TARGET", "NEWVARS", "FILEINFO", "VARINFO"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::ApplyDictionary(command))
}
