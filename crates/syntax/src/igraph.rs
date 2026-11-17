//! Parser of IGRAPH, the older chart command: variables assigned to axes and legends, and chart
//! elements that summarize them.
//!
//! Rules taken from the manual:
//! - Subcommands come in any order; one given twice counts the last time, except CATORDER (one
//!   per variable) and the elements, which add up.
//! - X1, Y and X2 take VAR(name) or, for Y and SUMMARYVAR, one of $COUNT, $PCT and $CASE, with
//!   TYPE = SCALE [(MIN=n MAX=n)] | CATEGORICAL and TITLE='text'. COLOR, STYLE and SIZE add
//!   LEGEND = ON | OFF and CLUSTER | STACK.
//! - The elements: BAR, LINE, AREA and PIE take a summary function in parentheses; BOX, ERRORBAR
//!   (CI(level), SD(n) or SE(n)), HISTOGRAM, SCATTER and FITLINE have keywords of their own.
//! - Keywords that only change the look of the chart are accepted and listed in `ignored`, so the
//!   run can name them in one warning. REFLINE, SPIKE, a local regression fit and the intervals
//!   of a fit line cannot be drawn and give an error.

use crate::error::SyntaxError;
use crate::graph::{GraphFunction as F, GraphInterval};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::table::matches_keyword;

/// What an axis or a legend holds: a variable or one of the built-in dimensions.
#[derive(Debug, Clone, PartialEq)]
pub enum IgraphVar {
    Var(String),
    Count,
    Pct,
    Case,
}

/// An assignment of X1, Y, X2, COLOR, STYLE, SIZE or SUMMARYVAR.
#[derive(Debug, Clone, PartialEq)]
pub struct IgraphRole {
    pub var: IgraphVar,
    /// TYPE: Some(true) for SCALE, Some(false) for CATEGORICAL, None to take the measurement
    /// level of the variable.
    pub scale: Option<bool>,
    pub max: Option<f64>,
    pub title: Option<String>,
    /// STACK (true) or CLUSTER (false) on COLOR and STYLE.
    pub stack: Option<bool>,
}

