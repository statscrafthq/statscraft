//! Parsers of the time series models and their ranges: USE and PREDICT (the historical and
//! the forecast period), EXSMOOTH (exponential smoothing) and ARIMA.
//!
//! Rules noted from the Command Syntax Reference:
//! - USE and PREDICT: `[start] [THRU [end]]`. A start or end is a case number or DATE keywords
//!   with their values (`Y 1990 M 4`); the two kinds do not mix. FIRST and LAST (PREDICT:
//!   END) stand for the ends of the file. USE ALL, or THRU alone, uses every case; USE also
//!   takes /PERMANENT.
//! - EXSMOOTH: `[VARIABLES=] series`, MODEL with one of NN (SINGLE), NA, NM, LN (HOLT), LA,
//!   LM (WINTERS), EN, EA, EM, DN, DA, DM (of several the first counts), PERIOD=n,
//!   SEASFACT=(values) or a variable, ALPHA, GAMMA, DELTA and PHI as a value or
//!   GRID[(start, end, step)], INITIAL=CALCULATE or (start, trend), APPLY [='name'].
//!   VARIABLES once; any other subcommand may repeat and the last counts.
//! - ARIMA: `[VARIABLES=] dependent [WITH regressors]`, MODEL [(p,d,q)[(sp,sd,sq)[period]]]
//!   with CONSTANT or NOCONSTANT and NOLOG, LG10 (LOG) or LN anywhere on it (the last of each
//!   kind counts), P, Q, SP and SQ as n (lags 1 to n) or (lags), D and SD as n, initial values
//!   AR, MA, SAR, SMA, REG (lists) and CON (one value), MXITER, MXLAMB, SSQPCT, PAREPS,
//!   CINPCT, APPLY [='name'] [SPECIFICATIONS | INITIAL | FIT], FORECAST=EXACT | CLS |
//!   AUTOINIT.
//! - TSPLOT and CASEPLOT: `VARIABLES= series`, DIFF[=n] and SDIFF[=n] (default 1), PERIOD=n,
//!   LN or NOLOG, ID=variable, MARK=variable or date, SPLIT=UNIFORM | SCALE, FORMAT with
//!   NOFILL or BOTTOM (CASEPLOT: LEFT), NOREFERENCE or REFERENCE[(value)], NOJOIN, JOIN or
//!   HILO, APPLY [='name'].

use crate::ast::{CasePoint, Command, UseRange, VarItem};
use crate::error::SyntaxError;
use crate::glm::list_until;
use crate::lexer::{Punct, TokenKind};
use crate::output::date_unit;
use crate::parser::{PResult, Parser};
use crate::procedures::at_end;

/// One start or end of a range: None for FIRST, LAST, END or nothing.
fn case_point(p: &mut Parser) -> PResult<Option<CasePoint>> {
    if p.eat_keyword("FIRST") || p.eat_keyword("LAST") || p.eat_word("END") {
        return Ok(None);
    }
    let start = p.pos;
    if let Some(n) = p.number() {
        if n < 1.0 || n.fract() != 0.0 {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("a case number is a whole number from 1, got {n}"),
            ));
        }
        return Ok(Some(CasePoint::Case(n as u64)));
    }
    let mut parts = Vec::new();
    while let Some(TokenKind::Word(word)) = p.peek() {
        if word.eq_ignore_ascii_case("THRU") {
            break;
        }
        let Some(unit) = date_unit(word) else {
            return Err(p.unexpected(&["a case number", "a DATE keyword such as YEAR or MONTH"]));
        };
        p.pos += 1;
        p.eat(Punct::Eq);
        let Some(value) = p.number() else {
            return Err(p.unexpected(&["the value of the DATE keyword"]));
        };
        if parts.iter().any(|(u, _)| *u == unit) {
            return Err(SyntaxError::new(
                p.span_from(start),
                format!("{} is given twice", unit.name()),
            ));
        }
        parts.push((unit, value));
        p.eat(Punct::Comma);
    }
    Ok((!parts.is_empty()).then_some(CasePoint::Date(parts)))
}

