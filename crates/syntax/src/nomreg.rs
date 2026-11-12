//! Parser of NOMREG: multinomial logistic regression of a nominal dependent variable.
//!
//! Rules from the Command Syntax Reference (NOMREG):
//! - `dependent [(BASE=FIRST|LAST|value ORDER=ASCENDING|DATA|DESCENDING)] [BY factors]
//!   [WITH covariates]` comes first; the subcommands follow in any order and, except TEST, the
//!   last one of each kind applies. Empty subcommands other than MODEL are ignored.
//! - MODEL and FULLFACTORIAL exclude each other. MODEL terms as in GLM: `a`, `a*b`, `a BY b`,
//!   `a(b)`, `a WITHIN b`, `x*x`; the word INTERCEPT among them is allowed and ignored (the
//!   INTERCEPT subcommand decides). A `|` starts the stepwise part, not supported yet.
//! - CRITERIA: CIN(95), DELTA(0), MXITER(100), MXSTEP(5), LCONVERGE(0), PCONVERGE(1E-6),
//!   SINGULAR(1E-8), BIAS(0), CHKSEP(20).
//! - PRINT keywords; NONE overrides the others. Without PRINT: CPS, MFI (and STEP).
//! - SAVE: ACPROB, ESTPROB[(root[:n])], PCPROB, PREDCAT, each with an optional new name.
//! - SCALE: a positive number, DEVIANCE or PEARSON. SUBPOP: a variable list.

