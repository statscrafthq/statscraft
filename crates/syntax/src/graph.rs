//! Parser of GRAPH: one chart of summaries by categories, of case values, or of the cases
//! themselves.
//!
//! Rules taken from the manual:
//! - The subcommands come in any order, the first one may lack its slash, and every keyword may
//!   be cut to three letters. One chart subcommand per command.
//! - TITLE and FOOTNOTE take one or two quoted lines, SUBTITLE one; a line has 72 characters at
//!   most.
//! - BAR [(SIMPLE | GROUPED | STACKED)], LINE [(SIMPLE | MULTIPLE | AREA)] and PIE take a
//!   function specification. The data decide between simple and grouped, whatever keyword was
//!   given; only STACKED and AREA change the drawing.
//! - A function specification: a count function (COUNT, PCT, CUPCT, CUFREQ) BY one or two
//!   category variables; a bare variable, which is COUNT BY that variable; one or more summary
//!   functions, each over a variable list (SUM(a b) is SUM(a) SUM(b)), with up to two BY
//!   variables; or VALUE(varlist) [BY var], one category per case.
//! - HISTOGRAM [(NORMAL)] = one variable. SCATTERPLOT [(BIVARIATE)] = x WITH y, then BY a
//!   control variable and BY a variable with (NAME) or (IDENTIFY) whose values label points.
//! - MISSING: LISTWISE (default) or VARIABLE, NOREPORT (default) or REPORT, EXCLUDE (default)
//!   or INCLUDE.

use crate::ast::{Command, VarItem};
use crate::error::SyntaxError;
use crate::lexer::{Punct, TokenKind};
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;
use crate::stats::variables;
use crate::table::matches_keyword;

/// A summary or count function of GRAPH.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphFunction {
    Count,
    Pct,
    Cupct,
    Cufreq,
    Minimum,
    Maximum,
    N,
    Sum,
    Cusum,
    Mean,
    StdDev,
    Variance,
    Median,
    Gmedian,
    Mode,
    Ptile(f64),
    Plt(f64),
    Pgt(f64),
    Nlt(f64),
    Ngt(f64),
    Pin(f64, f64),
    Nin(f64, f64),
}

impl GraphFunction {
    /// COUNT, PCT, CUPCT and CUFREQ count cases and take no variable.
    pub fn counts_cases(self) -> bool {
        matches!(self, Self::Count | Self::Pct | Self::Cupct | Self::Cufreq)
    }
}

/// A function over a list of variables; the list is empty for a count function.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphSummary {
    pub function: GraphFunction,
    pub variables: Vec<VarItem>,
}

/// What a bar, line or pie chart shows.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphData {
    /// Functions by up to two category variables.
    Summary {
        functions: Vec<GraphSummary>,
        by: Vec<String>,
    },
    /// `VALUE(varlist) [BY var]`: the values of every case, the BY variable naming the cases.
    Values {
        variables: Vec<VarItem>,
        by: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum GraphChart {
    Bar {
        stacked: bool,
        data: GraphData,
    },
    Line {
        area: bool,
        data: GraphData,
    },
    Pie(GraphData),
    Histogram {
        variable: String,
        normal: bool,
    },
    Scatter {
        x: String,
        y: String,
        /// The groups of points.
        control: Option<String>,
        /// The variable whose values label the points (NAME); IDENTIFY leaves them off.
        names: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    pub title: Vec<String>,
    pub subtitle: Option<String>,
    pub footnote: Vec<String>,
    pub chart: GraphChart,
    /// LISTWISE (true) or VARIABLE.
    pub listwise: bool,
    /// REPORT: categories of missing values are kept.
    pub report: bool,
    pub include: bool,
}

const CHARTS: [&str; 5] = ["BAR", "LINE", "PIE", "HISTOGRAM", "SCATTERPLOT"];

const SUBCOMMANDS: [&str; 15] = [
    "TITLE",
    "SUBTITLE",
    "FOOTNOTE",
    "BAR",
    "LINE",
    "PIE",
    "HISTOGRAM",
    "SCATTERPLOT",
    "ERRORBAR",
    "PARETO",
    "HILO",
    "PANEL",
    "INTERVAL",
    "TEMPLATE",
    "MISSING",
];

const NOT_YET: [&str; 6] = [
    "ERRORBAR", "PARETO", "HILO", "PANEL", "INTERVAL", "TEMPLATE",
];

const COUNTS: [(&str, GraphFunction); 4] = [
    ("COUNT", GraphFunction::Count),
    ("PCT", GraphFunction::Pct),
    ("CUPCT", GraphFunction::Cupct),
    ("CUFREQ", GraphFunction::Cufreq),
];

/// The summary functions with the numbers they take.
const SUMMARIES: [(&str, usize); 18] = [
    ("MINIMUM", 0),
    ("MAXIMUM", 0),
    ("N", 0),
    ("SUM", 0),
    ("CUSUM", 0),
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
];

fn word<'a>(p: &Parser<'a>) -> Option<&'a str> {
    match p.peek() {
        Some(TokenKind::Word(w)) => Some(w),
        _ => None,
    }
}

