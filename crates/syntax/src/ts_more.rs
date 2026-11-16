//! Parsers of SPECTRA (periodogram and spectral density), AREG (regression with first-order
//! autoregressive errors) and 2SLS (two-stage least squares).
//!
//! Rules noted from the Command Syntax Reference:
//! - SPECTRA: `VARIABLES= series` once; CENTER [NO], CROSS [NO], WINDOW= HAMMING, TUKEY,
//!   PARZEN, BARTLETT, UNIT (DANIELL) with an optional (span), NONE, or a list of weights
//!   (of several window keywords the first counts); PLOT= P S CS QS PH A G K ALL NONE with
//!   BY FREQ or BY PERIOD; SAVE= keyword (rootname) pairs for FREQ PER SIN COS P S RC IC CS
//!   QS PH A G K; APPLY [='name']. Other subcommands may repeat and the last counts.
//! - AREG: `[VARIABLES=] dependent WITH regressors`, METHOD= PW | CO | ML, CONSTANT or
//!   NOCONSTANT, RHO=value above -1 and below 1, MXITER=n, APPLY [='name'] [SPECIFICATIONS |
//!   INITIAL | FIT]. The last of a repeated subcommand counts.
//! - 2SLS: `[EQUATION=] dependent WITH predictors`, more equations after slashes,
//!   INSTRUMENTS= (cumulative), ENDOGENOUS= (documentation only), CONSTANT or NOCONSTANT,
//!   PRINT=COV, SAVE= PRED RESID, APPLY [='name'].

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::ts_models::{ArimaApply, apply_name, error, number, numbers, subcommands, whole};

// ---------------------------------------------------------------- SPECTRA

/// A named spectral window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpectraWindow {
    Hamming,
    Tukey,
    Parzen,
    Bartlett,
    /// UNIT or DANIELL: equal weights.
    Daniell,
    None,
}

/// WINDOW as written: a named window with its span, or the weights themselves.
#[derive(Debug, Clone, PartialEq)]
pub enum SpectraSmoothing {
    Named(SpectraWindow, Option<i64>),
    Weights(Vec<f64>),
}

/// The statistics SPECTRA plots and saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpectraItem {
    Freq,
    Per,
    Sin,
    Cos,
    P,
    S,
    Rc,
    Ic,
    Cs,
    Qs,
    Ph,
    A,
    G,
    K,
}

impl SpectraItem {
    /// Whether it belongs to a pair of series (CROSS).
    pub fn bivariate(self) -> bool {
        matches!(
            self,
            Self::Rc | Self::Ic | Self::Cs | Self::Qs | Self::Ph | Self::A | Self::G | Self::K
        )
    }
}

/// BY on PLOT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpectraBy {
    Frequency,
    Period,
}

/// PLOT as written: the plots (empty for NONE) and BY.
#[derive(Debug, Clone, PartialEq)]
pub struct SpectraPlot {
    pub items: Vec<SpectraItem>,
    /// ALL: every plot that applies.
    pub all: bool,
    pub by: Option<SpectraBy>,
}

/// SPECTRA as written: None for what the command leaves out, so that APPLY can fill it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Spectra {
    pub variables: Vec<VarItem>,
    pub apply: Option<Option<String>>,
    pub center: Option<bool>,
    pub cross: Option<bool>,
    pub window: Option<SpectraSmoothing>,
    pub plot: Option<SpectraPlot>,
    /// SAVE: each statistic with the root of its names.
    pub save: Vec<(SpectraItem, String)>,
}

const SPECTRA_ITEMS: [(&str, SpectraItem); 14] = [
    ("FREQ", SpectraItem::Freq),
    ("PER", SpectraItem::Per),
    ("SIN", SpectraItem::Sin),
    ("COS", SpectraItem::Cos),
    ("P", SpectraItem::P),
    ("S", SpectraItem::S),
    ("RC", SpectraItem::Rc),
    ("IC", SpectraItem::Ic),
    ("CS", SpectraItem::Cs),
    ("QS", SpectraItem::Qs),
    ("PH", SpectraItem::Ph),
    ("A", SpectraItem::A),
    ("G", SpectraItem::G),
    ("K", SpectraItem::K),
];

