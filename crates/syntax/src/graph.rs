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
//! - HISTOGRAM [(NORMAL)] = one variable. SCATTERPLOT [(BIVARIATE | OVERLAY | MATRIX | XYZ)]:
//!   x WITH y is one plot; lists on both sides of WITH overlay a plot per pair (every
//!   combination, or in order with (PAIR)); a list alone is a matrix of plots; x WITH y WITH z
//!   has three axes. As for bars, the variables decide and the keyword changes nothing. BY a
//!   control variable (not for an overlay) and BY a variable with (NAME) or (IDENTIFY) whose
//!   values label points may follow.
//! - ERRORBAR [(CI [level] | STERROR [n] | STDDEV [n])] = varlist [BY var] or var BY var BY var:
//!   means with bars of a confidence interval (level 50 to 99.9, default 95) or of n standard
//!   errors or deviations (n above 0, default 2). The number may stand in parentheses.
//! - PARETO [(CUM | NOCUM)] [(SIMPLE | STACKED)] takes COUNT, SUM or VALUE only.
//! - BAR (RANGE), LINE (DROP), LINE (DIFFERENCE) and HILO [(SIMPLE | GROUPED)] take a function
//!   specification; the series give the ends of each range.
//! - INTERVAL CI [(level)] | SE [(n)] | STDDEV [(n)]: error bars on a bar or line chart, read
//!   as on ERRORBAR.
//! - PANEL COLVAR=varlist [COLOP=CROSS | NEST] ROWVAR=varlist [ROWOP=CROSS | NEST].
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
    /// Lists on both sides of WITH: a group of points for each pair of a variable on the left
    /// (across) and one on the right (up), every combination or, with `pair`, in order.
    Overlay {
        x: Vec<VarItem>,
        y: Vec<VarItem>,
        pair: bool,
        names: Option<String>,
    },
    /// A variable list without WITH: a cell for each ordered pair of the variables.
    Matrix {
        variables: Vec<VarItem>,
        control: Option<String>,
        names: Option<String>,
    },
    /// `x WITH y WITH z`: one plot with three axes.
    Xyz {
        x: String,
        y: String,
        z: String,
        control: Option<String>,
        names: Option<String>,
    },
    /// Means of the variables by up to two BY variables, with error bars.
    ErrorBar {
        interval: GraphInterval,
        variables: Vec<VarItem>,
        by: Vec<String>,
    },
    /// Bars in descending order, with the cumulative percentage as a line unless `cumulative`
    /// is false (NOCUM).
    Pareto {
        cumulative: bool,
        data: GraphData,
    },
    /// BAR (RANGE): a floating bar between each pair of series.
    Range(GraphData),
    /// LINE (DROP): a line from the lowest to the highest series at each category.
    Drop(GraphData),
    /// LINE (DIFFERENCE): the area between two series.
    Difference(GraphData),
    /// HILO: high, low and optionally close as series.
    Hilo(GraphData),
}

/// What an error bar spans.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphInterval {
    /// A confidence interval of the mean at this level in percent.
    Ci(f64),
    /// This many standard errors of the mean on each side.
    Se(f64),
    /// This many standard deviations on each side.
    StdDev(f64),
}

/// PANEL: a chart for each combination of the categories of these variables.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GraphPanel {
    pub columns: Vec<String>,
    pub rows: Vec<String>,
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
    pub panel: Option<GraphPanel>,
    /// INTERVAL: error bars around the values of a bar or line chart.
    pub interval: Option<GraphInterval>,
}

const CHARTS: [&str; 8] = [
    "BAR",
    "LINE",
    "PIE",
    "HISTOGRAM",
    "SCATTERPLOT",
    "ERRORBAR",
    "PARETO",
    "HILO",
];

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

const NOT_YET: [&str; 1] = ["TEMPLATE"];

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

/// `[BY control] [BY id (NAME | IDENTIFY)]` after the variables of a scatterplot.
fn scatter_by(p: &mut Parser) -> PResult<(Option<String>, Option<String>)> {
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
    Ok((control, names))
}

