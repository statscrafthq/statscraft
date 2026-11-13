//! Parser of PLUM: ordinal regression with a cumulative link model.
//!
//! Rules from the Command Syntax Reference (PLUM), in our words:
//! - `PLUM dependent [BY factors] [WITH covariates]`; the dependent comes first. BY, WITH and
//!   WITHIN are reserved here. Subcommands follow in any order; a repeated one replaces the
//!   earlier one, except TEST, which accumulates. An empty subcommand changes nothing, except LOCATION,
//!   where empty means the default model, and SCALE, where empty means no scale part.
//! - CRITERIA: CIN(95), DELTA(0), BIAS(0) (both below 1), MXITER(100), MXSTEP(5), LCONVERGE(0),
//!   PCONVERGE(1E-6), SINGULAR(1E-8).
//! - LINK: CAUCHIT, CLOGLOG, LOGIT (default), NLOGLOG or PROBIT, one of them.
//! - LOCATION and SCALE: effects as in a GLM design (`a*b`, `a BY b`, `a(b)`, `a WITHIN b`).
//!   INTERCEPT may stand on LOCATION, never on SCALE.
//! - MISSING: EXCLUDE (default) or INCLUDE.
//! - PRINT: CELLINFO CORB COVB FIT HISTORY[(n)] KERNEL TPARALLEL PARAMETER SUMMARY.
//! - SAVE: ESTPROB[(root[:n])] (n defaults to 25; `(:n)` gives only the count), PREDCAT,
//!   PCPROB and ACPROB, each with an optional new name, saved in the order written.
//! - TEST: `TEST[(constants)] = ['label'] effect values ...; ...` or ALL with values for every
//!   parameter, the thresholds first. INTERCEPT names the thresholds, one value each.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::{CustomTest, Term, custom_test, list_until, terms};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlumLink {
    Cauchit,
    Cloglog,
    Logit,
    Nloglog,
    Probit,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlumCriteria {
    /// Confidence level in percent.
    pub cin: f64,
    /// Added to every empty cell of a subpopulation with cases.
    pub delta: f64,
    /// Added to every cell.
    pub bias: f64,
    pub mxiter: u32,
    pub mxstep: u32,
    pub lconverge: f64,
    pub pconverge: f64,
    pub singular: f64,
}