/// `[start] [THRU [end]]` of USE and PREDICT; whether THRU was written.
fn range(p: &mut Parser) -> PResult<(UseRange, bool)> {
    let start = p.pos;
    let first = case_point(p)?;
    let thru = p.eat_keyword("THRU");
    let last = if thru { case_point(p)? } else { None };
    if matches!(
        (&first, &last),
        (Some(CasePoint::Case(_)), Some(CasePoint::Date(_)))
            | (Some(CasePoint::Date(_)), Some(CasePoint::Case(_)))
    ) {
        return Err(SyntaxError::new(
            p.span_from(start),
            "a range is given either by case numbers or by dates, not both",
        ));
    }
    if let (Some(CasePoint::Case(a)), Some(CasePoint::Case(b))) = (&first, &last)
        && b < a
    {
        return Err(SyntaxError::new(
            p.span_from(start),
            "the end of the range comes before its start",
        ));
    }
    let range = UseRange {
        first,
        last,
        permanent: false,
    };
    Ok((range, thru))
}

/// `USE ALL`, `USE THRU`, `USE [start] [THRU [end]] [/PERMANENT]`.
pub(crate) fn use_command(p: &mut Parser) -> PResult<Command> {
    if p.eat_word("ALL") {
        p.finish(&[])?;
        return Ok(Command::Use(None));
    }
    let (mut range, _) = range(p)?;
    range.permanent = p.eat(Punct::Slash) && {
        if !p.eat_keyword("PERMANENT") {
            return Err(p.unexpected(&["PERMANENT"]));
        }
        true
    };
    p.finish(&["THRU", "/PERMANENT"])?;
    if range.first.is_none() && range.last.is_none() && !range.permanent {
        return Ok(Command::Use(None));
    }
    Ok(Command::Use(Some(range)))
}

pub(crate) fn predict(p: &mut Parser) -> PResult<Command> {
    let (range, thru) = range(p)?;
    p.finish(&["THRU"])?;
    if range.first.is_none() && range.last.is_none() && !thru {
        return Err(p.unexpected(&["a start", "THRU"]));
    }
    Ok(Command::Predict(range))
}

// ---------------------------------------------------------------- EXSMOOTH

/// The trend component of an EXSMOOTH model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trend {
    None,
    Linear,
    Exponential,
    Damped,
}

/// The seasonal component of an EXSMOOTH model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seasonality {
    None,
    Additive,
    Multiplicative,
}

const SMOOTHING_MODELS: [(&str, Trend, Seasonality); 15] = [
    ("NN", Trend::None, Seasonality::None),
    ("SINGLE", Trend::None, Seasonality::None),
    ("NA", Trend::None, Seasonality::Additive),
    ("NM", Trend::None, Seasonality::Multiplicative),
    ("LN", Trend::Linear, Seasonality::None),
    ("HOLT", Trend::Linear, Seasonality::None),
    ("LA", Trend::Linear, Seasonality::Additive),
    ("LM", Trend::Linear, Seasonality::Multiplicative),
    ("WINTERS", Trend::Linear, Seasonality::Multiplicative),
    ("EN", Trend::Exponential, Seasonality::None),
    ("EA", Trend::Exponential, Seasonality::Additive),
    ("EM", Trend::Exponential, Seasonality::Multiplicative),
    ("DN", Trend::Damped, Seasonality::None),
    ("DA", Trend::Damped, Seasonality::Additive),
    ("DM", Trend::Damped, Seasonality::Multiplicative),
];

/// The two-letter name of a model, such as "LA".
pub fn smoothing_model_name(trend: Trend, season: Seasonality) -> &'static str {
    SMOOTHING_MODELS
        .iter()
        .find(|(name, t, s)| *t == trend && *s == season && name.len() == 2)
        .expect("listed")
        .0
}

/// A smoothing parameter: one value, or a grid from start to end by step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Smoothing {
    Value(f64),
    /// None: the default grid of the parameter.
    Grid(Option<(f64, f64, f64)>),
}

/// The initial seasonal factors.
#[derive(Debug, Clone, PartialEq)]
pub enum SeasonalFactors {
    Values(Vec<f64>),
    Variable(String),
}

