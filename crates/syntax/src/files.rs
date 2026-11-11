//! Parsers of the commands that combine data files: ADD FILES, MATCH FILES and STAR JOIN.
//!
//! STAR JOIN, from the Command Syntax Reference: the subcommands SELECT, FROM, JOIN (one or
//! more), OUTFILE and the optional PASSPROTECT, in that order, every keyword in full (no
//! abbreviations). A field is `alias.name`; the name may hold periods itself (`t1.K9.1`), the
//! alias is the one FROM or JOIN gave. SELECT * is every field. ON compares a field of the FROM
//! source with one of its JOIN source, `=` only, several joined with AND.

use crate::ast::{Command, JoinField, JoinSource, Merge, MergeInput, StarJoin, Subset, VarItem};
use crate::error::{Span, SyntaxError};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};

const UNSUPPORTED: [&str; 5] = ["IN", "FIRST", "LAST", "MAP", "SORT"];

/// `(oldlist = newlist) ...` or `old = new ...`, up to the next '/'.
pub(crate) fn renames(p: &mut Parser) -> PResult<Vec<(Vec<VarItem>, Vec<VarItem>)>> {
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

/// A quoted file path, or the unquoted name of a dataset (DATASET NAME or DECLARE).
pub(crate) fn dataset_or_file(p: &mut Parser) -> PResult<String> {
    if let Some(TokenKind::Word(word)) = p.peek() {
        let word = word.clone();
        p.pos += 1;
        return Ok(word);
    }
    p.string("'*', a quoted file path or a dataset name")
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
                Some(dataset_or_file(p)?)
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
        return Err(p.unexpected(&["/FILE='path', /FILE=dataset or /FILE=*"]));
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

/// `/NAME` of STAR JOIN, the slash optional before the first.
fn join_subcommand(p: &mut Parser, name: &str) -> PResult<()> {
    let start = p.pos;
    p.eat(Punct::Slash);
    if p.eat_word(name) {
        return Ok(());
    }
    p.pos = start;
    Err(p.unexpected(&[&format!("/{name}")]))
}

/// A field as written, `alias.name`, with where it stands for errors.
struct Written {
    span: Span,
    text: String,
}

fn written(p: &mut Parser, what: &str) -> PResult<Written> {
    match p.peek() {
        Some(TokenKind::Word(w)) if w.contains('.') => {
            let field = Written {
                span: p.span(),
                text: w.clone(),
            };
            p.pos += 1;
            Ok(field)
        }
        _ => Err(p.unexpected(&[what])),
    }
}

/// `'file' [PASSWORD='text'] AS alias`, `dataset AS alias` or `* AS alias`. A password is read
/// and dropped: encrypted data files cannot be read, and an unencrypted file ignores it.
fn join_source(p: &mut Parser) -> PResult<JoinSource> {
    let file = if p.eat(Punct::Star) {
        None
    } else {
        Some(dataset_or_file(p)?)
    };
    if p.eat_word("PASSWORD") {
        p.expect(Punct::Eq)?;
        p.string("a quoted password")?;
    }
    if !p.eat_word("AS") {
        return Err(p.unexpected(&["AS alias"]));
    }
    let alias = p.name("an alias")?;
    Ok(JoinSource {
        file,
        alias,
        on: Vec::new(),
        flag: None,
    })
}

/// The source index and the field name of `alias.name`; the longest alias that fits wins, as an
/// alias may hold a period too.
fn resolve_field(sources: &[JoinSource], field: &Written) -> PResult<(usize, String)> {
    let found = sources
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            let prefix = field.text.get(..s.alias.len() + 1);
            prefix.is_some_and(|f| {
                f.ends_with('.') && f[..s.alias.len()].eq_ignore_ascii_case(&s.alias)
            })
        })
        .max_by_key(|(_, s)| s.alias.len());
    match found {
        Some((i, s)) if field.text.len() > s.alias.len() + 1 => {
            Ok((i, field.text[s.alias.len() + 1..].to_string()))
        }
        _ => {
            let aliases: Vec<&str> = sources.iter().map(|s| s.alias.as_str()).collect();
            let message = format!(
                "{} names no field of a source: write alias.name with an alias of FROM or JOIN ({})",
                field.text,
                aliases.join(", ")
            );
            Err(SyntaxError::new(field.span, message))
        }
    }
}