/// CATORDER: how the categories of a variable are ordered.
#[derive(Debug, Clone, PartialEq)]
pub struct IgraphOrder {
    pub variable: String,
    pub by: IgraphOrderBy,
    pub descending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IgraphOrderBy {
    Count,
    Occurrence,
    Label,
    Value,
}

/// FITLINE METHOD.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IgraphFit {
    None,
    Regression,
    Origin,
    Mean,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IgraphElement {
    Bar(Option<F>),
    Line(Option<F>),
    Area(Option<F>),
    Pie(Option<F>),
    Box {
        outliers: bool,
        extremes: bool,
    },
    ErrorBar(GraphInterval),
    Histogram {
        cumulative: bool,
        curve: bool,
    },
    Scatter,
    /// A line over all points (`total`) and one per legend group (`groups`).
    FitLine {
        method: IgraphFit,
        total: bool,
        groups: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Igraph {
    pub y: Option<IgraphRole>,
    pub x1: Option<IgraphRole>,
    pub x2: Option<IgraphRole>,
    pub color: Option<IgraphRole>,
    pub style: Option<IgraphRole>,
    pub size: Option<IgraphRole>,
    pub summary_var: Option<IgraphRole>,
    /// STACK or CLUSTER subcommand: a pie per category.
    pub stack: Option<String>,
    pub panel: Vec<String>,
    /// POINTLABEL with ALL: the variable whose values label the points.
    pub point_label: Option<String>,
    pub case_label: Option<String>,
    pub horizontal: bool,
    pub three: bool,
    pub normalize: bool,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub caption: Option<String>,
    pub viewname: Option<String>,
    pub orders: Vec<IgraphOrder>,
    pub elements: Vec<IgraphElement>,
    pub listwise: bool,
    pub report: bool,
    pub include: bool,
    /// The keywords given that only change the look of the chart.
    pub ignored: Vec<String>,
}

const SUBCOMMANDS: [&str; 37] = [
    "Y",
    "X1",
    "X2",
    "YLENGTH",
    "X1LENGTH",
    "X2LENGTH",
    "CATORDER",
    "COLOR",
    "REFLINE",
    "STYLE",
    "NORMALIZE",
    "SIZE",
    "STACK",
    "CLUSTER",
    "SUMMARYVAR",
    "PANEL",
    "POINTLABEL",
    "CASELABEL",
    "COORDINATE",
    "EFFECT",
    "TITLE",
    "SUBTITLE",
    "CAPTION",
    "VIEWNAME",
    "CHARTLOOK",
    "MISSING",
    "FORMAT",
    "SCATTER",
    "BAR",
    "PIE",
    "BOX",
    "LINE",
    "AREA",
    "ERRORBAR",
    "HISTOGRAM",
    "FITLINE",
    "SPIKE",
];

/// The summary functions with the numbers they take, each short name after the longer ones it
/// begins.
const FUNCTIONS: [(&str, usize); 30] = [
    ("FIRST", 0),
    ("KURTOSIS", 0),
    ("LAST", 0),
    ("MAXIMUM", 0),
    ("MINIMUM", 0),
    ("MEAN", 0),
    ("MEDIAN", 0),
    ("MODE", 0),
    ("NGT", 1),
    ("NIN", 2),
    ("NEQ", 1),
    ("NGE", 1),
    ("NLT", 1),
    ("NLE", 1),
    ("PGT", 1),
    ("PIN", 2),
    ("PEQ", 1),
    ("PGE", 1),
    ("PLT", 1),
    ("PLE", 1),
    ("PTILE", 1),
    ("SKEW", 0),
    ("STDDEV", 0),
    ("SEKURT", 0),
    ("SEMEAN", 0),
    ("SESKEW", 0),
    ("SUM", 0),
    ("SUMAV", 0),
    ("SUMSQ", 0),
    ("VARIANCE", 0),
];

fn word<'a>(p: &Parser<'a>) -> Option<&'a str> {
    match p.peek() {
        Some(TokenKind::Word(w)) => Some(w),
        _ => None,
    }
}

fn not_drawn(p: &Parser, what: &str) -> SyntaxError {
    SyntaxError::new(
        p.span(),
        format!("{what} of IGRAPH is not supported: it cannot be drawn yet"),
    )
}

/// `VAR(name)`, a bare name, or $COUNT, $PCT, $CASE.
fn reference(p: &mut Parser) -> PResult<IgraphVar> {
    p.eat(Punct::Eq);
    if p.at_keyword("VAR") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::LParen)) {
        p.pos += 2;
        let name = p.name("a variable")?;
        p.expect(Punct::RParen)?;
        return Ok(IgraphVar::Var(name));
    }
    for (name, var) in [
        ("$COUNT", IgraphVar::Count),
        ("$PCT", IgraphVar::Pct),
        ("$CASE", IgraphVar::Case),
    ] {
        if p.eat_word(name) {
            return Ok(var);
        }
    }
    Ok(IgraphVar::Var(
        p.name("VAR(variable), $COUNT, $PCT or $CASE")?,
    ))
}

fn variable(p: &mut Parser) -> PResult<String> {
    let start = p.pos;
    match reference(p)? {
        IgraphVar::Var(name) => Ok(name),
        _ => Err(SyntaxError::new(
            p.span_from(start),
            "a variable is needed here, not a built-in dimension",
        )),
    }
}

fn on_off(p: &mut Parser) -> PResult<bool> {
    p.eat(Punct::Eq);
    if p.eat_keyword("ON") {
        Ok(true)
    } else if p.eat_keyword("OFF") {
        Ok(false)
    } else {
        Err(p.unexpected(&["ON", "OFF"]))
    }
}

fn text(p: &mut Parser) -> PResult<String> {
    p.eat(Punct::Eq);
    p.string("a quoted string")
}

/// An assignment with its keywords; `legend` allows LEGEND, CLUSTER and STACK.
fn role(p: &mut Parser, legend: bool, ignored: &mut Vec<String>) -> PResult<IgraphRole> {
    let mut role = IgraphRole {
        var: reference(p)?,
        scale: None,
        max: None,
        title: None,
        stack: None,
    };
    loop {
        if p.eat_keyword("TYPE") {
            p.eat(Punct::Eq);
            if p.eat_keyword("CATEGORICAL") {
                role.scale = Some(false);
                continue;
            }
            if !p.eat_keyword("SCALE") {
                return Err(p.unexpected(&["SCALE", "CATEGORICAL"]));
            }
            role.scale = Some(true);
            let parens = p.eat(Punct::LParen);
            loop {
                let max = p.eat_keyword("MAX");
                if !max && !p.eat_keyword("MIN") {
                    break;
                }
                p.eat(Punct::Eq);
                let n = p.number().ok_or_else(|| p.unexpected(&["a number"]))?;
                match max {
                    true => role.max = Some(n),
                    false => ignored.push("MIN".into()),
                }
            }
            if parens {
                p.expect(Punct::RParen)?;
            }
        } else if p.eat_keyword("TITLE") {
            role.title = Some(text(p)?);
        } else if legend && p.eat_keyword("LEGEND") {
            if !on_off(p)? {
                ignored.push("LEGEND=OFF".into());
            }
        } else if legend && p.eat_keyword("CLUSTER") {
            role.stack = Some(false);
        } else if legend && p.eat_keyword("STACK") {
            role.stack = Some(true);
        } else {
            return Ok(role);
        }
    }
}

