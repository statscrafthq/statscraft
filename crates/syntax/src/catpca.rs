//! Parser of CATPCA: principal components of variables quantified by optimal scaling.
//!
//! Rules from the Command Syntax Reference (CATPCA), in our words; the subcommands it shares
//! with MULTIPLE CORRESPONDENCE are read in `optimal_scaling`:
//! - ANALYSIS is required. After a variable or a list, parentheses may give WEIGHT=n (1), and
//!   LEVEL= SPORD (spline ordinal, the default), SPNOM (spline nominal), ORDI, NOMI, MNOM
//!   (multiple nominal) or NUME. The two spline levels take DEGREE=n (2) and INKNOT=n interior
//!   knots (2, from 0). The parentheses cover the variables since the previous ones.
//! - ROTATION: NOROTATE (the default), VARIMAX, EQUAMAX, QUARTIMAX, PROMAX[(kappa)] with kappa
//!   from 1 below 10000 (4), OBLIMIN[(delta)] with delta at most 0.8 (0); KAISER (the
//!   default) or NOKAISER. RESAMPLE: NONE or BOOTSTRAP[(samples, percent, BALANCED or
//!   UNBALANCED, PROCRU or REFLEC)], with 1000, 95, BALANCED and PROCRU by default; a single
//!   number is the samples. ROTATION and RESAMPLE may not both appear.
//! - PRINT: DESCRIP[(varlist)], LOADING[(SORT | NOSORT)], CORR, VAF, OCORR, QUANT[(varlist)],
//!   HISTORY, OBJECT[([(label)] varlist)] and NONE. Without the subcommand or without
//!   keywords: DESCRIP LOADING CORR. A keyword written twice counts once (the last one), NONE
//!   drops the ones before it, and a keyword with a list written twice is an error.
//! - PLOT: OBJECT[(varlist)], LOADING[(varlist[(CENTR[(varlist)])])], CATEGORY(varlist),
//!   JOINTCAT[(varlist)], TRANS(varlist[(n)]), RESID(varlist[(n)]),
//!   BIPLOT[(LOADING | CENTR[(varlist)])][(varlist)], TRIPLOT[(varlist[(varlist)])],
//!   PROJCENTR(var, varlist) and VAF, each ending in an optional label length (n); NDIM(a, b);
//!   NONE; and the confidence ellipse keywords LDELLAREA, OBELLAREA, CTELLAREA and NELLPNT.
//!   Without the subcommand or without keywords: OBJECT and LOADING. A keyword written twice
//!   counts once (the last one); NONE drops the ones before it.
//! - SAVE: TRDATA[(root[(n)])], OBJECT[(root[(n)])], APPROX[(root)] and the ellipse areas
//!   LDELLAREA, OBELLAREA, CTELLAREA. Roots are cut to 5 characters.
//! - OUTFILE: DISCRDATA, TRDATA, OBJECT, APPROX and ELLCOORD, each with a file or dataset in
//!   parentheses; a file alone means TRDATA.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::factor::Rotation;
use crate::lexer::{Punct, TokenKind};
use crate::multiple_correspondence::variable_weight;
use crate::optimal_scaling::{
    ScalingCommon, dimension_list, distinct_files, label_length, object_list, optional_list,
    outfile_name, outfile_target, save_target, scaling_command, specified_lists, whole,
};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// The optimal scaling level of a variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingLevel {
    SplineOrdinal { degree: usize, inknot: usize },
    SplineNominal { degree: usize, inknot: usize },
    Ordinal,
    Nominal,
    MultipleNominal,
    Numeric,
}

/// What ANALYSIS says about a variable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatpcaVariable {
    pub weight: f64,
    pub level: ScalingLevel,
}

impl Default for CatpcaVariable {
    fn default() -> Self {
        CatpcaVariable {
            weight: 1.0,
            level: ScalingLevel::SplineOrdinal {
                degree: 2,
                inknot: 2,
            },
        }
    }
}

/// RESAMPLE=BOOTSTRAP and its parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CatpcaBootstrap {
    pub samples: usize,
    pub percent: f64,
    pub balanced: bool,
    /// Procrustes rotation (true) or reflection.
    pub procrustes: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CatpcaPrint {
    /// Some(list) when asked for; an empty list means the analysis variables.
    pub descrip: Option<Vec<VarItem>>,
    /// Some(sorted) when asked for.
    pub loading: Option<bool>,
    pub corr: bool,
    pub vaf: bool,
    pub ocorr: bool,
    pub quant: Option<Vec<VarItem>>,
    pub history: bool,
    /// The labelling variable and the variables shown next to the scores.
    pub object: Option<(Option<String>, Vec<VarItem>)>,
}