/// One of the statistic keywords, written in full (they are too short to abbreviate).
fn spectra_item(p: &mut Parser, allowed: &[SpectraItem]) -> Option<SpectraItem> {
    let Some(TokenKind::Word(word)) = p.peek() else {
        return None;
    };
    let (_, item) = SPECTRA_ITEMS
        .iter()
        .find(|(name, item)| allowed.contains(item) && word.eq_ignore_ascii_case(name))?;
    p.pos += 1;
    Some(*item)
}

/// `NO` after CENTER or CROSS: false for NO.
fn yes_no(p: &mut Parser) -> bool {
    p.eat(Punct::Eq);
    !p.eat_word("NO")
}

fn window(p: &mut Parser) -> PResult<SpectraSmoothing> {
    p.eat(Punct::Eq);
    let start = p.pos;
    if matches!(p.peek(), Some(TokenKind::Number(_)))
        || (p.at(Punct::Minus) && matches!(p.peek_at(1), Some(TokenKind::Number(_))))
    {
        let weights = numbers(p, false, "a weight")?;
        if !at_end(p) {
            return Err(p.unexpected(&["a weight", "'/'"]));
        }
        return Ok(SpectraSmoothing::Weights(weights));
    }
    const NAMES: [(&str, SpectraWindow); 7] = [
        ("HAMMING", SpectraWindow::Hamming),
        ("TUKEY", SpectraWindow::Tukey),
        ("PARZEN", SpectraWindow::Parzen),
        ("BARTLETT", SpectraWindow::Bartlett),
        ("UNIT", SpectraWindow::Daniell),
        ("DANIELL", SpectraWindow::Daniell),
        ("NONE", SpectraWindow::None),
    ];
    let mut first: Option<SpectraSmoothing> = None;
    while !at_end(p) {
        let Some((_, kind)) = NAMES.iter().find(|(name, _)| p.at_keyword(name)) else {
            return Err(p.unexpected(&[
                "HAMMING", "TUKEY", "PARZEN", "BARTLETT", "UNIT", "DANIELL", "NONE", "weights",
            ]));
        };
        p.pos += 1;
        let mut span = None;
        if *kind != SpectraWindow::None && p.eat(Punct::LParen) {
            let at = p.pos;
            let negative = p.eat(Punct::Minus);
            let v = number(p, "a span")?;
            if v.fract() != 0.0 {
                return Err(error(p, at, "the span is a whole number"));
            }
            span = Some(if negative { -v } else { v } as i64);
            p.expect(Punct::RParen)?;
        }
        // of several window keywords the first counts
        first.get_or_insert(SpectraSmoothing::Named(*kind, span));
        p.eat(Punct::Comma);
    }
    first.ok_or_else(|| error(p, start, "WINDOW needs a window name or weights"))
}

fn spectra_plot(p: &mut Parser) -> PResult<SpectraPlot> {
    p.eat(Punct::Eq);
    use SpectraItem::*;
    let mut plot = SpectraPlot {
        items: Vec::new(),
        all: false,
        by: None,
    };
    let mut none = false;
    while !at_end(p) {
        if p.eat_word("BY") {
            plot.by = Some(if p.eat_keyword("FREQ") || p.eat_keyword("FREQUENCY") {
                SpectraBy::Frequency
            } else if p.eat_keyword("PERIOD") {
                SpectraBy::Period
            } else {
                return Err(p.unexpected(&["FREQ", "PERIOD"]));
            });
        } else if p.eat_word("ALL") {
            plot.all = true;
        } else if p.eat_keyword("NONE") {
            none = true;
        } else if let Some(item) = spectra_item(p, &[P, S, Cs, Qs, Ph, A, G, K]) {
            if !plot.items.contains(&item) {
                plot.items.push(item);
            }
        } else {
            return Err(p.unexpected(&[
                "P", "S", "CS", "QS", "PH", "A", "G", "K", "ALL", "NONE", "BY",
            ]));
        }
        p.eat(Punct::Comma);
    }
    if none {
        plot.items.clear();
        plot.all = false;
    }
    Ok(plot)
}

fn spectra_save(p: &mut Parser) -> PResult<Vec<(SpectraItem, String)>> {
    p.eat(Punct::Eq);
    let mut saves = Vec::new();
    while !at_end(p) {
        let all: Vec<SpectraItem> = SPECTRA_ITEMS.iter().map(|(_, i)| *i).collect();
        let Some(item) = spectra_item(p, &all) else {
            return Err(p.unexpected(&[
                "FREQ", "PER", "SIN", "COS", "P", "S", "RC", "IC", "CS", "QS", "PH", "A", "G", "K",
            ]));
        };
        p.expect(Punct::LParen)?;
        let root = p.name("a root name for the new variables")?;
        p.expect(Punct::RParen)?;
        saves.retain(|(i, _)| *i != item);
        saves.push((item, root));
        p.eat(Punct::Comma);
    }
    if saves.is_empty() {
        return Err(p.unexpected(&["a statistic such as P (name)"]));
    }
    Ok(saves)
}