/// `(NAME [(numbers)])`: a summary function.
fn function(p: &mut Parser) -> PResult<F> {
    p.expect(Punct::LParen)?;
    let Some(w) = word(p) else {
        return Err(p.unexpected(&["a summary function"]));
    };
    let found = FUNCTIONS
        .iter()
        .find(|(k, _)| w.eq_ignore_ascii_case(k))
        .or_else(|| FUNCTIONS.iter().find(|(k, _)| matches_keyword(w, k)));
    let Some(&(name, count)) = found else {
        let names: Vec<&str> = FUNCTIONS.iter().map(|(k, _)| *k).collect();
        return Err(p.unexpected(&names));
    };
    p.pos += 1;
    let start = p.pos;
    let mut args = Vec::new();
    if count > 0 {
        p.expect(Punct::LParen)?;
        for i in 0..count {
            if i > 0 {
                p.eat(Punct::Comma);
            }
            args.push(p.number().ok_or_else(|| p.unexpected(&["a number"]))?);
        }
        p.expect(Punct::RParen)?;
    }
    p.expect(Punct::RParen)?;
    let bad = |message: String| Err(SyntaxError::new(p.span_from(start), message));
    Ok(match (name, args.as_slice()) {
        ("FIRST", _) => F::First,
        ("KURTOSIS", _) => F::Kurtosis,
        ("LAST", _) => F::Last,
        ("MAXIMUM", _) => F::Maximum,
        ("MINIMUM", _) => F::Minimum,
        ("MEAN", _) => F::Mean,
        ("MEDIAN", _) => F::Median,
        ("MODE", _) => F::Mode,
        ("SKEW", _) => F::Skew,
        ("STDDEV", _) => F::StdDev,
        ("SEKURT", _) => F::Sekurt,
        ("SEMEAN", _) => F::Semean,
        ("SESKEW", _) => F::Seskew,
        ("SUM", _) => F::Sum,
        ("SUMAV", _) => F::Sumav,
        ("SUMSQ", _) => F::Sumsq,
        ("VARIANCE", _) => F::Variance,
        ("PTILE", &[x]) if x > 0.0 && x < 100.0 => F::Ptile(x),
        ("PTILE", &[x]) => return bad(format!("PTILE needs a percent between 0 and 100, got {x}")),
        ("NGT", &[x]) => F::Ngt(x),
        ("NEQ", &[x]) => F::Neq(x),
        ("NGE", &[x]) => F::Nge(x),
        ("NLT", &[x]) => F::Nlt(x),
        ("NLE", &[x]) => F::Nle(x),
        ("PGT", &[x]) => F::Pgt(x),
        ("PEQ", &[x]) => F::Peq(x),
        ("PGE", &[x]) => F::Pge(x),
        ("PLT", &[x]) => F::Plt(x),
        ("PLE", &[x]) => F::Ple(x),
        (_, &[a, b]) if a > b => {
            return bad(format!(
                "{name} needs its lower bound first, got {a} and {b}"
            ));
        }
        ("NIN", &[a, b]) => F::Nin(a, b),
        (_, &[a, b]) => F::Pin(a, b),
        _ => unreachable!("the table gives every function its numbers"),
    })
}

/// Skips the value of a keyword that changes only the look: everything up to the next keyword
/// of the element or the end of the subcommand.
fn skip(p: &mut Parser, keywords: &[&str]) {
    let mut depth = 0usize;
    while !at_end(p) {
        match p.peek() {
            Some(TokenKind::Punct(Punct::LParen)) => depth += 1,
            Some(TokenKind::Punct(Punct::RParen)) => depth = depth.saturating_sub(1),
            Some(TokenKind::Word(w))
                if depth == 0 && keywords.iter().any(|k| matches_keyword(w, k)) =>
            {
                return;
            }
            _ => {}
        }
        p.pos += 1;
    }
}