impl CatpcaPrint {
    pub fn defaults() -> Self {
        CatpcaPrint {
            descrip: Some(Vec::new()),
            loading: Some(false),
            corr: true,
            ..CatpcaPrint::default()
        }
    }
}

/// One requested chart kind with its variables.
#[derive(Debug, Clone, PartialEq)]
pub enum CatpcaPlot {
    /// The variables whose categories label the objects; empty for case numbers.
    Object(Vec<VarItem>),
    /// The vector variables (empty: all) and the multiple nominal variables whose centroids
    /// join them (None: no centroids; empty: all).
    Loading(Vec<VarItem>, Option<Vec<VarItem>>),
    Category(Vec<VarItem>),
    /// Empty for every analysis variable.
    Jointcat(Vec<VarItem>),
    /// Each list with the number of dimensions to draw for a multiple nominal variable.
    Trans(Vec<(VarItem, usize)>),
    Resid(Vec<(VarItem, usize)>),
    /// Centroids (true) or loadings for the variables (empty: all), objects labelled by each
    /// of the last list (empty: case numbers).
    Biplot {
        centroids: bool,
        variables: Vec<VarItem>,
        labels: Vec<VarItem>,
    },
    /// The variables (empty: all) and the labelling variables.
    Triplot(Vec<VarItem>, Vec<VarItem>),
    /// The variable whose centroids are projected and the variables they go onto.
    Projcentr(String, Vec<VarItem>),
    Vaf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CatpcaPlots {
    /// Each chart with its label length (None for labels in full).
    pub charts: Vec<(CatpcaPlot, Option<usize>)>,
    pub ndim: Option<(usize, usize)>,
}

impl Default for CatpcaPlots {
    fn default() -> Self {
        CatpcaPlots {
            charts: vec![
                (CatpcaPlot::Object(Vec::new()), None),
                (CatpcaPlot::Loading(Vec::new(), None), None),
            ],
            ndim: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Catpca {
    /// VARIABLES, DISCRETIZATION, MISSING, SUPPLEMENTARY, CONFIGURATION, DIMENSION,
    /// NORMALIZATION, MAXITER and CRITITER.
    pub common: ScalingCommon,
    pub analysis: Vec<(Vec<VarItem>, CatpcaVariable)>,
    /// The rotation and whether Kaiser normalization applies; None without the subcommand.
    pub rotation: Option<(Rotation, bool)>,
    pub resample: Option<CatpcaBootstrap>,
    pub print: CatpcaPrint,
    pub plot: CatpcaPlots,
    /// SAVE: the root and the number of dimensions (None for all).
    pub save_trdata: Option<(String, Option<usize>)>,
    pub save_object: Option<(String, Option<usize>)>,
    pub save_approx: Option<String>,
    /// The confidence ellipse keywords named on PLOT, SAVE or OUTFILE, which only
    /// bootstrapping gives.
    pub ellipses: Vec<String>,
    pub outfile_discrdata: Option<String>,
    pub outfile_trdata: Option<String>,
    pub outfile_object: Option<String>,
    pub outfile_approx: Option<String>,
    pub outfile_ellcoord: Option<String>,
}

fn analysis_spec(p: &mut Parser) -> PResult<CatpcaVariable> {
    let mut out = CatpcaVariable::default();
    let (mut degree, mut inknot) = (None, None);
    let mut level = None;
    let start = p.pos;
    while !p.at(Punct::RParen) {
        if p.eat_keyword("WEIGHT") {
            out.weight = variable_weight(p)?;
        } else if p.eat_keyword("LEVEL") {
            p.eat(Punct::Eq);
            let found = [
                ("SPORD", 0),
                ("SPNOM", 1),
                ("ORDI", 2),
                ("NOMI", 3),
                ("MNOM", 4),
                ("NUME", 5),
            ]
            .into_iter()
            .find(|(k, _)| p.at_keyword(k));
            let Some((_, k)) = found else {
                return Err(p.unexpected(&["SPORD", "SPNOM", "ORDI", "NOMI", "MNOM", "NUME"]));
            };
            p.pos += 1;
            level = Some(k);
        } else if p.eat_keyword("DEGREE") {
            p.eat(Punct::Eq);
            degree = Some(whole(p, "the degree", 1)?);
        } else if p.eat_keyword("INKNOT") {
            p.eat(Punct::Eq);
            inknot = Some(whole(p, "the number of interior knots", 0)?);
        } else {
            return Err(p.unexpected(&["WEIGHT", "LEVEL", "DEGREE", "INKNOT"]));
        }
        p.eat(Punct::Comma);
    }
    let (degree_v, inknot_v) = (degree.unwrap_or(2), inknot.unwrap_or(2));
    out.level = match level.unwrap_or(0) {
        0 => ScalingLevel::SplineOrdinal {
            degree: degree_v,
            inknot: inknot_v,
        },
        1 => ScalingLevel::SplineNominal {
            degree: degree_v,
            inknot: inknot_v,
        },
        k => {
            if degree.is_some() || inknot.is_some() {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "DEGREE and INKNOT go only with LEVEL=SPORD or SPNOM",
                ));
            }
            match k {
                2 => ScalingLevel::Ordinal,
                3 => ScalingLevel::Nominal,
                4 => ScalingLevel::MultipleNominal,
                _ => ScalingLevel::Numeric,
            }
        }
    };
    Ok(out)
}

fn rotation(p: &mut Parser) -> PResult<(Rotation, bool)> {
    let (mut method, mut kaiser) = (Rotation::None, true);
    while !at_end(p) {
        let start = p.pos;
        if p.eat_keyword("NOROTATE") {
            method = Rotation::None;
        } else if p.eat_keyword("VARIMAX") {
            method = Rotation::Varimax;
        } else if p.eat_keyword("EQUAMAX") {
            method = Rotation::Equamax;
        } else if p.eat_keyword("QUARTIMAX") {
            method = Rotation::Quartimax;
        } else if p.eat_keyword("PROMAX") {
            let kappa = parameter(p)?.unwrap_or(4.0);
            if !(1.0..10000.0).contains(&kappa) {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "the PROMAX kappa lies from 1 to below 10000",
                ));
            }
            method = Rotation::Promax(kappa);
        } else if p.eat_keyword("OBLIMIN") {
            let delta = parameter(p)?.unwrap_or(0.0);
            if delta > 0.8 {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "the OBLIMIN delta is at most 0.8",
                ));
            }
            method = Rotation::Oblimin(delta);
        } else if p.eat_keyword("KAISER") {
            kaiser = true;
        } else if p.eat_keyword("NOKAISER") {
            kaiser = false;
        } else {
            return Err(p.unexpected(&[
                "NOROTATE",
                "VARIMAX",
                "EQUAMAX",
                "QUARTIMAX",
                "PROMAX",
                "OBLIMIN",
                "KAISER",
                "NOKAISER",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok((method, kaiser))
}

/// `(value)` after a rotation keyword; None without it.
fn parameter(p: &mut Parser) -> PResult<Option<f64>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    let value = p.number().ok_or_else(|| p.unexpected(&["a number"]))?;
    p.expect(Punct::RParen)?;
    Ok(Some(value))
}

fn resample(p: &mut Parser) -> PResult<Option<CatpcaBootstrap>> {
    if p.eat_keyword("NONE") || at_end(p) {
        return Ok(None);
    }
    if !p.eat_keyword("BOOTSTRAP") {
        return Err(p.unexpected(&["NONE", "BOOTSTRAP"]));
    }
    let mut out = CatpcaBootstrap {
        samples: 1000,
        percent: 95.0,
        balanced: true,
        procrustes: true,
    };
    if p.eat(Punct::LParen) {
        let mut numbers = Vec::new();
        while !p.at(Punct::RParen) {
            let start = p.pos;
            if let Some(x) = p.number() {
                if numbers.len() == 2 {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "BOOTSTRAP takes at most two numbers",
                    ));
                }
                numbers.push((x, start));
            } else if p.eat_keyword("BALANCED") {
                out.balanced = true;
            } else if p.eat_keyword("UNBALANCED") {
                out.balanced = false;
            } else if p.eat_keyword("PROCRU") {
                out.procrustes = true;
            } else if p.eat_keyword("REFLEC") {
                out.procrustes = false;
            } else {
                return Err(p.unexpected(&[
                    "a number",
                    "BALANCED",
                    "UNBALANCED",
                    "PROCRU",
                    "REFLEC",
                ]));
            }
            p.eat(Punct::Comma);
        }
        p.expect(Punct::RParen)?;
        if let Some(&(n, start)) = numbers.first() {
            if n.fract() != 0.0 || n < 1.0 {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "the number of bootstrap samples must be a positive whole number",
                ));
            }
            out.samples = n as usize;
        }
        if let Some(&(c, start)) = numbers.get(1) {
            if !(c > 0.0 && c < 100.0) {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "the confidence level lies above 0 and below 100",
                ));
            }
            out.percent = c;
        }
    }
    Ok(Some(out))
}

