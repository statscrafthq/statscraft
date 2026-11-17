//! Parser of PLS: partial least squares regression.
//!
//! Rules from the Command Syntax Reference (PLS), in our words:
//! - `PLS dependents [BY factors] [WITH covariates]`. A dependent may carry `MLEVEL=N|O|S` and
//!   `REFERENCE=FIRST|LAST|value`, which belong to the variable just before them (the last one
//!   of a TO range). `ALL MLEVEL=...` or `ALL REFERENCE=...` sets them for every dependent;
//!   ALL without them is the list of all variables.
//! - BY or WITH with nothing after it is ignored with a warning (the engine gives it).
//! - Subcommands: ID VARIABLE=name, MODEL effects, OUTDATASET CASES=, LATENTFACTORS= and
//!   PREDICTORS= (dataset names, not files), CRITERIA LATENTFACTORS=n (5). Each at most once,
//!   each keyword at most once, none empty, all spelled in full, the equals signs required.
//! - MODEL: main effects and interactions joined by `*` or BY; no nesting, no variable twice in
//!   one term.

use crate::ast::{Command, Value, VarItem};
use crate::error::SyntaxError;
use crate::glm::Term;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlsLevel {
    Nominal,
    Ordinal,
    Scale,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlsReference {
    First,
    Last,
    Value(Value),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlsDependent {
    pub item: VarItem,
    pub level: Option<PlsLevel>,
    pub reference: Option<PlsReference>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pls {
    pub dependents: Vec<PlsDependent>,
    /// `ALL MLEVEL=...` and `ALL REFERENCE=...`: for every dependent without its own.
    pub all_level: Option<PlsLevel>,
    pub all_reference: Option<PlsReference>,
    pub factors: Vec<VarItem>,
    pub covariates: Vec<VarItem>,
    /// BY or WITH written with no variables after it.
    pub empty_by: bool,
    pub empty_with: bool,
    pub id: Option<String>,
    pub model: Option<Vec<Term>>,
    pub out_cases: Option<String>,
    pub out_factors: Option<String>,
    pub out_predictors: Option<String>,
    pub latent_factors: usize,
}

/// True when the next tokens are `word =`.
fn at_setting(p: &Parser, word: &str) -> bool {
    matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(word))
        && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq))
}

fn once(p: &Parser, start: usize, seen: bool, what: &str) -> PResult<()> {
    if seen {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} is given more than once"),
        ));
    }
    Ok(())
}

/// MLEVEL and REFERENCE after a dependent.
fn settings(
    p: &mut Parser,
    level: &mut Option<PlsLevel>,
    reference: &mut Option<PlsReference>,
) -> PResult<()> {
    loop {
        let start = p.pos;
        if at_setting(p, "MLEVEL") {
            p.pos += 2;
            once(p, start, level.is_some(), "MLEVEL")?;
            *level = Some(if p.eat_word("N") {
                PlsLevel::Nominal
            } else if p.eat_word("O") {
                PlsLevel::Ordinal
            } else if p.eat_word("S") {
                PlsLevel::Scale
            } else {
                return Err(p.unexpected(&["N", "O", "S"]));
            });
        } else if at_setting(p, "REFERENCE") {
            p.pos += 2;
            once(p, start, reference.is_some(), "REFERENCE")?;
            *reference = Some(if p.eat_word("FIRST") {
                PlsReference::First
            } else if p.eat_word("LAST") {
                PlsReference::Last
            } else if let Some(v) = p.value() {
                PlsReference::Value(v)
            } else {
                return Err(p.unexpected(&["FIRST", "LAST", "a value"]));
            });
        } else {
            return Ok(());
        }
    }
}

/// A variable list after BY or WITH, up to `stop`; empty when nothing follows.
fn predictors(p: &mut Parser, stop: &str) -> PResult<Vec<VarItem>> {
    match p.peek() {
        Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case(stop) => {
            crate::glm::list_until(p, &[stop])
        }
        _ => Ok(Vec::new()),
    }
}

fn model(p: &mut Parser) -> PResult<Vec<Term>> {
    let mut out = Vec::new();
    while !at_end(p) {
        let start = p.pos;
        let mut names = vec![p.name("a predictor")?];
        loop {
            if p.at(Punct::LParen) || p.at_keyword("WITHIN") {
                return Err(SyntaxError::new(p.span(), "PLS does not take nested terms"));
            }
            if !(p.eat(Punct::Star) || p.eat_word("BY")) {
                break;
            }
            let name = p.name("a predictor")?;
            if names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!("{name} appears twice in one interaction"),
                ));
            }
            names.push(name);
        }
        out.push(Term {
            label: names.join(" * "),
            variables: names,
        });
        p.eat(Punct::Comma);
    }
    Ok(out)
}

