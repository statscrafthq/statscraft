//! Parsers of WEIGHTED KAPPA and FLEISS MULTIRATER KAPPA: agreement between raters.
//!
//! Rules from the Command Syntax Reference, in our words:
//! - WEIGHTED KAPPA takes one list (every pair of different variables, the earlier one as the
//!   row) or two lists joined by BY (every row variable with every column variable, a variable
//!   paired with itself skipped). A single list needs two different variables.
//! - FLEISS MULTIRATER KAPPA takes one list of at least two rating variables, optionally written
//!   as VARIABLES=list.
//! - Settings may be written `KEYWORD=value` or `KEYWORD(value)`. The confidence level spellings
//!   ASYMPTOTICCILEVEL and ASYMPTOTIC_CILEVEL and the case spellings IGNORECASE and IGNORE_CASE
//!   are both accepted; the level lies strictly between 0 and 100 (default 95).
//! - MISSING CLASSMISSING is TABLE (default) or INCLUDE on WEIGHTED KAPPA and EXCLUDE (default)
//!   or INCLUDE on FLEISS MULTIRATER KAPPA. Subcommands come in any order; a repeated setting
//!   keeps its last value.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::Punct;
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, PartialEq)]
pub struct WeightedKappa {
    /// The single list, or the row list before BY.
    pub rows: Vec<VarItem>,
    /// The column list after BY.
    pub columns: Option<Vec<VarItem>>,
    /// `CLASSMISSING=INCLUDE`: user-missing values are categories.
    pub include: bool,
    pub level: f64,
    /// `WEIGHTING=QUADRATIC`; LINEAR is the default.
    pub quadratic: bool,
    pub ignore_case: bool,
    /// `TABULATION`: which pairs get a crosstabulation.
    pub tabulation: Option<KappaTabulation>,
    /// `CASE(LOWER)`
    pub lower: bool,
    /// `ORDER(DESCENDING)`
    pub descending: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KappaTabulation {
    All,
    Pairs(Vec<VarItem>, Vec<VarItem>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FleissKappa {
    pub variables: Vec<VarItem>,
    /// `CLASSMISSING=INCLUDE`
    pub include: bool,
    pub level: f64,
    pub ignore_case: bool,
    /// `PRINT CATEGORY`: the table of the single categories.
    pub category: bool,
    /// `CASE(LOWER)`
    pub lower: bool,
}

/// The value of a setting written `=value` or `(value)`.
fn setting<T>(p: &mut Parser, read: impl FnOnce(&mut Parser) -> PResult<T>) -> PResult<T> {
    let parenthesized = p.eat(Punct::LParen);
    if !parenthesized {
        p.expect(Punct::Eq)?;
    }
    let value = read(p)?;
    if parenthesized {
        p.expect(Punct::RParen)?;
    }
    Ok(value)
}

/// One of `options`, by its position.
fn choice(p: &mut Parser, options: &[&str]) -> PResult<usize> {
    let Some(i) = options.iter().position(|o| p.at_keyword(o)) else {
        return Err(p.unexpected(options));
    };
    p.pos += 1;
    Ok(i)
}

fn level(p: &mut Parser) -> PResult<f64> {
    let start = p.pos;
    let Some(value) = p.number() else {
        return Err(p.unexpected(&["a confidence level"]));
    };
    if !(value > 0.0 && value < 100.0) {
        let message = format!("the confidence level must lie between 0 and 100, got {value}");
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(value)
}

/// The leading `[/]VARIABLES=` some examples write before the list.
fn variables_keyword(p: &mut Parser) {
    let slash = usize::from(p.at(Punct::Slash));
    let named = matches!(p.peek_at(slash), Some(crate::lexer::TokenKind::Word(w))
        if crate::table::matches_keyword(w, "VARIABLES"));
    if named && p.peek_at(slash + 1) == Some(&crate::lexer::TokenKind::Punct(Punct::Eq)) {
        p.pos += slash + 2;
    }
}

/// The shared CRITERIA keywords: the level and the case of strings. False when the keyword is
/// neither.
fn criterion(p: &mut Parser, level_out: &mut f64, ignore_case: &mut bool) -> PResult<bool> {
    if p.at_keyword("ASYMPTOTICCILEVEL") || p.at_keyword("ASYMPTOTIC_CILEVEL") {
        p.pos += 1;
        *level_out = setting(p, level)?;
    } else if p.at_keyword("IGNORECASE") || p.at_keyword("IGNORE_CASE") {
        p.pos += 1;
        *ignore_case = setting(p, |p| choice(p, &["TRUE", "FALSE"]))? == 0;
    } else {
        return Ok(false);
    }
    Ok(true)
}

fn case_setting(p: &mut Parser) -> PResult<bool> {
    Ok(setting(p, |p| choice(p, &["UPPER", "LOWER"]))? == 1)
}

fn missing(p: &mut Parser, default: &str) -> PResult<bool> {
    p.eat(Punct::Eq);
    if !p.eat_keyword("CLASSMISSING") {
        return Err(p.unexpected(&["CLASSMISSING"]));
    }
    Ok(setting(p, |p| choice(p, &[default, "INCLUDE"]))? == 1)
}

pub(crate) fn weighted_kappa(p: &mut Parser) -> PResult<Command> {
    variables_keyword(p);
    let rows = list_until(p, &["BY"])?;
    let columns = if p.eat_word("BY") {
        Some(p.var_list()?)
    } else {
        None
    };
    let mut command = WeightedKappa {
        rows,
        columns,
        include: false,
        level: 95.0,
        quadratic: false,
        ignore_case: false,
        tabulation: None,
        lower: false,
        descending: false,
    };
    while p.eat(Punct::Slash) {
        if p.eat_keyword("MISSING") {
            command.include = missing(p, "TABLE")?;
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if !criterion(p, &mut command.level, &mut command.ignore_case)? {
                    if !p.eat_keyword("WEIGHTING") {
                        return Err(p.unexpected(&[
                            "ASYMPTOTICCILEVEL",
                            "WEIGHTING",
                            "IGNORECASE",
                        ]));
                    }
                    command.quadratic = setting(p, |p| choice(p, &["LINEAR", "QUADRATIC"]))? == 1;
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("TABULATION") {
                    command.tabulation = Some(setting(p, |p| {
                        if p.eat_word("ALL") {
                            return Ok(KappaTabulation::All);
                        }
                        let rows = list_until(p, &["BY"])?;
                        if !p.eat_word("BY") {
                            return Err(p.unexpected(&["BY"]));
                        }
                        Ok(KappaTabulation::Pairs(rows, p.var_list()?))
                    })?);
                } else if p.eat_keyword("CASE") {
                    command.lower = case_setting(p)?;
                } else if p.eat_keyword("ORDER") {
                    command.descending =
                        setting(p, |p| choice(p, &["ASCENDING", "DESCENDING"]))? == 1;
                } else {
                    return Err(p.unexpected(&["TABULATION", "CASE", "ORDER"]));
                }
                p.eat(Punct::Comma);
            }
        } else {
            return Err(p.subcommand_error("WEIGHTED KAPPA", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::WeightedKappa(Box::new(command)))
}

pub(crate) fn fleiss_kappa(p: &mut Parser) -> PResult<Command> {
    variables_keyword(p);
    let mut command = FleissKappa {
        variables: p.var_list()?,
        include: false,
        level: 95.0,
        ignore_case: false,
        category: false,
        lower: false,
    };
    while p.eat(Punct::Slash) {
        if p.eat_keyword("MISSING") {
            command.include = missing(p, "EXCLUDE")?;
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if !criterion(p, &mut command.level, &mut command.ignore_case)? {
                    return Err(p.unexpected(&["ASYMPTOTICCILEVEL", "IGNORECASE"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("CATEGORY") {
                    command.category = true;
                } else if p.eat_keyword("CASE") {
                    command.lower = case_setting(p)?;
                } else {
                    return Err(p.unexpected(&["CATEGORY", "CASE"]));
                }
                p.eat(Punct::Comma);
            }
        } else {
            return Err(p.subcommand_error("FLEISS MULTIRATER KAPPA", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::FleissKappa(Box::new(command)))
}