fn print(p: &mut Parser) -> PResult<CatpcaPrint> {
    if at_end(p) {
        return Ok(CatpcaPrint::defaults());
    }
    let mut out = CatpcaPrint::default();
    let mut seen: Vec<&str> = Vec::new();
    while !at_end(p) {
        let start = p.pos;
        let mut once = |p: &Parser, name: &'static str| {
            if seen.contains(&name) {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    format!("PRINT names {name} twice; a keyword with a list may appear once"),
                ));
            }
            seen.push(name);
            Ok(())
        };
        if p.eat_keyword("CORR") {
            out.corr = true;
        } else if p.eat_keyword("VAF") {
            out.vaf = true;
        } else if p.eat_keyword("HISTORY") {
            out.history = true;
        } else if p.eat_keyword("OCORR") {
            out.ocorr = true;
        } else if p.eat_keyword("NONE") {
            out = CatpcaPrint::default();
            seen.clear();
        } else if p.eat_keyword("LOADING") {
            let mut sorted = false;
            if p.eat(Punct::LParen) {
                if p.eat_keyword("SORT") {
                    sorted = true;
                } else if !p.eat_keyword("NOSORT") {
                    return Err(p.unexpected(&["SORT", "NOSORT"]));
                }
                p.expect(Punct::RParen)?;
            }
            out.loading = Some(sorted);
        } else if p.eat_keyword("DESCRIP") {
            once(p, "DESCRIP")?;
            out.descrip = Some(optional_list(p)?);
        } else if p.eat_keyword("QUANT") {
            once(p, "QUANT")?;
            out.quant = Some(optional_list(p)?);
        } else if p.eat_keyword("OBJECT") {
            once(p, "OBJECT")?;
            out.object = Some(object_list(p)?);
        } else {
            return Err(p.unexpected(&[
                "DESCRIP", "LOADING", "CORR", "VAF", "OCORR", "QUANT", "HISTORY", "OBJECT", "NONE",
            ]));
        }
        p.eat(Punct::Comma);
    }
    Ok(out)
}

