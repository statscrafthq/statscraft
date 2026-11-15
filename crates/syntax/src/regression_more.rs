//! Parsers of PROBIT, CURVEFIT and WLS.
//!
//! PROBIT (rules noted from the Command Syntax Reference):
//! - `PROBIT responses OF subjects WITH covariates [BY factor(min,max)]`; BY and WITH come in
//!   either order after the two counts, each once. The factor range is two whole numbers.
//! - MODEL=PROBIT (default, also with no keyword) | LOGIT | BOTH. LOG=base (bare LOG: base e,
//!   left out: base 10) | NONE. CRITERIA: OPTOL(n) alias CONVERGE(n), ITERATE(n), P(p) for the
//!   heterogeneity cut between 0 and 1, STEPLIMIT(n). NATRES alone estimates the natural
//!   response rate, NATRES=value (from 0 to below 1) fixes it.
//! - PRINT left out or bare: FREQ, CI and RMP. With keywords only those: FREQ, CI, RMP,
//!   PARALL, DEFAULT (the first three), ALL (all four), NONE. MISSING=EXCLUDE (also DEFAULT) |
//!   INCLUDE. Every subcommand at most once.
//!
//! CURVEFIT:
//! - `[VARIABLES=] dependents [WITH independent]`; without WITH the cases' sequence numbers
//!   are the independent. MODEL names may be cut to three letters (S is one letter); ALL is
//!   all eleven. Every subcommand but VARIABLES may repeat; the last one counts.
//! - CIN between 0 and 100, UPPERBOUND=NO | a positive number, CONSTANT | NOCONSTANT,
//!   PLOT=FIT | NONE, ID=variable, PRINT=ANOVA, SAVE=PRED RESID CIN, TEMPLATE='file'.
//!
//! WLS:
//! - `[VARIABLES=] dependent WITH independents`. SOURCE=variable, DELTA (alias POWER) with
//!   values and grids `a TO b BY c` or `a BY c TO b`; every DELTA counts, at most 150 values.
//!   WEIGHT=variable instead of SOURCE and DELTA. CONSTANT | NOCONSTANT, PRINT=BEST | ALL,
//!   SAVE=WEIGHT. Other subcommands may repeat; the last counts.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::stats::variables;

/// The natural response rate of PROBIT.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NaturalResponse {
    /// Left at 0.
    None,
    /// Estimated with the other parameters.
    Estimate,
    /// Fixed at the given rate.
    Fixed(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Probit {
    pub response: String,
    pub total: String,
    /// The factor with its smallest and largest level.
    pub factor: Option<(String, f64, f64)>,
    pub covariates: Vec<VarItem>,
    pub probit: bool,
    pub logit: bool,
    /// The base of the logarithm taken of every covariate; None for LOG=NONE.
    pub log_base: Option<f64>,
    pub optol: Option<f64>,
    pub iterate: Option<usize>,
    /// Below this significance of the goodness-of-fit test the confidence limits use the
    /// heterogeneity factor.
    pub heterogeneity: f64,
    pub step_limit: f64,
    pub natres: NaturalResponse,
    pub freq: bool,
    pub ci: bool,
    pub rmp: bool,
    pub parall: bool,
    pub include: bool,
}

/// The eleven curves of CURVEFIT, in the order the output lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CurveModel {
    Linear,
    Logarithmic,
    Inverse,
    Quadratic,
    Cubic,
    Compound,
    Power,
    S,
    Growth,
    Exponential,
    Logistic,
}