/// EXSMOOTH as written: None for what the command leaves out, so that APPLY can fill it in.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Exsmooth {
    pub variables: Vec<VarItem>,
    pub apply: Option<Option<String>>,
    pub model: Option<(Trend, Seasonality)>,
    pub period: Option<u32>,
    pub seasfact: Option<SeasonalFactors>,
    pub alpha: Option<Smoothing>,
    pub gamma: Option<Smoothing>,
    pub delta: Option<Smoothing>,
    pub phi: Option<Smoothing>,
    /// Some(None): INITIAL=CALCULATE; Some(Some((start, trend))): given values.
    pub initial: Option<Option<(f64, f64)>>,
}

fn error(p: &Parser, start: usize, message: impl Into<String>) -> SyntaxError {
    SyntaxError::new(p.span_from(start), message.into())
}

fn number(p: &mut Parser, what: &str) -> PResult<f64> {
    p.number().ok_or_else(|| p.unexpected(&[what]))
}

/// A whole number of at least `low`.
fn whole(p: &mut Parser, low: u32, what: &str) -> PResult<u32> {
    let start = p.pos;
    match p.number() {
        Some(n) if n >= f64::from(low) && n.fract() == 0.0 && n <= f64::from(u32::MAX) => {
            Ok(n as u32)
        }
        Some(n) => Err(error(
            p,
            start,
            format!("{what} takes a whole number of at least {low}, got {n}"),
        )),
        None => Err(p.unexpected(&["a whole number"])),
    }
}

/// Numbers separated by commas or blanks, in parentheses when `parens` (else optional).
fn numbers(p: &mut Parser, parens: bool, what: &str) -> PResult<Vec<f64>> {
    let open = p.eat(Punct::LParen);
    if parens && !open {
        return Err(p.unexpected(&["'('"]));
    }
    let mut values = Vec::new();
    loop {
        p.eat(Punct::Comma);
        match p.number() {
            Some(v) => values.push(v),
            None => break,
        }
    }
    if open {
        p.expect(Punct::RParen)?;
    }
    if values.is_empty() {
        return Err(p.unexpected(&[what]));
    }
    Ok(values)
}

/// ALPHA, GAMMA, DELTA or PHI after the name: a value or GRID with an optional range.
fn smoothing(p: &mut Parser, name: &str, open: bool) -> PResult<Smoothing> {
    p.eat(Punct::Eq);
    let inside = |v: f64| {
        if open {
            v > 0.0 && v < 1.0
        } else {
            (0.0..=1.0).contains(&v)
        }
    };
    let bounds = if open {
        "above 0 and below 1"
    } else {
        "from 0 to 1"
    };
    let start = p.pos;
    if p.eat_keyword("GRID") {
        if !p.at(Punct::LParen) {
            return Ok(Smoothing::Grid(None));
        }
        let values = numbers(p, true, "the start, end and step of the grid")?;
        let [from, to, step] = values[..] else {
            return Err(error(
                p,
                start,
                format!("GRID of {name} takes three values: start, end and step"),
            ));
        };
        if !inside(from) || !inside(to) {
            return Err(error(
                p,
                start,
                format!("the grid of {name} starts and ends {bounds}"),
            ));
        }
        if to < from {
            return Err(error(
                p,
                start,
                format!("the grid of {name} ends at or above its start"),
            ));
        }
        if step <= 0.0 || (to > from && step > to - from) {
            return Err(error(
                p,
                start,
                format!(
                    "the step of the grid of {name} is above 0 and at most its end less its start"
                ),
            ));
        }
        return Ok(Smoothing::Grid(Some((from, to, step))));
    }
    let v = number(p, "a value or GRID")?;
    if !inside(v) {
        return Err(error(p, start, format!("{name} is {bounds}")));
    }
    Ok(Smoothing::Value(v))
}

fn apply_name(p: &mut Parser) -> PResult<Option<String>> {
    p.eat(Punct::Eq);
    Ok(match p.peek() {
        Some(TokenKind::Str(_)) => Some(p.string("a model name")?),
        _ => None,
    })
}

