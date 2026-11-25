//! Parser of XGRAPH: a chart expression of y variables with a summary function and a data
//! element in square brackets, crossed with BY over an x and a z axis, then subcommands.
//!
//! Rules taken from the manual:
//! - The chart expression comes first, after an optional CHART=; the subcommands follow in any
//!   order, each at most once, every name and keyword written in full.
//! - y: `(item + item ...)` where an item is `var [function] [element]`, `(var + var) [function]
//!   [element]` (the function over each variable) or `[count function] [element]`. Each pair of
//!   brackets holds one keyword. `> var` or `> 1` after the parentheses stacks.
//! - x and z: `var [c | s]`, `$CASENUM [LABEL=var]` or `1`, the placeholder for the y variables;
//!   `> var` after one clusters. [COLOR] and [PATTERN] may follow a stack or cluster variable.
//! - No more than two clusters, or a stack with a single cluster.
//! - BIN START=AUTO | x SIZE=AUTO | WIDTH(x) | COUNT(n); DISPLAY DOT=ASYMMETRIC | SYMMETRIC |
//!   FLAT; DISTRIBUTION TYPE=NORMAL; COORDINATE SPLIT=NO | YES; ERRORBAR CI(level) | STDDEV(n) |
//!   SE(n); MISSING USE=LISTWISE | VARIABLEWISE REPORT=NO | YES; PANEL as in GRAPH; TEMPLATE
//!   FILE='file'; TITLES TITLE= (two lines) SUBTITLE= (one) FOOTNOTE= (two), 255 bytes a line.

use crate::ast::Command;
use crate::error::SyntaxError;
use crate::graph::{
    GraphFunction as F, GraphInterval, GraphPanel, interval_spec, lines_up_to, panel,
};
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// What an item of y computes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum XgraphStat {
    /// VALUE: the value of every case.
    Value,
    Summary(F),
}

/// One blended item of y: variables (none for a count function) with their function.
#[derive(Debug, Clone, PartialEq)]
pub struct XgraphY {
    pub variables: Vec<String>,
    pub stat: Option<XgraphStat>,
}

/// The data element. LINE and BOX are additions to the manual's BAR, HISTOBAR and POINT.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum XgraphElement {
    #[default]
    Bar,
    Histobar,
    Point,
    Line,
    Box,
}

/// A categorical dimension: a variable (`scale` from [s] or [c] when given), the placeholder 1
/// for the y variables, or the cases ($CASENUM) named by an optional label variable.
#[derive(Debug, Clone, PartialEq)]
pub enum XgraphDim {
    Var { name: String, scale: Option<bool> },
    One,
    Cases { label: Option<String> },
}

