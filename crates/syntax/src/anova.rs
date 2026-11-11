//! Parser of ANOVA (analysis of variance for factorial designs).
//!
//! Rules from the Command Syntax Reference:
//! - `[VARIABLES=] dependents BY factors(min,max) ... [WITH covariates]`; a range follows the
//!   factors it applies to, so `a, b(1,2)` gives both the range 1 to 2. More analysis lists may
//!   follow, each after a slash, with or without VARIABLES=.
//! - COVARIATES=FIRST (default) | WITH | AFTER; MAXORDERS=ALL (default, up to five-way) | n |
//!   NONE; METHOD=UNIQUE (default) | EXPERIMENTAL | HIERARCHICAL.
//! - STATISTICS without keywords is MEAN and REG; with keywords only those (MCA, REG, MEAN,
//!   ALL, NONE); left out it is NONE. MISSING=EXCLUDE (default) | INCLUDE.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::stats::variables;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CovariateOrder {
    First,
    With,
    After,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnovaMethod {
    Unique,
    Experimental,
    Hierarchical,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnovaList {
    pub dependents: Vec<VarItem>,
    /// Each factor list with its range.
    pub factors: Vec<(Vec<VarItem>, f64, f64)>,
    pub covariates: Vec<VarItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Anova {
    pub lists: Vec<AnovaList>,
    pub covariate_order: CovariateOrder,
    /// The highest order of interaction, 1 for NONE.
    pub max_order: usize,
    pub method: AnovaMethod,
    pub mean: bool,
    pub reg: bool,
    pub mca: bool,
    pub include: bool,
}

const SUBCOMMANDS: [&str; 6] = [
    "VARIABLES",
    "COVARIATES",
    "MAXORDERS",
    "METHOD",
    "STATISTICS",
    "MISSING",
];

fn analysis_list(p: &mut Parser) -> PResult<AnovaList> {
    let dependents = variables(p)?;
    if !p.eat_word("BY") {
        return Err(p.unexpected(&["BY"]));
    }
    let mut factors = Vec::new();
    while matches!(p.peek(), Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case("WITH")) {
        let items = variables(p)?;
        p.expect(Punct::LParen)?;
        let start = p.pos;
        let (Some(min), _, Some(max)) = (p.number(), p.eat(Punct::Comma), p.number()) else {
            return Err(p.unexpected(&["the smallest and the largest value of the factors"]));
        };
        if min > max {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("the range of a factor needs its smaller value first, got {min} and {max}"),
            ));
        }
        p.expect(Punct::RParen)?;
        p.eat(Punct::Comma);
        factors.push((items, min.trunc(), max.trunc()));
    }
    if factors.is_empty() {
        return Err(p.unexpected(&["a factor name"]));
    }
    let covariates = match p.eat_word("WITH") {
        true => variables(p)?,
        false => Vec::new(),
    };
    Ok(AnovaList {
        dependents,
        factors,
        covariates,
    })
}

/// Whether BY comes before the next slash.
fn list_ahead(p: &Parser) -> bool {
    (0..)
        .map_while(|i| match p.peek_at(i) {
            None | Some(TokenKind::Punct(Punct::Slash)) => None,
            Some(token) => Some(token),
        })
        .any(|t| matches!(t, TokenKind::Word(w) if w.eq_ignore_ascii_case("BY")))
}

pub(crate) fn anova(p: &mut Parser) -> PResult<Command> {
    let mut command = Anova {
        lists: Vec::new(),
        covariate_order: CovariateOrder::First,
        max_order: 5,
        method: AnovaMethod::Unique,
        mean: false,
        reg: false,
        mca: false,
        include: false,
    };
    let mut first = true;
    loop {
        if !first && !p.eat(Punct::Slash) {
            break;
        }
        if p.peek().is_none() {
            break;
        }
        // an analysis list always has BY, which tells it from a subcommand of the same name
        let at_subcommand = SUBCOMMANDS.iter().any(|k| p.at_keyword(k)) && !list_ahead(p);
        if first || !at_subcommand || p.at_keyword("VARIABLES") {
            if p.eat_keyword("VARIABLES") {
                p.eat(Punct::Eq);
            }
            command.lists.push(analysis_list(p)?);
        } else if p.eat_keyword("COVARIATES") {
            p.eat(Punct::Eq);
            command.covariate_order = if p.eat_keyword("FIRST") {
                CovariateOrder::First
            } else if p.eat_keyword("WITH") {
                CovariateOrder::With
            } else if p.eat_keyword("AFTER") {
                CovariateOrder::After
            } else {
                return Err(p.unexpected(&["FIRST", "WITH", "AFTER"]));
            };
        } else if p.eat_keyword("MAXORDERS") {
            p.eat(Punct::Eq);
            let start = p.pos;
            command.max_order = if p.eat_word("ALL") {
                5
            } else if p.eat_keyword("NONE") {
                1
            } else {
                match p.number() {
                    Some(n) if (1.0..=5.0).contains(&n) && n.fract() == 0.0 => n as usize,
                    Some(n) => {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            format!("MAXORDERS takes a whole number from 1 to 5, got {n}"),
                        ));
                    }
                    None => return Err(p.unexpected(&["ALL", "NONE", "a number"])),
                }
            };
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            command.method = if p.eat_keyword("UNIQUE") {
                AnovaMethod::Unique
            } else if p.eat_keyword("EXPERIMENTAL") {
                AnovaMethod::Experimental
            } else if p.eat_keyword("HIERARCHICAL") {
                AnovaMethod::Hierarchical
            } else {
                return Err(p.unexpected(&["UNIQUE", "EXPERIMENTAL", "HIERARCHICAL"]));
            };
        } else if p.eat_keyword("STATISTICS") {
            p.eat(Punct::Eq);
            let (mut mean, mut reg, mut mca) = (at_end(p), at_end(p), false);
            while !at_end(p) {
                if p.eat_keyword("MEAN") {
                    mean = true;
                } else if p.eat_word("REG") {
                    reg = true;
                } else if p.eat_word("MCA") {
                    mca = true;
                } else if p.eat_word("ALL") {
                    (mean, reg, mca) = (true, true, true);
                } else if p.eat_keyword("NONE") {
                    (mean, reg, mca) = (false, false, false);
                } else {
                    return Err(p.unexpected(&["MCA", "REG", "MEAN", "ALL", "NONE"]));
                }
                p.eat(Punct::Comma);
            }
            (command.mean, command.reg, command.mca) = (mean, reg, mca);
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("EXCLUDE") {
                    command.include = false;
                } else if p.eat_keyword("INCLUDE") {
                    command.include = true;
                } else {
                    return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
                }
                p.eat(Punct::Comma);
            }
        }
        first = false;
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.lists.is_empty() {
        return Err(p.unexpected(&["VARIABLES"]));
    }
    Ok(Command::Anova(Box::new(command)))
}
