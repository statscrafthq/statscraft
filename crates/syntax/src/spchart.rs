//! Parser of SPCHART: Shewhart control charts and the process capability chart.
//!
//! Rules from the Command Syntax Reference, in our own words:
//! - Exactly one chart subcommand: XR or XS (X-bar with a range or standard deviation chart,
//!   `(XBARONLY)` drops the second chart), IR or I (individuals, with or without the moving
//!   range chart), NP, P, C, U (attribute charts) or CPCHART (process capability). XR may be
//!   left out: a bare variable specification is an XR chart. The first subcommand needs no
//!   slash.
//! - Variable charts take `var BY var` (cases are units, the BY variable names the subgroup)
//!   or `var var [...] [BY var]` (cases are subgroups). IR and I take `var [BY var]`.
//!   Attribute charts take `var BY var` or `COUNT(var) N(var | n) [BY var]`. CPCHART takes
//!   `var BY var`, `var var [...]` or a single variable.
//! - SPAN (2 to 100) goes with I, IR and CPCHART; CONFORM or NONCONFORM with P and NP, and
//!   one of them is required for a status variable; NORMAL and REFERENCE with CPCHART only;
//!   CPCHART requires LSL and USL. SIGMAS is above 1 and at most 10; MINSAMPLE at least 2.
//! - STATISTICS, RULES and CAPSIGMA may not be repeated; a repeated keyword in them counts
//!   once. RULES alone means ALL. Two different CAPSIGMA keywords are an error, and so is a
//!   CAPSIGMA that does not fit the data of a CPCHART.
//! - TITLE and FOOTNOTE take one or two quoted lines, SUBTITLE one, each at most 72
//!   characters. Keywords may be shortened to three letters where that is unambiguous.

