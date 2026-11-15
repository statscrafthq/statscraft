//! Parser of MULTIPLE IMPUTATION.
//!
//! Rules from the Command Syntax Reference, in our words:
//! - The variable list comes first and needs two or more variables (checked once TO and ALL
//!   are expanded). A variable named again moves to the place of its last mention.
//! - IMPUTE takes METHOD=AUTO | FCS | MONOTONE | NONE, NIMPUTATIONS=n (5), SCALEMODEL=LINEAR |
//!   PMM[(k)] (k 5), INTERACTIONS=NONE | TWOWAY, MAXPCTMISSING=NONE | p (0 < p < 100),
//!   MAXCASEDRAWS=n (50), MAXPARAMDRAWS=n (2), MAXITER=n (10), SINGULAR=x (1E-12) and
//!   MAXMODELPARAM=n (100).
//! - CONSTRAINTS varlist (MIN=NONE | x  MAX=NONE | x  RND=NO | x  ROLE=BOTH | DEP | IND) may be
//!   repeated; later settings of a variable win, keyword by keyword. A date variable's limit
//!   is written as a quoted date.
//! - MISSINGSUMMARIES OVERALL, VARIABLES[(MAXVARS=25 MINPCTMISSING=10)], PATTERNS or NONE.
//!   IMPUTATIONSUMMARIES MODELS, DESCRIPTIVES[(varlist)] or NONE. NONE stands alone.
//! - ANALYSISWEIGHT=var. OUTFILE IMPUTATIONS=file | dataset, FCSITERATIONS=file | dataset;
//!   IMPUTATIONS is needed unless METHOD=NONE.
//! - Every subcommand and keyword is written in full, every equals sign is required, a
//!   subcommand other than CONSTRAINTS comes at most once, a keyword at most once within its
//!   subcommand, and no subcommand is empty.

use crate::ast::{Command, Value, VarItem};
use crate::error::SyntaxError;
use crate::lexer::Punct;
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImputeMethod {
    Auto,
    Fcs,
    Monotone,
    None,
}

/// ROLE of a CONSTRAINTS variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImputeRole {
    /// Imputed and used as a predictor.
    Both,
    /// Imputed only.
    Dependent,
    /// A predictor only: imputed for the models but not saved.
    Predictor,
}

/// One CONSTRAINTS group. Each field is None when not given; `Some(None)` is NONE or NO.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImputeConstraint {
    pub variables: Vec<VarItem>,
    pub min: Option<Option<Value>>,
    pub max: Option<Option<Value>>,
    pub rnd: Option<Option<f64>>,
    pub role: Option<ImputeRole>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MultipleImputation {
    pub variables: Vec<VarItem>,
    pub method: ImputeMethod,
    pub nimputations: usize,
    /// SCALEMODEL=PMM(k); None is LINEAR.
    pub pmm: Option<usize>,
    pub twoway: bool,
    pub maxpctmissing: Option<f64>,
    pub maxcasedraws: usize,
    pub maxparamdraws: usize,
    pub maxiter: usize,
    pub singular: f64,
    pub maxmodelparam: usize,
    /// The IMPUTE keywords written, upper case, for the warnings of AUTO.
    pub given: Vec<&'static str>,
    pub constraints: Vec<ImputeConstraint>,
    pub overall: bool,
    /// MISSINGSUMMARIES VARIABLES: MAXVARS and MINPCTMISSING.
    pub variable_summary: Option<(usize, f64)>,
    pub patterns: bool,
    pub models: bool,
    /// IMPUTATIONSUMMARIES DESCRIPTIVES: empty for every imputed variable.
    pub descriptives: Option<Vec<VarItem>>,
    pub analysis_weight: Option<String>,
    pub imputations: Option<String>,
    pub fcs_iterations: Option<String>,
}

fn equals(p: &mut Parser) -> PResult<()> {
    p.expect(Punct::Eq)
}

