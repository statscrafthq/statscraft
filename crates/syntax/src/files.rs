//! Parsers of the commands that combine data files: ADD FILES and MATCH FILES.

use crate::ast::{Command, Merge, MergeInput, Subset, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

const UNSUPPORTED: [&str; 5] = ["IN", "FIRST", "LAST", "MAP", "SORT"];

/// `(oldlist = newlist) ...` or `old = new ...`, up to the next '/'.
fn renames(p: &mut Parser) -> PResult<Vec<(Vec<VarItem>, Vec<VarItem>)>> {
    let mut groups = Vec::new();
    while !matches!(p.peek(), None | Some(TokenKind::Punct(Punct::Slash))) {
        if p.eat(Punct::LParen) {
            let old = p.var_list()?;
            p.expect(Punct::Eq)?;
            let new = p.var_list()?;
            p.expect(Punct::RParen)?;
            groups.push((old, new));
        } else {
            let old = p.name("'(' or a variable name")?;
            p.expect(Punct::Eq)?;
            let new = p.name("a new variable name")?;
            groups.push((vec![VarItem::Name(old)], vec![VarItem::Name(new)]));
        }
    }
    if groups.is_empty() {
        return Err(p.unexpected(&["(old = new)"]));
    }
    Ok(groups)
}

fn merge(p: &mut Parser, command: &str, tables: bool) -> PResult<Merge> {
    let mut merge = Merge {
        inputs: Vec::new(),
        by: Vec::new(),
        subsets: Vec::new(),
    };
    p.eat(Punct::Slash);
    loop {
        let start = p.pos;
        let table = tables && p.eat_keyword("TABLE");
        if table || p.eat_keyword("FILE") {
            p.eat(Punct::Eq);
            let file = if p.eat(Punct::Star) {
                None
            } else {
                Some(p.string("'*' or a quoted file path")?)
            };
            merge.inputs.push(MergeInput {
                file,
                table,
                rename: Vec::new(),
            });
        } else if p.eat_keyword("RENAME") {
            p.eat(Punct::Eq);
            let groups = renames(p)?;
            let Some(input) = merge.inputs.last_mut() else {
                let message = format!("{command} /RENAME must follow the /FILE it renames in");
                return Err(SyntaxError::new(p.span_from(start), message));
            };
            input.rename.extend(groups);
        } else if p.eat_keyword("BY") {
            p.eat(Punct::Eq);
            merge.by.extend(p.var_list()?);
        } else if p.eat_keyword("KEEP") {
            p.eat(Punct::Eq);
            merge.subsets.push(Subset::Keep(p.var_list()?));
        } else if p.eat_keyword("DROP") {
            p.eat(Punct::Eq);
            merge.subsets.push(Subset::Drop(p.var_list()?));
        } else if merge.inputs.is_empty() {
            return Err(p.unexpected(&["FILE"]));
        } else {
            return Err(p.subcommand_error(command, &UNSUPPORTED));
        }
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    if merge.inputs.iter().all(|input| input.table) {
        return Err(p.unexpected(&["/FILE='path' or /FILE=*"]));
    }
    if merge.by.is_empty() && merge.inputs.iter().any(|input| input.table) {
        let mut error = SyntaxError::new(p.span(), "MATCH FILES /TABLE needs /BY key variables");
        error.expected = vec!["/BY varlist".to_string()];
        return Err(error);
    }
    Ok(merge)
}

pub(crate) fn add_files(p: &mut Parser) -> PResult<Command> {
    merge(p, "ADD FILES", false).map(Command::AddFiles)
}

pub(crate) fn match_files(p: &mut Parser) -> PResult<Command> {
    merge(p, "MATCH FILES", true).map(Command::MatchFiles)
}