impl Default for PlumCriteria {
    fn default() -> Self {
        Self {
            cin: 95.0,
            delta: 0.0,
            bias: 0.0,
            mxiter: 100,
            mxstep: 5,
            lconverge: 0.0,
            pconverge: 1e-6,
            singular: 1e-8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PlumPrint {
    pub cellinfo: bool,
    pub corb: bool,
    pub covb: bool,
    pub fit: bool,
    /// `HISTORY(n)`: every n-th iteration.
    pub history: Option<u32>,
    pub kernel: bool,
    pub tparallel: bool,
    pub parameter: bool,
    pub summary: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlumSave {
    /// The probabilities of the first `count` categories, named root1, root2, ...
    Estprob {
        root: Option<String>,
        count: usize,
    },
    Predcat(Option<String>),
    Pcprob(Option<String>),
    Acprob(Option<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plum {
    pub dependent: String,
    pub factors: Vec<VarItem>,
    pub covariates: Vec<VarItem>,
    pub criteria: PlumCriteria,
    pub link: PlumLink,
    /// None: the default location model.
    pub location: Option<Vec<Term>>,
    /// None or empty: no scale part.
    pub scale: Option<Vec<Term>>,
    /// `/MISSING=INCLUDE`
    pub include: bool,
    /// None: no PRINT subcommand.
    pub print: Option<PlumPrint>,
    pub save: Vec<PlumSave>,
    pub tests: Vec<CustomTest>,
}

/// A value in parentheses after a CRITERIA keyword.
fn value(p: &mut Parser) -> PResult<f64> {
    p.expect(Punct::LParen)?;
    let Some(v) = p.number() else {
        return Err(p.unexpected(&["a number"]));
    };
    p.expect(Punct::RParen)?;
    Ok(v)
}

fn criteria(p: &mut Parser, c: &mut PlumCriteria) -> PResult<()> {
    p.eat(Punct::Eq);
    while !at_end(p) {
        let start = p.pos;
        let check = |p: &Parser, ok: bool, rule: &str| {
            if ok {
                Ok(())
            } else {
                Err(SyntaxError::new(p.span_from(start), rule.to_string()))
            }
        };
        if p.eat_keyword("CIN") {
            let v = value(p)?;
            check(
                p,
                (0.0..100.0).contains(&v),
                "CIN is at least 0 and below 100",
            )?;
            c.cin = v;
        } else if p.eat_keyword("DELTA") {
            let v = value(p)?;
            check(
                p,
                (0.0..1.0).contains(&v),
                "DELTA is at least 0 and below 1",
            )?;
            c.delta = v;
        } else if p.eat_keyword("BIAS") {
            let v = value(p)?;
            check(p, (0.0..1.0).contains(&v), "BIAS is at least 0 and below 1")?;
            c.bias = v;
        } else if p.eat_keyword("MXITER") {
            let v = value(p)?;
            check(
                p,
                v >= 0.0 && v.fract() == 0.0,
                "MXITER is a whole number, 0 or more",
            )?;
            c.mxiter = v as u32;
        } else if p.eat_keyword("MXSTEP") {
            let v = value(p)?;
            check(
                p,
                v >= 1.0 && v.fract() == 0.0,
                "MXSTEP is a whole number, 1 or more",
            )?;
            c.mxstep = v as u32;
        } else if p.eat_keyword("LCONVERGE") {
            let v = value(p)?;
            check(p, v >= 0.0, "LCONVERGE is 0 or more")?;
            c.lconverge = v;
        } else if p.eat_keyword("PCONVERGE") {
            let v = value(p)?;
            check(p, v >= 0.0, "PCONVERGE is 0 or more")?;
            c.pconverge = v;
        } else if p.eat_keyword("SINGULAR") {
            let v = value(p)?;
            check(p, v > 0.0, "SINGULAR is above 0")?;
            c.singular = v;
        } else {
            return Err(p.unexpected(&[
                "CIN",
                "DELTA",
                "BIAS",
                "MXITER",
                "MXSTEP",
                "LCONVERGE",
                "PCONVERGE",
                "SINGULAR",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn print(p: &mut Parser) -> PResult<Option<PlumPrint>> {
    p.eat(Punct::Eq);
    let mut out = PlumPrint::default();
    let mut any = false;
    while !at_end(p) {
        any = true;
        if p.eat_keyword("CELLINFO") {
            out.cellinfo = true;
        } else if p.eat_keyword("CORB") {
            out.corb = true;
        } else if p.eat_keyword("COVB") {
            out.covb = true;
        } else if p.eat_keyword("FIT") {
            out.fit = true;
        } else if p.eat_keyword("HISTORY") {
            let mut every = 1;
            if p.at(Punct::LParen) {
                let start = p.pos;
                let v = value(p)?;
                if v < 1.0 || v.fract() != 0.0 {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "HISTORY takes a whole number, 1 or more",
                    ));
                }
                every = v as u32;
            }
            out.history = Some(every);
        } else if p.eat_keyword("KERNEL") {
            out.kernel = true;
        } else if p.eat_keyword("TPARALLEL") {
            out.tparallel = true;
        } else if p.eat_keyword("PARAMETER") {
            out.parameter = true;
        } else if p.eat_keyword("SUMMARY") {
            out.summary = true;
        } else {
            return Err(p.unexpected(&[
                "CELLINFO",
                "CORB",
                "COVB",
                "FIT",
                "HISTORY",
                "KERNEL",
                "TPARALLEL",
                "PARAMETER",
                "SUMMARY",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(any.then_some(out))
}

fn optional_name(p: &mut Parser) -> PResult<Option<String>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    let name = p.name("a new variable name")?;
    p.expect(Punct::RParen)?;
    Ok(Some(name))
}

fn save(p: &mut Parser) -> PResult<Vec<PlumSave>> {
    p.eat(Punct::Eq);
    let mut out: Vec<PlumSave> = Vec::new();
    while !at_end(p) {
        let start = p.pos;
        let item = if p.eat_keyword("ESTPROB") {
            let mut root = None;
            let mut count = 25;
            if p.eat(Punct::LParen) {
                if matches!(p.peek(), Some(TokenKind::Word(_))) {
                    root = Some(p.name("a root name")?);
                }
                if p.eat(Punct::Colon) {
                    let at = p.pos;
                    match p.number() {
                        Some(n) if n >= 1.0 && n.fract() == 0.0 => count = n as usize,
                        _ => {
                            return Err(SyntaxError::new(
                                p.span_from(at),
                                "the number of categories after ':' is a whole number, 1 or more",
                            ));
                        }
                    }
                }
                p.expect(Punct::RParen)?;
            }
            PlumSave::Estprob { root, count }
        } else if p.eat_keyword("PREDCAT") {
            PlumSave::Predcat(optional_name(p)?)
        } else if p.eat_keyword("PCPROB") {
            PlumSave::Pcprob(optional_name(p)?)
        } else if p.eat_keyword("ACPROB") {
            PlumSave::Acprob(optional_name(p)?)
        } else {
            return Err(p.unexpected(&["ESTPROB", "PREDCAT", "PCPROB", "ACPROB"]));
        };
        if out
            .iter()
            .any(|s| std::mem::discriminant(s) == std::mem::discriminant(&item))
        {
            return Err(SyntaxError::new(
                p.span_from(start),
                "each SAVE keyword is given only once",
            ));
        }
        out.push(item);
        p.eat(Punct::Comma);
    }
    Ok(out)
}

pub(crate) fn plum(p: &mut Parser) -> PResult<Command> {
    let dependent = p.name("the dependent variable")?;
    if p.at(Punct::Comma)
        || matches!(p.peek(), Some(TokenKind::Word(w))
            if !w.eq_ignore_ascii_case("BY") && !w.eq_ignore_ascii_case("WITH"))
    {
        return Err(SyntaxError::new(
            p.span(),
            "PLUM takes one dependent variable; factors follow BY and covariates WITH",
        ));
    }
    let factors = if p.eat_word("BY") {
        list_until(p, &["WITH"])?
    } else {
        Vec::new()
    };
    let covariates = if p.eat_word("WITH") {
        p.var_list()?
    } else {
        Vec::new()
    };
    let mut command = Plum {
        dependent,
        factors,
        covariates,
        criteria: PlumCriteria::default(),
        link: PlumLink::Logit,
        location: None,
        scale: None,
        include: false,
        print: None,
        save: Vec::new(),
        tests: Vec::new(),
    };
    while p.eat(Punct::Slash) {
        if p.eat_keyword("CRITERIA") {
            criteria(p, &mut command.criteria)?;
        } else if p.eat_keyword("LINK") {
            p.eat(Punct::Eq);
            const LINKS: [(&str, PlumLink); 5] = [
                ("CAUCHIT", PlumLink::Cauchit),
                ("CLOGLOG", PlumLink::Cloglog),
                ("LOGIT", PlumLink::Logit),
                ("NLOGLOG", PlumLink::Nloglog),
                ("PROBIT", PlumLink::Probit),
            ];
            if !at_end(p) {
                match LINKS.iter().find(|(n, _)| p.at_keyword(n)) {
                    Some((_, link)) => {
                        p.pos += 1;
                        command.link = *link;
                    }
                    None => return Err(p.unexpected(&LINKS.map(|(n, _)| n))),
                }
            }
        } else if p.eat_keyword("LOCATION") {
            p.eat(Punct::Eq);
            let all = terms(p)?;
            // the thresholds are the intercept, so INTERCEPT alone leaves only them
            command.location = (!all.is_empty()).then(|| {
                all.into_iter()
                    .filter(|t| !t.variables.is_empty())
                    .collect::<Vec<Term>>()
            });
        } else if p.eat_keyword("SCALE") {
            p.eat(Punct::Eq);
            let start = p.pos;
            let list = terms(p)?;
            if list.iter().any(|t| t.variables.is_empty()) {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "SCALE takes no INTERCEPT",
                ));
            }
            command.scale = (!list.is_empty()).then_some(list);
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            while !at_end(p) {
                if p.eat_keyword("INCLUDE") {
                    command.include = true;
                } else if p.eat_keyword("EXCLUDE") {
                    command.include = false;
                } else {
                    return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
                }
            }
        } else if p.eat_keyword("PRINT") {
            if let Some(print) = print(p)? {
                command.print = Some(print);
            }
        } else if p.eat_keyword("SAVE") {
            let list = save(p)?;
            if !list.is_empty() {
                command.save = list;
            }
        } else if p.eat_keyword("TEST") {
            command.tests.push(custom_test(p, false)?);
        } else {
            return Err(p.subcommand_error("PLUM", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Plum(Box::new(command)))
}
