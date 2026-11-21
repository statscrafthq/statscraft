//! Parser of BOOTSTRAP: resampling that the next procedure runs on.
//!
//! Rules from the Command Syntax Reference, in our words:
//! - Every subcommand is optional, each may appear once and none may be empty. A keyword may
//!   not repeat within its subcommand. Command, subcommand and keyword names are written in
//!   full: no abbreviations.
//! - SAMPLING METHOD=SIMPLE (the default), STRATIFIED(STRATA=varlist), or RESIDUAL and WILD
//!   with (RESIDUALS=varlist) or (PREDICTED=varlist), one variable for each TARGET variable,
//!   which they need.
//! - VARIABLES TARGET=varlist INPUT=varlist: the variables whose missing values decide which
//!   cases take part. The older form ANALYSIS(TARGET=... INPUT=...) and SAMPLING(STRATA=...)
//!   inside VARIABLES is taken too.
//! - CRITERIA CILEVEL=number (0 up to, not including, 100; default 95), CITYPE=PERCENTILE
//!   (default) or BCA, NSAMPLES=whole number of at least 1 (default 1000).
//! - MISSING USERMISSING=EXCLUDE (default) or INCLUDE: whether user-missing values of nominal
//!   and ordinal variables count as valid.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, PartialEq)]
pub struct Bootstrap {
    /// STRATIFIED(STRATA=...); empty for SIMPLE.
    pub strata: Vec<VarItem>,
    pub target: Vec<VarItem>,
    pub input: Vec<VarItem>,
    /// In percent.
    pub cilevel: f64,
    /// CITYPE=BCA; percentile intervals otherwise.
    pub bca: bool,
    pub samples: u64,
    /// USERMISSING=INCLUDE
    pub include_user_missing: bool,
    /// RESIDUAL or WILD resampling: the targets are rebuilt from predicted values and
    /// residuals instead of drawing cases.
    pub residual: Option<ResidualSampling>,
}

/// `RESIDUAL(RESIDUALS=varlist | PREDICTED=varlist)` or the same with WILD, one variable for
/// each TARGET in the order of TARGET.
#[derive(Debug, Clone, PartialEq)]
pub struct ResidualSampling {
    /// WILD: a case's own residual with a random sign, not a residual drawn from all cases.
    pub wild: bool,
    /// PREDICTED: the variables hold predicted values, not residuals.
    pub predicted: bool,
    pub variables: Vec<VarItem>,
}

impl Default for Bootstrap {
    fn default() -> Self {
        Bootstrap {
            strata: Vec::new(),
            target: Vec::new(),
            input: Vec::new(),
            cilevel: 95.0,
            bca: false,
            samples: 1000,
            include_user_missing: false,
            residual: None,
        }
    }
}

/// Fails when `word` was seen before in this subcommand.
fn once(p: &Parser, seen: &mut Vec<&'static str>, word: &'static str) -> PResult<()> {
    if seen.contains(&word) {
        let message = format!("{word} appears twice in the same subcommand");
        return Err(SyntaxError::new(p.span(), message));
    }
    seen.push(word);
    Ok(())
}

/// `= varlist` after a keyword, up to the next `keyword =`.
fn list(p: &mut Parser) -> PResult<Vec<VarItem>> {
    p.expect(Punct::Eq)?;
    let mut items = Vec::new();
    while let Some(TokenKind::Word(_)) = p.peek() {
        if p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
            break;
        }
        if p.eat_word("ALL") {
            items.push(VarItem::All);
        } else {
            let first = p.name("a variable name")?;
            match p.eat_word("TO") {
                true => items.push(VarItem::Range(first, p.name("a variable name")?)),
                false => items.push(VarItem::Name(first)),
            }
        }
        p.eat(Punct::Comma);
    }
    if items.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(items)
}

/// `(STRATA = varlist)` after STRATIFIED or SAMPLING.
fn strata(p: &mut Parser) -> PResult<Vec<VarItem>> {
    p.expect(Punct::LParen)?;
    if !p.eat_word("STRATA") {
        return Err(p.unexpected(&["STRATA"]));
    }
    let items = list(p)?;
    p.expect(Punct::RParen)?;
    Ok(items)
}