/// Whether the command continues with its variable list here, without a slash.
fn at_variables(p: &Parser) -> bool {
    if p.at_keyword("VARIABLES") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
        return true;
    }
    matches!(p.peek(), Some(TokenKind::Word(_))) && !p.at_keyword("APPLY")
}

/// Runs `sub` for every subcommand; the leading variable list goes to `variables`.
fn subcommands(
    p: &mut Parser,
    mut variables: impl FnMut(&mut Parser) -> PResult<()>,
    mut sub: impl FnMut(&mut Parser) -> PResult<bool>,
    name: &str,
) -> PResult<()> {
    let mut first = true;
    let mut seen_variables = false;
    loop {
        if !first && !p.eat(Punct::Slash) {
            break;
        }
        if first && p.eat(Punct::Slash) {
            first = false;
        }
        let start = p.pos;
        if (first && at_variables(p)) || p.at_keyword("VARIABLES") {
            if seen_variables {
                return Err(error(p, start, "VARIABLES is given only once"));
            }
            if p.at_keyword("VARIABLES") && p.peek_at(1) == Some(&TokenKind::Punct(Punct::Eq)) {
                p.pos += 2;
            }
            variables(p)?;
            seen_variables = true;
        } else if !sub(p)? {
            return Err(p.subcommand_error(name, &[]));
        }
        if !at_end(p) {
            return Err(p.unexpected(&["'/'", "end of command"]));
        }
        first = false;
    }
    p.finish(&["'/'"])
}

pub(crate) fn exsmooth(p: &mut Parser) -> PResult<Command> {
    let mut c = Exsmooth::default();
    let mut variables = Vec::new();
    subcommands(
        p,
        |p| {
            variables = list_until(p, &[])?;
            Ok(())
        },
        |p| {
            if p.eat_keyword("APPLY") {
                c.apply = Some(apply_name(p)?);
            } else if p.eat_keyword("MODEL") {
                p.eat(Punct::Eq);
                let mut model = None;
                while !at_end(p) {
                    let found = SMOOTHING_MODELS.iter().find(|(name, _, _)| {
                        if name.len() == 2 {
                            matches!(p.peek(), Some(TokenKind::Word(w)) if w.eq_ignore_ascii_case(name))
                        } else {
                            p.at_keyword(name)
                        }
                    });
                    let Some((_, t, s)) = found else {
                        return Err(p.unexpected(&["a model such as NN, LA or WINTERS"]));
                    };
                    p.pos += 1;
                    // of several keywords the first counts
                    model.get_or_insert((*t, *s));
                    p.eat(Punct::Comma);
                }
                c.model = Some(model.ok_or_else(|| p.unexpected(&["a model"]))?);
            } else if p.eat_keyword("PERIOD") {
                p.eat(Punct::Eq);
                c.period = Some(whole(p, 1, "PERIOD")?);
            } else if p.eat_keyword("SEASFACT") {
                p.eat(Punct::Eq);
                c.seasfact = Some(if p.at(Punct::LParen) {
                    SeasonalFactors::Values(numbers(p, true, "the seasonal factors")?)
                } else {
                    SeasonalFactors::Variable(p.name("a variable name")?)
                });
            } else if p.eat_keyword("ALPHA") {
                c.alpha = Some(smoothing(p, "ALPHA", false)?);
            } else if p.eat_keyword("GAMMA") {
                c.gamma = Some(smoothing(p, "GAMMA", false)?);
            } else if p.eat_keyword("DELTA") {
                c.delta = Some(smoothing(p, "DELTA", false)?);
            } else if p.eat_word("PHI") {
                c.phi = Some(smoothing(p, "PHI", true)?);
            } else if p.eat_keyword("INITIAL") {
                p.eat(Punct::Eq);
                let start = p.pos;
                c.initial = Some(if p.eat_keyword("CALCULATE") {
                    None
                } else {
                    let v = numbers(p, true, "the initial start and trend values")?;
                    let [a, b] = v[..] else {
                        return Err(error(
                            p,
                            start,
                            "INITIAL takes two values: the start and the trend",
                        ));
                    };
                    Some((a, b))
                });
            } else {
                return Ok(false);
            }
            Ok(true)
        },
        "EXSMOOTH",
    )?;
    c.variables = variables;
    if c.variables.is_empty() && c.apply.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "EXSMOOTH needs VARIABLES, or APPLY to use the previous EXSMOOTH's",
        ));
    }
    Ok(Command::Exsmooth(Box::new(c)))
}