fn whole(p: &mut Parser, what: &str) -> PResult<usize> {
    let start = p.pos;
    match p.number() {
        Some(n) if n >= 1.0 && n.fract() == 0.0 => Ok(n as usize),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} takes a positive whole number, got {n}"),
        )),
        None => Err(p.unexpected(&["a positive whole number"])),
    }
}

fn number(p: &mut Parser, what: &str, ok: fn(f64) -> bool, range: &str) -> PResult<f64> {
    let start = p.pos;
    match p.number() {
        Some(n) if ok(n) => Ok(n),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} takes {range}, got {n}"),
        )),
        None => Err(p.unexpected(&["a number"])),
    }
}

/// Eats `word` when it is next, failing when `seen` already holds it.
fn keyword(p: &mut Parser, seen: &mut Vec<&'static str>, word: &'static str) -> PResult<bool> {
    let start = p.pos;
    if !p.eat_word(word) {
        return Ok(false);
    }
    if seen.contains(&word) {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("{word} is given twice in one subcommand"),
        ));
    }
    seen.push(word);
    Ok(true)
}

fn impute(p: &mut Parser, c: &mut MultipleImputation) -> PResult<()> {
    let mut seen = Vec::new();
    while !at_end(p) {
        if keyword(p, &mut seen, "METHOD")? {
            equals(p)?;
            c.method = if p.eat_word("AUTO") {
                ImputeMethod::Auto
            } else if p.eat_word("FCS") {
                ImputeMethod::Fcs
            } else if p.eat_word("MONOTONE") {
                ImputeMethod::Monotone
            } else if p.eat_word("NONE") {
                ImputeMethod::None
            } else {
                return Err(p.unexpected(&["AUTO", "FCS", "MONOTONE", "NONE"]));
            };
        } else if keyword(p, &mut seen, "NIMPUTATIONS")? {
            equals(p)?;
            c.nimputations = whole(p, "NIMPUTATIONS")?;
        } else if keyword(p, &mut seen, "SCALEMODEL")? {
            equals(p)?;
            if p.eat_word("LINEAR") {
                c.pmm = None;
            } else if p.eat_word("PMM") {
                let mut k = 5;
                if p.eat(Punct::LParen) {
                    k = whole(p, "PMM")?;
                    p.expect(Punct::RParen)?;
                }
                c.pmm = Some(k);
            } else {
                return Err(p.unexpected(&["LINEAR", "PMM"]));
            }
        } else if keyword(p, &mut seen, "INTERACTIONS")? {
            equals(p)?;
            c.twoway = if p.eat_word("TWOWAY") {
                true
            } else if p.eat_word("NONE") {
                false
            } else {
                return Err(p.unexpected(&["NONE", "TWOWAY"]));
            };
        } else if keyword(p, &mut seen, "MAXPCTMISSING")? {
            equals(p)?;
            c.maxpctmissing = match p.eat_word("NONE") {
                true => None,
                false => Some(number(
                    p,
                    "MAXPCTMISSING",
                    |n| n > 0.0 && n < 100.0,
                    "a percentage above 0 and below 100",
                )?),
            };
        } else if keyword(p, &mut seen, "MAXCASEDRAWS")? {
            equals(p)?;
            c.maxcasedraws = whole(p, "MAXCASEDRAWS")?;
        } else if keyword(p, &mut seen, "MAXPARAMDRAWS")? {
            equals(p)?;
            c.maxparamdraws = whole(p, "MAXPARAMDRAWS")?;
        } else if keyword(p, &mut seen, "MAXITER")? {
            equals(p)?;
            c.maxiter = whole(p, "MAXITER")?;
        } else if keyword(p, &mut seen, "SINGULAR")? {
            equals(p)?;
            c.singular = number(p, "SINGULAR", |n| n > 0.0, "a positive number")?;
        } else if keyword(p, &mut seen, "MAXMODELPARAM")? {
            equals(p)?;
            c.maxmodelparam = whole(p, "MAXMODELPARAM")?;
        } else {
            return Err(p.unexpected(&[
                "METHOD",
                "NIMPUTATIONS",
                "SCALEMODEL",
                "INTERACTIONS",
                "MAXPCTMISSING",
                "MAXCASEDRAWS",
                "MAXPARAMDRAWS",
                "MAXITER",
                "SINGULAR",
                "MAXMODELPARAM",
            ]));
        }
    }
    c.given = seen;
    Ok(())
}

