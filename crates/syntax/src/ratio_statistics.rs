//! Parser of RATIO STATISTICS: descriptive statistics of the ratio of two variables.
//!
//! Rules from the Command Syntax Reference, in our words: the numerator comes first, WITH names
//! the denominator and BY an optional group variable, followed by (ASCENDING), (DESCENDING) or
//! (NOSORT); ASCENDING is the default. MISSING is EXCLUDE (default) or INCLUDE. PRINT and
//! OUTFILE('file or dataset') take the same statistics: AAD, BCOC with one or more (low, high)
//! intervals, CIN with an optional level from 0 up to 100 (default 95), COD, MAX, MDCOV, MEAN,
//! MEDIAN, MIN, MNCOV, PRD, RANGE, STDDEV, WCOC with percentages strictly between 0 and 100, and
//! WGTMEAN. A subcommand without content is ignored; a repeated subcommand keeps the last one.

use crate::ast::Command;
use crate::error::SyntaxError;
use crate::lexer::Punct;
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

#[derive(Debug, Clone, PartialEq)]
pub struct RatioStatistics {
    pub numerator: String,
    pub denominator: String,
    pub group: Option<(String, RatioOrder)>,
    /// `MISSING=INCLUDE`
    pub include: bool,
    /// None: only the case processing summary.
    pub print: Option<RatioRequest>,
    /// The file or dataset name and what goes into it.
    pub outfile: Option<(String, RatioRequest)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatioOrder {
    Ascending,
    Descending,
    NoSort,
}

/// The statistics of PRINT or OUTFILE.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RatioRequest {
    pub aad: bool,
    /// `BCOC`: intervals (low, high).
    pub bcoc: Vec<(f64, f64)>,
    /// `CIN`: the confidence level in percent.
    pub cin: Option<f64>,
    pub cod: bool,
    pub max: bool,
    pub mdcov: bool,
    pub mean: bool,
    pub median: bool,
    pub min: bool,
    pub mncov: bool,
    pub prd: bool,
    pub range: bool,
    pub stddev: bool,
    /// `WCOC`: percentages of the median.
    pub wcoc: Vec<f64>,
    pub wgtmean: bool,
}

const STATISTICS: [&str; 15] = [
    "AAD", "BCOC", "CIN", "COD", "MAX", "MDCOV", "MEAN", "MEDIAN", "MIN", "MNCOV", "PRD", "RANGE",
    "STDDEV", "WCOC", "WGTMEAN",
];

fn number(p: &mut Parser) -> PResult<(f64, usize)> {
    let start = p.pos;
    match p.number() {
        Some(value) => Ok((value, start)),
        None => Err(p.unexpected(&["a number"])),
    }
}

/// `low[,] high)`: the rest of one interval after its '('.
fn interval(p: &mut Parser) -> PResult<(f64, f64)> {
    let (low, start) = number(p)?;
    p.eat(Punct::Comma);
    let (high, _) = number(p)?;
    if low > high {
        let message = format!("the interval {low} to {high} of BCOC runs backwards");
        return Err(SyntaxError::new(p.span_from(start), message));
    }
    p.expect(Punct::RParen)?;
    Ok((low, high))
}

