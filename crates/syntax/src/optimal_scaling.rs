//! What MULTIPLE CORRESPONDENCE and CATPCA share: the subcommands that read the data
//! (VARIABLES, DISCRETIZATION, MISSING, SUPPLEMENTARY, CONFIGURATION) and tune the fit
//! (DIMENSION, NORMALIZATION, MAXITER, CRITITER), and the small parsers of their options.
//!
//! Rules from the Command Syntax Reference, in our words:
//! - The list of variables comes first; the word VARIABLES and the slash before it may be left
//!   out. Every subcommand appears at most once, in any order.
//! - DISCRETIZATION and MISSING put their specification in parentheses after a list; it covers
//!   the variables named since the previous parenthesis.
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

use crate::ast::VarItem;
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

/// The subcommands both commands read the same way.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalingCommon {
    pub variables: Vec<VarItem>,
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
}

impl Default for ScalingCommon {
    fn default() -> Self {
        ScalingCommon {
            variables: Vec::new(),
            discretization: Vec::new(),
            missing: Vec::new(),
            supplementary_objects: Vec::new(),
            supplementary_variables: Vec::new(),
            configuration: None,
            dimension: 2,
            normalization: McaNormalization::Q(-1.0),
            max_iterations: 100,
            criterion: 0.00001,
        }
    }
}

const COMMON: [&str; 9] = [
    "DISCRETIZATION",
    "MISSING",
    "SUPPLEMENTARY",
    "CONFIGURATION",
    "DIMENSION",
    "NORMALIZATION",
    "MAXITER",
    "CRITITER",
    "VARIABLES",
];