fn not_yet(p: &Parser, what: &str) -> SyntaxError {
    SyntaxError::new(p.span(), format!("{what} of GRAPH is not supported yet"))
}

/// `(KEYWORD)` after a chart subcommand: the index of the keyword in `allowed`, None when
/// there are no parentheses. Keywords in `unsupported` give an error.
fn chart_keyword(
    p: &mut Parser,
    chart: &str,
    allowed: &[&str],
    unsupported: &[&str],
) -> PResult<Option<usize>> {
    if !p.eat(Punct::LParen) {
        return Ok(None);
    }
    if let Some(name) = unsupported.iter().find(|k| p.at_keyword(k)) {
        return Err(not_yet(p, &format!("{chart}({name})")));
    }
    let Some(index) = allowed.iter().position(|k| p.at_keyword(k)) else {
        return Err(p.unexpected(allowed));
    };
    p.pos += 1;
    p.expect(Punct::RParen)?;
    Ok(Some(index))
}

/// Quoted lines of TITLE, SUBTITLE or FOOTNOTE, at most `most` of them.
fn lines(p: &mut Parser, what: &str, most: usize) -> PResult<Vec<String>> {
    p.eat(Punct::Eq);
    let start = p.pos;
    let mut out = Vec::new();
    while let Some(TokenKind::Str(_)) = p.peek() {
        let line_start = p.pos;
        let line = p.string("a quoted line")?;
        if line.chars().count() > 72 {
            let message = format!("a line of {what} has at most 72 characters");
            return Err(SyntaxError::new(p.span_from(line_start), message));
        }
        out.push(line);
        p.eat(Punct::Comma);
    }
    if out.is_empty() {
        return Err(p.unexpected(&["a quoted line"]));
    }
    if out.len() > most {
        let message = match most {
            1 => format!("{what} takes one line"),
            _ => format!("{what} takes at most {most} lines"),
        };
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(out)
}

/// A number argument of a function, after an optional comma.
fn argument(p: &mut Parser, function: &str) -> PResult<f64> {
    p.eat(Punct::Comma);
    p.number().ok_or_else(|| {
        let mut error = p.unexpected(&["a number"]);
        error.message = format!("{function} needs a number after its variables");
        error
    })
}

/// `NAME(varlist [, numbers])` with the name already read.
fn summary(p: &mut Parser, name: &str, numbers: usize) -> PResult<GraphSummary> {
    p.expect(Punct::LParen)?;
    let variables = variables(p)?;
    let start = p.pos;
    let mut args = Vec::new();
    for _ in 0..numbers {
        args.push(argument(p, name)?);
    }
    p.expect(Punct::RParen)?;
    let bad = |message: String| Err(SyntaxError::new(p.span_from(start), message));
    use GraphFunction as F;
    let function = match (name, args.as_slice()) {
        ("MINIMUM", _) => F::Minimum,
        ("MAXIMUM", _) => F::Maximum,
        ("N", _) => F::N,
        ("SUM", _) => F::Sum,
        ("CUSUM", _) => F::Cusum,
        ("MEAN", _) => F::Mean,
        ("STDDEV", _) => F::StdDev,
        ("VARIANCE", _) => F::Variance,
        ("MEDIAN", _) => F::Median,
        ("GMEDIAN", _) => F::Gmedian,
        ("MODE", _) => F::Mode,
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
    };
    Ok(GraphSummary {
        function,
        variables,
    })
}

/// `BY var [BY var]`, at most `most` of them.
fn by_list(p: &mut Parser, most: usize) -> PResult<Vec<String>> {
    let mut by = Vec::new();
    while p.eat_word("BY") {
        let start = p.pos;
        by.push(p.name("a category variable")?);
        if by.len() > most {
            let message = match most {
                1 => "only one BY variable is allowed here".to_string(),
                _ => format!("at most {most} BY variables are allowed"),
            };
            return Err(SyntaxError::new(p.span_from(start), message));
        }
    }
    Ok(by)
}

/// The function/variable specification of BAR, LINE and PIE.
fn data(p: &mut Parser) -> PResult<GraphData> {
    p.eat(Punct::Eq);
    let opens = p.peek_at(1) == Some(&TokenKind::Punct(Punct::LParen));
    if opens && p.at_keyword("VALUE") {
        p.pos += 2;
        let variables = variables(p)?;
        p.expect(Punct::RParen)?;
        let by = by_list(p, 1)?.pop();
        return Ok(GraphData::Values { variables, by });
    }
    let by_follows =
        matches!(p.peek_at(1), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("BY"));
    if let Some((_, function)) = COUNTS.iter().find(|(k, _)| p.at_keyword(k))
        && (by_follows || opens)
    {
        if opens {
            let message = "a count function takes no variables; write it as COUNT BY var";
            return Err(SyntaxError::new(p.span(), message));
        }
        p.pos += 1;
        let by = by_list(p, 2)?;
        return Ok(GraphData::Summary {
            functions: vec![GraphSummary {
                function: *function,
                variables: Vec::new(),
            }],
            by,
        });
    }
    if !opens {
        // a bare variable counts its categories
        let first = p.name("a function or a variable")?;
        let mut by = vec![first];
        by.extend(by_list(p, 1)?);
        return Ok(GraphData::Summary {
            functions: vec![GraphSummary {
                function: GraphFunction::Count,
                variables: Vec::new(),
            }],
            by,
        });
    }
    let mut functions = Vec::new();
    while let Some(w) = word(p) {
        if w.eq_ignore_ascii_case("BY") {
            break;
        }
        let Some((name, numbers)) = SUMMARIES.iter().find(|(k, _)| matches_keyword(w, k)) else {
            let mut expected: Vec<&str> = SUMMARIES.iter().map(|(k, _)| *k).collect();
            expected.extend(["VALUE", "BY"]);
            if COUNTS.iter().any(|(k, _)| matches_keyword(w, k)) {
                let message = "a count function stands alone, before BY";
                return Err(SyntaxError::new(p.span(), message));
            }
            return Err(p.unexpected(&expected));
        };
        p.pos += 1;
        functions.push(summary(p, name, *numbers)?);
    }
    let by = by_list(p, 2)?;
    Ok(GraphData::Summary { functions, by })
}

/// `x WITH y [BY control] [BY id (NAME | IDENTIFY)]`
fn scatter(p: &mut Parser) -> PResult<GraphChart> {
    p.eat(Punct::Eq);
    let x = p.name("a variable")?;
    if !p.eat_word("WITH") {
        if matches!(p.peek(), Some(TokenKind::Word(_))) {
            return Err(not_yet(
                p,
                "a variable list (OVERLAY or MATRIX scatterplots)",
            ));
        }
        return Err(p.unexpected(&["WITH"]));
    }
    let y = p.name("a variable")?;
    if matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case("WITH")) {
        return Err(not_yet(p, "a third axis (XYZ scatterplots)"));
    }
    if matches!(p.peek(), Some(TokenKind::Word(w)) if !w.eq_ignore_ascii_case("BY")) {
        return Err(not_yet(
            p,
            "a variable list (OVERLAY or MATRIX scatterplots)",
        ));
    }
    let (mut control, mut names) = (None, None);
    let mut labels_given = false;
    while p.eat_word("BY") {
        let start = p.pos;
        let name = p.name("a variable")?;
        if p.eat(Punct::LParen) {
            let shown = if p.eat_keyword("NAME") {
                true
            } else if p.eat_keyword("IDENTIFY") {
                false
            } else {
                return Err(p.unexpected(&["NAME", "IDENTIFY"]));
            };
            p.expect(Punct::RParen)?;
            if labels_given {
                let message = "only one variable labels the points";
                return Err(SyntaxError::new(p.span_from(start), message));
            }
            labels_given = true;
            names = shown.then_some(name);
        } else if control.is_some() || labels_given {
            let message = "the control variable comes once, before the label variable";
            return Err(SyntaxError::new(p.span_from(start), message));
        } else {
            control = Some(name);
        }
    }
    Ok(GraphChart::Scatter {
        x,
        y,
        control,
        names,
    })
}