// ---------------------------------------------------------------- ARIMA

/// The log transformation of ARIMA.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArimaLog {
    None,
    Log10,
    Ln,
}

/// What APPLY takes from the earlier model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArimaApply {
    /// The specifications only; new initial values.
    Specifications,
    /// The final estimates as initial values.
    Initial,
    /// The final estimates, without estimation.
    Fit,
}

/// How fitted values and forecasts are computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArimaForecast {
    Exact,
    Cls,
    Autoinit,
}

/// ARIMA as written: None for what the command leaves out, so that APPLY and TSET can fill it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Arima {
    /// The dependent series and, after WITH, the regressors.
    pub dependent: Vec<VarItem>,
    pub regressors: Vec<VarItem>,
    pub apply: Option<(Option<String>, ArimaApply)>,
    /// MODEL's (p, d, q).
    pub orders: Option<[u32; 3]>,
    /// MODEL's (sp, sd, sq).
    pub seasonal_orders: Option<[u32; 3]>,
    /// The period after the seasonal orders.
    pub period: Option<u32>,
    pub constant: Option<bool>,
    pub log: Option<ArimaLog>,
    /// P, Q, SP and SQ: the lags to fit.
    pub p: Option<Vec<u32>>,
    pub q: Option<Vec<u32>>,
    pub sp: Option<Vec<u32>>,
    pub sq: Option<Vec<u32>>,
    pub d: Option<u32>,
    pub sd: Option<u32>,
    pub ar: Option<Vec<f64>>,
    pub ma: Option<Vec<f64>>,
    pub sar: Option<Vec<f64>>,
    pub sma: Option<Vec<f64>>,
    pub reg: Option<Vec<f64>>,
    pub con: Option<f64>,
    pub mxiter: Option<u32>,
    pub mxlamb: Option<f64>,
    pub ssqpct: Option<f64>,
    pub pareps: Option<f64>,
    pub cinpct: Option<f64>,
    pub forecast: Option<ArimaForecast>,
}

/// `(p, d, q)` of MODEL.
fn orders(p: &mut Parser) -> PResult<[u32; 3]> {
    p.expect(Punct::LParen)?;
    let mut v = [0; 3];
    for (i, slot) in v.iter_mut().enumerate() {
        if i > 0 {
            p.eat(Punct::Comma);
        }
        *slot = whole(p, 0, "an order of MODEL")?;
    }
    p.expect(Punct::RParen)?;
    Ok(v)
}

/// P, Q, SP or SQ after the name: n for lags 1 to n, or (lags).
fn lags(p: &mut Parser, name: &str) -> PResult<Vec<u32>> {
    p.eat(Punct::Eq);
    let start = p.pos;
    if p.at(Punct::LParen) {
        let values = numbers(p, true, "a lag")?;
        let mut lags = Vec::new();
        for v in values {
            if v < 1.0 || v.fract() != 0.0 || v > 1000.0 {
                return Err(error(
                    p,
                    start,
                    format!("the lags of {name} are whole numbers from 1 to 1000"),
                ));
            }
            if !lags.contains(&(v as u32)) {
                lags.push(v as u32);
            }
        }
        lags.sort_unstable();
        return Ok(lags);
    }
    let n = whole(p, 0, name)?;
    if n > 1000 {
        return Err(error(p, start, format!("{name} is at most 1000")));
    }
    Ok((1..=n).collect())
}

