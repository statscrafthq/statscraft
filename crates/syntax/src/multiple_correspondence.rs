//! Parser of MULTIPLE CORRESPONDENCE: homogeneity analysis of categorical variables.
//!
//! Rules from the Command Syntax Reference (MULTIPLE CORRESPONDENCE), in our words; the
//! subcommands it shares with CATPCA are read in `optimal_scaling`:
//! - ANALYSIS is required. It gives each variable a weight, `(WEIGHT=n)` after a variable or a
//!   list; 1 when none. The weight covers the variables named since the previous parenthesis.
//! - PRINT: CORR, DESCRIP[(varlist)], DISCRIM, HISTORY, NONE, OBJECT[((label) varlist)], OCORR
//!   and QUANT[(varlist)]. Without the subcommand or without keywords: DESCRIP DISCRIM CORR.
//!   NONE drops the keywords before it; a keyword written twice counts once, except the ones
//!   that take a list, for which it is an error.
//! - PLOT: OBJECT, CATEGORY (or CATEGORIES), JOINTCAT, DISCRIM, BIPLOT, TRANS, RESID, NONE and
//!   NDIM(first, last). Every keyword but TRANS and NONE takes a label length in parentheses at
//!   its end. Without the subcommand or without keywords: OBJECT and DISCRIM. A keyword twice
//!   is an error; NONE drops the keywords before it.
//! - SAVE: TRDATA[(root[(n)])] and OBJECT[(root[(n)])]; the root is cut to 3 and 5 characters.
//! - OUTFILE: DISCRDATA, TRDATA and OBJECT, each with a file or dataset in parentheses.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::optimal_scaling::{
    ScalingCommon, assigned, dimension_list, distinct_files, label_length, object_list,
    optional_list, outfile_target, save_target, scaling_command, specified_lists, whole,
};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct McaPrint {
    pub corr: bool,
    /// Some(list) when asked for; an empty list means the analysis variables.
    pub descrip: Option<Vec<VarItem>>,
    pub discrim: bool,
    pub history: bool,
    /// The labelling variable and the variables shown next to the scores.
    pub object: Option<(Option<String>, Vec<VarItem>)>,
    pub ocorr: bool,
    pub quant: Option<Vec<VarItem>>,
}

impl McaPrint {
    pub fn defaults() -> Self {
        McaPrint {
            corr: true,
            descrip: Some(Vec::new()),
            discrim: true,
            ..McaPrint::default()
        }
    }
}

