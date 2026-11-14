//! Parser of COXREG (Cox proportional hazards regression), TIME PROGRAM and CLEAR TIME PROGRAM.
//!
//! Rules from the Command Syntax Reference (COXREG, TIME PROGRAM):
//! - `[VARIABLES=] time [WITH terms]` comes first; a term is a variable or `a BY b`. One time
//!   variable only. The other subcommands follow in any order.
//! - STATUS (required): `var [EVENT] (values)` with numbers, quoted strings and `low THRU high`
//!   ranges, then optionally `LOST (values)`; every other value is censored. The last STATUS
//!   counts.
//! - STRATA takes one variable. CATEGORICAL and CONTRAST as in LOGISTIC REGRESSION, but the
//!   default contrast is DEVIATION (the engine applies it).
//! - METHOD: ENTER (default), FSTEP or BSTEP with COND (default), LR or WALD, then terms or ALL;
//!   the word METHOD may be left out. Several run in order.
//! - PRINT: DEFAULT, SUMMARY, BASELINE, CORR, ALL and CI(level), CI alone meaning 95.
//! - CRITERIA: BCON (alias PCON, 1E-4), LCON (1E-5), ITERATE (20), PIN (0.05), POUT (0.1).
//! - PLOT: NONE, SURVIVAL, HAZARD, LML, OMS. PATTERN: `var(value) ... [BY var]`, repeatable.
//! - SAVE: SURVIVAL, SE, HAZARD (alias RESID), LML, DFBETA, PRESID, XBETA, each once, with an
//!   optional new name in parentheses.