/// `varlist [WITH varlist [(PAIR)] [WITH varlist]]`, then the BY variables. The lists decide
/// the plot whatever keyword was given: one list is a matrix, one variable on each side of WITH
/// a bivariate plot, longer lists an overlay, three single variables a plot with three axes.
fn scatter(p: &mut Parser) -> PResult<GraphChart> {
    p.eat(Punct::Eq);
    let start = p.pos;
    let mut lists = vec![variables(p)?];
    while p.eat_word("WITH") {
        lists.push(variables(p)?);
    }
    let mut pair = false;
    if lists.len() == 2 && p.eat(Punct::LParen) {
        if !p.eat_keyword("PAIR") {
            return Err(p.unexpected(&["PAIR"]));
        }
        p.expect(Punct::RParen)?;
        pair = true;
    }
    let span = p.span_from(start);
    let (control, names) = scatter_by(p)?;
    let one = |list: &[VarItem]| match list {
        [VarItem::Name(name)] => Some(name.clone()),
        _ => None,
    };
    let bad = |message: &str| Err(SyntaxError::new(span, message));
    match &lists[..] {
        [variables] => {
            if one(variables).is_some() {
                return bad("a scatterplot matrix needs two variables or more");
            }
            Ok(GraphChart::Matrix {
                variables: variables.clone(),
                control,
                names,
            })
        }
        [x, y] => match (one(x), one(y), control) {
            (Some(x), Some(y), control) => Ok(GraphChart::Scatter {
                x,
                y,
                control,
                names,
            }),
            (_, _, Some(_)) => bad(
                "an overlay scatterplot takes no control variable; its pairs are told apart \
                 by color",
            ),
            (_, _, None) => Ok(GraphChart::Overlay {
                x: x.clone(),
                y: y.clone(),
                pair,
                names,
            }),
        },
        [x, y, z] => match (one(x), one(y), one(z)) {
            (Some(x), Some(y), Some(z)) => Ok(GraphChart::Xyz {
                x,
                y,
                z,
                control,
                names,
            }),
            _ => bad("a three-axis scatterplot takes one variable on each axis"),
        },
        _ => bad("a scatterplot has at most three axes"),
    }
}

/// `[(CI [level] | STERROR [n] | SE [n] | STDDEV [n])]` after ERRORBAR.
fn interval(p: &mut Parser) -> PResult<GraphInterval> {
    if !p.eat(Punct::LParen) {
        return Ok(GraphInterval::Ci(95.0));
    }
    let interval = interval_spec(p)?;
    p.expect(Punct::RParen)?;
    Ok(interval)
}

/// `CI [level] | STERROR [n] | SE [n] | STDDEV [n]`, the number bare or in parentheses: the
/// interval of ERRORBAR and of INTERVAL.
fn interval_spec(p: &mut Parser) -> PResult<GraphInterval> {
    let keywords = ["CI", "STERROR", "SE", "STDDEV"];
    let Some(k) = keywords.iter().position(|k| p.at_keyword(k)) else {
        return Err(p.unexpected(&keywords));
    };
    p.pos += 1;
    let inner = p.eat(Punct::LParen);
    let start = p.pos;
    let given = p.number();
    if inner {
        p.expect(Punct::RParen)?;
    }
    let bad = |message: String| Err(SyntaxError::new(p.span_from(start), message));
    Ok(match (k, given) {
        (0, None) => GraphInterval::Ci(95.0),
        (0, Some(level)) if (50.0..=99.9).contains(&level) => GraphInterval::Ci(level),
        (0, Some(level)) => {
            return bad(format!(
                "CI needs a level from 50 to 99.9 percent, got {level}"
            ));
        }
        (_, Some(n)) if n <= 0.0 => {
            return bad(format!("{} needs a number above 0, got {n}", keywords[k]));
        }
        (3, n) => GraphInterval::StdDev(n.unwrap_or(2.0)),
        (_, n) => GraphInterval::Se(n.unwrap_or(2.0)),
    })
}

/// The variables of a PANEL keyword: names up to the next `KEYWORD =` or the end.
fn panel_variables(p: &mut Parser) -> PResult<Vec<String>> {
    p.eat(Punct::Eq);
    let mut names = Vec::new();
    while matches!(p.peek(), Some(TokenKind::Word(_)))
        && p.peek_at(1) != Some(&TokenKind::Punct(Punct::Eq))
    {
        names.push(p.name("a variable")?);
        p.eat(Punct::Comma);
    }
    if names.is_empty() {
        return Err(p.unexpected(&["a variable"]));
    }
    Ok(names)
}

