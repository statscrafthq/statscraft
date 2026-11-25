//! Parsers of MODEL PROGRAM, DERIVATIVES, CONSTRAINED FUNCTIONS, CLEAR MODEL PROGRAMS, NLR and CNLR
//! (nonlinear regression, unconstrained and constrained).
//!
//! Rules, from the Command Syntax Reference:
//! - `MODEL PROGRAM parameter[=value] ...` names every parameter of the model once, with a
//!   starting value (a number, signed) unless the FILE of NLR supplies it. Commas may separate
//!   the parameters. The transformations that follow, up to the next command of another kind,
//!   make up the model program; `DERIVATIVES.` starts the derivatives program inside it.
//! - `NLR dependent` with the subcommands FILE, OUTFILE (each one file), PRED (a variable
//!   name), SAVE (PRED, RESID with an optional name in parentheses, DERIVATIVES) and CRITERIA
//!   (ITER, CKDER, SSCON, PCON, RCON). A criterion's value follows the keyword in parentheses,
//!   after an equals sign, after a comma or after a space. A subcommand that repeats counts
//!   with its last occurrence.
//! - CNLR takes the same subcommands and SAVE also knows LOSS; its CRITERIA are ITER,
//!   MINORITERATION, CRSHTOL, STEPLIMIT, FTOLERANCE, LFTOLERANCE, NFTOLERANCE, LSTOLERANCE,
//!   OPTOLERANCE, FPRECISION, ISTEP and CKDER (a number in parentheses, after an equals sign, a
//!   comma or a space). LOSS=variable names the variable of the model program that holds the
//!   loss. BOOTSTRAP takes an optional number of samples. BOUNDS holds bounds separated by
//!   semicolons: each is two or three linear forms (sums of numbers, names and number times name)
//!   joined by = or EQ, <= or LE, >= or GE; two relations must point the same way.

use crate::ast::Command;
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// A parameter of the model with its starting value, if the command gives one.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelParameter {
    pub name: String,
    pub value: Option<f64>,
}

/// The CRITERIA of NLR; None keeps the default.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NlrCriteria {
    /// Most iterations; the default is 100 per parameter.
    pub iterations: Option<u32>,
    /// The critical value of the derivative check, 0 to 1 (default 0.5; 0 switches the check off).
    pub ckder: Option<f64>,
    /// The proportional reduction of the sum of squares that counts as no progress (1E-8).
    pub sscon: Option<f64>,
    /// The proportional change of every parameter that counts as no change (1E-8).
    pub pcon: Option<f64>,
    /// The correlation between residuals and derivatives that counts as zero (1E-8).
    pub rcon: Option<f64>,
}

/// SAVE of NLR: the variables appended to the active dataset.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NlrSave {
    pub pred: bool,
    /// The residuals; the name in parentheses, else the name the keyword was written with.
    pub resid: Option<String>,
    pub derivatives: bool,
    /// The loss variable (CNLR only).
    pub loss: bool,
}

/// NLR.
#[derive(Debug, Clone, PartialEq)]
pub struct Nlr {
    pub dependent: String,
    pub file: Option<String>,
    pub outfile: Option<String>,
    pub pred: Option<String>,
    pub save: NlrSave,
    pub criteria: NlrCriteria,
}

pub(crate) fn model_program(p: &mut Parser) -> PResult<Command> {
    let mut parameters: Vec<ModelParameter> = Vec::new();
    while p.peek().is_some() {
        let start = p.pos;
        let name = p.name("a parameter name")?;
        if parameters
            .iter()
            .any(|q| q.name.eq_ignore_ascii_case(&name))
        {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("the parameter {name} is named twice"),
            ));
        }
        let value = if p.eat(Punct::Eq) {
            match p.number() {
                Some(v) => Some(v),
                None => return Err(p.unexpected(&["a starting value"])),
            }
        } else {
            None
        };
        parameters.push(ModelParameter { name, value });
        p.eat(Punct::Comma);
    }
    if parameters.is_empty() {
        return Err(p.unexpected(&["a parameter name"]));
    }
    Ok(Command::ModelProgram(parameters))
}

pub(crate) fn derivatives(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::Derivatives)
}

pub(crate) fn clear_model_programs(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::ClearModelPrograms)
}

