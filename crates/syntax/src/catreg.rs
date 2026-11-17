//! Parser of CATREG: regression of a quantified response on quantified predictors.
//!
//! Rules from the Command Syntax Reference (CATREG), in our words:
//! - VARIABLES comes first and ANALYSIS is required. ANALYSIS names one response, then WITH and
//!   at least one predictor; parentheses after a variable or list give LEVEL= SPORD (the
//!   default), SPNOM, ORDI, NOMI or NUME, and DEGREE=n (2) and INKNOT=n (2) for the spline levels.
//!   WITH and TO are reserved there; TO among the predictors follows the VARIABLES order.
//! - A subcommand written twice: the last one counts, with a warning.
//! - DISCRETIZATION as for CATPCA. MISSING: varlist or ALL with LISTWISE (the default),
//!   MODEIMPU or EXTRACAT; both impute before the analysis. SUPPLEMENTARY=OBJECT(case numbers).
//! - INITIAL: NUMERICAL (the default), RANDOM, MULTISTART(threshold | ALL)(file), the threshold
//!   from 0 to 100 and 50 when left out, or FIXSIGNS(start number)(file).
//! - MAXITER: a positive whole number (100). CRITITER: from 0.000001 to 0.1 (0.00001).
//! - REGULARIZATION: NONE, RIDGE or LASSO with (start, stop, step), (0, 1, 0.02) by default, or
//!   ENET with a ridge and a lasso range, (0, 1, 0.1)(0, 1, 0.02) by default; an optional file
//!   for the table of every model. A range with start equal to stop and step 0 is one value.
//! - RESAMPLE: NONE, CROSSVAL[(folds)] (10) or BOOTSTRAP[(samples)] (50).
//! - PRINT: R, COEFF, DESCRIP[(varlist)], HISTORY, ANOVA, CORR, OCORR, QUANT[(varlist)], REGU,
//!   NONE; R COEFF DESCRIP ANOVA without the subcommand or with it empty; the last of
//!   repeated or contradicting keywords counts.
//! - PLOT: TRANS(varlist)[(length)], RESID(varlist)[(length)], REGU(values | ALL); none by
//!   default. A length above 60 becomes 60.
//! - SAVE: TRDATA, PRED and RES, each with an optional root (TRA, PRE, RES) cut to 5 letters.
//! - OUTFILE: TRDATA(file) and DISCRDATA(file); a file alone means TRDATA.

use crate::ast::{Command, VarItem};
use crate::catpca::{ScalingLevel, analysis_spec};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::optimal_scaling::{
    Discretize, Impute, McaMissing, ScalingCommon, discretize, distinct_files, label_length,
    object_numbers, optional_list, outfile_name, outfile_target, save_target, specified_lists,
    whole,
};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// How the iterations start.
#[derive(Debug, Clone, PartialEq)]
pub enum CatregInitial {
    Numerical,
    Random,
    /// Systematic starts over the signs of the (spline) ordinal predictors: every pattern
    /// (None) or those a loss threshold in percent allows, and the file for the signs.
    Multistart(Option<f64>, String),
    /// The signs in row `start` (from 1) of the file.
    Fixsigns(usize, String),
}

/// A penalty range: start, stop and step.
pub type PenaltyRange = (f64, f64, f64);

