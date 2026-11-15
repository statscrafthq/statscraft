//! Parser of MULTIPLE CORRESPONDENCE: homogeneity analysis of categorical variables.
//!
//! Rules from the Command Syntax Reference (MULTIPLE CORRESPONDENCE), in our words:
//! - The list of variables comes first; the word VARIABLES and the slash before it may be left
//!   out. ANALYSIS is required too. Every subcommand appears at most once, in any order.
//! - ANALYSIS gives each variable a weight, `(WEIGHT=n)` after a variable or a list; 1 when
//!   none. DISCRETIZATION and MISSING put their specification in parentheses after a list as
//!   well; the specification covers the variables named since the previous parenthesis.
//! - DISCRETIZATION: GROUPING with NCAT=n (7 when left out) and DISTR=NORMAL or UNIFORM, or
//!   with EQINTV=width; RANKING; MULTIPLYING. A list without parentheses, or the subcommand on
//!   its own, means GROUPING NCAT=7 DISTR=NORMAL.
//! - MISSING: PASSIVE (the default) or ACTIVE, each with MODEIMPU (the default), EXTRACAT or
//!   RANDIMPU, or LISTWISE. ALL stands for every variable and must stand alone.
//! - SUPPLEMENTARY: OBJECT(case numbers, TO allowed) and VARIABLE(varlist).
//! - CONFIGURATION: INITIAL (the default) or FIXED, then the file in parentheses.
//! - DIMENSION: a whole number from 1 (2 by default). NORMALIZATION: VPRINCIPAL (the default,
//!   q = -1), OPRINCIPAL (q = 1), SYMMETRICAL (q = 0), INDEPENDENT, or q from -1 to 1.
//! - MAXITER: a whole number (100); CRITITER: a positive number (0.00001).
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
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// How DISCRETIZATION recodes a variable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Discretize {
    /// Into `ncat` groups (7 when None) shaped like a normal or a uniform distribution.
    Grouping {
        ncat: Option<usize>,
        normal: bool,
    },
    /// Intervals of this width from the smallest value.
    Intervals(f64),
    Ranking,
    Multiplying,
}

/// What imputes a missing value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Impute {
    Mode,
    ExtraCategory,
    Random,
}

/// How a variable's missing values are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McaMissing {
    /// Left out of the fit; the imputation only fills the correlation tables.
    Passive(Impute),
    /// Imputed before the analysis.
    Active(Impute),
    /// The case is left out.
    Listwise,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum McaNormalization {
    /// The power (1 + q)/2 of each singular value goes to the objects, (1 - q)/2 to the
    /// categories: -1 variable principal, 1 object principal, 0 symmetrical.
    Q(f64),
    /// Both in principal coordinates.
    Independent,
}

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
    pub variables: Vec<VarItem>,
    /// Lists with their variable weight.
    pub analysis: Vec<(Vec<VarItem>, f64)>,
    /// Lists with their recoding; empty without the subcommand or without a list.
    pub discretization: Vec<(Vec<VarItem>, Discretize)>,
    pub missing: Vec<(Vec<VarItem>, McaMissing)>,
    /// Ranges of case numbers, first and last.
    pub supplementary_objects: Vec<(usize, usize)>,
    pub supplementary_variables: Vec<VarItem>,
    /// FIXED (true) or INITIAL, and the file.
    pub configuration: Option<(bool, String)>,
    pub dimension: usize,
    pub normalization: McaNormalization,
    pub max_iterations: usize,
    pub criterion: f64,
    pub print: McaPrint,
    pub plot: McaPlots,
    /// SAVE: the root and the number of dimensions (None for all).
    pub save_trdata: Option<(String, Option<usize>)>,
    pub save_object: Option<(String, Option<usize>)>,
    pub outfile_discrdata: Option<String>,
    pub outfile_trdata: Option<String>,
    pub outfile_object: Option<String>,
}