/// A MIN or MAX limit: NONE, a number or a quoted date.
fn limit(p: &mut Parser) -> PResult<Option<Value>> {
    if p.eat_word("NONE") {
        return Ok(None);
    }
    match p.value() {
        Some(value) => Ok(Some(value)),
        None => Err(p.unexpected(&["NONE", "a number", "a quoted date"])),
    }
}

fn constraint(p: &mut Parser) -> PResult<ImputeConstraint> {
    let mut c = ImputeConstraint {
        variables: p.var_list()?,
        ..ImputeConstraint::default()
    };
    p.expect(Punct::LParen)?;
    let mut seen = Vec::new();
    let open = p.pos;
    while !p.eat(Punct::RParen) {
        if keyword(p, &mut seen, "MIN")? {
            equals(p)?;
            c.min = Some(limit(p)?);
        } else if keyword(p, &mut seen, "MAX")? {
            equals(p)?;
            c.max = Some(limit(p)?);
        } else if keyword(p, &mut seen, "RND")? {
            equals(p)?;
            c.rnd = Some(match p.eat_word("NO") {
                true => None,
                false => Some(number(p, "RND", |n| n > 0.0, "a positive number")?),
            });
        } else if keyword(p, &mut seen, "ROLE")? {
            equals(p)?;
            c.role = Some(if p.eat_word("BOTH") {
                ImputeRole::Both
            } else if p.eat_word("DEP") {
                ImputeRole::Dependent
            } else if p.eat_word("IND") {
                ImputeRole::Predictor
            } else {
                return Err(p.unexpected(&["BOTH", "DEP", "IND"]));
            });
        } else {
            return Err(p.unexpected(&["MIN", "MAX", "RND", "ROLE", "')'"]));
        }
    }
    if seen.is_empty() {
        return Err(SyntaxError::new(
            p.span_from(open),
            "CONSTRAINTS needs MIN, MAX, RND or ROLE in the parentheses",
        ));
    }
    if let (Some(Some(Value::Number(min))), Some(Some(Value::Number(max)))) = (&c.min, &c.max)
        && min >= max
    {
        return Err(SyntaxError::new(
            p.span_from(open),
            format!("MIN ({min}) must be below MAX ({max})"),
        ));
    }
    Ok(c)
}

fn missing_summaries(p: &mut Parser, c: &mut MultipleImputation) -> PResult<()> {
    let start = p.pos;
    let mut seen = Vec::new();
    c.overall = false;
    while !at_end(p) {
        if keyword(p, &mut seen, "OVERALL")? {
            c.overall = true;
        } else if keyword(p, &mut seen, "VARIABLES")? {
            let (mut maxvars, mut minpct) = (25, 10.0);
            if p.eat(Punct::LParen) {
                let mut inner = Vec::new();
                while !p.eat(Punct::RParen) {
                    if keyword(p, &mut inner, "MAXVARS")? {
                        equals(p)?;
                        maxvars = whole(p, "MAXVARS")?;
                    } else if keyword(p, &mut inner, "MINPCTMISSING")? {
                        equals(p)?;
                        minpct = number(
                            p,
                            "MINPCTMISSING",
                            |n| (0.0..100.0).contains(&n),
                            "a percentage from 0 to below 100",
                        )?;
                    } else {
                        return Err(p.unexpected(&["MAXVARS", "MINPCTMISSING", "')'"]));
                    }
                }
            }
            c.variable_summary = Some((maxvars, minpct));
        } else if keyword(p, &mut seen, "PATTERNS")? {
            c.patterns = true;
        } else if !keyword(p, &mut seen, "NONE")? {
            return Err(p.unexpected(&["OVERALL", "VARIABLES", "PATTERNS", "NONE"]));
        }
    }
    alone(p, start, &seen, "MISSINGSUMMARIES")
}

