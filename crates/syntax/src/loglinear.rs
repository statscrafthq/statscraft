//! Parsers of the loglinear procedures HILOGLINEAR and GENLOG.
//!
//! HILOGLINEAR (rules noted from the Command Syntax Reference):
//! - The variable list comes first: names (TO and ALL allowed), each group closed by its
//!   `(min,max)` integer range, which applies to every name of the group.
//! - DESIGN may repeat; each one fits a model with the PRINT, PLOT, CRITERIA, MAXORDER and
//!   CWEIGHT settings given before it, which stay until replaced. METHOD [=BACKWARD] applies
//!   to the next design only. When the last subcommand is not DESIGN, one more default design
//!   is added and the run warns. A DESIGN without terms is the default model.
//! - CRITERIA: CONVERGE(n), ITERATE(20), P(0.05), DELTA(0.5), MAXSTEPS(10), DEFAULT; at least
//!   one keyword. PRINT: FREQ, RESID, ESTIM, ASSOCIATION, DEFAULT, ALL, NONE. PLOT: RESID,
//!   NORMPROB (also NORMPLOT), DEFAULT, ALL, NONE; PLOT alone means DEFAULT. CWEIGHT: a
//!   variable (only once) or a parenthesized list of weights with `n*w` repeats. MISSING
//!   once: EXCLUDE (or DEFAULT) or INCLUDE.
//!
//! GENLOG:
//! - `factors [BY factors] [WITH covariates]`; without BY every factor is a dependent one.
//! - CSTRUCTURE=var, GRESID=varlist, GLOR=varlist, MODEL=POISSON|MULTINOMIAL, CRITERIA with
//!   CONVERGE(0.001), ITERATE(20) (ITERATION accepted too), DELTA(0.5), CIN(95), EPS(1E-8),
//!   DEFAULT; PRINT keywords with NONE overriding; PLOT=DEFAULT, RESID(ADJRESID DEV),
//!   NORMPROB(ADJRESID DEV), NONE; SAVE=RESID ZRESID ADJRESID DEV PRED, each with an optional
//!   `(name)`; MISSING; DESIGN with `a BY b`, `a*b` and `a(n)` for the n-th category alone.
//! - The last of each subcommand applies.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// One variable group of HILOGLINEAR with its range.
#[derive(Debug, Clone, PartialEq)]
pub struct HiloRange {
    pub variables: Vec<VarItem>,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HiloCriteria {
    /// None: the larger of 0.25 and a thousandth of the largest cell.
    pub converge: Option<f64>,
    pub iterate: u32,
    pub p: f64,
    pub delta: f64,
    pub maxsteps: u32,
}

impl Default for HiloCriteria {
    fn default() -> Self {
        Self {
            converge: None,
            iterate: 20,
            p: 0.05,
            delta: 0.5,
            maxsteps: 10,
        }
    }
}

/// What PRINT shows; None in `HiloDesign::print` is the default display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HiloPrint {
    pub freq: bool,
    pub resid: bool,
    pub estim: bool,
    pub association: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CellWeights {
    Variable(String),
    Values(Vec<f64>),
}

/// One DESIGN with the settings in force for it.
#[derive(Debug, Clone, PartialEq)]
pub struct HiloDesign {
    /// The generating class; None for the default model.
    pub terms: Option<Vec<Vec<String>>>,
    pub backward: bool,
    pub maxorder: Option<usize>,
    pub criteria: HiloCriteria,
    pub cweight: Option<CellWeights>,
    /// None: the default display.
    pub print: Option<HiloPrint>,
    /// Residual plots and normal plots.
    pub plot_resid: bool,
    pub plot_normal: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hiloglinear {
    pub ranges: Vec<HiloRange>,
    pub designs: Vec<HiloDesign>,
    /// The last design was added because the command did not end with DESIGN.
    pub implicit_design: bool,
    pub include: bool,
}

fn value_in_parens(p: &mut Parser) -> PResult<f64> {
    p.expect(Punct::LParen)?;
    let Some(n) = p.number() else {
        return Err(p.unexpected(&["a number"]));
    };
    p.expect(Punct::RParen)?;
    Ok(n)
}

fn check(p: &Parser, start: usize, ok: bool, what: &str) -> PResult<()> {
    if ok {
        Ok(())
    } else {
        Err(SyntaxError::new(p.span_from(start), what))
    }
}

fn missing(p: &mut Parser) -> PResult<bool> {
    p.eat(Punct::Eq);
    if p.eat_keyword("INCLUDE") {
        Ok(true)
    } else if p.eat_keyword("EXCLUDE") || p.eat_keyword("DEFAULT") {
        Ok(false)
    } else {
        Err(p.unexpected(&["EXCLUDE", "INCLUDE"]))
    }
}

fn hilo_criteria(p: &mut Parser, c: &mut HiloCriteria) -> PResult<()> {
    p.eat(Punct::Eq);
    if at_end(p) {
        return Err(SyntaxError::new(
            p.span(),
            "HILOGLINEAR CRITERIA needs at least one keyword",
        ));
    }
    const KEYWORDS: [&str; 6] = ["CONVERGE", "ITERATE", "P", "DELTA", "MAXSTEPS", "DEFAULT"];
    while !at_end(p) {
        let start = p.pos;
        // P is a one-letter keyword, so it must be written in full
        let i = if p.eat_word("P") {
            2
        } else if let Some(i) = KEYWORDS.iter().position(|k| *k != "P" && p.at_keyword(k)) {
            p.pos += 1;
            i
        } else {
            return Err(p.unexpected(&KEYWORDS));
        };
        if i == 5 {
            *c = HiloCriteria::default();
            p.eat(Punct::Comma);
            continue;
        }
        let v = value_in_parens(p)?;
        match i {
            0 => {
                check(p, start, v > 0.0, "CONVERGE must be above 0")?;
                c.converge = Some(v);
            }
            1 => {
                check(
                    p,
                    start,
                    v >= 1.0 && v.fract() == 0.0,
                    "ITERATE takes a positive whole number",
                )?;
                c.iterate = v as u32;
            }
            2 => {
                check(p, start, v > 0.0 && v < 1.0, "P is above 0 and below 1")?;
                c.p = v;
            }
            3 => {
                check(p, start, (0.0..=1.0).contains(&v), "DELTA is from 0 to 1")?;
                c.delta = v;
            }
            _ => {
                check(
                    p,
                    start,
                    (1.0..=99.0).contains(&v) && v.fract() == 0.0,
                    "MAXSTEPS is a whole number from 1 to 99",
                )?;
                c.maxsteps = v as u32;
            }
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn hilo_print(p: &mut Parser) -> PResult<Option<HiloPrint>> {
    p.eat(Punct::Eq);
    if at_end(p) {
        return Ok(None);
    }
    let mut out = HiloPrint::default();
    let (mut none, mut default) = (false, false);
    while !at_end(p) {
        if p.eat_keyword("FREQ") {
            out.freq = true;
        } else if p.eat_keyword("RESID") {
            out.resid = true;
        } else if p.eat_keyword("ESTIM") {
            out.estim = true;
        } else if p.eat_keyword("ASSOCIATION") {
            out.association = true;
        } else if p.eat_keyword("DEFAULT") {
            default = true;
        } else if p.eat_word("ALL") {
            out = HiloPrint {
                freq: true,
                resid: true,
                estim: true,
                association: true,
            };
        } else if p.eat_keyword("NONE") {
            none = true;
        } else {
            return Err(p.unexpected(&[
                "FREQ",
                "RESID",
                "ESTIM",
                "ASSOCIATION",
                "DEFAULT",
                "ALL",
                "NONE",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(if none {
        Some(HiloPrint::default())
    } else if default && out == HiloPrint::default() {
        None
    } else {
        Some(out)
    })
}

/// `(w w n*w ...)`
fn weight_matrix(p: &mut Parser) -> PResult<Vec<f64>> {
    p.expect(Punct::LParen)?;
    let mut out = Vec::new();
    while !p.eat(Punct::RParen) {
        let start = p.pos;
        let Some(first) = p.number() else {
            return Err(p.unexpected(&["a cell weight", "')'"]));
        };
        if p.eat(Punct::Star) {
            check(
                p,
                start,
                first >= 1.0 && first.fract() == 0.0,
                "the repeat count before * is a positive whole number",
            )?;
            let Some(w) = p.number() else {
                return Err(p.unexpected(&["a cell weight"]));
            };
            out.extend(std::iter::repeat_n(w, first as usize));
        } else {
            out.push(first);
        }
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(SyntaxError::new(p.span(), "CWEIGHT needs the cell weights"));
    }
    Ok(out)
}

/// Terms joined by `*` (or BY), separated by commas or spaces.
fn hilo_terms(p: &mut Parser) -> PResult<Vec<Vec<String>>> {
    let mut terms = Vec::new();
    while !at_end(p) {
        let mut term = vec![p.name("a variable name")?];
        while p.eat(Punct::Star) || p.eat_word("BY") {
            term.push(p.name("a variable name")?);
        }
        terms.push(term);
        p.eat(Punct::Comma);
    }
    Ok(terms)
}

pub(crate) fn hiloglinear(p: &mut Parser) -> PResult<Command> {
    let mut ranges = Vec::new();
    while matches!(p.peek(), Some(TokenKind::Word(_))) {
        let variables = list_until(p, &[])?;
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
        ranges.push(HiloRange {
            variables,
            min,
            max,
        });
        p.eat(Punct::Comma);
    }
    if ranges.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    let mut current = HiloDesign {
        terms: None,
        backward: false,
        maxorder: None,
        criteria: HiloCriteria::default(),
        cweight: None,
        print: None,
        plot_resid: false,
        plot_normal: false,
    };
    let mut command = Hiloglinear {
        ranges,
        designs: Vec::new(),
        implicit_design: false,
        include: false,
    };
    let mut last_was_design = false;
    let mut missing_given = false;
    let mut cweight_variable = false;
    while p.eat(Punct::Slash) {
        let start = p.pos;
        last_was_design = false;
        if p.eat_keyword("DESIGN") {
            p.eat(Punct::Eq);
            let terms = hilo_terms(p)?;
            current.terms = (!terms.is_empty()).then_some(terms);
            command.designs.push(current.clone());
            current.backward = false;
            last_was_design = true;
        } else if p.eat_keyword("METHOD") {
            let equals = p.eat(Punct::Eq);
            if !p.eat_keyword("BACKWARD") && equals {
                return Err(p.unexpected(&["BACKWARD"]));
            }
            current.backward = true;
        } else if p.eat_keyword("MAXORDER") {
            p.eat(Punct::Eq);
            let at = p.pos;
            let Some(k) = p.number() else {
                return Err(p.unexpected(&["the highest order"]));
            };
            check(
                p,
                at,
                k >= 1.0 && k.fract() == 0.0,
                "MAXORDER takes a positive whole number",
            )?;
            current.maxorder = Some(k as usize);
        } else if p.eat_keyword("CRITERIA") {
            hilo_criteria(p, &mut current.criteria)?;
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
        } else if p.eat_keyword("PRINT") {
            current.print = hilo_print(p)?;
        } else if p.eat_keyword("PLOT") {
            p.eat(Punct::Eq);
            let (mut resid, mut normal) = (false, false);
            if at_end(p) {
                (resid, normal) = (true, true);
            }
            while !at_end(p) {
                if p.eat_keyword("RESID") {
                    resid = true;
                } else if p.eat_keyword("NORMPROB") || p.eat_keyword("NORMPLOT") {
                    normal = true;
                } else if p.eat_keyword("DEFAULT") || p.eat_word("ALL") {
                    (resid, normal) = (true, true);
                } else if p.eat_keyword("NONE") {
                    (resid, normal) = (false, false);
                } else {
                    return Err(p.unexpected(&["RESID", "NORMPROB", "DEFAULT", "ALL", "NONE"]));
                }
                p.eat(Punct::Comma);
            }
            (current.plot_resid, current.plot_normal) = (resid, normal);
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
            return Err(p.subcommand_error("HILOGLINEAR", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if !last_was_design {
        current.terms = None;
        command.designs.push(current);
        command.implicit_design = true;
    }
    Ok(Command::Hiloglinear(Box::new(command)))
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GenlogCriteria {
    pub converge: f64,
    pub iterate: u32,
    pub delta: f64,
    pub cin: f64,
    pub eps: f64,
}

impl Default for GenlogCriteria {
    fn default() -> Self {
        Self {
            converge: 0.001,
            iterate: 20,
            delta: 0.5,
            cin: 95.0,
            eps: 1e-8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenlogPrint {
    pub freq: bool,
    pub resid: bool,
    pub zresid: bool,
    pub adjresid: bool,
    pub dev: bool,
    pub iteration: bool,
    pub cov: bool,
    pub design: bool,
    pub estim: bool,
    pub corr: bool,
}

impl Default for GenlogPrint {
    fn default() -> Self {
        Self {
            freq: true,
            resid: true,
            zresid: false,
            adjresid: true,
            dev: true,
            iteration: false,
            cov: false,
            design: false,
            estim: false,
            corr: false,
        }
    }
}

/// The residuals a GENLOG plot shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GenlogResiduals {
    pub adjusted: bool,
    pub deviance: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GenlogPlot {
    /// Residuals against the observed and expected counts.
    pub resid: Option<GenlogResiduals>,
    /// Normal and detrended normal plots.
    pub normprob: Option<GenlogResiduals>,
}

impl Default for GenlogPlot {
    fn default() -> Self {
        let adjusted = Some(GenlogResiduals {
            adjusted: true,
            deviance: false,
        });
        Self {
            resid: adjusted,
            normprob: adjusted,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenlogSaveKind {
    Resid,
    Zresid,
    Adjresid,
    Dev,
    Pred,
}

/// One factor of a DESIGN term, with the category `(n)` when only that one is meant, or a
/// covariate.
#[derive(Debug, Clone, PartialEq)]
pub struct GenlogPart {
    pub name: String,
    pub category: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Genlog {
    pub dependents: Vec<VarItem>,
    /// The factors after BY: a logit model.
    pub independents: Vec<VarItem>,
    pub covariates: Vec<VarItem>,
    pub cstructure: Option<String>,
    pub gresid: Vec<VarItem>,
    pub glor: Vec<VarItem>,
    pub multinomial: bool,
    pub criteria: GenlogCriteria,
    pub print: GenlogPrint,
    pub plot: GenlogPlot,
    pub save: Vec<(GenlogSaveKind, Option<String>)>,
    pub include: bool,
    /// None: the saturated model.
    pub design: Option<Vec<Vec<GenlogPart>>>,
}

fn genlog_criteria(p: &mut Parser, c: &mut GenlogCriteria) -> PResult<()> {
    p.eat(Punct::Eq);
    const KEYWORDS: [&str; 6] = ["CONVERGE", "ITERATE", "DELTA", "CIN", "EPS", "DEFAULT"];
    while !at_end(p) {
        let start = p.pos;
        let i = if p.eat_keyword("ITERATION") {
            1
        } else if let Some(i) = KEYWORDS.iter().position(|k| p.at_keyword(k)) {
            p.pos += 1;
            i
        } else {
            return Err(p.unexpected(&KEYWORDS));
        };
        if i == 5 {
            *c = GenlogCriteria::default();
            p.eat(Punct::Comma);
            continue;
        }
        let v = value_in_parens(p)?;
        match i {
            0 => {
                check(p, start, v > 0.0, "CONVERGE must be above 0")?;
                c.converge = v;
            }
            1 => {
                check(
                    p,
                    start,
                    v >= 1.0 && v.fract() == 0.0,
                    "ITERATE takes a positive whole number",
                )?;
                c.iterate = v as u32;
            }
            2 => {
                check(p, start, v >= 0.0, "DELTA cannot be negative")?;
                c.delta = v;
            }
            3 => {
                check(
                    p,
                    start,
                    (50.0..=99.99).contains(&v),
                    "CIN is from 50 to 99.99",
                )?;
                c.cin = v;
            }
            _ => {
                check(p, start, v > 0.0, "EPS must be above 0")?;
                c.eps = v;
            }
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

fn genlog_print(p: &mut Parser) -> PResult<GenlogPrint> {
    p.eat(Punct::Eq);
    if at_end(p) {
        return Ok(GenlogPrint::default());
    }
    let off = GenlogPrint {
        freq: false,
        resid: false,
        adjresid: false,
        dev: false,
        ..GenlogPrint::default()
    };
    let mut out = off;
    let mut none = false;
    while !at_end(p) {
        if p.eat_keyword("FREQ") {
            out.freq = true;
        } else if p.eat_keyword("RESID") {
            out.resid = true;
        } else if p.eat_keyword("ZRESID") {
            out.zresid = true;
        } else if p.eat_keyword("ADJRESID") {
            out.adjresid = true;
        } else if p.eat_keyword("DEV") {
            out.dev = true;
        } else if p.eat_keyword("ITERATION") {
            out.iteration = true;
        } else if p.eat_keyword("COV") {
            out.cov = true;
        } else if p.eat_keyword("DESIGN") {
            out.design = true;
        } else if p.eat_keyword("ESTIM") {
            out.estim = true;
        } else if p.eat_keyword("CORR") {
            out.corr = true;
        } else if p.eat_word("ALL") {
            out = GenlogPrint {
                freq: true,
                resid: true,
                zresid: true,
                adjresid: true,
                dev: true,
                iteration: true,
                cov: true,
                design: true,
                estim: true,
                corr: true,
            };
        } else if p.eat_keyword("DEFAULT") {
            out.freq = true;
            out.resid = true;
            out.adjresid = true;
            out.dev = true;
        } else if p.eat_keyword("NONE") {
            none = true;
        } else {
            return Err(p.unexpected(&[
                "FREQ",
                "RESID",
                "ZRESID",
                "ADJRESID",
                "DEV",
                "ITERATION",
                "COV",
                "DESIGN",
                "ESTIM",
                "CORR",
                "ALL",
                "DEFAULT",
                "NONE",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(if none { off } else { out })
}

/// `(ADJRESID DEV)` after RESID or NORMPROB; ADJRESID when absent or empty.
fn residual_kinds(p: &mut Parser) -> PResult<GenlogResiduals> {
    let mut out = GenlogResiduals::default();
    if p.eat(Punct::LParen) {
        while !p.eat(Punct::RParen) {
            if p.eat_keyword("ADJRESID") {
                out.adjusted = true;
            } else if p.eat_keyword("DEV") {
                out.deviance = true;
            } else {
                return Err(p.unexpected(&["ADJRESID", "DEV", "')'"]));
            }
            p.eat(Punct::Comma);
        }
    }
    if !out.adjusted && !out.deviance {
        out.adjusted = true;
    }
    Ok(out)
}

fn genlog_plot(p: &mut Parser) -> PResult<GenlogPlot> {
    p.eat(Punct::Eq);
    let mut out = GenlogPlot {
        resid: None,
        normprob: None,
    };
    if at_end(p) {
        return Ok(GenlogPlot::default());
    }
    while !at_end(p) {
        if p.eat_keyword("DEFAULT") {
            out = GenlogPlot::default();
        } else if p.eat_keyword("RESID") {
            out.resid = Some(residual_kinds(p)?);
        } else if p.eat_keyword("NORMPROB") {
            out.normprob = Some(residual_kinds(p)?);
        } else if p.eat_keyword("NONE") {
            out = GenlogPlot {
                resid: None,
                normprob: None,
            };
        } else {
            return Err(p.unexpected(&["DEFAULT", "RESID", "NORMPROB", "NONE"]));
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn genlog_save(p: &mut Parser) -> PResult<Vec<(GenlogSaveKind, Option<String>)>> {
    p.eat(Punct::Eq);
    const KINDS: [(&str, GenlogSaveKind); 5] = [
        ("RESID", GenlogSaveKind::Resid),
        ("ZRESID", GenlogSaveKind::Zresid),
        ("ADJRESID", GenlogSaveKind::Adjresid),
        ("DEV", GenlogSaveKind::Dev),
        ("PRED", GenlogSaveKind::Pred),
    ];
    let mut out: Vec<(GenlogSaveKind, Option<String>)> = Vec::new();
    while !at_end(p) {
        let start = p.pos;
        let Some(&(_, kind)) = KINDS.iter().find(|(k, _)| p.at_keyword(k)) else {
            return Err(p.unexpected(&["RESID", "ZRESID", "ADJRESID", "DEV", "PRED"]));
        };
        p.pos += 1;
        let name = if p.eat(Punct::LParen) {
            let name = p.name("a variable name")?;
            p.expect(Punct::RParen)?;
            check(
                p,
                start,
                !name.starts_with('#') && !name.starts_with('$'),
                "a saved variable cannot be a scratch or system variable",
            )?;
            Some(name)
        } else {
            None
        };
        check(
            p,
            start,
            !out.iter().any(|(k, _)| *k == kind),
            "each SAVE keyword is given only once",
        )?;
        out.push((kind, name));
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn genlog_design(p: &mut Parser) -> PResult<Vec<Vec<GenlogPart>>> {
    let part = |p: &mut Parser| -> PResult<GenlogPart> {
        let name = p.name("a factor or covariate")?;
        let mut category = None;
        if p.eat(Punct::LParen) {
            let at = p.pos;
            let Some(n) = p.number() else {
                return Err(p.unexpected(&["a category number"]));
            };
            check(
                p,
                at,
                n >= 1.0 && n.fract() == 0.0,
                "the category in parentheses is a positive whole number",
            )?;
            p.expect(Punct::RParen)?;
            category = Some(n as u32);
        }
        Ok(GenlogPart { name, category })
    };
    let mut terms = Vec::new();
    while !at_end(p) {
        let mut term = vec![part(p)?];
        while p.eat(Punct::Star) || p.eat_word("BY") {
            term.push(part(p)?);
        }
        terms.push(term);
        p.eat(Punct::Comma);
    }
    Ok(terms)
}

pub(crate) fn genlog(p: &mut Parser) -> PResult<Command> {
    let dependents = list_until(p, &["BY", "WITH"])?;
    if dependents.is_empty() {
        return Err(p.unexpected(&["a factor"]));
    }
    let mut command = Genlog {
        dependents,
        independents: Vec::new(),
        covariates: Vec::new(),
        cstructure: None,
        gresid: Vec::new(),
        glor: Vec::new(),
        multinomial: false,
        criteria: GenlogCriteria::default(),
        print: GenlogPrint::default(),
        plot: GenlogPlot::default(),
        save: Vec::new(),
        include: false,
        design: None,
    };
    if p.eat_word("BY") {
        command.independents = list_until(p, &["WITH"])?;
        if command.independents.is_empty() {
            return Err(p.unexpected(&["a factor"]));
        }
    }
    if p.eat_word("WITH") {
        command.covariates = p.var_list()?;
    }
    while p.eat(Punct::Slash) {
        if p.eat_keyword("CSTRUCTURE") {
            p.eat(Punct::Eq);
            command.cstructure = Some(p.name("a variable name")?);
        } else if p.eat_keyword("GRESID") {
            p.eat(Punct::Eq);
            command.gresid = p.var_list()?;
        } else if p.eat_keyword("GLOR") {
            p.eat(Punct::Eq);
            command.glor = p.var_list()?;
        } else if p.eat_keyword("MODEL") {
            p.eat(Punct::Eq);
            command.multinomial = if p.eat_keyword("MULTINOMIAL") {
                true
            } else if p.eat_keyword("POISSON") {
                false
            } else {
                return Err(p.unexpected(&["POISSON", "MULTINOMIAL"]));
            };
        } else if p.eat_keyword("CRITERIA") {
            genlog_criteria(p, &mut command.criteria)?;
        } else if p.eat_keyword("PRINT") {
            command.print = genlog_print(p)?;
        } else if p.eat_keyword("PLOT") {
            command.plot = genlog_plot(p)?;
        } else if p.eat_keyword("SAVE") {
            command.save = genlog_save(p)?;
        } else if p.eat_keyword("MISSING") {
            command.include = missing(p)?;
        } else if p.eat_keyword("DESIGN") {
            p.eat(Punct::Eq);
            let terms = genlog_design(p)?;
            command.design = (!terms.is_empty()).then_some(terms);
        } else {
            return Err(p.subcommand_error("GENLOG", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    if !command.independents.is_empty() && !command.multinomial {
        return Err(SyntaxError::new(
            p.span(),
            "a logit model (factors after BY) needs /MODEL=MULTINOMIAL",
        ));
    }
    Ok(Command::Genlog(Box::new(command)))
}