/// `KEYWORD = name` of OUTDATASET: a dataset name, never a file.
fn dataset_name(p: &mut Parser) -> PResult<String> {
    p.expect(Punct::Eq)?;
    if let Some(TokenKind::Str(_)) = p.peek() {
        return Err(SyntaxError::new(
            p.span(),
            "OUTDATASET takes a dataset name, not a file",
        ));
    }
    p.name("a dataset name")
}

pub(crate) fn pls(p: &mut Parser) -> PResult<Command> {
    let mut command = Pls {
        dependents: Vec::new(),
        all_level: None,
        all_reference: None,
        factors: Vec::new(),
        covariates: Vec::new(),
        empty_by: false,
        empty_with: false,
        id: None,
        model: None,
        out_cases: None,
        out_factors: None,
        out_predictors: None,
        latent_factors: 5,
    };
    while let Some(TokenKind::Word(w)) = p.peek() {
        if w.eq_ignore_ascii_case("BY") || w.eq_ignore_ascii_case("WITH") {
            break;
        }
        if p.eat_word("ALL") {
            if at_setting(p, "MLEVEL") || at_setting(p, "REFERENCE") {
                settings(p, &mut command.all_level, &mut command.all_reference)?;
                continue;
            }
            command.dependents.push(PlsDependent {
                item: VarItem::All,
                level: None,
                reference: None,
            });
            continue;
        }
        let first = p.name("a dependent variable")?;
        let item = if p.eat_word("TO") {
            VarItem::Range(first, p.name("a variable name")?)
        } else {
            VarItem::Name(first)
        };
        let mut dependent = PlsDependent {
            item,
            level: None,
            reference: None,
        };
        settings(p, &mut dependent.level, &mut dependent.reference)?;
        command.dependents.push(dependent);
        p.eat(Punct::Comma);
    }
    if command.dependents.is_empty() {
        return Err(p.unexpected(&["a dependent variable"]));
    }
    if p.eat_word("BY") {
        command.factors = predictors(p, "WITH")?;
        command.empty_by = command.factors.is_empty();
    }
    if p.eat_word("WITH") {
        command.covariates = predictors(p, "")?;
        command.empty_with = command.covariates.is_empty();
    }
    let mut seen: Vec<&str> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        let name = ["ID", "MODEL", "OUTDATASET", "CRITERIA"]
            .into_iter()
            .find(|n| p.eat_word(n));
        let Some(name) = name else {
            return Err(p.subcommand_error("PLS", &[]));
        };
        once(
            p,
            start,
            seen.contains(&name),
            &format!("subcommand {name}"),
        )?;
        seen.push(name);
        if at_end(p) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("subcommand {name} is empty"),
            ));
        }
        match name {
            "ID" => {
                if !p.eat_word("VARIABLE") {
                    return Err(p.unexpected(&["VARIABLE"]));
                }
                p.expect(Punct::Eq)?;
                command.id = Some(p.name("a variable name")?);
            }
            "MODEL" => {
                p.eat(Punct::Eq);
                let terms = model(p)?;
                if terms.is_empty() {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "subcommand MODEL is empty",
                    ));
                }
                command.model = Some(terms);
            }
            "OUTDATASET" => {
                while !at_end(p) {
                    let at = p.pos;
                    let slot = if p.eat_word("CASES") {
                        &mut command.out_cases
                    } else if p.eat_word("LATENTFACTORS") {
                        &mut command.out_factors
                    } else if p.eat_word("PREDICTORS") {
                        &mut command.out_predictors
                    } else {
                        return Err(p.unexpected(&["CASES", "LATENTFACTORS", "PREDICTORS"]));
                    };
                    let given = slot.is_some();
                    let name = dataset_name(p)?;
                    once(p, at, given, "this OUTDATASET keyword")?;
                    *slot = Some(name);
                }
                let names: Vec<&String> = [
                    &command.out_cases,
                    &command.out_factors,
                    &command.out_predictors,
                ]
                .into_iter()
                .flatten()
                .collect();
                for (i, a) in names.iter().enumerate() {
                    if names[..i].iter().any(|b| b.eq_ignore_ascii_case(a)) {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            format!("OUTDATASET names the dataset {a} twice"),
                        ));
                    }
                }
            }
            _ => {
                if !p.eat_word("LATENTFACTORS") {
                    return Err(p.unexpected(&["LATENTFACTORS"]));
                }
                p.expect(Punct::Eq)?;
                let at = p.pos;
                match p.number() {
                    Some(n) if n >= 1.0 && n.fract() == 0.0 && n <= 1e6 => {
                        command.latent_factors = n as usize;
                    }
                    Some(_) => {
                        return Err(SyntaxError::new(
                            p.span_from(at),
                            "LATENTFACTORS takes a positive whole number",
                        ));
                    }
                    None => return Err(p.unexpected(&["a positive whole number"])),
                }
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Pls(Box::new(command)))
}