use crate::ast::{Command, Value, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::graph::lines;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpchartKind {
    Xr,
    Xs,
    Ir,
    I,
    Np,
    P,
    C,
    U,
    Capability,
}

/// The subgroup size of `N(...)`: a variable or one number for every subgroup.
#[derive(Debug, Clone, PartialEq)]
pub enum SpchartSize {
    Variable(String),
    Constant(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SpchartData {
    /// `var BY var`: each case is a unit; the BY variable names its subgroup.
    Units { process: String, by: String },
    /// `var var [...] [BY var]`: each case is a subgroup with one value per variable.
    Subgroups {
        variables: Vec<VarItem>,
        by: Option<String>,
    },
    /// `var [BY var]`: single values in case order, without subgroups.
    Individuals { process: String, by: Option<String> },
    /// `COUNT(var) N(var | n) [BY var]`: each case is a subgroup with its count and size.
    Counts {
        count: String,
        size: SpchartSize,
        by: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapSigma {
    Rbar,
    Sbar,
    Mrbar,
    Within,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpchartNormal {
    Capability,
    Performance,
    Both,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spchart {
    pub kind: SpchartKind,
    pub data: SpchartData,
    pub xbar_only: bool,
    pub title: Vec<String>,
    pub subtitle: Option<String>,
    pub footnote: Vec<String>,
    /// STATISTICS keywords in their full spelling, each once, in the order given.
    pub statistics: Vec<&'static str>,
    /// RULES keywords in their full spelling, ALL expanded; empty without RULES.
    pub rules: Vec<&'static str>,
    pub id: Option<String>,
    /// None for the chart's default.
    pub capsigma: Option<CapSigma>,
    pub span: Option<u32>,
    /// CONFORM (false) or NONCONFORM (true) with its value.
    pub conform: Option<(bool, Value)>,
    pub sigmas: f64,
    pub minsample: u32,
    pub lsl: Option<f64>,
    pub usl: Option<f64>,
    pub target: Option<f64>,
    /// MISSING=REPORT: subgroups for missing BY values.
    pub report: bool,
    /// MISSING=INCLUDE: user-missing process values count as valid.
    pub include: bool,
    pub normal: SpchartNormal,
    /// REFERENCE: lines at the specification limits (and target), and at the mean and the
    /// SIGMAS capability sigmas around it.
    pub reference_limits: bool,
    pub reference_sigmas: bool,
}

/// The STATISTICS keywords in the order of the manual's four groups.
pub const SPCHART_STATISTICS: [&str; 37] = [
    "CP", "CPL", "CPU", "K", "CPK", "CR", "CPM", "CZL", "CZU", "CZMIN", "CZMAX", "CZOUT", "CZLOUT",
    "CZUOUT", "PP", "PPL", "PPU", "PPK", "PR", "PPM", "PZL", "PZU", "PZMIN", "PZMAX", "PZOUT",
    "PZLOUT", "PZUOUT", "N", "MEAN", "STDDEV", "CAPSIGMA", "LSL", "USL", "TARGET", "AZOUT",
    "AZLOUT", "AZUOUT",
];

/// The rules in the order of the manual.
pub const SPCHART_RULES: [&str; 11] = [
    "UCL",
    "R2UPPER",
    "R4UPPER",
    "R8UPPER",
    "R8LOWER",
    "R4LOWER",
    "R2LOWER",
    "LCL",
    "TRUP",
    "TRDOWN",
    "ALTERNATING",
];

/// The STATISTICS keywords that need the capability sigma.
pub const CAPABILITY_SIGMA_STATISTICS: [&str; 14] = [
    "CP", "CPL", "CPU", "CPK", "CR", "CPM", "CZL", "CZU", "CZMIN", "CZMAX", "CZOUT", "CZLOUT",
    "CZUOUT", "CAPSIGMA",
];

const SUBCOMMANDS: [&str; 28] = [
    "TEMPLATE",
    "TITLE",
    "SUBTITLE",
    "FOOTNOTE",
    "XR",
    "XS",
    "IR",
    "I",
    "NP",
    "P",
    "C",
    "U",
    "CPCHART",
    "STATISTICS",
    "RULES",
    "ID",
    "CAPSIGMA",
    "SPAN",
    "CONFORM",
    "NONCONFORM",
    "SIGMAS",
    "MINSAMPLE",
    "LSL",
    "USL",
    "TARGET",
    "MISSING",
    "NORMAL",
    "REFERENCE",
];

/// The keyword of `list` that `word` names: written in full, or shortened to three letters or
/// more when only one keyword starts that way.
fn pick(word: &str, list: &[&'static str]) -> Option<&'static str> {
    if let Some(k) = list.iter().find(|k| k.eq_ignore_ascii_case(word)) {
        return Some(k);
    }
    let mut found = list.iter().filter(|k| {
        word.len() >= 3 && k.len() > word.len() && k[..word.len()].eq_ignore_ascii_case(word)
    });
    match (found.next(), found.next()) {
        (Some(k), None) => Some(k),
        _ => None,
    }
}

fn word<'a>(p: &Parser<'a>) -> Option<&'a str> {
    match p.peek() {
        Some(TokenKind::Word(w)) => Some(w),
        _ => None,
    }
}

/// The next word as a keyword of `list`, consumed.
fn keyword(p: &mut Parser, list: &[&'static str]) -> PResult<&'static str> {
    match word(p).and_then(|w| pick(w, list)) {
        Some(k) => {
            p.pos += 1;
            Ok(k)
        }
        None => Err(p.unexpected(list)),
    }
}

/// `[=] n` as a whole number from `low` to `high`.
fn whole(p: &mut Parser, what: &str, low: f64, high: f64) -> PResult<u32> {
    p.eat(Punct::Eq);
    let start = p.pos;
    match p.number() {
        Some(n) if n.fract() == 0.0 && n >= low && n <= high => Ok(n as u32),
        Some(n) => Err(SyntaxError::new(
            p.span_from(start),
            format!("{what} takes a whole number from {low} to {high}, got {n}"),
        )),
        None => Err(p.unexpected(&["a whole number"])),
    }
}

fn number(p: &mut Parser) -> PResult<f64> {
    p.eat(Punct::Eq);
    p.number().ok_or_else(|| p.unexpected(&["a number"]))
}

/// One name in parentheses after a keyword such as COUNT.
fn in_parens(p: &mut Parser, what: &str) -> PResult<String> {
    p.expect(Punct::LParen)?;
    let name = p.name(what)?;
    p.expect(Punct::RParen)?;
    Ok(name)
}

fn by(p: &mut Parser) -> PResult<Option<String>> {
    match p.eat_word("BY") {
        true => Ok(Some(p.name("the subgroup variable")?)),
        false => Ok(None),
    }
}

/// The variable specification of a chart subcommand, after its name.
fn chart_data(p: &mut Parser, kind: SpchartKind) -> PResult<(SpchartData, bool)> {
    p.eat(Punct::Eq);
    let start = p.pos;
    let single = |items: &[VarItem]| match items {
        [VarItem::Name(name)] => Some(name.clone()),
        _ => None,
    };
    let data = match kind {
        SpchartKind::Xr | SpchartKind::Xs | SpchartKind::Capability => {
            let items = list_until(p, &["BY"])?;
            let by = by(p)?;
            match (single(&items), by) {
                (Some(process), Some(by)) => SpchartData::Units { process, by },
                (Some(process), None) if kind == SpchartKind::Capability => {
                    SpchartData::Individuals { process, by: None }
                }
                (Some(_), None) => {
                    let message = "give a BY variable after one variable, or two variables or more";
                    return Err(SyntaxError::new(p.span_from(start), message));
                }
                (None, Some(_)) if kind == SpchartKind::Capability => {
                    let message = "CPCHART takes BY only after a single variable";
                    return Err(SyntaxError::new(p.span_from(start), message));
                }
                (None, by) => SpchartData::Subgroups {
                    variables: items,
                    by,
                },
            }
        }
        SpchartKind::Ir | SpchartKind::I => SpchartData::Individuals {
            process: p.name("the process variable")?,
            by: by(p)?,
        },
        SpchartKind::Np | SpchartKind::P | SpchartKind::C | SpchartKind::U => {
            if p.at_keyword("COUNT") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::LParen)) {
                p.pos += 1;
                let count = in_parens(p, "the count variable")?;
                if !p.eat_word("N") {
                    return Err(p.unexpected(&["N"]));
                }
                p.expect(Punct::LParen)?;
                let at = p.pos;
                let size = match p.number() {
                    Some(n) if n >= 1.0 && n.fract() == 0.0 => SpchartSize::Constant(n),
                    Some(n) => {
                        let message = format!("N takes a positive whole number, got {n}");
                        return Err(SyntaxError::new(p.span_from(at), message));
                    }
                    None => SpchartSize::Variable(p.name("the subgroup size variable")?),
                };
                p.expect(Punct::RParen)?;
                SpchartData::Counts {
                    count,
                    size,
                    by: by(p)?,
                }
            } else {
                let process = p.name("the process variable")?;
                if !p.eat_word("BY") {
                    return Err(p.unexpected(&["BY", "COUNT(var) N(var)"]));
                }
                SpchartData::Units {
                    process,
                    by: p.name("the subgroup variable")?,
                }
            }
        }
    };
    let mut xbar_only = false;
    if matches!(kind, SpchartKind::Xr | SpchartKind::Xs) && p.eat(Punct::LParen) {
        if !p.eat_keyword("XBARONLY") {
            return Err(p.unexpected(&["XBARONLY"]));
        }
        p.expect(Punct::RParen)?;
        xbar_only = true;
    }
    Ok((data, xbar_only))
}

pub(crate) fn spchart(p: &mut Parser) -> PResult<Command> {
    let mut chart: Option<(SpchartKind, SpchartData, bool)> = None;
    let mut c = Spchart {
        kind: SpchartKind::Xr,
        data: SpchartData::Individuals {
            process: String::new(),
            by: None,
        },
        xbar_only: false,
        title: Vec::new(),
        subtitle: None,
        footnote: Vec::new(),
        statistics: Vec::new(),
        rules: Vec::new(),
        id: None,
        capsigma: None,
        span: None,
        conform: None,
        sigmas: 3.0,
        minsample: 2,
        lsl: None,
        usl: None,
        target: None,
        report: false,
        include: false,
        normal: SpchartNormal::Capability,
        reference_limits: true,
        reference_sigmas: false,
    };
    let (mut statistics_given, mut rules_given, mut capsigma_given) = (false, false, false);
    let (mut normal_given, mut reference_given) = (false, false);
    let mut first = true;
    loop {
        if !first && !p.eat(Punct::Slash) {
            break;
        }
        if first {
            p.eat(Punct::Slash);
        }
        let start = p.pos;
        let name = word(p).and_then(|w| pick(w, &SUBCOMMANDS));
        // without a slash, a variable followed by BY starts the default XR specification
        let leading = std::mem::replace(&mut first, false);
        let by_next =
            matches!(p.peek_at(1), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("BY"));
        let name = match name {
            Some(name) if !(leading && by_next) => {
                p.pos += 1;
                name
            }
            _ if leading && word(p).is_some() => "XR",
            _ => return Err(p.subcommand_error("SPCHART", &[])),
        };
        let here = p.span_from(start);
        let repeated = |given: &mut bool| -> PResult<()> {
            if std::mem::replace(given, true) {
                return Err(SyntaxError::new(here, format!("{name} is given only once")));
            }
            Ok(())
        };
        match name {
            "TEMPLATE" => {
                return Err(SyntaxError::new(
                    p.span_from(start),
                    "subcommand TEMPLATE of SPCHART is not supported yet",
                ));
            }
            "TITLE" => c.title = lines(p, "TITLE", 2)?,
            "SUBTITLE" => c.subtitle = lines(p, "SUBTITLE", 1)?.pop(),
            "FOOTNOTE" => c.footnote = lines(p, "FOOTNOTE", 2)?,
            "XR" | "XS" | "IR" | "I" | "NP" | "P" | "C" | "U" | "CPCHART" => {
                if chart.is_some() {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "SPCHART takes only one chart subcommand",
                    ));
                }
                let kind = match name {
                    "XR" => SpchartKind::Xr,
                    "XS" => SpchartKind::Xs,
                    "IR" => SpchartKind::Ir,
                    "I" => SpchartKind::I,
                    "NP" => SpchartKind::Np,
                    "P" => SpchartKind::P,
                    "C" => SpchartKind::C,
                    "U" => SpchartKind::U,
                    _ => SpchartKind::Capability,
                };
                let (data, xbar_only) = chart_data(p, kind)?;
                chart = Some((kind, data, xbar_only));
            }
            "STATISTICS" => {
                repeated(&mut statistics_given)?;
                p.eat(Punct::Eq);
                while !at_end(p) {
                    let k = keyword(p, &SPCHART_STATISTICS)?;
                    if !c.statistics.contains(&k) {
                        c.statistics.push(k);
                    }
                    p.eat(Punct::Comma);
                }
            }
            "RULES" => {
                repeated(&mut rules_given)?;
                p.eat(Punct::Eq);
                let mut list = vec!["ALL"];
                list.extend(SPCHART_RULES);
                let mut all = at_end(p);
                while !at_end(p) {
                    match keyword(p, &list)? {
                        "ALL" => all = true,
                        k if !c.rules.contains(&k) => c.rules.push(k),
                        _ => {}
                    }
                    p.eat(Punct::Comma);
                }
                if all {
                    c.rules = SPCHART_RULES.to_vec();
                }
            }
            "ID" => {
                p.eat(Punct::Eq);
                c.id = Some(p.name("a variable name")?);
            }
            "CAPSIGMA" => {
                repeated(&mut capsigma_given)?;
                p.eat(Punct::Eq);
                while !at_end(p) {
                    let at = p.pos;
                    let k = match keyword(p, &["RBAR", "SBAR", "MRBAR", "WITHIN"])? {
                        "RBAR" => CapSigma::Rbar,
                        "SBAR" => CapSigma::Sbar,
                        "MRBAR" => CapSigma::Mrbar,
                        _ => CapSigma::Within,
                    };
                    if c.capsigma.is_some_and(|given| given != k) {
                        return Err(SyntaxError::new(
                            p.span_from(at),
                            "CAPSIGMA takes one estimator",
                        ));
                    }
                    c.capsigma = Some(k);
                }
            }
            "SPAN" => c.span = Some(whole(p, "SPAN", 2.0, 100.0)?),
            "CONFORM" | "NONCONFORM" => {
                p.eat(Punct::Eq);
                let Some(value) = p.value() else {
                    return Err(p.unexpected(&["a number", "a quoted value"]));
                };
                c.conform = Some((name == "NONCONFORM", value));
            }
            "SIGMAS" => {
                let at = p.pos;
                let n = number(p)?;
                if !(n > 1.0 && n <= 10.0) {
                    return Err(SyntaxError::new(
                        p.span_from(at),
                        format!("SIGMAS takes a number above 1 and at most 10, got {n}"),
                    ));
                }
                c.sigmas = n;
            }
            "MINSAMPLE" => c.minsample = whole(p, "MINSAMPLE", 2.0, 100.0)?,
            "LSL" => c.lsl = Some(number(p)?),
            "USL" => c.usl = Some(number(p)?),
            "TARGET" => c.target = Some(number(p)?),
            "MISSING" => {
                p.eat(Punct::Eq);
                while !at_end(p) {
                    match keyword(p, &["NOREPORT", "REPORT", "EXCLUDE", "INCLUDE"])? {
                        "NOREPORT" => c.report = false,
                        "REPORT" => c.report = true,
                        "EXCLUDE" => c.include = false,
                        _ => c.include = true,
                    }
                    p.eat(Punct::Comma);
                }
            }
            "NORMAL" => {
                p.eat(Punct::Eq);
                normal_given = true;
                c.normal = match keyword(p, &["CAPABILITY", "PERFORMANCE", "BOTH", "NONE"])? {
                    "CAPABILITY" => SpchartNormal::Capability,
                    "PERFORMANCE" => SpchartNormal::Performance,
                    "BOTH" => SpchartNormal::Both,
                    _ => SpchartNormal::None,
                };
            }
            _ => {
                // REFERENCE
                p.eat(Punct::Eq);
                reference_given = true;
                let (mut limits, mut sigmas, mut none) = (false, false, false);
                while !at_end(p) {
                    match keyword(p, &["SL", "SIGMAS", "NONE"])? {
                        "SL" => limits = true,
                        "SIGMAS" => sigmas = true,
                        _ => none = true,
                    }
                    p.eat(Punct::Comma);
                }
                if none && (limits || sigmas) {
                    return Err(SyntaxError::new(
                        p.span_from(start),
                        "REFERENCE=NONE cannot go with SL or SIGMAS",
                    ));
                }
                if !none && !sigmas {
                    limits = true;
                }
                (c.reference_limits, c.reference_sigmas) = (limits, sigmas);
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    let Some((kind, data, xbar_only)) = chart else {
        return Err(SyntaxError::new(
            p.span(),
            "SPCHART needs a chart subcommand: XR, XS, IR, I, NP, P, C, U or CPCHART",
        ));
    };
    (c.kind, c.data, c.xbar_only) = (kind, data, xbar_only);
    check(p, &c, normal_given || reference_given)?;
    Ok(Command::Spchart(Box::new(c)))
}

/// The rules that tie subcommands to the chart.
fn check(p: &Parser, c: &Spchart, capability_options: bool) -> PResult<()> {
    let fail = |message: &str| Err(SyntaxError::new(p.span(), message.to_string()));
    let capability = c.kind == SpchartKind::Capability;
    let individuals = matches!(c.data, SpchartData::Individuals { .. });
    let span_fits =
        matches!(c.kind, SpchartKind::I | SpchartKind::Ir) || (capability && individuals);
    if c.span.is_some() && !span_fits {
        return fail("SPAN goes with IR, I, or CPCHART of a single variable");
    }
    let attribute = matches!(c.kind, SpchartKind::P | SpchartKind::Np);
    if c.conform.is_some() && !attribute {
        return fail("CONFORM and NONCONFORM go with P and NP only");
    }
    if attribute && matches!(c.data, SpchartData::Units { .. }) && c.conform.is_none() {
        return fail("a status variable needs CONFORM or NONCONFORM to tell what counts");
    }
    if capability_options && !capability {
        return fail("NORMAL and REFERENCE go with CPCHART only");
    }
    if capability && (c.lsl.is_none() || c.usl.is_none()) {
        return fail("CPCHART needs both LSL and USL");
    }
    if let (Some(lsl), Some(usl)) = (c.lsl, c.usl)
        && lsl >= usl
    {
        return fail("LSL must be below USL");
    }
    if capability && let Some(sigma) = c.capsigma {
        let fits = match sigma {
            CapSigma::Mrbar => individuals,
            _ => !individuals,
        };
        if !fits {
            return fail(match sigma {
                CapSigma::Mrbar => "CAPSIGMA=MRBAR needs CPCHART of a single variable",
                _ => "this CAPSIGMA needs subgroups: CPCHART var BY var or var var ...",
            });
        }
    }
    let valid = match (c.kind, c.capsigma) {
        (_, None) => true,
        (SpchartKind::Xr | SpchartKind::Xs, Some(s)) => s != CapSigma::Mrbar,
        (SpchartKind::I | SpchartKind::Ir, Some(s)) => s == CapSigma::Mrbar,
        _ => true,
    };
    let applicable = c
        .statistics
        .iter()
        .any(|k| CAPABILITY_SIGMA_STATISTICS.contains(k));
    if !valid && applicable {
        return fail(match c.kind {
            SpchartKind::I | SpchartKind::Ir => "IR and I take CAPSIGMA=MRBAR only",
            _ => "XR and XS take CAPSIGMA=RBAR, SBAR or WITHIN",
        });
    }
    Ok(())
}

pub(crate) const SPCHART_SYNTAX: &str = "SPCHART [/TITLE='line' ['line']] [/SUBTITLE='line'] [/FOOTNOTE='line' ['line']]\n\
                 \x20  /XR | /XS = var BY group | var var [var ...] [BY label] [(XBARONLY)]\n\
                 \x20  | /IR | /I = var [BY label]\n\
                 \x20  | /P | /NP = status BY group | COUNT(var) N(var | n) [BY label]\n\
                 \x20  | /C | /U = defects BY group | COUNT(var) N(var | n) [BY label]\n\
                 \x20  | /CPCHART = var BY group | var var [var ...] | var\n\
                 \x20  [/STATISTICS=keywords] [/RULES=[ALL] [UCL] [R2UPPER] [R4UPPER] [R8UPPER] [R8LOWER]\n\
                 \x20            [R4LOWER] [R2LOWER] [LCL] [TRUP] [TRDOWN] [ALTERNATING]] [/ID=var]\n\
                 \x20  [/CAPSIGMA=RBAR | SBAR | MRBAR | WITHIN] [/SPAN=2] [/CONFORM | /NONCONFORM=value]\n\
                 \x20  [/SIGMAS=3] [/MINSAMPLE=2] [/LSL=value] [/USL=value] [/TARGET=value]\n\
                 \x20  [/MISSING=[NOREPORT | REPORT] [EXCLUDE | INCLUDE]]\n\
                 \x20  [/NORMAL=CAPABILITY | PERFORMANCE | BOTH | NONE] [/REFERENCE=[SL] [SIGMAS] | NONE].\n\
                 One chart subcommand; XR may be left out. Each control chart is a line chart of the process\n\
                 with its center line and the control limits at SIGMAS sigmas (lower limits of ranges, standard\n\
                 deviations and counts stop at 0, a p chart's upper one at 1, an np chart's at the subgroup\n\
                 size); a series that is the same everywhere carries its value in its name. LSL and USL add\n\
                 fixed limit lines to the first chart. var BY group: cases are units, gathered into subgroups\n\
                 by the group variable in ascending order; var var ...: each case is a subgroup, one unit per\n\
                 variable. XR and XS plot subgroup means with sigma from the mean of R/d2(n) or S/c4(n), then\n\
                 the ranges or standard deviations (left out with XBARONLY); subgroups below MINSAMPLE units are\n\
                 dropped. IR and I plot each value with sigma from the mean moving range over SPAN values over\n\
                 d2(SPAN); IR adds the moving ranges, missing for the first SPAN-1 points. P plots the share\n\
                 nonconforming against the pooled share, NP the number; CONFORM names the one good value of the\n\
                 status (any other value is a defect), NONCONFORM the one bad value. C plots defects per\n\
                 subgroup, U defects per unit. COUNT and N read counts and sizes from each case; a count above\n\
                 its size drops a p or np subgroup. Unequal sizes give each subgroup its own limits, with a\n\
                 warning for np and c. BY label only names the points, else they are numbered by case.\n\
                 RULES (ALL when empty) checks the first chart: beyond a control limit, 2 of 3 past 2 sigma, 4\n\
                 of 5 past 1 sigma, 8 on one side of the center, 6 rising or falling, 14 going up and down in\n\
                 turn; only the last point of a pattern counts. The Rule Violations table names the points by\n\
                 ID, else by the BY value. CPCHART draws a histogram with the normal curve of the mean and the\n\
                 capability sigma (NORMAL: PERFORMANCE uses the standard deviation, NONE none, BOTH draws the\n\
                 capability curve); LSL and USL are required, REFERENCE values go to the footnote.\n\
                 STATISTICS (XR, XS, IR, I and CPCHART): CP CPL CPU K CPK CR CPM CZL CZU CZMIN CZMAX CZOUT\n\
                 CZLOUT CZUOUT with the capability sigma, PP PPL PPU PPK PR PPM PZL PZU PZMIN PZMAX PZOUT\n\
                 PZLOUT PZUOUT with the standard deviation of all values, N MEAN STDDEV CAPSIGMA LSL USL\n\
                 TARGET, and AZOUT AZLOUT AZUOUT, the observed percentages at or past the limits. Keywords\n\
                 that need LSL and USL, or TARGET between them, are left out with a warning. CAPSIGMA: RBAR\n\
                 (XR), SBAR (XS) or WITHIN for subgroups, MRBAR for single values. CPCHART without STATISTICS\n\
                 lists its indices in a table. MISSING: REPORT keeps subgroups for missing BY values, INCLUDE\n\
                 counts user-missing process values. Weights count only for units, whole positive ones; each\n\
                 unit of an individuals chart is repeated by its weight. A chart has 2 to 3000 points.\n\
                 Not supported yet: TEMPLATE.";
