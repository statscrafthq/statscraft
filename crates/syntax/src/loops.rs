//! Parsers of DO REPEAT, END REPEAT, LOOP, END LOOP, BREAK and VECTOR, and the expansion of
//! DO REPEAT bodies.

use crate::ast::{
    Command, DoRepeat, LoopHeader, LoopIndex, RepeatList, Value, VarItem, VectorSpec,
    VectorVariables,
};
use crate::commands::format;
use crate::error::SyntaxError;
use crate::lexer::{Punct, Token, TokenKind};
use crate::parser::{PResult, Parser};
use crate::split::RawCommand;
use crate::table::{Resolution, resolve_tokens};

/// A replacement list of DO REPEAT: variables, or values with `n1 TO n2` counted out.
fn repeat_list(p: &mut Parser) -> PResult<RepeatList> {
    if let Some(TokenKind::Word(_)) = p.peek() {
        return Ok(RepeatList::Variables(p.var_list()?));
    }
    let mut values = Vec::new();
    while let Some(value) = p.value() {
        if let Value::Number(low) = value
            && p.eat_word("TO")
        {
            let start = p.pos;
            let high = p
                .number()
                .ok_or_else(|| p.unexpected(&["a whole number"]))?;
            let whole = |n: f64| n.fract() == 0.0 && n.abs() < 1e9;
            if !whole(low) || !whole(high) || low > high || high - low > 1e5 {
                return Err(SyntaxError::new(
                    p.span_from(start - 2),
                    "a value range of DO REPEAT goes from a whole number up to a larger one",
                ));
            }
            values.extend((low as i64..=high as i64).map(|n| Value::Number(n as f64)));
        } else {
            values.push(value);
        }
        p.eat(Punct::Comma);
    }
    if values.is_empty() {
        return Err(p.unexpected(&["variable names", "values"]));
    }
    Ok(RepeatList::Values(values))
}

pub(crate) fn do_repeat(p: &mut Parser) -> PResult<Command> {
    let mut lists: Vec<(String, RepeatList)> = Vec::new();
    loop {
        let start = p.pos;
        let name = p.name("a stand-in name")?;
        if lists.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name)) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("stand-in {name} is given twice"),
            ));
        }
        p.expect(Punct::Eq)?;
        lists.push((name, repeat_list(p)?));
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::DoRepeat(DoRepeat {
        lists,
        body: Vec::new(),
        print: false,
    }))
}

pub(crate) fn loop_command(p: &mut Parser) -> PResult<Command> {
    let mut index = None;
    let is_index = matches!(p.peek(), Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case("IF"))
        && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq));
    if is_index {
        let variable = p.name("an index variable")?;
        p.pos += 1;
        let first = p.expr()?;
        let (mut last, mut by) = (None, None);
        loop {
            if last.is_none() && p.eat_word("TO") {
                last = Some(p.expr()?);
            } else if by.is_none() && p.eat_word("BY") {
                by = Some(p.expr()?);
            } else {
                break;
            }
        }
        let Some(last) = last else {
            return Err(p.unexpected(&["TO"]));
        };
        index = Some(LoopIndex {
            variable,
            first,
            last,
            by,
        });
    }
    let condition = if p.eat_keyword("IF") {
        Some(p.expr()?)
    } else {
        None
    };
    p.finish(&["TO", "BY", "IF"])?;
    Ok(Command::Loop(LoopHeader { index, condition }))
}

pub(crate) fn end_loop(p: &mut Parser) -> PResult<Command> {
    let condition = if p.eat_keyword("IF") {
        Some(p.expr()?)
    } else {
        None
    };
    p.finish(&["IF"])?;
    Ok(Command::EndLoop(condition))
}

pub(crate) fn break_command(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::Break)
}

/// `(n [format])` or `(format, n)` of a short-form VECTOR, after the opening parenthesis.
fn vector_size(p: &mut Parser) -> PResult<(u32, Option<crate::ast::Format>)> {
    let (mut count, mut shown) = (None, None);
    while !p.eat(Punct::RParen) {
        if p.eat(Punct::Comma) {
            continue;
        }
        match p.peek() {
            Some(TokenKind::Number(n)) if count.is_none() && n.fract() == 0.0 && *n >= 1.0 => {
                if *n > 10_000.0 {
                    return Err(SyntaxError::new(
                        p.span(),
                        "a vector has at most 10000 elements",
                    ));
                }
                count = Some(*n as u32);
                p.pos += 1;
            }
            Some(TokenKind::Word(word)) if shown.is_none() => {
                let parsed = format(word).map_err(|m| SyntaxError::new(p.span(), m))?;
                shown = Some(parsed);
                p.pos += 1;
            }
            _ => return Err(p.unexpected(&["the number of elements", "a format", "')'"])),
        }
    }
    let count = count.ok_or_else(|| p.unexpected(&["the number of elements"]))?;
    Ok((count, shown))
}