pub(crate) fn arima(p: &mut Parser) -> PResult<Command> {
    let mut c = Arima::default();
    let (mut dependent, mut regressors) = (Vec::new(), Vec::new());
    subcommands(
        p,
        |p| {
            dependent = list_until(p, &["WITH"])?;
            if p.eat_word("WITH") {
                regressors = list_until(p, &[])?;
            }
            Ok(())
        },
        |p| {
            let start = p.pos;
            if p.eat_keyword("APPLY") {
                p.eat(Punct::Eq);
                let name = match p.peek() {
                    Some(TokenKind::Str(_)) => Some(p.string("a model name")?),
                    _ => None,
                };
                let how = if p.eat_keyword("SPECIFICATIONS") {
                    ArimaApply::Specifications
                } else if p.eat_keyword("INITIAL") {
                    ArimaApply::Initial
                } else if p.eat_word("FIT") {
                    ArimaApply::Fit
                } else {
                    ArimaApply::Specifications
                };
                c.apply = Some((name, how));
            } else if p.eat_keyword("MODEL") {
                p.eat(Punct::Eq);
                let (mut any, mut groups) = (false, 0);
                while !at_end(p) {
                    if p.at(Punct::LParen) {
                        let o = orders(p)?;
                        match groups {
                            0 => {
                                c.orders = Some(o);
                                c.seasonal_orders = None;
                                c.period = None;
                            }
                            1 => c.seasonal_orders = Some(o),
                            _ => return Err(p.unexpected(&["CONSTANT", "a log keyword"])),
                        }
                        groups += 1;
                    } else if let Some(n) = p.number() {
                        if groups != 2 || n < 1.0 || n.fract() != 0.0 {
                            return Err(error(
                                p,
                                start,
                                "the period on MODEL is a whole number of at least 1 after the seasonal orders",
                            ));
                        }
                        c.period = Some(n as u32);
                    } else if p.eat_keyword("CONSTANT") {
                        c.constant = Some(true);
                    } else if p.eat_keyword("NOCONSTANT") {
                        c.constant = Some(false);
                    } else if p.eat_keyword("NOLOG") {
                        c.log = Some(ArimaLog::None);
                    } else if p.eat_word("LG10") || p.eat_word("LOG") {
                        c.log = Some(ArimaLog::Log10);
                    } else if p.eat_word("LN") {
                        c.log = Some(ArimaLog::Ln);
                    } else {
                        return Err(p.unexpected(&[
                            "(p,d,q)",
                            "CONSTANT",
                            "NOCONSTANT",
                            "NOLOG",
                            "LG10",
                            "LN",
                        ]));
                    }
                    any = true;
                    p.eat(Punct::Comma);
                }
                if !any {
                    return Err(p.unexpected(&["(p,d,q)", "CONSTANT", "LN"]));
                }
            } else if p.eat_word("P") {
                c.p = Some(lags(p, "P")?);
            } else if p.eat_word("Q") {
                c.q = Some(lags(p, "Q")?);
            } else if p.eat_word("SP") {
                c.sp = Some(lags(p, "SP")?);
            } else if p.eat_word("SQ") {
                c.sq = Some(lags(p, "SQ")?);
            } else if p.eat_word("D") {
                p.eat(Punct::Eq);
                c.d = Some(whole(p, 0, "D")?);
            } else if p.eat_word("SD") {
                p.eat(Punct::Eq);
                c.sd = Some(whole(p, 0, "SD")?);
            } else if p.eat_word("AR") {
                p.eat(Punct::Eq);
                c.ar = Some(numbers(p, false, "a value")?);
            } else if p.eat_word("MA") {
                p.eat(Punct::Eq);
                c.ma = Some(numbers(p, false, "a value")?);
            } else if p.eat_word("SAR") {
                p.eat(Punct::Eq);
                c.sar = Some(numbers(p, false, "a value")?);
            } else if p.eat_word("SMA") {
                p.eat(Punct::Eq);
                c.sma = Some(numbers(p, false, "a value")?);
            } else if p.eat_word("REG") {
                p.eat(Punct::Eq);
                c.reg = Some(numbers(p, false, "a value")?);
            } else if p.eat_word("CON") {
                p.eat(Punct::Eq);
                c.con = Some(number(p, "a value")?);
            } else if p.eat_keyword("MXITER") {
                p.eat(Punct::Eq);
                c.mxiter = Some(whole(p, 0, "MXITER")?);
            } else if p.eat_keyword("MXLAMB") {
                p.eat(Punct::Eq);
                let v = number(p, "a number")?;
                if v <= 0.0 {
                    return Err(error(p, start, "MXLAMB must be above 0"));
                }
                c.mxlamb = Some(v);
            } else if p.eat_keyword("SSQPCT") {
                p.eat(Punct::Eq);
                let v = number(p, "a percentage")?;
                if v <= 0.0 || v > 100.0 {
                    return Err(error(p, start, "SSQPCT is above 0 and at most 100"));
                }
                c.ssqpct = Some(v);
            } else if p.eat_keyword("PAREPS") {
                p.eat(Punct::Eq);
                let v = number(p, "a number")?;
                if v <= 0.0 {
                    return Err(error(p, start, "PAREPS must be above 0"));
                }
                c.pareps = Some(v);
            } else if p.eat_keyword("CINPCT") {
                p.eat(Punct::Eq);
                let v = number(p, "a percentage")?;
                if v <= 0.0 || v >= 100.0 {
                    return Err(error(p, start, "CINPCT is above 0 and below 100"));
                }
                c.cinpct = Some(v);
            } else if p.eat_keyword("FORECAST") {
                p.eat(Punct::Eq);
                c.forecast = Some(if p.eat_keyword("EXACT") {
                    ArimaForecast::Exact
                } else if p.eat_word("CLS") {
                    ArimaForecast::Cls
                } else if p.eat_keyword("AUTOINIT") {
                    ArimaForecast::Autoinit
                } else {
                    return Err(p.unexpected(&["EXACT", "CLS", "AUTOINIT"]));
                });
            } else {
                return Ok(false);
            }
            Ok(true)
        },
        "ARIMA",
    )?;
    if dependent.len() > 1 || matches!(dependent.first(), Some(VarItem::Range(..) | VarItem::All)) {
        return Err(SyntaxError::new(
            p.span(),
            "ARIMA takes one dependent series; put the regressors after WITH",
        ));
    }
    c.dependent = dependent;
    c.regressors = regressors;
    if c.dependent.is_empty() && c.apply.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            "ARIMA needs a dependent series, or APPLY to use the previous ARIMA's",
        ));
    }
    Ok(Command::Arima(Box::new(c)))
}