pub const CURVE_MODELS: [(&str, CurveModel); 11] = [
    ("LINEAR", CurveModel::Linear),
    ("LOGARITHMIC", CurveModel::Logarithmic),
    ("INVERSE", CurveModel::Inverse),
    ("QUADRATIC", CurveModel::Quadratic),
    ("CUBIC", CurveModel::Cubic),
    ("COMPOUND", CurveModel::Compound),
    ("POWER", CurveModel::Power),
    ("S", CurveModel::S),
    ("GROWTH", CurveModel::Growth),
    ("EXPONENTIAL", CurveModel::Exponential),
    ("LGSTIC", CurveModel::Logistic),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Curvefit {
    pub dependents: Vec<VarItem>,
    pub independent: Option<String>,
    /// Sorted, without repeats.
    pub models: Vec<CurveModel>,
    /// The level of the saved prediction intervals, in percent.
    pub cin: f64,
    pub upper_bound: Option<f64>,
    pub constant: bool,
    pub plot: bool,
    pub id: Option<String>,
    pub anova: bool,
    pub save_pred: bool,
    pub save_resid: bool,
    pub save_cin: bool,
    pub template: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Wls {
    pub dependent: String,
    pub independents: Vec<VarItem>,
    pub source: Option<String>,
    /// The powers of every DELTA subcommand in order; empty when none was given.
    pub deltas: Vec<f64>,
    pub weight: Option<String>,
    pub constant: bool,
    pub print_all: bool,
    pub save_weight: bool,
}

/// The largest number of powers WLS takes.
const MAX_DELTAS: usize = 150;

fn error(p: &Parser, start: usize, message: impl Into<String>) -> SyntaxError {
    SyntaxError::new(p.span_from(start), message.into())
}

/// `(n)` after a CRITERIA keyword.
fn parenthesized(p: &mut Parser) -> PResult<f64> {
    p.expect(Punct::LParen)?;
    let Some(value) = p.number() else {
        return Err(p.unexpected(&["a number"]));
    };
    p.expect(Punct::RParen)?;
    Ok(value)
}

/// Fails when `name` was given before in this command, else notes it.
fn once(p: &Parser, start: usize, seen: &mut Vec<&'static str>, name: &'static str) -> PResult<()> {
    if seen.contains(&name) {
        return Err(error(p, start, format!("{name} may be given only once")));
    }
    seen.push(name);
    Ok(())
}

/// The variable list of CURVEFIT and WLS, after an optional `VARIABLES=`.
fn variables_with(p: &mut Parser) -> PResult<(Vec<VarItem>, Option<Vec<VarItem>>)> {
    if p.at_keyword("VARIABLES") && matches!(p.peek_at(1), Some(TokenKind::Punct(Punct::Eq))) {
        p.pos += 2;
    }
    let left = variables(p)?;
    let right = match p.eat_word("WITH") {
        true => Some(variables(p)?),
        false => None,
    };
    Ok((left, right))
}

pub(crate) fn probit(p: &mut Parser) -> PResult<Command> {
    let response = p.name("the variable of response counts")?;
    if !p.eat_word("OF") {
        return Err(p.unexpected(&["OF"]));
    }
    let total = p.name("the variable of subject counts")?;
    let (mut factor, mut covariates) = (None, None);
    loop {
        let start = p.pos;
        if p.eat_word("WITH") {
            if covariates.is_some() {
                return Err(error(p, start, "WITH may be given only once"));
            }
            covariates = Some(variables(p)?);
        } else if p.eat_word("BY") {
            if factor.is_some() {
                return Err(error(p, start, "PROBIT takes one factor after BY"));
            }
            let name = p.name("the factor")?;
            let range_start = p.pos;
            p.expect(Punct::LParen)?;
            let min = p.number();
            p.eat(Punct::Comma);
            let max = p.number();
            p.expect(Punct::RParen)?;
            match (min, max) {
                (Some(min), Some(max))
                    if min.fract() == 0.0 && max.fract() == 0.0 && min <= max =>
                {
                    factor = Some((name, min, max));
                }
                _ => {
                    return Err(error(
                        p,
                        range_start,
                        "the factor range is two whole numbers, the smaller first",
                    ));
                }
            }
        } else {
            break;
        }
    }
    let Some(covariates) = covariates else {
        return Err(p.unexpected(&["WITH", "BY"]));
    };
    let mut command = Probit {
        response,
        total,
        factor,
        covariates,
        probit: true,
        logit: false,
        log_base: Some(10.0),
        optol: None,
        iterate: None,
        heterogeneity: 0.15,
        step_limit: 0.1,
        natres: NaturalResponse::None,
        freq: true,
        ci: true,
        rmp: true,
        parall: false,
        include: false,
    };
    let mut seen = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        if p.eat_keyword("MODEL") {
            once(p, start, &mut seen, "MODEL")?;
            p.eat(Punct::Eq);
            (command.probit, command.logit) = if at_end(p) || p.eat_keyword("PROBIT") {
                (true, false)
            } else if p.eat_keyword("LOGIT") {
                (false, true)
            } else if p.eat_keyword("BOTH") {
                (true, true)
            } else {
                return Err(p.unexpected(&["PROBIT", "LOGIT", "BOTH"]));
            };
        } else if p.eat_word("LOG") {
            once(p, start, &mut seen, "LOG")?;
            p.eat(Punct::Eq);
            let value_start = p.pos;
            command.log_base = if at_end(p) {
                Some(std::f64::consts::E)
            } else if p.eat_keyword("NONE") {
                None
            } else {
                match p.number() {
                    Some(b) if b > 0.0 && b != 1.0 => Some(b),
                    Some(_) => {
                        return Err(error(
                            p,
                            value_start,
                            "the base of the logarithm is a positive number other than 1",
                        ));
                    }
                    None => return Err(p.unexpected(&["a number", "NONE"])),
                }
            };
        } else if p.eat_keyword("CRITERIA") {
            once(p, start, &mut seen, "CRITERIA")?;
            p.eat(Punct::Eq);
            while !at_end(p) {
                let key = p.pos;
                if p.eat_keyword("OPTOL") || p.eat_keyword("CONVERGE") {
                    let v = parenthesized(p)?;
                    if v <= 0.0 {
                        return Err(error(p, key, "OPTOL takes a positive number"));
                    }
                    command.optol = Some(v);
                } else if p.eat_keyword("ITERATE") {
                    let v = parenthesized(p)?;
                    if v < 1.0 || v.fract() != 0.0 {
                        return Err(error(p, key, "ITERATE takes a positive whole number"));
                    }
                    command.iterate = Some(v as usize);
                } else if p.eat_word("P") {
                    let v = parenthesized(p)?;
                    if !(0.0..=1.0).contains(&v) {
                        return Err(error(p, key, "P takes a probability from 0 to 1"));
                    }
                    command.heterogeneity = v;
                } else if p.eat_keyword("STEPLIMIT") {
                    let v = parenthesized(p)?;
                    if v <= 0.0 {
                        return Err(error(p, key, "STEPLIMIT takes a positive number"));
                    }
                    command.step_limit = v;
                } else {
                    return Err(p.unexpected(&["OPTOL", "CONVERGE", "ITERATE", "P", "STEPLIMIT"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("NATRES") {
            once(p, start, &mut seen, "NATRES")?;
            command.natres = if p.eat(Punct::Eq) || !at_end(p) {
                let value_start = p.pos;
                match p.number() {
                    Some(v) if (0.0..1.0).contains(&v) => NaturalResponse::Fixed(v),
                    Some(_) => {
                        return Err(error(
                            p,
                            value_start,
                            "a natural response rate is at least 0 and below 1",
                        ));
                    }
                    None => return Err(p.unexpected(&["a rate"])),
                }
            } else {
                NaturalResponse::Estimate
            };
        } else if p.eat_keyword("PRINT") {
            once(p, start, &mut seen, "PRINT")?;
            p.eat(Punct::Eq);
            if !at_end(p) {
                let mut set = [false; 4];
                while !at_end(p) {
                    if p.eat_keyword("FREQ") {
                        set[0] = true;
                    } else if p.eat_word("CI") {
                        set[1] = true;
                    } else if p.eat_word("RMP") {
                        set[2] = true;
                    } else if p.eat_keyword("PARALL") {
                        set[3] = true;
                    } else if p.eat_keyword("DEFAULT") {
                        set[..3].fill(true);
                    } else if p.eat_word("ALL") {
                        set = [true; 4];
                    } else if p.eat_keyword("NONE") {
                        set = [false; 4];
                    } else {
                        return Err(p.unexpected(&[
                            "FREQ", "CI", "RMP", "PARALL", "DEFAULT", "ALL", "NONE",
                        ]));
                    }
                    p.eat(Punct::Comma);
                }
                [command.freq, command.ci, command.rmp, command.parall] = set;
            }
        } else if p.eat_keyword("MISSING") {
            once(p, start, &mut seen, "MISSING")?;
            p.eat(Punct::Eq);
            command.include = if p.eat_keyword("INCLUDE") {
                true
            } else if p.eat_keyword("EXCLUDE") || p.eat_keyword("DEFAULT") {
                false
            } else {
                return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
            };
        } else {
            return Err(p.subcommand_error("PROBIT", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Probit(Box::new(command)))
}

pub(crate) fn curvefit(p: &mut Parser) -> PResult<Command> {
    let mut command = Curvefit {
        dependents: Vec::new(),
        independent: None,
        models: vec![CurveModel::Linear],
        cin: 95.0,
        upper_bound: None,
        constant: true,
        plot: true,
        id: None,
        anova: false,
        save_pred: false,
        save_resid: false,
        save_cin: false,
        template: false,
    };
    // a leading slash is allowed when another subcommand comes before VARIABLES
    let mut first = !p.eat(Punct::Slash);
    loop {
        let start = p.pos;
        if first && p.at_keyword("APPLY") || !first && p.eat_keyword("APPLY") {
            return Err(error(p, start, "APPLY of CURVEFIT is not supported yet"));
        }
        let variables_here = (first && !matches!(p.peek(), Some(TokenKind::Punct(_)) | None))
            || p.at_keyword("VARIABLES");
        if variables_here {
            if !command.dependents.is_empty() {
                return Err(error(p, start, "VARIABLES may be given only once"));
            }
            let (left, right) = variables_with(p)?;
            command.dependents = left;
            command.independent = match right.as_deref() {
                None => None,
                Some([VarItem::Name(name)]) => Some(name.clone()),
                Some(_) => {
                    return Err(error(
                        p,
                        start,
                        "CURVEFIT takes one independent variable after WITH",
                    ));
                }
            };
        } else if p.eat_keyword("MODEL") {
            p.eat(Punct::Eq);
            let mut models = Vec::new();
            while !at_end(p) {
                if p.eat_word("ALL") {
                    models.extend(CURVE_MODELS.iter().map(|(_, m)| *m));
                } else if let Some((_, m)) = CURVE_MODELS.iter().find(|(name, _)| match *name {
                    "S" => {
                        matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("S"))
                    }
                    _ => p.at_keyword(name),
                }) {
                    p.pos += 1;
                    models.push(*m);
                } else {
                    let mut names: Vec<&str> = CURVE_MODELS.iter().map(|(n, _)| *n).collect();
                    names.push("ALL");
                    return Err(p.unexpected(&names));
                }
                p.eat(Punct::Comma);
            }
            if models.is_empty() {
                return Err(p.unexpected(&["a model name"]));
            }
            models.sort();
            models.dedup();
            command.models = models;
        } else if p.eat_keyword("CIN") {
            p.eat(Punct::Eq);
            match p.number() {
                Some(v) if v > 0.0 && v < 100.0 => command.cin = v,
                Some(_) => return Err(error(p, start, "CIN is above 0 and below 100")),
                None => return Err(p.unexpected(&["a percentage"])),
            }
        } else if p.eat_keyword("UPPERBOUND") {
            p.eat(Punct::Eq);
            command.upper_bound = if p.eat_word("NO") {
                None
            } else {
                match p.number() {
                    Some(v) if v > 0.0 => Some(v),
                    Some(_) => return Err(error(p, start, "UPPERBOUND is a positive number")),
                    None => return Err(p.unexpected(&["NO", "a number"])),
                }
            };
        } else if p.eat_keyword("CONSTANT") {
            command.constant = true;
        } else if p.eat_keyword("NOCONSTANT") {
            command.constant = false;
        } else if p.eat_keyword("PLOT") {
            p.eat(Punct::Eq);
            command.plot = if p.eat_word("FIT") {
                true
            } else if p.eat_keyword("NONE") {
                false
            } else {
                return Err(p.unexpected(&["FIT", "NONE"]));
            };
        } else if p.eat_word("ID") {
            p.eat(Punct::Eq);
            command.id = Some(p.name("a variable name")?);
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("ANOVA") {
                return Err(p.unexpected(&["ANOVA"]));
            }
            command.anova = true;
        } else if p.eat_keyword("SAVE") {
            p.eat(Punct::Eq);
            let (mut pred, mut resid, mut cin) = (false, false, false);
            while !at_end(p) {
                if p.eat_keyword("PRED") {
                    pred = true;
                } else if p.eat_keyword("RESID") {
                    resid = true;
                } else if p.eat_word("CIN") {
                    cin = true;
                } else {
                    return Err(p.unexpected(&["PRED", "RESID", "CIN"]));
                }
                p.eat(Punct::Comma);
            }
            if !(pred || resid || cin) {
                return Err(p.unexpected(&["PRED", "RESID", "CIN"]));
            }
            (command.save_pred, command.save_resid, command.save_cin) = (pred, resid, cin);
        } else if p.eat_keyword("TEMPLATE") {
            p.eat(Punct::Eq);
            p.file("a template file")?;
            command.template = true;
        } else {
            return Err(p.subcommand_error("CURVEFIT", &["APPLY"]));
        }
        first = false;
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    if command.dependents.is_empty() {
        return Err(p.unexpected(&["VARIABLES"]));
    }
    Ok(Command::Curvefit(Box::new(command)))
}

/// The values and grids of one DELTA subcommand.
fn deltas(p: &mut Parser, out: &mut Vec<f64>) -> PResult<()> {
    while !at_end(p) {
        let start = p.pos;
        let Some(first) = p.number() else {
            return Err(p.unexpected(&["a power"]));
        };
        let (mut to, mut by) = (None, None);
        for _ in 0..2 {
            if to.is_none() && p.eat_word("TO") {
                to = Some(
                    p.number()
                        .ok_or_else(|| p.unexpected(&["the last power"]))?,
                );
            } else if by.is_none() && p.eat_word("BY") {
                by = Some(p.number().ok_or_else(|| p.unexpected(&["the step"]))?);
            }
        }
        match (to, by) {
            (None, None) => out.push(first),
            (Some(last), Some(step)) if step > 0.0 && last >= first => {
                // a whole number of steps, allowing for rounding in the step
                let steps = ((last - first) / step + 1e-9).floor() as usize;
                if steps >= MAX_DELTAS {
                    return Err(error(
                        p,
                        start,
                        format!("WLS takes at most {MAX_DELTAS} powers"),
                    ));
                }
                out.extend((0..=steps).map(|k| first + k as f64 * step));
            }
            _ => {
                return Err(error(
                    p,
                    start,
                    "a grid of powers is `first TO last BY step` with a positive step and last not below first",
                ));
            }
        }
        if out.len() > MAX_DELTAS {
            return Err(error(
                p,
                start,
                format!("WLS takes at most {MAX_DELTAS} powers"),
            ));
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

pub(crate) fn wls(p: &mut Parser) -> PResult<Command> {
    let mut command = Wls {
        dependent: String::new(),
        independents: Vec::new(),
        source: None,
        deltas: Vec::new(),
        weight: None,
        constant: true,
        print_all: false,
        save_weight: false,
    };
    // a leading slash is allowed when another subcommand comes before VARIABLES
    let mut first = !p.eat(Punct::Slash);
    loop {
        let start = p.pos;
        if first && p.at_keyword("APPLY") || !first && p.eat_keyword("APPLY") {
            return Err(error(p, start, "APPLY of WLS is not supported yet"));
        }
        let variables_here = (first && !matches!(p.peek(), Some(TokenKind::Punct(_)) | None))
            || p.at_keyword("VARIABLES");
        if variables_here {
            if !command.dependent.is_empty() {
                return Err(error(p, start, "VARIABLES may be given only once"));
            }
            let (left, right) = variables_with(p)?;
            let ([VarItem::Name(dependent)], Some(right)) = (left.as_slice(), right) else {
                return Err(error(
                    p,
                    start,
                    "WLS takes one dependent variable, WITH and the independent variables",
                ));
            };
            command.dependent = dependent.clone();
            command.independents = right;
        } else if p.eat_keyword("SOURCE") {
            p.eat(Punct::Eq);
            command.source = Some(p.name("the source variable")?);
        } else if p.eat_keyword("DELTA") || p.eat_keyword("POWER") {
            p.eat(Punct::Eq);
            deltas(p, &mut command.deltas)?;
        } else if p.eat_keyword("WEIGHT") {
            p.eat(Punct::Eq);
            command.weight = Some(p.name("the weight variable")?);
        } else if p.eat_keyword("CONSTANT") {
            command.constant = true;
        } else if p.eat_keyword("NOCONSTANT") {
            command.constant = false;
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            command.print_all = if p.eat_keyword("BEST") {
                false
            } else if p.eat_word("ALL") {
                true
            } else {
                return Err(p.unexpected(&["BEST", "ALL"]));
            };
        } else if p.eat_keyword("SAVE") {
            p.eat(Punct::Eq);
            if !p.eat_keyword("WEIGHT") {
                return Err(p.unexpected(&["WEIGHT"]));
            }
            command.save_weight = true;
        } else {
            return Err(p.subcommand_error("WLS", &["APPLY"]));
        }
        first = false;
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    if command.dependent.is_empty() {
        return Err(p.unexpected(&["VARIABLES"]));
    }
    if command.weight.is_some() && (command.source.is_some() || !command.deltas.is_empty()) {
        return Err(SyntaxError::new(
            p.span(),
            "WLS takes either WEIGHT or SOURCE and DELTA, not both",
        ));
    }
    Ok(Command::Wls(Box::new(command)))
}