/// `CI(level)`, `SD(n)` or `SE(n)`.
fn interval(p: &mut Parser, k: &str) -> PResult<GraphInterval> {
    p.expect(Punct::LParen)?;
    let start = p.pos;
    let n = p.number().ok_or_else(|| p.unexpected(&["a number"]))?;
    p.expect(Punct::RParen)?;
    let bad = |message: String| Err(SyntaxError::new(p.span_from(start), message));
    match k {
        "CI" if n > 0.0 && n < 100.0 => Ok(GraphInterval::Ci(n)),
        "CI" => bad(format!("CI needs a level between 0 and 100, got {n}")),
        _ if n <= 0.0 || n > 6.0 => {
            bad(format!("{k} needs a number above 0 and at most 6, got {n}"))
        }
        "SD" => Ok(GraphInterval::StdDev(n)),
        _ => Ok(GraphInterval::Se(n)),
    }
}

/// An element subcommand with its name already read.
fn element(p: &mut Parser, name: &str, ignored: &mut Vec<String>) -> PResult<IgraphElement> {
    let function = match name {
        "BAR" | "LINE" | "AREA" | "PIE" if p.at(Punct::LParen) => Some(function(p)?),
        _ => None,
    };
    let keywords: &[&str] = match name {
        "BAR" => &["LABEL", "BASELINE", "KEY", "SHAPE", "BARBASE"],
        "LINE" => &[
            "STYLE",
            "DROPLINE",
            "LABEL",
            "LINELABEL",
            "INTERPOLATE",
            "BREAK",
            "KEY",
        ],
        "AREA" => &[
            "POINTLABEL",
            "AREALABEL",
            "BASELINE",
            "INTERPOLATE",
            "BREAK",
            "KEY",
        ],
        "PIE" => &["START", "CW", "CCW", "SLICE", "STACK", "KEY"],
        "BOX" => &[
            "OUTLIERS", "EXTREME", "MEDIAN", "WHISKER", "CAPWIDTH", "BOXBASE", "LABEL", "KEY",
        ],
        "ERRORBAR" => &[
            "CI",
            "SD",
            "SE",
            "LABEL",
            "CAPWIDTH",
            "CAPSTYLE",
            "SYMBOL",
            "BASELINE",
            "DIRECTION",
            "KEY",
        ],
        "HISTOGRAM" => &[
            "CUM",
            "SHAPE",
            "X1INTERVAL",
            "X2INTERVAL",
            "X1START",
            "X2START",
            "CURVE",
            "KEY",
        ],
        "SCATTER" => &["COINCIDENT", "KEY"],
        _ => &["METHOD", "INTERVAL", "LINE", "SPIKE", "KEY"],
    };
    let (mut outliers, mut extremes, mut interval_given) = (true, true, None);
    let (mut cumulative, mut curve) = (false, false);
    let (mut method, mut total, mut groups) = (IgraphFit::Regression, false, false);
    while !at_end(p) {
        let Some(&k) = keywords.iter().find(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(keywords));
        };
        p.pos += 1;
        match (name, k) {
            ("BOX", "OUTLIERS") => outliers = on_off(p)?,
            ("BOX", "EXTREME") => extremes = on_off(p)?,
            ("ERRORBAR", "CI" | "SD" | "SE") => interval_given = Some(interval(p, k)?),
            ("HISTOGRAM", "CUM") => {
                cumulative = !p.eat(Punct::Eq) || on_off(p)?;
            }
            ("HISTOGRAM", "CURVE") => curve = on_off(p)?,
            ("HISTOGRAM", "X1INTERVAL") => {
                p.eat(Punct::Eq);
                if !p.eat_keyword("AUTO") {
                    ignored.push("X1INTERVAL".into());
                    skip(p, keywords);
                }
            }
            ("HISTOGRAM", "X1START") => {
                p.eat(Punct::Eq);
                if p.number() != Some(0.0) {
                    ignored.push("X1START".into());
                }
            }
            ("SCATTER", "COINCIDENT") => {
                p.eat(Punct::Eq);
                if p.eat_keyword("JITTER") {
                    ignored.push("COINCIDENT=JITTER".into());
                } else if !p.eat_keyword("NONE") {
                    return Err(p.unexpected(&["NONE", "JITTER"]));
                }
                p.number();
            }
            ("FITLINE", "METHOD") => {
                p.eat(Punct::Eq);
                method = if p.eat_keyword("NONE") {
                    IgraphFit::None
                } else if p.eat_keyword("MEAN") {
                    IgraphFit::Mean
                } else if p.at_keyword("LLR") {
                    return Err(not_drawn(p, "FITLINE METHOD=LLR (a local regression)"));
                } else {
                    let origin = p.eat_keyword("ORIGIN");
                    if !origin && !p.eat_keyword("REGRESSION") {
                        return Err(p.unexpected(&["REGRESSION", "ORIGIN", "MEAN", "NONE", "LLR"]));
                    }
                    if !p.eat_keyword("LINEAR") {
                        return Err(p.unexpected(&["LINEAR"]));
                    }
                    match origin {
                        true => IgraphFit::Origin,
                        false => IgraphFit::Regression,
                    }
                };
            }
            ("FITLINE", "INTERVAL") => {
                p.pos -= 1;
                return Err(not_drawn(p, "FITLINE INTERVAL (prediction bands)"));
            }
            ("FITLINE", "LINE") => {
                p.eat(Punct::Eq);
                loop {
                    if p.eat_keyword("TOTAL") {
                        total = true;
                    } else if p.eat_keyword("MEFFECT") {
                        groups = true;
                    } else {
                        break;
                    }
                }
            }
            ("FITLINE", "SPIKE") => {
                let at = p.pos;
                if on_off(p)? {
                    p.pos = at - 1;
                    return Err(not_drawn(p, "FITLINE SPIKE=ON"));
                }
            }
            (_, "KEY") => {
                if !on_off(p)? {
                    ignored.push("KEY=OFF".into());
                }
            }
            _ => {
                ignored.push(k.to_string());
                skip(p, keywords);
            }
        }
    }
    Ok(match name {
        "BAR" => IgraphElement::Bar(function),
        "LINE" => IgraphElement::Line(function),
        "AREA" => IgraphElement::Area(function),
        "PIE" => IgraphElement::Pie(function),
        "BOX" => IgraphElement::Box { outliers, extremes },
        // the manual requires one of CI, SD and SE; without one the bars are a 95% interval
        "ERRORBAR" => IgraphElement::ErrorBar(interval_given.unwrap_or(GraphInterval::Ci(95.0))),
        "HISTOGRAM" => IgraphElement::Histogram { cumulative, curve },
        "SCATTER" => IgraphElement::Scatter,
        _ => IgraphElement::FitLine {
            method,
            // without LINE the fit is over all points
            total: total || !groups,
            groups,
        },
    })
}