/// A criterion's value: `(n)`, `=n`, `,n` or ` n`.
fn value(p: &mut Parser, keyword: &str) -> PResult<f64> {
    let start = p.pos;
    if !p.eat(Punct::Eq) {
        p.eat(Punct::Comma);
    }
    let open = p.eat(Punct::LParen);
    let Some(v) = p.number() else {
        return Err(p.unexpected(&["a number"]));
    };
    if open {
        p.expect(Punct::RParen)?;
    }
    if v < 0.0 {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{keyword} is not negative"),
        ));
    }
    Ok(v)
}

fn criteria(p: &mut Parser, into: &mut NlrCriteria) -> PResult<()> {
    p.eat(Punct::Eq);
    let mut any = false;
    while !at_end(p) {
        let start = p.pos;
        if p.eat_keyword("ITER") {
            let v = value(p, "ITER")?;
            if v < 1.0 || v.fract() != 0.0 || v > f64::from(u32::MAX) {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "ITER is a whole number above 0",
                ));
            }
            into.iterations = Some(v as u32);
        } else if p.eat_keyword("CKDER") {
            let v = value(p, "CKDER")?;
            if v > 1.0 {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "CKDER lies from 0 to 1",
                ));
            }
            into.ckder = Some(v);
        } else if p.eat_keyword("SSCON") {
            into.sscon = Some(value(p, "SSCON")?);
        } else if p.eat_keyword("PCON") {
            into.pcon = Some(value(p, "PCON")?);
        } else if p.eat_keyword("RCON") {
            into.rcon = Some(value(p, "RCON")?);
        } else {
            return Err(p.unexpected(&["ITER", "CKDER", "SSCON", "PCON", "RCON"]));
        }
        any = true;
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&["ITER", "CKDER", "SSCON", "PCON", "RCON"]));
    }
    Ok(())
}

fn save(p: &mut Parser, loss_allowed: bool) -> PResult<NlrSave> {
    p.eat(Punct::Eq);
    let mut save = NlrSave::default();
    let mut any = false;
    while !at_end(p) {
        if p.eat_keyword("PRED") {
            save.pred = true;
        } else if let Some(TokenKind::Word(w)) = p.peek()
            && crate::table::matches_keyword(w, "RESID")
        {
            let written = w.clone();
            p.pos += 1;
            save.resid = Some(if p.eat(Punct::LParen) {
                let name = p.name("a variable name")?;
                p.expect(Punct::RParen)?;
                name
            } else {
                written
            });
        } else if p.eat_keyword("DERIVATIVES") {
            save.derivatives = true;
        } else if p.at_keyword("LOSS") {
            if !loss_allowed {
                return Err(SyntaxError::new(
                    p.span(),
                    "LOSS of SAVE is a CNLR keyword: NLR minimizes squared residuals",
                ));
            }
            p.pos += 1;
            save.loss = true;
        } else {
            return Err(p.unexpected(&["PRED", "RESID", "DERIVATIVES", "LOSS"]));
        }
        any = true;
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&["PRED", "RESID", "DERIVATIVES", "LOSS"]));
    }
    Ok(save)
}

pub(crate) fn nlr(p: &mut Parser) -> PResult<Command> {
    let dependent = p.name("the dependent variable")?;
    let mut command = Nlr {
        dependent,
        file: None,
        outfile: None,
        pred: None,
        save: NlrSave::default(),
        criteria: NlrCriteria::default(),
    };
    while p.eat(Punct::Slash) {
        if p.eat_keyword("FILE") {
            p.eat(Punct::Eq);
            command.file = Some(p.file("a file")?);
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            command.outfile = Some(p.file("a file")?);
        } else if p.eat_keyword("PRED") {
            p.eat(Punct::Eq);
            command.pred = Some(p.name("a variable name")?);
        } else if p.eat_keyword("SAVE") {
            command.save = save(p, false)?;
        } else if p.eat_keyword("CRITERIA") {
            criteria(p, &mut command.criteria)?;
        } else {
            return Err(p.subcommand_error("NLR", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Nlr(Box::new(command)))
}

pub(crate) fn constrained_functions(p: &mut Parser) -> PResult<Command> {
    p.finish(&[])?;
    Ok(Command::ConstrainedFunctions)
}

/// A linear form: a sum of numbers times names, and a constant.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LinearForm {
    pub terms: Vec<(String, f64)>,
    pub constant: f64,
}

/// How the two sides of a bound relate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundRelation {
    Le,
    Ge,
    Eq,
}

/// One bound of BOUNDS: two or three linear forms and the relations between them.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterBound {
    pub sides: Vec<LinearForm>,
    pub relations: Vec<BoundRelation>,
}