pub(crate) fn spectra(p: &mut Parser) -> PResult<Command> {
    let mut c = Spectra::default();
    let mut variables = Vec::new();
    subcommands(
        p,
        |p| {
            variables = list_until(p, &[])?;
            Ok(())
        },
        |p| {
            if p.eat_keyword("APPLY") {
                c.apply = Some(apply_name(p)?);
            } else if p.eat_keyword("CENTER") {
                c.center = Some(yes_no(p));
            } else if p.eat_keyword("CROSS") {
                c.cross = Some(yes_no(p));
            } else if p.eat_keyword("WINDOW") {
                c.window = Some(window(p)?);
            } else if p.eat_keyword("PLOT") {
                c.plot = Some(spectra_plot(p)?);
            } else if p.eat_keyword("SAVE") {
                c.save = spectra_save(p)?;
            } else {
                return Ok(false);
            }
            Ok(true)
        },
        "SPECTRA",
    )?;
    c.variables = variables;
    if c.variables.is_empty() && c.apply.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "SPECTRA needs VARIABLES, or APPLY to use the previous SPECTRA's",
        ));
    }
    Ok(Command::Spectra(Box::new(c)))
}

// ---------------------------------------------------------------- AREG

/// The estimation method of AREG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AregMethod {
    /// Prais-Winsten generalized least squares.
    Pw,
    /// Cochrane-Orcutt.
    Co,
    /// Exact maximum likelihood.
    Ml,
}

/// AREG as written: None for what the command leaves out, so that APPLY and TSET can fill it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Areg {
    pub dependent: Vec<VarItem>,
    pub regressors: Vec<VarItem>,
    pub apply: Option<(Option<String>, ArimaApply)>,
    pub method: Option<AregMethod>,
    pub constant: Option<bool>,
    pub rho: Option<f64>,
    pub mxiter: Option<u32>,
}

pub(crate) fn areg(p: &mut Parser) -> PResult<Command> {
    let mut c = Areg::default();
    let (mut dependent, mut regressors) = (Vec::new(), Vec::new());
    subcommands(
        p,
        |p| {
            dependent = list_until(p, &["WITH"])?;
            if p.eat_word("WITH") {
                regressors = list_until(p, &[])?;
            }
            Ok(())
        },
        |p| {
            let start = p.pos;
            if p.eat_keyword("APPLY") {
                let name = apply_name(p)?;
                let how = if p.eat_keyword("SPECIFICATIONS") {
                    ArimaApply::Specifications
                } else if p.eat_keyword("INITIAL") {
                    ArimaApply::Initial
                } else if p.eat_word("FIT") {
                    ArimaApply::Fit
                } else {
                    ArimaApply::Specifications
                };
                c.apply = Some((name, how));
            } else if p.eat_keyword("METHOD") {
                p.eat(Punct::Eq);
                c.method = Some(if p.eat_word("PW") {
                    AregMethod::Pw
                } else if p.eat_word("CO") {
                    AregMethod::Co
                } else if p.eat_word("ML") {
                    AregMethod::Ml
                } else {
                    return Err(p.unexpected(&["PW", "CO", "ML"]));
                });
            } else if p.eat_keyword("CONSTANT") {
                c.constant = Some(true);
            } else if p.eat_keyword("NOCONSTANT") {
                c.constant = Some(false);
            } else if p.eat_keyword("RHO") {
                p.eat(Punct::Eq);
                let negative = p.eat(Punct::Minus);
                let v = number(p, "a value")?;
                let v = if negative { -v } else { v };
                if v <= -1.0 || v >= 1.0 {
                    return Err(error(p, start, "RHO is above -1 and below 1"));
                }
                c.rho = Some(v);
            } else if p.eat_keyword("MXITER") {
                p.eat(Punct::Eq);
                c.mxiter = Some(whole(p, 1, "MXITER")?);
            } else {
                return Ok(false);
            }
            Ok(true)
        },
        "AREG",
    )?;
    if dependent.len() > 1 || matches!(dependent.first(), Some(VarItem::Range(..) | VarItem::All)) {
        return Err(SyntaxError::new(
            p.span(),
            "AREG takes one dependent series; put the regressors after WITH",
        ));
    }
    if !dependent.is_empty() && regressors.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "AREG needs at least one regressor after WITH",
        ));
    }
    c.dependent = dependent;
    c.regressors = regressors;
    if c.dependent.is_empty() && c.apply.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "AREG needs a dependent series WITH regressors, or APPLY to use the previous AREG's",
        ));
    }
    Ok(Command::Areg(Box::new(c)))
}