pub(crate) fn bootstrap(p: &mut Parser) -> PResult<Command> {
    let mut b = Bootstrap::default();
    let mut subcommands: Vec<&'static str> = Vec::new();
    while p.eat(Punct::Slash) {
        let name = ["SAMPLING", "VARIABLES", "CRITERIA", "MISSING"]
            .into_iter()
            .find(|name| p.eat_word(name));
        let Some(name) = name else {
            return Err(p.subcommand_error("BOOTSTRAP", &[]));
        };
        once(p, &mut subcommands, name)?;
        if at_end(p) {
            let message = format!("the {name} subcommand of BOOTSTRAP is empty");
            return Err(SyntaxError::new(p.span(), message));
        }
        let mut seen = Vec::new();
        while !at_end(p) {
            match name {
                "SAMPLING" if p.eat_word("METHOD") => {
                    once(p, &mut seen, "METHOD")?;
                    p.expect(Punct::Eq)?;
                    if p.eat_word("STRATIFIED") {
                        b.strata = strata(p)?;
                    } else if p.at_keyword("RESIDUAL") || p.at_keyword("WILD") {
                        let wild = p.at_keyword("WILD");
                        p.pos += 1;
                        p.expect(Punct::LParen)?;
                        let predicted = if p.eat_word("RESIDUALS") {
                            false
                        } else if p.eat_word("PREDICTED") {
                            true
                        } else {
                            return Err(p.unexpected(&["RESIDUALS", "PREDICTED"]));
                        };
                        let variables = list(p)?;
                        p.expect(Punct::RParen)?;
                        b.residual = Some(ResidualSampling {
                            wild,
                            predicted,
                            variables,
                        });
                    } else if !p.eat_word("SIMPLE") {
                        return Err(p.unexpected(&["SIMPLE", "STRATIFIED", "RESIDUAL", "WILD"]));
                    }
                }
                "SAMPLING" => return Err(p.unexpected(&["METHOD"])),
                "VARIABLES" if p.eat_word("TARGET") => {
                    once(p, &mut seen, "TARGET")?;
                    b.target = list(p)?;
                }
                "VARIABLES" if p.eat_word("INPUT") => {
                    once(p, &mut seen, "INPUT")?;
                    b.input = list(p)?;
                }
                // the older form: ANALYSIS(TARGET=... INPUT[S]=...)
                "VARIABLES" if p.eat_word("ANALYSIS") => {
                    once(p, &mut seen, "ANALYSIS")?;
                    p.expect(Punct::LParen)?;
                    while !p.eat(Punct::RParen) {
                        if p.eat_word("TARGET") || p.eat_word("TARGETS") {
                            b.target = list(p)?;
                        } else if p.eat_word("INPUT") || p.eat_word("INPUTS") {
                            b.input = list(p)?;
                        } else {
                            return Err(p.unexpected(&["TARGET", "INPUT", "')'"]));
                        }
                    }
                }
                "VARIABLES" if p.eat_word("SAMPLING") => {
                    once(p, &mut seen, "SAMPLING")?;
                    b.strata = strata(p)?;
                }
                "VARIABLES" => return Err(p.unexpected(&["TARGET", "INPUT"])),
                "CRITERIA" if p.eat_word("CILEVEL") => {
                    once(p, &mut seen, "CILEVEL")?;
                    p.expect(Punct::Eq)?;
                    let start = p.pos;
                    match p.number() {
                        Some(level) if (0.0..100.0).contains(&level) => b.cilevel = level,
                        Some(level) => {
                            let message = format!(
                                "CILEVEL must lie from 0 up to (not including) 100, got {level}"
                            );
                            return Err(SyntaxError::new(p.span_from(start), message));
                        }
                        None => return Err(p.unexpected(&["a confidence level in percent"])),
                    }
                }
                "CRITERIA" if p.eat_word("CITYPE") => {
                    once(p, &mut seen, "CITYPE")?;
                    p.expect(Punct::Eq)?;
                    b.bca = if p.eat_word("BCA") {
                        true
                    } else if p.eat_word("PERCENTILE") {
                        false
                    } else {
                        return Err(p.unexpected(&["PERCENTILE", "BCA"]));
                    };
                }
                "CRITERIA" if p.eat_word("NSAMPLES") => {
                    once(p, &mut seen, "NSAMPLES")?;
                    p.expect(Punct::Eq)?;
                    b.samples = match p.peek() {
                        Some(TokenKind::Number(n)) if n.fract() == 0.0 && *n >= 1.0 && *n < 1e9 => {
                            let n = *n as u64;
                            p.pos += 1;
                            n
                        }
                        _ => return Err(p.unexpected(&["a whole number of samples"])),
                    };
                }
                "CRITERIA" => return Err(p.unexpected(&["CILEVEL", "CITYPE", "NSAMPLES"])),
                _ if p.eat_word("USERMISSING") => {
                    once(p, &mut seen, "USERMISSING")?;
                    p.expect(Punct::Eq)?;
                    b.include_user_missing = if p.eat_word("INCLUDE") {
                        true
                    } else if p.eat_word("EXCLUDE") {
                        false
                    } else {
                        return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
                    };
                }
                _ => return Err(p.unexpected(&["USERMISSING"])),
            }
            p.eat(Punct::Comma);
        }
    }
    p.finish(&["'/'"])?;
    if let Some(r) = &b.residual
        && b.target.is_empty()
    {
        return Err(SyntaxError::new(
            p.span(),
            format!(
                "{} resampling needs the target variables: give /VARIABLES TARGET=varlist",
                if r.wild { "WILD" } else { "RESIDUAL" }
            ),
        ));
    }
    Ok(Command::Bootstrap(Box::new(b)))
}