/// The CRITERIA of CNLR; None keeps the default.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CnlrCriteria {
    pub iterations: Option<u32>,
    pub minor_iterations: Option<u32>,
    pub crshtol: Option<f64>,
    pub steplimit: Option<f64>,
    pub ftolerance: Option<f64>,
    pub lftolerance: Option<f64>,
    pub nftolerance: Option<f64>,
    pub lstolerance: Option<f64>,
    pub optolerance: Option<f64>,
    pub fprecision: Option<f64>,
    pub istep: Option<f64>,
    pub ckder: Option<f64>,
}

/// CNLR.
#[derive(Debug, Clone, PartialEq)]
pub struct Cnlr {
    pub dependent: String,
    pub file: Option<String>,
    pub outfile: Option<String>,
    pub pred: Option<String>,
    pub save: NlrSave,
    pub criteria: CnlrCriteria,
    pub bounds: Vec<ParameterBound>,
    /// The variable of the model program that holds the loss of a case.
    pub loss: Option<String>,
    /// BOOTSTRAP: Some(None) for the default number of samples.
    pub bootstrap: Option<Option<u32>>,
}

fn is_relation_word(p: &Parser) -> bool {
    matches!(p.peek(), Some(TokenKind::Word(w))
        if ["EQ", "LE", "GE"].iter().any(|r| w.eq_ignore_ascii_case(r)))
}

fn relation(p: &mut Parser) -> Option<BoundRelation> {
    if p.eat(Punct::Le) || p.eat_word("LE") {
        Some(BoundRelation::Le)
    } else if p.eat(Punct::Ge) || p.eat_word("GE") {
        Some(BoundRelation::Ge)
    } else if p.eat(Punct::Eq) || p.eat_word("EQ") {
        Some(BoundRelation::Eq)
    } else {
        None
    }
}

/// A linear form: terms of numbers and one name each, joined by + and -, the product by *.
fn linear(p: &mut Parser) -> PResult<LinearForm> {
    let mut form = LinearForm::default();
    let mut first = true;
    loop {
        let mut sign = 1.0;
        if p.eat(Punct::Plus) {
        } else if p.eat(Punct::Minus) {
            sign = -1.0;
        } else if !first {
            break;
        }
        let start = p.pos;
        let mut coefficient = 1.0;
        let mut name: Option<String> = None;
        loop {
            if let Some(v) = p.number() {
                coefficient *= v;
            } else if matches!(p.peek(), Some(TokenKind::Word(_))) && !is_relation_word(p) {
                if name.is_some() {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "a bound is linear: a term has one parameter",
                    ));
                }
                name = Some(p.name("a parameter name")?);
            } else {
                return Err(p.unexpected(&["a number", "a parameter name"]));
            }
            if !p.eat(Punct::Star) {
                break;
            }
        }
        match name {
            Some(n) => form.terms.push((n, sign * coefficient)),
            None => form.constant += sign * coefficient,
        }
        first = false;
        if !(p.at(Punct::Plus) || p.at(Punct::Minus)) {
            break;
        }
    }
    Ok(form)
}

fn bound(p: &mut Parser) -> PResult<ParameterBound> {
    let start = p.pos;
    let mut sides = vec![linear(p)?];
    let mut relations = Vec::new();
    while let Some(r) = relation(p) {
        relations.push(r);
        sides.push(linear(p)?);
    }
    let mixed = relations.len() == 2 && relations[0] != relations[1];
    if relations.is_empty()
        || relations.len() > 2
        || mixed
        || (relations.len() == 2 && relations[0] == BoundRelation::Eq)
    {
        return Err(SyntaxError::new(
            p.span_from(start),
            "a bound has one relation, or two that point the same way (<= and <=, or >= and >=)",
        ));
    }
    Ok(ParameterBound { sides, relations })
}

fn bounds(p: &mut Parser, into: &mut Vec<ParameterBound>) -> PResult<()> {
    p.eat(Punct::Eq);
    loop {
        into.push(bound(p)?);
        if !p.eat(Punct::Semicolon) || at_end(p) {
            return Ok(());
        }
    }
}