/// Skips an optional parenthesized specification of a confidence ellipse keyword.
fn skip_parenthesis(p: &mut Parser) -> PResult<()> {
    if !p.eat(Punct::LParen) {
        return Ok(());
    }
    while !p.eat(Punct::RParen) {
        if p.peek().is_none() || p.at(Punct::Slash) || p.at(Punct::LParen) {
            return Err(p.unexpected(&["')'"]));
        }
        p.pos += 1;
    }
    Ok(())
}

/// A word that names a keyword, not a variable: `(CENTR`.
fn at_paren_keyword(p: &Parser, keyword: &str) -> bool {
    p.at(Punct::LParen)
        && matches!(p.peek_at(1), Some(TokenKind::Word(w))
            if w.len() >= 3 && keyword.starts_with(&w.to_uppercase()))
}

/// LOADING's `[(varlist [(CENTR[(varlist)])])]`.
fn loading_plot(p: &mut Parser) -> PResult<CatpcaPlot> {
    let (mut vars, mut centr) = (Vec::new(), None);
    let opens = p.at(Punct::LParen)
        && matches!(
            p.peek_at(1),
            Some(TokenKind::Word(_)) | Some(TokenKind::Punct(Punct::LParen))
        );
    if !opens {
        return Ok(CatpcaPlot::Loading(vars, centr));
    }
    p.pos += 1;
    // `CENTR` may also follow the opening parenthesis directly, without its own parentheses
    let bare = p.at_keyword("CENTR");
    if !bare && !p.at(Punct::LParen) {
        vars = p.var_list()?;
    }
    let inner = !bare && p.eat(Punct::LParen);
    if bare || inner {
        if !p.eat_keyword("CENTR") {
            return Err(p.unexpected(&["CENTR"]));
        }
        centr = Some(optional_list(p)?);
        if inner {
            p.expect(Punct::RParen)?;
        }
    }
    p.expect(Punct::RParen)?;
    Ok(CatpcaPlot::Loading(vars, centr))
}