/// The statistics up to the end of the subcommand; None when there are none.
fn request(p: &mut Parser) -> PResult<Option<RatioRequest>> {
    let mut r = RatioRequest::default();
    let mut any = false;
    while !at_end(p) {
        let Some(i) = STATISTICS.iter().position(|k| p.at_keyword(k)) else {
            return Err(p.unexpected(&STATISTICS));
        };
        p.pos += 1;
        any = true;
        match STATISTICS[i] {
            "AAD" => r.aad = true,
            "BCOC" => {
                p.expect(Punct::LParen)?;
                if p.eat(Punct::LParen) {
                    // ((low, high) (low, high) ...)
                    loop {
                        r.bcoc.push(interval(p)?);
                        p.eat(Punct::Comma);
                        if !p.eat(Punct::LParen) {
                            break;
                        }
                    }
                    p.expect(Punct::RParen)?;
                } else {
                    // (low, high) [(low, high) ...]
                    r.bcoc.push(interval(p)?);
                    while p.eat(Punct::LParen) {
                        r.bcoc.push(interval(p)?);
                    }
                }
            }
            "CIN" => {
                let mut level = 95.0;
                if p.eat(Punct::LParen) {
                    let (value, start) = number(p)?;
                    if !(0.0..100.0).contains(&value) {
                        let message =
                            format!("CIN must lie from 0 up to (not including) 100, got {value}");
                        return Err(SyntaxError::new(p.span_from(start), message));
                    }
                    level = value;
                    p.expect(Punct::RParen)?;
                }
                r.cin = Some(level);
            }
            "COD" => r.cod = true,
            "MAX" => r.max = true,
            "MDCOV" => r.mdcov = true,
            "MEAN" => r.mean = true,
            "MEDIAN" => r.median = true,
            "MIN" => r.min = true,
            "MNCOV" => r.mncov = true,
            "PRD" => r.prd = true,
            "RANGE" => r.range = true,
            "STDDEV" => r.stddev = true,
            "WCOC" => {
                p.expect(Punct::LParen)?;
                loop {
                    let (value, start) = number(p)?;
                    if !(value > 0.0 && value < 100.0) {
                        let message =
                            format!("WCOC percentages must lie between 0 and 100, got {value}");
                        return Err(SyntaxError::new(p.span_from(start), message));
                    }
                    r.wcoc.push(value);
                    p.eat(Punct::Comma);
                    if p.eat(Punct::RParen) {
                        break;
                    }
                }
            }
            _ => r.wgtmean = true,
        }
        p.eat(Punct::Comma);
    }
    Ok(any.then_some(r))
}

pub(crate) fn ratio_statistics(p: &mut Parser) -> PResult<Command> {
    let numerator = p.name("the numerator variable")?;
    if !p.eat_word("WITH") {
        return Err(p
            .unexpected(&["WITH"])
            .hint("RATIO STATISTICS starts with numerator WITH denominator"));
    }
    let denominator = p.name("the denominator variable")?;
    let group = if p.eat_word("BY") {
        let name = p.name("the group variable")?;
        let mut order = RatioOrder::Ascending;
        if p.eat(Punct::LParen) {
            order = if p.eat_keyword("ASCENDING") {
                RatioOrder::Ascending
            } else if p.eat_keyword("DESCENDING") {
                RatioOrder::Descending
            } else if p.eat_keyword("NOSORT") {
                RatioOrder::NoSort
            } else {
                return Err(p.unexpected(&["ASCENDING", "DESCENDING", "NOSORT"]));
            };
            p.expect(Punct::RParen)?;
        }
        Some((name, order))
    } else {
        None
    };
    let mut command = RatioStatistics {
        numerator,
        denominator,
        group,
        include: false,
        print: None,
        outfile: None,
    };
    while p.eat(Punct::Slash) {
        if p.eat_keyword("MISSING") {
            p.eat(Punct::Eq);
            if p.eat_keyword("INCLUDE") {
                command.include = true;
            } else if p.eat_keyword("EXCLUDE") {
                command.include = false;
            } else if !at_end(p) {
                return Err(p.unexpected(&["EXCLUDE", "INCLUDE"]));
            }
        } else if p.eat_keyword("PRINT") {
            p.eat(Punct::Eq);
            if let Some(r) = request(p)? {
                command.print = Some(r);
            }
        } else if p.eat_keyword("OUTFILE") {
            p.expect(Punct::LParen)?;
            let file = p.string("a quoted file or dataset name")?;
            p.expect(Punct::RParen)?;
            p.eat(Punct::Eq);
            if let Some(r) = request(p)? {
                command.outfile = Some((file, r));
            }
        } else {
            return Err(p.subcommand_error("RATIO STATISTICS", &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
    }
    p.finish(&["'/'"])?;
    Ok(Command::RatioStatistics(Box::new(command)))
}