#[derive(Debug, Clone, PartialEq)]
pub enum CatregPenalty {
    Ridge(PenaltyRange),
    Lasso(PenaltyRange),
    /// The ridge range, then the lasso range.
    Enet(PenaltyRange, PenaltyRange),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatregResample {
    None,
    Crossval(usize),
    Bootstrap(usize),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CatregPrint {
    pub r: bool,
    pub coeff: bool,
    /// Some(list) when asked for; an empty list means the analysis variables.
    pub descrip: Option<Vec<VarItem>>,
    pub history: bool,
    pub anova: bool,
    pub corr: bool,
    pub ocorr: bool,
    pub quant: Option<Vec<VarItem>>,
    pub regu: bool,
}

impl CatregPrint {
    pub fn defaults() -> Self {
        CatregPrint {
            r: true,
            coeff: true,
            descrip: Some(Vec::new()),
            anova: true,
            ..CatregPrint::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CatregPlot {
    /// The variables and the label length (None for labels in full).
    pub trans: Option<(Vec<VarItem>, Option<usize>)>,
    pub resid: Option<(Vec<VarItem>, Option<usize>)>,
    /// The ridge penalties of the elastic net plots; empty for all (or for ridge and lasso).
    pub regu: Option<Vec<f64>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Catreg {
    /// VARIABLES, DISCRETIZATION, MISSING, SUPPLEMENTARY objects, MAXITER and CRITITER.
    pub common: ScalingCommon,
    pub dependent: (String, ScalingLevel),
    pub predictors: Vec<(Vec<VarItem>, ScalingLevel)>,
    pub initial: CatregInitial,
    /// The penalty and the file for the table of every model.
    pub regularization: Option<(CatregPenalty, Option<String>)>,
    pub resample: CatregResample,
    pub print: CatregPrint,
    pub plot: CatregPlot,
    /// SAVE roots.
    pub save_trdata: Option<String>,
    pub save_pred: Option<String>,
    pub save_res: Option<String>,
    pub outfile_trdata: Option<String>,
    pub outfile_discrdata: Option<String>,
    /// Subcommands written more than once, for the warning.
    pub repeated: Vec<String>,
}

const SUBCOMMANDS: [&str; 14] = [
    "VARIABLES",
    "ANALYSIS",
    "DISCRETIZATION",
    "MISSING",
    "SUPPLEMENTARY",
    "INITIAL",
    "MAXITER",
    "CRITITER",
    "REGULARIZATION",
    "RESAMPLE",
    "PRINT",
    "PLOT",
    "SAVE",
    "OUTFILE",
];

/// The level in parentheses; WEIGHT and MNOM belong to CATPCA only.
fn level(p: &mut Parser) -> PResult<ScalingLevel> {
    let start = p.pos;
    let spec = analysis_spec(p)?;
    if spec.weight != 1.0 || spec.level == ScalingLevel::MultipleNominal {
        return Err(SyntaxError::new(
            p.span_from(start),
            "CATREG takes LEVEL=SPORD, SPNOM, ORDI, NOMI or NUME with DEGREE and INKNOT; no WEIGHT or MNOM",
        ));
    }
    Ok(spec.level)
}

/// The response with its level, and the predictor lists with theirs.
type AnalysisSpec = ((String, ScalingLevel), Vec<(Vec<VarItem>, ScalingLevel)>);

fn analysis(p: &mut Parser) -> PResult<AnalysisSpec> {
    let default = ScalingLevel::SplineOrdinal {
        degree: 2,
        inknot: 2,
    };
    let dependent = p.name("the response variable")?;
    let dep_level = match p.eat(Punct::LParen) {
        true => {
            let l = level(p)?;
            p.expect(Punct::RParen)?;
            l
        }
        false => default,
    };
    if !p.eat_word("WITH") {
        return Err(SyntaxError::new(
            p.span(),
            "ANALYSIS names exactly one response variable, then WITH and the predictors",
        ));
    }
    let predictors = specified_lists(p, default, level)?;
    if predictors.is_empty() {
        return Err(p.unexpected(&["a predictor"]));
    }
    Ok(((dependent, dep_level), predictors))
}

/// `(start, stop, step)` with defaults when the parenthesis is left out.
fn penalty_range(p: &mut Parser, default: PenaltyRange) -> PResult<PenaltyRange> {
    if !(p.at(Punct::LParen) && matches!(p.peek_at(1), Some(TokenKind::Number(_)))) {
        return Ok(default);
    }
    p.pos += 1;
    let start_pos = p.pos;
    let mut v = Vec::new();
    while !p.at(Punct::RParen) {
        v.push(
            p.number()
                .ok_or_else(|| p.unexpected(&["a penalty value"]))?,
        );
        p.eat(Punct::Comma);
    }
    let span = p.span_from(start_pos);
    p.expect(Punct::RParen)?;
    let [a, b, c] = v[..] else {
        return Err(SyntaxError::new(
            span,
            "a penalty range has three values: start, stop and step",
        ));
    };
    if a < 0.0 || b < a || c < 0.0 || (c == 0.0 && b != a) {
        return Err(SyntaxError::new(
            span,
            "a penalty range needs 0 <= start <= stop and a positive step (0 only when stop equals start)",
        ));
    }
    if c > 0.0 && (b - a) / c > 100_000.0 {
        return Err(SyntaxError::new(
            span,
            "the penalty range has too many values",
        ));
    }
    Ok((a, b, c))
}

/// An optional `('file')` after a keyword's own parentheses.
fn optional_file(p: &mut Parser) -> PResult<Option<String>> {
    match p.at(Punct::LParen) && !matches!(p.peek_at(1), Some(TokenKind::Number(_))) {
        true => Ok(Some(outfile_target(p)?)),
        false => Ok(None),
    }
}

fn initial(p: &mut Parser) -> PResult<CatregInitial> {
    if p.eat_keyword("NUMERICAL") || at_end(p) {
        return Ok(CatregInitial::Numerical);
    }
    if p.eat_keyword("RANDOM") {
        return Ok(CatregInitial::Random);
    }
    if p.eat_keyword("MULTISTART") {
        let mut threshold = Some(50.0);
        if p.at(Punct::LParen)
            && matches!(
                p.peek_at(1),
                Some(TokenKind::Number(_)) | Some(TokenKind::Word(_))
            )
        {
            p.pos += 1;
            let start = p.pos;
            threshold = if p.eat_word("ALL") {
                None
            } else {
                match p.number() {
                    Some(t) if (0.0..=100.0).contains(&t) => Some(t),
                    Some(_) => {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            "the MULTISTART threshold lies from 0 to 100",
                        ));
                    }
                    None => return Err(p.unexpected(&["a threshold", "ALL"])),
                }
            };
            p.expect(Punct::RParen)?;
        }
        let Some(file) = optional_file(p)? else {
            return Err(SyntaxError::new(
                p.span(),
                "MULTISTART needs a file or dataset in parentheses for the signs of each start",
            ));
        };
        return Ok(CatregInitial::Multistart(threshold, file));
    }
    if p.eat_keyword("FIXSIGNS") {
        p.expect(Punct::LParen)?;
        let start = whole(p, "the start number", 1)?;
        p.expect(Punct::RParen)?;
        let file = outfile_target(p)?;
        return Ok(CatregInitial::Fixsigns(start, file));
    }
    Err(p.unexpected(&["NUMERICAL", "RANDOM", "MULTISTART", "FIXSIGNS"]))
}

fn regularization(p: &mut Parser) -> PResult<Option<(CatregPenalty, Option<String>)>> {
    if p.eat_keyword("NONE") || at_end(p) {
        return Ok(None);
    }
    let penalty = if p.eat_keyword("RIDGE") {
        CatregPenalty::Ridge(penalty_range(p, (0.0, 1.0, 0.02))?)
    } else if p.eat_keyword("LASSO") {
        CatregPenalty::Lasso(penalty_range(p, (0.0, 1.0, 0.02))?)
    } else if p.eat_keyword("ENET") {
        let ridge = penalty_range(p, (0.0, 1.0, 0.1))?;
        CatregPenalty::Enet(ridge, penalty_range(p, (0.0, 1.0, 0.02))?)
    } else {
        return Err(p.unexpected(&["NONE", "RIDGE", "LASSO", "ENET"]));
    };
    Ok(Some((penalty, optional_file(p)?)))
}

fn resample(p: &mut Parser) -> PResult<CatregResample> {
    if p.eat_keyword("NONE") || at_end(p) {
        return Ok(CatregResample::None);
    }
    let (cross, default) = if p.eat_keyword("CROSSVAL") {
        (true, 10)
    } else if p.eat_keyword("BOOTSTRAP") {
        (false, 50)
    } else {
        return Err(p.unexpected(&["NONE", "CROSSVAL", "BOOTSTRAP"]));
    };
    let mut n = default;
    if p.eat(Punct::LParen) {
        n = whole(
            p,
            if cross {
                "the number of folds"
            } else {
                "the number of samples"
            },
            2,
        )?;
        p.expect(Punct::RParen)?;
    }
    Ok(match cross {
        true => CatregResample::Crossval(n),
        false => CatregResample::Bootstrap(n),
    })
}

fn print(p: &mut Parser) -> PResult<CatregPrint> {
    if at_end(p) {
        return Ok(CatregPrint::defaults());
    }
    let mut out = CatregPrint::default();
    while !at_end(p) {
        if p.eat_keyword("R") {
            out.r = true;
        } else if p.eat_keyword("COEFF") {
            out.coeff = true;
        } else if p.eat_keyword("DESCRIP") {
            out.descrip = Some(optional_list(p)?);
        } else if p.eat_keyword("HISTORY") {
            out.history = true;
        } else if p.eat_keyword("ANOVA") {
            out.anova = true;
        } else if p.eat_keyword("CORR") {
            out.corr = true;
        } else if p.eat_keyword("OCORR") {
            out.ocorr = true;
        } else if p.eat_keyword("QUANT") {
            out.quant = Some(optional_list(p)?);
        } else if p.eat_keyword("REGU") {
            out.regu = true;
        } else if p.eat_keyword("NONE") {
            out = CatregPrint::default();
        } else {
            return Err(p.unexpected(&[
                "R", "COEFF", "DESCRIP", "HISTORY", "ANOVA", "CORR", "OCORR", "QUANT", "REGU",
                "NONE",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

/// `(varlist)[(length)]` after TRANS or RESID.
fn plot_list(p: &mut Parser, keyword: &str) -> PResult<(Vec<VarItem>, Option<usize>)> {
    if !p.eat(Punct::LParen) {
        return Err(p
            .unexpected(&["'('"])
            .hint(format!("{keyword} needs a list of variables")));
    }
    let items = p.var_list()?;
    p.expect(Punct::RParen)?;
    Ok((items, label_length(p)?.map(|l| l.min(60))))
}

fn plot(p: &mut Parser) -> PResult<CatregPlot> {
    let mut out = CatregPlot::default();
    while !at_end(p) {
        if p.eat_keyword("TRANS") {
            out.trans = Some(plot_list(p, "TRANS")?);
        } else if p.eat_keyword("RESID") {
            out.resid = Some(plot_list(p, "RESID")?);
        } else if p.eat_keyword("REGU") {
            let mut values = Vec::new();
            if p.eat(Punct::LParen) {
                if !p.eat_word("ALL") {
                    while !p.at(Punct::RParen) {
                        values.push(
                            p.number()
                                .ok_or_else(|| p.unexpected(&["a ridge penalty", "ALL"]))?,
                        );
                        p.eat(Punct::Comma);
                    }
                }
                p.expect(Punct::RParen)?;
            }
            out.regu = Some(values);
        } else {
            return Err(p.unexpected(&["TRANS", "RESID", "REGU"]));
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

/// A SAVE root: the first name in parentheses, cut to five letters.
fn root(p: &mut Parser, default: &str) -> PResult<String> {
    let start = p.pos;
    let (root, dims) = save_target(p, default, 5)?;
    if dims.is_some() {
        return Err(SyntaxError::new(
            p.span_from(start),
            "a CATREG SAVE root takes no number in parentheses",
        ));
    }
    Ok(root)
}

pub(crate) fn catreg(p: &mut Parser) -> PResult<Command> {
    let mut common = ScalingCommon::default();
    let mut dependent = None;
    let mut predictors = Vec::new();
    let mut initial_spec = CatregInitial::Numerical;
    let mut regularization_spec = None;
    let mut resample_spec = CatregResample::None;
    let mut print_spec = CatregPrint::defaults();
    let mut plot_spec = CatregPlot::default();
    let (mut save_trdata, mut save_pred, mut save_res) = (None, None, None);
    let (mut outfile_trdata, mut outfile_discrdata) = (None, None);
    let mut seen: Vec<&str> = Vec::new();
    let mut repeated: Vec<String> = Vec::new();
    loop {
        let slash = p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        if !slash && !seen.is_empty() {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
        let Some(name) = SUBCOMMANDS.into_iter().find(|k| p.at_keyword(k)) else {
            return Err(p.subcommand_error("CATREG", &[]));
        };
        if seen.is_empty() && name != "VARIABLES" {
            return Err(SyntaxError::new(
                p.span(),
                "VARIABLES must be the first subcommand of CATREG",
            ));
        }
        p.pos += 1;
        p.eat(Punct::Eq);
        if seen.contains(&name) {
            if !repeated.iter().any(|r| r == name) {
                repeated.push(name.to_string());
            }
        } else {
            seen.push(name);
        }
        let start = p.pos;
        match name {
            "VARIABLES" => common.variables = p.var_list()?,
            "ANALYSIS" => {
                let (d, list) = analysis(p)?;
                dependent = Some(d);
                predictors = list;
            }
            "DISCRETIZATION" => {
                common.discretization = specified_lists(
                    p,
                    Discretize::Grouping {
                        ncat: None,
                        normal: true,
                    },
                    discretize,
                )?
            }
            "MISSING" => {
                common.missing = specified_lists(p, McaMissing::Listwise, |p| {
                    if p.eat_keyword("LISTWISE") {
                        Ok(McaMissing::Listwise)
                    } else if p.eat_keyword("MODEIMPU") {
                        Ok(McaMissing::Active(Impute::Mode))
                    } else if p.eat_keyword("EXTRACAT") {
                        Ok(McaMissing::Active(Impute::ExtraCategory))
                    } else {
                        Err(p.unexpected(&["LISTWISE", "MODEIMPU", "EXTRACAT"]))
                    }
                })?;
                if common.missing.iter().any(|(items, _)| {
                    items.contains(&VarItem::All) && (items.len() > 1 || common.missing.len() > 1)
                }) {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "ALL on MISSING must be the only variable specification",
                    ));
                }
            }
            "SUPPLEMENTARY" => {
                common.supplementary_objects.clear();
                while !at_end(p) {
                    if !p.eat_keyword("OBJECT") {
                        return Err(p.unexpected(&["OBJECT"]));
                    }
                    p.expect(Punct::LParen)?;
                    common.supplementary_objects.extend(object_numbers(p)?);
                    p.expect(Punct::RParen)?;
                    p.eat(Punct::Comma);
                }
            }
            "INITIAL" => initial_spec = initial(p)?,
            "MAXITER" => common.max_iterations = whole(p, "the most iterations", 1)?,
            "CRITITER" => {
                common.criterion = match p.number() {
                    Some(v) if (0.000001..=0.1).contains(&v) => v,
                    Some(_) => {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            "CRITITER lies from 0.000001 to 0.1",
                        ));
                    }
                    None => return Err(p.unexpected(&["the convergence criterion"])),
                }
            }
            "REGULARIZATION" => regularization_spec = regularization(p)?,
            "RESAMPLE" => resample_spec = resample(p)?,
            "PRINT" => print_spec = print(p)?,
            "PLOT" => plot_spec = plot(p)?,
            "SAVE" => {
                (save_trdata, save_pred, save_res) = (None, None, None);
                while !at_end(p) {
                    if p.eat_keyword("TRDATA") {
                        save_trdata = Some(root(p, "TRA")?);
                    } else if p.eat_keyword("PRED") {
                        save_pred = Some(root(p, "PRE")?);
                    } else if p.eat_keyword("RES") {
                        save_res = Some(root(p, "RES")?);
                    } else {
                        return Err(p.unexpected(&["TRDATA", "PRED", "RES"]));
                    }
                    p.eat(Punct::Comma);
                }
                // SAVE alone saves the transformed variables
                if (&save_trdata, &save_pred, &save_res) == (&None, &None, &None) {
                    save_trdata = Some("TRA".into());
                }
            }
            _ => {
                (outfile_trdata, outfile_discrdata) = (None, None);
                if matches!(p.peek(), Some(TokenKind::Str(_))) {
                    outfile_trdata = Some(outfile_name(p)?);
                }
                while !at_end(p) {
                    if p.eat_keyword("TRDATA") {
                        outfile_trdata = Some(outfile_target(p)?);
                    } else if p.eat_keyword("DISCRDATA") {
                        outfile_discrdata = Some(outfile_target(p)?);
                    } else {
                        return Err(p.unexpected(&["TRDATA", "DISCRDATA"]));
                    }
                    p.eat(Punct::Comma);
                }
                distinct_files(p, start, &[&outfile_trdata, &outfile_discrdata])?;
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    let Some(dependent) = dependent else {
        return Err(SyntaxError::new(
            p.span(),
            "CATREG needs the ANALYSIS subcommand",
        ));
    };
    Ok(Command::Catreg(Box::new(Catreg {
        common,
        dependent,
        predictors,
        initial: initial_spec,
        regularization: regularization_spec,
        resample: resample_spec,
        print: print_spec,
        plot: plot_spec,
        save_trdata,
        save_pred,
        save_res,
        outfile_trdata,
        outfile_discrdata,
        repeated,
    })))
}