/// `COLVAR=varlist [COLOP=CROSS | NEST] ROWVAR=varlist [ROWOP=CROSS | NEST]`, at least one of
/// COLVAR and ROWVAR. A panel is a chart of its own, so CROSS and NEST both give a chart per
/// combination that has cases.
fn panel(p: &mut Parser) -> PResult<GraphPanel> {
    let mut panel = GraphPanel::default();
    let keywords = ["COLVAR", "ROWVAR", "COLOP", "ROWOP"];
    while !at_end(p) {
        let Some(k) = keywords.iter().position(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(&keywords));
        };
        p.pos += 1;
        match k {
            0 => panel.columns = panel_variables(p)?,
            1 => panel.rows = panel_variables(p)?,
            _ => {
                p.eat(Punct::Eq);
                if !p.eat_keyword("CROSS") && !p.eat_keyword("NEST") {
                    return Err(p.unexpected(&["CROSS", "NEST"]));
                }
            }
        }
    }
    if panel.columns.is_empty() && panel.rows.is_empty() {
        return Err(p.unexpected(&["COLVAR", "ROWVAR"]));
    }
    Ok(panel)
}

/// PARETO allows COUNT (or a bare variable), SUM and VALUE.
fn pareto_data(p: &mut Parser) -> PResult<GraphData> {
    let start = p.pos;
    let data = data(p)?;
    if let GraphData::Summary { functions, .. } = &data
        && functions
            .iter()
            .any(|f| !matches!(f.function, GraphFunction::Count | GraphFunction::Sum))
    {
        let message = "PARETO takes COUNT, SUM or VALUE";
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    Ok(data)
}

pub(crate) fn graph(p: &mut Parser) -> PResult<Command> {
    let mut title = Vec::new();
    let mut subtitle = None;
    let mut footnote = Vec::new();
    let mut chart: Option<GraphChart> = None;
    let mut panel_given = None;
    let mut bars = None;
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
            "PANEL" => {
                panel_given = Some(panel(p)?);
                None
            }
            "BAR" => {
                let allowed = ["SIMPLE", "GROUPED", "STACKED", "RANGE"];
                let k = chart_keyword(p, "BAR", &allowed, &[])?;
                Some(match k {
                    Some(3) => GraphChart::Range(data(p)?),
                    _ => GraphChart::Bar {
                        stacked: k == Some(2),
                        data: data(p)?,
                    },
                })
            }
            "LINE" => {
                let allowed = ["SIMPLE", "MULTIPLE", "AREA", "DROP", "DIFFERENCE"];
                let k = chart_keyword(p, "LINE", &allowed, &[])?;
                Some(match k {
                    Some(3) => GraphChart::Drop(data(p)?),
                    Some(4) => GraphChart::Difference(data(p)?),
                    _ => GraphChart::Line {
                        area: k == Some(2),
                        data: data(p)?,
                    },
                })
            }
            "HILO" => {
                chart_keyword(p, "HILO", &["SIMPLE", "GROUPED"], &[])?;
                Some(GraphChart::Hilo(data(p)?))
            }
            "ERRORBAR" => {
                let interval = interval(p)?;
                p.eat(Punct::Eq);
                let variables = variables(p)?;
                let by = by_list(p, 2)?;
                Some(GraphChart::ErrorBar {
                    interval,
                    variables,
                    by,
                })
            }
            "PARETO" => {
                let mut cumulative = true;
                while p.eat(Punct::LParen) {
                    let keywords = ["CUM", "NOCUM", "SIMPLE", "STACKED"];
                    let Some(k) = keywords.iter().position(|k| p.at_keyword(k)) else {
                        return Err(p.unexpected(&keywords));
                    };
                    p.pos += 1;
                    p.expect(Punct::RParen)?;
                    if k < 2 {
                        cumulative = k == 0;
                    }
                }
                Some(GraphChart::Pareto {
                    cumulative,
                    data: pareto_data(p)?,
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
            "INTERVAL" => {
                p.eat(Punct::Eq);
                bars = Some(interval_spec(p)?);
                None
            }
            _ => {
                let kinds = ["BIVARIATE", "OVERLAY", "MATRIX", "XYZ"];
                chart_keyword(p, "SCATTERPLOT", &kinds, &[])?;
                Some(scatter(p)?)
            }
        };
        if let Some(made) = made {
            if chart.is_some() {
                let message = "GRAPH draws one chart; give only one of BAR, LINE, PIE, \
                               HISTOGRAM, SCATTERPLOT, ERRORBAR, PARETO and HILO";
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
        panel: panel_given,
        interval: bars,
    })))
}
