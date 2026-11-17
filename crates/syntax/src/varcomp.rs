//! Parser of VARCOMP: variance components of a linear model with random factors.
//!
//! Rules (our notes from the Command Syntax Reference, VARCOMP):
//! - `dependent BY factors [WITH covariates]` first, at least one factor; then subcommands in
//!   any order. RANDOM is required and adds up over repeats; every other subcommand counts
//!   once, the last one winning (CRITERIA and PRINT add up keyword by keyword).
//! - METHOD: MINQUE(1) by default, MINQUE(0), ML, REML or SSTYPE(3 | 1); a keyword is required.
//! - INTERCEPT: INCLUDE (default) or EXCLUDE; INTERCEPT on DESIGN overrides EXCLUDE.
//! - MISSING: EXCLUDE (default) or INCLUDE. REGWGT names one variable, the last one written.
//! - CRITERIA: CONVERGE(1E-8), EPS(1E-8) and ITERATE(50), positive.
//! - PRINT: EMS, SS and HISTORY[(n)] (every n-th iteration, 1 by default).
//! - OUTFILE: VAREST and one of COVB or CORB, then a file or dataset in parentheses.
//! - DESIGN: terms as in UNIANOVA, INTERCEPT among them; empty means the default design.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::{Term, list_until, terms};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// How the variance components are estimated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarcompMethod {
    /// MINQUE with prior weight 0 or 1 on the random effects.
    Minque(u8),
    Ml,
    Reml,
    /// The ANOVA method with Type I or III sums of squares.
    Sstype(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VarcompPrint {
    pub ems: bool,
    pub ss: bool,
    /// HISTORY(n): every n-th iteration.
    pub history: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VarcompOutfile {
    pub varest: bool,
    /// COVB (false) or CORB (true).
    pub matrix: Option<bool>,
    pub file: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Varcomp {
    pub dependent: String,
    pub factors: Vec<VarItem>,
    pub covariates: Vec<VarItem>,
    pub random: Vec<String>,
    pub method: VarcompMethod,
    pub intercept: bool,
    pub include: bool,
    pub regwgt: Option<String>,
    pub converge: f64,
    pub eps: f64,
    pub iterate: usize,
    pub print: VarcompPrint,
    pub outfile: Option<VarcompOutfile>,
    /// DESIGN terms; None for the default design.
    pub design: Option<Vec<Term>>,
}

fn error(p: &Parser, start: usize, message: impl Into<String>) -> SyntaxError {
    SyntaxError::new(p.span_from(start), message)
}

/// `(number)` that must be one of `allowed`.
fn choice(p: &mut Parser, what: &str, allowed: &[f64]) -> PResult<u8> {
    let start = p.pos;
    p.expect(Punct::LParen)?;
    let n = p.number();
    p.expect(Punct::RParen)?;
    match n {
        Some(n) if allowed.contains(&n) => Ok(n as u8),
        _ => {
            let list: Vec<String> = allowed.iter().map(f64::to_string).collect();
            Err(error(p, start, format!("{what} is {}", list.join(" or "))))
        }
    }
}

/// `(value)` above 0, whole when `whole`.
fn positive(p: &mut Parser, what: &str, whole: bool) -> PResult<f64> {
    let start = p.pos;
    p.expect(Punct::LParen)?;
    let n = p.number();
    p.expect(Punct::RParen)?;
    match n {
        Some(n) if n > 0.0 && (!whole || n.fract() == 0.0) => Ok(n),
        _ if whole => Err(error(
            p,
            start,
            format!("{what} needs a positive whole number"),
        )),
        _ => Err(error(p, start, format!("{what} needs a positive value"))),
    }
}

fn outfile(p: &mut Parser) -> PResult<VarcompOutfile> {
    let mut out = VarcompOutfile {
        varest: false,
        matrix: None,
        file: String::new(),
    };
    while !p.at(Punct::LParen) {
        let start = p.pos;
        if p.eat_keyword("VAREST") {
            out.varest = true;
        } else if p.at_keyword("COVB") || p.at_keyword("CORB") {
            let corr = p.at_keyword("CORB");
            p.pos += 1;
            if out.matrix.is_some_and(|m| m != corr) {
                return Err(error(p, start, "OUTFILE takes COVB or CORB, not both"));
            }
            out.matrix = Some(corr);
        } else {
            return Err(p.unexpected(&["VAREST", "COVB", "CORB", "('file')"]));
        }
        p.eat(Punct::Comma);
    }
    if !out.varest && out.matrix.is_none() {
        return Err(p.unexpected(&["VAREST", "COVB", "CORB"]));
    }
    p.expect(Punct::LParen)?;
    out.file = p.file("a quoted file name or a dataset name")?;
    p.expect(Punct::RParen)?;
    Ok(out)
}

pub(crate) fn varcomp(p: &mut Parser) -> PResult<Command> {
    let dependent = p.name("the dependent variable")?;
    if !p.eat_word("BY") {
        return Err(SyntaxError::new(
            p.span(),
            "VARCOMP needs BY and at least one factor after the dependent variable",
        ));
    }
    let factors = list_until(p, &["WITH"])?;
    let covariates = if p.eat_word("WITH") {
        p.var_list()?
    } else {
        Vec::new()
    };
    let mut c = Varcomp {
        dependent,
        factors,
        covariates,
        random: Vec::new(),
        method: VarcompMethod::Minque(1),
        intercept: true,
        include: false,
        regwgt: None,
        converge: 1e-8,
        eps: 1e-8,
        iterate: 50,
        print: VarcompPrint::default(),
        outfile: None,
        design: None,
    };
    while p.eat(Punct::Slash) {
        if p.eat_keyword("RANDOM") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                let name = p.name("a factor")?;
                if !c.random.iter().any(|r| r.eq_ignore_ascii_case(&name)) {
                    c.random.push(name);
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            c.method = if p.eat_keyword("MINQUE") {
                if p.at(Punct::LParen) {
                    VarcompMethod::Minque(choice(p, "MINQUE", &[0.0, 1.0])?)
                } else {
                    VarcompMethod::Minque(1)
                }
            } else if p.eat_keyword("ML") {
                VarcompMethod::Ml
            } else if p.eat_keyword("REML") {
                VarcompMethod::Reml
            } else if p.eat_keyword("SSTYPE") {
                if p.at(Punct::LParen) {
                    VarcompMethod::Sstype(choice(p, "SSTYPE", &[1.0, 3.0])?)
                } else {
                    VarcompMethod::Sstype(3)
                }
            } else {
                return Err(p.unexpected(&["MINQUE", "ML", "REML", "SSTYPE"]));
            };
        } else if p.eat_keyword("INTERCEPT") {
            p.eat(Punct::Eq);
            if p.eat_keyword("INCLUDE") {
                c.intercept = true;
            } else if p.eat_keyword("EXCLUDE") {
                c.intercept = false;
            } else {
                return Err(p.unexpected(&["INCLUDE", "EXCLUDE"]));
            }
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            if p.eat_keyword("INCLUDE") {
                c.include = true;
            } else if p.eat_keyword("EXCLUDE") {
                c.include = false;
            } else {
                return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
            }
        } else if p.eat_keyword("REGWGT") {
            p.eat(Punct::Eq);
            // the last variable named wins
            c.regwgt = Some(p.name("a variable name")?);
            while matches!(p.peek(), Some(TokenKind::Word(_))) {
                c.regwgt = Some(p.name("a variable name")?);
            }
        } else if p.eat_keyword("CRITERIA") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("CONVERGE") {
                    c.converge = positive(p, "CONVERGE", false)?;
                } else if p.eat_keyword("EPS") {
                    c.eps = positive(p, "EPS", false)?;
                } else if p.eat_keyword("ITERATE") {
                    c.iterate = positive(p, "ITERATE", true)? as usize;
                } else {
                    return Err(p.unexpected(&["CONVERGE", "EPS", "ITERATE"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("EMS") {
                    c.print.ems = true;
                } else if p.eat_word("SS") {
                    c.print.ss = true;
                } else if p.eat_keyword("HISTORY") {
                    c.print.history = Some(if p.at(Punct::LParen) {
                        positive(p, "HISTORY", true)? as usize
                    } else {
                        1
                    });
                } else {
                    return Err(p.unexpected(&["EMS", "HISTORY", "SS"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            c.outfile = Some(outfile(p)?);
        } else if p.eat_keyword("DESIGN") {
            p.eat(Punct::Eq);
            let t = terms(p)?;
            c.design = (!t.is_empty()).then_some(t);
        } else {
            return Err(p.subcommand_error("VARCOMP", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if c.random.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "VARCOMP needs RANDOM with at least one factor",
        ));
    }
    Ok(Command::Varcomp(Box::new(c)))
}