// ---------------------------------------------------------------- TSPLOT and CASEPLOT

/// Where MARK draws its line: where a variable changes, or at a date.
#[derive(Debug, Clone, PartialEq)]
pub enum PlotMark {
    Variable(String),
    Date(Vec<(crate::output::DateUnit, f64)>),
}

/// The line of FORMAT=REFERENCE.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlotReference {
    None,
    Mean,
    Value(f64),
}

/// How several series are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlotJoin {
    NoJoin,
    Join,
    Hilo,
}

/// TSPLOT or CASEPLOT as written: None for what the command leaves out, for APPLY.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SeriesPlot {
    /// CASEPLOT: the cases run down the vertical axis.
    pub casewise: bool,
    pub variables: Vec<VarItem>,
    pub apply: Option<Option<String>>,
    pub diff: Option<u32>,
    pub sdiff: Option<u32>,
    pub period: Option<u32>,
    pub ln: Option<bool>,
    pub id: Option<String>,
    pub mark: Option<PlotMark>,
    /// SPLIT=SCALE (true) or UNIFORM.
    pub split_scale: Option<bool>,
    /// BOTTOM (TSPLOT) or LEFT (CASEPLOT) against NOFILL.
    pub fill: Option<bool>,
    pub reference: Option<PlotReference>,
    pub join: Option<PlotJoin>,
}