fn plot(p: &mut Parser, ellipses: &mut Vec<String>) -> PResult<CatpcaPlots> {
    if at_end(p) {
        return Ok(CatpcaPlots::default());
    }
    let mut out = CatpcaPlots {
        charts: Vec::new(),
        ndim: None,
    };
    const KEYWORDS: [&str; 17] = [
        "OBJECT",
        "LOADING",
        "CATEGORY",
        "JOINTCAT",
        "TRANS",
        "RESID",
        "BIPLOT",
        "TRIPLOT",
        "PROJCENTR",
        "VAF",
        "NDIM",
        "NONE",
        "LDELLAREA",
        "OBELLAREA",
        "CTELLAREA",
        "NELLPNT",
        "CATEGORIES",
    ];
    while !at_end(p) {
        let start = p.pos;
        let Some(name) = KEYWORDS.into_iter().find(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(&KEYWORDS[..16]));
        };
        p.pos += 1;
        let chart = match name {
            "NONE" => {
                out.charts.clear();
                p.eat(Punct::Comma);
                continue;
            }
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
            "LDELLAREA" | "OBELLAREA" | "CTELLAREA" | "NELLPNT" => {
                skip_parenthesis(p)?;
                ellipses.push(format!("PLOT {name}"));
                p.eat(Punct::Comma);
                continue;
            }
            "OBJECT" => CatpcaPlot::Object(optional_list(p)?),
            "LOADING" => loading_plot(p)?,
            "CATEGORY" | "CATEGORIES" => {
                let items = optional_list(p)?;
                if items.is_empty() {
                    return Err(p
                        .unexpected(&["'('"])
                        .hint("CATEGORY needs a list of variables"));
                }
                CatpcaPlot::Category(items)
            }
            "JOINTCAT" => CatpcaPlot::Jointcat(optional_list(p)?),
            "TRANS" => CatpcaPlot::Trans(dimension_list(p, "TRANS")?),
            "RESID" => CatpcaPlot::Resid(dimension_list(p, "RESID")?),
            "BIPLOT" => {
                let (mut centroids, mut variables) = (false, Vec::new());
                if at_paren_keyword(p, "LOADING") || at_paren_keyword(p, "CENTR") {
                    p.pos += 1;
                    centroids = p.eat_keyword("CENTR");
                    if !centroids {
                        p.pos += 1;
                    }
                    variables = optional_list(p)?;
                    p.expect(Punct::RParen)?;
                }
                CatpcaPlot::Biplot {
                    centroids,
                    variables,
                    labels: optional_list(p)?,
                }
            }
            "TRIPLOT" => {
                let (mut variables, mut labels) = (Vec::new(), Vec::new());
                if p.at(Punct::LParen) && matches!(p.peek_at(1), Some(TokenKind::Word(_))) {
                    p.pos += 1;
                    variables = p.var_list()?;
                    labels = optional_list(p)?;
                    p.expect(Punct::RParen)?;
                }
                CatpcaPlot::Triplot(variables, labels)
            }
            "PROJCENTR" => {
                p.expect(Punct::LParen)?;
                let from = p.name("the variable whose centroids are projected")?;
                p.eat(Punct::Comma);
                let onto = p.var_list()?;
                p.expect(Punct::RParen)?;
                CatpcaPlot::Projcentr(from, onto)
            }
            _ => CatpcaPlot::Vaf,
        };
        let length = label_length(p)?;
        // a keyword written again replaces the earlier one
        out.charts
            .retain(|(c, _)| std::mem::discriminant(c) != std::mem::discriminant(&chart));
        out.charts.push((chart, length));
        p.eat(Punct::Comma);
    }
    Ok(out)
}