use crate::ast::{Command, Value, VarItem};
use crate::error::SyntaxError;
use crate::glm::{Term, list_until, term};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, PartialEq)]
pub enum NomregBase {
    First,
    Last,
    Value(Value),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NomregOrder {
    Ascending,
    Data,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NomregCriteria {
    pub cin: f64,
    pub delta: f64,
    pub mxiter: u32,
    pub mxstep: u32,
    pub lconverge: f64,
    pub pconverge: f64,
    pub singular: f64,
    pub bias: f64,
    pub chksep: u32,
}

impl Default for NomregCriteria {
    fn default() -> Self {
        Self {
            cin: 95.0,
            delta: 0.0,
            mxiter: 100,
            mxstep: 5,
            lconverge: 0.0,
            pconverge: 1e-6,
            singular: 1e-8,
            bias: 0.0,
            chksep: 20,
        }
    }
}

/// The tables PRINT asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NomregPrint {
    pub cps: bool,
    pub mfi: bool,
    pub parameter: bool,
    pub lrt: bool,
    pub fit: bool,
    pub summary: bool,
    pub classtable: bool,
    pub ic: bool,
    pub kernel: bool,
    pub covb: bool,
    pub corb: bool,
}

impl Default for NomregPrint {
    fn default() -> Self {
        Self {
            cps: true,
            mfi: true,
            parameter: false,
            lrt: false,
            fit: false,
            summary: false,
            classtable: false,
            ic: false,
            kernel: false,
            covb: false,
            corb: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NomregScale {
    Value(f64),
    Deviance,
    Pearson,
}

/// A casewise variable of SAVE.
#[derive(Debug, Clone, PartialEq)]
pub enum NomregSave {
    /// The probability of every response category, the first `count` of them.
    Estprob {
        root: Option<String>,
        count: u32,
    },
    Predcat(Option<String>),
    Pcprob(Option<String>),
    Acprob(Option<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Nomreg {
    pub dependent: String,
    pub base: NomregBase,
    pub order: NomregOrder,
    pub factors: Vec<VarItem>,
    pub covariates: Vec<VarItem>,
    pub criteria: NomregCriteria,
    /// The MODEL terms; None for the default model. FULLFACTORIAL sets `fullfactorial`.
    pub model: Option<Vec<Term>>,
    pub fullfactorial: bool,
    pub intercept: bool,
    /// `/MISSING=INCLUDE`
    pub include: bool,
    pub print: NomregPrint,
    pub save: Vec<NomregSave>,
    pub scale: NomregScale,
    pub subpop: Option<Vec<VarItem>>,
}

fn value_in_parens(p: &mut Parser) -> PResult<f64> {
    p.expect(Punct::LParen)?;
    let Some(n) = p.number() else {
        return Err(p.unexpected(&["a number"]));
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

/// `(BASE=... ORDER=...)` after the dependent variable.
fn dependent_options(p: &mut Parser, command: &mut Nomreg) -> PResult<()> {
    while !p.eat(Punct::RParen) {
        if p.eat_keyword("BASE") {
            p.eat(Punct::Eq);
            command.base = if p.eat_keyword("FIRST") {
                NomregBase::First
            } else if p.eat_keyword("LAST") {
                NomregBase::Last
            } else if let Some(v) = p.value() {
                NomregBase::Value(v)
            } else {
                return Err(p.unexpected(&["FIRST", "LAST", "a value"]));
            };
        } else if p.eat_keyword("ORDER") {
            p.eat(Punct::Eq);
            command.order = if p.eat_keyword("ASCENDING") {
                NomregOrder::Ascending
            } else if p.eat_keyword("DATA") {
                NomregOrder::Data
            } else if p.eat_keyword("DESCENDING") {
                NomregOrder::Descending
            } else {
                return Err(p.unexpected(&["ASCENDING", "DATA", "DESCENDING"]));
            };
        } else {
            return Err(p.unexpected(&["BASE", "ORDER", "')'"]));
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn criteria(p: &mut Parser, c: &mut NomregCriteria) -> PResult<()> {
    p.eat(Punct::Eq);
    const KEYWORDS: [&str; 9] = [
        "CIN",
        "DELTA",
        "MXITER",
        "MXSTEP",
        "LCONVERGE",
        "PCONVERGE",
        "SINGULAR",
        "BIAS",
        "CHKSEP",
    ];
    while !at_end(p) {
        let Some(i) = KEYWORDS.iter().position(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(&KEYWORDS));
        };
        p.pos += 1;
        let start = p.pos;
        let v = value_in_parens(p)?;
        let whole = |p: &Parser, ok_zero: bool, what: &str| {
            checked(
                p,
                start,
                v,
                v.fract() == 0.0 && (v >= 1.0 || ok_zero && v == 0.0),
                what,
            )
            .map(|v| v as u32)
        };
        match i {
            0 => {
                c.cin = checked(
                    p,
                    start,
                    v,
                    (0.0..100.0).contains(&v),
                    "CIN is at least 0 and below 100",
                )?
            }
            1 | 7 => {
                let v = checked(
                    p,
                    start,
                    v,
                    (0.0..1.0).contains(&v),
                    "the value is at least 0 and below 1",
                )?;
                if i == 1 {
                    c.delta = v;
                } else {
                    c.bias = v;
                }
            }
            2 => c.mxiter = whole(p, false, "MXITER takes a positive whole number")?,
            3 => c.mxstep = whole(p, false, "MXSTEP takes a positive whole number")?,
            4 => c.lconverge = checked(p, start, v, v >= 0.0, "LCONVERGE cannot be negative")?,
            5 => c.pconverge = checked(p, start, v, v >= 0.0, "PCONVERGE cannot be negative")?,
            6 => c.singular = checked(p, start, v, v > 0.0, "SINGULAR must be above 0")?,
            _ => c.chksep = whole(p, true, "CHKSEP takes a whole number of 0 or more")?,
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn print(p: &mut Parser) -> PResult<Option<NomregPrint>> {
    p.eat(Punct::Eq);
    let mut out = NomregPrint {
        cps: false,
        mfi: false,
        ..NomregPrint::default()
    };
    let mut none = false;
    if at_end(p) {
        return Ok(None);
    }
    while !at_end(p) {
        let start = p.pos;
        if p.eat_keyword("NONE") {
            none = true;
        } else if p.eat_keyword("CPS") {
            out.cps = true;
        } else if p.eat_keyword("MFI") {
            out.mfi = true;
        } else if p.eat_keyword("PARAMETER") {
            out.parameter = true;
        } else if p.eat_word("LRT") {
            out.lrt = true;
        } else if p.eat_word("FIT") {
            out.fit = true;
        } else if p.eat_keyword("SUMMARY") {
            out.summary = true;
        } else if p.eat_keyword("CLASSTABLE") {
            out.classtable = true;
        } else if p.eat_word("IC") {
            out.ic = true;
        } else if p.eat_keyword("KERNEL") {
            out.kernel = true;
        } else if p.eat_keyword("COVB") {
            out.covb = true;
        } else if p.eat_keyword("CORB") {
            out.corb = true;
        } else if p.eat_keyword("STEP") {
            // the step summary exists only with a stepwise method, which is not supported yet
        } else if let Some(word) = ["CELLPROB", "HISTORY", "ASSOCIATION"]
            .into_iter()
            .find(|k| p.at_keyword(k))
        {
            p.pos += 1;
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("PRINT {word} of NOMREG is not supported yet"),
            ));
        } else {
            return Err(p.unexpected(&[
                "CPS",
                "MFI",
                "PARAMETER",
                "LRT",
                "FIT",
                "SUMMARY",
                "CLASSTABLE",
                "IC",
                "KERNEL",
                "COVB",
                "CORB",
                "STEP",
                "NONE",
            ]));
        }
        p.eat(Punct::Comma);
    }
    if none {
        out = NomregPrint {
            cps: false,
            mfi: false,
            ..NomregPrint::default()
        };
    }
    Ok(Some(out))
}

fn new_name(p: &mut Parser) -> PResult<Option<String>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    let name = p.name("a variable name")?;
    p.expect(Punct::RParen)?;
    Ok(Some(name))
}

fn save(p: &mut Parser) -> PResult<Vec<NomregSave>> {
    p.eat(Punct::Eq);
    let mut out: Vec<NomregSave> = Vec::new();
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
                    let Some(n) = p.number() else {
                        return Err(p.unexpected(&["a number"]));
                    };
                    count = checked(
                        p,
                        at,
                        n,
                        n >= 1.0 && n.fract() == 0.0,
                        "the number of probabilities is a positive whole number",
                    )? as u32;
                }
                p.expect(Punct::RParen)?;
            }
            NomregSave::Estprob { root, count }
        } else if p.eat_keyword("PREDCAT") {
            NomregSave::Predcat(new_name(p)?)
        } else if p.eat_keyword("PCPROB") {
            NomregSave::Pcprob(new_name(p)?)
        } else if p.eat_keyword("ACPROB") {
            NomregSave::Acprob(new_name(p)?)
        } else {
            return Err(p.unexpected(&["ACPROB", "ESTPROB", "PCPROB", "PREDCAT"]));
        };
        if out
            .iter()
            .any(|o| std::mem::discriminant(o) == std::mem::discriminant(&item))
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

pub(crate) fn nomreg(p: &mut Parser) -> PResult<Command> {
    let dependent = p.name("the dependent variable")?;
    let mut command = Nomreg {
        dependent,
        base: NomregBase::Last,
        order: NomregOrder::Ascending,
        factors: Vec::new(),
        covariates: Vec::new(),
        criteria: NomregCriteria::default(),
        model: None,
        fullfactorial: false,
        intercept: true,
        include: false,
        print: NomregPrint::default(),
        save: Vec::new(),
        scale: NomregScale::Value(1.0),
        subpop: None,
    };
    if p.eat(Punct::LParen) {
        dependent_options(p, &mut command)?;
    }
    if matches!(p.peek(), Some(TokenKind::Word(w))
        if !w.eq_ignore_ascii_case("BY") && !w.eq_ignore_ascii_case("WITH"))
    {
        return Err(SyntaxError::new(
            p.span(),
            "NOMREG takes one dependent variable; factors follow BY and covariates WITH",
        ));
    }
    if p.eat_word("BY") {
        command.factors = list_until(p, &["WITH"])?;
    }
    if p.eat_word("WITH") {
        command.covariates = p.var_list()?;
    }
    let mut model_given = false;
    while p.eat(Punct::Slash) {
        if p.eat_keyword("CRITERIA") {
            criteria(p, &mut command.criteria)?;
        } else if p.eat_keyword("FULLFACTORIAL") {
            command.fullfactorial = true;
        } else if p.eat_keyword("INTERCEPT") {
            p.eat(Punct::Eq);
            if p.eat_keyword("INCLUDE") {
                command.intercept = true;
            } else if p.eat_keyword("EXCLUDE") {
                command.intercept = false;
            } else if !at_end(p) {
                return Err(p.unexpected(&["INCLUDE", "EXCLUDE"]));
            }
        } else if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            if p.eat_keyword("INCLUDE") {
                command.include = true;
            } else if p.eat_keyword("EXCLUDE") {
                command.include = false;
            } else if !at_end(p) {
                return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
            }
        } else if p.eat_keyword("MODEL") {
            p.eat(Punct::Eq);
            model_given = true;
            let mut terms = Vec::new();
            while !at_end(p) {
                if p.at(Punct::Or) {
                    return Err(SyntaxError::new(
                        p.span(),
                        "stepwise methods on NOMREG MODEL (after '|') are not supported yet",
                    ));
                }
                if !p.eat_keyword("INTERCEPT") {
                    terms.push(term(p)?);
                }
                p.eat(Punct::Comma);
            }
            command.model = Some(terms);
        } else if p.eat_keyword("PRINT") {
            if let Some(print) = print(p)? {
                command.print = print;
            }
        } else if p.eat_keyword("SAVE") {
            let items = save(p)?;
            if !items.is_empty() {
                command.save = items;
            }
        } else if p.eat_keyword("SCALE") {
            p.eat(Punct::Eq);
            command.scale = if p.eat_keyword("DEVIANCE") {
                NomregScale::Deviance
            } else if p.eat_keyword("PEARSON") {
                NomregScale::Pearson
            } else if matches!(
                p.peek(),
                Some(TokenKind::Number(_) | TokenKind::Punct(Punct::Minus))
            ) {
                let at = p.pos;
                let n = p
                    .number()
                    .ok_or_else(|| p.unexpected(&["a positive number"]))?;
                NomregScale::Value(checked(p, at, n, n > 0.0, "SCALE takes a positive number")?)
            } else if at_end(p) {
                command.scale
            } else {
                return Err(p.unexpected(&["a positive number", "DEVIANCE", "PEARSON"]));
            };
        } else if p.eat_keyword("SUBPOP") {
            p.eat(Punct::Eq);
            if !at_end(p) {
                command.subpop = Some(p.var_list()?);
            }
        } else {
            return Err(p.subcommand_error("NOMREG", &["STEPWISE", "TEST", "OUTFILE"]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if model_given && command.fullfactorial {
        return Err(SyntaxError::new(
            p.span(),
            "MODEL and FULLFACTORIAL cannot both be given",
        ));
    }
    Ok(Command::Nomreg(Box::new(command)))
}