fn cnlr_criteria(p: &mut Parser, into: &mut CnlrCriteria) -> PResult<()> {
    p.eat(Punct::Eq);
    const NAMES: [&str; 12] = [
        "ITER",
        "MINORITERATION",
        "CRSHTOL",
        "STEPLIMIT",
        "FTOLERANCE",
        "LFTOLERANCE",
        "NFTOLERANCE",
        "LSTOLERANCE",
        "OPTOLERANCE",
        "FPRECISION",
        "ISTEP",
        "CKDER",
    ];
    let mut any = false;
    while !at_end(p) {
        let start = p.pos;
        let Some(name) = NAMES.iter().find(|n| p.at_keyword(n)) else {
            return Err(p.unexpected(&NAMES));
        };
        p.pos += 1;
        let v = value(p, name)?;
        let check = |ok: bool, what: &str| {
            if ok {
                Ok(())
            } else {
                Err(SyntaxError::new(
                    p.span_from(start),
                    format!("{name} {what}"),
                ))
            }
        };
        match *name {
            "ITER" | "MINORITERATION" => {
                check(
                    v >= 1.0 && v.fract() == 0.0 && v <= f64::from(u32::MAX),
                    "is a whole number above 0",
                )?;
                if *name == "ITER" {
                    into.iterations = Some(v as u32);
                } else {
                    into.minor_iterations = Some(v as u32);
                }
            }
            "CRSHTOL" => {
                check(v <= 1.0, "lies from 0 to 1")?;
                into.crshtol = Some(v);
            }
            "STEPLIMIT" => {
                check(v > 0.0, "is above 0")?;
                into.steplimit = Some(v);
            }
            "FTOLERANCE" => {
                check(v > 0.0, "is above 0")?;
                into.ftolerance = Some(v);
            }
            "LFTOLERANCE" => {
                check(v > 0.0, "is above 0")?;
                into.lftolerance = Some(v);
            }
            "NFTOLERANCE" => {
                check(v > 0.0, "is above 0")?;
                into.nftolerance = Some(v);
            }
            "LSTOLERANCE" => {
                check(v < 1.0, "lies from 0 to below 1")?;
                into.lstolerance = Some(v);
            }
            "OPTOLERANCE" => {
                check(v > 0.0 && v < 1.0, "lies above 0 and below 1")?;
                into.optolerance = Some(v);
            }
            "FPRECISION" => {
                check(v > 0.0 && v < 1.0, "lies above 0 and below 1")?;
                into.fprecision = Some(v);
            }
            "ISTEP" => {
                check(v > 0.0, "is above 0")?;
                into.istep = Some(v);
            }
            _ => {
                check(v <= 1.0, "lies from 0 to 1")?;
                into.ckder = Some(v);
            }
        }
        any = true;
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&NAMES));
    }
    Ok(())
}

pub(crate) fn cnlr(p: &mut Parser) -> PResult<Command> {
    let dependent = p.name("the dependent variable")?;
    let mut command = Cnlr {
        dependent,
        file: None,
        outfile: None,
        pred: None,
        save: NlrSave::default(),
        criteria: CnlrCriteria::default(),
        bounds: Vec::new(),
        loss: None,
        bootstrap: None,
    };
    while p.eat(Punct::Slash) {
        let start = p.pos;
        if p.eat_keyword("FILE") {
            p.eat(Punct::Eq);
            command.file = Some(p.file("a file")?);
        } else if p.eat_keyword("OUTFILE") {
            p.eat(Punct::Eq);
            command.outfile = Some(p.file("a file")?);
        } else if p.eat_keyword("PRED") {
            p.eat(Punct::Eq);
            command.pred = Some(p.name("a variable name")?);
        } else if p.eat_keyword("SAVE") {
            command.save = save(p, true)?;
        } else if p.eat_keyword("CRITERIA") {
            cnlr_criteria(p, &mut command.criteria)?;
        } else if p.eat_keyword("BOUNDS") {
            bounds(p, &mut command.bounds)?;
        } else if p.eat_keyword("LOSS") {
            p.eat(Punct::Eq);
            command.loss = Some(p.name("the loss variable")?);
        } else if p.eat_keyword("BOOTSTRAP") {
            let open = p.eat(Punct::Eq);
            let paren = p.eat(Punct::LParen);
            command.bootstrap = Some(match p.number() {
                Some(v) if v >= 2.0 && v.fract() == 0.0 && v <= f64::from(u32::MAX) => {
                    Some(v as u32)
                }
                Some(_) => {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "BOOTSTRAP takes a whole number of samples, at least 2",
                    ));
                }
                None if open || paren => return Err(p.unexpected(&["a number of samples"])),
                None => None,
            });
            if paren {
                p.expect(Punct::RParen)?;
            }
        } else {
            return Err(p.subcommand_error("CNLR", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Cnlr(Box::new(command)))
}