/// The x or z axis with the variable that clusters it.
#[derive(Debug, Clone, PartialEq)]
pub struct XgraphAxis {
    pub dim: XgraphDim,
    pub cluster: Option<XgraphDim>,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum XgraphBinSize {
    #[default]
    Auto,
    Width(f64),
    Count(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum XgraphDot {
    #[default]
    Asymmetric,
    Symmetric,
    Flat,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Xgraph {
    pub y: Vec<XgraphY>,
    pub element: XgraphElement,
    pub stack: Option<XgraphDim>,
    pub x: Option<XgraphAxis>,
    pub z: Option<XgraphAxis>,
    /// BIN: the start (None for AUTO) and the size.
    pub bin: Option<(Option<f64>, XgraphBinSize)>,
    pub dot: Option<XgraphDot>,
    /// DISTRIBUTION TYPE=NORMAL.
    pub normal: bool,
    /// COORDINATE SPLIT=YES.
    pub split: bool,
    pub interval: Option<GraphInterval>,
    /// USE=LISTWISE (true) or VARIABLEWISE.
    pub listwise: bool,
    pub report: bool,
    pub panel: Option<GraphPanel>,
    pub template: bool,
    pub title: Vec<String>,
    pub subtitle: Option<String>,
    pub footnote: Vec<String>,
}

const SUBCOMMANDS: [&str; 9] = [
    "BIN",
    "DISPLAY",
    "DISTRIBUTION",
    "COORDINATE",
    "ERRORBAR",
    "MISSING",
    "PANEL",
    "TEMPLATE",
    "TITLES",
];

const ELEMENTS: [(&str, XgraphElement); 5] = [
    ("BAR", XgraphElement::Bar),
    ("HISTOBAR", XgraphElement::Histobar),
    ("POINT", XgraphElement::Point),
    ("LINE", XgraphElement::Line),
    ("BOX", XgraphElement::Box),
];

/// The functions with the numbers they take.
const FUNCTIONS: [(&str, usize); 24] = [
    ("COUNT", 0),
    ("PCT", 0),
    ("CUPCT", 0),
    ("CUFREQ", 0),
    ("MINIMUM", 0),
    ("MAXIMUM", 0),
    ("VALIDN", 0),
    ("SUM", 0),
    ("CUSUM", 0),
    ("CSUM", 0),
    ("MEAN", 0),
    ("STDDEV", 0),
    ("VARIANCE", 0),
    ("MEDIAN", 0),
    ("GMEDIAN", 0),
    ("MODE", 0),
    ("PTILE", 1),
    ("PLT", 1),
    ("PGT", 1),
    ("NLT", 1),
    ("NGT", 1),
    ("PIN", 2),
    ("NIN", 2),
    ("GPTILE", 1),
];

fn word<'a>(p: &Parser<'a>) -> Option<&'a str> {
    match p.peek() {
        Some(TokenKind::Word(w)) => Some(w),
        _ => None,
    }
}

/// One of `keys` written in full; its index.
fn keyword(p: &mut Parser, keys: &[&str]) -> PResult<usize> {
    let found = word(p).and_then(|w| keys.iter().position(|k| w.eq_ignore_ascii_case(k)));
    match found {
        Some(k) => {
            p.pos += 1;
            Ok(k)
        }
        None => Err(p.unexpected(keys)),
    }
}

/// `KEY=` with KEY written in full.
fn key(p: &mut Parser, keys: &[&str]) -> PResult<usize> {
    let k = keyword(p, keys)?;
    p.expect(Punct::Eq)?;
    Ok(k)
}

/// `(n)` after a keyword.
fn parenthesized(p: &mut Parser) -> PResult<f64> {
    p.expect(Punct::LParen)?;
    let Some(n) = p.number() else {
        return Err(p.unexpected(&["a number"]));
    };
    p.expect(Punct::RParen)?;
    Ok(n)
}

/// A function with its numbers, the name already read inside the brackets.
fn function(p: &mut Parser, name: &str, numbers: usize) -> PResult<XgraphStat> {
    let start = p.pos - 1;
    let mut args = Vec::new();
    if numbers > 0 {
        p.expect(Punct::LParen)?;
        for i in 0..numbers {
            if i > 0 {
                p.eat(Punct::Comma);
            }
            let Some(n) = p.number() else {
                return Err(p.unexpected(&["a number"]));
            };
            args.push(n);
        }
        p.expect(Punct::RParen)?;
    }
    let bad = |message: String| Err(SyntaxError::new(p.span_from(start), message));
    Ok(XgraphStat::Summary(match (name, args.as_slice()) {
        ("COUNT", _) => F::Count,
        ("PCT", _) => F::Pct,
        ("CUPCT", _) => F::Cupct,
        ("CUFREQ", _) => F::Cufreq,
        ("MINIMUM", _) => F::Minimum,
        ("MAXIMUM", _) => F::Maximum,
        ("VALIDN", _) => F::N,
        ("SUM", _) => F::Sum,
        ("CUSUM" | "CSUM", _) => F::Cusum,
        ("MEAN", _) => F::Mean,
        ("STDDEV", _) => F::StdDev,
        ("VARIANCE", _) => F::Variance,
        ("MEDIAN", _) => F::Median,
        ("GMEDIAN", _) => F::Gmedian,
        ("MODE", _) => F::Mode,
        ("GPTILE", &[x]) if x > 0.0 && x < 100.0 => F::Gptile(x),
        ("GPTILE", &[x]) => {
            return bad(format!(
                "GPTILE needs a percentile between 0 and 100, got {x}"
            ));
        }
        ("PTILE", &[x]) if x > 0.0 && x < 100.0 => F::Ptile(x),
        ("PTILE", &[x]) => {
            return bad(format!(
                "PTILE needs a percentile between 0 and 100, got {x}"
            ));
        }
        ("PLT", &[x]) => F::Plt(x),
        ("PGT", &[x]) => F::Pgt(x),
        ("NLT", &[x]) => F::Nlt(x),
        ("NGT", &[x]) => F::Ngt(x),
        (_, &[low, high]) if low > high => {
            return bad(format!(
                "{name} needs its lower bound first, got {low} and {high}"
            ));
        }
        ("PIN", &[low, high]) => F::Pin(low, high),
        (_, &[low, high]) => F::Nin(low, high),
        _ => unreachable!("the table gives every function its numbers"),
    }))
}

/// `[keyword]` groups after the variables of an item: a function and an element.
fn brackets(
    p: &mut Parser,
    item: &mut XgraphY,
    element: &mut Option<XgraphElement>,
) -> PResult<()> {
    while p.eat(Punct::LBracket) {
        let start = p.pos;
        let w = word(p).unwrap_or("");
        let twice = |p: &Parser, what: &str| {
            SyntaxError::new(p.span_from(start), format!("an item takes one {what}"))
        };
        if let Some((_, e)) = ELEMENTS.iter().find(|(k, _)| w.eq_ignore_ascii_case(k)) {
            p.pos += 1;
            if element.is_some_and(|given| given != *e) {
                return Err(twice(p, "data element; every item of y takes the same one"));
            }
            *element = Some(*e);
        } else if w.eq_ignore_ascii_case("VALUE") {
            p.pos += 1;
            if item.stat.is_some() {
                return Err(twice(p, "function"));
            }
            item.stat = Some(XgraphStat::Value);
        } else if let Some((name, n)) = FUNCTIONS.iter().find(|(k, _)| w.eq_ignore_ascii_case(k)) {
            p.pos += 1;
            if item.stat.is_some() {
                return Err(twice(p, "function"));
            }
            item.stat = Some(function(p, name, *n)?);
        } else {
            let mut expected: Vec<&str> = FUNCTIONS.iter().map(|(k, _)| *k).collect();
            expected.extend(["VALUE"]);
            expected.extend(ELEMENTS.iter().map(|(k, _)| *k));
            return Err(p.unexpected(&expected));
        }
        p.expect(Punct::RBracket)?;
    }
    Ok(())
}

/// `(item + item ...)`; the element given in the items, if any.
fn y_items(p: &mut Parser) -> PResult<(Vec<XgraphY>, Option<XgraphElement>)> {
    p.expect(Punct::LParen)?;
    let (mut items, mut element) = (Vec::new(), None);
    loop {
        let mut item = XgraphY {
            variables: Vec::new(),
            stat: None,
        };
        if p.eat(Punct::LParen) {
            loop {
                item.variables.push(p.name("a variable")?);
                if !p.eat(Punct::Plus) {
                    break;
                }
            }
            p.expect(Punct::RParen)?;
        } else if !p.at(Punct::LBracket) {
            item.variables.push(p.name("a variable or a [function]")?);
        }
        brackets(p, &mut item, &mut element)?;
        items.push(item);
        if !p.eat(Punct::Plus) {
            break;
        }
    }
    p.expect(Punct::RParen)?;
    Ok((items, element))
}

/// A dimension with its bracketed markers: [c], [s], [LABEL=var], [COLOR], [PATTERN].
fn dim(p: &mut Parser) -> PResult<XgraphDim> {
    let mut dim = match p.peek() {
        Some(TokenKind::Number(n)) if *n == 1.0 => {
            p.pos += 1;
            XgraphDim::One
        }
        Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("$CASENUM") => {
            p.pos += 1;
            XgraphDim::Cases { label: None }
        }
        _ => XgraphDim::Var {
            name: p.name("a variable, $CASENUM or 1")?,
            scale: None,
        },
    };
    while p.eat(Punct::LBracket) {
        let k = keyword(p, &["C", "S", "LABEL", "COLOR", "PATTERN"])?;
        match (&mut dim, k) {
            (XgraphDim::Var { scale, .. }, 0 | 1) => *scale = Some(k == 1),
            (XgraphDim::Cases { label }, 2) => {
                p.expect(Punct::Eq)?;
                *label = Some(p.name("a label variable")?);
            }
            // colors and patterns only change the look
            (_, 3 | 4) => {}
            _ => {
                let message = "[c] and [s] follow a variable, [LABEL=var] follows $CASENUM";
                return Err(SyntaxError::new(p.span_from(p.pos - 1), message));
            }
        }
        p.expect(Punct::RBracket)?;
    }
    Ok(dim)
}

fn axis(p: &mut Parser) -> PResult<XgraphAxis> {
    let dim = dim(p)?;
    let cluster = match p.eat(Punct::Gt) {
        true => Some(self::dim(p)?),
        false => None,
    };
    Ok(XgraphAxis { dim, cluster })
}

/// The chart expression up to the first slash.
fn chart_expression(p: &mut Parser, g: &mut Xgraph) -> PResult<()> {
    if word(p).is_some_and(|w| w.eq_ignore_ascii_case("CHART")) {
        p.pos += 1;
        p.expect(Punct::Eq)?;
    }
    let start = p.pos;
    let (y, element) = y_items(p)?;
    g.y = y;
    g.element = element.unwrap_or_default();
    if p.eat(Punct::Gt) {
        g.stack = Some(dim(p)?);
    }
    if p.eat_word("BY") {
        g.x = Some(axis(p)?);
        if p.eat_word("BY") {
            g.z = Some(axis(p)?);
        }
    }
    let clusters = [&g.x, &g.z]
        .iter()
        .filter(|a| a.as_ref().is_some_and(|a| a.cluster.is_some()))
        .count();
    if clusters + usize::from(g.stack.is_some()) > 2 {
        let message = "a chart has room for two clusters, or a stack with a single cluster";
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(())
}

pub(crate) fn xgraph(p: &mut Parser) -> PResult<Command> {
    let mut g = Xgraph {
        listwise: true,
        ..Xgraph::default()
    };
    chart_expression(p, &mut g)?;
    let mut seen: Vec<usize> = Vec::new();
    while p.eat(Punct::Slash) {
        let start = p.pos;
        let Some(s) =
            word(p).and_then(|w| SUBCOMMANDS.iter().position(|k| w.eq_ignore_ascii_case(k)))
        else {
            return Err(p.subcommand_error("XGRAPH", &[]));
        };
        p.pos += 1;
        if seen.contains(&s) {
            let message = format!(
                "{} is given twice; each subcommand comes once",
                SUBCOMMANDS[s]
            );
            return Err(SyntaxError::new(p.span_from(start), message));
        }
        seen.push(s);
        match SUBCOMMANDS[s] {
            "BIN" => {
                let (mut start, mut size) = (None, XgraphBinSize::Auto);
                while !at_end(p) {
                    if key(p, &["START", "SIZE"])? == 0 {
                        start = p.number();
                        if start.is_none() && !p.eat_word("AUTO") {
                            return Err(p.unexpected(&["AUTO", "a number"]));
                        }
                    } else {
                        size = match keyword(p, &["AUTO", "WIDTH", "COUNT"])? {
                            0 => XgraphBinSize::Auto,
                            1 => match parenthesized(p)? {
                                w if w > 0.0 => XgraphBinSize::Width(w),
                                w => {
                                    let message = format!("WIDTH needs a number above 0, got {w}");
                                    return Err(SyntaxError::new(p.span_from(p.pos - 2), message));
                                }
                            },
                            _ => match parenthesized(p)? {
                                n if n >= 1.0 && n.fract() == 0.0 => {
                                    XgraphBinSize::Count(n as usize)
                                }
                                n => {
                                    let message = format!(
                                        "COUNT needs a whole number of bins, 1 or more, got {n}"
                                    );
                                    return Err(SyntaxError::new(p.span_from(p.pos - 2), message));
                                }
                            },
                        };
                    }
                }
                g.bin = Some((start, size));
            }
            "DISPLAY" => {
                key(p, &["DOT"])?;
                g.dot = Some(match keyword(p, &["ASYMMETRIC", "SYMMETRIC", "FLAT"])? {
                    0 => XgraphDot::Asymmetric,
                    1 => XgraphDot::Symmetric,
                    _ => XgraphDot::Flat,
                });
            }
            "DISTRIBUTION" => {
                key(p, &["TYPE"])?;
                keyword(p, &["NORMAL"])?;
                g.normal = true;
            }
            "COORDINATE" => {
                key(p, &["SPLIT"])?;
                g.split = keyword(p, &["NO", "YES"])? == 1;
            }
            "ERRORBAR" => g.interval = Some(interval_spec(p)?),
            "MISSING" => {
                while !at_end(p) {
                    if key(p, &["USE", "REPORT"])? == 0 {
                        g.listwise = keyword(p, &["LISTWISE", "VARIABLEWISE"])? == 0;
                    } else {
                        g.report = keyword(p, &["NO", "YES"])? == 1;
                    }
                }
            }
            "PANEL" => g.panel = Some(panel(p)?),
            "TEMPLATE" => {
                key(p, &["FILE"])?;
                p.string("a quoted file name")?;
                g.template = true;
            }
            _ => {
                if at_end(p) {
                    return Err(p.unexpected(&["TITLE", "SUBTITLE", "FOOTNOTE"]));
                }
                while !at_end(p) {
                    match key(p, &["TITLE", "SUBTITLE", "FOOTNOTE"])? {
                        0 => g.title = lines_up_to(p, "TITLE", 2, 255)?,
                        1 => g.subtitle = lines_up_to(p, "SUBTITLE", 1, 255)?.pop(),
                        _ => g.footnote = lines_up_to(p, "FOOTNOTE", 2, 255)?,
                    }
                }
            }
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::Xgraph(Box::new(g)))
}
