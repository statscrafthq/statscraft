//! Parser of LOGISTIC REGRESSION: binary logistic regression.
//!
//! Rules from the Command Syntax Reference (LOGISTIC REGRESSION):
//! - `[VARIABLES=] dependent [WITH terms]`; a term is a variable or an interaction `a BY b`.
//!   The subcommands follow in any order; the last ORIGIN, PRINT, CRITERIA, MISSING, CASEWISE
//!   and SAVE apply; CATEGORICAL and CONTRAST accumulate; METHOD subcommands run in order.
//! - METHOD: ENTER (default), FSTEP or BSTEP with COND (default), LR or WALD in parentheses,
//!   then the terms or ALL (none: every term after WITH). The word METHOD may be left out.
//! - CONTRAST(var): INDICATOR (default), DEVIATION, SIMPLE (reference category in
//!   parentheses), DIFFERENCE, HELMERT, REPEATED, POLYNOMIAL(metric), SPECIAL(matrix).
//! - PRINT: DEFAULT, SUMMARY, CORR, ITER[(n)], GOODFIT, CI(level), ALL.
//! - CRITERIA: BCON(0.001), ITERATE(20), LCON(0), PIN(0.05), POUT(0.10), EPS(1E-8), CUT(0.5).
//! - CASEWISE: temporary variables (default PRED PGROUP RESID ZRESID SRESID) and OUTLIER(2).
//! - NOCONST is another name of ORIGIN.

use crate::ast::Command;
use crate::error::SyntaxError;
use crate::glm::{Contrast, contrast};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// The statistic that removes a variable in FSTEP and BSTEP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    Cond,
    Lr,
    Wald,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogisticMethodKind {
    Enter,
    Fstep(Removal),
    Bstep(Removal),
}