/// `VAR(name) (ASCENDING | DESCENDING  COUNT | OCCURRENCE | LABEL | VALUE  SHOWEMPTY | OMITEMPTY)`.
fn catorder(p: &mut Parser, ignored: &mut Vec<String>) -> PResult<IgraphOrder> {
    let mut order = IgraphOrder {
        variable: variable(p)?,
        by: IgraphOrderBy::Value,
        descending: false,
    };
    if !p.eat(Punct::LParen) {
        return Ok(order);
    }
    while !p.eat(Punct::RParen) {
        let keys = [
            "ASCENDING",
            "DESCENDING",
            "COUNT",
            "OCCURRENCE",
            "LABEL",
            "VALUES",
            "VALUE",
            "SHOWEMPTY",
            "OMITEMPTY",
        ];
        let Some(k) = keys.iter().position(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(&keys));
        };
        p.pos += 1;
        match k {
            0 | 1 => order.descending = k == 1,
            2 => order.by = IgraphOrderBy::Count,
            3 => order.by = IgraphOrderBy::Occurrence,
            4 => order.by = IgraphOrderBy::Label,
            5 | 6 => order.by = IgraphOrderBy::Value,
            7 => ignored.push("SHOWEMPTY".into()),
            _ => {}
        }
        p.eat(Punct::Comma);
    }
    Ok(order)
}