/// One requested chart kind with its variables and label length.
#[derive(Debug, Clone, PartialEq)]
pub enum McaPlot {
    /// The variables whose categories label the objects; empty for case numbers.
    Object(Vec<VarItem>),
    Category(Vec<VarItem>),
    /// Empty for every analysis variable.
    Jointcat(Vec<VarItem>),
    Discrim(Vec<VarItem>),
    /// The variables whose centroids are drawn (empty: all) and those labelling the objects.
    Biplot(Vec<VarItem>, Vec<VarItem>),
    /// Each list with the number of dimensions to draw.
    Trans(Vec<(VarItem, usize)>),
    Resid(Vec<(VarItem, usize)>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct McaPlots {
    /// Each chart with its label length (None for labels in full).
    pub charts: Vec<(McaPlot, Option<usize>)>,
    pub ndim: Option<(usize, usize)>,
}

impl Default for McaPlots {
    fn default() -> Self {
        McaPlots {
            charts: vec![
                (McaPlot::Object(Vec::new()), None),
                (McaPlot::Discrim(Vec::new()), None),
            ],
            ndim: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MultipleCorrespondence {
    /// VARIABLES, DISCRETIZATION, MISSING, SUPPLEMENTARY, CONFIGURATION, DIMENSION,
    /// NORMALIZATION, MAXITER and CRITITER.
    pub common: ScalingCommon,
    /// Lists with their variable weight.
    pub analysis: Vec<(Vec<VarItem>, f64)>,
    pub print: McaPrint,
    pub plot: McaPlots,
    /// SAVE: the root and the number of dimensions (None for all).
    pub save_trdata: Option<(String, Option<usize>)>,
    pub save_object: Option<(String, Option<usize>)>,
    pub outfile_discrdata: Option<String>,
    pub outfile_trdata: Option<String>,
    pub outfile_object: Option<String>,
}

fn print(p: &mut Parser) -> PResult<McaPrint> {
    p.eat(Punct::Eq);
    if at_end(p) {
        return Ok(McaPrint::defaults());
    }
    let mut out = McaPrint::default();
    let twice = |p: &Parser, start: usize, name: &str| {
        SyntaxError::new(
            p.span_from(start),
            format!("PRINT names {name} twice; a keyword with a list may appear once"),
        )
    };
    let mut seen: Vec<&str> = Vec::new();
    while !at_end(p) {
        let start = p.pos;
        if p.eat_keyword("CORR") {
            out.corr = true;
        } else if p.eat_keyword("DISCRIM") {
            out.discrim = true;
        } else if p.eat_keyword("HISTORY") {
            out.history = true;
        } else if p.eat_keyword("OCORR") {
            out.ocorr = true;
        } else if p.eat_keyword("NONE") {
            out = McaPrint::default();
            seen.clear();
        } else if p.eat_keyword("DESCRIP") {
            if seen.contains(&"DESCRIP") {
                return Err(twice(p, start, "DESCRIP"));
            }
            seen.push("DESCRIP");
            out.descrip = Some(optional_list(p)?);
        } else if p.eat_keyword("QUANT") {
            if seen.contains(&"QUANT") {
                return Err(twice(p, start, "QUANT"));
            }
            seen.push("QUANT");
            out.quant = Some(optional_list(p)?);
        } else if p.eat_keyword("OBJECT") {
            if seen.contains(&"OBJECT") {
                return Err(twice(p, start, "OBJECT"));
            }
            seen.push("OBJECT");
            out.object = Some(object_list(p)?);
        } else {
            return Err(p.unexpected(&[
                "CORR", "DESCRIP", "DISCRIM", "HISTORY", "NONE", "OBJECT", "OCORR", "QUANT",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn plot(p: &mut Parser) -> PResult<McaPlots> {
    p.eat(Punct::Eq);
    if at_end(p) {
        return Ok(McaPlots::default());
    }
    let mut out = McaPlots {
        charts: Vec::new(),
        ndim: None,
    };
    let mut seen: Vec<&str> = Vec::new();
    while !at_end(p) {
        let start = p.pos;
        let word = match p.peek() {
            Some(TokenKind::Word(w)) => w.to_uppercase(),
            _ => String::new(),
        };
        // CATEGORIES is written in full in places
        let name = if word == "CATEGORIES" {
            p.pos += 1;
            "CATEGORY"
        } else if let Some(name) = [
            "OBJECT", "CATEGORY", "JOINTCAT", "DISCRIM", "BIPLOT", "TRANS", "RESID", "NONE", "NDIM",
        ]
        .into_iter()
        .find(|k| p.at_keyword(k))
        {
            p.pos += 1;
            name
        } else {
            return Err(p.unexpected(&[
                "OBJECT", "CATEGORY", "JOINTCAT", "DISCRIM", "BIPLOT", "TRANS", "RESID", "NONE",
                "NDIM",
            ]));
        };
        if name == "NONE" {
            out.charts.clear();
            seen.clear();
            p.eat(Punct::Comma);
            continue;
        }
        if seen.contains(&name) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("PLOT names {name} twice"),
            ));
        }
        seen.push(name);
        let chart = match name {
            "NDIM" => {
                if p.eat(Punct::LParen) {
                    let first = whole(p, "the first dimension", 1)?;
                    p.eat(Punct::Comma);
                    let last = whole(p, "the last dimension", 2)?;
                    p.expect(Punct::RParen)?;
                    if last <= first {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            "NDIM needs the first dimension below the last",
                        ));
                    }
                    out.ndim = Some((first, last));
                }
                p.eat(Punct::Comma);
                continue;
            }
            "OBJECT" => McaPlot::Object(optional_list(p)?),
            "JOINTCAT" => McaPlot::Jointcat(optional_list(p)?),
            "DISCRIM" => McaPlot::Discrim(optional_list(p)?),
            "CATEGORY" => {
                let items = optional_list(p)?;
                if items.is_empty() {
                    return Err(p
                        .unexpected(&["'('"])
                        .hint("CATEGORY needs a list of variables"));
                }
                McaPlot::Category(items)
            }
            "BIPLOT" => {
                let mut centroids = Vec::new();
                if p.at(Punct::LParen) && p.peek_at(1) == Some(&TokenKind::Punct(Punct::LParen)) {
                    p.pos += 2;
                    centroids = p.var_list()?;
                    p.expect(Punct::RParen)?;
                    p.expect(Punct::RParen)?;
                }
                McaPlot::Biplot(centroids, optional_list(p)?)
            }
            "TRANS" => {
                out.charts
                    .push((McaPlot::Trans(dimension_list(p, "TRANS")?), None));
                p.eat(Punct::Comma);
                continue;
            }
            _ => McaPlot::Resid(dimension_list(p, "RESID")?),
        };
        let length = label_length(p)?;
        out.charts.push((chart, length));
        p.eat(Punct::Comma);
    }
    Ok(out)
}

/// ANALYSIS: lists, each with an optional `(WEIGHT=n)`.
pub(crate) fn weighted_lists(p: &mut Parser) -> PResult<Vec<(Vec<VarItem>, f64)>> {
    let lists = specified_lists(p, 1.0, |p| {
        if !p.eat_keyword("WEIGHT") {
            return Err(p.unexpected(&["WEIGHT"]));
        }
        variable_weight(p)
    })?;
    if lists.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    Ok(lists)
}

/// The value after WEIGHT, which must be positive.
pub(crate) fn variable_weight(p: &mut Parser) -> PResult<f64> {
    let start = p.pos;
    let w = assigned(p, "a variable weight")?;
    if w <= 0.0 {
        return Err(SyntaxError::new(
            p.span_from(start),
            "a variable weight must be positive",
        ));
    }
    Ok(w)
}

pub(crate) fn multiple_correspondence(p: &mut Parser) -> PResult<Command> {
    let mut analysis = Vec::new();
    let mut print_spec = McaPrint::defaults();
    let mut plot_spec = McaPlots::default();
    let (mut save_trdata, mut save_object) = (None, None);
    let (mut discrdata, mut trdata, mut object) = (None, None, None);
    let common = scaling_command(
        p,
        "MULTIPLE CORRESPONDENCE",
        &["ANALYSIS", "PRINT", "PLOT", "SAVE", "OUTFILE"],
        |p, name| {
            let start = p.pos;
            match name {
                "ANALYSIS" => analysis = weighted_lists(p)?,
                "PRINT" => print_spec = print(p)?,
                "PLOT" => plot_spec = plot(p)?,
                "SAVE" => {
                    while !at_end(p) {
                        if p.eat_keyword("TRDATA") {
                            save_trdata = Some(save_target(p, "TRA", 3)?);
                        } else if p.eat_keyword("OBJECT") {
                            save_object = Some(save_target(p, "OBSCO", 5)?);
                        } else {
                            return Err(p.unexpected(&["TRDATA", "OBJECT"]));
                        }
                        p.eat(Punct::Comma);
                    }
                }
                _ => {
                    while !at_end(p) {
                        if p.eat_keyword("DISCRDATA") {
                            discrdata = Some(outfile_target(p)?);
                        } else if p.eat_keyword("TRDATA") {
                            trdata = Some(outfile_target(p)?);
                        } else if p.eat_keyword("OBJECT") {
                            object = Some(outfile_target(p)?);
                        } else {
                            return Err(p.unexpected(&["DISCRDATA", "TRDATA", "OBJECT"]));
                        }
                        p.eat(Punct::Comma);
                    }
                    distinct_files(p, start, &[&discrdata, &trdata, &object])?;
                }
            }
            Ok(())
        },
    )?;
    if analysis.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "MULTIPLE CORRESPONDENCE needs the ANALYSIS subcommand",
        ));
    }
    Ok(Command::MultipleCorrespondence(Box::new(
        MultipleCorrespondence {
            common,
            analysis,
            print: print_spec,
            plot: plot_spec,
            save_trdata,
            save_object,
            outfile_discrdata: discrdata,
            outfile_trdata: trdata,
            outfile_object: object,
        },
    )))
}