use crate::ast::{Command, MissingValue, Value};
use crate::error::SyntaxError;
use crate::glm::{Contrast, contrast};
use crate::lexer::{Punct, TokenKind};
use crate::logistic::{LogisticMethod, LogisticMethodKind, checked, method, terms, value};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CoxregPrint {
    /// SUMMARY: the tables of the last step of each block only.
    pub summary: bool,
    pub corr: bool,
    pub baseline: bool,
    /// `CI(level)` in percent.
    pub ci: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoxregCriteria {
    pub bcon: f64,
    pub lcon: f64,
    pub iterate: u32,
    pub pin: f64,
    pub pout: f64,
}

impl Default for CoxregCriteria {
    fn default() -> Self {
        Self {
            bcon: 1e-4,
            lcon: 1e-5,
            iterate: 20,
            pin: 0.05,
            pout: 0.1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CoxregPlot {
    pub survival: bool,
    pub hazard: bool,
    pub lml: bool,
    pub oms: bool,
}

/// One PATTERN: covariate values, and the categorical variable whose categories get a line
/// each.
#[derive(Debug, Clone, PartialEq)]
pub struct CoxregPattern {
    pub values: Vec<(String, f64)>,
    pub by: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoxregTemp {
    Survival,
    Se,
    Hazard,
    Lml,
    Dfbeta,
    Presid,
    Xbeta,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Coxreg {
    pub time: String,
    /// The terms after WITH.
    pub terms: Vec<Vec<String>>,
    pub status: String,
    pub events: Vec<MissingValue>,
    pub strata: Option<String>,
    pub categorical: Vec<String>,
    pub contrasts: Vec<(String, Contrast)>,
    pub methods: Vec<LogisticMethod>,
    /// `/MISSING=INCLUDE`
    pub include: bool,
    pub print: CoxregPrint,
    pub criteria: CoxregCriteria,
    pub plot: CoxregPlot,
    pub patterns: Vec<CoxregPattern>,
    pub save: Vec<(CoxregTemp, Option<String>)>,
}

/// Values and ranges in parentheses, separated by blanks or commas.
fn value_list(p: &mut Parser) -> PResult<Vec<MissingValue>> {
    p.expect(Punct::LParen)?;
    let mut out = Vec::new();
    while let Some(v) = p.value_or_range()? {
        out.push(v);
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a value"]));
    }
    p.expect(Punct::RParen)?;
    Ok(out)
}

fn print(p: &mut Parser) -> PResult<CoxregPrint> {
    p.eat(Punct::Eq);
    let mut out = CoxregPrint::default();
    let mut any = false;
    while !at_end(p) {
        any = true;
        if p.eat_keyword("DEFAULT") {
        } else if p.eat_keyword("SUMMARY") {
            out.summary = true;
        } else if p.eat_keyword("CORR") {
            out.corr = true;
        } else if p.eat_keyword("BASELINE") {
            out.baseline = true;
        } else if p.eat_word("CI") {
            let start = p.pos;
            let level = if p.at(Punct::LParen) {
                value(p, "a confidence level")?
            } else {
                95.0
            };
            out.ci = Some(checked(
                p,
                start,
                level,
                level > 0.0 && level < 100.0,
                "CI takes a level above 0 and below 100",
            )?);
        } else if p.eat_word("ALL") {
            out.corr = true;
            out.baseline = true;
            out.ci = Some(out.ci.unwrap_or(95.0));
        } else {
            return Err(p.unexpected(&["DEFAULT", "SUMMARY", "BASELINE", "CORR", "CI", "ALL"]));
        }
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&["DEFAULT", "SUMMARY", "BASELINE", "CORR", "CI", "ALL"]));
    }
    Ok(out)
}

fn criteria(p: &mut Parser, c: &mut CoxregCriteria) -> PResult<()> {
    p.eat(Punct::Eq);
    const KEYWORDS: [&str; 6] = ["BCON", "PCON", "LCON", "ITERATE", "PIN", "POUT"];
    while !at_end(p) {
        let Some(i) = KEYWORDS.iter().position(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        let start = p.pos;
        let v = value(p, "a number")?;
        match i {
            0 | 1 => c.bcon = checked(p, start, v, v >= 0.0, "BCON cannot be negative")?,
            2 => c.lcon = checked(p, start, v, v >= 0.0, "LCON cannot be negative")?,
            3 => {
                c.iterate = checked(
                    p,
                    start,
                    v,
                    v >= 1.0 && v.fract() == 0.0,
                    "ITERATE takes a positive whole number",
                )? as u32
            }
            _ => {
                let v = checked(
                    p,
                    start,
                    v,
                    v > 0.0 && v <= 1.0,
                    "a probability must lie above 0 and at most 1",
                )?;
                if i == 4 {
                    c.pin = v;
                } else {
                    c.pout = v;
                }
            }
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn plot(p: &mut Parser) -> PResult<CoxregPlot> {
    p.eat(Punct::Eq);
    let mut out = CoxregPlot::default();
    while !at_end(p) {
        if p.eat_keyword("NONE") {
            out = CoxregPlot::default();
        } else if p.eat_keyword("SURVIVAL") {
            out.survival = true;
        } else if p.eat_keyword("HAZARD") {
            out.hazard = true;
        } else if p.eat_keyword("LML") {
            out.lml = true;
        } else if p.eat_keyword("OMS") {
            out.oms = true;
        } else {
            return Err(p.unexpected(&["NONE", "SURVIVAL", "HAZARD", "LML", "OMS"]));
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn pattern(p: &mut Parser) -> PResult<CoxregPattern> {
    p.eat(Punct::Eq);
    let mut out = CoxregPattern {
        values: Vec::new(),
        by: None,
    };
    while matches!(p.peek(), Some(TokenKind::Word(_))) {
        if p.eat_word("BY") {
            out.by = Some(p.name("a categorical variable")?);
            break;
        }
        let name = p.name("a variable name")?;
        let v = value(p, "a value")?;
        out.values.push((name, v));
        p.eat(Punct::Comma);
    }
    if out.values.is_empty() && out.by.is_none() {
        return Err(p.unexpected(&["a variable name", "BY"]));
    }
    Ok(out)
}

const TEMPS: [(&str, CoxregTemp); 8] = [
    ("SURVIVAL", CoxregTemp::Survival),
    ("HAZARD", CoxregTemp::Hazard),
    ("RESID", CoxregTemp::Hazard),
    ("LML", CoxregTemp::Lml),
    ("DFBETA", CoxregTemp::Dfbeta),
    ("PRESID", CoxregTemp::Presid),
    ("XBETA", CoxregTemp::Xbeta),
    ("SE", CoxregTemp::Se),
];

fn save(p: &mut Parser) -> PResult<Vec<(CoxregTemp, Option<String>)>> {
    p.eat(Punct::Eq);
    let mut out: Vec<(CoxregTemp, Option<String>)> = Vec::new();
    let names: Vec<&str> = TEMPS.iter().map(|(n, _)| *n).collect();
    while !at_end(p) {
        let start = p.pos;
        // SE is two letters, written in full
        let found = TEMPS.iter().find(|(n, _)| match *n {
            "SE" => matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("SE")),
            n => p.at_keyword(n),
        });
        let Some(&(_, t)) = found else {
            return Err(p.unexpected(&names));
        };
        p.pos += 1;
        let name = if p.eat(Punct::LParen) {
            let name = p.name("a variable name")?;
            p.expect(Punct::RParen)?;
            Some(name)
        } else {
            None
        };
        if out.iter().any(|(v, _)| *v == t) {
            return Err(SyntaxError::new(
                p.span_from(start),
                "a temporary variable is saved only once",
            ));
        }
        out.push((t, name));
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&names));
    }
    Ok(out)
}

pub(crate) fn coxreg(p: &mut Parser) -> PResult<Command> {
    p.eat(Punct::Slash);
    if p.eat_keyword("VARIABLES") {
        p.eat(Punct::Eq);
    }
    let time = p.name("the survival time variable")?;
    if p.at(Punct::Comma)
        || matches!(p.peek(), Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case("WITH"))
    {
        return Err(SyntaxError::new(
            p.span(),
            "COXREG takes one survival time variable; the covariates follow WITH",
        ));
    }
    let terms_after_with = if p.eat_word("WITH") {
        let list = terms(p)?;
        if list.is_empty() {
            return Err(p.unexpected(&["a variable name"]));
        }
        list
    } else {
        Vec::new()
    };
    let mut command = Coxreg {
        time,
        terms: terms_after_with,
        status: String::new(),
        events: Vec::new(),
        strata: None,
        categorical: Vec::new(),
        contrasts: Vec::new(),
        methods: Vec::new(),
        include: false,
        print: CoxregPrint::default(),
        criteria: CoxregCriteria::default(),
        plot: CoxregPlot::default(),
        patterns: Vec::new(),
        save: Vec::new(),
    };
    while p.eat(Punct::Slash) {
        if p.at_keyword("ENTER") || p.at_keyword("FSTEP") || p.at_keyword("BSTEP") {
            command.methods.push(method(p)?);
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            command.methods.push(method(p)?);
        } else if p.eat_keyword("STATUS") {
            p.eat(Punct::Eq);
            command.status = p.name("the status variable")?;
            p.eat_keyword("EVENT");
            command.events = value_list(p)?;
            if p.eat_keyword("LOST") {
                // every value that is not an event is censored, so LOST changes nothing
                value_list(p)?;
            }
        } else if p.eat_keyword("STRATA") {
            p.eat(Punct::Eq);
            command.strata = Some(p.name("the strata variable")?);
        } else if p.eat_keyword("CATEGORICAL") {
            p.eat(Punct::Eq);
            while matches!(p.peek(), Some(TokenKind::Word(_))) {
                let name = p.name("a variable name")?;
                if !command
                    .categorical
                    .iter()
                    .any(|c| c.eq_ignore_ascii_case(&name))
                {
                    command.categorical.push(name);
                }
                p.eat(Punct::Comma);
            }
            if command.categorical.is_empty() {
                return Err(p.unexpected(&["a variable name"]));
            }
        } else if p.eat_keyword("CONTRAST") {
            p.expect(Punct::LParen)?;
            let variable = p.name("a variable name")?;
            p.expect(Punct::RParen)?;
            p.eat(Punct::Eq);
            let kind = contrast(p, true)?;
            command
                .contrasts
                .retain(|(v, _)| !v.eq_ignore_ascii_case(&variable));
            command.contrasts.push((variable, kind));
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
            command.print = print(p)?;
        } else if p.eat_keyword("CRITERIA") {
            criteria(p, &mut command.criteria)?;
        } else if p.eat_keyword("PLOT") {
            command.plot = plot(p)?;
        } else if p.eat_keyword("PATTERN") {
            command.patterns.push(pattern(p)?);
        } else if p.eat_keyword("SAVE") {
            command.save = save(p)?;
        } else if p.eat_keyword("EXTERNAL") {
            // only a memory hint: every case is held in memory anyway
        } else {
            return Err(p.subcommand_error("COXREG", &["OUTFILE"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.status.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "COXREG needs STATUS: the status variable and its event values",
        ));
    }
    let strings = command
        .events
        .iter()
        .filter(|v| matches!(v, MissingValue::Value(Value::String(_))))
        .count();
    if strings > 0 && strings < command.events.len() {
        return Err(SyntaxError::new(
            p.span(),
            "STATUS takes either quoted strings or numbers and ranges, not both",
        ));
    }
    if command.methods.is_empty() && !command.terms.is_empty() {
        command.methods.push(LogisticMethod {
            kind: LogisticMethodKind::Enter,
            terms: Vec::new(),
        });
    }
    if command.terms.is_empty() && command.methods.iter().any(|m| m.terms.is_empty()) {
        return Err(SyntaxError::new(
            p.span(),
            "the covariates follow WITH, or every METHOD names its own",
        ));
    }
    Ok(Command::Coxreg(Box::new(command)))
}

pub(crate) fn time_program(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::TimeProgram)
}

pub(crate) fn clear_time_program(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::ClearTimeProgram)
}