pub(crate) fn graph(p: &mut Parser) -> PResult<Command> {
    let mut title = Vec::new();
    let mut subtitle = None;
    let mut footnote = Vec::new();
    let mut chart: Option<GraphChart> = None;
    let (mut listwise, mut report, mut include) = (true, false, false);
    p.eat(Punct::Slash);
    loop {
        let start = p.pos;
        let Some(w) = word(p) else {
            return Err(p.unexpected(&SUBCOMMANDS));
        };
        let Some(name) = SUBCOMMANDS.iter().find(|k| matches_keyword(w, k)) else {
            return Err(p.subcommand_error("GRAPH", &[]));
        };
        if NOT_YET.contains(name) {
            return Err(not_yet(p, &format!("subcommand {name}")));
        }
        p.pos += 1;
        let made = match *name {
            "TITLE" => {
                title = lines(p, "TITLE", 2)?;
                None
            }
            "SUBTITLE" => {
                subtitle = lines(p, "SUBTITLE", 1)?.pop();
                None
            }
            "FOOTNOTE" => {
                footnote = lines(p, "FOOTNOTE", 2)?;
                None
            }
            "MISSING" => {
                p.eat(Punct::Eq);
                if at_end(p) {
                    return Err(p.unexpected(&["LISTWISE", "VARIABLE", "REPORT", "INCLUDE"]));
                }
                while !at_end(p) {
                    let keywords = [
                        "LISTWISE", "VARIABLE", "NOREPORT", "REPORT", "EXCLUDE", "INCLUDE",
                    ];
                    let Some(k) = keywords.iter().position(|k| p.at_keyword(k)) else {
                        return Err(p.unexpected(&keywords));
                    };
                    p.pos += 1;
                    match k {
                        0 | 1 => listwise = k == 0,
                        2 | 3 => report = k == 3,
                        _ => include = k == 5,
                    }
                    p.eat(Punct::Comma);
                }
                None
            }
            "BAR" => {
                let k = chart_keyword(p, "BAR", &["SIMPLE", "GROUPED", "STACKED"], &["RANGE"])?;
                Some(GraphChart::Bar {
                    stacked: k == Some(2),
                    data: data(p)?,
                })
            }
            "LINE" => {
                let allowed = ["SIMPLE", "MULTIPLE", "AREA"];
                let k = chart_keyword(p, "LINE", &allowed, &["DROP", "DIFFERENCE"])?;
                Some(GraphChart::Line {
                    area: k == Some(2),
                    data: data(p)?,
                })
            }
            "PIE" => Some(GraphChart::Pie(data(p)?)),
            "HISTOGRAM" => {
                let normal = chart_keyword(p, "HISTOGRAM", &["NORMAL"], &[])?.is_some();
                p.eat(Punct::Eq);
                let variable = p.name("a variable")?;
                if matches!(p.peek(), Some(TokenKind::Word(_))) {
                    let message = "HISTOGRAM takes one variable";
                    return Err(SyntaxError::new(p.span(), message));
                }
                Some(GraphChart::Histogram { variable, normal })
            }
            _ => {
                let unsupported = ["OVERLAY", "MATRIX", "XYZ"];
                chart_keyword(p, "SCATTERPLOT", &["BIVARIATE"], &unsupported)?;
                Some(scatter(p)?)
            }
        };
        if let Some(made) = made {
            if chart.is_some() {
                let message = "GRAPH draws one chart; give only one of BAR, LINE, PIE, \
                               HISTOGRAM and SCATTERPLOT";
                return Err(SyntaxError::new(p.span_from(start), message));
            }
            chart = Some(made);
        }
        if !p.eat(Punct::Slash) {
            break;
        }
    }
    p.finish(&["'/'"])?;
    let Some(chart) = chart else {
        return Err(p.unexpected(&CHARTS));
    };
    Ok(Command::Graph(Box::new(Graph {
        title,
        subtitle,
        footnote,
        chart,
        listwise,
        report,
        include,
    })))
}