pub(crate) fn star_join(p: &mut Parser) -> PResult<Command> {
    join_subcommand(p, "SELECT")?;
    // the fields stay as written until FROM and JOIN have named the aliases
    let mut selected: Vec<(Written, Option<String>)> = Vec::new();
    let all = p.eat(Punct::Star);
    if !all {
        loop {
            let field = written(p, "alias.field or *")?;
            let rename = match p.eat_word("AS") {
                true => Some(p.name("a new variable name")?),
                false => None,
            };
            selected.push((field, rename));
            if !p.eat(Punct::Comma) {
                break;
            }
        }
    }
    join_subcommand(p, "FROM")?;
    let mut sources = vec![join_source(p)?];
    let mut keys: Vec<Vec<(Written, Written)>> = Vec::new();
    while p.at(Punct::Slash)
        && matches!(p.peek_at(1), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("JOIN"))
    {
        p.pos += 2;
        let mut source = join_source(p)?;
        if !p.eat_word("ON") {
            return Err(p.unexpected(&["ON alias.key = alias.key"]));
        }
        let mut pairs = Vec::new();
        loop {
            let left = written(p, "alias.key")?;
            p.expect(Punct::Eq)?;
            pairs.push((left, written(p, "alias.key")?));
            if !p.eat_word("AND") {
                break;
            }
        }
        if p.eat_word("IN") {
            p.expect(Punct::Eq)?;
            source.flag = Some(p.name("a new variable name")?);
        }
        sources.push(source);
        keys.push(pairs);
    }
    if sources.len() == 1 {
        return Err(p.unexpected(&["/JOIN"]));
    }
    join_subcommand(p, "OUTFILE")?;
    if !p.eat_word("FILE") {
        return Err(p.unexpected(&["FILE="]));
    }
    p.expect(Punct::Eq)?;
    let outfile = match p.eat(Punct::Star) {
        true => None,
        false => Some(dataset_or_file(p)?),
    };
    let mut encrypt = false;
    if p.at(Punct::Slash)
        && matches!(p.peek_at(1), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("PASSPROTECT"))
    {
        p.pos += 2;
        let mut password = false;
        loop {
            if p.eat_word("PASSWORD") {
                p.expect(Punct::Eq)?;
                encrypt = !p.eat_word("NONE");
                if encrypt {
                    p.string("a quoted password or NONE")?;
                }
                password = true;
            } else if p.eat_word("ENCRYPTEDPW") {
                p.expect(Punct::Eq)?;
                if !p.eat_word("YES") && !p.eat_word("NO") {
                    return Err(p.unexpected(&["YES", "NO"]));
                }
            } else {
                break;
            }
        }
        if !password {
            return Err(p.unexpected(&["PASSWORD="]));
        }
    }
    p.finish(&["/PASSPROTECT"])?;

    for (i, source) in sources.iter().enumerate() {
        if sources[..i]
            .iter()
            .any(|s| s.alias.eq_ignore_ascii_case(&source.alias))
        {
            let message = format!(
                "the alias {} is given twice; every FROM and JOIN needs its own",
                source.alias
            );
            return Err(SyntaxError::new(p.span(), message));
        }
    }
    for (j, pairs) in keys.iter().enumerate() {
        let join = j + 1;
        for (left, right) in pairs {
            let (a, b) = (
                resolve_field(&sources, left)?,
                resolve_field(&sources, right)?,
            );
            let pair = match (a.0, b.0) {
                (0, s) if s == join => (a.1, b.1),
                (s, 0) if s == join => (b.1, a.1),
                _ => {
                    let message = format!(
                        "ON compares a field of FROM ({}) with one of this JOIN ({}): {} = {}",
                        sources[0].alias, sources[join].alias, left.text, right.text
                    );
                    return Err(SyntaxError::new(left.span, message));
                }
            };
            sources[join].on.push(pair);
        }
    }
    let select = match all {
        true => None,
        false => {
            let mut fields: Vec<JoinField> = Vec::new();
            for (field, rename) in selected {
                let (source, name) = resolve_field(&sources, &field)?;
                // a field named twice for the same source is taken once
                let twice = fields
                    .iter()
                    .any(|f| f.source == source && f.name.eq_ignore_ascii_case(&name));
                if !twice {
                    fields.push(JoinField {
                        source,
                        name,
                        rename,
                    });
                }
            }
            Some(fields)
        }
    };
    Ok(Command::StarJoin(StarJoin {
        select,
        sources,
        outfile,
        encrypt,
    }))
}