pub(crate) fn vector(p: &mut Parser) -> PResult<Command> {
    let mut specs = Vec::new();
    loop {
        let mut names = vec![p.name("a vector name")?];
        while p.eat(Punct::Comma) || matches!(p.peek(), Some(TokenKind::Word(_))) {
            names.push(p.name("a vector name")?);
        }
        let variables = if p.eat(Punct::Eq) {
            if names.len() > 1 {
                return Err(p.unexpected(&["'('"]));
            }
            let start = p.pos;
            let first = p.name("a variable name")?;
            if !p.eat_word("TO") {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "VECTOR needs its variables as a range: first TO last",
                ));
            }
            let last = p.name("a variable name")?;
            VectorVariables::Existing(VarItem::Range(first, last))
        } else if p.eat(Punct::LParen) {
            let (count, format) = vector_size(p)?;
            VectorVariables::New { count, format }
        } else {
            return Err(p.unexpected(&["'='", "'('"]));
        };
        specs.push(VectorSpec { names, variables });
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Vector(specs))
}

/// The tokens that stand for one item of a replacement list.
fn replacement(item: &RepeatItem) -> Vec<TokenKind> {
    match item {
        RepeatItem::Name(name) => vec![TokenKind::Word(name.clone())],
        RepeatItem::Value(Value::Number(n)) if *n < 0.0 => {
            vec![TokenKind::Punct(Punct::Minus), TokenKind::Number(-n)]
        }
        RepeatItem::Value(Value::Number(n)) => vec![TokenKind::Number(*n)],
        RepeatItem::Value(Value::String(s)) => vec![TokenKind::Str(s.clone())],
    }
}

/// One item of an expanded replacement list.
#[derive(Debug, Clone, PartialEq)]
pub enum RepeatItem {
    Name(String),
    Value(Value),
}

/// A command of a DO REPEAT body with every stand-in after its name replaced by its item.
pub fn substitute(raw: &RawCommand, items: &[(&str, &RepeatItem)]) -> RawCommand {
    let used = match resolve_tokens(&raw.tokens) {
        Resolution::Supported(_, used) | Resolution::Unsupported(_, used) => used,
        _ => 0,
    };
    let mut tokens: Vec<Token> = raw.tokens[..used].to_vec();
    for token in &raw.tokens[used..] {
        let found = match &token.kind {
            TokenKind::Word(word) => items.iter().find(|(n, _)| n.eq_ignore_ascii_case(word)),
            _ => None,
        };
        match found {
            Some((_, item)) => tokens.extend(replacement(item).into_iter().map(|kind| Token {
                kind,
                span: token.span,
            })),
            None => tokens.push(token.clone()),
        }
    }
    RawCommand {
        tokens,
        ..raw.clone()
    }
}

/// A command as text, its tokens separated by blanks.
pub fn command_text(raw: &RawCommand) -> String {
    let text = |kind: &TokenKind| match kind {
        TokenKind::Word(w) => w.clone(),
        TokenKind::Number(n) => n.to_string(),
        TokenKind::Str(s) => format!("'{}'", s.replace('\'', "''")),
        TokenKind::Punct(p) => p.as_str().to_string(),
    };
    let words: Vec<String> = raw.tokens.iter().map(|t| text(&t.kind)).collect();
    words.join(" ")
}

/// The first item of a list, to check the body of DO REPEAT before it runs.
pub(crate) fn sample_item(list: &RepeatList) -> RepeatItem {
    match list {
        RepeatList::Variables(items) => match items.first() {
            Some(VarItem::Name(n) | VarItem::Range(n, _)) => RepeatItem::Name(n.clone()),
            _ => RepeatItem::Name("x".into()),
        },
        RepeatList::Values(values) => RepeatItem::Value(values[0].clone()),
    }
}