pub(crate) fn catpca(p: &mut Parser) -> PResult<Command> {
    let mut analysis = Vec::new();
    let mut rotation_spec = None;
    let mut resample_spec = None;
    let (mut seen_rotation, mut seen_resample) = (false, false);
    let mut print_spec = CatpcaPrint::defaults();
    let mut plot_spec = CatpcaPlots::default();
    let (mut save_trdata, mut save_object, mut save_approx) = (None, None, None);
    let mut ellipses = Vec::new();
    let (mut discrdata, mut trdata, mut object, mut approx, mut ellcoord) =
        (None, None, None, None, None);
    let common = scaling_command(
        p,
        "CATPCA",
        &[
            "ANALYSIS", "ROTATION", "RESAMPLE", "PRINT", "PLOT", "SAVE", "OUTFILE",
        ],
        |p, name| {
            let start = p.pos;
            match name {
                "ANALYSIS" => {
                    analysis = specified_lists(p, CatpcaVariable::default(), analysis_spec)?;
                    if analysis.is_empty() {
                        return Err(p.unexpected(&["a variable name"]));
                    }
                }
                "ROTATION" => {
                    seen_rotation = true;
                    rotation_spec = Some(rotation(p)?);
                }
                "RESAMPLE" => {
                    seen_resample = true;
                    resample_spec = resample(p)?;
                }
                "PRINT" => print_spec = print(p)?,
                "PLOT" => plot_spec = plot(p, &mut ellipses)?,
                "SAVE" => {
                    while !at_end(p) {
                        if p.eat_keyword("TRDATA") {
                            save_trdata = Some(save_target(p, "TRA", 5)?);
                        } else if p.eat_keyword("OBJECT") {
                            save_object = Some(save_target(p, "OBSCO", 5)?);
                        } else if p.eat_keyword("APPROX") {
                            save_approx = Some(save_target(p, "APP", 5)?.0);
                        } else if let Some(k) =
                            ["LDELLAREA", "OBELLAREA", "CTELLAREA", "ELLAREAOBJ"]
                                .into_iter()
                                .find(|k| p.at_keyword(k))
                        {
                            p.pos += 1;
                            ellipses.push(format!("SAVE {k}"));
                        } else {
                            return Err(p.unexpected(&[
                                "TRDATA",
                                "OBJECT",
                                "APPROX",
                                "LDELLAREA",
                                "OBELLAREA",
                                "CTELLAREA",
                            ]));
                        }
                        p.eat(Punct::Comma);
                    }
                }
                _ => {
                    // a file alone is the transformed data
                    if matches!(p.peek(), Some(TokenKind::Str(_))) {
                        trdata = Some(outfile_name(p)?);
                    } else if p.at(Punct::LParen) {
                        trdata = Some(outfile_target(p)?);
                    }
                    while !at_end(p) {
                        if p.eat_keyword("DISCRDATA") {
                            discrdata = Some(outfile_target(p)?);
                        } else if p.eat_keyword("TRDATA") {
                            trdata = Some(outfile_target(p)?);
                        } else if p.eat_keyword("OBJECT") {
                            object = Some(outfile_target(p)?);
                        } else if p.eat_keyword("APPROX") {
                            approx = Some(outfile_target(p)?);
                        } else if p.eat_keyword("ELLCOORD") {
                            ellcoord = Some(outfile_target(p)?);
                            ellipses.push("OUTFILE ELLCOORD".into());
                        } else {
                            return Err(p.unexpected(&[
                                "DISCRDATA",
                                "TRDATA",
                                "OBJECT",
                                "APPROX",
                                "ELLCOORD",
                            ]));
                        }
                        p.eat(Punct::Comma);
                    }
                    distinct_files(
                        p,
                        start,
                        &[&discrdata, &trdata, &object, &approx, &ellcoord],
                    )?;
                }
            }
            Ok(())
        },
    )?;
    if analysis.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "CATPCA needs the ANALYSIS subcommand",
        ));
    }
    if seen_rotation && seen_resample {
        return Err(SyntaxError::new(
            p.span(),
            "CATPCA cannot have both ROTATION and RESAMPLE",
        ));
    }
    Ok(Command::Catpca(Box::new(Catpca {
        common,
        analysis,
        rotation: rotation_spec,
        resample: resample_spec,
        print: print_spec,
        plot: plot_spec,
        save_trdata,
        save_object,
        save_approx,
        ellipses,
        outfile_discrdata: discrdata,
        outfile_trdata: trdata,
        outfile_object: object,
        outfile_approx: approx,
        outfile_ellcoord: ellcoord,
    })))
}
