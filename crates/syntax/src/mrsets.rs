//! MRSETS: the syntax tree of defining, deleting and listing multiple response sets.

use crate::ast::{Command, Value, VarItem};
use crate::ctables::{at_end_of_subcommand, choice, var_list};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

/// One subcommand of MRSETS. They run in the order written.
#[derive(Debug, Clone, PartialEq)]
pub enum Mrsets {
    /// `/MDGROUP` or `/MCGROUP`: define a set, replacing one of the same name.
    Define(MrsetsGroup),
    /// `/DELETE NAME=[...]`; None is `NAME=ALL`.
    Delete(Option<Vec<String>>),
    /// `/DISPLAY NAME=[...]`; None is `NAME=ALL`.
    Display(Option<Vec<String>>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MrsetsGroup {
    /// With the leading `$`.
    pub name: String,
    pub label: Option<String>,
    pub variables: Vec<VarItem>,
    /// The counted value of `/MDGROUP`; None for `/MCGROUP`.
    pub value: Option<Value>,
}

fn not_supported(p: &Parser, start: usize, what: &str) -> SyntaxError {
    SyntaxError::new(
        p.span_from(start),
        format!("{what} in MRSETS is not supported yet"),
    )
}

fn set_name(p: &mut Parser) -> PResult<String> {
    let span = p.span();
    let name = p.name("a set name such as $news")?;
    if !name.starts_with('$') {
        let mut error = SyntaxError::new(span, format!("set name {name} does not start with $"));
        error.expected = vec!["a set name such as $news".into()];
        return Err(error);
    }
    Ok(name)
}

/// `/MDGROUP` (`dichotomy`) or `/MCGROUP` with the parser after the subcommand name.
fn group(p: &mut Parser, start: usize, dichotomy: bool) -> PResult<MrsetsGroup> {
    let (mut name, mut label, mut variables, mut value) = (None, None, None, None);
    // an attribute given twice takes its last value
    while !at_end_of_subcommand(p) {
        let at = p.pos;
        if p.eat_keyword("NAME") {
            p.expect(Punct::Eq)?;
            name = Some(set_name(p)?);
        } else if p.eat_keyword("LABEL") {
            p.expect(Punct::Eq)?;
            label = Some(p.string("a quoted label")?);
        } else if p.eat_keyword("VARIABLES") {
            p.expect(Punct::Eq)?;
            variables = Some(var_list(p)?);
        } else if dichotomy && p.eat_keyword("VALUE") {
            p.expect(Punct::Eq)?;
            value = Some(
                p.value()
                    .ok_or_else(|| p.unexpected(&["a number", "a quoted string"]))?,
            );
        } else if dichotomy && p.eat_keyword("CATEGORYLABELS") {
            if choice(p, &["VARLABELS", "COUNTEDVALUES"])? == 1 {
                // the file writer has no place for this choice yet, so a SAVE would lose it
                return Err(not_supported(p, at, "CATEGORYLABELS=COUNTEDVALUES"));
            }
        } else if dichotomy && p.eat_keyword("LABELSOURCE") {
            choice(p, &["VARLABEL"])?;
            return Err(not_supported(p, at, "LABELSOURCE=VARLABEL"));
        } else if dichotomy {
            return Err(p.unexpected(&[
                "NAME",
                "LABEL",
                "VARIABLES",
                "VALUE",
                "CATEGORYLABELS",
                "LABELSOURCE",
            ]));
        } else {
            return Err(p.unexpected(&["NAME", "LABEL", "VARIABLES"]));
        }
    }
    let subcommand = if dichotomy { "MDGROUP" } else { "MCGROUP" };
    let missing =
        |what: &str| SyntaxError::new(p.span_from(start), format!("/{subcommand} needs {what}"));
    Ok(MrsetsGroup {
        name: name.ok_or_else(|| missing("NAME=$name"))?,
        label,
        variables: variables.ok_or_else(|| missing("VARIABLES=varlist"))?,
        value: match (dichotomy, value) {
            (true, None) => return Err(missing("VALUE=, the value that counts as a response")),
            (_, value) => value,
        },
    })
}

/// `NAME=[$a $b ...]` or `NAME=ALL`; None for ALL.
fn set_names(p: &mut Parser) -> PResult<Option<Vec<String>>> {
    if !p.eat_keyword("NAME") {
        return Err(p.unexpected(&["NAME"]));
    }
    p.expect(Punct::Eq)?;
    if p.eat_word("ALL") {
        return Ok(None);
    }
    if !p.eat(Punct::LBracket) {
        return Err(p
            .unexpected(&["'['", "ALL"])
            .hint("set names go in square brackets: NAME=[$a $b]"));
    }
    let mut names = vec![set_name(p)?];
    p.eat(Punct::Comma);
    while !p.eat(Punct::RBracket) {
        if !matches!(p.peek(), Some(TokenKind::Word(_))) {
            return Err(p.unexpected(&["a set name", "']'"]));
        }
        names.push(set_name(p)?);
        p.eat(Punct::Comma);
    }
    Ok(Some(names))
}

pub(crate) fn mrsets(p: &mut Parser) -> PResult<Command> {
    let mut subcommands = Vec::new();
    p.eat(Punct::Slash);
    loop {
        let start = p.pos;
        subcommands.push(if p.eat_keyword("MDGROUP") {
            Mrsets::Define(group(p, start, true)?)
        } else if p.eat_keyword("MCGROUP") {
            Mrsets::Define(group(p, start, false)?)
        } else if p.eat_keyword("DELETE") {
            Mrsets::Delete(set_names(p)?)
        } else if p.eat_keyword("DISPLAY") {
            Mrsets::Display(set_names(p)?)
        } else {
            return Err(p.unexpected(&["MDGROUP", "MCGROUP", "DELETE", "DISPLAY"]));
        });
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Mrsets(subcommands))
}