const SUBCOMMANDS: [&str; 14] = [
    "VARIABLES",
    "ANALYSIS",
    "DISCRETIZATION",
    "MISSING",
    "SUPPLEMENTARY",
    "CONFIGURATION",
    "DIMENSION",
    "NORMALIZATION",
    "MAXITER",
    "CRITITER",
    "PRINT",
    "PLOT",
    "SAVE",
    "OUTFILE",
];

fn whole(p: &mut Parser, what: &str, low: usize) -> PResult<usize> {
    let start = p.pos;
    match p.number() {
        Some(n) if n.fract() == 0.0 && n >= low as f64 && n < 1e9 => Ok(n as usize),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} must be a whole number from {low}, got {n}"),
        )),
        None => Err(p.unexpected(&[what])),
    }
}

/// `= value` after a keyword inside parentheses; the equals sign may be left out.
fn assigned(p: &mut Parser, what: &str) -> PResult<f64> {
    p.eat(Punct::Eq);
    p.number().ok_or_else(|| p.unexpected(&[what]))
}

/// Lists, each followed by an optional specification in parentheses that covers the items
/// since the previous one.
fn specified_lists<T: Clone>(
    p: &mut Parser,
    default: T,
    mut spec: impl FnMut(&mut Parser) -> PResult<T>,
) -> PResult<Vec<(Vec<VarItem>, T)>> {
    let mut out = Vec::new();
    while !at_end(p) {
        let items = p.var_list()?;
        let value = match p.eat(Punct::LParen) {
            true => {
                let value = spec(p)?;
                p.expect(Punct::RParen)?;
                value
            }
            false => default.clone(),
        };
        out.push((items, value));
        p.eat(Punct::Comma);
    }
    Ok(out)
}

fn discretize(p: &mut Parser) -> PResult<Discretize> {
    if p.eat_keyword("RANKING") {
        return Ok(Discretize::Ranking);
    }
    if p.eat_keyword("MULTIPLYING") {
        return Ok(Discretize::Multiplying);
    }
    p.eat_keyword("GROUPING");
    let (mut ncat, mut normal) = (None, true);
    loop {
        if p.eat_keyword("NCAT") {
            p.eat(Punct::Eq);
            ncat = Some(whole(p, "the number of categories", 1)?);
        } else if p.eat_keyword("EQINTV") {
            let start = p.pos;
            let width = assigned(p, "the interval width")?;
            if width <= 0.0 {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "the EQINTV interval width must be positive",
                ));
            }
            return Ok(Discretize::Intervals(width));
        } else if p.eat_keyword("DISTR") {
            p.eat(Punct::Eq);
            normal = if p.eat_keyword("NORMAL") {
                true
            } else if p.eat_keyword("UNIFORM") {
                false
            } else {
                return Err(p.unexpected(&["NORMAL", "UNIFORM"]));
            };
        } else if p.at(Punct::RParen) {
            return Ok(Discretize::Grouping { ncat, normal });
        } else {
            return Err(p.unexpected(&[
                "GROUPING",
                "RANKING",
                "MULTIPLYING",
                "NCAT",
                "EQINTV",
                "DISTR",
            ]));
        }
    }
}

fn impute(p: &mut Parser) -> Impute {
    if p.eat_keyword("EXTRACAT") {
        Impute::ExtraCategory
    } else if p.eat_keyword("RANDIMPU") {
        Impute::Random
    } else {
        p.eat_keyword("MODEIMPU");
        Impute::Mode
    }
}

fn missing(p: &mut Parser) -> PResult<McaMissing> {
    if p.eat_keyword("LISTWISE") {
        return Ok(McaMissing::Listwise);
    }
    if p.eat_keyword("ACTIVE") {
        return Ok(McaMissing::Active(impute(p)));
    }
    p.eat_keyword("PASSIVE");
    let how = impute(p);
    if !p.at(Punct::RParen) {
        return Err(p.unexpected(&["PASSIVE", "ACTIVE", "LISTWISE", "MODEIMPU", "EXTRACAT"]));
    }
    Ok(McaMissing::Passive(how))
}

