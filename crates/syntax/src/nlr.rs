//! Parsers of MODEL PROGRAM, DERIVATIVES, CLEAR MODEL PROGRAMS and NLR (nonlinear regression).
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

fn save(p: &mut Parser) -> PResult<NlrSave> {
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
        } else {
            return Err(p.unexpected(&["PRED", "RESID", "DERIVATIVES"]));
        }
        any = true;
        p.eat(Punct::Comma);
    }
    if !any {
        return Err(p.unexpected(&["PRED", "RESID", "DERIVATIVES"]));
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
            command.save = save(p)?;
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
