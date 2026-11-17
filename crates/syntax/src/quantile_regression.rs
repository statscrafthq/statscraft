//! Parser of QUANTILE REGRESSION: linear models of conditional quantiles.
//!
//! Rules from the Command Syntax Reference (QUANTILE REGRESSION), in our words:
//! - `QUANTILE REGRESSION dependent [BY factors] [WITH covariates]`, one numeric dependent.
//!   Subcommands follow in any order; a repeated one replaces the earlier one.
//! - MISSING: CLASSMISSING = EXCLUDE (default) | INCLUDE.
//! - CRITERIA: QUANTILE = values and at most one grid `v1 TO v2 BY v3` (0 <= v1 <= v2 <= 1,
//!   0.01 <= v3 <= 1), every value in [0, 1], 0.5 by default; IID = TRUE | FALSE;
//!   BANDWIDTH = BOFINGER | HALL_SHEATHER; CILEVEL in (0, 100), 95; METHOD = AUTO | SIMPLEX |
//!   INTERIOR_POINT; TOL and CONV in (0, 0.001], 1E-12 and 1E-6; MAXITER a positive whole
//!   number, 2000.
//! - MODEL: effects as in a GLM design, REGWGT = variable, INTERCEPT = TRUE | FALSE.
//! - SAVE: PRED, RESID, CIPREDL, CIPREDU, each with an optional name in parentheses.
//! - PREDICT_EFFECTS NUM_TOP_EFFECTS = n (3); PREDICTION_TABLES and PREDICTION_LINES list
//!   effects of the model.
//! - PRINT: PARAMETER, COVB, CORB; when given, only what it names is shown.
//! - PLOT: MAX_CATEGORIES = n (10), PREDICTED_BY_OBSERVED = TRUE | FALSE, PARAM_EST = n | ALL (50).
//! - OUTFILE: COVB and CORB = file or dataset, FILE_SEPARATE = TRUE | FALSE, MODEL or PARAMETER =
//!   file. EXTERNAL takes nothing.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::glm::{Term, list_until, term};
use crate::lexer::Punct;
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrMethod {
    Auto,
    Simplex,
    InteriorPoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QrSave {
    Pred,
    Resid,
    Cipredl,
    Cipredu,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuantileRegression {
    pub dependent: String,
    pub factors: Vec<VarItem>,
    pub covariates: Vec<VarItem>,
    /// CLASSMISSING = INCLUDE: user-missing factor values are valid.
    pub include: bool,
    /// As written, the grid expanded; duplicates are left to the procedure, which warns.
    pub quantiles: Vec<f64>,
    pub iid: bool,
    pub hall_sheather: bool,
    pub cilevel: f64,
    pub method: QrMethod,
    pub tol: f64,
    pub conv: f64,
    pub maxiter: u32,
    /// None: the intercept and every main effect.
    pub effects: Option<Vec<Term>>,
    pub regwgt: Option<String>,
    pub intercept: bool,
    pub save: Vec<(QrSave, Option<String>)>,
    pub top_effects: usize,
    pub prediction_tables: Option<Vec<Term>>,
    pub prediction_lines: Option<Vec<Term>>,
    pub print_parameter: bool,
    pub print_covb: bool,
    pub print_corb: bool,
    pub max_categories: usize,
    pub predicted_by_observed: bool,
    /// None: ALL.
    pub param_est: Option<usize>,
    pub outfile_covb: Option<String>,
    pub outfile_corb: Option<String>,
    pub file_separate: bool,
    /// MODEL or PARAMETER: the file, and whether the covariances go in (MODEL).
    pub outfile_model: Option<(String, bool)>,
}

fn error<T>(p: &Parser, start: usize, message: &str) -> PResult<T> {
    Err(SyntaxError::new(p.span_from(start), message.to_string()))
}

fn boolean(p: &mut Parser) -> PResult<bool> {
    p.eat(Punct::Eq);
    if p.eat_keyword("TRUE") {
        Ok(true)
    } else if p.eat_keyword("FALSE") {
        Ok(false)
    } else {
        Err(p.unexpected(&["TRUE", "FALSE"]))
    }
}

/// `= number` checked by `ok`.
fn checked(p: &mut Parser, ok: impl Fn(f64) -> bool, rule: &str) -> PResult<f64> {
    p.eat(Punct::Eq);
    let start = p.pos;
    match p.number() {
        Some(v) if ok(v) => Ok(v),
        Some(_) => error(p, start, rule),
        None => Err(p.unexpected(&["a number"])),
    }
}

fn whole(v: f64) -> bool {
    v.fract() == 0.0 && v >= 0.0 && v <= f64::from(u32::MAX)
}

fn quantiles(p: &mut Parser) -> PResult<Vec<f64>> {
    p.eat(Punct::Eq);
    let mut out = Vec::new();
    let mut grid = false;
    while let Some(v) = {
        let start = p.pos;
        p.number().map(|v| (v, start))
    } {
        let (v, start) = v;
        if !(0.0..=1.0).contains(&v) {
            return error(p, start, "a quantile is between 0 and 1");
        }
        if !p.eat_word("TO") {
            out.push(v);
            continue;
        }
        let to = checked(
            p,
            |t| (v..=1.0).contains(&t),
            "TO is followed by a value from the first one up to 1",
        )?;
        if !p.eat_word("BY") {
            return Err(p.unexpected(&["BY"]));
        }
        let by = checked(
            p,
            |b| (0.01..=1.0).contains(&b),
            "BY takes a step from 0.01 to 1",
        )?;
        if grid {
            return error(
                p,
                start,
                "only one grid of quantiles (TO ... BY) is allowed",
            );
        }
        grid = true;
        let mut i = 0.0;
        // rounded so that 0.1 steps give 0.3, not 0.30000000000000004
        while v + i * by <= to + 1e-9 {
            out.push(((v + i * by) * 1e10).round() / 1e10);
            i += 1.0;
        }
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a quantile"]));
    }
    Ok(out)
}

fn criteria(p: &mut Parser, c: &mut QuantileRegression) -> PResult<()> {
    while !at_end(p) {
        if p.eat_keyword("QUANTILE") {
            c.quantiles = quantiles(p)?;
        } else if p.eat_keyword("IID") {
            c.iid = boolean(p)?;
        } else if p.eat_keyword("BANDWIDTH") {
            p.eat(Punct::Eq);
            c.hall_sheather = if p.eat_keyword("BOFINGER") {
                false
            } else if p.eat_keyword("HALL_SHEATHER") {
                true
            } else {
                return Err(p.unexpected(&["BOFINGER", "HALL_SHEATHER"]));
            };
        } else if p.eat_keyword("CILEVEL") {
            c.cilevel = checked(
                p,
                |v| v > 0.0 && v < 100.0,
                "CILEVEL is above 0 and below 100",
            )?;
        } else if p.eat_keyword("METHOD") {
            p.eat(Punct::Eq);
            c.method = if p.eat_keyword("AUTO") {
                QrMethod::Auto
            } else if p.eat_keyword("SIMPLEX") {
                QrMethod::Simplex
            } else if p.eat_keyword("INTERIOR_POINT") {
                QrMethod::InteriorPoint
            } else {
                return Err(p.unexpected(&["AUTO", "SIMPLEX", "INTERIOR_POINT"]));
            };
        } else if p.eat_keyword("TOL") {
            c.tol = checked(
                p,
                |v| v > 0.0 && v <= 1e-3,
                "TOL is above 0 and at most 0.001",
            )?;
        } else if p.eat_keyword("CONV") {
            c.conv = checked(
                p,
                |v| v > 0.0 && v <= 1e-3,
                "CONV is above 0 and at most 0.001",
            )?;
        } else if p.eat_keyword("MAXITER") {
            let v = checked(
                p,
                |v| whole(v) && v >= 1.0,
                "MAXITER is a whole number, 1 or more",
            )?;
            c.maxiter = v as u32;
        } else {
            return Err(p.unexpected(&[
                "QUANTILE",
                "IID",
                "BANDWIDTH",
                "CILEVEL",
                "METHOD",
                "TOL",
                "CONV",
                "MAXITER",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

/// A keyword followed by `=`, so that a variable of the same name stays an effect.
fn assignment(p: &mut Parser, keyword: &str) -> bool {
    if p.at_keyword(keyword) && p.peek_at(1) == Some(&crate::lexer::TokenKind::Punct(Punct::Eq)) {
        p.pos += 2;
        return true;
    }
    false
}

fn model(p: &mut Parser, c: &mut QuantileRegression) -> PResult<()> {
    let mut effects = Vec::new();
    while !at_end(p) {
        if assignment(p, "REGWGT") {
            c.regwgt = Some(p.name("the regression weight variable")?);
        } else if assignment(p, "INTERCEPT") {
            c.intercept = boolean(p)?;
        } else {
            effects.push(term(p)?);
        }
        p.eat(Punct::Comma);
    }
    c.effects = (!effects.is_empty()).then_some(effects);
    Ok(())
}

fn effect_list(p: &mut Parser) -> PResult<Vec<Term>> {
    let mut out = Vec::new();
    while !at_end(p) {
        out.push(term(p)?);
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn save(p: &mut Parser) -> PResult<Vec<(QrSave, Option<String>)>> {
    let mut out: Vec<(QrSave, Option<String>)> = Vec::new();
    while !at_end(p) {
        let start = p.pos;
        let kind = if p.eat_keyword("PRED") {
            QrSave::Pred
        } else if p.eat_keyword("RESID") {
            QrSave::Resid
        } else if p.eat_keyword("CIPREDL") {
            QrSave::Cipredl
        } else if p.eat_keyword("CIPREDU") {
            QrSave::Cipredu
        } else {
            return Err(p.unexpected(&["PRED", "RESID", "CIPREDL", "CIPREDU"]));
        };
        let mut name = None;
        if p.eat(Punct::LParen) {
            if !p.at(Punct::RParen) {
                name = Some(p.name("a variable name or root name")?);
            }
            p.expect(Punct::RParen)?;
        }
        if out.iter().any(|(k, _)| *k == kind) {
            return error(p, start, "each SAVE keyword is given only once");
        }
        out.push((kind, name));
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn count(p: &mut Parser, least: f64, rule: &str) -> PResult<usize> {
    Ok(checked(p, |v| whole(v) && v >= least, rule)? as usize)
}

fn outfile(p: &mut Parser, c: &mut QuantileRegression) -> PResult<()> {
    while !at_end(p) {
        let start = p.pos;
        if p.eat_keyword("COVB") {
            p.eat(Punct::Eq);
            c.outfile_covb = Some(p.file("a file or dataset")?);
        } else if p.eat_keyword("CORB") {
            p.eat(Punct::Eq);
            c.outfile_corb = Some(p.file("a file or dataset")?);
        } else if p.eat_keyword("FILE_SEPARATE") {
            c.file_separate = boolean(p)?;
        } else if p.at_keyword("MODEL") || p.at_keyword("PARAMETER") {
            let with_covariance = p.eat_keyword("MODEL");
            if !with_covariance {
                p.pos += 1;
            }
            if c.outfile_model.is_some() {
                return error(p, start, "OUTFILE takes MODEL or PARAMETER, once");
            }
            p.eat(Punct::Eq);
            c.outfile_model = Some((p.file("a file")?, with_covariance));
        } else {
            return Err(p.unexpected(&["COVB", "CORB", "FILE_SEPARATE", "MODEL", "PARAMETER"]));
        }
        p.eat(Punct::Comma);
    }
    Ok(())
}

pub(crate) fn quantile_regression(p: &mut Parser) -> PResult<Command> {
    let dependent = p.name("the dependent variable")?;
    if !at_end(p) && !p.at_keyword("BY") && !p.at_keyword("WITH") {
        return Err(SyntaxError::new(
            p.span(),
            "QUANTILE REGRESSION takes one dependent variable; factors follow BY and covariates WITH",
        ));
    }
    let factors = if p.eat_word("BY") {
        list_until(p, &["WITH"])?
    } else {
        Vec::new()
    };
    let covariates = if p.eat_word("WITH") {
        p.var_list()?
    } else {
        Vec::new()
    };
    let mut c = QuantileRegression {
        dependent,
        factors,
        covariates,
        include: false,
        quantiles: vec![0.5],
        iid: true,
        hall_sheather: false,
        cilevel: 95.0,
        method: QrMethod::Auto,
        tol: 1e-12,
        conv: 1e-6,
        maxiter: 2000,
        effects: None,
        regwgt: None,
        intercept: true,
        save: Vec::new(),
        top_effects: 3,
        prediction_tables: None,
        prediction_lines: None,
        print_parameter: true,
        print_covb: false,
        print_corb: false,
        max_categories: 10,
        predicted_by_observed: false,
        param_est: Some(50),
        outfile_covb: None,
        outfile_corb: None,
        file_separate: false,
        outfile_model: None,
    };
    while p.eat(Punct::Slash) {
        // the long names first: PRE alone would match several
        if p.eat_word("PREDICT_EFFECTS") {
            while !at_end(p) {
                if p.eat_keyword("NUM_TOP_EFFECTS") {
                    c.top_effects = count(p, 0.0, "NUM_TOP_EFFECTS is a whole number, 0 or more")?;
                } else {
                    return Err(p.unexpected(&["NUM_TOP_EFFECTS"]));
                }
            }
        } else if p.eat_word("PREDICTION_TABLES") {
            c.prediction_tables = Some(effect_list(p)?);
        } else if p.eat_word("PREDICTION_LINES") {
            c.prediction_lines = Some(effect_list(p)?);
        } else if p.eat_keyword("MISSING") {
            while !at_end(p) {
                if !p.eat_keyword("CLASSMISSING") {
                    return Err(p.unexpected(&["CLASSMISSING"]));
                }
                p.eat(Punct::Eq);
                c.include = if p.eat_keyword("INCLUDE") {
                    true
                } else if p.eat_keyword("EXCLUDE") {
                    false
                } else {
                    return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
                };
            }
        } else if p.eat_keyword("CRITERIA") {
            criteria(p, &mut c)?;
        } else if p.eat_keyword("MODEL") {
            model(p, &mut c)?;
        } else if p.eat_keyword("SAVE") {
            c.save = save(p)?;
        } else if p.eat_keyword("PRINT") {
            (c.print_parameter, c.print_covb, c.print_corb) = (false, false, false);
            while !at_end(p) {
                if p.eat_keyword("PARAMETER") {
                    c.print_parameter = true;
                } else if p.eat_keyword("COVB") {
                    c.print_covb = true;
                } else if p.eat_keyword("CORB") {
                    c.print_corb = true;
                } else {
                    return Err(p.unexpected(&["PARAMETER", "COVB", "CORB"]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("PLOT") {
            while !at_end(p) {
                if p.eat_keyword("MAX_CATEGORIES") {
                    c.max_categories =
                        count(p, 1.0, "MAX_CATEGORIES is a whole number, 1 or more")?;
                } else if p.eat_keyword("PREDICTED_BY_OBSERVED") {
                    c.predicted_by_observed = boolean(p)?;
                } else if p.eat_keyword("PARAM_EST") {
                    p.eat(Punct::Eq);
                    c.param_est = if p.eat_keyword("ALL") {
                        None
                    } else {
                        Some(count(
                            p,
                            0.0,
                            "PARAM_EST is a whole number, 0 or more, or ALL",
                        )?)
                    };
                } else {
                    return Err(p.unexpected(&[
                        "MAX_CATEGORIES",
                        "PREDICTED_BY_OBSERVED",
                        "PARAM_EST",
                    ]));
                }
                p.eat(Punct::Comma);
            }
        } else if p.eat_keyword("OUTFILE") {
            outfile(p, &mut c)?;
        } else if p.eat_keyword("EXTERNAL") {
            // the data are held in memory either way
        } else {
            return Err(p.subcommand_error("QUANTILE REGRESSION", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::QuantileRegression(Box::new(c)))
}