/// NONE with another keyword, or nothing at all, is an error.
fn alone(p: &Parser, start: usize, seen: &[&str], subcommand: &str) -> PResult<()> {
    if seen.is_empty() {
        return Err(p.unexpected(&["a keyword"]));
    }
    if seen.contains(&"NONE") && seen.len() > 1 {
        return Err(SyntaxError::new(
            p.span_from(start),
            format!("NONE cannot stand with another keyword on {subcommand}"),
        ));
    }
    Ok(())
}

fn imputation_summaries(p: &mut Parser, c: &mut MultipleImputation) -> PResult<()> {
    let start = p.pos;
    let mut seen = Vec::new();
    c.models = false;
    while !at_end(p) {
        if keyword(p, &mut seen, "MODELS")? {
            c.models = true;
        } else if keyword(p, &mut seen, "DESCRIPTIVES")? {
            let mut list = Vec::new();
            if p.eat(Punct::LParen) {
                list = p.var_list()?;
                p.expect(Punct::RParen)?;
            }
            c.descriptives = Some(list);
        } else if !keyword(p, &mut seen, "NONE")? {
            return Err(p.unexpected(&["MODELS", "DESCRIPTIVES", "NONE"]));
        }
    }
    alone(p, start, &seen, "IMPUTATIONSUMMARIES")
}

fn outfile(p: &mut Parser, c: &mut MultipleImputation) -> PResult<()> {
    let mut seen = Vec::new();
    while !at_end(p) {
        if keyword(p, &mut seen, "IMPUTATIONS")? {
            equals(p)?;
            c.imputations = Some(p.file("a file or dataset name")?);
        } else if keyword(p, &mut seen, "FCSITERATIONS")? {
            equals(p)?;
            c.fcs_iterations = Some(p.file("a file or dataset name")?);
        } else {
            return Err(p.unexpected(&["IMPUTATIONS", "FCSITERATIONS"]));
        }
    }
    if seen.is_empty() {
        return Err(p.unexpected(&["IMPUTATIONS", "FCSITERATIONS"]));
    }
    Ok(())
}

pub(crate) fn multiple_imputation(p: &mut Parser) -> PResult<Command> {
    let mut c = MultipleImputation {
        variables: p.var_list()?,
        method: ImputeMethod::Auto,
        nimputations: 5,
        pmm: None,
        twoway: false,
        maxpctmissing: None,
        maxcasedraws: 50,
        maxparamdraws: 2,
        maxiter: 10,
        singular: 1e-12,
        maxmodelparam: 100,
        given: Vec::new(),
        constraints: Vec::new(),
        overall: true,
        variable_summary: None,
        patterns: false,
        models: true,
        descriptives: None,
        analysis_weight: None,
        imputations: None,
        fcs_iterations: None,
    };
    let mut seen = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        if p.eat_word("CONSTRAINTS") {
            c.constraints.push(constraint(p)?);
        } else if keyword(p, &mut seen, "IMPUTE")? {
            if at_end(p) {
                return Err(p.unexpected(&["METHOD", "NIMPUTATIONS", "SCALEMODEL"]));
            }
            impute(p, &mut c)?;
        } else if keyword(p, &mut seen, "MISSINGSUMMARIES")? {
            missing_summaries(p, &mut c)?;
        } else if keyword(p, &mut seen, "IMPUTATIONSUMMARIES")? {
            imputation_summaries(p, &mut c)?;
        } else if keyword(p, &mut seen, "ANALYSISWEIGHT")? {
            p.eat(Punct::Eq);
            c.analysis_weight = Some(p.name("a variable name")?);
        } else if keyword(p, &mut seen, "OUTFILE")? {
            outfile(p, &mut c)?;
        } else {
            p.pos = start;
            return Err(p.subcommand_error("MULTIPLE IMPUTATION", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if c.method != ImputeMethod::None && c.imputations.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "MULTIPLE IMPUTATION needs /OUTFILE IMPUTATIONS= for the imputed data, \
             unless /IMPUTE METHOD=NONE",
        ));
    }
    Ok(Command::MultipleImputation(Box::new(c)))
}