#[derive(Debug, Clone, PartialEq)]
pub struct LogisticMethod {
    pub kind: LogisticMethodKind,
    /// The terms, each a list of variables joined by BY; empty is ALL.
    pub terms: Vec<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogisticPrint {
    /// SUMMARY: the tables of the last step of each block only.
    pub summary: bool,
    pub corr: bool,
    /// `ITER(n)`: the iteration history, every n-th iteration.
    pub iter: Option<u32>,
    pub goodfit: bool,
    /// `CI(level)` in percent.
    pub ci: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogisticCriteria {
    pub bcon: f64,
    pub iterate: u32,
    pub lcon: f64,
    pub pin: f64,
    pub pout: f64,
    pub eps: f64,
    pub cut: f64,
}

impl Default for LogisticCriteria {
    fn default() -> Self {
        Self {
            bcon: 0.001,
            iterate: 20,
            lcon: 0.0,
            pin: 0.05,
            pout: 0.10,
            eps: 1e-8,
            cut: 0.5,
        }
    }
}

/// The temporary variables of CASEWISE and SAVE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogisticTemp {
    Pred,
    Pgroup,
    Resid,
    Dev,
    Lresid,
    Sresid,
    Zresid,
    Lever,
    Cook,
    Dfbeta,
}

pub(crate) const LOGISTIC_TEMPS: [(&str, LogisticTemp); 10] = [
    ("PRED", LogisticTemp::Pred),
    ("PGROUP", LogisticTemp::Pgroup),
    ("RESID", LogisticTemp::Resid),
    ("DEV", LogisticTemp::Dev),
    ("LRESID", LogisticTemp::Lresid),
    ("SRESID", LogisticTemp::Sresid),
    ("ZRESID", LogisticTemp::Zresid),
    ("LEVER", LogisticTemp::Lever),
    ("COOK", LogisticTemp::Cook),
    ("DFBETA", LogisticTemp::Dfbeta),
];

#[derive(Debug, Clone, PartialEq)]
pub struct LogisticCasewise {
    pub temps: Vec<LogisticTemp>,
    /// `OUTLIER(value)`: only cases whose studentized residual is at least this far from 0.
    pub outlier: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Logistic {
    pub dependent: String,
    /// The terms after WITH.
    pub terms: Vec<Vec<String>>,
    pub categorical: Vec<String>,
    pub contrasts: Vec<(String, Contrast)>,
    pub methods: Vec<LogisticMethod>,
    /// `/ORIGIN`: no constant.
    pub origin: bool,
    pub id: Option<String>,
    pub print: LogisticPrint,
    pub criteria: LogisticCriteria,
    /// `/MISSING=INCLUDE`
    pub include: bool,
    pub casewise: Option<LogisticCasewise>,
    pub save: Vec<(LogisticTemp, Option<String>)>,
    /// CLASSPLOT, which is not drawn yet.
    pub classplot: bool,
}

/// Terms separated by commas or blanks, each `a` or `a BY b ...`, up to a slash or the end.
fn terms(p: &mut Parser) -> PResult<Vec<Vec<String>>> {
    let mut out = Vec::new();
    while matches!(p.peek(), Some(TokenKind::Word(_))) {
        let mut term = vec![p.name("a variable name")?];
        while p.eat_word("BY") {
            term.push(p.name("a variable name")?);
        }
        out.push(term);
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn value(p: &mut Parser, what: &str) -> PResult<f64> {
    p.expect(Punct::LParen)?;
    let Some(n) = p.number() else {
        return Err(p.unexpected(&[what]));
    };
    p.expect(Punct::RParen)?;
    Ok(n)
}

fn checked(p: &Parser, start: usize, value: f64, ok: bool, what: &str) -> PResult<f64> {
    if ok {
        Ok(value)
    } else {
        Err(SyntaxError::new(
            p.span_from(start),
            format!("{what}, got {value}"),
        ))
    }
}

fn method(p: &mut Parser) -> PResult<LogisticMethod> {
    let kind = if p.eat_keyword("ENTER") {
        LogisticMethodKind::Enter
    } else {
        let forward = if p.eat_keyword("FSTEP") {
            true
        } else if p.eat_keyword("BSTEP") {
            false
        } else {
            return Err(p.unexpected(&["ENTER", "FSTEP", "BSTEP"]));
        };
        let removal = if p.eat(Punct::LParen) {
            let r = if p.eat_keyword("COND") {
                Removal::Cond
            } else if p.eat_word("LR") {
                Removal::Lr
            } else if p.eat_keyword("WALD") {
                Removal::Wald
            } else {
                return Err(p.unexpected(&["COND", "LR", "WALD"]));
            };
            p.expect(Punct::RParen)?;
            r
        } else {
            Removal::Cond
        };
        if forward {
            LogisticMethodKind::Fstep(removal)
        } else {
            LogisticMethodKind::Bstep(removal)
        }
    };
    let terms = if p.eat_word("ALL") {
        Vec::new()
    } else {
        terms(p)?
    };
    Ok(LogisticMethod { kind, terms })
}

fn print(p: &mut Parser) -> PResult<LogisticPrint> {
    p.eat(Punct::Eq);
    let mut out = LogisticPrint {
        summary: false,
        corr: false,
        iter: None,
        goodfit: false,
        ci: None,
    };
    let mut any = false;
    while !at_end(p) {
        any = true;
        if p.eat_keyword("DEFAULT") {
        } else if p.eat_keyword("SUMMARY") {
            out.summary = true;
        } else if p.eat_keyword("CORR") {
            out.corr = true;
        } else if p.eat_keyword("ITER") {
            let start = p.pos;
            let n = if p.at(Punct::LParen) {
                value(p, "a number")?
            } else {
                1.0
            };
            let n = checked(
                p,
                start,
                n,
                n >= 1.0 && n.fract() == 0.0,
                "ITER takes a positive whole number",
            )?;
            out.iter = Some(n as u32);
        } else if p.eat_keyword("GOODFIT") {
            out.goodfit = true;
        } else if p.eat_word("CI") {
            let start = p.pos;
            let level = value(p, "a confidence level")?;
            out.ci = Some(checked(
                p,
                start,
                level,
                (1.0..=99.0).contains(&level) && level.fract() == 0.0,
                "CI takes a whole number from 1 to 99",
            )?);
        } else if p.eat_word("ALL") {
            out.corr = true;
            out.goodfit = true;
            out.iter = Some(out.iter.unwrap_or(1));
            out.ci = Some(out.ci.unwrap_or(95.0));
        } else {
            return Err(
                p.unexpected(&["DEFAULT", "SUMMARY", "CORR", "ITER", "GOODFIT", "CI", "ALL"])
            );
        }
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&["DEFAULT", "SUMMARY", "CORR", "ALL"]));
    }
    Ok(out)
}

fn criteria(p: &mut Parser, c: &mut LogisticCriteria) -> PResult<()> {
    p.eat(Punct::Eq);
    const KEYWORDS: [&str; 7] = ["BCON", "ITERATE", "LCON", "PIN", "POUT", "EPS", "CUT"];
    while !at_end(p) {
        let Some(i) = KEYWORDS.iter().position(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        let start = p.pos;
        let v = value(p, "a number")?;
        match i {
            0 => c.bcon = checked(p, start, v, v >= 0.0, "BCON cannot be negative")?,
            1 => {
                c.iterate = checked(
                    p,
                    start,
                    v,
                    v >= 1.0 && v.fract() == 0.0,
                    "ITERATE takes a positive whole number",
                )? as u32
            }
            2 => c.lcon = checked(p, start, v, v >= 0.0, "LCON cannot be negative")?,
            3 | 4 => {
                let v = checked(
                    p,
                    start,
                    v,
                    v > 0.0 && v <= 1.0,
                    "a probability must lie above 0 and at most 1",
                )?;
                if i == 3 {
                    c.pin = v;
                } else {
                    c.pout = v;
                }
            }
            5 => {
                c.eps = checked(
                    p,
                    start,
                    v,
                    (1e-12..=0.05).contains(&v),
                    "EPS must lie from 1E-12 to 0.05",
                )?
            }
            _ => {
                c.cut = checked(
                    p,
                    start,
                    v,
                    v > 0.0 && v < 1.0,
                    "CUT must lie between 0 and 1",
                )?
            }
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn temp(p: &mut Parser) -> PResult<Option<LogisticTemp>> {
    let found = LOGISTIC_TEMPS.iter().find(|(n, _)| p.at_keyword(n));
    Ok(found.map(|&(_, t)| {
        p.pos += 1;
        t
    }))
}

fn casewise(p: &mut Parser) -> PResult<LogisticCasewise> {
    p.eat(Punct::Eq);
    let mut temps = Vec::new();
    let mut outlier = None;
    while !at_end(p) {
        if p.eat_keyword("OUTLIER") {
            let start = p.pos;
            let v = if p.at(Punct::LParen) {
                value(p, "a number")?
            } else {
                2.0
            };
            outlier = Some(checked(p, start, v, v > 0.0, "OUTLIER must be above 0")?);
        } else if let Some(t) = temp(p)? {
            if !temps.contains(&t) {
                temps.push(t);
            }
        } else {
            let mut names: Vec<&str> = LOGISTIC_TEMPS.iter().map(|(n, _)| *n).collect();
            names.push("OUTLIER");
            return Err(p.unexpected(&names));
        }
        p.eat(Punct::Comma);
    }
    if temps.is_empty() {
        use LogisticTemp as T;
        temps = vec![T::Pred, T::Pgroup, T::Resid, T::Zresid, T::Sresid];
    }
    Ok(LogisticCasewise { temps, outlier })
}

fn save(p: &mut Parser) -> PResult<Vec<(LogisticTemp, Option<String>)>> {
    p.eat(Punct::Eq);
    let mut out: Vec<(LogisticTemp, Option<String>)> = Vec::new();
    while !at_end(p) {
        let start = p.pos;
        let Some(t) = temp(p)? else {
            let names: Vec<&str> = LOGISTIC_TEMPS.iter().map(|(n, _)| *n).collect();
            return Err(p.unexpected(&names));
        };
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
        return Err(p.unexpected(&["PRED", "a temporary variable"]));
    }
    Ok(out)
}

pub(crate) fn logistic(p: &mut Parser) -> PResult<Command> {
    p.eat(Punct::Slash);
    if p.eat_keyword("VARIABLES") {
        p.eat(Punct::Eq);
    }
    let dependent = p.name("the dependent variable")?;
    if p.at(Punct::Comma)
        || matches!(p.peek(), Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case("WITH"))
    {
        return Err(SyntaxError::new(
            p.span(),
            "LOGISTIC REGRESSION takes one dependent variable; the independent variables follow WITH",
        ));
    }
    let terms = if p.eat_word("WITH") {
        let list = terms(p)?;
        if list.is_empty() {
            return Err(p.unexpected(&["a variable name"]));
        }
        list
    } else {
        Vec::new()
    };
    let mut command = Logistic {
        dependent,
        terms,
        categorical: Vec::new(),
        contrasts: Vec::new(),
        methods: Vec::new(),
        origin: false,
        id: None,
        print: LogisticPrint {
            summary: false,
            corr: false,
            iter: None,
            goodfit: false,
            ci: None,
        },
        criteria: LogisticCriteria::default(),
        include: false,
        casewise: None,
        save: Vec::new(),
        classplot: false,
    };
    while p.eat(Punct::Slash) {
        if p.at_keyword("ENTER") || p.at_keyword("FSTEP") || p.at_keyword("BSTEP") {
            command.methods.push(method(p)?);
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            command.methods.push(method(p)?);
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
        } else if p.eat_keyword("ORIGIN") || p.eat_keyword("NOCONST") {
            command.origin = true;
        } else if p.eat_keyword("NOORIGIN") {
            command.origin = false;
        } else if p.eat_word("ID") {
            p.eat(Punct::Eq);
            command.id = Some(p.name("a variable name")?);
        } else if p.eat_keyword("PRINT") {
            command.print = print(p)?;
        } else if p.eat_keyword("CRITERIA") {
            criteria(p, &mut command.criteria)?;
        } else if p.eat_keyword("CLASSPLOT") {
            command.classplot = true;
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
        } else if p.eat_keyword("CASEWISE") {
            command.casewise = Some(casewise(p)?);
        } else if p.eat_keyword("SAVE") {
            command.save = save(p)?;
        } else if p.eat_keyword("EXTERNAL") {
            // only a memory hint: every case is held in memory anyway
        } else {
            return Err(p.subcommand_error("LOGISTIC REGRESSION", &["SELECT", "OUTFILE"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if command.methods.is_empty() {
        command.methods.push(LogisticMethod {
            kind: LogisticMethodKind::Enter,
            terms: Vec::new(),
        });
    }
    if command.terms.is_empty() && command.methods.iter().any(|m| m.terms.is_empty()) {
        return Err(SyntaxError::new(
            p.span(),
            "the independent variables follow WITH, or every METHOD names its own",
        ));
    }
    Ok(Command::Logistic(Box::new(command)))
}
