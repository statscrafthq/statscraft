//! Parser of LOGLINEAR, the general loglinear model with contrasts (rules noted from the
//! Command Syntax Reference):
//! - `factors(min,max) ... [BY factors(min,max) ...] [WITH covariates]`: each group closed by
//!   its whole-number range; the factors after BY are the independent ones of a logit model.
//!   At most 10 factors and 200 covariates, checked by the engine, which expands the lists.
//! - Subcommands form an ordered stream. DESIGN fits a model with the settings given before it
//!   (they carry over to later designs); DESIGN itself does not. When the last subcommand is not
//!   DESIGN one more, saturated model is added. A DESIGN without effects is the saturated model.
//! - CWEIGHT: one variable (only once in the command) or matrices `(w n*w ...)`, any number.
//!   GRESID: a variable or a matrix of coefficients, each its own linear combination, all of
//!   them shown for every design. CONTRAST(factor)=DEVIATION[(ref)] | DIFFERENCE | HELMERT |
//!   SIMPLE[(ref)] | REPEATED | POLYNOMIAL[(metric)] | [BASIS] SPECIAL(matrix), the last given
//!   for a factor in force. CRITERIA: CONVERGE(0.001), ITERATE(20) (ITERATION accepted),
//!   DELTA(0.5), DEFAULT. PRINT: FREQ, RESID, DESIGN, ESTIM, COR (COV), DEFAULT, ALL, NONE;
//!   cumulative, NONE clears everything. PLOT: RESID, NORMPROB, DEFAULT (ALL), NONE;
//!   cumulative, NONE clears. MISSING=EXCLUDE|INCLUDE once.
//! - Effects of DESIGN: names joined by BY, `factor(n)` the n-th partition (contrast column).

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::{Contrast, list_until};
use crate::lexer::{Punct, TokenKind};
use crate::loglinear::{
    CellWeights, GenlogPart, HiloRange, check, genlog_design, missing, value_in_parens,
    value_matrix, weight_matrix,
};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// The contrast of one factor.
#[derive(Debug, Clone, PartialEq)]
pub enum LoglinearContrastKind {
    Named(Contrast),
    /// The k x k matrix as given; `basis` (BASIS SPECIAL) makes the contrasts equal to it,
    /// otherwise its transpose is the basis of the design columns.
    Special { values: Vec<f64>, basis: bool },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoglinearContrast {
    pub variable: String,
    pub kind: LoglinearContrastKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoglinearCriteria {
    pub converge: f64,
    pub iterate: u32,
    pub delta: f64,
}

impl Default for LoglinearCriteria {
    fn default() -> Self {
        Self {
            converge: 0.001,
            iterate: 20,
            delta: 0.5,
        }
    }
}

/// What PRINT shows for one model. ESTIM is added for a saturated model that DESIGN did not
/// name (`estim_by_default`) unless NONE was given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoglinearPrint {
    pub freq: bool,
    pub resid: bool,
    pub design: bool,
    pub estim: bool,
    pub cor: bool,
    pub none: bool,
}

impl Default for LoglinearPrint {
    fn default() -> Self {
        Self {
            freq: true,
            resid: true,
            design: false,
            estim: false,
            cor: false,
            none: false,
        }
    }
}

/// One DESIGN with the settings in force for it.
#[derive(Debug, Clone, PartialEq)]
pub struct LoglinearModel {
    /// None: the saturated model of every effect.
    pub terms: Option<Vec<Vec<GenlogPart>>>,
    pub contrasts: Vec<LoglinearContrast>,
    pub criteria: LoglinearCriteria,
    pub cweight: Option<CellWeights>,
    pub gresid: Vec<CellWeights>,
    pub print: LoglinearPrint,
    pub plot_resid: bool,
    pub plot_normal: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Loglinear {
    pub dependents: Vec<HiloRange>,
    /// The factors after BY: a logit model.
    pub independents: Vec<HiloRange>,
    pub covariates: Vec<VarItem>,
    pub models: Vec<LoglinearModel>,
    /// The last model was added because the command did not end with DESIGN.
    pub implicit_design: bool,
    pub include: bool,
}

fn ranges(p: &mut Parser, stop: &[&str]) -> PResult<Vec<HiloRange>> {
    let mut out = Vec::new();
    while let Some(TokenKind::Word(w)) = p.peek() {
        if stop.iter().any(|s| w.eq_ignore_ascii_case(s)) {
            break;
        }
        let variables = list_until(p, stop)?;
        let start = p.pos;
        p.expect(Punct::LParen)?;
        let Some(min) = p.number() else {
            return Err(p.unexpected(&["the minimum value"]));
        };
        p.eat(Punct::Comma);
        let Some(max) = p.number() else {
            return Err(p.unexpected(&["the maximum value"]));
        };
        p.expect(Punct::RParen)?;
        check(
            p,
            start,
            min.fract() == 0.0 && max.fract() == 0.0 && min <= max,
            "the range is two whole numbers, the minimum first",
        )?;
        out.push(HiloRange {
            variables,
            min,
            max,
        });
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(out)
}

fn criteria(p: &mut Parser, c: &mut LoglinearCriteria) -> PResult<()> {
    p.eat(Punct::Eq);
    if at_end(p) {
        return Err(p.unexpected(&["CONVERGE", "ITERATE", "DELTA", "DEFAULT"]));
    }
    while !at_end(p) {
        let start = p.pos;
        if p.eat_keyword("CONVERGE") {
            c.converge = value_in_parens(p)?;
            check(p, start, c.converge > 0.0, "CONVERGE is above 0")?;
        } else if p.eat_keyword("ITERATE") || p.eat_keyword("ITERATION") {
            let v = value_in_parens(p)?;
            check(
                p,
                start,
                v >= 1.0 && v.fract() == 0.0 && v <= f64::from(u32::MAX),
                "ITERATE is a whole number above 0",
            )?;
            c.iterate = v as u32;
        } else if p.eat_keyword("DELTA") {
            c.delta = value_in_parens(p)?;
            check(p, start, c.delta >= 0.0, "DELTA is not negative")?;
        } else if p.eat_keyword("DEFAULT") {
            *c = LoglinearCriteria::default();
        } else {
            return Err(p.unexpected(&["CONVERGE", "ITERATE", "DELTA", "DEFAULT"]));
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn print(p: &mut Parser, state: &mut LoglinearPrint) -> PResult<()> {
    p.eat(Punct::Eq);
    let mut none = false;
    let mut next = *state;
    let mut any = false;
    while !at_end(p) {
        any = true;
        if p.eat_keyword("FREQ") {
            next.freq = true;
        } else if p.eat_keyword("RESID") {
            next.resid = true;
        } else if p.eat_keyword("DESIGN") {
            next.design = true;
        } else if p.eat_keyword("ESTIM") {
            next.estim = true;
        } else if p.eat_keyword("COR") || p.eat_keyword("COV") {
            next.cor = true;
        } else if p.eat_keyword("DEFAULT") {
            next.freq = true;
            next.resid = true;
        } else if p.eat_word("ALL") {
            next = LoglinearPrint {
                freq: true,
                resid: true,
                design: true,
                estim: true,
                cor: true,
                none: false,
            };
        } else if p.eat_keyword("NONE") {
            none = true;
        } else {
            return Err(p.unexpected(&[
                "FREQ", "RESID", "DESIGN", "ESTIM", "COR", "DEFAULT", "ALL", "NONE",
            ]));
        }
        p.eat(Punct::Comma);
    }
    if !any {
        next.freq = true;
        next.resid = true;
    }
    *state = if none {
        LoglinearPrint {
            freq: false,
            resid: false,
            design: false,
            estim: false,
            cor: false,
            none: true,
        }
    } else {
        LoglinearPrint {
            none: false,
            ..next
        }
    };
    Ok(())
}

fn plot(p: &mut Parser, resid: &mut bool, normal: &mut bool) -> PResult<()> {
    p.eat(Punct::Eq);
    // PLOT without a keyword shows no plots
    while !at_end(p) {
        if p.eat_keyword("RESID") {
            *resid = true;
        } else if p.eat_keyword("NORMPROB") {
            *normal = true;
        } else if p.eat_keyword("DEFAULT") || p.eat_word("ALL") {
            (*resid, *normal) = (true, true);
        } else if p.eat_keyword("NONE") {
            (*resid, *normal) = (false, false);
        } else {
            return Err(p.unexpected(&["RESID", "NORMPROB", "DEFAULT", "ALL", "NONE"]));
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

/// The contrast after `CONTRAST(var)=`.
fn contrast_kind(p: &mut Parser) -> PResult<LoglinearContrastKind> {
    let basis = p.eat_keyword("BASIS");
    if p.at_keyword("SPECIAL") {
        p.pos += 1;
        let start = p.pos;
        let values = value_matrix(p, "contrast coefficient", "SPECIAL needs the matrix")?;
        let k = (values.len() as f64).sqrt().round() as usize;
        check(
            p,
            start,
            k >= 2 && k * k == values.len(),
            "SPECIAL takes as many values as the number of categories squared",
        )?;
        return Ok(LoglinearContrastKind::Special { values, basis });
    }
    if basis {
        return Err(p.unexpected(&["SPECIAL"]));
    }
    Ok(LoglinearContrastKind::Named(crate::glm::contrast(p, false)?))
}

pub(crate) fn loglinear(p: &mut Parser) -> PResult<Command> {
    let dependents = ranges(p, &["BY", "WITH"])?;
    let independents = if p.eat_word("BY") {
        ranges(p, &["WITH"])?
    } else {
        Vec::new()
    };
    let covariates = if p.eat_word("WITH") {
        p.var_list()?
    } else {
        Vec::new()
    };
    let mut current = LoglinearModel {
        terms: None,
        contrasts: Vec::new(),
        criteria: LoglinearCriteria::default(),
        cweight: None,
        gresid: Vec::new(),
        print: LoglinearPrint::default(),
        plot_resid: false,
        plot_normal: false,
    };
    let mut command = Loglinear {
        dependents,
        independents,
        covariates,
        models: Vec::new(),
        implicit_design: false,
        include: false,
    };
    let mut last_was_design = false;
    let mut missing_given = false;
    let mut cweight_variable = false;
    let mut designs = 0;
    while p.eat(Punct::Slash) {
        let start = p.pos;
        last_was_design = false;
        if p.eat_keyword("DESIGN") {
            p.eat(Punct::Eq);
            let terms = genlog_design(p)?;
            current.terms = (!terms.is_empty()).then_some(terms);
            command.models.push(current.clone());
            current.terms = None;
            designs += 1;
            last_was_design = true;
        } else if p.eat_keyword("CWEIGHT") {
            p.eat(Punct::Eq);
            if p.at(Punct::LParen) {
                if cweight_variable {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "CWEIGHT names a variable once and then cannot be given again",
                    ));
                }
                current.cweight = Some(CellWeights::Values(weight_matrix(p)?));
            } else {
                if current.cweight.is_some() {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "a CWEIGHT variable is allowed only as the one CWEIGHT subcommand",
                    ));
                }
                cweight_variable = true;
                current.cweight = Some(CellWeights::Variable(p.name("a variable name")?));
            }
        } else if p.eat_keyword("GRESID") {
            p.eat(Punct::Eq);
            if p.at(Punct::LParen) {
                current.gresid.push(CellWeights::Values(value_matrix(
                    p,
                    "coefficient",
                    "GRESID needs the coefficients",
                )?));
            } else {
                for item in p.var_list()? {
                    match item {
                        VarItem::Name(n) => current.gresid.push(CellWeights::Variable(n)),
                        _ => {
                            return Err(SyntaxError::new(
                                p.span_from(start),
                                "GRESID takes variable names or a matrix of coefficients",
                            ));
                        }
                    }
                }
            }
        } else if p.eat_keyword("CONTRAST") {
            p.expect(Punct::LParen)?;
            let variable = p.name("a factor")?;
            p.expect(Punct::RParen)?;
            p.eat(Punct::Eq);
            let kind = contrast_kind(p)?;
            current
                .contrasts
                .retain(|c| !c.variable.eq_ignore_ascii_case(&variable));
            current.contrasts.push(LoglinearContrast { variable, kind });
        } else if p.eat_keyword("CRITERIA") {
            criteria(p, &mut current.criteria)?;
        } else if p.eat_keyword("PRINT") {
            print(p, &mut current.print)?;
        } else if p.eat_keyword("PLOT") {
            plot(p, &mut current.plot_resid, &mut current.plot_normal)?;
        } else if p.eat_keyword("MISSING") {
            if missing_given {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "MISSING is given only once",
                ));
            }
            missing_given = true;
            command.include = missing(p)?;
        } else {
            return Err(p.subcommand_error("LOGLINEAR", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if !last_was_design {
        current.terms = None;
        command.models.push(current);
        command.implicit_design = designs > 0;
    }
    Ok(Command::Loglinear(Box::new(command)))
}