pub(crate) fn igraph(p: &mut Parser) -> PResult<crate::ast::Command> {
    let mut g = Igraph {
        listwise: true,
        ..Igraph::default()
    };
    let mut ignored = Vec::new();
    loop {
        p.eat(Punct::Slash);
        if p.peek().is_none() {
            break;
        }
        let Some(w) = word(p) else {
            return Err(p.unexpected(&SUBCOMMANDS));
        };
        let Some(&name) = SUBCOMMANDS
            .iter()
            .find(|k| w.eq_ignore_ascii_case(k))
            .or_else(|| SUBCOMMANDS.iter().find(|k| matches_keyword(w, k)))
        else {
            return Err(p.subcommand_error("IGRAPH", &[]));
        };
        if matches!(name, "REFLINE" | "SPIKE") {
            return Err(not_drawn(p, &format!("subcommand {name}")));
        }
        p.pos += 1;
        match name {
            "Y" => g.y = Some(role(p, false, &mut ignored)?),
            "X1" => g.x1 = Some(role(p, false, &mut ignored)?),
            "X2" => g.x2 = Some(role(p, false, &mut ignored)?),
            "COLOR" => g.color = Some(role(p, true, &mut ignored)?),
            "STYLE" => g.style = Some(role(p, true, &mut ignored)?),
            "SIZE" => g.size = Some(role(p, true, &mut ignored)?),
            "SUMMARYVAR" => g.summary_var = Some(role(p, false, &mut ignored)?),
            "STACK" | "CLUSTER" => g.stack = Some(variable(p)?),
            "PANEL" => {
                p.eat(Punct::Eq);
                g.panel.clear();
                while !at_end(p) {
                    g.panel.push(variable(p)?);
                    p.eat(Punct::Comma);
                }
                if g.panel.is_empty() {
                    return Err(p.unexpected(&["a variable"]));
                }
            }
            "POINTLABEL" => {
                let name = variable(p)?;
                g.point_label = None;
                if p.eat_keyword("ALL") {
                    g.point_label = Some(name);
                } else {
                    p.eat_keyword("NONE");
                }
            }
            "CASELABEL" => g.case_label = Some(variable(p)?),
            "COORDINATE" => {
                p.eat(Punct::Eq);
                let keys = ["HORIZONTAL", "VERTICAL", "THREE"];
                let Some(k) = keys.iter().position(|k| p.at_keyword(k)) else {
                    return Err(p.unexpected(&keys));
                };
                p.pos += 1;
                (g.horizontal, g.three) = (k == 0, k == 2);
            }
            "EFFECT" => {
                p.eat(Punct::Eq);
                if p.eat_keyword("THREE") {
                    ignored.push("EFFECT=THREE".into());
                } else if !p.eat_keyword("NONE") {
                    return Err(p.unexpected(&["NONE", "THREE"]));
                }
            }
            "NORMALIZE" => g.normalize = true,
            "TITLE" => g.title = Some(text(p)?),
            "SUBTITLE" => g.subtitle = Some(text(p)?),
            "CAPTION" => g.caption = Some(text(p)?),
            "VIEWNAME" => g.viewname = Some(text(p)?),
            "CATORDER" => {
                let order = catorder(p, &mut ignored)?;
                g.orders
                    .retain(|o| !o.variable.eq_ignore_ascii_case(&order.variable));
                g.orders.push(order);
            }
            "MISSING" => {
                p.eat(Punct::Eq);
                let keys = [
                    "LISTWISE",
                    "VARIABLEWISE",
                    "NOREPORT",
                    "REPORT",
                    "EXCLUDE",
                    "INCLUDE",
                ];
                while !at_end(p) {
                    let Some(k) = keys.iter().position(|k| p.at_keyword(k)) else {
                        return Err(p.unexpected(&keys));
                    };
                    p.pos += 1;
                    match k {
                        0 | 1 => g.listwise = k == 0,
                        2 | 3 => g.report = k == 3,
                        _ => g.include = k == 5,
                    }
                    p.eat(Punct::Comma);
                }
            }
            "YLENGTH" | "X1LENGTH" | "X2LENGTH" | "CHARTLOOK" | "FORMAT" => {
                ignored.push(name.to_string());
                while !at_end(p) {
                    p.pos += 1;
                }
            }
            _ => g.elements.push(element(p, name, &mut ignored)?),
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'"]));
        }
    }
    for name in ignored {
        if !g.ignored.contains(&name) {
            g.ignored.push(name);
        }
    }
    Ok(crate::ast::Command::Igraph(Box::new(g)))
}