// ---------------------------------------------------------------- 2SLS

/// One structural equation: the dependent variable and its predictors.
#[derive(Debug, Clone, PartialEq)]
pub struct Equation {
    pub dependent: String,
    pub predictors: Vec<VarItem>,
}

/// 2SLS as written: None for what the command leaves out, so that APPLY and TSET can fill it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TwoStage {
    pub equations: Vec<Equation>,
    /// Every INSTRUMENTS list, joined.
    pub instruments: Option<Vec<VarItem>>,
    /// ENDOGENOUS: documentation only.
    pub endogenous: Vec<VarItem>,
    pub constant: Option<bool>,
    /// PRINT=COV.
    pub print_cov: bool,
    /// SAVE: the predicted values and the residuals; both false without SAVE.
    pub save_pred: bool,
    pub save_resid: bool,
    pub apply: Option<Option<String>>,
}

fn equation(p: &mut Parser) -> PResult<Equation> {
    if p.eat_keyword("EQUATION") {
        p.eat(Punct::Eq);
    }
    let start = p.pos;
    let dependent = p.name("the dependent variable")?;
    if !p.eat_word("WITH") {
        return Err(error(
            p,
            start,
            "an equation is one dependent variable, WITH and its predictors",
        ));
    }
    let predictors = list_until(p, &[])?;
    Ok(Equation {
        dependent,
        predictors,
    })
}

pub(crate) fn two_stage(p: &mut Parser) -> PResult<Command> {
    let mut c = TwoStage::default();
    let mut first = None;
    let mut equations = Vec::new();
    subcommands(
        p,
        |p| {
            first = Some(equation(p)?);
            Ok(())
        },
        |p| {
            let next_is_with =
                matches!(p.peek_at(1), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("WITH"));
            if p.at_keyword("EQUATION") || next_is_with {
                equations.push(equation(p)?);
            } else if p.eat_keyword("INSTRUMENTS") {
                p.eat(Punct::Eq);
                c.instruments
                    .get_or_insert_with(Vec::new)
                    .extend(list_until(p, &[])?);
            } else if p.eat_keyword("ENDOGENOUS") {
                p.eat(Punct::Eq);
                c.endogenous.extend(list_until(p, &[])?);
            } else if p.eat_keyword("CONSTANT") {
                c.constant = Some(true);
            } else if p.eat_keyword("NOCONSTANT") {
                c.constant = Some(false);
            } else if p.eat_keyword("PRINT") {
                p.eat(Punct::Eq);
                if !p.eat_keyword("COV") {
                    return Err(p.unexpected(&["COV"]));
                }
                c.print_cov = true;
            } else if p.eat_keyword("SAVE") {
                p.eat(Punct::Eq);
                let mut any = false;
                while !at_end(p) {
                    if p.eat_keyword("PRED") {
                        c.save_pred = true;
                    } else if p.eat_keyword("RESID") {
                        c.save_resid = true;
                    } else {
                        return Err(p.unexpected(&["PRED", "RESID"]));
                    }
                    any = true;
                    p.eat(Punct::Comma);
                }
                if !any {
                    return Err(p.unexpected(&["PRED", "RESID"]));
                }
            } else if p.eat_keyword("APPLY") {
                c.apply = Some(apply_name(p)?);
            } else {
                return Ok(false);
            }
            Ok(true)
        },
        "2SLS",
    )?;
    c.equations = first.into_iter().chain(equations).collect();
    if c.equations.is_empty() && c.apply.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "2SLS needs an equation (dependent WITH predictors), or APPLY to use the previous 2SLS's",
        ));
    }
    if c.instruments.is_none() && c.apply.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "2SLS needs INSTRUMENTS, the variables that predict the endogenous ones",
        ));
    }
    Ok(Command::TwoStage(Box::new(c)))
}