/// Case numbers with TO, inside the parentheses already opened.
fn object_numbers(p: &mut Parser) -> PResult<Vec<(usize, usize)>> {
    let mut out = Vec::new();
    while !p.at(Punct::RParen) {
        let first = whole(p, "a case number", 1)?;
        let start = p.pos;
        let last = match p.eat_word("TO") {
            true => whole(p, "a case number", 1)?,
            false => first,
        };
        if last < first {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("in {first} TO {last} the first case number must be the smaller"),
            ));
        }
        out.push((first, last));
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a case number"]));
    }
    Ok(out)
}

/// `(n)` with a label length; None without the parenthesis.
fn label_length(p: &mut Parser) -> PResult<Option<usize>> {
    if p.at(Punct::LParen) && matches!(p.peek_at(1), Some(TokenKind::Number(_))) {
        p.pos += 1;
        let n = whole(p, "a label length", 0)?;
        p.expect(Punct::RParen)?;
        return Ok(Some(n));
    }
    Ok(None)
}

/// `(varlist)` when a parenthesis with a name follows; empty otherwise.
fn optional_list(p: &mut Parser) -> PResult<Vec<VarItem>> {
    if p.at(Punct::LParen) && matches!(p.peek_at(1), Some(TokenKind::Word(_))) {
        p.pos += 1;
        let items = p.var_list()?;
        p.expect(Punct::RParen)?;
        return Ok(items);
    }
    Ok(Vec::new())
}