pub(crate) fn whole(p: &mut Parser, what: &str, low: usize) -> PResult<usize> {
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
pub(crate) fn assigned(p: &mut Parser, what: &str) -> PResult<f64> {
    p.eat(Punct::Eq);
    p.number().ok_or_else(|| p.unexpected(&[what]))
}

/// Lists, each followed by an optional specification in parentheses that covers the items
/// since the previous one.
pub(crate) fn specified_lists<T: Clone>(
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
pub(crate) fn label_length(p: &mut Parser) -> PResult<Option<usize>> {
    if p.at(Punct::LParen) && matches!(p.peek_at(1), Some(TokenKind::Number(_))) {
        p.pos += 1;
        let n = whole(p, "a label length", 0)?;
        p.expect(Punct::RParen)?;
        return Ok(Some(n));
    }
    Ok(None)
}

/// `(varlist)` when a parenthesis with a name follows; empty otherwise.
pub(crate) fn optional_list(p: &mut Parser) -> PResult<Vec<VarItem>> {
    if p.at(Punct::LParen) && matches!(p.peek_at(1), Some(TokenKind::Word(_))) {
        p.pos += 1;
        let items = p.var_list()?;
        p.expect(Punct::RParen)?;
        return Ok(items);
    }
    Ok(Vec::new())
}

/// PRINT OBJECT's `[([(label)] varlist)]`: the labelling variable and the variables shown.
pub(crate) fn object_list(p: &mut Parser) -> PResult<(Option<String>, Vec<VarItem>)> {
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
    Ok((label, items))
}

/// `(var [(n)] var ...)` for TRANS and RESID: each variable (or `a TO b`) may be followed by a
/// dimension count.
pub(crate) fn dimension_list(p: &mut Parser, keyword: &str) -> PResult<Vec<(VarItem, usize)>> {
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

/// `(root[(n)])` after a SAVE keyword, cut to `max` characters; `default` without it.
pub(crate) fn save_target(
    p: &mut Parser,
    default: &str,
    max: usize,
) -> PResult<(String, Option<usize>)> {
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

/// The file or dataset in parentheses after an OUTFILE keyword.
pub(crate) fn outfile_target(p: &mut Parser) -> PResult<String> {
    p.expect(Punct::LParen)?;
    let file = outfile_name(p)?;
    p.expect(Punct::RParen)?;
    Ok(file)
}

/// A file or dataset name, never the active dataset.
pub(crate) fn outfile_name(p: &mut Parser) -> PResult<String> {
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
    Ok(file)
}

/// An error when two OUTFILE keywords name the same file or dataset.
pub(crate) fn distinct_files(p: &Parser, start: usize, files: &[&Option<String>]) -> PResult<()> {
    let files: Vec<String> = files
        .iter()
        .copied()
        .flatten()
        .map(|f| f.to_lowercase())
        .collect();
    if (1..files.len()).any(|i| files[..i].contains(&files[i])) {
        return Err(SyntaxError::new(
            p.span_from(start),
            "every OUTFILE keyword needs its own file or dataset",
        ));
    }
    Ok(())
}

/// One of the shared subcommands, the parser just after its name and equals sign.
fn common_subcommand(p: &mut Parser, name: &str, c: &mut ScalingCommon) -> PResult<()> {
    let start = p.pos;
    match name {
        "VARIABLES" => c.variables = p.var_list()?,
        "DISCRETIZATION" => {
            c.discretization = specified_lists(
                p,
                Discretize::Grouping {
                    ncat: None,
                    normal: true,
                },
                discretize,
            )?
        }
        "MISSING" => {
            c.missing = specified_lists(p, McaMissing::Passive(Impute::Mode), missing)?;
            if c.missing.iter().any(|(items, _)| {
                items.contains(&VarItem::All) && (items.len() > 1 || c.missing.len() > 1)
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
                    c.supplementary_objects.extend(object_numbers(p)?);
                    p.expect(Punct::RParen)?;
                } else if p.eat_keyword("VARIABLE") {
                    p.expect(Punct::LParen)?;
                    c.supplementary_variables.extend(p.var_list()?);
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
            c.configuration = Some((fixed, file));
        }
        "DIMENSION" => c.dimension = whole(p, "the number of dimensions", 1)?,
        "NORMALIZATION" => {
            c.normalization = if p.eat_keyword("VPRINCIPAL") {
                McaNormalization::Q(-1.0)
            } else if p.eat_keyword("OPRINCIPAL") {
                McaNormalization::Q(1.0)
            } else if p.eat_keyword("SYMMETRICAL") {
                McaNormalization::Q(0.0)
            } else if p.eat_keyword("INDEPENDENT") {
                McaNormalization::Independent
            } else {
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
        "MAXITER" => c.max_iterations = whole(p, "the most iterations", 0)?,
        _ => {
            c.criterion = match p.number() {
                Some(v) if v > 0.0 => v,
                Some(_) => {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "CRITITER must be positive",
                    ));
                }
                None => return Err(p.unexpected(&["the convergence criterion"])),
            }
        }
    }
    Ok(())
}

/// Reads a whole command: the shared subcommands into the result, the command's `own` ones
/// through `subcommand`, which gets the parser after the name and equals sign. Checks that
/// every subcommand appears once and that VARIABLES is there.
pub(crate) fn scaling_command(
    p: &mut Parser,
    who: &str,
    own: &[&'static str],
    mut subcommand: impl FnMut(&mut Parser, &'static str) -> PResult<()>,
) -> PResult<ScalingCommon> {
    let mut common = ScalingCommon::default();
    let names: Vec<&'static str> = COMMON.iter().chain(own).copied().collect();
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
                || names.iter().any(|k| p.at_keyword(k))
                    && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq))
            {
                return Err(p.unexpected(&["VARIABLES", "a variable name"]));
            }
            "VARIABLES"
        } else {
            match names.iter().find(|k| p.at_keyword(k)) {
                Some(name) => {
                    p.pos += 1;
                    *name
                }
                None => return Err(p.subcommand_error(who, &[])),
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
        match COMMON.contains(&name) {
            true => common_subcommand(p, name, &mut common)?,
            false => subcommand(p, name)?,
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    if common.variables.is_empty() {
        return Err(p.unexpected(&["VARIABLES"]));
    }
    Ok(common)
}