fn series_plot(p: &mut Parser, casewise: bool) -> PResult<Command> {
    let name = if casewise { "CASEPLOT" } else { "TSPLOT" };
    let fill_word = if casewise { "LEFT" } else { "BOTTOM" };
    let mut c = SeriesPlot {
        casewise,
        ..SeriesPlot::default()
    };
    let mut variables = Vec::new();
    subcommands(
        p,
        |p| {
            variables = list_until(p, &[])?;
            Ok(())
        },
        |p| {
            let start = p.pos;
            let count = |p: &mut Parser, what: &str| -> PResult<u32> {
                p.eat(Punct::Eq);
                if at_end(p) {
                    return Ok(1);
                }
                whole(p, 1, what)
            };
            if p.eat_keyword("APPLY") {
                c.apply = Some(apply_name(p)?);
            } else if p.eat_keyword("DIFF") {
                c.diff = Some(count(p, "DIFF")?);
            } else if p.eat_keyword("SDIFF") {
                c.sdiff = Some(count(p, "SDIFF")?);
            } else if p.eat_keyword("PERIOD") {
                p.eat(Punct::Eq);
                c.period = Some(whole(p, 1, "PERIOD")?);
            } else if p.eat_word("LN") {
                c.ln = Some(true);
            } else if p.eat_keyword("NOLOG") {
                c.ln = Some(false);
            } else if p.eat_word("ID") {
                p.eat(Punct::Eq);
                c.id = Some(p.name("a variable name")?);
            } else if p.eat_keyword("MARK") {
                p.eat(Punct::Eq);
                let is_date = matches!(
                    (p.peek(), p.peek_at(1)),
                    (Some(TokenKind::Word(w)), Some(TokenKind::Number(_)) | Some(TokenKind::Punct(Punct::Eq)))
                        if date_unit(w).is_some()
                );
                c.mark = Some(if is_date {
                    match case_point(p)? {
                        Some(CasePoint::Date(parts)) => PlotMark::Date(parts),
                        _ => return Err(error(p, start, "MARK takes a variable or a date")),
                    }
                } else {
                    PlotMark::Variable(p.name("a variable name or a date")?)
                });
            } else if p.eat_keyword("SPLIT") {
                p.eat(Punct::Eq);
                c.split_scale = Some(if p.eat_keyword("SCALE") {
                    true
                } else if p.eat_keyword("UNIFORM") {
                    false
                } else {
                    return Err(p.unexpected(&["UNIFORM", "SCALE"]));
                });
            } else if p.eat_keyword("FORMAT") {
                p.eat(Punct::Eq);
                let mut any = false;
                while !at_end(p) {
                    if p.eat_keyword("NOFILL") {
                        c.fill = Some(false);
                    } else if p.eat_keyword(fill_word) {
                        c.fill = Some(true);
                    } else if p.eat_keyword("NOREFERENCE") {
                        c.reference = Some(PlotReference::None);
                    } else if p.eat_keyword("REFERENCE") {
                        c.reference = Some(if p.eat(Punct::LParen) {
                            let v = number(p, "a value")?;
                            p.expect(Punct::RParen)?;
                            PlotReference::Value(v)
                        } else {
                            PlotReference::Mean
                        });
                    } else if p.eat_keyword("NOJOIN") {
                        c.join = Some(PlotJoin::NoJoin);
                    } else if p.eat_keyword("JOIN") {
                        c.join = Some(PlotJoin::Join);
                    } else if p.eat_keyword("HILO") {
                        c.join = Some(PlotJoin::Hilo);
                    } else {
                        return Err(p.unexpected(&[
                            "NOFILL",
                            fill_word,
                            "NOREFERENCE",
                            "REFERENCE",
                            "NOJOIN",
                            "JOIN",
                            "HILO",
                        ]));
                    }
                    any = true;
                    p.eat(Punct::Comma);
                }
                if !any {
                    return Err(p.unexpected(&["a FORMAT keyword"]));
                }
            } else {
                return Ok(false);
            }
            Ok(true)
        },
        name,
    )?;
    c.variables = variables;
    if c.variables.is_empty() && c.apply.is_none() {
        return Err(SyntaxError::new(
            p.span(),
            format!("{name} needs VARIABLES, or APPLY to use the previous {name}'s"),
        ));
    }
    Ok(Command::SeriesPlot(Box::new(c)))
}

pub(crate) fn tsplot(p: &mut Parser) -> PResult<Command> {
    series_plot(p, false)
}

pub(crate) fn caseplot(p: &mut Parser) -> PResult<Command> {
    series_plot(p, true)
}