/// `(var [(n)] var ...)` for TRANS and RESID: each variable (or `a TO b`) may be followed by a
/// dimension count.
fn dimension_list(p: &mut Parser, keyword: &str) -> PResult<Vec<(VarItem, usize)>> {
    if !p.eat(Punct::LParen) {
        return Err(p
            .unexpected(&["'('"])
            .hint(format!("{keyword} needs a list of variables")));
    }
    let mut out = Vec::new();
    while matches!(p.peek(), Some(TokenKind::Word(_))) {
        let first = p.name("a variable name")?;
        let item = match p.eat_word("TO") {
            true => VarItem::Range(first, p.name("a variable name")?),
            false => VarItem::Name(first),
        };
        let dims = match p.eat(Punct::LParen) {
            true => {
                let n = whole(p, "a number of dimensions", 1)?;
                p.expect(Punct::RParen)?;
                n
            }
            false => 1,
        };
        out.push((item, dims));
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a variable name"]));
    }
    p.expect(Punct::RParen)?;
    Ok(out)
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
            let (mut label, mut items) = (None, Vec::new());
            if p.eat(Punct::LParen) {
                if p.eat(Punct::LParen) {
                    label = Some(p.name("the labelling variable")?);
                    p.expect(Punct::RParen)?;
                }
                if !p.at(Punct::RParen) {
                    items = p.var_list()?;
                }
                p.expect(Punct::RParen)?;
            }
            out.object = Some((label, items));
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

/// `(root[(n)])` after a SAVE keyword, cut to `max` characters; `default` without it.
fn save_target(p: &mut Parser, default: &str, max: usize) -> PResult<(String, Option<usize>)> {
    if !p.eat(Punct::LParen) {
        return Ok((default.to_string(), None));
    }
    let root = p.name("a root name")?;
    let dims = match p.eat(Punct::LParen) {
        true => {
            let n = whole(p, "a number of dimensions", 1)?;
            p.expect(Punct::RParen)?;
            Some(n)
        }
        false => None,
    };
    // later roots are ignored
    while matches!(p.peek(), Some(TokenKind::Word(_))) {
        p.pos += 1;
    }
    p.expect(Punct::RParen)?;
    Ok((root.chars().take(max).collect(), dims))
}

fn outfile_target(p: &mut Parser) -> PResult<String> {
    p.expect(Punct::LParen)?;
    let start = p.pos;
    if p.at(Punct::Star) {
        return Err(SyntaxError::new(
            p.span(),
            "OUTFILE cannot replace the active dataset; name a file or another dataset",
        ));
    }
    let file = p.file("a file or dataset name")?;
    if file.is_empty() {
        return Err(SyntaxError::new(
            p.span_from(start),
            "the file name is empty",
        ));
    }
    p.expect(Punct::RParen)?;
    Ok(file)
}

pub(crate) fn multiple_correspondence(p: &mut Parser) -> PResult<Command> {
    let mut command = MultipleCorrespondence {
        variables: Vec::new(),
        analysis: Vec::new(),
        discretization: Vec::new(),
        missing: Vec::new(),
        supplementary_objects: Vec::new(),
        supplementary_variables: Vec::new(),
        configuration: None,
        dimension: 2,
        normalization: McaNormalization::Q(-1.0),
        max_iterations: 100,
        criterion: 0.00001,
        print: McaPrint::defaults(),
        plot: McaPlots::default(),
        save_trdata: None,
        save_object: None,
        outfile_discrdata: None,
        outfile_trdata: None,
        outfile_object: None,
    };
    let mut seen: Vec<&str> = Vec::new();
    let mut first = true;
    loop {
        let slash = p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        if !slash && !first {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
        let start = p.pos;
        // the variable list may come first without its keyword
        let name = if first && !p.at_keyword("VARIABLES") {
            if !matches!(p.peek(), Some(TokenKind::Word(_)))
                || SUBCOMMANDS[1..].iter().any(|k| p.at_keyword(k))
                    && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq))
            {
                return Err(p.unexpected(&["VARIABLES", "a variable name"]));
            }
            "VARIABLES"
        } else {
            match SUBCOMMANDS.iter().find(|k| p.at_keyword(k)) {
                Some(name) => {
                    p.pos += 1;
                    *name
                }
                None => return Err(p.subcommand_error("MULTIPLE CORRESPONDENCE", &[])),
            }
        };
        first = false;
        if seen.contains(&name) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("{name} appears twice; every subcommand may appear once"),
            ));
        }
        seen.push(name);
        p.eat(Punct::Eq);
        match name {
            "VARIABLES" => command.variables = p.var_list()?,
            "ANALYSIS" => {
                command.analysis = specified_lists(p, 1.0, |p| {
                    if !p.eat_keyword("WEIGHT") {
                        return Err(p.unexpected(&["WEIGHT"]));
                    }
                    let start = p.pos;
                    let w = assigned(p, "a variable weight")?;
                    if w <= 0.0 {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            "a variable weight must be positive",
                        ));
                    }
                    Ok(w)
                })?;
                if command.analysis.is_empty() {
                    return Err(p.unexpected(&["a variable name"]));
                }
            }
            "DISCRETIZATION" => {
                command.discretization = specified_lists(
                    p,
                    Discretize::Grouping {
                        ncat: None,
                        normal: true,
                    },
                    discretize,
                )?
            }
            "MISSING" => {
                command.missing = specified_lists(p, McaMissing::Passive(Impute::Mode), missing)?;
                if command.missing.iter().any(|(items, _)| {
                    items.contains(&VarItem::All) && (items.len() > 1 || command.missing.len() > 1)
                }) {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "ALL on MISSING must be the only variable specification",
                    ));
                }
            }
            "SUPPLEMENTARY" => {
                while !at_end(p) {
                    if p.eat_keyword("OBJECT") {
                        p.expect(Punct::LParen)?;
                        command.supplementary_objects.extend(object_numbers(p)?);
                        p.expect(Punct::RParen)?;
                    } else if p.eat_keyword("VARIABLE") {
                        p.expect(Punct::LParen)?;
                        command.supplementary_variables.extend(p.var_list()?);
                        p.expect(Punct::RParen)?;
                    } else {
                        return Err(p.unexpected(&["OBJECT", "VARIABLE"]));
                    }
                    p.eat(Punct::Comma);
                }
            }
            "CONFIGURATION" => {
                let fixed = if p.eat_keyword("FIXED") {
                    true
                } else {
                    p.eat_keyword("INITIAL");
                    false
                };
                p.expect(Punct::LParen)?;
                let file = p.file("a file name")?;
                p.expect(Punct::RParen)?;
                command.configuration = Some((fixed, file));
            }
            "DIMENSION" => command.dimension = whole(p, "the number of dimensions", 1)?,
            "NORMALIZATION" => {
                command.normalization = if p.eat_keyword("VPRINCIPAL") {
                    McaNormalization::Q(-1.0)
                } else if p.eat_keyword("OPRINCIPAL") {
                    McaNormalization::Q(1.0)
                } else if p.eat_keyword("SYMMETRICAL") {
                    McaNormalization::Q(0.0)
                } else if p.eat_keyword("INDEPENDENT") {
                    McaNormalization::Independent
                } else {
                    let start = p.pos;
                    match p.number() {
                        Some(q) if (-1.0..=1.0).contains(&q) => McaNormalization::Q(q),
                        Some(q) => {
                            return Err(SyntaxError::new(
                                p.span_from(start),
                                format!("a NORMALIZATION value lies from -1 to 1, got {q}"),
                            ));
                        }
                        None => {
                            return Err(p.unexpected(&[
                                "VPRINCIPAL",
                                "OPRINCIPAL",
                                "SYMMETRICAL",
                                "INDEPENDENT",
                                "a value from -1 to 1",
                            ]));
                        }
                    }
                }
            }
            "MAXITER" => command.max_iterations = whole(p, "the most iterations", 0)?,
            "CRITITER" => {
                let start = p.pos;
                command.criterion = match p.number() {
                    Some(c) if c > 0.0 => c,
                    Some(_) => {
                        return Err(SyntaxError::new(
                            p.span_from(start),
                            "CRITITER must be positive",
                        ));
                    }
                    None => return Err(p.unexpected(&["the convergence criterion"])),
                }
            }
            "PRINT" => command.print = print(p)?,
            "PLOT" => command.plot = plot(p)?,
            "SAVE" => {
                while !at_end(p) {
                    if p.eat_keyword("TRDATA") {
                        command.save_trdata = Some(save_target(p, "TRA", 3)?);
                    } else if p.eat_keyword("OBJECT") {
                        command.save_object = Some(save_target(p, "OBSCO", 5)?);
                    } else {
                        return Err(p.unexpected(&["TRDATA", "OBJECT"]));
                    }
                    p.eat(Punct::Comma);
                }
            }
            _ => {
                while !at_end(p) {
                    if p.eat_keyword("DISCRDATA") {
                        command.outfile_discrdata = Some(outfile_target(p)?);
                    } else if p.eat_keyword("TRDATA") {
                        command.outfile_trdata = Some(outfile_target(p)?);
                    } else if p.eat_keyword("OBJECT") {
                        command.outfile_object = Some(outfile_target(p)?);
                    } else {
                        return Err(p.unexpected(&["DISCRDATA", "TRDATA", "OBJECT"]));
                    }
                    p.eat(Punct::Comma);
                }
                let files: Vec<String> = [
                    &command.outfile_discrdata,
                    &command.outfile_trdata,
                    &command.outfile_object,
                ]
                .into_iter()
                .flatten()
                .map(|f| f.to_lowercase())
                .collect();
                if (1..files.len()).any(|i| files[..i].contains(&files[i])) {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "every OUTFILE keyword needs its own file or dataset",
                    ));
                }
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    if command.variables.is_empty() {
        return Err(p.unexpected(&["VARIABLES"]));
    }
    if command.analysis.is_empty() {
        return Err(SyntaxError::new(
            p.span(),
            "MULTIPLE CORRESPONDENCE needs the ANALYSIS subcommand",
        ));
    }
    Ok(Command::MultipleCorrespondence(Box::new(command)))
}
